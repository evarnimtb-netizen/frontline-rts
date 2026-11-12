//! S10b-ai Build D: sweeping before an attack (`docs/slices/S10b-ai.md`
//! R9, R10, L1-L4; values in `[ai.obstacles]`, switch `sweep`).
//!
//! * **Suspects (R9).** Every plan the AI reads `FortState::blasts`: a mine
//!   of another player that went off within `suspect_s` is a suspect (the
//!   blast record keeps no victim; every enemy blast counts, a mine field
//!   is near it either way). `AiState::suspects_seen` keeps how many blast
//!   records the last read saw.
//! * **Want (R10).** A non-own point with assigned attackers wants a sweep
//!   when a suspect or a known enemy mine lies within `sweep_near_m` of the
//!   line nearest attacker -> point, or a known enemy mine within
//!   `sweep_point_m` of the point (`sweep_wanted` per plan and point). The
//!   lane starts [`SHORT_M`] short of the first along the line (never
//!   behind the attacker) and runs `[fortify] lane_max_m` toward the point,
//!   clamped to the map. Suspects and mines inside a lane this point
//!   already has (alive or swept) do not count: one sweep per lane.
//! * **Sweeper.** The point's own `clear_mines` squad at >= 60 % strength,
//!   else the nearest free one within `sweep_reach_m` of the lane start;
//!   none: no job (no buy). At most [`MAX_SWEEPS`] entries; L4 before
//!   every start.
//! * **Phases.** Go (a `Move` to the lane start) -> Sweep (`ClearMines`
//!   sent; the job watches the order; `tries` is 0 until the order stands,
//!   then 1 + the whole metres of its `along`, the L2 progress mark) ->
//!   Done. A swept lane stays as a Done entry (its squad free) while the
//!   point is attacked, up to `suspect_s`, so its suspects want no second
//!   sweep.
//! * **Ends.** The order gone with the lane clear of known enemy mines:
//!   `sweeps_done`. The sweeper gone or under 60 %, the point no longer
//!   attacked: the job ends (no refusal). The order refused or gone with
//!   mines left, a Go or Sweep phase without progress for its stall time
//!   (L2), the deadline `job_max_s` (L1): a failure, L3 (kind Sweep, at the
//!   lane middle).
//! * **Attackers** of the point hold [`HOLD_M`] short of the lane start
//!   while a sweep of the point is alive (`order_infantry`), then go on.
//!
//! Nothing runs with the switch off (no state read, no RNG).

use frontline_content::setup::{Capability, MatchSetup};

use super::{
    AiSweep, DriveArgs, HabitEnd, JobKind, MAX_SWEEPS, Mine, PointEval, Rel, SweepPhase, TaskKind,
    sq_m,
};
use crate::command::CommandKind;
use crate::fort::near_segment;
use crate::fx::{Fx, FxVec2};
use crate::game::{Order, SquadId};

/// The lane starts this far short of the first suspect or mine (m).
const SHORT_M: i32 = 10;
/// Attackers hold this far short of the lane start (m).
pub(super) const HOLD_M: i32 = 10;
/// Go: the sweeper's leader this close to the lane start sends the order.
const ARRIVE_M: i32 = 8;
/// A sweeper below this strength ends its job.
const STRONG_PCT: u32 = 60;
/// L2: a phase's stall time is this many nominal steps ...
const STALL_X: u64 = 3;
/// ... plus this (s).
const STALL_SLACK_S: u64 = 60;

/// Whether the AI sweeps (`[ai.obstacles] sweep` with fortifications on).
pub(super) fn sweep_on(s: &MatchSetup) -> bool {
    s.rules.fortify.is_some() && s.rules.ai.obstacles.sweep != 0
}

/// How a sweep job ended.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum SweepEnd {
    /// The lane is clear (`sweeps_done`).
    Swept,
    /// Ended without a refusal (sweeper gone or weak, point not attacked).
    Quit,
    /// L1, L2 or a refused order: L3.
    Failed,
}

impl super::Planner<'_> {
    /// R9: the suspects (positions of other players' mine blasts younger
    /// than `suspect_s`).
    fn suspects(&mut self) -> Vec<FxVec2> {
        let s = &self.g.setup;
        let hz = s.tick_hz.max(1) as u64;
        let keep = s.rules.ai.obstacles.suspect_s as u64 * hz;
        let b = &self.g.fort.blasts;
        self.ai.suspects_seen = b.len().min(crate::fort::MAX_BLASTS) as u32;
        b.iter()
            .filter(|x| x.owner != self.me && self.tick.saturating_sub(x.tick) < keep)
            .map(|x| x.pos)
            .collect()
    }

    /// Other players' mines this AI knows.
    fn known_enemy_mines(&self) -> Vec<FxVec2> {
        let bit = 1u8 << self.me.0.min(7);
        self.g
            .fort
            .mines
            .values()
            .filter(|m| m.owner != self.me && m.known & bit != 0)
            .map(|m| m.pos)
            .collect()
    }

    /// The lane half width (`[fortify] lane_half_cm`).
    fn lane_half(&self) -> Fx {
        let cm = self
            .g
            .setup
            .rules
            .fortify
            .as_ref()
            .map_or(0, |t| t.lane_half_cm);
        Fx::from_milli(cm.min(i32::MAX as u32 / 10) as i32 * 10)
    }

    /// Whether a known enemy mine lies in the lane `a`-`b`.
    fn lane_has_mine(&self, a: FxVec2, b: FxVec2) -> bool {
        let half = self.lane_half();
        self.known_enemy_mines()
            .iter()
            .any(|&p| near_segment(p, a, b, half))
    }

    /// R10: the lane of point `p` attacked from `from`, if a suspect or a
    /// known enemy mine wants one (those inside a lane the point already
    /// has are left out). `fixed_b`: the hook's caller fixes the lane end.
    fn sweep_lane(
        &self,
        p: usize,
        from: FxVec2,
        to: FxVec2,
        suspects: &[FxVec2],
        mines: &[FxVec2],
        fixed_b: Option<FxVec2>,
    ) -> Option<(FxVec2, FxVec2)> {
        let s = &self.g.setup;
        let o = &s.rules.ai.obstacles;
        let near = Fx::from_int(o.sweep_near_m.min(i32::MAX as u32) as i32);
        let at_point = sq_m(Fx::from_int(o.sweep_point_m.min(i32::MAX as u32) as i32));
        let d = (to - from).normalize_or_zero();
        if d == FxVec2::ZERO {
            return None;
        }
        let covered = |q: FxVec2| {
            self.ai
                .sweeps
                .values()
                .any(|j| j.point as usize == p && near_segment(q, j.a, j.b, near))
        };
        let first = suspects
            .iter()
            .filter(|&&q| near_segment(q, from, to, near))
            .chain(mines.iter().filter(|&&q| {
                near_segment(q, from, to, near) || (q - to).length_sq_raw() <= at_point
            }))
            .filter(|&&q| !covered(q))
            .map(|&q| (q - from).dot(d))
            .min()?;
        let start = (first - Fx::from_int(SHORT_M)).max(Fx::ZERO);
        let a = self.terrain.clamp(from + d * start);
        // 10 cm under `lane_max_m`: the sim refuses a lane over it, and
        // the fixed-point unit vector may be a hair long.
        let lane = s
            .rules
            .fortify
            .as_ref()
            .map_or(0, |t| t.lane_max_m.min(10_000));
        let mut len = Fx::from_milli((lane as i32 * 1000 - 100).max(0));
        // Shortened (not bent) to stay on the map.
        let size = Fx::from_int(s.map.size_m);
        let on_map = |v: FxVec2| [v.x, v.y].iter().all(|c| *c >= Fx::ZERO && *c <= size);
        while len > Fx::ZERO && !on_map(a + d * len) {
            len = (len - Fx::ONE).max(Fx::ZERO);
        }
        let b = fixed_b.unwrap_or(a + d * len);
        Some((a, b))
    }

    /// Whether an own squad's task attacks or takes point `p`.
    fn sweep_attacked(&self, p: u8) -> bool {
        self.ai.tasks.values().any(|t| {
            t.point == p
                && matches!(
                    t.kind,
                    TaskKind::Attack | TaskKind::Capture | TaskKind::Suppress
                )
        })
    }

    /// Where the attackers of point `p` hold while its sweep is alive.
    pub(super) fn sweep_hold(&self, p: usize) -> Option<FxVec2> {
        let j = self
            .ai
            .sweeps
            .values()
            .find(|j| j.point as usize == p && j.phase != SweepPhase::Done)?;
        let d = (j.b - j.a).normalize_or_zero();
        Some(self.terrain.clamp(j.a - d * Fx::from_int(HOLD_M)))
    }

    /// Starts sweep jobs for the points that want one (after the breach
    /// starts; the sweepers are claimed as hunts are).
    pub(super) fn sweep_start(
        &mut self,
        mine: &[Mine],
        assigned: &[(usize, usize)],
        points: &[PointEval],
        claimed: &[SquadId],
        fixed_b: Option<FxVec2>,
    ) -> Vec<SquadId> {
        let g = self.g;
        let s = &g.setup;
        let mut took = Vec::new();
        if !sweep_on(s) {
            return took;
        }
        let suspects = self.suspects();
        let mines = self.known_enemy_mines();
        if suspects.is_empty() && mines.is_empty() {
            return took;
        }
        let o = &s.rules.ai.obstacles;
        let hz = s.tick_hz.max(1) as u64;
        let reach = sq_m(Fx::from_int(o.sweep_reach_m.min(i32::MAX as u32) as i32));
        for (i, pt) in points.iter().enumerate() {
            if pt.rel == Rel::Own
                || self
                    .ai
                    .sweeps
                    .values()
                    .any(|j| j.point as usize == i && j.phase != SweepPhase::Done)
            {
                continue;
            }
            let here: Vec<&Mine> = assigned
                .iter()
                .filter(|a| a.1 == i)
                .map(|a| &mine[a.0])
                .collect();
            let Some(lead) = here
                .iter()
                .min_by_key(|m| ((m.lead - pt.pos).length_sq_raw(), m.id))
                .map(|m| m.lead)
            else {
                continue;
            };
            let Some((a, b)) = self.sweep_lane(i, lead, pt.pos, &suspects, &mines, fixed_b) else {
                continue;
            };
            self.ai.counters.sweep_wanted += 1;
            let mid = (a + b).div_int(2);
            if self.ai.refused_near(
                JobKind::Sweep,
                mid,
                self.tick,
                o.refused_s as u64 * hz,
                o.refused_m,
            ) {
                continue;
            }
            let busy = |id: SquadId| {
                self.ai
                    .sweeps
                    .get(&id)
                    .is_some_and(|j| j.phase != SweepPhase::Done)
            };
            let free = |m: &&Mine| {
                m.can(Capability::ClearMines)
                    && m.strength_pct >= STRONG_PCT
                    && !claimed.contains(&m.id)
                    && !took.contains(&m.id)
                    && !busy(m.id)
            };
            let key = |m: &&Mine| ((m.lead - a).length_sq_raw(), m.id);
            let pick = here
                .iter()
                .copied()
                .filter(free)
                .min_by_key(key)
                .or_else(|| {
                    mine.iter()
                        .filter(free)
                        .filter(|m| (m.lead - a).length_sq_raw() <= reach)
                        .min_by_key(key)
                });
            let Some(m) = pick else { continue };
            if !self.sweep_room(m.id) {
                break;
            }
            // L4 guard (fixed 300 s window; the `loose` control shows it).
            if self.ai.refused_near(
                JobKind::Sweep,
                mid,
                self.tick,
                super::fort::REORDER_WINDOW_S * hz,
                o.refused_m,
            ) {
                self.ai.counters.refused_reorders += 1;
            }
            self.ai.counters.sweeps_started += 1;
            self.ai.sweeps.insert(
                m.id,
                AiSweep {
                    point: i as u8,
                    a,
                    b,
                    start: self.tick,
                    since: self.tick,
                    phase: SweepPhase::Go,
                    tries: 0,
                },
            );
            self.ai.emit(
                self.due,
                CommandKind::Move {
                    squad: m.id,
                    target: a,
                },
            );
            took.push(m.id);
        }
        took
    }

    /// Room for a job of squad `id`: under [`MAX_SWEEPS`], else the oldest
    /// swept (Done) entry makes room; `id`'s own entry is replaced.
    fn sweep_room(&mut self, id: SquadId) -> bool {
        if self.ai.sweeps.remove(&id).is_some() || self.ai.sweeps.len() < MAX_SWEEPS {
            return true;
        }
        let old = self
            .ai
            .sweeps
            .iter()
            .filter(|(_, j)| j.phase == SweepPhase::Done)
            .min_by_key(|(k, j)| (j.since, **k))
            .map(|(k, _)| *k);
        match old {
            Some(k) => {
                self.ai.sweeps.remove(&k);
                true
            }
            None => false,
        }
    }

    /// Runs the sweep jobs (before the assignment): their squads leave
    /// `mine`; ended jobs free theirs; swept entries stay while their point
    /// is attacked, at most `suspect_s`.
    pub(super) fn sweep_continue(&mut self, mine: &mut Vec<Mine>, busy: &[SquadId]) {
        if self.ai.sweeps.is_empty() {
            return;
        }
        let g = self.g;
        let me = self.me;
        let hz = g.setup.tick_hz.max(1) as u64;
        let keep = g.setup.rules.ai.obstacles.suspect_s as u64 * hz;
        let ids: Vec<SquadId> = self.ai.sweeps.keys().copied().collect();
        for id in ids {
            let job = self.ai.sweeps[&id].clone();
            let own = g
                .squads
                .get(&id)
                .is_some_and(|q| q.owner == me && !q.members.is_empty());
            if job.phase == SweepPhase::Done {
                if !own
                    || !sweep_on(&g.setup)
                    || !self.sweep_attacked(job.point)
                    || self.tick >= job.since + keep
                {
                    self.ai.sweeps.remove(&id);
                }
                continue;
            }
            let k = mine.iter().position(|m| m.id == id);
            if own && busy.contains(&id) {
                if let Some(k) = k {
                    mine.remove(k);
                }
                continue;
            }
            let Some(k) = k else {
                // Gone, or taken by a habit before this one (a retreat).
                self.ai.sweeps.remove(&id);
                continue;
            };
            let m = mine[k].clone();
            let mut job = job;
            match self.run_sweep(&m, &mut job) {
                None => {
                    self.ai.sweeps.insert(id, job);
                    mine.remove(k);
                }
                Some(end) => {
                    self.sweep_end(&job, end);
                    if end == SweepEnd::Swept {
                        job.phase = SweepPhase::Done;
                        job.since = self.tick;
                        self.ai.sweeps.insert(id, job);
                    } else {
                        self.ai.sweeps.remove(&id);
                    }
                }
            }
        }
    }

    /// Counts an ended job; a failure writes its refusal (L3) at the lane
    /// middle.
    fn sweep_end(&mut self, job: &AiSweep, end: SweepEnd) {
        let hz = self.g.setup.tick_hz.max(1) as u64;
        if self.tick.saturating_sub(job.start) > super::fort::OVERDUE_S * hz + self.plan_ticks() {
            self.ai.counters.jobs_overdue += 1;
        }
        match end {
            SweepEnd::Swept => self.ai.counters.sweeps_done += 1,
            SweepEnd::Failed => {
                let mid = (job.a + job.b).div_int(2);
                self.ai.refuse(JobKind::Sweep, mid, self.tick);
                self.ai.counters.s10b_refused += 1;
            }
            SweepEnd::Quit => {}
        }
    }

    /// One plan of a sweep job: `Some(end)` when it ended (not yet counted).
    fn run_sweep(&mut self, m: &Mine, job: &mut AiSweep) -> Option<SweepEnd> {
        let g = self.g;
        let s = &g.setup;
        let o = &s.rules.ai.obstacles;
        let hz = s.tick_hz.max(1) as u64;
        let tick = self.tick;
        if tick >= job.start + o.job_max_s as u64 * hz {
            return Some(SweepEnd::Failed);
        }
        if m.strength_pct < STRONG_PCT || !self.sweep_attacked(job.point) {
            return Some(SweepEnd::Quit);
        }
        let id = m.id;
        let pending = self
            .ai
            .pending
            .iter()
            .any(|c| super::squad_of(&c.kind) == Some(id));
        let q = &g.squads[&id];
        match job.phase {
            SweepPhase::Go => {
                if (m.lead - job.a).length_sq_raw() <= sq_m(Fx::from_int(ARRIVE_M)) {
                    self.ai.emit(
                        self.due,
                        CommandKind::ClearMines {
                            squad: id,
                            a: job.a,
                            b: job.b,
                        },
                    );
                    job.phase = SweepPhase::Sweep;
                    job.since = tick;
                    job.tries = 0;
                    return None;
                }
                // L2: the walk from as far as `sweep_reach_m`, three times.
                let ty = &s.squad_types[q.type_idx as usize];
                let speed: Fx = ty.speed.into();
                let speed = speed.floor_int().max(1) as u64;
                let walk = (o.sweep_reach_m as u64 / speed) * STALL_X + STALL_SLACK_S;
                if tick > job.since + walk * hz {
                    return Some(SweepEnd::Failed);
                }
                let going = matches!(q.order, Order::Move { target } if target == job.a);
                if !going && !pending {
                    self.ai.emit(
                        self.due,
                        CommandKind::Move {
                            squad: id,
                            target: job.a,
                        },
                    );
                }
                None
            }
            SweepPhase::Sweep => {
                if pending {
                    return None;
                }
                match q.order {
                    Order::ClearMines { a, b, along, .. } if a == job.a && b == job.b => {
                        let mark = 1 + along.floor_int().clamp(0, 254) as u8;
                        if mark != job.tries {
                            job.tries = mark;
                            job.since = tick;
                            return None;
                        }
                        let lift = s.rules.fortify.as_ref().map_or(0, |t| t.lift_s) as u64;
                        let stall = (lift * STALL_X + STALL_SLACK_S) * hz;
                        (tick > job.since + stall).then_some(SweepEnd::Failed)
                    }
                    // Never seen standing: refused (counted by the sim).
                    _ if job.tries == 0 => Some(SweepEnd::Failed),
                    _ if !self.lane_has_mine(job.a, job.b) => Some(SweepEnd::Swept),
                    _ => Some(SweepEnd::Failed),
                }
            }
            SweepPhase::Done => Some(SweepEnd::Quit),
        }
    }

    /// The fortify-test hook's Sweep habit: `args.squad` sweeps for the
    /// attack of `args.others` on point `args.point` (their tasks set to
    /// Attack each plan; they hold short of the lane while the sweep is
    /// alive). `args.at` fixes the lane end.
    pub(super) fn drive_sweep(&mut self, args: &DriveArgs) -> Option<HabitEnd> {
        let g = self.g;
        let me = self.me;
        let sq = args.squad?;
        let p = args.point;
        let known = self.known_enemies();
        let kv = self.known_vehicles();
        let mut points = self.evaluate_points(&known, &kv);
        let at = points.get(p)?.pos;
        let alive = |ai: &super::AiState| {
            ai.sweeps
                .get(&sq)
                .is_some_and(|j| j.phase != SweepPhase::Done)
        };
        let was = alive(self.ai);
        let (refused, done) = (self.ai.counters.s10b_refused, self.ai.counters.sweeps_done);
        let mine_of = |id: SquadId| {
            let q = g
                .squads
                .get(&id)
                .filter(|q| q.owner == me && !q.members.is_empty())?;
            let ty = &g.setup.squad_types[q.type_idx as usize];
            Some(Mine {
                id,
                role: super::ai_kind(&g.setup, q.type_idx),
                lead: self.ents.transform[q.lead(self.ents).index()].pos.xy(),
                strength_pct: q.members.len() as u32 * 100 / ty.men.max(1) as u32,
                caps: ty.caps,
            })
        };
        for &o in &args.others {
            if mine_of(o).is_some() {
                self.task(o, TaskKind::Attack, p as u8, at);
            }
        }
        let busy: Vec<SquadId> = self
            .ai
            .pending
            .iter()
            .filter_map(|c| super::squad_of(&c.kind))
            .collect();
        let mut mine: Vec<Mine> = std::iter::once(sq)
            .chain(args.others.iter().copied())
            .filter_map(mine_of)
            .collect();
        self.sweep_continue(&mut mine, &busy);
        let ended = was && !alive(self.ai);
        if !was && !alive(self.ai) && !busy.contains(&sq) {
            for (i, pt) in points.iter_mut().enumerate() {
                pt.rel = if i == p { Rel::Enemy } else { Rel::Own };
            }
            let assigned: Vec<(usize, usize)> = mine
                .iter()
                .enumerate()
                .filter(|(_, m)| m.id != sq)
                .map(|(k, _)| (k, p))
                .collect();
            self.sweep_start(&mine, &assigned, &points, &[], args.at);
        }
        if let Some(h) = self.sweep_hold(p) {
            for &o in &args.others {
                let Some(q) = g.squads.get(&o).filter(|q| q.owner == me) else {
                    continue;
                };
                let going = matches!(q.order, Order::Move { target } if target == h);
                let sent = self
                    .ai
                    .pending
                    .iter()
                    .any(|c| super::squad_of(&c.kind) == Some(o));
                if !going && !sent && o != sq {
                    self.ai.emit(
                        self.due,
                        CommandKind::Move {
                            squad: o,
                            target: h,
                        },
                    );
                }
            }
        }
        ended.then(|| {
            if self.ai.counters.sweeps_done > done && self.ai.counters.s10b_refused == refused {
                HabitEnd::Done
            } else {
                HabitEnd::Failed
            }
        })
    }
}
