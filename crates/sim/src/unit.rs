//! Soldier-level orders and detached men (S1b "Orders on a soldier set").
//!
//! Squads stay the unit of organisation, but the player may order any set
//! of his soldiers. Each soldier carries a [`Unit`] in the `unit` entity
//! column:
//!
//! * **Stance order** ([`StanceOrder`], `SetStance`): `Auto` keeps the S1
//!   automatic stance. `Stand`, `Crouch` and `Prone` hold that posture, but
//!   suppression may still force a lower one (crouched, pinned prone), never
//!   a higher one. In a trench a man still crouches below the lip (or lies
//!   on the floor when ordered prone) and rises to the fire step to shoot.
//!   A man ordered prone walks at the rules' `prone_speed_pct`. It never
//!   detaches anyone.
//! * **Detached** men: a man given `MoveUnits`, `AttackMoveUnits`,
//!   `StopUnits`, `RetreatUnits` or `DigUnits` leaves his formation slot and
//!   follows his own [`UnitOrder`] with his own path ([`UnitNav`]), but stays
//!   in his squad (strength, cards, AI). Squad steering, squad digging,
//!   focus and mortar fire skip him; he still auto-acquires targets (by his
//!   own order's engage rule, without the squad focus), takes cover on
//!   arrival, gets suppressed and digs.
//! * **Regroup:** any squad-level order (`Move`, `AttackMove`, `Stop`,
//!   `Retreat`, `Dig`, `Barrage`, `Regroup`), or a unit order naming every
//!   living man of a squad, clears `detached` for the whole squad. If the
//!   formation leader was detached, the first man still in formation leads
//!   instead. `Stop`, an in-range `Barrage` and `Regroup` of a squad that is
//!   not walking form a line around the leader, so the men walk back.
//!
//! **Vehicles (S3).** A vehicle id in `MoveUnits`, `AttackMoveUnits`,
//! `RetreatUnits` or `StopUnits` stands for its squad of one, which gets the
//! squad order; soldiers in the same command are handled as above. The
//! vehicle commands (`ReverseUnits`, `FaceUnits`, `AttackUnit`, `SetAmmo`,
//! `RepairUnits`, `FollowUnits`) are in `crate::orders`. Every other
//! soldier-level command drops vehicle ids. `DigUnits` keeps only men who
//! may dig the named profile (the AT ditch: engineers).
//!
//! Several men sent to one point get slots on a line through it across the
//! direction from their centroid (spacing of the lowest id's squad type),
//! assigned by where they stand along the line, and each paths to his own
//! slot. `DigUnits` splits the line among only the named diggers in a
//! [`UnitDig`] job kept in the match state (`crate::dig` works it); men
//! sent along the line are assigned segments in order along it.
//!
//! Detached steering runs serially in ascending id after squad steering
//! (Intent stage). Path requests share the per-player queues with squads
//! ([`PathReq`]).

use frontline_content::DigProfile;
use serde::{Deserialize, Serialize};

use crate::dig::DigJob;
use crate::ecs::{EntityKind, PlayerId, SimId};
use crate::fx::{Fx, FxVec2};
use crate::game::{Ctx, MatchState, NavPhase, Order, OrderKind, PathReq, SquadId};
use crate::squad::{self, assign_by_axis, line_slots, point_along, polyline_length};

/// Most ids in one soldier-level command.
pub const MAX_UNITS: usize = 120;
/// A detached man moves on to his next waypoint within this distance (m).
const WAYPOINT_REACH_M: i32 = 2;
/// Arriving gives up after this long (s).
const ARRIVE_TIMEOUT_S: u32 = 20;

/// Stance a soldier was ordered to keep (`SetStance`).
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
#[repr(u8)]
pub enum StanceOrder {
    /// The automatic S1 stance.
    #[default]
    Auto = 0,
    Stand = 1,
    Crouch = 2,
    Prone = 3,
}

/// A detached soldier's own order.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum UnitOrder {
    #[default]
    Idle,
    Move {
        target: FxVec2,
    },
    /// Move, engaging enemies on the way: he halts while he has a target.
    AttackMove {
        target: FxVec2,
    },
    /// To the own base, faster; ignores enemies.
    Retreat,
    /// Digging his share of unit dig job `job` (`MatchState::unit_digs`).
    /// Becomes `Idle` when the trench is finished.
    Dig {
        job: u32,
    },
    /// Escorting own vehicle `vehicle` (S3 `FollowUnits`, `crate::orders`).
    Follow {
        vehicle: SimId,
    },
}

impl UnitOrder {
    pub fn kind(&self) -> OrderKind {
        match self {
            UnitOrder::Idle => OrderKind::Idle,
            UnitOrder::Move { .. } => OrderKind::Move,
            UnitOrder::AttackMove { .. } => OrderKind::AttackMove,
            UnitOrder::Retreat => OrderKind::Retreat,
            UnitOrder::Dig { .. } => OrderKind::Dig,
            UnitOrder::Follow { .. } => OrderKind::Follow,
        }
    }
}

/// A detached soldier's navigation state (one man's [`crate::game::Nav`]).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct UnitNav {
    pub phase: NavPhase,
    /// His own spot: where he walks and stands.
    pub goal: FxVec2,
    /// Smoothed waypoints; the last is the goal (or a partial end).
    pub path: Vec<FxVec2>,
    /// Index of the waypoint he walks to.
    pub next: u32,
    /// The path stops short of the goal (search cap); ask again at its end.
    pub partial: bool,
    /// Unit vector he faces once there.
    pub facing: FxVec2,
    /// Ticks spent in the current phase.
    pub phase_ticks: u32,
}

/// Soldier-level order state (see the module docs). A man in formation
/// has `detached == false`, `order == Idle` and a default `nav`.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Unit {
    pub stance: StanceOrder,
    pub detached: bool,
    pub order: UnitOrder,
    pub nav: UnitNav,
}

impl Unit {
    /// Formation state, keeping the stance order.
    fn rejoin(&mut self) {
        *self = Unit {
            stance: self.stance,
            ..Unit::default()
        };
    }
}

/// A trench dug by a set of detached men (`DigUnits`).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct UnitDig {
    pub owner: PlayerId,
    pub line: Vec<FxVec2>,
    pub job: DigJob,
}

/// Unit vector of heading `yaw`.
fn yaw_dir(yaw: Fx) -> FxVec2 {
    let (s, c) = yaw.sin_cos();
    FxVec2::new(c, s)
}

impl MatchState {
    // ---------------------------------------------------------- commands --

    /// The command's men: at most [`MAX_UNITS`] ids, sorted, deduplicated,
    /// keeping live soldiers of `p` in a squad. `None` if nothing is left.
    pub(crate) fn unit_set(&self, ctx: &Ctx, p: PlayerId, ids: &[SimId]) -> Option<Vec<SimId>> {
        if ids.is_empty() || ids.len() > MAX_UNITS {
            return None;
        }
        let ents = &*ctx.ents;
        let mut men = ids.to_vec();
        men.sort_unstable();
        men.dedup();
        men.retain(|&m| {
            let i = m.index();
            i < ents.len()
                && ents.kind[i] == EntityKind::Soldier
                && ents.owner[i] == p
                && ents.squad[i].is_some_and(|s| self.squads.contains_key(&s))
        });
        (!men.is_empty()).then_some(men)
    }

    /// The vehicle squads `ids` names (S3: a vehicle is a squad of one, so
    /// its id stands for its squad), ascending. Empty for more than
    /// [`MAX_UNITS`] ids.
    pub(crate) fn vehicle_squads(&self, ctx: &Ctx, p: PlayerId, ids: &[SimId]) -> Vec<SquadId> {
        if ids.len() > MAX_UNITS {
            return Vec::new();
        }
        let ents = &*ctx.ents;
        let mut out: Vec<SquadId> = ids
            .iter()
            .filter_map(|&v| {
                let i = v.index();
                (i < ents.len() && ents.kind[i] == EntityKind::Vehicle && ents.owner[i] == p)
                    .then(|| ents.squad[i])
                    .flatten()
            })
            .filter(|s| self.squads.contains_key(s))
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Splits sorted `men` into squads named whole (every living man) and
    /// the other men, both ascending.
    pub(crate) fn split_whole(&self, ctx: &Ctx, men: &[SimId]) -> (Vec<SquadId>, Vec<SimId>) {
        let mut touched: Vec<SquadId> = men
            .iter()
            .filter_map(|m| ctx.ents.squad[m.index()])
            .collect();
        touched.sort_unstable();
        touched.dedup();
        let whole: Vec<SquadId> = touched
            .into_iter()
            .filter(|s| {
                self.squads[s]
                    .members
                    .iter()
                    .all(|m| men.binary_search(m).is_ok())
            })
            .collect();
        let rest = men
            .iter()
            .copied()
            .filter(|m| ctx.ents.squad[m.index()].is_none_or(|s| whole.binary_search(&s).is_err()))
            .collect();
        (whole, rest)
    }

    /// `MoveUnits`, `AttackMoveUnits` and `RetreatUnits`.
    pub(crate) fn units_move(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        ids: &[SimId],
        order: UnitOrder,
    ) -> bool {
        let squad_order = match order {
            UnitOrder::Move { target } => Order::Move { target },
            UnitOrder::AttackMove { target } => Order::AttackMove { target },
            _ => Order::Retreat,
        };
        // Vehicles in the set get the squad order (S3).
        let vehicles = self.vehicle_squads(ctx, p, ids);
        for &id in &vehicles {
            self.order_move(ctx, p, id, squad_order.clone());
        }
        let Some(men) = self.unit_set(ctx, p, ids) else {
            return !vehicles.is_empty();
        };
        let (whole, rest) = self.split_whole(ctx, &men);
        for id in whole {
            self.order_move(ctx, p, id, squad_order.clone());
        }
        if rest.is_empty() {
            return true;
        }
        let target = match order {
            UnitOrder::Move { target } | UnitOrder::AttackMove { target } => {
                ctx.terrain.clamp(target)
            }
            _ => self.base_of(p),
        };
        let order = match order {
            UnitOrder::Move { .. } => UnitOrder::Move { target },
            UnitOrder::AttackMove { .. } => UnitOrder::AttackMove { target },
            o => o,
        };
        let ents = &*ctx.ents;
        let (mut sx, mut sy) = (0i64, 0i64);
        for m in &rest {
            let q = ents.transform[m.index()].pos.xy();
            sx += q.x.raw() as i64;
            sy += q.y.raw() as i64;
        }
        let n = rest.len() as i64;
        let centroid = FxVec2::new(Fx::from_raw((sx / n) as i32), Fx::from_raw((sy / n) as i32));
        let mut facing = (target - centroid).normalize_or_zero();
        if facing == FxVec2::ZERO {
            facing = yaw_dir(ents.transform[rest[0].index()].yaw);
        }
        let sq = ents.squad[rest[0].index()].expect("unit_set keeps squad men");
        let ty = &self.setup.squad_types[self.squads[&sq].type_idx as usize];
        let slots: Vec<FxVec2> = line_slots(target, facing, rest.len(), ty.spacing.into())
            .into_iter()
            .map(|q| ctx.terrain.clamp(q))
            .collect();
        let slots = assign_by_axis(&rest, ents, &slots, -facing.perp());
        for (&m, &slot) in rest.iter().zip(&slots) {
            self.detach(ctx, m, order.clone(), facing);
            self.request_unit_path(ctx, m, slot);
        }
        true
    }

    /// `StopUnits`.
    pub(crate) fn units_stop(&mut self, ctx: &mut Ctx, p: PlayerId, ids: &[SimId]) -> bool {
        let vehicles = self.vehicle_squads(ctx, p, ids);
        for &id in &vehicles {
            self.order_stop(ctx, p, id);
        }
        let Some(men) = self.unit_set(ctx, p, ids) else {
            return !vehicles.is_empty();
        };
        let (whole, rest) = self.split_whole(ctx, &men);
        for id in whole {
            self.order_stop(ctx, p, id);
        }
        for m in rest {
            let t = ctx.ents.transform[m.index()];
            self.call_off_throw(ctx, m);
            self.detach(ctx, m, UnitOrder::Idle, yaw_dir(t.yaw));
            let nav = &mut ctx.ents.unit[m.index()].nav;
            nav.phase = NavPhase::Holding;
            nav.goal = t.pos.xy();
        }
        true
    }

    /// `DigUnits`.
    pub(crate) fn units_dig(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        ids: &[SimId],
        line: &[FxVec2],
        profile: DigProfile,
    ) -> bool {
        let Some(mut men) = self.unit_set(ctx, p, ids) else {
            return false;
        };
        men.retain(|m| {
            let sq = &self.squads[&ctx.ents.squad[m.index()].expect("squad man")];
            let ty = &self.setup.squad_types[sq.type_idx as usize];
            self.digs_profile(ty.role, ty.dig_rate_pct, profile)
        });
        if men.is_empty() || !self.dig_line_ok(ctx, line, profile) {
            return false;
        }
        // Exactly one whole squad: its own Dig order.
        let first = ctx.ents.squad[men[0].index()].expect("squad man");
        if self.squads[&first].members.len() == men.len()
            && men.iter().all(|m| ctx.ents.squad[m.index()] == Some(first))
        {
            return self.order_dig(ctx, p, first, line, profile);
        }
        if !self.at_ditch_room(profile) {
            return false;
        }
        let mut job = DigJob::new(
            line,
            men.len(),
            p,
            self.base_of(p),
            ctx.ents,
            ctx.terrain,
            &self.setup.rules,
            profile,
            self.setup.tick_hz,
        );
        // Segments in order along the line, men by where they stand along it.
        let along = (line[line.len() - 1] - line[0]).normalize_or_zero();
        let mut order: Vec<(i64, SimId)> = men
            .iter()
            .map(|&m| (ctx.ents.transform[m.index()].pos.xy().dot_raw(along), m))
            .collect();
        order.sort_unstable();
        let id = self.next_unit_dig;
        self.next_unit_dig += 1;
        let facing = job.facing(line);
        for (seg, &(_, m)) in job.segments.iter_mut().zip(&order) {
            seg.digger = Some(m);
            let spot = ctx
                .terrain
                .clamp(point_along(line, (seg.from + seg.to).div_int(2)));
            self.detach(ctx, m, UnitOrder::Dig { job: id }, facing);
            self.request_unit_path(ctx, m, spot);
        }
        self.unit_digs.insert(
            id,
            UnitDig {
                owner: p,
                line: line.to_vec(),
                job,
            },
        );
        true
    }

    /// `SetStance`.
    pub(crate) fn set_stance(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        ids: &[SimId],
        stance: StanceOrder,
    ) -> bool {
        let Some(men) = self.unit_set(ctx, p, ids) else {
            return false;
        };
        for m in men {
            ctx.ents.unit[m.index()].stance = stance;
            ctx.hash.mark_entity_chunk(m.chunk());
        }
        true
    }

    /// `Regroup`: detached men rejoin; a squad that is not walking forms a
    /// line around its leader.
    pub(crate) fn regroup(&mut self, ctx: &mut Ctx, p: PlayerId, id: SquadId) -> bool {
        if !self.owned(p, id) {
            return false;
        }
        if self.clear_detached(ctx, id) {
            let sq = &self.squads[&id];
            if matches!(sq.nav.phase, NavPhase::Holding | NavPhase::Arriving) {
                let facing = sq.nav.facing;
                self.regroup_here(ctx, id, facing);
            }
        }
        true
    }

    // ----------------------------------------------------------- helpers --

    /// Makes `m` detached with `order`, facing `facing` once there. His
    /// navigation state is reset; the caller sets a goal or a path.
    pub(crate) fn detach(&mut self, ctx: &mut Ctx, m: SimId, order: UnitOrder, facing: FxVec2) {
        let u = &mut ctx.ents.unit[m.index()];
        u.detached = true;
        u.order = order;
        u.nav = UnitNav {
            facing,
            ..UnitNav::default()
        };
        ctx.hash.mark_entity_chunk(m.chunk());
    }

    /// Asks for a path for detached man `m` to `goal`; he keeps his last
    /// target until it is served.
    pub(crate) fn request_unit_path(&mut self, ctx: &mut Ctx, m: SimId, goal: FxVec2) {
        let nav = &mut ctx.ents.unit[m.index()].nav;
        nav.phase = NavPhase::PathPending;
        nav.goal = goal;
        nav.path.clear();
        nav.next = 0;
        nav.partial = false;
        nav.phase_ticks = 0;
        ctx.hash.mark_entity_chunk(m.chunk());
        let q = &mut self.path_queue.queues[ctx.ents.owner[m.index()].0 as usize];
        if !q.contains(&PathReq::Unit(m)) {
            q.push(PathReq::Unit(m));
        }
    }

    /// Clears `detached` for every man of squad `id` (see the module docs).
    /// Returns whether anyone was detached.
    pub(crate) fn clear_detached(&mut self, ctx: &mut Ctx, id: SquadId) -> bool {
        let sq = self.squads.get_mut(&id).expect("live squad");
        let detached = |m: &SimId| ctx.ents.unit[m.index()].detached;
        if !sq.members.iter().any(detached) {
            return false;
        }
        // The first man still in formation leads the regrouped squad.
        if detached(&sq.members[0])
            && let Some(k) = sq.members.iter().position(|m| !detached(m))
        {
            let lead = sq.members.remove(k);
            sq.members.insert(0, lead);
            if k < sq.nav.slots.len() {
                let slot = sq.nav.slots.remove(k);
                sq.nav.slots.insert(0, slot);
            }
        }
        let owner = sq.owner.0 as usize;
        let members = sq.members.clone();
        for &m in &members {
            let u = &mut ctx.ents.unit[m.index()];
            if u.detached {
                u.rejoin();
                ctx.hash.mark_entity_chunk(m.chunk());
            }
        }
        self.path_queue.queues[owner]
            .retain(|r| !matches!(r, PathReq::Unit(m) if members.contains(m)));
        true
    }

    /// Puts squad `id` on a line around its leader, facing `facing`; the men
    /// walk there (Arriving).
    pub(crate) fn regroup_here(&mut self, ctx: &mut Ctx, id: SquadId, facing: FxVec2) {
        let sq = self.squads.get_mut(&id).expect("live squad");
        let spacing: Fx = self.setup.squad_types[sq.type_idx as usize].spacing.into();
        let center = ctx.ents.transform[sq.members[0].index()].pos.xy();
        let mut facing = facing;
        if facing == FxVec2::ZERO {
            facing = yaw_dir(ctx.ents.transform[sq.members[0].index()].yaw);
        }
        let slots: Vec<FxVec2> = line_slots(center, facing, sq.members.len(), spacing)
            .into_iter()
            .map(|q| ctx.terrain.clamp(q))
            .collect();
        let nav = &mut sq.nav;
        nav.slots = assign_by_axis(&sq.members, ctx.ents, &slots, -facing.perp());
        nav.phase = NavPhase::Arriving;
        nav.goal = center;
        nav.path.clear();
        nav.next = 0;
        nav.partial = false;
        nav.facing = facing;
        nav.phase_ticks = 0;
    }

    /// Whether `line` is a valid line for a `profile` dig (as for `Dig`).
    pub(crate) fn dig_line_ok(&self, ctx: &Ctx, line: &[FxVec2], profile: DigProfile) -> bool {
        if !(2..=crate::game::MAX_DIG_POINTS).contains(&line.len())
            || !line.iter().all(|q| ctx.terrain.contains(*q))
        {
            return false;
        }
        let len = polyline_length(line);
        !len.is_zero() && len <= Fx::from(self.setup.rules.dig_profile(profile).max_length)
    }

    // ---------------------------------------------------------- steering --

    /// Steers every detached man for this tick (see the module docs).
    pub(crate) fn steer_units(&mut self, ctx: &mut Ctx) {
        let hz = self.setup.tick_hz;
        let mv = &self.setup.rules.movement;
        let arrive = Fx::from(mv.arrive_radius);
        let arrive_sq = arrive.mul_int(2).mul(arrive.mul_int(2));
        let reach_sq = Fx::from_int(WAYPOINT_REACH_M * WAYPOINT_REACH_M);
        let mut repath = Vec::new();
        for i in 0..ctx.ents.len() {
            if ctx.ents.kind[i] != EntityKind::Soldier || !ctx.ents.unit[i].detached {
                continue;
            }
            let m = SimId(i as u32);
            let Some(sq) = ctx.ents.squad[i].and_then(|s| self.squads.get(&s)) else {
                continue;
            };
            let ty = &self.setup.squad_types[sq.type_idx as usize];
            let mut speed: Fx = ty.speed.into();
            let u = &ctx.ents.unit[i];
            if u.order == UnitOrder::Retreat {
                speed = speed.mul_int(mv.retreat_speed_pct as i32).div_int(100);
            }
            let speed = squad::stance_speed(speed, u.stance, mv.prone_speed_pct);
            let pos = ctx.ents.transform[i].pos.xy();
            let has_target = ctx.ents.combat[i].target.is_some();
            ctx.hash.mark_entity_chunk(m.chunk());
            let u = &mut ctx.ents.unit[i];
            u.nav.phase_ticks = u.nav.phase_ticks.saturating_add(1);
            match u.nav.phase {
                NavPhase::PathPending => {}
                // Halt to fight while he has a target in clear line.
                NavPhase::Moving
                    if matches!(u.order, UnitOrder::AttackMove { .. }) && has_target =>
                {
                    squad::set_target(ctx, m, pos, speed);
                }
                NavPhase::Moving => {
                    let nav = &mut u.nav;
                    let last = nav.path.len() as u32 - 1;
                    while nav.next < last
                        && (nav.path[nav.next as usize] - pos).length_sq() <= reach_sq
                    {
                        nav.next += 1;
                    }
                    let wp = nav.path[nav.next as usize];
                    if nav.next == last && (wp - pos).length_sq() <= reach_sq.max(arrive_sq) {
                        if nav.partial {
                            repath.push((m, nav.goal));
                        } else {
                            let goal = nav.goal;
                            let cover =
                                !matches!(u.order, UnitOrder::Dig { .. } | UnitOrder::Retreat);
                            let goal = if cover {
                                let mut slot = [goal];
                                squad::seek_cover(
                                    sq.owner,
                                    goal,
                                    &[m],
                                    ctx.ents,
                                    ctx.terrain,
                                    (!self.at_ditches.is_empty()).then_some(&self.vderived.block),
                                    &mut slot,
                                );
                                slot[0]
                            } else {
                                goal
                            };
                            let nav = &mut ctx.ents.unit[i].nav;
                            nav.goal = goal;
                            nav.phase = NavPhase::Arriving;
                            nav.phase_ticks = 0;
                        }
                        continue;
                    }
                    squad::set_target(ctx, m, wp, speed);
                }
                NavPhase::Arriving | NavPhase::Holding => {
                    let (goal, facing) = (u.nav.goal, u.nav.facing);
                    let there = (goal - pos).length_sq() <= arrive_sq;
                    if u.nav.phase == NavPhase::Arriving
                        && (there || u.nav.phase_ticks >= ARRIVE_TIMEOUT_S * hz)
                    {
                        u.nav.phase = NavPhase::Holding;
                        u.nav.phase_ticks = 0;
                        if matches!(
                            u.order,
                            UnitOrder::Move { .. }
                                | UnitOrder::AttackMove { .. }
                                | UnitOrder::Retreat
                        ) {
                            u.order = UnitOrder::Idle;
                        }
                    }
                    squad::set_target(ctx, m, goal, speed);
                    // In position: face the line's direction.
                    let t = &mut ctx.ents.transform[i];
                    if there && facing != FxVec2::ZERO {
                        let yaw = Fx::atan2(facing.y, facing.x);
                        if t.yaw != yaw {
                            t.yaw = yaw;
                        }
                    }
                }
            }
        }
        for (m, goal) in repath {
            self.request_unit_path(ctx, m, goal);
        }
    }
}
