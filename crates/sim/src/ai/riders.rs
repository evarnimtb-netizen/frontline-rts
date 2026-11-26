//! S15-ai A1 (docs/slices/S15-ussr.md 7; `[ai.riders]`, every number a key
//! there): infantry riding a tank's deck to the attack. Plans draw no RNG
//! and read jobs, tanks and squads in ascending order. Runs only with
//! `[ai.riders] jobs` 1 and `[riders] enabled` 1 (no rider slots exist
//! otherwise, so no job can start).
//!
//! * **Want** (after the points are assigned): fewer than `squads_max`
//!   jobs, a non-own point the armour planner sends its tanks to
//!   (`vehicle_point`), an own free vehicle with rider slots that attacks
//!   (a tank; not one that holds overwatch: assault guns, tank destroyers,
//!   `ai_overwatch` heavies) in the own base (idle or leaving it, not
//!   retreating), with an empty
//!   deck, no job and mobile; and an own squad in the own base
//!   with no job, half strength or more, that fits the free slots (cards
//!   tagged `vehicle_riders` first, then `line`; the nearest to the tank,
//!   then the lowest id) and was not refused with that tank within
//!   `recheck_s` (L3). Tanks are tried in ascending id. The squad gets
//!   `Embark`; a moving tank gets `Stop`.
//! * **Board**: the tank waits (out of the tank habits). Everyone aboard:
//!   **Ride**. Nobody aboard and the squad no longer boarding (refused,
//!   ordered away), or nobody aboard after `stall_s`: the job ends as a
//!   stall (L2, L3). Some aboard after `stall_s`: it rides with them.
//! * **Ride**: the tank is back in the tank habits, which send it to the
//!   attack. The deck is told off (`Disembark` over the sides; **Drop**,
//!   the tank waits again) once the tank is within `drop_m` of the job's
//!   point, or stands idle at its habit's goal (within [`ARRIVE_M`]).
//!   R3 (the riders jumped off) ends the job as forced off.
//! * **Drop**: ends when the deck is empty (dropped); the squad is then a
//!   free squad and joins the point's attack through the assignment. A
//!   deck not empty [`DROP_WAIT_S`] after every slot's exit time ends as a
//!   stall.
//! * **Ends** (`AiCounters::rider_job_ends`, by
//!   [`crate::s15::RIDER_END_NAMES`]): dropped; forced off (the deck
//!   emptied by R3 or a forced exit while riding, or the squad gone);
//!   tank lost (gone, wrecked, abandoned, not own, or sent back to the
//!   base to retreat or rearm: the deck is told off); deadline (L1:
//!   `job_max_s` after the job began, the deck told off); stall (L2: the
//!   board above, the tank standing still `stall_s` on the way, the deck
//!   told off; or a drop that does not empty). Stalls are remembered as
//!   refused pairings (L3).
//!
//! `RiderJob::phase_at` is the tick the job began while it boards and
//! rides (L1 counts from it) and the tick of the drop order in Drop (no
//! layout change: the plan's layout commit had these five fields only).

use super::{Mine, MyTank, PointEval, Rel, TaskKind, sq_m};
use crate::command::CommandKind;
use crate::ecs::{EntityKind, SimId};
use crate::fx::{Fx, FxVec2};
use crate::game::{Order, SquadId};
use crate::s15::{
    MAX_RIDER_JOBS, MAX_RIDER_REFUSED, RIDER_BOARD, RIDER_DROP, RIDER_RIDE, RiderJob, RiderRefused,
};
use frontline_content::VehicleRole;

/// `AiCounters::rider_job_ends` indices (`crate::s15::RIDER_END_NAMES`).
pub const END_DROPPED: usize = 0;
pub const END_FORCED: usize = 1;
pub const END_TANK_LOST: usize = 2;
pub const END_DEADLINE: usize = 3;
pub const END_STALL: usize = 4;
/// A tank this close to its habit's goal, idle, has arrived (m).
pub const ARRIVE_M: i32 = 20;
/// A drop whose deck is not empty this long after every slot's exit time
/// is a stall (s).
pub const DROP_WAIT_S: u64 = 5;

/// What one plan decided for a job.
enum Step {
    Keep(RiderJob),
    End(usize),
}

impl super::Planner<'_> {
    /// Whether the rider jobs run (`[ai.riders] jobs` and `[riders]
    /// enabled`).
    fn riders_on(&self) -> bool {
        let r = &self.g.setup.rules;
        r.ai.riders.jobs != 0 && r.riders.enabled != 0
    }

    /// Whether vehicle `id` is one the armour planner sends to attack (a
    /// tank that does not hold overwatch) and has rider slots.
    fn rider_tank(&self, id: SimId) -> bool {
        let g = self.g;
        let Some(vs) = g.vehicles.get(&id) else {
            return false;
        };
        let v = &g.setup.vehicles[vs.type_idx as usize];
        let heavy = v.ai_overwatch && g.setup.rules.ai.armour.heavy_overwatch != 0;
        v.role == VehicleRole::Tank && !heavy && g.rider_slots_of(id)
    }

    /// Whether `at` lies in the own base.
    fn at_own_base(&self, at: FxVec2) -> bool {
        let s = &self.g.setup;
        let base = &s.map.bases[s.players[self.me.0 as usize].base as usize];
        (at - FxVec2::from(base.center)).length_sq_raw() <= sq_m(base.radius.into())
    }

    /// Before the tank habits (after the transport jobs): runs every rider
    /// job, ends those that are over and takes the tanks that board or drop
    /// out of `tanks`. Jobs whose squad or tank has a command pending wait.
    pub(super) fn riders_continue(&mut self, tanks: &mut Vec<MyTank>, busy: &[SquadId]) {
        let hz = self.g.setup.tick_hz.max(1) as u64;
        let recheck = self.g.setup.rules.ai.riders.recheck_s as u64 * hz;
        let tick = self.tick;
        self.ai
            .riders
            .refused
            .retain(|r| tick < r.at.saturating_add(recheck));
        if self.ai.riders.jobs.is_empty() {
            return;
        }
        let jobs = std::mem::take(&mut self.ai.riders.jobs);
        let mut keep = Vec::with_capacity(jobs.len());
        for j in jobs {
            let tank_sq = self.ents.squad.get(j.tank.index()).copied().flatten();
            if busy.contains(&j.squad) || tank_sq.is_some_and(|q| busy.contains(&q)) {
                keep.push(j);
                continue;
            }
            match self.rider_step(j, tank_sq) {
                Step::Keep(j) => {
                    if j.phase != RIDER_RIDE {
                        tanks.retain(|t| t.id != j.tank);
                    }
                    keep.push(j);
                }
                Step::End(why) => {
                    self.ai.counters.rider_job_ends[why] += 1;
                    // S15 Fix (review 4): a squad still told to board the
                    // tank is stopped, so that the habits take it again
                    // (`Order::Embark` keeps it out of the free list).
                    if self
                        .g
                        .squads
                        .get(&j.squad)
                        .is_some_and(|q| q.order == (Order::Embark { vehicle: j.tank }))
                    {
                        self.ai.emit(self.due, CommandKind::Stop { squad: j.squad });
                    }
                    if why == END_STALL {
                        self.rider_refuse(j.tank, j.squad);
                    }
                }
            }
        }
        self.ai.riders.jobs = keep;
    }

    /// One job's plan (module docs).
    fn rider_step(&mut self, mut j: RiderJob, tank_sq: Option<SquadId>) -> Step {
        let g = self.g;
        let a = &g.setup.rules.ai.riders;
        let hz = g.setup.tick_hz.max(1) as u64;
        let i = j.tank.index();
        let vs = g.vehicles.get(&j.tank).filter(|vs| {
            self.ents.kind.get(i) == Some(&EntityKind::Vehicle)
                && self.ents.owner[i] == self.me
                && vs.wreck_tick.is_none()
                && vs.abandoned.is_none()
        });
        let aboard = g.riders_of(j.tank).len();
        let men = g.squads.get(&j.squad).map_or(0, |q| q.members.len());
        let Some(vs) = vs else {
            return Step::End(END_TANK_LOST);
        };
        let Some(tank_sq) = tank_sq else {
            return Step::End(END_TANK_LOST);
        };
        if men == 0 {
            return Step::End(END_FORCED);
        }
        let age = self.tick.saturating_sub(j.phase_at);
        let tank_order = g.squads.get(&tank_sq).map(|q| q.order.clone());
        match j.phase {
            RIDER_BOARD => {
                if age >= a.job_max_s as u64 * hz {
                    self.rider_off(j.tank, aboard);
                    return Step::End(END_DEADLINE);
                }
                let boarding = g
                    .squads
                    .get(&j.squad)
                    .is_some_and(|q| q.order == (Order::Embark { vehicle: j.tank }));
                if aboard >= men {
                    j.phase = RIDER_RIDE;
                    return Step::Keep(j);
                }
                if aboard == 0 && (!boarding || age >= a.stall_s as u64 * hz) {
                    return Step::End(END_STALL);
                }
                if age >= a.stall_s as u64 * hz {
                    // Some aboard: ride with them (the others walk on).
                    j.phase = RIDER_RIDE;
                    return Step::Keep(j);
                }
                if tank_order.is_some_and(|o| o != Order::Idle) {
                    self.ai.emit(self.due, CommandKind::Stop { squad: tank_sq });
                }
                Step::Keep(j)
            }
            RIDER_RIDE => {
                if aboard == 0 {
                    return Step::End(END_FORCED);
                }
                let task = self.ai.tasks.get(&tank_sq).copied();
                if matches!(tank_order, Some(Order::Retreat))
                    || task.is_some_and(|t| matches!(t.kind, TaskKind::Retreat | TaskKind::Rearm))
                {
                    self.rider_off(j.tank, aboard);
                    return Step::End(END_TANK_LOST);
                }
                if age >= a.job_max_s as u64 * hz {
                    self.rider_off(j.tank, aboard);
                    return Step::End(END_DEADLINE);
                }
                let pos = self.ents.transform[i].pos.xy();
                let point: FxVec2 = g
                    .setup
                    .map
                    .points
                    .get(j.point as usize)
                    .map_or(pos, |p| p.pos.into());
                let near = (point - pos).length_sq_raw()
                    <= sq_m(Fx::from_int(a.drop_m.min(30_000) as i32));
                let arrived = tank_order == Some(Order::Idle)
                    && task.is_some_and(|t| {
                        (t.target - pos).length_sq_raw() <= sq_m(Fx::from_int(ARRIVE_M))
                    });
                if near || arrived {
                    self.rider_off(j.tank, aboard);
                    j.phase = RIDER_DROP;
                    j.phase_at = self.tick;
                    return Step::Keep(j);
                }
                // Still for `stall_s` after it moved during the job (a tank
                // that stood since before the job has not set out yet).
                let still = vs.still_ticks as u64;
                if still >= a.stall_s as u64 * hz && still < age {
                    self.rider_off(j.tank, aboard);
                    return Step::End(END_STALL);
                }
                Step::Keep(j)
            }
            _ => {
                if aboard == 0 {
                    return Step::End(END_DROPPED);
                }
                // Every slot's exit time (over the sides where the deck
                // has it), then the wait.
                let per_man = g.transport_of(j.tank).map_or(0, |t| {
                    t.exit_over_sides_ticks
                        .unwrap_or(t.exit_ticks)
                        .max(t.exit_ticks) as u64
                });
                if age >= DROP_WAIT_S * hz + per_man * vs.riders.len() as u64 {
                    self.rider_off(j.tank, aboard);
                    return Step::End(END_STALL);
                }
                if vs.unload.is_none() {
                    self.rider_off(j.tank, aboard);
                }
                Step::Keep(j)
            }
        }
    }

    /// Tells the deck of `tank` off over the sides (nothing with nobody
    /// aboard).
    fn rider_off(&mut self, tank: SimId, aboard: usize) {
        if aboard == 0 {
            return;
        }
        self.ai.emit(
            self.due,
            CommandKind::Disembark {
                vehicles: vec![tank],
                over_sides: true,
            },
        );
    }

    /// L3: remembers a failed pairing (full: the oldest goes).
    fn rider_refuse(&mut self, tank: SimId, squad: SquadId) {
        let r = &mut self.ai.riders.refused;
        if r.len() >= MAX_RIDER_REFUSED {
            r.remove(0);
        }
        r.push(RiderRefused {
            tank,
            squad,
            at: self.tick,
        });
    }

    /// After the points are assigned, before the tank habits: new rider
    /// jobs (module docs, Want). Takes the boarding tanks out of `tanks`;
    /// returns the squads taken.
    pub(super) fn riders_start(
        &mut self,
        tanks: &mut Vec<MyTank>,
        mine: &[Mine],
        assigned: &[(usize, usize)],
        points: &[PointEval],
        hunting: &[SquadId],
    ) -> Vec<SquadId> {
        let mut taken = Vec::new();
        if !self.riders_on() {
            return taken;
        }
        let g = self.g;
        let s = &g.setup;
        let max = (s.rules.ai.riders.squads_max as usize).min(MAX_RIDER_JOBS);
        let Some(pi) = self.vehicle_point(points, assigned) else {
            return taken;
        };
        if points[pi].rel == Rel::Own {
            return taken;
        }
        let mut ids: Vec<(SimId, SquadId, FxVec2)> = tanks
            .iter()
            .filter(|t| !t.immobile)
            .map(|t| (t.id, t.squad, t.pos))
            .collect();
        ids.sort_unstable_by_key(|t| t.0);
        let riders_tag = s.tag("vehicle_riders");
        let line_tag = s.tag("line");
        for (tank, tank_sq, pos) in ids {
            if self.ai.riders.jobs.len() >= max {
                break;
            }
            if self.ai.riders.jobs.iter().any(|j| j.tank == tank)
                || !self.rider_tank(tank)
                || !self.at_own_base(pos)
                || g.squads
                    .get(&tank_sq)
                    .is_none_or(|q| q.order == Order::Retreat)
            {
                continue;
            }
            let Some(vs) = g.vehicles.get(&tank) else {
                continue;
            };
            if vs.unload.is_some() || vs.burn > 0 || vs.riders.iter().any(Option::is_some) {
                continue;
            }
            let free = vs.riders.len();
            let mut best: Option<(u8, u64, SquadId)> = None;
            for m in mine {
                let Some(sq) = g.squads.get(&m.id) else {
                    continue;
                };
                let ty = &s.squad_types[sq.type_idx as usize];
                let rank = if ty.has_tag(riders_tag) {
                    0
                } else if ty.has_tag(line_tag) {
                    1
                } else {
                    continue;
                };
                if m.strength_pct < 50
                    || sq.members.is_empty()
                    || sq.members.len() > free
                    || hunting.contains(&m.id)
                    || taken.contains(&m.id)
                    || self.ai.riders.jobs.iter().any(|j| j.squad == m.id)
                    || self
                        .ai
                        .riders
                        .refused
                        .iter()
                        .any(|r| r.tank == tank && r.squad == m.id)
                    || matches!(sq.order, Order::Retreat | Order::Embark { .. })
                    || !self.at_own_base(m.lead)
                {
                    continue;
                }
                let key = (rank, (m.lead - pos).length_sq_raw(), m.id);
                if best.is_none_or(|b| key < b) {
                    best = Some(key);
                }
            }
            let Some((_, _, sid)) = best else {
                continue;
            };
            self.ai.emit(
                self.due,
                CommandKind::Embark {
                    squads: vec![sid],
                    vehicle: tank,
                },
            );
            let job = RiderJob {
                tank,
                squad: sid,
                point: pi as u8,
                phase: RIDER_BOARD,
                phase_at: self.tick,
            };
            // The tank's old task is stale once it waits out of the tank
            // habits (the arrival test reads the task they give it next).
            self.ai.tasks.remove(&tank_sq);
            let at = self.ai.riders.jobs.partition_point(|j| j.tank < tank);
            self.ai.riders.jobs.insert(at, job);
            self.ai.counters.rider_jobs += 1;
            tanks.retain(|t| t.id != tank);
            taken.push(sid);
        }
        taken
    }
}
