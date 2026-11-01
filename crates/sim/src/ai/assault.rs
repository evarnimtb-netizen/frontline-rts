//! S3b AI rules (`docs/slices/S3b-balance.md` B3–B5): fire-and-move with
//! a base of fire on `SuppressUnits`, AT-team hunts and satchel hunts.
//! Every distance and time is rules data (`[ai.assault]`); see the module
//! docs of [`crate::ai`], step 8.

use frontline_content::{ItemKind, SquadRole};

use super::{AiState, Known, KnownVehicle, Mine, MyTank, Planner, PointEval, Rel, TaskKind, sq_m};
use crate::command::CommandKind;
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{Order, SquadId};

/// Most enemy vehicles the AI times as slow (satchel hunts).
pub const MAX_SLOW: usize = 256;
/// Base-of-fire spots tried per plan and squad (one terrain ray each).
const SPOT_TRIES: usize = 8;

/// Squared distance from `p` to segment `[a, b]` (Q32 metres²).
fn seg_dist_sq(p: FxVec2, a: FxVec2, b: FxVec2) -> u64 {
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
        self.slow_since.len() <= MAX_SLOW && self.slow_since.values().all(|&t| t <= tick)
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
        let body = &g.setup.rules.combat.body;
        let mut claimed = Vec::new();
        for (i, pt) in points.iter().enumerate() {
            if pt.rel == Rel::Own {
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
                        && matches!(
                            m.role,
                            SquadRole::Rifle | SquadRole::Mg | SquadRole::Engineer
                        )
                })
                .collect();
            if here.len() < 2 || free.is_empty() {
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
                    SquadRole::Mg => 0,
                    SquadRole::Rifle => 1,
                    _ => 2,
                };
                (!cont, role, m.id)
            };
            let Some(base) = free
                .iter()
                .copied()
                .filter(|m| m.role != SquadRole::Engineer)
                .min_by_key(|m| rank(m))
            else {
                continue;
            };
            let Some(line) = self.assault_line(base.lead, i, points, known) else {
                continue;
            };
            let movers: Vec<&Mine> = here.iter().copied().filter(|m| m.id != base.id).collect();
            // Lift: a manoeuvre squad is at the line.
            let lift = sq_m(Fx::from_int(cfg.lift_fire_m as i32));
            if movers
                .iter()
                .any(|m| seg_dist_sq(m.lead, line[0], line[1]) <= lift)
            {
                continue;
            }
            let (support, height) = match base.role {
                SquadRole::Mg => (cfg.support_mg_m, body.prone_mm),
                _ => (cfg.support_rifle_m, body.stand_mm - body.muzzle_drop_mm),
            };
            let sq = &g.squads[&base.id];
            let mid = (line[0] + line[1]).div_int(2);
            let within = (mid - base.lead).length_sq_raw() <= sq_m(Fx::from_int(support as i32));
            if within && self.fire_line(base.lead, height, &line) {
                let on = matches!(&sq.order, Order::Suppress { line: l } if same_line(l, &line, 4));
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
            } else {
                let spot = self.support_spot(base.lead, &line, support, height);
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
            }
            claimed.push(base.id);
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

    /// AT-team and satchel hunts (module docs of [`crate::ai`], step 8).
    /// Returns the squads on a hunt (the infantry orders and escorts skip
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
        if kv.is_empty() {
            return out;
        }
        for (j, m) in mine.iter().enumerate() {
            // The point the plan assigned it (a hunt keeps no point of its
            // own, so this does not flip between plans).
            let home = assigned
                .iter()
                .find(|a| a.0 == j)
                .map_or(m.lead, |a| points[a.1].pos);
            let spot = match m.role {
                SquadRole::AntiTank => self.at_hunt(m, home, known, kv),
                SquadRole::Engineer => self.satchel_hunt(m, kv),
                _ => None,
            };
            let Some((spot, hold)) = spot else { continue };
            let was = self.ai.tasks.get(&m.id).map(|t| t.kind) == Some(TaskKind::Hunt);
            if !was {
                match m.role {
                    SquadRole::AntiTank => self.ai.counters.at_hunts += 1,
                    _ => self.ai.counters.satchel_hunts += 1,
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
            self.task(m.id, TaskKind::Hunt, super::NO_POINT, spot);
            out.push(m.id);
        }
        out
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
    ) -> Option<(FxVec2, bool)> {
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
        let v = kv
            .iter()
            .filter(|v| {
                (v.pos - m.lead).length_sq_raw() <= r
                    || (v.pos - home).length_sq_raw() <= r
                    || last.is_some_and(|s| (v.pos - s).length_sq_raw() <= r)
            })
            .min_by_key(|v| ((v.pos - m.lead).length_sq_raw(), v.id))?;
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
            return Some((m.lead, true));
        }
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
            .min_by_key(|s| ((*s - m.lead).length_sq_raw(), s.x, s.y))
            .map(|s| (s, false))
    }

    /// An engineer squad's spot beside the nearest enemy vehicle that has
    /// been slow long enough, within `ai.assault.satchel_hunt_m`; `(spot,
    /// false)`.
    fn satchel_hunt(&self, m: &Mine, kv: &[KnownVehicle]) -> Option<(FxVec2, bool)> {
        let g = self.g;
        let cfg = &g.setup.rules.ai.assault;
        let sq = &g.squads[&m.id];
        let carries = sq
            .members
            .iter()
            .any(|id| self.ents.kit[id.index()].items[ItemKind::Satchel.index()] > 0);
        if !carries {
            return None;
        }
        let hz = g.setup.tick_hz.max(1) as u64;
        let since = self.tick.saturating_sub(cfg.satchel_slow_s as u64 * hz);
        let r = sq_m(Fx::from_int(cfg.satchel_hunt_m as i32));
        let v = kv
            .iter()
            .filter(|v| {
                self.ai.slow_since.get(&v.id).is_some_and(|&t| t <= since)
                    && (v.pos - m.lead).length_sq_raw() <= r
            })
            .min_by_key(|v| ((v.pos - m.lead).length_sq_raw(), v.id))?;
        let geo = &g.vderived.geo[v.type_idx as usize];
        let left = crate::vehicle::yaw_dir(v.yaw).perp();
        let side = if (m.lead - v.pos).dot(left) >= Fx::ZERO {
            Fx::ONE
        } else {
            -Fx::ONE
        };
        let off = geo.hull_half.y + Fx::from_int(cfg.satchel_stand_m as i32);
        Some((self.terrain.clamp(v.pos + left * (side * off)), false))
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
