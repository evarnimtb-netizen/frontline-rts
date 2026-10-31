//! Library behind the `frontline-cli` binary (§8.1).
//!
//! The binary is a thin clap front-end; everything it does is reachable from
//! here so tests and other tools can drive it without spawning a process.
//!
//! * [`lower`] turns a content [`Scenario`](frontline_content::Scenario) into
//!   a sim [`SimConfig`](frontline_sim::SimConfig).
//! * [`simulate`] runs a headless battle and builds a [`report::Report`].
//! * [`schema`] exports JSON Schemas for every def type.
//! * [`validate`] checks scenario files and reports every problem at once.

pub mod lower;
pub mod report;
pub mod schema;
pub mod simulate;
pub mod validate;

pub use simulate::{SimulateArgs, simulate};
