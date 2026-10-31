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
//! * **Mortar.** A [`BARRAGE_ROUNDS`]-round barrage from a mortar team
//!   250 m away on a rifle squad, once in a trench and once in the open.
//!   Pass: losses in the trench are at most [`MORTAR_LIMIT_PCT`] % of those
//!   in the open.
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
/// Time given to a barrage (fire, flight and the last burst), game seconds.
const BARRAGE_S: u64 = 90;
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
const SMOKE_CONTROL_SHOTS: u32 = 20;

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
    /// Print the report as JSON instead of text.
    #[arg(long)]
    pub json: bool,
}

impl Default for FortifyArgs {
    fn default() -> FortifyArgs {
        FortifyArgs {
            seeds: 5,
            first_seed: 1,
            threads: 2,
            mods: None,
            json: false,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct FortifyReport {
    pub rifles: Experiment,
    pub mortar: Experiment,
    pub grenade: GrenadeExperiment,
    pub smoke: SmokeExperiment,
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
fn squad(w: &World, p: u8, role: &str) -> Result<SquadId> {
    let g = w.game().expect("match world");
    let s = g.setup();
    g.squads()
        .iter()
        .find(|(_, q)| {
            q.owner == PlayerId(p) && s.squad_types[q.type_idx as usize].id.ends_with(role)
        })
        .map(|(id, _)| *id)
        .ok_or_else(|| anyhow::anyhow!("player {p} has no {role} squad"))
}

/// Buys player `p`'s squad type ending in `role` and steps until it arrives.
fn call_in(w: &mut World, p: u8, role: &str) -> Result<SquadId> {
    let s = w.match_setup().expect("match world");
    let faction = &s.factions[s.players[p as usize].faction as usize];
    let Some(&ty) = faction
        .roster
        .iter()
        .find(|&&t| s.squad_types[t as usize].id.ends_with(role))
    else {
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
        ..SkirmishArgs::default()
    })?;
    setup.players.iter_mut().for_each(|p| p.is_ai = false);
    setup.rules.start_manpower = 10_000;
    Ok(World::new_match(setup, args.threads)?)
}

fn rifle_run(args: &FortifyArgs, seed: u64, dug: u8) -> Result<Run> {
    let mut w = world(args, seed)?;
    let open = 1 - dug;
    let a = squad(&w, dug, "rifle_squad")?;
    let b = squad(&w, open, "rifle_squad")?;
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
fn mortar_run(args: &FortifyArgs, seed: u64, target: u8, trench: bool) -> Result<(u32, f64)> {
    let mut w = world(args, seed)?;
    let gunner = 1 - target;
    let rifle = squad(&w, target, "rifle_squad")?;
    if trench {
        dig(&mut w);
    }
    w.place_squad(rifle, at(TRENCH_X, LINE_Y), EAST);
    let m = call_in(&mut w, gunner, "mortar_team")?;
    w.place_squad(m, at(TRENCH_X + MORTAR_GAP_M, LINE_Y), WEST);
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let start = w.tick();
    let before = losses(&w, target);
    let shells = w.stats().explosions;
    w.step(&[Command {
        player: PlayerId(gunner),
        seq: 1,
        kind: CommandKind::Barrage {
            squad: m,
            target: at(TRENCH_X, LINE_Y),
            rounds: BARRAGE_ROUNDS,
        },
    }]);
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
    let a = squad(&w, dug, "rifle_squad")?;
    let b = squad(&w, att, "rifle_squad")?;
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
    let mg = call_in(&mut w, gunner, "mg_team")?;
    let park = if gunner == 0 {
        at(80, 900)
    } else {
        at(900, 80)
    };
    w.place_squad(mg, park, EAST);
    let rifle = squad(&w, hidden, "rifle_squad")?;
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
    let mut assault = Vec::new();
    let mut smoke = Vec::new();
    for seed in seeds {
        for side in [0u8, 1] {
            rifles.push(rifle_run(args, seed, side)?);
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
                ..SkirmishArgs::default()
            })?;
            let (dug, t_dug) = mortar_run(args, seed, side, true)?;
            let (open, t_open) = mortar_run(args, seed, side, false)?;
            for (trench, l, t) in [(true, dug, t_dug), (false, open, t_open)] {
                mortar.push(Run {
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
    let rifles = Experiment::new(
        "rifle squad in a trench vs rifle squad in the open at 150 m",
        rifles,
        RIFLE_LIMIT_PCT,
    );
    let mortar = Experiment::new(
        "10-round 81 mm barrage on a rifle squad: trench vs open",
        mortar,
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
    Ok(FortifyReport {
        pass: rifles.pass && mortar.pass && smoke.pass,
        rifles,
        mortar,
        grenade,
        smoke,
    })
}

/// Human summary of a report (several lines, no trailing newline).
pub fn summary_text(r: &FortifyReport) -> String {
    let mut out = String::new();
    for e in [&r.rifles, &r.mortar] {
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
            if e.pass { "PASS" } else { "FAIL" }
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
    out += &format!("fortify-test: {}", if r.pass { "PASS" } else { "FAIL" });
    out
}
