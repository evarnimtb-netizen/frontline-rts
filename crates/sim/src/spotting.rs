//! Spotting (S1 "Spotting", §5.6 Perception, §2.2 ray budget).
//!
//! Visibility is per player: bit `p` of a soldier's `Combat::seen_by` says
//! player `p` sees him. Soldiers only engage targets their player sees, and
//! the snapshot carries the bits so the presenter can draw fog of war.
//!
//! Each tick, for every player (in parallel, one task per player, reading
//! only the previous tick's state):
//!
//! 1. the player's soldiers become observers, each with an eye point and a
//!    sight range from his squad type;
//! 2. a rotating cursor walks the enemy soldiers and corpses (checked like
//!    a prone man, so a body is drawn only where the player can see it) in
//!    ascending id from where it stopped last tick. A tick checks at most
//!    `enemies / spot_period` targets (rounded up), so every enemy is
//!    re-checked at least once per period, and stops early when the
//!    player's ray budget would run out;
//!    the cursor then resumes there next tick, so a dense front slows the
//!    cadence identically on every client;
//! 3. a check scales each observer's range by the target's concealment
//!    (prone, in a trench, in woods; a recent shot replaces all three with
//!    `firing_spot_pct`) and casts one LOS ray (`Heightfield::line_of_sight`,
//!    against the combined surface including dug terrain) from each of the
//!    nearest `spot_observers` observers in range, nearest first, to the
//!    target's head; the first clear ray spots him. A ray through a smoke
//!    cloud counts as blocked (`crate::items`). No observer in range, or
//!    all rays blocked, hides him. The result holds until his next check.
//!
//! The per-player results are applied serially in player order. A
//! crouched man in a trench has his head below the lip, so flat rays never
//! reach it: he is hidden until he rises to fire. An observer in a trench
//! looks over the parapet from his fire-step eye height.

use crate::ecs::{Entities, EntityKind, PlayerId, SimId, Stance};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::MatchState;
use crate::terrain::Heightfield;

/// Eye (and head check point) below the top of the hit capsule (mm).
pub(crate) const EYE_DROP_MM: i32 = 100;

/// One player's spotting result for a tick.
#[derive(Clone, Debug, Default)]
pub(crate) struct SpotPass {
    /// `(target, visible)` per checked target, ascending check order.
    pub changes: Vec<(SimId, bool)>,
    /// Cursor for the next tick (an entity index).
    pub cursor: u32,
    pub rays: u32,
    /// Scratch: observers `(id, xy, eye, sight)`.
    observers: Vec<(SimId, FxVec2, FxVec3, Fx)>,
}

/// Head point of a soldier: the top of his capsule less the eye drop.
#[inline]
pub(crate) fn head(pos: FxVec3, height: Fx) -> FxVec3 {
    FxVec3::new(pos.x, pos.y, pos.z + height - Fx::from_milli(EYE_DROP_MM))
}

/// Runs player `p`'s spotting pass for this tick into `out`.
pub(crate) fn spot(
    p: PlayerId,
    ents: &Entities,
    terrain: &Heightfield,
    game: &MatchState,
    tick: u64,
    out: &mut SpotPass,
) {
    let setup = game.setup();
    let rules = &setup.rules.combat;
    let n = ents.len();
    let cursor = game.spot_cursor[p.0 as usize] as usize;
    out.changes.clear();
    out.rays = 0;
    out.cursor = if n == 0 { 0 } else { (cursor % n) as u32 };
    out.observers.clear();
    let mut enemies = 0u32;
    for i in 0..n {
        let kind = ents.kind[i];
        if ents.owner[i] != p {
            enemies += matches!(kind, EntityKind::Soldier | EntityKind::Corpse) as u32;
            continue;
        }
        if kind != EntityKind::Soldier {
            continue;
        }
        let id = SimId(i as u32);
        let c = &ents.combat[i];
        let Some(sq) = ents.squad[i].and_then(|s| game.squad(s)) else {
            continue;
        };
        let sight = Fx::from(setup.squad_types[sq.type_idx as usize].sight);
        let pos = ents.transform[i].pos;
        // In a trench he looks over the parapet from the fire step.
        let eye_h = if c.in_trench { c.fire_height } else { c.height };
        out.observers.push((id, pos.xy(), head(pos, eye_h), sight));
    }
    if enemies == 0 || n == 0 {
        return;
    }
    let quota = enemies.div_ceil(rules.spot_period_ticks.max(1));
    let k_max = rules.spot_observers.max(1) as usize;
    let budget = rules.spot_rays_per_tick;
    let mut checked = 0u32;
    // Nearest observers in range: (dist², id, eye), sorted.
    let mut near: Vec<(u64, SimId, FxVec3)> = Vec::with_capacity(k_max + 1);
    let start = out.cursor as usize;
    for step in 0..n {
        let i = (start + step) % n;
        let corpse = ents.kind[i] == EntityKind::Corpse;
        if !(corpse || ents.kind[i] == EntityKind::Soldier) || ents.owner[i] == p {
            continue;
        }
        if checked == quota || out.rays + k_max as u32 > budget {
            out.cursor = i as u32;
            return;
        }
        checked += 1;
        out.cursor = ((i + 1) % n) as u32;
        let c = &ents.combat[i];
        let pos = ents.transform[i].pos;
        let pct = if !corpse && (c.since_shot as u32) < rules.reveal_ticks {
            rules.firing_spot_pct
        } else {
            let mut pct = 100;
            if c.stance == Stance::Prone {
                pct = pct * rules.prone_spot_pct / 100;
            }
            if c.in_trench {
                pct = pct * rules.trench_spot_pct / 100;
            }
            if terrain.woods_at(pos.xy()) {
                pct = pct * rules.woods_spot_pct / 100;
            }
            pct
        };
        near.clear();
        for &(oid, oxy, eye, sight) in &out.observers {
            let range = Fx::from_raw((sight.raw() as i64 * pct as i64 / 100) as i32);
            let d = (pos.xy() - oxy).length_sq_raw();
            let r = range.raw() as u64;
            if d > r * r {
                continue;
            }
            let key = (d, oid, eye);
            if near.len() == k_max {
                if (key.0, key.1) >= (near[k_max - 1].0, near[k_max - 1].1) {
                    continue;
                }
                near.pop();
            }
            let at = near.partition_point(|e| (e.0, e.1) < (key.0, key.1));
            near.insert(at, key);
        }
        let target = head(pos, c.height);
        let mut seen = false;
        for &(_, oid, eye) in &near {
            out.rays += 1;
            if terrain.line_of_sight(eye, target)
                && !game.smoke_blocks(eye, target, tick, (SimId(i as u32), oid))
            {
                seen = true;
                break;
            }
        }
        if seen != (c.seen_by & (1 << p.0) != 0) {
            out.changes.push((SimId(i as u32), seen));
        }
    }
}
