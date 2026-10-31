//! Frontline deterministic simulation core.
//!
//! Lockstep multiplayer needs every client to compute bit-identical state
//! from the same commands (§5.11). This crate therefore:
//!
//! * keeps all sim state and logic in Q16.16 fixed point ([`fx`]). Floats
//!   appear only when building the presenter [`RenderSnapshot`];
//! * draws randomness from a counter-based [`rng::rng`] keyed by
//!   `(seed, tick, id, purpose)`, never from mutable RNG state;
//! * stores entities in a custom struct-of-arrays store ([`ecs`]) with stable
//!   [`SimId`]s and ascending-id iteration;
//! * runs a fixed stage order ([`World::step`]) whose parallel parts write
//!   only their own chunk and merge events serially in a sorted order;
//! * hashes all state incrementally in chunks of 256 ids ([`hash`]).
//!
//! Worlds come in two modes: bench worlds from a [`SimConfig`] (the M0
//! load test) and match worlds from a content [`MatchSetup`] resolved by
//! `frontline_content` from the defs under `mods/` ([`game`]: players,
//! squads, orders; [`path`]: grid A*; [`squad`]: formations).
//!
//! It has no Godot, rendering or audio dependency, and does no file IO.

#![deny(clippy::float_arithmetic)]

pub mod command;
pub mod ecs;
pub mod fx;
pub mod game;
mod hash;
pub mod path;
pub mod projectile;
pub mod rng;
pub mod snapshot;
pub mod spatial;
mod squad;
mod systems;
pub mod terrain;
pub mod world;

pub use command::{Command, CommandKind};
pub use ecs::{CHUNK_SIZE, Entities, EntityKind, IdAllocator, PlayerId, SimId};
pub use frontline_content::MatchSetup;
pub use fx::{Fx, FxVec2, FxVec3};
pub use game::{MatchState, NavPhase, Order, OrderKind, Outcome, Squad, SquadId};
pub use projectile::{Projectile, ProjectilePool};
pub use rng::{Purpose, RngStream, rng};
pub use snapshot::{NO_SQUAD, PlayerSnapshot, RenderSnapshot, SquadSnapshot};
pub use terrain::{Heightfield, TerrainKind};
pub use world::{
    SimConfig, SimError, SimState, SoldierParams, StageTimes, Stats, TickRate, WeaponParams, World,
    match_config,
};

/// ORCA neighbour cap.
pub use systems::MAX_NEIGHBOURS;
