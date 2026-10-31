//! `simbench`: kernel (LOS ray, ORCA) and composed-tick benchmarks for the
//! §2.2 M0 spike gates.
//!
//! ```text
//! simbench [--threads 3] [--json] [--out FILE] los
//! simbench orca --agents 700,2000
//! simbench tick --tier battalion --seconds 30 --paced
//! simbench all [--quick]
//! ```
//!
//! Build with `--release` (or `--profile bench-release`); debug numbers are
//! meaningless.

use std::path::PathBuf;

use anyhow::Context;
use clap::{Args, Parser, Subcommand};

use frontline_simbench::los::{self, LosBenchConfig};
use frontline_simbench::orca::{self, OrcaBenchConfig};
use frontline_simbench::report::{self, Machine, Report};
use frontline_simbench::tick::{self, TickBenchConfig, Tier};

#[derive(Parser, Debug)]
#[command(
    name = "simbench",
    version,
    about = "Frontline M0 spike benchmarks (§2.2 gates)"
)]
struct Cli {
    /// Worker threads (default 3 = the Deck's sim thread count, §2.2).
    #[arg(long, global = true, default_value_t = 3)]
    threads: usize,
    /// Print JSON instead of text.
    #[arg(long, global = true)]
    json: bool,
    /// Also write the JSON report to this file.
    #[arg(long, global = true)]
    out: Option<PathBuf>,
    /// World / scenario seed.
    #[arg(long, global = true, default_value_t = 1)]
    seed: u64,
    /// Machine label for the report (default: detected CPU name).
    #[arg(long, global = true)]
    machine: Option<String>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// LOS rays over a 1 m heightfield: linear vs tiled vs Morton, with and without max-mip.
    Los(LosArgs),
    /// Fixed-point ORCA avoidance with ≤8 neighbours from a 16 m spatial hash.
    Orca(OrcaArgs),
    /// Composed tick (`World::step` + ray budget) at Battalion/Regiment caps.
    Tick(TickArgs),
    /// Run los, orca and tick (both tiers).
    All {
        /// Shorter runs (smoke test, noisier numbers).
        #[arg(long)]
        quick: bool,
        /// Pace the composed tick at the tick rate.
        #[arg(long)]
        paced: bool,
    },
}

#[derive(Args, Debug, Clone)]
struct LosArgs {
    /// Map side in metres (power of two); 1 m cells.
    #[arg(long, default_value_t = 4096)]
    map_m: u32,
    #[arg(long, default_value_t = 300_000)]
    rays: usize,
    #[arg(long, default_value_t = 5)]
    reps: u32,
    /// Max-mip pyramid levels (level l = 2^l m blocks).
    #[arg(long, default_value_t = 6)]
    mip_levels: u32,
    #[arg(long, default_value_t = 300)]
    min_m: u32,
    #[arg(long, default_value_t = 600)]
    max_m: u32,
}

#[derive(Args, Debug, Clone)]
struct OrcaArgs {
    /// Agent counts to run.
    #[arg(long, value_delimiter = ',', default_value = "700,2000")]
    agents: Vec<usize>,
    #[arg(long, default_value_t = 900)]
    ticks: u32,
    #[arg(long, default_value_t = 60)]
    warmup: u32,
    /// Neighbour search radius in metres. Default 8 (pessimistic: about 6.7
    /// of the 8 neighbour slots used); the M0 sim uses 4.
    #[arg(long, default_value_t = 8.0)]
    neighbour_dist: f64,
}

#[derive(Args, Debug, Clone)]
struct TickArgs {
    /// Tier(s) to run.
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        default_value = "battalion,regiment"
    )]
    tier: Vec<Tier>,
    /// Measured seconds of sim time.
    #[arg(long, default_value_t = 20.0)]
    seconds: f64,
    /// Warm-up seconds (fills the projectile pool), not measured.
    #[arg(long, default_value_t = 5.0)]
    warmup: f64,
    /// Sleep to the 30 Hz deadline after each tick, like the real sim thread.
    #[arg(long)]
    paced: bool,
    /// Skip the synthetic ray budget.
    #[arg(long)]
    no_rays: bool,
    /// LOS variant for the ray budget (e.g. tiled16+maxmip); default: fastest by calibration.
    #[arg(long)]
    los_variant: Option<String>,
    /// LOS heightfield side (m, power of two).
    #[arg(long, default_value_t = 4096)]
    los_map_m: u32,
    #[arg(long, default_value_t = 6)]
    mip_levels: u32,
    /// Override the sim map side (m, multiple of 64).
    #[arg(long)]
    sim_map_m: Option<i32>,
    /// Spawn the soldiers in a centred square of this side (m) instead of
    /// over the whole sim map, for contact-density crowds.
    #[arg(long)]
    spawn_m: Option<i32>,
    /// Weapon cooldown range in ticks, "min,max" (lower = heavier fire). The
    /// default keeps the projectile pool near its tier cap.
    #[arg(long, value_delimiter = ',', num_args = 2, default_values_t = [2u16, 8])]
    cooldown: Vec<u16>,
}

impl LosArgs {
    fn config(&self, cli: &Cli) -> LosBenchConfig {
        LosBenchConfig {
            map_m: self.map_m,
            rays: self.rays,
            reps: self.reps,
            mip_levels: self.mip_levels,
            min_m: self.min_m,
            max_m: self.max_m,
            seed: cli.seed,
            threads: cli.threads,
        }
    }
}

impl OrcaArgs {
    fn config(&self, cli: &Cli) -> OrcaBenchConfig {
        OrcaBenchConfig {
            agents: self.agents.clone(),
            ticks: self.ticks,
            warmup: self.warmup,
            threads: cli.threads,
            seed: cli.seed,
            neighbour_dist_m: self.neighbour_dist,
        }
    }
}

impl TickArgs {
    fn config(&self, cli: &Cli) -> TickBenchConfig {
        TickBenchConfig {
            tiers: self.tier.clone(),
            seconds: self.seconds,
            warmup_s: self.warmup,
            paced: self.paced,
            threads: cli.threads,
            seed: cli.seed,
            rays: !self.no_rays,
            los_variant: self.los_variant.clone(),
            los_map_m: self.los_map_m,
            mip_levels: self.mip_levels,
            sim_map_m: self.sim_map_m,
            spawn_area_m: self.spawn_m,
            cooldown: (self.cooldown[0], self.cooldown[1]),
        }
    }
}

fn progress(json: bool, msg: &str) {
    if !json {
        eprintln!("simbench: {msg}");
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    anyhow::ensure!(cli.threads >= 1, "--threads must be at least 1");
    let machine = Machine::detect(cli.machine.clone());
    let mut rep = Report {
        schema: report::SCHEMA,
        baseline_notice: machine.baseline_notice(),
        machine,
        threads: cli.threads,
        seed: cli.seed,
        profile: report::build_profile(),
        los: None,
        orca: Vec::new(),
        tick: Vec::new(),
        gates: Vec::new(),
    };

    let (los_args, orca_args, tick_args) = match &cli.cmd {
        Cmd::Los(a) => (Some(a.clone()), None, None),
        Cmd::Orca(a) => (None, Some(a.clone()), None),
        Cmd::Tick(a) => (None, None, Some(a.clone())),
        Cmd::All { quick, paced } => {
            let mut l = LosArgs::parse_default();
            let mut o = OrcaArgs::parse_default();
            let mut t = TickArgs::parse_default();
            t.paced = *paced;
            if *quick {
                l.rays = 50_000;
                l.reps = 2;
                o.ticks = 150;
                o.warmup = 15;
                t.seconds = 3.0;
                t.warmup = 1.0;
            }
            (Some(l), Some(o), Some(t))
        }
    };

    if let Some(a) = los_args {
        progress(cli.json, "LOS kernel (building 3 layouts + pyramids)...");
        rep.los = Some(los::run_bench(&a.config(&cli))?);
    }
    if let Some(a) = orca_args {
        progress(cli.json, "ORCA kernel...");
        rep.orca = orca::run_bench(&a.config(&cli))?;
    }
    if let Some(a) = tick_args {
        progress(
            cli.json,
            "composed tick (calibrating LOS variant, then running tiers)...",
        );
        rep.tick = tick::run_bench(&a.config(&cli))?.0;
    }
    rep.gates = report::gate_checks(&rep.machine, rep.los.as_ref(), &rep.orca, &rep.tick);

    let json = serde_json::to_string_pretty(&rep)?;
    if let Some(path) = &cli.out {
        std::fs::write(path, &json).with_context(|| format!("writing {}", path.display()))?;
        progress(cli.json, &format!("wrote {}", path.display()));
    }
    if cli.json {
        println!("{json}");
    } else {
        print!("{}", rep.to_text());
    }
    Ok(())
}

/// Default-valued argument structs for `all`.
trait ParseDefault: Sized {
    fn parse_default() -> Self;
}

impl<T: clap::FromArgMatches + clap::Args> ParseDefault for T {
    fn parse_default() -> Self {
        let cmd = T::augment_args(clap::Command::new("defaults"));
        let m = cmd.get_matches_from(["defaults"]);
        T::from_arg_matches(&m).expect("defaults parse")
    }
}
