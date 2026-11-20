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
//! - `rg` (T152; G1-G3): each faction's first line squad halted at
//!   (400, 500) facing the other's at (500, 500), 100 m, for 60 s; the
//!   target squad's own rifle grenades taken away (only the firer's count).
//!   Run 1 the target in the open, run 2 in a dug trench.
//!   `<f>.fired_per_run` (the firer's `s11b.rg_fired`, open run),
//!   `grenades_carried` (the firer's at the start), `open_loss_pct`,
//!   `trench_vs_open_pct` (the target's men lost); control `rifle_grenades`
//!   0 (the kind stripped from every squad type, as the setup does):
//!   `fired_per_run` and `open_loss_pct` (the rifles' share of the losses).
//! - `light` (M2): each faction's light and heavy mortar team fires 8
//!   bombs on the enemy's first line squad in the open 250 m away (the
//!   point marked): `<tube>.vs_heavy_pct` (the target's losses under the
//!   light tube as % of under the heavy one), `rounds_fired` (bursts).
use anyhow::{Result, anyhow};
use frontline_content::ItemKind;
use frontline_sim::{FxVec2, PlayerId, SquadId, World};
use serde::Serialize;

use super::s10b::{fields, pct};
use super::s11a1::{barrage_once, mask_once, mission, order, par_seeds, ratio, run_out, spawn};
use super::{EAST, FortifyArgs, WEST, at, members, world_with};

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

#[derive(Clone, Debug, Default, Serialize)]
pub struct RgControl {
    pub fired_per_run: Option<f64>,
    pub open_loss_pct: Option<f64>,
}

/// One faction's `rg` numbers.
#[derive(Clone, Debug, Default, Serialize)]
pub struct RgSide {
    pub fired_per_run: Option<f64>,
    pub grenades_carried: Option<f64>,
    pub open_loss_pct: Option<f64>,
    pub trench_loss_pct: Option<f64>,
    pub trench_vs_open_pct: Option<f64>,
    pub control: RgControl,
}

/// `fortify10.s11b.rg`.
#[derive(Clone, Debug, Default, Serialize)]
pub struct RgRow {
    pub germany: RgSide,
    pub usa: RgSide,
}

/// One light tube's `light` numbers.
#[derive(Clone, Debug, Default, Serialize)]
pub struct LightTube {
    pub loss_pct: Option<f64>,
    pub heavy_loss_pct: Option<f64>,
    pub vs_heavy_pct: Option<f64>,
    /// Bursts of the light tube's runs.
    pub rounds_fired: Option<f64>,
}

/// `fortify10.s11b.light`.
#[derive(Clone, Debug, Default, Serialize)]
pub struct LightRow {
    pub kz_grw42: LightTube,
    pub m2_60mm: LightTube,
}

/// The S11b rows (`fortify10.s11b`).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S11bReport {
    pub how_mask: HowMaskRow,
    pub how_barrage: HowBarrageRow,
    pub rg: RgRow,
    pub light: LightRow,
}

/// Summary lines, one per row.
pub fn summary_text(r: &S11bReport) -> String {
    let v = serde_json::to_value(r).unwrap_or_default();
    ["how_mask", "how_barrage", "rg", "light"]
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

/// `rg`: the run length (s) and the squads' distance (m).
const RG_S: u64 = 60;
const RG_GAP_M: i32 = 100;

/// Rifle grenades squad `q`'s living men carry.
fn grenades(w: &World, q: SquadId) -> u32 {
    let k = ItemKind::RifleGrenade.index();
    members(w, q)
        .iter()
        .map(|m| w.entities().kit()[m.index()].items[k] as u32)
        .sum()
}

/// Moves player `p`'s start squads but `keep` to a corner.
fn park_others(w: &mut World, p: u8, keep: SquadId) {
    let corner = if p == 0 { at(60, 950) } else { at(964, 950) };
    for (id, q) in w.game().unwrap().squads().clone() {
        if id != keep && q.owner == PlayerId(p) {
            w.place_squad(id, corner, if p == 0 { EAST } else { WEST });
        }
    }
}

/// One `rg` run, player `fp` firing: (grenades fired, carried at the
/// start, target men lost, target men).
fn rg_once(args: &FortifyArgs, seed: u64, fp: u8, trench: bool, on: bool) -> Result<[u32; 4]> {
    let mut w = world_with(args, seed, |s| {
        super::s11a1::flat(s);
        if !on {
            s.rifle_grenades_off();
        }
    })?;
    let tp = 1 - fp;
    let (fx, tx, y) = (400, 400 + RG_GAP_M, 500);
    let firer = super::squad(&w, fp, "line")?;
    let target = super::squad(&w, tp, "line")?;
    park_others(&mut w, fp, firer);
    park_others(&mut w, tp, target);
    if trench {
        w.dig_trench_now(&[at(tx, y + 16), at(tx, y - 16)], -1);
    }
    w.place_squad(firer, at(fx, y), EAST);
    w.place_squad(target, at(tx, y), WEST);
    let k = ItemKind::RifleGrenade.index();
    for m in members(&w, target) {
        w.edit_soldier(m, |kit, _| kit.items[k] = 0);
    }
    let carried = grenades(&w, firer);
    let men = members(&w, target).len() as u32;
    let g = w.game().unwrap();
    let (f0, l0) = (
        g.players()[fp as usize].s11b.rg_fired,
        g.players()[tp as usize].losses,
    );
    let hz = w.match_setup().unwrap().tick_hz as u64;
    for _ in 0..RG_S * hz {
        w.step(&[]);
    }
    let g = w.game().unwrap();
    Ok([
        g.players()[fp as usize].s11b.rg_fired - f0,
        carried,
        g.players()[tp as usize].losses - l0,
        men,
    ])
}

/// The `rg` row (module docs).
pub fn rg_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<RgRow> {
    // [side][open, trench, control open][fired, carried, lost, men]
    let mut t = [[[0u32; 4]; 3]; 2];
    let mut runs = 0u32;
    let all = par_seeds(seeds, |seed| {
        let mut v = [[[0u32; 4]; 3]; 2];
        for fp in 0..2u8 {
            v[fp as usize][0] = rg_once(args, seed, fp, false, true)?;
            v[fp as usize][1] = rg_once(args, seed, fp, true, true)?;
            v[fp as usize][2] = rg_once(args, seed, fp, false, false)?;
        }
        Ok(v)
    })?;
    for v in all {
        runs += 1;
        for (side, kinds) in v.iter().enumerate() {
            for (kind, r) in kinds.iter().enumerate() {
                for (x, y) in t[side][kind].iter_mut().zip(r) {
                    *x += y;
                }
            }
        }
    }
    let side = |p: usize| {
        let per = |v: u32| (runs > 0).then(|| (v as f64 * 10.0 / runs as f64).round() / 10.0);
        let loss = |k: usize| pct(t[p][k][2], t[p][k][3]);
        RgSide {
            fired_per_run: per(t[p][0][0]),
            grenades_carried: per(t[p][0][1]),
            open_loss_pct: loss(0),
            trench_loss_pct: loss(1),
            trench_vs_open_pct: ratio(loss(1), loss(0)),
            control: RgControl {
                fired_per_run: per(t[p][2][0]),
                open_loss_pct: loss(2),
            },
        }
    };
    Ok(RgRow {
        germany: side(0),
        usa: side(1),
    })
}

/// The light and heavy mortar cards, by player (Germany, USA).
const LIGHT: [&str; 2] = ["ww2:ger_kz_grw42_team", "ww2:usa_60mm_mortar_squad"];
const HEAVY: [&str; 2] = ["ww2:ger_mortar_team", "ww2:usa_mortar_team"];
/// `light`: bombs a run and the distance (m).
const LIGHT_ROUNDS: u16 = 8;
const LIGHT_RANGE_M: i32 = 250;

/// One `light` run of player `p`'s tube `card`: (target men lost, men,
/// bursts).
fn light_once(args: &FortifyArgs, seed: u64, p: u8, card: &str) -> Result<[u32; 3]> {
    let mut w = world_with(args, seed, super::s11a1::flat)?;
    let what = format!("s11b.light seed {seed} {card}");
    let tp = 1 - p;
    let (mx, tx, y) = (300, 300 + LIGHT_RANGE_M, 500);
    let target = super::squad(&w, tp, "line")?;
    park_others(&mut w, tp, target);
    w.place_squad(target, at(tx, y), WEST);
    let sq = spawn(&mut w, p, card, at(mx, y), EAST)?;
    let point: FxVec2 = at(tx, y);
    if !w.mark_intel(PlayerId(p), point) {
        anyhow::bail!("{what}: could not mark the target");
    }
    for _ in 0..2 * w.match_setup().unwrap().tick_hz {
        w.step(&[]);
    }
    let men = members(&w, target).len() as u32;
    let l0 = w.game().unwrap().players()[tp as usize].losses;
    w.enable_burst_log();
    if let Some(r) = order(&mut w, p, sq, point, LIGHT_ROUNDS) {
        anyhow::bail!("{what}: refused {r}");
    }
    let id = mission(&w, sq)
        .ok_or_else(|| anyhow!("{what}: no mission"))?
        .id;
    let bursts = run_out(&mut w, sq, id, &what)?.len() as u32;
    let lost = w.game().unwrap().players()[tp as usize].losses - l0;
    Ok([lost, men, bursts])
}

/// The `light` row (module docs).
pub fn light_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<LightRow> {
    let mut t = [[[0u32; 3]; 2]; 2]; // [side][light, heavy][lost, men, bursts]
    let all = par_seeds(seeds, |seed| {
        let mut v = [[[0u32; 3]; 2]; 2];
        for p in 0..2u8 {
            v[p as usize][0] = light_once(args, seed, p, LIGHT[p as usize])?;
            v[p as usize][1] = light_once(args, seed, p, HEAVY[p as usize])?;
        }
        Ok(v)
    })?;
    for v in all {
        for (p, tubes) in v.iter().enumerate() {
            for (k, r) in tubes.iter().enumerate() {
                for (x, y) in t[p][k].iter_mut().zip(r) {
                    *x += y;
                }
            }
        }
    }
    let tube = |p: usize| {
        let loss = |k: usize| pct(t[p][k][0], t[p][k][1]);
        LightTube {
            loss_pct: loss(0),
            heavy_loss_pct: loss(1),
            vs_heavy_pct: ratio(loss(0), loss(1)),
            rounds_fired: (t[p][0][1] > 0).then_some(t[p][0][2] as f64),
        }
    };
    Ok(LightRow {
        kz_grw42: tube(0),
        m2_60mm: tube(1),
    })
}

/// Every S11b row.
pub fn s11b_rows(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<S11bReport> {
    Ok(S11bReport {
        how_mask: how_mask_row(args, seeds.clone())?,
        how_barrage: how_barrage_row(args, seeds.clone())?,
        rg: rg_row(args, seeds.clone())?,
        light: light_row(args, seeds)?,
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
        for (f, g) in [("germany", &r.rg.germany), ("usa", &r.rg.usa)] {
            assert!(g.fired_per_run.unwrap() >= 1.0, "{f} fired");
            assert_eq!(g.control.fired_per_run, Some(0.0), "{f} control");
        }
        for t in [&r.light.kz_grw42, &r.light.m2_60mm] {
            assert!(t.rounds_fired.unwrap() > 0.0);
            assert!(t.heavy_loss_pct.is_some());
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
