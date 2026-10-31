//! `frontline-cli`: headless tooling for Frontline content and the sim (§8.1).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use frontline_cli::{SimulateArgs, schema, simulate, validate};

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
