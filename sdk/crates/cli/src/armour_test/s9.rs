//! S9 armour-test cases (`docs/slices/S9-at-guns.md` 15): MG against a
//! shielded crew (33), the ambush (34 Pak 40 v Sherman, 35 57 mm v Pz IV),
//! the gun optic inside the arc (37), recrew and capture (38), mobility
//! and the tow (39), the gun pit (40). Build C adds 36.

use anyhow::{Result, anyhow};
use frontline_content::vehicle::VehicleSetup;
use frontline_sim::scenarios::{at, dir_deg, members};
use frontline_sim::unit::StanceOrder;
use frontline_sim::vehicle::GunState;
use frontline_sim::{
    ArmourEventKind, EntityKind, Fx, FxVec2, FxVec3, PenResult, PlayerId, SimId, World,
};

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
    mg_series_at(env, seed, flip, mode, false)
}

/// Case 40's frontal MG series with the gun in a dug pit.
fn mg_series_in_pit(env: &Env, seed: u64, flip: u8) -> Result<(u64, u64, u64)> {
    mg_series_at(env, seed, flip, Fire::Front, true)
}

fn mg_series_at(env: &Env, seed: u64, flip: u8, mode: Fire, pit: bool) -> Result<(u64, u64, u64)> {
    const ROUNDS: u64 = 500;
    const RANGE_M: i32 = 200;
    let (own, gun_card, crew_card) = gun_side(flip);
    let shooter = 1 - own;
    let mut w = open_top_world(env, seed)?;
    let gun_at = at(500, 500);
    let (_, v) = spawn_card(&mut w, own, gun_card, gun_at, dir_deg(0))?;
    if pit {
        w.dig_pit_now(
            &[at(496, 500), at(500, 500)],
            frontline_content::DigProfile::GunPit,
        );
    }
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
        // In a pit the MG stands on the ground around it, not on the floor.
        let ground = if pit {
            Fx::from_ratio(55, 100)
        } else {
            Fx::ZERO
        };
        let from = (chest.xy() + dir * Fx::from_int(RANGE_M))
            .extend(chest.z + Fx::from_ratio(1, 10) + ground);
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

fn gun_state(w: &World, g: SimId) -> Option<GunState> {
    vstate(w, g).and_then(|v| v.served).map(|s| s.state)
}

/// Runs until `f` holds (at most `max_s` s); the seconds it took.
fn time_until(w: &mut World, max_s: u32, mut f: impl FnMut(&World) -> bool) -> Option<f64> {
    let rate = hz(w) as f64;
    let start = w.tick();
    while ((w.tick() - start) as f64) < max_s as f64 * rate {
        if f(w) {
            return Some((w.tick() - start) as f64 / rate);
        }
        w.step(&[]);
    }
    None
}

fn shells_def(w: &World, player: u8) -> Result<u16> {
    let s = w.match_setup().ok_or_else(|| anyhow!("setup"))?;
    let f = s.players[player as usize].faction;
    s.stocks
        .iter()
        .position(|d| {
            d.faction == f
                && d.kind == frontline_content::setup::StockKind::Crate
                && d.supply == Some(frontline_content::setup::SupplyKind::Shells)
        })
        .map(|k| k as u16)
        .ok_or_else(|| anyhow!("no shells crate"))
}

/// One capture run: `(swap s, neutral s, served, own-crate rounds, gun's
/// faction crate rounds, fired rounds left)`.
fn capture_run(env: &Env, seed: u64, flip: u8) -> Result<(f64, f64, bool, i64, i64, bool)> {
    let (own, card, _) = gun_side(flip);
    let enemy = 1 - own;
    let mut w = open_top_world(env, seed)?;
    let at0 = at(400, 500);
    let (_, g) = spawn_card(&mut w, own, card, at0, dir_deg(0))?;
    for _ in 0..hz(&w) {
        w.step(&[]);
    }
    let slots = |w: &World| vstate(w, g).and_then(|v| v.served).map(|s| s.slots);
    // The swap.
    let gunner = slots(&w)
        .and_then(|s| s[0].man)
        .ok_or_else(|| anyhow!("no gunner"))?;
    w.kill_soldier(gunner);
    w.step(&[]);
    let swap = time_until(&mut w, 12, |w: &World| {
        slots(w).is_some_and(|s| s[0].man.is_some() && s[0].swap == 0)
    })
    .ok_or_else(|| anyhow!("no swap"))?;
    // All dead: neutral.
    for s in slots(&w).unwrap_or_default() {
        if let Some(m) = s.man {
            w.kill_soldier(m);
        }
    }
    let neutral = time_until(&mut w, 30, |w: &World| {
        w.entities().kind()[g.index()] == EntityKind::Abandoned
    })
    .ok_or_else(|| anyhow!("never neutral"))?;
    // An enemy rifle squad serves it.
    let card_e = if enemy == 0 {
        "ww2:ger_grenadiergruppe"
    } else {
        "ww2:usa_rifle_squad"
    };
    let (rifles, _) = (spawn_soldiers(&mut w, enemy, card_e, at(405, 500))?, ());
    order(
        &mut w,
        enemy,
        vec![CommandKind::Recrew {
            squad: rifles,
            vehicle: g,
        }],
    );
    let served = time_until(&mut w, 40, |w: &World| {
        w.entities().kind()[g.index()] == EntityKind::Vehicle
            && w.entities().owner()[g.index()] == PlayerId(enemy)
            && slots(w).is_some_and(|s| s.iter().any(|x| x.man.is_some()))
    })
    .is_some();
    let ammo = |w: &World| vstate(w, g).map_or(0, |v| v.ammo[0] as i64);
    // The captor's own crate does not fit; a crate of the gun's faction does.
    let n0 = ammo(&w);
    w.edit_vehicle(g, |vs| vs.ammo[0] = (n0 as u16).saturating_sub(5));
    let spent = ammo(&w);
    let own_d = shells_def(&w, enemy)?;
    w.add_crate_now(PlayerId(enemy), own_d, at(401, 500));
    let rate = hz(&w);
    for _ in 0..30 * rate {
        w.step(&[]);
    }
    let own_rounds = ammo(&w) - spent;
    let gun_d = shells_def(&w, own)?;
    w.add_crate_now(PlayerId(enemy), gun_d, at(399, 500));
    for _ in 0..60 * rate {
        w.step(&[]);
    }
    let gun_rounds = ammo(&w) - spent - own_rounds;
    // The rounds at the gun fire at the old owner's tank in the arc.
    let tank = if own == 0 { GER_TANK } else { USA_TANK };
    let (_, _t) = spawn_card(&mut w, own, tank, at(610, 500), dir_deg(180))?;
    let left0 = ammo(&w);
    let mut fired = false;
    for _ in 0..40 * rate {
        w.step(&[]);
        if ammo(&w) < left0
            || w.snapshot()
                .armour_events
                .iter()
                .any(|e| e.kind == ArmourEventKind::Fired as u8 && e.vehicle == g.0)
        {
            fired = true;
            break;
        }
    }
    Ok((swap, neutral, served, own_rounds, gun_rounds, fired))
}

/// Spawns the soldier card `card` and returns its squad.
fn spawn_soldiers(w: &mut World, p: u8, card: &str, c: FxVec2) -> Result<frontline_sim::SquadId> {
    let t = w
        .match_setup()
        .and_then(|s| s.squad_type_index(card))
        .ok_or_else(|| anyhow!("no card {card}"))?;
    w.spawn_squad_at(PlayerId(p), t, c, dir_deg(0))
        .ok_or_else(|| anyhow!("cannot spawn {card}"))
}

/// Case 38 (T77, done-when 3): swap, neutral, an enemy squad serves the
/// gun, and the captured gun's ammunition rule.
pub(super) fn case38(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(38, "recrew, abandon and capture");
    let (mut swap, mut neu, mut own_c, mut gun_c) = (0.0, 0.0, 0i64, 0i64);
    let (mut served, mut fired, mut n) = (0u32, 0u32, 0u32);
    let mut gun_refilled = 0u32;
    for (seed, flip) in env.runs() {
        r.runs += 1;
        let (a, b, s, oc, gc, f) = capture_run(env, seed * 31 + flip as u64, flip)?;
        swap += a;
        neu += b;
        served += s as u32;
        own_c += oc;
        gun_c += gc;
        gun_refilled += (gc > 0) as u32;
        fired += f as u32;
        n += 1;
    }
    let n1 = f64::from(n.max(1));
    let (swap, neu) = (swap / n1, neu / n1);
    let served_pct = 100.0 * f64::from(served) / n1;
    let fired_pct = 100.0 * f64::from(fired) / n1;
    let refill_pct = 100.0 * f64::from(gun_refilled) / n1;
    r.check_v(
        "slot_swap_s",
        (2.5..=3.5).contains(&swap),
        round3(swap),
        "2.5..3.5",
        format!("mean over {n} runs"),
    );
    r.check_v(
        "neutral_after_s",
        (9.0..=12.0).contains(&neu),
        round3(neu),
        "9..12",
        "every man dead to neutral".to_string(),
    );
    r.check_v(
        "enemy_serves_pct",
        served_pct >= 100.0,
        round3(served_pct),
        ">= 100",
        "enemy rifle squad's Recrew".to_string(),
    );
    r.check_v(
        "own_crate_rounds",
        own_c == 0,
        own_c as f64,
        "0",
        "the captor's own crate never fits".to_string(),
    );
    r.check_v(
        "gun_faction_crate_rounds",
        gun_c >= 1,
        gun_c as f64,
        ">= 1",
        format!("{refill_pct:.0} % of runs drew from a crate of the gun's own faction"),
    );
    r.check_v(
        "left_rounds_fired_pct",
        fired_pct >= 100.0,
        round3(fired_pct),
        ">= 100",
        "the captured gun fires the rounds left at it".to_string(),
    );
    Ok(r)
}

/// One mobility run: `(emplace s, pack s, re-lay s, push m/s, tow m/s,
/// crew on the tow)`.
fn mobility_run(env: &Env, seed: u64, flip: u8) -> Result<(f64, f64, f64, f64, f64, bool)> {
    let (own, card, _) = gun_side(flip);
    let tow_card = if own == 0 {
        "ww2:ger_sdkfz11_squad"
    } else {
        "ww2:usa_m3a1_squad"
    };
    let limber = |w: &mut World, g: SimId| {
        w.edit_vehicle(g, |vs| {
            if let Some(sv) = vs.served.as_mut() {
                sv.state = GunState::Limbered;
            }
        });
    };
    let c = at(400, 500);
    // Times: emplace, re-lay, pack.
    let mut w = open_top_world(env, seed)?;
    let (_, g) = spawn_card(&mut w, own, card, c, dir_deg(0))?;
    for _ in 0..hz(&w) {
        w.step(&[]);
    }
    limber(&mut w, g);
    order(
        &mut w,
        own,
        vec![CommandKind::Emplace {
            guns: vec![g],
            facing: at(800, 500),
        }],
    );
    let emplace = time_until(&mut w, 60, |w: &World| {
        gun_state(w, g) == Some(GunState::Emplaced)
    })
    .ok_or_else(|| anyhow!("never emplaced"))?;
    order(
        &mut w,
        own,
        vec![CommandKind::FaceUnits {
            units: vec![g],
            toward: at(400, 900),
        }],
    );
    w.step(&[]);
    let relay = time_until(&mut w, 40, |w: &World| {
        vstate(w, g)
            .and_then(|v| v.served)
            .is_some_and(|s| s.relay_ticks == 0)
    })
    .ok_or_else(|| anyhow!("never re-laid"))?;
    order(&mut w, own, vec![CommandKind::PackGun { guns: vec![g] }]);
    let pack = time_until(&mut w, 40, |w: &World| {
        gun_state(w, g) == Some(GunState::Limbered)
    })
    .ok_or_else(|| anyhow!("never packed"))?;
    // Push.
    let mut w = open_top_world(env, seed)?;
    let (_, g) = spawn_card(&mut w, own, card, c, dir_deg(0))?;
    for _ in 0..hz(&w) {
        w.step(&[]);
    }
    limber(&mut w, g);
    order(
        &mut w,
        own,
        vec![CommandKind::PushGun {
            guns: vec![g],
            point: at(460, 500),
        }],
    );
    for _ in 0..3 * hz(&w) {
        w.step(&[]);
    }
    let p0 = pos(&w, g);
    for _ in 0..10 * hz(&w) {
        w.step(&[]);
    }
    let push = f64::from((pos(&w, g) - p0).length().to_f32()) / 10.0;
    // Hitch, ride, tow.
    let mut w = open_top_world(env, seed)?;
    let (_, g) = spawn_card(&mut w, own, card, c, dir_deg(0))?;
    let (tq, tow) = spawn_card(&mut w, own, tow_card, at(392, 500), dir_deg(180))?;
    for _ in 0..hz(&w) {
        w.step(&[]);
    }
    limber(&mut w, g);
    order(&mut w, own, vec![CommandKind::Hitch { guns: vec![g], tow }]);
    time_until(&mut w, 40, |w: &World| {
        gun_state(w, g) == Some(GunState::Hitched)
    })
    .ok_or_else(|| anyhow!("never hitched"))?;
    for _ in 0..25 * hz(&w) {
        w.step(&[]);
    }
    let crew = gun_def(&w, g)?.service.len();
    let rode = w.game().is_some_and(|g| g.riders_of(tow).len() == crew);
    order(
        &mut w,
        own,
        vec![CommandKind::Move {
            squad: tq,
            target: at(50, 500),
        }],
    );
    for _ in 0..8 * hz(&w) {
        w.step(&[]);
    }
    let t0 = pos(&w, tow);
    for _ in 0..10 * hz(&w) {
        w.step(&[]);
    }
    let tow_speed = f64::from((pos(&w, tow) - t0).length().to_f32()) / 10.0;
    let hit = vstate(&w, g)
        .and_then(|v| v.served)
        .is_some_and(|s| s.state == GunState::Hitched);
    Ok((
        emplace,
        pack,
        relay,
        push,
        if hit { tow_speed } else { 0.0 },
        rode,
    ))
}

/// Case 39 (T78): the mobility times, push speed and the tow.
pub(super) fn case39(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(39, "mobility and the tow");
    let mut sum = [0.0f64; 5];
    let (mut rode, mut n) = (0u32, 0u32);
    for (seed, flip) in env.runs() {
        r.runs += 1;
        let (e, p, l, s, t, c) = mobility_run(env, seed * 37 + flip as u64, flip)?;
        for (x, v) in sum.iter_mut().zip([e, p, l, s, t]) {
            *x += v;
        }
        rode += c as u32;
        n += 1;
    }
    let n1 = f64::from(n.max(1));
    let mean = |k: usize| sum[k] / n1;
    let (e, p, l, s, t) = (mean(0), mean(1), mean(2), mean(3), mean(4));
    r.check_v(
        "emplace_s",
        (27.0..=33.0).contains(&e),
        round3(e),
        "27..33 (Pak 40 30 s; 57 mm 25 s)",
        format!("mean of {n} runs, both guns"),
    );
    r.check_v(
        "pack_s",
        (15.0..=21.0).contains(&p),
        round3(p),
        "15..21",
        format!("mean of {n} runs"),
    );
    r.check_v(
        "pushed_cm_s_full_crew",
        (30.0..=40.0).contains(&(s * 100.0)),
        round3(s * 100.0),
        "30..40",
        format!("over 10 s, flat firm ground, mean of {n} runs"),
    );
    r.check_v(
        "hitched_tow_cm_s",
        t * 100.0 >= 400.0,
        round3(t * 100.0),
        ">= 400",
        "hitched, flat ground, the tow at its capped speed".to_string(),
    );
    r.check_v(
        "relay_trail_s",
        (12.0..=18.0).contains(&l),
        round3(l),
        "12..18",
        format!("FaceUnits re-lay, mean of {n} runs"),
    );
    let rode_pct = 100.0 * f64::from(rode) / n1;
    r.check_v(
        "crew_ride_tow_pct",
        rode_pct >= 100.0,
        round3(rode_pct),
        ">= 100",
        "the whole crew aboard after the Hitch".to_string(),
    );
    Ok(r)
}

/// One pit series: AP rounds aimed at the carriage from 200 m in front,
/// one gun per round (a penetration can wreck it); `(rounds that struck
/// the gun at all, rounds that struck its shield)`. From the front the
/// shield covers the axle, so what the pit changes is whether the round
/// reaches the gun at all: earth instead of carriage (3.10).
fn carriage_hits(env: &Env, seed: u64, flip: u8, in_pit: bool) -> Result<(u64, u64)> {
    const ROUNDS: u64 = 12;
    let (own, card, _) = gun_side(flip);
    let enemy = 1 - own;
    let c = at(150, 500);
    let (mut carriage, mut other) = (0, 0);
    for k in 0..ROUNDS {
        let mut w = open_top_world(env, seed + k)?;
        w.enable_hit_log();
        let (_, g) = spawn_card(&mut w, own, card, c, dir_deg(0))?;
        if in_pit {
            w.dig_pit_now(
                &[at(146, 500), at(150, 500)],
                frontline_content::DigProfile::GunPit,
            );
        }
        for _ in 0..hz(&w) {
            w.step(&[]);
        }
        let weapon = weapon_idx(&w, "ww2:kwk40_l48")?;
        let ammo = w
            .match_setup()
            .and_then(|s| s.ammo.iter().position(|a| a.id == "ww2:75mm_pzgr39"))
            .ok_or_else(|| anyhow!("no round"))? as u16;
        let ground = w.entities().transform()[g.index()].pos.z;
        // Aimed at the axle, 45 cm over the gun's ground, plus 31 cm for
        // the drop over 200 m (the tool has no compensation).
        let dy = Fx::from_ratio((k % 7) as i64 - 3, 5);
        let aim = c.extend(ground + Fx::from_ratio(76, 100)) + FxVec3::new(Fx::ZERO, dy, Fx::ZERO);
        let origin = at(350, 500);
        let from = origin.extend(w.terrain().height_at(origin) + Fx::from_ratio(15, 10));
        if !w.fire_shell_now(enemy, weapon, ammo, from, aim) {
            continue;
        }
        for _ in 0..hz(&w) {
            w.step(&[]);
        }
        let log = w.take_hit_log();
        let first = log.iter().find(|h| h.vehicle == g).map(|h| h.part.0);
        carriage += u64::from(first.is_some());
        other += u64::from(first == Some(1));
    }
    Ok((carriage, other))
}

/// Case 40 (T79): the pit: shots at the carriage strike the parapet; the
/// crew's losses in a pit, reported against the open.
pub(super) fn case40(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(40, "the AT gun pit");
    let (mut open_c, mut pit_c, mut other) = (0u64, 0u64, 0u64);
    let (mut pit_mg, mut open_mg) = (0u64, 0u64);
    for (seed, flip) in env.runs() {
        r.runs += 1;
        let (c0, _) = carriage_hits(env, seed * 41 + flip as u64, flip, false)?;
        let (c1, o) = carriage_hits(env, seed * 41 + flip as u64, flip, true)?;
        open_c += c0;
        pit_c += c1;
        other += o;
        open_mg += mg_series(env, seed * 43 + flip as u64, flip, Fire::Front)?.0;
        pit_mg += mg_series_in_pit(env, seed * 43 + flip as u64, flip)?.0;
    }
    let pct = 100.0 * pit_c as f64 / open_c.max(1) as f64;
    r.check_v(
        "pit_carriage_hits_pct",
        pct <= 20.0,
        round3(pct),
        "<= 20",
        format!("{pit_c} of the rounds aimed at the carriage reached the gun in a pit, {open_c} in the open ({other} shield)"),
    );
    let mg_pct = 100.0 * pit_mg as f64 / open_mg.max(1) as f64;
    r.check_v(
        "pit_crew_losses_pct_of_open",
        true,
        round3(mg_pct),
        "report",
        format!("{pit_mg} MG hits on the crew in a pit, {open_mg} with the gun in the open"),
    );
    Ok(r)
}
