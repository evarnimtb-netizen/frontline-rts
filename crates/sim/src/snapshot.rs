//! Render snapshot for the presenter (§6 "Snapshots").
//!
//! This is the only place where sim values become `f32`. Nothing here is read
//! back by the sim. Coordinates are sim axes (x east, y north, z up, metres);
//! the Godot bridge converts to its y-up frame.

use crate::ecs::{Entities, EntityKind, SimId};
use crate::projectile::ProjectilePool;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderSnapshot {
    /// Tick this snapshot shows (ticks completed).
    pub tick: u64,
    /// Per live soldier, in ascending id order: `[x, y, z, yaw]`.
    pub soldiers: Vec<[f32; 4]>,
    /// Ids matching `soldiers`, for interpolating between snapshots.
    pub soldier_ids: Vec<SimId>,
    /// Per live projectile, in slot order: `[x, y, z]`.
    pub projectiles: Vec<[f32; 3]>,
}

impl RenderSnapshot {
    /// Refills `self` in place (reuses allocations).
    pub(crate) fn fill(&mut self, tick: u64, ents: &Entities, pool: &ProjectilePool) {
        self.tick = tick;
        self.soldiers.clear();
        self.soldier_ids.clear();
        for (i, kind) in ents.kind().iter().enumerate() {
            if *kind == EntityKind::Soldier {
                let t = &ents.transform()[i];
                let [x, y, z] = t.pos.to_f32();
                self.soldiers.push([x, y, z, t.yaw.to_f32()]);
                self.soldier_ids.push(SimId(i as u32));
            }
        }
        self.projectiles.clear();
        self.projectiles
            .extend(pool.slots().iter().flatten().map(|p| p.pos.to_f32()));
    }
}
