//! Counter-based random numbers (§5.11).
//!
//! There is no RNG *state*: every roll is a pure function of
//! `(world_seed, tick, sim_id, purpose)`, so the result never depends on the
//! order in which entities or threads ask for it. Several rolls for the same
//! key come from [`RngStream`], which adds a draw counter to the key.
//!
//! The mixer is the SplitMix64 finaliser, chained over the key words.
//! (`tick_salt` from §5.11 joins the key with networking in M1.)

use crate::ecs::SimId;
use crate::fx::Fx;

/// What a roll is for. Distinct purposes give independent streams for the
/// same entity and tick. New systems add constants here; never reuse a value.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Purpose(pub u32);

impl Purpose {
    pub const SPAWN: Purpose = Purpose(1);
    pub const WAYPOINT: Purpose = Purpose(2);
    pub const WEAPON_COOLDOWN: Purpose = Purpose(3);
    pub const WEAPON_SHOT: Purpose = Purpose(4);
    /// Initial soldier positions in `World::new` (kept apart from `SPAWN`,
    /// which `spawn_soldier` draws yaw and speed from for the same id).
    pub const SPAWN_POS: Purpose = Purpose(5);
    /// First waypoint at spawn (kept apart from `WAYPOINT`, which a re-pick
    /// on the same tick would otherwise repeat).
    pub const SPAWN_WAYPOINT: Purpose = Purpose(6);
    /// Aim error of a small-arms shot (match mode).
    pub const AIM: Purpose = Purpose(7);
    /// Scatter of a mortar shell's aim point (S5b: the precision spread
    /// around the mission's aim point).
    pub const SHELL_AIM: Purpose = Purpose(8);
    /// Crater depth of a burst (keyed by its event index in the tick).
    pub const CRATER: Purpose = Purpose(9);
    /// MG target pick among the nearest enemies.
    pub const SWEEP: Purpose = Purpose(10);
    /// AI commander plan noise (keyed by the player as the id).
    pub const AI_PLAN: Purpose = Purpose(11);
    /// Landing scatter of a thrown item (keyed by the thrower, with his
    /// throw count as the tick).
    pub const THROW: Purpose = Purpose(12);
    /// A thinning smoke cloud's sight-line roll (keyed by the target).
    pub const SMOKE: Purpose = Purpose(13);
    /// Crater depth of a thrown item's burst (keyed by the item id).
    pub const ITEM_CRATER: Purpose = Purpose(14);
    // ---- S3 armour (`docs/slices/S3-armour.md` "RNG keys"). Projectiles
    // have no id and their slots are reused, so the shell purposes are
    // keyed by (tick, slot as the id).
    /// The ±7 % penetration variation of a plate hit (tick, slot).
    pub const PENETRATION: Purpose = Purpose(15);
    /// Spall ray directions (tick, slot); rays by index within the stream.
    pub const SPALL: Purpose = Purpose(16);
    /// APHE filler burst (tick, slot).
    pub const HE_FILLER: Purpose = Purpose(17);
    /// Main-gun spread (tick, vehicle id).
    pub const GUN_SPREAD: Purpose = Purpose(18);
    /// Bog roll and unbog time of a trench crossing (crossing count as the
    /// tick, vehicle id).
    pub const BOG: Purpose = Purpose(19);
    /// Fire start and each put-out roll (tick, vehicle id).
    pub const FIRE: Purpose = Purpose(20);
    /// Ammo-rack detonation (tick, slot).
    pub const AMMO_RACK: Purpose = Purpose(21);
    // ---- S3b (`docs/slices/S3b-balance.md` "Fragments and blasts").
    /// Fragment ray directions of a shell, rocket or tank-HE burst (tick,
    /// the burst's event index as the id); rays by index within the stream.
    pub const FRAGMENT: Purpose = Purpose(22);
    /// Fragment ray directions of a thrown item's burst (tick, item id).
    pub const ITEM_FRAGMENT: Purpose = Purpose(23);
    /// Inner fragment rays of HE that beat a plate (tick, slot).
    pub const HE_INSIDE: Purpose = Purpose(24);
    /// Lead estimation error against a moving target (the tick slot holds
    /// the target id shifted left 8 bits OR the ranging count; the shooter
    /// id as the id).
    pub const LEAD: Purpose = Purpose(25);
    /// Ammo-fire roll of a rack hit that did not detonate (tick, slot).
    pub const AMMO_FIRE: Purpose = Purpose(26);
    /// S3b Suppress order: where on the line a shot aims (tick, shooter;
    /// a vehicle's MGs skip two draws per mount). The main gun draws
    /// nothing: its HE aims at the line's middle.
    pub const SUPPRESS: Purpose = Purpose(27);
    /// S4 single-shot AT item (Panzerfaust): the shot's spread and lead
    /// estimate (tick, firer).
    pub const SINGLE_SHOT: Purpose = Purpose(28);
    /// S5b: a fire mission's aim offset bearing (order tick, squad id).
    pub const MISSION_AIM: Purpose = Purpose(29);
    /// S6: a cook-off burst's crater depth (tick, cook-off id).
    pub const COOKOFF_CRATER: Purpose = Purpose(30);
    /// S6: a cook-off burst's fragment rays (tick, cook-off id).
    pub const COOKOFF_FRAGMENT: Purpose = Purpose(31);
    /// S7: the inner rays of a thrown item going off inside an open box
    /// (tick, item id).
    pub const INSIDE_ITEM: Purpose = Purpose(32);
    /// S8a: the plate rule at an MG shield (per projectile slot).
    pub const SHIELD: Purpose = Purpose(33);
    /// S10a: a Bangalore charge's burst crater (tick, charge id x 8 + k).
    pub const CHARGE_CRATER: Purpose = Purpose(34);
    /// S10a: a Bangalore charge's burst fragments (tick, as above).
    pub const CHARGE_FRAGMENT: Purpose = Purpose(35);
    /// S10b: a sweeper's detection roll (tick, sweeper id; the mine id
    /// mixed into the roll, `sweep::detect_roll`).
    pub const MINE_DETECT: Purpose = Purpose(36);
    /// S10b: a demolition charge's burst crater (tick, charge id x 8;
    /// review R3: its own pair, not the thrown items').
    pub const DEMO_CRATER: Purpose = Purpose(37);
    /// S10b: a demolition charge's burst fragments (tick, as above).
    pub const DEMO_FRAGMENT: Purpose = Purpose(38);
    /// Terrain lattice values; `TERRAIN + octave`.
    pub const TERRAIN: Purpose = Purpose(100);
    /// Match-map lattice values; `MAP_TERRAIN + octave`.
    pub const MAP_TERRAIN: Purpose = Purpose(120);
}

/// SplitMix64 finaliser (Stafford variant 13).
#[inline]
pub const fn mix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// One 64-bit roll for `(world_seed, tick, sim_id, purpose)`.
#[inline]
pub const fn rng(world_seed: u64, tick: u64, sim_id: SimId, purpose: Purpose) -> u64 {
    let h = mix64(world_seed ^ 0x6A09_E667_F3BC_C908);
    let h = mix64(h ^ tick);
    mix64(h ^ (((sim_id.0 as u64) << 32) | purpose.0 as u64))
}

/// Uniform integer in `[0, n)` from a roll (multiply-shift, no modulo bias
/// worth caring about for n < 2^32).
#[inline]
pub const fn below(roll: u64, n: u32) -> u32 {
    (((roll >> 32) * n as u64) >> 32) as u32
}

/// Uniform integer in `[lo, hi]` (inclusive).
#[inline]
pub const fn range_i32(roll: u64, lo: i32, hi: i32) -> i32 {
    if hi <= lo {
        return lo;
    }
    let span = (hi as i64 - lo as i64 + 1) as u64;
    // The offset is < span, so the sum stays in [lo, hi]; add in i64 because
    // the offset alone can exceed i32::MAX when the span is wider than 2^31.
    (lo as i64 + (((roll >> 32) * span) >> 32) as i64) as i32
}

/// Uniform fixed-point value in `[lo, hi)`.
#[inline]
pub const fn range_fx(roll: u64, lo: Fx, hi: Fx) -> Fx {
    if hi.raw() <= lo.raw() {
        return lo;
    }
    let span = (hi.raw() as i64 - lo.raw() as i64) as u64;
    // As in `range_i32`: the offset is < span, so the i64 sum fits in i32.
    Fx::from_raw((lo.raw() as i64 + (((roll >> 32) * span) >> 32) as i64) as i32)
}

/// Several independent rolls for one key: draw `k` is
/// `mix64(rng(key) ^ mix64(k))`.
#[derive(Clone, Debug)]
pub struct RngStream {
    base: u64,
    counter: u64,
}

impl RngStream {
    #[inline]
    pub const fn new(world_seed: u64, tick: u64, sim_id: SimId, purpose: Purpose) -> RngStream {
        RngStream {
            base: rng(world_seed, tick, sim_id, purpose),
            counter: 0,
        }
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let v = mix64(self.base ^ mix64(self.counter));
        self.counter += 1;
        v
    }

    #[inline]
    pub fn range_i32(&mut self, lo: i32, hi: i32) -> i32 {
        range_i32(self.next_u64(), lo, hi)
    }

    #[inline]
    pub fn range_fx(&mut self, lo: Fx, hi: Fx) -> Fx {
        range_fx(self.next_u64(), lo, hi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_wider_than_2_pow_31_stay_in_bounds() {
        for k in 0..2000u64 {
            let roll = mix64(k);
            let v = range_i32(roll, i32::MIN, i32::MAX);
            let _ = v; // full range: any value is in bounds, must not overflow
            let v = range_i32(roll, -2_000_000_000, 2_000_000_000);
            assert!((-2_000_000_000..=2_000_000_000).contains(&v));
            let (lo, hi) = (Fx::from_int(-20_000), Fx::from_int(20_000));
            let f = range_fx(roll, lo, hi);
            assert!(f >= lo && f < hi, "{f:?}");
        }
        // Extremes of the roll.
        assert_eq!(range_i32(0, i32::MIN, i32::MAX), i32::MIN);
        assert_eq!(range_i32(u64::MAX, i32::MIN, i32::MAX), i32::MAX);
        assert_eq!(range_fx(0, Fx::MIN, Fx::MAX), Fx::MIN);
        assert!(range_fx(u64::MAX, Fx::MIN, Fx::MAX) < Fx::MAX);
    }
}
