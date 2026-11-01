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
//!
//! Soldier-level orders (S1b) are in `crate::unit`: detached men keep their
//! own order in the `unit` entity column, path through the same queues
//! ([`PathReq::Unit`]) and dig trenches as [`UnitDig`] jobs kept here.
//! Thrown items (S1b) are in `crate::items`: per-soldier kits in the `kit`
//! entity column, items in flight and smoke clouds kept here.
//!
//! Vehicles (S3) are in `crate::vehicle`: a vehicle is a squad of one
//! whose only member is the vehicle entity, with its [`VehicleState`] kept
//! here. Squad orders `Move`, `AttackMove`, `Stop` and `Retreat` (and the
//! `*Units` versions naming the vehicle id) work on it; `Dig`, `Barrage`,
//! `Regroup` and stance orders are rejected. Its path is a
//! [`PathReq::Vehicle`] on the vehicle costs; it drives in the Movement
//! stage, not in squad steering. Vehicle call-ins wait for the rules'
//! unlock time and count against the per-player vehicle limit (alive or
//! queued), and every squad type counts its own pop (men, or its vehicle's
//! `pop`).

use std::collections::BTreeMap;

use frontline_content::{DigProfile, MatchSetup, SquadRole, WeaponClass};
use serde::{Deserialize, Serialize};

use crate::ai::AiState;
use crate::armour::ArmourTables;
use crate::combat;
use crate::command::{Command, CommandKind};
use crate::dig::DigJob;
use crate::ecs::{Entities, EntityKind, PlayerId, SimId};
use crate::fx::{Fx, FxVec2};
use crate::hash::HashCache;
use crate::items::{Kit, SmokeCloud, Thrown};
use crate::path::{
    PathCost, PathScratch, VehiclePathCost, find_infantry_path, find_path, find_vehicle_path,
};
use crate::rules::{Defeat, capture_full};
use crate::squad;
use crate::terrain::Heightfield;
use crate::unit::{UnitDig, UnitOrder};
use crate::vehicle::{self, VehicleDerived, VehicleState};

/// Most points in a Dig polyline.
pub const MAX_DIG_POINTS: usize = 16;
/// Most rounds in one Barrage order.
pub const MAX_BARRAGE_ROUNDS: u16 = 50;
/// Most queued call-ins per player (the HUD's "n/5" queue bar,
/// `docs/slices/S1b-hud.md`).
pub const MAX_CALL_IN_QUEUE: usize = 5;
/// Longest stored path (waypoints); a smoothed path is far shorter.
pub const MAX_PATH_POINTS: usize = 8192;
/// Most AT-ditch lines in a match (S3; each job adds one).
pub const MAX_AT_DITCHES: usize = 1024;

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
    /// Escort friendly vehicle `vehicle` (S3 `FollowUnits`): the men walk
    /// in single file behind it (`crate::unit` escort steering). Becomes
    /// `Idle` when the vehicle is gone.
    Follow {
        vehicle: SimId,
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
    /// S3: escorting a vehicle.
    Follow = 6,
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
            Order::Follow { .. } => OrderKind::Follow,
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
    /// AT teams (S3): the team's launcher; `None` for other squads.
    pub launcher: Option<Launcher>,
}

/// An AT team's launcher (S3, `crate::combat` "Launchers"): who carries
/// it, the hand-over after its gunner falls, the rockets left and the
/// `AttackUnit` vehicle.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Launcher {
    /// The man carrying it; `None` while it waits to be picked up.
    pub carrier: Option<SimId>,
    /// Ticks until the next man takes it up (after the carrier fell).
    pub handover: u16,
    /// Rockets left.
    pub rounds: u16,
    /// Ticks in a row in the own base toward the next rocket.
    pub resupply: u16,
    /// `AttackUnit`: the enemy vehicle the gunner focuses while it is seen.
    pub attack: Option<SimId>,
    /// Against the current target: aim at its turret, not its hull.
    pub aim_turret: bool,
}

impl Squad {
    /// Formation leader: the first member who is not detached (S1b), else
    /// `members[0]`. The squad must have members.
    pub fn lead(&self, ents: &Entities) -> SimId {
        self.members
            .iter()
            .copied()
            .find(|m| !ents.unit[m.index()].detached)
            .unwrap_or(self.members[0])
    }
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
    /// S3 report counters: enemy soldiers and crewmen killed, by
    /// [`kill_class`] (they add up to `kills`), and AT rockets fired.
    pub kills_by: [u32; kill_class::COUNT],
    pub rockets: u32,
}

/// What killed a soldier or crewman (`PlayerState::kills_by`, S3 report).
pub mod kill_class {
    pub const RIFLE: u8 = 0;
    pub const SMG: u8 = 1;
    pub const MG: u8 = 2;
    pub const MORTAR: u8 = 3;
    /// A main-gun penetrator (AP, APHE).
    pub const TANK_AP: u8 = 4;
    /// A main-gun HE round.
    pub const TANK_HE: u8 = 5;
    /// A coax or hull MG.
    pub const VEHICLE_MG: u8 = 6;
    /// An AT team's rocket.
    pub const ROCKET: u8 = 7;
    pub const SATCHEL: u8 = 8;
    pub const FRAG: u8 = 9;
    /// Crew lost with a vehicle burnt out or knocked out by a setup tool.
    pub const OTHER: u8 = 10;
    pub const COUNT: usize = 11;
    /// Report names, by class.
    pub const NAMES: [&str; COUNT] = [
        "rifle",
        "smg",
        "mg",
        "mortar",
        "tank_ap",
        "tank_he",
        "vehicle_mg",
        "rocket",
        "satchel",
        "frag",
        "other",
    ];
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

/// Who a queued path is for.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum PathReq {
    /// The squad's formation leader walks it.
    Squad(SquadId),
    /// A detached soldier walks it to his own spot (`crate::unit`).
    Unit(SimId),
    /// A vehicle squad's vehicle drives it (S3, `crate::vehicle`), on the
    /// vehicle costs; `avoid` is a grid node the search makes 8× dearer
    /// (the stuck re-path).
    Vehicle { squad: SquadId, avoid: Option<u32> },
}

/// Path requests: one FIFO per player served round-robin, and the expansion
/// debt carried over from a search that overran the tick budget.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct PathQueue {
    pub queues: Vec<Vec<PathReq>>,
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
    /// Trenches dug by detached men (`DigUnits`), by never-reused job id.
    pub(crate) unit_digs: BTreeMap<u32, UnitDig>,
    pub(crate) next_unit_dig: u32,
    /// Items in flight or waiting for their fuse, ascending id.
    pub(crate) thrown: Vec<Thrown>,
    pub(crate) next_thrown: u32,
    /// Smoke clouds, ascending id.
    pub(crate) smoke: Vec<SmokeCloud>,
    pub(crate) next_smoke: u32,
    /// Vehicles and wrecks (S3), by entity id (`crate::vehicle`).
    pub(crate) vehicles: BTreeMap<SimId, VehicleState>,
    /// AT-ditch lines (S3): every AT-ditch job's line, added when it
    /// starts. The vehicle block layer reads them.
    pub(crate) at_ditches: Vec<Vec<FxVec2>>,
    /// Armour events so far (S3): the next `ArmourEvent::seq`.
    pub(crate) armour_seq: u32,
    /// Derived from the setup (S3): plate normals and ammo cosines. Not
    /// saved and not hashed (the setup is); rebuilt by
    /// [`MatchState::rebuild_derived`] after load.
    #[serde(skip)]
    pub(crate) armour: ArmourTables,
    /// Derived vehicle index, frames and block layer (S3); rebuilt by
    /// `World` on load and new match (`rebuild_vehicle_derived`).
    #[serde(skip)]
    pub(crate) vderived: VehicleDerived,
    /// Diagnostic hit log (`World::enable_hit_log`, armour-test): not
    /// saved, not hashed, never read by the sim.
    #[serde(skip)]
    pub(crate) hit_log: Option<Vec<crate::damage::HitRecord>>,
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
                kills_by: [0; kill_class::COUNT],
                rockets: 0,
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
            unit_digs: BTreeMap::new(),
            next_unit_dig: 0,
            thrown: Vec::new(),
            next_thrown: 0,
            smoke: Vec::new(),
            next_smoke: 0,
            vehicles: BTreeMap::new(),
            at_ditches: Vec::new(),
            armour_seq: 0,
            armour: ArmourTables::new(&setup.ammo, &setup.vehicles),
            vderived: VehicleDerived::new(&setup, grid_dim(&setup)),
            hit_log: None,
            setup,
        }
    }

    /// Rebuilds the derived caches from the setup (after load). The vehicle
    /// structures that also need the entities and terrain are rebuilt by
    /// `rebuild_vehicle_derived`.
    pub(crate) fn rebuild_derived(&mut self) {
        self.armour = ArmourTables::new(&self.setup.ammo, &self.setup.vehicles);
        self.vderived = VehicleDerived::new(&self.setup, grid_dim(&self.setup));
    }

    // ------------------------------------------------------------ queries --

    #[inline]
    pub fn setup(&self) -> &MatchSetup {
        &self.setup
    }
    /// Armour tables derived from the setup (S3).
    #[inline]
    pub fn armour(&self) -> &ArmourTables {
        &self.armour
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
    /// Trenches dug by detached men, by job id (`UnitOrder::Dig`).
    #[inline]
    pub fn unit_digs(&self) -> &BTreeMap<u32, UnitDig> {
        &self.unit_digs
    }
    /// Vehicles and wrecks by entity id (S3).
    #[inline]
    pub fn vehicles(&self) -> &BTreeMap<SimId, VehicleState> {
        &self.vehicles
    }
    #[inline]
    pub fn vehicle(&self, id: SimId) -> Option<&VehicleState> {
        self.vehicles.get(&id)
    }
    /// AT-ditch lines (S3).
    #[inline]
    pub fn at_ditches(&self) -> &[Vec<FxVec2>] {
        &self.at_ditches
    }
    /// Derived vehicle data (S3): index, frames, block layer. Valid after
    /// the Movement stage of the last tick.
    #[inline]
    pub fn vehicle_derived(&self) -> &VehicleDerived {
        &self.vderived
    }

    /// Base centre of `player`.
    pub fn base_of(&self, player: PlayerId) -> FxVec2 {
        let base = self.setup.players[player.0 as usize].base as usize;
        self.setup.map.bases[base].center.into()
    }

    /// Pop-cap count of `player`: soldiers alive, each live vehicle's
    /// `pop`, and every queued call-in's squad-type pop (its men, or its
    /// vehicle's pop, S3).
    pub fn population(&self, ents: &Entities, player: PlayerId) -> u32 {
        let alive = ents
            .soldiers()
            .filter(|id| ents.owner[id.index()] == player)
            .count() as u32;
        let vehicles: u32 = self
            .vehicles
            .iter()
            .filter(|(id, _)| {
                ents.kind[id.index()] == EntityKind::Vehicle && ents.owner[id.index()] == player
            })
            .map(|(_, vs)| self.setup.vehicles[vs.type_idx as usize].pop)
            .sum();
        let queued: u32 = self.players[player.0 as usize]
            .call_ins
            .iter()
            .map(|c| self.setup.squad_types[c.squad_type as usize].pop(&self.setup.vehicles))
            .sum();
        alive + vehicles + queued
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
            CommandKind::Dig {
                squad,
                line,
                profile,
            } => self.order_dig(ctx, p, *squad, line, *profile),
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
            CommandKind::MoveUnits { units, target } => {
                self.units_move(ctx, p, units, UnitOrder::Move { target: *target })
            }
            CommandKind::AttackMoveUnits { units, target } => {
                self.units_move(ctx, p, units, UnitOrder::AttackMove { target: *target })
            }
            CommandKind::RetreatUnits { units } => {
                self.units_move(ctx, p, units, UnitOrder::Retreat)
            }
            CommandKind::StopUnits { units } => self.units_stop(ctx, p, units),
            CommandKind::DigUnits {
                units,
                line,
                profile,
            } => self.units_dig(ctx, p, units, line, *profile),
            CommandKind::SetStance { units, stance } => self.set_stance(ctx, p, units, *stance),
            &CommandKind::Regroup { squad } => {
                !self.is_vehicle_squad(squad) && self.regroup(ctx, p, squad)
            }
            CommandKind::Throw {
                units,
                item,
                target,
            } => self.throw(ctx, p, units, *item, *target, false),
            CommandKind::ThrowEach {
                units,
                item,
                target,
            } => self.throw(ctx, p, units, *item, *target, true),
            CommandKind::ReverseUnits { units, target } => {
                self.units_reverse(ctx, p, units, ctx.terrain.clamp(*target))
            }
            CommandKind::FaceUnits { units, toward } => self.units_face(ctx, p, units, *toward),
            CommandKind::AttackUnit { units, target } => self.units_attack(ctx, p, units, *target),
            CommandKind::SetAmmo { units, ammo } => self.units_set_ammo(ctx, p, units, *ammo),
            CommandKind::RepairUnits { units } => self.units_repair(ctx, p, units),
            CommandKind::FollowUnits { units, vehicle } => {
                self.units_follow(ctx, p, units, *vehicle)
            }
        }
    }

    /// Whether `vehicle` is a live vehicle of `player` (an escort's).
    pub(crate) fn escorts(&self, ents: &Entities, player: PlayerId, vehicle: SimId) -> bool {
        ents.kind.get(vehicle.index()) == Some(&EntityKind::Vehicle)
            && ents.owner[vehicle.index()] == player
    }

    /// Whether a squad type of `role` digging at `rate` may dig `profile`
    /// (S3: the AT ditch is the engineers').
    pub(crate) fn digs_profile(&self, role: SquadRole, rate: u32, profile: DigProfile) -> bool {
        rate > 0
            && (!self.setup.rules.dig_profile(profile).engineers_only
                || role == SquadRole::Engineer)
    }

    /// Whether a job digging `profile` may start: an AT-ditch job needs
    /// room in the AT-ditch list for its sections (S3).
    pub(crate) fn at_ditch_room(&self, profile: DigProfile) -> bool {
        profile != DigProfile::AtDitch || self.at_ditches.len() < MAX_AT_DITCHES
    }

    /// Records an AT-ditch section once its first pass is stamped
    /// (`crate::dig`), so only ground a ditch job has really dug blocks
    /// vehicles and loses the trench discount; an order cancelled before
    /// any digging leaves the map as it was. A section already recorded is
    /// not added twice; when the list is full the section is dug but not
    /// recorded.
    pub(crate) fn record_at_ditch(&mut self, line: Vec<FxVec2>) {
        if self.at_ditches.len() >= MAX_AT_DITCHES || self.at_ditches.contains(&line) {
            return;
        }
        self.mark_ditch_block(&line);
        self.at_ditches.push(line);
    }

    /// A live squad owned by `player`.
    pub(crate) fn owned(&self, player: PlayerId, id: SquadId) -> bool {
        self.squads
            .get(&id)
            .is_some_and(|s| s.owner == player && !s.members.is_empty())
    }

    /// Squad orders clear `detached` for the whole squad (`crate::unit`);
    /// walking orders regroup the men through the formation.
    pub(crate) fn order_move(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        id: SquadId,
        order: Order,
    ) -> bool {
        if !self.owned(p, id) {
            return false;
        }
        self.clear_detached(ctx, id);
        let goal = match &order {
            Order::Move { target } | Order::AttackMove { target } => ctx.terrain.clamp(*target),
            _ => self.base_of(p),
        };
        if self.is_vehicle_squad(id) {
            // A short Move to a point behind the hull backs up to it.
            let v = self.squads[&id].members[0];
            let back = matches!(order, Order::Move { .. })
                && vehicle::reverses_to(
                    &ctx.ents.transform[v.index()],
                    goal,
                    self.setup.rules.vehicles.driving.reverse_order_m,
                );
            if let Some(vs) = self.vehicles.get_mut(&v) {
                vs.reverse = back;
                vs.clear_orders();
            }
        }
        let order = match order {
            Order::Move { .. } => Order::Move { target: goal },
            Order::AttackMove { .. } => Order::AttackMove { target: goal },
            o => o,
        };
        self.squads.get_mut(&id).expect("owned").order = order;
        self.request_path(id, goal);
        true
    }

    /// Detached men make the squad form a line around its leader instead.
    pub(crate) fn order_stop(&mut self, ctx: &mut Ctx, p: PlayerId, id: SquadId) -> bool {
        if !self.owned(p, id) {
            return false;
        }
        if self.is_vehicle_squad(id) {
            let sq = self.squads.get_mut(&id).expect("owned");
            sq.order = Order::Idle;
            let v = sq.members[0];
            vehicle::hold_vehicle(&mut sq.nav, ctx.ents.transform[v.index()].pos.xy());
            if let Some(vs) = self.vehicles.get_mut(&v) {
                vs.reverse = false;
                vs.clear_orders();
            }
            return true;
        }
        let regroup = self.clear_detached(ctx, id);
        let sq = self.squads.get_mut(&id).expect("owned");
        sq.order = Order::Idle;
        let facing = sq.nav.heading;
        if regroup {
            self.regroup_here(ctx, id, facing);
        } else {
            hold_in_place(sq, ctx.ents, facing);
        }
        true
    }

    /// `Dig`: squads that can dig; the AT-ditch profile engineers only (S3).
    pub(crate) fn order_dig(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        id: SquadId,
        line: &[FxVec2],
        profile: DigProfile,
    ) -> bool {
        if !self.owned(p, id) {
            return false;
        }
        let ty = &self.setup.squad_types[self.squads[&id].type_idx as usize];
        if !self.digs_profile(ty.role, ty.dig_rate_pct, profile)
            || !self.dig_line_ok(ctx, line, profile)
            || !self.at_ditch_room(profile)
        {
            return false;
        }
        self.clear_detached(ctx, id);
        let len = squad::polyline_length(line);
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
            &self.setup.rules,
            profile,
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
        let lead = self.squads[&id].lead(ctx.ents);
        let lead = ctx.ents.transform[lead.index()].pos.xy();
        let dist = (target - lead).length();
        if dist < min {
            return false;
        }
        let regroup = self.clear_detached(ctx, id);
        let sq = self.squads.get_mut(&id).expect("owned");
        sq.order = Order::Barrage { target, rounds };
        let facing = (target - lead).normalize_or_zero();
        if dist <= max {
            if regroup {
                self.regroup_here(ctx, id, facing);
            } else {
                hold_in_place(sq, ctx.ents, facing);
            }
            let sq = self.squads.get_mut(&id).expect("owned");
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
        let (cost, men, delay) = (ty.cost, ty.pop(&self.setup.vehicles), ty.call_in_ticks);
        // Vehicles (S3): only after the unlock time and under the limit
        // (alive or queued).
        let vr = &self.setup.rules.vehicles;
        if ty.vehicle.is_some()
            && (ctx.tick < vr.unlock_ticks as u64 || self.vehicles_counted(ctx.ents, p) >= vr.limit)
        {
            return false;
        }
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
    /// Vehicle squads queue a vehicle request.
    pub(crate) fn request_path(&mut self, id: SquadId, goal: FxVec2) {
        if self.is_vehicle_squad(id) {
            let sq = self.squads.get_mut(&id).expect("live squad");
            vehicle::request_vehicle_path(sq, id, &mut self.path_queue, goal, None);
            return;
        }
        let sq = self.squads.get_mut(&id).expect("live squad");
        sq.nav.phase = NavPhase::PathPending;
        sq.nav.goal = goal;
        sq.nav.path.clear();
        sq.nav.next = 0;
        sq.nav.partial = false;
        sq.nav.slots.clear();
        sq.nav.phase_ticks = 0;
        let q = &mut self.path_queue.queues[sq.owner.0 as usize];
        if !q.contains(&PathReq::Squad(id)) {
            q.push(PathReq::Squad(id));
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
        let vehicle_type = ty.vehicle;
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
        if let Some(vt) = vehicle_type {
            self.spawn_vehicle(ctx, owner, id, squad_type, vt, spot, facing);
            return id;
        }
        let slots: Vec<FxVec2> =
            squad::line_slots(spot, facing, ty.men as usize, ty.spacing.into())
                .into_iter()
                .map(|q| ctx.terrain.clamp(q))
                .collect();
        let yaw = Fx::atan2(facing.y, facing.x);
        let speed: Fx = ty.speed.into();
        let mut members = Vec::with_capacity(slots.len());
        for (k, &q) in slots.iter().enumerate() {
            let sid = ctx.ents.spawn(EntityKind::Soldier, owner);
            let i = sid.index();
            ctx.ents.transform[i].pos = q.extend(ctx.terrain.height_at(q));
            ctx.ents.transform[i].yaw = yaw;
            ctx.ents.mover[i].waypoint = q;
            ctx.ents.mover[i].speed = speed;
            ctx.ents.squad[i] = Some(id);
            // The first `weapon_men` men carry the squad weapon, the rest
            // the sidearm (S3).
            let wk = match ty.sidearm {
                Some(side) if k >= ty.weapon_men as usize => side,
                _ => ty.weapon,
            };
            ctx.ents.combat[i] = combat::spawn_combat(
                ty.hp,
                &self.setup.weapons[wk as usize],
                &self.setup.rules.combat.body,
            );
            // The first man leads the squad and carries the leader's kit.
            ctx.ents.kit[i] = Kit {
                items: ty.full_kit(&self.setup.items, k == 0),
                leader: k == 0,
                ..Kit::default()
            };
            ctx.hash.mark_entity_chunk(sid.chunk());
            members.push(sid);
        }
        // AT teams: the first man carries the launcher (S3).
        let w = &self.setup.weapons[ty.weapon as usize];
        let launcher = (w.class == WeaponClass::Launcher).then(|| Launcher {
            carrier: members.first().copied(),
            rounds: w.rounds_carried.min(u16::MAX as u32) as u16,
            ..Launcher::default()
        });
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
                launcher,
            },
        );
        id
    }

    /// Spawns vehicle squad `id` (a squad of one) at `spot` facing
    /// `facing`, settled on the ground.
    #[allow(clippy::too_many_arguments)]
    fn spawn_vehicle(
        &mut self,
        ctx: &mut Ctx,
        owner: PlayerId,
        id: SquadId,
        squad_type: u16,
        vt: u16,
        spot: FxVec2,
        facing: FxVec2,
    ) {
        let v = &self.setup.vehicles[vt as usize];
        let geo = self.vderived.geo[vt as usize];
        let vid = ctx.ents.spawn(EntityKind::Vehicle, owner);
        let i = vid.index();
        ctx.ents.transform[i].pos = spot.extend(ctx.terrain.height_at(spot));
        ctx.ents.transform[i].yaw = Fx::atan2(facing.y, facing.x);
        ctx.ents.squad[i] = Some(id);
        let mut vs = VehicleState::new(vt, v, &self.setup.rules.vehicles);
        vehicle::settle(
            ctx.terrain,
            &self.vderived.block,
            &geo,
            &mut vs,
            &mut ctx.ents.transform[i],
        );
        vs.progress_at = spot;
        self.vehicles.insert(vid, vs);
        ctx.hash.mark_entity_chunk(vid.chunk());
        self.squads.insert(
            id,
            Squad {
                type_idx: squad_type,
                owner,
                members: vec![vid],
                order: Order::Idle,
                nav: Nav {
                    phase: NavPhase::Holding,
                    goal: spot,
                    heading: facing,
                    facing,
                    slots: vec![spot],
                    ..Nav::default()
                },
                focus: None,
                launcher: None,
            },
        );
    }

    // ------------------------------------------------------------- paths --

    /// Serves queued path requests round-robin by player until this tick's
    /// expansion budget (minus last tick's overrun) is spent. Returns the
    /// node expansions used (diagnostics).
    pub(crate) fn serve_paths(&mut self, ctx: &mut Ctx, scratch: &mut PathScratch) -> u32 {
        let terrain = ctx.terrain;
        let mv = &self.setup.rules.movement;
        let cost = PathCost {
            slope_cost_pct: mv.slope_cost_pct,
            max_slope_pct: mv.max_slope_pct,
            trench_cost_pct: mv.trench_cost_pct,
            ditch_cost_pct: mv.ditch_cost_pct,
        };
        let max_exp = mv.path_max_expansions;
        // Infantry: AT-ditch costs once a match has ditches (S3).
        let ditches = (!self.at_ditches.is_empty()).then_some(&self.vderived.block);
        let foot_path = |from: FxVec2, to: FxVec2, scratch: &mut PathScratch| match ditches {
            Some(d) => find_infantry_path(terrain, from, to, &cost, d, max_exp, scratch),
            None => find_path(terrain, from, to, &cost, max_exp, scratch),
        };
        let pq = &mut self.path_queue;
        let n = pq.queues.len();
        let mut avail = mv.path_budget_per_tick as i64 - pq.debt as i64;
        let start = avail;
        while avail > 0 {
            let Some(p) = (0..n)
                .map(|k| (pq.cursor as usize + k) % n)
                .find(|&p| !pq.queues[p].is_empty())
            else {
                break;
            };
            let req = pq.queues[p].remove(0);
            pq.cursor = ((p + 1) % n) as u8;
            match req {
                PathReq::Squad(id) => {
                    let Some(sq) = self.squads.get_mut(&id) else {
                        continue;
                    };
                    if sq.nav.phase != NavPhase::PathPending || sq.members.is_empty() {
                        continue;
                    }
                    let from = ctx.ents.transform[sq.lead(ctx.ents).index()].pos.xy();
                    let r = foot_path(from, sq.nav.goal, scratch);
                    avail -= r.expansions as i64;
                    let nav = &mut sq.nav;
                    nav.path = r.points;
                    nav.next = 0;
                    nav.partial = !r.reached && !r.exhausted;
                    nav.phase = NavPhase::Moving;
                    nav.phase_ticks = 0;
                }
                PathReq::Unit(m) => {
                    let i = m.index();
                    if ctx.ents.kind.get(i) != Some(&EntityKind::Soldier)
                        || !ctx.ents.unit[i].detached
                        || ctx.ents.unit[i].nav.phase != NavPhase::PathPending
                    {
                        continue;
                    }
                    let from = ctx.ents.transform[i].pos.xy();
                    let goal = ctx.ents.unit[i].nav.goal;
                    let r = foot_path(from, goal, scratch);
                    avail -= r.expansions as i64;
                    let nav = &mut ctx.ents.unit[i].nav;
                    nav.path = r.points;
                    nav.next = 0;
                    nav.partial = !r.reached && !r.exhausted;
                    nav.phase = NavPhase::Moving;
                    nav.phase_ticks = 0;
                    ctx.hash.mark_entity_chunk(m.chunk());
                }
                PathReq::Vehicle { squad, avoid } => {
                    let Some(sq) = self.squads.get_mut(&squad) else {
                        continue;
                    };
                    let v = sq.members[0];
                    let Some(vs) = self.vehicles.get(&v) else {
                        continue;
                    };
                    if sq.nav.phase != NavPhase::PathPending
                        || ctx.ents.kind[v.index()] != EntityKind::Vehicle
                    {
                        continue;
                    }
                    let vcost = PathCost {
                        max_slope_pct: self.setup.vehicles[vs.type_idx as usize]
                            .mobility
                            .max_slope_pct,
                        trench_cost_pct: 100,
                        ..cost
                    };
                    let veh = VehiclePathCost {
                        block: &self.vderived.block,
                        avoid,
                        costs: &self.setup.rules.vehicles.path,
                    };
                    let from = ctx.ents.transform[v.index()].pos.xy();
                    let r = find_vehicle_path(
                        terrain,
                        from,
                        sq.nav.goal,
                        &vcost,
                        &veh,
                        max_exp,
                        scratch,
                    );
                    avail -= r.expansions as i64;
                    let nav = &mut sq.nav;
                    nav.path = r.points;
                    nav.next = 0;
                    nav.partial = !r.reached && !r.exhausted;
                    nav.phase = NavPhase::Moving;
                    nav.phase_ticks = 0;
                }
            }
        }
        pq.debt = if avail < 0 { (-avail) as u32 } else { 0 };
        (start - avail).clamp(0, u32::MAX as i64) as u32
    }

    // -------------------------------------------------------- validation --

    /// Checks a loaded match state against the entity store (untrusted
    /// saves) at `tick`: references in range, squad membership consistent
    /// both ways.
    pub(crate) fn validate(&self, ents: &Entities, tick: u64) -> Result<(), &'static str> {
        let s = &self.setup;
        let np = s.players.len();
        // Id counters are bumped unchecked; one at the top would overflow.
        if [
            self.next_squad,
            self.next_unit_dig,
            self.next_thrown,
            self.next_smoke,
        ]
        .contains(&u32::MAX)
        {
            return Err("an id counter is exhausted");
        }
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
                ai.validate(self, PlayerId(p as u8), tick)?;
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
            if let Some(vt) = s.squad_types[sq.type_idx as usize].vehicle {
                // A vehicle squad: exactly its vehicle, of the squad type's
                // vehicle type, holding or on a path like any squad.
                let ok = sq.members.len() == 1 && {
                    let v = sq.members[0];
                    ents.kind.get(v.index()) == Some(&EntityKind::Vehicle)
                        && ents.squad[v.index()] == Some(id)
                        && ents.owner[v.index()] == sq.owner
                        && self.vehicles.get(&v).is_some_and(|vs| vs.type_idx == vt)
                };
                if !ok
                    || sq.focus.is_some()
                    || matches!(sq.order, Order::Dig { .. } | Order::Barrage { .. })
                {
                    return Err("vehicle squad is inconsistent");
                }
            } else if sq
                .members
                .iter()
                .any(|m| ents.kind.get(m.index()) == Some(&EntityKind::Vehicle))
            {
                return Err("a soldier squad lists a vehicle");
            }
            for &m in &sq.members {
                if ents.kind.get(m.index()) == Some(&EntityKind::Vehicle) {
                    continue;
                }
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
            if let Order::Dig { line, job } = &sq.order
                && !dig_consistent(s, line, job, 16, ents)
            {
                return Err("dig order is inconsistent");
            }
            if let Order::Follow { vehicle } = sq.order
                && !self.escorts(ents, sq.owner, vehicle)
            {
                return Err("follow order names no own vehicle");
            }
            let ty = &s.squad_types[sq.type_idx as usize];
            let w = &s.weapons[ty.weapon as usize];
            let launcher_ok = match &sq.launcher {
                None => w.class != WeaponClass::Launcher,
                Some(l) => {
                    w.class == WeaponClass::Launcher
                        && l.carrier.is_none_or(|c| sq.members.contains(&c))
                        && l.handover as u32 <= w.handover_ticks
                        && l.rounds as u32 <= w.rounds_carried
                        && (l.resupply as u32) < s.rules.items.resupply_ticks.max(1)
                        && l.attack.is_none_or(|t| {
                            ents.kind.get(t.index()) == Some(&EntityKind::Vehicle)
                                && ents.owner[t.index()] != sq.owner
                        })
                }
            };
            if !launcher_ok {
                return Err("launcher state is inconsistent");
            }
        }
        for (&id, d) in &self.unit_digs {
            if id >= self.next_unit_dig
                || d.owner.0 as usize >= np
                || !dig_consistent(s, &d.line, &d.job, crate::unit::MAX_UNITS, ents)
            {
                return Err("unit dig job is inconsistent");
            }
        }
        for (i, u) in ents.unit.iter().enumerate() {
            let soldier = ents.kind[i] == EntityKind::Soldier;
            let nav = &u.nav;
            let bad = if !u.detached {
                // Corpses lose their stance order too (vacant slots are
                // checked by the entity store).
                u.order != UnitOrder::Idle
                    || *nav != Default::default()
                    || (!soldier && u.stance != Default::default())
            } else {
                !soldier
                    || nav.path.len() > MAX_PATH_POINTS
                    || nav.next as usize > nav.path.len()
                    || (nav.phase == NavPhase::Moving && nav.next as usize >= nav.path.len())
                    || matches!(u.order, UnitOrder::Dig { job } if self
                        .unit_digs
                        .get(&job)
                        .is_none_or(|d| d.owner != ents.owner[i]))
                    || matches!(u.order, UnitOrder::Follow { vehicle }
                        if !self.escorts(ents, ents.owner[i], vehicle))
            };
            if bad {
                return Err("soldier unit order state is inconsistent");
            }
        }
        if self.spot_cursor.iter().any(|&c| c as usize > ents.len()) {
            return Err("spotting cursor out of range");
        }
        for (i, c) in ents.combat.iter().enumerate() {
            let kind = ents.kind[i];
            let vehicle = matches!(kind, EntityKind::Vehicle | EntityKind::Wreck);
            let bad = c.seen_by >> np != 0
                || c.target.is_some_and(|t| t.index() >= ents.len())
                || c.suppression > 100_000
                || (kind == EntityKind::Soldier && (c.hp == 0 || c.corpse != 0))
                || (kind == EntityKind::Corpse && (c.corpse == 0 || ents.squad[i].is_some()))
                || (vehicle
                    && *c
                        != crate::ecs::Combat {
                            seen_by: c.seen_by,
                            ..Default::default()
                        });
            if bad {
                return Err("soldier combat state is inconsistent");
            }
        }
        let linked = ents.squad.iter().filter(|s| s.is_some()).count();
        if linked != members {
            return Err("soldier squad links disagree with squad member lists");
        }
        self.validate_items(ents)?;
        self.validate_vehicles(ents, tick)?;
        let mut queued: Vec<PathReq> = self.path_queue.queues.iter().flatten().copied().collect();
        let nodes = grid_dim(s) * grid_dim(s);
        if queued.iter().any(|r| match *r {
            PathReq::Unit(m) => m.index() >= ents.len(),
            // Requests of squads that are gone are skipped when served.
            PathReq::Squad(q) => self.is_vehicle_squad(q),
            PathReq::Vehicle { squad, avoid } => {
                avoid.is_some_and(|n| n >= nodes)
                    || (self.squads.contains_key(&squad) && !self.is_vehicle_squad(squad))
            }
        }) {
            return Err("a path request names an unknown soldier or the wrong squad kind");
        }
        let n = queued.len();
        // One request per squad, whatever its `avoid`.
        for r in &mut queued {
            if let PathReq::Vehicle { squad, .. } = r {
                *r = PathReq::Vehicle {
                    squad: *squad,
                    avoid: None,
                };
            }
        }
        queued.sort_unstable();
        queued.dedup();
        if queued.len() != n {
            return Err("a squad or soldier is queued for a path twice");
        }
        Ok(())
    }

    /// Checks the S3 vehicle state of a loaded save: every vehicle and
    /// wreck entity has a state and the other way round, states in range,
    /// references to live entities of the right kind, the wreck cap, the
    /// AT-ditch lines.
    fn validate_vehicles(&self, ents: &Entities, tick: u64) -> Result<(), &'static str> {
        let s = &self.setup;
        let size = Fx::from_int(s.map.size_m);
        let kind = |id: SimId| ents.kind.get(id.index()).copied();
        let vehicle_entities = ents
            .kind
            .iter()
            .filter(|k| matches!(k, EntityKind::Vehicle | EntityKind::Wreck))
            .count();
        if vehicle_entities != self.vehicles.len() {
            return Err("vehicle entities and vehicle states disagree");
        }
        let mut wrecks = 0u32;
        for (&id, vs) in &self.vehicles {
            vs.validate(s, size, tick)?;
            let wreck = match kind(id) {
                Some(EntityKind::Vehicle) => false,
                Some(EntityKind::Wreck) => true,
                _ => return Err("a vehicle state names no vehicle"),
            };
            let i = id.index();
            if wreck != vs.wreck_tick.is_some()
                || (wreck && ents.squad[i].is_some())
                || (!wreck && ents.squad[i].is_none_or(|q| !self.squads.contains_key(&q)))
            {
                return Err("a vehicle's squad or wreck state is inconsistent");
            }
            wrecks += wreck as u32;
            let enemy = |t: SimId| ents.owner[t.index()] != ents.owner[i];
            let target_ok = vs.target.is_none_or(|t| {
                matches!(kind(t), Some(EntityKind::Vehicle | EntityKind::Soldier)) && enemy(t)
            }) && vs
                .attack
                .is_none_or(|t| kind(t) == Some(EntityKind::Vehicle) && enemy(t))
                && vs.mgs.iter().all(|m| {
                    m.target
                        .is_none_or(|t| kind(t) == Some(EntityKind::Soldier) && enemy(t))
                });
            let followers_ok = vs.followers.iter().all(|&f| {
                kind(f) == Some(EntityKind::Soldier) && ents.owner[f.index()] == ents.owner[i]
            });
            if !target_ok
                || !followers_ok
                || (wreck
                    && (vs.target.is_some()
                        || vs.attack.is_some()
                        || vs.mgs.iter().any(|m| m.target.is_some())
                        || !vs.followers.is_empty()))
            {
                return Err("a vehicle refers to a missing or wrong entity");
            }
        }
        if wrecks > s.rules.vehicles.wreck_cap {
            return Err("more wrecks than the cap");
        }
        let on_map = |q: &FxVec2| q.x >= Fx::ZERO && q.y >= Fx::ZERO && q.x <= size && q.y <= size;
        if self.at_ditches.len() > MAX_AT_DITCHES
            || self
                .at_ditches
                .iter()
                .any(|l| !(2..=MAX_DIG_POINTS).contains(&l.len()) || !l.iter().all(on_map))
        {
            return Err("bad AT-ditch line");
        }
        Ok(())
    }
}

/// Grid points per side of the setup's map (the path grid).
pub(crate) fn grid_dim(s: &MatchSetup) -> u32 {
    (s.map.size_m / crate::terrain::SPACING_M + 1) as u32
}

/// Whether a dig order's `line` and `job` are in range (loaded saves):
/// at most `max_segments` segments, diggers inside the entity store.
fn dig_consistent(
    s: &MatchSetup,
    line: &[FxVec2],
    job: &DigJob,
    max_segments: usize,
    ents: &Entities,
) -> bool {
    let len = squad::polyline_length(line);
    let size = Fx::from_int(s.map.size_m);
    let on_map = |q: &FxVec2| q.x >= Fx::ZERO && q.y >= Fx::ZERO && q.x <= size && q.y <= size;
    let dig = job.rules(&s.rules);
    (2..=MAX_DIG_POINTS).contains(&line.len())
        && line.iter().all(on_map)
        && len <= Fx::from(dig.max_length)
        && (1..=max_segments).contains(&job.segments.len())
        && job.front.abs() == 1
        && job.segments.iter().all(|g| {
            g.from >= Fx::ZERO
                && g.from <= g.to
                && g.to <= len
                && g.passes as u32 <= dig.passes
                && g.work <= DigJob::required(g, dig, s.tick_hz)
                && g.digger.is_none_or(|d| d.index() < ents.len())
        })
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
