//! S8b AI habits for recon cars' buys and supply trucks
//! (`docs/slices/S8b-trucks-recon.md` 9; `[ai.recon]`, `[ai.truck]`). One
//! saved job per own truck ([`AiTruck`], keyed by the vehicle's squad):
//!
//! * **Load.** Stopped in the base the sim loads and charges it; the job
//!   leaves at its target (`load_shells_kg` of shells, the def's other
//!   kinds), or at `min_load_pct` of it after `load_wait_s` or once the
//!   crate budget (`[ai.supply] shell_mp_max_pct`) is spent.
//! * **Follow.** `follow_m` behind the centre of the own armour, on the
//!   candidate that known enemies cannot see (`los_rays` lines).
//! * **Serve** (first). A stranded tank (immobile, or at the rearm trigger
//!   and farther than `tank_crate_far_m` from every stock): it drives
//!   within reach and stays until the tank has its rounds, the truck is out
//!   of shells, or a threat comes (`AiCounters::stranded_served`).
//! * **Drop.** `DropCrates` for low mortars, a low HQ, then squads holding
//!   a point short of ammunition; nothing where a crate already lies.
//! * **Return** to the base once the cargo is below `return_below_pct`.
//!
//! A truck runs from any seen enemy within `flee_m`.
//!
//! **No loops** (S7 tick fix): a move that ends idle short of its goal is a
//! failed move; after `max_fails` the job gives the goal up. A goal given
//! up (also a drop or a service that timed out, the base, a follow spot) is
//! remembered in `AiTruck::gave_up` and not sent again near it until the
//! truck has loaded again; no move is sent twice within `repath_s`.
//! S10b-ai L5 (F8): with `[ai.truck] refusal_memory` 1 each give-up is
//! also an `AiState` refusal (kind `TruckGoal`), so a goal pushed out of
//! the four-goal memory is still not sent again until a truck loads.

use frontline_content::VehicleRole;
use frontline_content::setup::{StockKind, SupplyKind};
use frontline_content::vehicle::VehicleSetup;
use serde::{Deserialize, Serialize};

use super::{AiKind, JobKind, Known, KnownVehicle, MyTank, Rel, sq_m};
use crate::command::CommandKind;
use crate::ecs::{EntityKind, SimId};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{Order, SquadId};
use crate::vehicle::VehicleState;

/// Most truck jobs one AI keeps.
pub const MAX_TRUCKS: usize = 8;
/// A follow spot this close to the last move needs no new order (m).
const SETTLE_M: i32 = 40;
/// Candidates around a follow spot lie this far off (m).
const RING_M: i32 = 25;
/// Candidates around a follow spot.
const RING_N: i32 = 8;
/// Fleeing trucks go this far per order (m).
const FLEE_STEP_M: i32 = 60;
/// A crate lying this close to a client means it needs no other (m).
const CRATE_NEAR_M: i32 = 12;
/// A job serves or drops at most this long (s).
const JOB_MAX_S: u64 = 180;
/// Goals one job remembers giving up on.
pub const GAVE_UP_GOALS: usize = 4;
/// Eye heights of the sight lines (mm): an enemy's and a truck's.
const ENEMY_EYE_MM: i32 = 1500;
const TRUCK_MM: i32 = 2000;

/// A truck job's phase.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum TruckPhase {
    #[default]
    Load = 0,
    Follow = 1,
    Serve = 2,
    Drop = 3,
    Return = 4,
}

/// One own truck's job (module docs).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct AiTruck {
    pub phase: TruckPhase,
    /// The tank it serves.
    pub client: Option<SimId>,
    /// Tick the phase began.
    pub since: u64,
    /// Failed moves in this phase.
    pub fails: u8,
    /// The last move sent, and when.
    pub moved_to: FxVec2,
    pub moved_at: u64,
    /// The client's main-gun rounds at the start of the service.
    pub rounds: u16,
    /// Goals given up on since the last load, latest first (module docs).
    pub gave_up: [Option<FxVec2>; GAVE_UP_GOALS],
}

impl AiTruck {
    fn give_up(&mut self, at: FxVec2) {
        self.gave_up.rotate_right(1);
        self.gave_up[0] = Some(at);
    }

    /// Whether `at` lies near a goal given up on.
    fn gave_up_on(&self, at: FxVec2) -> bool {
        self.gave_up
            .iter()
            .flatten()
            .any(|&p| (p - at).length_sq_raw() <= sq_m(Fx::from_int(SETTLE_M)))
    }
}

/// The client's main-gun rounds.
fn rounds_of(vs: &VehicleState) -> u16 {
    vs.ammo
        .iter()
        .map(|&n| n as u32)
        .sum::<u32>()
        .saturating_add(u32::from(vs.loaded.is_some()))
        .min(u16::MAX as u32) as u16
}

/// A drop the AI wants: where, what, how many.
struct DropWant {
    point: FxVec2,
    kind: SupplyKind,
    count: u8,
}

impl super::Planner<'_> {
    /// Takes the trucks out of `tanks` and runs their jobs (after the
    /// vehicles' upkeep: repair and retreat come first).
    pub(super) fn truck_jobs(
        &mut self,
        tanks: &mut Vec<MyTank>,
        known: &[Known],
        kv: &[KnownVehicle],
        busy: &[SquadId],
    ) {
        let g = self.g;
        let me = self.me;
        self.ai.trucks.retain(|id, _| {
            g.squads.get(id).is_some_and(|s| {
                s.owner == me
                    && s.members.first().is_some_and(|v| {
                        g.vehicles.get(v).is_some_and(|vs| {
                            g.setup.vehicles[vs.type_idx as usize].role == VehicleRole::Truck
                        })
                    })
            })
        });
        let is_truck = |t: &MyTank| {
            g.vehicles
                .get(&t.id)
                .is_some_and(|vs| g.setup.vehicles[vs.type_idx as usize].role == VehicleRole::Truck)
        };
        let (trucks, rest): (Vec<MyTank>, Vec<MyTank>) = tanks.drain(..).partition(is_truck);
        *tanks = rest;
        for t in trucks {
            if busy.contains(&t.squad) || t.immobile {
                continue;
            }
            if self.ai.trucks.len() >= MAX_TRUCKS && !self.ai.trucks.contains_key(&t.squad) {
                continue;
            }
            let tick = self.tick;
            let mut job = self
                .ai
                .trucks
                .get(&t.squad)
                .copied()
                .unwrap_or_else(|| AiTruck {
                    since: tick,
                    moved_at: tick,
                    ..AiTruck::default()
                });
            self.run_truck(&t, &mut job, known, kv);
            self.ai.trucks.insert(t.squad, job);
        }
    }

    /// The fortify-test hook's truck habit (S10b-ai liveness row, L5): the
    /// truck `args.squad` alone runs its job (no enemies known). The plan
    /// its `Drop` ends returns `Failed` with a new truck refusal, else
    /// `Done`.
    pub(super) fn drive_truck(&mut self, args: &super::DriveArgs) -> Option<super::HabitEnd> {
        let g = self.g;
        let sq = args.squad?;
        let q = g
            .squads
            .get(&sq)
            .filter(|q| q.owner == self.me && !q.members.is_empty())?;
        let v = q.members[0];
        let vs = g.vehicles.get(&v)?;
        let t = &self.ents.transform[v.index()];
        let mut tanks = vec![MyTank {
            squad: sq,
            id: v,
            pos: t.pos.xy(),
            yaw: t.yaw,
            immobile: vs.immobile(&g.setup.vehicles[vs.type_idx as usize]),
        }];
        let busy: Vec<SquadId> = self
            .ai
            .pending
            .iter()
            .filter_map(|c| super::squad_of(&c.kind))
            .collect();
        let was = self.ai.trucks.get(&sq).map(|j| j.phase);
        let refused = self.ai.counters.s10b_refused;
        self.truck_jobs(&mut tanks, &[], &[], &busy);
        let now = self.ai.trucks.get(&sq).map(|j| j.phase);
        let end = if self.ai.counters.s10b_refused > refused {
            super::HabitEnd::Failed
        } else {
            super::HabitEnd::Done
        };
        (was == Some(TruckPhase::Drop) && now != Some(TruckPhase::Drop)).then_some(end)
    }

    /// Gives goal `at` up (the job's own memory); with `[ai.truck]
    /// refusal_memory` 1 also an L3 refusal of kind `TruckGoal` (S10b-ai
    /// L5, F8), which outlives the job's four-goal memory.
    fn truck_give_up(&mut self, job: &mut AiTruck, at: FxVec2) {
        job.give_up(at);
        if self.g.setup.rules.ai.truck.refusal_memory != 0 {
            self.ai.refuse(JobKind::TruckGoal, at, self.tick);
            self.ai.counters.s10b_refused += 1;
        }
    }

    /// Whether goal `at` is not to be sent: given up by this job, or (with
    /// `refusal_memory` 1) within `refused_m` of a `TruckGoal` refusal (L4).
    /// A truck refusal does not age out after `refused_s`: it lives until
    /// an own truck loads again (the S8b rule the four-goal memory broke;
    /// a 300 s window let the F8 scene re-order the HQ 326 s after its
    /// refusal). `refused_s` 0 still turns the check off (`loose`).
    fn truck_refused(&self, job: &AiTruck, at: FxVec2) -> bool {
        if job.gave_up_on(at) {
            return true;
        }
        let s = &self.g.setup;
        let o = &s.rules.ai.obstacles;
        if s.rules.ai.truck.refusal_memory == 0 || o.refused_s == 0 {
            return false;
        }
        self.ai
            .refused_near(JobKind::TruckGoal, at, self.tick, u64::MAX, o.refused_m)
    }

    /// Whether the own base is not to be sent (Follow -> Return). With
    /// `refusal_memory` 1 and `refused_s` > 0 a base give-up ages out after
    /// `refused_s` (L4), the job's own memory of it included: the base is
    /// where a truck loads, so an until-a-load refusal there would strand a
    /// lone truck for the match (fix 21). Else `truck_refused`.
    fn base_refused(&self, job: &AiTruck, base: FxVec2) -> bool {
        let s = &self.g.setup;
        let o = &s.rules.ai.obstacles;
        if s.rules.ai.truck.refusal_memory == 0 || o.refused_s == 0 {
            return self.truck_refused(job, base);
        }
        let hz = s.tick_hz.max(1) as u64;
        self.ai.refused_near(
            JobKind::TruckGoal,
            base,
            self.tick,
            o.refused_s as u64 * hz,
            o.refused_m,
        )
    }

    /// A move order for the truck, never the same one within `repath_s`.
    pub(super) fn truck_drive(&mut self, t: &MyTank, job: &mut AiTruck, goal: FxVec2) {
        let a = &self.g.setup.rules.ai.truck;
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
    }

    /// Whether the move sent ended idle short of its goal; counts the
    /// failure. True once `max_fails` is reached (it then stays put).
    fn truck_failed(&mut self, t: &MyTank, job: &mut AiTruck, idle: bool) -> bool {
        let a = &self.g.setup.rules.ai.truck;
        let hz = self.g.setup.tick_hz.max(1) as u64;
        if idle
            && self.tick >= job.moved_at + a.repath_s as u64 * hz
            && (t.pos - job.moved_to).length_sq_raw() > sq_m(Fx::from_int(a.arrive_m as i32))
        {
            job.fails = job.fails.saturating_add(1);
            self.ai.counters.truck_fails += 1;
        }
        job.fails as u32 >= a.max_fails
    }

    fn run_truck(&mut self, t: &MyTank, job: &mut AiTruck, known: &[Known], kv: &[KnownVehicle]) {
        let g = self.g;
        let s = &g.setup;
        let a = &s.rules.ai.truck;
        let hz = s.tick_hz.max(1) as u64;
        let tick = self.tick;
        let vs = &g.vehicles[&t.id];
        let v = &s.vehicles[vs.type_idx as usize];
        let Some(sup) = v.supply.as_ref() else { return };
        let sq = &g.squads[&t.squad];
        let idle = sq.order == Order::Idle;
        let base = g.base_of(self.me);
        let in_base = crate::supply::in_own_base(s, self.me, t.pos);
        let set = |job: &mut AiTruck, phase: TruckPhase| {
            job.phase = phase;
            job.since = tick;
            job.fails = 0;
            // A move sent for the last phase says nothing about this one.
            job.moved_to = t.pos;
        };
        // Run from any seen enemy near (not in the base).
        if !in_base && a.flee_m > 0 {
            let r = sq_m(Fx::from_int(a.flee_m as i32));
            let near = known
                .iter()
                .map(|k| (k.pos, k.id))
                .chain(kv.iter().map(|k| (k.pos, k.id)))
                .filter(|(p, _)| (*p - t.pos).length_sq_raw() <= r)
                .min_by_key(|(p, id)| ((*p - t.pos).length_sq_raw(), *id));
            if let Some((p, _)) = near {
                if job.phase == TruckPhase::Serve {
                    self.end_serve(job, true);
                }
                if matches!(job.phase, TruckPhase::Serve | TruckPhase::Drop) {
                    set(job, TruckPhase::Follow);
                }
                self.ai.counters.truck_flees += 1;
                let away = (t.pos - p).normalize_or_zero() + (base - t.pos).normalize_or_zero();
                let goal = self
                    .terrain
                    .clamp(t.pos + away.normalize_or_zero() * Fx::from_int(FLEE_STEP_M));
                if idle || (goal - job.moved_to).length_sq_raw() > sq_m(Fx::from_int(SETTLE_M)) {
                    self.truck_drive(t, job, goal);
                }
                return;
            }
        }
        // S10a-ai: a material run (`ai::fort`) first.
        if self.material_truck(t, job, in_base, idle) {
            return;
        }
        let want = self.truck_target(sup);
        let load: [u32; 3] = std::array::from_fn(|k| want[k].min(sup.load_g[k]));
        let total_g: u32 = load.iter().sum();
        let held_g: u32 = vs.cargo_g.iter().sum();
        match job.phase {
            TruckPhase::Load => {
                if !in_base {
                    if self.truck_failed(t, job, idle) {
                        self.truck_give_up(job, base);
                        set(job, TruckPhase::Follow);
                    } else if idle {
                        self.truck_drive(t, job, base);
                    }
                    return;
                }
                if !idle {
                    return;
                }
                let done = (0..3).all(|k| vs.cargo_g[k] >= load[k]);
                let enough = held_g as u64 * 100 >= total_g as u64 * a.min_load_pct as u64;
                let waited = tick >= job.since + a.load_wait_s as u64 * hz;
                if done || (enough && (waited || self.supply_budget_spent())) {
                    job.gave_up = [None; GAVE_UP_GOALS];
                    // L5: a load ends the truck refusals (as the job's own).
                    if a.refusal_memory != 0 {
                        self.ai.refused.retain(|r| r.kind != JobKind::TruckGoal);
                    }
                    set(job, TruckPhase::Follow);
                }
            }
            TruckPhase::Follow => {
                if (held_g as u64) * 100 < total_g as u64 * a.return_below_pct as u64
                    && !self.base_refused(job, base)
                {
                    set(job, TruckPhase::Return);
                    self.truck_drive(t, job, base);
                    return;
                }
                if let Some((client, rounds)) = self.stranded_tank(t, vs, job) {
                    job.client = Some(client);
                    job.rounds = rounds;
                    set(job, TruckPhase::Serve);
                    self.ai.counters.truck_serves += 1;
                    return;
                }
                if let Some(d) = self.truck_drop(t, vs, sup, job) {
                    self.ai.emit(
                        self.due,
                        CommandKind::DropCrates {
                            vehicle: t.id,
                            point: d.point,
                            count: d.count,
                            kind: d.kind,
                        },
                    );
                    self.ai.counters.truck_drops_ordered += 1;
                    set(job, TruckPhase::Drop);
                    job.moved_to = d.point;
                    job.moved_at = tick;
                    return;
                }
                if self.truck_failed(t, job, idle) {
                    // It takes where it stands: this spot is not sent again.
                    let spot = job.moved_to;
                    self.truck_give_up(job, spot);
                    job.fails = 0;
                    job.moved_to = t.pos;
                    return;
                }
                if let Some(spot) = self.follow_spot(t, known, kv)
                    && !self.truck_refused(job, spot)
                    && ((spot - job.moved_to).length_sq_raw() > sq_m(Fx::from_int(SETTLE_M))
                        || (idle
                            && (t.pos - spot).length_sq_raw()
                                > sq_m(Fx::from_int(a.arrive_m as i32))))
                    && tick >= job.moved_at + a.repath_s as u64 * hz
                {
                    self.truck_drive(t, job, spot);
                }
            }
            TruckPhase::Serve => {
                let client = job.client.and_then(|c| g.vehicles.get(&c).map(|x| (c, x)));
                let Some((c, cs)) = client.filter(|(c, _)| {
                    self.ents.kind.get(c.index()) == Some(&EntityKind::Vehicle)
                        && self.ents.owner[c.index()] == self.me
                }) else {
                    self.end_serve(job, true);
                    set(job, TruckPhase::Follow);
                    return;
                };
                let cv = &s.vehicles[cs.type_idx as usize];
                let round_g = super::supply::tank_round_g(s, cv);
                let dry = vs.cargo_g[SupplyKind::Shells.index()] < round_g;
                let at = self.ents.transform[c.index()].pos.xy();
                let radius: Fx = sup.vehicle_radius.into();
                let reach = sq_m(radius - Fx::from_int(3));
                let in_reach = (t.pos - at).length_sq_raw() <= reach;
                let gained = rounds_of(cs) > job.rounds;
                // A service still giving rounds runs on past the limit, and
                // a truck in reach has no move to fail.
                let late = tick >= job.since + JOB_MAX_S * hz && !(in_reach && gained);
                let failed = !in_reach && self.truck_failed(t, job, idle);
                if self.rearmed(cs, cv) || dry || late || failed {
                    if (late || failed) && !gained {
                        self.truck_give_up(job, at);
                        if late && !failed {
                            self.ai.counters.truck_fails += 1;
                        }
                    }
                    self.end_serve(job, false);
                    set(job, TruckPhase::Follow);
                    return;
                }
                if !in_reach {
                    let side = (t.pos - at).normalize_or_zero() * (radius - Fx::from_int(5));
                    let goal = self.terrain.clamp(at + side);
                    if idle || (goal - job.moved_to).length_sq_raw() > sq_m(Fx::from_int(SETTLE_M))
                    {
                        self.truck_drive(t, job, goal);
                    }
                } else if !idle {
                    self.ai.emit(self.due, CommandKind::Stop { squad: t.squad });
                }
            }
            TruckPhase::Drop => {
                let late = tick >= job.since + JOB_MAX_S * hz;
                let goal = job.moved_to;
                let reach = s.rules.supply.as_ref().map_or(0, |sp| sp.drop_reach_m) + a.arrive_m;
                if vs.drop.is_none() && tick >= job.since + hz {
                    // Over without getting there (refused, or stopped short):
                    // that point is not ordered again.
                    if (t.pos - goal).length_sq_raw() > sq_m(Fx::from_int(reach as i32)) {
                        self.truck_give_up(job, goal);
                        self.ai.counters.truck_fails += 1;
                    }
                    set(job, TruckPhase::Follow);
                } else if late {
                    self.ai.emit(self.due, CommandKind::Stop { squad: t.squad });
                    self.truck_give_up(job, goal);
                    self.ai.counters.truck_fails += 1;
                    set(job, TruckPhase::Follow);
                }
            }
            TruckPhase::Return => {
                if in_base {
                    if !idle {
                        self.ai.emit(self.due, CommandKind::Stop { squad: t.squad });
                    }
                    set(job, TruckPhase::Load);
                } else if self.truck_failed(t, job, idle) {
                    self.truck_give_up(job, base);
                    set(job, TruckPhase::Follow);
                } else if idle {
                    self.truck_drive(t, job, base);
                }
            }
        }
    }

    /// The grams per kind this AI loads: `load_shells_kg` of shells, the def's
    /// load of the others.
    fn truck_target(&self, sup: &frontline_content::vehicle::TruckSupplySetup) -> [u32; 3] {
        let a = &self.g.setup.rules.ai.truck;
        let mut t = sup.load_g;
        let k = SupplyKind::Shells.index();
        t[k] = t[k].min(a.load_shells_kg.saturating_mul(1000));
        t
    }

    /// Whether crates and truck loads have used `shell_mp_max_pct` of the
    /// income so far (`ai::supply`).
    fn supply_budget_spent(&self) -> bool {
        let g = self.g;
        let s = &g.setup;
        let minutes = self.tick / (60 * s.tick_hz.max(1) as u64);
        let income = s.rules.start_manpower as u64 + s.rules.income_per_min as u64 * minutes;
        let budget =
            income * s.rules.ai.supply.shell_mp_max_pct as u64 / 100 * crate::supply::MICRO;
        let acc = &g.supply6.accounts[self.me.0 as usize];
        acc.crate_micro + acc.truck_load_micro >= budget
    }

    /// The end of a service: counted when the tank gained rounds.
    pub(super) fn end_serve(&mut self, job: &mut AiTruck, gone: bool) {
        if let Some(c) = job.client.take()
            && !gone
            && let Some(cs) = self.g.vehicles.get(&c)
            && rounds_of(cs) > job.rounds
        {
            self.ai.counters.stranded_served += 1;
        }
    }

    /// The nearest own vehicle stranded for want of rounds (module docs),
    /// no other truck on it, no enemy vehicle near it, not given up on.
    fn stranded_tank(
        &self,
        t: &MyTank,
        truck: &VehicleState,
        job: &AiTruck,
    ) -> Option<(SimId, u16)> {
        let g = self.g;
        let s = &g.setup;
        let shells = truck.cargo_g[SupplyKind::Shells.index()];
        let mut best: Option<(u64, SimId, u16)> = None;
        for (id, sq) in &g.squads {
            if sq.owner != self.me
                || self
                    .ai
                    .trucks
                    .iter()
                    .any(|(q, j)| *q != t.squad && j.client == sq.members.first().copied())
            {
                continue;
            }
            let _ = id;
            let Some(&m) = sq.members.first() else {
                continue;
            };
            let Some(cs) = g.vehicles.get(&m) else {
                continue;
            };
            let cv: &VehicleSetup = &s.vehicles[cs.type_idx as usize];
            if cv.main_ammo().is_empty() || cv.role == VehicleRole::Truck {
                continue;
            }
            let round_g = super::supply::tank_round_g(s, cv);
            if round_g == u32::MAX || shells < round_g.saturating_mul(5) {
                continue;
            }
            let at = self.ents.transform[m.index()].pos.xy();
            if crate::supply::in_own_base(s, self.me, at) || self.truck_refused(job, at) {
                continue;
            }
            let (have, load) =
                cv.main_ammo()
                    .iter()
                    .enumerate()
                    .fold((0u32, 0u32), |(h, l), (k, a)| {
                        let n = cs.ammo.get(k).copied().unwrap_or(0) as u32
                            + u32::from(cs.loaded == Some(k as u8));
                        (h + n, l + a.count as u32)
                    });
            let low = have * 100 < load * s.rules.ai.supply.tank_crate_below_pct
                || self.needs_rearm(cs, cv);
            if !low {
                continue;
            }
            let immobile = cs.immobile(cv);
            if !immobile && !self.far_from_sources(at, round_g) {
                continue;
            }
            let d = (at - t.pos).length_sq_raw();
            if best.is_none_or(|(b, bid, _)| (d, m) < (b, bid)) {
                best = Some((d, m, rounds_of(cs)));
            }
        }
        best.map(|(_, c, r)| (c, r))
    }

    /// Where a crate is wanted (module docs). Only what the sim accepts:
    /// the crate caps and the cargo.
    fn truck_drop(
        &self,
        t: &MyTank,
        vs: &VehicleState,
        sup: &frontline_content::vehicle::TruckSupplySetup,
        job: &AiTruck,
    ) -> Option<DropWant> {
        let g = self.g;
        let s = &g.setup;
        let me = self.me;
        let sp = s.rules.supply.as_ref()?;
        let cfg = &s.rules.ai.supply;
        let (live, shell_crates) = g.supply6.crates_of(s, me);
        let in_flight = self
            .ai
            .pending
            .iter()
            .filter(|c| matches!(c.kind, CommandKind::DropCrates { .. }))
            .count() as u32;
        let can = |kind: SupplyKind| -> Option<u32> {
            let def = sup.crates[kind.index()]?;
            let g_each = s.stocks[def as usize].g;
            let room = sp.crate_cap.saturating_sub(live + in_flight);
            let room = if kind == SupplyKind::Shells {
                room.min(sp.shell_crate_cap.saturating_sub(shell_crates + in_flight))
            } else {
                room
            };
            let n = (vs.cargo_g[kind.index()] / g_each.max(1)).min(room);
            (n > 0).then_some(n)
        };
        let lying = |at: FxVec2, kind: SupplyKind| {
            g.supply6.crates.values().any(|c| {
                c.owner == me
                    && s.stocks[c.def as usize].supply == Some(kind)
                    && (c.pos - at).length_sq_raw() <= sq_m(Fx::from_int(CRATE_NEAR_M))
            })
        };
        let far_enough =
            |at: FxVec2| !crate::supply::in_own_base(s, me, at) && !self.truck_refused(job, at);
        // Mortars low on bombs (outside the base and the HQ's radius).
        if let Some(n) = can(SupplyKind::Shells) {
            let hq = g
                .supply6
                .structures
                .values()
                .find(|x| {
                    x.owner == me && x.built && s.stocks[x.def as usize].kind == StockKind::Hq
                })
                .map(|h| (h.pos, s.stocks[h.def as usize].radius));
            let spot = g
                .squads
                .values()
                .filter(|q| {
                    q.owner == me
                        && !q.members.is_empty()
                        && super::ai_kind(s, q.type_idx) == AiKind::Mortar
                })
                .filter_map(|q| {
                    let (_, pct) = crate::supply::mortar_bombs(s, self.ents, q);
                    let pct = pct?;
                    let ty = &s.squad_types[q.type_idx as usize];
                    let gunner = crate::structures::mortar_gunner(ty, self.ents, &q.members)?;
                    let at = self.ents.transform[gunner.index()].pos.xy();
                    let in_hq = hq.is_some_and(|(p, r)| (at - p).length_sq_raw() <= sq_m(r.into()));
                    (pct < cfg.crate_run_below_pct
                        && !in_hq
                        && far_enough(at)
                        && !lying(at, SupplyKind::Shells))
                    .then(|| ((at - t.pos).length_sq_raw(), at))
                })
                .min_by_key(|&(d, at)| (d, at.x.raw(), at.y.raw()));
            if let Some((_, at)) = spot {
                return Some(DropWant {
                    point: self.terrain.clamp(at + FxVec2::from_int(3, 0)),
                    kind: SupplyKind::Shells,
                    count: n.min(2) as u8,
                });
            }
        }
        // A low HQ: the crates stock in.
        for kind in [SupplyKind::Shells, SupplyKind::SmallArms] {
            let Some(n) = can(kind) else { continue };
            let hq = g.supply6.structures.values().find(|x| {
                x.owner == me && x.built && s.stocks[x.def as usize].kind == StockKind::Hq
            });
            if let Some(h) = hq {
                let cap = s.stocks[h.def as usize].cap_g[kind.index()];
                let low = cap != 0
                    && (h.stock_g[kind.index()] as u64) * 100
                        < cap as u64 * cfg.crate_run_below_pct as u64;
                if low && !lying(h.pos, kind) && !self.truck_refused(job, h.pos) {
                    let side = (t.pos - h.pos).normalize_or_zero() * Fx::from_int(6);
                    return Some(DropWant {
                        point: self.terrain.clamp(h.pos + side),
                        kind,
                        count: n.min(3) as u8,
                    });
                }
            }
        }
        // Squads holding a point short of rounds.
        if let Some(n) = can(SupplyKind::SmallArms) {
            let spot = self
                .ai
                .tasks
                .iter()
                .filter(|(_, x)| x.kind == super::TaskKind::Defend)
                .filter_map(|(id, _)| {
                    let q = g.squads.get(id)?;
                    let pct = crate::supply::squad_ammo_pct(s, self.ents, q)?;
                    let at = self.ents.transform[q.lead(self.ents).index()].pos.xy();
                    (pct < cfg.crate_run_below_pct
                        && far_enough(at)
                        && !lying(at, SupplyKind::SmallArms))
                    .then_some((pct, *id, at))
                })
                .min_by_key(|&(p, id, _)| (p, id));
            if let Some((_, _, at)) = spot {
                return Some(DropWant {
                    point: self.terrain.clamp(at + FxVec2::from_int(3, 0)),
                    kind: SupplyKind::SmallArms,
                    count: n.min(1) as u8,
                });
            }
        }
        None
    }

    /// Where the truck waits: `follow_m` behind the centre of the own
    /// armour (else behind the own front point), on the first candidate no
    /// known enemy (soldier or vehicle) sees. `None`: nothing to follow.
    fn follow_spot(&self, t: &MyTank, known: &[Known], kv: &[KnownVehicle]) -> Option<FxVec2> {
        let g = self.g;
        let s = &g.setup;
        let a = &s.rules.ai.truck;
        let base = g.base_of(self.me);
        let mut n = 0i32;
        let mut sum = FxVec2::ZERO;
        for sq in g.squads.values().filter(|q| q.owner == self.me) {
            let Some(vs) = sq.members.first().and_then(|m| g.vehicles.get(m)) else {
                continue;
            };
            if matches!(
                s.vehicles[vs.type_idx as usize].role,
                VehicleRole::Tank | VehicleRole::AssaultGun | VehicleRole::TankDestroyer
            ) {
                sum += self.ents.transform[sq.members[0].index()].pos.xy();
                n += 1;
            }
        }
        let centre = if n > 0 {
            sum.div_int(n)
        } else {
            let rel: Vec<(FxVec2, Rel)> = g
                .points
                .iter()
                .zip(&s.map.points)
                .map(|(c, ps)| {
                    let rel = match c.owner {
                        Some(o) if o == self.me => Rel::Own,
                        None => Rel::Neutral,
                        Some(_) => Rel::Enemy,
                    };
                    (ps.pos.into(), rel)
                })
                .collect();
            rel.iter()
                .filter(|(_, r)| *r == Rel::Own)
                .max_by_key(|(p, _)| ((*p - base).length_sq_raw(), p.x.raw(), p.y.raw()))
                .map(|(p, _)| *p)?
        };
        let back = (base - centre).normalize_or_zero();
        let at = self
            .terrain
            .clamp(centre + back * Fx::from_int(a.follow_m.min(30_000) as i32));
        let r = Fx::from_int(RING_M);
        let turn = Fx::PI.mul_int(2).div_int(RING_N);
        let mut near: Vec<(u64, SimId, FxVec2)> = known
            .iter()
            .map(|k| (k.pos, k.id))
            .chain(kv.iter().map(|k| (k.pos, k.id)))
            .map(|(p, id)| ((p - at).length_sq_raw(), id, p))
            .collect();
        near.sort_unstable_by_key(|(d, id, _)| (*d, *id));
        near.truncate(a.los_rays as usize);
        let seen = |p: FxVec2| {
            let to: FxVec3 = p.extend(self.terrain.height_at(p) + Fx::from_milli(TRUCK_MM));
            near.iter().any(|&(_, _, e)| {
                let from = e.extend(self.terrain.height_at(e) + Fx::from_milli(ENEMY_EYE_MM));
                self.terrain.line_of_sight(from, to)
            })
        };
        if !seen(at) {
            return Some(at);
        }
        for k in 0..RING_N {
            let (sn, cs) = turn.mul_int(k).sin_cos();
            let p = self.terrain.clamp(at + FxVec2::new(cs, sn) * r);
            if !seen(p) {
                return Some(p);
            }
        }
        // Every candidate is seen: back toward the base.
        let _ = t;
        Some(self.terrain.clamp(at + back * Fx::from_int(2 * RING_M)))
    }

    /// Whether a truck is coming to or serving vehicle `id`.
    pub(super) fn truck_coming(&self, id: SimId) -> bool {
        let hz = self.g.setup.tick_hz.max(1) as u64;
        self.ai
            .trucks
            .values()
            .any(|j| j.client == Some(id) && self.tick < j.since + JOB_MAX_S * hz)
    }

    /// Own trucks with shells for ten rounds of `round_g`: squared distance
    /// from `from`, position, reach (m).
    pub(super) fn truck_stock(
        &self,
        from: FxVec2,
        round_g: u32,
    ) -> Option<(u64, FxVec2, frontline_content::Fixed)> {
        let g = self.g;
        g.vehicles
            .iter()
            .filter_map(|(&id, vs)| {
                let d = g.setup.vehicles[vs.type_idx as usize].supply.as_ref()?;
                let i = id.index();
                (self.ents.owner[i] == self.me
                    && self.ents.kind[i] == EntityKind::Vehicle
                    && vs.ammo_fire == 0
                    && vs.cargo_g[SupplyKind::Shells.index()] >= round_g.saturating_mul(10))
                .then(|| {
                    let p = self.ents.transform[i].pos.xy();
                    ((p - from).length_sq_raw(), p, d.vehicle_radius)
                })
            })
            .min_by_key(|&(d, p, _)| (d, p.x.raw(), p.y.raw()))
    }

    /// S8b: recon cars and trucks, each on its own share of the income
    /// (never a tank's buy: `mix_slot`), with manpower left for the mix
    /// (module docs).
    pub(super) fn buy_recon_truck(&mut self) {
        let g = self.g;
        let s = &g.setup;
        let hz = s.tick_hz.max(1) as u64;
        let tick = self.tick;
        let (r, t) = (&s.rules.ai.recon, &s.rules.ai.truck);
        let cars = self.of_role(VehicleRole::Recon);
        let want_cars = if tick >= r.second_after_s as u64 * hz {
            2
        } else if tick >= r.after_s as u64 * hz {
            1
        } else {
            0
        };
        if cars < want_cars {
            self.buy_role(VehicleRole::Recon, r.income_max_pct);
        }
        let trucks = self.of_role(VehicleRole::Truck);
        let fighting = self.of_role(VehicleRole::Tank)
            + self.of_role(VehicleRole::AssaultGun)
            + self.of_role(VehicleRole::TankDestroyer)
            + self.of_role(VehicleRole::Recon);
        let hq = g.supply6.structures.values().any(|x| {
            x.owner == self.me && x.built && s.stocks[x.def as usize].kind == StockKind::Hq
        });
        if trucks == 0
            && fighting >= t.min_vehicles as usize
            && (hq || tick >= t.hq_wait_s as u64 * hz)
        {
            self.buy_role(VehicleRole::Truck, t.income_max_pct);
        }
    }

    /// Own live or pending vehicles of `role`.
    pub(super) fn of_role(&self, role: VehicleRole) -> usize {
        let g = self.g;
        let s = &g.setup;
        let is_role = |t: u16| {
            s.squad_types[t as usize]
                .vehicle
                .is_some_and(|v| s.vehicles[v as usize].role == role)
        };
        g.squads
            .values()
            .filter(|q| q.owner == self.me && is_role(q.type_idx) && !q.members.is_empty())
            .count()
            + self
                .ai
                .pending
                .iter()
                .filter(
                    |c| matches!(c.kind, CommandKind::CallIn { squad_type } if is_role(squad_type)),
                )
                .count()
    }

    fn buy_role(&mut self, role: VehicleRole, income_pct: u32) {
        let reserve = self.g.setup.rules.ai.armour.tank_reserve_mp;
        self.buy_role_within(role, income_pct, reserve);
    }

    /// Buys a card of `role`: its cost plus what the role has spent stays
    /// within `income_pct` of the income so far; at least `reserve`
    /// manpower stays.
    pub(super) fn buy_role_within(&mut self, role: VehicleRole, income_pct: u32, reserve: u32) {
        let g = self.g;
        let s = &g.setup;
        let me = self.me;
        let faction = &s.factions[s.players[me.0 as usize].faction as usize];
        let card = faction.roster.iter().copied().find(|&t| {
            let ty = &s.squad_types[t as usize];
            ty.ai_weight > 0
                && ty
                    .vehicle
                    .is_some_and(|v| s.vehicles[v as usize].role == role)
        });
        let Some(card) = card else { return };
        let ty = &s.squad_types[card as usize];
        let is_role = |t: usize| {
            s.squad_types[t]
                .vehicle
                .is_some_and(|v| s.vehicles[v as usize].role == role)
        };
        let spent: u64 = self
            .ai
            .counters
            .bought
            .iter()
            .enumerate()
            .filter(|(k, _)| is_role(*k))
            .map(|(k, &n)| n as u64 * s.squad_types[k].cost as u64)
            .sum();
        let minutes = self.tick / (60 * s.tick_hz.max(1) as u64);
        let income = s.rules.start_manpower as u64 + s.rules.income_per_min as u64 * minutes;
        let mp = g.players[me.0 as usize].manpower as u64;
        if (spent + ty.cost as u64) * 100 > income * income_pct as u64
            || mp < ty.cost as u64 + reserve as u64
            || g.call_in_check(self.ents, me, card, self.due).is_err()
        {
            return;
        }
        self.ai
            .emit(self.due, CommandKind::CallIn { squad_type: card });
        let c = &mut self.ai.counters;
        if c.bought.len() < s.squad_types.len() {
            c.bought.resize(s.squad_types.len(), 0);
        }
        c.bought[card as usize] += 1;
    }
}
