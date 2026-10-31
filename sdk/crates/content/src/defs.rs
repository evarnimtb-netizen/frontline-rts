//! Def files of a content mod (§7.1, §7.4): squads, weapons, factions, maps
//! and rules, one TOML file each under `defs/<kind>/`.
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

/// Map side limits (the sim's 64 m tiling).
pub const MAP_SIZE_STEP_M: u32 = 64;
pub const MAP_SIZE_MAX_M: u32 = 8192;
/// Most men in one squad.
pub const MAX_SQUAD_MEN: u8 = 16;

/// What a squad does on the field. Drives AI choices and which orders apply
/// (only squads that can dig take `Dig`, only mortars take `Barrage`).
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SquadRole {
    Rifle,
    Mg,
    Engineer,
    Mortar,
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
    /// Soldiers in the squad, 1..=16.
    pub men: u8,
    /// Weapon id every member carries.
    pub weapon: String,
    /// Manpower cost of a call-in.
    pub cost: u32,
    /// Seconds from call-in to arrival at the base.
    pub call_in_s: u32,
    /// Hit points per soldier.
    #[serde(default = "default_hp")]
    pub hp: u32,
    /// Walking speed in cm/s.
    #[schemars(range(min = 10, max = 1000))]
    pub speed_cm_s: u32,
    /// Sight range in metres.
    pub sight_m: u32,
    /// Formation spacing between men in decimetres.
    #[schemars(range(min = 5, max = 200))]
    pub spacing_dm: u32,
    /// Digging speed per man in percent of the base rate; 0 = cannot dig.
    #[serde(default)]
    pub dig_rate_pct: u32,
}

fn default_hp() -> u32 {
    100
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
    /// Dispersion half-angle in milliradians.
    pub spread_mrad: u32,
    pub muzzle_velocity_m_s: u32,
    /// Time after moving before it can fire (MG, mortar).
    #[serde(default)]
    pub setup_ms: u32,
    /// Suppression added to a soldier by one near miss (out of 100).
    pub suppression: u32,
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
    pub difficulty: DifficultyTableDef,
    pub ai: AiDef,
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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DigDef {
    pub depth_cm: u32,
    pub floor_cm: u32,
    pub parapet_cm: u32,
    pub max_length_m: u32,
    pub work_per_m: u32,
    /// The trench deepens in this many equal passes.
    pub passes: u32,
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
        range(&mut out, "men", self.men as u32, 1, MAX_SQUAD_MEN as u32);
        range(&mut out, "call_in_s", self.call_in_s, 0, 600);
        range(&mut out, "hp", self.hp, 1, 10_000);
        range(&mut out, "speed_cm_s", self.speed_cm_s, 10, 1000);
        range(&mut out, "sight_m", self.sight_m, 1, 2000);
        range(&mut out, "spacing_dm", self.spacing_dm, 5, 200);
        range(&mut out, "dig_rate_pct", self.dig_rate_pct, 0, 1000);
        range(&mut out, "cost", self.cost, 0, 100_000);
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
        let d = &self.dig;
        range(&mut out, "dig.depth_cm", d.depth_cm, 10, 1000);
        range(&mut out, "dig.floor_cm", d.floor_cm, 10, 1000);
        range(&mut out, "dig.parapet_cm", d.parapet_cm, 0, 300);
        range(&mut out, "dig.max_length_m", d.max_length_m, 1, 1000);
        range(&mut out, "dig.work_per_m", d.work_per_m, 1, 10_000_000);
        range(&mut out, "dig.passes", d.passes, 1, 32);
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
        ] {
            range(&mut out, &format!("ai.{name}"), v, lo, hi);
        }
        if a.mix_rifle_pct + a.mix_mg_pct + a.mix_engineer_pct + a.mix_mortar_pct == 0 {
            out.push(Issue::new(
                "ai.mix_rifle_pct",
                "the mix must not be all zero",
            ));
        }
        if a.dig_length_m > self.dig.max_length_m {
            out.push(Issue::new(
                "ai.dig_length_m",
                "must be at most dig.max_length_m",
            ));
        }
        out
    }
}
