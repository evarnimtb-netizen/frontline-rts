//! Vehicle and escort orders (S3, `docs/slices/S3-armour.md` "Commands",
//! "Escort").
//!
//! The `*Units` vehicle commands take the S1b unit list: at most
//! [`crate::unit::MAX_UNITS`] ids, sorted and deduplicated when applied;
//! own live vehicle ids stand for their squads of one, own soldiers are
//! handled as each command says, everything else is dropped. A command
//! that leaves nothing to do is rejected (counted, no effect).
//!
//! * **`ReverseUnits`**: vehicles get a `Move` to the target that they
//!   drive backwards, the front kept toward the enemy; soldiers a
//!   `MoveUnits`.
//! * **`FaceUnits`**: vehicles stop and pivot until the hull front points
//!   at the point (within `vehicles.driving.face_done_deg`), then hold. Any later move or
//!   stop order ends the turn.
//! * **`AttackUnit`**: the target must be a live enemy vehicle the player
//!   sees. Vehicles keep it as `VehicleState::attack` (their gunner takes
//!   it first while it is seen and in range, `crate::gunnery`); AT teams
//!   with a man in the set keep it as `Launcher::attack` (the launcher
//!   takes it first within its engagement distances). Every Intent stage
//!   ([`MatchState::approach_attacks`]) a vehicle whose gunner does not
//!   have it (out of range, or no part in a line of fire) drives toward it
//!   while it is seen, re-aimed once a second when it has moved more than
//!   `vehicles.driving.approach_slack_m`; once the gunner has it, the approach stops and
//!   the vehicle holds. A move, stop or retreat order clears the attack.
//! * **`SetAmmo`**: the vehicles' `ammo_choice`.
//! * **`RepairUnits`**: each vehicle's crew starts the field repair of its
//!   worst repairable component (`MatchState::start_repair`). A move,
//!   stop or retreat order ends a repair.
//! * **`FollowUnits`**: the named vehicle must be an own live vehicle.
//!   Squads named whole get the squad order `Order::Follow`, other men a
//!   detached `UnitOrder::Follow`; vehicle ids are dropped.
//! * **`SuppressUnits`** (S3b, docs/slices/S3b-balance.md B2): 1 or 2
//!   points on the map (a second one further than
//!   `combat.suppress.line_max_m` from the first is pulled in along the
//!   line). Vehicles stop and keep the line as `VehicleState::suppress`
//!   (`crate::gunnery` fires their MGs at it, and their HE at its middle);
//!   men whose weapon fires HE (mortars) are dropped; squads named whole
//!   get `Order::Suppress` and hold where they stand, other men a detached
//!   `UnitOrder::Suppress`, holding (`crate::combat` "Suppress" fires at
//!   it). It lasts until another order.
//!
//! **Escort steering** ([`MatchState::steer_escorts`], Intent stage, before
//! squad steering): every tick the followers of each vehicle are listed
//! afresh from the orders (ascending soldier id, the saved
//! `VehicleState::followers`); orders naming a vehicle that is gone become
//! a Stop. Follower `k` stands in file `k / n`, row `k % n` (`n` the rules'
//! `vehicles.escort.file_men`): rows `gap_mm` apart starting `behind_m`
//! behind the hull,
//! on the line from the nearest enemy the owner sees (soldier or vehicle)
//! through the vehicle, else from the vehicle's move goal, else straight
//! behind the hull; further files stand `gap_mm` to alternate
//! sides. Squads hold their slots (squad steering walks the men there, at
//! catch-up pace when far) and men face the enemy; detached followers walk
//! to theirs. A vehicle with followers drives no faster than the slowest
//! follower's squad walks (`crate::vehicle`).

use crate::ecs::{Entities, EntityKind, PlayerId, SimId};
use crate::fx::{Fx, FxVec2};
use crate::game::{Ctx, MatchState, NavPhase, Order, SquadId};
use crate::unit::UnitOrder;
use crate::vehicle::{self, AmmoChoice};

// S3b: the face tolerance, approach slack and escort file layout are rules
// data (`[vehicles.driving]`, `[vehicles.escort]`).

impl MatchState {
    /// `ReverseUnits` (see the module docs).
    pub(crate) fn units_reverse(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        ids: &[SimId],
        target: FxVec2,
    ) -> bool {
        let vehicles = self.vehicle_squads(ctx, p, ids);
        for &id in &vehicles {
            self.order_move(ctx, p, id, Order::Move { target });
            let v = self.squads[&id].members[0];
            if let Some(vs) = self.vehicles.get_mut(&v) {
                vs.reverse = true;
            }
        }
        let men = self
            .unit_set(ctx, p, ids)
            .is_some_and(|men| self.units_move(ctx, p, &men, UnitOrder::Move { target }));
        men || !vehicles.is_empty()
    }

    /// `FaceUnits` (see the module docs).
    pub(crate) fn units_face(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        ids: &[SimId],
        toward: FxVec2,
    ) -> bool {
        if !ctx.terrain.contains(toward) {
            return false;
        }
        let vehicles = self.vehicle_squads(ctx, p, ids);
        for &id in &vehicles {
            self.order_stop(ctx, p, id);
            let v = self.squads[&id].members[0];
            if let Some(vs) = self.vehicles.get_mut(&v) {
                vs.face = Some(toward);
            }
        }
        !vehicles.is_empty()
    }

    /// `AttackUnit` (see the module docs).
    pub(crate) fn units_attack(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        ids: &[SimId],
        target: SimId,
    ) -> bool {
        let ti = target.index();
        // S3c: any abandoned vehicle may be ordered shot at (denial).
        let abandoned = ctx.ents.kind.get(ti) == Some(&EntityKind::Abandoned);
        if ids.len() > crate::unit::MAX_UNITS
            || (!abandoned
                && (ctx.ents.kind.get(ti) != Some(&EntityKind::Vehicle)
                    || !self.teams.enemy_seen(
                        p.0,
                        ctx.ents.owner[ti].0,
                        ctx.ents.combat[ti].seen_by,
                    )))
        {
            return false;
        }
        let vehicles = self.vehicle_squads(ctx, p, ids);
        for &id in &vehicles {
            let v = self.squads[&id].members[0];
            if let Some(vs) = self.vehicles.get_mut(&v) {
                vs.attack = Some(target);
                vs.face = None;
            }
        }
        let mut teams = 0usize;
        // S3c: launcher teams keep their rockets for live vehicles.
        if abandoned {
            return !vehicles.is_empty();
        }
        if let Some(men) = self.unit_set(ctx, p, ids) {
            let mut squads: Vec<SquadId> = men
                .iter()
                .filter_map(|m| ctx.ents.squad[m.index()])
                .collect();
            squads.sort_unstable();
            squads.dedup();
            for sid in squads {
                let Some(sq) = self.squads.get_mut(&sid) else {
                    continue;
                };
                if let Some(l) = sq.launcher.as_mut() {
                    l.attack = Some(target);
                    teams += 1;
                }
                // S4: squads with single-shot AT items (Panzerfausts) aim
                // their nearest carrier at it first, at the next search.
                if let Some(pz) = sq.pzf.as_mut() {
                    pz.attack = Some(target);
                    pz.search = 0;
                    teams += 1;
                }
            }
        }
        !vehicles.is_empty() || teams > 0
    }

    /// `SetAmmo`.
    pub(crate) fn units_set_ammo(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        ids: &[SimId],
        ammo: AmmoChoice,
    ) -> bool {
        let vehicles = self.vehicle_squads(ctx, p, ids);
        for &id in &vehicles {
            let v = self.squads[&id].members[0];
            if let Some(vs) = self.vehicles.get_mut(&v) {
                vs.ammo_choice = ammo;
            }
        }
        !vehicles.is_empty()
    }

    /// `RepairUnits`: accepted if any crew starts a repair.
    pub(crate) fn units_repair(&mut self, ctx: &mut Ctx, p: PlayerId, ids: &[SimId]) -> bool {
        let vehicles = self.vehicle_squads(ctx, p, ids);
        let mut any = false;
        for &id in &vehicles {
            let v = self.squads[&id].members[0];
            any |= self.start_repair(ctx.ents, v);
        }
        any
    }

    /// `FollowUnits` (see the module docs).
    pub(crate) fn units_follow(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        ids: &[SimId],
        vehicle: SimId,
    ) -> bool {
        if !self.escorts(ctx.ents, p, vehicle) {
            return false;
        }
        let Some(men) = self.unit_set(ctx, p, ids) else {
            return false;
        };
        let (whole, rest) = self.split_whole(ctx, &men);
        for id in whole {
            self.clear_detached(ctx, id);
            let sq = self.squads.get_mut(&id).expect("live squad");
            sq.order = Order::Follow { vehicle };
            hold_here(sq, ctx);
        }
        for m in rest {
            let t = ctx.ents.transform[m.index()];
            self.call_off_throw(ctx, m);
            self.detach(
                ctx,
                m,
                UnitOrder::Follow { vehicle },
                vehicle::yaw_dir(t.yaw),
            );
            let nav = &mut ctx.ents.unit[m.index()].nav;
            nav.phase = NavPhase::Holding;
            nav.goal = t.pos.xy();
        }
        true
    }

    /// `SuppressUnits` (see the module docs).
    pub(crate) fn units_suppress(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        ids: &[SimId],
        line: &[FxVec2],
    ) -> bool {
        let Some(line) = self.suppress_line(ctx, line) else {
            return false;
        };
        let vehicles = self.vehicle_squads(ctx, p, ids);
        for &id in &vehicles {
            self.order_stop(ctx, p, id);
            let v = self.squads[&id].members[0];
            if let Some(vs) = self.vehicles.get_mut(&v) {
                vs.suppress = Some(line);
            }
        }
        let men = self.unit_set(ctx, p, ids).map(|mut men| {
            men.retain(|&m| {
                let sq = ctx.ents.squad[m.index()].and_then(|s| self.squads.get(&s));
                sq.is_some_and(|_| {
                    let w = ctx.ents.kit[m.index()].weapon;
                    self.setup.weapons[w as usize].he.is_none()
                })
            });
            men
        });
        let Some(men) = men.filter(|m| !m.is_empty()) else {
            return !vehicles.is_empty();
        };
        let mid = (line[0] + line[1]).div_int(2);
        let (whole, rest) = self.split_whole(ctx, &men);
        for id in whole {
            self.clear_detached(ctx, id);
            let sq = self.squads.get_mut(&id).expect("live squad");
            sq.order = Order::Suppress { line };
            hold_here(sq, ctx);
            let lead = ctx.ents.transform[sq.lead(ctx.ents).index()].pos.xy();
            let facing = (mid - lead).normalize_or_zero();
            if facing != FxVec2::ZERO {
                sq.nav.facing = facing;
            }
        }
        for m in rest {
            let t = ctx.ents.transform[m.index()];
            self.call_off_throw(ctx, m);
            let mut facing = (mid - t.pos.xy()).normalize_or_zero();
            if facing == FxVec2::ZERO {
                facing = vehicle::yaw_dir(t.yaw);
            }
            self.detach(ctx, m, UnitOrder::Suppress { line }, facing);
            let nav = &mut ctx.ents.unit[m.index()].nav;
            nav.phase = NavPhase::Holding;
            nav.goal = t.pos.xy();
        }
        true
    }

    /// A `SuppressUnits` line: 1 or 2 points on the map, the second pulled
    /// to within `combat.suppress.line_max_m` of the first.
    pub(crate) fn suppress_line(&self, ctx: &Ctx, line: &[FxVec2]) -> Option<[FxVec2; 2]> {
        let (a, b) = match line {
            [a] => (*a, *a),
            [a, b] => (*a, *b),
            _ => return None,
        };
        if !ctx.terrain.contains(a) || !ctx.terrain.contains(b) {
            return None;
        }
        let max = Fx::from_int(self.setup.rules.combat.suppress.line_max_m as i32);
        let d = b - a;
        let b = if d.length_sq_raw() > FxVec2::new(max, Fx::ZERO).length_sq_raw() {
            ctx.terrain.clamp(a + d.normalize_or_zero() * max)
        } else {
            b
        };
        Some([a, b])
    }

    /// The `AttackUnit` approach (Intent stage, ascending vehicle id; see
    /// the module docs).
    pub(crate) fn approach_attacks(&mut self, ctx: &mut Ctx) {
        let hz = self.setup.tick_hz.max(1) as u64;
        let slack = FxVec2::from_int(self.setup.rules.vehicles.driving.approach_slack_m as i32, 0)
            .length_sq_raw();
        let ids: Vec<SimId> = self
            .vehicles
            .iter()
            .filter(|(_, vs)| vs.attack.is_some())
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            let i = id.index();
            if ctx.ents.kind[i] != EntityKind::Vehicle {
                continue;
            }
            let owner = ctx.ents.owner[i];
            let vs = self.vehicles.get_mut(&id).expect("listed");
            let t = vs.attack.expect("listed");
            let ti = t.index();
            let abandoned = ctx.ents.kind.get(ti) == Some(&EntityKind::Abandoned);
            if !abandoned
                && (ctx.ents.kind.get(ti) != Some(&EntityKind::Vehicle)
                    || !self.teams.hostile_id(owner, ctx.ents.owner[ti]))
            {
                vs.attack = None;
                continue;
            }
            if !self.teams.sees(owner.0, ctx.ents.combat[ti].seen_by) {
                continue;
            }
            let Some(sid) = ctx.ents.squad[i] else {
                continue;
            };
            let has_it = vs.target == Some(t);
            let v = &self.setup.vehicles[vs.type_idx as usize];
            let immobile = vs.immobile(v);
            let sq = self.squads.get_mut(&sid).expect("vehicle squad");
            let approaching = matches!(sq.order, Order::Move { .. });
            if has_it {
                if approaching {
                    sq.order = Order::Idle;
                    vehicle::hold_vehicle(&mut sq.nav, ctx.ents.transform[i].pos.xy());
                }
                continue;
            }
            let tpos = ctx.ents.transform[ti].pos.xy();
            let stale = match sq.order {
                Order::Idle => true,
                Order::Move { target } => {
                    (ctx.tick + id.0 as u64).is_multiple_of(hz)
                        && (target - tpos).length_sq_raw() > slack
                }
                _ => false,
            };
            if !stale || immobile {
                continue;
            }
            sq.order = Order::Move { target: tpos };
            vs.reverse = false;
            self.request_path(sid, tpos);
        }
    }

    /// Escort steering (see the module docs).
    pub(crate) fn steer_escorts(&mut self, ctx: &mut Ctx) {
        if self.vehicles.is_empty() {
            return;
        }
        // Followers from the orders, ascending soldier id.
        let mut follows: Vec<(SimId, SimId)> = Vec::new();
        let mut stop_squads: Vec<(PlayerId, SquadId)> = Vec::new();
        let mut stop_men: Vec<(PlayerId, SimId)> = Vec::new();
        for i in 0..ctx.ents.len() {
            if ctx.ents.kind[i] != EntityKind::Soldier {
                continue;
            }
            let m = SimId(i as u32);
            let owner = ctx.ents.owner[i];
            let u = &ctx.ents.unit[i];
            let (vehicle, squad) = if u.detached {
                match u.order {
                    UnitOrder::Follow { vehicle } => (vehicle, None),
                    _ => continue,
                }
            } else {
                let Some(sid) = ctx.ents.squad[i] else {
                    continue;
                };
                match self.squads.get(&sid).map(|s| &s.order) {
                    Some(&Order::Follow { vehicle }) => (vehicle, Some(sid)),
                    _ => continue,
                }
            };
            if self.escorts(ctx.ents, owner, vehicle) {
                follows.push((vehicle, m));
            } else if let Some(sid) = squad {
                if stop_squads.last() != Some(&(owner, sid)) {
                    stop_squads.push((owner, sid));
                }
            } else {
                stop_men.push((owner, m));
            }
        }
        stop_squads.sort_unstable();
        stop_squads.dedup();
        for (p, sid) in stop_squads {
            self.order_stop(ctx, p, sid);
        }
        for (p, m) in stop_men {
            self.units_stop(ctx, p, &[m]);
        }
        follows.sort_unstable();
        for vs in self.vehicles.values_mut() {
            vs.followers.clear();
        }
        let esc = &self.setup.rules.vehicles.escort;
        let behind = Fx::from_int(esc.behind_m as i32);
        let gap = Fx::from_milli(esc.gap_mm as i32);
        let file_men = esc.file_men as usize;
        let mut k0 = 0usize;
        while k0 < follows.len() {
            let v = follows[k0].0;
            let k1 = k0 + follows[k0..].iter().take_while(|f| f.0 == v).count();
            let men: Vec<SimId> = follows[k0..k1].iter().map(|f| f.1).collect();
            k0 = k1;
            let vi = v.index();
            let owner = ctx.ents.owner[vi];
            let t = ctx.ents.transform[vi];
            let vpos = t.pos.xy();
            let Some(vs) = self.vehicles.get(&v) else {
                continue;
            };
            let geo = self.vderived.geo[vs.type_idx as usize];
            // From the nearest enemy the owner sees, else from the goal.
            let enemy = (0..ctx.ents.len())
                .filter(|&j| {
                    matches!(ctx.ents.kind[j], EntityKind::Soldier | EntityKind::Vehicle)
                        && self.teams.enemy_seen(
                            owner.0,
                            ctx.ents.owner[j].0,
                            ctx.ents.combat[j].seen_by,
                        )
                })
                .map(|j| {
                    let q = ctx.ents.transform[j].pos.xy();
                    ((q - vpos).length_sq_raw(), j, q)
                })
                .min()
                .map(|(_, _, q)| q);
            let goal = ctx.ents.squad[vi]
                .and_then(|s| self.squads.get(&s))
                .filter(|s| matches!(s.nav.phase, NavPhase::Moving | NavPhase::PathPending))
                .map(|s| s.nav.goal);
            let mut dir = match (enemy, goal) {
                (Some(e), _) => (vpos - e).normalize_or_zero(),
                (None, Some(g)) => (vpos - g).normalize_or_zero(),
                _ => FxVec2::ZERO,
            };
            if dir == FxVec2::ZERO {
                dir = -vehicle::yaw_dir(t.yaw);
            }
            let side = dir.perp();
            let start = geo.hull_half.x + behind;
            let slot = |k: usize| {
                let row = (k % file_men) as i32;
                let file = (k / file_men) as i32;
                // Files 1, 2, 3 ... alternate left and right of the first.
                let lateral = gap.mul_int((file + 1) / 2);
                let lateral = if file % 2 == 1 { lateral } else { -lateral };
                ctx.terrain
                    .clamp(vpos + dir * (start + gap.mul_int(row)) + side * lateral)
            };
            let facing = -dir;
            for (k, &m) in men.iter().enumerate() {
                let s = slot(k);
                let mi = m.index();
                if ctx.ents.unit[mi].detached {
                    let nav = &mut ctx.ents.unit[mi].nav;
                    nav.phase = NavPhase::Holding;
                    nav.goal = s;
                    nav.facing = facing;
                    ctx.hash.mark_entity_chunk(m.chunk());
                    continue;
                }
                let Some(sq) = ctx.ents.squad[mi].and_then(|sid| self.squads.get_mut(&sid)) else {
                    continue;
                };
                if !matches!(sq.nav.phase, NavPhase::Holding) {
                    hold_here(sq, ctx);
                }
                if let Some(j) = sq.members.iter().position(|&x| x == m) {
                    sq.nav.slots[j] = s;
                }
                sq.nav.facing = facing;
                sq.nav.goal = vpos;
            }
            if let Some(vs) = self.vehicles.get_mut(&v) {
                vs.followers = men;
            }
        }
    }

    /// The walking speed of `vehicle`'s slowest follower's squad, if it
    /// has followers (the escort speed cap).
    pub(crate) fn escort_speed(&self, ents: &Entities, vehicle: SimId) -> Option<Fx> {
        let vs = self.vehicles.get(&vehicle)?;
        vs.followers
            .iter()
            .filter_map(|m| ents.squad[m.index()].and_then(|s| self.squads.get(&s)))
            .map(|sq| Fx::from(self.setup.squad_types[sq.type_idx as usize].speed))
            .min()
    }
}

/// A squad holds where its men stand (slots on their positions).
fn hold_here(sq: &mut crate::game::Squad, ctx: &Ctx) {
    let nav = &mut sq.nav;
    nav.phase = NavPhase::Holding;
    nav.path.clear();
    nav.next = 0;
    nav.partial = false;
    nav.phase_ticks = 0;
    nav.slots = sq
        .members
        .iter()
        .map(|m| ctx.ents.transform[m.index()].pos.xy())
        .collect();
}
