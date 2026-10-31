//! Squad steering and formations (S1 "Squads and formation").
//!
//! Each tick, serially in ascending squad id, a squad turns its navigation
//! state into per-soldier targets (`Mover::waypoint` and `Mover::speed`);
//! the Movement stage then walks the men there with ORCA avoidance.
//!
//! * **Moving:** the leader (`members[0]`) walks the path. The others hold
//!   slots of a staggered column behind him, rotated to the leader's heading.
//!   They aim a little past their slot so they walk at the leader's pace,
//!   walk faster when they fall behind and wait when ahead; the leader slows
//!   down when the squad is strung out.
//! * **Arriving:** at the goal the men spread on a line across the move
//!   direction (or along the trench line for Dig). Slots are assigned by
//!   each man's position across the line, so paths do not cross.
//! * **Holding:** men stay on their slots and face `facing`.
//! * **AttackMove** squads that have a focus target (`crate::combat`) and at
//!   least one man with a target in clear line halt where each man stands
//!   until the fight is over, then walk on.
//!
//! Digging squads get their slots from `crate::dig` (each man's spot on his
//! share of the line).
//!
//! Detached men (S1b, `crate::unit`) are left out: the formation is the
//! members still in it, led by the first of them ([`Squad::lead`]). A man
//! ordered prone walks at the rules' `prone_speed_pct`.
//!
//! **Cover on arrival** (all orders but Dig and Retreat): each man, in
//! member order, moves his line slot to the nearest free trench spot within
//! [`COVER_RADIUS_M`] (on the floor: dug `combat::TRENCH_MM` below the base
//! surface there and [`TRENCH_MARGIN_MM`] around; sampled every 0.25 m);
//! failing that, if his owner sees an enemy within [`COVER_ENEMY_M`], to the
//! nearest spot on a 2.5 m grid with terrain between it and that enemy's
//! eyes (at most [`COVER_RAYS`] sight rays per man). Spots keep
//! [`COVER_GAP_MM`] apart, also from where other friendly squads stand.

use crate::combat::TRENCH_MM;
use crate::ecs::{Entities, PlayerId, SimId};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{Ctx, MatchState, NavPhase, Order, Squad};
use crate::terrain::Heightfield;
use crate::unit::StanceOrder;

/// The leader moves on to the next waypoint within this distance (m).
const WAYPOINT_REACH_M: i32 = 2;
/// A member this far from his slot is behind: he speeds up (m).
const CATCH_UP_M: i32 = 2;
/// A member this far from his slot makes the leader slow down (m).
const STRETCHED_M: i32 = 10;
/// Leader speed while the squad is strung out (percent).
const STRETCHED_SPEED_PCT: i32 = 70;
/// Members on the move aim this far past their (moving) slot, so they keep
/// walking with the leader instead of stopping at it every tick (mm).
const SLOT_LEAD_MM: i32 = 1500;
/// Arriving gives up waiting for stragglers after this long (s).
const ARRIVE_TIMEOUT_S: u32 = 20;
/// Men look for cover this far from their line slot (m).
const COVER_RADIUS_M: i32 = 10;
/// Only enemies this close to the goal count for terrain cover (m).
const COVER_ENEMY_M: i32 = 400;
/// Most sight rays one man spends looking for terrain cover.
const COVER_RAYS: usize = 12;
/// Cover spots of two men keep at least this far apart (mm).
const COVER_GAP_MM: i32 = 1200;
/// Trench spots are sampled on this grid (one detail cell, mm) ...
const TRENCH_STEP_MM: i32 = 250;
/// ... and need dug ground this far around them too (mm).
const TRENCH_MARGIN_MM: i32 = 400;
/// Terrain cover spots are sampled on this grid (mm).
const COVER_STEP_MM: i32 = 2500;
/// Enemy eye height and the covered man's chest height for cover rays (mm).
const COVER_EYE_MM: i32 = 1500;
const COVER_CHEST_MM: i32 = 1000;

/// Total length of a polyline.
pub(crate) fn polyline_length(line: &[FxVec2]) -> Fx {
    line.windows(2).fold(Fx::ZERO, |acc, w| {
        acc.saturating_add((w[1] - w[0]).length())
    })
}

/// Point at arc length `s` along `line` (clamped to its ends).
pub(crate) fn point_along(line: &[FxVec2], s: Fx) -> FxVec2 {
    let mut left = s;
    for w in line.windows(2) {
        let d = w[1] - w[0];
        let len = d.length();
        if left <= len {
            if len.is_zero() {
                return w[0];
            }
            return w[0] + d * left.div(len);
        }
        left -= len;
    }
    *line.last().expect("non-empty line")
}

/// `n` positions on a line through `center` across `facing` (a unit
/// vector), `spacing` apart, from the facing direction's right to left.
pub(crate) fn line_slots(center: FxVec2, facing: FxVec2, n: usize, spacing: Fx) -> Vec<FxVec2> {
    let right = -facing.perp();
    (0..n)
        .map(|i| {
            // Offset (i - (n-1)/2) · spacing, in halves to stay integral.
            let halves = 2 * i as i32 - (n as i32 - 1);
            center + right * spacing.mul_int(halves).div_int(2)
        })
        .collect()
}

/// Walking speed of a man with stance order `stance`.
pub(crate) fn stance_speed(speed: Fx, stance: StanceOrder, prone_pct: u32) -> Fx {
    if stance == StanceOrder::Prone {
        speed.mul_int(prone_pct as i32).div_int(100)
    } else {
        speed
    }
}

/// Assigns `slots` (ordered along `axis`) to members by where they stand
/// along `axis`, ties by id. Returns the slot of each member, in member order.
pub(crate) fn assign_by_axis(
    members: &[SimId],
    ents: &Entities,
    slots: &[FxVec2],
    axis: FxVec2,
) -> Vec<FxVec2> {
    let mut order: Vec<(i64, SimId, usize)> = members
        .iter()
        .enumerate()
        .map(|(k, &m)| (ents.transform[m.index()].pos.xy().dot_raw(axis), m, k))
        .collect();
    order.sort_unstable();
    let mut out = vec![FxVec2::ZERO; members.len()];
    for (slot, &(_, _, k)) in order.iter().enumerate() {
        out[k] = slots[slot];
    }
    out
}

/// Arrival slots and facing for a squad reaching its goal: slots for the
/// men in formation (`form`); detached men keep where they stand.
fn arrival(
    sq: &Squad,
    form: &[SimId],
    ents: &Entities,
    terrain: &Heightfield,
    spacing: Fx,
) -> (Vec<FxVec2>, FxVec2) {
    let n = form.len();
    let (slots, facing, axis) = match &sq.order {
        Order::Dig { line, job } => {
            // Spread along the trench line, facing the parapet side.
            let len = polyline_length(line);
            let slots: Vec<FxVec2> = (0..n)
                .map(|i| point_along(line, len.mul_int(2 * i as i32 + 1).div_int(2 * n as i32)))
                .collect();
            let along = (line[line.len() - 1] - line[0]).normalize_or_zero();
            (slots, job.facing(line), along)
        }
        _ => {
            let facing = sq.nav.heading;
            let slots = line_slots(sq.nav.goal, facing, n, spacing);
            (slots, facing, -facing.perp())
        }
    };
    let slots: Vec<FxVec2> = slots.into_iter().map(|p| terrain.clamp(p)).collect();
    let mut slots = assign_by_axis(form, ents, &slots, axis);
    if !matches!(sq.order, Order::Dig { .. } | Order::Retreat) {
        seek_cover(sq.owner, sq.nav.goal, form, ents, terrain, &mut slots);
    }
    // Back to member order.
    let mut out = Vec::with_capacity(sq.members.len());
    let mut k = 0;
    for &m in &sq.members {
        if form.get(k) == Some(&m) {
            out.push(slots[k]);
            k += 1;
        } else {
            out.push(ents.transform[m.index()].pos.xy());
        }
    }
    (out, facing)
}

/// Offsets within [`COVER_RADIUS_M`] on a grid of `step_mm`, nearest first
/// (ties by `(y, x)`), the zero offset included.
fn cover_offsets(step_mm: i32) -> Vec<FxVec2> {
    let r = COVER_RADIUS_M * 1000 / step_mm;
    let mut v: Vec<(i32, i32, i32)> = (-r..=r)
        .flat_map(|y| (-r..=r).map(move |x| (x * x + y * y, y, x)))
        .filter(|&(d, _, _)| d <= r * r)
        .collect();
    v.sort_unstable();
    v.into_iter()
        .map(|(_, y, x)| FxVec2::new(Fx::from_milli(x * step_mm), Fx::from_milli(y * step_mm)))
        .collect()
}

/// Trench and terrain cover search offsets (built once; pure data).
fn offsets() -> &'static (Vec<FxVec2>, Vec<FxVec2>) {
    static OFFSETS: std::sync::OnceLock<(Vec<FxVec2>, Vec<FxVec2>)> = std::sync::OnceLock::new();
    OFFSETS.get_or_init(|| (cover_offsets(TRENCH_STEP_MM), cover_offsets(COVER_STEP_MM)))
}

/// Whether a trench cell lies within reach of `p` (grid pre-check).
fn trench_near(terrain: &Heightfield, p: FxVec2) -> bool {
    let reach = Fx::from_int(COVER_RADIUS_M + crate::terrain::SPACING_M);
    let (x0, y0) = terrain.nearest_grid(p - FxVec2::new(reach, reach));
    let (x1, y1) = terrain.nearest_grid(p + FxVec2::new(reach, reach));
    (y0..=y1).any(|gy| (x0..=x1).any(|gx| terrain.is_trench(gx, gy)))
}

/// A trench spot: `p` and the four points [`TRENCH_MARGIN_MM`] around it
/// are all `combat::TRENCH_MM` below the base surface, so `p` is near the
/// middle of the floor and a man who stops within the arrive radius of it
/// still stands in the trench.
fn trench_spot(terrain: &Heightfield, p: FxVec2) -> bool {
    let deep = Fx::from_milli(TRENCH_MM);
    let m = Fx::from_milli(TRENCH_MARGIN_MM);
    let dug = |q: FxVec2| {
        terrain.has_detail_at(q) && terrain.base_height_at(q) - terrain.height_at(q) >= deep
    };
    dug(p)
        && [(m, Fx::ZERO), (-m, Fx::ZERO), (Fx::ZERO, m), (Fx::ZERO, -m)]
            .iter()
            .all(|&(x, y)| dug(p + FxVec2::new(x, y)))
}

/// Moves arrival `slots` of men `own` (of `owner`, arriving near `goal`)
/// into cover (see the module docs).
pub(crate) fn seek_cover(
    owner: PlayerId,
    goal: FxVec2,
    own: &[SimId],
    ents: &Entities,
    terrain: &Heightfield,
    slots: &mut [FxVec2],
) {
    let gap = (Fx::from_milli(COVER_GAP_MM).raw() as u64).pow(2);
    let free = |taken: &[FxVec2], p: FxVec2| taken.iter().all(|t| (*t - p).length_sq_raw() >= gap);
    let owner = owner.0;
    // Spots other friends stand on or walk to.
    let near_goal = (Fx::from_int(3 * COVER_RADIUS_M).raw() as u64).pow(2);
    let mut taken: Vec<FxVec2> = ents
        .soldiers()
        .filter(|id| {
            let i = id.index();
            ents.owner[i].0 == owner
                && !own.contains(id)
                && (ents.mover[i].waypoint - goal).length_sq_raw() <= near_goal
        })
        .map(|id| ents.mover[id.index()].waypoint)
        .collect();
    // Nearest enemy the owner sees, by distance to the goal (ties by id).
    let reach = (Fx::from_int(COVER_ENEMY_M).raw() as u64).pow(2);
    let enemy = ents
        .soldiers()
        .filter(|id| {
            let i = id.index();
            ents.owner[i].0 != owner && ents.combat[i].seen_by & (1 << owner) != 0
        })
        .map(|id| {
            let pos = ents.transform[id.index()].pos;
            ((pos.xy() - goal).length_sq_raw(), id, pos)
        })
        .filter(|(d, _, _)| *d <= reach)
        .min_by_key(|(d, id, _)| (*d, *id))
        .map(|(_, _, pos)| FxVec3::new(pos.x, pos.y, pos.z + Fx::from_milli(COVER_EYE_MM)));
    let (fine, coarse) = offsets();
    let trenches = slots.iter().any(|&p| trench_near(terrain, p));
    for slot in slots.iter_mut() {
        let in_trench = trenches
            .then(|| {
                fine.iter()
                    .map(|&o| *slot + o)
                    .find(|&p| terrain.contains(p) && trench_spot(terrain, p) && free(&taken, p))
            })
            .flatten();
        let pick = in_trench.or_else(|| {
            let eye = enemy?;
            coarse
                .iter()
                .map(|&o| *slot + o)
                .filter(|&p| terrain.contains(p) && free(&taken, p))
                .take(COVER_RAYS)
                .find(|&p| {
                    let chest = p.extend(terrain.height_at(p) + Fx::from_milli(COVER_CHEST_MM));
                    !terrain.line_of_sight(eye, chest)
                })
        });
        if let Some(p) = pick {
            *slot = p;
        }
        taken.push(*slot);
    }
}

/// Staggered-column slot `k` (`k >= 1`) behind a leader at `lead` heading
/// `f`: two files, one row per two men.
fn column_slot(lead: FxVec2, f: FxVec2, k: usize, spacing: Fx) -> FxVec2 {
    let row = k.div_ceil(2) as i32;
    let side = if k % 2 == 1 { 1 } else { -1 };
    lead - f * spacing.mul_int(row) + f.perp() * spacing.mul_int(side).div_int(2)
}

/// Writes a member's target and speed (only marking the chunk on change).
pub(crate) fn set_target(ctx: &mut Ctx, m: SimId, target: FxVec2, speed: Fx) {
    let mv = &mut ctx.ents.mover[m.index()];
    if mv.waypoint != target || mv.speed != speed {
        mv.waypoint = target;
        mv.speed = speed;
        ctx.hash.mark_entity_chunk(m.chunk());
    }
}

impl MatchState {
    /// Steers every squad for this tick (see the module docs).
    pub(crate) fn steer_squads(&mut self, ctx: &mut Ctx) {
        let hz = self.setup.tick_hz;
        let mv = &self.setup.rules.movement;
        let arrive = Fx::from(mv.arrive_radius);
        let arrive_sq = arrive.mul_int(2).mul(arrive.mul_int(2));
        let reach_sq = Fx::from_int(WAYPOINT_REACH_M * WAYPOINT_REACH_M);
        let catch_sq = Fx::from_int(CATCH_UP_M * CATCH_UP_M);
        let stretched_sq = Fx::from_int(STRETCHED_M * STRETCHED_M);
        let mut repath = Vec::new();
        let prone_pct = mv.prone_speed_pct;
        let mut form: Vec<SimId> = Vec::new();
        for (&id, sq) in self.squads.iter_mut() {
            form.clear();
            form.extend(
                sq.members
                    .iter()
                    .copied()
                    .filter(|m| !ctx.ents.unit[m.index()].detached),
            );
            // Every man detached: nobody to steer.
            let Some(&lead) = form.first() else {
                continue;
            };
            let ty = &self.setup.squad_types[sq.type_idx as usize];
            let spacing: Fx = ty.spacing.into();
            let mut speed: Fx = ty.speed.into();
            if sq.order == Order::Retreat {
                speed = speed.mul_int(mv.retreat_speed_pct as i32).div_int(100);
            }
            let fast = speed.mul_int(mv.catch_up_speed_pct as i32).div_int(100);
            // A man's own pace (prone men crawl).
            let pace = |ents: &Entities, m: SimId, v: Fx| {
                stance_speed(v, ents.unit[m.index()].stance, prone_pct)
            };
            let lpos = ctx.ents.transform[lead.index()].pos.xy();
            sq.nav.phase_ticks = sq.nav.phase_ticks.saturating_add(1);
            match sq.nav.phase {
                // Men keep their last targets until the path arrives.
                NavPhase::PathPending => {}
                // Halt only while someone can actually shoot: a man's target
                // always had a clear line from his muzzle when it was picked
                // and is re-checked once a second, whereas the focus needs no
                // line. Halting on the focus alone froze squads that saw each
                // other across a crest they could not shoot over.
                NavPhase::Moving
                    if matches!(sq.order, Order::AttackMove { .. })
                        && sq.focus.is_some()
                        && form
                            .iter()
                            .any(|m| ctx.ents.combat[m.index()].target.is_some()) =>
                {
                    for &m in &form {
                        let here = ctx.ents.transform[m.index()].pos.xy();
                        let v = pace(ctx.ents, m, speed);
                        set_target(ctx, m, here, v);
                    }
                }
                NavPhase::Moving => {
                    let nav = &mut sq.nav;
                    let last = nav.path.len() as u32 - 1;
                    while nav.next < last
                        && (nav.path[nav.next as usize] - lpos).length_sq() <= reach_sq
                    {
                        nav.next += 1;
                    }
                    let wp = nav.path[nav.next as usize];
                    let to_wp = wp - lpos;
                    if to_wp.length_sq_raw() > (Fx::from_milli(250).raw() as u64).pow(2) {
                        nav.heading = to_wp.normalize_or_zero();
                    }
                    if nav.next == last && to_wp.length_sq() <= reach_sq.max(arrive_sq) {
                        if nav.partial {
                            repath.push((id, nav.goal));
                        } else {
                            let (slots, facing) =
                                arrival(sq, &form, ctx.ents, ctx.terrain, spacing);
                            let nav = &mut sq.nav;
                            nav.slots = slots;
                            nav.facing = facing;
                            nav.phase = NavPhase::Arriving;
                            nav.phase_ticks = 0;
                        }
                        continue;
                    }
                    let f = nav.heading;
                    let lead_by = f * Fx::from_milli(SLOT_LEAD_MM);
                    let mut strung_out = false;
                    for (k, &m) in form.iter().enumerate().skip(1) {
                        let slot = column_slot(lpos, f, k, spacing);
                        let off = slot - ctx.ents.transform[m.index()].pos.xy();
                        let gap = off.length_sq();
                        strung_out |= gap > stretched_sq;
                        // Ahead of the slot (it is behind him): wait there.
                        let target = if off.dot_raw(f) < 0 {
                            slot
                        } else {
                            slot + lead_by
                        };
                        let target = ctx.terrain.clamp(target);
                        let v = pace(ctx.ents, m, if gap > catch_sq { fast } else { speed });
                        set_target(ctx, m, target, v);
                    }
                    let lead_speed = if strung_out {
                        speed.mul_int(STRETCHED_SPEED_PCT).div_int(100)
                    } else {
                        speed
                    };
                    let v = pace(ctx.ents, lead, lead_speed);
                    set_target(ctx, lead, wp, v);
                }
                NavPhase::Arriving | NavPhase::Holding => {
                    let yaw = Fx::atan2(sq.nav.facing.y, sq.nav.facing.x);
                    let mut all_there = true;
                    for (k, &m) in sq.members.iter().enumerate() {
                        if ctx.ents.unit[m.index()].detached {
                            continue;
                        }
                        let slot = sq.nav.slots[k];
                        let gap = (slot - ctx.ents.transform[m.index()].pos.xy()).length_sq();
                        let there = gap <= arrive_sq;
                        all_there &= there;
                        let v = pace(ctx.ents, m, if gap > catch_sq { fast } else { speed });
                        set_target(ctx, m, slot, v);
                        // In position: face the line's direction.
                        let t = &mut ctx.ents.transform[m.index()];
                        if there && t.yaw != yaw {
                            t.yaw = yaw;
                            ctx.hash.mark_entity_chunk(m.chunk());
                        }
                    }
                    let nav = &mut sq.nav;
                    if nav.phase == NavPhase::Arriving
                        && (all_there || nav.phase_ticks >= ARRIVE_TIMEOUT_S * hz)
                    {
                        nav.phase = NavPhase::Holding;
                        nav.phase_ticks = 0;
                        if matches!(
                            sq.order,
                            Order::Move { .. } | Order::AttackMove { .. } | Order::Retreat
                        ) {
                            sq.order = Order::Idle;
                        }
                    }
                }
            }
        }
        for (id, goal) in repath {
            self.request_path(id, goal);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_slots_are_centred_and_spaced() {
        let s = line_slots(
            FxVec2::from_int(100, 100),
            FxVec2::new(Fx::ZERO, Fx::ONE), // facing north: right is east
            4,
            Fx::from_int(3),
        );
        let xs: Vec<Fx> = s.iter().map(|p| p.x).collect();
        assert_eq!(
            xs,
            [
                Fx::from_milli(95_500),
                Fx::from_milli(98_500),
                Fx::from_milli(101_500),
                Fx::from_milli(104_500)
            ]
        );
        assert!(s.iter().all(|p| p.y == Fx::from_int(100)));
    }

    #[test]
    fn polyline_helpers() {
        let line = [
            FxVec2::from_int(0, 0),
            FxVec2::from_int(10, 0),
            FxVec2::from_int(10, 20),
        ];
        assert_eq!(polyline_length(&line), Fx::from_int(30));
        assert_eq!(
            point_along(&line, Fx::from_int(15)),
            FxVec2::from_int(10, 5)
        );
        assert_eq!(
            point_along(&line, Fx::from_int(99)),
            FxVec2::from_int(10, 20)
        );
    }

    #[test]
    fn column_slots_trail_the_leader() {
        let f = FxVec2::new(Fx::ONE, Fx::ZERO); // heading east
        let lead = FxVec2::from_int(50, 50);
        let s1 = column_slot(lead, f, 1, Fx::from_int(2));
        let s2 = column_slot(lead, f, 2, Fx::from_int(2));
        let s3 = column_slot(lead, f, 3, Fx::from_int(2));
        assert_eq!(s1, FxVec2::from_int(48, 51));
        assert_eq!(s2, FxVec2::from_int(48, 49));
        assert_eq!(s3, FxVec2::from_int(46, 51));
    }
}
