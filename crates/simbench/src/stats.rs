//! Timing summaries.

use std::time::Duration;

use serde::Serialize;

/// Summary of a sample of durations, in milliseconds.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Dist {
    pub n: usize,
    pub mean_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
}

impl Dist {
    pub fn from_durations(samples: &[Duration]) -> Dist {
        let mut ms: Vec<f64> = samples.iter().map(|d| d.as_secs_f64() * 1e3).collect();
        if ms.is_empty() {
            return Dist::default();
        }
        ms.sort_by(f64::total_cmp);
        let mean = ms.iter().sum::<f64>() / ms.len() as f64;
        Dist {
            n: ms.len(),
            mean_ms: mean,
            p50_ms: percentile_sorted(&ms, 50.0),
            p95_ms: percentile_sorted(&ms, 95.0),
            p99_ms: percentile_sorted(&ms, 99.0),
            max_ms: *ms.last().expect("non-empty"),
        }
    }
}

/// Nearest-rank percentile of an ascending slice.
pub fn percentile_sorted(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

/// Median of a sample (sorts a copy).
pub fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    percentile_sorted(&s, 50.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank() {
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(percentile_sorted(&v, 95.0), 95.0);
        assert_eq!(percentile_sorted(&v, 99.0), 99.0);
        assert_eq!(percentile_sorted(&v, 100.0), 100.0);
        assert_eq!(percentile_sorted(&[3.0], 50.0), 3.0);
        assert_eq!(median(&[3.0, 1.0, 2.0]), 2.0);
    }
}
