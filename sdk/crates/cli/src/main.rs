//! `frontline-cli`: headless tooling for Frontline content and the sim (§8.1).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use frontline_cli::skirmish::ReportFormat;
use frontline_cli::{
    FortifyArgs, SimulateArgs, SkirmishArgs, fortify, schema, simulate, skirmish, validate,
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
    /// the --minutes limit.
    Skirmish(SkirmishArgs),
    /// Stage trench-vs-open fights and check the S1 cover thresholds.
    ///
    /// Exits 0 when both experiments pass, 1 otherwise.
    FortifyTest(FortifyArgs),
    /// Export JSON Schemas for every def type.
    Schema {
        /// Output directory (created if missing).
        #[arg(long, value_name = "DIR")]
        out: PathBuf,
    },
    /// Check scenario files and list every problem with a suggested fix.
    Validate {
        #[arg(required = true, value_name = "TOML")]
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
        Command::Skirmish(args) => {
            let report = skirmish::skirmish(&args)?;
            match args.report {
                ReportFormat::Text => println!("{}", skirmish::summary_text(&report)),
                ReportFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
            }
            Ok(if report.winner.is_some() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(3)
            })
        }
        Command::FortifyTest(args) => {
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
        Command::Schema { out } => {
            for path in schema::export(&out)? {
                println!("{}", path.display());
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Validate { files, json } => {
            let results: Vec<_> = files.iter().map(|f| validate::scenario_file(f)).collect();
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
