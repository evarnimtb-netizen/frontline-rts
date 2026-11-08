//! S9 AI habits for AT guns (`docs/slices/S9-at-guns.md` 12; `[ai.guns]`).
//! One saved job per own gun ([`AiGun`], keyed by the gun's squad):
//!
//! * **Site.** The most threatened own or contested point; the approach is
//!   the line from it toward the nearest known enemy vehicle, else the
//!   enemy base. Candidates lie `site_min_m`..`site_max_m` behind the point
//!   within `site_spread_deg` of that line, away from known enemy men. The
//!   score is the sight lines from gun height to `los_rays` points along
//!   the approach, then cover (woods, or hull-down: the gun-height line
//!   clear and the low line blocked), then the nearest.
//! * **Go.** The tow drives to the site with the crew riding. No unhitching
//!   with an enemy within `contact_m`: the site moves back toward the base.
//! * **Unhitch, Emplace** facing the approach. A tractor goes home once; a
//!   half-track goes back to the transport habits.
//! * **Hold.** `SetAmbush` to `engage_still_m`; the usual targeting fires.
//!   An armoured target in reach outside the arc re-lays the trail.
//! * **Pit.** An idle engineer squad within `pit_engineer_m` digs a pit.
//! * **Recrew.** Idle infantry within `recrew_m` serve a neutral gun with
//!   rounds left, or an own gun whose crew is gone.
//!
//! **No loops** (S7 tick fix): a move that ends idle short of its goal is a
//! failed move and after `max_fails` the tow unhitches where it stands; a
//! refused `Emplace` is repeated at most that often; no order is sent twice
//! within `repath_s`.

use frontline_content::VehicleRole;
use frontline_content::setup::DigProfile;
use serde::{Deserialize, Serialize};

use super::{AiKind, Known, KnownVehicle, Mine, MyTank, PointEval, Rel, sq_m};
use crate::armour::deg;
use crate::command::CommandKind;
use crate::ecs::{EntityKind, SimId};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{Order, SquadId};
use crate::vehicle::GunState;

/// Most gun jobs one AI keeps.
pub const MAX_GUNS: usize = 8;
/// Most guns one AI remembers trying to recrew.
pub const MAX_RECREW_TRIES: usize = 16;
/// Eye heights of the site's sight lines (mm): the enemy's, the gun's, and
/// the low line that decides hull-down.
const ENEMY_EYE_MM: i32 = 1500;
const GUN_EYE_MM: i32 = 1200;
const LOW_EYE_MM: i32 = 450;
/// A site given up for contact moves this far toward the base (m).
const BACK_OFF_M: i32 = 60;
/// A neutral gun with an enemy this close is left alone (m).
const RECREW_CLEAR_M: i32 = 40;
/// Half the length of a gun pit's line (m).
const PIT_HALF_M: i32 = 2;

/// A gun job's phase.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum GunPhase {
    #[default]
    Site = 0,
    Go = 1,
    Unhitch = 2,
    Emplace = 3,
    Hold = 4,
    /// Given up (`max_fails`): no more orders.
    Stop = 5,
}

/// One own gun's job (module docs).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct AiGun {
    pub phase: GunPhase,
    /// Tick the phase began.
    pub since: u64,
    pub site: FxVec2,
    /// The point it faces.
    pub facing: FxVec2,
    /// The last order sent, and when.
    pub moved_to: FxVec2,
    pub moved_at: u64,
    /// Failed moves, or `Emplace` orders sent, in this phase.
    pub fails: u8,
    /// The tow's squad while the gun comes off it.
    pub tow: Option<SquadId>,
    /// The gun's place counted as cover (`guns_in_cover`) once.
    pub counted: bool,
    /// The engineers ordered to dig its pit.
    pub pit_by: Option<SquadId>,
    /// A pit was ordered (never twice).
    pub pit: bool,
}

/// The gun's main-gun rounds, loaded one included.
fn rounds_of(vs: &crate::vehicle::VehicleState) -> u32 {
    vs.ammo.iter().map(|&n| n as u32).sum::<u32>() + u32::from(vs.loaded.is_some())
}

impl super::Planner<'_> {
    /// Takes the guns, their tows, their crews and the men on gun work out
    /// of the other habits; returns the guns.
    pub(super) fn take_guns(
        &mut self,
        tanks: &mut Vec<MyTank>,
        mine: &mut Vec<Mine>,
    ) -> Vec<MyTank> {
        let g = self.g;
        let me = self.me;
        let mut guns = Vec::new();
        tanks.retain(|t| {
            let Some(vs) = g.vehicles.get(&t.id) else {
                return true;
            };
            if vs.served.is_some() {
                guns.push(MyTank { ..*t });
                return false;
            }
            vs.towing.is_none()
                && g.setup.vehicles[vs.type_idx as usize].role != VehicleRole::Tractor
        });
        let mut serving: Vec<SquadId> = g
            .vehicles
            .iter()
            .filter(|&(id, _)| {
                self.ents.kind[id.index()] == EntityKind::Vehicle
                    && self.ents.owner[id.index()] == me
            })
            .filter_map(|(_, vs)| vs.served.as_ref()?.crew)
            .collect();
        serving.extend(self.ai.guns.values().filter_map(|j| j.pit_by));
        mine.retain(|m| {
            if serving.contains(&m.id) {
                return false;
            }
            // Men on their way to serve a gun.
            !matches!(g.squads[&m.id].order, Order::Recrew { vehicle }
                if g.vehicles.get(&vehicle).is_some_and(|v| v.served.is_some()))
        });
        guns
    }

    /// Runs the gun jobs (module docs), and sends idle infantry to serve
    /// guns that have no crew.
    pub(super) fn gun_jobs(
        &mut self,
        guns: &[MyTank],
        mine: &mut Vec<Mine>,
        points: &[PointEval],
        known: &[Known],
        kv: &[KnownVehicle],
    ) {
        let g = self.g;
        let me = self.me;
        self.ai.guns.retain(|id, _| {
            g.squads.get(id).is_some_and(|s| {
                s.owner == me
                    && s.members
                        .first()
                        .is_some_and(|v| g.vehicles.get(v).is_some_and(|x| x.served.is_some()))
            })
        });
        let tick = self.tick;
        for t in guns {
            if self.ai.guns.len() >= MAX_GUNS && !self.ai.guns.contains_key(&t.squad) {
                continue;
            }
            let mut job = self.ai.guns.get(&t.squad).copied().unwrap_or(AiGun {
                since: tick,
                ..AiGun::default()
            });
            self.run_gun(t, &mut job, mine, points, known, kv);
            self.ai.guns.insert(t.squad, job);
        }
        self.gun_recrews(mine, known, kv);
    }

    fn gun_set(&self, job: &mut AiGun, phase: GunPhase) {
        job.phase = phase;
        job.since = self.tick;
        job.fails = 0;
    }

    fn run_gun(
        &mut self,
        t: &MyTank,
        job: &mut AiGun,
        mine: &mut Vec<Mine>,
        points: &[PointEval],
        known: &[Known],
        kv: &[KnownVehicle],
    ) {
        let g = self.g;
        let Some(sv) = g.vehicles[&t.id].served.as_ref() else {
            return;
        };
        if job.phase == GunPhase::Stop {
            return;
        }
        if job.phase == GunPhase::Site {
            let Some((site, facing)) = self.gun_site(t, points, known, kv) else {
                return;
            };
            job.site = site;
            job.facing = facing;
            job.moved_to = t.pos;
            job.moved_at = 0;
            self.gun_set(job, GunPhase::Go);
        }
        match sv.state {
            GunState::Hitched => self.gun_go(t, sv.hitched, job, known, kv),
            GunState::Limbered => self.gun_limbered(t, job),
            GunState::Emplaced => self.gun_hold(t, job, mine, known, kv),
            _ => {}
        }
    }

    /// The enemy nearest `at` within `m`, soldiers and vehicles.
    fn gun_contact(&self, at: FxVec2, m: i32, known: &[Known], kv: &[KnownVehicle]) -> bool {
        let r = sq_m(Fx::from_int(m));
        known.iter().any(|k| (k.pos - at).length_sq_raw() <= r)
            || kv.iter().any(|k| (k.pos - at).length_sq_raw() <= r)
    }

    fn gun_go(
        &mut self,
        t: &MyTank,
        tow: Option<SimId>,
        job: &mut AiGun,
        known: &[Known],
        kv: &[KnownVehicle],
    ) {
        let g = self.g;
        let s = &g.setup;
        let a = &s.rules.ai.guns;
        let hz = s.tick_hz.max(1) as u64;
        let tick = self.tick;
        let Some(tow) = tow else { return };
        let Some(tq) = self.ents.squad[tow.index()] else {
            return;
        };
        let Some(sq) = g.squads.get(&tq) else { return };
        let tpos = self.ents.transform[tow.index()].pos.xy();
        let idle = sq.order == Order::Idle;
        let arrive = sq_m(Fx::from_int(a.arrive_m as i32));
        let there = (tpos - job.site).length_sq_raw() <= arrive;
        if idle && !there && job.moved_at != 0 && tick >= job.moved_at + a.repath_s as u64 * hz {
            job.fails = job.fails.saturating_add(1);
            self.ai.counters.gun_ai_fails += 1;
        }
        let spent = job.fails as u32 >= a.max_fails;
        let late = tick >= job.since + a.go_max_s as u64 * hz;
        if there || spent || late {
            // Off the tow here, unless the enemy is on it; then a site
            // further back, as long as the job has fails left.
            if !spent && self.gun_contact(tpos, a.contact_m as i32, known, kv) {
                let back = (g.base_of(self.me) - tpos).normalize_or_zero();
                job.site = self.terrain.clamp(tpos + back * Fx::from_int(BACK_OFF_M));
                job.fails = job.fails.saturating_add(1);
                job.since = tick;
                self.gun_drive(tq, job);
                return;
            }
            self.ai
                .emit(self.due, CommandKind::Unhitch { guns: vec![t.id] });
            self.ai.counters.gun_unhitches += 1;
            job.tow = Some(tq);
            self.gun_set(job, GunPhase::Unhitch);
            return;
        }
        if job.moved_to != job.site || (idle && tick >= job.moved_at + a.repath_s as u64 * hz) {
            self.gun_drive(tq, job);
        }
    }

    fn gun_drive(&mut self, tq: SquadId, job: &mut AiGun) {
        self.ai.emit(
            self.due,
            CommandKind::Move {
                squad: tq,
                target: job.site,
            },
        );
        job.moved_to = job.site;
        job.moved_at = self.tick;
    }

    fn gun_limbered(&mut self, t: &MyTank, job: &mut AiGun) {
        let g = self.g;
        let s = &g.setup;
        let a = &s.rules.ai.guns;
        let hz = s.tick_hz.max(1) as u64;
        // Off the tow: a tractor goes home once.
        if let Some(tq) = job.tow.take()
            && let Some(q) = g.squads.get(&tq).filter(|q| q.owner == self.me)
            && let Some(&v) = q.members.first()
            && g.vehicles
                .get(&v)
                .is_some_and(|x| s.vehicles[x.type_idx as usize].role == VehicleRole::Tractor)
        {
            self.ai.emit(
                self.due,
                CommandKind::Move {
                    squad: tq,
                    target: g.base_of(self.me),
                },
            );
        }
        if self.tick < job.moved_at + a.repath_s as u64 * hz && job.phase == GunPhase::Emplace {
            return;
        }
        if job.phase == GunPhase::Emplace && job.fails as u32 >= a.max_fails {
            self.ai.counters.gun_ai_fails += 1;
            self.gun_set(job, GunPhase::Stop);
            return;
        }
        if job.phase != GunPhase::Emplace {
            self.gun_set(job, GunPhase::Emplace);
        }
        self.ai.emit(
            self.due,
            CommandKind::Emplace {
                guns: vec![t.id],
                facing: job.facing,
            },
        );
        job.fails = job.fails.saturating_add(1);
        job.moved_at = self.tick;
    }

    /// Points along the approach from `p` in direction `dir`, at an
    /// enemy's eye height.
    fn gun_samples(&self, p: FxVec2, dir: FxVec2) -> Vec<FxVec3> {
        let a = &self.g.setup.rules.ai.guns;
        let rays = a.los_rays.max(1) as i32;
        let out = Fx::from_int(a.approach_m as i32);
        (0..rays)
            .map(|k| {
                let c = self.terrain.clamp(p + dir * out.mul_int(k).div_int(rays));
                c.extend(self.terrain.height_at(c) + Fx::from_milli(ENEMY_EYE_MM))
            })
            .collect()
    }

    /// Whether a gun at `at` has cover against `samples`: woods, or the
    /// ground masking its carriage from at least half of the points it
    /// sees from gun height (hull-down).
    fn gun_cover(&self, at: FxVec2, samples: &[FxVec3]) -> bool {
        if self.terrain.woods_at(at) {
            return true;
        }
        let ground = self.terrain.height_at(at);
        let eye = at.extend(ground + Fx::from_milli(GUN_EYE_MM));
        let low = at.extend(ground + Fx::from_milli(LOW_EYE_MM));
        let seen: Vec<&FxVec3> = samples
            .iter()
            .filter(|&&p| self.terrain.line_of_sight(eye, p))
            .collect();
        let masked = seen
            .iter()
            .filter(|&&&p| !self.terrain.line_of_sight(low, p))
            .count();
        !seen.is_empty() && masked * 2 >= seen.len()
    }

    fn gun_hold(
        &mut self,
        t: &MyTank,
        job: &mut AiGun,
        mine: &mut Vec<Mine>,
        known: &[Known],
        kv: &[KnownVehicle],
    ) {
        let g = self.g;
        let s = &g.setup;
        let a = &s.rules.ai.guns;
        let hz = s.tick_hz.max(1) as u64;
        let tick = self.tick;
        let vs = &g.vehicles[&t.id];
        let v = &s.vehicles[vs.type_idx as usize];
        if job.phase != GunPhase::Hold {
            self.gun_set(job, GunPhase::Hold);
            self.ai.emit(
                self.due,
                CommandKind::SetAmbush {
                    guns: vec![t.id],
                    range_m: a.engage_still_m.min(2000) as u16,
                },
            );
            // A site chosen from elsewhere (a tow lost) is judged as it
            // stands.
            let dir = (job.facing - t.pos).normalize_or_zero();
            let samples = self.gun_samples(t.pos + dir * Fx::from_int(a.site_min_m as i32), dir);
            job.counted = self.gun_cover(t.pos, &samples);
            if job.counted {
                self.ai.counters.guns_in_cover += 1;
            }
            job.moved_at = tick;
        }
        // The pit is done (the engineers stopped digging).
        if let Some(q) = job.pit_by
            && !g
                .squads
                .get(&q)
                .is_some_and(|x| matches!(x.order, Order::Dig { .. }))
        {
            job.pit_by = None;
            if !job.counted {
                job.counted = true;
                self.ai.counters.guns_in_cover += 1;
            }
        }
        // Only a gun that has no cover yet gets a pit.
        if !job.pit
            && !job.counted
            && a.pit_engineer_m > 0
            && !self.gun_contact(t.pos, a.contact_m as i32, known, kv)
        {
            let r = sq_m(Fx::from_int(a.pit_engineer_m as i32));
            let eng = mine
                .iter()
                .filter(|m| m.role == AiKind::Engineer && (m.lead - t.pos).length_sq_raw() <= r)
                .min_by_key(|m| ((m.lead - t.pos).length_sq_raw(), m.id))
                .map(|m| m.id);
            if let Some(q) = eng.filter(|q| {
                g.digs_profile(
                    &s.squad_types[g.squads[q].type_idx as usize],
                    DigProfile::GunPit,
                )
            }) {
                let (sin, cos) = t.yaw.sin_cos();
                let side = FxVec2::new(-sin, cos) * Fx::from_int(PIT_HALF_M);
                self.ai.emit(
                    self.due,
                    CommandKind::Dig {
                        squad: q,
                        line: vec![t.pos - side, t.pos + side],
                        profile: DigProfile::GunPit,
                    },
                );
                job.pit = true;
                job.pit_by = Some(q);
                self.ai.counters.gun_pits += 1;
                mine.retain(|m| m.id != q);
            }
        }
        // Re-lay for an armoured target in reach outside the arc, with
        // nothing to shoot inside it.
        let (Some([lo, hi]), None) = (v.gun_arc_deg, vs.target) else {
            return;
        };
        if tick < job.moved_at + a.relay_gap_s as u64 * hz {
            return;
        }
        let r = sq_m(Fx::from_int(a.engage_still_m as i32));
        let target = kv
            .iter()
            .filter(|k| {
                s.vehicles[k.type_idx as usize].role != VehicleRole::Truck
                    && (k.pos - t.pos).length_sq_raw() <= r
            })
            .min_by_key(|k| ((k.pos - t.pos).length_sq_raw(), k.id));
        let Some(k) = target else { return };
        let to = k.pos - t.pos;
        let off = (Fx::atan2(to.y, to.x) - t.yaw).wrap_angle();
        if off < deg(lo) || off > deg(hi) {
            self.ai.emit(
                self.due,
                CommandKind::FaceUnits {
                    units: vec![t.id],
                    toward: k.pos,
                },
            );
            job.moved_at = tick;
            self.ai.counters.gun_relays += 1;
        }
    }

    /// Where the gun goes: `(site, the point it faces)` (module docs).
    fn gun_site(
        &self,
        t: &MyTank,
        points: &[PointEval],
        known: &[Known],
        kv: &[KnownVehicle],
    ) -> Option<(FxVec2, FxVec2)> {
        let g = self.g;
        let a = &g.setup.rules.ai.guns;
        let held = (0..points.len())
            .filter(|&i| points[i].rel != Rel::Enemy)
            .max_by_key(|&i| {
                (
                    points[i].armour,
                    points[i].threat,
                    points[i].priority,
                    -(i as i64),
                )
            })
            .or_else(|| (!points.is_empty()).then_some(0))?;
        let p = points[held].pos;
        let enemy_base = (0..g.setup.players.len() as u8)
            .map(crate::ecs::PlayerId)
            .find(|&o| o != self.me)
            .map(|o| g.base_of(o));
        let toward = kv
            .iter()
            .filter(|k| g.setup.vehicles[k.type_idx as usize].role != VehicleRole::Truck)
            .min_by_key(|k| ((k.pos - p).length_sq_raw(), k.id))
            .map(|k| k.pos)
            .or(enemy_base)?;
        let dir = (toward - p).normalize_or_zero();
        if dir == FxVec2::ZERO {
            return None;
        }
        let out = Fx::from_int(a.approach_m as i32);
        let facing = self.terrain.clamp(p + dir * out);
        let samples = self.gun_samples(p, dir);
        let standoff = sq_m(Fx::from_int(a.enemy_standoff_m as i32));
        let rings = a.site_rings.max(1) as i32;
        let n = a.site_bearings.max(1) as i32;
        let span = a.site_max_m.saturating_sub(a.site_min_m) as i32;
        let others: Vec<FxVec2> = self
            .ai
            .guns
            .iter()
            .filter(|&(&q, j)| q != t.squad && j.phase != GunPhase::Site)
            .map(|(_, j)| j.site)
            .collect();
        let close = sq_m(Fx::from_int(a.site_apart_m as i32));
        let mut best: Option<((i32, i32, i64, i32), FxVec2)> = None;
        for r in 0..rings {
            let back = a.site_min_m as i32 + if rings > 1 { span * r / (rings - 1) } else { 0 };
            for b in 0..n {
                let lean = if n > 1 {
                    deg(a.site_spread_deg as i32 * (2 * b - (n - 1)) / (n - 1))
                } else {
                    Fx::ZERO
                };
                let (sin, cos) = lean.sin_cos();
                let way = FxVec2::new(dir.x * cos - dir.y * sin, dir.x * sin + dir.y * cos);
                let c = self.terrain.clamp(p - way * Fx::from_int(back));
                if known
                    .iter()
                    .any(|m| (m.pos - c).length_sq_raw() <= standoff)
                    || others.iter().any(|&o| (o - c).length_sq_raw() <= close)
                {
                    continue;
                }
                let ground = self.terrain.height_at(c);
                let eye = c.extend(ground + Fx::from_milli(GUN_EYE_MM));
                let sees = samples
                    .iter()
                    .filter(|&&s| self.terrain.line_of_sight(eye, s))
                    .count() as i32;
                if sees == 0 {
                    continue;
                }
                let cover = self.gun_cover(c, &samples) as i32;
                let near = -((c - t.pos).length_sq_raw().min(i64::MAX as u64) as i64);
                let key = (cover, sees, near, -(r * n + b));
                if best.as_ref().is_none_or(|(k, _)| key > *k) {
                    best = Some((key, c));
                }
            }
        }
        // Nowhere with a sight line: behind the point on the approach.
        let site = best.map_or_else(
            || {
                self.terrain
                    .clamp(p - dir * Fx::from_int(a.site_min_m as i32))
            },
            |(_, c)| c,
        );
        Some((site, facing))
    }

    /// Idle infantry near a gun with no crew serve it (module docs).
    fn gun_recrews(&mut self, mine: &mut Vec<Mine>, known: &[Known], kv: &[KnownVehicle]) {
        let g = self.g;
        let s = &g.setup;
        let a = &s.rules.ai.guns;
        if a.recrew_m == 0 {
            return;
        }
        let hz = s.tick_hz.max(1) as u64;
        let tick = self.tick;
        let gap = a.recrew_gap_s as u64 * hz;
        self.ai
            .gun_recrews
            .retain(|id, &mut at| g.vehicles.contains_key(id) && tick < at + gap);
        let me = self.me;
        let r = sq_m(Fx::from_int(a.recrew_m as i32));
        let list: Vec<SimId> = g
            .vehicles
            .iter()
            .filter(|&(&id, vs)| {
                let i = id.index();
                let free = match self.ents.kind[i] {
                    EntityKind::Abandoned => true,
                    // An own gun only once its crew is gone (they may be
                    // riding or walking back to it).
                    EntityKind::Vehicle => {
                        crate::guns::own_gun_unserved(self.ents, me, id, vs)
                            && vs.served.as_ref().is_some_and(|sv| {
                                sv.state != GunState::Hitched
                                    && sv.crew.is_none_or(|q| !g.squads.contains_key(&q))
                            })
                    }
                    _ => false,
                };
                free && vs.served.is_some() && rounds_of(vs) > 0
            })
            .map(|(&id, _)| id)
            .collect();
        for id in list {
            if self.ai.gun_recrews.len() >= MAX_RECREW_TRIES {
                return;
            }
            if self.ai.gun_recrews.contains_key(&id) {
                continue;
            }
            let at = self.ents.transform[id.index()].pos.xy();
            if self.gun_contact(at, RECREW_CLEAR_M, known, kv) {
                continue;
            }
            let pick = mine
                .iter()
                .filter(|m| {
                    m.role == AiKind::Rifle
                        && (m.lead - at).length_sq_raw() <= r
                        && crate::guns::can_serve(
                            s,
                            &s.squad_types[g.squads[&m.id].type_idx as usize],
                        )
                })
                .min_by_key(|m| ((m.lead - at).length_sq_raw(), m.id))
                .map(|m| m.id);
            let Some(q) = pick else { continue };
            self.ai.emit(
                self.due,
                CommandKind::Recrew {
                    squad: q,
                    vehicle: id,
                },
            );
            self.ai.counters.gun_recrew_orders += 1;
            self.ai.gun_recrews.insert(id, tick);
            mine.retain(|m| m.id != q);
        }
    }

    /// S9: manpower the other buys leave for the next gun: what the guns'
    /// share of the income (`income_max_pct`) has put aside so far, at most
    /// the card's price. Nothing until a gun may be bought or at `max_guns`.
    pub(super) fn gun_pot(&self) -> i64 {
        let g = self.g;
        let s = &g.setup;
        let a = &s.rules.ai.guns;
        let hz = s.tick_hz.max(1) as u64;
        if (self.tick < a.after_s as u64 * hz && !self.ai.enemy_vehicle_seen)
            || self.of_role(VehicleRole::AtGun) >= a.max_guns as usize
        {
            return 0;
        }
        let faction = &s.factions[s.players[self.me.0 as usize].faction as usize];
        let Some(card) = faction.roster.iter().copied().find(|&t| {
            let ty = &s.squad_types[t as usize];
            ty.ai_weight > 0
                && ty
                    .vehicle
                    .is_some_and(|v| s.vehicles[v as usize].role == VehicleRole::AtGun)
        }) else {
            return 0;
        };
        let cost = s.squad_types[card as usize].cost as i64;
        let spent: i64 = self
            .ai
            .counters
            .bought
            .iter()
            .enumerate()
            .filter(|(k, _)| {
                s.squad_types[*k]
                    .vehicle
                    .is_some_and(|v| s.vehicles[v as usize].role == VehicleRole::AtGun)
            })
            .map(|(k, &n)| n as i64 * s.squad_types[k].cost as i64)
            .sum();
        let minutes = (self.tick / (60 * hz)) as i64;
        let income = s.rules.start_manpower as i64 + s.rules.income_per_min as i64 * minutes;
        (income * a.income_max_pct as i64 / 100 - spent).clamp(0, cost)
    }

    /// S9: the AI's gun buy (module docs; S9-13: from `after_s`, or once an
    /// enemy vehicle has been seen).
    pub(super) fn buy_gun(&mut self) {
        let g = self.g;
        let s = &g.setup;
        let a = &s.rules.ai.guns;
        let hz = s.tick_hz.max(1) as u64;
        if self.tick < a.after_s as u64 * hz && !self.ai.enemy_vehicle_seen {
            return;
        }
        if self.of_role(VehicleRole::AtGun) >= a.max_guns as usize {
            return;
        }
        self.buy_role_within(VehicleRole::AtGun, a.income_max_pct, a.reserve_mp);
    }
}
