//! S3c AI armour habits (`docs/slices/S3c-armour.md` Part B, B1 and B5):
//! rearm trips, crews re-crewing or walking back, and denial fire at enemy
//! abandoned vehicles. Every distance and time is rules data
//! (`[ai.armour]`); see the module docs of [`crate::ai`], step 9.

use frontline_content::ComponentKind;
use frontline_content::vehicle::VehicleSetup;

use super::{Known, KnownVehicle, MyTank, NO_POINT, Planner, TaskKind, sq_m};
use crate::command::CommandKind;
use crate::ecs::{EntityKind, SimId};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{Order, SquadId};
use crate::gunnery::{AP, HE};
use crate::vehicle::VehicleState;

/// A crew squad within this of its own abandoned vehicle waits there while
/// men are still getting out (m).
const CREW_WAIT_M: i32 = 30;

impl Planner<'_> {
    /// B1: whether a vehicle should drive back to rearm: every limited MG
    /// dry, HE at or below `combat.suppress.tank_he_reserve`, or the
    /// penetrator at or below `ai.armour.rearm_ap_below`.
    pub(super) fn needs_rearm(&self, vs: &VehicleState, v: &VehicleSetup) -> bool {
        let r = &self.g.setup.rules;
        let limited: Vec<bool> = v
            .mounts
            .iter()
            .skip(1)
            .zip(&vs.mgs)
            .filter_map(|(m, st)| m.rounds.map(|n| st.spent >= n))
            .collect();
        let mg_dry = !limited.is_empty() && limited.iter().all(|&d| d);
        let left = |k: u8| {
            vs.ammo
                .get(k as usize)
                .map(|&n| n as u32 + u32::from(vs.loaded == Some(k)))
        };
        mg_dry
            || left(HE).is_some_and(|n| n <= r.combat.suppress.tank_he_reserve)
            || left(AP).is_some_and(|n| n <= r.ai.armour.rearm_ap_below)
    }

    /// B1: main-gun rounds and MG stowage at least `ai.armour.rearm_full_pct`
    /// full.
    pub(super) fn rearmed(&self, vs: &VehicleState, v: &VehicleSetup) -> bool {
        let pct = self.g.setup.rules.ai.armour.rearm_full_pct;
        let gun = v.main_gun().ammo.iter().enumerate().all(|(k, a)| {
            let have =
                vs.ammo.get(k).copied().unwrap_or(0) as u32 + u32::from(vs.loaded == Some(k as u8));
            have * 100 >= a.count as u32 * pct
        });
        let mgs = v.mounts.iter().skip(1).zip(&vs.mgs).all(|(m, st)| {
            m.rounds
                .is_none_or(|n| n.saturating_sub(st.spent) * 100 >= n * pct)
        });
        gun && mgs
    }

    /// B1, in `vehicle_upkeep`: a vehicle on a rearm trip keeps driving
    /// back or waits in base until rearmed (or `rearm_max_s`); one that
    /// needs it with no known enemy vehicle within `threat_m` starts a
    /// trip. Returns whether the vehicle was handled (it then takes no
    /// other task this plan).
    pub(super) fn rearm(&mut self, t: &MyTank, kv: &[KnownVehicle]) -> bool {
        let g = self.g;
        let s = &g.setup;
        let vs = &g.vehicles[&t.id];
        let v = &s.vehicles[vs.type_idx as usize];
        let base = &s.map.bases[s.players[self.me.0 as usize].base as usize];
        let base_at: FxVec2 = base.center.into();
        let in_base = (t.pos - base_at).length_sq_raw() <= sq_m(base.radius.into());
        let hz = s.tick_hz.max(1);
        let on_trip = self.ai.tasks.get(&t.squad).map(|k| k.kind) == Some(TaskKind::Rearm);
        if on_trip {
            if !in_base {
                if t.immobile {
                    // Stuck on the way: the field repair rule goes first.
                    self.ai.tasks.remove(&t.squad);
                    return false;
                }
                // The retreat ended short (blocked) or was replaced.
                self.ai
                    .emit(self.due, CommandKind::Retreat { squad: t.squad });
                return true;
            }
            if !self.rearmed(vs, v) && vs.base_ticks < s.rules.ai.armour.rearm_max_s * hz {
                return true;
            }
            self.ai.tasks.remove(&t.squad);
            return false;
        }
        if t.immobile || in_base || !self.needs_rearm(vs, v) {
            return false;
        }
        let threat = sq_m(Fx::from_int(s.rules.ai.armour.threat_m as i32));
        if kv.iter().any(|k| (k.pos - t.pos).length_sq_raw() <= threat) {
            return false;
        }
        if let Some(e) = self.ai.escorts.remove(&t.squad) {
            self.release(&e, t.id, &[]);
        }
        self.ai
            .emit(self.due, CommandKind::Retreat { squad: t.squad });
        self.task(t.squad, TaskKind::Rearm, NO_POINT, base_at);
        self.ai.counters.rearms += 1;
        true
    }

    /// B5: whether the AI re-crews abandoned vehicle `id` with crew squad
    /// `crew` now: not burning, not useless (gun out and broken down), no
    /// known enemy soldier within `recrew_clear_m` and no known enemy
    /// vehicle within `threat_m` of it, and the re-crew check passes
    /// (men, vehicle limit, pop).
    fn recrewable(&self, crew: SquadId, id: SimId, known: &[Known], kv: &[KnownVehicle]) -> bool {
        let g = self.g;
        let Some(vs) = g.vehicles.get(&id) else {
            return false;
        };
        let a = &g.setup.rules.ai.armour;
        let broken = [
            ComponentKind::Engine,
            ComponentKind::TrackLeft,
            ComponentKind::TrackRight,
            ComponentKind::Transmission,
        ]
        .iter()
        .any(|&k| vs.component(k) == 0);
        if vs.burn > 0 || vs.ammo_fire > 0 || (vs.component(ComponentKind::Gun) == 0 && broken) {
            return false;
        }
        let at = self.ents.transform[id.index()].pos.xy();
        let clear = sq_m(Fx::from_int(a.recrew_clear_m as i32));
        let threat = sq_m(Fx::from_int(a.threat_m as i32));
        !known.iter().any(|k| (k.pos - at).length_sq_raw() <= clear)
            && !kv.iter().any(|k| (k.pos - at).length_sq_raw() <= threat)
            && g.recrew_check(self.ents, self.me, crew, id).is_ok()
    }

    /// B5: the AI's crew squads (ascending id). Each re-crews its own
    /// abandoned vehicle when [`Self::recrewable`], else the nearest other
    /// abandoned own vehicle within `recrew_reach_m` that is; waits beside
    /// its vehicle while men are still getting out; otherwise walks back
    /// to base (where it is dismissed). S4 (S3c-33 (A)): with no own
    /// vehicle to man, a crew captures the nearest enemy abandoned vehicle
    /// within `ai.armour.capture_m` that is re-crewable, while the rules
    /// allow captures (`recrew_check`).
    pub(super) fn crews(&mut self, crews: &[SquadId], known: &[Known], kv: &[KnownVehicle]) {
        let g = self.g;
        let reach = sq_m(Fx::from_int(g.setup.rules.ai.armour.recrew_reach_m as i32));
        let capture_m = g.setup.rules.ai.armour.capture_m;
        let capture = sq_m(Fx::from_int(capture_m as i32));
        let enemy: Vec<SimId> = if capture_m == 0 || g.setup.rules.vehicles.recrew.capture == 0 {
            Vec::new()
        } else {
            g.vehicles
                .keys()
                .copied()
                .filter(|v| {
                    self.ents.kind[v.index()] == EntityKind::Abandoned
                        && self.ents.owner[v.index()] != self.me
                })
                .collect()
        };
        let wait = sq_m(Fx::from_int(CREW_WAIT_M));
        let own: Vec<SimId> = g
            .vehicles
            .keys()
            .copied()
            .filter(|v| {
                self.ents.kind[v.index()] == EntityKind::Abandoned
                    && self.ents.owner[v.index()] == self.me
            })
            .collect();
        let mut taken: Vec<SimId> = Vec::new();
        for &id in crews {
            let sq = &g.squads[&id];
            if sq.members.is_empty() {
                continue;
            }
            let lead = self.ents.transform[sq.lead(self.ents).index()].pos.xy();
            let near = |v: &SimId| {
                (self.ents.transform[v.index()].pos.xy() - lead).length_sq_raw() <= reach
            };
            let mine = own
                .iter()
                .copied()
                .find(|v| g.vehicles[v].crew_squad == Some(id));
            let pick = mine
                .filter(|v| near(v) && !taken.contains(v) && self.recrewable(id, *v, known, kv))
                .or_else(|| {
                    own.iter()
                        .copied()
                        .filter(|v| {
                            Some(*v) != mine
                                && near(v)
                                && !taken.contains(v)
                                && self.recrewable(id, *v, known, kv)
                        })
                        .min_by_key(|v| {
                            (
                                (self.ents.transform[v.index()].pos.xy() - lead).length_sq_raw(),
                                *v,
                            )
                        })
                })
                .or_else(|| {
                    // S4 capture.
                    enemy
                        .iter()
                        .copied()
                        .filter(|v| {
                            (self.ents.transform[v.index()].pos.xy() - lead).length_sq_raw()
                                <= capture
                                && !taken.contains(v)
                                && self.recrewable(id, *v, known, kv)
                        })
                        .min_by_key(|v| {
                            (
                                (self.ents.transform[v.index()].pos.xy() - lead).length_sq_raw(),
                                *v,
                            )
                        })
                });
            if let Some(v) = pick {
                taken.push(v);
                if sq.order != (Order::Recrew { vehicle: v }) {
                    self.ai.emit(
                        self.due,
                        CommandKind::Recrew {
                            squad: id,
                            vehicle: v,
                        },
                    );
                    if self.ents.owner[v.index()] == self.me {
                        self.ai.counters.recrews += 1;
                    } else {
                        self.ai.counters.capture_orders += 1;
                    }
                }
                let at = self.ents.transform[v.index()].pos.xy();
                self.task(id, TaskKind::Recrew, NO_POINT, at);
                continue;
            }
            // Men still getting out of its own (re-crewable) vehicle: wait.
            let exiting = mine.is_some_and(|v| {
                let vs = &g.vehicles[&v];
                vs.exit.iter().any(|&e| e > 0)
                    && vs.burn == 0
                    && vs.ammo_fire == 0
                    && (self.ents.transform[v.index()].pos.xy() - lead).length_sq_raw() <= wait
            });
            if exiting {
                continue;
            }
            if sq.order != Order::Retreat && !self.in_own_base(sq) {
                self.ai.emit(self.due, CommandKind::Retreat { squad: id });
                self.ai.counters.crew_retreats += 1;
                let base = g.base_of(self.me);
                self.task(id, TaskKind::Retreat, NO_POINT, base);
            }
        }
    }

    /// B5 denial (S3c-9 (A)): a holding vehicle with no live target and no
    /// known live enemy vehicle within `face_range_m` fires at the nearest
    /// enemy abandoned vehicle within `deny_m` it has a line of sight to
    /// (`AttackUnit`), while `vehicles.recrew.capture` is on; once a live
    /// enemy vehicle is near again it stops. Returns whether it gave the
    /// vehicle an order (or kept its denial).
    pub(super) fn deny(&mut self, t: &MyTank, kv: &[KnownVehicle]) -> bool {
        let g = self.g;
        let s = &g.setup;
        let a = &s.rules.ai.armour;
        let vs = &g.vehicles[&t.id];
        let denying = vs
            .attack
            .is_some_and(|x| self.ents.kind[x.index()] == EntityKind::Abandoned);
        let range = sq_m(Fx::from_int(a.face_range_m as i32));
        let live_near = kv.iter().any(|k| (k.pos - t.pos).length_sq_raw() <= range);
        if denying && (live_near || vs.target.is_some_and(|x| Some(x) != vs.attack)) {
            self.ai.emit(self.due, CommandKind::Stop { squad: t.squad });
            return true;
        }
        if denying {
            return true;
        }
        if a.deny_m == 0
            || s.rules.vehicles.recrew.capture == 0
            || live_near
            || vs.target.is_some()
            || vs.suppress.is_some()
            || g.squads[&t.squad].order != Order::Idle
        {
            return false;
        }
        let r = sq_m(Fx::from_int(a.deny_m as i32));
        let eye = self.ents.transform[t.id.index()].pos
            + FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::from_int(2));
        let target = g
            .vehicles
            .keys()
            .copied()
            .filter(|v| {
                let i = v.index();
                self.ents.kind[i] == EntityKind::Abandoned
                    && self.ents.owner[i] != self.me
                    && (self.ents.transform[i].pos.xy() - t.pos).length_sq_raw() <= r
                    && self.terrain.line_of_sight(
                        eye,
                        self.ents.transform[i].pos + FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE),
                    )
            })
            .min_by_key(|v| {
                (
                    (self.ents.transform[v.index()].pos.xy() - t.pos).length_sq_raw(),
                    *v,
                )
            });
        let Some(target) = target else {
            return false;
        };
        self.ai.emit(
            self.due,
            CommandKind::AttackUnit {
                units: vec![t.id],
                target,
            },
        );
        self.ai.counters.denials += 1;
        true
    }
}
