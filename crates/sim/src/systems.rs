//! M0 per-chunk system kernels.
//!
//! Each function processes one chunk of [`CHUNK_SIZE`] ids (or projectile
//! slots). It writes only that chunk's slices and reads shared state
//! immutably, so [`crate::World`] can run chunks on any number of threads and
//! get identical results. Each returns whether anything in its chunk changed
//! (for hash dirty flags) and, where needed, events that the caller merges
//! serially in chunk order.

use crate::ecs::Combat;
use crate::ecs::{CHUNK_SIZE, Entities, EntityKind, Mover, SimId, Transform, Velocity, Weapon};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::projectile::{NO_WEAPON, Projectile, ProjectileEnd, substep};
use crate::rng::{Purpose, RngStream};
use crate::spatial::SpatialHash;
use crate::terrain::{DETAIL_PER_M, Heightfield};
use crate::vehicle::{MAX_VEHICLE_CIRCLES, Obstacles};
use crate::world::{SoldierParams, WeaponParams};

/// Hard cap on ORCA neighbours (§5.5).
pub const MAX_NEIGHBOURS: usize = 8;

// ---------------------------------------------------------------- Intent --

/// Picks a new seeded waypoint for soldiers that reached theirs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pick_waypoints(
    chunk: usize,
    kind: &[EntityKind],
    transform: &[Transform],
    mover: &mut [Mover],
    terrain: &Heightfield,
    p: &SoldierParams,
    seed: u64,
    tick: u64,
) -> bool {
    let base = chunk * CHUNK_SIZE;
    let arrive_sq = p.arrive_radius.mul(p.arrive_radius);
    let mut changed = false;
    for (j, m) in mover.iter_mut().enumerate() {
        if kind[j] != EntityKind::Soldier {
            continue;
        }
        let pos = transform[j].pos.xy();
        if (m.waypoint - pos).length_sq() > arrive_sq {
            continue;
        }
        let id = SimId((base + j) as u32);
        m.waypoint = random_waypoint(
            terrain,
            pos,
            p.wander_radius,
            seed,
            tick,
            id,
            Purpose::WAYPOINT,
        );
        m.ordered = false;
        m.legs = m.legs.wrapping_add(1);
        changed = true;
    }
    changed
}

/// Seeded point within `±radius` of `around`, clamped to the map.
pub(crate) fn random_waypoint(
    terrain: &Heightfield,
    around: FxVec2,
    radius: Fx,
    seed: u64,
    tick: u64,
    id: SimId,
    purpose: Purpose,
) -> FxVec2 {
    let mut r = RngStream::new(seed, tick, id, purpose);
    let dx = r.range_fx(-radius, radius);
    let dy = r.range_fx(-radius, radius);
    terrain.clamp(around + FxVec2::new(dx, dy))
}

// -------------------------------------------------------------- Movement --

/// One ORCA half-plane: velocities `v` with `det(dir, point - v) <= 0` are
/// allowed.
#[derive(Copy, Clone, Debug)]
struct OrcaLine {
    point: FxVec2,
    dir: FxVec2,
}

/// One ORCA half-plane against an obstacle at `rel_pos` (from the agent)
/// moving with relative velocity `rel_vel`, `combined_r` apart at contact.
/// The agent takes `share` of the avoidance (½ between soldiers, all of it
/// against a vehicle). `tie` picks the push direction for a coincident
/// obstacle at rest. `None`: the obstacle needs no line.
#[allow(clippy::too_many_arguments)]
#[inline]
fn orca_line(
    rel_pos: FxVec2,
    rel_vel: FxVec2,
    vel: FxVec2,
    combined_r: Fx,
    combined_r_sq: Fx,
    inv_tau: Fx,
    inv_dt: Fx,
    share: Fx,
    tie: bool,
) -> Option<OrcaLine> {
    let dist_sq = rel_pos.length_sq();
    let (dir, u);
    if dist_sq > combined_r_sq {
        // No collision yet: project onto the truncated velocity obstacle.
        let w = rel_vel - rel_pos * inv_tau;
        let w_len_sq = w.length_sq();
        let dot1 = w.dot(rel_pos);
        if dot1.is_negative() && dot1.mul(dot1) > combined_r_sq.mul(w_len_sq) {
            // Cut-off circle.
            let w_len = w.length();
            if w_len.is_zero() {
                return None;
            }
            let unit_w = w / w_len;
            dir = FxVec2::new(unit_w.y, -unit_w.x);
            u = unit_w * (combined_r.mul(inv_tau) - w_len);
        } else {
            // Legs.
            let leg = (dist_sq - combined_r_sq).sqrt();
            let (px, py) = (rel_pos.x, rel_pos.y);
            if rel_pos.det(w).raw() > 0 {
                dir = FxVec2::new(
                    px.mul(leg) - py.mul(combined_r),
                    px.mul(combined_r) + py.mul(leg),
                ) / dist_sq;
            } else {
                dir = -(FxVec2::new(
                    px.mul(leg) + py.mul(combined_r),
                    -px.mul(combined_r) + py.mul(leg),
                ) / dist_sq);
            }
            u = dir * rel_vel.dot(dir) - rel_vel;
        }
    } else {
        // Already overlapping: resolve within one tick.
        let w = rel_vel - rel_pos * inv_dt;
        let w_len = w.length();
        let unit_w = if w_len.is_zero() {
            // Coincident and at rest: split by id, deterministically.
            if tie {
                FxVec2::new(Fx::ONE, Fx::ZERO)
            } else {
                FxVec2::new(Fx::NEG_ONE, Fx::ZERO)
            }
        } else {
            w / w_len
        };
        dir = FxVec2::new(unit_w.y, -unit_w.x);
        u = unit_w * (combined_r.mul(inv_dt) - w_len);
    }
    Some(OrcaLine {
        point: vel + u * share,
        dir,
    })
}

/// Simplified fixed-point ORCA (after RVO2): builds one half-plane per
/// neighbour (up to [`MAX_NEIGHBOURS`] nearest, ties by id), then projects the
/// preferred velocity onto each violated half-plane in order and clamps to
/// max speed. This replaces RVO2's incremental 2D linear program with
/// sequential projection: cheaper, slightly less optimal, fully deterministic.
///
/// S3: in match worlds with vehicles, `obstacles` adds up to
/// [`MAX_VEHICLE_CIRCLES`] vehicle and wreck circles (two per hull, along
/// its axis) after the soldier neighbours. The soldier takes all of the
/// avoidance against them; vehicles never avoid soldiers.
///
/// Writes the new velocity of every id in the chunk into `out` (zero for
/// non-soldiers). Reads previous-tick positions and velocities only, and
/// writes no sim state (so it marks no hash chunk dirty; integration does).
pub(crate) fn plan_velocities(
    chunk: usize,
    out: &mut [FxVec2],
    ents: &Entities,
    spatial: &SpatialHash,
    obstacles: Option<&Obstacles>,
    p: &SoldierParams,
    tick_hz: i32,
) {
    let base = chunk * CHUNK_SIZE;
    let combined_r = p.radius.mul_int(2);
    let combined_r_sq = combined_r.mul(combined_r);
    let inv_tau = Fx::ONE / p.time_horizon;
    let inv_dt = Fx::from_int(tick_hz);
    let neigh_sq = p.neighbour_dist.mul(p.neighbour_dist);
    // Validated to 1..=8 by `SimConfig::validate`.
    let max_n = (p.max_neighbours as usize).clamp(1, MAX_NEIGHBOURS);

    for (j, out_v) in out.iter_mut().enumerate() {
        let i = base + j;
        if ents.kind[i] != EntityKind::Soldier {
            *out_v = FxVec2::ZERO;
            continue;
        }
        let id = SimId(i as u32);
        let pos = ents.transform[i].pos.xy();
        let vel = ents.velocity[i].lin;
        let mover = &ents.mover[i];

        // Preferred velocity: toward the waypoint at preferred speed.
        let to_wp = mover.waypoint - pos;
        let pref = if to_wp.length_sq() <= p.arrive_radius.mul(p.arrive_radius) {
            FxVec2::ZERO
        } else {
            to_wp.normalize_or_zero() * mover.speed
        };

        // Nearest neighbours, sorted by (dist², id).
        let mut near: [(u64, SimId); MAX_NEIGHBOURS] =
            [(u64::MAX, SimId(u32::MAX)); MAX_NEIGHBOURS];
        let mut n = 0usize;
        spatial.for_each_near(pos, p.neighbour_dist, |other| {
            if other == id {
                return;
            }
            let d = ents.transform[other.index()].pos.xy() - pos;
            let dsq = d.length_sq_raw();
            if dsq >= (neigh_sq.raw() as u64) << Fx::FRAC_BITS {
                return;
            }
            let key = (dsq, other);
            if n < max_n {
                near[n] = key;
                n += 1;
            } else if key < near[max_n - 1] {
                near[max_n - 1] = key;
            } else {
                return;
            }
            // Insertion step to keep `near[..n]` sorted.
            let mut k = n - 1;
            while k > 0 && near[k] < near[k - 1] {
                near.swap(k, k - 1);
                k -= 1;
            }
        });

        let mut lines = [OrcaLine {
            point: FxVec2::ZERO,
            dir: FxVec2::ZERO,
        }; MAX_NEIGHBOURS + MAX_VEHICLE_CIRCLES];
        let mut nl = 0usize;
        for &(_, other) in &near[..n] {
            let rel_pos = ents.transform[other.index()].pos.xy() - pos;
            let rel_vel = vel - ents.velocity[other.index()].lin;
            if let Some(line) = orca_line(
                rel_pos,
                rel_vel,
                vel,
                combined_r,
                combined_r_sq,
                inv_tau,
                inv_dt,
                Fx::HALF,
                id < other,
            ) {
                lines[nl] = line;
                nl += 1;
            }
        }
        // A man in a trench stays put while a hull crosses over him (S3).
        if let Some(ob) = obstacles.filter(|_| !ents.combat[i].in_trench) {
            let mut circles = [(FxVec2::ZERO, Fx::ZERO, FxVec2::ZERO); MAX_VEHICLE_CIRCLES];
            let k = ob.near(pos, p.neighbour_dist, &mut circles);
            for &(c, r, v) in &circles[..k] {
                let cr = p.radius + r;
                if let Some(line) = orca_line(
                    c - pos,
                    vel - v,
                    vel,
                    cr,
                    cr.mul(cr),
                    inv_tau,
                    inv_dt,
                    Fx::ONE,
                    true,
                ) {
                    lines[nl] = line;
                    nl += 1;
                }
            }
        }

        let mut v = pref;
        for line in &lines[..nl] {
            if line.dir.det(line.point - v).raw() > 0 {
                v = line.point + line.dir * line.dir.dot(v - line.point);
            }
        }
        *out_v = v.clamp_length(mover.speed);
    }
}

/// Grades up to this (percent) on detail ground do not slow a soldier.
const STEEP_FREE_PCT: i64 = 60;
/// Slowest pace on steep detail ground (percent of the planned speed).
const STEEP_MIN_PCT: i64 = 15;

/// Speed factor (percent) for a soldier at `xy` moving along `v` on detail
/// ground: 100 up to [`STEEP_FREE_PCT`] grade over the next 0.25 m, then
/// falling as `100 · 100 / (100 + 2 · (grade − 60))`, at least
/// [`STEEP_MIN_PCT`]. A 1.6 m trench wall (~400 %) is climbed at ~15 %.
fn steep_pct(terrain: &Heightfield, xy: FxVec2, v: FxVec2) -> i64 {
    let ahead = terrain.clamp(xy + v.normalize_or_zero().div_int(DETAIL_PER_M));
    let dh = (terrain.height_at(ahead).raw() as i64 - terrain.height_at(xy).raw() as i64).abs();
    // dh / 0.25 m in percent.
    let grade = (dh * 100 * DETAIL_PER_M as i64) >> Fx::FRAC_BITS;
    if grade <= STEEP_FREE_PCT {
        100
    } else {
        (100 * 100 / (100 + 2 * (grade - STEEP_FREE_PCT))).max(STEEP_MIN_PCT)
    }
}

/// Applies planned velocities: position, terrain height and heading. On
/// detail ground (dug or cratered) steep slopes slow the soldier down
/// ([`steep_pct`]); his stored velocity is the slowed one. Elsewhere the
/// planned velocity is used as is. Pinned soldiers (match mode) stay put.
pub(crate) fn integrate_soldiers(
    kind: &[EntityKind],
    transform: &mut [Transform],
    velocity: &mut [Velocity],
    combat: &[Combat],
    planned: &[FxVec2],
    terrain: &Heightfield,
    tick_hz: i32,
) -> bool {
    let mut changed = false;
    let min_turn_speed_sq = Fx::from_milli(50).mul(Fx::from_milli(50));
    for j in 0..kind.len() {
        if kind[j] != EntityKind::Soldier {
            continue;
        }
        changed = true;
        let mut v = if combat[j].pinned {
            FxVec2::ZERO
        } else {
            planned[j]
        };
        let t = &mut transform[j];
        let from = t.pos.xy();
        if v != FxVec2::ZERO && terrain.has_detail_at(from) {
            let pct = steep_pct(terrain, from, v);
            if pct < 100 {
                v = v.mul_int(pct as i32).div_int(100);
            }
        }
        velocity[j].lin = v;
        let xy = terrain.clamp(from + v.div_int(tick_hz));
        t.pos = xy.extend(terrain.height_at(xy));
        if v.length_sq() > min_turn_speed_sq {
            t.yaw = Fx::atan2(v.y, v.x);
        }
    }
    changed
}

// --------------------------------------------------------------- Weapons --

/// A projectile spawn request; merged serially in `(shooter, seq)` order.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct FireRequest {
    pub shooter: SimId,
    pub seq: u16,
    pub pos: FxVec3,
    pub vel: FxVec3,
    /// Weapon index, or `NO_WEAPON` (bench).
    pub weapon: u16,
    pub player: u8,
    /// Lifetime in substeps (match rounds; bench uses its config).
    pub ttl: u16,
    /// A shell (mortar, main gun, rocket): spawned first, may use the
    /// reserved pool slots.
    pub he: bool,
    /// S3: the round (index into `MatchSetup::ammo`) of a main-gun shell
    /// or rocket, `NO_AMMO` otherwise.
    pub ammo: u16,
}

/// Counts down weapon cooldowns and fires seeded shots when ready.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fire_weapons(
    chunk: usize,
    kind: &[EntityKind],
    transform: &[Transform],
    weapon: &mut [Weapon],
    p: &WeaponParams,
    seed: u64,
    tick: u64,
    out: &mut Vec<FireRequest>,
) -> bool {
    let base = chunk * CHUNK_SIZE;
    let mut changed = false;
    for (j, w) in weapon.iter_mut().enumerate() {
        if kind[j] != EntityKind::Soldier {
            continue;
        }
        changed = true;
        if w.cooldown > 0 {
            w.cooldown -= 1;
            continue;
        }
        let id = SimId((base + j) as u32);
        let mut r = RngStream::new(seed, tick, id, Purpose::WEAPON_SHOT);
        let yaw = transform[j].yaw + r.range_fx(-p.spread, p.spread);
        let elev = r.range_fx(p.elevation_min, p.elevation_max);
        let speed = r.range_fx(p.muzzle_speed_min, p.muzzle_speed_max);
        let (sy, cy) = yaw.sin_cos();
        let (se, ce) = elev.sin_cos();
        let horiz = speed.mul(ce);
        let vel = FxVec3::new(horiz.mul(cy), horiz.mul(sy), speed.mul(se));
        let mut pos = transform[j].pos;
        pos.z += p.muzzle_height;
        out.push(FireRequest {
            shooter: id,
            seq: 0,
            pos,
            vel,
            weapon: NO_WEAPON,
            player: 0,
            ammo: crate::projectile::NO_AMMO,
            ttl: 0,
            he: false,
        });
        w.shots = w.shots.wrapping_add(1);
        let mut rc = RngStream::new(seed, tick, id, Purpose::WEAPON_COOLDOWN);
        w.cooldown = rc.range_i32(p.cooldown_min as i32, p.cooldown_max as i32) as u16;
    }
    changed
}

/// Builds a pool entry from a request.
pub(crate) fn projectile_from(req: &FireRequest, p: &WeaponParams) -> Projectile {
    Projectile {
        pos: req.pos,
        vel: req.vel,
        owner: req.shooter,
        drag: p.drag,
        ttl: p.ttl_substeps.max(1),
        weapon: NO_WEAPON,
        player: 0,
        ammo: crate::projectile::NO_AMMO,
        origin: req.pos,
    }
}

// ------------------------------------------------------------- Ballistics --

/// Integrates one chunk of projectile slots for `substeps` 1/60 s substeps.
/// Finished projectiles are cleared in place and reported as
/// `(slot, end)` in slot order.
pub(crate) fn integrate_projectiles(
    chunk: usize,
    slots: &mut [Option<Projectile>],
    terrain: &Heightfield,
    substeps: u32,
    ended: &mut Vec<(u32, ProjectileEnd)>,
) -> bool {
    let base = chunk * CHUNK_SIZE;
    let mut changed = false;
    for (j, slot) in slots.iter_mut().enumerate() {
        let Some(p) = slot else { continue };
        changed = true;
        for _ in 0..substeps {
            if let Some(end) = substep(p, terrain) {
                ended.push(((base + j) as u32, end));
                *slot = None;
                break;
            }
        }
    }
    changed
}
