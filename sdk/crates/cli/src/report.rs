//! The JSON report written by `simulate --report`.
//!
//! Hashes are hex strings (`0x` + 16 digits) because JSON numbers lose
//! precision above 2^53 in most readers. Times are milliseconds.

use std::time::Duration;

use frontline_content::Scenario;
use frontline_sim::{SimConfig, TerrainKind, World};
use serde::Serialize;

/// Report format version; bump on breaking changes to the shape.
pub const REPORT_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub report_version: u32,
    pub tool: String,
    /// The scenario as parsed (with defaults filled in), when one was given.
    pub scenario: Option<Scenario>,
    /// The sim configuration actually run.
    pub config: ConfigEcho,
    pub run: RunInfo,
    pub final_tick: u64,
    pub final_hash: String,
    /// Every hash the sim published during the run (every `hash_interval`).
    pub hashes: Vec<TickHash>,
    pub timing: Timing,
    /// Present only with `--paced`.
    pub paced: Option<PacedStats>,
    pub entities: EntityCounts,
    pub stats: SimStats,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TickHash {
    pub tick: u64,
    pub hash: String,
}

pub fn hex(h: u64) -> String {
    format!("{h:#018x}")
}

#[derive(Clone, Debug, Serialize)]
pub struct ConfigEcho {
    pub seed: u64,
    pub tick_hz: i32,
    /// 1/60 s ballistic substeps per tick.
    pub substeps: u32,
    pub hash_interval: u32,
    pub map_size_m: i32,
    pub terrain: &'static str,
    pub soldiers: u32,
    pub players: u8,
    pub projectile_cap: u32,
    /// Ticks a soldier waits between shots, `[min, max]`.
    pub fire_cooldown_ticks: [u16; 2],
    /// Expected steady spawn rate: soldiers / mean fire period.
    pub expected_shots_per_s: f64,
}

impl ConfigEcho {
    pub fn new(c: &SimConfig) -> ConfigEcho {
        let hz = c.tick_rate.hz();
        let mean_period = (c.weapon.cooldown_min as f64 + c.weapon.cooldown_max as f64) / 2.0 + 1.0;
        ConfigEcho {
            seed: c.seed,
            tick_hz: hz,
            substeps: c.tick_rate.substeps(),
            hash_interval: c.hash_interval,
            map_size_m: c.map_size_m,
            terrain: match c.terrain {
                TerrainKind::Flat => "flat",
                TerrainKind::Noise => "noise",
            },
            soldiers: c.soldiers,
            players: c.players,
            projectile_cap: c.projectile_cap,
            fire_cooldown_ticks: [c.weapon.cooldown_min, c.weapon.cooldown_max],
            expected_shots_per_s: round3(c.soldiers as f64 * hz as f64 / mean_period),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct RunInfo {
    pub threads: usize,
    /// Ticks stepped in this run.
    pub ticks: u64,
    pub start_tick: u64,
    pub end_tick: u64,
    pub paced: bool,
    pub loaded_from: Option<String>,
    pub saved: Option<SavedInfo>,
    /// Wall time of the tick loop (includes pacing sleeps).
    pub wall_ms: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SavedInfo {
    pub tick: u64,
    pub path: String,
    pub bytes: usize,
    pub hash: String,
    pub projectiles_live: usize,
}

/// mean / P95 / P99 / max of a set of durations, in ms.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Summary {
    pub mean_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
}

impl Summary {
    /// Nearest-rank percentiles. Sorts `ns` in place.
    pub fn from_nanos(ns: &mut [u64]) -> Summary {
        if ns.is_empty() {
            return Summary::default();
        }
        ns.sort_unstable();
        let n = ns.len();
        let rank = |p: usize| ns[(p * n).div_ceil(100).clamp(1, n) - 1];
        let ms = |v: u64| round3(v as f64 / 1e6);
        let sum: u128 = ns.iter().map(|&v| v as u128).sum();
        Summary {
            mean_ms: round3(sum as f64 / n as f64 / 1e6),
            p95_ms: ms(rank(95)),
            p99_ms: ms(rank(99)),
            max_ms: ms(ns[n - 1]),
        }
    }
}

/// Per-stage wall-clock times (§5.2 order), measured by the sim each tick.
#[derive(Clone, Debug, Default, Serialize)]
pub struct StageSummaries {
    pub intent: Summary,
    pub perception: Summary,
    pub movement: Summary,
    pub weapons: Summary,
    pub ballistics: Summary,
    pub damage: Summary,
    pub status: Summary,
    pub world_hash: Summary,
    pub snapshot: Summary,
    /// Sum of the stages.
    pub total: Summary,
}

#[derive(Clone, Debug, Serialize)]
pub struct Timing {
    pub stages: StageSummaries,
    /// Whole `World::step` call, timed outside the sim.
    pub tick: Summary,
}

/// Tick-time statistics for a paced (real tick rate) run.
#[derive(Clone, Debug, Serialize)]
pub struct PacedStats {
    pub period_ms: f64,
    /// Ticks whose compute time exceeded the period.
    pub overruns: u64,
    /// Ticks that started more than 1 ms after their deadline.
    pub late_starts: u64,
    pub max_lateness_ms: f64,
    /// The §2.2 SCS tier gate: P95 ≤ 13.5 ms and P99 ≤ 18 ms.
    pub gate: Gate,
}

#[derive(Clone, Debug, Serialize)]
pub struct Gate {
    pub p95_limit_ms: f64,
    pub p99_limit_ms: f64,
    pub pass: bool,
}

impl Gate {
    pub const P95_MS: f64 = 13.5;
    pub const P99_MS: f64 = 18.0;

    pub fn check(tick: &Summary) -> Gate {
        Gate {
            p95_limit_ms: Self::P95_MS,
            p99_limit_ms: Self::P99_MS,
            pass: tick.p95_ms <= Self::P95_MS && tick.p99_ms <= Self::P99_MS,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct EntityCounts {
    pub soldiers: usize,
    pub entity_chunks: usize,
    pub id_high_water: u32,
    pub projectiles_live: usize,
    pub projectile_capacity: usize,
    /// Highest live projectile count seen at the end of any tick in the run.
    pub projectiles_peak: usize,
    pub projectiles_mean: f64,
}

/// Sim counters at the end of the run (cumulative since tick 0).
#[derive(Clone, Debug, Serialize)]
pub struct SimStats {
    pub shots_fired: u64,
    pub shots_dropped: u64,
    pub ground_hits: u64,
    pub expired: u64,
    pub out_of_bounds: u64,
    pub commands_applied: u64,
    pub commands_rejected: u64,
}

impl SimStats {
    pub fn new(w: &World) -> SimStats {
        let s = w.stats();
        SimStats {
            shots_fired: s.shots_fired,
            shots_dropped: s.shots_dropped,
            ground_hits: s.ground_hits,
            expired: s.expired,
            out_of_bounds: s.out_of_bounds,
            commands_applied: s.commands_applied,
            commands_rejected: s.commands_rejected,
        }
    }
}

pub(crate) fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

pub(crate) fn ns(d: Duration) -> u64 {
    d.as_nanos().min(u64::MAX as u128) as u64
}
