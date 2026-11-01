//! `armour-test`: the S3 staged armour cases (`docs/slices/S3-armour.md`
//! "`frontline-cli armour-test`").
//!
//! Each case is staged by `frontline_sim::scenarios` (setup tools, AI off,
//! test unlock rules) and then driven by real commands, for seeds
//! `--first-seed ..` × both factions (the measured side is player 0, then
//! player 1). **Hard** checks test mechanics and fail the run; **report**
//! rows print a number and, where the owner set one, the target next to
//! it. No balance number is a hard check.
//!
//! Statistical checks pool all seeds and factions: a count is "within 3σ"
//! when |observed − expected| ≤ 3·√(Σ p(1 − p)) + 1.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Result, anyhow};
use frontline_content::vehicle::AmmoMechanism;
use frontline_content::{ComponentKind, Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::armour::{self, BoxPart, PPM, PenResult};
use frontline_sim::scenarios::{
    self, AtRun, Duel, LANE_Y, SatchelRun, TRENCH_X, at, cmd, dir_deg, members, toughen,
};
use frontline_sim::{
    ArmourEventKind, BlastKind, CommandKind, EntityKind, Fx, FxVec2, FxVec3, ItemKind, LoadedSnap,
    Order, PlayerId, SimId, SquadId, VehicleState, World,
};
use serde::Serialize;

use crate::report::{Summary, ns, round3};
use crate::skirmish::mods_dir;

/// Case 1: hits wanted per run and pairing (10 runs make 130 ≥ 100).
pub const HITS_PER_RUN: usize = 13;
/// Case 12: crossings per angle per run (the spec's 200), and in the
/// cratered variant.
pub const CROSSINGS: u32 = 200;
pub const CRATERED_CROSSINGS: u32 = 20;

/// Options for [`armour_test`]; also the clap definition of the subcommand.
#[derive(Clone, Debug, clap::Args)]
pub struct ArmourArgs {
    /// Seeds `first-seed .. first-seed + seeds` are run, each with the
    /// factions both ways.
    #[arg(long, value_name = "N", default_value_t = 5)]
    pub seeds: u32,
    #[arg(long, value_name = "S", default_value_t = 1)]
    pub first_seed: u64,
    /// Sim worker threads. Never changes results.
    #[arg(long, value_name = "T", default_value_t = 3)]
    pub threads: usize,
    /// Mod directory (default: `mods/ww2` here or in the source tree).
    #[arg(long, value_name = "DIR")]
    pub mods: Option<PathBuf>,
    /// Only these cases (1–14), e.g. `--case 1 --case 7`.
    #[arg(long = "case", value_name = "N")]
    pub cases: Vec<u32>,
    /// Also run case 14, the stress case (lifts the vehicle limit and the
    /// pop cap).
    #[arg(long)]
    pub stress: bool,
    /// Case 12's crossings per angle per run.
    #[arg(long, value_name = "N", default_value_t = CROSSINGS)]
    pub crossings: u32,
    /// Print the report as JSON instead of text.
    #[arg(long)]
    pub json: bool,
    /// Print one line per run to stderr (diagnostics).
    #[arg(long)]
    pub verbose: bool,
    /// Tuning override (S3b): a partial TOML file merged onto the rules
    /// (`ww2:skirmish`, before the test unlock is derived from it) and defs.
    /// Repeatable; `docs/modding/tuning.md`.
    #[arg(long = "rules-override", value_name = "FILE")]
    pub rules_override: Vec<PathBuf>,
}

impl Default for ArmourArgs {
    fn default() -> ArmourArgs {
        ArmourArgs {
            seeds: 5,
            first_seed: 1,
            threads: 3,
            mods: None,
            cases: Vec::new(),
            stress: false,
            crossings: CROSSINGS,
            json: false,
            verbose: false,
            rules_override: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Check {
    pub name: String,
    pub pass: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Row {
    pub name: String,
    pub value: String,
    /// The owner's target, when one exists.
    pub target: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CaseReport {
    pub case: u32,
    pub name: &'static str,
    pub runs: u32,
    pub checks: Vec<Check>,
    pub rows: Vec<Row>,
    pub wall_s: f64,
}

impl CaseReport {
    fn new(case: u32, name: &'static str) -> CaseReport {
        CaseReport {
            case,
            name,
            runs: 0,
            checks: Vec::new(),
            rows: Vec::new(),
            wall_s: 0.0,
        }
    }
    fn check(&mut self, name: impl Into<String>, pass: bool, detail: impl Into<String>) {
        self.checks.push(Check {
            name: name.into(),
            pass,
            detail: detail.into(),
        });
    }
    fn row(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.rows.push(Row {
            name: name.into(),
            value: value.into(),
            target: None,
        });
    }
    fn target(&mut self, name: impl Into<String>, value: impl Into<String>, target: &str) {
        self.rows.push(Row {
            name: name.into(),
            value: value.into(),
            target: Some(target.into()),
        });
    }
    fn pass(&self) -> bool {
        self.checks.iter().all(|c| c.pass)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ArmourTestReport {
    pub seeds: Vec<u64>,
    pub threads: usize,
    pub stress: bool,
    pub cases: Vec<CaseReport>,
    pub pass: bool,
    pub wall_s: f64,
}

/// Loaded defs and the run options.
struct Env {
    defs: ModDefs,
    rules: String,
    threads: usize,
    seeds: Vec<u64>,
    crossings: u32,
    verbose: bool,
}

impl Env {
    fn setup(&self, seed: u64) -> Result<MatchSetup> {
        let mut req = MatchRequest::skirmish(seed, "ww2:germany", "ww2:usa", Difficulty::Normal);
        req.rules = self.rules.clone();
        self.defs.resolve(&req).map_err(|e| anyhow!("{e}"))
    }
    /// Every (seed, measured player) run.
    fn runs(&self) -> Vec<(u64, u8)> {
        self.seeds
            .iter()
            .flat_map(|&s| [(s, 0u8), (s, 1u8)])
            .collect()
    }
}

// ------------------------------------------------------------- helpers --

fn hz(w: &World) -> u64 {
    w.match_setup().map_or(30, |s| s.tick_hz as u64)
}

/// Steps once with `kinds` as player `p`'s commands.
fn order(w: &mut World, p: u8, kinds: Vec<CommandKind>) {
    let seq = w.tick() as u32 * 16;
    let cmds: Vec<_> = kinds
        .into_iter()
        .enumerate()
        .map(|(k, kind)| cmd(p, seq + k as u32, kind))
        .collect();
    w.step(&cmds);
}

fn vstate(w: &World, id: SimId) -> Option<VehicleState> {
    w.game().and_then(|g| g.vehicle(id)).cloned()
}

fn alive(w: &World, id: SimId) -> bool {
    w.entities().kind().get(id.index()) == Some(&EntityKind::Vehicle)
}

/// Knocked out, immobilised or its gun destroyed.
fn disabled(w: &World, id: SimId) -> bool {
    let Some(vs) = vstate(w, id) else {
        return true;
    };
    if !alive(w, id) || vs.wreck_tick.is_some() {
        return true;
    }
    let s = w.match_setup().expect("match");
    vs.immobilised(&s.vehicles[vs.type_idx as usize]) || vs.component(ComponentKind::Gun) == 0
}

fn losses(w: &World, p: u8) -> u32 {
    w.game().map_or(0, |g| g.players()[p as usize].losses)
}

fn pos(w: &World, id: SimId) -> FxVec2 {
    w.entities().transform()[id.index()].pos.xy()
}

fn dist(a: FxVec2, b: FxVec2) -> f64 {
    let d = a - b;
    let (x, y) = (f64::from(d.x.to_f32()), f64::from(d.y.to_f32()));
    (x * x + y * y).sqrt()
}

fn events(w: &World, kind: ArmourEventKind, vehicle: SimId) -> usize {
    w.snapshot()
        .armour_events
        .iter()
        .filter(|e| e.kind == kind as u8 && e.vehicle == vehicle.0)
        .count()
}

fn pct(n: f64, d: f64) -> String {
    if d <= 0.0 {
        "n/a".into()
    } else {
        format!("{:.1} %", n * 100.0 / d)
    }
}

fn mmss(ticks: u64, hz: u64) -> String {
    format!("{:.1} s", ticks as f64 / hz as f64)
}

/// `|obs − exp| ≤ 3·√var + 1`.
fn within_3s(obs: f64, exp: f64, var: f64) -> bool {
    (obs - exp).abs() <= 3.0 * var.sqrt() + 1.0
}

/// Ammo index → whether it is a launcher's HEAT rocket.
fn is_rocket(s: &MatchSetup, ammo: u16) -> bool {
    s.ammo
        .get(ammo as usize)
        .is_some_and(|a| a.mechanism == AmmoMechanism::Heat)
}

fn fired_by(w: &World, v: SimId) -> usize {
    events(w, ArmourEventKind::Fired, v)
}

// ------------------------------------------------------------ case 1 --

#[derive(Default)]
struct PairStat {
    hits: u64,
    pens: u64,
    ricochets: u64,
    expected: f64,
    var: f64,
    mismatches: u64,
    chance_mismatches: u64,
}

fn case1(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(1, "penetration statistics");
    let mut pairs: BTreeMap<(i32, i32), PairStat> = BTreeMap::new();
    for (seed, p) in env.runs() {
        for range in [150, 300] {
            for angle in [0, 20, 30, 90] {
                let mut s = scenarios::pen_stats(env.setup(seed)?, env.threads, p, range, angle)?;
                r.runs += 1;
                let target = s.tanks[1].1;
                let w = &mut s.world;
                w.enable_hit_log();
                let limit = w.tick() + 60 * hz(w);
                let mut got = Vec::new();
                while got.len() < HITS_PER_RUN && w.tick() < limit {
                    w.step(&[]);
                    toughen(w, target, false);
                    got.extend(
                        w.take_hit_log()
                            .into_iter()
                            .filter(|h| h.vehicle == target && h.player == p),
                    );
                }
                let st = pairs.entry((range, angle)).or_default();
                for h in got {
                    st.hits += 1;
                    // The rule replayed, and the CLI's closed form.
                    let again =
                        armour::penetrates(h.pen0, h.c, h.thickness, h.extra, h.cos_ricochet, h.v);
                    if again != h.result {
                        st.mismatches += 1;
                    }
                    let ppm =
                        armour::pen_chance_ppm(h.pen0, h.c, h.thickness, h.extra, h.cos_ricochet);
                    let wrong = (ppm == PPM && h.result != PenResult::Penetration)
                        || (ppm == 0 && h.result == PenResult::Penetration);
                    if wrong {
                        st.chance_mismatches += 1;
                    }
                    let q = f64::from(ppm) / f64::from(PPM);
                    st.expected += q;
                    st.var += q * (1.0 - q);
                    match h.result {
                        PenResult::Penetration => st.pens += 1,
                        PenResult::Ricochet => st.ricochets += 1,
                        PenResult::NonPenetration => {}
                    }
                }
            }
        }
    }
    let total_hits: u64 = pairs.values().map(|s| s.hits).sum();
    let mism: u64 = pairs
        .values()
        .map(|s| s.mismatches + s.chance_mismatches)
        .sum();
    r.check(
        "every logged hit's outcome equals the rule (penetrates, pen_chance_ppm)",
        mism == 0 && total_hits > 0,
        format!("{total_hits} hits, {mism} mismatches"),
    );
    for (&(range, angle), st) in &pairs {
        r.check(
            format!("{range} m, {angle}°: at least 100 hits"),
            st.hits >= 100,
            format!("{} hits", st.hits),
        );
        r.check(
            format!("{range} m, {angle}°: penetrations within 3σ of Σp"),
            within_3s(st.pens as f64, st.expected, st.var),
            format!(
                "observed {}, expected {:.1} (σ {:.1})",
                st.pens,
                st.expected,
                st.var.sqrt()
            ),
        );
        r.row(
            format!("{range} m, target at {angle}°"),
            format!(
                "{} hits: {} pen ({}), {} ricochet, expected pen {:.1}",
                st.hits,
                st.pens,
                pct(st.pens as f64, st.hits as f64),
                st.ricochets,
                st.expected
            ),
        );
    }
    Ok(r)
}

// ------------------------------------------------------------ case 2 --

fn case2(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(2, "hull-down");
    let (mut hull, mut turret) = (0u64, 0u64);
    let mut outcome: BTreeMap<&str, u32> = BTreeMap::new();
    for (seed, p) in env.runs() {
        let mut s = scenarios::hull_down(env.setup(seed)?, env.threads, p)?;
        r.runs += 1;
        let (hd, open) = (s.tanks[0].1, s.tanks[1].1);
        let w = &mut s.world;
        w.enable_hit_log();
        let limit = w.tick() + 120 * hz(w);
        while w.tick() < limit && alive(w, hd) && alive(w, open) {
            w.step(&[]);
            for h in w.take_hit_log() {
                if h.vehicle == hd {
                    match h.part {
                        BoxPart::Hull => hull += 1,
                        BoxPart::Turret => turret += 1,
                    }
                }
            }
        }
        let key = match (disabled(w, hd), disabled(w, open)) {
            (false, true) => "hull-down tank won",
            (true, false) => "open tank won",
            (true, true) => "both disabled",
            (false, false) => "no result in 2 min",
        };
        *outcome.entry(key).or_default() += 1;
    }
    let hits = hull + turret;
    r.check(
        "hull hits ≤ 2 % of hits on the hull-down tank",
        hits > 0 && hull * 100 <= 2 * hits,
        format!("{hull} hull of {hits} hits"),
    );
    for (k, n) in outcome {
        r.row(k, format!("{n} of {} runs", r.runs));
    }
    Ok(r)
}

// ------------------------------------------------------------ case 3 --

#[derive(Default)]
struct DuelStat {
    runs: u32,
    pz_wins: u32,
    sh_wins: u32,
    first_disable_ticks: Vec<u64>,
    /// Hits by (shooter player, part).
    hits: BTreeMap<(u8, BoxPart), u64>,
    aim_turret: u64,
    aim_hull: u64,
    /// Knock-outs by cause (`ArmourEvent::component`: crew, rack, fire),
    /// and ticks to each side's first main-gun shot (S3b baseline).
    ko_cause: [u32; 3],
    first_shot: [Vec<u64>; 2],
}

fn case3(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(3, "tank duels");
    let mut stats: BTreeMap<&str, DuelStat> = BTreeMap::new();
    let mut tick_hz = 30;
    for (seed, flip) in env.runs() {
        for (name, kind) in [
            ("a 300 m frontal", Duel::Frontal300),
            ("b 300 m, Panzer IV angled 30°", Duel::Angled300),
            ("c 150 m, Sherman on the flank", Duel::Flank150),
        ] {
            let mut s = scenarios::duel(env.setup(seed)?, env.threads, kind, flip == 1)?;
            r.runs += 1;
            let (pz, sh) = (s.tanks[0].1, s.tanks[1].1);
            let w = &mut s.world;
            tick_hz = hz(w);
            w.enable_hit_log();
            let start = w.tick();
            let limit = start + 120 * hz(w);
            let st = stats.entry(name).or_default();
            st.runs += 1;
            let mut first = None;
            let mut shot: [Option<u64>; 2] = [None, None];
            while w.tick() < limit && alive(w, pz) && alive(w, sh) {
                w.step(&[]);
                for e in &w.snapshot().armour_events {
                    if e.kind == ArmourEventKind::KnockedOut as u8 {
                        if let Some(c) = st.ko_cause.get_mut(e.component as usize) {
                            *c += 1;
                        }
                    } else if e.kind == ArmourEventKind::Fired as u8 {
                        let side = (e.vehicle == sh.0) as usize;
                        if shot[side].is_none() {
                            shot[side] = Some(w.tick() - start);
                        }
                    }
                }
                for h in w.take_hit_log() {
                    *st.hits.entry((h.player, h.part)).or_default() += 1;
                }
                let dis = w.snapshot().armour_events.iter().any(|e| {
                    e.kind == ArmourEventKind::Disabled as u8
                        || e.kind == ArmourEventKind::KnockedOut as u8
                });
                if dis && first.is_none() {
                    first = Some(w.tick() - start);
                }
                if kind == Duel::Angled300
                    && let Some(v) = vstate(w, sh)
                    && v.target == Some(pz)
                {
                    if v.aim_turret {
                        st.aim_turret += 1;
                    } else {
                        st.aim_hull += 1;
                    }
                }
            }
            if let Some(t) = first {
                st.first_disable_ticks.push(t);
            }
            for (k, t) in shot.iter().enumerate() {
                if let Some(t) = t {
                    st.first_shot[k].push(*t);
                }
            }
            match (alive(w, pz), alive(w, sh)) {
                (true, false) => st.pz_wins += 1,
                (false, true) => st.sh_wins += 1,
                _ => {}
            }
        }
    }
    for (name, st) in &stats {
        let mut t = st.first_disable_ticks.clone();
        t.sort_unstable();
        let median = t
            .get(t.len() / 2)
            .map_or("none".into(), |&x| mmss(x, tick_hz));
        let hits = |p: u8| {
            let h = st.hits.get(&(p, BoxPart::Hull)).copied().unwrap_or(0);
            let tu = st.hits.get(&(p, BoxPart::Turret)).copied().unwrap_or(0);
            format!("{h} hull / {tu} turret")
        };
        let med = |v: &Vec<u64>| {
            let mut v = v.clone();
            v.sort_unstable();
            v.get(v.len() / 2).map_or("none".into(), |&x| {
                format!("{:.1} s", x as f64 / tick_hz as f64)
            })
        };
        r.row(
            format!("3{name}: knock-outs by cause, first shot"),
            format!(
                "crew {}, ammo rack {}, fire {}; first main-gun shot (median) Panzer IV {}, \
                 Sherman {}",
                st.ko_cause[0],
                st.ko_cause[1],
                st.ko_cause[2],
                med(&st.first_shot[0]),
                med(&st.first_shot[1])
            ),
        );
        r.row(
            format!("3{name}"),
            format!(
                "Panzer IV won {}, Sherman won {}, no knock-out {} of {}; first disabling hit \
                 (median) {median}; hits by Panzer IV {}, by Sherman {}",
                st.pz_wins,
                st.sh_wins,
                st.runs - st.pz_wins - st.sh_wins,
                st.runs,
                hits(0),
                hits(1)
            ),
        );
    }
    if let Some(st) = stats.get("c 150 m, Sherman on the flank") {
        let h = st.hits.get(&(1, BoxPart::Hull)).copied().unwrap_or(0);
        let all = h + st.hits.get(&(1, BoxPart::Turret)).copied().unwrap_or(0);
        r.check(
            "3c (both parts penetrated at 150 m on the flank): ≥ 40 % of the Sherman's hits on the hull",
            all > 0 && h * 100 >= 40 * all,
            format!("{h} of {all}"),
        );
    }
    if let Some(st) = stats.get("b 300 m, Panzer IV angled 30°") {
        r.check(
            "3b: the Sherman's gunner aims at the angled Panzer IV's turret",
            st.aim_turret > 0 && st.aim_hull == 0,
            format!(
                "ticks aiming turret {}, hull {}",
                st.aim_turret, st.aim_hull
            ),
        );
    }
    // 3d: crossing vs stationary hit rate.
    let (mut shots, mut hits) = ([0u64; 2], [0u64; 2]);
    for (seed, p) in env.runs() {
        for (k, moving) in [(0usize, false), (1, true)] {
            let mut s = scenarios::crossing_target(env.setup(seed)?, env.threads, p, moving)?;
            r.runs += 1;
            let (shooter, target) = (s.tanks[0].1, s.tanks[1].1);
            let tsq = s.tanks[1].0;
            let w = &mut s.world;
            w.enable_hit_log();
            if moving {
                order(
                    w,
                    1 - p,
                    vec![CommandKind::Move {
                        squad: tsq,
                        target: at(650, LANE_Y + 130),
                    }],
                );
            }
            let limit = w.tick() + 40 * hz(w);
            while w.tick() < limit {
                w.step(&[]);
                toughen(w, target, false);
                shots[k] += fired_by(w, shooter) as u64;
                hits[k] += w
                    .take_hit_log()
                    .iter()
                    .filter(|h| h.vehicle == target)
                    .count() as u64;
            }
        }
    }
    let rate = |k: usize| hits[k] as f64 / shots[k].max(1) as f64;
    r.target(
        "3d hit rate, crossing at 6 m/s vs stationary, 300 m",
        format!(
            "crossing {}/{} ({}), stationary {}/{} ({}): crossing is {} of stationary",
            hits[1],
            shots[1],
            pct(hits[1] as f64, shots[1] as f64),
            hits[0],
            shots[0],
            pct(hits[0] as f64, shots[0] as f64),
            pct(rate(1), rate(0))
        ),
        "crossing ≤ 60 % of stationary",
    );
    Ok(r)
}

// ------------------------------------------------------------ case 4 --

fn case4(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(4, "AT team vs tank");
    let mut no_hit: Vec<String> = Vec::new();
    // The HE variant's team is under HE fire, so its rockets go out with
    // suppressed spread: about 57 % hit at 100 m, and a run of 6 misses
    // happens by chance (S3 fix pass, slice log). Counted, not failed.
    let mut he_no_hit: Vec<String> = Vec::new();
    let mut violations = 0u32;
    let mut stat: BTreeMap<&str, (u64, u64, u32, u32)> = BTreeMap::new();
    for (seed, p) in env.runs() {
        for (name, run) in [
            ("flank", AtRun::Flank),
            ("front", AtRun::Front),
            ("HE from 100 m", AtRun::He),
        ] {
            let mut s = scenarios::at_team(env.setup(seed)?, env.threads, p, run)?;
            r.runs += 1;
            let team = s.squads[0];
            let (tsq, tank) = s.tanks[0];
            let w = &mut s.world;
            w.enable_hit_log();
            let setup = w.match_setup().expect("match").clone();
            let moving: Fx = setup.rules.vehicles.moving_speed.into();
            let ty = w.game().unwrap().squad(team).unwrap().type_idx;
            let lw = &setup.weapons[setup.squad_types[ty as usize].weapon as usize];
            let engage_moving = f64::from(Fx::from(lw.engage_moving).to_f32());
            let men0 = members(w, team).len() as u32;
            let lane = TRENCH_X + scenarios::AT_FLANK_M;
            let (north, south) = (at(lane, LANE_Y + 110), at(lane, LANE_Y - 110));
            match run {
                AtRun::Flank => order(
                    w,
                    1 - p,
                    vec![CommandKind::Move {
                        squad: tsq,
                        target: north,
                    }],
                ),
                AtRun::Front => order(
                    w,
                    1 - p,
                    vec![CommandKind::Move {
                        squad: tsq,
                        target: at(TRENCH_X + 30, LANE_Y),
                    }],
                ),
                AtRun::He => {}
            }
            let mut going_north = true;
            let limit = w.tick() + 120 * hz(w);
            let (mut rocket_hits, mut disabling) = (0u64, 0u64);
            while w.tick() < limit && alive(w, tank) && !members(w, team).is_empty() {
                let fired0 = w.stats().rockets_fired;
                // The tank and the gunner as they stand when the rocket leaves.
                let carrier = w
                    .game()
                    .and_then(|g| g.squad(team))
                    .and_then(|q| q.launcher.as_ref())
                    .and_then(|l| l.carrier);
                let (tpos, tspeed) = (pos(w, tank), vstate(w, tank).map_or(Fx::ZERO, |v| v.speed));
                let cpos = carrier.map(|c| pos(w, c));
                if run == AtRun::Flank
                    && w.game()
                        .unwrap()
                        .squad(tsq)
                        .is_some_and(|q| q.order == Order::Idle)
                {
                    going_north = !going_north;
                    let target = if going_north { north } else { south };
                    order(w, 1 - p, vec![CommandKind::Move { squad: tsq, target }]);
                } else {
                    w.step(&[]);
                }
                if w.stats().rockets_fired > fired0
                    && tspeed.abs() > moving
                    && cpos.is_some_and(|c| dist(c, tpos) > engage_moving + 1.0)
                {
                    violations += 1;
                }
                let hits: Vec<_> = w
                    .take_hit_log()
                    .into_iter()
                    .filter(|h| h.vehicle == tank && is_rocket(&setup, h.ammo))
                    .collect();
                if !hits.is_empty() {
                    rocket_hits += hits.len() as u64;
                    if events(w, ArmourEventKind::Disabled, tank) > 0
                        || events(w, ArmourEventKind::KnockedOut, tank) > 0
                    {
                        disabling += 1;
                    }
                }
            }
            if rocket_hits == 0 {
                if run == AtRun::He {
                    he_no_hit.push(format!("seed {seed} p{p}"));
                } else {
                    no_hit.push(format!("seed {seed} p{p} {name}"));
                }
            }
            if env.verbose {
                eprintln!(
                    "case 4 seed {seed} p{p} {name}: {:.0} s, rockets fired {}, hits {rocket_hits}, \
                     disabling {disabling}, team men {} of {men0}, tank at {:?} disabled {}",
                    w.tick() as f64 / hz(w) as f64,
                    w.stats().rockets_fired,
                    members(w, team).len(),
                    pos(w, tank),
                    disabled(w, tank)
                );
            }
            let lost = men0 - members(w, team).len() as u32;
            let e = stat.entry(name).or_default();
            e.0 += rocket_hits;
            e.1 += disabling;
            e.2 += lost;
            e.3 += disabled(w, tank) as u32;
        }
    }
    r.check(
        "at least one rocket hit per flank and front run",
        no_hit.is_empty(),
        if no_hit.is_empty() {
            "every run".to_string()
        } else {
            format!("no hit in: {}", no_hit.join(", "))
        },
    );
    r.check(
        "no launcher shot at a moving tank beyond engage_moving",
        violations == 0,
        format!("{violations} shots"),
    );
    r.row(
        "HE from 100 m, runs with no rocket hit (team under HE fire)",
        if he_no_hit.is_empty() {
            "none".to_string()
        } else {
            format!("{}: {}", he_no_hit.len(), he_no_hit.join(", "))
        },
    );
    for (name, (hits, dis, lost, tanks)) in &stat {
        let row = format!(
            "rocket hits {hits}, disabling {dis} ({}), tanks disabled {tanks}, team men lost {lost}",
            pct(*dis as f64, *hits as f64)
        );
        if *name == "flank" {
            r.target("flank, disabling-hit rate", row, "≥ 50 %");
        } else {
            r.row(*name, row);
        }
    }
    Ok(r)
}

// ------------------------------------------------------- cases 5 and 6 --

fn case5(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(5, "lone tank vs dug-in rifles, no AT");
    let (mut hurt, mut died) = (Vec::new(), 0u32);
    let (mut lost, mut men, mut pinned, mut living) = (0u32, 0u32, 0u64, 0u64);
    // S3b baseline: the tank's rounds (main gun and MGs) and the
    // defenders' time up on the fire step.
    let (mut tank_rounds, mut up) = (0u64, 0u64);
    for (seed, p) in env.runs() {
        let mut s = scenarios::lone_tank(env.setup(seed)?, env.threads, p, false, false)?;
        r.runs += 1;
        let rifles = s.squads[0];
        let tank = s.tanks[0].1;
        let w = &mut s.world;
        let before = vstate(w, tank).expect("tank");
        let men0 = members(w, rifles).len() as u32;
        let lost0 = losses(w, 1 - p);
        let limit = w.tick() + 180 * hz(w);
        while w.tick() < limit {
            w.step(&[]);
            tank_rounds += w
                .snapshot()
                .shots
                .iter()
                .filter(|sh| sh.shooter == tank)
                .count() as u64;
            for m in members(w, rifles) {
                let c = &w.entities().combat()[m.index()];
                living += 1;
                pinned += c.pinned as u64;
                up += (c.in_trench && c.height == c.fire_height) as u64;
            }
        }
        match vstate(w, tank) {
            Some(after) if alive(w, tank) => {
                if after
                    .seats
                    .iter()
                    .zip(&before.seats)
                    .any(|(a, b)| a.hp != b.hp)
                    || after.components != before.components
                {
                    hurt.push(format!("seed {seed} p{p}"));
                }
            }
            _ => died += 1,
        }
        lost += losses(w, 1 - p) - lost0;
        men += men0;
    }
    r.check(
        "no component or seat damage from small arms",
        hurt.is_empty(),
        if hurt.is_empty() {
            "none".to_string()
        } else {
            hurt.join(", ")
        },
    );
    r.check(
        "the tank survives every run",
        died == 0,
        format!("{died} lost"),
    );
    r.row(
        "defender losses",
        format!("{lost} of {men} men ({})", pct(lost as f64, men as f64)),
    );
    r.row(
        "share of defender time pinned",
        pct(pinned as f64, living as f64),
    );
    r.row(
        "tank rounds fired (gun and MGs), defender time up on the fire step",
        format!("{tank_rounds}, {}", pct(up as f64, living as f64)),
    );
    Ok(r)
}

fn case6(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(6, "lone tank vs dug-in rifles + AT team, 300 → 50 m");
    let (mut dis, mut lost, mut rockets, mut hits, mut pens) = (0u32, 0u32, 0u64, 0u64, 0u64);
    for (seed, p) in env.runs() {
        let mut s = scenarios::lone_tank(env.setup(seed)?, env.threads, p, true, true)?;
        r.runs += 1;
        let (tsq, tank) = s.tanks[0];
        let w = &mut s.world;
        w.enable_hit_log();
        let setup = w.match_setup().expect("match").clone();
        let lost0 = losses(w, 1 - p);
        let rockets0 = w.stats().rockets_fired;
        order(
            w,
            p,
            vec![CommandKind::AttackMove {
                squad: tsq,
                target: at(TRENCH_X + 50, LANE_Y),
            }],
        );
        let limit = w.tick() + 180 * hz(w);
        while w.tick() < limit && !disabled(w, tank) {
            w.step(&[]);
            for h in w.take_hit_log() {
                if h.vehicle == tank && is_rocket(&setup, h.ammo) {
                    hits += 1;
                    pens += (h.result == PenResult::Penetration) as u64;
                }
            }
        }
        dis += disabled(w, tank) as u32;
        lost += losses(w, 1 - p) - lost0;
        rockets += w.stats().rockets_fired - rockets0;
        if env.verbose {
            eprintln!(
                "case 6 seed {seed} p{p}: {:.0} s, tank at {:?} disabled {}, rockets {}, \
                 defenders lost {}",
                w.tick() as f64 / hz(w) as f64,
                pos(w, tank),
                disabled(w, tank),
                w.stats().rockets_fired - rockets0,
                losses(w, 1 - p) - lost0
            );
        }
    }
    r.target(
        "tank disabled",
        format!("{dis} of {} ({})", r.runs, pct(dis as f64, r.runs as f64)),
        "≥ 60 %",
    );
    r.row(
        "rockets",
        format!("{rockets} fired, {hits} hit, {pens} penetrated"),
    );
    r.row("defender losses", format!("{lost} over {} runs", r.runs));
    Ok(r)
}

// ------------------------------------------------------------ case 7 --

/// Every attacker with a frag who is free to throw throws it at the
/// trench section in front of him.
fn throw_wave(w: &mut World, p: u8, squad: SquadId) {
    let kinds: Vec<CommandKind> = members(w, squad)
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
                .clamp(Fx::from_int(LANE_Y - 15), Fx::from_int(LANE_Y + 15));
            CommandKind::Throw {
                units: vec![m],
                item: ItemKind::Frag,
                target: FxVec2::new(Fx::from_int(TRENCH_X), y),
            }
        })
        .collect();
    order(w, p, kinds);
}

#[derive(Default)]
struct AssaultStat {
    att_lost: u32,
    def_lost: u32,
    def_men: u32,
    def_shots: u64,
    stopped: u64,
    /// Defender man-ticks alive, pinned, up on the fire step, and the sum
    /// of their suppression points (S3b baseline).
    def_ticks: u64,
    def_pinned: u64,
    def_up: u64,
    def_supp: u64,
}

fn case7(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(
        7,
        "tank-supported assault (7) and the same without the tank (7b)",
    );
    let mut st = [AssaultStat::default(), AssaultStat::default()];
    for (seed, p) in env.runs() {
        for (k, with_tank) in [(0usize, true), (1, false)] {
            let mut s = scenarios::assault(env.setup(seed)?, env.threads, p, with_tank)?;
            r.runs += 1;
            let (att, def) = (s.squads[0], s.squads[1]);
            let tank = s.tanks.first().copied();
            let w = &mut s.world;
            let start = w.tick();
            let lost0 = (losses(w, p), losses(w, 1 - p));
            let stopped0 = w.stats().rounds_stopped;
            let men0 = members(w, def).len() as u32;
            let goal = at(TRENCH_X + 25, LANE_Y);
            match tank {
                Some((tsq, tv)) => order(
                    w,
                    p,
                    vec![
                        CommandKind::FollowUnits {
                            units: members(w, att),
                            vehicle: tv,
                        },
                        CommandKind::Move {
                            squad: tsq,
                            target: goal,
                        },
                    ],
                ),
                None => order(
                    w,
                    p,
                    vec![CommandKind::Move {
                        squad: att,
                        target: goal,
                    }],
                ),
            }
            let mut released = tank.is_none();
            let limit = start + 120 * hz(w);
            let throw_x = Fx::from_int(TRENCH_X + 27);
            while w.tick() < limit {
                if !released && let Some((tsq, _)) = tank {
                    let arrived = w
                        .game()
                        .unwrap()
                        .squad(tsq)
                        .is_none_or(|q| q.order == Order::Idle);
                    if arrived || w.tick() > start + 90 * hz(w) {
                        released = true;
                        order(
                            w,
                            p,
                            vec![CommandKind::Move {
                                squad: att,
                                target: at(TRENCH_X + 25, LANE_Y + 6),
                            }],
                        );
                        continue;
                    }
                }
                let close = released
                    && w.game().unwrap().squad(att).is_some_and(|q| {
                        q.nav.phase == frontline_sim::NavPhase::Holding
                            || pos(w, q.lead(w.entities())).x <= throw_x
                    });
                if close && (w.tick() - start).is_multiple_of(hz(w)) {
                    throw_wave(w, p, att);
                } else {
                    w.step(&[]);
                }
                let owner = w.entities().owner();
                for m in members(w, def) {
                    let c = &w.entities().combat()[m.index()];
                    st[k].def_ticks += 1;
                    st[k].def_pinned += c.pinned as u64;
                    st[k].def_up += (c.in_trench && c.height == c.fire_height) as u64;
                    st[k].def_supp += (c.suppression / 1000) as u64;
                }
                st[k].def_shots += w
                    .snapshot()
                    .shots
                    .iter()
                    .filter(|sh| owner.get(sh.shooter.index()) == Some(&PlayerId(1 - p)))
                    .count() as u64;
            }
            st[k].att_lost += losses(w, p) - lost0.0;
            st[k].def_lost += losses(w, 1 - p) - lost0.1;
            st[k].def_men += men0;
            st[k].stopped += w.stats().rounds_stopped - stopped0;
        }
    }
    let [t, b] = &st;
    r.row(
        "7 with tank: attacker / defender losses",
        format!(
            "{} / {} of {} ({})",
            t.att_lost,
            t.def_lost,
            t.def_men,
            pct(t.def_lost as f64, t.def_men as f64)
        ),
    );
    r.row(
        "7b without tank: attacker / defender losses",
        format!(
            "{} / {} of {} ({})",
            b.att_lost,
            b.def_lost,
            b.def_men,
            pct(b.def_lost as f64, b.def_men as f64)
        ),
    );
    r.row(
        "7: rounds stopped by vehicles per defender round",
        format!(
            "{} of {} ({})",
            t.stopped,
            t.def_shots,
            pct(t.stopped as f64, t.def_shots as f64)
        ),
    );
    for (name, a) in [("7", t), ("7b", b)] {
        r.row(
            format!("{name}: defender posture (share of man-time)"),
            format!(
                "pinned {}, up on the fire step {}, mean suppression {:.1} points",
                pct(a.def_pinned as f64, a.def_ticks as f64),
                pct(a.def_up as f64, a.def_ticks as f64),
                a.def_supp as f64 / a.def_ticks.max(1) as f64
            ),
        );
    }
    let met_att = 2 * t.att_lost <= b.att_lost;
    let met_def = 4 * t.def_lost >= 3 * t.def_men;
    r.target(
        "owner question 5",
        format!(
            "attacker losses {} vs baseline {} ({} of it: {}); defenders lost {}: {}",
            t.att_lost,
            b.att_lost,
            pct(t.att_lost as f64, b.att_lost as f64),
            if met_att { "met" } else { "missed" },
            pct(t.def_lost as f64, t.def_men as f64),
            if met_def { "met" } else { "missed" }
        ),
        "attacker losses ≤ 50 % of 7b's, defenders lose ≥ 75 %",
    );
    Ok(r)
}

// ------------------------------------------------------------ case 8 --

fn case8(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(8, "satchels");
    let mut failures = Vec::new();
    let mut bursts_checked = 0u32;
    let mut stat: BTreeMap<&str, (u32, u32, u32, u32)> = BTreeMap::new();
    for (seed, p) in env.runs() {
        for (name, run) in [
            ("40 m, tank MGs live", SatchelRun::Open),
            ("40 m, rifles supporting", SatchelRun::Supported),
            ("in a trench the tank crosses", SatchelRun::Trench),
        ] {
            let mut s = scenarios::satchel(env.setup(seed)?, env.threads, p, run)?;
            r.runs += 1;
            let eng = s.squads[0];
            let (tsq, tank) = s.tanks[0];
            let w = &mut s.world;
            let setup = w.match_setup().expect("match").clone();
            let ty = &setup.squad_types[w.game().unwrap().squad(eng).unwrap().type_idx as usize];
            let (_, it) = ty
                .kit(&setup.items, ItemKind::Satchel)
                .ok_or_else(|| anyhow!("engineers carry no satchel"))?;
            let lethal: Fx = it
                .he
                .as_ref()
                .map_or(Fx::ZERO, |h| h.blast.lethal_radius.into());
            let fuse = Fx::from_int(it.fuse_ticks as i32).div_int(setup.tick_hz as i32);
            let men0 = members(w, eng).len() as u32;
            let throws0 = w.stats().throws;
            let geo =
                w.game().unwrap().vehicle_derived().geo[vstate(w, tank).unwrap().type_idx as usize];
            match run {
                SatchelRun::Trench => order(
                    w,
                    1 - p,
                    vec![CommandKind::Move {
                        squad: tsq,
                        target: at(TRENCH_X - 25, LANE_Y),
                    }],
                ),
                _ => order(
                    w,
                    p,
                    vec![CommandKind::Move {
                        squad: eng,
                        target: at(TRENCH_X + 6, LANE_Y),
                    }],
                ),
            }
            let limit = w.tick() + 60 * hz(w);
            let mut thrown = false;
            while w.tick() < limit && !members(w, eng).is_empty() {
                // A satchel beside the nearer track at the tank's position
                // when the fuse runs out (the AI's rule), once a second.
                let mut kinds = Vec::new();
                // In the open once a second; at the trench every tick (the
                // crossing is short).
                let now = run == SatchelRun::Trench || w.tick().is_multiple_of(hz(w));
                if !thrown && alive(w, tank) && now {
                    let vs = vstate(w, tank).unwrap();
                    let t = w.entities().transform()[tank.index()];
                    let slow = w.match_setup().unwrap().rules.ai.infantry.satchel_slow_mm_s;
                    if vs.speed.abs() <= Fx::from_milli(slow as i32) {
                        let at_ = t.pos.xy() + vs.avg_vel * fuse;
                        let left = frontline_sim::vehicle::yaw_dir(t.yaw).perp();
                        let men = members(w, eng);
                        let lead = pos(w, men[0]);
                        let side = if (lead - at_).dot(left) >= Fx::ZERO {
                            Fx::ONE
                        } else {
                            -Fx::ONE
                        };
                        let target = at_ + left * (side * (geo.hull_half.y + Fx::from_milli(800)));
                        kinds.push(CommandKind::Throw {
                            units: men,
                            item: ItemKind::Satchel,
                            target,
                        });
                    }
                }
                if env.verbose && w.tick().is_multiple_of(hz(w) / 2) && alive(w, tank) {
                    let vs = vstate(w, tank).unwrap();
                    eprintln!(
                        "case 8 seed {seed} p{p} {name} t {}: tank {:?} speed {:?} crossing {} men {} order {}",
                        w.tick() / hz(w),
                        pos(w, tank),
                        vs.speed,
                        vs.crossing,
                        members(w, eng).len(),
                        !kinds.is_empty()
                    );
                }
                if kinds.is_empty() {
                    w.step(&[]);
                } else {
                    order(w, p, kinds);
                    thrown = w.stats().throws > throws0;
                }
                for e in w.snapshot().explosions.clone() {
                    if e.kind != BlastKind::Satchel {
                        continue;
                    }
                    let f = |x: f32| Fx::from_milli((x * 1000.0).round() as i32);
                    let bp = FxVec3::new(f(e.pos[0]), f(e.pos[1]), f(e.pos[2]));
                    for (v, k) in scenarios::tracks_in_blast(w, bp, lethal, Fx::from_milli(50)) {
                        bursts_checked += 1;
                        let cut = vstate(w, v)
                            .is_none_or(|vs| vs.component(k) == 0 || vs.wreck_tick.is_some());
                        if !cut {
                            failures.push(format!("seed {seed} p{p} {name}: {k:?} kept"));
                        }
                    }
                }
            }
            let e = stat.entry(name).or_default();
            e.0 += disabled(w, tank) as u32;
            e.1 += men0 - members(w, eng).len() as u32;
            e.2 += men0;
            e.3 += (w.stats().throws > throws0) as u32;
        }
    }
    r.check(
        "a satchel burst with a track anchor inside its lethal radius destroys that track",
        failures.is_empty(),
        if failures.is_empty() {
            format!("{bursts_checked} tracks in reach, all destroyed")
        } else {
            failures.join("; ")
        },
    );
    for (name, (dis, lost, men, threw)) in stat {
        r.row(
            name,
            format!(
                "tank disabled in {dis} of {} runs; satchel thrown in {threw}; engineers lost {lost} of {men}",
                r.runs / 3
            ),
        );
    }
    Ok(r)
}

// ------------------------------------------------------------ case 9 --

/// Parapet lip above the base ground along the trench, per metre.
fn lip_profile(w: &World) -> Vec<f32> {
    (LANE_Y - 20..=LANE_Y + 20)
        .map(|y| {
            let base = w.terrain().base_height_at(at(TRENCH_X, y));
            let top = (0..=8)
                .map(|k| {
                    w.terrain().height_at(FxVec2::new(
                        Fx::from_int(TRENCH_X) + Fx::from_milli(500 * k),
                        Fx::from_int(y),
                    ))
                })
                .max()
                .unwrap_or(base);
            (top - base).to_f32()
        })
        .collect()
}

fn case9(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(9, "tank HE vs infantry");
    let mut lost = [0u32; 2];
    let mut fired = [0u64; 2];
    let (mut lip_lost, mut trench_hits, mut gaps) = (0f64, 0u64, Vec::new());
    for (seed, p) in env.runs() {
        for (k, trench) in [(0usize, false), (1, true)] {
            let mut s = scenarios::he_infantry(env.setup(seed)?, env.threads, p, trench)?;
            r.runs += 1;
            let tank = s.tanks[0].1;
            let w = &mut s.world;
            let lost0 = losses(w, 1 - p);
            let lip0 = if trench { lip_profile(w) } else { Vec::new() };
            let limit = w.tick() + 150 * hz(w);
            let mut he = 0u64;
            let mut done_at = None;
            while w.tick() < limit && done_at.is_none_or(|t| w.tick() < t + 15 * hz(w)) {
                w.step(&[]);
                he += w
                    .snapshot()
                    .armour_events
                    .iter()
                    .filter(|e| {
                        e.kind == ArmourEventKind::Fired as u8
                            && e.vehicle == tank.0
                            && e.component == LoadedSnap::He as u8
                    })
                    .count() as u64;
                if trench {
                    trench_hits += w
                        .snapshot()
                        .explosions
                        .iter()
                        .filter(|e| {
                            e.kind == BlastKind::Shell
                                && (e.pos[0] - TRENCH_X as f32).abs() <= 3.0
                                && (e.pos[1] - LANE_Y as f32).abs() <= 22.0
                        })
                        .count() as u64;
                }
                if env.verbose {
                    for e in &w.snapshot().explosions {
                        eprintln!(
                            "case 9 seed {seed} p{p} trench {trench} t {}: burst at {:?} (HE fired {he})",
                            w.tick(),
                            e.pos
                        );
                    }
                }
                if he >= 10 && done_at.is_none() {
                    done_at = Some(w.tick());
                    w.edit_vehicle(tank, |vs| vs.components[ComponentKind::Gun.index()] = 0);
                }
            }
            fired[k] += he;
            lost[k] += losses(w, 1 - p) - lost0;
            if trench {
                let lip1 = lip_profile(w);
                lip_lost += lip0
                    .iter()
                    .zip(&lip1)
                    .map(|(a, b)| f64::from(a - b).max(0.0))
                    .sum::<f64>()
                    / lip0.len().max(1) as f64;
                let mut run = 0;
                let mut best = 0;
                for h in &lip1 {
                    run = if *h <= 0.05 { run + 1 } else { 0 };
                    best = best.max(run);
                }
                gaps.push(best);
            }
        }
    }
    r.check(
        "trench casualties ≤ 40 % of open",
        lost[0] > 0 && lost[1] * 100 <= 40 * lost[0],
        format!(
            "trench {} vs open {} men lost ({} HE fired at the trench, {} in the open: \
             {:.2} vs {:.2} men per round)",
            lost[1],
            lost[0],
            fired[1],
            fired[0],
            lost[1] as f64 / fired[1].max(1) as f64,
            lost[0] as f64 / fired[0].max(1) as f64
        ),
    );
    let runs = gaps.len().max(1) as f64;
    r.row(
        "lip height lost per HE burst within 3 m of the trench",
        format!(
            "{:.3} m mean lip lost per run, {} bursts over {} runs: {:.4} m per burst",
            lip_lost / runs,
            trench_hits,
            gaps.len(),
            lip_lost / trench_hits.max(1) as f64
        ),
    );
    let opened = gaps.iter().filter(|g| **g >= 4).count();
    r.row(
        "hits to open a 4 m gap in the parapet",
        format!(
            "opened after 10 rounds in {opened} of {} runs (widest gap per run: {:?} m)",
            gaps.len(),
            gaps
        ),
    );
    Ok(r)
}

// ----------------------------------------------------------- case 10 --

fn case10(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(10, "mortar vs a closed tank");
    let mut bad = Vec::new();
    let (mut shells, mut tracks) = (0u64, 0u64);
    for (seed, p) in env.runs() {
        let mut s = scenarios::mortar_tank(env.setup(seed)?, env.threads, p)?;
        r.runs += 1;
        let m = s.squads[0];
        let tank = s.tanks[0].1;
        let w = &mut s.world;
        let before = vstate(w, tank).expect("tank");
        let pens0 = w.stats().penetrations;
        let ko0 = w.stats().knockouts;
        let target = pos(w, tank);
        order(
            w,
            p,
            vec![CommandKind::Barrage {
                squad: m,
                target,
                rounds: 20,
            }],
        );
        let limit = w.tick() + 180 * hz(w);
        let mut n = 0u64;
        while w.tick() < limit && n < 20 {
            w.step(&[]);
            n += w
                .snapshot()
                .explosions
                .iter()
                .filter(|e| e.kind == BlastKind::Shell)
                .count() as u64;
        }
        shells += n;
        let after = vstate(w, tank).expect("tank");
        let inner = [
            ComponentKind::Engine,
            ComponentKind::Gun,
            ComponentKind::TurretRing,
            ComponentKind::AmmoRack,
        ];
        let inner_hit = inner
            .iter()
            .any(|&k| after.component(k) != before.component(k));
        let seats = after
            .seats
            .iter()
            .zip(&before.seats)
            .any(|(a, b)| a.hp != b.hp);
        if w.stats().penetrations != pens0 || w.stats().knockouts != ko0 || inner_hit || seats {
            bad.push(format!("seed {seed} p{p}"));
        }
        tracks += [ComponentKind::TrackLeft, ComponentKind::TrackRight]
            .iter()
            .filter(|&&k| after.component(k) == 0)
            .count() as u64;
    }
    r.check(
        "no penetration, seat or internal component damage, no knock-out",
        bad.is_empty(),
        if bad.is_empty() {
            "none".to_string()
        } else {
            bad.join(", ")
        },
    );
    r.row(
        "tracks lost per shell",
        format!(
            "{tracks} tracks over {shells} shells ({:.3} per shell)",
            tracks as f64 / shells.max(1) as f64
        ),
    );
    Ok(r)
}

// ----------------------------------------------------------- case 11 --

fn case11(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(11, "enfilade");
    let mut lost = [0u32; 2];
    let mut men = [0u32; 2];
    for (seed, p) in env.runs() {
        for (k, end_on) in [(0usize, true), (1, false)] {
            let mut s = scenarios::enfilade(env.setup(seed)?, env.threads, p, end_on)?;
            r.runs += 1;
            let w = &mut s.world;
            let lost0 = losses(w, 1 - p);
            men[k] += members(w, s.squads[0]).len() as u32;
            let limit = w.tick() + 90 * hz(w);
            while w.tick() < limit {
                w.step(&[]);
            }
            lost[k] += losses(w, 1 - p) - lost0;
        }
    }
    r.row(
        "defender losses, tank off the trench's end (enfilade)",
        format!(
            "{} of {} ({})",
            lost[0],
            men[0],
            pct(lost[0] as f64, men[0] as f64)
        ),
    );
    r.row(
        "defender losses, tank in front of it",
        format!(
            "{} of {} ({})",
            lost[1],
            men[1],
            pct(lost[1] as f64, men[1] as f64)
        ),
    );
    Ok(r)
}

// ----------------------------------------------------------- case 12 --

/// Drives the tank from `center − dir·16 m` to `center + dir·16 m`.
/// Returns `(crossings started, bogged, ticks, arrived)`.
fn drive_across(
    w: &mut World,
    p: u8,
    tsq: SquadId,
    tank: SimId,
    center: FxVec2,
    dir: FxVec2,
) -> (u32, bool, u64, bool) {
    w.edit_vehicle(tank, |vs| {
        vs.bog = 0;
        vs.on_blocked = false;
        vs.crossing = false;
        vs.clear_ticks = u16::MAX;
    });
    let from = center - dir * Fx::from_int(16);
    let to = center + dir * Fx::from_int(16);
    w.place_vehicle(tank, from, Fx::atan2(dir.y, dir.x));
    let c0 = vstate(w, tank).map_or(0, |v| v.crossings);
    let t0 = w.tick();
    order(
        w,
        p,
        vec![CommandKind::Move {
            squad: tsq,
            target: to,
        }],
    );
    let limit = t0 + 60 * hz(w);
    loop {
        let vs = vstate(w, tank).expect("tank");
        if vs.bog > 0 {
            return (vs.crossings - c0, true, w.tick() - t0, false);
        }
        let idle = w
            .game()
            .unwrap()
            .squad(tsq)
            .is_some_and(|q| q.order == Order::Idle);
        if idle || w.tick() >= limit {
            let arrived = dist(pos(w, tank), to) < 4.0;
            return (vs.crossings - c0, false, w.tick() - t0, arrived);
        }
        w.step(&[]);
    }
}

fn case12(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(12, "trench crossing");
    let mut wrong_rolls = 0u64;
    let (mut n, mut bogs, mut exp, mut var) = (0u64, 0u64, 0f64, 0f64);
    let (mut cross_ticks, mut cross_n, mut open_ticks, mut open_n) = (0u64, 0u64, 0u64, 0u64);
    let mut per_angle: BTreeMap<i32, (u64, u64)> = BTreeMap::new();
    let (mut blocked, mut stuck, mut crater_n) = (0u32, 0u32, 0u32);
    for (seed, p) in env.runs() {
        let mut s = scenarios::crossing(env.setup(seed)?, env.threads, p, false)?;
        r.runs += 1;
        let (tsq, tank) = s.tanks[0];
        let w = &mut s.world;
        let bog_pct = {
            let st = w.match_setup().unwrap();
            st.vehicles[vstate(w, tank).unwrap().type_idx as usize]
                .mobility
                .bog_pct
        };
        let q = f64::from(bog_pct) / 100.0;
        for angle in [0, 30, 60] {
            let dir = dir_deg(180 + angle);
            let center = at(TRENCH_X, LANE_Y);
            for _ in 0..env.crossings {
                let (c, bogged, t, _) = drive_across(w, p, tsq, tank, center, dir);
                n += 1;
                if c != 1 {
                    wrong_rolls += 1;
                }
                bogs += bogged as u64;
                exp += q;
                var += q * (1.0 - q);
                let e = per_angle.entry(angle).or_default();
                e.0 += 1;
                e.1 += bogged as u64;
                if !bogged {
                    cross_ticks += t;
                    cross_n += 1;
                }
            }
            let (_, _, t, arrived) = drive_across(w, p, tsq, tank, at(TRENCH_X, LANE_Y - 150), dir);
            if arrived {
                open_ticks += t;
                open_n += 1;
            }
        }
        // The cratered trench (first seed only per faction would do; all
        // runs keep it simple and comparable).
        let mut s = scenarios::crossing(env.setup(seed)?, env.threads, p, true)?;
        let (tsq, tank) = s.tanks[0];
        let m = s.squads[0];
        let w = &mut s.world;
        order(
            w,
            p,
            vec![CommandKind::Barrage {
                squad: m,
                target: at(TRENCH_X, LANE_Y),
                rounds: 10,
            }],
        );
        let limit = w.tick() + 120 * hz(w);
        let mut shells = 0;
        while w.tick() < limit && shells < 10 {
            w.step(&[]);
            shells += w
                .snapshot()
                .explosions
                .iter()
                .filter(|e| e.kind == BlastKind::Shell)
                .count();
        }
        w.step(&[]);
        {
            let g = w.game().unwrap();
            let block = &g.vehicle_derived().block;
            for y in LANE_Y - 22..=LANE_Y + 22 {
                for dx in -3..=3 {
                    let pt = at(TRENCH_X + dx, y);
                    if block.blocked_at(w.terrain(), pt) {
                        blocked += 1;
                    }
                }
            }
        }
        for angle in [0, 30, 60] {
            for _ in 0..CRATERED_CROSSINGS {
                let (_, bogged, _, arrived) =
                    drive_across(w, p, tsq, tank, at(TRENCH_X, LANE_Y), dir_deg(180 + angle));
                crater_n += 1;
                if !bogged && !arrived {
                    stuck += 1;
                }
            }
        }
    }
    r.check(
        "exactly one bog roll per crossing",
        wrong_rolls == 0,
        format!("{wrong_rolls} of {n} crossings did not start exactly one"),
    );
    r.check(
        "bogs within 3σ of bog_pct",
        within_3s(bogs as f64, exp, var),
        format!("{bogs} of {n}, expected {exp:.1} (σ {:.1})", var.sqrt()),
    );
    let mean = |t: u64, k: u64| t as f64 / k.max(1) as f64 / 30.0;
    r.check(
        "crossing is slower than open ground",
        cross_n > 0 && open_n > 0 && mean(cross_ticks, cross_n) > mean(open_ticks, open_n),
        format!(
            "32 m across the trench {:.1} s, on open ground {:.1} s",
            mean(cross_ticks, cross_n),
            mean(open_ticks, open_n)
        ),
    );
    r.check(
        "the cratered trench stays crossable (no blocked node, no crossing stuck)",
        blocked == 0 && stuck == 0,
        format!("{blocked} blocked samples, {stuck} of {crater_n} crossings stuck"),
    );
    for (angle, (k, b)) in per_angle {
        r.row(
            format!("{angle}°"),
            format!("{b} bogged of {k} ({})", pct(b as f64, k as f64)),
        );
    }
    Ok(r)
}

// ----------------------------------------------------------- case 13 --

fn case13(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(13, "AT ditch");
    let (mut on_blocked, mut outcomes) = (0u32, BTreeMap::<&str, u32>::new());
    let mut satchels: Vec<Option<u32>> = Vec::new();
    for (seed, p) in env.runs() {
        let mut s = scenarios::at_ditch(env.setup(seed)?, env.threads, p)?;
        r.runs += 1;
        let (tsq, tank) = s.tanks[0];
        let w = &mut s.world;
        let goal = at(TRENCH_X - 40, LANE_Y);
        order(
            w,
            p,
            vec![CommandKind::Move {
                squad: tsq,
                target: goal,
            }],
        );
        let limit = w.tick() + 120 * hz(w);
        let mut was_blocked = false;
        while w.tick() < limit {
            w.step(&[]);
            if vstate(w, tank).is_some_and(|v| v.on_blocked) {
                was_blocked = true;
            }
            if w.game()
                .unwrap()
                .squad(tsq)
                .is_some_and(|q| q.order == Order::Idle)
            {
                break;
            }
        }
        on_blocked += was_blocked as u32;
        let key = if dist(pos(w, tank), goal) < 6.0 {
            "went round the ditch"
        } else {
            "stopped short (unreachable)"
        };
        *outcomes.entry(key).or_default() += 1;
        // Satchels on the ditch wall until a vehicle can cross straight
        // (the tank is taken off first: its MGs would kill the engineers).
        w.wreck_vehicle(tank);
        let open = |w: &World| {
            let g = w.game().unwrap();
            (LANE_Y - 10..=LANE_Y + 10).any(|y| {
                !g.vehicle_derived().block.segment_blocked(
                    w.terrain(),
                    at(TRENCH_X + 10, y),
                    at(TRENCH_X - 10, y),
                )
            })
        };
        let mut used = None;
        for k in 0..16u32 {
            if open(w) {
                used = Some(k);
                break;
            }
            let off = if k % 2 == 0 {
                (k / 2) as i32 * 2
            } else {
                -(k.div_ceil(2) as i32) * 2
            };
            let y = LANE_Y + off;
            let Some(eng) = scenarios::spawn(
                w,
                p,
                frontline_content::SquadRole::Engineer,
                at(TRENCH_X + 6, y),
                WEST_DIR,
            ) else {
                break;
            };
            // The squad leader carries the satchel: line him up with the
            // spot, 6 m from it.
            let lead = members(w, eng)
                .first()
                .map_or(at(TRENCH_X + 6, y), |&m| pos(w, m));
            let dy = lead.y - Fx::from_int(y);
            w.place_squad(
                eng,
                FxVec2::new(Fx::from_int(TRENCH_X + 6), Fx::from_int(y) - dy),
                WEST_DIR,
            );
            order(
                w,
                p,
                vec![CommandKind::Throw {
                    units: members(w, eng),
                    item: ItemKind::Satchel,
                    target: at(TRENCH_X, y),
                }],
            );
            let lim = w.tick() + 12 * hz(w);
            let mut burst = false;
            while w.tick() < lim && !burst {
                w.step(&[]);
                burst = w
                    .snapshot()
                    .explosions
                    .iter()
                    .any(|e| e.kind == BlastKind::Satchel);
                if env.verbose {
                    for e in w.snapshot().explosions.iter() {
                        let (gx, gy) = w
                            .terrain()
                            .nearest_grid(at(e.pos[0] as i32, e.pos[1] as i32));
                        eprintln!(
                            "case 13 seed {seed} p{p} satchel {k} at {:?}, dug {} dm",
                            e.pos,
                            w.terrain().dug_dm(gx, gy)
                        );
                    }
                }
            }
            w.step(&[]);
            if env.verbose && !burst {
                eprintln!(
                    "case 13 seed {seed} p{p} satchel {k}: no burst (throws {})",
                    w.stats().throws
                );
            }
        }
        if used.is_none() && open(w) {
            used = Some(16);
        }
        satchels.push(used);
    }
    r.check(
        "the tank never puts a track sample on a blocked node",
        on_blocked == 0,
        format!("{on_blocked} runs with a sample on blocked ground"),
    );
    for (k, v) in outcomes {
        r.row(format!("ordered across: {k}"), format!("{v} of {}", r.runs));
    }
    let opened: Vec<u32> = satchels.iter().flatten().copied().collect();
    r.row(
        "satchels needed to open a straight vehicle path",
        format!(
            "{:?} (none: not open after 16); mean {:.1}",
            satchels,
            opened.iter().sum::<u32>() as f64 / opened.len().max(1) as f64
        ),
    );
    Ok(r)
}

const WEST_DIR: FxVec2 = scenarios::WEST;

// ----------------------------------------------------------- case 14 --

// Wall-clock time here only measures; it never feeds the sim.
#[allow(clippy::disallowed_methods)]
fn case14(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(
        14,
        "stress (limits lifted: 50 vehicles per player, pop cap 2000)",
    );
    let seed = env.seeds.first().copied().unwrap_or(1);
    let mut s = scenarios::stress(env.setup(seed)?, 3)?;
    r.runs = 1;
    let w = &mut s.world;
    let soldiers = w.entities().soldier_count();
    let vehicles = s.tanks.len();
    let (mut tick, mut mv, mut wp, mut bal, mut dmg, mut per) = (
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let (mut int, mut sts, mut hsh, mut snap) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut peak_box, mut peak_exp) = (0u64, 0u32);
    let mut seq = 0u32;
    for t in 0..120 * hz(w) {
        let mut cmds = Vec::new();
        if t % 600 == 0 {
            for (k, &(sq, v)) in s.tanks.iter().enumerate() {
                if !alive(w, v) {
                    continue;
                }
                seq += 1;
                let tx = 200 + ((k as i32 * 37 + t as i32 / 6) % 600);
                let ty = 200 + ((k as i32 * 53 + t as i32 / 9) % 600);
                let p = w.entities().owner()[v.index()].0;
                cmds.push(cmd(
                    p,
                    seq,
                    CommandKind::AttackMove {
                        squad: sq,
                        target: at(tx, ty),
                    },
                ));
            }
            for (k, &sq) in s.squads.iter().enumerate() {
                let Some(q) = w.game().unwrap().squad(sq) else {
                    continue;
                };
                seq += 1;
                let tx = 300 + ((k as i32 * 41 + t as i32 / 7) % 400);
                let ty = 300 + ((k as i32 * 29 + t as i32 / 5) % 400);
                cmds.push(cmd(
                    q.owner.0,
                    seq,
                    CommandKind::AttackMove {
                        squad: sq,
                        target: at(tx, ty),
                    },
                ));
            }
        }
        let t0 = Instant::now();
        w.step(&cmds);
        tick.push(ns(t0.elapsed()));
        let st = w.stage_times();
        int.push(ns(st.intent));
        sts.push(ns(st.status));
        hsh.push(ns(st.world_hash));
        snap.push(ns(st.snapshot));
        mv.push(ns(st.movement));
        wp.push(ns(st.weapons));
        bal.push(ns(st.ballistics));
        dmg.push(ns(st.damage));
        per.push(ns(st.perception));
        peak_exp = peak_exp.max(st.path_expansions);
        peak_box = peak_box.max(w.stats().armour_box_peak);
    }
    let all = Summary::from_nanos(&mut tick);
    r.check(
        "sim tick P99 < 10 ms on 3 threads",
        all.p99_ms < 10.0,
        format!("p99 {:.3} ms, max {:.3} ms", all.p99_ms, all.max_ms),
    );
    r.row(
        "units",
        format!("{vehicles} vehicles, {soldiers} soldiers at the start"),
    );
    for (name, v) in [
        ("intent (paths, AI)", &mut int),
        ("perception", &mut per),
        ("movement", &mut mv),
        ("weapons", &mut wp),
        ("ballistics", &mut bal),
        ("damage", &mut dmg),
        ("status", &mut sts),
        ("hash", &mut hsh),
        ("snapshot", &mut snap),
    ] {
        let s = Summary::from_nanos(v);
        r.row(
            format!("stage {name}"),
            format!("p99 {:.3} ms, max {:.3} ms", s.p99_ms, s.max_ms),
        );
    }
    r.row(
        "whole tick",
        format!(
            "mean {:.3} ms, p95 {:.3}, p99 {:.3}, max {:.3}",
            all.mean_ms, all.p95_ms, all.p99_ms, all.max_ms
        ),
    );
    r.row("max armour_box_tests in one tick", peak_box.to_string());
    r.row("max path expansions in one tick", peak_exp.to_string());
    r.row(
        "knocked out, penetrations",
        format!("{}, {}", w.stats().knockouts, w.stats().penetrations),
    );
    Ok(r)
}

// --------------------------------------------------------------- run --

type CaseFn = fn(&Env) -> Result<CaseReport>;

/// Runs the cases (see the module docs).
// Wall-clock time here only measures; it never feeds the sim.
#[allow(clippy::disallowed_methods)]
pub fn armour_test(args: &ArmourArgs) -> Result<ArmourTestReport> {
    let dir = mods_dir(args.mods.as_deref())?;
    let (mut defs, _) = crate::overrides::load_defs(Some(&dir), &args.rules_override)?;
    let rules = defs
        .add_rules_override(&dir.join("tests/rules_test_unlock.toml"))
        .map_err(|e| anyhow!("{e}"))?;
    let env = Env {
        defs,
        rules,
        threads: args.threads.max(1),
        seeds: (0..args.seeds as u64)
            .map(|k| args.first_seed + k)
            .collect(),
        crossings: args.crossings,
        verbose: args.verbose,
    };
    let all: [(u32, CaseFn); 14] = [
        (1, case1),
        (2, case2),
        (3, case3),
        (4, case4),
        (5, case5),
        (6, case6),
        (7, case7),
        (8, case8),
        (9, case9),
        (10, case10),
        (11, case11),
        (12, case12),
        (13, case13),
        (14, case14),
    ];
    let start = Instant::now();
    let mut cases = Vec::new();
    for (n, f) in all {
        let wanted = if args.cases.is_empty() {
            n != 14 || args.stress
        } else {
            args.cases.contains(&n) && (n != 14 || args.stress || args.cases.contains(&14))
        };
        if !wanted {
            continue;
        }
        let t = Instant::now();
        let mut r = f(&env)?;
        r.wall_s = round3(t.elapsed().as_secs_f64());
        eprintln!("case {n} done in {:.1} s", r.wall_s);
        cases.push(r);
    }
    let pass = cases.iter().all(|c| c.pass());
    Ok(ArmourTestReport {
        seeds: env.seeds.clone(),
        threads: env.threads,
        stress: args.stress,
        cases,
        pass,
        wall_s: round3(start.elapsed().as_secs_f64()),
    })
}

/// Human summary of a report.
pub fn summary_text(r: &ArmourTestReport) -> String {
    let mut out = format!(
        "armour-test: seeds {:?} × both factions, {} thread(s){}\n",
        r.seeds,
        r.threads,
        if r.stress {
            ", with the stress case"
        } else {
            ""
        }
    );
    for c in &r.cases {
        out += &format!(
            "\ncase {} — {} ({} runs, {:.1} s): {}\n",
            c.case,
            c.name,
            c.runs,
            c.wall_s,
            if c.pass() { "pass" } else { "FAIL" }
        );
        for k in &c.checks {
            out += &format!(
                "  [{}] {}: {}\n",
                if k.pass { "ok" } else { "FAIL" },
                k.name,
                k.detail
            );
        }
        for row in &c.rows {
            match &row.target {
                Some(t) => out += &format!("  report: {}: {} (target: {t})\n", row.name, row.value),
                None => out += &format!("  report: {}: {}\n", row.name, row.value),
            }
        }
    }
    out += &format!(
        "\n{} in {:.1} s",
        if r.pass {
            "all hard checks pass"
        } else {
            "SOME HARD CHECKS FAILED"
        },
        r.wall_s
    );
    out
}
