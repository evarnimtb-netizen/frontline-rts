//! Line-of-sight ray march over a [`HeightGrid`] (§5.6, M0 kernel gate).
//!
//! # Ray model
//!
//! A ray from `a` to `b` (Q16.16 metres, z up) is sampled at `n + 1` points
//! `p_k = a + floor(k · (b − a) / n)` per axis, with `n` = the major-axis
//! length in whole metres. Each step moves at most one 1 m cell per axis (a
//! Bresenham-style DDA). The ray is blocked if any interior sample
//! `k ∈ 1..n` lies strictly below the flat-topped height of its cell.
//!
//! Every coordinate is exact integer arithmetic and monotone in `k`. Two
//! consequences:
//!
//! * The sample at any `k` can be computed directly (`seek`) or
//!   incrementally (`step`) with identical results, so memory layouts can't
//!   change the answer.
//! * Over any run of samples the lowest `z` is at one end of the run. If that
//!   minimum is at or above a pyramid block's max height, no sample inside
//!   the block can be blocked, and the whole block is skipped. The max-mip
//!   march therefore returns exactly the linear march's result.
//!
//! The building-occluder BVH that the §2.2 gate includes is out of scope for
//! M0 here; these numbers are heightfield-only.

use frontline_sim::fx::Fx;
use frontline_sim::rng::mix64;

use crate::grid::{HEIGHT_TO_Q16_SHIFT, HeightGrid, Layout, Linear, Morton, Tiled};

const FRAC: u32 = Fx::FRAC_BITS;
/// Highest pyramid level the march will use (64 m blocks at 6).
pub const MAX_MIP_LEVEL: u32 = 12;

/// A LOS query in Q16.16 raw coordinates.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Ray {
    pub o: [i64; 3],
    pub d: [i64; 3],
    /// Sample count along the major axis (≥ 1).
    pub n: i64,
}

impl Ray {
    /// Ray from `a` to `b`, both Q16.16 raw `[x, y, z]`, inside the grid.
    pub fn new(a: [i64; 3], b: [i64; 3]) -> Ray {
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let major = d[0].abs().max(d[1].abs());
        let n = ((major + (1 << FRAC) - 1) >> FRAC).max(1);
        Ray { o: a, d, n }
    }

    /// Coordinate `axis` of sample `k`.
    #[inline(always)]
    pub fn at(&self, axis: usize, k: i64) -> i64 {
        self.o[axis] + (k * self.d[axis]).div_euclid(self.n)
    }

    /// First sample index after `k` whose `axis` coordinate leaves the block
    /// `b` (cells `[b << shift, (b + 1) << shift)`). `i64::MAX` if never.
    #[inline(always)]
    fn exit_k(&self, axis: usize, b: i64, shift: u32) -> i64 {
        let (o, d, n) = (self.o[axis], self.d[axis], self.n);
        let s = shift + FRAC;
        if d > 0 {
            let x = (b + 1) << s;
            // smallest k with o + floor(k d / n) >= x  <=>  k >= ceil((x - o) n / d)
            ((x - o) * n + d - 1).div_euclid(d)
        } else if d < 0 {
            let x = b << s;
            // smallest k with o + floor(k d / n) < x  <=>  k > (o - x) n / -d
            ((o - x) * n).div_euclid(-d) + 1
        } else {
            i64::MAX
        }
    }
}

/// Incremental exact evaluation of `o + floor(k · d / n)`.
#[derive(Copy, Clone, Debug)]
struct Axis {
    v: i64,
    q: i64,
    r: i64,
    acc: i64,
}

impl Axis {
    #[inline(always)]
    fn new(ray: &Ray, axis: usize, k: i64) -> Axis {
        let d = ray.d[axis];
        let (q, r) = (d.div_euclid(ray.n), d.rem_euclid(ray.n));
        Axis {
            v: ray.at(axis, k),
            q,
            r,
            acc: (k * r).rem_euclid(ray.n),
        }
    }
    #[inline(always)]
    fn seek(&mut self, ray: &Ray, axis: usize, k: i64) {
        self.v = ray.at(axis, k);
        self.acc = (k * self.r).rem_euclid(ray.n);
    }
    #[inline(always)]
    fn step(&mut self, n: i64) {
        self.v += self.q;
        self.acc += self.r;
        if self.acc >= n {
            self.acc -= n;
            self.v += 1;
        }
    }
}

/// Outcome of one ray plus work counters.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct RayResult {
    pub visible: bool,
    /// Base cells tested.
    pub cells: u32,
    /// Pyramid block tests.
    pub blocks: u32,
}

#[inline(always)]
fn cell_height_q16<L: Layout>(g: &HeightGrid<L>, x: i64, y: i64) -> i64 {
    (g.height((x >> FRAC) as u32, (y >> FRAC) as u32) as i64) << HEIGHT_TO_Q16_SHIFT
}

/// Plain march: tests every interior sample's cell.
pub fn trace_linear<L: Layout>(g: &HeightGrid<L>, ray: &Ray) -> RayResult {
    let n = ray.n;
    let mut ax = Axis::new(ray, 0, 1);
    let mut ay = Axis::new(ray, 1, 1);
    let mut az = Axis::new(ray, 2, 1);
    let mut cells = 0;
    for _ in 1..n {
        cells += 1;
        if az.v < cell_height_q16(g, ax.v, ay.v) {
            return RayResult {
                visible: false,
                cells,
                blocks: 0,
            };
        }
        ax.step(n);
        ay.step(n);
        az.step(n);
    }
    RayResult {
        visible: true,
        cells,
        blocks: 0,
    }
}

/// Max-mip march: tries the coarsest pyramid block first and skips it when
/// the ray's lowest sample inside it clears the block's max height. On a
/// failed block it descends a level, and it climbs again only after leaving
/// the failed parent block.
pub fn trace_maxmip<L: Layout>(g: &HeightGrid<L>, ray: &Ray, max_level: u32) -> RayResult {
    let top = max_level.min(g.top_level()).min(MAX_MIP_LEVEL) as usize;
    if top == 0 {
        return trace_linear(g, ray);
    }
    let n = ray.n;
    let mut k = 1i64;
    let mut ax = Axis::new(ray, 0, k);
    let mut ay = Axis::new(ray, 1, k);
    let mut az = Axis::new(ray, 2, k);
    // fail_exit[l]: sample index where the last failed level-l block ends;
    // levels below l run until k reaches it.
    let mut fail_exit = [0i64; MAX_MIP_LEVEL as usize + 2];
    let mut level = top;
    let (mut cells, mut blocks) = (0u32, 0u32);

    while k < n {
        if level == 0 {
            cells += 1;
            if az.v < cell_height_q16(g, ax.v, ay.v) {
                return RayResult {
                    visible: false,
                    cells,
                    blocks,
                };
            }
            k += 1;
            ax.step(n);
            ay.step(n);
            az.step(n);
        } else {
            blocks += 1;
            let shift = level as u32;
            let (bx, by) = (ax.v >> (FRAC + shift), ay.v >> (FRAC + shift));
            let ke = ray
                .exit_k(0, bx, shift)
                .min(ray.exit_k(1, by, shift))
                .min(n);
            let zmin = az.v.min(ray.at(2, ke - 1));
            let lvl = &g.levels[level];
            let hmax = (lvl.get(bx as u32, by as u32) as i64) << HEIGHT_TO_Q16_SHIFT;
            if zmin >= hmax {
                k = ke;
                ax.seek(ray, 0, k);
                ay.seek(ray, 1, k);
                az.seek(ray, 2, k);
            } else {
                fail_exit[level] = ke;
                level -= 1;
                continue;
            }
        }
        while level < top && k >= fail_exit[level + 1] {
            level += 1;
        }
    }
    RayResult {
        visible: true,
        cells,
        blocks,
    }
}

/// Which memory layout a grid uses.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutKind {
    Linear,
    Tiled16,
    Morton,
}

impl LayoutKind {
    pub const ALL: [LayoutKind; 3] = [LayoutKind::Linear, LayoutKind::Tiled16, LayoutKind::Morton];

    pub fn name(self) -> &'static str {
        match self {
            LayoutKind::Linear => Linear::NAME,
            LayoutKind::Tiled16 => Tiled::NAME,
            LayoutKind::Morton => Morton::NAME,
        }
    }
}

/// A LOS kernel configuration: layout plus whether max-mip skipping is on.
#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct LosVariant {
    pub layout: LayoutKind,
    pub maxmip: bool,
}

impl LosVariant {
    pub fn all() -> Vec<LosVariant> {
        LayoutKind::ALL
            .iter()
            .flat_map(|&layout| {
                [false, true]
                    .into_iter()
                    .map(move |maxmip| LosVariant { layout, maxmip })
            })
            .collect()
    }

    pub fn label(&self) -> String {
        format!(
            "{}{}",
            self.layout.name(),
            if self.maxmip { "+maxmip" } else { "" }
        )
    }

    /// Parses labels like `tiled16+maxmip` or `linear`.
    pub fn parse(s: &str) -> Option<LosVariant> {
        let (name, maxmip) = match s.strip_suffix("+maxmip") {
            Some(n) => (n, true),
            None => (s, false),
        };
        let layout = LayoutKind::ALL.into_iter().find(|l| l.name() == name)?;
        Some(LosVariant { layout, maxmip })
    }
}

/// A heightfield in any of the three layouts (runtime choice, static
/// dispatch inside the kernels).
pub enum AnyGrid {
    Linear(HeightGrid<Linear>),
    Tiled16(HeightGrid<Tiled>),
    Morton(HeightGrid<Morton>),
}

impl AnyGrid {
    pub fn build(kind: LayoutKind, dim: u32, row_major: &[u16], levels: u32) -> AnyGrid {
        match kind {
            LayoutKind::Linear => {
                AnyGrid::Linear(HeightGrid::from_row_major(dim, row_major, levels))
            }
            LayoutKind::Tiled16 => {
                AnyGrid::Tiled16(HeightGrid::from_row_major(dim, row_major, levels))
            }
            LayoutKind::Morton => {
                AnyGrid::Morton(HeightGrid::from_row_major(dim, row_major, levels))
            }
        }
    }

    pub fn kind(&self) -> LayoutKind {
        match self {
            AnyGrid::Linear(_) => LayoutKind::Linear,
            AnyGrid::Tiled16(_) => LayoutKind::Tiled16,
            AnyGrid::Morton(_) => LayoutKind::Morton,
        }
    }

    /// Traces a batch, writing one result per ray.
    pub fn trace_batch(&self, rays: &[Ray], maxmip: Option<u32>, out: &mut [RayResult]) {
        fn run<L: Layout>(
            g: &HeightGrid<L>,
            rays: &[Ray],
            mip: Option<u32>,
            out: &mut [RayResult],
        ) {
            match mip {
                Some(top) => {
                    for (r, o) in rays.iter().zip(out) {
                        *o = trace_maxmip(g, r, top);
                    }
                }
                None => {
                    for (r, o) in rays.iter().zip(out) {
                        *o = trace_linear(g, r);
                    }
                }
            }
        }
        match self {
            AnyGrid::Linear(g) => run(g, rays, maxmip, out),
            AnyGrid::Tiled16(g) => run(g, rays, maxmip, out),
            AnyGrid::Morton(g) => run(g, rays, maxmip, out),
        }
    }

    pub fn trace(&self, ray: &Ray, maxmip: Option<u32>) -> RayResult {
        let mut out = [RayResult::default()];
        self.trace_batch(std::slice::from_ref(ray), maxmip, &mut out);
        out[0]
    }

    pub fn height(&self, x: u32, y: u32) -> u16 {
        match self {
            AnyGrid::Linear(g) => g.height(x, y),
            AnyGrid::Tiled16(g) => g.height(x, y),
            AnyGrid::Morton(g) => g.height(x, y),
        }
    }

    pub fn dim(&self) -> u32 {
        match self {
            AnyGrid::Linear(g) => g.dim(),
            AnyGrid::Tiled16(g) => g.dim(),
            AnyGrid::Morton(g) => g.dim(),
        }
    }

    pub fn base_bytes(&self) -> usize {
        match self {
            AnyGrid::Linear(g) => g.base_bytes(),
            AnyGrid::Tiled16(g) => g.base_bytes(),
            AnyGrid::Morton(g) => g.base_bytes(),
        }
    }

    pub fn pyramid_bytes(&self) -> usize {
        match self {
            AnyGrid::Linear(g) => g.pyramid_bytes(),
            AnyGrid::Tiled16(g) => g.pyramid_bytes(),
            AnyGrid::Morton(g) => g.pyramid_bytes(),
        }
    }
}

/// Eye height of an observer and centre-mass height of a target (m · 1000).
pub const EYE_MILLI: i64 = 1700;
pub const TARGET_MILLI: i64 = 1000;

/// Builds a ray from a ground point `from` (Q16.16 raw xy) along `angle`
/// for `len`, eye height at the start and target height at the end. The end
/// is clamped 1 m inside the grid.
pub fn ray_from(
    heights: impl Fn(u32, u32) -> u16,
    dim: u32,
    from: [i64; 2],
    angle: Fx,
    len: Fx,
) -> Ray {
    let one = 1i64 << FRAC;
    let lo = one;
    let hi = ((dim as i64 - 1) << FRAC) - 1;
    let (s, c) = angle.sin_cos();
    let len = len.raw() as i64;
    let ax = from[0].clamp(lo, hi);
    let ay = from[1].clamp(lo, hi);
    let bx = (ax + ((c.raw() as i64 * len) >> FRAC)).clamp(lo, hi);
    let by = (ay + ((s.raw() as i64 * len) >> FRAC)).clamp(lo, hi);
    let ground = |x: i64, y: i64| {
        (heights((x >> FRAC) as u32, (y >> FRAC) as u32) as i64) << HEIGHT_TO_Q16_SHIFT
    };
    let milli = |m: i64| (m << FRAC) / 1000;
    Ray::new(
        [ax, ay, ground(ax, ay) + milli(EYE_MILLI)],
        [bx, by, ground(bx, by) + milli(TARGET_MILLI)],
    )
}

/// `count` seeded rays with lengths in `[min_m, max_m]`, starting anywhere
/// in the grid with a margin of `max_m` where the map allows, so few rays
/// get clamped.
pub fn random_rays(grid: &AnyGrid, count: usize, min_m: u32, max_m: u32, seed: u64) -> Vec<Ray> {
    let dim = grid.dim();
    let margin = if dim > 3 * max_m { max_m } else { 1 };
    let span = (dim - 2 * margin) as u64;
    (0..count as u64)
        .map(|i| {
            let r = |j: u64| mix64(mix64(seed ^ 0x105_u64.wrapping_mul(0x9E37)) ^ (i << 3) ^ j);
            let x = (((r(0) >> 32) * span) >> 32) as i64 + margin as i64;
            let y = (((r(1) >> 32) * span) >> 32) as i64 + margin as i64;
            let fx = (r(2) & 0xFFFF) as i64;
            let fy = (r(3) & 0xFFFF) as i64;
            let angle = Fx::from_raw((((r(4) >> 32) * Fx::TAU.raw() as u64) >> 32) as i32);
            let lspan = ((max_m - min_m) as u64) << FRAC;
            let len =
                Fx::from_raw((((min_m as u64) << FRAC) + (((r(5) >> 32) * lspan) >> 32)) as i32);
            ray_from(
                |x, y| grid.height(x, y),
                dim,
                [(x << FRAC) | fx, (y << FRAC) | fy],
                angle,
                len,
            )
        })
        .collect()
}

/// Order-sensitive checksum of visibility results.
pub fn visibility_checksum(results: &[RayResult]) -> u64 {
    results
        .iter()
        .enumerate()
        .filter(|(_, r)| r.visible)
        .fold(0u64, |h, (i, _)| h ^ mix64(i as u64))
}

// ------------------------------------------------------------- benchmark --

/// Settings for [`run_bench`].
#[derive(Clone, Debug, serde::Serialize)]
pub struct LosBenchConfig {
    pub map_m: u32,
    pub rays: usize,
    pub reps: u32,
    pub mip_levels: u32,
    pub min_m: u32,
    pub max_m: u32,
    pub seed: u64,
    pub threads: usize,
}

/// Timing for one kernel variant.
#[derive(Clone, Debug, serde::Serialize)]
pub struct LosVariantResult {
    pub variant: String,
    pub layout: LayoutKind,
    pub maxmip: bool,
    /// Per-ray cost with `threads` workers busy: wall × threads / rays
    /// (median over reps). This is the §2.2 gate metric.
    pub cpu_us_per_ray: f64,
    /// Throughput view: wall / rays with `threads` workers.
    pub wall_us_per_ray: f64,
    /// One worker, no contention.
    pub single_thread_us_per_ray: f64,
    pub visible_fraction: f64,
    pub mean_cells_tested: f64,
    pub mean_block_tests: f64,
    pub checksum: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct LosReport {
    pub config: LosBenchConfig,
    pub cell_m: u32,
    pub base_mib: f64,
    pub pyramid_mib: f64,
    pub terrain_gen_ms: f64,
    pub variants: Vec<LosVariantResult>,
    /// Every variant returned identical visibility for every ray.
    pub results_identical: bool,
    pub fastest: String,
    pub notes: Vec<String>,
}

fn time_batch(
    pool: &rayon::ThreadPool,
    grid: &AnyGrid,
    rays: &[Ray],
    mip: Option<u32>,
    out: &mut [RayResult],
) -> std::time::Duration {
    use rayon::prelude::*;
    const BATCH: usize = 256;
    let t = std::time::Instant::now();
    pool.install(|| {
        rays.par_chunks(BATCH)
            .zip(out.par_chunks_mut(BATCH))
            .for_each(|(r, o)| grid.trace_batch(r, mip, o));
    });
    t.elapsed()
}

/// Builds the heightfield in every layout, generates seeded rays, and times
/// each variant. Variants are interleaved within each rep so drift (thermal,
/// background load) hits all of them alike.
pub fn run_bench(cfg: &LosBenchConfig) -> anyhow::Result<LosReport> {
    anyhow::ensure!(
        cfg.map_m.is_power_of_two(),
        "--map-m must be a power of two"
    );
    anyhow::ensure!(
        cfg.min_m <= cfg.max_m && cfg.max_m < cfg.map_m,
        "bad ray lengths"
    );
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(cfg.threads)
        .build()?;
    let single = rayon::ThreadPoolBuilder::new().num_threads(1).build()?;

    let t = std::time::Instant::now();
    let hm = pool.install(|| crate::grid::generate_terrain(cfg.map_m, cfg.seed));
    let terrain_gen_ms = t.elapsed().as_secs_f64() * 1e3;
    let grids: Vec<AnyGrid> = LayoutKind::ALL
        .iter()
        .map(|&k| AnyGrid::build(k, cfg.map_m, &hm, cfg.mip_levels))
        .collect();
    drop(hm);
    let rays = random_rays(
        &grids[0],
        cfg.rays,
        cfg.min_m,
        cfg.max_m,
        cfg.seed ^ 0x4C4F53,
    );

    let variants = LosVariant::all();
    let mut out = vec![RayResult::default(); rays.len()];
    let mut walls: Vec<Vec<f64>> = vec![Vec::new(); variants.len()];
    let mut singles: Vec<f64> = vec![0.0; variants.len()];
    let mut summaries = Vec::new();

    let mip_of = |v: &LosVariant| v.maxmip.then_some(cfg.mip_levels);
    let grid_of =
        |v: &LosVariant| &grids[LayoutKind::ALL.iter().position(|&k| k == v.layout).unwrap()];

    // Warm-up + correctness pass (also gives the work counters).
    for v in &variants {
        time_batch(&pool, grid_of(v), &rays, mip_of(v), &mut out);
        let n = out.len() as f64;
        summaries.push((
            out.iter().filter(|r| r.visible).count() as f64 / n,
            out.iter().map(|r| r.cells as f64).sum::<f64>() / n,
            out.iter().map(|r| r.blocks as f64).sum::<f64>() / n,
            visibility_checksum(&out),
        ));
    }
    for _ in 0..cfg.reps.max(1) {
        for (i, v) in variants.iter().enumerate() {
            let d = time_batch(&pool, grid_of(v), &rays, mip_of(v), &mut out);
            walls[i].push(d.as_secs_f64());
        }
    }
    // Single-thread reference on a quarter of the rays (one pass).
    let sub = (rays.len() / 4).max(1);
    for (i, v) in variants.iter().enumerate() {
        let d = time_batch(
            &single,
            grid_of(v),
            &rays[..sub],
            mip_of(v),
            &mut out[..sub],
        );
        singles[i] = d.as_secs_f64() * 1e6 / sub as f64;
    }

    let n = rays.len() as f64;
    let results: Vec<LosVariantResult> = variants
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let wall = crate::stats::median(&walls[i]);
            let (vis, cells, blocks, sum) = summaries[i];
            LosVariantResult {
                variant: v.label(),
                layout: v.layout,
                maxmip: v.maxmip,
                cpu_us_per_ray: wall * 1e6 * cfg.threads as f64 / n,
                wall_us_per_ray: wall * 1e6 / n,
                single_thread_us_per_ray: singles[i],
                visible_fraction: vis,
                mean_cells_tested: cells,
                mean_block_tests: blocks,
                checksum: format!("{sum:016x}"),
            }
        })
        .collect();
    let results_identical = summaries
        .windows(2)
        .all(|w| w[0].3 == w[1].3 && w[0].0 == w[1].0);
    let fastest = results
        .iter()
        .min_by(|a, b| a.cpu_us_per_ray.total_cmp(&b.cpu_us_per_ray))
        .map(|r| r.variant.clone())
        .unwrap_or_default();
    let mib = |b: usize| b as f64 / (1024.0 * 1024.0);
    Ok(LosReport {
        config: cfg.clone(),
        cell_m: 1,
        base_mib: mib(grids[0].base_bytes()),
        pyramid_mib: mib(grids[0].pyramid_bytes()),
        terrain_gen_ms,
        variants: results,
        results_identical,
        fastest,
        notes: vec![
            "Heightfield only: the building-occluder BVH in the §2.2 gate is out of scope for M0 here.".into(),
            "Rays start anywhere on the map (cold-cache, pessimistic); real perception rays cluster around the front.".into(),
            "Cells are flat-topped 1 m squares, u16 heights in 1/64 m; one sample per metre along the major axis.".into(),
        ],
    })
}

/// Picks the fastest variant with a short calibration run (`rays` rays per
/// variant). Used by the composed tick.
pub fn calibrate_fastest(
    pool: &rayon::ThreadPool,
    hm: &[u16],
    dim: u32,
    levels: u32,
    rays: usize,
    seed: u64,
) -> LosVariant {
    let mut best = (
        f64::MAX,
        LosVariant {
            layout: LayoutKind::Tiled16,
            maxmip: true,
        },
    );
    for layout in LayoutKind::ALL {
        let g = AnyGrid::build(layout, dim, hm, levels);
        let rs = random_rays(&g, rays, 300, 600, seed);
        let mut out = vec![RayResult::default(); rs.len()];
        for maxmip in [false, true] {
            let mip = maxmip.then_some(levels);
            time_batch(pool, &g, &rs, mip, &mut out);
            let t = (0..3)
                .map(|_| time_batch(pool, &g, &rs, mip, &mut out).as_secs_f64())
                .fold(f64::MAX, f64::min);
            if t < best.0 {
                best = (t, LosVariant { layout, maxmip });
            }
        }
    }
    best.1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::generate_terrain;

    /// Reference: direct formula per sample, no incremental stepping.
    fn trace_reference(g: &AnyGrid, ray: &Ray) -> bool {
        (1..ray.n).all(|k| {
            let (x, y, z) = (ray.at(0, k), ray.at(1, k), ray.at(2, k));
            z >= (g.height((x >> FRAC) as u32, (y >> FRAC) as u32) as i64) << HEIGHT_TO_Q16_SHIFT
        })
    }

    fn grids(dim: u32, seed: u64) -> Vec<AnyGrid> {
        let hm = generate_terrain(dim, seed);
        LayoutKind::ALL
            .iter()
            .map(|&k| AnyGrid::build(k, dim, &hm, 8))
            .collect()
    }

    #[test]
    fn all_layouts_and_maxmip_agree_with_reference() {
        let dim = 1024;
        let gs = grids(dim, 11);
        let rays = random_rays(&gs[0], 4000, 20, 600, 99);
        let mut visible = 0;
        for ray in &rays {
            let want = trace_reference(&gs[0], ray);
            visible += want as u32;
            for g in &gs {
                assert_eq!(
                    g.trace(ray, None).visible,
                    want,
                    "{:?} linear {ray:?}",
                    g.kind()
                );
                for top in [1, 3, 6, 8] {
                    assert_eq!(
                        g.trace(ray, Some(top)).visible,
                        want,
                        "{:?} maxmip L{top} {ray:?}",
                        g.kind()
                    );
                }
            }
        }
        // The terrain must give a real mix, or the test proves nothing.
        assert!(visible > 200 && visible < 3800, "visible {visible}/4000");
    }

    #[test]
    fn maxmip_skips_on_flat_ground() {
        let dim = 512;
        let hm = vec![100u16; (dim * dim) as usize];
        let g = AnyGrid::build(LayoutKind::Tiled16, dim, &hm, 6);
        let one = 1i64 << FRAC;
        let z = (100i64 << HEIGHT_TO_Q16_SHIFT) + one;
        let ray = Ray::new([10 * one, 20 * one, z], [490 * one, 300 * one, z]);
        let lin = g.trace(&ray, None);
        let mip = g.trace(&ray, Some(6));
        assert!(lin.visible && mip.visible);
        assert!(
            mip.cells == 0 && mip.blocks < lin.cells / 4,
            "{mip:?} vs {lin:?}"
        );
    }

    #[test]
    fn exit_k_is_exact() {
        let gs = grids(256, 3);
        let rays = random_rays(&gs[0], 500, 5, 200, 5);
        for ray in &rays {
            for shift in 0..6u32 {
                let mut k = 1;
                while k < ray.n {
                    let s = FRAC + shift;
                    let (bx, by) = (ray.at(0, k) >> s, ray.at(1, k) >> s);
                    let ke = ray.exit_k(0, bx, shift).min(ray.exit_k(1, by, shift));
                    assert!(ke > k);
                    for j in k..ke.min(ray.n + 1) {
                        assert_eq!((ray.at(0, j) >> s, ray.at(1, j) >> s), (bx, by));
                    }
                    if ke <= ray.n {
                        assert_ne!((ray.at(0, ke) >> s, ray.at(1, ke) >> s), (bx, by));
                    }
                    k = ke;
                }
            }
        }
    }

    #[test]
    fn variant_labels_round_trip() {
        for v in LosVariant::all() {
            assert_eq!(LosVariant::parse(&v.label()), Some(v));
        }
        assert_eq!(LosVariant::parse("nope"), None);
    }
}
