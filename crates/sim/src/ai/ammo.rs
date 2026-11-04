//! S5a AI ammunition habits (docs/slices/S5a-ammo.md "AI"; `[ai.ammo]`,
//! `[ai.assault]` ambush keys):
//!
//! * **Reserve and return.** A squad's ammunition share is
//!   [`crate::supply::squad_ammo_pct`]. Below `return_below_pct` with no
//!   known enemy within `threat_radius_m`, or below `dry_pct` at all, it
//!   gets task `Refill`: it walks to a ring spot in the own base (a
//!   `Retreat` when under threat), waits there until `refill_full_pct` or
//!   `refill_max_s`, or until its refill has stood paused (no manpower) for
//!   `rearm_stall_s`, then is free again. **Staggered:** per point (the
//!   squad's task point), at most `max_returning_pct` of the squads tasked
//!   there (at least one) start a refill in one plan, the lowest share
//!   first; a point's last squad holds unless it is dry.
//! * **Mortar reserve** (in `Planner::mortars`): a mortar barrages only
//!   above `mortar_reserve_pct` of its bombs unless an own point is under
//!   threat, and never orders more rounds than it holds.
//! * **Buys** keep `float_mp` back while an own unit waits in base for a
//!   refill (`Planner::waiting_refill`).
//! * **Panzerfaust ambush hold** (S4-42 / T12): a squad with Panzerfausts
//!   (not an AT team or engineers) sees a known enemy vehicle within
//!   `ambush_watch_m` of its point or itself. A moving vehicle: it stays if
//!   it stands in a trench or the vehicle's projected path (its averaged
//!   velocity) passes within the Panzerfaust's
//!   `engage_moving` + `ambush_offset_m` of it; else it moves to the best
//!   of eight spots `ambush_offset_m` to either side of the path 40, 60 and
//!   80 m ahead of the vehicle and where the path passes nearest the
//!   squad, within half `ambush_watch_m` of the squad (dug ground first,
//!   then a spot the vehicle has no terrain line of sight to now, one ray
//!   each, then the nearest; ties by position; none: it stays). A stopped
//!   vehicle is stalked (S5a-30 (A)): the squad goes to the nearest of the
//!   S4 hunt's flank spots off its turret's front inside the Panzerfaust's
//!   `engage_still` (less `at_hunt_inset_m`), or stays if already in
//!   reach. It holds fire (`SetFire hold`) crouched on the way and prone
//!   at the spot; the carriers fire by the launcher's own rule
//!   (`engage_moving` / `engage_still`) and the squad springs to
//!   discipline with the first shot, or when a man is suppressed to
//!   `combat.fire.hold_spring_points` (`crate::unit`). It ends with the last tube, after
//!   `ambush_max_s` (then the S4 hunt may take the squad until the vehicle
//!   is gone), when the vehicle is destroyed or seen beyond
//!   `ambush_release_m` (at least `ambush_watch_m`, S5a Fix review M1; an
//!   unseen vehicle keeps the ambush), or when
//!   the squad retreats; then `SetFire discipline` and the automatic
//!   stance.

use frontline_content::ItemKind;

use super::assault::seg_dist_sq;
use super::{AiAmbush, AiKind, Known, KnownVehicle, Mine, NO_POINT, Planner, PointEval, TaskKind};
use super::{ring_slot, sq_m};
use crate::command::CommandKind;
use crate::ecs::SimId;
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{Order, SquadId};
use crate::unit::{FireMode, StanceOrder};

/// Ambush end reasons (`AiCounters::ambush_ends`).
pub const AMBUSH_SPENT: usize = 0;
pub const AMBUSH_TIMEOUT: usize = 1;
pub const AMBUSH_GONE: usize = 2;
pub const AMBUSH_RETREAT: usize = 3;
pub const AMBUSH_ENDS: usize = 4;
/// Report names of the `AMBUSH_*` reasons.
pub const AMBUSH_END_NAMES: [&str; AMBUSH_ENDS] = ["spent", "timeout", "gone", "retreat"];

/// Distances ahead of the vehicle of the ambush spots (m).
const AHEAD_M: [i32; 3] = [40, 60, 80];

impl Planner<'_> {
    /// S5a: whether an own unit waits in the base for a refill (a squad on
    /// a refill trip that arrived, or a vehicle on a rearm trip).
    pub(super) fn waiting_refill(&self) -> bool {
        // Legacy content refills for free: nothing to keep back.
        self.g.setup.rules.supply.is_some()
            && (self.ai.refill.values().any(|&t| t > 0)
                || self.ai.tasks.values().any(|t| t.kind == TaskKind::Rearm))
    }

    /// S5a: the player's refill stood paused (no manpower) this plan.
    pub(super) fn refill_paused(&self) -> bool {
        let a = &self.g.players[self.me.0 as usize].ammo;
        let hz = self.g.setup.tick_hz.max(1) as u64;
        a.paused_at > 0 && a.paused_at + hz >= self.tick
    }

    /// S5a: refill trips (module docs). Squads on a trip leave `mine`.
    pub(super) fn refills(&mut self, mine: &mut Vec<Mine>, known: &[Known]) {
        let g = self.g;
        let me = self.me;
        // Trips whose squad is gone or took another task end.
        let tasks = &self.ai.tasks;
        self.ai.refill.retain(|id, _| {
            g.squads.get(id).is_some_and(|s| s.owner == me)
                && tasks.get(id).is_some_and(|t| t.kind == TaskKind::Refill)
        });
        self.ai.counters.plans += 1;
        if g.setup.rules.supply.is_none() {
            return;
        }
        let cfg = g.setup.rules.ai.ammo.clone();
        let hz = g.setup.tick_hz.max(1) as u64;
        let base = g.base_of(me);
        let mut keep = Vec::with_capacity(mine.len());
        let mut cands: Vec<(u32, Mine, bool)> = Vec::new();
        // S6 (owner Q4): with `base_bomb_s = 0` the base gives mortars no
        // bombs, so a mortar squad makes no refill trip for them (crates
        // and the HQ feed it, `ai::supply`).
        let no_base_bombs = g
            .setup
            .rules
            .supply
            .as_ref()
            .is_some_and(|s| s.base_bomb_s == 0);
        for m in mine.drain(..) {
            let sq = &g.squads[&m.id];
            if no_base_bombs && m.role == AiKind::Mortar {
                keep.push(m);
                continue;
            }
            let Some(pct) = crate::supply::squad_ammo_pct(&g.setup, self.ents, sq) else {
                keep.push(m);
                continue;
            };
            // An ambush runs to its end (it has its own end rules).
            if self.ai.ambush.get(&m.id).is_some_and(|a| !a.done) {
                keep.push(m);
                continue;
            }
            let on_trip = self.ai.tasks.get(&m.id).map(|t| t.kind) == Some(TaskKind::Refill);
            if on_trip {
                if self.in_own_base(sq) {
                    let arrived = match self.ai.refill.get(&m.id) {
                        Some(&t) if t > 0 => t,
                        _ => {
                            self.ai.refill.insert(m.id, self.tick);
                            self.tick
                        }
                    };
                    let waited = self.tick.saturating_sub(arrived);
                    let stalled = self.refill_paused() && waited >= cfg.rearm_stall_s as u64 * hz;
                    if pct >= cfg.refill_full_pct
                        || waited >= cfg.refill_max_s as u64 * hz
                        || stalled
                    {
                        self.ai.counters.refill_stalls += u32::from(stalled);
                        self.ai.tasks.remove(&m.id);
                        self.ai.refill.remove(&m.id);
                        keep.push(m);
                        continue;
                    }
                } else if !matches!(sq.order, Order::Move { .. } | Order::Retreat) {
                    let spot = self.terrain.clamp(ring_slot(base, m.id.0 % 8 + 1));
                    self.ai.emit(
                        self.due,
                        CommandKind::Move {
                            squad: m.id,
                            target: spot,
                        },
                    );
                }
                self.ai.counters.refill_away += 1;
                continue;
            }
            let threat = self.threat_at(known, m.lead) > 0;
            if pct < cfg.dry_pct || (pct < cfg.return_below_pct && !threat) {
                cands.push((pct, m, threat));
            } else {
                keep.push(m);
            }
        }
        // Staggered, lowest share first.
        cands.sort_by_key(|(p, m, _)| (*p, m.id));
        let mut started: Vec<(u8, u32)> = Vec::new();
        for (pct, m, threat) in cands {
            let point = self.ai.tasks.get(&m.id).map_or(NO_POINT, |t| t.point);
            let tasked = self
                .ai
                .tasks
                .values()
                .filter(|t| t.point == point && point != NO_POINT)
                .count() as u32;
            let allowed = (tasked * cfg.max_returning_pct / 100).max(1);
            let n = match started.iter_mut().find(|(p, _)| *p == point) {
                Some((_, n)) => n,
                None => {
                    started.push((point, 0));
                    &mut started.last_mut().expect("pushed").1
                }
            };
            let dry = pct < cfg.dry_pct;
            let last = point != NO_POINT && tasked <= 1;
            if !dry && (*n >= allowed || last) {
                keep.push(m);
                continue;
            }
            *n += 1;
            let spot = self.terrain.clamp(ring_slot(base, m.id.0 % 8 + 1));
            let cmd = if threat {
                CommandKind::Retreat { squad: m.id }
            } else {
                CommandKind::Move {
                    squad: m.id,
                    target: spot,
                }
            };
            self.ai.emit(self.due, cmd);
            self.task(m.id, TaskKind::Refill, NO_POINT, spot);
            self.ai.refill.insert(m.id, 0);
            self.ai.counters.refill_trips += 1;
            self.ai.counters.refill_dry_trips += u32::from(dry);
            self.ai.counters.refill_away += 1;
        }
        *mine = keep;
    }

    /// S5a: Panzerfaust tubes squad `id`'s men carry.
    fn tubes(&self, id: SquadId) -> u16 {
        let k = ItemKind::SingleShotAt.index();
        self.g.squads[&id]
            .members
            .iter()
            .map(|m| self.ents.kit()[m.index()].items[k] as u16)
            .sum()
    }

    /// S5a: Panzerfaust ambushes (module docs). Returns the squads in one.
    pub(super) fn ambushes(
        &mut self,
        mine: &[Mine],
        assigned: &[(usize, usize)],
        points: &[PointEval],
        kv: &[KnownVehicle],
    ) -> Vec<SquadId> {
        let g = self.g;
        let me = self.me;
        let cfg = g.setup.rules.ai.assault.clone();
        let hz = g.setup.tick_hz.max(1) as u64;
        // Ambushes of squads that are gone, retreating or no longer here.
        let ids: Vec<SquadId> = self.ai.ambush.keys().copied().collect();
        for id in ids {
            let over = g
                .squads
                .get(&id)
                .is_none_or(|sq| sq.owner != me || sq.order == Order::Retreat);
            if over {
                self.end_ambush(id, AMBUSH_RETREAT);
            }
        }
        let mut out = Vec::new();
        if cfg.ambush_watch_m == 0 {
            return out;
        }
        let watch = sq_m(Fx::from_int(cfg.ambush_watch_m as i32));
        // Hysteresis (S5a Fix, review M1): an ambush ends as "gone" only
        // beyond `ambush_release_m` (0: the watch).
        let release = sq_m(Fx::from_int(
            cfg.ambush_release_m.max(cfg.ambush_watch_m).min(1500) as i32,
        ));
        for (j, m) in mine.iter().enumerate() {
            if matches!(m.role, AiKind::AntiTank | AiKind::Engineer) {
                continue;
            }
            let sq = &g.squads[&m.id];
            if sq.pzf.is_none() {
                continue;
            }
            let ty = &g.setup.squad_types[sq.type_idx as usize];
            let Some(w) = ty
                .kit(&g.setup.items, ItemKind::SingleShotAt)
                .and_then(|(_, it)| it.weapon)
                .map(|w| &g.setup.weapons[w as usize])
            else {
                continue;
            };
            let home = assigned
                .iter()
                .find(|a| a.0 == j)
                .map_or(m.lead, |a| points[a.1].pos);
            let within = |v: &KnownVehicle, r: u64| {
                (v.pos - home).length_sq_raw() <= r || (v.pos - m.lead).length_sq_raw() <= r
            };
            let near = |v: &KnownVehicle| within(v, watch);
            let tubes = self.tubes(m.id);
            if let Some(a) = self.ai.ambush.get(&m.id).cloned() {
                // Seen: within watch; unseen: still alive (it may come).
                let seen = kv.iter().find(|v| v.id == a.vehicle);
                let alive = g
                    .vehicles
                    .get(&a.vehicle)
                    .is_some_and(|vs| vs.wreck_tick.is_none());
                let gone = match seen {
                    Some(v) => !within(v, release),
                    None => !alive,
                };
                let lost = a.tubes.saturating_sub(tubes);
                if !a.done {
                    self.ai.counters.ambush_shots += lost as u32;
                }
                let why = if gone {
                    Some(AMBUSH_GONE)
                } else if tubes == 0 {
                    Some(AMBUSH_SPENT)
                } else if !a.done && self.tick >= a.since + cfg.ambush_max_s as u64 * hz {
                    Some(AMBUSH_TIMEOUT)
                } else {
                    None
                };
                match why {
                    Some(AMBUSH_TIMEOUT) => {
                        // Free again, but no new ambush on this vehicle.
                        self.ai.counters.ambush_ends[AMBUSH_TIMEOUT] += 1;
                        self.release_ambush(m.id);
                        if let Some(e) = self.ai.ambush.get_mut(&m.id) {
                            e.done = true;
                            e.tubes = tubes;
                        }
                        continue;
                    }
                    Some(why) => {
                        self.end_ambush(m.id, why);
                        if why != AMBUSH_GONE {
                            continue;
                        }
                    }
                    None => {
                        if let Some(e) = self.ai.ambush.get_mut(&m.id) {
                            e.tubes = tubes;
                        }
                        if a.done {
                            continue;
                        }
                        self.hold_ambush(m, a.spot);
                        out.push(m.id);
                        continue;
                    }
                }
            }
            if tubes == 0 {
                continue;
            }
            // A new ambush on the nearest vehicle in watch.
            let Some(v) = kv
                .iter()
                .filter(|v| near(v))
                .min_by_key(|v| ((v.pos - m.lead).length_sq_raw(), v.id))
            else {
                continue;
            };
            let spot = self.ambush_spot(m, home, v, w, &cfg);
            self.ai.ambush.insert(
                m.id,
                AiAmbush {
                    vehicle: v.id,
                    since: self.tick,
                    spot,
                    tubes,
                    done: false,
                },
            );
            self.ai.counters.ambushes += 1;
            let units: Vec<SimId> = sq
                .members
                .iter()
                .copied()
                .take(crate::unit::MAX_UNITS)
                .collect();
            self.ai.emit(
                self.due,
                CommandKind::SetFire {
                    units: units.clone(),
                    mode: FireMode::Hold,
                },
            );
            let _ = units;
            self.hold_ambush(m, spot);
            out.push(m.id);
        }
        out
    }

    /// Keeps an ambushing squad going to (or holding at) its spot:
    /// crouched on the way, prone there.
    fn hold_ambush(&mut self, m: &Mine, spot: FxVec2) {
        let sq = &self.g.squads[&m.id];
        let near = |a: FxVec2, r: i32| (a - spot).length_sq_raw() <= sq_m(Fx::from_int(r));
        let there = near(m.lead, 8);
        let on_track = match &sq.order {
            Order::Move { target } => near(*target, 5),
            Order::Idle => there,
            _ => false,
        };
        let want = if there {
            StanceOrder::Prone
        } else {
            StanceOrder::Crouch
        };
        let units: Vec<SimId> = sq
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
        if !on_track {
            self.ai.emit(
                self.due,
                CommandKind::Move {
                    squad: m.id,
                    target: spot,
                },
            );
        }
        self.task(m.id, TaskKind::Ambush, NO_POINT, spot);
    }

    /// The spot to wait at (module docs).
    fn ambush_spot(
        &self,
        m: &Mine,
        home: FxVec2,
        v: &KnownVehicle,
        w: &frontline_content::setup::WeaponSetup,
        cfg: &frontline_content::tuning::AiAssaultTuning,
    ) -> FxVec2 {
        let g = self.g;
        let offset = Fx::from_int(cfg.ambush_offset_m as i32);
        let engage: Fx = w.engage_moving.into();
        let moving = v.vel.length() > Fx::from(g.setup.rules.vehicles.moving_speed);
        // A stopped vehicle within the watch is stalked: to the nearest
        // flank spot off its turret's front inside `engage_still`, holding
        // fire (S5a-30 (A)); already in reach, the squad stays.
        if !moving {
            let still = (Fx::from(w.engage_still).min(w.range_max.into())
                - Fx::from_int(cfg.at_hunt_inset_m as i32))
            .max(Fx::from_int(10));
            if (m.lead - v.pos).length_sq_raw() <= sq_m(still) {
                return m.lead;
            }
            return self.flank_spot(v, still, m.lead).unwrap_or(m.lead);
        }
        let _ = home;
        let dir = v.vel.normalize_or_zero();
        let lead = g.squads[&m.id].lead(self.ents);
        let in_trench = self.ents.combat[lead.index()].in_trench;
        let end = v.pos + dir * Fx::from_int(cfg.ambush_watch_m as i32);
        let reach = engage + offset;
        if in_trench || dir == FxVec2::ZERO || seg_dist_sq(m.lead, v.pos, end) <= sq_m(reach) {
            return m.lead;
        }
        let side = dir.perp();
        let eye = FxVec3::new(
            v.pos.x,
            v.pos.y,
            self.terrain.height_at(v.pos) + Fx::from_int(2),
        );
        let half = Fx::HALF;
        // Where the path passes nearest the squad (m ahead of the vehicle).
        let along = (m.lead - v.pos)
            .dot(dir)
            .clamp(Fx::ZERO, Fx::from_int(cfg.ambush_watch_m as i32));
        let walk = sq_m(Fx::from_int(cfg.ambush_watch_m as i32 / 2));
        // (not dug, seen, distance², x, y): lowest wins.
        type Key = (u8, u8, u64, i32, i32);
        let mut best: Option<(Key, FxVec2)> = None;
        let aheads = AHEAD_M.map(Fx::from_int);
        for ahead in aheads.into_iter().chain([along]) {
            for s in [Fx::ONE, -Fx::ONE] {
                let p = self
                    .terrain
                    .clamp(v.pos + dir * ahead + side * (offset * s));
                // A spot no farther than half the watch distance.
                if (p - m.lead).length_sq_raw() > walk {
                    continue;
                }
                let h = self.terrain.height_at(p);
                let dug =
                    self.terrain.has_detail_at(p) && self.terrain.base_height_at(p) - h >= half;
                let seen = self
                    .terrain
                    .line_of_sight(eye, FxVec3::new(p.x, p.y, h + half));
                let key = (
                    u8::from(!dug),
                    u8::from(seen),
                    (p - m.lead).length_sq_raw(),
                    p.x.raw(),
                    p.y.raw(),
                );
                if best.is_none_or(|(b, _)| key < b) {
                    best = Some((key, p));
                }
            }
        }
        best.map_or(m.lead, |(_, p)| p)
    }

    /// Back to discipline and the automatic stance (no count).
    fn release_ambush(&mut self, id: SquadId) {
        let Some(sq) = self.g.squads.get(&id) else {
            return;
        };
        let fire: Vec<SimId> = sq
            .members
            .iter()
            .copied()
            .filter(|u| self.ents.unit[u.index()].fire != FireMode::Discipline)
            .take(crate::unit::MAX_UNITS)
            .collect();
        let stance: Vec<SimId> = sq
            .members
            .iter()
            .copied()
            .filter(|u| self.ents.unit[u.index()].stance != StanceOrder::Auto)
            .take(crate::unit::MAX_UNITS)
            .collect();
        if !fire.is_empty() {
            self.ai.emit(
                self.due,
                CommandKind::SetFire {
                    units: fire,
                    mode: FireMode::Discipline,
                },
            );
        }
        if !stance.is_empty() {
            self.ai.emit(
                self.due,
                CommandKind::SetStance {
                    units: stance,
                    stance: StanceOrder::Auto,
                },
            );
        }
        if self.ai.tasks.get(&id).map(|t| t.kind) == Some(TaskKind::Ambush) {
            self.ai.tasks.remove(&id);
        }
    }

    /// Ends squad `id`'s ambush for `why` (an `AMBUSH_*`): counted (unless
    /// it had already timed out), released, forgotten.
    fn end_ambush(&mut self, id: SquadId, why: usize) {
        let Some(a) = self.ai.ambush.remove(&id) else {
            return;
        };
        if !a.done {
            self.ai.counters.ambush_ends[why] += 1;
            self.release_ambush(id);
        }
    }
}
