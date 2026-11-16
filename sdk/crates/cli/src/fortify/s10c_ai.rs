//! S10c-ai fortify-test row `s10c.belt` (docs/slices/S10c-ai-build-C.md
//! item 7, C7: do the pieces fit together; three-part row: value,
//! exposure, control). A report row: `pass` ignores it.
//!
//! For each player `p` and seed: `p` holds the capture point nearest its
//! base (`hold_point`, its AI trench dug), a front point by plain F1
//! (`front_home` 1: the home point is the only own point); its lane is
//! staged with the FrontAp and FrontWire lines done (`ap_done`,
//! `wire_done`: the row is about the AT field and the belt), S10a works,
//! wire and the bunker off, `at_rows` 1. `p`'s Baupioniere (US: engineers,
//! no `build_heavy`: the log line) stand at the point; their carry is
//! topped up to [`BELT_CARRY_KG`] kg every second (staged supply: a belt is
//! over any squad's carry and F10 sends no front squad home, so in a match
//! the rest comes by truck; the stage file's "loaded truck"). Then
//! `drive_ai_habit(Fort)` until the AI's AT field row 1 stands
//! ([`AT_ROW_MINES`] AT mines) and the belt stands ([`BELT_PIECES`] own
//! blocking pieces beyond the field), at most [`BELT_DRIVE_S`]:
//! `belt_built_pct`, `belt_pieces` (report, mean). Then the enemy's medium
//! tank is spawned at its base and ordered onto the point, and `p`'s own
//! medium tank at its base ordered to the nearest other point, for
//! [`BELT_TANK_S`] s (the habit still driven): `enemy_tank_mined_pct` (the
//! enemy tank immobilised or wrecked), `own_blocked` (the AI's counter,
//! summed: must be 0). Exposure: `enemy_tank_reached_belt_pct` (within
//! 15 m of a belt piece), `field_laid_pct`, `builder_alive_pct`.
//! Controls: `control.enemy_tank_mined_pct` / `control.belt_built_pct`
//! with `belt` 0, `control.own_blocked` with `belt_detour_pct` 0.
use anyhow::{Result, bail};
use frontline_sim::{CommandKind, Fx, FxVec2, PlayerId, World};
use serde::Serialize;

use super::s10b::{MEDIUM_TANKS, fields, pct};
use super::s10b_ai::hold_point;
use super::{EAST, FortifyArgs, members, pos, step_with, world_with};

/// kg the builders are topped up to each second.
pub const BELT_CARRY_KG: u32 = 160;
/// AT mines of row 1 (`at_row_mines` default) that make the field.
pub const AT_ROW_MINES: usize = 8;
/// Blocking pieces that make the belt (`belt_pieces`; a 20 m log line
/// stands as five 4 m pieces).
pub const BELT_PIECES: usize = 5;
/// The longest drive for field and belt (s).
pub const BELT_DRIVE_S: u64 = 1800;
/// The tanks' run (s).
pub const BELT_TANK_S: u64 = 300;
/// "Reached the belt": within this many metres of a belt piece.
pub const REACHED_M: i32 = 15;

const BUILDERS: [&str; 2] = ["ger_baupioniergruppe", "usa_engineer_squad"];

/// The control half of the belt row.
#[derive(Clone, Debug, Default, Serialize)]
pub struct BeltControl {
    /// `belt` 0.
    pub belt_built_pct: Option<f64>,
    pub enemy_tank_mined_pct: Option<f64>,
    /// `belt_detour_pct` 0 (sum over the runs).
    pub own_blocked: Option<f64>,
}

/// C7: the belt beyond the AI's AT field.
#[derive(Clone, Debug, Default, Serialize)]
pub struct BeltRow {
    pub belt_built_pct: Option<f64>,
    /// Report: mean belt pieces standing after the drive.
    pub belt_pieces: Option<f64>,
    pub enemy_tank_mined_pct: Option<f64>,
    /// Sum of the AI's `own_blocked` over the runs (must be 0).
    pub own_blocked: Option<f64>,
    /// Exposure: runs in which the enemy tank came within 15 m of a piece.
    pub enemy_tank_reached_belt_pct: Option<f64>,
    /// Exposure of `belt_built_pct`: runs in which the AI's row 1 stood.
    pub field_laid_pct: Option<f64>,
    /// Exposure: runs whose builders lived through the drive.
    pub builder_alive_pct: Option<f64>,
    pub control: BeltControl,
}

/// The S10c-ai rows (`fortify10.s10c`).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S10cAiReport {
    pub belt: BeltRow,
}

/// Summary lines, one per row.
pub fn summary_text(r: &S10cAiReport) -> String {
    let v = serde_json::to_value(r).unwrap_or_default();
    format!("  S10c-ai belt: {}\n", fields(&v["belt"]))
}

/// One belt run's results.
#[derive(Default)]
struct Once {
    laid: bool,
    built: bool,
    pieces: usize,
    mined: bool,
    reached: bool,
    own_blocked: u32,
    alive: bool,
}

/// One belt run of player `p`; `mode` 0: as set, 1: `belt` 0, 2:
/// `belt_detour_pct` 0.
fn belt_once(args: &FortifyArgs, seed: u64, p: u8, mode: u8) -> Result<Once> {
    use frontline_sim::ai::{AiHabit, AiState, AiTrench, DriveArgs, FrontLane};
    let mut w = world_with(args, seed, |s| {
        s.rules.ai.fort.works = 0;
        s.rules.ai.fort.wire = 0;
        let o = &mut s.rules.ai.obstacles;
        o.front = 1;
        o.front_home = 1;
        o.mines = 1;
        o.at_mines = 1;
        o.at_rows = 1;
        o.bunker = 0;
        o.belt = u32::from(mode != 1);
        if mode == 2 {
            o.belt_detour_pct = 0;
        }
        // The drive lasts up to 30 min: no ticket drain ends the match.
        s.rules.ticket_drain_per_min_per_point = 0;
    })?;
    let enemy = 1 - p;
    let (i, pt, base_e, line) = hold_point(&mut w, p)?;
    let s = w.match_setup().unwrap();
    let hz = s.tick_hz as u64;
    let at_f = s.forts.iter().position(|f| f.id == "ww2:at_minefield");
    let blocking: Vec<usize> = s
        .forts
        .iter()
        .enumerate()
        .filter(|(_, f)| f.id == "ww2:hedgehog" || f.id == "ww2:log_barrier")
        .map(|(k, _)| k)
        .collect();
    let card = |w: &World, c: &str| {
        w.match_setup()
            .unwrap()
            .squad_type_index(&format!("ww2:{c}"))
            .ok_or_else(|| anyhow::anyhow!("no {c} card"))
    };
    let bty = card(&w, BUILDERS[p as usize])?;
    let Some(bau) = w.spawn_squad_at(PlayerId(p), bty, pt, EAST) else {
        bail!("the builders were not spawned");
    };
    let dir = (base_e - pt).normalize_or_zero();
    let side = dir.perp() * Fx::from_int(10);
    let mut ai = AiState::new(w.tick());
    ai.trenches.push(AiTrench {
        point: i as u8,
        line,
    });
    ai.front_lanes.push(FrontLane {
        point: i as u8,
        since: w.tick(),
        ap: [
            pt + dir * Fx::from_int(50) - side,
            pt + dir * Fx::from_int(50) + side,
        ],
        wire: [
            pt + dir * Fx::from_int(30) - side,
            pt + dir * Fx::from_int(30) + side,
        ],
        ap_done: true,
        wire_done: true,
    });
    let drive = DriveArgs {
        squad: Some(bau),
        point: i,
        ..Default::default()
    };
    let at_mines = |w: &World| -> Vec<FxVec2> {
        w.game()
            .unwrap()
            .fort()
            .mines
            .values()
            .filter(|m| m.owner == PlayerId(p) && Some(m.fort as usize) == at_f)
            .map(|m| m.pos)
            .collect()
    };
    // Belt pieces: own blocking pieces farther from the point than the
    // field's nearest AT mine (the belt stands beyond the field).
    let belt = |w: &World| -> Vec<FxVec2> {
        let field = at_mines(w);
        let Some(near) = field.iter().map(|m| (*m - pt).length_sq_raw()).min() else {
            return Vec::new();
        };
        w.game()
            .unwrap()
            .fort()
            .pieces
            .values()
            .filter(|q| {
                q.owner == PlayerId(p)
                    && blocking.contains(&(q.fort as usize))
                    && (q.pos - pt).length_sq_raw() > near
            })
            .map(|q| q.pos)
            .collect()
    };
    let t = w.tick();
    let drive_s = if mode == 1 {
        BELT_DRIVE_S / 3
    } else {
        BELT_DRIVE_S
    };
    let mut last = t;
    loop {
        let laid = at_mines(&w).len() >= AT_ROW_MINES;
        if (laid && (mode == 1 || belt(&w).len() >= BELT_PIECES)) || w.tick() >= t + drive_s * hz {
            break;
        }
        if w.tick() >= last + hz {
            last = w.tick();
            w.edit_fort(|f| {
                let g = f.carried_g.entry(bau).or_insert(0);
                *g = (*g).max(BELT_CARRY_KG * 1000);
            });
        }
        w.drive_ai_habit(&mut ai, PlayerId(p), AiHabit::Fort, &drive);
    }
    let laid = at_mines(&w).len() >= AT_ROW_MINES;
    let pieces = belt(&w);
    let alive = w.game().unwrap().squad(bau).is_some();
    // The enemy tank from its base onto the point; the own tank from its
    // base to the nearest other point.
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
    let tt = card(&w, MEDIUM_TANKS[enemy as usize])?;
    let Some(et) = w.spawn_squad_at(PlayerId(enemy), tt, base_e, -dir) else {
        bail!("the enemy tank was not spawned at {base_e:?}");
    };
    let ev = members(&w, et)[0];
    step_with(
        &mut w,
        enemy,
        vec![CommandKind::Move {
            squad: et,
            target: pt,
        }],
    );
    if let Some(next) = next {
        let base = w.game().unwrap().base_of(PlayerId(p));
        let ot = card(&w, MEDIUM_TANKS[p as usize])?;
        if let Some(own) = w.spawn_squad_at(PlayerId(p), ot, base, dir) {
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
    let stopped = |w: &World| {
        w.game().unwrap().vehicle(ev).is_none_or(|vs| {
            let def = &w.match_setup().unwrap().vehicles[vs.type_idx as usize];
            vs.immobilised(def) || vs.wreck_tick.is_some()
        })
    };
    let mut reached = false;
    let t0 = w.tick();
    while w.tick() < t0 + BELT_TANK_S * hz {
        w.drive_ai_habit(&mut ai, PlayerId(p), AiHabit::Fort, &drive);
        if w.game().unwrap().vehicle(ev).is_some() {
            let at = pos(&w, ev);
            reached |= pieces
                .iter()
                .any(|b| (*b - at).length() <= Fx::from_int(REACHED_M));
        }
    }
    Ok(Once {
        laid,
        built: pieces.len() >= BELT_PIECES,
        pieces: pieces.len(),
        mined: stopped(&w),
        reached,
        own_blocked: ai.counters.own_blocked,
        alive,
    })
}

/// The belt row (module docs).
pub fn belt_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<BeltRow> {
    let (mut runs, mut built, mut pieces, mut mined, mut reached) =
        (0u32, 0u32, 0usize, 0u32, 0u32);
    let (mut blocked, mut laid, mut alive) = (0u32, 0u32, 0u32);
    let (mut c_built, mut c_mined, mut c_blocked) = (0u32, 0u32, 0u32);
    for seed in seeds {
        for p in [0u8, 1] {
            let r = belt_once(args, seed, p, 0)?;
            runs += 1;
            built += r.built as u32;
            pieces += r.pieces;
            mined += r.mined as u32;
            reached += r.reached as u32;
            blocked += r.own_blocked;
            laid += r.laid as u32;
            alive += r.alive as u32;
            let c = belt_once(args, seed, p, 1)?;
            c_built += c.built as u32;
            c_mined += c.mined as u32;
            let c = belt_once(args, seed, p, 2)?;
            c_blocked += c.own_blocked;
        }
    }
    Ok(BeltRow {
        belt_built_pct: pct(built, runs),
        belt_pieces: (runs > 0).then(|| pieces as f64 / runs as f64),
        enemy_tank_mined_pct: pct(mined, runs),
        own_blocked: (runs > 0).then_some(blocked as f64),
        enemy_tank_reached_belt_pct: pct(reached, runs),
        field_laid_pct: pct(laid, runs),
        builder_alive_pct: pct(alive, runs),
        control: BeltControl {
            belt_built_pct: pct(c_built, runs),
            enemy_tank_mined_pct: pct(c_mined, runs),
            own_blocked: (runs > 0).then_some(c_blocked as f64),
        },
    })
}

/// Every S10c-ai row.
pub fn s10c_ai_rows(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<S10cAiReport> {
    Ok(S10cAiReport {
        belt: belt_row(args, seeds)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One-off: the belt row on seeds 1-5 (as `fortify-test`; run by
    /// hand, about two minutes in release; the result goes to the log).
    #[test]
    #[ignore]
    fn belt_row_one_off() {
        let r = belt_row(&FortifyArgs::default(), 1..6).unwrap();
        eprintln!("{}", serde_json::to_string(&r).unwrap());
    }
}
