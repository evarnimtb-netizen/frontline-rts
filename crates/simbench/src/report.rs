//! Machine description, §2.2 M0 gate comparison and text output.

use std::fmt::Write as _;

use serde::Serialize;

use crate::los::LosReport;
use crate::orca::OrcaResult;
use crate::tick::{TickResult, Tier};

pub const SCHEMA: &str = "frontline-simbench/1";

/// §2.2 M0 gate limits.
pub mod gates {
    /// Target hardware names, as used in [`super::GateCheck::target_hardware`].
    pub const DECK: &str = "Steam Deck";
    pub const RYZEN_3600: &str = "Ryzen 5 3600";
    /// §2.2 sim worker counts: Deck 3, 3600 4.
    pub const DECK_THREADS: usize = 3;
    pub const RYZEN_3600_THREADS: usize = 4;
    /// Deck LOS, CPU µs per ray with 3 threads (tiled + max-mip + occluder BVH).
    pub const DECK_LOS_US_PER_RAY: f64 = 2.9;
    /// Deck ORCA, CPU µs per agent.
    pub const DECK_ORCA_US_PER_AGENT: f64 = 0.9;
    /// Composed paced tick: P95 / P99 ms (Deck at Battalion, 3600 at Regiment).
    pub const TICK_P95_MS: f64 = 13.5;
    pub const TICK_P99_MS: f64 = 18.0;
}

#[derive(Clone, Debug, Serialize)]
pub struct Machine {
    pub cpu: String,
    pub os: &'static str,
    pub arch: &'static str,
    pub logical_cpus: usize,
    pub is_gate_hardware: bool,
    /// Which §2.2 gate device this is (`gates::DECK` / `gates::RYZEN_3600`).
    pub gate_device: Option<&'static str>,
    /// 1-minute load average when the run started (None if unavailable).
    /// Other work on the machine skews tail latencies badly.
    pub load_avg_1m: Option<f64>,
}

impl Machine {
    pub fn detect(label: Option<String>) -> Machine {
        let cpu = label.or_else(cpu_brand).unwrap_or_else(|| "unknown".into());
        let lower = cpu.to_lowercase();
        let gate_device = gate_device(&lower);
        let is_gate_hardware = gate_device.is_some();
        Machine {
            cpu,
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            logical_cpus: std::thread::available_parallelism().map_or(1, |n| n.get()),
            is_gate_hardware,
            gate_device,
            load_avg_1m: load_avg_1m(),
        }
    }

    /// Warning text if the machine was busy at start (load > half the CPUs).
    pub fn load_warning(&self) -> Option<String> {
        let load = self.load_avg_1m?;
        (load > self.logical_cpus as f64 / 2.0).then(|| {
            format!(
                "machine busy at start (load average {load:.1} on {} CPUs): tail latencies are not trustworthy",
                self.logical_cpus
            )
        })
    }

    pub fn baseline_notice(&self) -> String {
        if self.is_gate_hardware {
            format!(
                "Measured on {}: a §2.2 gate device. The GPU-load scene and presenter are still not running.",
                self.cpu
            )
        } else {
            format!(
                "BASELINE ONLY: measured on {} ({}/{}), not a Steam Deck or Ryzen 5 3600. \
                 Pass/fail below is against the Deck/3600 limits and does not decide the gate.",
                self.cpu, self.os, self.arch
            )
        }
    }
}

/// Only the Deck APU and a plain Ryzen 5 3600 (not the faster 3600X/XT) are
/// gate hardware. CPU detection on Windows returns "unknown"; pass
/// `--machine` there.
fn gate_device(cpu_lower: &str) -> Option<&'static str> {
    if cpu_lower.contains("custom apu") {
        return Some(gates::DECK);
    }
    let i = cpu_lower.find("ryzen 5 3600")?;
    let rest = &cpu_lower[i + "ryzen 5 3600".len()..];
    (!rest.starts_with(|c: char| c.is_ascii_alphanumeric())).then_some(gates::RYZEN_3600)
}

fn load_avg_1m() -> Option<f64> {
    #[cfg(target_os = "macos")]
    {
        // "{ 1.23 2.34 3.45 }"
        let out = std::process::Command::new("sysctl")
            .args(["-n", "vm.loadavg"])
            .output()
            .ok()?;
        let s = String::from_utf8(out.stdout).ok()?;
        s.split_whitespace().find_map(|w| w.parse().ok())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let s = std::fs::read_to_string("/proc/loadavg").ok()?;
        s.split_whitespace().next()?.parse().ok()
    }
}

fn cpu_brand() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .ok()?;
        let s = String::from_utf8(out.stdout).ok()?.trim().to_owned();
        (!s.is_empty()).then_some(s)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let info = std::fs::read_to_string("/proc/cpuinfo").ok()?;
        info.lines()
            .find(|l| l.starts_with("model name"))
            .and_then(|l| l.split(':').nth(1))
            .map(|s| s.trim().to_owned())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct GateCheck {
    pub gate: String,
    pub target_hardware: &'static str,
    pub metric: String,
    pub measured: f64,
    pub limit: f64,
    /// Measured ≤ limit on *this* machine.
    pub within_limit_here: bool,
    /// True only when every §2.2 condition for this gate was met: right
    /// device, thread count, pacing and scene. Otherwise `within_limit_here`
    /// is indicative only.
    pub decisive: bool,
    /// The unmet conditions (empty when `decisive`).
    pub caveats: Vec<String>,
}

fn check(
    machine: &Machine,
    gate: &str,
    hw: &'static str,
    metric: String,
    measured: f64,
    limit: f64,
    mut caveats: Vec<String>,
) -> GateCheck {
    if machine.gate_device != Some(hw) {
        caveats.insert(0, format!("not measured on a {hw}"));
    }
    GateCheck {
        gate: gate.into(),
        target_hardware: hw,
        metric,
        measured,
        limit,
        within_limit_here: measured <= limit,
        decisive: caveats.is_empty(),
        caveats,
    }
}

fn threads_caveat(threads: usize, want: usize) -> Option<String> {
    (threads != want).then(|| format!("{threads} threads (gate: {want})"))
}

/// Compares results with the §2.2 M0 limits. A check is `decisive` only if
/// the run met that gate's conditions; M0 simbench cannot yet provide the GPU
/// load scene, presenter, late-game snapshot or vehicles, so today every
/// check is indicative.
pub fn gate_checks(
    machine: &Machine,
    los: Option<&LosReport>,
    orca: &[OrcaResult],
    ticks: &[TickResult],
) -> Vec<GateCheck> {
    let kernel_caveats = |threads: usize| -> Vec<String> {
        threads_caveat(threads, gates::DECK_THREADS)
            .into_iter()
            .chain(["no GPU load scene".to_owned()])
            .collect()
    };
    let mut g = Vec::new();
    if let Some(l) = los {
        // The gate names the tiled + max-mip configuration; also show the best.
        if let Some(v) = l.variants.iter().find(|v| v.variant == "tiled16+maxmip") {
            let mut cav = kernel_caveats(l.config.threads);
            cav.push("no occluder BVH".into());
            g.push(check(
                machine,
                "M0.1 LOS ray",
                gates::DECK,
                format!(
                    "cpu µs/ray, {} ({} threads, no occluder BVH)",
                    v.variant, l.config.threads
                ),
                v.cpu_us_per_ray,
                gates::DECK_LOS_US_PER_RAY,
                cav,
            ));
        }
        if let Some(v) = l
            .variants
            .iter()
            .find(|v| v.variant == l.fastest && v.variant != "tiled16+maxmip")
        {
            let mut cav = kernel_caveats(l.config.threads);
            cav.push("no occluder BVH".into());
            g.push(check(
                machine,
                "M0.1 LOS ray (fastest variant)",
                gates::DECK,
                format!(
                    "cpu µs/ray, {} ({} threads, no occluder BVH)",
                    v.variant, l.config.threads
                ),
                v.cpu_us_per_ray,
                gates::DECK_LOS_US_PER_RAY,
                cav,
            ));
        }
    }
    for o in orca {
        g.push(check(
            machine,
            "M0.1 ORCA",
            gates::DECK,
            format!("cpu µs/agent, {} agents ({} threads)", o.agents, o.threads),
            o.cpu_us_per_agent,
            gates::DECK_ORCA_US_PER_AGENT,
            kernel_caveats(o.threads),
        ));
    }
    for t in ticks {
        let (hw, want_threads) = match t.tier {
            Tier::Battalion => (gates::DECK, gates::DECK_THREADS),
            Tier::Regiment => (gates::RYZEN_3600, gates::RYZEN_3600_THREADS),
        };
        let pace = if t.paced { "paced" } else { "unpaced" };
        let mut cav: Vec<String> = Vec::new();
        if !t.paced {
            cav.push("unpaced".into());
        }
        cav.extend(threads_caveat(t.threads, want_threads));
        cav.extend(
            [
                "no late-game snapshot",
                "no presenter",
                "no vehicles",
                "perception/damage/status are placeholders",
            ]
            .map(String::from),
        );
        g.push(check(
            machine,
            "M0.2 composed tick P95",
            hw,
            format!("{} P95 ms ({pace}, {} threads)", t.tier.name(), t.threads),
            t.total.p95_ms,
            gates::TICK_P95_MS,
            cav.clone(),
        ));
        g.push(check(
            machine,
            "M0.2 composed tick P99",
            hw,
            format!("{} P99 ms ({pace}, {} threads)", t.tier.name(), t.threads),
            t.total.p99_ms,
            gates::TICK_P99_MS,
            cav,
        ));
    }
    g
}

/// Everything one `simbench` invocation measured.
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub schema: &'static str,
    pub machine: Machine,
    pub baseline_notice: String,
    pub threads: usize,
    pub seed: u64,
    pub profile: &'static str,
    pub los: Option<LosReport>,
    pub orca: Vec<OrcaResult>,
    pub tick: Vec<TickResult>,
    pub gates: Vec<GateCheck>,
}

pub fn build_profile() -> &'static str {
    if cfg!(debug_assertions) {
        "debug (numbers meaningless)"
    } else {
        "release"
    }
}

impl Report {
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "simbench: Frontline M0 spike benchmarks (§2.2)");
        let _ = writeln!(
            s,
            "machine: {} | {} logical CPUs | build: {} | workers: {} | seed: {}",
            self.machine.cpu, self.machine.logical_cpus, self.profile, self.threads, self.seed
        );
        let _ = writeln!(s, "!! {}", self.baseline_notice);
        if let Some(w) = self.machine.load_warning() {
            let _ = writeln!(s, "!! {w}");
        }

        if let Some(l) = &self.los {
            let c = &l.config;
            let _ = writeln!(s, "\n== LOS ray kernel ==");
            let _ = writeln!(
                s,
                "map {0}x{0} m at {1} m cells | base {2:.1} MiB u16 + max-mip pyramid {3:.1} MiB ({4} levels) | {5} rays of {6}-{7} m | {8} reps (median)",
                c.map_m,
                l.cell_m,
                l.base_mib,
                l.pyramid_mib,
                c.mip_levels,
                c.rays,
                c.min_m,
                c.max_m,
                c.reps
            );
            let _ = writeln!(
                s,
                "{:<18} {:>11} {:>11} {:>11} {:>8} {:>8} {:>8}",
                "variant", "cpu µs/ray", "wall µs/ray", "1-thr µs", "visible", "cells", "blocks"
            );
            for v in &l.variants {
                let _ = writeln!(
                    s,
                    "{:<18} {:>11.3} {:>11.3} {:>11.3} {:>7.1}% {:>8.1} {:>8.1}",
                    v.variant,
                    v.cpu_us_per_ray,
                    v.wall_us_per_ray,
                    v.single_thread_us_per_ray,
                    v.visible_fraction * 100.0,
                    v.mean_cells_tested,
                    v.mean_block_tests
                );
            }
            let _ = writeln!(
                s,
                "results identical across variants: {} | fastest: {}",
                if l.results_identical {
                    "yes"
                } else {
                    "NO (bug!)"
                },
                l.fastest
            );
            for n in &l.notes {
                let _ = writeln!(s, "note: {n}");
            }
        }

        if !self.orca.is_empty() {
            let _ = writeln!(
                s,
                "\n== ORCA avoidance kernel (Q16.16, ≤8 neighbours, 16 m hash) =="
            );
            let _ = writeln!(
                s,
                "{:>7} {:>8} {:>12} {:>12} {:>10} {:>9} {:>9} {:>9} {:>9} {:>9} {:>6}",
                "agents",
                "field m",
                "cpu µs/agt",
                "wall µs/agt",
                "1-thr µs",
                "tick ms",
                "grid ms",
                "plan ms",
                "integ ms",
                "neigh",
                "det"
            );
            for o in &self.orca {
                let _ = writeln!(
                    s,
                    "{:>7} {:>8} {:>12.3} {:>12.3} {:>10.3} {:>9.3} {:>9.3} {:>9.3} {:>9.3} {:>9.2} {:>6}",
                    o.agents,
                    o.field_m,
                    o.cpu_us_per_agent,
                    o.wall_us_per_agent,
                    o.single_thread_us_per_agent,
                    o.mean_tick_ms,
                    o.rebuild_ms,
                    o.plan_ms,
                    o.integrate_ms,
                    o.mean_neighbours,
                    if o.deterministic { "ok" } else { "FAIL" }
                );
            }
        }

        for t in &self.tick {
            let _ = writeln!(
                s,
                "\n== Composed tick: {} ({} soldiers, pool {}, {} m sim map{}, {} rays/tick{}) ==",
                t.tier.name(),
                t.soldiers,
                t.projectile_cap,
                t.sim_map_m,
                if t.spawn_area_m > 0 {
                    format!(" (spawn {} m)", t.spawn_area_m)
                } else {
                    String::new()
                },
                t.rays_per_tick,
                t.los_variant
                    .as_deref()
                    .map(|v| format!(" via {v}"))
                    .unwrap_or_default()
            );
            let _ = writeln!(
                s,
                "{} Hz, {}, {} warm-up + {} measured ticks, {} workers",
                t.tick_hz,
                if t.paced {
                    "paced"
                } else {
                    "unpaced (back-to-back)"
                },
                t.warmup_ticks,
                t.ticks,
                t.threads
            );
            let _ = writeln!(
                s,
                "{:<14} {:>8} {:>8} {:>8} {:>8} {:>8}",
                "ms", "mean", "p50", "p95", "p99", "max"
            );
            for (name, d) in [
                ("total", &t.total),
                ("sim step", &t.step),
                ("ray budget", &t.rays),
            ] {
                let _ = writeln!(
                    s,
                    "{:<14} {:>8.3} {:>8.3} {:>8.3} {:>8.3} {:>8.3}",
                    name, d.mean_ms, d.p50_ms, d.p95_ms, d.p99_ms, d.max_ms
                );
            }
            let st = &t.stages;
            let _ = writeln!(
                s,
                "stage means ms: intent {:.3} | perception {:.3} | movement {:.3} | weapons {:.3} | ballistics {:.3} | damage {:.3} | status {:.3} | hash {:.3} | snapshot {:.3}",
                st.intent_ms,
                st.perception_ms,
                st.movement_ms,
                st.weapons_ms,
                st.ballistics_ms,
                st.damage_ms,
                st.status_ms,
                st.world_hash_ms,
                st.snapshot_ms
            );
            let _ = writeln!(
                s,
                "projectiles live mean {:.0} / peak {} (cap {}) | shots {} fired, {} dropped | rays visible {:.1}% | overruns {}",
                t.mean_live_projectiles,
                t.peak_live_projectiles,
                t.projectile_cap,
                t.shots_fired,
                t.shots_dropped,
                t.visible_ray_fraction * 100.0,
                t.overruns
            );
        }
        if !self.tick.is_empty() {
            let _ = writeln!(
                s,
                "note: no late-game snapshot yet (warm-up fills the pool); perception/damage/status are sim placeholders; no vehicles; rays run after step on the sim's own pool."
            );
        }

        if !self.gates.is_empty() {
            let _ = writeln!(
                s,
                "\n== §2.2 M0 gates (limits are for the target hardware) =="
            );
            let _ = writeln!(
                s,
                "{:<32} {:<13} {:>10} {:>8}  {:<10} {:<11} metric",
                "gate", "target hw", "measured", "limit", "here", "verdict"
            );
            for g in &self.gates {
                let _ = writeln!(
                    s,
                    "{:<32} {:<13} {:>10.3} {:>8.2}  {:<10} {:<11} {}",
                    g.gate,
                    g.target_hardware,
                    g.measured,
                    g.limit,
                    if g.within_limit_here {
                        "within"
                    } else {
                        "OVER"
                    },
                    if g.decisive {
                        "PASS/FAIL"
                    } else {
                        "indicative"
                    },
                    g.metric
                );
                if !g.caveats.is_empty() {
                    let _ = writeln!(s, "{:<32} not decisive: {}", "", g.caveats.join("; "));
                }
            }
            if !self.machine.is_gate_hardware {
                let _ = writeln!(
                    s,
                    "These are {} baselines. Gate verdicts need the Deck LCD/OLED (3 threads, GPU-load scene) and a Ryzen 5 3600.",
                    self.machine.cpu
                );
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_device_detection() {
        assert_eq!(gate_device("amd custom apu 0405"), Some(gates::DECK));
        assert_eq!(
            gate_device("amd ryzen 5 3600 6-core processor"),
            Some(gates::RYZEN_3600)
        );
        assert_eq!(gate_device("amd ryzen 5 3600"), Some(gates::RYZEN_3600));
        assert_eq!(gate_device("amd ryzen 5 3600x 6-core processor"), None);
        assert_eq!(gate_device("amd ryzen 5 3600xt"), None);
        assert_eq!(gate_device("apple m3"), None);
    }
}
