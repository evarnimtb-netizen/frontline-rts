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
//! * **Mines** (Build B, R1-R4). For each player `p` and seed: `p` holds
//!   the capture point nearest its base, its AI trench dug across the way
//!   to the enemy base (`ai.dig_offset`, `ai.dig_length`), S10a works and
//!   wire off; an engineer squad of `p` carrying [`MINES_CARRY_KG`] kg at
//!   the point. `World::drive_ai_habit(Fort)` drives the S10b fort habit
//!   alone until `p` has an AT field standing (>= [`AT_FIELD_MINES`] AT
//!   mines; at most [`MINES_DRIVE_S`]): `field_laid_pct`. Then the enemy's
//!   medium tank, placed [`MINES_TANK_M`] m out on the straight line from
//!   its base to the point (moved 0-4 m across it by the seed, the AT mine
//!   step), is ordered onto the point (unknown mines are
//!   not priced) for at most [`MINES_TANK_S`] s: `tank_stopped_pct`
//!   (immobilised or wrecked). Exposure `tank_reached_field_pct`: runs in
//!   which the tank came within 5 m of one of `p`'s AT mines
//!   (`FortAccount::at_exposure_ticks` > 0); `builder_alive_pct` (the
//!   engineers lived through the drive). Control (`control.*`): the
//!   same with `[ai.obstacles] mines` 0 (nothing laid, the tank drives on).
//! * **Obstacle** (Build C, R5, R6). For each player `p` and seed: `p` holds
//!   its nearest point (as the mines row, mines off); `p`'s emplaced AT gun
//!   stands [`GUN_OUT_M`] m in front of the point and [`GUN_SIDE_M`] m to its
//!   side in Hold, facing the enemy base; `p`'s engineers with
//!   [`BARRIER_CARRY_KG`] kg beside it. `drive_ai_habit(Fort)` until the
//!   barrier stands ([`BARRIER_PIECES`] pieces; at most [`OBST_DRIVE_S`]):
//!   `barrier_built_pct`. Then the
//!   enemy's medium tank, [`MINES_TANK_M`] m out, is ordered to the gun and
//!   `p`'s own medium tank, at `p`'s base, to the nearest point `p` does not
//!   hold, both for [`OBST_TANK_S`] s (the habit still driven):
//!   `enemy_time_in_arc_s` (report: seconds the enemy tank spends inside the
//!   gun's 90-degree arc within 400 m), `own_blocked` (the AI counter, must
//!   be 0). Exposure `enemy_tank_reached_barrier_pct` (the enemy tank came
//!   within 10 m of a barrier piece) and (T115e) `own_tank_drove_pct` (the
//!   own tank got [`OWN_DROVE_M`] m from its start: its route was tried). Controls: `control.own_blocked` with
//!   `own_route_m` 0, `control.enemy_time_in_arc_s` with `obstacles` 0.
//! * **Bunker** (Build C, R7). For each player `p` and seed, in two sets
//!   (`ger_baupioniergruppe`: `build_heavy`; `usa_engineer_squad`:
//!   `build_structure` at `heavy_by_field_pct`, keys `us_*`): `p` holds its nearest point
//!   (as the mines row, mines and obstacles off); the builders carry their
//!   full carry of [`BUNKER_CARRY_KG`] kg and three 40 kg construction crates
//!   stand by the bunker site (250 kg is over any squad's carry; a match
//!   brings the rest by truck); `p`'s MG team stands at the point (the
//!   hook's `others`). `drive_ai_habit(Fort)` until the bunker stands (at
//!   most [`BUNKER_DRIVE_S`]): `built_pct`, `built_s` (report, mean over
//!   built runs); then [`BUNKER_MAN_S`] s more: `manned_pct` (the MG team's
//!   leader within [`MANNED_M`] m of the bunker centre). Exposure
//!   `builder_alive_pct`. Control (`control.built_pct`, `control.manned_pct`):
//!   `bunker` 0.

use anyhow::{Result, bail};
use frontline_sim::{CommandKind, Fx, FxVec2, PlayerId, SimId, World};
use serde::Serialize;

use super::s10b::{
    AP_ROW_GAP_M, AP_STEP_CM, AT_ROW_GAP_M, AT_RUN_M, AT_STEP_CM, MEDIUM_TANKS, cross, field,
    fields, lay_ids, pct,
};
use super::{EAST, FortifyArgs, LINE_Y, TRENCH_X, at, members, pos, step_with, world_with};

/// The mines row (module docs): kg the engineer carries, AT mines that make
/// a field, the longest drive (s), the tank's start distance (m) and run (s).
pub const MINES_CARRY_KG: u32 = 160;
pub const AT_FIELD_MINES: usize = 8;
pub const MINES_DRIVE_S: u64 = 900;
pub const MINES_TANK_M: i32 = 150;
pub const MINES_TANK_S: u64 = 120;
/// The engineer card of each player (faction order as `MEDIUM_TANKS`).
const ENGINEERS: [&str; 2] = ["ger_engineer_squad", "usa_engineer_squad"];

/// The obstacle row (module docs): the gun's distance in front of the
/// point (m), the engineers' kg, the pieces of a standing barrier, the
/// tanks' drive (s).
pub const GUN_OUT_M: i32 = 10;
/// The gun stands this far to the side of the point (m): on the axis the
/// barrier lies across the own route out of the point and R6 refuses it
/// (Build C measure: 3 rejections a run, nothing built).
pub const GUN_SIDE_M: i32 = 60;
/// The longest drive of the obstacle row (s; five 2-minute segments and
/// the walks).
pub const OBST_DRIVE_S: u64 = 1500;
pub const BARRIER_CARRY_KG: u32 = 600;
pub const BARRIER_PIECES: usize = 5;
/// The obstacle row: the own tank drove when it got this far (m) from base.
pub const OWN_DROVE_M: i32 = 30;
pub const OBST_TANK_S: u64 = 180;
/// The AT gun card of each player (faction order as `MEDIUM_TANKS`).
const AT_GUNS: [&str; 2] = ["ger_pak40_squad", "usa_57mm_m1_squad"];

/// The bunker row (module docs): kg asked, the longest drive (s; 25 min),
/// the manning wait (s), the manned distance (m).
pub const BUNKER_CARRY_KG: u32 = 250;
pub const BUNKER_DRIVE_S: u64 = 1500;
pub const BUNKER_MAN_S: u64 = 60;
pub const MANNED_M: i32 = 5;
/// The builder card of each set, the MG card of each player.
const BUNKER_BUILDERS: [&str; 2] = ["ger_baupioniergruppe", "usa_engineer_squad"];
const MG_TEAMS: [&str; 2] = ["ger_smg_gruppe", "usa_lmg_squad"];

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

/// The control half of the mines row: `mines` 0.
#[derive(Clone, Debug, Default, Serialize)]
pub struct MinesControl {
    pub tank_stopped_pct: Option<f64>,
    pub field_laid_pct: Option<f64>,
}

/// T109/T110: the AI lays an AT field on the approach and it stops a tank.
#[derive(Clone, Debug, Default, Serialize)]
pub struct MinesRow {
    pub tank_stopped_pct: Option<f64>,
    pub field_laid_pct: Option<f64>,
    /// Seconds of driving until the AT field stood (mean over laid runs).
    pub laid_s: Option<f64>,
    /// Exposure: runs in which the tank came within 5 m of an AT mine.
    pub tank_reached_field_pct: Option<f64>,
    /// Exposure of `field_laid_pct`: runs whose engineer squad lived
    /// through the drive.
    pub builder_alive_pct: Option<f64>,
    pub control: MinesControl,
}

/// The control half of the obstacle row.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ObstacleControl {
    /// `obstacles` 0 (nothing built).
    pub barrier_built_pct: Option<f64>,
    /// `own_route_m` 0 (must be > 0 to show the rule matters).
    pub own_blocked: Option<f64>,
    /// `obstacles` 0 (no barrier).
    pub enemy_time_in_arc_s: Option<f64>,
}

/// T111/T115: a log barrier before an own gun, never on the own route.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ObstacleRow {
    pub barrier_built_pct: Option<f64>,
    /// Report: mean barrier pieces standing after the drive (the L1
    /// deadline `job_max_s` 900 s ends a one-builder 20 m barrier at 4 of
    /// 5; Build C measure, owner question S10b-ai-C1).
    pub barrier_pieces: Option<f64>,
    /// Report: mean seconds the enemy tank spent in the gun's arc.
    pub enemy_time_in_arc_s: Option<f64>,
    /// Sum of the AI's `own_blocked` over the runs (must be 0).
    pub own_blocked: Option<f64>,
    /// Exposure: runs in which the enemy tank came within 10 m of the barrier.
    pub enemy_tank_reached_barrier_pct: Option<f64>,
    /// Exposure of `own_blocked` (T115e): runs in which the own tank drove
    /// [`OWN_DROVE_M`] m from its start.
    pub own_tank_drove_pct: Option<f64>,
    /// Exposure of `barrier_built_pct`: runs whose engineers lived through
    /// the drive.
    pub builder_alive_pct: Option<f64>,
    pub control: ObstacleControl,
}

/// The control half of the bunker row: `bunker` 0.
#[derive(Clone, Debug, Default, Serialize)]
pub struct BunkerControl {
    pub built_pct: Option<f64>,
    /// No bunker, so no squad in one (T113's control).
    pub manned_pct: Option<f64>,
}

/// One builder set of the bunker row.
#[derive(Clone, Debug, Default, Serialize)]
pub struct BunkerSet {
    pub built_pct: Option<f64>,
    /// Report: seconds of driving until the bunker stood (built runs).
    pub built_s: Option<f64>,
    pub manned_pct: Option<f64>,
    /// Exposure: runs whose builders lived through the drive.
    pub builder_alive_pct: Option<f64>,
    pub control: BunkerControl,
}

/// T113: the AI builds the bunker (Baupioniere; US engineers at 50 %: the
/// `us_*` keys) and mans it.
#[derive(Clone, Debug, Default, Serialize)]
pub struct BunkerRow {
    pub built_pct: Option<f64>,
    pub built_s: Option<f64>,
    pub manned_pct: Option<f64>,
    pub builder_alive_pct: Option<f64>,
    pub us_built_pct: Option<f64>,
    pub us_built_s: Option<f64>,
    pub us_manned_pct: Option<f64>,
    pub us_builder_alive_pct: Option<f64>,
    /// Both sets with `bunker` 0.
    pub control: BunkerControl,
}

/// The S10b-ai rows (`fortify10.s10b_ai`).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S10bAiReport {
    pub paths: PathsRow,
    pub mines: MinesRow,
    pub obstacle: ObstacleRow,
    pub bunker: BunkerRow,
}

/// The rows in summary order.
const ROWS: &[&str] = &["paths", "mines", "obstacle", "bunker"];

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

/// Stages `p` holding the capture point nearest its base with its AI
/// trench dug across the way to the enemy base: (point index, point,
/// enemy base, trench line).
fn hold_point(w: &mut World, p: u8) -> Result<(usize, FxVec2, FxVec2, [FxVec2; 2])> {
    let enemy = 1 - p;
    let (i, pt, base_e, line) = {
        let g = w.game().unwrap();
        let s = w.match_setup().unwrap();
        let base = g.base_of(PlayerId(p));
        let base_e = g.base_of(PlayerId(enemy));
        let Some((i, q0)) = s
            .map
            .points
            .iter()
            .enumerate()
            .map(|(i, q)| (i, FxVec2::from(q.pos)))
            .min_by_key(|(i, q)| ((*q - base).length_sq_raw(), *i))
        else {
            bail!("the map has no capture point");
        };
        let dir = (base_e - q0).normalize_or_zero();
        let mid = q0 + dir * Fx::from(s.rules.ai.dig_offset);
        let half = dir.perp() * Fx::from(s.rules.ai.dig_length).div_int(2);
        (i, q0, base_e, [mid - half, mid + half])
    };
    if !w.set_point_owner(i, Some(PlayerId(p))) || !w.dig_trench_now(&line, -1) {
        bail!("the held point {i} was not staged");
    }
    Ok((i, pt, base_e, line))
}

/// One mines run of player `p` with `[ai.obstacles] mines` set: (field
/// laid, seconds to lay it, tank stopped, tank reached the field, builder
/// alive after the drive).
fn mines_once(
    args: &FortifyArgs,
    seed: u64,
    p: u8,
    mines: u32,
) -> Result<(bool, u64, bool, bool, bool)> {
    use frontline_sim::ai::{AiHabit, AiState, AiTrench, DriveArgs};
    let mut w = world_with(args, seed, |s| {
        s.rules.ai.fort.works = 0;
        s.rules.ai.fort.wire = 0;
        s.rules.ai.obstacles.mines = mines;
        s.rules.ai.obstacles.at_mines = 1;
    })?;
    let enemy = 1 - p;
    let (i, pt, base_e, line) = hold_point(&mut w, p)?;
    let s = w.match_setup().unwrap();
    let at_f = s.forts.iter().position(|f| f.id == "ww2:at_minefield");
    let hz = s.tick_hz as u64;
    let Some(ty) = s.squad_type_index(&format!("ww2:{}", ENGINEERS[p as usize])) else {
        bail!("no {} card", ENGINEERS[p as usize]);
    };
    let Some(eng) = w.spawn_squad_at(PlayerId(p), ty, pt, EAST) else {
        bail!("the engineers were not spawned");
    };
    w.edit_fort(|f| {
        f.carried_g.insert(eng, MINES_CARRY_KG * 1000);
    });
    let at_mines = |w: &World| {
        w.game()
            .unwrap()
            .fort()
            .mines
            .values()
            .filter(|m| m.owner == PlayerId(p) && Some(m.fort as usize) == at_f)
            .count()
    };
    let mut ai = AiState::new(w.tick());
    ai.trenches.push(AiTrench {
        point: i as u8,
        line,
    });
    let drive = DriveArgs {
        squad: Some(eng),
        point: i,
        ..Default::default()
    };
    let start = w.tick();
    // The control drives a minute (nothing to lay).
    let drive_s = if mines == 0 { 60 } else { MINES_DRIVE_S };
    while at_mines(&w) < AT_FIELD_MINES && w.tick() < start + drive_s * hz {
        w.drive_ai_habit(&mut ai, PlayerId(p), AiHabit::Fort, &drive);
    }
    let laid = at_mines(&w) >= AT_FIELD_MINES;
    let alive = w.game().unwrap().squad(eng).is_some();
    let laid_s = (w.tick() - start) / hz;
    // The enemy tank drives the approach onto the point.
    let s = w.match_setup().unwrap();
    let Some(tt) = s.squad_type_index(&format!("ww2:{}", MEDIUM_TANKS[enemy as usize])) else {
        bail!("no {} card", MEDIUM_TANKS[enemy as usize]);
    };
    let dir = (base_e - pt).normalize_or_zero();
    // Moved across the approach by the seed's offset (0-4 m, the AT
    // field's mine step), as the paths row moves its field. The tank still
    // drives the 4 m path grid: every seed crosses on the route's own lane
    // (Build C measure, log), the lane the AI's row is laid for.
    let side = dir.perp() * Fx::from_milli(((seed * 37) % 400) as i32 * 10);
    let from = pt + dir * Fx::from_int(MINES_TANK_M) + side;
    let Some(tank) = w.spawn_squad_at(PlayerId(enemy), tt, from, -dir) else {
        bail!("the tank was not spawned at {from:?}");
    };
    let v = members(&w, tank)[0];
    step_with(
        &mut w,
        enemy,
        vec![CommandKind::Move {
            squad: tank,
            target: pt + side,
        }],
    );
    let exp0 = w.game().unwrap().fort().accounts[p as usize].at_exposure_ticks;
    let stopped = |w: &World| {
        w.game().unwrap().vehicle(v).is_none_or(|vs| {
            let def = &w.match_setup().unwrap().vehicles[vs.type_idx as usize];
            vs.immobilised(def) || vs.wreck_tick.is_some()
        })
    };
    let t0 = w.tick();
    while !stopped(&w)
        && (pos(&w, v) - (pt + side)).length() > Fx::from_int(5)
        && w.tick() < t0 + MINES_TANK_S * hz
    {
        w.step(&[]);
    }
    for _ in 0..hz {
        w.step(&[]);
    }
    let reached = w.game().unwrap().fort().accounts[p as usize].at_exposure_ticks > exp0;
    Ok((laid, laid_s, stopped(&w), reached, alive))
}

/// The mines row (module docs).
pub fn mines_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<MinesRow> {
    let (mut laid, mut laid_s, mut stopped, mut reached, mut runs) = (0u32, 0u64, 0u32, 0u32, 0u32);
    let (mut c_laid, mut c_stopped, mut alive) = (0u32, 0u32, 0u32);
    for seed in seeds {
        for p in [0u8, 1] {
            let (l, t, s, r, a) = mines_once(args, seed, p, 1)?;
            alive += a as u32;
            laid += l as u32;
            laid_s += if l { t } else { 0 };
            stopped += s as u32;
            reached += r as u32;
            let (l, _, s, _, _) = mines_once(args, seed, p, 0)?;
            c_laid += l as u32;
            c_stopped += s as u32;
            runs += 1;
        }
    }
    Ok(MinesRow {
        tank_stopped_pct: pct(stopped, runs),
        field_laid_pct: pct(laid, runs),
        laid_s: (laid > 0).then(|| laid_s as f64 / laid as f64),
        tank_reached_field_pct: pct(reached, runs),
        builder_alive_pct: pct(alive, runs),
        control: MinesControl {
            tank_stopped_pct: pct(c_stopped, runs),
            field_laid_pct: pct(c_laid, runs),
        },
    })
}

/// Every S10b-ai row.
pub fn s10b_ai_rows(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<S10bAiReport> {
    Ok(S10bAiReport {
        paths: paths_row(args, seeds.clone())?,
        mines: mines_row(args, seeds.clone())?,
        obstacle: obstacle_row(args, seeds.clone())?,
        bunker: bunker_row(args, seeds)?,
    })
}

/// One obstacle run (module docs). `mode` 0: as set; 1: `own_route_m` 0;
/// 2: `obstacles` 0. (barrier built, seconds the enemy tank spent in the
/// arc, own_blocked, enemy tank reached the barrier, barrier pieces).
fn obstacle_once(
    args: &FortifyArgs,
    seed: u64,
    p: u8,
    mode: u8,
) -> Result<(bool, f64, u32, bool, usize, bool, bool)> {
    use frontline_sim::ai::{AiGun, AiHabit, AiState, AiTrench, DriveArgs, GunPhase};
    let mut w = world_with(args, seed, |s| {
        s.rules.ai.fort.works = 0;
        s.rules.ai.fort.wire = 0;
        s.rules.ai.fort.gun_hold_s = 0;
        s.rules.ai.obstacles.mines = 0;
        s.rules.ai.obstacles.obstacles = u32::from(mode != 2);
        if mode == 1 {
            s.rules.ai.obstacles.own_route_m = 0;
        }
    })?;
    let enemy = 1 - p;
    let (i, pt, base_e, line) = hold_point(&mut w, p)?;
    let dir = (base_e - pt).normalize_or_zero();
    let s = w.match_setup().unwrap();
    let hz = s.tick_hz as u64;
    let bar_f = s.forts.iter().position(|f| f.id == "ww2:log_barrier");
    let card = |w: &World, c: &str| {
        w.match_setup()
            .unwrap()
            .squad_type_index(&format!("ww2:{c}"))
            .ok_or_else(|| anyhow::anyhow!("no {c} card"))
    };
    let gun_at = pt + dir * Fx::from_int(GUN_OUT_M) + dir.perp() * Fx::from_int(GUN_SIDE_M);
    let gty = card(&w, AT_GUNS[p as usize])?;
    let Some(gsq) = w.spawn_squad_at(PlayerId(p), gty, gun_at, dir) else {
        bail!("the gun was not spawned at {gun_at:?}");
    };
    let site = pos(&w, members(&w, gsq)[0]);
    let ety = card(&w, ENGINEERS[p as usize])?;
    let Some(eng) = w.spawn_squad_at(PlayerId(p), ety, site - dir * Fx::from_int(8), EAST) else {
        bail!("the engineers were not spawned");
    };
    w.edit_fort(|f| {
        f.carried_g.insert(eng, BARRIER_CARRY_KG * 1000);
    });
    let mut ai = AiState::new(w.tick());
    ai.trenches.push(AiTrench {
        point: i as u8,
        line,
    });
    let t = w.tick();
    ai.guns.insert(
        gsq,
        AiGun {
            phase: GunPhase::Hold,
            site,
            facing: site + dir * Fx::from_int(100),
            moved_to: site,
            moved_at: t,
            quiet_at: t,
            pit: true,
            ..Default::default()
        },
    );
    let drive = DriveArgs {
        squad: Some(eng),
        point: i,
        ..Default::default()
    };
    let pieces = |w: &World| -> Vec<FxVec2> {
        w.game()
            .unwrap()
            .fort()
            .pieces
            .values()
            .filter(|q| q.owner == PlayerId(p) && Some(q.fort as usize) == bar_f)
            .map(|q| q.pos)
            .collect()
    };
    let drive_s = if mode == 2 { 60 } else { OBST_DRIVE_S };
    while pieces(&w).len() < BARRIER_PIECES && w.tick() < t + drive_s * hz {
        w.drive_ai_habit(&mut ai, PlayerId(p), AiHabit::Fort, &drive);
    }
    let n_pieces = pieces(&w).len();
    let built = n_pieces >= BARRIER_PIECES;
    let alive = w.game().unwrap().squad(eng).is_some();
    // The enemy tank drives onto the gun; the own tank from base to the
    // nearest point `p` does not hold.
    let s = w.match_setup().unwrap();
    let next = s
        .map
        .points
        .iter()
        .enumerate()
        .filter(|&(j, _)| j != i)
        .map(|(j, q)| (j, FxVec2::from(q.pos)))
        .min_by_key(|(j, q)| ((*q - pt).length_sq_raw(), *j))
        .map(|(_, q)| q);
    // In front of the gun, so its drive meets the barrier's line.
    let from = site + dir * Fx::from_int(MINES_TANK_M);
    let tt = card(&w, MEDIUM_TANKS[enemy as usize])?;
    let Some(et) = w.spawn_squad_at(PlayerId(enemy), tt, from, -dir) else {
        bail!("the enemy tank was not spawned at {from:?}");
    };
    let ev = members(&w, et)[0];
    step_with(
        &mut w,
        enemy,
        vec![CommandKind::Move {
            squad: et,
            target: site,
        }],
    );
    let mut own_v = None;
    if let Some(next) = next {
        let base = w.game().unwrap().base_of(PlayerId(p));
        let ot = card(&w, MEDIUM_TANKS[p as usize])?;
        if let Some(own) = w.spawn_squad_at(PlayerId(p), ot, base, dir) {
            let v = members(&w, own)[0];
            own_v = Some((v, pos(&w, v)));
            step_with(
                &mut w,
                p,
                vec![CommandKind::Move {
                    squad: own,
                    target: next,
                }],
            );
        }
    }
    let bar = pieces(&w);
    let (mut in_arc, mut reached, mut drove) = (0u64, false, false);
    let t0 = w.tick();
    while w.tick() < t0 + OBST_TANK_S * hz {
        w.drive_ai_habit(&mut ai, PlayerId(p), AiHabit::Fort, &drive);
        if let Some((v, from)) = own_v
            && w.game().unwrap().vehicle(v).is_some()
        {
            drove |= (pos(&w, v) - from).length() >= Fx::from_int(OWN_DROVE_M);
        }
        let Some(vs) = w.game().unwrap().vehicle(ev) else {
            continue;
        };
        if vs.wreck_tick.is_some() {
            continue;
        }
        let at = pos(&w, ev);
        reached |= bar.iter().any(|b| (*b - at).length() <= Fx::from_int(10));
        let v = at - site;
        let ahead = v.dot(dir);
        let side = v.dot(dir.perp()).abs();
        if ahead > Fx::ZERO && side <= ahead && v.length() <= Fx::from_int(400) {
            in_arc += 1;
        }
    }
    Ok((
        built,
        in_arc as f64 / hz as f64,
        ai.counters.own_blocked,
        reached,
        n_pieces,
        alive,
        drove,
    ))
}

/// The obstacle row (module docs).
pub fn obstacle_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<ObstacleRow> {
    let (mut built, mut arc, mut blocked, mut reached, mut runs) = (0u32, 0f64, 0u32, 0u32, 0u32);
    let (mut c_blocked, mut c_arc, mut n_pieces) = (0u32, 0f64, 0f64);
    let (mut alive, mut c_built, mut drove) = (0u32, 0u32, 0u32);
    for seed in seeds {
        for p in [0u8, 1] {
            let (b, a, o, r, n, l, d) = obstacle_once(args, seed, p, 0)?;
            alive += l as u32;
            drove += d as u32;
            n_pieces += n as f64;
            built += b as u32;
            arc += a;
            blocked += o;
            reached += r as u32;
            c_blocked += obstacle_once(args, seed, p, 1)?.2;
            let c = obstacle_once(args, seed, p, 2)?;
            c_arc += c.1;
            c_built += c.0 as u32;
            runs += 1;
        }
    }
    let mean = |x: f64| (runs > 0).then(|| x / runs as f64);
    Ok(ObstacleRow {
        barrier_built_pct: pct(built, runs),
        barrier_pieces: mean(n_pieces),
        enemy_time_in_arc_s: mean(arc),
        own_blocked: (runs > 0).then_some(blocked as f64),
        enemy_tank_reached_barrier_pct: pct(reached, runs),
        own_tank_drove_pct: pct(drove, runs),
        builder_alive_pct: pct(alive, runs),
        control: ObstacleControl {
            barrier_built_pct: pct(c_built, runs),
            own_blocked: (runs > 0).then_some(c_blocked as f64),
            enemy_time_in_arc_s: mean(c_arc),
        },
    })
}

/// One bunker run (module docs) of player `p` with builder set `set` and
/// `[ai.obstacles] bunker` set: (built, seconds to build, manned, builders
/// alive).
fn bunker_once(
    args: &FortifyArgs,
    seed: u64,
    p: u8,
    set: usize,
    bunker: u32,
) -> Result<(bool, u64, bool, bool)> {
    use frontline_sim::ai::{AiHabit, AiState, AiTrench, DriveArgs};
    let mut w = world_with(args, seed, |s| {
        s.rules.ai.fort.works = 0;
        s.rules.ai.fort.wire = 0;
        s.rules.ai.obstacles.mines = 0;
        s.rules.ai.obstacles.obstacles = 0;
        s.rules.ai.obstacles.bunker = bunker;
    })?;
    let (i, pt, base_e, line) = hold_point(&mut w, p)?;
    let dir = (base_e - pt).normalize_or_zero();
    let s = w.match_setup().unwrap();
    let hz = s.tick_hz as u64;
    let bf = s.forts.iter().position(|f| f.id == "ww2:bunker");
    // R7's site: the trench end opposite the nest (by the point's parity).
    let [a, b] = line;
    let (end, other) = if i % 2 == 0 { (a, b) } else { (b, a) };
    let off = Fx::from_int(s.rules.ai.fort.nest_offset_m as i32);
    let site = end + (end - other).normalize_or_zero() * off;
    let Some(crate_def) = s
        .stocks
        .iter()
        .position(|d| d.id == "ww2:usa_construction_crate")
    else {
        bail!("no construction crate");
    };
    let card = |w: &World, c: &str| {
        w.match_setup()
            .unwrap()
            .squad_type_index(&format!("ww2:{c}"))
            .ok_or_else(|| anyhow::anyhow!("no {c} card"))
    };
    for k in 0..3 {
        let at = site - dir * Fx::from_int(6) + dir.perp() * Fx::from_int(k);
        if w.add_crate_now(PlayerId(p), crate_def as u16, at).is_none() {
            bail!("the crate was not placed");
        }
    }
    let ety = card(&w, BUNKER_BUILDERS[set])?;
    let Some(eng) = w.spawn_squad_at(PlayerId(p), ety, pt, EAST) else {
        bail!("the builders were not spawned");
    };
    w.edit_fort(|f| {
        f.carried_g.insert(eng, BUNKER_CARRY_KG * 1000);
    });
    let mty = card(&w, MG_TEAMS[p as usize])?;
    let Some(mg) = w.spawn_squad_at(PlayerId(p), mty, pt - dir * Fx::from_int(8), EAST) else {
        bail!("the MG team was not spawned");
    };
    let mut ai = AiState::new(w.tick());
    ai.trenches.push(AiTrench {
        point: i as u8,
        line,
    });
    let drive = DriveArgs {
        squad: Some(eng),
        point: i,
        others: vec![mg],
        ..Default::default()
    };
    let roof = |w: &World| -> Option<FxVec2> {
        w.game()
            .unwrap()
            .fort()
            .pieces
            .values()
            .find(|q| q.owner == PlayerId(p) && Some(q.fort as usize) == bf && q.part == 0)
            .map(|q| q.pos)
    };
    let start = w.tick();
    let drive_s = if bunker == 0 { 60 } else { BUNKER_DRIVE_S };
    while roof(&w).is_none() && w.tick() < start + drive_s * hz {
        w.drive_ai_habit(&mut ai, PlayerId(p), AiHabit::Fort, &drive);
    }
    let built_s = (w.tick() - start) / hz;
    let alive = w.game().unwrap().squad(eng).is_some();
    let Some(centre) = roof(&w) else {
        return Ok((false, built_s, false, alive));
    };
    let t0 = w.tick();
    while w.tick() < t0 + BUNKER_MAN_S * hz {
        w.drive_ai_habit(&mut ai, PlayerId(p), AiHabit::Fort, &drive);
    }
    let manned = members(&w, mg)
        .first()
        .is_some_and(|&m| (pos(&w, m) - centre).length() <= Fx::from_int(MANNED_M));
    Ok((true, built_s, manned, alive))
}

/// One builder set of the bunker row.
fn bunker_set(args: &FortifyArgs, seeds: std::ops::Range<u64>, set: usize) -> Result<BunkerSet> {
    let (mut built, mut secs, mut manned, mut alive, mut c_built, mut runs) =
        (0u32, 0u64, 0u32, 0u32, 0u32, 0u32);
    let mut c_manned = 0u32;
    for seed in seeds {
        for p in [0u8, 1] {
            let (b, t, m, a) = bunker_once(args, seed, p, set, 1)?;
            built += b as u32;
            secs += if b { t } else { 0 };
            manned += m as u32;
            alive += a as u32;
            let c = bunker_once(args, seed, p, set, 0)?;
            c_built += c.0 as u32;
            c_manned += c.2 as u32;
            runs += 1;
        }
    }
    Ok(BunkerSet {
        built_pct: pct(built, runs),
        built_s: (built > 0).then(|| secs as f64 / built as f64),
        manned_pct: pct(manned, runs),
        builder_alive_pct: pct(alive, runs),
        control: BunkerControl {
            built_pct: pct(c_built, runs),
            manned_pct: pct(c_manned, runs),
        },
    })
}

/// The bunker row (module docs).
pub fn bunker_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<BunkerRow> {
    let heavy = bunker_set(args, seeds.clone(), 0)?;
    let us = bunker_set(args, seeds, 1)?;
    let both = |a: Option<f64>, b: Option<f64>| a.zip(b).map(|(a, b)| (a + b) / 2.0);
    Ok(BunkerRow {
        built_pct: heavy.built_pct,
        built_s: heavy.built_s,
        manned_pct: heavy.manned_pct,
        builder_alive_pct: heavy.builder_alive_pct,
        us_built_pct: us.built_pct,
        us_built_s: us.built_s,
        us_manned_pct: us.manned_pct,
        us_builder_alive_pct: us.builder_alive_pct,
        control: BunkerControl {
            built_pct: both(heavy.control.built_pct, us.control.built_pct),
            manned_pct: both(heavy.control.manned_pct, us.control.manned_pct),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One-off measure of the obstacle row (the full fortify run plays it
    /// with the rest): `cargo test --release -p frontline-cli --lib
    /// obstacle_row_one_off -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn obstacle_row_one_off() {
        let args = FortifyArgs::default();
        let r = obstacle_row(&args, 1..4).unwrap();
        println!("obstacle {}", serde_json::to_string(&r).unwrap());
    }

    /// One-off measure of the bunker row: `cargo test --release -p
    /// frontline-cli --lib bunker_row_one_off -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bunker_row_one_off() {
        let args = FortifyArgs::default();
        let r = bunker_row(&args, 1..4).unwrap();
        println!("bunker {}", serde_json::to_string(&r).unwrap());
    }
}
