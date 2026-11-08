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
use frontline_sim::gunnery;
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
    /// Only these cases (1–15), e.g. `--case 1 --case 7`.
    #[arg(long = "case", value_name = "N")]
    pub cases: Vec<u32>,
    /// Also run case 14, the stress case (lifts the vehicle limit and the
    /// pop cap).
    #[arg(long)]
    pub stress: bool,
    /// Case 12's crossings per angle per run.
    #[arg(long, value_name = "N", default_value_t = CROSSINGS)]
    pub crossings: u32,
    /// Bare `--json`: print the report as JSON instead of text. With a
    /// file, `--json FILE`: print the text as usual and also write the
    /// checks and report rows as one JSON object (`docs/modding/tuning.md`).
    #[arg(long, value_name = "FILE", num_args = 0..=1)]
    pub json: Option<Option<PathBuf>>,
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
            json: None,
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
    /// The measured number the check judges, when it is one (`--json`).
    pub value: Option<f64>,
    /// The bound it is judged against, as text (`--json`).
    pub limit: Option<String>,
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
            value: None,
            limit: None,
        });
    }
    /// A check with its measured value and bound (`--json`).
    fn check_v(
        &mut self,
        name: impl Into<String>,
        pass: bool,
        value: f64,
        limit: impl Into<String>,
        detail: impl Into<String>,
    ) {
        self.checks.push(Check {
            name: name.into(),
            pass,
            detail: detail.into(),
            value: Some(value),
            limit: Some(limit.into()),
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
mod s8b;
mod s9;

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
    r.check_v(
        "every logged hit's outcome equals the rule (penetrates, pen_chance_ppm)",
        mism == 0 && total_hits > 0,
        mism as f64,
        "0 mismatches",
        format!("{total_hits} hits, {mism} mismatches"),
    );
    for (&(range, angle), st) in &pairs {
        r.check_v(
            format!("{range} m, {angle}°: at least 100 hits"),
            st.hits >= 100,
            st.hits as f64,
            ">= 100",
            format!("{} hits", st.hits),
        );
        r.check_v(
            format!("{range} m, {angle}°: penetrations within 3σ of Σp"),
            within_3s(st.pens as f64, st.expected, st.var),
            st.pens as f64,
            format!("{:.1} ± {:.1}", st.expected, 3.0 * st.var.sqrt()),
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
                    if h.part == BoxPart::HULL {
                        hull += 1;
                    } else {
                        turret += 1;
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
    r.check_v(
        "hull hits ≤ 2 % of hits on the hull-down tank",
        hits > 0 && hull * 100 <= 2 * hits,
        hull as f64 * 100.0 / hits.max(1) as f64,
        "<= 2 %",
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
    /// S3b, 3b: the Sherman's main-gun shots and the worst miss of its laid
    /// line from the Panzer IV's visible-mass centroid at the target (m).
    aim_shots: u64,
    aim_worst_m: f64,
    /// Knock-outs by cause (`ArmourEvent::component`: crew, rack, fire),
    /// and ticks to each side's first main-gun shot (S3b baseline).
    ko_cause: [u32; 4],
    first_shot: [Vec<u64>; 2],
    /// S3c: abandonments by trigger (`bail::AB_*`, index 0 unused).
    abandoned: [u32; 7],
    /// S3c (3f): penetrations by (shooter player, part), and main-gun
    /// shots fired (Panzer IV, Sherman).
    pens: BTreeMap<(u8, BoxPart), u64>,
    shots: [u32; 2],
}

/// S3b 3b check: how far (m, at the target's range) the Sherman's laid
/// gun line passes from the drop-compensated solution to the Panzer IV's
/// visible-mass centroid (a still target, so no lead).
fn aim_miss_m(w: &World, sh: SimId, pz: SimId) -> Option<f64> {
    let g = w.game()?;
    let d = g.vehicle_derived();
    let (fs, fp) = (d.frame(sh)?, d.frame(pz)?);
    let vs = g.vehicle(sh)?;
    let s = w.match_setup()?;
    let v = &s.vehicles[vs.type_idx as usize];
    let main = v.main_gun()?;
    let muzzle = gunnery::main_muzzle(fs, vs, main);
    let centre = gunnery::mass_centre(muzzle, fp);
    let speed = main
        .ammo
        .first()
        .and_then(|a| s.ammo.get(a.ammo as usize))
        .map(|a| a.speed)?;
    let (want, _) = gunnery::solution(muzzle, centre, FxVec2::ZERO, speed.into());
    let have = fs
        .hull
        .axes
        .apply(gunnery::gun_local(vs.turret_yaw, vs.gun_pitch));
    let f = |v: FxVec3| v.to_f32().map(f64::from);
    let (a, b) = (f(want), f(have));
    // The small angle between them: |a × b| / (|a| |b|) (acos near 1 is
    // ill-conditioned for unit vectors rounded to Q16.16).
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let len = |v: [f64; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let sin = len(cross) / (len(a) * len(b)).max(1e-9);
    let range = f64::from((centre - muzzle).length().to_f32());
    Some(sin.clamp(0.0, 1.0).asin() * range)
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
            ("e 150 m, Panzer IV on the flank", Duel::PzFlank150),
            ("f 600 m frontal", Duel::Frontal600),
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
                    } else if e.kind == ArmourEventKind::Abandoned as u8 {
                        if let Some(c) = st.abandoned.get_mut(e.component as usize) {
                            *c += 1;
                        }
                    } else if e.kind == ArmourEventKind::Fired as u8 {
                        let side = (e.vehicle == sh.0) as usize;
                        st.shots[side] += 1;
                        if shot[side].is_none() {
                            shot[side] = Some(w.tick() - start);
                        }
                        if kind == Duel::Angled300
                            && e.vehicle == sh.0
                            && let Some(m) = aim_miss_m(w, sh, pz)
                        {
                            st.aim_shots += 1;
                            st.aim_worst_m = st.aim_worst_m.max(m);
                        }
                    }
                }
                for h in w.take_hit_log() {
                    *st.hits.entry((h.player, h.part)).or_default() += 1;
                    if h.result == frontline_sim::armour::PenResult::Penetration {
                        *st.pens.entry((h.player, h.part)).or_default() += 1;
                    }
                }
                let dis = w.snapshot().armour_events.iter().any(|e| {
                    e.kind == ArmourEventKind::Disabled as u8
                        || e.kind == ArmourEventKind::KnockedOut as u8
                        || e.kind == ArmourEventKind::Abandoned as u8
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
            let n =
                |m: &BTreeMap<(u8, BoxPart), u64>, part| m.get(&(p, part)).copied().unwrap_or(0);
            format!(
                "{} hull / {} turret (penetrated {} / {})",
                n(&st.hits, BoxPart::HULL),
                n(&st.hits, BoxPart(1)),
                n(&st.pens, BoxPart::HULL),
                n(&st.pens, BoxPart(1))
            )
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
                "destroyed: crew {}, ammo rack {}, fire {}, ammo fire {}; S3c abandoned: \
                 ammo fire {}, engine fire {}, crew {}, gun and mobility {}, under fire {}; \
                 first main-gun shot (median) Panzer IV {}, Sherman {}; shots fired {} / {}",
                st.ko_cause[0],
                st.ko_cause[1],
                st.ko_cause[2],
                st.ko_cause[3],
                st.abandoned[1],
                st.abandoned[2],
                st.abandoned[3],
                st.abandoned[4],
                st.abandoned[5],
                med(&st.first_shot[0]),
                med(&st.first_shot[1]),
                st.shots[0],
                st.shots[1]
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
        let h = st.hits.get(&(1, BoxPart::HULL)).copied().unwrap_or(0);
        let all = h + st.hits.get(&(1, BoxPart(1))).copied().unwrap_or(0);
        r.check_v(
            "3c (both parts penetrated at 150 m on the flank): ≥ 40 % of the Sherman's hits on the hull",
            all > 0 && h * 100 >= 40 * all,
            h as f64 * 100.0 / all.max(1) as f64,
            ">= 40 %",
            format!("{h} of {all}"),
        );
    }
    if let Some(st) = stats.get("b 300 m, Panzer IV angled 30°") {
        // S3b: the centre of visible mass, not the weakest plate.
        r.check_v(
            "3b: the Sherman's gunner aims within 0.3 m of the angled Panzer IV's visible-mass \
             centroid",
            st.aim_hull > 0 && st.aim_turret == 0 && st.aim_shots > 0 && st.aim_worst_m <= 0.3,
            st.aim_worst_m,
            "<= 0.3 m",
            format!(
                "ticks aiming at the centroid {}, at the turret {}; {} shots, worst laid miss \
                 {:.2} m",
                st.aim_hull, st.aim_turret, st.aim_shots, st.aim_worst_m
            ),
        );
    }
    // S3b targets (report): the flanker wins.
    for (name, flanker) in [
        ("c 150 m, Sherman on the flank", 1usize),
        ("e 150 m, Panzer IV on the flank", 0),
    ] {
        if let Some(st) = stats.get(name) {
            let won = if flanker == 1 { st.sh_wins } else { st.pz_wins };
            r.target(
                format!("3{name}: the flanker's wins"),
                format!("{won} of {}", st.runs),
                "≥ 7 of 10",
            );
        }
    }
    if let Some(st) = stats.get("a 300 m frontal") {
        r.target(
            "3a 300 m frontal: Panzer IV wins",
            format!("{} of {}", st.pz_wins, st.runs),
            "6–8 of 10",
        );
    }
    // S3c (S3b-38 (A)): 600 m frontal. The target is written because seeds
    // 1-10 support it (Panzer IV 14 of 20 once shells flew their full
    // range): the M61 is just under the Panzer IV H's 80 mm hull front at
    // 600 m, the PzGr 39 still beats the Sherman's glacis.
    if let Some(st) = stats.get("f 600 m frontal") {
        r.target(
            "3f 600 m frontal: Panzer IV wins",
            format!("{} of {}", st.pz_wins, st.runs),
            "≥ 6 of 10",
        );
        r.row(
            "3f 600 m frontal (S3b-38): wins",
            format!(
                "Panzer IV {} ({}), Sherman {} ({}), neither {} of {}",
                st.pz_wins,
                pct(f64::from(st.pz_wins), f64::from(st.runs)),
                st.sh_wins,
                pct(f64::from(st.sh_wins), f64::from(st.runs)),
                st.runs - st.pz_wins - st.sh_wins,
                st.runs
            ),
        );
    }
    let ko: [u32; 4] = stats.values().fold([0; 4], |mut a, st| {
        for (k, n) in st.ko_cause.iter().enumerate() {
            a[k] += n;
        }
        a
    });
    let all: u32 = ko.iter().sum();
    r.target(
        "case 3 pooled knock-outs: rack detonations",
        format!(
            "{} of {all} ({}); crew {}, engine fire {}, ammo fire {}",
            ko[1],
            pct(f64::from(ko[1]), f64::from(all)),
            ko[0],
            ko[2],
            ko[3]
        ),
        "≤ 50 %",
    );
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
                // S3b Fix (review L2): "moving" as the sim's launcher rule
                // judges it, the ~1 s average velocity (`launcher_target_ok`),
                // not the instantaneous speed: a tank pulling away from a
                // stop is still "still" to the gunner for a moment.
                let (tpos, tspeed) = (
                    pos(w, tank),
                    vstate(w, tank).map_or(Fx::ZERO, |v| v.avg_vel.length()),
                );
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
                        || events(w, ArmourEventKind::Abandoned, tank) > 0
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
    r.check_v(
        "at least one rocket hit per flank and front run",
        no_hit.is_empty(),
        no_hit.len() as f64,
        "0 runs without a hit",
        if no_hit.is_empty() {
            "every run".to_string()
        } else {
            format!("no hit in: {}", no_hit.join(", "))
        },
    );
    r.check_v(
        "no launcher shot at a moving tank beyond engage_moving",
        violations == 0,
        violations as f64,
        "0 shots",
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

/// S3b: the suppress line along the staged north–south trench (30 m
/// about its middle, inside `combat.suppress.line_max_m`).
fn trench_suppress_line() -> Vec<FxVec2> {
    vec![at(TRENCH_X, LANE_Y - 15), at(TRENCH_X, LANE_Y + 15)]
}

/// Case 7 (S3b): the tank on Suppress stops this far east of the trench
/// (m), inside `ai.assault.support_tank_m` and short of the 30 m HE
/// clearance from the escort when it goes in.
const SUPPRESS_GAP_M: i32 = 60;

/// Case 5 tallies for one variant.
#[derive(Default)]
struct HoldStat {
    hurt: Vec<String>,
    died: u32,
    lost: u32,
    men: u32,
    pinned: u64,
    ducked: u64,
    living: u64,
    tank_rounds: u64,
    up: u64,
}

fn case5(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(
        5,
        "lone tank vs dug-in rifles, no AT (5); 5b: the tank on Suppress at the trench",
    );
    let mut st = [HoldStat::default(), HoldStat::default()];
    for (seed, p) in env.runs() {
        for (k, suppress) in [(0usize, false), (1, true)] {
            let a = &mut st[k];
            let mut s = scenarios::lone_tank(env.setup(seed)?, env.threads, p, false, false)?;
            r.runs += 1;
            let rifles = s.squads[0];
            let tank = s.tanks[0].1;
            let w = &mut s.world;
            let before = vstate(w, tank).expect("tank");
            let men0 = members(w, rifles).len() as u32;
            let lost0 = losses(w, 1 - p);
            if suppress {
                // S3b (owner question S3b-11 (A)): what a player would do.
                order(
                    w,
                    p,
                    vec![CommandKind::SuppressUnits {
                        units: vec![tank],
                        line: trench_suppress_line(),
                    }],
                );
            }
            let limit = w.tick() + 180 * hz(w);
            while w.tick() < limit {
                w.step(&[]);
                a.tank_rounds += w
                    .snapshot()
                    .shots
                    .iter()
                    .filter(|sh| sh.shooter == tank)
                    .count() as u64;
                for m in members(w, rifles) {
                    let c = &w.entities().combat()[m.index()];
                    a.living += 1;
                    a.pinned += c.pinned as u64;
                    a.ducked += c.ducked as u64;
                    a.up += (c.in_trench && !c.pinned && c.height == c.fire_height) as u64;
                }
            }
            match vstate(w, tank) {
                Some(after) if alive(w, tank) => {
                    if after
                        .seats
                        .iter()
                        .zip(&before.seats)
                        .any(|(x, y)| x.hp != y.hp)
                        || after.components != before.components
                    {
                        a.hurt.push(format!("seed {seed} p{p}"));
                    }
                }
                _ => a.died += 1,
            }
            a.lost += losses(w, 1 - p) - lost0;
            a.men += men0;
        }
    }
    let hurt: Vec<String> = st.iter().flat_map(|a| a.hurt.iter().cloned()).collect();
    r.check_v(
        "no component or seat damage from small arms",
        hurt.is_empty(),
        hurt.len() as f64,
        "0",
        if hurt.is_empty() {
            "none".to_string()
        } else {
            hurt.join(", ")
        },
    );
    let died: u32 = st.iter().map(|a| a.died).sum();
    r.check_v(
        "the tank survives every run",
        died == 0,
        died as f64,
        "0 lost",
        format!("{died} lost"),
    );
    for (name, a) in [("5", &st[0]), ("5b", &st[1])] {
        r.row(
            format!("{name}: defender losses"),
            format!(
                "{} of {} men ({})",
                a.lost,
                a.men,
                pct(a.lost as f64, a.men as f64)
            ),
        );
        r.row(
            format!("{name}: share of defender time pinned, ducked"),
            format!(
                "{}, {}",
                pct(a.pinned as f64, a.living as f64),
                pct(a.ducked as f64, a.living as f64)
            ),
        );
        r.row(
            format!("{name}: tank rounds fired (gun and MGs), defender time up on the fire step"),
            format!("{}, {}", a.tank_rounds, pct(a.up as f64, a.living as f64)),
        );
    }
    let b = &st[1];
    let met = b.pinned + b.ducked > 0 && b.lost > 0;
    r.target(
        "T9: the tank on Suppress pins men in a trench",
        format!(
            "pinned or ducked {}, kills {}: {}",
            pct((b.pinned + b.ducked) as f64, b.living as f64),
            b.lost,
            if met { "met" } else { "missed" }
        ),
        "> 0 % of man-time pinned or ducked and kills > 0",
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

/// S3b Tune, case 7: the rifles close into the trench at the latest this
/// long after their first grenade wave (s).
const CLOSE_IN_S: u64 = 10;

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
    /// S3b: man-ticks ducked.
    def_ducked: u64,
    def_up: u64,
    def_supp: u64,
}

fn case7(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(
        7,
        "tank-supported assault: the tank leads its escort to 60 m, then suppresses \
         the trench while the rifles go in (7, S3b), without the tank (7b), the tank \
         leading its escort to 25 m with no suppress order (7c, the S3 run)",
    );
    let mut st = [
        AssaultStat::default(),
        AssaultStat::default(),
        AssaultStat::default(),
    ];
    for (seed, p) in env.runs() {
        for (k, with_tank, suppress) in [(0usize, true, true), (1, false, false), (2, true, false)]
        {
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
            // S3b (owner question S3b-11 (A)), what a player would do: the
            // tank leads its escort to SUPPRESS_GAP_M, then suppresses the
            // trench while the rifles go in with grenades.
            let goal = if suppress {
                at(TRENCH_X + SUPPRESS_GAP_M, LANE_Y)
            } else {
                goal
            };
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
            // S3b Tune: in 7 and 7b the rifles close into the trench once
            // their grenades are out (or CLOSE_IN_S after the first wave),
            // and the tank lifts its Suppress when their lead is within the
            // AI's `ai.assault.lift_fire_m` of the trench (fortify-test's
            // suppressed assault does the same). 7c keeps the S3 run.
            let close_in = k != 2;
            let lift_x = Fx::from_int(
                TRENCH_X + w.match_setup().unwrap().rules.ai.assault.lift_fire_m as i32,
            );
            let (mut first_wave, mut closing, mut lifted) = (None, false, !suppress);
            while w.tick() < limit {
                if close_in && released {
                    let lead_x = w
                        .game()
                        .unwrap()
                        .squad(att)
                        .map(|q| (q.nav.phase, pos(w, q.lead(w.entities())).x));
                    let near = lead_x.is_some_and(|(ph, x)| {
                        ph == frontline_sim::NavPhase::Holding || x <= throw_x
                    });
                    if near && first_wave.is_none() {
                        first_wave = Some(w.tick());
                    }
                    let frags_left = members(w, att)
                        .iter()
                        .any(|m| w.entities().kit()[m.index()].items[ItemKind::Frag.index()] > 0);
                    if !closing
                        && first_wave
                            .is_some_and(|t| !frags_left || w.tick() >= t + CLOSE_IN_S * hz(w))
                    {
                        closing = true;
                        order(
                            w,
                            p,
                            vec![CommandKind::Move {
                                squad: att,
                                target: at(TRENCH_X, LANE_Y + 6),
                            }],
                        );
                        continue;
                    }
                    if !lifted
                        && let Some((tsq, _)) = tank
                        && lead_x.is_some_and(|(_, x)| x <= lift_x)
                    {
                        lifted = true;
                        order(w, p, vec![CommandKind::Stop { squad: tsq }]);
                        continue;
                    }
                }
                if !released && let Some((tsq, _)) = tank {
                    let arrived = w
                        .game()
                        .unwrap()
                        .squad(tsq)
                        .is_none_or(|q| q.order == Order::Idle);
                    if arrived || w.tick() > start + 90 * hz(w) {
                        released = true;
                        let mut kinds = vec![CommandKind::Move {
                            squad: att,
                            target: at(TRENCH_X + 25, LANE_Y + 6),
                        }];
                        if suppress && let Some((_, tv)) = tank {
                            kinds.push(CommandKind::SuppressUnits {
                                units: vec![tv],
                                line: trench_suppress_line(),
                            });
                        }
                        order(w, p, kinds);
                        continue;
                    }
                }
                let close = released
                    && w.game().unwrap().squad(att).is_some_and(|q| {
                        q.nav.phase == frontline_sim::NavPhase::Holding
                            || pos(w, q.lead(w.entities())).x <= throw_x
                    });
                if close && !closing && (w.tick() - start).is_multiple_of(hz(w)) {
                    throw_wave(w, p, att);
                } else {
                    w.step(&[]);
                }
                let owner = w.entities().owner();
                for m in members(w, def) {
                    let c = &w.entities().combat()[m.index()];
                    st[k].def_ticks += 1;
                    st[k].def_pinned += c.pinned as u64;
                    st[k].def_ducked += c.ducked as u64;
                    st[k].def_up += (c.in_trench && !c.pinned && c.height == c.fire_height) as u64;
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
    let [t, b, c] = &st;
    r.row(
        "7c tank leads, no suppress (S3 run): attacker / defender losses",
        format!(
            "{} / {} of {} ({})",
            c.att_lost,
            c.def_lost,
            c.def_men,
            pct(c.def_lost as f64, c.def_men as f64)
        ),
    );
    r.row(
        "7 with tank on Suppress: attacker / defender losses",
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
    for (name, a) in [("7", t), ("7b", b), ("7c", c)] {
        r.row(
            format!("{name}: defender posture (share of man-time)"),
            format!(
                "pinned {}, ducked {}, up on the fire step {}, mean suppression {:.1} points",
                pct(a.def_pinned as f64, a.def_ticks as f64),
                pct(a.def_ducked as f64, a.def_ticks as f64),
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
                .map_or(Fx::ZERO, |h| h.blast.blast_radius.into());
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
    r.check_v(
        "a satchel burst with a track anchor inside its blast radius destroys that track",
        failures.is_empty(),
        failures.len() as f64,
        "0 failures",
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
    // S3b Tune (owner question S3b-22 (A)): the lip lost within 1 m of
    // each burst (along the trench) that lands within 3 m of it, measured
    // across that tick: the local effect the 41 m mean dilutes.
    let (mut local_lost, mut local_bursts) = (0f64, 0u64);
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
            let mut prev = lip0.clone();
            let mut seen: std::collections::BTreeMap<
                u32,
                ([f32; 4], frontline_sim::Stance, bool, u8),
            > = std::collections::BTreeMap::new();
            while w.tick() < limit && done_at.is_none_or(|t| w.tick() < t + 15 * hz(w)) {
                w.step(&[]);
                if trench {
                    let near: Vec<f32> = w
                        .snapshot()
                        .explosions
                        .iter()
                        .filter(|e| {
                            e.kind == BlastKind::Shell
                                && (e.pos[0] - TRENCH_X as f32).abs() <= 3.0
                                && (e.pos[1] - LANE_Y as f32).abs() <= 20.0
                        })
                        .map(|e| e.pos[1])
                        .collect();
                    if !near.is_empty() {
                        let now = lip_profile(w);
                        for y in near {
                            // Profile samples are 1 m apart from LANE_Y - 20.
                            let (mut sum, mut n) = (0f64, 0u32);
                            for (k, (a, b)) in prev.iter().zip(&now).enumerate() {
                                let sy = (LANE_Y - 20 + k as i32) as f32;
                                if (sy - y).abs() <= 1.0 {
                                    sum += f64::from(a - b).max(0.0);
                                    n += 1;
                                }
                            }
                            local_lost += sum / f64::from(n.max(1));
                            local_bursts += 1;
                        }
                        prev = now;
                    }
                }
                if env.verbose {
                    // Diagnostics: the last state of every man of the target
                    // squad, printed when he drops out of the snapshot.
                    let snap = w.snapshot();
                    let mut now = std::collections::BTreeMap::new();
                    for (i, id) in snap.soldier_ids.iter().enumerate() {
                        if snap.soldier_owner[i] == 1 - p {
                            now.insert(
                                id.0,
                                (
                                    snap.soldiers[i],
                                    snap.soldier_stance[i],
                                    snap.soldier_ducked[i],
                                    snap.soldier_suppression[i],
                                ),
                            );
                        }
                    }
                    for (id, st) in &seen {
                        if !now.contains_key(id) {
                            eprintln!(
                                "case 9 seed {seed} p{p} trench {trench} t {}: man {id} died at ({:.1}, {:.1}, z {:.2}) stance {:?} ducked {} supp {}",
                                w.tick(),
                                st.0[0],
                                st.0[1],
                                st.0[2],
                                st.1,
                                st.2,
                                st.3
                            );
                        }
                    }
                    seen = now;
                }
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
    r.check_v(
        "trench casualties ≤ 40 % of open",
        lost[0] > 0 && lost[1] * 100 <= 40 * lost[0],
        lost[1] as f64 * 100.0 / lost[0].max(1) as f64,
        "<= 40 %",
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
    r.target(
        "T14: lip lost within 1 m of each HE burst within 3 m of the trench (S3b-22 (A))",
        format!(
            "{:.3} m per burst over {local_bursts} bursts",
            local_lost / local_bursts.max(1) as f64
        ),
        "≥ 0.05 m",
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
            vec![CommandKind::FireMission {
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
    r.check_v(
        "no penetration, seat or internal component damage, no knock-out",
        bad.is_empty(),
        bad.len() as f64,
        "0",
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
            vec![CommandKind::FireMission {
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
    r.check_v(
        "exactly one bog roll per crossing",
        wrong_rolls == 0,
        wrong_rolls as f64,
        "0 crossings",
        format!("{wrong_rolls} of {n} crossings did not start exactly one"),
    );
    r.check_v(
        "bogs within 3σ of bog_pct",
        within_3s(bogs as f64, exp, var),
        bogs as f64,
        format!("{exp:.1} ± {:.1}", 3.0 * var.sqrt()),
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
    r.check_v(
        "the cratered trench stays crossable (no blocked node, no crossing stuck)",
        blocked == 0 && stuck == 0,
        (blocked + stuck) as f64,
        "0",
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
    r.check_v(
        "the tank never puts a track sample on a blocked node",
        on_blocked == 0,
        on_blocked as f64,
        "0 runs",
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

// ----------------------------------------------------------- case 15 --

/// A flat staging world for case 15 at `seed`, with `edit` on the setup.
fn stage15(env: &Env, seed: u64, edit: impl FnOnce(&mut MatchSetup)) -> Result<World> {
    let mut s = env.setup(seed)?;
    scenarios::staging(&mut s, true);
    edit(&mut s);
    World::new_match(s, env.threads).map_err(|e| anyhow!("{e:?}"))
}

/// Steps until `f` holds or `secs` pass; the ticks taken, if it held.
fn until(w: &mut World, secs: u64, mut f: impl FnMut(&World) -> bool) -> Option<u64> {
    let start = w.tick();
    let end = start + secs * hz(w);
    while w.tick() < end {
        if f(w) {
            return Some(w.tick() - start);
        }
        w.step(&[]);
    }
    f(w).then(|| w.tick() - start)
}

/// The abandon event's cause for vehicle `v` this tick, if any.
fn abandon_cause(w: &World, v: SimId) -> Option<u8> {
    w.snapshot()
        .armour_events
        .iter()
        .find(|e| e.kind == ArmourEventKind::Abandoned as u8 && e.vehicle == v.0)
        .map(|e| e.component)
}

/// S3c case 15 "bail-out" (`docs/slices/S3c-armour.md` Part A step 9):
/// (a) an ammo fire: every man out before the burn-out, one by hatch;
/// (b) crew losses to two and (c) gun and track out: the crew bail out;
/// (d) the crew squad re-crews its tank; (e) riflemen re-crew it,
/// untrained (reload measured); (f) enemy riflemen capture it with
/// `capture` on and `capture_needs_crew` off, and are refused with
/// `capture` off or `capture_needs_crew` on. Hard checks for the
/// mechanics, report rows for the timings.
fn case15(env: &Env) -> Result<CaseReport> {
    use frontline_content::SquadRole;
    let mut r = CaseReport::new(15, "bail-out, abandoned tanks, re-crew and capture");
    let (mut a_ok, mut b_ok, mut c_ok, mut d_ok, mut e_ok, mut f_ok, mut f_off) =
        (0u32, 0u32, 0u32, 0u32, 0u32, 0u32, 0u32);
    let mut f_crew = 0u32;
    let (mut first_out, mut last_out, mut recrew_t) = (Vec::new(), Vec::new(), Vec::new());
    let (mut reload_trained, mut reload_untrained) = (0u64, 0u64);
    let tank = |w: &mut World, p: u8, x: i32, f: FxVec2| -> Result<(SquadId, SimId)> {
        scenarios::tank(w, p, at(x, LANE_Y), f).ok_or_else(|| anyhow!("no tank"))
    };
    let kind = |w: &World, v: SimId| w.entities().kind()[v.index()];
    for (seed, p) in env.runs() {
        r.runs += 1;
        let face = if p == 0 { scenarios::EAST } else { WEST_DIR };
        // (a) Ammo fire: the men leave by their hatches before it burns out.
        let mut w = stage15(env, seed, |_| {})?;
        let (_, v) = tank(&mut w, p, 500, face)?;
        w.edit_vehicle(v, |vs| vs.ammo_fire = 1);
        let start = w.tick();
        let mut outs: Vec<u64> = Vec::new();
        while w.tick() < start + 20 * hz(&w) && kind(&w, v) != EntityKind::Wreck {
            w.step(&[]);
            for e in &w.snapshot().armour_events {
                if e.kind == ArmourEventKind::CrewOut as u8 && e.vehicle == v.0 {
                    outs.push(w.tick() - start);
                }
            }
        }
        let g = w.game().expect("match");
        if outs.len() == 5
            && kind(&w, v) == EntityKind::Wreck
            && g.players()[p as usize].crew.out == 5
        {
            a_ok += 1;
        }
        if let (Some(&f), Some(&l)) = (outs.iter().min(), outs.iter().max()) {
            first_out.push(f);
            last_out.push(l);
        }
        // (b) Crew losses to two.
        let mut w = stage15(env, seed, |_| {})?;
        let (_, v) = tank(&mut w, p, 500, face)?;
        w.edit_vehicle(v, |vs| {
            vs.seats[2].hp = 0;
            vs.seats[3].hp = 0;
            vs.seats[4].hp = 0;
        });
        w.step(&[]);
        b_ok += (abandon_cause(&w, v) == Some(frontline_sim::bail::AB_CREW)) as u32;
        // (c) Gun and a track out.
        let mut w = stage15(env, seed, |_| {})?;
        let (_, v) = tank(&mut w, p, 500, face)?;
        w.edit_vehicle(v, |vs| {
            vs.components[ComponentKind::Gun.index()] = 0;
            vs.components[ComponentKind::TrackRight.index()] = 0;
        });
        w.step(&[]);
        c_ok += (abandon_cause(&w, v) == Some(frontline_sim::bail::AB_USELESS)) as u32;
        // (d) The crew squad re-crews its tank.
        let mut w = stage15(env, seed, |_| {})?;
        let (sq, v) = tank(&mut w, p, 500, face)?;
        let def = w.game().expect("match").vehicle(v).expect("tank").type_idx as usize;
        let reload0 = {
            let s = w.match_setup().expect("match");
            gunnery::reload_of(&vstate(&w, v).expect("tank"), &s.vehicles[def], s)
        };
        order(&mut w, p, vec![CommandKind::Abandon { squad: sq }]);
        let crew = until(&mut w, 10, |w| {
            vstate(w, v)
                .and_then(|vs| vs.crew_squad)
                .and_then(|c| w.game().and_then(|g| g.squad(c)))
                .is_some_and(|c| c.members.len() == 5)
        })
        .and_then(|_| vstate(&w, v).and_then(|vs| vs.crew_squad));
        if let Some(c) = crew {
            order(
                &mut w,
                p,
                vec![CommandKind::Recrew {
                    squad: c,
                    vehicle: v,
                }],
            );
            if let Some(t) = until(&mut w, 60, |w| kind(w, v) == EntityKind::Vehicle) {
                recrew_t.push(t);
                let vs = vstate(&w, v).expect("tank");
                d_ok += (w.entities().owner()[v.index()].0 == p
                    && vs.seats.iter().all(|s| s.hp > 0 && !s.untrained))
                    as u32;
            }
        }
        // (e) Riflemen re-crew the abandoned tank, untrained.
        let mut w = stage15(env, seed, |_| {})?;
        let (sq, v) = tank(&mut w, p, 500, face)?;
        order(&mut w, p, vec![CommandKind::Abandon { squad: sq }]);
        until(&mut w, 10, |w| {
            vstate(w, v).is_some_and(|vs| vs.exit.iter().all(|&e| e == 0))
        });
        if let Some(c) = vstate(&w, v).and_then(|vs| vs.crew_squad) {
            order(&mut w, p, vec![CommandKind::Retreat { squad: c }]);
        }
        let rif = scenarios::spawn(&mut w, p, SquadRole::Rifle, at(500, LANE_Y + 25), face)
            .ok_or_else(|| anyhow!("no riflemen"))?;
        order(
            &mut w,
            p,
            vec![CommandKind::Recrew {
                squad: rif,
                vehicle: v,
            }],
        );
        if until(&mut w, 60, |w| kind(w, v) == EntityKind::Vehicle).is_some() {
            let vs = vstate(&w, v).expect("tank");
            let s = w.match_setup().expect("match");
            let rl = gunnery::reload_of(&vs, &s.vehicles[def], s);
            reload_trained += reload0 as u64;
            reload_untrained += rl as u64;
            e_ok += (vs.seats.iter().all(|s| s.hp > 0 && s.untrained) && rl > reload0) as u32;
        }
        // (f) Enemy riflemen capture it (capture on, `capture_needs_crew`
        // off), or are refused (capture off; or a crew needed).
        for (capture, needs_crew) in [(1u32, 0u32), (0, 0), (1, 1)] {
            let mut w = stage15(env, seed, |s| {
                s.rules.vehicles.recrew.capture = capture;
                s.rules.vehicles.recrew.capture_needs_crew = needs_crew;
            })?;
            let (sq, v) = tank(&mut w, p, 500, face)?;
            order(&mut w, p, vec![CommandKind::Abandon { squad: sq }]);
            until(&mut w, 10, |w| {
                vstate(w, v).is_some_and(|vs| vs.exit.iter().all(|&e| e == 0))
            });
            if let Some(c) = vstate(&w, v).and_then(|vs| vs.crew_squad) {
                order(&mut w, p, vec![CommandKind::Retreat { squad: c }]);
            }
            let q = 1 - p;
            let enemy = scenarios::spawn(&mut w, q, SquadRole::Rifle, at(500, LANE_Y - 25), face)
                .ok_or_else(|| anyhow!("no enemy riflemen"))?;
            order(
                &mut w,
                q,
                vec![CommandKind::Recrew {
                    squad: enemy,
                    vehicle: v,
                }],
            );
            if capture == 0 {
                f_off += w
                    .snapshot()
                    .recrew_refusals
                    .iter()
                    .any(|x| x.why == frontline_sim::RecrewRefusal::NoCapture)
                    as u32;
                continue;
            }
            if needs_crew == 1 {
                f_crew += w
                    .snapshot()
                    .recrew_refusals
                    .iter()
                    .any(|x| x.why == frontline_sim::RecrewRefusal::CaptureNeedsCrew)
                    as u32;
                continue;
            }
            if until(&mut w, 60, |w| kind(w, v) == EntityKind::Vehicle).is_some() {
                f_ok += (w.entities().owner()[v.index()].0 == q
                    && w.game().expect("match").players()[q as usize].crew.captured == 1)
                    as u32;
            }
        }
    }
    let n = r.runs;
    let all = |k: u32| (k == n, format!("{k} of {n}"));
    for (name, k) in [
        (
            "a ammo fire: all five out by hatch before the burn-out",
            a_ok,
        ),
        ("b crew losses to two: abandoned (crew)", b_ok),
        ("c gun and a track out: abandoned (gun and mobility)", c_ok),
        ("d the crew squad re-crews its tank (trained)", d_ok),
        ("e riflemen re-crew it, untrained, slower reload", e_ok),
        ("f an enemy squad captures it with capture on", f_ok),
        ("f with capture off the enemy is refused", f_off),
        (
            "f with capture_needs_crew enemy riflemen are refused",
            f_crew,
        ),
    ] {
        let (pass, detail) = all(k);
        r.check_v(name, pass, k as f64, format!("{n} of {n}"), detail);
    }
    let med = |v: &mut Vec<u64>| {
        v.sort_unstable();
        v.get(v.len() / 2)
            .map_or("none".into(), |&x| format!("{:.1} s", x as f64 / 30.0))
    };
    r.row(
        "a first / last man out (median)",
        format!("{} / {}", med(&mut first_out), med(&mut last_out)),
    );
    r.row("d order to re-crewed (median)", med(&mut recrew_t));
    r.row(
        "e reload trained → untrained (sum over runs, ticks)",
        format!("{reload_trained} → {reload_untrained}"),
    );
    Ok(r)
}

// ----------------------------------------------------------- case 14 --

// ----------------------------------------------------------- case 16 --

/// S4 (B7): a German Grenadiergruppe (two Panzerfaust 60s) against a
/// Sherman `d` m east of its first Panzerfaust carrier (S4 review: from the
/// carrier, not the squad anchor, whose men stand off to the side), the
/// tank standing or driving across (40 and 55 m still, inside the 60 m
/// still reach; 35 m moving, inside the 40 m moving reach; 55 m moving,
/// beyond it); per run 60 s. Hard: no Panzerfaust is
/// fired beyond the weapon's engagement distance for the tank's motion
/// (`engage_still` 60 m, `engage_moving` 40 m; +2 m for one tick of tank
/// travel), and every Panzerfaust round that hits the tank is a physical
/// HEAT hit in the armour log. Report: fired, hits, penetrations, tanks
/// disabled, per variant.
fn case16(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(
        16,
        "Grenadiergruppe Panzerfausts vs a Sherman, 40 / 55 m still, 35 / 55 m moving",
    );
    let mut far = 0u32;
    let mut stray = 0u32;
    for (d, moving) in [(40, false), (55, false), (35, true), (55, true)] {
        let (mut fired, mut hits, mut pens, mut dis, mut runs) = (0u32, 0u64, 0u64, 0u32, 0u32);
        for &seed in &env.seeds {
            let mut setup = env.setup(seed)?;
            setup.players.iter_mut().for_each(|p| p.is_ai = false);
            setup.map.terrain.octaves.clear();
            setup.map.terrain.bands.clear();
            setup.map.woods.clear();
            let mut w = World::new_match(setup, env.threads)?;
            let s = w.match_setup().expect("match").clone();
            let g = s
                .squad_type_index("ww2:ger_grenadiergruppe")
                .ok_or_else(|| anyhow!("no ww2:ger_grenadiergruppe"))?;
            let sh = s
                .squad_type_index("ww2:usa_m4a3_75_squad")
                .ok_or_else(|| anyhow!("no ww2:usa_m4a3_75_squad"))?;
            let pzf = s
                .items
                .iter()
                .find(|it| it.kind == ItemKind::SingleShotAt)
                .and_then(|it| it.weapon)
                .ok_or_else(|| anyhow!("no single-shot AT item"))?;
            let wpn = &s.weapons[pzf as usize];
            let pzf_ammo = wpn.ammo;
            let (still_m, moving_m) = (
                Fx::from(wpn.engage_still).to_f32() as f64,
                Fx::from(wpn.engage_moving).to_f32() as f64,
            );
            let x = TRENCH_X - 100;
            let east = FxVec2::new(Fx::ONE, Fx::ZERO);
            let north = FxVec2::new(Fx::ZERO, Fx::ONE);
            let Some(gsq) = w.spawn_squad_at(PlayerId(0), g, at(x, LANE_Y), east) else {
                return Err(anyhow!("could not place the Grenadiergruppe"));
            };
            // The first man with a Panzerfaust: the tank's distance is
            // from him.
            let carrier = members(&w, gsq)
                .into_iter()
                .find(|m| w.entities().kit()[m.index()].items[ItemKind::SingleShotAt.index()] > 0)
                .ok_or_else(|| anyhow!("no Panzerfaust carrier"))?;
            let c_at = pos(&w, carrier);
            let (cx, cy) = (c_at.x.round_int(), c_at.y.round_int());
            let y0 = if moving { cy - 70 } else { cy };
            let Some(tsq) = w.spawn_squad_at(PlayerId(1), sh, at(cx + d, y0), north) else {
                return Err(anyhow!("could not place the Sherman"));
            };
            let tank = members(&w, tsq)[0];
            let men = members(&w, gsq);
            w.enable_hit_log();
            if moving {
                order(
                    &mut w,
                    1,
                    vec![CommandKind::Move {
                        squad: tsq,
                        target: at(cx + d, cy + 90),
                    }],
                );
            }
            runs += 1;
            let shots0 = w.game().expect("match").players()[0].s4.single_shots;
            for _ in 0..60 * hz(&w) {
                w.step(&[]);
                // Every Panzerfaust fired this tick: within its engagement
                // distance of the tank.
                let tank_at = pos(&w, tank);
                let moving_now = w
                    .game()
                    .and_then(|g| g.vehicle(tank))
                    .is_some_and(|v| v.avg_vel.length() > Fx::from(s.rules.vehicles.moving_speed));
                for sh in w
                    .snapshot()
                    .shots
                    .iter()
                    .filter(|sh| sh.weapon == pzf && men.contains(&sh.shooter))
                {
                    let reach = if moving_now { moving_m } else { still_m };
                    let (dx, dy) = (
                        f64::from(sh.from[0] - tank_at.x.to_f32()),
                        f64::from(sh.from[1] - tank_at.y.to_f32()),
                    );
                    if (dx * dx + dy * dy).sqrt() > reach + 2.0 {
                        far += 1;
                    }
                }
                for h in w.take_hit_log() {
                    if Some(h.ammo) == pzf_ammo && h.vehicle == tank {
                        hits += 1;
                        pens += (h.result == PenResult::Penetration) as u64;
                    } else if Some(h.ammo) == pzf_ammo {
                        stray += 1;
                    }
                }
                if disabled(&w, tank) {
                    break;
                }
            }
            fired += w.game().expect("match").players()[0].s4.single_shots - shots0;
            dis += disabled(&w, tank) as u32;
            if env.verbose {
                eprintln!(
                    "case 16 seed {seed} {d} m {}: fired {}, disabled {}",
                    if moving { "moving" } else { "still" },
                    w.game().expect("match").players()[0].s4.single_shots - shots0,
                    disabled(&w, tank)
                );
            }
        }
        r.runs += runs;
        r.row(
            format!("{d} m {}", if moving { "moving" } else { "still" }),
            format!(
                "{fired} Panzerfausts fired in {runs} runs, {hits} hit, {pens} penetrated; tank disabled in {dis}"
            ),
        );
    }
    r.check_v(
        "no Panzerfaust beyond its engagement distance",
        far == 0,
        far as f64,
        "0",
        format!("{far} fired too far"),
    );
    r.check_v(
        "Panzerfaust hits are physical hits on a vehicle",
        stray == 0,
        stray as f64,
        "0",
        format!("{stray} logged on another vehicle"),
    );
    Ok(r)
}

/// S7 cases 17 and 18 (vehicles.md 10, casemate duel): a StuG III G
/// against a Sherman, 120 s, every seed both ways. Reports who wins (the
/// other knocked out, disabled or abandoned), the time to each side's
/// first hit (median, s), and in how many runs the StuG fired. Report-only
/// checks: `docs/slices/targets/S7.toml` T34 holds the bounds.
fn casemate_case(env: &Env, n: u32, kind: scenarios::CasemateDuel) -> Result<CaseReport> {
    let name = match kind {
        scenarios::CasemateDuel::Frontal300 => "casemate duel: StuG frontal vs Sherman at 300 m",
        scenarios::CasemateDuel::Flank45 => {
            "casemate duel: Sherman 45° off the StuG's nose at 200 m"
        }
    };
    let mut r = CaseReport::new(n, name);
    let (mut stug_wins, mut sh_wins, mut stug_fired, mut runs) = (0u32, 0u32, 0u32, 0u32);
    let mut first_hit: [Vec<f64>; 2] = [Vec::new(), Vec::new()];
    let mut hits = [0u32; 2];
    let mut pens = [0u32; 2];
    let mut pivots = 0u32;
    for (seed, flip) in env.runs() {
        let mut s = scenarios::casemate_duel(env.setup(seed)?, env.threads, kind, flip == 1)?;
        runs += 1;
        r.runs += 1;
        let (stug, sh) = (s.tanks[0].1, s.tanks[1].1);
        let w = &mut s.world;
        let rate = hz(w) as f64;
        w.enable_hit_log();
        let start = w.tick();
        let limit = start + 120 * hz(w);
        let mut first: [Option<u64>; 2] = [None, None];
        let mut fired = false;
        let mut pivoted = false;
        while w.tick() < limit && !disabled(w, stug) && !disabled(w, sh) {
            w.step(&[]);
            fired |= w
                .snapshot()
                .armour_events
                .iter()
                .any(|e| e.kind == ArmourEventKind::Fired as u8 && e.vehicle == stug.0);
            pivoted |= vstate(w, stug).is_some_and(|v| v.lay_turn.is_some());
            for h in w.take_hit_log() {
                let side = (h.vehicle == stug) as usize;
                // side 0: the StuG hit the Sherman; 1: the Sherman hit the StuG.
                hits[side] += 1;
                if h.result == PenResult::Penetration {
                    pens[side] += 1;
                }
                if first[side].is_none() {
                    first[side] = Some(w.tick() - start);
                }
            }
        }
        for (k, f) in first.iter().enumerate() {
            if let Some(t) = f {
                first_hit[k].push(*t as f64 / rate);
            }
        }
        stug_fired += u32::from(fired);
        pivots += u32::from(pivoted);
        match (disabled(w, stug), disabled(w, sh)) {
            (false, true) => stug_wins += 1,
            (true, false) => sh_wins += 1,
            _ => {}
        }
    }
    let med = |v: &Vec<f64>| {
        let mut v = v.clone();
        v.sort_by(f64::total_cmp);
        v.get(v.len() / 2).copied().unwrap_or(-1.0)
    };
    let p = |k: u32| k as f64 * 100.0 / runs.max(1) as f64;
    r.row(
        "result",
        format!(
            "StuG won {stug_wins}, Sherman won {sh_wins}, neither {} of {runs}; StuG fired in \
             {stug_fired}, asked for a hull turn in {pivots}; hits StuG -> Sherman {} (pen {}), \
             Sherman -> StuG {} (pen {})",
            runs - stug_wins - sh_wins,
            hits[0],
            pens[0],
            hits[1],
            pens[1]
        ),
    );
    let report = |r: &mut CaseReport, name: &str, v: f64, what: String| {
        r.check_v(name, true, round3(v), "report (S7.toml T34)", what);
    };
    report(
        &mut r,
        "StuG wins pct",
        p(stug_wins),
        format!("{stug_wins} of {runs}"),
    );
    report(
        &mut r,
        "Sherman wins pct",
        p(sh_wins),
        format!("{sh_wins} of {runs}"),
    );
    report(
        &mut r,
        "StuG fired pct",
        p(stug_fired),
        format!("{stug_fired} of {runs}"),
    );
    report(
        &mut r,
        "first hit s StuG",
        med(&first_hit[0]),
        format!(
            "median of {} runs with a hit (-1: none)",
            first_hit[0].len()
        ),
    );
    report(
        &mut r,
        "first hit s Sherman",
        med(&first_hit[1]),
        format!(
            "median of {} runs with a hit (-1: none)",
            first_hit[1].len()
        ),
    );
    Ok(r)
}

fn case17(env: &Env) -> Result<CaseReport> {
    casemate_case(env, 17, scenarios::CasemateDuel::Frontal300)
}

fn case18(env: &Env) -> Result<CaseReport> {
    casemate_case(env, 18, scenarios::CasemateDuel::Flank45)
}

// ------------------------------------------------------- cases 19, 20 --

/// S7 open-top cases: flat open ground, both sides human, the crew stay in
/// (no bail-out on losses).
fn open_top_world(env: &Env, seed: u64) -> Result<World> {
    let mut s = env.setup(seed)?;
    open_top_from(&mut s, env.threads, |_| {})
}

/// `open_top_world`'s staging on an existing setup, then `edit` on it.
fn open_top_from(
    s: &mut MatchSetup,
    threads: usize,
    edit: impl FnOnce(&mut MatchSetup),
) -> Result<World> {
    s.players.iter_mut().for_each(|p| p.is_ai = false);
    s.rules.start_manpower = 100_000;
    s.map.terrain.octaves.clear();
    s.map.terrain.bands.clear();
    s.map.woods.clear();
    s.rules.vehicles.bail.crew_below = 0;
    edit(s);
    World::new_match(s.clone(), threads).map_err(|e| anyhow!("{e}"))
}

fn spawn_card(w: &mut World, p: u8, card: &str, at: FxVec2, f: FxVec2) -> Result<(SquadId, SimId)> {
    let t = w
        .match_setup()
        .and_then(|s| s.squad_type_index(card))
        .ok_or_else(|| anyhow!("no card {card}"))?;
    let sq = w
        .spawn_squad_at(PlayerId(p), t, at, f)
        .ok_or_else(|| anyhow!("cannot spawn {card}"))?;
    let v = members(w, sq)[0];
    Ok((sq, v))
}

/// The vehicle's living crew HP, seat by seat.
fn crew_hp(w: &World, v: SimId) -> Vec<u16> {
    vstate(w, v).map_or(Vec::new(), |vs| vs.seats.iter().map(|s| s.hp).collect())
}

fn crew_hurt(before: &[u16], after: &[u16]) -> bool {
    after.len() < before.len() || after.iter().zip(before).any(|(a, b)| a < b)
}

fn no_ammo(w: &mut World, v: SimId) {
    w.edit_vehicle(v, |vs| {
        vs.ammo.iter_mut().for_each(|n| *n = 0);
        vs.loaded = None;
    });
}

/// Case 19 trials per run and kind (stopped, moving).
const CASE19_TRIALS: u64 = 3;

/// Case 19 (vehicles.md 10): a German rifleman lobs a stick grenade from 15
/// m at a disarmed M10's hull centre, stopped or crossing at speed: where
/// it comes to rest (inside the open turret or not) and whether the crew
/// is hit (no success roll: the lob's physics decides).
fn case19(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(19, "grenades at a stopped and a moving M10 from 15 m");
    // [stopped, moving]: trials, inside, crew hit.
    let mut n = [0u32; 2];
    let mut inside = [0u32; 2];
    let mut hit = [0u32; 2];
    for (seed, flip) in env.runs() {
        r.runs += 1;
        for k in 0..CASE19_TRIALS {
            for (m, moving) in [false, true].into_iter().enumerate() {
                let mut w = open_top_world(env, seed * 16 + k * 2 + flip as u64)?;
                // The M10 faces across the throw line; the bearing varies.
                let az = (seed * 37 + k * 71 + flip as u64 * 13) % 360;
                let f = dir_deg(az as i32);
                let c = at(500, 500);
                let (vsq, v) = spawn_card(&mut w, 1, "ww2:usa_m10_squad", c, f)?;
                no_ammo(&mut w, v);
                let side = FxVec2::new(-f.y, f.x);
                // A moving M10 starts 25 m back along its path and crosses.
                let start = if moving { c - f * Fx::from_int(25) } else { c };
                if moving {
                    w.place_vehicle(v, start, Fx::atan2(f.y, f.x));
                    order(
                        &mut w,
                        1,
                        vec![CommandKind::Move {
                            squad: vsq,
                            target: c + f * Fx::from_int(60),
                        }],
                    );
                }
                let (gsq, _) = spawn_card(
                    &mut w,
                    0,
                    "ww2:ger_grenadiergruppe",
                    c + side * Fx::from_int(15),
                    -side,
                )?;
                // Grenades only: the squad's Panzerfaust stays out of it;
                // S8a: so do its rifles and MG (6.4 lets them fire at the
                // M10's exposed crew, S7-7), so the case still measures
                // the grenade alone.
                for m in members(&w, gsq) {
                    w.edit_soldier(m, |k, c| {
                        k.items[ItemKind::SingleShotAt.index()] = 0;
                        k.rounds.iter_mut().for_each(|n| *n = 0);
                        c.mag = 0;
                    });
                }
                let Some(man) = members(&w, gsq)
                    .into_iter()
                    .find(|m| w.entities().kit()[m.index()].items[ItemKind::Frag.index()] > 0)
                else {
                    continue;
                };
                // Moving: throw (at where it is, no lead) when it comes
                // within 6 m of the thrower's line.
                for t in 0..10 * hz(&w) {
                    w.step(&[]);
                    let near = dist(pos(&w, v), c) < 6.0;
                    if (!moving && t + 1 >= hz(&w)) || (moving && near) {
                        break;
                    }
                }
                let before = crew_hp(&w, v);
                let target = pos(&w, v);
                order(
                    &mut w,
                    0,
                    vec![CommandKind::Throw {
                        units: vec![man],
                        item: ItemKind::Frag,
                        target,
                    }],
                );
                n[m] += 1;
                let mut was_inside = false;
                for _ in 0..8 * hz(&w) {
                    w.step(&[]);
                    let g = w.game().expect("match");
                    was_inside |= g
                        .thrown()
                        .iter()
                        .any(|t| t.inside.is_some_and(|i| i.vehicle == v));
                    if g.thrown().is_empty() && w.tick() > 0 && was_inside {
                        break;
                    }
                }
                inside[m] += u32::from(was_inside);
                hit[m] += u32::from(crew_hurt(&before, &crew_hp(&w, v)));
                if env.verbose {
                    eprintln!(
                        "case 19 seed {seed} k {k} moving {moving}: inside {was_inside}, crew {before:?} -> {:?}, throws {}",
                        crew_hp(&w, v),
                        w.stats().throws
                    );
                }
            }
        }
    }
    let p = |a: u32, b: u32| a as f64 * 100.0 / b.max(1) as f64;
    r.row(
        "result",
        format!(
            "stopped: {} of {} inside, crew hit in {}; moving: {} of {} inside, crew hit in {}",
            inside[0], n[0], hit[0], inside[1], n[1], hit[1]
        ),
    );
    let mut report = |name: &str, v: f64, what: String| {
        r.check_v(name, true, round3(v), "report (S7.toml T35)", what);
    };
    report(
        "stopped inside pct",
        p(inside[0], n[0]),
        format!("{} of {}", inside[0], n[0]),
    );
    report(
        "stopped crew hit pct",
        p(hit[0], n[0]),
        format!("{} of {}", hit[0], n[0]),
    );
    report(
        "moving inside pct",
        p(inside[1], n[1]),
        format!("{} of {}", inside[1], n[1]),
    );
    report(
        "moving crew hit pct",
        p(hit[1], n[1]),
        format!("{} of {}", hit[1], n[1]),
    );
    Ok(r)
}

/// Case 20 bombs per run and vehicle.
const CASE20_BOMBS: u64 = 6;

/// Case 20 (vehicles.md 10): 81 mm bombs (GrW 34) bursting on the ground 3-6
/// m from a stopped M10 and a stopped Sherman (bearing and distance vary):
/// how often the crew is hit. Fragments reach an open turret's crew over the
/// rim; walls and roofs stop them (no fragment pierces a plate, S7-8).
fn case20(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(20, "81 mm near misses 3-6 m from an M10 and a Sherman");
    let mut n = [0u32; 2];
    let mut hit = [0u32; 2];
    for (seed, flip) in env.runs() {
        r.runs += 1;
        for (m, card) in ["ww2:usa_m10_squad", "ww2:usa_m4a3_75_squad"]
            .into_iter()
            .enumerate()
        {
            for k in 0..CASE20_BOMBS {
                let mut w = open_top_world(env, seed * 32 + k * 2 + flip as u64)?;
                let weapon = w
                    .match_setup()
                    .and_then(|s| s.weapons.iter().position(|x| x.id == "ww2:grw34"))
                    .ok_or_else(|| anyhow!("no grw34"))? as u16;
                let (_, v) = spawn_card(&mut w, 1, card, at(500, 500), dir_deg(0))?;
                no_ammo(&mut w, v);
                w.step(&[]);
                let before = crew_hp(&w, v);
                let az = ((seed * 53 + k * 61 + flip as u64 * 29) % 360) as i32;
                let d = 3.0 + ((seed + k * 7 + flip as u64) % 4) as f64;
                let p = pos(&w, v) + dir_deg(az) * Fx::from_milli((d * 1000.0) as i32);
                if !w.drop_shell_now(0, weapon, p) {
                    continue;
                }
                n[m] += 1;
                for _ in 0..hz(&w) {
                    w.step(&[]);
                }
                hit[m] += u32::from(crew_hurt(&before, &crew_hp(&w, v)));
            }
        }
    }
    let p = |a: u32, b: u32| a as f64 * 100.0 / b.max(1) as f64;
    r.row(
        "result",
        format!(
            "M10 crew hit by {} of {} bombs; Sherman crew by {} of {}",
            hit[0], n[0], hit[1], n[1]
        ),
    );
    r.check_v(
        "m10 crew hit pct",
        true,
        round3(p(hit[0], n[0])),
        "report (S7.toml T36)",
        format!("{} of {}", hit[0], n[0]),
    );
    r.check_v(
        "sherman crew hit pct",
        hit[1] == 0,
        round3(p(hit[1], n[1])),
        "0 (a closed tank)",
        format!("{} of {}", hit[1], n[1]),
    );
    Ok(r)
}

/// Case 21 shots per run, weapon, vehicle and range.
const CASE21_SHOTS: u64 = 6;

/// One small-arms round of `gun` for `p` at vehicle `v` from `d` m on the
/// bearing `from_deg` (world), aimed at `aim` (drop compensated). Whether
/// it struck the vehicle and whether it got through a plate.
fn small_shot(
    w: &mut World,
    p: u8,
    gun: u16,
    aim: FxVec3,
    from_deg: i32,
    d: i32,
) -> Option<(bool, bool)> {
    let speed = f64::from(Fx::from(w.match_setup()?.weapons[gun as usize].muzzle_speed).to_f32());
    let from2 = aim.xy() + dir_deg(from_deg) * Fx::from_int(d);
    let from = from2.extend(aim.z + Fx::from_ratio(10, 100));
    let t = f64::from(d) / speed.max(1.0);
    let drop = 0.5 * 9.81 * t * t;
    let up = aim + FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::from_milli((drop * 1000.0) as i32));
    let (pens0, stopped0) = (w.stats().small_arms_pens, w.stats().rounds_stopped);
    if !w.fire_round_now(p, gun, from, up) {
        return None;
    }
    for _ in 0..hz(w) {
        w.step(&[]);
    }
    let pens = w.stats().small_arms_pens - pens0;
    let stopped = w.stats().rounds_stopped - stopped0;
    Some((pens + stopped > 0, pens > 0))
}

/// Case 21 (vehicles.md 6.4, 10): small arms against the half-tracks'
/// thin plates, rounds fired straight at the plate (no spread): MG 42 at the
/// M3A1's face-hardened side and the .50 at the 251's sloped side from 50,
/// 150 and 300 m; rifles at both fronts from 50 m; MG 42 at the M10's
/// exposed loader from 100 m (S7-7).
fn case21(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(21, "small arms against half-track plates (6.4)");
    r.runs = env.runs().len() as u32;
    // (label, gun, card, bearing of the shooter from the vehicle facing
    // north (0 = east: its side), ranges)
    let combos: [(&str, &str, &str, i32, &[i32]); 4] = [
        (
            "mg42 m3a1 side",
            "ww2:mg42_bipod",
            "ww2:usa_m3a1_squad",
            0,
            &[50, 150, 300],
        ),
        (
            "m2hb 251 side",
            "ww2:m2hb",
            "ww2:ger_sdkfz251_1_squad",
            0,
            &[50, 150, 300],
        ),
        (
            "rifle 251 front",
            "ww2:m1_garand",
            "ww2:ger_sdkfz251_1_squad",
            90,
            &[50],
        ),
        (
            "rifle m3a1 front",
            "ww2:kar98k",
            "ww2:usa_m3a1_squad",
            90,
            &[50],
        ),
    ];
    for (label, gun_id, card, bearing, ranges) in combos {
        for &d in ranges {
            let (mut hit, mut pen) = (0u32, 0u32);
            for (seed, flip) in env.runs() {
                let mut w = open_top_world(env, seed * 64 + d as u64 + flip as u64)?;
                let gun = w
                    .match_setup()
                    .and_then(|s| s.weapons.iter().position(|x| x.id == gun_id))
                    .ok_or_else(|| anyhow!("no {gun_id}"))? as u16;
                let owner = if card.starts_with("ww2:ger") { 0 } else { 1 };
                let (_, v) = spawn_card(&mut w, owner, card, at(500, 500), dir_deg(90))?;
                w.edit_vehicle(v, |vs| {
                    vs.mgs.iter_mut().for_each(|m| m.spent = u32::MAX / 2)
                });
                w.step(&[]);
                let base = w.entities().transform()[v.index()].pos;
                for k in 0..CASE21_SHOTS {
                    // Spread over the plate: 1.6 m along it, 0.5-1.4 m up.
                    let along = ((seed * 7 + k * 13 + flip as u64 * 5) % 17) as i32 * 10 - 80;
                    let up = 50 + ((seed * 11 + k * 7) % 10) as i32 * 10;
                    let off = if bearing == 0 {
                        FxVec3::new(
                            Fx::ZERO,
                            Fx::from_milli(along * 10),
                            Fx::from_milli(up * 10),
                        )
                    } else {
                        FxVec3::new(Fx::from_milli(along * 5), Fx::ZERO, Fx::from_milli(up * 10))
                    };
                    if let Some((h, p)) = small_shot(&mut w, 1 - owner, gun, base + off, bearing, d)
                    {
                        hit += u32::from(h);
                        pen += u32::from(p);
                    }
                }
            }
            let pct = round3(pen as f64 * 100.0 / hit.max(1) as f64);
            let name = if ranges.len() == 1 {
                format!("{label} pen pct")
            } else {
                format!("{label} {d} pen pct")
            };
            let (pass, limit) = match (label, d) {
                ("mg42 m3a1 side", 50) => (pct >= 80.0, ">= 80"),
                ("mg42 m3a1 side", _) => (true, "report"),
                ("m2hb 251 side", _) => (pct >= 80.0, ">= 80"),
                _ => (pen == 0, "0"),
            };
            r.check_v(
                name,
                pass,
                pct,
                limit,
                format!("{pen} of {hit} rounds that struck"),
            );
        }
    }
    // S7-7: MG 42 rounds at the M10's exposed loader's head from 100 m.
    let (mut shots, mut hits) = (0u32, 0u32);
    for (seed, flip) in env.runs() {
        let mut w = open_top_world(env, seed * 64 + 7 + flip as u64)?;
        let gun = w
            .match_setup()
            .and_then(|s| s.weapons.iter().position(|x| x.id == "ww2:mg42_bipod"))
            .ok_or_else(|| anyhow!("no mg42_bipod"))? as u16;
        let (_, v) = spawn_card(&mut w, 1, "ww2:usa_m10_squad", at(500, 500), dir_deg(180))?;
        no_ammo(&mut w, v);
        w.step(&[]);
        let s = w.match_setup().ok_or_else(|| anyhow!("match"))?;
        let vt = &s.vehicles[vstate(&w, v).map_or(0, |x| x.type_idx) as usize];
        let k = vt
            .seats
            .iter()
            .position(|x| x.role == frontline_content::SeatRole::Loader)
            .ok_or_else(|| anyhow!("no loader"))?;
        let part = vt.seats[k].part;
        for _ in 0..CASE21_SHOTS {
            let f = *w
                .game()
                .and_then(|g| g.vehicle_derived().frame(v))
                .ok_or_else(|| anyhow!("frame"))?;
            let sb = frontline_sim::damage::part_box(&f, &part);
            let head = sb.center + sb.axes.z * (sb.half.z - Fx::from_ratio(15, 100));
            let before = crew_hp(&w, v).get(k).copied().unwrap_or(0);
            if small_shot(&mut w, 0, gun, head, 180, 100).is_none() {
                continue;
            }
            shots += 1;
            hits += u32::from(crew_hp(&w, v).get(k).copied().unwrap_or(0) < before);
            w.edit_vehicle(v, |vs| vs.seats[k].hp = 100);
        }
    }
    r.check_v(
        "mg42 m10 loader hit pct",
        hits > 0,
        round3(hits as f64 * 100.0 / shots.max(1) as f64),
        "> 0 (report)",
        format!("{hits} of {shots}"),
    );
    Ok(r)
}

/// S8a: a transport of `card` for player `p` at `c` facing `f` (its MG
/// dry when `silent`), with a squad of `inf` (also player `p`) seated in
/// it.
fn loaded_transport(
    w: &mut World,
    p: u8,
    card: &str,
    inf: &str,
    c: FxVec2,
    f: FxVec2,
    silent: bool,
) -> Result<(SquadId, SimId, SquadId)> {
    let (vsq, v) = spawn_card(w, p, card, c, f)?;
    if silent {
        w.edit_vehicle(v, |vs| {
            vs.mgs.iter_mut().for_each(|m| m.spent = u32::MAX / 2)
        });
    }
    let (isq, _) = spawn_card(w, p, inf, c - f * Fx::from_int(12), f)?;
    if !w.seat_now(isq, v) {
        return Err(anyhow!("{inf} does not fit in {card}"));
    }
    Ok((vsq, v, isq))
}

/// HP of each of `men` (0 once dead or gone).
fn men_hp(w: &World, men: &[SimId]) -> Vec<u16> {
    men.iter()
        .map(|m| {
            let i = m.index();
            if w.entities().kind().get(i) == Some(&EntityKind::Soldier) {
                w.entities().combat()[i].hp
            } else {
                0
            }
        })
        .collect()
}

fn weapon_idx(w: &World, id: &str) -> Result<u16> {
    w.match_setup()
        .and_then(|s| s.weapons.iter().position(|x| x.id == id))
        .map(|k| k as u16)
        .ok_or_else(|| anyhow!("no {id}"))
}

/// Case 22 (vehicles.md 10): one bazooka rocket at a loaded 251's
/// compartment side from 100 m (no spread; the point along the side
/// varies): how often a passenger is hit, and the passengers lost.
fn case22(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(22, "a bazooka rocket into a loaded SdKfz 251 (6.6)");
    let (mut n, mut hit, mut lost, mut ko) = (0u32, 0u32, 0u32, 0u32);
    for (seed, flip) in env.runs() {
        r.runs += 1;
        for k in 0..3u64 {
            let mut w = open_top_world(env, seed * 32 + k * 2 + flip as u64)?;
            let gun = weapon_idx(&w, "ww2:m9_bazooka")?;
            let (_, v, isq) = loaded_transport(
                &mut w,
                0,
                "ww2:ger_sdkfz251_1_squad",
                "ww2:ger_grenadiergruppe",
                at(500, 500),
                dir_deg(90),
                true,
            )?;
            w.step(&[]);
            let men = members(&w, isq);
            let before = men_hp(&w, &men);
            // The right side faces east: from 100 m east, at the
            // compartment (1.0-1.6 m up, 2.2 m along it).
            let base = w.entities().transform()[v.index()].pos;
            let along = ((seed * 7 + k * 5 + flip as u64 * 3) % 9) as i32 * 25 - 100 - 74;
            let up = 100 + ((seed * 3 + k * 11) % 7) as i32 * 10;
            let aim = FxVec3::new(
                base.x + Fx::ONE,
                base.y + Fx::from_milli(along * 10),
                base.z + Fx::from_milli(up * 10),
            );
            let speed = w
                .match_setup()
                .and_then(|s| {
                    let wd = &s.weapons[gun as usize];
                    wd.ammo
                        .and_then(|a| s.ammo.get(a as usize))
                        .map(|a| a.speed)
                })
                .map_or(80.0, |sp| f64::from(Fx::from(sp).to_f32()));
            let t = 100.0 / speed.max(1.0);
            let drop = Fx::from_milli((0.5 * 9.81 * t * t * 1000.0) as i32);
            let from = FxVec3::new(
                aim.x + Fx::from_int(100),
                aim.y,
                aim.z + Fx::from_ratio(1, 10),
            );
            let up_aim = FxVec3::new(aim.x, aim.y, aim.z + drop);
            if !w.fire_rocket_now(1, gun, from, up_aim) {
                return Err(anyhow!("cannot fire the rocket"));
            }
            for _ in 0..3 * hz(&w) {
                w.step(&[]);
            }
            let after = men_hp(&w, &men);
            n += 1;
            let h = before.iter().zip(&after).filter(|(b, a)| a < b).count() as u32;
            let dead = after.iter().filter(|&&a| a == 0).count() as u32;
            hit += u32::from(h > 0);
            lost += dead;
            ko += u32::from(!alive(&w, v));
            if env.verbose {
                eprintln!(
                    "case 22 seed {seed} k {k}: {h} men hit, {dead} lost, alive {}",
                    alive(&w, v)
                );
            }
        }
    }
    let pct = round3(hit as f64 * 100.0 / n.max(1) as f64);
    r.check_v(
        "passenger hit pct",
        pct >= 50.0,
        pct,
        ">= 50 (S8a.toml T45a)",
        format!("{hit} of {n} rockets hit a passenger"),
    );
    r.check_v(
        "passengers lost mean",
        true,
        round3(lost as f64 / n.max(1) as f64),
        "report",
        format!("{lost} men lost in {n} rockets; vehicle knocked out {ko} times"),
    );
    Ok(r)
}

/// Case 23 (vehicles.md 10): a German rifleman lobs a stick grenade from 15
/// m at a loaded 251 (as case 19), stopped or driving across: the share of
/// passengers lost when stopped, and how often the grenade comes to rest
/// inside when it moves.
fn case23(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(
        23,
        "a stick grenade at a loaded SdKfz 251, stopped and moving",
    );
    let mut n = [0u32; 2];
    let mut inside = [0u32; 2];
    let mut riders = [0u32; 2];
    let mut lost = [0u32; 2];
    let mut speed_sum = 0.0f64;
    for (seed, flip) in env.runs() {
        r.runs += 1;
        for k in 0..CASE19_TRIALS {
            for (m, moving) in [false, true].into_iter().enumerate() {
                let mut w = open_top_world(env, seed * 16 + k * 2 + flip as u64)?;
                let az = (seed * 37 + k * 71 + flip as u64 * 13) % 360;
                let f = dir_deg(az as i32);
                let c = at(500, 500);
                let start = if moving { c - f * Fx::from_int(25) } else { c };
                let (vsq, v, isq) = loaded_transport(
                    &mut w,
                    1,
                    "ww2:ger_sdkfz251_1_squad",
                    "ww2:ger_grenadiergruppe",
                    start,
                    f,
                    true,
                )?;
                if moving {
                    order(
                        &mut w,
                        1,
                        vec![CommandKind::Move {
                            squad: vsq,
                            target: c + f * Fx::from_int(60),
                        }],
                    );
                }
                let side = FxVec2::new(-f.y, f.x);
                let (gsq, _) = spawn_card(
                    &mut w,
                    0,
                    "ww2:ger_grenadiergruppe",
                    c + side * Fx::from_int(15),
                    -side,
                )?;
                for man in members(&w, gsq) {
                    w.edit_soldier(man, |kt, cb| {
                        kt.items[ItemKind::SingleShotAt.index()] = 0;
                        kt.rounds.iter_mut().for_each(|x| *x = 0);
                        cb.mag = 0;
                    });
                }
                let Some(man) = members(&w, gsq)
                    .into_iter()
                    .find(|x| w.entities().kit()[x.index()].items[ItemKind::Frag.index()] > 0)
                else {
                    continue;
                };
                for t in 0..10 * hz(&w) {
                    w.step(&[]);
                    let near = dist(pos(&w, v), c) < 6.0;
                    if (!moving && t + 1 >= hz(&w)) || (moving && near) {
                        break;
                    }
                }
                if moving {
                    speed_sum += f64::from(vstate(&w, v).map_or(0.0, |x| x.speed.to_f32()));
                }
                let men = members(&w, isq);
                let target = pos(&w, v);
                order(
                    &mut w,
                    0,
                    vec![CommandKind::Throw {
                        units: vec![man],
                        item: ItemKind::Frag,
                        target,
                    }],
                );
                n[m] += 1;
                let mut was_inside = false;
                for _ in 0..8 * hz(&w) {
                    w.step(&[]);
                    let g = w.game().expect("match");
                    was_inside |= g
                        .thrown()
                        .iter()
                        .any(|t| t.inside.is_some_and(|i| i.vehicle == v));
                    if g.thrown().is_empty() && was_inside {
                        break;
                    }
                }
                let dead = men_hp(&w, &men).iter().filter(|&&h| h == 0).count() as u32;
                inside[m] += u32::from(was_inside);
                riders[m] += men.len() as u32;
                lost[m] += dead;
                if env.verbose {
                    eprintln!(
                        "case 23 seed {seed} k {k} moving {moving}: inside {was_inside}, {dead} of {} lost",
                        men.len()
                    );
                }
            }
        }
    }
    let p = |a: u32, b: u32| round3(a as f64 * 100.0 / b.max(1) as f64);
    let stopped_lost = p(lost[0], riders[0]);
    let moving_in = p(inside[1], n[1]);
    r.row(
        "result",
        format!(
            "stopped: {} of {} inside, {} of {} passengers lost; moving ({:.1} m/s mean at the throw): {} of {} inside, {} of {} lost",
            inside[0],
            n[0],
            lost[0],
            riders[0],
            speed_sum / n[1].max(1) as f64,
            inside[1],
            n[1],
            lost[1],
            riders[1]
        ),
    );
    r.check_v(
        "stopped passengers lost pct",
        stopped_lost >= 50.0,
        stopped_lost,
        ">= 50 (S8a.toml T46a)",
        format!("{} of {}", lost[0], riders[0]),
    );
    r.check_v(
        "stopped inside pct",
        true,
        p(inside[0], n[0]),
        "report",
        format!("{} of {}", inside[0], n[0]),
    );
    r.check_v(
        "moving inside pct",
        moving_in <= 20.0,
        moving_in,
        "<= 20 (S8a.toml T46b)",
        format!("{} of {}", inside[1], n[1]),
    );
    r.check_v(
        "moving passengers lost pct",
        true,
        p(lost[1], riders[1]),
        "report",
        format!("{} of {}", lost[1], riders[1]),
    );
    Ok(r)
}

/// Case 24 (vehicles.md 10): ten men get out of a 251 under MG 42 fire
/// from 200 m off its right side, by the rear door or over the sides. The
/// gun (no spread but a small aim jitter) fires 15 rounds a second at the
/// nearest man on foot within 20 m of the vehicle, else at the head of the
/// man getting out (the slot box top), from a second before the order.
/// Exit time and men hit until 5 s after the last man is out.
fn case24(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(24, "dismounting under MG 42 fire: door against sides");
    let mut ticks = [0u64; 2];
    let mut hits = [0u32; 2];
    let mut n = [0u32; 2];
    for (seed, flip) in env.runs() {
        r.runs += 1;
        for (m, over_sides) in [false, true].into_iter().enumerate() {
            let mut w = open_top_world(env, seed * 8 + flip as u64)?;
            let gun = weapon_idx(&w, "ww2:mg42_bipod")?;
            let (_, v, isq) = loaded_transport(
                &mut w,
                0,
                "ww2:ger_sdkfz251_1_squad",
                "ww2:ger_grenadiergruppe",
                at(500, 500),
                dir_deg(90),
                false,
            )?;
            let men = members(&w, isq);
            let before = men_hp(&w, &men);
            let hz = hz(&w);
            let fire = |w: &mut World, t: u64| {
                if !t.is_multiple_of(2) {
                    return;
                }
                let vp = w.entities().transform()[v.index()].pos;
                let foot = men
                    .iter()
                    .filter(|x| {
                        let i = x.index();
                        w.entities().kind()[i] == EntityKind::Soldier
                            && w.entities().unit()[i].riding.is_none()
                            && dist(pos(w, **x), vp.xy()) < 20.0
                    })
                    .min_by_key(|x| (dist(pos(w, **x), vp.xy()) * 1000.0) as i64);
                let aim = match foot {
                    Some(x) => {
                        let p = w.entities().transform()[x.index()].pos;
                        FxVec3::new(p.x, p.y, p.z + Fx::from_ratio(9, 10))
                    }
                    None => {
                        let Some(f) = w.game().and_then(|g| g.vehicle_derived().frame(v)) else {
                            return;
                        };
                        if f.n_riders == 0 {
                            return;
                        }
                        let b = f.riders[0];
                        b.center + b.axes.z * (b.half.z - Fx::from_ratio(15, 100))
                    }
                };
                let j = ((seed * 31 + t * 17) % 9) as i32 - 4;
                let jz = ((seed * 13 + t * 7) % 5) as i32 - 2;
                let aim = FxVec3::new(
                    aim.x,
                    aim.y + Fx::from_milli(j * 100),
                    aim.z + Fx::from_milli(jz * 100),
                );
                let speed = 755.0f64;
                let tt = 200.0 / speed;
                let drop = Fx::from_milli((0.5 * 9.81 * tt * tt * 1000.0) as i32);
                let from = FxVec3::new(
                    aim.x + Fx::from_int(200),
                    aim.y,
                    aim.z + Fx::from_ratio(1, 10),
                );
                w.fire_round_now(1, gun, from, FxVec3::new(aim.x, aim.y, aim.z + drop));
            };
            for t in 0..hz {
                fire(&mut w, t);
                w.step(&[]);
            }
            order(
                &mut w,
                0,
                vec![CommandKind::Disembark {
                    vehicles: vec![v],
                    over_sides,
                }],
            );
            let t0 = w.tick();
            let mut out_at = None;
            for t in 0..40 * hz {
                fire(&mut w, hz + t);
                w.step(&[]);
                let aboard = w.game().map_or(0, |g| g.riders_of(v).len());
                if aboard == 0 && out_at.is_none() {
                    out_at = Some(w.tick());
                }
                if out_at.is_some_and(|o| w.tick() >= o + 5 * hz) {
                    break;
                }
            }
            let after = men_hp(&w, &men);
            let h = before.iter().zip(&after).filter(|(b, a)| a < b).count() as u32;
            ticks[m] += out_at.unwrap_or(w.tick()) - t0;
            hits[m] += h;
            n[m] += 1;
            if env.verbose {
                eprintln!(
                    "case 24 seed {seed} flip {flip} sides {over_sides}: out in {} ticks, {h} men hit",
                    out_at.unwrap_or(w.tick()) - t0
                );
            }
        }
    }
    let mean = |a: u64, b: u32| a as f64 / b.max(1) as f64;
    let (door_s, sides_s) = (mean(ticks[0], n[0]) / 30.0, mean(ticks[1], n[1]) / 30.0);
    let faster = sides_s < door_s;
    r.row(
        "result",
        format!(
            "door: {door_s:.1} s, {:.2} men hit; sides: {sides_s:.1} s, {:.2} men hit",
            mean(hits[0] as u64, n[0]),
            mean(hits[1] as u64, n[1])
        ),
    );
    r.check_v(
        "sides faster",
        faster,
        f64::from(u8::from(faster)),
        "1 (S8a.toml T47a)",
        format!("door {door_s:.1} s, sides {sides_s:.1} s"),
    );
    r.check_v(
        "door exit s",
        true,
        round3(door_s),
        "report",
        "order to the last man out",
    );
    r.check_v(
        "sides exit s",
        true,
        round3(sides_s),
        "report",
        "order to the last man out",
    );
    r.check_v(
        "door men hit mean",
        true,
        round3(mean(hits[0] as u64, n[0])),
        "report",
        "men hit until 5 s after the last is out",
    );
    r.check_v(
        "sides men hit mean",
        true,
        round3(mean(hits[1] as u64, n[1])),
        "report",
        "men hit until 5 s after the last is out",
    );
    Ok(r)
}

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
    r.check_v(
        "sim tick P99 < 10 ms on 3 threads",
        all.p99_ms < 10.0,
        all.p99_ms,
        "< 10 ms",
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
    let all: [(u32, CaseFn); 39] = [
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
        (15, case15),
        (16, case16),
        (17, case17),
        (18, case18),
        (19, case19),
        (20, case20),
        (21, case21),
        (22, case22),
        (23, case23),
        (24, case24),
        (25, s8b::case25),
        (26, s8b::case26),
        (27, s8b::case27),
        (28, s8b::case28),
        (29, s8b::case29),
        (30, s8b::case30),
        (31, s8b::case31),
        (32, s8b::case32),
        (33, s9::case33),
        (34, s9::case34),
        (35, s9::case35),
        (37, s9::case37),
        (38, s9::case38),
        (39, s9::case39),
        (40, s9::case40),
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
