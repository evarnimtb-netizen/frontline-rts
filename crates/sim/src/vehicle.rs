//! Vehicles (S3, `docs/slices/S3-armour.md` "Vehicle model", "Movement and
//! terrain", "Sim structure"): per-vehicle state, the pose from track
//! samples, `drive`, the RTS path follower, push-out, and the derived
//! vehicle index, box frames and block layer.
//!
//! **Entity and squad.** A vehicle is an entity of kind
//! [`EntityKind::Vehicle`] and a squad of one: its squad's only member,
//! with a [`VehicleState`] in `MatchState::vehicles`. A knocked-out
//! vehicle keeps its id as [`EntityKind::Wreck`] with `wreck_tick` set and
//! no squad. Squad orders reach a vehicle through its squad; its path goes
//! through the shared queue as [`crate::game::PathReq::Vehicle`].
//!
//! **Pose.** Each track samples the combined surface at five points, rear
//! (0) to front (4). Rear support is the higher of samples 0–1, front
//! support the higher of 3–4, each at its own position, so a ditch
//! narrower than about half the track is bridged and a crest never gives
//! a zero lever arm. The track lines give the pitch (their mean slope),
//! the roll (left against right at the hull centre) and the height of the
//! hull origin. Pitch and roll are clamped to ±30° and move at most 20°/s.
//! The rotation is `Rz(yaw) · Ry(−pitch) · Rx(roll)` ([`Basis::hull`]),
//! the snapshot's convention.
//!
//! **Driving** ([`drive`]) takes the §5.12.3 inputs, an `i8` throttle and
//! steer, so direct control (M5) can drive the same code. It accelerates
//! at `accel`, brakes at `brake`, turns at the hull rate (on the spot when
//! slow), and caps speed by slope (linear to 30 % at `max_slope_pct`),
//! woods (50 %), a trench crossing (1 m/s) and crew and engine damage. It
//! refuses a step that would put a track sample on a vehicle-blocked node
//! or onto a slope steeper than `max_slope_pct`; a vehicle whose footprint
//! ends up on blocked ground (a ditch dug under it) is immobile (shown as
//! bogged) until the ground opens again. A new trench crossing (a track
//! sample over ground dug 1 m or more after 2 s with none) rolls `bog_pct`
//! once (RNG `BOG`, keyed by the crossing count).
//!
//! **Path follower** ([`MatchState::move_vehicles`], Movement stage,
//! serial in ascending id after the soldiers): turns the squad's path into
//! throttle and steer, halts on an attack-move while its main gun has a
//! target it can fire at (S3 gunnery), pivots when the heading error is over 60° at low
//! speed, reverses for a `Move` whose target was behind and within 40 m,
//! yields to vehicles and wrecks in a box at least 1.5 hull lengths ahead
//! (between two crossing vehicles the higher id yields; two meeting
//! head-on both brake and keep right), slows to walking pace with a
//! friendly soldier within 4 m ahead, re-checks the segments ahead against
//! the block layer every 0.5 s, and re-paths with an avoided node after 5
//! s with under 1 m of progress (at most one re-path per 2 s). After each
//! vehicle's step, soldiers inside its footprint are pushed out sideways,
//! never into deeper dug ground, and never a man whose capsule top is below
//! the hull's belly (in a trench under a crossing tank).
//!
//! **Derived** ([`VehicleDerived`], not saved): the vehicle index (a second
//! [`SpatialHash`] of vehicles and wrecks at their hull origins; queries
//! pad by [`VehicleDerived::max_reach`]) and the world-space box frames are
//! rebuilt at the start of Movement (soldiers plan against them) and after
//! the vehicles drive, and on load. The block layer is a bit per grid node:
//! blocked within 2 m of an AT-ditch line where the node's `dug` depth is
//! at least the vehicles' `ditch_block`, or under a wreck's footprint
//! (padded 2 m), dilated by one node; one more dilation is a clearance band
//! that paths pay ×4 to enter and smoothing never cuts into, because a
//! hull pivoting there could put a sample on a blocked node. It is updated
//! at the end of each tick
//! from the terrain's dirty tiles and from rectangles marked when a ditch
//! line or a wreck is added or removed, and rebuilt on load; readers during
//! a tick see the layer as of the end of the previous tick.

use frontline_content::setup::VehicleRulesSetup;
use frontline_content::vehicle::{FixedVec3, VehicleSetup};
use frontline_content::{COMPONENTS, ComponentKind, MatchSetup, SeatRole};
use serde::{Deserialize, Serialize};

use crate::armour::deg;
use crate::damage::ArmourEv;
use crate::ecs::{Entities, EntityKind, PlayerId, SimId, Transform, Velocity};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{MatchState, Nav, NavPhase, Order, PathQueue, PathReq, Squad, SquadId};
use crate::hash::HashCache;
use crate::rng::{Purpose, RngStream};
use crate::snapshot::{ArmourEventKind, NONE_U8};
use crate::spatial::SpatialHash;
use crate::terrain::{Heightfield, SPACING_M, TRENCH_NAV_DM};

/// Height samples per track, rear (0) to front (4).
pub const TRACK_SAMPLES: usize = 5;
/// Hull pitch and roll limit (degrees).
pub const MAX_TILT_DEG: i32 = 30;
/// Fastest change of pitch and roll (degrees per second).
const TILT_RATE_DEG_S: i32 = 20;
/// A vehicle circle's radius is half the hull width plus this (mm).
const CIRCLE_PAD_MM: i32 = 250;
/// Vehicle circles a soldier avoids at most, on top of his neighbours.
pub const MAX_VEHICLE_CIRCLES: usize = 4;
/// The follower moves on to the next waypoint within this distance (m).
const WAYPOINT_REACH_M: i32 = 3;
/// The last waypoint counts as reached within this distance (mm).
const ARRIVE_MM: i32 = 2500;
/// Over this heading error the follower stops pushing and turns (pivots
/// on the spot below [`PIVOT_SPEED_MM_S`]).
const PIVOT_ERR_DEG: i32 = 60;
const PIVOT_SPEED_MM_S: i32 = 1000;
/// Full steer at this heading error and beyond.
const FULL_STEER_DEG: i32 = 10;
/// Full throttle up to this heading error, falling to none at
/// [`PIVOT_ERR_DEG`].
const FULL_THROTTLE_DEG: i32 = 20;
/// A `Move` to a target behind the hull and this close reverses (m).
const REVERSE_ORDER_M: i32 = 40;
/// Yield box length in percent of the hull length.
const YIELD_LEN_PCT: i32 = 150;
/// A vehicle slower than this counts as standing (yield rule, mm/s).
const YIELD_STILL_MM_S: i32 = 500;
/// A friendly soldier this far ahead slows a vehicle to walking pace (m).
const FRIENDLY_AHEAD_M: i32 = 4;
/// Walking pace (mm/s).
const WALK_MM_S: i32 = 1500;
/// Stuck check: less than this progress (m) in [`STUCK_S`] re-paths.
const STUCK_PROGRESS_M: i32 = 1;
const STUCK_S: u32 = 5;
/// Heading offset to the right while a vehicle comes head-on (degrees).
const KEEP_RIGHT_DEG: i32 = 35;
/// A vehicle making no progress backs off for 1 s after this long.
const BACK_OFF_S: u32 = 2;
/// At most one re-path per vehicle per this many seconds.
const REPATH_GAP_S: u32 = 2;
/// Cost of the avoided node in a stuck re-path, percent.
pub const AVOID_COST_PCT: u32 = 800;
/// Cost of a node next to a blocked one (the clearance band), percent.
pub const MARGIN_COST_PCT: u32 = 400;
/// AT-ditch lines block nodes within this distance (m).
const DITCH_REACH_M: i32 = 2;
/// A wreck's footprint is padded by this much for the block layer (m).
const WRECK_PAD_M: i32 = 2;
/// A soldier pushed out of a footprint lands this far past its side (mm),
/// beyond his own radius.
const PUSH_GAP_MM: i32 = 100;
/// Push-out never moves a man to ground dug this much deeper (mm).
const PUSH_DEEPER_MM: i32 = 100;
/// Spotting check points sit this far below the box tops (mm).
const SPOT_DROP_MM: i32 = 100;

/// Main-gun ammo choice (`SetAmmo`; S3 stage 4 acts on it).
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
#[repr(u8)]
pub enum AmmoChoice {
    #[default]
    Auto = 0,
    Ap = 1,
    He = 2,
}

/// One crew seat, in the def's seat order.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Seat {
    /// 0: nobody in it (the crewman died or moved to another seat).
    pub hp: u16,
    /// Ticks left of a crewman moving in (0: none). The seat and the one he
    /// left both count as empty meanwhile.
    pub swap: u16,
    /// The seat index he comes from while `swap > 0`.
    pub swap_from: u8,
}

/// One vehicle MG (coax or hull), in mount order after the main gun.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct MgState {
    /// The soldier it engages.
    pub target: Option<SimId>,
    /// Ticks until the next round.
    pub cooldown: u16,
    /// Rounds fired (the fire-interval spread, as a soldier's `shots`).
    pub shots: u32,
    /// Rounds fired from the current belt.
    pub belt: u16,
}

/// Per-vehicle state (`MatchState::vehicles`), saved and hashed through
/// postcard bytes. Fields the later S3 stages act on (gunnery, damage,
/// repair, escort) are here from the start so the save shape is stable.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct VehicleState {
    /// Index into `MatchSetup::vehicles`.
    pub type_idx: u16,
    /// Hull pitch (positive: nose up) and roll (positive: left side up),
    /// radians, within ±[`MAX_TILT_DEG`].
    pub pitch: Fx,
    pub roll: Fx,
    /// Hull-relative turret yaw and gun pitch (radians).
    pub turret_yaw: Fx,
    pub gun_pitch: Fx,
    /// Signed speed along the hull's forward axis (m/s).
    pub speed: Fx,
    /// Planar velocity averaged over about 1 s (gunnery lead).
    pub avg_vel: FxVec2,
    /// Following its path backwards (a short `Move` to a point behind).
    pub reverse: bool,
    /// In the def's seat order.
    pub seats: Vec<Seat>,
    /// HP by `ComponentKind::index`.
    pub components: [u16; COMPONENTS],
    /// Main-gun rounds left, per entry of the main gun's ammo list
    /// (penetrator first, then HE).
    pub ammo: Vec<u16>,
    /// The ammo-list entry in the breech, if loaded.
    pub loaded: Option<u8>,
    pub ammo_choice: AmmoChoice,
    /// Main-gun reload ticks left.
    pub reload: u16,
    /// The main gun's target: an enemy vehicle (`AttackUnit` or auto, AP)
    /// or an enemy soldier (HE at the best infantry group).
    pub target: Option<SimId>,
    /// `AttackUnit`'s vehicle: the main gun focuses it while it is seen.
    pub attack: Option<SimId>,
    /// Against a vehicle target: aim at its turret, not its hull (chosen at
    /// each re-target, `crate::gunnery::aim_choice`).
    pub aim_turret: bool,
    /// Coax and hull MGs, in mount order after the main gun.
    pub mgs: Vec<MgState>,
    /// Shots already fired at `target` (ranging in).
    pub ranging: u8,
    /// Ticks since the main gun fired, saturating (`u16::MAX`: never).
    pub since_fire: u16,
    /// Ticks the engine fire has burned (0: not burning).
    pub burn: u16,
    /// Component index being field-repaired.
    pub repairing: Option<u8>,
    /// Field-repair progress of `repairing`, in HP × ticks (the def's rate
    /// spread over ticks).
    pub repair_progress: u32,
    /// Ticks in a row inside the own base (base repair, rearm and crew).
    pub base_ticks: u32,
    /// The player whose round last hit it (credited for crew that die in a
    /// fire or with the vehicle).
    pub last_attacker: Option<u8>,
    /// Ticks left bogged in a trench crossing.
    pub bog: u32,
    /// A track sample is on a vehicle-blocked node (immobile, shown as
    /// bogged, until the ground opens).
    pub on_blocked: bool,
    /// A track sample is over ground dug at least `crossing_dug`.
    pub crossing: bool,
    /// Trench crossings started (the bog roll's key).
    pub crossings: u32,
    /// Ticks in a row with no track sample over dug ground, saturating.
    pub clear_ticks: u16,
    /// Stuck check: where the last metre of progress was measured, and the
    /// ticks since.
    pub progress_at: FxVec2,
    pub progress_ticks: u16,
    /// Tick of the last follower re-path (stuck or newly blocked).
    pub repath_tick: Option<u64>,
    /// Soldiers escorting it (`FollowUnits`), ascending; listed afresh
    /// every Intent stage (`crate::orders`).
    pub followers: Vec<SimId>,
    /// Set when knocked out (the entity is then a `Wreck`).
    pub wreck_tick: Option<u64>,
    /// `FaceUnits`: the point the hull front turns to while holding.
    pub face: Option<FxVec2>,
}

impl VehicleState {
    /// A fresh, full vehicle of type `type_idx` (crew at full HP, every
    /// component whole, full ammo).
    pub fn new(type_idx: u16, v: &VehicleSetup, rules: &VehicleRulesSetup) -> VehicleState {
        let crew = rules.crew_hp.min(u16::MAX as u32) as u16;
        let mut components = [0u16; COMPONENTS];
        for c in &v.components {
            components[c.kind.index()] = c.hp.min(u16::MAX as u32) as u16;
        }
        VehicleState {
            type_idx,
            pitch: Fx::ZERO,
            roll: Fx::ZERO,
            turret_yaw: Fx::ZERO,
            gun_pitch: Fx::ZERO,
            speed: Fx::ZERO,
            avg_vel: FxVec2::ZERO,
            reverse: false,
            seats: v
                .seats
                .iter()
                .map(|_| Seat {
                    hp: crew,
                    ..Seat::default()
                })
                .collect(),
            components,
            ammo: v.main_gun().ammo.iter().map(|a| a.count).collect(),
            loaded: None,
            ammo_choice: AmmoChoice::Auto,
            reload: 0,
            target: None,
            attack: None,
            aim_turret: false,
            mgs: vec![MgState::default(); v.mounts.len().saturating_sub(1)],
            ranging: 0,
            since_fire: u16::MAX,
            burn: 0,
            repairing: None,
            repair_progress: 0,
            base_ticks: 0,
            last_attacker: None,
            bog: 0,
            on_blocked: false,
            crossing: false,
            crossings: 0,
            clear_ticks: u16::MAX,
            progress_at: FxVec2::ZERO,
            progress_ticks: 0,
            repath_tick: None,
            followers: Vec::new(),
            wreck_tick: None,
            face: None,
        }
    }

    /// A new move, stop or retreat order ends an `AttackUnit` focus, a
    /// `FaceUnits` turn and a field repair (`crate::orders`).
    pub fn clear_orders(&mut self) {
        self.attack = None;
        self.face = None;
        self.repairing = None;
        self.repair_progress = 0;
    }

    /// The seat of `role`, if the vehicle has one.
    pub fn seat(&self, v: &VehicleSetup, role: SeatRole) -> Option<&Seat> {
        v.seats
            .iter()
            .position(|s| s.role == role)
            .and_then(|k| self.seats.get(k))
    }

    /// Whether `role`'s seat is manned and not mid-swap.
    pub fn manned(&self, v: &VehicleSetup, role: SeatRole) -> bool {
        self.seat(v, role).is_some_and(|s| s.hp > 0 && s.swap == 0)
    }

    #[inline]
    pub fn component(&self, k: ComponentKind) -> u16 {
        self.components[k.index()]
    }

    /// Cannot move at all: no driver, engine or a track destroyed, bogged,
    /// on blocked ground, under repair, or a wreck.
    pub fn immobile(&self, v: &VehicleSetup) -> bool {
        self.wreck_tick.is_some()
            || self.bog > 0
            || self.on_blocked
            || self.repairing.is_some()
            || !self.manned(v, SeatRole::Driver)
            || self.component(ComponentKind::Engine) == 0
            || self.component(ComponentKind::TrackLeft) == 0
            || self.component(ComponentKind::TrackRight) == 0
    }

    /// Mechanically unable to move (`vehicle_flags::IMMOBILISED`): no
    /// driver, engine or a track destroyed.
    pub fn immobilised(&self, v: &VehicleSetup) -> bool {
        !self.manned(v, SeatRole::Driver)
            || self.component(ComponentKind::Engine) == 0
            || self.component(ComponentKind::TrackLeft) == 0
            || self.component(ComponentKind::TrackRight) == 0
    }

    /// Checks a loaded state against its def (untrusted saves): counts
    /// match, values in range. References are checked by the caller.
    pub(crate) fn validate(&self, s: &MatchSetup, map: Fx, tick: u64) -> Result<(), &'static str> {
        let Some(v) = s.vehicles.get(self.type_idx as usize) else {
            return Err("vehicle type out of range");
        };
        let r = &s.rules.vehicles;
        let tilt = deg(MAX_TILT_DEG) + Fx::from_milli(1);
        let top = Fx::from(v.mobility.speed).max(Fx::from(v.mobility.reverse));
        let swap_max = r.seat_swap_ticks.min(u16::MAX as u32) as u16;
        let bad = self.seats.len() != v.seats.len()
            || self.seats.iter().any(|st| {
                st.hp as u32 > r.crew_hp
                    || st.swap > swap_max
                    || (st.swap > 0 && st.swap_from as usize >= v.seats.len())
            })
            || v.components
                .iter()
                .any(|c| self.components[c.kind.index()] as u32 > c.hp)
            || self.ammo.len() != v.main_gun().ammo.len()
            || self
                .ammo
                .iter()
                .zip(&v.main_gun().ammo)
                .any(|(n, a)| *n > a.count)
            || self.loaded.is_some_and(|l| l as usize >= self.ammo.len())
            || self
                .ammo
                .iter()
                .zip(&v.main_gun().ammo)
                .enumerate()
                .any(|(k, (n, a))| {
                    *n as u32 + (self.loaded == Some(k as u8)) as u32 > a.count as u32
                })
            || self.reload as u32 > s.weapons[v.main_gun().weapon as usize].reload_ticks * 2
            || self.mgs.len() != v.mounts.len().saturating_sub(1)
            || self.mgs.iter().zip(v.mounts.iter().skip(1)).any(|(m, mt)| {
                let w = &s.weapons[mt.weapon as usize];
                m.belt as u32 >= w.magazine.max(1)
                    || m.cooldown as u32
                        > w.reload_ticks.max(w.interval_milli.div_ceil(1000)).max(1)
            })
            || self.burn as u32 > r.fire_burnout_ticks
            || self.repairing.is_some_and(|k| {
                v.components
                    .iter()
                    .find(|c| c.kind.index() == k as usize)
                    .and_then(|c| c.repair.as_ref())
                    .is_none_or(|rep| self.repair_progress >= rep.ticks.max(1))
            })
            || (self.repairing.is_none() && self.repair_progress != 0)
            || self
                .last_attacker
                .is_some_and(|p| p as usize >= s.players.len())
            || self.pitch.abs() > tilt
            || self.roll.abs() > tilt
            || self.turret_yaw.abs() > Fx::PI
            || self.gun_pitch.abs() > Fx::FRAC_PI_2
            || self.speed.abs() > top.mul_int(2) + Fx::ONE
            || self.avg_vel.x.abs() > Fx::from_int(100)
            || self.avg_vel.y.abs() > Fx::from_int(100)
            || self.bog > v.mobility.unbog_ticks[1]
            || self.repairing.is_some_and(|k| k as usize >= COMPONENTS)
            || !(Fx::ZERO..=map).contains(&self.progress_at.x)
            || !(Fx::ZERO..=map).contains(&self.progress_at.y)
            || self.repath_tick.is_some_and(|t| t > tick)
            || self.wreck_tick.is_some_and(|t| t > tick)
            || self.followers.len() > crate::unit::MAX_UNITS
            || !self.followers.windows(2).all(|w| w[0] < w[1])
            || self.face.is_some_and(|f| {
                !(Fx::ZERO..=map).contains(&f.x) || !(Fx::ZERO..=map).contains(&f.y)
            })
            || (self.wreck_tick.is_some() && self.face.is_some());
        if bad {
            return Err("vehicle state is out of range");
        }
        Ok(())
    }
}

// ------------------------------------------------------------- geometry --

pub(crate) fn v3(f: FixedVec3) -> FxVec3 {
    FxVec3::new(f.x.into(), f.y.into(), f.z.into())
}

/// Unit vector of heading `yaw`.
#[inline]
pub fn yaw_dir(yaw: Fx) -> FxVec2 {
    let (s, c) = yaw.sin_cos();
    FxVec2::new(c, s)
}

/// Geometry of a vehicle type in sim units, derived from its setup.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Geo {
    /// Hull box in the hull frame (x forward, y left, z up from the
    /// ground under the hull's centre).
    pub hull_center: FxVec3,
    pub hull_half: FxVec3,
    /// Turret pivot in the hull frame; turret box in the turret frame.
    pub pivot: FxVec3,
    pub turret_center: FxVec3,
    pub turret_half: FxVec3,
    /// Track centrelines at ±`track_y`.
    pub track_y: Fx,
    /// Avoidance circle radius (half the hull width plus 0.25 m).
    pub circle_r: Fx,
    /// Horizontal radius around the hull origin covering hull, turret and
    /// gun in any pose.
    pub reach: Fx,
    /// Commander's eye above the hull origin.
    pub eye: Fx,
}

impl Geo {
    pub fn of(v: &VehicleSetup) -> Geo {
        let hull_center = v3(v.hull.center);
        let hull_half = v3(v.hull.half);
        let pivot = v3(v.turret_pivot);
        let turret_center = v3(v.turret.center);
        let turret_half = v3(v.turret.half);
        let track_y = v
            .components
            .iter()
            .find(|c| c.kind == ComponentKind::TrackLeft)
            .and_then(|c| c.anchors.first())
            .map(|a| Fx::from(a.pos.y).abs())
            .filter(|y| *y > Fx::ZERO && *y <= hull_half.y)
            .unwrap_or_else(|| hull_half.y.mul_int(85).div_int(100));
        let hull_r = FxVec2::new(hull_center.x.abs() + hull_half.x, hull_half.y).length();
        let turret_r = pivot.xy().length()
            + FxVec2::new(turret_center.x.abs() + turret_half.x, turret_half.y).length();
        let gun_r = pivot.xy().length() + v3(v.main_gun().muzzle).xy().length();
        Geo {
            hull_center,
            hull_half,
            pivot,
            turret_center,
            turret_half,
            track_y,
            circle_r: hull_half.y + Fx::from_milli(CIRCLE_PAD_MM),
            reach: hull_r.max(turret_r).max(gun_r),
            eye: v.eye_height.into(),
        }
    }

    /// Track sample `k`'s x in the hull frame: rear to front of the hull.
    #[inline]
    fn sample_x(&self, k: usize) -> Fx {
        self.hull_center.x + self.hull_half.x.mul_int(k as i32 - 2).div_int(2)
    }
}

/// An orthonormal basis: forward (x), left (y), up (z).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Basis {
    pub x: FxVec3,
    pub y: FxVec3,
    pub z: FxVec3,
}

impl Basis {
    /// The hull's rotation `Rz(yaw) · Ry(−pitch) · Rx(roll)`.
    pub fn hull(yaw: Fx, pitch: Fx, roll: Fx) -> Basis {
        let (sy, cy) = yaw.sin_cos();
        let (sp, cp) = pitch.sin_cos();
        let (sr, cr) = roll.sin_cos();
        Basis {
            x: FxVec3::new(cy * cp, sy * cp, sp),
            y: FxVec3::new(
                -(cy * sp * sr) - sy * cr,
                -(sy * sp * sr) + cy * cr,
                cp * sr,
            ),
            z: FxVec3::new(
                -(cy * sp * cr) + sy * sr,
                -(sy * sp * cr) - cy * sr,
                cp * cr,
            ),
        }
    }

    /// This basis turned by `a` about its own up axis (counter-clockwise
    /// seen from above): the turret's.
    pub fn turned(&self, a: Fx) -> Basis {
        let (s, c) = a.sin_cos();
        Basis {
            x: self.x * c + self.y * s,
            y: self.y * c - self.x * s,
            z: self.z,
        }
    }

    /// A local vector in world axes.
    #[inline]
    pub fn apply(&self, v: FxVec3) -> FxVec3 {
        self.x * v.x + self.y * v.y + self.z * v.z
    }
}

/// An oriented box in world space.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct BoxFrame {
    pub center: FxVec3,
    pub axes: Basis,
    pub half: FxVec3,
}

/// World-space frames of one vehicle or wreck (derived, once per tick).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct VehicleFrame {
    pub id: SimId,
    pub owner: PlayerId,
    pub wreck: bool,
    /// Index into `MatchSetup::vehicles`.
    pub type_idx: u16,
    /// Hull origin (on the ground under the hull's centre) and heading.
    pub origin: FxVec3,
    pub yaw: Fx,
    pub hull: BoxFrame,
    /// Turret pivot in world space.
    pub pivot: FxVec3,
    pub turret: BoxFrame,
    /// [`Geo::reach`].
    pub reach: Fx,
}

impl VehicleFrame {
    /// The two avoidance circle centres along the hull axis and their
    /// radius.
    pub fn circles(&self, geo: &Geo) -> ([FxVec2; 2], Fx) {
        let c = self.hull.center.xy();
        let a = self.hull.axes.x.xy() * self.hull.half.x.div_int(2);
        ([c + a, c - a], geo.circle_r)
    }

    /// Spotting check points: the turret top, then the hull top centre.
    pub fn spot_points(&self) -> [FxVec3; 2] {
        let drop = Fx::from_milli(SPOT_DROP_MM);
        let t = &self.turret;
        let h = &self.hull;
        [
            t.center + t.axes.z * (t.half.z - drop),
            h.center + h.axes.z * (h.half.z - drop),
        ]
    }
}

/// The frames of a vehicle in pose `t`, `vs`.
pub fn frame_of(
    id: SimId,
    owner: PlayerId,
    t: &Transform,
    vs: &VehicleState,
    geo: &Geo,
) -> VehicleFrame {
    let hull_axes = Basis::hull(t.yaw, vs.pitch, vs.roll);
    let origin = t.pos;
    let pivot = origin + hull_axes.apply(geo.pivot);
    let turret_axes = hull_axes.turned(vs.turret_yaw);
    VehicleFrame {
        id,
        owner,
        wreck: vs.wreck_tick.is_some(),
        type_idx: vs.type_idx,
        origin,
        yaw: t.yaw,
        hull: BoxFrame {
            center: origin + hull_axes.apply(geo.hull_center),
            axes: hull_axes,
            half: geo.hull_half,
        },
        pivot,
        turret: BoxFrame {
            center: pivot + turret_axes.apply(geo.turret_center),
            axes: turret_axes,
            half: geo.turret_half,
        },
        reach: geo.reach,
    }
}

/// Ground under the tracks: per track (0 left, 1 right) and sample (rear
/// to front), the surface height, the depth below the base surface, and
/// the position.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Ground {
    pub h: [[Fx; TRACK_SAMPLES]; 2],
    pub depth: [[Fx; TRACK_SAMPLES]; 2],
    pub xy: [[FxVec2; TRACK_SAMPLES]; 2],
}

/// Samples the ground under both tracks of a hull at `xy` heading `yaw`
/// (10 height samples; the depth comes from the detail tier).
pub fn sample_ground(terrain: &Heightfield, xy: FxVec2, yaw: Fx, geo: &Geo) -> Ground {
    let f = yaw_dir(yaw);
    let l = f.perp();
    let mut g = Ground::default();
    for t in 0..2 {
        let y = if t == 0 { geo.track_y } else { -geo.track_y };
        for k in 0..TRACK_SAMPLES {
            let p = terrain.clamp(xy + f * geo.sample_x(k) + l * y);
            let h = terrain.height_at(p);
            g.xy[t][k] = p;
            g.h[t][k] = h;
            g.depth[t][k] = (terrain.base_height_at(p) - h).max(Fx::ZERO);
        }
    }
    g
}

/// The hull pose the ground gives: origin height, pitch and roll (clamped
/// to ±[`MAX_TILT_DEG`]).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct GroundPose {
    pub z: Fx,
    pub pitch: Fx,
    pub roll: Fx,
}

pub fn ground_pose(g: &Ground, geo: &Geo) -> GroundPose {
    let mut slope = [Fx::ZERO; 2];
    let mut mid = [Fx::ZERO; 2];
    for t in 0..2 {
        let h = &g.h[t];
        // Supports: the higher sample of each pair; ties go to the outer.
        let (xr, hr) = if h[0] >= h[1] {
            (geo.sample_x(0), h[0])
        } else {
            (geo.sample_x(1), h[1])
        };
        let (xf, hf) = if h[4] >= h[3] {
            (geo.sample_x(4), h[4])
        } else {
            (geo.sample_x(3), h[3])
        };
        // xf − xr is at least half the hull length.
        slope[t] = (hf - hr) / (xf - xr);
        mid[t] = hr - slope[t] * xr;
    }
    let tilt = deg(MAX_TILT_DEG);
    let s = (slope[0] + slope[1]).div_int(2);
    GroundPose {
        z: (mid[0] + mid[1]).div_int(2),
        pitch: Fx::atan2(s, Fx::ONE).clamp(-tilt, tilt),
        roll: Fx::atan2(mid[0] - mid[1], geo.track_y.mul_int(2)).clamp(-tilt, tilt),
    }
}

/// Grade of a pitch in percent (|tan|·100).
fn grade_pct(pitch: Fx) -> u32 {
    let (s, c) = pitch.sin_cos();
    if c <= Fx::ZERO {
        return u32::MAX;
    }
    (((s.abs() / c).raw() as i64 * 100) >> Fx::FRAC_BITS).clamp(0, u32::MAX as i64) as u32
}

/// Moves `cur` toward `target` by at most `step`.
fn approach(cur: Fx, target: Fx, step: Fx) -> Fx {
    if target > cur {
        (cur + step).min(target)
    } else {
        (cur - step).max(target)
    }
}

/// Grid node index of the node nearest `p`.
#[inline]
pub fn node_at(terrain: &Heightfield, p: FxVec2) -> u32 {
    let (gx, gy) = terrain.nearest_grid(p);
    gy * terrain.dim() + gx
}

// ---------------------------------------------------------- block layer --

/// Where vehicles cannot go (see the module docs). A bit per grid node,
/// dilated by one node. Derived; never saved.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BlockLayer {
    dim: u32,
    /// Ditch depth that blocks, in decimetres: the smallest `ditch_block`
    /// of the match's vehicle types (`u16::MAX`: none).
    ditch_dm: u16,
    raw: Vec<bool>,
    /// Nodes of an AT ditch dug trench-deep (within 2 m of a line, `dug`
    /// at least `TRENCH_NAV_DM`): infantry path costs and cover (S3).
    ditch: Vec<bool>,
    blocked: Vec<bool>,
    /// Blocked dilated once more: the clearance band a hull turning on a
    /// node there could clip (paths avoid it, smoothing never enters it).
    margin: Vec<bool>,
    /// Node rectangles `[x0, y0, x1, y1]` (inclusive) to recompute.
    pending: Vec<[u32; 4]>,
}

/// A wreck's footprint for the block layer.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Footprint {
    pub center: FxVec2,
    pub fwd: FxVec2,
    pub half_x: Fx,
    pub half_y: Fx,
}

impl BlockLayer {
    pub fn new(dim: u32, ditch_dm: u16) -> BlockLayer {
        let n = (dim * dim) as usize;
        BlockLayer {
            dim,
            ditch_dm,
            raw: vec![false; n],
            ditch: vec![false; n],
            blocked: vec![false; n],
            margin: vec![false; n],
            pending: Vec::new(),
        }
    }

    /// The setup's ditch depth that blocks, in decimetres.
    pub fn ditch_dm_of(setup: &MatchSetup) -> u16 {
        setup
            .vehicles
            .iter()
            .map(|v| {
                let raw = Fx::from(v.mobility.ditch_block).raw() as i64;
                ((raw * 10 + (1 << (Fx::FRAC_BITS - 1))) >> Fx::FRAC_BITS).clamp(0, 255) as u16
            })
            .min()
            .unwrap_or(u16::MAX)
    }

    #[inline]
    pub fn is_blocked(&self, node: u32) -> bool {
        self.blocked.get(node as usize).copied().unwrap_or(false)
    }

    /// Whether `node` lies in an AT ditch (infantry: dear, no trench
    /// discount, no cover spot).
    #[inline]
    pub fn is_ditch(&self, node: u32) -> bool {
        self.ditch.get(node as usize).copied().unwrap_or(false)
    }

    /// Whether `node` is blocked or next to a blocked node.
    #[inline]
    pub fn is_margin(&self, node: u32) -> bool {
        self.margin.get(node as usize).copied().unwrap_or(false)
    }

    /// Whether the node nearest `p` is blocked.
    #[inline]
    pub fn blocked_at(&self, terrain: &Heightfield, p: FxVec2) -> bool {
        self.is_blocked(node_at(terrain, p))
    }

    /// Number of blocked nodes (tests and reports).
    pub fn blocked_count(&self) -> usize {
        self.blocked.iter().filter(|b| **b).count()
    }

    /// Whether the straight segment `a → b`, sampled every 2 m, touches a
    /// blocked node.
    pub fn segment_blocked(&self, terrain: &Heightfield, a: FxVec2, b: FxVec2) -> bool {
        segment_any(terrain, a, b, |n| self.is_blocked(n))
    }

    /// Whether the segment touches the clearance band (path smoothing).
    pub fn segment_in_margin(&self, terrain: &Heightfield, a: FxVec2, b: FxVec2) -> bool {
        segment_any(terrain, a, b, |n| self.is_margin(n))
    }

    /// Marks the nodes around the world rectangle `lo..hi` for recompute.
    pub(crate) fn mark_rect(&mut self, lo: FxVec2, hi: FxVec2) {
        if self.dim == 0 {
            return;
        }
        let max = self.dim as i32 - 1;
        let node = |v: Fx, up: bool| -> u32 {
            let n = v.raw() as i64 / ((SPACING_M as i64) << Fx::FRAC_BITS);
            let n = if up { n + 2 } else { n - 1 };
            n.clamp(0, max as i64) as u32
        };
        self.pending.push([
            node(lo.x, false),
            node(lo.y, false),
            node(hi.x, true),
            node(hi.y, true),
        ]);
    }

    /// Marks the nodes of changed terrain detail tiles (16 m) for recompute.
    pub(crate) fn mark_tiles(&mut self, tiles: &[(u16, u16)]) {
        if self.dim == 0 {
            return;
        }
        let per = (crate::terrain::TILE_M / SPACING_M) as u32;
        let max = self.dim - 1;
        for &(x, y) in tiles {
            let (x, y) = (x as u32 * per, y as u32 * per);
            self.pending.push([
                x.saturating_sub(1),
                y.saturating_sub(1),
                (x + per + 1).min(max),
                (y + per + 1).min(max),
            ]);
        }
    }

    pub(crate) fn mark_all(&mut self) {
        if self.dim > 0 {
            self.pending.push([0, 0, self.dim - 1, self.dim - 1]);
        }
    }

    /// Recomputes the marked rectangles from the terrain, the AT-ditch
    /// lines and the wreck footprints.
    pub(crate) fn update(
        &mut self,
        terrain: &Heightfield,
        ditches: &[Vec<FxVec2>],
        wrecks: &[Footprint],
    ) {
        if self.pending.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.pending);
        let dim = self.dim;
        let reach_sq = FxVec2::from_int(DITCH_REACH_M, 0).length_sq_raw();
        let pad = Fx::from_int(WRECK_PAD_M);
        for r in &pending {
            for gy in r[1]..=r[3] {
                for gx in r[0]..=r[2] {
                    let p = FxVec2::from_int(gx as i32 * SPACING_M, gy as i32 * SPACING_M);
                    let dug = terrain.dug_dm(gx, gy);
                    let near = (dug >= TRENCH_NAV_DM || dug as u16 >= self.ditch_dm)
                        && ditches.iter().any(|l| near_line(l, p, reach_sq));
                    self.ditch[(gy * dim + gx) as usize] = near && dug >= TRENCH_NAV_DM;
                    let ditch = near && dug as u16 >= self.ditch_dm;
                    let wreck = !ditch
                        && wrecks.iter().any(|w| {
                            let d = p - w.center;
                            d.dot(w.fwd).abs() <= w.half_x + pad
                                && d.det(w.fwd).abs() <= w.half_y + pad
                        });
                    self.raw[(gy * dim + gx) as usize] = ditch || wreck;
                }
            }
        }
        let max = dim - 1;
        // Two dilations: raw → blocked (rect + 1) → margin (rect + 2).
        let dilate = |from: &[bool], gx: u32, gy: u32| {
            (gy.saturating_sub(1)..=(gy + 1).min(max)).any(|ny| {
                (gx.saturating_sub(1)..=(gx + 1).min(max)).any(|nx| from[(ny * dim + nx) as usize])
            })
        };
        for (grow, pass) in [(1u32, 0), (2, 1)] {
            for r in &pending {
                for gy in r[1].saturating_sub(grow)..=(r[3] + grow).min(max) {
                    for gx in r[0].saturating_sub(grow)..=(r[2] + grow).min(max) {
                        let k = (gy * dim + gx) as usize;
                        if pass == 0 {
                            self.blocked[k] = dilate(&self.raw, gx, gy);
                        } else {
                            self.margin[k] = dilate(&self.blocked, gx, gy);
                        }
                    }
                }
            }
        }
    }
}

/// Whether any 2 m sample of the segment `a → b` (ends included) has a
/// nearest grid node passing `hit`.
fn segment_any(terrain: &Heightfield, a: FxVec2, b: FxVec2, hit: impl Fn(u32) -> bool) -> bool {
    let d = b - a;
    let steps = (d.length().raw() >> (Fx::FRAC_BITS + 1)).max(1);
    (0..=steps).any(|k| {
        let p = a + FxVec2::new(d.x.mul_int(k).div_int(steps), d.y.mul_int(k).div_int(steps));
        hit(node_at(terrain, p))
    })
}

/// Whether `p` is within `reach_sq` (raw Q32.32 squared metres) of the
/// polyline `line`.
fn near_line(line: &[FxVec2], p: FxVec2, reach_sq: u64) -> bool {
    line.windows(2).any(|w| {
        let (a, b) = (w[0], w[1]);
        let ab = b - a;
        let ap = p - a;
        let len_sq = ab.length_sq_raw() as i128;
        let t = ap.dot_raw(ab) as i128;
        let closest = if len_sq == 0 || t <= 0 {
            a
        } else if t >= len_sq {
            b
        } else {
            // a + ab · t / len², per axis in i128.
            let f = |o: Fx, d: Fx| {
                Fx::from_raw((o.raw() as i128 + d.raw() as i128 * t / len_sq) as i32)
            };
            FxVec2::new(f(a.x, ab.x), f(a.y, ab.y))
        };
        (p - closest).length_sq_raw() <= reach_sq
    })
}

// -------------------------------------------------------------- derived --

/// Derived vehicle data (see the module docs). Not saved and not part of
/// match-state equality.
#[derive(Clone, Debug, Default)]
pub struct VehicleDerived {
    /// Vehicles and wrecks at their hull origins.
    pub index: SpatialHash,
    /// Frames of every vehicle and wreck, ascending id.
    pub frames: Vec<VehicleFrame>,
    /// Per vehicle type.
    pub geo: Vec<Geo>,
    /// The largest [`Geo::reach`]: index queries pad by it.
    pub max_reach: Fx,
    pub block: BlockLayer,
}

impl PartialEq for VehicleDerived {
    /// Derived data is rebuilt from the state; it never decides equality.
    fn eq(&self, _: &VehicleDerived) -> bool {
        true
    }
}
impl Eq for VehicleDerived {}

impl VehicleDerived {
    /// Empty derived data sized for `setup` (call `rebuild` before use).
    pub fn new(setup: &MatchSetup, grid_dim: u32) -> VehicleDerived {
        let geo: Vec<Geo> = setup.vehicles.iter().map(Geo::of).collect();
        let max_reach = geo.iter().map(|g| g.reach).max().unwrap_or(Fx::ZERO);
        VehicleDerived {
            index: SpatialHash::new(setup.map.size_m),
            frames: Vec::new(),
            geo,
            max_reach,
            block: BlockLayer::new(grid_dim, BlockLayer::ditch_dm_of(setup)),
        }
    }

    /// The frame of vehicle or wreck `id`, if indexed.
    pub fn frame(&self, id: SimId) -> Option<&VehicleFrame> {
        self.frames
            .binary_search_by_key(&id, |f| f.id)
            .ok()
            .map(|k| &self.frames[k])
    }
}

/// Avoidance obstacles for the soldiers' ORCA (Movement stage): read only.
#[derive(Copy, Clone)]
pub(crate) struct Obstacles<'a> {
    pub derived: &'a VehicleDerived,
    pub velocity: &'a [Velocity],
}

impl Obstacles<'_> {
    /// Up to [`MAX_VEHICLE_CIRCLES`] vehicle circles nearest `pos` within
    /// `dist` of their rim: `(centre, radius, velocity)`, nearest first
    /// (ties by id, then circle).
    pub fn near(
        &self,
        pos: FxVec2,
        dist: Fx,
        out: &mut [(FxVec2, Fx, FxVec2); MAX_VEHICLE_CIRCLES],
    ) -> usize {
        let d = self.derived;
        let mut best: [(u64, SimId, u8); MAX_VEHICLE_CIRCLES] =
            [(u64::MAX, SimId(u32::MAX), 0); MAX_VEHICLE_CIRCLES];
        let mut n = 0usize;
        d.index.for_each_near(pos, dist + d.max_reach, |id| {
            let Some(f) = d.frame(id) else { return };
            let Some(geo) = d.geo.get(f.type_idx as usize) else {
                return;
            };
            let (cs, r) = f.circles(geo);
            let reach = (dist + r).raw() as u64;
            for (k, c) in cs.iter().enumerate() {
                let dsq = (*c - pos).length_sq_raw();
                if dsq >= reach * reach {
                    continue;
                }
                let key = (dsq, id, k as u8);
                if n < MAX_VEHICLE_CIRCLES {
                    best[n] = key;
                    n += 1;
                } else if key < best[MAX_VEHICLE_CIRCLES - 1] {
                    best[MAX_VEHICLE_CIRCLES - 1] = key;
                } else {
                    continue;
                }
                let mut j = n - 1;
                while j > 0 && best[j] < best[j - 1] {
                    best.swap(j, j - 1);
                    j -= 1;
                }
            }
        });
        for (o, &(_, id, k)) in out.iter_mut().zip(&best[..n]) {
            let f = d.frame(id).expect("indexed");
            let (cs, r) = f.circles(&d.geo[f.type_idx as usize]);
            *o = (cs[k as usize], r, self.velocity[id.index()].lin);
        }
        n
    }
}

// ---------------------------------------------------------------- drive --

/// Read-only inputs of [`drive`].
pub struct DriveCtx<'a> {
    pub terrain: &'a Heightfield,
    pub block: &'a BlockLayer,
    pub v: &'a VehicleSetup,
    pub geo: &'a Geo,
    pub rules: &'a VehicleRulesSetup,
    pub hz: u32,
    pub seed: u64,
}

/// Steps vehicle `id` one tick with the §5.12.3 inputs (see the module
/// docs): `throttle` and `steer` in −127..=127 (−128 counts as −127),
/// positive throttle forward, positive steer counter-clockwise. Writes the
/// pose, the speed, the planar velocity and the ground flags. Returns
/// whether the hull moved or turned.
pub fn drive(
    ctx: &DriveCtx,
    id: SimId,
    vs: &mut VehicleState,
    t: &mut Transform,
    vel: &mut Velocity,
    throttle: i8,
    steer: i8,
) -> bool {
    let (v, m, r) = (ctx.v, &ctx.v.mobility, ctx.rules);
    let hz = ctx.hz.max(1) as i32;
    let throttle = (throttle as i32).max(-127);
    let steer = (steer as i32).max(-127);
    let immobile = vs.immobile(v);
    if vs.bog > 0 {
        vs.bog -= 1;
    }

    // Speed limits (percent of the def's speeds).
    let mut pct: i64 = 100;
    let grade = grade_pct(vs.pitch) as i64;
    let max_slope = m.max_slope_pct.max(1) as i64;
    pct = pct * (100 - 70 * grade.min(max_slope) / max_slope) / 100;
    let xy = t.pos.xy();
    if ctx.terrain.woods_at(xy) {
        pct = pct * 50 / 100;
    }
    let driver = vs.seat(v, SeatRole::Driver).map_or(0, |s| s.hp as u32);
    if driver < r.wounded_hp {
        pct = pct * 80 / 100;
    }
    if (vs.component(ComponentKind::Engine) as u32) < 50 {
        pct = pct * 50 / 100;
    }
    let scale = |s: Fx| s.mul_int(pct as i32).div_int(100);
    let mut max_f = scale(m.speed.into());
    let mut max_r = scale(m.reverse.into());
    if vs.crossing {
        let cap: Fx = r.crossing_speed.into();
        max_f = max_f.min(cap);
        max_r = max_r.min(cap);
    }
    let target = if immobile {
        Fx::ZERO
    } else if throttle >= 0 {
        max_f.mul_int(throttle).div_int(127)
    } else {
        -max_r.mul_int(-throttle).div_int(127)
    };

    // Speed: accelerate away from zero, brake toward it.
    let accel = Fx::from(m.accel).div_int(hz);
    let brake = Fx::from(m.brake).div_int(hz);
    let s0 = vs.speed;
    let speed = if immobile {
        Fx::ZERO
    } else if target > s0 {
        approach(s0, target, if s0 >= Fx::ZERO { accel } else { brake })
    } else {
        approach(s0, target, if s0 > Fx::ZERO { brake } else { accel })
    };
    let turn = if immobile {
        Fx::ZERO
    } else {
        deg(m.turn_deg_s).mul_int(steer).div_int(127).div_int(hz)
    };

    // Candidate pose; refused steps fall back to turning only, then to
    // standing still.
    let yaw0 = t.yaw;
    let yaw1 = (yaw0 + turn).wrap_angle();
    let step = yaw_dir(yaw1) * speed.div_int(hz);
    let ok = |g: &Ground| {
        let gp = ground_pose(g, ctx.geo);
        !blocked(ctx, g)
            && !(grade_pct(gp.pitch) > m.max_slope_pct && gp.pitch.abs() > vs.pitch.abs())
    };
    let moving = step != FxVec2::ZERO;
    let turning = yaw1 != yaw0;
    let (mut xy1, mut yaw, mut speed) = (ctx.terrain.clamp(xy + step), yaw1, speed);
    let mut g = sample_ground(ctx.terrain, xy1, yaw, ctx.geo);
    if (moving || turning) && !ok(&g) {
        speed = Fx::ZERO;
        xy1 = xy;
        g = sample_ground(ctx.terrain, xy1, yaw, ctx.geo);
        if !(turning && ok(&g)) {
            yaw = yaw0;
            g = sample_ground(ctx.terrain, xy1, yaw, ctx.geo);
        }
    }

    // Ground flags: blocked footprint, crossing and the bog roll.
    vs.on_blocked = blocked(ctx, &g);
    let dug: Fx = r.crossing_dug.into();
    let over = g.depth.iter().flatten().any(|d| *d >= dug);
    if over {
        if vs.clear_ticks as u32 >= r.crossing_gap_ticks {
            vs.crossings = vs.crossings.saturating_add(1);
            let mut rr = RngStream::new(ctx.seed, vs.crossings as u64, id, Purpose::BOG);
            if rr.range_i32(0, 100) < m.bog_pct as i32 {
                let [lo, hi] = m.unbog_ticks;
                vs.bog = rr.range_i32(lo as i32, hi.max(lo) as i32) as u32;
                speed = Fx::ZERO;
            }
        }
        vs.clear_ticks = 0;
    } else {
        vs.clear_ticks = vs.clear_ticks.saturating_add(1);
    }
    vs.crossing = over;

    // Pose: pitch and roll follow the ground at a limited rate.
    let gp = ground_pose(&g, ctx.geo);
    let rate = deg(TILT_RATE_DEG_S).div_int(hz);
    vs.pitch = approach(vs.pitch, gp.pitch, rate);
    vs.roll = approach(vs.roll, gp.roll, rate);
    let changed = xy1 != xy || yaw != yaw0;
    t.pos = xy1.extend(gp.z);
    t.yaw = yaw;
    vs.speed = speed;
    vel.lin = (xy1 - xy).mul_int(hz);
    vs.avg_vel = vs.avg_vel + (vel.lin - vs.avg_vel).div_int(hz);
    changed
}

/// Whether any track sample of `g` is on a blocked node.
fn blocked(ctx: &DriveCtx, g: &Ground) -> bool {
    g.xy.iter()
        .flatten()
        .any(|p| ctx.block.blocked_at(ctx.terrain, *p))
}

/// Settles a vehicle on the ground at its current pose (spawn and setup
/// tools): height, pitch and roll at once, ground flags.
pub(crate) fn settle(
    terrain: &Heightfield,
    block: &BlockLayer,
    geo: &Geo,
    vs: &mut VehicleState,
    t: &mut Transform,
) {
    let g = sample_ground(terrain, t.pos.xy(), t.yaw, geo);
    let gp = ground_pose(&g, geo);
    t.pos.z = gp.z;
    vs.pitch = gp.pitch;
    vs.roll = gp.roll;
    vs.on_blocked = g.xy.iter().flatten().any(|p| block.blocked_at(terrain, *p));
}

// ------------------------------------------------------- path following --

/// Asks for a vehicle path to `goal`; the vehicle brakes until it is
/// served. A request already queued for the squad gets the new `avoid`.
pub(crate) fn request_vehicle_path(
    sq: &mut Squad,
    id: SquadId,
    queue: &mut PathQueue,
    goal: FxVec2,
    avoid: Option<u32>,
) {
    let nav = &mut sq.nav;
    nav.phase = NavPhase::PathPending;
    nav.goal = goal;
    nav.path.clear();
    nav.next = 0;
    nav.partial = false;
    nav.slots.clear();
    nav.phase_ticks = 0;
    let q = &mut queue.queues[sq.owner.0 as usize];
    match q
        .iter_mut()
        .find(|r| matches!(r, PathReq::Vehicle { squad, .. } if *squad == id))
    {
        Some(r) => *r = PathReq::Vehicle { squad: id, avoid },
        None => q.push(PathReq::Vehicle { squad: id, avoid }),
    }
}

/// Stops a vehicle squad where it stands (arrival, Stop).
pub(crate) fn hold_vehicle(nav: &mut Nav, pos: FxVec2) {
    nav.phase = NavPhase::Holding;
    nav.path.clear();
    nav.next = 0;
    nav.partial = false;
    nav.phase_ticks = 0;
    nav.slots = vec![pos];
}

/// Whether a `Move` from a hull at `t` to `target` reverses.
pub(crate) fn reverses_to(t: &Transform, target: FxVec2) -> bool {
    let d = target - t.pos.xy();
    d.dot_raw(yaw_dir(t.yaw)) < 0
        && d.length_sq_raw() <= FxVec2::from_int(REVERSE_ORDER_M, 0).length_sq_raw()
}

/// Whether an attack-moving vehicle halts: its main gun has a target and
/// can fire at it (gunner in his seat, gun whole, a round loaded or left).
pub(crate) fn halts_to_fire(vs: &VehicleState, v: &VehicleSetup) -> bool {
    vs.target.is_some()
        && vs.manned(v, SeatRole::Gunner)
        && vs.component(ComponentKind::Gun) > 0
        && (vs.loaded.is_some() || vs.ammo.iter().any(|n| *n > 0))
}

/// Speed cap as a throttle magnitude against the def's top speed.
fn throttle_for(cap: Fx, top: Fx) -> i32 {
    if top <= Fx::ZERO {
        return 0;
    }
    ((cap.raw() as i64 * 127) / top.raw() as i64).clamp(0, 127) as i32
}

/// The follower's re-path after it found itself stuck or its way newly
/// blocked: `(squad, goal, avoided node)`.
type Repath = (SquadId, FxVec2, Option<u32>);

impl MatchState {
    /// Vehicles of `player` alive plus vehicle call-ins queued (the limit's
    /// count).
    pub fn vehicles_counted(&self, ents: &Entities, player: PlayerId) -> u32 {
        let alive = self
            .vehicles
            .keys()
            .filter(|id| {
                ents.kind[id.index()] == EntityKind::Vehicle && ents.owner[id.index()] == player
            })
            .count() as u32;
        let queued = self.players[player.0 as usize]
            .call_ins
            .iter()
            .filter(|c| {
                self.setup.squad_types[c.squad_type as usize]
                    .vehicle
                    .is_some()
            })
            .count() as u32;
        alive + queued
    }

    /// Whether `id` is a vehicle squad.
    pub fn is_vehicle_squad(&self, id: SquadId) -> bool {
        self.squads.get(&id).is_some_and(|sq| {
            self.setup.squad_types[sq.type_idx as usize]
                .vehicle
                .is_some()
        })
    }

    /// Rebuilds the vehicle index and frames from the current poses.
    pub(crate) fn rebuild_vehicle_index(&mut self, ents: &Entities) {
        let d = &mut self.vderived;
        d.index.rebuild_kinds(ents, |k| {
            matches!(k, EntityKind::Vehicle | EntityKind::Wreck)
        });
        d.frames.clear();
        for (&id, vs) in &self.vehicles {
            let Some(geo) = d.geo.get(vs.type_idx as usize) else {
                continue;
            };
            let i = id.index();
            d.frames
                .push(frame_of(id, ents.owner[i], &ents.transform[i], vs, geo));
        }
    }

    /// Wreck footprints for the block layer.
    fn wreck_footprints(&self, ents: &Entities) -> Vec<Footprint> {
        self.vehicles
            .iter()
            .filter(|(_, vs)| vs.wreck_tick.is_some())
            .filter_map(|(id, vs)| {
                let geo = self.vderived.geo.get(vs.type_idx as usize)?;
                let t = &ents.transform[id.index()];
                let fwd = yaw_dir(t.yaw);
                Some(Footprint {
                    center: t.pos.xy() + fwd * geo.hull_center.x,
                    fwd,
                    half_x: geo.hull_half.x,
                    half_y: geo.hull_half.y,
                })
            })
            .collect()
    }

    /// Marks the block layer around a wreck footprint (added or removed).
    pub(crate) fn mark_wreck_block(&mut self, ents: &Entities, id: SimId) {
        let Some(vs) = self.vehicles.get(&id) else {
            return;
        };
        let Some(geo) = self.vderived.geo.get(vs.type_idx as usize) else {
            return;
        };
        let c = ents.transform[id.index()].pos.xy();
        let r = geo.reach + Fx::from_int(WRECK_PAD_M);
        let rv = FxVec2::new(r, r);
        self.vderived.block.mark_rect(c - rv, c + rv);
    }

    /// Marks the block layer around an AT-ditch line.
    pub(crate) fn mark_ditch_block(&mut self, line: &[FxVec2]) {
        let Some(&first) = line.first() else { return };
        let (mut lo, mut hi) = (first, first);
        for q in line {
            lo = FxVec2::new(lo.x.min(q.x), lo.y.min(q.y));
            hi = FxVec2::new(hi.x.max(q.x), hi.y.max(q.y));
        }
        let pad = FxVec2::from_int(DITCH_REACH_M, DITCH_REACH_M);
        self.vderived.block.mark_rect(lo - pad, hi + pad);
    }

    /// Brings the block layer up to date: the terrain tiles changed this
    /// tick plus the rectangles marked since (end of tick, and setup tools).
    pub(crate) fn update_block(
        &mut self,
        ents: &Entities,
        terrain: &Heightfield,
        tiles: &[(u16, u16)],
    ) {
        self.vderived.block.mark_tiles(tiles);
        let wrecks = self.wreck_footprints(ents);
        let MatchState {
            vderived,
            at_ditches,
            ..
        } = self;
        vderived.block.update(terrain, at_ditches, &wrecks);
    }

    /// Rebuilds every derived vehicle structure from the state (load, new
    /// match).
    pub(crate) fn rebuild_vehicle_derived(&mut self, ents: &Entities, terrain: &Heightfield) {
        self.vderived = VehicleDerived::new(&self.setup, terrain.dim());
        self.rebuild_vehicle_index(ents);
        self.vderived.block.mark_all();
        self.update_block(ents, terrain, &[]);
    }

    /// Turns vehicle `id` into a wreck at `tick` (knock-out): no squad, no
    /// crew, stopped, seen by everyone, blocking vehicles. Over the rules'
    /// `wreck_cap` the oldest wreck is removed and its id freed. Returns
    /// the removed wreck's id, if any. Followers, targets and the knock-out
    /// bookkeeping are the Damage stage's (S3 stage 4).
    pub(crate) fn make_wreck(
        &mut self,
        ents: &mut Entities,
        hash: &mut HashCache,
        id: SimId,
        tick: u64,
    ) -> Option<SimId> {
        let i = id.index();
        if ents.kind.get(i) != Some(&EntityKind::Vehicle) {
            return None;
        }
        if let Some(sid) = ents.squad[i].take() {
            self.squads.remove(&sid);
        }
        ents.kind[i] = EntityKind::Wreck;
        ents.velocity[i] = Velocity::default();
        ents.combat[i].seen_by = ((1u32 << self.players.len()) - 1) as u16;
        hash.mark_entity_chunk(id.chunk());
        if let Some(vs) = self.vehicles.get_mut(&id) {
            vs.wreck_tick = Some(tick);
            vs.speed = Fx::ZERO;
            vs.avg_vel = FxVec2::ZERO;
            vs.reverse = false;
            vs.target = None;
            vs.attack = None;
            vs.ranging = 0;
            for m in &mut vs.mgs {
                *m = MgState::default();
            }
            vs.repairing = None;
            vs.repair_progress = 0;
            vs.burn = 0;
            vs.base_ticks = 0;
            vs.followers.clear();
            vs.face = None;
            vs.reload = 0;
            vs.loaded = None;
            vs.bog = 0;
            for s in &mut vs.seats {
                *s = Seat::default();
            }
        }
        self.mark_wreck_block(ents, id);
        let cap = self.setup.rules.vehicles.wreck_cap as usize;
        let wrecks: Vec<(u64, SimId)> = self
            .vehicles
            .iter()
            .filter_map(|(&w, vs)| vs.wreck_tick.map(|t| (t, w)))
            .collect();
        if wrecks.len() <= cap {
            return None;
        }
        let (_, oldest) = *wrecks.iter().min().expect("over the cap");
        self.mark_wreck_block(ents, oldest);
        self.vehicles.remove(&oldest);
        ents.despawn(oldest);
        hash.mark_entity_chunk(oldest.chunk());
        Some(oldest)
    }

    /// Movement stage, after the soldiers: every vehicle follows its path
    /// (or stands), drives one step and pushes soldiers out of its
    /// footprint, serially in ascending id. `soldiers` is this tick's
    /// soldier hash (built before the soldiers moved; queries are padded).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn move_vehicles(
        &mut self,
        ents: &mut Entities,
        terrain: &Heightfield,
        soldiers: &SpatialHash,
        soldier_r: Fx,
        hash: &mut HashCache,
        seed: u64,
        tick: u64,
        events: &mut Vec<ArmourEv>,
    ) {
        if self.vehicles.is_empty() {
            return;
        }
        let hz = self.setup.tick_hz;
        let mut repath: Vec<Repath> = Vec::new();
        let mut bogged: Vec<(SimId, u8, FxVec3)> = Vec::new();
        let escorts: Vec<(SimId, Option<Fx>)> = self
            .vehicles
            .keys()
            .map(|&id| (id, self.escort_speed(ents, id)))
            .collect();
        let MatchState {
            setup,
            vehicles,
            squads,
            vderived,
            ..
        } = self;
        for (&id, vs) in vehicles.iter_mut() {
            let i = id.index();
            if ents.kind[i] != EntityKind::Vehicle {
                continue;
            }
            let v = &setup.vehicles[vs.type_idx as usize];
            let geo = &vderived.geo[vs.type_idx as usize];
            let (throttle, steer) =
                match ents.squad[i].and_then(|s| squads.get_mut(&s).map(|q| (s, q))) {
                    Some((sid, sq)) => follow(
                        &Follow {
                            id,
                            sid,
                            escort: escorts
                                .binary_search_by_key(&id, |e| e.0)
                                .ok()
                                .and_then(|k| escorts[k].1),
                            ents,
                            terrain,
                            soldiers,
                            derived: vderived,
                            v,
                            geo,
                            hz,
                            tick,
                        },
                        sq,
                        vs,
                        &mut repath,
                    ),
                    None => (0, 0),
                };
            let ctx = DriveCtx {
                terrain,
                block: &vderived.block,
                v,
                geo,
                rules: &setup.rules.vehicles,
                hz,
                seed,
            };
            let mut t = ents.transform[i];
            let mut vel = ents.velocity[i];
            let was_bogged = vs.bog > 0;
            let moved = drive(&ctx, id, vs, &mut t, &mut vel, throttle, steer);
            if vs.bog > 0 && !was_bogged {
                bogged.push((id, ents.owner[i].0, t.pos));
            }
            ents.transform[i] = t;
            ents.velocity[i] = vel;
            hash.mark_entity_chunk(id.chunk());
            if moved {
                let f = frame_of(id, ents.owner[i], &t, vs, geo);
                push_out(ents, terrain, soldiers, hash, &f, geo, soldier_r);
            }
        }
        for (id, owner, pos) in bogged {
            let ev = ArmourEv::new(ArmourEventKind::Bogged, id, owner, NONE_U8).at(pos);
            self.push_event(events, ev);
        }
        for (sid, goal, avoid) in repath {
            if let Some(sq) = self.squads.get_mut(&sid) {
                request_vehicle_path(sq, sid, &mut self.path_queue, goal, avoid);
                let lead = sq.members[0];
                if let Some(vs) = self.vehicles.get_mut(&lead) {
                    vs.repath_tick = Some(tick);
                    vs.progress_ticks = 0;
                }
            }
        }
    }
}

/// Read-only inputs of [`follow`].
struct Follow<'a> {
    id: SimId,
    sid: SquadId,
    /// The escort speed cap (`crate::orders`).
    escort: Option<Fx>,
    ents: &'a Entities,
    terrain: &'a Heightfield,
    soldiers: &'a SpatialHash,
    derived: &'a VehicleDerived,
    v: &'a VehicleSetup,
    geo: &'a Geo,
    hz: u32,
    tick: u64,
}

/// The RTS path follower: throttle and steer for this tick from the
/// squad's path (see the module docs). Arrival and the stuck and blocked
/// checks update the squad; re-paths go to `repath`.
fn follow(c: &Follow, sq: &mut Squad, vs: &mut VehicleState, repath: &mut Vec<Repath>) -> (i8, i8) {
    let i = c.id.index();
    let t = &c.ents.transform[i];
    let pos = t.pos.xy();
    let nav = &mut sq.nav;
    nav.phase_ticks = nav.phase_ticks.saturating_add(1);
    if nav.phase != NavPhase::Moving {
        vs.progress_at = pos;
        vs.progress_ticks = 0;
        // `FaceUnits`: pivot until the front points at the spot.
        let Some(f) = vs.face else { return (0, 0) };
        let to = f - pos;
        let err = (Fx::atan2(to.y, to.x) - t.yaw).wrap_angle();
        if to.length_sq_raw() == 0 || err.abs() <= deg(crate::orders::FACE_DONE_DEG) {
            vs.face = None;
            return (0, 0);
        }
        let full = deg(FULL_STEER_DEG);
        let steer = (err.raw() as i64 * 127 / full.raw() as i64).clamp(-127, 127);
        return (0, steer as i8);
    }
    if nav.phase_ticks <= 1 {
        vs.progress_at = pos;
        vs.progress_ticks = 0;
    }
    // Attack-move: halt while the main gun has a target to fire at.
    if matches!(sq.order, Order::AttackMove { .. }) && halts_to_fire(vs, c.v) {
        vs.progress_at = pos;
        vs.progress_ticks = 0;
        return (0, 0);
    }
    let reach_sq = FxVec2::from_int(WAYPOINT_REACH_M, 0).length_sq_raw();
    let last = nav.path.len() as u32 - 1;
    while nav.next < last && (nav.path[nav.next as usize] - pos).length_sq_raw() <= reach_sq {
        nav.next += 1;
    }
    let wp = nav.path[nav.next as usize];
    let to = wp - pos;
    let dist = to.length();
    if nav.next == last && dist <= Fx::from_milli(ARRIVE_MM) {
        if nav.partial {
            repath.push((c.sid, nav.goal, None));
        } else {
            hold_vehicle(nav, pos);
            if matches!(
                sq.order,
                Order::Move { .. } | Order::AttackMove { .. } | Order::Retreat
            ) {
                sq.order = Order::Idle;
            }
            vs.reverse = false;
        }
        return (0, 0);
    }
    if dist > Fx::from_milli(250) {
        nav.heading = to.normalize_or_zero();
    }
    let gap_ok = vs
        .repath_tick
        .is_none_or(|r| c.tick >= r + (REPATH_GAP_S * c.hz) as u64);

    // Stuck: under 1 m of progress in 5 s re-paths around the node ahead.
    let fwd = yaw_dir(t.yaw);
    let dir = if vs.reverse { -fwd } else { fwd };
    // Backing off (below) is not progress.
    let backing =
        (BACK_OFF_S * c.hz..(BACK_OFF_S + 1) * c.hz).contains(&(vs.progress_ticks as u32));
    if !backing
        && (pos - vs.progress_at).length_sq_raw()
            >= FxVec2::from_int(STUCK_PROGRESS_M, 0).length_sq_raw()
    {
        vs.progress_at = pos;
        vs.progress_ticks = 0;
    } else {
        vs.progress_ticks = vs.progress_ticks.saturating_add(1);
    }
    if vs.progress_ticks as u32 >= STUCK_S * c.hz && gap_ok {
        let ahead = pos + dir * (c.geo.hull_half.x + Fx::from_int(SPACING_M));
        repath.push((c.sid, nav.goal, Some(node_at(c.terrain, ahead))));
        return (0, 0);
    }
    // Pressed against something for 2 s: back off for 1 s (a hull nose to
    // nose with blocked ground cannot pivot).
    if (BACK_OFF_S * c.hz..(BACK_OFF_S + 1) * c.hz).contains(&(vs.progress_ticks as u32)) {
        return (if vs.reverse { 80 } else { -80 }, 0);
    }
    // Every 0.5 s: has the way ahead become blocked?
    let every = (c.hz / 2).max(1);
    if nav.phase_ticks.is_multiple_of(every) && gap_ok {
        let next2 = nav.path.get(nav.next as usize + 1).copied();
        let b = &c.derived.block;
        if b.segment_blocked(c.terrain, pos + dir * c.geo.hull_half.x, wp)
            || next2.is_some_and(|n| b.segment_blocked(c.terrain, wp, n))
        {
            repath.push((c.sid, nav.goal, None));
            return (0, 0);
        }
    }

    // Heading and throttle; keep right of a vehicle coming head-on.
    let (cap, head_on) = yield_cap(c, dir);
    let mut desired = Fx::atan2(to.y, to.x);
    if vs.reverse {
        desired = (desired + Fx::PI).wrap_angle();
    }
    if head_on {
        desired = (desired - deg(KEEP_RIGHT_DEG)).wrap_angle();
    }
    let err = (desired - t.yaw).wrap_angle();
    let full = deg(FULL_STEER_DEG);
    let steer = (err.raw() as i64 * 127 / full.raw() as i64).clamp(-127, 127) as i32;
    let ae = err.abs();
    let (lo, hi) = (deg(FULL_THROTTLE_DEG), deg(PIVOT_ERR_DEG));
    let mut throttle: i32 = if ae >= hi {
        0
    } else if ae <= lo {
        127
    } else {
        ((hi - ae).raw() as i64 * 127 / (hi - lo).raw() as i64) as i32
    };
    let top: Fx = if vs.reverse {
        c.v.mobility.reverse.into()
    } else {
        c.v.mobility.speed.into()
    };
    // Brake for the last waypoint: v² = 2·brake·d.
    if nav.next == last {
        let cap = (Fx::from(c.v.mobility.brake).mul_int(2) * dist).sqrt();
        throttle = throttle.min(throttle_for(cap, top).max(8));
    }
    if let Some(cap) = cap {
        throttle = throttle.min(throttle_for(cap, top));
    }
    // Escorted: no faster than the followers walk.
    if let Some(cap) = c.escort {
        throttle = throttle.min(throttle_for(cap, top).max(8));
    }
    if ae >= hi && vs.speed.abs() < Fx::from_milli(PIVOT_SPEED_MM_S) {
        // Pivot on the spot.
        throttle = 0;
    }
    if vs.reverse {
        throttle = -throttle;
    }
    (throttle as i8, steer as i8)
}

/// Speed cap from what is ahead (`None`: no cap). A wreck or a standing
/// vehicle in the yield box is braked for (the speed from which the hull
/// stops 1 m short of it), one coming head-on too, in half the gap (it
/// brakes as well); the box is at least 1.5 hull lengths and long enough
/// for that at top speed. One driving the same
/// way is followed at its speed; one crossing is yielded to by the higher
/// id only. A friendly soldier within 4 m ahead caps at walking pace.
fn yield_cap(c: &Follow, dir: FxVec2) -> (Option<Fx>, bool) {
    let i = c.id.index();
    let t = &c.ents.transform[i];
    let owner = c.ents.owner[i];
    let pos = t.pos.xy();
    let hx = c.geo.hull_half.x;
    let hy = c.geo.hull_half.y;
    let front = pos + dir * hx;
    let brake2 = Fx::from(c.v.mobility.brake).mul_int(2);
    let top = Fx::from(c.v.mobility.speed);
    // At least 1.5 hull lengths, and long enough for two vehicles closing
    // head-on at top speed to stop (each in half the gap, plus 2 m).
    let len = hx
        .mul_int(2)
        .mul_int(YIELD_LEN_PCT)
        .div_int(100)
        .max((top * top / brake2).mul_int(2) + Fx::from_int(2));
    let mut cap: Option<Fx> = None;
    let mut head_on = false;
    let mut lower = |v: Fx| cap = Some(cap.map_or(v, |c| c.min(v)));
    let stop_at = |along: Fx| (brake2 * (along - Fx::ONE).max(Fx::ZERO)).sqrt();
    let still = Fx::from_milli(YIELD_STILL_MM_S);
    c.derived
        .index
        .for_each_near(front, len + c.derived.max_reach, |other| {
            if other == c.id {
                return;
            }
            let Some(f) = c.derived.frame(other) else {
                return;
            };
            let Some(og) = c.derived.geo.get(f.type_idx as usize) else {
                return;
            };
            let (cs, r) = f.circles(og);
            let mut along_min: Option<Fx> = None;
            for cc in cs {
                let rel = cc - front;
                let along = rel.dot(dir);
                let lat = dir.det(rel).abs();
                if along >= -r && along <= len + r && lat <= hy + r + Fx::from_milli(CIRCLE_PAD_MM)
                {
                    along_min = Some(along_min.map_or(along, |a: Fx| a.min(along)));
                }
            }
            // Gap from our front to the other's circle rim.
            let Some(along) = along_min.map(|a| a - r) else {
                return;
            };
            let ov = c.ents.velocity[other.index()].lin;
            let toward = ov.dot(dir);
            if toward < -still {
                // Head-on: both brake, each in half the gap, and keep right.
                lower(stop_at(along.div_int(2)));
                head_on = true;
            } else if f.wreck || ov.length() <= still {
                lower(stop_at(along));
            } else if toward > still {
                // Same way: no faster than it while close.
                if along < hx.mul_int(2) {
                    lower(toward);
                }
            } else if c.id > other {
                lower(stop_at(along));
            }
        });
    let ahead = Fx::from_int(FRIENDLY_AHEAD_M);
    let walk = Fx::from_milli(WALK_MM_S);
    c.soldiers.for_each_near(front, ahead + hy + Fx::ONE, |s| {
        let si = s.index();
        if c.ents.owner[si] != owner || c.ents.kind[si] != EntityKind::Soldier {
            return;
        }
        let rel = c.ents.transform[si].pos.xy() - front;
        let along = rel.dot(dir);
        if along >= Fx::ZERO && along <= ahead && dir.det(rel).abs() <= hy + Fx::HALF {
            lower(walk);
        }
    });
    (cap, head_on)
}

/// Pushes soldiers out of a moving vehicle's footprint, sideways (see the
/// module docs).
fn push_out(
    ents: &mut Entities,
    terrain: &Heightfield,
    soldiers: &SpatialHash,
    hash: &mut HashCache,
    f: &VehicleFrame,
    geo: &Geo,
    soldier_r: Fx,
) {
    let c = f.hull.center.xy();
    let fwd = yaw_dir(f.yaw);
    let left = fwd.perp();
    let (hx, hy) = (geo.hull_half.x, geo.hull_half.y);
    let belly = geo.hull_center.z - geo.hull_half.z;
    let reach = geo.reach + Fx::from_int(2);
    let mut hits: Vec<SimId> = Vec::new();
    soldiers.for_each_near(c, reach, |s| hits.push(s));
    hits.sort_unstable();
    let depth = |p: FxVec2| (terrain.base_height_at(p) - terrain.height_at(p)).max(Fx::ZERO);
    for s in hits {
        let si = s.index();
        if ents.kind[si] != EntityKind::Soldier {
            continue;
        }
        let pos = ents.transform[si].pos;
        let rel = pos.xy() - c;
        let lx = rel.dot(fwd);
        let ly = rel.dot(left);
        if lx.abs() > hx || ly.abs() > hy {
            continue;
        }
        // The hull's belly above his position: a man below it (in a trench
        // under the hull) stays where he is.
        let ox = lx + geo.hull_center.x;
        let a = &f.hull.axes;
        let belly_z = f.origin.z + a.x.z * ox + a.y.z * ly + a.z.z * belly;
        if pos.z + ents.combat[si].height < belly_z {
            continue;
        }
        let here = depth(pos.xy());
        let out = hy + soldier_r + Fx::from_milli(PUSH_GAP_MM);
        let first = if ly >= Fx::ZERO { Fx::ONE } else { Fx::NEG_ONE };
        let deeper = Fx::from_milli(PUSH_DEEPER_MM);
        let dest = [first, -first].into_iter().find_map(|side| {
            let p = terrain.clamp(c + fwd * lx + left * (out * side));
            (depth(p) <= here + deeper).then_some(p)
        });
        if let Some(p) = dest {
            ents.transform[si].pos = p.extend(terrain.height_at(p));
            hash.mark_entity_chunk(s.chunk());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::TerrainKind;
    use frontline_content::{Difficulty, MatchRequest, ModDefs};

    fn setup() -> MatchSetup {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
        let mut defs = ModDefs::load(&root).unwrap();
        let rules = defs
            .add_rules_override(&root.join("tests/rules_test_unlock.toml"))
            .unwrap();
        let mut req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
        req.rules = rules;
        defs.resolve(&req).unwrap()
    }

    /// A Panzer IV on flat 256 m ground at (100, 100) heading east.
    struct Rig {
        s: MatchSetup,
        terrain: Heightfield,
        block: BlockLayer,
        geo: Geo,
        vs: VehicleState,
        t: Transform,
        vel: Velocity,
    }

    impl Rig {
        fn new() -> Rig {
            let s = setup();
            let terrain = Heightfield::generate(TerrainKind::Flat, 256, 1);
            let block = BlockLayer::new(terrain.dim(), BlockLayer::ditch_dm_of(&s));
            let geo = Geo::of(&s.vehicles[0]);
            let vs = VehicleState::new(0, &s.vehicles[0], &s.rules.vehicles);
            let mut r = Rig {
                s,
                terrain,
                block,
                geo,
                vs,
                t: Transform::default(),
                vel: Velocity::default(),
            };
            r.place(FxVec2::from_int(100, 100), Fx::ZERO);
            r
        }

        fn place(&mut self, p: FxVec2, yaw: Fx) {
            self.t.pos = p.extend(Fx::ZERO);
            self.t.yaw = yaw;
            settle(
                &self.terrain,
                &self.block,
                &self.geo,
                &mut self.vs,
                &mut self.t,
            );
        }

        fn drive(&mut self, throttle: i8, steer: i8, ticks: u32) {
            for _ in 0..ticks {
                let ctx = DriveCtx {
                    terrain: &self.terrain,
                    block: &self.block,
                    v: &self.s.vehicles[0],
                    geo: &self.geo,
                    rules: &self.s.rules.vehicles,
                    hz: 30,
                    seed: 7,
                };
                drive(
                    &ctx,
                    SimId(3),
                    &mut self.vs,
                    &mut self.t,
                    &mut self.vel,
                    throttle,
                    steer,
                );
                assert!(!self.on_blocked(), "a track sample on a blocked node");
            }
        }

        fn on_blocked(&self) -> bool {
            let g = sample_ground(&self.terrain, self.t.pos.xy(), self.t.yaw, &self.geo);
            g.xy.iter()
                .flatten()
                .any(|p| self.block.blocked_at(&self.terrain, *p))
        }

        fn xy(&self) -> FxVec2 {
            self.t.pos.xy()
        }

        /// A trench along `line` (the rules' profile, finished).
        fn trench(&mut self, line: &[FxVec2]) {
            let d = self.s.rules.dig.clone();
            let n = d.passes as i32;
            let len = crate::squad::polyline_length(line);
            crate::dig::stamp(&mut self.terrain, line, 1, Fx::ZERO, len, n, n, &d, 0);
        }
    }

    #[test]
    fn drive_accelerates_turns_and_pivots() {
        let mut r = Rig::new();
        r.drive(127, 0, 90);
        // 1.5 m/s² for 3 s.
        assert!(
            (r.vs.speed - Fx::from_milli(4500)).abs() < Fx::from_milli(60),
            "{:?}",
            r.vs.speed
        );
        assert_eq!(r.t.yaw, Fx::ZERO);
        assert!(r.xy().x > Fx::from_int(106));
        r.drive(127, 0, 120);
        assert_eq!(r.vs.speed, Fx::from_int(6), "top speed");
        // Braking at 4 m/s²: 1.5 s to stand.
        r.drive(0, 0, 46);
        assert_eq!(r.vs.speed, Fx::ZERO);
        // Pivot: 30°/s on the spot.
        let at = r.xy();
        r.drive(0, 127, 30);
        assert_eq!(r.xy(), at);
        assert!(
            (r.t.yaw - deg(30)).abs() < Fx::from_milli(10),
            "{:?}",
            r.t.yaw
        );
        // Reverse at most 2 m/s.
        r.drive(-127, 0, 90);
        assert_eq!(r.vs.speed, Fx::from_int(-2));
        assert!(r.vel.lin.dot(yaw_dir(r.t.yaw)) < Fx::ZERO);
    }

    #[test]
    fn immobile_vehicles_do_not_move() {
        let mut r = Rig::new();
        let at = r.xy();
        r.vs.seats[0].hp = 0; // driver
        r.drive(127, 127, 30);
        assert_eq!(r.xy(), at);
        assert_eq!(r.t.yaw, Fx::ZERO);
        r.vs.seats[0].hp = 100;
        r.vs.components[ComponentKind::TrackLeft.index()] = 0;
        r.drive(127, 0, 30);
        assert_eq!(r.xy(), at);
        r.vs.components[ComponentKind::TrackLeft.index()] = 100;
        r.vs.bog = 10;
        r.drive(127, 0, 10);
        assert_eq!(r.xy(), at);
        assert_eq!(r.vs.bog, 0);
        r.drive(127, 0, 10);
        assert!(r.xy().x > at.x, "free again");
        // A damaged engine halves top speed; a wounded driver 80 %.
        let mut r = Rig::new();
        r.vs.components[ComponentKind::Engine.index()] = 40;
        r.vs.seats[0].hp = 40;
        r.drive(127, 0, 200);
        assert_eq!(r.vs.speed, Fx::from_int(6).mul_int(50 * 80).div_int(10_000));
    }

    #[test]
    fn drive_refuses_blocked_ground_and_a_footprint_on_it_is_immobile() {
        let mut r = Rig::new();
        // A wreck footprint across the way at x = 130.
        let wreck = Footprint {
            center: FxVec2::from_int(130, 100),
            fwd: FxVec2::new(Fx::ZERO, Fx::ONE),
            half_x: Fx::from_int(3),
            half_y: Fx::from_int(1),
        };
        r.block.mark_all();
        r.block.update(&r.terrain, &[], &[wreck]);
        assert!(r.block.blocked_at(&r.terrain, FxVec2::from_int(130, 100)));
        // Full throttle into it for 10 s: stops short (`drive` asserts no
        // sample is ever on a blocked node).
        r.drive(127, 0, 300);
        assert!(r.xy().x < Fx::from_int(124), "{:?}", r.xy());
        assert_eq!(r.vs.speed, Fx::ZERO);
        // Nose against it, a pivot would swing a front corner in; backing
        // off first frees it.
        r.drive(-127, 0, 30);
        r.drive(0, 127, 30);
        assert!(r.t.yaw > deg(20));
        // Ground that becomes blocked under it: immobile until it opens.
        let mut r = Rig::new();
        let under = Footprint {
            center: r.xy(),
            ..wreck
        };
        r.block.mark_all();
        r.block.update(&r.terrain, &[], &[under]);
        let at = r.xy();
        let ctx_drive = |r: &mut Rig| {
            let ctx = DriveCtx {
                terrain: &r.terrain,
                block: &r.block,
                v: &r.s.vehicles[0],
                geo: &r.geo,
                rules: &r.s.rules.vehicles,
                hz: 30,
                seed: 7,
            };
            drive(&ctx, SimId(3), &mut r.vs, &mut r.t, &mut r.vel, 127, 0);
        };
        for _ in 0..30 {
            ctx_drive(&mut r);
        }
        assert!(r.vs.on_blocked);
        assert_eq!(r.xy(), at);
        r.block.mark_all();
        r.block.update(&r.terrain, &[], &[]);
        for _ in 0..30 {
            ctx_drive(&mut r);
        }
        assert!(!r.vs.on_blocked);
        assert!(r.xy().x > at.x);
    }

    #[test]
    fn slopes_slow_and_steep_ones_stop_a_climb() {
        // A 40 % ramp from x = 120 to 160, then a 70 % wall from x = 200.
        let mut r = Rig::new();
        r.terrain.edit_heights_for_test(|gx, _, h| {
            let x = gx as i32 * SPACING_M;
            *h = if x <= 120 {
                Fx::ZERO
            } else if x <= 160 {
                Fx::from_int(x - 120).mul_int(40).div_int(100)
            } else if x <= 200 {
                Fx::from_int(16)
            } else {
                Fx::from_int(16) + Fx::from_int(x - 200).mul_int(70).div_int(100)
            };
        });
        r.place(FxVec2::from_int(100, 100), Fx::ZERO);
        let mut slowest = Fx::from_int(10);
        for _ in 0..30 * 40 {
            r.drive(127, 0, 1);
            let x = r.xy().x;
            if x > Fx::from_int(135) && x < Fx::from_int(150) {
                slowest = slowest.min(r.vs.speed);
            }
        }
        // 40 % of 50 %: 100 − 70·0.8 = 44 % of 6 m/s, about 2.6 m/s.
        assert!(
            slowest < Fx::from_int(3) && slowest > Fx::from_int(2),
            "{slowest:?}"
        );
        assert!(r.vs.pitch.abs() <= deg(MAX_TILT_DEG));
        assert!(
            r.xy().x > Fx::from_int(160),
            "climbed the ramp: {:?}",
            r.xy()
        );
        assert!(
            r.xy().x < Fx::from_int(206),
            "stopped at the wall: {:?}",
            r.xy()
        );
    }

    #[test]
    fn pose_bridges_narrow_ditches_and_follows_slopes() {
        let mut r = Rig::new();
        // A 1.6 m wide trench under the middle of the hull, across it.
        r.trench(&[FxVec2::from_int(100, 80), FxVec2::from_int(100, 120)]);
        r.place(FxVec2::from_int(100, 100), Fx::ZERO);
        // A nose dive would be about 29°; it rests on the edges (or the
        // parapet just behind the trench).
        assert!(r.vs.pitch.abs() < deg(5), "no nose dive: {:?}", r.vs.pitch);
        assert!(r.t.pos.z > -Fx::from_milli(50), "{:?}", r.t.pos.z);
        // Rear on flat ground, front up a 25 % slope: nose up.
        let mut r = Rig::new();
        r.terrain.edit_heights_for_test(|gx, _, h| {
            let x = gx as i32 * SPACING_M;
            *h = Fx::from_int((x - 100).max(0)).div_int(4);
        });
        r.place(FxVec2::from_int(100, 100), Fx::ZERO);
        assert!(
            r.vs.pitch > deg(5) && r.vs.pitch < deg(14),
            "{:?}",
            r.vs.pitch
        );
        // Heading north across the same slope: rolled, left side down
        // (the left is west, lower).
        r.place(FxVec2::from_int(120, 100), Fx::FRAC_PI_2);
        assert!(r.vs.roll < -deg(5), "{:?}", r.vs.roll);
        assert!(r.vs.pitch.abs() < deg(1));
    }

    #[test]
    fn a_trench_crossing_rolls_the_bog_once() {
        let mut r = Rig::new();
        r.s.vehicles[0].mobility.bog_pct = 100;
        r.s.vehicles[0].mobility.unbog_ticks = [60, 60];
        r.trench(&[FxVec2::from_int(130, 60), FxVec2::from_int(130, 140)]);
        // Over the trench at full throttle: capped at 1 m/s, bogged once.
        let mut capped = false;
        let mut bogged_ticks = 0;
        for _ in 0..30 * 40 {
            r.drive(127, 0, 1);
            if r.vs.crossing {
                capped |= r.vs.speed <= Fx::ONE && r.vs.speed > Fx::ZERO;
            }
            bogged_ticks += (r.vs.bog > 0) as u32;
        }
        assert!(capped);
        assert_eq!(r.vs.crossings, 1);
        assert_eq!(bogged_ticks, 60);
        assert!(r.xy().x > Fx::from_int(140), "{:?}", r.xy());
        // Driving along a trench with the left track in it: one crossing,
        // not one per node. (A hull straddling a trench has no track
        // sample over it: no crossing at all.)
        let mut r = Rig::new();
        r.s.vehicles[0].mobility.bog_pct = 0;
        r.trench(&[FxVec2::from_int(60, 102), FxVec2::from_int(200, 102)]);
        r.place(FxVec2::from_int(70, 101), Fx::ZERO);
        r.drive(127, 0, 30 * 60);
        assert_eq!(r.vs.crossings, 1);
    }

    #[test]
    fn push_out_moves_men_aside_but_never_one_in_a_trench_under_the_hull() {
        let mut r = Rig::new();
        r.trench(&[FxVec2::from_int(100, 60), FxVec2::from_int(100, 140)]);
        let mut ents = Entities::default();
        let mut put = |p: FxVec2, h: Fx| {
            let id = ents.spawn(EntityKind::Soldier, PlayerId(1));
            ents.transform[id.index()].pos = p.extend(r.terrain.height_at(p));
            ents.combat[id.index()].height = h;
            id
        };
        // Hull centred at (102, 100): one man in the trench at x = 100,
        // one standing in the open at x = 104 just left of the axis, one
        // right of the axis.
        let deep = put(FxVec2::from_int(100, 100), Fx::from_milli(1000));
        let open = put(
            FxVec2::new(Fx::from_int(104), Fx::from_milli(100_300)),
            Fx::from_milli(1700),
        );
        let right = put(
            FxVec2::new(Fx::from_int(103), Fx::from_milli(99_500)),
            Fx::from_milli(1700),
        );
        let mut hash = HashCache::default();
        let mut spatial = SpatialHash::new(256);
        spatial.rebuild(&ents);
        r.place(FxVec2::from_int(102, 100), Fx::ZERO);
        let f = frame_of(SimId(9), PlayerId(0), &r.t, &r.vs, &r.geo);
        let before = ents.transform[deep.index()].pos;
        push_out(
            &mut ents,
            &r.terrain,
            &spatial,
            &mut hash,
            &f,
            &r.geo,
            Fx::from_milli(300),
        );
        assert_eq!(
            ents.transform[deep.index()].pos,
            before,
            "spared under the hull"
        );
        let out = Fx::from_int(100) + r.geo.hull_half.y + Fx::from_milli(300);
        assert!(ents.transform[open.index()].pos.y >= out - Fx::from_milli(10));
        assert!(
            ents.transform[right.index()].pos.y <= Fx::from_int(200) - out + Fx::from_milli(10)
        );
        assert_eq!(ents.transform[open.index()].pos.x, Fx::from_int(104));
    }

    #[test]
    fn block_layer_follows_ditch_lines_wrecks_and_dirty_rects() {
        let s = setup();
        let mut terrain = Heightfield::generate(TerrainKind::Flat, 256, 1);
        let mut b = BlockLayer::new(terrain.dim(), BlockLayer::ditch_dm_of(&s));
        assert_eq!(b.ditch_dm, 19);
        // A deep cut (2.3 m) along y = 100 that is not an AT-ditch line:
        // depth alone never blocks.
        let d = s.rules.at_ditch.clone();
        let n = d.passes as i32;
        let line = vec![FxVec2::from_int(60, 100), FxVec2::from_int(90, 100)];
        crate::dig::stamp(
            &mut terrain,
            &line,
            1,
            Fx::ZERO,
            Fx::from_int(30),
            n,
            n,
            &d,
            0,
        );
        // Stacked craters on it go deeper still (additive): still open.
        for k in 0..4 {
            terrain.crater(
                FxVec2::from_int(70 + k, 100),
                Fx::from_milli(600),
                Fx::ONE,
                1,
            );
        }
        assert!(terrain.dug_dm(terrain.nearest_grid(FxVec2::from_int(72, 100)).0, 25) > 25);
        b.mark_all();
        b.update(&terrain, &[], &[]);
        assert_eq!(b.blocked_count(), 0);
        // As a ditch line it blocks, dilated by a node, with a clearance band.
        b.mark_all();
        b.update(&terrain, std::slice::from_ref(&line), &[]);
        let at = |x: i32, y: i32| node_at(&terrain, FxVec2::from_int(x, y));
        assert!(b.is_blocked(at(76, 100)) && b.is_blocked(at(76, 104)));
        assert!(!b.is_blocked(at(76, 108)) && b.is_margin(at(76, 108)));
        assert!(!b.is_margin(at(76, 112)));
        // A rect update outside leaves it; one over it recomputes it.
        b.mark_rect(FxVec2::from_int(200, 200), FxVec2::from_int(210, 210));
        b.update(&terrain, &[], &[]);
        assert!(b.is_blocked(at(76, 100)));
        b.mark_rect(FxVec2::from_int(70, 96), FxVec2::from_int(80, 104));
        b.update(&terrain, &[], &[]);
        assert!(!b.is_blocked(at(76, 100)));
    }

    #[test]
    fn hull_basis_is_orthonormal_and_follows_the_convention() {
        let b = Basis::hull(deg(30), deg(10), deg(-5));
        let one = Fx::ONE.raw() as i64;
        for v in [b.x, b.y, b.z] {
            let l = v.length().raw() as i64;
            assert!((l - one).abs() < 40, "{l}");
        }
        assert!(b.x.dot(b.y).abs() < Fx::from_milli(2));
        assert!(b.x.dot(b.z).abs() < Fx::from_milli(2));
        assert!(b.y.dot(b.z).abs() < Fx::from_milli(2));
        // Positive pitch raises the nose; negative roll lowers the left.
        assert!(b.x.z > Fx::ZERO);
        assert!(b.y.z < Fx::ZERO);
        // Turned basis keeps up.
        let t = b.turned(deg(90));
        assert_eq!(t.z, b.z);
        assert!((t.x - b.y).length() < Fx::from_milli(2));
    }

    #[test]
    fn approach_is_bounded() {
        assert_eq!(approach(Fx::ZERO, Fx::ONE, Fx::HALF), Fx::HALF);
        assert_eq!(approach(Fx::ZERO, Fx::HALF, Fx::ONE), Fx::HALF);
        assert_eq!(approach(Fx::ONE, -Fx::ONE, Fx::HALF), Fx::HALF);
    }

    #[test]
    fn near_line_measures_segment_distance() {
        let l = vec![FxVec2::from_int(10, 10), FxVec2::from_int(30, 10)];
        let r = FxVec2::from_int(2, 0).length_sq_raw();
        assert!(near_line(&l, FxVec2::from_int(20, 12), r));
        assert!(!near_line(&l, FxVec2::from_int(20, 13), r));
        assert!(near_line(&l, FxVec2::from_int(31, 11), r));
        assert!(!near_line(&l, FxVec2::from_int(33, 10), r));
    }
}
