//! Vehicle and ammunition defs (S3, `docs/slices/S3-armour.md`):
//! `defs/vehicles/*.toml` and `defs/ammo/*.toml`, their per-file checks,
//! and their lowering into [`VehicleSetup`] and [`AmmoSetup`].
//!
//! S3 models a vehicle as two oriented boxes (hull and turret). Each box
//! face is an armour plate from the def's `[armor]` table; its ballistic
//! normal is tilted by the authored `slope_deg`. Crew seats and internal
//! components are small boxes in the hull or turret frame; tracks and the
//! gun barrel are anchor points. Frames, all in centimetres in the def:
//!
//! * **hull frame:** x forward, y left, z up, origin on the ground under the
//!   hull's centre;
//! * **turret frame:** the hull frame moved to the turret `pivot` and turned
//!   by the turret's yaw about the hull's up axis.
//!
//! Angles stay integer degrees here: this crate has no trigonometry (§7.9),
//! so the sim builds its armour tables (plate normals, cosines) from them.
//! Units are in the field names, as for every def.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::defs::{ItemHeDef, check_id};
use crate::issue::Issue;
use crate::setup::{Fixed, ItemHeSetup};

// ------------------------------------------------------------- enums --

/// S7 (`docs/slices/S7-casemate-td.md` 1): the four plate groups of one
/// armour box. A box face maps to its group; both sides share `Side`, and
/// the bottom face reads the `Top` plate (as in S3). A vehicle's **zone id**
/// is `box index × 4 + group` and its name `<box id>_<group>`, so a vehicle
/// whose boxes are `hull` then `turret` keeps the S3 ids and names
/// (`hull_front` = 0 ... `turret_top` = 7; owner question S7-9).
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum PlateGroup {
    Front = 0,
    Side = 1,
    Rear = 2,
    Top = 3,
}

/// Plate groups per armour box.
pub const PLATE_GROUPS: usize = 4;
/// S7: most armour boxes per vehicle (box 0 is the running-gear hull).
pub const MAX_ARMOUR_BOXES: usize = 4;
/// Most zone ids per vehicle (`MAX_ARMOUR_BOXES × PLATE_GROUPS`).
pub const MAX_ZONES: usize = MAX_ARMOUR_BOXES * PLATE_GROUPS;

impl PlateGroup {
    pub const ALL: [PlateGroup; PLATE_GROUPS] = [
        PlateGroup::Front,
        PlateGroup::Side,
        PlateGroup::Rear,
        PlateGroup::Top,
    ];

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The def key, e.g. `front`.
    pub const fn name(self) -> &'static str {
        match self {
            PlateGroup::Front => "front",
            PlateGroup::Side => "side",
            PlateGroup::Rear => "rear",
            PlateGroup::Top => "top",
        }
    }

    pub fn from_name(s: &str) -> Option<PlateGroup> {
        PlateGroup::ALL.into_iter().find(|z| z.name() == s)
    }

    /// Top plates are flat (`slope_deg` must be 0).
    pub const fn is_top(self) -> bool {
        matches!(self, PlateGroup::Top)
    }

    /// The group of zone id `z`.
    pub const fn of_zone(z: u8) -> PlateGroup {
        match z % 4 {
            0 => PlateGroup::Front,
            1 => PlateGroup::Side,
            2 => PlateGroup::Rear,
            _ => PlateGroup::Top,
        }
    }

    /// Zone id of this group on armour box `b`.
    #[inline]
    pub const fn zone(self, b: usize) -> u8 {
        (b * PLATE_GROUPS + self as usize) as u8
    }
}

/// A crew seat. The order is the snapshot's and the HUD's pip order
/// (D G C L R).
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum SeatRole {
    Driver = 0,
    Gunner = 1,
    Commander = 2,
    Loader = 3,
    /// Radio operator, who fires the hull MG.
    RadioMg = 4,
}

/// Number of [`SeatRole`]s (most seats in one vehicle).
pub const SEAT_ROLES: usize = 5;

impl SeatRole {
    pub const ALL: [SeatRole; SEAT_ROLES] = [
        SeatRole::Driver,
        SeatRole::Gunner,
        SeatRole::Commander,
        SeatRole::Loader,
        SeatRole::RadioMg,
    ];

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn name(self) -> &'static str {
        match self {
            SeatRole::Driver => "driver",
            SeatRole::Gunner => "gunner",
            SeatRole::Commander => "commander",
            SeatRole::Loader => "loader",
            SeatRole::RadioMg => "radio_mg",
        }
    }
}

/// A vehicle component. The order is the snapshot's
/// (`VehicleSnapshot::components`).
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum ComponentKind {
    Engine = 0,
    TrackLeft = 1,
    TrackRight = 2,
    /// Main gun: a breech box inside the turret; S3b: plus the barrel box
    /// of the main-gun mount (`MountDef::barrel`), which a round must strike.
    Gun = 3,
    TurretRing = 4,
    /// S3b: may have several boxes (the stowage bins), one HP pool.
    AmmoRack = 5,
    /// S3b: the gearbox and final drives (damaged: half speed; destroyed:
    /// immobilised).
    Transmission = 6,
    /// S8b (6.8): a wheeled vehicle's front axles (anchors only, like a
    /// track); at 0 HP the vehicle cannot move.
    WheelsFront = 7,
    /// S8b (6.8): its rear axles.
    WheelsRear = 8,
    /// S8b (section 7): a supply truck's load (a box; detonates like a
    /// rack while it holds enough shells or explosives).
    Cargo = 9,
}

/// Number of [`ComponentKind`]s (indices of `VehicleSetup::components`; a
/// part a vehicle does not have has 0 HP there).
pub const COMPONENTS: usize = 10;

impl ComponentKind {
    pub const ALL: [ComponentKind; COMPONENTS] = [
        ComponentKind::Engine,
        ComponentKind::TrackLeft,
        ComponentKind::TrackRight,
        ComponentKind::Gun,
        ComponentKind::TurretRing,
        ComponentKind::AmmoRack,
        ComponentKind::Transmission,
        ComponentKind::WheelsFront,
        ComponentKind::WheelsRear,
        ComponentKind::Cargo,
    ];

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn name(self) -> &'static str {
        match self {
            ComponentKind::Engine => "engine",
            ComponentKind::TrackLeft => "track_left",
            ComponentKind::TrackRight => "track_right",
            ComponentKind::Gun => "gun",
            ComponentKind::TurretRing => "turret_ring",
            ComponentKind::AmmoRack => "ammo_rack",
            ComponentKind::Transmission => "transmission",
            ComponentKind::WheelsFront => "wheels_front",
            ComponentKind::WheelsRear => "wheels_rear",
            ComponentKind::Cargo => "cargo",
        }
    }

    /// Needs at least one box (internal); running gear is anchors only.
    pub const fn has_box(self) -> bool {
        !self.is_running_gear()
    }

    pub const fn is_track(self) -> bool {
        matches!(self, ComponentKind::TrackLeft | ComponentKind::TrackRight)
    }

    /// S8b: a track or a wheel axle (anchors only; a blast under it or a
    /// hit in the band damages it; at 0 HP the vehicle cannot move).
    pub const fn is_running_gear(self) -> bool {
        matches!(
            self,
            ComponentKind::TrackLeft
                | ComponentKind::TrackRight
                | ComponentKind::WheelsFront
                | ComponentKind::WheelsRear
        )
    }
}

/// Which frame a part's coordinates are in.
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum Frame {
    #[default]
    Hull = 0,
    Turret = 1,
}

/// Where a vehicle weapon sits.
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum MountKind {
    /// The main gun in the turret (exactly one).
    TurretMain = 0,
    /// Coaxial MG: follows the turret and the gun.
    Coax = 1,
    /// Hull MG: fixed in the hull, within `arc_deg` of straight ahead.
    Hull = 2,
    /// S7: the main gun in a casemate (no turret): it turns within
    /// `traverse_deg` of the hull's front about its `pivot`; outside that
    /// arc the driver turns the hull (`docs/slices/S7-casemate-td.md` 3).
    Casemate = 3,
    /// S8a: a pintle or ring MG (vehicles.md 6.7): it turns within
    /// `traverse_deg` of the hull's front about its `pivot` (hull frame) at
    /// `traverse_deg_s`; an exposed seat fires it.
    Pintle = 4,
}

impl MountKind {
    /// The main gun (exactly one per vehicle): `turret_main` or `casemate`.
    pub const fn is_main(self) -> bool {
        matches!(self, MountKind::TurretMain | MountKind::Casemate)
    }
}

/// How a round defeats armour (§2.4).
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum AmmoMechanism {
    /// Solid armour-piercing shot.
    Ap = 0,
    /// Armour-piercing with an HE filler that bursts inside.
    Aphe = 1,
    /// High explosive: bursts on contact.
    He = 2,
    /// Shaped charge: penetration does not fall with range.
    Heat = 3,
}

impl AmmoMechanism {
    /// Rounds that pierce armour (a core and spall after penetration).
    pub const fn penetrator(self) -> bool {
        !matches!(self, AmmoMechanism::He)
    }

    /// Rounds that burst on contact (an `[he]` blast).
    pub const fn bursts(self) -> bool {
        matches!(self, AmmoMechanism::He | AmmoMechanism::Heat)
    }

    pub const fn name(self) -> &'static str {
        match self {
            AmmoMechanism::Ap => "ap",
            AmmoMechanism::Aphe => "aphe",
            AmmoMechanism::He => "he",
            AmmoMechanism::Heat => "heat",
        }
    }
}

/// What a vehicle is for (S7: assault guns and tank destroyers; the AI's
/// habits read it).
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum VehicleRole {
    Tank,
    AssaultGun,
    TankDestroyer,
    /// S8a: an infantry carrier (half-track): `[transport]`, no main gun;
    /// outside the rules' vehicle `limit`, unlocked at `transport_unlock_s`.
    Transport,
    /// S8b: an armoured car (optics, scouting); outside the vehicle limit.
    Recon,
    /// S8b: a supply truck (no weapons needed); outside the vehicle limit.
    Truck,
}

impl VehicleRole {
    pub const fn name(self) -> &'static str {
        match self {
            VehicleRole::Tank => "tank",
            VehicleRole::AssaultGun => "assault_gun",
            VehicleRole::TankDestroyer => "tank_destroyer",
            VehicleRole::Transport => "transport",
            VehicleRole::Recon => "recon",
            VehicleRole::Truck => "truck",
        }
    }

    /// S8b: outside the rules' vehicle `limit` (capped by its own card).
    pub const fn outside_limit(self) -> bool {
        matches!(
            self,
            VehicleRole::Transport | VehicleRole::Recon | VehicleRole::Truck
        )
    }
}

#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum MobilityKind {
    Tracked,
    /// S8a (vehicles.md 6.8): driven front wheels and rear tracks. Same
    /// drive as `tracked` (neutral pivot, S8a-6); the difference is data
    /// (bog chance, turn rate, speed).
    Halftrack,
    /// S8b (6.8): a bicycle model: a turning radius, no neutral pivot,
    /// any ground dug `ditch_block_dm` deep is a wall.
    Wheeled,
}

/// Armour material. Only rolled homogeneous armour exists in S3
/// (multiplier 1.0); the §5.7 material matrix comes with armour meshes.
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ArmourMaterial {
    #[default]
    Rha,
    /// S8a (S8a-4): face-hardened plate; its thickness counts as the rules'
    /// `[vehicles] face_hardened_pct` of itself against every round.
    FaceHardened,
}

// -------------------------------------------------------------- vehicle --

/// Most crew seats, components boxes and weapons per vehicle are fixed by
/// the enums; these cap the rest.
pub const MAX_MOUNTS: usize = 4;
pub const MAX_ANCHORS: usize = 4;
pub const MAX_STATIONS: usize = 4;
/// Most boxes one component may have (S3b: stowage bins).
pub const MAX_COMPONENT_BOXES: usize = 4;
/// Most inner boxes per vehicle: every crew seat box plus every component
/// box (the sim's damage pass keeps this many; S3b Fix, a performance cap).
/// S8a: 14 -> 24 (passenger slot boxes count from S8a Build B).
pub const MAX_INNER_BOXES: usize = 24;
/// S8a: most passenger slots of a transport.
pub const MAX_PASSENGERS: usize = 12;
/// S8a: most MG shields per vehicle.
pub const MAX_SHIELDS: usize = 2;
/// Most spall rays of a penetration: the sim traces the core plus at most
/// this many (S3b Fix: was 8 in validation while the sim kept 7).
pub const MAX_SPALL_RAYS: u32 = 7;
/// Most filler fragment rays of an APHE round (S3b).
pub const MAX_FILLER_RAYS: u32 = 16;
/// Most rounds a main gun carries: a penetrator, HE and a second
/// (special) penetrator (S3b, e.g. PzGr 40 APCR).
pub const MAX_MAIN_AMMO: usize = 3;
/// Steepest climb a vehicle def may allow: the 30° pitch clamp (tan 30° is
/// 0.577), so a slope it may climb never exceeds the clamp.
pub const MAX_VEHICLE_SLOPE_PCT: u32 = 57;

/// A vehicle (`defs/vehicles/*.toml`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VehicleDef {
    /// Namespaced id, e.g. `ww2:ger_pz4_h`.
    pub id: String,
    pub name: String,
    /// Short name for the feed, cards and markers ("Panzer IV"); default:
    /// `name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,
    pub faction: String,
    pub role: VehicleRole,
    /// Manpower cost of a call-in.
    pub cost: u32,
    /// Population it uses (soldier squads use their men).
    pub pop: u32,
    /// Seconds after a call-in before this vehicle can be called in again
    /// (S3b; the call-in spawns it at once at the base).
    pub recall_cooldown_s: u32,
    /// Seconds from the match start before its first call-in (S3b; the
    /// rules' vehicle unlock applies as well).
    #[serde(default)]
    pub initial_cooldown_s: u32,
    /// Sight range of its crew, in metres.
    pub sight_m: u32,
    /// How far it can be seen, in percent of the observer's sight (size).
    #[serde(default = "default_spot_size_pct")]
    pub spot_size_pct: u32,
    /// Height of the observer's eye (commander's hatch), cm above ground.
    #[serde(default = "default_eye_height_cm")]
    pub eye_height_cm: u32,
    /// Citations for the historical figures (§2.3); may be a placeholder.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
    pub mobility: MobilityDef,
    /// The running gear (track band) of box 0.
    pub hull: HullDef,
    /// S7: the turret, if any (none: a casemate main gun).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turret: Option<TurretDef>,
    /// S7: 1..=4 armour boxes, box 0 the running-gear hull (frame `hull`).
    /// A turret needs exactly one `turret`-frame box.
    pub boxes: Vec<ArmourBoxDef>,
    /// S7: the crew seats (each role at most once; driver and gunner
    /// required), in the snapshot's order.
    pub seats: Vec<SeatDef>,
    pub weapons: Vec<MountDef>,
    pub components: Vec<ComponentDef>,
    /// Direct-control stations (§5.12), unused until M5.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stations: Vec<StationDef>,
    /// S3c: the squad def (role `crew`, the vehicle's faction) its crew
    /// becomes when it bails out. None: the crew cannot bail out (they stay
    /// and die with the vehicle, as before S3c).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crew_squad: Option<String>,
    /// S3c: the hatches the crew leave by (needed with `crew_squad`; every
    /// seat in exactly one hatch's `seats`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hatches: Vec<HatchDef>,
    /// S5a (owner D3): the most rare rounds (ammo defs with `rare = true`:
    /// APCR, HVAP) the vehicle holds, all rare types together; at most the
    /// rules' `supply.rare_cap_max`. Default: `rare_cap_max` when the main
    /// gun lists a rare round, else 0. Set on every vehicle of a mod with a
    /// `[supply]` table.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rare_cap: Option<u32>,
    /// S8a (6.7): MG shields: armour only, no contents, no zone id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shields: Vec<ShieldDef>,
    /// S8a (6.6): an infantry carrier's door and timings (role
    /// `transport`); its seats are `passenger_slots`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<TransportDef>,
    /// S8a: one per passenger seat (hull frame); the box is the seated
    /// man's hit volume.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub passenger_slots: Vec<PassengerSlotDef>,
    /// S8b (6.9): stationary observation; absent: no effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optics: Option<OpticsDef>,
    /// S8b (section 7): a supply truck's stock; exactly on role `truck`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supply: Option<TruckSupplyDef>,
}

/// S8b (7.3): a supply truck: a stock in kg per kind, paid as it loads in
/// the own base, serving vehicles and men in radius and dropping crates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TruckSupplyDef {
    /// Most kg of each kind.
    pub cap_kg_total: u32,
    /// What it takes on while stopped in base, kg per kind name.
    pub load_kg: BTreeMap<String, u32>,
    /// Men are served within this (m).
    pub radius_m: u32,
    /// Vehicles rearm within this (m).
    pub vehicle_radius_m: u32,
    /// One standard main-gun round per this (s).
    pub round_interval_s: u32,
    /// The crate defs it drops (`DropCrates`), one per kind at most.
    pub crates: Vec<String>,
    /// The round whose bursts its detonating cargo throws.
    pub cookoff: String,
}

/// S8b (6.9): a vehicle stopped for `settle_s` sees `stationary_sight_pct`
/// percent of its `sight_m`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OpticsDef {
    /// Seconds stopped before the bonus applies.
    pub settle_s: u32,
    /// Percent of `sight_m` (100..=300).
    pub stationary_sight_pct: u32,
}

/// S8a (6.7): an MG shield: one plate all round a thin box. It follows a
/// pintle mount's yaw about the mount's pivot, or sits in the hull frame.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShieldDef {
    /// Index into `weapons` of the pintle mount it turns with (box in that
    /// mount's frame: from the pivot, x along the MG); omitted: hull frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mount: Option<usize>,
    #[serde(rename = "box")]
    pub bounds: BoxDef,
    /// Millimetres (RHA; at most one decimal).
    #[schemars(with = "f64")]
    pub thickness_mm: TenthsMm,
}

/// S8a (6.6): how men get in and out of an infantry carrier.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TransportDef {
    /// Passengers (equals the number of `passenger_slots`).
    pub capacity: u32,
    /// The door, hull frame, cm: men board and exit here.
    pub entry: [i32; 3],
    /// Time per man boarding by the door (ms).
    pub board_ms: u32,
    /// Time per man out of the door (ms).
    pub exit_ms: u32,
    /// Time per man over the sides (ms); omitted: no side exits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_over_sides_ms: Option<u32>,
    /// A man going over the side stands this much higher for his exit
    /// time (cm).
    #[serde(default)]
    pub exit_raise_cm: u32,
}

/// S8a: one passenger seat; the box (hull frame, cm) is the man's hit
/// volume while he rides.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PassengerSlotDef {
    #[serde(rename = "box")]
    pub bounds: BoxDef,
}

/// S3c: a crew hatch. The men of `seats` leave by it one after another, in
/// the listed order, `exit_s` each.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HatchDef {
    pub seats: Vec<SeatRole>,
    #[serde(default)]
    pub frame: Frame,
    /// The opening, cm in its frame (on or inside the frame's box).
    pub pos: [i32; 3],
    /// Time for one man to get out, in milliseconds (100..=30000).
    pub exit_ms: u32,
}

/// Most hatches per vehicle.
pub const MAX_HATCHES: usize = 8;

/// S8b: whether a def with these traits lacks part `k` (0 HP, no box).
fn component_absent(
    k: ComponentKind,
    turret: bool,
    main_gun: bool,
    mobility: MobilityKind,
    role: VehicleRole,
) -> bool {
    let wheeled = mobility == MobilityKind::Wheeled;
    match k {
        ComponentKind::TurretRing => !turret,
        ComponentKind::Gun | ComponentKind::AmmoRack => !main_gun,
        ComponentKind::TrackLeft | ComponentKind::TrackRight => wheeled,
        ComponentKind::WheelsFront | ComponentKind::WheelsRear => !wheeled,
        ComponentKind::Cargo => role != VehicleRole::Truck,
        ComponentKind::Engine | ComponentKind::Transmission => false,
    }
}

fn default_spot_size_pct() -> u32 {
    200
}
fn default_eye_height_cm() -> u32 {
    250
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MobilityDef {
    #[serde(rename = "type")]
    pub kind: MobilityKind,
    /// Top speed on open flat ground.
    pub speed_cm_s: u32,
    pub reverse_cm_s: u32,
    /// Hull turn rate (S8b: not used by, and omitted on, `wheeled`).
    #[serde(default)]
    pub turn_deg_s: u32,
    /// S8b: full-lock turning radius at the vehicle's origin, cm (required
    /// on `wheeled`; forbidden otherwise).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_radius_cm: Option<u32>,
    /// S8b: speed in woods in percent of `speed_cm_s` (default 50).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub woods_speed_pct: Option<u32>,
    /// S8b: percent of speed lost at `max_slope_pct`, linear (default 70).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slope_loss_pct: Option<u32>,
    pub accel_cm_s2: u32,
    pub brake_cm_s2: u32,
    /// Steepest slope it climbs; speed falls linearly to 30 % there.
    pub max_slope_pct: u32,
    /// Ground inside an AT-ditch footprint at least this deep blocks it;
    /// S8b: `wheeled`: any ground dug this deep, ditch or not.
    pub ditch_block_dm: u32,
    /// Chance to bog once per trench crossing.
    pub bog_pct: u32,
    /// A bogged vehicle frees itself after a rolled time in this range.
    pub unbog_s: [u32; 2],
    /// Main-gun spread while it moves, in percent.
    pub moving_spread_pct: u32,
}

/// An axis-aligned box in a frame, in centimetres: `center ± half`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoxDef {
    pub center: [i32; 3],
    pub half: [u32; 3],
}

impl BoxDef {
    fn lo(&self, a: usize) -> i64 {
        self.center[a] as i64 - self.half[a] as i64
    }
    fn hi(&self, a: usize) -> i64 {
        self.center[a] as i64 + self.half[a] as i64
    }
    /// Whether `inner` lies inside `self` (touching faces is inside).
    pub fn contains(&self, inner: &BoxDef) -> bool {
        (0..3).all(|a| inner.lo(a) >= self.lo(a) && inner.hi(a) <= self.hi(a))
    }
}

/// A part's box (crew seat, internal component) with its frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PartBoxDef {
    #[serde(default)]
    pub frame: Frame,
    pub center: [i32; 3],
    pub half: [u32; 3],
}

impl PartBoxDef {
    fn bounds(&self) -> BoxDef {
        BoxDef {
            center: self.center,
            half: self.half,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HullDef {
    /// The lower part of each side of box 0, in percent of the side's
    /// height, is running gear: AP and HEAT hits there damage that side's
    /// track.
    pub track_band_pct: u32,
    /// Extra armour a round meets going through the running gear.
    pub track_mm: u32,
}

/// S7: the turret (its armour is the one `turret`-frame box).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TurretDef {
    /// Turret pivot in the hull frame (on the hull roof).
    pub pivot: [i32; 3],
    /// Traverse rate (deg/s; S7: a hand-traversed turret is slow).
    pub traverse_deg_s: u32,
    /// Main gun elevation rate (deg/s).
    pub elevate_deg_s: u32,
}

/// S7: one armour box: a box in its frame with its own four plates. A
/// face listed in `open` has no plate (an open top).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArmourBoxDef {
    /// Unique per vehicle; zone names are `<id>_<group>`.
    pub id: String,
    /// `hull` or `turret` (the turret frame from the pivot).
    #[serde(default)]
    pub frame: Frame,
    #[serde(rename = "box")]
    pub bounds: BoxDef,
    /// The plates; a group in `open` has none.
    pub armor: BoxArmorDef,
    /// Plate groups with no plate (S7: `["top"]` for an open-topped turret).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open: Vec<PlateGroup>,
    /// Open boxes: the floor items rest on, cm in the box's frame (default:
    /// the box bottom).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor_cm: Option<i32>,
    /// S8b (6.5): open faces with a wire screen: thrown items bounce off
    /// and fall beside the vehicle; bullets and fragments pass.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grenade_screen: Vec<PlateGroup>,
}

/// S7: an armour box's plates by group (`front`, `side`, `rear`, `top`;
/// the bottom reads `top`). Every group not `open` needs one.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoxArmorDef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub front: Option<PlateDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<PlateDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rear: Option<PlateDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top: Option<PlateDef>,
}

impl BoxArmorDef {
    pub fn get(&self, g: PlateGroup) -> Option<&PlateDef> {
        match g {
            PlateGroup::Front => self.front.as_ref(),
            PlateGroup::Side => self.side.as_ref(),
            PlateGroup::Rear => self.rear.as_ref(),
            PlateGroup::Top => self.top.as_ref(),
        }
    }
}

/// S7: one crew seat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SeatDef {
    /// Unique per vehicle (the presenter's label, e.g. `asst_driver`).
    pub id: String,
    pub role: SeatRole,
    /// The man's box in its frame.
    #[serde(rename = "box")]
    pub bounds: PartBoxDef,
    /// The armour box he sits in (default `hull`, box 0): an open box
    /// exposes him (S7 Build B).
    #[serde(default, rename = "in", skip_serializing_if = "Option::is_none")]
    pub in_box: Option<String>,
}

/// A length in millimetres with at most one decimal (`63.5`), kept as
/// tenths so lowering never depends on float rounding beyond that one
/// decimal. Armour figures need the half millimetre.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct TenthsMm(pub u32);

impl TryFrom<f64> for TenthsMm {
    type Error = String;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn try_from(v: f64) -> Result<TenthsMm, String> {
        let t = (v * 10.0).round();
        if !(0.0..=1e7).contains(&t) || (v * 10.0 - t).abs() > 1e-6 {
            return Err(format!(
                "{v} mm: expected a non-negative length with at most one decimal"
            ));
        }
        Ok(TenthsMm(t as u32))
    }
}

impl From<TenthsMm> for f64 {
    fn from(t: TenthsMm) -> f64 {
        f64::from(t.0) / 10.0
    }
}

impl std::fmt::Display for TenthsMm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.is_multiple_of(10) {
            write!(f, "{}", self.0 / 10)
        } else {
            write!(f, "{}.{}", self.0 / 10, self.0 % 10)
        }
    }
}

/// One armour plate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlateDef {
    /// Millimetres, at most one decimal (`63.5`).
    #[schemars(with = "f64")]
    pub thickness_mm: TenthsMm,
    /// Slope from vertical (the plate leans back, its normal tilts up).
    /// The S3 stand-in for armour-mesh normals; top plates are flat.
    #[serde(default)]
    pub slope_deg: u32,
    #[serde(default)]
    pub material: ArmourMaterial,
}

/// A weapon on the vehicle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MountDef {
    /// Weapon id: a `cannon` for `turret_main` and `casemate`, an `mg` for
    /// the others.
    pub def: String,
    pub mount: MountKind,
    /// Muzzle point: from the turret pivot in the turret frame
    /// (`turret_main`, `coax`), from the gun's `pivot` in the gun frame
    /// (`casemate`), in the hull frame for `hull`.
    pub muzzle: [i32; 3],
    /// `casemate` (S7): the gun's pivot in the hull frame, cm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pivot: Option<[i32; 3]>,
    /// `casemate` (S7): the traverse arc `[min, max]` in degrees from the
    /// hull's front, positive to the left (counter-clockwise from above).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traverse_deg: Option<[i32; 2]>,
    /// `casemate` (S7): traverse and elevation rates, deg/s.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traverse_deg_s: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elevate_deg_s: Option<u32>,
    /// `turret_main`: gun depression and elevation limits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elevation_deg: Option<[i32; 2]>,
    /// `turret_main`: rounds carried: one armour-piercing round (ap, aphe
    /// or heat), at most one he round and, with the he round, at most one
    /// second armour-piercing round (S3b: a special round such as APCR; the
    /// first penetrator listed is the standard one).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ammo: Vec<AmmoLoadDef>,
    /// `turret_main` (S3b): the exposed barrel, a box from the muzzle back
    /// `length_cm` along the gun, `radius_cm` half-width. A main-gun round
    /// or rocket striking it damages the gun; small arms stop on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub barrel: Option<BarrelDef>,
    /// `hull`: traverse either side of straight ahead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc_deg: Option<u32>,
    /// Who fires it (default: the gunner; `hull` needs one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seat: Option<SeatRole>,
    /// `coax`, `hull` (S3b Fix): rounds stowed for this MG, 1..=20000; it
    /// stops firing when they are spent and refills a belt per rearm step
    /// in the own base. Omitted: no limit (it pays only in belt reloads).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rounds: Option<u32>,
}

/// The main gun's exposed barrel (S3b).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BarrelDef {
    pub length_cm: u32,
    pub radius_cm: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AmmoLoadDef {
    /// Ammo id (`defs/ammo/`).
    pub id: String,
    pub count: u32,
}

/// One component.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ComponentDef {
    #[serde(rename = "type")]
    pub kind: ComponentKind,
    pub hp: u32,
    /// Internal components (and the gun's breech): a box inside its frame
    /// ...
    #[serde(default, rename = "box", skip_serializing_if = "Option::is_none")]
    pub bounds: Option<PartBoxDef>,
    /// ... or (S3b) several boxes sharing one HP pool, e.g. ammo stowage
    /// bins. Set `box` or `boxes`, not both.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub boxes: Vec<PartBoxDef>,
    /// Tracks: anchor points (front, middle, rear) for blast overpressure.
    /// S3b: the gun has none (its barrel is the main-gun mount's box).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anchors: Vec<AnchorDef>,
    /// Field repair by the crew; none if absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair: Option<RepairDef>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnchorDef {
    #[serde(default)]
    pub frame: Frame,
    pub pos: [i32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RepairDef {
    /// The crew repairs the component up to this many HP ...
    pub to_hp: u32,
    /// ... over this long (from 0 HP; less damage takes proportionally less).
    pub time_s: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StationDef {
    pub id: String,
    pub seats: Vec<SeatRole>,
}

// ----------------------------------------------------------------- ammo --

/// Air drag coefficient. Rounds have no drag in S3; any value but 0 is
/// rejected. Kept as millionths so defs stay comparable and hashable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "f64", into = "f64")]
pub struct DragCoeff {
    pub micro: i64,
}

impl From<f64> for DragCoeff {
    #[allow(clippy::cast_possible_truncation)]
    fn from(v: f64) -> DragCoeff {
        let mut micro = (v * 1e6).round() as i64;
        if micro == 0 && v != 0.0 {
            // Tiny but non-zero still counts as drag.
            micro = if v > 0.0 { 1 } else { -1 };
        }
        DragCoeff { micro }
    }
}

impl From<DragCoeff> for f64 {
    #[allow(clippy::cast_precision_loss)]
    fn from(d: DragCoeff) -> f64 {
        d.micro as f64 / 1e6
    }
}

/// A round (`defs/ammo/*.toml`, §7.4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AmmoDef {
    pub id: String,
    pub name: String,
    pub mechanism: AmmoMechanism,
    pub caliber_mm: u32,
    pub muzzle_velocity_m_s: u32,
    /// Must be 0 in S3 (no drag on any round).
    #[serde(default)]
    #[schemars(with = "f64")]
    pub drag_coeff: DragCoeff,
    /// `[range_m, mm]` rows, ascending range, against a plate at
    /// `pen_reference_angle_deg`. Interpolated linearly, clamped at the
    /// ends; one row means flat (HE, HEAT).
    pub penetration: Vec<[u32; 2]>,
    /// Plate angle of the published table (30° for AP tables, 0° flat).
    #[serde(default)]
    pub pen_reference_angle_deg: u32,
    /// Impacts more oblique than this (from the plate normal) ricochet.
    pub ricochet_angle_deg: u32,
    /// Damage to the gun when this round strikes its barrel (S3b; it was
    /// "external parts within 1 m of a non-penetrating hit").
    #[serde(default)]
    pub nonpen_damage: u32,
    /// Damage to a track from a hit in the running-gear band.
    #[serde(default)]
    pub track_damage: u32,
    /// Penetrators: what goes on through the plate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub core: Option<CoreDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spall: Option<SpallDef>,
    /// `aphe`: the filler's burst inside.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filler: Option<FillerDef>,
    /// `he` and `heat`: the burst on contact (against men and tracks).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub he: Option<ItemHeDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
    /// S5a: what one round weighs and which supply kind refills it
    /// (main-gun rounds are `shells`; rockets and the Panzerfaust round
    /// `explosives`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supply: Option<crate::defs::SupplyDef>,
    /// S5a (owner D3): a rare round (APCR, HVAP): refilled one per
    /// `supply.rare_rearm_base_s` up to the vehicle's `rare_cap`; never a
    /// weapon's only feed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub rare: bool,
}

/// The core (or HEAT jet) after penetration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoreDef {
    /// Damage to every seat and component box it crosses ...
    pub damage: u32,
    /// ... along this much of the shell path behind the plate ...
    pub length_cm: u32,
    /// ... up to this many boxes.
    pub max_boxes: u32,
}

/// Spall rays from the entry point after penetration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpallDef {
    /// At most 8 (the §2.2 penetration budget).
    pub rays: u32,
    /// Damage to the first box each ray hits.
    pub damage: u32,
    /// Half-angle of the cone around the shell path.
    pub cone_deg: u32,
}

/// An AP round's HE filler (S3b: fragment rays, physical-contact rule).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FillerDef {
    /// Bursts this far past the entry point along the shell path ...
    pub offset_cm: u32,
    /// ... into this many fragment rays (at most 16) ...
    pub rays: u32,
    /// ... in a cone of this half-angle around the path ...
    pub cone_deg: u32,
    /// ... each dealing `damage` to the first box it crosses within
    /// `reach_cm`.
    pub damage: u32,
    pub reach_cm: u32,
}

// ----------------------------------------------------- per-file checks --

fn range(out: &mut Vec<Issue>, path: &str, v: i64, lo: i64, hi: i64) {
    if v < lo || v > hi {
        out.push(Issue::new(path, format!("must be in {lo}..={hi}, got {v}")));
    }
}

fn check_box(out: &mut Vec<Issue>, path: &str, b: &BoxDef) {
    for (a, h) in b.half.iter().enumerate() {
        range(out, &format!("{path}.half[{a}]"), *h as i64, 1, 2000);
    }
    for (a, c) in b.center.iter().enumerate() {
        range(out, &format!("{path}.center[{a}]"), *c as i64, -2000, 2000);
    }
}

/// `part` must lie inside its frame's bounds ([`VehicleDef::frame_bounds`]).
fn check_part(out: &mut Vec<Issue>, path: &str, part: &PartBoxDef, frames: &[Option<BoxDef>; 2]) {
    check_box(out, path, &part.bounds());
    let name = match part.frame {
        Frame::Hull => "hull",
        Frame::Turret => "turret",
    };
    let Some(frame) = frames[part.frame as usize] else {
        out.push(
            Issue::new(path, format!("no {name} frame on this vehicle"))
                .with_hint("set frame = \"hull\""),
        );
        return;
    };
    if !frame.contains(&part.bounds()) {
        let lo = |a| frame.lo(a);
        let hi = |a| frame.hi(a);
        out.push(
            Issue::new(
                path,
                format!(
                    "box {:?} ± {:?} lies outside its frame: the {name} boxes span x {}..{}, y {}..{}, z {}..{} cm",
                    part.center,
                    part.half,
                    lo(0),
                    hi(0),
                    lo(1),
                    hi(1),
                    lo(2),
                    hi(2)
                ),
            )
            .with_hint(format!(
                "move it inside the {name} boxes, or set frame = \"{}\" if it belongs to the {}",
                match part.frame {
                    Frame::Hull => "turret",
                    Frame::Turret => "hull",
                },
                match part.frame {
                    Frame::Hull => "turret",
                    Frame::Turret => "hull",
                },
            )),
        );
    }
}

impl VehicleDef {
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        check_id(&mut out, "id", &self.id);
        if self.name.trim().is_empty() {
            out.push(Issue::new("name", "must not be empty"));
        }
        range(&mut out, "cost", self.cost as i64, 0, 100_000);
        range(&mut out, "pop", self.pop as i64, 1, 120);
        range(
            &mut out,
            "recall_cooldown_s",
            self.recall_cooldown_s as i64,
            0,
            crate::MAX_COOLDOWN_S as i64,
        );
        range(
            &mut out,
            "initial_cooldown_s",
            self.initial_cooldown_s as i64,
            0,
            crate::MAX_INITIAL_COOLDOWN_S as i64,
        );
        range(&mut out, "sight_m", self.sight_m as i64, 1, 2000);
        range(
            &mut out,
            "spot_size_pct",
            self.spot_size_pct as i64,
            1,
            1000,
        );
        range(
            &mut out,
            "eye_height_cm",
            self.eye_height_cm as i64,
            50,
            1000,
        );

        let m = &self.mobility;
        range(
            &mut out,
            "mobility.speed_cm_s",
            m.speed_cm_s as i64,
            10,
            3000,
        );
        range(
            &mut out,
            "mobility.reverse_cm_s",
            m.reverse_cm_s as i64,
            1,
            m.speed_cm_s as i64,
        );
        let wheeled = m.kind == MobilityKind::Wheeled;
        if wheeled {
            match m.turn_radius_cm {
                Some(r) => range(&mut out, "mobility.turn_radius_cm", r as i64, 300, 2000),
                None => out.push(
                    Issue::new(
                        "mobility.turn_radius_cm",
                        "`wheeled` needs a turning radius",
                    )
                    .with_hint("turn_radius_cm = 700"),
                ),
            }
            if m.turn_deg_s != 0 {
                out.push(Issue::new(
                    "mobility.turn_deg_s",
                    "`wheeled` steers by turn_radius_cm; remove turn_deg_s",
                ));
            }
        } else {
            range(&mut out, "mobility.turn_deg_s", m.turn_deg_s as i64, 1, 360);
            if m.turn_radius_cm.is_some() {
                out.push(Issue::new(
                    "mobility.turn_radius_cm",
                    "only `wheeled` has a turning radius",
                ));
            }
        }
        if let Some(p) = m.woods_speed_pct {
            range(&mut out, "mobility.woods_speed_pct", p as i64, 1, 100);
        }
        if let Some(p) = m.slope_loss_pct {
            range(&mut out, "mobility.slope_loss_pct", p as i64, 0, 95);
        }
        if let Some(o) = &self.optics {
            range(&mut out, "optics.settle_s", o.settle_s as i64, 0, 60);
            range(
                &mut out,
                "optics.stationary_sight_pct",
                o.stationary_sight_pct as i64,
                100,
                300,
            );
        }
        self.validate_truck(&mut out);
        range(
            &mut out,
            "mobility.accel_cm_s2",
            m.accel_cm_s2 as i64,
            1,
            2000,
        );
        range(
            &mut out,
            "mobility.brake_cm_s2",
            m.brake_cm_s2 as i64,
            1,
            5000,
        );
        if m.max_slope_pct == 0 || m.max_slope_pct > MAX_VEHICLE_SLOPE_PCT {
            out.push(
                Issue::new(
                    "mobility.max_slope_pct",
                    format!(
                        "must be in 1..={MAX_VEHICLE_SLOPE_PCT} (the 30° pitch clamp), got {}",
                        m.max_slope_pct
                    ),
                )
                .with_hint("max_slope_pct = 50"),
            );
        }
        range(
            &mut out,
            "mobility.ditch_block_dm",
            m.ditch_block_dm as i64,
            1,
            100,
        );
        range(&mut out, "mobility.bog_pct", m.bog_pct as i64, 0, 100);
        range(&mut out, "mobility.unbog_s[1]", m.unbog_s[1] as i64, 0, 600);
        if m.unbog_s[0] > m.unbog_s[1] {
            out.push(Issue::new(
                "mobility.unbog_s",
                format!("[{}, {}] is not [min, max]", m.unbog_s[0], m.unbog_s[1]),
            ));
        }
        range(
            &mut out,
            "mobility.moving_spread_pct",
            m.moving_spread_pct as i64,
            100,
            1000,
        );

        // S7: armour boxes (box 0 the running-gear hull).
        if self.boxes.is_empty() || self.boxes.len() > MAX_ARMOUR_BOXES {
            out.push(Issue::new(
                "boxes",
                format!(
                    "needs 1..={MAX_ARMOUR_BOXES} armour boxes, got {}",
                    self.boxes.len()
                ),
            ));
        }
        let zero = BoxDef {
            center: [0, 0, 100],
            half: [100, 100, 50],
        };
        let hull = self.boxes.first().map_or(&zero, |b| &b.bounds);
        if self.boxes.first().is_some_and(|b| b.frame != Frame::Hull) {
            out.push(Issue::new(
                "boxes[0].frame",
                "box 0 is the running-gear hull: frame = \"hull\"",
            ));
        }
        if hull.lo(2) < 0 {
            out.push(Issue::new(
                "boxes[0].box",
                "the hull box reaches below the ground (center.z < half.z)",
            ));
        }
        let turret_boxes = self
            .boxes
            .iter()
            .filter(|b| b.frame == Frame::Turret)
            .count();
        let casemate = self.weapons.iter().any(|w| w.mount == MountKind::Casemate);
        match (&self.turret, turret_boxes) {
            (Some(_), 1) | (None, 0) => {}
            (Some(_), n) => out.push(Issue::new(
                "boxes",
                format!("a [turret] needs exactly one frame = \"turret\" box, got {n}"),
            )),
            (None, _) => out.push(
                Issue::new("boxes", "a frame = \"turret\" box needs a [turret]")
                    .with_hint("add [turret] pivot, traverse_deg_s, elevate_deg_s"),
            ),
        }
        if self.turret.is_some() && casemate {
            out.push(Issue::new(
                "turret",
                "a vehicle has a turret or a casemate gun, not both",
            ));
        }
        for (i, b) in self.boxes.iter().enumerate() {
            let p = format!("boxes[{i}]");
            check_id_word(&mut out, &format!("{p}.id"), &b.id);
            if self.boxes[..i].iter().any(|o| o.id == b.id) {
                out.push(Issue::new(
                    format!("{p}.id"),
                    format!("`{}` is used twice", b.id),
                ));
            }
            check_box(&mut out, &format!("{p}.box"), &b.bounds);
            for (k, g) in b.open.iter().enumerate() {
                if b.open[..k].contains(g) {
                    out.push(Issue::new(
                        format!("{p}.open[{k}]"),
                        format!("`{}` is listed twice", g.name()),
                    ));
                }
            }
            for (k, g) in b.grenade_screen.iter().enumerate() {
                if !b.open.contains(g) || b.grenade_screen[..k].contains(g) {
                    out.push(Issue::new(
                        format!("{p}.grenade_screen[{k}]"),
                        format!("`{}` is not an open face or is listed twice", g.name()),
                    ));
                }
            }
            if i == 0 && !b.open.is_empty() {
                out.push(Issue::new(
                    format!("{p}.open"),
                    "box 0 (the running-gear hull) has no open face",
                ));
            }
            if let Some(f) = b.floor_cm {
                if b.open.is_empty() {
                    out.push(Issue::new(
                        format!("{p}.floor_cm"),
                        "only an open box has a floor",
                    ));
                }
                range(
                    &mut out,
                    &format!("{p}.floor_cm"),
                    f as i64,
                    b.bounds.lo(2),
                    b.bounds.hi(2),
                );
            }
            for g in PlateGroup::ALL {
                let path = format!("{p}.armor.{}", g.name());
                let open = b.open.contains(&g);
                let Some(pl) = b.armor.get(g) else {
                    if !open {
                        out.push(
                            Issue::new(path, "missing plate")
                                .with_hint(format!("{} = {{ thickness_mm = 20 }}", g.name())),
                        );
                    }
                    continue;
                };
                if open {
                    out.push(Issue::new(path, "an open face has no plate"));
                    continue;
                }
                if !(10..=5000).contains(&pl.thickness_mm.0) {
                    out.push(Issue::new(
                        format!("{path}.thickness_mm"),
                        format!("must be in 1..=500 mm, got {}", pl.thickness_mm),
                    ));
                }
                if g.is_top() {
                    if pl.slope_deg != 0 {
                        out.push(
                            Issue::new(
                                format!("{path}.slope_deg"),
                                format!("top plates are flat, got {}°", pl.slope_deg),
                            )
                            .with_hint("remove slope_deg"),
                        );
                    }
                } else {
                    range(
                        &mut out,
                        &format!("{path}.slope_deg"),
                        pl.slope_deg as i64,
                        0,
                        80,
                    );
                }
            }
        }
        range(
            &mut out,
            "hull.track_band_pct",
            self.hull.track_band_pct as i64,
            0,
            100,
        );
        range(&mut out, "hull.track_mm", self.hull.track_mm as i64, 0, 500);
        if let Some(t) = &self.turret {
            for (a, p) in t.pivot.iter().enumerate() {
                range(
                    &mut out,
                    &format!("turret.pivot[{a}]"),
                    *p as i64,
                    -2000,
                    2000,
                );
            }
            if (0..2).any(|a| {
                let p = t.pivot[a] as i64;
                p < hull.lo(a) || p > hull.hi(a)
            }) {
                out.push(Issue::new(
                    "turret.pivot",
                    "the pivot must lie over the hull box (x and y inside it)",
                ));
            }
            range(
                &mut out,
                "turret.traverse_deg_s",
                t.traverse_deg_s as i64,
                1,
                360,
            );
            range(
                &mut out,
                "turret.elevate_deg_s",
                t.elevate_deg_s as i64,
                1,
                90,
            );
        }
        // Where parts of each frame may lie (S3b Tune: the turret basket;
        // S7: up to `EXPOSED_CM` above an open-topped box).
        let frames = self.frame_bounds();

        let roles: Vec<SeatRole> = self.seats.iter().map(|s| s.role).collect();
        let seats = &roles;
        if seats.is_empty() || seats.len() > SEAT_ROLES {
            out.push(Issue::new(
                "seats",
                format!("needs 1..={SEAT_ROLES} seats, got {}", seats.len()),
            ));
        }
        for (i, st) in self.seats.iter().enumerate() {
            let p = format!("seats[{i}]");
            if seats[..i].contains(&st.role) {
                out.push(Issue::new(
                    format!("{p}.role"),
                    format!("`{}` is listed twice", st.role.name()),
                ));
            }
            check_id_word(&mut out, &format!("{p}.id"), &st.id);
            if self.seats[..i].iter().any(|o| o.id == st.id) {
                out.push(Issue::new(
                    format!("{p}.id"),
                    format!("`{}` is used twice", st.id),
                ));
            }
            if let Some(b) = &st.in_box
                && !self.boxes.iter().any(|x| &x.id == b)
            {
                out.push(Issue::new(
                    format!("{p}.in"),
                    format!("no armour box `{b}`"),
                ));
            }
            check_part(&mut out, &format!("{p}.box"), &st.bounds, &frames);
        }
        // S8a-1: a gunner only with a main gun.
        let gunner = if self.main_gun().is_some() {
            &[SeatRole::Driver, SeatRole::Gunner][..]
        } else {
            &[SeatRole::Driver][..]
        };
        for &need in gunner {
            if !seats.contains(&need) {
                out.push(
                    Issue::new("seats", format!("needs a `{}` seat", need.name()))
                        .with_hint(format!("[[seats]] role = \"{}\"", need.name())),
                );
            }
        }

        if self.weapons.len() > MAX_MOUNTS {
            out.push(Issue::new(
                "weapons",
                format!("at most {MAX_MOUNTS} weapons, got {}", self.weapons.len()),
            ));
        }
        let mains = self.weapons.iter().filter(|w| w.mount.is_main()).count();
        // S8a-1: a transport may have a main gun; S8b: a truck has none;
        // every other role exactly one.
        let (want_mains, needs) = match self.role {
            VehicleRole::Transport => (0..=1, "at most one"),
            VehicleRole::Truck => (0..=0, "no"),
            _ => (1..=1, "exactly one"),
        };
        if !want_mains.contains(&mains) {
            out.push(Issue::new(
                "weapons",
                format!("needs {needs} main gun (`turret_main` or `casemate`), got {mains}"),
            ));
        }
        if self.weapons.is_empty() && self.role != VehicleRole::Truck {
            out.push(Issue::new("weapons", "needs at least one weapon"));
        }
        for kind in [MountKind::Coax, MountKind::Hull] {
            let n = self.weapons.iter().filter(|w| w.mount == kind).count();
            if n > 1 {
                out.push(Issue::new(
                    "weapons",
                    format!("needs at most one `{kind:?}` mount, got {n}"),
                ));
            }
        }
        if self.turret.is_none()
            && self
                .weapons
                .iter()
                .any(|w| matches!(w.mount, MountKind::TurretMain | MountKind::Coax))
        {
            out.push(
                Issue::new("weapons", "`turret_main` and `coax` mounts need a [turret]")
                    .with_hint("mount = \"casemate\" with pivot and traverse_deg"),
            );
        }
        for (i, w) in self.weapons.iter().enumerate() {
            let p = format!("weapons[{i}]");
            for (a, c) in w.muzzle.iter().enumerate() {
                range(
                    &mut out,
                    &format!("{p}.muzzle[{a}]"),
                    *c as i64,
                    -2000,
                    2000,
                );
            }
            match w.mount {
                MountKind::TurretMain | MountKind::Casemate => {
                    let casemate = w.mount == MountKind::Casemate;
                    let cfields = w.pivot.is_some()
                        || w.traverse_deg.is_some()
                        || w.traverse_deg_s.is_some()
                        || w.elevate_deg_s.is_some();
                    if !casemate && cfields {
                        out.push(Issue::new(
                            p.clone(),
                            "`pivot`, `traverse_deg`, `traverse_deg_s` and `elevate_deg_s` are casemate fields",
                        ));
                    }
                    if casemate {
                        match w.pivot {
                            None => out.push(
                                Issue::new(format!("{p}.pivot"), "a casemate gun needs a pivot")
                                    .with_hint("pivot = [110, -15, 165]"),
                            ),
                            Some(pv) => {
                                if (0..2).any(|a| {
                                    let c = pv[a] as i64;
                                    c < hull.lo(a) || c > hull.hi(a)
                                }) || !(0..=1000).contains(&pv[2])
                                {
                                    out.push(Issue::new(
                                        format!("{p}.pivot"),
                                        "the pivot must lie over the hull box, 0..=1000 cm up",
                                    ));
                                }
                            }
                        }
                        match w.traverse_deg {
                            None => out.push(
                                Issue::new(
                                    format!("{p}.traverse_deg"),
                                    "a casemate gun needs its arc",
                                )
                                .with_hint("traverse_deg = [-10, 10]"),
                            ),
                            Some([lo, hi]) => {
                                if !(-90..=0).contains(&lo) || !(0..=90).contains(&hi) || lo == hi {
                                    out.push(Issue::new(
                                        format!("{p}.traverse_deg"),
                                        format!(
                                            "must be [-90..=0, 0..=90] and not empty, got [{lo}, {hi}]"
                                        ),
                                    ));
                                }
                            }
                        }
                        match w.traverse_deg_s {
                            None => out.push(Issue::new(
                                format!("{p}.traverse_deg_s"),
                                "a casemate gun needs its traverse rate",
                            )),
                            Some(r) => {
                                range(&mut out, &format!("{p}.traverse_deg_s"), r as i64, 1, 360)
                            }
                        }
                        match w.elevate_deg_s {
                            None => out.push(Issue::new(
                                format!("{p}.elevate_deg_s"),
                                "a casemate gun needs its elevation rate",
                            )),
                            Some(r) => {
                                range(&mut out, &format!("{p}.elevate_deg_s"), r as i64, 1, 90)
                            }
                        }
                    }
                    match w.elevation_deg {
                        None => out.push(
                            Issue::new(format!("{p}.elevation_deg"), "the main gun needs limits")
                                .with_hint("elevation_deg = [-8, 20]"),
                        ),
                        Some([lo, hi]) => {
                            if !(-30..=0).contains(&lo) || !(1..=60).contains(&hi) {
                                out.push(Issue::new(
                                    format!("{p}.elevation_deg"),
                                    format!("must be [-30..=0, 1..=60], got [{lo}, {hi}]"),
                                ));
                            }
                        }
                    }
                    if w.ammo.is_empty() || w.ammo.len() > MAX_MAIN_AMMO {
                        out.push(Issue::new(
                            format!("{p}.ammo"),
                            format!(
                                "the main gun carries 1..={MAX_MAIN_AMMO} rounds, got {}",
                                w.ammo.len()
                            ),
                        ));
                    }
                    match &w.barrel {
                        None => out.push(
                            Issue::new(format!("{p}.barrel"), "the main gun needs a barrel box")
                                .with_hint("barrel = { length_cm = 300, radius_cm = 8 }"),
                        ),
                        Some(b) => {
                            range(
                                &mut out,
                                &format!("{p}.barrel.length_cm"),
                                b.length_cm as i64,
                                1,
                                1000,
                            );
                            range(
                                &mut out,
                                &format!("{p}.barrel.radius_cm"),
                                b.radius_cm as i64,
                                1,
                                50,
                            );
                        }
                    }
                    for (k, a) in w.ammo.iter().enumerate() {
                        range(
                            &mut out,
                            &format!("{p}.ammo[{k}].count"),
                            a.count as i64,
                            0,
                            500,
                        );
                    }
                    if w.arc_deg.is_some() {
                        out.push(Issue::new(
                            format!("{p}.arc_deg"),
                            "only a hull mount has an arc",
                        ));
                    }
                    if w.rounds.is_some() {
                        out.push(
                            Issue::new(format!("{p}.rounds"), "only an MG mount has `rounds`")
                                .with_hint("the main gun's rounds are its `ammo` counts"),
                        );
                    }
                }
                MountKind::Pintle => {
                    match w.pivot {
                        None => out.push(
                            Issue::new(format!("{p}.pivot"), "a pintle mount needs a pivot")
                                .with_hint("pivot = [150, 0, 190]"),
                        ),
                        Some(pv) => {
                            if (0..2).any(|a| {
                                let c = pv[a] as i64;
                                c < hull.lo(a) || c > hull.hi(a)
                            }) || !(0..=1000).contains(&pv[2])
                            {
                                out.push(Issue::new(
                                    format!("{p}.pivot"),
                                    "the pivot must lie over the hull box, 0..=1000 cm up",
                                ));
                            }
                        }
                    }
                    match w.traverse_deg {
                        None => out.push(
                            Issue::new(format!("{p}.traverse_deg"), "a pintle mount needs its arc")
                                .with_hint("traverse_deg = [-35, 35] (or [-180, 180], a ring)"),
                        ),
                        Some([lo, hi]) => {
                            if !(-180..=0).contains(&lo) || !(0..=180).contains(&hi) || lo == hi {
                                out.push(Issue::new(
                                    format!("{p}.traverse_deg"),
                                    format!(
                                        "must be [-180..=0, 0..=180] and not empty, got [{lo}, {hi}]"
                                    ),
                                ));
                            }
                        }
                    }
                    match w.traverse_deg_s {
                        None => out.push(Issue::new(
                            format!("{p}.traverse_deg_s"),
                            "a pintle mount needs its traverse rate",
                        )),
                        Some(r) => {
                            range(&mut out, &format!("{p}.traverse_deg_s"), r as i64, 1, 360)
                        }
                    }
                    if w.elevate_deg_s.is_some() || w.arc_deg.is_some() {
                        out.push(Issue::new(
                            p.clone(),
                            "a pintle mount has no `elevate_deg_s` or `arc_deg`",
                        ));
                    }
                    if !w.ammo.is_empty() || w.elevation_deg.is_some() || w.barrel.is_some() {
                        out.push(Issue::new(
                            p.clone(),
                            "only the main gun has `ammo`, `elevation_deg` and `barrel`",
                        ));
                    }
                    if let Some(n) = w.rounds {
                        range(&mut out, &format!("{p}.rounds"), n as i64, 1, 20_000);
                    }
                    if w.seat.is_none() {
                        out.push(
                            Issue::new(format!("{p}.seat"), "a pintle mount needs a seat")
                                .with_hint("seat = \"commander\""),
                        );
                    }
                }
                MountKind::Coax | MountKind::Hull => {
                    if w.pivot.is_some()
                        || w.traverse_deg.is_some()
                        || w.traverse_deg_s.is_some()
                        || w.elevate_deg_s.is_some()
                    {
                        out.push(Issue::new(
                            p.clone(),
                            "`pivot`, `traverse_deg`, `traverse_deg_s` and `elevate_deg_s` are casemate fields",
                        ));
                    }
                    if let Some(n) = w.rounds {
                        range(&mut out, &format!("{p}.rounds"), n as i64, 1, 20_000);
                    }
                    if !w.ammo.is_empty() || w.elevation_deg.is_some() || w.barrel.is_some() {
                        out.push(Issue::new(
                            p.clone(),
                            "only the main gun has `ammo`, `elevation_deg` and `barrel`",
                        ));
                    }
                    if w.mount == MountKind::Hull {
                        match w.arc_deg {
                            None => out.push(
                                Issue::new(format!("{p}.arc_deg"), "a hull mount needs an arc")
                                    .with_hint("arc_deg = 15"),
                            ),
                            Some(a) => range(&mut out, &format!("{p}.arc_deg"), a as i64, 1, 90),
                        }
                        if w.seat.is_none() {
                            out.push(
                                Issue::new(format!("{p}.seat"), "a hull mount needs a seat")
                                    .with_hint("seat = \"radio_mg\""),
                            );
                        }
                    } else if w.arc_deg.is_some() {
                        out.push(Issue::new(
                            format!("{p}.arc_deg"),
                            "only a hull mount has an arc",
                        ));
                    }
                }
            }
            if let Some(s) = w.seat
                && !seats.contains(&s)
            {
                out.push(Issue::new(
                    format!("{p}.seat"),
                    format!("`{}` is not in crew.seats", s.name()),
                ));
            }
        }

        for kind in ComponentKind::ALL {
            let n = self.components.iter().filter(|c| c.kind == kind).count();
            // S7: a turret ring exactly with a turret; S8a: a gun and an
            // ammo rack exactly with a main gun; S8b: wheels or tracks by
            // the mobility, cargo exactly on a truck.
            let want = if component_absent(
                kind,
                self.turret.is_some(),
                self.main_gun().is_some(),
                self.mobility.kind,
                self.role,
            ) {
                0
            } else {
                1
            };
            if n != want {
                out.push(
                    Issue::new(
                        "components",
                        format!(
                            "needs {} `{}` component, got {n}",
                            if want == 1 { "exactly one" } else { "no" },
                            kind.name()
                        ),
                    )
                    .with_hint(format!("[[components]] type = \"{}\" ...", kind.name())),
                );
            }
        }
        // S3b Fix: the damage pass tests at most MAX_INNER_BOXES inner boxes
        // (seats first, then components); a box past the cap could never
        // be damaged.
        let inner = self.seats.len()
            + self.passenger_slots.len()
            + self
                .components
                .iter()
                .filter(|c| c.kind.has_box())
                .map(|c| c.bounds.is_some() as usize + c.boxes.len())
                .sum::<usize>();
        if inner > MAX_INNER_BOXES {
            out.push(
                Issue::new(
                    "components",
                    format!(
                        "at most {MAX_INNER_BOXES} inner boxes (crew seats, passenger slots and component boxes), got {inner}"
                    ),
                )
                .with_hint("merge stowage bins or drop a seat"),
            );
        }
        for (i, c) in self.components.iter().enumerate() {
            let p = format!("components[{i}]");
            range(&mut out, &format!("{p}.hp"), c.hp as i64, 1, 1000);
            let n_boxes = c.bounds.is_some() as usize + c.boxes.len();
            match (c.kind.has_box(), n_boxes) {
                (true, 0) => out.push(
                    Issue::new(
                        format!("{p}.box"),
                        format!("`{}` needs a box", c.kind.name()),
                    )
                    .with_hint("box = { frame = \"hull\", center = [x, y, z], half = [x, y, z] }"),
                ),
                (false, n) if n > 0 => out.push(Issue::new(
                    format!("{p}.box"),
                    "running gear has anchors, not a box",
                )),
                _ => {}
            }
            if c.bounds.is_some() && !c.boxes.is_empty() {
                out.push(
                    Issue::new(format!("{p}.boxes"), "set `box` or `boxes`, not both")
                        .with_hint("move the `box` into `boxes`"),
                );
            }
            if c.boxes.len() > MAX_COMPONENT_BOXES {
                out.push(Issue::new(
                    format!("{p}.boxes"),
                    format!("at most {MAX_COMPONENT_BOXES} boxes, got {}", c.boxes.len()),
                ));
            }
            if c.kind.has_box() {
                if let Some(b) = &c.bounds {
                    check_part(&mut out, &format!("{p}.box"), b, &frames);
                }
                for (k, b) in c.boxes.iter().enumerate() {
                    check_part(&mut out, &format!("{p}.boxes[{k}]"), b, &frames);
                }
            }
            let anchors = match c.kind {
                k if k.is_running_gear() => 1..=MAX_ANCHORS,
                _ => 0..=0,
            };
            if !anchors.contains(&c.anchors.len()) {
                out.push(Issue::new(
                    format!("{p}.anchors"),
                    format!(
                        "`{}` takes {}..={} anchors, got {}",
                        c.kind.name(),
                        anchors.start(),
                        anchors.end(),
                        c.anchors.len()
                    ),
                ));
            }
            for (k, a) in c.anchors.iter().enumerate() {
                for (ax, v) in a.pos.iter().enumerate() {
                    range(
                        &mut out,
                        &format!("{p}.anchors[{k}].pos[{ax}]"),
                        *v as i64,
                        -2000,
                        2000,
                    );
                }
            }
            if let Some(r) = &c.repair {
                range(
                    &mut out,
                    &format!("{p}.repair.to_hp"),
                    r.to_hp as i64,
                    1,
                    c.hp as i64,
                );
                range(
                    &mut out,
                    &format!("{p}.repair.time_s"),
                    r.time_s as i64,
                    1,
                    600,
                );
            }
        }

        // S3c: hatches cover every seat once; each opening is on or inside
        // its frame's box (the turret frame from the pivot).
        if self.hatches.len() > MAX_HATCHES {
            out.push(Issue::new("hatches", format!("at most {MAX_HATCHES}")));
        }
        if self.crew_squad.is_some() && self.hatches.is_empty() {
            out.push(
                Issue::new("hatches", "a vehicle with a crew_squad needs hatches").with_hint(
                    "[[hatches]] seats = [\"driver\"], pos = [200, 60, 176], exit_ms = 4000",
                ),
            );
        }
        if let Some(c) = &self.crew_squad {
            check_id(&mut out, "crew_squad", c);
        }
        if !self.hatches.is_empty() {
            for r in seats {
                let n = self
                    .hatches
                    .iter()
                    .filter(|h| h.seats.contains(r))
                    .map(|h| h.seats.iter().filter(|x| *x == r).count())
                    .sum::<usize>();
                if n != 1 {
                    out.push(Issue::new(
                        "hatches",
                        format!(
                            "seat `{}` must be in exactly one hatch (found {n})",
                            r.name()
                        ),
                    ));
                }
            }
        }
        for (i, h) in self.hatches.iter().enumerate() {
            for (k, r) in h.seats.iter().enumerate() {
                if !seats.contains(r) {
                    out.push(Issue::new(
                        format!("hatches[{i}].seats[{k}]"),
                        format!("`{}` is not in crew.seats", r.name()),
                    ));
                }
            }
            if h.seats.is_empty() {
                out.push(Issue::new(
                    format!("hatches[{i}].seats"),
                    "must not be empty",
                ));
            }
            range(
                &mut out,
                &format!("hatches[{i}].exit_ms"),
                h.exit_ms as i64,
                100,
                30_000,
            );
            let Some(b) = frames[h.frame as usize] else {
                out.push(Issue::new(
                    format!("hatches[{i}].frame"),
                    "no turret: use frame = \"hull\"",
                ));
                continue;
            };
            let on = (0..3).all(|a| {
                let lo = b.center[a] as i64 - b.half[a] as i64;
                let hi = b.center[a] as i64 + b.half[a] as i64;
                (lo..=hi).contains(&(h.pos[a] as i64))
            });
            if !on {
                out.push(Issue::new(
                    format!("hatches[{i}].pos"),
                    "must be on or inside its frame's box",
                ));
            }
        }

        if self.stations.len() > MAX_STATIONS {
            out.push(Issue::new("stations", format!("at most {MAX_STATIONS}")));
        }
        for (i, s) in self.stations.iter().enumerate() {
            for (k, r) in s.seats.iter().enumerate() {
                if !seats.contains(r) {
                    out.push(Issue::new(
                        format!("stations[{i}].seats[{k}]"),
                        format!("`{}` is not in crew.seats", r.name()),
                    ));
                }
            }
        }
        self.validate_s8a(&mut out, &frames);
        out
    }

    /// S8b: `[supply]` exactly on role `truck` (cross-references are
    /// checked by the mod set, which knows the supply and ammo defs).
    fn validate_truck(&self, out: &mut Vec<Issue>) {
        let truck = self.role == VehicleRole::Truck;
        match (&self.supply, truck) {
            (None, true) => {
                out.push(
                    Issue::new("supply", "a `truck` vehicle needs [supply]")
                        .with_hint("[supply] cap_kg_total = 2500 ..."),
                );
                return;
            }
            (Some(_), false) => {
                out.push(Issue::new("supply", "only role = \"truck\" has [supply]"));
                return;
            }
            _ => {}
        }
        if truck && self.transport.is_some() {
            out.push(Issue::new(
                "transport",
                "a truck carries supply only (S8b-5)",
            ));
        }
        let Some(t) = &self.supply else {
            return;
        };
        range(out, "supply.cap_kg_total", t.cap_kg_total as i64, 1, 10_000);
        range(out, "supply.radius_m", t.radius_m as i64, 1, 50);
        range(
            out,
            "supply.vehicle_radius_m",
            t.vehicle_radius_m as i64,
            1,
            50,
        );
        range(
            out,
            "supply.round_interval_s",
            t.round_interval_s as i64,
            1,
            60,
        );
        if t.load_kg.is_empty() {
            out.push(Issue::new("supply.load_kg", "a truck loads something"));
        }
        for (k, kg) in &t.load_kg {
            if crate::defs::SupplyKind::from_name(k).is_none() {
                out.push(Issue::new(
                    format!("supply.load_kg.{k}"),
                    "not a supply kind (small_arms, explosives, shells)",
                ));
            }
            range(
                out,
                &format!("supply.load_kg.{k}"),
                *kg as i64,
                0,
                t.cap_kg_total as i64,
            );
        }
        if t.crates.is_empty() || t.crates.len() > 3 {
            out.push(Issue::new("supply.crates", "needs 1 to 3 crate defs"));
        }
    }

    /// S8a: shields, `[transport]` and passenger slots.
    fn validate_s8a(&self, out: &mut Vec<Issue>, frames: &[Option<BoxDef>; 2]) {
        if self.shields.len() > MAX_SHIELDS {
            out.push(Issue::new(
                "shields",
                format!("at most {MAX_SHIELDS} shields, got {}", self.shields.len()),
            ));
        }
        for (i, sh) in self.shields.iter().enumerate() {
            let p = format!("shields[{i}]");
            check_box(out, &format!("{p}.box"), &sh.bounds);
            if !(1..=500).contains(&(sh.thickness_mm.0 / 10)) {
                out.push(Issue::new(
                    format!("{p}.thickness_mm"),
                    "must be 1..=500 mm",
                ));
            }
            if let Some(k) = sh.mount
                && self
                    .weapons
                    .get(k)
                    .is_none_or(|w| w.mount != MountKind::Pintle)
            {
                out.push(Issue::new(
                    format!("{p}.mount"),
                    format!("weapons[{k}] is not a pintle mount"),
                ));
            }
        }
        let transport = self.role == VehicleRole::Transport;
        match (&self.transport, transport) {
            (None, true) => out.push(
                Issue::new("transport", "a `transport` vehicle needs [transport]")
                    .with_hint("[transport] capacity = 10 ..."),
            ),
            (Some(_), false) => out.push(Issue::new(
                "transport",
                "only role = \"transport\" has [transport]",
            )),
            _ => {}
        }
        if !transport && !self.passenger_slots.is_empty() {
            out.push(Issue::new(
                "passenger_slots",
                "only role = \"transport\" has passenger slots",
            ));
        }
        let Some(t) = &self.transport else {
            return;
        };
        range(
            out,
            "transport.capacity",
            t.capacity as i64,
            1,
            MAX_PASSENGERS as i64,
        );
        if self.passenger_slots.len() != t.capacity as usize {
            out.push(Issue::new(
                "passenger_slots",
                format!(
                    "needs one slot per passenger ({}), got {}",
                    t.capacity,
                    self.passenger_slots.len()
                ),
            ));
        }
        for (a, c) in t.entry.iter().enumerate() {
            range(
                out,
                &format!("transport.entry[{a}]"),
                *c as i64,
                -2000,
                2000,
            );
        }
        range(out, "transport.board_ms", t.board_ms as i64, 100, 30_000);
        range(out, "transport.exit_ms", t.exit_ms as i64, 100, 30_000);
        if let Some(ms) = t.exit_over_sides_ms {
            range(out, "transport.exit_over_sides_ms", ms as i64, 100, 30_000);
        }
        range(
            out,
            "transport.exit_raise_cm",
            t.exit_raise_cm as i64,
            0,
            200,
        );
        for (i, sl) in self.passenger_slots.iter().enumerate() {
            let part = PartBoxDef {
                frame: Frame::Hull,
                center: sl.bounds.center,
                half: sl.bounds.half,
            };
            check_part(out, &format!("passenger_slots[{i}].box"), &part, frames);
        }
    }

    /// The main gun mount (validated defs have exactly one).
    pub fn main_gun(&self) -> Option<&MountDef> {
        self.weapons.iter().find(|w| w.mount.is_main())
    }

    /// Where parts of each frame may lie, by `Frame as usize` (`None`: no
    /// such frame). Hull: the bounds of every hull-frame box; turret: the
    /// turret box extended down to the hull floor (S3b Tune, the basket).
    /// S7: either extends `EXPOSED_CM` above an open-topped box of its
    /// frame (an exposed crewman's head and shoulders).
    pub fn frame_bounds(&self) -> [Option<BoxDef>; 2] {
        let hull = self.boxes.first().map(|b| b.bounds);
        let mut out = [None, None];
        for (k, frame) in [Frame::Hull, Frame::Turret].into_iter().enumerate() {
            let mut lo = [i64::MAX; 3];
            let mut hi = [i64::MIN; 3];
            for b in self.boxes.iter().filter(|b| b.frame == frame) {
                let bb = match (frame, &self.turret, hull) {
                    (Frame::Turret, Some(t), Some(h)) => basket_def(&b.bounds, t.pivot[2], &h),
                    _ => b.bounds,
                };
                let up = if b.open.contains(&PlateGroup::Top) {
                    EXPOSED_CM
                } else {
                    0
                };
                for a in 0..3 {
                    lo[a] = lo[a].min(bb.lo(a));
                    hi[a] = hi[a].max(bb.hi(a) + if a == 2 { up } else { 0 });
                }
            }
            if lo[0] <= hi[0] {
                out[k] = Some(BoxDef {
                    center: [0, 1, 2].map(|a| (lo[a] + hi[a]).div_euclid(2) as i32),
                    half: [0, 1, 2].map(|a| ((hi[a] - lo[a] + 1) / 2) as u32),
                });
            }
        }
        out
    }
}

/// S7: how far above an open-topped box a part of its frame may reach
/// (cm): an exposed crewman's head and shoulders.
pub const EXPOSED_CM: i64 = 50;

/// A short lowercase id word (`hull`, `asst_driver`).
fn check_id_word(out: &mut Vec<Issue>, path: &str, id: &str) {
    let ok = !id.is_empty()
        && id.len() <= 24
        && id
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_');
    if !ok {
        out.push(Issue::new(
            path,
            format!("`{id}`: 1..=24 of a-z, 0-9 and _"),
        ));
    }
}

impl AmmoDef {
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        check_id(&mut out, "id", &self.id);
        if self.name.trim().is_empty() {
            out.push(Issue::new("name", "must not be empty"));
        }
        range(&mut out, "caliber_mm", self.caliber_mm as i64, 1, 500);
        range(
            &mut out,
            "muzzle_velocity_m_s",
            self.muzzle_velocity_m_s as i64,
            1,
            3000,
        );
        if self.drag_coeff.micro != 0 {
            out.push(
                Issue::new(
                    "drag_coeff",
                    format!(
                        "rounds have no drag in S3; must be 0, got {}",
                        f64::from(self.drag_coeff)
                    ),
                )
                .with_hint("drag_coeff = 0 (or remove it)"),
            );
        }
        range(
            &mut out,
            "pen_reference_angle_deg",
            self.pen_reference_angle_deg as i64,
            0,
            60,
        );
        // At most 80°: the ricochet test then keeps every accepted impact
        // cosine above cos 80°, with no extra cap.
        range(
            &mut out,
            "ricochet_angle_deg",
            self.ricochet_angle_deg as i64,
            1,
            80,
        );
        if self.penetration.is_empty() {
            out.push(
                Issue::new("penetration", "needs at least one [range_m, mm] row")
                    .with_hint("penetration = [[100, 106], [500, 96]]"),
            );
        }
        for (i, [r, mm]) in self.penetration.iter().enumerate() {
            range(
                &mut out,
                &format!("penetration[{i}][0]"),
                *r as i64,
                0,
                10_000,
            );
            range(
                &mut out,
                &format!("penetration[{i}][1]"),
                *mm as i64,
                1,
                2000,
            );
            if i > 0 && *r <= self.penetration[i - 1][0] {
                out.push(Issue::new(
                    format!("penetration[{i}][0]"),
                    "ranges must be strictly ascending",
                ));
            }
        }
        if self.mechanism == AmmoMechanism::He && self.penetration.len() > 1 {
            out.push(
                Issue::new("penetration", "HE penetration is flat: one row")
                    .with_hint("penetration = [[0, 10]]"),
            );
        }
        range(
            &mut out,
            "nonpen_damage",
            self.nonpen_damage as i64,
            0,
            1000,
        );
        range(&mut out, "track_damage", self.track_damage as i64, 0, 1000);
        let mech = self.mechanism.name();
        let need = |out: &mut Vec<Issue>, key: &str, have: bool, want: bool, example: &str| {
            if have != want {
                let msg = if want {
                    format!("`{mech}` rounds need [{key}]")
                } else {
                    format!("`{mech}` rounds must not have [{key}]")
                };
                let mut i = Issue::new(key, msg);
                if want {
                    i = i.with_hint(format!("[{key}] {example}"));
                }
                out.push(i);
            }
        };
        let pen = self.mechanism.penetrator();
        need(
            &mut out,
            "core",
            self.core.is_some(),
            pen,
            "damage = 100, length_cm = 150, max_boxes = 3",
        );
        need(
            &mut out,
            "spall",
            self.spall.is_some(),
            pen,
            "rays = 6, damage = 35, cone_deg = 25",
        );
        need(
            &mut out,
            "filler",
            self.filler.is_some(),
            self.mechanism == AmmoMechanism::Aphe,
            "offset_cm = 100, rays = 10, cone_deg = 60, damage = 60, reach_cm = 150",
        );
        need(
            &mut out,
            "he",
            self.he.is_some(),
            self.mechanism.bursts(),
            "damage = 150 ...",
        );
        if let Some(c) = &self.core {
            range(&mut out, "core.damage", c.damage as i64, 0, 1000);
            range(&mut out, "core.length_cm", c.length_cm as i64, 1, 500);
            range(&mut out, "core.max_boxes", c.max_boxes as i64, 1, 3);
        }
        if let Some(s) = &self.spall {
            range(
                &mut out,
                "spall.rays",
                s.rays as i64,
                0,
                MAX_SPALL_RAYS as i64,
            );
            range(&mut out, "spall.damage", s.damage as i64, 0, 1000);
            range(&mut out, "spall.cone_deg", s.cone_deg as i64, 1, 60);
        }
        if let Some(f) = &self.filler {
            range(&mut out, "filler.offset_cm", f.offset_cm as i64, 0, 500);
            range(
                &mut out,
                "filler.rays",
                f.rays as i64,
                0,
                MAX_FILLER_RAYS as i64,
            );
            range(&mut out, "filler.cone_deg", f.cone_deg as i64, 1, 89);
            range(&mut out, "filler.damage", f.damage as i64, 0, 1000);
            range(&mut out, "filler.reach_cm", f.reach_cm as i64, 1, 500);
        }
        if let Some(h) = &self.he {
            for i in h.validate() {
                out.push(Issue {
                    path: format!("he.{}", i.path),
                    ..i
                });
            }
            if h.cuts_trenches {
                out.push(Issue::new("he.cuts_trenches", "only satchels cut trenches"));
            }
            // One number per fact: an HE round's plate rule uses its flat
            // penetration; a HEAT round's jet is its penetration and its
            // blast defeats no plate.
            let want = if self.mechanism == AmmoMechanism::He {
                self.flat_pen_mm()
            } else {
                0
            };
            if h.pen_mm != want {
                out.push(
                    Issue::new(
                        "he.pen_mm",
                        format!("must be {want} for a `{mech}` round (got {})", h.pen_mm),
                    )
                    .with_hint(format!("pen_mm = {want}")),
                );
            }
        }
        out
    }

    /// Penetration at range 0 (the flat value of a one-row table).
    pub fn flat_pen_mm(&self) -> u32 {
        self.penetration.first().map_or(0, |r| r[1])
    }
}

// ---------------------------------------------------------- the setups --

/// A point or vector in a vehicle frame, in metres.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FixedVec3 {
    pub x: Fixed,
    pub y: Fixed,
    pub z: Fixed,
}

impl FixedVec3 {
    pub const fn from_cm(v: [i32; 3]) -> FixedVec3 {
        FixedVec3 {
            x: Fixed::from_ratio(v[0] as i64, 100),
            y: Fixed::from_ratio(v[1] as i64, 100),
            z: Fixed::from_ratio(v[2] as i64, 100),
        }
    }
    pub const fn from_cm_u(v: [u32; 3]) -> FixedVec3 {
        FixedVec3::from_cm([v[0] as i32, v[1] as i32, v[2] as i32])
    }
}

/// A box in a frame, in metres.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoxSetup {
    pub center: FixedVec3,
    pub half: FixedVec3,
}

impl From<&BoxDef> for BoxSetup {
    fn from(b: &BoxDef) -> BoxSetup {
        BoxSetup {
            center: FixedVec3::from_cm(b.center),
            half: FixedVec3::from_cm_u(b.half),
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartBoxSetup {
    pub frame: Frame,
    pub bounds: BoxSetup,
}

impl From<&PartBoxDef> for PartBoxSetup {
    fn from(b: &PartBoxDef) -> PartBoxSetup {
        PartBoxSetup {
            frame: b.frame,
            bounds: (&b.bounds()).into(),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorSetup {
    pub frame: Frame,
    pub pos: FixedVec3,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlateSetup {
    /// Millimetres (from tenths, so exact for one decimal).
    pub thickness: Fixed,
    /// Integer degrees from vertical; the sim tilts the face normal by it.
    pub slope_deg: i32,
    pub material: ArmourMaterial,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobilitySetup {
    pub kind: MobilityKind,
    /// m/s.
    pub speed: Fixed,
    pub reverse: Fixed,
    pub turn_deg_s: i32,
    /// S8b: full-lock radius (m; zero unless `Wheeled`).
    pub turn_radius: Fixed,
    /// S8b: percent of speed in woods.
    pub woods_speed_pct: u32,
    /// S8b: percent of speed lost at `max_slope_pct`.
    pub slope_loss_pct: u32,
    /// m/s².
    pub accel: Fixed,
    pub brake: Fixed,
    pub max_slope_pct: u32,
    /// m.
    pub ditch_block: Fixed,
    pub bog_pct: u32,
    pub unbog_ticks: [u32; 2],
    pub moving_spread_pct: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatSetup {
    /// S7: the def's seat id (a presenter label).
    pub id: String,
    pub role: SeatRole,
    pub part: PartBoxSetup,
    /// S7: index of the armour box he sits in.
    pub in_box: u8,
}

/// S7: a resolved armour box (see [`ArmourBoxDef`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArmourBoxSetup {
    pub id: String,
    pub frame: Frame,
    /// In its frame (turret frame from the pivot), metres.
    pub bounds: BoxSetup,
    /// By [`PlateGroup::index`]; an open group's plate is zero.
    pub plates: [PlateSetup; PLATE_GROUPS],
    /// Open groups, bit [`PlateGroup::index`].
    pub open: u8,
    /// S8b: open groups with a grenade screen (a subset of `open`).
    pub screen: u8,
    /// The floor items rest on (frame z, m); the box bottom when closed.
    pub floor: Fixed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmmoLoadSetup {
    /// Index into `MatchSetup::ammo`.
    pub ammo: u16,
    pub count: u16,
    /// S5a: a rare round (its ammo def's `rare`; D3).
    pub rare: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MountSetup {
    /// Index into `MatchSetup::weapons`.
    pub weapon: u16,
    pub kind: MountKind,
    /// Turret frame from the pivot (`TurretMain`, `Coax`); hull frame
    /// (`Hull`).
    pub muzzle: FixedVec3,
    /// `TurretMain`: [depression, elevation] in integer degrees; 0 others.
    pub elevation_deg: [i32; 2],
    /// `TurretMain`: rounds carried (penetrator first, then HE, then the
    /// special penetrator, S3b).
    pub ammo: Vec<AmmoLoadSetup>,
    /// `TurretMain` (S3b): the exposed barrel's length and half-width, m
    /// (0 others).
    pub barrel_length: Fixed,
    pub barrel_radius: Fixed,
    /// `Hull`: traverse either side, integer degrees; 0 others.
    pub arc_deg: i32,
    pub seat: SeatRole,
    /// `Coax`, `Hull` (S3b Fix): rounds stowed; `None` unlimited.
    pub rounds: Option<u32>,
    /// S8a `Pintle`: the pivot in the hull frame (the muzzle is from it,
    /// turned by the mount's yaw), the arc `[min, max]` in integer degrees
    /// from the hull front (positive left) and the traverse rate (deg/s);
    /// zero for other kinds.
    pub pivot: FixedVec3,
    pub traverse_deg: [i32; 2],
    pub traverse_deg_s: i32,
}

/// S8a: a resolved MG shield (see [`ShieldDef`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShieldSetup {
    /// Index into `VehicleSetup::mounts` of the pintle it turns with
    /// (`None`: hull frame).
    pub mount: Option<u8>,
    /// In the mount's frame (from its pivot) or the hull frame, m.
    pub bounds: BoxSetup,
    /// mm (RHA).
    pub thickness: Fixed,
}

/// S8a: a resolved `[transport]` (see [`TransportDef`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportSetup {
    /// Door, hull frame, m.
    pub entry: FixedVec3,
    pub board_ticks: u32,
    pub exit_ticks: u32,
    /// `None`: no side exits.
    pub exit_over_sides_ticks: Option<u32>,
    pub exit_raise: Fixed,
    /// Passenger slot boxes (hull frame, m); `len()` is the capacity.
    pub slots: Vec<BoxSetup>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairSetup {
    pub to_hp: u32,
    /// Ticks to repair from 0 HP to `to_hp`.
    pub ticks: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentSetup {
    pub kind: ComponentKind,
    pub hp: u32,
    /// Its boxes (S3b: several for stowage bins; none for tracks).
    pub parts: Vec<PartBoxSetup>,
    pub anchors: Vec<AnchorSetup>,
    pub repair: Option<RepairSetup>,
}

/// S3c: a resolved hatch (see [`HatchDef`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HatchSetup {
    /// Seat indices (the def's seat order) in exit order.
    pub seats: Vec<u8>,
    pub frame: Frame,
    /// The opening in its frame (m; turret frame from the pivot).
    pub pos: FixedVec3,
    pub exit_ticks: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StationSetup {
    pub id: String,
    pub seats: Vec<SeatRole>,
}

/// A resolved vehicle type (see [`VehicleDef`]). Lengths in metres
/// ([`Fixed`]), angles in integer degrees, durations in ticks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VehicleSetup {
    pub id: String,
    pub name: String,
    pub short_name: String,
    /// Index into `MatchSetup::factions`.
    pub faction: u16,
    pub role: VehicleRole,
    pub cost: u32,
    pub pop: u32,
    /// Re-call cooldown and the cooldown from the match start (ticks).
    pub recall_cooldown_ticks: u32,
    pub initial_cooldown_ticks: u32,
    pub sight: Fixed,
    pub spot_size_pct: u32,
    pub eye_height: Fixed,
    pub mobility: MobilitySetup,
    /// Box 0's bounds (the running-gear hull, hull frame).
    pub hull: BoxSetup,
    pub track_band_pct: u32,
    pub track_mm: u32,
    /// The gun frame's pivot in the hull frame: the turret's, or (S7) the
    /// casemate gun's.
    pub turret_pivot: FixedVec3,
    /// The turret box in the turret frame; S7: a zero box at the pivot for
    /// a casemate gun (the gun frame keeps its role, no armour).
    pub turret: BoxSetup,
    /// Traverse rate of the turret or the casemate gun (deg/s).
    pub traverse_deg_s: i32,
    /// Main gun elevation rate (deg/s).
    pub elevate_deg_s: i32,
    /// S7: the vehicle has a turret (a turret ring, full traverse).
    pub has_turret: bool,
    /// S7: a casemate gun's arc `[min, max]` in integer degrees from the
    /// hull's front, positive left; `None` with a turret.
    pub gun_arc_deg: Option<[i32; 2]>,
    /// S7: the armour boxes (1..=4; box 0 the running-gear hull). Zone id
    /// = box index × 4 + [`PlateGroup::index`].
    pub boxes: Vec<ArmourBoxSetup>,
    /// In the def's seat order.
    pub seats: Vec<SeatSetup>,
    /// Main gun first, then the def's order.
    pub mounts: Vec<MountSetup>,
    /// By [`ComponentKind::index`].
    pub components: Vec<ComponentSetup>,
    pub stations: Vec<StationSetup>,
    /// S3c: the crew squad type (index into `MatchSetup::squad_types`);
    /// filled in by the setup after the squad list is built.
    pub crew_squad: Option<u16>,
    /// S3c: hatches (every seat in one).
    pub hatches: Vec<HatchSetup>,
    /// S5a (D3): the most rare rounds it holds (filled in by the setup).
    pub rare_cap: u32,
    /// S5a (D4): main-gun rounds cost this share of their shell price (%;
    /// filled in by the setup: a full rack costs at most the rules'
    /// `rack_refill_max_card_pct` of the card).
    pub rack_price_pct: u32,
    /// S8a: MG shields.
    pub shields: Vec<ShieldSetup>,
    /// S8a: an infantry carrier's door, timings and slots.
    pub transport: Option<TransportSetup>,
    /// S8b (6.9): ticks stopped before `stationary_sight_pct` applies.
    pub settle_ticks: u32,
    /// S8b (6.9): percent of `sight` once settled (100: no effect).
    pub stationary_sight_pct: u32,
    /// S8b (section 7): the supply truck's stock; `None` on every other
    /// role.
    pub supply: Option<TruckSupplySetup>,
}

/// S8b: a resolved `[supply]` (see [`TruckSupplyDef`]). Arrays are by
/// `SupplyKind::index`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TruckSupplySetup {
    /// Most grams of each kind.
    pub cap_g: u32,
    /// Grams it takes on in base.
    pub load_g: [u32; 3],
    pub radius: Fixed,
    pub vehicle_radius: Fixed,
    pub round_interval_ticks: u32,
    /// Index into `MatchSetup::stocks` of the crate it drops per kind.
    pub crates: [Option<u16>; 3],
    /// Index into `MatchSetup::ammo` (a round with a blast).
    pub cookoff: u16,
}

impl VehicleSetup {
    /// The main gun (validated setups have it first); S8a-1: `None` on a
    /// transport without one.
    pub fn main_gun(&self) -> Option<&MountSetup> {
        self.mounts.first().filter(|m| m.kind.is_main())
    }

    /// S8b: whether the def has part `k` (an absent part reads 0 HP and is
    /// never damaged or tested).
    pub fn has_part(&self, k: ComponentKind) -> bool {
        self.components.get(k.index()).is_some_and(|c| c.hp > 0)
    }

    /// The main gun's rounds (S8a: empty without one).
    pub fn main_ammo(&self) -> &[AmmoLoadSetup] {
        self.main_gun().map_or(&[], |g| &g.ammo)
    }

    /// S8a: the weapon a card shows (the main gun, else the first MG).
    pub fn primary_weapon(&self) -> u16 {
        self.mounts.first().map_or(0, |m| m.weapon)
    }

    /// S8a: the MG mounts (coax, hull, pintle), after the main gun, in the
    /// order of `VehicleState::mgs`.
    pub fn mg_mounts(&self) -> &[MountSetup] {
        &self.mounts[self.main_gun().is_some() as usize..]
    }

    /// The plate of zone id `z` (`None`: no such box).
    pub fn plate(&self, z: u8) -> Option<&PlateSetup> {
        self.boxes
            .get(z as usize / PLATE_GROUPS)
            .map(|b| &b.plates[z as usize % PLATE_GROUPS])
    }

    /// Whether zone `z` is an open face (S7).
    pub fn zone_open(&self, z: u8) -> bool {
        self.boxes
            .get(z as usize / PLATE_GROUPS)
            .is_some_and(|b| b.open & (1 << (z as usize % PLATE_GROUPS)) != 0)
    }

    /// Zone count (`boxes × 4`).
    pub fn zones(&self) -> usize {
        self.boxes.len() * PLATE_GROUPS
    }

    /// Zone `z`'s name, `<box id>_<group>` (S7-9).
    pub fn zone_name(&self, z: u8) -> String {
        match self.boxes.get(z as usize / PLATE_GROUPS) {
            Some(b) => format!("{}_{}", b.id, PlateGroup::of_zone(z).name()),
            None => format!("zone{z}"),
        }
    }

    /// The zone id named `name` (`hull_front`, `casemate_side`).
    pub fn zone_by_name(&self, name: &str) -> Option<u8> {
        (0..self.zones() as u8).find(|&z| self.zone_name(z) == name)
    }

    /// S7: the upper armour box: the turret box, else the box whose top is
    /// highest (ties: the lower index), box 0 alone. Aim points and
    /// spotting read it where S3 read the turret.
    pub fn upper_box(&self) -> usize {
        if let Some(k) = self.boxes.iter().position(|b| b.frame == Frame::Turret) {
            return k;
        }
        let top = |b: &ArmourBoxSetup| b.bounds.center.z.0 as i64 + b.bounds.half.z.0 as i64;
        let mut best = 0;
        for (k, b) in self.boxes.iter().enumerate().skip(1) {
            if best == 0 || top(b) > top(&self.boxes[best]) {
                best = k;
            }
        }
        best
    }

    /// The seat for `role`, if this vehicle has one.
    pub fn seat(&self, role: SeatRole) -> Option<&SeatSetup> {
        self.seats.iter().find(|s| s.role == role)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PenRowSetup {
    /// m.
    pub range: Fixed,
    pub mm: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreSetup {
    pub damage: u32,
    pub length: Fixed,
    pub max_boxes: u8,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpallSetup {
    pub rays: u8,
    pub damage: u32,
    pub cone_deg: i32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FillerSetup {
    pub offset: Fixed,
    pub rays: u8,
    pub cone_deg: i32,
    pub damage: u32,
    pub reach: Fixed,
}

/// A resolved round (see [`AmmoDef`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmmoSetup {
    pub id: String,
    pub name: String,
    pub mechanism: AmmoMechanism,
    pub caliber_mm: u32,
    /// Muzzle speed, m/s.
    pub speed: Fixed,
    /// Ascending range; at the reference angle (not yet normalised).
    pub penetration: Vec<PenRowSetup>,
    pub pen_ref_deg: i32,
    pub ricochet_deg: i32,
    pub nonpen_damage: u32,
    pub track_damage: u32,
    pub core: Option<CoreSetup>,
    pub spall: Option<SpallSetup>,
    pub filler: Option<FillerSetup>,
    /// `He` and `Heat`: the contact burst. For `He`, `blast.pen_mm` equals
    /// the round's flat penetration; for `Heat` it is 0 (the jet is the
    /// penetration).
    pub blast: Option<ItemHeSetup>,
    /// S5a: weight and supply kind (priced refill).
    pub supply: Option<crate::setup::SupplySetup>,
    /// S5a: a rare round (D3).
    pub rare: bool,
}

// ------------------------------------------------------------- lowering --

pub(crate) fn lower_ammo(a: &AmmoDef) -> AmmoSetup {
    AmmoSetup {
        id: a.id.clone(),
        name: a.name.clone(),
        mechanism: a.mechanism,
        caliber_mm: a.caliber_mm,
        speed: Fixed::from_int(a.muzzle_velocity_m_s as i32),
        penetration: a
            .penetration
            .iter()
            .map(|r| PenRowSetup {
                range: Fixed::from_int(r[0] as i32),
                mm: r[1],
            })
            .collect(),
        pen_ref_deg: a.pen_reference_angle_deg as i32,
        ricochet_deg: a.ricochet_angle_deg as i32,
        nonpen_damage: a.nonpen_damage,
        track_damage: a.track_damage,
        core: a.core.map(|c| CoreSetup {
            damage: c.damage,
            length: Fixed::from_cm(c.length_cm),
            max_boxes: c.max_boxes as u8,
        }),
        spall: a.spall.map(|s| SpallSetup {
            rays: s.rays as u8,
            damage: s.damage,
            cone_deg: s.cone_deg as i32,
        }),
        filler: a.filler.map(|f| FillerSetup {
            offset: Fixed::from_cm(f.offset_cm),
            rays: f.rays.min(MAX_FILLER_RAYS) as u8,
            cone_deg: f.cone_deg as i32,
            damage: f.damage,
            reach: Fixed::from_cm(f.reach_cm),
        }),
        blast: a.he.as_ref().map(crate::setup::lower_item_he),
        supply: a.supply.as_ref().map(|sp| crate::setup::SupplySetup {
            kind: sp.kind(),
            g: sp.g,
        }),
        rare: a.rare,
    }
}

/// Lowers one vehicle. `weapon`, `ammo` and `faction` map ids to setup
/// indices (validated defs resolve every id).
pub(crate) fn lower_vehicle(
    v: &VehicleDef,
    hz: u32,
    faction: u16,
    weapon: impl Fn(&str) -> u16,
    ammo: impl Fn(&str) -> u16,
    ammo_def: impl Fn(&str) -> Option<AmmoMechanism>,
) -> VehicleSetup {
    let m = &v.mobility;
    let main = v.main_gun();
    let mut mounts: Vec<&MountDef> = v.weapons.iter().collect();
    // Main gun first; the rest keep the def's order.
    mounts.sort_by_key(|w| !w.mount.is_main());
    let mount = |w: &MountDef| {
        // Penetrators in the def's order, the HE round second (S3b: the
        // standard penetrator, HE, then a special penetrator).
        let mut loads: Vec<&AmmoLoadDef> = w
            .ammo
            .iter()
            .filter(|a| ammo_def(&a.id) != Some(AmmoMechanism::He))
            .collect();
        if let Some(he) = w
            .ammo
            .iter()
            .find(|a| ammo_def(&a.id) == Some(AmmoMechanism::He))
        {
            loads.insert(1.min(loads.len()), he);
        }
        MountSetup {
            weapon: weapon(&w.def),
            kind: w.mount,
            muzzle: FixedVec3::from_cm(w.muzzle),
            elevation_deg: w.elevation_deg.unwrap_or([0, 0]),
            ammo: loads
                .into_iter()
                .map(|a| AmmoLoadSetup {
                    ammo: ammo(&a.id),
                    count: a.count.min(u16::MAX as u32) as u16,
                    rare: false,
                })
                .collect(),
            arc_deg: w.arc_deg.unwrap_or(0) as i32,
            seat: w.seat.unwrap_or(SeatRole::Gunner),
            rounds: w.rounds,
            barrel_length: w
                .barrel
                .map_or(Fixed::ZERO, |b| Fixed::from_cm(b.length_cm)),
            barrel_radius: w
                .barrel
                .map_or(Fixed::ZERO, |b| Fixed::from_cm(b.radius_cm)),
            pivot: if w.mount == MountKind::Pintle {
                FixedVec3::from_cm(w.pivot.unwrap_or([0, 0, 0]))
            } else {
                FixedVec3::default()
            },
            traverse_deg: if w.mount == MountKind::Pintle {
                w.traverse_deg.unwrap_or([0, 0])
            } else {
                [0, 0]
            },
            traverse_deg_s: if w.mount == MountKind::Pintle {
                w.traverse_deg_s.unwrap_or(1) as i32
            } else {
                0
            },
        }
    };
    let component = |k: ComponentKind| {
        let Some(c) = v.components.iter().find(|c| c.kind == k) else {
            // S7: no turret ring without a turret: 0 HP, no box; the sim
            // never reads it.
            return ComponentSetup {
                kind: k,
                hp: 0,
                parts: Vec::new(),
                anchors: Vec::new(),
                repair: None,
            };
        };
        ComponentSetup {
            kind: k,
            hp: c.hp,
            parts: c
                .bounds
                .iter()
                .chain(&c.boxes)
                .map(PartBoxSetup::from)
                .collect(),
            anchors: c
                .anchors
                .iter()
                .map(|a| AnchorSetup {
                    frame: a.frame,
                    pos: FixedVec3::from_cm(a.pos),
                })
                .collect(),
            repair: c.repair.map(|r| RepairSetup {
                to_hp: r.to_hp,
                ticks: r.time_s * hz,
            }),
        }
    };
    VehicleSetup {
        id: v.id.clone(),
        name: v.name.clone(),
        short_name: v.short_name.clone().unwrap_or_else(|| v.name.clone()),
        faction,
        role: v.role,
        cost: v.cost,
        pop: v.pop,
        recall_cooldown_ticks: v.recall_cooldown_s * hz,
        initial_cooldown_ticks: v.initial_cooldown_s * hz,
        sight: Fixed::from_int(v.sight_m as i32),
        spot_size_pct: v.spot_size_pct,
        eye_height: Fixed::from_cm(v.eye_height_cm),
        mobility: MobilitySetup {
            kind: m.kind,
            speed: Fixed::from_cm(m.speed_cm_s),
            reverse: Fixed::from_cm(m.reverse_cm_s),
            turn_deg_s: m.turn_deg_s as i32,
            turn_radius: Fixed::from_cm(m.turn_radius_cm.unwrap_or(0)),
            woods_speed_pct: m.woods_speed_pct.unwrap_or(50),
            slope_loss_pct: m.slope_loss_pct.unwrap_or(70),
            accel: Fixed::from_cm(m.accel_cm_s2),
            brake: Fixed::from_cm(m.brake_cm_s2),
            max_slope_pct: m.max_slope_pct,
            ditch_block: Fixed::from_ratio(m.ditch_block_dm as i64, 10),
            bog_pct: m.bog_pct,
            unbog_ticks: [m.unbog_s[0] * hz, m.unbog_s[1] * hz],
            moving_spread_pct: m.moving_spread_pct,
        },
        hull: v
            .boxes
            .first()
            .map_or_else(BoxSetup::default, |b| (&b.bounds).into()),
        track_band_pct: v.hull.track_band_pct,
        track_mm: v.hull.track_mm,
        turret_pivot: FixedVec3::from_cm(match (&v.turret, main) {
            (Some(t), _) => t.pivot,
            (None, Some(m)) => m.pivot.unwrap_or([0, 0, 0]),
            (None, None) => [0, 0, 0],
        }),
        turret: v
            .boxes
            .iter()
            .find(|b| b.frame == Frame::Turret)
            .map_or_else(BoxSetup::default, |b| (&b.bounds).into()),
        traverse_deg_s: match (&v.turret, main) {
            (Some(t), _) => t.traverse_deg_s as i32,
            (None, Some(m)) => m.traverse_deg_s.unwrap_or(1) as i32,
            (None, None) => 1,
        },
        elevate_deg_s: match (&v.turret, main) {
            (Some(t), _) => t.elevate_deg_s as i32,
            (None, Some(m)) => m.elevate_deg_s.unwrap_or(1) as i32,
            (None, None) => 1,
        },
        has_turret: v.turret.is_some(),
        gun_arc_deg: match (&v.turret, main) {
            (None, Some(m)) => m.traverse_deg,
            _ => None,
        },
        boxes: v
            .boxes
            .iter()
            .map(|b| {
                let open = b.open.iter().fold(0u8, |m, g| m | (1 << g.index()));
                ArmourBoxSetup {
                    id: b.id.clone(),
                    frame: b.frame,
                    bounds: (&b.bounds).into(),
                    plates: PlateGroup::ALL.map(|g| {
                        b.armor
                            .get(g)
                            .filter(|_| open & (1 << g.index()) == 0)
                            .map_or(PlateSetup::default(), |p| PlateSetup {
                                thickness: Fixed::from_ratio(p.thickness_mm.0 as i64, 10),
                                slope_deg: p.slope_deg as i32,
                                material: p.material,
                            })
                    }),
                    open,
                    screen: b
                        .grenade_screen
                        .iter()
                        .fold(0u8, |m, g| m | (1 << g.index())),
                    // The box bottom unless set (the same fixed-point
                    // arithmetic as the bounds, so it lies in them).
                    floor: {
                        let bb: BoxSetup = (&b.bounds).into();
                        let (lo, hi) = (
                            Fixed(bb.center.z.0 - bb.half.z.0),
                            Fixed(bb.center.z.0 + bb.half.z.0),
                        );
                        b.floor_cm
                            .map_or(lo, |f| Fixed::from_ratio(f as i64, 100).clamp(lo, hi))
                    },
                }
            })
            .collect(),
        seats: v
            .seats
            .iter()
            .map(|st| SeatSetup {
                id: st.id.clone(),
                role: st.role,
                part: (&st.bounds).into(),
                in_box: st
                    .in_box
                    .as_ref()
                    .and_then(|b| v.boxes.iter().position(|x| &x.id == b))
                    .unwrap_or(0) as u8,
            })
            .collect(),
        shields: v
            .shields
            .iter()
            .map(|sh| ShieldSetup {
                // The def's weapon index, mapped to the sorted mount list.
                mount: sh.mount.and_then(|k| {
                    let w = v.weapons.get(k)?;
                    mounts
                        .iter()
                        .position(|m| std::ptr::eq(*m, w))
                        .map(|i| i as u8)
                }),
                bounds: (&sh.bounds).into(),
                thickness: Fixed::from_ratio(sh.thickness_mm.0 as i64, 10),
            })
            .collect(),
        settle_ticks: v.optics.map_or(0, |o| o.settle_s * hz),
        stationary_sight_pct: v.optics.map_or(100, |o| o.stationary_sight_pct),
        // Crate indices are filled by `lower_vehicles`, which knows the
        // supply defs.
        supply: v.supply.as_ref().map(|t| {
            let mut load_g = [0u32; 3];
            for (k, kg) in &t.load_kg {
                if let Some(kind) = crate::defs::SupplyKind::from_name(k) {
                    load_g[kind.index()] = kg.saturating_mul(1000);
                }
            }
            TruckSupplySetup {
                cap_g: t.cap_kg_total.saturating_mul(1000),
                load_g,
                radius: Fixed::from_int(t.radius_m as i32),
                vehicle_radius: Fixed::from_int(t.vehicle_radius_m as i32),
                round_interval_ticks: t.round_interval_s.saturating_mul(hz),
                crates: [None; 3],
                cookoff: ammo(&t.cookoff),
            }
        }),
        transport: v.transport.as_ref().map(|t| TransportSetup {
            entry: FixedVec3::from_cm(t.entry),
            board_ticks: (t.board_ms as u64 * hz as u64).div_ceil(1000) as u32,
            exit_ticks: (t.exit_ms as u64 * hz as u64).div_ceil(1000) as u32,
            exit_over_sides_ticks: t
                .exit_over_sides_ms
                .map(|ms| (ms as u64 * hz as u64).div_ceil(1000) as u32),
            exit_raise: Fixed::from_cm(t.exit_raise_cm),
            slots: v
                .passenger_slots
                .iter()
                .map(|sl| (&sl.bounds).into())
                .collect(),
        }),
        mounts: mounts.into_iter().map(mount).collect(),
        components: ComponentKind::ALL.into_iter().map(component).collect(),
        stations: v
            .stations
            .iter()
            .map(|s| StationSetup {
                id: s.id.clone(),
                seats: s.seats.clone(),
            })
            .collect(),
        crew_squad: None,
        hatches: v
            .hatches
            .iter()
            .map(|h| HatchSetup {
                seats: h
                    .seats
                    .iter()
                    .filter_map(|r| v.seats.iter().position(|x| x.role == *r))
                    .map(|k| k as u8)
                    .collect(),
                frame: h.frame,
                pos: FixedVec3::from_cm(h.pos),
                exit_ticks: (h.exit_ms as u64 * hz as u64).div_ceil(1000) as u32,
            })
            .collect(),
        rare_cap: 0,
        rack_price_pct: 100,
    }
}

// ------------------------------------------------------ setup checks --

fn fx_in(v: Fixed, lo: i32, hi: i32) -> bool {
    v >= Fixed::from_int(lo) && v <= Fixed::from_int(hi)
}

fn box_ok(b: &BoxSetup) -> bool {
    let h = [b.half.x, b.half.y, b.half.z];
    let c = [b.center.x, b.center.y, b.center.z];
    h.iter().all(|&v| v > Fixed::ZERO && fx_in(v, 0, 20)) && c.iter().all(|&v| fx_in(v, -20, 20))
}

/// The turret box extended down to the hull floor (turret frame): where
/// a turret-frame part may lie (S3b Tune, the turret basket).
fn basket_def(turret: &BoxDef, pivot_z: i32, hull: &BoxDef) -> BoxDef {
    let lo = (hull.lo(2) - pivot_z as i64).min(turret.lo(2));
    let hi = turret.hi(2);
    BoxDef {
        center: [
            turret.center[0],
            turret.center[1],
            ((lo + hi).div_euclid(2)) as i32,
        ],
        half: [turret.half[0], turret.half[1], ((hi - lo + 1) / 2) as u32],
    }
}

/// The bounds of every hull-frame armour box (S7: a part may lie in any).
fn hull_bounds(v: &VehicleSetup) -> BoxSetup {
    let mut lo = [i64::MAX; 3];
    let mut hi = [i64::MIN; 3];
    for b in v.boxes.iter().filter(|b| b.frame == Frame::Hull) {
        let c = [b.bounds.center.x, b.bounds.center.y, b.bounds.center.z];
        let h = [b.bounds.half.x, b.bounds.half.y, b.bounds.half.z];
        for a in 0..3 {
            lo[a] = lo[a].min(c[a].0 as i64 - h[a].0 as i64);
            hi[a] = hi[a].max(c[a].0 as i64 + h[a].0 as i64);
        }
    }
    if lo[0] > hi[0] {
        return v.hull;
    }
    let ctr = |a: usize| Fixed(((lo[a] + hi[a]).div_euclid(2)) as i32);
    let half = |a: usize| Fixed(((hi[a] - lo[a] + 1) / 2) as i32);
    BoxSetup {
        center: FixedVec3 {
            x: ctr(0),
            y: ctr(1),
            z: ctr(2),
        },
        half: FixedVec3 {
            x: half(0),
            y: half(1),
            z: half(2),
        },
    }
}

/// [`basket_def`] for a resolved vehicle (fixed point).
fn basket(v: &VehicleSetup) -> BoxSetup {
    let t = &v.turret;
    let hull_lo = v.hull.center.z.0 as i64 - v.hull.half.z.0 as i64;
    let lo = (hull_lo - v.turret_pivot.z.0 as i64).min(t.center.z.0 as i64 - t.half.z.0 as i64);
    let hi = t.center.z.0 as i64 + t.half.z.0 as i64;
    let mut b = *t;
    b.center.z = Fixed(((lo + hi).div_euclid(2)) as i32);
    b.half.z = Fixed(((hi - lo + 1) / 2) as i32);
    b
}

fn inside(outer: &BoxSetup, inner: &BoxSetup) -> bool {
    let o = [
        (outer.center.x, outer.half.x),
        (outer.center.y, outer.half.y),
        (outer.center.z, outer.half.z),
    ];
    let i = [
        (inner.center.x, inner.half.x),
        (inner.center.y, inner.half.y),
        (inner.center.z, inner.half.z),
    ];
    o.iter().zip(&i).all(|(&(oc, oh), &(ic, ih))| {
        let (oc, oh, ic, ih) = (oc.0 as i64, oh.0 as i64, ic.0 as i64, ih.0 as i64);
        ic - ih >= oc - oh && ic + ih <= oc + oh
    })
}

/// Structural checks of resolved ammo (saves are untrusted): everything
/// the sim's armour tables and penetration code rely on.
pub(crate) fn check_ammo(a: &AmmoSetup) -> Option<&'static str> {
    let rows_ok = !a.penetration.is_empty()
        && a.penetration.windows(2).all(|w| w[0].range < w[1].range)
        && a.penetration
            .iter()
            .all(|r| fx_in(r.range, 0, 10_000) && (1..=2000).contains(&r.mm));
    if !rows_ok {
        return Some("ammo.penetration: empty, unsorted or out of range");
    }
    if !fx_in(a.speed, 1, 3000)
        || !(0..=60).contains(&a.pen_ref_deg)
        || !(1..=80).contains(&a.ricochet_deg)
        || a.nonpen_damage > 1000
        || a.track_damage > 1000
    {
        return Some("ammo.values: out of range");
    }
    let pen = a.mechanism.penetrator();
    if a.core.is_some() != pen
        || a.spall.is_some() != pen
        || a.filler.is_some() != (a.mechanism == AmmoMechanism::Aphe)
        || a.blast.is_some() != a.mechanism.bursts()
    {
        return Some("ammo.mechanism: parts disagree with the mechanism");
    }
    if let Some(c) = &a.core
        && (c.damage > 1000 || !fx_in(c.length, 0, 5) || !(1..=3).contains(&c.max_boxes))
    {
        return Some("ammo.core: out of range");
    }
    if let Some(s) = &a.spall
        && (s.rays as u32 > MAX_SPALL_RAYS || s.damage > 1000 || !(1..=60).contains(&s.cone_deg))
    {
        return Some("ammo.spall: out of range");
    }
    if let Some(f) = &a.filler
        && (f.damage > 1000
            || !fx_in(f.offset, 0, 5)
            || !fx_in(f.reach, 0, 5)
            || f.rays as u32 > MAX_FILLER_RAYS
            || !(1..=89).contains(&f.cone_deg))
    {
        return Some("ammo.filler: out of range");
    }
    if let Some(b) = &a.blast {
        let pen = match a.mechanism {
            AmmoMechanism::He => a.penetration[0].mm,
            _ => 0,
        };
        if !crate::setup::item_he_ok(b) || b.blast.pen_mm != pen || b.cuts_trenches {
            return Some("ammo.blast: out of range");
        }
    }
    None
}

/// Structural checks of a resolved vehicle against the setup's sizes.
pub(crate) fn check_vehicle(
    v: &VehicleSetup,
    weapons: &[crate::setup::WeaponSetup],
    ammo: &[AmmoSetup],
    factions: usize,
) -> Option<&'static str> {
    use crate::defs::WeaponClass;
    if v.faction as usize >= factions {
        return Some("vehicles.faction: index out of range");
    }
    let m = &v.mobility;
    let turn_ok = if m.kind == MobilityKind::Wheeled {
        fx_in(m.turn_radius, 3, 20) && m.turn_radius > Fixed::ZERO && m.turn_deg_s == 0
    } else {
        (1..=360).contains(&m.turn_deg_s) && m.turn_radius == Fixed::ZERO
    };
    let optics_ok = v.settle_ticks <= 60 * 30 && (100..=300).contains(&v.stationary_sight_pct);
    if !optics_ok {
        return Some("vehicles.optics: out of range");
    }
    if !(1..=120).contains(&v.pop)
        || v.cost > 100_000
        || !fx_in(v.sight, 1, 2000)
        || !(1..=1000).contains(&v.spot_size_pct)
        || !fx_in(v.eye_height, 0, 10)
        || !(m.speed > Fixed::ZERO && fx_in(m.speed, 0, 30))
        || !(m.reverse > Fixed::ZERO && m.reverse <= m.speed)
        || !turn_ok
        || !(1..=100).contains(&m.woods_speed_pct)
        || m.slope_loss_pct > 95
        || !(m.accel > Fixed::ZERO && fx_in(m.accel, 0, 20))
        || !(m.brake > Fixed::ZERO && fx_in(m.brake, 0, 50))
        || !(1..=MAX_VEHICLE_SLOPE_PCT).contains(&m.max_slope_pct)
        || !(m.ditch_block > Fixed::ZERO && fx_in(m.ditch_block, 0, 10))
        || m.bog_pct > 100
        || m.unbog_ticks[0] > m.unbog_ticks[1]
        || m.unbog_ticks[1] > 600 * 30
        || !(100..=1000).contains(&m.moving_spread_pct)
        || v.recall_cooldown_ticks > crate::MAX_COOLDOWN_S * 30
        || v.initial_cooldown_ticks > crate::MAX_INITIAL_COOLDOWN_S * 30
    {
        return Some("vehicles.values: out of range");
    }
    let casemate = v.mounts.first().map(|m| m.kind) == Some(MountKind::Casemate);
    let turret_ok = if v.has_turret {
        box_ok(&v.turret) && !casemate && v.gun_arc_deg.is_none()
    } else if v.main_gun().is_none() {
        // S8a-1: no main gun (a transport).
        v.turret == BoxSetup::default()
            && v.gun_arc_deg.is_none()
            && matches!(v.role, VehicleRole::Transport | VehicleRole::Truck)
    } else {
        v.turret == BoxSetup::default()
            && casemate
            && v.gun_arc_deg.is_some_and(|[lo, hi]| {
                (-90..=0).contains(&lo) && (0..=90).contains(&hi) && lo < hi
            })
    };
    if !box_ok(&v.hull)
        || !turret_ok
        || v.track_band_pct > 100
        || v.track_mm > 500
        || !(1..=360).contains(&v.traverse_deg_s)
        || !(1..=90).contains(&v.elevate_deg_s)
        || !fx_in(v.turret_pivot.z, 0, 20)
        || v.hull.center.z.0 < v.hull.half.z.0
    {
        return Some("vehicles.boxes: out of range");
    }
    // S7: armour boxes.
    let turret_boxes = v.boxes.iter().filter(|b| b.frame == Frame::Turret).count();
    if v.boxes.is_empty()
        || v.boxes.len() > MAX_ARMOUR_BOXES
        || v.boxes[0].frame != Frame::Hull
        || v.boxes[0].bounds != v.hull
        || v.boxes[0].open != 0
        || turret_boxes != v.has_turret as usize
        || v.boxes
            .iter()
            .any(|b| b.frame == Frame::Turret && b.bounds != v.turret)
    {
        return Some("vehicles.boxes: count, frames or bounds wrong");
    }
    for b in &v.boxes {
        let lo = b.bounds.center.z.0 as i64 - b.bounds.half.z.0 as i64;
        let hi = b.bounds.center.z.0 as i64 + b.bounds.half.z.0 as i64;
        if !box_ok(&b.bounds)
            || b.open >= 1 << PLATE_GROUPS
            || b.screen & !b.open != 0
            || b.id.is_empty()
            || b.id.len() > 24
            || !(lo..=hi).contains(&(b.floor.0 as i64))
        {
            return Some("vehicles.boxes: box, open faces or floor out of range");
        }
        for (g, p) in PlateGroup::ALL.iter().zip(&b.plates) {
            let open = b.open & (1 << g.index()) != 0;
            let slope_ok = if g.is_top() {
                p.slope_deg == 0
            } else {
                (0..=80).contains(&p.slope_deg)
            };
            let thick_ok = if open {
                p.thickness == Fixed::ZERO && p.slope_deg == 0
            } else {
                p.thickness >= Fixed::from_int(1) && p.thickness <= Fixed::from_int(500)
            };
            if !thick_ok || !slope_ok {
                return Some("vehicles.armour: plate out of range");
            }
        }
    }
    let basket = if v.has_turret { basket(v) } else { v.turret };
    // Parts lie within their frame's bounds: hull frame, every hull-frame
    // box; S7: plus EXPOSED_CM above an open-topped box.
    let exposed = |f: Frame| {
        v.boxes
            .iter()
            .any(|b| b.frame == f && b.open & (1 << PlateGroup::Top.index()) != 0)
    };
    let grow = |mut b: BoxSetup, f: Frame| {
        if exposed(f) {
            let e = Fixed::from_ratio(EXPOSED_CM, 100);
            b.half.z = Fixed(b.half.z.0 + e.0 / 2);
            b.center.z = Fixed(b.center.z.0 + e.0 / 2);
        }
        b
    };
    let hull_frame = grow(hull_bounds(v), Frame::Hull);
    let turret_frame = grow(basket, Frame::Turret);
    let frame = |f: Frame| match f {
        Frame::Hull => &hull_frame,
        Frame::Turret => &turret_frame,
    };
    if v.seats.is_empty()
        || v.seats.len() > SEAT_ROLES
        || v.seats
            .iter()
            .enumerate()
            .any(|(i, s)| v.seats[..i].iter().any(|o| o.role == s.role))
        || v.seat(SeatRole::Driver).is_none()
        || (v.main_gun().is_some() && v.seat(SeatRole::Gunner).is_none())
        || v.seats.iter().any(|s| {
            !box_ok(&s.part.bounds)
                || (s.part.frame == Frame::Turret && !v.has_turret)
                || !inside(frame(s.part.frame), &s.part.bounds)
                || s.in_box as usize >= v.boxes.len()
                || s.id.is_empty()
                || s.id.len() > 24
        })
    {
        return Some("vehicles.seats: missing, repeated or outside the frame");
    }
    if (v.mounts.is_empty() && v.role != VehicleRole::Truck) || v.mounts.len() > MAX_MOUNTS {
        return Some("vehicles.mounts: count out of range");
    }
    if v.supply.is_some() != (v.role == VehicleRole::Truck) {
        return Some("vehicles.supply: exactly on role truck");
    }
    if let Some(t) = &v.supply {
        let g_ok = (1..=10_000_000).contains(&t.cap_g) && t.load_g.iter().all(|g| *g <= t.cap_g);
        let r_ok = fx_in(t.radius, 0, 50)
            && t.radius > Fixed::ZERO
            && fx_in(t.vehicle_radius, 0, 50)
            && t.vehicle_radius > Fixed::ZERO;
        if !g_ok
            || !r_ok
            || !(1..=60 * 30).contains(&t.round_interval_ticks)
            || !ammo
                .get(t.cookoff as usize)
                .is_some_and(|a| a.blast.is_some())
        {
            return Some("vehicles.supply: out of range");
        }
    }
    for (i, w) in v.mounts.iter().enumerate() {
        let Some(ws) = weapons.get(w.weapon as usize) else {
            return Some("vehicles.mounts.weapon: index out of range");
        };
        let main = w.kind.is_main();
        if (main && i != 0)
            || (w.kind == MountKind::TurretMain && !v.has_turret)
            || (w.kind == MountKind::Coax && !v.has_turret)
            || (main && ws.class != WeaponClass::Cannon)
            || (!main && ws.class != WeaponClass::Mg)
            || v.seat(w.seat).is_none()
        {
            return Some("vehicles.mounts: main gun not first, wrong class or seat");
        }
        let elev_ok = !main
            || ((-30..=0).contains(&w.elevation_deg[0]) && (1..=60).contains(&w.elevation_deg[1]));
        let ammo_ok = if main {
            (1..=MAX_MAIN_AMMO).contains(&w.ammo.len())
                && w.ammo
                    .iter()
                    .all(|l| (l.ammo as usize) < ammo.len() && l.count <= 500)
                && w.barrel_length > Fixed::ZERO
                && fx_in(w.barrel_length, 0, 10)
                && w.barrel_radius > Fixed::ZERO
                && fx_in(w.barrel_radius, 0, 1)
        } else {
            w.ammo.is_empty() && w.barrel_length == Fixed::ZERO && w.barrel_radius == Fixed::ZERO
        };
        let arc_ok = if w.kind == MountKind::Hull {
            (1..=90).contains(&w.arc_deg)
        } else {
            w.arc_deg == 0
        };
        // S8a: a pintle's pivot, arc and rate; zero on other kinds.
        let pintle_ok = if w.kind == MountKind::Pintle {
            let [lo, hi] = w.traverse_deg;
            (-180..=0).contains(&lo)
                && (0..=180).contains(&hi)
                && lo < hi
                && (1..=360).contains(&w.traverse_deg_s)
                && [w.pivot.x, w.pivot.y, w.pivot.z]
                    .iter()
                    .all(|&c| fx_in(c, -20, 20))
        } else {
            w.pivot == FixedVec3::default() && w.traverse_deg == [0, 0] && w.traverse_deg_s == 0
        };
        let muzzle_ok = [w.muzzle.x, w.muzzle.y, w.muzzle.z]
            .iter()
            .all(|&c| fx_in(c, -20, 20));
        // S3b: standard penetrator, HE, special penetrator (`modset`).
        let mech = |k: usize| {
            w.ammo
                .get(k)
                .and_then(|l| ammo.get(l.ammo as usize))
                .map(|a| a.mechanism)
        };
        let order_ok = w.ammo.len() < 3
            || (mech(1) == Some(AmmoMechanism::He)
                && mech(0).is_some_and(|m| m.penetrator())
                && mech(2).is_some_and(|m| m.penetrator()));
        let rounds_ok = match w.rounds {
            Some(n) => !main && (1..=20_000).contains(&n),
            None => true,
        };
        if !(elev_ok && ammo_ok && arc_ok && muzzle_ok && order_ok && rounds_ok && pintle_ok) {
            return Some("vehicles.mounts: limits, ammo or arc out of range");
        }
    }
    if v.components.len() != COMPONENTS {
        return Some("vehicles.components: wrong count");
    }
    for (k, c) in ComponentKind::ALL.iter().zip(&v.components) {
        // S7: no turret ring without a turret (0 HP, nothing else).
        // S8a-1: no gun or ammo rack without a main gun. S8b: no wheels on
        // tracks and the other way round, cargo only on a truck.
        if component_absent(*k, v.has_turret, v.main_gun().is_some(), m.kind, v.role) {
            if c.kind != *k
                || c.hp != 0
                || !c.parts.is_empty()
                || !c.anchors.is_empty()
                || c.repair.is_some()
            {
                return Some("vehicles.components: a part it cannot have");
            }
            continue;
        }
        let anchors_ok = match k {
            k if k.is_running_gear() => (1..=MAX_ANCHORS).contains(&c.anchors.len()),
            _ => c.anchors.is_empty(),
        };
        let part_ok = if k.has_box() {
            (1..=MAX_COMPONENT_BOXES).contains(&c.parts.len())
                && c.parts
                    .iter()
                    .all(|p| box_ok(&p.bounds) && inside(frame(p.frame), &p.bounds))
        } else {
            c.parts.is_empty()
        };
        let repair_ok = c
            .repair
            .as_ref()
            .is_none_or(|r| (1..=c.hp).contains(&r.to_hp) && (1..=600 * 30).contains(&r.ticks));
        let anchor_pos_ok = c.anchors.iter().all(|a| {
            [a.pos.x, a.pos.y, a.pos.z]
                .iter()
                .all(|&p| fx_in(p, -20, 20))
        });
        if c.kind != *k
            || !(1..=1000).contains(&c.hp)
            || !anchors_ok
            || !part_ok
            || !repair_ok
            || !anchor_pos_ok
        {
            return Some("vehicles.components: kind, hp, box, anchors or repair out of range");
        }
    }
    let slots = v.transport.as_ref().map_or(0, |t| t.slots.len());
    if v.seats.len() + slots + v.components.iter().map(|c| c.parts.len()).sum::<usize>()
        > MAX_INNER_BOXES
    {
        return Some("vehicles: more inner boxes than MAX_INNER_BOXES");
    }
    // S8a: shields and the transport.
    if v.shields.len() > MAX_SHIELDS
        || v.shields.iter().any(|sh| {
            !box_ok(&sh.bounds)
                || sh.thickness < Fixed::ONE
                || sh.thickness > Fixed::from_int(500)
                || sh.mount.is_some_and(|k| {
                    v.mounts
                        .get(k as usize)
                        .is_none_or(|m| m.kind != MountKind::Pintle)
                })
        })
    {
        return Some("vehicles.shields: out of range");
    }
    if (v.role == VehicleRole::Transport) != v.transport.is_some() {
        return Some("vehicles.transport: exactly on a transport");
    }
    if let Some(t) = &v.transport {
        let tick_ok = |n: u32| (1..=30 * 30).contains(&n);
        if t.slots.is_empty()
            || t.slots.len() > MAX_PASSENGERS
            || t.slots
                .iter()
                .any(|b| !box_ok(b) || !inside(frame(Frame::Hull), b))
            || !tick_ok(t.board_ticks)
            || !tick_ok(t.exit_ticks)
            || t.exit_over_sides_ticks.is_some_and(|n| !tick_ok(n))
            || !fx_in(t.exit_raise, 0, 2)
            || [t.entry.x, t.entry.y, t.entry.z]
                .iter()
                .any(|&c| !fx_in(c, -20, 20))
        {
            return Some("vehicles.transport: slots, door or times out of range");
        }
    }
    if v.stations.len() > MAX_STATIONS
        || v.stations
            .iter()
            .any(|s| s.seats.iter().any(|&r| v.seat(r).is_none()))
    {
        return Some("vehicles.stations: unknown seat");
    }
    // S3c: hatches cover every seat exactly once (a crew can bail out only
    // with them), openings within 20 m of the frame origin.
    let mut seen = vec![0u8; v.seats.len()];
    for h in &v.hatches {
        for &k in &h.seats {
            match seen.get_mut(k as usize) {
                Some(n) => *n += 1,
                None => return Some("vehicles.hatches: unknown seat"),
            }
        }
    }
    if v.hatches.len() > MAX_HATCHES
        || v.hatches.iter().any(|h| {
            h.seats.is_empty()
                || !(1..=30 * 30).contains(&h.exit_ticks)
                || !fx_in(h.pos.x, -20, 20)
                || !fx_in(h.pos.y, -20, 20)
                || !fx_in(h.pos.z, -20, 20)
        })
        || (!v.hatches.is_empty() && seen.iter().any(|&n| n != 1))
        || (v.crew_squad.is_some() && v.hatches.is_empty())
    {
        return Some("vehicles.hatches: out of range or a seat not in exactly one hatch");
    }
    None
}
