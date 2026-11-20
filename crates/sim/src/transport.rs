//! S8a transport (vehicles.md 6.6, lean; `docs/slices/S8a-halftracks.md`
//! 5): infantry riding half-tracks.
//!
//! **Passengers stay soldiers.** A man aboard keeps his entity, squad, HP,
//! kit and suppression; `Unit::riding` names the vehicle and the passenger
//! slot, and the vehicle's `VehicleState::riders` names him back. He is not
//! in the soldier spatial hash, plans no velocity, is not steered, does not
//! fire or throw, and neither captures nor contests a point. After the
//! vehicles move (Movement stage) his transform is set to the bottom of his
//! slot box ([`MatchState::follow_slots`]). His hit volume is the slot box
//! (`VehicleFrame::riders`): struck from outside when it rises over the
//! rim (`HitVia::Rider`), by cores, bursts and fragments inside
//! (`Inner::Rider`), and by blasts as an exposed seat. Rounds stopped by
//! his vehicle suppress him (a near miss). He observes from his slot's top
//! (`crate::spotting`) and is seen while his vehicle is seen and his slot
//! rises over the rim.
//!
//! **Embark** (`Embark { squads, vehicle }`): an own live transport that is
//! not burning or unloading; a squad whose men do not fit the slots not
//! promised to other squads is refused (squads never split, S8a-2). The
//! squad walks to the door (`[transport] entry`, order `Embark`); while
//! the vehicle is no faster than `move_block_cm_s` one man within
//! `board_reach_cm` of the door boards per `board_ms` (squads ascending,
//! members in order, the lowest free slot). A mounted squad takes no squad
//! or soldier orders (`MatchState::apply`); its vehicle drives.
//!
//! **Disembark** (`Disembark { vehicles, over_sides }`): the vehicle stops;
//! one man per `exit_ms` gets out of the door (lowest slot first), or per
//! `exit_over_sides_ms` over the side away from the last round that struck
//! it (his box raised `exit_raise_cm` for his exit time). A squad whose
//! last rider is out holds where its men stand (order `Idle`).
//!
//! **Knock-out, abandon, fire.** All riders of a wreck, an abandoned or a
//! burning vehicle get out at once on their own side, unable to fire for
//! `stun_ms` and with `[vehicles.bail] bail_suppression`.
//!
//! The Status stage runs [`MatchState::transport_status`] after the
//! vehicles' own status, serially in ascending vehicle id.

use frontline_content::vehicle::TransportSetup;

use crate::ecs::{Entities, EntityKind, PlayerId, SimId};
use crate::fx::{Fx, FxVec2};
use crate::game::{Ctx, MatchState, NavPhase, Order, SquadId};
use crate::hash::HashCache;
use crate::unit::Riding;
use crate::vehicle::{Trip, Unload, VehicleState};

/// How a man gets out.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Exit {
    Door,
    Side,
    /// Everyone at once (knock-out, abandon, fire): his own side, stunned.
    Forced,
}

/// The slot of the man getting out over the side now (his box is raised),
/// if any: the lowest occupied slot while a side unload's door timer runs.
pub(crate) fn exiting_over_side(vs: &VehicleState) -> Option<usize> {
    match vs.unload {
        Some(u) if u.over_sides && vs.door_ticks > 0 => vs.riders.iter().position(|r| r.is_some()),
        _ => None,
    }
}

/// Unit vector of heading `yaw`.
fn yaw_dir(yaw: Fx) -> FxVec2 {
    let (s, c) = yaw.sin_cos();
    FxVec2::new(c, s)
}

/// Squared metres as raw Q32.
fn sq_m(m: Fx) -> u64 {
    (m.raw().max(0) as u64).pow(2)
}

impl MatchState {
    /// S8a: the transport def of vehicle `id`, if it is one.
    pub fn transport_of(&self, id: SimId) -> Option<&TransportSetup> {
        let vs = self.vehicles.get(&id)?;
        self.setup.vehicles[vs.type_idx as usize].transport.as_ref()
    }

    /// S8a: the height of rider `r`'s slot box (his eyes are the rules'
    /// `eye_drop_mm` below its top).
    pub(crate) fn rider_height(&self, r: Riding) -> Fx {
        self.vehicles
            .get(&r.vehicle)
            .and_then(|vs| self.vderived.geo.get(vs.type_idx as usize))
            .and_then(|g| g.slots.get(r.slot as usize))
            .map_or(Fx::ZERO, |s| s.1.z.mul_int(2))
    }

    /// S8a: whether rider `r`'s slot rises over its box's rim (S8a-9).
    pub(crate) fn rider_exposed(&self, r: Riding) -> bool {
        self.vehicles
            .get(&r.vehicle)
            .and_then(|vs| self.vderived.geo.get(vs.type_idx as usize))
            .is_some_and(|g| (r.slot as u32) < 16 && g.slots_above_rim & (1 << r.slot) != 0)
    }

    /// S8a: whether any man of squad `id` rides a transport.
    pub fn squad_rides(&self, ents: &Entities, id: SquadId) -> bool {
        self.squads.get(&id).is_some_and(|sq| {
            sq.members
                .iter()
                .any(|m| ents.unit[m.index()].riding.is_some())
        })
    }

    /// S8a: the riders of vehicle `id`, slot order.
    pub fn riders_of(&self, id: SimId) -> Vec<SimId> {
        self.vehicles
            .get(&id)
            .map(|vs| vs.riders.iter().flatten().copied().collect())
            .unwrap_or_default()
    }

    /// The door of vehicle `id` on the ground, a metre out from the hull
    /// (where men walk to and step out).
    pub(crate) fn door_spot(&self, ents: &Entities, id: SimId) -> Option<(FxVec2, FxVec2)> {
        let t = self.transport_of(id)?;
        let f = self.vderived.frame(id)?;
        let tr = &ents.transform[id.index()];
        let fwd = yaw_dir(tr.yaw);
        let door = crate::damage::anchor_at(
            f,
            frontline_content::Frame::Hull,
            crate::vehicle::v3(t.entry),
        );
        let mut out = (door.xy() - f.hull.center.xy()).normalize_or_zero();
        if out == FxVec2::ZERO {
            out = -fwd;
        }
        Some((door.xy() + out, out))
    }

    /// S8a: men promised to transport `id`: the squads walking to it or
    /// riding in it (all their men).
    fn promised(&self, id: SimId) -> u32 {
        self.squads
            .values()
            .filter(|sq| sq.order == (Order::Embark { vehicle: id }))
            .map(|sq| sq.members.len() as u32)
            .sum()
    }

    /// Whether `id` is a live transport of `p` men may board now.
    fn boardable(&self, ents: &Entities, p: PlayerId, id: SimId) -> bool {
        let i = id.index();
        ents.kind.get(i) == Some(&EntityKind::Vehicle)
            && ents.owner[i] == p
            && self.transport_of(id).is_some()
            && self
                .vehicles
                .get(&id)
                .is_some_and(|vs| vs.burn == 0 && vs.ammo_fire == 0 && vs.unload.is_none())
    }

    /// `Embark` (module docs). Accepted when any squad was.
    pub(crate) fn order_embark(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        squads: &[SquadId],
        vehicle: SimId,
    ) -> bool {
        if squads.is_empty()
            || squads.len() > crate::unit::MAX_UNITS
            || !self.boardable(ctx.ents, p, vehicle)
        {
            return false;
        }
        let cap = self
            .transport_of(vehicle)
            .map_or(0, |t| t.slots.len() as u32);
        let mut promised = self.promised(vehicle);
        let mut list = squads.to_vec();
        list.sort_unstable();
        list.dedup();
        let Some((door, _)) = self.door_spot(ctx.ents, vehicle) else {
            return false;
        };
        let goal = ctx.terrain.clamp(door);
        let mut any = false;
        for sid in list {
            if !self.owned(p, sid) || self.is_vehicle_squad(sid) {
                continue;
            }
            let sq = &self.squads[&sid];
            if sq.order == (Order::Embark { vehicle }) || self.squad_rides(ctx.ents, sid) {
                continue;
            }
            let n = sq.members.len() as u32;
            if promised + n > cap {
                continue;
            }
            promised += n;
            self.crew_leaves_guns(ctx, sid, Some(vehicle));
            self.clear_detached(ctx, sid);
            self.squads.get_mut(&sid).expect("owned").order = Order::Embark { vehicle };
            self.request_path(sid, goal);
            any = true;
        }
        any
    }

    /// `Disembark` (module docs). Accepted when any vehicle was.
    pub(crate) fn order_disembark(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        vehicles: &[SimId],
        over_sides: bool,
    ) -> bool {
        if vehicles.is_empty() || vehicles.len() > crate::unit::MAX_UNITS {
            return false;
        }
        let mut list = vehicles.to_vec();
        list.sort_unstable();
        list.dedup();
        let mut any = false;
        for id in list {
            let i = id.index();
            if ctx.ents.kind.get(i) != Some(&EntityKind::Vehicle) || ctx.ents.owner[i] != p {
                continue;
            }
            let Some(t) = self.transport_of(id) else {
                continue;
            };
            let sides = over_sides && t.exit_over_sides_ticks.is_some();
            let Some(vs) = self.vehicles.get(&id) else {
                continue;
            };
            if vs.riders.iter().all(|r| r.is_none()) {
                continue;
            }
            let under_fire = self.under_fire(ctx.ents, id, ctx.tick);
            if let Some(sq) = ctx.ents.squad[i] {
                self.order_stop(ctx, p, sq);
            }
            let vs = self.vehicles.get_mut(&id).expect("listed");
            // The exit timer starts now (a boarding pause ends).
            if vs.unload.is_none() {
                vs.door_ticks = 0;
            }
            vs.unload = Some(Unload {
                over_sides: sides,
                under_fire: under_fire || vs.unload.is_some_and(|u| u.under_fire),
            });
            any = true;
        }
        any
    }

    /// S8a: whether transport `id` is under fire: a penetration within
    /// `[ai.transport] hit_s`, or a rider suppressed past
    /// `rider_suppression_pct` (the AI's and the report's rule).
    pub fn under_fire(&self, ents: &Entities, id: SimId, tick: u64) -> bool {
        let Some(vs) = self.vehicles.get(&id) else {
            return false;
        };
        let a = &self.setup.rules.ai.transport;
        let hz = self.setup.tick_hz.max(1) as u64;
        let hit = vs
            .last_pen
            .is_some_and(|t| tick.saturating_sub(t) <= a.hit_s as u64 * hz);
        hit || vs
            .riders
            .iter()
            .flatten()
            .any(|m| ents.combat[m.index()].suppression >= a.rider_suppression_pct * 1000)
    }

    /// Movement stage, after the vehicles moved and their frames were
    /// rebuilt: every rider stands at the bottom of his slot box, facing
    /// with the hull, moving with it.
    pub(crate) fn follow_slots(&self, ents: &mut Entities, hash: &mut HashCache) {
        for (&id, vs) in &self.vehicles {
            if vs.riders.iter().all(|r| r.is_none()) {
                continue;
            }
            let Some(f) = self.vderived.frame(id) else {
                continue;
            };
            let vel = ents.velocity[id.index()];
            for n in 0..f.n_riders as usize {
                let Some(m) = vs.riders.get(f.rider_slot[n] as usize).copied().flatten() else {
                    continue;
                };
                let b = &f.riders[n];
                let pos = b.center - b.axes.z * b.half.z;
                let i = m.index();
                let t = &mut ents.transform[i];
                if t.pos != pos || t.yaw != f.yaw || ents.velocity[i] != vel {
                    t.pos = pos;
                    t.yaw = f.yaw;
                    ents.velocity[i] = vel;
                    hash.mark_entity_chunk(m.chunk());
                }
            }
        }
    }

    /// The Status-stage part (module docs): squads walking to their
    /// vehicle, then per transport (ascending id) stale slots, forced
    /// dismounts, unloading and boarding, and the end of a trip.
    pub(crate) fn transport_status(&mut self, ctx: &mut Ctx) {
        self.embark_upkeep(ctx);
        let ids: Vec<SimId> = self
            .vehicles
            .iter()
            .filter(|(_, vs)| !vs.riders.is_empty())
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            self.clean_riders(ctx, id);
            let kind = ctx.ents.kind[id.index()];
            let vs = &self.vehicles[&id];
            let forced = kind != EntityKind::Vehicle || vs.burn > 0 || vs.ammo_fire > 0;
            if forced {
                let slots: Vec<usize> = (0..vs.riders.len())
                    .filter(|&k| vs.riders[k].is_some())
                    .collect();
                for k in slots {
                    self.rider_out(ctx, id, k, Exit::Forced);
                }
                let vs = self.vehicles.get_mut(&id).expect("listed");
                vs.unload = None;
                vs.door_ticks = 0;
            } else {
                self.door(ctx, id);
            }
            self.end_trip(ctx, id);
        }
    }

    /// Drops slots that no longer hold their rider (a man killed: his
    /// corpse no longer rides) and the reverse.
    fn clean_riders(&mut self, ctx: &mut Ctx, id: SimId) {
        let vs = self.vehicles.get_mut(&id).expect("listed");
        for (k, r) in vs.riders.iter_mut().enumerate() {
            let Some(m) = *r else { continue };
            let i = m.index();
            let ok = ctx.ents.kind.get(i) == Some(&EntityKind::Soldier)
                && ctx.ents.unit[i].riding
                    == Some(Riding {
                        vehicle: id,
                        slot: k as u8,
                    });
            if !ok {
                *r = None;
            }
        }
    }

    /// Unloading or boarding at the door of live transport `id`.
    fn door(&mut self, ctx: &mut Ctx, id: SimId) {
        let tt = &self.setup.rules.vehicles.transport;
        let (block_cm_s, reach_cm) = (tt.move_block_cm_s, tt.board_reach_cm);
        let Some(t) = self.transport_of(id) else {
            return;
        };
        let (board_ticks, exit_ticks, side_ticks) = (
            t.board_ticks.max(1),
            t.exit_ticks.max(1),
            t.exit_over_sides_ticks.unwrap_or(t.exit_ticks).max(1),
        );
        let block = Fx::from_ratio(block_cm_s as i64, 100);
        let vs = &self.vehicles[&id];
        // Nobody climbs in or out of a moving vehicle.
        if vs.speed.abs() > block {
            return;
        }
        if let Some(u) = vs.unload {
            let per = if u.over_sides { side_ticks } else { exit_ticks };
            let per = per.min(u16::MAX as u32) as u16;
            let vs = self.vehicles.get_mut(&id).expect("listed");
            if vs.door_ticks == 0 {
                vs.door_ticks = per;
            }
            vs.door_ticks -= 1;
            if vs.door_ticks > 0 {
                return;
            }
            let Some(k) = vs.riders.iter().position(|r| r.is_some()) else {
                vs.unload = None;
                return;
            };
            let how = if u.over_sides { Exit::Side } else { Exit::Door };
            self.rider_out(ctx, id, k, how);
            let vs = self.vehicles.get_mut(&id).expect("listed");
            if vs.riders.iter().any(|r| r.is_some()) {
                vs.door_ticks = per;
            } else {
                vs.unload = None;
                vs.door_ticks = 0;
            }
            return;
        }
        let vs = self.vehicles.get_mut(&id).expect("listed");
        if vs.door_ticks > 0 {
            vs.door_ticks -= 1;
            return;
        }
        // The next man within reach of the door: squads ascending, men in
        // squad order.
        let Some((door, _)) = self.door_spot(ctx.ents, id) else {
            return;
        };
        let reach = Fx::from_ratio(reach_cm as i64, 100);
        let reach_sq = sq_m(reach + Fx::ONE);
        let owner = ctx.ents.owner[id.index()];
        let mut next: Option<(SquadId, SimId)> = None;
        for (&sid, sq) in &self.squads {
            if sq.owner != owner || sq.order != (Order::Embark { vehicle: id }) {
                continue;
            }
            let m = sq.members.iter().copied().find(|m| {
                let i = m.index();
                ctx.ents.unit[i].riding.is_none()
                    && (ctx.ents.transform[i].pos.xy() - door).length_sq_raw() <= reach_sq
            });
            if let Some(m) = m {
                next = Some((sid, m));
                break;
            }
        }
        let Some((sid, m)) = next else {
            return;
        };
        let vs = &self.vehicles[&id];
        let Some(slot) = vs.riders.iter().position(|r| r.is_none()) else {
            // No room after all: the squad stops walking to it.
            self.embark_off(ctx, sid);
            return;
        };
        self.board(ctx, id, sid, m, slot, board_ticks);
    }

    /// S8a tools and tests ([`crate::World::seat_now`]): every man of
    /// squad `sid` boards transport `id` at once.
    pub(crate) fn seat_now(&mut self, ctx: &mut Ctx, sid: SquadId, id: SimId) -> bool {
        let Some(sq) = self.squads.get(&sid) else {
            return false;
        };
        let owner = sq.owner;
        let men = sq.members.clone();
        if self.is_vehicle_squad(sid) || !self.boardable(ctx.ents, owner, id) {
            return false;
        }
        let free = self.vehicles[&id]
            .riders
            .iter()
            .filter(|r| r.is_none())
            .count();
        if men.len() > free || self.squad_rides(ctx.ents, sid) {
            return false;
        }
        self.clear_detached(ctx, sid);
        self.squads.get_mut(&sid).expect("listed").order = Order::Embark { vehicle: id };
        for m in men {
            let slot = self.vehicles[&id]
                .riders
                .iter()
                .position(|r| r.is_none())
                .expect("room checked");
            self.board(ctx, id, sid, m, slot, 0);
        }
        for q in &mut self.path_queue.queues {
            q.retain(|r| *r != crate::game::PathReq::Squad(sid));
        }
        let sq = self.squads.get_mut(&sid).expect("listed");
        sq.nav.phase = NavPhase::Holding;
        sq.nav.path.clear();
        sq.nav.slots = sq
            .members
            .iter()
            .map(|q| ctx.ents.transform[q.index()].pos.xy())
            .collect();
        true
    }

    /// Soldier `m` of squad `sid` takes slot `slot` of transport `id`.
    fn board(
        &mut self,
        ctx: &mut Ctx,
        id: SimId,
        sid: SquadId,
        m: SimId,
        slot: usize,
        board_ticks: u32,
    ) {
        let first_of_squad = !self.squad_rides(ctx.ents, sid);
        let walk_mm_s = {
            let ty = &self.setup.squad_types[self.squads[&sid].type_idx as usize];
            ((Fx::from(ty.speed).raw().max(0) as u64 * 1000) >> Fx::FRAC_BITS) as u32
        };
        let i = m.index();
        ctx.ents.unit[i].riding = Some(Riding {
            vehicle: id,
            slot: slot as u8,
        });
        ctx.ents.kit[i].windup = None;
        ctx.ents.combat[i].target = None;
        ctx.ents.velocity[i] = Default::default();
        ctx.hash.mark_entity_chunk(m.chunk());
        let vpos = ctx.ents.transform[id.index()].pos.xy();
        let tick = ctx.tick;
        let vs = self.vehicles.get_mut(&id).expect("listed");
        vs.riders[slot] = Some(m);
        vs.door_ticks = board_ticks.min(u16::MAX as u32) as u16;
        let trip = vs.trip.get_or_insert(Trip {
            from: vpos,
            since: tick,
            squads: 0,
            walk_mm_s,
        });
        if first_of_squad {
            trip.squads = trip.squads.saturating_add(1);
        }
        // He sits down at once (the Movement stage keeps him there).
        if let Some(f) = self.vderived.frame(id) {
            let geo = &self.vderived.geo[self.vehicles[&id].type_idx as usize];
            let (c, h, _) = geo.slots[slot];
            let axes = f.hull.axes;
            let t = &mut ctx.ents.transform[i];
            t.pos = f.origin + axes.apply(c) - axes.z * h.z;
            t.yaw = f.yaw;
        }
    }

    /// The man in slot `k` of transport `id` gets out (module docs).
    fn rider_out(&mut self, ctx: &mut Ctx, id: SimId, k: usize, how: Exit) {
        let Some(m) = self.vehicles[&id].riders[k] else {
            return;
        };
        let vs = &self.vehicles[&id];
        let geo = &self.vderived.geo[vs.type_idx as usize];
        let tr = ctx.ents.transform[id.index()];
        let fwd = yaw_dir(tr.yaw);
        let left = fwd.perp();
        let (slot_c, _, _) = geo.slots[k];
        let gap = Fx::from_ratio(7, 10) + Fx::from_ratio(6, 10).mul_int((k % 2) as i32);
        let side_of =
            |sign: Fx| tr.pos.xy() + fwd * slot_c.x + left * ((geo.hull_half.y + gap).mul(sign));
        let (spot, facing) = match how {
            Exit::Door => {
                let (door, out) = self.door_spot(ctx.ents, id).unwrap_or((tr.pos.xy(), -fwd));
                let lat = out.perp();
                let j = k as i32;
                let spot = door
                    + out * Fx::from_ratio(8, 10).mul_int(j / 3)
                    + lat * Fx::from_ratio(8, 10).mul_int(j % 3 - 1);
                (spot, out)
            }
            Exit::Side => {
                // Away from the shooter: the side the last round travelled
                // toward (the left with no hit yet).
                let sign = if vs.last_hit_dir.dot(left) < Fx::ZERO {
                    -Fx::ONE
                } else {
                    Fx::ONE
                };
                (side_of(sign), left * sign)
            }
            Exit::Forced => {
                let sign = if slot_c.y < Fx::ZERO {
                    -Fx::ONE
                } else {
                    Fx::ONE
                };
                (side_of(sign), left * sign)
            }
        };
        let spot = ctx.terrain.clamp(spot);
        let i = m.index();
        let s = &self.setup;
        let hz = s.tick_hz.max(1) as u64;
        ctx.ents.unit[i].riding = None;
        ctx.ents.transform[i].pos = spot.extend(ctx.terrain.height_at(spot));
        ctx.ents.transform[i].yaw = Fx::atan2(facing.y, facing.x);
        ctx.ents.velocity[i] = Default::default();
        ctx.ents.mover[i].waypoint = spot;
        if how == Exit::Forced {
            let stun = s.rules.vehicles.transport.stun_ms as u64 * hz / 1000;
            let w = &mut ctx.ents.weapon[i];
            w.cooldown = w.cooldown.max(stun.min(u16::MAX as u64) as u16);
            let c = &mut ctx.ents.combat[i];
            c.suppression = c
                .suppression
                .max((s.rules.vehicles.bail.bail_suppression * 1000).min(100_000));
        }
        ctx.hash.mark_entity_chunk(m.chunk());
        let under_fire =
            how == Exit::Forced || self.vehicles[&id].unload.is_some_and(|u| u.under_fire);
        let vs = self.vehicles.get_mut(&id).expect("listed");
        vs.riders[k] = None;
        // His squad: he stands where he got out; with its last rider out it
        // holds there.
        let Some(sid) = ctx.ents.squad[i] else {
            return;
        };
        let still_riding = self.squad_rides(ctx.ents, sid);
        let Some(sq) = self.squads.get_mut(&sid) else {
            return;
        };
        if sq.nav.slots.len() != sq.members.len() {
            sq.nav.slots = sq
                .members
                .iter()
                .map(|q| ctx.ents.transform[q.index()].pos.xy())
                .collect();
        }
        if let Some(j) = sq.members.iter().position(|&q| q == m) {
            sq.nav.slots[j] = spot;
        }
        if sq.nav.phase != NavPhase::Holding {
            sq.nav.phase = NavPhase::Holding;
            sq.nav.phase_ticks = 0;
            sq.nav.path.clear();
        }
        if !still_riding {
            sq.order = Order::Idle;
            crate::game::hold_in_place(sq, ctx.ents, facing);
            let owner = sq.owner;
            if under_fire && let Some(p) = self.players.get_mut(owner.0 as usize) {
                p.s8a.dismounts_under_fire = p.s8a.dismounts_under_fire.saturating_add(1);
            }
        }
    }

    /// The trip of transport `id` ends when its last rider is out: the
    /// report counters (`S8aCounters`).
    fn end_trip(&mut self, ctx: &mut Ctx, id: SimId) {
        let vs = &self.vehicles[&id];
        if vs.trip.is_none() || vs.riders.iter().any(|r| r.is_some()) {
            return;
        }
        let trip = vs.trip.expect("checked");
        let owner = ctx.ents.owner[id.index()];
        let to = ctx.ents.transform[id.index()].pos.xy();
        let hz = self.setup.tick_hz.max(1) as u64;
        let min_ride =
            Fx::from_int(self.setup.rules.ai.transport.counted_ride_m.min(30_000) as i32);
        let dist = (to - trip.from).length();
        let dist_mm = (dist.raw().max(0) as u64 * 1000) >> Fx::FRAC_BITS;
        let walk = dist_mm * hz / trip.walk_mm_s.max(1) as u64;
        let ride = ctx.tick.saturating_sub(trip.since);
        let n = trip.squads.max(1) as u64;
        let long = (to - trip.from).length_sq_raw() >= sq_m(min_ride);
        self.vehicles.get_mut(&id).expect("listed").trip = None;
        if let Some(p) = self.players.get_mut(owner.0 as usize) {
            let c = &mut p.s8a;
            c.rides = c.rides.saturating_add(n as u32);
            c.ride_ticks = c.ride_ticks.saturating_add(ride * n);
            c.ride_walk_ticks = c.ride_walk_ticks.saturating_add(walk * n);
            if long {
                c.transported = c.transported.saturating_add(n as u32);
                c.transport_trips = c.transport_trips.saturating_add(1);
            }
        }
    }

    /// Squad `sid` stops walking to its vehicle and holds.
    fn embark_off(&mut self, ctx: &mut Ctx, sid: SquadId) {
        let Some(sq) = self.squads.get_mut(&sid) else {
            return;
        };
        sq.order = Order::Idle;
        let facing = sq.nav.heading;
        crate::game::hold_in_place(sq, ctx.ents, facing);
        for q in &mut self.path_queue.queues {
            q.retain(|r| *r != crate::game::PathReq::Squad(sid));
        }
    }

    /// Squads with `Embark`: those whose vehicle can no longer take them
    /// (gone, burning, unloading, not theirs) and with nobody aboard stop;
    /// the others' walkers queue at the door, re-pathing when it moved.
    fn embark_upkeep(&mut self, ctx: &mut Ctx) {
        let ids: Vec<(SquadId, SimId)> = self
            .squads
            .iter()
            .filter_map(|(&sid, sq)| match sq.order {
                Order::Embark { vehicle } => Some((sid, vehicle)),
                _ => None,
            })
            .collect();
        for (sid, v) in ids {
            let owner = self.squads[&sid].owner;
            let rides = self.squad_rides(ctx.ents, sid);
            if !self.boardable(ctx.ents, owner, v) {
                if !rides {
                    self.embark_off(ctx, sid);
                }
                continue;
            }
            let Some((door, out)) = self.door_spot(ctx.ents, v) else {
                continue;
            };
            let sq = &self.squads[&sid];
            if !matches!(sq.nav.phase, NavPhase::Arriving | NavPhase::Holding) {
                continue;
            }
            let walkers: Vec<usize> = (0..sq.members.len())
                .filter(|&j| ctx.ents.unit[sq.members[j].index()].riding.is_none())
                .collect();
            let Some(&first) = walkers.first() else {
                continue;
            };
            // The door moved away: walk there again.
            let lead = ctx.ents.transform[sq.members[first].index()].pos.xy();
            let far = sq_m(Fx::from_int(8));
            if (lead - door).length_sq_raw() > far
                && (sq.nav.goal - door).length_sq_raw() > sq_m(Fx::from_int(4))
            {
                let goal = ctx.terrain.clamp(door);
                self.request_path(sid, goal);
                continue;
            }
            let lat = out.perp();
            let mut slots: Vec<FxVec2> = if sq.nav.slots.len() == sq.members.len() {
                sq.nav.slots.clone()
            } else {
                sq.members
                    .iter()
                    .map(|q| ctx.ents.transform[q.index()].pos.xy())
                    .collect()
            };
            // A man not yet past the door's face walks to the hull corner on
            // his side first (a straight line to the door would cross the
            // hull; men steer around vehicles only locally).
            let (hc, ax, ay, hh) = match self.vderived.frame(v) {
                Some(f) => (
                    f.hull.center.xy(),
                    f.hull.axes.x.xy(),
                    f.hull.axes.y.xy(),
                    f.hull.half,
                ),
                None => (door, out, lat, crate::fx::FxVec3::ZERO),
            };
            let ext = |d: FxVec2| d.dot(ax).abs().mul(hh.x) + d.dot(ay).abs().mul(hh.y);
            let (out_ext, lat_ext) = (ext(out), ext(lat));
            for (n, &j) in walkers.iter().enumerate() {
                let p = ctx.ents.transform[sq.members[j].index()].pos.xy();
                let rel = p - hc;
                let slot = if rel.dot(out) < out_ext + Fx::from_ratio(2, 10) {
                    let side = if rel.dot(lat) < Fx::ZERO {
                        -Fx::ONE
                    } else {
                        Fx::ONE
                    };
                    hc + out * (out_ext + Fx::from_ratio(4, 10))
                        + lat * (lat_ext + Fx::from_ratio(12, 10)).mul(side)
                } else {
                    let n = n as i32;
                    door + out * Fx::from_ratio(7, 10).mul_int(n / 3)
                        + lat * Fx::from_ratio(7, 10).mul_int(n % 3 - 1)
                };
                slots[j] = ctx.terrain.clamp(slot);
            }
            let sq = self.squads.get_mut(&sid).expect("listed");
            if sq.nav.slots != slots {
                sq.nav.slots = slots;
                sq.nav.facing = -out;
            }
        }
    }

    /// S8a load check: every slot names a live own soldier who rides that
    /// slot, and the reverse; slot counts match the defs; door timers and
    /// unloads only on transports.
    pub(crate) fn validate_riders(&self, ents: &Entities) -> Result<(), &'static str> {
        for (&id, vs) in &self.vehicles {
            let v = &self.setup.vehicles[vs.type_idx as usize];
            let slots = v.transport.as_ref().map_or(0, |t| t.slots.len());
            if vs.riders.len() != slots {
                return Err("transport slot count differs from its def");
            }
            match &v.transport {
                None => {
                    if vs.door_ticks != 0 || vs.unload.is_some() || vs.trip.is_some() {
                        return Err("door state on a vehicle that is not a transport");
                    }
                }
                Some(t) => {
                    let most = t
                        .board_ticks
                        .max(t.exit_ticks)
                        .max(t.exit_over_sides_ticks.unwrap_or(0));
                    if vs.door_ticks as u32 > most {
                        return Err("door timer over its longest time");
                    }
                    if vs.unload.is_some_and(|u| u.over_sides) && t.exit_over_sides_ticks.is_none()
                    {
                        return Err("side exit on a transport without one");
                    }
                }
            }
            for (k, r) in vs.riders.iter().enumerate() {
                let Some(m) = *r else { continue };
                let i = m.index();
                if ents.kind.get(i) != Some(&EntityKind::Soldier)
                    || ents.owner[i] != ents.owner[id.index()]
                    || ents.unit[i].riding
                        != Some(Riding {
                            vehicle: id,
                            slot: k as u8,
                        })
                {
                    return Err("a passenger slot names no rider of it");
                }
            }
        }
        for (i, u) in ents.unit.iter().enumerate() {
            let Some(r) = u.riding else { continue };
            let back = self
                .vehicles
                .get(&r.vehicle)
                .and_then(|vs| vs.riders.get(r.slot as usize).copied().flatten());
            if back != Some(SimId(i as u32)) {
                return Err("a rider's slot does not name him");
            }
            if ents.kind[i] != EntityKind::Soldier || u.detached {
                return Err("a rider is not a soldier in formation");
            }
        }
        for sq in self.squads.values() {
            if let Order::Embark { vehicle } = sq.order
                && self.transport_of(vehicle).is_none()
            {
                return Err("Embark names no transport");
            }
        }
        Ok(())
    }
}
