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
//!
//! Squads are staged with the sim's setup tools (`World::place_squad`,
//! `World::dig_trench_now`), not commands, and the AI is off for both
//! players. Only the barrage is a real command.

use std::path::PathBuf;

use anyhow::{Result, bail};
use frontline_content::MatchSetup;
use frontline_sim::{Command, CommandKind, Fx, FxVec2, PlayerId, SquadId, World};
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
    pub pass: bool,
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

/// Runs both experiments.
pub fn fortify_test(args: &FortifyArgs) -> Result<FortifyReport> {
    if args.threads == 0 {
        bail!("--threads must be at least 1");
    }
    let seeds = args.first_seed..args.first_seed + args.seeds as u64;
    let mut rifles = Vec::new();
    let mut mortar = Vec::new();
    for seed in seeds {
        for side in [0u8, 1] {
            rifles.push(rifle_run(args, seed, side)?);
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
    Ok(FortifyReport {
        pass: rifles.pass && mortar.pass,
        rifles,
        mortar,
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
    out += &format!("fortify-test: {}", if r.pass { "PASS" } else { "FAIL" });
    out
}
