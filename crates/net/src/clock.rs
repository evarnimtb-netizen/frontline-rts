//! Relay clock estimate (S2 "Turns": clock sync).
//!
//! Every `Pong` gives one offset sample, `relay ≈ local + offset`, assuming
//! the path is symmetric. The estimate is the median of the last
//! [`SAMPLES`] samples, so one slow round trip does not move it. The
//! offset in use slews toward that median by at most [`SLEW_US_PER_S`] per
//! second of local time, and steps only when it is more than [`STEP_US`]
//! off: crystal drift of 20–50 ppm (≤ 50 µs/s) is followed smoothly, so the
//! tick schedule never jumps under the presenter.
//!
//! The same samples feed the RTT statistics behind the lobby's input delay
//! suggestion ([`crate::client::suggest_d`]).

use std::collections::VecDeque;

/// Offset samples in the median.
pub const SAMPLES: usize = 8;
/// Largest slew of the offset in use, µs per second of local time.
pub const SLEW_US_PER_S: u64 = 1000;
/// A larger gap between the median and the offset in use is stepped.
pub const STEP_US: i64 = 50_000;
/// RTT samples kept for P99 and jitter.
pub const RTT_SAMPLES: usize = 32;

#[derive(Clone, Debug, Default)]
pub struct ClockSync {
    samples: VecDeque<i64>,
    rtts: VecDeque<u64>,
    /// Offset in use (µs); `None` before the first sample.
    offset: Option<i64>,
    /// Local time the slew last consumed up to.
    slewed_to: u64,
    steps: u32,
}

impl ClockSync {
    pub fn new() -> ClockSync {
        ClockSync::default()
    }

    /// One round trip: `t_send` and `now` on the local clock, `t_relay` on
    /// the relay's. Samples with a negative RTT (a corrupt echo) are dropped.
    pub fn on_pong(&mut self, t_send: u64, t_relay: u64, now: u64) {
        let Some(rtt) = now.checked_sub(t_send) else {
            return;
        };
        // An echo older than 10 s is not a useful sample.
        if rtt > 10_000_000 {
            return;
        }
        // i128: `t_relay` comes off the wire and may be anything.
        let sample = t_relay as i128 + (rtt / 2) as i128 - now as i128;
        let Ok(sample) = i64::try_from(sample) else {
            return;
        };
        if self.samples.len() == SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
        if self.rtts.len() == RTT_SAMPLES {
            self.rtts.pop_front();
        }
        self.rtts.push_back(rtt);
        if self.offset.is_none() {
            self.offset = Some(sample);
            self.slewed_to = now;
        }
        self.update(now);
    }

    /// Median of the recent samples.
    pub fn median(&self) -> Option<i64> {
        if self.samples.is_empty() {
            return None;
        }
        let mut s: Vec<i64> = self.samples.iter().copied().collect();
        s.sort_unstable();
        let n = s.len();
        Some(if n % 2 == 1 {
            s[n / 2]
        } else {
            // Midpoint without overflow.
            s[n / 2 - 1] + ((s[n / 2] as i128 - s[n / 2 - 1] as i128) / 2) as i64
        })
    }

    /// Moves the offset in use toward the median for the local time that
    /// passed since the last call. Call it at least every few ms.
    pub fn update(&mut self, now: u64) {
        let (Some(cur), Some(target)) = (self.offset, self.median()) else {
            return;
        };
        let gap = target.saturating_sub(cur);
        if gap.unsigned_abs() > STEP_US as u64 {
            self.offset = Some(target);
            self.slewed_to = now;
            self.steps += 1;
            return;
        }
        let elapsed = now.saturating_sub(self.slewed_to);
        // Whole µs of slew this call may use; the remainder stays in
        // `slewed_to` so slow polling never loses budget.
        let budget = elapsed * SLEW_US_PER_S / 1_000_000;
        if budget == 0 {
            return;
        }
        self.slewed_to += budget * 1_000_000 / SLEW_US_PER_S;
        let step = gap.clamp(-(budget as i64), budget as i64);
        self.offset = Some(cur.saturating_add(step));
    }

    pub fn offset(&self) -> Option<i64> {
        self.offset
    }

    /// Times the offset was stepped rather than slewed (after the first).
    pub fn steps(&self) -> u32 {
        self.steps
    }

    pub fn samples(&self) -> usize {
        self.samples.len()
    }

    /// Relay time for a local time.
    pub fn to_relay(&self, local: u64) -> Option<u64> {
        self.offset
            .map(|o| (local as i128 + o as i128).clamp(0, u64::MAX as i128) as u64)
    }

    /// Local time for a relay time.
    pub fn to_local(&self, relay: u64) -> Option<u64> {
        self.offset
            .map(|o| (relay as i128 - o as i128).clamp(0, u64::MAX as i128) as u64)
    }

    /// Highest of the recent RTTs (with 32 samples, the max is the P97–P99
    /// estimate that matters for sizing D).
    pub fn rtt_p99(&self) -> Option<u64> {
        self.rtts.iter().copied().max()
    }

    pub fn rtt_last(&self) -> Option<u64> {
        self.rtts.back().copied()
    }

    /// Mean absolute difference of consecutive RTTs (RFC 3550 style).
    pub fn jitter(&self) -> u64 {
        if self.rtts.len() < 2 {
            return 0;
        }
        let sum: u64 = self
            .rtts
            .iter()
            .zip(self.rtts.iter().skip(1))
            .map(|(a, b)| a.abs_diff(*b))
            .sum();
        sum / (self.rtts.len() as u64 - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_sample_sets_offset_then_median_rules() {
        let mut c = ClockSync::new();
        // Relay is 5 s ahead, RTT 20 ms.
        c.on_pong(1_000_000, 6_010_000, 1_020_000);
        assert_eq!(c.offset(), Some(5_000_000));
        // One outlier (late relay stamp) does not move the median far.
        for i in 0..6 {
            let t = 2_000_000 + i * 100_000;
            c.on_pong(t, t + 5_010_000, t + 20_000);
        }
        c.on_pong(3_000_000, 3_000_000 + 5_040_000, 3_020_000);
        assert_eq!(c.median(), Some(5_000_000));
    }

    #[test]
    fn slews_at_most_one_ms_per_second_and_steps_big_gaps() {
        let mut c = ClockSync::new();
        c.on_pong(0, 10_000, 20_000);
        assert_eq!(c.offset(), Some(0));
        // The relay clock now reads 10 ms later: within the step limit, so
        // the offset slews 1 ms per second.
        let mut now = 1_000_000;
        for _ in 0..8 {
            c.on_pong(now - 20_000, now - 10_000 + 10_000, now);
            now += 1_000;
        }
        let before = c.offset().unwrap();
        assert!(before < 2_000, "slewed too fast: {before}");
        c.update(now + 4_000_000);
        let after = c.offset().unwrap();
        assert!(
            (3_900..=6_100).contains(&(after - before)),
            "{before} -> {after}"
        );
        // A 200 ms jump is stepped.
        for _ in 0..8 {
            now += 1_000;
            c.on_pong(now - 20_000, now + 200_000 - 10_000, now);
        }
        assert_eq!(c.offset(), Some(200_000));
        assert!(c.steps() >= 1);
    }

    #[test]
    fn rtt_stats() {
        let mut c = ClockSync::new();
        for (i, rtt) in [10_000u64, 14_000, 12_000, 30_000].iter().enumerate() {
            let t = i as u64 * 1_000_000;
            c.on_pong(t, t, t + rtt);
        }
        assert_eq!(c.rtt_p99(), Some(30_000));
        assert_eq!(c.jitter(), (4_000 + 2_000 + 18_000) / 3);
    }
}
