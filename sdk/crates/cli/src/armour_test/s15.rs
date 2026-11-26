//! S15 Build C armour-test cases 47-56: row W5 of `docs/slices/S15-ussr.md`
//! 10, the Soviet guns and vehicles against the German armour. Report
//! rows only: no case has a threshold (plan: "each row prints kills and
//! rounds; none has a threshold").
//!
//! 47 T-34-85 against a Panzer IV H, frontal 500 m (both shoot: both ways);
//! 48 T-34-85 against a Panther's front, 500 m; 49 T-34-85 500 m off a
//! Panther's flank; 50 IS-2 against a Panther, frontal 800 m; 51 IS-2
//! against a Tiger, frontal 800 m; 52 SU-85 against a Tiger, frontal 500
//! m; 53 ZiS-2 500 m off a Panther's flank; 54 45 mm M-42 against a Panzer
//! IV H, frontal 500 m; 55 the PTRD team 100 m from an Sd.Kfz. 251; 56 the
//! PTRD team 100 m off a Panzer IV H's flank.
//!
//! Staging as S13's cases 41-46 (`s13.rs`): open flat ground, every
//! vehicle's sight raised to 1 km (staged as spotted), crews never bail,
//! rule O1 on (the default). A vehicle observer's sight saturates at about
//! 327 m in the sim (`sight · pct` overflows Q16.16 in `spotting.rs`), so
//! a 200 % tank is seen to about 655 m whatever the sight: in the 800 m
//! cases every vehicle's `spot_size_pct` is raised to 300 (seen to about
//! 980 m) and the guns' engagement range (600 m, every cannon of the mod)
//! to 1 km, so they fire at all. The German side is player 0, the Soviet
//! side player 1 (a Germany-USA setup: the USSR cards are in every setup
//! and spawn by card). Rounds: main-gun rounds a vehicle fired (racks and
//! breech before minus after). The PTRD team is infantry: its row gives
//! the men it lost. The hit log holds shell hits only, so a PTRD hole
//! shows in the target's "disabled in", not in its zones.

use std::collections::BTreeMap;

use anyhow::Result;
use frontline_content::Fixed;
use frontline_sim::armour::PenResult;
use frontline_sim::scenarios::{at, dir_deg, members};
use frontline_sim::{EntityKind, SimId, SquadId, World};

use super::{CaseReport, Env, alive, disabled, hz, open_top_from, spawn_card, vstate};

const PZ4: &str = "ww2:ger_pz4_h_squad";
const PANTHER: &str = "ww2:ger_panther_g_squad";
const TIGER: &str = "ww2:ger_tiger_1_squad";
const SDKFZ251: &str = "ww2:ger_sdkfz251_1_squad";
const T34: &str = "ww2:ussr_t34_85_squad";
const IS2: &str = "ww2:ussr_is2_squad";
const SU85: &str = "ww2:ussr_su85_squad";
const ZIS2: &str = "ww2:ussr_57mm_zis2_squad";
const M42: &str = "ww2:ussr_45mm_m42_squad";
const PTRD: &str = "ww2:ussr_raschyot_ptr";

/// One unit: player, card, position (m), facing (deg counter-clockwise
/// from east).
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

/// What one run left: per unit whether it ended disabled (vehicles only),
/// the main-gun rounds it used, the hits it took (zone name, penetrated).
#[derive(Default)]
struct Out {
    vehicle: Vec<bool>,
    dead: Vec<bool>,
    rounds: Vec<u32>,
    men_lost: Vec<u32>,
    hits: Vec<(usize, String, bool)>,
}

/// Main-gun rounds on board: the racks and the round in the breech.
fn rounds_left(w: &World, v: SimId) -> u32 {
    vstate(w, v).map_or(0, |s| {
        s.ammo.iter().map(|&n| n as u32).sum::<u32>() + u32::from(s.loaded.is_some())
    })
}

/// Living men of an infantry unit's squad.
fn men(w: &World, sq: SquadId) -> u32 {
    let kind = w.entities().kind();
    members(w, sq)
        .iter()
        .filter(|m| kind.get(m.index()) == Some(&EntityKind::Soldier))
        .count() as u32
}

/// Disabled as the S13 cases count it; a vehicle without a main gun (the
/// half-track) only when knocked out or immobilised.
fn down(w: &World, v: SimId) -> bool {
    let Some(vs) = vstate(w, v) else {
        return true;
    };
    let s = w.match_setup().expect("match");
    let def = &s.vehicles[vs.type_idx as usize];
    if def.main_gun().is_some() {
        return disabled(w, v);
    }
    !alive(w, v) || vs.wreck_tick.is_some() || vs.immobilised(def)
}

/// Plays `units` (mirrored east-west with `flip`) for at most `secs`, or
/// until one side has no live vehicle (a side of infantry only never runs
/// out: the run lasts `secs`). `range_m`: the cannons' engagement range.
fn fight(env: &Env, seed: u64, flip: bool, units: &[Unit], range_m: i32, secs: u64) -> Result<Out> {
    let mut s = env.setup(seed)?;
    let mut w: World = open_top_from(&mut s, env.threads, |s| {
        for v in &mut s.vehicles {
            v.sight = Fixed::from_int(1000);
        }
        if range_m > 600 {
            for v in &mut s.vehicles {
                v.spot_size_pct = v.spot_size_pct.max(300);
            }
            for wp in &mut s.weapons {
                if wp.range_max == Fixed::from_int(600) {
                    wp.range_max = Fixed::from_int(range_m);
                }
            }
        }
    })?;
    let mx = |x: i32| if flip { 1000 - x } else { x };
    let md = |d: i32| if flip { 180 - d } else { d };
    let mut ids = Vec::new();
    for x in units {
        ids.push(spawn_card(
            &mut w,
            x.p,
            x.card,
            at(mx(x.x), x.y),
            dir_deg(md(x.deg)),
        )?);
    }
    let vehicle: Vec<bool> = ids.iter().map(|&(_, v)| vstate(&w, v).is_some()).collect();
    let start_rounds: Vec<u32> = ids.iter().map(|&(_, v)| rounds_left(&w, v)).collect();
    let start_men: Vec<u32> = ids.iter().map(|&(sq, _)| men(&w, sq)).collect();
    w.enable_hit_log();
    let rate = hz(&w);
    let limit = w.tick() + secs * rate;
    let n = units.len();
    let mut out = Out {
        dead: vec![false; n],
        rounds: vec![0; n],
        men_lost: vec![0; n],
        vehicle: vehicle.clone(),
        ..Out::default()
    };
    let unit_of = |v: SimId| ids.iter().position(|&(_, x)| x == v);
    while w.tick() < limit {
        w.step(&[]);
        let setup = w.match_setup().expect("match").clone();
        for h in w.take_hit_log() {
            let Some(t) = unit_of(h.vehicle) else {
                continue;
            };
            let vdef = &setup.vehicles[vstate(&w, h.vehicle).map_or(0, |v| v.type_idx) as usize];
            out.hits.push((
                t,
                vdef.zone_name(h.zone),
                h.result == PenResult::Penetration,
            ));
        }
        for (k, &(_, v)) in ids.iter().enumerate() {
            out.dead[k] = out.dead[k] || (vehicle[k] && down(&w, v));
        }
        let side_out = |p: u8| {
            units
                .iter()
                .enumerate()
                .filter(|(k, x)| x.p == p && vehicle[*k])
                .count()
                > 0
                && units
                    .iter()
                    .enumerate()
                    .all(|(k, x)| x.p != p || !vehicle[k] || out.dead[k])
        };
        if side_out(0) || side_out(1) {
            break;
        }
    }
    for (k, &(_, v)) in ids.iter().enumerate() {
        if vehicle[k] {
            out.rounds[k] = start_rounds[k].saturating_sub(rounds_left(&w, v));
        } else {
            out.men_lost[k] = start_men[k].saturating_sub(men(&w, ids[k].0));
        }
    }
    Ok(out)
}

/// Hits on unit `t` by zone: "zone hits/pens".
fn zones(outs: &[Out], t: usize) -> String {
    let mut m: BTreeMap<&str, (u32, u32)> = BTreeMap::new();
    for o in outs {
        for (k, z, pen) in &o.hits {
            if *k == t {
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

/// Plays the case over every (seed, flip) run and writes its rows: per
/// unit the runs it ended disabled ("kills" of it), the main-gun rounds it
/// used (vehicles) and the hits it took by zone.
fn duel(
    env: &Env,
    case: u32,
    name: &'static str,
    units: &[Unit],
    range_m: i32,
    secs: u64,
) -> Result<CaseReport> {
    let mut r = CaseReport::new(case, name);
    let mut outs = Vec::new();
    for (seed, flip) in env.runs() {
        outs.push(fight(env, seed, flip == 1, units, range_m, secs)?);
    }
    r.runs = outs.len() as u32;
    for (k, x) in units.iter().enumerate() {
        let infantry = outs.first().is_some_and(|o| !o.vehicle[k]);
        let who = format!(
            "{} {}",
            if x.p == 0 { "Germany" } else { "USSR" },
            x.card.trim_start_matches("ww2:")
        );
        if infantry {
            let lost: u32 = outs.iter().map(|o| o.men_lost[k]).sum();
            r.row(format!("{who}: men lost (all runs)"), lost.to_string());
            continue;
        }
        let kills = outs.iter().filter(|o| o.dead[k]).count();
        let rounds: u32 = outs.iter().map(|o| o.rounds[k]).sum();
        let pens: u32 = outs
            .iter()
            .flat_map(|o| o.hits.iter())
            .filter(|(t, _, p)| *t == k && *p)
            .count() as u32;
        r.row(
            format!("{who}: disabled in / rounds used / penetrations taken"),
            format!("{kills} of {} / {rounds} / {pens}", outs.len()),
        );
        r.row(
            format!("{who}: hits taken by zone (hits/pens)"),
            zones(&outs, k),
        );
    }
    Ok(r)
}

/// Case 47: T-34-85 against a Panzer IV H, frontal 500 m (report).
pub(super) fn case47(env: &Env) -> Result<CaseReport> {
    let units = [u(0, PZ4, 250, 500, 0), u(1, T34, 750, 500, 180)];
    duel(
        env,
        47,
        "W5: T-34-85 against a Panzer IV H, frontal 500 m",
        &units,
        600,
        120,
    )
}

/// Case 48: T-34-85 against a Panther's front, 500 m (report).
pub(super) fn case48(env: &Env) -> Result<CaseReport> {
    let units = [u(0, PANTHER, 250, 500, 0), u(1, T34, 750, 500, 180)];
    duel(
        env,
        48,
        "W5: T-34-85 against a Panther's front, 500 m",
        &units,
        600,
        120,
    )
}

/// Case 49: T-34-85 500 m off a Panther's flank (the Panther faces north,
/// turret on its hull line; report).
pub(super) fn case49(env: &Env) -> Result<CaseReport> {
    let units = [u(0, PANTHER, 250, 500, 90), u(1, T34, 750, 500, 180)];
    duel(
        env,
        49,
        "W5: T-34-85 500 m off a Panther's flank",
        &units,
        600,
        120,
    )
}

/// Case 50: IS-2 against a Panther, frontal 800 m (report).
pub(super) fn case50(env: &Env) -> Result<CaseReport> {
    let units = [u(0, PANTHER, 100, 500, 0), u(1, IS2, 900, 500, 180)];
    duel(
        env,
        50,
        "W5: IS-2 against a Panther, frontal 800 m",
        &units,
        1000,
        180,
    )
}

/// Case 51: IS-2 against a Tiger, frontal 800 m (report).
pub(super) fn case51(env: &Env) -> Result<CaseReport> {
    let units = [u(0, TIGER, 100, 500, 0), u(1, IS2, 900, 500, 180)];
    duel(
        env,
        51,
        "W5: IS-2 against a Tiger, frontal 800 m",
        &units,
        1000,
        180,
    )
}

/// Case 52: SU-85 against a Tiger, frontal 500 m (report).
pub(super) fn case52(env: &Env) -> Result<CaseReport> {
    let units = [u(0, TIGER, 250, 500, 0), u(1, SU85, 750, 500, 180)];
    duel(
        env,
        52,
        "W5: SU-85 against a Tiger, frontal 500 m",
        &units,
        600,
        120,
    )
}

/// Case 53: ZiS-2 500 m off a Panther's flank (report).
pub(super) fn case53(env: &Env) -> Result<CaseReport> {
    let units = [u(0, PANTHER, 250, 500, 90), u(1, ZIS2, 750, 500, 180)];
    duel(
        env,
        53,
        "W5: ZiS-2 500 m off a Panther's flank",
        &units,
        600,
        120,
    )
}

/// Case 54: 45 mm M-42 against a Panzer IV H, frontal 500 m (report).
pub(super) fn case54(env: &Env) -> Result<CaseReport> {
    let units = [u(0, PZ4, 250, 500, 0), u(1, M42, 750, 500, 180)];
    duel(
        env,
        54,
        "W5: 45 mm M-42 against a Panzer IV H, frontal 500 m",
        &units,
        600,
        120,
    )
}

/// Case 55: the PTRD team 100 m from an Sd.Kfz. 251 (front; report).
pub(super) fn case55(env: &Env) -> Result<CaseReport> {
    let units = [u(0, SDKFZ251, 450, 500, 0), u(1, PTRD, 550, 500, 180)];
    duel(
        env,
        55,
        "W5: PTRD team 100 m from an Sd.Kfz. 251",
        &units,
        600,
        90,
    )
}

/// Case 56: the PTRD team 100 m off a Panzer IV H's flank (report).
pub(super) fn case56(env: &Env) -> Result<CaseReport> {
    let units = [u(0, PZ4, 450, 500, 90), u(1, PTRD, 550, 500, 180)];
    duel(
        env,
        56,
        "W5: PTRD team 100 m off a Panzer IV H's flank",
        &units,
        600,
        90,
    )
}
