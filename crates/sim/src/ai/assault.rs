//! S3b AI rules (`docs/slices/S3b-balance.md` B3–B5): fire-and-move with
//! a base of fire on `SuppressUnits`, AT-team hunts and satchel hunts.
//! Every distance and time is rules data (`[ai.assault]`); see the module
//! docs of [`crate::ai`], step 8.

use frontline_content::ItemKind;

use super::AiKind;

use super::{
    AiHunt, AiState, Known, KnownVehicle, Mine, MyTank, NO_POINT, Planner, PointEval, TaskKind,
    sq_m,
};
use crate::command::CommandKind;
use crate::ecs::EntityKind;
use crate::ecs::SimId;
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{Order, SquadId};
use crate::unit::StanceOrder;

/// Most enemy vehicles the AI times as slow (satchel hunts).
pub const MAX_SLOW: usize = 256;
/// Most sticky satchel hunts the AI keeps (S3c B2).
pub const MAX_HUNTS: usize = 64;
/// Why a sticky satchel hunt ended (`AiCounters::hunt_ends` index): the
/// vehicle became a wreck or was abandoned ...
pub const END_GONE: usize = 0;
/// ... it was unseen longer than `satchel_stick_s` ...
pub const END_UNSEEN: usize = 1;
/// ... it drove faster than slow longer than `satchel_chase_s` ...
pub const END_FAST: usize = 2;
/// ... it got beyond the leash ...
pub const END_LEASH: usize = 3;
/// ... `satchel_hunt_max_s` passed ...
pub const END_TIMEOUT: usize = 4;
/// ... the squad has no satchel left (thrown, or its carrier fell) ...
pub const END_SPENT: usize = 5;
/// ... the squad retreats or is gone.
pub const END_RETREAT: usize = 6;
pub const HUNT_ENDS: usize = 7;
/// Report names of the `END_*` reasons.
pub const HUNT_END_NAMES: [&str; HUNT_ENDS] = [
    "gone", "unseen", "fast", "leash", "timeout", "spent", "retreat",
];
/// Base-of-fire spots tried per plan and squad (one terrain ray each).
const SPOT_TRIES: usize = 8;

/// Squared distance from `p` to segment `[a, b]` (Q32 metres²).
pub(super) fn seg_dist_sq(p: FxVec2, a: FxVec2, b: FxVec2) -> u64 {
    let d = b - a;
    let len_sq = d.length_sq_raw();
    if len_sq == 0 {
        return (p - a).length_sq_raw();
    }
    let t = ((p - a).dot_raw(d).clamp(0, len_sq as i64) as i128 * Fx::ONE.raw() as i128
        / len_sq as i128) as i32;
    (p - (a + d * Fx::from_raw(t))).length_sq_raw()
}

/// `line` within `r` metres of `other` at both ends (the same order).
fn same_line(line: &[FxVec2; 2], other: &[FxVec2; 2], r: i32) -> bool {
    let r = sq_m(Fx::from_int(r));
    (line[0] - other[0]).length_sq_raw() <= r && (line[1] - other[1]).length_sq_raw() <= r
}

impl AiState {
    /// Checks the S3b AI state of a loaded save (part of
    /// [`AiState::validate`]).
    pub(crate) fn validate_assault(&self, tick: u64) -> bool {
        self.slow_since.len() <= MAX_SLOW
            && self.slow_since.values().all(|&t| t <= tick)
            && self.hunts.len() <= MAX_HUNTS
            && self.hunts.values().all(|h| {
                h.since <= tick
                    && h.last_seen <= tick
                    && h.since <= h.last_seen
                    && h.fast_since.is_none_or(|f| f <= tick)
            })
    }
}

impl Planner<'_> {
    /// Remembers since when each seen enemy vehicle has been slower than
    /// `ai.infantry.satchel_slow_mm_s`; unseen or faster ones are dropped.
    pub(super) fn note_slow(&mut self, kv: &[KnownVehicle]) {
        let slow = Fx::from_milli(self.g.setup.rules.ai.infantry.satchel_slow_mm_s as i32);
        let tick = self.tick;
        self.ai.slow_since.retain(|id, _| {
            kv.iter()
                .any(|v| v.id == *id && v.speed.abs() <= slow && v.vel.length() <= slow)
        });
        for v in kv {
            if v.speed.abs() <= slow
                && v.vel.length() <= slow
                && self.ai.slow_since.len() < MAX_SLOW
            {
                self.ai.slow_since.entry(v.id).or_insert(tick);
            }
        }
    }

    /// Whether a man at `from` (this high above the ground there, mm) has
    /// a line of fire to the middle of `line` (`crate::combat::suppress_aim`).
    fn fire_line(&self, from: FxVec2, height_mm: u32, line: &[FxVec2; 2]) -> bool {
        let z = self.terrain.height_at(from) + Fx::from_milli(height_mm as i32);
        let eye = FxVec3::new(from.x, from.y, z);
        crate::combat::suppress_aim(
            self.terrain,
            &self.g.setup.rules.combat,
            line,
            Fx::HALF,
            eye,
            Fx::from_int(10_000),
        )
        .is_some_and(|(_, probe)| self.terrain.line_of_sight(eye, probe))
    }

    /// The line a base of fire suppresses for an attack on point `i` from
    /// `from`: across the approach at the probed enemy trench, else at the
    /// middle of the spotted enemies near the point; `ai.assault.trench_line_m`
    /// long.
    pub(super) fn assault_line(
        &self,
        from: FxVec2,
        i: usize,
        points: &[PointEval],
        known: &[Known],
    ) -> Option<[FxVec2; 2]> {
        let at = points[i].pos;
        let dir = (at - from).normalize_or_zero();
        if dir == FxVec2::ZERO {
            return None;
        }
        let centre = self.enemy_trench(from, at).or_else(|| {
            let r = sq_m(self.g.setup.rules.ai.threat_radius.into());
            let near: Vec<FxVec2> = known
                .iter()
                .filter(|k| (k.pos - at).length_sq_raw() <= r)
                .map(|k| k.pos)
                .collect();
            if near.is_empty() {
                return None;
            }
            let n = near.len() as i64;
            let sx: i64 = near.iter().map(|p| p.x.raw() as i64).sum();
            let sy: i64 = near.iter().map(|p| p.y.raw() as i64).sum();
            Some(FxVec2::new(
                Fx::from_raw((sx / n) as i32),
                Fx::from_raw((sy / n) as i32),
            ))
        })?;
        let len = self.g.setup.rules.ai.assault.trench_line_m as i32;
        let half = dir.perp() * Fx::from_int(len).div_int(2);
        Some([
            self.terrain.clamp(centre - half),
            self.terrain.clamp(centre + half),
        ])
    }

    /// A spot within `support` m of `line`'s middle with a line of fire to
    /// it, on the side of `from` (at most [`SPOT_TRIES`] rays), else the
    /// first candidate.
    fn support_spot(
        &self,
        from: FxVec2,
        line: &[FxVec2; 2],
        support: u32,
        height_mm: u32,
        avoid: Option<(FxVec2, u64)>,
    ) -> FxVec2 {
        let mid = (line[0] + line[1]).div_int(2);
        let back = {
            let d = (from - mid).normalize_or_zero();
            if d == FxVec2::ZERO {
                -line_dir(line).perp()
            } else {
                d
            }
        };
        let deg20 = Fx::from_int(20).mul(Fx::PI).div_int(180);
        let mut first = None;
        let mut tries = 0;
        for pct in [85, 65, 45] {
            let dist = Fx::from_int(support as i32).mul_int(pct).div_int(100);
            for turn in [Fx::ZERO, deg20, -deg20] {
                if tries >= SPOT_TRIES {
                    break;
                }
                tries += 1;
                let (s, c) = turn.sin_cos();
                let dir = FxVec2::new(back.x * c - back.y * s, back.x * s + back.y * c);
                let spot = self.terrain.clamp(mid + dir * dist);
                // S3c: a second base keeps its distance from the first.
                if avoid.is_some_and(|(a, r)| (spot - a).length_sq_raw() < r) {
                    continue;
                }
                first.get_or_insert(spot);
                if self.fire_line(spot, height_mm, line) {
                    return spot;
                }
            }
        }
        first.unwrap_or(from)
    }

    /// Fire-and-move (module docs of [`crate::ai`], step 8): for every
    /// non-own point with at least two of the squads assigned to it, one
    /// suppresses while the others advance in bounds. Returns the squads
    /// it gave an order or a hold (the infantry orders skip them).
    pub(super) fn fire_and_move(
        &mut self,
        mine: &[Mine],
        assigned: &[(usize, usize)],
        points: &[PointEval],
        known: &[Known],
        skip: &[SquadId],
    ) -> Vec<SquadId> {
        let g = self.g;
        let cfg = &g.setup.rules.ai.assault;
        let mut claimed = Vec::new();
        for (i, pt) in points.iter().enumerate() {
            if pt.rel.friendly() {
                continue;
            }
            let here: Vec<&Mine> = assigned
                .iter()
                .filter(|a| a.1 == i)
                .map(|a| &mine[a.0])
                .collect();
            let free: Vec<&Mine> = here
                .iter()
                .copied()
                .filter(|m| {
                    !skip.contains(&m.id)
                        && matches!(m.role, AiKind::Rifle | AiKind::Mg | AiKind::Engineer)
                })
                .collect();
            // S10a-ai: a breach squad of this point counts toward the
            // bases (it left `mine` with its job).
            let breaching = self
                .ai
                .breaches
                .values()
                .filter(|b| b.point as usize == i)
                .count();
            let n_here = here.len() + breaching;
            if n_here < 2 || free.is_empty() {
                continue;
            }
            // The base: the squad already suppressing for this point, else
            // an MG team, else a rifle squad (lowest id).
            let rank = |m: &Mine| {
                let cont = self
                    .ai
                    .tasks
                    .get(&m.id)
                    .is_some_and(|t| t.kind == TaskKind::Suppress && t.point == i as u8);
                let role = match m.role {
                    AiKind::Mg => 0,
                    AiKind::Rifle => 1,
                    _ => 2,
                };
                (!cont, role, m.id)
            };
            let Some(base) = free
                .iter()
                .copied()
                .filter(|m| m.role != AiKind::Engineer)
                .min_by_key(|m| rank(m))
            else {
                continue;
            };
            // S10a-ai: during a breach the base aims at the defender
            // nearest the crossing (`ai::breach`), not the point's middle.
            let focus = self.breach_focus(i, known).map(|at| {
                let len = cfg.trench_line_m as i32;
                let half =
                    (at - base.lead).normalize_or_zero().perp() * Fx::from_int(len).div_int(2);
                [self.terrain.clamp(at - half), self.terrain.clamp(at + half)]
            });
            let Some(line) = focus.or_else(|| self.assault_line(base.lead, i, points, known))
            else {
                continue;
            };
            // S3c (B4): a second base when the first one's fire weight is
            // under the enemy's strongest MG team weapon's.
            let second = if n_here >= 3
                && cfg.second_base_below_pct > 0
                && self.fire_weight(base) * 100
                    < self.enemy_mg_weight() * cfg.second_base_below_pct as u64
            {
                let rank2 = |m: &Mine| {
                    let cont = self
                        .ai
                        .tasks
                        .get(&m.id)
                        .is_some_and(|t| t.kind == TaskKind::Suppress && t.point == i as u8);
                    let role = match m.role {
                        AiKind::Mg => 0,
                        _ => 1,
                    };
                    (!cont, role, m.id)
                };
                free.iter()
                    .copied()
                    .filter(|m| m.id != base.id && matches!(m.role, AiKind::Mg | AiKind::Rifle))
                    .min_by_key(|m| rank2(m))
            } else {
                None
            };
            let movers: Vec<&Mine> = here
                .iter()
                .copied()
                .filter(|m| m.id != base.id && second.is_none_or(|b2| b2.id != m.id))
                .collect();
            // Lift: a manoeuvre squad is at the line.
            let lift = sq_m(Fx::from_int(cfg.lift_fire_m as i32));
            if movers
                .iter()
                .any(|m| seg_dist_sq(m.lead, line[0], line[1]) <= lift)
            {
                continue;
            }
            let first_at = self.base_of_fire(base, i, &line, None);
            claimed.push(base.id);
            if let Some(b2) = second {
                let was = self
                    .ai
                    .tasks
                    .get(&b2.id)
                    .is_some_and(|t| t.kind == TaskKind::Suppress && t.point == i as u8);
                if !was {
                    self.ai.counters.second_bases += 1;
                }
                self.base_of_fire(b2, i, &line, Some(first_at));
                claimed.push(b2.id);
            }
            let mid = (line[0] + line[1]).div_int(2);
            // Bounds: a manoeuvre squad well ahead of the rearmost halts and
            // fires until the others close up.
            let free_movers: Vec<&Mine> = movers
                .iter()
                .copied()
                .filter(|m| !skip.contains(&m.id))
                .collect();
            if free_movers.len() < 2 {
                continue;
            }
            let dist = |m: &Mine| (mid - m.lead).length();
            let rear = free_movers
                .iter()
                .map(|m| dist(m))
                .max()
                .unwrap_or(Fx::ZERO);
            let bound = Fx::from_int(cfg.bound_m as i32);
            for m in free_movers {
                if rear - dist(m) <= bound {
                    continue;
                }
                let sq = &g.squads[&m.id];
                if sq.order != Order::Idle {
                    self.ai.emit(self.due, CommandKind::Stop { squad: m.id });
                    self.ai.counters.bounds += 1;
                }
                self.task(m.id, TaskKind::Attack, i as u8, m.lead);
                claimed.push(m.id);
            }
        }
        claimed.sort_unstable();
        claimed.dedup();
        claimed
    }

    /// Squad `base` as a base of fire on `line` for point `i`: in reach
    /// and with a line of fire it suppresses the line, else it walks to a
    /// support spot (at least `ai.assault.second_base_gap_m` from `avoid`,
    /// the first base, for a second base). Returns where it fires from.
    pub(super) fn base_of_fire(
        &mut self,
        base: &Mine,
        i: usize,
        line: &[FxVec2; 2],
        avoid: Option<FxVec2>,
    ) -> FxVec2 {
        let g = self.g;
        let cfg = &g.setup.rules.ai.assault;
        let body = &g.setup.rules.combat.body;
        let (support, height) = match base.role {
            AiKind::Mg => (cfg.support_mg_m, body.prone_mm),
            _ => (cfg.support_rifle_m, body.stand_mm - body.muzzle_drop_mm),
        };
        let gap = avoid.map(|a| (a, sq_m(Fx::from_int(cfg.second_base_gap_m as i32))));
        let apart = |p: FxVec2| gap.is_none_or(|(a, r)| (p - a).length_sq_raw() >= r);
        let sq = &g.squads[&base.id];
        let mid = (line[0] + line[1]).div_int(2);
        let within = (mid - base.lead).length_sq_raw() <= sq_m(Fx::from_int(support as i32));
        if within && apart(base.lead) && self.fire_line(base.lead, height, line) {
            let on = matches!(&sq.order, Order::Suppress { line: l } if same_line(l, line, 4));
            if !on {
                let mut units = sq.members.clone();
                units.truncate(crate::unit::MAX_UNITS);
                self.ai.emit(
                    self.due,
                    CommandKind::SuppressUnits {
                        units,
                        line: line.to_vec(),
                    },
                );
                self.ai.counters.suppress_orders += 1;
            }
            self.task(base.id, TaskKind::Suppress, i as u8, mid);
            return base.lead;
        }
        let spot = self.support_spot(base.lead, line, support, height, gap);
        let near = |a: FxVec2, r: i32| (a - spot).length_sq_raw() <= sq_m(Fx::from_int(r));
        let on_track = match &sq.order {
            Order::Idle => near(base.lead, 15),
            Order::Move { target } => near(*target, 10),
            _ => false,
        };
        if !on_track {
            self.ai.emit(
                self.due,
                CommandKind::Move {
                    squad: base.id,
                    target: spot,
                },
            );
        }
        self.task(base.id, TaskKind::Suppress, i as u8, spot);
        spot
    }

    /// A squad's fire weight (S3c B4): rounds per minute × suppression per
    /// round, summed over its men (S4: each by the weapon he carries).
    fn fire_weight(&self, m: &Mine) -> u64 {
        let sq = &self.g.squads[&m.id];
        sq.members
            .iter()
            .map(|p| self.weapon_weight(self.ents.kit()[p.index()].weapon))
            .sum()
    }

    /// One weapon's rounds per minute × suppression per round.
    fn weapon_weight(&self, k: u16) -> u64 {
        let g = self.g;
        let Some(w) = g.setup.weapons.get(k as usize) else {
            return 0;
        };
        let rpm = 60 * g.setup.tick_hz as u64 * 1000 / w.interval_milli.max(1) as u64;
        rpm * w.suppression as u64
    }

    /// The strongest MG team weapon in the enemy factions' rosters (S3c
    /// B4; doctrine knowledge, not sight).
    fn enemy_mg_weight(&self) -> u64 {
        let g = self.g;
        let mine = g.setup.players[self.me.0 as usize].faction;
        g.setup
            .squad_types
            .iter()
            .enumerate()
            .filter(|&(k, t)| {
                super::ai_kind(&g.setup, k as u16) == AiKind::Mg
                    && t.faction != mine
                    && g.setup.players.iter().any(|p| p.faction == t.faction)
            })
            .map(|(_, t)| self.weapon_weight(t.weapon))
            .max()
            .unwrap_or(0)
    }

    /// A vehicle halted at its assault or standoff goal suppresses the
    /// enemy trench (or the enemies seen) at point `i` while its escort
    /// goes in, and lifts once own men are at the line. Returns whether it
    /// holds a Suppress order now (it is then not turned to face threats).
    pub(super) fn tank_suppress(
        &mut self,
        t: &MyTank,
        i: usize,
        points: &[PointEval],
        known: &[Known],
        mine: &[Mine],
    ) -> bool {
        let g = self.g;
        let cfg = &g.setup.rules.ai.assault;
        let vs = &g.vehicles[&t.id];
        let Some(line) = self.assault_line(t.pos, i, points, known) else {
            return vs.suppress.is_some();
        };
        let lift = sq_m(Fx::from_int(cfg.lift_fire_m as i32));
        let at_line = mine
            .iter()
            .any(|m| seg_dist_sq(m.lead, line[0], line[1]) <= lift);
        let mid = (line[0] + line[1]).div_int(2);
        let far = (mid - t.pos).length_sq_raw() > sq_m(Fx::from_int(cfg.support_tank_m as i32));
        // The turret's MG height, roughly: the def's turret top.
        let height = 2200;
        if at_line || far || !self.fire_line(t.pos, height, &line) {
            if vs.suppress.is_some() {
                self.ai.emit(self.due, CommandKind::Stop { squad: t.squad });
            }
            return false;
        }
        if !vs.suppress.is_some_and(|l| same_line(&l, &line, 4)) {
            self.ai.emit(
                self.due,
                CommandKind::SuppressUnits {
                    units: vec![t.id],
                    line: line.to_vec(),
                },
            );
            self.ai.counters.suppress_orders += 1;
        }
        true
    }

    /// AT-team and satchel hunts (module docs of [`crate::ai`], step 8),
    /// and (S3c) AT teams following an own vehicle's assault. Returns the
    /// squads on a hunt or a follow (the infantry orders and escorts skip
    /// them).
    pub(super) fn hunts(
        &mut self,
        mine: &[Mine],
        assigned: &[(usize, usize)],
        points: &[PointEval],
        known: &[Known],
        kv: &[KnownVehicle],
    ) -> Vec<SquadId> {
        let mut out = Vec::new();
        self.prune_hunts();
        for (j, m) in mine.iter().enumerate() {
            // The point the plan assigned it (a hunt keeps no point of its
            // own, so this does not flip between plans).
            let home = assigned
                .iter()
                .find(|a| a.0 == j)
                .map_or(m.lead, |a| points[a.1].pos);
            let pick = match m.role {
                AiKind::AntiTank => self
                    .at_hunt(m, home, known, kv)
                    .map(|(s, h, stuck)| (s, h, TaskKind::Hunt, stuck))
                    .or_else(|| {
                        self.at_follow(m)
                            .map(|s| (s, false, TaskKind::Follow, false))
                    }),
                AiKind::Engineer => self
                    .satchel_hunt(m, kv)
                    .map(|(s, h)| (s, h, TaskKind::Hunt, false)),
                // S4: squads carrying a Panzerfaust hunt close vehicles.
                _ => self
                    .pzf_hunt(m, known, kv)
                    .map(|(s, h)| (s, h, TaskKind::Hunt, false)),
            };
            let Some((spot, hold, kind, stuck)) = pick else {
                continue;
            };
            let was = self.ai.tasks.get(&m.id).map(|t| t.kind) == Some(kind);
            if !was {
                match (m.role, kind) {
                    (_, TaskKind::Follow) => self.ai.counters.at_follows += 1,
                    (AiKind::AntiTank, _) => {
                        self.ai.counters.at_hunts += 1;
                        self.ai.counters.at_stuck_hunts += u32::from(stuck);
                    }
                    (AiKind::Engineer, _) => self.ai.counters.satchel_hunts += 1,
                    _ => self.ai.counters.pzf_hunts += 1,
                }
            }
            let sq = &self.g.squads[&m.id];
            let near = |a: FxVec2, r: i32| (a - spot).length_sq_raw() <= sq_m(Fx::from_int(r));
            let cmd = if hold {
                (sq.order != Order::Idle).then_some(CommandKind::Stop { squad: m.id })
            } else {
                let on_track = match &sq.order {
                    Order::Move { target } => near(*target, 15),
                    Order::Idle => near(m.lead, 4),
                    _ => false,
                };
                (!on_track).then_some(CommandKind::Move {
                    squad: m.id,
                    target: spot,
                })
            };
            if let Some(c) = cmd {
                self.ai.emit(self.due, c);
            }
            self.task(m.id, kind, NO_POINT, spot);
            if m.role == AiKind::Engineer {
                self.hunt_stance(m);
            }
            out.push(m.id);
        }
        out
    }

    /// Ends the hunts of squads that are gone, no longer the AI's or
    /// retreating (S3c B2); squads with a command still pending keep theirs.
    fn prune_hunts(&mut self) {
        let g = self.g;
        let ids: Vec<SquadId> = self.ai.hunts.keys().copied().collect();
        for id in ids {
            let over = g
                .squads
                .get(&id)
                .is_none_or(|sq| sq.owner != self.me || sq.order == Order::Retreat);
            if over {
                self.end_hunt(id, END_RETREAT);
            }
        }
    }

    /// Ends squad `id`'s sticky hunt for `why` (an `END_*`): counted, and
    /// its men's stance back to automatic.
    fn end_hunt(&mut self, id: SquadId, why: usize) {
        if self.ai.hunts.remove(&id).is_none() {
            return;
        }
        self.ai.counters.hunt_ends[why] += 1;
        let Some(sq) = self.g.squads.get(&id) else {
            return;
        };
        let units: Vec<SimId> = sq
            .members
            .iter()
            .copied()
            .filter(|u| self.ents.unit[u.index()].stance != StanceOrder::Auto)
            .take(crate::unit::MAX_UNITS)
            .collect();
        if !units.is_empty() {
            self.ai.emit(
                self.due,
                CommandKind::SetStance {
                    units,
                    stance: StanceOrder::Auto,
                },
            );
        }
    }

    /// Hunters crouch on the approach and crawl within
    /// `ai.assault.satchel_crawl_m` of the vehicle while its hull MG's arc
    /// covers them (S3c B2).
    fn hunt_stance(&mut self, m: &Mine) {
        let g = self.g;
        let Some(h) = self.ai.hunts.get(&m.id) else {
            return;
        };
        let cfg = &g.setup.rules.ai.assault;
        let crawl = sq_m(Fx::from_int(cfg.satchel_crawl_m as i32));
        let to = m.lead - h.last_pos;
        let covered = cfg.satchel_crawl_m > 0
            && to.length_sq_raw() <= crawl
            && g.vehicles.get(&h.vehicle).is_some_and(|vs| {
                let v = &g.setup.vehicles[vs.type_idx as usize];
                let yaw = self.ents.transform[h.vehicle.index()].yaw;
                let fwd = crate::vehicle::yaw_dir(yaw);
                v.mounts.iter().any(|mt| {
                    mt.kind == frontline_content::vehicle::MountKind::Hull && {
                        let arc = Fx::from_int(mt.arc_deg).mul(Fx::PI).div_int(180);
                        let off = (Fx::atan2(to.y, to.x) - Fx::atan2(fwd.y, fwd.x))
                            .wrap_angle()
                            .abs();
                        off <= arc
                    }
                })
            });
        let want = if covered {
            StanceOrder::Prone
        } else {
            StanceOrder::Crouch
        };
        let units: Vec<SimId> = g.squads[&m.id]
            .members
            .iter()
            .copied()
            .filter(|u| self.ents.unit[u.index()].stance != want)
            .take(crate::unit::MAX_UNITS)
            .collect();
        if !units.is_empty() {
            self.ai.emit(
                self.due,
                CommandKind::SetStance {
                    units,
                    stance: want,
                },
            );
        }
    }

    /// S3c (B3): an AT team with rockets and no hunt follows the nearest
    /// own vehicle on an assault within `ai.assault.at_follow_m`,
    /// `at_follow_back_m` behind it.
    fn at_follow(&self, m: &Mine) -> Option<FxVec2> {
        let g = self.g;
        let cfg = &g.setup.rules.ai.assault;
        if cfg.at_follow_m == 0 {
            return None;
        }
        let sq = &g.squads[&m.id];
        if sq.launcher.as_ref().is_none_or(|l| l.rounds == 0) {
            return None;
        }
        let r = sq_m(Fx::from_int(cfg.at_follow_m as i32));
        let (pos, goal) = self
            .ai
            .tasks
            .iter()
            .filter(|(_, t)| matches!(t.kind, TaskKind::Assault | TaskKind::Standoff))
            .filter_map(|(id, t)| {
                let q = g.squads.get(id)?;
                let &v = q.members.first()?;
                (self.ents.kind[v.index()] == EntityKind::Vehicle)
                    .then(|| (self.ents.transform[v.index()].pos.xy(), t.target))
            })
            .filter(|(p, _)| (*p - m.lead).length_sq_raw() <= r)
            .min_by_key(|(p, _)| ((*p - m.lead).length_sq_raw(), p.x, p.y))?;
        let mut dir = (goal - pos).normalize_or_zero();
        if dir == FxVec2::ZERO {
            dir = (pos - g.base_of(self.me)).normalize_or_zero();
        }
        Some(
            self.terrain
                .clamp(pos - dir * Fx::from_int(cfg.at_follow_back_m as i32)),
        )
    }

    /// An AT team's firing spot against the nearest known enemy vehicle in
    /// reach of the team or its point: `(spot, hold)` with `hold` when it
    /// already stands in range off the turret's front.
    fn at_hunt(
        &self,
        m: &Mine,
        home: FxVec2,
        known: &[Known],
        kv: &[KnownVehicle],
    ) -> Option<(FxVec2, bool, bool)> {
        let g = self.g;
        let cfg = &g.setup.rules.ai.assault;
        let sq = &g.squads[&m.id];
        if sq.launcher.as_ref().is_none_or(|l| l.rounds == 0) {
            return None;
        }
        let w = &g.setup.weapons[g.setup.squad_types[sq.type_idx as usize].weapon as usize];
        let inset = Fx::from_int(cfg.at_hunt_inset_m as i32);
        let reach_of = |e: frontline_content::Fixed| {
            (Fx::from(e).min(w.range_max.into()) - inset).max(Fx::from_int(10))
        };
        let reach = reach_of(w.engage_still);
        let r = sq_m(Fx::from_int(cfg.at_hunt_m as i32));
        // A team already hunting keeps at it near its last spot, so the
        // hunt does not flip with the plan's point assignment.
        let last = self
            .ai
            .tasks
            .get(&m.id)
            .filter(|t| t.kind == TaskKind::Hunt)
            .map(|t| t.target);
        let in_reach = |v: &KnownVehicle| {
            (v.pos - m.lead).length_sq_raw() <= r
                || (v.pos - home).length_sq_raw() <= r
                || last.is_some_and(|s| (v.pos - s).length_sq_raw() <= r)
        };
        // S3c (B3): a stuck vehicle (bogged, immobilised) is stalked from
        // further out: it cannot turn away or leave.
        let stuck_r = sq_m(Fx::from_int(cfg.at_hunt_stuck_m as i32));
        let stuck = |v: &KnownVehicle| {
            (v.pos - m.lead).length_sq_raw() <= stuck_r
                && g.vehicles
                    .get(&v.id)
                    .is_some_and(|vs| vs.immobile(&g.setup.vehicles[vs.type_idx as usize]))
        };
        let v = kv
            .iter()
            .filter(|v| in_reach(v) || stuck(v))
            .min_by_key(|v| ((v.pos - m.lead).length_sq_raw(), v.id))?;
        let far = !in_reach(v);
        let clear = sq_m(Fx::from_int(cfg.at_hunt_clear_m as i32));
        let mid = (m.lead + v.pos).div_int(2);
        if known.iter().any(|k| (k.pos - mid).length_sq_raw() <= clear) {
            return None;
        }
        // In reach for the vehicle's motion (the launcher's own fire
        // discipline): hold and let the gunner fire, whatever the aspect.
        let moving = v.vel.length() > Fx::from(g.setup.rules.vehicles.moving_speed);
        let now = if moving {
            reach_of(w.engage_moving)
        } else {
            reach
        };
        if (m.lead - v.pos).length_sq_raw() <= sq_m(now) {
            return Some((m.lead, true, far));
        }
        self.flank_spot(v, reach, m.lead).map(|s| (s, false, far))
    }

    /// The nearest to `from` of five spots `reach` from vehicle `v` (sides,
    /// rear quarters, rear) off its turret's front.
    pub(super) fn flank_spot(&self, v: &KnownVehicle, reach: Fx, from: FxVec2) -> Option<FxVec2> {
        let g = self.g;
        let fwd = crate::vehicle::yaw_dir(v.yaw);
        let turret = g
            .vehicles
            .get(&v.id)
            .map_or(fwd, |vs| crate::vehicle::yaw_dir(v.yaw + vs.turret_yaw));
        let off_front = |d: FxVec2| d.dot(turret) < Fx::HALF;
        let left = fwd.perp();
        let dirs = [
            left,
            -left,
            (left - fwd).normalize_or_zero(),
            (-left - fwd).normalize_or_zero(),
            -fwd,
        ];
        dirs.iter()
            .copied()
            .filter(|&d| off_front(d))
            .map(|d| self.terrain.clamp(v.pos + d * reach))
            .min_by_key(|s| ((*s - from).length_sq_raw(), s.x, s.y))
    }

    /// S4 Panzerfaust hunt: a squad whose men still carry a single-shot AT
    /// item goes for the nearest known enemy vehicle within
    /// `ai.assault.pzf_hunt_m` of its leader, unless a known enemy soldier
    /// is within `pzf_clear_m` of the route's middle: it holds when the
    /// vehicle is within the item weapon's reach for its motion (the men
    /// fire by the weapon's own rules), else walks to the nearest flank
    /// spot `at_hunt_inset_m` inside its `engage_still`.
    fn pzf_hunt(&self, m: &Mine, known: &[Known], kv: &[KnownVehicle]) -> Option<(FxVec2, bool)> {
        let g = self.g;
        let cfg = &g.setup.rules.ai.assault;
        if cfg.pzf_hunt_m == 0 || kv.is_empty() {
            return None;
        }
        let sq = &g.squads[&m.id];
        sq.pzf?;
        let k = ItemKind::SingleShotAt.index();
        if !sq
            .members
            .iter()
            .any(|p| self.ents.kit()[p.index()].items[k] > 0)
        {
            return None;
        }
        let ty = &g.setup.squad_types[sq.type_idx as usize];
        let (_, it) = ty.kit(&g.setup.items, ItemKind::SingleShotAt)?;
        let w = &g.setup.weapons[it.weapon? as usize];
        let r = sq_m(Fx::from_int(cfg.pzf_hunt_m as i32));
        let v = kv
            .iter()
            .filter(|v| (v.pos - m.lead).length_sq_raw() <= r)
            .min_by_key(|v| ((v.pos - m.lead).length_sq_raw(), v.id))?;
        let clear = sq_m(Fx::from_int(cfg.pzf_clear_m as i32));
        let mid = (m.lead + v.pos).div_int(2);
        if known.iter().any(|k| (k.pos - mid).length_sq_raw() <= clear) {
            return None;
        }
        let inset = Fx::from_int(cfg.at_hunt_inset_m as i32);
        let reach_of = |e: frontline_content::Fixed| {
            (Fx::from(e).min(w.range_max.into()) - inset).max(Fx::from_int(10))
        };
        let moving = v.vel.length() > Fx::from(g.setup.rules.vehicles.moving_speed);
        let now = reach_of(if moving {
            w.engage_moving
        } else {
            w.engage_still
        });
        if (m.lead - v.pos).length_sq_raw() <= sq_m(now) {
            return Some((m.lead, true));
        }
        self.flank_spot(v, reach_of(w.engage_still), m.lead)
            .map(|s| (s, false))
    }

    /// An engineer squad's sticky satchel hunt (S3c B2, module docs of
    /// [`crate::ai`] step 8): keeps its hunt while the rules allow, else
    /// starts one on the nearest enemy vehicle within
    /// `ai.assault.satchel_hunt_m` that has been slow for `satchel_slow_s`
    /// or is stuck. Returns the spot `satchel_stand_m` outside the hull
    /// side nearer the squad (`(spot, false)`).
    fn satchel_hunt(&mut self, m: &Mine, kv: &[KnownVehicle]) -> Option<(FxVec2, bool)> {
        let g = self.g;
        let cfg = &g.setup.rules.ai.assault;
        let hz = g.setup.tick_hz.max(1) as u64;
        let tick = self.tick;
        let sq = &g.squads[&m.id];
        let carries = sq
            .members
            .iter()
            .any(|id| self.ents.kit[id.index()].items[ItemKind::Satchel.index()] > 0);
        if let Some(mut h) = self.ai.hunts.get(&m.id).cloned() {
            let live = self.ents.kind.get(h.vehicle.index()) == Some(&EntityKind::Vehicle)
                && g.vehicles.contains_key(&h.vehicle);
            let slow = Fx::from_milli(g.setup.rules.ai.infantry.satchel_slow_mm_s as i32);
            let mut why = None;
            if !carries {
                why = Some(END_SPENT);
            } else if !live {
                why = Some(END_GONE);
            } else if let Some(v) = kv.iter().find(|v| v.id == h.vehicle) {
                if h.unseen {
                    // Seen again after losing sight of it.
                    self.ai.counters.hunts_kept_unseen += 1;
                    h.unseen = false;
                }
                h.last_seen = tick;
                h.last_pos = v.pos;
                if v.speed.abs() > slow || v.vel.length() > slow {
                    let f = *h.fast_since.get_or_insert(tick);
                    if tick - f > cfg.satchel_chase_s as u64 * hz {
                        why = Some(END_FAST);
                    }
                } else {
                    h.fast_since = None;
                }
            } else if tick - h.last_seen > cfg.satchel_stick_s as u64 * hz {
                why = Some(END_UNSEEN);
            } else {
                h.unseen = true;
            }
            let leash = sq_m(Fx::from_int(
                (cfg.satchel_hunt_m + cfg.satchel_leash_m) as i32,
            ));
            if why.is_none() && (h.last_pos - m.lead).length_sq_raw() > leash {
                why = Some(END_LEASH);
            }
            if why.is_none() && tick - h.since > cfg.satchel_hunt_max_s as u64 * hz {
                why = Some(END_TIMEOUT);
            }
            if let Some(w) = why {
                self.end_hunt(m.id, w);
                return None;
            }
            let spot = self.hunt_spot(m, &h, kv);
            self.ai.hunts.insert(m.id, h);
            return Some((spot, false));
        }
        if !carries || self.ai.hunts.len() >= MAX_HUNTS {
            return None;
        }
        let since = tick.saturating_sub(cfg.satchel_slow_s as u64 * hz);
        let r = sq_m(Fx::from_int(cfg.satchel_hunt_m as i32));
        let v = kv
            .iter()
            .filter(|v| {
                let stuck = g
                    .vehicles
                    .get(&v.id)
                    .is_some_and(|vs| vs.immobile(&g.setup.vehicles[vs.type_idx as usize]));
                (stuck || self.ai.slow_since.get(&v.id).is_some_and(|&t| t <= since))
                    && (v.pos - m.lead).length_sq_raw() <= r
            })
            .min_by_key(|v| ((v.pos - m.lead).length_sq_raw(), v.id))?;
        let h = AiHunt {
            vehicle: v.id,
            since: tick,
            last_seen: tick,
            last_pos: v.pos,
            fast_since: None,
            unseen: false,
        };
        let spot = self.hunt_spot(m, &h, kv);
        self.ai.hunts.insert(m.id, h);
        Some((spot, false))
    }

    /// Where a hunt's squad goes: `satchel_stand_m` outside the hull side
    /// nearer it while the vehicle is seen, else beside where it was last
    /// seen, on the squad's side.
    fn hunt_spot(&self, m: &Mine, h: &AiHunt, kv: &[KnownVehicle]) -> FxVec2 {
        let g = self.g;
        let stand = Fx::from_int(g.setup.rules.ai.assault.satchel_stand_m as i32);
        let half = g.vehicles.get(&h.vehicle).map_or(Fx::from_int(2), |vs| {
            g.vderived.geo[vs.type_idx as usize].hull_half.y
        });
        if let Some(v) = kv.iter().find(|v| v.id == h.vehicle) {
            let left = crate::vehicle::yaw_dir(v.yaw).perp();
            let side = if (m.lead - v.pos).dot(left) >= Fx::ZERO {
                Fx::ONE
            } else {
                -Fx::ONE
            };
            return self.terrain.clamp(v.pos + left * (side * (half + stand)));
        }
        let mut dir = (m.lead - h.last_pos).normalize_or_zero();
        if dir == FxVec2::ZERO {
            dir = FxVec2::new(Fx::ONE, Fx::ZERO);
        }
        self.terrain.clamp(h.last_pos + dir * (half + stand))
    }
}

/// Unit direction of a line (east for a point).
fn line_dir(line: &[FxVec2; 2]) -> FxVec2 {
    let d = (line[1] - line[0]).normalize_or_zero();
    if d == FxVec2::ZERO {
        FxVec2::new(Fx::ONE, Fx::ZERO)
    } else {
        d
    }
}
