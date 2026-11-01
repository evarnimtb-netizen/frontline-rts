//! State hash (§5.1 step 4, §5.11).
//!
//! * One xxh3-64 per chunk of [`CHUNK_SIZE`] entity ids and per chunk of
//!   projectile slots, kept in [`HashCache`] and recomputed only for chunks
//!   marked dirty since the last update (every tick, in the World/Hash stage).
//! * One xxh3-64 per terrain detail tile, recomputed only for tiles the
//!   world drained from the terrain's dirty list this tick. The base tier
//!   is hashed once. The terrain hash is the base hash alone while there
//!   are no tiles (so bench hashes are unchanged), else xxh3 over the base
//!   hash, pool cap and drop count, and the tile hashes in pool order.
//! * A "globals" hash over everything that is not per-chunk: config, tick,
//!   id allocator (high-water mark and free list), projectile free list,
//!   stats counters, the terrain hash and, in match mode, the match setup
//!   bytes and the whole [`MatchState`] (players, squads, queues, S3
//!   vehicle states and AT-ditch lines: small, so it is simply re-hashed
//!   at every publish).
//! * In match mode each entity also hashes its `squad` link, its combat
//!   state, its soldier-level order state (`unit`) and its items (`kit`),
//!   and match projectiles
//!   hash their weapon and player. Bench worlds
//!   skip those (always default there, checked on load), so their hashes
//!   are unchanged by match mode.
//! * The combined hash is xxh3 over `globals ‖ entity chunks ‖ pool chunks`.
//!   It is published every K ticks; [`full_hash`] recomputes it from scratch
//!   and must always match the incremental value.
//!
//! Encoding is explicit little-endian bytes, independent of platform layout.

use std::cell::RefCell;

use xxhash_rust::xxh3::xxh3_64;

use crate::dig::DigJob;
use crate::ecs::{CHUNK_SIZE, Combat, Entities, EntityKind, PlayerId, SimId};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{MatchState, Order, Outcome, PathReq};
use crate::items::Kit;
use crate::projectile::{NO_WEAPON, Projectile, ProjectilePool};
use crate::terrain::{DetailTile, Heightfield};
use crate::unit::{Unit, UnitOrder};
use crate::world::SimState;

/// Explicit byte encoding for hashing.
pub(crate) trait StableHash {
    fn put(&self, out: &mut Vec<u8>);
}

impl StableHash for u8 {
    fn put(&self, out: &mut Vec<u8>) {
        out.push(*self);
    }
}
impl StableHash for bool {
    fn put(&self, out: &mut Vec<u8>) {
        out.push(*self as u8);
    }
}
impl StableHash for u16 {
    fn put(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_le_bytes());
    }
}
impl StableHash for u32 {
    fn put(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_le_bytes());
    }
}
impl StableHash for u64 {
    fn put(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_le_bytes());
    }
}
impl StableHash for Fx {
    fn put(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.raw().to_le_bytes());
    }
}
impl StableHash for FxVec2 {
    fn put(&self, out: &mut Vec<u8>) {
        self.x.put(out);
        self.y.put(out);
    }
}
impl StableHash for FxVec3 {
    fn put(&self, out: &mut Vec<u8>) {
        self.x.put(out);
        self.y.put(out);
        self.z.put(out);
    }
}
impl StableHash for Projectile {
    fn put(&self, out: &mut Vec<u8>) {
        self.pos.put(out);
        self.vel.put(out);
        self.owner.0.put(out);
        self.drag.put(out);
        self.ttl.put(out);
        if self.weapon != NO_WEAPON {
            self.weapon.put(out);
            self.player.put(out);
        }
    }
}
impl StableHash for Option<SimId> {
    fn put(&self, out: &mut Vec<u8>) {
        self.map_or(u32::MAX, |s| s.0).put(out);
    }
}
impl StableHash for Combat {
    fn put(&self, out: &mut Vec<u8>) {
        self.hp.put(out);
        (self.stance as u8).put(out);
        self.in_trench.put(out);
        self.pinned.put(out);
        self.suppression.put(out);
        self.height.put(out);
        self.fire_height.put(out);
        self.target.put(out);
        self.mag.put(out);
        self.still.put(out);
        self.since_shot.put(out);
        self.retarget.put(out);
        self.seen_by.put(out);
        self.corpse.put(out);
    }
}

impl StableHash for Unit {
    fn put(&self, out: &mut Vec<u8>) {
        (self.stance as u8).put(out);
        self.detached.put(out);
        (self.order.kind() as u8).put(out);
        match self.order {
            UnitOrder::Idle | UnitOrder::Retreat => {}
            UnitOrder::Move { target } | UnitOrder::AttackMove { target } => target.put(out),
            UnitOrder::Dig { job } => job.put(out),
        }
        let n = &self.nav;
        (n.phase as u8).put(out);
        n.goal.put(out);
        n.path.put(out);
        n.next.put(out);
        n.partial.put(out);
        n.facing.put(out);
        n.phase_ticks.put(out);
    }
}

impl StableHash for Kit {
    fn put(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.items);
        self.leader.put(out);
        self.throws.put(out);
        self.resupply.put(out);
        match self.windup {
            None => 0xFFu8.put(out),
            Some(w) => {
                (w.kind as u8).put(out);
                w.target.put(out);
                w.ticks.put(out);
            }
        }
    }
}

fn put_dig(line: &[FxVec2], job: &DigJob, out: &mut Vec<u8>) {
    line.put(out);
    (job.front as u8).put(out);
    (job.segments.len() as u32).put(out);
    for g in &job.segments {
        g.from.put(out);
        g.to.put(out);
        g.work.put(out);
        g.passes.put(out);
        g.digger.map_or(u32::MAX, |d| d.0).put(out);
    }
}

thread_local! {
    static BUF: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(CHUNK_SIZE * 64));
}

/// Hashes the bytes written by `f` using a reused per-thread buffer.
fn hash_with(f: impl FnOnce(&mut Vec<u8>)) -> u64 {
    BUF.with_borrow_mut(|buf| {
        buf.clear();
        f(buf);
        xxh3_64(buf)
    })
}

impl<T: StableHash> StableHash for [T] {
    fn put(&self, out: &mut Vec<u8>) {
        (self.len() as u32).put(out);
        for v in self {
            v.put(out);
        }
    }
}
impl StableHash for Option<PlayerId> {
    fn put(&self, out: &mut Vec<u8>) {
        match self {
            None => 0xFFu8.put(out),
            Some(p) => p.0.put(out),
        }
    }
}

/// Hash of entity chunk `c` (all columns of ids `c*256 .. c*256+256`).
/// `squads` adds the squad and combat columns (match mode).
pub(crate) fn entity_chunk(ents: &Entities, c: usize, squads: bool) -> u64 {
    let lo = c * CHUNK_SIZE;
    let hi = (lo + CHUNK_SIZE).min(ents.len());
    hash_with(|out| {
        for i in lo..hi {
            let kind = ents.kind[i];
            (kind as u8).put(out);
            if kind == EntityKind::Vacant {
                continue;
            }
            ents.owner[i].0.put(out);
            let t = &ents.transform[i];
            t.pos.put(out);
            t.yaw.put(out);
            ents.velocity[i].lin.put(out);
            let m = &ents.mover[i];
            m.waypoint.put(out);
            m.speed.put(out);
            m.ordered.put(out);
            m.legs.put(out);
            let w = &ents.weapon[i];
            w.cooldown.put(out);
            w.shots.put(out);
            if squads {
                ents.squad[i].map_or(u32::MAX, |s| s.0).put(out);
                ents.combat[i].put(out);
                ents.unit[i].put(out);
                ents.kit[i].put(out);
            }
        }
    })
}

/// Hash of projectile slot chunk `c`.
pub(crate) fn pool_chunk(pool: &ProjectilePool, c: usize) -> u64 {
    let slots = pool.slots();
    let lo = c * CHUNK_SIZE;
    let hi = (lo + CHUNK_SIZE).min(slots.len());
    hash_with(|out| {
        for s in &slots[lo..hi] {
            match s {
                None => 0u8.put(out),
                Some(p) => {
                    1u8.put(out);
                    p.put(out);
                }
            }
        }
    })
}

/// Hash of the base tier's heights and flags (immutable).
pub(crate) fn terrain(state: &SimState) -> u64 {
    hash_with(|out| {
        (state.terrain.size_m() as u32).put(out);
        state.terrain.dim().put(out);
        for h in state.terrain.heights() {
            h.put(out);
        }
        // Bench terrain has no flags; hashing none keeps its hash unchanged.
        if !state.terrain.flags().is_empty() {
            out.extend_from_slice(state.terrain.flags());
        }
    })
}

/// Hash of one detail tile (coordinates, last-change tick, heights).
pub(crate) fn tile(t: &DetailTile) -> u64 {
    hash_with(|out| {
        let (x, y) = t.coords();
        x.put(out);
        y.put(out);
        t.changed_tick().put(out);
        for h in t.heights() {
            h.put(out);
        }
    })
}

/// Combined terrain hash from the base hash and the tile hashes (in pool
/// order). Equal to `base` while the detail tier is empty and untouched.
pub(crate) fn terrain_combined(base: u64, terrain: &Heightfield, tiles: &[u64]) -> u64 {
    let d = terrain.detail();
    if d.tiles().is_empty() && d.dropped() == 0 {
        return base;
    }
    hash_with(|out| {
        base.put(out);
        d.cap().put(out);
        d.dropped().put(out);
        (tiles.len() as u32).put(out);
        for h in tiles {
            h.put(out);
        }
    })
}

/// Encodes the whole match state (the setup goes in as bytes separately).
fn put_game(g: &MatchState, out: &mut Vec<u8>) {
    (g.players.len() as u32).put(out);
    for p in &g.players {
        p.manpower.put(out);
        p.tickets.put(out);
        p.surrendered.put(out);
        (p.call_ins.len() as u32).put(out);
        for c in &p.call_ins {
            c.squad_type.put(out);
            c.ready_tick.put(out);
        }
        p.kills.put(out);
        p.losses.put(out);
        p.defeat.map_or(0u8, |d| d as u8).put(out);
        p.point_ticks.put(out);
        p.income_acc.put(out);
        p.drain_acc.put(out);
    }
    g.next_squad.put(out);
    (g.squads.len() as u32).put(out);
    for (id, sq) in &g.squads {
        id.0.put(out);
        sq.type_idx.put(out);
        sq.owner.0.put(out);
        (sq.members.len() as u32).put(out);
        for m in &sq.members {
            m.0.put(out);
        }
        (sq.order.kind() as u8).put(out);
        match &sq.order {
            Order::Idle | Order::Retreat => {}
            Order::Move { target } | Order::AttackMove { target } => target.put(out),
            Order::Dig { line, job } => put_dig(line, job, out),
            Order::Barrage { target, rounds } => {
                target.put(out);
                rounds.put(out);
            }
        }
        let n = &sq.nav;
        (n.phase as u8).put(out);
        n.goal.put(out);
        n.path.put(out);
        n.next.put(out);
        n.partial.put(out);
        n.heading.put(out);
        n.facing.put(out);
        n.slots.put(out);
        n.phase_ticks.put(out);
        sq.focus.put(out);
    }
    (g.points.len() as u32).put(out);
    for c in &g.points {
        c.owner.put(out);
        c.progress.put(out);
        c.capturer.put(out);
        c.contested.put(out);
    }
    let q = &g.path_queue;
    (q.queues.len() as u32).put(out);
    for list in &q.queues {
        (list.len() as u32).put(out);
        for r in list {
            match r {
                PathReq::Squad(id) => {
                    0u8.put(out);
                    id.0.put(out);
                }
                PathReq::Unit(m) => {
                    1u8.put(out);
                    m.0.put(out);
                }
                PathReq::Vehicle { squad, avoid } => {
                    2u8.put(out);
                    squad.0.put(out);
                    avoid.unwrap_or(u32::MAX).put(out);
                }
            }
        }
    }
    q.cursor.put(out);
    q.debt.put(out);
    g.spot_cursor.put(out);
    match g.outcome {
        Outcome::Running => 0u8.put(out),
        Outcome::Won { winner } => {
            1u8.put(out);
            winner.0.put(out);
        }
        Outcome::Draw => 2u8.put(out),
    }
    g.next_unit_dig.put(out);
    (g.unit_digs.len() as u32).put(out);
    for (id, d) in &g.unit_digs {
        id.put(out);
        d.owner.0.put(out);
        put_dig(&d.line, &d.job, out);
    }
    g.next_thrown.put(out);
    (g.thrown.len() as u32).put(out);
    for t in &g.thrown {
        t.id.put(out);
        t.item.put(out);
        t.owner.0.put(out);
        t.thrower.0.put(out);
        t.from.put(out);
        t.vel.put(out);
        t.step.put(out);
        t.pos.put(out);
        t.landed.put(out);
        t.fuse.put(out);
    }
    g.next_smoke.put(out);
    (g.smoke.len() as u32).put(out);
    for c in &g.smoke {
        c.id.put(out);
        c.item.put(out);
        c.pos.put(out);
        c.age.put(out);
    }
    // S3 vehicles (`VehicleState` through postcard, as the AI state) and
    // the AT-ditch lines.
    let bytes = postcard::to_allocvec(&g.vehicles).expect("serialising vehicles cannot fail");
    (bytes.len() as u32).put(out);
    out.extend_from_slice(&bytes);
    (g.at_ditches.len() as u32).put(out);
    for l in &g.at_ditches {
        l.put(out);
    }
    // AI commander state: postcard's encoding is explicit and stable (varint
    // integers, fields in declaration order, maps in key order).
    for ai in &g.ai {
        match ai {
            None => 0u8.put(out),
            Some(ai) => {
                1u8.put(out);
                let bytes = postcard::to_allocvec(ai).expect("serialising AI state cannot fail");
                (bytes.len() as u32).put(out);
                out.extend_from_slice(&bytes);
            }
        }
    }
}

/// Hash of all non-chunked state. `setup_bytes` is the postcard encoding of
/// the match setup (empty in bench mode).
pub(crate) fn globals(
    state: &SimState,
    config_bytes: &[u8],
    setup_bytes: &[u8],
    terrain_hash: u64,
) -> u64 {
    hash_with(|out| {
        out.extend_from_slice(config_bytes);
        state.tick.put(out);
        let alloc = state.entities.allocator();
        alloc.high_water().put(out);
        (alloc.free_list().len() as u32).put(out);
        for id in alloc.free_list() {
            id.put(out);
        }
        (state.projectiles.capacity() as u32).put(out);
        (state.projectiles.free_list().len() as u32).put(out);
        for s in state.projectiles.free_list() {
            s.put(out);
        }
        let st = &state.stats;
        for v in [
            st.shots_fired,
            st.shots_dropped,
            st.ground_hits,
            st.expired,
            st.out_of_bounds,
            st.commands_applied,
            st.commands_rejected,
        ] {
            v.put(out);
        }
        terrain_hash.put(out);
        if let Some(g) = &state.game {
            for v in [
                st.soldier_hits,
                st.near_misses,
                st.explosions,
                st.deaths,
                st.throws,
            ] {
                v.put(out);
            }
            (setup_bytes.len() as u32).put(out);
            out.extend_from_slice(setup_bytes);
            put_game(g, out);
        }
    })
}

/// Combines globals and chunk hashes into the published state hash.
pub(crate) fn combine(globals: u64, entity_chunks: &[u64], pool_chunks: &[u64]) -> u64 {
    hash_with(|out| {
        globals.put(out);
        (entity_chunks.len() as u32).put(out);
        for h in entity_chunks {
            h.put(out);
        }
        (pool_chunks.len() as u32).put(out);
        for h in pool_chunks {
            h.put(out);
        }
    })
}

/// Full recompute from scratch (no cache).
pub(crate) fn full_hash(state: &SimState, config_bytes: &[u8], setup_bytes: &[u8]) -> u64 {
    let squads = state.game.is_some();
    let ent: Vec<u64> = (0..state.entities.chunk_count())
        .map(|c| entity_chunk(&state.entities, c, squads))
        .collect();
    let pool: Vec<u64> = (0..state.projectiles.chunk_count())
        .map(|c| pool_chunk(&state.projectiles, c))
        .collect();
    let tiles: Vec<u64> = state.terrain.detail().tiles().iter().map(tile).collect();
    let terrain = terrain_combined(terrain(state), &state.terrain, &tiles);
    combine(
        globals(state, config_bytes, setup_bytes, terrain),
        &ent,
        &pool,
    )
}

/// Cached per-chunk hashes with dirty flags. Derived data: rebuilt on load.
#[derive(Clone, Debug, Default)]
pub(crate) struct HashCache {
    pub entity: Vec<u64>,
    pub entity_dirty: Vec<bool>,
    pub pool: Vec<u64>,
    pub pool_dirty: Vec<bool>,
    /// Base tier hash (computed once).
    pub terrain: u64,
    /// Per detail tile, in pool order.
    pub tiles: Vec<u64>,
    /// Scratch list of dirty chunk indices.
    pub scratch: Vec<usize>,
}

impl HashCache {
    /// All chunks dirty; terrain hashed.
    pub fn new(state: &SimState) -> HashCache {
        let ne = state.entities.chunk_count();
        let np = state.projectiles.chunk_count();
        HashCache {
            entity: vec![0; ne],
            entity_dirty: vec![true; ne],
            pool: vec![0; np],
            pool_dirty: vec![true; np],
            terrain: terrain(state),
            tiles: state.terrain.detail().tiles().iter().map(tile).collect(),
            scratch: Vec::new(),
        }
    }

    /// Re-hashes the listed (dirty) tiles that still exist.
    pub fn update_tiles(&mut self, terrain: &Heightfield, dirty: &[(u16, u16)]) {
        let tiles = terrain.detail().tiles();
        self.tiles.resize(tiles.len(), 0);
        for &(x, y) in dirty {
            if let Some(i) = terrain.tile_index(x, y) {
                self.tiles[i] = tile(&tiles[i]);
            }
        }
    }

    /// The current combined terrain hash.
    pub fn terrain_hash(&self, terrain: &Heightfield) -> u64 {
        terrain_combined(self.terrain, terrain, &self.tiles)
    }

    /// Keeps the entity chunk list in step with the column length.
    pub fn resize_entities(&mut self, chunks: usize) {
        if chunks > self.entity.len() {
            self.entity.resize(chunks, 0);
            self.entity_dirty.resize(chunks, true);
        }
    }

    #[inline]
    pub fn mark_entity_chunk(&mut self, c: usize) {
        self.resize_entities(c + 1);
        self.entity_dirty[c] = true;
    }

    #[inline]
    pub fn mark_pool_slot(&mut self, slot: u32) {
        self.pool_dirty[slot as usize / CHUNK_SIZE] = true;
    }

    /// ORs per-chunk "changed" flags from a parallel stage into the dirty set.
    pub fn merge_entity_flags(&mut self, flags: &[bool]) {
        for (d, f) in self.entity_dirty.iter_mut().zip(flags) {
            *d |= *f;
        }
    }

    pub fn merge_pool_flags(&mut self, flags: &[bool]) {
        for (d, f) in self.pool_dirty.iter_mut().zip(flags) {
            *d |= *f;
        }
    }
}
