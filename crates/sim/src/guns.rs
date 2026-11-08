//! S9: served guns (`docs/slices/S9-at-guns.md` 3, 4, 5). A gun is a
//! vehicle (`crew_model = "served"`, `mobility.type = "towed"`) with no
//! seats of its own: its crew are ordinary soldiers in service slots, each
//! with [`crate::unit::Serving`], struck by bullets that miss the shield
//! box like any other man.
//!
//! **Slots and the mirror.** Each Status stage, before gunnery reads it
//! next tick: dead or lost men leave their slots; a dead gunner's or
//! loader's place is taken by the lowest-priority living man
//! (`slot_swap_s`; gunner first); every man stands at his slot, walking
//! there after a swap; then the slots are mirrored into
//! `VehicleState::seats` (hp, swap, untrained). A pinned man reads as not
//! manned. So `crate::gunnery` (manned, reload, spread) and
//! `crate::spotting` (the commander rule) run unchanged.
//!
//! **Fire.** A man at a slot fires his own weapon only while the gun has
//! no target and an enemy is within `self_defence_m` (S9-12 (A)). A gun
//! holding fire ([`holds_fire`]) keeps laying on its target until it comes
//! within `hold_fire_m`, or something strikes the gun.
//!
//! **Penetrators.** A core through the shield or carriage goes on along
//! its line and strikes the men on the next `core.length_cm`
//! ([`MatchState::core_through_crew`]). The men's capsules are ordinary:
//! bullets that miss the shield reach them, bullets that strike it stop
//! (`crate::armour`).
//!
//! **Mobility** (spec 6): emplace, pack, push, hitch, unhitch and the
//! trail re-lay run in [`MatchState::guns_status`]; a hitched gun follows
//! its tow on a rigid bar ([`MatchState::tow_bars`]). A man who is ordered
//! off a gun leaves it; a crew that lost its tow walks back.
//!
//! **Neutral and captured** (spec 9): a gun with no man of its owner at a
//! slot or within `hold_m` for `abandon_s` turns `EntityKind::Abandoned`
//! (neutral, seen by all); `Recrew` serves it, and a squad of another
//! player takes it over, the owner column changing with it. A served gun
//! draws rounds only from sources of its own def faction.

use frontline_content::setup::Capability;
use frontline_content::vehicle::ServicePose;
use frontline_content::{AmmoMechanism, MatchSetup};
use serde::{Deserialize, Serialize};

use crate::combat::{BallisticEvent, Blow, DamageCtx, add_suppression, capsule, seg_seg};
use crate::command::CommandKind;
use crate::damage::ArmourEv;
use crate::ecs::{Entities, EntityKind, PlayerId, SimId, Transform};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::SquadId;
use crate::game::{Ctx, MatchState, Nav, NavPhase, Order, Squad, hold_in_place};
use crate::snapshot::ArmourEventKind;
use crate::unit::{Serving, StanceOrder};
use crate::vehicle::{self, GunState, Seat, VehicleState, yaw_dir};

/// World position (xy) of service-slot offset `at` of a gun at `t`.
fn slot_xy(t: &Transform, at: frontline_content::vehicle::FixedVec3) -> FxVec2 {
    let fwd = yaw_dir(t.yaw);
    let left = FxVec2::new(-fwd.y, fwd.x);
    t.pos.xy() + fwd * Fx::from(at.x) + left * Fx::from(at.y)
}

/// The stance order for pose `p`.
fn stance_of(p: ServicePose) -> StanceOrder {
    match p {
        ServicePose::Kneel => StanceOrder::Crouch,
        ServicePose::Prone => StanceOrder::Prone,
    }
}

/// Whether a crewman of `gun` may fire his own weapon at `target`.
pub(crate) fn may_fire_rifle(
    game: &MatchState,
    gun: SimId,
    me: FxVec2,
    transform: &[Transform],
    target: SimId,
    kind: &[EntityKind],
) -> bool {
    let Some(vs) = game.vehicle(gun) else {
        return true;
    };
    if kind.get(gun.index()) != Some(&EntityKind::Vehicle) {
        return true;
    }
    if vs.target.is_some() && vs.gun_ready() {
        return false;
    }
    let r = Fx::from_int(game.setup().rules.vehicles.guns.self_defence_m as i32);
    (transform[target.index()].pos.xy() - me).length_sq_raw()
        <= FxVec2::new(r, Fx::ZERO).length_sq_raw()
}

/// Whether gun `vs` holds its fire at aim point `aim` (`hold_fire_m`,
/// S9): a target farther than the hold range is laid on but not fired at,
/// until the gun is struck (`last_attacker`).
pub(crate) fn holds_fire(
    vs: &VehicleState,
    s: &MatchSetup,
    muzzle: FxVec3,
    aim: Option<FxVec3>,
) -> bool {
    let (Some(sv), Some(p)) = (vs.served.as_ref(), aim) else {
        return false;
    };
    if sv.hold_fire_m == 0 || vs.last_attacker.is_some() {
        return false;
    }
    let hold = (sv.hold_fire_m as i32).max(s.rules.vehicles.guns.self_defence_m as i32);
    (p - muzzle).xy().length_sq_raw() > FxVec2::new(Fx::from_int(hold), Fx::ZERO).length_sq_raw()
}

impl MatchState {
    /// Load checks of the served-gun links (`World::validate`): a gun's
    /// state only on a served def, slot men alive, same owner, each naming
    /// his slot back; a serving man names a live served gun.
    pub(crate) fn validate_guns(&self, ents: &Entities) -> Result<(), &'static str> {
        for (&id, vs) in &self.vehicles {
            let v = &self.setup.vehicles[vs.type_idx as usize];
            let Some(sv) = &vs.served else {
                if v.served() {
                    return Err("a served gun has no crew state");
                }
                continue;
            };
            if !v.served() || sv.slots.len() != v.service.len() {
                return Err("crew state on a vehicle that is not a served gun");
            }
            if sv.hold_fire_m > 2000 || sv.state as u8 > GunState::Unhitching as u8 {
                return Err("served gun state out of range");
            }
            // The tow link, both sides; a tow or gun that just stopped being
            // a vehicle is unlinked in its next Status stage.
            let on_tow = matches!(
                sv.state,
                GunState::Hitched | GunState::Hitching | GunState::Unhitching
            );
            if on_tow != sv.hitched.is_some()
                || (ents.kind[id.index()] == EntityKind::Abandoned && on_tow)
            {
                return Err("a gun's tow link does not match its state");
            }
            if let Some(t) = sv.hitched
                && self.vehicles.get(&t).is_none_or(|x| x.towing != Some(id))
            {
                return Err("a gun's tow does not name it back");
            }
            // A turn in progress names where it ends; only then.
            if sv.relay_to.is_some() != (sv.relay_ticks > 0 || sv.state == GunState::Emplacing) {
                return Err("a gun's turn target does not match its state");
            }
            if let Some(q) = sv.crew
                && self
                    .squads
                    .get(&q)
                    .is_some_and(|s| s.owner != ents.owner[id.index()])
            {
                return Err("a gun's crew squad is of another player");
            }
            for (k, sl) in sv.slots.iter().enumerate() {
                let Some(m) = sl.man else { continue };
                let i = m.index();
                if ents.kind.get(i) != Some(&EntityKind::Soldier)
                    || ents.owner[i] != ents.owner[id.index()]
                    || ents.unit[i].serving
                        != Some(Serving {
                            gun: id,
                            slot: k as u8,
                        })
                {
                    return Err("a service slot names no man of it");
                }
            }
        }
        for (&id, vs) in &self.vehicles {
            let Some(g) = vs.towing else { continue };
            let ok = self.setup.vehicles[vs.type_idx as usize].tow.is_some()
                && self
                    .vehicles
                    .get(&g)
                    .and_then(|x| x.served.as_ref())
                    .is_some_and(|s| s.hitched == Some(id));
            if !ok {
                return Err("a tow names no gun of it");
            }
        }
        for id in ents.soldiers() {
            if let Some(s) = ents.unit[id.index()].serving {
                let ok = self
                    .vehicles
                    .get(&s.gun)
                    .and_then(|v| v.served.as_ref())
                    .and_then(|sv| sv.slots.get(s.slot as usize))
                    .is_some_and(|sl| sl.man == Some(id));
                if !ok {
                    return Err("a serving man names no slot of a gun");
                }
            }
        }
        Ok(())
    }

    /// Spawns gun `gun`'s crew and puts them at its slots (men ascending
    /// in slot order).
    pub(crate) fn spawn_gun_crew(&mut self, ctx: &mut Ctx, owner: PlayerId, gun: SimId) {
        let i = gun.index();
        let vs = &self.vehicles[&gun];
        let v = &self.setup.vehicles[vs.type_idx as usize];
        let Some(crew_t) = v.crew_squad else {
            return;
        };
        let t = ctx.ents.transform[i];
        let id = SquadId(self.next_squad);
        self.next_squad += 1;
        self.spawn_men(ctx, owner, id, crew_t, t.pos.xy(), yaw_dir(t.yaw));
        let members = self.squads[&id].members.clone();
        let vs = self.vehicles.get_mut(&gun).expect("just spawned");
        let v = &self.setup.vehicles[vs.type_idx as usize];
        let sv = vs.served.as_mut().expect("a served gun");
        sv.crew = Some(id);
        for (k, &m) in members.iter().enumerate().take(sv.slots.len()) {
            sv.slots[k].man = Some(m);
            let u = &mut ctx.ents.unit[m.index()];
            u.serving = Some(Serving { gun, slot: k as u8 });
            u.stance = stance_of(v.service[k].pose);
            ctx.hash.mark_entity_chunk(m.chunk());
        }
        self.snap_crew(ctx.ents, ctx.terrain, ctx.hash, gun);
    }

    /// Puts gun `gun`'s men exactly at their slots (spawn, and the test
    /// harness after moving a gun).
    pub(crate) fn snap_crew(
        &self,
        ents: &mut Entities,
        terrain: &crate::terrain::Heightfield,
        hash: &mut crate::hash::HashCache,
        gun: SimId,
    ) {
        let Some(vs) = self.vehicles.get(&gun) else {
            return;
        };
        let v = &self.setup.vehicles[vs.type_idx as usize];
        let Some(sv) = vs.served.as_ref() else {
            return;
        };
        let t = ents.transform[gun.index()];
        for (k, sl) in sv.slots.iter().enumerate() {
            let Some(m) = sl.man else { continue };
            let q = slot_xy(&t, v.service[k].at);
            let mi = m.index();
            ents.transform[mi].pos = q.extend(terrain.height_at(q));
            ents.transform[mi].yaw = t.yaw;
            ents.velocity[mi] = Default::default();
            ents.mover[mi].waypoint = q;
            hash.mark_entity_chunk(m.chunk());
        }
    }

    /// The Status-stage step for every served gun (see the module docs),
    /// ascending gun id.
    pub(crate) fn guns_status(&mut self, ctx: &mut Ctx, events: &mut Vec<ArmourEv>) {
        let ids: Vec<SimId> = self
            .vehicles
            .iter()
            .filter(|(_, v)| v.served.is_some())
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            self.gun_status_one(ctx, id, events);
        }
    }

    fn gun_status_one(&mut self, ctx: &mut Ctx, id: SimId, events: &mut Vec<ArmourEv>) {
        self.gun_crew_tick(ctx, id);
        if ctx.ents.kind[id.index()] != EntityKind::Vehicle {
            self.unlink_tow(ctx, id);
            return;
        }
        self.gun_tow_link(ctx, id);
        self.gun_mobility(ctx, id);
        self.gun_relay(ctx, id);
        self.gun_auto_serve(ctx, id);
        self.gun_abandon(ctx, id, events);
        if ctx.ents.kind[id.index()] == EntityKind::Vehicle
            && self
                .squads
                .values()
                .any(|q| q.order == (Order::Recrew { vehicle: id }))
        {
            self.boarding(ctx, id, events);
        }
    }

    /// Slots: lost men, auto-swap, walking to the slots, the seat mirror.
    fn gun_crew_tick(&mut self, ctx: &mut Ctx, id: SimId) {
        let MatchState {
            setup,
            vehicles,
            squads,
            ..
        } = self;
        let i = id.index();
        let vs = vehicles.get_mut(&id).expect("listed");
        let v = &setup.vehicles[vs.type_idx as usize];
        let VehicleState { seats, served, .. } = vs;
        let sv = served.as_mut().expect("listed");
        // A wreck or an abandoned gun has no crew: the men walk off as an
        // ordinary squad.
        if ctx.ents.kind[i] != EntityKind::Vehicle {
            let mut released = false;
            for sl in &mut sv.slots {
                if let Some(m) = sl.man.take()
                    && ctx.ents.kind[m.index()] == EntityKind::Soldier
                {
                    ctx.ents.unit[m.index()].serving = None;
                    ctx.ents.unit[m.index()].stance = StanceOrder::Auto;
                    ctx.hash.mark_entity_chunk(m.chunk());
                    released = true;
                }
                sl.swap = 0;
            }
            if released && let Some(sq) = sv.crew.and_then(|q| squads.get_mut(&q)) {
                let facing = sq.nav.heading;
                hold_in_place(sq, ctx.ents, facing);
            }
            sv.crew = None;
            return;
        }
        let swap_ticks = (setup.rules.vehicles.guns.slot_swap_s * setup.tick_hz)
            .clamp(1, u16::MAX as u32) as u16;
        for (k, sl) in sv.slots.iter_mut().enumerate() {
            sl.swap = sl.swap.saturating_sub(1);
            let lost = sl.man.is_some_and(|m| {
                ctx.ents.kind[m.index()] != EntityKind::Soldier
                    || ctx.ents.unit[m.index()].serving
                        != Some(Serving {
                            gun: id,
                            slot: k as u8,
                        })
            });
            if lost {
                sl.man = None;
            }
        }
        // Auto-swap: the gunner first, then the loader; the last man
        // listed (not another gunner) moves up.
        for need in [
            frontline_content::vehicle::ServiceRole::Gunner,
            frontline_content::vehicle::ServiceRole::Loader,
        ] {
            let Some(n) = v.service.iter().position(|s| s.role == need) else {
                continue;
            };
            if sv.slots[n].man.is_some() || sv.slots[n].swap > 0 {
                continue;
            }
            let donor = (0..sv.slots.len()).rev().find(|&k| {
                k != n
                    && sv.slots[k].man.is_some()
                    && sv.slots[k].swap == 0
                    && v.service[k].role != frontline_content::vehicle::ServiceRole::Gunner
            });
            let Some(d) = donor else { continue };
            let m = sv.slots[d].man.take().expect("found");
            sv.slots[n] = crate::vehicle::Slot {
                man: Some(m),
                swap: swap_ticks,
            };
            let u = &mut ctx.ents.unit[m.index()];
            u.serving = Some(Serving {
                gun: id,
                slot: n as u8,
            });
            u.stance = stance_of(v.service[n].pose);
            ctx.hash.mark_entity_chunk(m.chunk());
        }
        // Everyone at his slot (a man who just swapped walks there).
        let t = ctx.ents.transform[i];
        let speed = sv.crew.and_then(|c| squads.get(&c)).map_or(Fx::ONE, |q| {
            Fx::from(setup.squad_types[q.type_idx as usize].speed)
        });
        let step = speed.div_int(setup.tick_hz.max(1) as i32);
        for (k, sl) in sv.slots.iter().enumerate() {
            let Some(m) = sl.man else { continue };
            let mi = m.index();
            let target = slot_xy(&t, v.service[k].at);
            let p = ctx.ents.transform[mi].pos.xy();
            let d = target - p;
            let dist = d.length();
            let q = if dist <= step {
                target
            } else {
                p + d.normalize_or_zero() * step
            };
            let z = ctx.terrain.height_at(q);
            let old = ctx.ents.transform[mi];
            // Nothing to do at his slot on unchanged ground (a pit dug
            // under him lowers him).
            if dist == Fx::ZERO && old.yaw == t.yaw && old.pos.z == z {
                continue;
            }
            let tr = &mut ctx.ents.transform[mi];
            tr.pos = q.extend(z);
            tr.yaw = t.yaw;
            ctx.ents.velocity[mi] = Default::default();
            ctx.ents.mover[mi].waypoint = q;
            ctx.hash.mark_entity_chunk(m.chunk());
        }
        // The mirror.
        let trained = sv
            .crew
            .and_then(|c| squads.get(&c))
            .is_some_and(|q| setup.squad_types[q.type_idx as usize].has(Capability::GunCrew));
        let hp_max = setup.rules.vehicles.crew_hp.min(u16::MAX as u32) as u16;
        let far = Fx::HALF;
        let mut changed = false;
        for (j, st) in v.seats.iter().enumerate() {
            let slot = v
                .service
                .iter()
                .position(|s| s.role.seat() == Some(st.role));
            let new = match slot.and_then(|k| sv.slots[k].man.map(|m| (k, m))) {
                Some((k, m)) => {
                    let mi = m.index();
                    let c = &ctx.ents.combat[mi];
                    let away = (ctx.ents.transform[mi].pos.xy() - slot_xy(&t, v.service[k].at))
                        .length()
                        > far;
                    Seat {
                        hp: c.hp.min(hp_max),
                        swap: if sv.slots[k].swap > 0 {
                            sv.slots[k].swap
                        } else if away || c.pinned {
                            1
                        } else {
                            0
                        },
                        swap_from: 0,
                        untrained: !trained,
                    }
                }
                None => Seat::default(),
            };
            if seats[j] != new {
                seats[j] = new;
                changed = true;
            }
        }
        if changed {
            ctx.hash.mark_entity_chunk(id.chunk());
        }
    }

    /// S9: a penetrator through gun `id`'s plates: its core goes on along
    /// the round's line and strikes the serving men within `core.length_cm`
    /// (Soldier capsules near the gun: a small addition to the core trace).
    pub(crate) fn core_through_crew(
        &self,
        ctx: &mut DamageCtx,
        id: SimId,
        e: &BallisticEvent,
        point: FxVec3,
        attacker: u8,
        class: u8,
    ) {
        let Some(sv) = self.vehicles.get(&id).and_then(|v| v.served.as_ref()) else {
            return;
        };
        let Blow::Armour { vel, .. } = e.blow else {
            return;
        };
        let Some(a) = self.setup.ammo.get(e.ammo as usize) else {
            return;
        };
        if a.mechanism == AmmoMechanism::He {
            return;
        }
        let Some(core) = &a.core else {
            return;
        };
        let body = &self.setup.rules.combat.body;
        let dir = vel.normalize_or_zero();
        let reach = dir * Fx::from(core.length);
        let mut struck = 0u32;
        for sl in &sv.slots {
            let Some(m) = sl.man else { continue };
            let mi = m.index();
            if ctx.ents.kind[mi] != EntityKind::Soldier || struck >= core.max_boxes as u32 {
                continue;
            }
            let (a0, a1, r) = capsule(&ctx.ents.transform[mi], &ctx.ents.combat[mi], body);
            let (_, d2) = seg_seg(FxVec3::ZERO, reach, a0 - point, a1 - a0);
            if d2 > r.mul(r) {
                continue;
            }
            struck += 1;
            let dmg = core.damage.min(u16::MAX as u32) as u16;
            let c = &mut ctx.ents.combat[mi];
            c.hp = c.hp.saturating_sub(dmg);
            add_suppression(c, self.setup.rules.combat.hit_suppression * 1000);
            if c.hp == 0 {
                ctx.fallen.push((m, attacker, class));
            }
            ctx.hash.mark_entity_chunk(m.chunk());
        }
    }
}

/// S9: per-player gun counters (saved and hashed, read only by reports).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct S9Counters {
    /// Gun cards bought.
    pub guns_bought: u32,
    /// Guns that finished emplacing.
    pub guns_emplaced: u32,
    /// Of those, in woods, hull-down or a pit (Build C's siting).
    pub guns_in_cover: u32,
    /// Gun rounds fired.
    pub gun_rounds: u32,
    /// Tanks knocked out by guns.
    pub gun_ko_tanks: u32,
    /// Men killed at their slots.
    pub gun_crew_lost: u32,
    /// Own guns left to turn neutral.
    pub guns_neutral: u32,
    /// Neutral guns of another player taken.
    pub guns_captured: u32,
    /// Own guns served again after standing unmanned.
    pub guns_recrewed: u32,
    /// Guns left behind by a dead or lost tow.
    pub tows_lost: u32,
    /// AI: times a gun was unhitched at its site.
    pub ai_unhitches: u32,
    /// AI: rounds fired by a gun holding an ambush.
    pub ai_ambush_rounds: u32,
    /// AI: gun jobs given up.
    pub gun_ai_fails: u32,
    /// Enemy men and vehicles killed by guns this player captured.
    pub captured_gun_kills: u32,
}

/// Whether squad type `ty` may serve a gun (S9-7 (A): a gun crew, or
/// infantry that may re-crew vehicles, under the `any_infantry` rule).
pub fn can_serve(s: &MatchSetup, ty: &frontline_content::setup::SquadTypeSetup) -> bool {
    ty.has(Capability::GunCrew)
        || (ty.has(Capability::Recrew) && s.rules.vehicles.recrew.any_infantry != 0)
}

/// Whether live gun `id` is `p`'s and nobody serves it.
pub(crate) fn own_gun_unserved(ents: &Entities, p: PlayerId, id: SimId, vs: &VehicleState) -> bool {
    ents.kind.get(id.index()) == Some(&EntityKind::Vehicle)
        && ents.owner[id.index()] == p
        && vs
            .served
            .as_ref()
            .is_some_and(|sv| sv.slots.iter().all(|k| k.man.is_none()))
}

/// Men at the slots of `sv`.
fn men_at(sv: &crate::vehicle::Served) -> usize {
    sv.slots.iter().filter(|k| k.man.is_some()).count()
}

fn yaw_to(from: FxVec2, to: FxVec2, or: Fx) -> Fx {
    let d = to - from;
    if d == FxVec2::ZERO {
        or
    } else {
        Fx::atan2(d.y, d.x)
    }
}

impl MatchState {
    /// Whether squad `id` is a served gun's squad of one.
    pub(crate) fn is_gun_squad(&self, ents: &Entities, id: SquadId) -> bool {
        self.is_vehicle_squad(id)
            && self
                .squads
                .get(&id)
                .and_then(|q| q.members.first())
                .filter(|m| ents.kind.get(m.index()) == Some(&EntityKind::Vehicle))
                .and_then(|m| self.vehicles.get(m))
                .is_some_and(|vs| vs.served.is_some())
    }

    /// `p`'s live served guns among `ids`, ascending (S9 commands).
    fn own_guns(&self, ents: &Entities, p: PlayerId, ids: &[SimId]) -> Vec<SimId> {
        if ids.is_empty() || ids.len() > crate::unit::MAX_UNITS {
            return Vec::new();
        }
        let mut list: Vec<SimId> = ids.to_vec();
        list.sort_unstable();
        list.dedup();
        list.retain(|&id| {
            ents.kind.get(id.index()) == Some(&EntityKind::Vehicle)
                && ents.owner[id.index()] == p
                && self.vehicles.get(&id).is_some_and(|v| v.served.is_some())
        });
        list
    }

    fn squad_of_gun(&self, ents: &Entities, gun: SimId) -> Option<SquadId> {
        ents.squad.get(gun.index()).copied().flatten()
    }

    /// `Emplace` (spec 6). A limbered gun with no crew at it yet is
    /// accepted while its crew is coming; the timer runs only with
    /// `min_men` at the slots.
    pub(crate) fn order_emplace(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        guns: &[SimId],
        facing: FxVec2,
    ) -> bool {
        if !ctx.terrain.contains(facing) {
            return false;
        }
        let mut any = false;
        for gun in self.own_guns(ctx.ents, p, guns) {
            let vs = &self.vehicles[&gun];
            let sv = vs.served.as_ref().expect("own_guns");
            if !matches!(sv.state, GunState::Limbered | GunState::Pushing)
                || (men_at(sv) < self.setup.rules.vehicles.guns.min_men as usize
                    && sv.crew.is_none_or(|q| !self.squads.contains_key(&q)))
            {
                continue;
            }
            let ticks = self.setup.vehicles[vs.type_idx as usize]
                .mobility
                .towed
                .map_or(1, |t| t.emplace_ticks);
            let t = ctx.ents.transform[gun.index()];
            let yaw = yaw_to(t.pos.xy(), facing, t.yaw);
            if let Some(q) = self.squad_of_gun(ctx.ents, gun) {
                self.order_stop(ctx, p, q);
            }
            let sv = self
                .vehicles
                .get_mut(&gun)
                .and_then(|v| v.served.as_mut())
                .expect("own_guns");
            sv.state = GunState::Emplacing;
            sv.timer = ticks.clamp(1, u16::MAX as u32) as u16;
            sv.relay_to = Some(yaw);
            ctx.hash.mark_entity_chunk(gun.chunk());
            any = true;
        }
        any
    }

    /// `PackGun`: an emplaced gun packs; one still emplacing is limbered
    /// at once.
    pub(crate) fn order_pack(&mut self, ctx: &mut Ctx, p: PlayerId, guns: &[SimId]) -> bool {
        let mut any = false;
        for gun in self.own_guns(ctx.ents, p, guns) {
            let vs = self.vehicles.get_mut(&gun).expect("own_guns");
            let ticks = self.setup.vehicles[vs.type_idx as usize]
                .mobility
                .towed
                .map_or(1, |t| t.pack_ticks);
            let sv = vs.served.as_mut().expect("own_guns");
            match sv.state {
                GunState::Emplaced => {
                    sv.state = GunState::Packing;
                    sv.timer = ticks.clamp(1, u16::MAX as u32) as u16;
                    sv.relay_ticks = 0;
                    sv.relay_to = None;
                }
                GunState::Emplacing => {
                    sv.state = GunState::Limbered;
                    sv.timer = 0;
                    sv.relay_to = None;
                }
                _ => continue,
            }
            vs.target = None;
            ctx.hash.mark_entity_chunk(gun.chunk());
            any = true;
        }
        any
    }

    /// `PushGun` (spec 6): limbered guns with sound wheels, a crew and a
    /// non-zero push speed go to `point` on the wheel layer.
    pub(crate) fn order_push(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        guns: &[SimId],
        point: FxVec2,
    ) -> bool {
        let goal = ctx.terrain.clamp(point);
        let mut any = false;
        for gun in self.own_guns(ctx.ents, p, guns) {
            let vs = &self.vehicles[&gun];
            let v = &self.setup.vehicles[vs.type_idx as usize];
            let sv = vs.served.as_ref().expect("own_guns");
            if !matches!(sv.state, GunState::Limbered | GunState::Pushing)
                || Fx::from(v.mobility.speed) == Fx::ZERO
                || vs.immobilised(v)
                || men_at(sv) == 0
            {
                continue;
            }
            let Some(q) = self.squad_of_gun(ctx.ents, gun) else {
                continue;
            };
            self.squads.get_mut(&q).expect("owned").order = Order::Move { target: goal };
            self.request_path(q, goal);
            let vs = self.vehicles.get_mut(&gun).expect("own_guns");
            vs.clear_orders();
            vs.served.as_mut().expect("own_guns").state = GunState::Pushing;
            ctx.hash.mark_entity_chunk(gun.chunk());
            any = true;
        }
        any
    }

    /// The hitch point of tow `tow`: the middle of its hull's rear face.
    fn hitch_point(&self, ents: &Entities, tow: SimId) -> FxVec2 {
        let vs = &self.vehicles[&tow];
        let geo = &self.vderived.geo[vs.type_idx as usize];
        let t = &ents.transform[tow.index()];
        t.pos.xy() + yaw_dir(t.yaw) * (geo.hull_center.x - geo.hull_half.x)
    }

    /// Whether `gun`'s trail tip is within `hitch_reach_m` of `tow`'s hitch.
    fn in_hitch_reach(&self, ents: &Entities, gun: SimId, tow: SimId) -> bool {
        let vs = &self.vehicles[&gun];
        let bar = self.setup.vehicles[vs.type_idx as usize]
            .mobility
            .towed
            .map_or(Fx::ZERO, |t| Fx::from(t.tow_bar));
        let t = &ents.transform[gun.index()];
        let tip = t.pos.xy() - yaw_dir(t.yaw) * bar;
        let r = Fx::from_int(self.setup.rules.vehicles.guns.hitch_reach_m as i32);
        (tip - self.hitch_point(ents, tow)).length_sq_raw()
            <= FxVec2::new(r, Fx::ZERO).length_sq_raw()
    }

    /// `Hitch` (spec 6): the limbered gun of `guns` nearest the tow goes
    /// on the hitch of own tow `tow` (stopped, free, strong enough). The
    /// tow stops; the timer runs while it stands.
    pub(crate) fn order_hitch(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        guns: &[SimId],
        tow: SimId,
    ) -> bool {
        let ti = tow.index();
        let Some(tv) = self.vehicles.get(&tow).filter(|_| {
            ctx.ents.kind.get(ti) == Some(&EntityKind::Vehicle) && ctx.ents.owner[ti] == p
        }) else {
            return false;
        };
        let td = &self.setup.vehicles[tv.type_idx as usize];
        let (Some(rating), None) = (td.tow, tv.towing) else {
            return false;
        };
        let hitch = self.hitch_point(ctx.ents, tow);
        let chosen = self
            .own_guns(ctx.ents, p, guns)
            .into_iter()
            .filter(|&g| {
                let vs = &self.vehicles[&g];
                let sv = vs.served.as_ref().expect("own_guns");
                sv.state == GunState::Limbered
                    && rating.max_kg >= self.setup.vehicles[vs.type_idx as usize].mass_kg
                    && men_at(sv) > 0
                    && self.in_hitch_reach(ctx.ents, g, tow)
            })
            .min_by_key(|&g| {
                (
                    (ctx.ents.transform[g.index()].pos.xy() - hitch).length_sq_raw(),
                    g.0,
                )
            });
        let Some(gun) = chosen else {
            return false;
        };
        if let Some(q) = self.squad_of_gun(ctx.ents, tow) {
            self.order_stop(ctx, p, q);
        }
        if let Some(q) = self.squad_of_gun(ctx.ents, gun) {
            self.order_stop(ctx, p, q);
        }
        let ticks = self.setup.vehicles[self.vehicles[&gun].type_idx as usize]
            .mobility
            .towed
            .map_or(1, |t| t.hitch_ticks);
        let sv = self
            .vehicles
            .get_mut(&gun)
            .and_then(|v| v.served.as_mut())
            .expect("own_guns");
        sv.state = GunState::Hitching;
        sv.timer = ticks.clamp(1, u16::MAX as u32) as u16;
        sv.hitched = Some(tow);
        self.vehicles.get_mut(&tow).expect("checked").towing = Some(gun);
        ctx.hash.mark_entity_chunk(gun.chunk());
        ctx.hash.mark_entity_chunk(tow.chunk());
        true
    }

    /// `Unhitch` (spec 6): the tow stops, the crew get down, the gun is
    /// limbered once `hitch_s` has run with the tow standing.
    pub(crate) fn order_unhitch(&mut self, ctx: &mut Ctx, p: PlayerId, guns: &[SimId]) -> bool {
        let mut any = false;
        for gun in self.own_guns(ctx.ents, p, guns) {
            let vs = &self.vehicles[&gun];
            let sv = vs.served.as_ref().expect("own_guns");
            let (GunState::Hitched, Some(tow)) = (sv.state, sv.hitched) else {
                continue;
            };
            let ticks = self.setup.vehicles[vs.type_idx as usize]
                .mobility
                .towed
                .map_or(1, |t| t.hitch_ticks);
            if let Some(q) = self.squad_of_gun(ctx.ents, tow) {
                self.order_stop(ctx, p, q);
            }
            self.order_disembark(ctx, p, &[tow], false);
            let sv = self
                .vehicles
                .get_mut(&gun)
                .and_then(|v| v.served.as_mut())
                .expect("own_guns");
            sv.state = GunState::Unhitching;
            sv.timer = ticks.clamp(1, u16::MAX as u32) as u16;
            ctx.hash.mark_entity_chunk(gun.chunk());
            any = true;
        }
        any
    }

    /// `SetAmbush`: hold fire until a target is within `range_m` (0: off).
    pub(crate) fn order_ambush(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        guns: &[SimId],
        range_m: u16,
    ) -> bool {
        let list = self.own_guns(ctx.ents, p, guns);
        for &gun in &list {
            let sv = self
                .vehicles
                .get_mut(&gun)
                .and_then(|v| v.served.as_mut())
                .expect("own_guns");
            sv.hold_fire_m = range_m.min(2000);
            ctx.hash.mark_entity_chunk(gun.chunk());
        }
        !list.is_empty()
    }

    /// The first call-in of gun card `ty` (squad `q` of `p`): a card with
    /// a tow arrives hitched with the crew aboard; one without, limbered.
    pub(crate) fn gun_arrives(&mut self, ctx: &mut Ctx, p: PlayerId, ty: u16, q: SquadId) {
        let t = &self.setup.squad_types[ty as usize];
        let (Some(v), tow_card) = (t.vehicle, t.tow_card) else {
            return;
        };
        if !self.setup.vehicles[v as usize].served() {
            return;
        }
        self.players[p.0 as usize].s9.guns_bought += 1;
        let gun = self.squads[&q].members[0];
        let Some(tc) = tow_card else {
            let sv = self
                .vehicles
                .get_mut(&gun)
                .and_then(|v| v.served.as_mut())
                .expect("a gun card");
            sv.state = GunState::Limbered;
            return;
        };
        let tq = self.spawn_squad(ctx, p, tc);
        let tow = self.squads[&tq].members[0];
        self.hitch_now(ctx, gun, tow);
    }

    /// Puts `gun` on `tow`'s hitch at once, its crew aboard if it fits.
    pub(crate) fn hitch_now(&mut self, ctx: &mut Ctx, gun: SimId, tow: SimId) {
        let hitch = self.hitch_point(ctx.ents, tow);
        let tt = ctx.ents.transform[tow.index()];
        let v = &self.setup.vehicles[self.vehicles[&gun].type_idx as usize];
        let bar = v.mobility.towed.map_or(Fx::ZERO, |t| Fx::from(t.tow_bar));
        let back = yaw_dir(tt.yaw) * -Fx::ONE;
        let xy = ctx.terrain.clamp(hitch + back * bar);
        let i = gun.index();
        ctx.ents.transform[i].pos = xy.extend(ctx.terrain.height_at(xy));
        ctx.ents.transform[i].yaw = (tt.yaw + Fx::PI).wrap_angle();
        {
            let block = self.vderived.block_of(v);
            let geo = &self.vderived.geo[self.vehicles[&gun].type_idx as usize];
            let vs = self.vehicles.get_mut(&gun).expect("a gun");
            vehicle::settle(ctx.terrain, block, geo, vs, &mut ctx.ents.transform[i]);
            let sv = vs.served.as_mut().expect("a gun");
            sv.state = GunState::Hitched;
            sv.hitched = Some(tow);
            sv.timer = 0;
        }
        self.vehicles.get_mut(&tow).expect("a tow").towing = Some(gun);
        ctx.hash.mark_entity_chunk(gun.chunk());
        ctx.hash.mark_entity_chunk(tow.chunk());
        let crew = self.vehicles[&gun].served.as_ref().and_then(|s| s.crew);
        self.release_men(ctx, gun);
        if let Some(q) = crew {
            self.seat_now(ctx, q, tow);
        }
    }

    /// The men at `gun`'s slots leave them (the crew link stays).
    fn release_men(&mut self, ctx: &mut Ctx, gun: SimId) {
        let Some(sv) = self.vehicles.get_mut(&gun).and_then(|v| v.served.as_mut()) else {
            return;
        };
        let crew = sv.crew;
        for sl in &mut sv.slots {
            if let Some(m) = sl.man.take()
                && ctx.ents.kind[m.index()] == EntityKind::Soldier
            {
                ctx.ents.unit[m.index()].serving = None;
                ctx.ents.unit[m.index()].stance = StanceOrder::Auto;
                ctx.hash.mark_entity_chunk(m.chunk());
            }
            sl.swap = 0;
        }
        if let Some(sq) = crew.and_then(|q| self.squads.get_mut(&q)) {
            let facing = sq.nav.heading;
            hold_in_place(sq, ctx.ents, facing);
        }
    }

    /// An order that sends men away takes them off their guns first: the
    /// squad-level orders release the crew for good (the gun no longer
    /// names it); the unit orders release the men named.
    pub(crate) fn crew_ordered_off(&mut self, ctx: &mut Ctx, p: PlayerId, kind: &CommandKind) {
        if !self.vehicles.values().any(|v| v.served.is_some()) {
            return;
        }
        let squad_order = match kind {
            CommandKind::Move { squad, .. }
            | CommandKind::AttackMove { squad, .. }
            | CommandKind::Retreat { squad }
            | CommandKind::Dig { squad, .. }
            | CommandKind::Build { squad, .. }
            | CommandKind::IssueCrate { squad, .. }
            | CommandKind::PlaceCrate { squad, .. }
            | CommandKind::PickUpCrate { squad, .. } => Some(*squad),
            CommandKind::Recrew { squad, vehicle } => {
                let serving_it = self
                    .vehicles
                    .get(vehicle)
                    .and_then(|v| v.served.as_ref())
                    .is_some_and(|s| s.crew == Some(*squad));
                (!serving_it).then_some(*squad)
            }
            _ => None,
        };
        if let Some(q) = squad_order
            && self.squads.get(&q).is_some_and(|s| s.owner == p)
        {
            let guns: Vec<SimId> = self
                .vehicles
                .iter()
                .filter(|(_, v)| v.served.as_ref().is_some_and(|s| s.crew == Some(q)))
                .map(|(&id, _)| id)
                .collect();
            for g in guns {
                self.release_men(ctx, g);
                let sv = self
                    .vehicles
                    .get_mut(&g)
                    .and_then(|v| v.served.as_mut())
                    .expect("listed");
                sv.crew = None;
            }
        }
        let units: &[SimId] = match kind {
            CommandKind::MoveUnits { units, .. }
            | CommandKind::AttackMoveUnits { units, .. }
            | CommandKind::RetreatUnits { units }
            | CommandKind::DigUnits { units, .. }
            | CommandKind::FollowUnits { units, .. } => units,
            _ => &[],
        };
        for &m in units.iter().take(crate::unit::MAX_UNITS) {
            let i = m.index();
            if i >= ctx.ents.len()
                || ctx.ents.kind[i] != EntityKind::Soldier
                || ctx.ents.owner[i] != p
            {
                continue;
            }
            let Some(s) = ctx.ents.unit[i].serving.take() else {
                continue;
            };
            if let Some(sl) = self
                .vehicles
                .get_mut(&s.gun)
                .and_then(|v| v.served.as_mut())
                .and_then(|sv| sv.slots.get_mut(s.slot as usize))
            {
                sl.man = None;
                sl.swap = 0;
            }
            ctx.ents.unit[i].stance = StanceOrder::Auto;
            ctx.hash.mark_entity_chunk(m.chunk());
        }
    }

    /// The push speed cap of gun `id` while it is pushed: its full-crew
    /// speed × men at the slots ÷ `push_men` (at most full).
    pub(crate) fn push_speed(&self, id: SimId) -> Option<Fx> {
        let vs = self.vehicles.get(&id)?;
        let sv = vs.served.as_ref()?;
        if sv.state != GunState::Pushing {
            return None;
        }
        let v = &self.setup.vehicles[vs.type_idx as usize];
        let need = v.mobility.towed.map_or(1, |t| t.push_men).max(1) as i32;
        let men = (men_at(sv) as i32).min(need);
        Some(Fx::from(v.mobility.speed).mul_int(men).div_int(need))
    }

    /// Every hitched gun follows its tow's hitch on a rigid bar (Movement
    /// stage, after the tows drove; ascending tow id).
    pub(crate) fn tow_bars(
        &mut self,
        ents: &mut Entities,
        terrain: &crate::terrain::Heightfield,
        hash: &mut crate::hash::HashCache,
    ) {
        let pairs: Vec<(SimId, SimId)> = self
            .vehicles
            .iter()
            .filter_map(|(&tow, vs)| vs.towing.map(|g| (tow, g)))
            .collect();
        let hz = self.setup.tick_hz.max(1) as i32;
        for (tow, gun) in pairs {
            let hitched = self
                .vehicles
                .get(&gun)
                .and_then(|v| v.served.as_ref())
                .is_some_and(|s| s.state == GunState::Hitched && s.hitched == Some(tow));
            if !hitched
                || ents.kind[gun.index()] != EntityKind::Vehicle
                || ents.kind[tow.index()] != EntityKind::Vehicle
            {
                continue;
            }
            let hitch = self.hitch_point(ents, tow);
            let i = gun.index();
            let t0 = ents.transform[i];
            let mut dir = (t0.pos.xy() - hitch).normalize_or_zero();
            if dir == FxVec2::ZERO {
                dir = yaw_dir(t0.yaw);
            }
            let ty = self.vehicles[&gun].type_idx as usize;
            let v = &self.setup.vehicles[ty];
            let bar = v.mobility.towed.map_or(Fx::ZERO, |t| Fx::from(t.tow_bar));
            let xy = terrain.clamp(hitch + dir * bar);
            let block = self.vderived.block_of(v);
            let geo = &self.vderived.geo[ty];
            let vs = self.vehicles.get_mut(&gun).expect("listed");
            let t = &mut ents.transform[i];
            t.pos = xy.extend(t0.pos.z);
            t.yaw = Fx::atan2(dir.y, dir.x);
            vehicle::settle(terrain, block, geo, vs, t);
            ents.velocity[i].lin = (xy - t0.pos.xy()).mul_int(hz);
            hash.mark_entity_chunk(gun.chunk());
        }
    }

    /// The tow gone, captured or abandoned: the gun stands limbered.
    fn gun_tow_link(&mut self, ctx: &mut Ctx, id: SimId) {
        let Some(tow) = self.vehicles[&id].served.as_ref().and_then(|s| s.hitched) else {
            return;
        };
        let i = tow.index();
        if ctx.ents.kind.get(i) == Some(&EntityKind::Vehicle)
            && ctx.ents.owner[i] == ctx.ents.owner[id.index()]
            && self.vehicles.contains_key(&tow)
        {
            return;
        }
        self.unlink_tow(ctx, id);
        let vs = self.vehicles.get_mut(&id).expect("listed");
        let sv = vs.served.as_mut().expect("listed");
        sv.state = GunState::Limbered;
        sv.timer = 0;
        ctx.hash.mark_entity_chunk(id.chunk());
        let owner = ctx.ents.owner[id.index()].0 as usize;
        if let Some(p) = self.players.get_mut(owner) {
            p.s9.tows_lost += 1;
        }
    }

    /// Breaks gun `id`'s link to its tow (both sides).
    fn unlink_tow(&mut self, ctx: &mut Ctx, id: SimId) {
        let Some(sv) = self.vehicles.get_mut(&id).and_then(|v| v.served.as_mut()) else {
            return;
        };
        let Some(tow) = sv.hitched.take() else {
            return;
        };
        if let Some(t) = self.vehicles.get_mut(&tow)
            && t.towing == Some(id)
        {
            t.towing = None;
            ctx.hash.mark_entity_chunk(tow.chunk());
        }
        ctx.hash.mark_entity_chunk(id.chunk());
    }

    /// Runs the current mode change (spec 6): work needs `min_men` at the
    /// slots; hitching and unhitching run only while the tow stands.
    fn gun_mobility(&mut self, ctx: &mut Ctx, id: SimId) {
        let i = id.index();
        let min_men = self.setup.rules.vehicles.guns.min_men as usize;
        let hz = self.setup.tick_hz.max(1);
        let vs = &self.vehicles[&id];
        let v = &self.setup.vehicles[vs.type_idx as usize];
        let sv = vs.served.as_ref().expect("listed");
        let men = men_at(sv);
        let owner = ctx.ents.owner[i];
        let _ = hz;
        match sv.state {
            GunState::Emplacing if men >= min_men => {
                self.turn_gun(ctx, id, true);
                let vs = self.vehicles.get_mut(&id).expect("listed");
                let sv = vs.served.as_mut().expect("listed");
                sv.timer = sv.timer.saturating_sub(1);
                if sv.timer == 0 {
                    sv.state = GunState::Emplaced;
                    if let Some(y) = sv.relay_to.take() {
                        ctx.ents.transform[i].yaw = y;
                    }
                    self.players[owner.0 as usize].s9.guns_emplaced += 1;
                }
                ctx.hash.mark_entity_chunk(id.chunk());
            }
            GunState::Packing if men >= min_men => {
                let sv = self
                    .vehicles
                    .get_mut(&id)
                    .and_then(|v| v.served.as_mut())
                    .expect("listed");
                sv.timer = sv.timer.saturating_sub(1);
                if sv.timer == 0 {
                    sv.state = GunState::Limbered;
                }
                ctx.hash.mark_entity_chunk(id.chunk());
            }
            GunState::Pushing => {
                let done = self
                    .squad_of_gun(ctx.ents, id)
                    .and_then(|q| self.squads.get(&q))
                    .is_none_or(|q| q.nav.phase == NavPhase::Holding);
                if done || men == 0 || vs.immobilised(v) {
                    if let Some(q) = self.squad_of_gun(ctx.ents, id) {
                        self.order_stop(ctx, owner, q);
                    }
                    self.vehicles
                        .get_mut(&id)
                        .and_then(|v| v.served.as_mut())
                        .expect("listed")
                        .state = GunState::Limbered;
                    ctx.hash.mark_entity_chunk(id.chunk());
                }
            }
            GunState::Hitching => {
                let tow = sv.hitched.expect("hitching");
                if men == 0 || !self.in_hitch_reach(ctx.ents, id, tow) {
                    self.unlink_tow(ctx, id);
                    let sv = self
                        .vehicles
                        .get_mut(&id)
                        .and_then(|v| v.served.as_mut())
                        .expect("listed");
                    sv.state = GunState::Limbered;
                    sv.timer = 0;
                    return;
                }
                if men < min_men || self.vehicles[&tow].speed != Fx::ZERO {
                    return;
                }
                let timer = sv.timer.max(1);
                self.slide_to_hitch(ctx, id, tow, timer);
                let sv = self
                    .vehicles
                    .get_mut(&id)
                    .and_then(|v| v.served.as_mut())
                    .expect("listed");
                sv.timer = sv.timer.saturating_sub(1);
                ctx.hash.mark_entity_chunk(id.chunk());
                if sv.timer == 0 {
                    sv.state = GunState::Hitched;
                    self.crew_mounts(ctx, id, tow, owner);
                }
            }
            GunState::Unhitching => {
                let tow = sv.hitched.expect("unhitching");
                if self.vehicles[&tow].speed != Fx::ZERO {
                    return;
                }
                let sv = self
                    .vehicles
                    .get_mut(&id)
                    .and_then(|v| v.served.as_mut())
                    .expect("listed");
                sv.timer = sv.timer.saturating_sub(1);
                ctx.hash.mark_entity_chunk(id.chunk());
                if sv.timer == 0 {
                    sv.state = GunState::Limbered;
                    self.unlink_tow(ctx, id);
                }
            }
            _ => {}
        }
    }

    /// One tick of the gun's turn toward `relay_to` (emplacing, or the
    /// trail re-lay when `relay` is false, whose ticks left divide it).
    fn turn_gun(&mut self, ctx: &mut Ctx, id: SimId, emplacing: bool) {
        let i = id.index();
        let vs = &self.vehicles[&id];
        let sv = vs.served.as_ref().expect("listed");
        let Some(to) = sv.relay_to else {
            return;
        };
        let left = if emplacing { sv.timer } else { sv.relay_ticks }.max(1);
        let err = (to - ctx.ents.transform[i].yaw).wrap_angle();
        ctx.ents.transform[i].yaw =
            (ctx.ents.transform[i].yaw + err.div_int(left as i32)).wrap_angle();
    }

    /// Slides `id` toward the bar position behind `tow` over the rest of
    /// the hitching time.
    fn slide_to_hitch(&mut self, ctx: &mut Ctx, id: SimId, tow: SimId, timer: u16) {
        let i = id.index();
        let hitch = self.hitch_point(ctx.ents, tow);
        let vs = &self.vehicles[&id];
        let bar = self.setup.vehicles[vs.type_idx as usize]
            .mobility
            .towed
            .map_or(Fx::ZERO, |t| Fx::from(t.tow_bar));
        let t = ctx.ents.transform[i];
        let mut dir = (t.pos.xy() - hitch).normalize_or_zero();
        if dir == FxVec2::ZERO {
            dir = yaw_dir(t.yaw);
        }
        let want = hitch + dir * bar;
        let xy = ctx
            .terrain
            .clamp(t.pos.xy() + (want - t.pos.xy()).div_int(timer as i32));
        let yaw_err = (Fx::atan2(dir.y, dir.x) - t.yaw).wrap_angle();
        let tr = &mut ctx.ents.transform[i];
        tr.pos = xy.extend(ctx.terrain.height_at(xy));
        tr.yaw = (t.yaw + yaw_err.div_int(timer as i32)).wrap_angle();
    }

    /// Hitched: the crew leave the slots and ride the tow, or follow it
    /// on foot when it has no room.
    fn crew_mounts(&mut self, ctx: &mut Ctx, gun: SimId, tow: SimId, p: PlayerId) {
        let crew = self.vehicles[&gun].served.as_ref().and_then(|s| s.crew);
        self.release_men(ctx, gun);
        let Some(q) = crew.filter(|q| self.squads.contains_key(q)) else {
            return;
        };
        if !self.order_embark(ctx, p, &[q], tow) {
            let men = self.squads[&q].members.clone();
            self.units_follow(ctx, p, &men, tow);
        }
    }

    /// Emplaced: a trail re-lay runs, or starts for a target outside the
    /// arc (`lay_turn`) or a `FaceUnits` order.
    fn gun_relay(&mut self, ctx: &mut Ctx, id: SimId) {
        let i = id.index();
        let min_men = self.setup.rules.vehicles.guns.min_men as usize;
        let vs = self.vehicles.get_mut(&id).expect("listed");
        let face = vs.face.take();
        let lay = vs.lay_turn;
        let attacked = vs.last_attacker.is_some();
        let ticks = self.setup.vehicles[vs.type_idx as usize]
            .mobility
            .towed
            .map_or(1, |t| t.relay_trail_ticks);
        let sv = vs.served.as_mut().expect("listed");
        if sv.state != GunState::Emplaced {
            return;
        }
        let men = men_at(sv);
        if sv.relay_ticks > 0 {
            if men >= min_men {
                self.turn_gun(ctx, id, false);
                let sv = self
                    .vehicles
                    .get_mut(&id)
                    .and_then(|v| v.served.as_mut())
                    .expect("listed");
                sv.relay_ticks -= 1;
                if sv.relay_ticks == 0
                    && let Some(y) = sv.relay_to.take()
                {
                    ctx.ents.transform[i].yaw = y;
                }
                ctx.hash.mark_entity_chunk(id.chunk());
            }
            return;
        }
        let t = ctx.ents.transform[i];
        let to = match face {
            Some(p) => Some(yaw_to(t.pos.xy(), p, t.yaw)),
            None if lay.is_some() && (sv.hold_fire_m == 0 || attacked) => lay,
            None => None,
        };
        if let Some(to) = to
            && (to - t.yaw).wrap_angle().abs() > Fx::from_milli(2)
            && men >= min_men
        {
            sv.relay_to = Some(to);
            sv.relay_ticks = ticks.clamp(1, u16::MAX as u32) as u16;
            ctx.hash.mark_entity_chunk(id.chunk());
        }
    }

    /// A linked crew standing idle (dismounted, its tow lost) goes back
    /// to its gun.
    fn gun_auto_serve(&mut self, ctx: &mut Ctx, id: SimId) {
        let sv = self.vehicles[&id].served.as_ref().expect("listed");
        if matches!(sv.state, GunState::Hitched | GunState::Hitching) || men_at(sv) > 0 {
            return;
        }
        let Some(q) = sv.crew else { return };
        let Some(sq) = self.squads.get(&q) else {
            return;
        };
        let idle = sq.order == Order::Idle
            || matches!(sq.order, Order::Follow { vehicle } if Some(vehicle) == sv.hitched);
        if !idle || sq.members.is_empty() || self.squad_rides(ctx.ents, q) {
            return;
        }
        self.walk_to_gun(ctx, q, id);
    }

    /// Squad `q` walks to gun `id` (a `Recrew` order).
    pub(crate) fn walk_to_gun(&mut self, ctx: &mut Ctx, q: SquadId, id: SimId) {
        let vpos = ctx.ents.transform[id.index()].pos.xy();
        let lead = self.squads[&q].lead(ctx.ents);
        let from = ctx.ents.transform[lead.index()].pos.xy();
        let vs = &self.vehicles[&id];
        let half = self.vderived.geo[vs.type_idx as usize].hull_half.y;
        let mut dir = (from - vpos).normalize_or_zero();
        if dir == FxVec2::ZERO {
            dir = FxVec2::new(Fx::ONE, Fx::ZERO);
        }
        let goal = ctx.terrain.clamp(vpos + dir * (half + Fx::ONE));
        self.squads.get_mut(&q).expect("live").order = Order::Recrew { vehicle: id };
        self.request_path(q, goal);
    }

    /// A gun with nobody near turns neutral after `abandon_s`.
    fn gun_abandon(&mut self, ctx: &mut Ctx, id: SimId, events: &mut Vec<ArmourEv>) {
        let i = id.index();
        let hz = self.setup.tick_hz.max(1);
        let g = &self.setup.rules.vehicles.guns;
        let need = (g.abandon_s * hz).clamp(1, u16::MAX as u32) as u16;
        let hold = g.hold_m;
        let owner = ctx.ents.owner[i];
        let pos = ctx.ents.transform[i].pos.xy();
        let sv = self.vehicles[&id].served.as_ref().expect("listed");
        let state = sv.state;
        let held = men_at(sv) > 0
            || matches!(
                state,
                GunState::Hitched | GunState::Hitching | GunState::Unhitching
            );
        let r = FxVec2::new(Fx::from_int(hold as i32), Fx::ZERO).length_sq_raw();
        let near = !held
            && ctx.ents.soldiers().any(|m| {
                ctx.ents.owner[m.index()] == owner
                    && ctx.ents.unit[m.index()].riding.is_none()
                    && (ctx.ents.transform[m.index()].pos.xy() - pos).length_sq_raw() <= r
            });
        let sv = self
            .vehicles
            .get_mut(&id)
            .and_then(|v| v.served.as_mut())
            .expect("listed");
        if held || near {
            if sv.abandon_ticks != 0 {
                sv.abandon_ticks = 0;
                ctx.hash.mark_entity_chunk(id.chunk());
            }
            return;
        }
        sv.abandon_ticks += 1;
        ctx.hash.mark_entity_chunk(id.chunk());
        if sv.abandon_ticks < need {
            return;
        }
        // Neutral: mode changes under way stop; rounds, parts and state stay.
        sv.state = match sv.state {
            GunState::Emplacing | GunState::Pushing => GunState::Limbered,
            GunState::Packing => GunState::Emplaced,
            s => s,
        };
        sv.timer = 0;
        sv.relay_ticks = 0;
        sv.relay_to = None;
        sv.abandon_ticks = 0;
        sv.crew = None;
        sv.hold_fire_m = 0;
        let vs = self.vehicles.get_mut(&id).expect("listed");
        for st in &mut vs.seats {
            *st = Seat::default();
        }
        if let Some(p) = self.players.get_mut(owner.0 as usize) {
            p.s9.guns_neutral += 1;
        }
        self.abandon(ctx, id, crate::bail::AB_GUN, events);
    }

    /// The men `men` of squad `q` serve gun `id`: a neutral gun comes
    /// alive as the squad owner's, a new squad of one; the men stay
    /// soldiers of their squad.
    pub(crate) fn man_gun(
        &mut self,
        ctx: &mut Ctx,
        id: SimId,
        q: SquadId,
        men: &[SimId],
        events: &mut Vec<ArmourEv>,
    ) {
        let i = id.index();
        let p = self.squads[&q].owner;
        let last = ctx.ents.owner[i];
        let neutral = ctx.ents.kind[i] == EntityKind::Abandoned;
        let type_idx = self.vehicles[&id].type_idx;
        // The gun: neutral → live, this player's.
        if neutral {
            let vt = self
                .setup
                .squad_types
                .iter()
                .position(|t| t.vehicle == Some(type_idx))
                .expect("a gun card") as u16;
            let sid = SquadId(self.next_squad);
            self.next_squad += 1;
            let t = ctx.ents.transform[i];
            let facing = yaw_dir(t.yaw);
            ctx.ents.kind[i] = EntityKind::Vehicle;
            ctx.ents.owner[i] = p;
            ctx.ents.squad[i] = Some(sid);
            ctx.ents.combat[i].seen_by = 1 << p.0;
            ctx.hash.mark_entity_chunk(id.chunk());
            self.squads.insert(
                sid,
                Squad {
                    type_idx: vt,
                    owner: p,
                    members: vec![id],
                    order: Order::Idle,
                    nav: Nav {
                        phase: NavPhase::Holding,
                        goal: t.pos.xy(),
                        heading: facing,
                        facing,
                        slots: vec![t.pos.xy()],
                        ..Nav::default()
                    },
                    ranging: Vec::new(),
                    focus: None,
                    launcher: None,
                    drops: Vec::new(),
                    pzf: None,
                },
            );
            let vs = self.vehicles.get_mut(&id).expect("listed");
            vs.abandoned = None;
            vs.abandon_cause = 0;
            vs.board = None;
            vs.crew_squad = None;
            vs.progress_at = t.pos.xy();
            vs.progress_ticks = 0;
            vs.last_attacker = None;
            let captured = p != last;
            if let Some(pl) = self.players.get_mut(p.0 as usize) {
                if captured {
                    pl.crew.captured += 1;
                    pl.s9.guns_captured += 1;
                } else {
                    pl.crew.recrewed += 1;
                    pl.s9.guns_recrewed += 1;
                }
            }
            self.forget(ctx.ents, ctx.hash, id);
            self.mark_wreck_block(ctx.ents, id);
            let ev = ArmourEv::new(ArmourEventKind::Recrewed, id, p.0, last.0)
                .component(if captured {
                    crate::bail::RECREW_CAPTURED
                } else {
                    crate::bail::RECREW_OWN
                })
                .at(t.pos);
            self.push_event(events, ev);
        }
        // The men, lowest slots first (the slots are in priority order).
        let vs = self.vehicles.get_mut(&id).expect("listed");
        let v = &self.setup.vehicles[vs.type_idx as usize];
        let sv = vs.served.as_mut().expect("a gun");
        sv.crew = Some(q);
        sv.hold_fire_m = 0;
        sv.abandon_ticks = 0;
        vs.board = None;
        let mut it = men.iter().copied();
        for (k, sl) in sv.slots.iter_mut().enumerate() {
            if sl.man.is_some() {
                continue;
            }
            let Some(m) = it.next() else { break };
            sl.man = Some(m);
            sl.swap = 0;
            let u = &mut ctx.ents.unit[m.index()];
            u.serving = Some(Serving {
                gun: id,
                slot: k as u8,
            });
            u.stance = stance_of(v.service[k].pose);
            u.detached = false;
            ctx.hash.mark_entity_chunk(m.chunk());
        }
        ctx.hash.mark_entity_chunk(id.chunk());
        if let Some(sq) = self.squads.get_mut(&q) {
            sq.order = Order::Idle;
            let facing = sq.nav.heading;
            hold_in_place(sq, ctx.ents, facing);
        }
    }

    /// World tool ([`crate::World::hitch_gun_now`]): puts `gun` on `tow`'s
    /// hitch at once if it may.
    pub(crate) fn hitch_gun_now(&mut self, ctx: &mut Ctx, gun: SimId, tow: SimId) -> bool {
        let gun_ok = ctx.ents.kind.get(gun.index()) == Some(&EntityKind::Vehicle)
            && self.vehicles.get(&gun).is_some_and(|v| v.served.is_some());
        let tow_ok = ctx.ents.kind.get(tow.index()) == Some(&EntityKind::Vehicle)
            && self.vehicles.get(&tow).is_some_and(|v| {
                v.towing.is_none() && self.setup.vehicles[v.type_idx as usize].tow.is_some()
            });
        if !gun_ok || !tow_ok || ctx.ents.owner[gun.index()] != ctx.ents.owner[tow.index()] {
            return false;
        }
        self.hitch_now(ctx, gun, tow);
        true
    }
}
