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
fn solve_charge(
    r: i128,
    h: i128,
    v: i128,
    tan_min: i128,
    tan_max: i128,
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
    let u = (r + sq) * ONE / (2 * a);
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
pub fn solve(ind: &IndirectSetup, from: FxVec3, to: FxVec3) -> Option<Shot> {
    let d = to.xy() - from.xy();
    let r = d.length().raw() as i128;
    let h = (to.z - from.z).raw() as i128;
    let (tmin, tmax) = (
        Fx::from(ind.tan_min).raw() as i128,
        Fx::from(ind.tan_max).raw() as i128,
    );
    for (k, &v) in ind.charges.iter().enumerate() {
        let v = Fx::from(v).raw() as i128;
        let Some((u, vx, t)) = solve_charge(r, h, v, tmin, tmax) else {
            continue;
        };
        let sat = |x: i128| x.clamp(i32::MIN as i128, i32::MAX as i128) as i32;
        let vxx = sat(vx * d.x.raw() as i128 / r);
        let vxy = sat(vx * d.y.raw() as i128 / r);
        let vz = sat(u * vx / ONE) + G.raw() / (2 * SUBSTEP_HZ);
        return Some(Shot {
            charge: k as u8,
            vel: FxVec3::new(Fx::from_raw(vxx), Fx::from_raw(vxy), Fx::from_raw(vz)),
            flight: Fx::from_raw(sat(t)),
            tan: Fx::from_raw(sat(u)),
        });
    }
    None
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
        })
    }

    fn m1() -> IndirectSetup {
        IndirectSetup::lower(&frontline_content::defs::IndirectDef {
            charges_m_s: vec![70, 95, 117, 137, 155, 172, 188],
            elevation_deg: [45, 85],
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
}
