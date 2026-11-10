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
//! capture, income, tickets and victory; [`ai`]: the AI commander; [`unit`]:
//! soldier-level orders, stance orders and detached men; [`items`]: thrown
//! frag, smoke and satchel items and smoke clouds; [`armour`]: plates,
//! penetration, the box narrowphase and the derived armour tables, S3;
//! [`vehicle`]: vehicle state, pose, driving, the vehicle index and block
//! layer, S3; [`gunnery`]: main guns and vehicle MGs, S3; [`damage`]: shells
//! on plates, penetration, components, crew, fire, repair, blasts on
//! tracks and knock-out, S3; [`orders`]: vehicle orders and escorts, S3; [`scenarios`]: staged S3
//! setups for `armour-test` and the Godot autoplay scenes).
//! Terrain
//! is a two-tier heightfield whose sparse detail tiles are edited by digging
//! and craters ([`terrain`]).
//!
//! It has no Godot, rendering or audio dependency, and does no file IO.

#![deny(clippy::float_arithmetic)]

pub mod ai;
pub mod armour;
pub mod bail;
pub mod combat;
pub mod command;
pub mod damage;
pub mod dig;
pub mod ecs;
pub mod fire_mission;
pub mod fort;
pub mod fx;
pub mod game;
pub mod gunnery;
pub mod guns;
mod hash;
pub mod indirect;
pub mod items;
pub mod orders;
pub mod path;
pub mod projectile;
pub mod rng;
pub mod rules;
pub mod scenarios;
pub mod snapshot;
pub mod spatial;
mod spotting;
mod squad;
pub mod structures;
pub mod supply;
mod systems;
pub mod terrain;
pub mod transport;
pub mod unit;
pub mod vehicle;
pub mod world;

pub use ai::{AiCommand, AiCounters, AiEscort, AiOverwatch, AiState, AiTrench, Task, TaskKind};
pub use armour::{ArmourTables, PenResult};
pub use bail::{RecrewEvent, RecrewRefusal};
pub use command::{Command, CommandKind};
pub use dig::{DigJob, DigSegment};
pub use ecs::{CHUNK_SIZE, Combat, Entities, EntityKind, IdAllocator, PlayerId, SimId, Stance};
pub use fire_mission::{
    BurstRecord, FireMissionEvent, FireMissionRefusal, FirePreview, FmCounters, IntelCell, Mission,
    RangingCell,
};
pub use frontline_content::{ITEM_KINDS, ItemKind, MatchSetup};
pub use fx::{Fx, FxVec2, FxVec3};
pub use game::{
    CallInEvent, CallInRefusal, CapturePoint, CrewCounters, Launcher, MatchState, NavPhase, Order,
    OrderKind, Outcome, PathReq, PlayerState, S8aCounters, Squad, SquadId, kill_class,
};
pub use guns::S9Counters;
pub use items::{BlastKind, Kit, MAX_SMOKE, MAX_THROWN, SmokeCloud, Thrown, ThrownInside, Windup};
pub use projectile::{Projectile, ProjectilePool};
pub use rng::{Purpose, RngStream, rng};
pub use rules::Defeat;
pub use snapshot::{
    ArmourEvent, ArmourEventKind, CorpseSnapshot, CrateSnapshot, ExplosionSnapshot, LoadedSnap,
    NO_SQUAD, PlayerSnapshot, PointSnapshot, RenderSnapshot, SeatSnap, ShellKind, ShellSnapshot,
    ShotSnapshot, SmokeSnapshot, SquadSnapshot, StructureSnapshot, ThrownSnapshot, UnitSnapshot,
    VehicleSnapshot, tile_heights_f32, vehicle_flags,
};
pub use structures::{
    CookOff, Crate, S6Account, StaticBox, StaticRef, Structure, SupplyEvent, SupplyRefusal,
    SupplyState,
};
pub use supply::{AmmoAccount, fired_class};
pub use terrain::{DetailTile, Heightfield, TILE_M, TILE_POINTS, TerrainKind};
pub use unit::{FireMode, MAX_UNITS, StanceOrder, Unit, UnitDig, UnitNav, UnitOrder};
pub use vehicle::{AmmoChoice, BlockLayer, CrateDrop, MgState, Seat, VehicleFrame, VehicleState};
pub use world::{
    SimConfig, SimError, SimState, SoldierParams, StageTimes, Stats, TickRate, WeaponParams, World,
    match_config,
};

/// ORCA neighbour cap.
pub use systems::MAX_NEIGHBOURS;
