//! Combat (S1 "Combat", "Suppression", "HE"; §5.7, §5.8, §5.10.4).
//!
//! Match-mode work per tick, in `World::step` stage order (spotting is in
//! `crate::spotting`):
//!
//! * **Weapons** (stage 4).
//!   1. *Marks:* a serial pass lists every live soldier with his feet, aim
//!      point and `seen_by` bits, so the parallel part reads no column it
//!      also writes.
//!   2. *Squad focus* (serial, ascending squad id): each squad that may
//!      engage keeps its focus target while it stays seen and in range of
//!      the leader, else takes the nearest such enemy.
//!   3. *Aim and fire* (per entity chunk, in parallel): a soldier may engage
//!      by his squad's order (`Idle` and `AttackMove` always; `Move` and `Dig`
//!      only while he stands still; `Retreat` and mortar crews never). He
//!      drops a target that is dead, unseen or out of range, and searches
//!      again when he has none or once a second: the squad focus first, then
//!      the nearest seen enemies in range, at most [`LOS_TRIES`] LOS rays
//!      from his firing muzzle. Opening fire costs half a second of aiming.
//!      When his cooldown is up and his weapon is set up (MG: still for
//!      `setup_ticks`), he fires a real round at the target's aim point,
//!      compensated for drop, with angular error from the weapon spread
//!      scaled by movement, suppression or pin, and range.
//!      MGs ignore the focus and pick among the four nearest (seeded), so
//!      their bursts sweep and suppress a whole squad; set up, they lie
//!      prone.
//!   4. *Mortars* (serial): a team on a Barrage order, holding, set up and in
//!      range fires one shell per interval on a ballistic arc whose flight
//!      time grows with range (steep, about 60°), scattered by the spread.
//!      The order ends with its last round. Holding out of range, the team
//!      walks to 90 % of its range again, or drops the order when it already
//!      tried that spot (unreachable) or the target is inside the minimum.
//!   5. Requests are spawned in sorted order, HE shells first; small arms
//!      may not take the pool's last `HE_RESERVE` slots. Dropped rounds are
//!      counted (`Stats::shots_dropped`).
//! * **Ballistics** (stage 5, per projectile chunk): each 1/60 s substep of
//!   a small-arms round tests the swept segment against the hit capsules of
//!   enemy soldiers near it (spatial hash), then the terrain up to the
//!   nearest capsule hit, so earth (a parapet, a trench lip) blocks first.
//!   Enemies within `near_miss` of the segment (or of a ground impact) are
//!   near-missed. Shells only test terrain and burst where they land.
//! * **Damage** (stage 6, serial, in projectile slot order): hits take hit
//!   points and add suppression; near misses add the weapon's suppression
//!   (halved for a man crouched below cover). A burst damages soldiers whose
//!   body point is in clear line from the blast point (+0.3 m): full damage
//!   inside the lethal radius, falling linearly to zero at the damage radius;
//!   it suppresses to the suppression radius (halved behind earth) and
//!   carves a crater. Soldiers at 0 HP die in ascending id: they become
//!   corpses, leave their squad (squads left empty are deleted) and count
//!   as a loss and a kill.
//! * **Status** (stage 7, per entity chunk): suppression decays; stance is
//!   set (pinned → prone; in a trench → crouched below the lip, standing on
//!   the fire step while aiming; a set-up MG → prone; suppressed →
//!   crouched; else standing) with
//!   the matching capsule heights; the stillness and shot timers advance;
//!   corpses count down and are removed.
//!
//! No friendly fire in S1: rounds and blasts ignore the firing player's own
//! soldiers. Rounds have no drag, so the drop compensation is exact.

use frontline_content::WeaponClass;
use frontline_content::setup::{CombatSetup, WeaponSetup};

use crate::ecs::{
    Combat, Entities, EntityKind, PlayerId, SimId, Stance, Transform, Velocity, Weapon,
};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{MatchState, NavPhase, Order, SquadId};
use crate::hash::HashCache;
use crate::projectile::{Projectile, ProjectileEnd, SUBSTEP_HZ, advance, finish};
use crate::rng::{Purpose, RngStream};
use crate::spatial::SpatialHash;
use crate::systems::FireRequest;
use crate::terrain::Heightfield;
use crate::world::Stats;

/// Hit capsule heights by stance (mm).
pub const STAND_MM: i32 = 1750;
pub const CROUCH_MM: i32 = 1200;
pub const PRONE_MM: i32 = 400;
/// Capsule radius standing or crouched (mm).
const CAPSULE_R_MM: i32 = 250;
/// Prone: a horizontal capsule along the heading (radius, half length; mm).
const PRONE_R_MM: i32 = 200;
const PRONE_HALF_MM: i32 = 700;
/// The muzzle sits this far below the top of the firing capsule (mm).
const MUZZLE_DROP_MM: i32 = 150;
/// Aim point of a man on a fire step: this far below his top (mm).
const HEAD_AIM_MM: i32 = 150;
/// A soldier this far below the base surface is in a trench (mm).
pub const TRENCH_MM: i32 = 1000;
/// Below this speed a soldier counts as standing still (mm/s).
const STILL_MM_S: i32 = 300;
/// A burst's damage rays start this far above the blast point (mm).
const BLAST_LIFT_MM: i32 = 300;
/// Mortar muzzle height above the crew's feet (mm).
const SHELL_MUZZLE_MM: i32 = 800;
/// Most LOS rays per target search.
pub const LOS_TRIES: usize = 2;
/// MGs sweep: each search picks among this many nearest enemies (seeded),
/// ignoring the squad focus, so their fire suppresses a whole squad.
const SWEEP_POOL: usize = 4;
/// Mortar flight time: `sqrt(range · this)` seconds, at least
/// [`SHELL_MIN_FLIGHT_S`] (about a 60° arc at every range).
const SHELL_FLIGHT_MILLI: i32 = 350;
const SHELL_MIN_FLIGHT_S: i32 = 4;
/// A mortar team holding out of range walks to 90 % of its range again
/// unless that spot is within this distance of where it last walked (m).
const BARRAGE_RETRY_M: i32 = 20;
/// Crater depth roll, percent of the weapon's crater depth.
const CRATER_DEPTH_PCT: (i32, i32) = (75, 140);

/// 9.81 m/s².
const GRAVITY: Fx = Fx::from_ratio(981, 100);

/// Whether a squad's order lets its men open fire.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Engage {
    Never,
    /// Only men standing still.
    Still,
    Always,
}

pub(crate) fn engage(order: &Order) -> Engage {
    match order {
        Order::Retreat | Order::Barrage { .. } => Engage::Never,
        Order::Move { .. } | Order::Dig { .. } => Engage::Still,
        Order::Idle | Order::AttackMove { .. } => Engage::Always,
    }
}

/// A live soldier as other soldiers see him this tick.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Mark {
    pub id: SimId,
    pub owner: u8,
    pub pos: FxVec3,
    pub aim: FxVec3,
    pub seen_by: u16,
}

/// Where shooters aim: the middle of the upper body, the head and
/// shoulders of a man on a fire step, the middle of a prone man.
pub(crate) fn aim_point(pos: FxVec3, c: &Combat) -> FxVec3 {
    let dz = match c.stance {
        Stance::Prone => c.height.div_int(2),
        _ if c.in_trench => c.height - Fx::from_milli(HEAD_AIM_MM),
        _ => c.height.mul_int(3).div_int(5),
    };
    FxVec3::new(pos.x, pos.y, pos.z + dz)
}

/// Middle of the body (blast damage target).
fn body_point(pos: FxVec3, c: &Combat) -> FxVec3 {
    FxVec3::new(pos.x, pos.y, pos.z + c.height.div_int(2))
}

/// Lists live soldiers (`marks`, ascending id) and indexes them by id
/// (`mark_of`, `u32::MAX` for others).
pub(crate) fn build_marks(ents: &Entities, marks: &mut Vec<Mark>, mark_of: &mut Vec<u32>) {
    marks.clear();
    mark_of.clear();
    mark_of.resize(ents.len(), u32::MAX);
    for id in ents.soldiers() {
        let i = id.index();
        let c = &ents.combat[i];
        let pos = ents.transform[i].pos;
        mark_of[i] = marks.len() as u32;
        marks.push(Mark {
            id,
            owner: ents.owner[i].0,
            pos,
            aim: aim_point(pos, c),
            seen_by: c.seen_by,
        });
    }
}

#[inline]
fn range_sq(range: Fx) -> u64 {
    let r = range.raw().max(0) as u64;
    r * r
}

/// The mark of `t` if it is an enemy of `owner` that `owner` sees within
/// `range_sq` (Q32) of `from`.
fn valid_target<'a>(
    marks: &'a [Mark],
    mark_of: &[u32],
    t: SimId,
    owner: u8,
    from: FxVec2,
    range_sq: u64,
) -> Option<&'a Mark> {
    let k = *mark_of.get(t.index())?;
    let m = marks.get(k as usize)?;
    (m.owner != owner
        && m.seen_by & (1 << owner) != 0
        && (m.pos.xy() - from).length_sq_raw() <= range_sq)
        .then_some(m)
}

// ---------------------------------------------------------------- focus --

impl MatchState {
    /// Updates every squad's focus target (see the module docs).
    pub(crate) fn update_focus(&mut self, ents: &Entities, marks: &[Mark], mark_of: &[u32]) {
        let setup = &self.setup;
        for sq in self.squads.values_mut() {
            let ty = &setup.squad_types[sq.type_idx as usize];
            let w = &setup.weapons[ty.weapon as usize];
            if sq.members.is_empty() || w.he.is_some() || engage(&sq.order) == Engage::Never {
                sq.focus = None;
                continue;
            }
            let lead = ents.transform[sq.members[0].index()].pos.xy();
            let owner = sq.owner.0;
            let r = range_sq(w.range_max.into());
            if sq
                .focus
                .is_some_and(|f| valid_target(marks, mark_of, f, owner, lead, r).is_some())
            {
                continue;
            }
            sq.focus = marks
                .iter()
                .filter(|m| m.owner != owner && m.seen_by & (1 << owner) != 0)
                .map(|m| ((m.pos.xy() - lead).length_sq_raw(), m.id))
                .filter(|(d, _)| *d <= r)
                .min()
                .map(|(_, id)| id);
        }
    }
}

// ----------------------------------------------------------- aim + fire --

/// Read-only inputs of [`aim_and_fire`].
pub(crate) struct FireCtx<'a> {
    pub kind: &'a [EntityKind],
    pub owner: &'a [PlayerId],
    pub transform: &'a [Transform],
    pub velocity: &'a [Velocity],
    pub squad: &'a [Option<SquadId>],
    pub marks: &'a [Mark],
    pub mark_of: &'a [u32],
    pub game: &'a MatchState,
    pub terrain: &'a Heightfield,
    pub seed: u64,
    pub tick: u64,
}

#[inline]
fn is_still(v: &Velocity) -> bool {
    v.lin.length_sq_raw() <= range_sq(Fx::from_milli(STILL_MM_S))
}

/// Muzzle of a soldier in his firing posture.
#[inline]
fn muzzle(pos: FxVec3, c: &Combat) -> FxVec3 {
    FxVec3::new(
        pos.x,
        pos.y,
        pos.z + c.fire_height - Fx::from_milli(MUZZLE_DROP_MM),
    )
}

/// Target search for soldier `i`: the squad focus (if any and valid),
/// then the `pool` nearest other seen enemies in range. Candidates are tried
/// from index `rot` (wrapping) and the first with a clear line from `from`
/// wins; at most [`LOS_TRIES`] rays.
fn search(
    ctx: &FireCtx,
    i: usize,
    from: FxVec3,
    r: u64,
    focus: Option<SimId>,
    pool: usize,
    rot: usize,
) -> Option<SimId> {
    let owner = ctx.owner[i].0;
    let xy = from.xy();
    // Candidates `(dist², id, mark index)`: the focus, then the nearest.
    let mut cands = [(u64::MAX, SimId(u32::MAX), 0u32); 1 + SWEEP_POOL];
    let mut n = 0usize;
    if let Some(f) = focus
        && valid_target(ctx.marks, ctx.mark_of, f, owner, xy, r).is_some()
    {
        cands[0] = (0, f, ctx.mark_of[f.index()]);
        n = 1;
    }
    let first = n;
    let cap = first + pool.min(SWEEP_POOL);
    for (k, m) in ctx.marks.iter().enumerate() {
        if Some(m.id) == focus || m.owner == owner || m.seen_by & (1 << owner) == 0 {
            continue;
        }
        let d = (m.pos.xy() - xy).length_sq_raw();
        if d > r {
            continue;
        }
        let key = (d, m.id, k as u32);
        let mut j = if n < cap {
            n += 1;
            n - 1
        } else if key < cands[cap - 1] {
            cap - 1
        } else {
            continue;
        };
        cands[j] = key;
        // Keep the nearest part sorted.
        while j > first && cands[j] < cands[j - 1] {
            cands.swap(j, j - 1);
            j -= 1;
        }
    }
    (0..n.min(LOS_TRIES)).find_map(|k| {
        let (_, id, m) = cands[(rot + k) % n];
        ctx.terrain
            .line_of_sight(from, ctx.marks[m as usize].aim)
            .then_some(id)
    })
}

/// Triangular roll in `[-s, s]`.
#[inline]
fn tri(r: &mut RngStream, s: Fx) -> Fx {
    (r.range_fx(-s, s) + r.range_fx(-s, s)).div_int(2)
}

/// Spread of a shot in percent of the weapon's (movement, suppression or
/// pin, range).
fn spread_pct(rules: &CombatSetup, c: &Combat, moving: bool, dist: Fx, range: Fx) -> i64 {
    let mut pct: i64 = 100;
    if moving {
        pct = pct * rules.moving_spread_pct as i64 / 100;
    }
    if c.pinned {
        pct = pct * rules.pinned_spread_pct as i64 / 100;
    } else {
        pct = pct * (100 + (c.suppression / 1000) as i64) / 100;
    }
    let extra =
        rules.range_spread_pct as i64 * dist.raw().max(0) as i64 / range.raw().max(1) as i64;
    pct * (100 + extra) / 100
}

/// Velocity of a round fired from `from` at `aim` (drop-compensated, then
/// turned by triangular yaw and pitch errors of up to `spread` radians).
fn shot_velocity(from: FxVec3, aim: FxVec3, speed: Fx, spread: Fx, r: &mut RngStream) -> FxVec3 {
    let mut d = aim - from;
    let t = d.length().div(speed);
    d.z += GRAVITY.mul(t).mul(t).div_int(2);
    let dir = d.normalize_or_zero();
    let right = FxVec3::new(-dir.y, dir.x, Fx::ZERO).normalize_or_zero();
    let up = FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE);
    let turned = dir + right * tri(r, spread) + up * tri(r, spread);
    turned.normalize_or_zero() * speed
}

/// Ticks until the shot after shot number `shots` (1-based): the weapon's
/// milli-tick interval spread over successive shots, so the long-run rate is
/// exact (Bresenham on the soldier's shot counter, which is hashed).
pub(crate) fn interval_after(w: &WeaponSetup, shots: u32) -> u32 {
    let n = shots.max(1) as u64;
    let i = w.interval_milli as u64;
    (n * i / 1000 - (n - 1) * i / 1000) as u32
}

/// A whole number of ticks no shorter than the interval.
fn interval_ceil(w: &WeaponSetup) -> u32 {
    w.interval_milli.div_ceil(1000)
}

/// Substeps a round of `w` lives: 1.25 × its range in flight, plus two.
fn round_ttl(w: &WeaponSetup) -> u16 {
    let t = Fx::from(w.range_max)
        .mul_int(5)
        .div_int(4)
        .div(w.muzzle_speed.into());
    (t.mul_int(SUBSTEP_HZ).floor_int() + 2).clamp(2, u16::MAX as i32) as u16
}

/// Targeting and small-arms fire for one entity chunk (see the module
/// docs). Writes only this chunk's combat and weapon state; fire requests go
/// to `out` in id order.
pub(crate) fn aim_and_fire(
    chunk: usize,
    ctx: &FireCtx,
    combat: &mut [Combat],
    weapon: &mut [Weapon],
    out: &mut Vec<FireRequest>,
) -> bool {
    let base = chunk * crate::ecs::CHUNK_SIZE;
    let setup = ctx.game.setup();
    let rules = &setup.rules.combat;
    let hz = setup.tick_hz;
    let mut changed = false;
    for (j, (c, ws)) in combat.iter_mut().zip(weapon.iter_mut()).enumerate() {
        let i = base + j;
        if ctx.kind[i] != EntityKind::Soldier {
            continue;
        }
        let Some(sq) = ctx.squad[i].and_then(|s| ctx.game.squad(s)) else {
            continue;
        };
        changed = true;
        ws.cooldown = ws.cooldown.saturating_sub(1);
        c.retarget = c.retarget.saturating_sub(1);
        let ty = &setup.squad_types[sq.type_idx as usize];
        let w = &setup.weapons[ty.weapon as usize];
        let still = is_still(&ctx.velocity[i]);
        let may = w.he.is_none()
            && match engage(&sq.order) {
                Engage::Never => false,
                Engage::Still => still,
                Engage::Always => true,
            };
        if !may {
            c.target = None;
            continue;
        }
        let owner = ctx.owner[i].0;
        let pos = ctx.transform[i].pos;
        let r = range_sq(w.range_max.into());
        if let Some(t) = c.target
            && valid_target(ctx.marks, ctx.mark_of, t, owner, pos.xy(), r).is_none()
        {
            c.target = None;
        }
        let from = muzzle(pos, c);
        if c.target.is_none() || c.retarget == 0 {
            let before = c.target;
            c.target = if w.class == WeaponClass::Mg {
                let roll =
                    RngStream::new(ctx.seed, ctx.tick, SimId(i as u32), Purpose::SWEEP).next_u64();
                search(ctx, i, from, r, None, SWEEP_POOL, (roll >> 32) as usize)
            } else {
                search(ctx, i, from, r, sq.focus, LOS_TRIES, 0)
            };
            c.retarget = if c.target.is_some() { hz } else { hz / 3 }.max(1) as u16;
            if c.target.is_some() && before.is_none() {
                // Aim before opening fire (and rise from cover); switching
                // targets mid-fight costs nothing.
                ws.cooldown = ws.cooldown.max((hz / 2) as u16);
            }
        }
        let Some(t) = c.target else { continue };
        if ws.cooldown > 0 || (c.still as u32) < w.setup_ticks {
            continue;
        }
        let id = SimId(i as u32);
        let aim = ctx.marks[ctx.mark_of[t.index()] as usize].aim;
        let dist = (aim - from).length();
        let range: Fx = w.range_max.into();
        let pct = spread_pct(rules, c, !still, dist, range);
        let spread =
            Fx::from_raw((Fx::from(w.spread).raw() as i64 * pct / 100).min(i32::MAX as i64) as i32);
        let mut rng = RngStream::new(ctx.seed, ctx.tick, id, Purpose::AIM);
        let vel = shot_velocity(from, aim, w.muzzle_speed.into(), spread, &mut rng);
        out.push(FireRequest {
            shooter: id,
            seq: 0,
            pos: from,
            vel,
            weapon: ty.weapon,
            player: owner,
            ttl: round_ttl(w),
            he: false,
        });
        ws.shots = ws.shots.wrapping_add(1);
        c.since_shot = 0;
        c.mag = c.mag.saturating_sub(1);
        if c.mag == 0 {
            c.mag = w.magazine.min(u16::MAX as u32) as u16;
            ws.cooldown = w.reload_ticks.max(interval_ceil(w)).min(u16::MAX as u32) as u16;
        } else {
            ws.cooldown = interval_after(w, ws.shots).min(u16::MAX as u32) as u16;
        }
    }
    changed
}

// --------------------------------------------------------------- mortars --

impl MatchState {
    /// Fires the mortar teams' barrages (see the module docs). Runs after
    /// [`aim_and_fire`] counted their cooldowns down.
    pub(crate) fn fire_mortars(
        &mut self,
        ents: &mut Entities,
        terrain: &Heightfield,
        hash: &mut HashCache,
        seed: u64,
        tick: u64,
        out: &mut Vec<FireRequest>,
    ) {
        let setup = &self.setup;
        let mut repath = Vec::new();
        for (&id, sq) in self.squads.iter_mut() {
            let Order::Barrage { target, rounds } = &mut sq.order else {
                continue;
            };
            if sq.members.is_empty() || sq.nav.phase != NavPhase::Holding || *rounds == 0 {
                continue;
            }
            let ty = &setup.squad_types[sq.type_idx as usize];
            let w = &setup.weapons[ty.weapon as usize];
            if w.he.is_none() {
                continue;
            }
            let gunner = sq.members[0];
            let i = gunner.index();
            let pos = ents.transform[i].pos;
            let d = (*target - pos.xy()).length();
            let (min, max): (Fx, Fx) = (w.range_min.into(), w.range_max.into());
            if d < min || d > max {
                // Holding out of range (the gunner fell and the next man
                // stands elsewhere, or the walk ended short): walk to 90 % of
                // the range again, unless the last walk already went for
                // that spot; then it cannot be reached and the order ends.
                let facing = (*target - pos.xy()).normalize_or_zero();
                let spot = terrain.clamp(*target - facing * max.mul_int(9).div_int(10));
                let tried =
                    (spot - sq.nav.goal).length_sq_raw() <= range_sq(Fx::from_int(BARRAGE_RETRY_M));
                if d < min || tried {
                    sq.order = Order::Idle;
                } else {
                    repath.push((id, spot));
                }
                continue;
            }
            let c = &ents.combat[i];
            if ents.weapon[i].cooldown > 0 || (c.still as u32) < w.setup_ticks {
                continue;
            }
            // Scatter the aim point by the spread over the range.
            let mut rng = RngStream::new(seed, tick, gunner, Purpose::SHELL_AIM);
            let pct = 100 + (c.suppression / 1000) as i64;
            let s = d.mul(w.spread.into()).raw() as i64 * pct / 100;
            let s = Fx::from_raw(s.min(i32::MAX as i64) as i32);
            let aim_xy = terrain.clamp(*target + FxVec2::new(tri(&mut rng, s), tri(&mut rng, s)));
            let from = FxVec3::new(pos.x, pos.y, pos.z + Fx::from_milli(SHELL_MUZZLE_MM));
            let dxy = aim_xy - from.xy();
            let flight = dxy
                .length()
                .mul(Fx::from_milli(SHELL_FLIGHT_MILLI))
                .sqrt()
                .max(Fx::from_int(SHELL_MIN_FLIGHT_S));
            let rise = terrain.height_at(aim_xy) - from.z;
            let vz = (rise + GRAVITY.mul(flight).mul(flight).div_int(2)).div(flight);
            let vel = FxVec3::new(dxy.x.div(flight), dxy.y.div(flight), vz);
            let ttl = (flight + Fx::from_int(4)).mul_int(SUBSTEP_HZ).floor_int();
            out.push(FireRequest {
                shooter: gunner,
                seq: 0,
                pos: from,
                vel,
                weapon: ty.weapon,
                player: sq.owner.0,
                ttl: ttl.clamp(2, u16::MAX as i32) as u16,
                he: true,
            });
            let ws = &mut ents.weapon[i];
            ws.shots = ws.shots.wrapping_add(1);
            ws.cooldown = interval_ceil(w).max(w.reload_ticks).min(u16::MAX as u32) as u16;
            ents.combat[i].since_shot = 0;
            hash.mark_entity_chunk(gunner.chunk());
            *rounds -= 1;
            if *rounds == 0 {
                sq.order = Order::Idle;
            }
        }
        for (id, spot) in repath {
            self.request_path(id, spot);
        }
    }
}

/// Spawns this tick's sorted fire requests into the pool: HE shells first,
/// then small arms while more than [`crate::projectile::HE_RESERVE`] slots
/// are free. Returns `(fired, dropped)`.
pub(crate) fn spawn_rounds(
    reqs: &[FireRequest],
    pool: &mut crate::projectile::ProjectilePool,
    hash: &mut HashCache,
) -> (u64, u64) {
    let (mut fired, mut dropped) = (0u64, 0u64);
    for he in [true, false] {
        for r in reqs.iter().filter(|r| r.he == he) {
            fired += 1;
            if !he && pool.free_count() <= crate::projectile::HE_RESERVE {
                dropped += 1;
                continue;
            }
            let p = Projectile {
                pos: r.pos,
                vel: r.vel,
                owner: r.shooter,
                drag: Fx::ZERO,
                ttl: r.ttl.max(1),
                weapon: r.weapon,
                player: r.player,
            };
            match pool.spawn(p) {
                Some(slot) => hash.mark_pool_slot(slot),
                None => dropped += 1,
            }
        }
    }
    (fired, dropped)
}

// ------------------------------------------------------------ ballistics --

/// What a projectile did to soldiers or the ground (merged in slot order).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Blow {
    Hit(SimId),
    NearMiss(SimId),
    /// An HE shell burst at this point (on the surface).
    Burst(FxVec3),
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) struct BallisticEvent {
    pub weapon: u16,
    pub player: u8,
    pub blow: Blow,
}

/// Hit capsule `(a, b, radius)` of a soldier: a vertical segment standing or
/// crouched, a horizontal one along the heading when prone.
fn capsule(t: &Transform, c: &Combat) -> (FxVec3, FxVec3, Fx) {
    let pos = t.pos;
    if c.stance == Stance::Prone {
        let (s, co) = t.yaw.sin_cos();
        let half = Fx::from_milli(PRONE_HALF_MM);
        let f = FxVec3::new(co.mul(half), s.mul(half), Fx::ZERO);
        let r = Fx::from_milli(PRONE_R_MM);
        let mid = FxVec3::new(pos.x, pos.y, pos.z + r);
        return (mid - f, mid + f, r);
    }
    let r = Fx::from_milli(CAPSULE_R_MM);
    let top = (c.height - r).max(r);
    (
        FxVec3::new(pos.x, pos.y, pos.z + r),
        FxVec3::new(pos.x, pos.y, pos.z + top),
        r,
    )
}

/// Closest points of segments `p1 + s·d1` and `p2 + t·d2` (`s, t` in
/// `[0, 1]`; after Ericson, *Real-Time Collision Detection* 5.1.9). Inputs
/// are relative to a nearby origin so all products stay in Q16.16.
/// Returns `(s, distance²)`.
fn seg_seg(p1: FxVec3, d1: FxVec3, p2: FxVec3, d2: FxVec3) -> (Fx, Fx) {
    let eps = Fx::from_raw(16);
    let r = p1 - p2;
    let a = d1.dot(d1);
    let e = d2.dot(d2);
    let f = d2.dot(r);
    let clamp01 = |v: Fx| v.clamp(Fx::ZERO, Fx::ONE);
    let (s, t);
    if a <= eps && e <= eps {
        (s, t) = (Fx::ZERO, Fx::ZERO);
    } else if a <= eps {
        s = Fx::ZERO;
        t = clamp01(f.div(e));
    } else {
        let c = d1.dot(r);
        if e <= eps {
            t = Fx::ZERO;
            s = clamp01((-c).div(a));
        } else {
            let b = d1.dot(d2);
            let denom = a.mul(e) - b.mul(b);
            let mut s0 = if denom > eps {
                clamp01((b.mul(f) - c.mul(e)).div(denom))
            } else {
                Fx::ZERO
            };
            let mut t0 = (b.mul(s0) + f).div(e);
            if t0 < Fx::ZERO {
                t0 = Fx::ZERO;
                s0 = clamp01((-c).div(a));
            } else if t0 > Fx::ONE {
                t0 = Fx::ONE;
                s0 = clamp01((b - c).div(a));
            }
            (s, t) = (s0, t0);
        }
    }
    let gap = (p1 + d1 * s) - (p2 + d2 * t);
    (s, gap.dot(gap))
}

/// Read-only inputs of [`fly`].
pub(crate) struct FlightCtx<'a> {
    pub ents: &'a Entities,
    pub spatial: &'a SpatialHash,
    pub terrain: &'a Heightfield,
    pub weapons: &'a [WeaponSetup],
    pub near_miss: Fx,
}

/// A soldier near a round's segment this substep.
struct Near {
    id: SimId,
    centre: FxVec3,
    /// Unclamped position of his closest approach along the segment.
    s: Fx,
    dist_sq: Fx,
}

/// Integrates one chunk of match projectile slots for `substeps` substeps
/// (see the module docs). Finished projectiles are cleared and reported in
/// `ended`; their effects go to `events`, both in slot order.
pub(crate) fn fly(
    chunk: usize,
    slots: &mut [Option<Projectile>],
    ctx: &FlightCtx,
    substeps: u32,
    ended: &mut Vec<(u32, ProjectileEnd)>,
    events: &mut Vec<BallisticEvent>,
) -> bool {
    let base = chunk * crate::ecs::CHUNK_SIZE;
    let ents = ctx.ents;
    let mut near: Vec<Near> = Vec::new();
    let mut changed = false;
    for (j, slot) in slots.iter_mut().enumerate() {
        let Some(p) = slot else { continue };
        changed = true;
        let slot_id = (base + j) as u32;
        let he = ctx
            .weapons
            .get(p.weapon as usize)
            .is_some_and(|w| w.he.is_some());
        let (weapon, player, shooter) = (p.weapon, p.player, p.owner);
        let ev = |blow| BallisticEvent {
            weapon,
            player,
            blow,
        };
        for _ in 0..substeps {
            let from = p.pos;
            let next = advance(p);
            if he {
                if let Some(g) = ctx.terrain.segment_hit(from, next) {
                    let surface = FxVec3::new(g.x, g.y, ctx.terrain.height_at(g.xy()));
                    p.pos = g;
                    events.push(ev(Blow::Burst(surface)));
                    ended.push((slot_id, ProjectileEnd::GroundHit));
                    *slot = None;
                    break;
                }
                if let Some(end) = finish(p, next, ctx.terrain) {
                    ended.push((slot_id, end));
                    *slot = None;
                    break;
                }
                continue;
            }
            // Soldiers near the segment, nearest capsule hit first.
            let d = next - from;
            let len = d.length();
            let mid = from + d.div_int(2);
            let reach = len.div_int(2) + ctx.near_miss + Fx::from_int(2);
            let len_sq = d.dot(d);
            near.clear();
            let mut hit: Option<(Fx, SimId)> = None;
            let nm_sq = ctx.near_miss.mul(ctx.near_miss);
            ctx.spatial.for_each_near(mid.xy(), reach, |id| {
                let i = id.index();
                if ents.owner[i].0 == player || id == shooter {
                    return;
                }
                let t = &ents.transform[i];
                let c = &ents.combat[i];
                // Local coordinates around the soldier keep products small.
                let o = t.pos;
                let (a, b, r) = capsule(t, c);
                let (s, dsq) = seg_seg(from - o, d, a - o, b - a);
                if dsq <= r.mul(r) && hit.is_none_or(|(hs, hid)| (s, id) < (hs, hid)) {
                    hit = Some((s, id));
                }
                let centre = body_point(o, c);
                let rel = centre - from;
                let su = if len_sq > Fx::ZERO {
                    rel.dot(d).div(len_sq)
                } else {
                    Fx::ZERO
                };
                let gap = rel - d * su.clamp(Fx::ZERO, Fx::ONE);
                near.push(Near {
                    id,
                    centre,
                    s: su,
                    dist_sq: gap.dot(gap),
                });
            });
            let mut end_s = Fx::ONE;
            if let Some((s, _)) = hit {
                end_s = s;
            }
            let stop = from + d * end_s;
            let ground = ctx.terrain.segment_hit(from, stop);
            if let Some(g) = ground {
                end_s = if len > Fx::ZERO {
                    (g - from).length().div(len)
                } else {
                    Fx::ZERO
                };
                hit = None;
            }
            // Near misses: passed (closest approach inside the segment and
            // before the round stopped), or close to where it struck earth.
            for n in &near {
                if Some(n.id) == hit.map(|h| h.1) {
                    continue;
                }
                let passed = n.s >= Fx::ZERO && n.s < end_s && n.dist_sq <= nm_sq;
                let by_impact = ground.is_some_and(|g| {
                    let v = n.centre - g;
                    v.dot(v) <= nm_sq
                });
                if passed || by_impact {
                    events.push(ev(Blow::NearMiss(n.id)));
                }
            }
            if let Some(g) = ground {
                p.pos = g;
                ended.push((slot_id, ProjectileEnd::GroundHit));
                *slot = None;
                break;
            }
            if let Some((s, id)) = hit {
                p.pos = from + d * s;
                events.push(ev(Blow::Hit(id)));
                ended.push((slot_id, ProjectileEnd::SoldierHit));
                *slot = None;
                break;
            }
            if let Some(end) = finish(p, next, ctx.terrain) {
                ended.push((slot_id, end));
                *slot = None;
                break;
            }
        }
    }
    changed
}

// ---------------------------------------------------------------- damage --

/// A burst this tick, for the snapshot.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) struct Explosion {
    pub pos: FxVec3,
    pub weapon: u16,
}

/// Mutable world parts the damage stage needs.
pub(crate) struct DamageCtx<'a> {
    pub ents: &'a mut Entities,
    pub terrain: &'a mut Heightfield,
    pub spatial: &'a SpatialHash,
    pub hash: &'a mut HashCache,
    pub stats: &'a mut Stats,
    pub seed: u64,
    pub tick: u64,
}

#[inline]
fn add_suppression(c: &mut Combat, milli: u32) {
    c.suppression = c.suppression.saturating_add(milli).min(100_000);
}

/// Whether a man is crouched below cover (in a trench, not on the step).
#[inline]
fn covered(c: &Combat) -> bool {
    c.in_trench && c.height < c.fire_height
}

impl MatchState {
    /// Applies this tick's ballistic events in order, then kills soldiers
    /// left at 0 HP (see the module docs). Bursts are reported in
    /// `explosions`.
    pub(crate) fn apply_blows(
        &mut self,
        ctx: &mut DamageCtx,
        events: &[BallisticEvent],
        explosions: &mut Vec<Explosion>,
    ) {
        let rules = self.setup.rules.combat.clone();
        // (victim, killer) in the order they fell.
        let mut dead: Vec<(SimId, u8)> = Vec::new();
        let mut damage =
            |ents: &mut Entities, hash: &mut HashCache, id: SimId, dmg: u32, by: u8| {
                let c = &mut ents.combat[id.index()];
                if c.hp == 0 {
                    return;
                }
                c.hp = c.hp.saturating_sub(dmg.min(u16::MAX as u32) as u16);
                if c.hp == 0 {
                    dead.push((id, by));
                }
                hash.mark_entity_chunk(id.chunk());
            };
        for (k, e) in events.iter().enumerate() {
            let w = &self.setup.weapons[e.weapon as usize];
            match e.blow {
                Blow::Hit(id) => {
                    if ctx.ents.kind[id.index()] != EntityKind::Soldier {
                        continue;
                    }
                    ctx.stats.soldier_hits += 1;
                    add_suppression(
                        &mut ctx.ents.combat[id.index()],
                        rules.hit_suppression * 1000,
                    );
                    damage(ctx.ents, ctx.hash, id, w.damage, e.player);
                }
                Blow::NearMiss(id) => {
                    let c = &mut ctx.ents.combat[id.index()];
                    if ctx.ents.kind[id.index()] != EntityKind::Soldier || c.hp == 0 {
                        continue;
                    }
                    ctx.stats.near_misses += 1;
                    let mut add = w.suppression * 1000;
                    if covered(c) {
                        add /= 2;
                    }
                    add_suppression(c, add);
                    ctx.hash.mark_entity_chunk(id.chunk());
                }
                Blow::Burst(pos) => {
                    let Some(he) = &w.he else { continue };
                    ctx.stats.explosions += 1;
                    explosions.push(Explosion {
                        pos,
                        weapon: e.weapon,
                    });
                    let lift = FxVec3::new(pos.x, pos.y, pos.z + Fx::from_milli(BLAST_LIFT_MM));
                    let (lethal, reach): (Fx, Fx) =
                        (he.lethal_radius.into(), he.damage_radius.into());
                    let supp_r: Fx = he.suppression_radius.into();
                    let mut ids = Vec::new();
                    ctx.spatial
                        .for_each_near(pos.xy(), supp_r, |id| ids.push(id));
                    for id in ids {
                        let i = id.index();
                        if ctx.ents.kind[i] != EntityKind::Soldier
                            || ctx.ents.owner[i].0 == e.player
                        {
                            continue;
                        }
                        let c = ctx.ents.combat[i];
                        if c.hp == 0 {
                            continue;
                        }
                        let body = body_point(ctx.ents.transform[i].pos, &c);
                        let dist = (body - pos).length();
                        if dist > supp_r {
                            continue;
                        }
                        let clear = ctx.terrain.line_of_sight(lift, body);
                        let mut add = (w.suppression as i64 * 1000 * (supp_r - dist).raw() as i64
                            / supp_r.raw().max(1) as i64)
                            as u32;
                        if !clear {
                            add /= 2;
                        }
                        add_suppression(&mut ctx.ents.combat[i], add);
                        ctx.hash.mark_entity_chunk(id.chunk());
                        if clear && dist < reach {
                            let dmg = if dist <= lethal {
                                w.damage
                            } else {
                                (w.damage as i64 * (reach - dist).raw() as i64
                                    / (reach - lethal).raw().max(1) as i64)
                                    as u32
                            };
                            damage(ctx.ents, ctx.hash, id, dmg, e.player);
                        }
                    }
                    let mut rng =
                        RngStream::new(ctx.seed, ctx.tick, SimId(k as u32), Purpose::CRATER);
                    let pct = rng.range_i32(CRATER_DEPTH_PCT.0, CRATER_DEPTH_PCT.1);
                    let depth = Fx::from(he.crater_depth).mul_int(pct).div_int(100);
                    ctx.terrain
                        .crater(pos.xy(), he.crater_radius.into(), depth, ctx.tick);
                }
            }
        }
        dead.sort_unstable();
        for (id, by) in dead {
            self.kill(ctx.ents, ctx.hash, id, rules.corpse_ticks);
            ctx.stats.deaths += 1;
            if let Some(p) = self.players.get_mut(by as usize) {
                p.kills += 1;
            }
        }
    }

    /// Turns soldier `id` into a corpse and takes him out of his squad.
    pub(crate) fn kill(
        &mut self,
        ents: &mut Entities,
        hash: &mut HashCache,
        id: SimId,
        corpse_ticks: u32,
    ) {
        let i = id.index();
        let owner = ents.owner[i].0 as usize;
        ents.kind[i] = EntityKind::Corpse;
        ents.velocity[i] = Velocity::default();
        let c = &mut ents.combat[i];
        *c = Combat {
            stance: Stance::Prone,
            height: Fx::from_milli(PRONE_MM),
            fire_height: Fx::from_milli(PRONE_MM),
            corpse: corpse_ticks.clamp(1, u16::MAX as u32) as u16,
            seen_by: c.seen_by,
            ..Combat::default()
        };
        hash.mark_entity_chunk(id.chunk());
        if let Some(p) = self.players.get_mut(owner) {
            p.losses += 1;
        }
        let Some(sid) = ents.squad[i].take() else {
            return;
        };
        let Some(sq) = self.squads.get_mut(&sid) else {
            return;
        };
        if let Some(k) = sq.members.iter().position(|m| *m == id) {
            sq.members.remove(k);
            if k < sq.nav.slots.len() {
                sq.nav.slots.remove(k);
            }
        }
        if sq.focus == Some(id) {
            sq.focus = None;
        }
        if sq.members.is_empty() {
            self.squads.remove(&sid);
            for q in &mut self.path_queue.queues {
                q.retain(|s| *s != sid);
            }
        }
    }
}

// ---------------------------------------------------------------- status --

/// Read-only inputs of [`status`].
pub(crate) struct StatusCtx<'a> {
    pub kind: &'a [EntityKind],
    pub transform: &'a [Transform],
    pub velocity: &'a [Velocity],
    pub weapon: &'a [Weapon],
    pub squad: &'a [Option<SquadId>],
    pub game: &'a MatchState,
    pub terrain: &'a Heightfield,
}

/// Suppression decay, stance and timers for one entity chunk; corpses that
/// ran out go to `gone` (see the module docs).
pub(crate) fn status(
    chunk: usize,
    ctx: &StatusCtx,
    combat: &mut [Combat],
    gone: &mut Vec<SimId>,
) -> bool {
    let base = chunk * crate::ecs::CHUNK_SIZE;
    let setup = ctx.game.setup();
    let rules = &setup.rules.combat;
    let aim_ticks = (setup.tick_hz / 2) as u16;
    let parapet: Fx = setup.rules.dig.parapet_height.into();
    let exposure: Fx = rules.fire_exposure.into();
    let mut changed = false;
    for (j, c) in combat.iter_mut().enumerate() {
        let i = base + j;
        match ctx.kind[i] {
            EntityKind::Corpse => {
                changed = true;
                c.corpse = c.corpse.saturating_sub(1);
                if c.corpse == 0 {
                    gone.push(SimId(i as u32));
                }
                continue;
            }
            EntityKind::Soldier => {}
            EntityKind::Vacant => continue,
        }
        changed = true;
        let sq = ctx.squad[i].and_then(|s| ctx.game.squad(s));
        let retreating = sq.is_some_and(|s| s.order == Order::Retreat);
        let weapon =
            sq.map(|s| &setup.weapons[setup.squad_types[s.type_idx as usize].weapon as usize]);
        c.suppression = c.suppression.saturating_sub(rules.suppression_decay_milli);
        let points = c.suppression / 1000;
        c.pinned = points >= rules.pin_suppression && !retreating;
        let pos = ctx.transform[i].pos;
        let base_z = if ctx.terrain.has_detail_at(pos.xy()) {
            ctx.terrain.base_height_at(pos.xy())
        } else {
            pos.z
        };
        c.in_trench = base_z - pos.z >= Fx::from_milli(TRENCH_MM);
        if is_still(&ctx.velocity[i]) {
            c.still = c.still.saturating_add(1);
        } else {
            c.still = 0;
        }
        c.since_shot = c.since_shot.saturating_add(1);
        let aiming =
            c.target.is_some() && (ctx.weapon[i].cooldown <= aim_ticks || c.since_shot < aim_ticks);
        let crouched = points >= rules.crouch_suppression;
        let (stand, crouch, prone) = (
            Fx::from_milli(STAND_MM),
            Fx::from_milli(CROUCH_MM),
            Fx::from_milli(PRONE_MM),
        );
        let step = (base_z + parapet + exposure - pos.z).max(stand);
        (c.stance, c.height, c.fire_height) = if c.pinned {
            (Stance::Prone, prone, prone)
        } else if c.in_trench {
            if aiming {
                (Stance::Stand, step, step)
            } else {
                (Stance::Crouch, crouch, step)
            }
        } else if weapon
            .is_some_and(|w| w.class == WeaponClass::Mg && c.still as u32 >= w.setup_ticks)
        {
            // A set-up MG fires lying down.
            (Stance::Prone, prone, prone)
        } else if crouched {
            (Stance::Crouch, crouch, crouch)
        } else {
            (Stance::Stand, stand, stand)
        };
    }
    changed
}

/// Fresh combat state for a soldier of squad type `ty` (match spawns).
pub(crate) fn spawn_combat(hp: u32, w: &WeaponSetup) -> Combat {
    Combat {
        hp: hp.clamp(1, u16::MAX as u32) as u16,
        height: Fx::from_milli(STAND_MM),
        fire_height: Fx::from_milli(STAND_MM),
        mag: w.magazine.clamp(1, u16::MAX as u32) as u16,
        since_shot: u16::MAX,
        ..Combat::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dig::stamp;
    use crate::game::Ctx;
    use crate::terrain::TerrainKind;
    use frontline_content::MatchSetup;

    fn v(x: i32, y: i32, z: i32) -> FxVec3 {
        FxVec3::new(Fx::from_milli(x), Fx::from_milli(y), Fx::from_milli(z))
    }

    #[test]
    fn segment_distance_matches_geometry() {
        // A horizontal segment passing 0.2 m beside a vertical capsule axis.
        let (s, d) = seg_seg(
            v(-5000, 200, 1000),
            v(10_000, 0, 0),
            v(0, 0, 0),
            v(0, 0, 1750),
        );
        assert!((s - Fx::HALF).abs() <= Fx::from_milli(2), "{s:?}");
        assert!((d - Fx::from_milli(40)).abs() <= Fx::from_milli(2), "{d:?}");
        // Passing over the top: closest to the capsule's top end.
        let (_, d) = seg_seg(
            v(-5000, 0, 2750),
            v(10_000, 0, 0),
            v(0, 0, 0),
            v(0, 0, 1750),
        );
        assert!((d - Fx::ONE).abs() <= Fx::from_milli(2), "{d:?}");
        // Parallel segments.
        let (_, d) = seg_seg(v(0, 300, 0), v(0, 0, 1000), v(0, 0, 0), v(0, 0, 1000));
        assert!((d - Fx::from_milli(90)).abs() <= Fx::from_milli(2), "{d:?}");
    }

    #[test]
    fn fractional_fire_interval_keeps_the_rate() {
        let mut w = setup().weapons[0].clone();
        // 1200 rpm at 30 Hz: 1.5 ticks, so 1, 2, 1, 2 ...
        w.interval_milli = 1500;
        let gaps: Vec<u32> = (1..=6).map(|n| interval_after(&w, n)).collect();
        assert_eq!(gaps, [1, 2, 1, 2, 1, 2]);
        // 500 rpm: 3.6 ticks; 1000 shots take exactly 3600 ticks.
        w.interval_milli = 3600;
        let total: u32 = (1..=1000).map(|n| interval_after(&w, n)).sum();
        assert_eq!(total, 3600);
        assert_eq!(interval_ceil(&w), 4);
    }

    #[test]
    fn engage_rules_follow_the_order() {
        assert_eq!(engage(&Order::Retreat), Engage::Never);
        assert_eq!(engage(&Order::Idle), Engage::Always);
        let target = FxVec2::ZERO;
        assert_eq!(engage(&Order::Move { target }), Engage::Still);
        assert_eq!(engage(&Order::AttackMove { target }), Engage::Always);
    }

    fn setup() -> MatchSetup {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
        let defs = frontline_content::ModDefs::load(&root).unwrap();
        let req = frontline_content::MatchRequest::skirmish(
            1,
            "ww2:germany",
            "ww2:usa",
            frontline_content::Difficulty::Normal,
        );
        defs.resolve(&req).unwrap()
    }

    /// Flat ground at height 0 with one US rifle squad (player 1).
    struct Rig {
        game: MatchState,
        ents: Entities,
        terrain: Heightfield,
        hash: HashCache,
        spatial: SpatialHash,
        squad: SquadId,
    }

    /// A trench along x = 500 from y = 480 to 520, parapet to the east.
    const TRENCH_X: i32 = 500;

    fn rig(trench: bool) -> Rig {
        let setup = setup();
        let mut terrain = Heightfield::generate(TerrainKind::Flat, setup.map.size_m, 1);
        if trench {
            let line = [
                FxVec2::from_int(TRENCH_X, 480),
                FxVec2::from_int(TRENCH_X, 520),
            ];
            stamp(
                &mut terrain,
                &line,
                -1,
                Fx::ZERO,
                Fx::from_int(40),
                8,
                8,
                &setup.rules.dig,
                0,
            );
        }
        let ty = setup.squad_type_index("ww2:usa_rifle_squad").unwrap();
        let mut game = MatchState::new(setup);
        let mut ents = Entities::default();
        let mut hash = HashCache::default();
        let mut ctx = Ctx {
            ents: &mut ents,
            terrain: &terrain,
            hash: &mut hash,
            tick: 0,
        };
        let squad = game.spawn_squad(&mut ctx, PlayerId(1), ty);
        let spatial = SpatialHash::new(game.setup().map.size_m);
        Rig {
            game,
            ents,
            terrain,
            hash,
            spatial,
            squad,
        }
    }

    impl Rig {
        fn members(&self) -> Vec<SimId> {
            self.game.squad(self.squad).unwrap().members.clone()
        }

        /// Puts member `k` at `(x, y)` on the surface and runs the status
        /// stage once (stance, capsule heights).
        fn put(&mut self, k: usize, x: i32, y: i32) -> SimId {
            let id = self.members()[k];
            let xy = FxVec2::from_int(x, y);
            self.ents.transform[id.index()].pos = xy.extend(self.terrain.height_at(xy));
            self.ents.transform[id.index()].yaw = Fx::PI;
            id
        }

        fn status(&mut self) {
            let ctx = StatusCtx {
                kind: &self.ents.kind,
                transform: &self.ents.transform,
                velocity: &self.ents.velocity,
                weapon: &self.ents.weapon,
                squad: &self.ents.squad,
                game: &self.game,
                terrain: &self.terrain,
            };
            let mut gone = Vec::new();
            status(0, &ctx, &mut self.ents.combat, &mut gone);
            self.spatial.rebuild(&self.ents);
        }

        /// Flies one exactly aimed rifle round (player 0) from `from` at
        /// `aim`; returns what it struck.
        fn shoot(&self, from: FxVec3, aim: FxVec3) -> (ProjectileEnd, Vec<BallisticEvent>) {
            let setup = self.game.setup();
            let kar = setup
                .weapons
                .iter()
                .position(|w| w.id == "ww2:kar98k")
                .unwrap();
            let w = &setup.weapons[kar];
            let mut rng = RngStream::new(1, 0, SimId(0), Purpose::AIM);
            let vel = shot_velocity(from, aim, w.muzzle_speed.into(), Fx::ZERO, &mut rng);
            let mut slots = vec![Some(Projectile {
                pos: from,
                vel,
                owner: SimId(9999),
                drag: Fx::ZERO,
                ttl: round_ttl(w),
                weapon: kar as u16,
                player: 0,
            })];
            let ctx = FlightCtx {
                ents: &self.ents,
                spatial: &self.spatial,
                terrain: &self.terrain,
                weapons: &setup.weapons,
                near_miss: setup.rules.combat.near_miss.into(),
            };
            let (mut ended, mut events) = (Vec::new(), Vec::new());
            for _ in 0..60 {
                fly(0, &mut slots, &ctx, 2, &mut ended, &mut events);
                if let Some(&(_, end)) = ended.first() {
                    return (end, events);
                }
            }
            panic!("round never ended");
        }

        fn burst(&mut self, at: FxVec2) {
            let mortar = self
                .game
                .setup()
                .weapons
                .iter()
                .position(|w| w.he.is_some())
                .unwrap() as u16;
            let pos = at.extend(self.terrain.height_at(at));
            let mut stats = Stats::default();
            let mut ctx = DamageCtx {
                ents: &mut self.ents,
                terrain: &mut self.terrain,
                spatial: &self.spatial,
                hash: &mut self.hash,
                stats: &mut stats,
                seed: 1,
                tick: 1,
            };
            let ev = [BallisticEvent {
                weapon: mortar,
                player: 0,
                blow: Blow::Burst(pos),
            }];
            self.game.apply_blows(&mut ctx, &ev, &mut Vec::new());
        }

        fn hp(&self, id: SimId) -> u16 {
            self.ents.combat[id.index()].hp
        }
    }

    #[test]
    fn trench_lip_stops_rounds_until_the_man_rises_to_fire() {
        let mut r = rig(true);
        let man = r.put(0, TRENCH_X, 500);
        r.status();
        let c = r.ents.combat[man.index()];
        assert!(c.in_trench && c.stance == Stance::Crouch, "{c:?}");
        let pos = r.ents.transform[man.index()].pos;
        assert_eq!(pos.z, Fx::from_milli(-1600), "on the trench floor");
        // A rifleman 100 m east (the parapet side), lying on the ground.
        let muzzle = v((TRENCH_X + 100) * 1000, 500_000, 250);
        // Crouched: head 0.4 m below ground. The lip takes the round.
        let (end, ev) = r.shoot(muzzle, aim_point(pos, &c));
        assert_eq!(end, ProjectileEnd::GroundHit, "{ev:?}");
        assert!(!ev.iter().any(|e| matches!(e.blow, Blow::Hit(_))));
        // He rises to fire: 0.3 m above the parapet, and only that shows.
        r.ents.combat[man.index()].target = Some(SimId(9999));
        r.status();
        let c = r.ents.combat[man.index()];
        assert_eq!(c.stance, Stance::Stand);
        let top = pos.z + c.height;
        // Parapet 0.3 m, plus 0.3 m exposed.
        assert!(
            (top - Fx::from_milli(600)).abs() <= Fx::from_milli(1),
            "{top:?}"
        );
        let (end, ev) = r.shoot(muzzle, aim_point(pos, &c));
        assert_eq!(end, ProjectileEnd::SoldierHit);
        assert_eq!(ev.last().unwrap().blow, Blow::Hit(man));
        // His chest is still behind the parapet.
        let chest = FxVec3::new(pos.x, pos.y, Fx::from_milli(100));
        let (end, _) = r.shoot(muzzle, chest);
        assert_eq!(end, ProjectileEnd::GroundHit);
        // The same man standing in the open is hit in the chest.
        let mut open = rig(false);
        let man = open.put(0, TRENCH_X, 500);
        open.status();
        let chest = FxVec3::new(pos.x, pos.y, Fx::from_milli(1100));
        let (end, ev) = open.shoot(muzzle, chest);
        assert_eq!(end, ProjectileEnd::SoldierHit);
        assert_eq!(ev.last().unwrap().blow, Blow::Hit(man));
    }

    #[test]
    fn near_misses_suppress_and_misses_pass_by() {
        let mut r = rig(false);
        let man = r.put(0, 500, 500);
        r.status();
        // 1 m to his side at head height: a near miss, no hit.
        let muzzle = v(600_000, 501_000, 1500);
        let (end, ev) = r.shoot(muzzle, v(400_000, 501_000, 1500));
        assert_ne!(end, ProjectileEnd::SoldierHit);
        assert_eq!(
            ev.iter().filter(|e| e.blow == Blow::NearMiss(man)).count(),
            1,
            "{ev:?}"
        );
        // 3 m away: nothing.
        let (_, ev) = r.shoot(v(600_000, 503_000, 1500), v(400_000, 503_000, 1500));
        assert!(ev.is_empty(), "{ev:?}");
        // Own rounds never touch him (no friendly fire).
        let mut slots = vec![Some(Projectile {
            pos: v(510_000, 500_000, 1000),
            vel: v(-760_000, 0, 0),
            owner: SimId(9999),
            drag: Fx::ZERO,
            ttl: 60,
            weapon: 0,
            player: 1,
        })];
        let ctx = FlightCtx {
            ents: &r.ents,
            spatial: &r.spatial,
            terrain: &r.terrain,
            weapons: &r.game.setup().weapons,
            near_miss: Fx::from_int(2),
        };
        let (mut ended, mut events) = (Vec::new(), Vec::new());
        fly(0, &mut slots, &ctx, 2, &mut ended, &mut events);
        assert!(events.is_empty() && ended.is_empty());
    }

    #[test]
    fn suppression_crouches_then_pins() {
        let mut r = rig(false);
        let man = r.put(0, 500, 500);
        let rules = r.game.setup().rules.combat.clone();
        let decay = rules.suppression_decay_milli;
        let check = |r: &mut Rig, points: u32, stance: Stance, pinned: bool| {
            r.ents.combat[man.index()].suppression = points * 1000 + decay;
            r.status();
            let c = r.ents.combat[man.index()];
            assert_eq!((c.stance, c.pinned), (stance, pinned), "at {points}");
            assert_eq!(c.suppression, points * 1000, "decays per tick");
        };
        check(&mut r, rules.crouch_suppression - 1, Stance::Stand, false);
        check(&mut r, rules.crouch_suppression, Stance::Crouch, false);
        check(&mut r, rules.pin_suppression - 1, Stance::Crouch, false);
        check(&mut r, rules.pin_suppression, Stance::Prone, true);
        let c = r.ents.combat[man.index()];
        assert_eq!(c.height, Fx::from_milli(PRONE_MM));
        // A retreating squad runs even under fire.
        r.game.squads.get_mut(&r.squad).unwrap().order = Order::Retreat;
        check(&mut r, rules.pin_suppression, Stance::Crouch, false);
        // Pinned soldiers do not move.
        r.game.squads.get_mut(&r.squad).unwrap().order = Order::Idle;
        check(&mut r, 100, Stance::Prone, true);
        let before = r.ents.transform[man.index()].pos;
        let planned = vec![FxVec2::from_int(2, 0); r.ents.len()];
        crate::systems::integrate_soldiers(
            &r.ents.kind,
            &mut r.ents.transform,
            &mut r.ents.velocity,
            &r.ents.combat,
            &planned,
            &r.terrain,
            30,
        );
        assert_eq!(r.ents.transform[man.index()].pos, before);
        // Near misses add the weapon's suppression; full decay takes
        // 100 / decay-per-second seconds.
        r.ents.combat[man.index()].suppression = 0;
        let mut stats = Stats::default();
        let mut ctx = DamageCtx {
            ents: &mut r.ents,
            terrain: &mut r.terrain,
            spatial: &r.spatial,
            hash: &mut r.hash,
            stats: &mut stats,
            seed: 1,
            tick: 1,
        };
        let mg = r
            .game
            .setup()
            .weapons
            .iter()
            .position(|w| w.class == WeaponClass::Mg)
            .unwrap();
        let ev = BallisticEvent {
            weapon: mg as u16,
            player: 0,
            blow: Blow::NearMiss(man),
        };
        r.game.apply_blows(&mut ctx, &[ev; 3], &mut Vec::new());
        let per = r.game.setup().weapons[mg].suppression;
        assert_eq!(r.ents.combat[man.index()].suppression, 3 * per * 1000);
        assert_eq!(stats.near_misses, 3);
    }

    #[test]
    fn trench_walls_stop_blast_damage_except_near_direct_hits() {
        // Men crouched on the floor; bursts 3 m to either side of the
        // trench line (inside the lethal radius) do no damage.
        let mut r = rig(true);
        let a = r.put(0, TRENCH_X, 495);
        let b = r.put(1, TRENCH_X, 505);
        r.status();
        assert!(r.ents.combat[a.index()].in_trench);
        r.burst(FxVec2::from_int(TRENCH_X + 3, 495));
        r.burst(FxVec2::from_int(TRENCH_X - 3, 505));
        assert_eq!((r.hp(a), r.hp(b)), (100, 100));
        // Suppressed though (halved behind earth).
        assert!(r.ents.combat[a.index()].suppression > 0);
        // A shell in the trench, 2 m along it from a: lethal.
        r.burst(FxVec2::from_int(TRENCH_X, 497));
        assert_eq!(r.ents.kind[a.index()], EntityKind::Corpse);
        assert_eq!(r.game.players[1].losses, 1);
        assert_eq!(r.game.players[0].kills, 1);
        assert!(!r.game.squad(r.squad).unwrap().members.contains(&a));
        // In the open the same 3 m burst kills.
        let mut open = rig(false);
        let a = open.put(0, TRENCH_X, 495);
        open.status();
        open.burst(FxVec2::from_int(TRENCH_X + 3, 495));
        assert_eq!(open.ents.kind[a.index()], EntityKind::Corpse);
        // Each burst leaves a crater.
        assert!(open.terrain.height_at(FxVec2::from_int(TRENCH_X + 3, 495)) < Fx::from_milli(-300));
    }

    #[test]
    fn last_man_down_deletes_the_squad() {
        let mut r = rig(false);
        let n = r.members().len();
        for k in 0..n {
            r.put(k, 500 + k as i32, 500);
        }
        r.status();
        r.burst(FxVec2::from_int(503, 500));
        r.burst(FxVec2::from_int(503, 500));
        assert!(r.game.squad(r.squad).is_none());
        assert_eq!(r.game.players[1].losses, n as u32);
        assert!(r.ents.soldiers().next().is_none());
        // Corpses count down and are reported for removal.
        let corpse_ticks = r.game.setup().rules.combat.corpse_ticks;
        let mut gone = Vec::new();
        for _ in 0..corpse_ticks {
            let ctx = StatusCtx {
                kind: &r.ents.kind,
                transform: &r.ents.transform,
                velocity: &r.ents.velocity,
                weapon: &r.ents.weapon,
                squad: &r.ents.squad,
                game: &r.game,
                terrain: &r.terrain,
            };
            status(0, &ctx, &mut r.ents.combat, &mut gone);
        }
        assert_eq!(gone.len(), n);
    }
}
