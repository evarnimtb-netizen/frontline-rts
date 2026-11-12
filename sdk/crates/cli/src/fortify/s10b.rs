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
//! * **Sweep** (Build C). Each player's field engineers sweep a
//!   [`SWEEP_LANE_M`] m lane with ten enemy AP mines at 0, 15, ... 135 cm
//!   from the centreline (`found_pct`, `expected_pct`,
//!   `found_vs_expected_pts`, `lifted_of_found_pct`, `sweeper_losses`); each
//!   player's assault squad lays a Bangalore into a two-row AP field
//!   (`bangalore_cleared_pct`: mines within 150 cm of a burst removed).
//! * **Obstacle** (Build D). The other player's 30 m hedgehog line (one
//!   every 3 m) and, separately, a 20 m log barrier stand across x =
//!   `TRENCH_X`; each player's medium tank starts 30 m west and is ordered
//!   60 m east of the line (`hedgehog_crossed_pct`, `log_crossed_pct`: the
//!   hull crossed the line between its ends; `tank_reached_pct`); the line
//!   squad walks across (`infantry_speed_pct`: the leader's mean speed
//!   within 3 m of the line over his speed elsewhere, lowest run).
//! * **Demolish** (Build D). Each player's field engineers demolish the
//!   middle hedgehog of the other's line (`destroyed_pct`, `charge_laid_s`
//!   from the order); then the medium tank is ordered through
//!   (`gap_crossed_pct`).

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
    /// Report beside `found_pct`: mean over mines of `1 - (1 - p)^n`
    /// (a miss's own n, a find's full-pass n from the lane geometry).
    pub expected_pct: Option<f64>,
    /// Sweepers' men lost, all runs together.
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
    /// Runs whose tank came within [`APPROACH_M`] of the line (review R5:
    /// without it a tank that never moved reads as blocked).
    pub approached_pct: Option<f64>,
    /// The same tank with no line staged: runs it drove through the band
    /// the line would have held (must be 100, or the crossing check is
    /// blind).
    pub control_crossed_pct: Option<f64>,
}

/// T106: demolition opens an obstacle line.
#[derive(Clone, Debug, Default, Serialize)]
pub struct DemolishRow {
    pub destroyed_pct: Option<f64>,
    pub gap_crossed_pct: Option<f64>,
    pub charge_laid_s: Option<f64>,
    /// Runs whose tank came within [`APPROACH_M`] of the line after the
    /// charge (review R5).
    pub approached_pct: Option<f64>,
}

/// T107h: the open-pit control of the bunker's MG row.
#[derive(Clone, Debug, Default, Serialize)]
pub struct BunkerControl {
    pub mg_losses_pct: Option<f64>,
    /// T107a's control: the same staged volleys with armour 1 on the walls
    /// (must lose hp, or the volleys never reached the bunker; review R6).
    pub small_arms_hp_lost: Option<f64>,
    /// T107g/h's control: the five on open ground, no pit (must lose men,
    /// or the MG never engaged that spot; review R6).
    pub open_mg_losses_pct: Option<f64>,
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
    let mut cover = [0.0; 2];
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
            lay(
                &mut w,
                "ww2:at_minefield",
                owner,
                &[cm(TRENCH_X * 100, LINE_Y * 100 + left)],
            )?;
            let (imm, _) = drive(&mut w, t, at(fx, LINE_Y))?;
            direct += imm as u32;
            direct_n += 1;
            // Field.
            let mut w = world(args, seed)?;
            // The tank keeps to its path-node line (y = LINE_Y): the field
            // moves by the lateral offset instead.
            let off = FxVec2::new(Fx::ZERO, Fx::from_milli(((seed * 37) % 400) as i32 * 10));
            let mines: Vec<FxVec2> = field(
                TRENCH_X - AT_ROW_GAP_M,
                3,
                AT_ROW_GAP_M,
                AT_STEP_CM,
                AT_STEP_CM / 3,
            )
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
            let ap = field(
                TRENCH_X - AP_ROW_GAP_M / 2,
                2,
                AP_ROW_GAP_M,
                AP_STEP_CM,
                AP_STEP_CM / 2,
            );
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

/// T104 lane: metres long, mines, their spacing along it and lateral step.
pub const SWEEP_LANE_M: i32 = 30;
const SWEEP_MINES: i32 = 10;
const SWEEP_FIRST_CM: i32 = 300;
const SWEEP_STEP_CM: i32 = 250;
const SWEEP_OFFSET_CM: i32 = 15;

/// Sweeper-ticks a mine at lateral offset `o` m would be within `r` m of a
/// sweeper on a full pass: the sweepers walk +-1 m from the centreline at
/// `step` m per tick; one whose line passes within `trig` m of the mine
/// steps on it, so his count ends at the trigger circle.
pub fn full_pass_ticks(o: f64, r: f64, trig: f64, step: f64) -> f64 {
    [o - 1.0, o + 1.0]
        .iter()
        .map(|l| {
            let l = l.abs();
            if l >= r || step <= 0.0 {
                0.0
            } else if l < trig {
                ((r * r - l * l).sqrt() - (trig * trig - l * l).sqrt()) / step
            } else {
                2.0 * (r * r - l * l).sqrt() / step
            }
        })
        .sum::<f64>()
        .floor()
}

/// One T104 sweep: player `p`'s field engineers sweep a [`SWEEP_LANE_M`] m
/// lane east along `LINE_Y` with ten enemy AP mines. Returns (found,
/// expected finds, lifted, sweepers lost, mines).
fn sweep_once(args: &FortifyArgs, seed: u64, p: u8) -> Result<(u32, f64, u32, u32, u32)> {
    let mut w = world(args, seed)?;
    let eng = super::s10a::call_in_card(&mut w, p, &super::s10a::FIELD_ENGINEERS)?;
    let a = at(TRENCH_X - SWEEP_LANE_M / 2, LINE_Y);
    let b = a + FxVec2::from_int(SWEEP_LANE_M, 0);
    w.place_squad(eng, a - FxVec2::from_int(3, 0), EAST);
    let owner = 1 - p;
    let f = fort_of(&w, "ww2:ap_minefield")?;
    let mut mines = Vec::new();
    for k in 0..SWEEP_MINES {
        let pt = a + cm(SWEEP_FIRST_CM + k * SWEEP_STEP_CM, k * SWEEP_OFFSET_CM);
        let Some(id) = w.lay_mine_now(f, pt, PlayerId(owner)) else {
            bail!("seed {seed}: sweep mine {k} was not staged");
        };
        mines.push(id);
    }
    let setup = w.match_setup().unwrap();
    let hz = setup.tick_hz as u64;
    let t = setup
        .rules
        .fortify
        .clone()
        .unwrap_or_else(|| unreachable!("[fortify]"));
    let pr = t.detect_pct_per_s as f64 / (100.0 * hz as f64);
    let r = t.detect_cm as f64 / 100.0;
    let trig = setup
        .items
        .iter()
        .find(|i| i.id == "ww2:ap_mine")
        .and_then(|i| i.mine.as_ref())
        .map_or(0.0, |m| m.trigger_cm as f64 / 100.0);
    let ty = w
        .game()
        .unwrap()
        .squad(eng)
        .map(|q| q.type_idx)
        .unwrap_or(0);
    let speed = Fx::from(setup.squad_types[ty as usize].speed).to_f32() as f64;
    let step = speed * t.sweep_speed_pct as f64 / 100.0 / hz as f64;
    let men = members(&w, eng);
    step_with(
        &mut w,
        p,
        vec![CommandKind::ClearMines { squad: eng, a, b }],
    );
    let bit = 1u8 << p;
    // Per mine: found, sweeper-ticks in range while unknown, gone unknown.
    let mut rec = vec![(false, 0u32, false); mines.len()];
    for _ in 0..600 * hz {
        let before: Vec<Option<u8>> = mines
            .iter()
            .map(|id| w.game().unwrap().fort().mines.get(id).map(|m| m.known))
            .collect();
        w.step(&[]);
        let near: Vec<FxVec2> = men
            .iter()
            .filter(|&&m| alive(&w, m))
            .map(|&m| pos(&w, m))
            .collect();
        for (k, id) in mines.iter().enumerate() {
            let Some(known) = before[k] else { continue };
            if known & bit != 0 {
                continue;
            }
            match w.game().unwrap().fort().mines.get(id) {
                Some(m) => {
                    let d2 = Fx::from_milli(t.detect_cm as i32 * 10);
                    rec[k].1 += near.iter().filter(|&&q| (q - m.pos).length() <= d2).count() as u32;
                    if m.known & bit != 0 {
                        rec[k].0 = true;
                    }
                }
                None => rec[k].2 = true,
            }
        }
        let sweeping = w
            .game()
            .unwrap()
            .squad(eng)
            .is_some_and(|q| matches!(q.order, frontline_sim::Order::ClearMines { .. }));
        if !sweeping {
            break;
        }
    }
    let mut expected = 0.0;
    for (k, &(found, n, _)) in rec.iter().enumerate() {
        // A miss ran its full pass: its own count; a find cut the count
        // short, so its full pass comes from the lane geometry.
        let o = (k as i32 * SWEEP_OFFSET_CM) as f64 / 100.0;
        let n = if found {
            full_pass_ticks(o, r, trig, step)
        } else {
            n as f64
        };
        expected += 1.0 - (1.0 - pr).powf(n);
    }
    let found = rec.iter().filter(|x| x.0).count() as u32;
    let lifted = w.game().unwrap().fort().accounts[p as usize].mines_lifted;
    let lost = men.iter().filter(|&&m| !alive(&w, m)).count() as u32;
    Ok((found, expected, lifted, lost, mines.len() as u32))
}

/// T104e: player `p`'s assault squad lays a Bangalore east toward
/// `LINE_Y`; once the charge lies (its 8 s fuse running), a two-row enemy AP
/// field is staged across it at 1 m and 2 m from its near end (mines every
/// 2 m, the second row staggered by 1 m). (Mines within 150 cm of a burst,
/// of them removed.) The field is staged after the lay so that the walk to
/// the charge's spot sets none off: the row measures the burst only.
fn bangalore_once(args: &FortifyArgs, seed: u64, p: u8) -> Result<(u32, u32)> {
    let mut w = world(args, seed)?;
    let eng = super::s10a::call_in_card(&mut w, p, &super::s10a::ASSAULT_CARDS)?;
    w.place_squad(eng, at(TRENCH_X - 8, LINE_Y), EAST);
    let units = members(&w, eng);
    step_with(
        &mut w,
        p,
        vec![CommandKind::PlaceBangalore {
            units,
            target: at(TRENCH_X, LINE_Y),
        }],
    );
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let mut charge = None;
    for _ in 0..60 * hz {
        w.step(&[]);
        if let Some(c) = w.game().unwrap().fort().charges.first() {
            charge = Some((c.from, c.to));
            break;
        }
    }
    let Some((from, to)) = charge else {
        bail!("seed {seed}: no Bangalore was laid");
    };
    let along = (to - from).normalize_or_zero();
    let across = along.perp();
    let mut pts = Vec::new();
    for k in -4..=4 {
        pts.push(from + along + across * Fx::from_int(2 * k));
        pts.push(from + along.mul_int(2) + across * Fx::from_int(2 * k + 1));
    }
    lay(&mut w, "ww2:ap_minefield", 1 - p, &pts)?;
    for _ in 0..12 * hz {
        w.step(&[]);
        if w.game().unwrap().fort().charges.is_empty() {
            break;
        }
    }
    for _ in 0..3 * hz {
        w.step(&[]);
    }
    let reach = Fx::from_milli(1500);
    let near: Vec<FxVec2> = pts
        .iter()
        .copied()
        .filter(|q| {
            (0..3).any(|k| {
                (*q - (from + (to - from).mul_int(2 * k + 1).div_int(6))).length() <= reach
            })
        })
        .collect();
    let standing: Vec<FxVec2> = w
        .game()
        .unwrap()
        .fort()
        .mines
        .values()
        .map(|m| m.pos)
        .collect();
    let gone = near.iter().filter(|q| !standing.contains(q)).count() as u32;
    Ok((near.len() as u32, gone))
}

/// The `sweep` row over `seeds` (each player sweeping in turn).
pub fn sweep_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<SweepRow> {
    let (mut found, mut expected, mut lifted, mut lost, mut n) = (0u32, 0.0, 0u32, 0u32, 0u32);
    let (mut near, mut gone) = (0u32, 0u32);
    for seed in seeds {
        for p in [0u8, 1] {
            let (f, e, l, x, m) = sweep_once(args, seed, p)?;
            found += f;
            expected += e;
            lifted += l;
            lost += x;
            n += m;
            let (a, g) = bangalore_once(args, seed, p)?;
            near += a;
            gone += g;
        }
    }
    let found_pct = pct(found, n);
    let expected_pct = (n > 0).then(|| (expected * 1000.0 / n as f64).round() / 10.0);
    Ok(SweepRow {
        found_vs_expected_pts: found_pct
            .zip(expected_pct)
            .map(|(f, e)| ((f - e).abs() * 10.0).round() / 10.0),
        lifted_of_found_pct: pct(lifted, found),
        found_pct,
        expected_pct,
        sweeper_losses: Some(lost as f64),
        bangalore_cleared_pct: pct(gone, near),
    })
}

// ---- Build D: obstacles and demolition (T105, T106).

/// Hedgehogs in a line (3 m apart, 30 m) at `TRENCH_X` across the tank's
/// route; the log barrier's length.
const HEDGEHOGS: i32 = 11;
const HEDGEHOG_STEP_M: i32 = 3;
const LOG_M: i32 = 20;
/// The tank starts this far west of the line and is ordered this far east.
const OBST_WEST_M: i32 = 30;
const OBST_EAST_M: i32 = 60;
/// A tank whose hull centre came this near the line's x reached it (half a
/// medium tank's length plus the block pad and some slack).
pub const APPROACH_M: i32 = 8;

/// Whether `track` came within [`APPROACH_M`] of x = `TRENCH_X`.
fn approached(track: &[FxVec2]) -> bool {
    let x = Fx::from_int(TRENCH_X);
    track
        .iter()
        .any(|q| (q.x - x).abs() <= Fx::from_int(APPROACH_M))
}

/// Stages `owner`'s obstacle line across x = `TRENCH_X` (hedgehogs, or a
/// log barrier); the line's (y0, y1); the hedgehogs' piece ids.
fn obstacle_line(w: &mut World, owner: u8, log: bool) -> Result<(i32, i32, Vec<u32>)> {
    let before: Vec<u32> = w.game().unwrap().fort().pieces.keys().copied().collect();
    if log {
        let f = fort_of(w, "ww2:log_barrier")?;
        let (a, b) = (
            at(TRENCH_X, LINE_Y - LOG_M / 2),
            at(TRENCH_X, LINE_Y + LOG_M / 2),
        );
        if !w.build_now(f, &[a, b], PlayerId(owner)) {
            bail!("the log barrier was not staged");
        }
        return Ok((LINE_Y - LOG_M / 2, LINE_Y + LOG_M / 2, Vec::new()));
    }
    let f = fort_of(w, "ww2:hedgehog")?;
    let half = (HEDGEHOGS - 1) * HEDGEHOG_STEP_M / 2;
    for k in 0..HEDGEHOGS {
        let p = at(TRENCH_X, LINE_Y - half + k * HEDGEHOG_STEP_M);
        if !w.build_now(f, &[p, p + EAST], PlayerId(owner)) {
            bail!("hedgehog {k} was not staged");
        }
    }
    let ids = w
        .game()
        .unwrap()
        .fort()
        .pieces
        .keys()
        .copied()
        .filter(|id| !before.contains(id))
        .collect();
    Ok((LINE_Y - half, LINE_Y + half, ids))
}

/// Whether the path `track` crosses x = `x` between y0 and y1 (m).
fn crosses(track: &[FxVec2], x: i32, y0: i32, y1: i32) -> bool {
    let x = Fx::from_int(x);
    track.windows(2).any(|s| {
        let (a, b) = (s[0], s[1]);
        if (a.x < x) == (b.x < x) || a.x == b.x {
            return false;
        }
        let y = a.y + (b.y - a.y) * ((x - a.x) / (b.x - a.x));
        y >= Fx::from_int(y0) && y <= Fx::from_int(y1)
    })
}

/// `p`'s medium tank from `OBST_WEST_M` west of the line ordered
/// `OBST_EAST_M` east of it (120 s at most): (hull track, reached).
fn tank_through(w: &mut World, p: u8) -> Result<(Vec<FxVec2>, bool)> {
    let s = w.match_setup().unwrap();
    let Some(ty) = s.squad_type_index(&format!("ww2:{}", MEDIUM_TANKS[p as usize])) else {
        bail!("no {} card", MEDIUM_TANKS[p as usize]);
    };
    let start = at(TRENCH_X - OBST_WEST_M, LINE_Y);
    let Some(tank) = w.spawn_squad_at(PlayerId(p), ty, start, EAST) else {
        bail!("the tank was not spawned at {start:?}");
    };
    let v = members(w, tank)[0];
    let target = at(TRENCH_X + OBST_EAST_M, LINE_Y);
    step_with(
        w,
        p,
        vec![CommandKind::Move {
            squad: tank,
            target,
        }],
    );
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let mut track = vec![pos(w, v)];
    let mut reached = false;
    // A vehicle is never `alive` (that is a soldier check): the run stops
    // on the wreck instead (review R5 found every run ending at tick 1).
    let wrecked = |w: &World| {
        w.game()
            .unwrap()
            .vehicle(v)
            .is_none_or(|vs| vs.wreck_tick.is_some())
    };
    for _ in 0..120 * hz {
        w.step(&[]);
        if wrecked(w) {
            break;
        }
        track.push(pos(w, v));
        if (pos(w, v) - target).length() <= Fx::from_int(5) {
            reached = true;
            break;
        }
    }
    Ok((track, reached))
}

/// `p`'s line squad walks from 30 m west of the line to 30 m east: the
/// leader's mean speed inside the line's band (x within 3 m of the line,
/// y between its ends) over his speed outside it, in percent.
fn infantry_band_pct(w: &mut World, p: u8, y0: i32, y1: i32) -> Result<Option<f64>> {
    let sq = squad(w, p, "line")?;
    w.place_squad(sq, at(TRENCH_X - 30, LINE_Y), EAST);
    let lead = members(w, sq)[0];
    let target = at(TRENCH_X + 30, LINE_Y);
    step_with(w, p, vec![CommandKind::Move { squad: sq, target }]);
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let (mut din, mut tin, mut dout, mut tout) = (0.0f64, 0u32, 0.0f64, 0u32);
    let band = |q: FxVec2| {
        (q.x - Fx::from_int(TRENCH_X)).abs() <= Fx::from_int(3)
            && q.y >= Fx::from_int(y0)
            && q.y <= Fx::from_int(y1)
    };
    let mut last = pos(w, lead);
    for _ in 0..90 * hz {
        w.step(&[]);
        if !alive(w, lead) {
            break;
        }
        let now = pos(w, lead);
        // Walking only: the first and last 8 m (starting, slowing) left out.
        let walking = now.x >= Fx::from_int(TRENCH_X - 22) && now.x <= target.x - Fx::from_int(8);
        let d = (now - last).length().to_f32() as f64;
        if walking && d > 0.0 {
            if band(now) {
                din += d;
                tin += 1;
            } else {
                dout += d;
                tout += 1;
            }
        }
        last = now;
        if now.x >= target.x - Fx::from_int(8) {
            break;
        }
    }
    Ok((tin > 0 && tout > 0 && dout > 0.0)
        .then(|| ((din / tin as f64) / (dout / tout as f64) * 1000.0).round() / 10.0))
}

/// The `obstacle` row (T105) over `seeds`, each player's tank and rifles
/// against the other's lines.
pub fn obstacle_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<ObstacleRow> {
    let (mut hx, mut lx, mut reached, mut runs) = (0u32, 0u32, 0u32, 0u32);
    let (mut near, mut ctl, mut ctl_runs) = (0u32, 0u32, 0u32);
    let mut speed: Option<f64> = None;
    for seed in seeds {
        for p in [0u8, 1] {
            for log in [false, true] {
                let mut w = world(args, seed)?;
                let (y0, y1, _) = obstacle_line(&mut w, 1 - p, log)?;
                let (track, got) = tank_through(&mut w, p)?;
                let crossed = crosses(&track, TRENCH_X, y0, y1) as u32;
                if log {
                    lx += crossed;
                } else {
                    hx += crossed;
                }
                reached += got as u32;
                near += approached(&track) as u32;
                runs += 1;
                let mut w = world(args, seed)?;
                let (y0, y1, _) = obstacle_line(&mut w, 1 - p, log)?;
                if let Some(s) = infantry_band_pct(&mut w, p, y0, y1)? {
                    speed = Some(speed.map_or(s, |m| m.min(s)));
                }
            }
            // Control: no line; the same tank must drive through the band.
            let mut w = world(args, seed)?;
            let half = (HEDGEHOGS - 1) * HEDGEHOG_STEP_M / 2;
            let (track, _) = tank_through(&mut w, p)?;
            ctl += crosses(&track, TRENCH_X, LINE_Y - half, LINE_Y + half) as u32;
            ctl_runs += 1;
        }
    }
    Ok(ObstacleRow {
        hedgehog_crossed_pct: pct(hx, runs / 2),
        log_crossed_pct: pct(lx, runs / 2),
        infantry_speed_pct: speed,
        tank_reached_pct: pct(reached, runs),
        approached_pct: pct(near, runs),
        control_crossed_pct: pct(ctl, ctl_runs),
    })
}

/// `p`'s field engineers demolish the middle hedgehog of the other
/// player's line; then `p`'s tank is ordered through. (destroyed, ticks
/// from the order to the charge, hz, crossed between the line's ends).
fn demolish_once(
    args: &FortifyArgs,
    seed: u64,
    p: u8,
) -> Result<(bool, Option<u64>, u64, bool, bool)> {
    let mut w = world(args, seed)?;
    let (y0, y1, ids) = obstacle_line(&mut w, 1 - p, false)?;
    let mid = ids[ids.len() / 2];
    let eng = super::s10a::call_in_card(&mut w, p, &super::s10a::FIELD_ENGINEERS)?;
    w.place_squad(eng, at(TRENCH_X - 15, LINE_Y + 2), EAST);
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let t0 = w.tick();
    step_with(
        &mut w,
        p,
        vec![CommandKind::Demolish {
            squad: eng,
            at: at(TRENCH_X, LINE_Y),
        }],
    );
    let mut laid = None;
    for _ in 0..120 * hz {
        if !w.game().unwrap().fort().charges.is_empty() {
            laid = Some(w.tick() - t0);
            break;
        }
        w.step(&[]);
    }
    let fuse = w
        .match_setup()
        .unwrap()
        .rules
        .fortify
        .as_ref()
        .map_or(10, |t| t.demolish_fuse_s as u64);
    for _ in 0..(fuse + 2) * hz {
        w.step(&[]);
    }
    let destroyed = !w.game().unwrap().fort().pieces.contains_key(&mid);
    // The engineers step back west, out of the tank's way.
    step_with(
        &mut w,
        p,
        vec![CommandKind::Move {
            squad: eng,
            target: at(TRENCH_X - 20, LINE_Y + 20),
        }],
    );
    let (track, _) = tank_through(&mut w, p)?;
    Ok((
        destroyed,
        laid,
        hz,
        crosses(&track, TRENCH_X, y0, y1),
        approached(&track),
    ))
}

/// The `demolish` row (T106) over `seeds`, each player demolishing in turn.
pub fn demolish_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<DemolishRow> {
    let (mut gone, mut crossed, mut near, mut n) = (0u32, 0u32, 0u32, 0u32);
    let (mut laid_s, mut laid_n) = (0.0f64, 0u32);
    for seed in seeds {
        for p in [0u8, 1] {
            let (d, laid, hz, x, a) = demolish_once(args, seed, p)?;
            gone += d as u32;
            near += a as u32;
            crossed += x as u32;
            n += 1;
            if let Some(t) = laid {
                laid_s += t as f64 / hz as f64;
                laid_n += 1;
            }
        }
    }
    Ok(DemolishRow {
        destroyed_pct: pct(gone, n),
        gap_crossed_pct: pct(crossed, n),
        charge_laid_s: (laid_n > 0).then(|| (laid_s / laid_n as f64 * 10.0).round() / 10.0),
        approached_pct: pct(near, n),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obstacle_row_shows_the_tank_reaching_the_line() {
        // Review R5. Control: drop the Move order in `tank_through` (the
        // tank never leaves its start) -> the crossed pcts still read 0
        // (T105a/b pass) but `approached_pct` and `control_crossed_pct`
        // read 0 and this test fails.
        let r = obstacle_row(&FortifyArgs::default(), 1..2).unwrap();
        eprintln!("{r:?}");
        assert_eq!(r.approached_pct, Some(100.0), "every tank came to the line");
        assert_eq!(r.control_crossed_pct, Some(100.0), "with no line it drives through");
        assert_eq!(r.hedgehog_crossed_pct, Some(0.0));
        assert_eq!(r.log_crossed_pct, Some(0.0));
    }
}
