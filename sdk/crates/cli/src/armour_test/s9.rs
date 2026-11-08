//! S9 armour-test cases (`docs/slices/S9-at-guns.md` 15): MG against a
//! shielded crew (33), the ambush (34 Pak 40 v Sherman, 35 57 mm v Pz IV),
//! the gun optic inside the arc (37). Build B and C add 36, 38-40.

use anyhow::{Result, anyhow};
use frontline_content::vehicle::VehicleSetup;
use frontline_sim::scenarios::{at, dir_deg, members};
use frontline_sim::unit::StanceOrder;
use frontline_sim::{ArmourEventKind, Fx, FxVec2, FxVec3, PenResult, SimId, World};

use super::{
    CaseReport, CommandKind, Env, disabled, hz, open_top_world, order, pos, spawn_card, vstate,
    weapon_idx,
};
use crate::report::round3;

const GER_GUN: &str = "ww2:ger_pak40_squad";
const GER_CREW: &str = "ww2:ger_pak40_crew";
const USA_GUN: &str = "ww2:usa_57mm_m1_squad";
const USA_CREW: &str = "ww2:usa_57mm_crew";
const GER_TANK: &str = "ww2:ger_pz4_h_squad";
const USA_TANK: &str = "ww2:usa_m4a3_75_squad";

/// `(owner, gun card, crew card)` of side `flip`.
fn gun_side(flip: u8) -> (u8, &'static str, &'static str) {
    if flip == 0 {
        (0, GER_GUN, GER_CREW)
    } else {
        (1, USA_GUN, USA_CREW)
    }
}

/// A pseudo-random triangular value in `[-1, 1]`, repeatable from `(a, b)`.
fn tri(a: u64, b: u64) -> f64 {
    let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x % 100_000) as f64 / 100_000.0
    };
    next() + next() - 1.0
}

fn gun_def(w: &World, v: SimId) -> Result<&VehicleSetup> {
    let t = vstate(w, v).ok_or_else(|| anyhow!("no gun"))?.type_idx;
    w.match_setup()
        .map(|s| &s.vehicles[t as usize])
        .ok_or_else(|| anyhow!("no setup"))
}

/// Case 33's firing mode.
#[derive(Copy, Clone, PartialEq)]
enum Fire {
    /// The same crew in the open, shot at from the front.
    Open,
    /// Serving a gun, shot at from the front.
    Front,
    /// Serving a gun, from 45° off the front.
    Flank,
}

/// One series of MG rounds at the crew: `(rounds that struck a man, rounds
/// stopped on the gun, rounds fired)`. Every man is made whole after each
/// round, so the count is hits per round and never saturates.
fn mg_series(env: &Env, seed: u64, flip: u8, mode: Fire) -> Result<(u64, u64, u64)> {
    const ROUNDS: u64 = 500;
    const RANGE_M: i32 = 200;
    let (own, gun_card, crew_card) = gun_side(flip);
    let shooter = 1 - own;
    let mut w = open_top_world(env, seed)?;
    let gun_at = at(500, 500);
    let (_, v) = spawn_card(&mut w, own, gun_card, gun_at, dir_deg(0))?;
    let def = gun_def(&w, v)?.clone();
    let served = vstate(&w, v)
        .and_then(|x| x.served)
        .ok_or_else(|| anyhow!("no crew"))?;
    let crew: Vec<SimId> = served.slots.iter().filter_map(|s| s.man).collect();
    // The men's places, read off the gun.
    let spots: Vec<FxVec2> = crew.iter().map(|&m| pos(&w, m)).collect();
    let men = if mode == Fire::Open {
        // A second world holds the same crew in the open.
        let mut open = open_top_world(env, seed)?;
        let setup = open.match_setup().ok_or_else(|| anyhow!("setup"))?;
        let t = setup
            .squad_type_index(crew_card)
            .ok_or_else(|| anyhow!("crew card"))?;
        let sq = open
            .spawn_squad_at(frontline_sim::PlayerId(own), t, gun_at, dir_deg(0))
            .ok_or_else(|| anyhow!("no crew squad"))?;
        let m = members(&open, sq);
        for (&a, &q) in m.iter().zip(&spots) {
            open.place_soldier(a, q);
            open.edit_unit(a, |u| {
                u.stance = StanceOrder::Crouch;
                // Detached and idle: he holds this spot, not a formation slot.
                u.detached = true;
                u.nav.goal = q;
                u.nav.phase = frontline_sim::NavPhase::Holding;
            });
        }
        w = open;
        m
    } else {
        crew
    };
    let mg = weapon_idx(&w, "ww2:mg42_bipod")?;
    let spread = w.match_setup().map_or(0.0, |s| {
        f64::from(Fx::from(s.weapons[mg as usize].spread).to_f32())
    });
    let half = f64::from(RANGE_M) * spread;
    let bearing = if mode == Fire::Flank { 45 } else { 0 };
    let dir = dir_deg(bearing);
    let right = FxVec2::new(-dir.y, dir.x);
    for _ in 0..hz(&w) {
        w.step(&[]);
    }
    let full = def.service.len().max(1);
    let (mut hits, mut stops, mut fired) = (0u64, 0u64, 0u64);
    for k in 0..ROUNDS {
        let m = men[k as usize % men.len().min(full)];
        let p = w.entities().transform()[m.index()].pos;
        let chest = p + FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::from_ratio(6, 10));
        let (ny, nz) = (
            tri(seed + flip as u64, k * 2 + 1) * half,
            tri(seed + flip as u64, k * 2 + 2) * half,
        );
        let aim = chest
            + (right * Fx::from_milli((ny * 1000.0) as i32))
                .extend(Fx::from_milli((nz * 1000.0) as i32));
        let from =
            (chest.xy() + dir * Fx::from_int(RANGE_M)).extend(chest.z + Fx::from_ratio(1, 10));
        let hp0: Vec<u16> = men
            .iter()
            .map(|&a| w.entities().combat()[a.index()].hp)
            .collect();
        let stopped0 = w.stats().rounds_stopped;
        let pens0 = w.stats().small_arms_pens;
        if !w.fire_round_now(shooter, mg, from, aim) {
            continue;
        }
        fired += 1;
        for _ in 0..12 {
            w.step(&[]);
        }
        stops += w.stats().rounds_stopped - stopped0 + w.stats().small_arms_pens - pens0;
        for (&a, &h) in men.iter().zip(&hp0) {
            let now = w.entities().combat()[a.index()].hp;
            if now < h {
                hits += 1;
            }
            w.edit_soldier(a, |_, c| {
                c.hp = h;
                c.suppression = 0;
                c.pinned = false;
            });
        }
    }
    Ok((hits, stops, fired))
}

/// Case 33 (S9 done-when 1): MG rounds that miss the shield reach the men,
/// rounds that strike it stop; from 45° the flank men are in the open.
pub(super) fn case33(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(33, "MG against a shielded crew (done-when)");
    let (mut open, mut front, mut flank) = (0u64, 0u64, 0u64);
    let (mut stops, mut fired) = (0u64, 0u64);
    for (seed, flip) in env.runs() {
        r.runs += 1;
        open += mg_series(env, seed * 11 + flip as u64, flip, Fire::Open)?.0;
        let (h, st, n) = mg_series(env, seed * 13 + flip as u64, flip, Fire::Front)?;
        front += h;
        stops += st;
        fired += n;
        flank += mg_series(env, seed * 17 + flip as u64, flip, Fire::Flank)?.0;
    }
    let pct = 100.0 * front as f64 / open.max(1) as f64;
    let ratio = flank as f64 / front.max(1) as f64;
    let stop_pct = 100.0 * stops as f64 / fired.max(1) as f64;
    r.check_v(
        "front_losses_pct_of_open",
        pct <= 30.0,
        round3(pct),
        "<= 30",
        format!("{front} hits with the gun, {open} on the same crew in the open"),
    );
    r.check_v(
        "flank45_over_front",
        ratio >= 3.0,
        round3(ratio),
        ">= 3",
        format!("{flank} hits from 45 deg, {front} frontal"),
    );
    r.check_v(
        "front_shield_stops_pct",
        true,
        round3(stop_pct),
        "report",
        format!("{stops} of {fired} rounds stopped on the gun"),
    );
    Ok(r)
}

/// An ambush: gun `gun_card` (with its crew) of player `own` holding fire
/// to 400 m against tank `tank_card` driving at it. `(gun fired first,
/// tank disabled, hits on the tank, gun rounds fired, seconds to the first
/// shot)`.
fn ambush(
    env: &Env,
    seed: u64,
    mirror: bool,
    own: u8,
    gun_card: &str,
    tank_card: &str,
) -> Result<(bool, bool, u64, u64, f64)> {
    let mut w = open_top_world(env, seed)?;
    w.enable_hit_log();
    let mx = |x: i32| if mirror { 1000 - x } else { x };
    let yaw = if mirror { 180 } else { 0 };
    let (_, gun) = spawn_card(&mut w, own, gun_card, at(mx(150), 500), dir_deg(yaw))?;
    let (tsq, tank) = spawn_card(
        &mut w,
        1 - own,
        tank_card,
        at(mx(760), 500),
        dir_deg(180 - yaw),
    )?;
    w.edit_vehicle(gun, |vs| {
        if let Some(sv) = vs.served.as_mut() {
            sv.hold_fire_m = 400;
        }
    });
    order(
        &mut w,
        1 - own,
        vec![CommandKind::Move {
            squad: tsq,
            target: at(mx(240), 500),
        }],
    );
    let rate = hz(&w);
    let limit = w.tick() + 150 * rate;
    let (mut first, mut hits, mut rounds, mut at_s) = (None, 0u64, 0u64, 0.0f64);
    let start = w.tick();
    while w.tick() < limit && !disabled(&w, tank) && !disabled(&w, gun) {
        w.step(&[]);
        for e in &w.snapshot().armour_events {
            if e.kind != ArmourEventKind::Fired as u8 {
                continue;
            }
            if e.vehicle == gun.0 {
                rounds += 1;
                if first.is_none() {
                    first = Some(true);
                    at_s = (w.tick() - start) as f64 / rate as f64;
                }
            } else if e.vehicle == tank.0 && first.is_none() {
                first = Some(false);
            }
        }
        hits += w
            .take_hit_log()
            .iter()
            .filter(|h| h.vehicle == tank && h.result == PenResult::Penetration)
            .count() as u64;
    }
    Ok((
        first.unwrap_or(false),
        disabled(&w, tank),
        hits,
        rounds,
        at_s,
    ))
}

fn ambush_case(
    env: &Env,
    r: &mut CaseReport,
    own: u8,
    gun: &str,
    tank: &str,
    hard: bool,
) -> Result<()> {
    let (mut first, mut dis, mut hits, mut rounds, mut runs) = (0u32, 0u32, 0u64, 0u64, 0u32);
    let mut secs = 0.0f64;
    for (seed, flip) in env.runs() {
        for rep in 0..5u64 {
            let (f, d, h, n, t) = ambush(env, seed * 19 + rep * 7, flip == 1, own, gun, tank)?;
            r.runs += 1;
            runs += 1;
            first += u32::from(f);
            dis += u32::from(d);
            hits += h;
            rounds += n;
            secs += t;
        }
    }
    let n = f64::from(runs.max(1));
    let fp = 100.0 * f64::from(first) / n;
    let dp = 100.0 * f64::from(dis) / n;
    r.check_v(
        "fires_first_pct",
        !hard || fp >= 80.0,
        round3(fp),
        if hard { ">= 80 (T74a)" } else { "report" },
        format!("{first} of {runs} runs"),
    );
    r.check_v(
        "disabled_pct",
        !hard || dp >= 60.0,
        round3(dp),
        if hard { ">= 60 (T74b)" } else { "report" },
        format!("{dis} of {runs} runs disabled"),
    );
    r.check_v(
        "hits_per_disable",
        true,
        round3(hits as f64 / f64::from(dis.max(1))),
        "report",
        format!("{hits} penetrations, {dis} disabled; {rounds} gun rounds"),
    );
    r.row("first_shot_s", format!("{:.1} mean", secs / n));
    Ok(())
}

/// Case 34 (S9 done-when 2): a concealed Pak 40 holding fire to 400 m.
pub(super) fn case34(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(34, "ambush: Pak 40 v advancing Sherman (done-when)");
    ambush_case(env, &mut r, 0, GER_GUN, USA_TANK, true)?;
    Ok(r)
}

/// Case 35 (T74d/e, report): the 57 mm against an advancing Pz IV.
pub(super) fn case35(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(35, "ambush: 57 mm M1 v advancing Panzer IV");
    ambush_case(env, &mut r, 1, USA_GUN, GER_TANK, false)?;
    Ok(r)
}

/// The farthest ladder distance at which `gun` (with its crew, player
/// `own`) sees a stationary enemy tank on bearing `bearing` from the gun's
/// front. Each distance is a fresh world (no carried-over sighting).
fn seen_at(
    env: &Env,
    own: u8,
    card: &str,
    tank: &str,
    bearing: i32,
    ladder: &[i32],
) -> Result<i32> {
    let mut best = 0;
    for &d in ladder {
        let mut w = open_top_world(env, 37)?;
        let (_, gun) = spawn_card(&mut w, own, card, at(100, 500), dir_deg(0))?;
        let dir = dir_deg(bearing);
        let t = at(100, 500) + dir * Fx::from_int(d);
        let (_, v) = spawn_card(&mut w, 1 - own, tank, t, -dir)?;
        // The target fires nothing and stays stationary.
        for _ in 0..3 * hz(&w) {
            w.step(&[]);
        }
        let _ = gun;
        if w.entities().combat()[v.index()].seen_by & (1 << own) != 0 {
            best = best.max(d);
        }
    }
    Ok(best)
}

/// Case 37 (T76, owner question 12 (A)): the optic reaches 437 m inside the
/// arc against a stationary tank and not outside it.
pub(super) fn case37(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(37, "gun optic inside the arc");
    r.runs = 2;
    let ladder = [100, 300, 360, 380, 400, 420, 430, 440, 460, 480];
    let in_arc = seen_at(env, 0, GER_GUN, USA_TANK, 20, &ladder)?;
    let out = seen_at(
        env,
        0,
        GER_GUN,
        USA_TANK,
        60,
        &[300, 320, 340, 350, 360, 380, 400, 420],
    )?;
    r.check_v(
        "sees_tank_in_arc_m",
        in_arc as f64 >= 400.0,
        f64::from(in_arc),
        ">= 400",
        "farthest ladder step, stationary Sherman 20 deg off the gun's front".to_string(),
    );
    r.check_v(
        "sees_tank_out_arc_m",
        out as f64 <= 350.0,
        f64::from(out),
        "<= 350 (no optic: sight 175 m x the tank's 200 %)",
        "60 deg off the front (outside the +-32 deg arc)".to_string(),
    );
    Ok(r)
}
