//! Presenter math in plain Rust: axis conversion, interpolation, culling and
//! MultiMesh instance packing. No Godot types, so it is unit tested.
//!
//! Axes: the sim uses x east, y north, z up with the map spanning
//! `[0, size]²`. Godot is Y-up with −Z forward; the bridge maps sim north to
//! −Z and centres the map on the origin: `godot = (x − h, z, −(y − h))` with
//! `h = size / 2`.

/// Floats per instance: 12 (3×4 transform rows) + 4 custom data.
pub const STRIDE: usize = 16;

#[inline]
pub fn sim_to_godot(p: [f32; 3], half: f32) -> [f32; 3] {
    [p[0] - half, p[2], -(p[1] - half)]
}

#[inline]
pub fn godot_to_sim_xy(x: f32, z: f32, half: f32) -> [f32; 2] {
    [x + half, -z + half]
}

/// Rotation about Godot +Y that turns a model facing −Z toward the sim
/// heading `yaw` (counter-clockwise from east).
#[inline]
pub fn model_yaw(sim_yaw: f32) -> f32 {
    sim_yaw - std::f32::consts::FRAC_PI_2
}

#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Shortest-arc angle interpolation.
#[inline]
pub fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let mut d = (b - a) % TAU;
    if d > PI {
        d -= TAU;
    } else if d < -PI {
        d += TAU;
    }
    a + d * t
}

/// Writes one instance: rotation `angle` about +Y, uniform `scale`, origin
/// `pos`, then custom data. Godot's buffer layout is the 3×4 matrix by rows.
#[inline]
pub fn write_yaw_instance(
    out: &mut [f32],
    pos: [f32; 3],
    angle: f32,
    scale: f32,
    custom: [f32; 4],
) {
    let (s, c) = angle.sin_cos();
    let (s, c) = (s * scale, c * scale);
    out[..STRIDE].copy_from_slice(&[
        c, 0.0, s, pos[0], //
        0.0, scale, 0.0, pos[1], //
        -s, 0.0, c, pos[2], //
        custom[0], custom[1], custom[2], custom[3],
    ]);
}

/// Writes a streak whose local −Z points along `dir` (unit), scaled to
/// `length` along Z and `width` across. Local z ∈ [0, 1] trails behind
/// `head`.
#[inline]
pub fn write_streak_instance(
    out: &mut [f32],
    head: [f32; 3],
    dir: [f32; 3],
    length: f32,
    width: f32,
    custom: [f32; 4],
) {
    use crate::meshgen::{cross, normalize, scale};
    let z = scale(dir, -1.0);
    let helper = if dir[1].abs() < 0.95 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let x = normalize(cross(helper, z));
    let y = cross(z, x);
    let (x, y, z) = (scale(x, width), scale(y, width), scale(z, length));
    // Columns are the basis vectors; the buffer stores rows.
    out[..STRIDE].copy_from_slice(&[
        x[0], y[0], z[0], head[0], //
        x[1], y[1], z[1], head[1], //
        x[2], y[2], z[2], head[2], //
        custom[0], custom[1], custom[2], custom[3],
    ]);
}

/// Six planes `(normal, d)` with outward normals, as `Camera3D.get_frustum`
/// returns them: a point is outside when `n·p − d > r`.
#[derive(Clone, Debug, Default)]
pub struct Frustum {
    pub planes: Vec<[f32; 4]>,
}

impl Frustum {
    /// No planes: everything is visible.
    pub fn all() -> Frustum {
        Frustum { planes: Vec::new() }
    }

    #[inline]
    pub fn sphere_visible(&self, p: [f32; 3], r: f32) -> bool {
        self.planes
            .iter()
            .all(|pl| pl[0] * p[0] + pl[1] * p[1] + pl[2] * p[2] - pl[3] <= r)
    }
}

/// Builds the interpolation start for snapshot `b` from the older snapshot
/// `a`: for each id in `b_ids`, `a`'s value for that id (both lists
/// ascending), or `b`'s own value for ids that just appeared. An id whose
/// planar position jumped more than `max_jump` metres is treated as a new
/// entity too (ids are reused without a generation, so a despawn followed by
/// a spawn can hand a new soldier an old id); those ids are appended to
/// `snapped` so per-id presenter state can be reset.
pub fn carry_over(
    a_ids: &[u32],
    a_vals: &[[f32; 4]],
    b_ids: &[u32],
    b_vals: &[[f32; 4]],
    max_jump: f32,
    out: &mut Vec<[f32; 4]>,
    snapped: &mut Vec<u32>,
) {
    out.clear();
    out.reserve(b_ids.len());
    let max2 = max_jump * max_jump;
    let mut j = 0;
    for (i, &id) in b_ids.iter().enumerate() {
        while j < a_ids.len() && a_ids[j] < id {
            j += 1;
        }
        let b = b_vals[i];
        if j < a_ids.len() && a_ids[j] == id && j < a_vals.len() {
            let a = a_vals[j];
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            if dx * dx + dy * dy <= max2 {
                out.push(a);
            } else {
                out.push(b);
                snapped.push(id);
            }
        } else {
            out.push(b);
        }
    }
}

/// Picks the two snapshots to draw between at render time `r` (seconds, same
/// clock as `stamps`). `stamps` are the scheduled tick times of the frames
/// held, oldest first. Returns `(a, b, alpha)`: draw `lerp(a, b, alpha)`.
/// Before the oldest frame it holds the oldest; past the newest it holds the
/// newest (alpha clamped to 1), so the drawn state never runs ahead of the
/// sim and never feeds back into itself.
pub fn interp_pair(stamps: &[f64], r: f64) -> (usize, usize, f32) {
    let n = stamps.len();
    if n <= 1 {
        return (0, 0, 1.0);
    }
    let b = stamps.iter().position(|&t| t >= r).unwrap_or(n - 1);
    if b == 0 {
        return (0, 0, 1.0);
    }
    let a = b - 1;
    let span = stamps[b] - stamps[a];
    let alpha = if span > 0.0 {
        ((r - stamps[a]) / span).clamp(0.0, 1.0) as f32
    } else {
        1.0
    };
    (a, b, alpha)
}

/// Shadow-caster culling (§4.1 rule 4): a caster matters when the shadow it
/// throws, swept from the caster along the light direction down to the
/// ground, can land inside the camera frustum within the shadow distance.
#[derive(Clone, Debug, Default)]
pub struct CasterCull {
    pub frustum: Frustum,
    pub cam: [f32; 3],
    /// Unit camera forward (Godot −Z of the camera basis).
    pub cam_fwd: [f32; 3],
    /// Directional shadow max distance (view depth).
    pub max_depth: f32,
    /// Unit direction the light travels (Godot −Z of the light basis).
    pub light: [f32; 3],
    /// False when there is no shadowed directional light: nothing casts.
    pub enabled: bool,
}

impl CasterCull {
    /// `p` is the caster centre, `r` its bounding radius and `h` its height
    /// above the ground (sets how far the shadow can reach).
    pub fn casts_into_view(&self, p: [f32; 3], r: f32, h: f32) -> bool {
        if !self.enabled {
            return false;
        }
        let down = (-self.light[1]).max(0.15);
        let reach = (h / down).min(40.0);
        let q = [
            p[0] + self.light[0] * reach,
            p[1] + self.light[1] * reach,
            p[2] + self.light[2] * reach,
        ];
        let depth = |x: [f32; 3]| {
            (x[0] - self.cam[0]) * self.cam_fwd[0]
                + (x[1] - self.cam[1]) * self.cam_fwd[1]
                + (x[2] - self.cam[2]) * self.cam_fwd[2]
        };
        if depth(p).min(depth(q)) > self.max_depth + r {
            return false;
        }
        // Capsule p→q against each plane: outside only if both ends are.
        self.frustum.planes.iter().all(|pl| {
            let dp = pl[0] * p[0] + pl[1] * p[1] + pl[2] * p[2] - pl[3];
            let dq = pl[0] * q[0] + pl[1] * q[1] + pl[2] * q[2] - pl[3];
            dp.min(dq) <= r
        })
    }
}

/// MultiMesh capacity policy with hysteresis: grow to 1.5× what is needed
/// when `count` exceeds `cap`, shrink to 2× when `count` falls below a
/// quarter of `cap`. Rounded up to 64, clamped to `[min, max]`.
pub fn next_capacity(cap: usize, count: usize, min: usize, max: usize) -> usize {
    let round = |n: usize| n.div_ceil(64) * 64;
    let want = if count > cap {
        round(count + count / 2)
    } else if count * 4 < cap {
        round(count * 2)
    } else {
        cap
    };
    want.clamp(min.max(1), max.max(min.max(1)))
}

/// Running bounding box of the instances written this frame.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub lo: [f32; 3],
    pub hi: [f32; 3],
}

impl Default for Bounds {
    fn default() -> Self {
        Bounds {
            lo: [f32::MAX; 3],
            hi: [f32::MIN; 3],
        }
    }
}

impl Bounds {
    #[inline]
    pub fn add(&mut self, c: [f32; 3], r: f32) {
        for ((lo, hi), c) in self.lo.iter_mut().zip(&mut self.hi).zip(c) {
            *lo = lo.min(c - r);
            *hi = hi.max(c + r);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.lo[0] > self.hi[0]
    }
}

/// Cheap integer hash → [0, 1) (animation phase offsets, placeholder layout).
#[inline]
pub fn hash01(x: u32) -> f32 {
    let mut h = x.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Bilinear height lookup on a copy of the sim heightfield (Godot axes in).
#[derive(Clone, Debug, Default)]
pub struct HeightSampler {
    pub heights: Vec<f32>,
    pub dim: usize,
    pub spacing: f32,
    pub half: f32,
}

impl HeightSampler {
    pub fn at_sim(&self, x: f32, y: f32) -> f32 {
        if self.dim < 2 {
            return 0.0;
        }
        let max = (self.dim - 1) as f32;
        let gx = (x / self.spacing).clamp(0.0, max);
        let gy = (y / self.spacing).clamp(0.0, max);
        let ix = (gx as usize).min(self.dim - 2);
        let iy = (gy as usize).min(self.dim - 2);
        let (fx, fy) = (gx - ix as f32, gy - iy as f32);
        let h = |i: usize, j: usize| self.heights[j * self.dim + i];
        let a = lerp(h(ix, iy), h(ix + 1, iy), fx);
        let b = lerp(h(ix, iy + 1), h(ix + 1, iy + 1), fx);
        lerp(a, b, fy)
    }

    pub fn at_godot(&self, x: f32, z: f32) -> f32 {
        let [sx, sy] = godot_to_sim_xy(x, z, self.half);
        self.at_sim(sx, sy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transform(buf: &[f32], p: [f32; 3]) -> [f32; 3] {
        let mut o = [0.0; 3];
        for r in 0..3 {
            o[r] =
                buf[r * 4] * p[0] + buf[r * 4 + 1] * p[1] + buf[r * 4 + 2] * p[2] + buf[r * 4 + 3];
        }
        o
    }

    #[test]
    fn axes() {
        // Sim north (+y) is Godot −Z; up is +Y; map centred.
        assert_eq!(sim_to_godot([512.0, 612.0, 3.0], 512.0), [0.0, 3.0, -100.0]);
        assert_eq!(godot_to_sim_xy(0.0, -100.0, 512.0), [512.0, 612.0]);
    }

    #[test]
    fn yaw_points_model_forward_along_heading() {
        let mut buf = [0.0; STRIDE];
        for yaw in [0.0f32, 0.7, 2.0, -2.5] {
            write_yaw_instance(&mut buf, [0.0; 3], model_yaw(yaw), 1.0, [0.0; 4]);
            // Model forward is −Z.
            let f = transform(&buf, [0.0, 0.0, -1.0]);
            // Heading in sim axes (cos, sin) → Godot (cos, 0, −sin).
            let want = [yaw.cos(), 0.0, -yaw.sin()];
            for i in 0..3 {
                assert!(
                    (f[i] - want[i]).abs() < 1e-5,
                    "yaw {yaw}: {f:?} vs {want:?}"
                );
            }
        }
    }

    #[test]
    fn streak_trails_behind_head() {
        let mut buf = [0.0; STRIDE];
        let dir = crate::meshgen::normalize([1.0, 0.2, -0.5]);
        write_streak_instance(&mut buf, [10.0, 5.0, 0.0], dir, 8.0, 0.1, [0.0; 4]);
        let tail = transform(&buf, [0.0, 0.0, 1.0]);
        for i in 0..3 {
            assert!((tail[i] - (([10.0, 5.0, 0.0][i]) - dir[i] * 8.0)).abs() < 1e-4);
        }
    }

    #[test]
    fn carry_over_matches_ids_and_snaps_jumps() {
        let v = |x: f32| [x, 0.0, 0.0, 0.0];
        let (mut out, mut snapped) = (Vec::new(), Vec::new());
        carry_over(
            &[1, 3, 5],
            &[v(10.0), v(30.0), v(50.0)],
            &[0, 3, 4, 5],
            &[v(0.0), v(33.0), v(44.0), v(80.0)],
            10.0,
            &mut out,
            &mut snapped,
        );
        // 0 and 4 are new; 3 moved 3 m (interpolate); 5 jumped 30 m (snap).
        assert_eq!(out, vec![v(0.0), v(30.0), v(44.0), v(80.0)]);
        assert_eq!(snapped, vec![5]);
    }

    #[test]
    fn interp_pair_brackets_render_time() {
        let st = [1.0, 2.0, 3.0];
        assert_eq!(interp_pair(&st, 0.5), (0, 0, 1.0));
        let (a, b, t) = interp_pair(&st, 1.25);
        assert_eq!((a, b), (0, 1));
        assert!((t - 0.25).abs() < 1e-6);
        let (a, b, t) = interp_pair(&st, 2.5);
        assert_eq!((a, b), (1, 2));
        assert!((t - 0.5).abs() < 1e-6);
        // Past the newest: hold it.
        assert_eq!(interp_pair(&st, 9.0), (1, 2, 1.0));
        assert_eq!(interp_pair(&[4.0], 9.0), (0, 0, 1.0));
    }

    /// The drawn position depends only on render time, never on what was
    /// drawn before or on when frames arrive (finding: feedback lag).
    #[test]
    fn interpolation_has_no_feedback() {
        // A soldier walking +1 m per tick, ticks at t = 0, 1, 2, ...
        let pos_at = |r: f64, have: usize| {
            let stamps: Vec<f64> = (0..have).map(|k| k as f64).collect();
            let start = have.saturating_sub(3);
            let (a, b, t) = interp_pair(&stamps[start..], r);
            let (a, b) = ((start + a) as f32, (start + b) as f32);
            a + (b - a) * t
        };
        for &r in &[2.1, 2.5, 2.9, 3.3] {
            // Whichever frames are held (as long as they cover r), the
            // answer is r.
            for have in [4usize, 5, 6] {
                let oldest = have.saturating_sub(3) as f64;
                if r >= oldest && r <= (have - 1) as f64 {
                    assert!(
                        (pos_at(r, have) - r as f32).abs() < 1e-5,
                        "r {r} have {have}"
                    );
                }
            }
        }
    }

    #[test]
    fn capacity_hysteresis() {
        assert_eq!(next_capacity(64, 10, 64, 4096), 64);
        assert_eq!(next_capacity(64, 100, 64, 4096), 192);
        assert_eq!(next_capacity(192, 100, 64, 4096), 192);
        assert_eq!(next_capacity(1024, 100, 64, 4096), 256);
        assert_eq!(next_capacity(1024, 5000, 64, 2064), 2064);
    }

    #[test]
    fn caster_cull_follows_shadow() {
        // Camera at origin looking down −Z; one plane: x <= 10.
        let c = CasterCull {
            frustum: Frustum {
                planes: vec![[1.0, 0.0, 0.0, 10.0]],
            },
            cam: [0.0; 3],
            cam_fwd: [0.0, 0.0, -1.0],
            max_depth: 100.0,
            // Light going down and toward −x.
            light: crate::meshgen::normalize([-1.0, -1.0, 0.0]),
            enabled: true,
        };
        // Outside the view at x = 11, but its shadow falls back toward −x.
        assert!(c.casts_into_view([11.0, 1.0, -5.0], 0.5, 2.0));
        // Far outside: the shadow cannot reach.
        assert!(!c.casts_into_view([30.0, 1.0, -5.0], 0.5, 2.0));
        // Beyond the shadow distance.
        assert!(!c.casts_into_view([0.0, 1.0, -150.0], 0.5, 2.0));
        let off = CasterCull {
            enabled: false,
            ..c
        };
        assert!(!off.casts_into_view([0.0, 1.0, -5.0], 0.5, 2.0));
    }

    #[test]
    fn lerp_angle_wraps() {
        let v = lerp_angle(3.0, -3.0, 0.5);
        assert!((v - std::f32::consts::PI).abs() < 0.01 || (v + std::f32::consts::PI).abs() < 0.01);
    }

    #[test]
    fn frustum_test() {
        let f = Frustum {
            planes: vec![[1.0, 0.0, 0.0, 5.0]],
        };
        assert!(f.sphere_visible([4.0, 0.0, 0.0], 0.0));
        assert!(!f.sphere_visible([7.0, 0.0, 0.0], 1.0));
        assert!(f.sphere_visible([5.5, 0.0, 0.0], 1.0));
    }

    #[test]
    fn heights_bilinear() {
        let s = HeightSampler {
            heights: vec![0.0, 4.0, 0.0, 4.0],
            dim: 2,
            spacing: 4.0,
            half: 2.0,
        };
        assert!((s.at_sim(2.0, 1.0) - 2.0).abs() < 1e-6);
        assert!((s.at_godot(0.0, 0.0) - 2.0).abs() < 1e-6);
    }
}
