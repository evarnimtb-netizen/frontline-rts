//! Heightfield: the two-tier terrain of §5.10.1.
//!
//! * **Base tier:** a square grid of Q16.16 heights at 4 m spacing, flat,
//!   seeded integer value noise (bench) or generated from a map def (match),
//!   plus a per-point flag layer (`flags`, match maps only) that marks woods.
//!   It never changes after generation, so it is hashed once (at creation
//!   and on load).
//! * **Detail tier** ([`DetailTier`]): sparse 0.25 m tiles of 64 × 64 points
//!   (16 × 16 m), created the first time an edit changes a point in them
//!   (initialised by bilinear sampling of the base) and pooled under a cap.
//!   Each detail point belongs to exactly one tile; a point without a tile
//!   is the base surface sampled there.
//!
//! All sim queries ([`Heightfield::height_at`], [`Heightfield::segment_hit`],
//! [`Heightfield::line_of_sight`]) see the combined surface. Where no tile
//! is near they take the base-only path, which is bit-identical to the old
//! single-tier code, so bench worlds (never edited) keep their hashes.
//!
//! Edits ([`Heightfield::edit`], [`Heightfield::crater`], and the trench
//! stamp in `crate::dig`) are integer stamps applied serially, so terrain is
//! deterministic by construction. Changed tiles are recorded in a dirty list
//! that the world drains once per tick to re-hash them (one xxh3 per tile)
//! and to tell the presenter which chunks to rebuild. A derived per-grid-node
//! layer (`dug`: deepest dig in the node's 4 × 4 m cell) feeds the path
//! search's trench discount; it is rebuilt from the tiles on load.

use frontline_content::setup::MapSetup;
use serde::{Deserialize, Serialize};

use crate::ecs::SimId;
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::rng::{Purpose, range_fx, rng};

/// Heightfield generator.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum TerrainKind {
    /// Height 0 everywhere.
    Flat,
    /// Two octaves of seeded value noise (64 m lattice ±12 m, 16 m lattice ±2 m).
    #[default]
    Noise,
    /// Generated from a match setup's map def ([`Heightfield::generate_map`]);
    /// only valid for worlds made by `World::new_match`.
    Map,
}

/// Grid-point flag: inside a woods patch (cuts spotting, not bullets).
pub const FLAG_WOODS: u8 = 1;

/// Grid spacing in metres.
pub const SPACING_M: i32 = 4;

/// Detail points per metre (0.25 m spacing).
pub const DETAIL_PER_M: i32 = 4;
/// Raw Q16.16 bits per detail step: a detail coordinate is `raw >> 14`.
pub const DETAIL_SHIFT: u32 = 14;
/// Detail points per tile side.
pub const TILE_POINTS: i32 = 64;
/// Tile side in metres.
pub const TILE_M: i32 = TILE_POINTS / DETAIL_PER_M;
/// Most detail tiles a world may hold (§5.10.8 pool cap; 16 KB each).
pub const MAX_DETAIL_TILES: u32 = 512;
/// A grid node whose cell has been dug at least this deep (dm) is a trench
/// cell for the path search.
pub const TRENCH_NAV_DM: u8 = 10;

const NO_TILE: u32 = u32::MAX;
const TILE_LEN: usize = (TILE_POINTS * TILE_POINTS) as usize;
const LOG2_TILE_POINTS: u32 = 6;

/// One 16 × 16 m detail tile: 64 × 64 heights at 0.25 m. Tile `(x, y)`
/// owns detail points `x·64 .. x·64+63` (and likewise in y).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct DetailTile {
    pub(crate) x: u16,
    pub(crate) y: u16,
    /// Tick in which the tile last changed (the presenter can compare this
    /// against its own copy to catch up after skipped snapshots).
    pub(crate) changed_tick: u64,
    /// Row-major (y, then x) heights.
    pub(crate) heights: Vec<Fx>,
}

impl DetailTile {
    /// Tile coordinates `(x, y)`; the tile's origin is `(x, y) · TILE_M` m.
    #[inline]
    pub fn coords(&self) -> (u16, u16) {
        (self.x, self.y)
    }
    #[inline]
    pub fn changed_tick(&self) -> u64 {
        self.changed_tick
    }
    /// 64 × 64 heights, row-major: point `(i, j)` (east, north) is at
    /// `origin + (i, j) · 0.25 m` and stored at `j · 64 + i`.
    #[inline]
    pub fn heights(&self) -> &[Fx] {
        &self.heights
    }
}

/// The sparse detail tier (see the module docs). `tiles` is in creation
/// order, which is deterministic; the other fields are derived or
/// transient and are not saved.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct DetailTier {
    tiles: Vec<DetailTile>,
    /// Pool cap (at most [`MAX_DETAIL_TILES`]).
    cap: u32,
    /// Tile edits dropped because the pool was full of non-flat tiles.
    dropped: u32,
    /// Tile slot (`y · tiles_per_side + x`) → index into `tiles`, or
    /// `NO_TILE`; empty until the first tile exists. Derived.
    #[serde(skip)]
    index: Vec<u32>,
    /// Per base grid node: deepest dig below the base in its cell (dm,
    /// saturated); empty until the first tile exists. Derived.
    #[serde(skip)]
    dug: Vec<u8>,
    /// Tiles changed since the last [`Heightfield::take_dirty`]. Transient:
    /// always drained by the end of a tick.
    #[serde(skip)]
    dirty: Vec<(u16, u16)>,
}

impl DetailTier {
    fn new() -> DetailTier {
        DetailTier {
            cap: MAX_DETAIL_TILES,
            ..DetailTier::default()
        }
    }

    /// Tiles in creation order.
    #[inline]
    pub fn tiles(&self) -> &[DetailTile] {
        &self.tiles
    }
    #[inline]
    pub fn cap(&self) -> u32 {
        self.cap
    }
    /// Tile edits dropped because the pool was full.
    #[inline]
    pub fn dropped(&self) -> u32 {
        self.dropped
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Heightfield {
    size_m: i32,
    /// Points per side (`size_m / SPACING_M + 1`).
    dim: u32,
    heights: Vec<Fx>,
    /// Per grid point `FLAG_*` bits; empty when the generator sets none.
    flags: Vec<u8>,
    detail: DetailTier,
}

impl Heightfield {
    /// `size_m` must be a positive multiple of 64. [`TerrainKind::Map`] needs
    /// its map def, so here it gives flat ground; use
    /// [`Heightfield::generate_map`].
    pub fn generate(kind: TerrainKind, size_m: i32, seed: u64) -> Heightfield {
        assert!(
            size_m > 0 && size_m % 64 == 0,
            "map size must be a positive multiple of 64 m"
        );
        let dim = (size_m / SPACING_M + 1) as u32;
        let mut heights = vec![Fx::ZERO; (dim * dim) as usize];
        if kind == TerrainKind::Noise {
            let octaves = [(64, Fx::from_int(12), 0u32), (16, Fx::from_int(2), 1u32)];
            for gy in 0..dim {
                for gx in 0..dim {
                    let (x, y) = (gx as i32 * SPACING_M, gy as i32 * SPACING_M);
                    let mut h = Fx::ZERO;
                    for &(period, amp, octave) in &octaves {
                        h += lattice_noise(seed, x, y, period, amp, octave);
                    }
                    heights[(gy * dim + gx) as usize] = h;
                }
            }
        }
        Heightfield {
            size_m,
            dim,
            heights,
            flags: Vec::new(),
            detail: DetailTier::new(),
        }
    }

    /// Generates a match map: base height + smooth value-noise octaves +
    /// ridge/valley bands, and the woods flag layer. Integer-only, so every
    /// platform builds the same terrain. The map def has been validated
    /// (`MatchSetup::validate`).
    pub fn generate_map(map: &MapSetup) -> Heightfield {
        let size_m = map.size_m;
        assert!(
            size_m > 0 && size_m % 64 == 0,
            "map size must be a positive multiple of 64 m"
        );
        let t = &map.terrain;
        let dim = (size_m / SPACING_M + 1) as u32;
        let n = (dim * dim) as usize;
        let mut heights = vec![Fx::from_raw(t.base_height.0); n];
        let mut flags = vec![0u8; n];
        let woods: Vec<(FxVec2, u64)> = map
            .woods
            .iter()
            .map(|w| {
                let r = w.radius.0 as i64;
                (FxVec2::from(w.center), (r * r) as u64)
            })
            .collect();
        for gy in 0..dim {
            for gx in 0..dim {
                let (x, y) = (gx as i32 * SPACING_M, gy as i32 * SPACING_M);
                let p = FxVec2::from_int(x, y);
                let i = (gy * dim + gx) as usize;
                let mut h = heights[i];
                for (k, o) in t.octaves.iter().enumerate() {
                    let amp = Fx::from_raw(o.amplitude.0);
                    h += smooth_noise(t.seed, x, y, o.period_m, amp, k as u32);
                }
                for b in &t.bands {
                    h += band(
                        p,
                        b.from.into(),
                        b.to.into(),
                        Fx::from_raw(b.height.0),
                        Fx::from_raw(b.half_width.0),
                    );
                }
                heights[i] = h;
                if woods
                    .iter()
                    .any(|&(c, r_sq)| (p - c).length_sq_raw() <= r_sq)
                {
                    flags[i] |= FLAG_WOODS;
                }
            }
        }
        // S3c: a point-symmetric map takes the far half (x + y beyond the
        // size; on the anti-diagonal, the east end) from its mirror cell.
        if t.point_symmetric {
            let last = dim - 1;
            for gy in 0..dim {
                for gx in 0..dim {
                    let far = gx + gy > last || (gx + gy == last && gx > gy);
                    if far {
                        let i = (gy * dim + gx) as usize;
                        let j = ((last - gy) * dim + (last - gx)) as usize;
                        heights[i] = heights[j];
                    }
                }
            }
        }
        Heightfield {
            size_m,
            dim,
            heights,
            flags,
            detail: DetailTier::new(),
        }
    }

    /// Checks a loaded heightfield: `size_m` is a positive multiple of 64,
    /// `dim` matches it and there are exactly `dim²` heights.
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.size_m <= 0 || self.size_m % 64 != 0 {
            return Err("terrain size is not a positive multiple of 64 m");
        }
        if self.dim != (self.size_m / SPACING_M + 1) as u32 {
            return Err("terrain grid dimension disagrees with its size");
        }
        let n = self.dim as usize * self.dim as usize;
        if self.heights.len() != n {
            return Err("terrain height count disagrees with its grid dimension");
        }
        if !self.flags.is_empty() && self.flags.len() != n {
            return Err("terrain flag count disagrees with its grid dimension");
        }
        let d = &self.detail;
        if d.cap == 0 || d.cap > MAX_DETAIL_TILES || d.tiles.len() > d.cap as usize {
            return Err("terrain detail tile pool exceeds its cap");
        }
        let side = self.tiles_per_side();
        let mut seen = vec![false; (side * side) as usize];
        for t in &d.tiles {
            if t.x as i32 >= side || t.y as i32 >= side || t.heights.len() != TILE_LEN {
                return Err("terrain detail tile out of range");
            }
            let slot = &mut seen[(t.y as i32 * side + t.x as i32) as usize];
            if *slot {
                return Err("terrain detail tile listed twice");
            }
            *slot = true;
        }
        Ok(())
    }

    /// Rebuilds the derived detail layers (tile index, dug depths) from the
    /// saved tiles. Called on load, after [`Self::validate`].
    pub(crate) fn rebuild_derived(&mut self) {
        let d = &mut self.detail;
        d.index.clear();
        d.dug.clear();
        d.dirty.clear();
        if d.tiles.is_empty() {
            return;
        }
        let side = self.size_m / TILE_M;
        d.index = vec![NO_TILE; (side * side) as usize];
        for (i, t) in d.tiles.iter().enumerate() {
            d.index[(t.y as i32 * side + t.x as i32) as usize] = i as u32;
        }
        d.dug = vec![0; (self.dim * self.dim) as usize];
        let coords: Vec<(i32, i32)> = d
            .tiles
            .iter()
            .map(|t| (t.x as i32 * TILE_POINTS, t.y as i32 * TILE_POINTS))
            .collect();
        for (x0, y0) in coords {
            self.update_dug(x0, y0, x0 + TILE_POINTS - 1, y0 + TILE_POINTS - 1);
        }
    }

    /// A copy claiming a different size without regenerating (bad-save tests).
    #[cfg(test)]
    pub(crate) fn with_size_for_test(&self, size_m: i32) -> Heightfield {
        Heightfield {
            size_m,
            ..self.clone()
        }
    }

    /// Lowers the detail pool cap (tests of the pool-full path).
    #[cfg(test)]
    pub(crate) fn set_tile_cap_for_test(&mut self, cap: u32) {
        self.detail.cap = cap;
    }

    /// Appends a copy of the first tile (bad-save tests).
    #[cfg(test)]
    pub(crate) fn duplicate_first_tile_for_test(&mut self) {
        let t = self.detail.tiles[0].clone();
        self.detail.tiles.push(t);
    }

    /// Edits grid heights in place (`f(gx, gy, &mut h)`), for tests.
    #[cfg(test)]
    pub(crate) fn edit_heights_for_test(&mut self, mut f: impl FnMut(u32, u32, &mut Fx)) {
        for gy in 0..self.dim {
            for gx in 0..self.dim {
                f(gx, gy, &mut self.heights[(gy * self.dim + gx) as usize]);
            }
        }
    }

    #[inline]
    pub fn size_m(&self) -> i32 {
        self.size_m
    }

    #[inline]
    pub fn size(&self) -> Fx {
        Fx::from_int(self.size_m)
    }

    #[inline]
    pub fn dim(&self) -> u32 {
        self.dim
    }

    #[inline]
    pub fn heights(&self) -> &[Fx] {
        &self.heights
    }

    /// Per grid point `FLAG_*` bits (empty if none were generated).
    #[inline]
    pub fn flags(&self) -> &[u8] {
        &self.flags
    }

    /// Height of grid point `(gx, gy)`; both must be below [`Self::dim`].
    #[inline]
    pub fn grid_height(&self, gx: u32, gy: u32) -> Fx {
        self.heights[(gy * self.dim + gx) as usize]
    }

    /// Grid point nearest to `p` (clamped to the map).
    #[inline]
    pub fn nearest_grid(&self, p: FxVec2) -> (u32, u32) {
        let p = self.clamp(p);
        let half = (SPACING_M as i64) << (Fx::FRAC_BITS - 1);
        let cell = (SPACING_M as i64) << Fx::FRAC_BITS;
        let g = |v: Fx| (((v.raw() as i64 + half) / cell) as u32).min(self.dim - 1);
        (g(p.x), g(p.y))
    }

    /// Whether `p` is in woods (nearest grid point).
    #[inline]
    pub fn woods_at(&self, p: FxVec2) -> bool {
        if self.flags.is_empty() {
            return false;
        }
        let (gx, gy) = self.nearest_grid(p);
        self.flags[(gy * self.dim + gx) as usize] & FLAG_WOODS != 0
    }

    #[inline]
    pub fn contains(&self, p: FxVec2) -> bool {
        let s = self.size();
        p.x >= Fx::ZERO && p.y >= Fx::ZERO && p.x <= s && p.y <= s
    }

    #[inline]
    pub fn clamp(&self, p: FxVec2) -> FxVec2 {
        let s = self.size();
        FxVec2::new(p.x.clamp(Fx::ZERO, s), p.y.clamp(Fx::ZERO, s))
    }

    #[inline]
    fn at(&self, gx: u32, gy: u32) -> i64 {
        self.heights[(gy * self.dim + gx) as usize].raw() as i64
    }

    /// Bilinear height of the base tier only (ignores digging); positions
    /// are clamped to the map. The path search plans on this surface.
    pub fn base_height_at(&self, p: FxVec2) -> Fx {
        let p = self.clamp(p);
        let cell = (SPACING_M as i64) << Fx::FRAC_BITS;
        let max_cell = self.dim as i64 - 2;
        let ix = (p.x.raw() as i64 / cell).min(max_cell);
        let iy = (p.y.raw() as i64 / cell).min(max_cell);
        // Fractions within the cell in Q16.
        let fx = ((p.x.raw() as i64 - ix * cell) << Fx::FRAC_BITS) / cell;
        let fy = ((p.y.raw() as i64 - iy * cell) << Fx::FRAC_BITS) / cell;
        let (gx, gy) = (ix as u32, iy as u32);
        let h00 = self.at(gx, gy);
        let h10 = self.at(gx + 1, gy);
        let h01 = self.at(gx, gy + 1);
        let h11 = self.at(gx + 1, gy + 1);
        let a = h00 + (((h10 - h00) * fx) >> 16);
        let b = h01 + (((h11 - h01) * fx) >> 16);
        Fx::from_raw((a + (((b - a) * fy) >> 16)) as i32)
    }

    /// Height of the combined surface (detail tiles over the base) at `p`,
    /// bilinear between the four detail points around it; positions are
    /// clamped to the map. Without a tile among those points this is
    /// exactly [`Self::base_height_at`].
    pub fn height_at(&self, p: FxVec2) -> Fx {
        if self.detail.tiles.is_empty() {
            return self.base_height_at(p);
        }
        let p = self.clamp(p);
        let last = self.size_m * DETAIL_PER_M - 1;
        let cx = (p.x.raw() >> DETAIL_SHIFT).min(last);
        let cy = (p.y.raw() >> DETAIL_SHIFT).min(last);
        let inner = TILE_POINTS - 1;
        let corner_tiles = if cx & inner != inner && cy & inner != inner {
            // All four points in the same tile.
            self.tile_of(cx, cy).is_some()
        } else {
            [(0, 0), (1, 0), (0, 1), (1, 1)]
                .iter()
                .any(|&(dx, dy)| self.tile_of(cx + dx, cy + dy).is_some())
        };
        if !corner_tiles {
            return self.base_height_at(p);
        }
        // Fractions within the 0.25 m cell in Q16.
        let fx = ((p.x.raw() - (cx << DETAIL_SHIFT)) as i64) << (Fx::FRAC_BITS - DETAIL_SHIFT);
        let fy = ((p.y.raw() - (cy << DETAIL_SHIFT)) as i64) << (Fx::FRAC_BITS - DETAIL_SHIFT);
        let h00 = self.detail_point(cx, cy).raw() as i64;
        let h10 = self.detail_point(cx + 1, cy).raw() as i64;
        let h01 = self.detail_point(cx, cy + 1).raw() as i64;
        let h11 = self.detail_point(cx + 1, cy + 1).raw() as i64;
        let a = h00 + (((h10 - h00) * fx) >> 16);
        let b = h01 + (((h11 - h01) * fx) >> 16);
        Fx::from_raw((a + (((b - a) * fy) >> 16)) as i32)
    }

    /// First sample point on segment `a → b` at or below the ground, marching
    /// in steps of at most 1 m, refined to under 0.25 m over detail tiles, so
    /// any rise with a top at least one detail cell wide (a parapet, a
    /// trench lip) is never stepped over. `None` if the segment stays above
    /// ground.
    pub fn segment_hit(&self, a: FxVec3, b: FxVec3) -> Option<FxVec3> {
        let d = b - a;
        let len = d.length();
        if self.detail_in_box(a.xy(), b.xy()) {
            return self.march(a, b, len, true, true);
        }
        let steps = (len.raw() >> Fx::FRAC_BITS).clamp(1, 64) + 1;
        for k in 1..=steps {
            let p = a + FxVec3::new(
                d.x.mul_int(k).div_int(steps),
                d.y.mul_int(k).div_int(steps),
                d.z.mul_int(k).div_int(steps),
            );
            if p.z <= self.height_at(p.xy()) {
                return Some(p);
            }
        }
        None
    }

    /// [`Self::segment_hit`] at steps under 0.25 m on all ground (S3: the
    /// terrain re-test in front of a box hit, so a crest just in front of a
    /// hull blocks the shot).
    pub fn segment_hit_fine(&self, a: FxVec3, b: FxVec3) -> Option<FxVec3> {
        let d = b - a;
        let steps = ((d.length().raw() >> Fx::FRAC_BITS).clamp(0, 4096) + 1) * DETAIL_PER_M;
        let part = |v: Fx, k: i32| Fx::from_raw((v.raw() as i64 * k as i64 / steps as i64) as i32);
        (1..=steps).find_map(|k| {
            let p = a + FxVec3::new(part(d.x, k), part(d.y, k), part(d.z, k));
            (p.z <= self.height_at(p.xy())).then_some(p)
        })
    }

    /// Whether the open segment `a → b` clears the combined surface (for
    /// spotting rays). Samples like [`Self::segment_hit`] over detail tiles
    /// and every metre elsewhere; the end points themselves are not tested.
    pub fn line_of_sight(&self, a: FxVec3, b: FxVec3) -> bool {
        let len = (b - a).length();
        let fine = self.detail_in_box(a.xy(), b.xy());
        self.march(a, b, len, fine, false).is_none()
    }

    /// Marches `a → b` in equal steps under 1 m (rays over 4 km: coarser),
    /// each split in four over detail tiles when `fine`; returns the first
    /// sample at or below the surface. `with_end`: test `b` itself too (a
    /// swept projectile segment, not a sight line).
    fn march(&self, a: FxVec3, b: FxVec3, len: Fx, fine: bool, with_end: bool) -> Option<FxVec3> {
        let d = b - a;
        let coarse = (len.raw() >> Fx::FRAC_BITS).clamp(1, 4096) + 1;
        let sub = if fine { DETAIL_PER_M } else { 1 };
        let steps = coarse * sub;
        let last = if with_end { steps } else { steps - 1 };
        // `d · k / steps` in i64: `d · k` overflows Q16.16 on long rays.
        let part = |v: Fx, k: i32| Fx::from_raw((v.raw() as i64 * k as i64 / steps as i64) as i32);
        for k in 1..=last {
            let p = a + FxVec3::new(part(d.x, k), part(d.y, k), part(d.z, k));
            // Over bare base ground only every `sub`-th sample is needed.
            if k % sub != 0 && !self.has_detail_at(p.xy()) {
                continue;
            }
            if p.z <= self.height_at(p.xy()) {
                return Some(p);
            }
        }
        None
    }

    // -------------------------------------------------------- detail tier --

    /// The detail tier (tiles, pool cap).
    #[inline]
    pub fn detail(&self) -> &DetailTier {
        &self.detail
    }

    /// Tiles per map side.
    #[inline]
    pub fn tiles_per_side(&self) -> i32 {
        self.size_m / TILE_M
    }

    /// Detail tile `(x, y)`, if that part of the map has been edited.
    pub fn detail_tile(&self, x: u16, y: u16) -> Option<&DetailTile> {
        self.tile_of(x as i32 * TILE_POINTS, y as i32 * TILE_POINTS)
            .map(|i| &self.detail.tiles[i])
    }

    /// Whether `p` lies in a detail tile.
    #[inline]
    pub fn has_detail_at(&self, p: FxVec2) -> bool {
        if self.detail.tiles.is_empty() {
            return false;
        }
        let p = self.clamp(p);
        self.tile_of(p.x.raw() >> DETAIL_SHIFT, p.y.raw() >> DETAIL_SHIFT)
            .is_some()
    }

    /// Whether any detail tile lies within 0.25 m of the box spanned by `a`
    /// and `b`.
    fn detail_in_box(&self, a: FxVec2, b: FxVec2) -> bool {
        if self.detail.tiles.is_empty() {
            return false;
        }
        let side = self.tiles_per_side();
        let tile = |v: Fx, pad: i32| {
            ((v.raw() >> DETAIL_SHIFT) + pad).clamp(0, side * TILE_POINTS - 1) >> LOG2_TILE_POINTS
        };
        let (x0, x1) = (tile(a.x.min(b.x), -1), tile(a.x.max(b.x), 1));
        let (y0, y1) = (tile(a.y.min(b.y), -1), tile(a.y.max(b.y), 1));
        (y0..=y1)
            .any(|ty| (x0..=x1).any(|tx| self.detail.index[(ty * side + tx) as usize] != NO_TILE))
    }

    /// Index into the tile list of the tile owning detail point `(px, py)`.
    #[inline]
    fn tile_of(&self, px: i32, py: i32) -> Option<usize> {
        if self.detail.index.is_empty() || px < 0 || py < 0 {
            return None;
        }
        let side = self.tiles_per_side();
        let (tx, ty) = (px >> LOG2_TILE_POINTS, py >> LOG2_TILE_POINTS);
        if tx >= side || ty >= side {
            return None;
        }
        let i = self.detail.index[(ty * side + tx) as usize];
        (i != NO_TILE).then_some(i as usize)
    }

    /// Base height at detail point `(px, py)`.
    #[inline]
    pub(crate) fn base_point(&self, px: i32, py: i32) -> Fx {
        self.base_height_at(FxVec2::new(
            Fx::from_raw(px << DETAIL_SHIFT),
            Fx::from_raw(py << DETAIL_SHIFT),
        ))
    }

    /// Combined height at detail point `(px, py)`.
    #[inline]
    fn detail_point(&self, px: i32, py: i32) -> Fx {
        match self.tile_of(px, py) {
            Some(i) => {
                let local =
                    ((py & (TILE_POINTS - 1)) << LOG2_TILE_POINTS) | (px & (TILE_POINTS - 1));
                self.detail.tiles[i].heights[local as usize]
            }
            None => self.base_point(px, py),
        }
    }

    /// Deepest dig below the base in grid node `(gx, gy)`'s 4 × 4 m cell, in
    /// decimetres (0 where nothing was dug).
    #[inline]
    pub fn dug_dm(&self, gx: u32, gy: u32) -> u8 {
        if self.detail.dug.is_empty() {
            return 0;
        }
        self.detail.dug[(gy * self.dim + gx) as usize]
    }

    /// Whether grid node `(gx, gy)` is a trench cell (dug at least
    /// [`TRENCH_NAV_DM`] deep somewhere in its cell).
    #[inline]
    pub fn is_trench(&self, gx: u32, gy: u32) -> bool {
        self.dug_dm(gx, gy) >= TRENCH_NAV_DM
    }

    /// Rewrites the detail points in `[x0, x1] × [y0, y1]` (detail indices,
    /// clipped to the map) as `f(px, py, base, current) -> new`. Tiles are
    /// visited in ascending `(y, x)`; a tile is created (initialised from
    /// the base) only if `f` changes one of its points. Changed tiles get
    /// `changed_tick = tick` and go on the dirty list.
    ///
    /// When the pool is full, the oldest tile that is flat again (equal to
    /// the base everywhere) is recycled; if there is none, the edit to that
    /// tile is dropped and counted ([`DetailTier::dropped`]).
    pub fn edit(
        &mut self,
        (x0, y0): (i32, i32),
        (x1, y1): (i32, i32),
        tick: u64,
        mut f: impl FnMut(i32, i32, Fx, Fx) -> Fx,
    ) {
        let side = self.tiles_per_side();
        let last = side * TILE_POINTS - 1;
        let (x0, y0, x1, y1) = (x0.max(0), y0.max(0), x1.min(last), y1.min(last));
        if x0 > x1 || y0 > y1 {
            return;
        }
        let mut any = false;
        for ty in (y0 >> LOG2_TILE_POINTS)..=(y1 >> LOG2_TILE_POINTS) {
            for tx in (x0 >> LOG2_TILE_POINTS)..=(x1 >> LOG2_TILE_POINTS) {
                let (ox, oy) = (tx * TILE_POINTS, ty * TILE_POINTS);
                let (xa, xb) = (x0.max(ox), x1.min(ox + TILE_POINTS - 1));
                let (ya, yb) = (y0.max(oy), y1.min(oy + TILE_POINTS - 1));
                let local =
                    |px: i32, py: i32| (((py - oy) << LOG2_TILE_POINTS) | (px - ox)) as usize;
                if let Some(i) = self.tile_of(ox, oy) {
                    let mut buf = std::mem::take(&mut self.detail.tiles[i].heights);
                    let mut changed = false;
                    for py in ya..=yb {
                        for px in xa..=xb {
                            let k = local(px, py);
                            let new = f(px, py, self.base_point(px, py), buf[k]);
                            changed |= new != buf[k];
                            buf[k] = new;
                        }
                    }
                    let t = &mut self.detail.tiles[i];
                    t.heights = buf;
                    if changed {
                        t.changed_tick = tick;
                        self.detail.dirty.push((tx as u16, ty as u16));
                        any = true;
                    }
                    continue;
                }
                // No tile yet: only build one if a point actually changes.
                let mut changes = Vec::new();
                for py in ya..=yb {
                    for px in xa..=xb {
                        let base = self.base_point(px, py);
                        let new = f(px, py, base, base);
                        if new != base {
                            changes.push((local(px, py), new));
                        }
                    }
                }
                if changes.is_empty() {
                    continue;
                }
                let mut buf = self.base_tile(ox, oy);
                for (k, h) in changes {
                    buf[k] = h;
                }
                any |= self.insert_tile(tx, ty, buf, tick);
            }
        }
        if any {
            self.update_dug(x0, y0, x1, y1);
        }
    }

    /// The base surface sampled at a tile's 64 × 64 points.
    fn base_tile(&self, ox: i32, oy: i32) -> Vec<Fx> {
        let mut h = Vec::with_capacity(TILE_LEN);
        for py in oy..oy + TILE_POINTS {
            for px in ox..ox + TILE_POINTS {
                h.push(self.base_point(px, py));
            }
        }
        h
    }

    /// Adds tile `(tx, ty)` to the pool (see [`Self::edit`] for a full
    /// pool). Returns whether it was stored.
    fn insert_tile(&mut self, tx: i32, ty: i32, heights: Vec<Fx>, tick: u64) -> bool {
        let side = self.tiles_per_side();
        if self.detail.index.is_empty() {
            self.detail.index = vec![NO_TILE; (side * side) as usize];
            self.detail.dug = vec![0; (self.dim * self.dim) as usize];
        }
        let tile = DetailTile {
            x: tx as u16,
            y: ty as u16,
            changed_tick: tick,
            heights,
        };
        let i = if self.detail.tiles.len() < self.detail.cap as usize {
            self.detail.tiles.push(tile);
            self.detail.tiles.len() - 1
        } else {
            let flat = (0..self.detail.tiles.len()).find(|&i| {
                let t = &self.detail.tiles[i];
                t.heights == self.base_tile(t.x as i32 * TILE_POINTS, t.y as i32 * TILE_POINTS)
            });
            let Some(i) = flat else {
                self.detail.dropped = self.detail.dropped.saturating_add(1);
                return false;
            };
            // Demote the flat tile back to the base (its area looks the
            // same either way) and reuse its slot.
            let old = std::mem::replace(&mut self.detail.tiles[i], tile);
            self.detail.index[(old.y as i32 * side + old.x as i32) as usize] = NO_TILE;
            self.detail.dirty.push((old.x, old.y));
            i
        };
        self.detail.index[(ty * side + tx) as usize] = i as u32;
        self.detail.dirty.push((tx as u16, ty as u16));
        true
    }

    /// Recomputes `dug` for every grid node whose cell overlaps the detail
    /// box `[x0, x1] × [y0, y1]`.
    fn update_dug(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        if self.detail.dug.is_empty() {
            return;
        }
        // A detail point belongs to the cell of its nearest grid node.
        let per = SPACING_M * DETAIL_PER_M; // detail points per grid step
        let max = self.dim as i32 - 1;
        let node = |v: i32| (v + per / 2).div_euclid(per).clamp(0, max) as u32;
        for gy in node(y0)..=node(y1) {
            for gx in node(x0)..=node(x1) {
                let v = self.node_dug(gx, gy);
                self.detail.dug[(gy * self.dim + gx) as usize] = v;
            }
        }
    }

    /// Deepest dig (dm) over the detail points of grid node `(gx, gy)`'s cell
    /// (`±2 m` around it), counting only points inside tiles.
    fn node_dug(&self, gx: u32, gy: u32) -> u8 {
        let per = SPACING_M * DETAIL_PER_M;
        let last = self.tiles_per_side() * TILE_POINTS - 1;
        let (cx, cy) = (gx as i32 * per, gy as i32 * per);
        let mut deepest = 0i64;
        for py in (cy - per / 2).max(0)..(cy + per / 2).min(last + 1) {
            for px in (cx - per / 2).max(0)..(cx + per / 2).min(last + 1) {
                if let Some(i) = self.tile_of(px, py) {
                    let local =
                        ((py & (TILE_POINTS - 1)) << LOG2_TILE_POINTS) | (px & (TILE_POINTS - 1));
                    let h = self.detail.tiles[i].heights[local as usize];
                    deepest = deepest.max(self.base_point(px, py).raw() as i64 - h.raw() as i64);
                }
            }
        }
        ((deepest * 10) >> Fx::FRAC_BITS).clamp(0, 255) as u8
    }

    /// Drains the dirty tile list into `out`: tiles changed (or demoted)
    /// since the last call, ascending `(y, x)`, each once.
    pub(crate) fn take_dirty(&mut self, out: &mut Vec<(u16, u16)>) {
        out.clear();
        out.append(&mut self.detail.dirty);
        out.sort_unstable_by_key(|&(x, y)| (y, x));
        out.dedup();
    }

    /// Index of tile `(x, y)` in [`DetailTier::tiles`].
    pub(crate) fn tile_index(&self, x: u16, y: u16) -> Option<usize> {
        self.tile_of(x as i32 * TILE_POINTS, y as i32 * TILE_POINTS)
    }

    /// Blasts a crater at `center` (§5.10.2 `Crater`): the current surface
    /// is lowered by a bowl `depth · (1 − (r/R)²)` inside `radius` R and
    /// raised by a rim peaking at `depth / 4` at R, fading to nothing at
    /// 0.5 R and 1.5 R. Additive, so craters on craters (or in a trench)
    /// deepen what is there.
    pub fn crater(&mut self, center: FxVec2, radius: Fx, depth: Fx, tick: u64) {
        self.crater_with(center, radius, depth, tick, false, None);
    }

    /// [`Self::crater`], except when `center` is on raised spoil (the
    /// surface above the base surface: a parapet or berm; S3b): loose
    /// spoil flies, so inside `radius` every point still above the base
    /// surface is lowered by `spoil_pct` % of `depth` times the bowl shape,
    /// never below the base surface, and no rim rises on it; points at or
    /// below the base surface (dug walls, undug ground) get the usual bowl,
    /// and their rim never rises above the base surface.
    pub fn crater_spoil(
        &mut self,
        center: FxVec2,
        radius: Fx,
        depth: Fx,
        spoil_pct: u32,
        tick: u64,
    ) {
        let on_spoil = self.height_at(center) > self.base_height_at(center);
        let spoil = on_spoil.then(|| depth.mul_int(spoil_pct as i32).div_int(100));
        self.crater_with(center, radius, depth, tick, false, spoil);
    }

    /// A crater that levels what it hits instead of deepening it (satchel
    /// charges): inside `radius` the ground becomes the crater bowl below
    /// the *base* surface, so a trench there fills in and its walls are
    /// gone; in the rim band nothing stays above the base surface plus the
    /// rim, so a parapet is blown down, while deeper ground (the trench
    /// beyond the crater) is kept.
    pub fn crater_cut(&mut self, center: FxVec2, radius: Fx, depth: Fx, tick: u64) {
        self.crater_with(center, radius, depth, tick, true, None);
    }

    fn crater_with(
        &mut self,
        center: FxVec2,
        radius: Fx,
        depth: Fx,
        tick: u64,
        cut: bool,
        spoil: Option<Fx>,
    ) {
        if radius <= Fx::ZERO || depth <= Fx::ZERO {
            return;
        }
        let reach = radius.mul_int(3).div_int(2);
        let lo = |v: Fx| (v - reach).raw() >> DETAIL_SHIFT;
        let hi = |v: Fx| ((v + reach).raw() >> DETAIL_SHIFT) + 1;
        let rim = depth.div_int(4);
        let r_sq = (radius.raw() as i64 * radius.raw() as i64) as u128;
        self.edit(
            (lo(center.x), lo(center.y)),
            (hi(center.x), hi(center.y)),
            tick,
            |px, py, base, cur| {
                let p = FxVec2::new(
                    Fx::from_raw(px << DETAIL_SHIFT),
                    Fx::from_raw(py << DETAIL_SHIFT),
                );
                let d_sq = (p - center).length_sq_raw();
                let dist = Fx::sqrt_q32(d_sq);
                if dist >= reach {
                    return cur;
                }
                let t = dist.div(radius); // r / R
                let mut delta = Fx::ZERO;
                // 1 − (r/R)², from the exact squares.
                let bowl = if dist < radius {
                    let frac = Fx::from_raw((((d_sq as u128) << Fx::FRAC_BITS) / r_sq) as i32);
                    Fx::ONE - frac
                } else {
                    Fx::ZERO
                };
                delta -= depth.mul(bowl);
                let off = (t - Fx::ONE).abs();
                let rise = if off < Fx::HALF {
                    rim.mul(Fx::ONE - off.mul_int(2))
                } else {
                    Fx::ZERO
                };
                if let Some(sp) = spoil {
                    // S3b: a burst on loose spoil.
                    return if cur > base {
                        (cur - sp.mul(bowl)).max(base)
                    } else {
                        let low = cur + delta;
                        (low + rise).min(low.max(base))
                    };
                }
                delta += rise;
                match (cut, dist < radius) {
                    (false, _) => cur + delta,
                    (true, true) => base + delta,
                    (true, false) => cur.min(base + delta),
                }
            },
        );
    }
}

/// `3t² − 2t³` for `t` in `[0, 1]` (Q16.16).
#[inline]
fn smoothstep(t: Fx) -> Fx {
    let t = t.clamp(Fx::ZERO, Fx::ONE);
    t.mul(t).mul(Fx::from_int(3) - t.mul_int(2))
}

/// Value noise on a square lattice of `period` metres with smoothstep
/// interpolation (rounded hills instead of the bilinear facets of
/// [`lattice_noise`]).
fn smooth_noise(seed: u64, x: i32, y: i32, period: i32, amp: Fx, octave: u32) -> Fx {
    let (lx, ly) = (x / period, y / period);
    let sx = smoothstep(Fx::from_ratio((x % period) as i64, period as i64)).raw() as i64;
    let sy = smoothstep(Fx::from_ratio((y % period) as i64, period as i64)).raw() as i64;
    let v = |i: i32, j: i32| -> i64 {
        let key = SimId(((j as u32) << 16) | (i as u32 & 0xFFFF));
        let roll = rng(seed, 0, key, Purpose(Purpose::MAP_TERRAIN.0 + octave));
        range_fx(roll, -amp, amp).raw() as i64
    };
    let (v00, v10, v01, v11) = (v(lx, ly), v(lx + 1, ly), v(lx, ly + 1), v(lx + 1, ly + 1));
    let a = v00 + (((v10 - v00) * sx) >> 16);
    let b = v01 + (((v11 - v01) * sx) >> 16);
    Fx::from_raw((a + (((b - a) * sy) >> 16)) as i32)
}

/// Height offset of a ridge/valley band at `p`: `height` on the segment
/// `a → b`, falling off with a smoothstep to zero at `half_width`.
fn band(p: FxVec2, a: FxVec2, b: FxVec2, height: Fx, half_width: Fx) -> Fx {
    let ab = b - a;
    let len_sq = ab.length_sq_raw() as i64;
    let t = if len_sq == 0 {
        Fx::ZERO
    } else {
        let d = (p - a).dot_raw(ab).clamp(0, len_sq);
        Fx::from_raw(((d as i128) << Fx::FRAC_BITS).div_euclid(len_sq as i128) as i32)
    };
    let closest = a + ab * t;
    let dist = (p - closest).length();
    if dist >= half_width {
        return Fx::ZERO;
    }
    height.mul(smoothstep(Fx::ONE - dist.div(half_width)))
}

/// Bilinear value noise on a square lattice of `period` metres.
fn lattice_noise(seed: u64, x: i32, y: i32, period: i32, amp: Fx, octave: u32) -> Fx {
    let (lx, ly) = (x / period, y / period);
    let (rx, ry) = (x % period, y % period);
    let v = |i: i32, j: i32| -> i64 {
        // Lattice coordinates fit in 16 bits for maps up to 4 km.
        let key = SimId(((j as u32) << 16) | (i as u32 & 0xFFFF));
        let roll = rng(seed, 0, key, Purpose(Purpose::TERRAIN.0 + octave));
        range_fx(roll, -amp, amp).raw() as i64
    };
    let (v00, v10, v01, v11) = (v(lx, ly), v(lx + 1, ly), v(lx, ly + 1), v(lx + 1, ly + 1));
    let p = period as i64;
    let a = v00 + (v10 - v00) * rx as i64 / p;
    let b = v01 + (v11 - v01) * rx as i64 / p;
    Fx::from_raw((a + (b - a) * ry as i64 / p) as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise() -> Heightfield {
        Heightfield::generate(TerrainKind::Noise, 256, 3)
    }

    fn v(x_mm: i32, y_mm: i32) -> FxVec2 {
        FxVec2::new(Fx::from_milli(x_mm), Fx::from_milli(y_mm))
    }

    /// Lowers a 1 × 1 m square at `(x, y)` m by 1 m.
    fn pit(t: &mut Heightfield, x: i32, y: i32, tick: u64) {
        let (px, py) = (x * DETAIL_PER_M, y * DETAIL_PER_M);
        t.edit((px, py), (px + 4, py + 4), tick, |_, _, base, _| {
            base - Fx::ONE
        });
    }

    #[test]
    fn detail_surface_matches_base_until_edited() {
        let mut t = noise();
        let probes = [v(10_100, 20_700), v(100_000, 3_330), v(255_900, 255_900)];
        let before: Vec<Fx> = probes.iter().map(|&p| t.height_at(p)).collect();
        // A no-op edit creates no tile.
        t.edit((0, 0), (200, 200), 1, |_, _, _, cur| cur);
        assert!(t.detail().tiles().is_empty());
        pit(&mut t, 50, 50, 2);
        assert_eq!(t.detail().tiles().len(), 1);
        // Far away: still exactly the base path.
        for (p, h) in probes.iter().zip(&before) {
            assert_eq!(t.height_at(*p), *h);
        }
        // Inside the tile, untouched points sample the base to within
        // fixed-point rounding.
        let q = v(55_130, 52_870);
        assert!((t.height_at(q) - t.base_height_at(q)).abs() <= Fx::from_raw(8));
        // The pit is 1 m below the base.
        let c = v(50_500, 50_500);
        assert!((t.base_height_at(c) - t.height_at(c) - Fx::ONE).abs() <= Fx::from_raw(8));
    }

    #[test]
    fn edits_across_tile_edges_touch_each_tile_once() {
        let mut t = Heightfield::generate(TerrainKind::Flat, 128, 1);
        let mut dirty = Vec::new();
        // A 2 × 2 m pit centred on the corner of four tiles.
        t.edit((60, 60), (68, 68), 9, |_, _, _, _| -Fx::ONE);
        t.take_dirty(&mut dirty);
        assert_eq!(dirty, vec![(0, 0), (1, 0), (0, 1), (1, 1)]);
        assert!(t.detail().tiles().iter().all(|t| t.changed_tick() == 9));
        // Smooth across the edges: the cell straddling the corner is fully dug.
        assert_eq!(t.height_at(v(16_000, 16_000)), -Fx::ONE);
        assert_eq!(t.height_at(v(15_900, 16_100)), -Fx::ONE);
        // Drained.
        t.take_dirty(&mut dirty);
        assert!(dirty.is_empty());
        assert!(t.detail_tile(1, 1).is_some() && t.detail_tile(2, 2).is_none());
    }

    #[test]
    fn crater_has_bowl_and_rim() {
        let mut t = Heightfield::generate(TerrainKind::Flat, 128, 1);
        let c = FxVec2::from_int(64, 64);
        let (r, depth) = (Fx::from_int(2), Fx::from_milli(800));
        t.crater(c, r, depth, 5);
        let at = |dx_mm: i32| t.height_at(c + v(dx_mm, 0));
        assert_eq!(at(0), -depth);
        // Bowl deepest in the middle, rising outward.
        assert!(at(500) > at(0) && at(1000) > at(500) && at(1500) > at(1000));
        assert!(at(1000) < Fx::ZERO);
        // Rim peaks at R at depth / 4, gone at 1.5 R.
        assert!((at(2000) - depth.div_int(4)).abs() <= Fx::from_milli(1));
        assert!(at(2000) > at(2500) && at(2500) > Fx::ZERO);
        assert_eq!(at(3000), Fx::ZERO);
        assert_eq!(at(3500), Fx::ZERO);
        // Round: same depth on a diagonal at the same distance.
        let diag = t.height_at(c + v(707, 707));
        assert!((diag - at(1000)).abs() <= Fx::from_milli(20));
        // A second crater deepens the first.
        t.crater(c, r, depth, 6);
        assert_eq!(t.height_at(c), -depth.mul_int(2));
        assert!(t.dug_dm(16, 16) >= 15);
    }

    #[test]
    fn a_crater_on_a_parapet_blows_the_spoil_away() {
        // A 0.45 m spoil bank 4 m wide across x = 60..64 m (a parapet), on
        // flat ground at 0 (the base surface).
        let mut t = Heightfield::generate(TerrainKind::Flat, 128, 1);
        let (x0, x1) = (60 * DETAIL_PER_M, 64 * DETAIL_PER_M);
        t.edit((x0, 0), (x1, 511), 1, |_, _, _, _| Fx::from_milli(450));
        let c = FxVec2::from_int(62, 64);
        let (r, depth) = (Fx::from_milli(600), Fx::from_milli(300));
        let before = t.height_at(c);
        // 150 %: the centre loses 0.45 m of spoil, down to the base.
        t.crater_spoil(c, r, depth, 150, 2);
        assert!(
            t.height_at(c) <= Fx::from_milli(1),
            "all the spoil there is gone"
        );
        let lost = before - t.height_at(c);
        assert!(lost >= Fx::from_milli(400));
        // No rim rises on the spoil, and none above the base anywhere.
        let rim = t.height_at(c + v(600, 0));
        assert!(rim <= Fx::from_milli(450), "{rim:?}");
        // Half way out: the bowl shape, 0.45 × 0.75.
        let half = t.height_at(c + v(300, 0));
        assert!(
            (half - Fx::from_milli(450 - 338)).abs() <= Fx::from_milli(20),
            "{half:?}"
        );
        // Never below the base surface on spoil.
        let mut deep = Heightfield::generate(TerrainKind::Flat, 128, 1);
        deep.edit((x0, 0), (x1, 511), 1, |_, _, _, _| Fx::from_milli(100));
        deep.crater_spoil(c, r, depth, 150, 2);
        assert!(deep.height_at(c) >= Fx::ZERO && deep.height_at(c) <= Fx::from_milli(1));
        // Centred on undug ground: today's crater, rim and all.
        let mut flat = Heightfield::generate(TerrainKind::Flat, 128, 1);
        let mut usual = flat.clone();
        let g = FxVec2::from_int(30, 30);
        flat.crater_spoil(g, r, depth, 150, 2);
        usual.crater(g, r, depth, 2);
        for k in -20..=20 {
            let p = g + v(k * 100, 0);
            assert_eq!(flat.height_at(p), usual.height_at(p));
        }
    }

    #[test]
    fn segment_hit_and_los_see_a_parapet_the_coarse_march_steps_over() {
        // A 0.3 m high ridge across x = 50 m with a flat top one detail
        // cell (0.25 m) wide: the narrowest top the 0.25 m march must hit.
        let mut t = Heightfield::generate(TerrainKind::Flat, 128, 1);
        let x = 50 * DETAIL_PER_M;
        t.edit((x, 0), (x + 1, 511), 1, |_, _, _, _| Fx::from_milli(300));
        let z = Fx::from_milli(200);
        let a = FxVec3::new(Fx::from_int(40), Fx::from_int(64), z);
        let b = FxVec3::new(Fx::from_int(60), Fx::from_int(64), z);
        let hit = t.segment_hit(a, b).expect("parapet stops the shot");
        assert!(hit.x >= Fx::from_milli(49_800) && hit.x <= Fx::from_milli(50_450));
        assert!(!t.line_of_sight(a, b));
        // Above the parapet: clear.
        let up = FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::from_milli(200));
        assert!(t.segment_hit(a + up, b + up).is_none());
        assert!(t.line_of_sight(a + up, b + up));
    }

    #[test]
    fn full_pool_recycles_flat_tiles_then_drops_edits() {
        let mut t = Heightfield::generate(TerrainKind::Flat, 128, 1);
        t.set_tile_cap_for_test(2);
        pit(&mut t, 4, 4, 1); // tile (0, 0)
        pit(&mut t, 20, 4, 2); // tile (1, 0)
        // Full, nothing flat: dropped.
        pit(&mut t, 36, 4, 3);
        assert_eq!(t.detail().dropped(), 1);
        assert!(t.detail_tile(2, 0).is_none());
        assert_eq!(t.height_at(FxVec2::from_int(36, 4)), Fx::ZERO);
        // Fill tile (0, 0) back to the base: it is recycled for (2, 0).
        t.edit((0, 0), (63, 63), 4, |_, _, base, _| base);
        pit(&mut t, 36, 4, 5);
        assert!(t.detail_tile(0, 0).is_none() && t.detail_tile(2, 0).is_some());
        assert_eq!(t.detail().tiles().len(), 2);
        let mut dirty = Vec::new();
        t.take_dirty(&mut dirty);
        assert!(dirty.contains(&(0, 0)) && dirty.contains(&(2, 0)));
    }

    #[test]
    fn derived_layers_rebuild_identically() {
        let mut t = noise();
        pit(&mut t, 30, 30, 1);
        pit(&mut t, 31, 33, 2);
        t.crater(FxVec2::from_int(100, 90), Fx::from_int(2), Fx::ONE, 3);
        let mut loaded: Heightfield =
            postcard::from_bytes(&postcard::to_allocvec(&t).unwrap()).unwrap();
        loaded.validate().unwrap();
        loaded.rebuild_derived();
        t.take_dirty(&mut Vec::new());
        assert_eq!(loaded, t);
        assert!(t.is_trench(25, 22) || t.dug_dm(25, 22) >= 5);
    }
}
