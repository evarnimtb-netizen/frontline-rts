//! Benchmark heightfield: 1 m cells of `u16` height (1/64 m units) plus a
//! max-mip pyramid, stored in a pluggable memory [`Layout`].
//!
//! This models the base tier of the §5.10.1 heightfield (≈18 MB of u16 at
//! 3×3 km, 32 MB at 4×4 km). Cells are flat-topped: a cell's height applies
//! to its whole 1×1 m square. That makes the max-mip pyramid an exact upper
//! bound, so block skipping can never change a LOS result.
//!
//! Terrain comes from five octaves of seeded integer value noise (1 km down to
//! 4 m), so 300–600 m rays see a realistic mix of hills and dips.

use frontline_sim::rng::mix64;
use rayon::prelude::*;

/// Height units per metre (heights are `u16` in 1/64 m: 0..1024 m).
pub const HEIGHT_UNITS_PER_M: i64 = 64;
/// Shift from height units to Q16.16 raw (`h << 10`).
pub const HEIGHT_TO_Q16_SHIFT: u32 = 10;

/// Maps a cell `(x, y)` of a square power-of-two grid to a storage index.
pub trait Layout: Send + Sync + Sized {
    /// Human-readable name used in reports.
    const NAME: &'static str;
    fn new(dim: u32) -> Self;
    fn index(&self, x: u32, y: u32) -> usize;
}

/// Row-major: `y * dim + x`.
#[derive(Clone, Debug)]
pub struct Linear {
    shift: u32,
}

impl Layout for Linear {
    const NAME: &'static str = "linear";
    fn new(dim: u32) -> Self {
        Linear {
            shift: dim.trailing_zeros(),
        }
    }
    #[inline(always)]
    fn index(&self, x: u32, y: u32) -> usize {
        ((y << self.shift) | x) as usize
    }
}

/// Row-major 16×16 tiles (512 B of u16 per tile), row-major inside a tile.
/// Grids smaller than 16 use one tile.
#[derive(Clone, Debug)]
pub struct Tiled {
    tile_shift: u32,
    tile_mask: u32,
    tiles_row_shift: u32,
}

impl Layout for Tiled {
    const NAME: &'static str = "tiled16";
    fn new(dim: u32) -> Self {
        let log = dim.trailing_zeros();
        let tile_shift = log.min(4);
        Tiled {
            tile_shift,
            tile_mask: (1 << tile_shift) - 1,
            tiles_row_shift: log - tile_shift,
        }
    }
    #[inline(always)]
    fn index(&self, x: u32, y: u32) -> usize {
        let ts = self.tile_shift;
        let tile = ((y >> ts) << self.tiles_row_shift) | (x >> ts);
        let inner = ((y & self.tile_mask) << ts) | (x & self.tile_mask);
        ((tile << (2 * ts)) | inner) as usize
    }
}

/// Full Z-order (Morton) curve via a bit-spread lookup table.
#[derive(Clone, Debug)]
pub struct Morton {
    spread: Vec<u32>,
}

/// Spreads the low 16 bits of `v` to the even bit positions.
const fn spread_bits(v: u32) -> u32 {
    let mut x = v & 0xFFFF;
    x = (x | (x << 8)) & 0x00FF_00FF;
    x = (x | (x << 4)) & 0x0F0F_0F0F;
    x = (x | (x << 2)) & 0x3333_3333;
    x = (x | (x << 1)) & 0x5555_5555;
    x
}

impl Layout for Morton {
    const NAME: &'static str = "morton";
    fn new(dim: u32) -> Self {
        Morton {
            spread: (0..dim).map(spread_bits).collect(),
        }
    }
    #[inline(always)]
    fn index(&self, x: u32, y: u32) -> usize {
        (self.spread[x as usize] | (self.spread[y as usize] << 1)) as usize
    }
}

/// One pyramid level: `dim × dim` cells of max height in its layout.
#[derive(Clone, Debug)]
pub struct Level<L: Layout> {
    pub dim: u32,
    pub layout: L,
    pub heights: Vec<u16>,
}

impl<L: Layout> Level<L> {
    #[inline(always)]
    pub fn get(&self, x: u32, y: u32) -> u16 {
        self.heights[self.layout.index(x, y)]
    }
}

/// A heightfield and its max-mip pyramid. `levels[0]` is the 1 m base grid;
/// `levels[l]` holds the max over `2^l × 2^l` base cells.
#[derive(Clone, Debug)]
pub struct HeightGrid<L: Layout> {
    pub levels: Vec<Level<L>>,
}

impl<L: Layout> HeightGrid<L> {
    /// Builds from row-major base heights. `dim` must be a power of two.
    /// Pyramid levels are built up to `max_level` (or until 1×1).
    pub fn from_row_major(dim: u32, row_major: &[u16], max_level: u32) -> Self {
        assert!(dim.is_power_of_two(), "grid dim must be a power of two");
        assert_eq!(row_major.len(), (dim as usize) * (dim as usize));
        let base_layout = L::new(dim);
        let mut base = vec![0u16; row_major.len()];
        for y in 0..dim {
            for x in 0..dim {
                base[base_layout.index(x, y)] = row_major[(y * dim + x) as usize];
            }
        }
        let mut levels = vec![Level {
            dim,
            layout: base_layout,
            heights: base,
        }];
        let top = max_level.min(dim.trailing_zeros());
        for _ in 0..top {
            let prev = levels.last().expect("base level exists");
            let d = prev.dim / 2;
            let layout = L::new(d);
            let mut h = vec![0u16; (d as usize) * (d as usize)];
            for y in 0..d {
                for x in 0..d {
                    let m = prev
                        .get(2 * x, 2 * y)
                        .max(prev.get(2 * x + 1, 2 * y))
                        .max(prev.get(2 * x, 2 * y + 1))
                        .max(prev.get(2 * x + 1, 2 * y + 1));
                    h[layout.index(x, y)] = m;
                }
            }
            levels.push(Level {
                dim: d,
                layout,
                heights: h,
            });
        }
        HeightGrid { levels }
    }

    #[inline(always)]
    pub fn dim(&self) -> u32 {
        self.levels[0].dim
    }

    /// Highest pyramid level available.
    pub fn top_level(&self) -> u32 {
        self.levels.len() as u32 - 1
    }

    /// Base cell height in height units.
    #[inline(always)]
    pub fn height(&self, x: u32, y: u32) -> u16 {
        self.levels[0].get(x, y)
    }

    /// Bytes in the base grid.
    pub fn base_bytes(&self) -> usize {
        self.levels[0].heights.len() * 2
    }

    /// Bytes in pyramid levels 1.. (excluding the base).
    pub fn pyramid_bytes(&self) -> usize {
        self.levels[1..].iter().map(|l| l.heights.len() * 2).sum()
    }
}

/// Generates `dim × dim` row-major heights (1/64 m units) from seeded value
/// noise. Pure integer, so identical on every machine.
pub fn generate_terrain(dim: u32, seed: u64) -> Vec<u16> {
    // (lattice period m, amplitude m·64). Base height 200 m.
    const BASE: i64 = 200 * HEIGHT_UNITS_PER_M;
    const OCTAVES: [(u32, i64); 5] = [
        (1024, 45 * HEIGHT_UNITS_PER_M),
        (256, 14 * HEIGHT_UNITS_PER_M),
        (64, 4 * HEIGHT_UNITS_PER_M),
        (16, HEIGHT_UNITS_PER_M),
        (4, HEIGHT_UNITS_PER_M / 4),
    ];
    let lattices: Vec<(u32, u32, Vec<i64>)> = OCTAVES
        .iter()
        .enumerate()
        .map(|(o, &(period, amp))| {
            let ld = dim / period + 2;
            let vals = (0..ld * ld)
                .map(|i| {
                    let key = mix64(seed ^ ((o as u64) << 56) ^ i as u64);
                    // Uniform in [-amp, amp].
                    ((key >> 33) as i64 % (2 * amp + 1)) - amp
                })
                .collect();
            (period, ld, vals)
        })
        .collect();

    let mut out = vec![0u16; (dim as usize) * (dim as usize)];
    out.par_chunks_mut(dim as usize)
        .enumerate()
        .for_each(|(y, row)| {
            let y = y as u32;
            for (x, cell) in row.iter_mut().enumerate() {
                let x = x as u32;
                let mut h = BASE;
                for (period, ld, vals) in &lattices {
                    let p = *period as i64;
                    let (lx, ly) = (x / period, y / period);
                    let (rx, ry) = ((x % period) as i64, (y % period) as i64);
                    let v = |i: u32, j: u32| vals[(j * ld + i) as usize];
                    let (v00, v10) = (v(lx, ly), v(lx + 1, ly));
                    let (v01, v11) = (v(lx, ly + 1), v(lx + 1, ly + 1));
                    let a = v00 * p + (v10 - v00) * rx;
                    let b = v01 * p + (v11 - v01) * rx;
                    h += (a * p + (b - a) * ry).div_euclid(p * p);
                }
                *cell = h.clamp(0, u16::MAX as i64) as u16;
            }
        });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn indices_are_a_permutation<L: Layout>(dim: u32) {
        let l = L::new(dim);
        let mut seen = vec![false; (dim * dim) as usize];
        for y in 0..dim {
            for x in 0..dim {
                let i = l.index(x, y);
                assert!(!seen[i], "{} dup at {x},{y}", L::NAME);
                seen[i] = true;
            }
        }
    }

    #[test]
    fn layouts_are_bijections() {
        for dim in [1, 2, 8, 16, 64, 256] {
            indices_are_a_permutation::<Linear>(dim);
            indices_are_a_permutation::<Tiled>(dim);
            indices_are_a_permutation::<Morton>(dim);
        }
    }

    #[test]
    fn pyramid_is_block_max() {
        let dim = 64;
        let hm = generate_terrain(dim, 7);
        let g = HeightGrid::<Tiled>::from_row_major(dim, &hm, 6);
        assert_eq!(g.top_level(), 6);
        for l in 1..=6u32 {
            let lvl = &g.levels[l as usize];
            for by in 0..lvl.dim {
                for bx in 0..lvl.dim {
                    let mut m = 0;
                    for y in by << l..(by + 1) << l {
                        for x in bx << l..(bx + 1) << l {
                            m = m.max(hm[(y * dim + x) as usize]);
                        }
                    }
                    assert_eq!(lvl.get(bx, by), m);
                }
            }
        }
    }
}
