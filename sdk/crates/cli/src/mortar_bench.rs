//! `mortar-bench` (S5b): the same fire mission observed and unobserved
//! (docs/slices/S5b-mortars.md "The CLI bench for the 3x ratio", targets
//! T1 and T2).
//!
//! Flat open ground (Kessel Ridge with its relief and woods taken away),
//! nobody else on the map. For each range, seed and faction one mortar
//! squad fires the same `--rounds` mission at a ground point straight
//! east of its tube, three times:
//!
//! 1. **Observed.** The squad's own leader has walked out as a spotter
//!    ([`SPOT`]: 120 m toward the target, 30 m to the side, inside the
//!    150 m voice link) and sees the point and the bursts: the mission
//!    ranges in.
//! 2. **Map fire.** Nobody observes (both observe ranges set to 0 in this
//!    run's setup); the target cell is marked in the player's knowledge
//!    map before the order.
//! 3. **Observed, unseen bursts.** As 1, but smoke is laid on the
//!    spotter's sight line right after the order (and again every 20 s):
//!    the point was seen when the call went out, the bursts are not, so
//!    the mission fires on its first data.
//!
//! Each burst's miss (burst point to target, on the ground plane) comes
//! from the sim's diagnostic burst log. Pass: the pooled ratio of the mean
//! miss, map fire over observed, is 2.5-3.5; hard checks: in run 3 the aim
//! error never moves from its start, in run 1 it falls by
//! `ranging_step_pct` per seen burst. Each range's own ratio is reported
//! against 2.0-4.5.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Result, anyhow, bail};
use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::fire_mission::mode;
use frontline_sim::{Command, CommandKind, Fx, FxVec2, Mission, Order, PlayerId, SquadId, World};
use serde::Serialize;

use crate::skirmish::mods_dir;

/// Pooled ratio band (pass) and each range's report band.
pub const POOLED: (f64, f64) = (2.5, 3.5);
pub const PER_RANGE: (f64, f64) = (2.0, 4.5);
/// The tube stands here (m); targets lie straight east of it.
const TUBE: (i32, i32) = (250, 500);
/// The spotter's spot relative to the tube (m): toward the target and to
/// the side, inside the voice link.
const SPOT: (i32, i32) = (120, 30);
/// Time the spotter is given to walk out (s).
const WALK_S: u64 = 75;

/// Options for [`mortar_bench`]; also the clap definition.
#[derive(Clone, Debug, clap::Args)]
pub struct BenchArgs {
    /// Seeds `first-seed .. first-seed + seeds` are run, each with both
    /// factions' mortars.
    #[arg(long, value_name = "N", default_value_t = 50)]
    pub seeds: u32,
    #[arg(long, value_name = "S", default_value_t = 1)]
    pub first_seed: u64,
    /// Rounds in the mission (the done-when's "same mission").
    #[arg(long, value_name = "R", default_value_t = 8)]
    pub rounds: u16,
    /// Ranges from tube to target (m).
    #[arg(long, value_name = "M,M,..", value_delimiter = ',', default_values_t = [200u32, 300, 400, 500])]
    pub ranges: Vec<u32>,
    /// Runs played at once (each on one sim thread). Never changes results.
    #[arg(long, value_name = "J", default_value_t = 3)]
    pub jobs: usize,
    /// Mod directory (default: `mods/ww2` here or in the source tree).
    #[arg(long, value_name = "DIR")]
    pub mods: Option<PathBuf>,
    /// Print the report as JSON instead of text.
    #[arg(long)]
    pub json: bool,
    /// Tuning override: a partial TOML file merged onto the rules and defs.
    /// Repeatable; `docs/modding/tuning.md`.
    #[arg(long = "rules-override", value_name = "FILE")]
    pub rules_override: Vec<PathBuf>,
}

impl Default for BenchArgs {
    fn default() -> BenchArgs {
        BenchArgs {
            seeds: 50,
            first_seed: 1,
            rounds: 8,
            ranges: vec![200, 300, 400, 500],
            jobs: 3,
            mods: None,
            json: false,
            rules_override: Vec::new(),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunKind {
    Observed,
    Map,
    Unseen,
}

/// One mission.
#[derive(Clone, Debug, Serialize)]
pub struct BenchRun {
    pub seed: u64,
    pub faction: String,
    pub range_m: u32,
    pub kind: RunKind,
    /// The miss of each burst (m), in burst order.
    pub misses_m: Vec<f64>,
    /// The mission's aim error at the order (m).
    pub start_error_m: f64,
    /// ... and after its last burst while it was live (m).
    pub end_error_m: f64,
    pub adjusting_rounds: u32,
    pub seen: u32,
    pub mean_flight_s: f64,
    /// Charges used (index, 0 = the lowest), each once.
    pub charges: Vec<u8>,
    /// The hard check of this kind of run held (run 1: the error fell by
    /// the step per seen burst; run 3: it never moved; run 2: no ranging).
    pub ranging_ok: bool,
}

/// One range's means over its runs.
#[derive(Clone, Debug, Serialize)]
pub struct RangeRow {
    pub range_m: u32,
    pub runs: u32,
    pub observed_m: f64,
    pub map_m: f64,
    pub unseen_m: f64,
    /// Map fire over observed.
    pub ratio: f64,
    pub unseen_ratio: f64,
    pub in_band: bool,
    pub adjusting_rounds: f64,
    pub seen: f64,
    pub flight_s: f64,
    pub charges: Vec<u8>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BenchReport {
    pub rounds: u16,
    pub seeds: u32,
    pub rows: Vec<RangeRow>,
    pub pooled_observed_m: f64,
    pub pooled_map_m: f64,
    pub pooled_unseen_m: f64,
    pub pooled_ratio: f64,
    pub ranging_failures: Vec<String>,
    pub pass: bool,
    pub wall_s: f64,
    pub runs: Vec<BenchRun>,
}

fn mean(v: impl Iterator<Item = f64>) -> f64 {
    let (mut s, mut n) = (0.0, 0u32);
    for x in v {
        s += x;
        n += 1;
    }
    if n == 0 { 0.0 } else { s / f64::from(n) }
}

fn metres(f: Fx) -> f64 {
    f64::from(f.raw()) / 65536.0
}

fn xy(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

fn mission(w: &World, sq: SquadId) -> Option<Mission> {
    match &w.game()?.squad(sq)?.order {
        Order::FireMission { m, .. } => Some(*m),
        _ => None,
    }
}

fn step_cmd(w: &mut World, p: u8, kind: CommandKind) {
    let seq = w.tick() as u32;
    w.step(&[Command {
        player: PlayerId(p),
        seq,
        kind,
    }]);
}

struct Env {
    defs: ModDefs,
    rules: String,
}

impl Env {
    fn setup(&self, seed: u64, kind: RunKind) -> Result<MatchSetup> {
        let mut req = MatchRequest::skirmish(seed, "ww2:germany", "ww2:usa", Difficulty::Normal);
        req.rules = self.rules.clone();
        let mut s = self.defs.resolve(&req).map_err(|e| anyhow!("{e}"))?;
        s.players.iter_mut().for_each(|p| p.is_ai = false);
        s.map.terrain.octaves.clear();
        s.map.terrain.bands.clear();
        s.map.woods.clear();
        let fm = s
            .rules
            .fire_mission
            .as_mut()
            .ok_or_else(|| anyhow!("the rules have no [fire_mission]: nothing to bench"))?;
        if kind == RunKind::Map {
            fm.observe_leader_m = 0;
            fm.observe_observer_m = 0;
        }
        Ok(s)
    }
}

/// One mission of `p`'s mortar at `range` m.
fn run(
    env: &Env,
    args: &BenchArgs,
    seed: u64,
    p: u8,
    range: u32,
    kind: RunKind,
) -> Result<BenchRun> {
    let setup = env.setup(seed, kind)?;
    let card = ["ww2:ger_mortar_team", "ww2:usa_mortar_team"][p as usize];
    let faction = setup.factions[setup.players[p as usize].faction as usize]
        .id
        .clone();
    let ty = setup
        .squad_type_index(card)
        .ok_or_else(|| anyhow!("no {card}"))?;
    let step_pct = setup
        .rules
        .fire_mission
        .as_ref()
        .expect("checked")
        .ranging_step_pct;
    let hz = u64::from(setup.tick_hz);
    let mut w = World::new_match(setup, 1)?;
    let east = FxVec2::new(Fx::ONE, Fx::ZERO);
    let tube = xy(TUBE.0, TUBE.1);
    let target = xy(TUBE.0 + range as i32, TUBE.1);
    let sq = w
        .spawn_squad_at(PlayerId(p), ty, tube, east)
        .ok_or_else(|| anyhow!("could not place the {card}"))?;
    let spot = tube + xy(SPOT.0, SPOT.1);
    if kind == RunKind::Map {
        if !w.mark_intel(PlayerId(p), target) {
            bail!("could not mark the target in the knowledge map");
        }
        for _ in 0..5 * hz {
            w.step(&[]);
        }
    } else {
        let leader = {
            let g = w.game().expect("match");
            g.squad_leader(w.entities(), g.squad(sq).expect("live"))
                .ok_or_else(|| anyhow!("the {card} has no leader"))?
        };
        step_cmd(
            &mut w,
            p,
            CommandKind::MoveUnits {
                units: vec![leader],
                target: spot,
            },
        );
        for _ in 0..WALK_S * hz {
            w.step(&[]);
        }
    }
    w.enable_burst_log();
    step_cmd(
        &mut w,
        p,
        CommandKind::FireMission {
            squad: sq,
            target,
            rounds: args.rounds,
        },
    );
    if let Some(e) = w.snapshot().fire_mission_refusals.first() {
        bail!(
            "seed {seed} {card} {range} m {kind:?}: refused: {}",
            e.refusal.text()
        );
    }
    let m0 = mission(&w, sq).ok_or_else(|| anyhow!("no mission"))?;
    let want = if kind == RunKind::Map {
        mode::MAP
    } else {
        mode::OBSERVED
    };
    if m0.mode != want {
        bail!(
            "seed {seed} {card} {range} m {kind:?}: the mission started as {}",
            mode::NAMES[m0.mode as usize]
        );
    }
    let t0 = w.tick();
    let mid = spot + (target - spot) * Fx::from_ratio(1, 2);
    let side = FxVec2::new(Fx::ZERO, Fx::from_int(12));
    let mut charges: Vec<u8> = Vec::new();
    let mut last_fired = 0u16;
    let mut flights = Vec::new();
    loop {
        if kind == RunKind::Unseen && (w.tick() - t0).is_multiple_of(20 * hz) {
            for c in [mid, mid + side, mid - side] {
                w.smoke_now(c);
            }
        }
        w.step(&[]);
        match mission(&w, sq) {
            Some(m) => {
                if m.fired > last_fired {
                    last_fired = m.fired;
                    if !charges.contains(&m.charge) {
                        charges.push(m.charge);
                    }
                    flights.push(f64::from(m.flight) / hz as f64);
                }
            }
            None => break,
        }
        if w.tick() > t0 + 600 * hz {
            bail!("seed {seed} {card} {range} m {kind:?}: the mission never ended");
        }
    }
    for _ in 0..30 * hz {
        w.step(&[]);
    }
    let log = w.take_burst_log();
    let misses_m: Vec<f64> = log
        .iter()
        .filter(|b| b.mission == m0.id)
        .map(|b| metres((b.pos.xy() - target).length()))
        .collect();
    if misses_m.len() != args.rounds as usize {
        bail!(
            "seed {seed} {card} {range} m {kind:?}: {} of {} bombs burst",
            misses_m.len(),
            args.rounds
        );
    }
    let start = metres(m0.err0);
    let live: Vec<_> = log
        .iter()
        .filter(|b| b.mission == m0.id && b.live)
        .collect();
    let end = live.last().map_or(start, |b| metres(b.offset));
    let seen = live.iter().filter(|b| b.seen).count() as u32;
    let ranging_ok = match kind {
        RunKind::Map | RunKind::Unseen => {
            seen == 0 && live.iter().all(|b| (metres(b.offset) - start).abs() < 0.01)
        }
        RunKind::Observed => {
            let mut e = start;
            let mut ok = true;
            for (k, b) in live.iter().enumerate() {
                if b.seen {
                    e = e * f64::from(step_pct) / 100.0;
                }
                ok &= (metres(b.offset) - e).abs() < 0.01 * (k + 1) as f64;
            }
            ok && seen > 0
        }
    };
    let fm = &w.game().expect("match").players()[p as usize].fm;
    Ok(BenchRun {
        seed,
        faction,
        range_m: range,
        kind,
        misses_m,
        start_error_m: start,
        end_error_m: end,
        adjusting_rounds: fm.adjust,
        seen,
        mean_flight_s: mean(flights.into_iter()),
        charges,
        ranging_ok,
    })
}

/// Runs the bench (see the module docs).
pub fn mortar_bench(args: &BenchArgs) -> Result<BenchReport> {
    if args.rounds == 0 || args.rounds > 50 {
        bail!("--rounds must be 1..=50");
    }
    if args.ranges.is_empty() || args.ranges.iter().any(|&r| !(100..=700).contains(&r)) {
        bail!("--ranges must be 100..=700 m (the tube stands 250 m from the map's west edge)");
    }
    let started = Instant::now();
    let dir = mods_dir(args.mods.as_deref())?;
    let (mut defs, _) = crate::overrides::load_defs(Some(&dir), &args.rules_override)?;
    let rules = defs
        .add_rules_override(&dir.join("tests/rules_test_unlock.toml"))
        .map_err(|e| anyhow!("{e}"))?;
    let env = Env { defs, rules };
    let mut jobs: Vec<(u64, u8, u32, RunKind)> = Vec::new();
    for &range in &args.ranges {
        for seed in args.first_seed..args.first_seed + u64::from(args.seeds) {
            for p in [0u8, 1] {
                for kind in [RunKind::Observed, RunKind::Map, RunKind::Unseen] {
                    jobs.push((seed, p, range, kind));
                }
            }
        }
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: std::sync::Mutex<Vec<(usize, Result<BenchRun>)>> =
        std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..args.jobs.max(1).min(jobs.len()) {
            scope.spawn(|| {
                loop {
                    let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(&(seed, p, range, kind)) = jobs.get(k) else {
                        break;
                    };
                    let r = run(&env, args, seed, p, range, kind);
                    results.lock().expect("no panics").push((k, r));
                }
            });
        }
    });
    let mut results = results.into_inner().expect("no panics");
    results.sort_by_key(|r| r.0);
    let mut runs = Vec::with_capacity(results.len());
    for (_, r) in results {
        runs.push(r?);
    }
    let miss = |rs: &[&BenchRun], kind: RunKind| {
        mean(
            rs.iter()
                .filter(|r| r.kind == kind)
                .flat_map(|r| r.misses_m.iter().copied()),
        )
    };
    let mut rows = Vec::new();
    for &range in &args.ranges {
        let rs: Vec<&BenchRun> = runs.iter().filter(|r| r.range_m == range).collect();
        let obs: Vec<&&BenchRun> = rs.iter().filter(|r| r.kind == RunKind::Observed).collect();
        let (o, m, u) = (
            miss(&rs, RunKind::Observed),
            miss(&rs, RunKind::Map),
            miss(&rs, RunKind::Unseen),
        );
        let ratio = if o > 0.0 { m / o } else { 0.0 };
        let mut charges: Vec<u8> = rs.iter().flat_map(|r| r.charges.iter().copied()).collect();
        charges.sort_unstable();
        charges.dedup();
        rows.push(RangeRow {
            range_m: range,
            runs: obs.len() as u32,
            observed_m: o,
            map_m: m,
            unseen_m: u,
            ratio,
            unseen_ratio: if o > 0.0 { u / o } else { 0.0 },
            in_band: (PER_RANGE.0..=PER_RANGE.1).contains(&ratio),
            adjusting_rounds: mean(obs.iter().map(|r| f64::from(r.adjusting_rounds))),
            seen: mean(obs.iter().map(|r| f64::from(r.seen))),
            flight_s: mean(rs.iter().map(|r| r.mean_flight_s)),
            charges,
        });
    }
    let all: Vec<&BenchRun> = runs.iter().collect();
    let (po, pm, pu) = (
        miss(&all, RunKind::Observed),
        miss(&all, RunKind::Map),
        miss(&all, RunKind::Unseen),
    );
    let pooled_ratio = if po > 0.0 { pm / po } else { 0.0 };
    let ranging_failures: Vec<String> = runs
        .iter()
        .filter(|r| !r.ranging_ok)
        .map(|r| {
            format!(
                "seed {} {} {} m {:?}: error {:.2} -> {:.2} m, {} seen",
                r.seed, r.faction, r.range_m, r.kind, r.start_error_m, r.end_error_m, r.seen
            )
        })
        .collect();
    let pass = (POOLED.0..=POOLED.1).contains(&pooled_ratio) && ranging_failures.is_empty();
    Ok(BenchReport {
        rounds: args.rounds,
        seeds: args.seeds,
        rows,
        pooled_observed_m: po,
        pooled_map_m: pm,
        pooled_unseen_m: pu,
        pooled_ratio,
        ranging_failures,
        pass,
        wall_s: started.elapsed().as_secs_f64(),
        runs,
    })
}

/// Human summary of a report.
pub fn summary_text(r: &BenchReport) -> String {
    let mut out = format!(
        "mortar-bench: the same {}-round mission observed (a spotter in the voice link), as map fire, and observed with the bursts hidden by smoke; {} seeds x both factions\n",
        r.rounds, r.seeds
    );
    out += "  range  runs  observed m  map m  unseen m  map/obs  unseen/obs  adjusting  seen  flight s  charges\n";
    for row in &r.rows {
        out += &format!(
            "  {:>5}  {:>4}  {:>10.1}  {:>5.1}  {:>8.1}  {:>7.2}{}  {:>10.2}  {:>9.2}  {:>4.2}  {:>8.1}  {:?}\n",
            row.range_m,
            row.runs,
            row.observed_m,
            row.map_m,
            row.unseen_m,
            row.ratio,
            if row.in_band { " " } else { "!" },
            row.unseen_ratio,
            row.adjusting_rounds,
            row.seen,
            row.flight_s,
            row.charges,
        );
    }
    out += &format!(
        "  pooled: observed {:.1} m, map {:.1} m, unseen {:.1} m; map/observed {:.2} (pass {:.1}-{:.1}; each range reported against {:.1}-{:.1}, ! outside)\n",
        r.pooled_observed_m,
        r.pooled_map_m,
        r.pooled_unseen_m,
        r.pooled_ratio,
        POOLED.0,
        POOLED.1,
        PER_RANGE.0,
        PER_RANGE.1
    );
    if r.ranging_failures.is_empty() {
        out += "  ranging: every observed run ranged by the step per seen burst; no unseen or map run ranged: PASS\n";
    } else {
        out += &format!(
            "  ranging: {} runs broke the rule: FAIL\n",
            r.ranging_failures.len()
        );
        for f in r.ranging_failures.iter().take(10) {
            out += &format!("    {f}\n");
        }
    }
    out += &format!(
        "mortar-bench: {} ({:.1} s)",
        if r.pass { "PASS" } else { "FAIL" },
        r.wall_s
    );
    out
}
