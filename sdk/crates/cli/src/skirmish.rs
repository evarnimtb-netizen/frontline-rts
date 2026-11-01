//! `skirmish`: run a whole Kessel Ridge match headlessly, AI against AI, and
//! report the winner, the match length, per-player stats and tick timings
//! (S1 "Done when" 2 and 5).
//!
//! The match runs unpaced (as fast as the machine allows) unless `--paced`
//! is given. Tick times are measured around `World::step` and summarised
//! twice: over the whole match and over the "peak" ticks, those with at
//! least [`PEAK_SHARE_PCT`] % of the match's highest live soldier count.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use frontline_content::vehicle::AmmoMechanism;
use frontline_content::{Difficulty, MatchRequest, MatchSetup, PlayerRequest, SquadRole};
use frontline_sim::{
    ArmourEventKind, BlastKind, Outcome, PlayerId, SquadId, World, kill_class, vehicle_flags,
};
use serde::Serialize;

use crate::report::{Summary, hex, ns, round3};

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
            rules_override: Vec::new(),
            seeds: None,
            both_bases: false,
            summary: false,
            jobs: None,
            ab: false,
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
    /// Knock-outs of both players by cause (`ko_crew`, `ko_ammo_rack`,
    /// `ko_fire`; S3b baseline).
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
    pub hits_by_zone: BTreeMap<String, u32>,
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
}

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
}

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
                    .map(|&t| &setup.squad_types[t as usize])
                    .find(|t| t.role == SquadRole::Rifle)
                    .map_or(0, |t| t.cost / t.men.max(1) as u32)
            })
            .collect();
        ArmourTally {
            players: vec![PlayerArmour::default(); n],
            at_men_lost: vec![0; n],
            vehicle_cost,
            man_value,
            ..ArmourTally::default()
        }
    }

    fn observe(&mut self, w: &World) {
        let g = w.game().expect("match world");
        let s = g.setup();
        let snap = w.snapshot();
        for e in &snap.armour_events {
            let owner = e.owner as usize;
            if e.ammo != u16::MAX {
                self.last_ammo.insert(e.vehicle, e.ammo);
            }
            let k = e.kind;
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
                    _ => "ko_fire",
                };
                if let Some(p) = self.players.get_mut(owner) {
                    p.vehicles_lost += 1;
                    p.tank_mp_lost += cost;
                    *p.disabled_by.entry(cause.into()).or_default() += 1;
                }
                let rocket = self
                    .last_ammo
                    .get(&e.vehicle)
                    .and_then(|&a| s.ammo.get(a as usize))
                    .is_some_and(|a| a.mechanism == AmmoMechanism::Heat);
                if let Some(p) = self.players.get_mut(e.attacker as usize) {
                    if rocket {
                        p.ko_by_rocket += 1;
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
    let stats_at_start = world.stats().clone();
    note_squads(&world, &mut fielded);

    let start = Instant::now();
    let running = |w: &World| w.game().is_some_and(|g| g.outcome() == Outcome::Running);
    let (mut frag_bursts, mut satchel_bursts) = (0u64, 0u64);
    let smoke_at_start = world.snapshot().smoke.last().map_or(0, |c| c.id as u64 + 1);
    let mut smoke_seen = smoke_at_start;
    let throws_at_start = world.stats().throws;
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
        if args.trace_vehicles && world.tick().is_multiple_of(10 * hz) {
            trace_vehicles(&world);
        }
        maybe_save(&world, false)?;
    }
    let wall = start.elapsed();
    maybe_save(&world, true)?;

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
            let role_bought = |r: SquadRole| {
                fielded
                    .values()
                    .filter(|(o, t)| owned(o) && s.squad_types[*t as usize].role == r)
                    .count() as u32
            };
            armour.vehicles_bought = role_bought(SquadRole::Vehicle);
            armour.at_teams_bought = role_bought(SquadRole::AntiTank);
            armour.kills_by = names(&pl.kills_by);
            // Kills by own tank weapons and rockets, valued at the enemy
            // rifleman's share (the knock-outs were counted as they came).
            let man = (0..s.players.len())
                .filter(|&q| q != p)
                .map(|q| tally.man_value[q])
                .max()
                .unwrap_or(0);
            let k = |c: u8| pl.kills_by[c as usize];
            armour.tank_mp_killed +=
                man * (k(kill_class::TANK_AP) + k(kill_class::TANK_HE) + k(kill_class::VEHICLE_MG));
            armour.at_mp_killed += man * k(kill_class::ROCKET);
            let at_cost = s.factions[s.players[p].faction as usize]
                .roster
                .iter()
                .map(|&t| &s.squad_types[t as usize])
                .find(|t| t.role == SquadRole::AntiTank)
                .map_or(0, |t| t.cost / t.men.max(1) as u32);
            armour.at_mp_lost = tally.at_men_lost[p] * at_cost;
            armour.at_shots = pl.rockets;
            if let Some(a) = ai {
                let c = &a.counters;
                armour.escorts_started = c.escorts_started;
                armour.escorts_reached = c.escorts_reached;
                armour.escorts_released = c.escorts_released;
                armour.satchels_at_vehicles = c.satchels_at_vehicles;
                armour.repairs = c.repairs;
                armour.retreats = c.retreats;
                armour.faces = c.faces;
            }
            PlayerReport {
                player: p as u8,
                faction: faction(p),
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
            if k.starts_with("ko_") {
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
            "{} p{p} manpower {} pop {}/{} squads {squads} call-ins {:?} vehicles {}/{}",
            mmss(w.tick() as f64 / hz),
            ps.manpower,
            ps.pop,
            ps.pop_cap,
            ps.call_ins,
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

fn summarise(matches: &[ManyMatch], jobs: usize) -> ManySummary {
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
         knock-outs: {} ({}; rack {:.0} %)\n\
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
