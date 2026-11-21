//! S13 armour-test cases (`docs/slices/S13-heavy-wave.md` 7): the heavy
//! wave's duels, each run with rule O1 (the default) and with an `o1_off`
//! control (`[armour] obliquity` 0) on the same seeds.
//!
//! 41 Panther G frontal at 350 m against two Shermans (75) coming from 45°
//! either side of its nose; 42 M36 against a Panther, frontal 350 m; 43
//! M4A3 (76) W with its 2 HVAP against a Panther, frontal 350 m; 44 Tiger
//! against two Shermans (75), frontal 350 m; 45 a Jumbo driving at a PaK 40
//! from 400 m; 46 a 76 W 200 m off a Panther's flank.
//!
//! Every vehicle stands on open flat ground with its sight raised to 1 km
//! (staged as spotted: at 350 m two still tanks rarely spot each other, as
//! `scenarios::Duel::Frontal600`), crews never bail (`open_top_world`).
//! Hits are tallied by zone name (`VehicleSetup::zone_name`: the five new
//! vehicles have three boxes, `hull`, `superstructure`, `turret`). The
//! plan names `scenarios::duel_cards`; its two-tank lanes do not stage two
//! attackers, a 45° approach or a moving tank, so these cases spawn by card
//! on the same staging (`spawn_card`).

use std::collections::BTreeMap;

use anyhow::Result;
use frontline_sim::armour::PenResult;
use frontline_sim::gunnery::SPECIAL;
use frontline_sim::scenarios::{at, dir_deg};
use frontline_sim::{SimId, World};

use super::{CaseReport, CommandKind, Env, disabled, hz, open_top_from, order, spawn_card, vstate};

const PANTHER: &str = "ww2:ger_panther_g_squad";
const TIGER: &str = "ww2:ger_tiger_1_squad";
const SHERMAN: &str = "ww2:usa_m4a3_75_squad";
const W76: &str = "ww2:usa_m4a3_76w_squad";
const M36: &str = "ww2:usa_m36_squad";
const JUMBO: &str = "ww2:usa_m4a3e2_squad";
const PAK: &str = "ww2:ger_pak40_squad";

/// One vehicle of a staged fight: player, card, position (m), facing (deg
/// counter-clockwise from east).
struct Unit {
    p: u8,
    card: &'static str,
    x: i32,
    y: i32,
    deg: i32,
}

const fn u(p: u8, card: &'static str, x: i32, y: i32, deg: i32) -> Unit {
    Unit { p, card, x, y, deg }
}

/// What one run left: per unit whether it ended disabled, the hits it
/// took (with the zone name and result) and the rare rounds it fired; the
/// first penetration's second.
#[derive(Default)]
struct Out {
    dead: Vec<bool>,
    /// (target unit, zone name, penetrated).
    hits: Vec<(usize, String, bool)>,
    /// Hits each unit took before it was first disabled (all its hits when
    /// never).
    hits_before_down: Vec<u32>,
    rare_fired: Vec<u32>,
    first_pen_s: Option<f64>,
}

/// Plays `units` (mirrored east-west with `flip`) for at most `secs`, or
/// until one player has no live vehicle; `drive`: unit `k` is ordered to
/// move to the position of unit `j`.
fn fight(
    env: &Env,
    seed: u64,
    flip: bool,
    o1: bool,
    units: &[Unit],
    drive: Option<(usize, usize)>,
    secs: u64,
) -> Result<Out> {
    let mut s = env.setup(seed)?;
    let mut w: World = open_top_from(&mut s, env.threads, |s| {
        for v in &mut s.vehicles {
            v.sight = frontline_content::Fixed::from_int(1000);
        }
        if !o1 {
            s.rules.armour.obliquity = 0;
        }
    })?;
    let mx = |x: i32| if flip { 1000 - x } else { x };
    let md = |d: i32| if flip { 180 - d } else { d };
    let mut ids: Vec<(frontline_sim::SquadId, SimId)> = Vec::new();
    for x in units {
        ids.push(spawn_card(
            &mut w,
            x.p,
            x.card,
            at(mx(x.x), x.y),
            dir_deg(md(x.deg)),
        )?);
    }
    let rare0: Vec<u32> = ids
        .iter()
        .map(|&(_, v)| {
            vstate(&w, v).map_or(0, |s| {
                s.ammo.get(SPECIAL as usize).copied().unwrap_or(0) as u32
            })
        })
        .collect();
    if let Some((k, j)) = drive {
        let target = at(mx(units[j].x), units[j].y);
        order(
            &mut w,
            units[k].p,
            vec![CommandKind::Move {
                squad: ids[k].0,
                target,
            }],
        );
    }
    w.enable_hit_log();
    let rate = hz(&w);
    let start = w.tick();
    let limit = start + secs * rate;
    let n = units.len();
    let mut out = Out {
        dead: vec![false; n],
        hits_before_down: vec![0; n],
        rare_fired: vec![0; n],
        ..Out::default()
    };
    let unit_of = |v: SimId| ids.iter().position(|&(_, x)| x == v);
    let mut down = vec![false; n];
    while w.tick() < limit {
        w.step(&[]);
        let setup = w.match_setup().expect("match").clone();
        for h in w.take_hit_log() {
            let Some(t) = unit_of(h.vehicle) else {
                continue;
            };
            let vdef = &setup.vehicles[vstate(&w, h.vehicle).map_or(0, |v| v.type_idx) as usize];
            let pen = h.result == PenResult::Penetration;
            out.hits.push((t, vdef.zone_name(h.zone), pen));
            if !down[t] {
                out.hits_before_down[t] += 1;
            }
            if pen && out.first_pen_s.is_none() {
                out.first_pen_s = Some((w.tick() - start) as f64 / rate as f64);
            }
        }
        for (k, &(_, v)) in ids.iter().enumerate() {
            down[k] = down[k] || disabled(&w, v);
        }
        let side_out = |p: u8| units.iter().enumerate().all(|(k, x)| x.p != p || down[k]);
        if side_out(0) || side_out(1) {
            break;
        }
    }
    for (k, &(_, v)) in ids.iter().enumerate() {
        out.dead[k] = down[k];
        let left = vstate(&w, v).map_or(0, |s| {
            s.ammo.get(SPECIAL as usize).copied().unwrap_or(0) as u32
        });
        out.rare_fired[k] = rare0[k].saturating_sub(left);
    }
    Ok(out)
}

/// The fight over every (seed, flip) run, with O1 on and off.
fn runs(
    env: &Env,
    units: &[Unit],
    drive: Option<(usize, usize)>,
    secs: u64,
) -> Result<[Vec<Out>; 2]> {
    let mut on = Vec::new();
    let mut off = Vec::new();
    for (seed, flip) in env.runs() {
        on.push(fight(env, seed, flip == 1, true, units, drive, secs)?);
        off.push(fight(env, seed, flip == 1, false, units, drive, secs)?);
    }
    Ok([on, off])
}

/// Hits on units of player `p` by zone: "zone hits/pens".
fn zones(outs: &[Out], units: &[Unit], p: u8) -> String {
    let mut m: BTreeMap<&str, (u32, u32)> = BTreeMap::new();
    for o in outs {
        for (t, z, pen) in &o.hits {
            if units[*t].p == p {
                let e = m.entry(z.as_str()).or_default();
                e.0 += 1;
                e.1 += u32::from(*pen);
            }
        }
    }
    if m.is_empty() {
        return "none".into();
    }
    m.iter()
        .map(|(z, (h, p))| format!("{z} {h}/{p}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Penetrations on player `p`'s units in zone `zone`.
fn pens_in(outs: &[Out], units: &[Unit], p: u8, zone: &str) -> u32 {
    outs.iter()
        .flat_map(|o| o.hits.iter())
        .filter(|(t, z, pen)| units[*t].p == p && z == zone && *pen)
        .count() as u32
}

/// Runs in which every unit of player `p` stood and every enemy was down.
fn wins(outs: &[Out], units: &[Unit], p: u8) -> u32 {
    outs.iter()
        .filter(|o| {
            units
                .iter()
                .enumerate()
                .all(|(k, x)| if x.p == p { !o.dead[k] } else { o.dead[k] })
        })
        .count() as u32
}

fn median_first_pen(outs: &[Out]) -> String {
    let mut v: Vec<f64> = outs.iter().filter_map(|o| o.first_pen_s).collect();
    v.sort_by(f64::total_cmp);
    v.get(v.len() / 2)
        .map_or("none".into(), |x| format!("{x:.1} s"))
}

/// The common rows of one variant (O1 on or the control).
fn rows(r: &mut CaseReport, tag: &str, outs: &[Out], units: &[Unit]) {
    r.row(
        format!("{tag}: wins Germany / USA of {}", outs.len()),
        format!(
            "{} / {}; first penetration (median) {}",
            wins(outs, units, 0),
            wins(outs, units, 1),
            median_first_pen(outs)
        ),
    );
    r.row(
        format!("{tag}: hits on Germany by zone (hits/pens)"),
        zones(outs, units, 0),
    );
    r.row(
        format!("{tag}: hits on USA by zone (hits/pens)"),
        zones(outs, units, 1),
    );
}

/// The `o1_off` control's value of check `id`, as a check that always
/// passes (the targets file reads it as the row's control).
fn control(r: &mut CaseReport, id: &str, value: f64) {
    r.check_v(
        format!("{id}_o1_off"),
        true,
        value,
        "report",
        "o1_off control run",
    );
}

fn tags() -> [&'static str; 2] {
    ["O1", "o1_off"]
}

/// Case 41: Panther G frontal at 350 m against two Shermans (75) 45°
/// either side of its nose (report).
pub(super) fn case41(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(41, "Panther G at 350 m against two Shermans (75) from 45°");
    // 350 m at ±45° from (300, 500), each facing the Panther.
    let units = [
        u(0, PANTHER, 300, 500, 0),
        u(1, SHERMAN, 547, 253, 135),
        u(1, SHERMAN, 547, 747, 225),
    ];
    let outs = runs(env, &units, None, 120)?;
    r.runs = outs[0].len() as u32;
    for (k, o) in outs.iter().enumerate() {
        rows(&mut r, tags()[k], o, &units);
    }
    Ok(r)
}

/// Case 42: M36 against a Panther, frontal 350 m. Bound: no glacis
/// penetration (`superstructure_front`).
pub(super) fn case42(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(42, "M36 against a Panther, frontal 350 m");
    let units = [u(0, PANTHER, 325, 500, 0), u(1, M36, 675, 500, 180)];
    let outs = runs(env, &units, None, 120)?;
    r.runs = outs[0].len() as u32;
    for (k, o) in outs.iter().enumerate() {
        rows(&mut r, tags()[k], o, &units);
    }
    let g = pens_in(&outs[0], &units, 0, "superstructure_front");
    r.check_v(
        "glacis_pens",
        g == 0,
        g as f64,
        "0",
        format!(
            "o1_off: {}",
            pens_in(&outs[1], &units, 0, "superstructure_front")
        ),
    );
    let c = pens_in(&outs[1], &units, 0, "superstructure_front");
    control(&mut r, "glacis_pens", c as f64);
    Ok(r)
}

/// Case 43: M4A3 (76) W with its 2 HVAP against a Panther, frontal 350 m.
/// Bound: no glacis penetration; HVAP fired reported.
pub(super) fn case43(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(
        43,
        "M4A3 (76) W with 2 HVAP against a Panther, frontal 350 m",
    );
    let units = [u(0, PANTHER, 325, 500, 0), u(1, W76, 675, 500, 180)];
    let outs = runs(env, &units, None, 120)?;
    r.runs = outs[0].len() as u32;
    for (k, o) in outs.iter().enumerate() {
        rows(&mut r, tags()[k], o, &units);
        let hvap: u32 = o.iter().map(|x| x.rare_fired[1]).sum();
        r.row(
            format!("{}: HVAP fired by the 76 W", tags()[k]),
            hvap.to_string(),
        );
    }
    let g = pens_in(&outs[0], &units, 0, "superstructure_front");
    r.check_v(
        "glacis_pens",
        g == 0,
        g as f64,
        "0",
        format!(
            "o1_off: {}",
            pens_in(&outs[1], &units, 0, "superstructure_front")
        ),
    );
    let c = pens_in(&outs[1], &units, 0, "superstructure_front");
    control(&mut r, "glacis_pens", c as f64);
    Ok(r)
}

/// Case 44: Tiger against two Shermans (75), frontal 350 m. Bound: the
/// Tiger wins >= 6 of 10.
pub(super) fn case44(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(44, "Tiger against two Shermans (75), frontal 350 m");
    let units = [
        u(0, TIGER, 325, 500, 0),
        u(1, SHERMAN, 675, 485, 180),
        u(1, SHERMAN, 675, 515, 180),
    ];
    let outs = runs(env, &units, None, 120)?;
    r.runs = outs[0].len() as u32;
    for (k, o) in outs.iter().enumerate() {
        rows(&mut r, tags()[k], o, &units);
    }
    let n = outs[0].len() as u32;
    let won = wins(&outs[0], &units, 0);
    r.check_v(
        "tiger_wins",
        won * 10 >= 6 * n,
        won as f64,
        ">= 60 % of runs",
        format!("{won} of {n}; o1_off {}", wins(&outs[1], &units, 0)),
    );
    control(&mut r, "tiger_wins", wins(&outs[1], &units, 0) as f64);
    Ok(r)
}

/// Case 45: a Jumbo driving at a PaK 40 from 400 m. Bound: in >= 6 of 10
/// runs none of the first three hits on the Jumbo disables it (a run whose
/// Jumbo took fewer hits and stood counts too).
pub(super) fn case45(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(45, "Jumbo leading into a PaK 40 at 400 m");
    let units = [u(0, PAK, 300, 500, 0), u(1, JUMBO, 700, 500, 180)];
    let outs = runs(env, &units, Some((1, 0)), 120)?;
    r.runs = outs[0].len() as u32;
    let ok = |o: &Out| !o.dead[1] || o.hits_before_down[1] >= 3;
    for (k, o) in outs.iter().enumerate() {
        rows(&mut r, tags()[k], o, &units);
        let hits: Vec<String> = o
            .iter()
            .map(|x| x.hits_before_down[1].to_string())
            .collect();
        r.row(
            format!(
                "{}: hits on the Jumbo before it was disabled, per run",
                tags()[k]
            ),
            hits.join(" "),
        );
        let rare: u32 = o.iter().map(|x| x.rare_fired[0]).sum();
        r.row(
            format!("{}: PzGr 40 fired by the PaK 40", tags()[k]),
            rare.to_string(),
        );
    }
    let n = outs[0].len() as u32;
    let good = outs[0].iter().filter(|o| ok(o)).count() as u32;
    r.check_v(
        "jumbo_survives",
        good * 10 >= 6 * n,
        good as f64,
        ">= 60 % of runs",
        format!(
            "{good} of {n}; o1_off {}",
            outs[1].iter().filter(|o| ok(o)).count()
        ),
    );
    control(
        &mut r,
        "jumbo_survives",
        outs[1].iter().filter(|o| ok(o)).count() as f64,
    );
    Ok(r)
}

/// Case 46: a 76 W 200 m off a Panther's flank (the Panther faces north,
/// turret on its hull line). Bound: the Panther is put out in >= 7 of 10.
pub(super) fn case46(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(46, "M4A3 (76) W 200 m on a Panther's flank");
    let units = [u(0, PANTHER, 400, 500, 90), u(1, W76, 600, 500, 180)];
    let outs = runs(env, &units, None, 120)?;
    r.runs = outs[0].len() as u32;
    for (k, o) in outs.iter().enumerate() {
        rows(&mut r, tags()[k], o, &units);
    }
    let n = outs[0].len() as u32;
    let kills = outs[0].iter().filter(|o| o.dead[0]).count() as u32;
    r.check_v(
        "panther_out",
        kills * 10 >= 7 * n,
        kills as f64,
        ">= 70 % of runs",
        format!(
            "{kills} of {n}; o1_off {}",
            outs[1].iter().filter(|o| o.dead[0]).count()
        ),
    );
    control(
        &mut r,
        "panther_out",
        outs[1].iter().filter(|o| o.dead[0]).count() as f64,
    );
    Ok(r)
}
