//! S5b: the high-angle charge solver (artillery-on-map.md §4.5, §4.8).
//!
//! A mortar with an `[indirect]` table (`IndirectSetup`: muzzle velocity
//! per charge, the tangents of its laying limits) fires each bomb with the
//! **lowest charge** that has a high-angle solution inside the limits. For
//! horizontal distance `R`, height difference `h` (target minus muzzle) and
//! charge velocity `v`, the elevation `u = tan θ` solves
//! `a u² − R u + (a + h) = 0` with `a = g R² / (2 v²)`; the high root
//! `u = (R + sqrt(R² − 4a(a + h))) / (2a)` is taken. The launch velocity is
//! `vx = v / sqrt(1 + u²)` along the ground and `vz = u · vx`; the flight
//! time is `R / vx`. No drag: sim shells fly with `drag = 0`, so the bomb
//! lands where the solver aims it.
//!
//! Pure integer math on raw Q16.16 values widened to `i128` (600 m squared
//! does not fit `Fx`). The projectile integrates semi-implicit Euler at
//! [`SUBSTEP_HZ`] (velocity first), which lands `g·t·dt/2` low; the launch
//! `vz` gets half a substep of gravity added, so the discrete arc passes
//! through the solved one at every substep.

use frontline_content::setup::IndirectSetup;

use crate::fx::{Fx, FxVec3};
use crate::projectile::SUBSTEP_HZ;
use crate::terrain::Heightfield;

/// 9.81 m/s².
const G: Fx = Fx::from_ratio(981, 100);
const ONE: i128 = 1 << Fx::FRAC_BITS;

/// One solved bomb.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Shot {
    /// Index of the charge used (0 = the lowest).
    pub charge: u8,
    /// Launch velocity (m/s).
    pub vel: FxVec3,
    /// Time of flight (s).
    pub flight: Fx,
    /// `tan` of the elevation.
    pub tan: Fx,
}

/// The high-angle solution of charge velocity `v` for horizontal distance
/// `r` and height difference `h` (m): `(u, vx, flight)` as raw Q16.16, or
/// `None` when it cannot reach or the elevation is outside `[tan_min,
/// tan_max]`.
///
/// S11a1: with `low` the low root `u = (R − sqrt(R² − 4a(a + h))) / (2a)`.
fn solve_root(
    r: i128,
    h: i128,
    v: i128,
    tan_min: i128,
    tan_max: i128,
    low: bool,
) -> Option<(i128, i128, i128)> {
    if r <= 0 || v <= 0 {
        return None;
    }
    // a = g R² / (2 v²), Q16.16.
    let a = G.raw() as i128 * r * r / (2 * v * v);
    if a <= 0 {
        return None;
    }
    // R² − 4a(a + h), Q32.32.
    let disc = r * r - 4 * a * (a + h);
    if disc < 0 {
        return None;
    }
    let sq = (disc as u128).isqrt() as i128;
    let u = if low {
        (r - sq) * ONE / (2 * a)
    } else {
        (r + sq) * ONE / (2 * a)
    };
    if u < tan_min || u > tan_max {
        return None;
    }
    let n = ((ONE * ONE + u * u) as u128).isqrt() as i128;
    let vx = v * ONE / n;
    if vx <= 0 {
        return None;
    }
    let t = r * ONE / vx;
    Some((u, vx, t))
}

/// Solves a bomb from muzzle `from` to ground point `to`: the lowest charge
/// with a high-angle solution inside the laying limits. `None`: no charge
/// can (too close for the top elevation, or beyond the highest charge).
///
/// S11a1: a `low_angle` table (a gun's) tries each charge's high root and
/// then its low root (the first inside the band wins); a mortar's table
/// takes the high root only, as before.
pub fn solve(ind: &IndirectSetup, from: FxVec3, to: FxVec3) -> Option<Shot> {
    candidates(ind, from, to).next()
}

/// Every solution inside the band, in the order the solver tries them:
/// lowest charge first, the high root before the low root (`low_angle`
/// tables only; a double root is offered once).
fn candidates(ind: &IndirectSetup, from: FxVec3, to: FxVec3) -> impl Iterator<Item = Shot> + '_ {
    let d = to.xy() - from.xy();
    let r = d.length().raw() as i128;
    let h = (to.z - from.z).raw() as i128;
    let (tmin, tmax) = (
        Fx::from(ind.tan_min).raw() as i128,
        Fx::from(ind.tan_max).raw() as i128,
    );
    let roots: &[bool] = if ind.low_angle {
        &[false, true]
    } else {
        &[false]
    };
    ind.charges.iter().enumerate().flat_map(move |(k, &v)| {
        let v = Fx::from(v).raw() as i128;
        let mut prev: Option<i128> = None;
        roots.iter().filter_map(move |&low| {
            let (u, vx, t) = solve_root(r, h, v, tmin, tmax, low)?;
            if prev == Some(u) {
                return None;
            }
            prev = Some(u);
            let sat = |x: i128| x.clamp(i32::MIN as i128, i32::MAX as i128) as i32;
            let vxx = sat(vx * d.x.raw() as i128 / r);
            let vxy = sat(vx * d.y.raw() as i128 / r);
            let vz = sat(u * vx / ONE) + G.raw() / (2 * SUBSTEP_HZ);
            Some(Shot {
                charge: k as u8,
                vel: FxVec3::new(Fx::from_raw(vxx), Fx::from_raw(vxy), Fx::from_raw(vz)),
                flight: Fx::from_raw(sat(t)),
                tan: Fx::from_raw(sat(u)),
            })
        })
    })
}

/// S11a1 G2: steps of a flight the mask test reads ([`MASK_FIRST`] to
/// [`MASK_LAST`] of [`MASK_STEPS`]), and the clearance (m) the shell must
/// keep above the ground there.
pub const MASK_STEPS: i64 = 20;
pub const MASK_FIRST: i64 = 2;
pub const MASK_LAST: i64 = 16;
const MASK_CLEAR: Fx = Fx::from_ratio(1, 2);

/// S11a1 G2: the gun's mount as the solver checks it: the hull's axes where
/// it stands and the mount's elevation limits (rad).
#[derive(Copy, Clone, Debug)]
pub struct GunMount {
    pub hull: crate::vehicle::Basis,
    pub lo: Fx,
    pub hi: Fx,
}

/// Whether shot `s` from `from` clears the ground by [`MASK_CLEAR`] at
/// steps [`MASK_FIRST`]..=[`MASK_LAST`] of [`MASK_STEPS`] (no rays: one
/// height read a step). The arc is the drag-free one the solver solved
/// (the half-substep `vz` correction left out).
pub fn arc_clear(terrain: &Heightfield, from: FxVec3, s: &Shot) -> bool {
    let t_all = s.flight.raw() as i64;
    let g = G.raw() as i64;
    let vz = s.vel.z.raw() as i64 - g / (2 * SUBSTEP_HZ as i64);
    let one = 1i64 << Fx::FRAC_BITS;
    (MASK_FIRST..=MASK_LAST).all(|i| {
        let t = t_all * i / MASK_STEPS;
        let at = |p: Fx, v: Fx| {
            Fx::from_raw(
                (p.raw() as i64 + v.raw() as i64 * t / one).clamp(i32::MIN as i64, i32::MAX as i64)
                    as i32,
            )
        };
        let xy = crate::fx::FxVec2::new(at(from.x, s.vel.x), at(from.y, s.vel.y));
        let z = from.z.raw() as i64 + vz * t / one - g * t / one * t / one / 2;
        let ground = terrain.height_at(terrain.clamp(xy)) + MASK_CLEAR;
        z >= ground.raw() as i64
    })
}

/// S11a1 G2: a gun's solution from muzzle `from` to ground point `to`: the
/// first candidate (lowest charge, high root then low root) that the mount
/// can lay where the gun stands (`mount`) and, with `mask`, whose arc
/// clears the ground ([`arc_clear`]). Refusals: `Masked` when a candidate
/// was in the band and layable but none cleared; else `InsideMinimum` when
/// the range is under `min`; else `CannotReach`. At most 10 candidates x
/// 15 height reads; no rays.
pub fn solve_gun(
    ind: &IndirectSetup,
    from: FxVec3,
    to: FxVec3,
    terrain: &Heightfield,
    mask: bool,
    mount: Option<&GunMount>,
    min: Fx,
) -> Result<Shot, crate::fire_mission::FireMissionRefusal> {
    use crate::fire_mission::FireMissionRefusal as R;
    let mut masked = false;
    for s in candidates(ind, from, to) {
        if let Some(m) = mount {
            let pitch = crate::gunnery::to_lay(&m.hull, s.vel).1;
            if pitch < m.lo || pitch > m.hi {
                continue;
            }
        }
        if mask && !arc_clear(terrain, from, &s) {
            masked = true;
            continue;
        }
        return Ok(s);
    }
    if masked {
        Err(R::Masked)
    } else if (to.xy() - from.xy()).length() < min {
        Err(R::InsideMinimum)
    } else {
        Err(R::CannotReach)
    }
}

/// S5b: an upper bound on any bomb's time of flight (whole seconds): the
/// level-ground maximum `2 v / g` of the highest charge (at 90 deg), plus
/// 20 s for a target up to 1,000 m below the muzzle (`sqrt(2 H / g)` is
/// 14.3 s). The load check of a saved mission bounds its burst deadline
/// with it (Review L1: a fixed 60 s refused saves of modded long-flight
/// mortars).
pub fn flight_bound_s(ind: &IndirectSetup) -> u64 {
    let v = ind.charges.iter().map(|c| c.0).max().unwrap_or(0).max(0) as u64;
    // 2 v / 9.81 with v raw Q16.16, rounded up.
    (v * 200).div_ceil(981 << Fx::FRAC_BITS) + 20
}

#[cfg(test)]
// The f64 reference and the miss in metres are test-side only.
#[allow(clippy::float_arithmetic)]
mod tests {
    use super::*;
    use frontline_content::Fixed;

    use crate::ecs::SimId;
    use crate::fx::FxVec2;
    use crate::projectile::{NO_AMMO, Projectile, advance};

    /// The GrW 34's table (mods/ww2 grw34.toml).
    fn grw34() -> IndirectSetup {
        IndirectSetup::lower(&frontline_content::defs::IndirectDef {
            charges_m_s: vec![75, 105, 130, 152, 174],
            elevation_deg: [45, 87],
            ..Default::default()
        })
    }

    fn m1() -> IndirectSetup {
        IndirectSetup::lower(&frontline_content::defs::IndirectDef {
            charges_m_s: vec![70, 95, 117, 137, 155, 172, 188],
            elevation_deg: [45, 85],
            ..Default::default()
        })
    }

    /// f64 reference: (charge, tan, flight) of the lowest valid charge.
    fn reference(ind: &IndirectSetup, r: f64, h: f64) -> Option<(usize, f64, f64)> {
        let to_f = |f: Fixed| f.0 as f64 / 65536.0;
        for (k, &v) in ind.charges.iter().enumerate() {
            let v = to_f(v);
            let a = 9.81 * r * r / (2.0 * v * v);
            let disc = r * r - 4.0 * a * (a + h);
            if disc < 0.0 {
                continue;
            }
            let u = (r + disc.sqrt()) / (2.0 * a);
            if u < to_f(ind.tan_min) || u > to_f(ind.tan_max) {
                continue;
            }
            let vx = v / (1.0 + u * u).sqrt();
            return Some((k, u, r / vx));
        }
        None
    }

    #[test]
    fn matches_an_f64_reference_within_half_a_percent() {
        for ind in [grw34(), m1()] {
            for r in (60..=600).step_by(10) {
                for h in [-20i32, 0, 20] {
                    let from = FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::from_int(100));
                    let to = FxVec3::new(Fx::from_int(r), Fx::ZERO, Fx::from_int(100 + h));
                    let got = solve(&ind, from, to);
                    let want = reference(&ind, r as f64, h as f64);
                    match (got, want) {
                        (None, None) => {}
                        (Some(s), Some((k, u, t))) => {
                            assert_eq!(s.charge as usize, k, "charge at {r} m, h {h}");
                            let tf = s.flight.raw() as f64 / 65536.0;
                            let uf = s.tan.raw() as f64 / 65536.0;
                            assert!((tf - t).abs() / t < 0.005, "flight {tf} vs {t} at {r} m");
                            let (a, b) = (uf.atan(), u.atan());
                            assert!((a - b).abs() / b < 0.005, "angle at {r} m");
                        }
                        (g, w) => panic!("{r} m, h {h}: {g:?} vs {w:?}"),
                    }
                }
            }
        }
    }

    #[test]
    fn grw34_charges_flight_times_and_limits() {
        let ind = grw34();
        let at = |r: i32| {
            solve(
                &ind,
                FxVec3::ZERO,
                FxVec3::new(Fx::from_int(r), Fx::ZERO, Fx::ZERO),
            )
        };
        // Charge I from 60 m (87 deg, the firing table's first row) to 560
        // m, charge II beyond.
        for r in [60, 100, 200, 300, 400, 500, 560] {
            assert_eq!(at(r).expect("reaches").charge, 0, "{r} m");
        }
        assert_eq!(at(600).expect("reaches").charge, 1);
        // Too close for the top elevation.
        assert!(at(50).is_none());
        let t = |r: i32| at(r).unwrap().flight.raw() as f64 / 65536.0;
        assert!((14.8..15.4).contains(&t(200)), "{}", t(200));
        assert!((13.9..14.5).contains(&t(400)), "{}", t(400));
        assert!((20.0..21.2).contains(&t(600)), "{}", t(600));
    }

    /// Flies a solved bomb with the sim's own integrator over flat ground
    /// at the target's height and returns the 2D miss (m).
    fn fly(ind: &IndirectSetup, r: i32, h: i32) -> f64 {
        let from = FxVec3::new(Fx::from_int(10), Fx::from_int(10), Fx::from_int(50));
        let dir = FxVec2::new(Fx::from_ratio(3, 5), Fx::from_ratio(4, 5));
        let to_xy = from.xy() + dir * Fx::from_int(r);
        let to = to_xy.extend(Fx::from_int(50 + h));
        let s = solve(ind, from, to).expect("reaches");
        let mut p = Projectile {
            pos: from,
            vel: s.vel,
            owner: SimId(0),
            drag: Fx::ZERO,
            ttl: u16::MAX,
            weapon: 0,
            player: 0,
            ammo: NO_AMMO,
            origin: from,
            mission: 0,
        };
        let mut steps = 0;
        loop {
            let next = advance(&mut p);
            if p.vel.z < Fx::ZERO && next.z <= to.z {
                // Interpolate the crossing of the target height.
                let dz = (p.pos.z - next.z).raw() as f64;
                let f = if dz > 0.0 {
                    (p.pos.z - to.z).raw() as f64 / dz
                } else {
                    0.0
                };
                let x = p.pos.x.raw() as f64 + f * (next.x - p.pos.x).raw() as f64;
                let y = p.pos.y.raw() as f64 + f * (next.y - p.pos.y).raw() as f64;
                let (ex, ey) = (to.x.raw() as f64, to.y.raw() as f64);
                return ((x - ex).powi(2) + (y - ey).powi(2)).sqrt() / 65536.0;
            }
            p.pos = next;
            steps += 1;
            assert!(steps < 60 * 60, "never landed");
        }
    }

    #[test]
    fn a_fired_bomb_lands_within_a_metre_of_its_aim_point() {
        for ind in [grw34(), m1()] {
            for r in [100, 300, 600] {
                for h in [-20, 0, 20] {
                    let miss = fly(&ind, r, h);
                    assert!(miss < 1.0, "{r} m, h {h}: {miss} m");
                }
            }
        }
    }

    /// Review L1: a modded long-flight table (it passes `validate`) flies
    /// past 60 s; the bound covers every solved flight, drops included.
    #[test]
    fn flight_bound_covers_long_modded_tables() {
        let ind = IndirectSetup::lower(&frontline_content::defs::IndirectDef {
            charges_m_s: vec![100, 300],
            elevation_deg: [45, 89],
            ..Default::default()
        });
        let bound = flight_bound_s(&ind) as f64;
        let mut longest = 0.0f64;
        for r in (40..=5000).step_by(20) {
            for h in [-200i32, 0, 50] {
                let from = FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::from_int(300));
                let to = FxVec3::new(Fx::from_int(r), Fx::ZERO, Fx::from_int(300 + h));
                if let Some(s) = solve(&ind, from, to) {
                    let t = s.flight.raw() as f64 / 65536.0;
                    longest = longest.max(t);
                    assert!(t <= bound, "{r} m, h {h}: {t} s > {bound} s");
                }
            }
        }
        assert!(
            longest > 60.0,
            "the table should fly past 60 s, longest {longest}"
        );
        // The ww2 tables stay under the old fixed 60 s window.
        assert!(flight_bound_s(&grw34()) + 4 <= 60);
        assert!(flight_bound_s(&m1()) + 4 <= 64);
    }

    // ---- S11a1 G2: the low root, the candidate order, the mask ----

    fn table(charges: &[u32], band: [u32; 2], low: bool) -> IndirectSetup {
        IndirectSetup::lower(&frontline_content::defs::IndirectDef {
            charges_m_s: charges.to_vec(),
            elevation_deg: band,
            low_angle: low,
            ..Default::default()
        })
    }

    /// The 105 mm M3's table (S11a1 C2: +30 deg, so low-angle only).
    fn m3() -> IndirectSetup {
        table(&[205, 220, 240, 270, 311], [0, 30], true)
    }

    /// The leIG 18's table (S11a1 C1: band 0..73).
    fn leig() -> IndirectSetup {
        table(&[92, 120, 150, 180, 210], [0, 73], true)
    }

    /// f64 reference of the S11a1 candidate order: lowest charge first,
    /// high root then low root, the first inside the band.
    fn reference_gun(ind: &IndirectSetup, r: f64, h: f64) -> Option<(usize, f64, f64)> {
        let to_f = |f: Fixed| f.0 as f64 / 65536.0;
        for (k, &v) in ind.charges.iter().enumerate() {
            let v = to_f(v);
            let a = 9.81 * r * r / (2.0 * v * v);
            let disc = r * r - 4.0 * a * (a + h);
            if disc < 0.0 {
                continue;
            }
            for u in [(r + disc.sqrt()) / (2.0 * a), (r - disc.sqrt()) / (2.0 * a)] {
                if u < to_f(ind.tan_min) || u > to_f(ind.tan_max) {
                    continue;
                }
                let vx = v / (1.0 + u * u).sqrt();
                return Some((k, u, r / vx));
            }
        }
        None
    }

    /// G2: the low root against an f64 reference (angle and flight within
    /// half a percent, the same charge). Control: the same M3 table with
    /// `low_angle` false has no solution at 300 m (the high root is far
    /// above 30 deg), so the low root is what the gun fires.
    #[test]
    fn low_root_matches_an_f64_reference() {
        let mut n_low = 0;
        for ind in [m3(), leig()] {
            for r in (100..=9000).step_by(50) {
                for h in [-20i32, 0, 20] {
                    let from = FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::from_int(100));
                    let to = FxVec3::new(Fx::from_int(r), Fx::ZERO, Fx::from_int(100 + h));
                    let got = solve(&ind, from, to);
                    let want = reference_gun(&ind, r as f64, h as f64);
                    match (got, want) {
                        (None, None) => {}
                        (Some(s), Some((k, u, t))) => {
                            assert_eq!(s.charge as usize, k, "charge at {r} m, h {h}");
                            let tf = s.flight.raw() as f64 / 65536.0;
                            let uf = s.tan.raw() as f64 / 65536.0;
                            assert!((tf - t).abs() / t < 0.005, "flight {tf} vs {t} at {r} m");
                            let (a, b) = (uf.atan(), u.atan());
                            assert!(
                                (a - b).abs() / b.max(0.01) < 0.005,
                                "angle {a} vs {b} at {r} m"
                            );
                            n_low += (u < 1.0) as u32;
                        }
                        (g, w) => panic!("{r} m, h {h}: {g:?} vs {w:?}"),
                    }
                }
            }
        }
        assert!(n_low > 100, "the low root was exercised ({n_low})");
        let off = table(&[205, 220, 240, 270, 311], [0, 30], false);
        let to = FxVec3::new(Fx::from_int(300), Fx::ZERO, Fx::ZERO);
        assert!(
            solve(&off, FxVec3::ZERO, to).is_none(),
            "control: high root only"
        );
        assert!(solve(&m3(), FxVec3::ZERO, to).is_some());
    }

    /// G2: a mortar table (`low_angle` false) solves as before; with
    /// `low_angle` set on the same table nothing changes on level ground
    /// (the low root is under 45 deg there). The three tests above stay
    /// as they were.
    #[test]
    fn mortar_solutions_unchanged_with_low_angle_off() {
        for (old, charges, band) in [
            (grw34(), vec![75, 105, 130, 152, 174], [45, 87]),
            (m1(), vec![70, 95, 117, 137, 155, 172, 188], [45, 85]),
        ] {
            let low = table(&charges, band, true);
            let flat = Heightfield::generate(crate::terrain::TerrainKind::Flat, 1024, 0);
            for r in (40..=1200).step_by(5) {
                let from = FxVec3::new(Fx::from_int(10), Fx::from_int(10), Fx::ZERO);
                let to = FxVec3::new(Fx::from_int(10 + r), Fx::from_int(10), Fx::ZERO);
                let a = solve(&old, from, to);
                assert_eq!(a, solve(&low, from, to), "{r} m");
                // solve_gun without mount or mask is solve.
                assert_eq!(
                    a,
                    solve_gun(&old, from, to, &flat, false, None, Fx::ZERO).ok(),
                    "{r} m"
                );
            }
        }
    }

    /// A flat 1024 m field with a 25 m crest across x = 500 (top 40 m
    /// wide, 20 m flanks; the mask reads 20 steps of a flight, so a crest
    /// narrower than a step can fall between two reads).
    fn crest() -> Heightfield {
        let mut h = Heightfield::generate(crate::terrain::TerrainKind::Flat, 1024, 0);
        let step = 1024 / (h.dim() as i32 - 1);
        h.edit_heights_for_test(|gx, _, z| {
            let d = (gx as i32 * step - 500).abs();
            *z = if d <= 20 {
                Fx::from_int(25)
            } else if d <= 40 {
                Fx::from_int(25 * (40 - d) / 20)
            } else {
                Fx::ZERO
            };
        });
        h.rebuild_derived();
        h
    }

    /// Flies `s` from `from` with the sim's integrator until it meets the
    /// ground; returns where.
    fn burst(terrain: &Heightfield, from: FxVec3, s: &Shot) -> FxVec3 {
        let mut p = Projectile {
            pos: from,
            vel: s.vel,
            owner: SimId(0),
            drag: Fx::ZERO,
            ttl: u16::MAX,
            weapon: 0,
            player: 0,
            ammo: NO_AMMO,
            origin: from,
            mission: 0,
        };
        for _ in 0..(120 * SUBSTEP_HZ) {
            let next = advance(&mut p);
            if let Some(hit) = terrain.segment_hit(p.pos, next) {
                return hit;
            }
            p.pos = next;
        }
        panic!("never landed");
    }

    /// G2: an M3-like +30 deg table 300 m behind a 25 m crest, the target
    /// 80 m beyond it: `Masked`. Control `mask` off: it fires and the shell
    /// bursts on the crest.
    #[test]
    fn flat_gun_is_masked_behind_a_crest() {
        let t = crest();
        let from = FxVec3::new(Fx::from_int(200), Fx::from_int(500), Fx::ONE);
        let to = FxVec3::new(Fx::from_int(580), Fx::from_int(500), Fx::ZERO);
        assert_eq!(
            solve_gun(&m3(), from, to, &t, true, None, Fx::ZERO),
            Err(crate::fire_mission::FireMissionRefusal::Masked)
        );
        let s = solve_gun(&m3(), from, to, &t, false, None, Fx::ZERO).expect("control: fires");
        let b = burst(&t, from, &s);
        assert!(
            (460..=540).contains(&b.x.floor_int()),
            "bursts on the crest at {b:?}"
        );
    }

    /// G2: a +73 deg table behind the same crest clears it and its shell
    /// lands within 3 m of the target (no aim error, no scatter here).
    /// The gun stands 450 m behind the crest, not 300 m: at 380 m the
    /// leIG's lowest charge needs 77 deg (its 73 deg range is 482 m).
    /// Control: the +30 deg M3 table is masked from the same spot. (The
    /// mask reads steps 2..=16 of 20: the last fifth of a flight is never
    /// tested, by the spec.)
    #[test]
    fn high_angle_gun_clears_the_crest() {
        let t = crest();
        let from = FxVec3::new(Fx::from_int(50), Fx::from_int(500), Fx::ONE);
        let to = FxVec3::new(Fx::from_int(580), Fx::from_int(500), Fx::ZERO);
        let s = solve_gun(&leig(), from, to, &t, true, None, Fx::ZERO).expect("clears");
        let b = burst(&t, from, &s);
        let miss = (b.xy() - to.xy()).length();
        assert!(miss <= Fx::from_int(3), "miss {miss:?}");
        assert_eq!(
            solve_gun(&m3(), from, to, &t, true, None, Fx::ZERO),
            Err(crate::fire_mission::FireMissionRefusal::Masked),
            "control: the +30 deg M3 is masked from the same spot"
        );
    }

    /// G2 (spec: "a flat 300 m shot of the M3 has its apex at 2.6 m and
    /// must pass"): level ground, muzzle 1 m up. Control: a 3 m bank at the
    /// middle of the flight masks it.
    #[test]
    fn flat_shot_is_not_masked_on_level_ground() {
        let mut t = Heightfield::generate(crate::terrain::TerrainKind::Flat, 1024, 0);
        let from = FxVec3::new(Fx::from_int(100), Fx::from_int(500), Fx::ONE);
        let to = FxVec3::new(Fx::from_int(400), Fx::from_int(500), Fx::ZERO);
        let s = solve_gun(&m3(), from, to, &t, true, None, Fx::ZERO).expect("not masked");
        assert_eq!(s.charge, 0);
        let step = 1024 / (t.dim() as i32 - 1);
        t.edit_heights_for_test(|gx, _, z| {
            if (gx as i32 * step - 250).abs() <= 8 {
                *z = Fx::from_int(3);
            }
        });
        t.rebuild_derived();
        assert_eq!(
            solve_gun(&m3(), from, to, &t, true, None, Fx::ZERO).map(|s| s.charge),
            Err(crate::fire_mission::FireMissionRefusal::Masked),
            "control: a 3 m bank masks every charge"
        );
    }

    /// G2: the elevation laid on the hull. An M3 (mount -9..+30) standing
    /// nose-down 20 deg toward a target 3,500 m off cannot lay any
    /// solution (each needs 30.4 deg or more on the hull): `CannotReach`.
    /// Control: on level ground it fires.
    #[test]
    fn gun_on_a_slope_refuses_what_it_cannot_lay() {
        use crate::armour::deg;
        let t = Heightfield::generate(crate::terrain::TerrainKind::Flat, 1024, 0);
        let from = FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE);
        let to = FxVec3::new(Fx::from_int(3500), Fx::ZERO, Fx::ZERO);
        let mount = |pitch: i32| GunMount {
            hull: crate::vehicle::Basis::hull(Fx::ZERO, deg(pitch), Fx::ZERO),
            lo: deg(-9),
            hi: deg(30),
        };
        assert_eq!(
            solve_gun(&m3(), from, to, &t, false, Some(&mount(-20)), Fx::ZERO),
            Err(crate::fire_mission::FireMissionRefusal::CannotReach)
        );
        assert!(solve_gun(&m3(), from, to, &t, false, Some(&mount(0)), Fx::ZERO).is_ok());
    }
}
