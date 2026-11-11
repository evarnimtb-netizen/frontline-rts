//! S10b fortify-test rows (docs/slices/S10b-obstacles.md 7): AT and AP
//! mines, sweeping, obstacles, demolition and the bunker. Report rows; the
//! targets T102-T107 read them (`fortify10.s10b.*`), `pass` ignores them.
//!
//! Build A lays out every field as `null`; the stage that gives a mechanic
//! its behaviour fills its row (C: `atmine`, `apmine`, `sweep`; D:
//! `obstacle`, `demolish`; E: `bunker`).

//!
//! * **AT mine** (Build B). For each mine-owning player, the other
//!   faction's medium tank (Pz IV H against a US field, M4A3 against a
//!   German one) drives east [`AT_RUN_M`] m on open ground. Direct: one
//!   mine under its left track's path (`direct_immobilised_pct`). Field:
//!   three AT rows of [`FIELD_ROW_M`] m across the path, [`AT_ROW_GAP_M`] m
//!   apart, mines every 4 m, row k shifted by 4k/3 m; the tank's lateral
//!   offset is `(seed * 37) % 400` cm (the field moves: a driven tank keeps
//!   to its path-node line) (`field_immobilised_pct`,
//!   `field_knocked_out_pct`); then the tank side's rifle squad walks the
//!   same field (`infantry_triggered`: mines set off). The geometric
//!   coverage estimate (share of lateral offsets that put a mine within
//!   `trigger_cm` of a track centreline) goes to the summary beside it.
//! * **AP mine** (Build B). A rifleman walks past one AP mine at 50 cm
//!   (`near_miss_triggered`: mines set off) and at 20 cm
//!   (`contact_triggered_pct`); the medium tank drives over a two-row AP
//!   field (`vehicle_triggered`); a rifle squad crosses a two-row AP field
//!   of 30 m, mines every 2 m (`crossing_losses_pct`).

use anyhow::{Result, bail};
use frontline_sim::{CommandKind, Fx, FxVec2, PlayerId, SimId, World};
use serde::Serialize;

use super::s10a::{alive, fort_of};
use super::{EAST, FortifyArgs, LINE_Y, TRENCH_X, at, members, pos, squad, step_with, world};

/// Metres a tank drives (the field's middle half-way).
pub const AT_RUN_M: i32 = 120;
/// A field row's length across the path.
pub const FIELD_ROW_M: i32 = 30;
/// Gap between AT rows.
pub const AT_ROW_GAP_M: i32 = 4;
/// AT mines along a row (m).
const AT_STEP_CM: i32 = 400;
/// AP rows: gap and mine step.
const AP_ROW_GAP_M: i32 = 4;
const AP_STEP_CM: i32 = 200;
const MEDIUM_TANKS: [&str; 2] = ["ger_pz4_h_squad", "usa_m4a3_75_squad"];

/// T102: a tank driven over one AT mine, and through an AT field.
#[derive(Clone, Debug, Default, Serialize)]
pub struct AtMineRow {
    pub direct_immobilised_pct: Option<f64>,
    pub field_immobilised_pct: Option<f64>,
    pub field_knocked_out_pct: Option<f64>,
    pub infantry_triggered: Option<f64>,
    /// Report beside `field_immobilised_pct`: the geometric share of
    /// lateral offsets that put a mine under a track (mean of both tanks).
    pub coverage_est_pct: Option<f64>,
}

/// T103: AP mines go off on contact only.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ApMineRow {
    pub near_miss_triggered: Option<f64>,
    pub contact_triggered_pct: Option<f64>,
    pub vehicle_triggered: Option<f64>,
    pub crossing_losses_pct: Option<f64>,
}

/// T104: a sweep against the expected find rate.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SweepRow {
    pub found_vs_expected_pts: Option<f64>,
    pub lifted_of_found_pct: Option<f64>,
    pub found_pct: Option<f64>,
    pub sweeper_losses: Option<f64>,
    pub bangalore_cleared_pct: Option<f64>,
}

/// T105: obstacles block vehicles, not infantry.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ObstacleRow {
    pub hedgehog_crossed_pct: Option<f64>,
    pub log_crossed_pct: Option<f64>,
    pub infantry_speed_pct: Option<f64>,
    pub tank_reached_pct: Option<f64>,
}

/// T106: demolition opens an obstacle line.
#[derive(Clone, Debug, Default, Serialize)]
pub struct DemolishRow {
    pub destroyed_pct: Option<f64>,
    pub gap_crossed_pct: Option<f64>,
    pub charge_laid_s: Option<f64>,
}

/// T107h: the open-pit control of the bunker's MG row.
#[derive(Clone, Debug, Default, Serialize)]
pub struct BunkerControl {
    pub mg_losses_pct: Option<f64>,
}

/// T107: the bunker against named weapons.
#[derive(Clone, Debug, Default, Serialize)]
pub struct BunkerRow {
    pub small_arms_hp_lost: Option<f64>,
    pub survived_81mm_x3_pct: Option<f64>,
    pub sheltered_losses: Option<f64>,
    pub slit_satchel_losses_pct: Option<f64>,
    pub roof_satchel_losses: Option<f64>,
    pub ap_shots_to_break: Option<f64>,
    pub mg_losses_pct: Option<f64>,
    pub control: BunkerControl,
}

/// The six S10b rows (`fortify10.s10b`).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S10bReport {
    pub atmine: AtMineRow,
    pub apmine: ApMineRow,
    pub sweep: SweepRow,
    pub obstacle: ObstacleRow,
    pub demolish: DemolishRow,
    pub bunker: BunkerRow,
}

/// One row's fields as `name value` pairs (`-` for a row not yet run).
fn fields(v: &serde_json::Value) -> String {
    let Some(m) = v.as_object() else {
        return String::new();
    };
    m.iter()
        .map(|(k, x)| match x {
            serde_json::Value::Object(_) => format!("{k} ({})", fields(x)),
            serde_json::Value::Number(n) => format!("{k} {n}"),
            _ => format!("{k} -"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Summary lines, one per row.
pub fn summary_text(r: &S10bReport) -> String {
    let v = serde_json::to_value(r).unwrap_or_default();
    let mut out = String::new();
    for row in [
        "atmine", "apmine", "sweep", "obstacle", "demolish", "bunker",
    ] {
        out += &format!("  S10b {row}: {}\n", fields(&v[row]));
    }
    out
}

fn cm(x_cm: i32, y_cm: i32) -> FxVec2 {
    FxVec2::new(Fx::from_milli(x_cm * 10), Fx::from_milli(y_cm * 10))
}

fn set_off(w: &World, p: u8) -> u32 {
    w.game().unwrap().fort().accounts[p as usize].mines_set_off
}

/// Mines `owner` lays of blueprint `id` (staged, no work).
fn lay(w: &mut World, id: &str, owner: u8, pts: &[FxVec2]) -> Result<()> {
    let f = fort_of(w, id)?;
    for &p in pts {
        if w.lay_mine_now(f, p, PlayerId(owner)).is_none() {
            bail!("mine at {p:?} was not staged");
        }
    }
    Ok(())
}

/// Rows across an eastward path (x in m from `x0`, every `gap` m): `rows`
/// rows of `FIELD_ROW_M`, mines every `step_cm`, row k shifted by
/// `k * shift_cm`, centred on `LINE_Y`.
fn field(x0: i32, rows: i32, gap: i32, step_cm: i32, shift_cm: i32) -> Vec<FxVec2> {
    let mut out = Vec::new();
    for k in 0..rows {
        let n = FIELD_ROW_M * 100 / step_cm;
        for j in 0..=n {
            let y = (LINE_Y - FIELD_ROW_M / 2) * 100 + j * step_cm + k * shift_cm;
            out.push(cm((x0 + k * gap) * 100, y));
        }
    }
    out
}

/// The lateral offsets (cm, hull +y) of the tank card's track centrelines.
fn track_ys(w: &World, card: &str) -> Vec<i32> {
    let s = w.match_setup().unwrap();
    let Some(ty) = s.squad_types.iter().find(|t| t.id.ends_with(card)) else {
        return Vec::new();
    };
    let Some(v) = ty.vehicle.and_then(|v| s.vehicles.get(v as usize)) else {
        return Vec::new();
    };
    v.components
        .iter()
        .filter(|c| c.kind.is_track())
        .filter_map(|c| c.anchors.first())
        .map(|a| (Fx::from(a.pos.y).mul_int(100)).round_int())
        .collect()
}

/// The medium tank of player `p`, placed at `start` facing east and sent
/// `AT_RUN_M` east; steps until it is immobilised, knocked out or there
/// (or 90 s). (immobilised, knocked out).
fn drive(w: &mut World, p: u8, start: FxVec2) -> Result<(bool, bool)> {
    let s = w.match_setup().unwrap();
    let Some(ty) = s.squad_type_index(&format!("ww2:{}", MEDIUM_TANKS[p as usize])) else {
        bail!("no {} card", MEDIUM_TANKS[p as usize]);
    };
    let Some(tank) = w.spawn_squad_at(PlayerId(p), ty, start, EAST) else {
        bail!("the tank was not spawned at {start:?}");
    };
    let v = members(w, tank)[0];
    let target = start + FxVec2::from_int(AT_RUN_M, 0);
    step_with(w, p, vec![CommandKind::Move { squad: tank, target }]);
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let state = |w: &World| {
        let g = w.game().unwrap();
        g.vehicle(v).map_or((false, true), |vs| {
            let def = &w.match_setup().unwrap().vehicles[vs.type_idx as usize];
            (vs.immobilised(def), vs.wreck_tick.is_some())
        })
    };
    for _ in 0..90 * hz {
        let (imm, ko) = state(w);
        if imm || ko || pos(w, v).x >= target.x - Fx::from_int(3) {
            break;
        }
        w.step(&[]);
    }
    // Let a burst of this tick land.
    for _ in 0..hz {
        w.step(&[]);
    }
    Ok(state(w))
}

/// A rifleman of `p`'s line squad (the rest sent far off) walks east past
/// a spot `off_cm` beside him; whether the AP mine there went off.
fn pass_mine(args: &FortifyArgs, seed: u64, owner: u8, off_cm: i32) -> Result<bool> {
    let mut w = world(args, seed)?;
    let p = 1 - owner;
    let sq = squad(&w, p, "line")?;
    w.place_squad(sq, at(TRENCH_X, LINE_Y), EAST);
    let men = members(&w, sq);
    let m = men[0];
    let start = pos(&w, m);
    // Others step 30 m south of his line first.
    let rest: Vec<SimId> = men[1..].to_vec();
    step_with(
        &mut w,
        p,
        vec![CommandKind::MoveUnits {
            units: rest,
            target: start - FxVec2::from_int(0, 30),
        }],
    );
    let mine = start + FxVec2::new(Fx::from_int(15), Fx::from_milli(off_cm * 10));
    let f = fort_of(&w, "ww2:ap_minefield")?;
    if w.lay_mine_now(f, mine, PlayerId(owner)).is_none() {
        bail!("seed {seed}: the AP mine was not staged");
    }
    let to = start + FxVec2::from_int(30, 0);
    step_with(
        &mut w,
        p,
        vec![CommandKind::MoveUnits {
            units: vec![m],
            target: to,
        }],
    );
    let hz = w.match_setup().unwrap().tick_hz as u64;
    for _ in 0..40 * hz {
        if !alive(&w, m) || pos(&w, m).x >= to.x - Fx::ONE {
            break;
        }
        w.step(&[]);
    }
    Ok(set_off(&w, owner) > 0)
}

/// A line squad of `p` placed west of the field and moved `AT_RUN_M` east;
/// (losses, men).
fn cross(w: &mut World, p: u8, start: FxVec2) -> Result<(u32, u32)> {
    let sq = squad(w, p, "line")?;
    w.place_squad(sq, start, EAST);
    let men = members(w, sq);
    let target = start + FxVec2::from_int(AT_RUN_M / 2, 0);
    step_with(w, p, vec![CommandKind::Move { squad: sq, target }]);
    let hz = w.match_setup().unwrap().tick_hz as u64;
    for _ in 0..120 * hz {
        let g = w.game().unwrap();
        let there = g.squad(sq).is_none_or(|q| {
            q.members
                .iter()
                .all(|&m| pos(w, m).x >= target.x - Fx::from_int(5))
        });
        if there {
            break;
        }
        w.step(&[]);
    }
    for _ in 0..hz {
        w.step(&[]);
    }
    let lost = men.iter().filter(|&&m| !alive(w, m)).count() as u32;
    Ok((lost, men.len() as u32))
}

fn pct(a: u32, b: u32) -> Option<f64> {
    (b > 0).then(|| (a as f64 * 1000.0 / b as f64).round() / 10.0)
}

/// Share (%) of lateral offsets over one period of the staggered AT field
/// that put a mine within `trigger_cm` of a track centreline at `ys`.
pub fn at_coverage_pct(ys: &[i32], trigger_cm: i32) -> f64 {
    // Three rows shifted by a third: mines every 4/3 m across the path,
    // which is `AT_STEP_CM` in third-centimetres.
    let period = AT_STEP_CM;
    let mut hit = 0;
    for k in 0..period {
        let covered = ys.iter().any(|&y| {
            let rel = (k + y * 3).rem_euclid(period);
            let d = rel.min(period - rel);
            d <= trigger_cm * 3
        });
        hit += covered as i32;
    }
    (hit as f64 * 1000.0 / period as f64).round() / 10.0
}

/// The `atmine` and `apmine` rows over `seeds` (each player owning the
/// mines in turn). Also returns the coverage estimates (owner 0, 1).
pub fn mine_rows(
    args: &FortifyArgs,
    seeds: std::ops::Range<u64>,
) -> Result<(AtMineRow, ApMineRow)> {
    let fx = TRENCH_X - AT_RUN_M / 2;
    let (mut direct, mut direct_n) = (0u32, 0u32);
    let (mut fimm, mut fko, mut fn_) = (0u32, 0u32, 0u32);
    let mut inf_set = 0u32;
    let (mut near, mut contact, mut ap_n) = (0u32, 0u32, 0u32);
    let mut veh_set = 0u32;
    let (mut lost, mut men) = (0u32, 0u32);
    let mut cover = vec![0.0; 2];
    for seed in seeds {
        for owner in [0u8, 1] {
            let t = 1 - owner;
            // Direct: one mine under the left track's path.
            let mut w = world(args, seed)?;
            let ys = track_ys(&w, MEDIUM_TANKS[t as usize]);
            let left = ys.iter().copied().max().unwrap_or(0);
            let trig = w
                .match_setup()
                .unwrap()
                .items
                .iter()
                .find(|i| i.id == "ww2:at_mine")
                .and_then(|i| i.mine.as_ref())
                .map_or(0, |m| m.trigger_cm as i32);
            cover[owner as usize] = at_coverage_pct(&ys, trig);
            lay(&mut w, "ww2:at_minefield", owner, &[cm(TRENCH_X * 100, LINE_Y * 100 + left)])?;
            let (imm, _) = drive(&mut w, t, at(fx, LINE_Y))?;
            direct += imm as u32;
            direct_n += 1;
            // Field.
            let mut w = world(args, seed)?;
            // The tank keeps to its path-node line (y = LINE_Y): the field
            // moves by the lateral offset instead.
            let off = FxVec2::new(Fx::ZERO, Fx::from_milli(((seed * 37) % 400) as i32 * 10));
            let mines: Vec<FxVec2> =
                field(TRENCH_X - AT_ROW_GAP_M, 3, AT_ROW_GAP_M, AT_STEP_CM, AT_STEP_CM / 3)
                    .into_iter()
                    .map(|p| p - off)
                    .collect();
            lay(&mut w, "ww2:at_minefield", owner, &mines)?;
            let (imm, ko) = drive(&mut w, t, at(fx, LINE_Y))?;
            fimm += imm as u32;
            fko += ko as u32;
            fn_ += 1;
            let before = set_off(&w, owner);
            cross(&mut w, t, at(fx, LINE_Y - 8))?;
            inf_set += set_off(&w, owner) - before;
            // AP: near miss and contact.
            near += pass_mine(args, seed, owner, 50)? as u32;
            contact += pass_mine(args, seed, owner, 20)? as u32;
            ap_n += 1;
            // AP field: a tank, then a fresh world and a rifle squad.
            let ap = field(TRENCH_X - AP_ROW_GAP_M / 2, 2, AP_ROW_GAP_M, AP_STEP_CM, AP_STEP_CM / 2);
            let mut w = world(args, seed)?;
            let moved: Vec<FxVec2> = ap.iter().map(|&p| p - off).collect();
            lay(&mut w, "ww2:ap_minefield", owner, &moved)?;
            drive(&mut w, t, at(fx, LINE_Y))?;
            veh_set += set_off(&w, owner);
            let mut w = world(args, seed)?;
            lay(&mut w, "ww2:ap_minefield", owner, &ap)?;
            let (l, m) = cross(&mut w, t, at(fx + AT_RUN_M / 4, LINE_Y))?;
            lost += l;
            men += m;
        }
    }
    Ok((
        AtMineRow {
            direct_immobilised_pct: pct(direct, direct_n),
            field_immobilised_pct: pct(fimm, fn_),
            field_knocked_out_pct: pct(fko, fn_),
            infantry_triggered: Some(inf_set as f64),
            coverage_est_pct: Some((cover[0] + cover[1]) / 2.0),
        },
        ApMineRow {
            near_miss_triggered: Some(near as f64),
            contact_triggered_pct: pct(contact, ap_n),
            vehicle_triggered: Some(veh_set as f64),
            crossing_losses_pct: pct(lost, men),
        },
    ))
}
