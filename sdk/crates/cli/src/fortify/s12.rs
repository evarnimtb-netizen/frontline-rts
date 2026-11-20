//! S12 fortify-test rows (docs/slices/S12-wounds-medics.md 8, rows R7 and
//! R8; Build A): the `wounds` scene under `fortify10.s12`. The S12
//! switches are on in the row only (`[wounds] enabled` 1); each row's
//! control is the same scene with `enabled` 0, which plays as the base
//! (`<BASE>`, every S12 switch 0). Every row is driven by player commands;
//! no AI code runs. Rifle grenades are off (small arms only).
//!
//! A man "out" left his squad: dead, or (with wounds on) downed. Wounded
//! men still in the squad are not out.
//!
//! - `trench` (R7, hard): each faction's line squad in a dug trench under
//!   the other's line squad's rifles, 100 m, 300 s. `out_pct` (men out of
//!   men), `dead_pct`, `downed_pct`, `wounded_pct` (wounded at the end),
//!   control `out_pct` with `enabled` 0; `vs_base_pct` = `out_pct` as % of
//!   the control's; `pass` when `vs_base_pct` <= 110. `head_300`: the
//!   same with `head_damage_pct` 300 (a report).
//! - `prone` (R8): each faction's line squad prone in the open under the
//!   other's MG (its roster's `mg` card, called in), 100 m, 300 s. The same numbers; R8 holds when
//!   `vs_base_pct` <= 100 (men die no faster than at the base).
use anyhow::Result;
use frontline_content::MatchSetup;
use frontline_sim::command::CommandKind;
use frontline_sim::{PlayerId, SquadId, World};
use serde::Serialize;

use super::s10b::{fields, pct};
use super::s11a1::{par_seeds, ratio};
use super::{EAST, FortifyArgs, WEST, at, members, step_with, world_with};

/// The run length (s).
const RUN_S: u64 = 300;
/// `trench`: the squads' distance (m).
const TRENCH_GAP_M: i32 = 100;
/// `prone`: the MG's distance (m).
const PRONE_GAP_M: i32 = 100;
/// R7: the trench's losses with wounds on, at most this % of the base's.
pub const R7_MAX_PCT: f64 = 110.0;

/// One variant's numbers.
#[derive(Clone, Debug, Default, Serialize)]
pub struct WoundsRun {
    pub out_pct: Option<f64>,
    pub dead_pct: Option<f64>,
    pub downed_pct: Option<f64>,
    pub wounded_pct: Option<f64>,
}

/// `fortify10.s12.trench` (R7) and `.prone` (R8).
#[derive(Clone, Debug, Default, Serialize)]
pub struct WoundsRow {
    pub out_pct: Option<f64>,
    pub dead_pct: Option<f64>,
    pub downed_pct: Option<f64>,
    pub wounded_pct: Option<f64>,
    /// `out_pct` as % of the control's.
    pub vs_base_pct: Option<f64>,
    /// Exposure: men under fire (both factions, every seed).
    pub men: Option<f64>,
    pub pass: Option<bool>,
    /// `enabled` 0: the base.
    pub control: WoundsRun,
    /// `trench` only: `head_damage_pct` 300 (a report).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_300: Option<WoundsRun>,
}

/// The S12 rows (`fortify10.s12`).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S12Report {
    pub trench: WoundsRow,
    pub prone: WoundsRow,
}

/// Summary lines, one per row.
pub fn summary_text(r: &S12Report) -> String {
    let v = serde_json::to_value(r).unwrap_or_default();
    ["trench", "prone"]
        .iter()
        .map(|k| format!("  S12 {k}: {}\n", fields(&v[*k])))
        .collect()
}

/// Moves player `p`'s squads but `keep` to a corner.
fn park_others(w: &mut World, p: u8, keep: SquadId) {
    let corner = if p == 0 { at(60, 950) } else { at(964, 950) };
    for (id, q) in w.game().unwrap().squads().clone() {
        if id != keep && q.owner == PlayerId(p) {
            w.place_squad(id, corner, if p == 0 { EAST } else { WEST });
        }
    }
}

/// The wounds variants: base (`enabled` 0), on, on with head 300 %.
#[derive(Clone, Copy, PartialEq)]
pub enum Variant {
    Base,
    On,
    Head300,
}

fn edit(v: Variant) -> impl FnOnce(&mut MatchSetup) {
    move |s: &mut MatchSetup| {
        super::s11a1::flat(s);
        s.rifle_grenades_off();
        let w = &mut s.rules.wounds;
        w.enabled = (v != Variant::Base) as u32;
        if v == Variant::Head300 {
            w.head_damage_pct = 300;
        }
    }
}

/// The scene of one run, target player `tp` (the other fires), set up
/// and not yet run: (world, target squad). `prone`: R8's scene, else R7's.
pub fn scene(
    args: &FortifyArgs,
    seed: u64,
    tp: u8,
    prone: bool,
    v: Variant,
) -> Result<(World, SquadId)> {
    let mut w = world_with(args, seed, edit(v))?;
    let fp = 1 - tp;
    let y = 500;
    let tx = 400;
    let target = super::squad(&w, tp, "line")?;
    park_others(&mut w, tp, target);
    if prone {
        let q = super::call_in(&mut w, fp, "mg")?;
        park_others(&mut w, fp, q);
        w.place_squad(q, at(tx + PRONE_GAP_M, y), WEST);
    } else {
        let q = super::squad(&w, fp, "line")?;
        park_others(&mut w, fp, q);
        w.dig_trench_now(&[at(tx, y + 16), at(tx, y - 16)], -1);
        w.place_squad(q, at(tx + TRENCH_GAP_M, y), WEST);
    }
    w.place_squad(target, at(tx, y), EAST);
    if prone {
        let men = members(&w, target);
        step_with(
            &mut w,
            tp,
            vec![CommandKind::SetStance {
                units: men,
                stance: frontline_sim::StanceOrder::Prone,
            }],
        );
    }
    Ok((w, target))
}

/// One run: [out, dead, downed, wounded, men] of the target squad.
fn once(args: &FortifyArgs, seed: u64, tp: u8, prone: bool, v: Variant) -> Result<[u32; 5]> {
    let (mut w, target) = scene(args, seed, tp, prone, v)?;
    let men = members(&w, target).len() as u32;
    let g = w.game().unwrap();
    let pl = &g.players()[tp as usize];
    let (l0, d0) = (pl.losses, pl.s12.downed);
    let hz = w.match_setup().unwrap().tick_hz as u64;
    for _ in 0..RUN_S * hz {
        w.step(&[]);
    }
    let left = members(&w, target);
    let wounded = left
        .iter()
        .filter(|m| w.entities().combat()[m.index()].wound.state == frontline_sim::wounds::WOUNDED)
        .count() as u32;
    let pl = &w.game().unwrap().players()[tp as usize];
    Ok([
        men - left.len() as u32,
        pl.losses - l0,
        pl.s12.downed - d0,
        wounded,
        men,
    ])
}

fn run_of(t: &[u32; 5]) -> WoundsRun {
    WoundsRun {
        out_pct: pct(t[0], t[4]),
        dead_pct: pct(t[1], t[4]),
        downed_pct: pct(t[2], t[4]),
        wounded_pct: pct(t[3], t[4]),
    }
}

/// One row (module docs): `prone` R8, else R7.
pub fn row(args: &FortifyArgs, seeds: std::ops::Range<u64>, prone: bool) -> Result<WoundsRow> {
    let variants: &[Variant] = if prone {
        &[Variant::On, Variant::Base]
    } else {
        &[Variant::On, Variant::Base, Variant::Head300]
    };
    let mut t = [[0u32; 5]; 3];
    let all = par_seeds(seeds, |seed| {
        let mut v = [[0u32; 5]; 3];
        for (k, &var) in variants.iter().enumerate() {
            for tp in 0..2u8 {
                let r = once(args, seed, tp, prone, var)?;
                for (x, y) in v[k].iter_mut().zip(r) {
                    *x += y;
                }
            }
        }
        Ok(v)
    })?;
    for v in all {
        for (a, b) in t.iter_mut().zip(v) {
            for (x, y) in a.iter_mut().zip(b) {
                *x += y;
            }
        }
    }
    let on = run_of(&t[0]);
    let control = run_of(&t[1]);
    let vs = ratio(on.out_pct, control.out_pct).or(
        // No base losses: on passes only with none either.
        (t[1][4] > 0 && t[1][0] == 0).then_some(if t[0][0] == 0 { 100.0 } else { f64::INFINITY }),
    );
    let max = if prone { 100.0 } else { R7_MAX_PCT };
    Ok(WoundsRow {
        out_pct: on.out_pct,
        dead_pct: on.dead_pct,
        downed_pct: on.downed_pct,
        wounded_pct: on.wounded_pct,
        vs_base_pct: vs,
        men: (t[0][4] > 0).then_some(t[0][4] as f64),
        pass: vs.map(|v| v <= max),
        control,
        head_300: (!prone).then(|| run_of(&t[2])),
    })
}

/// Every S12 row.
pub fn s12_rows(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<S12Report> {
    Ok(S12Report {
        trench: row(args, seeds.clone(), false)?,
        prone: row(args, seeds, true)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R7/R8 (Build A): each row runs on one seed, men are hit, and with
    /// wounds on some are wounded or down where the control (the base) has
    /// none. (The R7 and R8 verdicts are read off the fortify10 set, not
    /// one seed.)
    #[test]
    fn fortify_s12_rows_run() {
        let r = s12_rows(&FortifyArgs::default(), 1..2).unwrap();
        eprintln!("{}", serde_json::to_string(&r).unwrap());
        for (k, row) in [("trench", &r.trench), ("prone", &r.prone)] {
            assert!(row.men.unwrap() > 0.0, "{k}");
            assert_eq!(
                row.control.downed_pct,
                Some(0.0),
                "{k}: none down at the base"
            );
            assert_eq!(
                row.control.wounded_pct,
                Some(0.0),
                "{k}: none wounded at the base"
            );
            assert!(
                row.downed_pct.unwrap() + row.wounded_pct.unwrap() > 0.0,
                "{k}: wounds on"
            );
        }
        assert!(
            r.trench.control.out_pct.unwrap() > 0.0,
            "the base loses men"
        );
        assert!(r.trench.head_300.is_some() && r.prone.head_300.is_none());
    }

    /// The scene with wounds on plays the same at 1, 2, 3 and 8 threads
    /// (state hashes equal every 30 ticks for 90 s). Control: another seed differs.
    #[test]
    fn s12_wounds_scene_hash_is_thread_count_free() {
        let hashes = |threads: usize, seed: u64| {
            let args = FortifyArgs {
                threads,
                ..FortifyArgs::default()
            };
            let (mut w, _) = scene(&args, seed, 1, false, Variant::On).unwrap();
            let mut v = Vec::new();
            for _ in 0..90 {
                for _ in 0..30 {
                    w.step(&[]);
                }
                v.push(w.state_hash());
            }
            let hits: u32 = w
                .game()
                .unwrap()
                .players()
                .iter()
                .map(|p| {
                    let s = &p.s12;
                    s.torso_hits
                        .iter()
                        .chain(&s.head_hits)
                        .chain(&s.leg_hits)
                        .sum::<u32>()
                })
                .sum();
            (v, hits)
        };
        let (one, hits) = hashes(1, 1);
        assert!(hits > 0, "wounds came into play");
        for n in [2, 3, 8] {
            assert_eq!(hashes(n, 1).0, one, "{n} threads");
        }
        assert_ne!(hashes(1, 2).0, one, "control: another seed");
    }

    /// One-off: the S12 rows on seeds 1..=`S12_SEEDS` (default 1; prints
    /// the values); run by hand in release.
    #[test]
    #[ignore]
    fn s12_rows_one_off() {
        let t = std::time::Instant::now();
        let n: u64 = std::env::var("S12_SEEDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);
        let r = s12_rows(&FortifyArgs::default(), 1..n + 1).unwrap();
        eprintln!("{}", serde_json::to_string(&r).unwrap());
        eprintln!("s12 rows wall s: {:.1}", t.elapsed().as_secs_f64());
    }
}
