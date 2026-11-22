//! S7 AI habits for assault guns and tank destroyers
//! (`docs/slices/S7-casemate-td.md` 6; S13-ai H2: and tanks whose def has
//! `ai_overwatch`, with `[ai.armour] heavy_overwatch` 1): overwatch instead of leading
//! assaults, open-topped vehicles backing off from infantry, and infantry
//! throwing into stopped open-topped enemy vehicles. Every distance is
//! rules data (`[ai.armour]`, `[ai.infantry]`).
//!
//! * **Anchor.** `overwatch_back_m` behind the most threatened own or
//!   contested point (seen enemy vehicles, then threat, then priority; with
//!   none, the point the vehicles go for), on the line toward the threat:
//!   the known enemy vehicle nearest that point, else the enemy side (the
//!   line from the own base through the point). A known enemy vehicle
//!   within `td_hunt_m` of the vehicle replaces it: the anchor is where the
//!   vehicle stands and the threat is that vehicle.
//! * **Spot.** The best of the anchor and `overwatch_candidates` points on a
//!   ring of `overwatch_ring_m` around it by terrain lines to the threat:
//!   hull-down (the gun-height line clear, the hull-centre line blocked) 2,
//!   clear 1, else 0; points within `td_infantry_standoff_m` of known enemy
//!   infantry are passed over; ties go to the nearest, then the lower
//!   index; with every point passed over, a point `td_infantry_standoff_m`
//!   behind the anchor (away from the threat). It drives there (`Move`),
//!   then, while its gun has no target, faces the threat (`FaceUnits`) and
//!   holds; the gun fires through the usual targeting (a casemate turns its
//!   hull with `lay_turn`).
//! * **Relocate.** After `relocate_after_shots` main-gun shots from the
//!   spot, or a penetration since it was chosen, it takes the next best spot
//!   at least `relocate_m` from the old one. An anchor that moved more than
//!   `relocate_m` picks a new spot too.
//! * **Open top.** An open-topped vehicle with known enemy infantry within
//!   `open_top_infantry_m` reverses away (`ReverseUnits`, 40 m).
//! * **Throws.** A squad with frags (else satchels) within
//!   `open_top_throw_m` of a known enemy open-topped vehicle no faster than
//!   `open_top_slow_mm_s` throws at its open box's centre (`Throw`; the lob
//!   decides, no success roll).

use frontline_content::{ItemKind, VehicleRole};

use super::{AiOverwatch, Known, KnownVehicle, MyTank, PointEval, Rel, TaskKind, ThrowPick, sq_m};
use crate::command::CommandKind;
use crate::ecs::SimId;
use crate::fx::{Fx, FxVec2};
use crate::game::Order;

/// An open-topped vehicle backs off this far from the infantry (m).
const BACKOFF_M: i32 = 40;
/// The vehicle is at its spot within this (m).
const AT_SPOT_M: i32 = 6;
/// A move order within this of the spot needs no new order (m).
const ON_TRACK_M: i32 = 5;
/// A vehicle idle short of its spot this long after the spot was chosen
/// takes where it stands as the spot (s): the spot was on ground it cannot
/// reach, and re-sending it there only re-runs a failed path search.
const SPOT_GIVE_UP_S: u64 = 20;
/// A recon car orders no second back-off within this (s): one it cannot
/// drive ends idle at once, and re-sending it only re-runs a path search.
const BACKOFF_GAP_S: u64 = 10;
/// A recon car's spot is clear this far outside `at_threat_m` (m), more
/// than `AT_SPOT_M`: stopped at it, the car is not in reach of the threat.
const CLEAR_SLACK_M: i32 = 12;
/// A spot in a threat's reach moves toward the own base in steps of this
/// many metres, at most `CLEAR_STEPS` times.
const CLEAR_STEP_M: i32 = 20;
const CLEAR_STEPS: i32 = 16;
/// The threat on the enemy side of a point with no known vehicle (m).
const ENEMY_SIDE_M: i32 = 150;
/// Sight-line heights above the ground at the threat (mm).
const THREAT_EYE_MM: i32 = 1500;

impl super::Planner<'_> {
    /// S7: overwatch, back-off and facing for an assault gun or tank
    /// destroyer (module docs). False for a tank (or with
    /// `overwatch_back_m` 0): the caller gives it a tank's tasks.
    pub(super) fn overwatch(
        &mut self,
        t: &MyTank,
        points: &[PointEval],
        known: &[Known],
        kv: &[KnownVehicle],
        target_point: Option<usize>,
    ) -> bool {
        let g = self.g;
        let r = &g.setup.rules.ai.armour;
        let Some(vs) = g.vehicles.get(&t.id) else {
            return false;
        };
        let v = &g.setup.vehicles[vs.type_idx as usize];
        // S13-ai H2: a tank def with `ai_overwatch` (Panther, Tiger) holds
        // overwatch too with `heavy_overwatch` 1.
        let heavy = v.ai_overwatch && r.heavy_overwatch != 0;
        if (v.role == VehicleRole::Tank && !heavy) || r.overwatch_back_m == 0 {
            return false;
        }
        let Some(sq) = g.squads.get(&t.squad) else {
            return false;
        };
        let open = v.boxes.iter().any(|b| b.open != 0);
        // S8b: a recon car backs off from AT teams and armour first.
        let recon = v.role == VehicleRole::Recon;
        if recon && !t.immobile && self.recon_backoff(t, sq, known, kv) {
            return true;
        }
        // Open top: back away from infantry.
        if open && !t.immobile && r.open_top_infantry_m > 0 {
            let near = sq_m(Fx::from_int(r.open_top_infantry_m as i32));
            if let Some(k) = known
                .iter()
                .filter(|k| (k.pos - t.pos).length_sq_raw() <= near)
                .min_by_key(|k| ((k.pos - t.pos).length_sq_raw(), k.id))
            {
                let away = (t.pos - k.pos).normalize_or_zero();
                let target = self.terrain.clamp(t.pos + away * Fx::from_int(BACKOFF_M));
                let going = matches!(&sq.order, Order::Move { target: tg }
                    if (*tg - target).length_sq_raw() <= sq_m(Fx::from_int(15)));
                if !going {
                    self.ai.emit(
                        self.due,
                        CommandKind::ReverseUnits {
                            units: vec![t.id],
                            target,
                        },
                    );
                    self.ai.counters.open_top_backoffs += 1;
                }
                self.task(t.squad, TaskKind::Retreat, super::NO_POINT, target);
                return true;
            }
        }
        // The anchor and the threat.
        let hunt = kv
            .iter()
            .filter(|k| (k.pos - t.pos).length_sq_raw() <= sq_m(Fx::from_int(r.td_hunt_m as i32)))
            .min_by_key(|k| ((k.pos - t.pos).length_sq_raw(), k.id));
        let (anchor, axis, pi) = match hunt {
            // S8b: `view_back_m` short of the point the infantry goes for.
            _ if recon => {
                let i = target_point.or_else(|| {
                    (0..points.len()).max_by_key(|&i| (points[i].priority, -(i as i64)))
                });
                let Some(i) = i else {
                    return false;
                };
                let p = points[i].pos;
                let out = (p - g.base_of(self.me)).normalize_or_zero();
                let view = Fx::from_int(g.setup.rules.ai.recon.view_back_m.min(30_000) as i32);
                let anchor = self.terrain.clamp(p - out * view);
                (
                    anchor,
                    self.terrain.clamp(p + out * Fx::from_int(ENEMY_SIDE_M)),
                    i as u8,
                )
            }
            Some(k) => (t.pos, k.pos, super::NO_POINT),
            None => {
                let held = (0..points.len())
                    .filter(|&i| points[i].rel != Rel::Enemy)
                    .max_by_key(|&i| {
                        (
                            points[i].armour,
                            points[i].threat,
                            points[i].priority,
                            -(i as i64),
                        )
                    });
                let Some(i) = held.or_else(|| (!points.is_empty()).then_some(0)) else {
                    return false;
                };
                let p = points[i].pos;
                let threat = kv
                    .iter()
                    .min_by_key(|k| ((k.pos - p).length_sq_raw(), k.id))
                    .map(|k| k.pos)
                    .unwrap_or_else(|| {
                        let out = (p - g.base_of(self.me)).normalize_or_zero();
                        self.terrain.clamp(p + out * Fx::from_int(ENEMY_SIDE_M))
                    });
                let dir = (threat - p).normalize_or_zero();
                let anchor = self
                    .terrain
                    .clamp(p - dir * Fx::from_int(r.overwatch_back_m as i32));
                (anchor, threat, i as u8)
            }
        };
        let rounds: u16 = vs
            .ammo
            .iter()
            .map(|&n| n as u32)
            .sum::<u32>()
            .saturating_add(u32::from(vs.loaded.is_some()))
            .min(u16::MAX as u32) as u16;
        let relocate_m = Fx::from_int(r.relocate_m as i32);
        let old = self.ai.overwatch.get(&t.squad).copied();
        let fired = old.is_some_and(|o| {
            o.held
                && (o.rounds.saturating_sub(rounds) as u32 >= r.relocate_after_shots
                    || vs.last_pen.is_some_and(|p| p > o.since))
        });
        let moved = old.is_some_and(|o| (o.anchor - anchor).length() > relocate_m);
        let mut entry = match old {
            Some(o) if !fired && !moved => AiOverwatch { axis, ..o },
            _ => {
                if old.is_none() && self.ai.overwatch.len() >= super::MAX_OVERWATCH {
                    return false;
                }
                let avoid = if fired { old.map(|o| o.spot) } else { None };
                let spot = self.overwatch_spot(t, anchor, axis, known, avoid);
                if fired {
                    self.ai.counters.relocations += 1;
                }
                AiOverwatch {
                    anchor,
                    spot,
                    axis,
                    rounds,
                    since: self.tick,
                    held: false,
                    backed_at: None,
                }
            }
        };
        let hz = g.setup.tick_hz.max(1) as u64;
        // S8b: a recon car does not take a spot in reach of a known threat.
        if recon {
            entry.spot = self.recon_clear_spot(entry.spot, known, kv);
        }
        if sq.order == Order::Idle
            && !t.immobile
            && self.tick >= entry.since + SPOT_GIVE_UP_S * hz
            && (t.pos - entry.spot).length_sq_raw() > sq_m(Fx::from_int(AT_SPOT_M))
        {
            entry.spot = t.pos;
            if recon {
                self.ai.counters.recon_fails += 1;
            }
        }
        let at_spot = (t.pos - entry.spot).length_sq_raw() <= sq_m(Fx::from_int(AT_SPOT_M));
        if !at_spot && !t.immobile {
            let on_track = matches!(&sq.order, Order::Move { target } | Order::AttackMove { target }
                if (*target - entry.spot).length_sq_raw() <= sq_m(Fx::from_int(ON_TRACK_M)));
            if !on_track {
                self.ai.emit(
                    self.due,
                    CommandKind::Move {
                        squad: t.squad,
                        target: entry.spot,
                    },
                );
            }
        } else {
            if !entry.held {
                entry.held = true;
                entry.rounds = rounds;
                self.ai.counters.overwatch_holds += 1;
                if recon {
                    self.ai.counters.recon_stops += 1;
                }
            }
            // Face the threat while idle there and not engaged: a face would
            // clear `lay_turn` and swing a laid gun off its target.
            if sq.order == Order::Idle && vs.lay_turn.is_none() && vs.target.is_none() {
                let d = entry.axis - t.pos;
                let off = (Fx::atan2(d.y, d.x) - t.yaw).wrap_angle().abs();
                let slack = Fx::from_int(r.face_slack_deg as i32)
                    .mul(Fx::PI)
                    .div_int(180);
                let same = vs
                    .face
                    .is_some_and(|f| (f - entry.axis).length_sq_raw() <= sq_m(Fx::from_int(20)));
                if off > slack && !same {
                    self.ai.emit(
                        self.due,
                        CommandKind::FaceUnits {
                            units: vec![t.id],
                            toward: entry.axis,
                        },
                    );
                    self.ai.counters.faces += 1;
                }
            }
        }
        self.ai.overwatch.insert(t.squad, entry);
        self.task(t.squad, TaskKind::Defend, pi, entry.spot);
        true
    }

    /// S8b: the known AT teams and armoured enemies a recon car keeps away
    /// from: (position, id).
    fn recon_threats(&self, known: &[Known], kv: &[KnownVehicle]) -> Vec<(FxVec2, SimId)> {
        let g = self.g;
        known
            .iter()
            .filter(|k| k.at)
            .map(|k| (k.pos, k.id))
            .chain(
                kv.iter()
                    .filter(|k| {
                        matches!(
                            g.setup.vehicles[k.type_idx as usize].role,
                            VehicleRole::Tank
                                | VehicleRole::AssaultGun
                                | VehicleRole::TankDestroyer
                        )
                    })
                    .map(|k| (k.pos, k.id)),
            )
            .collect()
    }

    /// S8b: `spot`, or the first point toward the own base that no known
    /// threat reaches (`at_threat_m` and `CLEAR_SLACK_M`).
    fn recon_clear_spot(&self, spot: FxVec2, known: &[Known], kv: &[KnownVehicle]) -> FxVec2 {
        let g = self.g;
        let r = &g.setup.rules.ai.recon;
        if r.at_threat_m == 0 {
            return spot;
        }
        let reach = sq_m(Fx::from_int(
            r.at_threat_m.min(30_000) as i32 + CLEAR_SLACK_M,
        ));
        let threats = self.recon_threats(known, kv);
        let clear = |p: FxVec2| {
            threats
                .iter()
                .all(|(e, _)| (*e - p).length_sq_raw() > reach)
        };
        if clear(spot) {
            return spot;
        }
        let back = (g.base_of(self.me) - spot).normalize_or_zero();
        let at = |k: i32| {
            self.terrain
                .clamp(spot + back * Fx::from_int(k * CLEAR_STEP_M))
        };
        (1..=CLEAR_STEPS)
            .map(at)
            .find(|&p| clear(p))
            .unwrap_or_else(|| at(CLEAR_STEPS))
    }

    /// S8b: a recon car with a known AT team or armoured enemy within
    /// `at_threat_m` reverses away from it (`ReverseUnits`) whatever it was
    /// doing; the place it backs to is its spot from then on. False with
    /// no such threat, or with no room to remember the back-off.
    fn recon_backoff(
        &mut self,
        t: &MyTank,
        sq: &crate::game::Squad,
        known: &[Known],
        kv: &[KnownVehicle],
    ) -> bool {
        let g = self.g;
        let r = &g.setup.rules.ai.recon;
        if r.at_threat_m == 0 {
            return false;
        }
        let near = sq_m(Fx::from_int(r.at_threat_m as i32));
        let team = self
            .recon_threats(known, kv)
            .into_iter()
            .filter(|(p, _)| (*p - t.pos).length_sq_raw() <= near)
            .min_by_key(|(p, id)| ((*p - t.pos).length_sq_raw(), *id));
        let Some((p, _)) = team else {
            return false;
        };
        let away = (t.pos - p).normalize_or_zero();
        let target = self
            .terrain
            .clamp(t.pos + away * Fx::from_int(r.backoff_m.min(30_000) as i32));
        let hz = g.setup.tick_hz.max(1) as u64;
        let tick = self.tick;
        let old = self.ai.overwatch.get(&t.squad).copied();
        if old.is_none() && self.ai.overwatch.len() >= super::MAX_OVERWATCH {
            return false;
        }
        // A move already taking it away from the threat is left alone (a
        // short spot behind the car is a reverse too, but not away).
        let here = (t.pos - p).length_sq_raw();
        let backing =
            matches!(&sq.order, Order::Move { target: tg } if (*tg - p).length_sq_raw() > here);
        let recent = old
            .and_then(|o| o.backed_at)
            .is_some_and(|b| tick < b + BACKOFF_GAP_S * hz);
        if !backing && !recent {
            // Backing up only if the threat is ahead: a car that faces away
            // drives off, not backward into it.
            let (sn, cs) = t.yaw.sin_cos();
            let ahead = FxVec2::new(cs, sn);
            let kind = if ahead.x.mul(away.x) + ahead.y.mul(away.y) > Fx::ZERO {
                CommandKind::Move {
                    squad: t.squad,
                    target,
                }
            } else {
                CommandKind::ReverseUnits {
                    units: vec![t.id],
                    target,
                }
            };
            self.ai.emit(self.due, kind);
            self.ai.counters.recon_backoffs += 1;
            let mut entry = old.unwrap_or(AiOverwatch {
                anchor: t.pos,
                spot: target,
                axis: p,
                rounds: 0,
                since: tick,
                held: false,
                backed_at: None,
            });
            entry.spot = target;
            entry.held = false;
            entry.backed_at = Some(tick);
            self.ai.overwatch.insert(t.squad, entry);
        }
        self.task(t.squad, TaskKind::Retreat, super::NO_POINT, target);
        true
    }

    /// The best overwatch spot around `anchor` toward `axis` (module docs);
    /// `avoid`: the spot it leaves (the new one is `relocate_m` from it).
    fn overwatch_spot(
        &self,
        t: &MyTank,
        anchor: FxVec2,
        axis: FxVec2,
        known: &[Known],
        avoid: Option<FxVec2>,
    ) -> FxVec2 {
        let g = self.g;
        let r = &g.setup.rules.ai.armour;
        // S13 (review V3b): the hull line at the hull body's centre (box 0
        // and an S13 superstructure; box 0 alone for every pre-S13 def).
        let body_dz = g.vehicles.get(&t.id).map_or(Fx::ZERO, |vs| {
            let v = &g.setup.vehicles[vs.type_idx as usize];
            Fx::from(v.hull_body().center.z) - Fx::from(v.hull.center.z)
        });
        let (gun_h, hull_h) = match g.vderived.frame(t.id) {
            Some(f) => (
                f.pivot.z - f.origin.z,
                f.hull.center.z - f.origin.z + body_dz,
            ),
            None => (Fx::from_int(2), Fx::ONE),
        };
        let threat = axis.extend(self.terrain.height_at(axis) + Fx::from_milli(THREAT_EYE_MM));
        let standoff = sq_m(Fx::from_int(r.td_infantry_standoff_m as i32));
        let relocate = sq_m(Fx::from_int(r.relocate_m as i32));
        let n = r.overwatch_candidates.max(1) as i32;
        let ring = Fx::from_int(r.overwatch_ring_m as i32);
        let mut best: Option<((i32, i64, i32), FxVec2)> = None;
        for k in 0..=n {
            let c = if k == 0 {
                anchor
            } else {
                let (s, co) = Fx::TAU.mul_int(k - 1).div_int(n).sin_cos();
                self.terrain.clamp(anchor + FxVec2::new(co, s) * ring)
            };
            if known
                .iter()
                .any(|m| (m.pos - c).length_sq_raw() <= standoff)
                || avoid.is_some_and(|a| (a - c).length_sq_raw() < relocate)
            {
                continue;
            }
            let ground = self.terrain.height_at(c);
            let gun = c.extend(ground + gun_h);
            let hull = c.extend(ground + hull_h);
            let score = match (
                self.terrain.line_of_sight(gun, threat),
                self.terrain.line_of_sight(hull, threat),
            ) {
                (true, false) => 2,
                (true, true) => 1,
                _ => 0,
            };
            let near = -((c - t.pos).length_sq_raw().min(i64::MAX as u64) as i64);
            let key = (score, near, -k);
            if best.as_ref().is_none_or(|(b, _)| key > *b) {
                best = Some((key, c));
            }
        }
        // Every candidate passed over: not the rejected anchor but a point
        // the stand-off distance further back from the threat.
        best.map_or_else(
            || {
                let back = (anchor - axis).normalize_or_zero();
                self.terrain
                    .clamp(anchor + back * Fx::from_int(r.td_infantry_standoff_m as i32))
            },
            |(_, c)| c,
        )
    }

    /// S7: a squad's throw into a slow enemy open-topped vehicle in reach
    /// (module docs): frags first, else a satchel.
    pub(super) fn open_top_throw(
        &self,
        sq: &crate::game::Squad,
        kv: &[KnownVehicle],
    ) -> Option<ThrowPick> {
        let g = self.g;
        let ri = &g.setup.rules.ai.infantry;
        if ri.open_top_throw_m == 0 || kv.is_empty() || sq.members.is_empty() {
            return None;
        }
        let lead = self.ents.transform[sq.lead(self.ents).index()].pos.xy();
        let reach = sq_m(Fx::from_int(ri.open_top_throw_m as i32));
        let slow = Fx::from_milli(ri.open_top_slow_mm_s as i32);
        kv.iter().find_map(|k| {
            if k.speed.abs() > slow || (k.pos - lead).length_sq_raw() > reach {
                return None;
            }
            let v = &g.setup.vehicles[k.type_idx as usize];
            // S8b (6.5): a screened face turns the item away.
            let b = v.boxes.iter().position(|b| b.open & !b.screen != 0)?;
            let f = g.vderived.frame(k.id)?;
            let target = self.terrain.clamp(f.boxes.get(b)?.center.xy());
            [ItemKind::Frag, ItemKind::Satchel]
                .into_iter()
                .find_map(|item| {
                    let men = self.throwers(sq, item, target);
                    (!men.is_empty()).then_some((item, men, target))
                })
        })
    }
}
