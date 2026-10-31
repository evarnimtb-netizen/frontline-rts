//! M0 heightfield: a square grid of Q16.16 heights, flat or seeded integer
//! value noise. Bilinear sampling and a swept-segment ground test.
//!
//! This is a stand-in for the two-tier tiled heightfield of §5.10.1. It is
//! immutable after generation, so it is hashed once (at creation and on load).

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
}

/// Grid spacing in metres.
pub const SPACING_M: i32 = 4;

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Heightfield {
    size_m: i32,
    /// Points per side (`size_m / SPACING_M + 1`).
    dim: u32,
    heights: Vec<Fx>,
}

impl Heightfield {
    /// `size_m` must be a positive multiple of 64.
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
        if self.heights.len() != self.dim as usize * self.dim as usize {
            return Err("terrain height count disagrees with its grid dimension");
        }
        Ok(())
    }

    /// A copy claiming a different size without regenerating (bad-save tests).
    #[cfg(test)]
    pub(crate) fn with_size_for_test(&self, size_m: i32) -> Heightfield {
        Heightfield {
            size_m,
            ..self.clone()
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

    /// Bilinear height at `(x, y)`; positions are clamped to the map.
    pub fn height_at(&self, p: FxVec2) -> Fx {
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

    /// First sample point on segment `a → b` at or below the ground, marching
    /// in steps of at most 1 m. `None` if the segment stays above ground.
    pub fn segment_hit(&self, a: FxVec3, b: FxVec3) -> Option<FxVec3> {
        let d = b - a;
        let len = d.length();
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
