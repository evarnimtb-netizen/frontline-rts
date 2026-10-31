//! Frontline GDExtension (`godot-bridge`, §4.1, §13): a thin adapter that
//! owns the sim thread and turns render snapshots into batched MultiMesh
//! uploads. M0 render spike (§2.2 gate 7, §15 M0).
//!
//! Everything except [`node`] and the entry point is plain Rust and is unit
//! tested without Godot.

mod entry;
mod gfx;
pub mod handoff;
pub mod intents;
pub mod match_events;
pub mod match_frame;
mod match_node;
pub mod match_setup;
pub mod match_thread;
pub mod meshgen;
mod node;
pub mod present;
pub mod rig;
pub mod terrain_view;

pub use match_node::FrontlineMatch;
pub use node::FrontlineSim;
