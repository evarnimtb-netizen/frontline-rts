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
//! * def ids become indices into the sorted `weapons`, `squad_types` and
//!   `factions` lists (sorted by id, so indices are stable for a def set).
//!
//! [`MatchSetup::validate`] re-checks the structure without the defs, since a
//! setup also arrives inside untrusted save files.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::defs::{SquadRole, WeaponClass, parse_color};
use crate::issue::{ContentError, Issue};
use crate::modset::ModDefs;
use crate::scenario::ms_to_ticks;

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
    /// Sorted by id.
    pub squad_types: Vec<SquadTypeSetup>,
    /// Sorted by id.
    pub factions: Vec<FactionSetup>,
    /// In player-slot order (player id = index).
    pub players: Vec<PlayerSetup>,
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
    pub movement: MovementSetup,
    pub dig: DigSetup,
    pub combat: CombatSetup,
    pub difficulty: DifficultyTable,
    pub ai: AiSetup,
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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MovementSetup {
    pub slope_cost_pct: u32,
    pub max_slope_pct: u32,
    /// A soldier this close to his target counts as there (m).
    pub arrive_radius: Fixed,
    pub retreat_speed_pct: u32,
    pub catch_up_speed_pct: u32,
    pub path_budget_per_tick: u32,
    pub path_max_expansions: u32,
    /// Path step cost between two trench cells, in percent of normal.
    pub trench_cost_pct: u32,
}

/// Trench profile and digging pace. One man at `dig_rate_pct = 100` does
/// 100 work points per second.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DigSetup {
    pub depth: Fixed,
    pub floor_width: Fixed,
    pub parapet_height: Fixed,
    pub max_length: Fixed,
    pub work_per_m: u32,
    /// The trench deepens in this many equal passes.
    pub passes: u32,
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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeSetup {
    pub lethal_radius: Fixed,
    pub damage_radius: Fixed,
    pub suppression_radius: Fixed,
    pub crater_radius: Fixed,
    pub crater_depth: Fixed,
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
    pub call_in_ticks: u32,
    pub hp: u32,
    /// Walking speed (m/s).
    pub speed: Fixed,
    pub sight: Fixed,
    /// Formation spacing (m).
    pub spacing: Fixed,
    pub dig_rate_pct: u32,
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
                spread: Fixed::from_ratio(w.spread_mrad as i64, 1000),
                muzzle_speed: Fixed::from_int(w.muzzle_velocity_m_s as i32),
                setup_ticks: ms_to_ticks(w.setup_ms, hz),
                suppression: w.suppression,
                he: w.he.as_ref().map(|h| HeSetup {
                    lethal_radius: Fixed::from_cm(h.lethal_radius_cm),
                    damage_radius: Fixed::from_cm(h.damage_radius_cm),
                    suppression_radius: Fixed::from_int(h.suppression_radius_m as i32),
                    crater_radius: Fixed::from_cm(h.crater_radius_cm),
                    crater_depth: Fixed::from_cm(h.crater_depth_cm),
                }),
            })
            .collect();
        let squad_types: Vec<SquadTypeSetup> = self
            .squads
            .values()
            .map(|s| SquadTypeSetup {
                id: s.id.clone(),
                name: s.name.clone(),
                faction: index_of(self.factions.keys(), &s.faction).unwrap_or(0),
                role: s.role,
                men: s.men,
                weapon: index_of(self.weapons.keys(), &s.weapon).unwrap_or(0),
                cost: s.cost,
                call_in_ticks: s.call_in_s * hz,
                hp: s.hp,
                speed: Fixed::from_cm(s.speed_cm_s),
                sight: Fixed::from_int(s.sight_m as i32),
                spacing: Fixed::from_ratio(s.spacing_dm as i64, 10),
                dig_rate_pct: s.dig_rate_pct,
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
                movement: MovementSetup {
                    slope_cost_pct: m.slope_cost_pct,
                    max_slope_pct: m.max_slope_pct,
                    arrive_radius: Fixed::from_cm(m.arrive_cm),
                    retreat_speed_pct: m.retreat_speed_pct,
                    catch_up_speed_pct: m.catch_up_speed_pct,
                    path_budget_per_tick: m.path_budget_per_tick,
                    path_max_expansions: m.path_max_expansions,
                    trench_cost_pct: m.trench_cost_pct,
                },
                dig: DigSetup {
                    depth: Fixed::from_cm(rules.dig.depth_cm),
                    floor_width: Fixed::from_cm(rules.dig.floor_cm),
                    parapet_height: Fixed::from_cm(rules.dig.parapet_cm),
                    max_length: Fixed::from_int(rules.dig.max_length_m as i32),
                    work_per_m: rules.dig.work_per_m,
                    passes: rules.dig.passes,
                },
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
                    }
                },
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
            squad_types,
            factions,
            players,
        };
        let issues = setup.validate();
        if issues.is_empty() {
            Ok(setup)
        } else {
            Err(invalid(issues))
        }
    }
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
                && (100..=300).contains(&m.catch_up_speed_pct),
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
        let d = &r.dig;
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
                && fx_in(a.dig_offset, 0, 100),
            "rules.ai",
            "out of range",
        );

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
                check(
                    he.lethal_radius > Fixed::ZERO
                        && he.lethal_radius <= he.damage_radius
                        && fx_in(he.damage_radius, 0, 100)
                        && fx_in(he.suppression_radius, 0, 200)
                        && fx_in(he.crater_radius, 0, 20)
                        && fx_in(he.crater_depth, 0, 5),
                    "weapons.he",
                    "out of range",
                );
            }
        }
        let (nw, nf, nt) = (
            self.weapons.len(),
            self.factions.len(),
            self.squad_types.len(),
        );
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
            check(
                (1..=16).contains(&s.men),
                "squad_types.men",
                "must be in 1..=16",
            );
            check(
                s.speed > Fixed::ZERO && fx_in(s.speed, 0, 20),
                "squad_types.speed",
                "must be in (0, 20] m/s",
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
