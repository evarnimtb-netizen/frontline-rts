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
//!      from his firing muzzle (a ray through smoke counts as blocked,
//!      `crate::items`). A man winding up a throw does not fire. Opening
//!      fire costs half a second of aiming.
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
//!      may not take the pool's last `SHELL_RESERVE` slots. Dropped rounds are
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
//!   carves a crater. Thrown frags and satchels that go off this tick burst
//!   the same way with their own damage, suppression and crater (a satchel's
//!   levels trenches), after the projectile events, in item id order; smoke
//!   items start their cloud. Soldiers at 0 HP die in ascending id: they become
//!   corpses, leave their squad (squads left empty are deleted) and count
//!   as a loss and a kill.
//! * **Status** (stage 7, per entity chunk): suppression decays; stance is
//!   set (pinned → prone; in a trench → crouched below the lip, or lying
//!   on the floor when ordered prone, standing on the fire step while
//!   aiming or winding up a throw; elsewhere by the man's stance order (S1b): `Auto` → a set-up
//!   MG prone, suppressed crouched, else standing; `Stand` → standing,
//!   crouched when suppressed; `Crouch` and `Prone` as ordered) with
//!   the matching capsule heights; the stillness and shot timers advance;
//!   corpses count down and are removed.
//!
//! Detached men (S1b, `crate::unit`) engage by their own order (`Idle` and
//! `AttackMove` always, `Move` and `Dig` while still, `Retreat` never) and
//! ignore the squad focus; focus and mortar fire use the formation leader.
//!
//! No friendly fire in S1: rounds and blasts ignore the firing player's own
//! soldiers. Rounds have no drag, so the drop compensation is exact.

use frontline_content::WeaponClass;
use frontline_content::setup::{CombatSetup, HeSetup, WeaponSetup};
use frontline_content::vehicle::AmmoSetup;

use crate::armour::{self, ArmourHit, ArmourTables};
use crate::damage::{ArmourEv, FireRolls};
use crate::ecs::{
    Combat, Entities, EntityKind, PlayerId, SimId, Stance, Transform, Velocity, Weapon,
};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{MatchState, NavPhase, Order, PathReq, SquadId};
use crate::hash::HashCache;
use crate::items::{BlastKind, Kit, Thrown};
use crate::projectile::{NO_AMMO, Projectile, ProjectileEnd, SUBSTEP_HZ, advance, finish};
use crate::rng::{Purpose, RngStream};
use crate::spatial::SpatialHash;
use crate::systems::FireRequest;
use crate::terrain::Heightfield;
use crate::unit::{StanceOrder, Unit, UnitOrder};
use crate::vehicle::VehicleDerived;
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
/// A burst more than this above the ground under it carves no crater (mm).
const CRATER_LIFT_MM: i32 = 500;

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

/// Engage rule of a soldier: his own order if detached, else his squad's.
pub(crate) fn engage_of(squad: &Order, u: &Unit) -> Engage {
    if !u.detached {
        return engage(squad);
    }
    match u.order {
        UnitOrder::Retreat => Engage::Never,
        UnitOrder::Move { .. } | UnitOrder::Dig { .. } => Engage::Still,
        UnitOrder::Idle | UnitOrder::AttackMove { .. } => Engage::Always,
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
            // Vehicles aim through their own gunnery (S3).
            if sq.members.is_empty()
                || ty.vehicle.is_some()
                || w.he.is_some()
                || engage(&sq.order) == Engage::Never
            {
                sq.focus = None;
                continue;
            }
            let lead = ents.transform[sq.lead(ents).index()].pos.xy();
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
    pub unit: &'a [Unit],
    pub kit: &'a [Kit],
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
        let aim = ctx.marks[m as usize].aim;
        (ctx.terrain.line_of_sight(from, aim)
            && !ctx
                .game
                .smoke_blocks(from, aim, ctx.tick, (id, SimId(i as u32))))
        .then_some(id)
    })
}

/// Triangular roll in `[-s, s]`.
#[inline]
pub(crate) fn tri(r: &mut RngStream, s: Fx) -> Fx {
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
pub(crate) fn shot_velocity(
    from: FxVec3,
    aim: FxVec3,
    speed: Fx,
    spread: Fx,
    r: &mut RngStream,
) -> FxVec3 {
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
pub(crate) fn round_ttl(w: &WeaponSetup) -> u16 {
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
        let u = &ctx.unit[i];
        // A man winding up a throw does not fire.
        let may = w.he.is_none()
            && ctx.kit[i].windup.is_none()
            && match engage_of(&sq.order, u) {
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
                let focus = if u.detached { None } else { sq.focus };
                search(ctx, i, from, r, focus, LOS_TRIES, 0)
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
            ammo: crate::projectile::NO_AMMO,
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
            if sq.members.is_empty() {
                continue;
            }
            // The formation leader fires; with every man detached, nobody.
            let gunner = sq.lead(ents);
            let Order::Barrage { target, rounds } = &mut sq.order else {
                continue;
            };
            if sq.nav.phase != NavPhase::Holding
                || *rounds == 0
                || ents.unit[gunner.index()].detached
            {
                continue;
            }
            let ty = &setup.squad_types[sq.type_idx as usize];
            let w = &setup.weapons[ty.weapon as usize];
            if w.he.is_none() {
                continue;
            }
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
                ammo: crate::projectile::NO_AMMO,
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
/// then small arms while more than [`crate::projectile::SHELL_RESERVE`] slots
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
            if !he && pool.free_count() <= crate::projectile::SHELL_RESERVE {
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
                ammo: r.ammo,
                origin: r.pos,
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

/// What a projectile did to soldiers, vehicles or the ground (merged in
/// slot order).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Blow {
    Hit(SimId),
    NearMiss(SimId),
    /// A shell burst at this point (on the surface for a ground hit; at the
    /// hit point on a soldier).
    Burst(FxVec3),
    /// A shell met a vehicle's or wreck's plate (S3). The Damage stage
    /// rolls everything (`crate::damage`); a bursting round bursts there
    /// too.
    Armour {
        vehicle: SimId,
        hit: ArmourHit,
        /// The round's velocity at impact.
        vel: FxVec3,
        /// Its muzzle point (penetration range).
        origin: FxVec3,
    },
    /// A small-arms round stopped on a vehicle or wreck: no damage, no
    /// event (counted only).
    Stopped(SimId),
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) struct BallisticEvent {
    pub weapon: u16,
    pub player: u8,
    /// The round's ammo (`NO_AMMO` for small arms and mortars), its pool
    /// slot (RNG keys) and shooter.
    pub ammo: u16,
    pub slot: u32,
    pub shooter: SimId,
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
    /// S3: vehicles and wrecks (`None` when there are none), their plates,
    /// and the rounds.
    pub vehicles: Option<&'a VehicleDerived>,
    pub armour: &'a ArmourTables,
    pub ammo: &'a [AmmoSetup],
}

/// A soldier near a round's segment this substep.
struct Near {
    id: SimId,
    centre: FxVec3,
    /// Unclamped position of his closest approach along the segment.
    s: Fx,
    dist_sq: Fx,
}

/// The broadphase pad around a vehicle's hull origin: its horizontal reach
/// plus the height of hull and turret (m).
const VEHICLE_SPHERE_PAD_M: i32 = 3;

/// The first vehicle or wreck box the segment `from → from + d` enters,
/// among those not owned by `player` (wrecks belong to nobody): `(t, id,
/// hit)`, earliest first, ties by id. One bounding-sphere test per nearby
/// vehicle, then at most two box slab tests (counted in `tests`).
fn first_box(
    ctx: &FlightCtx,
    vd: &VehicleDerived,
    from: FxVec3,
    d: FxVec3,
    player: u8,
    tests: &mut u64,
) -> Option<(Fx, SimId, ArmourHit)> {
    let mid = from + d.div_int(2);
    let pad = Fx::from_int(VEHICLE_SPHERE_PAD_M);
    let reach = d.length().div_int(2) + vd.max_reach + pad;
    let mut best: Option<(Fx, SimId, ArmourHit)> = None;
    vd.index.for_each_near(mid.xy(), reach, |id| {
        let Some(f) = vd.frame(id) else { return };
        if !f.wreck && f.owner.0 == player {
            return;
        }
        let Some(a) = ctx.armour.vehicles.get(f.type_idx as usize) else {
            return;
        };
        if !armour::segment_near(from, d, f.origin, f.reach + pad) {
            return;
        }
        *tests += 2;
        if let Some(h) = armour::hit_armour(from, d, f, a)
            && best.is_none_or(|(bt, bid, _)| (h.t, id) < (bt, bid))
        {
            best = Some((h.t, id, h));
        }
    });
    best
}

/// The tie rule between a box hit and a capsule hit at segment fractions
/// `box_t` and `capsule_t`: the earlier wins, the box on a tie. (The
/// terrain test that follows includes its end point, so terrain wins a tie
/// with either; ties within boxes or capsules go to the lower id.)
#[inline]
fn box_before(box_t: Option<Fx>, capsule_t: Option<Fx>) -> bool {
    box_t.is_some_and(|b| capsule_t.is_none_or(|c| b <= c))
}

/// Integrates one chunk of match projectile slots for `substeps` substeps
/// (see the module docs). Finished projectiles are cleared and reported in
/// `ended`; their effects go to `events`, both in slot order. `box_tests`
/// counts the armour slab tests.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fly(
    chunk: usize,
    slots: &mut [Option<Projectile>],
    ctx: &FlightCtx,
    substeps: u32,
    ended: &mut Vec<(u32, ProjectileEnd)>,
    events: &mut Vec<BallisticEvent>,
    box_tests: &mut u64,
) -> bool {
    let base = chunk * crate::ecs::CHUNK_SIZE;
    let ents = ctx.ents;
    let mut near: Vec<Near> = Vec::new();
    let mut changed = false;
    for (j, slot) in slots.iter_mut().enumerate() {
        let Some(p) = slot else { continue };
        changed = true;
        let slot_id = (base + j) as u32;
        let shell = p.ammo != NO_AMMO;
        // Mortar shells test only the ground and vehicles (as in S1);
        // main-gun shells and rockets also soldiers, like small arms.
        let mortar = !shell
            && ctx
                .weapons
                .get(p.weapon as usize)
                .is_some_and(|w| w.he.is_some());
        let bursts = mortar
            || ctx
                .ammo
                .get(p.ammo as usize)
                .is_some_and(|a| a.mechanism.bursts());
        let (weapon, player, shooter, ammo) = (p.weapon, p.player, p.owner, p.ammo);
        let ev = |blow| BallisticEvent {
            weapon,
            player,
            ammo,
            slot: slot_id,
            shooter,
            blow,
        };
        for _ in 0..substeps {
            let from = p.pos;
            let next = advance(p);
            let d = next - from;
            let len = d.length();
            let boxed = ctx
                .vehicles
                .and_then(|vd| first_box(ctx, vd, from, d, player, box_tests));
            // Soldiers near the segment, nearest capsule hit first.
            let mut hit: Option<(Fx, SimId)> = None;
            near.clear();
            if !mortar {
                let mid = from + d.div_int(2);
                let reach = len.div_int(2) + ctx.near_miss + Fx::from_int(2);
                let len_sq = d.dot(d);
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
            }
            // Earliest hit wins; at equal t: terrain, then box, then
            // capsule. A box hit re-tests the terrain in front of it at
            // 0.25 m steps (a crest just in front of a hull blocks).
            let box_first = box_before(boxed.map(|b| b.0), hit.map(|h| h.0));
            let mut end_s = Fx::ONE;
            let ground = if box_first {
                let (bt, _, _) = boxed.expect("box first");
                end_s = bt;
                ctx.terrain.segment_hit_fine(from, from + d * bt)
            } else {
                if let Some((s, _)) = hit {
                    end_s = s;
                }
                ctx.terrain.segment_hit(from, from + d * end_s)
            };
            if let Some(g) = ground {
                end_s = if len > Fx::ZERO {
                    (g - from).length().div(len)
                } else {
                    Fx::ZERO
                };
            }
            let capsule_hit = if ground.is_none() && !box_first {
                hit
            } else {
                None
            };
            let box_hit = if ground.is_none() && box_first {
                boxed
            } else {
                None
            };
            // Near misses: passed (closest approach inside the segment and
            // before the round stopped), or close to where it struck earth.
            let nm_sq = ctx.near_miss.mul(ctx.near_miss);
            for n in &near {
                if Some(n.id) == capsule_hit.map(|h| h.1) {
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
                if bursts {
                    let surface = FxVec3::new(g.x, g.y, ctx.terrain.height_at(g.xy()));
                    events.push(ev(Blow::Burst(surface)));
                }
                ended.push((slot_id, ProjectileEnd::GroundHit));
                *slot = None;
                break;
            }
            if let Some((_, vid, h)) = box_hit {
                p.pos = h.point;
                let blow = if shell || mortar {
                    Blow::Armour {
                        vehicle: vid,
                        hit: h,
                        vel: p.vel,
                        origin: p.origin,
                    }
                } else {
                    Blow::Stopped(vid)
                };
                events.push(ev(blow));
                ended.push((slot_id, ProjectileEnd::VehicleHit));
                *slot = None;
                break;
            }
            if let Some((s, id)) = capsule_hit {
                p.pos = from + d * s;
                events.push(ev(if bursts {
                    Blow::Burst(p.pos)
                } else {
                    Blow::Hit(id)
                }));
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
    /// Weapon index (shells) or item index (frags, satchels).
    pub weapon: u16,
    pub kind: BlastKind,
}

/// One HE burst: a mortar shell or a thrown item going off.
pub(crate) struct Burst<'a> {
    /// On the surface.
    pub pos: FxVec3,
    pub player: u8,
    pub he: &'a HeSetup,
    pub damage: u32,
    /// Suppression points at the blast point.
    pub suppression: u32,
    /// Satchel: the crater levels trenches (`Heightfield::crater_cut`).
    pub cut: bool,
    /// Crater depth roll.
    pub key: (SimId, Purpose),
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

/// Takes `dmg` hit points from soldier `id`; one who drops to 0 joins
/// `dead` with the player who killed him.
fn hurt(
    ents: &mut Entities,
    hash: &mut HashCache,
    dead: &mut Vec<(SimId, u8)>,
    id: SimId,
    dmg: u32,
    by: u8,
) {
    let c = &mut ents.combat[id.index()];
    if c.hp == 0 {
        return;
    }
    c.hp = c.hp.saturating_sub(dmg.min(u16::MAX as u32) as u16);
    if c.hp == 0 {
        dead.push((id, by));
    }
    hash.mark_entity_chunk(id.chunk());
}

/// Damages and suppresses the enemies of `b.player` around a burst and
/// carves its crater (see the module docs).
fn burst(ctx: &mut DamageCtx, b: &Burst, dead: &mut Vec<(SimId, u8)>) {
    let pos = b.pos;
    let lift = FxVec3::new(pos.x, pos.y, pos.z + Fx::from_milli(BLAST_LIFT_MM));
    let he = b.he;
    let (lethal, reach): (Fx, Fx) = (he.lethal_radius.into(), he.damage_radius.into());
    let supp_r: Fx = he.suppression_radius.into();
    let mut ids = Vec::new();
    ctx.spatial
        .for_each_near(pos.xy(), supp_r, |id| ids.push(id));
    for id in ids {
        let i = id.index();
        if ctx.ents.kind[i] != EntityKind::Soldier || ctx.ents.owner[i].0 == b.player {
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
        let mut add = (b.suppression as i64 * 1000 * (supp_r - dist).raw() as i64
            / supp_r.raw().max(1) as i64) as u32;
        if !clear {
            add /= 2;
        }
        add_suppression(&mut ctx.ents.combat[i], add);
        ctx.hash.mark_entity_chunk(id.chunk());
        if clear && dist < reach {
            let dmg = if dist <= lethal {
                b.damage
            } else {
                (b.damage as i64 * (reach - dist).raw() as i64
                    / (reach - lethal).raw().max(1) as i64) as u32
            };
            hurt(ctx.ents, ctx.hash, dead, id, dmg, b.player);
        }
    }
    // A burst on a vehicle (or a man) well above the ground leaves it as
    // it is (S3).
    if pos.z > ctx.terrain.height_at(pos.xy()) + Fx::from_milli(CRATER_LIFT_MM) {
        return;
    }
    let mut rng = RngStream::new(ctx.seed, ctx.tick, b.key.0, b.key.1);
    let pct = rng.range_i32(CRATER_DEPTH_PCT.0, CRATER_DEPTH_PCT.1);
    let depth = Fx::from(he.crater_depth).mul_int(pct).div_int(100);
    if b.cut {
        ctx.terrain
            .crater_cut(pos.xy(), he.crater_radius.into(), depth, ctx.tick);
    } else {
        ctx.terrain
            .crater(pos.xy(), he.crater_radius.into(), depth, ctx.tick);
    }
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
    /// Applies this tick's ballistic events in order, then the thrown items
    /// that go off now (`crate::items`, in id order: blasts, smoke), then
    /// kills soldiers left at 0 HP (see the module docs). Bursts are
    /// reported in `explosions`; `fused` is scratch.
    pub(crate) fn apply_blows(
        &mut self,
        ctx: &mut DamageCtx,
        events: &[BallisticEvent],
        explosions: &mut Vec<Explosion>,
        fused: &mut Vec<Thrown>,
        armour_events: &mut Vec<ArmourEv>,
    ) {
        let rules = self.setup.rules.combat.clone();
        // (victim, killer) in the order they fell.
        let mut dead: Vec<(SimId, u8)> = Vec::new();
        let mut fires = FireRolls::default();
        for (k, e) in events.iter().enumerate() {
            let burst_at = match e.blow {
                Blow::Hit(id) => {
                    if ctx.ents.kind[id.index()] != EntityKind::Soldier {
                        continue;
                    }
                    let w = &self.setup.weapons[e.weapon as usize];
                    ctx.stats.soldier_hits += 1;
                    add_suppression(
                        &mut ctx.ents.combat[id.index()],
                        rules.hit_suppression * 1000,
                    );
                    hurt(ctx.ents, ctx.hash, &mut dead, id, w.damage, e.player);
                    None
                }
                Blow::NearMiss(id) => {
                    let w = &self.setup.weapons[e.weapon as usize];
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
                    None
                }
                Blow::Stopped(_) => {
                    ctx.stats.rounds_stopped += 1;
                    None
                }
                Blow::Burst(pos) => Some(pos),
                Blow::Armour { .. } => self.armour_blow(ctx, e, &mut fires, armour_events),
            };
            let Some(pos) = burst_at else { continue };
            // Mortar shells burst by their weapon's HE, main-gun HE and
            // rockets by their round's blast.
            let blast = match self.setup.ammo.get(e.ammo as usize) {
                Some(a) => a
                    .blast
                    .as_ref()
                    .map(|b| (b.blast.clone(), b.damage, b.suppression)),
                None => {
                    let w = &self.setup.weapons[e.weapon as usize];
                    w.he.clone().map(|h| (h, w.damage, w.suppression))
                }
            };
            let Some((he, damage, suppression)) = blast else {
                continue;
            };
            ctx.stats.explosions += 1;
            explosions.push(Explosion {
                pos,
                weapon: e.weapon,
                kind: BlastKind::Shell,
            });
            let b = Burst {
                pos,
                player: e.player,
                he: &he,
                damage,
                suppression,
                cut: false,
                key: (SimId(k as u32), Purpose::CRATER),
            };
            burst(ctx, &b, &mut dead);
        }
        self.take_fused(fused);
        for t in fused.iter() {
            let it = &self.setup.items[t.item as usize];
            if let Some(h) = &it.he {
                ctx.stats.explosions += 1;
                let kind = if h.cuts_trenches {
                    BlastKind::Satchel
                } else {
                    BlastKind::Frag
                };
                explosions.push(Explosion {
                    pos: t.pos,
                    weapon: t.item,
                    kind,
                });
                let b = Burst {
                    pos: t.pos,
                    player: t.owner.0,
                    he: &h.blast,
                    damage: h.damage,
                    suppression: h.suppression,
                    cut: h.cuts_trenches,
                    key: (SimId(t.id), Purpose::ITEM_CRATER),
                };
                burst(ctx, &b, &mut dead);
            }
        }
        for t in fused.iter() {
            if self.setup.items[t.item as usize].smoke.is_some() {
                self.start_smoke(t.item, t.pos);
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
        self.sweep_vehicle_refs(ctx.ents);
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
        ents.unit[i] = Unit::default();
        ents.kit[i] = Kit::default();
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
        for q in &mut self.path_queue.queues {
            q.retain(|r| *r != PathReq::Unit(id));
        }
        if sq.members.is_empty() {
            self.squads.remove(&sid);
            for q in &mut self.path_queue.queues {
                q.retain(|r| *r != PathReq::Squad(sid));
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
    pub unit: &'a [Unit],
    pub kit: &'a [Kit],
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
            EntityKind::Vacant | EntityKind::Vehicle | EntityKind::Wreck => continue,
        }
        changed = true;
        let sq = ctx.squad[i].and_then(|s| ctx.game.squad(s));
        let u = &ctx.unit[i];
        let retreating = if u.detached {
            u.order == UnitOrder::Retreat
        } else {
            sq.is_some_and(|s| s.order == Order::Retreat)
        };
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
        // Winding up a throw counts as aiming: up on the fire step.
        let aiming = (c.target.is_some()
            && (ctx.weapon[i].cooldown <= aim_ticks || c.since_shot < aim_ticks))
            || ctx.kit[i].windup.is_some();
        let crouched = points >= rules.crouch_suppression;
        let (stand, crouch, prone) = (
            Fx::from_milli(STAND_MM),
            Fx::from_milli(CROUCH_MM),
            Fx::from_milli(PRONE_MM),
        );
        let step = (base_z + parapet + exposure - pos.z).max(stand);
        // Suppression only ever lowers the ordered stance.
        (c.stance, c.height, c.fire_height) = if c.pinned {
            (Stance::Prone, prone, prone)
        } else if c.in_trench {
            if aiming {
                (Stance::Stand, step, step)
            } else if u.stance == StanceOrder::Prone {
                (Stance::Prone, prone, step)
            } else {
                (Stance::Crouch, crouch, step)
            }
        } else {
            match u.stance {
                StanceOrder::Auto
                    if weapon.is_some_and(|w| {
                        w.class == WeaponClass::Mg && c.still as u32 >= w.setup_ticks
                    }) =>
                {
                    // A set-up MG fires lying down.
                    (Stance::Prone, prone, prone)
                }
                StanceOrder::Auto | StanceOrder::Stand if !crouched => {
                    (Stance::Stand, stand, stand)
                }
                StanceOrder::Auto | StanceOrder::Stand | StanceOrder::Crouch => {
                    (Stance::Crouch, crouch, crouch)
                }
                StanceOrder::Prone => (Stance::Prone, prone, prone),
            }
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
    fn earliest_hit_tie_rule() {
        let h = Fx::HALF;
        assert!(box_before(Some(h), Some(h)), "box before capsule on a tie");
        assert!(box_before(Some(h), Some(h + Fx::EPSILON)));
        assert!(!box_before(Some(h + Fx::EPSILON), Some(h)));
        assert!(box_before(Some(Fx::ONE), None));
        assert!(!box_before(None, Some(h)));
        // Terrain wins a tie: the re-test includes the segment's end, so a
        // box face lying exactly on the ground surface is reached as ground.
        let t = Heightfield::generate(TerrainKind::Flat, 256, 1);
        let z = t.height_at(FxVec2::from_int(100, 100));
        let a = FxVec3::new(Fx::from_int(90), Fx::from_int(100), z + Fx::ONE);
        let b = FxVec3::new(Fx::from_int(100), Fx::from_int(100), z);
        assert_eq!(t.segment_hit_fine(a, b), Some(b));
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
                unit: &self.ents.unit,
                kit: &self.ents.kit,
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
                ammo: crate::projectile::NO_AMMO,
                origin: from,
            })];
            let ctx = FlightCtx {
                ents: &self.ents,
                spatial: &self.spatial,
                terrain: &self.terrain,
                weapons: &setup.weapons,
                near_miss: setup.rules.combat.near_miss.into(),
                vehicles: None,
                armour: self.game.armour(),
                ammo: &setup.ammo,
            };
            let (mut ended, mut events) = (Vec::new(), Vec::new());
            for _ in 0..60 {
                fly(0, &mut slots, &ctx, 2, &mut ended, &mut events, &mut 0);
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
                ammo: NO_AMMO,
                slot: 0,
                shooter: SimId(0),
                blow: Blow::Burst(pos),
            }];
            self.game.apply_blows(
                &mut ctx,
                &ev,
                &mut Vec::new(),
                &mut Vec::new(),
                &mut Vec::new(),
            );
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
            ammo: crate::projectile::NO_AMMO,
            origin: FxVec3::ZERO,
        })];
        let ctx = FlightCtx {
            ents: &r.ents,
            spatial: &r.spatial,
            terrain: &r.terrain,
            weapons: &r.game.setup().weapons,
            near_miss: Fx::from_int(2),
            vehicles: None,
            armour: r.game.armour(),
            ammo: &r.game.setup().ammo,
        };
        let (mut ended, mut events) = (Vec::new(), Vec::new());
        fly(0, &mut slots, &ctx, 2, &mut ended, &mut events, &mut 0);
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
            ammo: NO_AMMO,
            slot: 0,
            shooter: SimId(0),
            blow: Blow::NearMiss(man),
        };
        r.game.apply_blows(
            &mut ctx,
            &[ev; 3],
            &mut Vec::new(),
            &mut Vec::new(),
            &mut Vec::new(),
        );
        let per = r.game.setup().weapons[mg].suppression;
        assert_eq!(r.ents.combat[man.index()].suppression, 3 * per * 1000);
        assert_eq!(stats.near_misses, 3);
    }

    #[test]
    fn stance_orders_hold_unless_suppression_forces_lower() {
        let mut r = rig(false);
        let man = r.put(0, 500, 500);
        let rules = r.game.setup().rules.combat.clone();
        let decay = rules.suppression_decay_milli;
        let check = |r: &mut Rig, order: StanceOrder, points: u32, want: Stance| {
            r.ents.unit[man.index()].stance = order;
            r.ents.combat[man.index()].suppression = points * 1000 + decay;
            r.status();
            let c = r.ents.combat[man.index()];
            assert_eq!(c.stance, want, "{order:?} at {points}");
            let h = match want {
                Stance::Stand => STAND_MM,
                Stance::Crouch => CROUCH_MM,
                Stance::Prone => PRONE_MM,
            };
            assert_eq!(
                (c.height, c.fire_height),
                (Fx::from_milli(h), Fx::from_milli(h))
            );
        };
        let (crouch, pin) = (rules.crouch_suppression, rules.pin_suppression);
        use Stance as S;
        use StanceOrder as O;
        for (order, calm, shaken, pinned) in [
            (O::Auto, S::Stand, S::Crouch, S::Prone),
            (O::Stand, S::Stand, S::Crouch, S::Prone),
            (O::Crouch, S::Crouch, S::Crouch, S::Prone),
            (O::Prone, S::Prone, S::Prone, S::Prone),
        ] {
            check(&mut r, order, 0, calm);
            check(&mut r, order, crouch, shaken);
            check(&mut r, order, pin, pinned);
        }
        // An ordered stance beats the set-up MG's automatic prone.
        let mg = r
            .game
            .setup()
            .squad_types
            .iter()
            .position(|t| r.game.setup().weapons[t.weapon as usize].class == WeaponClass::Mg)
            .unwrap();
        r.game.squads.get_mut(&r.squad).unwrap().type_idx = mg as u16;
        r.ents.combat[man.index()].still = u16::MAX;
        check(&mut r, O::Auto, 0, S::Prone);
        check(&mut r, O::Crouch, 0, S::Crouch);
    }

    #[test]
    fn stance_orders_in_a_trench_still_crouch_and_rise_to_fire() {
        let mut r = rig(true);
        let man = r.put(0, TRENCH_X, 500);
        for order in [
            StanceOrder::Auto,
            StanceOrder::Stand,
            StanceOrder::Crouch,
            StanceOrder::Prone,
        ] {
            r.ents.unit[man.index()].stance = order;
            r.ents.combat[man.index()].target = None;
            r.status();
            let low = r.ents.combat[man.index()];
            let want = if order == StanceOrder::Prone {
                Stance::Prone
            } else {
                Stance::Crouch
            };
            assert!(low.in_trench && low.stance == want, "{order:?}: {low:?}");
            // Below the lip: near misses count half.
            assert!(covered(&low), "{order:?}");
            // Rises to the fire step to shoot, whatever the order.
            r.ents.combat[man.index()].target = Some(SimId(9999));
            r.status();
            let up = r.ents.combat[man.index()];
            assert_eq!(up.stance, Stance::Stand, "{order:?}");
            assert_eq!(up.height, low.fire_height, "{order:?}");
            assert_eq!(up.height, up.fire_height);
        }
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
                unit: &r.ents.unit,
                kit: &r.ents.kit,
                game: &r.game,
                terrain: &r.terrain,
            };
            status(0, &ctx, &mut r.ents.combat, &mut gone);
        }
        assert_eq!(gone.len(), n);
    }
}
