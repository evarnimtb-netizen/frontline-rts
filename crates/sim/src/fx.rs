//! Q16.16 fixed-point scalar and vectors, plus deterministic trigonometry.
//!
//! Every value in sim state and every sim computation uses these types, so
//! results are bit-identical on every platform and compiler (§5.11).
//!
//! * [`Fx`] is a Q16.16 number in an `i32` (range ±32768, resolution 1/65536).
//!   Products and quotients use `i64` intermediates. Arithmetic saturates
//!   instead of wrapping or panicking, so an out-of-range result is clamped the
//!   same way on every machine.
//! * [`FxVec2`] / [`FxVec3`] are plain component vectors. Squared lengths are
//!   also available as raw `i64` (Q32.32) for distances beyond 181 m, whose
//!   square does not fit in Q16.16.
//! * `sin`/`cos` read a quarter-wave table built at compile time by an integer
//!   CORDIC in a `const fn`; `atan2` runs integer CORDIC vectoring at runtime.
//!   No platform float math is involved anywhere. The only CORDIC inputs are
//!   the committed integer constants below.
//!
//! Conversions to `f32` exist only for the presenter snapshot.

use core::fmt;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

use serde::{Deserialize, Serialize};

const FRAC_BITS: u32 = 16;
const ONE_RAW: i32 = 1 << FRAC_BITS;

#[inline]
const fn sat_i32(v: i64) -> i32 {
    if v > i32::MAX as i64 {
        i32::MAX
    } else if v < i32::MIN as i64 {
        i32::MIN
    } else {
        v as i32
    }
}

#[inline]
const fn sat_i64(v: i128) -> i64 {
    if v > i64::MAX as i128 {
        i64::MAX
    } else if v < i64::MIN as i128 {
        i64::MIN
    } else {
        v as i64
    }
}

/// Q16.16 fixed-point number.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Fx(i32);

impl Fx {
    pub const FRAC_BITS: u32 = FRAC_BITS;
    pub const ZERO: Fx = Fx(0);
    pub const ONE: Fx = Fx(ONE_RAW);
    pub const HALF: Fx = Fx(ONE_RAW / 2);
    pub const NEG_ONE: Fx = Fx(-ONE_RAW);
    pub const MAX: Fx = Fx(i32::MAX);
    pub const MIN: Fx = Fx(i32::MIN);
    /// Smallest positive value (1/65536).
    pub const EPSILON: Fx = Fx(1);
    /// π rounded to Q16.16.
    pub const PI: Fx = Fx(205_887);
    /// 2π rounded to Q16.16.
    pub const TAU: Fx = Fx(411_775);
    /// π/2 rounded to Q16.16.
    pub const FRAC_PI_2: Fx = Fx(102_944);

    #[inline]
    pub const fn from_raw(raw: i32) -> Fx {
        Fx(raw)
    }

    #[inline]
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// Integer to fixed point, saturating outside ±32767.
    #[inline]
    pub const fn from_int(v: i32) -> Fx {
        Fx(sat_i32((v as i64) << FRAC_BITS))
    }

    /// `num / den` as fixed point (truncated toward zero), saturating.
    /// A zero denominator saturates by the sign of `num` (0/0 = 0).
    #[inline]
    pub const fn from_ratio(num: i64, den: i64) -> Fx {
        if den == 0 {
            return Fx::sat_by_sign(num);
        }
        let n = (num as i128) << FRAC_BITS;
        let q = n / den as i128;
        Fx(if q > i32::MAX as i128 {
            i32::MAX
        } else if q < i32::MIN as i128 {
            i32::MIN
        } else {
            q as i32
        })
    }

    /// Thousandths to fixed point: `from_milli(1500)` is 1.5.
    #[inline]
    pub const fn from_milli(milli: i32) -> Fx {
        Fx::from_ratio(milli as i64, 1000)
    }

    #[inline]
    const fn sat_by_sign(v: i64) -> Fx {
        if v > 0 {
            Fx::MAX
        } else if v < 0 {
            Fx::MIN
        } else {
            Fx::ZERO
        }
    }

    /// Floor to integer.
    #[inline]
    pub const fn floor_int(self) -> i32 {
        self.0 >> FRAC_BITS
    }

    /// Round to nearest integer (halves round up).
    #[inline]
    pub const fn round_int(self) -> i32 {
        ((self.0 as i64 + (ONE_RAW as i64 / 2)) >> FRAC_BITS) as i32
    }

    /// Saturating multiply (result floored toward −∞).
    #[inline]
    pub const fn mul(self, rhs: Fx) -> Fx {
        Fx(sat_i32((self.0 as i64 * rhs.0 as i64) >> FRAC_BITS))
    }

    /// Saturating divide (truncated toward zero). Division by zero saturates
    /// by the sign of the dividend.
    #[inline]
    pub const fn div(self, rhs: Fx) -> Fx {
        if rhs.0 == 0 {
            return Fx::sat_by_sign(self.0 as i64);
        }
        Fx(sat_i32(((self.0 as i64) << FRAC_BITS) / rhs.0 as i64))
    }

    #[inline]
    pub const fn mul_int(self, rhs: i32) -> Fx {
        Fx(sat_i32(self.0 as i64 * rhs as i64))
    }

    /// Divide by an integer (truncated toward zero); `/0` saturates.
    #[inline]
    pub const fn div_int(self, rhs: i32) -> Fx {
        if rhs == 0 {
            return Fx::sat_by_sign(self.0 as i64);
        }
        Fx(sat_i32(self.0 as i64 / rhs as i64))
    }

    #[inline]
    pub const fn saturating_add(self, rhs: Fx) -> Fx {
        Fx(self.0.saturating_add(rhs.0))
    }

    #[inline]
    pub const fn saturating_sub(self, rhs: Fx) -> Fx {
        Fx(self.0.saturating_sub(rhs.0))
    }

    /// Absolute value; `MIN.abs()` saturates to `MAX`.
    #[inline]
    pub const fn abs(self) -> Fx {
        Fx(self.0.saturating_abs())
    }

    #[inline]
    pub const fn signum(self) -> i32 {
        self.0.signum()
    }

    #[inline]
    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    #[inline]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Square root (floor). Negative inputs return zero.
    #[inline]
    pub const fn sqrt(self) -> Fx {
        if self.0 <= 0 {
            return Fx::ZERO;
        }
        // sqrt(raw / 2^16) * 2^16 == sqrt(raw * 2^16)
        Fx(((self.0 as u64) << FRAC_BITS).isqrt() as i32)
    }

    /// Square root of a raw Q32.32 value (e.g. a squared length from
    /// [`FxVec2::length_sq_raw`]), returned as Q16.16 and saturated.
    #[inline]
    pub const fn sqrt_q32(raw_q32: u64) -> Fx {
        Fx(sat_i32(raw_q32.isqrt() as i64))
    }

    /// Converts a raw Q32.32 value (a product of two Q16.16 numbers) to
    /// Q16.16, saturating.
    #[inline]
    pub const fn from_q32(raw_q32: i64) -> Fx {
        Fx(sat_i32(raw_q32 >> FRAC_BITS))
    }

    /// Sine of an angle in radians.
    #[inline]
    pub fn sin(self) -> Fx {
        trig::sin_cos(self).0
    }

    /// Cosine of an angle in radians.
    #[inline]
    pub fn cos(self) -> Fx {
        trig::sin_cos(self).1
    }

    /// `(sin, cos)` of an angle in radians.
    #[inline]
    pub fn sin_cos(self) -> (Fx, Fx) {
        trig::sin_cos(self)
    }

    /// Four-quadrant arctangent of `y / x`, in radians in `(-π, π]`.
    /// `atan2(0, 0)` is 0.
    #[inline]
    pub fn atan2(y: Fx, x: Fx) -> Fx {
        trig::atan2(y, x)
    }

    /// Wraps an angle into `[-π, π)`, where π is [`Fx::PI`].
    ///
    /// The period is `2 * PI` (411774 raw), not [`Fx::TAU`] (411775 raw, 2π
    /// rounded on its own); with `TAU` the result could be `+PI`.
    #[inline]
    pub fn wrap_angle(self) -> Fx {
        let pi = Fx::PI.0 as i64;
        let a = (self.0 as i64 + pi).rem_euclid(2 * pi) - pi;
        Fx(a as i32)
    }

    /// Presenter-only conversion. Never feed the result back into the sim.
    #[allow(clippy::float_arithmetic, clippy::cast_precision_loss)]
    #[inline]
    pub fn to_f32(self) -> f32 {
        self.0 as f32 / ONE_RAW as f32
    }
}

impl fmt::Debug for Fx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for Fx {
    /// Exact decimal rendering with 5 fractional digits, integer-only.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let neg = self.0 < 0;
        let mag = (self.0 as i64).unsigned_abs();
        let int = mag >> FRAC_BITS;
        let frac = ((mag & (ONE_RAW as u64 - 1)) * 100_000) >> FRAC_BITS;
        write!(f, "{}{}.{:05}", if neg { "-" } else { "" }, int, frac)
    }
}

impl Add for Fx {
    type Output = Fx;
    #[inline]
    fn add(self, rhs: Fx) -> Fx {
        self.saturating_add(rhs)
    }
}
impl Sub for Fx {
    type Output = Fx;
    #[inline]
    fn sub(self, rhs: Fx) -> Fx {
        self.saturating_sub(rhs)
    }
}
impl Mul for Fx {
    type Output = Fx;
    #[inline]
    fn mul(self, rhs: Fx) -> Fx {
        Fx::mul(self, rhs)
    }
}
impl Div for Fx {
    type Output = Fx;
    #[inline]
    fn div(self, rhs: Fx) -> Fx {
        Fx::div(self, rhs)
    }
}
impl Neg for Fx {
    type Output = Fx;
    #[inline]
    fn neg(self) -> Fx {
        Fx(self.0.saturating_neg())
    }
}
impl AddAssign for Fx {
    #[inline]
    fn add_assign(&mut self, rhs: Fx) {
        *self = *self + rhs;
    }
}
impl SubAssign for Fx {
    #[inline]
    fn sub_assign(&mut self, rhs: Fx) {
        *self = *self - rhs;
    }
}
impl MulAssign for Fx {
    #[inline]
    fn mul_assign(&mut self, rhs: Fx) {
        *self = *self * rhs;
    }
}
impl DivAssign for Fx {
    #[inline]
    fn div_assign(&mut self, rhs: Fx) {
        *self = *self / rhs;
    }
}

/// 2D fixed-point vector (sim ground plane: x east, y north).
#[derive(
    Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize,
)]
pub struct FxVec2 {
    pub x: Fx,
    pub y: Fx,
}

impl FxVec2 {
    pub const ZERO: FxVec2 = FxVec2 {
        x: Fx::ZERO,
        y: Fx::ZERO,
    };

    #[inline]
    pub const fn new(x: Fx, y: Fx) -> FxVec2 {
        FxVec2 { x, y }
    }

    #[inline]
    pub const fn from_int(x: i32, y: i32) -> FxVec2 {
        FxVec2 {
            x: Fx::from_int(x),
            y: Fx::from_int(y),
        }
    }

    /// Dot product, saturated to Q16.16.
    #[inline]
    pub const fn dot(self, o: FxVec2) -> Fx {
        Fx::from_q32(self.dot_raw(o))
    }

    /// Dot product as raw Q32.32, saturated to `i64` (exact unless both
    /// terms are near ±2^62, i.e. components near `Fx::MIN`).
    #[inline]
    pub const fn dot_raw(self, o: FxVec2) -> i64 {
        sat_i64(self.x.0 as i128 * o.x.0 as i128 + self.y.0 as i128 * o.y.0 as i128)
    }

    /// 2D cross product / determinant `self.x * o.y - self.y * o.x`, saturated.
    /// The i64 difference cannot overflow: its magnitude is at most
    /// `2^62 + (2^62 - 2^31) < 2^63`.
    #[inline]
    pub const fn det(self, o: FxVec2) -> Fx {
        Fx::from_q32(self.x.0 as i64 * o.y.0 as i64 - self.y.0 as i64 * o.x.0 as i64)
    }

    /// Squared length as raw Q32.32; never overflows.
    #[inline]
    pub const fn length_sq_raw(self) -> u64 {
        let x = self.x.0 as i64;
        let y = self.y.0 as i64;
        (x * x) as u64 + (y * y) as u64
    }

    /// Squared length, saturated to Q16.16 (exact below ~181 m).
    #[inline]
    pub const fn length_sq(self) -> Fx {
        let r = self.length_sq_raw() >> FRAC_BITS;
        Fx(if r > i32::MAX as u64 {
            i32::MAX
        } else {
            r as i32
        })
    }

    #[inline]
    pub const fn length(self) -> Fx {
        Fx::sqrt_q32(self.length_sq_raw())
    }

    /// Unit vector in the same direction, or zero for a zero vector.
    #[inline]
    pub fn normalize_or_zero(self) -> FxVec2 {
        let len = self.length();
        if len.is_zero() {
            FxVec2::ZERO
        } else {
            FxVec2::new(self.x / len, self.y / len)
        }
    }

    /// Clamps the length to `max` (keeps direction).
    #[inline]
    pub fn clamp_length(self, max: Fx) -> FxVec2 {
        let len = self.length();
        if len <= max || len.is_zero() {
            self
        } else {
            self * (max / len)
        }
    }

    /// Counter-clockwise perpendicular `(-y, x)`.
    #[inline]
    pub fn perp(self) -> FxVec2 {
        FxVec2::new(-self.y, self.x)
    }

    #[inline]
    pub const fn div_int(self, d: i32) -> FxVec2 {
        FxVec2::new(self.x.div_int(d), self.y.div_int(d))
    }

    #[inline]
    pub const fn mul_int(self, m: i32) -> FxVec2 {
        FxVec2::new(self.x.mul_int(m), self.y.mul_int(m))
    }

    #[inline]
    pub const fn extend(self, z: Fx) -> FxVec3 {
        FxVec3 {
            x: self.x,
            y: self.y,
            z,
        }
    }

    #[inline]
    pub fn to_f32(self) -> [f32; 2] {
        [self.x.to_f32(), self.y.to_f32()]
    }
}

/// 3D fixed-point vector (x east, y north, z up).
#[derive(
    Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize,
)]
pub struct FxVec3 {
    pub x: Fx,
    pub y: Fx,
    pub z: Fx,
}

impl FxVec3 {
    pub const ZERO: FxVec3 = FxVec3 {
        x: Fx::ZERO,
        y: Fx::ZERO,
        z: Fx::ZERO,
    };

    #[inline]
    pub const fn new(x: Fx, y: Fx, z: Fx) -> FxVec3 {
        FxVec3 { x, y, z }
    }

    #[inline]
    pub const fn xy(self) -> FxVec2 {
        FxVec2 {
            x: self.x,
            y: self.y,
        }
    }

    #[inline]
    pub const fn dot(self, o: FxVec3) -> Fx {
        Fx::from_q32(self.dot_raw(o))
    }

    /// Dot product as raw Q32.32, saturated to `i64`.
    #[inline]
    pub const fn dot_raw(self, o: FxVec3) -> i64 {
        sat_i64(
            self.x.0 as i128 * o.x.0 as i128
                + self.y.0 as i128 * o.y.0 as i128
                + self.z.0 as i128 * o.z.0 as i128,
        )
    }

    #[inline]
    pub fn cross(self, o: FxVec3) -> FxVec3 {
        FxVec3::new(
            FxVec2::new(self.y, self.z).det(FxVec2::new(o.y, o.z)),
            FxVec2::new(self.z, self.x).det(FxVec2::new(o.z, o.x)),
            FxVec2::new(self.x, self.y).det(FxVec2::new(o.x, o.y)),
        )
    }

    /// Squared length as raw Q32.32; never overflows.
    #[inline]
    pub const fn length_sq_raw(self) -> u64 {
        let x = self.x.0 as i64;
        let y = self.y.0 as i64;
        let z = self.z.0 as i64;
        (x * x) as u64 + (y * y) as u64 + (z * z) as u64
    }

    #[inline]
    pub const fn length(self) -> Fx {
        Fx::sqrt_q32(self.length_sq_raw())
    }

    #[inline]
    pub fn normalize_or_zero(self) -> FxVec3 {
        let len = self.length();
        if len.is_zero() {
            FxVec3::ZERO
        } else {
            FxVec3::new(self.x / len, self.y / len, self.z / len)
        }
    }

    #[inline]
    pub const fn div_int(self, d: i32) -> FxVec3 {
        FxVec3::new(self.x.div_int(d), self.y.div_int(d), self.z.div_int(d))
    }

    #[inline]
    pub const fn mul_int(self, m: i32) -> FxVec3 {
        FxVec3::new(self.x.mul_int(m), self.y.mul_int(m), self.z.mul_int(m))
    }

    #[inline]
    pub fn to_f32(self) -> [f32; 3] {
        [self.x.to_f32(), self.y.to_f32(), self.z.to_f32()]
    }
}

macro_rules! vec_ops {
    ($t:ident { $($f:ident),+ }) => {
        impl Add for $t {
            type Output = $t;
            #[inline]
            fn add(self, o: $t) -> $t { $t { $($f: self.$f + o.$f),+ } }
        }
        impl Sub for $t {
            type Output = $t;
            #[inline]
            fn sub(self, o: $t) -> $t { $t { $($f: self.$f - o.$f),+ } }
        }
        impl Neg for $t {
            type Output = $t;
            #[inline]
            fn neg(self) -> $t { $t { $($f: -self.$f),+ } }
        }
        impl Mul<Fx> for $t {
            type Output = $t;
            #[inline]
            fn mul(self, s: Fx) -> $t { $t { $($f: self.$f * s),+ } }
        }
        impl Div<Fx> for $t {
            type Output = $t;
            #[inline]
            fn div(self, s: Fx) -> $t { $t { $($f: self.$f / s),+ } }
        }
        impl AddAssign for $t {
            #[inline]
            fn add_assign(&mut self, o: $t) { *self = *self + o; }
        }
        impl SubAssign for $t {
            #[inline]
            fn sub_assign(&mut self, o: $t) { *self = *self - o; }
        }
    };
}
vec_ops!(FxVec2 { x, y });
vec_ops!(FxVec3 { x, y, z });

/// Integer CORDIC trigonometry. Internal precision is Q2.30.
mod trig {
    use super::Fx;

    /// `round(atan(2^-i) * 2^30)` for i = 0..=30. Committed constants; these
    /// and [`CORDIC_K`] are the only inputs to the trig tables.
    const ATAN_Q30: [i64; 31] = [
        843_314_857,
        497_837_829,
        263_043_837,
        133_525_159,
        67_021_687,
        33_543_516,
        16_775_851,
        8_388_437,
        4_194_283,
        2_097_149,
        1_048_576,
        524_288,
        262_144,
        131_072,
        65_536,
        32_768,
        16_384,
        8_192,
        4_096,
        2_048,
        1_024,
        512,
        256,
        128,
        64,
        32,
        16,
        8,
        4,
        2,
        1,
    ];
    /// CORDIC gain correction `prod(1/sqrt(1 + 2^-2i)) * 2^30`.
    const CORDIC_K: i64 = 652_032_874;
    /// π/2 in Q2.30.
    const HALF_PI_Q30: i64 = 1_686_629_713;
    const Q30_ONE: i64 = 1 << 30;

    /// Steps per quarter turn in the sine table.
    const QUARTER: usize = 1024;
    /// Steps per full turn.
    const FULL: i64 = 4 * QUARTER as i64;

    /// CORDIC rotation mode: `(sin θ, cos θ)` for θ in `[0, π/2]`, Q2.30.
    const fn cordic_sin_cos(theta: i64) -> (i64, i64) {
        let mut x = CORDIC_K;
        let mut y = 0i64;
        let mut z = theta;
        let mut i = 0;
        while i < ATAN_Q30.len() {
            let dx = x >> i;
            let dy = y >> i;
            if z >= 0 {
                x -= dy;
                y += dx;
                z -= ATAN_Q30[i];
            } else {
                x += dy;
                y -= dx;
                z += ATAN_Q30[i];
            }
            i += 1;
        }
        (y, x)
    }

    const fn build_sin_table() -> [i32; QUARTER + 1] {
        let mut t = [0i32; QUARTER + 1];
        let mut k = 1;
        while k < QUARTER {
            let theta = HALF_PI_Q30 * k as i64 / QUARTER as i64;
            let (s, _) = cordic_sin_cos(theta);
            t[k] = if s < 0 {
                0
            } else if s > Q30_ONE {
                Q30_ONE as i32
            } else {
                s as i32
            };
            k += 1;
        }
        t[0] = 0;
        t[QUARTER] = Q30_ONE as i32;
        t
    }

    /// Quarter-wave sine table, Q2.30, built at compile time from integers.
    static SIN_Q30: [i32; QUARTER + 1] = build_sin_table();

    /// Sine at full-turn step `n` (wrapped), Q2.30.
    #[inline]
    fn sin_step(n: i64) -> i64 {
        let n = n.rem_euclid(FULL) as usize;
        let r = n % QUARTER;
        match n / QUARTER {
            0 => SIN_Q30[r] as i64,
            1 => SIN_Q30[QUARTER - r] as i64,
            2 => -(SIN_Q30[r] as i64),
            _ => -(SIN_Q30[QUARTER - r] as i64),
        }
    }

    #[inline]
    fn q30_to_fx(v: i64) -> Fx {
        Fx::from_raw(((v + (1 << 13)) >> 14) as i32)
    }

    pub(super) fn sin_cos(angle: Fx) -> (Fx, Fx) {
        let tau = Fx::TAU.raw() as i64;
        let a = (angle.raw() as i64).rem_euclid(tau);
        // Position on the full turn in table steps with 16 fractional bits.
        let p = (a << 28) / tau;
        let idx = p >> 16;
        let frac = p & 0xFFFF;
        let lerp = |a: i64, b: i64| a + (((b - a) * frac) >> 16);
        let quarter = QUARTER as i64;
        let s = lerp(sin_step(idx), sin_step(idx + 1));
        let c = lerp(sin_step(idx + quarter), sin_step(idx + 1 + quarter));
        (q30_to_fx(s), q30_to_fx(c))
    }

    pub(super) fn atan2(y: Fx, x: Fx) -> Fx {
        let mut x = x.raw() as i64;
        let mut y = y.raw() as i64;
        if x == 0 && y == 0 {
            return Fx::ZERO;
        }
        // Normalise magnitude to ~2^40 for precision; headroom for CORDIC gain.
        let m = x.unsigned_abs().max(y.unsigned_abs());
        let bits = 64 - m.leading_zeros() as i32;
        let shift = 40 - bits;
        if shift > 0 {
            x <<= shift;
            y <<= shift;
        } else if shift < 0 {
            x >>= -shift;
            y >>= -shift;
        }
        // Rotate into the right half-plane.
        let mut z = 0i64;
        if x < 0 {
            if y >= 0 {
                (x, y) = (y, -x);
                z = HALF_PI_Q30;
            } else {
                (x, y) = (-y, x);
                z = -HALF_PI_Q30;
            }
        }
        // Vectoring mode: drive y to zero, accumulating the angle.
        let mut i = 0;
        while i < ATAN_Q30.len() {
            let dx = x >> i;
            let dy = y >> i;
            if y > 0 {
                x += dy;
                y -= dx;
                z += ATAN_Q30[i];
            } else {
                x -= dy;
                y += dx;
                z -= ATAN_Q30[i];
            }
            i += 1;
        }
        let r = q30_to_fx(z);
        // Keep the range (-π, π].
        if r <= -Fx::PI { Fx::PI } else { r.min(Fx::PI) }
    }
}

#[cfg(test)]
#[allow(clippy::float_arithmetic)]
mod tests {
    use super::*;

    const ULP: f64 = 1.0 / 65536.0;

    fn f(x: Fx) -> f64 {
        x.raw() as f64 / 65536.0
    }

    #[test]
    fn basic_arithmetic() {
        let a = Fx::from_int(3);
        let b = Fx::from_milli(1500);
        assert_eq!(a + b, Fx::from_milli(4500));
        assert_eq!(a - b, b);
        assert_eq!(a * b, Fx::from_milli(4500));
        assert_eq!(a / b, Fx::from_int(2));
        assert_eq!(-a, Fx::from_int(-3));
        assert_eq!(Fx::from_ratio(1, 4), Fx::from_raw(16384));
        assert_eq!(Fx::from_ratio(-7, 2), Fx::from_milli(-3500));
        assert_eq!(Fx::from_int(-3).abs(), a);
        assert_eq!(a.min(b), b);
        assert_eq!(a.max(b), a);
        assert!(Fx::from_int(-1) < Fx::ZERO);
        assert_eq!(Fx::from_milli(2500).floor_int(), 2);
        assert_eq!(Fx::from_milli(-2500).floor_int(), -3);
        assert_eq!(Fx::from_milli(2500).round_int(), 3);
        assert_eq!(Fx::from_int(7).mul_int(3), Fx::from_int(21));
        assert_eq!(Fx::from_int(7).div_int(2), Fx::from_milli(3500));
        // 1/3 is truncated; 3 * (1/3) is one ulp short of 1 at most.
        let third = Fx::from_ratio(1, 3);
        assert!((Fx::ONE - third.mul_int(3)).raw() <= 1);
        assert_eq!(format!("{}", Fx::from_milli(-1250)), "-1.25000");
    }

    #[test]
    fn saturation() {
        assert_eq!(Fx::MAX + Fx::ONE, Fx::MAX);
        assert_eq!(Fx::MIN - Fx::ONE, Fx::MIN);
        assert_eq!(Fx::from_int(30000) * Fx::from_int(30000), Fx::MAX);
        assert_eq!(Fx::from_int(-30000) * Fx::from_int(30000), Fx::MIN);
        assert_eq!(Fx::ONE / Fx::ZERO, Fx::MAX);
        assert_eq!(-Fx::ONE / Fx::ZERO, Fx::MIN);
        assert_eq!(Fx::ZERO / Fx::ZERO, Fx::ZERO);
        assert_eq!(Fx::from_int(1_000_000), Fx::MAX);
        assert_eq!(-Fx::MIN, Fx::MAX);
        assert_eq!(Fx::MIN.abs(), Fx::MAX);
        assert_eq!(Fx::from_int(20000) / Fx::from_milli(100), Fx::MAX);
    }

    #[test]
    fn sqrt_and_lengths() {
        assert_eq!(Fx::from_int(10_000).sqrt(), Fx::from_int(100));
        assert_eq!(Fx::from_int(-4).sqrt(), Fx::ZERO);
        for raw in [
            1,
            2,
            3,
            100,
            65_536,
            131_072,
            1 << 20,
            123_456_789,
            i32::MAX,
        ] {
            let x = Fx::from_raw(raw);
            let err = (f(x.sqrt()) - f(x).sqrt()).abs();
            assert!(err <= ULP, "sqrt({x}) err {err}");
        }
        let v = FxVec2::from_int(3, 4);
        assert_eq!(v.length(), Fx::from_int(5));
        assert_eq!(v.length_sq(), Fx::from_int(25));
        assert_eq!(
            FxVec3::new(Fx::from_int(2), Fx::from_int(3), Fx::from_int(6)).length(),
            Fx::from_int(7)
        );
        // 3 km vector: square overflows Q16.16 but the raw path does not.
        let far = FxVec2::from_int(3000, 4000);
        assert_eq!(far.length(), Fx::from_int(5000));
        assert_eq!(far.length_sq(), Fx::MAX);
        let n = FxVec2::from_int(10, 0).normalize_or_zero();
        assert_eq!(n, FxVec2::new(Fx::ONE, Fx::ZERO));
        assert_eq!(FxVec2::from_int(1, 0).det(FxVec2::from_int(0, 1)), Fx::ONE);
        let c = FxVec3::new(Fx::ONE, Fx::ZERO, Fx::ZERO).cross(FxVec3::new(
            Fx::ZERO,
            Fx::ONE,
            Fx::ZERO,
        ));
        assert_eq!(c, FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE));
    }

    #[test]
    fn sin_cos_accuracy() {
        // Sweep ±4 turns in steps of ~0.0012 rad.
        let mut max_err = 0f64;
        let mut raw = -4 * Fx::TAU.raw();
        while raw <= 4 * Fx::TAU.raw() {
            let a = Fx::from_raw(raw);
            let (s, c) = a.sin_cos();
            let x = f(a);
            max_err = max_err
                .max((f(s) - x.sin()).abs())
                .max((f(c) - x.cos()).abs());
            raw += 77;
        }
        assert!(
            max_err <= 3.0 * ULP,
            "sin/cos max error {max_err} ({} ulp)",
            max_err / ULP
        );
        assert_eq!(Fx::ZERO.sin(), Fx::ZERO);
        assert_eq!(Fx::ZERO.cos(), Fx::ONE);
        assert!((Fx::FRAC_PI_2.sin() - Fx::ONE).abs().raw() <= 2);
        assert!((Fx::PI.cos() + Fx::ONE).abs().raw() <= 2);
    }

    #[test]
    fn atan2_accuracy() {
        let mut max_err = 0f64;
        for r in [
            Fx::from_raw(50),
            Fx::from_milli(10),
            Fx::ONE,
            Fx::from_int(250),
            Fx::from_int(20_000),
        ] {
            for k in 0..3600 {
                let theta = Fx::from_raw(-Fx::PI.raw() + (k * Fx::TAU.raw()) / 3600);
                let (s, c) = theta.sin_cos();
                let (y, x) = (s * r, c * r);
                let got = f(Fx::atan2(y, x));
                let want = f(y).atan2(f(x));
                let mut err = (got - want).abs();
                if err > std::f64::consts::PI {
                    err = (err - std::f64::consts::TAU).abs(); // ±π wrap
                }
                // Tiny radii quantise (x, y) themselves; compare to their atan2.
                max_err = max_err.max(err);
            }
        }
        assert!(
            max_err <= 3.0 * ULP,
            "atan2 max error {max_err} ({} ulp)",
            max_err / ULP
        );
        assert_eq!(Fx::atan2(Fx::ZERO, Fx::ZERO), Fx::ZERO);
        assert_eq!(Fx::atan2(Fx::ZERO, Fx::NEG_ONE), Fx::PI);
        assert!((Fx::atan2(Fx::ONE, Fx::ZERO) - Fx::FRAC_PI_2).abs().raw() <= 1);
        assert!(
            (Fx::atan2(Fx::NEG_ONE, Fx::ZERO) + Fx::FRAC_PI_2)
                .abs()
                .raw()
                <= 1
        );
        assert!(
            (Fx::atan2(Fx::ONE, Fx::ONE) - Fx::from_raw(51_472))
                .abs()
                .raw()
                <= 1
        );
    }

    #[test]
    fn wrap_angle_range() {
        for raw in (-10 * Fx::TAU.raw()..10 * Fx::TAU.raw()).step_by(9973) {
            let w = Fx::from_raw(raw).wrap_angle();
            assert!(w >= -Fx::PI && w < Fx::PI);
            assert_eq!(
                (w - Fx::from_raw(raw)).raw().rem_euclid(2 * Fx::PI.raw()),
                0
            );
        }
        // Regression: +π used to wrap to +π (period TAU = 2·PI + 1 raw).
        assert_eq!(Fx::PI.wrap_angle(), -Fx::PI);
        assert_eq!((-Fx::PI).wrap_angle(), -Fx::PI);
        assert_eq!(Fx::TAU.wrap_angle(), Fx::from_raw(1));
    }

    #[test]
    fn dot_and_det_saturate_instead_of_overflowing() {
        let m = FxVec2::new(Fx::MIN, Fx::MIN);
        assert_eq!(m.dot_raw(m), i64::MAX);
        assert_eq!(m.dot(m), Fx::MAX);
        // det's extremes fit in i64 and saturate only to Q16.16.
        let a = FxVec2::new(Fx::MIN, Fx::MIN);
        let b = FxVec2::new(Fx::MAX, Fx::MIN);
        assert_eq!(a.det(b), Fx::MAX);
        assert_eq!(b.det(a), Fx::MIN);
        let m3 = FxVec3::new(Fx::MIN, Fx::MIN, Fx::MIN);
        assert_eq!(m3.dot_raw(m3), i64::MAX);
        let n3 = FxVec3::new(Fx::MAX, Fx::MAX, Fx::MAX);
        assert_eq!(m3.dot_raw(n3), i64::MIN);
        // Small values are exact.
        let u = FxVec2::from_int(3, -4);
        assert_eq!(u.dot(u), Fx::from_int(25));
        assert_eq!(u.det(FxVec2::from_int(1, 0)), Fx::from_int(4));
    }
}
