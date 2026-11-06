//! Armour and penetration (S3, `docs/slices/S3-armour.md` "Ballistics and
//! penetration"): the one ballistics function.
//!
//! Pure functions plus [`ArmourTables`], derived from the (hashed)
//! [`MatchSetup`] and never saved. The sim, `frontline-cli ballistics` and
//! the golden tests all call these same functions; there is no second copy
//! of the math.
//!
//! **Plates.** A vehicle is 1..=4 armour boxes (S7; S3 had a hull and a
//! turret box). Each box face is a plate of its zone ([`Face::group`]: zone
//! id = box × 4 + group; S7 open faces have no plate); the face is
//! geometrically vertical (or flat), but its **ballistic normal** is tilted
//! by the zone's `slope_deg`: a front plate sloped `s` back from vertical
//! has the outward normal `(cos s, 0, sin s)` in the box frame (x forward,
//! y left, z up). Bottom faces use the top zone's thickness.
//!
//! **Penetration rule** (fixed point, no division in the decision):
//!
//! ```text
//! pen0(range) = interp(penetration, range) / cos(pen_reference_angle)
//! c           = −(dir · n)                  impact cosine
//! c ≤ cos(ricochet_angle)                   → Ricochet (also c ≤ 0)
//! v           = triangular(−7 %, +7 %)      sum of two counter-RNG uniforms
//! pen0 · (1 + v) · c ≥ thickness + extra · c → Penetration
//! else                                       → NonPenetration
//! ```
//!
//! `extra` is the running gear's `track_mm` for hits in a hull side's track
//! band. For a flat shot at a face with slope `s` and horizontal angle `a`
//! from the face normal, `c = cos s · cos a`; a descending shell at a
//! back-sloped plate gets `cos(s − d) · …` ([`shot_dir`]). Trigonometry is
//! `Fx::sin_cos` and `Fx::atan2` only; angles in the setup are integer
//! degrees ([`deg`]).

use frontline_content::vehicle::{AmmoSetup, VehicleSetup};
use frontline_content::{AmmoMechanism, MAX_ARMOUR_BOXES, PlateGroup};

use crate::fx::{Fx, FxVec3};
use crate::rng::RngStream;
use crate::vehicle::{BoxFrame, VehicleFrame};

/// Half the width of each of the two uniforms whose sum is the ±7 %
/// penetration variation (raw Q16.16; 2294 / 65536 = 0.035).
const HALF_BAND_RAW: i32 = 2294;
/// Half-width of the triangular variation: ±7 % (0.070007).
pub const PEN_BAND: Fx = Fx::from_raw(2 * HALF_BAND_RAW);
/// Probabilities from [`pen_chance_ppm`] are in parts per million.
pub const PPM: u32 = 1_000_000;

/// 9.81 m/s² (as `combat`).
const GRAVITY: Fx = Fx::from_ratio(981, 100);

/// Integer degrees to radians.
#[inline]
pub fn deg(d: i32) -> Fx {
    Fx::from_raw((d as i64 * Fx::PI.raw() as i64 / 180) as i32)
}

/// Which box of a vehicle: an armour box by index (S7: 0 the running-gear
/// hull, up to `MAX_ARMOUR_BOXES`), or the main gun's exposed barrel.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoxPart(pub u8);

impl BoxPart {
    /// Box 0, the running-gear hull.
    pub const HULL: BoxPart = BoxPart(0);
    /// S3b: the main gun's exposed barrel (no plate; a strike damages the
    /// gun, `crate::damage`).
    pub const BARREL: BoxPart = BoxPart(u8::MAX);

    /// Its armour box index (`None`: the barrel).
    #[inline]
    pub const fn armour_box(self) -> Option<usize> {
        if self.0 == u8::MAX {
            None
        } else {
            Some(self.0 as usize)
        }
    }
}

/// A face of a box, by its outward axis in the box frame.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Face {
    /// +x.
    Front = 0,
    /// −x.
    Rear = 1,
    /// +y.
    Left = 2,
    /// −y.
    Right = 3,
    /// +z.
    Top = 4,
    /// −z (uses the top zone).
    Bottom = 5,
}

impl Face {
    pub const ALL: [Face; 6] = [
        Face::Front,
        Face::Rear,
        Face::Left,
        Face::Right,
        Face::Top,
        Face::Bottom,
    ];

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The plate group of this face (both sides `Side`; the bottom reads
    /// `Top`).
    pub const fn group(self) -> PlateGroup {
        match self {
            Face::Front => PlateGroup::Front,
            Face::Rear => PlateGroup::Rear,
            Face::Left | Face::Right => PlateGroup::Side,
            Face::Top | Face::Bottom => PlateGroup::Top,
        }
    }

    /// The zone id of this face of `part` (S7: box × 4 + group; the barrel
    /// reads box 1's front, nominal: a barrel strike never runs the plate
    /// rule).
    pub const fn zone(self, part: BoxPart) -> u8 {
        match part.armour_box() {
            Some(b) => self.group().zone(b),
            None => PlateGroup::Front.zone(1),
        }
    }

    /// The box and face a zone names (sides: the left face; tops: the top
    /// face). For tools that start from a zone (`ballistics`).
    pub const fn of_zone(z: u8) -> (BoxPart, Face) {
        let face = match PlateGroup::of_zone(z) {
            PlateGroup::Front => Face::Front,
            PlateGroup::Side => Face::Left,
            PlateGroup::Rear => Face::Rear,
            PlateGroup::Top => Face::Top,
        };
        (BoxPart(z / 4), face)
    }

    /// The face's untilted outward horizontal direction (x, y); zero for
    /// the top and bottom.
    const fn horizontal(self) -> (i32, i32) {
        match self {
            Face::Front => (1, 0),
            Face::Rear => (-1, 0),
            Face::Left => (0, 1),
            Face::Right => (0, -1),
            Face::Top | Face::Bottom => (0, 0),
        }
    }
}

/// The outward ballistic normal of `face` in its box frame, tilted up by
/// `slope_deg` (unit length up to fixed-point rounding). Tops and bottoms
/// are flat.
pub fn plate_normal(face: Face, slope_deg: i32) -> FxVec3 {
    match face {
        Face::Top => FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE),
        Face::Bottom => FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::NEG_ONE),
        _ => {
            let (s, c) = deg(slope_deg).sin_cos();
            let (hx, hy) = face.horizontal();
            FxVec3::new(c.mul_int(hx), c.mul_int(hy), s)
        }
    }
}

/// Impact cosine of a round travelling along the unit vector `dir` onto a
/// plate with outward unit normal `n`: `−(dir · n)`. At most 0 when the
/// round meets the plate from behind its tilted normal.
#[inline]
pub fn impact_cos(dir: FxVec3, n: FxVec3) -> Fx {
    -dir.dot(n)
}

/// Direction of a round arriving at `face` (in the box frame) at
/// horizontal angle `angle` from the face's normal (positive turns the
/// arrival toward +z rotation, i.e. counter-clockwise seen from above),
/// descending at `descent` below the horizontal. For tops and bottoms the
/// horizontal part comes from the front. Combine with [`plate_normal`] and
/// [`impact_cos`]: a flat shot gives `c = cos s · cos a`.
pub fn shot_dir(face: Face, angle: Fx, descent: Fx) -> FxVec3 {
    let (hx, hy) = match face.horizontal() {
        (0, 0) => (1, 0),
        h => h,
    };
    let (sa, ca) = angle.sin_cos();
    let (sd, cd) = descent.sin_cos();
    // The face's outward horizontal normal turned by `angle`; the round
    // travels against it, and down by `descent`.
    let (hx, hy) = (Fx::from_int(hx), Fx::from_int(hy));
    let ox = hx.mul(ca) - hy.mul(sa);
    let oy = hx.mul(sa) + hy.mul(ca);
    FxVec3::new(-ox.mul(cd), -oy.mul(cd), -sd)
}

/// Descent angle at impact of a round aimed at a point at `range` on the
/// muzzle's level, at muzzle speed `speed` (no drag): the launch angle of
/// `combat`'s drop compensation, `atan(g·t / 2v)` with `t = range / v`.
/// `ballistics --descent auto` uses it.
pub fn aimed_descent(speed: Fx, range: Fx) -> Fx {
    if speed <= Fx::ZERO {
        return Fx::ZERO;
    }
    let t = range.div(speed);
    Fx::atan2(GRAVITY.mul(t).div_int(2), speed)
}

/// What a round does at a plate.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum PenResult {
    /// Too oblique: the round is spent (an event only).
    Ricochet = 0,
    NonPenetration = 1,
    Penetration = 2,
}

/// The penetration rule (see the module docs). `pen0` is the normalised
/// penetration at the impact range (mm), `c` the impact cosine,
/// `thickness` and `extra` in mm, `v` the variation from [`variation`]. No
/// division.
pub fn penetrates(pen0: Fx, c: Fx, thickness: Fx, extra: Fx, cos_ricochet: Fx, v: Fx) -> PenResult {
    if c <= cos_ricochet || c <= Fx::ZERO {
        return PenResult::Ricochet;
    }
    let pen = pen0.mul(Fx::ONE + v);
    if pen.mul(c) >= thickness + extra.mul(c) {
        PenResult::Penetration
    } else {
        PenResult::NonPenetration
    }
}

/// The ±7 % triangular variation: the sum of two uniforms, two draws from
/// `r` (keyed by the caller: `PENETRATION` with tick and slot).
pub fn variation(r: &mut RngStream) -> Fx {
    let h = Fx::from_raw(HALF_BAND_RAW);
    r.range_fx(-h, h) + r.range_fx(-h, h)
}

/// Thickness a round meets along its path, for people: `thickness / c +
/// extra`. Saturates as `c` nears 0.
pub fn effective_thickness(c: Fx, thickness: Fx, extra: Fx) -> Fx {
    if c <= Fx::ZERO {
        return Fx::MAX;
    }
    thickness.div(c).saturating_add(extra)
}

/// Exact chance (ppm) that [`penetrates`] gives `Penetration` with a
/// continuous ±[`PEN_BAND`] triangular variation, in closed form (no
/// sampling). 0 when the impact ricochets.
pub fn pen_chance_ppm(pen0: Fx, c: Fx, thickness: Fx, extra: Fx, cos_ricochet: Fx) -> u32 {
    if c <= cos_ricochet || c <= Fx::ZERO || pen0 <= Fx::ZERO {
        return 0;
    }
    // Penetration when v ≥ x = need / (pen0·c) − 1, in raw Q16.16 (i64 so
    // a very thin pen or oblique hit cannot overflow).
    let need = thickness.raw() as i64 * 65_536 + extra.raw() as i64 * c.raw() as i64;
    let have = pen0.raw() as i64 * c.raw() as i64; // Q32.32
    let x = (need * 65_536 / have.max(1)) - 65_536; // Q16.16, i64
    let a = PEN_BAND.raw() as i64;
    let ppm = PPM as i64;
    if x <= -a {
        PPM
    } else if x >= a {
        0
    } else if x <= 0 {
        // 1 − (x + a)² / 2a²
        (ppm - (x + a) * (x + a) * ppm / (2 * a * a)) as u32
    } else {
        ((a - x) * (a - x) * ppm / (2 * a * a)) as u32
    }
}

/// One round's derived numbers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AmmoTable {
    pub mechanism: Option<AmmoMechanism>,
    /// `cos(ricochet_angle)`: impacts with `c` at or below it ricochet.
    pub cos_ricochet: Fx,
    /// `cos(pen_reference_angle)`.
    pub cos_ref: Fx,
    /// `(range m, pen0 mm)` ascending, already divided by `cos_ref`.
    pub rows: Vec<(Fx, Fx)>,
}

impl AmmoTable {
    pub fn new(a: &AmmoSetup) -> AmmoTable {
        let cos_ref = deg(a.pen_ref_deg).cos();
        AmmoTable {
            mechanism: Some(a.mechanism),
            cos_ricochet: deg(a.ricochet_deg).cos(),
            cos_ref,
            rows: a
                .penetration
                .iter()
                .map(|r| (Fx::from(r.range), Fx::from_int(r.mm as i32).div(cos_ref)))
                .collect(),
        }
    }

    /// Normalised penetration (mm at 0°) at `range` metres: piecewise
    /// linear in i64, clamped to the table's ends.
    pub fn pen0(&self, range: Fx) -> Fx {
        let Some(&(r_first, p_first)) = self.rows.first() else {
            return Fx::ZERO;
        };
        if range <= r_first {
            return p_first;
        }
        for w in self.rows.windows(2) {
            let ((r0, p0), (r1, p1)) = (w[0], w[1]);
            if range <= r1 {
                let num =
                    (p1.raw() as i64 - p0.raw() as i64) * (range.raw() as i64 - r0.raw() as i64);
                let den = (r1.raw() as i64 - r0.raw() as i64).max(1);
                return Fx::from_raw((p0.raw() as i64 + num / den) as i32);
            }
        }
        self.rows[self.rows.len() - 1].1
    }
}

/// One plate face of a vehicle type.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct PlateFace {
    pub zone: Option<u8>,
    /// Outward ballistic normal in the box frame.
    pub normal: FxVec3,
    /// mm (0 for an open face).
    pub thickness: Fx,
    /// S7: an open face (no plate).
    pub open: bool,
}

/// A vehicle type's plates: `faces[box][face]` (S7: up to
/// `MAX_ARMOUR_BOXES` boxes, `boxes` in use).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VehicleArmour {
    pub faces: [[PlateFace; 6]; MAX_ARMOUR_BOXES],
    /// Armour boxes in use (1..=4).
    pub boxes: u8,
    /// Extra mm in a hull side's track band, and the band's height in
    /// percent of the side.
    pub track_mm: Fx,
    pub track_band_pct: u32,
}

impl VehicleArmour {
    pub fn new(v: &VehicleSetup) -> VehicleArmour {
        let mut faces = [[PlateFace::default(); 6]; MAX_ARMOUR_BOXES];
        for (b, bx) in v.boxes.iter().enumerate().take(MAX_ARMOUR_BOXES) {
            for f in Face::ALL {
                let g = f.group();
                let plate = &bx.plates[g.index()];
                faces[b][f.index()] = PlateFace {
                    zone: Some(g.zone(b)),
                    normal: plate_normal(f, plate.slope_deg),
                    thickness: plate.thickness.into(),
                    open: bx.open & (1 << g.index()) != 0,
                };
            }
        }
        VehicleArmour {
            faces,
            boxes: v.boxes.len().clamp(1, MAX_ARMOUR_BOXES) as u8,
            track_mm: Fx::from_int(v.track_mm as i32),
            track_band_pct: v.track_band_pct,
        }
    }

    /// The plate of `part`'s face `f` (the barrel, which has none, reads
    /// the last box's; callers never run the plate rule on it).
    #[inline]
    pub fn face(&self, part: BoxPart, f: Face) -> &PlateFace {
        let last = self.boxes.max(1) as usize - 1;
        &self.faces[(part.0 as usize).min(last)][f.index()]
    }
}

/// Derived armour tables for a match: per ammo index and per vehicle index
/// of the setup. Built in `MatchState::new` and after load; never saved.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArmourTables {
    pub ammo: Vec<AmmoTable>,
    pub vehicles: Vec<VehicleArmour>,
}

impl ArmourTables {
    pub fn new(ammo: &[AmmoSetup], vehicles: &[VehicleSetup]) -> ArmourTables {
        ArmourTables {
            ammo: ammo.iter().map(AmmoTable::new).collect(),
            vehicles: vehicles.iter().map(VehicleArmour::new).collect(),
        }
    }
}

/// A round meeting a vehicle's plate (the narrowphase result,
/// [`hit_armour`]; M2's mesh model returns the same).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ArmourHit {
    /// Fraction of the substep segment at the hit.
    pub t: Fx,
    pub part: BoxPart,
    pub face: Face,
    /// Zone id (box × 4 + group).
    pub zone: u8,
    /// Outward ballistic normal in world space.
    pub normal: FxVec3,
    pub point: FxVec3,
    /// The hit point in the box's own frame (from the box centre): the
    /// track band reads its height.
    pub local: FxVec3,
}

// ---------------------------------------------------------- narrowphase --

/// Slab test of a segment against an axis-aligned box: `p` is the start
/// relative to the box centre and `q` the segment, both in the box's
/// axes; `half` the half extents. Returns `(t, face, entry point)` with
/// `t` the fraction of the segment at entry (0 when it starts inside). The
/// face is the last slab entered; on an edge or corner (equal entry `t`)
/// the lower axis wins: x, then y, then z.
pub fn slab(p: FxVec3, q: FxVec3, half: FxVec3) -> Option<(Fx, Face, FxVec3)> {
    let (mut near, mut far) = (Fx::MIN, Fx::MAX);
    let mut face = Face::Front;
    let axes = [
        (p.x, q.x, half.x, Face::Front, Face::Rear),
        (p.y, q.y, half.y, Face::Left, Face::Right),
        (p.z, q.z, half.z, Face::Top, Face::Bottom),
    ];
    for (pk, qk, hk, pos_face, neg_face) in axes {
        if qk.is_zero() {
            if pk.abs() > hk {
                return None;
            }
            continue;
        }
        let t1 = (-hk - pk).div(qk);
        let t2 = (hk - pk).div(qk);
        // Moving toward +k enters through the −k face.
        let (tn, tf, f) = if qk > Fx::ZERO {
            (t1, t2, neg_face)
        } else {
            (t2, t1, pos_face)
        };
        if tn > near {
            near = tn;
            face = f;
        }
        far = far.min(tf);
    }
    if near > far || far < Fx::ZERO || near > Fx::ONE {
        return None;
    }
    let t = near.max(Fx::ZERO);
    Some((t, face, p + q * t))
}

/// A world-space box's local frame: `v` (relative to the box centre, so
/// products stay small) in its axes.
#[inline]
pub fn to_local(b: &BoxFrame, v: FxVec3) -> FxVec3 {
    FxVec3::new(b.axes.x.dot(v), b.axes.y.dot(v), b.axes.z.dot(v))
}

/// Segment `from → from + d` against the oriented box `b`: `(t, face,
/// local entry point)`.
pub fn segment_box(b: &BoxFrame, from: FxVec3, d: FxVec3) -> Option<(Fx, Face, FxVec3)> {
    slab(to_local(b, from - b.center), to_local(b, d), b.half)
}

/// Squared distance (raw Q32.32) from `p` to the oriented box `b` (0
/// inside).
pub fn box_dist_sq_raw(b: &BoxFrame, p: FxVec3) -> u64 {
    let l = to_local(b, p - b.center);
    let ex = |v: Fx, h: Fx| (v.abs() - h).max(Fx::ZERO);
    FxVec3::new(ex(l.x, b.half.x), ex(l.y, b.half.y), ex(l.z, b.half.z)).length_sq_raw()
}

/// Whether the segment `from → from + d` passes within `r` of `c` (the
/// broadphase bounding-sphere test). Inputs near each other (a vehicle's
/// reach), so the products stay small.
pub fn segment_near(from: FxVec3, d: FxVec3, c: FxVec3, r: Fx) -> bool {
    let rel = c - from;
    let len_sq = d.dot_raw(d);
    let s = if len_sq > 0 {
        (rel.dot_raw(d).clamp(0, len_sq) as i128 * 65_536 / len_sq as i128) as i32
    } else {
        0
    };
    let gap = rel - d * Fx::from_raw(s);
    let rr = r.raw().max(0) as u64;
    gap.length_sq_raw() <= rr * rr
}

/// The narrowphase (`docs/slices/S3-armour.md` "Geometry and pose"): the
/// first plate of `f`'s armour boxes (S7: in box order; S3's hull, then
/// turret) or (S3b) its barrel box that the segment `from → from + d`
/// enters, with its ballistic normal in world space. At equal `t` the lower
/// box index wins, then the barrel. M2 replaces the body with the mesh BVH;
/// callers do not change.
pub fn hit_armour(
    from: FxVec3,
    d: FxVec3,
    f: &VehicleFrame,
    a: &VehicleArmour,
) -> Option<ArmourHit> {
    let mut best: Option<ArmourHit> = None;
    let n = (f.n_boxes as usize).min(MAX_ARMOUR_BOXES);
    let parts = f.boxes[..n]
        .iter()
        .enumerate()
        .map(|(k, b)| (BoxPart(k as u8), b))
        .chain(std::iter::once((BoxPart::BARREL, &f.barrel)));
    for (part, b) in parts {
        if b.half.x <= Fx::ZERO {
            continue;
        }
        let Some((t, face, local)) = segment_box(b, from, d) else {
            continue;
        };
        if best.is_some_and(|h| h.t <= t) {
            continue;
        }
        let plate = a.face(part, face);
        best = Some(ArmourHit {
            t,
            part,
            face,
            zone: face.zone(part),
            normal: b.axes.apply(plate.normal),
            point: from + d * t,
            local,
        });
    }
    best
}

/// Whether a hull-side hit at box-frame `local` is in the running-gear
/// band: the lower `band_pct` of the side.
pub fn in_track_band(hit: &ArmourHit, half_z: Fx, band_pct: u32) -> bool {
    hit.part == BoxPart::HULL
        && matches!(hit.face, Face::Left | Face::Right)
        && hit.local.z + half_z < half_z.mul_int(2).mul_int(band_pct as i32).div_int(100)
}

/// Everything about one shot at one plate (the `ballistics` CLI's
/// one-shot output, and the golden tests).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlateShot {
    pub pen0: Fx,
    pub c: Fx,
    pub thickness: Fx,
    pub extra: Fx,
    pub effective: Fx,
    pub ricochet: bool,
    pub chance_ppm: u32,
    /// The rule at both ends of the band (`v = ∓PEN_BAND`).
    pub at_low: PenResult,
    pub at_high: PenResult,
}

/// A shot from `ammo` at `zone` of `vehicle` at `range` metres, arriving at
/// horizontal `angle` and `descent` (radians), with the track band's extra
/// armour if `track_band` (hull sides only).
pub fn plate_shot(
    ammo: &AmmoTable,
    vehicle: &VehicleArmour,
    zone: u8,
    range: Fx,
    angle: Fx,
    descent: Fx,
    track_band: bool,
) -> PlateShot {
    let (part, face) = Face::of_zone(zone);
    let plate = vehicle.face(part, face);
    let c = impact_cos(shot_dir(face, angle, descent), plate.normal);
    let extra = if track_band && zone == PlateGroup::Side.zone(0) {
        vehicle.track_mm
    } else {
        Fx::ZERO
    };
    let pen0 = ammo.pen0(range);
    let rule = |v| penetrates(pen0, c, plate.thickness, extra, ammo.cos_ricochet, v);
    PlateShot {
        pen0,
        c,
        thickness: plate.thickness,
        extra,
        effective: effective_thickness(c, plate.thickness, extra),
        ricochet: c <= ammo.cos_ricochet || c <= Fx::ZERO,
        chance_ppm: pen_chance_ppm(pen0, c, plate.thickness, extra, ammo.cos_ricochet),
        at_low: rule(-PEN_BAND),
        at_high: rule(PEN_BAND),
    }
}

#[cfg(test)]
#[allow(clippy::float_arithmetic)]
mod tests {
    use super::*;
    use crate::ecs::SimId;
    use crate::rng::Purpose;

    fn close(a: Fx, b: f64, tol: f64) -> bool {
        (a.to_f32() as f64 - b).abs() <= tol
    }

    #[test]
    fn plate_normals_are_tilted_unit_vectors() {
        let n = plate_normal(Face::Front, 47);
        assert!(close(n.x, 47f64.to_radians().cos(), 1e-4), "{n:?}");
        assert!(n.y.is_zero());
        assert!(close(n.z, 47f64.to_radians().sin(), 1e-4));
        let r = plate_normal(Face::Rear, 10);
        assert!(r.x < Fx::ZERO && r.z > Fx::ZERO);
        let l = plate_normal(Face::Left, 25);
        let rt = plate_normal(Face::Right, 25);
        assert!(l.y > Fx::ZERO && rt.y < Fx::ZERO && l.z == rt.z);
        assert_eq!(plate_normal(Face::Top, 0).z, Fx::ONE);
        assert_eq!(plate_normal(Face::Bottom, 0).z, Fx::NEG_ONE);
        for f in Face::ALL {
            for s in [0, 10, 30, 47, 80] {
                let len = plate_normal(f, s).length();
                assert!(close(len, 1.0, 2e-4), "{f:?} {s}: {len:?}");
            }
        }
    }

    #[test]
    fn compound_angle_is_cos_slope_times_cos_angle() {
        for (slope, angle) in [(0, 0), (47, 0), (47, 30), (10, 20), (30, 60), (25, 45)] {
            for face in [Face::Front, Face::Rear, Face::Left, Face::Right] {
                let c = impact_cos(
                    shot_dir(face, deg(angle), Fx::ZERO),
                    plate_normal(face, slope),
                );
                let want = (slope as f64).to_radians().cos() * (angle as f64).to_radians().cos();
                assert!(
                    close(c, want, 3e-4),
                    "{face:?} {slope} {angle}: {c:?} vs {want}"
                );
            }
        }
        // A descending round at a back-sloped plate: cos(s − d).
        let c = impact_cos(
            shot_dir(Face::Front, Fx::ZERO, deg(5)),
            plate_normal(Face::Front, 47),
        );
        assert!(close(c, 42f64.to_radians().cos(), 3e-4), "{c:?}");
        // Straight down onto a top plate: c = 1; a flat shot: c = 0.
        let top = plate_normal(Face::Top, 0);
        assert!(close(
            impact_cos(shot_dir(Face::Top, Fx::ZERO, deg(90)), top),
            1.0,
            1e-4
        ));
        assert!(close(
            impact_cos(shot_dir(Face::Top, Fx::ZERO, Fx::ZERO), top),
            0.0,
            1e-4
        ));
    }

    #[test]
    fn interpolation_between_rows_and_clamped_at_the_ends() {
        let t = AmmoTable {
            mechanism: None,
            cos_ricochet: Fx::ZERO,
            cos_ref: Fx::ONE,
            rows: vec![
                (Fx::from_int(100), Fx::from_int(106)),
                (Fx::from_int(500), Fx::from_int(96)),
                (Fx::from_int(1000), Fx::from_int(85)),
            ],
        };
        assert_eq!(t.pen0(Fx::ZERO), Fx::from_int(106));
        assert_eq!(t.pen0(Fx::from_int(100)), Fx::from_int(106));
        assert_eq!(t.pen0(Fx::from_int(300)), Fx::from_int(101));
        assert_eq!(t.pen0(Fx::from_int(500)), Fx::from_int(96));
        assert!(close(t.pen0(Fx::from_int(750)), 90.5, 1e-4));
        assert_eq!(t.pen0(Fx::from_int(1000)), Fx::from_int(85));
        assert_eq!(t.pen0(Fx::from_int(5000)), Fx::from_int(85));
        let flat = AmmoTable {
            rows: vec![(Fx::ZERO, Fx::from_int(160))],
            ..t.clone()
        };
        assert_eq!(flat.pen0(Fx::from_int(77)), Fx::from_int(160));
        assert_eq!(AmmoTable::default().pen0(Fx::ONE), Fx::ZERO);
    }

    #[test]
    fn penetrates_at_the_edges() {
        let cos70 = deg(70).cos();
        let pen0 = Fx::from_int(100);
        let t = Fx::from_int(80);
        // Exactly at the ricochet cosine, and from behind the plate.
        assert_eq!(
            penetrates(pen0, cos70, t, Fx::ZERO, cos70, Fx::ZERO),
            PenResult::Ricochet
        );
        assert_eq!(
            penetrates(
                pen0,
                cos70 + Fx::EPSILON,
                Fx::ONE,
                Fx::ZERO,
                cos70,
                Fx::ZERO
            ),
            PenResult::Penetration
        );
        assert_eq!(
            penetrates(pen0, -Fx::HALF, t, Fx::ZERO, cos70, Fx::ZERO),
            PenResult::Ricochet
        );
        assert_eq!(
            penetrates(pen0, Fx::ZERO, t, Fx::ZERO, -Fx::ONE, Fx::ZERO),
            PenResult::Ricochet
        );
        // pen · c ≥ thickness: equality penetrates.
        assert_eq!(
            penetrates(pen0, Fx::ONE, Fx::from_int(100), Fx::ZERO, cos70, Fx::ZERO),
            PenResult::Penetration
        );
        assert_eq!(
            penetrates(
                pen0,
                Fx::ONE,
                Fx::from_int(100) + Fx::EPSILON,
                Fx::ZERO,
                cos70,
                Fx::ZERO
            ),
            PenResult::NonPenetration
        );
        // The band: 100 mm ± 7 % against 95 mm and 105 mm.
        let low = -PEN_BAND;
        let high = PEN_BAND;
        assert_eq!(
            penetrates(pen0, Fx::ONE, Fx::from_int(95), Fx::ZERO, cos70, low),
            PenResult::NonPenetration
        );
        assert_eq!(
            penetrates(pen0, Fx::ONE, Fx::from_int(105), Fx::ZERO, cos70, high),
            PenResult::Penetration
        );
        // Track band: the extra armour counts along the path.
        assert_eq!(
            penetrates(
                Fx::from_int(45),
                Fx::ONE,
                Fx::from_int(30),
                Fx::from_int(20),
                cos70,
                Fx::ZERO
            ),
            PenResult::NonPenetration
        );
    }

    #[test]
    fn closed_form_chance() {
        let cos80 = deg(80).cos();
        let p = |pen: i32, t: i32| {
            pen_chance_ppm(Fx::from_int(pen), Fx::ONE, Fx::from_int(t), Fx::ZERO, cos80)
        };
        assert_eq!(p(100, 90), PPM);
        assert_eq!(p(100, 110), 0);
        // Even odds at the centre of the band.
        assert!(
            (p(100, 100) as i64 - 500_000).abs() < 200,
            "{}",
            p(100, 100)
        );
        // Monotonic.
        let mut last = PPM;
        for t in 90..=110 {
            let now = p(100, t);
            assert!(now <= last);
            last = now;
        }
        // A ricochet never penetrates.
        assert_eq!(
            pen_chance_ppm(Fx::from_int(500), deg(85).cos(), Fx::ONE, Fx::ZERO, cos80),
            0
        );
    }

    #[test]
    fn sampled_variation_matches_the_closed_form() {
        let (pen0, c, t) = (Fx::from_int(100), Fx::ONE, Fx::from_int(97));
        let cos80 = deg(80).cos();
        let n = 20_000u32;
        let mut pens = 0u32;
        for k in 0..n {
            let mut r = RngStream::new(7, k as u64, SimId(3), Purpose(9999));
            let v = variation(&mut r);
            assert!(v.abs() <= PEN_BAND);
            if penetrates(pen0, c, t, Fx::ZERO, cos80, v) == PenResult::Penetration {
                pens += 1;
            }
        }
        let p = pen_chance_ppm(pen0, c, t, Fx::ZERO, cos80) as f64 / PPM as f64;
        let expect = p * n as f64;
        let sigma = (n as f64 * p * (1.0 - p)).sqrt();
        assert!(
            (pens as f64 - expect).abs() <= 4.0 * sigma + 1.0,
            "{pens} vs {expect}"
        );
    }

    fn v(x: i32, y: i32, z: i32) -> FxVec3 {
        FxVec3::new(Fx::from_milli(x), Fx::from_milli(y), Fx::from_milli(z))
    }

    #[test]
    fn slab_names_each_face_and_breaks_edge_ties_by_axis() {
        let half = v(2000, 1000, 500);
        // From outside along each axis, toward the centre.
        let cases = [
            (v(5000, 0, 0), v(-6000, 0, 0), Face::Front),
            (v(-5000, 0, 0), v(6000, 0, 0), Face::Rear),
            (v(0, 4000, 0), v(0, -6000, 0), Face::Left),
            (v(0, -4000, 0), v(0, 6000, 0), Face::Right),
            (v(0, 0, 3000), v(0, 0, -6000), Face::Top),
            (v(0, 0, -3000), v(0, 0, 6000), Face::Bottom),
        ];
        for (p, q, want) in cases {
            let (t, face, at) = slab(p, q, half).expect("hit");
            assert_eq!(face, want, "{p:?}");
            // Enters on the face's plane.
            let on = match want {
                Face::Front | Face::Rear => at.x.abs(),
                Face::Left | Face::Right => at.y.abs(),
                Face::Top | Face::Bottom => at.z.abs(),
            };
            let h = match want {
                Face::Front | Face::Rear => half.x,
                Face::Left | Face::Right => half.y,
                Face::Top | Face::Bottom => half.z,
            };
            assert!((on - h).abs() <= Fx::from_raw(4), "{want:?}: {at:?}");
            assert!(t > Fx::ZERO && t < Fx::ONE);
        }
        // Oblique: through the left face of the front half.
        let (_, face, _) = slab(v(1000, 3000, 0), v(0, -4000, 100), half).unwrap();
        assert_eq!(face, Face::Left);
        // An edge (front and left at once): x wins; a corner: x wins too.
        let (_, face, _) = slab(v(3000, 2000, 0), v(-2000, -2000, 0), half).unwrap();
        assert_eq!(face, Face::Front);
        let (_, face, _) = slab(v(3000, 2000, 1500), v(-2000, -2000, -2000), half).unwrap();
        assert_eq!(face, Face::Front);
        // Left and top together: y before z.
        let (_, face, _) = slab(v(0, 2000, 1500), v(0, -2000, -2000), half).unwrap();
        assert_eq!(face, Face::Left);
        // Misses: parallel outside a slab, passing beside, stopping short,
        // pointing away.
        assert!(slab(v(5000, 1500, 0), v(-9000, 0, 0), half).is_none());
        assert!(slab(v(5000, 3000, 0), v(-9000, -1000, 0), half).is_none());
        assert!(slab(v(5000, 0, 0), v(-2000, 0, 0), half).is_none());
        assert!(slab(v(5000, 0, 0), v(2000, 0, 0), half).is_none());
        // Starting inside: t = 0.
        let (t, _, _) = slab(v(0, 0, 0), v(5000, 0, 0), half).unwrap();
        assert_eq!(t, Fx::ZERO);
    }

    #[test]
    fn track_band_is_the_lower_part_of_a_hull_side() {
        let hit = |part, face, z| ArmourHit {
            t: Fx::ZERO,
            part,
            face,
            zone: face.zone(part),
            normal: FxVec3::ZERO,
            point: FxVec3::ZERO,
            local: v(0, 1400, z),
        };
        let hz = Fx::from_milli(680);
        // 45 % of 1.36 m = 0.612 m from the bottom (z = −0.068).
        assert!(in_track_band(&hit(BoxPart::HULL, Face::Left, -100), hz, 45));
        assert!(!in_track_band(&hit(BoxPart::HULL, Face::Left, -50), hz, 45));
        assert!(in_track_band(
            &hit(BoxPart::HULL, Face::Right, -600),
            hz,
            45
        ));
        assert!(!in_track_band(
            &hit(BoxPart::HULL, Face::Front, -600),
            hz,
            45
        ));
        assert!(!in_track_band(&hit(BoxPart(1), Face::Left, -600), hz, 45));
    }

    #[test]
    fn segment_near_and_box_distance() {
        let c = v(0, 0, 0);
        assert!(segment_near(
            v(-5000, 1000, 0),
            v(10_000, 0, 0),
            c,
            Fx::from_milli(1001)
        ));
        assert!(!segment_near(
            v(-5000, 1000, 0),
            v(10_000, 0, 0),
            c,
            Fx::from_milli(999)
        ));
        // Beyond the end: distance to the end point.
        assert!(!segment_near(
            v(2000, 0, 0),
            v(3000, 0, 0),
            c,
            Fx::from_milli(1900)
        ));
        let b = BoxFrame {
            center: c,
            axes: crate::vehicle::Basis::hull(Fx::ZERO, Fx::ZERO, Fx::ZERO),
            half: v(1000, 1000, 1000),
        };
        assert_eq!(box_dist_sq_raw(&b, v(500, 0, 0)), 0);
        let d = Fx::sqrt_q32(box_dist_sq_raw(&b, v(3000, 0, 0)));
        assert!((d - Fx::from_int(2)).abs() <= Fx::from_raw(8), "{d:?}");
    }

    #[test]
    fn aimed_descent_is_the_drop_compensation_angle() {
        // 790 m/s at 1000 m: atan(9.81 · 1.266 / 1580) = 0.45°.
        let d = aimed_descent(Fx::from_int(790), Fx::from_int(1000));
        assert!(close(d, 0.00786, 2e-5), "{d:?}");
        assert_eq!(aimed_descent(Fx::ZERO, Fx::from_int(10)), Fx::ZERO);
    }
}
