//! S9 AI habits for AT guns (`docs/slices/S9-at-guns.md` 12; `[ai.guns]`).
//! One saved job per own gun ([`AiGun`], keyed by the gun's squad):
//!
//! * **Site** (S9b). The most threatened own or contested point; with the
//!   enemy holding them all the gun waits where it is. The corridor is where enemy armour drives to it: the vehicle path from
//!   each seen enemy armoured vehicle (at most `corridor_paths`, nearest
//!   first), or from the enemy base with none seen, sampled every
//!   `corridor_step_m` over its last `corridor_m`, at an enemy's eye
//!   height; with no path, `los_rays` points on the straight line toward
//!   that vehicle or base. Candidates lie on rings `site_min_m`..
//!   `site_max_m` all around the point, never more than `site_forward_m`
//!   nearer to the enemy base than the point, away from known enemy men,
//!   the other gun and the points the enemy holds (`site_enemy_point_m`),
//!   and not within `corridor_clear_m` of the corridor (a gun on the route
//!   is overrun). The score is how many corridor samples within
//!   `engage_still_m` the gun sees from gun height inside its arc when it
//!   faces the middle of what it sees. Sites that see less than
//!   `site_view_min_pct` of the best are dropped; among the rest cover
//!   (woods, or hull-down: the gun-height line clear and the low line
//!   blocked), then the count, then the nearest.
//! * **Go.** The tow drives to the site with the crew riding. No unhitching
//!   with an enemy within `contact_m`: the site moves back toward the base.
//! * **Unhitch, Emplace** facing the approach. The tow goes back to the
//!   transport habits (a tractor with `[transport]` too).
//! * **Hold.** `SetAmbush` to `engage_still_m`; the usual targeting fires.
//!   An armoured target in reach outside the arc re-lays the trail.
//! * **Re-site.** With no vehicle in reach for `resite_idle_s` and armour
//!   known at another held point out of reach, the gun packs, a free tow
//!   (its own first) comes for it (`Pack`, `Fetch`) and the job starts
//!   over at **Site**; at most `resite_max` times.
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
use frontline_content::tuning::AiGunsTuning;
use frontline_content::vehicle::VehicleSetup;
use serde::{Deserialize, Serialize};

use super::{AiKind, Known, KnownVehicle, Mine, MyTank, PointEval, Rel, sq_m};
use crate::armour::deg;
use crate::command::CommandKind;
use crate::ecs::{EntityKind, PlayerId, SimId};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{Order, SquadId};
use crate::path::{PathCost, PathScratch, VehiclePathCost, find_vehicle_path};
use crate::terrain::Heightfield;
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
    /// Re-site: packing.
    Pack = 6,
    /// Re-site: waiting for the tow.
    Fetch = 7,
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
    /// The tow that brought it, or is coming for it (re-site).
    pub tow: Option<SquadId>,
    /// The gun's place counted as cover (`guns_in_cover`) once.
    pub counted: bool,
    /// The engineers ordered to dig its pit.
    pub pit_by: Option<SquadId>,
    /// A pit was ordered (never twice at one place).
    pub pit: bool,
    /// The site was chosen with cover: the gun must end up on it.
    pub site_cover: bool,
    /// Tick a vehicle was last in reach (or the gun came to Hold).
    pub quiet_at: u64,
    /// Moves made so far.
    pub resites: u8,
}

/// Armour for the corridor: what a gun is there to stop.
pub(super) fn is_armour(r: VehicleRole) -> bool {
    matches!(
        r,
        VehicleRole::Tank | VehicleRole::AssaultGun | VehicleRole::TankDestroyer
    )
}

/// Points on the polyline `way` every `step`, walking back from its end
/// (included) over at most `len`.
fn walk_back(way: &[FxVec2], step: Fx, len: Fx) -> Vec<FxVec2> {
    let mut out = Vec::new();
    let Some(&end) = way.last() else { return out };
    out.push(end);
    let mut walked = Fx::ZERO;
    let mut next = step;
    for pair in way.windows(2).rev() {
        let (a, b) = (pair[0], pair[1]);
        let seg = (a - b).length();
        if seg == Fx::ZERO {
            continue;
        }
        let d = (a - b).normalize_or_zero();
        while next <= len && next <= walked + seg {
            out.push(b + d * (next - walked));
            next += step;
        }
        if next > len {
            break;
        }
        walked += seg;
    }
    out
}

/// A candidate position with its place in the search order (ties go to the
/// earlier one).
struct Candidate {
    pos: FxVec2,
    order: i32,
}

/// Rings `site_min_m`..`site_max_m` all around `p`, bearing 0 on `dir`,
/// never more than `site_forward_m` nearer to the enemy base than `p`.
fn site_candidates(
    terrain: &Heightfield,
    p: FxVec2,
    dir: FxVec2,
    enemy_base: FxVec2,
    a: &AiGunsTuning,
) -> Vec<Candidate> {
    let rings = a.site_rings.max(1) as i32;
    let n = a.site_bearings.max(1) as i32;
    let span = a.site_max_m.saturating_sub(a.site_min_m) as i32;
    let limit = (p - enemy_base).length() - Fx::from_int(a.site_forward_m as i32);
    let mut out = Vec::new();
    for r in 0..rings {
        let radius = a.site_min_m as i32 + if rings > 1 { span * r / (rings - 1) } else { 0 };
        for b in 0..n {
            let (sin, cos) = deg(360 * b / n).sin_cos();
            let way = FxVec2::new(dir.x * cos - dir.y * sin, dir.x * sin + dir.y * cos);
            let pos = terrain.clamp(p + way * Fx::from_int(radius));
            if (pos - enemy_base).length() < limit {
                continue;
            }
            out.push(Candidate {
                pos,
                order: -(r * n + b),
            });
        }
    }
    out
}

/// The samples a gun at `c` sees from gun height within `reach` and inside
/// its arc `[lo, hi]` when it faces the middle of what it sees, and the
/// point it would face. `None` when it sees none, or stands within `clear`
/// of a sample.
fn site_view(
    terrain: &Heightfield,
    c: FxVec2,
    samples: &[FxVec3],
    reach: Fx,
    clear: Fx,
    arc: Option<[i32; 2]>,
) -> Option<(Vec<FxVec3>, FxVec2)> {
    let eye = c.extend(terrain.height_at(c) + Fx::from_milli(GUN_EYE_MM));
    let r = sq_m(reach);
    let off_route = sq_m(clear);
    if samples
        .iter()
        .any(|s| (s.xy() - c).length_sq_raw() < off_route)
    {
        return None;
    }
    let seen: Vec<FxVec3> = samples
        .iter()
        .filter(|s| (s.xy() - c).length_sq_raw() <= r)
        .filter(|&&s| terrain.line_of_sight(eye, s))
        .copied()
        .collect();
    let first = seen.first()?;
    // Sums of offsets, not of positions: no overflow on any map.
    let n = seen.len() as i64;
    let (mut sx, mut sy) = (0i64, 0i64);
    for s in &seen {
        sx += (s.x - c.x).raw() as i64;
        sy += (s.y - c.y).raw() as i64;
    }
    let mid = FxVec2::new(Fx::from_raw((sx / n) as i32), Fx::from_raw((sy / n) as i32));
    let way = if mid == FxVec2::ZERO {
        first.xy() - c
    } else {
        mid
    };
    let facing = c + way;
    let in_arc: Vec<FxVec3> = match arc {
        None => seen,
        Some([lo, hi]) => {
            let yaw = Fx::atan2(way.y, way.x);
            seen.into_iter()
                .filter(|s| {
                    let to = s.xy() - c;
                    let off = (Fx::atan2(to.y, to.x) - yaw).wrap_angle();
                    off >= deg(lo) && off <= deg(hi)
                })
                .collect()
        }
    };
    (!in_arc.is_empty()).then_some((in_arc, facing))
}

/// One candidate's result (`gun_site`).
struct Site {
    pos: FxVec2,
    facing: FxVec2,
    /// Corridor samples seen.
    count: i32,
    cover: bool,
    /// Minus the squared distance from the gun.
    near: i64,
    order: i32,
    seen: Vec<FxVec3>,
}

/// The fewest samples a site must see to stay in (`site_view_min_pct` of
/// the best, rounded up, at least one).
fn view_cut(sites: &[Site], pct: u32) -> i32 {
    let best = sites.iter().map(|x| x.count).max().unwrap_or(0) as u32;
    (best * pct).div_ceil(100).max(1) as i32
}

/// Cover, then the count, then the nearest, among the sites at `cut` or
/// over.
fn choose_site(sites: &[Site], cut: i32) -> Option<&Site> {
    sites
        .iter()
        .filter(|x| x.count >= cut)
        .max_by_key(|x| (x.cover, x.count, x.near, x.order))
}

/// Whether a gun at `at` has cover against `samples`: woods, or the
/// ground masking its carriage from at least half of the points it sees
/// from gun height (hull-down).
fn gun_cover(terrain: &Heightfield, at: FxVec2, samples: &[FxVec3]) -> bool {
    if terrain.woods_at(at) {
        return true;
    }
    let ground = terrain.height_at(at);
    let eye = at.extend(ground + Fx::from_milli(GUN_EYE_MM));
    let low = at.extend(ground + Fx::from_milli(LOW_EYE_MM));
    let seen: Vec<&FxVec3> = samples
        .iter()
        .filter(|&&p| terrain.line_of_sight(eye, p))
        .collect();
    let masked = seen
        .iter()
        .filter(|&&&p| !terrain.line_of_sight(low, p))
        .count();
    !seen.is_empty() && masked * 2 >= seen.len()
}

/// What the site search takes from the world (`gun_site`).
struct SiteSearch<'a> {
    terrain: &'a Heightfield,
    /// The point to cover.
    point: FxVec2,
    /// Bearing 0 of the rings.
    dir: FxVec2,
    enemy_base: FxVec2,
    /// Where enemy armour drives, at an enemy's eye height.
    samples: &'a [FxVec3],
    arc: Option<[i32; 2]>,
    /// The gun.
    from: FxVec2,
    /// Known enemy men.
    men: &'a [FxVec2],
    /// The other gun's site.
    others: &'a [FxVec2],
    /// The points the enemy holds.
    garrisons: &'a [FxVec2],
}

/// The best site and the point it faces, and whether it has cover (module
/// docs). `None` when no site sees any of the corridor.
fn best_site(q: &SiteSearch, a: &AiGunsTuning) -> Option<(FxVec2, FxVec2, bool)> {
    let reach = Fx::from_int(a.engage_still_m as i32);
    let clear = Fx::from_int(a.corridor_clear_m as i32);
    let within = |list: &[FxVec2], c: FxVec2, m: u32| {
        let r = sq_m(Fx::from_int(m as i32));
        list.iter().any(|&o| (o - c).length_sq_raw() <= r)
    };
    let mut sites: Vec<Site> = Vec::new();
    for c in site_candidates(q.terrain, q.point, q.dir, q.enemy_base, a) {
        if within(q.men, c.pos, a.enemy_standoff_m)
            || within(q.others, c.pos, a.site_apart_m)
            || within(q.garrisons, c.pos, a.site_enemy_point_m)
        {
            continue;
        }
        let Some((seen, facing)) = site_view(q.terrain, c.pos, q.samples, reach, clear, q.arc)
        else {
            continue;
        };
        sites.push(Site {
            pos: c.pos,
            facing,
            count: seen.len() as i32,
            cover: false,
            near: -((c.pos - q.from).length_sq_raw().min(i64::MAX as u64) as i64),
            order: c.order,
            seen,
        });
    }
    let cut = view_cut(&sites, a.site_view_min_pct);
    for x in sites.iter_mut().filter(|x| x.count >= cut) {
        x.cover = gun_cover(q.terrain, x.pos, &x.seen);
    }
    choose_site(&sites, cut).map(|b| (b.pos, b.facing, b.cover))
}

/// The own or contested point a gun covers: the most armour, then threat,
/// then priority, then the first. `None` when the enemy holds them all: a
/// site around such a point is the enemy's ground.
fn point_to_cover(points: &[PointEval]) -> Option<usize> {
    (0..points.len())
        .filter(|&i| points[i].rel != Rel::Enemy)
        .max_by_key(|&i| {
            (
                points[i].armour,
                points[i].threat,
                points[i].priority,
                -(i as i64),
            )
        })
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
        let fetching: Vec<SquadId> = self
            .ai
            .guns
            .values()
            .filter(|j| matches!(j.phase, GunPhase::Pack | GunPhase::Fetch))
            .filter_map(|j| j.tow)
            .collect();
        tanks.retain(|t| {
            let Some(vs) = g.vehicles.get(&t.id) else {
                return true;
            };
            if vs.served.is_some() {
                guns.push(MyTank { ..*t });
                return false;
            }
            // A tractor with seats works as a transport like any half-track;
            // one with none has no habit to go to.
            vs.towing.is_none()
                && !fetching.contains(&t.squad)
                && (g.setup.vehicles[vs.type_idx as usize].role != VehicleRole::Tractor
                    || g.transport_of(t.id).is_some())
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
        if sv.state == GunState::Hitched && job.phase == GunPhase::Fetch {
            job.phase = GunPhase::Site;
        }
        if job.phase == GunPhase::Site {
            let Some((site, facing, cover)) = self.gun_site(t, points, known, kv) else {
                return;
            };
            job.site = site;
            job.facing = facing;
            job.site_cover = cover;
            job.moved_to = t.pos;
            job.moved_at = 0;
            self.gun_set(job, GunPhase::Go);
        }
        match sv.state {
            GunState::Hitched => self.gun_go(t, sv.hitched, job, known, kv),
            GunState::Limbered if matches!(job.phase, GunPhase::Pack | GunPhase::Fetch) => {
                self.gun_fetch(t, job);
            }
            GunState::Limbered => self.gun_limbered(t, job),
            // A `Pack` still on its way, or refused.
            GunState::Emplaced if job.phase == GunPhase::Pack => {
                let hz = g.setup.tick_hz.max(1) as u64;
                if self.tick >= job.since + g.setup.rules.ai.guns.repath_s as u64 * hz {
                    self.gun_set(job, GunPhase::Hold);
                    job.quiet_at = self.tick;
                }
            }
            GunState::Emplaced => self.gun_hold(t, job, mine, points, known, kv),
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
        // A covered site is only cover with the gun on it.
        let there = if job.site_cover {
            (t.pos - job.site).length_sq_raw() <= sq_m(Fx::from_int(a.arrive_cover_m as i32))
        } else {
            (tpos - job.site).length_sq_raw() <= sq_m(Fx::from_int(a.arrive_m as i32))
        };
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
                job.site_cover = false;
                self.gun_drive(t, tq, tpos, job);
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
            self.gun_drive(t, tq, tpos, job);
        }
    }

    /// The tow drives to the site; for a covered site, past it by the
    /// tow's lead over the gun, so that the gun stops on it.
    fn gun_drive(&mut self, t: &MyTank, tq: SquadId, tpos: FxVec2, job: &mut AiGun) {
        let target = if job.site_cover {
            let way = (job.site - tpos).normalize_or_zero();
            self.terrain.clamp(job.site + way * (tpos - t.pos).length())
        } else {
            job.site
        };
        self.ai
            .emit(self.due, CommandKind::Move { squad: tq, target });
        job.moved_to = job.site;
        job.moved_at = self.tick;
    }

    fn gun_limbered(&mut self, t: &MyTank, job: &mut AiGun) {
        let g = self.g;
        let s = &g.setup;
        let a = &s.rules.ai.guns;
        let hz = s.tick_hz.max(1) as u64;
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

    fn gun_hold(
        &mut self,
        t: &MyTank,
        job: &mut AiGun,
        mine: &mut Vec<Mine>,
        points: &[PointEval],
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
            // On the site that was chosen with cover it has that cover;
            // anywhere else it is judged where it stands, along the way it
            // faces.
            let on_site = job.site_cover
                && (t.pos - job.site).length_sq_raw()
                    <= sq_m(Fx::from_int(a.arrive_cover_m as i32));
            let dir = (job.facing - t.pos).normalize_or_zero();
            let out = Fx::from_int(a.approach_m as i32).div_int(a.los_rays.max(1) as i32);
            job.counted = on_site
                || gun_cover(
                    self.terrain,
                    t.pos,
                    &self.gun_samples(t.pos + dir * out, dir),
                );
            if job.counted {
                self.ai.counters.guns_in_cover += 1;
            }
            job.moved_at = tick;
            job.quiet_at = tick;
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
        if self.gun_resite(t, job, points, known, kv) {
            return;
        }
        // Re-lay for an armoured target in reach outside the arc, with
        // nothing to shoot inside it (or the target it holds lies outside).
        let Some([lo, hi]) = v.gun_arc_deg else {
            return;
        };
        if vs.target.is_some() && vs.lay_turn.is_none() {
            return;
        }
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

    /// Starts a re-site (module docs) when it is due. Whether it did.
    fn gun_resite(
        &mut self,
        t: &MyTank,
        job: &mut AiGun,
        points: &[PointEval],
        known: &[Known],
        kv: &[KnownVehicle],
    ) -> bool {
        let g = self.g;
        let s = &g.setup;
        let a = &s.rules.ai.guns;
        if a.resite_idle_s == 0 || job.resites as u32 >= a.resite_max {
            return false;
        }
        let tick = self.tick;
        let hz = s.tick_hz.max(1) as u64;
        let vs = &g.vehicles[&t.id];
        let reach = sq_m(Fx::from_int(a.engage_still_m as i32));
        let armour_at = |k: &KnownVehicle| {
            s.vehicles[k.type_idx as usize].role != VehicleRole::Truck
                && (k.pos - t.pos).length_sq_raw() <= reach
        };
        if vs
            .target
            .is_some_and(|x| self.ents.kind[x.index()] == EntityKind::Vehicle)
            || kv.iter().any(armour_at)
        {
            job.quiet_at = tick;
            return false;
        }
        if tick < job.quiet_at + a.resite_idle_s as u64 * hz
            || self.gun_contact(t.pos, a.contact_m as i32, known, kv)
            || !points.iter().any(|p| {
                p.rel != Rel::Enemy && p.armour > 0 && (p.pos - t.pos).length_sq_raw() > reach
            })
        {
            return false;
        }
        // A free tow, its own first; never a truck on its supply round.
        let mass = s.vehicles[vs.type_idx as usize].mass_kg;
        let me = self.me;
        let claimed: Vec<SquadId> = self
            .ai
            .guns
            .values()
            .filter(|j| matches!(j.phase, GunPhase::Pack | GunPhase::Fetch))
            .filter_map(|j| j.tow)
            .collect();
        let own = job.tow;
        let tow = g
            .vehicles
            .iter()
            .filter(|&(&id, x)| {
                let d = &s.vehicles[x.type_idx as usize];
                self.ents.kind[id.index()] == EntityKind::Vehicle
                    && self.ents.owner[id.index()] == me
                    && x.towing.is_none()
                    && x.riders.iter().all(Option::is_none)
                    && d.role != VehicleRole::Truck
                    && d.tow.is_some_and(|r| r.max_kg >= mass)
            })
            .filter_map(|(&id, _)| self.ents.squad[id.index()].map(|q| (id, q)))
            .filter(|(_, q)| !claimed.contains(q))
            .min_by_key(|&(id, q)| {
                (
                    Some(q) != own,
                    (self.ents.transform[id.index()].pos.xy() - t.pos).length_sq_raw(),
                    id.0,
                )
            })
            .map(|(_, q)| q);
        let Some(tq) = tow else { return false };
        self.ai
            .emit(self.due, CommandKind::PackGun { guns: vec![t.id] });
        self.ai.counters.gun_resites += 1;
        job.tow = Some(tq);
        job.resites = job.resites.saturating_add(1);
        job.site_cover = false;
        job.pit = false;
        job.pit_by = None;
        self.gun_set(job, GunPhase::Pack);
        true
    }

    /// Re-site: the gun is limbered; its tow comes and takes it. With no
    /// tow left, or none in `go_max_s`, it goes back into action here.
    fn gun_fetch(&mut self, t: &MyTank, job: &mut AiGun) {
        let g = self.g;
        let a = &g.setup.rules.ai.guns;
        let hz = g.setup.tick_hz.max(1) as u64;
        let tick = self.tick;
        if job.phase == GunPhase::Pack {
            self.gun_set(job, GunPhase::Fetch);
            job.moved_at = 0;
        }
        let tow = job
            .tow
            .and_then(|q| {
                g.squads
                    .get(&q)
                    .filter(|x| x.owner == self.me)
                    .map(|x| (q, x))
            })
            .and_then(|(q, x)| x.members.first().map(|&v| (q, x, v)))
            .filter(|&(_, _, v)| {
                self.ents.kind[v.index()] == EntityKind::Vehicle
                    && g.vehicles.get(&v).is_some_and(|x| x.towing.is_none())
            });
        let late = tick >= job.since + a.go_max_s as u64 * hz;
        let Some((tq, sq, v)) = tow.filter(|_| !late && job.fails as u32 <= a.max_fails) else {
            self.ai.counters.gun_ai_fails += 1;
            self.gun_set(job, GunPhase::Emplace);
            job.moved_at = 0;
            return;
        };
        let due = tick >= job.moved_at + a.repath_s as u64 * hz;
        if g.in_hitch_reach(self.ents, t.id, v) {
            if due {
                self.ai.emit(
                    self.due,
                    CommandKind::Hitch {
                        guns: vec![t.id],
                        tow: v,
                    },
                );
                job.fails = job.fails.saturating_add(1);
                job.moved_at = tick;
            }
            return;
        }
        if job.moved_at == 0 || (sq.order == Order::Idle && due) {
            if job.moved_at != 0 {
                job.fails = job.fails.saturating_add(1);
                self.ai.counters.gun_ai_fails += 1;
            }
            let target = self.terrain.clamp(g.trail_tip(self.ents, t.id));
            self.ai
                .emit(self.due, CommandKind::Move { squad: tq, target });
            job.moved_at = tick;
        }
    }

    /// The vehicle route of a `ty` from `from` to `to`, start included;
    /// `None` when the search does not reach `to`.
    pub(super) fn gun_route(
        &self,
        from: FxVec2,
        to: FxVec2,
        ty: &VehicleSetup,
        scratch: &mut PathScratch,
    ) -> Option<Vec<FxVec2>> {
        self.gun_route_on(from, to, ty, self.g.vderived.block_of(ty), scratch)
    }

    /// [`Self::gun_route`] on block layer `block` (S10c-ai C5: the layer
    /// with a belt's nodes blocked).
    pub(super) fn gun_route_on(
        &self,
        from: FxVec2,
        to: FxVec2,
        ty: &VehicleSetup,
        block: &crate::vehicle::BlockLayer,
        scratch: &mut PathScratch,
    ) -> Option<Vec<FxVec2>> {
        let s = &self.g.setup;
        let mv = &s.rules.movement;
        let cost = PathCost {
            slope_cost_pct: mv.slope_cost_pct,
            max_slope_pct: ty.mobility.max_slope_pct,
            trench_cost_pct: 100,
            ditch_cost_pct: mv.ditch_cost_pct,
        };
        let veh = VehiclePathCost {
            block,
            avoid: None,
            costs: &s.rules.vehicles.path,
            mines: None,
        };
        let r = find_vehicle_path(
            self.terrain,
            from,
            to,
            &cost,
            &veh,
            mv.path_max_expansions,
            scratch,
        );
        r.reached.then(|| {
            let mut way = Vec::with_capacity(r.points.len() + 1);
            way.push(from);
            way.extend(r.points);
            way
        })
    }

    /// The ground enemy armour drives over to reach `p` (module docs), at
    /// an enemy's eye height. Empty when no route is found.
    fn gun_corridor(&self, p: FxVec2, kv: &[KnownVehicle], enemy: PlayerId) -> Vec<FxVec3> {
        let g = self.g;
        let s = &g.setup;
        let a = &s.rules.ai.guns;
        let step = Fx::from_int(a.corridor_step_m.max(1) as i32);
        let len = Fx::from_int(a.corridor_m as i32);
        let mut scratch = PathScratch::default();
        let mut out: Vec<FxVec2> = Vec::new();
        let mut near: Vec<&KnownVehicle> = kv
            .iter()
            .filter(|k| is_armour(s.vehicles[k.type_idx as usize].role))
            .collect();
        near.sort_by_key(|k| ((k.pos - p).length_sq_raw(), k.id));
        for k in near.iter().take(a.corridor_paths as usize) {
            let ty = &s.vehicles[k.type_idx as usize];
            if let Some(way) = self.gun_route(k.pos, p, ty, &mut scratch) {
                out.extend(walk_back(&way, step, len));
            }
        }
        // None seen, or they stand so near that their routes say little.
        if out.len() < a.los_rays as usize {
            let roster = &s.factions[s.players[enemy.0 as usize].faction as usize].roster;
            let ty = roster
                .iter()
                .filter_map(|&t| s.vehicles.get(s.squad_types[t as usize].vehicle? as usize))
                .filter(|v| is_armour(v.role))
                .min_by_key(|v| v.mobility.max_slope_pct);
            if let Some(way) =
                ty.and_then(|ty| self.gun_route(g.base_of(enemy), p, ty, &mut scratch))
            {
                out.extend(walk_back(&way, step, len));
            }
        }
        out.iter()
            .map(|&c| {
                let c = self.terrain.clamp(c);
                c.extend(self.terrain.height_at(c) + Fx::from_milli(ENEMY_EYE_MM))
            })
            .collect()
    }

    /// The ground a site must see: the corridor, or with no route found
    /// the straight line along `dir`.
    fn gun_sight_samples(
        &self,
        p: FxVec2,
        dir: FxVec2,
        kv: &[KnownVehicle],
        enemy: PlayerId,
    ) -> Vec<FxVec3> {
        let corridor = self.gun_corridor(p, kv, enemy);
        if corridor.is_empty() {
            self.gun_samples(p, dir)
        } else {
            corridor
        }
    }

    /// Where the gun goes: `(site, the point it faces, whether the site
    /// has cover)` (module docs). `None` leaves it where it is.
    fn gun_site(
        &self,
        t: &MyTank,
        points: &[PointEval],
        known: &[Known],
        kv: &[KnownVehicle],
    ) -> Option<(FxVec2, FxVec2, bool)> {
        let g = self.g;
        let s = &g.setup;
        let a = &s.rules.ai.guns;
        let p = points[point_to_cover(points)?].pos;
        let enemy = (0..s.players.len() as u8)
            .map(PlayerId)
            .find(|&o| o != self.me)?;
        let enemy_base = g.base_of(enemy);
        let toward = kv
            .iter()
            .filter(|k| s.vehicles[k.type_idx as usize].role != VehicleRole::Truck)
            .min_by_key(|k| ((k.pos - p).length_sq_raw(), k.id))
            .map_or(enemy_base, |k| k.pos);
        let dir = (toward - p).normalize_or_zero();
        if dir == FxVec2::ZERO {
            return None;
        }
        let samples = self.gun_sight_samples(p, dir, kv, enemy);
        let men: Vec<FxVec2> = known.iter().map(|m| m.pos).collect();
        let others: Vec<FxVec2> = self
            .ai
            .guns
            .iter()
            .filter(|&(&q, j)| q != t.squad && j.phase != GunPhase::Site)
            .map(|(_, j)| j.site)
            .collect();
        let garrisons: Vec<FxVec2> = points
            .iter()
            .filter(|x| x.rel == Rel::Enemy)
            .map(|x| x.pos)
            .collect();
        let search = SiteSearch {
            terrain: self.terrain,
            point: p,
            dir,
            enemy_base,
            samples: &samples,
            arc: s.vehicles[g.vehicles[&t.id].type_idx as usize].gun_arc_deg,
            from: t.pos,
            men: &men,
            others: &others,
            garrisons: &garrisons,
        };
        let Some((site, facing, cover)) = best_site(&search, a) else {
            // Nowhere with a sight line: behind the point on the approach.
            let site = self
                .terrain
                .clamp(p - dir * Fx::from_int(a.site_min_m as i32));
            let facing = self
                .terrain
                .clamp(p + dir * Fx::from_int(a.approach_m as i32));
            return Some((site, facing, false));
        };
        Some((site, facing, cover))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::TerrainKind;

    const SIZE: i32 = 1024;

    fn at(x: i32, y: i32) -> FxVec2 {
        FxVec2::new(Fx::from_int(x), Fx::from_int(y))
    }

    fn tuning() -> AiGunsTuning {
        AiGunsTuning {
            engage_still_m: 400,
            site_forward_m: 0,
            site_enemy_point_m: 250,
            site_view_min_pct: 50,
            corridor_clear_m: 150,
            ..AiGunsTuning::default()
        }
    }

    fn eval(rel: Rel, armour: u32) -> PointEval {
        PointEval {
            pos: at(512, 512),
            rel,
            threat: 0,
            armour,
            priority: 0,
            demand: 0,
            assigned: 0,
            trench: None,
            trench_done: false,
            wants_works: false,
            hold: None,
        }
    }

    #[test]
    fn with_every_point_enemy_held_a_gun_has_no_point_to_cover() {
        assert_eq!(point_to_cover(&[]), None);
        assert_eq!(
            point_to_cover(&[eval(Rel::Enemy, 3), eval(Rel::Enemy, 1)]),
            None
        );
    }

    #[test]
    fn the_point_to_cover_is_the_most_armoured_one_not_enemy_held() {
        let points = [
            eval(Rel::Enemy, 9),
            eval(Rel::Neutral, 1),
            eval(Rel::Own, 2),
            eval(Rel::Own, 2),
        ];
        assert_eq!(point_to_cover(&points), Some(2));
    }

    /// Enemy armour drives along y = 512 from x = 900 to the point at
    /// (512, 512), every 25 m, at an enemy's eye height.
    fn corridor(t: &Heightfield) -> Vec<FxVec3> {
        (0..16)
            .map(|k| {
                let c = at(512 + 25 * k, 512);
                c.extend(t.height_at(c) + Fx::from_milli(ENEMY_EYE_MM))
            })
            .collect()
    }

    /// Flat, with one 12 m ridge across the map behind the point (x 370..
    /// 380) and one along the route to its south (y 440..450): only the
    /// ground north of the route, on the enemy's side of the first ridge,
    /// looks at the corridor.
    fn ridges() -> Heightfield {
        let mut t = Heightfield::generate(TerrainKind::Flat, SIZE, 1);
        t.edit_heights_for_test(|gx, gy, h| {
            let (x, y) = (gx as i32 * 4, gy as i32 * 4);
            if (370..=380).contains(&x) || (440..=450).contains(&y) {
                *h = Fx::from_int(12);
            }
        });
        t
    }

    fn search<'a>(t: &'a Heightfield, samples: &'a [FxVec3]) -> SiteSearch<'a> {
        SiteSearch {
            terrain: t,
            point: at(512, 512),
            dir: FxVec2::new(Fx::ONE, Fx::ZERO),
            enemy_base: at(960, 512),
            samples,
            arc: Some([-32, 32]),
            from: at(300, 512),
            men: &[],
            others: &[],
            garrisons: &[],
        }
    }

    fn site(count: i32, cover: bool, order: i32) -> Site {
        Site {
            pos: at(order, 0),
            facing: at(0, 0),
            count,
            cover,
            near: 0,
            order,
            seen: Vec::new(),
        }
    }

    #[test]
    fn a_gun_on_a_ridge_map_sees_the_corridor_within_reach() {
        let t = ridges();
        let samples = corridor(&t);
        let a = tuning();
        let (pos, facing, _) = best_site(&search(&t, &samples), &a).expect("a site");
        assert!(
            pos.y > Fx::from_int(450) && pos.x > Fx::from_int(380),
            "the open ground north of the route: {pos:?}"
        );
        let eye = pos.extend(t.height_at(pos) + Fx::from_milli(GUN_EYE_MM));
        let reach = sq_m(Fx::from_int(a.engage_still_m as i32));
        let seen = samples
            .iter()
            .filter(|&&s| (s.xy() - pos).length_sq_raw() <= reach && t.line_of_sight(eye, s))
            .count();
        assert!(seen >= 8, "sees {seen} of {}", samples.len());
        assert!(facing.y < pos.y, "faces the corridor: {facing:?}");
        // Not on the route.
        let clear = sq_m(Fx::from_int(a.corridor_clear_m as i32));
        assert!(
            samples
                .iter()
                .all(|s| (s.xy() - pos).length_sq_raw() >= clear)
        );
    }

    #[test]
    fn a_covered_site_that_sees_nothing_loses_to_an_open_one_that_does() {
        let t = ridges();
        let samples = corridor(&t);
        let reach = Fx::from_int(400);
        let clear = Fx::from_int(150);
        // Behind the ridge: blind, so it is no site at all, whatever its
        // cover.
        assert!(site_view(&t, at(300, 512), &samples, reach, clear, None).is_none());
        assert!(site_view(&t, at(512, 700), &samples, reach, clear, None).is_some());
        // Sees too little of what the best site sees: dropped before cover
        // counts.
        let sites = [site(1, true, 1), site(10, false, 2)];
        let cut = view_cut(&sites, 50);
        assert_eq!(cut, 5);
        assert_eq!(choose_site(&sites, cut).map(|x| x.order), Some(2));
        // At the cut or over it, cover decides, then the count.
        let sites = [site(6, true, 1), site(10, false, 2), site(5, true, 3)];
        let cut = view_cut(&sites, 50);
        assert_eq!(choose_site(&sites, cut).map(|x| x.order), Some(1));
    }

    #[test]
    fn no_site_within_the_clearance_of_the_corridor() {
        let t = Heightfield::generate(TerrainKind::Flat, SIZE, 1);
        let samples = corridor(&t);
        let reach = Fx::from_int(400);
        let clear = Fx::from_int(150);
        assert!(site_view(&t, at(512, 600), &samples, reach, clear, None).is_none());
        assert!(site_view(&t, at(512, 700), &samples, reach, clear, None).is_some());
    }

    #[test]
    fn no_site_forward_of_the_limit_or_near_a_point_the_enemy_holds() {
        let t = Heightfield::generate(TerrainKind::Flat, SIZE, 1);
        let samples = corridor(&t);
        let p = at(512, 512);
        let base = at(960, 512);
        for forward in [0, 100] {
            let a = AiGunsTuning {
                site_forward_m: forward,
                ..tuning()
            };
            let limit = (p - base).length() - Fx::from_int(forward as i32);
            let list = site_candidates(&t, p, FxVec2::new(Fx::ONE, Fx::ZERO), base, &a);
            assert!(!list.is_empty());
            assert!(list.iter().all(|c| (c.pos - base).length() >= limit));
        }
        let garrison = [at(512, 760)];
        let a = tuning();
        let mut q = search(&t, &samples);
        q.garrisons = &garrison;
        let (pos, _, _) = best_site(&q, &a).expect("a site");
        assert!((pos - garrison[0]).length() > Fx::from_int(a.site_enemy_point_m as i32));
        // Every candidate taken away: no site, and the caller falls back.
        let all_held = [at(512, 512)];
        q.garrisons = &all_held;
        let a = AiGunsTuning {
            site_enemy_point_m: 300,
            ..a
        };
        assert!(best_site(&q, &a).is_none());
    }

    #[test]
    fn with_no_route_the_samples_are_the_straight_line() {
        use frontline_content::{Difficulty, MatchRequest, ModDefs};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
        let defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
        let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
        let mut setup = defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
        setup.players.iter_mut().for_each(|p| p.is_ai = true);
        let w = crate::World::new_match(setup, 1).unwrap();
        let g = w.game().unwrap();
        let enemy = PlayerId(1);
        let p = at(512, 512);
        let dir = (g.base_of(enemy) - p).normalize_or_zero();
        // Open ground: the route, every `corridor_step_m` over its last
        // `corridor_m`.
        let mut ai = g.ai(PlayerId(0)).unwrap().clone();
        let mut flat = Heightfield::generate(TerrainKind::Flat, g.setup().map.size_m, 1);
        let samples = |ai: &mut super::super::AiState, terrain: &Heightfield| {
            super::super::Planner {
                g,
                ai,
                me: PlayerId(0),
                ents: w.entities(),
                terrain,
                due: 0,
                tick: 0,
                quality: 100,
                rng: crate::rng::RngStream::new(
                    1,
                    0,
                    crate::ecs::SimId(0),
                    crate::rng::Purpose::AI_PLAN,
                ),
                mortar_rays: 0,
                at_sites: Default::default(),
                own_routes: Default::default(),
            }
            .gun_sight_samples(p, dir, &[], enemy)
        };
        let a = &g.setup().rules.ai.guns;
        let route = samples(&mut ai, &flat);
        assert!(route.len() > a.los_rays as usize, "{}", route.len());
        // A wall all around the point: no route, the straight line.
        flat.edit_heights_for_test(|gx, gy, h| {
            let (x, y) = (gx as i32 * 4 - 512, gy as i32 * 4 - 512);
            if (x * x + y * y - 40 * 40).abs() < 800 {
                *h = Fx::from_int(40);
            }
        });
        let line = samples(&mut ai, &flat);
        assert_eq!(line.len(), a.los_rays as usize);
        let step = Fx::from_int(a.approach_m as i32).div_int(a.los_rays as i32);
        assert!((line[1].xy() - (p + dir * step)).length() < Fx::from_int(1));
    }
}
