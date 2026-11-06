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

// --------------------------------------------------------------- vehicles --

/// Box sizes of a vehicle type in metres, as the match state sends them
/// (`rules.vehicle_types`, godot/docs/match_state.md "Vehicles (S3)"). The
/// hull origin is on the ground under the hull's centre; "forward" is −Z.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VehicleDims {
    pub hull_length: f32,
    pub hull_width: f32,
    pub hull_height: f32,
    /// Belly height above the ground.
    pub hull_ground: f32,
    /// Turret pivot forward of the hull centre, and its height.
    pub turret_x: f32,
    pub turret_base: f32,
    pub turret_length: f32,
    pub turret_width: f32,
    pub turret_height: f32,
    /// Turret box centre forward of the pivot.
    pub turret_offset_x: f32,
    /// Muzzle forward of, and above, the pivot.
    pub gun_length: f32,
    pub gun_height: f32,
    /// Upper front plate slope from vertical (presenter look only: a steep
    /// glacis reads as a Sherman, a near-vertical one as a Panzer IV).
    pub glacis_deg: f32,
    /// S7: a casemate gun: the "turret" part is the casemate block, fixed
    /// to the hull, from `turret_floor` (below the pivot) up; only the gun
    /// turns (by the state's turret yaw) about the pivot.
    pub casemate: bool,
    pub turret_floor: f32,
    /// S7: an open-topped turret: walls, no roof.
    pub open_top: bool,
}

impl Default for VehicleDims {
    /// The Panzer IV H of the S3 example def.
    fn default() -> Self {
        VehicleDims {
            hull_length: 5.8,
            hull_width: 2.8,
            hull_height: 1.36,
            hull_ground: 0.40,
            turret_x: 0.20,
            turret_base: 1.76,
            turret_length: 2.3,
            turret_width: 1.9,
            turret_height: 0.9,
            turret_offset_x: 0.0,
            gun_length: 4.5,
            gun_height: 0.45,
            glacis_deg: 10.0,
            casemate: false,
            turret_floor: 0.0,
            open_top: false,
        }
    }
}

impl VehicleDims {
    /// Gun trunnion (the pitch axis) relative to the turret pivot: forward
    /// distance and height. Near the turret front, at the muzzle height.
    pub fn trunnion(&self) -> (f32, f32) {
        // S7: a casemate gun pitches at its pivot in the front plate.
        if self.casemate {
            return (0.0, self.gun_height);
        }
        let fwd = self.turret_offset_x + self.turret_length * 0.42;
        (fwd.min(self.gun_length * 0.5), self.gun_height)
    }
}

/// Drawn pose of one vehicle (match state `vehicles[]`, Godot space).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VehiclePose {
    pub pos: [f32; 3],
    /// Hull rotation about +Y of a model facing −Z.
    pub yaw: f32,
    /// + = nose up.
    pub pitch: f32,
    /// + = left side up.
    pub roll: f32,
    /// Hull-relative, + = counter-clockwise seen from above.
    pub turret_yaw: f32,
    /// + = muzzle up.
    pub gun_pitch: f32,
}

/// Parts drawn as separate MultiMesh batches (one per type and part).
pub const VEHICLE_PARTS: [&str; 3] = ["hull", "turret", "gun"];

impl MeshData {
    /// Convex polygon `profile` of (z, y) points, extruded across x from
    /// `x0` to `x1` (`x0 < x1`). The points go counter-clockwise with z as
    /// the horizontal and y as the vertical axis (as seen from −x). Gives
    /// sloped front and rear plates.
    pub fn add_prism_x(&mut self, profile: &[[f32; 2]], x0: f32, x1: f32, color: [f32; 4]) {
        let n = profile.len();
        let p = |i: usize, x: f32| [x, profile[i % n][1], profile[i % n][0]];
        // Side faces.
        for i in 0..n {
            let (a, b) = (profile[i], profile[(i + 1) % n]);
            // Outward normal of the edge in that plane.
            let (dz, dy) = (b[0] - a[0], b[1] - a[1]);
            let nrm = [0.0, -dz, dy];
            let v0 = self.vert(p(i, x0), nrm, color, None);
            let v1 = self.vert(p(i + 1, x0), nrm, color, None);
            let v2 = self.vert(p(i + 1, x1), nrm, color, None);
            let v3 = self.vert(p(i, x1), nrm, color, None);
            self.tri_ccw(v0, v3, v2);
            self.tri_ccw(v0, v2, v1);
        }
        // End caps.
        for (x, nx) in [(x1, 1.0f32), (x0, -1.0)] {
            let base = self.positions.len() as i32;
            for i in 0..n {
                self.vert(p(i, x), [nx, 0.0, 0.0], color, None);
            }
            for i in 1..n as i32 - 1 {
                if nx > 0.0 {
                    self.tri_ccw(base, base + i + 1, base + i);
                } else {
                    self.tri_ccw(base, base + i, base + i + 1);
                }
            }
        }
    }

    /// Loft between two convex polygons of (x, z) points with the same
    /// count: `bottom` at height `y0`, `top` at `y1`. Either winding; faces
    /// are oriented outward from the centroid. Sloped turret sides, both
    /// caps.
    pub fn add_loft_y(
        &mut self,
        bottom: &[[f32; 2]],
        y0: f32,
        top: &[[f32; 2]],
        y1: f32,
        color: [f32; 4],
    ) {
        let n = bottom.len().min(top.len());
        let pb = |i: usize| [bottom[i % n][0], y0, bottom[i % n][1]];
        let pt = |i: usize| [top[i % n][0], y1, top[i % n][1]];
        let mut centre = [0.0; 3];
        for i in 0..n {
            centre = add(centre, scale(add(pb(i), pt(i)), 0.5 / n as f32));
        }
        for i in 0..n {
            let quad = [pb(i), pb(i + 1), pt(i + 1), pt(i)];
            self.add_outward_quad(quad, centre, color);
        }
        for (pts, y, ny) in [(top, y1, 1.0f32), (bottom, y0, -1.0)] {
            let base = self.positions.len() as i32;
            for q in pts.iter().take(n) {
                self.vert([q[0], y, q[1]], [0.0, ny, 0.0], color, None);
            }
            // Geometric normal of (0, 1, 2) taken counter-clockwise.
            let p = |k: usize| [pts[k][0], y, pts[k][1]];
            let g = cross(sub(p(1), p(0)), sub(p(2), p(0)));
            let flip = g[1] * ny < 0.0;
            for i in 1..n as i32 - 1 {
                if flip {
                    self.tri_ccw(base, base + i + 1, base + i);
                } else {
                    self.tri_ccw(base, base + i, base + i + 1);
                }
            }
        }
    }

    /// Planar quad `q` (in order around its edge), wound and lit to face
    /// away from `inside`.
    fn add_outward_quad(&mut self, q: [[f32; 3]; 4], inside: [f32; 3], color: [f32; 4]) {
        let mut g = cross(sub(q[1], q[0]), sub(q[2], q[0]));
        let flip = dot(g, sub(q[0], inside)) < 0.0;
        if flip {
            g = scale(g, -1.0);
        }
        let v: Vec<i32> = q.iter().map(|&p| self.vert(p, g, color, None)).collect();
        if flip {
            self.tri_ccw(v[0], v[2], v[1]);
            self.tri_ccw(v[0], v[3], v[2]);
        } else {
            self.tri_ccw(v[0], v[1], v[2]);
            self.tri_ccw(v[0], v[2], v[3]);
        }
    }
}

/// Hull part: lower hull, two tracks with sloped ends, fenders, the
/// superstructure with its glacis, an engine deck and exhausts. Team
/// tinted except the running gear. Origin on the ground under the hull
/// centre, facing −Z.
pub fn vehicle_hull_mesh(d: &VehicleDims) -> MeshData {
    let mut m = MeshData::default();
    let paint = [1.0, 1.0, 1.0, TEAM_TINT];
    let shade = [0.82, 0.82, 0.80, TEAM_TINT];
    let track = [0.13, 0.12, 0.11, FIXED];
    let wheels = [0.24, 0.23, 0.21, FIXED];
    let metal = [0.20, 0.19, 0.17, FIXED];
    let (l, w, h, g) = (d.hull_length, d.hull_width, d.hull_height, d.hull_ground);
    let hl = l * 0.5;
    let tw = (w * 0.17).clamp(0.3, 0.6);
    let track_top = g + h * 0.45;
    // Tracks: side profile with sloped front and rear runs (z forward = −).
    for side in [-1.0f32, 1.0] {
        let x_out = side * w * 0.5;
        let x_in = side * (w * 0.5 - tw);
        let (x0, x1) = if side < 0.0 {
            (x_out, x_in)
        } else {
            (x_in, x_out)
        };
        let prof = [
            [-hl * 0.80, 0.0],
            [hl * 0.80, 0.0],
            [hl, g * 0.9],
            [hl * 0.96, track_top],
            [-hl * 0.98, track_top],
            [-hl, g * 0.9],
        ];
        m.add_prism_x(&prof, x0, x1, track);
        // Road-wheel band on the outside face.
        m.add_box(
            [side * (w * 0.5 + 0.01), g * 0.85, 0.0],
            [0.02, g * 0.45, hl * 0.74],
            wheels,
            None,
        );
    }
    // Lower hull between the tracks.
    m.add_box(
        [0.0, g + h * 0.22, 0.0],
        [w * 0.5 - tw, h * 0.22, hl * 0.94],
        shade,
        None,
    );
    // Fenders over the tracks.
    for side in [-1.0f32, 1.0] {
        m.add_box(
            [side * (w * 0.5 - tw * 0.5), track_top + 0.03, -0.02],
            [tw * 0.5 + 0.02, 0.03, hl * 0.99],
            shade,
            None,
        );
    }
    // Superstructure: full width, sloped glacis at the front, a short
    // sloped plate at the rear.
    let top = g + h;
    let rise = top - track_top;
    let glacis = rise * d.glacis_deg.to_radians().tan().clamp(0.0, 2.5);
    let sup = [
        [-hl * 0.97 + glacis, top],
        [-hl * 0.97, track_top],
        [hl * 0.97, track_top],
        [hl * 0.92, top],
    ];
    // Profile in (z, y), counter-clockwise seen from +x: front is −z.
    let sup_ccw = [sup[1], sup[2], sup[3], sup[0]];
    m.add_prism_x(&sup_ccw, -w * 0.47, w * 0.47, paint);
    // Engine deck grille and two exhausts at the rear.
    m.add_box(
        [0.0, top + 0.03, hl * 0.62],
        [w * 0.3, 0.03, hl * 0.22],
        metal,
        None,
    );
    for side in [-1.0f32, 1.0] {
        m.add_tube(
            [side * w * 0.22, g + h * 0.55, hl * 0.99],
            [side * w * 0.22, g + h * 0.55, hl * 1.06],
            0.09,
            0.09,
            1.0,
            6,
            1,
            metal,
            None,
        );
    }
    // Driver's visor block on the front plate.
    m.add_box(
        [-w * 0.2, top - 0.08, -hl * 0.97 + glacis * 0.9],
        [0.2, 0.07, 0.06],
        metal,
        None,
    );
    m
}

/// Turret part: sloped-sided turret, mantlet and commander's cupola. Origin
/// at the turret pivot (turret ring centre, ring height), facing −Z.
pub fn vehicle_turret_mesh(d: &VehicleDims) -> MeshData {
    let mut m = MeshData::default();
    let paint = [1.0, 1.0, 1.0, TEAM_TINT];
    let metal = [0.22, 0.21, 0.19, FIXED];
    let (tl, tw, th) = (d.turret_length * 0.5, d.turret_width * 0.5, d.turret_height);
    let cz = -d.turret_offset_x;
    if d.casemate {
        // S7 blockout: a low casemate block, its front sloped back (a
        // narrower top), from the floor up; origin at the gun pivot.
        let y0 = d.turret_floor;
        let bottom = vec![[-tw, cz - tl], [-tw, cz + tl], [tw, cz + tl], [tw, cz - tl]];
        let top = vec![
            [-tw * 0.9, cz - tl * 0.7],
            [-tw * 0.9, cz + tl * 0.95],
            [tw * 0.9, cz + tl * 0.95],
            [tw * 0.9, cz - tl * 0.7],
        ];
        m.add_loft_y(&bottom, y0, &top, y0 + th, paint);
        // The mantlet (Saukopf) around the gun at the pivot.
        m.add_box([0.0, d.gun_height, -0.05], [0.28, 0.24, 0.18], paint, None);
        return m;
    }
    if d.open_top {
        // S7 blockout: an open-topped turret: four 6 cm walls, no roof.
        let wall = 0.03;
        let h2 = th * 0.5;
        m.add_box([0.0, h2, cz - tl + wall], [tw, h2, wall], paint, None);
        m.add_box([0.0, h2, cz + tl - wall], [tw, h2, wall], paint, None);
        m.add_box([-tw + wall, h2, cz], [wall, h2, tl], paint, None);
        m.add_box([tw - wall, h2, cz], [wall, h2, tl], paint, None);
        // The gun's mount at the front wall.
        let (tf, ty) = d.trunnion();
        m.add_box([0.0, ty, -tf], [0.22, 0.2, 0.18], metal, None);
        return m;
    }
    // Top view (x, z): the front (−z) is narrower, the sides taper to the
    // roof.
    let ring = |sx: f32, sz: f32| -> Vec<[f32; 2]> {
        vec![
            [-tw * 0.62 * sx, cz - tl * sz],
            [-tw * sx, cz - tl * 0.45 * sz],
            [-tw * sx, cz + tl * 0.7 * sz],
            [-tw * 0.75 * sx, cz + tl * sz],
            [tw * 0.75 * sx, cz + tl * sz],
            [tw * sx, cz + tl * 0.7 * sz],
            [tw * sx, cz - tl * 0.45 * sz],
            [tw * 0.62 * sx, cz - tl * sz],
        ]
    };
    let bottom = ring(1.0, 1.0);
    let top = ring(0.84, 0.9);
    m.add_loft_y(&bottom, 0.0, &top, th, paint);
    // Mantlet around the gun's root.
    let (tf, ty) = d.trunnion();
    m.add_box(
        [0.0, ty, -tf - 0.08],
        [tw * 0.42, th * 0.3, 0.14],
        paint,
        None,
    );
    // Cupola, rear left.
    m.add_tube(
        [-tw * 0.45, th, cz + tl * 0.45],
        [-tw * 0.45, th + 0.28, cz + tl * 0.45],
        0.3,
        0.27,
        1.0,
        8,
        1,
        metal,
        None,
    );
    m
}

/// Gun part: barrel and muzzle brake. Origin at the trunnion (the pitch
/// axis), barrel along −Z to the muzzle.
pub fn vehicle_gun_mesh(d: &VehicleDims) -> MeshData {
    let mut m = MeshData::default();
    let dark = [0.17, 0.16, 0.15, FIXED];
    let (tf, _) = d.trunnion();
    let len = (d.gun_length - tf).max(0.5);
    m.add_tube(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, -len + 0.25],
        0.1,
        0.075,
        1.0,
        8,
        1,
        dark,
        None,
    );
    m.add_tube(
        [0.0, 0.0, -len + 0.28],
        [0.0, 0.0, -len],
        0.11,
        0.11,
        1.0,
        8,
        1,
        dark,
        None,
    );
    m
}

/// One vehicle part by name ("hull", "turret", "gun").
pub fn vehicle_part_mesh(d: &VehicleDims, part: &str) -> Option<MeshData> {
    match part {
        "hull" => Some(vehicle_hull_mesh(d)),
        "turret" => Some(vehicle_turret_mesh(d)),
        "gun" => Some(vehicle_gun_mesh(d)),
        _ => None,
    }
}

/// A 3×4 transform as Godot's MultiMesh buffer stores it (rows).
pub type Rows = [f32; 12];

fn mat_mul(a: &[[f32; 3]; 3], b: &[[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let mut o = [[0.0; 3]; 3];
    for (i, row) in o.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    o
}

fn mat_vec(a: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [dot(a[0], v), dot(a[1], v), dot(a[2], v)]
}

fn rot_y(t: f32) -> [[f32; 3]; 3] {
    let (s, c) = t.sin_cos();
    [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]]
}

fn rot_x(t: f32) -> [[f32; 3]; 3] {
    let (s, c) = t.sin_cos();
    [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]]
}

fn rot_z(t: f32) -> [[f32; 3]; 3] {
    let (s, c) = t.sin_cos();
    [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]
}

fn rows(b: &[[f32; 3]; 3], o: [f32; 3]) -> Rows {
    [
        b[0][0], b[0][1], b[0][2], o[0], //
        b[1][0], b[1][1], b[1][2], o[1], //
        b[2][0], b[2][1], b[2][2], o[2],
    ]
}

/// World transforms of a vehicle's hull, turret and gun, composed hull ×
/// turret yaw × gun pitch. Hull: yaw about +Y, then pitch about the hull's
/// right axis (+ raises the −Z nose), then roll about its forward axis
/// (+ raises the left, −X, side). The turret turns about the hull's up axis
/// at its pivot; the gun pitches about the turret's right axis at the
/// trunnion.
pub fn compose_vehicle(p: &VehiclePose, d: &VehicleDims) -> [Rows; 3] {
    let hull = mat_mul(&mat_mul(&rot_y(p.yaw), &rot_x(p.pitch)), &rot_z(-p.roll));
    let pivot = add(p.pos, mat_vec(&hull, [0.0, d.turret_base, -d.turret_x]));
    let turret = mat_mul(&hull, &rot_y(p.turret_yaw));
    let (tf, ty) = d.trunnion();
    let trunnion = add(pivot, mat_vec(&turret, [0.0, ty, -tf]));
    let gun = mat_mul(&turret, &rot_x(p.gun_pitch));
    // S7: a casemate block stays with the hull; only the gun turns.
    let upper = if d.casemate { hull } else { turret };
    [
        rows(&hull, p.pos),
        rows(&upper, pivot),
        rows(&gun, trunnion),
    ]
}

/// Floats per vehicle in a packed pose list: position (3), yaw, pitch,
/// roll, turret yaw, gun pitch, then 4 custom-data floats (tint rgb and
/// shade) copied to every part's instance.
pub const POSE_STRIDE: usize = 12;

/// Fills the three part buffers (16 floats per instance: 3×4 rows, then
/// the custom data) for every pose in `poses`, padded with zeros to `cap`
/// instances (Godot wants the whole buffer). Poses past `cap` are dropped.
/// Returns the instance count written.
pub fn vehicle_buffers(
    poses: &[f32],
    d: &VehicleDims,
    cap: usize,
    out: &mut [Vec<f32>; 3],
) -> usize {
    const STRIDE: usize = 16;
    let n = (poses.len() / POSE_STRIDE).min(cap);
    for buf in out.iter_mut() {
        buf.clear();
        buf.resize(cap * STRIDE, 0.0);
    }
    for i in 0..n {
        let q = &poses[i * POSE_STRIDE..(i + 1) * POSE_STRIDE];
        let pose = VehiclePose {
            pos: [q[0], q[1], q[2]],
            yaw: q[3],
            pitch: q[4],
            roll: q[5],
            turret_yaw: q[6],
            gun_pitch: q[7],
        };
        let parts = compose_vehicle(&pose, d);
        for (k, rows) in parts.iter().enumerate() {
            let o = &mut out[k][i * STRIDE..(i + 1) * STRIDE];
            o[..12].copy_from_slice(rows);
            o[12..].copy_from_slice(&q[8..12]);
        }
    }
    n
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

// ------------------------------------------------------------ Godot API --

/// GDScript access to the vehicle meshes and the instance composition, so
/// the S3 presenter (`godot/match/vehicle_view.gd`) draws the same parts
/// on the mock source and on the bridge's state. Static functions only:
///
/// ```gdscript
/// var mesh := FrontlineMeshes.vehicle_part(dims, "turret")
/// var bufs := FrontlineMeshes.vehicle_buffers(dims, poses, cap)  # [hull, turret, gun]
/// ```
mod gd_api {
    use godot::classes::{ArrayMesh, RefCounted};
    use godot::prelude::*;

    use super::{VEHICLE_PARTS, VehicleDims, vehicle_buffers, vehicle_part_mesh};

    #[derive(GodotClass)]
    #[class(base = RefCounted, no_init)]
    pub struct FrontlineMeshes {
        base: Base<RefCounted>,
    }

    /// Dims from a `rules.vehicle_types` entry (missing keys: the default).
    fn dims(d: &VarDictionary) -> VehicleDims {
        let mut out = VehicleDims::default();
        let get = |k: &str, dflt: f32| -> f32 {
            d.get(k)
                .and_then(|v| {
                    v.try_to::<f64>()
                        .ok()
                        .or_else(|| v.try_to::<i64>().ok().map(|i| i as f64))
                })
                .map_or(dflt, |x| x as f32)
        };
        out.hull_length = get("hull_length", out.hull_length);
        out.hull_width = get("hull_width", out.hull_width);
        out.hull_height = get("hull_height", out.hull_height);
        out.hull_ground = get("hull_ground", out.hull_ground);
        out.turret_x = get("turret_x", out.turret_x);
        out.turret_base = get("turret_base", out.turret_base);
        out.turret_length = get("turret_length", out.turret_length);
        out.turret_width = get("turret_width", out.turret_width);
        out.turret_height = get("turret_height", out.turret_height);
        out.turret_offset_x = get("turret_offset_x", out.turret_offset_x);
        out.gun_length = get("gun_length", out.gun_length);
        out.gun_height = get("gun_height", out.gun_height);
        out.glacis_deg = get("glacis_deg", out.glacis_deg);
        out.turret_floor = get("turret_floor", out.turret_floor);
        let flag = |k: &str| {
            d.get(k)
                .and_then(|v| v.try_to::<bool>().ok())
                .unwrap_or(false)
        };
        out.casemate = flag("casemate");
        out.open_top = flag("open_top");
        out
    }

    #[godot_api]
    impl FrontlineMeshes {
        /// One part mesh ("hull", "turret" or "gun") for a vehicle type's
        /// box sizes; null for an unknown part. Vertex colour alpha 1 marks
        /// team-tinted faces (no material set).
        #[func]
        fn vehicle_part(dims_dict: VarDictionary, part: GString) -> Option<Gd<ArrayMesh>> {
            let md = vehicle_part_mesh(&dims(&dims_dict), &part.to_string())?;
            Some(crate::gfx::build_mesh(&md, None))
        }

        /// The part names, in buffer order.
        #[func]
        fn vehicle_parts() -> PackedStringArray {
            VEHICLE_PARTS.iter().map(|s| GString::from(*s)).collect()
        }

        /// Three MultiMesh buffers (hull, turret, gun), each `cap` × 16
        /// floats, from packed poses (`meshgen::POSE_STRIDE` floats per
        /// vehicle: x y z, yaw, pitch, roll, turret yaw, gun pitch, custom
        /// r g b a). Instances past the pose count are zero.
        #[func]
        fn vehicle_buffers(
            dims_dict: VarDictionary,
            poses: PackedFloat32Array,
            cap: i64,
        ) -> VarArray {
            let mut out: [Vec<f32>; 3] = Default::default();
            vehicle_buffers(
                poses.as_slice(),
                &dims(&dims_dict),
                cap.max(0) as usize,
                &mut out,
            );
            let mut arr = VarArray::new();
            for b in out {
                arr.push(&PackedFloat32Array::from(b.as_slice()).to_variant());
            }
            arr
        }
    }
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

    fn sherman() -> VehicleDims {
        VehicleDims {
            hull_length: 5.9,
            hull_width: 2.6,
            hull_height: 1.5,
            hull_ground: 0.43,
            turret_x: 0.1,
            turret_base: 1.93,
            turret_length: 2.2,
            turret_width: 1.8,
            turret_height: 0.85,
            turret_offset_x: -0.1,
            gun_length: 4.0,
            gun_height: 0.4,
            glacis_deg: 47.0,
            ..VehicleDims::default()
        }
    }

    #[test]
    fn vehicle_parts_are_small_and_closed() {
        for d in [VehicleDims::default(), sherman()] {
            for part in VEHICLE_PARTS {
                let m = vehicle_part_mesh(&d, part).unwrap();
                assert!(m.tri_count() < 400, "{part}: {} triangles", m.tri_count());
                assert!(m.tri_count() >= 12, "{part}: {} triangles", m.tri_count());
                assert_eq!(winding_ok_fraction(&m), 1.0, "{part} winding");
                assert!(m.custom0.is_empty());
            }
        }
        assert!(vehicle_part_mesh(&VehicleDims::default(), "wheel").is_none());
    }

    #[test]
    fn hull_mesh_fits_its_box() {
        let d = VehicleDims::default();
        let m = vehicle_hull_mesh(&d);
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &m.positions {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        assert!(lo[1] >= -1e-4 && (hi[1] - (d.hull_ground + d.hull_height)).abs() < 0.08);
        assert!(hi[0] <= d.hull_width * 0.5 + 0.05 && lo[0] >= -d.hull_width * 0.5 - 0.05);
        // Exhausts stick out a little at the rear (+Z); the nose is at −Z.
        assert!(
            (lo[2] + d.hull_length * 0.5).abs() < 0.05,
            "front {}",
            lo[2]
        );
        assert!(hi[2] < d.hull_length * 0.5 + 0.3);
        // Some faces take the team tint, the running gear does not.
        assert!(m.colors.iter().any(|c| c[3] == TEAM_TINT));
        assert!(m.colors.iter().any(|c| c[3] == FIXED));
    }

    fn apply(r: &Rows, p: [f32; 3]) -> [f32; 3] {
        [
            r[0] * p[0] + r[1] * p[1] + r[2] * p[2] + r[3],
            r[4] * p[0] + r[5] * p[1] + r[6] * p[2] + r[7],
            r[8] * p[0] + r[9] * p[1] + r[10] * p[2] + r[11],
        ]
    }

    fn near(a: [f32; 3], b: [f32; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-4)
    }

    #[test]
    fn compose_vehicle_conventions() {
        let d = VehicleDims::default();
        let (tf, ty) = d.trunnion();
        let mut p = VehiclePose {
            pos: [10.0, 2.0, -5.0],
            ..Default::default()
        };
        // Level, unturned: the muzzle is straight ahead (−Z) of the pivot.
        let [hull, turret, gun] = compose_vehicle(&p, &d);
        assert!(near(apply(&hull, [0.0; 3]), p.pos));
        let pivot = [10.0, 2.0 + d.turret_base, -5.0 - d.turret_x];
        assert!(near(apply(&turret, [0.0; 3]), pivot));
        let muzzle = apply(&gun, [0.0, 0.0, -(d.gun_length - tf)]);
        assert!(near(
            muzzle,
            [pivot[0], pivot[1] + ty, pivot[2] - d.gun_length]
        ));
        // Yaw +90°: a model facing −Z faces −X (counter-clockwise from above).
        p.yaw = std::f32::consts::FRAC_PI_2;
        let [hull, ..] = compose_vehicle(&p, &d);
        let nose = sub(apply(&hull, [0.0, 0.0, -1.0]), p.pos);
        assert!(near(nose, [-1.0, 0.0, 0.0]), "{nose:?}");
        // Pitch +: nose up. Roll +: left (−X) side up.
        p.yaw = 0.0;
        p.pitch = 0.2;
        let [hull, ..] = compose_vehicle(&p, &d);
        assert!(apply(&hull, [0.0, 0.0, -1.0])[1] > p.pos[1]);
        p.pitch = 0.0;
        p.roll = 0.2;
        let [hull, ..] = compose_vehicle(&p, &d);
        assert!(apply(&hull, [-1.0, 0.0, 0.0])[1] > p.pos[1]);
        // Turret yaw + (counter-clockwise) points the gun to the left
        // (−X); gun pitch + raises the muzzle; both ride on the hull.
        p.roll = 0.0;
        p.turret_yaw = std::f32::consts::FRAC_PI_2;
        p.gun_pitch = 0.3;
        let [_, turret, gun] = compose_vehicle(&p, &d);
        let base = apply(&turret, [0.0; 3]);
        let tip = sub(apply(&gun, [0.0, 0.0, -1.0]), apply(&gun, [0.0; 3]));
        assert!(tip[0] < -0.9 && tip[1] > 0.25, "{tip:?}");
        assert!(near(base, pivot));
    }

    #[test]
    fn vehicle_buffers_layout() {
        let d = VehicleDims::default();
        let mut poses = vec![0.0; POSE_STRIDE * 2];
        poses[..3].copy_from_slice(&[1.0, 2.0, 3.0]);
        poses[8..12].copy_from_slice(&[0.5, 0.6, 0.7, 0.9]);
        poses[POSE_STRIDE + 3] = 1.0;
        let mut out: [Vec<f32>; 3] = Default::default();
        assert_eq!(vehicle_buffers(&poses, &d, 4, &mut out), 2);
        for b in &out {
            assert_eq!(b.len(), 4 * 16);
            assert_eq!(&b[12..16], &[0.5, 0.6, 0.7, 0.9]);
            assert!(b[32..].iter().all(|&x| x == 0.0), "padding is zero");
        }
        // Hull origin column, then the second vehicle's yaw in its basis.
        assert_eq!((out[0][3], out[0][7], out[0][11]), (1.0, 2.0, 3.0));
        assert!((out[0][16] - 1.0f32.cos()).abs() < 1e-6);
        // Capacity drops the rest.
        assert_eq!(vehicle_buffers(&poses, &d, 1, &mut out), 1);
        assert_eq!(out[2].len(), 16);
    }

    #[test]
    fn placeholder_meshes_are_small() {
        assert!(tank_mesh().tri_count() < 200);
        assert_eq!(tracer_mesh().tri_count(), 12);
    }

    #[test]
    fn s7_casemate_and_open_top_parts() {
        let stug = VehicleDims {
            casemate: true,
            turret_length: 3.0,
            turret_width: 2.2,
            turret_height: 0.66,
            turret_offset_x: -0.7,
            turret_floor: -0.23,
            gun_length: 3.0,
            gun_height: 0.0,
            ..VehicleDims::default()
        };
        let m = vehicle_turret_mesh(&stug);
        assert!(m.tri_count() > 0 && winding_ok_fraction(&m) > 0.99);
        // The block stays with the hull: turret yaw turns only the gun.
        let p = VehiclePose {
            turret_yaw: 0.15,
            ..VehiclePose::default()
        };
        let [hull, upper, gun] = compose_vehicle(&p, &stug);
        assert_eq!(hull[..3], upper[..3]);
        assert_ne!(gun[..3], upper[..3]);
        assert_eq!(stug.trunnion(), (0.0, 0.0));
        let m10 = VehicleDims {
            open_top: true,
            ..VehicleDims::default()
        };
        let walls = vehicle_turret_mesh(&m10);
        // Four walls and the gun mount: five boxes, no roof.
        assert_eq!(walls.tri_count(), 5 * 12);
    }
}
