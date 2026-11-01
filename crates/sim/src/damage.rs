//! Vehicle damage (S3, `docs/slices/S3-armour.md` "After the hit",
//! "Components", "Crew", "Knock-out and wrecks"): the armour part of the
//! Damage stage and the vehicle part of the Status stage, both serial.
//!
//! **A shell on a plate** (`Blow::Armour`, in slot order): the range is
//! measured from the round's muzzle point; the plate's thickness, ballistic
//! normal and the ±7 % variation (RNG `PENETRATION`, keyed by tick and
//! slot) go through [`armour::penetrates`]. Main-gun shells and rockets use
//! their ammo table; a mortar shell its `pen_mm`. A hit in a hull side's
//! track band meets the running gear's `track_mm` too, and any non-ricochet
//! hit there deals the round's `track_damage` to that side's track.
//!
//! * **Ricochet:** the round is spent (an event only).
//! * **Non-penetration:** `nonpen_damage` to each external part (track,
//!   barrel) with an anchor within `vehicles.gunnery.external_reach_mm` of the impact.
//! * **Penetration** (penetrators): the core goes on along the shell path
//!   for its `length`, dealing its damage to every seat and component box it
//!   crosses (at most `max_boxes`, nearest first); `rays` spall rays in a
//!   cone of `cone_deg` around the path (RNG `SPALL`, rays by index), each
//!   dealing its damage to the first box it meets within
//!   `vehicles.gunnery.spall_reach_m`; an APHE filler bursts `offset` past the entry,
//!   dealing its damage to every box within `radius` (RNG-free). At most
//!   8 rays × 14 boxes.
//! * **HE on a plate:** if its `pen_mm` beats the plate, an inner burst at
//!   the contact point deals `inside_damage` to boxes within
//!   `inside_radius` (none of the S3 rounds does).
//!
//! HE and HEAT burst on contact as well (infantry, `combat`); a burst more
//! than 0.5 m above the ground carves no crater.
//!
//! **Components and crew.** HP never go below 0. An engine hit for at least
//! `fire_min_damage` starts a fire with `fire_start_pct` (RNG `FIRE`, keyed
//! by tick and vehicle, draws in hit order); each ammo-rack hit for at least
//! `rack_min_damage` detonates it with `rack_detonate_pct` (RNG `AMMO_RACK`,
//! tick and slot), and a rack destroyed by damage detonates. A crewman at 0
//! HP is dead: a loss for the owner and a kill for the attacker. A hit
//! stops a field repair.
//!
//! **Blasts near a vehicle** ([`MatchState::blast_tracks`], after each
//! burst of a shell, rocket or thrown item): every enemy vehicle with a
//! track anchor within the blast's lethal radius and in clear line from
//! 0.3 m above the blast point takes `damage × vs_vehicle_pct` on that
//! track (once per track). Nothing else of a closed vehicle is touched; the
//! vehicle a bursting round struck is left to its plate hit.
//!
//! **Knock-out** ([`MatchState::knock_out`]): when every seat is empty, the
//! ammo rack detonates or a fire has burned `fire_burnout`. The remaining
//! crew die (losses; kills for the last attacker), followers get Stop,
//! every vehicle target, `AttackUnit` target, MG target, soldier target and
//! squad focus naming it is cleared, and it becomes a wreck
//! (`MatchState::make_wreck`: wreck cap, id freed LIFO).
//!
//! **Status** ([`MatchState::vehicle_status`], after the soldiers' status):
//! seat swaps count down and an empty driver or gunner seat is refilled
//! (gunner first) by the living crewman of lowest priority (radio, loader,
//! commander) over `seat_swap`; fires burn the engine at `fire_burn_hp_s`
//! and roll to go out every `fire_check` while a crewman lives; field
//! repairs add HP at the def's rate to the worst repairable component, then
//! the next, until none is left (stopped with no crew); inside the own base
//! a vehicle regains `base_repair_pct_s` of every component's HP per second,
//! one round per `base_rearm` and one dead seat per `base_crew`.

use frontline_content::setup::HeSetup;
use frontline_content::vehicle::{Frame, PartBoxSetup, VehicleSetup};
use frontline_content::{AmmoMechanism, ComponentKind, SeatRole};

use crate::armour::{self, PenResult};
use crate::combat::{BallisticEvent, Blow, DamageCtx, tri};
use crate::ecs::{Entities, EntityKind, PlayerId, SimId};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{Ctx, MatchState, PathReq, kill_class};
use crate::hash::HashCache;
use crate::projectile::NO_AMMO;
use crate::rng::{Purpose, RngStream};
use crate::snapshot::{ArmourEventKind, NONE_U8};
use crate::vehicle::{BoxFrame, Seat, VehicleFrame, VehicleState};
use crate::world::Stats;

// S3b: the external-part reach and the spall reach are rules data
// (`[vehicles.gunnery]`, `frontline_content::tuning`).
/// Most rays per penetration (core plus spall).
pub const MAX_RAYS: usize = 8;
/// Most internal boxes per vehicle.
pub const MAX_BOXES: usize = 14;

/// `ArmourEventKind::KnockedOut` causes (`ArmourEvent::component`).
pub const KO_CREW: u8 = 0;
pub const KO_RACK: u8 = 1;
pub const KO_FIRE: u8 = 2;

/// An armour event in sim units (the snapshot converts it).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct ArmourEv {
    pub kind: ArmourEventKind,
    pub vehicle: SimId,
    pub owner: u8,
    pub attacker: u8,
    pub component: u8,
    pub zone: u8,
    pub ammo: u16,
    pub pos: FxVec3,
    pub seq: u32,
}

impl ArmourEv {
    pub fn new(kind: ArmourEventKind, vehicle: SimId, owner: u8, attacker: u8) -> ArmourEv {
        ArmourEv {
            kind,
            vehicle,
            owner,
            attacker,
            component: NONE_U8,
            zone: NONE_U8,
            ammo: u16::MAX,
            pos: FxVec3::ZERO,
            seq: 0,
        }
    }
    pub fn component(mut self, c: u8) -> ArmourEv {
        self.component = c;
        self
    }
    pub fn zone(mut self, z: u8) -> ArmourEv {
        self.zone = z;
        self
    }
    pub fn ammo(mut self, a: u16) -> ArmourEv {
        self.ammo = a;
        self
    }
    pub fn at(mut self, p: FxVec3) -> ArmourEv {
        self.pos = p;
        self
    }
}

/// What a box inside a vehicle is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Inner {
    /// Seat index in the def's order.
    Seat(u8),
    Part(ComponentKind),
}

/// A part box in world space.
pub fn part_box(f: &VehicleFrame, p: &PartBoxSetup) -> BoxFrame {
    let c = crate::vehicle::v3(p.bounds.center);
    let half = crate::vehicle::v3(p.bounds.half);
    match p.frame {
        Frame::Hull => BoxFrame {
            center: f.origin + f.hull.axes.apply(c),
            axes: f.hull.axes,
            half,
        },
        Frame::Turret => BoxFrame {
            center: f.pivot + f.turret.axes.apply(c),
            axes: f.turret.axes,
            half,
        },
    }
}

/// An anchor point in world space.
pub fn anchor_at(f: &VehicleFrame, frame: Frame, pos: FxVec3) -> FxVec3 {
    match frame {
        Frame::Hull => f.origin + f.hull.axes.apply(pos),
        Frame::Turret => f.pivot + f.turret.axes.apply(pos),
    }
}

/// Every seat and internal component box of a vehicle in world space:
/// seats in def order, then components by kind.
pub fn inner_boxes(f: &VehicleFrame, v: &VehicleSetup) -> Vec<(BoxFrame, Inner)> {
    let mut out = Vec::with_capacity(MAX_BOXES);
    for (k, s) in v.seats.iter().enumerate() {
        out.push((part_box(f, &s.part), Inner::Seat(k as u8)));
    }
    for c in &v.components {
        if let Some(p) = &c.part {
            out.push((part_box(f, p), Inner::Part(c.kind)));
        }
    }
    out.truncate(MAX_BOXES);
    out
}

/// The boxes a ray `from → from + d` enters, nearest first (ties by box
/// order), at most `max`.
pub fn ray_boxes(boxes: &[(BoxFrame, Inner)], from: FxVec3, d: FxVec3, max: usize) -> Vec<usize> {
    let mut hits: Vec<(Fx, usize)> = boxes
        .iter()
        .enumerate()
        .filter_map(|(k, (b, _))| armour::segment_box(b, from, d).map(|(t, _, _)| (t, k)))
        .collect();
    hits.sort_unstable();
    hits.into_iter().take(max).map(|(_, k)| k).collect()
}

/// Spall ray `k`'s directions for a penetration along unit `dir` (RNG
/// stream `r`, one pair of draws per ray in ray order).
pub fn spall_dir(dir: FxVec3, cone: Fx, r: &mut RngStream) -> FxVec3 {
    let right = FxVec3::new(-dir.y, dir.x, Fx::ZERO).normalize_or_zero();
    let up = FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE);
    (dir + right * tri(r, cone) + up * tri(r, cone)).normalize_or_zero()
}

/// One hit's damage to one inner box or external part.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Dealt {
    pub inner: Inner,
    pub dmg: u32,
}

/// What a plate hit does: the rule's result, the damage in order, and
/// where a bursting round bursts.
struct Resolved {
    result: PenResult,
    dealt: Vec<Dealt>,
    burst: Option<FxVec3>,
    /// The rule's inputs, for the diagnostic hit log.
    record: Option<HitRecord>,
}

/// One main-gun or rocket hit on a live vehicle's plate with the inputs of
/// the penetration rule (`armour::penetrates`), for `armour-test` case 1.
/// Diagnostics only: kept by [`crate::World::enable_hit_log`], never saved
/// or hashed, never read by the sim.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct HitRecord {
    pub tick: u64,
    pub vehicle: SimId,
    pub player: u8,
    pub ammo: u16,
    pub part: armour::BoxPart,
    pub zone: u8,
    pub track_band: bool,
    pub pen0: Fx,
    pub c: Fx,
    pub thickness: Fx,
    pub extra: Fx,
    pub cos_ricochet: Fx,
    pub v: Fx,
    pub result: PenResult,
}

/// The per-tick fire-roll streams, one per vehicle (draws in hit order).
#[derive(Default)]
pub(crate) struct FireRolls(Vec<(SimId, RngStream)>);

impl FireRolls {
    fn roll(&mut self, seed: u64, tick: u64, id: SimId, pct: u32) -> bool {
        let k = match self.0.iter().position(|(v, _)| *v == id) {
            Some(k) => k,
            None => {
                self.0
                    .push((id, RngStream::new(seed, tick, id, Purpose::FIRE)));
                self.0.len() - 1
            }
        };
        self.0[k].1.range_i32(0, 99) < pct as i32
    }
}

impl MatchState {
    /// Appends `ev` with the next armour event number.
    pub(crate) fn push_event(&mut self, events: &mut Vec<ArmourEv>, mut ev: ArmourEv) {
        ev.seq = self.armour_seq;
        self.armour_seq = self.armour_seq.wrapping_add(1);
        events.push(ev);
    }

    /// Resolves one `Blow::Armour` (see the module docs). Returns the burst
    /// point for a bursting round.
    pub(crate) fn armour_blow(
        &mut self,
        ctx: &mut DamageCtx,
        e: &BallisticEvent,
        fires: &mut FireRolls,
        events: &mut Vec<ArmourEv>,
    ) -> Option<FxVec3> {
        let Blow::Armour {
            vehicle: id, hit, ..
        } = e.blow
        else {
            return None;
        };
        let alive = ctx.ents.kind.get(id.index()) == Some(&EntityKind::Vehicle);
        let r = self.resolve(ctx.seed, ctx.tick, e, alive)?;
        if !alive {
            // A wreck (or one knocked out earlier this tick) only stops it.
            return r.burst;
        }
        let owner = ctx.ents.owner[id.index()].0;
        let attacker = e.player;
        if let (Some(log), Some(rec)) = (&mut self.hit_log, r.record) {
            log.push(rec);
        }
        let vs = self.vehicles.get_mut(&id).expect("vehicle state");
        vs.repairing = None;
        vs.repair_progress = 0;
        vs.last_attacker = Some(attacker);
        if e.ammo != NO_AMMO {
            let (kind, stat) = match r.result {
                PenResult::Ricochet => (ArmourEventKind::Ricochet, &mut ctx.stats.ricochets),
                PenResult::NonPenetration => (
                    ArmourEventKind::NonPenetration,
                    &mut ctx.stats.non_penetrations,
                ),
                PenResult::Penetration => {
                    (ArmourEventKind::Penetration, &mut ctx.stats.penetrations)
                }
            };
            *stat += 1;
            let ev = ArmourEv::new(kind, id, owner, attacker)
                .zone(hit.zone.index() as u8)
                .ammo(e.ammo)
                .at(hit.point);
            self.push_event(events, ev);
        }
        let class = self.round_class(ctx.ents, e.weapon, e.ammo, e.shooter);
        let ko = self.deal(
            ctx,
            id,
            &r.dealt,
            (attacker, class),
            SimId(e.slot),
            fires,
            events,
            hit.point,
            e.ammo,
        );
        if let Some(cause) = ko {
            self.knock_out(ctx, id, cause, class, events);
        }
        r.burst
    }

    /// The rolls and the damage list of one `Blow::Armour` (read only).
    fn resolve(&self, seed: u64, tick: u64, e: &BallisticEvent, alive: bool) -> Option<Resolved> {
        let Blow::Armour {
            vehicle: id,
            hit,
            vel,
            origin,
        } = e.blow
        else {
            return None;
        };
        let setup = &self.setup;
        let ammo = setup.ammo.get(e.ammo as usize);
        let weapon = setup.weapons.get(e.weapon as usize)?;
        let bursts = ammo.map_or(weapon.he.is_some(), |a| a.mechanism.bursts());
        let burst = bursts.then_some(hit.point);
        let mut out = Resolved {
            result: PenResult::Ricochet,
            dealt: Vec::new(),
            burst,
            record: None,
        };
        if !alive {
            return Some(out);
        }
        let f = self.vderived.frame(id).copied()?;
        let vs = self.vehicles.get(&id)?;
        let vt = vs.type_idx as usize;
        let v = &setup.vehicles[vt];
        let plates = &self.armour.vehicles[vt];
        let plate = plates.face(hit.part, hit.face);
        let dir = vel.normalize_or_zero();
        let c = armour::impact_cos(dir, hit.normal);
        let band = armour::in_track_band(&hit, f.hull.half.z, plates.track_band_pct);
        let extra = if band { plates.track_mm } else { Fx::ZERO };
        let (pen0, cos_ric) = match (ammo, self.armour.ammo.get(e.ammo as usize)) {
            (Some(_), Some(t)) => (t.pen0((hit.point - origin).length()), t.cos_ricochet),
            // A mortar shell: its flat `pen_mm`, ricochet over 80°.
            _ => (
                Fx::from_int(weapon.he.as_ref().map_or(0, |h| h.pen_mm) as i32),
                armour::deg(80).cos(),
            ),
        };
        let mut rng = RngStream::new(seed, tick, SimId(e.slot), Purpose::PENETRATION);
        let var = armour::variation(&mut rng);
        out.result = armour::penetrates(pen0, c, plate.thickness, extra, cos_ric, var);
        if self.hit_log.is_some() && e.ammo != NO_AMMO {
            out.record = Some(HitRecord {
                tick,
                vehicle: id,
                player: e.player,
                ammo: e.ammo,
                part: hit.part,
                zone: hit.zone.index() as u8,
                track_band: band,
                pen0,
                c,
                thickness: plate.thickness,
                extra,
                cos_ricochet: cos_ric,
                v: var,
                result: out.result,
            });
        }
        if out.result == PenResult::Ricochet {
            return Some(out);
        }
        let dealt = &mut out.dealt;
        // The running gear: any hit in the track band.
        if band && let Some(a) = ammo {
            let track = if hit.face == armour::Face::Left {
                ComponentKind::TrackLeft
            } else {
                ComponentKind::TrackRight
            };
            dealt.push(Dealt {
                inner: Inner::Part(track),
                dmg: a.track_damage,
            });
        }
        let boxes = inner_boxes(&f, v);
        let mech = ammo.map(|a| a.mechanism);
        match (out.result, mech) {
            (PenResult::Penetration, Some(m)) if m.penetrator() => {
                let a = ammo.expect("penetrator ammo");
                if let Some(core) = &a.core {
                    let len: Fx = core.length.into();
                    for k in ray_boxes(&boxes, hit.point, dir * len, core.max_boxes as usize) {
                        dealt.push(Dealt {
                            inner: boxes[k].1,
                            dmg: core.damage,
                        });
                    }
                }
                if let Some(sp) = &a.spall {
                    let mut r = RngStream::new(seed, tick, SimId(e.slot), Purpose::SPALL);
                    let cone = armour::deg(sp.cone_deg);
                    let reach =
                        Fx::from_int(self.setup.rules.vehicles.gunnery.spall_reach_m as i32);
                    for _ in 0..(sp.rays as usize).min(MAX_RAYS - 1) {
                        let d = spall_dir(dir, cone, &mut r);
                        if let Some(&k) = ray_boxes(&boxes, hit.point, d * reach, 1).first() {
                            dealt.push(Dealt {
                                inner: boxes[k].1,
                                dmg: sp.damage,
                            });
                        }
                    }
                }
                if m == AmmoMechanism::Aphe
                    && let Some(fl) = &a.filler
                {
                    let at = hit.point + dir * Fx::from(fl.offset);
                    within(&boxes, at, fl.radius.into(), fl.damage, dealt);
                }
            }
            (PenResult::Penetration, _) => {
                // HE beat the plate: the inner burst at the contact point.
                let inside = ammo
                    .and_then(|a| a.blast.as_ref())
                    .map(|b| &b.blast)
                    .or(weapon.he.as_ref());
                if let Some(h) = inside {
                    within(
                        &boxes,
                        hit.point,
                        h.inside_radius.into(),
                        h.inside_damage,
                        dealt,
                    );
                }
            }
            (_, _) => {
                // Non-penetration: external parts near the impact.
                let nonpen = ammo.map_or(0, |a| a.nonpen_damage);
                if nonpen > 0 {
                    let reach =
                        Fx::from_milli(self.setup.rules.vehicles.gunnery.external_reach_mm as i32)
                            .raw() as u64;
                    for comp in &v.components {
                        let near = comp.anchors.iter().any(|an| {
                            let p = anchor_at(&f, an.frame, crate::vehicle::v3(an.pos));
                            (p - hit.point).length_sq_raw() <= reach * reach
                        });
                        if near {
                            dealt.push(Dealt {
                                inner: Inner::Part(comp.kind),
                                dmg: nonpen,
                            });
                        }
                    }
                }
            }
        }
        Some(out)
    }

    /// Applies `dealt` to vehicle `id` in order (see the module docs);
    /// `by` is the attacking player and the round's `game::kill_class`.
    /// Returns a knock-out cause if it ends the vehicle.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn deal(
        &mut self,
        ctx: &mut DamageCtx,
        id: SimId,
        dealt: &[Dealt],
        by: (u8, u8),
        rack_key: SimId,
        fires: &mut FireRolls,
        events: &mut Vec<ArmourEv>,
        pos: FxVec3,
        ammo: u16,
    ) -> Option<u8> {
        let owner = ctx.ents.owner[id.index()].0;
        let (attacker, class) = by;
        let r = &self.setup.rules.vehicles;
        let mut rack_rng = RngStream::new(ctx.seed, ctx.tick, rack_key, Purpose::AMMO_RACK);
        let mut detonated = false;
        let mut dead = 0u32;
        let mut evs: Vec<ArmourEv> = Vec::new();
        let vs = self.vehicles.get_mut(&id).expect("vehicle state");
        let v = &self.setup.vehicles[vs.type_idx as usize];
        let ev = |k| ArmourEv::new(k, id, owner, attacker).ammo(ammo).at(pos);
        for d in dealt {
            match d.inner {
                Inner::Seat(k) => {
                    let Some(s) = vs.seats.get_mut(k as usize) else {
                        continue;
                    };
                    if s.hp == 0 {
                        continue;
                    }
                    s.hp = s.hp.saturating_sub(d.dmg.min(u16::MAX as u32) as u16);
                    if s.hp == 0 {
                        *s = Seat::default();
                        dead += 1;
                    }
                    evs.push(
                        ev(ArmourEventKind::CrewHit).component(v.seats[k as usize].role as u8),
                    );
                }
                Inner::Part(kind) => {
                    let hp = &mut vs.components[kind.index()];
                    if *hp == 0 {
                        continue;
                    }
                    *hp = hp.saturating_sub(d.dmg.min(u16::MAX as u32) as u16);
                    let gone = *hp == 0;
                    if kind == ComponentKind::Engine
                        && d.dmg >= r.fire_min_damage
                        && vs.burn == 0
                        && fires.roll(ctx.seed, ctx.tick, id, r.fire_start_pct)
                    {
                        vs.burn = 1;
                    }
                    if kind == ComponentKind::AmmoRack
                        && (gone
                            || (d.dmg >= r.rack_min_damage
                                && rack_rng.range_i32(0, 99) < r.rack_detonate_pct as i32))
                    {
                        detonated = true;
                    }
                    let k = if gone {
                        ArmourEventKind::Disabled
                    } else {
                        ArmourEventKind::ComponentHit
                    };
                    evs.push(ev(k).component(kind as u8));
                }
            }
        }
        let crew_gone = vs.seats.iter().all(|s| s.hp == 0);
        for e in evs {
            self.push_event(events, e);
        }
        if dead > 0 {
            self.crew_lost(PlayerId(owner), Some(attacker), class, dead);
        }
        if detonated {
            Some(KO_RACK)
        } else if crew_gone {
            Some(KO_CREW)
        } else {
            None
        }
    }

    /// A blast near vehicles (S3 "Blasts near a vehicle"): every enemy
    /// vehicle of `player` (not `skip`, the one a bursting round struck)
    /// with an anchor of a track within the blast's lethal radius and in
    /// clear line from just above the blast point takes `damage ×
    /// vs_vehicle_pct` on that track, once per track. Nothing else of a
    /// closed vehicle is touched. Vehicles in ascending id.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn blast_tracks(
        &mut self,
        ctx: &mut DamageCtx,
        pos: FxVec3,
        he: &HeSetup,
        damage: u32,
        player: u8,
        skip: Option<SimId>,
        ammo: u16,
        fires: &mut FireRolls,
        events: &mut Vec<ArmourEv>,
    ) {
        let dmg = (damage as u64 * he.vs_vehicle_pct as u64 / 100) as u32;
        if dmg == 0 || self.vehicles.is_empty() {
            return;
        }
        let lethal: Fx = he.lethal_radius.into();
        let lift = FxVec3::new(
            pos.x,
            pos.y,
            pos.z + Fx::from_milli(self.setup.rules.combat.blast.lift_mm as i32),
        );
        let mut ids: Vec<SimId> = Vec::new();
        self.vderived
            .index
            .for_each_near(pos.xy(), lethal + self.vderived.max_reach, |id| {
                ids.push(id)
            });
        ids.sort_unstable();
        ids.dedup();
        let reach = lethal.raw().max(0) as u64;
        for id in ids {
            if Some(id) == skip
                || ctx.ents.kind.get(id.index()) != Some(&EntityKind::Vehicle)
                || ctx.ents.owner[id.index()].0 == player
            {
                continue;
            }
            let (Some(f), Some(vs)) = (self.vderived.frame(id).copied(), self.vehicles.get(&id))
            else {
                continue;
            };
            let v = &self.setup.vehicles[vs.type_idx as usize];
            let dealt: Vec<Dealt> = v
                .components
                .iter()
                .filter(|c| {
                    matches!(c.kind, ComponentKind::TrackLeft | ComponentKind::TrackRight)
                        && c.anchors.iter().any(|an| {
                            let p = anchor_at(&f, an.frame, crate::vehicle::v3(an.pos));
                            (p - pos).length_sq_raw() <= reach * reach
                                && ctx.terrain.line_of_sight(lift, p)
                        })
                })
                .map(|c| Dealt {
                    inner: Inner::Part(c.kind),
                    dmg,
                })
                .collect();
            if dealt.is_empty() {
                continue;
            }
            let vs = self.vehicles.get_mut(&id).expect("vehicle state");
            vs.repairing = None;
            vs.repair_progress = 0;
            vs.last_attacker = Some(player);
            ctx.stats.blast_track_hits += dealt.len() as u64;
            // Tracks never start a fire or set off the rack: no knock-out.
            self.deal(
                ctx,
                id,
                &dealt,
                (player, kill_class::OTHER),
                id,
                fires,
                events,
                pos,
                ammo,
            );
        }
    }

    /// Counts `n` crewmen of `owner` lost, killed by `by` with a round of
    /// `game::kill_class` `class`.
    fn crew_lost(&mut self, owner: PlayerId, by: Option<u8>, class: u8, n: u32) {
        if let Some(p) = self.players.get_mut(owner.0 as usize) {
            p.losses += n;
        }
        if let Some(p) = by.and_then(|b| self.players.get_mut(b as usize))
            && by != Some(owner.0)
        {
            p.kills += n;
            p.kills_by[class as usize] += n;
        }
    }

    /// Knocks vehicle `id` out (see the module docs).
    pub(crate) fn knock_out(
        &mut self,
        ctx: &mut DamageCtx,
        id: SimId,
        cause: u8,
        class: u8,
        events: &mut Vec<ArmourEv>,
    ) {
        let mut c = Ctx {
            ents: &mut *ctx.ents,
            terrain: &*ctx.terrain,
            hash: &mut *ctx.hash,
            tick: ctx.tick,
        };
        self.knock_out_with(&mut c, ctx.stats, id, cause, class, events);
    }

    /// [`Self::knock_out`] with a plain match context (Status stage).
    pub(crate) fn knock_out_with(
        &mut self,
        ctx: &mut Ctx,
        stats: &mut Stats,
        id: SimId,
        cause: u8,
        class: u8,
        events: &mut Vec<ArmourEv>,
    ) {
        let i = id.index();
        if ctx.ents.kind.get(i) != Some(&EntityKind::Vehicle) {
            return;
        }
        let owner = ctx.ents.owner[i];
        let Some(vs) = self.vehicles.get(&id) else {
            return;
        };
        let alive = vs.seats.iter().filter(|s| s.hp > 0).count() as u32;
        let by = vs.last_attacker;
        let followers = vs.followers.clone();
        let squad = ctx.ents.squad[i];
        let pos = ctx.ents.transform[i].pos;
        self.crew_lost(owner, by, class, alive);
        stats.knockouts += 1;
        // Followers stop where they are.
        for m in followers {
            let mi = m.index();
            if ctx.ents.kind.get(mi) != Some(&EntityKind::Soldier) {
                continue;
            }
            if let Some(sq) = ctx.ents.squad[mi] {
                if ctx.ents.unit[mi].detached {
                    self.units_stop(ctx, ctx.ents.owner[mi], &[m]);
                } else {
                    self.order_stop(ctx, ctx.ents.owner[mi], sq);
                }
            }
        }
        if let Some(sq) = squad {
            for q in &mut self.path_queue.queues {
                q.retain(|r| !matches!(r, PathReq::Vehicle { squad, .. } if *squad == sq));
            }
        }
        let removed = self.make_wreck(ctx.ents, ctx.hash, id, ctx.tick);
        for gone in std::iter::once(id).chain(removed) {
            self.forget(ctx.ents, ctx.hash, gone);
        }
        let ev = ArmourEv::new(
            ArmourEventKind::KnockedOut,
            id,
            owner.0,
            by.unwrap_or(NONE_U8),
        )
        .component(cause)
        .at(pos);
        self.push_event(events, ev);
    }

    /// Clears every reference to `id` as a target (vehicle targets,
    /// `AttackUnit`, MG targets, soldier targets, squad focus).
    fn forget(&mut self, ents: &mut Entities, hash: &mut HashCache, id: SimId) {
        for vs in self.vehicles.values_mut() {
            if vs.target == Some(id) {
                vs.target = None;
                vs.ranging = 0;
            }
            if vs.attack == Some(id) {
                vs.attack = None;
            }
            for m in &mut vs.mgs {
                if m.target == Some(id) {
                    m.target = None;
                }
            }
        }
        for sq in self.squads.values_mut() {
            if sq.focus == Some(id) {
                sq.focus = None;
            }
            if let Some(l) = &mut sq.launcher
                && l.attack == Some(id)
            {
                l.attack = None;
            }
        }
        for (i, c) in ents.combat.iter_mut().enumerate() {
            if c.target == Some(id) && ents.kind[i] == EntityKind::Soldier {
                c.target = None;
                hash.mark_entity_chunk(SimId(i as u32).chunk());
            }
        }
    }

    /// Drops vehicle references to entities that are no longer valid
    /// (soldier targets and followers that died this tick), so the saved
    /// state never names a corpse or a freed id. End of the Damage stage.
    pub(crate) fn sweep_vehicle_refs(&mut self, ents: &Entities) {
        let kind = |id: SimId| ents.kind.get(id.index()).copied();
        for vs in self.vehicles.values_mut() {
            if vs.target.is_some_and(|t| {
                !matches!(kind(t), Some(EntityKind::Vehicle | EntityKind::Soldier))
            }) {
                vs.target = None;
                vs.ranging = 0;
            }
            if vs
                .attack
                .is_some_and(|t| kind(t) != Some(EntityKind::Vehicle))
            {
                vs.attack = None;
            }
            for m in &mut vs.mgs {
                if m.target
                    .is_some_and(|t| kind(t) != Some(EntityKind::Soldier))
                {
                    m.target = None;
                }
            }
            vs.followers
                .retain(|&f| kind(f) == Some(EntityKind::Soldier));
        }
    }

    // ------------------------------------------------------------- status --

    /// Seat swaps, fires, field and base repair for every vehicle (Status
    /// stage, ascending id; see the module docs).
    pub(crate) fn vehicle_status(
        &mut self,
        ctx: &mut Ctx,
        stats: &mut Stats,
        seed: u64,
        events: &mut Vec<ArmourEv>,
    ) {
        if self.vehicles.is_empty() {
            return;
        }
        let tick = ctx.tick;
        let r = self.setup.rules.vehicles.clone();
        let hz = self.setup.tick_hz.max(1);
        let ids: Vec<SimId> = self.vehicles.keys().copied().collect();
        let mut ko: Vec<(SimId, u8)> = Vec::new();
        for id in ids {
            let i = id.index();
            if ctx.ents.kind[i] != EntityKind::Vehicle {
                continue;
            }
            let owner = ctx.ents.owner[i];
            let pos = ctx.ents.transform[i].pos;
            let base = &self.setup.map.bases[self.setup.players[owner.0 as usize].base as usize];
            let br: Fx = base.radius.into();
            let in_base = (pos.xy() - FxVec2::from(base.center)).length_sq_raw()
                <= FxVec2::new(br, Fx::ZERO).length_sq_raw();
            let vs = self.vehicles.get_mut(&id).expect("listed");
            let v = &self.setup.vehicles[vs.type_idx as usize];
            swap_seats(vs, v, r.seat_swap_ticks);
            // Fire.
            let crew = vs.seats.iter().any(|s| s.hp > 0);
            let mut fire_events: Vec<ArmourEv> = Vec::new();
            if vs.burn > 0 {
                vs.burn = vs.burn.saturating_add(1);
                let n = vs.burn as u64;
                let lost = (n * r.fire_burn_hp_s as u64 / hz as u64)
                    - ((n - 1) * r.fire_burn_hp_s as u64 / hz as u64);
                let e = &mut vs.components[ComponentKind::Engine.index()];
                if *e > 0 && lost > 0 {
                    *e = e.saturating_sub(lost.min(u16::MAX as u64) as u16);
                    if *e == 0 {
                        fire_events.push(
                            ArmourEv::new(ArmourEventKind::Disabled, id, owner.0, NONE_U8)
                                .component(ComponentKind::Engine as u8)
                                .at(pos),
                        );
                    }
                }
                if vs.burn as u32 >= r.fire_burnout_ticks {
                    ko.push((id, KO_FIRE));
                } else if crew && (vs.burn as u32 - 1).is_multiple_of(r.fire_check_ticks.max(1)) {
                    let mut rr = RngStream::new(seed, tick, id, Purpose::FIRE);
                    if rr.range_i32(0, 99) < r.fire_out_pct as i32 {
                        vs.burn = 0;
                    }
                }
            }
            // Field repair.
            if let Some(k) = vs.repairing {
                if !crew {
                    vs.repairing = None;
                    vs.repair_progress = 0;
                } else {
                    repair_step(vs, v, k);
                }
            }
            // Base repair, rearm and crew.
            if in_base {
                vs.base_ticks = vs.base_ticks.saturating_add(1);
                let n = vs.base_ticks as u64;
                for c in &v.components {
                    let max = c.hp as u64;
                    let rate = r.base_repair_pct_s as u64 * max;
                    let add = n * rate / (100 * hz as u64) - (n - 1) * rate / (100 * hz as u64);
                    let hp = &mut vs.components[c.kind.index()];
                    *hp = (*hp as u64 + add).min(max) as u16;
                }
                if n.is_multiple_of(r.base_rearm_ticks.max(1) as u64) {
                    rearm_one(vs, v);
                }
                if n.is_multiple_of(r.base_crew_ticks.max(1) as u64)
                    && let Some(s) = vs.seats.iter_mut().find(|s| s.hp == 0 && s.swap == 0)
                {
                    s.hp = r.crew_hp.min(u16::MAX as u32) as u16;
                }
            } else {
                vs.base_ticks = 0;
            }
            for ev in fire_events {
                self.push_event(events, ev);
            }
        }
        for (id, cause) in ko {
            self.knock_out_with(ctx, stats, id, cause, kill_class::OTHER, events);
        }
    }

    /// Starts a field repair of vehicle `id`'s worst repairable component
    /// (`RepairUnits`, the AI). False if there is nothing to repair, no
    /// crew, or it is a wreck.
    pub fn start_repair(&mut self, ents: &Entities, id: SimId) -> bool {
        if ents.kind.get(id.index()) != Some(&EntityKind::Vehicle) {
            return false;
        }
        let Some(vs) = self.vehicles.get_mut(&id) else {
            return false;
        };
        let v = &self.setup.vehicles[vs.type_idx as usize];
        if !vs.seats.iter().any(|s| s.hp > 0) {
            return false;
        }
        match worst_repairable(vs, v) {
            Some(k) => {
                if vs.repairing != Some(k) {
                    vs.repairing = Some(k);
                    vs.repair_progress = 0;
                }
                true
            }
            None => false,
        }
    }
}

/// HP of `dealt` boxes within `radius` of `at` (filler, HE inner burst).
fn within(boxes: &[(BoxFrame, Inner)], at: FxVec3, radius: Fx, dmg: u32, out: &mut Vec<Dealt>) {
    let r = radius.raw().max(0) as u64;
    for (b, inner) in boxes {
        if armour::box_dist_sq_raw(b, at) <= r * r {
            out.push(Dealt { inner: *inner, dmg });
        }
    }
}

/// Refills an empty driver or gunner seat (gunner first) from the living
/// crewman of lowest priority (radio, loader, commander), and counts
/// running swaps down.
pub fn swap_seats(vs: &mut VehicleState, v: &VehicleSetup, swap_ticks: u32) {
    for s in &mut vs.seats {
        s.swap = s.swap.saturating_sub(1);
    }
    let idx = |role: SeatRole| v.seats.iter().position(|s| s.role == role);
    for need in [SeatRole::Gunner, SeatRole::Driver] {
        let Some(n) = idx(need) else { continue };
        if vs.seats[n].hp > 0 {
            continue;
        }
        let donor = [SeatRole::RadioMg, SeatRole::Loader, SeatRole::Commander]
            .into_iter()
            .filter_map(idx)
            .find(|&d| vs.seats[d].hp > 0 && vs.seats[d].swap == 0);
        let Some(d) = donor else { continue };
        vs.seats[n] = Seat {
            hp: vs.seats[d].hp,
            swap: swap_ticks.clamp(1, u16::MAX as u32) as u16,
            swap_from: d as u8,
        };
        vs.seats[d] = Seat::default();
    }
}

/// Whether a field repair would find work: a crewman alive and a
/// repairable component below its repair target (the AI's check).
pub fn can_field_repair(vs: &VehicleState, v: &VehicleSetup) -> bool {
    vs.seats.iter().any(|s| s.hp > 0) && worst_repairable(vs, v).is_some()
}

/// The repairable component in the worst shape (lowest share of its HP,
/// ties by kind), below its repair target.
fn worst_repairable(vs: &VehicleState, v: &VehicleSetup) -> Option<u8> {
    v.components
        .iter()
        .filter_map(|c| {
            let rep = c.repair.as_ref()?;
            let hp = vs.components[c.kind.index()] as u32;
            (hp < rep.to_hp && rep.ticks > 0)
                .then(|| (hp as u64 * 1000 / c.hp.max(1) as u64, c.kind.index() as u8))
        })
        .min()
        .map(|(_, k)| k)
}

/// One tick of field repair on component `k`: `to_hp` HP per the def's
/// ticks, then the next repairable component, until none is left.
fn repair_step(vs: &mut VehicleState, v: &VehicleSetup, k: u8) {
    let Some(c) = v.components.iter().find(|c| c.kind.index() == k as usize) else {
        vs.repairing = None;
        return;
    };
    let Some(rep) = &c.repair else {
        vs.repairing = None;
        return;
    };
    vs.repair_progress += rep.to_hp;
    let add = vs.repair_progress / rep.ticks.max(1);
    vs.repair_progress %= rep.ticks.max(1);
    let hp = &mut vs.components[k as usize];
    *hp = (*hp as u32 + add).min(rep.to_hp.max(*hp as u32)) as u16;
    if *hp as u32 >= rep.to_hp {
        vs.repair_progress = 0;
        vs.repairing = worst_repairable(vs, v);
    }
}

/// Base rearm: one round of the entry furthest below its load (ties: the
/// first).
fn rearm_one(vs: &mut VehicleState, v: &VehicleSetup) {
    let best = v
        .main_gun()
        .ammo
        .iter()
        .enumerate()
        .map(|(k, a)| {
            let have = vs.ammo[k] as u32 + (vs.loaded == Some(k as u8)) as u32;
            (a.count as u32).saturating_sub(have)
        })
        .enumerate()
        .filter(|(_, short)| *short > 0)
        .max_by_key(|&(k, short)| (short, std::cmp::Reverse(k)));
    if let Some((k, _)) = best {
        vs.ammo[k] += 1;
    }
}
