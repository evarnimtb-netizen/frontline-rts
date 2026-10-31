//! Procedural rigged soldier and its baked bone-matrix animation texture
//! (§4.1 rule 4, §2.2 M0 gate 7). No external assets.
//!
//! The rig has [`BONE_COUNT`] bones. Each vertex carries `[bone0, bone1, w0,
//! w1]` in CUSTOM0 (the mesh is drawn on a skeleton-less MultiMesh, so the
//! engine's BONES/WEIGHTS skinning path is never involved). The animation
//! texture is RGBA32F: one row per frame, three texels per bone holding the
//! rows of that bone's 3×4 skinning matrix (`pose × inverse bind`). The
//! shader (`godot/shaders/soldier_skinned.gdshader`) picks clip, frame and
//! blend from `INSTANCE_CUSTOM`.
//!
//! Clips: walk, then the pose clips idle (stand), crouch, prone and death,
//! in that row order. The pose clips are addressed as `IDLE_ROW0 + k ·
//! FRAMES_PER_CLIP` with `k` = [`Clip::pose_index`]; the instance writes
//! `k + phase` into one custom float, so bench instances (which write a
//! plain idle phase in `[0, 1)`) still get the idle clip. Death is the only
//! clip that does not loop: phase 0 stands, phase 1 lies on his back (the
//! corpse pose).

use crate::meshgen::{FIXED, MeshData, Skin, TEAM_TINT, rigid};

pub const BONE_COUNT: usize = 11;
/// Frames per clip (one full cycle each).
pub const FRAMES_PER_CLIP: usize = 32;
pub const WALK_ROW0: usize = 0;
pub const IDLE_ROW0: usize = FRAMES_PER_CLIP;
pub const CROUCH_ROW0: usize = 2 * FRAMES_PER_CLIP;
pub const PRONE_ROW0: usize = 3 * FRAMES_PER_CLIP;
pub const DEATH_ROW0: usize = 4 * FRAMES_PER_CLIP;
pub const TEX_WIDTH: usize = BONE_COUNT * 3;
pub const TEX_HEIGHT: usize = 5 * FRAMES_PER_CLIP;
/// Seconds the death fall takes (the presenter maps time since death to the
/// death clip's phase).
pub const DEATH_FALL_S: f32 = 1.1;
/// Metres walked per walk cycle (two steps).
pub const STRIDE_M: f32 = 1.5;

pub const PELVIS: usize = 0;
pub const CHEST: usize = 1;
pub const HEAD: usize = 2;
pub const L_UPPER_ARM: usize = 3;
pub const L_FOREARM: usize = 4;
pub const R_UPPER_ARM: usize = 5;
pub const R_FOREARM: usize = 6;
pub const L_THIGH: usize = 7;
pub const L_SHIN: usize = 8;
pub const R_THIGH: usize = 9;
pub const R_SHIN: usize = 10;

/// (parent, joint position in bind pose). Character faces −Z; its right is +X.
pub const BONES: [(Option<usize>, [f32; 3]); BONE_COUNT] = [
    (None, [0.0, 0.95, 0.0]),                // pelvis
    (Some(PELVIS), [0.0, 1.05, 0.0]),        // chest
    (Some(CHEST), [0.0, 1.50, 0.0]),         // head (neck joint)
    (Some(CHEST), [-0.22, 1.42, 0.0]),       // L upper arm (shoulder)
    (Some(L_UPPER_ARM), [-0.24, 1.14, 0.0]), // L forearm (elbow)
    (Some(CHEST), [0.22, 1.42, 0.0]),        // R upper arm
    (Some(R_UPPER_ARM), [0.24, 1.14, 0.0]),  // R forearm
    (Some(PELVIS), [-0.10, 0.92, 0.0]),      // L thigh (hip)
    (Some(L_THIGH), [-0.10, 0.50, 0.0]),     // L shin (knee)
    (Some(PELVIS), [0.10, 0.92, 0.0]),       // R thigh
    (Some(R_THIGH), [0.10, 0.50, 0.0]),      // R shin
];

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Clip {
    Walk,
    Idle,
    /// Kneeling, rifle ready.
    Crouch,
    /// Lying on the belly, arms forward.
    Prone,
    /// Falls backward; non-looping (phase 0 → 1).
    Death,
}

impl Clip {
    /// Every clip in texture row order.
    pub const ALL: [Clip; 5] = [
        Clip::Walk,
        Clip::Idle,
        Clip::Crouch,
        Clip::Prone,
        Clip::Death,
    ];

    pub fn row0(self) -> usize {
        match self {
            Clip::Walk => WALK_ROW0,
            Clip::Idle => IDLE_ROW0,
            Clip::Crouch => CROUCH_ROW0,
            Clip::Prone => PRONE_ROW0,
            Clip::Death => DEATH_ROW0,
        }
    }

    /// Index among the pose clips (idle 0, crouch 1, prone 2, death 3), the
    /// integer part of the instance's pose value. Walk has none.
    pub fn pose_index(self) -> Option<u8> {
        match self {
            Clip::Walk => None,
            Clip::Idle => Some(0),
            Clip::Crouch => Some(1),
            Clip::Prone => Some(2),
            Clip::Death => Some(3),
        }
    }

    pub fn loops(self) -> bool {
        self != Clip::Death
    }

    /// Phase of texture frame `f`: looping clips cover `[0, 1)`, death
    /// covers `[0, 1]` so its last frame is the final lying pose.
    pub fn frame_phase(self, f: usize) -> f32 {
        if self.loops() {
            f as f32 / FRAMES_PER_CLIP as f32
        } else {
            f as f32 / (FRAMES_PER_CLIP - 1) as f32
        }
    }
}

// ------------------------------------------------------------- mesh LODs --

/// Level of detail: 0 ≈ 1.2k triangles, 1 ≈ 300, 2 ≈ 100.
pub fn soldier_mesh(lod: u8) -> MeshData {
    let (sides, rings, head_sides, head_stacks) = match lod {
        0 => (10, 3, 10, 8),
        1 => (6, 1, 6, 4),
        _ => (4, 1, 4, 2),
    };
    let uniform = [0.95, 0.95, 0.95, TEAM_TINT];
    let skin_c = [0.78, 0.60, 0.48, FIXED];
    let helmet = [0.26, 0.28, 0.24, FIXED];
    let leather = [0.20, 0.15, 0.10, FIXED];
    let wood = [0.38, 0.24, 0.12, FIXED];
    let steel = [0.10, 0.10, 0.11, FIXED];

    let mut m = MeshData::default();
    let blend = |bone: usize, parent: usize| {
        move |t: f32| -> Skin {
            // Blend toward the parent over the first 25 % of the segment.
            let wp = if t < 0.25 {
                0.5 * (1.0 - t / 0.25)
            } else {
                0.0
            };
            [bone as f32, parent as f32, 1.0 - wp, wp]
        }
    };
    let j = |b: usize| BONES[b].1;

    // Pelvis and torso.
    m.add_box(
        [0.0, 0.95, 0.0],
        [0.17, 0.11, 0.10],
        uniform,
        Some(rigid(PELVIS)),
    );
    let chest = blend(CHEST, PELVIS);
    m.add_tube(
        [0.0, 1.02, 0.0],
        [0.0, 1.47, 0.0],
        0.17,
        0.20,
        0.6,
        sides,
        rings,
        uniform,
        Some(&chest),
    );
    // Head, helmet.
    if lod < 2 {
        m.add_tube(
            [0.0, 1.46, 0.0],
            [0.0, 1.55, 0.0],
            0.05,
            0.05,
            1.0,
            sides.min(6),
            1,
            skin_c,
            Some(&|_| rigid(HEAD)),
        );
    }
    m.add_ellipsoid(
        [0.0, 1.63, 0.0],
        [0.10, 0.12, 0.11],
        head_sides,
        head_stacks,
        skin_c,
        Some(rigid(HEAD)),
    );
    m.add_ellipsoid(
        [0.0, 1.69, 0.01],
        [0.135, 0.09, 0.145],
        head_sides,
        head_stacks.max(3) / 2 + 1,
        helmet,
        Some(rigid(HEAD)),
    );

    // Arms.
    for (upper, fore, side) in [
        (L_UPPER_ARM, L_FOREARM, -1.0f32),
        (R_UPPER_ARM, R_FOREARM, 1.0),
    ] {
        let ua = blend(upper, CHEST);
        m.add_tube(
            j(upper),
            j(fore),
            0.06,
            0.05,
            1.0,
            sides,
            rings,
            uniform,
            Some(&ua),
        );
        let fa = blend(fore, upper);
        let wrist = [0.25 * side, 0.88, 0.0];
        m.add_tube(
            j(fore),
            wrist,
            0.05,
            0.04,
            1.0,
            sides,
            rings,
            uniform,
            Some(&fa),
        );
        if lod == 0 {
            m.add_box(
                [0.25 * side, 0.84, 0.0],
                [0.035, 0.045, 0.04],
                skin_c,
                Some(rigid(fore)),
            );
        }
    }

    // Legs and boots.
    for (thigh, shin, side) in [(L_THIGH, L_SHIN, -1.0f32), (R_THIGH, R_SHIN, 1.0)] {
        let th = blend(thigh, PELVIS);
        m.add_tube(
            j(thigh),
            j(shin),
            0.085,
            0.065,
            1.0,
            sides,
            rings,
            uniform,
            Some(&th),
        );
        let sh = blend(shin, thigh);
        m.add_tube(
            j(shin),
            [0.10 * side, 0.09, 0.0],
            0.06,
            0.05,
            1.0,
            sides,
            rings,
            leather,
            Some(&sh),
        );
        m.add_box(
            [0.10 * side, 0.045, -0.04],
            [0.055, 0.045, 0.13],
            leather,
            Some(rigid(shin)),
        );
    }

    // Rifle, carried in the right forearm (held at port arms once the
    // forearm is raised by the poses below), and a pack.
    if lod < 2 {
        m.add_box(
            [0.25, 0.88, -0.30],
            [0.025, 0.04, 0.42],
            wood,
            Some(rigid(R_FOREARM)),
        );
        m.add_box(
            [0.25, 0.90, -0.78],
            [0.012, 0.012, 0.10],
            steel,
            Some(rigid(R_FOREARM)),
        );
        m.add_box(
            [0.0, 1.25, 0.17],
            [0.14, 0.15, 0.07],
            leather,
            Some(rigid(CHEST)),
        );
    } else {
        m.add_box(
            [0.25, 0.88, -0.35],
            [0.03, 0.04, 0.5],
            wood,
            Some(rigid(R_FOREARM)),
        );
    }
    m
}

// ---------------------------------------------------------- 3×4 matrices --

/// Affine 3×4 matrix, row-major: `rows[r] = [m_r0, m_r1, m_r2, t_r]`.
pub type Mat34 = [[f32; 4]; 3];

pub const IDENTITY: Mat34 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
];

pub fn mul(a: &Mat34, b: &Mat34) -> Mat34 {
    let mut o = [[0.0; 4]; 3];
    for r in 0..3 {
        for c in 0..4 {
            let mut v = a[r][0] * b[0][c] + a[r][1] * b[1][c] + a[r][2] * b[2][c];
            if c == 3 {
                v += a[r][3];
            }
            o[r][c] = v;
        }
    }
    o
}

pub fn translate(t: [f32; 3]) -> Mat34 {
    [
        [1.0, 0.0, 0.0, t[0]],
        [0.0, 1.0, 0.0, t[1]],
        [0.0, 0.0, 1.0, t[2]],
    ]
}

/// Rotation `Ry(y) · Rx(x) · Rz(z)` (radians).
pub fn rotation(x: f32, y: f32, z: f32) -> Mat34 {
    let (sx, cx) = x.sin_cos();
    let (sy, cy) = y.sin_cos();
    let (sz, cz) = z.sin_cos();
    let rx = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, cx, -sx, 0.0],
        [0.0, sx, cx, 0.0],
    ];
    let ry = [
        [cy, 0.0, sy, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [-sy, 0.0, cy, 0.0],
    ];
    let rz = [
        [cz, -sz, 0.0, 0.0],
        [sz, cz, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ];
    mul(&ry, &mul(&rx, &rz))
}

pub fn apply(m: &Mat34, p: [f32; 3]) -> [f32; 3] {
    let mut o = [0.0; 3];
    for (r, out) in o.iter_mut().enumerate() {
        *out = m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] + m[r][3];
    }
    o
}

// ------------------------------------------------------------- animation --

/// Local Euler rotations (x, y, z) per bone and a root offset for one frame.
/// Positive X rotation swings a hanging limb forward (toward −Z).
pub fn pose(clip: Clip, phase: f32) -> ([[f32; 3]; BONE_COUNT], [f32; 3]) {
    use std::f32::consts::TAU;
    let mut rot = [[0.0f32; 3]; BONE_COUNT];
    let w = TAU * phase;
    let (s, c) = w.sin_cos();
    let mut root = [0.0f32; 3];
    // Rifle carry, shared by both clips: right arm forward and forearm raised.
    rot[R_UPPER_ARM] = [0.35, 0.0, 0.15];
    rot[R_FOREARM] = [1.25, 0.0, -0.25];
    match clip {
        Clip::Walk => {
            rot[L_THIGH][0] = 0.5 * s;
            rot[R_THIGH][0] = -0.5 * s;
            rot[L_SHIN][0] = -0.8 * c.max(0.0);
            rot[R_SHIN][0] = -0.8 * (-c).max(0.0);
            rot[L_UPPER_ARM][0] = -0.45 * s;
            rot[L_FOREARM][0] = 0.25 + 0.15 * (-s).max(0.0);
            rot[R_UPPER_ARM][0] += 0.08 * s;
            rot[CHEST] = [0.08, 0.12 * s, 0.0];
            rot[PELVIS][1] = -0.08 * s;
            rot[HEAD][1] = -0.08 * s;
            root[1] = 0.035 * (2.0 * w).cos().abs() - 0.02;
        }
        Clip::Idle => {
            let b = s; // one breath per cycle
            rot[CHEST][0] = 0.03 * b;
            rot[HEAD] = [-0.02 * b, 0.25 * (0.5 * w).sin(), 0.0];
            rot[L_UPPER_ARM] = [0.02 * b, 0.0, -0.06];
            rot[L_FOREARM][0] = 0.1;
            rot[L_THIGH][2] = -0.04;
            rot[R_THIGH][2] = 0.04;
            root[1] = -0.005 + 0.004 * b;
        }
        Clip::Crouch => {
            // Pelvis down to about 0.52 m: left foot planted with the thigh
            // near horizontal, right knee on the ground and the shin back.
            let b = s;
            root[1] = -0.37 + 0.004 * b;
            rot[PELVIS][1] = 0.15;
            rot[CHEST] = [0.28 + 0.02 * b, -0.15, 0.0];
            rot[HEAD] = [-0.2, 0.1 + 0.15 * (0.5 * w).sin(), 0.0];
            rot[L_THIGH] = [1.45, 0.0, -0.06];
            rot[L_SHIN][0] = -1.4;
            rot[R_THIGH] = [0.25, 0.0, 0.06];
            rot[R_SHIN][0] = -1.75;
            rot[L_UPPER_ARM] = [0.55, 0.0, 0.25];
            rot[L_FOREARM][0] = 0.9;
            // Rifle level at low ready: forearm hanging in world space.
            rot[R_UPPER_ARM] = [0.9, 0.0, 0.15];
            rot[R_FOREARM] = [-1.18, 0.0, -0.1];
        }
        Clip::Prone => {
            // The whole body pitched forward onto the belly about the
            // pelvis, head toward −Z; arms reach forward to the rifle.
            let b = s;
            root = [0.0, -0.78 + 0.004 * b, -0.15];
            rot[PELVIS][0] = -std::f32::consts::FRAC_PI_2;
            rot[HEAD][0] = -1.0;
            rot[L_UPPER_ARM] = [2.55, 0.0, 0.35];
            rot[L_FOREARM][0] = 0.55;
            rot[R_UPPER_ARM] = [2.95, 0.0, -0.3];
            rot[R_FOREARM] = [1.8, 0.0, 0.0];
            rot[L_THIGH][2] = -0.12;
            rot[R_THIGH][2] = 0.12;
            rot[L_SHIN][0] = -0.05;
            rot[R_SHIN][0] = -0.05;
        }
        Clip::Death => {
            // Falls backward about the pelvis, knees buckling half way;
            // `phase` runs 0 → 1 once (ease-in, like a body dropping).
            let t = phase.clamp(0.0, 1.0);
            let fall = t * t;
            let buckle = (std::f32::consts::PI * t).sin();
            root = [0.0, -0.80 * t.powf(1.3), 0.25 * fall];
            rot[PELVIS][0] = std::f32::consts::FRAC_PI_2 * fall;
            rot[CHEST][0] = -0.15 * fall;
            rot[HEAD][0] = 0.4 * buckle - 0.2 * fall;
            rot[L_THIGH] = [0.7 * buckle, 0.0, -0.1 * t];
            rot[R_THIGH] = [0.5 * buckle, 0.0, 0.15 * t];
            rot[L_SHIN][0] = -buckle;
            rot[R_SHIN][0] = -0.8 * buckle;
            rot[L_UPPER_ARM] = [0.6 * buckle, 0.0, -1.1 * t];
            rot[L_FOREARM][0] = 0.4 * t;
            rot[R_UPPER_ARM] = [0.35 * (1.0 - t) + 0.5 * buckle, 0.0, 0.15 + 0.9 * t];
            // The rifle (fixed across the forearm) ends flat on the ground.
            rot[R_FOREARM] = [1.25 * (1.0 - t) - 1.42 * t, 0.0, -0.25 * (1.0 - t)];
        }
    }
    (rot, root)
}

/// Skinning matrices (`global pose × inverse bind`) for one frame.
pub fn skin_matrices(clip: Clip, phase: f32) -> [Mat34; BONE_COUNT] {
    let (rot, root) = pose(clip, phase);
    let mut global = [IDENTITY; BONE_COUNT];
    for b in 0..BONE_COUNT {
        let (parent, joint) = BONES[b];
        let r = rotation(rot[b][0], rot[b][1], rot[b][2]);
        let local = match parent {
            None => mul(&translate(crate::meshgen::add(joint, root)), &r),
            Some(p) => mul(&translate(crate::meshgen::sub(joint, BONES[p].1)), &r),
        };
        // Parents come before children in BONES.
        global[b] = match parent {
            None => local,
            Some(p) => mul(&global[p], &local),
        };
    }
    let mut out = [IDENTITY; BONE_COUNT];
    for b in 0..BONE_COUNT {
        let j = BONES[b].1;
        out[b] = mul(&global[b], &translate([-j[0], -j[1], -j[2]]));
    }
    out
}

/// Bakes every clip into RGBA32F texel data (`TEX_WIDTH × TEX_HEIGHT × 4`).
pub fn bake_animation_texture() -> Vec<f32> {
    let mut data = vec![0.0f32; TEX_WIDTH * TEX_HEIGHT * 4];
    for clip in Clip::ALL {
        for f in 0..FRAMES_PER_CLIP {
            let mats = skin_matrices(clip, clip.frame_phase(f));
            let row = clip.row0() + f;
            for (b, m) in mats.iter().enumerate() {
                for (r, mrow) in m.iter().enumerate() {
                    let i = (row * TEX_WIDTH + b * 3 + r) * 4;
                    data[i..i + 4].copy_from_slice(mrow);
                }
            }
        }
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meshgen::winding_ok_fraction;

    #[test]
    fn parents_precede_children() {
        for (b, (p, _)) in BONES.iter().enumerate() {
            if let Some(p) = p {
                assert!(*p < b);
            }
        }
    }

    #[test]
    fn lod_triangle_budgets() {
        let t: Vec<usize> = (0..3).map(|l| soldier_mesh(l).tri_count()).collect();
        assert!((1000..=2000).contains(&t[0]), "LOD0 has {} tris", t[0]);
        assert!(t[1] < t[0] / 2 && t[2] < t[1], "LOD tris {t:?}");
    }

    #[test]
    fn skin_data_is_valid() {
        for lod in 0..3 {
            let m = soldier_mesh(lod);
            assert_eq!(m.custom0.len(), m.positions.len());
            for s in &m.custom0 {
                assert!((s[0] as usize) < BONE_COUNT && (s[1] as usize) < BONE_COUNT);
                assert!((s[2] + s[3] - 1.0).abs() < 1e-5);
            }
            assert!(winding_ok_fraction(&m) > 0.97, "lod {lod}");
        }
    }

    #[test]
    fn rest_pose_is_identity() {
        // With zero rotations every skinning matrix must be the identity.
        let mut global = [IDENTITY; BONE_COUNT];
        for b in 0..BONE_COUNT {
            let (parent, joint) = BONES[b];
            let local = match parent {
                None => translate(joint),
                Some(p) => translate(crate::meshgen::sub(joint, BONES[p].1)),
            };
            global[b] = match parent {
                None => local,
                Some(p) => mul(&global[p], &local),
            };
            let j = joint;
            let s = mul(&global[b], &translate([-j[0], -j[1], -j[2]]));
            for r in 0..3 {
                for c in 0..4 {
                    assert!((s[r][c] - IDENTITY[r][c]).abs() < 1e-5);
                }
            }
        }
    }

    #[test]
    fn walk_moves_feet_and_keeps_joints_attached() {
        let a = skin_matrices(Clip::Walk, 0.25);
        let b = skin_matrices(Clip::Walk, 0.75);
        let foot = [-0.10, 0.05, 0.0];
        let fa = apply(&a[L_SHIN], foot);
        let fb = apply(&b[L_SHIN], foot);
        assert!(
            (fa[2] - fb[2]).abs() > 0.3,
            "left foot should swing: {fa:?} {fb:?}"
        );
        // The knee transformed by thigh and by shin must coincide.
        let knee = BONES[L_SHIN].1;
        let k1 = apply(&a[L_THIGH], knee);
        let k2 = apply(&a[L_SHIN], knee);
        for i in 0..3 {
            assert!((k1[i] - k2[i]).abs() < 1e-5);
        }
    }

    /// Lowest point of the posed LOD1 mesh and its extent along Z.
    fn posed_bounds(clip: Clip, phase: f32) -> (f32, f32, f32, f32) {
        let mats = skin_matrices(clip, phase);
        let m = soldier_mesh(1);
        let (mut lo, mut hi, mut zlo, mut zhi) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for (p, s) in m.positions.iter().zip(&m.custom0) {
            let a = apply(&mats[s[0] as usize], *p);
            let b = apply(&mats[s[1] as usize], *p);
            let q = [
                a[0] * s[2] + b[0] * s[3],
                a[1] * s[2] + b[1] * s[3],
                a[2] * s[2] + b[2] * s[3],
            ];
            lo = lo.min(q[1]);
            hi = hi.max(q[1]);
            zlo = zlo.min(q[2]);
            zhi = zhi.max(q[2]);
        }
        (lo, hi, zlo, zhi)
    }

    #[test]
    fn stance_poses_have_plausible_heights() {
        let (lo, stand, ..) = posed_bounds(Clip::Idle, 0.0);
        assert!(lo.abs() < 0.08 && stand > 1.65, "stand {lo} {stand}");
        let (lo, crouch, ..) = posed_bounds(Clip::Crouch, 0.0);
        assert!(lo > -0.12 && lo < 0.1, "crouch feet {lo}");
        assert!((1.0..1.4).contains(&crouch), "crouch top {crouch}");
        let (lo, prone, zlo, zhi) = posed_bounds(Clip::Prone, 0.0);
        assert!(lo > -0.15 && prone < 0.6, "prone {lo}..{prone}");
        assert!(zhi - zlo > 1.5, "prone body lies along Z: {zlo}..{zhi}");
        let (_, dead, ..) = posed_bounds(Clip::Death, 1.0);
        assert!(dead < 0.6, "corpse lies flat: top {dead}");
        let (_, start, ..) = posed_bounds(Clip::Death, 0.0);
        assert!(start > 1.6, "death starts standing: {start}");
    }

    #[test]
    fn prone_head_is_forward() {
        // The face (head bone) ends up ahead of the pelvis, toward −Z.
        let m = skin_matrices(Clip::Prone, 0.0);
        let head = apply(&m[HEAD], [0.0, 1.63, 0.0]);
        let hip = apply(&m[PELVIS], BONES[PELVIS].1);
        assert!(head[2] < hip[2] - 0.5, "head {head:?} hip {hip:?}");
    }

    #[test]
    fn texture_layout() {
        let d = bake_animation_texture();
        assert_eq!(d.len(), TEX_WIDTH * TEX_HEIGHT * 4);
        assert!(d.iter().all(|v| v.is_finite()));
        // Row 0 of bone 0 at frame 0 = first texel.
        let m = skin_matrices(Clip::Walk, 0.0);
        assert_eq!(&d[0..4], &m[0][0]);
    }
}
