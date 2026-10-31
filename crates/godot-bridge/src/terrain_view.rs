//! Presenter copy of the match terrain and its chunked mesh (§5.10.7, S1
//! "Terrain mesh that rebuilds only dug chunks"). Plain Rust, unit tested.
//!
//! [`TerrainStore`] holds the 4 m base grid (f32) and the detail tiles the
//! sim reported, so the presenter can answer `height_at` (picking,
//! overlays, effects) without touching the sim. The map is meshed in
//! [`CHUNK_M`] chunks, each its own `RenderingServer` instance; a changed
//! detail tile marks only the chunks it touches for a rebuild.
//!
//! Inside a chunk every 16 m tile slot is meshed at the base 4 m spacing,
//! or at the 0.25 m detail spacing when it, or the slot to its east, north
//! or north-east, has a detail tile (its last row and column of vertices
//! are that neighbour's first detail points). Along any edge between a
//! base-resolution slot and a detail one the detail points are the base
//! surface, which is linear along the grid line, so the two meshes meet
//! without cracks.
//!
//! This is CPU re-meshing, not yet the GPU height-texture terrain of §4.1
//! rule 6; with chunks and per-frame rebuild caps it is cheap enough for
//! S1's trench and crater counts.

use std::collections::{BTreeMap, BTreeSet};

use frontline_sim::terrain::{FLAG_WOODS, SPACING_M};
use frontline_sim::{Heightfield, TILE_M, TILE_POINTS};

use crate::match_frame::TileUpdate;
use crate::meshgen::MeshData;

/// Chunk side (m): 4 × 4 tile slots.
pub const CHUNK_M: i32 = 64;
const SLOTS_PER_CHUNK: i32 = CHUNK_M / TILE_M;
const DETAIL_STEP: f32 = 1.0 / 4.0;
const TP: usize = TILE_POINTS as usize;

pub struct TerrainStore {
    /// Base heights, row-major (`y · dim + x`), sim metres.
    base: Vec<f32>,
    /// Woods weight per grid point (0 or 1).
    woods: Vec<f32>,
    dim: usize,
    spacing: f32,
    size_m: f32,
    half: f32,
    tiles_per_side: i32,
    tiles: BTreeMap<(u16, u16), Vec<f32>>,
}

impl TerrainStore {
    pub fn new(hf: &Heightfield) -> TerrainStore {
        let size_m = hf.size_m() as f32;
        TerrainStore {
            base: hf.heights().iter().map(|h| h.to_f32()).collect(),
            woods: hf
                .flags()
                .iter()
                .map(|f| if f & FLAG_WOODS != 0 { 1.0 } else { 0.0 })
                .collect(),
            dim: hf.dim() as usize,
            spacing: SPACING_M as f32,
            size_m,
            half: size_m * 0.5,
            tiles_per_side: hf.size_m() / TILE_M,
            tiles: BTreeMap::new(),
        }
    }

    pub fn size_m(&self) -> f32 {
        self.size_m
    }

    pub fn half(&self) -> f32 {
        self.half
    }

    pub fn chunks_per_side(&self) -> i32 {
        (self.size_m as i32 / CHUNK_M).max(1)
    }

    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    pub fn base_heights(&self) -> (&[f32], usize) {
        (&self.base, self.dim)
    }

    fn grid(&self, gx: usize, gy: usize) -> f32 {
        let m = self.dim - 1;
        self.base[gy.min(m) * self.dim + gx.min(m)]
    }

    /// Bilinear base surface at sim `(x, y)` (clamped to the map).
    pub fn base_at(&self, x: f32, y: f32) -> f32 {
        bilinear(&self.base, self.dim, self.spacing, x, y)
    }

    fn woods_at(&self, x: f32, y: f32) -> f32 {
        bilinear(&self.woods, self.dim, self.spacing, x, y)
    }

    /// Height of detail point `(i, j)` (0.25 m units from the map origin).
    pub fn detail_point(&self, i: i32, j: i32) -> f32 {
        let last = self.size_m as i32 * 4;
        let (i, j) = (i.clamp(0, last), j.clamp(0, last));
        let key = ((i / TILE_POINTS) as u16, (j / TILE_POINTS) as u16);
        match self.tiles.get(&key) {
            Some(t) => t[(j % TILE_POINTS) as usize * TP + (i % TILE_POINTS) as usize],
            None => self.base_at(i as f32 * DETAIL_STEP, j as f32 * DETAIL_STEP),
        }
    }

    fn has_tile_point(&self, i: i32, j: i32) -> bool {
        if i < 0 || j < 0 {
            return false;
        }
        self.tiles
            .contains_key(&((i / TILE_POINTS) as u16, (j / TILE_POINTS) as u16))
    }

    /// Combined surface at sim `(x, y)`: bilinear over detail points where
    /// a tile is near, else the base (as the sim's `height_at`).
    pub fn height_at(&self, x: f32, y: f32) -> f32 {
        if self.tiles.is_empty() {
            return self.base_at(x, y);
        }
        let x = x.clamp(0.0, self.size_m);
        let y = y.clamp(0.0, self.size_m);
        let last = self.size_m as i32 * 4 - 1;
        let ci = ((x * 4.0) as i32).min(last);
        let cj = ((y * 4.0) as i32).min(last);
        let near = [(0, 0), (1, 0), (0, 1), (1, 1)]
            .iter()
            .any(|&(di, dj)| self.has_tile_point(ci + di, cj + dj));
        if !near {
            return self.base_at(x, y);
        }
        let (fx, fy) = (x * 4.0 - ci as f32, y * 4.0 - cj as f32);
        let a = lerp(self.detail_point(ci, cj), self.detail_point(ci + 1, cj), fx);
        let b = lerp(
            self.detail_point(ci, cj + 1),
            self.detail_point(ci + 1, cj + 1),
            fx,
        );
        lerp(a, b, fy)
    }

    /// Godot `(x, z)` in, Godot Y out.
    pub fn height_at_godot(&self, x: f32, z: f32) -> f32 {
        self.height_at(x + self.half, -z + self.half)
    }

    /// Applies a tile update and returns the chunks whose mesh changed.
    pub fn apply(&mut self, u: &TileUpdate, dirty: &mut BTreeSet<(i32, i32)>) {
        match &u.heights {
            Some(h) if h.len() == TP * TP => {
                self.tiles.insert((u.x, u.y), h.clone());
            }
            _ => {
                self.tiles.remove(&(u.x, u.y));
            }
        }
        // The tile's own slot and the slots whose last row or column are
        // its first points.
        for (dx, dy) in [(0, 0), (-1, 0), (0, -1), (-1, -1)] {
            let (sx, sy) = (u.x as i32 + dx, u.y as i32 + dy);
            if sx >= 0 && sy >= 0 && sx < self.tiles_per_side && sy < self.tiles_per_side {
                dirty.insert((sx / SLOTS_PER_CHUNK, sy / SLOTS_PER_CHUNK));
            }
        }
    }

    fn slot_needs_detail(&self, sx: i32, sy: i32) -> bool {
        [(0, 0), (1, 0), (0, 1), (1, 1)].iter().any(|&(dx, dy)| {
            let (x, y) = (sx + dx, sy + dy);
            x >= 0
                && y >= 0
                && x < self.tiles_per_side
                && y < self.tiles_per_side
                && self.tiles.contains_key(&(x as u16, y as u16))
        })
    }

    /// Mesh of chunk `(cx, cy)` in Godot space (map centred on the origin).
    pub fn chunk_mesh(&self, cx: i32, cy: i32) -> MeshData {
        let mut m = MeshData::default();
        let sx0 = cx * SLOTS_PER_CHUNK;
        let sy0 = cy * SLOTS_PER_CHUNK;
        let all_base = (0..SLOTS_PER_CHUNK)
            .all(|j| (0..SLOTS_PER_CHUNK).all(|i| !self.slot_needs_detail(sx0 + i, sy0 + j)));
        if all_base {
            // One base grid for the whole chunk.
            let g0 = (cx * CHUNK_M) as usize / SPACING_M as usize;
            let h0 = (cy * CHUNK_M) as usize / SPACING_M as usize;
            let n = (CHUNK_M / SPACING_M) as usize;
            self.base_grid(&mut m, g0, h0, n);
            return m;
        }
        for j in 0..SLOTS_PER_CHUNK {
            for i in 0..SLOTS_PER_CHUNK {
                let (sx, sy) = (sx0 + i, sy0 + j);
                if sx >= self.tiles_per_side || sy >= self.tiles_per_side {
                    continue;
                }
                if self.slot_needs_detail(sx, sy) {
                    self.detail_grid(&mut m, sx, sy);
                } else {
                    let n = (TILE_M / SPACING_M) as usize;
                    let g0 = (sx * TILE_M) as usize / SPACING_M as usize;
                    let h0 = (sy * TILE_M) as usize / SPACING_M as usize;
                    self.base_grid(&mut m, g0, h0, n);
                }
            }
        }
        m
    }

    /// `n × n` base quads from grid node `(g0, h0)`.
    fn base_grid(&self, m: &mut MeshData, g0: usize, h0: usize, n: usize) {
        let first = m.positions.len() as i32;
        let s = self.spacing;
        for j in 0..=n {
            for i in 0..=n {
                let (gx, gy) = (g0 + i, h0 + j);
                let h = self.grid(gx, gy);
                let dx = (self.grid(gx + 1, gy) - self.grid(gx.saturating_sub(1), gy)) / (2.0 * s);
                let dy = (self.grid(gx, gy + 1) - self.grid(gx, gy.saturating_sub(1))) / (2.0 * s);
                let (x, y) = (gx as f32 * s, gy as f32 * s);
                let c = ground_color(h, dx, dy, 0.0, self.woods_at(x, y));
                m.vert(
                    [x - self.half, h, -(y - self.half)],
                    [-dx, 1.0, dy],
                    c,
                    None,
                );
            }
        }
        quads(m, first, n + 1);
    }

    /// 64 × 64 detail quads of tile slot `(sx, sy)`.
    fn detail_grid(&self, m: &mut MeshData, sx: i32, sy: i32) {
        let first = m.positions.len() as i32;
        let (i0, j0) = (sx * TILE_POINTS, sy * TILE_POINTS);
        let n = TILE_POINTS;
        // Heights with a one-point border for normals.
        let w = (n + 3) as usize;
        let mut h = vec![0.0f32; w * w];
        for j in 0..w {
            for i in 0..w {
                h[j * w + i] = self.detail_point(i0 + i as i32 - 1, j0 + j as i32 - 1);
            }
        }
        let at = |i: i32, j: i32| h[(j + 1) as usize * w + (i + 1) as usize];
        for j in 0..=n {
            for i in 0..=n {
                let y = at(i, j);
                let dx = (at(i + 1, j) - at(i - 1, j)) / (2.0 * DETAIL_STEP);
                let dy = (at(i, j + 1) - at(i, j - 1)) / (2.0 * DETAIL_STEP);
                let (x, yy) = ((i0 + i) as f32 * DETAIL_STEP, (j0 + j) as f32 * DETAIL_STEP);
                let dug = self.base_at(x, yy) - y;
                let c = ground_color(y, dx, dy, dug, self.woods_at(x, yy));
                m.vert(
                    [x - self.half, y, -(yy - self.half)],
                    [-dx, 1.0, dy],
                    c,
                    None,
                );
            }
        }
        quads(m, first, (n + 1) as usize);
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn bilinear(v: &[f32], dim: usize, spacing: f32, x: f32, y: f32) -> f32 {
    if dim < 2 {
        return 0.0;
    }
    let max = (dim - 1) as f32;
    let gx = (x / spacing).clamp(0.0, max);
    let gy = (y / spacing).clamp(0.0, max);
    let ix = (gx as usize).min(dim - 2);
    let iy = (gy as usize).min(dim - 2);
    let (fx, fy) = (gx - ix as f32, gy - iy as f32);
    let h = |i: usize, j: usize| v[j * dim + i];
    lerp(
        lerp(h(ix, iy), h(ix + 1, iy), fx),
        lerp(h(ix, iy + 1), h(ix + 1, iy + 1), fx),
        fy,
    )
}

/// Index buffer for an `n × n`-vertex grid starting at vertex `first`
/// (rows run north in sim, so a quad (i,j)→(i+1,j)→(i+1,j+1) is
/// counter-clockwise seen from above; see `meshgen::terrain_mesh`).
fn quads(m: &mut MeshData, first: i32, n: usize) {
    let v = |i: usize, j: usize| first + (j * n + i) as i32;
    for j in 0..n - 1 {
        for i in 0..n - 1 {
            m.tri_ccw(v(i, j), v(i + 1, j), v(i + 1, j + 1));
            m.tri_ccw(v(i, j), v(i + 1, j + 1), v(i, j + 1));
        }
    }
}

/// sRGB ground colour: grass by height, dirt on slopes, woods floor, and
/// churned earth where the ground is dug (`dug` > 0, m below the base) or
/// heaped (`dug` < 0: parapets and crater rims).
pub fn ground_color(h: f32, dx: f32, dy: f32, dug: f32, woods: f32) -> [f32; 4] {
    let slope = (dx * dx + dy * dy).sqrt();
    let t = ((h - 10.0) / 30.0).clamp(0.0, 1.0);
    let grass = [0.29 + 0.07 * t, 0.35 + 0.04 * t, 0.18];
    let forest = [0.19, 0.24, 0.13];
    let dirt = [0.36, 0.31, 0.22];
    let mix = |a: [f32; 3], b: [f32; 3], k: f32| {
        [
            a[0] + (b[0] - a[0]) * k,
            a[1] + (b[1] - a[1]) * k,
            a[2] + (b[2] - a[2]) * k,
        ]
    };
    let mut c = mix(grass, forest, woods.clamp(0.0, 1.0) * 0.85);
    c = mix(c, dirt, (slope * 2.5).clamp(0.0, 1.0));
    if dug > 0.03 {
        // Fresh earth, darker and damper with depth.
        let k = (dug / 0.25).clamp(0.0, 1.0);
        let deep = (dug / 1.6).clamp(0.0, 1.0);
        let earth = mix([0.40, 0.31, 0.21], [0.23, 0.17, 0.11], deep);
        c = mix(c, earth, k);
    } else if dug < -0.03 {
        let k = (-dug / 0.15).clamp(0.0, 1.0);
        c = mix(c, [0.44, 0.36, 0.25], k * 0.9);
    }
    [c[0], c[1], c[2], 1.0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::match_frame::TickEvents;
    use crate::match_frame::tests_support::test_world;
    use frontline_sim::FxVec2;

    #[test]
    fn heights_match_the_sim_before_and_after_digging() {
        let mut w = test_world(false);
        let mut store = TerrainStore::new(w.terrain());
        let probe = |w: &frontline_sim::World, s: &TerrainStore, x: f32, y: f32| {
            let p = FxVec2::new(
                frontline_sim::Fx::from_milli((x * 1000.0) as i32),
                frontline_sim::Fx::from_milli((y * 1000.0) as i32),
            );
            (w.terrain().height_at(p).to_f32(), s.height_at(x, y))
        };
        for &(x, y) in &[(100.0, 100.0), (512.3, 400.7), (3.0, 1020.0)] {
            let (a, b) = probe(&w, &store, x, y);
            assert!((a - b).abs() < 0.01, "{x},{y}: sim {a} store {b}");
        }
        // A finished trench: the store follows the sim's dug surface.
        let a = FxVec2::from_int(300, 200);
        let b = FxVec2::from_int(318, 203);
        assert!(w.dig_trench_now(&[a, b], 1));
        let mut dirty = BTreeSet::new();
        for t in TickEvents::all_tiles(&w) {
            store.apply(&t, &mut dirty);
        }
        assert!(store.tile_count() > 0 && !dirty.is_empty());
        for k in 0..40 {
            let (x, y) = (300.0 + k as f32 * 0.45, 200.0 + k as f32 * 0.075);
            let (a, b) = probe(&w, &store, x, y);
            assert!((a - b).abs() < 0.01, "{x},{y}: sim {a} store {b}");
        }
        let (dug, _) = probe(&w, &store, 309.0, 201.5);
        assert!(store.base_at(309.0, 201.5) - dug > 1.2, "trench is deep");
        // Chunk (4, 3) holds (300, 200): it is meshed at detail resolution.
        assert!(dirty.contains(&(4, 3)));
        let m = store.chunk_mesh(4, 3);
        assert!(m.positions.len() > 65 * 65);
        let plain = store.chunk_mesh(0, 15);
        assert_eq!(plain.positions.len(), 17 * 17);
        // Removing the tiles brings back the base.
        for (x, y) in store.tiles.keys().copied().collect::<Vec<_>>() {
            store.apply(
                &TileUpdate {
                    x,
                    y,
                    heights: None,
                },
                &mut dirty,
            );
        }
        let h = store.height_at(309.0, 201.5);
        assert!((h - store.base_at(309.0, 201.5)).abs() < 1e-4);
        w.step(&[]);
    }

    #[test]
    fn detail_slot_edges_meet_base_slots() {
        // Synthetic store: a flat-ish base with one dug tile in the middle;
        // every vertex on a shared slot edge has the same height on both
        // sides.
        let w = test_world(false);
        let mut store = TerrainStore::new(w.terrain());
        let mut h = vec![0.0f32; TP * TP];
        for j in 0..TP {
            for i in 0..TP {
                let (x, y) = (
                    (10 * TILE_POINTS + i as i32) as f32 * DETAIL_STEP,
                    (10 * TILE_POINTS + j as i32) as f32 * DETAIL_STEP,
                );
                h[j * TP + i] = store.base_at(x, y) - if (20..40).contains(&i) { 1.0 } else { 0.0 };
            }
        }
        let mut dirty = BTreeSet::new();
        store.apply(
            &TileUpdate {
                x: 10,
                y: 10,
                heights: Some(h),
            },
            &mut dirty,
        );
        // Tile 10 is in chunk 2 (slots 8..12); its -x/-y neighbours too.
        assert_eq!(dirty.iter().copied().collect::<Vec<_>>(), vec![(2, 2)]);
        let m = store.chunk_mesh(2, 2);
        // Collect heights by planar position; duplicates must agree.
        let mut seen: BTreeMap<(i64, i64), f32> = BTreeMap::new();
        for p in &m.positions {
            let key = ((p[0] * 100.0).round() as i64, (p[2] * 100.0).round() as i64);
            if let Some(&y) = seen.get(&key) {
                assert!((y - p[1]).abs() < 1e-4, "crack at {key:?}: {y} vs {}", p[1]);
            } else {
                seen.insert(key, p[1]);
            }
        }
    }
}
