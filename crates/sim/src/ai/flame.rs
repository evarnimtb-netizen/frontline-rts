//! S14-ai (docs/slices/S14-assault-kit.md 7, A1-A4 and liveness;
//! `[ai.flame]`, every number a key there): flame jobs, refuel, the flame
//! buy and the AI's `ClearTrench`. Plans draw no RNG and read squads, men,
//! points and pieces in ascending order.
//!
//! * **A1 flame job** (`jobs`, needs `[flame] enabled`). Want: a non-own
//!   point (ascending) with no job, fewer than [`MAX_FLAME_JOBS`] jobs in
//!   all, and a target: a known enemy bunker (its roof within `target_m` of
//!   the point and a known enemy within [`BUNKER_MANNED_M`] of its centre;
//!   the nearest to the point, ties by position) else the known enemy in a
//!   dug cell nearest the point within `target_m` (ties by id). A target
//!   within [`FAILED_NEAR_M`] of one that failed less than `recheck_s` ago
//!   is skipped (L3). The job takes the squad attacking the point with the
//!   `flame` capability, a flame man fuelled over `reserve_pct` and half
//!   strength or more, nearest the target (ties by id). Phases (the S10a-ai
//!   breach shape): *Close* (crouched, `Move` to `stage_m` short of
//!   the target, a bunker from its front; a squad already within `stage_m`
//!   of the target goes on at once, and a flame man within `stand_m` burns; the point's base of fire aims at
//!   the target, [`super::Planner::flame_focus`]); *Smoke* (the breach
//!   smoke rule, `ai::breach`, if a man carries smoke, then
//!   `[ai.fort] suppress_lead_s`); *Go* (the squad `Move`s to the stand
//!   spot `stand_m` from the target, the flame man `MoveUnits` there; a
//!   `Move`, as the breach stage's: an `AttackMove` halts at every enemy in
//!   sight and the job would stall under fire);
//!   *Burn* (`SuppressUnits` at the aim: a bunker's slit point, its centre
//!   plus facing times the roof's half depth, from the fort def; else the
//!   trench cell) until his fuel is at or under `reserve_pct` or no known
//!   enemy is within [`TARGET_CLEAR_M`] of the target; *Clear* (a trench
//!   target and `clear` 1: `ClearTrench` at it, until the clear ends).
//! * **Ends** (`AiCounters::flame_job_ends`, by [`super::FLAME_END_NAMES`]):
//!   done (burnt out, cleared, the point taken or no longer attacked),
//!   spent (fuel at the reserve before the burn ended with nothing to
//!   clear), dead (the operator gone and no squad man has his flamethrower
//!   or is picking it up), weak (the squad gone or under half strength),
//!   deadline (`job_max_s` since the start, L1), stall (no progress for
//!   `stall_s`, L2: progress is a phase change, the walker 1 m nearer his
//!   spot, a fuel unit fired, a clear step or phase). Dead, weak, deadline
//!   and stall are failures (L3).
//! * **A2 refuel** (`refuel`): a squad with a flame man at or under
//!   `reserve_pct` and no job takes the S5a refill trip (`ai::ammo`) as a
//!   dry squad, whatever its squad share; the trip ends when that tank is
//!   refilled like any refill (`refill_full_pct`).
//! * **A3 buy** (`buy`): a non-own point attacked (an own Attack or Capture
//!   task) with a flame target and no own `flame` squad attacking it: the
//!   urgent buy takes the faction's flame card (`ai_weight` over 0), while
//!   fewer than `cards_max` flame squads live (pending buys count).
//! * **A4 clear** (`clear`, needs `[clear] enabled`): an own infantry squad
//!   attacking (Attack or Capture task) a point within `clear_near_m` of a
//!   known enemy in a dug cell, on no job, with two frags or more among its
//!   men: `ClearTrench` at the such enemy nearest its leader (ties by id),
//!   once per squad per `recheck_s`. Its squad stays out of the assignment while the clear
//!   runs (a later order would end it).
//!
//! S14-ai keeps its jobs in `AiState::flame` ([`AiFlameState`]; the
//! plan's layout commit had only the counters, see the S14 log).

use std::collections::BTreeMap;

use frontline_content::setup::Capability;
use frontline_content::{ItemKind, MatchSetup, WeaponClass};
use serde::{Deserialize, Serialize};

use super::breach::AiBreach;
use super::{Known, Mine, PointEval, Rel, TaskKind, sq_m};
use crate::command::CommandKind;
use crate::ecs::{EntityKind, SimId};
use crate::fx::{Fx, FxVec2};
use crate::game::{Order, SquadId};
use crate::unit::UnitOrder;

/// Most flame jobs one AI runs at once (spec 7: "four in all").
pub const MAX_FLAME_JOBS: usize = 4;
/// Most failed flame targets one AI remembers (L3; full: the oldest goes).
pub const MAX_FLAME_FAILED: usize = 16;
/// Most squads with a dated AI clear order (A4).
pub const MAX_FLAME_CLEARED: usize = 64;
/// A1 end indices (`AiCounters::flame_job_ends`).
pub const END_DONE: usize = 0;
pub const END_SPENT: usize = 1;
pub const END_DEAD: usize = 2;
pub const END_WEAK: usize = 3;
pub const END_DEADLINE: usize = 4;
pub const END_STALL: usize = 5;
/// Spec 7: the burn ends with no known enemy this close to the target (m).
pub const TARGET_CLEAR_M: i32 = 6;
/// A known enemy this close to a bunker's centre mans it (m).
pub const BUNKER_MANNED_M: i32 = 4;
/// A target this close to a failed one is the same target (m; L3).
pub const FAILED_NEAR_M: i32 = 8;
/// The squad is at its stage spot, the flame man at his stand spot, this
/// close (m).
const STAGE_ARRIVE_M: i32 = 6;
const STAND_ARRIVE_M: i32 = 3;
/// An order already given is not repeated while its spot is this close to
/// the wanted one (m).
const SAME_SPOT_M: i32 = 2;
/// A walk makes progress when the walker comes this much nearer (dm).
const WALK_PROGRESS_DM: u32 = 10;

/// Where a flame job stands (spec 7 A1).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum FlamePhase {
    #[default]
    Close = 0,
    Smoke = 1,
    Go = 2,
    Burn = 3,
    Clear = 4,
}

/// One flame squad's job (keyed by the squad).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AiFlame {
    /// The attacked point.
    pub point: u8,
    /// The target: the enemy's trench cell, or the bunker's centre.
    pub target: FxVec2,
    /// Where the flame man fires (the trench cell, or the slit point).
    pub aim: FxVec2,
    /// Where the squad stages and the flame man stands.
    pub stage: FxVec2,
    pub stand: FxVec2,
    pub bunker: bool,
    /// The operator (a squad man who picks the flamethrower up takes over).
    pub man: SimId,
    pub phase: FlamePhase,
    /// Ticks the job and the phase began.
    pub since: u64,
    pub phase_at: u64,
    /// L2: the phase's best progress measure so far (lower is further)
    /// and the tick it was reached.
    pub best: u32,
    pub best_at: u64,
}

/// The S14-ai state (`AiState::flame`).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct AiFlameState {
    /// A1 jobs, by the flame squad (at most [`MAX_FLAME_JOBS`]).
    pub jobs: BTreeMap<SquadId, AiFlame>,
    /// L3: failed targets and the tick each failed, in failing order (at
    /// most [`MAX_FLAME_FAILED`]).
    pub failed: Vec<(FxVec2, u64)>,
    /// A4: the tick of each squad's last AI `ClearTrench` (at most
    /// [`MAX_FLAME_CLEARED`]; dropped after `recheck_s`).
    pub cleared: BTreeMap<SquadId, u64>,
}

impl AiFlameState {
    /// Load checks (untrusted saves): bounds, ticks not in the future,
    /// points on the map, operators in range.
    pub(super) fn valid(
        &self,
        np: usize,
        tick: u64,
        ents_len: usize,
        on_map: impl Fn(FxVec2) -> bool,
        bad_squad: impl Fn(&SquadId) -> bool,
    ) -> bool {
        self.jobs.len() <= MAX_FLAME_JOBS
            && self.jobs.iter().all(|(id, j)| {
                !bad_squad(id)
                    && (j.point as usize) < np
                    && [j.target, j.aim, j.stage, j.stand].into_iter().all(&on_map)
                    && (j.man.index()) < ents_len
                    && j.since <= j.phase_at
                    && j.phase_at <= tick
                    && j.best_at <= tick
            })
            && self.failed.len() <= MAX_FLAME_FAILED
            && self.failed.windows(2).all(|w| w[0].1 <= w[1].1)
            && self.failed.iter().all(|&(p, t)| on_map(p) && t <= tick)
            && self.cleared.len() <= MAX_FLAME_CLEARED
            && self
                .cleared
                .iter()
                .all(|(id, &t)| !bad_squad(id) && t <= tick)
    }
}

/// A1 on: flame jobs with flame in the rules.
pub(super) fn jobs_on(s: &MatchSetup) -> bool {
    s.rules.flame.enabled != 0 && s.rules.ai.flame.jobs != 0
}

/// Whether a man holds a flame weapon in hand (not on his sidearm).
fn holds_flame(g: &crate::game::MatchState, ents: &crate::ecs::Entities, man: SimId) -> bool {
    let i = man.index();
    ents.kind.get(i) == Some(&EntityKind::Soldier)
        && g.setup.weapons[ents.kit[i].weapon as usize].class == WeaponClass::Flame
}

impl super::Planner<'_> {
    /// Fuel of a man's flamethrower in % of a full tank (his slot's load,
    /// at least the weapon's magazine); `None` when he holds no flame
    /// weapon. A man on his sidearm (dry) has 0.
    pub(super) fn fuel_pct(&self, man: SimId) -> Option<u32> {
        let g = self.g;
        let s = &g.setup;
        if !holds_flame(g, self.ents, man) {
            return None;
        }
        let i = man.index();
        let k = &self.ents.kit[i];
        if k.on_sidearm {
            return Some(0);
        }
        let sq = g.squads.get(&self.ents.squad[i]?)?;
        let ty = &s.squad_types[sq.type_idx as usize];
        let w = &s.weapons[k.weapon as usize];
        let Some(f) = ty.feed_slot(w) else {
            return Some(0);
        };
        let have = crate::supply::man_total(s, ty, k, &self.ents.combat[i], f) as u64;
        let full = (crate::supply::man_load(ty, k, f).max(w.magazine)).max(1) as u64;
        Some((have * 100 / full).min(100) as u32)
    }

    /// The squad's flame man (the lowest id holding a flame weapon) and his
    /// fuel %.
    fn flame_man(&self, id: SquadId) -> Option<(SimId, u32)> {
        let sq = self.g.squads.get(&id)?;
        sq.members
            .iter()
            .filter_map(|&m| self.fuel_pct(m).map(|p| (m, p)))
            .min_by_key(|&(m, _)| m)
    }

    /// A2: the squad's flame man's fuel % (refuel on, flame on).
    pub(super) fn tank_pct(&self, id: SquadId) -> Option<u32> {
        let s = &self.g.setup;
        if s.rules.flame.enabled == 0 || s.rules.ai.flame.refuel == 0 {
            return None;
        }
        self.flame_man(id).map(|(_, p)| p)
    }

    /// A2: the squad's flame man is at or under `reserve_pct`.
    pub(super) fn tank_low(&self, id: SquadId) -> Option<u32> {
        let p = self.tank_pct(id)?;
        (p <= self.g.setup.rules.ai.flame.reserve_pct).then_some(p)
    }

    /// A1 target of point `p`: (target, aim, bunker facing if a bunker).
    fn flame_target(&self, p: usize, known: &[Known]) -> Option<(FxVec2, FxVec2, Option<FxVec2>)> {
        let g = self.g;
        let s = &g.setup;
        let pos: FxVec2 = s.map.points.get(p)?.pos.into();
        let r = sq_m(Fx::from_int(s.rules.ai.flame.target_m as i32));
        let manned = sq_m(Fx::from_int(BUNKER_MANNED_M));
        if let Some(b) = super::fort::fort_of(s, super::fort::WorkKind::Bunker) {
            let me = self.me;
            let roof = g
                .fort
                .pieces
                .values()
                .filter(|q| {
                    q.owner != me
                        && q.fort == b
                        && q.part == 0
                        && (q.pos - pos).length_sq_raw() <= r
                        && known
                            .iter()
                            .any(|k| (k.pos - q.pos).length_sq_raw() <= manned)
                })
                .min_by_key(|q| ((q.pos - pos).length_sq_raw(), q.pos.x.raw(), q.pos.y.raw()));
            if let Some(q) = roof {
                let (sn, cs) = q.yaw.sin_cos();
                let face = FxVec2::new(cs, sn);
                let half: Fx = s.forts[b as usize]
                    .pieces
                    .first()
                    .map_or(Fx::ONE, |d| d.half[0].into());
                let aim = q.pos + face * half;
                return Some((q.pos, self.terrain.clamp(aim), Some(face)));
            }
        }
        known
            .iter()
            .filter(|k| k.in_trench && (k.pos - pos).length_sq_raw() <= r)
            .min_by_key(|k| ((k.pos - pos).length_sq_raw(), k.id))
            .map(|k| (k.pos, k.pos, None))
    }

    /// L3: a target near one that failed less than `recheck_s` ago.
    fn flame_failed_near(&self, at: FxVec2) -> bool {
        let s = &self.g.setup;
        let hz = s.tick_hz.max(1) as u64;
        let wait = s.rules.ai.flame.recheck_s as u64 * hz;
        let r = sq_m(Fx::from_int(FAILED_NEAR_M));
        self.ai
            .flame
            .failed
            .iter()
            .any(|&(p, t)| self.tick.saturating_sub(t) < wait && (p - at).length_sq_raw() <= r)
    }

    /// The base of fire's aim for point `p` while a flame job closes on its
    /// target there (the lowest job squad's), beside `breach_focus`.
    pub(super) fn flame_focus(&self, p: usize) -> Option<FxVec2> {
        self.ai
            .flame
            .jobs
            .values()
            .find(|j| j.point as usize == p && j.phase != FlamePhase::Clear)
            .map(|j| j.target)
    }

    /// Flame squads of point `p` on a job (they count toward its bases).
    pub(super) fn flame_jobs_at(&self, p: usize) -> usize {
        self.ai
            .flame
            .jobs
            .values()
            .filter(|j| j.point as usize == p)
            .count()
    }

    // ------------------------------------------------------------ start --

    /// A1: new flame jobs (after the assignment). Returns the squads taken.
    pub(super) fn flame_start(
        &mut self,
        mine: &[Mine],
        assigned: &[(usize, usize)],
        points: &[PointEval],
        known: &[Known],
        claimed: &[SquadId],
    ) -> Vec<SquadId> {
        let g = self.g;
        let s = &g.setup;
        let mut took = Vec::new();
        if !jobs_on(s) {
            return took;
        }
        let cfg = &s.rules.ai.flame;
        for (i, pt) in points.iter().enumerate() {
            if self.ai.flame.jobs.len() >= MAX_FLAME_JOBS {
                break;
            }
            if pt.rel == Rel::Own || self.flame_jobs_at(i) > 0 {
                continue;
            }
            let Some((target, aim, face)) = self.flame_target(i, known) else {
                continue;
            };
            if self.flame_failed_near(target) {
                continue;
            }
            let pick = assigned
                .iter()
                .filter(|a| a.1 == i)
                .map(|a| &mine[a.0])
                .filter(|m| {
                    m.can(Capability::Flame)
                        && m.strength_pct >= 50
                        && !claimed.contains(&m.id)
                        && !took.contains(&m.id)
                        && self
                            .flame_man(m.id)
                            .is_some_and(|(_, p)| p > cfg.reserve_pct)
                })
                .min_by_key(|m| ((m.lead - target).length_sq_raw(), m.id));
            let Some(m) = pick else { continue };
            let (man, _) = self.flame_man(m.id).expect("picked with one");
            let dir = match face {
                Some(f) => f,
                None => {
                    let d = (m.lead - target).normalize_or_zero();
                    if d == FxVec2::ZERO {
                        FxVec2::new(Fx::ONE, Fx::ZERO)
                    } else {
                        d
                    }
                }
            };
            let from = if face.is_some() { aim } else { target };
            let stage = self
                .terrain
                .clamp(from + dir * Fx::from_int(cfg.stage_m as i32));
            let stand = self
                .terrain
                .clamp(from + dir * Fx::from_int(cfg.stand_m as i32));
            self.ai.counters.flame_jobs += 1;
            // Its task (the end rule reads it; the assignment skips it).
            self.task(m.id, TaskKind::Attack, i as u8, pt.pos);
            let mut job = AiFlame {
                point: i as u8,
                target: self.terrain.clamp(target),
                aim,
                stage,
                stand,
                bunker: face.is_some(),
                man,
                phase: FlamePhase::Close,
                since: self.tick,
                phase_at: self.tick,
                best: u32::MAX,
                best_at: self.tick,
            };
            let units: Vec<SimId> = g.squads[&m.id]
                .members
                .iter()
                .copied()
                .take(crate::unit::MAX_UNITS)
                .collect();
            self.ai.emit(
                self.due,
                CommandKind::SetStance {
                    units,
                    stance: crate::unit::StanceOrder::Crouch,
                },
            );
            let m = m.clone();
            match self.run_flame(&m, &mut job, known, false) {
                None => {
                    self.ai.flame.jobs.insert(m.id, job);
                }
                Some(end) => self.flame_end(&job, end),
            }
            took.push(m.id);
        }
        took
    }

    // ----------------------------------------------------------- phases --

    /// Runs the flame jobs (before the assignment): their squads leave
    /// `mine`; ended jobs free theirs. Also keeps squads on an AI clear
    /// (A4) out of `mine` while the clear runs.
    pub(super) fn flame_continue(
        &mut self,
        mine: &mut Vec<Mine>,
        busy: &[SquadId],
        known: &[Known],
    ) {
        let g = self.g;
        let me = self.me;
        let hz = g.setup.tick_hz.max(1) as u64;
        let wait = g.setup.rules.ai.flame.recheck_s as u64 * hz;
        let tick = self.tick;
        self.ai
            .flame
            .cleared
            .retain(|id, t| g.squads.get(id).is_some_and(|q| q.owner == me) && tick - *t < wait);
        self.ai.flame.failed.retain(|&(_, t)| tick - t < wait);
        let clearing: Vec<SquadId> = self
            .ai
            .flame
            .cleared
            .keys()
            .copied()
            .filter(|id| g.clears.contains_key(id) && !self.ai.flame.jobs.contains_key(id))
            .collect();
        mine.retain(|m| !clearing.contains(&m.id));
        let ids: Vec<SquadId> = self.ai.flame.jobs.keys().copied().collect();
        for id in ids {
            if busy.contains(&id) && g.squads.get(&id).is_some_and(|q| q.owner == me) {
                continue;
            }
            let mut job = self.ai.flame.jobs[&id].clone();
            let Some(k) = mine.iter().position(|m| m.id == id) else {
                self.ai.flame.jobs.remove(&id);
                self.flame_end(&job, END_WEAK);
                continue;
            };
            let m = mine[k].clone();
            if let Some(end) = self.run_flame(&m, &mut job, known, true) {
                self.ai.flame.jobs.remove(&id);
                self.flame_end(&job, end);
                continue;
            }
            self.ai.flame.jobs.insert(id, job);
            mine.remove(k);
        }
    }

    /// Counts an ended job; a failure dates its target (L3).
    fn flame_end(&mut self, job: &AiFlame, end: usize) {
        self.ai.counters.flame_job_ends[end] += 1;
        if matches!(end, END_DEAD | END_WEAK | END_DEADLINE | END_STALL) {
            let f = &mut self.ai.flame.failed;
            if f.len() >= MAX_FLAME_FAILED {
                f.remove(0);
            }
            f.push((job.target, self.tick));
        }
    }

    /// Enters phase `p`.
    fn flame_phase(&self, job: &mut AiFlame, p: FlamePhase) {
        job.phase = p;
        job.phase_at = self.tick;
        job.best = u32::MAX;
        job.best_at = self.tick;
    }

    /// L2: notes the phase's progress measure (lower is further).
    fn flame_progress(&self, job: &mut AiFlame, measure: u32, slack: u32) {
        if job.best == u32::MAX || measure.saturating_add(slack) <= job.best {
            job.best = measure;
            job.best_at = self.tick;
        }
    }

    /// One plan of a flame job: `Some(end)` when it ended. `check` runs the
    /// end rules that read the last plan's tasks.
    fn run_flame(
        &mut self,
        m: &Mine,
        job: &mut AiFlame,
        known: &[Known],
        check: bool,
    ) -> Option<usize> {
        let g = self.g;
        let s = &g.setup;
        let cfg = &s.rules.ai.flame;
        let hz = s.tick_hz.max(1) as u64;
        let tick = self.tick;
        let sq = &g.squads[&m.id];
        // The operator, or the man who has his flamethrower now (one is
        // picking it up: the job waits).
        let mut picking = false;
        if !(sq.members.contains(&job.man) && holds_flame(g, self.ents, job.man)) {
            match self.flame_man(m.id) {
                Some((x, _)) => job.man = x,
                None => {
                    picking = sq
                        .drops
                        .iter()
                        .any(|d| s.weapons[d.weapon as usize].class == WeaponClass::Flame);
                    if !picking {
                        return Some(END_DEAD);
                    }
                }
            }
        }
        if m.strength_pct < 50 {
            return Some(END_WEAK);
        }
        if g.points[job.point as usize].owner == Some(self.me)
            || (check && !self.point_attacked(job.point))
        {
            return Some(END_DONE);
        }
        if tick > job.since + cfg.job_max_s as u64 * hz {
            return Some(END_DEADLINE);
        }
        if tick > job.best_at + cfg.stall_s as u64 * hz {
            return Some(END_STALL);
        }
        if picking {
            return None;
        }
        let fuel = self.fuel_pct(job.man);
        let spent = fuel.is_none_or(|p| p <= cfg.reserve_pct);
        let near = sq_m(Fx::from_int(TARGET_CLEAR_M));
        let enemy_left = known
            .iter()
            .any(|k| (k.pos - job.target).length_sq_raw() <= near);
        let clear_on = cfg.clear != 0 && s.rules.clear.enabled != 0;
        if job.phase != FlamePhase::Burn && job.phase != FlamePhase::Clear && spent {
            return Some(END_SPENT);
        }
        let lead = m.lead;
        match job.phase {
            FlamePhase::Close => {
                // A squad already nearer the target than its stage spot
                // does not walk back to it.
                let d = (lead - job.stage).length();
                let inside = (lead - job.target).length() <= Fx::from_int(cfg.stage_m as i32);
                if d > Fx::from_int(STAGE_ARRIVE_M) && !inside {
                    self.flame_progress(job, dm(d), WALK_PROGRESS_DM);
                    let given = matches!(sq.order, Order::Move { target }
                        if (target - job.stage).length() <= Fx::from_int(SAME_SPOT_M));
                    if !given {
                        self.ai.emit(
                            self.due,
                            CommandKind::Move {
                                squad: m.id,
                                target: job.stage,
                            },
                        );
                    }
                    return None;
                }
                let tmp = AiBreach {
                    point: job.point,
                    crossing: job.stand,
                    section: 0,
                    phase: super::BreachPhase::Smoke,
                    since: tick,
                    tries: 0,
                    suppress_at: None,
                    smoke_at: None,
                    prep_at: None,
                };
                if self.smoke_kit(m.id).is_some() && self.breach_smoke(m.id, &tmp, job.target) {
                    self.flame_phase(job, FlamePhase::Smoke);
                    return None;
                }
                self.flame_phase(job, FlamePhase::Go);
                self.flame_go(m.id, job);
            }
            FlamePhase::Smoke => {
                if tick >= job.phase_at + s.rules.ai.fort.suppress_lead_s as u64 * hz {
                    self.flame_phase(job, FlamePhase::Go);
                    self.flame_go(m.id, job);
                }
            }
            FlamePhase::Go => {
                let at = self.ents.transform[job.man.index()].pos.xy();
                let d = (at - job.stand).length();
                let inside = (at - job.target).length() <= Fx::from_int(cfg.stand_m as i32);
                if d > Fx::from_int(STAND_ARRIVE_M) && !inside {
                    self.flame_progress(job, dm(d), WALK_PROGRESS_DM);
                    let u = &self.ents.unit[job.man.index()];
                    let given = u.detached
                        && matches!(u.order, UnitOrder::Move { target }
                            if (target - job.stand).length() <= Fx::from_int(SAME_SPOT_M));
                    if !given {
                        self.flame_go(m.id, job);
                    }
                    return None;
                }
                self.flame_phase(job, FlamePhase::Burn);
                self.flame_burn(job);
            }
            FlamePhase::Burn => {
                if spent || !enemy_left {
                    if !job.bunker && clear_on {
                        self.flame_phase(job, FlamePhase::Clear);
                        self.ai.emit(
                            self.due,
                            CommandKind::ClearTrench {
                                squad: m.id,
                                at: job.target,
                            },
                        );
                        self.ai.counters.clear_orders += 1;
                        return None;
                    }
                    return Some(if enemy_left { END_SPENT } else { END_DONE });
                }
                let left = self.fuel_units(job.man);
                self.flame_progress(job, left, 1);
                let u = &self.ents.unit[job.man.index()];
                let given = u.detached
                    && matches!(u.order, UnitOrder::Suppress { line }
                        if (line[0] - job.aim).length() <= Fx::from_int(SAME_SPOT_M));
                if !given {
                    self.flame_burn(job);
                }
            }
            FlamePhase::Clear => {
                // The order is applied after the reaction delay: the
                // clear exists from the plan after.
                let Some(c) = g.clears.get(&m.id) else {
                    return (tick > job.phase_at).then_some(END_DONE);
                };
                let measure = 10_000u32.saturating_sub(c.step as u32 * 4 + c.phase as u32);
                self.flame_progress(job, measure, 1);
            }
        }
        None
    }

    /// Fuel units the man still has (his weapon and pouches).
    fn fuel_units(&self, man: SimId) -> u32 {
        let g = self.g;
        let s = &g.setup;
        let i = man.index();
        let Some(sq) = self.ents.squad[i].and_then(|q| g.squads.get(&q)) else {
            return 0;
        };
        let ty = &s.squad_types[sq.type_idx as usize];
        let k = &self.ents.kit[i];
        ty.feed_slot(&s.weapons[k.weapon as usize]).map_or(0, |f| {
            crate::supply::man_total(s, ty, k, &self.ents.combat[i], f)
        })
    }

    /// Go: the squad to the stand spot, the flame man ahead of it.
    fn flame_go(&mut self, id: SquadId, job: &AiFlame) {
        self.ai.emit(
            self.due,
            CommandKind::Move {
                squad: id,
                target: job.stand,
            },
        );
        self.ai.emit(
            self.due,
            CommandKind::MoveUnits {
                units: vec![job.man],
                target: job.stand,
            },
        );
    }

    /// Burn: the flame man fires at the aim.
    fn flame_burn(&mut self, job: &AiFlame) {
        self.ai.emit(
            self.due,
            CommandKind::SuppressUnits {
                units: vec![job.man],
                line: vec![job.aim, job.aim],
            },
        );
    }

    // -------------------------------------------------------------- A4 --

    /// A4: `ClearTrench` for attackers of a point with a known enemy in a
    /// dug cell near it.
    /// Returns the squads ordered.
    pub(super) fn flame_clears(
        &mut self,
        mine: &[Mine],
        assigned: &[(usize, usize)],
        points: &[PointEval],
        known: &[Known],
        claimed: &[SquadId],
    ) -> Vec<SquadId> {
        let g = self.g;
        let s = &g.setup;
        let mut took = Vec::new();
        if s.rules.ai.flame.clear == 0 || s.rules.clear.enabled == 0 {
            return took;
        }
        let r = sq_m(Fx::from_int(s.rules.ai.flame.clear_near_m as i32));
        let frag = ItemKind::Frag.index();
        for &(k, i) in assigned {
            let m = &mine[k];
            if points[i].rel == Rel::Own
                || claimed.contains(&m.id)
                || took.contains(&m.id)
                || g.clears.contains_key(&m.id)
                || self.ai.flame.cleared.contains_key(&m.id)
                || self.ai.flame.cleared.len() >= MAX_FLAME_CLEARED
                || !matches!(
                    self.ai.tasks.get(&m.id).map(|t| t.kind),
                    Some(TaskKind::Attack | TaskKind::Capture)
                )
            {
                continue;
            }
            let frags: u32 = g.squads[&m.id]
                .members
                .iter()
                .map(|x| self.ents.kit[x.index()].items[frag] as u32)
                .sum();
            if frags < 2 {
                continue;
            }
            let pos = points[i].pos;
            let Some(at) = known
                .iter()
                .filter(|e| e.in_trench && (e.pos - pos).length_sq_raw() <= r)
                .min_by_key(|e| ((e.pos - m.lead).length_sq_raw(), e.id))
                .map(|e| e.pos)
            else {
                continue;
            };
            self.ai
                .emit(self.due, CommandKind::ClearTrench { squad: m.id, at });
            self.ai.flame.cleared.insert(m.id, self.tick);
            self.ai.counters.clear_orders += 1;
            took.push(m.id);
        }
        took
    }

    // -------------------------------------------------------------- A3 --

    /// A3: the faction's flame card when a non-own point is attacked with a
    /// flame target and no own flame squad attacks it, while fewer than
    /// `cards_max` flame squads live (`alive` counts pending buys).
    pub(super) fn flame_card(&self, roster: &[u16], alive: &[u32]) -> Option<u16> {
        let g = self.g;
        let s = &g.setup;
        if s.rules.flame.enabled == 0 || s.rules.ai.flame.buy == 0 {
            return None;
        }
        let is_flame = |t: u16| s.squad_types[t as usize].has(Capability::Flame);
        let n: u32 = (0..s.squad_types.len())
            .filter(|&t| is_flame(t as u16))
            .map(|t| alive.get(t).copied().unwrap_or(0))
            .sum();
        if n >= s.rules.ai.flame.cards_max {
            return None;
        }
        let known = self.known_enemies();
        let me = self.me;
        let attacking = |p: usize, flame: bool| {
            self.ai.tasks.iter().any(|(id, t)| {
                t.point as usize == p
                    && matches!(t.kind, TaskKind::Attack | TaskKind::Capture)
                    && (!flame
                        || g.squads.get(id).is_some_and(|q| {
                            s.squad_types[q.type_idx as usize].has(Capability::Flame)
                        }))
            })
        };
        let wanted = (0..s.map.points.len()).any(|p| {
            g.points[p].owner != Some(me)
                && attacking(p, false)
                && !attacking(p, true)
                && self.flame_target(p, &known).is_some()
        });
        if !wanted {
            return None;
        }
        roster
            .iter()
            .copied()
            .find(|&t| s.squad_types[t as usize].ai_weight > 0 && is_flame(t))
    }
}

/// A length in decimetres (progress measure).
fn dm(d: Fx) -> u32 {
    d.mul_int(10).floor_int().max(0) as u32
}
