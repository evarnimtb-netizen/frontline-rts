//! [`MatchSetup`]: everything the sim needs to start a skirmish, resolved
//! from a mod's defs and lowered to fixed point.
//!
//! The setup is plain data. The sim stores it in its saved state and hashes
//! its postcard bytes, so two clients with different defs on disk can never
//! run the "same" match. Lowering rules:
//!
//! * distances and speeds become [`Fixed`] (Q16.16 raw `i32`, read by the sim
//!   as `Fx::from_raw`), via integer `num / den` truncated toward zero;
//! * durations become whole ticks at the rules' tick rate, rounding half up;
//! * def ids become indices into the sorted `weapons`, `items`,
//!   `squad_types`, `factions`, `ammo` and `vehicles` lists (sorted by id,
//!   so indices are stable for a def set);
//! * angles stay integer degrees (the content crate has no trigonometry;
//!   the sim builds its armour tables from them, S3).
//!
//! [`MatchSetup::validate`] re-checks the structure without the defs, since a
//! setup also arrives inside untrusted save files.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::defs::{
    DigProfileDef, ITEM_KINDS, ItemHeDef, ItemKind, MAX_ITEM_COUNT, SquadRole, WeaponClass,
    parse_color,
};
use crate::issue::{ContentError, Issue};
use crate::modset::ModDefs;
use crate::scenario::ms_to_ticks;
use crate::tuning::{
    AiArmourTuning, AiAssaultTuning, AiInfantryTuning, BailTuning, BlastTuning, BodyTuning,
    CallInTuning, CoverTuning, DigCommonTuning, DrivingTuning, EscortTuning, FormationTuning,
    GunneryTuning, MortarTuning, RecrewTuning, SuppressTuning, SuppressionTuning, ThrowTuning,
    TuningRefs, VehicleDamageTuning, VehiclePathTuning, check_tuning,
};
use crate::vehicle::{
    AmmoSetup, VehicleSetup, check_ammo, check_vehicle, lower_ammo, lower_vehicle,
};

/// Q16.16 fixed-point value as its raw `i32` (1.0 = 65536).
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Fixed(pub i32);

impl Fixed {
    pub const ZERO: Fixed = Fixed(0);
    pub const ONE: Fixed = Fixed(1 << 16);

    /// `num / den`, truncated toward zero and saturated (same rule as the
    /// sim's `Fx::from_ratio`).
    pub const fn from_ratio(num: i64, den: i64) -> Fixed {
        if den == 0 {
            return Fixed(0);
        }
        let q = ((num as i128) << 16) / den as i128;
        Fixed(if q > i32::MAX as i128 {
            i32::MAX
        } else if q < i32::MIN as i128 {
            i32::MIN
        } else {
            q as i32
        })
    }

    pub const fn from_int(v: i32) -> Fixed {
        Fixed::from_ratio(v as i64, 1)
    }

    /// Metres from centimetres.
    pub const fn from_cm(cm: u32) -> Fixed {
        Fixed::from_ratio(cm as i64, 100)
    }

    pub const fn raw(self) -> i32 {
        self.0
    }

    /// Integer part (floor).
    pub const fn floor_int(self) -> i32 {
        self.0 >> 16
    }
}

/// A point on the map in metres (x east, y north).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FixedVec2 {
    pub x: Fixed,
    pub y: Fixed,
}

impl FixedVec2 {
    pub const fn from_m(p: [u32; 2]) -> FixedVec2 {
        FixedVec2 {
            x: Fixed::from_int(p[0] as i32),
            y: Fixed::from_int(p[1] as i32),
        }
    }
}

/// AI strength.
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    Easy,
    #[default]
    Normal,
    Hard,
}

/// One player's choices for a match.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlayerRequest {
    /// Faction id, e.g. `ww2:germany`.
    pub faction: String,
    /// Controlled by the sim-side AI commander.
    #[serde(default)]
    pub ai: bool,
    #[serde(default)]
    pub difficulty: Difficulty,
}

/// What the lobby (menu, CLI) asks for. Player `i` starts at the map's
/// base `i`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MatchRequest {
    /// Rules id, e.g. `ww2:skirmish`.
    pub rules: String,
    /// Map id, e.g. `ww2:kessel_ridge`.
    pub map: String,
    pub seed: u64,
    pub players: Vec<PlayerRequest>,
}

impl MatchRequest {
    /// The S1 default: Kessel Ridge, Germany (human) vs USA (AI).
    pub fn skirmish(seed: u64, human: &str, ai: &str, difficulty: Difficulty) -> MatchRequest {
        MatchRequest {
            rules: "ww2:skirmish".into(),
            map: "ww2:kessel_ridge".into(),
            seed,
            players: vec![
                PlayerRequest {
                    faction: human.into(),
                    ai: false,
                    difficulty,
                },
                PlayerRequest {
                    faction: ai.into(),
                    ai: true,
                    difficulty,
                },
            ],
        }
    }
}

// ------------------------------------------------------------ the setup --

/// A resolved, fixed-point match setup. See the module docs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchSetup {
    /// Mod namespace the defs came from (`ww2`).
    pub mod_id: String,
    pub rules_id: String,
    /// World seed (RNG streams). The terrain uses `map.terrain.seed`.
    pub seed: u64,
    pub tick_hz: u32,
    pub hash_interval: u32,
    pub rules: RulesSetup,
    pub map: MapSetup,
    /// Sorted by id.
    pub weapons: Vec<WeaponSetup>,
    /// Thrown items (S1b abilities), sorted by id.
    pub items: Vec<ItemSetup>,
    /// Sorted by id.
    pub squad_types: Vec<SquadTypeSetup>,
    /// Sorted by id.
    pub factions: Vec<FactionSetup>,
    /// In player-slot order (player id = index).
    pub players: Vec<PlayerSetup>,
    /// Rounds for cannons and launchers (S3), sorted by id.
    pub ammo: Vec<AmmoSetup>,
    /// Vehicle types (S3), sorted by id.
    pub vehicles: Vec<VehicleSetup>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RulesSetup {
    pub tickets: u32,
    pub pop_cap: u32,
    pub projectile_cap: u32,
    pub start_manpower: u32,
    pub income_per_min: u32,
    pub point_income_per_min: u32,
    pub ticket_drain_per_min_per_point: u32,
    pub capture_radius: Fixed,
    pub capture_ticks: u32,
    pub capture_max_men: u8,
    /// Capture speed per man beyond the first, percent of one man's.
    pub capture_extra_man_pct: u32,
    pub movement: MovementSetup,
    /// The standard trench profile (`DigProfile::Trench`).
    pub dig: DigSetup,
    /// The anti-tank ditch profile (`DigProfile::AtDitch`, S3).
    pub at_ditch: DigSetup,
    pub combat: CombatSetup,
    pub items: ItemRulesSetup,
    /// Vehicle rules (S3).
    pub vehicles: VehicleRulesSetup,
    pub difficulty: DifficultyTable,
    pub ai: AiSetup,
    /// Cover on arrival (S3b).
    pub cover: CoverTuning,
    /// Spawn spots of bought squads (S3b).
    pub call_in: CallInTuning,
}

impl RulesSetup {
    /// The profile a dig order names.
    pub fn dig_profile(&self, p: DigProfile) -> &DigSetup {
        match p {
            DigProfile::Trench => &self.dig,
            DigProfile::AtDitch => &self.at_ditch,
        }
    }
}

/// Which dug cross-section a dig order makes (S3).
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
pub enum DigProfile {
    #[default]
    Trench = 0,
    AtDitch = 1,
}

/// Vehicle rules (see `VehicleRulesDef`): durations in ticks, distances
/// and speeds in metres.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VehicleRulesSetup {
    pub unlock_ticks: u32,
    pub limit: u32,
    pub wreck_cap: u32,
    pub seat_swap_ticks: u32,
    pub crew_hp: u32,
    pub wounded_hp: u32,
    /// m/s.
    pub moving_speed: Fixed,
    pub moving_spot_pct: u32,
    pub near_spot: Fixed,
    pub moving_target_spread_pct: u32,
    pub ranging_step_pct: u32,
    pub ranging_floor_pct: u32,
    /// Radians.
    pub he_area_spread: Fixed,
    pub he_clear: Fixed,
    pub crossing_dug: Fixed,
    /// m/s.
    pub crossing_speed: Fixed,
    pub crossing_gap_ticks: u32,
    pub fire_min_damage: u32,
    pub fire_start_pct: u32,
    pub fire_burn_hp_s: u32,
    pub fire_check_ticks: u32,
    pub fire_out_pct: u32,
    pub fire_burnout_ticks: u32,
    pub rack_min_damage: u32,
    pub rack_detonate_pct: u32,
    pub base_repair_pct_s: u32,
    pub base_rearm_ticks: u32,
    pub base_crew_ticks: u32,
    /// S3b tuning tables (TOML units; see `crate::tuning`).
    pub driving: DrivingTuning,
    pub path: VehiclePathTuning,
    pub escort: EscortTuning,
    pub gunnery: GunneryTuning,
    pub damage: VehicleDamageTuning,
    /// S3c tuning tables.
    pub bail: BailTuning,
    pub recrew: RecrewTuning,
}

/// Thrown-item rules (see `ItemRulesDef`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemRulesSetup {
    /// A man in his own base gets one item back every this many ticks.
    pub resupply_ticks: u32,
    /// S3b tuning table (TOML units).
    pub throw: ThrowTuning,
}

/// AI commander tuning (see `AiDef`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiSetup {
    pub retreat_strength_pct: u32,
    pub threat_radius: Fixed,
    /// Target share per [`SquadRole`] in role order (rifle, MG, engineer,
    /// mortar), percent.
    pub mix_pct: [u32; 4],
    pub barrage_rounds: u16,
    pub dig_length: Fixed,
    pub dig_offset: Fixed,
    /// A squad throws at most one item this often.
    pub throw_cooldown_ticks: u32,
    pub frag_group_men: u32,
    /// S3: vehicle and AT-team shares of the mix, percent.
    pub mix_vehicle_pct: u32,
    pub mix_at_pct: u32,
    pub tank_standoff: Fixed,
    pub assault_close: Fixed,
    pub at_clear_ticks: u32,
    pub repair_clear: Fixed,
    /// S3b tuning tables (TOML units).
    pub infantry: AiInfantryTuning,
    pub armour: AiArmourTuning,
    pub assault: AiAssaultTuning,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MovementSetup {
    pub slope_cost_pct: u32,
    pub max_slope_pct: u32,
    /// A soldier this close to his target counts as there (m).
    pub arrive_radius: Fixed,
    pub retreat_speed_pct: u32,
    pub catch_up_speed_pct: u32,
    /// Walking speed of a man ordered prone (crawling), in percent.
    pub prone_speed_pct: u32,
    pub path_budget_per_tick: u32,
    pub path_max_expansions: u32,
    /// Path step cost between two trench cells, in percent of normal.
    pub trench_cost_pct: u32,
    /// Infantry path step cost into an AT-ditch cell, in percent.
    pub ditch_cost_pct: u32,
    /// S3b tuning table (TOML units).
    pub formation: FormationTuning,
}

/// A dig profile and its pace. One man at `dig_rate_pct = 100` does 100
/// work points per second.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DigSetup {
    pub depth: Fixed,
    pub floor_width: Fixed,
    /// Horizontal run of each wall, floor edge to lip (S3; the S1 trench
    /// used a fixed 0.4 m).
    pub wall_run: Fixed,
    pub parapet_height: Fixed,
    pub max_length: Fixed,
    pub work_per_m: u32,
    /// The trench deepens in this many equal passes.
    pub passes: u32,
    /// Only engineer squads dig it (S3: the AT ditch).
    pub engineers_only: bool,
    /// The spoil bank shape and digger reach (the rules' `[dig.common]`,
    /// the same for every profile; S3b).
    pub common: DigCommonTuning,
}

impl DigSetup {
    fn lower(d: &DigProfileDef, common: &DigCommonTuning) -> DigSetup {
        DigSetup {
            depth: Fixed::from_cm(d.depth_cm),
            floor_width: Fixed::from_cm(d.floor_cm),
            wall_run: Fixed::from_cm(d.wall_cm),
            parapet_height: Fixed::from_cm(d.parapet_cm),
            max_length: Fixed::from_int(d.max_length_m as i32),
            work_per_m: d.work_per_m,
            passes: d.passes,
            engineers_only: d.engineers_only,
            common: common.clone(),
        }
    }
}

/// Spotting, accuracy, suppression and corpse rules (see `CombatDef`).
/// Suppression is in points out of 100; the sim keeps it in thousandths.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombatSetup {
    pub spot_rays_per_tick: u32,
    pub spot_period_ticks: u32,
    pub spot_observers: u32,
    pub prone_spot_pct: u32,
    pub trench_spot_pct: u32,
    pub woods_spot_pct: u32,
    pub firing_spot_pct: u32,
    pub reveal_ticks: u32,
    pub near_miss: Fixed,
    pub hit_suppression: u32,
    /// Suppression lost per tick, in thousandths of a point.
    pub suppression_decay_milli: u32,
    pub crouch_suppression: u32,
    pub pin_suppression: u32,
    pub moving_spread_pct: u32,
    pub pinned_spread_pct: u32,
    pub range_spread_pct: u32,
    pub fire_exposure: Fixed,
    pub corpse_ticks: u32,
    /// S3b tuning tables (TOML units).
    pub body: BodyTuning,
    pub mortar: MortarTuning,
    pub blast: BlastTuning,
    pub suppression: SuppressionTuning,
    pub suppress: SuppressTuning,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DifficultyTable {
    pub easy: DifficultySetup,
    pub normal: DifficultySetup,
    pub hard: DifficultySetup,
}

impl DifficultyTable {
    pub fn get(&self, d: Difficulty) -> &DifficultySetup {
        match d {
            Difficulty::Easy => &self.easy,
            Difficulty::Normal => &self.normal,
            Difficulty::Hard => &self.hard,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DifficultySetup {
    pub income_pct: u32,
    pub reaction_ticks: u32,
    pub replan_ticks: u32,
    pub decision_quality_pct: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapSetup {
    pub id: String,
    pub name: String,
    pub size_m: i32,
    pub terrain: TerrainSetup,
    pub woods: Vec<CircleSetup>,
    pub points: Vec<PointSetup>,
    pub bases: Vec<CircleSetup>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerrainSetup {
    pub seed: u64,
    pub base_height: Fixed,
    pub octaves: Vec<OctaveSetup>,
    /// Ridges (positive height) and valleys (negative height).
    pub bands: Vec<BandSetup>,
    /// S3c: the base-0 half is mirrored through the map centre onto the
    /// other half (`TerrainDef::symmetry = "point"`).
    #[serde(default)]
    pub point_symmetric: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OctaveSetup {
    pub period_m: i32,
    pub amplitude: Fixed,
}

/// A raised (or, with negative `height`, lowered) band along `from → to`
/// with a smoothstep falloff to zero at `half_width`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BandSetup {
    pub from: FixedVec2,
    pub to: FixedVec2,
    pub height: Fixed,
    pub half_width: Fixed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CircleSetup {
    pub center: FixedVec2,
    pub radius: Fixed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PointSetup {
    pub name: String,
    pub pos: FixedVec2,
    pub value: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponSetup {
    pub id: String,
    pub name: String,
    pub class: WeaponClass,
    pub range_min: Fixed,
    pub range_max: Fixed,
    pub damage: u32,
    /// Time between rounds in thousandths of a tick (at least 1000). The
    /// sim spreads the fraction over successive shots, so 1200 rpm at 30 Hz
    /// (1.5 ticks) fires at 1, 2, 1, 2 ... ticks instead of rounding.
    pub interval_milli: u32,
    pub magazine: u32,
    pub reload_ticks: u32,
    /// Dispersion half-angle in radians.
    pub spread: Fixed,
    pub muzzle_speed: Fixed,
    pub setup_ticks: u32,
    pub suppression: u32,
    pub he: Option<HeSetup>,
    /// Firing reveals the shooter: sight range against him in percent, for
    /// this many ticks (S3; the rules' values unless the def sets them).
    pub fire_reveal_pct: u32,
    pub fire_reveal_ticks: u32,
    /// Launchers (S3): the round (index into `MatchSetup::ammo`), rounds
    /// per team, fire discipline against still and moving vehicles, and
    /// the hand-over time when the gunner dies. `range_max` and 0
    /// otherwise.
    pub ammo: Option<u16>,
    pub rounds_carried: u32,
    pub engage_still: Fixed,
    pub engage_moving: Fixed,
    pub handover_ticks: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeSetup {
    /// S3b: overpressure radius (full damage with a clear line).
    pub blast_radius: Fixed,
    /// S3b fragments: rays per burst, ray length, damage at the burst and
    /// at full reach (percent), the elevation band (integer degrees), and
    /// damage to a track whose band a fragment strikes.
    pub frag_count: u32,
    pub frag_reach: Fixed,
    pub frag_damage: u32,
    pub frag_far_pct: u32,
    pub frag_elev_deg: [i32; 2],
    pub frag_track_damage: u32,
    pub suppression_radius: Fixed,
    pub crater_radius: Fixed,
    pub crater_depth: Fixed,
    /// S3: armour it defeats on contact (mm); S3b: the inner fragment rays
    /// when it does (count, damage each, reach); its damage to track
    /// anchors in percent.
    pub pen_mm: u32,
    pub inside_damage: u32,
    pub inside_rays: u32,
    pub inside_reach: Fixed,
    pub vs_vehicle_pct: u32,
}

/// A thrown item (see `ItemDef`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemSetup {
    pub id: String,
    pub name: String,
    pub kind: ItemKind,
    pub range_min: Fixed,
    pub range_max: Fixed,
    pub windup_ticks: u32,
    /// From the start of the throw; never before landing.
    pub fuse_ticks: u32,
    pub scatter_pct: u32,
    pub he: Option<ItemHeSetup>,
    pub smoke: Option<SmokeSetup>,
}

/// An item's blast: a weapon-style [`HeSetup`] plus its own damage and
/// suppression.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemHeSetup {
    pub damage: u32,
    pub suppression: u32,
    pub blast: HeSetup,
    /// The crater levels a trench or parapet instead of deepening it.
    pub cuts_trenches: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmokeSetup {
    pub radius: Fixed,
    pub grow_ticks: u32,
    pub hold_ticks: u32,
    pub fade_ticks: u32,
}

impl SmokeSetup {
    /// Ticks from landing until the cloud is gone.
    pub fn life_ticks(&self) -> u32 {
        self.grow_ticks + self.hold_ticks + self.fade_ticks
    }
}

/// One loadout entry: `count` per man, `leader_count` for the squad leader.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitSetup {
    /// Index into [`MatchSetup::items`].
    pub item: u16,
    pub count: u8,
    pub leader_count: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SquadTypeSetup {
    pub id: String,
    pub name: String,
    /// Index into [`MatchSetup::factions`].
    pub faction: u16,
    pub role: SquadRole,
    pub men: u8,
    /// Index into [`MatchSetup::weapons`].
    pub weapon: u16,
    pub cost: u32,
    /// Ticks after a call-in before this type can be called in again, and
    /// before its first call-in (S3b; a call-in spawns the squad at once).
    pub recall_cooldown_ticks: u32,
    pub initial_cooldown_ticks: u32,
    /// Types with the same group share one re-call timer (S3b): a dense
    /// index over the mod's group ids in sorted order.
    pub cooldown_group: Option<u16>,
    pub hp: u32,
    /// Walking speed (m/s).
    pub speed: Fixed,
    pub sight: Fixed,
    /// Formation spacing (m).
    pub spacing: Fixed,
    pub dig_rate_pct: u32,
    /// S3b: suppression thresholds (duck, release, pin) are scaled by this
    /// (percent; 100 for vehicles).
    pub steadiness_pct: u32,
    /// What the men carry, at most one entry per item kind.
    pub loadout: Vec<KitSetup>,
    /// S3: the vehicle (index into `MatchSetup::vehicles`) for `Vehicle`
    /// squads. Those have `men = 0`, the main gun as `weapon`, and the
    /// vehicle's cost, cooldowns, speed and sight; [`Self::pop`] is the
    /// vehicle's.
    pub vehicle: Option<u16>,
    /// The first `weapon_men` men carry `weapon` (all of them when it is at
    /// least `men`), the rest `sidearm`.
    pub weapon_men: u8,
    /// Index into `MatchSetup::weapons`.
    pub sidearm: Option<u16>,
}

impl SquadTypeSetup {
    /// Population it uses: its men, or its vehicle's `pop` (S3).
    pub fn pop(&self, vehicles: &[VehicleSetup]) -> u32 {
        match self.vehicle {
            Some(v) => vehicles.get(v as usize).map_or(0, |v| v.pop),
            None => self.men as u32,
        }
    }

    /// The loadout entry for items of `kind`, with the item's setup.
    pub fn kit<'a>(
        &'a self,
        items: &'a [ItemSetup],
        kind: ItemKind,
    ) -> Option<(&'a KitSetup, &'a ItemSetup)> {
        self.loadout.iter().find_map(|k| {
            let it = items.get(k.item as usize)?;
            (it.kind == kind).then_some((k, it))
        })
    }

    /// Items a man of this type carries when full, by [`ItemKind`].
    pub fn full_kit(&self, items: &[ItemSetup], leader: bool) -> [u8; ITEM_KINDS] {
        let mut out = [0; ITEM_KINDS];
        for kind in ItemKind::ALL {
            if let Some((k, _)) = self.kit(items, kind) {
                out[kind.index()] = if leader { k.leader_count } else { k.count };
            }
        }
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactionSetup {
    pub id: String,
    pub name: String,
    /// `0xRRGGBB`.
    pub color_rgb: u32,
    /// Indices into [`MatchSetup::squad_types`].
    pub roster: Vec<u16>,
    pub start_squads: Vec<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerSetup {
    /// Index into [`MatchSetup::factions`].
    pub faction: u16,
    pub is_ai: bool,
    pub difficulty: Difficulty,
    /// Index into [`MapSetup::bases`].
    pub base: u8,
}

// -------------------------------------------------------------- resolve --

impl ModDefs {
    /// Resolves `req` against these defs into a [`MatchSetup`]. Every problem
    /// (unknown ids, too few bases, ...) is reported at once.
    pub fn resolve(&self, req: &MatchRequest) -> Result<MatchSetup, ContentError> {
        let mut issues = Vec::new();
        let rules = self.rules.get(&req.rules);
        if rules.is_none() {
            issues.push(Issue::new(
                "rules",
                format!("unknown rules `{}`", req.rules),
            ));
        }
        let map = self.maps.get(&req.map);
        if map.is_none() {
            issues.push(Issue::new("map", format!("unknown map `{}`", req.map)));
        }
        if req.players.len() < 2 {
            issues.push(Issue::new("players", "a match needs at least two players"));
        }
        if let Some(m) = map
            && req.players.len() > m.bases.len()
        {
            issues.push(Issue::new(
                "players",
                format!(
                    "map `{}` has {} bases, {} players requested",
                    m.id,
                    m.bases.len(),
                    req.players.len()
                ),
            ));
        }
        for (i, p) in req.players.iter().enumerate() {
            if !self.factions.contains_key(&p.faction) {
                issues.push(Issue::new(
                    format!("players[{i}].faction"),
                    format!("unknown faction `{}`", p.faction),
                ));
            }
        }
        let (Some(rules), Some(map)) = (rules, map) else {
            return Err(invalid(issues));
        };
        if !issues.is_empty() {
            return Err(invalid(issues));
        }

        let hz = rules.tick_hz;
        let weapons: Vec<WeaponSetup> = self
            .weapons
            .values()
            .map(|w| WeaponSetup {
                id: w.id.clone(),
                name: w.name.clone(),
                class: w.class,
                range_min: Fixed::from_int(w.min_range_m as i32),
                range_max: Fixed::from_int(w.range_m as i32),
                damage: w.damage,
                // 60 s * hz / rpm in milli-ticks, rounded half up, at least
                // one tick.
                interval_milli: ((120_000 * hz as u64 + w.rounds_per_min as u64)
                    / (2 * w.rounds_per_min as u64))
                    .max(1000) as u32,
                magazine: w.magazine,
                reload_ticks: ms_to_ticks(w.reload_ms, hz),
                spread: Fixed::from_ratio(
                    w.spread_mrad as i64 * 1000 + w.spread_urad as i64,
                    1_000_000,
                ),
                muzzle_speed: Fixed::from_int(w.muzzle_velocity_m_s as i32),
                setup_ticks: ms_to_ticks(w.setup_ms, hz),
                suppression: w.suppression,
                he: w.he.as_ref().map(|h| HeSetup {
                    blast_radius: Fixed::from_cm(h.blast_radius_cm),
                    frag_count: h.frag_count,
                    frag_reach: Fixed::from_cm(h.frag_reach_cm),
                    frag_damage: h.frag_damage,
                    frag_far_pct: h.frag_far_pct,
                    frag_elev_deg: h.frag_elev_deg.map(|d| d as i32),
                    frag_track_damage: h.frag_track_damage,
                    suppression_radius: Fixed::from_int(h.suppression_radius_m as i32),
                    crater_radius: Fixed::from_cm(h.crater_radius_cm),
                    crater_depth: Fixed::from_cm(h.crater_depth_cm),
                    pen_mm: h.pen_mm,
                    inside_damage: h.inside_damage,
                    inside_rays: h.inside_rays,
                    inside_reach: Fixed::from_cm(h.inside_reach_cm),
                    vs_vehicle_pct: h.vs_vehicle_pct,
                }),
                fire_reveal_pct: w.fire_reveal_pct.unwrap_or(rules.combat.firing_spot_pct),
                fire_reveal_ticks: ms_to_ticks(
                    w.fire_reveal_ms.unwrap_or(rules.combat.reveal_ms),
                    hz,
                ),
                ammo: w.ammo.as_ref().and_then(|a| index_of(self.ammo.keys(), a)),
                rounds_carried: w.rounds_carried,
                engage_still: Fixed::from_int(w.engage_still_m.unwrap_or(w.range_m) as i32),
                engage_moving: Fixed::from_int(w.engage_moving_m.unwrap_or(w.range_m) as i32),
                handover_ticks: ms_to_ticks(w.handover_ms, hz),
            })
            .collect();
        let items: Vec<ItemSetup> = self
            .items
            .values()
            .map(|it| ItemSetup {
                id: it.id.clone(),
                name: it.name.clone(),
                kind: it.kind,
                range_min: Fixed::from_int(it.min_range_m as i32),
                range_max: Fixed::from_int(it.range_m as i32),
                windup_ticks: ms_to_ticks(it.windup_ms, hz),
                fuse_ticks: ms_to_ticks(it.fuse_ms, hz),
                scatter_pct: it.scatter_pct,
                he: it.he.as_ref().map(lower_item_he),
                smoke: it.smoke.as_ref().map(|m| SmokeSetup {
                    radius: Fixed::from_int(m.radius_m as i32),
                    grow_ticks: ms_to_ticks(m.grow_ms, hz),
                    hold_ticks: ms_to_ticks(m.hold_ms, hz),
                    fade_ticks: ms_to_ticks(m.fade_ms, hz),
                }),
            })
            .collect();
        let ammo: Vec<AmmoSetup> = self.ammo.values().map(lower_ammo).collect();
        let vehicles = self.lower_vehicles(hz);
        let weapon_idx = |id: &str| index_of(self.weapons.keys(), id).unwrap_or(0);
        // Cooldown groups (S3b): a dense index in sorted id order.
        let groups: std::collections::BTreeSet<&str> = self
            .squads
            .values()
            .filter_map(|s| s.cooldown_group.as_deref())
            .collect();
        let group_of = |s: &crate::SquadDef| {
            s.cooldown_group
                .as_deref()
                .and_then(|g| groups.iter().position(|x| *x == g))
                .map(|k| k as u16)
        };
        let squad_types: Vec<SquadTypeSetup> = self
            .squads
            .values()
            .map(|s| {
                let faction = index_of(self.factions.keys(), &s.faction).unwrap_or(0);
                let cooldown_group = group_of(s);
                let vehicle = s
                    .vehicle
                    .as_ref()
                    .and_then(|v| index_of(self.vehicles.keys(), v));
                if let Some(v) = vehicle {
                    let vs = &vehicles[v as usize];
                    return SquadTypeSetup {
                        id: s.id.clone(),
                        name: s.name.clone(),
                        faction,
                        role: s.role,
                        men: 0,
                        weapon: vs.main_gun().weapon,
                        cost: vs.cost,
                        recall_cooldown_ticks: vs.recall_cooldown_ticks,
                        initial_cooldown_ticks: vs.initial_cooldown_ticks,
                        cooldown_group,
                        hp: s.hp,
                        speed: vs.mobility.speed,
                        sight: vs.sight,
                        spacing: Fixed::ONE,
                        dig_rate_pct: 0,
                        steadiness_pct: crate::defs::DEFAULT_STEADINESS_PCT,
                        loadout: Vec::new(),
                        vehicle: Some(v),
                        weapon_men: 0,
                        sidearm: None,
                    };
                }
                SquadTypeSetup {
                    id: s.id.clone(),
                    name: s.name.clone(),
                    faction,
                    role: s.role,
                    men: s.men,
                    weapon: weapon_idx(&s.weapon),
                    cost: s.cost,
                    recall_cooldown_ticks: s.recall_cooldown_s * hz,
                    initial_cooldown_ticks: s.initial_cooldown_s * hz,
                    cooldown_group,
                    hp: s.hp,
                    speed: Fixed::from_cm(s.speed_cm_s),
                    sight: Fixed::from_int(s.sight_m as i32),
                    spacing: Fixed::from_ratio(s.spacing_dm as i64, 10),
                    dig_rate_pct: s.dig_rate_pct,
                    steadiness_pct: s.steadiness_pct,
                    loadout: s
                        .loadout
                        .iter()
                        .map(|l| KitSetup {
                            item: index_of(self.items.keys(), &l.item).unwrap_or(0),
                            count: l.count,
                            leader_count: l.leader_count,
                        })
                        .collect(),
                    vehicle: None,
                    weapon_men: s.weapon_men.unwrap_or(s.men),
                    sidearm: s.sidearm.as_deref().map(weapon_idx),
                }
            })
            .collect();
        let squad_idx = |id: &String| index_of(self.squads.keys(), id).unwrap_or(0);
        let factions: Vec<FactionSetup> = self
            .factions
            .values()
            .map(|f| FactionSetup {
                id: f.id.clone(),
                name: f.name.clone(),
                color_rgb: parse_color(&f.color).unwrap_or(0),
                roster: f.roster.iter().map(squad_idx).collect(),
                start_squads: f.start_squads.iter().map(squad_idx).collect(),
            })
            .collect();
        let players = req
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| PlayerSetup {
                faction: index_of(self.factions.keys(), &p.faction).unwrap_or(0),
                is_ai: p.ai,
                difficulty: p.difficulty,
                base: i as u8,
            })
            .collect();

        let t = &map.terrain;
        let mut bands: Vec<BandSetup> = t
            .ridges
            .iter()
            .map(|r| BandSetup {
                from: FixedVec2::from_m(r.from),
                to: FixedVec2::from_m(r.to),
                height: Fixed::from_int(r.height_m as i32),
                half_width: Fixed::from_ratio(r.width_m as i64, 2),
            })
            .collect();
        bands.extend(t.valleys.iter().map(|v| BandSetup {
            from: FixedVec2::from_m(v.from),
            to: FixedVec2::from_m(v.to),
            height: Fixed::from_int(-(v.depth_m as i32)),
            half_width: Fixed::from_ratio(v.width_m as i64, 2),
        }));
        let diff = |d: &crate::defs::DifficultyDef| DifficultySetup {
            income_pct: d.income_pct,
            reaction_ticks: ms_to_ticks(d.reaction_ms, hz),
            replan_ticks: ms_to_ticks(d.replan_ms, hz).max(1),
            decision_quality_pct: d.decision_quality_pct,
        };
        let m = &rules.movement;
        let setup = MatchSetup {
            mod_id: self
                .manifest
                .as_ref()
                .map(|m| m.id.clone())
                .unwrap_or_default(),
            rules_id: rules.id.clone(),
            seed: req.seed,
            tick_hz: hz,
            hash_interval: rules.hash_interval,
            rules: RulesSetup {
                tickets: rules.tickets,
                pop_cap: rules.pop_cap,
                projectile_cap: rules.projectile_cap,
                start_manpower: rules.economy.start_manpower,
                income_per_min: rules.economy.income_per_min,
                point_income_per_min: rules.economy.point_income_per_min,
                ticket_drain_per_min_per_point: rules.tickets_drain.per_min_per_point,
                capture_radius: Fixed::from_int(rules.capture.radius_m as i32),
                capture_ticks: rules.capture.time_s * hz,
                capture_max_men: rules.capture.max_men,
                capture_extra_man_pct: rules.capture.extra_man_pct,
                movement: MovementSetup {
                    slope_cost_pct: m.slope_cost_pct,
                    max_slope_pct: m.max_slope_pct,
                    arrive_radius: Fixed::from_cm(m.arrive_cm),
                    retreat_speed_pct: m.retreat_speed_pct,
                    catch_up_speed_pct: m.catch_up_speed_pct,
                    prone_speed_pct: m.prone_speed_pct,
                    path_budget_per_tick: m.path_budget_per_tick,
                    path_max_expansions: m.path_max_expansions,
                    trench_cost_pct: m.trench_cost_pct,
                    ditch_cost_pct: m.ditch_cost_pct,
                    formation: m.formation.clone(),
                },
                dig: DigSetup::lower(&rules.dig.profiles.trench, &rules.dig.common),
                at_ditch: DigSetup::lower(&rules.dig.profiles.at_ditch, &rules.dig.common),
                combat: {
                    let c = &rules.combat;
                    CombatSetup {
                        spot_rays_per_tick: c.spot_rays_per_tick,
                        spot_period_ticks: ms_to_ticks(c.spot_period_ms, hz).max(1),
                        spot_observers: c.spot_observers,
                        prone_spot_pct: c.prone_spot_pct,
                        trench_spot_pct: c.trench_spot_pct,
                        woods_spot_pct: c.woods_spot_pct,
                        firing_spot_pct: c.firing_spot_pct,
                        reveal_ticks: ms_to_ticks(c.reveal_ms, hz),
                        near_miss: Fixed::from_cm(c.near_miss_cm),
                        hit_suppression: c.hit_suppression,
                        suppression_decay_milli: c.suppression_decay_per_s * 1000 / hz,
                        crouch_suppression: c.crouch_suppression,
                        pin_suppression: c.pin_suppression,
                        moving_spread_pct: c.moving_spread_pct,
                        pinned_spread_pct: c.pinned_spread_pct,
                        range_spread_pct: c.range_spread_pct,
                        fire_exposure: Fixed::from_cm(c.fire_exposure_cm),
                        corpse_ticks: c.corpse_s * hz,
                        body: c.body.clone(),
                        mortar: c.mortar.clone(),
                        blast: c.blast.clone(),
                        suppression: c.suppression.clone(),
                        suppress: c.suppress.clone(),
                    }
                },
                items: ItemRulesSetup {
                    resupply_ticks: rules.items.resupply_s * hz,
                    throw: rules.items.throw.clone(),
                },
                vehicles: {
                    let v = &rules.vehicles;
                    VehicleRulesSetup {
                        unlock_ticks: v.unlock_s * hz,
                        limit: v.limit,
                        wreck_cap: v.wreck_cap,
                        seat_swap_ticks: v.seat_swap_s * hz,
                        crew_hp: v.crew_hp,
                        wounded_hp: v.wounded_hp,
                        moving_speed: Fixed::from_cm(v.moving_cm_s),
                        moving_spot_pct: v.moving_spot_pct,
                        near_spot: Fixed::from_int(v.near_spot_m as i32),
                        moving_target_spread_pct: v.moving_target_spread_pct,
                        ranging_step_pct: v.ranging_step_pct,
                        ranging_floor_pct: v.ranging_floor_pct,
                        he_area_spread: Fixed::from_ratio(v.he_area_spread_mrad as i64, 1000),
                        he_clear: Fixed::from_int(v.he_clear_m as i32),
                        crossing_dug: Fixed::from_cm(v.crossing_dug_cm),
                        crossing_speed: Fixed::from_cm(v.crossing_speed_cm_s),
                        crossing_gap_ticks: v.crossing_gap_s * hz,
                        fire_min_damage: v.fire_min_damage,
                        fire_start_pct: v.fire_start_pct,
                        fire_burn_hp_s: v.fire_burn_hp_s,
                        fire_check_ticks: v.fire_check_s * hz,
                        fire_out_pct: v.fire_out_pct,
                        fire_burnout_ticks: v.fire_burnout_s * hz,
                        rack_min_damage: v.rack_min_damage,
                        rack_detonate_pct: v.rack_detonate_pct,
                        base_repair_pct_s: v.base_repair_pct_s,
                        base_rearm_ticks: v.base_rearm_s * hz,
                        base_crew_ticks: v.base_crew_s * hz,
                        driving: v.driving.clone(),
                        path: v.path.clone(),
                        escort: v.escort.clone(),
                        gunnery: v.gunnery.clone(),
                        damage: v.damage.clone(),
                        bail: v.bail.clone(),
                        recrew: v.recrew.clone(),
                    }
                },
                difficulty: DifficultyTable {
                    easy: diff(&rules.difficulty.easy),
                    normal: diff(&rules.difficulty.normal),
                    hard: diff(&rules.difficulty.hard),
                },
                ai: {
                    let a = &rules.ai;
                    AiSetup {
                        retreat_strength_pct: a.retreat_strength_pct,
                        threat_radius: Fixed::from_int(a.threat_radius_m as i32),
                        mix_pct: [
                            a.mix_rifle_pct,
                            a.mix_mg_pct,
                            a.mix_engineer_pct,
                            a.mix_mortar_pct,
                        ],
                        barrage_rounds: a.barrage_rounds as u16,
                        dig_length: Fixed::from_int(a.dig_length_m as i32),
                        dig_offset: Fixed::from_int(a.dig_offset_m as i32),
                        throw_cooldown_ticks: a.throw_cooldown_s * hz,
                        frag_group_men: a.frag_group_men,
                        mix_vehicle_pct: a.mix_vehicle_pct,
                        mix_at_pct: a.mix_at_pct,
                        tank_standoff: Fixed::from_int(a.tank_standoff_m as i32),
                        assault_close: Fixed::from_int(a.assault_close_m as i32),
                        at_clear_ticks: a.at_clear_s * hz,
                        repair_clear: Fixed::from_int(a.repair_clear_m as i32),
                        infantry: a.infantry.clone(),
                        armour: a.armour.clone(),
                        assault: a.assault.clone(),
                    }
                },
                cover: rules.cover.clone(),
                call_in: rules.call_in.clone(),
            },
            map: MapSetup {
                id: map.id.clone(),
                name: map.name.clone(),
                size_m: map.size_m as i32,
                terrain: TerrainSetup {
                    seed: t.seed,
                    base_height: Fixed::from_int(t.base_height_m as i32),
                    octaves: t
                        .octaves
                        .iter()
                        .map(|o| OctaveSetup {
                            period_m: o.period_m as i32,
                            amplitude: Fixed::from_cm(o.amplitude_cm),
                        })
                        .collect(),
                    bands,
                    point_symmetric: t.symmetry == crate::defs::TerrainSymmetry::Point,
                },
                woods: map
                    .woods
                    .iter()
                    .map(|w| CircleSetup {
                        center: FixedVec2::from_m(w.center),
                        radius: Fixed::from_int(w.radius_m as i32),
                    })
                    .collect(),
                points: map
                    .capture_points
                    .iter()
                    .map(|c| PointSetup {
                        name: c.name.clone(),
                        pos: FixedVec2::from_m(c.pos),
                        value: c.value,
                    })
                    .collect(),
                bases: map
                    .bases
                    .iter()
                    .map(|b| CircleSetup {
                        center: FixedVec2::from_m(b.pos),
                        radius: Fixed::from_int(b.radius_m as i32),
                    })
                    .collect(),
            },
            weapons,
            items,
            squad_types,
            factions,
            players,
            ammo,
            vehicles,
        };
        let issues = setup.validate();
        if issues.is_empty() {
            Ok(setup)
        } else {
            Err(invalid(issues))
        }
    }
}

impl ModDefs {
    /// Every vehicle def lowered at `hz`, sorted by id (also what
    /// `frontline-cli ballistics` builds its armour tables from, without a
    /// whole match).
    pub fn lower_vehicles(&self, hz: u32) -> Vec<VehicleSetup> {
        self.vehicles
            .values()
            .map(|v| {
                let mut out = lower_vehicle(
                    v,
                    hz,
                    index_of(self.factions.keys(), &v.faction).unwrap_or(0),
                    |w| index_of(self.weapons.keys(), w).unwrap_or(0),
                    |a| index_of(self.ammo.keys(), a).unwrap_or(0),
                    |a| self.ammo.get(a).map(|d| d.mechanism),
                );
                // S3c: the crew squad type, by its index in the sorted
                // squad list (`squad_types` has the same order).
                out.crew_squad = v
                    .crew_squad
                    .as_ref()
                    .and_then(|c| index_of(self.squads.keys(), c));
                out
            })
            .collect()
    }

    /// Every ammo def lowered, sorted by id.
    pub fn lower_ammo(&self) -> Vec<AmmoSetup> {
        self.ammo.values().map(lower_ammo).collect()
    }
}

/// Lowers an item's (or a round's) blast.
pub(crate) fn lower_item_he(h: &ItemHeDef) -> ItemHeSetup {
    ItemHeSetup {
        damage: h.damage,
        suppression: h.suppression,
        blast: HeSetup {
            blast_radius: Fixed::from_cm(h.blast_radius_cm),
            frag_count: h.frag_count,
            frag_reach: Fixed::from_cm(h.frag_reach_cm),
            frag_damage: h.frag_damage,
            frag_far_pct: h.frag_far_pct,
            frag_elev_deg: h.frag_elev_deg.map(|d| d as i32),
            frag_track_damage: h.frag_track_damage,
            suppression_radius: Fixed::from_int(h.suppression_radius_m as i32),
            crater_radius: Fixed::from_cm(h.crater_radius_cm),
            crater_depth: Fixed::from_cm(h.crater_depth_cm),
            pen_mm: h.pen_mm,
            inside_damage: h.inside_damage,
            inside_rays: h.inside_rays,
            inside_reach: Fixed::from_cm(h.inside_reach_cm),
            vs_vehicle_pct: h.vs_vehicle_pct,
        },
        cuts_trenches: h.cuts_trenches,
    }
}

fn fx_in(v: Fixed, lo: i32, hi: i32) -> bool {
    v >= Fixed::from_int(lo) && v <= Fixed::from_int(hi)
}

/// Range checks of one blast (weapon, item or round).
pub(crate) fn he_ok(b: &HeSetup) -> bool {
    fx_in(b.blast_radius, 0, 20)
        && b.frag_count <= crate::defs::MAX_FRAG_RAYS
        && fx_in(b.frag_reach, 0, 50)
        && b.frag_damage <= 10_000
        && b.frag_far_pct <= 100
        && (0..=89).contains(&b.frag_elev_deg[0])
        && (b.frag_elev_deg[0]..=89).contains(&b.frag_elev_deg[1])
        && b.frag_track_damage <= 1000
        && fx_in(b.suppression_radius, 0, 200)
        && fx_in(b.crater_radius, 0, 20)
        && fx_in(b.crater_depth, 0, 5)
        && b.pen_mm <= 2000
        && b.inside_damage <= 10_000
        && b.inside_rays <= crate::defs::MAX_INSIDE_RAYS
        && fx_in(b.inside_reach, 0, 10)
        && b.vs_vehicle_pct <= 100
}

pub(crate) fn item_he_ok(h: &ItemHeSetup) -> bool {
    h.damage <= 10_000 && h.suppression <= 100 && he_ok(&h.blast)
}

/// Position of `id` among sorted map keys.
fn index_of<'a>(mut keys: impl Iterator<Item = &'a String>, id: &str) -> Option<u16> {
    keys.position(|k| k == id).map(|i| i as u16)
}

fn invalid(issues: Vec<Issue>) -> ContentError {
    ContentError::Invalid {
        origin: "match setup".into(),
        issues,
    }
}

// ------------------------------------------------------------- validate --

/// Most players in one match (§2.2 lobby limit).
pub const MAX_MATCH_PLAYERS: usize = 10;

impl MatchSetup {
    /// Structural checks the sim relies on (indices in range, positions on
    /// the map, magnitudes that keep fixed-point math in range). Defs that
    /// passed [`ModDefs::load`] always produce a valid setup; this exists
    /// for setups read back from saves.
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        let mut check = |ok: bool, path: &str, msg: &str| {
            if !ok {
                out.push(Issue::new(path, msg));
            }
        };
        let fx_in =
            |v: Fixed, lo: i32, hi: i32| v >= Fixed::from_int(lo) && v <= Fixed::from_int(hi);

        check(
            crate::TICK_HZ_ALLOWED.contains(&self.tick_hz),
            "tick_hz",
            "must be 30 or 20",
        );
        check(
            self.hash_interval >= 1,
            "hash_interval",
            "must be at least 1",
        );
        let size = self.map.size_m;
        check(
            (64..=8192).contains(&size) && size % 64 == 0,
            "map.size_m",
            "must be a multiple of 64 in 64..=8192",
        );
        let on_map = |p: FixedVec2| fx_in(p.x, 0, size) && fx_in(p.y, 0, size);
        let t = &self.map.terrain;
        check(
            fx_in(t.base_height, -1000, 1000),
            "map.terrain.base_height",
            "out of range",
        );
        for o in &t.octaves {
            check(
                o.period_m >= 8 && o.period_m <= size.max(8) && o.period_m % 4 == 0,
                "map.terrain.octaves.period_m",
                "must be a multiple of 4 in 8..=size_m",
            );
            check(
                fx_in(o.amplitude, 0, 100),
                "map.terrain.octaves.amplitude",
                "out of range",
            );
        }
        for b in &t.bands {
            check(
                on_map(b.from) && on_map(b.to),
                "map.terrain.bands",
                "off the map",
            );
            check(
                fx_in(b.height, -200, 200),
                "map.terrain.bands.height",
                "out of range",
            );
            check(
                b.half_width >= Fixed::from_int(4) && fx_in(b.half_width, 4, 2000),
                "map.terrain.bands.half_width",
                "out of range",
            );
        }
        for c in self.map.woods.iter().chain(&self.map.bases) {
            check(on_map(c.center), "map.circles", "off the map");
            check(
                c.radius > Fixed::ZERO && fx_in(c.radius, 0, 1000),
                "map.circles.radius",
                "out of range",
            );
        }
        check(
            !self.map.points.is_empty(),
            "map.points",
            "need at least one",
        );
        check(
            self.map.points.len() <= crate::MAX_CAPTURE_POINTS,
            "map.points",
            "too many points",
        );
        for p in &self.map.points {
            check(on_map(p.pos), "map.points.pos", "off the map");
            check(
                (1..=100).contains(&p.value),
                "map.points.value",
                "out of range",
            );
        }

        let r = &self.rules;
        check(
            r.pop_cap >= 1 && r.pop_cap <= 10_000,
            "rules.pop_cap",
            "out of range",
        );
        check(
            r.projectile_cap <= 1 << 20,
            "rules.projectile_cap",
            "must be at most 1048576",
        );
        check(
            fx_in(r.capture_radius, 1, 100),
            "rules.capture_radius",
            "out of range",
        );
        let m = &r.movement;
        check(
            m.arrive_radius > Fixed::ZERO && fx_in(m.arrive_radius, 0, 5),
            "rules.movement.arrive_radius",
            "must be in (0, 5] m",
        );
        check(
            m.max_slope_pct >= 1 && m.max_slope_pct <= 1000 && m.slope_cost_pct <= 10_000,
            "rules.movement.slope",
            "out of range",
        );
        check(
            (10..=500).contains(&m.retreat_speed_pct)
                && (100..=300).contains(&m.catch_up_speed_pct)
                && (10..=100).contains(&m.prone_speed_pct),
            "rules.movement.speed_pct",
            "out of range",
        );
        check(
            (64..=1_000_000).contains(&m.path_budget_per_tick)
                && (64..=1_000_000).contains(&m.path_max_expansions),
            "rules.movement.path_budget",
            "out of range",
        );
        check(
            (10..=100).contains(&m.trench_cost_pct),
            "rules.movement.trench_cost_pct",
            "out of range",
        );
        for d in [&r.dig, &r.at_ditch] {
            check(
                fx_in(d.max_length, 1, 1000),
                "rules.dig.max_length",
                "out of range",
            );
            check(
                d.depth >= Fixed::from_cm(10)
                    && fx_in(d.depth, 0, 10)
                    && d.floor_width >= Fixed::from_cm(10)
                    && fx_in(d.floor_width, 0, 10)
                    && d.wall_run >= Fixed::from_cm(10)
                    && fx_in(d.wall_run, 0, 5)
                    && d.parapet_height >= Fixed::ZERO
                    && fx_in(d.parapet_height, 0, 3),
                "rules.dig.profile",
                "out of range",
            );
            check(
                (1..=10_000_000).contains(&d.work_per_m) && (1..=32).contains(&d.passes),
                "rules.dig.work",
                "out of range",
            );
        }
        let d = &r.dig;
        let v = &r.vehicles;
        check(
            v.unlock_ticks <= 3600 * 30
                && v.limit <= 50
                && (1..=256).contains(&v.wreck_cap)
                && v.seat_swap_ticks <= 60 * 30
                && (1..=1000).contains(&v.crew_hp)
                && v.wounded_hp <= v.crew_hp
                && v.moving_speed > Fixed::ZERO
                && fx_in(v.moving_speed, 0, 10)
                && (1..=1000).contains(&v.moving_spot_pct)
                && fx_in(v.near_spot, 0, 200)
                && (100..=1000).contains(&v.moving_target_spread_pct)
                && v.ranging_step_pct <= 100
                && (1..=100).contains(&v.ranging_floor_pct)
                && fx_in(v.he_area_spread, 0, 1)
                && fx_in(v.he_clear, 0, 500),
            "rules.vehicles",
            "out of range",
        );
        check(
            v.crossing_dug >= Fixed::from_cm(10)
                && fx_in(v.crossing_dug, 0, 10)
                && v.crossing_speed >= Fixed::from_cm(10)
                && fx_in(v.crossing_speed, 0, 30)
                && v.crossing_gap_ticks <= 60 * 30
                && v.fire_min_damage <= 1000
                && v.fire_start_pct <= 100
                && v.fire_burn_hp_s <= 100
                && (1..=60 * 30).contains(&v.fire_check_ticks)
                && v.fire_out_pct <= 100
                && (1..=600 * 30).contains(&v.fire_burnout_ticks)
                && v.rack_min_damage <= 1000
                && v.rack_detonate_pct <= 100
                && v.base_repair_pct_s <= 100
                && (1..=600 * 30).contains(&v.base_rearm_ticks)
                && (1..=600 * 30).contains(&v.base_crew_ticks),
            "rules.vehicles",
            "out of range",
        );
        let c = &r.combat;
        check(
            (1..=100_000).contains(&c.spot_rays_per_tick)
                && (1..=16).contains(&c.spot_observers)
                && c.spot_observers <= c.spot_rays_per_tick
                && (1..=10_000).contains(&c.spot_period_ticks)
                && c.reveal_ticks <= 10_000,
            "rules.combat.spotting",
            "out of range",
        );
        check(
            [
                c.prone_spot_pct,
                c.trench_spot_pct,
                c.woods_spot_pct,
                c.firing_spot_pct,
                c.moving_spread_pct,
                c.pinned_spread_pct,
            ]
            .iter()
            .all(|v| (1..=1000).contains(v))
                && c.range_spread_pct <= 1000,
            "rules.combat.pct",
            "out of range",
        );
        check(
            (1..=100).contains(&c.crouch_suppression)
                && (c.crouch_suppression..=100).contains(&c.pin_suppression)
                && c.hit_suppression <= 100
                && c.suppression_decay_milli <= 100_000,
            "rules.combat.suppression",
            "out of range",
        );
        check(
            fx_in(c.near_miss, 0, 20) && fx_in(c.fire_exposure, 0, 2) && c.corpse_ticks <= 600 * 30,
            "rules.combat.values",
            "out of range",
        );

        // Same bounds as the defs; the sim's economy maths relies on them.
        check(
            r.start_manpower <= 1_000_000
                && r.income_per_min <= 1_000_000
                && r.point_income_per_min <= 1_000_000
                && r.ticket_drain_per_min_per_point <= 100_000
                && r.tickets <= 1_000_000,
            "rules.economy",
            "out of range",
        );
        check(
            r.tickets >= 1 && r.capture_ticks >= 1 && r.capture_max_men >= 1,
            "rules.capture",
            "tickets, capture time and capture men must be at least 1",
        );
        for t in [&r.difficulty.easy, &r.difficulty.normal, &r.difficulty.hard] {
            check(
                t.income_pct <= 1000
                    && t.reaction_ticks <= 60 * 30
                    && (1..=60 * 30).contains(&t.replan_ticks)
                    && t.decision_quality_pct <= 100,
                "rules.difficulty",
                "out of range",
            );
        }
        let a = &r.ai;
        check(
            a.retreat_strength_pct <= 100
                && fx_in(a.threat_radius, 1, 500)
                && a.mix_pct.iter().all(|&v| v <= 100)
                && a.mix_pct.iter().sum::<u32>() > 0
                && (1..=50).contains(&a.barrage_rounds)
                && fx_in(a.dig_length, 2, 1000)
                && a.dig_length <= d.max_length
                && fx_in(a.dig_offset, 0, 100)
                && (1..=600 * 30).contains(&a.throw_cooldown_ticks)
                && (1..=16).contains(&a.frag_group_men)
                && a.mix_vehicle_pct <= 100
                && a.mix_at_pct <= 100
                && fx_in(a.tank_standoff, 1, 2000)
                && fx_in(a.assault_close, 1, 500)
                && (1..=600 * 30).contains(&a.at_clear_ticks)
                && fx_in(a.repair_clear, 1, 2000),
            "rules.ai",
            "out of range",
        );
        check(
            (1..=600 * 30).contains(&r.items.resupply_ticks),
            "rules.items.resupply_ticks",
            "out of range",
        );
        check(
            r.capture_extra_man_pct <= 1000,
            "rules.capture_extra_man_pct",
            "out of range",
        );
        check(
            (1..=10_000).contains(&m.ditch_cost_pct),
            "rules.movement.ditch_cost_pct",
            "out of range",
        );
        // S3b tuning tables: the same ranges as the defs.
        let mut tuning = Vec::new();
        check_tuning(
            &mut tuning,
            "rules.",
            TuningRefs {
                body: &c.body,
                mortar: &c.mortar,
                blast: &c.blast,
                suppression: &c.suppression,
                suppress: &c.suppress,
                ai_infantry: &a.infantry,
                ai_armour: &a.armour,
                ai_assault: &a.assault,
                driving: &v.driving,
                vehicle_path: &v.path,
                escort: &v.escort,
                gunnery: &v.gunnery,
                bail: &v.bail,
                recrew: &v.recrew,
                vehicle_damage: &v.damage,
                formation: &m.formation,
                cover: &r.cover,
                throw: &r.items.throw,
                dig_common: &r.dig.common,
                call_in: &r.call_in,
            },
        );
        r.at_ditch
            .common
            .check("rules.at_ditch.common", &mut tuning);
        for i in &tuning {
            check(false, &i.path, &i.message);
        }
        for it in &self.items {
            check(
                it.range_min >= Fixed::ZERO
                    && it.range_min < it.range_max
                    && fx_in(it.range_max, 1, 100),
                "items.range",
                "out of range",
            );
            check(
                it.windup_ticks <= 10 * 30 && it.fuse_ticks <= 30 * 30 && it.scatter_pct <= 100,
                "items.values",
                "out of range",
            );
            let he = matches!(it.kind, ItemKind::Frag | ItemKind::Satchel);
            check(
                he == it.he.is_some() && (it.kind == ItemKind::Smoke) == it.smoke.is_some(),
                "items.effect",
                "frag and satchel items need a blast, smoke items a cloud",
            );
            if let Some(h) = &it.he {
                check(item_he_ok(h), "items.he", "out of range");
            }
            if let Some(m) = &it.smoke {
                check(
                    m.radius > Fixed::ZERO
                        && fx_in(m.radius, 0, 50)
                        && m.grow_ticks <= 60 * 30
                        && m.hold_ticks <= 300 * 30
                        && m.fade_ticks <= 60 * 30,
                    "items.smoke",
                    "out of range",
                );
            }
        }

        for w in &self.weapons {
            check(
                w.interval_milli >= 1000,
                "weapons.interval_milli",
                "must be at least 1000 (one tick)",
            );
            check(
                fx_in(w.range_max, 1, 5000),
                "weapons.range_max",
                "out of range",
            );
            check(
                fx_in(w.muzzle_speed, 1, 3000),
                "weapons.muzzle_speed",
                "out of range",
            );
            check(
                w.range_min >= Fixed::ZERO && w.range_min < w.range_max,
                "weapons.range_min",
                "must be in 0..range_max",
            );
            check(
                (1..=1000).contains(&w.magazine)
                    && w.damage <= 10_000
                    && w.suppression <= 100
                    && w.reload_ticks <= 60 * 30
                    && w.setup_ticks <= 60 * 30
                    && w.interval_milli <= 60 * 30 * 1000
                    && w.spread >= Fixed::ZERO
                    && fx_in(w.spread, 0, 1),
                "weapons.values",
                "out of range",
            );
            if let Some(he) = &w.he {
                check(he_ok(he), "weapons.he", "out of range");
            }
            let launcher = w.class == WeaponClass::Launcher;
            check(
                (100..=1000).contains(&w.fire_reveal_pct)
                    && w.fire_reveal_ticks <= 60 * 30
                    && w.handover_ticks <= 60 * 30
                    && w.engage_still >= Fixed::ZERO
                    && w.engage_still <= w.range_max
                    && w.engage_moving >= Fixed::ZERO
                    && w.engage_moving <= w.range_max,
                "weapons.reveal_engage",
                "out of range",
            );
            check(
                if launcher {
                    w.ammo.is_some_and(|a| (a as usize) < self.ammo.len())
                        && (1..=50).contains(&w.rounds_carried)
                } else {
                    w.ammo.is_none() && w.rounds_carried == 0
                },
                "weapons.ammo",
                "launchers need a known round, other weapons none",
            );
        }
        for a in &self.ammo {
            if let Some(m) = check_ammo(a) {
                check(false, "ammo", m);
            }
        }
        let (nw, nf, nt) = (
            self.weapons.len(),
            self.factions.len(),
            self.squad_types.len(),
        );
        for v in &self.vehicles {
            if let Some(m) = check_vehicle(v, &self.weapons, &self.ammo, nf) {
                check(false, "vehicles", m);
            }
        }
        for s in &self.squad_types {
            check(
                (s.weapon as usize) < nw,
                "squad_types.weapon",
                "index out of range",
            );
            check(
                (s.faction as usize) < nf,
                "squad_types.faction",
                "index out of range",
            );
            let is_vehicle = s.role == SquadRole::Vehicle;
            if is_vehicle {
                let vehicle = s.vehicle.and_then(|v| self.vehicles.get(v as usize));
                check(
                    vehicle.is_some_and(|v| {
                        v.faction == s.faction
                            && v.cost == s.cost
                            && v.recall_cooldown_ticks == s.recall_cooldown_ticks
                            && v.initial_cooldown_ticks == s.initial_cooldown_ticks
                            && v.main_gun().weapon == s.weapon
                    }) && s.men == 0
                        && s.weapon_men == 0
                        && s.sidearm.is_none()
                        && s.loadout.is_empty()
                        && s.dig_rate_pct == 0,
                    "squad_types.vehicle",
                    "a vehicle squad must match its vehicle and have no men",
                );
            } else {
                check(
                    s.vehicle.is_none()
                        && (1..=16).contains(&s.weapon_men)
                        && (s.weapon_men >= s.men || s.sidearm.is_some())
                        && s.sidearm.is_none_or(|w| (w as usize) < nw),
                    "squad_types.weapon_men",
                    "weapon_men out of range, or men without a weapon and no sidearm",
                );
                check(
                    (1..=16).contains(&s.men),
                    "squad_types.men",
                    "must be in 1..=16",
                );
            }
            check(
                s.speed > Fixed::ZERO && fx_in(s.speed, 0, 20),
                "squad_types.speed",
                "must be in (0, 20] m/s",
            );
            check(
                s.recall_cooldown_ticks <= crate::MAX_COOLDOWN_S.saturating_mul(self.tick_hz)
                    && s.initial_cooldown_ticks
                        <= crate::MAX_INITIAL_COOLDOWN_S.saturating_mul(self.tick_hz)
                    && s.cooldown_group
                        .is_none_or(|g| (g as usize) < self.squad_types.len()),
                "squad_types.cooldown",
                "re-call cooldown, start cooldown or cooldown group out of range",
            );
            check(
                (1..=10_000).contains(&s.hp) && fx_in(s.sight, 1, 2000),
                "squad_types.hp_sight",
                "out of range",
            );
            check(
                s.spacing > Fixed::ZERO && fx_in(s.spacing, 0, 20),
                "squad_types.spacing",
                "must be in (0, 20] m",
            );
            check(
                (25..=400).contains(&s.steadiness_pct),
                "squad_types.steadiness_pct",
                "must be in 25..=400",
            );
            let mut kinds = [false; ITEM_KINDS];
            for k in &s.loadout {
                let ok = self.items.get(k.item as usize).is_some_and(|it| {
                    !std::mem::replace(&mut kinds[it.kind.index()], true)
                        && k.count <= MAX_ITEM_COUNT
                        && k.leader_count <= MAX_ITEM_COUNT
                });
                check(
                    ok,
                    "squad_types.loadout",
                    "unknown item, a kind twice, or too many",
                );
            }
        }
        for f in &self.factions {
            check(
                f.roster
                    .iter()
                    .chain(&f.start_squads)
                    .all(|&i| (i as usize) < nt),
                "factions.roster",
                "index out of range",
            );
            // S3c: crews are never bought or fielded at the start.
            check(
                f.roster.iter().chain(&f.start_squads).all(|&i| {
                    self.squad_types
                        .get(i as usize)
                        .is_none_or(|t| t.role != SquadRole::Crew)
                }),
                "factions.roster",
                "a crew squad cannot be bought or start on the field",
            );
        }
        for v in &self.vehicles {
            check(
                v.crew_squad.is_none_or(|c| {
                    self.squad_types
                        .get(c as usize)
                        .is_some_and(|t| t.role == SquadRole::Crew && t.faction == v.faction)
                }),
                "vehicles.crew_squad",
                "not a crew squad of the vehicle's faction",
            );
        }
        check(
            (2..=MAX_MATCH_PLAYERS).contains(&self.players.len()),
            "players",
            "need 2..=10 players",
        );
        for p in &self.players {
            check(
                (p.faction as usize) < nf,
                "players.faction",
                "index out of range",
            );
            check(
                (p.base as usize) < self.map.bases.len(),
                "players.base",
                "index out of range",
            );
        }
        out
    }

    /// Index of the squad type with def id `id`.
    pub fn squad_type_index(&self, id: &str) -> Option<u16> {
        self.squad_types
            .iter()
            .position(|s| s.id == id)
            .map(|i| i as u16)
    }
}
