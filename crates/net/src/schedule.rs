//! Turn timing on the relay clock (S2 "Turns"). One turn is one tick.
//!
//! Turn `T ≥ base_tick` closes at `base_close + span(T - base_tick)`. At
//! `Start`, `base_tick = 0` and `base_close = start_at + period`; every
//! `Resume { at, close_at }` rebases to `(at, close_at)`. Spans are computed
//! from the tick count, never accumulated, so 30 Hz (33,333.3 µs) does not
//! drift by a microsecond per tick.

use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Schedule {
    pub base_tick: u64,
    /// Relay time (µs) at which `base_tick` closes.
    pub base_close: u64,
    pub tick_hz: u32,
    /// Pacing in percent of real time (100 outside relay tests).
    pub speed_pct: u16,
}

impl Schedule {
    /// The schedule a `Start` sets up: turn 0 opens at `start_at`.
    pub fn new(start_at: u64, tick_hz: u32, speed_pct: u16) -> Schedule {
        let mut s = Schedule {
            base_tick: 0,
            base_close: 0,
            tick_hz: tick_hz.max(1),
            speed_pct: speed_pct.max(1),
        };
        s.base_close = start_at.saturating_add(s.span(1));
        s
    }

    /// Relay µs that `n` turns take.
    pub fn span(&self, n: u64) -> u64 {
        let per_s = self.tick_hz as u128 * self.speed_pct as u128;
        (n as u128 * 100_000_000 / per_s).min(u64::MAX as u128) as u64
    }

    /// One turn, rounded down (33,333 µs at 30 Hz).
    pub fn period(&self) -> u64 {
        self.span(1)
    }

    /// When turn `t` closes. Turns before the base are placed one period
    /// apart backwards (only used for display).
    pub fn close_at(&self, t: u64) -> u64 {
        // Saturating: the values come from the relay, which a client does
        // not trust to keep them sane.
        if t >= self.base_tick {
            self.base_close
                .saturating_add(self.span(t - self.base_tick))
        } else {
            self.base_close
                .saturating_sub(self.span(self.base_tick - t))
        }
    }

    /// The turn open at relay time `now`: the first that closes after it.
    pub fn open_at(&self, now: u64) -> u64 {
        if now < self.base_close {
            return self.base_tick;
        }
        let per_s = self.tick_hz as u128 * self.speed_pct as u128;
        let mut k = ((now - self.base_close) as u128 * per_s / 100_000_000) as u64;
        // Integer rounding can put the guess one off either way.
        while k < u64::MAX / 2 && self.close_at(self.base_tick.saturating_add(k + 1)) <= now {
            k += 1;
        }
        while k > 0 && self.close_at(self.base_tick.saturating_add(k)) > now {
            k -= 1;
        }
        self.base_tick.saturating_add(k + 1)
    }

    /// `Resume { at, close_at }`.
    pub fn rebase(&mut self, at: u64, close_at: u64) {
        self.base_tick = at;
        self.base_close = close_at;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closes_and_opens_agree() {
        let s = Schedule::new(1_000_000, 30, 100);
        assert_eq!(s.period(), 33_333);
        assert_eq!(s.close_at(0), 1_033_333);
        // Spans come from the tick count: 20 minutes later the schedule is
        // still within a microsecond of exact.
        assert_eq!(s.close_at(29), 1_999_999);
        assert_eq!(s.close_at(30 * 60 * 20 - 1), 1_000_000 + 1_200_000_000 - 1);
        assert_eq!(s.open_at(0), 0);
        assert_eq!(s.open_at(1_033_332), 0);
        assert_eq!(s.open_at(1_033_333), 1);
        for t in [5u64, 77, 36_000, 1_000_000] {
            assert_eq!(s.open_at(s.close_at(t)), t + 1);
            assert_eq!(s.open_at(s.close_at(t) - 1), t);
        }
        let mut r = s;
        r.rebase(100, 50_000_000);
        assert_eq!(r.open_at(49_999_999), 100);
        assert_eq!(r.close_at(101), 50_033_333);
        let fast = Schedule::new(0, 30, 200);
        assert_eq!(fast.period(), 16_666);
    }
}
