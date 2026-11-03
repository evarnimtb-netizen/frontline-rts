//! Library behind the `frontline-cli` binary (§8.1).
//!
//! The binary is a thin clap front-end; everything it does is reachable from
//! here so tests and other tools can drive it without spawning a process.
//!
//! * [`lower`] turns a content [`Scenario`](frontline_content::Scenario) into
//!   a sim [`SimConfig`](frontline_sim::SimConfig).
//! * [`simulate`] runs a headless battle and builds a [`report::Report`].
//! * [`skirmish`] runs a whole AI-vs-AI match and reports winner and stats.
//! * [`fortify`] stages trench-versus-open fights (the S1 cover check).
//! * [`schema`] exports JSON Schemas for every def type.
//! * [`validate`] checks scenario files and mod folders and reports every
//!   problem at once.
//! * [`ballistics`] (S3) prints penetration for one shot, a table or a
//!   golden file, through the sim's own armour functions.
//! * [`armour_test`] (S3) runs the staged armour cases
//!   (`frontline_sim::scenarios`) with hard checks and report rows.
//! * [`desync_diff`] compares two sim saves field by field (multiplayer
//!   desync dumps, S2).
//! * [`mortar_bench`] (S5b) fires the same mortar mission observed and as
//!   map fire and compares the misses (the 3x rule).

pub mod armour_test;
pub mod ballistics;
pub mod desync_diff;
pub mod fortify;
pub mod lower;
pub mod mortar_bench;
pub mod overrides;
pub mod report;
pub mod s3c_tally;
pub mod schema;
pub mod simulate;
pub mod skirmish;
pub mod validate;

pub use armour_test::{ArmourArgs, armour_test};
pub use ballistics::BallisticsArgs;
pub use fortify::{FortifyArgs, fortify_test};
pub use mortar_bench::{BenchArgs, mortar_bench};
pub use simulate::{SimulateArgs, simulate};
pub use skirmish::{SkirmishArgs, skirmish};
