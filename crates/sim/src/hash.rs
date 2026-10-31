//! State hash (§5.1 step 4, §5.11).
//!
//! * One xxh3-64 per chunk of [`CHUNK_SIZE`] entity ids and per chunk of
//!   projectile slots, kept in [`HashCache`] and recomputed only for chunks
//!   marked dirty since the last update (every tick, in the World/Hash stage).
//! * A "globals" hash over everything that is not per-chunk: config, tick,
//!   id allocator (high-water mark and free list), projectile free list,
//!   stats counters and the terrain.
//! * The combined hash is xxh3 over `globals ‖ entity chunks ‖ pool chunks`.
//!   It is published every K ticks; [`full_hash`] recomputes it from scratch
//!   and must always match the incremental value.
//!
//! Encoding is explicit little-endian bytes, independent of platform layout.

use std::cell::RefCell;

use xxhash_rust::xxh3::xxh3_64;

use crate::ecs::{CHUNK_SIZE, Entities, EntityKind};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::projectile::{Projectile, ProjectilePool};
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

/// Hash of entity chunk `c` (all columns of ids `c*256 .. c*256+256`).
pub(crate) fn entity_chunk(ents: &Entities, c: usize) -> u64 {
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

/// Hash of the terrain heights (immutable in M0).
pub(crate) fn terrain(state: &SimState) -> u64 {
    hash_with(|out| {
        (state.terrain.size_m() as u32).put(out);
        state.terrain.dim().put(out);
        for h in state.terrain.heights() {
            h.put(out);
        }
    })
}

/// Hash of all non-chunked state.
pub(crate) fn globals(state: &SimState, config_bytes: &[u8], terrain_hash: u64) -> u64 {
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
pub(crate) fn full_hash(state: &SimState, config_bytes: &[u8]) -> u64 {
    let ent: Vec<u64> = (0..state.entities.chunk_count())
        .map(|c| entity_chunk(&state.entities, c))
        .collect();
    let pool: Vec<u64> = (0..state.projectiles.chunk_count())
        .map(|c| pool_chunk(&state.projectiles, c))
        .collect();
    combine(globals(state, config_bytes, terrain(state)), &ent, &pool)
}

/// Cached per-chunk hashes with dirty flags. Derived data: rebuilt on load.
#[derive(Clone, Debug, Default)]
pub(crate) struct HashCache {
    pub entity: Vec<u64>,
    pub entity_dirty: Vec<bool>,
    pub pool: Vec<u64>,
    pub pool_dirty: Vec<bool>,
    pub terrain: u64,
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
            scratch: Vec::new(),
        }
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
