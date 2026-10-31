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
//!
//! Seeking cover on arrival (trench cells, terrain between the man and the
//! enemy) comes with spotting and trenches in later stages.

use crate::ecs::{Entities, SimId};
use crate::fx::{Fx, FxVec2};
use crate::game::{Ctx, MatchState, NavPhase, Order, Squad};
use crate::terrain::Heightfield;

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

/// Assigns `slots` (ordered along `axis`) to members by where they stand
/// along `axis`, ties by id. Returns the slot of each member, in member order.
fn assign_by_axis(
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

/// Arrival slots and facing for a squad reaching its goal.
fn arrival(
    sq: &Squad,
    ents: &Entities,
    terrain: &Heightfield,
    spacing: Fx,
    base: FxVec2,
) -> (Vec<FxVec2>, FxVec2) {
    let n = sq.members.len();
    let (slots, facing, axis) = match &sq.order {
        Order::Dig { line } => {
            // Spread along the trench line, facing away from the own base.
            let len = polyline_length(line);
            let slots: Vec<FxVec2> = (0..n)
                .map(|i| point_along(line, len.mul_int(2 * i as i32 + 1).div_int(2 * n as i32)))
                .collect();
            let along = (line[line.len() - 1] - line[0]).normalize_or_zero();
            let mut facing = along.perp();
            if facing.dot_raw(point_along(line, len.div_int(2)) - base) < 0 {
                facing = -facing;
            }
            (slots, facing, along)
        }
        _ => {
            let facing = sq.nav.heading;
            let slots = line_slots(sq.nav.goal, facing, n, spacing);
            (slots, facing, -facing.perp())
        }
    };
    let slots: Vec<FxVec2> = slots.into_iter().map(|p| terrain.clamp(p)).collect();
    (assign_by_axis(&sq.members, ents, &slots, axis), facing)
}

/// Staggered-column slot `k` (`k >= 1`) behind a leader at `lead` heading
/// `f`: two files, one row per two men.
fn column_slot(lead: FxVec2, f: FxVec2, k: usize, spacing: Fx) -> FxVec2 {
    let row = k.div_ceil(2) as i32;
    let side = if k % 2 == 1 { 1 } else { -1 };
    lead - f * spacing.mul_int(row) + f.perp() * spacing.mul_int(side).div_int(2)
}

/// Writes a member's target and speed (only marking the chunk on change).
fn set_target(ctx: &mut Ctx, m: SimId, target: FxVec2, speed: Fx) {
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
        for (&id, sq) in self.squads.iter_mut() {
            if sq.members.is_empty() {
                continue;
            }
            let ty = &self.setup.squad_types[sq.type_idx as usize];
            let spacing: Fx = ty.spacing.into();
            let mut speed: Fx = ty.speed.into();
            if sq.order == Order::Retreat {
                speed = speed.mul_int(mv.retreat_speed_pct as i32).div_int(100);
            }
            let fast = speed.mul_int(mv.catch_up_speed_pct as i32).div_int(100);
            let lead = sq.members[0];
            let lpos = ctx.ents.transform[lead.index()].pos.xy();
            sq.nav.phase_ticks = sq.nav.phase_ticks.saturating_add(1);
            match sq.nav.phase {
                // Men keep their last targets until the path arrives.
                NavPhase::PathPending => {}
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
                            let base = {
                                let b = self.setup.players[sq.owner.0 as usize].base as usize;
                                self.setup.map.bases[b].center.into()
                            };
                            let (slots, facing) = arrival(sq, ctx.ents, ctx.terrain, spacing, base);
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
                    for k in 1..sq.members.len() {
                        let m = sq.members[k];
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
                        set_target(ctx, m, target, if gap > catch_sq { fast } else { speed });
                    }
                    let lead_speed = if strung_out {
                        speed.mul_int(STRETCHED_SPEED_PCT).div_int(100)
                    } else {
                        speed
                    };
                    set_target(ctx, lead, wp, lead_speed);
                }
                NavPhase::Arriving | NavPhase::Holding => {
                    let yaw = Fx::atan2(sq.nav.facing.y, sq.nav.facing.x);
                    let mut all_there = true;
                    for (k, &m) in sq.members.iter().enumerate() {
                        let slot = sq.nav.slots[k];
                        let gap = (slot - ctx.ents.transform[m.index()].pos.xy()).length_sq();
                        let there = gap <= arrive_sq;
                        all_there &= there;
                        set_target(ctx, m, slot, if gap > catch_sq { fast } else { speed });
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
