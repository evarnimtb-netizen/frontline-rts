//! Vehicle gunnery (S3, `docs/slices/S3-armour.md` "Gunnery"): the
//! Weapons stage's vehicle part, serial in ascending vehicle id after the
//! soldiers' parallel aim and fire.
//!
//! **Targets.** Once a second (staggered by id), or when the target is
//! gone, the gunner picks: the `AttackUnit` vehicle while it is seen and in
//! range; else the nearest seen enemy vehicle within the main gun's
//! `range_max` that has a visible part; else the best seen infantry group
//! in range (more men within the HE damage radius first, then closer), never
//! within `he_clear` of the own men. A vehicle target gets the aim part
//! from [`aim_choice`]: the visible part (hull or turret) with the better
//! penetration chance for the vehicle's penetrator against the zone that
//! faces the shooter, the hull unless the turret is better by more than
//! `aim_hull_margin_pct` points. Retreating vehicles do not use the main
//! gun.
//!
//! **Ammo.** `Auto` wants the penetrator against vehicles and HE against
//! men; `Ap` and `He` force one. A wrong round in the breech is unloaded
//! (back to the rack) and the wanted one loaded, at the cost of one
//! reload; an empty breech loads when the reload is done. The reload is the
//! gun's, ×1.5 with the loader wounded and ×2 with him gone.
//!
//! **Laying.** The solution leads the target by its velocity (a vehicle's
//! ~1 s average, a soldier's current) times the flight time and
//! compensates drop as `combat::shot_velocity` does, two fixed iterations.
//! [`lay_gun`] turns the turret at the def's traverse (×0.5 with the ring
//! damaged, fixed with it destroyed) and the gun at [`ELEVATE_DEG_S`]
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
//! nearest seen enemy soldier within [`COAX_CONE_DEG`] of the turret; the
//! hull MG the nearest seen enemy soldiers within its `arc_deg` (sweeping
//! among the four nearest, as S1 MGs). Both need their crewman (gunner,
//! radio operator), fire at the MG's interval with belts and reloads, and
//! go through the S1 spread (moving, range). Vehicles are never suppressed.
//!
//! Firing the main gun sets `since_fire` (spotting reveal) and an
//! `ArmourEventKind::Fired` event.

use frontline_content::vehicle::{MountKind, MountSetup, VehicleSetup};
use frontline_content::{ComponentKind, MatchSetup, SeatRole};

use crate::armour::{self, AmmoTable, BoxPart, Face, VehicleArmour, deg};
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
use crate::vehicle::{AmmoChoice, Basis, VehicleFrame, VehicleState};

/// Gun elevation rate (°/s); the defs have no figure for it.
pub const ELEVATE_DEG_S: i32 = 6;
/// The coax engages men this close to the turret's line (°).
pub const COAX_CONE_DEG: i32 = 5;
/// Most candidates a target search scores (nearest first).
const GROUP_CANDIDATES: usize = 16;
/// Most line-of-fire rays per target search.
const SEARCH_RAYS: usize = 2;
/// MGs pick among this many nearest soldiers (seeded), as S1 MGs.
const SWEEP_POOL: usize = 4;
/// Shells live 1.25 × the gun's range in flight, plus two substeps.
const SHELL_TTL_PCT: i32 = 125;

/// 9.81 m/s².
const GRAVITY: Fx = Fx::from_ratio(981, 100);

/// The penetrator (first ammo entry) and HE (second) of a main gun.
pub const AP: u8 = 0;
pub const HE: u8 = 1;

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
    let elev = deg(ELEVATE_DEG_S).div_int(hz);
    let perr = pitch - vs.gun_pitch;
    vs.gun_pitch += perr.clamp(-elev, elev);
    step == err && perr.abs() <= elev
}

/// Which part of `target` to aim at from `from` (see the module docs):
/// `None` when neither part's centre is in a clear line of fire.
pub fn aim_choice(
    from: FxVec3,
    target: &VehicleFrame,
    plates: &VehicleArmour,
    ap: &AmmoTable,
    terrain: &Heightfield,
    margin_pct: u32,
) -> Option<BoxPart> {
    let chance = |part: BoxPart| {
        let b = if part == BoxPart::Hull {
            &target.hull
        } else {
            &target.turret
        };
        if !terrain.line_of_sight(from, b.center) {
            return None;
        }
        let rel = b.center - from;
        let range = rel.length();
        let dir = rel.normalize_or_zero();
        // The face toward the shooter: the side axis the line meets most.
        let l = armour::to_local(b, -dir);
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
        let plate = plates.face(part, face);
        let c = armour::impact_cos(dir, b.axes.apply(plate.normal));
        Some(armour::pen_chance_ppm(
            ap.pen0(range),
            c,
            plate.thickness,
            Fx::ZERO,
            ap.cos_ricochet,
        ))
    };
    match (chance(BoxPart::Hull), chance(BoxPart::Turret)) {
        (None, None) => None,
        (Some(_), None) => Some(BoxPart::Hull),
        (None, Some(_)) => Some(BoxPart::Turret),
        (Some(h), Some(t)) => {
            let margin = margin_pct * (armour::PPM / 100);
            Some(if t > h.saturating_add(margin) {
                BoxPart::Turret
            } else {
                BoxPart::Hull
            })
        }
    }
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
        let ap_left = vs.loaded == Some(AP) || vs.ammo.first().is_some_and(|n| *n > 0);

        // Re-target once a second (staggered), or when the target is gone.
        let valid = vs
            .target
            .and_then(|t| self.gun_target(ents, ctx, owner, &f, t, main, vs.aim_turret));
        let due = (ctx.tick + id.0 as u64).is_multiple_of(hz as u64);
        let (target, aim_turret) = if valid.is_none() || due {
            self.pick_target(ents, ctx, id, &f, gunner, gun_ok && ap_left)
        } else {
            (vs.target, vs.aim_turret)
        };
        let aim = target.and_then(|t| self.gun_target(ents, ctx, owner, &f, t, main, aim_turret));
        let target = aim.and(target);

        let moving: Fx = setup.rules.vehicles.moving_speed.into();
        let target_moving = target
            .and_then(|t| self.vehicles.get(&t))
            .is_some_and(|t| t.avg_vel.length() > moving);
        let clear = aim.is_none_or(|a| a.vehicle || !self.own_men_near(ents, ctx, owner, a.point));
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
        let wanted = match vs.ammo_choice {
            AmmoChoice::Ap => AP,
            AmmoChoice::He => HE,
            AmmoChoice::Auto => match aim {
                Some(a) if !a.vehicle => HE,
                _ => AP,
            },
        };
        let wanted = if vs.ammo_choice == AmmoChoice::Auto
            && vs.ammo.get(wanted as usize).is_none_or(|n| *n == 0)
            && vs.loaded != Some(wanted)
        {
            1 - wanted
        } else {
            wanted
        };
        let reload_ticks = reload_of(vs, v, setup);
        if vs.reload == 0 {
            match vs.loaded {
                Some(k)
                    if k != wanted
                        && target.is_some()
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
        let (want_yaw, want_pitch) = match aim {
            Some(a) => to_lay(&f.hull.axes, solution(muzzle, a.point, a.vel, speed).0),
            None => (Fx::ZERO, Fx::ZERO),
        };
        if gunner {
            lay_gun(vs, v, want_yaw, want_pitch, hz);
        }

        // Fire the main gun when laid within the spread.
        if let (Some(a), Some(k), true) = (aim, vs.loaded, gun_ok)
            && vs.reload == 0
            && !(a.vehicle && k == HE && vs.ammo_choice == AmmoChoice::Auto)
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
                let kind = if k == AP { 1 } else { 2 };
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
                MountKind::Coax => (SeatRole::Gunner, deg(COAX_CONE_DEG), turret_dir),
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
            let ok = |t: SimId| self.mg_target_ok(ctx, owner, from, axis, cone, r, t);
            if st.target.is_some_and(|t| !ok(t)) {
                st.target = None;
            }
            if m.kind == MountKind::Coax && main_man.is_some_and(ok) {
                st.target = main_man;
            } else if st.target.is_none() || due {
                let roll = RngStream::new(ctx.seed, ctx.tick, id, Purpose::SWEEP).next_u64();
                st.target = self.mg_search(ctx, owner, from, axis, cone, r, roll);
            }
            let Some(t) = st.target else { continue };
            if st.cooldown > 0 {
                continue;
            }
            let aim = ctx.marks[ctx.mark_of[t.index()] as usize].aim;
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
            let spread = Fx::from_raw(
                (Fx::from(w.spread).raw() as i64 * pct / 100).min(i32::MAX as i64) as i32,
            );
            let mut rng = RngStream::new(ctx.seed, ctx.tick, id, Purpose::AIM);
            // Each MG its own draws: skip the other mounts'.
            for _ in 0..k * 2 {
                rng.next_u64();
            }
            let vel =
                crate::combat::shot_velocity(from, aim, w.muzzle_speed.into(), spread, &mut rng);
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
        self.vehicles.get_mut(&id).expect("listed").mgs = mgs;
        if let Some(ev) = fired {
            self.push_event(events, ev);
        }
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
                let point = if aim_turret {
                    tf.turret.center
                } else {
                    tf.hull.center
                };
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
                Some(Aim {
                    point: m.pos + FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::HALF),
                    vel: ents.velocity[t.index()].lin,
                    vehicle: false,
                })
            }
            _ => None,
        }
    }

    /// The main gun's next target (see the module docs): `(target, aim at
    /// the turret)`.
    fn pick_target(
        &self,
        ents: &Entities,
        ctx: &GunCtx,
        id: SimId,
        f: &VehicleFrame,
        gunner: bool,
        armour_ok: bool,
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
        let ap = main
            .ammo
            .first()
            .and_then(|a| self.armour.ammo.get(a.ammo as usize));
        let margin = setup.rules.vehicles.aim_hull_margin_pct;
        // Vehicles: the ordered one first, then the nearest.
        if armour_ok && let Some(ap) = ap {
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
                let plates = &self.armour.vehicles[tf.type_idx as usize];
                if let Some(part) = aim_choice(from, tf, plates, ap, ctx.terrain, margin) {
                    return (Some(t), part == BoxPart::Turret);
                }
            }
        }
        // Infantry: the best group (more men near, then closer).
        let he = main
            .ammo
            .get(HE as usize)
            .and_then(|a| setup.ammo.get(a.ammo as usize))
            .and_then(|a| a.blast.as_ref());
        let blast_r: Fx = he.map_or(Fx::from_int(6), |b| b.blast.damage_radius.into());
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
        axis: FxVec3,
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
            && in_cone(axis, m.pos.xy() - from.xy(), cone)
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
        axis: FxVec3,
        cone: Fx,
        r: u64,
        roll: u64,
    ) -> Option<SimId> {
        let mut cands: Vec<(u64, SimId)> = ctx
            .marks
            .iter()
            .filter(|m| self.mg_target_ok(ctx, owner, from, axis, cone, r, m.id))
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

/// Whether the horizontal direction `d` lies within `cone` of `axis`'s
/// heading.
fn in_cone(axis: FxVec3, d: FxVec2, cone: Fx) -> bool {
    let a = Fx::atan2(axis.y, axis.x);
    let b = Fx::atan2(d.y, d.x);
    (b - a).wrap_angle().abs() <= cone
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
