//! Standalone ORCA avoidance kernel benchmark (§5.5, M0 kernel gate).
//!
//! The kernel mirrors `frontline_sim`'s movement stage: up to
//! [`MAX_NEIGHBOURS`] nearest neighbours (ties by index) from a 16 m uniform
//! spatial hash, one ORCA half-plane per neighbour in Q16.16, sequential
//! projection instead of RVO2's 2D linear program, then a max-speed clamp.
//! The sim keeps its kernel `pub(crate)`, so this is a copy; it should be
//! replaced by the sim's own kernel once that is exposed.
//!
//! Unlike the sim's M0 uniform spawn, agents here move in squads of 10 with
//! formation offsets, and squads cross paths. That gives the crowded
//! neighbourhoods the gate is about.

use std::time::{Duration, Instant};

use frontline_sim::fx::{Fx, FxVec2};
use frontline_sim::rng::{mix64, range_fx};
use frontline_sim::{MAX_NEIGHBOURS, SoldierParams};
use rayon::prelude::*;
use serde::Serialize;

/// Spatial hash cell size (§5.4).
pub const CELL_M: i32 = 16;
/// Agents per parallel chunk (matches the sim's `CHUNK_SIZE`).
pub const CHUNK: usize = 256;
const SQUAD: usize = 10;

/// Struct-of-arrays agent state.
#[derive(Clone, Debug)]
pub struct Agents {
    pub pos: Vec<FxVec2>,
    pub vel: Vec<FxVec2>,
    pub waypoint: Vec<FxVec2>,
    pub speed: Vec<Fx>,
    /// Formation offset from the squad target.
    pub offset: Vec<FxVec2>,
    pub leg: Vec<u32>,
}

/// Uniform spatial hash in CSR form, rebuilt per tick by counting sort.
#[derive(Clone, Debug, Default)]
pub struct Grid {
    dim: u32,
    cell_start: Vec<u32>,
    ids: Vec<u32>,
    cell_of: Vec<u32>,
    cursor: Vec<u32>,
}

impl Grid {
    pub fn new(field_m: i32) -> Grid {
        let dim = ((field_m + CELL_M - 1) / CELL_M).max(1) as u32;
        Grid {
            dim,
            ..Grid::default()
        }
    }

    #[inline]
    fn coord(&self, v: Fx) -> u32 {
        (v.raw() >> Fx::FRAC_BITS)
            .div_euclid(CELL_M)
            .clamp(0, self.dim as i32 - 1) as u32
    }

    pub fn rebuild(&mut self, pos: &[FxVec2]) {
        let cells = (self.dim * self.dim) as usize;
        self.cell_start.clear();
        self.cell_start.resize(cells + 1, 0);
        self.cell_of.clear();
        for p in pos {
            let c = self.coord(p.y) * self.dim + self.coord(p.x);
            self.cell_of.push(c);
            self.cell_start[c as usize + 1] += 1;
        }
        for c in 0..cells {
            self.cell_start[c + 1] += self.cell_start[c];
        }
        self.ids.clear();
        self.ids.resize(pos.len(), 0);
        self.cursor.clear();
        self.cursor.extend_from_slice(&self.cell_start);
        for (i, &c) in self.cell_of.iter().enumerate() {
            let slot = &mut self.cursor[c as usize];
            self.ids[*slot as usize] = i as u32;
            *slot += 1;
        }
    }

    /// Indices in cells overlapping `center ± r`, row-major by cell, each
    /// cell ascending.
    #[inline]
    pub fn for_each_near(&self, center: FxVec2, r: Fx, mut f: impl FnMut(u32)) {
        let (x0, x1) = (self.coord(center.x - r), self.coord(center.x + r));
        let (y0, y1) = (self.coord(center.y - r), self.coord(center.y + r));
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                let c = (cy * self.dim + cx) as usize;
                for &id in &self.ids[self.cell_start[c] as usize..self.cell_start[c + 1] as usize] {
                    f(id);
                }
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct Line {
    point: FxVec2,
    dir: FxVec2,
}

/// Plans new velocities for agents `base..base + out.len()`. Reads only the
/// previous positions/velocities. Returns the number of neighbours used.
pub fn plan_chunk(
    base: usize,
    out: &mut [FxVec2],
    a: &Agents,
    grid: &Grid,
    p: &SoldierParams,
    tick_hz: i32,
) -> u64 {
    let combined_r = p.radius.mul_int(2);
    let combined_r_sq = combined_r.mul(combined_r);
    let inv_tau = Fx::ONE / p.time_horizon;
    let inv_dt = Fx::from_int(tick_hz);
    let neigh_sq_q32 = (p.neighbour_dist.mul(p.neighbour_dist).raw() as u64) << Fx::FRAC_BITS;
    let arrive_sq = p.arrive_radius.mul(p.arrive_radius);
    let max_n = (p.max_neighbours as usize).clamp(1, MAX_NEIGHBOURS);
    let mut used = 0u64;

    for (j, out_v) in out.iter_mut().enumerate() {
        let i = base + j;
        let pos = a.pos[i];
        let vel = a.vel[i];
        let to_wp = a.waypoint[i] - pos;
        let pref = if to_wp.length_sq() <= arrive_sq {
            FxVec2::ZERO
        } else {
            to_wp.normalize_or_zero() * a.speed[i]
        };

        let mut near = [(u64::MAX, u32::MAX); MAX_NEIGHBOURS];
        let mut n = 0usize;
        grid.for_each_near(pos, p.neighbour_dist, |o| {
            if o as usize == i {
                return;
            }
            let dsq = (a.pos[o as usize] - pos).length_sq_raw();
            if dsq >= neigh_sq_q32 {
                return;
            }
            let key = (dsq, o);
            if n < max_n {
                near[n] = key;
                n += 1;
            } else if key < near[max_n - 1] {
                near[max_n - 1] = key;
            } else {
                return;
            }
            let mut k = n - 1;
            while k > 0 && near[k] < near[k - 1] {
                near.swap(k, k - 1);
                k -= 1;
            }
        });
        used += n as u64;

        let mut lines = [Line {
            point: FxVec2::ZERO,
            dir: FxVec2::ZERO,
        }; MAX_NEIGHBOURS];
        let mut nl = 0;
        for &(_, o) in &near[..n] {
            let o = o as usize;
            let rel_pos = a.pos[o] - pos;
            let rel_vel = vel - a.vel[o];
            let dist_sq = rel_pos.length_sq();
            let (dir, u);
            if dist_sq > combined_r_sq {
                let w = rel_vel - rel_pos * inv_tau;
                let w_len_sq = w.length_sq();
                let dot1 = w.dot(rel_pos);
                if dot1.is_negative() && dot1.mul(dot1) > combined_r_sq.mul(w_len_sq) {
                    let w_len = w.length();
                    if w_len.is_zero() {
                        continue;
                    }
                    let unit_w = w / w_len;
                    dir = FxVec2::new(unit_w.y, -unit_w.x);
                    u = unit_w * (combined_r.mul(inv_tau) - w_len);
                } else {
                    let leg = (dist_sq - combined_r_sq).sqrt();
                    let (px, py) = (rel_pos.x, rel_pos.y);
                    if rel_pos.det(w).raw() > 0 {
                        dir = FxVec2::new(
                            px.mul(leg) - py.mul(combined_r),
                            px.mul(combined_r) + py.mul(leg),
                        ) / dist_sq;
                    } else {
                        dir = -(FxVec2::new(
                            px.mul(leg) + py.mul(combined_r),
                            -px.mul(combined_r) + py.mul(leg),
                        ) / dist_sq);
                    }
                    u = dir * rel_vel.dot(dir) - rel_vel;
                }
            } else {
                let w = rel_vel - rel_pos * inv_dt;
                let w_len = w.length();
                let unit_w = if w_len.is_zero() {
                    if i < o {
                        FxVec2::new(Fx::ONE, Fx::ZERO)
                    } else {
                        FxVec2::new(Fx::NEG_ONE, Fx::ZERO)
                    }
                } else {
                    w / w_len
                };
                dir = FxVec2::new(unit_w.y, -unit_w.x);
                u = unit_w * (combined_r.mul(inv_dt) - w_len);
            }
            lines[nl] = Line {
                point: vel + u * Fx::HALF,
                dir,
            };
            nl += 1;
        }

        let mut v = pref;
        for l in &lines[..nl] {
            if l.dir.det(l.point - v).raw() > 0 {
                v = l.point + l.dir * l.dir.dot(v - l.point);
            }
        }
        *out_v = v.clamp_length(a.speed[i]);
    }
    used
}

/// Seeded squad target for leg `leg`.
fn squad_target(seed: u64, squad: usize, leg: u32, field: Fx) -> FxVec2 {
    let r = mix64(seed ^ mix64(((squad as u64) << 32) | leg as u64));
    let margin = Fx::from_int(20);
    FxVec2::new(
        range_fx(r, margin, field - margin),
        range_fx(mix64(r), margin, field - margin),
    )
}

/// The benchmark scene: `n` agents in squads of 10 on a square field sized
/// so squads keep meeting (`side ≈ 12 m · √n`).
#[derive(Clone, Debug)]
pub struct Scene {
    pub agents: Agents,
    pub field: Fx,
    pub seed: u64,
    pub params: SoldierParams,
}

impl Scene {
    pub fn new(n: usize, seed: u64, params: SoldierParams) -> Scene {
        let side = ((n as f64).sqrt() * 12.0).max(64.0) as i32;
        let field = Fx::from_int(side);
        let mut a = Agents {
            pos: Vec::with_capacity(n),
            vel: vec![FxVec2::ZERO; n],
            waypoint: Vec::with_capacity(n),
            speed: Vec::with_capacity(n),
            offset: Vec::with_capacity(n),
            leg: vec![0; n],
        };
        let spread = Fx::from_int(5);
        for i in 0..n {
            let squad = i / SQUAD;
            let r = mix64(seed ^ 0x0BCA ^ ((i as u64) << 8));
            let off = FxVec2::new(
                range_fx(r, -spread, spread),
                range_fx(mix64(r), -spread, spread),
            );
            let start = squad_target(seed, squad, u32::MAX, field);
            a.pos.push(clamp(start + off, field));
            a.offset.push(off);
            a.waypoint
                .push(clamp(squad_target(seed, squad, 0, field) + off, field));
            a.speed
                .push(range_fx(mix64(r ^ 1), params.speed_min, params.speed_max));
        }
        Scene {
            agents: a,
            field,
            seed,
            params,
        }
    }
}

fn clamp(p: FxVec2, field: Fx) -> FxVec2 {
    FxVec2::new(p.x.clamp(Fx::ZERO, field), p.y.clamp(Fx::ZERO, field))
}

/// Per-tick timings of [`step`].
#[derive(Copy, Clone, Debug, Default)]
pub struct StepTimes {
    pub rebuild: Duration,
    pub plan: Duration,
    pub integrate: Duration,
    pub neighbours: u64,
}

/// One tick: rebuild the hash (serial), plan (parallel per chunk), integrate
/// and re-pick reached waypoints (parallel per chunk).
pub fn step(
    scene: &mut Scene,
    grid: &mut Grid,
    planned: &mut Vec<FxVec2>,
    pool: &rayon::ThreadPool,
    tick_hz: i32,
) -> StepTimes {
    let n = scene.agents.pos.len();
    let t0 = Instant::now();
    grid.rebuild(&scene.agents.pos);
    let t1 = Instant::now();
    planned.resize(n, FxVec2::ZERO);
    let (a, params) = (&scene.agents, &scene.params);
    let g = &*grid;
    let neighbours = pool.install(|| {
        planned
            .par_chunks_mut(CHUNK)
            .enumerate()
            .map(|(c, out)| plan_chunk(c * CHUNK, out, a, g, params, tick_hz))
            .sum()
    });
    let t2 = Instant::now();
    let (field, seed) = (scene.field, scene.seed);
    let arrive_sq = scene.params.arrive_radius.mul(scene.params.arrive_radius);
    let Agents {
        pos,
        vel,
        waypoint,
        offset,
        leg,
        ..
    } = &mut scene.agents;
    pool.install(|| {
        pos.par_chunks_mut(CHUNK)
            .zip(vel.par_chunks_mut(CHUNK))
            .zip(waypoint.par_chunks_mut(CHUNK))
            .zip(leg.par_chunks_mut(CHUNK))
            .enumerate()
            .for_each(|(c, (((p, v), w), l))| {
                for j in 0..p.len() {
                    let i = c * CHUNK + j;
                    v[j] = planned[i];
                    p[j] = clamp(p[j] + v[j].div_int(tick_hz), field);
                    if (w[j] - p[j]).length_sq() <= arrive_sq {
                        l[j] += 1;
                        w[j] = clamp(
                            squad_target(seed, i / SQUAD, l[j], field) + offset[i],
                            field,
                        );
                    }
                }
            });
    });
    let t3 = Instant::now();
    StepTimes {
        rebuild: t1 - t0,
        plan: t2 - t1,
        integrate: t3 - t2,
        neighbours,
    }
}

/// Order-sensitive hash of all mutable agent state: positions, velocities,
/// waypoints and leg counters (determinism check).
pub fn state_hash(a: &Agents) -> u64 {
    let pack = |v: FxVec2| (v.x.raw() as u32 as u64) << 32 | v.y.raw() as u32 as u64;
    (0..a.pos.len()).fold(0x5EED_u64, |h, i| {
        let w = pack(a.pos[i])
            ^ mix64(pack(a.vel[i]))
            ^ mix64(mix64(pack(a.waypoint[i])) ^ a.leg[i] as u64);
        mix64(h ^ w)
    })
}

// ------------------------------------------------------------- benchmark --

#[derive(Clone, Debug, Serialize)]
pub struct OrcaBenchConfig {
    pub agents: Vec<usize>,
    pub ticks: u32,
    pub warmup: u32,
    pub threads: usize,
    pub seed: u64,
    pub neighbour_dist_m: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct OrcaResult {
    pub agents: usize,
    pub field_m: i32,
    pub ticks: u32,
    pub threads: usize,
    /// Gate metric: (rebuild + (plan + integrate) × threads) / agents.
    pub cpu_us_per_agent: f64,
    /// Wall time per tick / agents with `threads` workers.
    pub wall_us_per_agent: f64,
    /// Same scene on one worker.
    pub single_thread_us_per_agent: f64,
    pub mean_tick_ms: f64,
    pub rebuild_ms: f64,
    pub plan_ms: f64,
    pub integrate_ms: f64,
    pub mean_neighbours: f64,
    /// Final state hash identical with 1 and `threads` workers.
    pub deterministic: bool,
}

fn run_one(n: usize, cfg: &OrcaBenchConfig, threads: usize) -> anyhow::Result<(OrcaResult, u64)> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()?;
    let params = SoldierParams {
        neighbour_dist: Fx::from_milli((cfg.neighbour_dist_m * 1000.0).round() as i32),
        ..SoldierParams::default()
    };
    let mut scene = Scene::new(n, cfg.seed, params);
    let mut grid = Grid::new(scene.field.floor_int() + 1);
    let mut planned = Vec::new();
    const HZ: i32 = 30;
    for _ in 0..cfg.warmup {
        step(&mut scene, &mut grid, &mut planned, &pool, HZ);
    }
    let mut sum = StepTimes::default();
    for _ in 0..cfg.ticks {
        let t = step(&mut scene, &mut grid, &mut planned, &pool, HZ);
        sum.rebuild += t.rebuild;
        sum.plan += t.plan;
        sum.integrate += t.integrate;
        sum.neighbours += t.neighbours;
    }
    let ticks = cfg.ticks.max(1) as f64;
    let ms = |d: Duration| d.as_secs_f64() * 1e3 / ticks;
    let (rb, pl, ig) = (ms(sum.rebuild), ms(sum.plan), ms(sum.integrate));
    let tick_ms = rb + pl + ig;
    let us_per = |m: f64| m * 1e3 / n as f64;
    Ok((
        OrcaResult {
            agents: n,
            field_m: scene.field.floor_int(),
            ticks: cfg.ticks,
            threads,
            cpu_us_per_agent: us_per(rb + (pl + ig) * threads as f64),
            wall_us_per_agent: us_per(tick_ms),
            single_thread_us_per_agent: 0.0,
            mean_tick_ms: tick_ms,
            rebuild_ms: rb,
            plan_ms: pl,
            integrate_ms: ig,
            mean_neighbours: sum.neighbours as f64 / (ticks * n as f64),
            deterministic: true,
        },
        state_hash(&scene.agents),
    ))
}

pub fn run_bench(cfg: &OrcaBenchConfig) -> anyhow::Result<Vec<OrcaResult>> {
    cfg.agents
        .iter()
        .map(|&n| {
            anyhow::ensure!(n > 0, "agent count must be positive");
            let (mut r, h) = run_one(n, cfg, cfg.threads)?;
            let (r1, h1) = run_one(n, cfg, 1)?;
            r.single_thread_us_per_agent = r1.wall_us_per_agent;
            r.deterministic = h == h1;
            Ok(r)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(n: usize, threads: usize, ticks: u32) -> u64 {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let mut scene = Scene::new(n, 5, SoldierParams::default());
        let mut grid = Grid::new(scene.field.floor_int() + 1);
        let mut planned = Vec::new();
        for _ in 0..ticks {
            step(&mut scene, &mut grid, &mut planned, &pool, 30);
        }
        state_hash(&scene.agents)
    }

    #[test]
    fn thread_count_does_not_change_results() {
        let h1 = run(700, 1, 60);
        assert_eq!(h1, run(700, 3, 60));
        assert_eq!(h1, run(700, 8, 60));
    }

    #[test]
    fn head_on_pair_keeps_apart() {
        let params = SoldierParams::default();
        let mut scene = Scene::new(2, 1, params.clone());
        scene.field = Fx::from_int(200);
        let a = &mut scene.agents;
        let y = Fx::from_int(100);
        a.pos = vec![
            FxVec2::new(Fx::from_int(80), y),
            FxVec2::new(Fx::from_int(120), y),
        ];
        a.waypoint = vec![
            FxVec2::new(Fx::from_int(140), y),
            FxVec2::new(Fx::from_int(60), y),
        ];
        a.speed = vec![params.speed_max; 2];
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap();
        let mut grid = Grid::new(scene.field.floor_int() + 1);
        let mut planned = Vec::new();
        let mut min_d = Fx::MAX;
        for _ in 0..30 * 30 {
            step(&mut scene, &mut grid, &mut planned, &pool, 30);
            let d = (scene.agents.pos[0] - scene.agents.pos[1]).length();
            min_d = min_d.min(d);
        }
        assert!(min_d > Fx::from_milli(700), "min distance {min_d:?}");
    }

    #[test]
    fn neighbour_search_is_capped_and_finds_squadmates() {
        let scene = Scene::new(700, 2, SoldierParams::default());
        let mut grid = Grid::new(scene.field.floor_int() + 1);
        grid.rebuild(&scene.agents.pos);
        let mut out = vec![FxVec2::ZERO; 700];
        let used = plan_chunk(0, &mut out, &scene.agents, &grid, &scene.params, 30);
        assert!(used <= 700 * MAX_NEIGHBOURS as u64);
        assert!(used > 700, "squads should give neighbours: {used}");
    }
}
