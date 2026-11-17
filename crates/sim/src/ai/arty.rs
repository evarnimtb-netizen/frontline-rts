//! S11a1 artillery AI state (docs/slices/S11a1.md 3, rules A1-A6, P1-P3):
//! the infantry-gun and observer-team records the AI keeps. Build A holds
//! the saved types, their caps and their load check only; the planner that
//! fills them is Build C's (`[ai.arty]`, every switch 0 until then).

use serde::{Deserialize, Serialize};

use crate::ecs::PlayerId;
use crate::fx::{Fx, FxVec2};
use crate::game::{MatchState, SquadId};

/// Most infantry-gun records one AI keeps (`AiState::arty`).
pub const MAX_ARTY: usize = 8;
/// Most observer-team records one AI keeps (`AiState::fo`).
pub const MAX_FO: usize = 8;

/// Where an AI infantry gun stands in its job.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum ArtyPhase {
    /// Ordered to emplace where it arrived (A2).
    #[default]
    Emplace,
    /// Emplaced: takes missions (A4).
    Ready,
}

/// One AI infantry gun, keyed by the gun's squad (saved, hashed).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct AiArty {
    pub phase: ArtyPhase,
    /// Tick the phase began.
    pub since: u64,
    /// Tick of the last order sent to the gun.
    pub last_order: u64,
    /// Missions ordered.
    pub missions: u16,
    /// The running mission is preparation fire (P1).
    pub prep: bool,
}

/// One AI observer team, keyed by its squad (saved, hashed).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct AiFo {
    /// The point the post looks at.
    pub point: u8,
    /// The chosen post, once found.
    pub post: Option<FxVec2>,
    /// The next candidate the post search tests (A3).
    pub cursor: u8,
    /// Tick the record began or the search restarted.
    pub since: u64,
    /// The team reached its post.
    pub arrived: bool,
    /// No new search before this tick (`post_retry_s`).
    pub retry_at: u64,
}

/// Candidates the A3 post search can test (5 distances x 3 sides).
pub const FO_CANDIDATES: u8 = 15;

/// Load check of the S11a1 AI records of `player` at `tick` (spec 3): keys
/// are squads that exist or existed (`< next_squad`; a squad that still
/// exists must be the player's: records of dead squads are dropped at the
/// next plan, so a save between may hold one), sizes within the caps, the
/// post on the map, the point in range, times not after `tick`.
pub(super) fn validate_arty(
    g: &MatchState,
    player: PlayerId,
    tick: u64,
    arty: &std::collections::BTreeMap<SquadId, AiArty>,
    fo: &std::collections::BTreeMap<SquadId, AiFo>,
    fo_lost_at: u64,
) -> bool {
    let size = Fx::from_int(g.setup.map.size_m);
    let on_map = |p: FxVec2| p.x >= Fx::ZERO && p.y >= Fx::ZERO && p.x <= size && p.y <= size;
    let ok_squad =
        |id: &SquadId| id.0 < g.next_squad && g.squads.get(id).is_none_or(|sq| sq.owner == player);
    arty.len() <= MAX_ARTY
        && fo.len() <= MAX_FO
        && fo_lost_at <= tick
        && arty
            .iter()
            .all(|(id, a)| ok_squad(id) && a.since <= tick && a.last_order <= tick)
        && fo.iter().all(|(id, f)| {
            ok_squad(id)
                && (f.point as usize) < g.points.len()
                && f.post.is_none_or(on_map)
                && f.cursor <= FO_CANDIDATES
                && f.since <= tick
                && f.retry_at <= tick + 3600 * g.setup.tick_hz.max(1) as u64
        })
}

// ------------------------------------------------------------ Build C --
//
// The planner (spec 4 A1-A6). Per plan, after the mortars:
//
// * **Records.** Records of squads that are gone, no longer the player's, or
//   no longer a served infantry gun are dropped; an observer record dropped
//   counts `fo_lost` and sets `fo_lost_at`. New own infantry guns (with
//   `buy` 1) and observer teams (with `fo` 1) get a record, ascending id.
// * **A2 Emplace.** A limbered gun gets `Emplace` facing the map centre at
//   most every `[ai.guns] repath_s`; emplaced it is Ready.
// * **A3 Observer.** With a Ready gun and fewer than `fo_max` teams alive
//   or called in, the faction's observer card (a card with an `observer`
//   and a `radio` man) is bought, not before `fo_rebuy_s` after the last
//   loss. A team looks for a post that sees the front point: 15 candidates
//   (`post_m` + 0, -50, +50, -100, +100 m short of the point on the line
//   from the own base, each at 0, +`post_side_m`, -`post_side_m`), passed
//   over near a known enemy (`post_clear_m`), one ray each from a crouching
//   eye, at most `post_rays` a plan (its own count, not the mortars'
//   budget); `cursor` keeps the place. None clear: `fo_post_fails`, the
//   first candidate, a new search after `post_retry_s`. A new front point
//   or a known enemy within [`FO_CLEAR_M`] of the team clears the post.
// * **A4 Fire, A5 Budget.** A Ready gun not on a mission, with targets
//   (the mortar scorer without noise, the gun's band, `danger_close_m`),
//   holding more than `reserve_pct` of its HE load and with its shells
//   bought inside `shell_mp_max_pct` of the income so far, checks the mode
//   of the best `MODE_CHECKS` targets with the rays the mortars left; the
//   first Observed or Direct one that the knowledge record and the sim's
//   own refusal check accept gets `FireMission` of `mission_rounds`. No AI
//   map fire. Checks run and none fit: `arty_no_observer`.
// * **A6 Lift.** A gun on an AI mission whose target has an own man within
//   `danger_close_m` gets `Stop` (`arty_lifts`).
//
// No RNG: every choice is by ascending id or a fixed candidate order.

use frontline_content::VehicleRole;
use frontline_content::setup::member_trait;

use super::{Known, Mine, PointEval, Rel, sq_m};
use crate::command::CommandKind;
use crate::ecs::{EntityKind, SimId};
use crate::game::Order;
use crate::vehicle::GunState;

/// A known enemy this close to an observer team clears its post (m).
pub const FO_CLEAR_M: i32 = 80;
/// An observer team whose leader is this close to its post has arrived (m).
const FO_ARRIVED_M: i32 = 10;
/// The post candidates' steps from `post_m` (m), in order.
const POST_STEPS: [i32; 5] = [0, -50, 50, -100, 100];

impl super::Planner<'_> {
    /// S11a1 A3: whether squad type `t` is an observer team (a man with
    /// `observer` and a man with `radio`; no vehicle).
    fn is_fo_type(&self, t: u16) -> bool {
        let ty = &self.g.setup.squad_types[t as usize];
        ty.vehicle.is_none()
            && ty
                .members
                .iter()
                .any(|m| m.traits & member_trait::OBSERVER != 0)
            && ty
                .members
                .iter()
                .any(|m| m.traits & member_trait::RADIO != 0)
    }

    /// S11a1: the gun of squad `id` when it is an own served infantry gun
    /// on the map.
    fn inf_gun_of(&self, id: SquadId) -> Option<SimId> {
        let g = self.g;
        let sq = g.squads.get(&id).filter(|q| q.owner == self.me)?;
        let gun = *sq.members.first()?;
        let vs = g.vehicles.get(&gun)?;
        (vs.served.is_some()
            && self.ents.kind[gun.index()] == EntityKind::Vehicle
            && g.setup
                .vehicles
                .get(vs.type_idx as usize)
                .is_some_and(|v| v.role == VehicleRole::InfGun))
        .then_some(gun)
    }

    /// S11a1 wiring (spec 4 A1): observer teams leave the lists before
    /// `assign` (no infantry habit orders them). Infantry guns and their
    /// crews already left them in `take_guns`.
    pub(super) fn take_arty(&mut self, mine: &mut Vec<Mine>) {
        let g = self.g;
        mine.retain(|m| !self.is_fo_type(g.squads[&m.id].type_idx));
    }

    /// S11a1 A1: the infantry guns' pot (as `gun_pot` with `[ai.arty]`
    /// keys and role `inf_gun`); 0 with `buy` 0.
    pub(super) fn arty_pot(&self) -> i64 {
        let g = self.g;
        let s = &g.setup;
        let a = &s.rules.ai.arty;
        let hz = s.tick_hz.max(1) as u64;
        if a.buy == 0
            || self.tick < a.after_s as u64 * hz
            || self.of_role(VehicleRole::InfGun) >= a.max_guns as usize
        {
            return 0;
        }
        let is_inf = |t: usize| {
            s.squad_types[t]
                .vehicle
                .is_some_and(|v| s.vehicles[v as usize].role == VehicleRole::InfGun)
        };
        let faction = &s.factions[s.players[self.me.0 as usize].faction as usize];
        let Some(card) = faction
            .roster
            .iter()
            .copied()
            .find(|&t| s.squad_types[t as usize].ai_weight > 0 && is_inf(t as usize))
        else {
            return 0;
        };
        let cost = s.squad_types[card as usize].cost as i64;
        let spent: i64 = self
            .ai
            .counters
            .bought
            .iter()
            .enumerate()
            .filter(|(k, _)| is_inf(*k))
            .map(|(k, &n)| n as i64 * s.squad_types[k].cost as i64)
            .sum();
        let minutes = (self.tick / (60 * hz)) as i64;
        let income = s.rules.start_manpower as i64 + s.rules.income_per_min as i64 * minutes;
        (income * a.income_max_pct as i64 / 100 - spent).clamp(0, cost)
    }

    /// Manpower the other buys leave alone: the AT guns' pot and the
    /// infantry guns' pot (one helper for every place that held
    /// `gun_pot`).
    pub(super) fn held_pots(&self) -> i64 {
        self.gun_pot() + self.arty_pot()
    }

    /// S11a1 A1: the infantry-gun buy.
    pub(super) fn buy_arty(&mut self) {
        let s = &self.g.setup;
        let a = &s.rules.ai.arty;
        let hz = s.tick_hz.max(1) as u64;
        if a.buy == 0
            || self.tick < a.after_s as u64 * hz
            || self.of_role(VehicleRole::InfGun) >= a.max_guns as usize
        {
            return;
        }
        let n = self.ai.pending.len();
        self.buy_role_within(VehicleRole::InfGun, a.income_max_pct, a.reserve_mp);
        if self.ai.pending.len() > n {
            self.ai.counters.arty_buys += 1;
        }
    }

    /// Whether a pending AI command names squad `id`.
    fn arty_busy(&self, id: SquadId) -> bool {
        self.ai
            .pending
            .iter()
            .any(|c| super::squad_of(&c.kind) == Some(id))
    }

    /// The S11a1 plan step (module notes above), after the mortars.
    pub(super) fn arty(&mut self, points: &[PointEval], known: &[Known]) {
        let g = self.g;
        let a = &g.setup.rules.ai.arty;
        if g.setup.rules.fire_mission.is_none() {
            return;
        }
        let tick = self.tick;
        // Records of squads that are gone.
        let gone: Vec<SquadId> = self
            .ai
            .arty
            .keys()
            .copied()
            .filter(|&id| self.inf_gun_of(id).is_none())
            .collect();
        for id in gone {
            self.ai.arty.remove(&id);
        }
        let lost: Vec<SquadId> = self
            .ai
            .fo
            .keys()
            .copied()
            .filter(|id| {
                !g.squads
                    .get(id)
                    .is_some_and(|q| q.owner == self.me && !q.members.is_empty())
            })
            .collect();
        for id in lost {
            self.ai.fo.remove(&id);
            self.ai.counters.fo_lost += 1;
            self.ai.fo_lost_at = tick;
        }
        // New records, ascending id.
        for (&id, q) in &g.squads {
            if q.owner != self.me || q.members.is_empty() {
                continue;
            }
            if a.buy != 0
                && !self.ai.arty.contains_key(&id)
                && self.ai.arty.len() < MAX_ARTY
                && self.inf_gun_of(id).is_some()
            {
                self.ai.arty.insert(
                    id,
                    AiArty {
                        since: tick,
                        ..AiArty::default()
                    },
                );
            }
            if a.fo != 0
                && !self.ai.fo.contains_key(&id)
                && self.ai.fo.len() < MAX_FO
                && self.is_fo_type(q.type_idx)
            {
                self.ai.fo.insert(
                    id,
                    AiFo {
                        point: u8::MAX,
                        since: tick,
                        ..AiFo::default()
                    },
                );
            }
        }
        let own: Vec<FxVec2> = self
            .ents
            .soldiers()
            .filter(|id| self.ents.owner[id.index()] == self.me)
            .map(|id| self.ents.transform[id.index()].pos.xy())
            .collect();
        // A2, A4-A6 per gun, ascending id.
        let ids: Vec<SquadId> = self.ai.arty.keys().copied().collect();
        for id in &ids {
            self.run_arty(*id, known, &own);
        }
        // P3.
        if a.prep != 0 {
            self.lift_mortar_prep(known, &own);
        }
        // A3.
        if a.fo != 0 {
            self.buy_fo();
            let front = points
                .iter()
                .enumerate()
                .filter(|(_, p)| p.rel != Rel::Own || p.threat > 0)
                .max_by_key(|(_, p)| p.priority)
                .or_else(|| points.iter().enumerate().max_by_key(|(_, p)| p.priority))
                .map(|(i, p)| (i as u8, p.pos));
            let fos: Vec<SquadId> = self.ai.fo.keys().copied().collect();
            if let Some(front) = front {
                for id in fos {
                    self.run_fo(id, front, known);
                }
            }
        }
    }

    /// A2 and A4-A6 for gun squad `id`.
    fn run_arty(&mut self, id: SquadId, known: &[Known], own: &[FxVec2]) {
        let g = self.g;
        let s = &g.setup;
        let a = &s.rules.ai.arty;
        let hz = s.tick_hz.max(1) as u64;
        let tick = self.tick;
        let Some(gun) = self.inf_gun_of(id) else {
            return;
        };
        let Some(mut rec) = self.ai.arty.get(&id).copied() else {
            return;
        };
        let vs = &g.vehicles[&gun];
        let state = vs.served.as_ref().map(|sv| sv.state);
        let pos = self.ents.transform[gun.index()].pos.xy();
        let busy = self.arty_busy(id);
        // A2.
        if state == Some(GunState::Emplaced) {
            if rec.phase != ArtyPhase::Ready {
                rec.phase = ArtyPhase::Ready;
                rec.since = tick;
            }
        } else {
            if rec.phase != ArtyPhase::Emplace {
                rec.phase = ArtyPhase::Emplace;
                rec.since = tick;
            }
            let repath = s.rules.ai.guns.repath_s as u64 * hz;
            if state == Some(GunState::Limbered)
                && !busy
                && (rec.last_order == 0 || tick >= rec.last_order + repath)
            {
                let c = Fx::from_int(s.map.size_m / 2);
                self.ai.emit(
                    self.due,
                    CommandKind::Emplace {
                        guns: vec![gun],
                        facing: FxVec2::new(c, c),
                    },
                );
                rec.last_order = tick;
            }
            self.ai.arty.insert(id, rec);
            return;
        }
        let sq = &g.squads[&id];
        // A6: lift a mission with own men near its target.
        if let Order::FireMission { target, .. } = sq.order {
            let r = sq_m(Fx::from_int(a.danger_close_m.min(2000) as i32));
            if !busy && own.iter().any(|o| (*o - target).length_sq_raw() <= r) {
                self.ai.emit(self.due, CommandKind::Stop { squad: id });
                self.ai.counters.arty_lifts += 1;
                rec.last_order = tick;
                rec.prep = false;
            }
            self.ai.arty.insert(id, rec);
            return;
        }
        rec.prep = false;
        if busy {
            self.ai.arty.insert(id, rec);
            return;
        }
        // A4: targets.
        let v = &s.vehicles[vs.type_idx as usize];
        let (Some(k), Some(wi)) = (g.gun_he_slot(v), g.gun_indirect_weapon(sq)) else {
            self.ai.arty.insert(id, rec);
            return;
        };
        let w = &s.weapons[wi as usize];
        let Some(ind) = w.indirect.as_ref() else {
            self.ai.arty.insert(id, rec);
            return;
        };
        let min = Fx::from(w.range_min) + Fx::from_int(20);
        // The table's reach (`range_m`), as the sim's order check reads it.
        let max = Fx::from(ind.range) - Fx::from_int(10);
        let mut targets = self.fire_targets(pos, min, max, a.danger_close_m, false, known, own);
        // P1: the breach job's focus first.
        if let Some(t) = self.prep_target(pos, min, max, a.danger_close_m, known, own) {
            targets.insert(0, t);
        }
        if targets.is_empty() {
            self.ai.arty.insert(id, rec);
            return;
        }
        let held =
            vs.ammo.get(k as usize).copied().unwrap_or(0) as u32 + u32::from(vs.loaded == Some(k));
        let full = v
            .main_gun()
            .and_then(|m| m.ammo.get(k as usize))
            .map_or(0, |l| l.count as u32);
        // A4 / A5: the reserve, then the shell budget.
        if held == 0 || held * 100 <= full * a.reserve_pct {
            self.ai.counters.arty_reserve_holds += 1;
            self.ai.arty.insert(id, rec);
            return;
        }
        let minutes = tick / (60 * hz);
        let income = s.rules.start_manpower as u64 + s.rules.income_per_min as u64 * minutes;
        let shells = g.players[self.me.0 as usize].s11.gun_shell_micro;
        if shells as u128 * 100
            > income as u128 * crate::supply::MICRO as u128 * a.shell_mp_max_pct as u128
        {
            self.ai.counters.arty_budget_holds += 1;
            self.ai.arty.insert(id, rec);
            return;
        }
        let rounds = (a.mission_rounds.min(held) as u16).max(1);
        let mut checked = false;
        for t in targets.iter().take(super::mortar::MODE_CHECKS) {
            if self.mortar_rays < crate::fire_mission::OBSERVE_RAYS {
                break;
            }
            self.mortar_rays -= crate::fire_mission::OBSERVE_RAYS;
            checked = true;
            let (md, _) = g.mode_for(self.ents, self.terrain, id, pos, t.pos, tick);
            if md == crate::fire_mission::mode::MAP
                || !g.known_by_record(self.me, t.pos)
                || g.mission_refusal(self.ents, self.terrain, id, t.pos, rounds, tick)
                    .is_some()
            {
                continue;
            }
            self.ai.emit(
                self.due,
                CommandKind::FireMission {
                    squad: id,
                    target: t.pos,
                    rounds,
                },
            );
            self.ai.counters.arty_missions += 1;
            if t.prep {
                self.ai.counters.arty_prep_missions += 1;
            }
            rec.missions = rec.missions.saturating_add(1);
            rec.last_order = tick;
            rec.prep = t.prep;
            self.ai.arty.insert(id, rec);
            return;
        }
        if checked {
            self.ai.counters.arty_no_observer += 1;
        }
        self.ai.arty.insert(id, rec);
    }

    /// S11a1 P1: the preparation target for a tube at `at` with the band
    /// `min..=max`, when there is one in the band with no own man within
    /// `danger_m` of it (score above every scored target; no RNG).
    pub(super) fn prep_target(
        &self,
        at: FxVec2,
        min: Fx,
        max: Fx,
        danger_m: u32,
        known: &[Known],
        own: &[FxVec2],
    ) -> Option<super::mortar::Target> {
        let f = self.prep_focus(known)?;
        let d = (f - at).length();
        let danger = sq_m(Fx::from_int(danger_m.min(2000) as i32));
        (d >= min && d <= max && !own.iter().any(|o| (*o - f).length_sq_raw() <= danger)).then_some(
            super::mortar::Target {
                score: i64::MAX,
                pos: f,
                prep: true,
            },
        )
    }

    /// S11a1 P3: a mortar mission on the preparation focus (within
    /// `prep_near_m`) with an own man within `[ai.arty] danger_close_m` of
    /// its target is stopped (`arty_lifts`), as A6 stops a gun's.
    fn lift_mortar_prep(&mut self, known: &[Known], own: &[FxVec2]) {
        let g = self.g;
        let a = &g.setup.rules.ai.arty;
        let Some(f) = self.prep_focus(known) else {
            return;
        };
        let near = sq_m(Fx::from_int(a.prep_near_m.min(2000) as i32));
        let r = sq_m(Fx::from_int(a.danger_close_m.min(2000) as i32));
        let lift: Vec<SquadId> = g
            .squads
            .iter()
            .filter(|(id, q)| {
                q.owner == self.me && !self.ai.arty.contains_key(id) && !self.arty_busy(**id)
            })
            .filter_map(|(&id, q)| match q.order {
                Order::FireMission { target, .. }
                    if (target - f).length_sq_raw() <= near
                        && own.iter().any(|o| (*o - target).length_sq_raw() <= r) =>
                {
                    Some(id)
                }
                _ => None,
            })
            .collect();
        for id in lift {
            self.ai.emit(self.due, CommandKind::Stop { squad: id });
            self.ai.counters.arty_lifts += 1;
        }
    }

    /// A3: buys the faction's observer card when a gun is Ready and fewer
    /// than `fo_max` teams are alive or called in.
    fn buy_fo(&mut self) {
        let g = self.g;
        let s = &g.setup;
        let a = &s.rules.ai.arty;
        let hz = s.tick_hz.max(1) as u64;
        if !self.ai.arty.values().any(|r| r.phase == ArtyPhase::Ready)
            || (self.ai.fo_lost_at != 0
                && self.tick < self.ai.fo_lost_at + a.fo_rebuy_s as u64 * hz)
        {
            return;
        }
        let alive = g
            .squads
            .values()
            .filter(|q| q.owner == self.me && !q.members.is_empty() && self.is_fo_type(q.type_idx))
            .count()
            + self
                .ai
                .pending
                .iter()
                .filter(|c| {
                    matches!(c.kind, CommandKind::CallIn { squad_type } if self.is_fo_type(squad_type))
                })
                .count();
        if alive >= a.fo_max as usize {
            return;
        }
        let faction = &s.factions[s.players[self.me.0 as usize].faction as usize];
        let Some(card) = faction.roster.iter().copied().find(|&t| self.is_fo_type(t)) else {
            return;
        };
        let cost = s.squad_types[card as usize].cost as i64;
        let mp = g.players[self.me.0 as usize].manpower as i64;
        if mp - self.held_pots() < cost
            || g.call_in_check(self.ents, self.me, card, self.due).is_err()
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
        c.fo_buys += 1;
    }

    /// The A3 post candidate `k` (0..15) for the point at `front`.
    fn post_candidate(&self, front: FxVec2, k: u8) -> FxVec2 {
        let s = &self.g.setup;
        let a = &s.rules.ai.arty;
        let dir = (front - self.g.base_of(self.me)).normalize_or_zero();
        let side = FxVec2::new(-dir.y, dir.x);
        let d = a.post_m as i32 + POST_STEPS[(k / 3) as usize % POST_STEPS.len()];
        let l = match k % 3 {
            0 => 0,
            1 => a.post_side_m as i32,
            _ => -(a.post_side_m as i32),
        };
        self.terrain
            .clamp(front - dir * Fx::from_int(d.max(0)) + side * Fx::from_int(l))
    }

    /// A3 for observer team `id`: the post search, the move, the crouch.
    fn run_fo(&mut self, id: SquadId, (point, front): (u8, FxVec2), known: &[Known]) {
        let g = self.g;
        let s = &g.setup;
        let a = &s.rules.ai.arty;
        let hz = s.tick_hz.max(1) as u64;
        let tick = self.tick;
        let Some(mut rec) = self.ai.fo.get(&id).copied() else {
            return;
        };
        let sq = &g.squads[&id];
        let lead = self.ents.transform[sq.lead(self.ents).index()].pos.xy();
        if self.arty_busy(id) {
            return;
        }
        let reset = |rec: &mut AiFo| {
            rec.post = None;
            rec.cursor = 0;
            rec.arrived = false;
            rec.retry_at = 0;
            rec.since = tick;
        };
        if rec.point != point {
            reset(&mut rec);
            rec.point = point;
        }
        let near = sq_m(Fx::from_int(FO_CLEAR_M));
        if rec.post.is_some() && known.iter().any(|k| (k.pos - lead).length_sq_raw() <= near) {
            reset(&mut rec);
        }
        if rec.retry_at != 0 && tick >= rec.retry_at {
            reset(&mut rec);
        }
        let Some(post) = rec.post else {
            // The search: one ray a candidate, at most `post_rays` a plan.
            let mut rays = a.post_rays as usize;
            let fm = s.rules.fire_mission.as_ref().expect("fire_mission rules");
            let body = &s.rules.combat.body;
            let to = front.extend(
                crate::fire_mission::ground_point(self.terrain, front)
                    + Fx::from_milli(fm.point_lift_mm as i32),
            );
            let eye_up = Fx::from_milli(body.crouch_mm as i32 - body.eye_drop_mm as i32);
            let clear = sq_m(Fx::from_int(a.post_clear_m.min(2000) as i32));
            let key = SimId(sq.members[0].0);
            let mut found = None;
            while rec.cursor < FO_CANDIDATES && rays > 0 {
                let p = self.post_candidate(front, rec.cursor);
                rec.cursor += 1;
                if known.iter().any(|k| (k.pos - p).length_sq_raw() <= clear) {
                    continue;
                }
                rays -= 1;
                let eye = p.extend(self.terrain.height_at(p) + eye_up);
                if self.terrain.line_of_sight(eye, to) && !g.smoke_blocks(eye, to, tick, (key, key))
                {
                    found = Some(p);
                    break;
                }
            }
            let post = match found {
                Some(p) => {
                    self.ai.counters.fo_posts += 1;
                    p
                }
                None if rec.cursor >= FO_CANDIDATES => {
                    self.ai.counters.fo_post_fails += 1;
                    rec.retry_at = tick + a.post_retry_s.max(1) as u64 * hz;
                    rec.cursor = 0;
                    self.post_candidate(front, 0)
                }
                None => {
                    self.ai.fo.insert(id, rec);
                    return;
                }
            };
            rec.post = Some(post);
            rec.arrived = false;
            rec.since = tick;
            self.ai.emit(
                self.due,
                CommandKind::Move {
                    squad: id,
                    target: post,
                },
            );
            self.ai.fo.insert(id, rec);
            return;
        };
        if !rec.arrived {
            if (lead - post).length_sq_raw() <= sq_m(Fx::from_int(FO_ARRIVED_M)) {
                rec.arrived = true;
                self.ai.emit(
                    self.due,
                    CommandKind::SetStance {
                        units: sq.members.clone(),
                        stance: crate::unit::StanceOrder::Crouch,
                    },
                );
            } else {
                let on_way = matches!(sq.order, Order::Move { target }
                    if (target - post).length_sq_raw() <= sq_m(Fx::from_int(FO_ARRIVED_M)));
                let repath = s.rules.ai.guns.repath_s as u64 * hz;
                if !on_way && tick >= rec.since + repath {
                    self.ai.emit(
                        self.due,
                        CommandKind::Move {
                            squad: id,
                            target: post,
                        },
                    );
                    rec.since = tick;
                }
            }
        }
        self.ai.fo.insert(id, rec);
    }
}
