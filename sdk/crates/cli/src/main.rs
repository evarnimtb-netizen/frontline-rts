//! `frontline-cli`: headless tooling for Frontline content and the sim (§8.1).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use frontline_cli::skirmish::ReportFormat;
use frontline_cli::{
    ArmourArgs, BallisticsArgs, FortifyArgs, SimulateArgs, SkirmishArgs, armour_test, ballistics,
    desync_diff, fortify, overrides, schema, simulate, skirmish, validate,
};

#[derive(Parser)]
#[command(
    name = "frontline-cli",
    version,
    about = "Headless tooling for Frontline content and the sim"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a headless battle and report hashes, timings and counts.
    Simulate(SimulateArgs),
    /// Play a whole skirmish headlessly (AI vs AI) and print winner and stats.
    ///
    /// Exits 0 when the match has a winner, 3 when it ended in a draw or hit
    /// the --minutes limit, 4 when it has a winner but an S3 check failed
    /// (each AI fields a vehicle and an AT team, at least one penetration).
    ///
    /// A/B testing (S3b): `--seeds N [--both-bases] --summary` plays N seeds
    /// (both base orders) in parallel and prints one table; add
    /// `--rules-override FILE` to try a tuning change, and `--ab` to play
    /// the same matches without it too and print the change of each
    /// headline number. Exits 0 when every match ran.
    Skirmish(SkirmishArgs),
    /// Stage trench-vs-open fights and check the S1 cover thresholds.
    ///
    /// Exits 0 when both experiments pass, 1 otherwise.
    FortifyTest(FortifyArgs),
    /// Stage the S3 armour cases (penetration statistics, hull-down, duels,
    /// AT teams, satchels, HE, mortars, enfilade, trench crossing, AT
    /// ditch; --stress adds the stress case) and check their mechanics.
    ///
    /// Exits 0 when every hard check passes, 1 otherwise. Report rows (with
    /// the owner's targets) never fail the run.
    ArmourTest(ArmourArgs),
    /// Compare two sim saves field by field (a multiplayer desync dump's
    /// `paused.sav` files, or `checkpoint.sav` from both clients).
    ///
    /// Prints the first differences by entity, squad, player and point id.
    /// When each save sits next to a dump's `manifest.json`, it also reads
    /// the desync tick and, for `paused.sav`, the running worlds' state
    /// hashes, which catch a divergence only in derived data. Exits 1 when
    /// the saves differ in any field or hash (or those manifest hashes
    /// differ), 0 when they are identical.
    DesyncDiff {
        #[arg(value_name = "A.sav")]
        a: PathBuf,
        #[arg(value_name = "B.sav")]
        b: PathBuf,
        /// Differences to print.
        #[arg(long, value_name = "N", default_value_t = desync_diff::DEFAULT_LIMIT)]
        max: usize,
    },
    /// Penetration of one round against one armour plate (S3).
    ///
    /// One shot: --ammo, --target, --zone, --range, --angle, --descent,
    /// --track-band. --table: one ammo against every zone of a target.
    /// --golden FILE: run golden rows (exits 1 if any fails). The math is
    /// the sim's own (`frontline_sim::armour`).
    Ballistics(BallisticsArgs),
    /// Export JSON Schemas for every def type.
    Schema {
        /// Output directory (created if missing).
        #[arg(long, value_name = "DIR")]
        out: PathBuf,
    },
    /// Check scenario files or mod folders (a folder with `mod.toml`: every
    /// def, the rules overrides in its `tests/`, and every rules × map
    /// resolve) and list every problem with a suggested fix.
    Validate {
        #[arg(required = true, value_name = "TOML|MOD_DIR")]
        files: Vec<PathBuf>,
        /// Print machine-readable JSON instead of text.
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    match cli.command {
        Command::Simulate(args) => {
            let report = simulate::simulate(&args)?;
            if let Some(path) = &args.report {
                simulate::write_report(&report, path)?;
            }
            eprintln!("{}", simulate::summary_line(&report));
            Ok(ExitCode::SUCCESS)
        }
        Command::Skirmish(args) if args.ab => {
            let report = skirmish::skirmish_ab(&args)?;
            match args.report {
                ReportFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
                ReportFormat::Text => print!("{}", skirmish::ab_text(&report)),
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Skirmish(args) if args.seeds.is_some() => {
            let report = skirmish::skirmish_many(&args)?;
            match (args.report, args.summary) {
                (ReportFormat::Json, _) => println!("{}", serde_json::to_string_pretty(&report)?),
                (ReportFormat::Text, true) => print!("{}", skirmish::many_text(&report)),
                (ReportFormat::Text, false) => {
                    for l in &report.overrides {
                        eprintln!("{l}");
                    }
                    for m in &report.matches {
                        println!("{}\n", skirmish::summary_text(&m.report));
                    }
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Skirmish(args) => {
            overrides::announce(args.mods.as_deref(), &args.rules_override)?;
            let report = skirmish::skirmish(&args)?;
            match args.report {
                ReportFormat::Text => println!("{}", skirmish::summary_text(&report)),
                ReportFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
            }
            Ok(if report.winner.is_none() {
                ExitCode::from(3)
            } else if !report.s3_failures.is_empty() {
                ExitCode::from(4)
            } else {
                ExitCode::SUCCESS
            })
        }
        Command::FortifyTest(args) => {
            overrides::announce(args.mods.as_deref(), &args.rules_override)?;
            let report = fortify::fortify_test(&args)?;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("{}", fortify::summary_text(&report));
            }
            Ok(if report.pass {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        Command::ArmourTest(args) => {
            overrides::announce(args.mods.as_deref(), &args.rules_override)?;
            let report = armour_test::armour_test(&args)?;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("{}", armour_test::summary_text(&report));
            }
            Ok(if report.pass {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        Command::DesyncDiff { a, b, max } => {
            let read = |p: &PathBuf| {
                std::fs::read(p).map_err(|e| anyhow::anyhow!("reading {}: {e}", p.display()))
            };
            let mut report = desync_diff::diff_saves(&read(&a)?, &read(&b)?, max)?;
            report.add_manifests(&a, &b);
            print!("{}", desync_diff::render(&report));
            Ok(if report.differs() {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            })
        }
        Command::Ballistics(args) => {
            let (out, ok) = ballistics::run(&args)?;
            println!("{}", out.trim_end());
            Ok(if ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        Command::Schema { out } => {
            for path in schema::export(&out)? {
                println!("{}", path.display());
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Validate { files, json } => {
            let results: Vec<_> = files.iter().map(|f| validate::path(f)).collect();
            if json {
                println!("{}", serde_json::to_string_pretty(&results)?);
            } else {
                for r in &results {
                    if r.ok {
                        println!("{}: ok", r.file);
                    }
                    for p in &r.problems {
                        let fix = p
                            .fix
                            .as_ref()
                            .map(|f| format!(" (fix: {f})"))
                            .unwrap_or_default();
                        if p.path.is_empty() {
                            println!("{}: {}{fix}", r.file, p.message);
                        } else {
                            println!("{}: {}: {}{fix}", r.file, p.path, p.message);
                        }
                    }
                }
            }
            Ok(if results.iter().all(|r| r.ok) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
    }
}
