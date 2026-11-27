//! S5b AI fire missions (docs/slices/S5b-mortars.md "AI mission logic";
//! `[ai.mortar]`, `[ai.ammo] mortar_reserve_pct`, `[fire_mission]`).
//!
//! Per plan, for each mortar squad (ascending id) that is not on a mission:
//!
//! 1. **Targets.** Spotted enemy groups by squad (their centre), in range of
//!    the gunner (`range_min + 20` to `range_max − 10`), never within
//!    `ai.infantry.danger_close_m` of an own man; score `still·10 +
//!    moving·5 + in_trench·20 + noise`, at least 20 (a man still for
//!    [`STILL_TICKS`]; the bombs land 20-30 s after the order, so a moving
//!    group counts half). **Plus** the player's intel cells
//!    where an enemy was seen in a trench within `intel_keep_s` and that no
//!    spotted group stands near (score 30 + 10 per such neighbouring cell,
//!    at most +30, no noise; the cell keeps no head count, so its manned
//!    neighbours stand in for the spec's "10 per man seen there last").
//!    Best first (ties: the lower position).
//! 2. **Observed or direct now.** The sim's own read-only mode check
//!    ([`crate::game::MatchState::mode_for`], at most 4 rays each) on the best
//!    [`MODE_CHECKS`] targets; the first that is not map fire gets a
//!    `FireMission` of `mission_rounds` (never more than it holds) when the
//!    squad holds more than `mortar_reserve_pct` of its bombs or an own
//!    point is under threat.
//! 3. **Spotter.** Else, when the squad is not walking, its leader (a living
//!    man of a `leader` slot who is not the gunner) is not already out and
//!    no spotter came back less than `spot_max_s` ago: up to `spot_samples`
//!    spots on the line from the gunner toward the best target (40, 80, 120
//!    and 150 m, then 30 m to either side of the 80 and 120 m points),
//!    pulled inside the voice link (`observer_link_m − 10`), not past
//!    `danger_close_m` short of the target, within the leader's observe
//!    range of it, not within [`SPOT_CLEAR_M`] of a spotted enemy; the first
//!    with a clear sight line from a crouching eye to the target (+
//!    `point_lift_mm`; one ray each, smoke blocks it) wins:
//!    `MoveUnits { [leader], spot }`, recorded in [`AiState::spotters`].
//!    While he is out the squad holds where it is (no support move); the
//!    next plans fire through him (step 2). He is called back (`Regroup`)
//!    `spot_max_s` after he arrived with no mission (or `3 × spot_max_s`
//!    after he was sent if he never arrives), and `spot_hold_s` after the
//!    last mission with nothing new; a squad order that regroups the squad
//!    (moving, retreating, refilling) ends the record too. The throwers'
//!    regroup-on-walk rule skips a squad with a spotter out.
//! 4. **Map fire** (fallback, no spotter out): the best target, while the
//!    squad holds more than `map_fire_min_pct` of its bombs and the score is
//!    at least `map_fire_min_score` (the sim fires it as map fire); S6b:
//!    only where the sim's map start error at that range is at most
//!    `map_fire_max_miss_m` (else `fm_miss_holds`).
//! 5. **No blind fire:** a target is ordered only where the knowledge
//!    rule's lasting part accepts it ([`crate::game::MatchState::known_by_record`]: a
//!    capture point or an intel cell, which stay known well past the
//!    reaction delay; an enemy seen only this instant may be gone when the
//!    order applies), else passed over (`AiCounters::fm_blind_holds`); a
//!    squad with fewer than [`MIN_BOMBS`] orders nothing. The sim's refusal
//!    count for an AI player stays 0.
//! 6. Otherwise, with no spotter out: the support spot as before
//!    (`support_back_m` behind the most important point).
//!
//! Legacy rules (no `[fire_mission]`) keep the S4/S5a planner unchanged
//! (`mortars_legacy`), so old content plays as before.

use std::collections::BTreeMap;

use frontline_content::setup::{Capability, member_trait};
use serde::{Deserialize, Serialize};

use super::{AiState, Known, Mine, NO_POINT, Planner, PointEval, Rel, TaskKind, sq_m};
use crate::command::CommandKind;
use crate::ecs::SimId;
use crate::fire_mission::{self, intel_cols, intel_flag, mode};
use crate::fx::{Fx, FxVec2};
use crate::game::{NavPhase, Order, Squad, SquadId};

/// Most spotter records one AI keeps (one per mortar squad).
pub const MAX_SPOTTERS: usize = 64;

/// Targets the mode check looks at per plan and squad (each at most
/// `fire_mission::OBSERVE_RAYS` rays).
pub const MODE_CHECKS: usize = 3;
/// Most sight-line rays the mortar planner casts in one plan over all its
/// squads: a mode check counts `fire_mission::OBSERVE_RAYS`, the spot
/// search one per spot. Squads are taken in turn from a start that rotates
/// with the plan count; one that finds the budget spent waits for the next
/// plan (`AiCounters::fm_ray_holds`). The knowledge rule's own rays are not
/// counted: AI targets are seen men or intel cells, which it accepts before
/// casting any. Bounds the plan tick's cost (T14 tick p99).
pub const PLAN_RAYS: usize = 48;
/// A squad holding fewer bombs orders no mission: the man with the last
/// one may fall before the order applies (the sim would refuse it).
pub const MIN_BOMBS: u32 = 2;
/// A spot within this of a spotted enemy is passed over (m).
pub const SPOT_CLEAR_M: i32 = 50;
/// An enemy soldier standing still this long (ticks, 3 s at 30 Hz) is
/// scored as a fixed target (step 1).
pub const STILL_TICKS: u16 = 90;
/// A spotter within this of his spot has arrived (m).
const ARRIVED_M: i32 = 5;
/// The spot offsets (along the line, to its left) tried in order (m): the
/// spec's eight first, then wider to the sides (S5b Part B).
const SPOTS: [(i32, i32); 16] = [
    (40, 0),
    (80, 0),
    (120, 0),
    (150, 0),
    (80, 30),
    (80, -30),
    (120, 30),
    (120, -30),
    (40, 40),
    (40, -40),
    (80, 70),
    (80, -70),
    (120, 70),
    (120, -70),
    (0, 80),
    (0, -80),
];

/// A mortar squad's spotter (its own leader sent forward), by squad.
/// Saved, hashed (with the AI state) and validated on load.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AiSpotter {
    /// The man sent out.
    pub man: SimId,
    /// Where he was sent, and the target he was sent to see.
    pub spot: FxVec2,
    pub target: FxVec2,
    /// Tick he was sent.
    pub since: u64,
    /// Tick he reached the spot.
    pub arrived: Option<u64>,
    /// Tick of the last plan that saw the squad on a mission while he was
    /// out.
    pub last_mission: Option<u64>,
    /// Called back at this tick: the record only blocks a new spotter until
    /// `spot_max_s` later.
    pub back: Option<u64>,
}

impl AiSpotter {
    /// Out (not called back).
    pub fn out(&self) -> bool {
        self.back.is_none()
    }
}

/// S5b AI counters (`AiCounters`), index by `fire_mission::mode`.
pub const FM_MODES: usize = mode::COUNT;

impl AiState {
    /// Load checks of the spotter records.
    pub(crate) fn validate_spotters(&self, next_squad: u32, ents_len: usize, tick: u64) -> bool {
        self.spotters.len() <= MAX_SPOTTERS
            && self.spotters.iter().all(|(id, s)| {
                id.0 < next_squad
                    && (s.man.0 as usize) < ents_len
                    && s.since <= tick
                    && s.arrived.is_none_or(|a| s.since <= a && a <= tick)
                    && s.last_mission.is_none_or(|l| s.since <= l && l <= tick)
                    && s.back.is_none_or(|b| s.since <= b && b <= tick)
            })
    }

    /// Whether squad `id` has a spotter out (the regroup-on-walk rule skips
    /// it).
    pub(crate) fn spotter_out(&self, id: SquadId) -> bool {
        self.spotters.get(&id).is_some_and(AiSpotter::out)
    }
}

/// One candidate target of a mortar squad (S11a1: or an infantry gun).
#[derive(Copy, Clone)]
pub(super) struct Target {
    pub(super) score: i64,
    pub(super) pos: FxVec2,
    /// S11a1 P1: the breach job's preparation target.
    pub(super) prep: bool,
}

impl Planner<'_> {
    /// Step 5 of the plan (module docs): every mortar squad.
    pub(super) fn mortars(&mut self, mine: &[Mine], points: &[PointEval], known: &[Known]) {
        if self.g.setup.rules.fire_mission.is_none() {
            self.mortars_legacy(mine, points, known);
            return;
        }
        let g = self.g;
        let s = &g.setup;
        // Records of squads that are gone.
        self.ai
            .spotters
            .retain(|id, _| g.squads.get(id).is_some_and(|q| q.owner == self.me));
        // S17a A5: own and allied men (mission bursts hurt both).
        let own: Vec<FxVec2> = self
            .ents
            .soldiers()
            .filter(|id| {
                self.g
                    .teams
                    .allied(self.me.0, self.ents.owner[id.index()].0)
            })
            .map(|id| self.ents.transform[id.index()].pos.xy())
            .collect();
        let front = points
            .iter()
            .filter(|p| p.rel != Rel::Own || p.threat > 0)
            .max_by_key(|p| p.priority)
            .or_else(|| points.iter().max_by_key(|p| p.priority))
            .map(|p| p.pos);
        let defend = points.iter().any(|p| p.rel == Rel::Own && p.threat > 0);
        let tubes: Vec<&Mine> = mine
            .iter()
            .filter(|m| {
                s.squad_types[g.squads[&m.id].type_idx as usize].has(Capability::FireMission)
            })
            .collect();
        // The ray budget's turn order rotates with the plan count.
        let start = if tubes.is_empty() {
            0
        } else {
            self.ai.counters.plans as usize % tubes.len()
        };
        for k in 0..tubes.len() {
            let m = tubes[(start + k) % tubes.len()];
            let sq = &g.squads[&m.id];
            let on_mission = matches!(sq.order, Order::FireMission { .. });
            let out = self.spotter_upkeep(m.id, sq, on_mission);
            if on_mission {
                continue;
            }
            let gunner = g
                .mortar_gunner(self.ents, sq)
                .map_or(m.lead, |g| self.ents.transform[g.index()].pos.xy());
            let mut targets = self.mortar_targets(m.id, sq, gunner, known, &own);
            // S11a1 P1: the breach job's focus first (`prep` 1 only).
            if s.rules.ai.arty.prep != 0 {
                let w = &s.weapons[s.squad_types[sq.type_idx as usize].weapon as usize];
                let (min, max) = (
                    Fx::from(w.range_min) + Fx::from_int(20),
                    Fx::from(w.range_max) - Fx::from_int(10),
                );
                // Fix C-4: P3's distance (`[ai.arty]`), so P3 does not
                // lift what P1 just ordered.
                let danger = s.rules.ai.arty.danger_close_m;
                if let Some(t) = self.prep_target(gunner, min, max, danger, known, &own, m.id) {
                    targets.insert(0, t);
                }
            }
            // 2-4 need rays: with the plan's budget spent a squad with
            // targets waits for the next plan (no map fire before the mode
            // check); it still keeps to its support spot.
            let starved = !targets.is_empty() && self.mortar_rays < fire_mission::OBSERVE_RAYS;
            if starved {
                self.ai.counters.fm_ray_holds += 1;
                let walking = matches!(sq.nav.phase, NavPhase::Moving | NavPhase::PathPending);
                if out.is_none()
                    && let Some(front) = front
                    && (walking || !self.in_band(sq, gunner, front))
                {
                    self.support_move(m, sq, front);
                }
                continue;
            }
            let (held, pct) = crate::supply::mortar_bombs(s, self.ents, sq);
            let am = &s.rules.ai.mortar;
            let rounds = match pct {
                Some(_) => (am.mission_rounds as u16).min(held.min(u16::MAX as u32) as u16),
                None => am.mission_rounds as u16,
            };
            let reserve_ok = held >= MIN_BOMBS
                && (defend || pct.is_none_or(|p| p > s.rules.ai.ammo.mortar_reserve_pct));
            let map_ok = held >= MIN_BOMBS && pct.is_none_or(|p| p > am.map_fire_min_pct);
            if !targets.is_empty() && !reserve_ok {
                self.ai.counters.mortar_reserve_holds += 1;
            }
            // 2. Observed or direct now.
            if reserve_ok && rounds > 0 && self.fire_observed(m.id, gunner, &targets, rounds) {
                continue;
            }
            if out.is_some() {
                // 3. A spotter is out: wait for him (no map fire, no move).
                continue;
            }
            // 3. Send the leader out to see one of the best targets.
            if !targets.is_empty()
                && reserve_ok
                && self.send_spotter(m.id, sq, gunner, &targets, known)
            {
                continue;
            }
            // 4. Map fire on the best target, from where the tube is set
            // up: not while it walks to its support spot; S6b: only where
            // the map start error is at most `map_fire_max_miss_m`.
            let walking = matches!(sq.nav.phase, NavPhase::Moving | NavPhase::PathPending);
            if let Some(best) = targets.first().copied()
                && best.score >= am.map_fire_min_score as i64
                && rounds > 0
            {
                if walking {
                    self.ai.counters.fm_walk_holds += 1;
                } else if !map_ok || !reserve_ok {
                    self.ai.counters.fm_map_holds += 1;
                } else if !self.map_fire_hits(gunner, best.pos) {
                    self.ai.counters.fm_miss_holds += 1;
                } else if self.knows(best.pos) {
                    self.fire(m.id, best.pos, rounds, mode::MAP as usize);
                    if best.prep {
                        self.ai.counters.mortar_prep_missions += 1;
                    }
                    continue;
                } else {
                    self.ai.counters.fm_blind_holds += 1;
                }
            }
            // 6. The support spot. A set-up tube stays while the front
            // point lies in its band (mortar sections displace rarely, and
            // moving drops the ranging memory).
            let Some(front) = front else { continue };
            if !walking && self.in_band(sq, gunner, front) {
                self.task(m.id, TaskKind::Support, NO_POINT, m.lead);
                continue;
            }
            self.support_move(m, sq, front);
        }
    }

    /// S6b: whether map fire at `target` starts within
    /// `ai.mortar.map_fire_max_miss_m` (the sim's own map start error at
    /// that range).
    fn map_fire_hits(&self, gunner: FxVec2, target: FxVec2) -> bool {
        let s = &self.g.setup;
        let Some(fm) = s.rules.fire_mission.as_ref() else {
            return true;
        };
        let err = fire_mission::start_error(fm, true, (target - gunner).length());
        err <= Fx::from_int(s.rules.ai.mortar.map_fire_max_miss_m.min(1000) as i32)
    }

    /// Calls squad `id`'s spotter back now.
    fn recall(&mut self, id: SquadId, timeout: bool) {
        let Some(mut rec) = self.ai.spotters.get(&id).cloned() else {
            return;
        };
        self.ai.emit(self.due, CommandKind::Regroup { squad: id });
        rec.back = Some(self.tick);
        if timeout {
            self.ai.counters.fm_spotter_timeouts += 1;
        } else {
            self.ai.counters.fm_spotter_recalls += 1;
        }
        self.ai.spotters.insert(id, rec);
    }

    /// Whether `front` lies between `range_min + danger_close_m + 20` and
    /// `range_max − 200` m of the gunner (S5b: the tube need not move).
    fn in_band(&self, sq: &Squad, gunner: FxVec2, front: FxVec2) -> bool {
        let s = &self.g.setup;
        let w = &s.weapons[s.squad_types[sq.type_idx as usize].weapon as usize];
        let lo =
            Fx::from(w.range_min) + Fx::from_int(s.rules.ai.infantry.danger_close_m as i32 + 20);
        let hi = (Fx::from(w.range_max) - Fx::from_int(200)).max(lo + Fx::from_int(50));
        let d = (front - gunner).length();
        d >= lo && d <= hi
    }

    /// The knowledge rule's lasting part (`MatchState::known_by_record`):
    /// a point the sim will still accept when the order applies a
    /// reaction delay later (an enemy seen only this instant may be gone).
    fn knows(&self, p: FxVec2) -> bool {
        self.g.known_by_record(self.me, p)
    }

    /// Emits the mission and its task; `mode` is what the AI expects.
    fn fire(&mut self, id: SquadId, target: FxVec2, rounds: u16, mode: usize) {
        self.ai.emit(
            self.due,
            CommandKind::FireMission {
                squad: id,
                target,
                rounds,
            },
        );
        self.ai.counters.fm_missions[mode] += 1;
        self.task(id, TaskKind::FireMission, NO_POINT, target);
    }

    /// Step 2: the first of the best targets an observer sees now.
    fn fire_observed(
        &mut self,
        id: SquadId,
        gunner: FxVec2,
        targets: &[Target],
        rounds: u16,
    ) -> bool {
        let spotter = self.ai.spotters.get(&id).filter(|s| s.out()).map(|s| s.man);
        for t in targets.iter().take(MODE_CHECKS) {
            if self.mortar_rays < fire_mission::OBSERVE_RAYS {
                break;
            }
            self.mortar_rays -= fire_mission::OBSERVE_RAYS;
            let (md, obs) = self
                .g
                .mode_for(self.ents, self.terrain, id, gunner, t.pos, self.tick);
            if md == mode::MAP {
                continue;
            }
            if !self.knows(t.pos) {
                self.ai.counters.fm_blind_holds += 1;
                continue;
            }
            self.fire(id, t.pos, rounds, md as usize);
            if t.prep {
                self.ai.counters.mortar_prep_missions += 1;
            }
            if spotter.is_some() && obs == spotter {
                self.ai.counters.fm_spotter_missions += 1;
                if let Some(s) = self.ai.spotters.get_mut(&id) {
                    s.last_mission = Some(self.tick);
                }
            }
            return true;
        }
        false
    }

    /// Keeps squad `id`'s spotter record up to date and calls him back when
    /// his time is up. Returns the man while he is out.
    fn spotter_upkeep(&mut self, id: SquadId, sq: &Squad, on_mission: bool) -> Option<SimId> {
        let am = &self.g.setup.rules.ai.mortar;
        let hz = self.g.setup.tick_hz as u64;
        let tick = self.tick;
        let rec = self.ai.spotters.get(&id)?.clone();
        if let Some(b) = rec.back {
            if tick > b + am.spot_max_s as u64 * hz {
                self.ai.spotters.remove(&id);
            }
            return None;
        }
        let man = rec.man;
        let i = man.index();
        let pending = self.ai.pending.iter().any(
            |c| matches!(&c.kind, CommandKind::MoveUnits { units, .. } if units.contains(&man)),
        );
        let alive = sq.members.contains(&man) && self.ents.combat[i].hp > 0;
        if !alive || (!pending && !self.ents.unit[i].detached) {
            // Dead, or regrouped by a squad order: the record ends.
            self.ai.spotters.remove(&id);
            return None;
        }
        let mut rec = rec;
        let at = self.ents.transform[i].pos.xy();
        if rec.arrived.is_none() && (at - rec.spot).length_sq_raw() <= sq_m(Fx::from_int(ARRIVED_M))
        {
            rec.arrived = Some(tick);
        }
        if on_mission {
            rec.last_mission = Some(tick);
        }
        let max = am.spot_max_s as u64 * hz;
        let recall = match (rec.arrived, rec.last_mission) {
            _ if on_mission || pending => None,
            (_, Some(l)) if tick > l + am.spot_hold_s as u64 * hz => Some(false),
            (_, Some(_)) => None,
            (Some(a), None) if tick > a + max => Some(true),
            (None, None) if tick > rec.since + 3 * max => Some(true),
            _ => None,
        };
        if let Some(timeout) = recall {
            self.ai.spotters.insert(id, rec);
            self.recall(id, timeout);
            return None;
        }
        self.ai.spotters.insert(id, rec);
        Some(man)
    }

    /// Step 1: the squad's targets, best first.
    fn mortar_targets(
        &mut self,
        id: SquadId,
        sq: &Squad,
        gunner: FxVec2,
        known: &[Known],
        own: &[FxVec2],
    ) -> Vec<Target> {
        let s = &self.g.setup;
        let w = &s.weapons[s.squad_types[sq.type_idx as usize].weapon as usize];
        let min = Fx::from(w.range_min) + Fx::from_int(20);
        let max = Fx::from(w.range_max) - Fx::from_int(10);
        let danger_m = s.rules.ai.infantry.danger_close_m;
        self.fire_targets(gunner, min, max, danger_m, true, known, own, id)
    }

    /// Step 1 for any tube at `gunner` (S11a1: generic over the band
    /// `min..=max`, the danger-close distance and the noise, which the
    /// infantry-gun planner leaves out: no RNG there). Best first.
    /// S11a2 S2: with `[ai.arty] ff_safe` 1 the own-men test is
    /// `own_near` (squad `firing` aside) in place of the `danger_m`
    /// distance; a target in the band it refuses counts `ff_holds`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn fire_targets(
        &mut self,
        gunner: FxVec2,
        min: Fx,
        max: Fx,
        danger_m: u32,
        noise: bool,
        known: &[Known],
        own: &[FxVec2],
        firing: SquadId,
    ) -> Vec<Target> {
        let g = self.g;
        let s = &g.setup;
        let danger = sq_m(Fx::from_int(danger_m.min(2000) as i32));
        let safe = s.rules.ai.arty.ff_safe != 0;
        let holds = std::cell::Cell::new(0u32);
        let (ents, me) = (self.ents, self.me);
        let pending = if safe {
            self.ai.pending.clone()
        } else {
            Vec::new()
        };
        let ok = |c: FxVec2| {
            let d = (c - gunner).length();
            if d < min || d > max {
                return false;
            }
            if safe {
                let near = super::arty::own_near(g, ents, me, c, Some(firing), &pending);
                holds.set(holds.get() + u32::from(near));
                !near
            } else {
                !own.iter().any(|o| (*o - c).length_sq_raw() <= danger)
            }
        };
        // Enemy groups by squad: (sum x, sum y, men, men in trenches, men
        // standing still).
        let mut groups: BTreeMap<u32, (i64, i64, i64, i64, i64)> = BTreeMap::new();
        for k in known {
            let e = groups.entry(k.squad).or_default();
            e.0 += k.pos.x.raw() as i64;
            e.1 += k.pos.y.raw() as i64;
            e.2 += 1;
            e.3 += k.in_trench as i64;
            e.4 += k.still as i64;
        }
        let mut out: Vec<Target> = Vec::new();
        let mut centres: Vec<FxVec2> = Vec::new();
        for &(sx, sy, n, trench, still) in groups.values() {
            let c = FxVec2::new(Fx::from_raw((sx / n) as i32), Fx::from_raw((sy / n) as i32));
            centres.push(c);
            if !ok(c) {
                continue;
            }
            // A moving man counts half: the bombs land 20-30 s after the
            // order (laying plus flight), so a group on the move is
            // elsewhere by then.
            let noise = if noise { self.noise() } else { 0 };
            let score = still * 10 + (n - still) * 5 + trench * 20 + noise;
            if score >= 20 {
                out.push(Target {
                    score,
                    pos: c,
                    prep: false,
                });
            }
        }
        // Intel cells: an enemy seen in a trench there lately.
        let fm = s.rules.fire_mission.as_ref().expect("S5b rules");
        let hz = s.tick_hz as u64;
        let keep = fm.intel_keep_s as u64 * hz;
        let intel = &g.players[self.me.0 as usize].intel;
        let cols = intel_cols(s.map.size_m, fm.intel_cell_m) as i64;
        let cell = Fx::from_int(fm.intel_cell_m as i32);
        let near = sq_m(Fx::from_int(fm.intel_snap_m as i32));
        let fresh = |c: &crate::fire_mission::IntelCell| {
            c.flags & intel_flag::TRENCH != 0 && self.tick.saturating_sub(c.seen) <= keep
        };
        for (&k, c) in intel {
            if !fresh(c) {
                continue;
            }
            let (x, y) = (k as i64 % cols, k as i64 / cols);
            let pos = FxVec2::new(
                cell.mul_int(x as i32) + cell.div_int(2),
                cell.mul_int(y as i32) + cell.div_int(2),
            );
            if !ok(pos) || centres.iter().any(|c| (*c - pos).length_sq_raw() <= near) {
                continue;
            }
            let mut manned = 0i64;
            for (dx, dy) in [
                (-1, -1),
                (0, -1),
                (1, -1),
                (-1, 0),
                (1, 0),
                (-1, 1),
                (0, 1),
                (1, 1),
            ] {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= cols || ny >= cols {
                    continue;
                }
                if intel.get(&((ny * cols + nx) as u32)).is_some_and(fresh) {
                    manned += 1;
                }
            }
            out.push(Target {
                score: 30 + 10 * manned.min(3),
                pos,
                prep: false,
            });
        }
        self.ai.counters.ff_holds += holds.get();
        out.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then(a.pos.x.raw().cmp(&b.pos.x.raw()))
                .then(a.pos.y.raw().cmp(&b.pos.y.raw()))
        });
        out
    }

    /// Step 3: sends the squad's leader to the first spot from which he
    /// would see one of the best [`MODE_CHECKS`] targets (in order; at most
    /// `spot_samples` rays in all).
    fn send_spotter(
        &mut self,
        id: SquadId,
        sq: &Squad,
        gunner: FxVec2,
        targets: &[Target],
        known: &[Known],
    ) -> bool {
        let g = self.g;
        let s = &g.setup;
        if self.ai.spotters.contains_key(&id)
            || self.ai.spotters.len() >= MAX_SPOTTERS
            || matches!(sq.nav.phase, NavPhase::Moving | NavPhase::PathPending)
        {
            return false;
        }
        let ty = &s.squad_types[sq.type_idx as usize];
        let gun = g.mortar_gunner(self.ents, sq);
        let Some(leader) = sq.members.iter().copied().find(|&mm| {
            let i = mm.index();
            Some(mm) != gun
                && self.ents.combat[i].hp > 0
                && !self.ents.unit[i].detached
                && ty
                    .members
                    .get(self.ents.kit[i].slot as usize)
                    .is_some_and(|sl| sl.traits & member_trait::LEADER != 0)
        }) else {
            return false;
        };
        let mut rays = (s.rules.ai.mortar.spot_samples as usize).min(self.mortar_rays);
        let before = rays;
        let found = targets.iter().take(MODE_CHECKS).find_map(|t| {
            self.spot_for(sq, gunner, t.pos, known, &mut rays)
                .map(|p| (p, t.pos))
        });
        self.mortar_rays -= before - rays;
        let Some((spot, target)) = found else {
            return false;
        };
        self.ai.emit(
            self.due,
            CommandKind::MoveUnits {
                units: vec![leader],
                target: spot,
            },
        );
        self.ai.spotters.insert(
            id,
            AiSpotter {
                man: leader,
                spot,
                target,
                since: self.tick,
                arrived: None,
                last_mission: None,
                back: None,
            },
        );
        self.ai.counters.fm_spotters += 1;
        true
    }

    /// The first spot (module docs, step 3) with a sight line to `target`;
    /// each ray cast counts down `rays`.
    fn spot_for(
        &self,
        sq: &Squad,
        gunner: FxVec2,
        target: FxVec2,
        known: &[Known],
        rays: &mut usize,
    ) -> Option<FxVec2> {
        let s = &self.g.setup;
        let fm = s.rules.fire_mission.as_ref()?;
        let body = &s.rules.combat.body;
        let to_t = target - gunner;
        let d = to_t.length();
        let dir = to_t.normalize_or_zero();
        if dir == FxVec2::ZERO {
            return None;
        }
        let side = FxVec2::new(-dir.y, dir.x);
        let link = Fx::from_int(fm.observer_link_m as i32 - 10);
        // S11a2 S2: with `ff_safe` 1 the spotter stands outside the own-men
        // filter (`[ai.arty] danger_close_m + 20`, the `+ 20` of `in_band`),
        // so he is no reason to lift.
        let stand_off = if s.rules.ai.arty.ff_safe != 0 {
            s.rules.ai.arty.danger_close_m as i32 + 20
        } else {
            s.rules.ai.infantry.danger_close_m as i32
        };
        let short = d - Fx::from_int(stand_off);
        let observe = sq_m(Fx::from_int(fm.observe_leader_m as i32 - 10));
        let clear = sq_m(Fx::from_int(SPOT_CLEAR_M));
        let to = target.extend(
            crate::fire_mission::ground_point(self.terrain, target)
                + Fx::from_milli(fm.point_lift_mm as i32),
        );
        // A standing eye: the spotter keeps the automatic stance (up when
        // not under fire).
        let eye_up = Fx::from_milli(body.stand_mm as i32 - body.eye_drop_mm as i32);
        let key = SimId(sq.members[0].0);
        for &(a, l) in &SPOTS {
            if *rays == 0 {
                return None;
            }
            let a = Fx::from_int(a);
            if a >= short {
                continue;
            }
            let off = (dir * a + side * Fx::from_int(l)).clamp_length(link);
            let p = self.terrain.clamp(gunner + off);
            if (p - target).length_sq_raw() > observe
                || known.iter().any(|k| (k.pos - p).length_sq_raw() <= clear)
            {
                continue;
            }
            let eye = p.extend(self.terrain.height_at(p) + eye_up);
            *rays -= 1;
            if self.terrain.line_of_sight(eye, to)
                && !self.g.smoke_blocks(eye, to, self.tick, (key, key))
            {
                return Some(p);
            }
        }
        None
    }

    /// Step 6: the support spot behind the front point.
    fn support_move(&mut self, m: &Mine, sq: &Squad, front: FxVec2) {
        let g = self.g;
        let back = (g.base_of(self.me) - front).normalize_or_zero();
        let spot = self
            .terrain
            .clamp(front + back * Fx::from_int(g.setup.rules.ai.infantry.support_back_m as i32));
        let near = |a: FxVec2, r: i32| (a - spot).length_sq_raw() <= sq_m(Fx::from_int(r));
        let on_track = match &sq.order {
            Order::Idle => near(m.lead, 40),
            Order::Move { target } => near(*target, 40),
            _ => false,
        };
        if !on_track {
            self.ai.emit(
                self.due,
                CommandKind::Move {
                    squad: m.id,
                    target: spot,
                },
            );
        }
        self.task(m.id, TaskKind::Support, NO_POINT, spot);
    }

    /// The S4/S5a mortar planner, for rules without `[fire_mission]`.
    fn mortars_legacy(&mut self, mine: &[Mine], points: &[PointEval], known: &[Known]) {
        let g = self.g;
        let s = &g.setup;
        // Enemy groups by squad: (squad, sum x, sum y, men, men in trenches).
        let mut groups: BTreeMap<u32, (i64, i64, i64, i64)> = BTreeMap::new();
        for k in known {
            let e = groups.entry(k.squad).or_default();
            e.0 += k.pos.x.raw() as i64;
            e.1 += k.pos.y.raw() as i64;
            e.2 += 1;
            e.3 += k.in_trench as i64;
        }
        // S17a A5: own and allied men (mission bursts hurt both).
        let own: Vec<FxVec2> = self
            .ents
            .soldiers()
            .filter(|id| {
                self.g
                    .teams
                    .allied(self.me.0, self.ents.owner[id.index()].0)
            })
            .map(|id| self.ents.transform[id.index()].pos.xy())
            .collect();
        let danger = sq_m(Fx::from_int(
            self.g.setup.rules.ai.infantry.danger_close_m as i32,
        ));
        let front = points
            .iter()
            .filter(|p| p.rel != Rel::Own || p.threat > 0)
            .max_by_key(|p| p.priority)
            .or_else(|| points.iter().max_by_key(|p| p.priority))
            .map(|p| p.pos);
        // S4: squads that can barrage (a member carries a mortar).
        for m in mine.iter().filter(|m| {
            s.squad_types[g.squads[&m.id].type_idx as usize].has(Capability::FireMission)
        }) {
            let sq = &g.squads[&m.id];
            if matches!(sq.order, Order::FireMission { .. }) {
                continue;
            }
            let w = &s.weapons[s.squad_types[sq.type_idx as usize].weapon as usize];
            let min = Fx::from(w.range_min) + Fx::from_int(20);
            let max = Fx::from(w.range_max) - Fx::from_int(10);
            let mut best: Option<(i64, FxVec2)> = None;
            for &(sx, sy, n, trench) in groups.values() {
                let c = FxVec2::new(Fx::from_raw((sx / n) as i32), Fx::from_raw((sy / n) as i32));
                let d = (c - m.lead).length();
                if d < min || d > max || own.iter().any(|o| (*o - c).length_sq_raw() <= danger) {
                    continue;
                }
                let score = n * 10 + trench * 20 + self.noise();
                if score >= 20 && best.is_none_or(|(b, _)| score > b) {
                    best = Some((score, c));
                }
            }
            // S5a mortar reserve: barrage only above the reserve unless an
            // own point is under threat, never more rounds than it holds.
            let (held, pct) = crate::supply::mortar_bombs(s, self.ents, sq);
            let defend = points.iter().any(|p| p.rel == Rel::Own && p.threat > 0);
            if best.is_some()
                && (held == 0
                    || (pct.is_some_and(|p| p <= s.rules.ai.ammo.mortar_reserve_pct) && !defend))
            {
                self.ai.counters.mortar_reserve_holds += 1;
                best = None;
            }
            if let Some((_, target)) = best {
                let rounds = match pct {
                    Some(_) => s
                        .rules
                        .ai
                        .barrage_rounds
                        .min(held.min(u16::MAX as u32) as u16),
                    None => s.rules.ai.barrage_rounds,
                };
                self.ai.emit(
                    self.due,
                    CommandKind::FireMission {
                        squad: m.id,
                        target,
                        rounds,
                    },
                );
                self.task(m.id, TaskKind::FireMission, NO_POINT, target);
                continue;
            }
            let Some(front) = front else { continue };
            self.support_move(m, sq, front);
        }
    }
}

#[cfg(test)]
impl crate::game::MatchState {
    /// Test hook (S11a2 S2): the spotter spot of player `me`'s mortar
    /// squad `id` at `gunner` for `target` (no known enemies, 16 rays).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn test_spot_for(
        &self,
        ai: &mut AiState,
        me: crate::ecs::PlayerId,
        ents: &crate::ecs::Entities,
        terrain: &crate::terrain::Heightfield,
        tick: u64,
        id: SquadId,
        gunner: FxVec2,
        target: FxVec2,
    ) -> Option<FxVec2> {
        let p = Planner {
            g: self,
            ai,
            me,
            ents,
            terrain,
            due: tick + 1,
            tick,
            quality: 100,
            rng: crate::rng::RngStream::new(
                1,
                tick,
                crate::ecs::SimId(0),
                crate::rng::Purpose::AI_PLAN,
            ),
            mortar_rays: 0,
            at_sites: Default::default(),
            own_routes: Default::default(),
        };
        let mut rays = SPOTS.len();
        p.spot_for(&self.squads[&id], gunner, target, &[], &mut rays)
    }
}
