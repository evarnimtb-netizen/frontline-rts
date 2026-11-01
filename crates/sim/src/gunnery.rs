//! Vehicle gunnery (S3, `docs/slices/S3-armour.md` "Gunnery"): the
//! Weapons stage's vehicle part, serial in ascending vehicle id after the
//! soldiers' parallel aim and fire.
//!
//! **Targets.** Once a second (staggered by id), or the tick its target is
//! lost, the gunner picks: the `AttackUnit` vehicle while it is seen and in
//! range; else the nearest seen enemy vehicle within the main gun's
//! `range_max` that has a visible part; else the best seen infantry group
//! in range (more men within `vehicles.gunnery.he_group_m` first, then
//! closer), never within `he_clear` of the own men. A vehicle target gets
//! its aim point from [`aim_choice`] (S3b): the centre of its visible mass
//! (hull and turret centres weighted by their projected areas toward the
//! shooter) when that point is in a clear line of fire, else the turret's
//! centre (partial hull-down). Retreating vehicles do not use the main
//! gun.
//!
//! **Ammo.** `Auto` wants the penetrator against vehicles and HE against
//! men; `Ap` and `He` force one. A wrong round in the breech is unloaded
//! (back to the rack) and the wanted one loaded, at the cost of one
//! reload; an empty breech loads when the reload is done. The reload is the
//! gun's, ×1.5 with the loader wounded and ×2 with him gone.
//!
//! **Laying.** The solution leads the target by its velocity (a vehicle's
//! ~1 s average, a soldier's current; S3b: against a moving vehicle the
//! gunner's estimate, [`lead_estimate`]) times the flight time and
//! compensates drop as `combat::shot_velocity` does, two fixed iterations.
//! [`lay_gun`] turns the turret at the def's traverse (×0.5 with the ring
//! damaged, fixed with it destroyed) and the gun at the def's `elevate_deg_s`
//! within its elevation limits. The gun fires only when laid within its
//! current spread of the solution, along its actual laid direction plus
//! spread (RNG `GUN_SPREAD`).
//!
//! **Spread.** The gun's (HE at men: the rules' `he_area_spread`); each
//! further shot at the same target 20 % tighter down to 50 % (ranging);
//! × the def's `moving_spread_pct` while moving; × `moving_target_spread_pct`
//! against a moving vehicle; ×1.5 with the gunner wounded; ×2 with the gun
//! damaged.
//!
//! **MGs.** The coax engages the main gun's soldier target, else the
//! nearest seen enemy soldier within `vehicles.gunnery.coax_cone_deg` of the turret; the
//! hull MG the nearest seen enemy soldiers within its `arc_deg` (sweeping
//! among the four nearest, as S1 MGs). Both need their crewman (gunner,
//! radio operator), fire at the MG's interval with belts and reloads, and
//! go through the S1 spread (moving, range). Vehicles are never suppressed.
//!
//! **Suppress (S3b, docs/slices/S3b-balance.md B2).** A vehicle holding
//! with `VehicleState::suppress` picks only vehicle targets for its main
//! gun (self-defence); without one, the gun lays on the line's middle,
//! `combat.suppress.aim_cm` above the ground there, and fires HE (on
//! `Auto` or `He`) at most every `combat.suppress.tank_he_s` while more
//! than `combat.suppress.tank_he_reserve` HE rounds are in the rack, never
//! within `he_clear` of own men. The MGs ignore soldiers and fire at
//! random points on the line (`Purpose::SUPPRESS`), the coax only within
//! its cone of the turret and the hull MG within its arc, each shot only
//! with a clear line to `combat.suppress.clear_cm` short of its aim point
//! (`crate::combat::suppress_aim`).
//!
//! Firing the main gun sets `since_fire` (spotting reveal) and an
//! `ArmourEventKind::Fired` event.

use frontline_content::vehicle::{MountKind, MountSetup, VehicleSetup};
use frontline_content::{ComponentKind, MatchSetup, SeatRole};

use crate::armour::{BoxPart, Face, deg};
use crate::combat::{Mark, interval_after, tri};
use crate::damage::ArmourEv;
use crate::ecs::{Entities, EntityKind, PlayerId, SimId};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{MatchState, Order};
use crate::hash::HashCache;
use crate::projectile::{NO_AMMO, SUBSTEP_HZ};
use crate::rng::{Purpose, RngStream};
use crate::snapshot::ArmourEventKind;
use crate::spatial::SpatialHash;
use crate::systems::FireRequest;
use crate::terrain::Heightfield;
use crate::vehicle::{AmmoChoice, Basis, BoxFrame, VehicleFrame, VehicleState};

// S3b: the gun elevation rate is each vehicle def's `turret.elevate_deg_s`;
// the coax cone is the rules' `vehicles.gunnery.coax_cone_deg`.
/// Most candidates a target search scores (nearest first).
const GROUP_CANDIDATES: usize = 16;
/// Most line-of-fire rays per target search.
const SEARCH_RAYS: usize = 2;
/// MGs pick among this many nearest soldiers (seeded), as S1 MGs.
const SWEEP_POOL: usize = 4;
/// RNG draws one MG shot takes from the `AIM` stream (`shot_velocity`: two
/// triangular rolls of two draws each).
const MG_AIM_DRAWS: usize = 4;
/// Shells live 1.25 × the gun's range in flight, plus two substeps.
const SHELL_TTL_PCT: i32 = 125;

/// 9.81 m/s².
const GRAVITY: Fx = Fx::from_ratio(981, 100);

/// The penetrator (first ammo entry) and HE (second) of a main gun; S3b:
/// a special penetrator (third, e.g. PzGr 40 APCR) on guns that carry one.
pub const AP: u8 = 0;
pub const HE: u8 = 1;
pub const SPECIAL: u8 = 2;

/// Direction of a gun laid at hull-relative `yaw` and `pitch`, in the hull
/// frame.
#[inline]
pub fn gun_local(yaw: Fx, pitch: Fx) -> FxVec3 {
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    FxVec3::new(cy * cp, sy * cp, sp)
}

/// World direction `d` as hull-relative `(yaw, pitch)`.
pub fn to_lay(hull: &Basis, d: FxVec3) -> (Fx, Fx) {
    let (x, y, z) = (hull.x.dot(d), hull.y.dot(d), hull.z.dot(d));
    let flat = FxVec2::new(x, y).length();
    (Fx::atan2(y, x), Fx::atan2(z, flat))
}

/// The main gun's muzzle in world space: the turret frame's mount point,
/// the barrel turned up by the gun's pitch about the mount's height.
pub fn main_muzzle(f: &VehicleFrame, vs: &VehicleState, m: &MountSetup) -> FxVec3 {
    let mz: FxVec3 = crate::vehicle::v3(m.muzzle);
    let trunnion = f.pivot + f.turret.axes.apply(FxVec3::new(Fx::ZERO, mz.y, mz.z));
    let dir = f.hull.axes.apply(gun_local(vs.turret_yaw, vs.gun_pitch));
    trunnion + dir * mz.x
}

/// An MG's muzzle in world space (coax: turret frame from the pivot; hull:
/// hull frame from the origin).
pub fn mg_muzzle(f: &VehicleFrame, m: &MountSetup) -> FxVec3 {
    let mz: FxVec3 = crate::vehicle::v3(m.muzzle);
    match m.kind {
        MountKind::Hull => f.origin + f.hull.axes.apply(mz),
        _ => f.pivot + f.turret.axes.apply(mz),
    }
}

/// Turns the turret and gun toward hull-relative `yaw` and `pitch` at the
/// vehicle's rates for one tick (§5.12: the one way a gun is aimed).
/// `pitch` is clamped to the elevation limits. Returns whether it is laid
/// (both reached).
pub fn lay_gun(vs: &mut VehicleState, v: &VehicleSetup, yaw: Fx, pitch: Fx, hz: u32) -> bool {
    let hz = hz.max(1) as i32;
    let ring = vs.component(ComponentKind::TurretRing);
    let mut traverse = deg(v.traverse_deg_s).div_int(hz);
    if ring == 0 {
        traverse = Fx::ZERO;
    } else if ring < 50 {
        traverse = traverse.div_int(2);
    }
    let [lo, hi] = v.main_gun().elevation_deg;
    let pitch = pitch.clamp(deg(lo), deg(hi));
    let err = (yaw.wrap_angle() - vs.turret_yaw).wrap_angle();
    let step = err.clamp(-traverse, traverse);
    vs.turret_yaw = (vs.turret_yaw + step).wrap_angle();
    let elev = deg(v.elevate_deg_s).div_int(hz);
    let perr = pitch - vs.gun_pitch;
    vs.gun_pitch += perr.clamp(-elev, elev);
    step == err && perr.abs() <= elev
}

/// Projected area of box `b` seen along unit `u` (a quarter of it:
/// `hy·hz·|u·x| + hx·hz·|u·y| + hx·hy·|u·z|`, dot products only).
fn projected_area(b: &BoxFrame, u: FxVec3) -> Fx {
    let h = b.half;
    h.y.mul(h.z).mul(b.axes.x.dot(u).abs())
        + h.x.mul(h.z).mul(b.axes.y.dot(u).abs())
        + h.x.mul(h.y).mul(b.axes.z.dot(u).abs())
}

/// The centre of `target`'s visible mass seen from `from` (S3b): the hull
/// and turret centres weighted by their projected areas toward the shooter.
/// Real gunners were taught to aim at the centre of the visible target.
pub fn mass_centre(from: FxVec3, target: &VehicleFrame) -> FxVec3 {
    let u = (target.hull.center - from).normalize_or_zero();
    let (h, t) = (&target.hull, &target.turret);
    let (ah, at) = (projected_area(h, u), projected_area(t, u));
    let sum = ah + at;
    if sum <= Fx::ZERO {
        return h.center;
    }
    // Relative to the hull centre, so the products stay small.
    h.center + (t.center - h.center) * at.div(sum)
}

/// Where to aim at `target` from `from` (S3b, see the module docs):
/// `Hull` = the centre of its visible mass ([`mass_centre`]) when that
/// point is in a clear line of fire; else `Turret` = the turret's centre
/// when that is (partial hull-down); else `None`. Launcher gunners use the
/// same rule.
pub fn aim_choice(from: FxVec3, target: &VehicleFrame, terrain: &Heightfield) -> Option<BoxPart> {
    if terrain.line_of_sight(from, mass_centre(from, target)) {
        Some(BoxPart::Hull)
    } else if terrain.line_of_sight(from, target.turret.center) {
        Some(BoxPart::Turret)
    } else {
        None
    }
}

/// The point a gunner at `from` aims at on `target` for an
/// [`aim_choice`] result (`turret`: the turret's centre; else the centre of
/// visible mass).
pub fn aim_at(from: FxVec3, target: &VehicleFrame, turret: bool) -> FxVec3 {
    if turret {
        target.turret.center
    } else {
        mass_centre(from, target)
    }
}

/// The gunner's estimate of a moving target's velocity (S3b lead
/// estimation error): `vel × (1 + e)`, `e` triangular within
/// ±`lead_error_pct`, shrunk like the spread by each ranging shot
/// (`ranging_step_pct` down to `ranging_floor_pct`). RNG `LEAD`, keyed by
/// the shooter, the target and the ranging count: the same target and
/// ranging step give the same estimate (no state).
#[allow(clippy::too_many_arguments)]
pub fn lead_estimate(
    vel: FxVec2,
    seed: u64,
    shooter: SimId,
    target: SimId,
    ranging: u8,
    error_pct: u32,
    step_pct: u32,
    floor_pct: u32,
) -> FxVec2 {
    if error_pct == 0 || vel == FxVec2::ZERO {
        return vel;
    }
    let shrink = (100 - step_pct as i64 * ranging as i64).max(floor_pct as i64);
    let span = Fx::from_int(error_pct as i32)
        .mul_int(shrink as i32)
        .div_int(10_000);
    let key = ((target.0 as u64) << 8) | ranging as u64;
    let mut r = RngStream::new(seed, key, shooter, Purpose::LEAD);
    let e = tri(&mut r, span);
    vel + vel * e
}

/// Drop-compensated, lead-corrected aim from `from` at a target at `at`
/// moving at `vel` (m/s), for a round at `speed`: two fixed iterations of
/// the flight time, then `combat::shot_velocity`'s drop term. Returns the
/// unit direction and the flight time.
pub fn solution(from: FxVec3, at: FxVec3, vel: FxVec2, speed: Fx) -> (FxVec3, Fx) {
    let speed = speed.max(Fx::ONE);
    let mut t = (at - from).length().div(speed);
    for _ in 0..2 {
        let lead = at + (vel * t).extend(Fx::ZERO);
        t = (lead - from).length().div(speed);
    }
    let mut d = at + (vel * t).extend(Fx::ZERO) - from;
    d.z += GRAVITY.mul(t).mul(t).div_int(2);
    (d.normalize_or_zero(), t)
}

/// Read-only inputs of the gunnery pass.
pub(crate) struct GunCtx<'a> {
    pub terrain: &'a Heightfield,
    /// This tick's soldiers (Weapons stage marks).
    pub marks: &'a [Mark],
    pub mark_of: &'a [u32],
    /// The soldier hash (soldiers where they now stand).
    pub soldiers: &'a SpatialHash,
    pub seed: u64,
    pub tick: u64,
}

#[inline]
fn range_sq(r: Fx) -> u64 {
    let r = r.raw().max(0) as u64;
    r * r
}

/// The main-gun target this tick, if any, and how to aim at it.
#[derive(Copy, Clone)]
struct Aim {
    point: FxVec3,
    vel: FxVec2,
    vehicle: bool,
}

impl MatchState {
    /// Vehicle gunnery for this tick (see the module docs). Fire requests
    /// go to `out` (sorted later with the soldiers'), events to `events`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn vehicle_gunnery(
        &mut self,
        ents: &mut Entities,
        ctx: &GunCtx,
        hash: &mut HashCache,
        out: &mut Vec<FireRequest>,
        events: &mut Vec<ArmourEv>,
    ) {
        if self.vehicles.is_empty() {
            return;
        }
        let ids: Vec<SimId> = self.vehicles.keys().copied().collect();
        for id in ids {
            if ents.kind[id.index()] != EntityKind::Vehicle {
                continue;
            }
            self.gun_one(ents, ctx, id, out, events);
            hash.mark_entity_chunk(id.chunk());
        }
    }

    fn gun_one(
        &mut self,
        ents: &Entities,
        ctx: &GunCtx,
        id: SimId,
        out: &mut Vec<FireRequest>,
        events: &mut Vec<ArmourEv>,
    ) {
        let i = id.index();
        let setup = &self.setup;
        let hz = setup.tick_hz.max(1);
        let owner = ents.owner[i];
        let Some(f) = self.vderived.frame(id).copied() else {
            return;
        };
        let order_retreat = ents.squad[i]
            .and_then(|s| self.squads.get(&s))
            .is_some_and(|sq| sq.order == Order::Retreat);
        let vs = self.vehicles.get(&id).expect("listed");
        let v = &setup.vehicles[vs.type_idx as usize];
        let main = v.main_gun();
        let gunner = vs.manned(v, SeatRole::Gunner);
        let gun_ok = gunner
            && vs.component(ComponentKind::Gun) > 0
            && vs.repairing.is_none()
            && !order_retreat;
        // S3b Suppress (module docs).
        let suppress = vs.suppress.filter(|_| !order_retreat);
        let ap_left = vs.loaded.is_some_and(|k| k != HE)
            || [AP, SPECIAL]
                .iter()
                .any(|&k| vs.ammo.get(k as usize).is_some_and(|n| *n > 0));

        // Re-target once a second (staggered), or when the target is gone.
        let valid = vs
            .target
            .and_then(|t| self.gun_target(ents, ctx, owner, &f, t, main, vs.aim_turret));
        let due = (ctx.tick + id.0 as u64).is_multiple_of(hz as u64);
        let lost = vs.target.is_some() && valid.is_none();
        let (target, aim_turret) = if lost || due {
            self.pick_target(
                ents,
                ctx,
                id,
                &f,
                gunner,
                gun_ok && ap_left,
                suppress.is_none(),
            )
        } else {
            (vs.target, vs.aim_turret)
        };
        let aim = target.and_then(|t| self.gun_target(ents, ctx, owner, &f, t, main, aim_turret));
        let target = aim.and(target);
        let sup = &setup.rules.combat.suppress;
        // Suppress: the HE aim point on the line's middle, when the gun has
        // no vehicle to fight.
        let area_probe = match (suppress, aim) {
            (Some(line), None) if matches!(vs.ammo_choice, AmmoChoice::Auto | AmmoChoice::He) => {
                let range: Fx = setup.weapons[main.weapon as usize].range_max.into();
                crate::combat::suppress_aim_at(
                    ctx.terrain,
                    &setup.rules.combat,
                    &line,
                    Fx::HALF,
                    f.turret.center,
                    range,
                    setup.rules.combat.suppress.tank_he_aim_cm,
                )
                .map(|(p, probe)| {
                    (
                        Aim {
                            point: p,
                            vel: FxVec2::ZERO,
                            vehicle: false,
                        },
                        probe,
                    )
                })
            }
            _ => None,
        };
        let area = area_probe.map(|(a, _)| a);

        let moving: Fx = setup.rules.vehicles.moving_speed.into();
        let target_moving = target
            .and_then(|t| self.vehicles.get(&t))
            .is_some_and(|t| t.avg_vel.length() > moving);
        let clear = aim
            .or(area)
            .is_none_or(|a| a.vehicle || !self.own_men_near(ents, ctx, owner, a.point));
        let special_better = match (aim, target) {
            (Some(a), Some(t)) if a.vehicle && main.ammo.len() > SPECIAL as usize => {
                self.special_better(&f, t, main, aim_turret)
            }
            _ => false,
        };
        let mut fired: Option<ArmourEv> = None;
        let vs = self.vehicles.get_mut(&id).expect("listed");
        if vs.target != target {
            vs.ranging = 0;
        }
        vs.target = target;
        vs.aim_turret = aim_turret;
        vs.since_fire = vs.since_fire.saturating_add(1);
        vs.reload = vs.reload.saturating_sub(1);

        // Ammo: the wanted round, swapping a wrong one for one reload.
        let has_special = vs.ammo.len() > SPECIAL as usize;
        let wanted = match vs.ammo_choice {
            AmmoChoice::Ap => AP,
            AmmoChoice::He => HE,
            AmmoChoice::Apcr if has_special => SPECIAL,
            AmmoChoice::Apcr => AP,
            AmmoChoice::Auto => match aim.or(area) {
                Some(a) if !a.vehicle => HE,
                _ if special_better => SPECIAL,
                _ => AP,
            },
        };
        // Auto: the first round in its preference order that is loaded or
        // left (a special round never goes at infantry).
        let wanted = if vs.ammo_choice == AmmoChoice::Auto {
            let order: &[u8] = match wanted {
                HE => &[HE, AP],
                SPECIAL => &[SPECIAL, AP, HE],
                _ => &[AP, SPECIAL, HE],
            };
            order
                .iter()
                .copied()
                .find(|&k| vs.loaded == Some(k) || vs.ammo.get(k as usize).is_some_and(|n| *n > 0))
                .unwrap_or(wanted)
        } else {
            wanted
        };
        let reload_ticks = reload_of(vs, v, setup);
        if vs.reload == 0 {
            match vs.loaded {
                Some(k)
                    if k != wanted
                        && (target.is_some() || area.is_some())
                        && vs.ammo.get(wanted as usize).is_some_and(|n| *n > 0) =>
                {
                    vs.ammo[k as usize] = vs.ammo[k as usize].saturating_add(1);
                    vs.loaded = None;
                    vs.reload = reload_ticks;
                }
                None => {
                    if let Some(n) = vs.ammo.get_mut(wanted as usize)
                        && *n > 0
                    {
                        *n -= 1;
                        vs.loaded = Some(wanted);
                    }
                }
                _ => {}
            }
        }

        // Laying: on the solution, or back to the front without a target.
        let round = vs
            .loaded
            .or(Some(wanted))
            .and_then(|k| main.ammo.get(k as usize))
            .map(|a| a.ammo);
        let speed: Fx = round
            .and_then(|a| setup.ammo.get(a as usize))
            .map_or(setup.weapons[main.weapon as usize].muzzle_speed, |a| {
                a.speed
            })
            .into();
        let muzzle = main_muzzle(&f, vs, main);
        let gr = &setup.rules.vehicles;
        let lead = |a: &Aim| match target {
            Some(t) if a.vehicle && target_moving => lead_estimate(
                a.vel,
                ctx.seed,
                id,
                t,
                vs.ranging,
                gr.gunnery.lead_error_pct,
                gr.ranging_step_pct,
                gr.ranging_floor_pct,
            ),
            _ => a.vel,
        };
        let (want_yaw, want_pitch) = match aim.or(area) {
            Some(a) => to_lay(&f.hull.axes, solution(muzzle, a.point, lead(&a), speed).0),
            None => (Fx::ZERO, Fx::ZERO),
        };
        // Suppress: HE at the line only at its interval, with the reserve
        // kept, and with the line of fire reaching the line.
        let area_ok = aim.is_none()
            && area.is_some_and(|a| {
                vs.since_fire as u32 >= sup.tank_he_s * hz
                    && vs
                        .ammo
                        .get(HE as usize)
                        .is_some_and(|&n| n as u32 > sup.tank_he_reserve)
                    && area_probe.is_some_and(|(_, probe)| ctx.terrain.line_of_sight(muzzle, probe))
                    && a.point != muzzle
            });
        if gunner {
            lay_gun(vs, v, want_yaw, want_pitch, hz);
        }

        // Fire the main gun when laid within the spread.
        if let (Some(a), Some(k), true) = (aim.or(area.filter(|_| area_ok)), vs.loaded, gun_ok)
            && vs.reload == 0
            && !(a.vehicle && k == HE && vs.ammo_choice == AmmoChoice::Auto)
            && (aim.is_some() || k == HE)
        {
            let spread = main_spread(vs, v, setup, &a, k, target_moving);
            let [lo, hi] = main.elevation_deg;
            let laid_yaw = (want_yaw - vs.turret_yaw).wrap_angle().abs() <= spread;
            let want_pitch = want_pitch.clamp(deg(lo), deg(hi));
            let laid_pitch = (want_pitch - vs.gun_pitch).abs() <= spread;
            if laid_yaw && laid_pitch && clear {
                let muzzle = main_muzzle(&f, vs, main);
                let dir = f.hull.axes.apply(gun_local(vs.turret_yaw, vs.gun_pitch));
                let mut rng = RngStream::new(ctx.seed, ctx.tick, id, Purpose::GUN_SPREAD);
                let right = FxVec3::new(-dir.y, dir.x, Fx::ZERO).normalize_or_zero();
                let up = FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE);
                let shot = (dir + right * tri(&mut rng, spread) + up * tri(&mut rng, spread))
                    .normalize_or_zero();
                let ammo = main.ammo[k as usize].ammo;
                let w = &setup.weapons[main.weapon as usize];
                let ttl = Fx::from(w.range_max)
                    .mul_int(SHELL_TTL_PCT)
                    .div_int(100)
                    .div(speed)
                    .mul_int(SUBSTEP_HZ)
                    .floor_int()
                    + 2;
                out.push(FireRequest {
                    shooter: id,
                    seq: 0,
                    pos: muzzle,
                    vel: shot * speed,
                    weapon: main.weapon,
                    player: owner.0,
                    ttl: ttl.clamp(2, u16::MAX as i32) as u16,
                    he: true,
                    ammo,
                });
                vs.loaded = None;
                vs.reload = reload_ticks;
                vs.since_fire = 0;
                vs.ranging = vs.ranging.saturating_add(1);
                let kind = match k {
                    AP => 1,
                    HE => 2,
                    _ => 3,
                };
                fired = Some(
                    ArmourEv::new(ArmourEventKind::Fired, id, owner.0, owner.0)
                        .component(kind)
                        .ammo(ammo)
                        .at(muzzle),
                );
            }
        }

        // MGs.
        let vs = self.vehicles.get(&id).expect("listed");
        let main_man = match aim {
            Some(a) if !a.vehicle => target,
            _ => None,
        };
        let moving = vs.speed.abs() > moving;
        let turret_dir = f.hull.axes.apply(gun_local(vs.turret_yaw, Fx::ZERO));
        let mut mgs = vs.mgs.clone();
        for (k, (m, st)) in v.mounts.iter().skip(1).zip(mgs.iter_mut()).enumerate() {
            st.cooldown = st.cooldown.saturating_sub(1);
            let (crew, cone, axis) = match m.kind {
                MountKind::Coax => (
                    SeatRole::Gunner,
                    deg(self.setup.rules.vehicles.gunnery.coax_cone_deg as i32),
                    turret_dir,
                ),
                MountKind::Hull => (m.seat, deg(m.arc_deg), f.hull.axes.x),
                MountKind::TurretMain => continue,
            };
            let seat_ok = vs.manned(v, crew);
            if !seat_ok {
                st.target = None;
                continue;
            }
            let w = &setup.weapons[m.weapon as usize];
            let from = mg_muzzle(&f, m);
            let r = range_sq(w.range_max.into());
            let axis_yaw = Fx::atan2(axis.y, axis.x);
            // Suppress: fire at the line instead (module docs).
            if let Some(line) = suppress {
                st.target = None;
                if st.cooldown > 0 {
                    continue;
                }
                let mut pick = RngStream::new(ctx.seed, ctx.tick, id, Purpose::SUPPRESS);
                for _ in 0..k * 2 {
                    pick.next_u64();
                }
                let t = pick.range_fx(Fx::ZERO, Fx::ONE);
                let Some((aim, probe)) = crate::combat::suppress_aim(
                    ctx.terrain,
                    &setup.rules.combat,
                    &line,
                    t,
                    from,
                    w.range_max.into(),
                ) else {
                    continue;
                };
                if !in_cone(axis_yaw, aim.xy() - from.xy(), cone)
                    || !ctx.terrain.line_of_sight(from, probe)
                {
                    continue;
                }
                mg_fire(
                    ctx, vs, v, setup, id, owner, k, m, st, crew, from, aim, moving, out,
                );
                continue;
            }
            let ok = |t: SimId| self.mg_target_ok(ctx, owner, from, axis_yaw, cone, r, t);
            let lost = st.target.is_some_and(|t| !ok(t));
            if lost {
                st.target = None;
            }
            if m.kind == MountKind::Coax && main_man.is_some_and(ok) {
                st.target = main_man;
            } else if lost || due {
                // Draw k of the stream: each MG its own roll.
                let mut sweep = RngStream::new(ctx.seed, ctx.tick, id, Purpose::SWEEP);
                for _ in 0..k {
                    sweep.next_u64();
                }
                let roll = sweep.next_u64();
                st.target = self.mg_search(ctx, owner, from, axis_yaw, cone, r, roll);
            }
            let Some(t) = st.target else { continue };
            if st.cooldown > 0 {
                continue;
            }
            let aim = ctx.marks[ctx.mark_of[t.index()] as usize].aim;
            mg_fire(
                ctx, vs, v, setup, id, owner, k, m, st, crew, from, aim, moving, out,
            );
        }
        self.vehicles.get_mut(&id).expect("listed").mgs = mgs;
        if let Some(ev) = fired {
            self.push_event(events, ev);
        }
    }

    /// S3b, `Auto`: whether the special penetrator should go at vehicle `t`
    /// (aimed at its turret or visible-mass centre from frame `f`): the
    /// standard round's penetration chance against the plate facing the
    /// shooter is under the rules' `special_below_pct` and the special
    /// round's is higher.
    fn special_better(&self, f: &VehicleFrame, t: SimId, main: &MountSetup, turret: bool) -> bool {
        let (Some(tf), Some(ap), Some(sp)) = (
            self.vderived.frame(t),
            main.ammo.get(AP as usize),
            main.ammo.get(SPECIAL as usize),
        ) else {
            return false;
        };
        let (Some(ap), Some(sp)) = (
            self.armour.ammo.get(ap.ammo as usize),
            self.armour.ammo.get(sp.ammo as usize),
        ) else {
            return false;
        };
        let part = if turret {
            BoxPart::Turret
        } else {
            BoxPart::Hull
        };
        let b = if turret { &tf.turret } else { &tf.hull };
        let from = f.turret.center;
        let rel = b.center - from;
        let range = rel.length();
        let dir = rel.normalize_or_zero();
        let l = crate::armour::to_local(b, -dir);
        let face = if l.x.abs() >= l.y.abs() {
            if l.x >= Fx::ZERO {
                Face::Front
            } else {
                Face::Rear
            }
        } else if l.y >= Fx::ZERO {
            Face::Left
        } else {
            Face::Right
        };
        let plates = &self.armour.vehicles[tf.type_idx as usize];
        let plate = plates.face(part, face);
        let c = crate::armour::impact_cos(dir, b.axes.apply(plate.normal));
        let chance = |a: &crate::armour::AmmoTable| {
            crate::armour::pen_chance_ppm(
                a.pen0(range),
                c,
                plate.thickness,
                Fx::ZERO,
                a.cos_ricochet,
            )
        };
        let (pa, ps) = (chance(ap), chance(sp));
        let below = self.setup.rules.vehicles.gunnery.special_below_pct;
        (pa as u64) * 100 < below as u64 * crate::armour::PPM as u64 && ps > pa
    }

    /// Whether `t` is still a valid main-gun target of a vehicle of `owner`
    /// at frame `f`: a live, seen enemy vehicle or soldier in range. Its
    /// aim point (a vehicle's turret or hull centre) and velocity.
    #[allow(clippy::too_many_arguments)]
    fn gun_target(
        &self,
        ents: &Entities,
        ctx: &GunCtx,
        owner: PlayerId,
        f: &VehicleFrame,
        t: SimId,
        main: &MountSetup,
        aim_turret: bool,
    ) -> Option<Aim> {
        let k = *ents.kind.get(t.index())?;
        if ents.owner[t.index()] == owner {
            return None;
        }
        let r = range_sq(self.setup.weapons[main.weapon as usize].range_max.into());
        let seen = ents.combat[t.index()].seen_by & (1 << owner.0) != 0;
        if !seen {
            return None;
        }
        match k {
            EntityKind::Vehicle => {
                let tf = self.vderived.frame(t)?;
                if (tf.origin.xy() - f.origin.xy()).length_sq_raw() > r {
                    return None;
                }
                let point = aim_at(f.turret.center, tf, aim_turret);
                Some(Aim {
                    point,
                    vel: self.vehicles.get(&t)?.avg_vel,
                    vehicle: true,
                })
            }
            EntityKind::Soldier => {
                let m = ctx.marks.get(*ctx.mark_of.get(t.index())? as usize)?;
                if (m.pos.xy() - f.origin.xy()).length_sq_raw() > r {
                    return None;
                }
                // Half a metre up, or, for a man in a trench, the head and
                // shoulders the shooters aim at: his feet are below the
                // ground, and a line to them meets the ground well short.
                let point = if ents.combat[t.index()].in_trench {
                    m.aim
                } else {
                    m.pos + FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::HALF)
                };
                Some(Aim {
                    point,
                    vel: ents.velocity[t.index()].lin,
                    vehicle: false,
                })
            }
            _ => None,
        }
    }

    /// The main gun's next target (see the module docs): `(target, aim at
    /// the turret)`. `infantry_ok` is false on a Suppress order (S3b).
    #[allow(clippy::too_many_arguments)]
    fn pick_target(
        &self,
        ents: &Entities,
        ctx: &GunCtx,
        id: SimId,
        f: &VehicleFrame,
        gunner: bool,
        armour_ok: bool,
        infantry_ok: bool,
    ) -> (Option<SimId>, bool) {
        if !gunner {
            return (None, false);
        }
        let setup = &self.setup;
        let i = id.index();
        let owner = ents.owner[i];
        let vs = &self.vehicles[&id];
        let v = &setup.vehicles[vs.type_idx as usize];
        let main = v.main_gun();
        let w = &setup.weapons[main.weapon as usize];
        let r = range_sq(w.range_max.into());
        let from = main_muzzle(f, vs, main);
        // Vehicles: the ordered one first, then the nearest.
        if armour_ok {
            let mut cands: Vec<(u64, SimId)> = Vec::new();
            for (&t, ts) in &self.vehicles {
                let ti = t.index();
                if ts.wreck_tick.is_some()
                    || ents.kind[ti] != EntityKind::Vehicle
                    || ents.owner[ti] == owner
                    || ents.combat[ti].seen_by & (1 << owner.0) == 0
                {
                    continue;
                }
                let d = (ents.transform[ti].pos.xy() - f.origin.xy()).length_sq_raw();
                if d > r {
                    continue;
                }
                let key = if vs.attack == Some(t) { 0 } else { d.max(1) };
                cands.push((key, t));
            }
            cands.sort_unstable();
            for &(_, t) in cands.iter().take(SEARCH_RAYS) {
                let Some(tf) = self.vderived.frame(t) else {
                    continue;
                };
                if let Some(part) = aim_choice(from, tf, ctx.terrain) {
                    return (Some(t), part == BoxPart::Turret);
                }
            }
        }
        if !infantry_ok {
            return (None, false);
        }
        // Infantry: the best group (more men near, then closer).
        let blast_r = Fx::from_int(setup.rules.vehicles.gunnery.he_group_m as i32);
        let mut near: Vec<(u64, u32)> = Vec::new();
        for (k, m) in ctx.marks.iter().enumerate() {
            if m.owner == owner.0 || m.seen_by & (1 << owner.0) == 0 {
                continue;
            }
            let d = (m.pos.xy() - f.origin.xy()).length_sq_raw();
            if d <= r {
                near.push((d, k as u32));
            }
        }
        if near.is_empty() {
            return (None, false);
        }
        near.sort_unstable();
        let br = range_sq(blast_r);
        let mut scored: Vec<(i64, u64, SimId, FxVec3)> = near
            .iter()
            .take(GROUP_CANDIDATES)
            .map(|&(d, k)| {
                let m = &ctx.marks[k as usize];
                let men = near
                    .iter()
                    .filter(|&&(_, j)| {
                        (ctx.marks[j as usize].pos.xy() - m.pos.xy()).length_sq_raw() <= br
                    })
                    .count() as i64;
                (-men, d, m.id, m.aim)
            })
            .collect();
        scored.sort_unstable_by_key(|&(men, d, id, _)| (men, d, id));
        let mut rays = 0;
        for &(_, _, t, aim) in &scored {
            if self.own_men_near(ents, ctx, owner, aim) {
                continue;
            }
            if rays >= SEARCH_RAYS {
                break;
            }
            rays += 1;
            if ctx.terrain.line_of_sight(from, aim) {
                return (Some(t), false);
            }
        }
        (None, false)
    }

    /// Whether own soldiers stand within the rules' `he_clear` of `p`.
    fn own_men_near(&self, ents: &Entities, ctx: &GunCtx, owner: PlayerId, p: FxVec3) -> bool {
        let clear: Fx = self.setup.rules.vehicles.he_clear.into();
        let r = range_sq(clear);
        let mut found = false;
        ctx.soldiers.for_each_near(p.xy(), clear, |s| {
            let i = s.index();
            if !found
                && ents.kind[i] == EntityKind::Soldier
                && ents.owner[i] == owner
                && (ents.transform[i].pos.xy() - p.xy()).length_sq_raw() <= r
            {
                found = true;
            }
        });
        found
    }

    /// Whether soldier `t` is a valid MG target from `from`: a live, seen
    /// enemy within `r` (Q32) and within `cone` of `axis`.
    #[allow(clippy::too_many_arguments)]
    fn mg_target_ok(
        &self,
        ctx: &GunCtx,
        owner: PlayerId,
        from: FxVec3,
        axis_yaw: Fx,
        cone: Fx,
        r: u64,
        t: SimId,
    ) -> bool {
        let Some(&k) = ctx.mark_of.get(t.index()) else {
            return false;
        };
        let Some(m) = ctx.marks.get(k as usize) else {
            return false;
        };
        m.owner != owner.0
            && m.seen_by & (1 << owner.0) != 0
            && (m.pos.xy() - from.xy()).length_sq_raw() <= r
            && in_cone(axis_yaw, m.pos.xy() - from.xy(), cone)
    }

    /// An MG's target search: among the [`SWEEP_POOL`] nearest valid
    /// soldiers, from a seeded start, the first with a clear line (at most
    /// [`SEARCH_RAYS`] rays).
    #[allow(clippy::too_many_arguments)]
    fn mg_search(
        &self,
        ctx: &GunCtx,
        owner: PlayerId,
        from: FxVec3,
        axis_yaw: Fx,
        cone: Fx,
        r: u64,
        roll: u64,
    ) -> Option<SimId> {
        let mut cands: Vec<(u64, SimId)> = ctx
            .marks
            .iter()
            .filter(|m| self.mg_target_ok(ctx, owner, from, axis_yaw, cone, r, m.id))
            .map(|m| ((m.pos.xy() - from.xy()).length_sq_raw(), m.id))
            .collect();
        cands.sort_unstable();
        cands.truncate(SWEEP_POOL);
        let n = cands.len();
        if n == 0 {
            return None;
        }
        let rot = ((roll >> 32) as usize) % n;
        (0..n.min(SEARCH_RAYS)).find_map(|k| {
            let (_, t) = cands[(rot + k) % n];
            let aim = ctx.marks[ctx.mark_of[t.index()] as usize].aim;
            ctx.terrain.line_of_sight(from, aim).then_some(t)
        })
    }
}

/// One MG shot of mount `k` (after the main gun) at `aim`: the S1 spread
/// (moving, range; ×1.5 with its crewman wounded), its own `AIM` draws,
/// belts and reloads.
#[allow(clippy::too_many_arguments)]
fn mg_fire(
    ctx: &GunCtx,
    vs: &VehicleState,
    v: &VehicleSetup,
    setup: &MatchSetup,
    id: SimId,
    owner: PlayerId,
    k: usize,
    m: &MountSetup,
    st: &mut crate::vehicle::MgState,
    crew: SeatRole,
    from: FxVec3,
    aim: FxVec3,
    moving: bool,
    out: &mut Vec<FireRequest>,
) {
    let w = &setup.weapons[m.weapon as usize];
    let mut pct: i64 = 100;
    if moving {
        pct = pct * setup.rules.combat.moving_spread_pct as i64 / 100;
    }
    let hp = vs.seat(v, crew).map_or(0, |s| s.hp as u32);
    if hp < setup.rules.vehicles.wounded_hp {
        pct = pct * 3 / 2;
    }
    let dist = (aim - from).length();
    let extra = setup.rules.combat.range_spread_pct as i64 * dist.raw().max(0) as i64
        / Fx::from(w.range_max).raw().max(1) as i64;
    pct = pct * (100 + extra) / 100;
    let spread =
        Fx::from_raw((Fx::from(w.spread).raw() as i64 * pct / 100).min(i32::MAX as i64) as i32);
    let mut rng = RngStream::new(ctx.seed, ctx.tick, id, Purpose::AIM);
    // Each MG its own draws: skip the other mounts' (a shot takes
    // `MG_AIM_DRAWS`).
    for _ in 0..k * MG_AIM_DRAWS {
        rng.next_u64();
    }
    let vel = crate::combat::shot_velocity(from, aim, w.muzzle_speed.into(), spread, &mut rng);
    out.push(FireRequest {
        shooter: id,
        seq: 1 + k as u16,
        pos: from,
        vel,
        weapon: m.weapon,
        player: owner.0,
        ttl: crate::combat::round_ttl(w),
        he: false,
        ammo: NO_AMMO,
    });
    st.shots = st.shots.wrapping_add(1);
    st.belt = st.belt.saturating_add(1);
    if st.belt as u32 >= w.magazine.max(1) {
        st.belt = 0;
        st.cooldown = w
            .reload_ticks
            .max(w.interval_milli.div_ceil(1000))
            .min(u16::MAX as u32) as u16;
    } else {
        st.cooldown = interval_after(w, st.shots).min(u16::MAX as u32) as u16;
    }
}

/// Whether the horizontal direction `d` lies within `cone` of heading
/// `axis_yaw`.
fn in_cone(axis_yaw: Fx, d: FxVec2, cone: Fx) -> bool {
    (Fx::atan2(d.y, d.x) - axis_yaw).wrap_angle().abs() <= cone
}

/// The main gun's reload in ticks with this crew: ×1.5 with the loader
/// wounded, ×2 with his seat empty (the gunner loads).
pub fn reload_of(vs: &VehicleState, v: &VehicleSetup, s: &MatchSetup) -> u16 {
    let base = s.weapons[v.main_gun().weapon as usize].reload_ticks;
    let pct = match vs.seat(v, SeatRole::Loader) {
        None => 100,
        Some(st) if st.hp == 0 || st.swap > 0 => 200,
        Some(st) if (st.hp as u32) < s.rules.vehicles.wounded_hp => 150,
        Some(_) => 100,
    };
    (base * pct / 100).clamp(1, u16::MAX as u32) as u16
}

/// The main gun's spread (radians) for this shot (see the module docs).
fn main_spread(
    vs: &VehicleState,
    v: &VehicleSetup,
    s: &MatchSetup,
    a: &Aim,
    round: u8,
    target_moving: bool,
) -> Fx {
    let r = &s.rules.vehicles;
    let w = &s.weapons[v.main_gun().weapon as usize];
    let base: Fx = if !a.vehicle && round == HE {
        r.he_area_spread.into()
    } else {
        w.spread.into()
    };
    let mut pct: i64 = 100;
    let step = r.ranging_step_pct as i64 * vs.ranging as i64;
    pct = pct * (100 - step).max(r.ranging_floor_pct as i64) / 100;
    let moving: Fx = r.moving_speed.into();
    if vs.speed.abs() > moving {
        pct = pct * v.mobility.moving_spread_pct as i64 / 100;
    }
    if a.vehicle && target_moving {
        pct = pct * r.moving_target_spread_pct as i64 / 100;
    }
    if vs
        .seat(v, SeatRole::Gunner)
        .is_some_and(|st| (st.hp as u32) < r.wounded_hp)
    {
        pct = pct * 3 / 2;
    }
    if vs.component(ComponentKind::Gun) < 50 {
        pct *= 2;
    }
    Fx::from_raw((base.raw() as i64 * pct / 100).clamp(0, i32::MAX as i64) as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Review fix: the MG skip count matches what one shot draws, so two
    /// MGs firing on the same tick never share a spread roll.
    #[test]
    fn one_mg_shot_takes_mg_aim_draws() {
        let key = (7, 100, SimId(3), Purpose::AIM);
        let mut shot = RngStream::new(key.0, key.1, key.2, key.3);
        crate::combat::shot_velocity(
            FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE),
            FxVec3::new(Fx::from_int(100), Fx::from_int(20), Fx::ONE),
            Fx::from_int(700),
            Fx::from_milli(5),
            &mut shot,
        );
        let mut skip = RngStream::new(key.0, key.1, key.2, key.3);
        for _ in 0..MG_AIM_DRAWS {
            skip.next_u64();
        }
        assert_eq!(shot.next_u64(), skip.next_u64());
    }

    fn setup() -> MatchSetup {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
        let defs = frontline_content::ModDefs::load(&root).unwrap();
        let req = frontline_content::MatchRequest::skirmish(
            1,
            "ww2:germany",
            "ww2:usa",
            frontline_content::Difficulty::Normal,
        );
        defs.resolve(&req).unwrap()
    }

    #[test]
    fn crew_and_gun_damage_scale_reload_and_spread() {
        let s = setup();
        let v = &s.vehicles[0];
        let mut vs = VehicleState::new(0, v, &s.rules.vehicles);
        let base = s.weapons[v.main_gun().weapon as usize].reload_ticks;
        assert_eq!(base, 195, "6.5 s at 30 Hz");
        let loader = v
            .seats
            .iter()
            .position(|x| x.role == SeatRole::Loader)
            .unwrap();
        let gunner = v
            .seats
            .iter()
            .position(|x| x.role == SeatRole::Gunner)
            .unwrap();
        assert_eq!(reload_of(&vs, v, &s) as u32, base);
        vs.seats[loader].hp = 40;
        assert_eq!(reload_of(&vs, v, &s) as u32, base * 3 / 2);
        vs.seats[loader].hp = 0;
        assert_eq!(reload_of(&vs, v, &s) as u32, base * 2);

        let at_tank = Aim {
            point: FxVec3::ZERO,
            vel: FxVec2::ZERO,
            vehicle: true,
        };
        let at_men = Aim {
            vehicle: false,
            ..at_tank
        };
        let vs = VehicleState::new(0, v, &s.rules.vehicles);
        let sp = |vs: &VehicleState, a: &Aim, k: u8, moving: bool| {
            main_spread(vs, v, &s, a, k, moving).raw()
        };
        let gun = Fx::from(s.weapons[v.main_gun().weapon as usize].spread).raw();
        assert_eq!(sp(&vs, &at_tank, AP, false), gun, "2.5 mrad");
        // HE at men: the area spread (6 mrad).
        assert_eq!(
            sp(&vs, &at_men, HE, false),
            Fx::from(s.rules.vehicles.he_area_spread).raw()
        );
        // A moving target × moving_target_spread_pct (S3b Tune: 600 %);
        // ranging in 20 % per shot down to 50 %.
        let mts = s.rules.vehicles.moving_target_spread_pct as i32;
        assert_eq!(sp(&vs, &at_tank, AP, true), gun * mts / 100);
        let mut r = vs.clone();
        r.ranging = 1;
        assert_eq!(sp(&r, &at_tank, AP, false), gun * 80 / 100);
        r.ranging = 9;
        assert_eq!(sp(&r, &at_tank, AP, false), gun / 2);
        // Own movement ×4 (Panzer IV), gunner wounded ×1.5, gun damaged ×2.
        let mut m = vs.clone();
        m.speed = Fx::from_int(3);
        assert_eq!(sp(&m, &at_tank, AP, false), gun * 4);
        let mut g = vs.clone();
        g.seats[gunner].hp = 30;
        assert_eq!(sp(&g, &at_tank, AP, false), gun * 3 / 2);
        g.seats[gunner].hp = 100;
        g.components[ComponentKind::Gun.index()] = 40;
        assert_eq!(sp(&g, &at_tank, AP, false), gun * 2);
    }

    #[test]
    fn to_lay_inverts_gun_local() {
        let hull = Basis::hull(deg(30), deg(5), deg(-3));
        for (y, p) in [(0, 0), (45, 10), (-120, -5), (170, 15)] {
            let d = hull.apply(gun_local(deg(y), deg(p)));
            let (ly, lp) = to_lay(&hull, d);
            assert!((ly - deg(y)).wrap_angle().abs() < Fx::from_milli(2), "{y}");
            assert!((lp - deg(p)).abs() < Fx::from_milli(2), "{p}");
        }
    }
}
