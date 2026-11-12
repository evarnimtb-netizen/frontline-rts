//! S10b-ai fortify-test rows (docs/slices/S10b-ai.md 7; three-part rows:
//! value, exposure, control). Report rows; the targets T116 read them
//! (`fortify10.s10b_ai.*`), `pass` ignores them.
//!
//! * **Paths** (Build B, R11, R12). For each player `p` and seed: the other
//!   player's two-row AP field (30 m rows, mines every 2 m) stands across
//!   the line, every mine known to `p`; `p`'s line squad is ordered 60 m
//!   east straight through it (`foot_losses`: `p`'s men lost to mines,
//!   `foot_mine_losses_pct` of the men). Then the other player's three-row
//!   AT field (as the S10b field row), known to `p`, and `p`'s medium tank
//!   ordered `AT_RUN_M` east through it (`tank_immobilised_pct`). Exposure:
//!   `routes_through_field_pct` (the straight order line comes within the
//!   band of a mine: 100 by staging) and `priced_pct` (runs whose leader or
//!   tank left the straight line by more than [`PRICED_OFF_M`] m). Control
//!   (`control.*`): the same runs with `[fortify] mine_paths` 0.

use anyhow::{Result, bail};
use frontline_sim::{CommandKind, Fx, FxVec2, PlayerId, SimId, World};
use serde::Serialize;

use super::s10b::{
    AP_ROW_GAP_M, AP_STEP_CM, AT_ROW_GAP_M, AT_RUN_M, AT_STEP_CM, MEDIUM_TANKS, cross, field,
    fields, lay_ids, pct,
};
use super::{EAST, FortifyArgs, LINE_Y, TRENCH_X, at, members, pos, step_with, world_with};

/// A route "priced" (left the straight line) past this many metres.
pub const PRICED_OFF_M: i32 = 5;

/// The control half of a three-part row: the same runs with the rule off.
#[derive(Clone, Debug, Default, Serialize)]
pub struct PathsControl {
    pub foot_mine_losses_pct: Option<f64>,
    pub tank_immobilised_pct: Option<f64>,
}

/// T116a/b: paths round known enemy mines.
#[derive(Clone, Debug, Default, Serialize)]
pub struct PathsRow {
    pub foot_losses: Option<f64>,
    pub foot_mine_losses_pct: Option<f64>,
    pub tank_immobilised_pct: Option<f64>,
    /// Exposure: runs whose straight order line crosses the field.
    pub routes_through_field_pct: Option<f64>,
    /// Exposure: runs whose route left the straight line by > 5 m.
    pub priced_pct: Option<f64>,
    pub control: PathsControl,
}

/// The S10b-ai rows (`fortify10.s10b_ai`).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S10bAiReport {
    pub paths: PathsRow,
}

/// The rows in summary order (Build B chunk 2 adds `mines`).
const ROWS: &[&str] = &["paths"];

/// Summary lines, one per row.
pub fn summary_text(r: &S10bAiReport) -> String {
    let v = serde_json::to_value(r).unwrap_or_default();
    let mut out = String::new();
    for row in ROWS {
        out += &format!("  S10b-ai {row}: {}\n", fields(&v[row]));
    }
    out
}

/// Marks mines `ids` known to player `p`.
fn know(w: &mut World, ids: &[u32], p: u8) {
    w.edit_fort(|f| {
        for id in ids {
            if let Some(m) = f.mines.get_mut(id) {
                m.known |= 1 << p;
            }
        }
    });
}

/// Whether the straight line `a -> b` (along y) comes within `band_cm` of
/// a mine at `pts`.
fn line_through(pts: &[FxVec2], a: FxVec2, b: FxVec2, band_cm: u32) -> bool {
    let r = Fx::from_milli(band_cm as i32 * 10);
    pts.iter()
        .any(|m| m.x >= a.x.min(b.x) - r && m.x <= a.x.max(b.x) + r && (m.y - a.y).abs() <= r)
}

/// The greatest distance (m) of `track` from y = `y`.
fn off(track: &[FxVec2], y: i32) -> i32 {
    track
        .iter()
        .map(|p| (p.y - Fx::from_int(y)).abs().floor_int())
        .max()
        .unwrap_or(0)
}

/// `p`'s medium tank placed at `start` facing east and ordered `run_m`
/// east; steps until immobilised, wrecked or there (or 120 s). (track,
/// immobilised).
fn tank_run(w: &mut World, p: u8, start: FxVec2, run_m: i32) -> Result<(Vec<FxVec2>, bool)> {
    let s = w.match_setup().unwrap();
    let Some(ty) = s.squad_type_index(&format!("ww2:{}", MEDIUM_TANKS[p as usize])) else {
        bail!("no {} card", MEDIUM_TANKS[p as usize]);
    };
    let Some(tank) = w.spawn_squad_at(PlayerId(p), ty, start, EAST) else {
        bail!("the tank was not spawned at {start:?}");
    };
    let v = members(w, tank)[0];
    let target = start + FxVec2::from_int(run_m, 0);
    step_with(
        w,
        p,
        vec![CommandKind::Move {
            squad: tank,
            target,
        }],
    );
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let state = |w: &World| {
        let g = w.game().unwrap();
        g.vehicle(v).map_or((false, true), |vs| {
            let def = &w.match_setup().unwrap().vehicles[vs.type_idx as usize];
            (vs.immobilised(def), vs.wreck_tick.is_some())
        })
    };
    let mut track = vec![pos(w, v)];
    for _ in 0..120 * hz {
        let (imm, ko) = state(w);
        if imm || ko || (pos(w, v) - target).length() <= Fx::from_int(3) {
            break;
        }
        w.step(&[]);
        track.push(pos(w, v));
    }
    for _ in 0..hz {
        w.step(&[]);
    }
    Ok((track, state(w).0))
}

/// One player's foot and tank run, `mine_paths` set: (foot men lost to
/// mines, men, foot priced, foot through, tank immobilised, tank priced,
/// tank through).
fn paths_once(args: &FortifyArgs, seed: u64, p: u8, mine_paths: u32) -> Result<[u32; 7]> {
    let owner = 1 - p;
    let edit = |s: &mut frontline_content::MatchSetup| {
        if let Some(f) = s.rules.fortify.as_mut() {
            f.mine_paths = mine_paths;
        }
    };
    let fx = TRENCH_X - AT_RUN_M / 2;
    // Foot: the S10b AP field, the squad 60 m east through it.
    let mut w = world_with(args, seed, edit)?;
    let bands = w
        .match_setup()
        .unwrap()
        .rules
        .fortify
        .as_ref()
        .map_or((300, 400), |f| {
            (f.mine_band_foot_cm, f.mine_band_vehicle_cm)
        });
    let ap = field(
        TRENCH_X - AP_ROW_GAP_M / 2,
        2,
        AP_ROW_GAP_M,
        AP_STEP_CM,
        AP_STEP_CM / 2,
    );
    let ids = lay_ids(&mut w, "ww2:ap_minefield", owner, &ap)?;
    know(&mut w, &ids, p);
    let start = at(fx + AT_RUN_M / 4, LINE_Y);
    let run = AT_RUN_M / 2;
    let through = line_through(&ap, start, start + FxVec2::from_int(run, 0), bands.0) as u32;
    let mut lead: Vec<FxVec2> = Vec::new();
    let before = w.game().unwrap().fort().accounts[p as usize].men_lost_to_mines;
    let (_, men) = cross(&mut w, p, start, run, |w, men: &[SimId]| {
        if let Some(&m) = men.iter().find(|&&m| super::s10a::alive(w, m)) {
            lead.push(pos(w, m));
        }
    })?;
    let lost = w.game().unwrap().fort().accounts[p as usize].men_lost_to_mines - before;
    let foot_priced = (off(&lead, LINE_Y) > PRICED_OFF_M) as u32;
    // Tank: the S10b three-row AT field, moved by the seed's offset.
    let mut w = world_with(args, seed, edit)?;
    let shift = FxVec2::new(Fx::ZERO, Fx::from_milli(((seed * 37) % 400) as i32 * 10));
    let at_f: Vec<FxVec2> = field(
        TRENCH_X - AT_ROW_GAP_M,
        3,
        AT_ROW_GAP_M,
        AT_STEP_CM,
        AT_STEP_CM / 3,
    )
    .into_iter()
    .map(|q| q - shift)
    .collect();
    let ids = lay_ids(&mut w, "ww2:at_minefield", owner, &at_f)?;
    know(&mut w, &ids, p);
    let start = at(fx, LINE_Y);
    let t_through =
        line_through(&at_f, start, start + FxVec2::from_int(AT_RUN_M, 0), bands.1) as u32;
    let (track, imm) = tank_run(&mut w, p, start, AT_RUN_M)?;
    let tank_priced = (off(&track, LINE_Y) > PRICED_OFF_M) as u32;
    Ok([
        lost,
        men,
        foot_priced,
        through,
        imm as u32,
        tank_priced,
        t_through,
    ])
}

/// The paths row (module docs).
pub fn paths_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<PathsRow> {
    let mut on = [0u32; 7];
    let mut ctl = [0u32; 7];
    let mut runs = 0u32;
    for seed in seeds {
        for p in [0u8, 1] {
            for (k, v) in paths_once(args, seed, p, 1)?.into_iter().enumerate() {
                on[k] += v;
            }
            for (k, v) in paths_once(args, seed, p, 0)?.into_iter().enumerate() {
                ctl[k] += v;
            }
            runs += 1;
        }
    }
    Ok(PathsRow {
        foot_losses: Some(on[0] as f64),
        foot_mine_losses_pct: pct(on[0], on[1]),
        tank_immobilised_pct: pct(on[4], runs),
        routes_through_field_pct: pct(on[3] + on[6], 2 * runs),
        priced_pct: pct(on[2] + on[5], 2 * runs),
        control: PathsControl {
            foot_mine_losses_pct: pct(ctl[0], ctl[1]),
            tank_immobilised_pct: pct(ctl[4], runs),
        },
    })
}

/// Every S10b-ai row.
pub fn s10b_ai_rows(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<S10bAiReport> {
    Ok(S10bAiReport {
        paths: paths_row(args, seeds)?,
    })
}
