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

use std::collections::BTreeMap;

use crate::defs::{
    DigProfileDef, ITEM_KINDS, ItemHeDef, ItemKind, MAX_ITEM_COUNT, SquadDef, WeaponClass,
    parse_color,
};
use crate::issue::{ContentError, Issue};
use crate::modset::ModDefs;
use crate::scenario::ms_to_ticks;
use crate::tuning::{
    AiAmmoTuning, AiArmourTuning, AiAssaultTuning, AiFortTuning, AiGunsTuning, AiInfantryTuning,
    AiMortarTuning, AiReconTuning, AiSupplyTuning, AiTransportTuning, AiTruckTuning, BailTuning,
    BlastTuning, BodyTuning, CallInTuning, CoverTuning, DigCommonTuning, DrivingTuning,
    EscortTuning, FireMissionTuning, FireTuning, FormationTuning, GunneryTuning, GunsTuning,
    MortarTuning, RecrewTuning, ShareTuning, SupplyTuning, SuppressTuning, SuppressionTuning,
    ThrowTuning, TransportTuning, TuningRefs, VehicleDamageTuning, VehiclePathTuning, check_tuning,
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
    /// S4: every tag of the mod's units, sorted (cards and the AI mix
    /// index into it).
    pub tags: Vec<String>,
    /// S4: training profiles, sorted by id.
    pub training: Vec<TrainingSetup>,
    /// S5a: counted rounds whose flight lives on the weapon (small arms,
    /// mortar bombs), sorted by id.
    pub cartridges: Vec<CartridgeSetup>,
    /// S6: HQ, dump and crate defs (`defs/supply/`), sorted by id.
    pub stocks: Vec<StockSetup>,
    /// S10a: fortification blueprints (`defs/fortifications/`), sorted by
    /// id; the same for every faction.
    pub forts: Vec<FortSetup>,
}

pub use crate::defs::FortShape;

/// S10a: one lowered piece box of a blueprint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PieceSetup {
    /// Index into `defs::PIECE_PARTS`.
    pub part: u8,
    /// Half extents (along the facing, across, up).
    pub half: [Fixed; 3],
    /// Offset in the facing frame; z of the box bottom.
    pub offset: [Fixed; 3],
    pub armour_mm: u32,
    pub hp: u32,
    /// z from the undug surface (a roof over a pit).
    pub z_from_base: bool,
}

/// S10a: a wire blueprint's band, lowered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireSetup {
    pub band: Fixed,
    pub height: Fixed,
    pub hp: u32,
}

/// S10a: a lowered blueprint (`FortDef`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FortSetup {
    pub id: String,
    pub name: String,
    pub shape: FortShape,
    pub pit: Option<DigProfile>,
    /// `Capability` bit index (`build_structure` or `place_wire`).
    pub capability: u8,
    /// Point: the whole work; line: per segment.
    pub materials_g: u32,
    pub work_ticks: u32,
    pub work_men: u32,
    pub limit_per_player: u32,
    pub min_spacing: Fixed,
    pub max_slope_pct: u32,
    pub segment: Fixed,
    pub max_length: Fixed,
    pub pieces: Vec<PieceSetup>,
    pub wire: Option<WireSetup>,
    /// S10b minefields: the `mine` item laid (index into
    /// `MatchSetup::items`).
    pub mine_item: Option<u16>,
    /// S10b obstacles: the pieces block vehicle path nodes.
    pub blocks_vehicles: bool,
}

/// S10b: what sets a mine off (`[mine]`), lowered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MineSetup {
    /// Index into `defs::MINE_TRIGGERS` (0 foot, 1 vehicle).
    pub trigger: u8,
    pub trigger_cm: u32,
}

/// S10a: a Bangalore item's charge, lowered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BangaloreSetup {
    pub length: Fixed,
    pub bursts: u8,
    pub place_ticks: u32,
}

pub use crate::defs::StockKind;

/// S6: one box of a supply def (lowered `StockBoxDef`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockBoxSetup {
    /// Index into `defs::STOCK_BOX_PARTS` (0 stock, 1 radio, 2 crate).
    pub part: u8,
    /// Half extents (along the facing, across, up).
    pub half: [Fixed; 3],
    /// Centre offset in the facing frame; z from the ground.
    pub offset: [Fixed; 3],
    pub hp: u32,
}

/// S6: a lowered supply def (an HQ, a dump or a crate).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockSetup {
    pub id: String,
    pub name: String,
    /// Index into `MatchSetup::factions`.
    pub faction: u16,
    pub kind: StockKind,
    pub cost: u32,
    pub pit: DigProfile,
    pub radius: Fixed,
    /// Stock caps in grams, by `SupplyKind::index`.
    pub cap_g: [u32; 3],
    pub rebuild_ticks: u32,
    /// Tag indices (into `MatchSetup::tags`) that may build it.
    pub built_by: Vec<u16>,
    /// Crate: its kind and grams.
    pub supply: Option<SupplyKind>,
    pub g: u32,
    pub carry_speed_pct: u32,
    /// Index into `MatchSetup::ammo`: the cook-off burst.
    pub cookoff: Option<u16>,
    pub boxes: Vec<StockBoxSetup>,
}

impl MatchSetup {
    /// S6: faction `f`'s def of `kind` (crates: of supply kind `supply`).
    pub fn stock_of(&self, f: u16, kind: StockKind, supply: Option<SupplyKind>) -> Option<u16> {
        self.stocks
            .iter()
            .position(|d| {
                d.faction == f && d.kind == kind && (kind != StockKind::Crate || d.supply == supply)
            })
            .map(|i| i as u16)
    }
}

/// S5a: most distinct feeds (cartridge or ammo ids) one squad type uses
/// (the size of the sim's per-man round counts).
pub const MAX_FEEDS: usize = 4;

pub use crate::defs::SupplyKind;

/// S5a: a round's supply kind and weight (grams, packaging included).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupplySetup {
    pub kind: SupplyKind,
    pub g: u32,
}

/// S5a: what a weapon's shot takes from a man's rounds: a cartridge
/// (index into [`MatchSetup::cartridges`]) or an ammo def (index into
/// [`MatchSetup::ammo`]).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Feed {
    Cartridge(u16),
    Ammo(u16),
}

/// S5a: a lowered cartridge def.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartridgeSetup {
    pub id: String,
    pub name: String,
    pub kind: SupplyKind,
    pub g: u32,
}

impl MatchSetup {
    /// Index of tag `name` in [`Self::tags`].
    pub fn tag(&self, name: &str) -> Option<u16> {
        self.tags.iter().position(|t| t == name).map(|i| i as u16)
    }

    /// Whether squad type `t` carries tag `name`.
    pub fn type_has_tag(&self, t: u16, name: &str) -> bool {
        self.squad_types
            .get(t as usize)
            .is_some_and(|ty| ty.has_tag(self.tag(name)))
    }

    /// The presenter's legacy `type` word of squad type `t` (S4: derived
    /// from its tags in the order crew, vehicle, at, mortar, engineer, mg,
    /// line; `other` for none of them).
    pub fn type_word(&self, t: u16) -> &'static str {
        let Some(ty) = self.squad_types.get(t as usize) else {
            return "other";
        };
        for (tag, word) in [
            ("crew", "crew"),
            ("vehicle", "vehicle"),
            ("at", "at"),
            ("mortar", "mortar"),
            ("engineer", "engineer"),
            ("mg", "mg"),
            ("line", "rifle"),
        ] {
            if ty.has_tag(self.tag(tag)) {
                return word;
            }
        }
        if ty.vehicle.is_some() {
            "vehicle"
        } else {
            "other"
        }
    }
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
    /// S6: the pit profiles (legacy rules: the trench's).
    pub mortar_pit: DigSetup,
    pub hq_pit: DigSetup,
    pub dump_pit: DigSetup,
    /// S9: an AT gun pit.
    pub gun_pit: DigSetup,
    /// S10a: the MG nest's and the dugout's pits.
    pub mg_nest_pit: DigSetup,
    pub dugout_pit: DigSetup,
    /// S10b: the bunker's pit.
    pub bunker_pit: DigSetup,
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
    /// S4: `[limit_groups]`, sorted by id.
    pub limit_groups: Vec<LimitGroupSetup>,
    /// S5a: `[supply]`; `None` (legacy content): unlimited rounds, free
    /// refills (S5a-22).
    pub supply: Option<SupplyTuning>,
    /// S5b: `[fire_mission]`; `None` (legacy content): missions run as the
    /// old barrage (S5b-1 (A)).
    pub fire_mission: Option<FireMissionTuning>,
    /// S10a: `[fortify]`; `None`: nothing can be built.
    pub fortify: Option<crate::tuning::FortTuning>,
}

impl RulesSetup {
    /// The profile a dig order names.
    pub fn dig_profile(&self, p: DigProfile) -> &DigSetup {
        match p {
            DigProfile::Trench => &self.dig,
            DigProfile::AtDitch => &self.at_ditch,
            DigProfile::MortarPit => &self.mortar_pit,
            DigProfile::HqPit => &self.hq_pit,
            DigProfile::DumpPit => &self.dump_pit,
            DigProfile::GunPit => &self.gun_pit,
            DigProfile::MgNestPit => &self.mg_nest_pit,
            DigProfile::DugoutPit => &self.dugout_pit,
            DigProfile::BunkerPit => &self.bunker_pit,
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
    /// S6: a mortar pit (rectangle, all-round parapet).
    MortarPit = 2,
    /// S6: the HQ's pit, dug before the HQ stands.
    HqPit = 3,
    /// S6: an ammunition dump's pit.
    DumpPit = 4,
    /// S9: an AT gun pit (all-round spoil, engineers only).
    GunPit = 5,
    /// S10a: the MG nest's pit (engineers only).
    MgNestPit = 6,
    /// S10a: the dugout's pit (engineers only).
    DugoutPit = 7,
    /// S10b: the bunker's pit (engineers only).
    BunkerPit = 8,
}

impl DigProfile {
    /// S6: whether this is a pit (dug by `Build`, or a short `Dig` line).
    pub fn is_pit(self) -> bool {
        matches!(
            self,
            DigProfile::MortarPit
                | DigProfile::HqPit
                | DigProfile::DumpPit
                | DigProfile::GunPit
                | DigProfile::MgNestPit
                | DigProfile::DugoutPit
                | DigProfile::BunkerPit
        )
    }
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
    /// S8a `[vehicles.transport]`.
    pub transport: TransportTuning,
    /// S9 `[vehicles.guns]`.
    pub guns: GunsTuning,
}

/// Thrown-item rules (see `ItemRulesDef`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemRulesSetup {
    /// A man in his own base gets one item back every this many ticks.
    pub resupply_ticks: u32,
    /// S4: a squad fires at most one single-shot AT item (Panzerfaust)
    /// this often.
    pub single_shot_gap_ticks: u32,
    /// S4: crew-weapon pickup reach (m; `items.crew_pickup_reach_m`).
    pub crew_pickup_reach_m: u32,
    /// S3b tuning table (TOML units).
    pub throw: ThrowTuning,
}

/// AI commander tuning (see `AiDef`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiSetup {
    pub retreat_strength_pct: u32,
    pub threat_radius: Fixed,
    /// S4: the target army mix by tag (`[ai.mix]`, or the legacy
    /// `mix_*_pct` keys), percent; the legacy tags first in their old
    /// order (line, mg, engineer, mortar, at, vehicle), then the others by
    /// name. `at` counts after the vehicle unlock once an enemy vehicle was
    /// seen, `vehicle` after the unlock under the vehicle limit.
    pub mix: Vec<MixSetup>,
    /// Rounds per AI mission (`[ai.mortar] mission_rounds`, or the legacy
    /// `ai.barrage_rounds`).
    pub barrage_rounds: u16,
    pub dig_length: Fixed,
    pub dig_offset: Fixed,
    /// A squad throws at most one item this often.
    pub throw_cooldown_ticks: u32,
    pub frag_group_men: u32,
    pub tank_standoff: Fixed,
    pub assault_close: Fixed,
    pub at_clear_ticks: u32,
    pub repair_clear: Fixed,
    /// S3b tuning tables (TOML units).
    pub infantry: AiInfantryTuning,
    pub armour: AiArmourTuning,
    pub assault: AiAssaultTuning,
    /// S5a: `[ai.ammo]`.
    pub ammo: AiAmmoTuning,
    /// S5b: `[ai.mortar]` (or the legacy values).
    pub mortar: AiMortarTuning,
    /// S6: `[ai.supply]` (or the S6 starting values).
    pub supply: AiSupplyTuning,
    /// S8a: `[ai.transport]` (or the S8a starting values).
    pub transport: AiTransportTuning,
    /// S8b: `[ai.recon]` (or the S8b starting values).
    pub recon: AiReconTuning,
    /// S8b: `[ai.truck]` (or the S8b starting values).
    pub truck: AiTruckTuning,
    /// S9: `[ai.guns]` (or the S9 starting values).
    pub guns: AiGunsTuning,
    /// S10a-ai: `[ai.fort]` (or the S10a-ai starting values).
    pub fort: AiFortTuning,
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
    /// S6: spoil all round (a pit), not on the front only.
    pub all_round: bool,
    /// S6: a pit's dug line length (outside length minus the floor and
    /// both walls; zero for line profiles).
    pub pit_line: Fixed,
    /// The spoil bank shape and digger reach (the rules' `[dig.common]`,
    /// the same for every profile; S3b).
    pub common: DigCommonTuning,
}

/// S6: a pit profile of the rules, else (legacy rules) the trench's.
fn pit_setup(
    dig: &crate::defs::DigDef,
    pick: impl Fn(&crate::defs::DigProfilesDef) -> Option<&DigProfileDef>,
) -> DigSetup {
    DigSetup::lower(
        pick(&dig.profiles).unwrap_or(&dig.profiles.trench),
        &dig.common,
    )
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
            all_round: d.all_round,
            pit_line: Fixed::from_cm(d.pit_length_cm.saturating_sub(d.floor_cm + 2 * d.wall_cm)),
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
    /// S5a: `[combat.share]`.
    pub share: ShareTuning,
    /// S5a: `[combat.fire]`.
    pub fire: FireTuning,
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
    /// S4: a crew weapon (picked up when its carrier falls).
    pub crew: bool,
    /// S4: squad-card name.
    pub short_name: String,
    /// S5a: what each shot takes from the man's rounds (`None`: launchers,
    /// cannons and legacy content).
    pub feed: Option<Feed>,
    /// S5a: rounds of `feed` a man carrying it has by default (the def's
    /// `load`, else `magazine`).
    pub load: u32,
    /// S5a: MG bursts: rounds per burst (0: none) and the pause after one
    /// (ticks). Used from Part B.
    pub burst_rounds: u32,
    pub burst_pause_ticks: u32,
    /// S5a: the weapon's own fire-discipline share of its range (%; 0: the
    /// rules' `combat.fire.hold_beyond_pct`).
    pub hold_beyond_pct: u32,
    /// S5b: high-angle fire (`[indirect]`); `None`: the S1 flight rule.
    pub indirect: Option<IndirectSetup>,
    /// S8a (6.4): small arms against plates: mm RHA beaten at 0 deg by
    /// range (rows rising; empty: never), and the core past the plate.
    pub pen: Vec<crate::vehicle::PenRowSetup>,
    pub core: Option<crate::vehicle::CoreSetup>,
}

/// S5b: a mortar's charges and laying limits, lowered. The tangents are
/// computed once at load (here, in f64, rounded to [`Fixed`]), so the sim's
/// charge solver needs no trigonometry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndirectSetup {
    /// Muzzle velocity per charge (m/s), strictly rising.
    pub charges: Vec<Fixed>,
    /// `tan` of the lowest and highest elevation.
    pub tan_min: Fixed,
    pub tan_max: Fixed,
}

impl IndirectSetup {
    /// Lowers an `[indirect]` def.
    pub fn lower(d: &crate::defs::IndirectDef) -> IndirectSetup {
        let tan = |deg: u32| {
            let t = (deg as f64).to_radians().tan();
            Fixed((t * 65536.0).round() as i32)
        };
        IndirectSetup {
            charges: d
                .charges_m_s
                .iter()
                .map(|&v| Fixed::from_int(v as i32))
                .collect(),
            tan_min: tan(d.elevation_deg[0]),
            tan_max: tan(d.elevation_deg[1]),
        }
    }

    /// Range checks for a loaded setup (saves are untrusted).
    pub fn ok(&self) -> bool {
        let c = &self.charges;
        (1..=crate::defs::MAX_CHARGES).contains(&c.len())
            && c.iter()
                .all(|v| (Fixed::from_int(20)..=Fixed::from_int(600)).contains(v))
            && c.windows(2).all(|p| p[0] < p[1])
            // tan 45 deg = 1 .. tan 89 deg = 57.29
            && Fixed::ONE <= self.tan_min
            && self.tan_min < self.tan_max
            && self.tan_max <= Fixed::from_int(58)
    }
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
    /// S8a thin-plate rule: mm RHA a fragment pierces at the burst.
    pub frag_pen_mm: u32,
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
    /// S4 single-shot AT items: the launcher weapon it fires as (index
    /// into `MatchSetup::weapons`); `windup_ticks` is the aim time.
    pub weapon: Option<u16>,
    /// S5a: weight and supply kind (priced refill).
    pub supply: Option<SupplySetup>,
    /// S10a: Bangalore items' charge.
    pub bangalore: Option<BangaloreSetup>,
    /// S10b: mine items' trigger.
    pub mine: Option<MineSetup>,
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

/// S4 member traits (bit set in [`MemberSetup::traits`]).
pub mod member_trait {
    /// The squad leader's slot (first slot only).
    pub const LEADER: u8 = 1;
    /// Serves a crew weapon (the gunner).
    pub const CREW_GUNNER: u8 = 2;
    /// Picks a fallen gunner's crew weapon up first.
    pub const CREW_ASSISTANT: u8 = 4;
    /// Picks a fallen gunner's crew weapon up first (after assistants
    /// tie on distance).
    pub const AMMO_BEARER: u8 = 8;
    /// S5b: observes fire missions at the long `observe_observer_m` (FO
    /// teams; binoculars and training).
    pub const OBSERVER: u8 = 16;
    /// Trait names in bit order.
    pub const NAMES: [(&str, u8); 5] = [
        ("leader", LEADER),
        ("crew_gunner", CREW_GUNNER),
        ("crew_assistant", CREW_ASSISTANT),
        ("ammo_bearer", AMMO_BEARER),
        ("observer", OBSERVER),
    ];
    /// Every trait bit.
    pub const ALL: u8 = LEADER | CREW_GUNNER | CREW_ASSISTANT | AMMO_BEARER | OBSERVER;
}

/// S4 capabilities: what a unit can do; orders and sim rules follow them
/// (bit `Capability as u32` of [`SquadTypeSetup::caps`]).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Capability {
    /// Dig (trench profile).
    DigBasic = 0,
    /// Dig profiles that need it (the AT ditch).
    DigFull = 1,
    /// Fire missions (a member carries a mortar; S5b, was `Barrage`).
    FireMission = 2,
    /// Board an own abandoned vehicle as untrained crew.
    Recrew = 3,
    /// Trained vehicle crew: may capture, dismissed in base.
    VehicleCrew = 4,
    /// A launcher member or a single-shot AT item in the kit.
    AtHunt = 5,
    /// S9: a trained gun crew (no untrained penalties at a served gun).
    GunCrew = 6,
    /// S10a: build fortifications (field engineers).
    BuildStructure = 7,
    /// S10a: construction engineers (construction tier).
    BuildHeavy = 8,
    /// S10a: lay barbed wire.
    PlaceWire = 9,
    /// S10a: cut barbed wire.
    ClearWire = 10,
    /// S10a: assault engineers (Bangalore torpedoes; assault tier).
    Breach = 11,
    /// S10b: lay minefields.
    PlaceMines = 12,
    /// S10b: sweep and lift mines.
    ClearMines = 13,
    /// S10b: demolish obstacles and pieces with a satchel.
    Demolish = 14,
}

impl Capability {
    pub const ALL: [Capability; 15] = [
        Capability::DigBasic,
        Capability::DigFull,
        Capability::FireMission,
        Capability::Recrew,
        Capability::VehicleCrew,
        Capability::AtHunt,
        Capability::GunCrew,
        Capability::BuildStructure,
        Capability::BuildHeavy,
        Capability::PlaceWire,
        Capability::ClearWire,
        Capability::Breach,
        Capability::PlaceMines,
        Capability::ClearMines,
        Capability::Demolish,
    ];

    #[inline]
    pub const fn bit(self) -> u32 {
        1 << self as u32
    }

    /// The TOML name (`crate::variant::CAPABILITIES`).
    pub const fn name(self) -> &'static str {
        crate::variant::CAPABILITIES[self as usize]
    }

    /// Every capability bit.
    pub const MASK: u32 = (1 << 15) - 1;
}

/// S4: one `[[members]]` slot, lowered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberSetup {
    /// Slot name (squad card).
    pub slot: String,
    /// Men in the slot (1..=16).
    pub count: u8,
    /// Index into [`MatchSetup::weapons`]: what each man carries.
    pub weapon: u16,
    /// Index into [`MatchSetup::weapons`]: what he fights with after he
    /// hands a crew weapon on (parsed and kept; S4 men hand nothing on).
    pub sidearm: Option<u16>,
    /// Items each man of the slot carries, by [`ItemKind::index`]; which
    /// item of a kind is [`SquadTypeSetup::items`].
    pub items: [u8; ITEM_KINDS],
    /// [`member_trait`] bits.
    pub traits: u8,
    /// S5a: rounds each man of the slot carries when full, counting those
    /// in his weapon, by the squad type's feed slot
    /// ([`SquadTypeSetup::feeds`]).
    pub rounds: [u16; MAX_FEEDS],
}

/// S4: a training profile, percent multipliers (100 = regular; see
/// `crate::variant::TrainingDef`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingSetup {
    pub id: String,
    pub name: String,
    pub spread_pct: u32,
    pub aim_time_pct: u32,
    pub reload_pct: u32,
    pub throw_scatter_pct: u32,
    pub steadiness_pct: u32,
    pub suppression_decay_pct: u32,
    pub spot_pct: u32,
    pub build_rate_pct: u32,
}

impl TrainingSetup {
    /// Whether every multiplier is 100.
    pub fn is_regular(&self) -> bool {
        [
            self.spread_pct,
            self.aim_time_pct,
            self.reload_pct,
            self.throw_scatter_pct,
            self.steadiness_pct,
            self.suppression_decay_pct,
            self.spot_pct,
            self.build_rate_pct,
        ]
        .iter()
        .all(|&v| v == 100)
    }
}

/// S4: a limit group of the rules (`[limit_groups]`): alive units of all
/// its cards count against `limit`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitGroupSetup {
    pub id: String,
    pub limit: u32,
}

/// S4: one `[ai.mix]` entry: a tag (index into [`MatchSetup::tags`]) and
/// its target share of bought squads in percent.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixSetup {
    pub tag: u16,
    pub pct: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SquadTypeSetup {
    pub id: String,
    pub name: String,
    /// S4: HUD tile name (`short_name`, else `name`).
    pub short_name: String,
    /// Index into [`MatchSetup::factions`].
    pub faction: u16,
    /// S4: the men by slot, in spawn and formation order; the first man
    /// of the first slot leads. Empty for vehicle squads.
    pub members: Vec<MemberSetup>,
    /// Men: the sum of the slot counts (0 for vehicle squads).
    pub men: u8,
    /// The squad's main weapon (index into [`MatchSetup::weapons`]): a
    /// vehicle's main gun; else the first crew weapon among the members
    /// (launcher, mortar, MG), else the leader's weapon.
    pub weapon: u16,
    /// S4: the focus weapon (squad focus range): the longest-reaching
    /// non-crew weapon among the members (ties: the earlier slot), else
    /// `weapon` (S4-23).
    pub small_arms: u16,
    /// S4: which item of each kind the men carry (index into
    /// [`MatchSetup::items`] by [`ItemKind::index`]).
    pub items: [Option<u16>; ITEM_KINDS],
    pub cost: u32,
    /// Ticks after a call-in before this card can be called in again, and
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
    /// S3: the vehicle (index into `MatchSetup::vehicles`) for vehicle
    /// squads. Those have no members, the main gun as `weapon`, and the
    /// vehicle's (or the card's) cost, cooldowns, speed and sight;
    /// [`Self::pop`] is the vehicle's.
    pub vehicle: Option<u16>,
    /// S9: the tow's card (index into `MatchSetup::squad_types`), bought
    /// and spawned with this gun card (`cost` already includes its price).
    pub tow_card: Option<u16>,
    /// S4: tags (indices into [`MatchSetup::tags`], ascending).
    pub tags: Vec<u16>,
    /// S4: [`Capability`] bits.
    pub caps: u32,
    /// S4: training profile (index into [`MatchSetup::training`]).
    pub training: u16,
    /// S4: most squads of this card alive at once per player; 0 = no cap.
    pub max_alive: u16,
    /// S4: limit group (index into [`RulesSetup::limit_groups`]).
    pub limit_group: Option<u16>,
    /// S4: the AI's weight among ready cards of the same mix tag.
    pub ai_weight: u32,
    /// S5a: the distinct feeds the men use (at most [`MAX_FEEDS`], in
    /// first-use order over the members: weapon, sidearm, then the rest of
    /// the slot's `rounds`).
    pub feeds: Vec<Feed>,
    /// S5a: the largest slot load of each feed (load validation).
    pub feed_cap: [u16; MAX_FEEDS],
}

impl MatchSetup {
    /// S5a: whether feed `f` names a cartridge or a round with `supply`.
    pub fn feed_ok(&self, f: Feed) -> bool {
        match f {
            Feed::Cartridge(c) => (c as usize) < self.cartridges.len(),
            Feed::Ammo(a) => self
                .ammo
                .get(a as usize)
                .is_some_and(|a| a.supply.is_some()),
        }
    }
}

impl SquadTypeSetup {
    /// S5a: the feed slot (index into [`Self::feeds`]) of weapon `w`.
    pub fn feed_slot(&self, w: &WeaponSetup) -> Option<usize> {
        let f = w.feed?;
        self.feeds.iter().position(|&x| x == f)
    }

    /// Population it uses: its men, or its vehicle's `pop` (S3).
    pub fn pop(&self, vehicles: &[VehicleSetup]) -> u32 {
        match self.vehicle {
            Some(v) => vehicles.get(v as usize).map_or(0, |v| v.pop),
            None => self.men as u32,
        }
    }

    /// Whether the card has capability `c` (S4).
    #[inline]
    pub fn has(&self, c: Capability) -> bool {
        self.caps & c.bit() != 0
    }

    /// Whether the card carries tag `tag` (index into `MatchSetup::tags`).
    #[inline]
    pub fn has_tag(&self, tag: Option<u16>) -> bool {
        tag.is_some_and(|t| self.tags.contains(&t))
    }

    /// The slot (index into `members`) of the man spawned `k`-th; men
    /// past the last slot belong to it.
    pub fn slot_of(&self, k: usize) -> u8 {
        let mut n = 0usize;
        for (s, m) in self.members.iter().enumerate() {
            n += m.count as usize;
            if k < n {
                return s as u8;
            }
        }
        self.members.len().saturating_sub(1) as u8
    }

    /// The item of `kind` the men carry, with its setup.
    pub fn kit<'a>(&self, items: &'a [ItemSetup], kind: ItemKind) -> Option<(u16, &'a ItemSetup)> {
        let i = self.items[kind.index()]?;
        Some((i, items.get(i as usize)?))
    }

    /// Items a man of slot `slot` carries when full, by [`ItemKind`].
    pub fn full_kit(&self, slot: u8) -> [u8; ITEM_KINDS] {
        self.members
            .get(slot as usize)
            .map_or([0; ITEM_KINDS], |m| m.items)
    }

    /// Items of `kind` the whole squad carries when full.
    pub fn squad_items(&self, kind: ItemKind) -> u32 {
        self.members
            .iter()
            .map(|m| m.items[kind.index()] as u32 * m.count as u32)
            .sum()
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
                    frag_pen_mm: h.frag_pen_mm,
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
                crew: w.is_crew_weapon(),
                short_name: w.short_name.clone().unwrap_or_else(|| w.name.clone()),
                feed: w.feed.as_deref().and_then(|f| self.feed_of(f)),
                load: w.load.unwrap_or(w.magazine),
                burst_rounds: w.burst_rounds,
                burst_pause_ticks: ms_to_ticks(w.burst_pause_ms, hz),
                hold_beyond_pct: w.hold_beyond_pct,
                indirect: w.indirect.as_ref().map(IndirectSetup::lower),
                pen: w
                    .penetration
                    .iter()
                    .map(|&[r, mm]| crate::vehicle::PenRowSetup {
                        range: Fixed::from_int(r as i32),
                        mm,
                    })
                    .collect(),
                core: w.core.map(|c| crate::vehicle::CoreSetup {
                    damage: c.damage,
                    length: Fixed::from_cm(c.length_cm),
                    max_boxes: c.max_boxes as u8,
                }),
            })
            .collect();
        let cartridge_ids: Vec<&String> = self.cartridges.keys().collect();
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
                weapon: it
                    .weapon
                    .as_ref()
                    .and_then(|w| index_of(self.weapons.keys(), w)),
                supply: it.supply.as_ref().map(|sp| SupplySetup {
                    kind: sp.kind(),
                    g: sp.g,
                }),
                bangalore: it.bangalore.as_ref().map(|b| BangaloreSetup {
                    length: Fixed::from_cm(b.length_cm),
                    bursts: b.bursts.min(8) as u8,
                    place_ticks: b.place_s.saturating_mul(hz),
                }),
                mine: it.mine.as_ref().map(|m| MineSetup {
                    trigger: crate::defs::MINE_TRIGGERS
                        .iter()
                        .position(|t| *t == m.trigger)
                        .unwrap_or(0) as u8,
                    trigger_cm: m.trigger_cm,
                }),
            })
            .collect();
        // S4: what the sim cannot run (unknown limit groups, mixed items
        // of one kind) is refused here.
        let refused = self.card_refusals(rules);
        if !refused.is_empty() {
            return Err(invalid(refused));
        }
        let ammo: Vec<AmmoSetup> = self.ammo.values().map(lower_ammo).collect();
        let mut vehicles = self.lower_vehicles(hz);
        // S5a: rare rounds, `rare_cap` and the rack price factor (D3, D4).
        for v in &mut vehicles {
            for m in &mut v.mounts {
                for a in &mut m.ammo {
                    a.rare = ammo.get(a.ammo as usize).is_some_and(|x| x.rare);
                }
            }
            let def = self.vehicles.get(&v.id);
            let any_rare = v
                .mounts
                .first()
                .is_some_and(|m| m.ammo.iter().any(|a| a.rare));
            let cap_max = rules.supply.as_ref().map_or(10, |sp| sp.rare_cap_max);
            v.rare_cap = def
                .and_then(|d| d.rare_cap)
                .unwrap_or(if any_rare { cap_max } else { 0 });
            v.rack_price_pct = match &rules.supply {
                None => 100,
                Some(sp) => {
                    let full: u64 = v.mounts.first().map_or(0, |m| {
                        m.ammo
                            .iter()
                            .map(|a| {
                                let g = ammo
                                    .get(a.ammo as usize)
                                    .and_then(|x| x.supply)
                                    .map_or(0, |x| x.g);
                                a.count as u64 * g as u64 * sp.shell_mp_per_t as u64
                            })
                            .sum()
                    });
                    let ceiling = (v.cost as u64 * sp.rack_refill_max_card_pct as u64 * 1_000_000)
                        .checked_div(full)
                        .unwrap_or(u64::MAX);
                    (sp.rack_rearm_price_pct as u64).min(ceiling) as u32
                }
            };
        }
        let weapon_idx = |id: &str| index_of(self.weapons.keys(), id).unwrap_or(0);
        // Cooldown groups (S3b): a dense index in sorted id order.
        let groups: std::collections::BTreeSet<&str> = self
            .squads
            .values()
            .filter_map(|s| s.cooldown_group.as_deref())
            .collect();
        // Abstract units (S4 templates) never reach the match.
        let group_of = |s: &crate::SquadDef| {
            s.cooldown_group
                .as_deref()
                .and_then(|g| groups.iter().position(|x| *x == g))
                .map(|k| k as u16)
        };
        // S4: tags (cards and the AI mix), training profiles, limit groups.
        let mix_by_tag = rules.ai.mix_by_tag();
        let tags: Vec<String> = {
            let mut t: std::collections::BTreeSet<String> = self
                .squads
                .values()
                .filter(|s| !s.is_abstract)
                .flat_map(|s| s.tags.iter().cloned())
                .collect();
            t.extend(mix_by_tag.keys().cloned());
            t.into_iter().collect()
        };
        let tag_idx = |t: &str| tags.iter().position(|x| x == t).map(|i| i as u16);
        let training: Vec<TrainingSetup> = if self.training.is_empty() {
            vec![TrainingSetup {
                id: String::new(),
                name: "Regular".into(),
                spread_pct: 100,
                aim_time_pct: 100,
                reload_pct: 100,
                throw_scatter_pct: 100,
                steadiness_pct: 100,
                suppression_decay_pct: 100,
                spot_pct: 100,
                build_rate_pct: 100,
            }]
        } else {
            self.training
                .values()
                .map(|t| TrainingSetup {
                    id: t.id.clone(),
                    name: t.name.clone(),
                    spread_pct: t.spread_pct,
                    aim_time_pct: t.aim_time_pct,
                    reload_pct: t.reload_pct,
                    throw_scatter_pct: t.throw_scatter_pct,
                    steadiness_pct: t.steadiness_pct,
                    suppression_decay_pct: t.suppression_decay_pct,
                    spot_pct: t.spot_pct,
                    build_rate_pct: t.build_rate_pct,
                })
                .collect()
        };
        let default_training = rules.training.as_ref().map(|t| t.default.clone());
        let training_idx = |s: &SquadDef| -> u16 {
            let want = s.training.clone().or_else(|| default_training.clone());
            want.and_then(|w| training.iter().position(|t| t.id == w))
                .or_else(|| training.iter().position(|t| t.is_regular()))
                .unwrap_or(0) as u16
        };
        let limit_idx = |g: &str| {
            rules
                .limit_groups
                .keys()
                .position(|x| x == g)
                .map(|i| i as u16)
        };
        let caps_of = |s: &SquadDef| -> u32 {
            s.capabilities
                .iter()
                .filter_map(|c| Capability::ALL.iter().find(|k| k.name() == c))
                .fold(0, |a, k| a | k.bit())
        };
        let tags_of = |s: &SquadDef| -> Vec<u16> {
            let mut v: Vec<u16> = s.tags.iter().filter_map(|t| tag_idx(t)).collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        let mut squad_types: Vec<SquadTypeSetup> = self
            .squads
            .values()
            .filter(|s| !s.is_abstract)
            .map(|s| {
                let faction = index_of(self.factions.keys(), &s.faction).unwrap_or(0);
                let cooldown_group = group_of(s);
                let vehicle = s
                    .vehicle
                    .as_ref()
                    .and_then(|v| index_of(self.vehicles.keys(), v));
                let short_name = s.short_name.clone().unwrap_or_else(|| s.name.clone());
                if let Some(v) = vehicle {
                    let vs = &vehicles[v as usize];
                    let gun = vs.primary_weapon();
                    // S4-15 (A): the card may override the vehicle def's
                    // cost, cooldown and unlock.
                    return SquadTypeSetup {
                        id: s.id.clone(),
                        name: s.name.clone(),
                        short_name,
                        faction,
                        members: Vec::new(),
                        men: 0,
                        weapon: gun,
                        small_arms: gun,
                        items: [None; ITEM_KINDS],
                        cost: if s.cost != 0 { s.cost } else { vs.cost },
                        recall_cooldown_ticks: s
                            .cooldown_s
                            .map_or(vs.recall_cooldown_ticks, |c| c * hz),
                        initial_cooldown_ticks: s
                            .unlock_s
                            .map_or(vs.initial_cooldown_ticks, |c| c * hz),
                        cooldown_group,
                        hp: s.hp,
                        speed: vs.mobility.speed,
                        sight: vs.sight,
                        spacing: Fixed::ONE,
                        dig_rate_pct: 0,
                        steadiness_pct: crate::defs::DEFAULT_STEADINESS_PCT,
                        vehicle: Some(v),
                        tow_card: s
                            .tow
                            .as_ref()
                            .and_then(|t| index_of(self.playable_ids(), t)),
                        tags: tags_of(s),
                        caps: caps_of(s),
                        training: training_idx(s),
                        max_alive: s.max_alive,
                        limit_group: s.limit_group.as_deref().and_then(limit_idx),
                        ai_weight: s.ai_weight.unwrap_or(crate::variant::DEFAULT_AI_WEIGHT),
                        feeds: Vec::new(),
                        feed_cap: [0; MAX_FEEDS],
                    };
                }
                // S4: the members, lowered (legacy cards were expanded into
                // `members` by the loader, exactly).
                let mut kit_items: [Option<u16>; ITEM_KINDS] = [None; ITEM_KINDS];
                let members: Vec<MemberSetup> = s
                    .members
                    .iter()
                    .map(|m| {
                        let mut items = [0u8; ITEM_KINDS];
                        for (id, &n) in &m.items {
                            if n == 0 {
                                continue;
                            }
                            if let (Some(i), Some(d)) =
                                (index_of(self.items.keys(), id), self.items.get(id))
                            {
                                items[d.kind.index()] = n;
                                kit_items[d.kind.index()].get_or_insert(i);
                            }
                        }
                        let traits = m
                            .traits
                            .iter()
                            .filter_map(|t| {
                                member_trait::NAMES
                                    .iter()
                                    .find(|(n, _)| n == t)
                                    .map(|(_, b)| *b)
                            })
                            .fold(0u8, |a, b| a | b);
                        MemberSetup {
                            slot: m.slot.clone(),
                            count: m.count,
                            weapon: weapon_idx(&m.weapon),
                            sidearm: m.sidearm.as_deref().map(weapon_idx),
                            items,
                            traits,
                            rounds: [0; MAX_FEEDS],
                        }
                    })
                    .collect();
                // S5a: feeds in first-use order and each slot's rounds.
                let (feeds, members, feed_cap) =
                    self.lower_member_rounds(s, &weapons, &cartridge_ids, members);
                let men: u32 = members.iter().map(|m| m.count as u32).sum();
                let crew_w = |w: u16| weapons.get(w as usize).is_some_and(|w| w.crew);
                let main = members
                    .iter()
                    .map(|m| m.weapon)
                    .find(|&w| crew_w(w))
                    .or_else(|| members.first().map(|m| m.weapon))
                    .unwrap_or(0);
                // The focus weapon: the longest-reaching non-crew weapon
                // (ties: the earlier slot), else the main weapon (S4-23).
                let small_arms = members
                    .iter()
                    .map(|m| m.weapon)
                    .filter(|&w| !crew_w(w))
                    .fold(None::<u16>, |best, w| match best {
                        Some(b)
                            if weapons[b as usize].range_max >= weapons[w as usize].range_max =>
                        {
                            Some(b)
                        }
                        _ => Some(w),
                    })
                    .unwrap_or(main);
                SquadTypeSetup {
                    id: s.id.clone(),
                    name: s.name.clone(),
                    short_name,
                    faction,
                    members,
                    men: men.min(u8::MAX as u32) as u8,
                    weapon: main,
                    small_arms,
                    items: kit_items,
                    cost: s.cost,
                    recall_cooldown_ticks: s.card_cooldown_s() * hz,
                    initial_cooldown_ticks: s.card_unlock_s() * hz,
                    cooldown_group,
                    hp: s.hp,
                    speed: Fixed::from_cm(s.speed_cm_s),
                    sight: Fixed::from_int(s.sight_m as i32),
                    spacing: Fixed::from_ratio(s.spacing_dm as i64, 10),
                    dig_rate_pct: s.dig_rate_pct,
                    steadiness_pct: s.steadiness_pct,
                    vehicle: None,
                    tow_card: None,
                    tags: tags_of(s),
                    caps: caps_of(s),
                    training: training_idx(s),
                    max_alive: s.max_alive,
                    limit_group: s.limit_group.as_deref().and_then(limit_idx),
                    ai_weight: s.ai_weight.unwrap_or(crate::variant::DEFAULT_AI_WEIGHT),
                    feeds,
                    feed_cap,
                }
            })
            .collect();
        // S9 (S9-2 (A)): a gun card with a tow pays gun + tow.
        for i in 0..squad_types.len() {
            if let Some(t) = squad_types[i].tow_card {
                let tow_cost = squad_types[t as usize].cost;
                squad_types[i].cost += tow_cost;
            }
        }
        let squad_idx = |id: &String| index_of(self.playable_ids(), id).unwrap_or(0);
        // S4: the match date drops cards it is outside of from the rosters.
        let date = crate::variant::parse_month(&rules.call_in.date);
        let fielded = |id: &String| {
            let Some(d) = date else { return true };
            self.squads.get(id).is_none_or(|s| {
                let m = |v: &Option<String>| v.as_deref().and_then(crate::variant::parse_month);
                m(&s.available_from).is_none_or(|a| a <= d)
                    && m(&s.available_to).is_none_or(|b| d <= b)
            })
        };
        let mut date_issues = Vec::new();
        let factions: Vec<FactionSetup> = self
            .factions
            .values()
            .map(|f| {
                for (i, s) in f.start_squads.iter().enumerate() {
                    if !fielded(s) {
                        date_issues.push(Issue::new(
                            format!("faction `{}`: start_squads[{i}]", f.id),
                            format!(
                                "squad `{s}` is not fielded on the match date {}",
                                rules.call_in.date
                            ),
                        ));
                    }
                }
                FactionSetup {
                    id: f.id.clone(),
                    name: f.name.clone(),
                    color_rgb: parse_color(&f.color).unwrap_or(0),
                    roster: f
                        .roster
                        .iter()
                        .filter(|s| fielded(s))
                        .map(squad_idx)
                        .collect(),
                    start_squads: f.start_squads.iter().map(squad_idx).collect(),
                }
            })
            .collect();
        if !date_issues.is_empty() {
            return Err(invalid(date_issues));
        }
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
        let stocks: Vec<StockSetup> = self
            .supply
            .values()
            .map(|d| lower_stock(d, &self.factions, &tags, &self.ammo, hz))
            .collect();
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
                mortar_pit: pit_setup(&rules.dig, |p| p.mortar_pit.as_ref()),
                hq_pit: pit_setup(&rules.dig, |p| p.hq_pit.as_ref()),
                dump_pit: pit_setup(&rules.dig, |p| p.dump_pit.as_ref()),
                gun_pit: pit_setup(&rules.dig, |p| p.gun_pit.as_ref()),
                mg_nest_pit: pit_setup(&rules.dig, |p| p.mg_nest_pit.as_ref()),
                dugout_pit: pit_setup(&rules.dig, |p| p.dugout_pit.as_ref()),
                bunker_pit: pit_setup(&rules.dig, |p| p.bunker_pit.as_ref()),
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
                        share: c.share.clone().unwrap_or_default(),
                        fire: c.fire.clone().unwrap_or_default(),
                    }
                },
                items: ItemRulesSetup {
                    resupply_ticks: rules.items.resupply_s * hz,
                    single_shot_gap_ticks: rules.items.single_shot_gap_s * hz,
                    crew_pickup_reach_m: rules.items.crew_pickup_reach_m,
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
                        transport: v.transport.clone(),
                        guns: v.guns.clone(),
                    }
                },
                difficulty: DifficultyTable {
                    easy: diff(&rules.difficulty.easy),
                    normal: diff(&rules.difficulty.normal),
                    hard: diff(&rules.difficulty.hard),
                },
                ai: {
                    let a = &rules.ai;
                    // S4: legacy tags first in their old order, then the
                    // others by name (`AiSetup::mix`).
                    const LEGACY: [&str; 6] = ["line", "mg", "engineer", "mortar", "at", "vehicle"];
                    let mut mix: Vec<MixSetup> = Vec::new();
                    for t in LEGACY {
                        if let (Some(&pct), Some(tag)) = (mix_by_tag.get(t), tag_idx(t)) {
                            mix.push(MixSetup { tag, pct });
                        }
                    }
                    for (t, &pct) in &mix_by_tag {
                        if !LEGACY.contains(&t.as_str())
                            && let Some(tag) = tag_idx(t)
                        {
                            mix.push(MixSetup { tag, pct });
                        }
                    }
                    AiSetup {
                        retreat_strength_pct: a.retreat_strength_pct,
                        threat_radius: Fixed::from_int(a.threat_radius_m as i32),
                        mix,
                        barrage_rounds: a.mortar_tuning().mission_rounds as u16,
                        dig_length: Fixed::from_int(a.dig_length_m as i32),
                        dig_offset: Fixed::from_int(a.dig_offset_m as i32),
                        throw_cooldown_ticks: a.throw_cooldown_s * hz,
                        frag_group_men: a.frag_group_men,
                        tank_standoff: Fixed::from_int(a.tank_standoff_m as i32),
                        assault_close: Fixed::from_int(a.assault_close_m as i32),
                        at_clear_ticks: a.at_clear_s * hz,
                        repair_clear: Fixed::from_int(a.repair_clear_m as i32),
                        infantry: a.infantry.clone(),
                        armour: a.armour.clone(),
                        assault: a.assault.clone(),
                        ammo: a.ammo.clone().unwrap_or_default(),
                        mortar: a.mortar_tuning(),
                        supply: a.supply.clone().unwrap_or_default(),
                        transport: a.transport.clone().unwrap_or_default(),
                        recon: a.recon.clone().unwrap_or_default(),
                        truck: a.truck.clone().unwrap_or_default(),
                        guns: a.guns.clone().unwrap_or_default(),
                        fort: a.fort.clone().unwrap_or_default(),
                    }
                },
                cover: rules.cover.clone(),
                call_in: rules.call_in.tuning(),
                limit_groups: rules
                    .limit_groups
                    .iter()
                    .map(|(id, &limit)| LimitGroupSetup {
                        id: id.clone(),
                        limit,
                    })
                    .collect(),
                supply: rules.supply.clone(),
                fire_mission: rules.fire_mission.clone(),
                fortify: rules.fortify.clone(),
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
            tags,
            training,
            cartridges: self
                .cartridges
                .values()
                .map(|c| CartridgeSetup {
                    id: c.id.clone(),
                    name: c.name.clone(),
                    kind: c.supply.kind(),
                    g: c.supply.g,
                })
                .collect(),
            stocks,
            forts: self
                .forts
                .values()
                .map(|d| lower_fort(d, hz, self.items.keys()))
                .collect(),
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
    /// Ids of the units that reach a match (all but abstract ones), sorted:
    /// the order of [`MatchSetup::squad_types`].
    pub fn playable_ids(&self) -> impl Iterator<Item = &String> {
        self.squads
            .iter()
            .filter(|(_, s)| !s.is_abstract)
            .map(|(id, _)| id)
    }

    /// S4: cards the sim cannot run, refused at resolve (the loader and
    /// `validate` accept them): an unknown limit group, and two different
    /// items of one kind in one squad.
    fn card_refusals(&self, rules: &crate::defs::RulesDef) -> Vec<Issue> {
        let mut out = Vec::new();
        for (id, s) in self.squads.iter().filter(|(_, s)| !s.is_abstract) {
            if let Some(g) = &s.limit_group
                && !rules.limit_groups.contains_key(g)
            {
                out.push(
                    Issue::new(
                        format!("squad `{id}`: limit_group"),
                        format!("the rules `{}` have no limit group `{g}`", rules.id),
                    )
                    .with_hint(format!("[limit_groups] \"{g}\" = 2")),
                );
            }
            let mut kinds: [Option<&str>; ITEM_KINDS] = [None; ITEM_KINDS];
            for m in &s.members {
                for (item, &n) in &m.items {
                    let Some(d) = self.items.get(item) else {
                        continue;
                    };
                    if n == 0 {
                        continue;
                    }
                    match kinds[d.kind.index()] {
                        Some(o) if o != item => out.push(
                            Issue::new(
                                format!("squad `{id}`: members"),
                                format!(
                                    "items `{o}` and `{item}` are both {:?}; one squad carries one \
                                     item of each kind",
                                    d.kind
                                ),
                            )
                            .with_hint("give every member the same item of that kind"),
                        ),
                        _ => kinds[d.kind.index()] = Some(item),
                    }
                }
            }
        }
        out
    }

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
                    .and_then(|c| index_of(self.playable_ids(), c));
                // S8b: the truck's crate defs, per kind.
                if let (Some(t), Some(d)) = (out.supply.as_mut(), v.supply.as_ref()) {
                    for c in &d.crates {
                        let kind = self
                            .supply
                            .get(c)
                            .and_then(|x| x.supply.as_deref().and_then(SupplyKind::from_name));
                        if let Some(kind) = kind {
                            t.crates[kind.index()] = index_of(self.supply.keys(), c);
                        }
                    }
                }
                out
            })
            .collect()
    }

    /// Every ammo def lowered, sorted by id.
    pub fn lower_ammo(&self) -> Vec<AmmoSetup> {
        self.ammo.values().map(lower_ammo).collect()
    }

    /// S5a: the feed a `feed = "<id>"` names: a cartridge, else an ammo def
    /// (the loader's cross-check reports unknown ids).
    pub fn feed_of(&self, id: &str) -> Option<Feed> {
        index_of(self.cartridges.keys(), id)
            .map(Feed::Cartridge)
            .or_else(|| index_of(self.ammo.keys(), id).map(Feed::Ammo))
    }

    /// S5a: the feeds of unit `s` in first-use order (weapon, sidearm, then
    /// the slot's other `rounds` ids, slot by slot), each slot's rounds by
    /// feed slot, and the largest slot load per feed. Feeds past
    /// [`MAX_FEEDS`] are dropped (the loader refuses such units).
    pub(crate) fn lower_member_rounds(
        &self,
        s: &SquadDef,
        weapons: &[WeaponSetup],
        _cartridges: &[&String],
        mut members: Vec<MemberSetup>,
    ) -> (Vec<Feed>, Vec<MemberSetup>, [u16; MAX_FEEDS]) {
        let mut feeds: Vec<Feed> = Vec::new();
        let mut cap = [0u16; MAX_FEEDS];
        for (md, ms) in s.members.iter().zip(members.iter_mut()) {
            let mut want: Vec<(Feed, u32)> = Vec::new();
            let add = |f: Feed, n: u32, want: &mut Vec<(Feed, u32)>| {
                if !want.iter().any(|(x, _)| *x == f) {
                    want.push((f, n));
                }
            };
            // Explicit `rounds` first (they override `load`), but feeds are
            // ordered weapon, sidearm, then the rest.
            let explicit: Vec<(Feed, u32)> = md
                .rounds
                .iter()
                .filter_map(|(id, &n)| self.feed_of(id).map(|f| (f, n)))
                .collect();
            let own = |w: u16| weapons.get(w as usize).and_then(|w| w.feed.map(|f| (f, w)));
            for w in [Some(ms.weapon), ms.sidearm].into_iter().flatten() {
                if let Some((f, wd)) = own(w) {
                    let n = explicit
                        .iter()
                        .find(|(x, _)| *x == f)
                        .map(|(_, n)| *n)
                        .or_else(|| {
                            // The S4 integer form: rounds of his weapon's feed.
                            (w == ms.weapon).then_some(md.ammo).flatten()
                        })
                        .unwrap_or(wd.load);
                    add(f, n, &mut want);
                }
            }
            for (f, n) in explicit {
                add(f, n, &mut want);
            }
            for (f, n) in want {
                let k = match feeds.iter().position(|&x| x == f) {
                    Some(k) => k,
                    None if feeds.len() < MAX_FEEDS => {
                        feeds.push(f);
                        feeds.len() - 1
                    }
                    None => continue,
                };
                let n = n.min(u16::MAX as u32) as u16;
                ms.rounds[k] = n;
                cap[k] = cap[k].max(n);
            }
        }
        (feeds, members, cap)
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
            frag_pen_mm: h.frag_pen_mm,
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
        && b.frag_pen_mm <= 100
}

pub(crate) fn item_he_ok(h: &ItemHeSetup) -> bool {
    h.damage <= 10_000 && h.suppression <= 100 && he_ok(&h.blast)
}

/// Position of `id` among sorted map keys.
/// S6: lowers one supply def (references checked by `check_stocks`).
fn lower_stock(
    d: &crate::defs::StockDef,
    factions: &BTreeMap<String, crate::defs::FactionDef>,
    tags: &[String],
    ammo: &BTreeMap<String, crate::vehicle::AmmoDef>,
    hz: u32,
) -> StockSetup {
    let mut cap_g = [0u32; 3];
    for (k, v) in &d.cap_kg {
        if let Some(kind) = SupplyKind::from_name(k) {
            cap_g[kind.index()] = v.saturating_mul(1000);
        }
    }
    let cm = |v: i32| Fixed::from_ratio(v as i64, 100);
    StockSetup {
        id: d.id.clone(),
        name: d.name.clone(),
        faction: index_of(factions.keys(), &d.faction).unwrap_or(0),
        kind: d.kind,
        cost: d.cost,
        pit: d.pit.unwrap_or(DigProfile::HqPit),
        radius: Fixed::from_int(d.radius_m as i32),
        cap_g,
        rebuild_ticks: d.rebuild_s.saturating_mul(hz),
        built_by: d
            .built_by
            .iter()
            .filter_map(|t| tags.iter().position(|x| x == t).map(|i| i as u16))
            .collect(),
        supply: d.supply.as_deref().and_then(SupplyKind::from_crate_name),
        g: d.kg.saturating_mul(1000),
        carry_speed_pct: d.carry_speed_pct,
        cookoff: d.cookoff.as_ref().and_then(|c| index_of(ammo.keys(), c)),
        boxes: d
            .boxes
            .iter()
            .map(|b| StockBoxSetup {
                part: crate::defs::STOCK_BOX_PARTS
                    .iter()
                    .position(|p| *p == b.part)
                    .unwrap_or(0) as u8,
                half: [
                    cm(b.size_cm[0] as i32 / 2),
                    cm(b.size_cm[1] as i32 / 2),
                    cm(b.size_cm[2] as i32 / 2),
                ],
                offset: [cm(b.offset_cm[0]), cm(b.offset_cm[1]), cm(b.offset_cm[2])],
                hp: b.hp,
            })
            .collect(),
    }
}

/// S10a: lowers one blueprint.
fn lower_fort<'a>(
    d: &crate::defs::FortDef,
    hz: u32,
    items: impl Iterator<Item = &'a String>,
) -> FortSetup {
    let cm = |v: i32| Fixed::from_ratio(v as i64, 100);
    FortSetup {
        id: d.id.clone(),
        name: d.name.clone(),
        shape: d.shape,
        pit: d.pit,
        capability: crate::variant::CAPABILITIES
            .iter()
            .position(|c| *c == d.capability)
            .unwrap_or(Capability::BuildStructure as usize) as u8,
        materials_g: d.materials_kg.saturating_mul(1000),
        work_ticks: d.work_s.saturating_mul(hz),
        work_men: d.work_men,
        limit_per_player: d.limit_per_player,
        min_spacing: Fixed::from_int(d.min_spacing_m as i32),
        max_slope_pct: d.max_slope_pct,
        segment: Fixed::from_cm(d.segment_cm),
        max_length: Fixed::from_int(d.max_length_m as i32),
        pieces: d
            .pieces
            .iter()
            .map(|b| PieceSetup {
                part: crate::defs::PIECE_PARTS
                    .iter()
                    .position(|p| *p == b.part)
                    .unwrap_or(0) as u8,
                half: [
                    cm(b.size_cm[0] as i32 / 2),
                    cm(b.size_cm[1] as i32 / 2),
                    cm(b.size_cm[2] as i32 / 2),
                ],
                offset: [cm(b.offset_cm[0]), cm(b.offset_cm[1]), cm(b.offset_cm[2])],
                armour_mm: b.armour_mm,
                hp: b.hp,
                z_from_base: b.z_from.as_deref() == Some("base"),
            })
            .collect(),
        wire: d.wire.as_ref().map(|w| WireSetup {
            band: Fixed::from_cm(w.band_cm),
            height: Fixed::from_cm(w.height_cm),
            hp: w.hp,
        }),
        mine_item: d.mine_item.as_deref().and_then(|m| index_of(items, m)),
        blocks_vehicles: d.blocks_vehicles,
    }
}

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
        for d in [
            &r.dig,
            &r.at_ditch,
            &r.mortar_pit,
            &r.hq_pit,
            &r.dump_pit,
            &r.gun_pit,
            &r.mg_nest_pit,
            &r.dugout_pit,
            &r.bunker_pit,
        ] {
            check(
                d.pit_line >= Fixed::ZERO && fx_in(d.pit_line, 0, 20),
                "rules.dig.pit_line",
                "out of range",
            );
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
                && a.mix
                    .iter()
                    .all(|m| m.pct <= 100 && (m.tag as usize) < self.tags.len())
                && a.mix.iter().map(|m| m.pct).sum::<u32>() > 0
                && (1..=a.mix.len())
                    .all(|k| a.mix[..k - 1].iter().all(|o| o.tag != a.mix[k - 1].tag))
                && (1..=50).contains(&a.barrage_rounds)
                && a.barrage_rounds as u32 == a.mortar.mission_rounds
                && fx_in(a.dig_length, 2, 1000)
                && a.dig_length <= d.max_length
                && fx_in(a.dig_offset, 0, 100)
                && (1..=600 * 30).contains(&a.throw_cooldown_ticks)
                && (1..=16).contains(&a.frag_group_men)
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
            r.items.single_shot_gap_ticks <= 600 * 30,
            "rules.items.single_shot_gap_ticks",
            "out of range",
        );
        check(
            (1..=1_000).contains(&r.items.crew_pickup_reach_m),
            "rules.items.crew_pickup_reach_m",
            "out of range",
        );
        check(
            r.limit_groups.len() <= u16::MAX as usize
                && r.limit_groups.iter().all(|g| g.limit <= 1000)
                && r.limit_groups.windows(2).all(|w| w[0].id < w[1].id),
            "rules.limit_groups",
            "out of range or not sorted",
        );
        check(
            !self.training.is_empty()
                && self.training.len() <= u16::MAX as usize
                && self.training.iter().all(|t| {
                    [
                        t.spread_pct,
                        t.aim_time_pct,
                        t.reload_pct,
                        t.throw_scatter_pct,
                        t.steadiness_pct,
                        t.suppression_decay_pct,
                        t.spot_pct,
                        t.build_rate_pct,
                    ]
                    .iter()
                    .all(|v| (25..=400).contains(v))
                }),
            "training",
            "need a profile; multipliers in 25..=400 %",
        );
        check(
            self.tags.len() <= u16::MAX as usize
                && self.tags.windows(2).all(|w| w[0] < w[1])
                && self.tags.iter().all(|t| !t.is_empty() && t.len() <= 24),
            "tags",
            "not sorted, or a tag out of range",
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
                transport: &v.transport,
                guns: &v.guns,
                vehicle_damage: &v.damage,
                formation: &m.formation,
                cover: &r.cover,
                throw: &r.items.throw,
                dig_common: &r.dig.common,
                call_in: &r.call_in,
                supply: r.supply.as_ref(),
                share: &c.share,
                fire: &c.fire,
                ai_ammo: &a.ammo,
                fire_mission: r.fire_mission.as_ref(),
                fortify: r.fortify.as_ref(),
                ai_mortar: &a.mortar,
                ai_supply: &a.supply,
                ai_transport: &a.transport,
                ai_recon: &a.recon,
                ai_truck: &a.truck,
                ai_guns: &a.guns,
                ai_fort: &a.fort,
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
            let he = matches!(
                it.kind,
                ItemKind::Frag | ItemKind::Satchel | ItemKind::Bangalore | ItemKind::Mine
            );
            check(
                (it.kind == ItemKind::Mine)
                    == it.mine.as_ref().is_some_and(|m| {
                        (m.trigger as usize) < crate::defs::MINE_TRIGGERS.len()
                            && (1..=200).contains(&m.trigger_cm)
                    }),
                "items.mine",
                "a mine item has a trigger; other items none",
            );
            check(
                (it.kind == ItemKind::Bangalore) == it.bangalore.is_some(),
                "items.bangalore",
                "a Bangalore item has a charge; other items none",
            );
            check(
                he == it.he.is_some() && (it.kind == ItemKind::Smoke) == it.smoke.is_some(),
                "items.effect",
                "frag and satchel items need a blast, smoke items a cloud",
            );
            check(
                (it.kind == ItemKind::SingleShotAt)
                    == it.weapon.is_some_and(|w| {
                        self.weapons
                            .get(w as usize)
                            .is_some_and(|w| w.class == WeaponClass::Launcher && w.ammo.is_some())
                    }),
                "items.weapon",
                "a single-shot AT item fires as a launcher weapon; other items name none",
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
            // S8a: the small-arms penetration table and core.
            check(
                w.pen.len() <= crate::defs::MAX_PEN_ROWS
                    && w.pen.windows(2).all(|p| p[0].range < p[1].range)
                    && w.pen.iter().all(|r| r.mm <= 100 && fx_in(r.range, 0, 5000))
                    && w.pen.is_empty() == w.core.is_none()
                    && w.core.is_none_or(|c| {
                        (1..=1000).contains(&c.damage)
                            && c.length > Fixed::ZERO
                            && fx_in(c.length, 0, 5)
                            && (1..=4).contains(&c.max_boxes)
                    }),
                "weapons.pen",
                "penetration rows or core out of range",
            );
            if let Some(ind) = &w.indirect {
                check(
                    ind.ok() && w.class == WeaponClass::Mortar && w.he.is_some(),
                    "weapons.indirect",
                    "charges or elevation limits out of range, or not a mortar",
                );
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
            // S5a.
            check(
                w.feed.is_none_or(|f| self.feed_ok(f))
                    && w.load <= u16::MAX as u32
                    && w.burst_rounds <= 1000
                    && w.burst_pause_ticks <= 10 * 30
                    && w.hold_beyond_pct <= 100,
                "weapons.feed",
                "unknown feed, or load, bursts or hold_beyond_pct out of range",
            );
        }
        for c in &self.cartridges {
            check(
                (1..=100_000).contains(&c.g) && !c.id.is_empty(),
                "cartridges",
                "weight out of range",
            );
        }
        // S6: supply defs (saves are untrusted).
        for d in &self.stocks {
            check(
                (d.faction as usize) < self.factions.len()
                    && d.cookoff.is_none_or(|a| {
                        self.ammo.get(a as usize).is_some_and(|a| a.blast.is_some())
                    })
                    && d.built_by.iter().all(|&t| (t as usize) < self.tags.len())
                    && (1..=4).contains(&d.boxes.len())
                    && d.boxes.iter().all(|b| {
                        b.part < 3
                            && (1..=10_000).contains(&b.hp)
                            && b.half.iter().all(|h| *h > Fixed::ZERO && fx_in(*h, 0, 10))
                            && b.offset
                                .iter()
                                .all(|o| (-(10 << 16)..=(10 << 16)).contains(&o.0))
                    })
                    && d.radius >= Fixed::ZERO
                    && fx_in(d.radius, 0, 100)
                    && d.cap_g.iter().all(|g| *g <= 10_000_000)
                    && d.g <= 200_000
                    && (10..=100).contains(&d.carry_speed_pct)
                    && (d.kind != StockKind::Crate || (d.supply.is_some() && d.g > 0)),
                "stocks",
                "a supply def is out of range",
            );
        }
        let supply_ok = |sp: &Option<SupplySetup>| sp.is_none_or(|x| (1..=100_000).contains(&x.g));
        check(
            self.items.iter().all(|i| supply_ok(&i.supply))
                && self.ammo.iter().all(|a| supply_ok(&a.supply)),
            "supply",
            "an item's or round's supply weight is out of range",
        );
        // (The rare rounds stowed may exceed `rare_cap` in a staged setup;
        // the loader refuses such defs and the base never refills above
        // the cap.)
        let cap_max = r.supply.as_ref().map_or(u32::MAX, |sp| sp.rare_cap_max);
        for v in &self.vehicles {
            check(
                v.rare_cap <= cap_max
                    && v.rack_price_pct <= 1000
                    && v.mounts.first().is_none_or(|m| {
                        m.ammo.iter().all(|a| {
                            self.ammo
                                .get(a.ammo as usize)
                                .is_some_and(|d| d.rare == a.rare)
                        })
                    }),
                "vehicles.rare_cap",
                "rare_cap above rare_cap_max, or a bad rack price or rare flag",
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
            // S8b: a truck's crate defs are crates of its kind and faction.
            if let Some(t) = &v.supply {
                let ok = t.crates.iter().enumerate().all(|(k, c)| {
                    c.is_none_or(|c| {
                        self.stocks.get(c as usize).is_some_and(|d| {
                            d.kind == StockKind::Crate
                                && d.faction == v.faction
                                && d.supply.map(|s| s.index()) == Some(k)
                                && d.g > 0
                        })
                    })
                });
                check(
                    ok,
                    "vehicles.supply.crates",
                    "not a crate of its kind and faction",
                );
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
            let is_vehicle = s.vehicle.is_some();
            if is_vehicle {
                let vehicle = s.vehicle.and_then(|v| self.vehicles.get(v as usize));
                check(
                    vehicle.is_some_and(|v| {
                        v.faction == s.faction
                            && v.primary_weapon() == s.weapon
                            && s.small_arms == s.weapon
                    }) && s.men == 0
                        && s.members.is_empty()
                        && s.items.iter().all(Option::is_none)
                        && s.dig_rate_pct == 0,
                    "squad_types.vehicle",
                    "a vehicle squad must match its vehicle and have no men",
                );
            } else {
                let total: u32 = s.members.iter().map(|m| m.count as u32).sum();
                check(
                    (1..=16).contains(&s.men)
                        && total == s.men as u32
                        && !s.members.is_empty()
                        && s.members.len() <= 16
                        && (s.small_arms as usize) < nw,
                    "squad_types.men",
                    "men must be in 1..=16 and equal the members' counts",
                );
                for m in &s.members {
                    check(
                        (1..=16).contains(&m.count)
                            && (m.weapon as usize) < nw
                            && m.sidearm.is_none_or(|w| (w as usize) < nw)
                            && m.traits & !member_trait::ALL == 0
                            && m.items.iter().all(|&n| n <= MAX_ITEM_COUNT)
                            && m.items
                                .iter()
                                .zip(&s.items)
                                .all(|(&n, it)| n == 0 || it.is_some()),
                        "squad_types.members",
                        "a member's count, weapon, traits or items are out of range",
                    );
                }
                let nf = s.feeds.len();
                check(
                    nf <= MAX_FEEDS
                        && s.feeds.iter().all(|&f| self.feed_ok(f))
                        && s.members.iter().all(|m| {
                            m.rounds
                                .iter()
                                .enumerate()
                                .all(|(k, &n)| n <= s.feed_cap[k] && (k < nf || n == 0))
                        })
                        && s.feed_cap[nf.min(MAX_FEEDS)..].iter().all(|&c| c == 0),
                    "squad_types.feeds",
                    "feeds or member rounds out of range",
                );
                for (k, it) in s.items.iter().enumerate() {
                    check(
                        it.is_none_or(|i| {
                            self.items
                                .get(i as usize)
                                .is_some_and(|d| d.kind.index() == k)
                        }),
                        "squad_types.items",
                        "unknown item, or an item of another kind",
                    );
                }
            }
            check(
                s.caps & !Capability::MASK == 0
                    && (s.training as usize) < self.training.len()
                    && s.tags.iter().all(|&t| (t as usize) < self.tags.len())
                    && s.tags.windows(2).all(|w| w[0] < w[1])
                    && s.tags.len() <= crate::variant::MAX_TAGS
                    && s.max_alive <= crate::variant::MAX_ALIVE_CAP
                    && s.limit_group
                        .is_none_or(|g| (g as usize) < r.limit_groups.len())
                    && s.ai_weight <= 10_000
                    && !s.short_name.is_empty(),
                "squad_types.card",
                "capabilities, training, tags, card limits or weight out of range",
            );
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
                f.roster
                    .iter()
                    .chain(&f.start_squads)
                    .all(|&i| !self.type_has_tag(i, "crew")),
                "factions.roster",
                "a crew squad cannot be bought or start on the field",
            );
        }
        // S9: a card's tow is a vehicle card of a tow vehicle of its faction
        // that can pull this gun.
        for s in &self.squad_types {
            let Some(t) = s.tow_card else { continue };
            let gun = s.vehicle.and_then(|v| self.vehicles.get(v as usize));
            let tow = self.squad_types.get(t as usize);
            let tv = tow
                .and_then(|t| t.vehicle)
                .and_then(|v| self.vehicles.get(v as usize));
            check(
                match (gun, tow, tv) {
                    (Some(g), Some(t), Some(tv)) => {
                        g.served()
                            && t.faction == s.faction
                            && tv.tow.is_some_and(|d| d.max_kg >= g.mass_kg)
                    }
                    _ => false,
                },
                "squad_types.tow",
                "a card's tow must be a vehicle card of its faction that can pull the gun",
            );
        }
        for v in &self.vehicles {
            check(
                v.crew_squad.is_none_or(|c| {
                    (self.type_has_tag(c, "crew")
                        || (v.served() && self.type_has_tag(c, "gun_crew")))
                        && self
                            .squad_types
                            .get(c as usize)
                            .is_some_and(|t| t.faction == v.faction)
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
