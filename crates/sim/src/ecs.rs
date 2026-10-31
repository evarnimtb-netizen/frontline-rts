//! Entity storage: a small custom struct-of-arrays store indexed by [`SimId`].
//!
//! # Why not `bevy_ecs`
//!
//! The sim needs three things that a general archetype ECS makes awkward:
//!
//! 1. **Stable, saved ids.** A [`SimId`] is a plain `u32` from an allocator
//!    with a LIFO free list, and that allocator is part of the saved and
//!    hashed state. Bevy's `Entity` ids are allocator-internal and are not
//!    meant to round-trip through a save file.
//! 2. **Fixed iteration order.** Every column is a `Vec` indexed by `SimId`,
//!    so iteration is always ascending id. Archetype iteration order shifts
//!    when components are added or removed.
//! 3. **Chunked hashing and parallelism.** Ids are grouped into fixed chunks
//!    of [`CHUNK_SIZE`]. The state hash keeps one xxh3 per chunk, and parallel
//!    stages hand out whole chunks (`par_chunks_mut`) so a worker writes only
//!    its own entities. Serialising the whole store is one serde derive.
//!
//! The cost is that unused slots keep default column values. A component set
//! is defined by [`EntityKind`], not by archetype. That suits an RTS where most
//! entities are soldiers, vehicles and structures with fixed component sets.
//!
//! Columns grow to the allocator's high-water mark and never shrink, so the
//! column length (and therefore the chunk count) is deterministic state.

use serde::{Deserialize, Serialize};

use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::SquadId;
use crate::items::Kit;
use crate::unit::Unit;

/// Ids per hash/parallel chunk.
pub const CHUNK_SIZE: usize = 256;

/// Stable simulation entity id. Reused through the allocator's free list.
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct SimId(pub u32);

impl SimId {
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn chunk(self) -> usize {
        self.0 as usize / CHUNK_SIZE
    }
}

/// Player slot (0..players).
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct PlayerId(pub u8);

/// `SimId` allocator. `next` is the high-water mark; freed ids go on a LIFO
/// stack and are reused first. Both are saved and hashed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdAllocator {
    next: u32,
    free: Vec<u32>,
}

impl IdAllocator {
    pub fn alloc(&mut self) -> SimId {
        match self.free.pop() {
            Some(i) => SimId(i),
            None => {
                let id = SimId(self.next);
                self.next = self.next.checked_add(1).expect("SimId space exhausted");
                id
            }
        }
    }

    pub fn free(&mut self, id: SimId) {
        debug_assert!(id.0 < self.next);
        self.free.push(id.0);
    }

    /// Number of ids ever handed out (= column length).
    #[inline]
    pub fn high_water(&self) -> u32 {
        self.next
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
    pub fn live(&self) -> u32 {
        self.next.saturating_sub(self.free.len() as u32)
    }

    /// Checks a loaded allocator: every free id is below `next` and listed
    /// once. A duplicate would hand the same id out twice.
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.free.len() > self.next as usize {
            return Err("id free list is longer than the id high-water mark");
        }
        let mut seen = vec![false; self.next as usize];
        for &i in &self.free {
            match seen.get_mut(i as usize) {
                Some(s) if !*s => *s = true,
                Some(_) => return Err("id free list contains a duplicate"),
                None => return Err("id free list contains an id above the high-water mark"),
            }
        }
        Ok(())
    }
}

/// Which component set an id carries. `Vacant` slots are free.
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
#[repr(u8)]
pub enum EntityKind {
    #[default]
    Vacant = 0,
    Soldier = 1,
    /// A dead soldier, kept for the presenter until `Combat::corpse` runs
    /// out (match mode). Not in squads, the spatial hash or `soldiers()`.
    Corpse = 2,
}

/// Position (z is terrain height for ground units) and heading.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Transform {
    pub pos: FxVec3,
    /// Heading in radians, counter-clockwise from +x.
    pub yaw: Fx,
}

/// Planar velocity in m/s.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Velocity {
    pub lin: FxVec2,
}

/// Walking state.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Mover {
    pub waypoint: FxVec2,
    /// Preferred speed in m/s.
    pub speed: Fx,
    /// Waypoint came from an order (not auto-picked).
    pub ordered: bool,
    /// Waypoints reached so far.
    pub legs: u32,
}

/// Small-arms state.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Weapon {
    /// Ticks until the next shot.
    pub cooldown: u16,
    pub shots: u32,
}

/// Infantry posture. Automatic: suppression, trenches and firing set it
/// each tick (`crate::combat`).
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
#[repr(u8)]
pub enum Stance {
    #[default]
    Stand = 0,
    Crouch = 1,
    Prone = 2,
}

/// Match-mode combat state of a soldier (S1 combat). Bench soldiers keep
/// the default, which is not hashed.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Combat {
    pub hp: u16,
    pub stance: Stance,
    /// Standing in a trench or crater at least 1 m below the base surface.
    pub in_trench: bool,
    /// Prone and unable to move (suppression at the pin threshold).
    pub pinned: bool,
    /// Suppression in thousandths of a point (0..=100_000).
    pub suppression: u32,
    /// Hit capsule height above the feet in the current posture (m).
    pub height: Fx,
    /// Capsule height while firing: on the fire step in a trench (m).
    pub fire_height: Fx,
    pub target: Option<SimId>,
    /// Rounds left in the magazine.
    pub mag: u16,
    /// Ticks spent standing still (weapon setup), saturating.
    pub still: u16,
    /// Ticks since the last shot, saturating.
    pub since_shot: u16,
    /// Ticks until the next target search.
    pub retarget: u16,
    /// Players that see this soldier: bit `p` for player slot `p`.
    pub seen_by: u16,
    /// Corpses: ticks left before the body is removed.
    pub corpse: u16,
}

/// SoA entity store. Columns are public for reading; mutate through
/// [`crate::World`] so hash dirty flags stay correct.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entities {
    pub(crate) alloc: IdAllocator,
    pub(crate) kind: Vec<EntityKind>,
    pub(crate) owner: Vec<PlayerId>,
    pub(crate) transform: Vec<Transform>,
    pub(crate) velocity: Vec<Velocity>,
    pub(crate) mover: Vec<Mover>,
    pub(crate) weapon: Vec<Weapon>,
    /// Squad a soldier belongs to (match mode; `None` for bench soldiers).
    pub(crate) squad: Vec<Option<SquadId>>,
    /// Combat state (match mode; default for bench soldiers).
    pub(crate) combat: Vec<Combat>,
    /// Soldier-level orders: stance order, detached flag and own order
    /// (match mode, S1b; default for bench soldiers, corpses and men in
    /// formation apart from the stance order).
    pub(crate) unit: Vec<Unit>,
    /// Thrown items carried and the throw being wound up (match mode, S1b;
    /// default for bench soldiers and corpses).
    pub(crate) kit: Vec<Kit>,
}

impl Entities {
    #[inline]
    pub fn allocator(&self) -> &IdAllocator {
        &self.alloc
    }
    #[inline]
    pub fn kind(&self) -> &[EntityKind] {
        &self.kind
    }
    #[inline]
    pub fn owner(&self) -> &[PlayerId] {
        &self.owner
    }
    #[inline]
    pub fn transform(&self) -> &[Transform] {
        &self.transform
    }
    #[inline]
    pub fn velocity(&self) -> &[Velocity] {
        &self.velocity
    }
    #[inline]
    pub fn mover(&self) -> &[Mover] {
        &self.mover
    }
    #[inline]
    pub fn weapon(&self) -> &[Weapon] {
        &self.weapon
    }
    #[inline]
    pub fn squad(&self) -> &[Option<SquadId>] {
        &self.squad
    }
    #[inline]
    pub fn combat(&self) -> &[Combat] {
        &self.combat
    }
    #[inline]
    pub fn unit(&self) -> &[Unit] {
        &self.unit
    }
    #[inline]
    pub fn kit(&self) -> &[Kit] {
        &self.kit
    }

    /// Column length (= allocator high-water mark).
    #[inline]
    pub fn len(&self) -> usize {
        self.kind.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.kind.is_empty()
    }

    #[inline]
    pub fn chunk_count(&self) -> usize {
        self.kind.len().div_ceil(CHUNK_SIZE)
    }

    #[inline]
    pub fn is_alive(&self, id: SimId) -> bool {
        self.kind
            .get(id.index())
            .is_some_and(|k| *k != EntityKind::Vacant)
    }

    /// Live soldiers in ascending id order.
    pub fn soldiers(&self) -> impl Iterator<Item = SimId> + '_ {
        self.kind
            .iter()
            .enumerate()
            .filter(|(_, k)| **k == EntityKind::Soldier)
            .map(|(i, _)| SimId(i as u32))
    }

    pub fn soldier_count(&self) -> usize {
        self.kind
            .iter()
            .filter(|k| **k == EntityKind::Soldier)
            .count()
    }

    pub(crate) fn spawn(&mut self, kind: EntityKind, owner: PlayerId) -> SimId {
        let id = self.alloc.alloc();
        let i = id.index();
        if i == self.kind.len() {
            self.kind.push(EntityKind::Vacant);
            self.owner.push(PlayerId::default());
            self.transform.push(Transform::default());
            self.velocity.push(Velocity::default());
            self.mover.push(Mover::default());
            self.weapon.push(Weapon::default());
            self.squad.push(None);
            self.combat.push(Combat::default());
            self.unit.push(Unit::default());
            self.kit.push(Kit::default());
        }
        debug_assert_eq!(self.kind.len(), self.alloc.high_water() as usize);
        debug_assert_eq!(self.kind[i], EntityKind::Vacant);
        // Vacant slots are kept at defaults (`despawn` resets them and `load`
        // checks it), but reset anyway so a reused id can never inherit state.
        self.kind[i] = kind;
        self.owner[i] = owner;
        self.transform[i] = Transform::default();
        self.velocity[i] = Velocity::default();
        self.mover[i] = Mover::default();
        self.weapon[i] = Weapon::default();
        self.squad[i] = None;
        self.combat[i] = Combat::default();
        self.unit[i] = Unit::default();
        self.kit[i] = Kit::default();
        id
    }

    /// Checks a loaded store: column lengths match the allocator, free ids
    /// are vacant, and every vacant slot holds default values. The state hash
    /// covers only the `kind` byte of a vacant slot, so this is what makes
    /// "same hash" imply "same behaviour" after a load.
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        self.alloc.validate()?;
        let n = self.alloc.high_water() as usize;
        let columns = [
            self.kind.len(),
            self.owner.len(),
            self.transform.len(),
            self.velocity.len(),
            self.mover.len(),
            self.weapon.len(),
            self.squad.len(),
            self.combat.len(),
            self.unit.len(),
            self.kit.len(),
        ];
        if columns.iter().any(|&l| l != n) {
            return Err("entity column lengths disagree with the allocator");
        }
        if self
            .alloc
            .free_list()
            .iter()
            .any(|&i| self.kind[i as usize] != EntityKind::Vacant)
        {
            return Err("id free list names a live entity");
        }
        let vacant = self
            .kind
            .iter()
            .filter(|k| **k == EntityKind::Vacant)
            .count();
        if vacant != self.alloc.free_list().len() {
            return Err("vacant slots and the id free list disagree");
        }
        let dirty_vacant = (0..n).any(|i| {
            self.kind[i] == EntityKind::Vacant
                && (self.owner[i] != PlayerId::default()
                    || self.transform[i] != Transform::default()
                    || self.velocity[i] != Velocity::default()
                    || self.mover[i] != Mover::default()
                    || self.weapon[i] != Weapon::default()
                    || self.squad[i].is_some()
                    || self.combat[i] != Combat::default()
                    || self.unit[i] != Unit::default()
                    || self.kit[i] != Kit::default())
        });
        if dirty_vacant {
            return Err("a vacant entity slot holds non-default values");
        }
        Ok(())
    }

    /// Frees `id` and resets its columns. Returns false if it was not alive.
    pub(crate) fn despawn(&mut self, id: SimId) -> bool {
        if !self.is_alive(id) {
            return false;
        }
        let i = id.index();
        self.kind[i] = EntityKind::Vacant;
        self.owner[i] = PlayerId::default();
        self.transform[i] = Transform::default();
        self.velocity[i] = Velocity::default();
        self.mover[i] = Mover::default();
        self.weapon[i] = Weapon::default();
        self.squad[i] = None;
        self.combat[i] = Combat::default();
        self.unit[i] = Unit::default();
        self.kit[i] = Kit::default();
        self.alloc.free(id);
        true
    }
}
