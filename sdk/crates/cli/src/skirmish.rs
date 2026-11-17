//! `skirmish`: run a whole Kessel Ridge match headlessly, AI against AI, and
//! report the winner, the match length, per-player stats and tick timings
//! (S1 "Done when" 2 and 5).
//!
//! The match runs unpaced (as fast as the machine allows) unless `--paced`
//! is given. Tick times are measured around `World::step` and summarised
//! twice: over the whole match and over the "peak" ticks, those with at
//! least [`PEAK_SHARE_PCT`] % of the match's highest live soldier count.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use frontline_content::vehicle::AmmoMechanism;
use frontline_content::{Difficulty, MatchRequest, MatchSetup, PlayerRequest};
use frontline_sim::{
    ArmourEventKind, BlastKind, Outcome, PlayerId, SquadId, World, fired_class, kill_class,
    vehicle_flags,
};
use serde::Serialize;

use crate::report::{Summary, hex, ns, round3};
use crate::s3c_tally::{S3cPlayer, S3cSummary, S3cTally};

/// Ticks with at least this share of the peak soldier count are "peak".
pub const PEAK_SHARE_PCT: usize = 90;

/// AI strength on the command line.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum DifficultyArg {
    Easy,
    #[default]
    Normal,
    Hard,
}

impl From<DifficultyArg> for Difficulty {
    fn from(d: DifficultyArg) -> Difficulty {
        match d {
            DifficultyArg::Easy => Difficulty::Easy,
            DifficultyArg::Normal => Difficulty::Normal,
            DifficultyArg::Hard => Difficulty::Hard,
        }
    }
}

/// How `skirmish` prints its result.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum ReportFormat {
    /// A short human summary.
    #[default]
    Text,
    /// The full [`SkirmishReport`] as pretty JSON on stdout.
    Json,
}

/// Options for [`skirmish`]; also the clap definition of the subcommand.
#[derive(Clone, Debug, clap::Args)]
pub struct SkirmishArgs {
    /// Both players are AI. Required (unless --load): the headless CLI has
    /// no human player.
    #[arg(long, required_unless_present = "load")]
    pub ai_vs_ai: bool,
    /// Match seed.
    #[arg(long, value_name = "N", default_value_t = 1, conflicts_with = "load")]
    pub seed: u64,
    /// AI difficulty for both players.
    #[arg(long, value_enum, default_value_t = DifficultyArg::Normal)]
    pub difficulty: DifficultyArg,
    /// Overrides --difficulty for player 1.
    #[arg(long, value_enum, value_name = "DIFFICULTY")]
    pub difficulty1: Option<DifficultyArg>,
    /// Faction of player 0 (base 0).
    #[arg(long, value_name = "ID", default_value = "ww2:germany")]
    pub faction0: String,
    /// Faction of player 1 (base 1).
    #[arg(long, value_name = "ID", default_value = "ww2:usa")]
    pub faction1: String,
    /// Stop after this many game minutes if nobody has won.
    #[arg(long, value_name = "M", default_value_t = 30)]
    pub minutes: u32,
    /// Sim worker threads (default: all cores). Never changes results.
    #[arg(long, value_name = "T")]
    pub threads: Option<usize>,
    /// Mod directory (default: `mods/ww2` here or in the source tree).
    #[arg(long, value_name = "DIR")]
    pub mods: Option<PathBuf>,
    /// Starting manpower override for both players (stress runs that fill
    /// the pop cap early). Changes the match, so hashes differ.
    #[arg(long, value_name = "MP")]
    pub start_manpower: Option<u32>,
    /// Player 0 starts at base 1 and player 1 at base 0 (balance checks:
    /// separates the map side from the player slot).
    #[arg(long)]
    pub swap_bases: bool,
    /// Run at the real tick rate (sleep to each tick's deadline).
    #[arg(long)]
    pub paced: bool,
    /// Save the full sim state to this file: at --save-at, or at the end of
    /// the run when it is not given or the match ends first.
    #[arg(long, value_name = "FILE")]
    pub save: Option<PathBuf>,
    /// Absolute tick at which to write --save.
    #[arg(long, value_name = "K", requires = "save")]
    pub save_at: Option<u64>,
    /// Continue a saved AI-vs-AI match instead of starting one. The match
    /// options (seed, factions, difficulty, manpower, bases) come from the
    /// save.
    #[arg(
        long,
        value_name = "FILE",
        conflicts_with_all = ["difficulty1", "start_manpower", "swap_bases"]
    )]
    pub load: Option<PathBuf>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = ReportFormat::Text)]
    pub report: ReportFormat,
    /// Print every vehicle's state to stderr every 10 game seconds (S3
    /// diagnostics: position, order, speed, flags, components, target).
    #[arg(long)]
    pub trace_vehicles: bool,
    /// Write the gun trace to this file (S9b diagnostics, `gun_trace.rs`:
    /// armour events and, every 5 game seconds, every vehicle and each
    /// gun's sight lines). Single matches only.
    #[arg(long, value_name = "FILE", conflicts_with = "seeds")]
    pub gun_trace: Option<PathBuf>,
    /// Tuning override (S3b): a partial TOML file merged onto the rules
    /// and defs (`docs/modding/tuning.md`). Repeatable; applied in order.
    /// Changes the match, so hashes differ.
    #[arg(long = "rules-override", value_name = "FILE")]
    pub rules_override: Vec<PathBuf>,
    /// Run this many matches, seeds `--seed .. --seed + N` (S3b A/B
    /// testing), in parallel; prints each match's report unless --summary.
    #[arg(long, value_name = "N", conflicts_with_all = ["load", "save", "trace_vehicles", "paced"])]
    pub seeds: Option<u64>,
    /// With --seeds: play every seed twice, normal and swapped bases.
    #[arg(long, requires = "seeds", conflicts_with = "swap_bases")]
    pub both_bases: bool,
    /// With --seeds: print one compact table over all the matches (wins per
    /// faction and base, length, tanks lost, knock-out causes, HE share,
    /// rockets, satchels, tick p99) instead of each report.
    #[arg(long, requires = "seeds")]
    pub summary: bool,
    /// With --seeds: matches run at once (default: cores / --threads).
    #[arg(long, value_name = "J", requires = "seeds")]
    pub jobs: Option<usize>,
    /// With --seeds and --rules-override: also play the same matches
    /// without the overrides (A) and print both summaries and the change
    /// A -> B of each headline number (one-command A/B test).
    #[arg(long, requires_all = ["seeds", "rules_override"])]
    pub ab: bool,
    /// Also write one JSON object with every number the --summary prints
    /// (per match and totals; `docs/modding/tuning.md`). The normal output is
    /// unchanged. Works for one match, --seeds and --ab.
    #[arg(long, value_name = "FILE")]
    pub json: Option<PathBuf>,
    /// With --seeds and --json: add `sides` (two per-player objects: kills,
    /// losses, rounds fired, buys, works, mines, builder plans, own_blocked)
    /// to each `per_match` entry (B1 diagnosis; read-only, off by default).
    #[arg(long, requires_all = ["seeds", "json"])]
    pub side_detail: bool,
}

impl Default for SkirmishArgs {
    fn default() -> SkirmishArgs {
        SkirmishArgs {
            ai_vs_ai: true,
            seed: 1,
            difficulty: DifficultyArg::Normal,
            difficulty1: None,
            faction0: "ww2:germany".into(),
            faction1: "ww2:usa".into(),
            minutes: 30,
            threads: None,
            mods: None,
            start_manpower: None,
            swap_bases: false,
            save: None,
            save_at: None,
            load: None,
            paced: false,
            report: ReportFormat::Text,
            trace_vehicles: false,
            gun_trace: None,
            rules_override: Vec::new(),
            seeds: None,
            both_bases: false,
            summary: false,
            jobs: None,
            ab: false,
            json: None,
            side_detail: false,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SkirmishReport {
    pub seed: u64,
    /// Tick the run started from (non-zero after --load).
    pub start_tick: u64,
    pub loaded_from: Option<String>,
    pub saved: Option<SavedMatch>,
    pub threads: usize,
    pub paced: bool,
    /// `"won"`, `"draw"` or `"time_limit"`.
    pub result: &'static str,
    pub winner: Option<u8>,
    pub winner_faction: Option<String>,
    pub tick_hz: u64,
    pub ticks: u64,
    pub game_seconds: f64,
    pub limit_minutes: u32,
    pub final_hash: String,
    pub players: Vec<PlayerReport>,
    /// S3c: the map's capture point names, in point order.
    pub point_names: Vec<String>,
    pub totals: Totals,
    /// S3: vehicles, AT teams and kills by weapon class.
    pub armour: ArmourTotals,
    /// S3 "Done when" 5 hard checks (when the rules have vehicles): a
    /// winner, at least one vehicle and one AT team per AI, at least one
    /// penetration. Empty when they all hold.
    pub s3_failures: Vec<String>,
    pub timing: SkirmishTiming,
    pub wall_ms: f64,
}

/// S3 match totals for the skirmish report.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ArmourTotals {
    /// The winner's base index (the map-side split across seeds).
    pub winner_base: Option<u8>,
    pub penetrations: u64,
    pub non_penetrations: u64,
    pub ricochets: u64,
    pub knockouts: u64,
    pub rockets_fired: u64,
    /// Tank losses of both players by cause: knock-outs (`ko_crew`,
    /// `ko_ammo_rack`, `ko_fire` (engine), S3b `ko_ammo_fire`) and S3c
    /// abandonments (`abandoned_<trigger>`).
    pub ko_causes: BTreeMap<String, u32>,
    /// Kills of both players by `kill_class` name.
    pub kills_by: BTreeMap<String, u32>,
    /// (mortar + tank HE kills) / all kills, percent; `he_flag` above 60.
    pub he_share_pct: f64,
    pub he_flag: bool,
}

/// One player's S3 numbers.
#[derive(Clone, Debug, Default, Serialize)]
pub struct PlayerArmour {
    pub vehicles_bought: u32,
    pub vehicles_lost: u32,
    pub bogged: u32,
    pub repaired: u32,
    pub at_teams_bought: u32,
    /// Enemy vehicles this player knocked out, by main gun and by rocket
    /// (credited by the last round that hit them).
    pub ko_by_gun: u32,
    pub ko_by_rocket: u32,
    /// S9b: enemy tanks, assault guns and tank destroyers put out of action
    /// (knocked out or abandoned) when the last round that struck them was
    /// one of this player's served AT guns' (a gun of this player fired that
    /// ammunition at that vehicle within 3 s before the impact, as
    /// `tools/gun_funnel.py` matches them).
    pub gun_armour_out: u32,
    /// Own vehicles put out of action, by cause: knocked out by crew
    /// losses (`ko_crew`, from penetrations), an ammo-rack detonation or
    /// fire; components destroyed (`track`, `engine`, `gun`,
    /// `turret_ring`, `rack_destroyed`).
    pub disabled_by: BTreeMap<String, u32>,
    pub kills_by: BTreeMap<String, u32>,
    /// Manpower lost per manpower killed for tanks and for AT teams. A
    /// soldier or crewman killed is valued at the enemy rifle squad's
    /// cost per man; a vehicle knocked out at its cost.
    pub tank_mp_lost: u32,
    pub tank_mp_killed: u32,
    pub at_mp_lost: u32,
    pub at_mp_killed: u32,
    /// AI counters (`AiCounters`) and AT rockets fired.
    pub escorts_started: u32,
    pub escorts_reached: u32,
    pub escorts_released: u32,
    pub at_shots: u32,
    pub satchels_at_vehicles: u32,
    pub repairs: u32,
    pub retreats: u32,
    pub faces: u32,
    /// S3b AI counters: `SuppressUnits` orders, bound halts, AT-team and
    /// satchel hunts started.
    pub suppress_orders: u32,
    pub bounds: u32,
    pub at_hunts: u32,
    pub satchel_hunts: u32,
    /// S3b baseline, as the firing player: main-gun penetrator hits on
    /// enemy vehicles (any result) and how many penetrated; HE shells on a
    /// plate; rocket hits and penetrations; components rockets destroyed
    /// (`Disabled` events of a rocket); and every main-gun or rocket hit
    /// by the plate it struck (`hull_front`, `turret_front`, ...).
    pub gun_hits: u32,
    pub gun_pens: u32,
    pub he_on_plate: u32,
    pub rocket_hits: u32,
    pub rocket_pens: u32,
    pub rocket_disabled_parts: u32,
    /// S4: of the rocket numbers, the single-shot AT items' (Panzerfaust)
    /// hits, penetrations and knock-outs (abandonments included).
    pub pzf_hits: u32,
    pub pzf_pens: u32,
    pub ko_by_pzf: u32,
    pub hits_by_zone: BTreeMap<String, u32>,
    /// S3c: own vehicles put out of action split into destroyed (a wreck
    /// straight away) and abandoned (the crew bailed out), the
    /// abandonments by trigger (`ammo_fire`, `engine_fire`, `crew`,
    /// `useless`, `under_fire`, `order`), and what became of the
    /// abandoned: `recrewed`, `captured` (by the enemy), `destroyed_later`,
    /// `scrapped`, `standing` at the end. `vehicles_lost` counts both,
    /// once per loss.
    pub vehicles_destroyed: u32,
    pub vehicles_abandoned: u32,
    pub abandon_causes: BTreeMap<String, u32>,
    pub abandoned_fates: BTreeMap<String, u32>,
    /// S3c, as the capturing player: enemy vehicles captured; and own
    /// crewmen alive at bail-outs, out alive, dismissed in base.
    pub captures: u32,
    pub crew_bailed: u32,
    pub crew_out: u32,
    pub crew_home: u32,
    /// S3c Part B AI counters (`AiCounters`): rearm trips, sticky hunts
    /// (kept unseen, throws, ends by reason), second bases, AT stuck hunts
    /// and follows, re-crews, crew retreats, denials.
    pub ai_s3c: BTreeMap<String, u32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SavedMatch {
    pub tick: u64,
    pub path: String,
    pub bytes: usize,
    pub hash: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlayerReport {
    pub player: u8,
    pub faction: String,
    /// S3c: the base it started at (the map side).
    pub base: u8,
    pub difficulty: String,
    /// `tickets`, `wiped` or `surrendered` when the player lost.
    pub defeat: Option<String>,
    pub tickets: u32,
    pub manpower: u32,
    pub kills: u32,
    pub losses: u32,
    /// Squads this player fielded during this run (starters included; after
    /// --load, those alive at the load plus later ones).
    pub squads_fielded: u32,
    pub squads_alive: u32,
    pub soldiers_alive: u32,
    pub points_held: u8,
    /// Point-seconds held over the match.
    pub hold_s: u32,
    pub ai_commands: u32,
    pub ai_rejected: u32,
    pub trenches_planned: u32,
    pub armour: PlayerArmour,
    /// S3c baseline counters (report only, `crate::s3c_tally`).
    pub s3c: S3cPlayer,
    /// S4 counters.
    pub s4: S4Player,
    /// S5a ammunition counters.
    pub s5a: S5aPlayer,
    /// S5b fire-mission counters (`frontline_sim::FmCounters`).
    pub s5b: frontline_sim::FmCounters,
    /// S5b Part B AI fire-mission counters (`AiCounters::fm_*`).
    pub s5b_ai: S5bAi,
    /// S6 per-player account (`frontline_sim::S6Account`), the stock its
    /// HQ and dumps held at the end (g), and the AI's crate runs to tanks.
    pub s6: frontline_sim::S6Account,
    pub s6_stock_held_g: u64,
    pub s6_tank_crate_runs: u32,
    /// Review S6 F2: stranded shells crates the AI's bearers picked up.
    pub s6_crate_pickups: u32,
    /// S7 per-vehicle-type counters and the roster's card prices.
    pub s7: S7Player,
    /// S8a transport counters (`frontline_sim::S8aCounters`) and the
    /// roster's transport cards (short ids).
    pub s8a: frontline_sim::S8aCounters,
    pub s8a_cards: Vec<String>,
    /// S8a AI transport counters: loads, rides, unloads, unloads under
    /// fire, failed moves (`AiCounters::transport_*`).
    pub s8a_ai: [u32; 5],
    /// Review M2: AI ride measurements (`AiCounters::obj_*`): rides that
    /// reached their point, order-to-arrival ticks, straight-walk ticks, lost.
    pub s8a_obj: (u32, u64, u64, u32),
    /// S8b ride attribution (`AiCounters`): board and foot ticks, unloads
    /// exposed, arrived, stuck, under fire.
    pub s8b_ride: [u64; 6],
    /// S8b AI counters: recon stops, backoffs, spots given up, truck moves
    /// failed, flee plans, services started, tanks served, drops ordered;
    /// failed transport moves are in `s8a_ai`.
    pub s8b_ai: [u32; 8],
    /// S9 per-player counters, and the AI's gun counters: unhitches,
    /// guns in cover, gun jobs failed, pits, re-lays, recrew orders.
    pub s9: frontline_sim::S9Counters,
    pub s9_ai: [u32; 7],
    /// S10a per-player fortification account (`FortAccount`) and the
    /// squads fielded from the three S10a cards ([`S10A_CARDS`]).
    pub s10: S10Player,
    /// B1 `--side-detail`: deaths by place ([`DEATH_PLACES`]; zero without it).
    pub death_places: [u32; 6],
    /// S10c-ai: the AI's S10c counters and the exposure seen ([`S10cSeen`]).
    pub s10c: S10cPlayer,
    /// S11a1: the player's `S11Counters` and the AI's S11a1 counters.
    pub s11a1: S11a1Player,
    /// B1-5 `--side-detail`: at [`SQUAD_MINUTES`], own foot squads with a
    /// man alive (no vehicle, not a crew) and own squads on a running
    /// (not parked) AI fort job (`None`: over sooner, or no side detail).
    pub squads_at_min: [Option<[u32; 2]>; 5],
}

/// B1-5: match minutes `--side-detail` samples the squads at.
pub const SQUAD_MINUTES: [u64; 5] = [1, 2, 3, 5, 10];

/// B1-5: player `p`'s foot squads with a man alive and squads on a
/// running AI fort job (read-only).
fn squads_now(g: &frontline_sim::MatchState, p: PlayerId) -> [u32; 2] {
    let s = g.setup();
    let foot = g
        .squads()
        .values()
        .filter(|q| {
            q.owner == p
                && !q.members.is_empty()
                && s.squad_types[q.type_idx as usize].vehicle.is_none()
                && s.type_word(q.type_idx) != "crew"
        })
        .count() as u32;
    let busy = g.ai(p).map_or(0, |a| {
        a.forts.values().filter(|j| !j.parked()).count() as u32
    });
    [foot, busy]
}

/// S10a: the three cards of the mechanics half (short ids; spec 3.4).
pub const S10A_CARDS: [&str; 3] = [
    "ger_sturmpioniergruppe",
    "ger_baupioniergruppe",
    "usa_assault_section",
];

/// S10a-ai: the AI's fort and breach counters in report order (names
/// of [`S10Player::ai`], `docs/slices/S10a-ai.md` 4).
pub const S10_AI: [&str; 14] = [
    "fort_jobs",
    "fort_refused",
    "fort_build_plans",
    "infantry_plans",
    "shelter_moves",
    "material_runs_truck",
    "material_runs_crate",
    "material_runs_walk",
    "breaches_started",
    "breaches_opened",
    "breaches_given_up",
    "breach_smokes",
    "breach_suppress",
    "wire_bypassed",
];

/// S10a numbers of one player (spec 7: all zero in AI play).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S10Player {
    pub fort: frontline_sim::fort::FortAccount,
    pub new_cards_bought: u32,
    /// S10a-ai: the AI counters ([`S10_AI`] order; zero for a human).
    pub ai: [u32; 14],
    /// S10a-ai: works completed by blueprint short id, those without wire
    /// and those over an HQ (roofs).
    pub works_done: BTreeMap<String, u32>,
    pub works_non_wire: u32,
    pub hq_roofs: u32,
    /// S10a-ai: wire sections the other players built; whether this player
    /// built an HQ; the income earned (start + base income per whole
    /// minute, as the AI budgets count it; MP).
    pub enemy_wire_built: u32,
    pub hq_built: bool,
    pub income_mp: u64,
    /// S10b use seen in the match ([`S10bOrdered`], review R13).
    pub s10b_ordered: u32,
    /// S10b-ai: the AI's obstacle counters ([`S10B_AI`] order; zero for a
    /// human and with every `[ai.obstacles]` switch off).
    pub s10b_ai: [u32; 14],
    /// S10b-ai chances seen ([`S10bAiSeen`]): [`CHANCE_LAY`],
    /// [`CHANCE_OBST`], [`CHANCE_BUNKER`].
    pub s10b_chances: u8,
    /// S10b-ai: grams drawn by this player's S10b works and their µMP at
    /// the construction price (R8, T117).
    pub s10b_materials_g: u64,
    pub s10b_materials_micro: u64,
    /// S10b-ai Build D: why no bunker started, how bunker jobs ended
    /// (`frontline_sim::ai::BUNKER_WHY` order; zero for a human).
    pub bunker_why: [u32; 16],
}

/// S10b-ai: the AI's obstacle counters in report order (names of
/// [`S10Player::s10b_ai`], `docs/slices/S10b-ai.md` 3).
pub const S10B_AI: [&str; 14] = [
    "s10b_jobs",
    "s10b_refused",
    "lay_sites",
    "sweep_wanted",
    "sweeps_started",
    "sweeps_done",
    "demolish_wanted",
    "demolitions_started",
    "gaps_opened",
    "demolish_given_up",
    "own_route_rejects",
    "own_blocked",
    "jobs_overdue",
    "refused_reorders",
];

/// S10b-ai chance bits (exposure of T109-T113): the AI chose a lay site or
/// held a job on a mine blueprint; held a job on an obstacle blueprint or
/// rejected an obstacle site by the own route rule; held a bunker job.
/// All come from the AI's own state, so they stay 0 with its switch off.
pub const CHANCE_LAY: u8 = 1;
pub const CHANCE_OBST: u8 = 2;
pub const CHANCE_BUNKER: u8 = 4;

impl S10Player {
    /// The S10b-ai counters of `a` in [`S10B_AI`] order.
    pub fn s10b_ai_counters(a: &frontline_sim::ai::AiCounters) -> [u32; 14] {
        [
            a.s10b_jobs,
            a.s10b_refused,
            a.lay_sites,
            a.sweep_wanted,
            a.sweeps_started,
            a.sweeps_done,
            a.demolish_wanted,
            a.demolitions_started,
            a.gaps_opened,
            a.demolish_given_up,
            a.own_route_rejects,
            a.own_blocked,
            a.jobs_overdue,
            a.refused_reorders,
        ]
    }
}

/// B1 diagnosis (`--side-detail`): where each player's men died, by the
/// fort state at the tick their body appeared ([`DEATH_PLACES`] order).
/// Read-only: it reads the snapshot and the fort state.
pub const DEATH_PLACES: [&str; 6] = [
    "dead",
    "near_enemy_nest",
    "near_enemy_works",
    "near_enemy_wire",
    "near_own_works",
    "near_own_wire",
];
/// Metres: a death this close to a piece counts as near its work.
const DEATH_WORKS_M: f32 = 40.0;
/// Metres: a death this close to a wire section counts as near it.
const DEATH_WIRE_M: f32 = 10.0;

#[derive(Clone, Debug, Default)]
pub struct DeathPlaces {
    seen: BTreeSet<frontline_sim::SimId>,
    pub per: Vec<[u32; 6]>,
}

impl DeathPlaces {
    pub fn observe(&mut self, w: &World) {
        let Some(g) = w.game() else { return };
        let s = g.setup();
        let n = s.players.len();
        if self.per.len() < n {
            self.per.resize(n, [0; 6]);
        }
        let fort = g.fort();
        let d2 = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2);
        let seg = |p: [f32; 2], a: [f32; 2], b: [f32; 2]| {
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l = dx * dx + dy * dy;
            let t = if l > 0.0 {
                (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l).clamp(0.0, 1.0)
            } else {
                0.0
            };
            d2(p, [a[0] + t * dx, a[1] + t * dy])
        };
        for c in &w.snapshot().corpses {
            if !self.seen.insert(c.id) {
                continue;
            }
            let o = c.owner as usize;
            let Some(row) = self.per.get_mut(o) else {
                continue;
            };
            let p = [c.pos[0], c.pos[1]];
            row[0] += 1;
            let (mut en, mut ew, mut ow) = (false, false, false);
            for pc in fort.pieces.values() {
                let Some(f) = s.forts.get(pc.fort as usize) else {
                    continue;
                };
                if f.blocks_vehicles || f.mine_item.is_some() {
                    continue;
                }
                if d2(p, pc.pos.to_f32()) > DEATH_WORKS_M * DEATH_WORKS_M {
                    continue;
                }
                if pc.owner.0 as usize == o {
                    ow = true;
                } else {
                    ew = true;
                    en |= f.id.ends_with("mg_nest");
                }
            }
            let (mut ewire, mut owire) = (false, false);
            for ws in fort.wire.values() {
                if seg(p, ws.a.to_f32(), ws.b.to_f32()) <= DEATH_WIRE_M * DEATH_WIRE_M {
                    if ws.owner.0 as usize == o {
                        owire = true;
                    } else {
                        ewire = true;
                    }
                }
            }
            row[1] += en as u32;
            row[2] += ew as u32;
            row[3] += ewire as u32;
            row[4] += ow as u32;
            row[5] += owire as u32;
        }
    }
}

/// S10b-ai, seen tick by tick: per player the chance bits (AI fort jobs
/// by blueprint class) and the grams S10b works drew (a work's `got_g`
/// rises within a step and restarts at the next one).
#[derive(Clone, Debug, Default)]
pub struct S10bAiSeen {
    got: BTreeMap<u32, u32>,
    pub chances: Vec<u8>,
    pub materials_g: Vec<u64>,
}

impl S10bAiSeen {
    pub fn observe(&mut self, w: &World) {
        let Some(g) = w.game() else { return };
        let s = g.setup();
        let n = s.players.len();
        if self.chances.len() < n {
            self.chances.resize(n, 0);
            self.materials_g.resize(n, 0);
        }
        for (&id, wk) in &g.fort().works {
            let Some(f) = s.forts.get(wk.fort as usize) else {
                continue;
            };
            if !S10bOrdered::s10b_blueprint(f) {
                continue;
            }
            let last = self.got.insert(id, wk.got_g).unwrap_or(0);
            let add = if wk.got_g >= last {
                wk.got_g - last
            } else {
                wk.got_g
            };
            if let Some(m) = self.materials_g.get_mut(wk.owner.0 as usize) {
                *m += add as u64;
            }
        }
        for p in 0..n {
            let Some(ai) = g.ai(PlayerId(p as u8)) else {
                continue;
            };
            let mut bits = 0u8;
            for j in ai.forts.values() {
                if let Some(f) = s.forts.get(j.fort as usize) {
                    if f.mine_item.is_some() {
                        bits |= CHANCE_LAY;
                    } else if f.blocks_vehicles {
                        bits |= CHANCE_OBST;
                    } else if matches!(f.pit, Some(frontline_content::setup::DigProfile::BunkerPit))
                    {
                        bits |= CHANCE_BUNKER;
                    }
                }
            }
            if ai.counters.lay_sites > 0 {
                bits |= CHANCE_LAY;
            }
            if ai.counters.own_route_rejects > 0 {
                bits |= CHANCE_OBST;
            }
            self.chances[p] |= bits;
        }
    }
}

/// S11a1 (docs/slices/S11a1.md 3, 7): the AI's S11a1 counters in report
/// order (names of [`S11a1Player::ai`]).
pub const S11A1_AI: [&str; 14] = [
    "arty_buys",
    "fo_buys",
    "arty_missions",
    "arty_prep_missions",
    "mortar_prep_missions",
    "arty_lifts",
    "arty_budget_holds",
    "arty_reserve_holds",
    "arty_no_observer",
    "fo_posts",
    "fo_post_fails",
    "fo_lost",
    "breach_prep_waits",
    "breach_prep_timeouts",
];

/// S11a1 numbers of one player in one match.
#[derive(Clone, Debug, Default, Serialize)]
pub struct S11a1Player {
    /// The player's infantry-gun counters (saved in the sim).
    pub s11: frontline_sim::fire_mission::S11Counters,
    /// [`S11A1_AI`] order (zero for a human).
    pub ai: [u32; 14],
}

impl S11a1Player {
    /// The S11a1 counters of `a` in [`S11A1_AI`] order.
    pub fn ai_counters(a: &frontline_sim::ai::AiCounters) -> [u32; 14] {
        [
            a.arty_buys,
            a.fo_buys,
            a.arty_missions,
            a.arty_prep_missions,
            a.mortar_prep_missions,
            a.arty_lifts,
            a.arty_budget_holds,
            a.arty_reserve_holds,
            a.arty_no_observer,
            a.fo_posts,
            a.fo_post_fails,
            a.fo_lost,
            a.breach_prep_waits,
            a.breach_prep_timeouts,
        ]
    }
}

/// S11a1 numbers of one faction over a `--seeds` run (sums and per-match
/// tallies of `s11a1_derived`, spec 7). The JSON blocks are
/// [`crate::json_out::s11a1_blocks`].
#[derive(Clone, Debug, Default, Serialize)]
pub struct S11a1Faction {
    pub matches: u32,
    /// Sums of the `S11Counters` arrays (by mode) and scalars.
    pub gun_missions: [u64; 3],
    pub gun_rounds: [u64; 3],
    pub gun_miss_mm: [u64; 3],
    pub masked: u64,
    pub not_ready: u64,
    pub radio_rounds: u64,
    pub low_rounds: u64,
    pub high_rounds: u64,
    pub gun_breaks: u64,
    pub gun_shell_micro: u64,
    pub gun_kills: u64,
    pub guns_bought: u64,
    pub gun_direct_rounds: u64,
    pub near_own_bursts: u64,
    /// [`S11A1_AI`] order.
    pub ai: [u64; 14],
    /// The faction's kills and income (MP) over the run (denominators).
    pub kills: u64,
    pub income_mp: u64,
    /// Matches with a gun bought; of those, with a gun mission round; with
    /// an observer team bought (AI).
    pub matches_with_gun: u32,
    pub matches_gun_fired: u32,
    pub matches_with_fo: u32,
}

impl S11a1Faction {
    /// The sum of AI counter `name` ([`S11A1_AI`]).
    pub fn get(&self, name: &str) -> u64 {
        S11A1_AI
            .iter()
            .position(|k| *k == name)
            .map_or(0, |k| self.ai[k])
    }

    /// Adds one match of one player.
    pub fn add(&mut self, c: &S11a1Player, kills: u32, income_mp: u64) {
        let s = &c.s11;
        self.matches += 1;
        for k in 0..3 {
            self.gun_missions[k] += s.gun_missions[k] as u64;
            self.gun_rounds[k] += s.gun_rounds[k] as u64;
            self.gun_miss_mm[k] += s.gun_miss_mm[k];
        }
        self.masked += s.masked as u64;
        self.not_ready += s.not_ready as u64;
        self.radio_rounds += s.radio_rounds as u64;
        self.low_rounds += s.low_rounds as u64;
        self.high_rounds += s.high_rounds as u64;
        self.gun_breaks += s.gun_breaks as u64;
        self.gun_shell_micro += s.gun_shell_micro;
        self.gun_kills += s.gun_kills as u64;
        self.guns_bought += s.guns_bought as u64;
        self.gun_direct_rounds += s.gun_direct_rounds as u64;
        self.near_own_bursts += s.near_own_bursts as u64;
        for (k, v) in c.ai.iter().enumerate() {
            self.ai[k] += *v as u64;
        }
        self.kills += kills as u64;
        self.income_mp += income_mp;
        let gun = s.guns_bought > 0;
        self.matches_with_gun += u32::from(gun);
        self.matches_gun_fired += u32::from(gun && s.gun_rounds.iter().sum::<u32>() > 0);
        self.matches_with_fo += u32::from(c.ai[1] > 0);
    }
}

/// S10c-ai (docs/slices/S10c-ai.md 3, 7): the AI's S10c counters in report
/// order (names of [`S10cPlayer::ai`]).
pub const S10C_AI: [&str; 12] = [
    "fort_job_fails",
    "truck_give_ups",
    "route_refusals",
    "front_plans",
    "front_calm_plans",
    "front_jobs",
    "front_drops",
    "front_lost",
    "front_resumes",
    "belt_jobs",
    "belt_route_rejects",
    "builder_buys",
];

/// S10c-ai: an enemy this close (m) to an own mine, wire middle or blocking
/// piece "approached" it (spec 7).
pub const S10C_NEAR_M: f32 = 30.0;

/// S10c-ai numbers of one player in one match: the AI's counters and what
/// [`S10cSeen`] saw (read-only, once a second).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S10cPlayer {
    /// [`S10C_AI`] order (zero for a human).
    pub ai: [u32; 12],
    /// `frontline_sim::ai::FRONT_WHY` and `MINE_WANT_WHY` order.
    pub front_why: [u32; 13],
    pub mine_want_why: [u32; 8],
    /// Own mines seen at a sampled second, and of those the ones an enemy
    /// soldier (AP) or enemy vehicle (AT) came within 30 m of.
    pub mines_seen: u32,
    pub mines_approached: u32,
    /// Own wire stood at a sampled second; an enemy soldier came within
    /// 30 m of a section's middle; an enemy vehicle within 30 m of an own
    /// blocking piece.
    pub wire_stood: bool,
    pub wire_approached: bool,
    pub obstacle_approached: bool,
    /// The other AI's `wire_known` held a section of this player; this
    /// player's AI held a section of another player.
    pub wire_seen_by_enemy: bool,
    pub knew_enemy_wire: bool,
    /// A saved front lane of this AI had its FrontAp field laid (Build B).
    pub front_field: bool,
    /// Own works done and own mines seen, by nearest map point (map order).
    pub works_by_point: Vec<u32>,
    pub mines_by_point: Vec<u32>,
    /// Mines seen whose nearest map point is the home point (the point
    /// nearest the layer's base), and the others.
    pub mines_home: u32,
    pub mines_front: u32,
}

impl S10cPlayer {
    /// The S10c counters of `a` in [`S10C_AI`] order.
    pub fn ai_counters(a: &frontline_sim::ai::AiCounters) -> [u32; 12] {
        [
            a.fort_job_fails,
            a.truck_give_ups,
            a.route_refusals,
            a.front_plans,
            a.front_calm_plans,
            a.front_jobs,
            a.front_drops,
            a.front_lost,
            a.front_resumes,
            a.belt_jobs,
            a.belt_route_rejects,
            a.builder_buys,
        ]
    }
}

/// S10c-ai: a unit in the observer's grid (owner, position, vehicle).
type S10cUnit = (u8, [f32; 2], bool);

/// S10c-ai exposure observer (Build A item 4), called once a second
/// (`tick % hz == 0`) and once at the end. Read-only: it reads the entity
/// columns, the fort state and the AI states. Enemy units go into a 30 m
/// grid, so each mine, wire section and piece looks at 9 cells only.
#[derive(Clone, Debug, Default)]
pub struct S10cSeen {
    /// Every mine seen: id -> (owner, nearest point).
    mines: BTreeMap<u32, (u8, u8)>,
    approached: BTreeSet<u32>,
    /// Works alive at the last look: id -> (owner, blueprint, nearest point).
    works: BTreeMap<u32, (u8, u16, u8)>,
    /// `FortAccount::works_done` per player at the last look.
    done: Vec<Vec<u32>>,
    points: Vec<[f32; 2]>,
    home: Vec<u8>,
    pub per: Vec<S10cPlayer>,
}

impl S10cSeen {
    fn nearest(&self, p: [f32; 2]) -> u8 {
        let d2 = |a: [f32; 2]| (a[0] - p[0]).powi(2) + (a[1] - p[1]).powi(2);
        (0..self.points.len())
            .min_by(|&a, &b| d2(self.points[a]).total_cmp(&d2(self.points[b])))
            .unwrap_or(0) as u8
    }

    pub fn observe(&mut self, w: &World) {
        use std::collections::HashMap;
        let Some(g) = w.game() else { return };
        let s = g.setup();
        let n = s.players.len();
        if self.per.len() < n {
            self.points = s
                .map
                .points
                .iter()
                .map(|ps| frontline_sim::fx::FxVec2::from(ps.pos).to_f32())
                .collect();
            self.per = vec![
                S10cPlayer {
                    works_by_point: vec![0; self.points.len()],
                    mines_by_point: vec![0; self.points.len()],
                    ..S10cPlayer::default()
                };
                n
            ];
            self.home = (0..n)
                .map(|p| self.nearest(g.base_of(PlayerId(p as u8)).to_f32()))
                .collect();
            self.done = vec![Vec::new(); n];
        }
        // Enemy candidates: soldiers on foot and vehicles, by 30 m cell.
        let cell = |p: [f32; 2]| {
            (
                (p[0] / S10C_NEAR_M).floor() as i32,
                (p[1] / S10C_NEAR_M).floor() as i32,
            )
        };
        let ents = w.entities();
        let (kind, owner, tf, unit) = (ents.kind(), ents.owner(), ents.transform(), ents.unit());
        let mut grid: HashMap<(i32, i32), Vec<S10cUnit>> = HashMap::new();
        for i in 0..ents.len() {
            use frontline_sim::ecs::EntityKind;
            let vehicle = match kind[i] {
                EntityKind::Soldier if unit[i].riding.is_none() => false,
                EntityKind::Vehicle => true,
                _ => continue,
            };
            let p = tf[i].pos.xy().to_f32();
            grid.entry(cell(p))
                .or_default()
                .push((owner[i].0, p, vehicle));
        }
        let r2 = S10C_NEAR_M * S10C_NEAR_M;
        // Whether a unit of another player than `own` (a vehicle when
        // `vehicle`, else a soldier) stands within 30 m of `p`.
        let near = |p: [f32; 2], own: u8, vehicle: bool| {
            let (cx, cy) = cell(p);
            (cx - 1..=cx + 1).any(|x| {
                (cy - 1..=cy + 1).any(|y| {
                    grid.get(&(x, y)).is_some_and(|v| {
                        v.iter().any(|&(o, q, veh)| {
                            o != own
                                && veh == vehicle
                                && (q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) <= r2
                        })
                    })
                })
            })
        };
        let fort = g.fort();
        for (&id, m) in &fort.mines {
            let o = m.owner.0;
            let p = m.pos.to_f32();
            if !self.mines.contains_key(&id) {
                let pt = self.nearest(p);
                self.mines.insert(id, (o, pt));
                if let Some(r) = self.per.get_mut(o as usize) {
                    r.mines_seen += 1;
                }
            }
            if self.approached.contains(&id) {
                continue;
            }
            let at = s
                .items
                .get(m.item as usize)
                .and_then(|it| it.mine.as_ref())
                .is_some_and(|t| t.trigger == 1);
            if near(p, o, at) {
                self.approached.insert(id);
                if let Some(r) = self.per.get_mut(o as usize) {
                    r.mines_approached += 1;
                }
            }
        }
        for ws in fort.wire.values() {
            let o = ws.owner.0;
            let Some(r) = self.per.get_mut(o as usize) else {
                continue;
            };
            r.wire_stood = true;
            if !r.wire_approached {
                let (a, b) = (ws.a.to_f32(), ws.b.to_f32());
                r.wire_approached = near([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0], o, false);
            }
        }
        for pc in fort.pieces.values() {
            let o = pc.owner.0;
            if !s
                .forts
                .get(pc.fort as usize)
                .is_some_and(|f| f.blocks_vehicles)
            {
                continue;
            }
            if let Some(r) = self.per.get_mut(o as usize)
                && !r.obstacle_approached
            {
                r.obstacle_approached = near(pc.pos.to_f32(), o, true);
            }
        }
        for p in 0..n {
            let Some(ai) = g.ai(PlayerId(p as u8)) else {
                continue;
            };
            for id in &ai.wire_known {
                if let Some(ws) = fort.wire.get(id)
                    && ws.owner.0 as usize != p
                {
                    self.per[p].knew_enemy_wire = true;
                    if let Some(r) = self.per.get_mut(ws.owner.0 as usize) {
                        r.wire_seen_by_enemy = true;
                    }
                }
            }
            if ai.front_lanes.iter().any(|l| l.ap_done) {
                self.per[p].front_field = true;
            }
        }
        // Works done: a work gone since the last look while its owner's
        // `works_done` of that blueprint rose (in id order).
        let mut gone: Vec<(u8, u16, u8)> = Vec::new();
        self.works.retain(|id, v| {
            let keep = fort.works.contains_key(id);
            if !keep {
                gone.push(*v);
            }
            keep
        });
        for (&id, wk) in &fort.works {
            if !self.works.contains_key(&id) {
                let a = wk.line.first().map_or([0.0; 2], |v| v.to_f32());
                let b = wk.line.last().map_or(a, |v| v.to_f32());
                let pt = self.nearest([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]);
                self.works.insert(id, (wk.owner.0, wk.fort, pt));
            }
        }
        for p in 0..n {
            let now = fort
                .accounts
                .get(p)
                .map_or(Vec::new(), |a| a.works_done.clone());
            let was = &mut self.done[p];
            for &(o, f, pt) in &gone {
                let f = f as usize;
                if was.len() <= f {
                    was.resize(f + 1, 0);
                }
                if o as usize == p && now.get(f).copied().unwrap_or(0) > was[f] {
                    was[f] += 1;
                    if let Some(c) = self.per[p].works_by_point.get_mut(pt as usize) {
                        *c += 1;
                    }
                }
            }
            *was = now;
        }
    }

    /// Mines by point and home / front, from every mine seen.
    pub fn finish(&mut self) {
        for &(o, pt) in self.mines.values() {
            let Some(r) = self.per.get_mut(o as usize) else {
                continue;
            };
            if let Some(c) = r.mines_by_point.get_mut(pt as usize) {
                *c += 1;
            }
            if self.home.get(o as usize) == Some(&pt) {
                r.mines_home += 1;
            } else {
                r.mines_front += 1;
            }
        }
    }
}

impl S10Player {
    /// The AI counters of `a` in [`S10_AI`] order.
    pub fn ai_counters(a: &frontline_sim::ai::AiCounters) -> [u32; 14] {
        [
            a.fort_jobs,
            a.fort_refused,
            a.fort_build_plans,
            a.infantry_plans,
            a.shelter_moves,
            a.material_runs[0],
            a.material_runs[1],
            a.material_runs[2],
            a.breaches_started,
            a.breaches_opened,
            a.breaches_given_up,
            a.breach_smokes,
            a.breach_suppress,
            a.wire_bypassed,
        ]
    }
}

/// S7 numbers of one player (`frontline_sim::S7Counters`), keyed by short
/// ids, plus what the derived shares need: the faction's roster card
/// prices and each vehicle type's card.
#[derive(Clone, Debug, Default, Serialize)]
pub struct S7Player {
    pub kills_by_vehicle: BTreeMap<String, u32>,
    pub kos_by_vehicle: BTreeMap<String, u32>,
    /// Build B: own items at rest inside enemy open boxes, enemy open-top
    /// seats and parts struck; AI throws ordered into open tops, overwatch
    /// spots reached, relocations, open-top back-offs.
    pub open_top_throws: u32,
    pub open_top_hits: u32,
    pub open_top_throw_orders: u32,
    pub overwatch_holds: u32,
    pub relocations: u32,
    pub open_top_backoffs: u32,
    pub card_cost: BTreeMap<String, u32>,
    pub vehicle_card: BTreeMap<String, String>,
    /// S9 (T33): vehicles the owner's manpower cap counts (tanks, assault
    /// guns and tank destroyers), by short id.
    pub tank_td_vehicles: BTreeSet<String>,
    /// S9: the roster's AT guns, by short id.
    pub gun_vehicles: BTreeSet<String>,
}

/// S5b Part B AI fire-mission counters of one player (or summed).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S5bAi {
    /// Missions ordered by the mode the AI expected (direct, observed, map).
    pub missions: [u64; 3],
    pub spotters: u64,
    pub spotter_missions: u64,
    pub spotter_timeouts: u64,
    pub spotter_recalls: u64,
    pub map_holds: u64,
    pub walk_holds: u64,
    pub blind_holds: u64,
    pub ray_holds: u64,
    /// S6b: map fire held by its expected miss, fire missions withdrawn
    /// because the tube was dry when due.
    pub miss_holds: u64,
    pub dry_drops: u64,
}

impl S5bAi {
    fn of(c: &frontline_sim::ai::AiCounters) -> S5bAi {
        S5bAi {
            missions: c.fm_missions.map(u64::from),
            spotters: c.fm_spotters as u64,
            spotter_missions: c.fm_spotter_missions as u64,
            spotter_timeouts: c.fm_spotter_timeouts as u64,
            spotter_recalls: c.fm_spotter_recalls as u64,
            map_holds: c.fm_map_holds as u64,
            walk_holds: c.fm_walk_holds as u64,
            ray_holds: c.fm_ray_holds as u64,
            blind_holds: c.fm_blind_holds as u64,
            miss_holds: c.fm_miss_holds as u64,
            dry_drops: c.fm_dry_drops as u64,
        }
    }

    fn add(&mut self, o: &S5bAi) {
        for k in 0..3 {
            self.missions[k] += o.missions[k];
        }
        self.spotters += o.spotters;
        self.spotter_missions += o.spotter_missions;
        self.spotter_timeouts += o.spotter_timeouts;
        self.spotter_recalls += o.spotter_recalls;
        self.map_holds += o.map_holds;
        self.walk_holds += o.walk_holds;
        self.ray_holds += o.ray_holds;
        self.blind_holds += o.blind_holds;
        self.miss_holds += o.miss_holds;
        self.dry_drops += o.dry_drops;
    }
}

/// S5a per-player report counters (`frontline_sim::AmmoAccount`).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S5aPlayer {
    /// Rounds fired by class (`fired_class::NAMES`).
    pub fired: [u32; fired_class::COUNT],
    /// Manpower paid for ammunition by supply kind (small arms,
    /// explosives, shells), in µMP.
    pub paid_micro: [u64; 3],
    /// Rounds, items and bombs refilled by supply kind.
    pub refilled: [u32; 3],
    /// Seconds on which a refill paused for lack of manpower.
    pub paused_s: u32,
    /// Enemy men killed by class (the HE share by source).
    pub kills_by: [u32; kill_class::COUNT],
    /// Part B AI counters (`AiCounters`): refill trips (all, dry,
    /// stalled), squads away summed over plans and plans, mortar reserve
    /// holds, rearm stalls, buys holding `float_mp`, ambushes started,
    /// tubes fired from them and ends by reason (`AMBUSH_END_NAMES`).
    pub ai: S5aAi,
}

/// S5a Part B AI counters of one player (or summed).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S5aAi {
    pub refill_trips: u64,
    pub refill_dry_trips: u64,
    pub refill_stalls: u64,
    pub refill_away: u64,
    pub plans: u64,
    pub mortar_reserve_holds: u64,
    pub rearm_stalls: u64,
    pub float_holds: u64,
    pub ambushes: u64,
    pub ambush_shots: u64,
    pub ambush_ends: [u64; frontline_sim::ai::AMBUSH_ENDS],
}

impl S5aAi {
    fn of(c: &frontline_sim::ai::AiCounters) -> S5aAi {
        S5aAi {
            refill_trips: c.refill_trips as u64,
            refill_dry_trips: c.refill_dry_trips as u64,
            refill_stalls: c.refill_stalls as u64,
            refill_away: c.refill_away as u64,
            plans: c.plans as u64,
            mortar_reserve_holds: c.mortar_reserve_holds as u64,
            rearm_stalls: c.rearm_stalls as u64,
            float_holds: c.float_holds as u64,
            ambushes: c.ambushes as u64,
            ambush_shots: c.ambush_shots as u64,
            ambush_ends: c.ambush_ends.map(|v| v as u64),
        }
    }

    fn add(&mut self, o: &S5aAi) {
        self.refill_trips += o.refill_trips;
        self.refill_dry_trips += o.refill_dry_trips;
        self.refill_stalls += o.refill_stalls;
        self.refill_away += o.refill_away;
        self.plans += o.plans;
        self.mortar_reserve_holds += o.mortar_reserve_holds;
        self.rearm_stalls += o.rearm_stalls;
        self.float_holds += o.float_holds;
        self.ambushes += o.ambushes;
        self.ambush_shots += o.ambush_shots;
        for (a, b) in self.ambush_ends.iter_mut().zip(&o.ambush_ends) {
            *a += b;
        }
    }
}

/// S4 per-player report counters.
#[derive(Clone, Debug, Default, Serialize)]
pub struct S4Player {
    /// AI call-ins by card id (`AiCounters::bought`).
    pub bought: BTreeMap<String, u32>,
    /// Single-shot AT items (Panzerfausts) fired.
    pub panzerfausts: u32,
    /// Crew weapons own men picked up.
    pub pickups: u32,
    /// Own population at 5, 10 and 20 match minutes (`None`: over sooner).
    pub pop_at_min: [Option<u32>; 3],
    /// AI buy plans held back by the pop cap alone.
    pub pop_cap_holds: u32,
    /// AI Panzerfaust hunts started and capture orders given.
    pub pzf_hunts: u32,
    pub capture_orders: u32,
}

/// Match minutes the S4 report samples the population at.
pub const POP_MINUTES: [u64; 3] = [5, 10, 20];

#[derive(Clone, Debug, Serialize)]
pub struct Totals {
    pub shots_fired: u64,
    pub shots_dropped: u64,
    pub soldier_hits: u64,
    pub near_misses: u64,
    pub explosions: u64,
    pub deaths: u64,
    /// Thrown items released (S1b), and the frag and satchel bursts and
    /// smoke clouds among them, counted since the start (or the load).
    pub throws: u64,
    pub frag_bursts: u64,
    pub satchel_bursts: u64,
    pub smoke_clouds: u64,
    pub detail_tiles: usize,
    pub peak_soldiers: usize,
    pub peak_tick: u64,
    pub peak_projectiles: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct SkirmishTiming {
    /// Every tick of the match.
    pub all: Summary,
    /// Ticks with at least `peak_share_pct` % of `totals.peak_soldiers`.
    pub peak: Summary,
    pub peak_ticks: usize,
    pub peak_share_pct: usize,
    /// Ticks over the tick period (33.3 ms at 30 Hz).
    pub overruns: u64,
}

/// `ArmourZone` names by index (the snapshot's `zone`).
const ZONE_NAMES: [&str; 8] = [
    "hull_front",
    "hull_side",
    "hull_rear",
    "hull_top",
    "turret_front",
    "turret_side",
    "turret_rear",
    "turret_top",
];

/// Per-tick S3 bookkeeping from the snapshot (armour events, repair
/// flags, AT team strength); none of it feeds back into the sim.
#[derive(Default)]
struct ArmourTally {
    players: Vec<PlayerArmour>,
    /// Last round (ammo index) that hit each vehicle.
    last_ammo: BTreeMap<u32, u16>,
    repairing: BTreeMap<u32, bool>,
    /// AT team squads: (owner, men last seen).
    at_men: BTreeMap<SquadId, (u8, usize)>,
    at_men_lost: Vec<u32>,
    /// Vehicle costs by vehicle type, and per player the enemy rifleman's
    /// value.
    vehicle_cost: Vec<u32>,
    man_value: Vec<u32>,
    /// S3c: abandoned vehicles still standing, by id: their last owner.
    abandoned: BTreeMap<u32, u8>,
    /// S4: ammo indices fired by single-shot AT items (Panzerfausts).
    pzf_ammo: Vec<u16>,
    /// S9b: by vehicle type, an AT gun and armour (tank, assault gun, tank
    /// destroyer).
    gun_type: Vec<bool>,
    armour_type: Vec<bool>,
    /// S9b: the last tick a gun of `attacker` fired `ammo` at vehicle
    /// `target`, and per vehicle whether its last impact was such a round.
    gun_shots: BTreeMap<(u8, u16, u32), u64>,
    gun_hit_last: BTreeMap<u32, bool>,
    /// S9b: `(attacker, vehicle)` already counted in `gun_armour_out`.
    gun_out_seen: BTreeSet<(u8, u32)>,
}

/// S3c abandonment trigger names, by `bail::AB_*` (index 0 unused).
pub const ABANDON_CAUSES: [&str; 7] = [
    "none",
    "ammo_fire",
    "engine_fire",
    "crew",
    "useless",
    "under_fire",
    "order",
];

impl ArmourTally {
    fn new(setup: &MatchSetup) -> ArmourTally {
        let n = setup.players.len();
        let vehicle_cost = (0..setup.vehicles.len())
            .map(|v| {
                setup
                    .squad_types
                    .iter()
                    .find(|t| t.vehicle == Some(v as u16))
                    .map_or(0, |t| t.cost)
            })
            .collect();
        // What a soldier of player p is worth to his killer.
        let man_value = (0..n)
            .map(|p| {
                let f = &setup.factions[setup.players[p].faction as usize];
                f.roster
                    .iter()
                    .find(|&&t| setup.type_has_tag(t, "line"))
                    .map(|&t| &setup.squad_types[t as usize])
                    .map_or(0, |t| t.cost / t.men.max(1) as u32)
            })
            .collect();
        let pzf_ammo = setup
            .items
            .iter()
            .filter_map(|it| it.weapon)
            .filter_map(|w| setup.weapons.get(w as usize).and_then(|w| w.ammo))
            .collect();
        use frontline_content::VehicleRole;
        let gun_type = setup
            .vehicles
            .iter()
            .map(|v| v.role == VehicleRole::AtGun)
            .collect();
        let armour_type = setup
            .vehicles
            .iter()
            .map(|v| {
                matches!(
                    v.role,
                    VehicleRole::Tank | VehicleRole::AssaultGun | VehicleRole::TankDestroyer
                )
            })
            .collect();
        ArmourTally {
            players: vec![PlayerArmour::default(); n],
            at_men_lost: vec![0; n],
            vehicle_cost,
            man_value,
            pzf_ammo,
            gun_type,
            armour_type,
            ..ArmourTally::default()
        }
    }

    /// A vehicle lost to `attacker`'s guns counts once a match: a crew that
    /// comes back and bails out again is the same vehicle.
    fn count_gun_out(&mut self, attacker: u8, vehicle: u32) {
        if self.gun_out_seen.insert((attacker, vehicle))
            && let Some(p) = self.players.get_mut(attacker as usize)
        {
            p.gun_armour_out += 1;
        }
    }

    fn observe(&mut self, w: &World) {
        let g = w.game().expect("match world");
        let s = g.setup();
        let snap = w.snapshot();
        let tick = w.tick();
        let window = 3 * s.tick_hz as u64;
        let type_of = |id: u32| {
            snap.vehicles
                .iter()
                .find(|v| v.id == id)
                .map(|v| v.type_idx as usize)
        };
        for e in &snap.armour_events {
            let owner = e.owner as usize;
            if e.ammo != u16::MAX {
                self.last_ammo.insert(e.vehicle, e.ammo);
            }
            let k = e.kind;
            // S9b: gun rounds and which vehicle each impact came from.
            if k == ArmourEventKind::Fired as u8 {
                let shooter = snap.vehicles.iter().find(|v| v.id == e.vehicle);
                if let Some(v) = shooter
                    && self.gun_type.get(v.type_idx as usize) == Some(&true)
                {
                    self.gun_shots.insert((e.owner, e.ammo, v.target), tick);
                }
            } else if k == ArmourEventKind::Ricochet as u8
                || k == ArmourEventKind::NonPenetration as u8
                || k == ArmourEventKind::Penetration as u8
            {
                let by_gun = self
                    .gun_shots
                    .get(&(e.attacker, e.ammo, e.vehicle))
                    .is_some_and(|&t| t + window >= tick);
                self.gun_hit_last.insert(e.vehicle, by_gun);
            }
            let gun_out = (k == ArmourEventKind::Abandoned as u8
                || k == ArmourEventKind::KnockedOut as u8)
                && !self.abandoned.contains_key(&e.vehicle)
                && self.gun_hit_last.remove(&e.vehicle) == Some(true)
                && type_of(e.vehicle).and_then(|t| self.armour_type.get(t)) == Some(&true);
            if gun_out {
                self.count_gun_out(e.attacker, e.vehicle);
            }
            if k == ArmourEventKind::Recrewed as u8 {
                self.gun_hit_last.remove(&e.vehicle);
            }
            let ammo = s.ammo.get(e.ammo as usize);
            let heat = ammo.is_some_and(|a| a.mechanism == AmmoMechanism::Heat);
            let impact = k == ArmourEventKind::Ricochet as u8
                || k == ArmourEventKind::NonPenetration as u8
                || k == ArmourEventKind::Penetration as u8;
            if let Some(a) = ammo
                && let Some(p) = self.players.get_mut(e.attacker as usize)
            {
                let pen = k == ArmourEventKind::Penetration as u8;
                if impact {
                    let zone = ZONE_NAMES.get(e.zone as usize).copied().unwrap_or("none");
                    *p.hits_by_zone.entry(zone.into()).or_default() += 1;
                    if heat {
                        p.rocket_hits += 1;
                        p.rocket_pens += pen as u32;
                        if self.pzf_ammo.contains(&e.ammo) {
                            p.pzf_hits += 1;
                            p.pzf_pens += pen as u32;
                        }
                    } else if a.mechanism.penetrator() {
                        p.gun_hits += 1;
                        p.gun_pens += pen as u32;
                    } else {
                        p.he_on_plate += 1;
                    }
                } else if heat && k == ArmourEventKind::Disabled as u8 {
                    p.rocket_disabled_parts += 1;
                }
            }
            let name = |c: u8| match c {
                0 => "engine",
                1 | 2 => "track",
                3 => "gun",
                4 => "turret_ring",
                _ => "rack_destroyed",
            };
            if k == ArmourEventKind::Bogged as u8 {
                if let Some(p) = self.players.get_mut(owner) {
                    p.bogged += 1;
                }
            } else if k == ArmourEventKind::Disabled as u8 {
                if let Some(p) = self.players.get_mut(owner) {
                    *p.disabled_by.entry(name(e.component).into()).or_default() += 1;
                }
            } else if k == ArmourEventKind::Abandoned as u8 {
                // S3c: a loss like a knock-out, credited the same way.
                let ty = snap
                    .vehicles
                    .iter()
                    .find(|v| v.id == e.vehicle)
                    .map(|v| v.type_idx as usize);
                let cost = ty
                    .and_then(|t| self.vehicle_cost.get(t))
                    .copied()
                    .unwrap_or(0);
                self.abandoned.insert(e.vehicle, e.owner);
                if let Some(p) = self.players.get_mut(owner) {
                    p.vehicles_lost += 1;
                    p.vehicles_abandoned += 1;
                    p.tank_mp_lost += cost;
                    let c = ABANDON_CAUSES
                        .get(e.component as usize)
                        .copied()
                        .unwrap_or("none");
                    *p.abandon_causes.entry(c.into()).or_default() += 1;
                    *p.disabled_by.entry(format!("abandoned_{c}")).or_default() += 1;
                }
                let rocket = self
                    .last_ammo
                    .get(&e.vehicle)
                    .and_then(|&a| s.ammo.get(a as usize))
                    .is_some_and(|a| a.mechanism == AmmoMechanism::Heat);
                let pzf = self
                    .last_ammo
                    .get(&e.vehicle)
                    .is_some_and(|a| self.pzf_ammo.contains(a));
                if let Some(p) = self.players.get_mut(e.attacker as usize) {
                    if rocket {
                        p.ko_by_rocket += 1;
                        p.ko_by_pzf += pzf as u32;
                        p.at_mp_killed += cost;
                    } else {
                        p.ko_by_gun += 1;
                        p.tank_mp_killed += cost;
                    }
                }
                self.last_ammo.remove(&e.vehicle);
            } else if k == ArmourEventKind::Recrewed as u8 {
                if let Some(last) = self.abandoned.remove(&e.vehicle)
                    && let Some(p) = self.players.get_mut(last as usize)
                {
                    let fate = if e.component == frontline_sim::bail::RECREW_CAPTURED {
                        "captured"
                    } else {
                        "recrewed"
                    };
                    *p.abandoned_fates.entry(fate.into()).or_default() += 1;
                }
                self.last_ammo.remove(&e.vehicle);
            } else if k == ArmourEventKind::KnockedOut as u8
                && let Some(last) = self.abandoned.remove(&e.vehicle)
            {
                // S3c: an abandoned vehicle burnt out or blown up later: no
                // second loss.
                if let Some(p) = self.players.get_mut(last as usize) {
                    *p.abandoned_fates
                        .entry("destroyed_later".into())
                        .or_default() += 1;
                }
                self.last_ammo.remove(&e.vehicle);
            } else if k == ArmourEventKind::KnockedOut as u8 {
                let ty = snap
                    .vehicles
                    .iter()
                    .find(|v| v.id == e.vehicle)
                    .map(|v| v.type_idx as usize);
                let cost = ty
                    .and_then(|t| self.vehicle_cost.get(t))
                    .copied()
                    .unwrap_or(0);
                let cause = match e.component {
                    0 => "ko_crew",
                    1 => "ko_ammo_rack",
                    3 => "ko_ammo_fire",
                    _ => "ko_fire",
                };
                if let Some(p) = self.players.get_mut(owner) {
                    p.vehicles_lost += 1;
                    p.vehicles_destroyed += 1;
                    p.tank_mp_lost += cost;
                    *p.disabled_by.entry(cause.into()).or_default() += 1;
                }
                let rocket = self
                    .last_ammo
                    .get(&e.vehicle)
                    .and_then(|&a| s.ammo.get(a as usize))
                    .is_some_and(|a| a.mechanism == AmmoMechanism::Heat);
                let pzf = self
                    .last_ammo
                    .get(&e.vehicle)
                    .is_some_and(|a| self.pzf_ammo.contains(a));
                if let Some(p) = self.players.get_mut(e.attacker as usize) {
                    if rocket {
                        p.ko_by_rocket += 1;
                        p.ko_by_pzf += pzf as u32;
                        p.at_mp_killed += cost;
                    } else {
                        p.ko_by_gun += 1;
                        p.tank_mp_killed += cost;
                    }
                }
                self.last_ammo.remove(&e.vehicle);
            }
        }
        for v in &snap.vehicles {
            let on = v.flags & vehicle_flags::REPAIRING != 0;
            let was = self.repairing.insert(v.id, on).unwrap_or(false);
            if on
                && !was
                && let Some(p) = self.players.get_mut(v.owner as usize)
            {
                p.repaired += 1;
            }
        }
        // AT team losses: men gone since the last look.
        let mut seen: Vec<SquadId> = Vec::new();
        for (&id, sq) in g.squads() {
            if sq.launcher.is_none() {
                continue;
            }
            seen.push(id);
            let now = sq.members.len();
            let e = self.at_men.entry(id).or_insert((sq.owner.0, now));
            if now < e.1 {
                self.at_men_lost[e.0 as usize] += (e.1 - now) as u32;
            }
            e.1 = now;
        }
        let gone: Vec<SquadId> = self
            .at_men
            .keys()
            .filter(|id| !seen.contains(id))
            .copied()
            .collect();
        for id in gone {
            let (o, n) = self.at_men.remove(&id).expect("listed");
            self.at_men_lost[o as usize] += n as u32;
        }
    }
}

/// The `mods/ww2` directory: `--mods`, else `./mods/ww2`, else the copy in
/// the source tree this binary was built from.
pub fn mods_dir(arg: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = arg {
        return Ok(p.to_owned());
    }
    let here = PathBuf::from("mods/ww2");
    if here.join("mod.toml").is_file() {
        return Ok(here);
    }
    let tree = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mods/ww2");
    if tree.join("mod.toml").is_file() {
        return Ok(tree);
    }
    bail!("cannot find mods/ww2; pass --mods <DIR>")
}

/// Loads the mod (with `--rules-override`) and resolves an AI-vs-AI match
/// from `args`.
pub fn match_setup(args: &SkirmishArgs) -> Result<MatchSetup> {
    let (defs, _) = crate::overrides::load_defs(args.mods.as_deref(), &args.rules_override)?;
    let d0: Difficulty = args.difficulty.into();
    let d1: Difficulty = args.difficulty1.unwrap_or(args.difficulty).into();
    let req = MatchRequest {
        rules: "ww2:skirmish".into(),
        map: "ww2:kessel_ridge".into(),
        seed: args.seed,
        players: vec![
            PlayerRequest {
                faction: args.faction0.clone(),
                ai: true,
                difficulty: d0,
            },
            PlayerRequest {
                faction: args.faction1.clone(),
                ai: true,
                difficulty: d1,
            },
        ],
    };
    let mut setup = defs.resolve(&req).map_err(|e| anyhow!("{e}"))?;
    if let Some(mp) = args.start_manpower {
        setup.rules.start_manpower = mp;
    }
    if args.swap_bases {
        let (b0, b1) = (setup.players[0].base, setup.players[1].base);
        (setup.players[0].base, setup.players[1].base) = (b1, b0);
    }
    Ok(setup)
}

/// Runs the match described by `args` to its end or the time limit.
pub fn skirmish(args: &SkirmishArgs) -> Result<SkirmishReport> {
    if !args.ai_vs_ai && args.load.is_none() {
        bail!("only --ai-vs-ai is supported headlessly");
    }
    let threads = match args.threads {
        Some(0) => bail!("--threads must be at least 1"),
        Some(t) => t,
        None => std::thread::available_parallelism().map_or(1, |n| n.get()),
    };
    let mut world = match &args.load {
        Some(path) => {
            let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
            let w = World::load(&bytes, threads).map_err(|e| anyhow!("{}: {e}", path.display()))?;
            match w.match_setup() {
                Some(s) if s.players.iter().all(|p| p.is_ai) => {}
                _ => bail!("{} is not an AI-vs-AI match save", path.display()),
            }
            w
        }
        None => World::new_match(match_setup(args)?, threads)?,
    };
    let hz = world.match_setup().expect("match world").tick_hz as u64;
    let limit = args.minutes as u64 * 60 * hz;
    let start_tick = world.tick();
    let save_at = match (&args.save, args.save_at) {
        (Some(_), Some(k)) if k < start_tick || k > limit => bail!(
            "--save-at {k} is outside this run's ticks {start_tick}..={limit} \
             (it is an absolute tick number)"
        ),
        (Some(_), k) => Some(k),
        (None, _) => None,
    };
    let mut saved = None;
    let mut maybe_save = |w: &World, at_end: bool| -> Result<()> {
        if let Some(path) = &args.save
            && saved.is_none()
            && (save_at == Some(Some(w.tick())) || at_end)
        {
            let bytes = w.save();
            fs::write(path, &bytes).with_context(|| format!("writing {}", path.display()))?;
            saved = Some(SavedMatch {
                tick: w.tick(),
                path: path.display().to_string(),
                bytes: bytes.len(),
                hash: hex(w.state_hash()),
            });
        }
        Ok(())
    };
    maybe_save(&world, false)?;

    let period = Duration::from_nanos(1_000_000_000 / hz);
    let mut tick_ns: Vec<u64> = Vec::with_capacity(limit as usize);
    let mut soldiers: Vec<usize> = Vec::with_capacity(limit as usize);
    let mut fielded: BTreeMap<SquadId, (PlayerId, u16)> = BTreeMap::new();
    let (mut overruns, mut peak_proj) = (0u64, 0usize);
    let (mut peak, mut peak_tick) = (0usize, 0u64);
    let note_squads = |w: &World, fielded: &mut BTreeMap<SquadId, (PlayerId, u16)>| {
        for (&id, sq) in w.game().expect("match world").squads() {
            fielded.entry(id).or_insert((sq.owner, sq.type_idx));
        }
    };
    let mut tally = ArmourTally::new(world.match_setup().expect("match world"));
    let mut s3c = S3cTally::new(world.match_setup().expect("match world"));
    let mut s10b_ordered = S10bOrdered::default();
    let mut s10b_ai_seen = S10bAiSeen::default();
    let mut death_places = DeathPlaces::default();
    let mut s10c_seen = S10cSeen::default();
    let stats_at_start = world.stats().clone();
    note_squads(&world, &mut fielded);

    let start = Instant::now();
    let running = |w: &World| w.game().is_some_and(|g| g.outcome() == Outcome::Running);
    let (mut frag_bursts, mut satchel_bursts) = (0u64, 0u64);
    let smoke_at_start = world.snapshot().smoke.last().map_or(0, |c| c.id as u64 + 1);
    let mut smoke_seen = smoke_at_start;
    let throws_at_start = world.stats().throws;
    // S4: own population at POP_MINUTES, per player.
    let n_players = world.match_setup().map_or(0, |s| s.players.len());
    let mut pop_at: Vec<[Option<u32>; 3]> = vec![[None; 3]; n_players];
    let mut squads_at: Vec<[Option<[u32; 2]>; 5]> = vec![[None; 5]; n_players];
    let mut gun_trace = match &args.gun_trace {
        Some(p) => Some(crate::gun_trace::GunTrace::create(p, &world)?),
        None => None,
    };
    while running(&world) && world.tick() < limit {
        if args.paced {
            let deadline = start + period * (world.tick() - start_tick) as u32;
            let now = Instant::now();
            if now < deadline {
                std::thread::sleep(deadline - now);
            }
        }
        let t = Instant::now();
        world.step(&[]);
        let dt = t.elapsed();
        if dt > period {
            overruns += 1;
        }
        tick_ns.push(ns(dt));
        let n = world.entities().soldier_count();
        soldiers.push(n);
        if n > peak {
            (peak, peak_tick) = (n, world.tick());
        }
        peak_proj = peak_proj.max(world.projectiles().live());
        for e in &world.snapshot().explosions {
            match e.kind {
                BlastKind::Frag => frag_bursts += 1,
                BlastKind::Satchel => satchel_bursts += 1,
                BlastKind::Shell => {}
            }
        }
        for c in &world.snapshot().smoke {
            smoke_seen = smoke_seen.max(c.id as u64 + 1);
        }
        note_squads(&world, &mut fielded);
        tally.observe(&world);
        s3c.observe(&world);
        s10b_ordered.observe(&world);
        s10b_ai_seen.observe(&world);
        // S10c-ai: exposure, once a second (read-only).
        if world.tick().is_multiple_of(hz) {
            s10c_seen.observe(&world);
        }
        if args.side_detail {
            death_places.observe(&world);
        }
        if args.side_detail {
            for (k, &m) in SQUAD_MINUTES.iter().enumerate() {
                if world.tick() == m * 60 * hz
                    && let Some(g) = world.game()
                {
                    for (p, at) in squads_at.iter_mut().enumerate() {
                        at[k] = Some(squads_now(g, PlayerId(p as u8)));
                    }
                }
            }
        }
        for (k, &m) in POP_MINUTES.iter().enumerate() {
            if world.tick() == m * 60 * hz
                && let Some(g) = world.game()
            {
                for (p, at) in pop_at.iter_mut().enumerate() {
                    at[k] = Some(g.population(world.entities(), PlayerId(p as u8)));
                }
            }
        }
        if args.trace_vehicles && world.tick().is_multiple_of(10 * hz) {
            trace_vehicles(&world);
        }
        if let Some(t) = gun_trace.as_mut() {
            t.observe(&world)?;
        }
        maybe_save(&world, false)?;
    }
    let wall = start.elapsed();
    maybe_save(&world, true)?;
    if let Some(t) = gun_trace {
        t.finish()?;
    }
    s3c.finish();
    s10c_seen.observe(&world);
    s10c_seen.finish();

    let threshold = (peak * PEAK_SHARE_PCT).div_ceil(100);
    let mut peak_ns: Vec<u64> = tick_ns
        .iter()
        .zip(&soldiers)
        .filter(|(_, n)| **n >= threshold)
        .map(|(t, _)| *t)
        .collect();
    let timing = SkirmishTiming {
        peak: Summary::from_nanos(&mut peak_ns),
        peak_ticks: peak_ns.len(),
        all: Summary::from_nanos(&mut tick_ns),
        peak_share_pct: PEAK_SHARE_PCT,
        overruns,
    };

    let g = world.game().expect("match world");
    let s = g.setup();
    let snap = world.snapshot();
    let (result, winner) = match g.outcome() {
        Outcome::Won { winner } => ("won", Some(winner.0)),
        Outcome::Draw => ("draw", None),
        Outcome::Running => ("time_limit", None),
    };
    let faction = |p: usize| s.factions[s.players[p].faction as usize].id.clone();
    let names = |k: &[u32]| -> BTreeMap<String, u32> {
        kill_class::NAMES
            .iter()
            .zip(k)
            .map(|(n, v)| (n.to_string(), *v))
            .collect()
    };
    let players = (0..s.players.len())
        .map(|p| {
            let pl = &g.players()[p];
            let ps = &snap.players[p];
            let ai = g.ai(PlayerId(p as u8));
            let owned = |o: &PlayerId| o.0 as usize == p;
            let mut armour = std::mem::take(&mut tally.players[p]);
            let tag_bought = |tag: &str| {
                fielded
                    .values()
                    .filter(|(o, t)| owned(o) && s.type_has_tag(*t, tag))
                    .count() as u32
            };
            armour.vehicles_bought = tag_bought("vehicle");
            armour.at_teams_bought = tag_bought("at");
            armour.kills_by = names(&pl.kills_by);
            // Kills by own tank weapons and rockets, valued at the enemy
            // rifleman's share (the knock-outs were counted as they came).
            let man = (0..s.players.len())
                .filter(|&q| q != p)
                .map(|q| tally.man_value[q])
                .max()
                .unwrap_or(0);
            let k = |c: u16| pl.kills_by[c as usize];
            armour.tank_mp_killed +=
                man * (k(kill_class::TANK_AP) + k(kill_class::TANK_HE) + k(kill_class::VEHICLE_MG));
            armour.at_mp_killed += man * k(kill_class::ROCKET);
            let at_cost = s.factions[s.players[p].faction as usize]
                .roster
                .iter()
                .find(|&&t| s.type_has_tag(t, "at"))
                .map(|&t| &s.squad_types[t as usize])
                .map_or(0, |t| t.cost / t.men.max(1) as u32);
            armour.at_mp_lost = tally.at_men_lost[p] * at_cost;
            armour.at_shots = pl.rockets;
            // S3c: crew counters, and the abandoned still standing or
            // scrapped by the cap.
            armour.captures = pl.crew.captured;
            armour.crew_bailed = pl.crew.bailed;
            armour.crew_out = pl.crew.out;
            armour.crew_home = pl.crew.home;
            if pl.crew.scrapped > 0 {
                armour
                    .abandoned_fates
                    .insert("scrapped".into(), pl.crew.scrapped);
            }
            let standing = g
                .vehicles()
                .iter()
                .filter(|(id, vs)| {
                    vs.abandoned.is_some() && world.entities().owner()[id.index()].0 as usize == p
                })
                .count() as u32;
            if standing > 0 {
                armour.abandoned_fates.insert("standing".into(), standing);
            }
            if let Some(a) = ai {
                let c = &a.counters;
                armour.escorts_started = c.escorts_started;
                armour.escorts_reached = c.escorts_reached;
                armour.escorts_released = c.escorts_released;
                armour.satchels_at_vehicles = c.satchels_at_vehicles;
                armour.repairs = c.repairs;
                armour.retreats = c.retreats;
                armour.faces = c.faces;
                armour.suppress_orders = c.suppress_orders;
                armour.bounds = c.bounds;
                armour.at_hunts = c.at_hunts;
                armour.satchel_hunts = c.satchel_hunts;
                let mut m = BTreeMap::new();
                for (k, v) in [
                    ("rearms", c.rearms),
                    ("hunts_kept_unseen", c.hunts_kept_unseen),
                    ("hunt_throws", c.hunt_throws),
                    ("second_bases", c.second_bases),
                    ("at_stuck_hunts", c.at_stuck_hunts),
                    ("at_follows", c.at_follows),
                    ("recrews", c.recrews),
                    ("crew_retreats", c.crew_retreats),
                    ("denials", c.denials),
                ] {
                    m.insert(k.to_string(), v);
                }
                for (k, &v) in c.hunt_ends.iter().enumerate() {
                    m.insert(
                        format!("hunt_end_{}", frontline_sim::ai::HUNT_END_NAMES[k]),
                        v,
                    );
                }
                armour.ai_s3c = m;
            }
            PlayerReport {
                player: p as u8,
                faction: faction(p),
                base: s.players[p].base,
                difficulty: format!("{:?}", s.players[p].difficulty).to_lowercase(),
                defeat: pl.defeat.map(|d| format!("{d:?}").to_lowercase()),
                tickets: pl.tickets,
                manpower: pl.manpower,
                kills: pl.kills,
                losses: pl.losses,
                squads_fielded: fielded.values().filter(|(o, _)| owned(o)).count() as u32,
                squads_alive: g.squads().values().filter(|q| owned(&q.owner)).count() as u32,
                soldiers_alive: snap
                    .soldier_owner
                    .iter()
                    .filter(|o| **o as usize == p)
                    .count() as u32,
                points_held: ps.points_held,
                hold_s: ps.hold_s,
                ai_commands: ai.map_or(0, |a| a.issued),
                ai_rejected: ai.map_or(0, |a| a.rejected),
                trenches_planned: ai.map_or(0, |a| a.trenches.len() as u32),
                armour,
                s3c: std::mem::take(&mut s3c.players[p]),
                s4: S4Player {
                    bought: ai.map_or_else(BTreeMap::new, |a| {
                        a.counters
                            .bought
                            .iter()
                            .enumerate()
                            .filter(|(_, n)| **n > 0)
                            .map(|(t, &n)| {
                                let id = &s.squad_types[t].id;
                                (
                                    id.split_once(':').map_or(id.as_str(), |x| x.1).to_owned(),
                                    n,
                                )
                            })
                            .collect()
                    }),
                    panzerfausts: pl.s4.single_shots,
                    pickups: pl.s4.pickups,
                    pop_at_min: pop_at.get(p).copied().unwrap_or([None; 3]),
                    pop_cap_holds: ai.map_or(0, |a| a.counters.pop_cap_holds),
                    pzf_hunts: ai.map_or(0, |a| a.counters.pzf_hunts),
                    capture_orders: ai.map_or(0, |a| a.counters.capture_orders),
                },
                s5a: S5aPlayer {
                    fired: pl.ammo.fired,
                    paid_micro: pl.ammo.paid,
                    refilled: pl.ammo.refilled,
                    paused_s: pl.ammo.paused_ticks / s.tick_hz.max(1),
                    kills_by: pl.kills_by,
                    ai: ai.map_or_else(S5aAi::default, |a| S5aAi::of(&a.counters)),
                },
                s5b: pl.fm,
                s11a1: S11a1Player {
                    s11: pl.s11,
                    ai: ai.map_or([0; 14], |a| S11a1Player::ai_counters(&a.counters)),
                },
                s5b_ai: ai.map_or_else(S5bAi::default, |a| S5bAi::of(&a.counters)),
                s6: g.supply6().accounts.get(p).copied().unwrap_or_default(),
                s6_stock_held_g: g
                    .supply6()
                    .structures
                    .values()
                    .filter(|x| x.owner.0 as usize == p)
                    .map(|x| x.stock_g.iter().map(|&v| v as u64).sum::<u64>())
                    .sum(),
                s6_tank_crate_runs: ai.map_or(0, |a| a.counters.tank_crate_runs),
                s6_crate_pickups: ai.map_or(0, |a| a.counters.s6_crate_pickups),
                s7: {
                    let short = |id: &str| id.split_once(':').map_or(id, |x| x.1).to_owned();
                    let named = |v: &[u32]| {
                        s.vehicles
                            .iter()
                            .zip(v)
                            .map(|(vt, &n)| (short(&vt.id), n))
                            .collect::<BTreeMap<_, _>>()
                    };
                    let roster = s
                        .factions
                        .get(s.players[p].faction as usize)
                        .map_or(&[][..], |f| &f.roster[..]);
                    S7Player {
                        kills_by_vehicle: named(&pl.s7.kills_by_vehicle),
                        kos_by_vehicle: named(&pl.s7.kos_by_vehicle),
                        open_top_throws: pl.s7.open_top_throws,
                        open_top_hits: pl.s7.open_top_hits,
                        open_top_throw_orders: ai.map_or(0, |a| a.counters.open_top_throws),
                        overwatch_holds: ai.map_or(0, |a| a.counters.overwatch_holds),
                        relocations: ai.map_or(0, |a| a.counters.relocations),
                        open_top_backoffs: ai.map_or(0, |a| a.counters.open_top_backoffs),
                        card_cost: roster
                            .iter()
                            .filter_map(|&t| s.squad_types.get(t as usize))
                            .map(|t| (short(&t.id), t.cost))
                            .collect(),
                        vehicle_card: roster
                            .iter()
                            .filter_map(|&t| s.squad_types.get(t as usize))
                            .filter_map(|t| {
                                let v = s.vehicles.get(t.vehicle? as usize)?;
                                Some((short(&v.id), short(&t.id)))
                            })
                            .collect(),
                        tank_td_vehicles: roster
                            .iter()
                            .filter_map(|&t| s.squad_types.get(t as usize))
                            .filter_map(|t| s.vehicles.get(t.vehicle? as usize))
                            .filter(|v| {
                                matches!(
                                    v.role,
                                    frontline_content::VehicleRole::Tank
                                        | frontline_content::VehicleRole::AssaultGun
                                        | frontline_content::VehicleRole::TankDestroyer
                                )
                            })
                            .map(|v| short(&v.id))
                            .collect(),
                        gun_vehicles: roster
                            .iter()
                            .filter_map(|&t| s.squad_types.get(t as usize))
                            .filter_map(|t| s.vehicles.get(t.vehicle? as usize))
                            .filter(|v| v.role == frontline_content::VehicleRole::AtGun)
                            .map(|v| short(&v.id))
                            .collect(),
                    }
                },
                s8a: pl.s8a,
                s8a_obj: ai.map_or((0, 0, 0, 0), |a| {
                    let c = &a.counters;
                    (c.obj_rides, c.obj_ride_ticks, c.obj_walk_ticks, c.obj_lost)
                }),
                s8b_ai: ai.map_or([0; 8], |a| {
                    let c = &a.counters;
                    [
                        c.recon_stops,
                        c.recon_backoffs,
                        c.recon_fails,
                        c.truck_fails,
                        c.truck_flees,
                        c.truck_serves,
                        c.stranded_served,
                        c.truck_drops_ordered,
                    ]
                }),
                s9: pl.s9,
                s10: S10Player {
                    fort: g.fort().accounts.get(p).cloned().unwrap_or_default(),
                    ai: ai.map_or([0; 14], |a| S10Player::ai_counters(&a.counters)),
                    works_done: g
                        .fort()
                        .accounts
                        .get(p)
                        .map(|a| {
                            a.works_done
                                .iter()
                                .enumerate()
                                .filter(|(_, n)| **n > 0)
                                .map(|(k, &n)| {
                                    let id = &s.forts[k].id;
                                    let short = id.split_once(':').map_or(id.as_str(), |x| x.1);
                                    (short.to_string(), n)
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                    works_non_wire: g.fort().accounts.get(p).map_or(0, |a| {
                        a.works_done
                            .iter()
                            .enumerate()
                            .filter(|(k, _)| s.forts[*k].wire.is_none())
                            .map(|(_, &n)| n)
                            .sum()
                    }),
                    hq_roofs: g.fort().accounts.get(p).map_or(0, |a| {
                        a.works_done
                            .iter()
                            .enumerate()
                            .filter(|(k, _)| {
                                s.forts[*k].shape == frontline_content::setup::FortShape::OverHq
                            })
                            .map(|(_, &n)| n)
                            .sum()
                    }),
                    enemy_wire_built: g
                        .fort()
                        .accounts
                        .iter()
                        .enumerate()
                        .filter(|(q, _)| *q != p)
                        .map(|(_, a)| a.wire_built)
                        .sum(),
                    hq_built: g
                        .supply6()
                        .accounts
                        .get(p)
                        .is_some_and(|a| a.hq_built_at > 0),
                    s10b_ordered: s10b_ordered.per_player.get(p).copied().unwrap_or(0),
                    s10b_ai: ai.map_or([0; 14], |a| S10Player::s10b_ai_counters(&a.counters)),
                    bunker_why: ai.map_or([0; 16], |a| a.counters.bunker_why),
                    s10b_chances: s10b_ai_seen.chances.get(p).copied().unwrap_or(0),
                    s10b_materials_g: s10b_ai_seen.materials_g.get(p).copied().unwrap_or(0),
                    s10b_materials_micro: s10b_ai_seen.materials_g.get(p).copied().unwrap_or(0)
                        * s.rules
                            .fortify
                            .as_ref()
                            .map_or(0, |f| f.construction_mp_per_t as u64),
                    income_mp: s.rules.start_manpower as u64
                        + s.rules.income_per_min as u64
                            * (world.tick() / (60 * s.tick_hz.max(1) as u64)),
                    new_cards_bought: fielded
                        .values()
                        .filter(|(o, t)| {
                            owned(o) && {
                                let id = &s.squad_types[*t as usize].id;
                                let short = id.split_once(':').map_or(id.as_str(), |x| x.1);
                                S10A_CARDS.contains(&short)
                            }
                        })
                        .count() as u32,
                },
                death_places: death_places.per.get(p).copied().unwrap_or([0; 6]),
                s10c: {
                    let mut c = s10c_seen.per.get(p).cloned().unwrap_or_default();
                    if let Some(a) = ai {
                        c.ai = S10cPlayer::ai_counters(&a.counters);
                        c.front_why = a.counters.front_why;
                        c.mine_want_why = a.counters.mine_want_why;
                    }
                    c
                },
                squads_at_min: squads_at.get(p).copied().unwrap_or([None; 5]),
                s9_ai: ai.map_or([0; 7], |a| {
                    let c = &a.counters;
                    [
                        c.gun_unhitches,
                        c.guns_in_cover,
                        c.gun_ai_fails,
                        c.gun_pits,
                        c.gun_relays,
                        c.gun_recrew_orders,
                        c.gun_resites,
                    ]
                }),
                s8b_ride: ai.map_or([0; 6], |a| {
                    let c = &a.counters;
                    [
                        c.obj_board_ticks,
                        c.obj_foot_ticks,
                        c.unload_exposed as u64,
                        c.unload_arrived as u64,
                        c.unload_stuck as u64,
                        c.unload_fire as u64,
                    ]
                }),
                s8a_ai: ai.map_or([0; 5], |a| {
                    let c = &a.counters;
                    [
                        c.transport_loads,
                        c.transport_rides,
                        c.transport_unloads,
                        c.transport_unloads_fire,
                        c.transport_fails,
                    ]
                }),
                s8a_cards: {
                    let roster = s
                        .factions
                        .get(s.players[p].faction as usize)
                        .map_or(&[][..], |f| &f.roster[..]);
                    roster
                        .iter()
                        .filter_map(|&t| s.squad_types.get(t as usize))
                        .filter(|t| {
                            t.vehicle
                                .and_then(|v| s.vehicles.get(v as usize))
                                .is_some_and(|v| v.transport.is_some())
                        })
                        .map(|t| t.id.split_once(':').map_or(&t.id[..], |x| x.1).to_owned())
                        .collect()
                },
            }
        })
        .collect::<Vec<PlayerReport>>();
    let st = world.stats();
    let mut kills = [0u32; kill_class::COUNT];
    for pl in g.players() {
        for (k, v) in kills.iter_mut().zip(&pl.kills_by) {
            *k += v;
        }
    }
    let all: u32 = kills.iter().sum();
    let he = kills[kill_class::MORTAR as usize] + kills[kill_class::TANK_HE as usize];
    let he_share_pct = if all == 0 {
        0.0
    } else {
        round3(he as f64 * 100.0 / all as f64)
    };
    let mut ko_causes: BTreeMap<String, u32> = BTreeMap::new();
    for p in &players {
        for (k, v) in &p.armour.disabled_by {
            // S3c: abandonments are tank losses too (`abandoned_<trigger>`).
            if k.starts_with("ko_") || k.starts_with("abandoned_") {
                *ko_causes.entry(k.clone()).or_default() += v;
            }
        }
    }
    let armour = ArmourTotals {
        ko_causes,
        winner_base: winner.map(|w| s.players[w as usize].base),
        penetrations: st.penetrations - stats_at_start.penetrations,
        non_penetrations: st.non_penetrations - stats_at_start.non_penetrations,
        ricochets: st.ricochets - stats_at_start.ricochets,
        knockouts: st.knockouts - stats_at_start.knockouts,
        rockets_fired: st.rockets_fired - stats_at_start.rockets_fired,
        kills_by: names(&kills),
        he_share_pct,
        he_flag: he_share_pct > 60.0,
    };
    let mut s3_failures = Vec::new();
    if !s.vehicles.is_empty() && s.squad_types.iter().any(|t| t.vehicle.is_some()) {
        if winner.is_none() {
            s3_failures.push("no winner".to_string());
        }
        for p in &players {
            if s.players[p.player as usize].is_ai {
                if p.armour.vehicles_bought == 0 {
                    s3_failures.push(format!("player {} fielded no vehicle", p.player));
                }
                if p.armour.at_teams_bought == 0 {
                    s3_failures.push(format!("player {} fielded no AT team", p.player));
                }
            }
        }
        if armour.penetrations == 0 {
            s3_failures.push("no penetration".to_string());
        }
    }
    Ok(SkirmishReport {
        seed: s.seed,
        start_tick,
        loaded_from: args.load.as_ref().map(|p| p.display().to_string()),
        saved,
        threads,
        paced: args.paced,
        result,
        winner,
        winner_faction: winner.map(|w| faction(w as usize)),
        tick_hz: hz,
        ticks: world.tick(),
        game_seconds: round3(world.tick() as f64 / hz as f64),
        limit_minutes: args.minutes,
        final_hash: hex(world.state_hash()),
        players,
        point_names: s.map.points.iter().map(|p| p.name.clone()).collect(),
        totals: Totals {
            shots_fired: st.shots_fired,
            shots_dropped: st.shots_dropped,
            soldier_hits: st.soldier_hits,
            near_misses: st.near_misses,
            explosions: st.explosions,
            deaths: st.deaths,
            throws: st.throws - throws_at_start,
            frag_bursts,
            satchel_bursts,
            smoke_clouds: smoke_seen - smoke_at_start,
            detail_tiles: world.terrain().detail().tiles().len(),
            peak_soldiers: peak,
            peak_tick,
            peak_projectiles: peak_proj,
        },
        armour,
        s3_failures,
        timing,
        wall_ms: round3(wall.as_secs_f64() * 1e3),
    })
}

/// One stderr line per live vehicle (`--trace-vehicles`).
fn trace_vehicles(w: &World) {
    let g = w.game().expect("match world");
    let hz = g.setup().tick_hz as f64;
    for (p, ps) in w.snapshot().players.iter().enumerate() {
        let squads = g
            .squads()
            .values()
            .filter(|q| q.owner.0 as usize == p)
            .count();
        eprintln!(
            "{} p{p} manpower {} pop {}/{} squads {squads} cooldowns {:?} vehicles {}/{}",
            mmss(w.tick() as f64 / hz),
            ps.manpower,
            ps.pop,
            ps.pop_cap,
            ps.cooldowns,
            ps.vehicles_counted,
            ps.vehicle_limit
        );
    }
    for v in &w.snapshot().vehicles {
        if v.flags & vehicle_flags::WRECK != 0 {
            continue;
        }
        let order = g
            .squad(SquadId(v.squad))
            .map_or("-".to_string(), |q| format!("{:?}", q.order));
        let task = g
            .ai(PlayerId(v.owner))
            .and_then(|a| a.tasks.get(&SquadId(v.squad)))
            .map_or("-".to_string(), |t| format!("{:?}@{}", t.kind, t.point));
        eprintln!(
            "{} p{} v{} at ({:.0}, {:.0}) speed {:.1} flags {:#x} hp {:?} seats {:?} ammo {:?} \
             target {} followers {} task {task} order {order}",
            mmss(w.tick() as f64 / hz),
            v.owner,
            v.id,
            v.pos[0],
            v.pos[1],
            v.speed,
            v.flags,
            v.components,
            v.seats,
            v.ammo,
            v.target as i64,
            v.followers,
        );
    }
}

fn mmss(seconds: f64) -> String {
    let s = seconds as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Human summary of a report (several lines, no trailing newline).
pub fn summary_text(r: &SkirmishReport) -> String {
    let head = match (r.result, &r.winner_faction) {
        ("won", Some(f)) => format!("winner: player {} ({f})", r.winner.unwrap_or(0)),
        ("draw", _) => "result: draw".into(),
        _ => format!("result: no winner within {} min", r.limit_minutes),
    };
    let mut out = format!(
        "skirmish seed {} on {} thread(s): {head} after {} ({} ticks); final hash {}\n",
        r.seed,
        r.threads,
        mmss(r.game_seconds),
        r.ticks,
        r.final_hash
    );
    if let Some(from) = &r.loaded_from {
        out += &format!("continued from {from} at tick {}\n", r.start_tick);
    }
    if let Some(sv) = &r.saved {
        out += &format!(
            "saved tick {} ({} bytes, hash {}) to {}\n",
            sv.tick, sv.bytes, sv.hash, sv.path
        );
    }
    out += "player  faction       difficulty  outcome      tickets  manpower  kills  losses  \
            squads  points  hold_s  ai_cmds  rejected\n";
    for p in &r.players {
        let outcome = match (&p.defeat, r.winner) {
            (Some(d), _) => format!("lost ({d})"),
            (None, Some(w)) if w == p.player => "won".into(),
            _ => "-".into(),
        };
        out += &format!(
            "{:<7} {:<13} {:<11} {:<12} {:>7}  {:>8}  {:>5}  {:>6}  {:>6}  {:>6}  {:>6}  {:>7}  {:>8}\n",
            p.player,
            p.faction,
            p.difficulty,
            outcome,
            p.tickets,
            p.manpower,
            p.kills,
            p.losses,
            p.squads_fielded,
            p.points_held,
            p.hold_s,
            p.ai_commands,
            p.ai_rejected,
        );
    }
    let t = &r.totals;
    out += &format!(
        "shots {} ({} dropped), hits {}, near misses {}, explosions {}, deaths {}, \
         terrain tiles {}\n\
         throws {}: frag bursts {}, satchel bursts {}, smoke clouds {}\n",
        t.shots_fired,
        t.shots_dropped,
        t.soldier_hits,
        t.near_misses,
        t.explosions,
        t.deaths,
        t.detail_tiles,
        t.throws,
        t.frag_bursts,
        t.satchel_bursts,
        t.smoke_clouds
    );
    let am = &r.armour;
    let kills = |m: &BTreeMap<String, u32>| {
        m.iter()
            .filter(|(_, v)| **v > 0)
            .map(|(k, v)| format!("{k} {v}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    out += &format!(
        "armour: penetrations {}, non-penetrations {}, ricochets {}, knocked out {}, \
         rockets {}; winner base {}\n\
         knock-outs by cause: {}\n\
         kills by weapon: {}\n",
        am.penetrations,
        am.non_penetrations,
        am.ricochets,
        am.knockouts,
        am.rockets_fired,
        am.winner_base.map_or("-".into(), |b| b.to_string()),
        kills(&am.ko_causes),
        kills(&am.kills_by),
    );
    for p in &r.players {
        let a = &p.armour;
        let ratio = |lost: u32, killed: u32| {
            if killed == 0 {
                format!("{lost}/0")
            } else {
                format!("{:.2}", lost as f64 / killed as f64)
            }
        };
        out += &format!(
            "  p{}: vehicles bought {}, lost {}, bogged {}, repaired {}; AT teams {}; \
             KO by gun {}, by rocket {}; disabled: {}\n\
             \x20     mp lost/killed: tanks {} ({}/{}), AT {} ({}/{}); escorts {} (reached \
             {}, released {}), AT shots {}, satchels at vehicles {}, repairs {}, \
             retreats {}, faces {}\n\
             \x20     S3b AI: suppress orders {}, bounds {}, AT hunts {}, satchel hunts {}\n\
             \x20     fired: gun hits {} (pen {}), HE on plates {}, rocket hits {} (pen {}, \
             parts destroyed {}); hits by plate: {}\n\
             \x20     kills: {}\n",
            p.player,
            a.vehicles_bought,
            a.vehicles_lost,
            a.bogged,
            a.repaired,
            a.at_teams_bought,
            a.ko_by_gun,
            a.ko_by_rocket,
            kills(&a.disabled_by),
            ratio(a.tank_mp_lost, a.tank_mp_killed),
            a.tank_mp_lost,
            a.tank_mp_killed,
            ratio(a.at_mp_lost, a.at_mp_killed),
            a.at_mp_lost,
            a.at_mp_killed,
            a.escorts_started,
            a.escorts_reached,
            a.escorts_released,
            a.at_shots,
            a.satchels_at_vehicles,
            a.repairs,
            a.retreats,
            a.faces,
            a.suppress_orders,
            a.bounds,
            a.at_hunts,
            a.satchel_hunts,
            a.gun_hits,
            a.gun_pens,
            a.he_on_plate,
            a.rocket_hits,
            a.rocket_pens,
            a.rocket_disabled_parts,
            kills(&a.hits_by_zone),
            kills(&a.kills_by),
        );
    }
    if am.he_flag {
        out += &format!(
            "FLAG: HE share {:.1} % of kills (mortar + tank HE above 60 %)\n",
            am.he_share_pct
        );
    } else {
        out += &format!("HE share {:.1} % of kills\n", am.he_share_pct);
    }
    if !r.s3_failures.is_empty() {
        out += &format!("S3 checks FAILED: {}\n", r.s3_failures.join("; "));
    }
    let (a, k) = (&r.timing.all, &r.timing.peak);
    out += &format!(
        "tick ms: mean {:.3}, p95 {:.3}, p99 {:.3}, max {:.3}; peak {} soldiers at {} \
         ({} ticks >= {} %): p99 {:.3}, max {:.3}; overruns {}; wall {:.1} s{}",
        a.mean_ms,
        a.p95_ms,
        a.p99_ms,
        a.max_ms,
        t.peak_soldiers,
        mmss(t.peak_tick as f64 / r.tick_hz as f64),
        r.timing.peak_ticks,
        r.timing.peak_share_pct,
        k.p99_ms,
        k.max_ms,
        r.timing.overruns,
        r.wall_ms / 1e3,
        if r.paced { " (paced)" } else { "" }
    );
    out
}

// ------------------------------------------------- many seeds (S3b A/B) --

/// One match of a `--seeds` run.
#[derive(Clone, Debug, Serialize)]
pub struct ManyMatch {
    pub seed: u64,
    /// Played with `--swap-bases` (player 0 at base 1).
    pub swapped: bool,
    pub report: SkirmishReport,
}

/// One faction's numbers over a `--seeds` run.
#[derive(Clone, Debug, Default, Serialize)]
pub struct FactionSummary {
    pub faction: String,
    pub wins: u32,
    /// Wins from base 0 and from base 1.
    pub wins_by_base: [u32; 2],
    pub tanks_bought: u32,
    pub tanks_lost: u32,
    /// Enemy tanks knocked out by this faction's guns and rockets.
    pub ko_by_gun: u32,
    pub ko_by_rocket: u32,
    pub rockets_fired: u32,
    pub rocket_hits: u32,
    pub rocket_pens: u32,
    /// Matches in which this faction fired at least one rocket.
    pub matches_with_rockets: u32,
    pub satchels_at_vehicles: u32,
    pub matches_with_satchels: u32,
    pub kills: u32,
    pub losses: u32,
    /// S3c: own tanks lost destroyed and abandoned (they add up to
    /// `tanks_lost`), abandonments by trigger, what became of them, enemy
    /// tanks captured, and own crewmen at bail-outs (alive, out, home).
    pub tanks_destroyed: u32,
    pub tanks_abandoned: u32,
    pub abandon_causes: BTreeMap<String, u32>,
    pub abandoned_fates: BTreeMap<String, u32>,
    pub captures: u32,
    pub crew_bailed: u32,
    pub crew_out: u32,
    pub crew_home: u32,
    /// S3c Part B AI counters, summed.
    pub ai_s3c: BTreeMap<String, u32>,
    /// S4 counters over the run.
    pub s4: S4Faction,
    /// S5a counters over the run.
    pub s5a: S5aFaction,
    /// S5b counters over the run.
    pub s5b: S5bFaction,
    /// S6 counters over the run.
    pub s6: S6Faction,
    /// S7 counters over the run.
    pub s7: S7Faction,
    /// S8a counters over the run.
    pub s8a: S8aFaction,
    /// S10a counters over the run (sums of `S10Player`).
    pub s10: S10Faction,
    /// S10b counters over the run (sums of the twelve `FortAccount` S10b
    /// counters; all 0 in AI play).
    pub s10b: S10bFaction,
    /// S10b-ai sums (the JSON block is `s10b_ai`, counters only).
    pub s10b_ai: S10bAiFaction,
    /// S10c-ai sums (the JSON blocks `s10c` and `s10c_derived`).
    pub s10c: S10cFaction,
    /// S11a1 sums (the JSON blocks `s11a1` and `s11a1_derived`).
    pub s11a1: S11a1Faction,
}

/// S10b: the twelve `FortAccount` counters in report order.
pub const S10B_COUNTERS: [&str; 12] = [
    "mines_laid",
    "mines_set_off",
    "mines_blasted",
    "mines_found",
    "mines_lifted",
    "men_lost_to_mines",
    "vehicles_mined",
    "obstacles_built",
    "bunkers_built",
    "sweeps",
    "demolitions",
    "pieces_demolished",
];

/// S10b numbers of one faction over a `--seeds` run (sums; spec 7).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S10bFaction {
    pub matches: u32,
    /// [`S10B_COUNTERS`] order.
    pub counters: [u64; 12],
    /// S10b use that need not finish (review R13): works started with an
    /// S10b blueprint and squads that took a ClearMines or Demolish order
    /// ([`S10bOrdered`]).
    pub ordered: u64,
}

/// Per-player S10b use seen tick by tick (review R13): works of an S10b
/// blueprint (a minefield, an obstacle, the bunker) by id, and squads that
/// enter a ClearMines or Demolish order. Counts started, interrupted and
/// refused-later use the finished-work counters miss.
#[derive(Clone, Debug, Default)]
pub struct S10bOrdered {
    works: BTreeSet<u32>,
    in_order: BTreeSet<SquadId>,
    pub per_player: Vec<u32>,
}

impl S10bOrdered {
    /// Whether blueprint `f` is an S10b one.
    pub fn s10b_blueprint(f: &frontline_content::setup::FortSetup) -> bool {
        f.mine_item.is_some()
            || f.blocks_vehicles
            || matches!(f.pit, Some(frontline_content::setup::DigProfile::BunkerPit))
    }

    pub fn observe(&mut self, w: &World) {
        let Some(g) = w.game() else { return };
        let s = g.setup();
        if self.per_player.len() < s.players.len() {
            self.per_player.resize(s.players.len(), 0);
        }
        for (&id, wk) in &g.fort().works {
            let s10b = s
                .forts
                .get(wk.fort as usize)
                .is_some_and(Self::s10b_blueprint);
            if s10b
                && self.works.insert(id)
                && let Some(n) = self.per_player.get_mut(wk.owner.0 as usize)
            {
                *n += 1;
            }
        }
        for (&id, q) in g.squads() {
            use frontline_sim::game::Order;
            let on = matches!(q.order, Order::ClearMines { .. } | Order::Demolish { .. });
            if !on {
                self.in_order.remove(&id);
            } else if self.in_order.insert(id)
                && let Some(n) = self.per_player.get_mut(q.owner.0 as usize)
            {
                *n += 1;
            }
        }
    }
}

impl S10bFaction {
    /// The twelve S10b counters of one account ([`S10B_COUNTERS`] order).
    pub fn of(a: &frontline_sim::fort::FortAccount) -> [u32; 12] {
        [
            a.mines_laid,
            a.mines_set_off,
            a.mines_blasted,
            a.mines_found,
            a.mines_lifted,
            a.men_lost_to_mines,
            a.vehicles_mined,
            a.obstacles_built,
            a.bunkers_built,
            a.sweeps,
            a.demolitions,
            a.pieces_demolished,
        ]
    }

    pub fn add(&mut self, a: &frontline_sim::fort::FortAccount, ordered: u32) {
        self.matches += 1;
        self.ordered += ordered as u64;
        for (k, v) in Self::of(a).iter().enumerate() {
            self.counters[k] += *v as u64;
        }
    }

    fn get(&self, name: &str) -> u64 {
        S10B_COUNTERS
            .iter()
            .position(|k| *k == name)
            .map_or(0, |k| self.counters[k])
    }

    /// Mechanics used at all: `mines_laid + obstacles_built +
    /// bunkers_built + sweeps + demolitions + ordered` (0 in AI play,
    /// R28; `ordered` since review R13).
    pub fn used(&self) -> u64 {
        [
            "mines_laid",
            "obstacles_built",
            "bunkers_built",
            "sweeps",
            "demolitions",
        ]
        .iter()
        .map(|k| self.get(k))
        .sum::<u64>()
            + self.ordered
    }
}

/// S10b-ai numbers of one faction over a `--seeds` run (sums and per-match
/// tallies of `s10b_ai_derived`; `docs/slices/S10b-ai-build-A.md` 8). The
/// JSON block is [`crate::json_out::s10b_ai_blocks`]: only counters, so the
/// all-off and no-behaviour runs stay blank for `neutral.py`.
#[derive(Clone, Debug, Default, Serialize)]
pub struct S10bAiFaction {
    pub matches: u32,
    /// [`S10B_AI`] order.
    pub ai: [u64; 14],
    pub lost_on_known: u64,
    pub at_exposure_ticks: u64,
    /// Ticks per second of the runs (`at_exposure_s_per_match`).
    pub tick_hz: u32,
    pub own_mine_losses: u64,
    pub own_pieces_demolished: u64,
    pub mines_laid: u64,
    /// `works_done.at_minefield`.
    pub at_fields: u64,
    pub vehicles_mined: u64,
    pub obstacles_built: u64,
    pub bunkers_built: u64,
    pub mines_lifted: u64,
    pub materials_micro: u64,
    pub income_mp: u64,
    pub matches_with_mines: u32,
    pub lay_chance_matches: u32,
    pub matches_with_obstacle: u32,
    pub obst_chance_matches: u32,
    /// Matches with a bunker built, among the bunker-chance matches (the
    /// matches in which the AI held a bunker job).
    pub bunker_matches: u32,
    pub bunker_chance_matches: u32,
    /// Review finding 8: matches with a bunker built (all matches), and
    /// matches whose bunker plans got past the point checks (a
    /// `bunker_why` count from `no_site` on, or a bunker job).
    pub matches_with_bunker: u32,
    pub bunker_ready_matches: u32,
    pub sweep_wanted_matches: u32,
    /// Matches with a sweep wanted and one started.
    pub sweep_when_wanted: u32,
    pub demolish_wanted_matches: u32,
    pub demolish_when_wanted: u32,
    /// Sums of [`S10Player::bunker_why`].
    pub bunker_why: [u64; 16],
}

impl S10bAiFaction {
    /// The sum of counter `name` ([`S10B_AI`]).
    pub fn get(&self, name: &str) -> u64 {
        S10B_AI
            .iter()
            .position(|k| *k == name)
            .map_or(0, |k| self.ai[k])
    }

    /// Adds one match of one player (`tick_hz` of its setup).
    pub fn add(&mut self, p: &S10Player, tick_hz: u32) {
        let a = &p.fort;
        let c = |name: &str| {
            S10B_AI
                .iter()
                .position(|k| *k == name)
                .map_or(0, |k| p.s10b_ai[k])
        };
        self.matches += 1;
        self.tick_hz = tick_hz;
        for (k, v) in p.s10b_ai.iter().enumerate() {
            self.ai[k] += *v as u64;
        }
        for (k, v) in p.bunker_why.iter().enumerate() {
            self.bunker_why[k] += *v as u64;
        }
        self.lost_on_known += a.lost_on_known as u64;
        self.at_exposure_ticks += a.at_exposure_ticks;
        self.own_mine_losses += a.own_mine_losses as u64;
        self.own_pieces_demolished += a.own_pieces_demolished as u64;
        self.mines_laid += a.mines_laid as u64;
        self.at_fields += p.works_done.get("at_minefield").copied().unwrap_or(0) as u64;
        self.vehicles_mined += a.vehicles_mined as u64;
        self.obstacles_built += a.obstacles_built as u64;
        self.bunkers_built += a.bunkers_built as u64;
        self.mines_lifted += a.mines_lifted as u64;
        self.materials_micro += p.s10b_materials_micro;
        self.income_mp += p.income_mp;
        self.matches_with_mines += u32::from(a.mines_laid > 0);
        self.lay_chance_matches += u32::from(p.s10b_chances & CHANCE_LAY != 0);
        self.matches_with_obstacle += u32::from(a.obstacles_built > 0);
        self.obst_chance_matches += u32::from(p.s10b_chances & CHANCE_OBST != 0);
        self.matches_with_bunker += u32::from(a.bunkers_built > 0);
        let ready = p.bunker_why[2..].iter().any(|&n| n > 0);
        self.bunker_ready_matches += u32::from(ready || p.s10b_chances & CHANCE_BUNKER != 0);
        if p.s10b_chances & CHANCE_BUNKER != 0 {
            self.bunker_chance_matches += 1;
            self.bunker_matches += u32::from(a.bunkers_built > 0);
        }
        if c("sweep_wanted") > 0 {
            self.sweep_wanted_matches += 1;
            self.sweep_when_wanted += u32::from(c("sweeps_started") > 0);
        }
        if c("demolish_wanted") > 0 {
            self.demolish_wanted_matches += 1;
            self.demolish_when_wanted += u32::from(c("demolitions_started") > 0);
        }
    }
}

/// S10c-ai numbers of one faction over a `--seeds` run (sums and per-match
/// tallies of `s10c_derived`; docs/slices/S10c-ai.md 7). The JSON blocks
/// are [`crate::json_out::s10c_blocks`].
#[derive(Clone, Debug, Default, Serialize)]
pub struct S10cFaction {
    pub matches: u32,
    /// [`S10C_AI`] order.
    pub ai: [u64; 12],
    pub front_why: [u64; 13],
    pub mine_want_why: [u64; 8],
    pub works_by_point: Vec<u64>,
    pub mines_by_point: Vec<u64>,
    pub mines_home: u64,
    pub mines_front: u64,
    pub mines_seen: u64,
    pub mines_approached: u64,
    /// `s10b_jobs` (the AI's S10b jobs) and own mines set off (account).
    pub s10b_jobs: u64,
    pub mines_set_off: u64,
    /// Matches with own mines laid (`FortAccount::mines_laid`), and of
    /// those with one approached; with own wire standing, and of those
    /// seen by the enemy AI and approached; with an own blocking piece
    /// approached; where this side's AI knew enemy wire; with a front
    /// field; with a belt job.
    pub matches_with_mines: u32,
    pub matches_mines_approached: u32,
    pub matches_with_wire: u32,
    pub matches_wire_seen: u32,
    pub matches_wire_approached: u32,
    pub matches_obstacle_approached: u32,
    pub matches_knew_enemy_wire: u32,
    pub matches_with_front_field: u32,
    pub matches_with_belt: u32,
}

impl S10cFaction {
    /// The sum of counter `name` ([`S10C_AI`]).
    pub fn get(&self, name: &str) -> u64 {
        S10C_AI
            .iter()
            .position(|k| *k == name)
            .map_or(0, |k| self.ai[k])
    }

    /// Adds one match of one player.
    pub fn add(&mut self, c: &S10cPlayer, s10: &S10Player) {
        self.matches += 1;
        for (k, v) in c.ai.iter().enumerate() {
            self.ai[k] += *v as u64;
        }
        for (k, v) in c.front_why.iter().enumerate() {
            self.front_why[k] += *v as u64;
        }
        for (k, v) in c.mine_want_why.iter().enumerate() {
            self.mine_want_why[k] += *v as u64;
        }
        for (sum, v) in [
            (&mut self.works_by_point, &c.works_by_point),
            (&mut self.mines_by_point, &c.mines_by_point),
        ] {
            if sum.len() < v.len() {
                sum.resize(v.len(), 0);
            }
            for (k, n) in v.iter().enumerate() {
                sum[k] += *n as u64;
            }
        }
        self.mines_home += c.mines_home as u64;
        self.mines_front += c.mines_front as u64;
        self.mines_seen += c.mines_seen as u64;
        self.mines_approached += c.mines_approached as u64;
        self.s10b_jobs += s10.s10b_ai[0] as u64;
        self.mines_set_off += s10.fort.mines_set_off as u64;
        let mines = s10.fort.mines_laid > 0;
        self.matches_with_mines += u32::from(mines);
        self.matches_mines_approached += u32::from(mines && c.mines_approached > 0);
        self.matches_with_wire += u32::from(c.wire_stood);
        self.matches_wire_seen += u32::from(c.wire_stood && c.wire_seen_by_enemy);
        self.matches_wire_approached += u32::from(c.wire_stood && c.wire_approached);
        self.matches_obstacle_approached += u32::from(c.obstacle_approached);
        self.matches_knew_enemy_wire += u32::from(c.knew_enemy_wire);
        self.matches_with_front_field += u32::from(c.front_field);
        self.matches_with_belt += u32::from(c.ai[9] > 0);
    }
}

/// S10a numbers of one faction over a `--seeds` run (sums; S10a-ai adds
/// the AI's counters and the per-match tallies of the derived shares).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S10Faction {
    pub matches: u32,
    pub works_started: u64,
    pub pieces_built: u64,
    pub pieces_lost: u64,
    pub wire_built: u64,
    pub wire_cut: u64,
    pub wire_blown: u64,
    pub wire_crushed: u64,
    pub bangalores: u64,
    pub materials_loaded_g: u64,
    pub materials_used_g: u64,
    pub new_cards_bought: u64,
    /// S10a-ai sums.
    pub ai: [u64; 14],
    pub works_done: BTreeMap<String, u64>,
    pub works_non_wire: u64,
    pub materials_micro: u64,
    pub materials_lost_g: u64,
    pub materials_used_crate_g: u64,
    pub materials_used_truck_g: u64,
    pub men_lost_in_wire: u64,
    pub income_mp: u64,
    /// S10a-ai: matches with a non-wire work completed, with wire built,
    /// with a gap made (`wire_cut + wire_blown > 0`), where the enemy built
    /// wire and of those with a breach opened, with an HQ built and of
    /// those with it roofed.
    pub matches_with_works: u32,
    pub matches_with_wire: u32,
    pub matches_with_gap: u32,
    pub matches_enemy_wire: u32,
    pub breach_when_enemy_wire: u32,
    pub matches_hq_built: u32,
    pub matches_hq_roofed: u32,
}

impl S10Faction {
    pub fn add(&mut self, p: &S10Player) {
        let a = &p.fort;
        for (k, v) in p.ai.iter().enumerate() {
            self.ai[k] += *v as u64;
        }
        for (k, v) in &p.works_done {
            *self.works_done.entry(k.clone()).or_default() += *v as u64;
        }
        self.works_non_wire += p.works_non_wire as u64;
        self.materials_micro += a.materials_micro;
        self.materials_lost_g += a.materials_lost_g;
        self.materials_used_crate_g += a.materials_used_crate_g;
        self.materials_used_truck_g += a.materials_used_truck_g;
        self.men_lost_in_wire += a.men_lost_in_wire as u64;
        self.income_mp += p.income_mp;
        self.matches_with_works += u32::from(p.works_non_wire > 0);
        self.matches_with_wire += u32::from(a.wire_built > 0);
        self.matches_with_gap += u32::from(a.wire_cut + a.wire_blown > 0);
        // Fix F10: only matches where the enemy's wire stood on a route
        // this AI took (it went round it, wire_bypassed, S10_AI 13, or
        // started a breach, 8): a faction cannot breach wire it never met.
        if p.enemy_wire_built > 0 && (p.ai[8] > 0 || p.ai[13] > 0) {
            self.matches_enemy_wire += 1;
            // breaches_opened (S10_AI index 9).
            self.breach_when_enemy_wire += u32::from(p.ai[9] > 0);
        }
        if p.hq_built {
            self.matches_hq_built += 1;
            self.matches_hq_roofed += u32::from(p.hq_roofs > 0);
        }
        self.matches += 1;
        self.works_started += a.works_started as u64;
        self.pieces_built += a.pieces_built as u64;
        self.pieces_lost += a.pieces_lost as u64;
        self.wire_built += a.wire_built as u64;
        self.wire_cut += a.wire_cut as u64;
        self.wire_blown += a.wire_blown as u64;
        self.wire_crushed += a.wire_crushed as u64;
        self.bangalores += a.bangalores as u64;
        self.materials_loaded_g += a.materials_loaded_g;
        self.materials_used_g += a.materials_used_g;
        self.new_cards_bought += p.new_cards_bought as u64;
    }
}

/// S8a numbers of one faction over a `--seeds` run (sums of
/// `frontline_sim::S8aCounters`; the transport cards as seen).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S8aFaction {
    pub matches: u32,
    pub rides: u64,
    pub transported: u64,
    pub ride_ticks: u64,
    pub ride_walk_ticks: u64,
    pub riders_lost: u64,
    pub small_arms_pens: u64,
    pub dismounts_under_fire: u64,
    pub transport_trips: u64,
    /// AI: loads, rides, unloads, unloads under fire, failed moves.
    pub ai_loads: u64,
    pub ai_rides: u64,
    pub ai_unloads: u64,
    pub ai_unloads_fire: u64,
    pub ai_fails: u64,
    /// Review M2: transported squads that reached their point, ticks from
    /// the Embark order to arrival, ticks of the straight walk from the
    /// order position, squads lost after boarding.
    pub obj_rides: u64,
    pub obj_ride_ticks: u64,
    pub obj_walk_ticks: u64,
    pub obj_lost: u64,
    /// S8b: obj_board_ticks, obj_foot_ticks, unloads exposed, arrived,
    /// stuck, under fire.
    pub ride: [u64; 6],
    /// S8b: `Player::s8b_ai`, and the trucks' totals (`S6Account`).
    pub ai8b: [u64; 8],
    pub truck_loaded_g: u64,
    pub truck_tank_rounds: u64,
    pub truck_men_rounds: u64,
    pub truck_crates_dropped: u64,
    pub truck_lost_g: u64,
    pub cargo_detonations: u64,
    /// S9: `Player::s9` and `s9_ai` (sums), and matches with a gun round fired.
    pub s9: frontline_sim::S9Counters,
    pub s9_ai: [u64; 7],
    pub gun_fired_matches: u64,
    /// S9b: enemy armour put out of action by this faction's guns
    /// (`PlayerArmour::gun_armour_out`, summed).
    pub gun_armour_out: u64,
    #[serde(skip)]
    pub transport_cards: std::collections::BTreeSet<String>,
}

/// S7 numbers of one faction over a `--seeds` run (sums; the card prices
/// and vehicle cards as seen).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S7Faction {
    pub matches: u32,
    pub kills_by_vehicle: BTreeMap<String, u64>,
    pub kos_by_vehicle: BTreeMap<String, u64>,
    pub open_top_throws: u64,
    pub open_top_hits: u64,
    pub open_top_throw_orders: u64,
    pub overwatch_holds: u64,
    pub relocations: u64,
    pub open_top_backoffs: u64,
    #[serde(skip)]
    pub card_cost: BTreeMap<String, u32>,
    #[serde(skip)]
    pub vehicle_card: BTreeMap<String, String>,
    /// S9 (T33): tanks, assault guns and tank destroyers by short id.
    #[serde(skip)]
    pub tank_td_vehicles: BTreeSet<String>,
    #[serde(skip)]
    pub gun_vehicles: BTreeSet<String>,
}

/// S6 numbers of one faction over a `--seeds` run (sums).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S6Faction {
    pub matches: u32,
    /// Matches in which an HQ of this faction stood.
    pub hq_built: u32,
    pub structures_built: u64,
    pub structures_lost: u64,
    pub crates_issued: [u64; 3],
    pub crate_micro: u64,
    pub crates_delivered: u64,
    pub crates_lost: u64,
    pub stock_in_g: u64,
    pub stock_out_g: u64,
    pub stock_lost_g: u64,
    pub stock_held_g: u64,
    pub base_bombs: u64,
    pub structure_bombs: u64,
    pub crate_bombs: u64,
    pub unsourced_bombs: u64,
    pub fwd_rounds: u64,
    pub fwd_items: u64,
    pub tank_rounds_fwd: u64,
    pub tank_rare_fwd: u64,
    pub tank_crate_rounds: u64,
    pub rack_credit_mp: u64,
    pub cookoffs: u64,
    pub tank_crate_runs: u64,
    pub crate_pickups: u64,
}

/// S5b numbers of one faction over a `--seeds` run (sums).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S5bFaction {
    pub matches: u32,
    pub missions: [u64; 3],
    pub refused: [u64; frontline_sim::FireMissionRefusal::COUNT],
    pub bombs: [u64; 3],
    pub miss_mm: [u64; 3],
    pub seen: u64,
    pub unseen: u64,
    pub adjust: u64,
    pub flight_ticks: u64,
    pub base_bombs: [u64; 3],
    pub upgrades: u64,
    pub downgrades: u64,
    pub no_solution: u64,
    /// Part B AI counters, summed.
    pub ai: S5bAi,
}

/// S5a numbers of one faction over a `--seeds` run.
#[derive(Clone, Debug, Default, Serialize)]
pub struct S5aFaction {
    pub matches: u32,
    pub fired: [u64; fired_class::COUNT],
    pub paid_micro: [u64; 3],
    pub refilled: [u64; 3],
    pub paused_s: u64,
    /// Enemy men killed by class (the HE share by source).
    pub kills_by: [u64; kill_class::COUNT],
    /// Part B AI counters, summed.
    pub ai: S5aAi,
    /// Matches with refill trips; normal-base matches with a Panzerfaust
    /// fired from an ambush.
    pub matches_with_refill: u32,
    pub matches_ambush_fired_normal: u32,
}

/// S4 numbers of one faction over a `--seeds` run.
#[derive(Clone, Debug, Default, Serialize)]
pub struct S4Faction {
    pub matches: u32,
    /// Per card id: matches with at least one bought, and the total bought.
    pub bought_matches: BTreeMap<String, u32>,
    pub bought: BTreeMap<String, u32>,
    pub panzerfausts: u32,
    pub pzf_hits: u32,
    pub pzf_pens: u32,
    pub ko_by_pzf: u32,
    /// Matches with at least one Panzerfaust fired: all, and with normal
    /// bases (player 0 at base 0).
    pub matches_with_pzf: u32,
    pub matches_with_pzf_normal: u32,
    pub pickups: u32,
    pub pzf_hunts: u32,
    pub capture_orders: u32,
    pub pop_cap_holds: u32,
    /// Population at `POP_MINUTES`: sums and how many matches were still
    /// running then.
    pub pop_sum: [u64; 3],
    pub pop_n: [u32; 3],
}

/// The compact table of a `--seeds` run (`--summary`).
#[derive(Clone, Debug, Default, Serialize)]
pub struct ManySummary {
    pub matches: u32,
    /// Matches without a winner (draw or time limit).
    pub no_winner: u32,
    /// Wins from base 0 and from base 1, any faction.
    pub wins_by_base: [u32; 2],
    pub factions: Vec<FactionSummary>,
    pub minutes_mean: f64,
    pub minutes_min: f64,
    pub minutes_max: f64,
    pub over_25_min: u32,
    /// Tank knock-outs by cause, both sides.
    pub ko_causes: BTreeMap<String, u32>,
    pub he_share_mean_pct: f64,
    pub he_share_max_pct: f64,
    /// Highest per-match tick p99 over the peak ticks and over all ticks
    /// (ms; matches run in parallel, so they read high).
    pub peak_p99_max_ms: f64,
    pub all_p99_max_ms: f64,
    /// Matches run at once.
    pub jobs: usize,
    /// S3c baseline lines (report only).
    pub s3c: S3cSummary,
}

/// The result of `skirmish --seeds`.
#[derive(Clone, Debug, Serialize)]
pub struct ManyReport {
    pub matches: Vec<ManyMatch>,
    pub summary: ManySummary,
    /// `--rules-override` log lines (what each file changed).
    pub overrides: Vec<String>,
}

/// Runs `--seeds` matches (both base orders with `--both-bases`) on a pool
/// of `--jobs` threads. Every match is the same as a single `skirmish` run
/// with its seed (and `--swap-bases`), so its hash is too.
pub fn skirmish_many(args: &SkirmishArgs) -> Result<ManyReport> {
    let n = args.seeds.unwrap_or(1);
    if n == 0 {
        bail!("--seeds must be at least 1");
    }
    let threads = match args.threads {
        Some(0) => bail!("--threads must be at least 1"),
        Some(t) => t,
        None => 1,
    };
    let cores = std::thread::available_parallelism().map_or(1, |c| c.get());
    let jobs = match args.jobs {
        Some(0) => bail!("--jobs must be at least 1"),
        Some(j) => j,
        None => (cores / threads).max(1),
    };
    // Checks the overrides once (and gives the log) before any thread runs.
    let (_, overrides) = crate::overrides::load_defs(args.mods.as_deref(), &args.rules_override)?;
    let mut runs: Vec<(u64, bool)> = Vec::new();
    for seed in args.seed..args.seed + n {
        runs.push((seed, args.swap_bases));
        if args.both_bases {
            runs.push((seed, true));
        }
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: std::sync::Mutex<Vec<(usize, Result<SkirmishReport>)>> =
        std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..jobs.min(runs.len()) {
            scope.spawn(|| {
                loop {
                    let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(&(seed, swap)) = runs.get(k) else {
                        break;
                    };
                    let one = SkirmishArgs {
                        seed,
                        swap_bases: swap,
                        threads: Some(threads),
                        seeds: None,
                        both_bases: false,
                        summary: false,
                        jobs: None,
                        ab: false,
                        ..args.clone()
                    };
                    let r = skirmish(&one);
                    results.lock().expect("no panics").push((k, r));
                }
            });
        }
    });
    let mut results = results.into_inner().expect("no panics");
    results.sort_by_key(|r| r.0);
    let mut matches = Vec::with_capacity(results.len());
    for (k, r) in results {
        let (seed, swapped) = runs[k];
        let report =
            r.with_context(|| format!("seed {seed}{}", if swapped { " swapped" } else { "" }))?;
        matches.push(ManyMatch {
            seed,
            swapped,
            report,
        });
    }
    let summary = summarise(&matches, jobs);
    Ok(ManyReport {
        matches,
        summary,
        overrides,
    })
}

pub(crate) fn summarise(matches: &[ManyMatch], jobs: usize) -> ManySummary {
    let mut s = ManySummary {
        matches: matches.len() as u32,
        minutes_min: f64::MAX,
        jobs,
        ..ManySummary::default()
    };
    let mut factions: BTreeMap<String, FactionSummary> = BTreeMap::new();
    let mut minutes = 0.0;
    let mut he = 0.0;
    for m in matches {
        let r = &m.report;
        let swapped = m.swapped;
        let min = r.game_seconds / 60.0;
        minutes += min;
        s.minutes_min = s.minutes_min.min(min);
        s.minutes_max = s.minutes_max.max(min);
        if min > 25.0 {
            s.over_25_min += 1;
        }
        he += r.armour.he_share_pct;
        s.he_share_max_pct = s.he_share_max_pct.max(r.armour.he_share_pct);
        s.peak_p99_max_ms = s.peak_p99_max_ms.max(r.timing.peak.p99_ms);
        s.all_p99_max_ms = s.all_p99_max_ms.max(r.timing.all.p99_ms);
        for (k, v) in &r.armour.ko_causes {
            *s.ko_causes.entry(k.clone()).or_default() += v;
        }
        match (r.winner, r.armour.winner_base) {
            (Some(_), Some(b)) => s.wins_by_base[(b as usize).min(1)] += 1,
            (Some(_), None) => {}
            (None, _) => s.no_winner += 1,
        }
        for p in &r.players {
            let f = factions
                .entry(p.faction.clone())
                .or_insert_with(|| FactionSummary {
                    faction: p.faction.clone(),
                    ..FactionSummary::default()
                });
            if r.winner == Some(p.player) {
                f.wins += 1;
                if let Some(b) = r.armour.winner_base {
                    f.wins_by_base[(b as usize).min(1)] += 1;
                }
            }
            let a = &p.armour;
            f.tanks_bought += a.vehicles_bought;
            f.tanks_lost += a.vehicles_lost;
            f.ko_by_gun += a.ko_by_gun;
            f.ko_by_rocket += a.ko_by_rocket;
            f.rockets_fired += a.at_shots;
            f.rocket_hits += a.rocket_hits;
            f.rocket_pens += a.rocket_pens;
            f.matches_with_rockets += u32::from(a.at_shots > 0);
            f.satchels_at_vehicles += a.satchels_at_vehicles;
            f.matches_with_satchels += u32::from(a.satchels_at_vehicles > 0);
            f.kills += p.kills;
            f.losses += p.losses;
            f.tanks_destroyed += a.vehicles_destroyed;
            f.tanks_abandoned += a.vehicles_abandoned;
            for (k, v) in &a.abandon_causes {
                *f.abandon_causes.entry(k.clone()).or_default() += v;
            }
            for (k, v) in &a.ai_s3c {
                *f.ai_s3c.entry(k.clone()).or_default() += v;
            }
            for (k, v) in &a.abandoned_fates {
                *f.abandoned_fates.entry(k.clone()).or_default() += v;
            }
            f.captures += a.captures;
            f.crew_bailed += a.crew_bailed;
            f.crew_out += a.crew_out;
            f.crew_home += a.crew_home;
            let q = &p.s4;
            let s4 = &mut f.s4;
            s4.matches += 1;
            for (k, &v) in &q.bought {
                *s4.bought.entry(k.clone()).or_default() += v;
                *s4.bought_matches.entry(k.clone()).or_default() += u32::from(v > 0);
            }
            s4.panzerfausts += q.panzerfausts;
            s4.pzf_hits += a.pzf_hits;
            s4.pzf_pens += a.pzf_pens;
            s4.ko_by_pzf += a.ko_by_pzf;
            s4.matches_with_pzf += u32::from(q.panzerfausts > 0);
            s4.matches_with_pzf_normal += u32::from(q.panzerfausts > 0 && !swapped);
            s4.pickups += q.pickups;
            let q5 = &p.s5a;
            let s5 = &mut f.s5a;
            s5.matches += 1;
            for k in 0..fired_class::COUNT {
                s5.fired[k] += q5.fired[k] as u64;
            }
            for k in 0..3 {
                s5.paid_micro[k] += q5.paid_micro[k];
                s5.refilled[k] += q5.refilled[k] as u64;
            }
            s5.paused_s += q5.paused_s as u64;
            for k in 0..kill_class::COUNT {
                s5.kills_by[k] += q5.kills_by[k] as u64;
            }
            s5.ai.add(&q5.ai);
            s5.matches_with_refill += u32::from(q5.ai.refill_trips > 0);
            s5.matches_ambush_fired_normal += u32::from(q5.ai.ambush_shots > 0 && !swapped);
            let b = &p.s5b;
            let s6 = &mut f.s5b;
            s6.matches += 1;
            for k in 0..3 {
                s6.missions[k] += b.missions[k] as u64;
                s6.bombs[k] += b.bombs[k] as u64;
                s6.miss_mm[k] += b.miss_mm[k];
                s6.base_bombs[k] += b.base_bombs[k] as u64;
            }
            for k in 0..frontline_sim::FireMissionRefusal::COUNT {
                s6.refused[k] += b.refused[k] as u64;
            }
            s6.seen += b.seen as u64;
            s6.unseen += b.unseen as u64;
            s6.adjust += b.adjust as u64;
            s6.flight_ticks += b.flight_ticks;
            s6.upgrades += b.upgrades as u64;
            s6.downgrades += b.downgrades as u64;
            s6.no_solution += b.no_solution as u64;
            s6.ai.add(&p.s5b_ai);
            let a = &p.s6;
            let t = &mut f.s6;
            t.matches += 1;
            t.hq_built += u32::from(a.hq_built_at > 0);
            t.structures_built += a.structures_built as u64;
            t.structures_lost += a.structures_lost as u64;
            for k in 0..3 {
                t.crates_issued[k] += a.crates_issued[k] as u64;
            }
            t.crate_micro += a.crate_micro;
            t.crates_delivered += a.crates_delivered as u64;
            t.crates_lost += a.crates_lost as u64;
            t.stock_in_g += a.stock_in_g;
            t.stock_out_g += a.stock_out_g;
            t.stock_lost_g += a.stock_lost_g;
            t.stock_held_g += p.s6_stock_held_g;
            t.base_bombs += a.base_bombs as u64;
            t.structure_bombs += a.structure_bombs as u64;
            t.crate_bombs += a.crate_bombs as u64;
            t.unsourced_bombs += a.unsourced_bombs as u64;
            t.fwd_rounds += a.fwd_rounds as u64;
            t.fwd_items += a.fwd_items as u64;
            t.tank_rounds_fwd += a.tank_rounds_fwd as u64;
            t.tank_rare_fwd += a.tank_rare_fwd as u64;
            t.tank_crate_rounds += a.tank_crate_rounds as u64;
            t.rack_credit_mp += a.rack_credit_mp as u64;
            t.cookoffs += a.cookoffs as u64;
            t.tank_crate_runs += p.s6_tank_crate_runs as u64;
            t.crate_pickups += p.s6_crate_pickups as u64;
            // S7.
            let u = &mut f.s7;
            u.matches += 1;
            u.open_top_throws += p.s7.open_top_throws as u64;
            u.open_top_hits += p.s7.open_top_hits as u64;
            u.open_top_throw_orders += p.s7.open_top_throw_orders as u64;
            u.overwatch_holds += p.s7.overwatch_holds as u64;
            u.relocations += p.s7.relocations as u64;
            u.open_top_backoffs += p.s7.open_top_backoffs as u64;
            for (k, v) in &p.s7.kills_by_vehicle {
                *u.kills_by_vehicle.entry(k.clone()).or_default() += *v as u64;
            }
            for (k, v) in &p.s7.kos_by_vehicle {
                *u.kos_by_vehicle.entry(k.clone()).or_default() += *v as u64;
            }
            for (k, v) in &p.s7.card_cost {
                u.card_cost.insert(k.clone(), *v);
            }
            for (k, v) in &p.s7.vehicle_card {
                u.vehicle_card.insert(k.clone(), v.clone());
            }
            u.tank_td_vehicles
                .extend(p.s7.tank_td_vehicles.iter().cloned());
            u.gun_vehicles.extend(p.s7.gun_vehicles.iter().cloned());
            // S8a.
            f.s10.add(&p.s10);
            f.s10b.add(&p.s10.fort, p.s10.s10b_ordered);
            f.s10b_ai.add(&p.s10, r.tick_hz as u32);
            f.s10c.add(&p.s10c, &p.s10);
            f.s11a1.add(&p.s11a1, p.kills, p.s10.income_mp);
            let e = &mut f.s8a;
            let c = &p.s8a;
            e.matches += 1;
            e.rides += c.rides as u64;
            e.transported += c.transported as u64;
            e.ride_ticks += c.ride_ticks;
            e.ride_walk_ticks += c.ride_walk_ticks;
            e.riders_lost += c.riders_lost as u64;
            e.small_arms_pens += c.small_arms_pens as u64;
            e.dismounts_under_fire += c.dismounts_under_fire as u64;
            e.transport_trips += c.transport_trips as u64;
            e.transport_cards.extend(p.s8a_cards.iter().cloned());
            e.ai_loads += p.s8a_ai[0] as u64;
            e.ai_rides += p.s8a_ai[1] as u64;
            e.ai_unloads += p.s8a_ai[2] as u64;
            e.ai_unloads_fire += p.s8a_ai[3] as u64;
            e.ai_fails += p.s8a_ai[4] as u64;
            e.obj_rides += p.s8a_obj.0 as u64;
            e.obj_ride_ticks += p.s8a_obj.1;
            e.obj_walk_ticks += p.s8a_obj.2;
            e.obj_lost += p.s8a_obj.3 as u64;
            for (a, b) in e.ride.iter_mut().zip(p.s8b_ride) {
                *a += b;
            }
            for (a, b) in e.ai8b.iter_mut().zip(p.s8b_ai) {
                *a += b as u64;
            }
            e.truck_loaded_g += p.s6.truck_loaded_g.iter().map(|&g| g as u64).sum::<u64>();
            e.truck_tank_rounds += p.s6.truck_tank_rounds as u64;
            e.truck_men_rounds += p.s6.truck_men_rounds as u64;
            e.truck_crates_dropped += p.s6.truck_crates_dropped as u64;
            e.truck_lost_g += p.s6.truck_lost_g;
            e.cargo_detonations += p.s6.cargo_detonations as u64;
            for (a, b) in e.s9_ai.iter_mut().zip(p.s9_ai) {
                *a += b as u64;
            }
            e.gun_fired_matches += (p.s9.gun_rounds > 0) as u64;
            e.gun_armour_out += p.armour.gun_armour_out as u64;
            let (a, b) = (&mut e.s9, &p.s9);
            a.guns_bought += b.guns_bought;
            a.guns_emplaced += b.guns_emplaced;
            a.guns_in_cover += b.guns_in_cover;
            a.gun_rounds += b.gun_rounds;
            a.gun_ko_tanks += b.gun_ko_tanks;
            a.gun_crew_lost += b.gun_crew_lost;
            a.guns_neutral += b.guns_neutral;
            a.guns_captured += b.guns_captured;
            a.guns_recrewed += b.guns_recrewed;
            a.tows_lost += b.tows_lost;
            a.ai_unhitches += b.ai_unhitches;
            a.ai_ambush_rounds += b.ai_ambush_rounds;
            a.gun_ai_fails += b.gun_ai_fails;
            a.captured_gun_kills += b.captured_gun_kills;
            s4.pzf_hunts += q.pzf_hunts;
            s4.capture_orders += q.capture_orders;
            s4.pop_cap_holds += q.pop_cap_holds;
            for k in 0..3 {
                if let Some(v) = q.pop_at_min[k] {
                    s4.pop_sum[k] += v as u64;
                    s4.pop_n[k] += 1;
                }
            }
        }
    }
    let n = matches.len().max(1) as f64;
    s.minutes_mean = round3(minutes / n);
    s.minutes_min = if matches.is_empty() {
        0.0
    } else {
        round3(s.minutes_min)
    };
    s.minutes_max = round3(s.minutes_max);
    s.he_share_mean_pct = round3(he / n);
    s.factions = factions.into_values().collect();
    let views: Vec<crate::s3c_tally::MatchView> = matches
        .iter()
        .map(|m| {
            let r = &m.report;
            crate::s3c_tally::MatchView {
                factions: r.players.iter().map(|p| p.faction.as_str()).collect(),
                bases: r.players.iter().map(|p| p.base).collect(),
                s3c: r.players.iter().map(|p| &p.s3c).collect(),
                winner: r.winner,
                seconds: r.game_seconds,
            }
        })
        .collect();
    let names = matches
        .first()
        .map_or_else(Vec::new, |m| m.report.point_names.clone());
    s.s3c = crate::s3c_tally::summarise(&views, names);
    s
}

/// The `--summary` table (and a line per match).
pub fn many_text(r: &ManyReport) -> String {
    let s = &r.summary;
    let swapped = r.matches.iter().filter(|m| m.swapped).count();
    let mut out = format!(
        "skirmish summary: {} matches ({} with swapped bases), {} at once\n",
        s.matches, swapped, s.jobs
    );
    for l in &r.overrides {
        out += &format!("{l}\n");
    }
    out += "faction        wins  base0/1  tanks lost/bought  ko gun/rocket  \
            rockets fired/hit/pen (matches)  satchels@veh (matches)  kills/losses\n";
    for f in &s.factions {
        out += &format!(
            "{:<14} {:>4}  {:>3}/{:<3}  {:>8}/{:<8}  {:>6}/{:<6}  {:>7}/{}/{} ({}/{})  {:>12} ({}/{})  {}/{}\n",
            f.faction,
            f.wins,
            f.wins_by_base[0],
            f.wins_by_base[1],
            f.tanks_lost,
            f.tanks_bought,
            f.ko_by_gun,
            f.ko_by_rocket,
            f.rockets_fired,
            f.rocket_hits,
            f.rocket_pens,
            f.matches_with_rockets,
            s.matches,
            f.satchels_at_vehicles,
            f.matches_with_satchels,
            s.matches,
            f.kills,
            f.losses,
        );
    }
    let ko_total: u32 = s.ko_causes.values().sum();
    let rack = s.ko_causes.get("ko_ammo_rack").copied().unwrap_or(0);
    let causes: Vec<String> = s
        .ko_causes
        .iter()
        .map(|(k, v)| format!("{k} {v}"))
        .collect();
    out += &format!(
        "wins by base: base 0 {}, base 1 {}; no winner {}\n\
         length min: mean {:.1}, {:.1}-{:.1}, {} over 25\n\
         tank losses: {} ({}; rack {:.0} %)\n\
         HE share of kills: mean {:.1} %, max {:.1} %\n\
         tick p99 ms (max over matches, parallel runs): peak {:.3}, all {:.3}\n",
        s.wins_by_base[0],
        s.wins_by_base[1],
        s.no_winner,
        s.minutes_mean,
        s.minutes_min,
        s.minutes_max,
        s.over_25_min,
        ko_total,
        if causes.is_empty() {
            "none".into()
        } else {
            causes.join(", ")
        },
        if ko_total == 0 {
            0.0
        } else {
            rack as f64 * 100.0 / ko_total as f64
        },
        s.he_share_mean_pct,
        s.he_share_max_pct,
        s.peak_p99_max_ms,
        s.all_p99_max_ms,
    );
    // S3c: tank losses split, abandonments and what became of them.
    let join = |m: &BTreeMap<String, u32>| {
        if m.is_empty() {
            "none".to_string()
        } else {
            m.iter()
                .map(|(k, v)| format!("{k} {v}"))
                .collect::<Vec<_>>()
                .join(", ")
        }
    };
    for f in &s.factions {
        out += &format!(
            "S3c {}: tanks lost {} = destroyed {} + abandoned {} ({}); abandoned then: {}; \
             captured {} enemy; crew alive at bail-out {}, out {}, home {}\n",
            f.faction,
            f.tanks_lost,
            f.tanks_destroyed,
            f.tanks_abandoned,
            join(&f.abandon_causes),
            join(&f.abandoned_fates),
            f.captures,
            f.crew_bailed,
            f.crew_out,
            f.crew_home,
        );
    }
    for f in &s.factions {
        out += &format!("S3c AI {}: {}\n", f.faction, join(&f.ai_s3c));
    }
    // S4: cards bought, Panzerfausts, pickups, captures, population.
    for f in &s.factions {
        let q = &f.s4;
        let n = q.matches.max(1) as f64;
        let cards: Vec<String> = q
            .bought
            .iter()
            .map(|(k, v)| format!("{k} {}/{} ({v})", q.bought_matches[k], q.matches))
            .collect();
        let pop: Vec<String> = (0..3)
            .map(|k| {
                if q.pop_n[k] == 0 {
                    "-".into()
                } else {
                    format!("{:.0}", q.pop_sum[k] as f64 / q.pop_n[k] as f64)
                }
            })
            .collect();
        out += &format!(
            "S4 {}: cards bought (matches with one/matches, total): {}\n\
             S4 {}: panzerfausts fired {} (matches {}/{}, normal bases {}), hit {}, pen {}, \
             ko {}; pzf hunts {}; pickups {:.1}/match; capture orders {}, captures {}; \
             mean pop at 5/10/20 min {}; pop-cap holds {:.1}/match\n",
            f.faction,
            if cards.is_empty() {
                "none".into()
            } else {
                cards.join(", ")
            },
            f.faction,
            q.panzerfausts,
            q.matches_with_pzf,
            q.matches,
            q.matches_with_pzf_normal,
            q.pzf_hits,
            q.pzf_pens,
            q.ko_by_pzf,
            q.pzf_hunts,
            q.pickups as f64 / n,
            q.capture_orders,
            f.captures,
            pop.join("/"),
            q.pop_cap_holds as f64 / n,
        );
    }
    // S5a: rounds fired, per tank, manpower on ammunition, HE kills by
    // source.
    for f in &s.factions {
        let q = &f.s5a;
        let n = q.matches.max(1) as f64;
        let tanks = f.tanks_bought.max(1) as f64;
        let fired: Vec<String> = (0..fired_class::COUNT)
            .map(|k| format!("{} {}", fired_class::NAMES[k], q.fired[k]))
            .collect();
        let main = q.fired[fired_class::TANK_AP]
            + q.fired[fired_class::TANK_HE]
            + q.fired[fired_class::TANK_RARE];
        let mp = |k: usize| q.paid_micro[k] as f64 / 1e6;
        let all_kills: u64 = q.kills_by.iter().sum();
        let pct = |c: u16| {
            if all_kills == 0 {
                0.0
            } else {
                q.kills_by[c as usize] as f64 * 100.0 / all_kills as f64
            }
        };
        out += &format!(
            "S5a {}: rounds fired {}\n\
             S5a {}: per tank bought: main gun {:.1} (AP {:.1}, HE {:.1}, rare {:.1}), MG {:.0}\n\
             S5a {}: MP on ammo small arms {:.0}, explosives {:.0}, shells {:.0} (total {:.0}, \
             {:.1}/match); refilled {}/{}/{}; refill paused {} s\n\
             S5a {}: kills by source % (of this side's kills): mortar {:.1}, tank HE {:.1}, \
             frag {:.1}, satchel {:.1}, rocket {:.1}, tank AP {:.1}, MG {:.1}, rifle {:.1}, \
             smg {:.1}, vehicle MG {:.1}\n\
             S5a {}: AI refill trips {} ({:.1}/match, {} dry, {} stalled; matches with a trip {}), \
             mean squads away {:.2}; mortar reserve holds {}; rearm stalls {}; float holds {}\n\
             S5a {}: ambushes {} (tubes gone during one, fired or with a fallen carrier, {}; normal-base matches with one {}), \
             ends {}\n",
            f.faction,
            fired.join(", "),
            f.faction,
            main as f64 / tanks,
            q.fired[fired_class::TANK_AP] as f64 / tanks,
            q.fired[fired_class::TANK_HE] as f64 / tanks,
            q.fired[fired_class::TANK_RARE] as f64 / tanks,
            q.fired[fired_class::VEHICLE_MG] as f64 / tanks,
            f.faction,
            mp(0),
            mp(1),
            mp(2),
            mp(0) + mp(1) + mp(2),
            (mp(0) + mp(1) + mp(2)) / n,
            q.refilled[0],
            q.refilled[1],
            q.refilled[2],
            q.paused_s,
            f.faction,
            pct(kill_class::MORTAR),
            pct(kill_class::TANK_HE),
            pct(kill_class::FRAG),
            pct(kill_class::SATCHEL),
            pct(kill_class::ROCKET),
            pct(kill_class::TANK_AP),
            pct(kill_class::MG),
            pct(kill_class::RIFLE),
            pct(kill_class::SMG),
            pct(kill_class::VEHICLE_MG),
            f.faction,
            q.ai.refill_trips,
            q.ai.refill_trips as f64 / n,
            q.ai.refill_dry_trips,
            q.ai.refill_stalls,
            q.matches_with_refill,
            q.ai.refill_away as f64 / q.ai.plans.max(1) as f64,
            q.ai.mortar_reserve_holds,
            q.ai.rearm_stalls,
            q.ai.float_holds,
            f.faction,
            q.ai.ambushes,
            q.ai.ambush_shots,
            q.matches_ambush_fired_normal,
            (0..frontline_sim::ai::AMBUSH_ENDS)
                .map(|k| format!(
                    "{} {}",
                    frontline_sim::ai::AMBUSH_END_NAMES[k],
                    q.ai.ambush_ends[k]
                ))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    // S5b: fire missions by mode, refusals, miss, flight, the base battery.
    for f in &s.factions {
        let q = &f.s5b;
        let n = q.matches.max(1) as f64;
        let bombs: u64 = q.bombs.iter().sum();
        let missions: u64 = q.missions.iter().sum();
        let miss = |k: usize| q.miss_mm[k] as f64 / 1000.0 / q.bombs[k].max(1) as f64;
        let refused: Vec<String> = (0..frontline_sim::FireMissionRefusal::COUNT)
            .filter(|&k| q.refused[k] > 0)
            .map(|k| {
                format!(
                    "{} {}",
                    frontline_sim::FireMissionRefusal::NAMES[k],
                    q.refused[k]
                )
            })
            .collect();
        let base: u64 = q.base_bombs.iter().sum();
        out += &format!(
            "S5b {}: missions {} ({:.1}/match): direct {}, observed {}, map {} ({:.1} % observed+direct); refused {} [{}]\n\
             S5b {}: bombs {} ({:.1}/match): direct {}, observed {}, map {} ({:.1} % observed+direct by bomb); mean planned miss m: direct {:.1}, observed {:.1}, map {:.1}\n\
             S5b {}: adjusting rounds {}, bursts seen {} / unseen {}, upgrades {}, downgrades {}, no solution {}, mean flight {:.1} s\n\
             S5b {}: bombs from inside the own base {} ({:.1} % of bombs; direct {}, observed {}, map {})\n",
            f.faction,
            missions,
            missions as f64 / n,
            q.missions[0],
            q.missions[1],
            q.missions[2],
            (q.missions[0] + q.missions[1]) as f64 * 100.0 / missions.max(1) as f64,
            q.refused.iter().sum::<u64>(),
            refused.join(", "),
            f.faction,
            bombs,
            bombs as f64 / n,
            q.bombs[0],
            q.bombs[1],
            q.bombs[2],
            (q.bombs[0] + q.bombs[1]) as f64 * 100.0 / bombs.max(1) as f64,
            miss(0),
            miss(1),
            miss(2),
            f.faction,
            q.adjust,
            q.seen,
            q.unseen,
            q.upgrades,
            q.downgrades,
            q.no_solution,
            q.flight_ticks as f64 / 30.0 / bombs.max(1) as f64,
            f.faction,
            base,
            base as f64 * 100.0 / bombs.max(1) as f64,
            q.base_bombs[0],
            q.base_bombs[1],
            q.base_bombs[2],
        );
        let a = &q.ai;
        let am: u64 = a.missions.iter().sum();
        out += &format!(
            "S5b {} AI: missions {} sent of {} planned: direct {}, observed {}, map {} ({:.1} % observed+direct); spotters sent {} ({:.2}/match), missions through a spotter {}, time-outs {}, recalls {}; map fire held {} (reserve) / {} (walking); blind targets passed over {}; waits for the plan's ray budget {}; map fire held by its miss {}, dry orders withdrawn {}\n",
            f.faction,
            am.saturating_sub(a.dry_drops),
            am,
            a.missions[0],
            a.missions[1],
            a.missions[2],
            (a.missions[0] + a.missions[1]) as f64 * 100.0 / am.max(1) as f64,
            a.spotters,
            a.spotters as f64 / n,
            a.spotter_missions,
            a.spotter_timeouts,
            a.spotter_recalls,
            a.map_holds,
            a.walk_holds,
            a.blind_holds,
            a.ray_holds,
            a.miss_holds,
            a.dry_drops,
        );
        let g = &f.s8a;
        let gn = g.matches.max(1) as f64;
        out += &format!(
            "S9 {} guns: bought {:.2}/match, rounds {:.1}/match, fired in {:.0} % of matches; enemy armour put out of action by guns {:.2}/match ({} in all; wrecks only {:.3}/match)\n",
            f.faction,
            g.s9.guns_bought as f64 / gn,
            g.s9.gun_rounds as f64 / gn,
            g.gun_fired_matches as f64 * 100.0 / gn,
            g.gun_armour_out as f64 / gn,
            g.gun_armour_out,
            g.s9.gun_ko_tanks as f64 / gn,
        );
    }
    // S10a-ai (spec 5): one "fort:" line per faction.
    for f in &s.factions {
        let (_, d) = crate::json_out::s10_blocks(&f.s10);
        let v = |k: &str| d[k].as_f64().unwrap_or(0.0);
        out += &format!(
            "fort: {} works {:.2}/match (in {:.0} %), wire {:.1} sections/match (in {:.0} %), gaps in {:.0} %, breaches {:.2}/match (opened {:.0} %; when enemy wire met {} %), builders {:.1} %, materials {:.2} % of income, lost {:.1} kg/match, men lost in wire {:.2}/match\n",
            f.faction,
            v("works_per_match"),
            v("matches_with_works_pct"),
            v("wire_sections_per_match"),
            v("matches_with_wire_pct"),
            v("matches_with_gap_pct"),
            v("breaches_started_per_match"),
            v("breach_opened_pct"),
            d["breach_when_enemy_wire_pct"]
                .as_f64()
                .map_or("-".into(), |x| format!("{x:.0}")),
            v("builder_share_pct"),
            v("materials_mp_pct"),
            v("materials_lost_kg_per_match"),
            v("men_lost_in_wire_per_match"),
        );
    }
    // S10b: only when a mechanic was used (AI play keeps the old text).
    for f in &s.factions {
        let b = &f.s10b;
        if b.counters.iter().any(|v| *v > 0) {
            let parts: Vec<String> = S10B_COUNTERS
                .iter()
                .zip(b.counters)
                .map(|(k, v)| format!("{k} {v}"))
                .collect();
            out += &format!(
                "s10b: {} used {}: {}\n",
                f.faction,
                b.used(),
                parts.join(", ")
            );
        }
    }
    // S10b-ai (Build A step 8): one "s10b-ai:" line per faction.
    for f in &s.factions {
        let (_, d) = crate::json_out::s10b_ai_blocks(&f.s10b_ai);
        let v = |k: &str| d[k].as_f64().unwrap_or(0.0);
        let opt = |k: &str| d[k].as_f64().map_or("-".into(), |x| format!("{x:.0}"));
        out += &format!(
            "s10b-ai: {} mines in {:.0} % ({:.2} laid/match, AT fields {:.2}/match, chance {}), obstacles in {:.0} % (chance {}), bunker in {:.0} % of matches (ready {}; done when started {} % of {}), sweep when wanted {} % ({}), demolish when wanted {} % ({}), mistakes {}, materials {:.2} % of income, overdue {}, re-orders {}\n",
            f.faction,
            v("matches_with_mines_pct"),
            v("mines_laid_per_match"),
            v("at_fields_per_match"),
            d["lay_chance_matches"],
            v("matches_with_obstacle_pct"),
            d["obst_chance_matches"],
            v("matches_with_bunker_pct"),
            d["bunker_ready_matches"],
            opt("bunker_done_when_started_pct"),
            d["bunker_chance_matches"],
            opt("sweep_when_wanted_pct"),
            d["sweep_wanted_matches"],
            opt("demolish_when_wanted_pct"),
            d["demolish_wanted_matches"],
            d["own_side_mistakes"],
            v("s10b_materials_mp_pct"),
            d["jobs_overdue"],
            d["refused_reorders"],
        );
        // Build D: why no bunker started (plans) and how its jobs ended.
        let why = &f.s10b_ai.bunker_why;
        if why.iter().any(|&n| n > 0) {
            let parts: Vec<String> = frontline_sim::ai::BUNKER_WHY
                .iter()
                .zip(why)
                .filter(|&(_, &n)| n > 0)
                .map(|(k, n)| format!("{k} {n}"))
                .collect();
            out += &format!("s10b-ai bunker why: {} {}\n", f.faction, parts.join(", "));
        }
    }
    out += "seed  bases    winner         base  length  tanks lost p0/p1  rockets p0/p1  satchels p0/p1  HE %   hash\n";
    for m in &r.matches {
        let rp = &m.report;
        let p = |k: usize| &rp.players[k].armour;
        out += &format!(
            "{:<5} {:<8} {:<14} {:>4}  {:>6}  {:>8}/{:<8}  {:>6}/{:<6}  {:>7}/{:<6}  {:>5.1}  {}\n",
            m.seed,
            if m.swapped { "swapped" } else { "normal" },
            rp.winner_faction.as_deref().unwrap_or(rp.result),
            rp.armour.winner_base.map_or("-".into(), |b| b.to_string()),
            mmss(rp.game_seconds),
            p(0).vehicles_lost,
            p(1).vehicles_lost,
            p(0).at_shots,
            p(1).at_shots,
            p(0).satchels_at_vehicles,
            p(1).satchels_at_vehicles,
            rp.armour.he_share_pct,
            rp.final_hash,
        );
    }
    out += &crate::s3c_tally::text(&s.s3c);
    out
}

/// `--ab`: the same matches without (A) and with (B) the overrides.
#[derive(Clone, Debug, Serialize)]
pub struct AbReport {
    pub a: ManyReport,
    pub b: ManyReport,
}

/// Runs `--ab` (see [`SkirmishArgs::ab`]).
pub fn skirmish_ab(args: &SkirmishArgs) -> Result<AbReport> {
    let a = skirmish_many(&SkirmishArgs {
        rules_override: Vec::new(),
        ab: false,
        ..args.clone()
    })?;
    let b = skirmish_many(&SkirmishArgs {
        ab: false,
        ..args.clone()
    })?;
    Ok(AbReport { a, b })
}

/// Both summaries and the headline numbers side by side.
pub fn ab_text(r: &AbReport) -> String {
    let (a, b) = (&r.a.summary, &r.b.summary);
    let mut out = format!(
        "=== A: no override\n{}\n=== B: with the overrides\n{}\n",
        many_text(&r.a),
        many_text(&r.b)
    );
    out += "=== A -> B\n";
    let row =
        |name: &str, x: f64, y: f64| format!("{name:<34} {x:>9.1} -> {y:<9.1} ({:+.1})\n", y - x);
    let rack = |s: &ManySummary| {
        let t: u32 = s.ko_causes.values().sum();
        if t == 0 {
            0.0
        } else {
            s.ko_causes.get("ko_ammo_rack").copied().unwrap_or(0) as f64 * 100.0 / t as f64
        }
    };
    for fa in &a.factions {
        let fb = b
            .factions
            .iter()
            .find(|f| f.faction == fa.faction)
            .cloned()
            .unwrap_or_default();
        let f = &fa.faction;
        out += &row(&format!("{f} wins"), fa.wins as f64, fb.wins as f64);
        out += &row(
            &format!("{f} tanks lost"),
            fa.tanks_lost as f64,
            fb.tanks_lost as f64,
        );
        out += &row(
            &format!("{f} rockets fired"),
            fa.rockets_fired as f64,
            fb.rockets_fired as f64,
        );
        out += &row(
            &format!("{f} satchels at vehicles"),
            fa.satchels_at_vehicles as f64,
            fb.satchels_at_vehicles as f64,
        );
    }
    out += &row(
        "base 0 wins",
        a.wins_by_base[0] as f64,
        b.wins_by_base[0] as f64,
    );
    out += &row(
        "base 1 wins",
        a.wins_by_base[1] as f64,
        b.wins_by_base[1] as f64,
    );
    out += &row("no winner", a.no_winner as f64, b.no_winner as f64);
    out += &row("length mean (min)", a.minutes_mean, b.minutes_mean);
    out += &row("rack share of knock-outs (%)", rack(a), rack(b));
    out += &row(
        "HE share mean (%)",
        a.he_share_mean_pct,
        b.he_share_mean_pct,
    );
    out += &row("HE share max (%)", a.he_share_max_pct, b.he_share_max_pct);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tally() -> ArmourTally {
        ArmourTally {
            players: vec![PlayerArmour::default(); 2],
            ..ArmourTally::default()
        }
    }

    /// The trace and `gun_armour_out` agree: a vehicle bailed out, recrewed
    /// and lost again is one vehicle (`tools/gun_funnel.py`).
    #[test]
    fn a_vehicle_lost_twice_to_guns_counts_once() {
        let mut t = tally();
        t.count_gun_out(0, 25);
        t.count_gun_out(0, 25);
        assert_eq!(t.players[0].gun_armour_out, 1);
        t.count_gun_out(0, 26);
        assert_eq!(t.players[0].gun_armour_out, 2);
    }

    /// Taken over and lost to the other side's guns: once for each side.
    #[test]
    fn each_side_counts_a_vehicle_for_itself() {
        let mut t = tally();
        t.count_gun_out(0, 25);
        t.count_gun_out(1, 25);
        assert_eq!(t.players[0].gun_armour_out, 1);
        assert_eq!(t.players[1].gun_armour_out, 1);
    }

    /// S10c-ai Build A item 4: player 0 lays three AP mines; an enemy rifle
    /// squad stands `dx` m from them for one second; the observer reads.
    fn approach_scene(dx: i32) -> S10cPlayer {
        use frontline_sim::FxVec2;
        let mut setup = match_setup(&SkirmishArgs {
            seed: 1,
            ..SkirmishArgs::default()
        })
        .unwrap();
        setup.players.iter_mut().for_each(|p| p.is_ai = false);
        let ap = setup
            .forts
            .iter()
            .position(|f| {
                f.mine_item
                    .and_then(|i| setup.items[i as usize].mine.as_ref())
                    .is_some_and(|m| m.trigger == 0)
            })
            .expect("an AP minefield blueprint") as u16;
        let rifle = setup
            .squad_types
            .iter()
            .position(|t| t.id == "ww2:usa_rifle_squad")
            .unwrap() as u16;
        let mut w = World::new_match(setup, 1).unwrap();
        let usa = (0..2u8)
            .find(|&p| {
                let s = w.match_setup().unwrap();
                s.factions[s.players[p as usize].faction as usize].id == "ww2:usa"
            })
            .unwrap();
        let ger = 1 - usa;
        for k in 0..3 {
            w.lay_mine_now(ap, FxVec2::from_int(400 + 2 * k, 600), PlayerId(ger))
                .expect("mine laid");
        }
        w.spawn_squad_at(
            PlayerId(usa),
            rifle,
            FxVec2::from_int(402 + dx, 600),
            FxVec2::from_int(-1, 0),
        )
        .unwrap();
        let hz = w.match_setup().unwrap().tick_hz as u64;
        let mut seen = S10cSeen::default();
        for _ in 0..hz {
            w.step(&[]);
            if w.tick().is_multiple_of(hz) {
                seen.observe(&w);
            }
        }
        seen.finish();
        let r = seen.per[ger as usize].clone();
        assert_eq!(r.mines_seen, 3, "the three mines are seen");
        assert_eq!(seen.per[usa as usize].mines_approached, 0);
        r
    }

    /// An enemy rifle squad 20 m from an own AP field: its three mine ids
    /// count once each. Control: 60 m -> 0 (seen with the squad at 60 m).
    #[test]
    fn s10c_observer_counts_an_approach() {
        let near = approach_scene(20);
        assert_eq!(near.mines_approached, 3);
        assert_eq!(near.mines_home + near.mines_front, 3);
        let far = approach_scene(60);
        assert_eq!(far.mines_approached, 0, "control: 60 m away");
    }

    /// Fix pass 1 (finding A-5): who counts as an approach. Germany's three
    /// AP mines with its own rifle squad 20 m off (own: no count), two
    /// German AT mines with a US rifle squad 20 m off (soldiers do not
    /// approach an AT mine) and a US tank `tank_dx` m off them. Tank at
    /// 20 m: the two AT mine ids count, nothing else (2). Control: the tank
    /// at 60 m -> 0 (an own squad or a soldier at an AT mine counted would
    /// make it 3 or more).
    #[test]
    fn s10c_observer_counts_enemy_soldiers_and_vehicles_only() {
        use frontline_sim::FxVec2;
        let run = |tank_dx: i32| {
            let mut setup = match_setup(&SkirmishArgs {
                seed: 1,
                ..SkirmishArgs::default()
            })
            .unwrap();
            setup.players.iter_mut().for_each(|p| p.is_ai = false);
            let blueprint = |at: bool| {
                setup
                    .forts
                    .iter()
                    .position(|f| {
                        f.mine_item
                            .and_then(|i| setup.items[i as usize].mine.as_ref())
                            .is_some_and(|m| (m.trigger != 0) == at)
                    })
                    .expect("a minefield blueprint") as u16
            };
            let (ap, at) = (blueprint(false), blueprint(true));
            let ty = |id: &str| setup.squad_types.iter().position(|t| t.id == id).unwrap() as u16;
            let (us_rifle, ger_rifle, tank) = (
                ty("ww2:usa_rifle_squad"),
                ty("ww2:ger_grenadiergruppe"),
                ty("ww2:usa_m4a3_75_squad"),
            );
            let mut w = World::new_match(setup, 1).unwrap();
            let usa = (0..2u8)
                .find(|&p| {
                    let s = w.match_setup().unwrap();
                    s.factions[s.players[p as usize].faction as usize].id == "ww2:usa"
                })
                .unwrap();
            let ger = 1 - usa;
            for k in 0..3 {
                w.lay_mine_now(ap, FxVec2::from_int(400 + 2 * k, 600), PlayerId(ger))
                    .expect("AP mine laid");
            }
            for k in 0..2 {
                w.lay_mine_now(at, FxVec2::from_int(400 + 4 * k, 700), PlayerId(ger))
                    .expect("AT mine laid");
            }
            let west = FxVec2::from_int(-1, 0);
            let mut put = |p: u8, t: u16, x: i32, y: i32| {
                w.spawn_squad_at(PlayerId(p), t, FxVec2::from_int(x, y), west)
                    .unwrap();
            };
            put(ger, ger_rifle, 422, 600);
            put(usa, us_rifle, 424, 700);
            put(usa, tank, 402, 700 + tank_dx);
            let hz = w.match_setup().unwrap().tick_hz as u64;
            let mut seen = S10cSeen::default();
            for _ in 0..hz {
                w.step(&[]);
                if w.tick().is_multiple_of(hz) {
                    seen.observe(&w);
                }
            }
            seen.finish();
            seen.per[ger as usize].mines_approached
        };
        assert_eq!(run(20), 2, "the tank's two AT mines only");
        assert_eq!(run(60), 0, "control: the tank 60 m off");
    }

    /// S11a1 Build B (C6): every `s11a1` / `s11a1_derived` name the S11a1
    /// targets file reads is written by `json_out::s11a1_blocks`, and every
    /// `S11Counters` and S11a1 `AiCounters` field is in the raw block.
    /// Control: renaming `gun_observed_pct` in `s11a1_blocks` fails it.
    #[test]
    fn s11a1_json_names_match_the_targets_file() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/slices/targets/S11a1.toml");
        let f = crate::targets::parse(&fs::read_to_string(path).unwrap()).unwrap();
        let mut one = S11a1Faction::default();
        one.add(&S11a1Player::default(), 0, 0);
        let (raw, derived) = crate::json_out::s11a1_blocks(&one);
        let mut n = 0;
        for t in &f.targets {
            for m in [Some(&t.metric), t.exposure.as_ref(), t.control.as_ref()]
                .into_iter()
                .flatten()
            {
                let parts: Vec<&str> = m.split('.').collect();
                // Skirmish blocks only (the fortify rows are `fortify10.s11a1`).
                if parts[0].starts_with("fortify") {
                    continue;
                }
                let Some(k) = parts.iter().position(|p| p.starts_with("s11a1")) else {
                    continue;
                };
                let block = match parts[k] {
                    "s11a1" => &raw,
                    "s11a1_derived" => &derived,
                    other => panic!("{}: unknown block {other}", t.id),
                };
                let rest = parts[k + 1..].join(".");
                assert!(
                    crate::json_out::lookup(block, &rest).is_some(),
                    "{}: {m} is not written",
                    t.id
                );
                n += 1;
            }
        }
        assert!(n >= 40, "only {n} s11a1 paths in the targets file");
        let s11 =
            serde_json::to_value(frontline_sim::fire_mission::S11Counters::default()).unwrap();
        for k in s11.as_object().unwrap().keys() {
            assert!(raw.get(k).is_some(), "S11Counters::{k} is not in `s11a1`");
        }
        for k in S11A1_AI {
            assert!(raw.get(k).is_some(), "AiCounters::{k} is not in `s11a1`");
        }
        for k in ["refused_masked", "refused_not_ready"] {
            assert!(raw.get(k).is_some(), "{k}");
        }
    }

    /// Build A item 7: every `s10c` / `s10c_derived` name the S10c-ai
    /// targets file reads is written by `json_out::s10c_blocks`. Control:
    /// renaming `knew_enemy_wire_pct` in `s10c_blocks` fails it (seen).
    #[test]
    fn s10c_json_names_match_the_targets_file() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/slices/targets/S10c-ai.toml");
        let f = crate::targets::parse(&fs::read_to_string(path).unwrap()).unwrap();
        let mut one = S10cFaction::default();
        one.add(&S10cPlayer::default(), &S10Player::default());
        let (raw, derived) = crate::json_out::s10c_blocks(&one);
        let mut n = 0;
        for t in &f.targets {
            for m in [Some(&t.metric), t.exposure.as_ref(), t.control.as_ref()]
                .into_iter()
                .flatten()
            {
                let parts: Vec<&str> = m.split('.').collect();
                let Some(k) = parts.iter().position(|p| p.starts_with("s10c")) else {
                    continue;
                };
                let block = match parts[k] {
                    "s10c" => &raw,
                    "s10c_derived" => &derived,
                    other => panic!("{}: unknown block {other}", t.id),
                };
                let rest = parts[k + 1..].join(".");
                assert!(
                    crate::json_out::lookup(block, &rest).is_some(),
                    "{}: {m} is not written",
                    t.id
                );
                n += 1;
            }
        }
        assert!(n >= 40, "only {n} s10c paths in the targets file");
        // Fix pass 3: every `front_why` reason is written by name, the
        // thirteenth (`at_wait`) too. Control: `json_out::s10c_blocks`
        // writing only the first 12 names fails it (seen).
        assert_eq!(frontline_sim::ai::FRONT_WHY.len(), 13);
        assert_eq!(frontline_sim::ai::FRONT_WHY[12], "at_wait");
        for name in frontline_sim::ai::FRONT_WHY {
            assert!(
                crate::json_out::lookup(&raw, &format!("front_why_{name}")).is_some(),
                "front_why_{name} is not written"
            );
        }
    }
}
