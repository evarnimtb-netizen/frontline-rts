//! `simulate`: run a headless battle and report hashes, timings and counts.
//!
//! A run either starts fresh from a scenario or continues from a save. It can
//! save once at a chosen tick, which together with `--load` gives the
//! save → load → continue check of §14. With `--paced` ticks start on a
//! wall-clock schedule (1/`tick_hz`), the way a client runs them, and the
//! report adds overrun counts and the §2.2 P95/P99 gate.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use frontline_content::{ContentError, Scenario};
use frontline_sim::{SimConfig, World};

use crate::lower;
use crate::report::{
    ConfigEcho, EntityCounts, Gate, PacedStats, REPORT_VERSION, Report, RunInfo, SavedInfo,
    SimStats, StageSummaries, Summary, TickHash, Timing, hex, ns, round3,
};

/// Options for [`simulate`]; also the clap definition of the subcommand.
#[derive(Clone, Debug, Default, clap::Args)]
pub struct SimulateArgs {
    /// Scenario file (TOML). Required unless --load is given; with --load it
    /// is checked against the configuration stored in the save.
    #[arg(long, value_name = "TOML", required_unless_present = "load")]
    pub scenario: Option<PathBuf>,
    /// Number of ticks to run.
    #[arg(long, value_name = "N")]
    pub ticks: u64,
    /// World seed (default: the scenario's `seed`).
    #[arg(long, value_name = "S", conflicts_with = "load")]
    pub seed: Option<u64>,
    /// Sim worker threads (default: all cores). Never changes results.
    #[arg(long, value_name = "T")]
    pub threads: Option<usize>,
    /// Write the JSON report here (`-` for stdout).
    #[arg(long, value_name = "JSON")]
    pub report: Option<PathBuf>,
    /// Absolute tick at which to write --save (default: the last tick).
    #[arg(long, value_name = "K", requires = "save")]
    pub save_at: Option<u64>,
    /// Save the full sim state to this file.
    #[arg(long, value_name = "FILE")]
    pub save: Option<PathBuf>,
    /// Continue from a save instead of starting fresh.
    #[arg(long, value_name = "FILE")]
    pub load: Option<PathBuf>,
    /// Run at the real tick rate (sleep to each tick's deadline).
    #[arg(long)]
    pub paced: bool,
    /// Accepted for symmetry with skirmish, armour-test and fortify-test,
    /// but refused: bench scenarios run without match rules or defs, so a
    /// tuning override cannot apply (S3b).
    #[arg(long = "rules-override", value_name = "FILE")]
    pub rules_override: Vec<PathBuf>,
}

/// Runs the simulation described by `args` and returns its report. Does not
/// write `args.report`; see [`write_report`].
pub fn simulate(args: &SimulateArgs) -> Result<Report> {
    if !args.rules_override.is_empty() {
        bail!(
            "--rules-override: `simulate` runs bench scenarios, which have no match rules or \
             defs to override; use `skirmish`, `armour-test` or `fortify-test`"
        );
    }
    let threads = match args.threads {
        Some(0) => bail!("--threads must be at least 1"),
        Some(t) => t,
        None => std::thread::available_parallelism().map_or(1, |n| n.get()),
    };
    // Parsed here, checked by `lower_checked` once the seed is known, so
    // content and sim-support problems are reported together.
    let scenario = args.scenario.as_deref().map(read_scenario).transpose()?;

    let (mut world, loaded_from) = match &args.load {
        Some(path) => {
            let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
            let world =
                World::load(&bytes, threads).map_err(|e| anyhow!("{}: {e}", path.display()))?;
            if let (Some(s), Some(sp)) = (&scenario, &args.scenario) {
                let want = lower_checked(s, world.config().seed, sp)?;
                check_same_config(&want, world.config(), sp, path)?;
            }
            (world, Some(path.display().to_string()))
        }
        None => {
            let (s, sp) = scenario
                .as_ref()
                .zip(args.scenario.as_ref())
                .ok_or_else(|| anyhow!("--scenario is required unless --load is given"))?;
            let config = lower_checked(s, args.seed.unwrap_or(s.seed), sp)?;
            (World::new(config, threads)?, None)
        }
    };

    let start_tick = world.tick();
    let end_tick = start_tick
        .checked_add(args.ticks)
        .ok_or_else(|| anyhow!("--ticks is too large"))?;
    let save_at = match (&args.save, args.save_at) {
        (Some(_), Some(k)) if k < start_tick || k > end_tick => bail!(
            "--save-at {k} is outside this run's ticks {start_tick}..={end_tick} \
             (it is an absolute tick number)"
        ),
        (Some(_), k) => Some(k.unwrap_or(end_tick)),
        (None, _) => None,
    };

    let hz = world.config().tick_rate.hz() as u64;
    let period = Duration::from_nanos(1_000_000_000 / hz);
    let n = args.ticks as usize;
    let mut stage_ns: [Vec<u64>; 10] = Default::default();
    for v in &mut stage_ns {
        v.reserve(n);
    }
    let mut tick_ns = Vec::with_capacity(n);
    let mut hashes = Vec::new();
    let mut saved = None;
    let (mut peak, mut live_sum) = (world.projectiles().live(), 0u64);
    let (mut overruns, mut late_starts, mut max_late) = (0u64, 0u64, Duration::ZERO);

    let save_to = args.save.as_deref();
    let maybe_save = |world: &World, saved: &mut Option<SavedInfo>| -> Result<()> {
        if let Some(path) = save_to
            && saved.is_none()
            && save_at == Some(world.tick())
        {
            *saved = Some(save_now(world, path)?);
        }
        Ok(())
    };

    maybe_save(&world, &mut saved)?;
    let loop_start = Instant::now();
    for i in 0..args.ticks {
        if args.paced {
            let deadline = loop_start + Duration::from_nanos(period.as_nanos() as u64 * i);
            let now = Instant::now();
            if now < deadline {
                std::thread::sleep(deadline - now);
            } else {
                let late = now - deadline;
                if late > Duration::from_millis(1) {
                    late_starts += 1;
                }
                max_late = max_late.max(late);
            }
        }
        let t = Instant::now();
        let published = world.step(&[]);
        let dt = t.elapsed();
        if dt > period {
            overruns += 1;
        }
        tick_ns.push(ns(dt));
        let st = world.stage_times();
        let parts = [
            st.intent,
            st.perception,
            st.movement,
            st.weapons,
            st.ballistics,
            st.damage,
            st.status,
            st.world_hash,
            st.snapshot,
            st.total(),
        ];
        for (v, d) in stage_ns.iter_mut().zip(parts) {
            v.push(ns(d));
        }
        if let Some(h) = published {
            hashes.push(TickHash {
                tick: world.tick(),
                hash: hex(h),
            });
        }
        let live = world.projectiles().live();
        peak = peak.max(live);
        live_sum += live as u64;
        maybe_save(&world, &mut saved)?;
    }
    let wall = loop_start.elapsed();

    let [
        intent,
        perception,
        movement,
        weapons,
        ballistics,
        damage,
        status,
        world_hash,
        snapshot,
        total,
    ] = stage_ns.map(|mut v| Summary::from_nanos(&mut v));
    let tick = Summary::from_nanos(&mut tick_ns);
    let paced = args.paced.then(|| PacedStats {
        period_ms: round3(period.as_secs_f64() * 1e3),
        overruns,
        late_starts,
        max_lateness_ms: round3(max_late.as_secs_f64() * 1e3),
        gate: Gate::check(&tick),
    });

    let e = world.entities();
    let p = world.projectiles();
    Ok(Report {
        report_version: REPORT_VERSION,
        tool: format!("frontline-cli {}", env!("CARGO_PKG_VERSION")),
        scenario,
        config: ConfigEcho::new(world.config()),
        run: RunInfo {
            threads,
            ticks: args.ticks,
            start_tick,
            end_tick: world.tick(),
            paced: args.paced,
            loaded_from,
            saved,
            wall_ms: round3(wall.as_secs_f64() * 1e3),
        },
        final_tick: world.tick(),
        final_hash: hex(world.state_hash()),
        hashes,
        timing: Timing {
            stages: StageSummaries {
                intent,
                perception,
                movement,
                weapons,
                ballistics,
                damage,
                status,
                world_hash,
                snapshot,
                total,
            },
            tick,
        },
        paced,
        entities: EntityCounts {
            soldiers: e.soldier_count(),
            entity_chunks: e.chunk_count(),
            id_high_water: e.allocator().high_water(),
            projectiles_live: p.live(),
            projectile_capacity: p.capacity(),
            projectiles_peak: peak,
            projectiles_mean: if n == 0 {
                p.live() as f64
            } else {
                round3(live_sum as f64 / n as f64)
            },
        },
        stats: SimStats::new(&world),
    })
}

/// Writes `report` as pretty JSON to `path`, or to stdout when `path` is `-`.
pub fn write_report(report: &Report, path: &Path) -> Result<()> {
    let mut json = serde_json::to_string_pretty(report)?;
    json.push('\n');
    if path == Path::new("-") {
        print!("{json}");
        return Ok(());
    }
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    fs::write(path, json).with_context(|| format!("writing {}", path.display()))
}

/// One-line human summary of a report.
pub fn summary_line(r: &Report) -> String {
    let mut s = format!(
        "ticks {}..{} on {} thread(s): final hash {}; tick mean {:.3} ms, p95 {:.3}, p99 {:.3}; \
         {} soldiers, {} projectiles (peak {}, {} dropped)",
        r.run.start_tick,
        r.final_tick,
        r.run.threads,
        r.final_hash,
        r.timing.tick.mean_ms,
        r.timing.tick.p95_ms,
        r.timing.tick.p99_ms,
        r.entities.soldiers,
        r.entities.projectiles_live,
        r.entities.projectiles_peak,
        r.stats.shots_dropped,
    );
    if let Some(p) = &r.paced {
        s += &format!(
            "; paced: {} overrun(s), gate {}",
            p.overruns,
            if p.gate.pass { "PASS" } else { "FAIL" }
        );
    }
    if let Some(sv) = &r.run.saved {
        s += &format!("; saved tick {} to {}", sv.tick, sv.path);
    }
    s
}

fn save_now(world: &World, path: &Path) -> Result<SavedInfo> {
    let bytes = world.save();
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    fs::write(path, &bytes).with_context(|| format!("writing save {}", path.display()))?;
    Ok(SavedInfo {
        tick: world.tick(),
        path: path.display().to_string(),
        bytes: bytes.len(),
        hash: hex(world.state_hash()),
        projectiles_live: world.projectiles().live(),
    })
}

/// Reads and parses a scenario without semantic checks.
pub fn read_scenario(path: &Path) -> Result<Scenario> {
    let src = fs::read_to_string(path).map_err(|source| {
        content_error(ContentError::Io {
            path: path.to_owned(),
            source,
        })
    })?;
    Scenario::parse_toml(&src, &path.display().to_string()).map_err(content_error)
}

/// Content validation plus sim support, then lowering.
fn lower_checked(s: &Scenario, seed: u64, path: &Path) -> Result<SimConfig> {
    lower::sim_config(s, seed).map_err(|issues| {
        content_error(ContentError::Invalid {
            origin: path.display().to_string(),
            issues,
        })
    })
}

fn check_same_config(
    want: &SimConfig,
    got: &SimConfig,
    scenario: &Path,
    save: &Path,
) -> Result<()> {
    if want == got {
        return Ok(());
    }
    let (a, b) = (
        serde_json::to_value(ConfigEcho::new(want))?,
        serde_json::to_value(ConfigEcho::new(got))?,
    );
    let mut diffs: Vec<String> = a
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(k, v)| b.get(k.as_str()) != Some(v))
        .map(|(k, v)| format!("{k}: scenario {v}, save {}", b[k.as_str()]))
        .collect();
    if diffs.is_empty() {
        diffs.push("internal sim parameters differ".into());
    }
    bail!(
        "{} does not match the configuration stored in {}:\n  - {}\n\
         (drop --scenario to continue the save as it is)",
        scenario.display(),
        save.display(),
        diffs.join("\n  - ")
    )
}

/// Keeps multi-line content errors readable instead of nesting them.
fn content_error(e: ContentError) -> anyhow::Error {
    anyhow!("{e}")
}
