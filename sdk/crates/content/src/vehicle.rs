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

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::defs::{ItemHeDef, check_id};
use crate::issue::Issue;
use crate::setup::{Fixed, ItemHeSetup};

// ------------------------------------------------------------- enums --

/// Armour zones (§7.4 names). A box face maps to the zone of its box and
/// side; the bottom face of a box uses that box's top zone.
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum ArmourZone {
    HullFront = 0,
    HullSide = 1,
    HullRear = 2,
    HullTop = 3,
    TurretFront = 4,
    TurretSide = 5,
    TurretRear = 6,
    TurretTop = 7,
}

/// Number of [`ArmourZone`]s.
pub const ARMOUR_ZONES: usize = 8;

impl ArmourZone {
    pub const ALL: [ArmourZone; ARMOUR_ZONES] = [
        ArmourZone::HullFront,
        ArmourZone::HullSide,
        ArmourZone::HullRear,
        ArmourZone::HullTop,
        ArmourZone::TurretFront,
        ArmourZone::TurretSide,
        ArmourZone::TurretRear,
        ArmourZone::TurretTop,
    ];

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The def key, e.g. `hull_front`.
    pub const fn name(self) -> &'static str {
        match self {
            ArmourZone::HullFront => "hull_front",
            ArmourZone::HullSide => "hull_side",
            ArmourZone::HullRear => "hull_rear",
            ArmourZone::HullTop => "hull_top",
            ArmourZone::TurretFront => "turret_front",
            ArmourZone::TurretSide => "turret_side",
            ArmourZone::TurretRear => "turret_rear",
            ArmourZone::TurretTop => "turret_top",
        }
    }

    pub fn from_name(s: &str) -> Option<ArmourZone> {
        ArmourZone::ALL.into_iter().find(|z| z.name() == s)
    }

    /// Top plates are flat in S3 (`slope_deg` must be 0).
    pub const fn is_top(self) -> bool {
        matches!(self, ArmourZone::HullTop | ArmourZone::TurretTop)
    }

    pub const fn is_turret(self) -> bool {
        self as u8 >= ArmourZone::TurretFront as u8
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
}

/// Number of [`ComponentKind`]s (every vehicle has each exactly once).
pub const COMPONENTS: usize = 7;

impl ComponentKind {
    pub const ALL: [ComponentKind; COMPONENTS] = [
        ComponentKind::Engine,
        ComponentKind::TrackLeft,
        ComponentKind::TrackRight,
        ComponentKind::Gun,
        ComponentKind::TurretRing,
        ComponentKind::AmmoRack,
        ComponentKind::Transmission,
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
        }
    }

    /// Needs at least one box (internal); tracks are anchors only.
    pub const fn has_box(self) -> bool {
        !self.is_track()
    }

    pub const fn is_track(self) -> bool {
        matches!(self, ComponentKind::TrackLeft | ComponentKind::TrackRight)
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

/// What a vehicle is for (S3: tanks only; tank destroyers are S3b).
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum VehicleRole {
    Tank,
}

#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum MobilityKind {
    Tracked,
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
}

// -------------------------------------------------------------- vehicle --

/// Most crew seats, components boxes and weapons per vehicle are fixed by
/// the enums; these cap the rest.
pub const MAX_MOUNTS: usize = 4;
pub const MAX_ANCHORS: usize = 4;
pub const MAX_STATIONS: usize = 4;
/// Most boxes one component may have (S3b: stowage bins).
pub const MAX_COMPONENT_BOXES: usize = 4;
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
    pub hull: HullDef,
    pub turret: TurretDef,
    pub armor: ArmorDef,
    pub crew: CrewDef,
    pub weapons: Vec<MountDef>,
    pub components: Vec<ComponentDef>,
    /// Direct-control stations (§5.12), unused until M5.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stations: Vec<StationDef>,
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
    /// Hull turn rate.
    pub turn_deg_s: u32,
    pub accel_cm_s2: u32,
    pub brake_cm_s2: u32,
    /// Steepest slope it climbs; speed falls linearly to 30 % there.
    pub max_slope_pct: u32,
    /// Ground inside an AT-ditch footprint at least this deep blocks it.
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
    /// The hull box in the hull frame.
    #[serde(rename = "box")]
    pub bounds: BoxDef,
    /// The lower part of each hull side, in percent of the side's height,
    /// is running gear: AP and HEAT hits there damage that side's track.
    pub track_band_pct: u32,
    /// Extra armour a round meets going through the running gear.
    pub track_mm: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TurretDef {
    /// Turret pivot in the hull frame (on the hull roof).
    pub pivot: [i32; 3],
    /// The turret box in the turret frame (origin at the pivot).
    #[serde(rename = "box")]
    pub bounds: BoxDef,
    pub traverse_deg_s: u32,
    /// Main gun elevation rate (deg/s).
    pub elevate_deg_s: u32,
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

/// Every [`ArmourZone`] once (missing or misspelled zones are errors).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArmorDef {
    pub hull_front: PlateDef,
    pub hull_side: PlateDef,
    pub hull_rear: PlateDef,
    pub hull_top: PlateDef,
    pub turret_front: PlateDef,
    pub turret_side: PlateDef,
    pub turret_rear: PlateDef,
    pub turret_top: PlateDef,
}

impl ArmorDef {
    pub fn get(&self, z: ArmourZone) -> &PlateDef {
        match z {
            ArmourZone::HullFront => &self.hull_front,
            ArmourZone::HullSide => &self.hull_side,
            ArmourZone::HullRear => &self.hull_rear,
            ArmourZone::HullTop => &self.hull_top,
            ArmourZone::TurretFront => &self.turret_front,
            ArmourZone::TurretSide => &self.turret_side,
            ArmourZone::TurretRear => &self.turret_rear,
            ArmourZone::TurretTop => &self.turret_top,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CrewDef {
    /// Seats in this vehicle, each role at most once; driver and gunner are
    /// required.
    pub seats: Vec<SeatRole>,
    /// One box per seat.
    pub boxes: CrewBoxesDef,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CrewBoxesDef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver: Option<PartBoxDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gunner: Option<PartBoxDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commander: Option<PartBoxDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loader: Option<PartBoxDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radio_mg: Option<PartBoxDef>,
}

impl CrewBoxesDef {
    pub fn get(&self, r: SeatRole) -> Option<&PartBoxDef> {
        match r {
            SeatRole::Driver => self.driver.as_ref(),
            SeatRole::Gunner => self.gunner.as_ref(),
            SeatRole::Commander => self.commander.as_ref(),
            SeatRole::Loader => self.loader.as_ref(),
            SeatRole::RadioMg => self.radio_mg.as_ref(),
        }
    }
}

/// A weapon on the vehicle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MountDef {
    /// Weapon id: a `cannon` for `turret_main`, an `mg` for the others.
    pub def: String,
    pub mount: MountKind,
    /// Muzzle point: from the turret pivot in the turret frame
    /// (`turret_main`, `coax`), in the hull frame for `hull`.
    pub muzzle: [i32; 3],
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

/// `part` must lie inside its frame's box (the hull or the turret).
fn check_part(out: &mut Vec<Issue>, path: &str, part: &PartBoxDef, hull: &BoxDef, turret: &BoxDef) {
    check_box(out, path, &part.bounds());
    let (frame, name) = match part.frame {
        Frame::Hull => (hull, "hull"),
        Frame::Turret => (turret, "turret"),
    };
    if !frame.contains(&part.bounds()) {
        let lo = |a| frame.lo(a);
        let hi = |a| frame.hi(a);
        out.push(
            Issue::new(
                path,
                format!(
                    "box {:?} ± {:?} lies outside its frame: the {name} box spans x {}..{}, y {}..{}, z {}..{} cm",
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
                "move it inside the {name} box, or set frame = \"{}\" if it belongs to the {}",
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
        range(&mut out, "mobility.turn_deg_s", m.turn_deg_s as i64, 1, 360);
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

        let hull = &self.hull.bounds;
        check_box(&mut out, "hull.box", hull);
        if hull.lo(2) < 0 {
            out.push(Issue::new(
                "hull.box",
                "the hull box reaches below the ground (center.z < half.z)",
            ));
        }
        range(
            &mut out,
            "hull.track_band_pct",
            self.hull.track_band_pct as i64,
            0,
            100,
        );
        range(&mut out, "hull.track_mm", self.hull.track_mm as i64, 0, 500);
        let turret_box = &self.turret.bounds;
        check_box(&mut out, "turret.box", turret_box);
        // S3b Tune: a turret-frame part (a turret crewman, the breech) may
        // reach below the turret box down to the hull floor: the turret
        // basket, where the gunner's and loader's legs and the loader's
        // standing place are, turning with the turret.
        let basket = basket_def(turret_box, self.turret.pivot[2], hull);
        let turret = &basket;
        for (a, p) in self.turret.pivot.iter().enumerate() {
            range(
                &mut out,
                &format!("turret.pivot[{a}]"),
                *p as i64,
                -2000,
                2000,
            );
        }
        if (0..2).any(|a| {
            let p = self.turret.pivot[a] as i64;
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
            self.turret.traverse_deg_s as i64,
            1,
            360,
        );
        range(
            &mut out,
            "turret.elevate_deg_s",
            self.turret.elevate_deg_s as i64,
            1,
            90,
        );

        for z in ArmourZone::ALL {
            let p = self.armor.get(z);
            let path = format!("armor.{}", z.name());
            if !(10..=5000).contains(&p.thickness_mm.0) {
                out.push(Issue::new(
                    format!("{path}.thickness_mm"),
                    format!("must be in 1..=500 mm, got {}", p.thickness_mm),
                ));
            }
            if z.is_top() {
                if p.slope_deg != 0 {
                    out.push(
                        Issue::new(
                            format!("{path}.slope_deg"),
                            format!("top plates are flat in S3, got {}°", p.slope_deg),
                        )
                        .with_hint("remove slope_deg"),
                    );
                }
            } else {
                range(
                    &mut out,
                    &format!("{path}.slope_deg"),
                    p.slope_deg as i64,
                    0,
                    80,
                );
            }
        }

        let seats = &self.crew.seats;
        if seats.is_empty() || seats.len() > SEAT_ROLES {
            out.push(Issue::new(
                "crew.seats",
                format!("needs 1..={SEAT_ROLES} seats, got {}", seats.len()),
            ));
        }
        for (i, r) in seats.iter().enumerate() {
            if seats[..i].contains(r) {
                out.push(Issue::new(
                    format!("crew.seats[{i}]"),
                    format!("`{}` is listed twice", r.name()),
                ));
            }
        }
        for need in [SeatRole::Driver, SeatRole::Gunner] {
            if !seats.contains(&need) {
                out.push(
                    Issue::new("crew.seats", format!("needs a `{}` seat", need.name()))
                        .with_hint(format!("add \"{}\" to crew.seats", need.name())),
                );
            }
        }
        for r in SeatRole::ALL {
            let path = format!("crew.boxes.{}", r.name());
            match (seats.contains(&r), self.crew.boxes.get(r)) {
                (true, None) => {
                    out.push(Issue::new(path, "the seat has no box").with_hint(format!(
                        "{} = {{ frame = \"hull\", center = [x, y, z], half = [x, y, z] }}",
                        r.name()
                    )))
                }
                (false, Some(_)) => out.push(Issue::new(
                    path,
                    format!("a box for a seat not in crew.seats (`{}`)", r.name()),
                )),
                (true, Some(b)) => check_part(&mut out, &path, b, hull, turret),
                (false, None) => {}
            }
        }

        if self.weapons.len() > MAX_MOUNTS {
            out.push(Issue::new(
                "weapons",
                format!("at most {MAX_MOUNTS} weapons, got {}", self.weapons.len()),
            ));
        }
        for kind in [MountKind::TurretMain, MountKind::Coax, MountKind::Hull] {
            let n = self.weapons.iter().filter(|w| w.mount == kind).count();
            if n > 1 || (kind == MountKind::TurretMain && n == 0) {
                out.push(Issue::new(
                    "weapons",
                    format!(
                        "needs {} `{:?}` mount, got {n}",
                        if kind == MountKind::TurretMain {
                            "exactly one"
                        } else {
                            "at most one"
                        },
                        kind
                    ),
                ));
            }
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
                MountKind::TurretMain => {
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
                }
                MountKind::Coax | MountKind::Hull => {
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
            if n != 1 {
                out.push(
                    Issue::new(
                        "components",
                        format!("needs exactly one `{}` component, got {n}", kind.name()),
                    )
                    .with_hint(format!("[[components]] type = \"{}\" ...", kind.name())),
                );
            }
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
                    "tracks have anchors, not a box",
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
                    check_part(&mut out, &format!("{p}.box"), b, hull, turret);
                }
                for (k, b) in c.boxes.iter().enumerate() {
                    check_part(&mut out, &format!("{p}.boxes[{k}]"), b, hull, turret);
                }
            }
            let anchors = match c.kind {
                k if k.is_track() => 1..=MAX_ANCHORS,
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
        out
    }

    /// The main gun mount (validated defs have exactly one).
    pub fn main_gun(&self) -> Option<&MountDef> {
        self.weapons
            .iter()
            .find(|w| w.mount == MountKind::TurretMain)
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
            range(&mut out, "spall.rays", s.rays as i64, 0, 8);
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

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    pub role: SeatRole,
    pub part: PartBoxSetup,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmmoLoadSetup {
    /// Index into `MatchSetup::ammo`.
    pub ammo: u16,
    pub count: u16,
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
    pub hull: BoxSetup,
    pub track_band_pct: u32,
    pub track_mm: u32,
    /// Turret pivot in the hull frame.
    pub turret_pivot: FixedVec3,
    /// Turret box in the turret frame.
    pub turret: BoxSetup,
    pub traverse_deg_s: i32,
    /// Main gun elevation rate (deg/s).
    pub elevate_deg_s: i32,
    /// By [`ArmourZone::index`].
    pub armour: [PlateSetup; ARMOUR_ZONES],
    /// In the def's seat order.
    pub seats: Vec<SeatSetup>,
    /// Main gun first, then the def's order.
    pub mounts: Vec<MountSetup>,
    /// By [`ComponentKind::index`].
    pub components: Vec<ComponentSetup>,
    pub stations: Vec<StationSetup>,
}

impl VehicleSetup {
    /// The main gun (validated setups have it first).
    pub fn main_gun(&self) -> &MountSetup {
        &self.mounts[0]
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
    let mut mounts: Vec<&MountDef> = v.weapons.iter().collect();
    // Main gun first; the rest keep the def's order.
    mounts.sort_by_key(|w| w.mount != MountKind::TurretMain);
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
                })
                .collect(),
            arc_deg: w.arc_deg.unwrap_or(0) as i32,
            seat: w.seat.unwrap_or(SeatRole::Gunner),
            barrel_length: w
                .barrel
                .map_or(Fixed::ZERO, |b| Fixed::from_cm(b.length_cm)),
            barrel_radius: w
                .barrel
                .map_or(Fixed::ZERO, |b| Fixed::from_cm(b.radius_cm)),
        }
    };
    let component = |k: ComponentKind| {
        let c = v
            .components
            .iter()
            .find(|c| c.kind == k)
            .expect("validated vehicles have every component");
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
            accel: Fixed::from_cm(m.accel_cm_s2),
            brake: Fixed::from_cm(m.brake_cm_s2),
            max_slope_pct: m.max_slope_pct,
            ditch_block: Fixed::from_ratio(m.ditch_block_dm as i64, 10),
            bog_pct: m.bog_pct,
            unbog_ticks: [m.unbog_s[0] * hz, m.unbog_s[1] * hz],
            moving_spread_pct: m.moving_spread_pct,
        },
        hull: (&v.hull.bounds).into(),
        track_band_pct: v.hull.track_band_pct,
        track_mm: v.hull.track_mm,
        turret_pivot: FixedVec3::from_cm(v.turret.pivot),
        turret: (&v.turret.bounds).into(),
        traverse_deg_s: v.turret.traverse_deg_s as i32,
        elevate_deg_s: v.turret.elevate_deg_s as i32,
        armour: ArmourZone::ALL.map(|z| {
            let p = v.armor.get(z);
            PlateSetup {
                thickness: Fixed::from_ratio(p.thickness_mm.0 as i64, 10),
                slope_deg: p.slope_deg as i32,
                material: p.material,
            }
        }),
        seats: v
            .crew
            .seats
            .iter()
            .filter_map(|&r| {
                v.crew.boxes.get(r).map(|b| SeatSetup {
                    role: r,
                    part: b.into(),
                })
            })
            .collect(),
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
        && (s.rays > 8 || s.damage > 1000 || !(1..=60).contains(&s.cone_deg))
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
    if !(1..=120).contains(&v.pop)
        || v.cost > 100_000
        || !fx_in(v.sight, 1, 2000)
        || !(1..=1000).contains(&v.spot_size_pct)
        || !fx_in(v.eye_height, 0, 10)
        || !(m.speed > Fixed::ZERO && fx_in(m.speed, 0, 30))
        || !(m.reverse > Fixed::ZERO && m.reverse <= m.speed)
        || !(1..=360).contains(&m.turn_deg_s)
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
    if !box_ok(&v.hull)
        || !box_ok(&v.turret)
        || v.track_band_pct > 100
        || v.track_mm > 500
        || !(1..=360).contains(&v.traverse_deg_s)
        || !(1..=90).contains(&v.elevate_deg_s)
        || !fx_in(v.turret_pivot.z, 0, 20)
        || v.hull.center.z.0 < v.hull.half.z.0
    {
        return Some("vehicles.boxes: out of range");
    }
    for (z, p) in ArmourZone::ALL.iter().zip(&v.armour) {
        let slope_ok = if z.is_top() {
            p.slope_deg == 0
        } else {
            (0..=80).contains(&p.slope_deg)
        };
        if !(p.thickness >= Fixed::from_int(1) && p.thickness <= Fixed::from_int(500)) || !slope_ok
        {
            return Some("vehicles.armour: plate out of range");
        }
    }
    let basket = basket(v);
    let frame = |f: Frame| match f {
        Frame::Hull => &v.hull,
        Frame::Turret => &basket,
    };
    if v.seats.is_empty()
        || v.seats.len() > SEAT_ROLES
        || v.seats
            .iter()
            .enumerate()
            .any(|(i, s)| v.seats[..i].iter().any(|o| o.role == s.role))
        || v.seat(SeatRole::Driver).is_none()
        || v.seat(SeatRole::Gunner).is_none()
        || v.seats
            .iter()
            .any(|s| !box_ok(&s.part.bounds) || !inside(frame(s.part.frame), &s.part.bounds))
    {
        return Some("vehicles.seats: missing, repeated or outside the frame");
    }
    if v.mounts.is_empty() || v.mounts.len() > MAX_MOUNTS {
        return Some("vehicles.mounts: count out of range");
    }
    for (i, w) in v.mounts.iter().enumerate() {
        let Some(ws) = weapons.get(w.weapon as usize) else {
            return Some("vehicles.mounts.weapon: index out of range");
        };
        let main = w.kind == MountKind::TurretMain;
        if main != (i == 0)
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
        if !(elev_ok && ammo_ok && arc_ok && muzzle_ok && order_ok) {
            return Some("vehicles.mounts: limits, ammo or arc out of range");
        }
    }
    if v.components.len() != COMPONENTS {
        return Some("vehicles.components: wrong count");
    }
    for (k, c) in ComponentKind::ALL.iter().zip(&v.components) {
        let anchors_ok = match k {
            k if k.is_track() => (1..=MAX_ANCHORS).contains(&c.anchors.len()),
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
    if v.stations.len() > MAX_STATIONS
        || v.stations
            .iter()
            .any(|s| s.seats.iter().any(|&r| v.seat(r).is_none()))
    {
        return Some("vehicles.stations: unknown seat");
    }
    None
}
