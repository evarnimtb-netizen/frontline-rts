//! Player commands: the only input to the sim (§5.1).
//!
//! [`crate::World::step`] sorts the commands for a tick by `(player, seq)`
//! before applying them (`Ord` is derived with those fields first, and the
//! payload breaks any remaining ties, so the order is total). Invalid commands
//! (dead unit, wrong owner) are ignored, identically on every client.

use frontline_content::setup::SupplyKind;
use frontline_content::{DigProfile, ItemKind};
use serde::{Deserialize, Serialize};

use crate::ecs::{PlayerId, SimId};
use crate::fx::FxVec2;
use crate::game::SquadId;
use crate::unit::{FireMode, StanceOrder};
use crate::vehicle::AmmoChoice;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct Command {
    pub player: PlayerId,
    /// Per-player sequence number.
    pub seq: u32,
    pub kind: CommandKind,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum CommandKind {
    /// Bench: walk to `target` (clamped to the map), then resume wandering.
    MoveSoldier { unit: SimId, target: FxVec2 },
    /// Bench: spawn a soldier owned by the issuing player at `pos`.
    Spawn { pos: FxVec2 },
    /// Bench: remove one of the issuing player's units.
    Despawn { unit: SimId },
    /// Move the squad to `target` (clamped to the map); it spreads in a line
    /// across the move direction on arrival.
    Move { squad: SquadId, target: FxVec2 },
    /// Like `Move`, engaging enemies on the way (engaging: combat stage).
    AttackMove { squad: SquadId, target: FxVec2 },
    /// Stop where the men stand.
    Stop { squad: SquadId },
    /// Back to the own base, faster.
    Retreat { squad: SquadId },
    /// Dig a trench along `line` (2..=16 points, at most the profile's
    /// `max_length`; squads that can dig only). The squad walks there,
    /// spreads along the line and digs it in passes. S3: `profile` picks
    /// the cross-section; the AT ditch is for engineers only.
    Dig {
        squad: SquadId,
        line: Vec<FxVec2>,
        profile: DigProfile,
    },
    /// S5b (was `Barrage`): mortar squads fire a mission of `rounds`
    /// (1..=50) at `target`. The sim picks the mode per round (direct,
    /// observed, map fire; `crate::fire_mission`); refused, with a
    /// `FireMissionEvent` in the snapshot, at ground the player has never
    /// seen, inside the minimum range, or without bombs.
    FireMission {
        squad: SquadId,
        target: FxVec2,
        rounds: u16,
    },
    /// Buy a squad (index into `MatchSetup::squad_types`, from the player's
    /// faction roster). S3b: it spawns at the base in the same tick and its
    /// type's re-call cooldown starts; refused (with a
    /// `game::CallInRefusal` in the snapshot) without the manpower, during
    /// the cooldown, over the pop cap, or for a vehicle before the unlock
    /// or at the limit. There is no queue.
    CallIn { squad_type: u16 },
    /// Give up; the last player standing wins.
    Surrender,
    // ---- Soldier-level orders (S1b, `crate::unit`). `units` holds 1 to
    // `unit::MAX_UNITS` ids; it is sorted and deduplicated when applied,
    // and dead, foreign and non-soldier ids are dropped (none left: the
    // command is ignored). A squad whose every living man is named gets the
    // matching squad order instead; the other men are detached. S3: in
    // `MoveUnits`, `AttackMoveUnits`, `StopUnits` and `RetreatUnits` an own
    // vehicle's id gives its squad of one the squad order.
    /// The men walk to `target` (clamped to the map) and spread in a small
    /// line around it.
    MoveUnits { units: Vec<SimId>, target: FxVec2 },
    /// Like `MoveUnits`, engaging enemies on the way.
    AttackMoveUnits { units: Vec<SimId>, target: FxVec2 },
    /// The men stop where they stand.
    StopUnits { units: Vec<SimId> },
    /// Only these men fall back to the own base.
    RetreatUnits { units: Vec<SimId> },
    /// These men dig a trench along `line` (as `Dig`), split among them.
    /// Men of squads that cannot dig (that profile) are dropped. Naming
    /// exactly one whole squad gives that squad a `Dig` order.
    DigUnits {
        units: Vec<SimId>,
        line: Vec<FxVec2>,
        profile: DigProfile,
    },
    /// Stance order for the men. Not an order to go anywhere: it neither
    /// detaches nor regroups anyone.
    SetStance {
        units: Vec<SimId>,
        stance: StanceOrder,
    },
    /// The squad's detached men rejoin it (its order is kept).
    Regroup { squad: SquadId },
    // ---- Thrown items (S1b abilities, `crate::items`). `units` as above;
    // there is no squad-level throw (a whole squad sends its men).
    /// The eligible man nearest `target` (ties: lowest id) throws one
    /// `item` there: he carries it, is not pinned or already throwing, and
    /// `target` is within his item's range. Nobody eligible: ignored. The
    /// thrower is detached and stays where he stands.
    Throw {
        units: Vec<SimId>,
        item: ItemKind,
        target: FxVec2,
    },
    /// Every eligible man throws one `item` at `target`, each with his own
    /// scatter.
    ThrowEach {
        units: Vec<SimId>,
        item: ItemKind,
        target: FxVec2,
    },
    // ---- Vehicle orders (S3). `units` as above; own vehicle ids are
    // accepted (each stands for its squad of one), soldiers are handled as
    // each command says.
    /// Vehicles back up to `target` (clamped), the front kept toward the
    /// enemy; soldiers in the set get a `MoveUnits`.
    ReverseUnits { units: Vec<SimId>, target: FxVec2 },
    /// Vehicles stop and turn the hull front toward `toward`, then hold.
    /// Soldiers are dropped.
    FaceUnits { units: Vec<SimId>, toward: FxVec2 },
    /// `target` must be a live enemy vehicle the player sees. Vehicles and
    /// launcher gunners in the set focus it while it is seen; vehicles
    /// without range or a line of fire move toward it until they have
    /// both. Other men ignore it.
    AttackUnit { units: Vec<SimId>, target: SimId },
    /// The main-gun round the vehicles load: `Auto`, `Ap` or `He`.
    SetAmmo { units: Vec<SimId>, ammo: AmmoChoice },
    /// The vehicles' crews repair the worst repairable component in place.
    RepairUnits { units: Vec<SimId> },
    /// The soldiers escort own vehicle `vehicle` in single file behind it
    /// (a whole squad as a squad order, other men one by one, detached).
    /// Vehicle ids in `units` are dropped.
    FollowUnits { units: Vec<SimId>, vehicle: SimId },
    /// S3b area fire (`crate::orders` "Suppress"): the men and vehicles
    /// hold and fire at points on `line` (1 or 2 points on the map; a
    /// second point further than the rules' `combat.suppress.line_max_m`
    /// from the first is pulled in) without a spotted target. Squads named
    /// whole get the squad order `Order::Suppress`, other men a detached
    /// `UnitOrder::Suppress`, vehicles `VehicleState::suppress`; mortar
    /// men are dropped (they keep `FireMission`). Lasts until another order.
    SuppressUnits {
        units: Vec<SimId>,
        line: Vec<FxVec2>,
    },
    // ---- S3c bail-out (`crate::bail`).
    /// Infantry squad `squad` walks to abandoned vehicle `vehicle` and
    /// boards it (a crew squad, or with the rules' `any_infantry` any
    /// infantry but an AT team; an enemy's with `capture`). Refused (a
    /// `RecrewRefusal` in the snapshot) without enough men, a free vehicle
    /// slot or pop.
    Recrew { squad: SquadId, vehicle: SimId },
    /// The crew of the player's vehicle squad `squad` bail out (vehicles
    /// with a crew squad only).
    Abandon { squad: SquadId },
    /// S6: build supply def `def` (an HQ or dump of the player's faction,
    /// index into `MatchSetup::stocks`) centred at `pos`, its long axis
    /// along `facing`: the squad (a `built_by` tag, able to dig) digs its
    /// pit and it stands when the pit is done. Paid on acceptance.
    /// Refused (a `SupplyEvent` in the snapshot) at the cap, during the
    /// HQ's `rebuild_s`, in another player's base or without manpower.
    Build {
        squad: SquadId,
        def: u16,
        pos: FxVec2,
        facing: FxVec2,
    },
    /// S6: a man of the squad in the own base takes a crate of `kind`
    /// (paid now: kg x the kind's price). One per `base_crate_issue_s`
    /// per player; `crate_cap` / `shell_crate_cap` live crates.
    IssueCrate { squad: SquadId, kind: SupplyKind },
    /// S6: the squad walks to `pos` and sets its carried crates down there.
    PlaceCrate { squad: SquadId, pos: FxVec2 },
    /// S6: the squad walks to own set-down crate `crate_id`; its first man
    /// within `pickup_m` takes it.
    PickUpCrate { squad: SquadId, crate_id: u32 },
    // ---- S5a fire discipline (`crate::combat`).
    /// The men's fire mode (`units` as above; vehicles are dropped: their
    /// MGs keep discipline). Not an order to go anywhere.
    SetFire { units: Vec<SimId>, mode: FireMode },
    // ---- S8a transport (`crate::transport`). `squads` and `vehicles`
    // hold 1 to `unit::MAX_UNITS` ids; they are sorted and deduplicated.
    /// The squads walk to own transport `vehicle` and board it (a squad
    /// that does not fit the free slots is refused).
    Embark {
        squads: Vec<SquadId>,
        vehicle: SimId,
    },
    /// The vehicles stop and their passengers get out, by the door or
    /// (`over_sides`, where the def allows) over the sides.
    Disembark {
        vehicles: Vec<SimId>,
        over_sides: bool,
    },
    /// S8b: own supply truck `vehicle` drives to `point` and sets down
    /// `count` crates of `kind` from its cargo there (one per `drop_s`).
    /// Refused without cargo for one crate or at the crate caps. Any
    /// other order to the truck ends it.
    DropCrates {
        vehicle: SimId,
        point: FxVec2,
        count: u8,
        kind: SupplyKind,
    },
    // ---- S9 guns (`crate::guns`). `guns` holds 1 to `unit::MAX_UNITS` of
    // the player's served guns (sorted and deduplicated when applied;
    // others are dropped).
    /// Emplace limbered guns facing the point `facing`: the crew work
    /// `emplace_s`, the gun cannot move and fires when done.
    Emplace { guns: Vec<SimId>, facing: FxVec2 },
    /// Pack emplaced guns (limbered after `pack_s`).
    PackGun { guns: Vec<SimId> },
    /// The crew push limbered guns to `point` (on wheels, speed by men).
    PushGun { guns: Vec<SimId>, point: FxVec2 },
    /// Hitch the limbered gun nearest tow `tow` to it.
    Hitch { guns: Vec<SimId>, tow: SimId },
    /// Unhitch: `hitch_s`, the crew get down, the gun limbered.
    Unhitch { guns: Vec<SimId> },
    /// Hold fire until a target is within `range_m` (0: off; capped).
    SetAmbush { guns: Vec<SimId>, range_m: u16 },
    // ---- S10a fortifications (`crate::fort`).
    /// Build blueprint `fort` (index into `MatchSetup::forts`): point and
    /// over_hq `line = [pos, toward]`; line blueprints 2..=16 points, at
    /// most `max_length_m`. Refused (a `FortEvent` in the snapshot) without
    /// the capability, at the cap, at a bad spot (spacing, slope, off the
    /// map, an enemy base, no own built unroofed HQ within 5 m).
    Fortify {
        squad: SquadId,
        fort: u16,
        line: Vec<FxVec2>,
    },
    /// Load `kg` of construction material in the own base, paid there: a
    /// `build_structure` or `place_wire` squad carries it (up to
    /// `carry_kg_per_man` per living man), an own truck's squad fills
    /// `VehicleState::material_g`.
    LoadMaterials { squad: SquadId, kg: u16 },
    /// A `clear_wire` squad cuts the nearest standing section within
    /// `cut_reach_m` of `at`.
    CutWire { squad: SquadId, at: FxVec2 },
    /// The nearest man carrying a Bangalore in a `breach` squad lays it
    /// toward `target`.
    PlaceBangalore { units: Vec<SimId>, target: FxVec2 },
    // ---- S10b mines and obstacles (`Demolish` refused `NotAllowed` until Build D).
    /// A `clear_mines` squad sweeps the lane `a`..`b` (at most
    /// `lane_max_m`), lifting the mines it finds.
    ClearMines {
        squad: SquadId,
        a: FxVec2,
        b: FxVec2,
    },
    /// The nearest man with a satchel in a `demolish` squad lays it against
    /// the nearest piece within `demolish_reach_m` of `at`.
    Demolish { squad: SquadId, at: FxVec2 },
    // ---- S12 wounds and medics (`crate::wounds`). Declared with their wire
    // encoding in S12a Build A; the sim refuses each (`FortRefusal::
    // NotAllowed`, "not built yet") until Build B.
    /// The nearest of `units` tends `target`: a medic stabilises a downed
    /// man, anyone bandages a wounded man.
    Tend { units: Vec<SimId>, target: SimId },
    /// One or two of `units` carry the downed man `target` to `to`.
    Carry {
        units: Vec<SimId>,
        target: SimId,
        to: FxVec2,
    },
    /// Evacuate `units` (wounded men at a staffed aid post or in base).
    Evacuate { units: Vec<SimId> },
    /// Fill infantry squad `squad` to its card size from base.
    Reinforce { squad: SquadId },
    // ---- S14 assault kit (`crate::flame`). Declared with their wire
    // encoding in S14a Build A; the sim refuses each (`FortRefusal::
    // NotAllowed`, "not built yet") until Build B (`ClearTrench`) and
    // Build C (`Strike`).
    /// C1: squad `squad` clears the trench at `at` (a dug cell within 8 m),
    /// step by step with grenades and flame.
    ClearTrench { squad: SquadId, at: FxVec2 },
    /// M2: each of `units` strikes the nearest enemy man in reach at once.
    Strike { units: Vec<SimId> },
}
