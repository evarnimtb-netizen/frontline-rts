//! Match mode (S1 skirmish): players, squads and their orders, call-ins,
//! capture points and the path request queue.
//!
//! A world made by [`crate::World::new_match`] carries a [`MatchState`] next
//! to the entity store. Everything here is saved, loaded and hashed (see
//! `hash::put_game`); squads live in a `BTreeMap` keyed by a never-reused
//! [`SquadId`], so iteration is ascending id. Soldiers point back at their
//! squad through the `squad` entity column.
//!
//! Order handling, all serial in the Intent stage:
//!
//! 1. commands are validated and turned into an [`Order`] plus a navigation
//!    request ([`Nav`]);
//! 2. ready call-ins spawn at their owner's base;
//! 3. the [`PathQueue`] serves path requests round-robin by player within a
//!    node-expansion budget (`crate::path`);
//! 4. digging squads work on their trench (`crate::dig`);
//! 5. squads steer their members (`crate::squad`).
//!
//! Implemented: Move, AttackMove (halts to fight while the squad has a focus
//! target and a man with a clear shot), Stop, Retreat, CallIn, Surrender,
//! Dig (the squad walks to the line, splits it and digs it in passes) and
//! Barrage (a mortar team in range holds and fires; out of range it first
//! walks to 90 % of its range from the target, and gives up when it cannot
//! get into range). Combat (`crate::combat`) and spotting
//! (`crate::spotting`) keep their per-match state here too: squad focus
//! targets, kills and losses, and the spotting cursors. Capture, income,
//! tickets and victory are in `crate::rules`; the AI commander's per-player
//! state (`crate::ai`) is kept here too.

use std::collections::BTreeMap;

use frontline_content::{MatchSetup, SquadRole};
use serde::{Deserialize, Serialize};

use crate::ai::AiState;
use crate::combat;
use crate::command::{Command, CommandKind};
use crate::dig::DigJob;
use crate::ecs::{Entities, EntityKind, PlayerId, SimId};
use crate::fx::{Fx, FxVec2};
use crate::hash::HashCache;
use crate::path::{PathCost, PathScratch, find_path};
use crate::rules::{Defeat, capture_full};
use crate::squad;
use crate::terrain::Heightfield;

/// Most points in a Dig polyline.
pub const MAX_DIG_POINTS: usize = 16;
/// Most rounds in one Barrage order.
pub const MAX_BARRAGE_ROUNDS: u16 = 50;
/// Most queued call-ins per player.
pub const MAX_CALL_IN_QUEUE: usize = 10;
/// Longest stored path (waypoints); a smoothed path is far shorter.
pub const MAX_PATH_POINTS: usize = 8192;

/// Stable squad id. Never reused within a match.
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct SquadId(pub u32);

/// What a squad was told to do.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Order {
    #[default]
    Idle,
    Move {
        target: FxVec2,
    },
    /// Move, engaging enemies on the way (engaging: combat stage).
    AttackMove {
        target: FxVec2,
    },
    /// To the own base, faster; ignores enemies.
    Retreat,
    /// Dig a trench along `line`; `job` tracks who digs what and how far.
    /// The order becomes `Idle` when the trench is finished.
    Dig {
        line: Vec<FxVec2>,
        job: DigJob,
    },
    /// Mortar fire on `target`; `rounds` left. The order becomes `Idle`
    /// after the last round.
    Barrage {
        target: FxVec2,
        rounds: u16,
    },
}

/// [`Order`] without its payload, for the snapshot.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum OrderKind {
    #[default]
    Idle = 0,
    Move = 1,
    AttackMove = 2,
    Retreat = 3,
    Dig = 4,
    Barrage = 5,
}

impl Order {
    pub fn kind(&self) -> OrderKind {
        match self {
            Order::Idle => OrderKind::Idle,
            Order::Move { .. } => OrderKind::Move,
            Order::AttackMove { .. } => OrderKind::AttackMove,
            Order::Retreat => OrderKind::Retreat,
            Order::Dig { .. } => OrderKind::Dig,
            Order::Barrage { .. } => OrderKind::Barrage,
        }
    }
}

/// Where a squad is in getting somewhere.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum NavPhase {
    /// Members stand on `slots`.
    #[default]
    Holding = 0,
    /// Waiting in the path queue.
    PathPending = 1,
    /// Leader follows `path`, members hold formation slots behind him.
    Moving = 2,
    /// Members walk to their `slots` at the destination.
    Arriving = 3,
}

/// Squad navigation state.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Nav {
    pub phase: NavPhase,
    /// Where the leader is headed.
    pub goal: FxVec2,
    /// Smoothed waypoints; the last is the goal (or a partial end).
    pub path: Vec<FxVec2>,
    /// Index of the waypoint the leader walks to.
    pub next: u32,
    /// The path stops short of the goal (search cap); ask again at its end.
    pub partial: bool,
    /// Unit vector: formation forward while moving.
    pub heading: FxVec2,
    /// Unit vector the men face once in position.
    pub facing: FxVec2,
    /// Destination per member (same order as `members`) while arriving or
    /// holding; empty otherwise.
    pub slots: Vec<FxVec2>,
    /// Ticks spent in the current phase.
    pub phase_ticks: u32,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Squad {
    /// Index into `MatchSetup::squad_types`.
    pub type_idx: u16,
    pub owner: PlayerId,
    /// Formation order: `members[0]` leads and follows the path.
    pub members: Vec<SimId>,
    pub order: Order,
    pub nav: Nav,
    /// Enemy the squad concentrates fire on (`crate::combat`).
    pub focus: Option<SimId>,
}

/// A bought squad on its way.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CallIn {
    pub squad_type: u16,
    /// Spawns at the start of this tick.
    pub ready_tick: u64,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PlayerState {
    pub manpower: u32,
    pub tickets: u32,
    pub surrendered: bool,
    /// In purchase order.
    pub call_ins: Vec<CallIn>,
    /// Enemy soldiers this player killed.
    pub kills: u32,
    /// Own soldiers lost.
    pub losses: u32,
    /// Set once the player is out of the match (`crate::rules`).
    pub defeat: Option<Defeat>,
    /// Sum over ticks of the points held (point-ticks), for the end screen.
    pub point_ticks: u64,
    /// Income fraction carried to the next tick (`crate::rules`).
    pub income_acc: u32,
    /// Ticket-drain fraction carried to the next tick.
    pub drain_acc: u32,
}

/// Capture point state (`crate::rules`).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct CapturePoint {
    pub owner: Option<PlayerId>,
    /// Progress toward `capturer`, in percent-ticks of one man's work
    /// (full at `rules::capture_full`). Zero when `capturer` is `None`.
    pub progress: u32,
    /// Never the owner.
    pub capturer: Option<PlayerId>,
    /// Soldiers of two or more players were on the point this tick.
    pub contested: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Outcome {
    #[default]
    Running,
    Won {
        winner: PlayerId,
    },
    /// Every remaining player was knocked out in the same tick.
    Draw,
}

/// Path requests: one FIFO per player served round-robin, and the expansion
/// debt carried over from a search that overran the tick budget.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct PathQueue {
    pub queues: Vec<Vec<SquadId>>,
    /// Player whose queue is served first next.
    pub cursor: u8,
    pub debt: u32,
}

/// All match-mode state. See the module docs.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct MatchState {
    pub(crate) setup: MatchSetup,
    pub(crate) players: Vec<PlayerState>,
    pub(crate) squads: BTreeMap<SquadId, Squad>,
    pub(crate) next_squad: u32,
    pub(crate) points: Vec<CapturePoint>,
    pub(crate) path_queue: PathQueue,
    pub(crate) outcome: Outcome,
    /// Per player: entity index the spotting pass resumes at.
    pub(crate) spot_cursor: Vec<u32>,
    /// Per player: AI commander state (`None` for human players).
    pub(crate) ai: Vec<Option<AiState>>,
}

/// Mutable world parts a match operation needs.
pub(crate) struct Ctx<'a> {
    pub ents: &'a mut Entities,
    pub terrain: &'a Heightfield,
    pub hash: &'a mut HashCache,
    pub tick: u64,
}

impl MatchState {
    /// Fresh state for a validated setup (no squads yet).
    pub(crate) fn new(setup: MatchSetup) -> MatchState {
        let r = &setup.rules;
        let players = setup
            .players
            .iter()
            .map(|_| PlayerState {
                manpower: r.start_manpower,
                tickets: r.tickets,
                surrendered: false,
                call_ins: Vec::new(),
                kills: 0,
                losses: 0,
                defeat: None,
                point_ticks: 0,
                income_acc: 0,
                drain_acc: 0,
            })
            .collect();
        let np = setup.players.len();
        let ai = setup
            .players
            .iter()
            .enumerate()
            .map(|(p, ps)| {
                ps.is_ai.then(|| {
                    let replan = r.difficulty.get(ps.difficulty).replan_ticks;
                    // Stagger AI players across the re-plan period.
                    AiState::new(replan as u64 * p as u64 / np as u64)
                })
            })
            .collect();
        MatchState {
            players,
            squads: BTreeMap::new(),
            next_squad: 0,
            points: vec![CapturePoint::default(); setup.map.points.len()],
            path_queue: PathQueue {
                queues: vec![Vec::new(); setup.players.len()],
                cursor: 0,
                debt: 0,
            },
            outcome: Outcome::Running,
            spot_cursor: vec![0; setup.players.len()],
            ai,
            setup,
        }
    }

    // ------------------------------------------------------------ queries --

    #[inline]
    pub fn setup(&self) -> &MatchSetup {
        &self.setup
    }
    #[inline]
    pub fn players(&self) -> &[PlayerState] {
        &self.players
    }
    /// Squads in ascending id order.
    #[inline]
    pub fn squads(&self) -> &BTreeMap<SquadId, Squad> {
        &self.squads
    }
    #[inline]
    pub fn squad(&self, id: SquadId) -> Option<&Squad> {
        self.squads.get(&id)
    }
    #[inline]
    pub fn points(&self) -> &[CapturePoint] {
        &self.points
    }
    #[inline]
    pub fn path_queue(&self) -> &PathQueue {
        &self.path_queue
    }
    #[inline]
    pub fn outcome(&self) -> Outcome {
        self.outcome
    }
    /// Per player: where the spotting cursor resumes (an entity index).
    #[inline]
    pub fn spot_cursor(&self) -> &[u32] {
        &self.spot_cursor
    }
    /// AI commander state of `player` (`None` for a human player).
    #[inline]
    pub fn ai(&self, player: PlayerId) -> Option<&AiState> {
        self.ai.get(player.0 as usize).and_then(|a| a.as_ref())
    }

    /// Base centre of `player`.
    pub fn base_of(&self, player: PlayerId) -> FxVec2 {
        let base = self.setup.players[player.0 as usize].base as usize;
        self.setup.map.bases[base].center.into()
    }

    /// Soldiers `player` has alive plus soldiers on order (pop-cap count).
    pub fn population(&self, ents: &Entities, player: PlayerId) -> u32 {
        let alive = ents
            .soldiers()
            .filter(|id| ents.owner[id.index()] == player)
            .count() as u32;
        let queued: u32 = self.players[player.0 as usize]
            .call_ins
            .iter()
            .map(|c| self.setup.squad_types[c.squad_type as usize].men as u32)
            .sum();
        alive + queued
    }

    // ---------------------------------------------------------- commands --

    /// Applies one command. Returns whether it was accepted. Bench commands
    /// (`MoveSoldier`, `Spawn`, `Despawn`) are always rejected here.
    pub(crate) fn apply(&mut self, c: &Command, ctx: &mut Ctx) -> bool {
        let p = c.player;
        if p.0 as usize >= self.players.len()
            || self.players[p.0 as usize].defeat.is_some()
            || self.outcome != Outcome::Running
        {
            return false;
        }
        match &c.kind {
            CommandKind::MoveSoldier { .. }
            | CommandKind::Spawn { .. }
            | CommandKind::Despawn { .. } => false,
            &CommandKind::Move { squad, target } => {
                self.order_move(ctx, p, squad, Order::Move { target })
            }
            &CommandKind::AttackMove { squad, target } => {
                self.order_move(ctx, p, squad, Order::AttackMove { target })
            }
            &CommandKind::Retreat { squad } => self.order_move(ctx, p, squad, Order::Retreat),
            &CommandKind::Stop { squad } => self.order_stop(ctx, p, squad),
            CommandKind::Dig { squad, line } => self.order_dig(ctx, p, *squad, line),
            &CommandKind::Barrage {
                squad,
                target,
                rounds,
            } => self.order_barrage(ctx, p, squad, target, rounds),
            &CommandKind::CallIn { squad_type } => self.call_in(ctx, p, squad_type),
            CommandKind::Surrender => {
                let pl = &mut self.players[p.0 as usize];
                pl.surrendered = true;
                pl.defeat = Some(Defeat::Surrendered);
                self.update_outcome();
                true
            }
        }
    }

    /// A live squad owned by `player`.
    fn owned(&self, player: PlayerId, id: SquadId) -> bool {
        self.squads
            .get(&id)
            .is_some_and(|s| s.owner == player && !s.members.is_empty())
    }

    fn order_move(&mut self, ctx: &mut Ctx, p: PlayerId, id: SquadId, order: Order) -> bool {
        if !self.owned(p, id) {
            return false;
        }
        let goal = match &order {
            Order::Move { target } | Order::AttackMove { target } => ctx.terrain.clamp(*target),
            _ => self.base_of(p),
        };
        let order = match order {
            Order::Move { .. } => Order::Move { target: goal },
            Order::AttackMove { .. } => Order::AttackMove { target: goal },
            o => o,
        };
        self.squads.get_mut(&id).expect("owned").order = order;
        self.request_path(id, goal);
        true
    }

    fn order_stop(&mut self, ctx: &mut Ctx, p: PlayerId, id: SquadId) -> bool {
        if !self.owned(p, id) {
            return false;
        }
        let sq = self.squads.get_mut(&id).expect("owned");
        sq.order = Order::Idle;
        let facing = sq.nav.heading;
        hold_in_place(sq, ctx.ents, facing);
        true
    }

    fn order_dig(&mut self, ctx: &mut Ctx, p: PlayerId, id: SquadId, line: &[FxVec2]) -> bool {
        if !self.owned(p, id) {
            return false;
        }
        let ty = &self.setup.squad_types[self.squads[&id].type_idx as usize];
        if ty.dig_rate_pct == 0 || !(2..=MAX_DIG_POINTS).contains(&line.len()) {
            return false;
        }
        if !line.iter().all(|q| ctx.terrain.contains(*q)) {
            return false;
        }
        let len = squad::polyline_length(line);
        if len.is_zero() || len > Fx::from(self.setup.rules.dig.max_length) {
            return false;
        }
        let goal = squad::point_along(line, len.div_int(2));
        let base = self.base_of(p);
        let sq = self.squads.get_mut(&id).expect("owned");
        let job = DigJob::new(
            line,
            sq.members.len(),
            p,
            base,
            ctx.ents,
            ctx.terrain,
            &self.setup.rules.dig,
            self.setup.tick_hz,
        );
        sq.order = Order::Dig {
            line: line.to_vec(),
            job,
        };
        self.request_path(id, goal);
        true
    }

    fn order_barrage(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        id: SquadId,
        target: FxVec2,
        rounds: u16,
    ) -> bool {
        if !self.owned(p, id) || !ctx.terrain.contains(target) {
            return false;
        }
        let ty = &self.setup.squad_types[self.squads[&id].type_idx as usize];
        if ty.role != SquadRole::Mortar || !(1..=MAX_BARRAGE_ROUNDS).contains(&rounds) {
            return false;
        }
        let w = &self.setup.weapons[ty.weapon as usize];
        let (min, max): (Fx, Fx) = (w.range_min.into(), w.range_max.into());
        let sq = self.squads.get_mut(&id).expect("owned");
        let lead = ctx.ents.transform[sq.members[0].index()].pos.xy();
        let dist = (target - lead).length();
        if dist < min {
            return false;
        }
        sq.order = Order::Barrage { target, rounds };
        let facing = (target - lead).normalize_or_zero();
        if dist <= max {
            hold_in_place(sq, ctx.ents, facing);
            // Where the team stands is where it "walked" (see the retry
            // rule in `combat::fire_mortars`).
            sq.nav.goal = lead;
        } else {
            // Walk up to 90 % of the range first.
            let spot = ctx
                .terrain
                .clamp(target - facing * max.mul_int(9).div_int(10));
            self.request_path(id, spot);
        }
        true
    }

    fn call_in(&mut self, ctx: &mut Ctx, p: PlayerId, squad_type: u16) -> bool {
        let faction = &self.setup.factions[self.setup.players[p.0 as usize].faction as usize];
        if !faction.roster.contains(&squad_type) {
            return false;
        }
        let ty = &self.setup.squad_types[squad_type as usize];
        let (cost, men, delay) = (ty.cost, ty.men as u32, ty.call_in_ticks);
        let pop = self.population(ctx.ents, p);
        let pl = &mut self.players[p.0 as usize];
        if pl.manpower < cost
            || pop + men > self.setup.rules.pop_cap
            || pl.call_ins.len() >= MAX_CALL_IN_QUEUE
        {
            return false;
        }
        pl.manpower -= cost;
        pl.call_ins.push(CallIn {
            squad_type,
            ready_tick: ctx.tick + delay as u64,
        });
        true
    }

    /// Asks for a path to `goal`; the squad waits in place until it is served.
    pub(crate) fn request_path(&mut self, id: SquadId, goal: FxVec2) {
        let sq = self.squads.get_mut(&id).expect("live squad");
        sq.nav.phase = NavPhase::PathPending;
        sq.nav.goal = goal;
        sq.nav.path.clear();
        sq.nav.next = 0;
        sq.nav.partial = false;
        sq.nav.slots.clear();
        sq.nav.phase_ticks = 0;
        let q = &mut self.path_queue.queues[sq.owner.0 as usize];
        if !q.contains(&id) {
            q.push(id);
        }
    }

    // ------------------------------------------------------------ spawns --

    /// Spawns the factions' starting squads (tick 0).
    pub(crate) fn spawn_start_squads(&mut self, ctx: &mut Ctx) {
        for p in 0..self.players.len() {
            let f = self.setup.players[p].faction as usize;
            for k in 0..self.setup.factions[f].start_squads.len() {
                let t = self.setup.factions[f].start_squads[k];
                self.spawn_squad(ctx, PlayerId(p as u8), t);
            }
        }
    }

    /// Spawns call-ins whose time has come, players in order, each queue in
    /// purchase order.
    pub(crate) fn spawn_call_ins(&mut self, ctx: &mut Ctx) {
        for p in 0..self.players.len() {
            while let Some(c) = self.players[p].call_ins.first().copied() {
                if c.ready_tick > ctx.tick {
                    break;
                }
                self.players[p].call_ins.remove(0);
                self.spawn_squad(ctx, PlayerId(p as u8), c.squad_type);
            }
        }
    }

    /// Spawns a full squad of `squad_type` at `owner`'s base, in a line facing
    /// the map centre. Squads spread around the base by id so they do not
    /// land on top of each other.
    pub(crate) fn spawn_squad(
        &mut self,
        ctx: &mut Ctx,
        owner: PlayerId,
        squad_type: u16,
    ) -> SquadId {
        let id = SquadId(self.next_squad);
        self.next_squad += 1;
        let ty = &self.setup.squad_types[squad_type as usize];
        let base = &self.setup.map.bases[self.setup.players[owner.0 as usize].base as usize];
        let center: FxVec2 = base.center.into();
        // Six spots on a ring at half the base radius, then the centre.
        let k = id.0 % 7;
        let spot = if k == 6 {
            center
        } else {
            let (s, c) = Fx::from_ratio(k as i64 * 1047, 1000).sin_cos(); // k · 60°
            center + FxVec2::new(c, s) * Fx::from(base.radius).div_int(2)
        };
        let spot = ctx.terrain.clamp(spot);
        let mid = FxVec2::from_int(self.setup.map.size_m / 2, self.setup.map.size_m / 2);
        let mut facing = (mid - spot).normalize_or_zero();
        if facing == FxVec2::ZERO {
            facing = FxVec2::new(Fx::ONE, Fx::ZERO);
        }
        let slots: Vec<FxVec2> =
            squad::line_slots(spot, facing, ty.men as usize, ty.spacing.into())
                .into_iter()
                .map(|q| ctx.terrain.clamp(q))
                .collect();
        let yaw = Fx::atan2(facing.y, facing.x);
        let speed: Fx = ty.speed.into();
        let combat = combat::spawn_combat(ty.hp, &self.setup.weapons[ty.weapon as usize]);
        let mut members = Vec::with_capacity(slots.len());
        for &q in &slots {
            let sid = ctx.ents.spawn(EntityKind::Soldier, owner);
            let i = sid.index();
            ctx.ents.transform[i].pos = q.extend(ctx.terrain.height_at(q));
            ctx.ents.transform[i].yaw = yaw;
            ctx.ents.mover[i].waypoint = q;
            ctx.ents.mover[i].speed = speed;
            ctx.ents.squad[i] = Some(id);
            ctx.ents.combat[i] = combat;
            ctx.hash.mark_entity_chunk(sid.chunk());
            members.push(sid);
        }
        self.squads.insert(
            id,
            Squad {
                type_idx: squad_type,
                owner,
                members,
                order: Order::Idle,
                nav: Nav {
                    phase: NavPhase::Holding,
                    goal: spot,
                    heading: facing,
                    facing,
                    slots,
                    ..Nav::default()
                },
                focus: None,
            },
        );
        id
    }

    // ------------------------------------------------------------- paths --

    /// Serves queued path requests round-robin by player until this tick's
    /// expansion budget (minus last tick's overrun) is spent.
    pub(crate) fn serve_paths(
        &mut self,
        ents: &Entities,
        terrain: &Heightfield,
        scratch: &mut PathScratch,
    ) {
        let mv = &self.setup.rules.movement;
        let cost = PathCost {
            slope_cost_pct: mv.slope_cost_pct,
            max_slope_pct: mv.max_slope_pct,
            trench_cost_pct: mv.trench_cost_pct,
        };
        let max_exp = mv.path_max_expansions;
        let pq = &mut self.path_queue;
        let n = pq.queues.len();
        let mut avail = mv.path_budget_per_tick as i64 - pq.debt as i64;
        while avail > 0 {
            let Some(p) = (0..n)
                .map(|k| (pq.cursor as usize + k) % n)
                .find(|&p| !pq.queues[p].is_empty())
            else {
                break;
            };
            let id = pq.queues[p].remove(0);
            pq.cursor = ((p + 1) % n) as u8;
            let Some(sq) = self.squads.get_mut(&id) else {
                continue;
            };
            if sq.nav.phase != NavPhase::PathPending || sq.members.is_empty() {
                continue;
            }
            let from = ents.transform[sq.members[0].index()].pos.xy();
            let r = find_path(terrain, from, sq.nav.goal, &cost, max_exp, scratch);
            avail -= r.expansions as i64;
            let nav = &mut sq.nav;
            nav.path = r.points;
            nav.next = 0;
            nav.partial = !r.reached && !r.exhausted;
            nav.phase = NavPhase::Moving;
            nav.phase_ticks = 0;
        }
        pq.debt = if avail < 0 { (-avail) as u32 } else { 0 };
    }

    // -------------------------------------------------------- validation --

    /// Checks a loaded match state against the entity store (untrusted
    /// saves): references in range, squad membership consistent both ways.
    pub(crate) fn validate(&self, ents: &Entities) -> Result<(), &'static str> {
        let s = &self.setup;
        let np = s.players.len();
        if self.players.len() != np
            || self.path_queue.queues.len() != np
            || self.path_queue.cursor as usize >= np
            || self.spot_cursor.len() != np
            || self.ai.len() != np
        {
            return Err("match player count disagrees with the setup");
        }
        if self.points.len() != s.map.points.len() {
            return Err("capture point count disagrees with the map");
        }
        let in_players = |p: Option<PlayerId>| p.is_none_or(|p| (p.0 as usize) < np);
        if self
            .points
            .iter()
            .any(|c| !in_players(c.owner) || !in_players(c.capturer))
        {
            return Err("capture point owned by an unknown player");
        }
        let full = capture_full(s.rules.capture_ticks);
        if self.points.iter().any(|c| {
            c.progress > full
                || (c.capturer.is_none() != (c.progress == 0))
                || (c.capturer.is_some() && c.capturer == c.owner)
        }) {
            return Err("capture point progress is inconsistent");
        }
        let standing = self.players.iter().filter(|p| p.defeat.is_none()).count();
        let consistent = match self.outcome {
            Outcome::Running => standing >= 2,
            Outcome::Won { winner } => {
                (winner.0 as usize) < np
                    && standing <= 1
                    && self.players[winner.0 as usize].defeat.is_none()
            }
            Outcome::Draw => standing == 0,
        };
        if !consistent {
            return Err("match outcome disagrees with the players");
        }
        let per_min = 60 * s.tick_hz;
        for (p, pl) in self.players.iter().enumerate() {
            if pl.surrendered != (pl.defeat == Some(Defeat::Surrendered))
                || pl.income_acc >= 100 * per_min
                || pl.drain_acc >= per_min
            {
                return Err("bad player state");
            }
            if self.ai[p].is_some() != s.players[p].is_ai {
                return Err("AI state disagrees with the setup");
            }
            if let Some(ai) = &self.ai[p] {
                ai.validate(self)?;
            }
        }
        for pl in &self.players {
            if pl.call_ins.len() > MAX_CALL_IN_QUEUE
                || pl
                    .call_ins
                    .iter()
                    .any(|c| c.squad_type as usize >= s.squad_types.len())
            {
                return Err("bad call-in queue");
            }
        }
        let mut members = 0usize;
        for (&id, sq) in &self.squads {
            if id.0 >= self.next_squad
                || sq.type_idx as usize >= s.squad_types.len()
                || sq.owner.0 as usize >= np
                || sq.members.len() > 16
            {
                return Err("squad out of range");
            }
            for &m in &sq.members {
                if !ents.is_alive(m)
                    || ents.kind[m.index()] != EntityKind::Soldier
                    || ents.squad[m.index()] != Some(id)
                    || ents.owner[m.index()] != sq.owner
                {
                    return Err("squad member is not a soldier of that squad");
                }
            }
            members += sq.members.len();
            if sq.members.is_empty() || sq.focus.is_some_and(|f| f.index() >= ents.len()) {
                return Err("squad out of range");
            }
            let nav = &sq.nav;
            if nav.path.len() > MAX_PATH_POINTS
                || nav.next as usize > nav.path.len()
                || (nav.phase == NavPhase::Moving && nav.next as usize >= nav.path.len())
                || !(nav.slots.is_empty() || nav.slots.len() == sq.members.len())
                || (matches!(nav.phase, NavPhase::Holding | NavPhase::Arriving)
                    && nav.slots.len() != sq.members.len())
            {
                return Err("squad navigation state is inconsistent");
            }
            if let Order::Dig { line, job } = &sq.order {
                let len = squad::polyline_length(line);
                let size = Fx::from_int(s.map.size_m);
                let on_map =
                    |q: &FxVec2| q.x >= Fx::ZERO && q.y >= Fx::ZERO && q.x <= size && q.y <= size;
                if !(2..=MAX_DIG_POINTS).contains(&line.len())
                    || !line.iter().all(on_map)
                    || len > Fx::from(s.rules.dig.max_length)
                    || !(1..=16).contains(&job.segments.len())
                    || job.front.abs() != 1
                    || job.segments.iter().any(|g| {
                        g.from < Fx::ZERO
                            || g.from > g.to
                            || g.to > len
                            || g.passes as u32 > s.rules.dig.passes
                            || g.work > DigJob::required(g, &s.rules.dig, s.tick_hz)
                    })
                {
                    return Err("dig order is inconsistent");
                }
            }
        }
        if self.spot_cursor.iter().any(|&c| c as usize > ents.len()) {
            return Err("spotting cursor out of range");
        }
        for (i, c) in ents.combat.iter().enumerate() {
            let kind = ents.kind[i];
            let bad = c.seen_by >> np != 0
                || c.target.is_some_and(|t| t.index() >= ents.len())
                || c.suppression > 100_000
                || (kind == EntityKind::Soldier && (c.hp == 0 || c.corpse != 0))
                || (kind == EntityKind::Corpse && (c.corpse == 0 || ents.squad[i].is_some()));
            if bad {
                return Err("soldier combat state is inconsistent");
            }
        }
        let linked = ents.squad.iter().filter(|s| s.is_some()).count();
        if linked != members {
            return Err("soldier squad links disagree with squad member lists");
        }
        let mut queued: Vec<SquadId> = self.path_queue.queues.iter().flatten().copied().collect();
        let n = queued.len();
        queued.sort_unstable();
        queued.dedup();
        if queued.len() != n {
            return Err("a squad is queued for a path twice");
        }
        Ok(())
    }
}

/// Stops `sq` where its men stand, facing `facing` (kept if zero).
fn hold_in_place(sq: &mut Squad, ents: &Entities, facing: FxVec2) {
    let nav = &mut sq.nav;
    nav.phase = NavPhase::Holding;
    nav.path.clear();
    nav.next = 0;
    nav.partial = false;
    nav.phase_ticks = 0;
    if facing != FxVec2::ZERO {
        nav.facing = facing;
    }
    nav.slots = sq
        .members
        .iter()
        .map(|m| ents.transform[m.index()].pos.xy())
        .collect();
}
