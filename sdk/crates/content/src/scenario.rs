//! Scenario files: the setup of a headless battle (`simulate`, benchmarks).
//!
//! ```toml
//! name = "battalion"
//! tick_hz = 30
//! hash_interval = 30
//!
//! [map]
//! size_m = 3072
//! heightfield = { kind = "noise", amplitude_m = 14 }
//!
//! [forces]
//! players = 2
//! soldiers = 700
//!
//! [projectiles]
//! pool_cap = 1800
//! fire_interval_ms = { min = 700, max = 2500 }
//! ```
//!
//! Unknown keys are errors, so a typo never silently falls back to a default.

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::issue::{ContentError, Issue};

/// Tick rates a match may use (§2.2: 30 Hz, or 20 Hz as a wholesale swap).
pub const TICK_HZ_ALLOWED: [u32; 2] = [30, 20];

/// Map sides are whole 64 m blocks (the sim's spatial and hash tiling).
pub const MAP_SIZE_STEP_M: u32 = 64;
pub const MAP_SIZE_MAX_M: u32 = 8192;
/// Lobby player limit (§2.2).
pub const MAX_PLAYERS: u8 = 10;
/// Sanity bound well above the Regiment cap; catches unit-of-measure slips.
pub const MAX_SOLDIERS: u32 = 100_000;
pub const MAX_PROJECTILE_POOL: u32 = 1_000_000;
pub const MAX_FIRE_INTERVAL_MS: u32 = 60_000;

/// A headless battle setup.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    /// Short identifier used in reports, e.g. `battalion`.
    pub name: String,
    /// Free-form notes for humans.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Default world seed. `frontline-cli simulate --seed` overrides it.
    #[serde(default = "default_seed")]
    pub seed: u64,
    /// Sim tick rate in Hz, a match constant: 30 (default) or 20.
    #[serde(default = "default_tick_hz")]
    #[schemars(extend("enum" = [30, 20]))]
    pub tick_hz: u32,
    /// Publish the combined state hash every this many ticks.
    #[serde(default = "default_hash_interval")]
    #[schemars(range(min = 1))]
    pub hash_interval: u32,
    pub map: MapDef,
    pub forces: ForcesDef,
    #[serde(default)]
    pub projectiles: ProjectilesDef,
}

/// Map geometry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MapDef {
    /// Side of the square map in metres: a multiple of 64 in 64..=8192
    /// (3 km maps use 3072, 4 km maps 4096).
    #[schemars(range(min = 64, max = 8192), extend("multipleOf" = 64))]
    pub size_m: u32,
    #[serde(default)]
    pub heightfield: HeightfieldDef,
}

/// Heightfield generator.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HeightfieldDef {
    /// Height 0 everywhere.
    Flat,
    /// Seeded value noise.
    Noise {
        /// Terrain seed. Omit to derive the terrain from the world seed.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seed: Option<u64>,
        /// Peak height deviation from 0 in metres.
        #[serde(default = "default_amplitude_m")]
        amplitude_m: u32,
    },
}

impl Default for HeightfieldDef {
    fn default() -> Self {
        HeightfieldDef::Noise {
            seed: None,
            amplitude_m: default_amplitude_m(),
        }
    }
}

/// Who is on the field at tick 0.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForcesDef {
    /// Number of players, 1..=10. Soldiers are dealt round-robin.
    #[serde(default = "default_players")]
    #[schemars(range(min = 1, max = 10))]
    pub players: u8,
    /// Total soldiers spawned at tick 0, across all players.
    #[schemars(range(max = 100_000))]
    pub soldiers: u32,
}

/// Projectile load.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectilesDef {
    /// Projectile pool capacity. Shots fired while it is full are dropped
    /// and counted.
    #[serde(default = "default_pool_cap")]
    #[schemars(range(min = 1, max = 1_000_000))]
    pub pool_cap: u32,
    /// Time between one soldier's shots, drawn per shot from [min, max].
    /// The spawn rate is about `soldiers / mean(interval)` shots per second.
    #[serde(default = "default_fire_interval")]
    pub fire_interval_ms: IntervalMs,
}

impl Default for ProjectilesDef {
    fn default() -> Self {
        ProjectilesDef {
            pool_cap: default_pool_cap(),
            fire_interval_ms: default_fire_interval(),
        }
    }
}

/// An inclusive range in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntervalMs {
    pub min: u32,
    pub max: u32,
}

fn default_seed() -> u64 {
    1
}
fn default_tick_hz() -> u32 {
    30
}
fn default_hash_interval() -> u32 {
    30
}
fn default_amplitude_m() -> u32 {
    14
}
fn default_players() -> u8 {
    2
}
fn default_pool_cap() -> u32 {
    1800
}
fn default_fire_interval() -> IntervalMs {
    IntervalMs {
        min: 700,
        max: 2500,
    }
}

/// Milliseconds to whole ticks, rounding half up. Integer-only so every
/// platform lowers a scenario identically.
pub fn ms_to_ticks(ms: u32, tick_hz: u32) -> u32 {
    ((ms as u64 * tick_hz as u64 + 500) / 1000) as u32
}

impl Scenario {
    /// Parses TOML without semantic checks. `origin` names the source in
    /// errors (usually the file path).
    pub fn parse_toml(src: &str, origin: &str) -> Result<Scenario, ContentError> {
        toml::from_str(src).map_err(|e| ContentError::Parse {
            origin: origin.to_owned(),
            message: e.to_string().trim_end().to_owned(),
        })
    }

    /// Parses and validates TOML.
    pub fn from_toml_str(src: &str, origin: &str) -> Result<Scenario, ContentError> {
        let s = Scenario::parse_toml(src, origin)?;
        let issues = s.validate();
        if issues.is_empty() {
            Ok(s)
        } else {
            Err(ContentError::Invalid {
                origin: origin.to_owned(),
                issues,
            })
        }
    }

    /// Reads, parses and validates a scenario file.
    pub fn load(path: &Path) -> Result<Scenario, ContentError> {
        let src = std::fs::read_to_string(path).map_err(|source| ContentError::Io {
            path: path.to_owned(),
            source,
        })?;
        Scenario::from_toml_str(&src, &path.display().to_string())
    }

    /// Serialises back to TOML (round-trips through [`Scenario::parse_toml`]).
    pub fn to_toml_string(&self) -> String {
        toml::to_string_pretty(self).expect("scenario is always representable as TOML")
    }

    /// Fire interval range in ticks at this scenario's tick rate.
    pub fn fire_period_ticks(&self) -> (u32, u32) {
        let r = self.projectiles.fire_interval_ms;
        (
            ms_to_ticks(r.min, self.tick_hz),
            ms_to_ticks(r.max, self.tick_hz),
        )
    }

    /// Expected projectile spawn rate in shots per second, as a
    /// (numerator, denominator) pair: `soldiers * hz * 2 / (pmin + pmax)`.
    pub fn spawn_rate_per_s(&self) -> (u64, u64) {
        let (lo, hi) = self.fire_period_ticks();
        (
            self.forces.soldiers as u64 * self.tick_hz as u64 * 2,
            (lo + hi).max(1) as u64,
        )
    }

    /// Every semantic problem, in field order. Empty means valid.
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();

        if self.name.trim().is_empty() {
            out.push(Issue::new("name", "must not be empty").with_hint("name = \"my_scenario\""));
        }
        if !TICK_HZ_ALLOWED.contains(&self.tick_hz) {
            out.push(
                Issue::new("tick_hz", format!("must be 30 or 20, got {}", self.tick_hz))
                    .with_hint("tick_hz = 30"),
            );
        }
        if self.hash_interval == 0 {
            out.push(
                Issue::new("hash_interval", "must be at least 1 tick")
                    .with_hint("hash_interval = 30 (one hash per second at 30 Hz)"),
            );
        }

        let size = self.map.size_m;
        if !(MAP_SIZE_STEP_M..=MAP_SIZE_MAX_M).contains(&size)
            || !size.is_multiple_of(MAP_SIZE_STEP_M)
        {
            let lo =
                (size / MAP_SIZE_STEP_M * MAP_SIZE_STEP_M).clamp(MAP_SIZE_STEP_M, MAP_SIZE_MAX_M);
            let hi = (lo + MAP_SIZE_STEP_M).min(MAP_SIZE_MAX_M);
            let hint = if lo == hi || size <= lo {
                format!("size_m = {lo}")
            } else {
                format!("size_m = {lo} or size_m = {hi}")
            };
            out.push(
                Issue::new(
                    "map.size_m",
                    format!("must be a multiple of 64 in 64..=8192, got {size}"),
                )
                .with_hint(hint),
            );
        }
        if let HeightfieldDef::Noise { amplitude_m, .. } = self.map.heightfield
            && amplitude_m > 1000
        {
            out.push(
                Issue::new(
                    "map.heightfield.amplitude_m",
                    format!("must be at most 1000 m, got {amplitude_m}"),
                )
                .with_hint("amplitude_m = 14"),
            );
        }

        let f = &self.forces;
        if f.players == 0 || f.players > MAX_PLAYERS {
            out.push(
                Issue::new(
                    "forces.players",
                    format!("must be in 1..={MAX_PLAYERS}, got {}", f.players),
                )
                .with_hint("players = 2"),
            );
        }
        if f.soldiers > MAX_SOLDIERS {
            out.push(Issue::new(
                "forces.soldiers",
                format!("must be at most {MAX_SOLDIERS}, got {}", f.soldiers),
            ));
        }

        let p = &self.projectiles;
        if p.pool_cap == 0 || p.pool_cap > MAX_PROJECTILE_POOL {
            out.push(
                Issue::new(
                    "projectiles.pool_cap",
                    format!("must be in 1..={MAX_PROJECTILE_POOL}, got {}", p.pool_cap),
                )
                .with_hint("pool_cap = 1800 (Battalion) or 5000 (Regiment)"),
            );
        }
        let r = p.fire_interval_ms;
        if r.min > r.max {
            out.push(
                Issue::new(
                    "projectiles.fire_interval_ms",
                    format!("min ({}) is greater than max ({})", r.min, r.max),
                )
                .with_hint(format!(
                    "fire_interval_ms = {{ min = {}, max = {} }}",
                    r.max, r.min
                )),
            );
        }
        if TICK_HZ_ALLOWED.contains(&self.tick_hz) {
            let tick_ms = 1000_u32.div_ceil(self.tick_hz);
            if ms_to_ticks(r.min, self.tick_hz) == 0 {
                out.push(
                    Issue::new(
                        "projectiles.fire_interval_ms.min",
                        format!(
                            "must be at least one tick ({tick_ms} ms at {} Hz), got {}",
                            self.tick_hz, r.min
                        ),
                    )
                    .with_hint(format!("min = {tick_ms}")),
                );
            }
        }
        if r.max > MAX_FIRE_INTERVAL_MS {
            out.push(Issue::new(
                "projectiles.fire_interval_ms.max",
                format!("must be at most {MAX_FIRE_INTERVAL_MS} ms, got {}", r.max),
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
        name = "t"
        [map]
        size_m = 512
        [forces]
        soldiers = 10
    "#;

    #[test]
    fn minimal_file_gets_defaults() {
        let s = Scenario::from_toml_str(MINIMAL, "t.toml").unwrap();
        assert_eq!(s.tick_hz, 30);
        assert_eq!(s.hash_interval, 30);
        assert_eq!(s.seed, 1);
        assert_eq!(s.forces.players, 2);
        assert_eq!(s.projectiles, ProjectilesDef::default());
        assert_eq!(s.map.heightfield, HeightfieldDef::default());
    }

    #[test]
    fn round_trips_through_toml() {
        let mut s = Scenario::from_toml_str(MINIMAL, "t.toml").unwrap();
        s.map.heightfield = HeightfieldDef::Noise {
            seed: Some(9),
            amplitude_m: 20,
        };
        s.description = Some("x".into());
        let back = Scenario::parse_toml(&s.to_toml_string(), "rt").unwrap();
        assert_eq!(back, s);
        s.map.heightfield = HeightfieldDef::Flat;
        let back = Scenario::parse_toml(&s.to_toml_string(), "rt").unwrap();
        assert_eq!(back, s);
        // Seeds above i64::MAX serialise and parse back too (no panic).
        let big = Scenario {
            seed: u64::MAX,
            ..s
        };
        let back = Scenario::parse_toml(&big.to_toml_string(), "big").unwrap();
        assert_eq!(back, big);
    }

    #[test]
    fn unknown_keys_are_rejected_with_location() {
        let src = MINIMAL.replace("soldiers = 10", "soldier = 10");
        let e = Scenario::from_toml_str(&src, "t.toml")
            .unwrap_err()
            .to_string();
        assert!(e.contains("soldier"), "{e}");
        assert!(e.contains("line"), "{e}");

        let src = MINIMAL.replace(
            "size_m = 512",
            "size_m = 512\nheightfield = { kind = \"noise\", amp = 3 }",
        );
        let e = Scenario::from_toml_str(&src, "t.toml")
            .unwrap_err()
            .to_string();
        assert!(e.contains("amp"), "{e}");

        let src = MINIMAL.replace(
            "size_m = 512",
            "size_m = 512\nheightfield = { kind = \"hills\" }",
        );
        let e = Scenario::from_toml_str(&src, "t.toml")
            .unwrap_err()
            .to_string();
        assert!(e.contains("flat") && e.contains("noise"), "{e}");
    }

    #[test]
    fn validation_collects_every_issue_with_fixes() {
        let src = r#"
            name = ""
            tick_hz = 60
            hash_interval = 0
            [map]
            size_m = 3000
            [forces]
            players = 0
            soldiers = 10
            [projectiles]
            pool_cap = 0
            fire_interval_ms = { min = 900, max = 100 }
        "#;
        let Err(ContentError::Invalid { issues, .. }) = Scenario::from_toml_str(src, "bad.toml")
        else {
            panic!("expected validation failure");
        };
        let paths: Vec<_> = issues.iter().map(|i| i.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "name",
                "tick_hz",
                "hash_interval",
                "map.size_m",
                "forces.players",
                "projectiles.pool_cap",
                "projectiles.fire_interval_ms",
            ]
        );
        let size = &issues[3];
        assert_eq!(size.hint.as_deref(), Some("size_m = 2944 or size_m = 3008"));
    }

    #[test]
    fn fire_interval_lowering_is_integer_and_rounds_half_up() {
        assert_eq!(ms_to_ticks(700, 30), 21);
        assert_eq!(ms_to_ticks(2500, 30), 75);
        assert_eq!(ms_to_ticks(50, 30), 2); // 1.5 ticks rounds up
        assert_eq!(ms_to_ticks(16, 30), 0);
        let mut s = Scenario::from_toml_str(MINIMAL, "t").unwrap();
        s.projectiles.fire_interval_ms.min = 10;
        assert_eq!(s.validate()[0].path, "projectiles.fire_interval_ms.min");
    }

    #[test]
    fn schema_exports() {
        let docs = crate::schemas();
        assert_eq!(docs[0].name, "scenario");
        let json = serde_json::to_value(&docs[0].schema).unwrap();
        assert_eq!(json["title"], "Scenario");
        assert_eq!(json["additionalProperties"], false);
        let req = json["required"].as_array().unwrap();
        assert!(req.iter().any(|v| v == "map"));
    }
}
