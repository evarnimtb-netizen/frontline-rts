//! Plain-Rust mesh builders (no Godot types), so they can be unit tested.
//!
//! Coordinates are Godot model space: Y up, forward −Z, right +X.
//! Builders take triangles counter-clockwise as seen from outside (the usual
//! right-hand rule) and store them clockwise, because Godot treats clockwise
//! triangles as front faces.

/// Per-vertex skinning for the bone-texture shader: `[bone0, bone1, w0, w1]`
/// packed into the mesh's CUSTOM0 channel (RGBA float).
pub type Skin = [f32; 4];

/// Rigid binding to one bone.
pub const fn rigid(bone: usize) -> Skin {
    [bone as f32, bone as f32, 1.0, 0.0]
}

/// Vertex colour alpha = 1 marks "uniform" parts that take the team tint.
pub const TEAM_TINT: f32 = 1.0;
pub const FIXED: f32 = 0.0;

#[derive(Clone, Debug, Default)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    /// Empty for meshes that are not skinned.
    pub custom0: Vec<Skin>,
    /// Clockwise (Godot front-face) triangle list.
    pub indices: Vec<i32>,
}

impl MeshData {
    pub fn tri_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub(crate) fn vert(
        &mut self,
        p: [f32; 3],
        n: [f32; 3],
        c: [f32; 4],
        skin: Option<Skin>,
    ) -> i32 {
        self.positions.push(p);
        self.normals.push(normalize(n));
        self.colors.push(c);
        if let Some(s) = skin {
            self.custom0.push(s);
        }
        (self.positions.len() - 1) as i32
    }

    /// Pushes a triangle given counter-clockwise from outside.
    pub(crate) fn tri_ccw(&mut self, a: i32, b: i32, c: i32) {
        self.indices.extend_from_slice(&[a, c, b]);
    }

    /// Axis-aligned box.
    pub fn add_box(
        &mut self,
        center: [f32; 3],
        half: [f32; 3],
        color: [f32; 4],
        skin: Option<Skin>,
    ) {
        // (normal, u, v) with u × v = normal.
        let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
            ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
            ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
            ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
            ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
        ];
        for (n, u, v) in faces {
            let corner = |su: f32, sv: f32| {
                let mut p = [0.0; 3];
                for i in 0..3 {
                    p[i] = center[i] + half[i] * (n[i] + su * u[i] + sv * v[i]);
                }
                p
            };
            let a = self.vert(corner(-1.0, -1.0), n, color, skin);
            let b = self.vert(corner(1.0, -1.0), n, color, skin);
            let c = self.vert(corner(1.0, 1.0), n, color, skin);
            let d = self.vert(corner(-1.0, 1.0), n, color, skin);
            self.tri_ccw(a, b, c);
            self.tri_ccw(a, c, d);
        }
    }

    /// Tapered elliptical tube from `p0` to `p1` with end caps. `squash`
    /// scales the cross-section along the second perpendicular (for a flat
    /// torso). `skin(t)` gives the binding at `t` in 0..=1 along the axis.
    #[allow(clippy::too_many_arguments)]
    pub fn add_tube(
        &mut self,
        p0: [f32; 3],
        p1: [f32; 3],
        r0: f32,
        r1: f32,
        squash: f32,
        sides: u32,
        rings: u32,
        color: [f32; 4],
        skin: Option<&dyn Fn(f32) -> Skin>,
    ) {
        let axis = sub(p1, p0);
        let a = normalize(axis);
        let (e1, e2) = perpendiculars(a);
        let ring_point = |t: f32, j: u32| -> ([f32; 3], [f32; 3]) {
            let th = j as f32 / sides as f32 * std::f32::consts::TAU;
            let (s, c) = th.sin_cos();
            let r = r0 + (r1 - r0) * t;
            let c3 = add(p0, scale(axis, t));
            let off = add(scale(e1, c * r), scale(e2, s * r * squash));
            let n = add(scale(e1, c), scale(e2, s / squash.max(1e-3)));
            (add(c3, off), n)
        };
        let sk = |t: f32| skin.map(|f| f(t));
        let base = self.positions.len() as i32;
        for k in 0..=rings {
            let t = k as f32 / rings as f32;
            for j in 0..sides {
                let (p, n) = ring_point(t, j);
                self.vert(p, n, color, sk(t));
            }
        }
        for k in 0..rings {
            for j in 0..sides {
                let j1 = (j + 1) % sides;
                let i = |kk: u32, jj: u32| base + (kk * sides + jj) as i32;
                self.tri_ccw(i(k, j), i(k, j1), i(k + 1, j1));
                self.tri_ccw(i(k, j), i(k + 1, j1), i(k + 1, j));
            }
        }
        // Caps.
        for (t, n) in [(0.0f32, scale(a, -1.0)), (1.0f32, a)] {
            let center = add(p0, scale(axis, t));
            let c = self.vert(center, n, color, sk(t));
            let ring0 = self.positions.len() as i32;
            for j in 0..sides {
                let (p, _) = ring_point(t, j);
                self.vert(p, n, color, sk(t));
            }
            for j in 0..sides {
                let r = ring0 + j as i32;
                let r1 = ring0 + ((j + 1) % sides) as i32;
                if t == 0.0 {
                    self.tri_ccw(c, r1, r);
                } else {
                    self.tri_ccw(c, r, r1);
                }
            }
        }
    }

    /// Ellipsoid (UV sphere) around `center` with radii `r`.
    pub fn add_ellipsoid(
        &mut self,
        center: [f32; 3],
        r: [f32; 3],
        sides: u32,
        stacks: u32,
        color: [f32; 4],
        skin: Option<Skin>,
    ) {
        let base = self.positions.len() as i32;
        for k in 0..=stacks {
            let phi =
                -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * k as f32 / stacks as f32;
            let (sp, cp) = phi.sin_cos();
            for j in 0..sides {
                let th = j as f32 / sides as f32 * std::f32::consts::TAU;
                let (st, ct) = th.sin_cos();
                // Frame (e1, e2, a) = (Z, X, Y): Z × X = Y, so the winding
                // matches `add_tube`.
                let d = [cp * st, sp, cp * ct];
                let p = [
                    center[0] + r[0] * d[0],
                    center[1] + r[1] * d[1],
                    center[2] + r[2] * d[2],
                ];
                let n = [d[0] / r[0], d[1] / r[1], d[2] / r[2]];
                self.vert(p, n, color, skin);
            }
        }
        for k in 0..stacks {
            for j in 0..sides {
                let j1 = (j + 1) % sides;
                let i = |kk: u32, jj: u32| base + (kk * sides + jj) as i32;
                // Skip the zero-area triangles at the poles.
                if k != 0 {
                    self.tri_ccw(i(k, j), i(k, j1), i(k + 1, j1));
                }
                if k != stacks - 1 {
                    self.tri_ccw(i(k, j), i(k + 1, j1), i(k + 1, j));
                }
            }
        }
    }
}

/// Placeholder tank (NOT a sim entity): hull, tracks, turret, barrel. Faces −Z.
pub fn tank_mesh() -> MeshData {
    let mut m = MeshData::default();
    let hull = [1.0, 1.0, 1.0, TEAM_TINT];
    let dark = [0.12, 0.12, 0.11, FIXED];
    m.add_box([0.0, 0.95, 0.0], [1.45, 0.45, 3.0], hull, None);
    m.add_box([-1.45, 0.55, 0.0], [0.33, 0.55, 3.15], dark, None);
    m.add_box([1.45, 0.55, 0.0], [0.33, 0.55, 3.15], dark, None);
    m.add_box([0.0, 1.75, 0.3], [1.0, 0.38, 1.3], hull, None);
    m.add_tube(
        [0.0, 1.8, -1.0],
        [0.0, 1.8, -4.6],
        0.1,
        0.08,
        1.0,
        8,
        1,
        dark,
        None,
    );
    m
}

/// Tracer streak: unit box from z = 0 (head) to z = +1 (tail), 1 × 1 across.
/// The instance basis scales Z by the streak length and X/Y by its width.
pub fn tracer_mesh() -> MeshData {
    let mut m = MeshData::default();
    m.add_box(
        [0.0, 0.0, 0.5],
        [0.5, 0.5, 0.5],
        [1.0, 1.0, 1.0, FIXED],
        None,
    );
    m
}

/// Terrain grid from sim heights (row-major, `dim × dim`, `spacing` metres).
/// Sim (x east, y north, z up) maps to Godot (x − half, z, −(y − half)).
/// `stride` skips grid points to cap the vertex count on large maps; the
/// last row and column are always kept, so the mesh covers the whole map
/// even when `stride` does not divide `dim - 1`.
pub fn terrain_mesh(heights: &[f32], dim: usize, spacing: f32, stride: usize) -> MeshData {
    let half = (dim - 1) as f32 * spacing * 0.5;
    let stride = stride.max(1);
    let mut grid: Vec<usize> = (0..dim).step_by(stride).collect();
    if grid.last() != Some(&(dim - 1)) {
        grid.push(dim - 1);
    }
    let n = grid.len();
    let h = |gx: usize, gy: usize| heights[gy.min(dim - 1) * dim + gx.min(dim - 1)];
    let mut m = MeshData::default();
    for j in 0..n {
        for i in 0..n {
            let (gx, gy) = (grid[i], grid[j]);
            let y = h(gx, gy);
            // Central-difference normal in sim axes, then converted.
            let dx = (h(gx + 1, gy) - h(gx.saturating_sub(1), gy)) / (2.0 * spacing);
            let dy = (h(gx, gy + 1) - h(gx, gy.saturating_sub(1))) / (2.0 * spacing);
            let n_sim = [-dx, -dy, 1.0];
            let normal = [n_sim[0], n_sim[2], -n_sim[1]];
            let slope = (dx * dx + dy * dy).sqrt();
            let t = ((y + 14.0) / 28.0).clamp(0.0, 1.0);
            let grass = [0.27 + 0.08 * t, 0.33 + 0.05 * t, 0.17];
            let dirt = [0.36, 0.31, 0.22];
            let k = (slope * 2.5).clamp(0.0, 1.0);
            let c = [
                grass[0] + (dirt[0] - grass[0]) * k,
                grass[1] + (dirt[1] - grass[1]) * k,
                grass[2] + (dirt[2] - grass[2]) * k,
                1.0,
            ];
            let px = gx as f32 * spacing - half;
            let pz = -(gy as f32 * spacing - half);
            m.vert([px, y, pz], normal, c, None);
        }
    }
    for j in 0..n - 1 {
        for i in 0..n - 1 {
            let v = |ii: usize, jj: usize| (jj * n + ii) as i32;
            // Seen from above (+Y), +x right and +sim-y = −Z: a quad
            // (i,j) → (i+1,j) → (i+1,j+1) is counter-clockwise.
            m.tri_ccw(v(i, j), v(i + 1, j), v(i + 1, j + 1));
            m.tri_ccw(v(i, j), v(i + 1, j + 1), v(i, j + 1));
        }
    }
    m
}

// ---------------------------------------------------------------- math --

pub fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn scale(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}
pub fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub fn normalize(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt();
    if l > 1e-12 {
        scale(a, 1.0 / l)
    } else {
        [0.0, 1.0, 0.0]
    }
}

/// Two unit vectors with `e1 × e2 = a`.
pub fn perpendiculars(a: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let helper = if a[1].abs() < 0.9 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let e1 = normalize(cross(helper, a));
    let e2 = cross(a, e1);
    (e1, e2)
}

#[cfg(test)]
pub(crate) fn winding_ok_fraction(m: &MeshData) -> f32 {
    let mut ok = 0;
    let mut total = 0;
    for t in m.indices.chunks(3) {
        let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
        let g = cross(
            sub(m.positions[b], m.positions[a]),
            sub(m.positions[c], m.positions[a]),
        );
        if dot(g, g) < 1e-14 {
            continue;
        }
        let n = add(add(m.normals[a], m.normals[b]), m.normals[c]);
        total += 1;
        // Stored clockwise, so the geometric normal points inward.
        if dot(g, n) < 0.0 {
            ok += 1;
        }
    }
    ok as f32 / total.max(1) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_and_tube_winding_is_clockwise_front() {
        let mut m = MeshData::default();
        m.add_box([0.0; 3], [1.0, 2.0, 3.0], [1.0; 4], None);
        m.add_tube(
            [0.0; 3],
            [0.3, 1.0, -0.2],
            0.2,
            0.1,
            0.6,
            10,
            3,
            [1.0; 4],
            None,
        );
        m.add_ellipsoid([0.0; 3], [0.2, 0.3, 0.2], 10, 8, [1.0; 4], None);
        assert_eq!(winding_ok_fraction(&m), 1.0);
    }

    #[test]
    fn terrain_winding_and_size() {
        let dim = 9;
        let heights: Vec<f32> = (0..dim * dim).map(|i| (i % 7) as f32 * 0.3).collect();
        let m = terrain_mesh(&heights, dim, 4.0, 1);
        assert_eq!(m.positions.len(), dim * dim);
        assert_eq!(m.tri_count(), 2 * (dim - 1) * (dim - 1));
        assert!(winding_ok_fraction(&m) > 0.99);
        // Centred on the origin.
        assert_eq!(m.positions[0][0], -16.0);
        assert_eq!(m.positions[0][2], 16.0);
    }

    #[test]
    fn terrain_keeps_edge_when_stride_does_not_divide() {
        // 10 cells, stride 3: points 0, 3, 6, 9 and the edge at 10.
        let dim = 11;
        let heights = vec![0.0; dim * dim];
        let m = terrain_mesh(&heights, dim, 4.0, 3);
        assert_eq!(m.positions.len(), 25);
        let max_x = m.positions.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
        let min_z = m.positions.iter().map(|p| p[2]).fold(f32::MAX, f32::min);
        assert_eq!(
            (max_x, min_z),
            (20.0, -20.0),
            "east and north edges reached"
        );
        assert!(winding_ok_fraction(&m) > 0.99);
    }

    #[test]
    fn placeholder_meshes_are_small() {
        assert!(tank_mesh().tri_count() < 200);
        assert_eq!(tracer_mesh().tri_count(), 12);
    }
}
