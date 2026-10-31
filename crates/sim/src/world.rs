//! The world: saved state, configuration, the tick loop and save/load.
//!
//! [`World::step`] runs one tick in the fixed stage order of §5.2:
//!
//! | # | Stage | Work | Parallel? |
//! |---|-------|---------|-----------|
//! | 1 | Intent | sorted command apply; bench: waypoint re-pick; match: call-ins, path queue, digging (terrain edits), squad steering | serial apply, per-chunk re-pick; match work serial |
//! | 2 | Perception | placeholder | – |
//! | 3 | Movement | spatial hash rebuild; ORCA plan; integrate | serial rebuild, per-chunk plan + integrate |
//! | 4 | Weapons | bench: cooldowns; fire → spawn requests (match: none yet) | per-chunk, sorted serial spawn |
//! | 5 | Ballistics | 60 Hz substeps, ground test, despawn | per-slot-chunk, serial free-list merge |
//! | 6 | Damage | placeholder | – |
//! | 7 | Status | placeholder | – |
//! | 8 | World/Hash | tick++, dirty chunk and terrain tile re-hash, publish every K; snapshot | per-chunk hash |
//!
//! A world is either a **bench** world ([`World::new`] from a [`SimConfig`]:
//! M0 soldiers wandering and firing at random) or a **match** world
//! ([`World::new_match`] from a content [`MatchSetup`]: players, squads and
//! orders, see [`crate::game`]). The mode is fixed for the world's lifetime.
//!
//! Parallel stages run on a rayon pool owned by the world. A chunk task writes
//! only its own chunk's slices, or appends to its own per-chunk buffer; the
//! buffers are merged serially in chunk order. No atomics, no shared counters,
//! no "remaining budget" reads, so the worker count never changes a result.

use std::sync::Arc;
use std::time::{Duration, Instant};

use frontline_content::MatchSetup;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::command::{Command, CommandKind};
use crate::ecs::{CHUNK_SIZE, Entities, EntityKind, PlayerId, SimId};
use crate::fx::{Fx, FxVec2};
use crate::game::{Ctx, MatchState};
use crate::hash::{self, HashCache};
use crate::path::PathScratch;
use crate::projectile::{ProjectileEnd, ProjectilePool, SUBSTEP_HZ};
use crate::rng::{Purpose, RngStream};
use crate::snapshot::RenderSnapshot;
use crate::spatial::SpatialHash;
use crate::systems::{self, FireRequest};
use crate::terrain::{Heightfield, TerrainKind};

/// Sim tick rate, a match constant (§2.2). 30 Hz is the default; 20 Hz is
/// allowed only as a wholesale replacement.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum TickRate {
    #[default]
    Hz30,
    Hz20,
}

impl TickRate {
    #[inline]
    pub const fn hz(self) -> i32 {
        match self {
            TickRate::Hz30 => 30,
            TickRate::Hz20 => 20,
        }
    }

    /// 1/60 s ballistic substeps per tick (2 at 30 Hz, 3 at 20 Hz).
    #[inline]
    pub const fn substeps(self) -> u32 {
        (SUBSTEP_HZ / self.hz()) as u32
    }
}

/// Soldier movement parameters.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SoldierParams {
    /// Agent radius for avoidance (m).
    pub radius: Fx,
    pub speed_min: Fx,
    pub speed_max: Fx,
    /// Neighbour search radius (m).
    pub neighbour_dist: Fx,
    /// ORCA neighbour cap, 1..=8.
    pub max_neighbours: u8,
    /// ORCA time horizon (s).
    pub time_horizon: Fx,
    /// A waypoint closer than this counts as reached (m).
    pub arrive_radius: Fx,
    /// Auto waypoints are picked within ±this of the current position (m).
    pub wander_radius: Fx,
}

impl Default for SoldierParams {
    fn default() -> Self {
        SoldierParams {
            radius: Fx::from_milli(400),
            speed_min: Fx::from_milli(1200),
            speed_max: Fx::from_milli(1800),
            neighbour_dist: Fx::from_int(4),
            max_neighbours: 8,
            time_horizon: Fx::from_int(2),
            arrive_radius: Fx::from_milli(1500),
            wander_radius: Fx::from_int(48),
        }
    }
}

/// Small-arms parameters for the M0 projectile load.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WeaponParams {
    /// Ticks between shots, seeded per shot in `[min, max]`.
    pub cooldown_min: u16,
    pub cooldown_max: u16,
    /// Yaw spread ± (rad).
    pub spread: Fx,
    pub elevation_min: Fx,
    pub elevation_max: Fx,
    pub muzzle_speed_min: Fx,
    pub muzzle_speed_max: Fx,
    /// Quadratic drag coefficient k (1/m).
    pub drag: Fx,
    /// Lifetime in 1/60 s substeps.
    pub ttl_substeps: u16,
    pub muzzle_height: Fx,
}

impl Default for WeaponParams {
    fn default() -> Self {
        WeaponParams {
            cooldown_min: 20,
            cooldown_max: 75,
            spread: Fx::from_milli(200),
            elevation_min: Fx::from_milli(20),
            elevation_max: Fx::from_milli(350),
            muzzle_speed_min: Fx::from_int(350),
            muzzle_speed_max: Fx::from_int(760),
            drag: Fx::from_ratio(4, 10_000),
            ttl_substeps: 180,
            muzzle_height: Fx::from_milli(1400),
        }
    }
}

/// Match configuration. Part of the saved and hashed state. The worker count
/// is deliberately not here: it must never affect results.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SimConfig {
    pub seed: u64,
    pub tick_rate: TickRate,
    /// Publish the combined state hash every K ticks.
    pub hash_interval: u32,
    /// Square map side (m); multiple of 64, at most 8192.
    pub map_size_m: i32,
    pub terrain: TerrainKind,
    /// Soldiers spawned at tick 0.
    pub soldiers: u32,
    /// Side (m) of the centred square the tick-0 soldiers spawn in; 0 means
    /// the whole map. Smaller areas give contact-density crowds.
    pub spawn_area_m: i32,
    pub players: u8,
    pub projectile_cap: u32,
    pub soldier: SoldierParams,
    pub weapon: WeaponParams,
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig {
            seed: 1,
            tick_rate: TickRate::Hz30,
            hash_interval: 30,
            map_size_m: 512,
            terrain: TerrainKind::Noise,
            soldiers: 700,
            spawn_area_m: 0,
            players: 2,
            projectile_cap: 1800,
            soldier: SoldierParams::default(),
            weapon: WeaponParams::default(),
        }
    }
}

impl SimConfig {
    /// Most soldiers spawned at tick 0 (well above the Regiment cap; bounds
    /// memory for untrusted saves and configs).
    pub const MAX_SOLDIERS: u32 = 65_536;
    /// Largest projectile pool.
    pub const MAX_PROJECTILE_CAP: u32 = 1 << 20;
    /// Upper bound for `neighbour_dist` and `arrive_radius` (m). Their squares
    /// are computed in Q16.16, which saturates beyond ~181 m.
    pub const MAX_SOLDIER_DIST_M: i32 = 64;
    /// Highest muzzle speed (m/s). The swept ground test samples at most 65
    /// points per 1/60 s substep, i.e. ≤ 1 m apart up to 3,840 m/s.
    pub const MAX_MUZZLE_SPEED: i32 = 3000;

    pub fn validate(&self) -> Result<(), SimError> {
        let bad = |m: &str| Err(SimError::InvalidConfig(m.to_owned()));
        let s = &self.soldier;
        let w = &self.weapon;
        if self.map_size_m < 64 || self.map_size_m > 8192 || self.map_size_m % 64 != 0 {
            return bad("map_size_m must be a multiple of 64 in 64..=8192");
        }
        if self.hash_interval == 0 {
            return bad("hash_interval must be at least 1");
        }
        if self.players == 0 {
            return bad("players must be at least 1");
        }
        if self.soldiers > Self::MAX_SOLDIERS {
            return bad("soldiers must be at most 65536");
        }
        if self.projectile_cap > Self::MAX_PROJECTILE_CAP {
            return bad("projectile_cap must be at most 1048576");
        }
        if self.spawn_area_m < 0 || self.spawn_area_m > self.map_size_m {
            return bad("spawn_area_m must be in 0..=map_size_m");
        }
        if s.radius <= Fx::ZERO || s.time_horizon <= Fx::ZERO || s.neighbour_dist <= Fx::ZERO {
            return bad("soldier radius, time_horizon and neighbour_dist must be positive");
        }
        let max_dist = Fx::from_int(Self::MAX_SOLDIER_DIST_M);
        if s.neighbour_dist > max_dist || s.arrive_radius < Fx::ZERO || s.arrive_radius > max_dist {
            return bad("soldier neighbour_dist and arrive_radius must be in 0..=64 m");
        }
        if s.radius > Fx::from_int(8) || s.speed_max > Fx::from_int(50) {
            return bad("soldier radius must be at most 8 m and speed_max at most 50 m/s");
        }
        if s.wander_radius < Fx::ZERO {
            return bad("soldier wander_radius must not be negative");
        }
        if s.max_neighbours == 0 || s.max_neighbours as usize > systems::MAX_NEIGHBOURS {
            return bad("soldier max_neighbours must be in 1..=8");
        }
        if s.speed_min < Fx::ZERO || s.speed_min > s.speed_max {
            return bad("soldier speed range is invalid");
        }
        if w.cooldown_min > w.cooldown_max
            || w.elevation_min > w.elevation_max
            || w.muzzle_speed_min > w.muzzle_speed_max
        {
            return bad("weapon ranges are invalid");
        }
        if w.spread < Fx::ZERO || w.drag < Fx::ZERO {
            return bad("weapon spread and drag must not be negative");
        }
        if w.muzzle_speed_min < Fx::ZERO
            || w.muzzle_speed_max > Fx::from_int(Self::MAX_MUZZLE_SPEED)
        {
            return bad("weapon muzzle speed must be in 0..=3000 m/s");
        }
        if w.elevation_min < -Fx::FRAC_PI_2 || w.elevation_max > Fx::FRAC_PI_2 {
            return bad("weapon elevation must be in -pi/2..=pi/2");
        }
        Ok(())
    }
}

/// The [`SimConfig`] a match world runs with, derived from its setup (and
/// checked against it on load).
pub fn match_config(setup: &MatchSetup) -> SimConfig {
    let speeds = setup.squad_types.iter().map(|t| Fx::from(t.speed));
    let speed_min = speeds.clone().min().unwrap_or(Fx::ONE);
    let speed_max = speeds.max().unwrap_or(Fx::ONE);
    SimConfig {
        seed: setup.seed,
        tick_rate: if setup.tick_hz == 20 {
            TickRate::Hz20
        } else {
            TickRate::Hz30
        },
        hash_interval: setup.hash_interval,
        map_size_m: setup.map.size_m,
        terrain: TerrainKind::Map,
        soldiers: 0,
        spawn_area_m: 0,
        players: setup.players.len() as u8,
        projectile_cap: setup.rules.projectile_cap,
        soldier: SoldierParams {
            speed_min,
            speed_max,
            arrive_radius: setup.rules.movement.arrive_radius.into(),
            wander_radius: Fx::ZERO,
            ..SoldierParams::default()
        },
        weapon: WeaponParams::default(),
    }
}

/// Counters that are part of the state (and the hash).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    pub shots_fired: u64,
    /// Shots dropped because the projectile pool was full.
    pub shots_dropped: u64,
    pub ground_hits: u64,
    pub expired: u64,
    pub out_of_bounds: u64,
    pub commands_applied: u64,
    pub commands_rejected: u64,
}

/// Everything that is saved. All of it is covered by the state hash, except
/// the non-`kind` columns of vacant entity slots, which `World::load` requires
/// to hold defaults.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SimState {
    pub(crate) config: SimConfig,
    /// Ticks completed.
    pub(crate) tick: u64,
    pub(crate) entities: Entities,
    pub(crate) projectiles: ProjectilePool,
    pub(crate) terrain: Heightfield,
    pub(crate) stats: Stats,
    /// Match mode only.
    pub(crate) game: Option<MatchState>,
}

#[derive(Debug, thiserror::Error)]
pub enum SimError {
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    #[error("thread pool: {0}")]
    ThreadPool(String),
    #[error("save data: {0}")]
    Save(String),
}

/// Wall-clock time spent in each stage of the last tick (diagnostics only;
/// never read by the sim).
#[derive(Clone, Copy, Debug, Default)]
pub struct StageTimes {
    pub intent: Duration,
    pub perception: Duration,
    pub movement: Duration,
    pub weapons: Duration,
    pub ballistics: Duration,
    pub damage: Duration,
    pub status: Duration,
    pub world_hash: Duration,
    pub snapshot: Duration,
}

impl StageTimes {
    pub fn total(&self) -> Duration {
        self.intent
            + self.perception
            + self.movement
            + self.weapons
            + self.ballistics
            + self.damage
            + self.status
            + self.world_hash
            + self.snapshot
    }
}

const SAVE_MAGIC: [u8; 8] = *b"FLSIM\0\0\0";
/// 2: match state and the entity `squad` column. 3: terrain detail tiles
/// and dig jobs.
const SAVE_VERSION: u32 = 3;

#[derive(Serialize)]
struct SaveRef<'a> {
    magic: [u8; 8],
    version: u32,
    state: &'a SimState,
}

#[derive(Deserialize)]
struct SaveOwned {
    magic: [u8; 8],
    version: u32,
    state: SimState,
}

/// The simulation. Owns its state, its worker pool and derived caches.
pub struct World {
    state: SimState,
    /// postcard encoding of the config, hashed into every published hash.
    config_bytes: Vec<u8>,
    /// postcard encoding of the match setup (empty in bench mode).
    setup_bytes: Vec<u8>,
    /// Shared so `step` can run on a worker of this pool (see [`World::step`]).
    threads: Arc<rayon::ThreadPool>,
    thread_count: usize,
    spatial: SpatialHash,
    hash: HashCache,
    snapshot: RenderSnapshot,
    published: Option<(u64, u64)>,
    times: StageTimes,
    // Per-tick scratch, reused to avoid steady-state allocation.
    planned: Vec<FxVec2>,
    flags: Vec<bool>,
    fire_bufs: Vec<Vec<FireRequest>>,
    fire_merged: Vec<FireRequest>,
    ended_bufs: Vec<Vec<(u32, ProjectileEnd)>>,
    cmd_order: Vec<usize>,
    hash_out: Vec<u64>,
    path_scratch: PathScratch,
    /// Terrain tiles changed in the last tick (for the snapshot).
    dirty_tiles: Vec<(u16, u16)>,
}

impl World {
    /// Creates a world and spawns `config.soldiers` soldiers at seeded
    /// positions, round-robin over players. `threads` is the worker count
    /// (at least 1); it never affects results.
    pub fn new(config: SimConfig, threads: usize) -> Result<World, SimError> {
        config.validate()?;
        if config.terrain == TerrainKind::Map {
            return Err(SimError::InvalidConfig(
                "map terrain needs a match setup (World::new_match)".into(),
            ));
        }
        let terrain = Heightfield::generate(config.terrain, config.map_size_m, config.seed);
        let state = SimState {
            projectiles: ProjectilePool::new(config.projectile_cap),
            entities: Entities::default(),
            terrain,
            stats: Stats::default(),
            tick: 0,
            config,
            game: None,
        };
        let mut world = World::from_state(state, threads)?;
        let cfg = &world.state.config;
        let (n, players, seed) = (cfg.soldiers, cfg.players, cfg.seed);
        let map = cfg.map_size_m;
        let area = if cfg.spawn_area_m == 0 {
            map
        } else {
            cfg.spawn_area_m
        };
        let lo = Fx::from_int((map - area) / 2);
        let hi = lo + Fx::from_int(area);
        for i in 0..n {
            let mut r = RngStream::new(seed, 0, SimId(i), Purpose::SPAWN_POS);
            let pos = FxVec2::new(r.range_fx(lo, hi), r.range_fx(lo, hi));
            world.spawn_soldier(PlayerId((i % players as u32) as u8), pos);
        }
        world.refresh_hashes();
        world.fill_snapshot();
        Ok(world)
    }

    /// Creates a match world: terrain generated from the map def, players
    /// from the setup, each faction's starting squads at its base.
    pub fn new_match(setup: MatchSetup, threads: usize) -> Result<World, SimError> {
        let issues = setup.validate();
        if !issues.is_empty() {
            let list: Vec<String> = issues.iter().map(|i| i.to_string()).collect();
            return Err(SimError::InvalidConfig(format!(
                "match setup: {}",
                list.join("; ")
            )));
        }
        let config = match_config(&setup);
        config.validate()?;
        let state = SimState {
            projectiles: ProjectilePool::new(config.projectile_cap),
            entities: Entities::default(),
            terrain: Heightfield::generate_map(&setup.map),
            stats: Stats::default(),
            tick: 0,
            config,
            game: Some(MatchState::new(setup)),
        };
        let mut world = World::from_state(state, threads)?;
        {
            let World { state, hash, .. } = &mut world;
            let SimState {
                entities,
                terrain,
                game,
                tick,
                ..
            } = state;
            let mut ctx = Ctx {
                ents: entities,
                terrain,
                hash,
                tick: *tick,
            };
            game.as_mut()
                .expect("match world")
                .spawn_start_squads(&mut ctx);
        }
        world.refresh_hashes();
        world.fill_snapshot();
        Ok(world)
    }

    fn from_state(state: SimState, threads: usize) -> Result<World, SimError> {
        let thread_count = threads.max(1);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(thread_count)
            .thread_name(|i| format!("sim-worker-{i}"))
            .build()
            .map(Arc::new)
            .map_err(|e| SimError::ThreadPool(e.to_string()))?;
        let config_bytes =
            postcard::to_allocvec(&state.config).map_err(|e| SimError::Save(e.to_string()))?;
        let setup_bytes = match &state.game {
            Some(g) => {
                postcard::to_allocvec(g.setup()).map_err(|e| SimError::Save(e.to_string()))?
            }
            None => Vec::new(),
        };
        let mut world = World {
            spatial: SpatialHash::new(state.config.map_size_m),
            hash: HashCache::new(&state),
            config_bytes,
            setup_bytes,
            threads: pool,
            thread_count,
            snapshot: RenderSnapshot::default(),
            published: None,
            times: StageTimes::default(),
            planned: Vec::new(),
            flags: Vec::new(),
            fire_bufs: Vec::new(),
            fire_merged: Vec::new(),
            ended_bufs: Vec::new(),
            cmd_order: Vec::new(),
            hash_out: Vec::new(),
            path_scratch: PathScratch::default(),
            dirty_tiles: Vec::new(),
            state,
        };
        world.refresh_hashes();
        world.fill_snapshot();
        Ok(world)
    }

    // ------------------------------------------------------------ queries --

    #[inline]
    pub fn tick(&self) -> u64 {
        self.state.tick
    }
    #[inline]
    pub fn config(&self) -> &SimConfig {
        &self.state.config
    }
    #[inline]
    pub fn entities(&self) -> &Entities {
        &self.state.entities
    }
    #[inline]
    pub fn projectiles(&self) -> &ProjectilePool {
        &self.state.projectiles
    }
    #[inline]
    pub fn terrain(&self) -> &Heightfield {
        &self.state.terrain
    }
    #[inline]
    pub fn stats(&self) -> &Stats {
        &self.state.stats
    }
    /// Match state (players, squads, ...); `None` for bench worlds.
    #[inline]
    pub fn game(&self) -> Option<&MatchState> {
        self.state.game.as_ref()
    }
    /// The match setup; `None` for bench worlds.
    #[inline]
    pub fn match_setup(&self) -> Option<&MatchSetup> {
        self.state.game.as_ref().map(|g| g.setup())
    }
    #[inline]
    pub fn state(&self) -> &SimState {
        &self.state
    }
    #[inline]
    pub fn threads(&self) -> usize {
        self.thread_count
    }
    /// Snapshot of the last completed tick.
    #[inline]
    pub fn snapshot(&self) -> &RenderSnapshot {
        &self.snapshot
    }
    /// Stage timings of the last tick.
    #[inline]
    pub fn stage_times(&self) -> &StageTimes {
        &self.times
    }
    /// Last published `(tick, hash)`.
    #[inline]
    pub fn published_hash(&self) -> Option<(u64, u64)> {
        self.published
    }

    /// Combined state hash from the incremental per-chunk cache.
    pub fn state_hash(&self) -> u64 {
        debug_assert!(
            !self.hash.entity_dirty.iter().any(|d| *d) && !self.hash.pool_dirty.iter().any(|d| *d)
        );
        let g = hash::globals(
            &self.state,
            &self.config_bytes,
            &self.setup_bytes,
            self.hash.terrain_hash(&self.state.terrain),
        );
        hash::combine(g, &self.hash.entity, &self.hash.pool)
    }

    /// Combined state hash recomputed from scratch (ignores the cache).
    pub fn full_state_hash(&self) -> u64 {
        hash::full_hash(&self.state, &self.config_bytes, &self.setup_bytes)
    }

    // ---------------------------------------------------------- save/load --

    /// Serialises the full sim state (postcard).
    pub fn save(&self) -> Vec<u8> {
        postcard::to_allocvec(&SaveRef {
            magic: SAVE_MAGIC,
            version: SAVE_VERSION,
            state: &self.state,
        })
        .expect("serialising sim state cannot fail")
    }

    /// Restores a world from [`World::save`] output with `threads` workers.
    pub fn load(bytes: &[u8], threads: usize) -> Result<World, SimError> {
        let mut s: SaveOwned =
            postcard::from_bytes(bytes).map_err(|e| SimError::Save(e.to_string()))?;
        if s.magic != SAVE_MAGIC {
            return Err(SimError::Save("not a sim save (bad magic)".into()));
        }
        if s.version != SAVE_VERSION {
            return Err(SimError::Save(format!(
                "unsupported save version {}",
                s.version
            )));
        }
        // Saves are the join-in-progress and replay transport, so treat them
        // as untrusted: anything that would panic later, alias an id or slot,
        // or hide state from the hash is rejected here.
        let st = &s.state;
        let err = |m: &str| SimError::Save(m.to_owned());
        st.config.validate()?;
        st.entities.validate().map_err(err)?;
        let max_owner = st.config.players;
        if st
            .entities
            .owner
            .iter()
            .zip(&st.entities.kind)
            .any(|(o, k)| *k != EntityKind::Vacant && o.0 >= max_owner)
        {
            return Err(err("an entity is owned by a player outside the config"));
        }
        if st.projectiles.capacity() != st.config.projectile_cap as usize {
            return Err(err("projectile pool size disagrees with config"));
        }
        st.projectiles.validate().map_err(err)?;
        st.terrain.validate().map_err(err)?;
        if st.terrain.size_m() != st.config.map_size_m {
            return Err(err("terrain size disagrees with config"));
        }
        if st
            .terrain
            .detail()
            .tiles()
            .iter()
            .any(|t| t.changed_tick() > st.tick)
        {
            return Err(err("terrain detail tile changed in the future"));
        }
        match &st.game {
            None if st.config.terrain == TerrainKind::Map => {
                return Err(err("map terrain without a match state"));
            }
            None if st.entities.squad.iter().any(|s| s.is_some()) => {
                return Err(err("squad links in a bench world"));
            }
            None => {}
            Some(g) => {
                let issues = g.setup().validate();
                if let Some(i) = issues.first() {
                    return Err(SimError::Save(format!("match setup: {i}")));
                }
                if st.config != match_config(g.setup()) {
                    return Err(err("config disagrees with the match setup"));
                }
                g.validate(&st.entities).map_err(err)?;
            }
        }
        s.state.terrain.rebuild_derived();
        World::from_state(s.state, threads)
    }

    // --------------------------------------------------------------- tick --

    /// Runs `op` on one of this world's worker threads, so rayon work inside
    /// it uses this pool (and joins it rather than hopping in from outside).
    /// For callers that do per-tick parallel work next to the sim, such as
    /// the benchmark's ray budget. Never affects sim results.
    pub fn install<R: Send>(&self, op: impl FnOnce() -> R + Send) -> R {
        self.threads.install(op)
    }

    /// Runs one tick with the commands scheduled for it. Returns the combined
    /// state hash if this tick publishes one (every `hash_interval` ticks).
    ///
    /// The whole tick runs on one worker of the world's pool (§4.1 rule 7):
    /// the serial parts run there and each parallel stage forks from it,
    /// instead of handing work from the caller thread to the pool per stage.
    pub fn step(&mut self, cmds: &[Command]) -> Option<u64> {
        let pool = Arc::clone(&self.threads);
        pool.install(|| self.step_in_pool(cmds))
    }

    // `Instant::now` here feeds only the diagnostic `StageTimes`.
    #[allow(clippy::disallowed_methods)]
    fn step_in_pool(&mut self, cmds: &[Command]) -> Option<u64> {
        let tick = self.state.tick;
        let seed = self.state.config.seed;
        let hz = self.state.config.tick_rate.hz();
        let substeps = self.state.config.tick_rate.substeps();

        // 1. Intent: serial command apply, then per-chunk waypoint re-pick
        //    (bench) or call-ins, paths and squad steering (match).
        let t0 = Instant::now();
        self.apply_commands(cmds);
        if self.state.game.is_some() {
            let World {
                state,
                hash,
                path_scratch,
                ..
            } = self;
            let SimState {
                entities,
                terrain,
                game,
                tick,
                ..
            } = state;
            let game = game.as_mut().expect("match world");
            let mut ctx = Ctx {
                ents: &mut *entities,
                terrain: &*terrain,
                hash: &mut *hash,
                tick: *tick,
            };
            game.spawn_call_ins(&mut ctx);
            game.serve_paths(ctx.ents, ctx.terrain, path_scratch);
            game.dig_work(entities, terrain, *tick);
            let mut ctx = Ctx {
                ents: entities,
                terrain,
                hash,
                tick: *tick,
            };
            game.steer_squads(&mut ctx);
        } else {
            let World {
                state,
                threads,
                hash,
                flags,
                ..
            } = self;
            let SimState {
                entities,
                terrain,
                config,
                ..
            } = state;
            let Entities {
                kind,
                transform,
                mover,
                ..
            } = entities;
            let (terrain, params) = (&*terrain, &config.soldier);
            threads.install(|| {
                mover
                    .par_chunks_mut(CHUNK_SIZE)
                    .zip(transform.par_chunks(CHUNK_SIZE))
                    .zip(kind.par_chunks(CHUNK_SIZE))
                    .enumerate()
                    .map(|(c, ((m, t), k))| {
                        systems::pick_waypoints(c, k, t, m, terrain, params, seed, tick)
                    })
                    .collect_into_vec(flags)
            });
            hash.merge_entity_flags(flags);
        }
        let t1 = Instant::now();

        // 2. Perception: placeholder (M1).
        let t2 = Instant::now();

        // 3. Movement: rebuild the spatial hash, plan (reads only the
        //    previous state), then integrate (writes own entity only).
        {
            let World {
                state,
                threads,
                hash,
                flags,
                spatial,
                planned,
                ..
            } = self;
            let SimState {
                entities,
                terrain,
                config,
                ..
            } = state;
            spatial.rebuild(entities);
            planned.clear();
            planned.resize(entities.len(), FxVec2::ZERO);
            let (ents, spatial, params) = (&*entities, &*spatial, &config.soldier);
            threads.install(|| {
                planned
                    .par_chunks_mut(CHUNK_SIZE)
                    .enumerate()
                    .for_each(|(c, out)| {
                        systems::plan_velocities(c, out, ents, spatial, params, hz)
                    })
            });
            let Entities {
                kind,
                transform,
                velocity,
                ..
            } = entities;
            let (terrain, planned) = (&*terrain, &*planned);
            threads.install(|| {
                kind.par_chunks(CHUNK_SIZE)
                    .zip(transform.par_chunks_mut(CHUNK_SIZE))
                    .zip(velocity.par_chunks_mut(CHUNK_SIZE))
                    .zip(planned.par_chunks(CHUNK_SIZE))
                    .map(|(((k, t), v), p)| systems::integrate_soldiers(k, t, v, p, terrain, hz))
                    .collect_into_vec(flags)
            });
            hash.merge_entity_flags(flags);
        }
        let t3 = Instant::now();

        // 4. Weapons: per-chunk fire into per-chunk buffers, then a sorted
        //    serial spawn into the capped pool. Bench only: match combat is
        //    a later stage.
        if self.state.game.is_none() {
            let World {
                state,
                threads,
                hash,
                flags,
                fire_bufs,
                fire_merged,
                ..
            } = self;
            let SimState {
                entities,
                config,
                projectiles,
                stats,
                ..
            } = state;
            let Entities {
                kind,
                transform,
                weapon,
                ..
            } = entities;
            fire_bufs.resize_with(kind.len().div_ceil(CHUNK_SIZE), Vec::new);
            let wp = &config.weapon;
            threads.install(|| {
                weapon
                    .par_chunks_mut(CHUNK_SIZE)
                    .zip(transform.par_chunks(CHUNK_SIZE))
                    .zip(kind.par_chunks(CHUNK_SIZE))
                    .zip(fire_bufs.par_iter_mut())
                    .enumerate()
                    .map(|(c, (((w, t), k), out))| {
                        out.clear();
                        systems::fire_weapons(c, k, t, w, wp, seed, tick, out)
                    })
                    .collect_into_vec(flags)
            });
            hash.merge_entity_flags(flags);
            fire_merged.clear();
            for b in fire_bufs.iter() {
                fire_merged.extend_from_slice(b);
            }
            // Already in id order by construction; sorting makes the merge
            // order explicit and independent of buffer layout.
            fire_merged.sort_unstable();
            for req in fire_merged.iter() {
                stats.shots_fired += 1;
                match projectiles.spawn(systems::projectile_from(req, wp)) {
                    Some(slot) => hash.mark_pool_slot(slot),
                    None => stats.shots_dropped += 1,
                }
            }
        }
        let t4 = Instant::now();

        // 5. Ballistics: per-slot-chunk substeps, serial free-list merge.
        {
            let World {
                state,
                threads,
                hash,
                flags,
                ended_bufs,
                ..
            } = self;
            let SimState {
                projectiles,
                terrain,
                stats,
                ..
            } = state;
            ended_bufs.resize_with(projectiles.chunk_count(), Vec::new);
            let terrain = &*terrain;
            threads.install(|| {
                projectiles
                    .slots_mut()
                    .par_chunks_mut(CHUNK_SIZE)
                    .zip(ended_bufs.par_iter_mut())
                    .enumerate()
                    .map(|(c, (s, out))| {
                        out.clear();
                        systems::integrate_projectiles(c, s, terrain, substeps, out)
                    })
                    .collect_into_vec(flags)
            });
            hash.merge_pool_flags(flags);
            for buf in ended_bufs.iter() {
                for &(slot, end) in buf {
                    projectiles.release(slot);
                    hash.mark_pool_slot(slot);
                    match end {
                        ProjectileEnd::GroundHit => stats.ground_hits += 1,
                        ProjectileEnd::Expired => stats.expired += 1,
                        ProjectileEnd::OutOfBounds => stats.out_of_bounds += 1,
                    }
                }
            }
        }
        let t5 = Instant::now();

        // 6. Damage and 7. Status: placeholders (M1+).
        let t6 = Instant::now();
        let t7 = Instant::now();

        // 8. World/Hash: advance the tick, re-hash dirty chunks, publish.
        self.state.tick += 1;
        self.refresh_hashes();
        let k = self.state.config.hash_interval as u64;
        let published = if self.state.tick.is_multiple_of(k) {
            let h = self.state_hash();
            self.published = Some((self.state.tick, h));
            Some(h)
        } else {
            None
        };
        let t8 = Instant::now();

        self.fill_snapshot();
        let t9 = Instant::now();

        self.times = StageTimes {
            intent: t1 - t0,
            perception: t2 - t1,
            movement: t3 - t2,
            weapons: t4 - t3,
            ballistics: t5 - t4,
            damage: t6 - t5,
            status: t7 - t6,
            world_hash: t8 - t7,
            snapshot: t9 - t8,
        };
        published
    }

    // ----------------------------------------------------------- internals --

    fn apply_commands(&mut self, cmds: &[Command]) {
        self.cmd_order.clear();
        self.cmd_order.extend(0..cmds.len());
        // Total order on (player, seq, payload); identical commands are
        // interchangeable, so a stable sort of indices is fully deterministic.
        self.cmd_order.sort_by(|&a, &b| cmds[a].cmp(&cmds[b]));
        let order = std::mem::take(&mut self.cmd_order);
        for &i in &order {
            let c = &cmds[i];
            if let Some(game) = &mut self.state.game {
                let mut ctx = Ctx {
                    ents: &mut self.state.entities,
                    terrain: &self.state.terrain,
                    hash: &mut self.hash,
                    tick: self.state.tick,
                };
                if game.apply(c, &mut ctx) {
                    self.state.stats.commands_applied += 1;
                } else {
                    self.state.stats.commands_rejected += 1;
                }
                continue;
            }
            let ok = match c.kind {
                CommandKind::MoveSoldier { unit, target } => {
                    self.owned_by(unit, c.player) && {
                        let target = self.state.terrain.clamp(target);
                        let m = &mut self.state.entities.mover[unit.index()];
                        m.waypoint = target;
                        m.ordered = true;
                        self.hash.mark_entity_chunk(unit.chunk());
                        true
                    }
                }
                CommandKind::Spawn { pos } => {
                    if c.player.0 < self.state.config.players {
                        self.spawn_soldier(c.player, pos);
                        true
                    } else {
                        false
                    }
                }
                CommandKind::Despawn { unit } => {
                    self.owned_by(unit, c.player) && {
                        self.state.entities.despawn(unit);
                        self.hash.mark_entity_chunk(unit.chunk());
                        true
                    }
                }
                // Squad orders need a match world.
                _ => false,
            };
            if ok {
                self.state.stats.commands_applied += 1;
            } else {
                self.state.stats.commands_rejected += 1;
            }
        }
        self.cmd_order = order;
    }

    fn owned_by(&self, unit: SimId, player: PlayerId) -> bool {
        self.state.entities.is_alive(unit) && self.state.entities.owner[unit.index()] == player
    }

    fn spawn_soldier(&mut self, owner: PlayerId, pos: FxVec2) -> SimId {
        let st = &mut self.state;
        let (seed, tick) = (st.config.seed, st.tick);
        let id = st.entities.spawn(EntityKind::Soldier, owner);
        let i = id.index();
        let xy = st.terrain.clamp(pos);
        let mut r = RngStream::new(seed, tick, id, Purpose::SPAWN);
        let p = &st.config.soldier;
        let t = &mut st.entities.transform[i];
        t.pos = xy.extend(st.terrain.height_at(xy));
        t.yaw = r.range_fx(-Fx::PI, Fx::PI);
        let m = &mut st.entities.mover[i];
        m.speed = r.range_fx(p.speed_min, p.speed_max);
        m.waypoint = systems::random_waypoint(
            &st.terrain,
            xy,
            p.wander_radius,
            seed,
            tick,
            id,
            Purpose::SPAWN_WAYPOINT,
        );
        st.entities.weapon[i].cooldown =
            r.range_i32(0, st.config.weapon.cooldown_max as i32) as u16;
        self.hash.mark_entity_chunk(id.chunk());
        id
    }

    /// Re-hashes dirty chunks (in parallel) and clears the dirty flags.
    fn refresh_hashes(&mut self) {
        let World {
            state,
            threads,
            hash,
            hash_out,
            dirty_tiles,
            ..
        } = self;
        hash.resize_entities(state.entities.chunk_count());
        state.terrain.take_dirty(dirty_tiles);
        hash.update_tiles(&state.terrain, dirty_tiles);
        let ents = &state.entities;
        let pool = &state.projectiles;
        let squads = state.game.is_some();

        hash.scratch.clear();
        hash.scratch.extend(
            hash.entity_dirty
                .iter()
                .enumerate()
                .filter(|(_, d)| **d)
                .map(|(c, _)| c),
        );
        let dirty = &hash.scratch;
        threads.install(|| {
            dirty
                .par_iter()
                .map(|&c| hash::entity_chunk(ents, c, squads))
                .collect_into_vec(hash_out)
        });
        for (&c, &h) in hash.scratch.iter().zip(hash_out.iter()) {
            hash.entity[c] = h;
            hash.entity_dirty[c] = false;
        }

        hash.scratch.clear();
        hash.scratch.extend(
            hash.pool_dirty
                .iter()
                .enumerate()
                .filter(|(_, d)| **d)
                .map(|(c, _)| c),
        );
        let dirty = &hash.scratch;
        threads.install(|| {
            dirty
                .par_iter()
                .map(|&c| hash::pool_chunk(pool, c))
                .collect_into_vec(hash_out)
        });
        for (&c, &h) in hash.scratch.iter().zip(hash_out.iter()) {
            hash.pool[c] = h;
            hash.pool_dirty[c] = false;
        }
    }

    fn fill_snapshot(&mut self) {
        self.snapshot.fill(
            self.state.tick,
            &self.state.entities,
            &self.state.projectiles,
            self.state.game.as_ref(),
            &self.dirty_tiles,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::{Velocity, Weapon};

    fn small() -> SimConfig {
        SimConfig {
            soldiers: 40,
            map_size_m: 128,
            projectile_cap: 64,
            ..SimConfig::default()
        }
    }

    /// A world a few ticks in, with one freed id and some projectiles.
    fn busy_world() -> World {
        let mut w = World::new(small(), 1).unwrap();
        w.step(&[Command {
            player: PlayerId(1),
            seq: 0,
            kind: CommandKind::Despawn { unit: SimId(3) },
        }]);
        for _ in 0..30 {
            w.step(&[]);
        }
        assert!(w.projectiles().live() > 0);
        w
    }

    /// Saves `w` after `edit` changes its state behind the validators' back.
    fn save_edited(w: &World, edit: impl FnOnce(&mut SimState)) -> Vec<u8> {
        let mut state = w.state.clone();
        edit(&mut state);
        postcard::to_allocvec(&SaveRef {
            magic: SAVE_MAGIC,
            version: SAVE_VERSION,
            state: &state,
        })
        .unwrap()
    }

    fn load_err(bytes: &[u8]) -> String {
        match World::load(bytes, 1) {
            Ok(_) => panic!("load accepted a bad save"),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn load_accepts_clean_save() {
        let w = busy_world();
        let loaded = World::load(&w.save(), 2).unwrap();
        assert_eq!(loaded.state_hash(), w.state_hash());
    }

    #[test]
    fn load_rejects_state_hidden_from_the_hash() {
        // A vacant slot with a velocity hashes like a clean one, so it must
        // not load (the next spawn into that id would otherwise inherit it).
        let w = busy_world();
        let bytes = save_edited(&w, |s| {
            s.entities.velocity[3] = Velocity {
                lin: FxVec2::from_int(5, 0),
            };
        });
        assert!(load_err(&bytes).contains("vacant"));
        let bytes = save_edited(&w, |s| {
            s.entities.weapon[3] = Weapon {
                cooldown: 0,
                shots: 9,
            }
        });
        assert!(load_err(&bytes).contains("vacant"));
    }

    #[test]
    fn load_rejects_inconsistent_allocators() {
        let w = busy_world();
        // Duplicate free id: would hand id 3 out twice.
        let bytes = save_edited(&w, |s| s.entities.alloc = dup_free(&s.entities.alloc));
        assert!(load_err(&bytes).contains("duplicate"));
        // Duplicate projectile slot.
        let bytes = save_edited(&w, |s| {
            let mut pool = ProjectilePool::new(s.config.projectile_cap);
            pool.free_mut_for_test().push(0);
            s.projectiles = pool;
        });
        assert!(load_err(&bytes).contains("duplicate"));
        // Live entity owned by a player outside the config.
        let bytes = save_edited(&w, |s| s.entities.owner[0] = PlayerId(7));
        assert!(load_err(&bytes).contains("player"));
    }

    fn dup_free(a: &crate::ecs::IdAllocator) -> crate::ecs::IdAllocator {
        let mut a = a.clone();
        let first = a.free_list()[0];
        a.free_mut_for_test().push(first);
        a
    }

    #[test]
    fn load_rejects_bad_terrain() {
        let w = busy_world();
        let other = Heightfield::generate(TerrainKind::Flat, 64, 1);
        let bytes = save_edited(&w, |s| s.terrain = other.with_size_for_test(128));
        assert!(load_err(&bytes).contains("terrain"));
    }

    #[test]
    fn load_rejects_oversized_config() {
        let w = busy_world();
        let bytes = save_edited(&w, |s| s.config.projectile_cap = u32::MAX);
        assert!(load_err(&bytes).contains("projectile_cap"));
    }

    fn match_world() -> World {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
        let defs = frontline_content::ModDefs::load(&root).unwrap();
        let req = frontline_content::MatchRequest::skirmish(
            1,
            "ww2:germany",
            "ww2:usa",
            frontline_content::Difficulty::Normal,
        );
        let mut w = World::new_match(defs.resolve(&req).unwrap(), 1).unwrap();
        for _ in 0..5 {
            w.step(&[]);
        }
        w
    }

    #[test]
    fn load_accepts_clean_match_save() {
        let w = match_world();
        let loaded = World::load(&w.save(), 2).unwrap();
        assert_eq!(loaded.state_hash(), w.state_hash());
        assert_eq!(loaded.state(), w.state());
    }

    #[test]
    fn load_rejects_inconsistent_match_state() {
        let w = match_world();
        // A squad member that no longer links back to its squad.
        let bytes = save_edited(&w, |s| s.entities.squad[0] = None);
        assert!(load_err(&bytes).contains("squad"));
        // A soldier claiming a squad that does not list him.
        let bytes = save_edited(&w, |s| {
            let g = s.game.as_mut().unwrap();
            let first = *g.squads.keys().next().unwrap();
            g.squads.get_mut(&first).unwrap().members.pop();
        });
        assert!(load_err(&bytes).contains("squad"));
        // Config that does not match the setup.
        let bytes = save_edited(&w, |s| s.config.projectile_cap += 1);
        assert!(load_err(&bytes).contains("config"));
        // Out-of-range squad type.
        let bytes = save_edited(&w, |s| {
            let g = s.game.as_mut().unwrap();
            g.squads.values_mut().next().unwrap().type_idx = 999;
        });
        assert!(load_err(&bytes).contains("squad"));
        // Match state dropped from a map world.
        let bytes = save_edited(&w, |s| s.game = None);
        assert!(load_err(&bytes).contains("match"));
    }

    #[test]
    fn terrain_edits_are_hashed_and_checked_on_load() {
        let mut w = match_world();
        let before = w.full_state_hash();
        let c = FxVec2::from_int(300, 300);
        w.state.terrain.crater(c, Fx::from_int(2), Fx::ONE, 3);
        w.refresh_hashes();
        assert_eq!(w.dirty_tiles, vec![(18, 18)]);
        assert_ne!(w.full_state_hash(), before);
        assert_eq!(w.state_hash(), w.full_state_hash());
        let loaded = World::load(&w.save(), 2).unwrap();
        assert_eq!(loaded.state(), w.state());
        assert_eq!(loaded.state_hash(), w.state_hash());
        // Each tile height is covered: change one behind the cache.
        let mut other = w.state.clone();
        other
            .terrain
            .edit((1200, 1200), (1200, 1200), 3, |_, _, _, h| h + Fx::ONE);
        assert_ne!(
            hash::full_hash(&other, &w.config_bytes, &w.setup_bytes),
            w.full_state_hash()
        );
        // A tile from the future, or listed twice, does not load.
        let bytes = save_edited(&w, |s| s.tick = 2);
        assert!(load_err(&bytes).contains("future"));
        let bytes = save_edited(&w, |s| {
            let mut t = s.terrain.clone();
            t.crater(FxVec2::from_int(400, 400), Fx::from_int(2), Fx::ONE, 3);
            t.duplicate_first_tile_for_test();
            s.terrain = t;
        });
        assert!(load_err(&bytes).contains("twice"));
    }

    #[test]
    fn bench_world_rejects_map_terrain_and_squad_orders() {
        let cfg = SimConfig {
            terrain: TerrainKind::Map,
            ..small()
        };
        assert!(World::new(cfg, 1).is_err());
        let mut w = World::new(small(), 1).unwrap();
        w.step(&[Command {
            player: PlayerId(0),
            seq: 0,
            kind: CommandKind::Surrender,
        }]);
        assert_eq!(w.stats().commands_rejected, 1);
    }

    #[test]
    fn spawn_resets_reused_slot() {
        let mut e = Entities::default();
        let a = e.spawn(EntityKind::Soldier, PlayerId(0));
        e.despawn(a);
        // Dirty the vacant slot directly (as a bad save could have).
        e.velocity[a.index()].lin = FxVec2::from_int(3, 4);
        e.mover[a.index()].legs = 5;
        let b = e.spawn(EntityKind::Soldier, PlayerId(1));
        assert_eq!(a, b);
        assert_eq!(e.velocity[b.index()], Velocity::default());
        assert_eq!(e.mover[b.index()].legs, 0);
    }

    #[test]
    fn validate_bounds_squared_distances_and_weapons() {
        let ok = SimConfig::default();
        assert!(ok.validate().is_ok());
        let mut c = ok.clone();
        c.soldier.neighbour_dist = Fx::from_int(200);
        assert!(c.validate().is_err(), "neighbour_dist beyond 64 m");
        let mut c = ok.clone();
        c.soldier.arrive_radius = Fx::from_int(200);
        assert!(c.validate().is_err(), "arrive_radius beyond 64 m");
        let mut c = ok.clone();
        c.weapon.drag = -Fx::ONE;
        assert!(c.validate().is_err(), "negative drag");
        let mut c = ok.clone();
        c.weapon.muzzle_speed_min = Fx::from_int(-20_000);
        assert!(c.validate().is_err(), "negative muzzle speed");
        let mut c = ok.clone();
        c.weapon.muzzle_speed_max = Fx::from_int(5000);
        assert!(c.validate().is_err(), "muzzle speed that tunnels terrain");
        let mut c = ok.clone();
        c.spawn_area_m = ok.map_size_m + 64;
        assert!(c.validate().is_err(), "spawn area larger than the map");
        let mut c = ok;
        c.soldiers = SimConfig::MAX_SOLDIERS + 1;
        assert!(c.validate().is_err(), "soldier count bound");
    }

    #[test]
    fn spawn_area_keeps_soldiers_central() {
        let cfg = SimConfig {
            map_size_m: 1024,
            spawn_area_m: 128,
            soldiers: 200,
            ..SimConfig::default()
        };
        let w = World::new(cfg, 1).unwrap();
        let (lo, hi) = (Fx::from_int(448), Fx::from_int(576));
        for t in w.entities().transform() {
            let p = t.pos.xy();
            assert!(p.x >= lo && p.x < hi && p.y >= lo && p.y < hi, "{p:?}");
        }
    }

    #[test]
    fn spawn_yaw_and_speed_are_independent_of_position() {
        // Regression: position and yaw/speed used to come from the same
        // stream, making yaw an exact linear function of x.
        let w = World::new(small(), 1).unwrap();
        let size = w.terrain().size().raw() as i64;
        let ents = w.entities();
        let linear = ents.soldiers().all(|id| {
            let t = ents.transform()[id.index()];
            let expect = -Fx::PI.raw() as i64 + t.pos.x.raw() as i64 * Fx::TAU.raw() as i64 / size;
            (t.yaw.raw() as i64 - expect).abs() <= 2
        });
        assert!(!linear);
    }
}
