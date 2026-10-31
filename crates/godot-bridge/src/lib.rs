//! Frontline GDExtension (`godot-bridge`, §4.1, §13): a thin adapter that
//! owns the sim thread and turns render snapshots into batched MultiMesh
//! uploads. M0 render spike (§2.2 gate 7, §15 M0).
//!
//! Everything except [`node`] and the entry point is plain Rust and is unit
//! tested without Godot.

mod entry;
pub mod handoff;
pub mod meshgen;
mod node;
pub mod present;
pub mod rig;

pub use node::FrontlineSim;
