//! S3c bail-out, abandoned vehicles and re-crewing
//! (`docs/slices/S3c-armour.md` Part A). Serial, in the Status stage after
//! the vehicles' own status ([`MatchState::crew_status`]), ascending
//! vehicle id, then ascending squad id.
//!
//! **Triggers** (`[vehicles.bail]`, vehicles whose def has a crew squad and
//! hatches), first that applies: the owner's `Abandon` order (6); an ammo
//! fire burning (1, `ammo_fire_bail`); an engine fire still burning after
//! `engine_fire_bail_s` (2); fewer than `crew_below` crewmen alive (3, also
//! every man dead: S3c-12 (A), the intact tank is abandoned); the main gun
//! destroyed and the vehicle broken down (engine, a track or the
//! transmission out; 4, `useless_bail`); gun out or broken down and a
//! penetration this tick within `under_fire_s` of the previous one (5). A
//! rack detonation still destroys the vehicle with everyone in it
//! (`crate::damage`).
//!
//! **Abandoning** turns the entity into [`EntityKind::Abandoned`]: no
//! squad (its followers stop, its path request goes), stopped, seen by
//! everyone, its last owner kept in the owner column, every order and
//! target cleared and every reference to it as a target dropped. It counts
//! toward nobody's pop or vehicle limit, blocks vehicles like a wreck,
//! stops rounds like any vehicle and takes damage from them. Its fires keep
//! burning (nobody fights them): an ammo fire after `ammo_fire_s` or an
//! engine fire after `fire_burnout` makes it a wreck, killing anyone still
//! inside.
//!
//! **Exits.** Every living crewman leaves by his seat's hatch; men sharing a
//! hatch go one after another in the hatch's seat order, `exit_s` each (×
//! `wounded_exit_pct` for a man below `wounded_hp`). Until his time is up
//! he is still a seat and is hit as one. Then he becomes a soldier of the
//! vehicle's crew squad (created with the first man out; a new one if the
//! last was wiped out) beside the hull below his hatch, on the side the
//! last round travelled toward (away from the shooter; the left side when
//! nothing hit it), `exit_clear_cm` outside the hull box; if that spot is
//! within `exit_clear_cm` of another hull (live, abandoned or wreck), the
//! other side, else ahead of or behind the hull, else the spot farthest from
//! the other hulls (S3c Fix). He keeps his HP,
//! carries the squad's weapon (the first `weapon_men` men out) or its
//! sidearm, cannot fire for `dismount_ms` and arrives with
//! `bail_suppression`.
//!
//! **Re-crew** (`Recrew`, `[vehicles.recrew]`): a crew squad, or (with
//! `any_infantry`) any infantry squad but an AT team, of at least `min_men`
//! walks to the vehicle (order `Recrew`). Nobody may board a vehicle that
//! burns, or whose gun and mobility are out under `useless_bail` (refusals
//! `Burning`, `Useless`): it would be abandoned again at once. Once all
//! exits are done, nothing blocks boarding and at
//! least `min_men` of its men stand within `reach_m` of the hull box, the
//! men there climb in together, `board_s` per man and hatch (the hatches
//! work at once): `board_s × ⌈men / hatches⌉`. Moving away or losing men
//! below `min_men` restarts it; a squad already boarding keeps its place
//! while it still can. When the time is up without a free vehicle slot or
//! room under the pop cap (counting the men boarding as gone), the re-crew
//! is refused once (snapshot `recrew_refusals`) and the squad goes `Idle`;
//! with them, those men (at most one per empty seat, the squad's order
//! first) leave the world as soldiers and fill the empty seats in seat
//! order with their HP; infantry are marked untrained. The vehicle becomes
//! a live vehicle of the boarding player with a new vehicle squad. An enemy
//! may do this (capture) when `capture` is on; with `capture_needs_crew` only
//! a crew squad may (refusal `CaptureNeedsCrew`). Men left over stay a squad
//! (order `Idle`).
//!
//! **Crew squads** are ordinary infantry for orders. One in its own base,
//! holding with no order, is dismissed after `crew_dismiss_s` (men removed,
//! no losses, no refund; S3c-4 (A)).
//!
//! **Cap.** Over `abandoned_cap` abandoned vehicles the oldest becomes a
//! wreck ("scrapped": no knock-out event, no losses beyond men still
//! inside).

use frontline_content::vehicle::VehicleSetup;
use frontline_content::{ComponentKind, MatchSetup, SquadRole};

use crate::combat;
use crate::damage::{ArmourEv, KO_AMMO_FIRE, KO_FIRE};
use crate::ecs::{EntityKind, PlayerId, SimId, Velocity};
use crate::fx::{Fx, FxVec2};
use crate::game::{Ctx, MatchState, Nav, NavPhase, Order, PathReq, Squad, SquadId, kill_class};
use crate::items::Kit;
use crate::snapshot::{ArmourEventKind, NONE_U8};
use crate::vehicle::{MgState, Seat, VehicleState, yaw_dir};
use crate::world::Stats;

/// `VehicleState::abandon_cause` and the `Abandoned` event's component: an
/// ammo fire, an engine fire, crew losses, gun and mobility out, disabled
/// under fire, the owner's order.
pub const AB_AMMO_FIRE: u8 = 1;
pub const AB_ENGINE_FIRE: u8 = 2;
pub const AB_CREW: u8 = 3;
pub const AB_USELESS: u8 = 4;
pub const AB_UNDER_FIRE: u8 = 5;
pub const AB_ORDER: u8 = 6;

/// `ArmourEventKind::Recrewed` components: re-crewed by its own side, or
/// captured.
pub const RECREW_OWN: u8 = 1;
pub const RECREW_CAPTURED: u8 = 2;

/// Why a `Recrew` was refused (the snapshot's `recrew_refusals`, for the
/// feed).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum RecrewRefusal {
    /// Not an abandoned vehicle, or not the player's squad.
    NotAbandoned = 0,
    /// The squad cannot man a vehicle (an AT team, a vehicle, or infantry
    /// with `any_infantry` off).
    NotCrew = 1,
    /// An enemy's vehicle and `capture` is off.
    NoCapture = 2,
    /// Fewer than `min_men` men.
    TooFewMen = 3,
    /// The player has as many vehicles as the limit.
    VehicleLimit = 4,
    /// It would take the player over the pop cap.
    PopCap = 5,
    /// It is on fire (engine or ammunition): the men would bail out again
    /// at once (S3c Fix).
    Burning = 6,
    /// Its gun and its mobility are out (`useless_bail`): the men would
    /// bail out again at once (S3c Fix).
    Useless = 7,
    /// An enemy's vehicle, `capture_needs_crew` is on and the squad is not
    /// a tank crew (S3c Fix).
    CaptureNeedsCrew = 8,
}

/// One refused `Recrew` (presentation only, like `CallInEvent`).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct RecrewEvent {
    pub player: PlayerId,
    pub vehicle: SimId,
    pub why: RecrewRefusal,
}

/// The longest any man can take to get out of `v`: its longest hatch
/// queue, every man wounded (the load check).
pub(crate) fn exit_max(v: &VehicleSetup, s: &MatchSetup) -> u32 {
    let pct = s.rules.vehicles.bail.wounded_exit_pct.max(100);
    v.hatches
        .iter()
        .map(|h| h.exit_ticks.max(1) * pct / 100 * h.seats.len() as u32 + 1)
        .max()
        .unwrap_or(0)
}

/// Boarding ticks for `men` men through `v`'s hatches.
pub(crate) fn board_ticks(v: &VehicleSetup, s: &MatchSetup, men: u32) -> u32 {
    let hatches = v.hatches.len().max(1) as u32;
    s.rules.vehicles.recrew.board_s * s.tick_hz.max(1) * men.max(1).div_ceil(hatches)
}

/// The most boarding ticks a save may hold for `v` (a full crew).
pub(crate) fn board_max(v: &VehicleSetup, s: &MatchSetup) -> u32 {
    board_ticks(v, s, v.seats.len() as u32)
}

/// Whether vehicles of this def can bail out at all.
pub fn can_bail(v: &VehicleSetup) -> bool {
    v.crew_squad.is_some() && !v.hatches.is_empty()
}

/// Whether any of `vs`'s mobility parts (engine, tracks, transmission) is
/// out.
pub(crate) fn immobile(vs: &VehicleState) -> bool {
    [
        ComponentKind::Engine,
        ComponentKind::TrackLeft,
        ComponentKind::TrackRight,
        ComponentKind::Transmission,
    ]
    .iter()
    .any(|&k| vs.component(k) == 0)
}

/// Why nobody may board abandoned vehicle `vs` now, if so: a manned
/// vehicle in this state would be abandoned again in the next Status stage
/// (a fire, or gun and mobility out under `useless_bail`).
pub(crate) fn board_blocked(s: &MatchSetup, vs: &VehicleState) -> Option<RecrewRefusal> {
    if vs.burn > 0 || vs.ammo_fire > 0 {
        return Some(RecrewRefusal::Burning);
    }
    if s.rules.vehicles.bail.useless_bail != 0
        && vs.component(ComponentKind::Gun) == 0
        && immobile(vs)
    {
        return Some(RecrewRefusal::Useless);
    }
    None
}

/// Whether a squad of `role` may man a vehicle under these rules.
pub fn can_crew(s: &MatchSetup, role: SquadRole) -> bool {
    match role {
        SquadRole::Crew => true,
        SquadRole::AntiTank | SquadRole::Vehicle => false,
        _ => s.rules.vehicles.recrew.any_infantry != 0,
    }
}

impl MatchState {
    /// Abandoned vehicles on the map.
    pub fn abandoned_count(&self) -> usize {
        self.vehicles
            .values()
            .filter(|vs| vs.abandoned.is_some())
            .count()
    }

    /// Why `p` cannot re-crew `vehicle` with `squad` now, if it cannot
    /// (the command's check; boarding checks the limit and pop again).
    pub fn recrew_check(
        &self,
        ents: &crate::ecs::Entities,
        p: PlayerId,
        squad: SquadId,
        vehicle: SimId,
    ) -> Result<(), RecrewRefusal> {
        let s = &self.setup;
        let r = &s.rules.vehicles.recrew;
        let Some(sq) = self.squads.get(&squad).filter(|q| q.owner == p) else {
            return Err(RecrewRefusal::NotAbandoned);
        };
        let Some(vs) = self
            .vehicles
            .get(&vehicle)
            .filter(|_| ents.kind.get(vehicle.index()) == Some(&EntityKind::Abandoned))
        else {
            return Err(RecrewRefusal::NotAbandoned);
        };
        let role = s.squad_types[sq.type_idx as usize].role;
        if !can_crew(s, role) {
            return Err(RecrewRefusal::NotCrew);
        }
        let enemy = ents.owner[vehicle.index()] != p;
        if r.capture == 0 && enemy {
            return Err(RecrewRefusal::NoCapture);
        }
        if r.capture_needs_crew != 0 && enemy && role != SquadRole::Crew {
            return Err(RecrewRefusal::CaptureNeedsCrew);
        }
        if let Some(why) = board_blocked(s, vs) {
            return Err(why);
        }
        if (sq.members.len() as u32) < r.min_men.max(s.rules.vehicles.bail.crew_below) {
            return Err(RecrewRefusal::TooFewMen);
        }
        if self.vehicles_counted(ents, p) >= s.rules.vehicles.limit {
            return Err(RecrewRefusal::VehicleLimit);
        }
        let v = &s.vehicles[vs.type_idx as usize];
        let men = (sq.members.len()).min(v.seats.len()) as u32;
        if self.population(ents, p).saturating_sub(men) + v.pop > s.rules.pop_cap {
            return Err(RecrewRefusal::PopCap);
        }
        Ok(())
    }

    /// `Recrew`: the squad walks to the abandoned vehicle and boards it
    /// (see the module docs). A refusal is logged for the snapshot.
    pub(crate) fn order_recrew(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        squad: SquadId,
        vehicle: SimId,
    ) -> bool {
        if let Err(why) = self.recrew_check(ctx.ents, p, squad, vehicle) {
            self.recrew_events.push(RecrewEvent {
                player: p,
                vehicle,
                why,
            });
            return false;
        }
        self.clear_detached(ctx, squad);
        let vpos = ctx.ents.transform[vehicle.index()].pos.xy();
        let lead = self.squads[&squad].lead(ctx.ents);
        let from = ctx.ents.transform[lead.index()].pos.xy();
        let vs = &self.vehicles[&vehicle];
        let half = self.vderived.geo[vs.type_idx as usize].hull_half.y;
        let mut dir = (from - vpos).normalize_or_zero();
        if dir == FxVec2::ZERO {
            dir = FxVec2::new(Fx::ONE, Fx::ZERO);
        }
        let goal = ctx.terrain.clamp(vpos + dir * (half + Fx::ONE));
        self.squads.get_mut(&squad).expect("checked").order = Order::Recrew { vehicle };
        self.request_path(squad, goal);
        true
    }

    /// `Abandon`: the crew of the player's vehicle squad bail out in the
    /// next Status stage.
    pub(crate) fn order_abandon(
        &mut self,
        ents: &crate::ecs::Entities,
        p: PlayerId,
        squad: SquadId,
    ) -> bool {
        if !self.owned(p, squad) || !self.is_vehicle_squad(squad) {
            return false;
        }
        let v = self.squads[&squad].members[0];
        if ents.kind.get(v.index()) != Some(&EntityKind::Vehicle) {
            return false;
        }
        let Some(vs) = self.vehicles.get_mut(&v) else {
            return false;
        };
        if !can_bail(&self.setup.vehicles[vs.type_idx as usize]) {
            return false;
        }
        vs.bail_order = true;
        true
    }

    /// The S3c part of the Status stage (module docs): triggers, abandoned
    /// vehicles' fires, exits and boarding, the cap, crew squads.
    pub(crate) fn crew_status(
        &mut self,
        ctx: &mut Ctx,
        stats: &mut Stats,
        events: &mut Vec<ArmourEv>,
    ) {
        let ids: Vec<SimId> = self.vehicles.keys().copied().collect();
        for &id in &ids {
            if ctx.ents.kind[id.index()] != EntityKind::Vehicle {
                continue;
            }
            if let Some(cause) = self.bail_cause(id, ctx.tick) {
                self.abandon(ctx, id, cause, events);
            }
        }
        let mut ko: Vec<(SimId, u8)> = Vec::new();
        for &id in &ids {
            if ctx.ents.kind[id.index()] != EntityKind::Abandoned {
                continue;
            }
            // Its fires burned this tick already if it was abandoned now.
            let fresh = self.vehicles[&id].abandoned == Some(ctx.tick);
            let burnt = if fresh { None } else { self.abandoned_fire(id) };
            if let Some(cause) = burnt {
                ko.push((id, cause));
                continue;
            }
            self.exits(ctx, id, events);
            self.boarding(ctx, id, events);
        }
        for (id, cause) in ko {
            self.knock_out_with(ctx, stats, id, cause, kill_class::OTHER, events);
        }
        self.abandoned_cap(ctx, stats);
        self.crew_squads(ctx);
    }

    /// The first trigger that applies to live vehicle `id` (module docs).
    fn bail_cause(&self, id: SimId, tick: u64) -> Option<u8> {
        let vs = self.vehicles.get(&id)?;
        let v = &self.setup.vehicles[vs.type_idx as usize];
        if !can_bail(v) {
            return None;
        }
        let b = &self.setup.rules.vehicles.bail;
        let hz = self.setup.tick_hz.max(1);
        let living = vs.seats.iter().filter(|s| s.hp > 0).count() as u32;
        let broken = immobile(vs);
        let gun_out = vs.component(ComponentKind::Gun) == 0;
        let cause = if vs.bail_order {
            AB_ORDER
        } else if b.ammo_fire_bail != 0 && vs.ammo_fire > 0 {
            AB_AMMO_FIRE
        } else if b.engine_fire_bail_s > 0 && vs.burn as u32 >= b.engine_fire_bail_s * hz {
            AB_ENGINE_FIRE
        } else if living == 0 || living < b.crew_below {
            AB_CREW
        } else if b.useless_bail != 0 && gun_out && broken {
            AB_USELESS
        } else if b.under_fire_s > 0
            && (gun_out || broken)
            && vs.last_pen == Some(tick)
            && vs
                .prev_pen
                .is_some_and(|t| tick - t <= (b.under_fire_s * hz) as u64)
        {
            AB_UNDER_FIRE
        } else {
            return None;
        };
        Some(cause)
    }

    /// Abandons live vehicle `id` (module docs).
    pub(crate) fn abandon(
        &mut self,
        ctx: &mut Ctx,
        id: SimId,
        cause: u8,
        events: &mut Vec<ArmourEv>,
    ) {
        let i = id.index();
        let owner = ctx.ents.owner[i];
        let tick = ctx.tick;
        let Some(vs) = self.vehicles.get(&id) else {
            return;
        };
        let followers = vs.followers.clone();
        let by = vs.last_attacker;
        let pos = ctx.ents.transform[i].pos;
        // Followers stop where they are (as for a knock-out).
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
        if let Some(sq) = ctx.ents.squad[i].take() {
            for q in &mut self.path_queue.queues {
                q.retain(|r| !matches!(r, PathReq::Vehicle { squad, .. } if *squad == sq));
            }
            self.squads.remove(&sq);
        }
        ctx.ents.kind[i] = EntityKind::Abandoned;
        ctx.ents.velocity[i] = Velocity::default();
        ctx.ents.combat[i].seen_by = ((1u32 << self.players.len()) - 1) as u16;
        ctx.hash.mark_entity_chunk(id.chunk());
        let s = &self.setup;
        let b = &s.rules.vehicles.bail;
        let vs = self.vehicles.get_mut(&id).expect("listed");
        let v = &s.vehicles[vs.type_idx as usize];
        vs.abandoned = Some(tick);
        vs.abandon_cause = cause;
        vs.bail_order = false;
        vs.speed = Fx::ZERO;
        vs.avg_vel = FxVec2::ZERO;
        vs.reverse = false;
        vs.target = None;
        vs.attack = None;
        vs.ranging = 0;
        for m in &mut vs.mgs {
            *m = MgState {
                spent: m.spent,
                ..MgState::default()
            };
        }
        vs.repairing = None;
        vs.repair_progress = 0;
        vs.base_ticks = 0;
        vs.followers.clear();
        vs.face = None;
        vs.suppress = None;
        vs.board = None;
        vs.crew_squad = None;
        // A man moving between seats stays in the one he moves to.
        for st in &mut vs.seats {
            st.swap = 0;
            st.swap_from = 0;
        }
        // Exit queues, hatch by hatch.
        let living = vs.seats.iter().filter(|st| st.hp > 0).count() as u32;
        for h in &v.hatches {
            let mut t = 0u32;
            for &k in &h.seats {
                let Some(st) = vs.seats.get(k as usize) else {
                    continue;
                };
                if st.hp == 0 {
                    continue;
                }
                let pct = if (st.hp as u32) < b.wounded_hp {
                    b.wounded_exit_pct
                } else {
                    100
                };
                t += (h.exit_ticks.max(1) * pct / 100).max(1);
                vs.exit[k as usize] = t.min(u16::MAX as u32) as u16;
            }
        }
        if let Some(pl) = self.players.get_mut(owner.0 as usize) {
            pl.crew.abandoned += 1;
            pl.crew.bailed += living;
        }
        self.forget(ctx.ents, ctx.hash, id);
        self.mark_wreck_block(ctx.ents, id);
        let ev = ArmourEv::new(
            ArmourEventKind::Abandoned,
            id,
            owner.0,
            by.unwrap_or(NONE_U8),
        )
        .component(cause)
        .at(pos);
        self.push_event(events, ev);
    }

    /// An abandoned vehicle's fires burn on; the knock-out cause when one
    /// burns it out.
    fn abandoned_fire(&mut self, id: SimId) -> Option<u8> {
        let r = &self.setup.rules.vehicles;
        let hz = self.setup.tick_hz.max(1);
        let vs = self.vehicles.get_mut(&id)?;
        if vs.ammo_fire > 0 {
            vs.ammo_fire = vs.ammo_fire.saturating_add(1);
            if vs.ammo_fire as u32 >= r.damage.ammo_fire_s * hz {
                return Some(KO_AMMO_FIRE);
            }
        }
        if vs.burn > 0 {
            vs.burn = vs.burn.saturating_add(1);
            let n = vs.burn as u64;
            let lost = (n * r.fire_burn_hp_s as u64 / hz as u64)
                - ((n - 1) * r.fire_burn_hp_s as u64 / hz as u64);
            let e = &mut vs.components[ComponentKind::Engine.index()];
            *e = e.saturating_sub(lost.min(u16::MAX as u64) as u16);
            if vs.burn as u32 >= r.fire_burnout_ticks {
                return Some(KO_FIRE);
            }
        }
        None
    }

    /// Counts the exit timers down; men whose time is up get out.
    fn exits(&mut self, ctx: &mut Ctx, id: SimId, events: &mut Vec<ArmourEv>) {
        let mut out: Vec<usize> = Vec::new();
        {
            let vs = self.vehicles.get_mut(&id).expect("listed");
            for (k, e) in vs.exit.iter_mut().enumerate() {
                if *e == 0 {
                    continue;
                }
                if vs.seats[k].hp == 0 {
                    *e = 0;
                    continue;
                }
                *e -= 1;
                if *e == 0 {
                    out.push(k);
                }
            }
        }
        for k in out {
            self.get_out(ctx, id, k, events);
        }
    }

    /// The man in seat `k` of abandoned vehicle `id` gets out (module docs).
    fn get_out(&mut self, ctx: &mut Ctx, id: SimId, k: usize, events: &mut Vec<ArmourEv>) {
        let i = id.index();
        let owner = ctx.ents.owner[i];
        let s = &self.setup;
        let b = &s.rules.vehicles.bail;
        let hz = s.tick_hz.max(1);
        let vs = &self.vehicles[&id];
        let v = &s.vehicles[vs.type_idx as usize];
        let Some(crew_type) = v.crew_squad else {
            return;
        };
        let hp = vs.seats[k].hp;
        let role = v.seats[k].role;
        let geo = &self.vderived.geo[vs.type_idx as usize];
        let Some(f) = self.vderived.frame(id).copied() else {
            return;
        };
        // Below the hatch, on the side the last round travelled toward.
        let t = ctx.ents.transform[i];
        let fwd = yaw_dir(t.yaw);
        let left = fwd.perp();
        let hatch = v
            .hatches
            .iter()
            .find(|h| h.seats.contains(&(k as u8)))
            .map(|h| crate::damage::anchor_at(&f, h.frame, crate::vehicle::v3(h.pos)))
            .unwrap_or(t.pos);
        let along = (hatch.xy() - t.pos.xy()).dot(fwd);
        let side = if vs.last_hit_dir.dot(left) < Fx::ZERO {
            -Fx::ONE
        } else {
            Fx::ONE
        };
        let clear = Fx::from_ratio(b.exit_clear_cm as i64, 100);
        let off = geo.hull_half.y + clear;
        let end = geo.hull_half.x + clear;
        let c = t.pos.xy() + fwd * geo.hull_center.x;
        // That side, else the other, else ahead or behind the hull: the
        // first spot clear of every other hull (live, abandoned or wreck)
        // by `exit_clear_cm`; if none is, the one farthest from them.
        let tries = [
            (t.pos.xy() + fwd * along + left * (off * side), left * side),
            (t.pos.xy() + fwd * along - left * (off * side), -left * side),
            (c + fwd * end, fwd),
            (c - fwd * end, -fwd),
        ];
        let clear_sq = FxVec2::new(clear, Fx::ZERO).length_sq_raw();
        let room = |p: FxVec2| -> u64 {
            self.vehicles
                .keys()
                .filter(|&&o| o != id)
                .map(|&o| self.hull_dist_sq(ctx, o, p))
                .min()
                .unwrap_or(u64::MAX)
        };
        let mut best = (0usize, 0u64);
        for (n, &(p, _)) in tries.iter().enumerate() {
            let r = room(ctx.terrain.clamp(p));
            if r >= clear_sq {
                best = (n, r);
                break;
            }
            if n == 0 || r > best.1 {
                best = (n, r);
            }
        }
        let (spot, facing) = tries[best.0];
        let spot = ctx.terrain.clamp(spot);
        // The crew squad: the linked one, else a new one.
        let squad = match vs.crew_squad.filter(|q| self.squads.contains_key(q)) {
            Some(q) => q,
            None => {
                let q = SquadId(self.next_squad);
                self.next_squad += 1;
                self.squads.insert(
                    q,
                    Squad {
                        type_idx: crew_type,
                        owner,
                        members: Vec::new(),
                        order: Order::Idle,
                        nav: Nav {
                            phase: NavPhase::Holding,
                            goal: spot,
                            heading: facing,
                            facing,
                            ..Nav::default()
                        },
                        focus: None,
                        launcher: None,
                    },
                );
                q
            }
        };
        let ty = &s.squad_types[crew_type as usize];
        let nth = self.squads[&squad].members.len();
        let wk = match ty.sidearm {
            Some(side) if nth >= ty.weapon_men as usize => side,
            _ => ty.weapon,
        };
        let sid = ctx.ents.spawn(EntityKind::Soldier, owner);
        let si = sid.index();
        ctx.ents.transform[si].pos = spot.extend(ctx.terrain.height_at(spot));
        ctx.ents.transform[si].yaw = Fx::atan2(facing.y, facing.x);
        ctx.ents.mover[si].waypoint = spot;
        ctx.ents.mover[si].speed = ty.speed.into();
        ctx.ents.squad[si] = Some(squad);
        let mut c = combat::spawn_combat(
            (hp as u32).min(ty.hp),
            &s.weapons[wk as usize],
            &s.rules.combat.body,
        );
        c.suppression = (b.bail_suppression * 1000).min(100_000);
        ctx.ents.combat[si] = c;
        ctx.ents.weapon[si].cooldown =
            (b.dismount_ms as u64 * hz as u64 / 1000).min(u16::MAX as u64) as u16;
        ctx.ents.kit[si] = Kit {
            items: ty.full_kit(&s.items, nth == 0),
            leader: nth == 0,
            ..Kit::default()
        };
        ctx.hash.mark_entity_chunk(sid.chunk());
        let sq = self.squads.get_mut(&squad).expect("crew squad");
        sq.members.push(sid);
        if sq.nav.phase == NavPhase::Holding || sq.nav.phase == NavPhase::Arriving {
            sq.nav.slots.push(spot);
        }
        let vs = self.vehicles.get_mut(&id).expect("listed");
        vs.seats[k] = Seat::default();
        vs.crew_squad = Some(squad);
        if let Some(pl) = self.players.get_mut(owner.0 as usize) {
            pl.crew.out += 1;
        }
        let ev = ArmourEv::new(ArmourEventKind::CrewOut, id, owner.0, NONE_U8)
            .component(role.index() as u8)
            .at(spot.extend(ctx.terrain.height_at(spot)));
        self.push_event(events, ev);
    }

    /// Distance² (raw) from `p` to the planar hull box of vehicle `id`.
    fn hull_dist_sq(&self, ctx: &Ctx, id: SimId, p: FxVec2) -> u64 {
        let vs = &self.vehicles[&id];
        let geo = &self.vderived.geo[vs.type_idx as usize];
        let t = &ctx.ents.transform[id.index()];
        let fwd = yaw_dir(t.yaw);
        let c = t.pos.xy() + fwd * geo.hull_center.x;
        let d = p - c;
        let ex = (d.dot(fwd).abs() - geo.hull_half.x).max(Fx::ZERO);
        let ey = (d.dot(fwd.perp()).abs() - geo.hull_half.y).max(Fx::ZERO);
        FxVec2::new(ex, ey).length_sq_raw()
    }

    /// Squads re-crewing vehicle `id` that have arrived stand along the
    /// hull side nearest their leader, 1 m out, spread over its length.
    fn hull_slots(&mut self, ctx: &Ctx, id: SimId) {
        let vs = &self.vehicles[&id];
        let geo = &self.vderived.geo[vs.type_idx as usize];
        let t = ctx.ents.transform[id.index()];
        let fwd = yaw_dir(t.yaw);
        let left = fwd.perp();
        let c = t.pos.xy() + fwd * geo.hull_center.x;
        let (hx, hy) = (geo.hull_half.x, geo.hull_half.y);
        for sq in self.squads.values_mut() {
            if sq.order != (Order::Recrew { vehicle: id })
                || !matches!(sq.nav.phase, NavPhase::Arriving | NavPhase::Holding)
                || sq.members.is_empty()
            {
                continue;
            }
            let lead = ctx.ents.transform[sq.lead(ctx.ents).index()].pos.xy();
            let side = if (lead - c).dot(left) < Fx::ZERO {
                -Fx::ONE
            } else {
                Fx::ONE
            };
            let n = sq.members.len() as i32;
            let out = left * ((hy + Fx::ONE) * side);
            let slots: Vec<FxVec2> = (0..n)
                .map(|k| {
                    let along = -hx + hx.mul_int(2 * k + 1).div_int(n);
                    ctx.terrain.clamp(c + fwd * along + out)
                })
                .collect();
            if sq.nav.slots != slots {
                sq.nav.slots = slots;
                sq.nav.facing = -left * side;
            }
        }
    }

    /// Boarding of abandoned vehicle `id` (module docs).
    fn boarding(&mut self, ctx: &mut Ctx, id: SimId, events: &mut Vec<ArmourEv>) {
        self.hull_slots(ctx, id);
        let s = &self.setup;
        let vs = &self.vehicles[&id];
        // Nobody climbs in while the last crew still climbs out, nor into
        // a vehicle they would have to leave again at once (a fire; gun and
        // mobility out): they wait at the hull.
        if vs.exit.iter().any(|&e| e > 0) || board_blocked(s, vs).is_some() {
            if vs.board.is_some() {
                self.vehicles.get_mut(&id).expect("listed").board = None;
            }
            return;
        }
        let v = &s.vehicles[vs.type_idx as usize];
        let r = &s.rules.vehicles.recrew;
        let reach = Fx::from_int(r.reach_m as i32);
        let reach_sq = FxVec2::new(reach, Fx::ZERO).length_sq_raw();
        let empty = vs.seats.iter().filter(|st| st.hp == 0).count();
        // Fewest men who board: `min_men`, and never so few that the crew
        // rule would make them bail out again at once.
        let fewest = r.min_men.max(s.rules.vehicles.bail.crew_below).max(1);
        // The boarding squad: the one already boarding while it still can
        // (a squad that arrives later does not reset its progress), else the
        // lowest id with a Recrew of this vehicle whose boarding men (one
        // per empty seat) all stand at the hull.
        let ready = |q: SquadId| -> Option<Vec<SimId>> {
            let sq = self.squads.get(&q)?;
            if sq.order != (Order::Recrew { vehicle: id }) {
                return None;
            }
            let want = sq.members.len().min(empty);
            let near: Vec<SimId> = sq
                .members
                .iter()
                .copied()
                .filter(|m| {
                    self.hull_dist_sq(ctx, id, ctx.ents.transform[m.index()].pos.xy()) <= reach_sq
                })
                .take(want)
                .collect();
            (near.len() == want && want as u32 >= fewest).then_some(near)
        };
        let mut pick: Option<(SquadId, Vec<SimId>)> =
            vs.board.and_then(|(bq, _)| ready(bq).map(|men| (bq, men)));
        if pick.is_none() {
            pick = self
                .squads
                .keys()
                .find_map(|&q| ready(q).map(|men| (q, men)));
        }
        let Some((q, men)) = pick else {
            if vs.board.is_some() {
                self.vehicles.get_mut(&id).expect("listed").board = None;
            }
            return;
        };
        let need = board_ticks(v, s, men.len() as u32);
        let t = match vs.board {
            Some((bq, t)) if bq == q => (t as u32 + 1).min(need),
            _ => 1,
        };
        self.vehicles.get_mut(&id).expect("listed").board =
            Some((q, t.min(u16::MAX as u32) as u16));
        if t < need {
            return;
        }
        // Done: the limit and pop again (the men boarding count as gone).
        let p = self.squads[&q].owner;
        let pop_after = self
            .population(ctx.ents, p)
            .saturating_sub(men.len() as u32)
            + v.pop;
        let why = if self.vehicles_counted(ctx.ents, p) >= s.rules.vehicles.limit {
            Some(RecrewRefusal::VehicleLimit)
        } else if pop_after > s.rules.pop_cap {
            Some(RecrewRefusal::PopCap)
        } else {
            None
        };
        if let Some(why) = why {
            // Refused once, for the feed; the squad stops waiting.
            self.recrew_events.push(RecrewEvent {
                player: p,
                vehicle: id,
                why,
            });
            self.vehicles.get_mut(&id).expect("listed").board = None;
            self.squads.get_mut(&q).expect("picked").order = Order::Idle;
            return;
        }
        self.man_vehicle(ctx, id, q, &men, events);
    }

    /// The men `men` of squad `q` board abandoned vehicle `id`, which
    /// becomes the squad owner's (module docs).
    fn man_vehicle(
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
        let untrained =
            self.setup.squad_types[self.squads[&q].type_idx as usize].role != SquadRole::Crew;
        let crew_hp = self.setup.rules.vehicles.crew_hp.min(u16::MAX as u32) as u16;
        let type_idx = self.vehicles[&id].type_idx;
        // Seats in seat order, each empty one to the next man.
        {
            let vs = self.vehicles.get_mut(&id).expect("listed");
            let mut it = men.iter();
            for st in vs.seats.iter_mut().filter(|st| st.hp == 0) {
                let Some(&m) = it.next() else { break };
                *st = Seat {
                    hp: ctx.ents.combat[m.index()].hp.min(crew_hp).max(1),
                    untrained,
                    ..Seat::default()
                };
            }
        }
        // The men leave the world (not a loss).
        for &m in men {
            let sq = self.squads.get_mut(&q).expect("boarding squad");
            if let Some(k) = sq.members.iter().position(|x| *x == m) {
                sq.members.remove(k);
                if k < sq.nav.slots.len() {
                    sq.nav.slots.remove(k);
                }
            }
            for queue in &mut self.path_queue.queues {
                queue.retain(|r| *r != PathReq::Unit(m));
            }
            ctx.ents.despawn(m);
            ctx.hash.mark_entity_chunk(m.chunk());
        }
        if self.squads[&q].members.is_empty() {
            self.squads.remove(&q);
            for queue in &mut self.path_queue.queues {
                queue.retain(|r| *r != PathReq::Squad(q));
            }
        } else {
            let sq = self.squads.get_mut(&q).expect("boarding squad");
            sq.order = Order::Idle;
            sq.focus = sq.focus.filter(|f| !men.contains(f));
        }
        // The vehicle: live again, the boarding player's, a new squad.
        let vt = self
            .setup
            .squad_types
            .iter()
            .position(|t| t.vehicle == Some(type_idx))
            .expect("a vehicle squad type") as u16;
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
                focus: None,
                launcher: None,
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
            } else {
                pl.crew.recrewed += 1;
            }
        }
        self.forget(ctx.ents, ctx.hash, id);
        self.mark_wreck_block(ctx.ents, id);
        let ev = ArmourEv::new(ArmourEventKind::Recrewed, id, p.0, last.0)
            .component(if captured {
                RECREW_CAPTURED
            } else {
                RECREW_OWN
            })
            .at(t.pos);
        self.push_event(events, ev);
    }

    /// Over the cap, the oldest abandoned vehicle is scrapped (a wreck).
    fn abandoned_cap(&mut self, ctx: &mut Ctx, stats: &mut Stats) {
        let cap = self.setup.rules.vehicles.bail.abandoned_cap as usize;
        loop {
            let list: Vec<(u64, SimId)> = self
                .vehicles
                .iter()
                .filter_map(|(&id, vs)| vs.abandoned.map(|t| (t, id)))
                .collect();
            if list.len() <= cap {
                return;
            }
            let (_, oldest) = *list.iter().min().expect("over the cap");
            let owner = ctx.ents.owner[oldest.index()];
            let (alive, by) = {
                let vs = &self.vehicles[&oldest];
                (
                    vs.seats.iter().filter(|st| st.hp > 0).count() as u32,
                    vs.last_attacker,
                )
            };
            if alive > 0 {
                if let Some(pl) = self.players.get_mut(owner.0 as usize) {
                    pl.losses += alive;
                }
                if let Some(pl) = by
                    .filter(|&b| b != owner.0)
                    .and_then(|b| self.players.get_mut(b as usize))
                {
                    pl.kills += alive;
                    pl.kills_by[kill_class::OTHER as usize] += alive;
                }
            }
            if let Some(pl) = self.players.get_mut(owner.0 as usize) {
                pl.crew.scrapped += 1;
            }
            let _ = stats;
            let removed = self.make_wreck(ctx.ents, ctx.hash, oldest, ctx.tick);
            for gone in std::iter::once(oldest).chain(removed) {
                self.forget(ctx.ents, ctx.hash, gone);
            }
        }
    }

    /// Crew squads: dismissal in base, stale links and Recrew orders.
    fn crew_squads(&mut self, ctx: &mut Ctx) {
        let s = &self.setup;
        let dismiss = s.rules.vehicles.bail.crew_dismiss_s * s.tick_hz.max(1);
        let mut gone: Vec<SquadId> = Vec::new();
        let mut idle: Vec<SquadId> = Vec::new();
        for (&q, sq) in &self.squads {
            if let Order::Recrew { vehicle } = sq.order
                && ctx.ents.kind.get(vehicle.index()) != Some(&EntityKind::Abandoned)
            {
                idle.push(q);
                continue;
            }
            if s.squad_types[sq.type_idx as usize].role != SquadRole::Crew
                || sq.order != Order::Idle
                || sq.nav.phase != NavPhase::Holding
                || sq.nav.phase_ticks < dismiss
                || sq.members.is_empty()
            {
                continue;
            }
            let base = &s.map.bases[s.players[sq.owner.0 as usize].base as usize];
            let br: Fx = base.radius.into();
            let lead = ctx.ents.transform[sq.lead(ctx.ents).index()].pos.xy();
            if (lead - FxVec2::from(base.center)).length_sq_raw()
                <= FxVec2::new(br, Fx::ZERO).length_sq_raw()
            {
                gone.push(q);
            }
        }
        for q in idle {
            let sq = self.squads.get_mut(&q).expect("listed");
            sq.order = Order::Idle;
        }
        for q in gone {
            let sq = self.squads.remove(&q).expect("listed");
            for queue in &mut self.path_queue.queues {
                queue.retain(|r| {
                    *r != PathReq::Squad(q)
                        && !matches!(r, PathReq::Unit(m) if sq.members.contains(m))
                });
            }
            if let Some(pl) = self.players.get_mut(sq.owner.0 as usize) {
                pl.crew.home += sq.members.len() as u32;
            }
            for m in sq.members {
                ctx.ents.despawn(m);
                ctx.hash.mark_entity_chunk(m.chunk());
            }
        }
        // Links to crew squads that are gone, boarding squads that left.
        for (&id, vs) in self.vehicles.iter_mut() {
            if vs.crew_squad.is_some_and(|q| !self.squads.contains_key(&q)) {
                vs.crew_squad = None;
            }
            if vs.board.is_some_and(|(q, _)| {
                self.squads
                    .get(&q)
                    .is_none_or(|sq| sq.order != (Order::Recrew { vehicle: id }))
            }) {
                vs.board = None;
            }
        }
    }
}
