//! Capped projectile pool (§2.2 projectile pool, §5.7 integration).
//!
//! Projectiles are not entities: they live in a fixed-capacity slot array with
//! a LIFO free list. Slots are hashed in chunks of [`CHUNK_SIZE`] like entities.
//! Spawn requests that find the pool full are dropped deterministically (in
//! sorted request order) and counted; burst bundling (§5.7) comes later.

use serde::{Deserialize, Serialize};

use crate::ecs::{CHUNK_SIZE, SimId};
use crate::fx::{Fx, FxVec3};
use crate::terrain::Heightfield;

/// Ballistic substep rate: projectiles integrate at a fixed 60 Hz at every
/// tick rate, so trajectories match at 20 and 30 Hz.
pub const SUBSTEP_HZ: i32 = 60;

/// Gravity per substep (9.81 m/s² / 60).
const GRAVITY_PER_SUBSTEP: Fx = Fx::from_ratio(981, 100 * SUBSTEP_HZ as i64);

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Projectile {
    pub pos: FxVec3,
    /// m/s.
    pub vel: FxVec3,
    pub owner: SimId,
    /// Quadratic drag coefficient k (1/m): `a = -k |v| v`.
    pub drag: Fx,
    /// Substeps left before expiry.
    pub ttl: u16,
}

/// How a projectile left the pool.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ProjectileEnd {
    GroundHit,
    Expired,
    OutOfBounds,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ProjectilePool {
    slots: Vec<Option<Projectile>>,
    /// LIFO; initially `cap-1, ..., 1, 0` so slot 0 is used first.
    free: Vec<u32>,
}

impl ProjectilePool {
    pub fn new(cap: u32) -> ProjectilePool {
        ProjectilePool {
            slots: vec![None; cap as usize],
            free: (0..cap).rev().collect(),
        }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    #[inline]
    pub fn live(&self) -> usize {
        self.slots.len() - self.free.len()
    }

    #[inline]
    pub fn slots(&self) -> &[Option<Projectile>] {
        &self.slots
    }

    #[inline]
    pub fn free_list(&self) -> &[u32] {
        &self.free
    }

    #[cfg(test)]
    pub(crate) fn free_mut_for_test(&mut self) -> &mut Vec<u32> {
        &mut self.free
    }

    #[inline]
    pub fn chunk_count(&self) -> usize {
        self.slots.len().div_ceil(CHUNK_SIZE)
    }

    /// Checks a loaded pool: free slots are in range, listed once and empty,
    /// and every empty slot is on the free list.
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        let mut seen = vec![false; self.slots.len()];
        for &i in &self.free {
            match seen.get_mut(i as usize) {
                Some(s) if !*s => *s = true,
                Some(_) => return Err("projectile free list contains a duplicate"),
                None => return Err("projectile free list contains an out-of-range slot"),
            }
            if self.slots[i as usize].is_some() {
                return Err("projectile free list names a live slot");
            }
        }
        if self.slots.iter().filter(|s| s.is_none()).count() != self.free.len() {
            return Err("empty projectile slots and the free list disagree");
        }
        Ok(())
    }

    /// Places `p` in a free slot; `None` if the pool is full.
    pub(crate) fn spawn(&mut self, p: Projectile) -> Option<u32> {
        let slot = self.free.pop()?;
        debug_assert!(self.slots[slot as usize].is_none());
        self.slots[slot as usize] = Some(p);
        Some(slot)
    }

    /// Returns a slot (already cleared by the integrator) to the free list.
    pub(crate) fn release(&mut self, slot: u32) {
        debug_assert!(self.slots[slot as usize].is_none());
        self.free.push(slot);
    }

    pub(crate) fn slots_mut(&mut self) -> &mut [Option<Projectile>] {
        &mut self.slots
    }
}

/// Advances one projectile by one 1/60 s substep: semi-implicit Euler with
/// gravity and quadratic drag, then a swept-segment ground test.
/// Returns `Some(end)` if the projectile is gone.
pub fn substep(p: &mut Projectile, terrain: &Heightfield) -> Option<ProjectileEnd> {
    // v += (g + drag) * dt
    let speed = p.vel.length();
    let drag_factor = p.drag * speed; // 1/s
    let dv_drag = (p.vel * drag_factor).div_int(SUBSTEP_HZ);
    p.vel -= dv_drag;
    p.vel.z -= GRAVITY_PER_SUBSTEP;
    // x += v * dt, testing the swept segment.
    let next = p.pos + p.vel.div_int(SUBSTEP_HZ);
    if let Some(hit) = terrain.segment_hit(p.pos, next) {
        p.pos = hit;
        return Some(ProjectileEnd::GroundHit);
    }
    p.pos = next;
    if !terrain.contains(p.pos.xy()) {
        return Some(ProjectileEnd::OutOfBounds);
    }
    p.ttl = p.ttl.saturating_sub(1);
    if p.ttl == 0 {
        return Some(ProjectileEnd::Expired);
    }
    None
}
