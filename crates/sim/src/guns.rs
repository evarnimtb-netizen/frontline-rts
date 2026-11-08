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
//! Build B adds emplace, pack, push, hitch and the neutral gun.

use frontline_content::setup::Capability;
use frontline_content::vehicle::ServicePose;
use frontline_content::{AmmoMechanism, MatchSetup};

use crate::combat::{BallisticEvent, Blow, DamageCtx, add_suppression, capsule, seg_seg};
use crate::ecs::{Entities, EntityKind, PlayerId, SimId, Transform};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::SquadId;
use crate::game::{Ctx, MatchState};
use crate::unit::{Serving, StanceOrder};
use crate::vehicle::{GunState, Seat, VehicleState, yaw_dir};

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
    if vs.target.is_some() {
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
            if sv.hold_fire_m > 2000 || sv.state as u8 > GunState::Hitching as u8 {
                return Err("served gun state out of range");
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
    pub(crate) fn guns_status(&mut self, ctx: &mut Ctx) {
        let ids: Vec<SimId> = self
            .vehicles
            .iter()
            .filter(|(_, v)| v.served.is_some())
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            self.gun_status_one(ctx, id);
        }
    }

    fn gun_status_one(&mut self, ctx: &mut Ctx, id: SimId) {
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
            if dist == Fx::ZERO && ctx.ents.transform[mi].yaw == t.yaw {
                continue;
            }
            let q = if dist <= step {
                target
            } else {
                p + d.normalize_or_zero() * step
            };
            let z = ctx.terrain.height_at(q);
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
