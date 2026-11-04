//! `fortify-test`: shows that trenches work as cover (S1 "Done when" 3).
//!
//! Two staged experiments on Kessel Ridge, repeated over several seeds and
//! with the factions swapped, so neither the RNG nor a faction's stats
//! decides the result:
//!
//! * **Rifles.** A rifle squad in a finished trench against a rifle squad in
//!   the open 150 m away. They fight until one squad is gone or
//!   [`RIFLE_FIGHT_S`] pass. Pass: the trench side's losses are at most
//!   [`RIFLE_LIMIT_PCT`] % of the open side's.
//! * **Mortar.** A [`BARRAGE_ROUNDS`]-round fire mission from a mortar
//!   squad 250 m away on a rifle squad, once in a trench and once in the
//!   open. S5b: the mortar's own leader sees the target, so the mission is
//!   Direct: it lays, fires an adjusting round, waits for its burst and
//!   then fires for effect. Pass: losses in the trench are at most
//!   [`MORTAR_LIMIT_PCT`] % of those in the open. Reported beside it (no
//!   threshold): the same mission after ranging on a reference point 40 m
//!   off, so the fire for effect comes without a warning round.
//! * **Grenade assault** (S1b). A rifle squad walks from [`ASSAULT_START_M`]
//!   to [`ASSAULT_GAP_M`] in front of a dug-in rifle squad (variant
//!   "advance"), or starts there (variant "at 25 m"). Once its lead
//!   is within [`ASSAULT_THROW_M`] of the trench (or it has arrived), every
//!   second each man who can throw a frag at the trench section in front
//!   of him does, as a player pressing G for each would; the fight goes on
//!   for [`ASSAULT_S`]. The same assault without grenades is the rifle-only
//!   baseline. Reported, no threshold yet: the defenders should lose
//!   clearly more to grenades than to rifles alone.
//! * **Smoke** (S1b). A rifle squad (a file pointing at the MG) is placed
//!   [`SMOKE_GAP_M`] from where an MG team will stand; its leader throws
//!   smoke [`SMOKE_THROW_M`] toward it, and once the cloud is full the MG
//!   arrives and watches for [`SMOKE_WATCH_S`]. The same without smoke is
//!   the control. Pass: behind smoke the squad is never spotted, never
//!   fired on and loses nobody, while the control squad is spotted and
//!   fired on.
//!
//! * **Suppressed assault** (S3b, docs/slices/S3b-balance.md T10, T11). A
//!   rifle squad walks from [`SUPPRESS_START_M`] to [`ASSAULT_GAP_M`] in
//!   front of a dug-in rifle squad and throws frags as in the grenade
//!   assault; in the supported variant an MG team stands beside its start
//!   and suppresses the trench (`SuppressUnits` on a 30 m line) from
//!   [`SUPPRESS_LEAD_S`] before the squad moves off. The fight lasts
//!   [`SUPPRESS_S`]. Reported against the targets (defenders lose at least
//!   75 % and attackers at most 50 % with the MG; the attackers lose at least
//!   75 % without it), not a pass condition. S3b Fix (review M5): a third,
//!   report-only variant gives the trench an MG team of its own
//!   ([`DEF_MG_DY`] beside the rifle squad) against the supported assault,
//!   so equal forces can be compared; the report also splits T10 by the
//!   entrenched faction. S3c (S3b-41 (A), B4): a fourth, report-only
//!   variant adds a rifle squad suppressing the same line from 20 m on the
//!   other side of the start (the AI's second base of fire), lifting with
//!   the MG; T10 per attacking faction is reported with it too.
//!
//! Squads are staged with the sim's setup tools (`World::place_squad`,
//! `World::dig_trench_now`), not commands, and the AI is off for both
//! players. The barrage, moves and throws are real commands.

use std::path::PathBuf;

use anyhow::{Result, bail};
use frontline_content::MatchSetup;
use frontline_sim::{
    BlastKind, Command, CommandKind, Fx, FxVec2, ItemKind, PlayerId, SimId, SquadId, World,
};
use serde::Serialize;

use crate::skirmish::{SkirmishArgs, match_setup};

/// Longest rifle fight, in game seconds.
pub const RIFLE_FIGHT_S: u64 = 180;
/// Rifles pass when trench losses ≤ this share of open losses.
pub const RIFLE_LIMIT_PCT: u32 = 50;
/// Mortar passes when trench losses ≤ this share of open losses.
pub const MORTAR_LIMIT_PCT: u32 = 40;
pub const BARRAGE_ROUNDS: u16 = 10;
/// Suppressed assault (S3b): the attackers start this far from the trench
/// ...
pub const SUPPRESS_START_M: i32 = 150;
/// ... their MG starts suppressing this long before they move ...
pub const SUPPRESS_LEAD_S: u64 = 5;
/// ... and the fight lasts this long from the MG's order, game seconds.
pub const SUPPRESS_S: u64 = 150;
/// S3b Tune: the squad closes into the trench this long after its first
/// grenade wave at the latest (s); the MG lifts at the rules'
/// `ai.assault.lift_fire_m`.
pub const CLOSE_IN_S: u64 = 10;
/// S3b Fix (review M5): the defenders' own MG team stands in the trench
/// this far (m, +y) from the rifle squad's centre.
const DEF_MG_DY: i32 = 12;

/// Time given to a barrage (fire, flight and the last burst), game seconds.
/// S5b: 90 -> 180: the mission is Direct (the mortar's leader sees the
/// trench), so it lays (4 s), fires adjusting rounds one at a time (each
/// waits for its burst, 13-15 s of flight, plus 4 s) and then fires for
/// effect.
const BARRAGE_S: u64 = 180;
/// Grenade assault: the attackers start this far from the trench (the
/// "advance" variant; the other starts at [`ASSAULT_GAP_M`]) ...
pub const ASSAULT_START_M: i32 = 60;
/// ... walk to this distance and throw from there ...
pub const ASSAULT_GAP_M: i32 = 25;
/// ... and the fight lasts this long from the start, game seconds.
pub const ASSAULT_S: u64 = 90;
/// Throwing starts once the attackers' lead is this close to the trench.
pub const ASSAULT_THROW_M: i32 = 27;
/// Smoke variant: from [`ASSAULT_START_M`] the leader throws smoke this far
/// toward the trench (it stands about 32 m in front of it), and the squad
/// walks to this distance, inside the cloud.
pub const ASSAULT_SMOKE_THROW_M: i32 = 28;
pub const ASSAULT_SMOKE_GAP_M: i32 = 27;
/// Smoke: the MG stands this far from the squad ...
pub const SMOKE_GAP_M: i32 = 100;
/// ... the smoke is thrown this far toward it ...
pub const SMOKE_THROW_M: i32 = 28;
/// ... and the MG watches this long once the cloud is full, game seconds.
pub const SMOKE_WATCH_S: u64 = 20;
/// The MG's spot (x) in the smoke experiment. At `TRENCH_X` the set-up
/// MG barely fires at a visible squad 100 m east (its prone muzzle loses
/// the line over a small rise), which would make a poor control.
const SMOKE_MG_X: i32 = 450;
/// The control only counts as fired on with at least this many rounds.
/// S4: 5 (was 20): the MG card is one tripod gun with a 6 s setup (the
/// S1-S3c MG team fired three bipod guns), so a seen control draws 5-130
/// rounds; the hidden side must still draw none (S4-33).
const SMOKE_CONTROL_SHOTS: u32 = 5;

/// Where things stand: open ground east of the map centre with clear lines
/// of sight (the same spots the sim's combat tests use).
const TRENCH_X: i32 = 420;
const LINE_Y: i32 = 420;
const RIFLE_GAP_M: i32 = 150;
const MORTAR_GAP_M: i32 = 250;

/// Options for [`fortify_test`]; also the clap definition of the subcommand.
#[derive(Clone, Debug, clap::Args)]
pub struct FortifyArgs {
    /// Seeds `first-seed .. first-seed + seeds` are run.
    #[arg(long, value_name = "N", default_value_t = 5)]
    pub seeds: u32,
    #[arg(long, value_name = "S", default_value_t = 1)]
    pub first_seed: u64,
    /// Sim worker threads. Never changes results.
    #[arg(long, value_name = "T", default_value_t = 2)]
    pub threads: usize,
    /// Mod directory (default: `mods/ww2` here or in the source tree).
    #[arg(long, value_name = "DIR")]
    pub mods: Option<PathBuf>,
    /// Bare `--json`: print the report as JSON instead of text. With a
    /// file, `--json FILE`: print the text as usual and also write the
    /// checks and report rows as one JSON object (`docs/modding/tuning.md`).
    #[arg(long, value_name = "FILE", num_args = 0..=1)]
    pub json: Option<Option<PathBuf>>,
    /// Tuning override (S3b): a partial TOML file merged onto the rules and
    /// defs. Repeatable; `docs/modding/tuning.md`.
    #[arg(long = "rules-override", value_name = "FILE")]
    pub rules_override: Vec<PathBuf>,
}

impl Default for FortifyArgs {
    fn default() -> FortifyArgs {
        FortifyArgs {
            seeds: 5,
            first_seed: 1,
            threads: 2,
            mods: None,
            json: None,
            rules_override: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct FortifyReport {
    pub rifles: Experiment,
    pub mortar: Experiment,
    /// S5b: the same mission after ranging on a reference point 40 m off
    /// (report only: fire for effect without a warning round).
    pub mortar_ranged: Experiment,
    pub grenade: GrenadeExperiment,
    pub smoke: SmokeExperiment,
    /// S3b: the MG-supported and the unsupported assault from 150 m.
    pub suppress: SuppressExperiment,
    /// Rifles, mortar and smoke pass (the grenade assault has no threshold
    /// yet).
    pub pass: bool,
}

/// The grenade assault, with grenades and rifle-only, per start distance.
#[derive(Clone, Debug, Serialize)]
pub struct GrenadeExperiment {
    pub name: &'static str,
    pub runs: Vec<AssaultRun>,
    pub variants: Vec<AssaultVariant>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AssaultVariant {
    /// Where the attackers start, metres from the trench.
    pub start_m: i32,
    /// Smoke first (see [`AssaultRun::smoke`]).
    pub smoke: bool,
    pub grenades: AssaultTotals,
    pub rifle_only: AssaultTotals,
    /// Defender losses with grenades as a percentage of rifle-only (null
    /// when the defenders lost nobody to rifles alone).
    pub ratio_pct: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct AssaultTotals {
    pub defender_losses: u32,
    pub attacker_losses: u32,
    pub frags_thrown: u32,
    /// Frags that went off below the base surface (in the trench).
    pub frags_in_trench: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct AssaultRun {
    pub seed: u64,
    pub start_m: i32,
    /// Faction id of the dug-in squad.
    pub entrenched: String,
    pub grenades: bool,
    /// The leader screened the trench with smoke before the advance.
    pub smoke: bool,
    pub frags_thrown: u32,
    pub frags_in_trench: u32,
    pub defender_losses: u32,
    pub attacker_losses: u32,
}

/// S3b: the assault from 150 m with and without an MG suppressing.
#[derive(Clone, Debug, Serialize)]
pub struct SuppressExperiment {
    pub name: &'static str,
    pub runs: Vec<SuppressRun>,
    pub supported: SuppressTotals,
    pub unsupported: SuppressTotals,
    /// S3b Fix (review M5, report only): the supported assault against a
    /// trench that has an MG team of its own (equal forces).
    pub defended: SuppressTotals,
    /// T10: with the MG, defenders lose ≥ 75 % and attackers ≤ 50 %.
    pub supported_met: bool,
    /// T11: without it, the attackers lose ≥ 75 %.
    pub unsupported_met: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SuppressTotals {
    pub defender_losses: u32,
    pub defender_men: u32,
    pub attacker_losses: u32,
    pub attacker_men: u32,
    /// Defender man-ticks alive, ducked and pinned.
    pub defender_ticks: u64,
    pub defender_ducked: u64,
    pub defender_pinned: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SuppressRun {
    pub seed: u64,
    /// Faction id of the dug-in squad.
    pub entrenched: String,
    /// An MG team suppressed the trench.
    pub mg: bool,
    /// S3b Fix (review M5): the trench had its own MG team as well.
    pub def_mg: bool,
    /// S3c (S3b-41 (A), B4): a rifle squad suppressed the trench too, as
    /// the AI's second base of fire (from 20 m on the other side of the
    /// start), lifting with the MG.
    #[serde(default)]
    pub second: bool,
    pub defender_losses: u32,
    pub attacker_losses: u32,
    pub attacker_men: u32,
    pub defender_men: u32,
    /// Share of defender man-time ducked and pinned (%).
    pub ducked_pct: f64,
    pub pinned_pct: f64,
    pub frags_thrown: u32,
    #[serde(skip)]
    pub ticks: (u64, u64, u64),
}

/// The smoke screen against an MG, with the no-smoke control.
#[derive(Clone, Debug, Serialize)]
pub struct SmokeExperiment {
    pub name: &'static str,
    pub runs: Vec<SmokeRun>,
    pub pass: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SmokeRun {
    pub seed: u64,
    /// Faction id of the squad behind the smoke (or not).
    pub hidden: String,
    pub smoke: bool,
    /// Ticks in which the MG's side saw one of the squad's men.
    pub seen_ticks: u32,
    /// Rounds the MG's side fired.
    pub mg_shots: u32,
    pub losses: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct Experiment {
    pub name: &'static str,
    pub runs: Vec<Run>,
    pub trench_losses: u32,
    pub open_losses: u32,
    /// Trench losses as a percentage of open losses (null when the open side
    /// lost nobody).
    pub ratio_pct: Option<f64>,
    pub limit_pct: u32,
    pub pass: bool,
}

/// One staged fight. In the rifle experiment both sides are in the same run;
/// in the mortar experiment `trench` says where the target squad stood.
#[derive(Clone, Debug, Serialize)]
pub struct Run {
    pub seed: u64,
    /// Faction id of the squad in (or, for the mortar, under fire in) the
    /// trench.
    pub entrenched: String,
    /// Mortar runs: was the target in the trench?
    pub trench: Option<bool>,
    pub trench_losses: u32,
    pub open_losses: u32,
    pub seconds: f64,
}

impl Experiment {
    fn new(name: &'static str, runs: Vec<Run>, limit_pct: u32) -> Experiment {
        let trench: u32 = runs.iter().map(|r| r.trench_losses).sum();
        let open: u32 = runs.iter().map(|r| r.open_losses).sum();
        Experiment {
            name,
            ratio_pct: (open > 0).then(|| (trench as f64 * 1000.0 / open as f64).round() / 10.0),
            pass: open > 0 && trench * 100 <= open * limit_pct,
            trench_losses: trench,
            open_losses: open,
            limit_pct,
            runs,
        }
    }
}

const EAST: FxVec2 = FxVec2::new(Fx::ONE, Fx::ZERO);
const WEST: FxVec2 = FxVec2::new(Fx::NEG_ONE, Fx::ZERO);
const NORTH: FxVec2 = FxVec2::new(Fx::ZERO, Fx::ONE);

fn at(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

/// A finished 32 m trench across the line of fire, parapet to the east.
fn dig(w: &mut World) {
    w.dig_trench_now(&[at(TRENCH_X, LINE_Y - 16), at(TRENCH_X, LINE_Y + 16)], -1);
}

/// Player `p`'s first squad with the given role name in its id.
/// Player `p`'s first squad whose card carries tag `role` (S4: by tag,
/// `line`, `mg`, ...).
fn squad(w: &World, p: u8, role: &str) -> Result<SquadId> {
    let g = w.game().expect("match world");
    let s = g.setup();
    g.squads()
        .iter()
        .find(|(_, q)| q.owner == PlayerId(p) && s.type_has_tag(q.type_idx, role))
        .map(|(id, _)| *id)
        .ok_or_else(|| anyhow::anyhow!("player {p} has no {role} squad"))
}

/// Buys player `p`'s first roster card with tag `role` and steps until it
/// arrives.
fn call_in(w: &mut World, p: u8, role: &str) -> Result<SquadId> {
    let s = w.match_setup().expect("match world");
    let faction = &s.factions[s.players[p as usize].faction as usize];
    let Some(&ty) = faction.roster.iter().find(|&&t| s.type_has_tag(t, role)) else {
        bail!("{} has no {role} in its roster", faction.id);
    };
    let known: Vec<SquadId> = w.game().unwrap().squads().keys().copied().collect();
    w.step(&[Command {
        player: PlayerId(p),
        seq: 0,
        kind: CommandKind::CallIn { squad_type: ty },
    }]);
    for _ in 0..30 * 120 {
        if let Some(id) = w
            .game()
            .unwrap()
            .squads()
            .keys()
            .find(|id| !known.contains(id))
        {
            return Ok(*id);
        }
        w.step(&[]);
    }
    bail!("the {role} call-in never arrived")
}

fn losses(w: &World, p: u8) -> u32 {
    w.game().unwrap().players()[p as usize].losses
}

fn faction(setup: &MatchSetup, p: u8) -> String {
    setup.factions[setup.players[p as usize].faction as usize]
        .id
        .clone()
}

/// A hand-driven match world (both players human, plenty of manpower).
fn world(args: &FortifyArgs, seed: u64) -> Result<World> {
    let mut setup = match_setup(&SkirmishArgs {
        seed,
        mods: args.mods.clone(),
        rules_override: args.rules_override.clone(),
        ..SkirmishArgs::default()
    })?;
    setup.players.iter_mut().for_each(|p| p.is_ai = false);
    setup.rules.start_manpower = 10_000;
    Ok(World::new_match(setup, args.threads)?)
}

fn rifle_run(args: &FortifyArgs, seed: u64, dug: u8) -> Result<Run> {
    let mut w = world(args, seed)?;
    let open = 1 - dug;
    let a = squad(&w, dug, "line")?;
    let b = squad(&w, open, "line")?;
    dig(&mut w);
    w.place_squad(a, at(TRENCH_X, LINE_Y), EAST);
    w.place_squad(b, at(TRENCH_X + RIFLE_GAP_M, LINE_Y), WEST);
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let start = w.tick();
    let alive = |w: &World, q| w.game().unwrap().squad(q).is_some();
    while alive(&w, a) && alive(&w, b) && w.tick() < start + RIFLE_FIGHT_S * hz {
        w.step(&[]);
    }
    Ok(Run {
        seed,
        entrenched: faction(w.match_setup().unwrap(), dug),
        trench: None,
        trench_losses: losses(&w, dug),
        open_losses: losses(&w, open),
        seconds: (w.tick() - start) as f64 / hz as f64,
    })
}

/// Barrage by `1 - target`'s mortar on `target`'s rifle squad, which stands
/// in a trench or not. Returns the target's losses and the time taken.
///
/// S5b: the mission is Direct (the mortar's leader sees the target). With
/// `ranged` the mortar first ranges in on a reference point
/// [`REFERENCE_M`] from the target (a 2-round mission, out of the bursts'
/// reach), so the barrage starts from the remembered aim and fires for
/// effect at once (the experiment's pass rule: trench protection under
/// fire for effect). Without it the barrage adjusts on the target itself:
/// its single adjusting rounds warn the squad, which drops (report only).
fn mortar_run(
    args: &FortifyArgs,
    seed: u64,
    target: u8,
    trench: bool,
    ranged: bool,
) -> Result<(u32, f64)> {
    let mut w = world(args, seed)?;
    let gunner = 1 - target;
    let rifle = squad(&w, target, "line")?;
    if trench {
        dig(&mut w);
    }
    w.place_squad(rifle, at(TRENCH_X, LINE_Y), EAST);
    let m = call_in(&mut w, gunner, "mortar")?;
    place_observing(&mut w, m, at(TRENCH_X, LINE_Y), MORTAR_GAP_M)?;
    let hz = w.match_setup().unwrap().tick_hz as u64;
    if ranged {
        range_in(&mut w, seed, gunner, m, at(TRENCH_X, LINE_Y))?;
    }
    let start = w.tick();
    let before = losses(&w, target);
    let shells = w.stats().explosions;
    w.step(&[Command {
        player: PlayerId(gunner),
        seq: 1,
        kind: CommandKind::FireMission {
            squad: m,
            target: at(TRENCH_X, LINE_Y),
            rounds: BARRAGE_ROUNDS,
        },
    }]);
    if let Some(r) = w.snapshot().fire_mission_refusals.first() {
        bail!(
            "seed {seed}: the fire mission was refused: {}",
            r.refusal.text()
        );
    }
    while w.tick() < start + BARRAGE_S * hz {
        w.step(&[]);
    }
    let fired = w.stats().explosions - shells;
    if fired != BARRAGE_ROUNDS as u64 {
        bail!("seed {seed}: the barrage burst {fired} of {BARRAGE_ROUNDS} shells");
    }
    Ok((
        losses(&w, target) - before,
        (w.tick() - start) as f64 / hz as f64,
    ))
}

/// S5b: the ranging mission's reference point lies this far from the
/// target (m): beyond the bursts' fragment reach (15 m) and suppression
/// radius, inside the ranging memory's 50 m cell.
const REFERENCE_M: i32 = 40;

/// S5b: mortar squad `m` (player `p`) ranges in on a point [`REFERENCE_M`]
/// from `target` that its leader sees (2 rounds), and waits for the bursts.
fn range_in(w: &mut World, seed: u64, p: u8, m: SquadId, target: FxVec2) -> Result<()> {
    use frontline_sim::fire_mission::mode;
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let r = REFERENCE_M;
    let refs = [at(0, r), at(0, -r), at(-r, 0), at(r, 0)];
    let Some(point) = refs.iter().map(|&d| target + d).find(|&pt| {
        let g = w.game().expect("match world");
        let sq = g.squad(m).expect("live");
        g.mortar_gunner(w.entities(), sq).is_some_and(|gn| {
            let gp = w.entities().transform()[gn.index()].pos.xy();
            g.mode_for(w.entities(), w.terrain(), m, gp, pt, w.tick()).0 == mode::DIRECT
        })
    }) else {
        bail!("seed {seed}: no reference point near the target is in the mortar leader's view");
    };
    w.step(&[Command {
        player: PlayerId(p),
        seq: 0,
        kind: CommandKind::FireMission {
            squad: m,
            target: point,
            rounds: 2,
        },
    }]);
    if let Some(r) = w.snapshot().fire_mission_refusals.first() {
        bail!(
            "seed {seed}: the ranging mission was refused: {}",
            r.refusal.text()
        );
    }
    let t0 = w.tick();
    let busy = |w: &World| {
        w.game()
            .unwrap()
            .squad(m)
            .is_some_and(|s| s.order.kind() == frontline_sim::game::OrderKind::FireMission)
    };
    while busy(w) {
        if w.tick() > t0 + 120 * hz {
            bail!("seed {seed}: the ranging mission never ended");
        }
        w.step(&[]);
    }
    // The last round's flight.
    for _ in 0..25 * hz {
        w.step(&[]);
    }
    let ranged = w
        .game()
        .unwrap()
        .squad(m)
        .is_some_and(|s| !s.ranging.is_empty());
    if !ranged {
        bail!("seed {seed}: the ranging mission left no ranging memory");
    }
    Ok(())
}

/// S5b: places mortar squad `m` `gap` m from `target`, east of it or at
/// the nearest bearing (10 degree steps, alternating north and south) from
/// which its own leader sees the target, so the mission is Direct (the
/// map's ground hides the trench from due east at 250 m).
pub(crate) fn place_observing(w: &mut World, m: SquadId, target: FxVec2, gap: i32) -> Result<()> {
    use frontline_sim::fire_mission::mode;
    for k in 0..36i32 {
        let deg = if k % 2 == 0 {
            k / 2 * 10
        } else {
            -(k / 2 + 1) * 10
        };
        let a = Fx::from_ratio(deg as i64 * 314_159, 180 * 100_000);
        let (sn, cs) = a.sin_cos();
        let spot = target + FxVec2::new(cs, sn) * Fx::from_int(gap);
        if !w.terrain().contains(spot) {
            continue;
        }
        let facing = (target - spot).normalize_or_zero();
        w.place_squad(m, spot, facing);
        let g = w.game().expect("match world");
        let sq = g.squad(m).expect("live");
        let Some(gunner) = g.mortar_gunner(w.entities(), sq) else {
            bail!("the mortar squad has no gunner");
        };
        let gp = w.entities().transform()[gunner.index()].pos.xy();
        let (md, _) = g.mode_for(w.entities(), w.terrain(), m, gp, target, w.tick());
        if md == mode::DIRECT || g.setup().rules.fire_mission.is_none() {
            return Ok(());
        }
    }
    bail!("no spot {gap} m from the target lets the mortar's leader see it")
}

fn members(w: &World, sq: SquadId) -> Vec<SimId> {
    w.game()
        .unwrap()
        .squad(sq)
        .map_or_else(Vec::new, |s| s.members.clone())
}

fn pos(w: &World, m: SimId) -> FxVec2 {
    w.entities().transform()[m.index()].pos.xy()
}

/// Steps once with `kinds` as player `p`'s commands.
fn step_with(w: &mut World, p: u8, kinds: Vec<CommandKind>) {
    let seq = w.tick() as u32 * 16;
    let cmds: Vec<Command> = kinds
        .into_iter()
        .enumerate()
        .map(|(k, kind)| Command {
            player: PlayerId(p),
            seq: seq + k as u32,
            kind,
        })
        .collect();
    w.step(&cmds);
}

/// Every attacker who still has a frag and is free to throw throws it at
/// the trench section in front of him (refused commands are not counted).
fn throw_wave(w: &mut World, p: u8, attackers: SquadId) {
    let kinds: Vec<CommandKind> = members(w, attackers)
        .into_iter()
        .filter(|&m| {
            let k = &w.entities().kit()[m.index()];
            k.items[ItemKind::Frag.index()] > 0
                && k.windup.is_none()
                && !w.entities().combat()[m.index()].pinned
        })
        .map(|m| {
            let y = pos(w, m)
                .y
                .clamp(Fx::from_int(LINE_Y - 15), Fx::from_int(LINE_Y + 15));
            CommandKind::Throw {
                units: vec![m],
                item: ItemKind::Frag,
                target: FxVec2::new(Fx::from_int(TRENCH_X), y),
            }
        })
        .collect();
    step_with(w, p, kinds);
}

/// One grenade assault on `dug`'s entrenched rifle squad (see the module
/// docs), with or without grenades.
fn assault_run(
    args: &FortifyArgs,
    seed: u64,
    dug: u8,
    start_m: i32,
    grenades: bool,
    smoke: bool,
) -> Result<AssaultRun> {
    let mut w = world(args, seed)?;
    let att = 1 - dug;
    let a = squad(&w, dug, "line")?;
    let b = squad(&w, att, "line")?;
    dig(&mut w);
    w.place_squad(a, at(TRENCH_X, LINE_Y), EAST);
    w.place_squad(b, at(TRENCH_X + start_m, LINE_Y), WEST);
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let start = w.tick();
    let (lost_d, lost_a) = (losses(&w, dug), losses(&w, att));
    let throws = w.stats().throws;
    let mut goal = at(TRENCH_X + ASSAULT_GAP_M, LINE_Y);
    if smoke {
        // The leader screens the trench first; the squad walks into the
        // cloud once it stands and throws from inside it.
        let leader = w.game().unwrap().squad(b).unwrap().lead(w.entities());
        let target = pos(&w, leader) - at(ASSAULT_SMOKE_THROW_M, 0);
        goal = FxVec2::new(Fx::from_int(TRENCH_X + ASSAULT_SMOKE_GAP_M), target.y);
        step_with(
            &mut w,
            att,
            vec![CommandKind::Throw {
                units: vec![leader],
                item: ItemKind::Smoke,
                target,
            }],
        );
        while w.snapshot().smoke.first().is_none_or(|c| c.radius < 7.99) {
            if w.tick() > start + 15 * hz {
                bail!("seed {seed}: the assault's smoke never came up");
            }
            w.step(&[]);
        }
    }
    step_with(
        &mut w,
        att,
        vec![CommandKind::Move {
            squad: b,
            target: goal,
        }],
    );
    let base = w.terrain().base_height_at(at(TRENCH_X, LINE_Y));
    let mut in_trench = 0;
    let throw_x = Fx::from_int(TRENCH_X + ASSAULT_THROW_M);
    while w.tick() < start + ASSAULT_S * hz {
        let close = w.game().unwrap().squad(b).is_some_and(|s| {
            let lead = pos(&w, s.lead(w.entities()));
            s.nav.phase == frontline_sim::NavPhase::Holding || lead.x <= throw_x
        });
        if grenades && close && (w.tick() - start).is_multiple_of(hz) {
            throw_wave(&mut w, att, b);
        } else {
            w.step(&[]);
        }
        in_trench += trench_bursts(&w, base);
    }
    Ok(AssaultRun {
        seed,
        start_m,
        entrenched: faction(w.match_setup().unwrap(), dug),
        grenades,
        smoke,
        frags_thrown: (w.stats().throws - throws - smoke as u64) as u32,
        frags_in_trench: in_trench,
        defender_losses: losses(&w, dug) - lost_d,
        attacker_losses: losses(&w, att) - lost_a,
    })
}

/// One suppressed assault on `dug`'s entrenched rifle squad (see the
/// module docs), with or without the MG; with `def_mg` (S3b Fix, review
/// M5) the trench also holds an MG team of its own.
fn suppress_run(
    args: &FortifyArgs,
    seed: u64,
    dug: u8,
    mg: bool,
    def_mg: bool,
    second: bool,
) -> Result<SuppressRun> {
    let mut w = world(args, seed)?;
    let att = 1 - dug;
    let a = squad(&w, dug, "line")?;
    let b = squad(&w, att, "line")?;
    dig(&mut w);
    w.place_squad(a, at(TRENCH_X, LINE_Y), EAST);
    w.place_squad(b, at(TRENCH_X + SUPPRESS_START_M, LINE_Y), WEST);
    let mut defenders = vec![a];
    if def_mg {
        let d = call_in(&mut w, dug, "mg")?;
        w.place_squad(d, at(TRENCH_X, LINE_Y + DEF_MG_DY), EAST);
        defenders.push(d);
    }
    let def_members =
        |w: &World| -> Vec<SimId> { defenders.iter().flat_map(|&d| members(w, d)).collect() };
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let def_men = def_members(&w).len() as u32;
    let mut mg_squad = None;
    let mut second_squad = None;
    if second {
        let r = call_in(&mut w, att, "line")?;
        second_squad = Some(r);
        w.place_squad(r, at(TRENCH_X + SUPPRESS_START_M, LINE_Y + 20), WEST);
        let units = members(&w, r);
        step_with(
            &mut w,
            att,
            vec![CommandKind::SuppressUnits {
                units,
                line: vec![at(TRENCH_X, LINE_Y - 15), at(TRENCH_X, LINE_Y + 15)],
            }],
        );
    }
    if mg {
        let m = call_in(&mut w, att, "mg")?;
        mg_squad = Some(m);
        w.place_squad(m, at(TRENCH_X + SUPPRESS_START_M, LINE_Y - 20), WEST);
        let units = members(&w, m);
        step_with(
            &mut w,
            att,
            vec![CommandKind::SuppressUnits {
                units,
                line: vec![at(TRENCH_X, LINE_Y - 15), at(TRENCH_X, LINE_Y + 15)],
            }],
        );
    }
    // S4 review: every attacking squad's men, since `attacker_losses`
    // counts every dead attacker (the MG and second squads too).
    let att_men: u32 = [Some(b), mg_squad, second_squad]
        .into_iter()
        .flatten()
        .map(|q| members(&w, q).len() as u32)
        .sum();
    let start = w.tick();
    let (lost_d, lost_a) = (losses(&w, dug), losses(&w, att));
    let throws = w.stats().throws;
    while w.tick() < start + SUPPRESS_LEAD_S * hz {
        w.step(&[]);
    }
    // The squad advances crouched (a player's choice under fire; both
    // variants alike).
    let men = members(&w, b);
    step_with(
        &mut w,
        att,
        vec![
            CommandKind::SetStance {
                units: men,
                stance: frontline_sim::StanceOrder::Crouch,
            },
            CommandKind::Move {
                squad: b,
                target: at(TRENCH_X + ASSAULT_GAP_M, LINE_Y),
            },
        ],
    );
    let throw_x = Fx::from_int(TRENCH_X + ASSAULT_THROW_M);
    let lift_x =
        Fx::from_int(TRENCH_X + w.match_setup().unwrap().rules.ai.assault.lift_fire_m as i32);
    let mut ticks = (0u64, 0u64, 0u64);
    // S3b Tune: the assault closes into the trench once its grenades are
    // out (or CLOSE_IN_S after the first wave), and the MG lifts its fire
    // when the squad's lead is within the AI's `ai.assault.lift_fire_m` of
    // the trench: fire and move, then grenades, then in.
    let (mut first_wave, mut closing, mut lifted) = (None, false, false);
    while w.tick() < start + SUPPRESS_S * hz {
        let lead_x = w
            .game()
            .unwrap()
            .squad(b)
            .map(|s| (s.nav.phase, pos(&w, s.lead(w.entities())).x));
        let close = lead_x
            .is_some_and(|(phase, x)| phase == frontline_sim::NavPhase::Holding || x <= throw_x);
        let frags_left = members(&w, b)
            .iter()
            .any(|m| w.entities().kit()[m.index()].items[ItemKind::Frag.index()] > 0);
        if close && first_wave.is_none() {
            first_wave = Some(w.tick());
        }
        if !closing && first_wave.is_some_and(|t| !frags_left || w.tick() >= t + CLOSE_IN_S * hz) {
            closing = true;
            step_with(
                &mut w,
                att,
                vec![CommandKind::Move {
                    squad: b,
                    target: at(TRENCH_X, LINE_Y),
                }],
            );
        } else if let Some(m) = mg_squad
            && !lifted
            && lead_x.is_some_and(|(_, x)| x <= lift_x)
        {
            lifted = true;
            let mut stops = vec![CommandKind::Stop { squad: m }];
            if let Some(r) = second_squad {
                stops.push(CommandKind::Stop { squad: r });
            }
            step_with(&mut w, att, stops);
        } else if close && !closing && (w.tick() - start).is_multiple_of(hz) {
            throw_wave(&mut w, att, b);
        } else {
            w.step(&[]);
        }
        for m in def_members(&w) {
            let c = &w.entities().combat()[m.index()];
            ticks.0 += 1;
            ticks.1 += c.ducked as u64;
            ticks.2 += c.pinned as u64;
        }
    }
    let share = |n: u64| (n as f64 * 1000.0 / ticks.0.max(1) as f64).round() / 10.0;
    Ok(SuppressRun {
        seed,
        entrenched: faction(w.match_setup().unwrap(), dug),
        mg,
        def_mg,
        defender_losses: losses(&w, dug) - lost_d,
        attacker_losses: losses(&w, att) - lost_a,
        attacker_men: att_men,
        second,
        defender_men: def_men,
        ducked_pct: share(ticks.1),
        pinned_pct: share(ticks.2),
        frags_thrown: (w.stats().throws - throws) as u32,
        ticks,
    })
}

/// Frag bursts of the last tick below the base surface `base`.
fn trench_bursts(w: &World, base: Fx) -> u32 {
    w.snapshot()
        .explosions
        .iter()
        .filter(|e| e.kind == BlastKind::Frag && Fx::from_milli((e.pos[2] * 1000.0) as i32) < base)
        .count() as u32
}

/// The smoke experiment for `hidden`'s rifle squad (see the module docs).
fn smoke_run(args: &FortifyArgs, seed: u64, hidden: u8, smoke: bool) -> Result<SmokeRun> {
    let mut w = world(args, seed)?;
    let gunner = 1 - hidden;
    // Everyone stays out of the way, each side in its own far corner.
    let all: Vec<(SquadId, PlayerId)> = w
        .game()
        .unwrap()
        .squads()
        .iter()
        .map(|(id, q)| (*id, q.owner))
        .collect();
    for (k, (sq, owner)) in all.into_iter().enumerate() {
        let k = 25 * k as i32;
        let spot = if owner.0 == 0 {
            at(80 + k, 960)
        } else {
            at(960, 80 + k)
        };
        w.place_squad(sq, spot, EAST);
    }
    let mg = call_in(&mut w, gunner, "mg")?;
    let park = if gunner == 0 {
        at(80, 900)
    } else {
        at(900, 80)
    };
    w.place_squad(mg, park, EAST);
    let rifle = squad(&w, hidden, "line")?;
    let spot = at(SMOKE_MG_X + SMOKE_GAP_M, LINE_Y);
    w.place_squad(rifle, spot, NORTH);
    let hz = w.match_setup().unwrap().tick_hz as u64;
    if smoke {
        let leader = members(&w, rifle)[0];
        let target = pos(&w, leader) - at(SMOKE_THROW_M, 0);
        step_with(
            &mut w,
            hidden,
            vec![CommandKind::Throw {
                units: vec![leader],
                item: ItemKind::Smoke,
                target: FxVec2::new(target.x, Fx::from_int(LINE_Y)),
            }],
        );
        let full = |w: &World| {
            w.snapshot()
                .smoke
                .first()
                .is_some_and(|c| c.density >= 1.0 && c.radius >= 7.99)
        };
        let t0 = w.tick();
        while !full(&w) {
            if w.tick() > t0 + 15 * hz {
                bail!("seed {seed}: the smoke cloud never came up");
            }
            w.step(&[]);
        }
    }
    w.place_squad(mg, at(SMOKE_MG_X, LINE_Y), EAST);
    let men = members(&w, rifle);
    let before = losses(&w, hidden);
    let (mut seen, mut shots) = (0, 0);
    for _ in 0..SMOKE_WATCH_S * hz {
        w.step(&[]);
        let s = w.snapshot();
        seen += s
            .soldier_ids
            .iter()
            .zip(&s.soldier_visible)
            .any(|(id, v)| men.contains(id) && v & (1 << gunner) != 0) as u32;
        shots += s
            .shots
            .iter()
            .filter(|f| w.entities().owner()[f.shooter.index()] == PlayerId(gunner))
            .count() as u32;
    }
    Ok(SmokeRun {
        seed,
        hidden: faction(w.match_setup().unwrap(), hidden),
        smoke,
        seen_ticks: seen,
        mg_shots: shots,
        losses: losses(&w, hidden) - before,
    })
}

/// Runs every experiment.
pub fn fortify_test(args: &FortifyArgs) -> Result<FortifyReport> {
    if args.threads == 0 {
        bail!("--threads must be at least 1");
    }
    let seeds = args.first_seed..args.first_seed + args.seeds as u64;
    let mut rifles = Vec::new();
    let mut mortar = Vec::new();
    let mut mortar_ranged = Vec::new();
    let mut assault = Vec::new();
    let mut smoke = Vec::new();
    let mut suppress = Vec::new();
    for seed in seeds {
        for side in [0u8, 1] {
            rifles.push(rifle_run(args, seed, side)?);
            for (mg, def_mg, second) in [
                (true, false, false),
                (false, false, false),
                (true, true, false),
                (true, false, true),
            ] {
                suppress.push(suppress_run(args, seed, side, mg, def_mg, second)?);
            }
            for start in [ASSAULT_START_M, ASSAULT_GAP_M] {
                for grenades in [true, false] {
                    assault.push(assault_run(args, seed, side, start, grenades, false)?);
                }
            }
            for grenades in [true, false] {
                assault.push(assault_run(
                    args,
                    seed,
                    side,
                    ASSAULT_START_M,
                    grenades,
                    true,
                )?);
            }
            for with in [true, false] {
                smoke.push(smoke_run(args, seed, side, with)?);
            }
            let setup = match_setup(&SkirmishArgs {
                seed,
                mods: args.mods.clone(),
                rules_override: args.rules_override.clone(),
                ..SkirmishArgs::default()
            })?;
            for (ranged, runs) in [(false, &mut mortar), (true, &mut mortar_ranged)] {
                let (dug, t_dug) = mortar_run(args, seed, side, true, ranged)?;
                let (open, t_open) = mortar_run(args, seed, side, false, ranged)?;
                for (trench, l, t) in [(true, dug, t_dug), (false, open, t_open)] {
                    runs.push(Run {
                        seed,
                        entrenched: faction(&setup, side),
                        trench: Some(trench),
                        trench_losses: if trench { l } else { 0 },
                        open_losses: if trench { 0 } else { l },
                        seconds: t,
                    });
                }
            }
        }
    }
    let rifles = Experiment::new(
        "rifle squad in a trench vs rifle squad in the open at 150 m",
        rifles,
        RIFLE_LIMIT_PCT,
    );
    let mortar = Experiment::new(
        "10-round mortar mission on a rifle squad: trench vs open (S5b: Direct, adjusting on the target)",
        mortar,
        MORTAR_LIMIT_PCT,
    );
    let mortar_ranged = Experiment::new(
        "the same mission after ranging on a reference point 40 m off (S5b, report only: no warning round)",
        mortar_ranged,
        MORTAR_LIMIT_PCT,
    );
    let totals = |start: i32, g: bool, smoke: bool| {
        let mut t = AssaultTotals::default();
        for r in assault
            .iter()
            .filter(|r| r.grenades == g && r.start_m == start && r.smoke == smoke)
        {
            t.defender_losses += r.defender_losses;
            t.attacker_losses += r.attacker_losses;
            t.frags_thrown += r.frags_thrown;
            t.frags_in_trench += r.frags_in_trench;
        }
        t
    };
    let variants = [
        (ASSAULT_START_M, false),
        (ASSAULT_GAP_M, false),
        (ASSAULT_START_M, true),
    ]
    .into_iter()
    .map(|(start, smoke)| {
        let (with, without) = (totals(start, true, smoke), totals(start, false, smoke));
        AssaultVariant {
            start_m: start,
            smoke,
            ratio_pct: (without.defender_losses > 0).then(|| {
                (with.defender_losses as f64 * 1000.0 / without.defender_losses as f64).round()
                    / 10.0
            }),
            grenades: with,
            rifle_only: without,
        }
    })
    .collect();
    let grenade = GrenadeExperiment {
        name: "grenade assault on a dug-in rifle squad, frags from 25 m",
        variants,
        runs: assault,
    };
    let hidden_ok = smoke
        .iter()
        .filter(|r| r.smoke)
        .all(|r| r.seen_ticks == 0 && r.mg_shots == 0 && r.losses == 0);
    let control_ok = smoke
        .iter()
        .filter(|r| !r.smoke)
        .all(|r| r.seen_ticks > 0 && r.mg_shots >= SMOKE_CONTROL_SHOTS);
    let smoke = SmokeExperiment {
        name: "smoke 28 m in front of a rifle squad, MG at 100 m",
        pass: hidden_ok && control_ok,
        runs: smoke,
    };
    let sum = |mg: bool, def_mg: bool| {
        let mut t = SuppressTotals::default();
        for r in suppress
            .iter()
            .filter(|r| r.mg == mg && r.def_mg == def_mg && !r.second)
        {
            t.defender_losses += r.defender_losses;
            t.defender_men += r.defender_men;
            t.attacker_losses += r.attacker_losses;
            t.attacker_men += r.attacker_men;
            t.defender_ticks += r.ticks.0;
            t.defender_ducked += r.ticks.1;
            t.defender_pinned += r.ticks.2;
        }
        t
    };
    let (supported, unsupported, defended) = (sum(true, false), sum(false, false), sum(true, true));
    let suppress = SuppressExperiment {
        name: "rifle squad advancing from 150 m on a dug-in rifle squad, with and without an MG suppressing the trench",
        supported_met: supported.defender_losses * 4 >= supported.defender_men * 3
            && supported.attacker_losses * 2 <= supported.attacker_men,
        unsupported_met: unsupported.attacker_losses * 4 >= unsupported.attacker_men * 3,
        supported,
        unsupported,
        defended,
        runs: suppress,
    };
    Ok(FortifyReport {
        pass: rifles.pass && mortar.pass && smoke.pass,
        rifles,
        mortar,
        mortar_ranged,
        grenade,
        smoke,
        suppress,
    })
}

/// Human summary of a report (several lines, no trailing newline).
pub fn summary_text(r: &FortifyReport) -> String {
    let mut out = String::new();
    for (e, judged) in [
        (&r.rifles, true),
        (&r.mortar, true),
        (&r.mortar_ranged, false),
    ] {
        out += &format!("{}\n", e.name);
        out += "  seed  entrenched    target   trench_losses  open_losses  seconds\n";
        for run in &e.runs {
            let target = match run.trench {
                None => "-",
                Some(true) => "trench",
                Some(false) => "open",
            };
            out += &format!(
                "  {:<5} {:<13} {:<8} {:>13}  {:>11}  {:>7.1}\n",
                run.seed, run.entrenched, target, run.trench_losses, run.open_losses, run.seconds
            );
        }
        let ratio = e
            .ratio_pct
            .map_or_else(|| "n/a".into(), |p| format!("{p:.1} %"));
        out += &format!(
            "  total: trench {} vs open {} = {ratio} (limit {} %): {}\n",
            e.trench_losses,
            e.open_losses,
            e.limit_pct,
            match (judged, e.pass) {
                (false, true) => "within (report only)",
                (false, false) => "over (report only)",
                (true, true) => "PASS",
                (true, false) => "FAIL",
            }
        );
    }
    let g = &r.grenade;
    out += &format!("{}\n", g.name);
    out += "  seed  entrenched    start  smoke  grenades  frags  in_trench  defender_losses  attacker_losses\n";
    for run in &g.runs {
        out += &format!(
            "  {:<5} {:<13} {:>3} m  {:<6} {:<9} {:>5}  {:>9}  {:>15}  {:>15}\n",
            run.seed,
            run.entrenched,
            run.start_m,
            if run.smoke { "yes" } else { "no" },
            if run.grenades { "yes" } else { "no" },
            run.frags_thrown,
            run.frags_in_trench,
            run.defender_losses,
            run.attacker_losses
        );
    }
    for v in &g.variants {
        let ratio = v
            .ratio_pct
            .map_or_else(|| "n/a".into(), |p| format!("{p:.1} %"));
        out += &format!(
            "  from {} m{}: defenders lost {} with grenades ({} frags, {} in the trench) vs {} to rifles alone = {ratio}; attackers lost {} vs {} (no threshold yet)\n",
            v.start_m,
            if v.smoke { " behind smoke" } else { "" },
            v.grenades.defender_losses,
            v.grenades.frags_thrown,
            v.grenades.frags_in_trench,
            v.rifle_only.defender_losses,
            v.grenades.attacker_losses,
            v.rifle_only.attacker_losses,
        );
    }
    let s = &r.smoke;
    out += &format!("{}\n", s.name);
    out += "  seed  hidden        smoke  seen_ticks  mg_shots  losses\n";
    for run in &s.runs {
        out += &format!(
            "  {:<5} {:<13} {:<6} {:>10}  {:>8}  {:>6}\n",
            run.seed,
            run.hidden,
            if run.smoke { "yes" } else { "no" },
            run.seen_ticks,
            run.mg_shots,
            run.losses
        );
    }
    out += &format!(
        "  behind smoke: never seen, never fired on; control: seen and fired on: {}\n",
        if s.pass { "PASS" } else { "FAIL" }
    );
    let x = &r.suppress;
    out += &format!("{}\n", x.name);
    out += "  (mg 2: MG + a rifle squad suppressing, S3c second base of fire)\n";
    out += "  seed  entrenched    mg   def_mg  defender_losses  attacker_losses  frags  ducked  pinned\n";
    for run in &x.runs {
        out += &format!(
            "  {:<5} {:<13} {:<4} {:<7} {:>9} of {:<4} {:>9} of {:<4} {:>5}  {:>5.1} %  {:>5.1} %\n",
            run.seed,
            run.entrenched,
            match (run.mg, run.second) {
                (true, true) => "2",
                (true, false) => "yes",
                _ => "no",
            },
            if run.def_mg { "yes" } else { "no" },
            run.defender_losses,
            run.defender_men,
            run.attacker_losses,
            run.attacker_men,
            run.frags_thrown,
            run.ducked_pct,
            run.pinned_pct
        );
    }
    let pct = |a: u32, b: u32| a as f64 * 100.0 / b.max(1) as f64;
    let tick_pct = |a: u64, b: u64| a as f64 * 100.0 / b.max(1) as f64;
    for (name, t, met, target) in [
        (
            "with the MG",
            &x.supported,
            x.supported_met,
            "T10: defenders lose >= 75 %, attackers <= 50 %",
        ),
        (
            "without",
            &x.unsupported,
            x.unsupported_met,
            "T11: attackers lose >= 75 %",
        ),
        (
            "with the MG, against a trench with its own MG",
            &x.defended,
            true,
            "report only (review M5)",
        ),
    ] {
        out += &format!(
            "  {name}: defenders lost {} of {} ({:.1} %), attackers {} of {} ({:.1} %); defenders ducked {:.1} %, pinned {:.1} % of man-time; {target}: {}\n",
            t.defender_losses,
            t.defender_men,
            pct(t.defender_losses, t.defender_men),
            t.attacker_losses,
            t.attacker_men,
            pct(t.attacker_losses, t.attacker_men),
            tick_pct(t.defender_ducked, t.defender_ticks),
            tick_pct(t.defender_pinned, t.defender_ticks),
            if target.starts_with("report") {
                "-"
            } else if met {
                "met"
            } else {
                "missed"
            }
        );
    }
    // S3b Fix (review H1): T10 per faction (the entrenched side's faction;
    // the other attacks with its own MG).
    let mut factions: Vec<&str> = x.runs.iter().map(|r| r.entrenched.as_str()).collect();
    factions.sort_unstable();
    factions.dedup();
    let factions_all = factions.clone();
    for f in factions {
        let (mut dl, mut dm, mut al, mut am) = (0, 0, 0, 0);
        for r in x
            .runs
            .iter()
            .filter(|r| r.mg && !r.def_mg && !r.second && r.entrenched == f)
        {
            dl += r.defender_losses;
            dm += r.defender_men;
            al += r.attacker_losses;
            am += r.attacker_men;
        }
        // S3c (B4): the same with a second base of fire (MG + rifle squad
        // suppressing); the attackers counted are the assaulting squad.
        let (mut dl2, mut dm2, mut al2, mut am2) = (0, 0, 0, 0);
        for r in x.runs.iter().filter(|r| r.second && r.entrenched == f) {
            dl2 += r.defender_losses;
            dm2 += r.defender_men;
            al2 += r.attacker_losses;
            am2 += r.attacker_men;
        }
        // S3c: name the attacking faction too (S3b-41: T10 per attacker).
        let attacker = factions_all
            .iter()
            .find(|g| **g != f)
            .map_or("the other faction", |g| *g);
        out += &format!(
            "  T10, {f} entrenched ({attacker} attacking): defenders lost {dl} of {dm} ({:.1} %), attackers {al} of {am} ({:.1} %)\n",
            pct(dl, dm),
            pct(al, am)
        );
        out += &format!(
            "  T10, {f} entrenched ({attacker} attacking with two bases of fire, MG + rifle squad suppressing): defenders lost {dl2} of {dm2} ({:.1} %), attackers {al2} of {am2} ({:.1} %)\n",
            pct(dl2, dm2),
            pct(al2, am2)
        );
    }
    out += &format!("fortify-test: {}", if r.pass { "PASS" } else { "FAIL" });
    out
}
