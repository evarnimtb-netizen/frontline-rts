//! S10b-ai Build D: demolishing enemy obstacles in an own vehicle's way
//! (`docs/slices/S10b-ai.md` R13, R14, L1-L4; values in `[ai.obstacles]`,
//! switch `demolish`).
//!
//! * **Want (R14).** An own AI vehicle with a task (it is in the plan's
//!   free vehicle list: escort, assault, standoff; gun tows belong to the
//!   gun jobs and are not served) whose way is blocked: its served path
//!   ends short of its goal and is not partial (the search ran dry and
//!   ended at the closest reachable point; a reached path ends on the
//!   goal), or its move has ended [`THERE_M`] or more short of the goal;
//!   and a known enemy obstacle (R13) within
//!   `demolish_near_m` of it (`demolish_wanted` per plan and vehicle).
//! * **Pieces.** The known piece nearest the line vehicle -> goal (ahead of
//!   the vehicle), then its neighbours within `demolish_near_m`, nearest
//!   first, alternating sides of the line, up to `demolish_pieces_max`.
//! * **Squad.** An own `demolish` squad, the nearest free one within
//!   `sweep_reach_m` of the vehicle; none: no job (no buy). One job per
//!   vehicle, at most [`MAX_DEMOLITIONS`]; L4 before every start (at the
//!   vehicle, which waits at the crossing).
//! * **Phases.** Go (a `Move` to [`STAND_M`] short of the piece and aside, vehicle
//!   side) -> Charge (`Demolish` sent; the job waits while the order stands
//!   and while its charge burns) -> Repath (the piece gone: `done` + 1 and a
//!   `Move` of the vehicle to its own goal; the job waits for the served
//!   path). The path reaching the goal ends the job `opened`
//!   (`gaps_opened`); else the next standing piece (a gone one is skipped)
//!   is Go; none left: `demolish_given_up` and L3.
//! * **Ends.** The vehicle or the squad gone, the vehicle immobile: the job
//!   ends (no refusal). The `Demolish` refused (no satchel, nothing there)
//!   or the piece standing after its charge, a phase without its progress
//!   mark (walk arrived, charge laid, path served) for its stall time (L2),
//!   the deadline `job_max_s` (L1), all pieces used: a failure, L3 (kind
//!   Demolish, at the vehicle).
//! * The vehicle and the squad leave the plan's lists while the job lives:
//!   the vehicle waits at its spot (one `Move` per piece, no re-path spam).
//!
//! Nothing runs with the switch off (no state read, no RNG).

use frontline_content::setup::{Capability, MatchSetup};

use super::obstacles::MAX_DEMOLISH_PIECES;
use super::{
    AiDemolish, DemolishPhase, DriveArgs, HabitEnd, JobKind, MAX_DEMOLITIONS, Mine, MyTank,
    TaskKind, sq_m,
};
use crate::command::CommandKind;
use crate::ecs::SimId;
use crate::fx::{Fx, FxVec2};
use crate::game::{NavPhase, Order, SquadId};

/// The squad walks to this far short of the piece, on the vehicle's side (m).
const STAND_M: i32 = 4;
/// Go: the squad's leader this close to its stand point sends the order.
const ARRIVE_M: i32 = 8;
/// A vehicle standing this close to its goal has reached it (m).
const THERE_M: i32 = 10;
/// L2: a phase's stall time is this many nominal steps ...
const STALL_X: u64 = 3;
/// ... plus this (s).
const STALL_SLACK_S: u64 = 60;

/// Whether the AI demolishes (`[ai.obstacles] demolish` with
/// fortifications on).
pub(super) fn demolish_on(s: &MatchSetup) -> bool {
    s.rules.fortify.is_some() && s.rules.ai.obstacles.demolish != 0
}

/// How a demolition job ended.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum DemolishEnd {
    /// The vehicle's path reaches its goal (`gaps_opened`).
    Opened,
    /// All pieces used, the path still short (`demolish_given_up`, L3).
    GaveUp,
    /// Deadline, stall or refusal (L3).
    Failed,
    /// Vehicle or squad lost (no refusal).
    Quit,
}

impl super::Planner<'_> {
    /// Own vehicle squad `squad`'s lead position, goal and way there:
    /// `Some(true)` its path reaches the goal (or it stands there),
    /// `Some(false)` its path is exhausted (it ends short and is not
    /// partial) or its move ended short of the goal, `None` not known yet
    /// (path pending or partial).
    fn vehicle_way(&self, squad: SquadId) -> Option<(FxVec2, FxVec2, Option<bool>)> {
        let q = self.g.squads.get(&squad)?;
        if q.owner != self.me || q.members.is_empty() {
            return None;
        }
        let nav = &q.nav;
        let at = self.ents.transform[q.members[0].index()].pos.xy();
        let way = match nav.phase {
            NavPhase::PathPending => None,
            NavPhase::Moving => match nav.path.last() {
                Some(e) if (*e - nav.goal).length_sq_raw() <= sq_m(Fx::from_milli(500)) => {
                    Some(true)
                }
                Some(_) if !nav.partial => Some(false),
                _ => None,
            },
            NavPhase::Holding | NavPhase::Arriving => {
                Some((at - nav.goal).length_sq_raw() <= sq_m(Fx::from_int(THERE_M)))
            }
        };
        Some((at, nav.goal, way))
    }

    /// Own vehicle squad `squad`'s lead position and goal, if its way is
    /// blocked ([`Self::vehicle_way`] `Some(false)`).
    fn vehicle_blocked(&self, squad: SquadId) -> Option<(FxVec2, FxVec2)> {
        match self.vehicle_way(squad)? {
            (at, goal, Some(false)) => Some((at, goal)),
            _ => None,
        }
    }

    /// R14: the pieces to blow for a vehicle at `v` headed to `goal`, among
    /// the known enemy obstacles `known` (ids); empty when none is within
    /// `demolish_near_m` of it.
    fn demolish_pieces(&self, v: FxVec2, goal: FxVec2, known: &[u32]) -> Vec<u32> {
        let g = self.g;
        let o = &g.setup.rules.ai.obstacles;
        let near = sq_m(Fx::from_int(o.demolish_near_m.min(i32::MAX as u32) as i32));
        let max = (o.demolish_pieces_max as usize).min(MAX_DEMOLISH_PIECES);
        let pos = |id: u32| g.fort.pieces.get(&id).map(|p| p.pos);
        let close: Vec<(u32, FxVec2)> = known
            .iter()
            .filter_map(|&id| Some((id, pos(id)?)))
            .filter(|(_, p)| (*p - v).length_sq_raw() <= near)
            .collect();
        let dir = (goal - v).normalize_or_zero();
        // Nearest the line vehicle -> goal (ahead of the vehicle first).
        let Some(&(first, fp)) = close.iter().min_by_key(|(id, p)| {
            let d = *p - v;
            let along = d.dot(dir);
            let off = if along >= Fx::ZERO {
                d.dot(dir.perp()).abs()
            } else {
                d.length()
            };
            (along < Fx::ZERO, off.raw(), *id)
        }) else {
            return Vec::new();
        };
        let mut sides: [Vec<(u64, u32)>; 2] = [Vec::new(), Vec::new()];
        for &(id, p) in &close {
            if id == first || (p - fp).length_sq_raw() > near {
                continue;
            }
            let s = usize::from((p - fp).dot(dir.perp()) < Fx::ZERO);
            sides[s].push(((p - fp).length_sq_raw(), id));
        }
        sides[0].sort_unstable();
        sides[1].sort_unstable();
        // Alternating, the side with the nearer neighbour first.
        let start = match (sides[0].first(), sides[1].first()) {
            (Some(a), Some(b)) => usize::from(b < a),
            (None, Some(_)) => 1,
            _ => 0,
        };
        let mut out = vec![first];
        let (mut i, mut k) = ([0usize; 2], start);
        while out.len() < max && (i[0] < sides[0].len() || i[1] < sides[1].len()) {
            if i[k] < sides[k].len() {
                out.push(sides[k][i[k]].1);
                i[k] += 1;
            }
            k ^= 1;
        }
        out.truncate(max);
        out
    }

    /// Starts demolition jobs for blocked own vehicles in `tanks` (before
    /// the vehicle tasks; their squads are claimed as hunts are, their
    /// vehicles leave `tanks`).
    pub(super) fn demolish_start(
        &mut self,
        mine: &[Mine],
        tanks: &mut Vec<MyTank>,
        claimed: &[SquadId],
    ) -> Vec<SquadId> {
        let g = self.g;
        let s = &g.setup;
        let mut took = Vec::new();
        if !demolish_on(s) || self.ai.demolitions.len() >= MAX_DEMOLITIONS {
            return took;
        }
        let blocked: Vec<(usize, FxVec2, FxVec2)> = tanks
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.immobile && self.ai.tasks.contains_key(&t.squad))
            .filter(|(_, t)| !self.ai.demolitions.values().any(|j| j.vehicle == t.id))
            .filter_map(|(k, t)| self.vehicle_blocked(t.squad).map(|(at, gl)| (k, at, gl)))
            .collect();
        if blocked.is_empty() {
            return took;
        }
        let known = self.known_obstacles();
        if known.is_empty() {
            return took;
        }
        let o = &s.rules.ai.obstacles;
        let hz = s.tick_hz.max(1) as u64;
        let reach = sq_m(Fx::from_int(o.sweep_reach_m.min(i32::MAX as u32) as i32));
        let mut gone = Vec::new();
        for (k, v, goal) in blocked {
            let pieces = self.demolish_pieces(v, goal, &known);
            if pieces.is_empty() {
                continue;
            }
            self.ai.counters.demolish_wanted += 1;
            if self.ai.demolitions.len() >= MAX_DEMOLITIONS
                || self.ai.refused_near(
                    JobKind::Demolish,
                    v,
                    self.tick,
                    o.refused_s as u64 * hz,
                    o.refused_m,
                )
            {
                continue;
            }
            let pick = mine
                .iter()
                .filter(|m| {
                    m.can(Capability::Demolish)
                        && !claimed.contains(&m.id)
                        && !took.contains(&m.id)
                        && !self.ai.demolitions.contains_key(&m.id)
                        && (m.lead - v).length_sq_raw() <= reach
                })
                .min_by_key(|m| ((m.lead - v).length_sq_raw(), m.id));
            let Some(m) = pick else { continue };
            // L4 guard (fixed 300 s window; the `loose` control shows it).
            if self.ai.refused_near(
                JobKind::Demolish,
                v,
                self.tick,
                super::fort::REORDER_WINDOW_S * hz,
                o.refused_m,
            ) {
                self.ai.counters.refused_reorders += 1;
            }
            let mut list = [None; MAX_DEMOLISH_PIECES];
            for (slot, p) in list.iter_mut().zip(&pieces) {
                *slot = Some(*p);
            }
            let job = AiDemolish {
                vehicle: tanks[k].id,
                pieces: list,
                done: 0,
                start: self.tick,
                since: self.tick,
                phase: DemolishPhase::Go,
            };
            self.ai.counters.demolitions_started += 1;
            let to = self.stand_point(&job, v);
            self.ai.demolitions.insert(m.id, job);
            if let Some(to) = to {
                self.ai.emit(
                    self.due,
                    CommandKind::Move {
                        squad: m.id,
                        target: to,
                    },
                );
            }
            took.push(m.id);
            gone.push(k);
        }
        gone.sort_unstable();
        for k in gone.into_iter().rev() {
            tanks.remove(k);
        }
        took
    }

    /// Where the squad stands to blow the job's current piece: [`STAND_M`]
    /// out from it toward the vehicle at `v` and [`STAND_M`] to the side
    /// (the satchel man then walks in beside the waiting vehicle, not
    /// through it).
    fn stand_point(&self, job: &AiDemolish, v: FxVec2) -> Option<FxVec2> {
        let id = job.pieces.get(job.done as usize).copied().flatten()?;
        let p = self.g.fort.pieces.get(&id)?.pos;
        let out = (v - p).normalize_or_zero();
        Some(p + (out + out.perp()) * Fx::from_int(STAND_M))
    }

    /// The own vehicle `v`'s squad and lead position, if it lives, is own
    /// and can drive.
    fn job_vehicle(&self, v: SimId) -> Option<(SquadId, FxVec2)> {
        let g = self.g;
        let sid = self.ents.squad.get(v.index()).copied().flatten()?;
        let q = g.squads.get(&sid)?;
        let vs = g.vehicles.get(&v)?;
        (q.owner == self.me
            && q.members.first() == Some(&v)
            && !vs.immobile(&g.setup.vehicles[vs.type_idx as usize]))
        .then(|| (sid, self.ents.transform[v.index()].pos.xy()))
    }

    /// Runs the demolition jobs (before the assignment): their squads leave
    /// `mine`, their vehicles leave `tanks`; ended jobs free both.
    pub(super) fn demolish_continue(
        &mut self,
        mine: &mut Vec<Mine>,
        tanks: &mut Vec<MyTank>,
        busy: &[SquadId],
    ) {
        if self.ai.demolitions.is_empty() {
            return;
        }
        let g = self.g;
        let me = self.me;
        let ids: Vec<SquadId> = self.ai.demolitions.keys().copied().collect();
        for id in ids {
            let mut job = self.ai.demolitions[&id].clone();
            let own = g
                .squads
                .get(&id)
                .is_some_and(|q| q.owner == me && !q.members.is_empty());
            let k = mine.iter().position(|m| m.id == id);
            if own && busy.contains(&id) {
                if let Some(k) = k {
                    mine.remove(k);
                }
                tanks.retain(|t| t.id != job.vehicle);
                continue;
            }
            let end = match (k, demolish_on(&g.setup)) {
                (Some(_), true) => self.run_demolish(id, &mut job),
                _ => Some(DemolishEnd::Quit),
            };
            match end {
                None => {
                    self.ai.demolitions.insert(id, job.clone());
                    if let Some(k) = k {
                        mine.remove(k);
                    }
                    tanks.retain(|t| t.id != job.vehicle);
                }
                Some(e) => {
                    self.demolish_end(&job, e);
                    self.ai.demolitions.remove(&id);
                }
            }
        }
    }

    /// Counts an ended job; a failure writes its refusal (L3) at the
    /// vehicle (it waits at the crossing).
    fn demolish_end(&mut self, job: &AiDemolish, end: DemolishEnd) {
        let hz = self.g.setup.tick_hz.max(1) as u64;
        if self.tick.saturating_sub(job.start) > super::fort::OVERDUE_S * hz + self.plan_ticks() {
            self.ai.counters.jobs_overdue += 1;
        }
        let at = self.job_vehicle(job.vehicle).map(|(_, p)| p).or_else(|| {
            job.pieces
                .iter()
                .flatten()
                .find_map(|id| self.g.fort.pieces.get(id).map(|p| p.pos))
        });
        match end {
            DemolishEnd::Opened => self.ai.counters.gaps_opened += 1,
            DemolishEnd::GaveUp | DemolishEnd::Failed => {
                if end == DemolishEnd::GaveUp {
                    self.ai.counters.demolish_given_up += 1;
                }
                if let Some(at) = at {
                    self.ai.refuse(JobKind::Demolish, at, self.tick);
                }
                self.ai.counters.s10b_refused += 1;
            }
            DemolishEnd::Quit => {}
        }
    }

    /// The next standing piece from `job.done` on (gone ones are skipped
    /// and count as done); `None`: all used.
    fn next_piece(&self, job: &mut AiDemolish) -> Option<u32> {
        let max =
            (self.g.setup.rules.ai.obstacles.demolish_pieces_max as usize).min(MAX_DEMOLISH_PIECES);
        while (job.done as usize) < max {
            match job.pieces[job.done as usize] {
                Some(id) if self.g.fort.pieces.contains_key(&id) => return Some(id),
                Some(_) => job.done += 1,
                None => return None,
            }
        }
        None
    }

    /// One plan of a demolition job of squad `id`: `Some(end)` when it
    /// ended (not yet counted).
    fn run_demolish(&mut self, id: SquadId, job: &mut AiDemolish) -> Option<DemolishEnd> {
        let g = self.g;
        let s = &g.setup;
        let o = &s.rules.ai.obstacles;
        let hz = s.tick_hz.max(1) as u64;
        let tick = self.tick;
        if tick >= job.start + o.job_max_s as u64 * hz {
            return Some(DemolishEnd::Failed);
        }
        let Some((vsq, v)) = self.job_vehicle(job.vehicle) else {
            return Some(DemolishEnd::Quit);
        };
        let sent = |ai: &super::AiState, sq: SquadId| {
            ai.pending
                .iter()
                .any(|c| super::squad_of(&c.kind) == Some(sq))
        };
        let pending = sent(self.ai, id);
        let q = &g.squads[&id];
        let lead = self.ents.transform[q.lead(self.ents).index()].pos.xy();
        let ty = &s.squad_types[q.type_idx as usize];
        let speed: Fx = ty.speed.into();
        let speed = speed.floor_int().max(1) as u64;
        let t = s.rules.fortify.as_ref();
        let place = t.map_or(0, |t| t.demolish_place_s + t.demolish_fuse_s) as u64;
        match job.phase {
            DemolishPhase::Go => {
                let Some(piece) = self.next_piece(job) else {
                    return Some(DemolishEnd::GaveUp);
                };
                let to = self.stand_point(job, v)?;
                if (lead - to).length_sq_raw() <= sq_m(Fx::from_int(ARRIVE_M)) {
                    let at = g.fort.pieces[&piece].pos;
                    self.ai
                        .emit(self.due, CommandKind::Demolish { squad: id, at });
                    job.phase = DemolishPhase::Charge;
                    job.since = tick;
                    return None;
                }
                // L2: the walk from as far as `sweep_reach_m`, three times.
                let walk = (o.sweep_reach_m as u64 / speed) * STALL_X + STALL_SLACK_S;
                if tick > job.since + walk * hz {
                    return Some(DemolishEnd::Failed);
                }
                let going = matches!(q.order, Order::Move { target } if target == to);
                if !going && !pending {
                    self.ai.emit(
                        self.due,
                        CommandKind::Move {
                            squad: id,
                            target: to,
                        },
                    );
                }
                None
            }
            DemolishPhase::Charge => {
                let piece = job.pieces[job.done as usize]?;
                // L2: a walk of `demolish_near_m`, the placing and the fuse.
                let step = o.demolish_near_m as u64 / speed + place;
                if tick > job.since + (step * STALL_X + STALL_SLACK_S) * hz {
                    return Some(DemolishEnd::Failed);
                }
                let ordered = matches!(q.order, Order::Demolish { piece: p, .. } if p == piece);
                let burning = g.fort.charges.iter().any(|c| c.piece == Some(piece));
                if pending || ordered || burning {
                    return None;
                }
                if g.fort.pieces.contains_key(&piece) {
                    // Refused (no satchel, nothing there) or it stood the
                    // charge.
                    return Some(DemolishEnd::Failed);
                }
                job.done += 1;
                job.phase = DemolishPhase::Repath;
                job.since = tick;
                let goal = g.squads[&vsq].nav.goal;
                self.ai.emit(
                    self.due,
                    CommandKind::Move {
                        squad: vsq,
                        target: goal,
                    },
                );
                None
            }
            DemolishPhase::Repath => {
                let way = if sent(self.ai, vsq) {
                    None
                } else {
                    self.vehicle_way(vsq).and_then(|w| w.2)
                };
                match way {
                    // L2: the path is served within a minute.
                    None => {
                        return (tick > job.since + STALL_SLACK_S * hz)
                            .then_some(DemolishEnd::Failed);
                    }
                    Some(true) => return Some(DemolishEnd::Opened),
                    Some(false) => {}
                }
                if self.next_piece(job).is_none() {
                    return Some(DemolishEnd::GaveUp);
                }
                job.phase = DemolishPhase::Go;
                job.since = tick;
                None
            }
            DemolishPhase::Done => Some(DemolishEnd::Quit),
        }
    }

    /// The fortify-test hook's Demolish habit: `args.squad` demolishes for
    /// the own vehicles in `args.others` (their tasks set to Assault on
    /// point `args.point` each plan; the caller has ordered their moves).
    pub(super) fn drive_demolish(&mut self, args: &DriveArgs) -> Option<HabitEnd> {
        let g = self.g;
        let me = self.me;
        let sq = args.squad?;
        let p = args.point;
        let at: FxVec2 = g.setup.map.points.get(p).map(|x| x.pos.into())?;
        let was = self.ai.demolitions.contains_key(&sq);
        let (opened, refused) = (self.ai.counters.gaps_opened, self.ai.counters.s10b_refused);
        let busy: Vec<SquadId> = self
            .ai
            .pending
            .iter()
            .filter_map(|c| super::squad_of(&c.kind))
            .collect();
        let mut tanks: Vec<MyTank> = Vec::new();
        let mut mine: Vec<Mine> = Vec::new();
        for &o in std::iter::once(&sq).chain(&args.others) {
            let Some(q) = g
                .squads
                .get(&o)
                .filter(|q| q.owner == me && !q.members.is_empty() && !busy.contains(&o))
            else {
                continue;
            };
            let ty = &g.setup.squad_types[q.type_idx as usize];
            let v = q.members[0];
            match g.vehicles.get(&v) {
                Some(vs) => {
                    self.task(o, TaskKind::Assault, p as u8, at);
                    let t = &self.ents.transform[v.index()];
                    tanks.push(MyTank {
                        squad: o,
                        id: v,
                        pos: t.pos.xy(),
                        yaw: t.yaw,
                        immobile: vs.immobile(&g.setup.vehicles[vs.type_idx as usize]),
                    });
                }
                None => mine.push(Mine {
                    id: o,
                    role: super::ai_kind(&g.setup, q.type_idx),
                    lead: self.ents.transform[q.lead(self.ents).index()].pos.xy(),
                    strength_pct: q.members.len() as u32 * 100 / ty.men.max(1) as u32,
                    caps: ty.caps,
                }),
            }
        }
        // As in a match: the jobs go on, then new ones start every plan.
        self.demolish_continue(&mut mine, &mut tanks, &busy);
        self.demolish_start(&mine, &mut tanks, &[]);
        let alive = self.ai.demolitions.contains_key(&sq);
        let ok = self.ai.counters.gaps_opened > opened && self.ai.counters.s10b_refused == refused;
        (was && !alive).then_some(if ok { HabitEnd::Done } else { HabitEnd::Failed })
    }
}
