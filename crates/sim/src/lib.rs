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
//! squads, orders; [`path`]: grid A*; [`squad`]: formations; [`dig`]:
//! trenches; `spotting`: per-player visibility; [`combat`]: targeting,
//! projectiles, suppression, stance, mortars and deaths; [`rules`]:
//! capture, income, tickets and victory; [`ai`]: the AI commander). Terrain
//! is a two-tier heightfield whose sparse detail tiles are edited by digging
//! and craters ([`terrain`]).
//!
//! It has no Godot, rendering or audio dependency, and does no file IO.

#![deny(clippy::float_arithmetic)]

pub mod ai;
pub mod combat;
pub mod command;
pub mod dig;
pub mod ecs;
pub mod fx;
pub mod game;
mod hash;
pub mod path;
pub mod projectile;
pub mod rng;
pub mod rules;
pub mod snapshot;
pub mod spatial;
mod spotting;
mod squad;
mod systems;
pub mod terrain;
pub mod world;

pub use ai::{AiCommand, AiState, AiTrench, Task, TaskKind};
pub use command::{Command, CommandKind};
pub use dig::{DigJob, DigSegment};
pub use ecs::{CHUNK_SIZE, Combat, Entities, EntityKind, IdAllocator, PlayerId, SimId, Stance};
pub use frontline_content::MatchSetup;
pub use fx::{Fx, FxVec2, FxVec3};
pub use game::{
    CallIn, CapturePoint, MatchState, NavPhase, Order, OrderKind, Outcome, PlayerState, Squad,
    SquadId,
};
pub use projectile::{Projectile, ProjectilePool};
pub use rng::{Purpose, RngStream, rng};
pub use rules::Defeat;
pub use snapshot::{
    CorpseSnapshot, ExplosionSnapshot, NO_SQUAD, PlayerSnapshot, PointSnapshot, RenderSnapshot,
    ShotSnapshot, SquadSnapshot, tile_heights_f32,
};
pub use terrain::{DetailTile, Heightfield, TILE_M, TILE_POINTS, TerrainKind};
pub use world::{
    SimConfig, SimError, SimState, SoldierParams, StageTimes, Stats, TickRate, WeaponParams, World,
    match_config,
};

/// ORCA neighbour cap.
pub use systems::MAX_NEIGHBOURS;
