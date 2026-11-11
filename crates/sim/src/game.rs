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
//!    request ([`Nav`]); an accepted call-in spawns its squad at once
//!    (S3b);
//! 2. the [`PathQueue`] serves path requests round-robin by player within a
//!    node-expansion budget (`crate::path`);
//! 3. digging squads work on their trench (`crate::dig`);
//! 4. squads steer their members (`crate::squad`).
//!
//! Call-ins (S3b, owner 2026-11-01: no queue, no delivery time): a
//! `CallIn` buys at once when the player has the manpower, the type's
//! re-call cooldown is over, the pop cap allows it and, for a vehicle, the
//! rules' unlock time has passed and the player is under the vehicle limit.
//! The squad spawns at its base in the same tick and its type's cooldown
//! starts ([`PlayerState::ready_at`], shared within a cooldown group).
//! Otherwise the command is refused with a [`CallInRefusal`]; every
//! call-in's result goes to the tick's [`CallInEvent`]s for the HUD feed.
//!
//! Implemented: Move, AttackMove (halts to fight while the squad has a focus
//! target and a man with a clear shot), Stop, Retreat, CallIn, Surrender,
//! Dig (the squad walks to the line, splits it and digs it in passes) and
//! FireMission (S5b, was Barrage: a mortar squad in range holds and fires;
//! out of range it first walks to 90 % of its range from the target, and
//! gives up when it cannot get into range; modes, observers, ranging and
//! the knowledge rule are in `crate::fire_mission`). Combat (`crate::combat`) and spotting
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
//! `*Units` versions naming the vehicle id) work on it; `Dig`, `FireMission`,
//! `Regroup` and stance orders are rejected. Its path is a
//! [`PathReq::Vehicle`] on the vehicle costs; it drives in the Movement
//! stage, not in squad steering. Vehicle call-ins wait for the rules'
//! unlock time and count against the per-player vehicle limit (vehicles
//! alive), and every squad type counts its own pop (men, or its vehicle's
//! `pop`).

use std::collections::BTreeMap;

use frontline_content::setup::{Capability, SquadTypeSetup};
use frontline_content::{DigProfile, ItemKind, MatchSetup, WeaponClass};
use serde::{Deserialize, Serialize};

use crate::ai::AiState;
use crate::armour::ArmourTables;
use crate::combat;
use crate::command::{Command, CommandKind};
use crate::dig::DigJob;
use crate::ecs::{Entities, EntityKind, PlayerId, SimId};
use crate::fire_mission::{
    FireMissionEvent, FireMissionRefusal, FmCounters, IntelCell, Mission, RangingCell, mode, phase,
};
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
/// Most rounds in one fire mission (S5b; was the Barrage order's).
pub const MAX_MISSION_ROUNDS: u16 = 50;
/// Spawn spots per base: six on a ring and the centre.
const SPAWN_SPOTS: u32 = 7;
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
    /// S5b (was `Barrage`): a mortar fire mission on `target`; `rounds`
    /// left, `m` its mode, phase and aim state (`crate::fire_mission`). The
    /// order becomes `Idle` after the last round or the last bomb.
    FireMission {
        target: FxVec2,
        rounds: u16,
        m: Mission,
    },
    /// Escort friendly vehicle `vehicle` (S3 `FollowUnits`): the men walk
    /// in single file behind it (`crate::unit` escort steering). Becomes
    /// `Idle` when the vehicle is gone.
    Follow {
        vehicle: SimId,
    },
    /// S3b `SuppressUnits`: the men hold and fire at points on `line` (a
    /// point when both ends are equal) without a spotted target
    /// (`crate::combat` "Suppress"). Lasts until another order.
    Suppress {
        line: [FxVec2; 2],
    },
    /// S3c `Recrew`: walk to abandoned vehicle `vehicle` and board it
    /// (`crate::bail`). Becomes `Idle` when it is no longer abandoned.
    Recrew {
        vehicle: SimId,
    },
    /// S8a `Embark`: walk to own transport `vehicle`'s door and board it
    /// (`crate::transport`); kept while the men ride. Becomes `Idle` when
    /// the men are out (or the vehicle is gone).
    Embark {
        vehicle: SimId,
    },
    /// S10a `Fortify`: build work `work` (`crate::fort`). Becomes `Idle`
    /// when the work is done (or gone).
    Fortify {
        work: u32,
    },
    /// S10a `CutWire`: cut wire section `section`; `man` the cutter,
    /// `work` the points done. Becomes `Idle` when the section is gone.
    CutWire {
        section: u32,
        man: Option<SimId>,
        work: u64,
    },
    /// S10b `ClearMines`: sweep the lane `a`..`b`; `along` how far the
    /// sweepers are, `lift` the mine being lifted, `work` its points done.
    /// Becomes `Idle` at the lane end with no known mine left in it.
    ClearMines {
        a: FxVec2,
        b: FxVec2,
        along: Fx,
        lift: Option<u32>,
        work: u64,
    },
    /// S10b `Demolish`: `man` lays a satchel against piece `piece`; `work`
    /// the placing points done. Becomes `Idle` when the charge is laid.
    Demolish {
        piece: u32,
        man: Option<SimId>,
        work: u64,
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
    /// S5b: was `Barrage` (same number).
    FireMission = 5,
    /// S3: escorting a vehicle.
    Follow = 6,
    /// S3b: area fire at a point or line.
    Suppress = 7,
    /// S3c: re-crewing an abandoned vehicle.
    Recrew = 8,
    /// S8a: boarding or riding a transport.
    Embark = 9,
    /// S10a: building a fortification.
    Fortify = 10,
    /// S10a: cutting wire.
    CutWire = 11,
    /// S10b: sweeping and lifting mines.
    ClearMines = 12,
    /// S10b: placing a demolition charge.
    Demolish = 13,
}

impl Order {
    pub fn kind(&self) -> OrderKind {
        match self {
            Order::Idle => OrderKind::Idle,
            Order::Move { .. } => OrderKind::Move,
            Order::AttackMove { .. } => OrderKind::AttackMove,
            Order::Retreat => OrderKind::Retreat,
            Order::Dig { .. } => OrderKind::Dig,
            Order::FireMission { .. } => OrderKind::FireMission,
            Order::Follow { .. } => OrderKind::Follow,
            Order::Suppress { .. } => OrderKind::Suppress,
            Order::Recrew { .. } => OrderKind::Recrew,
            Order::Embark { .. } => OrderKind::Embark,
            Order::Fortify { .. } => OrderKind::Fortify,
            Order::CutWire { .. } => OrderKind::CutWire,
            Order::ClearMines { .. } => OrderKind::ClearMines,
            Order::Demolish { .. } => OrderKind::Demolish,
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
    /// S4: crew weapons of fallen men waiting to be picked up (`crate::
    /// combat` "Crew weapons"), in the order they fell.
    pub drops: Vec<WeaponDrop>,
    /// S4: single-shot AT items (Panzerfaust): the squad's aim and gap;
    /// `Some` for squads whose members carry one at spawn.
    pub pzf: Option<SingleShot>,
    /// S5b: mortar squads' ranging memory (target areas it ranged on,
    /// `crate::fire_mission`); empty for other squads.
    pub ranging: Vec<RangingCell>,
}

/// Most crew weapons waiting at once in one squad (S4).
pub const MAX_DROPS: usize = 16;

/// S4: a crew weapon lying where its carrier fell (`crate::combat` "Crew
/// weapons").
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct WeaponDrop {
    /// Index into `MatchSetup::weapons`.
    pub weapon: u16,
    /// Where it lies.
    pub at: FxVec2,
    /// Ticks until the nearest man has it (the weapon's `handover_ticks`).
    pub ticks: u16,
    /// S5a: rounds left in it (legacy content: unused, a fresh belt).
    pub mag: u16,
}

/// S4: a squad's single-shot AT items (`crate::combat` "Panzerfaust").
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct SingleShot {
    /// The man aiming one, and at what.
    pub man: Option<SimId>,
    pub target: Option<SimId>,
    /// Against `target`: aim at its turret, not its hull.
    pub aim_turret: bool,
    /// Ticks of aiming left before the shot.
    pub aim: u16,
    /// Ticks until the squad may fire the next one
    /// (`items.single_shot_gap_ticks`).
    pub gap: u16,
    /// Ticks until the next target search.
    pub search: u16,
    /// `AttackUnit`: the enemy vehicle to fire at first.
    pub attack: Option<SimId>,
}

impl SingleShot {
    /// The fresh state a new squad of type `ty` starts with: `Some` when
    /// its members carry a single-shot AT item, else `None`. Every place
    /// that creates an infantry squad (call-in, bail-out crew) uses this,
    /// so the load check (`pzf_ok`) holds for any card (S4 review M1).
    pub fn for_type(ty: &SquadTypeSetup) -> Option<Self> {
        (ty.vehicle.is_none() && ty.squad_items(ItemKind::SingleShotAt) > 0).then(Self::default)
    }
}

/// An AT team's launcher (S3, `crate::combat` "Launchers"): who carries
/// it, the rockets left and the `AttackUnit` vehicle. S4: when its carrier
/// falls it is a [`WeaponDrop`] until the next man picks it up.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Launcher {
    /// The man carrying it; `None` while it waits to be picked up.
    pub carrier: Option<SimId>,
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

/// Why a call-in was refused (S3b). The HUD and the feed show it.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CallInRefusal {
    /// No such squad type, or not in the player's faction roster.
    NotInRoster,
    /// Vehicles are not unlocked yet: ticks until the unlock.
    Locked { ticks: u32 },
    /// The player has as many vehicles alive as the rules allow.
    VehicleLimit,
    /// The type's re-call cooldown: ticks until it is ready.
    Cooldown { ticks: u32 },
    /// S4: as many squads of this card alive as its `max_alive`.
    MaxAlive { max: u16 },
    /// S4: the card's limit group (index into `RulesSetup::limit_groups`)
    /// is full.
    GroupLimit { group: u16 },
    /// Short of manpower by this much.
    Manpower { short: u32 },
    /// The squad would take the player over the pop cap.
    PopCap,
}

/// One call-in command's result in a tick (S3b). Presentation only: kept
/// for the snapshot of the tick it happened in, never saved or hashed.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct CallInEvent {
    pub player: PlayerId,
    pub squad_type: u16,
    /// The squad that spawned, or why the call-in was refused.
    pub result: Result<SquadId, CallInRefusal>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PlayerState {
    pub manpower: u32,
    pub tickets: u32,
    pub surrendered: bool,
    /// S3b: per squad type (index into `MatchSetup::squad_types`), the
    /// first tick at which it can be called in again: the start cooldown,
    /// then the call-in tick plus the type's re-call cooldown (pushed to
    /// every type of its cooldown group).
    pub ready_at: Vec<u64>,
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
    /// S3c bail-out and re-crew counters (report).
    pub crew: CrewCounters,
    /// S4 counters (report).
    pub s4: S4Counters,
    /// S5a: prepaid ammunition credit, manpower paid by supply kind, refill
    /// pauses and rounds fired by class (`crate::supply`).
    pub ammo: crate::supply::AmmoAccount,
    /// S5b: the player's knowledge map for map fire, by cell index
    /// (`crate::fire_mission`): only cells where an enemy was seen.
    pub intel: BTreeMap<u32, IntelCell>,
    /// S5b: fire-mission counters (report).
    pub fm: FmCounters,
    /// S7: per-vehicle-type kill and knock-out counters (report).
    pub s7: S7Counters,
    /// S8a: transport and small-arms counters (report).
    pub s8a: S8aCounters,
    /// S9: gun counters (report).
    pub s9: crate::guns::S9Counters,
}

/// S8a per-player counters (saved and hashed, read only by reports;
/// `crate::transport`). A ride is one squad's loaded trip from the first
/// man aboard to the vehicle empty; `transported` counts rides of at least
/// `[ai.transport] min_ride_m` (straight line, board to unload).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct S8aCounters {
    pub rides: u32,
    pub transported: u32,
    /// Ticks the rides took (boarding to the last man out).
    pub ride_ticks: u64,
    /// Ticks the same squads would have walked (straight line at their
    /// walking speed).
    pub ride_walk_ticks: u64,
    /// Own men killed while riding.
    pub riders_lost: u32,
    /// Penetrations by this player's small arms (vehicles.md 6.4).
    pub small_arms_pens: u32,
    /// Unloads ordered under fire (or forced by a knock-out or fire).
    pub dismounts_under_fire: u32,
    /// Loaded trips of at least `min_ride_m` that ended in an unload.
    pub transport_trips: u32,
}

/// S7 per-player counters (saved and hashed, read only by reports): by
/// vehicle type (index into `MatchSetup::vehicles`, length the setup's
/// vehicle count), the enemy men killed and the enemy vehicles knocked
/// out by rounds, spall or bursts from a vehicle of that type (the
/// projectile's shooter, [`kill_class::with_vehicle`]). Later fire deaths
/// stay in `kills` only.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct S7Counters {
    pub kills_by_vehicle: Vec<u32>,
    pub kos_by_vehicle: Vec<u32>,
    /// Build B: seats and parts of enemy open-topped vehicles this player
    /// struck through an open face, over the rim or from inside.
    pub open_top_hits: u32,
    /// Items this player's men threw that came to rest inside an enemy
    /// open box.
    pub open_top_throws: u32,
}

/// S4 per-player counters (saved and hashed, read only by reports and
/// the HUD).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct S4Counters {
    /// Crew weapons own men picked up from a fallen man.
    pub pickups: u32,
    /// Single-shot AT items (Panzerfausts) fired.
    pub single_shots: u32,
}

/// S3c per-player counters (`crate::bail`; saved and hashed, read only by
/// reports and the HUD).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct CrewCounters {
    /// Own vehicles abandoned.
    pub abandoned: u32,
    /// Own abandoned vehicles manned again by own men.
    pub recrewed: u32,
    /// Enemy abandoned vehicles this player captured.
    pub captured: u32,
    /// Own crewmen alive when their vehicle was abandoned ...
    pub bailed: u32,
    /// ... and those who got out.
    pub out: u32,
    /// Crewmen dismissed in the own base (no refund).
    pub home: u32,
    /// Own abandoned vehicles scrapped by the abandoned cap.
    pub scrapped: u32,
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

    /// S7: a class value may carry the shooting vehicle's type in its high
    /// nibble (`class | (type + 1) << 4`) so per-type kills need no new
    /// plumbing. The low nibble is the class; [`base`] before indexing.
    const BASE_MASK: u8 = 0x0F;
    /// Most vehicle types a class can name (types past it go uncounted per
    /// type; `kills_by` is unaffected).
    pub const MAX_VEHICLE_TYPES: usize = 15;

    /// The class proper.
    #[inline]
    pub const fn base(c: u8) -> u8 {
        c & BASE_MASK
    }

    /// The shooting vehicle's type, if `c` carries one.
    #[inline]
    pub const fn vehicle(c: u8) -> Option<usize> {
        match c >> 4 {
            0 => None,
            t => Some(t as usize - 1),
        }
    }

    /// `c` carrying vehicle type `t` (unchanged past [`MAX_VEHICLE_TYPES`]).
    #[inline]
    pub const fn with_vehicle(c: u8, t: u16) -> u8 {
        if (t as usize) < MAX_VEHICLE_TYPES {
            base(c) | ((t as u8 + 1) << 4)
        } else {
            base(c)
        }
    }
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
    /// S5b: the next fire-mission number (`Mission::id`; starts at 1).
    pub(crate) next_mission: u32,
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
    /// Call-in results of the command being applied (S3b): `World` moves
    /// them to the snapshot right away, so this is empty between commands
    /// (never saved, hashed or read by the sim).
    #[serde(skip)]
    pub(crate) call_in_events: Vec<CallInEvent>,
    /// S3c: refused `Recrew` commands, moved to the snapshot like
    /// `call_in_events` (never saved, hashed or read by the sim).
    #[serde(skip)]
    pub(crate) recrew_events: Vec<crate::bail::RecrewEvent>,
    /// S4: crew weapons picked up this tick, moved to the snapshot like
    /// `call_in_events` (never saved, hashed or read by the sim).
    #[serde(skip)]
    pub(crate) pickup_events: Vec<PickupEvent>,
    /// S5b: refused fire missions, moved to the snapshot like
    /// `call_in_events` (never saved, hashed or read by the sim).
    #[serde(skip)]
    pub(crate) fm_events: Vec<FireMissionEvent>,
    /// S5b diagnostic burst log (`World::enable_burst_log`, mortar-bench):
    /// not saved, not hashed, never read by the sim.
    #[serde(skip)]
    pub(crate) burst_log: Option<Vec<crate::fire_mission::BurstRecord>>,
    /// S6: structures, crates, cook-offs and per-player accounts
    /// (`crate::structures`; saved, hashed, validated).
    pub(crate) supply6: crate::structures::SupplyState,
    /// S6: this tick's static boxes (derived each tick before the
    /// Ballistics stage; never saved or hashed).
    #[serde(skip)]
    pub(crate) statics: crate::structures::StaticList,
    /// S6: refused S6 commands, moved to the snapshot like
    /// `call_in_events` (never saved, hashed or read by the sim).
    #[serde(skip)]
    pub(crate) supply_events: Vec<crate::structures::SupplyEvent>,
    /// S6: bombs counted sources put into mortar squads this tick, per
    /// player (the T20 check; empty between ticks).
    #[serde(skip)]
    pub(crate) bombs_explained: Vec<u64>,
    /// S10a: works, pieces, wire, charges, carried kg and accounts
    /// (`crate::fort`; saved, hashed, validated).
    pub(crate) fort: crate::fort::FortState,
    /// S10a: refused S10a commands, moved to the snapshot like
    /// `supply_events` (never saved, hashed or read by the sim).
    #[serde(skip)]
    pub(crate) fort_events: Vec<crate::fort::FortEvent>,
    /// S10a test switch (`empty_fort_state_changes_nothing`): every S10a
    /// hook (fort stage, wire pace and path layer, piece statics, thrown
    /// items on pieces, overhead cover, wire blast and fragments, piece
    /// damage gates, charges, carried-kg trim) takes the pre-S10a path.
    /// Never saved or hashed; false in play.
    #[serde(skip)]
    pub(crate) fort_off: bool,
}

/// S4: a man picked up a fallen man's crew weapon (presentation only).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PickupEvent {
    pub squad: SquadId,
    pub man: SimId,
    /// Index into `MatchSetup::weapons`.
    pub weapon: u16,
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
        let start_ready: Vec<u64> = setup
            .squad_types
            .iter()
            .map(|t| t.initial_cooldown_ticks as u64)
            .collect();
        let players = setup
            .players
            .iter()
            .map(|_| PlayerState {
                manpower: r.start_manpower,
                tickets: r.tickets,
                surrendered: false,
                ready_at: start_ready.clone(),
                kills: 0,
                losses: 0,
                defeat: None,
                point_ticks: 0,
                income_acc: 0,
                drain_acc: 0,
                kills_by: [0; kill_class::COUNT],
                rockets: 0,
                crew: CrewCounters::default(),
                s4: S4Counters::default(),
                ammo: crate::supply::AmmoAccount::default(),
                intel: BTreeMap::new(),
                fm: FmCounters::default(),
                s7: S7Counters {
                    kills_by_vehicle: vec![0; setup.vehicles.len()],
                    kos_by_vehicle: vec![0; setup.vehicles.len()],
                    ..S7Counters::default()
                },
                s8a: S8aCounters::default(),
                s9: crate::guns::S9Counters::default(),
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
            next_mission: 1,
            armour: ArmourTables::new(
                &setup.ammo,
                &setup.vehicles,
                &setup.weapons,
                &setup.rules.vehicles.gunnery,
            ),
            vderived: VehicleDerived::new(&setup, grid_dim(&setup)),
            hit_log: None,
            call_in_events: Vec::new(),
            recrew_events: Vec::new(),
            pickup_events: Vec::new(),
            fm_events: Vec::new(),
            burst_log: None,
            supply6: crate::structures::SupplyState::new(setup.players.len()),
            statics: Default::default(),
            supply_events: Vec::new(),
            bombs_explained: Vec::new(),
            fort: crate::fort::FortState::new(setup.players.len()),
            fort_events: Vec::new(),
            fort_off: false,
            setup,
        }
    }

    /// Rebuilds the derived caches from the setup (after load). The vehicle
    /// structures that also need the entities and terrain are rebuilt by
    /// `rebuild_vehicle_derived`.
    pub(crate) fn rebuild_derived(&mut self) {
        self.armour = ArmourTables::new(
            &self.setup.ammo,
            &self.setup.vehicles,
            &self.setup.weapons,
            &self.setup.rules.vehicles.gunnery,
        );
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
    /// The armour tables (armour-test's closed-form penetration checks).
    pub fn armour_tables(&self) -> &ArmourTables {
        &self.armour
    }

    pub fn vehicle_derived(&self) -> &VehicleDerived {
        &self.vderived
    }

    /// Base centre of `player`.
    pub fn base_of(&self, player: PlayerId) -> FxVec2 {
        let base = self.setup.players[player.0 as usize].base as usize;
        self.setup.map.bases[base].center.into()
    }

    /// Pop-cap count of `player`: soldiers alive and each live vehicle's
    /// `pop` (S3). S3b: no call-in waits anywhere, a bought squad is alive
    /// at once.
    pub fn population(&self, ents: &Entities, player: PlayerId) -> u32 {
        let alive = ents
            .soldiers()
            .filter(|id| ents.owner[id.index()] == player)
            .count() as u32;
        // S9: a gun counts its own `pop`; its crew are soldiers (above).
        let vehicles: u32 = self
            .vehicles
            .iter()
            .filter(|(id, _)| {
                ents.kind[id.index()] == EntityKind::Vehicle && ents.owner[id.index()] == player
            })
            .map(|(_, vs)| self.setup.vehicles[vs.type_idx as usize].pop)
            .sum();
        alive + vehicles
    }

    /// Ticks until `player` can call in `squad_type` again (0: ready).
    pub fn cooldown_left(&self, player: PlayerId, squad_type: u16, tick: u64) -> u32 {
        self.players
            .get(player.0 as usize)
            .and_then(|p| p.ready_at.get(squad_type as usize))
            .map_or(0, |&t| t.saturating_sub(tick).min(u32::MAX as u64) as u32)
    }

    /// S4: `player`'s squads of `squad_type` alive now.
    pub fn alive_of_type(&self, player: PlayerId, squad_type: u16) -> u32 {
        self.squads
            .values()
            .filter(|sq| sq.owner == player && sq.type_idx == squad_type)
            .count() as u32
    }

    /// S4: `player`'s squads alive now whose card is in limit group `g`.
    pub fn alive_in_group(&self, player: PlayerId, g: u16) -> u32 {
        let s = &self.setup;
        self.squads
            .values()
            .filter(|sq| {
                sq.owner == player && s.squad_types[sq.type_idx as usize].limit_group == Some(g)
            })
            .count() as u32
    }

    /// S8a: whether squad type `t` is a transport vehicle's card.
    pub fn is_transport_type(&self, t: u16) -> bool {
        let s = &self.setup;
        s.squad_types
            .get(t as usize)
            .and_then(|ty| ty.vehicle)
            .and_then(|v| s.vehicles.get(v as usize))
            .is_some_and(|v| v.role == frontline_content::vehicle::VehicleRole::Transport)
    }

    /// S8b-1: whether squad type `t` is a vehicle card outside the limit
    /// (transport, recon car, truck: capped by its own card).
    pub fn is_outside_limit_type(&self, t: u16) -> bool {
        let s = &self.setup;
        s.squad_types
            .get(t as usize)
            .and_then(|ty| ty.vehicle)
            .and_then(|v| s.vehicles.get(v as usize))
            .is_some_and(|v| v.role.outside_limit())
    }

    /// Whether `player` could call in `squad_type` at `tick` (S3b), checked
    /// in this order: roster, unlock (the card's `unlock_s`, and the rules'
    /// vehicle unlock as a floor for vehicles), S4 `max_alive`, S4 limit
    /// group, vehicle limit, cooldown, manpower, pop cap. The AI and the
    /// command use the same rule.
    pub fn call_in_check(
        &self,
        ents: &Entities,
        player: PlayerId,
        squad_type: u16,
        tick: u64,
    ) -> Result<(), CallInRefusal> {
        let s = &self.setup;
        let Some(ps) = s.players.get(player.0 as usize) else {
            return Err(CallInRefusal::NotInRoster);
        };
        if !s.factions[ps.faction as usize].roster.contains(&squad_type) {
            return Err(CallInRefusal::NotInRoster);
        }
        let ty = &s.squad_types[squad_type as usize];
        let vr = &s.rules.vehicles;
        // S8a-5: a transport's floor is `[vehicles.transport] unlock_s`,
        // never later than the vehicles' own.
        let transport = self.is_transport_type(squad_type);
        let outside = self.is_outside_limit_type(squad_type);
        let unlock = match ty.vehicle {
            Some(_) if transport => ty
                .initial_cooldown_ticks
                .max((vr.transport.unlock_s * s.tick_hz).min(vr.unlock_ticks)),
            // S8b-1: a recon car or truck keeps its card's own unlock.
            Some(_) if outside => ty.initial_cooldown_ticks,
            Some(_) => ty.initial_cooldown_ticks.max(vr.unlock_ticks),
            None => ty.initial_cooldown_ticks,
        };
        if tick < unlock as u64 {
            return Err(CallInRefusal::Locked {
                ticks: (unlock as u64 - tick) as u32,
            });
        }
        if ty.max_alive > 0 && self.alive_of_type(player, squad_type) >= ty.max_alive as u32 {
            return Err(CallInRefusal::MaxAlive { max: ty.max_alive });
        }
        if let Some(g) = ty.limit_group
            && self.alive_in_group(player, g) >= s.rules.limit_groups[g as usize].limit
        {
            return Err(CallInRefusal::GroupLimit { group: g });
        }
        if ty.vehicle.is_some() && !outside && self.vehicles_counted(ents, player) >= vr.limit {
            return Err(CallInRefusal::VehicleLimit);
        }
        let wait = self.cooldown_left(player, squad_type, tick);
        if wait > 0 {
            return Err(CallInRefusal::Cooldown { ticks: wait });
        }
        let manpower = self.players[player.0 as usize].manpower;
        if manpower < ty.cost {
            return Err(CallInRefusal::Manpower {
                short: ty.cost - manpower,
            });
        }
        // S9: the tow that comes with a gun card counts too, and the crew
        // that come with a gun.
        let tow_pop = ty
            .tow_card
            .map_or(0, |t| s.squad_types[t as usize].pop(&s.vehicles))
            + ty.vehicle
                .map(|v| &s.vehicles[v as usize])
                .filter(|v| v.served())
                .and_then(|v| v.crew_squad)
                .map_or(0, |c| s.squad_types[c as usize].men as u32);
        if self.population(ents, player) + ty.pop(&s.vehicles) + tow_pop > s.rules.pop_cap {
            return Err(CallInRefusal::PopCap);
        }
        Ok(())
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
        // S8a: a squad with men aboard a transport takes only `Disembark`
        // (sent for its vehicle); its men are left out of soldier orders
        // (`unit_set`).
        if let Some(sq) = command_squad(&c.kind)
            && self.squad_rides(ctx.ents, sq)
        {
            return false;
        }
        // S9: an order that sends men away takes them off their guns first.
        self.crew_ordered_off(ctx, p, &c.kind);
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
            &CommandKind::FireMission {
                squad,
                target,
                rounds,
            } => self.order_fire_mission(ctx, p, squad, target, rounds),
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
            CommandKind::SetFire { units, mode } => self.set_fire(ctx, p, units, *mode),
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
            CommandKind::SuppressUnits { units, line } => self.units_suppress(ctx, p, units, line),
            &CommandKind::Recrew { squad, vehicle } => self.order_recrew(ctx, p, squad, vehicle),
            &CommandKind::Abandon { squad } => self.order_abandon(ctx.ents, p, squad),
            &CommandKind::Build {
                squad,
                def,
                pos,
                facing,
            } => self.order_build(ctx, p, squad, def, pos, facing),
            &CommandKind::IssueCrate { squad, kind } => self.issue_crate(ctx, p, squad, kind),
            &CommandKind::PlaceCrate { squad, pos } => self.place_crate(ctx, p, squad, pos),
            &CommandKind::PickUpCrate { squad, crate_id } => {
                self.pick_up_crate(ctx, p, squad, crate_id)
            }
            &CommandKind::DropCrates {
                vehicle,
                point,
                count,
                kind,
            } => self.order_drop_crates(ctx, p, vehicle, point, count, kind),
            CommandKind::Fortify { squad, fort, line } => {
                self.order_fortify(ctx, p, *squad, *fort, line)
            }
            &CommandKind::LoadMaterials { squad, kg } => self.load_materials(ctx, p, squad, kg),
            &CommandKind::CutWire { squad, at } => self.order_cut_wire(ctx, p, squad, at),
            // S10b: accepted by Build C (`ClearMines`) and D (`Demolish`).
            &CommandKind::ClearMines { squad, .. } | &CommandKind::Demolish { squad, .. } => {
                self.refuse_fort(p, Some(squad), crate::fort::FortRefusal::NotAllowed)
            }
            CommandKind::PlaceBangalore { units, target } => {
                self.place_bangalore(ctx, p, units, *target)
            }
            CommandKind::Embark { squads, vehicle } => self.order_embark(ctx, p, squads, *vehicle),
            CommandKind::Disembark {
                vehicles,
                over_sides,
            } => self.order_disembark(ctx, p, vehicles, *over_sides),
            CommandKind::Emplace { guns, facing } => self.order_emplace(ctx, p, guns, *facing),
            CommandKind::PackGun { guns } => self.order_pack(ctx, p, guns),
            CommandKind::PushGun { guns, point } => self.order_push(ctx, p, guns, *point),
            CommandKind::Hitch { guns, tow } => self.order_hitch(ctx, p, guns, *tow),
            CommandKind::Unhitch { guns } => self.order_unhitch(ctx, p, guns),
            CommandKind::SetAmbush { guns, range_m } => self.order_ambush(ctx, p, guns, *range_m),
        }
    }

    /// Whether `vehicle` is a live vehicle of `player` (an escort's).
    pub(crate) fn escorts(&self, ents: &Entities, player: PlayerId, vehicle: SimId) -> bool {
        ents.kind.get(vehicle.index()) == Some(&EntityKind::Vehicle)
            && ents.owner[vehicle.index()] == player
    }

    /// Whether squad type `ty` may dig `profile` (S4: `dig_basic` and a
    /// dig rate; a profile marked `engineers_only` needs `dig_full`).
    pub(crate) fn digs_profile(&self, ty: &SquadTypeSetup, profile: DigProfile) -> bool {
        ty.dig_rate_pct > 0
            && ty.has(Capability::DigBasic)
            && (!self.setup.rules.dig_profile(profile).engineers_only
                || ty.has(Capability::DigFull))
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
        if !self.owned(p, id) || self.is_gun_squad(ctx.ents, id) {
            return false;
        }
        self.clear_detached(ctx, id);
        let goal = match &order {
            Order::Move { target } | Order::AttackMove { target } => ctx.terrain.clamp(*target),
            // S6-9 (A): infantry to the nearest own built HQ behind it,
            // else the base; vehicles and tank crews to the base.
            _ => {
                let sq = &self.squads[&id];
                let base_only = self.is_vehicle_squad(id)
                    || self.setup.squad_types[sq.type_idx as usize].has(Capability::VehicleCrew);
                let lead = sq.lead(ctx.ents);
                self.retreat_goal(p, ctx.ents.transform[lead.index()].pos.xy(), base_only)
            }
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
        if !self.digs_profile(ty, profile)
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
        let goal = self.pit_approach(ctx.ents, id, line, goal, profile);
        self.request_path(id, goal);
        true
    }

    /// S5b: refuses a fire mission with a reason (event and counter).
    fn refuse_mission(&mut self, p: PlayerId, squad: SquadId, refusal: FireMissionRefusal) -> bool {
        self.fm_events.push(FireMissionEvent {
            player: p,
            squad,
            refusal,
        });
        self.players[p.0 as usize].fm.refused[refusal as usize] += 1;
        false
    }

    /// S5b: the refusal a fire mission of squad `id` at `target` would get
    /// now, if any (read-only; the HUD preview and the AI use it too).
    pub fn mission_refusal(
        &self,
        ents: &Entities,
        terrain: &Heightfield,
        id: SquadId,
        target: FxVec2,
        rounds: u16,
        tick: u64,
    ) -> Option<FireMissionRefusal> {
        let sq = self.squads.get(&id)?;
        if !terrain.contains(target) {
            return Some(FireMissionRefusal::OffMap);
        }
        let ty = &self.setup.squad_types[sq.type_idx as usize];
        if !ty.has(Capability::FireMission) || ty.vehicle.is_some() {
            return Some(FireMissionRefusal::NotMortar);
        }
        if !(1..=MAX_MISSION_ROUNDS).contains(&rounds) {
            return Some(FireMissionRefusal::Rounds);
        }
        let w = &self.setup.weapons[ty.weapon as usize];
        let (min, max): (Fx, Fx) = (w.range_min.into(), w.range_max.into());
        let lead = ents.transform[sq.lead(ents).index()].pos.xy();
        let dist = (target - lead).length();
        if dist < min {
            return Some(FireMissionRefusal::InsideMinimum);
        }
        // Legacy rules (no `[fire_mission]`): the old barrage's checks only.
        self.setup.rules.fire_mission.as_ref()?;
        if crate::supply::mortar_bombs(&self.setup, ents, sq).0 == 0 {
            return Some(FireMissionRefusal::NoBombs);
        }
        if !self.known_point(ents, terrain, sq.owner, target, tick) {
            return Some(FireMissionRefusal::UnknownGround);
        }
        if dist <= max
            && let Some(ind) = &w.indirect
        {
            let g = self
                .mortar_gunner(ents, sq)
                .unwrap_or_else(|| sq.lead(ents));
            let gp = ents.transform[g.index()].pos;
            let from = crate::fx::FxVec3::new(
                gp.x,
                gp.y,
                gp.z + Fx::from_milli(self.setup.rules.combat.mortar.muzzle_mm as i32),
            );
            let to = target.extend(terrain.height_at(target));
            if crate::indirect::solve(ind, from, to).is_none() {
                return Some(FireMissionRefusal::CannotReach);
            }
        }
        None
    }

    fn order_fire_mission(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        id: SquadId,
        target: FxVec2,
        rounds: u16,
    ) -> bool {
        if !self.owned(p, id) {
            return false;
        }
        if let Some(r) = self.mission_refusal(ctx.ents, ctx.terrain, id, target, rounds, ctx.tick) {
            return self.refuse_mission(p, id, r);
        }
        let ty = &self.setup.squad_types[self.squads[&id].type_idx as usize];
        let w = &self.setup.weapons[ty.weapon as usize];
        let max: Fx = w.range_max.into();
        let lead = self.squads[&id].lead(ctx.ents);
        let lead = ctx.ents.transform[lead.index()].pos.xy();
        let dist = (target - lead).length();
        let fm = self.setup.rules.fire_mission.clone();
        // Legacy content regrouped detached men (the old Barrage); S5b keeps
        // them out (the squad's own spotter), unless nobody left in the
        // formation carries the mortar.
        let regroup = match &fm {
            None => self.clear_detached(ctx, id),
            Some(_) if self.mortar_gunner(ctx.ents, &self.squads[&id]).is_none() => {
                self.clear_detached(ctx, id)
            }
            Some(_) => false,
        };
        let mut m = Mission::default();
        if let Some(fm) = &fm {
            let sq = &self.squads[&id];
            let g = self
                .mortar_gunner(ctx.ents, sq)
                .unwrap_or_else(|| sq.lead(ctx.ents));
            let gp = ctx.ents.transform[g.index()].pos.xy();
            let (mode0, obs) = self.mode_for(ctx.ents, ctx.terrain, id, gp, target, ctx.tick);
            let hz = self.setup.tick_hz;
            // The start error grows with the range from the tube; beyond
            // range the team walks to 90 % of it first and fires from there
            // (Review L2).
            let range = (target - gp).length();
            let range = if dist > max {
                range.min(max.mul_int(9).div_int(10))
            } else {
                range
            };
            let err = crate::fire_mission::start_error(fm, mode0 == mode::MAP, range);
            let mut cells = std::mem::take(&mut self.squads.get_mut(&id).expect("owned").ranging);
            crate::fire_mission::prune_ranging(&mut cells, fm, ctx.tick, hz, gp);
            let remembered = (mode0 != mode::MAP)
                .then(|| crate::fire_mission::ranged_cell(&cells, fm, target))
                .flatten()
                .filter(|c| c.offset.length() < err)
                .map(|c| c.offset);
            let offset = match remembered {
                Some(o) => o,
                None => {
                    let mut rng = crate::rng::RngStream::new(
                        self.setup.seed,
                        ctx.tick,
                        SimId(id.0),
                        crate::rng::Purpose::MISSION_AIM,
                    );
                    let a = rng.range_fx(Fx::ZERO, Fx::TAU);
                    let (sn, cs) = a.sin_cos();
                    FxVec2::new(cs.mul(err), sn.mul(err))
                }
            };
            let prep = if mode0 == mode::DIRECT {
                fm.direct_s
            } else {
                fm.fire_direction_s
            };
            m = Mission {
                id: self.next_mission,
                mode: mode0,
                phase: phase::LAYING,
                offset,
                err0: offset.length(),
                order_tick: ctx.tick,
                ready_at: ctx.tick + prep as u64 * hz as u64,
                observer: obs,
                ranged: remembered.is_some(),
                ..Mission::default()
            };
            self.next_mission += 1;
            self.players[p.0 as usize].fm.missions[mode0 as usize] += 1;
            self.squads.get_mut(&id).expect("owned").ranging = cells;
        }
        let sq = self.squads.get_mut(&id).expect("owned");
        sq.order = Order::FireMission { target, rounds, m };
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

    /// A call-in (S3b): buys and spawns the squad at once, or refuses it
    /// (see [`Self::call_in_check`]). Either way the result is logged for
    /// the snapshot.
    fn call_in(&mut self, ctx: &mut Ctx, p: PlayerId, squad_type: u16) -> bool {
        let result = match self.call_in_check(ctx.ents, p, squad_type, ctx.tick) {
            Err(why) => Err(why),
            Ok(()) => {
                let ty = &self.setup.squad_types[squad_type as usize];
                let ready = ctx.tick + ty.recall_cooldown_ticks as u64;
                let group = ty.cooldown_group;
                let cost = ty.cost;
                let pl = &mut self.players[p.0 as usize];
                pl.manpower -= cost;
                for (t, other) in self.setup.squad_types.iter().enumerate() {
                    if t == squad_type as usize
                        || (group.is_some() && other.cooldown_group == group)
                    {
                        pl.ready_at[t] = pl.ready_at[t].max(ready);
                    }
                }
                let q = self.spawn_squad(ctx, p, squad_type);
                self.gun_arrives(ctx, p, squad_type, q);
                Ok(q)
            }
        };
        // Unknown types are not logged: the presenter could not name them.
        if (squad_type as usize) < self.setup.squad_types.len() {
            self.call_in_events.push(CallInEvent {
                player: p,
                squad_type,
                result,
            });
        }
        result.is_ok()
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

    /// Where squad `id` of `owner` spawns (S3b): one of seven spots, six on
    /// a ring at `call_in.spawn_ring_pct` of the base radius (spot k at
    /// k·60° from the base's facing to the map centre, S3c) and the
    /// centre. The squad tries spot `id mod 7` first, then the next ones in
    /// turn, and takes the first with the fewest men (a vehicle counting
    /// `spawn_vehicle_weight`) within `call_in.spawn_clear_m`, so squads
    /// bought in quick succession, or onto a spot a squad still stands
    /// on, spread out. Every spot is inside the base.
    fn spawn_spot(
        &self,
        ents: &Entities,
        terrain: &Heightfield,
        owner: PlayerId,
        id: SquadId,
    ) -> FxVec2 {
        let base = &self.setup.map.bases[self.setup.players[owner.0 as usize].base as usize];
        let center: FxVec2 = base.center.into();
        let ci = &self.setup.rules.call_in;
        let ring = Fx::from(base.radius)
            .mul_int(ci.spawn_ring_pct as i32)
            .div_int(100);
        // S3c (S3c-8 (A)): the ring turns with the base's facing to the map
        // centre, so the two bases' spots are point-mirrors (they were at
        // fixed compass angles, up to 20 m apart in effect).
        let half = Fx::from_int(self.setup.map.size_m).div_int(2);
        let face = (FxVec2::new(half, half) - center).normalize_or_zero();
        let face = if face == FxVec2::ZERO {
            FxVec2::new(Fx::ONE, Fx::ZERO)
        } else {
            face
        };
        let spot = |k: u32| {
            let p = if k == SPAWN_SPOTS - 1 {
                center
            } else {
                let (s, c) = Fx::from_ratio(k as i64 * 1047, 1000).sin_cos(); // k · 60°
                let d = FxVec2::new(face.x.mul(c) - face.y.mul(s), face.x.mul(s) + face.y.mul(c));
                center + d * ring
            };
            terrain.clamp(p)
        };
        let clear = Fx::from_int(ci.spawn_clear_m as i32).raw().max(0) as u64;
        let clear2 = clear * clear;
        let crowd = |at: FxVec2| -> u32 {
            let near = |q: FxVec2| (q - at).length_sq_raw() <= clear2;
            let men = ents
                .soldiers()
                .filter(|m| near(ents.transform[m.index()].pos.xy()))
                .count() as u32;
            let vehicles = self
                .vehicles
                .keys()
                .filter(|v| near(ents.transform[v.index()].pos.xy()))
                .count() as u32;
            men + vehicles * ci.spawn_vehicle_weight
        };
        let first = id.0 % SPAWN_SPOTS;
        let mut best = (u32::MAX, spot(first));
        for j in 0..SPAWN_SPOTS {
            let at = spot((first + j) % SPAWN_SPOTS);
            let n = crowd(at);
            if n < best.0 {
                best = (n, at);
                if n == 0 {
                    break;
                }
            }
        }
        best.1
    }

    /// Spawns a full squad of `squad_type` at `owner`'s base, in a line facing
    /// the map centre, on the spot [`Self::spawn_spot`] picks.
    pub(crate) fn spawn_squad(
        &mut self,
        ctx: &mut Ctx,
        owner: PlayerId,
        squad_type: u16,
    ) -> SquadId {
        let id = SquadId(self.next_squad);
        self.next_squad += 1;
        let spot = self.spawn_spot(ctx.ents, ctx.terrain, owner, id);
        let ty = &self.setup.squad_types[squad_type as usize];
        let vehicle_type = ty.vehicle;
        let mid = FxVec2::from_int(self.setup.map.size_m / 2, self.setup.map.size_m / 2);
        let mut facing = (mid - spot).normalize_or_zero();
        if facing == FxVec2::ZERO {
            facing = FxVec2::new(Fx::ONE, Fx::ZERO);
        }
        if let Some(vt) = vehicle_type {
            self.spawn_vehicle(ctx, owner, id, squad_type, vt, spot, facing);
            return id;
        }
        self.spawn_men(ctx, owner, id, squad_type, spot, facing);
        id
    }

    /// Spawns soldier squad `id` of `squad_type` at `spot` in a line
    /// facing `facing` (S9: also a gun's crew, `crate::guns`).
    pub(crate) fn spawn_men(
        &mut self,
        ctx: &mut Ctx,
        owner: PlayerId,
        id: SquadId,
        squad_type: u16,
        spot: FxVec2,
        facing: FxVec2,
    ) {
        let ty = &self.setup.squad_types[squad_type as usize];
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
            // S4: men spawn by `[[members]]` slot, each with his slot's
            // weapon and items; the first man leads.
            let slot = ty.slot_of(k);
            let wk = ty.members[slot as usize].weapon;
            ctx.ents.combat[i] = combat::spawn_combat(
                ty.hp,
                &self.setup.weapons[wk as usize],
                &self.setup.rules.combat.body,
            );
            ctx.ents.kit[i] = Kit {
                items: ty.full_kit(slot),
                slot,
                weapon: wk,
                ..Kit::default()
            };
            // S5a: his rounds, a magazine of them in the weapon.
            crate::supply::spawn_load(
                &self.setup,
                ty,
                &mut ctx.ents.kit[i],
                &mut ctx.ents.combat[i],
            );
            ctx.hash.mark_entity_chunk(sid.chunk());
            members.push(sid);
        }
        // AT teams: the first man with the launcher carries it (S3).
        let w = &self.setup.weapons[ty.weapon as usize];
        let launcher = (w.class == WeaponClass::Launcher).then(|| Launcher {
            carrier: members
                .iter()
                .copied()
                .find(|m| ctx.ents.kit[m.index()].weapon == ty.weapon),
            rounds: w.rounds_carried.min(u16::MAX as u32) as u16,
            ..Launcher::default()
        });
        // S4: squads with a single-shot AT item (Panzerfaust).
        let pzf = SingleShot::for_type(ty);
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
                ranging: Vec::new(),
                focus: None,
                launcher,
                drops: Vec::new(),
                pzf,
            },
        );
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
            self.vderived.block_of(v),
            &geo,
            &mut vs,
            &mut ctx.ents.transform[i],
        );
        vs.progress_at = spot;
        let served = vs.served.is_some();
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
                ranging: Vec::new(),
                focus: None,
                launcher: None,
                drops: Vec::new(),
                pzf: None,
            },
        );
        if served {
            self.spawn_gun_crew(ctx, owner, vid);
        }
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
        // S10a: the wire layer, only when wire stands (S10a-ai: from the
        // derived cell index, rebuilt only when the wire changed).
        let wired = !self.fort.wire.is_empty() && !self.fort_off;
        if wired {
            self.fort
                .refresh_wire_index(&self.setup.forts, terrain.dim());
        }
        let wire_nodes = if wired {
            self.fort.path_nodes(&self.setup.forts, terrain.dim())
        } else {
            std::borrow::Cow::Borrowed(&[][..])
        };
        let (fort, forts) = (&self.fort, &self.setup.forts);
        let crosses = |a: FxVec2, b: FxVec2| fort.step_in_wire(forts, a, b);
        let wire = (!wire_nodes.is_empty()).then(|| crate::path::WireCost {
            nodes: &wire_nodes,
            cost_pct: self
                .setup
                .rules
                .fortify
                .as_ref()
                .map_or(100, |f| f.wire_path_cost_pct),
            crosses: &crosses,
        });
        let foot_path = |from: FxVec2, to: FxVec2, scratch: &mut PathScratch| match (ditches, wire)
        {
            (_, Some(_)) => crate::path::find_foot_path(
                terrain, from, to, &cost, ditches, wire, max_exp, scratch,
            ),
            (Some(d), None) => find_infantry_path(terrain, from, to, &cost, d, max_exp, scratch),
            (None, None) => find_path(terrain, from, to, &cost, max_exp, scratch),
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
                        block: self
                            .vderived
                            .block_of(&self.setup.vehicles[self.vehicles[&v].type_idx as usize]),
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
        // Id counters are bumped unchecked; one near the top would overflow.
        if [
            self.next_squad,
            self.next_unit_dig,
            self.next_thrown,
            self.next_smoke,
            self.next_mission,
            self.supply6.next_structure,
            self.supply6.next_crate,
            self.supply6.next_cookoff,
        ]
        .iter()
        .any(|&n| n > crate::bounds::MAX_ID_COUNTER)
        {
            return Err("an id counter is exhausted");
        }
        // S6.
        self.supply6.validate(s, ents, &self.squads)?;
        // S10a.
        self.fort.validate(s, ents, &self.squads, tick)?;
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
            // Counters are bumped unchecked (the save fuzz).
            use crate::bounds::counters_ok;
            if !(counters_ok(&[pl.kills, pl.losses, pl.rockets])
                && counters_ok(&pl.point_ticks)
                && counters_ok(&pl.kills_by)
                && counters_ok(&pl.crew)
                && counters_ok(&pl.s4)
                && counters_ok(&pl.ammo)
                && counters_ok(&pl.fm)
                && counters_ok(&pl.s7)
                && counters_ok(&pl.s8a)
                && counters_ok(&pl.s9))
            {
                return Err("a player counter is out of range");
            }
            // S5a: the prepaid credit stays below one point plus a round.
            if pl.ammo.credit as u64 >= crate::supply::MICRO + crate::supply::dearest_round(s)
                || pl.ammo.paused_at > tick
            {
                return Err("bad ammunition account");
            }
            // S5b: the knowledge map.
            if !crate::fire_mission::intel_ok(
                &pl.intel,
                s.rules.fire_mission.as_ref(),
                s.map.size_m,
                tick,
            ) {
                return Err("bad intel map");
            }
            if self.ai[p].is_some() != s.players[p].is_ai {
                return Err("AI state disagrees with the setup");
            }
            if let Some(ai) = &self.ai[p] {
                ai.validate(self, PlayerId(p as u8), tick, ents.len())?;
            }
        }
        // Re-call cooldowns (S3b): one per squad type, never further ahead
        // than the longest cooldown a type can have.
        let longest = s
            .squad_types
            .iter()
            .map(|t| t.recall_cooldown_ticks.max(t.initial_cooldown_ticks) as u64)
            .max()
            .unwrap_or(0);
        for pl in &self.players {
            if pl.s7.kills_by_vehicle.len() != s.vehicles.len()
                || pl.s7.kos_by_vehicle.len() != s.vehicles.len()
            {
                return Err("a player's per-vehicle counters have the wrong length");
            }
            if pl.ready_at.len() != s.squad_types.len()
                || pl
                    .ready_at
                    .iter()
                    .any(|&t| t > tick.saturating_add(longest))
            {
                return Err("bad call-in cooldowns");
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
                    || matches!(
                        sq.order,
                        Order::Dig { .. } | Order::FireMission { .. } | Order::Suppress { .. }
                    )
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
            if let Order::Suppress { line } = &sq.order
                && !self.suppress_ok(line)
            {
                return Err("suppress order is out of range");
            }
            // S5b: the fire mission and the ranging memory.
            if !self.mission_ok(sq, ents, tick) {
                return Err("fire mission state is inconsistent");
            }
            let ty = &s.squad_types[sq.type_idx as usize];
            let w = &s.weapons[ty.weapon as usize];
            let launcher_ok = match &sq.launcher {
                None => w.class != WeaponClass::Launcher,
                Some(l) => {
                    w.class == WeaponClass::Launcher
                        && l.carrier.is_none_or(|c| {
                            sq.members.contains(&c) && ents.kit[c.index()].weapon == ty.weapon
                        })
                        && l.rounds as u32 <= w.rounds_carried
                        && (l.resupply as u32)
                            < s.rules.items.resupply_ticks.max(1) + s.rules.supply.is_some() as u32
                        && l.attack.is_none_or(|t| {
                            ents.kind.get(t.index()) == Some(&EntityKind::Vehicle)
                                && ents.owner[t.index()] != sq.owner
                        })
                }
            };
            if !launcher_ok {
                return Err("launcher state is inconsistent");
            }
            // S4: crew weapons waiting to be picked up, and the squad's
            // single-shot AT state.
            let drops_ok = sq.drops.len() <= MAX_DROPS
                && sq.drops.iter().all(|d| {
                    s.weapons
                        .get(d.weapon as usize)
                        .is_some_and(|w| {
                            w.crew
                                && d.ticks as u32 <= w.handover_ticks.max(1)
                                && d.mag as u32 <= w.magazine
                        })
                        // S4 review L3: a weapon the squad's own men carry.
                        && ty.members.iter().any(|m| m.weapon == d.weapon)
                        && d.at.x >= Fx::ZERO
                        && d.at.y >= Fx::ZERO
                        && d.at.x <= Fx::from_int(s.map.size_m)
                        && d.at.y <= Fx::from_int(s.map.size_m)
                });
            if !drops_ok || (ty.vehicle.is_some() && !sq.drops.is_empty()) {
                return Err("crew weapon drops are inconsistent");
            }
            let live_enemy_vehicle = |t: SimId| {
                ents.kind.get(t.index()) == Some(&EntityKind::Vehicle)
                    && ents.owner[t.index()] != sq.owner
            };
            let pzf_ok = match &sq.pzf {
                None => ty.squad_items(ItemKind::SingleShotAt) == 0 || ty.vehicle.is_some(),
                Some(p) => {
                    let aim_max = ty
                        .kit(&s.items, ItemKind::SingleShotAt)
                        .map_or(0, |(_, it)| it.windup_ticks.max(1));
                    ty.squad_items(ItemKind::SingleShotAt) > 0
                        && p.man.is_some() == p.target.is_some()
                        && p.man.is_none_or(|m| sq.members.contains(&m))
                        && p.target.is_none_or(live_enemy_vehicle)
                        && p.attack.is_none_or(live_enemy_vehicle)
                        && p.aim as u32 <= aim_max
                        && p.gap as u32 <= s.rules.items.single_shot_gap_ticks
                        && p.search as u32 <= s.tick_hz.max(1)
                }
            };
            if !pzf_ok {
                return Err("single-shot AT state is inconsistent");
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
                    || (!soldier
                        && (u.stance != Default::default() || u.fire != Default::default()))
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
                    || matches!(u.order, UnitOrder::Suppress { line } if !self.suppress_ok(&line))
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
            let vehicle = matches!(
                kind,
                EntityKind::Vehicle | EntityKind::Wreck | EntityKind::Abandoned
            );
            let bad = c.seen_by >> np != 0
                || c.target.is_some_and(|t| t.index() >= ents.len())
                || c.suppression > 100_000
                || (c.ducked && (kind != EntityKind::Soldier || !c.in_trench || c.pinned))
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
        self.validate_riders(ents)?;
        self.validate_guns(ents)?;
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

    /// S5b load check of a squad's fire mission and ranging memory.
    fn mission_ok(&self, sq: &Squad, ents: &Entities, tick: u64) -> bool {
        let s = &self.setup;
        let size = Fx::from_int(s.map.size_m);
        let on_map = |p: FxVec2| p.x >= Fx::ZERO && p.y >= Fx::ZERO && p.x <= size && p.y <= size;
        let ty = &s.squad_types[sq.type_idx as usize];
        let fm = s.rules.fire_mission.as_ref();
        let cells_ok = match fm {
            None => sq.ranging.is_empty(),
            Some(fm) => {
                sq.ranging.len() <= fm.ranging_max_cells as usize
                    && (sq.ranging.is_empty() || ty.has(Capability::FireMission))
                    && sq.ranging.iter().all(|c| {
                        c.at <= tick
                            && on_map(c.target)
                            && on_map(c.from)
                            && c.offset.length() <= crate::fire_mission::max_error(fm, s.map.size_m)
                    })
            }
        };
        if !cells_ok {
            return false;
        }
        let Order::FireMission { target, rounds, m } = &sq.order else {
            return true;
        };
        if !ty.has(Capability::FireMission) || !on_map(*target) || *rounds > MAX_MISSION_ROUNDS {
            return false;
        }
        let Some(fm) = fm else {
            return *m == Mission::default();
        };
        let w = &s.weapons[ty.weapon as usize];
        let charges = w.indirect.as_ref().map_or(1, |i| i.charges.len());
        let hz = s.tick_hz as u64;
        let prep = fm.fire_direction_s.max(fm.direct_s).max(fm.adjust_wait_s) as u64 * hz;
        // The burst deadline is the flight plus 4 s (`combat.rs`); the
        // weapon's longest possible flight bounds it (Review L1).
        let flight_s = w
            .indirect
            .as_ref()
            .map_or(0, crate::indirect::flight_bound_s);
        let burst_window = (flight_s + 4).max(60) * hz;
        (m.mode as usize) < mode::COUNT
            && m.phase <= phase::EFFECT
            && m.id != 0
            && m.id < self.next_mission
            && m.order_tick <= tick
            && m.ready_at <= tick + prep
            && m.burst_by <= tick + burst_window
            && m.adjust as u32 <= fm.adjust_rounds_max
            && (m.charge as usize) < charges
            && m.offset.length() <= crate::fire_mission::max_error(fm, s.map.size_m)
            && m.err0 <= crate::fire_mission::max_error(fm, s.map.size_m)
            && m.observer.is_none_or(|o| o.index() < ents.len())
    }

    /// S3b: a saved suppress line lies on the map and is no longer than
    /// the rules allow (a metre of slack for the pull-in's rounding).
    pub(crate) fn suppress_ok(&self, line: &[FxVec2; 2]) -> bool {
        let map = Fx::from_int(self.setup.map.size_m);
        let max = Fx::from_int(self.setup.rules.combat.suppress.line_max_m as i32 + 1);
        line.iter()
            .all(|p| (Fx::ZERO..=map).contains(&p.x) && (Fx::ZERO..=map).contains(&p.y))
            && (line[1] - line[0]).length_sq_raw() <= FxVec2::new(max, Fx::ZERO).length_sq_raw()
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
            .filter(|k| {
                matches!(
                    k,
                    EntityKind::Vehicle | EntityKind::Wreck | EntityKind::Abandoned
                )
            })
            .count();
        if vehicle_entities != self.vehicles.len() {
            return Err("vehicle entities and vehicle states disagree");
        }
        let mut wrecks = 0u32;
        for (&id, vs) in &self.vehicles {
            vs.validate(s, size, tick)?;
            // S3b Fix: a vehicle's suppress line obeys the order's limits.
            if vs.suppress.is_some_and(|l| !self.suppress_ok(&l)) {
                return Err("a vehicle's suppress line is out of range");
            }
            let (wreck, abandoned) = match kind(id) {
                Some(EntityKind::Vehicle) => (false, false),
                Some(EntityKind::Wreck) => (true, false),
                Some(EntityKind::Abandoned) => (false, true),
                _ => return Err("a vehicle state names no vehicle"),
            };
            let i = id.index();
            let squadless = wreck || abandoned;
            if wreck != vs.wreck_tick.is_some()
                || abandoned != vs.abandoned.is_some()
                || (squadless && ents.squad[i].is_some())
                || (!squadless && ents.squad[i].is_none_or(|q| !self.squads.contains_key(&q)))
            {
                return Err("a vehicle's squad or wreck state is inconsistent");
            }
            // S3c: the crew squad link names a crew squad; a boarding squad
            // is re-crewing this vehicle.
            let crew_ok = vs.crew_squad.is_none_or(|q| {
                self.squads.get(&q).is_some_and(|sq| {
                    s.squad_types[sq.type_idx as usize].has(Capability::VehicleCrew)
                })
            }) && vs.board.is_none_or(|(q, _)| {
                self.squads
                    .get(&q)
                    .is_some_and(|sq| sq.order == Order::Recrew { vehicle: id })
            });
            if !crew_ok {
                return Err("a vehicle's crew squad or boarding squad is inconsistent");
            }
            wrecks += wreck as u32;
            let enemy = |t: SimId| ents.owner[t.index()] != ents.owner[i];
            // S3c: an abandoned vehicle (any side's) only by order.
            let ordered_abandoned =
                |t: SimId| kind(t) == Some(EntityKind::Abandoned) && vs.attack == Some(t);
            let target_ok = vs.target.is_none_or(|t| {
                (matches!(kind(t), Some(EntityKind::Vehicle | EntityKind::Soldier)) && enemy(t))
                    || ordered_abandoned(t)
            }) && vs.attack.is_none_or(|t| {
                (kind(t) == Some(EntityKind::Vehicle) && enemy(t))
                    || kind(t) == Some(EntityKind::Abandoned)
            }) && vs.mgs.iter().all(|m| {
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
        && job.front.unsigned_abs() == 1
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
/// S8a: the infantry squad a squad-level command addresses, if any.
fn command_squad(kind: &CommandKind) -> Option<SquadId> {
    match kind {
        CommandKind::Move { squad, .. }
        | CommandKind::AttackMove { squad, .. }
        | CommandKind::Stop { squad }
        | CommandKind::Retreat { squad }
        | CommandKind::Dig { squad, .. }
        | CommandKind::FireMission { squad, .. }
        | CommandKind::Regroup { squad }
        | CommandKind::Recrew { squad, .. }
        | CommandKind::Build { squad, .. }
        | CommandKind::IssueCrate { squad, .. }
        | CommandKind::PlaceCrate { squad, .. }
        | CommandKind::PickUpCrate { squad, .. }
        | CommandKind::Fortify { squad, .. }
        | CommandKind::CutWire { squad, .. }
        | CommandKind::ClearMines { squad, .. }
        | CommandKind::Demolish { squad, .. } => Some(*squad),
        _ => None,
    }
}

pub(crate) fn hold_in_place(sq: &mut Squad, ents: &Entities, facing: FxVec2) {
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
