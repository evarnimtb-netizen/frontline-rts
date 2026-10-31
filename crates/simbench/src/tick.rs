//! Composed-tick benchmark (§2.2 M0 gate 2): `frontline_sim::World::step` at
//! tier caps, plus the tier's global ray budget traced with the fastest LOS
//! kernel, optionally paced at the tick rate.
//!
//! Limits of this M0 measurement:
//! * There is no late-game snapshot yet. A warm-up period fills the
//!   projectile pool instead, and the soldiers keep their M0 random-walk
//!   behaviour.
//! * The perception, damage and status stages are still placeholders in the
//!   sim. The synthetic ray budget stands in for perception's main cost.
//! * Rays run after `World::step` returns, on the world's own worker pool
//!   (`World::install`), so they share threads with the sim as perception
//!   will. A separate pool is used only for LOS setup and calibration.
//! * There are no vehicles yet (§15 M0 caps add 100 / 300).

use std::time::{Duration, Instant};

use frontline_sim::ecs::EntityKind;
use frontline_sim::fx::Fx;
use frontline_sim::rng::mix64;
use frontline_sim::{SimConfig, StageTimes, World};
use rayon::prelude::*;
use serde::Serialize;

use crate::grid::generate_terrain;
use crate::los::{self, AnyGrid, LosVariant, RayResult};
use crate::stats::Dist;

/// Lobby battle-size tier (§2.2).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Battalion,
    Regiment,
}

impl Tier {
    pub fn soldiers(self) -> u32 {
        match self {
            Tier::Battalion => 700,
            Tier::Regiment => 2000,
        }
    }
    pub fn projectile_cap(self) -> u32 {
        match self {
            Tier::Battalion => 1800,
            Tier::Regiment => 5000,
        }
    }
    pub fn rays_per_tick(self) -> usize {
        match self {
            Tier::Battalion => 2400,
            Tier::Regiment => 7500,
        }
    }
    /// Sim map side. The M0 sim spawns uniformly, so the map is kept small
    /// enough that soldiers are in contact (≈19 m mean spacing).
    pub fn default_sim_map_m(self) -> i32 {
        match self {
            Tier::Battalion => 512,
            Tier::Regiment => 832,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Tier::Battalion => "battalion",
            Tier::Regiment => "regiment",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct TickBenchConfig {
    pub tiers: Vec<Tier>,
    pub seconds: f64,
    pub warmup_s: f64,
    pub paced: bool,
    pub threads: usize,
    pub seed: u64,
    pub rays: bool,
    pub los_variant: Option<String>,
    pub los_map_m: u32,
    pub mip_levels: u32,
    pub sim_map_m: Option<i32>,
    /// Side of the centred spawn square (m); `None` spawns over the whole map.
    pub spawn_area_m: Option<i32>,
    /// Weapon cooldown range in ticks (lower = heavier fire).
    pub cooldown: (u16, u16),
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct StageMeans {
    pub intent_ms: f64,
    pub perception_ms: f64,
    pub movement_ms: f64,
    pub weapons_ms: f64,
    pub ballistics_ms: f64,
    pub damage_ms: f64,
    pub status_ms: f64,
    pub world_hash_ms: f64,
    pub snapshot_ms: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct TickResult {
    pub tier: Tier,
    pub soldiers: u32,
    pub projectile_cap: u32,
    pub sim_map_m: i32,
    /// Side of the centred spawn square (m); 0 = whole map.
    pub spawn_area_m: i32,
    pub rays_per_tick: usize,
    pub los_variant: Option<String>,
    pub threads: usize,
    pub tick_hz: i32,
    pub paced: bool,
    pub warmup_ticks: u32,
    pub ticks: u32,
    /// `World::step` + ray budget.
    pub total: Dist,
    pub step: Dist,
    pub rays: Dist,
    pub stages: StageMeans,
    pub mean_live_projectiles: f64,
    pub peak_live_projectiles: usize,
    pub shots_fired: u64,
    pub shots_dropped: u64,
    pub visible_ray_fraction: f64,
    /// Ticks whose total exceeded the tick period.
    pub overruns: u32,
    pub final_state_hash: String,
}

/// Traces `budget` rays this tick: observers rotate through live soldiers;
/// each looks along a seeded bearing for 300–600 m. Soldier positions are
/// placed at the centre of the LOS map.
#[allow(clippy::too_many_arguments)]
fn trace_tick_rays(
    grid: &AnyGrid,
    mip: Option<u32>,
    world: &World,
    budget: usize,
    offset: i64,
    seed: u64,
    positions: &mut Vec<[i64; 2]>,
    results: &mut Vec<RayResult>,
) -> usize {
    let ents = world.entities();
    positions.clear();
    for (k, t) in ents.kind().iter().zip(ents.transform()) {
        if *k == EntityKind::Soldier {
            positions.push([t.pos.x.raw() as i64 + offset, t.pos.y.raw() as i64 + offset]);
        }
    }
    if positions.is_empty() {
        return 0;
    }
    results.resize(budget, RayResult::default());
    let tick = world.tick();
    // u64 so the observer cursor behaves the same on 32-bit targets.
    let cursor = tick.wrapping_mul(budget as u64);
    let n_pos = positions.len() as u64;
    let dim = grid.dim();
    let pos = &*positions;
    world.install(|| {
        results
            .par_chunks_mut(256)
            .enumerate()
            .for_each(|(c, out)| {
                for (j, o) in out.iter_mut().enumerate() {
                    let r = c * 256 + j;
                    let from = pos[(cursor.wrapping_add(r as u64) % n_pos) as usize];
                    let roll = mix64(seed ^ mix64(tick ^ ((r as u64) << 32)));
                    let angle = Fx::from_raw((((roll >> 32) * Fx::TAU.raw() as u64) >> 32) as i32);
                    let len = Fx::from_int(300)
                        + Fx::from_raw(((roll & 0xFFFF_FFFF) % (300 << 16)) as i32);
                    let ray = los::ray_from(|x, y| grid.height(x, y), dim, from, angle, len);
                    *o = grid.trace(&ray, mip);
                }
            });
    });
    results.iter().filter(|r| r.visible).count()
}

fn add_stages(acc: &mut StageTimes, t: &StageTimes) {
    acc.intent += t.intent;
    acc.perception += t.perception;
    acc.movement += t.movement;
    acc.weapons += t.weapons;
    acc.ballistics += t.ballistics;
    acc.damage += t.damage;
    acc.status += t.status;
    acc.world_hash += t.world_hash;
    acc.snapshot += t.snapshot;
}

pub fn run_bench(cfg: &TickBenchConfig) -> anyhow::Result<(Vec<TickResult>, Option<LosVariant>)> {
    let ray_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(cfg.threads)
        .thread_name(|i| format!("ray-worker-{i}"))
        .build()?;

    // LOS heightfield for the synthetic ray budget.
    let (grid, variant) = if cfg.rays {
        let hm = ray_pool.install(|| generate_terrain(cfg.los_map_m, cfg.seed));
        let v = match &cfg.los_variant {
            Some(s) => {
                LosVariant::parse(s).ok_or_else(|| anyhow::anyhow!("unknown LOS variant {s:?}"))?
            }
            None => los::calibrate_fastest(
                &ray_pool,
                &hm,
                cfg.los_map_m,
                cfg.mip_levels,
                60_000,
                cfg.seed,
            ),
        };
        (
            Some(AnyGrid::build(v.layout, cfg.los_map_m, &hm, cfg.mip_levels)),
            Some(v),
        )
    } else {
        (None, None)
    };
    let mip = variant.and_then(|v| v.maxmip.then_some(cfg.mip_levels));

    let mut out = Vec::new();
    for &tier in &cfg.tiers {
        let mut sim = SimConfig {
            seed: cfg.seed,
            soldiers: tier.soldiers(),
            projectile_cap: tier.projectile_cap(),
            map_size_m: cfg.sim_map_m.unwrap_or(tier.default_sim_map_m()),
            spawn_area_m: cfg.spawn_area_m.unwrap_or(0),
            ..SimConfig::default()
        };
        sim.weapon.cooldown_min = cfg.cooldown.0;
        sim.weapon.cooldown_max = cfg.cooldown.1;
        let (sim_map_m, spawn_area_m) = (sim.map_size_m, sim.spawn_area_m);
        let mut world = World::new(sim, cfg.threads)?;
        let hz = world.config().tick_rate.hz();
        let period = Duration::from_secs_f64(1.0 / hz as f64);
        let warmup = (cfg.warmup_s * hz as f64).round() as u32;
        let ticks = ((cfg.seconds * hz as f64).round() as u32).max(1);
        let budget = if grid.is_some() {
            tier.rays_per_tick()
        } else {
            0
        };
        let offset = grid
            .as_ref()
            .map(|g| ((g.dim() as i64 - sim_map_m as i64) / 2).max(1) << Fx::FRAC_BITS)
            .unwrap_or(0);

        let mut positions = Vec::new();
        let mut results = Vec::new();
        let (mut total, mut step, mut rays) = (Vec::new(), Vec::new(), Vec::new());
        let mut stage_sum = StageTimes::default();
        let (mut live_sum, mut live_peak, mut visible, mut overruns) = (0u64, 0usize, 0u64, 0u32);
        let (mut fired0, mut dropped0) = (0, 0);
        let mut deadline = Instant::now();

        for t in 0..warmup + ticks {
            if t == warmup {
                fired0 = world.stats().shots_fired;
                dropped0 = world.stats().shots_dropped;
                deadline = Instant::now();
            }
            let t0 = Instant::now();
            world.step(&[]);
            let t1 = Instant::now();
            let vis = match &grid {
                Some(g) => trace_tick_rays(
                    g,
                    mip,
                    &world,
                    budget,
                    offset,
                    cfg.seed,
                    &mut positions,
                    &mut results,
                ),
                None => 0,
            };
            let t2 = Instant::now();
            if t >= warmup {
                step.push(t1 - t0);
                rays.push(t2 - t1);
                total.push(t2 - t0);
                if t2 - t0 > period {
                    overruns += 1;
                }
                add_stages(&mut stage_sum, world.stage_times());
                let live = world.projectiles().live();
                live_sum += live as u64;
                live_peak = live_peak.max(live);
                visible += vis as u64;
                if cfg.paced {
                    deadline += period;
                    let now = Instant::now();
                    if deadline > now {
                        std::thread::sleep(deadline - now);
                    } else {
                        deadline = now;
                    }
                }
            }
        }

        let n = ticks as f64;
        let ms = |d: Duration| d.as_secs_f64() * 1e3 / n;
        out.push(TickResult {
            tier,
            soldiers: tier.soldiers(),
            projectile_cap: tier.projectile_cap(),
            sim_map_m,
            spawn_area_m,
            rays_per_tick: budget,
            los_variant: variant.map(|v| v.label()),
            threads: cfg.threads,
            tick_hz: hz,
            paced: cfg.paced,
            warmup_ticks: warmup,
            ticks,
            total: Dist::from_durations(&total),
            step: Dist::from_durations(&step),
            rays: Dist::from_durations(&rays),
            stages: StageMeans {
                intent_ms: ms(stage_sum.intent),
                perception_ms: ms(stage_sum.perception),
                movement_ms: ms(stage_sum.movement),
                weapons_ms: ms(stage_sum.weapons),
                ballistics_ms: ms(stage_sum.ballistics),
                damage_ms: ms(stage_sum.damage),
                status_ms: ms(stage_sum.status),
                world_hash_ms: ms(stage_sum.world_hash),
                snapshot_ms: ms(stage_sum.snapshot),
            },
            mean_live_projectiles: live_sum as f64 / n,
            peak_live_projectiles: live_peak,
            shots_fired: world.stats().shots_fired - fired0,
            shots_dropped: world.stats().shots_dropped - dropped0,
            visible_ray_fraction: if budget > 0 {
                visible as f64 / (n * budget as f64)
            } else {
                0.0
            },
            overruns,
            final_state_hash: format!("{:016x}", world.state_hash()),
        });
    }
    Ok((out, variant))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_tick_run_is_sane() {
        let cfg = TickBenchConfig {
            tiers: vec![Tier::Battalion],
            seconds: 0.5,
            warmup_s: 0.5,
            paced: false,
            threads: 2,
            seed: 3,
            rays: true,
            los_variant: Some("tiled16+maxmip".into()),
            los_map_m: 1024,
            mip_levels: 6,
            sim_map_m: None,
            spawn_area_m: Some(256),
            cooldown: (10, 30),
        };
        let (r, v) = run_bench(&cfg).unwrap();
        assert_eq!(v.unwrap().label(), "tiled16+maxmip");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].ticks, 15);
        assert_eq!(r[0].total.n, 15);
        assert!(r[0].mean_live_projectiles > 0.0);
        assert!(r[0].visible_ray_fraction > 0.0 && r[0].visible_ray_fraction < 1.0);
    }
}
