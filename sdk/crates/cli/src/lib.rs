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
//! * [`validate`] checks scenario files and reports every problem at once.

pub mod fortify;
pub mod lower;
pub mod report;
pub mod schema;
pub mod simulate;
pub mod skirmish;
pub mod validate;

pub use fortify::{FortifyArgs, fortify_test};
pub use simulate::{SimulateArgs, simulate};
pub use skirmish::{SkirmishArgs, skirmish};
