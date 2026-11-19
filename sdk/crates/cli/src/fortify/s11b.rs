//! S11b fortify-test rows (docs/slices/S11b.md 4 M2; Build C): three-part
//! rows (value, exposure, control) under `fortify10.s11b`. Every row is
//! driven by player commands; no AI code runs. A gun card arrives
//! emplaced and crewed at its spawn point (S11a1's helpers).
//!
//! - `how_mask` (T150; D 1492): each field howitzer (leFH 18, M2A1) at
//!   (50, 500) ordered onto a point 600 m east, 80 m behind a 25 m crest
//!   band (S11a1's `mask` scene at the design's distance; T129's row keeps
//!   530 m): `lefh_masked_pct`, `m2a1_masked_pct` (refused `masked`),
//!   exposure `orders`; control: the same orders on flat ground.
//! - `how_barrage` (T151; D 1138): each howitzer fires 8 rounds observed by
//!   its faction's observer team on the enemy's first line squad, 500 m
//!   away (S11a1's `barrage` scene; the German gun fires its mirror image):
//!   the squad in the open, then in a dug trench. `<gun>.open_loss_pct`,
//!   `trench_vs_open_pct`, `rounds_fired`; control: the trench dug but the
//!   squad 8 m in front of it, in the open (as S11a1's control).
//! - `rg` (T152; G1-G3) and `light` (M2): Build C chunk 2; null until built.
use anyhow::Result;
use serde::Serialize;

use super::FortifyArgs;
use super::s10b::{fields, pct};
use super::s11a1::{barrage_once, mask_once, par_seeds, ratio};

/// The field howitzer cards, by player (Germany, USA).
pub const HOWS: [&str; 2] = ["ww2:ger_lefh18_squad", "ww2:usa_105mm_m2a1_squad"];
/// `how_mask`: the point's distance from the gun (D 1492).
const HOW_MASK_M: i32 = 600;
/// `how_barrage`: rounds a run.
pub const HOW_BARRAGE_ROUNDS: u16 = 8;

#[derive(Clone, Debug, Default, Serialize)]
pub struct HowMaskControl {
    pub lefh_masked_pct: Option<f64>,
    pub m2a1_masked_pct: Option<f64>,
}

/// `fortify10.s11b.how_mask`.
#[derive(Clone, Debug, Default, Serialize)]
pub struct HowMaskRow {
    pub lefh_masked_pct: Option<f64>,
    pub m2a1_masked_pct: Option<f64>,
    /// Exposure: orders given behind the crest (both guns).
    pub orders: Option<f64>,
    pub control: HowMaskControl,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct HowBarrageControl {
    pub trench_vs_open_pct: Option<f64>,
}

/// One howitzer's `how_barrage` numbers.
#[derive(Clone, Debug, Default, Serialize)]
pub struct HowBarrageGun {
    pub open_loss_pct: Option<f64>,
    pub trench_loss_pct: Option<f64>,
    pub trench_vs_open_pct: Option<f64>,
    /// Bursts of all three runs.
    pub rounds_fired: Option<f64>,
    pub control: HowBarrageControl,
}

/// `fortify10.s11b.how_barrage`.
#[derive(Clone, Debug, Default, Serialize)]
pub struct HowBarrageRow {
    pub lefh: HowBarrageGun,
    pub m2a1: HowBarrageGun,
}

/// The S11b rows (`fortify10.s11b`).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S11bReport {
    pub how_mask: HowMaskRow,
    pub how_barrage: HowBarrageRow,
}

/// Summary lines, one per row.
pub fn summary_text(r: &S11bReport) -> String {
    let v = serde_json::to_value(r).unwrap_or_default();
    ["how_mask", "how_barrage"]
        .iter()
        .map(|k| format!("  S11b {k}: {}\n", fields(&v[*k])))
        .collect()
}

/// The `how_mask` row (module docs).
pub fn how_mask_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<HowMaskRow> {
    let mut n = 0u32;
    let mut v = [[0u32; 2]; 2]; // [crest, flat][lefh, m2a1] refused masked
    let all = par_seeds(seeds, |seed| {
        let mut r = [[false; 2]; 2];
        for (k, crest) in [true, false].into_iter().enumerate() {
            for p in 0..2u8 {
                r[k][p as usize] = mask_once(args, seed, p, HOWS[p as usize], HOW_MASK_M, crest)?.0;
            }
        }
        Ok(r)
    })?;
    for r in all {
        n += 1;
        for k in 0..2 {
            for p in 0..2 {
                v[k][p] += r[k][p] as u32;
            }
        }
    }
    Ok(HowMaskRow {
        lefh_masked_pct: pct(v[0][0], n),
        m2a1_masked_pct: pct(v[0][1], n),
        orders: (n > 0).then_some(2.0 * n as f64),
        control: HowMaskControl {
            lefh_masked_pct: pct(v[1][0], n),
            m2a1_masked_pct: pct(v[1][1], n),
        },
    })
}

/// The `how_barrage` row (module docs).
pub fn how_barrage_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<HowBarrageRow> {
    let mut t = [[[0u32; 4]; 3]; 2]; // [gun][kind][lost, men, down, bursts]
    let all = par_seeds(seeds, |seed| {
        let mut v = Vec::new();
        for p in 0..2u8 {
            for kind in 0..3u8 {
                v.push((
                    p,
                    kind,
                    barrage_once(args, seed, p, HOWS[p as usize], HOW_BARRAGE_ROUNDS, kind)?,
                ));
            }
        }
        Ok(v)
    })?;
    for v in all {
        for (p, kind, (l, m, d, b)) in v {
            let r = &mut t[p as usize][kind as usize];
            r[0] += l;
            r[1] += m;
            r[2] += d;
            r[3] += b;
        }
    }
    let gun = |p: usize| {
        let loss = |k: usize| pct(t[p][k][0], t[p][k][1]);
        HowBarrageGun {
            open_loss_pct: loss(0),
            trench_loss_pct: loss(1),
            trench_vs_open_pct: ratio(loss(1), loss(0)),
            rounds_fired: (t[p][0][1] > 0).then_some((t[p][0][3] + t[p][1][3] + t[p][2][3]) as f64),
            control: HowBarrageControl {
                trench_vs_open_pct: ratio(loss(2), loss(0)),
            },
        }
    };
    Ok(HowBarrageRow {
        lefh: gun(0),
        m2a1: gun(1),
    })
}

/// Every S11b row.
pub fn s11b_rows(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<S11bReport> {
    Ok(S11bReport {
        how_mask: how_mask_row(args, seeds.clone())?,
        how_barrage: how_barrage_row(args, seeds)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// M2 (Build C): each row runs on one seed and its control differs
    /// from its value (the crest and the trench make a difference).
    #[test]
    fn fortify_s11b_rows_run() {
        let r = s11b_rows(&FortifyArgs::default(), 1..2).unwrap();
        eprintln!("{}", serde_json::to_string(&r).unwrap());
        let m = &r.how_mask;
        assert_eq!(m.orders, Some(2.0));
        assert_eq!(m.lefh_masked_pct, Some(100.0), "leFH 18 masked");
        assert_eq!(m.m2a1_masked_pct, Some(100.0), "M2A1 masked");
        assert_eq!(m.control.lefh_masked_pct, Some(0.0), "flat: not masked");
        assert_eq!(m.control.m2a1_masked_pct, Some(0.0), "flat: not masked");
        for g in [&r.how_barrage.lefh, &r.how_barrage.m2a1] {
            assert!(g.rounds_fired.unwrap() > 0.0);
            assert_ne!(g.trench_vs_open_pct, g.control.trench_vs_open_pct);
        }
    }

    /// One-off: the S11b rows on seed 1 (prints the values), so that no
    /// stage waits for the whole `fortify-test`; run by hand in release.
    #[test]
    #[ignore]
    fn s11b_rows_one_off() {
        let t = std::time::Instant::now();
        let r = s11b_rows(&FortifyArgs::default(), 1..2).unwrap();
        eprintln!("{}", serde_json::to_string(&r).unwrap());
        eprintln!("s11b rows wall s: {:.1}", t.elapsed().as_secs_f64());
    }
}
