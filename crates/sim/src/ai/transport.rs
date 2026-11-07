//! S8a AI habits for half-tracks (`docs/slices/S8a-halftracks.md` 7;
//! `[ai.transport]`). One saved job per own transport ([`AiTransport`],
//! keyed by the vehicle's squad), in phases:
//!
//! 1. **Load.** A free line, engineer or MG squad of at least half strength
//!    within `PICK_M` of the vehicle whose assigned point lies at least
//!    `min_ride_m` + the walked last stretch from the squad, and at least
//!    `min_ride_m` farther than the vehicle's door, and that fits gets
//!    `Embark` (the newest first). Once
//!    everyone is aboard the job rides; with nobody aboard after `wait_s`
//!    the vehicle goes to support alone (and with some aboard it rides).
//! 2. **Ride** to `dismount_back_m` + `APPROACH_EXTRA_M` short of the
//!    squad's point. Every plan: everyone out at once (over the sides) when
//!    the vehicle is under fire (`MatchState::under_fire`); by the door when
//!    the ground `dismount_back_m` ahead lies within `dismount_sight_m` of a
//!    known enemy with a clear line (at most `los_rays` lines), when it
//!    arrives, or when it has stood still on the way for `stuck_s`.
//! 3. **Unload.** Until the last man is out; the squad is then free for the
//!    usual infantry orders (its point stays its task).
//! 4. **Support** for `support_s`: the vehicle is left to the S7 overwatch
//!    habit (hull-down or stand-off spot, the MG fights from there).
//! 5. **Return** to the base, then Load again.
//!
//! **No loops** (S7 tick fix): a move that ends with the vehicle idle
//! farther than `arrive_m` from its goal is a failed move; after
//! `max_fails` the job takes where the vehicle stands as reached (Unload
//! there, or Load there). No job sends the same move twice within
//! `repath_s`.

use serde::{Deserialize, Serialize};

use super::{AiKind, Known, Mine, MyTank, PointEval, sq_m};
use crate::command::CommandKind;
use crate::ecs::SimId;
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{Order, SquadId};

/// Squads within this of the vehicle may be loaded (m).
const PICK_M: i32 = 120;
/// A returning vehicle stops for a squad within this (m).
const MEET_M: i32 = 40;
/// Walking speed allowed for a picked squad to reach the door (mm/s): its
/// walk is added to `wait_s`.
const WALK_MM_S: i64 = 1000;
/// The ride ends this much farther short of the point than
/// `dismount_back_m` (m): the men walk the last stretch.
const APPROACH_EXTRA_M: i32 = 20;
/// Eye heights for the sight lines (mm): an enemy's and the riders' (the
/// rim).
const ENEMY_EYE_MM: i32 = 1500;
const RIDER_EYE_MM: i32 = 1800;
/// Most transport jobs one AI keeps.
pub const MAX_TRANSPORTS: usize = 32;

/// Most open ride measurements one AI keeps ([`AiRideMeas`]).
pub const MAX_RIDE_MEAS: usize = 32;
/// A ride measurement still open this long after the order is dropped (s).
const MEAS_TIMEOUT_S: u64 = 600;

/// S8a Fix (review M2): one transported squad's trip, measured from the
/// `Embark` order to its lead within `capture_radius` of the point it rode
/// for, against walking straight there from where it stood at the order
/// (report only: `AiCounters::obj_*`).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AiRideMeas {
    /// Tick of the `Embark` order (when it was planned).
    pub order_tick: u64,
    /// The squad lead's position at the order, and the point.
    pub from: FxVec2,
    pub point: FxVec2,
    /// A man of the squad has been seen aboard.
    pub rode: bool,
}

/// A transport job's phase.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum TransportPhase {
    #[default]
    Load = 0,
    Ride = 1,
    Unload = 2,
    Support = 3,
    Return = 4,
}

/// One own transport's job (module docs).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct AiTransport {
    pub phase: TransportPhase,
    /// The infantry squad it carries or waits for.
    pub squad: Option<SquadId>,
    /// Where it drives (Ride: the drop-off; Return: the base).
    pub goal: FxVec2,
    /// Tick the phase began.
    pub since: u64,
    /// Failed moves in this phase.
    pub fails: u8,
    /// The last move sent, and when.
    pub moved_to: FxVec2,
    pub moved_at: u64,
    /// Tick the vehicle was last seen moving on the way.
    pub moving_at: u64,
}

impl super::Planner<'_> {
    /// Step A of the plan (before the vehicles' upkeep): runs every own
    /// transport's job and takes the transports that are not supporting out
    /// of `tanks`. Returns the transports waiting for a squad.
    pub(super) fn transport_jobs(
        &mut self,
        tanks: &mut Vec<MyTank>,
        known: &[Known],
        busy: &[SquadId],
    ) -> Vec<MyTank> {
        self.ride_meas_update();
        let g = self.g;
        let me = self.me;
        self.ai.transports.retain(|id, _| {
            g.squads.get(id).is_some_and(|s| {
                s.owner == me
                    && s.members
                        .first()
                        .is_some_and(|v| g.transport_of(*v).is_some())
            })
        });
        let mut keep = Vec::with_capacity(tanks.len());
        let mut waiting = Vec::new();
        for t in tanks.drain(..) {
            if g.transport_of(t.id).is_none() {
                keep.push(t);
                continue;
            }
            if busy.contains(&t.squad) {
                // A command for it is still pending: leave it alone, and to
                // nobody else.
                if self
                    .ai
                    .transports
                    .get(&t.squad)
                    .is_some_and(|j| j.phase == TransportPhase::Support)
                {
                    keep.push(t);
                }
                continue;
            }
            if self.ai.transports.len() >= MAX_TRANSPORTS
                && !self.ai.transports.contains_key(&t.squad)
            {
                keep.push(t);
                continue;
            }
            let tick = self.tick;
            let mut job = self
                .ai
                .transports
                .get(&t.squad)
                .copied()
                .unwrap_or(AiTransport {
                    since: tick,
                    ..AiTransport::default()
                });
            self.run_job(&t, &mut job, known);
            self.ai.transports.insert(t.squad, job);
            match job.phase {
                TransportPhase::Support => keep.push(t),
                TransportPhase::Load if job.squad.is_none() => waiting.push(t),
                _ => {}
            }
        }
        *tanks = keep;
        waiting
    }

    /// Step B (after the points are assigned): loads a squad into each
    /// waiting transport (module docs, Load). Returns the squads taken.
    pub(super) fn transport_loads(
        &mut self,
        waiting: &[MyTank],
        mine: &[Mine],
        assigned: &[(usize, usize)],
        points: &[PointEval],
        claimed: &[SquadId],
    ) -> Vec<SquadId> {
        let g = self.g;
        let a = &g.setup.rules.ai.transport;
        let mut taken: Vec<SquadId> = Vec::new();
        for t in waiting {
            let Some(cap) = g.transport_of(t.id).map(|x| x.slots.len()) else {
                continue;
            };
            let free = g
                .vehicle(t.id)
                .map_or(0, |vs| vs.riders.iter().filter(|r| r.is_none()).count());
            let pick = sq_m(Fx::from_int(PICK_M));
            let min_ride = Fx::from_int(a.min_ride_m.min(30_000) as i32);
            // Review M1: measured from the squad, not the vehicle. The point
            // lies at least the ride plus the walked last stretch from the
            // squad, and the ride gains at least `min_ride_m` over the walk
            // to the door (no short hops, no walking backwards to board).
            let last_leg = Fx::from_int(a.dismount_back_m.min(30_000) as i32 + APPROACH_EXTRA_M);
            let worth = |lead: FxVec2, point: FxVec2| {
                let d_pl = (point - lead).length();
                let d_lv = (lead - t.pos).length();
                d_pl >= min_ride + last_leg && d_pl - d_lv >= min_ride
            };
            let mut best: Option<(SquadId, usize)> = None;
            for &(mi, pi) in assigned {
                let m = &mine[mi];
                let Some(sq) = g.squads.get(&m.id) else {
                    continue;
                };
                if !matches!(m.role, AiKind::Rifle | AiKind::Engineer | AiKind::Mg)
                    || m.strength_pct < 50
                    || claimed.contains(&m.id)
                    || taken.contains(&m.id)
                    || sq.members.len() > free.min(cap)
                    || (m.lead - t.pos).length_sq_raw() > pick
                    || !worth(m.lead, points[pi].pos)
                    || matches!(sq.order, Order::Retreat | Order::Embark { .. })
                {
                    continue;
                }
                // The newest squad first.
                if best.is_none_or(|(b, _)| m.id > b) {
                    best = Some((m.id, pi));
                }
            }
            let Some((sid, pi)) = best else { continue };
            self.ai.emit(
                self.due,
                CommandKind::Embark {
                    squads: vec![sid],
                    vehicle: t.id,
                },
            );
            self.ai.counters.transport_loads += 1;
            taken.push(sid);
            let at = points[pi].pos;
            let lead0 = mine.iter().find(|m| m.id == sid).map_or(t.pos, |m| m.lead);
            if self.ai.ride_meas.len() < MAX_RIDE_MEAS || self.ai.ride_meas.contains_key(&sid) {
                self.ai.ride_meas.insert(
                    sid,
                    AiRideMeas {
                        order_tick: self.tick,
                        from: lead0,
                        point: at,
                        rode: false,
                    },
                );
            }
            let dir = (at - t.pos).normalize_or_zero();
            let back = Fx::from_int(a.dismount_back_m as i32 + APPROACH_EXTRA_M);
            let goal = self.terrain.clamp(at - dir * back);
            // The wait counts from when it should be at the door.
            let lead = mine.iter().find(|m| m.id == sid).map_or(t.pos, |m| m.lead);
            let walk_s =
                ((lead - t.pos).length().raw().max(0) as i64 * 1000 / WALK_MM_S) >> Fx::FRAC_BITS;
            let hz = g.setup.tick_hz.max(1) as u64;
            if let Some(job) = self.ai.transports.get_mut(&t.squad) {
                job.squad = Some(sid);
                job.goal = goal;
                job.since = self.tick + walk_s.max(0) as u64 * hz;
                job.fails = 0;
            }
            self.task(sid, super::TaskKind::Capture, pi as u8, at);
        }
        taken
    }

    /// Review M2: closes the open ride measurements ([`AiRideMeas`]): a
    /// squad that rode and whose lead is within `capture_radius` of its
    /// point adds (order to arrival) and (straight walk from the order
    /// position to the edge of the radius) to the counters; a squad that
    /// never boarded is dropped; one gone or past `MEAS_TIMEOUT_S` after
    /// riding counts as lost.
    fn ride_meas_update(&mut self) {
        let g = self.g;
        let tick = self.tick;
        let hz = g.setup.tick_hz.max(1) as u64;
        let radius: Fx = g.setup.rules.capture_radius.into();
        let keys: Vec<SquadId> = self.ai.ride_meas.keys().copied().collect();
        for sid in keys {
            let m = self.ai.ride_meas[&sid];
            let Some(sq) = g.squads.get(&sid).filter(|q| !q.members.is_empty()) else {
                if m.rode {
                    self.ai.counters.obj_lost += 1;
                }
                self.ai.ride_meas.remove(&sid);
                continue;
            };
            let aboard = sq
                .members
                .iter()
                .any(|u| self.ents.unit()[u.index()].riding.is_some());
            let embarking = matches!(sq.order, Order::Embark { .. });
            if !m.rode && !aboard {
                // The order is applied a plan later; a squad no longer
                // walking to its vehicle never boarded.
                if !embarking && tick > m.order_tick + 5 * hz {
                    self.ai.ride_meas.remove(&sid);
                }
                continue;
            }
            if !m.rode
                && let Some(e) = self.ai.ride_meas.get_mut(&sid)
            {
                e.rode = true;
            }
            let lead = self.ents.transform[sq.lead(self.ents).index()].pos.xy();
            if !aboard && (lead - m.point).length_sq_raw() <= sq_m(radius) {
                let walk_mm_s = {
                    let ty = &g.setup.squad_types[sq.type_idx as usize];
                    ((Fx::from(ty.speed).raw().max(0) as u64 * 1000) >> Fx::FRAC_BITS).max(1)
                };
                let dist = ((m.point - m.from).length() - radius).max(Fx::ZERO);
                let dist_mm = (dist.raw().max(0) as u64 * 1000) >> Fx::FRAC_BITS;
                let c = &mut self.ai.counters;
                c.obj_rides += 1;
                c.obj_ride_ticks += tick.saturating_sub(m.order_tick);
                c.obj_walk_ticks += dist_mm * hz / walk_mm_s;
                self.ai.ride_meas.remove(&sid);
            } else if tick > m.order_tick + MEAS_TIMEOUT_S * hz {
                self.ai.counters.obj_lost += 1;
                self.ai.ride_meas.remove(&sid);
            }
        }
    }

    /// One plan of transport `t`'s job (module docs).
    fn run_job(&mut self, t: &MyTank, job: &mut AiTransport, known: &[Known]) {
        let g = self.g;
        let a = &g.setup.rules.ai.transport;
        let hz = g.setup.tick_hz.max(1) as u64;
        let tick = self.tick;
        let riders = g.riders_of(t.id).len();
        let sq = &g.squads[&t.squad];
        let idle = sq.order == Order::Idle;
        let base = g.base_of(self.me);
        let set = |job: &mut AiTransport, phase: TransportPhase| {
            job.phase = phase;
            job.since = tick;
            job.fails = 0;
        };
        // Its squad gone or no longer walking to it or riding it.
        if let Some(s) = job.squad {
            let ok = g
                .squads
                .get(&s)
                .is_some_and(|q| q.order == (Order::Embark { vehicle: t.id }));
            if !ok && riders == 0 {
                job.squad = None;
            }
        }
        match job.phase {
            TransportPhase::Load => {
                if riders > 0 && job.squad.is_some() {
                    let all = job
                        .squad
                        .and_then(|s| g.squads.get(&s))
                        .is_some_and(|q| q.members.len() == riders);
                    if all || tick >= job.since + a.wait_s as u64 * hz {
                        set(job, TransportPhase::Ride);
                        self.ai.counters.transport_rides += 1;
                        self.drive(t, job, job.goal);
                    }
                } else if tick >= job.since + 3 * a.wait_s as u64 * hz
                    || (tick >= job.since + a.wait_s as u64 * hz
                        && !self.infantry_coming()
                        && !(job.squad.is_none() && self.squad_waits(t.id, t.pos, PICK_M)))
                {
                    // Nobody came: support alone, then try again; a squad
                    // still walking to it stops (free for other orders).
                    if let Some(sq) = job.squad.filter(|&q| {
                        g.squads
                            .get(&q)
                            .is_some_and(|x| x.order == (Order::Embark { vehicle: t.id }))
                    }) {
                        self.ai.emit(self.due, CommandKind::Stop { squad: sq });
                    }
                    job.squad = None;
                    set(job, TransportPhase::Support);
                }
            }
            TransportPhase::Ride => {
                if riders == 0 {
                    set(job, TransportPhase::Support);
                    return;
                }
                let speed = g.vehicle(t.id).map_or(Fx::ZERO, |vs| vs.speed.abs());
                if speed > Fx::from_ratio(1, 2) {
                    job.moving_at = tick;
                }
                let fire = g.under_fire(self.ents, t.id, tick);
                let near =
                    (t.pos - job.goal).length_sq_raw() <= sq_m(Fx::from_int(a.arrive_m as i32));
                let stuck = tick >= job.moving_at.max(job.since) + a.stuck_s as u64 * hz;
                let failed = idle && !near;
                if failed {
                    job.fails = job.fails.saturating_add(1);
                    self.ai.counters.transport_fails += 1;
                }
                let give_up = job.fails as u32 >= a.max_fails;
                if fire || (idle && near) || give_up || stuck || self.exposed_ahead(t, job, known) {
                    self.ai.emit(
                        self.due,
                        CommandKind::Disembark {
                            vehicles: vec![t.id],
                            over_sides: fire,
                        },
                    );
                    self.ai.counters.transport_unloads += 1;
                    if fire {
                        self.ai.counters.transport_unloads_fire += 1;
                    }
                    set(job, TransportPhase::Unload);
                } else if failed {
                    self.drive(t, job, job.goal);
                }
            }
            TransportPhase::Unload => {
                if riders == 0 {
                    job.squad = None;
                    set(job, TransportPhase::Support);
                } else if g.vehicle(t.id).is_some_and(|vs| vs.unload.is_none())
                    && tick >= job.since + 2 * hz
                {
                    // The order was lost (refused while moving): again.
                    self.ai.emit(
                        self.due,
                        CommandKind::Disembark {
                            vehicles: vec![t.id],
                            over_sides: true,
                        },
                    );
                    job.since = tick;
                }
            }
            TransportPhase::Support => {
                // Back for the next squad once one waits near the base;
                // else it keeps supporting.
                if tick >= job.since + a.support_s as u64 * hz
                    && (self.squad_waits(t.id, base, 2 * PICK_M) || self.infantry_coming())
                {
                    job.goal = base;
                    set(job, TransportPhase::Return);
                    self.drive(t, job, base);
                }
            }
            TransportPhase::Return => {
                let s = &g.setup;
                let b = &s.map.bases[s.players[self.me.0 as usize].base as usize];
                let in_base =
                    (t.pos - FxVec2::from(b.center)).length_sq_raw() <= sq_m(Fx::from(b.radius));
                let near =
                    (t.pos - job.goal).length_sq_raw() <= sq_m(Fx::from_int(a.arrive_m as i32));
                // A squad on its way met on the road is loaded there.
                if in_base || (idle && near) || self.squad_waits(t.id, t.pos, MEET_M) {
                    if !idle && !in_base {
                        self.ai.emit(self.due, CommandKind::Stop { squad: t.squad });
                    }
                    set(job, TransportPhase::Load);
                    job.squad = None;
                } else if idle {
                    job.fails = job.fails.saturating_add(1);
                    self.ai.counters.transport_fails += 1;
                    if job.fails as u32 >= a.max_fails {
                        // Load where it stands.
                        set(job, TransportPhase::Load);
                        job.squad = None;
                    } else {
                        self.drive(t, job, job.goal);
                    }
                }
            }
        }
    }

    /// Whether an own line, engineer or MG squad of at least half strength
    /// that fits transport `v`, not walking to or riding one, stands within
    /// `r_m` of `at` with its task point farther than `min_ride_m` (the
    /// previous plan's tasks).
    fn squad_waits(&self, v: SimId, at: FxVec2, r_m: i32) -> bool {
        let g = self.g;
        let cap = g.transport_of(v).map_or(0, |t| t.slots.len());
        let r = sq_m(Fx::from_int(r_m));
        let ride = sq_m(Fx::from_int(
            g.setup.rules.ai.transport.min_ride_m.min(30_000) as i32,
        ));
        g.squads.iter().any(|(id, sq)| {
            let ty = &g.setup.squad_types[sq.type_idx as usize];
            sq.owner == self.me
                && !sq.members.is_empty()
                && sq.members.len() <= cap
                && sq.members.len() * 2 >= ty.men.max(1) as usize
                && matches!(
                    super::ai_kind(&g.setup, sq.type_idx),
                    AiKind::Rifle | AiKind::Engineer | AiKind::Mg
                )
                && !matches!(sq.order, Order::Retreat | Order::Embark { .. })
                && self.ai.tasks.get(id).is_some_and(|t| {
                    let lead = self.ents.transform[sq.lead(self.ents).index()].pos.xy();
                    (lead - at).length_sq_raw() <= r && (t.target - lead).length_sq_raw() > ride
                })
        })
    }

    /// Whether an infantry call-in of this AI is pending (a squad about to
    /// appear in the base).
    fn infantry_coming(&self) -> bool {
        let s = &self.g.setup;
        self.ai.pending.iter().any(|c| {
            matches!(c.kind, CommandKind::CallIn { squad_type }
                if s.squad_types.get(squad_type as usize).is_some_and(|t| t.vehicle.is_none())
                    && matches!(super::ai_kind(s, squad_type), AiKind::Rifle | AiKind::Engineer | AiKind::Mg))
        })
    }

    /// A move order for the transport, never the same one within
    /// `repath_s`.
    fn drive(&mut self, t: &MyTank, job: &mut AiTransport, goal: FxVec2) {
        let a = &self.g.setup.rules.ai.transport;
        let hz = self.g.setup.tick_hz.max(1) as u64;
        if job.moved_to == goal && self.tick < job.moved_at + a.repath_s as u64 * hz {
            return;
        }
        self.ai.emit(
            self.due,
            CommandKind::Move {
                squad: t.squad,
                target: goal,
            },
        );
        job.moved_to = goal;
        job.moved_at = self.tick;
        job.moving_at = self.tick;
    }

    /// Whether the ground `dismount_back_m` ahead of transport `t` (toward
    /// its goal) lies within `dismount_sight_m` of a known enemy with a
    /// clear line (nearest enemies first, at most `los_rays` lines).
    fn exposed_ahead(&self, t: &MyTank, job: &AiTransport, known: &[Known]) -> bool {
        let a = &self.g.setup.rules.ai.transport;
        let dir = (job.goal - t.pos).normalize_or_zero();
        let ahead = self
            .terrain
            .clamp(t.pos + dir * Fx::from_int(a.dismount_back_m as i32));
        let sight = sq_m(Fx::from_int(a.dismount_sight_m.min(30_000) as i32));
        let mut near: Vec<(u64, SimId, FxVec2)> = known
            .iter()
            .map(|k| ((k.pos - ahead).length_sq_raw(), k.id, k.pos))
            .filter(|(d, _, _)| *d <= sight)
            .collect();
        if near.is_empty() {
            return false;
        }
        if a.los_rays == 0 {
            return true;
        }
        near.sort_unstable_by_key(|(d, id, _)| (*d, *id));
        let to = ahead.extend(self.terrain.height_at(ahead) + Fx::from_milli(RIDER_EYE_MM));
        near.iter().take(a.los_rays as usize).any(|&(_, _, p)| {
            let from: FxVec3 = p.extend(self.terrain.height_at(p) + Fx::from_milli(ENEMY_EYE_MM));
            self.terrain.line_of_sight(from, to)
        })
    }
}
