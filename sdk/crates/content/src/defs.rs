//! Def files of a content mod (§7.1, §7.4): squads, weapons, items,
//! factions, maps and rules, one TOML file each under `defs/<kind>/`.
//!
//! Every def has a namespaced id (`ww2:kar98k`, §7.3). Values are integers
//! with the unit in the field name (`range_m`, `reload_ms`, `spread_mrad`),
//! so lowering into the fixed-point sim never depends on float rounding.
//! Unknown keys are errors. [`validate`](SquadDef::validate) methods check one
//! file on its own; cross-file references are checked by
//! [`crate::ModDefs`].

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::issue::Issue;
use crate::tuning::{
    AiArmourTuning, AiInfantryTuning, BlastTuning, BodyTuning, CoverTuning, DigCommonTuning,
    DrivingTuning, EscortTuning, FormationTuning, GunneryTuning, MortarTuning, SuppressionTuning,
    ThrowTuning, TuningRefs, VehiclePathTuning, check_tuning,
};

/// Map side limits (the sim's 64 m tiling).
pub const MAP_SIZE_STEP_M: u32 = 64;
pub const MAP_SIZE_MAX_M: u32 = 8192;
/// Most men in one squad.
pub const MAX_SQUAD_MEN: u8 = 16;

/// What a squad does on the field. Drives AI choices and which orders apply
/// (only squads that can dig take `Dig`, only mortars take `Barrage`).
/// A vehicle is a squad of one (`Vehicle`, S3): its squad def names the
/// vehicle def, which holds cost, pop, call-in time and sight.
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SquadRole {
    Rifle,
    Mg,
    Engineer,
    Mortar,
    /// Infantry anti-tank team: a launcher gunner and a loader (S3).
    AntiTank,
    /// A vehicle (S3): one per squad, see `SquadDef::vehicle`.
    Vehicle,
}

/// A buyable squad (`defs/units/*.toml`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SquadDef {
    /// Namespaced id, e.g. `ww2:ger_rifle_squad`.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Faction id that fields this squad.
    pub faction: String,
    pub role: SquadRole,
    /// `vehicle` squads: the vehicle def id. Its def holds cost, pop,
    /// call-in time, speed and sight, so the soldier fields below stay out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vehicle: Option<String>,
    /// Soldiers in the squad, 1..=16 (soldier squads; required).
    #[serde(default)]
    pub men: u8,
    /// Weapon id the first `weapon_men` men carry (soldier squads;
    /// required).
    #[serde(default)]
    pub weapon: String,
    /// How many men carry `weapon` (default: all). The rest carry
    /// `sidearm`; an AT team has one launcher gunner and a rifleman.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weapon_men: Option<u8>,
    /// Weapon id of the men without `weapon` (a rifle, SMG or MG).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidearm: Option<String>,
    /// Manpower cost of a call-in (soldier squads).
    #[serde(default)]
    pub cost: u32,
    /// Seconds from call-in to arrival at the base (soldier squads).
    #[serde(default)]
    pub call_in_s: u32,
    /// Hit points per soldier.
    #[serde(default = "default_hp")]
    pub hp: u32,
    /// Walking speed in cm/s (soldier squads; required).
    #[serde(default)]
    #[schemars(range(min = 10, max = 1000))]
    pub speed_cm_s: u32,
    /// Sight range in metres (soldier squads; required).
    #[serde(default)]
    pub sight_m: u32,
    /// Formation spacing between men in decimetres (soldier squads;
    /// required).
    #[serde(default)]
    #[schemars(range(min = 5, max = 200))]
    pub spacing_dm: u32,
    /// Digging speed per man in percent of the base rate; 0 = cannot dig.
    #[serde(default)]
    pub dig_rate_pct: u32,
    /// Items the men carry (S1b abilities); at most one entry per item
    /// kind. Empty: nothing to throw.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loadout: Vec<LoadoutDef>,
}

/// How many of one item a squad's men carry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LoadoutDef {
    /// Item id, e.g. `ww2:stielhandgranate`.
    pub item: String,
    /// Carried by every man but the squad leader.
    #[serde(default)]
    pub count: u8,
    /// Carried by the squad leader (the first man at spawn).
    #[serde(default)]
    pub leader_count: u8,
}

/// Most of one item a man carries.
pub const MAX_ITEM_COUNT: u8 = 10;

fn default_hp() -> u32 {
    100
}

/// What a thrown item does. The sim keeps per-soldier counts by kind, and
/// the presenter's ability buttons (G, T, F) map to kinds.
#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum ItemKind {
    /// Fragmentation grenade: a small HE blast.
    #[default]
    Frag = 0,
    /// Smoke grenade: a cloud that blocks spotting.
    Smoke = 1,
    /// Satchel charge: a big HE blast whose crater cuts trenches.
    Satchel = 2,
}

/// Number of [`ItemKind`]s.
pub const ITEM_KINDS: usize = 3;

impl ItemKind {
    pub const ALL: [ItemKind; ITEM_KINDS] = [ItemKind::Frag, ItemKind::Smoke, ItemKind::Satchel];

    /// Position in [`ItemKind::ALL`] (per-soldier count arrays).
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// A throwable item (`defs/items/*.toml`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    pub kind: ItemKind,
    /// Closest target, in metres.
    #[serde(default)]
    pub min_range_m: u32,
    /// Farthest target, in metres.
    pub range_m: u32,
    /// The thrower stands still this long before the item leaves his hand.
    pub windup_ms: u32,
    /// Fuse from the start of the throw (wind-up included). The item never
    /// goes off before it lands; 0 = on landing.
    #[serde(default)]
    pub fuse_ms: u32,
    /// Scatter of the landing point, in percent of the throw distance.
    pub scatter_pct: u32,
    /// Frag and satchel: the blast.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub he: Option<ItemHeDef>,
    /// Smoke: the cloud.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smoke: Option<SmokeDef>,
}

/// An item's blast (like a weapon's [`HeDef`], plus damage and suppression).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ItemHeDef {
    /// Damage inside the lethal radius.
    pub damage: u32,
    /// Suppression at the blast point (out of 100), falling to zero at the
    /// suppression radius.
    pub suppression: u32,
    pub lethal_radius_cm: u32,
    pub damage_radius_cm: u32,
    pub suppression_radius_m: u32,
    pub crater_radius_cm: u32,
    pub crater_depth_cm: u32,
    /// The crater levels what it hits instead of deepening it: a trench
    /// inside it fills in and a parapet is blown away (satchel charges).
    #[serde(default)]
    pub cuts_trenches: bool,
    /// Armour it defeats on contact, in mm, by the plate rule (S3).
    pub pen_mm: u32,
    /// When it defeats a plate: damage to boxes within `inside_radius_cm`
    /// of the contact point.
    #[serde(default)]
    pub inside_damage: u32,
    #[serde(default)]
    pub inside_radius_cm: u32,
    /// Damage to track anchors within the lethal radius, in percent of the
    /// blast's damage. Blasts never reach a closed vehicle's insides.
    pub vs_vehicle_pct: u32,
}

/// Checks the S3 vehicle fields every HE block has.
fn check_he_armour(out: &mut Vec<Issue>, prefix: &str, v: [u32; 4]) {
    let [pen_mm, inside_damage, inside_radius_cm, vs_vehicle_pct] = v;
    range(out, &format!("{prefix}pen_mm"), pen_mm, 0, 2000);
    range(
        out,
        &format!("{prefix}inside_damage"),
        inside_damage,
        0,
        10_000,
    );
    range(
        out,
        &format!("{prefix}inside_radius_cm"),
        inside_radius_cm,
        0,
        1000,
    );
    range(
        out,
        &format!("{prefix}vs_vehicle_pct"),
        vs_vehicle_pct,
        0,
        100,
    );
}

/// A smoke cloud: a cylinder that grows, holds, then thins out.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SmokeDef {
    /// Full radius (the cloud is as tall as it is wide).
    pub radius_m: u32,
    /// Growth from nothing to the full radius.
    pub grow_ms: u32,
    /// Full density after growing.
    pub hold_ms: u32,
    /// Thinning out: sight lines get through more and more often.
    pub fade_ms: u32,
}

/// Weapon family; decides how the sim fires it.
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WeaponClass {
    Rifle,
    Smg,
    Mg,
    /// Indirect fire at a ground position; needs `[he]`.
    Mortar,
    /// A vehicle's main gun (S3); it fires the rounds its mount carries.
    Cannon,
    /// An infantry anti-tank launcher (S3); it fires `ammo` and targets
    /// only vehicles.
    Launcher,
}

/// A weapon (`defs/weapons/*.toml`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WeaponDef {
    pub id: String,
    pub name: String,
    pub class: WeaponClass,
    /// Shortest range it can engage at, in metres (mortars).
    #[serde(default)]
    pub min_range_m: u32,
    /// Longest range it engages at, in metres.
    pub range_m: u32,
    /// Damage per hit (direct) or at the blast centre (HE).
    pub damage: u32,
    /// Cyclic rate in rounds per minute.
    pub rounds_per_min: u32,
    /// Rounds between reloads.
    pub magazine: u32,
    pub reload_ms: u32,
    /// Dispersion half-angle in milliradians ...
    #[serde(default)]
    pub spread_mrad: u32,
    /// ... or in microradians, for guns finer than 1 mrad (cannons). Set
    /// one of the two.
    #[serde(default)]
    pub spread_urad: u32,
    /// Cannons and launchers: nominal; their rounds fly at the ammo's
    /// `muzzle_velocity_m_s` (a launcher's must match its ammo's).
    pub muzzle_velocity_m_s: u32,
    /// Time after moving before it can fire (MG, mortar).
    #[serde(default)]
    pub setup_ms: u32,
    /// Suppression added to a soldier by one near miss (out of 100).
    pub suppression: u32,
    /// Firing reveals the shooter: sight range against him in percent, for
    /// how long (default: the rules' `firing_spot_pct` and `reveal_ms`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fire_reveal_pct: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fire_reveal_ms: Option<u32>,
    /// Launchers: opens fire on a vehicle standing still within this range,
    /// on a moving one (over 1 m/s) within `engage_moving_m` (default
    /// `range_m`). Applies to every player, human or AI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engage_still_m: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engage_moving_m: Option<u32>,
    /// Launchers: the round it fires (`defs/ammo/`) ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ammo: Option<String>,
    /// ... and how many the team carries (resupplied in base like items).
    #[serde(default)]
    pub rounds_carried: u32,
    /// Launchers: when the gunner dies, the next living man takes the
    /// weapon after this long.
    #[serde(default)]
    pub handover_ms: u32,
    /// High-explosive effect of each round.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub he: Option<HeDef>,
}

/// High-explosive blast.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HeDef {
    /// Full damage inside this radius.
    pub lethal_radius_cm: u32,
    /// Damage falls to zero at this radius.
    pub damage_radius_cm: u32,
    pub suppression_radius_m: u32,
    pub crater_radius_cm: u32,
    pub crater_depth_cm: u32,
    /// Armour it defeats on contact, in mm, by the plate rule (S3).
    pub pen_mm: u32,
    /// When it defeats a plate: damage to boxes within `inside_radius_cm`
    /// of the contact point.
    #[serde(default)]
    pub inside_damage: u32,
    #[serde(default)]
    pub inside_radius_cm: u32,
    /// Damage to track anchors within the lethal radius, in percent of the
    /// blast's damage. Blasts never reach a closed vehicle's insides.
    pub vs_vehicle_pct: u32,
}

/// A playable side (`defs/factions/*.toml`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FactionDef {
    pub id: String,
    pub name: String,
    /// Team colour as `#rrggbb`.
    pub color: String,
    /// Squad ids this faction can call in.
    pub roster: Vec<String>,
    /// Squad ids on the field at tick 0. Must be in `roster`.
    #[serde(default)]
    pub start_squads: Vec<String>,
}

/// A skirmish map (`defs/maps/*.toml`). The heightfield is generated from
/// `[terrain]` by the sim; nothing here is an image.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MapFileDef {
    pub id: String,
    pub name: String,
    /// Side of the square map: a multiple of 64 in 64..=8192.
    pub size_m: u32,
    pub terrain: TerrainDef,
    /// Woods patches: they cut spotting range inside them, not bullets.
    #[serde(default)]
    pub woods: Vec<WoodsDef>,
    pub capture_points: Vec<CapturePointDef>,
    /// One base per player slot, in slot order.
    pub bases: Vec<BaseDef>,
}

/// Heightfield shape. Height = base + value-noise octaves + ridges − valleys.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TerrainDef {
    /// Terrain seed: fixed per map, independent of the match seed.
    pub seed: u64,
    pub base_height_m: u32,
    #[serde(default)]
    pub octaves: Vec<OctaveDef>,
    #[serde(default)]
    pub ridges: Vec<RidgeDef>,
    #[serde(default)]
    pub valleys: Vec<ValleyDef>,
}

/// One octave of smooth value noise.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OctaveDef {
    /// Lattice spacing in metres (a multiple of 4).
    pub period_m: u32,
    /// Peak deviation in centimetres.
    pub amplitude_cm: u32,
}

/// A raised band along a segment, smooth falloff to both sides.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RidgeDef {
    pub from: [u32; 2],
    pub to: [u32; 2],
    pub height_m: u32,
    /// Full width at the foot, in metres.
    pub width_m: u32,
}

/// A lowered band along a segment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValleyDef {
    pub from: [u32; 2],
    pub to: [u32; 2],
    pub depth_m: u32,
    pub width_m: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WoodsDef {
    pub center: [u32; 2],
    pub radius_m: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapturePointDef {
    /// Short label shown in the HUD, e.g. `A`.
    pub name: String,
    pub pos: [u32; 2],
    /// Weight when counting held points for income and ticket drain.
    #[serde(default = "default_point_value")]
    pub value: u32,
}

fn default_point_value() -> u32 {
    1
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BaseDef {
    pub pos: [u32; 2],
    pub radius_m: u32,
}

/// Game-mode rules (`defs/rules/*.toml`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RulesDef {
    pub id: String,
    pub name: String,
    /// 30 (default) or 20.
    #[serde(default = "default_tick_hz")]
    pub tick_hz: u32,
    #[serde(default = "default_hash_interval")]
    pub hash_interval: u32,
    pub tickets: u32,
    /// Soldiers per side, alive or called in.
    pub pop_cap: u32,
    pub projectile_cap: u32,
    pub economy: EconomyDef,
    pub tickets_drain: TicketsDrainDef,
    pub capture: CaptureDef,
    pub movement: MovementDef,
    pub dig: DigDef,
    pub combat: CombatDef,
    pub items: ItemRulesDef,
    /// Vehicles (S3).
    pub vehicles: VehicleRulesDef,
    pub difficulty: DifficultyTableDef,
    pub ai: AiDef,
    /// Where arriving men look for cover (S3b tuning table).
    pub cover: CoverTuning,
}

/// Vehicle rules (S3, `docs/slices/S3-armour.md`). Per-vehicle numbers
/// (speeds, armour, bog chance) are in the vehicle defs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VehicleRulesDef {
    /// Vehicles can be called in from this much match time on (0 = from
    /// the start; the test override).
    pub unlock_s: u32,
    /// Most vehicles per player, alive or queued.
    pub limit: u32,
    /// Most wrecks on the map; beyond it the oldest is removed.
    pub wreck_cap: u32,
    /// A crewman moving into an empty driver or gunner seat takes this
    /// long; both seats count as empty meanwhile.
    pub seat_swap_s: u32,
    /// Crew hit points per seat, and below which a crewman is wounded.
    pub crew_hp: u32,
    pub wounded_hp: u32,
    /// A vehicle faster than this counts as moving (spotting, spread,
    /// launcher fire discipline).
    pub moving_cm_s: u32,
    /// Sight range against a moving vehicle, in percent (the size factor
    /// is the vehicle def's `spot_size_pct`; the highest applies).
    pub moving_spot_pct: u32,
    /// Enemy soldiers this close spot a vehicle without a sight line.
    pub near_spot_m: u32,
    /// Main-gun spread against a moving target, in percent.
    pub moving_target_spread_pct: u32,
    /// Each further shot at the same target is this much tighter ...
    pub ranging_step_pct: u32,
    /// ... down to this much of the first shot's spread.
    pub ranging_floor_pct: u32,
    /// Spread of HE fired at infantry (replaces the gun's AP spread).
    pub he_area_spread_mrad: u32,
    /// Tank HE is never fired within this distance of own men.
    pub he_clear_m: u32,
    /// The gunner aims at the hull unless the turret's penetration chance
    /// is better by more than this many points.
    pub aim_hull_margin_pct: u32,
    /// Track samples over ground dug at least this deep count as crossing a
    /// trench ...
    pub crossing_dug_cm: u32,
    /// ... which caps speed at this ...
    pub crossing_speed_cm_s: u32,
    /// ... and a new crossing (one bog roll) starts after this long with no
    /// sample over dug ground.
    pub crossing_gap_s: u32,
    /// An engine hit for at least this much damage may start a fire ...
    pub fire_min_damage: u32,
    /// ... with this chance. A fire burns the engine ...
    pub fire_start_pct: u32,
    pub fire_burn_hp_s: u32,
    /// ... and every `fire_check_s` a living crew puts it out with
    /// `fire_out_pct`. A fire still burning after `fire_burnout_s` knocks
    /// the vehicle out.
    pub fire_check_s: u32,
    pub fire_out_pct: u32,
    pub fire_burnout_s: u32,
    /// Each hit on the ammo rack for at least this much damage detonates
    /// it with `rack_detonate_pct` (a rack destroyed by damage always
    /// does).
    pub rack_min_damage: u32,
    pub rack_detonate_pct: u32,
    /// Inside the own base: percent of every component's HP back per
    /// second, one main-gun round back every `base_rearm_s`, and one dead
    /// crew seat back every `base_crew_s`.
    pub base_repair_pct_s: u32,
    pub base_rearm_s: u32,
    pub base_crew_s: u32,
    /// Path following, yielding and ground pose (S3b tuning table).
    pub driving: DrivingTuning,
    /// Vehicle path costs and blocked ground (S3b tuning table).
    pub path: VehiclePathTuning,
    /// The infantry file behind a vehicle (S3b tuning table).
    pub escort: EscortTuning,
    /// Vehicle weapons and hits on vehicles (S3b tuning table).
    pub gunnery: GunneryTuning,
}

/// Thrown items (S1b abilities).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ItemRulesDef {
    /// A soldier in his own base gets one item back this often, up to his
    /// loadout.
    pub resupply_s: u32,
    /// Throw flight and smoke reach (S3b tuning table).
    pub throw: ThrowTuning,
}

fn default_tick_hz() -> u32 {
    30
}
fn default_hash_interval() -> u32 {
    30
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EconomyDef {
    pub start_manpower: u32,
    pub income_per_min: u32,
    pub point_income_per_min: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TicketsDrainDef {
    pub per_min_per_point: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaptureDef {
    pub radius_m: u32,
    pub time_s: u32,
    pub max_men: u8,
    /// Each man beyond the first, up to `max_men`, adds this much capture
    /// speed (percent of one man's).
    pub extra_man_pct: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MovementDef {
    pub slope_cost_pct: u32,
    pub max_slope_pct: u32,
    pub arrive_cm: u32,
    pub retreat_speed_pct: u32,
    pub catch_up_speed_pct: u32,
    /// Walking speed of a man ordered prone (crawling), in percent.
    pub prone_speed_pct: u32,
    pub path_budget_per_tick: u32,
    pub path_max_expansions: u32,
    /// Path step cost between two trench cells, in percent of normal.
    pub trench_cost_pct: u32,
    /// Infantry path step cost into an AT-ditch cell, in percent of normal.
    pub ditch_cost_pct: u32,
    /// Squad walking, arrival and soldier movement details (S3b tuning
    /// table).
    pub formation: FormationTuning,
}

/// Dig profiles (S3 generalised the S1 trench to named profiles).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DigDef {
    pub profiles: DigProfilesDef,
    /// Spoil bank shape and digger reach (S3b tuning table).
    pub common: DigCommonTuning,
}

/// Every profile a `Dig` command can name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DigProfilesDef {
    /// The standard trench (S1).
    pub trench: DigProfileDef,
    /// The anti-tank ditch (S3): deep and wide enough to block vehicles.
    pub at_ditch: DigProfileDef,
}

/// A dug cross-section: a flat floor, walls sloping out to the lip, and a
/// spoil bank (parapet or berm) on the enemy side.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DigProfileDef {
    pub depth_cm: u32,
    pub floor_cm: u32,
    /// Horizontal run of each wall, floor edge to lip.
    pub wall_cm: u32,
    /// Height of the spoil bank.
    pub parapet_cm: u32,
    pub max_length_m: u32,
    pub work_per_m: u32,
    /// It deepens in this many equal passes.
    pub passes: u32,
    /// Only engineer squads dig it.
    #[serde(default)]
    pub engineers_only: bool,
}

/// Spotting, accuracy, suppression and corpse rules. Suppression is in
/// points out of 100 (a weapon's `suppression` is added per near miss).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CombatDef {
    /// Spotting LOS rays per player per tick.
    pub spot_rays_per_tick: u32,
    /// Each enemy soldier is re-checked this often while the ray budget lasts.
    pub spot_period_ms: u32,
    /// Most observers tried per target (rays per target check).
    pub spot_observers: u32,
    /// Sight range against a prone target, in percent.
    pub prone_spot_pct: u32,
    /// Sight range against a target in a trench, in percent.
    pub trench_spot_pct: u32,
    /// Sight range against a target in woods, in percent.
    pub woods_spot_pct: u32,
    /// Sight range against a soldier who fired recently (replaces the
    /// prone, trench and woods factors), in percent.
    pub firing_spot_pct: u32,
    /// How long a shot gives the shooter away.
    pub reveal_ms: u32,
    /// A round passing this close suppresses.
    pub near_miss_cm: u32,
    /// Suppression added by being hit.
    pub hit_suppression: u32,
    pub suppression_decay_per_s: u32,
    /// At this suppression a soldier crouches ...
    pub crouch_suppression: u32,
    /// ... and at this one he is pinned (prone, does not move).
    pub pin_suppression: u32,
    /// Weapon spread while the shooter moves, in percent.
    pub moving_spread_pct: u32,
    /// Weapon spread while pinned, in percent.
    pub pinned_spread_pct: u32,
    /// Extra spread at the weapon's full range, in percent (linear in range).
    pub range_spread_pct: u32,
    /// A soldier firing from a trench shows this much above the parapet.
    pub fire_exposure_cm: u32,
    /// Corpses stay this long (for the presenter).
    pub corpse_s: u32,
    /// Hit capsule and posture points (S3b tuning table).
    pub body: BodyTuning,
    /// Mortar laying and flight (S3b tuning table).
    pub mortar: MortarTuning,
    /// HE bursts (S3b tuning table).
    pub blast: BlastTuning,
    /// Suppression details (S3b tuning table).
    pub suppression: SuppressionTuning,
}

/// AI commander tuning (weights in data, §9). Difficulty is separate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AiDef {
    /// Squads below this strength (percent of full men) fall back to base
    /// while enemies are near.
    pub retreat_strength_pct: u32,
    /// Spotted enemies this close to a point or squad count as a threat.
    pub threat_radius_m: u32,
    /// Target army mix by squad role, in percent of squads.
    pub mix_rifle_pct: u32,
    pub mix_mg_pct: u32,
    pub mix_engineer_pct: u32,
    pub mix_mortar_pct: u32,
    /// Rounds per mortar Barrage order.
    pub barrage_rounds: u32,
    /// Trench length engineers dig at a held point ...
    pub dig_length_m: u32,
    /// ... this far in front of it, toward the nearest enemy.
    pub dig_offset_m: u32,
    /// A squad throws at most one item this often.
    pub throw_cooldown_s: u32,
    /// Frags go at enemies in a trench, or at a group of at least this many
    /// spotted men within 5 m of each other.
    pub frag_group_men: u32,
    /// S3 mix: vehicles after unlock (under the vehicle limit), and AT
    /// teams after unlock once an enemy vehicle has been seen, in percent
    /// of squads.
    pub mix_vehicle_pct: u32,
    pub mix_at_pct: u32,
    /// Tanks hold this far from a point where enemy AT was seen ...
    pub tank_standoff_m: u32,
    /// ... else advance with their escort to this far from the enemy
    /// trench, where the escort attacks.
    pub assault_close_m: u32,
    /// Enemy AT counts as gone from a point after this long unseen.
    pub at_clear_s: u32,
    /// An immobilised vehicle repairs when no seen enemy is this close.
    pub repair_clear_m: u32,
    /// Infantry, mortar and satchel rules (S3b tuning table).
    pub infantry: AiInfantryTuning,
    /// Vehicles and escorts (S3b tuning table).
    pub armour: AiArmourTuning,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DifficultyTableDef {
    pub easy: DifficultyDef,
    pub normal: DifficultyDef,
    pub hard: DifficultyDef,
}

/// How an AI player at one difficulty differs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DifficultyDef {
    pub income_pct: u32,
    pub reaction_ms: u32,
    pub replan_ms: u32,
    pub decision_quality_pct: u32,
}

// ------------------------------------------------------------ validation --

/// Pushes an issue when `v` is outside `lo..=hi`.
fn range(out: &mut Vec<Issue>, path: &str, v: u32, lo: u32, hi: u32) {
    if v < lo || v > hi {
        out.push(Issue::new(path, format!("must be in {lo}..={hi}, got {v}")));
    }
}

/// Namespaced id check (`mod:name`, lower-case ASCII, digits and `_`).
pub(crate) fn check_id(out: &mut Vec<Issue>, path: &str, id: &str) {
    let ok = id.split_once(':').is_some_and(|(ns, name)| {
        let part = |s: &str| {
            !s.is_empty()
                && s.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        };
        part(ns) && part(name)
    });
    if !ok {
        out.push(
            Issue::new(path, format!("`{id}` is not a namespaced id"))
                .with_hint("id = \"mymod:my_thing\" (lower-case letters, digits, `_`)"),
        );
    }
}

fn check_name(out: &mut Vec<Issue>, path: &str, name: &str) {
    if name.trim().is_empty() {
        out.push(Issue::new(path, "must not be empty"));
    }
}

impl SquadDef {
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        check_id(&mut out, "id", &self.id);
        check_name(&mut out, "name", &self.name);
        if self.role == SquadRole::Vehicle {
            // The vehicle def holds everything else.
            if self.vehicle.is_none() {
                out.push(
                    Issue::new("vehicle", "a `vehicle` squad names its vehicle def")
                        .with_hint("vehicle = \"mymod:my_tank\""),
                );
            }
            let soldier_fields = [
                ("men", self.men != 0),
                ("weapon", !self.weapon.is_empty()),
                ("weapon_men", self.weapon_men.is_some()),
                ("sidearm", self.sidearm.is_some()),
                ("cost", self.cost != 0),
                ("call_in_s", self.call_in_s != 0),
                ("hp", self.hp != default_hp()),
                ("speed_cm_s", self.speed_cm_s != 0),
                ("sight_m", self.sight_m != 0),
                ("spacing_dm", self.spacing_dm != 0),
                ("dig_rate_pct", self.dig_rate_pct != 0),
                ("loadout", !self.loadout.is_empty()),
            ];
            for (key, set) in soldier_fields {
                if set {
                    out.push(
                        Issue::new(key, "not allowed on a vehicle squad")
                            .with_hint("set it in the vehicle def, or remove it"),
                    );
                }
            }
            return out;
        }
        if self.vehicle.is_some() {
            out.push(Issue::new(
                "vehicle",
                "only squads with role = \"vehicle\" name a vehicle",
            ));
        }
        if self.men == 0 {
            out.push(Issue::new("men", "required: soldiers in the squad, 1..=16"));
        } else {
            range(&mut out, "men", self.men as u32, 1, MAX_SQUAD_MEN as u32);
        }
        if self.weapon.is_empty() {
            out.push(Issue::new(
                "weapon",
                "required: the weapon id the men carry",
            ));
        }
        if let Some(n) = self.weapon_men {
            range(&mut out, "weapon_men", n as u32, 1, self.men.max(1) as u32);
            if n < self.men && self.sidearm.is_none() {
                out.push(
                    Issue::new(
                        "sidearm",
                        format!(
                            "{} men carry no `weapon`; they need a sidearm",
                            self.men - n
                        ),
                    )
                    .with_hint("sidearm = \"ww2:kar98k\""),
                );
            }
        }
        if self.sidearm.is_some() && self.weapon_men.is_none_or(|n| n >= self.men) {
            out.push(Issue::new(
                "sidearm",
                "every man carries `weapon`; set weapon_men below men or remove it",
            ));
        }
        range(&mut out, "call_in_s", self.call_in_s, 0, 600);
        range(&mut out, "hp", self.hp, 1, 10_000);
        range(&mut out, "speed_cm_s", self.speed_cm_s, 10, 1000);
        range(&mut out, "sight_m", self.sight_m, 1, 2000);
        range(&mut out, "spacing_dm", self.spacing_dm, 5, 200);
        range(&mut out, "dig_rate_pct", self.dig_rate_pct, 0, 1000);
        range(&mut out, "cost", self.cost, 0, 100_000);
        for (i, l) in self.loadout.iter().enumerate() {
            let max = MAX_ITEM_COUNT as u32;
            range(
                &mut out,
                &format!("loadout[{i}].count"),
                l.count as u32,
                0,
                max,
            );
            range(
                &mut out,
                &format!("loadout[{i}].leader_count"),
                l.leader_count as u32,
                0,
                max,
            );
            if self.loadout[..i].iter().any(|o| o.item == l.item) {
                out.push(Issue::new(
                    format!("loadout[{i}].item"),
                    format!("`{}` is listed twice", l.item),
                ));
            }
        }
        out
    }
}

impl ItemDef {
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        check_id(&mut out, "id", &self.id);
        check_name(&mut out, "name", &self.name);
        range(&mut out, "range_m", self.range_m, 1, 100);
        if self.min_range_m >= self.range_m {
            out.push(Issue::new(
                "min_range_m",
                format!(
                    "must be below range_m ({}), got {}",
                    self.range_m, self.min_range_m
                ),
            ));
        }
        range(&mut out, "windup_ms", self.windup_ms, 0, 10_000);
        range(&mut out, "fuse_ms", self.fuse_ms, 0, 30_000);
        range(&mut out, "scatter_pct", self.scatter_pct, 0, 100);
        let he = matches!(self.kind, ItemKind::Frag | ItemKind::Satchel);
        if he != self.he.is_some() {
            out.push(
                Issue::new(
                    "he",
                    "frag and satchel items need an [he] table, smoke must not",
                )
                .with_hint("[he] damage = 150 ..."),
            );
        }
        if (self.kind == ItemKind::Smoke) != self.smoke.is_some() {
            out.push(
                Issue::new("smoke", "smoke items need a [smoke] table, others must not")
                    .with_hint("[smoke] radius_m = 8 ..."),
            );
        }
        if let Some(h) = &self.he {
            for i in h.validate() {
                out.push(Issue {
                    path: format!("he.{}", i.path),
                    ..i
                });
            }
        }
        if let Some(s) = &self.smoke {
            range(&mut out, "smoke.radius_m", s.radius_m, 1, 50);
            range(&mut out, "smoke.grow_ms", s.grow_ms, 0, 60_000);
            range(&mut out, "smoke.hold_ms", s.hold_ms, 0, 300_000);
            range(&mut out, "smoke.fade_ms", s.fade_ms, 0, 60_000);
        }
        out
    }
}

impl ItemHeDef {
    /// Checks one blast; paths are relative to the `[he]` table.
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        range(&mut out, "damage", self.damage, 0, 10_000);
        range(&mut out, "suppression", self.suppression, 0, 100);
        range(&mut out, "lethal_radius_cm", self.lethal_radius_cm, 1, 5000);
        range(
            &mut out,
            "damage_radius_cm",
            self.damage_radius_cm,
            1,
            10_000,
        );
        if self.damage_radius_cm < self.lethal_radius_cm {
            out.push(Issue::new(
                "damage_radius_cm",
                "must be at least lethal_radius_cm",
            ));
        }
        range(
            &mut out,
            "suppression_radius_m",
            self.suppression_radius_m,
            1,
            200,
        );
        range(&mut out, "crater_radius_cm", self.crater_radius_cm, 0, 2000);
        range(&mut out, "crater_depth_cm", self.crater_depth_cm, 0, 500);
        check_he_armour(
            &mut out,
            "",
            [
                self.pen_mm,
                self.inside_damage,
                self.inside_radius_cm,
                self.vs_vehicle_pct,
            ],
        );
        out
    }
}

impl WeaponDef {
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        check_id(&mut out, "id", &self.id);
        check_name(&mut out, "name", &self.name);
        range(&mut out, "range_m", self.range_m, 1, 5000);
        if self.min_range_m >= self.range_m {
            out.push(Issue::new(
                "min_range_m",
                format!(
                    "must be below range_m ({}), got {}",
                    self.range_m, self.min_range_m
                ),
            ));
        }
        range(&mut out, "damage", self.damage, 0, 10_000);
        range(&mut out, "rounds_per_min", self.rounds_per_min, 1, 1800);
        range(&mut out, "magazine", self.magazine, 1, 1000);
        range(&mut out, "reload_ms", self.reload_ms, 0, 60_000);
        range(&mut out, "spread_mrad", self.spread_mrad, 0, 500);
        range(&mut out, "spread_urad", self.spread_urad, 0, 500_000);
        if self.spread_mrad != 0 && self.spread_urad != 0 {
            out.push(
                Issue::new("spread_urad", "set spread_mrad or spread_urad, not both")
                    .with_hint("spread_urad = 2500 for 2.5 mrad"),
            );
        }
        if let Some(v) = self.fire_reveal_pct {
            range(&mut out, "fire_reveal_pct", v, 100, 1000);
        }
        if let Some(v) = self.fire_reveal_ms {
            range(&mut out, "fire_reveal_ms", v, 0, 60_000);
        }
        let launcher = self.class == WeaponClass::Launcher;
        for (key, v) in [
            ("engage_still_m", self.engage_still_m),
            ("engage_moving_m", self.engage_moving_m),
        ] {
            match v {
                Some(_) if !launcher => out.push(Issue::new(key, "only launchers have it")),
                Some(m) => range(&mut out, key, m, 1, self.range_m),
                None => {}
            }
        }
        match (&self.ammo, launcher) {
            (None, true) => out.push(
                Issue::new("ammo", "a launcher names the round it fires")
                    .with_hint("ammo = \"ww2:my_rocket\""),
            ),
            (Some(_), false) => out.push(Issue::new(
                "ammo",
                "only launchers name a round; a cannon's rounds are on its vehicle mount",
            )),
            _ => {}
        }
        if launcher {
            range(&mut out, "rounds_carried", self.rounds_carried, 1, 50);
            range(&mut out, "handover_ms", self.handover_ms, 0, 60_000);
        } else if self.rounds_carried != 0 || self.handover_ms != 0 {
            out.push(Issue::new(
                "rounds_carried",
                "only launchers have rounds_carried and handover_ms",
            ));
        }
        if self.class == WeaponClass::Cannon && self.he.is_some() {
            out.push(Issue::new(
                "he",
                "a cannon's HE is a round on its vehicle mount (defs/ammo), not [he]",
            ));
        }
        range(
            &mut out,
            "muzzle_velocity_m_s",
            self.muzzle_velocity_m_s,
            1,
            3000,
        );
        range(&mut out, "setup_ms", self.setup_ms, 0, 60_000);
        range(&mut out, "suppression", self.suppression, 0, 100);
        match (&self.he, self.class) {
            (None, WeaponClass::Mortar) => out.push(
                Issue::new("he", "a mortar needs an [he] table")
                    .with_hint("[he] lethal_radius_cm = 300 ..."),
            ),
            (Some(he), _) => {
                check_he_armour(
                    &mut out,
                    "he.",
                    [
                        he.pen_mm,
                        he.inside_damage,
                        he.inside_radius_cm,
                        he.vs_vehicle_pct,
                    ],
                );
                range(
                    &mut out,
                    "he.lethal_radius_cm",
                    he.lethal_radius_cm,
                    1,
                    5000,
                );
                if he.damage_radius_cm < he.lethal_radius_cm {
                    out.push(Issue::new(
                        "he.damage_radius_cm",
                        "must be at least lethal_radius_cm",
                    ));
                }
                range(
                    &mut out,
                    "he.damage_radius_cm",
                    he.damage_radius_cm,
                    1,
                    10_000,
                );
                range(
                    &mut out,
                    "he.suppression_radius_m",
                    he.suppression_radius_m,
                    1,
                    200,
                );
                range(
                    &mut out,
                    "he.crater_radius_cm",
                    he.crater_radius_cm,
                    0,
                    2000,
                );
                range(&mut out, "he.crater_depth_cm", he.crater_depth_cm, 0, 500);
            }
            (None, _) => {}
        }
        out
    }
}

/// Parses `#rrggbb` into `0xRRGGBB`.
pub fn parse_color(s: &str) -> Option<u32> {
    let hex = s.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
}

impl FactionDef {
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        check_id(&mut out, "id", &self.id);
        check_name(&mut out, "name", &self.name);
        if parse_color(&self.color).is_none() {
            out.push(
                Issue::new("color", format!("`{}` is not a #rrggbb colour", self.color))
                    .with_hint("color = \"#7d8b4a\""),
            );
        }
        if self.roster.is_empty() {
            out.push(Issue::new("roster", "must list at least one squad"));
        }
        for (i, s) in self.start_squads.iter().enumerate() {
            if !self.roster.contains(s) {
                out.push(
                    Issue::new(
                        format!("start_squads[{i}]"),
                        format!("`{s}` is not in the roster"),
                    )
                    .with_hint("add it to `roster` or remove it here"),
                );
            }
        }
        out
    }
}

impl MapFileDef {
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        check_id(&mut out, "id", &self.id);
        check_name(&mut out, "name", &self.name);
        let size = self.size_m;
        if !(MAP_SIZE_STEP_M..=MAP_SIZE_MAX_M).contains(&size)
            || !size.is_multiple_of(MAP_SIZE_STEP_M)
        {
            out.push(Issue::new(
                "size_m",
                format!("must be a multiple of 64 in 64..=8192, got {size}"),
            ));
        }
        let inside = |out: &mut Vec<Issue>, path: String, p: [u32; 2]| {
            if p[0] > size || p[1] > size {
                out.push(Issue::new(
                    path,
                    format!("[{}, {}] is outside the {size} m map", p[0], p[1]),
                ));
            }
        };
        let t = &self.terrain;
        range(&mut out, "terrain.base_height_m", t.base_height_m, 0, 1000);
        for (i, o) in t.octaves.iter().enumerate() {
            let p = format!("terrain.octaves[{i}]");
            if o.period_m < 8 || o.period_m > size.max(8) || !o.period_m.is_multiple_of(4) {
                out.push(Issue::new(
                    format!("{p}.period_m"),
                    format!("must be a multiple of 4 in 8..=size_m, got {}", o.period_m),
                ));
            }
            range(
                &mut out,
                &format!("{p}.amplitude_cm"),
                o.amplitude_cm,
                0,
                10_000,
            );
        }
        for (i, r) in t.ridges.iter().enumerate() {
            let p = format!("terrain.ridges[{i}]");
            inside(&mut out, format!("{p}.from"), r.from);
            inside(&mut out, format!("{p}.to"), r.to);
            range(&mut out, &format!("{p}.height_m"), r.height_m, 0, 200);
            range(&mut out, &format!("{p}.width_m"), r.width_m, 8, 4000);
        }
        for (i, v) in t.valleys.iter().enumerate() {
            let p = format!("terrain.valleys[{i}]");
            inside(&mut out, format!("{p}.from"), v.from);
            inside(&mut out, format!("{p}.to"), v.to);
            range(&mut out, &format!("{p}.depth_m"), v.depth_m, 0, 200);
            range(&mut out, &format!("{p}.width_m"), v.width_m, 8, 4000);
        }
        for (i, w) in self.woods.iter().enumerate() {
            inside(&mut out, format!("woods[{i}].center"), w.center);
            range(
                &mut out,
                &format!("woods[{i}].radius_m"),
                w.radius_m,
                1,
                1000,
            );
        }
        if self.capture_points.is_empty() {
            out.push(Issue::new("capture_points", "need at least one point"));
        }
        if self.capture_points.len() > crate::MAX_CAPTURE_POINTS {
            out.push(Issue::new("capture_points", "too many points"));
        }
        for (i, c) in self.capture_points.iter().enumerate() {
            check_name(&mut out, &format!("capture_points[{i}].name"), &c.name);
            inside(&mut out, format!("capture_points[{i}].pos"), c.pos);
            range(
                &mut out,
                &format!("capture_points[{i}].value"),
                c.value,
                1,
                100,
            );
        }
        if self.bases.len() < 2 {
            out.push(Issue::new(
                "bases",
                "need at least two bases (one per player)",
            ));
        }
        for (i, b) in self.bases.iter().enumerate() {
            inside(&mut out, format!("bases[{i}].pos"), b.pos);
            range(
                &mut out,
                &format!("bases[{i}].radius_m"),
                b.radius_m,
                4,
                500,
            );
        }
        out
    }
}

impl RulesDef {
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        check_id(&mut out, "id", &self.id);
        check_name(&mut out, "name", &self.name);
        if !crate::TICK_HZ_ALLOWED.contains(&self.tick_hz) {
            out.push(
                Issue::new("tick_hz", format!("must be 30 or 20, got {}", self.tick_hz))
                    .with_hint("tick_hz = 30"),
            );
        }
        range(&mut out, "hash_interval", self.hash_interval, 1, 10_000);
        range(&mut out, "tickets", self.tickets, 1, 1_000_000);
        range(&mut out, "pop_cap", self.pop_cap, 1, 10_000);
        range(&mut out, "projectile_cap", self.projectile_cap, 1, 1 << 20);
        range(
            &mut out,
            "economy.start_manpower",
            self.economy.start_manpower,
            0,
            1_000_000,
        );
        range(
            &mut out,
            "economy.income_per_min",
            self.economy.income_per_min,
            0,
            1_000_000,
        );
        range(
            &mut out,
            "economy.point_income_per_min",
            self.economy.point_income_per_min,
            0,
            1_000_000,
        );
        range(
            &mut out,
            "tickets_drain.per_min_per_point",
            self.tickets_drain.per_min_per_point,
            0,
            100_000,
        );
        range(&mut out, "capture.radius_m", self.capture.radius_m, 1, 100);
        range(&mut out, "capture.time_s", self.capture.time_s, 1, 600);
        range(
            &mut out,
            "capture.max_men",
            self.capture.max_men as u32,
            1,
            64,
        );
        let m = &self.movement;
        range(
            &mut out,
            "movement.slope_cost_pct",
            m.slope_cost_pct,
            0,
            10_000,
        );
        range(&mut out, "movement.max_slope_pct", m.max_slope_pct, 1, 1000);
        range(&mut out, "movement.arrive_cm", m.arrive_cm, 5, 500);
        range(
            &mut out,
            "movement.retreat_speed_pct",
            m.retreat_speed_pct,
            10,
            500,
        );
        range(
            &mut out,
            "movement.catch_up_speed_pct",
            m.catch_up_speed_pct,
            100,
            300,
        );
        range(
            &mut out,
            "movement.prone_speed_pct",
            m.prone_speed_pct,
            10,
            100,
        );
        range(
            &mut out,
            "movement.path_budget_per_tick",
            m.path_budget_per_tick,
            64,
            1_000_000,
        );
        range(
            &mut out,
            "movement.path_max_expansions",
            m.path_max_expansions,
            64,
            1_000_000,
        );
        range(
            &mut out,
            "movement.trench_cost_pct",
            m.trench_cost_pct,
            10,
            100,
        );
        for (name, d) in [
            ("trench", &self.dig.profiles.trench),
            ("at_ditch", &self.dig.profiles.at_ditch),
        ] {
            let p = format!("dig.profiles.{name}");
            range(&mut out, &format!("{p}.depth_cm"), d.depth_cm, 10, 1000);
            range(&mut out, &format!("{p}.floor_cm"), d.floor_cm, 10, 1000);
            range(&mut out, &format!("{p}.wall_cm"), d.wall_cm, 10, 500);
            range(&mut out, &format!("{p}.parapet_cm"), d.parapet_cm, 0, 300);
            range(
                &mut out,
                &format!("{p}.max_length_m"),
                d.max_length_m,
                1,
                1000,
            );
            range(
                &mut out,
                &format!("{p}.work_per_m"),
                d.work_per_m,
                1,
                10_000_000,
            );
            range(&mut out, &format!("{p}.passes"), d.passes, 1, 32);
        }
        let v = &self.vehicles;
        for (name, val, lo, hi) in [
            ("unlock_s", v.unlock_s, 0, 3600),
            ("limit", v.limit, 0, 50),
            ("wreck_cap", v.wreck_cap, 1, 256),
            ("seat_swap_s", v.seat_swap_s, 0, 60),
            ("crew_hp", v.crew_hp, 1, 1000),
            ("wounded_hp", v.wounded_hp, 0, v.crew_hp),
            ("moving_cm_s", v.moving_cm_s, 1, 1000),
            ("moving_spot_pct", v.moving_spot_pct, 1, 1000),
            ("near_spot_m", v.near_spot_m, 0, 200),
            (
                "moving_target_spread_pct",
                v.moving_target_spread_pct,
                100,
                1000,
            ),
            ("ranging_step_pct", v.ranging_step_pct, 0, 100),
            ("ranging_floor_pct", v.ranging_floor_pct, 1, 100),
            ("he_area_spread_mrad", v.he_area_spread_mrad, 0, 100),
            ("he_clear_m", v.he_clear_m, 0, 500),
            ("aim_hull_margin_pct", v.aim_hull_margin_pct, 0, 100),
            ("crossing_dug_cm", v.crossing_dug_cm, 10, 1000),
            ("crossing_speed_cm_s", v.crossing_speed_cm_s, 10, 3000),
            ("crossing_gap_s", v.crossing_gap_s, 0, 60),
            ("fire_min_damage", v.fire_min_damage, 0, 1000),
            ("fire_start_pct", v.fire_start_pct, 0, 100),
            ("fire_burn_hp_s", v.fire_burn_hp_s, 0, 100),
            ("fire_check_s", v.fire_check_s, 1, 60),
            ("fire_out_pct", v.fire_out_pct, 0, 100),
            ("fire_burnout_s", v.fire_burnout_s, 1, 600),
            ("rack_min_damage", v.rack_min_damage, 0, 1000),
            ("rack_detonate_pct", v.rack_detonate_pct, 0, 100),
            ("base_repair_pct_s", v.base_repair_pct_s, 0, 100),
            ("base_rearm_s", v.base_rearm_s, 1, 600),
            ("base_crew_s", v.base_crew_s, 1, 600),
        ] {
            range(&mut out, &format!("vehicles.{name}"), val, lo, hi);
        }
        let c = &self.combat;
        for (name, v, lo, hi) in [
            ("spot_rays_per_tick", c.spot_rays_per_tick, 1, 100_000),
            ("spot_period_ms", c.spot_period_ms, 1, 10_000),
            ("spot_observers", c.spot_observers, 1, 16),
            ("prone_spot_pct", c.prone_spot_pct, 1, 1000),
            ("trench_spot_pct", c.trench_spot_pct, 1, 1000),
            ("woods_spot_pct", c.woods_spot_pct, 1, 1000),
            ("firing_spot_pct", c.firing_spot_pct, 1, 1000),
            ("reveal_ms", c.reveal_ms, 0, 60_000),
            ("near_miss_cm", c.near_miss_cm, 0, 2000),
            ("hit_suppression", c.hit_suppression, 0, 100),
            ("suppression_decay_per_s", c.suppression_decay_per_s, 0, 100),
            ("crouch_suppression", c.crouch_suppression, 1, 100),
            ("pin_suppression", c.pin_suppression, 1, 100),
            ("moving_spread_pct", c.moving_spread_pct, 1, 1000),
            ("pinned_spread_pct", c.pinned_spread_pct, 1, 1000),
            ("range_spread_pct", c.range_spread_pct, 0, 1000),
            ("fire_exposure_cm", c.fire_exposure_cm, 0, 200),
            ("corpse_s", c.corpse_s, 0, 600),
        ] {
            range(&mut out, &format!("combat.{name}"), v, lo, hi);
        }
        if c.pin_suppression < c.crouch_suppression {
            out.push(Issue::new(
                "combat.pin_suppression",
                "must be at least crouch_suppression",
            ));
        }
        range(&mut out, "items.resupply_s", self.items.resupply_s, 1, 600);
        if c.spot_rays_per_tick < c.spot_observers {
            out.push(Issue::new(
                "combat.spot_rays_per_tick",
                "must be at least spot_observers",
            ));
        }
        for (name, t) in [
            ("easy", &self.difficulty.easy),
            ("normal", &self.difficulty.normal),
            ("hard", &self.difficulty.hard),
        ] {
            let p = format!("difficulty.{name}");
            range(&mut out, &format!("{p}.income_pct"), t.income_pct, 0, 1000);
            range(
                &mut out,
                &format!("{p}.reaction_ms"),
                t.reaction_ms,
                0,
                60_000,
            );
            range(
                &mut out,
                &format!("{p}.replan_ms"),
                t.replan_ms,
                100,
                60_000,
            );
            range(
                &mut out,
                &format!("{p}.decision_quality_pct"),
                t.decision_quality_pct,
                0,
                100,
            );
        }
        let a = &self.ai;
        for (name, v, lo, hi) in [
            ("retreat_strength_pct", a.retreat_strength_pct, 0, 100),
            ("threat_radius_m", a.threat_radius_m, 1, 500),
            ("mix_rifle_pct", a.mix_rifle_pct, 0, 100),
            ("mix_mg_pct", a.mix_mg_pct, 0, 100),
            ("mix_engineer_pct", a.mix_engineer_pct, 0, 100),
            ("mix_mortar_pct", a.mix_mortar_pct, 0, 100),
            ("barrage_rounds", a.barrage_rounds, 1, 50),
            ("dig_length_m", a.dig_length_m, 2, 1000),
            ("dig_offset_m", a.dig_offset_m, 0, 100),
            ("throw_cooldown_s", a.throw_cooldown_s, 1, 600),
            ("frag_group_men", a.frag_group_men, 1, 16),
            ("mix_vehicle_pct", a.mix_vehicle_pct, 0, 100),
            ("mix_at_pct", a.mix_at_pct, 0, 100),
            ("tank_standoff_m", a.tank_standoff_m, 1, 2000),
            ("assault_close_m", a.assault_close_m, 1, 500),
            ("at_clear_s", a.at_clear_s, 1, 600),
            ("repair_clear_m", a.repair_clear_m, 1, 2000),
        ] {
            range(&mut out, &format!("ai.{name}"), v, lo, hi);
        }
        if a.mix_rifle_pct + a.mix_mg_pct + a.mix_engineer_pct + a.mix_mortar_pct == 0 {
            out.push(Issue::new(
                "ai.mix_rifle_pct",
                "the mix must not be all zero",
            ));
        }
        if a.dig_length_m > self.dig.profiles.trench.max_length_m {
            out.push(Issue::new(
                "ai.dig_length_m",
                "must be at most dig.profiles.trench.max_length_m",
            ));
        }
        range(
            &mut out,
            "capture.extra_man_pct",
            self.capture.extra_man_pct,
            0,
            1000,
        );
        range(
            &mut out,
            "movement.ditch_cost_pct",
            self.movement.ditch_cost_pct,
            1,
            10_000,
        );
        check_tuning(
            &mut out,
            "",
            TuningRefs {
                body: &c.body,
                mortar: &c.mortar,
                blast: &c.blast,
                suppression: &c.suppression,
                ai_infantry: &a.infantry,
                ai_armour: &a.armour,
                driving: &v.driving,
                vehicle_path: &v.path,
                escort: &v.escort,
                gunnery: &v.gunnery,
                formation: &self.movement.formation,
                cover: &self.cover,
                throw: &self.items.throw,
                dig_common: &self.dig.common,
            },
        );
        out
    }
}
