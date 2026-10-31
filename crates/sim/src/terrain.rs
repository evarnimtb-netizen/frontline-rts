//! Heightfield: a square grid of Q16.16 heights at 4 m spacing, flat, seeded
//! integer value noise (bench) or generated from a map def (match). Bilinear
//! sampling and a swept-segment ground test.
//!
//! A per-point flag layer (`flags`, match maps only) marks woods. This is a
//! stand-in for the two-tier tiled heightfield of §5.10.1. It is immutable
//! after generation, so it is hashed once (at creation and on load).

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

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Heightfield {
    size_m: i32,
    /// Points per side (`size_m / SPACING_M + 1`).
    dim: u32,
    heights: Vec<Fx>,
    /// Per grid point `FLAG_*` bits; empty when the generator sets none.
    flags: Vec<u8>,
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
        Heightfield {
            size_m,
            dim,
            heights,
            flags,
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
