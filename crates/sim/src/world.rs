//! The world: saved state, configuration, the tick loop and save/load.
//!
//! [`World::step`] runs one tick in the fixed stage order of §5.2:
//!
//! | # | Stage | Work | Parallel? |
//! |---|-------|---------|-----------|
//! | 1 | Intent | sorted command apply (match: human and due AI commands together; S3b: an accepted call-in spawns its squad here); bench: waypoint re-pick; match: AI plans ([`crate::ai`]), path queue, digging (terrain edits), squad steering | serial apply, per-chunk re-pick; match work serial |
//! | 2 | Perception | match: spotting per player ([`crate::spotting`]) | per player, serial apply |
//! | 3 | Movement | spatial hash and (S3) vehicle index/frames rebuild; ORCA plan (soldiers also avoid vehicle circles); integrate (pinned men stay); S3: vehicles follow their paths, `drive` and push soldiers out, then the vehicle index/frames are rebuilt ([`crate::vehicle`]) | serial rebuilds, per-chunk plan + integrate, serial vehicles |
//! | 4 | Weapons | bench: cooldowns, random fire; match: marks, squad focus, targeting and fire, mortars ([`crate::combat`]), throw wind-ups and releases ([`crate::items`]); sorted spawn into the pool | per-chunk, serial focus/mortars/throws/spawn |
//! | 5 | Ballistics | 60 Hz substeps, ground test (match: capsule hits, near misses; thrown items), despawn | per-slot-chunk, serial free-list merge and thrown items |
//! | 6 | Damage | match: hits, suppression, HE bursts and craters, thrown items going off (blasts, smoke), deaths | serial |
//! | 7 | Status | match: suppression decay, stance, timers, corpse removal; smoke ageing and resupply; capture, income, tickets, victory ([`crate::rules`]) | per-chunk, serial removal, items and rules |
//! | 8 | World/Hash | tick++, dirty chunk and terrain tile re-hash, vehicle block layer update (S3), publish every K; snapshot | per-chunk hash |
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

use crate::combat::{self, BallisticEvent, Explosion, FireCtx, FlightCtx, Mark, StatusCtx};
use crate::command::{Command, CommandKind};
use crate::damage::ArmourEv;
use crate::ecs::{CHUNK_SIZE, Entities, EntityKind, PlayerId, SimId};
use crate::fx::{Fx, FxVec2};
use crate::game::{Ctx, MatchState, Nav, NavPhase, Order, SquadId};
use crate::gunnery::GunCtx;
use crate::hash::{self, HashCache};
use crate::path::PathScratch;
use crate::projectile::{NO_WEAPON, ProjectileEnd, ProjectilePool, SUBSTEP_HZ};
use crate::rng::{Purpose, RngStream};
use crate::snapshot::RenderSnapshot;
use crate::spatial::SpatialHash;
use crate::spotting::{self, SpotPass};
use crate::systems::{self, FireRequest};
use crate::terrain::{Heightfield, TerrainKind};
use crate::vehicle::{Obstacles, VehicleState};

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
    // Soldier speeds only: vehicles (S3) do not walk.
    let speeds = setup
        .squad_types
        .iter()
        .filter(|t| t.vehicle.is_none())
        .map(|t| Fx::from(t.speed));
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
    /// Match mode (hashed only there): rounds that hit a soldier, near
    /// misses, HE bursts (shells and thrown items), soldiers killed and
    /// items thrown.
    pub soldier_hits: u64,
    pub near_misses: u64,
    pub explosions: u64,
    pub deaths: u64,
    pub throws: u64,
    /// S3 (match mode, hashed): armour slab tests in all ticks and the most
    /// in one tick; small-arms rounds stopped by vehicles and wrecks;
    /// main-gun and rocket hits on plates by result; vehicles knocked out;
    /// tracks hit by blasts beside a vehicle; launcher rockets fired.
    pub armour_box_tests: u64,
    pub armour_box_peak: u64,
    pub rounds_stopped: u64,
    pub ricochets: u64,
    pub non_penetrations: u64,
    pub penetrations: u64,
    pub knockouts: u64,
    pub blast_track_hits: u64,
    pub rockets_fired: u64,
    /// S3b (match mode, hashed): fragment rays thrown, fragments that
    /// struck a man or a track, and rays cut by the per-tick budget.
    pub frag_rays: u64,
    pub frag_hits: u64,
    pub frag_rays_capped: u64,
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
    /// Path node expansions served in the tick (match mode).
    pub path_expansions: u32,
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
/// and dig jobs. 4: combat (entity `combat` column, corpses, projectile
/// weapons, squad focus, kills, spotting cursors, combat counters).
/// 5: match rules (defeats, point time, income and drain fractions,
/// contested points) and AI commander state. 6: weapon fire intervals in
/// milli-ticks (`WeaponSetup::interval_milli`). 7: soldier-level orders
/// (entity `unit` column, unit dig jobs, soldier path requests, the
/// `prone_speed_pct` movement rule). 8: thrown items (entity `kit` column,
/// items in flight, smoke clouds, item defs and loadouts in the setup, the
/// item and AI throw rules, AI throw cooldowns, the `throws` counter).
/// 9: S3 vehicles and armour (ammo and vehicle defs, AT teams, dig
/// profiles, vehicle and AI rules in the setup; vehicle state as the S3
/// sim stages add it). One bump for the whole slice. 10: S3b (the tuning
/// tables in the setup's rules, each vehicle's `elevate_deg_s`, the dig
/// profiles' `common`; the slice's damage, armour and pinning state as the
/// S3b stages add it). One bump for the whole slice. 11: S3c (bail-out:
/// the abandoned entity kind, seat `untrained`, the vehicle's bail and
/// boarding state, the `Recrew` order, per-player crew counters, the
/// vehicles' crew squad and hatches and the bail and re-crew tuning tables
/// in the setup). One bump for the whole slice. 12: S4 (squad types by
/// `[[members]]` slot with tags, capabilities, training, card block and
/// limit groups in the setup; each man's weapon and slot in his kit;
/// crew-weapon drops, the squad's single-shot AT state, per-player S4
/// counters and the AI's per-type buy counters; the S4 rules keys such as
/// `items.crew_pickup_reach_m`). One bump for the whole slice. 13: S5a
/// (cartridges, supply weights, feeds, member rounds, `rare_cap`, the
/// `[supply]` and `[combat.share]` rules in the setup; per-player ammo
/// accounts; counted rounds per man, sidearm and refill state in the kit;
/// the rounds left in a dropped crew weapon). One bump for the whole slice.
const SAVE_VERSION: u32 = 13;

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
    /// AI commands due this tick (match mode), merged with the human ones.
    ai_cmds: Vec<Command>,
    hash_out: Vec<u64>,
    path_scratch: PathScratch,
    /// Terrain tiles changed in the last tick (for the snapshot).
    dirty_tiles: Vec<(u16, u16)>,
    // Match combat scratch, and the last tick's events for the snapshot.
    spot_passes: Vec<SpotPass>,
    marks: Vec<Mark>,
    mark_of: Vec<u32>,
    blow_bufs: Vec<Vec<BallisticEvent>>,
    /// Armour slab tests per projectile chunk (S3).
    box_test_bufs: Vec<u64>,
    blows: Vec<BallisticEvent>,
    gone_bufs: Vec<Vec<SimId>>,
    /// Rounds fired in the last tick (including dropped ones), sorted.
    shots: Vec<FireRequest>,
    explosions: Vec<Explosion>,
    /// Damage-stage scratch: thrown items going off this tick.
    fused: Vec<crate::items::Thrown>,
    /// Armour events of the last tick (S3), for the snapshot.
    armour_events: Vec<ArmourEv>,
    /// Call-in results of the last tick (S3b), for the snapshot.
    call_in_events: Vec<crate::game::CallInEvent>,
    /// S3c: refused re-crews of the last tick, for the snapshot.
    recrew_events: Vec<crate::bail::RecrewEvent>,
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
            ai_cmds: Vec::new(),
            hash_out: Vec::new(),
            path_scratch: PathScratch::default(),
            dirty_tiles: Vec::new(),
            spot_passes: Vec::new(),
            marks: Vec::new(),
            mark_of: Vec::new(),
            blow_bufs: Vec::new(),
            box_test_bufs: Vec::new(),
            blows: Vec::new(),
            gone_bufs: Vec::new(),
            shots: Vec::new(),
            explosions: Vec::new(),
            fused: Vec::new(),
            armour_events: Vec::new(),
            call_in_events: Vec::new(),
            recrew_events: Vec::new(),
            state,
        };
        if let Some(g) = world.state.game.as_mut() {
            g.rebuild_vehicle_derived(&world.state.entities, &world.state.terrain);
        }
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
            None if st
                .entities
                .kind
                .iter()
                .zip(&st.entities.combat)
                .any(|(k, c)| {
                    matches!(
                        k,
                        EntityKind::Corpse
                            | EntityKind::Vehicle
                            | EntityKind::Wreck
                            | EntityKind::Abandoned
                    ) || *c != Default::default()
                })
                || st.entities.unit.iter().any(|u| *u != Default::default())
                || st.entities.kit.iter().any(|k| *k != Default::default())
                || st
                    .projectiles
                    .slots()
                    .iter()
                    .flatten()
                    .any(|p| p.weapon != NO_WEAPON) =>
            {
                return Err(err("combat state in a bench world"));
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
                g.validate(&st.entities, st.tick).map_err(err)?;
                let (nw, np) = (g.setup().weapons.len(), g.setup().players.len());
                let na = g.setup().ammo.len();
                let size = Fx::from_int(g.setup().map.size_m);
                let on_map = |q: crate::fx::FxVec3| {
                    (Fx::ZERO..=size).contains(&q.x) && (Fx::ZERO..=size).contains(&q.y)
                };
                if st.projectiles.slots().iter().flatten().any(|p| {
                    p.weapon as usize >= nw
                        || p.player as usize >= np
                        || (p.ammo != crate::projectile::NO_AMMO && p.ammo as usize >= na)
                        || !on_map(p.origin)
                }) {
                    return Err(err("projectile with an unknown weapon, round or player"));
                }
            }
        }
        s.state.terrain.rebuild_derived();
        if let Some(g) = &mut s.state.game {
            g.rebuild_derived();
        }
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
        self.armour_events.clear();
        self.apply_commands(cmds);
        let mut path_expansions = 0;
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
            game.run_ai(ctx.ents, ctx.terrain, *tick);
            game.approach_attacks(&mut ctx);
            path_expansions = game.serve_paths(&mut ctx, path_scratch);
            game.dig_work(entities, terrain, hash, *tick);
            let mut ctx = Ctx {
                ents: entities,
                terrain,
                hash,
                tick: *tick,
            };
            game.steer_escorts(&mut ctx);
            game.steer_squads(&mut ctx);
            game.steer_units(&mut ctx);
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

        // 2. Perception: spotting, one task per player reading the previous
        //    tick's state; results applied serially in player order.
        if self.state.game.is_some() {
            let World {
                state,
                threads,
                hash,
                spot_passes,
                ..
            } = self;
            let SimState {
                entities,
                terrain,
                game,
                ..
            } = state;
            let game = game.as_mut().expect("match world");
            spot_passes.resize_with(game.players.len(), SpotPass::default);
            {
                let (ents, terrain, g) = (&*entities, &*terrain, &*game);
                threads.install(|| {
                    spot_passes.par_iter_mut().enumerate().for_each(|(p, out)| {
                        spotting::spot(PlayerId(p as u8), ents, terrain, g, tick, out)
                    })
                });
            }
            for (p, pass) in spot_passes.iter().enumerate() {
                game.spot_cursor[p] = pass.cursor;
                for &(id, seen) in &pass.changes {
                    let c = &mut entities.combat[id.index()];
                    if seen {
                        c.seen_by |= 1 << p;
                    } else {
                        c.seen_by &= !(1u16 << p);
                    }
                    hash.mark_entity_chunk(id.chunk());
                }
            }
        }
        let t2 = Instant::now();

        // 3. Movement: rebuild the spatial hash (and, S3, the vehicle index
        //    and frames), plan (reads only the previous state), then
        //    integrate (writes own entity only); then vehicles drive
        //    serially in ascending id and the vehicle index is rebuilt.
        {
            let World {
                state,
                threads,
                hash,
                flags,
                spatial,
                planned,
                armour_events,
                ..
            } = self;
            let SimState {
                entities,
                terrain,
                config,
                game,
                ..
            } = state;
            spatial.rebuild(entities);
            if let Some(g) = game.as_mut() {
                g.rebuild_vehicle_index(entities);
            }
            planned.clear();
            planned.resize(entities.len(), FxVec2::ZERO);
            {
                let (ents, spatial, params) = (&*entities, &*spatial, &config.soldier);
                let obstacles =
                    game.as_ref()
                        .filter(|g| !g.vehicles.is_empty())
                        .map(|g| Obstacles {
                            derived: &g.vderived,
                            velocity: &ents.velocity,
                            slide_gap: Fx::from_milli(
                                g.setup().rules.movement.formation.slide_gap_mm as i32,
                            ),
                        });
                let obstacles = obstacles.as_ref();
                threads.install(|| {
                    planned
                        .par_chunks_mut(CHUNK_SIZE)
                        .enumerate()
                        .for_each(|(c, out)| {
                            systems::plan_velocities(c, out, ents, spatial, obstacles, params, hz)
                        })
                });
            }
            let Entities {
                kind,
                transform,
                velocity,
                combat,
                ..
            } = entities;
            let (terrain, planned, combat) = (&*terrain, &*planned, &*combat);
            let steep = game.as_ref().map_or(systems::BENCH_STEEP, |g| {
                let f = &g.setup().rules.movement.formation;
                systems::Steep {
                    free_pct: f.steep_free_pct as i64,
                    min_pct: f.steep_min_pct as i64,
                }
            });
            threads.install(|| {
                kind.par_chunks(CHUNK_SIZE)
                    .zip(transform.par_chunks_mut(CHUNK_SIZE))
                    .zip(velocity.par_chunks_mut(CHUNK_SIZE))
                    .zip(combat.par_chunks(CHUNK_SIZE))
                    .zip(planned.par_chunks(CHUNK_SIZE))
                    .map(|((((k, t), v), c), p)| {
                        systems::integrate_soldiers(k, t, v, c, p, terrain, hz, steep)
                    })
                    .collect_into_vec(flags)
            });
            hash.merge_entity_flags(flags);
            let soldier_r = state.config.soldier.radius;
            if let Some(g) = state.game.as_mut() {
                g.move_vehicles(
                    &mut state.entities,
                    &state.terrain,
                    spatial,
                    soldier_r,
                    hash,
                    seed,
                    tick,
                    armour_events,
                );
                g.rebuild_vehicle_index(&state.entities);
                // Ballistics and blasts test soldiers where they now stand.
                spatial.rebuild(&state.entities);
            }
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
        } else {
            let World {
                state,
                threads,
                hash,
                flags,
                fire_bufs,
                fire_merged,
                marks,
                mark_of,
                shots,
                spatial,
                armour_events,
                ..
            } = self;
            let SimState {
                entities,
                terrain,
                projectiles,
                stats,
                game,
                ..
            } = state;
            let game = game.as_mut().expect("match world");
            combat::build_marks(entities, marks, mark_of, &game.setup().rules.combat.body);
            game.update_focus(entities, marks, mark_of);
            fire_bufs.resize_with(entities.chunk_count(), Vec::new);
            {
                let Entities {
                    kind,
                    owner,
                    transform,
                    velocity,
                    squad,
                    combat,
                    weapon,
                    unit,
                    kit,
                    ..
                } = &mut *entities;
                let ctx = FireCtx {
                    kind,
                    owner,
                    transform,
                    velocity,
                    squad,
                    unit,
                    marks,
                    mark_of,
                    game: &*game,
                    terrain,
                    seed,
                    tick,
                };
                threads.install(|| {
                    combat
                        .par_chunks_mut(CHUNK_SIZE)
                        .zip(weapon.par_chunks_mut(CHUNK_SIZE))
                        .zip(kit.par_chunks_mut(CHUNK_SIZE))
                        .zip(fire_bufs.par_iter_mut())
                        .enumerate()
                        .map(|(c, (((cb, w), k), out))| {
                            out.clear();
                            combat::aim_and_fire(c, &ctx, cb, w, k, out)
                        })
                        .collect_into_vec(flags)
                });
            }
            hash.merge_entity_flags(flags);
            fire_merged.clear();
            for b in fire_bufs.iter() {
                fire_merged.extend_from_slice(b);
            }
            game.fire_launchers(
                entities,
                terrain,
                hash,
                seed,
                tick,
                fire_merged,
                &mut stats.rockets_fired,
            );
            game.fire_mortars(entities, terrain, hash, seed, tick, fire_merged);
            let gun_ctx = GunCtx {
                terrain,
                marks,
                mark_of,
                soldiers: spatial,
                seed,
                tick,
            };
            game.vehicle_gunnery(entities, &gun_ctx, hash, fire_merged, armour_events);
            stats.throws += game.release_throws(entities, terrain, hash, seed);
            fire_merged.sort_unstable();
            // S5a: every requested round is spent and counted (a request
            // the pool drops below included).
            game.count_fired(entities, fire_merged);
            let (fired, dropped) = combat::spawn_rounds(fire_merged, projectiles, hash);
            stats.shots_fired += fired;
            stats.shots_dropped += dropped;
            shots.clone_from(fire_merged);
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
                blow_bufs,
                box_test_bufs,
                blows,
                spatial,
                ..
            } = self;
            let SimState {
                projectiles,
                terrain,
                stats,
                entities,
                game,
                ..
            } = state;
            let (entities, game, spatial) = (&*entities, game.as_ref(), &*spatial);
            ended_bufs.resize_with(projectiles.chunk_count(), Vec::new);
            let terrain = &*terrain;
            match game {
                None => threads.install(|| {
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
                }),
                Some(g) => {
                    blow_bufs.resize_with(ended_bufs.len(), Vec::new);
                    box_test_bufs.resize(ended_bufs.len(), 0);
                    let ctx = FlightCtx {
                        ents: entities,
                        spatial,
                        terrain,
                        weapons: &g.setup().weapons,
                        near_miss: g.setup().rules.combat.near_miss.into(),
                        body: &g.setup().rules.combat.body,
                        vehicles: (!g.vehicles.is_empty()).then_some(&g.vderived),
                        armour: g.armour(),
                        ammo: &g.setup().ammo,
                    };
                    threads.install(|| {
                        projectiles
                            .slots_mut()
                            .par_chunks_mut(CHUNK_SIZE)
                            .zip(ended_bufs.par_iter_mut())
                            .zip(blow_bufs.par_iter_mut())
                            .zip(box_test_bufs.par_iter_mut())
                            .enumerate()
                            .map(|(c, (((s, out), ev), tests))| {
                                out.clear();
                                ev.clear();
                                *tests = 0;
                                combat::fly(c, s, &ctx, substeps, out, ev, tests)
                            })
                            .collect_into_vec(flags)
                    });
                    blows.clear();
                    for b in blow_bufs.iter() {
                        blows.extend_from_slice(b);
                    }
                    let tests: u64 = box_test_bufs.iter().sum();
                    stats.armour_box_tests += tests;
                    stats.armour_box_peak = stats.armour_box_peak.max(tests);
                }
            }
            if let Some(g) = state.game.as_mut() {
                g.fly_thrown(&state.terrain, substeps);
            }
            hash.merge_pool_flags(flags);
            for buf in ended_bufs.iter() {
                for &(slot, end) in buf {
                    projectiles.release(slot);
                    hash.mark_pool_slot(slot);
                    match end {
                        ProjectileEnd::GroundHit => stats.ground_hits += 1,
                        // Counted by the damage stage.
                        ProjectileEnd::SoldierHit | ProjectileEnd::VehicleHit => {}
                        ProjectileEnd::Expired => stats.expired += 1,
                        ProjectileEnd::OutOfBounds => stats.out_of_bounds += 1,
                    }
                }
            }
        }
        let t5 = Instant::now();

        // 6. Damage: serial, in projectile slot order.
        self.explosions.clear();
        if self.state.game.is_some() {
            let World {
                state,
                hash,
                spatial,
                blows,
                explosions,
                fused,
                armour_events,
                ..
            } = self;
            let SimState {
                entities,
                terrain,
                game,
                stats,
                ..
            } = state;
            let mut ctx = combat::DamageCtx {
                ents: entities,
                terrain,
                spatial,
                hash,
                stats,
                seed,
                tick,
            };
            game.as_mut().expect("match world").apply_blows(
                &mut ctx,
                blows,
                explosions,
                fused,
                armour_events,
            );
        }
        let t6 = Instant::now();

        // 7. Status: per-chunk stance and timers, then corpse removal in
        //    ascending id.
        if self.state.game.is_some() {
            let World {
                state,
                threads,
                hash,
                flags,
                gone_bufs,
                ..
            } = self;
            let SimState {
                entities,
                terrain,
                game,
                ..
            } = state;
            let game = game.as_ref().expect("match world");
            gone_bufs.resize_with(entities.chunk_count(), Vec::new);
            {
                let Entities {
                    kind,
                    transform,
                    velocity,
                    weapon,
                    squad,
                    combat,
                    unit,
                    kit,
                    ..
                } = &mut *entities;
                let ctx = StatusCtx {
                    kind,
                    transform,
                    velocity,
                    weapon,
                    squad,
                    unit,
                    kit,
                    game,
                    terrain,
                };
                threads.install(|| {
                    combat
                        .par_chunks_mut(CHUNK_SIZE)
                        .zip(gone_bufs.par_iter_mut())
                        .enumerate()
                        .map(|(c, (cb, gone))| {
                            gone.clear();
                            combat::status(c, &ctx, cb, gone)
                        })
                        .collect_into_vec(flags)
                });
            }
            hash.merge_entity_flags(flags);
            for gone in gone_bufs.iter() {
                for &id in gone {
                    entities.despawn(id);
                    hash.mark_entity_chunk(id.chunk());
                }
            }
            if let Some(game) = &mut self.state.game {
                let mut ctx = Ctx {
                    ents: &mut self.state.entities,
                    terrain: &self.state.terrain,
                    hash: &mut self.hash,
                    tick,
                };
                game.vehicle_status(
                    &mut ctx,
                    &mut self.state.stats,
                    seed,
                    &mut self.armour_events,
                );
                game.update_items(&mut self.state.entities, &mut self.hash);
                // S5a: crew feed, sharing and the priced base refill.
                game.supply_tick(&mut self.state.entities, &mut self.hash, tick);
                // S5a: squads holding fire spring to discipline.
                game.spring_holds(&mut self.state.entities, &mut self.hash);
                game.update_rules(&self.state.entities);
            }
        }
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
            path_expansions,
        };
        published
    }

    /// Starts (or restarts, empty) the diagnostic log of main-gun and rocket
    /// hits on live vehicles with the inputs of the penetration rule
    /// (`armour-test`). Never read by the sim; not saved.
    pub fn enable_hit_log(&mut self) {
        if let Some(g) = self.state.game.as_mut() {
            g.hit_log = Some(Vec::new());
        }
    }

    /// Takes the hits logged since the last call (empty when off).
    pub fn take_hit_log(&mut self) -> Vec<crate::damage::HitRecord> {
        self.state
            .game
            .as_mut()
            .and_then(|g| g.hit_log.as_mut())
            .map(std::mem::take)
            .unwrap_or_default()
    }

    // -------------------------------------------------------- setup tools --
    //
    // Direct state edits for tests and tools that stage a situation (the
    // CLI's `fortify-test`). They are not commands: using them in a
    // networked match would break lockstep. The state hash stays current.

    /// Puts squad `id`'s men on a line through `center` across `facing` (a
    /// unit vector), holding and idle. Returns false for an unknown squad.
    /// A vehicle squad's vehicle is placed at `center` facing `facing`
    /// ([`World::place_vehicle`]).
    pub fn place_squad(&mut self, id: SquadId, center: FxVec2, facing: FxVec2) -> bool {
        if let Some(g) = self.state.game.as_ref()
            && g.is_vehicle_squad(id)
        {
            let v = g.squads[&id].members[0];
            return self.place_vehicle(v, center, Fx::atan2(facing.y, facing.x));
        }
        let Some(game) = self.state.game.as_mut() else {
            return false;
        };
        let Some(sq) = game.squads.get_mut(&id) else {
            return false;
        };
        let spacing: Fx = game.setup.squad_types[sq.type_idx as usize].spacing.into();
        let t = &self.state.terrain;
        let slots: Vec<FxVec2> =
            crate::squad::line_slots(center, facing, sq.members.len(), spacing)
                .into_iter()
                .map(|q| t.clamp(q))
                .collect();
        let yaw = Fx::atan2(facing.y, facing.x);
        for (&m, &q) in sq.members.iter().zip(&slots) {
            let i = m.index();
            let ents = &mut self.state.entities;
            ents.transform[i].pos = q.extend(t.height_at(q));
            ents.transform[i].yaw = yaw;
            ents.velocity[i] = Default::default();
            ents.mover[i].waypoint = q;
            let (stance, fire) = (ents.unit[i].stance, ents.unit[i].fire);
            ents.unit[i] = crate::unit::Unit {
                stance,
                fire,
                ..Default::default()
            };
            self.hash.mark_entity_chunk(m.chunk());
        }
        sq.order = Order::Idle;
        sq.nav = Nav {
            phase: NavPhase::Holding,
            goal: center,
            heading: facing,
            facing,
            slots,
            ..Nav::default()
        };
        let members = sq.members.clone();
        for q in &mut game.path_queue.queues {
            q.retain(|r| match r {
                crate::game::PathReq::Squad(s) => *s != id,
                crate::game::PathReq::Unit(m) => !members.contains(m),
                crate::game::PathReq::Vehicle { squad, .. } => *squad != id,
            });
        }
        self.refresh_hashes();
        self.fill_snapshot();
        true
    }

    /// Digs a finished trench along `line` at once (the rules' profile),
    /// parapet on the left of the line direction for `front = 1`, on the
    /// right for `-1`. Match worlds only.
    pub fn dig_trench_now(&mut self, line: &[FxVec2], front: i8) -> bool {
        let Some(game) = self.state.game.as_ref() else {
            return false;
        };
        if line.len() < 2 {
            return false;
        }
        let dig = &game.setup.rules.dig;
        let n = dig.passes as i32;
        let len = crate::squad::polyline_length(line);
        crate::dig::stamp(
            &mut self.state.terrain,
            line,
            front,
            Fx::ZERO,
            len,
            n,
            n,
            dig,
            self.state.tick,
        );
        self.refresh_hashes();
        self.fill_snapshot();
        true
    }

    /// Spawns a squad of `squad_type` for `player` at once (as a call-in
    /// would, without cost, limits or delay) and places it at `center`
    /// facing `facing` (a unit vector). Match worlds only.
    pub fn spawn_squad_at(
        &mut self,
        player: PlayerId,
        squad_type: u16,
        center: FxVec2,
        facing: FxVec2,
    ) -> Option<SquadId> {
        let game = self.state.game.as_mut()?;
        if player.0 as usize >= game.players.len()
            || squad_type as usize >= game.setup.squad_types.len()
        {
            return None;
        }
        let mut ctx = Ctx {
            ents: &mut self.state.entities,
            terrain: &self.state.terrain,
            hash: &mut self.hash,
            tick: self.state.tick,
        };
        let id = game.spawn_squad(&mut ctx, player, squad_type);
        self.place_squad(id, center, facing);
        Some(id)
    }

    /// Puts vehicle `id` at `pos` heading `yaw`, stopped, settled on the
    /// ground, its squad idle and holding there (S3).
    pub fn place_vehicle(&mut self, id: SimId, pos: FxVec2, yaw: Fx) -> bool {
        let Some(game) = self.state.game.as_mut() else {
            return false;
        };
        let i = id.index();
        let ents = &mut self.state.entities;
        if ents.kind.get(i) != Some(&EntityKind::Vehicle) {
            return false;
        }
        let Some(vs) = game.vehicles.get_mut(&id) else {
            return false;
        };
        let t = &self.state.terrain;
        let xy = t.clamp(pos);
        ents.transform[i].pos = xy.extend(t.height_at(xy));
        ents.transform[i].yaw = yaw.wrap_angle();
        ents.velocity[i] = Default::default();
        vs.speed = Fx::ZERO;
        vs.avg_vel = FxVec2::ZERO;
        vs.reverse = false;
        vs.progress_at = xy;
        vs.progress_ticks = 0;
        let geo = game.vderived.geo[vs.type_idx as usize];
        crate::vehicle::settle(t, &game.vderived.block, &geo, vs, &mut ents.transform[i]);
        if let Some(sid) = ents.squad[i]
            && let Some(sq) = game.squads.get_mut(&sid)
        {
            sq.order = Order::Idle;
            crate::vehicle::hold_vehicle(&mut sq.nav, xy);
            sq.nav.goal = xy;
            let f = crate::vehicle::yaw_dir(yaw);
            sq.nav.heading = f;
            sq.nav.facing = f;
            for q in &mut game.path_queue.queues {
                q.retain(
                    |r| !matches!(r, crate::game::PathReq::Vehicle { squad, .. } if *squad == sid),
                );
            }
        }
        self.hash.mark_entity_chunk(id.chunk());
        game.rebuild_vehicle_index(&self.state.entities);
        self.refresh_hashes();
        self.fill_snapshot();
        true
    }

    /// Edits vehicle `id`'s state directly (staged tests: bogged, burning,
    /// component or seat HP). The caller keeps it within its def's ranges.
    pub fn edit_vehicle(&mut self, id: SimId, f: impl FnOnce(&mut VehicleState)) -> bool {
        let Some(vs) = self
            .state
            .game
            .as_mut()
            .and_then(|g| g.vehicles.get_mut(&id))
        else {
            return false;
        };
        f(vs);
        self.refresh_hashes();
        self.fill_snapshot();
        true
    }

    /// Edits AT team `id`'s launcher directly (staged tests: rockets
    /// left, carrier). The caller keeps it consistent with the squad.
    pub fn edit_launcher(
        &mut self,
        id: SquadId,
        f: impl FnOnce(&mut crate::game::Launcher),
    ) -> bool {
        let Some(l) = self
            .state
            .game
            .as_mut()
            .and_then(|g| g.squads.get_mut(&id))
            .and_then(|sq| sq.launcher.as_mut())
        else {
            return false;
        };
        f(l);
        self.refresh_hashes();
        self.fill_snapshot();
        true
    }

    /// S5a: edits soldier `id`'s kit and combat state directly (staged
    /// tests: rounds carried, magazine). The caller keeps them within his
    /// slot's loads.
    pub fn edit_soldier(
        &mut self,
        id: SimId,
        f: impl FnOnce(&mut crate::items::Kit, &mut crate::ecs::Combat),
    ) -> bool {
        if self.state.entities.kind.get(id.index()) != Some(&EntityKind::Soldier) {
            return false;
        }
        let i = id.index();
        let e = &mut self.state.entities;
        f(&mut e.kit[i], &mut e.combat[i]);
        self.hash.mark_entity_chunk(id.chunk());
        self.refresh_hashes();
        self.fill_snapshot();
        true
    }

    /// S5a: edits player `p`'s state directly (staged tests: manpower,
    /// ammunition credit).
    pub fn edit_player(
        &mut self,
        p: PlayerId,
        f: impl FnOnce(&mut crate::game::PlayerState),
    ) -> bool {
        let Some(pl) = self
            .state
            .game
            .as_mut()
            .and_then(|g| g.players.get_mut(p.0 as usize))
        else {
            return false;
        };
        f(pl);
        self.refresh_hashes();
        self.fill_snapshot();
        true
    }

    /// Kills soldier `id` at once (staged tests): a corpse, out of his
    /// squad, a loss for his owner and a kill for nobody.
    pub fn kill_soldier(&mut self, id: SimId) -> bool {
        let Some(game) = self.state.game.as_mut() else {
            return false;
        };
        if self.state.entities.kind.get(id.index()) != Some(&EntityKind::Soldier) {
            return false;
        }
        let ticks = game.setup.rules.combat.corpse_ticks;
        game.kill(&mut self.state.entities, &mut self.hash, id, ticks);
        self.refresh_hashes();
        self.fill_snapshot();
        true
    }

    /// Starts a field repair of vehicle `id` (staged tests; `RepairUnits`
    /// and the AI call `MatchState::start_repair` the same way). False if
    /// nothing is repairable.
    pub fn start_repair(&mut self, id: SimId) -> bool {
        let Some(game) = self.state.game.as_mut() else {
            return false;
        };
        let ok = game.start_repair(&self.state.entities, id);
        self.refresh_hashes();
        self.fill_snapshot();
        ok
    }

    /// Knocks vehicle `id` out at once and turns it into a wreck (staged
    /// tests): the Damage stage's knock-out (crew lost, followers stopped,
    /// targets cleared, wreck cap).
    pub fn wreck_vehicle(&mut self, id: SimId) -> bool {
        let tick = self.state.tick;
        let Some(game) = self.state.game.as_mut() else {
            return false;
        };
        if self.state.entities.kind.get(id.index()) != Some(&EntityKind::Vehicle) {
            return false;
        }
        let mut ctx = Ctx {
            ents: &mut self.state.entities,
            terrain: &self.state.terrain,
            hash: &mut self.hash,
            tick,
        };
        game.knock_out_with(
            &mut ctx,
            &mut self.state.stats,
            id,
            crate::damage::KO_CREW,
            crate::game::kill_class::OTHER,
            &mut self.armour_events,
        );
        game.rebuild_vehicle_index(&self.state.entities);
        self.refresh_hashes();
        self.fill_snapshot();
        true
    }

    /// Digs a finished AT ditch along `line` at once (the rules' `at_ditch`
    /// profile, berm on the left of the line direction for `front = 1`)
    /// and records the line, so it blocks vehicles (S3). Match worlds only.
    pub fn dig_at_ditch_now(&mut self, line: &[FxVec2], front: i8) -> bool {
        let Some(game) = self.state.game.as_mut() else {
            return false;
        };
        if !(2..=crate::game::MAX_DIG_POINTS).contains(&line.len())
            || !line.iter().all(|q| self.state.terrain.contains(*q))
            || game.at_ditches.len() >= crate::game::MAX_AT_DITCHES
        {
            return false;
        }
        let dig = &game.setup.rules.at_ditch;
        let n = dig.passes as i32;
        let len = crate::squad::polyline_length(line);
        crate::dig::stamp(
            &mut self.state.terrain,
            line,
            front,
            Fx::ZERO,
            len,
            n,
            n,
            dig,
            self.state.tick,
        );
        game.at_ditches.push(line.to_vec());
        game.mark_ditch_block(line);
        self.refresh_hashes();
        self.fill_snapshot();
        true
    }

    // ----------------------------------------------------------- internals --

    fn apply_commands(&mut self, cmds: &[Command]) {
        // Match mode: AI commands due now join the human ones. Indices past
        // `cmds.len()` refer to `ai`.
        let mut ai = std::mem::take(&mut self.ai_cmds);
        ai.clear();
        self.call_in_events.clear();
        self.recrew_events.clear();
        if let Some(game) = &mut self.state.game {
            game.take_ai_commands(self.state.tick, &mut ai);
        }
        let n = cmds.len();
        let get = |i: usize| if i < n { &cmds[i] } else { &ai[i - n] };
        self.cmd_order.clear();
        self.cmd_order.extend(0..n + ai.len());
        // Total order on (player, seq, payload); identical commands are
        // interchangeable, so a stable sort of indices is fully deterministic.
        self.cmd_order.sort_by(|&a, &b| get(a).cmp(get(b)));
        let order = std::mem::take(&mut self.cmd_order);
        for &i in &order {
            let c = get(i);
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
                    if i >= n {
                        game.ai_rejected(c.player);
                    }
                }
                // Call-in results go to the snapshot; the match state
                // never holds them between ticks.
                self.call_in_events.append(&mut game.call_in_events);
                self.recrew_events.append(&mut game.recrew_events);
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
        self.ai_cmds = ai;
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
        // The vehicle block layer follows terrain edits, ditch lines and
        // wrecks once per tick (S3, `crate::vehicle`).
        if let Some(g) = state.game.as_mut() {
            g.update_block(&state.entities, &state.terrain, dirty_tiles);
        }
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
        // Refusals from the Status stage (a limit reached while boarding).
        if let Some(game) = &mut self.state.game {
            self.recrew_events.append(&mut game.recrew_events);
            // S4: crew weapons picked up this tick (Weapons stage).
            self.snapshot.pickups = std::mem::take(&mut game.pickup_events);
        } else {
            self.snapshot.pickups.clear();
        }
        self.snapshot.fill(
            self.state.tick,
            &self.state.entities,
            &self.state.projectiles,
            self.state.game.as_ref(),
            &self.dirty_tiles,
        );
        self.snapshot
            .fill_events(&self.shots, &self.explosions, &self.armour_events);
        self.snapshot.call_ins.clone_from(&self.call_in_events);
        self.snapshot
            .recrew_refusals
            .clone_from(&self.recrew_events);
        self.snapshot
            .fill_shells(&self.state.projectiles, self.state.game.as_ref());
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
        // Seen by a player that does not exist.
        let bytes = save_edited(&w, |s| s.entities.combat[0].seen_by = 1 << 5);
        assert!(load_err(&bytes).contains("combat"));
        // A living soldier at 0 HP, or a corpse still in a squad.
        let bytes = save_edited(&w, |s| s.entities.combat[0].hp = 0);
        assert!(load_err(&bytes).contains("combat"));
        let bytes = save_edited(&w, |s| {
            s.entities.kind[0] = EntityKind::Corpse;
            s.entities.combat[0].corpse = 10;
        });
        assert!(load_err(&bytes).contains("squad"));
        // A round of an unknown weapon.
        let bytes = save_edited(&w, |s| {
            s.projectiles.spawn(crate::projectile::Projectile {
                pos: FxVec2::from_int(10, 10).extend(Fx::from_int(30)),
                vel: Default::default(),
                owner: SimId(0),
                drag: Fx::ZERO,
                ttl: 5,
                weapon: 200,
                player: 0,
                ammo: crate::projectile::NO_AMMO,
                origin: Default::default(),
            });
        });
        assert!(load_err(&bytes).contains("weapon"));
        // Capture progress without a capturer, or past full.
        let bytes = save_edited(&w, |s| s.game.as_mut().unwrap().points[0].progress = 5);
        assert!(load_err(&bytes).contains("capture"));
        let bytes = save_edited(&w, |s| {
            let c = &mut s.game.as_mut().unwrap().points[0];
            c.capturer = Some(PlayerId(0));
            c.progress = u32::MAX;
        });
        assert!(load_err(&bytes).contains("capture"));
        // A winner while both players still stand; a defeat that is not
        // reflected in the outcome.
        let bytes = save_edited(&w, |s| {
            s.game.as_mut().unwrap().outcome = crate::game::Outcome::Won {
                winner: PlayerId(0),
            }
        });
        assert!(load_err(&bytes).contains("outcome"));
        let bytes = save_edited(&w, |s| {
            s.game.as_mut().unwrap().players[1].defeat = Some(crate::rules::Defeat::Tickets)
        });
        assert!(load_err(&bytes).contains("outcome"));
        // Income fraction past a whole unit.
        let bytes = save_edited(&w, |s| {
            s.game.as_mut().unwrap().players[0].income_acc = 1 << 30
        });
        assert!(load_err(&bytes).contains("player"));
        // AI state on a human player, AI state dropped, and a bench command
        // smuggled into the AI queue.
        let bytes = save_edited(&w, |s| {
            let g = s.game.as_mut().unwrap();
            g.ai[0] = g.ai[1].clone();
        });
        assert!(load_err(&bytes).contains("AI"));
        let bytes = save_edited(&w, |s| s.game.as_mut().unwrap().ai[1] = None);
        assert!(load_err(&bytes).contains("AI"));
        let bytes = save_edited(&w, |s| {
            let ai = s.game.as_mut().unwrap().ai[1].as_mut().unwrap();
            ai.pending.push(crate::ai::AiCommand {
                due: u64::MAX,
                seq: 0,
                kind: CommandKind::Spawn { pos: FxVec2::ZERO },
            });
        });
        assert!(load_err(&bytes).contains("AI"));
        // A pending call-in of an unknown squad type would crash the next
        // plan (`buy` indexes the squad types with it).
        let bytes = save_edited(&w, |s| {
            let ai = s.game.as_mut().unwrap().ai[1].as_mut().unwrap();
            ai.pending.push(crate::ai::AiCommand {
                due: u64::MAX,
                seq: 0,
                kind: CommandKind::CallIn { squad_type: 999 },
            });
        });
        assert!(load_err(&bytes).contains("AI"));
        // S3b re-call cooldowns: one per squad type, never further ahead
        // than the longest cooldown.
        let bytes = save_edited(&w, |s| {
            s.game.as_mut().unwrap().players[0].ready_at.pop();
        });
        assert!(load_err(&bytes).contains("cooldown"));
        let bytes = save_edited(&w, |s| {
            s.game.as_mut().unwrap().players[0].ready_at[0] = u64::MAX;
        });
        assert!(load_err(&bytes).contains("cooldown"));
        let longest = w
            .match_setup()
            .unwrap()
            .squad_types
            .iter()
            .map(|t| t.recall_cooldown_ticks.max(t.initial_cooldown_ticks) as u64)
            .max()
            .unwrap();
        let tick = w.tick();
        let ok = save_edited(&w, |s| {
            s.game.as_mut().unwrap().players[0].ready_at[0] = tick + longest;
        });
        assert!(World::load(&ok, 1).is_ok());
        let bytes = save_edited(&w, |s| {
            s.game.as_mut().unwrap().players[0].ready_at[0] = tick + longest + 1;
        });
        assert!(load_err(&bytes).contains("cooldown"));
        // Economy values past the content ranges never reach the sim.
        let bytes = save_edited(&w, |s| {
            s.game.as_mut().unwrap().setup.rules.point_income_per_min = u32::MAX;
        });
        assert!(load_err(&bytes).contains("economy"));
    }

    #[test]
    fn huge_income_saturates_instead_of_overflowing() {
        // The largest values the content allows: 1e6 per minute per point
        // value, points worth 100, AI income at 1000 %.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
        let defs = frontline_content::ModDefs::load(&root).unwrap();
        let req = frontline_content::MatchRequest::skirmish(
            1,
            "ww2:germany",
            "ww2:usa",
            frontline_content::Difficulty::Hard,
        );
        let mut setup = defs.resolve(&req).unwrap();
        setup.rules.point_income_per_min = 1_000_000;
        setup.rules.ticket_drain_per_min_per_point = 100_000;
        setup.rules.difficulty.hard.income_pct = 1000;
        for p in &mut setup.map.points {
            p.value = 100;
        }
        let mut w = World::new_match(setup, 1).unwrap();
        let g = w.state.game.as_mut().unwrap();
        for c in &mut g.points {
            c.owner = Some(PlayerId(1));
        }
        assert_eq!(g.income_per_min(PlayerId(1)), u32::MAX);
        for _ in 0..3 {
            w.step(&[]);
        }
        let g = w.game().unwrap();
        assert!(g.players()[1].manpower > 1_000_000);
        assert_eq!(g.players()[0].tickets, 0);
    }

    #[test]
    fn match_rules_and_ai_state_are_hashed() {
        let w = match_world();
        let base = w.full_state_hash();
        fn ai(s: &mut SimState) -> &mut crate::ai::AiState {
            s.game.as_mut().unwrap().ai[1].as_mut().unwrap()
        }
        let edits: [fn(&mut SimState); 35] = [
            |s| s.game.as_mut().unwrap().players[0].point_ticks += 1,
            // S3b: a re-call cooldown.
            |s| s.game.as_mut().unwrap().players[1].ready_at[2] += 1,
            |s| s.game.as_mut().unwrap().points[2].contested = true,
            |s| ai(s).next_seq += 1,
            |s| s.game.as_mut().unwrap().players[1].drain_acc += 1,
            // S3.
            |s| s.game.as_mut().unwrap().players[1].kills_by[4] += 1,
            |s| s.game.as_mut().unwrap().players[0].rockets += 1,
            |s| ai(s).enemy_vehicle_seen = true,
            |s| {
                ai(s).at_seen.insert(1, 0);
            },
            |s| ai(s).counters.escorts_started += 1,
            |s| ai(s).counters.escorts_reached += 1,
            |s| ai(s).counters.satchels_at_vehicles += 1,
            |s| ai(s).counters.repairs += 1,
            |s| ai(s).counters.retreats += 1,
            |s| ai(s).counters.faces += 1,
            // S3b.
            |s| ai(s).counters.suppress_orders += 1,
            |s| ai(s).counters.bounds += 1,
            |s| ai(s).counters.at_hunts += 1,
            |s| ai(s).counters.satchel_hunts += 1,
            |s| {
                ai(s).slow_since.insert(SimId(3), 0);
            },
            // S3c Part B: every counter and each sticky-hunt field.
            |s| ai(s).counters.rearms += 1,
            |s| ai(s).counters.hunts_kept_unseen += 1,
            |s| ai(s).counters.hunt_throws += 1,
            |s| ai(s).counters.hunt_ends[3] += 1,
            |s| ai(s).counters.second_bases += 1,
            |s| ai(s).counters.at_stuck_hunts += 1,
            |s| ai(s).counters.at_follows += 1,
            |s| ai(s).counters.recrews += 1,
            |s| ai(s).counters.crew_retreats += 1,
            |s| ai(s).counters.denials += 1,
            |s| {
                ai(s).hunts.insert(SquadId(1), hunt());
            },
            |s| {
                let mut h = hunt();
                h.vehicle = SimId(4);
                ai(s).hunts.insert(SquadId(1), h);
            },
            |s| {
                let mut h = hunt();
                h.last_pos = FxVec2::new(Fx::ONE, Fx::ZERO);
                ai(s).hunts.insert(SquadId(1), h);
            },
            |s| {
                let mut h = hunt();
                h.fast_since = Some(0);
                ai(s).hunts.insert(SquadId(1), h);
            },
            |s| {
                let mut h = hunt();
                h.unseen = true;
                ai(s).hunts.insert(SquadId(1), h);
            },
        ];
        // A fresh hunt at tick 0 (S3c); the edits above change one field.
        fn hunt() -> crate::ai::AiHunt {
            crate::ai::AiHunt {
                vehicle: SimId(3),
                since: 0,
                last_seen: 0,
                last_pos: FxVec2::ZERO,
                fast_since: None,
                unseen: false,
            }
        }
        // Each hunt edit differs from the plain hunt too.
        let with_hunt = World::load(
            &save_edited(&w, |s| {
                ai(s).hunts.insert(SquadId(1), hunt());
            }),
            1,
        )
        .unwrap()
        .full_state_hash();
        for edit in &edits[31..] {
            let w2 = World::load(&save_edited(&w, *edit), 1).unwrap();
            assert_ne!(w2.full_state_hash(), with_hunt);
        }
        for edit in edits {
            let w2 = World::load(&save_edited(&w, edit), 1).unwrap();
            assert_ne!(w2.full_state_hash(), base);
        }
        // Escorts: keyed by a vehicle squad id that is used up and gone (an
        // escort whose vehicle was lost, dropped at the next plan).
        fn gone(s: &mut SimState) -> SquadId {
            let g = s.game.as_mut().unwrap();
            g.next_squad += 1;
            SquadId(g.next_squad - 1)
        }
        let gone_base = World::load(
            &save_edited(&w, |s| {
                gone(s);
            }),
            1,
        )
        .unwrap()
        .full_state_hash();
        let w2 = World::load(
            &save_edited(&w, |s| {
                let id = gone(s);
                ai(s).escorts.insert(
                    id,
                    crate::ai::AiEscort {
                        point: 1,
                        squads: vec![SquadId(1)],
                        started: vec![SquadId(1)],
                        since: 0,
                        reached: false,
                    },
                );
            }),
            1,
        )
        .unwrap();
        assert_ne!(w2.full_state_hash(), gone_base);
        // Loads reject broken S3 AI state.
        fn escort_at(s: &mut SimState, id: SquadId) {
            ai(s).escorts.insert(
                id,
                crate::ai::AiEscort {
                    point: 0,
                    squads: Vec::new(),
                    started: Vec::new(),
                    since: 0,
                    reached: false,
                },
            );
        }
        fn squad_of(s: &SimState, p: u8) -> SquadId {
            let g = s.game.as_ref().unwrap();
            *g.squads
                .iter()
                .find(|(_, q)| q.owner == PlayerId(p))
                .unwrap()
                .0
        }
        let bad: [fn(&mut SimState); 11] = [
            // S3c: sticky hunts with ticks in the future, out of order, or
            // keyed by a squad id never used.
            |s| {
                let mut h = hunt();
                h.since = u64::MAX;
                h.last_seen = u64::MAX;
                ai(s).hunts.insert(SquadId(1), h);
            },
            |s| {
                let mut h = hunt();
                h.since = 1;
                ai(s).hunts.insert(SquadId(1), h);
            },
            |s| {
                let mut h = hunt();
                h.fast_since = Some(u64::MAX);
                ai(s).hunts.insert(SquadId(1), h);
            },
            |s| {
                ai(s).hunts.insert(SquadId(9999), hunt());
            },
            // S3b: a slow-since tick in the future.
            |s| {
                ai(s).slow_since.insert(SimId(3), u64::MAX);
            },
            // Review fix: an escort keyed by an infantry squad (its own or
            // the enemy's) used to pass the load check and then panic the
            // first AI plan.
            |s| {
                let id = squad_of(s, 1);
                escort_at(s, id);
            },
            |s| {
                let id = squad_of(s, 0);
                escort_at(s, id);
            },
            |s| {
                ai(s).at_seen.insert(99, 0);
            },
            |s| {
                ai(s).at_seen.insert(0, u64::MAX);
            },
            |s| {
                ai(s).escorts.insert(
                    SquadId(9999),
                    crate::ai::AiEscort {
                        point: 0,
                        squads: Vec::new(),
                        started: Vec::new(),
                        since: 0,
                        reached: false,
                    },
                );
            },
            |s| {
                ai(s).escorts.insert(
                    SquadId(0),
                    crate::ai::AiEscort {
                        point: 0,
                        squads: vec![SquadId(2), SquadId(1)],
                        started: Vec::new(),
                        since: 0,
                        reached: false,
                    },
                );
            },
        ];
        for edit in bad {
            assert!(World::load(&save_edited(&w, edit), 1).is_err());
        }
    }

    /// `match_world` with player 0's first two soldiers sent off on their own
    /// (one still waiting for his path) and the third ordered prone.
    fn unit_world() -> World {
        let mut w = match_world();
        let men: Vec<SimId> = w
            .entities()
            .soldiers()
            .filter(|id| w.entities().owner()[id.index()] == PlayerId(0))
            .take(3)
            .collect();
        w.step(&[
            Command {
                player: PlayerId(0),
                seq: 0,
                kind: CommandKind::MoveUnits {
                    units: men[..2].to_vec(),
                    target: FxVec2::from_int(300, 300),
                },
            },
            Command {
                player: PlayerId(0),
                seq: 1,
                kind: CommandKind::SetStance {
                    units: vec![men[2]],
                    stance: crate::unit::StanceOrder::Prone,
                },
            },
        ]);
        w
    }

    #[test]
    fn unit_state_is_saved_hashed_and_checked_on_load() {
        use crate::game::PathReq;
        use crate::unit::{StanceOrder, UnitOrder};
        let w = unit_world();
        let loaded = World::load(&w.save(), 2).unwrap();
        assert_eq!(loaded.state(), w.state());
        assert_eq!(loaded.state_hash(), w.state_hash());
        let ents = w.entities();
        let men: Vec<SimId> = ents
            .soldiers()
            .filter(|id| ents.unit[id.index()].detached)
            .collect();
        assert_eq!(men.len(), 2);
        let (a, b) = (men[0].index(), men[1].index());
        let base = w.full_state_hash();
        type Edit = Box<dyn Fn(&mut SimState)>;
        let edits: [Edit; 4] = [
            Box::new(move |s| s.entities.unit[a].stance = StanceOrder::Crouch),
            Box::new(move |s| s.entities.unit[a].nav.goal.x += Fx::ONE),
            Box::new(move |s| {
                s.entities.unit[b].order = UnitOrder::AttackMove {
                    target: FxVec2::from_int(300, 300),
                }
            }),
            Box::new(|s| s.game.as_mut().unwrap().next_unit_dig += 1),
        ];
        for edit in edits {
            let w2 = World::load(&save_edited(&w, edit), 1).unwrap();
            assert_ne!(w2.full_state_hash(), base);
        }
        // A man in formation with an order of his own.
        let bytes = save_edited(&w, |s| {
            let k = s
                .entities
                .soldiers()
                .find(|id| !s.entities.unit[id.index()].detached)
                .unwrap();
            s.entities.unit[k.index()].order = UnitOrder::Retreat;
        });
        assert!(load_err(&bytes).contains("unit order"));
        // A digger of a job that does not exist.
        let bytes = save_edited(&w, move |s| {
            s.entities.unit[a].order = UnitOrder::Dig { job: 7 }
        });
        assert!(load_err(&bytes).contains("unit order"));
        // A walking man whose path is used up.
        let bytes = save_edited(&w, move |s| {
            let nav = &mut s.entities.unit[b].nav;
            nav.phase = NavPhase::Moving;
            nav.path.clear();
        });
        assert!(load_err(&bytes).contains("unit order"));
        // A path request for a soldier outside the store, or queued twice.
        let bytes = save_edited(&w, |s| {
            let q = &mut s.game.as_mut().unwrap().path_queue.queues[0];
            q.push(PathReq::Unit(SimId(1 << 20)));
        });
        assert!(load_err(&bytes).contains("unknown soldier"));
        let bytes = save_edited(&w, move |s| {
            let q = &mut s.game.as_mut().unwrap().path_queue.queues[0];
            q.push(PathReq::Unit(SimId(a as u32)));
            q.push(PathReq::Unit(SimId(a as u32)));
        });
        assert!(load_err(&bytes).contains("twice"));
    }

    /// S3b: a squad and a detached man on `SuppressUnits`; the squad
    /// order, the man's order and the duck flag are saved, hashed and
    /// checked on load.
    #[test]
    fn suppress_orders_and_the_duck_state_are_saved_hashed_and_checked() {
        use crate::game::Order;
        use crate::unit::UnitOrder;
        let mut w = match_world();
        let (whole, lone) = {
            let g = w.game().unwrap();
            let mut own = g.squads().iter().filter(|(_, q)| q.owner == PlayerId(0));
            let a = own.next().unwrap();
            let b = own.next().unwrap();
            (a.1.members.clone(), b.1.members[0])
        };
        let line = vec![FxVec2::from_int(500, 520), FxVec2::from_int(500, 560)];
        let mut units = whole.clone();
        units.push(lone);
        w.step(&[Command {
            player: PlayerId(0),
            seq: 0,
            kind: CommandKind::SuppressUnits {
                units,
                line: line.clone(),
            },
        }]);
        let g = w.game().unwrap();
        let sid = w.entities().squad()[whole[0].index()].unwrap();
        let pulled = FxVec2::from_int(500, 560);
        assert!(matches!(g.squads[&sid].order, Order::Suppress { line: l } if l[1] == pulled));
        assert!(matches!(
            w.entities().unit[lone.index()].order,
            UnitOrder::Suppress { .. }
        ));
        let loaded = World::load(&w.save(), 2).unwrap();
        assert_eq!(loaded.state(), w.state());
        assert_eq!(loaded.full_state_hash(), w.full_state_hash());
        let base = w.full_state_hash();
        let (m, l) = (whole[0].index(), lone.index());
        type Edit = Box<dyn Fn(&mut SimState)>;
        let edits: [Edit; 2] = [
            Box::new(move |s| {
                let g = s.game.as_mut().unwrap();
                if let Order::Suppress { line } = &mut g.squads.get_mut(&sid).unwrap().order {
                    line[0].y += Fx::ONE;
                }
            }),
            Box::new(move |s| {
                if let UnitOrder::Suppress { line } = &mut s.entities.unit[l].order {
                    line[1].x += Fx::ONE;
                }
            }),
        ];
        for edit in edits {
            let w2 = World::load(&save_edited(&w, edit), 1).unwrap();
            assert_ne!(w2.full_state_hash(), base);
        }
        // The duck flag: a man in a trench, ducked or not.
        let trench = World::load(
            &save_edited(&w, move |s| s.entities.combat[m].in_trench = true),
            1,
        )
        .unwrap();
        let ducked = World::load(
            &save_edited(&w, move |s| {
                s.entities.combat[m].in_trench = true;
                s.entities.combat[m].ducked = true;
            }),
            1,
        )
        .unwrap();
        assert_ne!(trench.full_state_hash(), ducked.full_state_hash());
        // Broken state is refused: ducked out of a trench, ducked and
        // pinned, a line off the map or too long.
        let bad: [Edit; 4] = [
            Box::new(move |s| s.entities.combat[m].ducked = true),
            Box::new(move |s| {
                let c = &mut s.entities.combat[m];
                (c.in_trench, c.ducked, c.pinned) = (true, true, true);
            }),
            Box::new(move |s| {
                if let UnitOrder::Suppress { line } = &mut s.entities.unit[l].order {
                    line[1].x = Fx::from_int(-5);
                }
            }),
            Box::new(move |s| {
                let g = s.game.as_mut().unwrap();
                if let Order::Suppress { line } = &mut g.squads.get_mut(&sid).unwrap().order {
                    line[1].y += Fx::from_int(20);
                }
            }),
        ];
        for edit in bad {
            assert!(World::load(&save_edited(&w, edit), 1).is_err());
        }
    }

    #[test]
    fn load_rejects_unit_state_in_bench_worlds() {
        let w = busy_world();
        let bytes = save_edited(&w, |s| {
            s.entities.unit[0].stance = crate::unit::StanceOrder::Prone;
        });
        assert!(load_err(&bytes).contains("bench"));
    }

    /// A match world with a smoke cloud up, a frag in the air and a man
    /// winding up a throw (player 0's men in their base).
    fn item_world() -> World {
        use frontline_content::ItemKind;
        let mut w = match_world();
        let men: Vec<SimId> = {
            let g = w.game().unwrap();
            let sq = g
                .squads()
                .values()
                .find(|s| s.owner == PlayerId(0))
                .unwrap();
            sq.members.clone()
        };
        let throw = |w: &mut World, m: SimId, item: ItemKind| {
            let at = w.entities().transform()[m.index()].pos.xy() + FxVec2::from_int(15, 0);
            let seq = w.tick() as u32;
            w.step(&[Command {
                player: PlayerId(0),
                seq,
                kind: CommandKind::Throw {
                    units: vec![m],
                    item,
                    target: at,
                },
            }]);
        };
        throw(&mut w, men[0], ItemKind::Smoke);
        for _ in 0..70 {
            w.step(&[]);
        }
        // S4: men 4 and 5 of a Grenadiergruppe are riflemen with a frag
        // (men 1-3 are the MG team, who carry none).
        throw(&mut w, men[4], ItemKind::Frag);
        for _ in 0..40 {
            w.step(&[]);
        }
        throw(&mut w, men[5], ItemKind::Frag);
        for _ in 0..5 {
            w.step(&[]);
        }
        let g = w.game().unwrap();
        assert_eq!(g.smoke().len(), 1);
        assert!(g.thrown().len() == 1 && !g.thrown()[0].landed);
        assert!(w.entities().kit[men[5].index()].windup.is_some());
        w
    }

    #[test]
    fn item_state_is_saved_hashed_and_checked_on_load() {
        use frontline_content::ItemKind;
        let w = item_world();
        let loaded = World::load(&w.save(), 2).unwrap();
        assert_eq!(loaded.state(), w.state());
        assert_eq!(loaded.state_hash(), w.state_hash());
        let ents = w.entities();
        let winder = ents
            .soldiers()
            .find(|id| ents.kit[id.index()].windup.is_some())
            .unwrap()
            .index();
        let leader = ents
            .soldiers()
            .find(|id| ents.kit[id.index()].slot == 0 && ents.squad[id.index()].is_some())
            .unwrap()
            .index();
        let base = w.full_state_hash();
        type Edit = Box<dyn Fn(&mut SimState)>;
        let edits: [Edit; 9] = [
            // Off the leader kit (S4: into the riflemen's slot), with no
            // more than a rifleman carries.
            Box::new(move |s| {
                s.entities.kit[leader].slot = 1;
                s.entities.kit[leader].items = Default::default();
            }),
            // S4: the weapon he carries (S5a: with no more rounds in it
            // than its magazine holds).
            Box::new(move |s| {
                s.entities.kit[leader].weapon ^= 1;
                s.entities.combat[leader].mag = 1;
            }),
            Box::new(move |s| s.entities.kit[winder].resupply = 3),
            Box::new(move |s| s.entities.kit[winder].windup.as_mut().unwrap().ticks -= 1),
            Box::new(move |s| s.entities.kit[winder].throws += 1),
            Box::new(|s| s.game.as_mut().unwrap().thrown[0].pos.z += Fx::ONE),
            Box::new(|s| s.game.as_mut().unwrap().smoke[0].age += 1),
            Box::new(|s| s.game.as_mut().unwrap().next_thrown += 1),
            Box::new(|s| s.stats.throws += 1),
        ];
        for edit in edits {
            let w2 = World::load(&save_edited(&w, edit), 1).unwrap();
            assert_ne!(w2.full_state_hash(), base);
        }
        // More items than the loadout, or a wind-up for an item he lacks.
        let bytes = save_edited(&w, move |s| s.entities.kit[winder].items[0] = 5);
        assert!(load_err(&bytes).contains("kit"));
        // A second leader kit in one squad (more items for everyone).
        let bytes = save_edited(&w, |s| {
            for k in s.entities.kit.iter_mut() {
                k.slot = 0;
            }
        });
        // (S4: a rifleman with a Panzerfaust in the leader's slot is
        // caught first, by his items.)
        let e = load_err(&bytes);
        assert!(e.contains("leader") || e.contains("kit"), "{e}");
        // S4: a weapon none of his squad's members carries, or a slot past
        // the squad type's.
        let bytes = save_edited(&w, move |s| s.entities.kit[leader].weapon = 9999);
        assert!(load_err(&bytes).contains("weapon"));
        let bytes = save_edited(&w, move |s| s.entities.kit[leader].slot = 99);
        let e = load_err(&bytes);
        assert!(e.contains("slot") || e.contains("kit"), "{e}");
        // Exhausted id counters would overflow on the next throw, cloud,
        // unit dig or squad.
        type Counter = fn(&mut crate::game::MatchState) -> &mut u32;
        let counters: [Counter; 4] = [
            |g| &mut g.next_thrown,
            |g| &mut g.next_smoke,
            |g| &mut g.next_unit_dig,
            |g| &mut g.next_squad,
        ];
        for c in counters {
            let bytes = save_edited(&w, move |s| *c(s.game.as_mut().unwrap()) = u32::MAX);
            assert!(load_err(&bytes).contains("counter"));
        }
        // An AI throw stamped in the future (overflow, or never again).
        for t in [u64::MAX, 1 << 40] {
            let bytes = save_edited(&w, move |s| {
                let g = s.game.as_mut().unwrap();
                let sq = *g.squads.keys().next().unwrap();
                g.ai[1].as_mut().unwrap().last_throw.insert(sq, t);
            });
            assert!(load_err(&bytes).contains("AI"));
        }
        let bytes = save_edited(&w, move |s| {
            s.entities.kit[winder].windup.as_mut().unwrap().kind = ItemKind::Satchel
        });
        assert!(load_err(&bytes).contains("kit"));
        // A corpse with items.
        let bytes = save_edited(&w, |s| {
            let i = s.entities.soldiers().next().unwrap().index();
            s.entities.kind[i] = EntityKind::Corpse;
            s.entities.combat[i].corpse = 10;
            s.entities.combat[i].hp = 0;
            let sq = s.entities.squad[i].take().unwrap();
            let g = s.game.as_mut().unwrap();
            let q = g.squads.get_mut(&sq).unwrap();
            let k = q.members.iter().position(|m| m.index() == i).unwrap();
            q.members.remove(k);
            q.nav.slots.remove(k);
            s.entities.unit[i] = Default::default();
        });
        let e = load_err(&bytes);
        assert!(e.contains("items"), "{e}");
        // Items in flight: unknown item, ids out of order.
        let bytes = save_edited(&w, |s| s.game.as_mut().unwrap().thrown[0].item = 99);
        assert!(load_err(&bytes).contains("thrown"));
        let bytes = save_edited(&w, |s| {
            let g = s.game.as_mut().unwrap();
            let t = g.thrown[0].clone();
            g.thrown.push(t);
        });
        assert!(load_err(&bytes).contains("thrown"));
        // Smoke from a frag, or a cloud past its life.
        let frag = w
            .match_setup()
            .unwrap()
            .items
            .iter()
            .position(|it| it.kind == ItemKind::Frag)
            .unwrap() as u16;
        let bytes = save_edited(&w, move |s| s.game.as_mut().unwrap().smoke[0].item = frag);
        assert!(load_err(&bytes).contains("smoke"));
        let bytes = save_edited(&w, |s| s.game.as_mut().unwrap().smoke[0].age = 1_000_000);
        assert!(load_err(&bytes).contains("smoke"));
    }

    #[test]
    fn load_rejects_item_state_in_bench_worlds() {
        let w = busy_world();
        let bytes = save_edited(&w, |s| s.entities.kit[0].items[0] = 1);
        assert!(load_err(&bytes).contains("bench"));
    }

    #[test]
    fn load_rejects_combat_state_in_bench_worlds() {
        let w = busy_world();
        let bytes = save_edited(&w, |s| s.entities.combat[0].hp = 50);
        assert!(load_err(&bytes).contains("bench"));
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

    // ------------------------------------------------------- S3 vehicles --

    /// Kessel Ridge with the test unlock override and `wreck_cap` 1: a
    /// German tank driving, a US tank holding, a wreck, a queued vehicle
    /// path request and an AT-ditch line.
    fn vehicle_world() -> (World, SimId, SimId) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
        let mut defs = frontline_content::ModDefs::load(&root).unwrap();
        let rules = defs
            .add_rules_override(&root.join("tests/rules_test_unlock.toml"))
            .unwrap();
        let mut req = frontline_content::MatchRequest::skirmish(
            1,
            "ww2:germany",
            "ww2:usa",
            frontline_content::Difficulty::Normal,
        );
        req.rules = rules;
        let mut setup = defs.resolve(&req).unwrap();
        setup.players.iter_mut().for_each(|p| p.is_ai = false);
        setup.rules.vehicles.wreck_cap = 1;
        let mut w = World::new_match(setup, 1).unwrap();
        let ty = |w: &World, id: &str| w.match_setup().unwrap().squad_type_index(id).unwrap();
        let (ger, usa) = (
            ty(&w, "ww2:ger_pz4_h_squad"),
            ty(&w, "ww2:usa_m4a3_75_squad"),
        );
        let east = FxVec2::new(Fx::ONE, Fx::ZERO);
        let a = w
            .spawn_squad_at(PlayerId(0), ger, FxVec2::from_int(450, 500), east)
            .unwrap();
        let b = w
            .spawn_squad_at(PlayerId(1), usa, FxVec2::from_int(600, 520), east)
            .unwrap();
        let c = w
            .spawn_squad_at(PlayerId(1), usa, FxVec2::from_int(620, 560), east)
            .unwrap();
        let g = w.game().unwrap();
        let (va, vc) = (g.squads[&a].members[0], g.squads[&c].members[0]);
        let vb = g.squads[&b].members[0];
        w.wreck_vehicle(vc);
        w.dig_at_ditch_now(&[FxVec2::from_int(520, 430), FxVec2::from_int(520, 460)], 1);
        let cmd = |seq, kind| Command {
            player: PlayerId(0),
            seq,
            kind,
        };
        w.step(&[cmd(
            0,
            CommandKind::Move {
                squad: a,
                target: FxVec2::from_int(500, 470),
            },
        )]);
        for _ in 0..40 {
            w.step(&[]);
        }
        // Queue a request without serving it (setup-tool edit).
        let g = w.state.game.as_mut().unwrap();
        g.squads.get_mut(&a).unwrap().nav.phase = NavPhase::PathPending;
        g.squads.get_mut(&a).unwrap().nav.path.clear();
        g.squads.get_mut(&a).unwrap().nav.next = 0;
        g.path_queue.queues[0].push(crate::game::PathReq::Vehicle {
            squad: a,
            avoid: Some(7),
        });
        w.refresh_hashes();
        (w, va, vb)
    }

    #[test]
    fn vehicle_state_is_saved_hashed_and_checked_on_load() {
        let (w, va, vb) = vehicle_world();
        let loaded = World::load(&w.save(), 2).unwrap();
        assert_eq!(loaded.state(), w.state());
        assert_eq!(loaded.state_hash(), w.state_hash());
        assert_eq!(loaded.full_state_hash(), w.full_state_hash());
        let vs = &w.game().unwrap().vehicles[&va];
        assert!(vs.speed != Fx::ZERO, "driving");
        // Every field of `VehicleState` changes the hash. The destructure
        // fails to compile when a field is added and not listed here.
        let VehicleState {
            type_idx: _,
            pitch: _,
            roll: _,
            turret_yaw: _,
            gun_pitch: _,
            speed: _,
            avg_vel: _,
            reverse: _,
            seats: _,
            components: _,
            ammo: _,
            loaded: _,
            ammo_choice: _,
            reload: _,
            target: _,
            attack: _,
            aim_turret: _,
            mgs: _,
            ranging: _,
            since_fire: _,
            burn: _,
            ammo_fire: _,
            repairing: _,
            repair_progress: _,
            base_ticks: _,
            last_attacker: _,
            bog: _,
            on_blocked: _,
            crossing: _,
            crossings: _,
            clear_ticks: _,
            progress_at: _,
            progress_ticks: _,
            repath_tick: _,
            followers: _,
            wreck_tick: _,
            face: _,
            suppress: _,
            abandoned: _,
            abandon_cause: _,
            exit: _,
            crew_squad: _,
            last_hit_dir: _,
            last_pen: _,
            prev_pen: _,
            board: _,
            bail_order: _,
            supply_due: _,
        } = vs.clone();
        type Edit = Box<dyn Fn(&mut VehicleState)>;
        let edits: Vec<(&str, Edit)> = vec![
            ("type_idx", Box::new(|v| v.type_idx ^= 1)),
            ("pitch", Box::new(|v| v.pitch += Fx::EPSILON)),
            ("roll", Box::new(|v| v.roll += Fx::EPSILON)),
            ("turret_yaw", Box::new(|v| v.turret_yaw += Fx::EPSILON)),
            ("gun_pitch", Box::new(|v| v.gun_pitch += Fx::EPSILON)),
            ("speed", Box::new(|v| v.speed += Fx::EPSILON)),
            ("avg_vel", Box::new(|v| v.avg_vel.y += Fx::EPSILON)),
            ("reverse", Box::new(|v| v.reverse ^= true)),
            ("seats.hp", Box::new(|v| v.seats[2].hp -= 1)),
            ("seats.swap", Box::new(|v| v.seats[1].swap = 3)),
            ("seats.swap_from", Box::new(|v| v.seats[1].swap_from = 4)),
            // S3c.
            (
                "seats.untrained",
                Box::new(|v| v.seats[1].untrained ^= true),
            ),
            ("abandoned", Box::new(|v| v.abandoned = Some(1))),
            ("abandon_cause", Box::new(|v| v.abandon_cause = 3)),
            ("exit", Box::new(|v| v.exit[0] = 5)),
            ("crew_squad", Box::new(|v| v.crew_squad = Some(SquadId(2)))),
            (
                "last_hit_dir",
                Box::new(|v| v.last_hit_dir.x += Fx::EPSILON),
            ),
            ("last_pen", Box::new(|v| v.last_pen = Some(1))),
            ("prev_pen", Box::new(|v| v.prev_pen = Some(1))),
            ("board", Box::new(|v| v.board = Some((SquadId(2), 1)))),
            ("bail_order", Box::new(|v| v.bail_order ^= true)),
            ("components", Box::new(|v| v.components[3] -= 1)),
            ("ammo", Box::new(|v| v.ammo[1] -= 1)),
            ("loaded", Box::new(|v| v.loaded = Some(1))),
            (
                "ammo_choice",
                Box::new(|v| v.ammo_choice = crate::vehicle::AmmoChoice::He),
            ),
            ("reload", Box::new(|v| v.reload = 9)),
            ("target", Box::new(move |v| v.target = Some(vb))),
            ("attack", Box::new(move |v| v.attack = Some(vb))),
            ("aim_turret", Box::new(|v| v.aim_turret ^= true)),
            ("mgs.target", Box::new(move |v| v.mgs[0].target = Some(vb))),
            ("mgs.cooldown", Box::new(|v| v.mgs[1].cooldown = 4)),
            ("mgs.shots", Box::new(|v| v.mgs[0].shots += 1)),
            ("mgs.belt", Box::new(|v| v.mgs[1].belt += 1)),
            ("mgs.spent", Box::new(|v| v.mgs[0].spent += 1)),
            ("repair_progress", Box::new(|v| v.repair_progress += 1)),
            ("base_ticks", Box::new(|v| v.base_ticks += 1)),
            ("supply_due", Box::new(|v| v.supply_due = 1)),
            ("last_attacker", Box::new(|v| v.last_attacker = Some(1))),
            ("ranging", Box::new(|v| v.ranging = 2)),
            ("since_fire", Box::new(|v| v.since_fire = 5)),
            ("burn", Box::new(|v| v.burn = 1)),
            ("ammo_fire", Box::new(|v| v.ammo_fire = 1)),
            ("repairing", Box::new(|v| v.repairing = Some(1))),
            ("bog", Box::new(|v| v.bog = 1)),
            ("on_blocked", Box::new(|v| v.on_blocked ^= true)),
            ("crossing", Box::new(|v| v.crossing ^= true)),
            ("crossings", Box::new(|v| v.crossings += 1)),
            ("clear_ticks", Box::new(|v| v.clear_ticks = 7)),
            ("progress_at", Box::new(|v| v.progress_at.x += Fx::EPSILON)),
            ("progress_ticks", Box::new(|v| v.progress_ticks += 1)),
            ("repath_tick", Box::new(|v| v.repath_tick = Some(1))),
            ("followers", Box::new(|v| v.followers.push(SimId(1)))),
            ("wreck_tick", Box::new(|v| v.wreck_tick = Some(1))),
            ("face", Box::new(|v| v.face = Some(FxVec2::from_int(5, 6)))),
            (
                "suppress",
                Box::new(|v| v.suppress = Some([FxVec2::from_int(5, 6), FxVec2::from_int(9, 6)])),
            ),
        ];
        let base = w.full_state_hash();
        for (name, edit) in &edits {
            let mut st = w.state.clone();
            edit(st.game.as_mut().unwrap().vehicles.get_mut(&va).unwrap());
            let w2 = World::from_state(st, 1).unwrap();
            assert_ne!(w2.full_state_hash(), base, "{name} is not hashed");
            assert_eq!(w2.full_state_hash(), w2.state_hash());
        }
        // The other S3 match state is hashed too.
        type StateEdit = Box<dyn Fn(&mut SimState)>;
        let more: Vec<StateEdit> = vec![
            Box::new(|s| s.game.as_mut().unwrap().at_ditches[0][1].y += Fx::ONE),
            Box::new(|s| s.game.as_mut().unwrap().armour_seq += 1),
            Box::new(|s| s.stats.armour_box_tests += 1),
            Box::new(|s| s.stats.armour_box_peak += 1),
            Box::new(|s| s.stats.rounds_stopped += 1),
            Box::new(|s| s.stats.ricochets += 1),
            Box::new(|s| s.stats.non_penetrations += 1),
            Box::new(|s| s.stats.penetrations += 1),
            Box::new(|s| s.stats.knockouts += 1),
            Box::new(|s| s.stats.blast_track_hits += 1),
            Box::new(|s| s.stats.rockets_fired += 1),
            Box::new(|s| s.stats.frag_rays += 1),
            Box::new(|s| s.stats.frag_hits += 1),
            Box::new(|s| s.stats.frag_rays_capped += 1),
            Box::new(|s| {
                s.game.as_mut().unwrap().path_queue.queues[0][0] = crate::game::PathReq::Vehicle {
                    squad: SquadId(3),
                    avoid: Some(8),
                }
            }),
        ];
        for edit in more {
            let mut st = w.state.clone();
            edit(&mut st);
            assert_ne!(World::from_state(st, 1).unwrap().full_state_hash(), base);
        }
        // S3c: every per-player crew counter is hashed.
        type CrewEdit = fn(&mut crate::game::CrewCounters);
        let crew: [CrewEdit; 7] = [
            |c| c.abandoned += 1,
            |c| c.recrewed += 1,
            |c| c.captured += 1,
            |c| c.bailed += 1,
            |c| c.out += 1,
            |c| c.home += 1,
            |c| c.scrapped += 1,
        ];
        for edit in crew {
            let mut st = w.state.clone();
            edit(&mut st.game.as_mut().unwrap().players[1].crew);
            assert_ne!(World::from_state(st, 1).unwrap().full_state_hash(), base);
        }

        // Loads reject broken vehicle state.
        fn vehicle(s: &mut SimState, id: SimId) -> &mut VehicleState {
            s.game.as_mut().unwrap().vehicles.get_mut(&id).unwrap()
        }
        let bad: Vec<(StateEdit, &str)> = vec![
            // S3c: bail state on a live vehicle, or out of range.
            (
                Box::new(move |s| vehicle(s, va).abandoned = Some(1)),
                "out of range",
            ),
            (
                Box::new(move |s| vehicle(s, va).exit[0] = 3),
                "out of range",
            ),
            (
                Box::new(move |s| vehicle(s, va).crew_squad = Some(SquadId(0))),
                "out of range",
            ),
            (
                Box::new(move |s| vehicle(s, va).board = Some((SquadId(0), 1))),
                "out of range",
            ),
            (
                Box::new(move |s| vehicle(s, va).abandon_cause = 9),
                "out of range",
            ),
            (
                Box::new(move |s| vehicle(s, va).prev_pen = Some(5)),
                "out of range",
            ),
            (
                Box::new(move |s| {
                    let v = vehicle(s, va);
                    v.last_hit_dir = FxVec2::from_int(9, 0);
                }),
                "out of range",
            ),
            // An abandoned vehicle state on a live vehicle entity (it keeps
            // its squad).
            (
                Box::new(move |s| {
                    let v = vehicle(s, va);
                    v.abandoned = Some(0);
                    v.abandon_cause = 6;
                    v.target = None;
                    v.attack = None;
                    v.followers.clear();
                    v.face = None;
                    v.suppress = None;
                    v.repairing = None;
                    v.repair_progress = 0;
                }),
                "squad",
            ),
            (Box::new(move |s| vehicle(s, va).type_idx = 9), "vehicle"),
            (
                Box::new(move |s| vehicle(s, va).seats.pop().map(|_| ()).unwrap()),
                "out of range",
            ),
            (
                Box::new(move |s| vehicle(s, va).components[0] = 999),
                "out of range",
            ),
            (
                Box::new(move |s| vehicle(s, va).ammo[0] = 999),
                "out of range",
            ),
            (
                Box::new(move |s| vehicle(s, va).pitch = Fx::ONE),
                "out of range",
            ),
            (
                Box::new(move |s| vehicle(s, va).loaded = Some(5)),
                "out of range",
            ),
            (
                Box::new(move |s| vehicle(s, va).wreck_tick = Some(9_999_999)),
                "out of range",
            ),
            (
                Box::new(move |s| {
                    // An own soldier.
                    let own = s.entities.owner[va.index()];
                    let man = s
                        .entities
                        .soldiers()
                        .find(|m| s.entities.owner[m.index()] == own)
                        .unwrap();
                    vehicle(s, va).target = Some(man)
                }),
                "wrong entity",
            ),
            (
                Box::new(move |s| {
                    let enemy = s.entities.owner[vb.index()];
                    let man = s
                        .entities
                        .soldiers()
                        .find(|m| s.entities.owner[m.index()] == enemy)
                        .unwrap();
                    vehicle(s, va).attack = Some(man)
                }),
                "wrong entity",
            ),
            (
                Box::new(move |s| vehicle(s, va).mgs[0].target = Some(vb)),
                "wrong entity",
            ),
            (
                Box::new(move |s| {
                    vehicle(s, va).mgs.pop();
                }),
                "out of range",
            ),
            (
                Box::new(move |s| vehicle(s, va).mgs[0].belt = 60_000),
                "out of range",
            ),
            // S3b Fix: more MG rounds spent than the mount stows.
            (
                Box::new(move |s| vehicle(s, va).mgs[0].spent = 60_000),
                "out of range",
            ),
            // S3b Fix: a suppress line longer than the order allows.
            (
                Box::new(move |s| {
                    vehicle(s, va).suppress =
                        Some([FxVec2::from_int(100, 100), FxVec2::from_int(300, 100)])
                }),
                "suppress line",
            ),
            // S3b Fix: a wreck still burning (knock-out puts fires out).
            (
                Box::new(move |s| {
                    let g = s.game.as_mut().unwrap();
                    let wreck = g
                        .vehicles
                        .values_mut()
                        .find(|v| v.wreck_tick.is_some())
                        .unwrap();
                    wreck.ammo_fire = 1;
                }),
                "out of range",
            ),
            (
                Box::new(move |s| {
                    let g = s.game.as_mut().unwrap();
                    let wreck = g
                        .vehicles
                        .values_mut()
                        .find(|v| v.wreck_tick.is_some())
                        .unwrap();
                    wreck.burn = 1;
                }),
                "out of range",
            ),
            (
                Box::new(move |s| {
                    let v = vehicle(s, va);
                    v.repairing = Some(0);
                    v.repair_progress = 1_000_000;
                }),
                "out of range",
            ),
            (
                Box::new(move |s| vehicle(s, va).last_attacker = Some(7)),
                "out of range",
            ),
            (
                Box::new(move |s| {
                    let v = vehicle(s, va);
                    v.loaded = Some(0);
                    // S5a: 42 PzGr 39 stowed.
                    v.ammo[0] = 42;
                }),
                "out of range",
            ),
            (
                Box::new(move |s| vehicle(s, va).followers = vec![vb]),
                "wrong entity",
            ),
            (
                Box::new(move |s| vehicle(s, va).face = Some(FxVec2::from_int(-5, 6))),
                "out of range",
            ),
            (
                Box::new(move |s| {
                    s.game.as_mut().unwrap().vehicles.remove(&va);
                }),
                "vehicle",
            ),
            (
                Box::new(move |s| s.entities.combat[va.index()].hp = 5),
                "combat",
            ),
            (
                Box::new(|s| s.game.as_mut().unwrap().at_ditches[0].truncate(1)),
                "AT-ditch",
            ),
            (
                Box::new(|s| {
                    s.game.as_mut().unwrap().path_queue.queues[0][0] =
                        crate::game::PathReq::Vehicle {
                            squad: SquadId(0),
                            avoid: None,
                        }
                }),
                "path request",
            ),
            (
                Box::new(|s| {
                    s.game.as_mut().unwrap().path_queue.queues[0][0] =
                        crate::game::PathReq::Vehicle {
                            squad: SquadId(3),
                            avoid: Some(u32::MAX),
                        }
                }),
                "path request",
            ),
            // A second wreck over the cap of 1: turn the US tank into one.
            (
                Box::new(move |s| {
                    let sq = s.entities.squad[vb.index()].take().unwrap();
                    s.entities.kind[vb.index()] = EntityKind::Wreck;
                    let g = s.game.as_mut().unwrap();
                    g.squads.remove(&sq);
                    g.vehicles.get_mut(&vb).unwrap().wreck_tick = Some(1);
                }),
                "cap",
            ),
        ];
        for (edit, want) in bad {
            let bytes = save_edited(&w, edit);
            assert!(World::load(&bytes, 1).is_err(), "accepted: {want}");
            let e = load_err(&bytes);
            assert!(e.contains(want), "{want}: {e}");
        }
    }

    #[test]
    fn the_wreck_cap_removes_the_oldest_and_frees_its_id() {
        let (mut w, va, vb) = vehicle_world();
        // `vehicle_world` already made one wreck; a second goes over the
        // cap of 1, so the first is removed and its id freed.
        let first = *w
            .game()
            .unwrap()
            .vehicles
            .iter()
            .find(|(_, vs)| vs.wreck_tick.is_some())
            .unwrap()
            .0;
        let near = w.entities().transform[first.index()].pos.xy();
        assert!(
            w.game()
                .unwrap()
                .vderived
                .block
                .blocked_at(&w.state.terrain, near)
        );
        assert!(w.wreck_vehicle(vb));
        let g = w.game().unwrap();
        assert!(!g.vehicles.contains_key(&first));
        assert!(g.vehicles[&vb].wreck_tick.is_some());
        assert_eq!(w.entities().kind[first.index()], EntityKind::Vacant);
        assert_eq!(w.entities().allocator().free_list().last(), Some(&first.0));
        assert!(
            !g.vderived.block.blocked_at(&w.state.terrain, near),
            "reopened"
        );
        assert!(g.vderived.frame(first).is_none());
        // Its id is reused for the next spawn; nothing still names it as
        // a vehicle.
        let rifle = w
            .match_setup()
            .unwrap()
            .squad_type_index("ww2:ger_grenadiergruppe")
            .unwrap();
        let sq = w
            .spawn_squad_at(
                PlayerId(0),
                rifle,
                FxVec2::from_int(300, 300),
                FxVec2::new(Fx::ONE, Fx::ZERO),
            )
            .unwrap();
        let g = w.game().unwrap();
        assert!(g.squads[&sq].members.contains(&first));
        assert!(!g.vehicles.contains_key(&first));
        assert!(
            g.vehicles
                .values()
                .all(|vs| vs.target != Some(first) && !vs.followers.contains(&first))
        );
        assert!(g.vderived.frame(first).is_none());
        let _ = va;
        let loaded = World::load(&w.save(), 1).unwrap();
        assert_eq!(loaded.state_hash(), w.state_hash());
    }

    // ------------------------------------------------- S3 gunnery, damage --

    use crate::fx::FxVec3;
    use frontline_content::ComponentKind;

    /// Flat open ground, test unlock, both sides human: a German tank at
    /// (500, 500) facing east and nothing else.
    fn flat_tank_world() -> (World, SimId) {
        flat_tank_world_with(|_| {})
    }

    /// [`flat_tank_world`] with `edit` applied to the setup first.
    fn flat_tank_world_with(edit: impl FnOnce(&mut MatchSetup)) -> (World, SimId) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
        let mut defs = frontline_content::ModDefs::load(&root).unwrap();
        let rules = defs
            .add_rules_override(&root.join("tests/rules_test_unlock.toml"))
            .unwrap();
        let mut req = frontline_content::MatchRequest::skirmish(
            1,
            "ww2:germany",
            "ww2:usa",
            frontline_content::Difficulty::Normal,
        );
        req.rules = rules;
        let mut setup = defs.resolve(&req).unwrap();
        setup.players.iter_mut().for_each(|p| p.is_ai = false);
        setup.map.terrain.octaves.clear();
        setup.map.terrain.bands.clear();
        setup.map.woods.clear();
        edit(&mut setup);
        let mut w = World::new_match(setup, 1).unwrap();
        let ger = w
            .match_setup()
            .unwrap()
            .squad_type_index("ww2:ger_pz4_h_squad")
            .unwrap();
        let sq = w
            .spawn_squad_at(
                PlayerId(0),
                ger,
                FxVec2::from_int(500, 500),
                FxVec2::new(Fx::ONE, Fx::ZERO),
            )
            .unwrap();
        let v = w.game().unwrap().squads[&sq].members[0];
        (w, v)
    }

    /// Puts a US round of `ammo` (an ammo id, or small arms for `None`) in
    /// flight at `from` with velocity `vel`.
    fn shoot(w: &mut World, ammo: Option<&str>, from: FxVec3, vel: FxVec3) {
        let s = w.match_setup().unwrap();
        let (weapon, ammo) = match ammo {
            Some(a) => {
                let k = s.ammo.iter().position(|x| x.id == a).unwrap();
                let wk = s
                    .weapons
                    .iter()
                    .position(|x| x.id == "ww2:m3_75mm")
                    .unwrap();
                (wk as u16, k as u16)
            }
            None => {
                let wk = s
                    .weapons
                    .iter()
                    .position(|x| x.id == "ww2:m1_garand")
                    .unwrap();
                (wk as u16, crate::projectile::NO_AMMO)
            }
        };
        // `from.z` is above the ground there.
        let from = FxVec3::new(
            from.x,
            from.y,
            from.z + w.state.terrain.height_at(from.xy()),
        );
        let slot = w
            .state
            .projectiles
            .spawn(crate::projectile::Projectile {
                pos: from,
                vel,
                owner: SimId(9999),
                drag: Fx::ZERO,
                ttl: 120,
                weapon,
                player: 1,
                ammo,
                origin: from,
            })
            .unwrap();
        w.hash.mark_pool_slot(slot);
    }

    fn fx3(x: i32, y: i32, z_milli: i32) -> FxVec3 {
        FxVec3::new(Fx::from_int(x), Fx::from_int(y), Fx::from_milli(z_milli))
    }

    /// Steps until no round flies (at most 2 s); returns the armour events
    /// seen.
    fn fly_out(w: &mut World) -> Vec<crate::snapshot::ArmourEvent> {
        let mut evs = Vec::new();
        for _ in 0..60 {
            w.step(&[]);
            evs.extend(w.snapshot().armour_events.iter().cloned());
            if w.projectiles().live() == 0 {
                break;
            }
        }
        evs
    }

    #[test]
    fn rounds_hit_the_earliest_of_ground_box_and_capsule() {
        use crate::snapshot::ArmourEventKind as K;
        // A Sherman AP round from 100 m east at hull height meets the hull
        // front of a tank facing east.
        let (mut w, v) = flat_tank_world();
        let ap = Some("ww2:75mm_m61");
        shoot(&mut w, ap, fx3(600, 500, 1100), fx3(-618, 0, 0));
        let evs = fly_out(&mut w);
        let plate: Vec<_> = evs.iter().filter(|e| matches!(e.kind, 1..=3)).collect();
        assert_eq!(plate.len(), 1, "{evs:?}");
        assert_eq!(plate[0].vehicle, v.0);
        assert_eq!(
            plate[0].zone,
            frontline_content::ArmourZone::HullFront as u8
        );
        // Small arms stop on it, silently.
        let stopped = w.stats().rounds_stopped;
        shoot(&mut w, None, fx3(530, 500, 1100), fx3(-800, 0, 0));
        assert!(fly_out(&mut w).is_empty());
        assert_eq!(w.stats().rounds_stopped, stopped + 1);

        // A German soldier in front of it: his capsule comes first.
        let (mut w, _) = flat_tank_world();
        let ger = w
            .match_setup()
            .unwrap()
            .squad_type_index("ww2:ger_grenadiergruppe")
            .unwrap();
        let sq = w
            .spawn_squad_at(
                PlayerId(0),
                ger,
                FxVec2::from_int(520, 500),
                FxVec2::new(Fx::ONE, Fx::ZERO),
            )
            .unwrap();
        let man = w.game().unwrap().squads[&sq].members[0];
        let mp = w.entities().transform[man.index()].pos;
        let hits = w.stats().soldier_hits;
        shoot(
            &mut w,
            ap,
            FxVec3::new(mp.x + Fx::from_int(200), mp.y, Fx::ONE),
            fx3(-618, 0, 0),
        );
        let evs = fly_out(&mut w);
        assert_eq!(w.stats().soldier_hits, hits + 1);
        assert!(evs.iter().all(|e| e.kind == K::Fired as u8), "{evs:?}");

        // A parapet in front of the hull blocks a round grazing it.
        let (mut w, _) = flat_tank_world();
        assert!(w.dig_trench_now(&[FxVec2::from_int(506, 490), FxVec2::from_int(506, 510)], 1));
        let p = FxVec2::from_int(505, 500);
        let top = w.terrain().height_at(p) - w.terrain().base_height_at(p);
        let z = top.raw() * 1000 / 65_536 - 50;
        let ground = w.stats().ground_hits;
        shoot(&mut w, ap, fx3(530, 500, z), fx3(-618, 0, 0));
        let evs = fly_out(&mut w);
        assert!(evs.iter().all(|e| !matches!(e.kind, 1..=3)), "{evs:?}");
        assert_eq!(w.stats().ground_hits, ground + 1);
    }

    #[test]
    fn the_fine_terrain_retest_finds_crests_the_coarse_test_steps_over() {
        let w = World::new(SimConfig::default(), 1).unwrap();
        let t = &w.state.terrain;
        let (mut fine_only, mut coarse_hits) = (0, 0);
        for k in 0..2000 {
            let mut r = RngStream::new(5, k, SimId(0), Purpose(9000));
            let x = r.range_i32(50, 450);
            let y = r.range_i32(50, 450);
            let a = FxVec2::from_int(x, y);
            let b = a + FxVec2::from_int(r.range_i32(-12, 12), r.range_i32(-12, 12));
            let lift = Fx::from_milli(r.range_i32(-200, 300));
            let a3 = a.extend(t.height_at(a) + Fx::ONE);
            let b3 = b.extend(t.height_at(b) + lift);
            let coarse = t.segment_hit(a3, b3);
            let fine = t.segment_hit_fine(a3, b3);
            if coarse.is_some() {
                coarse_hits += 1;
                assert!(
                    fine.is_some(),
                    "the fine test never misses what the coarse finds"
                );
            } else if fine.is_some() {
                fine_only += 1;
            }
        }
        assert!(coarse_hits > 100, "{coarse_hits}");
        assert!(fine_only > 0, "no case where 0.25 m steps mattered");
    }

    #[test]
    fn shells_burst_with_their_round_and_bursts_on_vehicles_carve_no_crater() {
        let (mut w, v) = flat_tank_world();
        // HE on the turret front: no penetration (10 mm), a burst, no crater.
        let tiles = w.terrain().detail().tiles().len();
        let ex = w.stats().explosions;
        shoot(
            &mut w,
            Some("ww2:75mm_m48"),
            fx3(560, 500, 2200),
            fx3(-463, 0, 0),
        );
        let evs = fly_out(&mut w);
        let hit = evs.iter().find(|e| matches!(e.kind, 1..=3)).unwrap();
        assert_eq!(
            hit.kind,
            crate::snapshot::ArmourEventKind::NonPenetration as u8
        );
        assert_eq!(hit.vehicle, v.0);
        assert_eq!(w.stats().explosions, ex + 1);
        assert_eq!(
            w.terrain().detail().tiles().len(),
            tiles,
            "no crater on the tank"
        );
        // HE on open ground carves one.
        shoot(
            &mut w,
            Some("ww2:75mm_m48"),
            fx3(560, 520, 1500),
            fx3(-463, 0, -40),
        );
        fly_out(&mut w);
        assert_eq!(w.stats().explosions, ex + 2);
        assert!(w.terrain().detail().tiles().len() > tiles);
    }

    #[test]
    fn knock_out_clears_targets_and_followers_and_counts_the_crew() {
        let (mut w, v) = flat_tank_world();
        let s = w.match_setup().unwrap();
        let usa = s.squad_type_index("ww2:usa_m4a3_75_squad").unwrap();
        let rifle = s.squad_type_index("ww2:ger_grenadiergruppe").unwrap();
        let us_rifle = s.squad_type_index("ww2:usa_rifle_squad").unwrap();
        let east = FxVec2::new(Fx::ONE, Fx::ZERO);
        let e = w
            .spawn_squad_at(PlayerId(1), usa, FxVec2::from_int(700, 500), -east)
            .unwrap();
        let enemy = w.game().unwrap().squads[&e].members[0];
        let f = w
            .spawn_squad_at(PlayerId(0), rifle, FxVec2::from_int(480, 500), east)
            .unwrap();
        let men = w.game().unwrap().squads[&f].members.clone();
        let foe = w
            .spawn_squad_at(PlayerId(1), us_rifle, FxVec2::from_int(300, 300), east)
            .unwrap();
        let foe_man = w.game().unwrap().squads[&foe].members[0];
        // Everyone points at the German tank: the Sherman's gun and order,
        // a US soldier, the US squad's focus; German men follow it.
        w.edit_vehicle(enemy, |vs| {
            vs.target = Some(v);
            vs.attack = Some(v);
        });
        w.edit_vehicle(v, |vs| vs.followers = men.clone());
        let g = w.state.game.as_mut().unwrap();
        g.squads.get_mut(&foe).unwrap().focus = Some(v);
        w.state.entities.combat[foe_man.index()].target = Some(v);
        let losses = w.game().unwrap().players[0].losses;
        assert!(w.wreck_vehicle(v));
        let g = w.game().unwrap();
        assert_eq!(g.vehicles[&enemy].target, None);
        assert_eq!(g.vehicles[&enemy].attack, None);
        assert_eq!(g.squads[&foe].focus, None);
        assert_eq!(w.entities().combat[foe_man.index()].target, None);
        assert_eq!(g.players[0].losses, losses + 5, "the crew count as losses");
        assert!(g.vehicles[&v].followers.is_empty());
        assert_eq!(g.squads[&f].order, Order::Idle);
        assert_eq!(w.stats().knockouts, 1);
        let ko = w
            .snapshot()
            .armour_events
            .iter()
            .find(|e| e.kind == crate::snapshot::ArmourEventKind::KnockedOut as u8)
            .unwrap();
        assert_eq!(ko.vehicle, v.0);
        let loaded = World::load(&w.save(), 1).unwrap();
        assert_eq!(loaded.state_hash(), w.state_hash());
    }

    #[test]
    fn a_crest_in_front_of_a_hull_makes_it_hull_down() {
        let (mut w, v) = flat_tank_world();
        // A 1.5 m crest 8 m east of the tank (grid x 127 = 508 m).
        w.state.terrain.edit_heights_for_test(|gx, _, h| {
            if gx == 127 {
                *h += Fx::from_milli(1500);
            }
        });
        w.state.terrain.rebuild_derived();
        let g = w.game().unwrap();
        let f = *g.vderived.frame(v).unwrap();
        let from = FxVec3::new(
            Fx::from_int(800),
            Fx::from_int(500),
            w.terrain().height_at(FxVec2::from_int(800, 500)) + Fx::from_milli(2300),
        );
        assert!(
            !w.terrain().line_of_sight(from, f.hull.center),
            "hull hidden"
        );
        let part = crate::gunnery::aim_choice(from, &f, w.terrain());
        assert_eq!(part, Some(crate::armour::BoxPart::Turret));
        // A round aimed level at the hull's upper edge over the crest top
        // (the coarse 1 m test can step over it; the re-test cannot).
        let top = w.terrain().height_at(FxVec2::from_int(508, 500))
            - w.terrain().height_at(FxVec2::from_int(500, 500));
        for dz in [-60, -20] {
            let ground = w.stats().ground_hits;
            let z = (top.raw() as i64 * 1000 / 65_536) as i32 + dz;
            shoot(
                &mut w,
                Some("ww2:75mm_m61"),
                fx3(530, 500, z),
                fx3(-618, 0, 0),
            );
            let evs = fly_out(&mut w);
            assert!(
                evs.iter().all(|e| !matches!(e.kind, 1..=3)),
                "{dz}: {evs:?}"
            );
            assert_eq!(w.stats().ground_hits, ground + 1);
        }
    }

    /// Applies `dealt` to vehicle `v` as the Damage stage would, at `tick`
    /// with the ammo-rack key `slot`. Returns the knock-out cause.
    fn deal_now(
        w: &mut World,
        v: SimId,
        dealt: &[crate::damage::Dealt],
        tick: u64,
        slot: u32,
        fires: &mut crate::damage::FireRolls,
    ) -> Option<u8> {
        let World {
            state,
            hash,
            spatial,
            ..
        } = w;
        let SimState {
            entities,
            terrain,
            stats,
            game,
            ..
        } = state;
        let mut ctx = combat::DamageCtx {
            ents: entities,
            terrain,
            spatial,
            hash,
            stats,
            seed: 1,
            tick,
        };
        let g = game.as_mut().unwrap();
        g.deal(
            &mut ctx,
            v,
            dealt,
            (0, crate::game::kill_class::TANK_AP),
            SimId(slot),
            fires,
            &mut Vec::new(),
            FxVec3::ZERO,
            0,
        )
    }

    #[test]
    fn fire_and_ammo_rack_rolls_match_their_odds() {
        use crate::damage::{Dealt, FireRolls, Inner, KO_RACK};
        let (mut w, v) = flat_tank_world();
        let fresh = w.game().unwrap().vehicles[&v].clone();
        let reset = |w: &mut World| {
            *w.state.game.as_mut().unwrap().vehicles.get_mut(&v).unwrap() = fresh.clone();
        };
        let hit = |inner, dmg| Dealt { inner, dmg };
        let rack = Inner::Part(ComponentKind::AmmoRack);
        let engine = Inner::Part(ComponentKind::Engine);
        let n = 600u32;
        let (mut det, mut fires, mut ammo_fires) = (0u32, 0u32, 0u32);
        for k in 0..n {
            reset(&mut w);
            let mut f = FireRolls::default();
            if deal_now(&mut w, v, &[hit(rack, 30)], 7, k, &mut f) == Some(KO_RACK) {
                det += 1;
            } else if w.game().unwrap().vehicles[&v].ammo_fire > 0 {
                ammo_fires += 1;
            }
            reset(&mut w);
            deal_now(&mut w, v, &[hit(engine, 40)], k as u64, 0, &mut f);
            if w.game().unwrap().vehicles[&v].burn > 0 {
                fires += 1;
            }
            // Below the thresholds: never.
            reset(&mut w);
            assert_eq!(deal_now(&mut w, v, &[hit(rack, 29)], 7, k, &mut f), None);
            assert_eq!(w.game().unwrap().vehicles[&v].ammo_fire, 0);
            reset(&mut w);
            deal_now(&mut w, v, &[hit(engine, 39)], k as u64, 0, &mut f);
            assert_eq!(w.game().unwrap().vehicles[&v].burn, 0);
        }
        let within = |count: u32, p: f64| {
            #[allow(clippy::float_arithmetic)]
            let ok = {
                let sigma = (n as f64 * p * (1.0 - p)).sqrt();
                (count as f64 - n as f64 * p).abs() <= 3.0 * sigma + 1.0
            };
            ok
        };
        // S3b: detonation 20 %; else an ammo fire 60 % of the rest.
        let (pd, pf) = {
            let r = &w.match_setup().unwrap().rules.vehicles;
            (r.rack_detonate_pct, r.damage.rack_fire_pct)
        };
        let pdet = pd as f64 / 100.0;
        #[allow(clippy::float_arithmetic)]
        let pfire = (1.0 - pdet) * pf as f64 / 100.0;
        assert!(within(det, pdet), "rack detonations {det} of {n}");
        assert!(within(ammo_fires, pfire), "ammo fires {ammo_fires} of {n}");
        assert!(within(fires, 0.30), "fires {fires} of {n}");
        // A rack destroyed by damage detonates or burns, never nothing;
        // the same key always gives the same roll.
        for k in 0..40 {
            reset(&mut w);
            let mut f = FireRolls::default();
            let ko = deal_now(&mut w, v, &[hit(rack, 60)], 7, k, &mut f);
            assert!(ko == Some(KO_RACK) || w.game().unwrap().vehicles[&v].ammo_fire > 0);
        }
        let first: Vec<_> = (0..20)
            .map(|k| {
                reset(&mut w);
                deal_now(&mut w, v, &[hit(rack, 30)], 7, k, &mut FireRolls::default())
            })
            .collect();
        let again: Vec<_> = (0..20)
            .map(|k| {
                reset(&mut w);
                deal_now(&mut w, v, &[hit(rack, 30)], 7, k, &mut FireRolls::default())
            })
            .collect();
        assert_eq!(first, again);
        // Every seat emptied: knocked out (crew); a crewman's death is a
        // loss and a kill.
        reset(&mut w);
        let losses = w.game().unwrap().players[0].losses;
        let kills = w.game().unwrap().players[1].kills;
        // S3c: four of five dead: no knock-out here any more; the Status
        // stage abandons the vehicle (`crew_below`), and the fifth man
        // bails out.
        let seats: Vec<Dealt> = (0..4).map(|k| hit(Inner::Seat(k), 100)).collect();
        let mut evs = Vec::new();
        {
            let World {
                state,
                hash,
                spatial,
                ..
            } = &mut w;
            let mut ctx = combat::DamageCtx {
                ents: &mut state.entities,
                terrain: &mut state.terrain,
                spatial,
                hash,
                stats: &mut state.stats,
                seed: 1,
                tick: 9,
            };
            let g = state.game.as_mut().unwrap();
            let ko = g.deal(
                &mut ctx,
                v,
                &seats,
                (1, crate::game::kill_class::TANK_AP),
                SimId(0),
                &mut FireRolls::default(),
                &mut evs,
                FxVec3::ZERO,
                0,
            );
            assert_eq!(ko, None);
        }
        assert_eq!(evs.len(), 4);
        assert_eq!(w.game().unwrap().players[0].losses, losses + 4);
        assert_eq!(w.game().unwrap().players[1].kills, kills + 4);
        assert_eq!(
            w.game().unwrap().players[1].kills_by[crate::game::kill_class::TANK_AP as usize],
            4
        );
        // Three dead: two still fight on.
        reset(&mut w);
        let three: Vec<Dealt> = (0..3).map(|k| hit(Inner::Seat(k), 100)).collect();
        assert_eq!(
            deal_now(&mut w, v, &three, 9, 0, &mut FireRolls::default()),
            None
        );
    }

    #[test]
    fn an_ammo_fire_burns_the_vehicle_out_and_nobody_puts_it_out() {
        use crate::damage::KO_AMMO_FIRE;
        let (mut w, v) = flat_tank_world();
        w.edit_vehicle(v, |vs| vs.ammo_fire = 1);
        let hz = w.match_setup().unwrap().tick_hz as u64;
        let secs = w.match_setup().unwrap().rules.vehicles.damage.ammo_fire_s as u64;
        let mut ko = None;
        for _ in 0..(secs + 2) * hz {
            w.step(&[]);
            for e in &w.snapshot().armour_events {
                if e.kind == crate::snapshot::ArmourEventKind::KnockedOut as u8 {
                    ko = Some((w.tick(), e.component));
                }
            }
            if ko.is_some() {
                break;
            }
            let vs = &w.game().unwrap().vehicles[&v];
            assert!(vs.ammo_fire > 0, "the crew never puts it out");
        }
        let (t, cause) = ko.expect("knocked out");
        assert_eq!(cause, KO_AMMO_FIRE);
        assert!(
            t >= secs * hz - 1 && t <= secs * hz + 1,
            "after {secs} s: {t}"
        );
    }

    #[test]
    fn penetrations_are_keyed_by_tick_and_slot() {
        // M61 into the Panzer IV's turret side by the gunner from 100 m
        // north: 30 mm, it goes through; core, spall and filler hit the
        // crew and the gun. (S3b: through the hull side by the driver every
        // filler ray strikes the transmission between the two front seats:
        // no variation to see there.)
        let run = |delay: u64, threads: usize| {
            let (w, v) = flat_tank_world();
            let st = w.state.clone();
            let mut w = World::from_state(st, threads).unwrap();
            for _ in 0..delay {
                w.step(&[]);
            }
            let p = FxVec3::new(
                Fx::from_milli(500_400),
                Fx::from_int(600),
                Fx::from_milli(2100),
            );
            shoot(&mut w, Some("ww2:75mm_m61"), p, fx3(0, -618, 0));
            let evs = fly_out(&mut w);
            assert!(
                evs.iter()
                    .any(|e| e.kind == crate::snapshot::ArmourEventKind::Penetration as u8),
                "{evs:?}"
            );
            let vs = w.game().unwrap().vehicles[&v].clone();
            (w.state_hash(), vs.seats.clone(), vs.components)
        };
        let a = run(0, 1);
        assert_eq!(
            run(0, 3),
            a,
            "the same shot at the same tick: identical damage"
        );
        // Other ticks roll other spall: not every outcome is the same.
        let outcomes: Vec<_> = (1..8).map(|d| run(d, 1)).map(|r| (r.1, r.2)).collect();

        assert!(outcomes.iter().any(|o| *o != (a.1.clone(), a.2)));
        // Something inside was hurt.
        assert!(a.1.iter().any(|s| s.hp < 100) || a.2 != [100, 100, 100, 80, 80, 60, 100]);
    }

    /// Puts a US 60 mm mortar shell in flight straight down onto `at`
    /// from 20 m up.
    fn drop_mortar(w: &mut World, at: FxVec2) {
        let s = w.match_setup().unwrap();
        let weapon = s
            .weapons
            .iter()
            .position(|x| x.id == "ww2:m1_mortar")
            .unwrap() as u16;
        let from = at.extend(w.state.terrain.height_at(at) + Fx::from_int(20));
        let slot = w
            .state
            .projectiles
            .spawn(crate::projectile::Projectile {
                pos: from,
                vel: FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::from_int(-60)),
                owner: SimId(9999),
                drag: Fx::ZERO,
                ttl: 120,
                weapon,
                player: 1,
                ammo: crate::projectile::NO_AMMO,
                origin: from,
            })
            .unwrap();
        w.hash.mark_pool_slot(slot);
    }

    /// A lone German rifleman (player 0) at `at` in `w`, his squad's other
    /// men killed.
    fn lone_rifleman(w: &mut World, at: FxVec2) -> SimId {
        let ty = w
            .match_setup()
            .unwrap()
            .squad_type_index("ww2:ger_grenadiergruppe")
            .unwrap();
        let north = FxVec2::new(Fx::ZERO, Fx::ONE);
        let sq = w.spawn_squad_at(PlayerId(0), ty, at, north).unwrap();
        let men = w.game().unwrap().squads[&sq].members.clone();
        for &m in &men[1..] {
            w.kill_soldier(m);
        }
        w.place_squad(sq, at, north);
        men[0]
    }

    #[test]
    fn fragments_stop_at_vehicles_and_reach_men_in_the_open() {
        // US mortar shells burst 3 m south of the German tank's centre
        // line; a German rifleman stands 2.5 m north of it. With the tank between, every
        // fragment that could reach him strikes the hull; with the tank
        // gone, some reach him (S3b physical-contact rule).
        // S3b Tune: in the open, three fresh men at x 494, 500 and 506
        // (one per run) and the shells 4 m apart: after the first burst a
        // man goes prone, the later bombs burst in earlier craters, and the
        // earth stops low fragments, so one man under 12 bombs could, with
        // 120 rays a bomb, by chance take none (the earth blocks, as it
        // should). The test means: some fragments reach men in the open.
        let run = |with_tank: bool, threads: usize, man_x: i32| {
            let (w, v) = flat_tank_world();
            let mut w = World::from_state(w.state.clone(), threads).unwrap();
            if !with_tank {
                w.place_vehicle(v, FxVec2::from_int(300, 300), Fx::ZERO);
            }
            let man = lone_rifleman(
                &mut w,
                FxVec2::new(Fx::from_int(man_x), Fx::from_milli(502_500)),
            );
            w.step(&[]);
            let hp = |w: &World| w.entities().combat()[man.index()].hp as u32;
            let start = hp(&w);
            for k in 0..12 {
                if w.entities().kind()[man.index()] != EntityKind::Soldier {
                    break;
                }
                let at = if with_tank {
                    FxVec2::new(Fx::from_int(499 + k % 3), Fx::from_int(497))
                } else {
                    FxVec2::new(
                        Fx::from_int(490 + 4 * (k % 6)),
                        Fx::from_int(497 - 4 * (k / 6)),
                    )
                };
                drop_mortar(&mut w, at);
                fly_out(&mut w);
            }
            let lost = if w.entities().kind()[man.index()] == EntityKind::Soldier {
                start - hp(&w)
            } else {
                start
            };
            (
                lost,
                w.stats().frag_rays,
                w.stats().frag_hits,
                w.state_hash(),
            )
        };
        let (lost, rays, _, h1) = run(true, 1, 500);
        assert_eq!(lost, 0, "the hull stops every fragment");
        let (w, _) = flat_tank_world();
        let n = w
            .match_setup()
            .unwrap()
            .weapons
            .iter()
            .find(|x| x.id == "ww2:m1_mortar")
            .and_then(|x| x.he.as_ref())
            .unwrap()
            .frag_count as u64;
        // S3c: bursts on an earlier crater's rim add rays down over the
        // fall (`frag_fall_probe_cm`), at most `n` per burst.
        assert!((12 * n..=24 * n).contains(&rays), "{rays} rays");
        assert_eq!(run(true, 3, 500).3, h1, "1 vs 3 threads");
        let (lost, hits) = [494, 500, 506].iter().fold((0, 0), |(l, h), &x| {
            let r = run(false, 1, x);
            (l + r.0, h + r.2)
        });
        assert!(lost > 0 && hits > 0, "in the open: {lost} hp, {hits} hits");
    }

    #[test]
    fn the_fragment_budget_scales_a_crowded_tick_down() {
        assert_eq!(combat::frag_share_for_test(96, 4096), 96);
        assert_eq!(combat::frag_share_for_test(96, 4800), 81);
        // 50 shells bursting in one tick ask for 6 000 rays (S3b Tune: 120
        // per bomb): each throws its share of the 4 096 budget.
        let share = combat::frag_share_for_test(120, 50 * 120);
        assert!(share < 120);
        let (mut w, _) = flat_tank_world();
        for k in 0..50 {
            drop_mortar(&mut w, FxVec2::from_int(300 + 4 * k, 300));
        }
        fly_out(&mut w);
        let st = w.stats();
        assert_eq!(st.explosions, 50);
        assert!(st.frag_rays <= combat::MAX_FRAG_RAYS_PER_TICK as u64);
        assert_eq!(st.frag_rays + st.frag_rays_capped, 50 * 120);
        assert_eq!(st.frag_rays, 50 * share as u64);
    }

    #[test]
    fn a_round_on_the_barrel_damages_the_gun_and_nothing_else() {
        // The Panzer IV faces east; its barrel runs from 1.15 m to 4.5 m in
        // front of the turret pivot (x 500.2) at 2.21 m. An M61 from the
        // north strikes it 3 m out from the pivot.
        let (mut w, v) = flat_tank_world();
        let fresh = w.game().unwrap().vehicles[&v].clone();
        let f = *w.game().unwrap().vderived.frame(v).unwrap();
        assert!(f.barrel.half.x > Fx::ZERO);
        let z = (f.barrel.center.z - w.terrain().height_at(f.barrel.center.xy())).raw() as i64
            * 1000
            / 65_536;
        let from = FxVec3::new(
            f.pivot.x + Fx::from_int(3),
            Fx::from_int(560),
            Fx::from_milli(z as i32),
        );
        shoot(&mut w, Some("ww2:75mm_m61"), from, fx3(0, -618, 0));
        let evs = fly_out(&mut w);
        let vs = &w.game().unwrap().vehicles[&v];
        let s = w.match_setup().unwrap();
        let m61 = s.ammo.iter().find(|a| a.id == "ww2:75mm_m61").unwrap();
        assert_eq!(
            vs.component(ComponentKind::Gun) as u32,
            fresh.component(ComponentKind::Gun) as u32 - m61.nonpen_damage,
            "{evs:?}"
        );
        assert_eq!(vs.seats, fresh.seats, "nobody inside is touched");
        assert_eq!(w.stats().penetrations, 0);
        // Small arms stop on it with no damage.
        let gun = vs.component(ComponentKind::Gun);
        let stopped = w.stats().rounds_stopped;
        shoot(&mut w, None, from, fx3(0, -800, 0));
        fly_out(&mut w);
        assert_eq!(w.stats().rounds_stopped, stopped + 1);
        let vs = &w.game().unwrap().vehicles[&v];
        assert_eq!(vs.component(ComponentKind::Gun), gun);
        // A non-penetrating hit on the hull near the barrel no longer
        // touches the gun (no proximity damage).
        let mut w2 = World::from_state(w.state.clone(), 1).unwrap();
        let gun = w2.game().unwrap().vehicles[&v].component(ComponentKind::Gun);
        shoot(
            &mut w2,
            Some("ww2:75mm_m61"),
            fx3(560, 500, 1200),
            fx3(-618, 0, 0),
        );
        fly_out(&mut w2);
        assert_eq!(
            w2.game().unwrap().vehicles[&v].component(ComponentKind::Gun),
            gun
        );
    }

    #[test]
    fn he_that_beats_a_plate_bursts_in_rays_inside() {
        // A made-up HE round that defeats 200 mm: its inner rays from the
        // contact point on the hull's front plate reach the front seats
        // and the transmission, never the engine at the rear.
        let (mut w, v) = flat_tank_world_with(|setup| {
            let a = setup
                .ammo
                .iter_mut()
                .find(|a| a.id == "ww2:75mm_m48")
                .unwrap();
            a.penetration[0].mm = 200;
            let b = a.blast.as_mut().unwrap();
            b.blast.pen_mm = 200;
            assert!(b.blast.inside_rays > 0);
        });
        let fresh = w.game().unwrap().vehicles[&v].clone();
        shoot(
            &mut w,
            Some("ww2:75mm_m48"),
            fx3(560, 500, 1300),
            fx3(-463, 0, 0),
        );
        let evs = fly_out(&mut w);
        assert!(
            evs.iter()
                .any(|e| e.kind == crate::snapshot::ArmourEventKind::Penetration as u8)
        );
        let vs = &w.game().unwrap().vehicles[&v];
        let hurt_front = vs.seats[0].hp < fresh.seats[0].hp
            || vs.seats[4].hp < fresh.seats[4].hp
            || vs.component(ComponentKind::Transmission)
                < fresh.component(ComponentKind::Transmission);
        assert!(hurt_front, "{vs:?}");
        assert_eq!(
            vs.component(ComponentKind::Engine),
            fresh.component(ComponentKind::Engine)
        );
    }

    #[test]
    fn mortar_blasts_break_tracks_but_never_reach_inside() {
        use crate::snapshot::ArmourEventKind as K;
        let (mut w, v) = flat_tank_world();
        let hp = |w: &World, k: ComponentKind| w.game().unwrap().vehicles[&v].component(k);
        let seats = |w: &World| -> Vec<u16> {
            w.game().unwrap().vehicles[&v]
                .seats
                .iter()
                .map(|s| s.hp)
                .collect()
        };
        let crew = seats(&w);
        // 1 m outside the right (south) track: overpressure 150 × 40 % on
        // that track (anchor within the 1.5 m blast radius), and S3b
        // fragments striking that side's track band, 1 each.
        let beside = FxVec2::new(Fx::from_int(500), Fx::from_milli(497_750));
        drop_mortar(&mut w, beside);
        let evs = fly_out(&mut w);
        let right = hp(&w, ComponentKind::TrackRight);
        assert!(right < 40 && right > 0, "{right} {evs:?}");
        assert!(w.stats().frag_hits > 0 && w.stats().frag_rays > 0);
        assert_eq!(
            hp(&w, ComponentKind::TrackLeft),
            100,
            "3.5 m off, and the hull shields the far side's band"
        );
        assert!(evs.iter().any(|e| e.kind == K::ComponentHit as u8));
        assert_eq!(w.stats().blast_track_hits, 1);
        // More near hits along that side break it (the first crater's rim
        // may shield the anchor from the next burst's overpressure; the
        // fragments still reach the band), with one Disabled event.
        let mut disabled = 0;
        for k in 0..6 {
            if hp(&w, ComponentKind::TrackRight) == 0 {
                break;
            }
            drop_mortar(
                &mut w,
                FxVec2::new(Fx::from_int(499 + (k % 3)), Fx::from_milli(497_750)),
            );
            let evs = fly_out(&mut w);
            disabled += evs
                .iter()
                .filter(|e| {
                    e.kind == K::Disabled as u8 && e.component == ComponentKind::TrackRight as u8
                })
                .count();
        }
        assert_eq!(hp(&w, ComponentKind::TrackRight), 0, "near hits break it");
        assert_eq!(disabled, 1);
        // A direct hit on the hull top: 8 mm against 12 mm, and its blast
        // spares the vehicle it struck.
        // (Mortar shells on plates make no armour event; the feed shows
        // main-gun and rocket matters only.)
        let (mut w, v) = flat_tank_world();
        let tiles = w.terrain().detail().tiles().len();
        drop_mortar(&mut w, FxVec2::from_int(500, 500));
        let evs = fly_out(&mut w);
        assert!(evs.is_empty(), "{evs:?}");
        assert_eq!(w.stats().explosions, 1, "it burst on the tank");
        assert_eq!(w.terrain().detail().tiles().len(), tiles, "no crater");
        let vs = &w.game().unwrap().vehicles[&v];
        assert_eq!(
            vs.components,
            VehicleState::new(
                vs.type_idx,
                &w.match_setup().unwrap().vehicles[vs.type_idx as usize],
                &w.match_setup().unwrap().rules.vehicles
            )
            .components
        );
        assert_eq!(seats(&w), crew);
        assert_eq!(w.stats().knockouts, 0);
        assert_eq!(w.stats().penetrations, 0);
    }

    /// Flat ground, test unlock: a German tank at (500, 500) with a rifle
    /// squad following it and one detached rifleman following it too, a
    /// Panzerschreck team attacking a US tank at (600, 500), and an
    /// engineer squad digging an AT ditch.
    fn order_world() -> (World, SimId, SimId, SquadId, SquadId) {
        // S3c: no bail-out, so the two tanks' brief fight leaves both
        // manned whatever it hits (the test is about order state).
        let (mut w, va) = flat_tank_world_with(|s| {
            let b = &mut s.rules.vehicles.bail;
            b.ammo_fire_bail = 0;
            b.engine_fire_bail_s = 0;
            b.crew_below = 0;
            b.useless_bail = 0;
            b.under_fire_s = 0;
        });
        let ty = |w: &World, id: &str| w.match_setup().unwrap().squad_type_index(id).unwrap();
        let east = FxVec2::new(Fx::ONE, Fx::ZERO);
        let north = FxVec2::new(Fx::ZERO, Fx::ONE);
        let usa = ty(&w, "ww2:usa_m4a3_75_squad");
        let b = w
            .spawn_squad_at(PlayerId(1), usa, FxVec2::from_int(600, 500), -east)
            .unwrap();
        let vb = w.game().unwrap().squads[&b].members[0];
        let rif = ty(&w, "ww2:ger_grenadiergruppe");
        let r = w
            .spawn_squad_at(PlayerId(0), rif, FxVec2::from_int(480, 520), north)
            .unwrap();
        let at = ty(&w, "ww2:ger_panzerschreck_team");
        let t = w
            .spawn_squad_at(PlayerId(0), at, FxVec2::from_int(540, 470), east)
            .unwrap();
        let eng = ty(&w, "ww2:ger_engineer_squad");
        let e = w
            .spawn_squad_at(PlayerId(0), eng, FxVec2::from_int(400, 400), north)
            .unwrap();
        let g = w.game().unwrap();
        let men = g.squads[&r].members.clone();
        let team = g.squads[&t].members.clone();
        let cmd = |seq, kind| Command {
            player: PlayerId(0),
            seq,
            kind,
        };
        // Let both sides see each other first.
        for _ in 0..60 {
            w.step(&[]);
        }
        w.step(&[
            cmd(
                0,
                CommandKind::FollowUnits {
                    units: men[..men.len() - 1].to_vec(),
                    vehicle: va,
                },
            ),
            cmd(
                1,
                CommandKind::FollowUnits {
                    units: men.clone(),
                    vehicle: va,
                },
            ),
            cmd(
                2,
                CommandKind::AttackUnit {
                    units: team.clone(),
                    target: vb,
                },
            ),
            cmd(
                3,
                CommandKind::Dig {
                    squad: e,
                    line: vec![FxVec2::from_int(390, 380), FxVec2::from_int(410, 380)],
                    profile: frontline_content::DigProfile::AtDitch,
                },
            ),
        ]);
        // Detach one man with his own follow order.
        w.step(&[cmd(
            4,
            CommandKind::FollowUnits {
                units: vec![men[0]],
                vehicle: va,
            },
        )]);
        for _ in 0..30 {
            w.step(&[]);
        }
        (w, va, vb, r, t)
    }

    /// Review fix: an AT-ditch order cancelled before any digging leaves
    /// the map as it was (an order along someone's trench used to turn
    /// that trench into "ditch" ground for the rest of the match); a dug
    /// section is recorded once its first pass is stamped.
    #[test]
    fn at_ditch_sections_are_recorded_only_once_dug() {
        let (mut w, _) = flat_tank_world();
        let eng = w
            .match_setup()
            .unwrap()
            .squad_type_index("ww2:ger_engineer_squad")
            .unwrap();
        let north = FxVec2::new(Fx::ZERO, Fx::ONE);
        let e = w
            .spawn_squad_at(PlayerId(0), eng, FxVec2::from_int(400, 400), north)
            .unwrap();
        let trench = [FxVec2::from_int(300, 300), FxVec2::from_int(330, 300)];
        assert!(w.dig_trench_now(&trench, 1));
        let cmd = |seq, kind| Command {
            player: PlayerId(0),
            seq,
            kind,
        };
        let ditch_on = |w: &World, p: FxVec2| {
            let node = crate::vehicle::node_at(w.terrain(), p);
            w.game().unwrap().vderived.block.is_ditch(node)
        };
        let mid = FxVec2::from_int(315, 300);
        w.step(&[cmd(
            0,
            CommandKind::Dig {
                squad: e,
                line: trench.to_vec(),
                profile: frontline_content::DigProfile::AtDitch,
            },
        )]);
        w.step(&[cmd(1, CommandKind::Stop { squad: e })]);
        w.step(&[]);
        assert!(w.game().unwrap().at_ditches.is_empty());
        assert!(!ditch_on(&w, mid), "a cancelled order changed the trench");

        // A real job near the engineers: sections appear as they are dug,
        // each a part of the ordered line.
        let line = [FxVec2::from_int(395, 400), FxVec2::from_int(405, 400)];
        w.step(&[cmd(
            2,
            CommandKind::Dig {
                squad: e,
                line: line.to_vec(),
                profile: frontline_content::DigProfile::AtDitch,
            },
        )]);
        let mut ticks = 0;
        while w.game().unwrap().at_ditches.is_empty() && ticks < 3 * 60 * 30 {
            w.step(&[]);
            ticks += 1;
        }
        let g = w.game().unwrap();
        assert!(!g.at_ditches.is_empty(), "no section recorded in 3 min");
        for l in &g.at_ditches {
            assert!(l.len() >= 2 && l.len() < 4);
            assert!(l.iter().all(|q| q.y == Fx::from_int(400)
                && q.x >= Fx::from_int(395)
                && q.x <= Fx::from_int(405)));
            assert!(l[1].x > l[0].x, "a section shorter than the line, in order");
        }
        assert!(g.at_ditches.iter().all(|l| l[..] != line[..]));
        assert!(!ditch_on(&w, mid));
        let loaded = World::load(&w.save(), 2).unwrap();
        assert_eq!(loaded.full_state_hash(), w.full_state_hash());
    }

    /// S4: the Panzerfaust aim and gap, crew-weapon drops and the S4
    /// counters are saved, every field changes the hash, and broken values
    /// are refused on load.
    #[test]
    fn s4_squad_and_player_state_is_saved_hashed_and_checked() {
        use crate::game::{S4Counters, SingleShot, WeaponDrop};
        let (mut w, va) = flat_tank_world_with(|_| {});
        let ty = |w: &World, id: &str| w.match_setup().unwrap().squad_type_index(id).unwrap();
        let t = ty(&w, "ww2:ger_grenadiergruppe");
        let g = w
            .spawn_squad_at(
                PlayerId(0),
                t,
                FxVec2::from_int(300, 500),
                FxVec2::new(Fx::ONE, Fx::ZERO),
            )
            .unwrap();
        // An enemy vehicle for the aim.
        let us_tank = ty(&w, "ww2:usa_m4a3_75_squad");
        let vs = w
            .spawn_squad_at(
                PlayerId(1),
                us_tank,
                FxVec2::from_int(600, 500),
                FxVec2::new(Fx::NEG_ONE, Fx::ZERO),
            )
            .unwrap();
        let vb = w.game().unwrap().squads[&vs].members[0];
        let _ = va;
        let men = w.game().unwrap().squads[&g].members.clone();
        let mg = w
            .match_setup()
            .unwrap()
            .weapons
            .iter()
            .position(|x| x.id == "ww2:mg42_bipod")
            .unwrap() as u16;
        {
            let gm = w.state.game.as_mut().unwrap();
            let sq = gm.squads.get_mut(&g).unwrap();
            sq.pzf = Some(SingleShot {
                man: Some(men[4]),
                target: Some(vb),
                aim_turret: false,
                aim: 10,
                gap: 0,
                search: 3,
                attack: Some(vb),
            });
            sq.drops.push(WeaponDrop {
                weapon: mg,
                at: FxVec2::from_int(300, 500),
                ticks: 40,
                mag: 7,
            });
            gm.players[0].s4 = S4Counters {
                pickups: 2,
                single_shots: 1,
            };
        }
        let w = World::from_state(w.state.clone(), 1).unwrap();
        let loaded = World::load(&w.save(), 2).unwrap();
        assert_eq!(loaded.state(), w.state());
        assert_eq!(loaded.full_state_hash(), w.full_state_hash());
        // Every field changes the hash (the destructures fail to compile
        // when a field is added and not listed here).
        let SingleShot {
            man: _,
            target: _,
            aim_turret: _,
            aim: _,
            gap: _,
            search: _,
            attack: _,
        } = SingleShot::default();
        let WeaponDrop {
            weapon: _,
            at: _,
            ticks: _,
            mag: _,
        } = WeaponDrop::default();
        let S4Counters {
            pickups: _,
            single_shots: _,
        } = S4Counters::default();
        type StateEdit = Box<dyn Fn(&mut SimState)>;
        fn pz(s: &mut SimState, g: SquadId) -> &mut SingleShot {
            s.game
                .as_mut()
                .unwrap()
                .squads
                .get_mut(&g)
                .unwrap()
                .pzf
                .as_mut()
                .unwrap()
        }
        fn dr(s: &mut SimState, g: SquadId) -> &mut WeaponDrop {
            &mut s.game.as_mut().unwrap().squads.get_mut(&g).unwrap().drops[0]
        }
        let (m0, m5) = (men[0], men[5]);
        let base = w.full_state_hash();
        let edits: Vec<(&str, StateEdit)> = vec![
            ("pzf.man", Box::new(move |s| pz(s, g).man = Some(m5))),
            ("pzf.target", Box::new(move |s| pz(s, g).target = None)),
            (
                "pzf.aim_turret",
                Box::new(move |s| pz(s, g).aim_turret ^= true),
            ),
            ("pzf.aim", Box::new(move |s| pz(s, g).aim -= 1)),
            ("pzf.gap", Box::new(move |s| pz(s, g).gap += 1)),
            ("pzf.search", Box::new(move |s| pz(s, g).search -= 1)),
            ("pzf.attack", Box::new(move |s| pz(s, g).attack = None)),
            ("drop.weapon", Box::new(move |s| dr(s, g).weapon += 1)),
            ("drop.at", Box::new(move |s| dr(s, g).at.x += Fx::ONE)),
            ("drop.ticks", Box::new(move |s| dr(s, g).ticks -= 1)),
            ("drop.mag", Box::new(move |s| dr(s, g).mag += 1)),
            (
                "s4.pickups",
                Box::new(|s| s.game.as_mut().unwrap().players[0].s4.pickups += 1),
            ),
            (
                "s4.single_shots",
                Box::new(|s| s.game.as_mut().unwrap().players[0].s4.single_shots += 1),
            ),
        ];
        for (name, edit) in &edits {
            let mut st = w.state.clone();
            edit(&mut st);
            let w2 = World::from_state(st, 1).unwrap();
            assert_ne!(w2.full_state_hash(), base, "{name} is not hashed");
            assert_eq!(w2.full_state_hash(), w2.state_hash());
        }
        // Loads refuse a broken aim, gap or drop.
        let bad: Vec<(StateEdit, &str)> = vec![
            (Box::new(move |s| pz(s, g).aim = 9999), "single-shot"),
            (Box::new(move |s| pz(s, g).gap = 9999), "single-shot"),
            (Box::new(move |s| pz(s, g).man = None), "single-shot"),
            (Box::new(move |s| pz(s, g).target = Some(m0)), "single-shot"),
            (Box::new(move |s| dr(s, g).ticks = 9999), "crew weapon"),
            (Box::new(move |s| dr(s, g).weapon = 0), "crew weapon"),
            (
                Box::new(move |s| s.game.as_mut().unwrap().squads.get_mut(&g).unwrap().pzf = None),
                "single-shot",
            ),
        ];
        for (edit, want) in bad {
            let bytes = save_edited(&w, edit);
            let e = load_err(&bytes);
            assert!(e.contains(want), "{e}");
        }
    }

    /// S5a: each man's carried rounds, sidearm and refill state, the
    /// rounds left in a dropped crew weapon, a vehicle's due rearm steps and
    /// the per-player ammunition account are saved, every field changes the
    /// hash, and broken values are refused on load.
    #[test]
    fn s5a_ammo_state_is_saved_hashed_and_checked() {
        use crate::supply::AmmoAccount;
        let (w, va) = flat_tank_world_with(|_| {});
        let mut w = w;
        let t = w
            .match_setup()
            .unwrap()
            .squad_type_index("ww2:ger_grenadiergruppe")
            .unwrap();
        let g = w
            .spawn_squad_at(
                PlayerId(0),
                t,
                FxVec2::from_int(300, 500),
                FxVec2::new(Fx::ONE, Fx::ZERO),
            )
            .unwrap();
        let men = w.game().unwrap().squads[&g].members.clone();
        let (lead, gunner) = (men[0], men[1]);
        // The gunner dry on his pistol, a refill under way, a paid account.
        assert!(w.edit_soldier(gunner, |k, c| {
            k.on_sidearm = true;
            k.refill = 20;
            c.mag = 5;
        }));
        assert!(w.edit_player(PlayerId(0), |p| {
            p.ammo = AmmoAccount {
                credit: 250_000,
                paid: [1, 2, 3],
                paused_ticks: 4,
                paused_at: 0,
                fired: [7; crate::supply::fired_class::COUNT],
                refilled: [5, 6, 7],
            };
        }));
        let w = World::from_state(w.state.clone(), 1).unwrap();
        let loaded = World::load(&w.save(), 2).unwrap();
        assert_eq!(loaded.state(), w.state());
        assert_eq!(loaded.full_state_hash(), w.full_state_hash());
        let AmmoAccount {
            credit: _,
            paid: _,
            paused_ticks: _,
            paused_at: _,
            fired: _,
            refilled: _,
        } = AmmoAccount::default();
        let crate::items::Kit {
            items: _,
            slot: _,
            weapon: _,
            throws: _,
            resupply: _,
            windup: _,
            rounds: _,
            on_sidearm: _,
            refill: _,
        } = crate::items::Kit::default();
        type StateEdit = Box<dyn Fn(&mut SimState)>;
        fn acct(s: &mut SimState) -> &mut AmmoAccount {
            &mut s.game.as_mut().unwrap().players[0].ammo
        }
        let (li, gi) = (lead.index(), gunner.index());
        let base = w.full_state_hash();
        let edits: Vec<(&str, StateEdit)> = vec![
            (
                "kit.rounds",
                Box::new(move |s| s.entities.kit[li].rounds[0] -= 1),
            ),
            (
                "kit.on_sidearm",
                Box::new(move |s| s.entities.kit[gi].on_sidearm = false),
            ),
            (
                "kit.refill",
                Box::new(move |s| s.entities.kit[gi].refill += 1),
            ),
            ("ammo.credit", Box::new(|s| acct(s).credit += 1)),
            ("ammo.paid", Box::new(|s| acct(s).paid[2] += 1)),
            ("ammo.paused_ticks", Box::new(|s| acct(s).paused_ticks += 1)),
            ("ammo.paused_at", Box::new(|s| acct(s).paused_at += 1)),
            ("ammo.fired", Box::new(|s| acct(s).fired[9] += 1)),
            ("ammo.refilled", Box::new(|s| acct(s).refilled[1] += 1)),
            (
                "vehicle.supply_due",
                Box::new(move |s| {
                    s.game
                        .as_mut()
                        .unwrap()
                        .vehicles
                        .get_mut(&va)
                        .unwrap()
                        .supply_due ^= 1
                }),
            ),
        ];
        for (name, edit) in &edits {
            let mut st = w.state.clone();
            edit(&mut st);
            let w2 = World::from_state(st, 1).unwrap();
            assert_ne!(w2.full_state_hash(), base, "{name} is not hashed");
            assert_eq!(w2.full_state_hash(), w2.state_hash());
        }
        let bad: Vec<(StateEdit, &str)> = vec![
            (
                Box::new(move |s| s.entities.kit[li].rounds[0] = 60_000),
                "rounds are inconsistent",
            ),
            (
                Box::new(move |s| s.entities.kit[li].rounds[3] = 1),
                "rounds are inconsistent",
            ),
            (
                Box::new(move |s| s.entities.kit[li].on_sidearm = true),
                "rounds are inconsistent",
            ),
            (
                Box::new(move |s| s.entities.combat[gi].mag = 9),
                "rounds are inconsistent",
            ),
            (
                Box::new(move |s| s.entities.kit[gi].refill = 9_999),
                "kit is inconsistent",
            ),
            (
                Box::new(|s| acct(s).credit = 50_000_000),
                "ammunition account",
            ),
            (
                Box::new(|s| acct(s).paused_at = 1_000_000),
                "ammunition account",
            ),
            (
                Box::new(move |s| {
                    s.game
                        .as_mut()
                        .unwrap()
                        .vehicles
                        .get_mut(&va)
                        .unwrap()
                        .supply_due = 1
                }),
                "vehicle",
            ),
        ];
        for (edit, want) in bad {
            let bytes = save_edited(&w, edit);
            let e = load_err(&bytes);
            assert!(e.contains(want), "{want}: {e}");
        }
    }

    /// S4 review L4: the AI's per-type buy counter is hashed, saved, and its
    /// length checked on load (L3).
    #[test]
    fn ai_buy_counters_are_hashed_and_checked() {
        let (mut w, _) = flat_tank_world_with(|s| s.players[1].is_ai = true);
        let n = w.match_setup().unwrap().squad_types.len();
        {
            let gm = w.state.game.as_mut().unwrap();
            let ai = gm.ai[1].as_mut().expect("player 1 is an AI");
            ai.counters.bought = vec![0; n];
            ai.counters.bought[0] = 2;
        }
        let w = World::from_state(w.state.clone(), 1).unwrap();
        let loaded = World::load(&w.save(), 2).unwrap();
        assert_eq!(loaded.state(), w.state());
        let base = w.full_state_hash();
        let mut st = w.state.clone();
        st.game.as_mut().unwrap().ai[1]
            .as_mut()
            .unwrap()
            .counters
            .bought[0] += 1;
        let w2 = World::from_state(st, 1).unwrap();
        assert_ne!(w2.full_state_hash(), base, "ai.bought is not hashed");
        let mut st = w.state.clone();
        st.game.as_mut().unwrap().ai[1]
            .as_mut()
            .unwrap()
            .counters
            .bought
            .push(1);
        let bad = World::from_state(st, 1).unwrap();
        assert!(
            World::load(&bad.save(), 1).is_err(),
            "a counter per type at most"
        );
    }

    /// S5a Part B: a man's fire mode and the AI's refill trips and
    /// ambushes are hashed, saved and checked on load.
    #[test]
    fn s5a_fire_mode_and_ai_ammo_state_are_saved_hashed_and_checked() {
        use crate::unit::FireMode;
        let (mut w, va) = flat_tank_world_with(|s| s.players[1].is_ai = true);
        let t = w
            .match_setup()
            .unwrap()
            .squad_type_index("ww2:ger_grenadiergruppe")
            .unwrap();
        let g = w
            .spawn_squad_at(
                PlayerId(1),
                t,
                FxVec2::from_int(300, 500),
                FxVec2::new(Fx::ONE, Fx::ZERO),
            )
            .unwrap();
        let man = w.game().unwrap().squads[&g].members[0].index();
        w.state.entities.unit[man].fire = FireMode::Hold;
        w.state.tick = 100;
        {
            let ai = w.state.game.as_mut().unwrap().ai[1].as_mut().unwrap();
            ai.refill.insert(g, 40);
            ai.ambush.insert(
                g,
                crate::ai::AiAmbush {
                    vehicle: va,
                    since: 50,
                    spot: FxVec2::from_int(310, 500),
                    tubes: 2,
                    done: false,
                },
            );
            ai.counters.ambushes = 1;
        }
        let w = World::from_state(w.state.clone(), 1).unwrap();
        let loaded = World::load(&w.save(), 2).unwrap();
        assert_eq!(loaded.state(), w.state());
        assert_eq!(loaded.full_state_hash(), w.full_state_hash());
        type StateEdit = Box<dyn Fn(&mut SimState)>;
        fn ai(s: &mut SimState) -> &mut crate::ai::AiState {
            s.game.as_mut().unwrap().ai[1].as_mut().unwrap()
        }
        let base = w.full_state_hash();
        let edits: Vec<(&str, StateEdit)> = vec![
            (
                "unit.fire",
                Box::new(move |s| s.entities.unit[man].fire = FireMode::AtWill),
            ),
            (
                "ai.refill",
                Box::new(move |s| *ai(s).refill.get_mut(&g).unwrap() += 1),
            ),
            (
                "ai.ambush.since",
                Box::new(move |s| ai(s).ambush.get_mut(&g).unwrap().since += 1),
            ),
            (
                "ai.ambush.spot",
                Box::new(move |s| ai(s).ambush.get_mut(&g).unwrap().spot.x += Fx::ONE),
            ),
            (
                "ai.ambush.tubes",
                Box::new(move |s| ai(s).ambush.get_mut(&g).unwrap().tubes -= 1),
            ),
            (
                "ai.ambush.done",
                Box::new(move |s| ai(s).ambush.get_mut(&g).unwrap().done = true),
            ),
            (
                "ai.counters.ambushes",
                Box::new(|s| ai(s).counters.ambushes += 1),
            ),
        ];
        for (name, edit) in &edits {
            let mut st = w.state.clone();
            edit(&mut st);
            let w2 = World::from_state(st, 1).unwrap();
            assert_ne!(w2.full_state_hash(), base, "{name} is not hashed");
        }
        let bad: Vec<(StateEdit, &str)> = vec![
            // A vehicle (not a soldier) with a fire mode.
            (
                Box::new(move |s| s.entities.unit[va.index()].fire = FireMode::Hold),
                "unit",
            ),
            (
                Box::new(move |s| *ai(s).refill.get_mut(&g).unwrap() = 1_000),
                "AI",
            ),
            (
                Box::new(move |s| {
                    ai(s).refill.insert(SquadId(9_999), 0);
                }),
                "AI",
            ),
            (
                Box::new(move |s| ai(s).ambush.get_mut(&g).unwrap().since = 1_000),
                "AI",
            ),
        ];
        for (edit, want) in bad {
            let bytes = save_edited(&w, edit);
            let e = load_err(&bytes);
            assert!(e.contains(want), "{want}: {e}");
        }
    }

    #[test]
    fn launcher_escort_and_dig_profile_state_is_saved_hashed_and_checked() {
        use crate::game::{Launcher, Order};
        use crate::unit::UnitOrder;
        let (w, va, vb, r, t) = order_world();
        let g = w.game().unwrap();
        assert!(matches!(g.squads[&r].order, Order::Follow { vehicle } if vehicle == va));
        assert_eq!(g.vehicles[&va].followers.len(), g.squads[&r].members.len());
        let l = g.squads[&t].launcher.clone().unwrap();
        assert_eq!(l.attack, Some(vb));
        // The engineers are still walking to the line: an AT-ditch section
        // is recorded when its first pass is stamped, not on the order.
        assert!(g.at_ditches.is_empty());
        let digging = g
            .squads
            .values()
            .find(|s| matches!(s.order, Order::Dig { .. }))
            .unwrap();
        let Order::Dig { job, .. } = &digging.order else {
            unreachable!()
        };
        assert_eq!(job.profile, frontline_content::DigProfile::AtDitch);
        let loaded = World::load(&w.save(), 2).unwrap();
        assert_eq!(loaded.state(), w.state());
        assert_eq!(loaded.full_state_hash(), w.full_state_hash());

        // Every launcher field changes the hash; the destructure fails to
        // compile when a field is added and not listed.
        let Launcher {
            carrier: _,
            rounds: _,
            resupply: _,
            attack: _,
            aim_turret: _,
        } = l;
        type StateEdit = Box<dyn Fn(&mut SimState)>;
        fn game(s: &mut SimState) -> &mut MatchState {
            s.game.as_mut().unwrap()
        }
        fn launcher_of(s: &mut SimState, t: SquadId) -> &mut Launcher {
            game(s)
                .squads
                .get_mut(&t)
                .unwrap()
                .launcher
                .as_mut()
                .unwrap()
        }
        let base = w.full_state_hash();
        let edits: Vec<(&str, StateEdit)> = vec![
            (
                "carrier",
                Box::new(move |s| launcher_of(s, t).carrier = None),
            ),
            (
                "drops",
                Box::new(move |s| {
                    let g = game(s);
                    let w = g.setup.squad_types[g.squads[&t].type_idx as usize].weapon;
                    game(s)
                        .squads
                        .get_mut(&t)
                        .unwrap()
                        .drops
                        .push(crate::game::WeaponDrop {
                            weapon: w,
                            at: FxVec2::from_int(300, 300),
                            ticks: 3,
                            mag: 0,
                        })
                }),
            ),
            ("rounds", Box::new(move |s| launcher_of(s, t).rounds -= 1)),
            (
                "resupply",
                Box::new(move |s| launcher_of(s, t).resupply += 1),
            ),
            ("attack", Box::new(move |s| launcher_of(s, t).attack = None)),
            (
                "aim_turret",
                Box::new(move |s| launcher_of(s, t).aim_turret ^= true),
            ),
            (
                "follow",
                Box::new(move |s| {
                    game(s).squads.get_mut(&r).unwrap().order = Order::Follow { vehicle: vb }
                }),
            ),
            (
                "unit follow",
                Box::new(move |s| {
                    let m = game(s).squads[&r].members[0];
                    s.entities.unit[m.index()].order = UnitOrder::Follow { vehicle: vb };
                }),
            ),
            (
                "dig profile",
                Box::new(|s| {
                    for sq in game(s).squads.values_mut() {
                        if let Order::Dig { job, .. } = &mut sq.order {
                            job.profile = frontline_content::DigProfile::Trench;
                        }
                    }
                }),
            ),
        ];
        for (name, edit) in &edits {
            let mut st = w.state.clone();
            edit(&mut st);
            let w2 = World::from_state(st, 1).unwrap();
            assert_ne!(w2.full_state_hash(), base, "{name} is not hashed");
            assert_eq!(w2.full_state_hash(), w2.state_hash());
        }

        // Loads reject broken launcher and follow state.
        let bad: Vec<(StateEdit, &str)> = vec![
            (Box::new(move |s| launcher_of(s, t).rounds = 7), "launcher"),
            (
                Box::new(move |s| launcher_of(s, t).carrier = Some(va)),
                "launcher",
            ),
            (
                Box::new(move |s| launcher_of(s, t).attack = Some(va)),
                "launcher",
            ),
            (
                Box::new(move |s| {
                    let g = game(s);
                    let w = g.setup.squad_types[g.squads[&t].type_idx as usize].weapon;
                    game(s)
                        .squads
                        .get_mut(&t)
                        .unwrap()
                        .drops
                        .push(crate::game::WeaponDrop {
                            weapon: w,
                            at: FxVec2::from_int(300, 300),
                            ticks: 9999,
                            mag: 0,
                        })
                }),
                "crew weapon",
            ),
            (
                Box::new(move |s| {
                    game(s).squads.get_mut(&r).unwrap().launcher = Some(Launcher::default())
                }),
                "launcher",
            ),
            (
                Box::new(move |s| game(s).squads.get_mut(&t).unwrap().launcher = None),
                "launcher",
            ),
            (
                Box::new(move |s| {
                    game(s).squads.get_mut(&r).unwrap().order = Order::Follow { vehicle: vb }
                }),
                "follow",
            ),
            (
                Box::new(move |s| {
                    let m = game(s).squads[&r].members[0];
                    s.entities.unit[m.index()].order = UnitOrder::Follow { vehicle: vb };
                }),
                "unit order",
            ),
        ];
        for (edit, want) in bad {
            let bytes = save_edited(&w, edit);
            let e = load_err(&bytes);
            assert!(e.contains(want), "{want}: {e}");
        }
    }

    #[test]
    fn satchel_craters_can_breach_an_at_ditch() {
        // A 30 m AT ditch running north-south; satchel craters (the item's
        // radius and depth, levelling) at the ditch floor, 2 m apart, out
        // from the middle, until a tank has a way across.
        let (mut w, _) = flat_tank_world();
        let line = [FxVec2::from_int(600, 485), FxVec2::from_int(600, 515)];
        assert!(w.dig_at_ditch_now(&line, 1));
        let s = w.match_setup().unwrap().clone();
        let satchel = s.items.iter().find(|i| i.id == "ww2:m37_satchel").unwrap();
        let he = &satchel.he.as_ref().unwrap().blast;
        let (r, d): (Fx, Fx) = (he.crater_radius.into(), he.crater_depth.into());
        let mid = FxVec2::from_int(600, 500);
        let open = |w: &World| {
            let g = w.game().unwrap();
            let b = &g.vderived.block;
            !b.blocked_at(w.terrain(), mid)
        };
        assert!(!open(&w));
        let mut used = 0;
        for k in 0..16 {
            // 0, +2, −2, +4, −4 ... m along the ditch.
            let off = if k % 2 == 1 { k / 2 + 1 } else { -(k / 2) } * 2;
            let at = FxVec2::from_int(600, 500 + off);
            w.state.terrain.crater_cut(at, r, d, w.state.tick);
            w.refresh_hashes();
            used = k + 1;
            if open(&w) {
                break;
            }
        }
        assert!(open(&w), "16 satchels did not open the ditch");
        // About 12 m of ditch has to be filled: the 4 m grid cell keeps its
        // deepest point, and the block layer is dilated by one node.
        assert!((5..=9).contains(&used), "{used} satchels");
        // A straight drive across the gap touches no blocked node now.
        let b = &w.game().unwrap().vderived.block;
        assert!(!b.segment_blocked(
            w.terrain(),
            FxVec2::from_int(580, 500),
            FxVec2::from_int(620, 500)
        ));
    }
}
