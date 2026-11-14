//! S8b armour-test cases (`docs/slices/S8b-trucks-recon.md` 12): wheels
//! against trenches (25), the bicycle model (26), a tank rearming from a
//! truck (27), cargo detonation (28), no stacking (29), the grenade screen
//! (30), and the recon guns, plates, optics and MG against a truck (31).

use anyhow::{Result, anyhow};
use frontline_content::setup::{StockKind, SupplyKind};
use frontline_content::{ComponentKind, PlateGroup};
use frontline_sim::armour::{self, deg, plate_shot};
use frontline_sim::ecs::Velocity;
use frontline_sim::scenarios::{at, dir_deg, members};
use frontline_sim::vehicle::{DriveCtx, drive};
use frontline_sim::{
    ArmourEventKind, Fx, FxVec2, FxVec3, ItemKind, PlayerId, SimId, Structure, World,
};

use super::{
    CaseReport, CommandKind, Env, crew_hp, dist, hz, no_ammo, open_top_world, order, pos,
    spawn_card, vstate, weapon_idx,
};
use crate::report::round3;

const GER: &str = "ww2:ger_sdkfz234_1_squad";
const USA: &str = "ww2:usa_m8_squad";
const GER_TRUCK: &str = "ww2:ger_opel_blitz_squad";
const USA_TRUCK: &str = "ww2:usa_cckw_squad";
const GER_TANK: &str = "ww2:ger_pz4_h_squad";
const USA_TANK: &str = "ww2:usa_m4a3_75_squad";

/// `(player, truck card, tank card, faction index)` of run `flip`.
fn side(flip: u8) -> (u8, &'static str, &'static str) {
    if flip == 0 {
        (0, GER_TRUCK, GER_TANK)
    } else {
        (1, USA_TRUCK, USA_TANK)
    }
}

/// A truck of `card` for `p` at `x, y` facing east holding `shells` kg.
fn truck_at(w: &mut World, p: u8, card: &str, x: i32, y: i32, shells: u32) -> Result<SimId> {
    let (_, v) = spawn_card(w, p, card, at(x, y), dir_deg(0))?;
    w.edit_vehicle(v, |vs| {
        vs.cargo_g[SupplyKind::Shells.index()] = shells * 1000
    });
    Ok(v)
}

/// A tank of `card` at `x, y` with 10 % of its standard rounds (its rare
/// racks full, so only the standard interval runs).
fn tank_at_tenth(w: &mut World, p: u8, card: &str, x: i32, y: i32) -> Result<SimId> {
    let (_, v) = spawn_card(w, p, card, at(x, y), dir_deg(0))?;
    let setup = w.match_setup().ok_or_else(|| anyhow!("setup"))?;
    let t = vstate(w, v).ok_or_else(|| anyhow!("no tank"))?.type_idx as usize;
    let counts: Vec<(u16, bool)> = setup.vehicles[t]
        .main_ammo()
        .iter()
        .map(|a| (a.count, a.rare))
        .collect();
    w.edit_vehicle(v, |vs| {
        for (k, &(c, rare)) in counts.iter().enumerate() {
            vs.ammo[k] = if rare { c } else { (c / 10).max(1) };
        }
        vs.loaded = None;
    });
    Ok(v)
}

fn held_rounds(w: &World, v: SimId) -> Result<u32> {
    let vs = vstate(w, v).ok_or_else(|| anyhow!("no vehicle"))?;
    Ok(vs.ammo.iter().map(|&n| n as u32).sum::<u32>() + u32::from(vs.loaded.is_some()))
}

fn manpower(w: &World, p: u8) -> Result<u32> {
    Ok(w.game().ok_or_else(|| anyhow!("no game"))?.players()[p as usize].manpower)
}

/// The account of `p` (S6 and S8b counters).
fn acc(w: &World, p: u8) -> Result<frontline_sim::S6Account> {
    Ok(w.game()
        .ok_or_else(|| anyhow!("no game"))?
        .supply6()
        .accounts[p as usize])
}

/// Grams a truck's shells fell by and rounds gained, per the current stock.
fn shell_g(w: &World, v: SimId) -> Result<u32> {
    Ok(vstate(w, v).ok_or_else(|| anyhow!("no truck"))?.cargo_g[SupplyKind::Shells.index()])
}

/// Rounds per ammo entry, the one in the breech included.
fn rack_view(w: &World, v: SimId) -> Result<Vec<u16>> {
    let vs = vstate(w, v).ok_or_else(|| anyhow!("no vehicle"))?;
    let mut out = vs.ammo.clone();
    if let Some(l) = vs.loaded {
        out[l as usize] += 1;
    }
    Ok(out)
}

/// The grams of the rounds `v` gained since `before`, by ammo entry.
fn grams_gained(w: &World, v: SimId, before: &[u16]) -> Result<u64> {
    let setup = w.match_setup().ok_or_else(|| anyhow!("setup"))?;
    let vs = vstate(w, v).ok_or_else(|| anyhow!("no vehicle"))?;
    let vt = &setup.vehicles[vs.type_idx as usize];
    let ammo = vt.main_ammo();
    let mut g = 0u64;
    for (k, &n) in vs.ammo.iter().enumerate() {
        let now = n as u32 + u32::from(vs.loaded == Some(k as u8));
        let was = before.get(k).copied().unwrap_or(0) as u32;
        g += u64::from(now.saturating_sub(was))
            * u64::from(setup.ammo[ammo[k].ammo as usize].supply.map_or(0, |s| s.g));
    }
    Ok(g)
}

/// `(player, card)` of run `flip`: the German car or the American.
fn car(flip: u8) -> (u8, &'static str) {
    if flip == 0 { (0, GER) } else { (1, USA) }
}

/// A closed square of trench of half-side `h` m round `c`, or (with `gap`)
/// open in the middle of its east side.
fn trench_ring(w: &mut World, c: FxVec2, h: i32, gap: Option<i32>) -> bool {
    let p = |dx: i32, dy: i32| c + FxVec2::from_int(dx, dy);
    let line = match gap {
        None => vec![p(-h, -h), p(h, -h), p(h, h), p(-h, h), p(-h, -h)],
        Some(g) => vec![
            p(h, g / 2),
            p(h, h),
            p(-h, h),
            p(-h, -h),
            p(h, -h),
            p(h, -g / 2),
        ],
    };
    let ok = w.dig_trench_now(&line, 1);
    w.step(&[]);
    ok
}

/// Orders vehicle squad `sq` to `to`, steps up to `secs`; `(arrived,
/// left the ring)`.
#[allow(clippy::too_many_arguments)]
fn drive_out(
    w: &mut World,
    p: u8,
    sq: frontline_sim::SquadId,
    v: SimId,
    c: FxVec2,
    h: i32,
    to: FxVec2,
    secs: u64,
) -> (bool, bool) {
    order(
        w,
        p,
        vec![CommandKind::Move {
            squad: sq,
            target: to,
        }],
    );
    let end = w.tick() + secs * hz(w);
    let bound = f64::from(h + 2);
    let mut out = false;
    while w.tick() < end {
        let q = pos(w, v);
        let (dx, dy) = (
            f64::from(q.x.to_f32()) - f64::from(c.x.to_f32()),
            f64::from(q.y.to_f32()) - f64::from(c.y.to_f32()),
        );
        out |= dx.abs() > bound || dy.abs() > bound;
        if dist(q, to) < 6.0 {
            return (true, out);
        }
        w.step(&[]);
    }
    (dist(pos(w, v), to) < 6.0, out)
}

/// Case 25 (6.8): a car ordered out of a closed trench ring must stop or
/// fail, never cross; through a 10 m gap it arrives; a tank crosses the
/// same closed ring.
pub(super) fn case25(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(25, "wheels against a trench line (6.8)");
    let (mut crossed, mut gap_n, mut gap_ok, mut tank_n, mut tank_ok) =
        (0u32, 0u32, 0u32, 0u32, 0u32);
    let (c, h) = (at(500, 500), 30);
    let to = c + FxVec2::from_int(60, 0);
    for (seed, flip) in env.runs() {
        r.runs += 1;
        let (p, card) = car(flip);
        // Closed ring: never across.
        let mut w = open_top_world(env, seed * 8 + flip as u64)?;
        if !trench_ring(&mut w, c, h, None) {
            return Err(anyhow!("trench ring"));
        }
        let (sq, v) = spawn_card(&mut w, p, card, c, dir_deg(0))?;
        no_ammo(&mut w, v);
        let (_, out) = drive_out(&mut w, p, sq, v, c, h, to, 60);
        crossed += u32::from(out);
        // A 10 m gap in the east side: it drives through.
        let mut w = open_top_world(env, seed * 8 + 1 + flip as u64)?;
        trench_ring(&mut w, c, h, Some(10));
        let (sq, v) = spawn_card(&mut w, p, card, c, dir_deg(0))?;
        no_ammo(&mut w, v);
        let (arrived, _) = drive_out(&mut w, p, sq, v, c, h, to, 90);
        gap_n += 1;
        gap_ok += u32::from(arrived);
        if env.verbose {
            eprintln!(
                "case 25 seed {seed} flip {flip}: gap arrived {arrived}, at {:?}",
                pos(&w, v)
            );
        }
        // A tank at the same closed ring crosses it.
        let mut w = open_top_world(env, seed * 8 + 2 + flip as u64)?;
        trench_ring(&mut w, c, h, None);
        let tank = if flip == 0 {
            "ww2:ger_pz4_h_squad"
        } else {
            "ww2:usa_m4a3_75_squad"
        };
        let (sq, v) = spawn_card(&mut w, p, tank, c, dir_deg(0))?;
        no_ammo(&mut w, v);
        let (arrived, _) = drive_out(&mut w, p, sq, v, c, h, to, 120);
        tank_n += 1;
        tank_ok += u32::from(arrived);
    }
    let pct = |a: u32, b: u32| round3(f64::from(a) * 100.0 / f64::from(b.max(1)));
    r.check_v(
        "crossings_closed_line",
        crossed == 0,
        f64::from(crossed),
        "0",
        format!("{crossed} of {} runs left the closed ring", r.runs),
    );
    r.check_v(
        "reached_through_gap_pct",
        gap_ok * 5 >= gap_n * 4,
        pct(gap_ok, gap_n),
        ">= 80",
        format!("{gap_ok} of {gap_n}"),
    );
    r.check_v(
        "tank_crosses_same_line_pct",
        tank_ok * 5 >= tank_n * 4,
        pct(tank_ok, tank_n),
        ">= 80",
        format!("{tank_ok} of {tank_n}"),
    );
    Ok(r)
}

/// Runs `drive` directly with fixed inputs on a copy of vehicle `v`.
/// Returns the positions per tick and the final speed.
fn free_drive(w: &World, v: SimId, throttle: i8, steer: i8, secs: u32) -> (Vec<FxVec2>, Fx, Fx) {
    let g = w.game().expect("match");
    let setup = w.match_setup().expect("match");
    let mut vs = vstate(w, v).expect("vehicle");
    let vt = &setup.vehicles[vs.type_idx as usize];
    let d = g.vehicle_derived();
    let ctx = DriveCtx {
        terrain: w.terrain(),
        block: d.block_of(vt),
        v: vt,
        geo: &d.geo[vs.type_idx as usize],
        rules: &setup.rules.vehicles,
        hz: 30,
        seed: 1,
        escape: None,
        piece_near: None,
    };
    let mut t = w.entities().transform()[v.index()];
    let mut vel = Velocity::default();
    let yaw0 = t.yaw;
    let mut pts = Vec::new();
    for _ in 0..secs * 30 {
        drive(&ctx, v, &mut vs, &mut t, &mut vel, throttle, steer);
        pts.push(t.pos.xy());
    }
    (pts, vs.speed, (t.yaw - yaw0).wrap_angle())
}

/// Case 26 (6.8): no neutral pivot, the turning radius, the 234's fast
/// reverse, a blast under a wheel.
pub(super) fn case26(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(26, "wheeled drive: bicycle, reverse, blast under a wheel");
    let (mut pivot_free, mut pivots, mut worst, mut rev_234, mut rev_m8) =
        (true, 0u32, 0f64, 0f64, 0f64);
    let (mut blasts, mut disabled, mut throws) = (0u32, 0u32, 0u32);
    for (seed, flip) in env.runs() {
        r.runs += 1;
        let (p, card) = car(flip);
        let mut w = open_top_world(env, seed * 12 + flip as u64)?;
        let (_, v) = spawn_card(&mut w, p, card, at(500, 500), dir_deg(0))?;
        no_ammo(&mut w, v);
        w.step(&[]);
        // No pivot: full lock at a standstill leaves the heading alone, and
        // a Face order is dropped.
        let (_, _, turned) = free_drive(&w, v, 0, 127, 5);
        pivot_free &= turned == Fx::ZERO;
        pivots += 1;
        let yaw0 = w.entities().transform()[v.index()].yaw;
        order(
            &mut w,
            p,
            vec![CommandKind::FaceUnits {
                units: vec![v],
                toward: at(500, 600),
            }],
        );
        for _ in 0..5 * hz(&w) {
            w.step(&[]);
        }
        pivot_free &= w.entities().transform()[v.index()].yaw == yaw0;
        // Full lock at half throttle: a circle of the def's radius.
        let (pts, _, _) = free_drive(&w, v, 64, 127, 40);
        let late = &pts[(pts.len() / 4)..];
        let mut chord = 0f64;
        for (i, a) in late.iter().enumerate().step_by(10) {
            for b in late[i..].iter().step_by(10) {
                chord = chord.max(dist(*a, *b));
            }
        }
        let want = if flip == 0 { 7.0 } else { 8.5 };
        let err = (chord / 2.0 - want).abs() * 100.0 / want;
        worst = worst.max(err);
        // Reverse at full throttle (10 s, flat ground).
        let (_, speed, _) = free_drive(&w, v, -127, 0, 10);
        let cm_s = -f64::from(speed.to_f32()) * 100.0;
        if flip == 0 {
            rev_234 = rev_234.max(cm_s);
        } else {
            rev_m8 = rev_m8.max(cm_s);
        }
        // A satchel beside the front axles: the squad stands where its
        // satchel man has the charge in reach (3-8 m).
        let eng = if p == 0 {
            "ww2:ger_engineer_squad"
        } else {
            "ww2:usa_engineer_squad"
        };
        let spot = FxVec2::new(Fx::from_int(502), Fx::from_ratio(4985, 10));
        // Where the squad's satchel man stands relative to its anchor.
        let satchel_man = |w: &World, sq: frontline_sim::SquadId| {
            members(w, sq)
                .into_iter()
                .find(|m| w.entities().kit()[m.index()].items[ItemKind::Satchel.index()] > 0)
        };
        let mut w = open_top_world(env, seed * 12 + 1 + flip as u64)?;
        let (esq, _) = spawn_card(&mut w, p, eng, at(502, 470), dir_deg(90))?;
        let Some(m) = satchel_man(&w, esq) else {
            continue;
        };
        let offset = pos(&w, m) - at(502, 470);
        let anchor = FxVec2::new(Fx::from_int(502), Fx::from_int(493)) - offset;
        let mut w = open_top_world(env, seed * 12 + 2 + flip as u64)?;
        let (_, v) = spawn_card(&mut w, 1 - p, card, at(500, 500), dir_deg(0))?;
        no_ammo(&mut w, v);
        let (esq, _) = spawn_card(&mut w, p, eng, anchor, dir_deg(90))?;
        let Some(m) = satchel_man(&w, esq) else {
            continue;
        };
        for x in members(&w, esq) {
            w.edit_soldier(x, |k, c| {
                k.rounds.iter_mut().for_each(|n| *n = 0);
                c.mag = 0;
            });
        }
        // Beside the near hull edge at the front axles (hull half-width
        // 1.15-1.25 m; satchel reach covers the anchors).
        order(
            &mut w,
            p,
            vec![CommandKind::Throw {
                units: vec![m],
                item: ItemKind::Satchel,
                target: spot,
            }],
        );
        throws += 1;
        if env.verbose {
            eprintln!("case 26 thrower at {} m", dist(pos(&w, m), spot));
        }
        let mut hit = false;
        for _ in 0..12 * hz(&w) {
            w.step(&[]);
            hit |= w.snapshot().armour_events.iter().any(|e| {
                e.kind == ArmourEventKind::Disabled as u8
                    && (e.component == ComponentKind::WheelsFront as u8
                        || e.component == ComponentKind::WheelsRear as u8)
            });
        }
        blasts += 1;
        disabled += u32::from(hit);
        if env.verbose {
            eprintln!(
                "case 26 blast: throws {}, thrown {}, comps {:?}",
                w.stats().throws,
                w.game().expect("m").thrown().len(),
                vstate(&w, v).map(|x| x.components)
            );
        }
    }
    r.row(
        "reverse",
        format!("234/1 {rev_234:.0} cm/s, M8 {rev_m8:.0} cm/s"),
    );
    r.check_v(
        "no_neutral_pivot",
        pivot_free,
        f64::from(u8::from(pivot_free)),
        "1",
        format!("{pivots} runs"),
    );
    r.check_v(
        "turn_radius_within_pct",
        worst <= 15.0,
        round3(worst),
        "<= 15",
        "worst of both cars".to_string(),
    );
    r.check_v(
        "reverse_speed_234_cm_s",
        rev_234 >= 900.0,
        round3(rev_234),
        ">= 900",
        "rear driving position".to_string(),
    );
    let pct = f64::from(disabled) * 100.0 / f64::from(blasts.max(1));
    r.check_v(
        "wheel_blast_disables_pct",
        blasts > 0 && disabled * 2 >= blasts,
        round3(pct),
        ">= 50",
        format!("{disabled} of {blasts} satchels ({throws} thrown)"),
    );
    Ok(r)
}

/// Whether point `p` lies outside the footprint of `v`'s hull and within
/// `m` metres of it.
fn beside(w: &World, v: SimId, p: FxVec3, m: Fx) -> bool {
    let g = w.game().expect("match");
    let Some(f) = g.vehicle_derived().frame(v) else {
        return false;
    };
    let b = &f.boxes[0];
    let d = p.xy() - b.center.xy();
    let (lx, ly) = (b.axes.x.xy().dot(d).abs(), b.axes.y.xy().dot(d).abs());
    let inside = lx <= b.half.x && ly <= b.half.y;
    !inside && lx <= b.half.x + m && ly <= b.half.y + m
}

fn dead(hp: &[u16]) -> usize {
    hp.iter().filter(|h| **h == 0).count()
}

/// Case 27 (7.3, 10): a tank at 10 % ammo 15 m from a truck: one standard
/// round per 4 s; the truck's stock falls by the grams of the rounds; no
/// manpower is charged.
pub(super) fn case27(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(27, "tank rearms from a truck at 15 m (7.3)");
    let (mut rounds, mut err, mut secs) = (0u64, 0u64, 0u64);
    let mut free = true;
    for (seed, flip) in env.runs() {
        r.runs += 1;
        let (p, tc, tank) = side(flip);
        let mut w = open_top_world(env, seed * 32 + flip as u64)?;
        let t = truck_at(&mut w, p, tc, 515, 500, 600)?;
        let v = tank_at_tenth(&mut w, p, tank, 500, 500)?;
        w.step(&[]);
        let before = rack_view(&w, v)?;
        let stock0 = shell_g(&w, t)?;
        let mp0 = manpower(&w, p)?;
        let n0 = held_rounds(&w, v)?;
        let t_s = 60u64;
        for _ in 0..t_s * hz(&w) {
            w.step(&[]);
        }
        let n1 = held_rounds(&w, v)?;
        let gained = n1.saturating_sub(n0) as u64;
        rounds += gained;
        secs += t_s;
        err += grams_gained(&w, v, &before)?.abs_diff(u64::from(stock0 - shell_g(&w, t)?));
        free &= manpower(&w, p)? >= mp0;
    }
    let per = secs as f64 / rounds.max(1) as f64;
    r.check_v(
        "seconds_per_round",
        (3.9..=4.1).contains(&per),
        round3(per),
        "3.9..=4.1",
        format!("{rounds} rounds in {secs} s"),
    );
    r.check_v(
        "stock_error_g",
        err == 0,
        err as f64,
        "0",
        "grams drawn minus grams of the rounds taken".to_string(),
    );
    r.check_v(
        "no_mp_charged",
        free,
        f64::from(u8::from(free)),
        ">= 1",
        "manpower never fell".to_string(),
    );
    Ok(r)
}

/// Case 28 (7.3, 10): a bazooka rocket at the cargo box of a loaded and of
/// an empty truck; a detonation or the fire after it counts.
pub(super) fn case28(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(28, "cargo detonation (7.3)");
    let trials = 3u64;
    let (mut loaded, mut loaded_n) = (0u32, 0u32);
    let (mut empty, mut empty_n) = (0u32, 0u32);
    for (seed, flip) in env.runs() {
        r.runs += 1;
        let (p, tc, _) = side(flip);
        for full in [true, false] {
            for k in 0..trials {
                let mut w = open_top_world(env, seed * 64 + k * 2 + flip as u64)?;
                let gun = weapon_idx(&w, "ww2:m9_bazooka")?;
                let v = truck_at(&mut w, p, tc, 500, 500, if full { 320 } else { 0 })?;
                w.step(&[]);
                let b = w.entities().transform()[v.index()].pos;
                // The cargo box's side: 1.3 m behind the hull centre, 1.5 m
                // up, 1 m to the north of the hull's axis.
                let aim = FxVec3::new(
                    b.x - Fx::from_ratio(13, 10),
                    b.y + Fx::ONE,
                    b.z + Fx::from_ratio(3, 2),
                );
                let from = FxVec3::new(
                    aim.x,
                    aim.y + Fx::from_int(60),
                    aim.z + Fx::from_ratio(1, 4),
                );
                // The rocket drops under gravity over 60 m at its speed.
                let speed = w
                    .match_setup()
                    .and_then(|s| {
                        let wd = &s.weapons[gun as usize];
                        wd.ammo
                            .and_then(|a| s.ammo.get(a as usize))
                            .map(|a| a.speed)
                    })
                    .map_or(80.0, |sp| f64::from(Fx::from(sp).to_f32()));
                let t = 60.0 / speed.max(1.0);
                let drop = Fx::from_milli((0.5 * 9.81 * t * t * 1000.0) as i32);
                let up = FxVec3::new(aim.x, aim.y, aim.z + drop);
                if !w.fire_rocket_now(1 - p, gun, from, up) {
                    return Err(anyhow!("cannot fire the rocket"));
                }
                let fire_s = w
                    .match_setup()
                    .map_or(30, |s| s.rules.vehicles.damage.ammo_fire_s as u64);
                let n = (6 + fire_s) * hz(&w);
                for _ in 0..n {
                    w.step(&[]);
                }
                let a = acc(&w, p)?;
                if full {
                    loaded_n += 1;
                    loaded += u32::from(a.cargo_detonations > 0);
                } else {
                    empty_n += 1;
                    empty += u32::from(a.cargo_detonations > 0);
                }
            }
        }
    }
    let lp = f64::from(loaded) * 100.0 / f64::from(loaded_n.max(1));
    let ep = f64::from(empty) * 100.0 / f64::from(empty_n.max(1));
    r.check_v(
        "detonated_loaded_pct",
        lp >= 50.0,
        round3(lp),
        ">= 50",
        format!("{loaded} of {loaded_n}"),
    );
    r.check_v(
        "detonated_empty_pct",
        empty == 0,
        round3(ep),
        "0",
        format!("{empty} of {empty_n}"),
    );
    Ok(r)
}

/// Case 29 (7.3, 10): a tank between an HQ and two trucks rearms at one
/// source's rate.
pub(super) fn case29(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(29, "tank between an HQ and two trucks: no stacking");
    let (mut rounds, mut secs, mut srcs) = (0u64, 0u64, 0u32);
    for (seed, flip) in env.runs() {
        r.runs += 1;
        let (p, tc, tank) = side(flip);
        let mut w = open_top_world(env, seed * 96 + flip as u64)?;
        let f = w.match_setup().ok_or_else(|| anyhow!("setup"))?.players[p as usize].faction;
        let hq = w
            .match_setup()
            .and_then(|s| s.stock_of(f, StockKind::Hq, None))
            .ok_or_else(|| anyhow!("no HQ def"))?;
        let boxes = w
            .match_setup()
            .map(|s| {
                let d = &s.stocks[hq as usize];
                [0, 1, 2, 3].map(|k| d.boxes.get(k).map_or(0, |b| b.hp))
            })
            .ok_or_else(|| anyhow!("setup"))?;
        let mut hid = 0;
        w.edit_supply6(|st| {
            hid = st.next_structure;
            st.next_structure += 1;
            st.structures.insert(
                hid,
                Structure {
                    owner: frontline_sim::PlayerId(p),
                    def: hq,
                    pos: at(505, 500),
                    yaw: Fx::ZERO,
                    builder: None,
                    built: true,
                    box_hp: boxes,
                    radio: true,
                    stock_g: [0, 0, 600_000],
                },
            );
        });
        let a = truck_at(&mut w, p, tc, 490, 500, 600)?;
        let b = truck_at(&mut w, p, tc, 510, 512, 600)?;
        let v = tank_at_tenth(&mut w, p, tank, 500, 500)?;
        w.step(&[]);
        let n0 = held_rounds(&w, v)?;
        let (s0, ta0, tb0) = (600_000u32, shell_g(&w, a)?, shell_g(&w, b)?);
        let t_s = 60u64;
        for _ in 0..t_s * hz(&w) {
            w.step(&[]);
        }
        rounds += u64::from(held_rounds(&w, v)?.saturating_sub(n0));
        secs += t_s;
        let stock = w
            .game()
            .and_then(|g| g.supply6().structures.get(&hid))
            .map_or(0, |s| s.stock_g[SupplyKind::Shells.index()]);
        let mut drawn = 0;
        drawn += u32::from(stock < s0);
        drawn += u32::from(shell_g(&w, a)? < ta0);
        drawn += u32::from(shell_g(&w, b)? < tb0);
        srcs = srcs.max(drawn);
    }
    let per = secs as f64 / rounds.max(1) as f64;
    r.check_v(
        "seconds_per_round",
        (3.9..=4.1).contains(&per),
        round3(per),
        "3.9..=4.1",
        format!("{rounds} rounds in {secs} s"),
    );
    r.check_v(
        "sources_drawn",
        srcs == 1,
        f64::from(srcs),
        "1",
        "structures, trucks, crates".to_string(),
    );
    Ok(r)
}

/// Case 30 (6.5): grenades thrown at the screened 234/1 turret and at the
/// M8; live grenades dropped on the screen; rounds through the screen.
pub(super) fn case30(env: &Env) -> Result<CaseReport> {
    const THROWS: u64 = 3;
    let mut r = CaseReport::new(30, "grenade screen on an open turret (6.5)");
    let [mut scr_n, mut scr_in, mut m8_n, mut m8_in] = [0u32; 4];
    let (mut drops, mut beside_n, mut lost) = (0u32, 0u32, 0usize);
    let (mut hits, mut shots) = (0u32, 0u32);
    for (seed, flip) in env.runs() {
        r.runs += 1;
        for k in 0..THROWS {
            for (card, screened) in [(GER, true), (USA, false)] {
                let mut w = open_top_world(env, seed * 32 + k * 2 + flip as u64)?;
                let az = (seed * 41 + k * 67 + flip as u64 * 19) % 360;
                let f = dir_deg(az as i32);
                let c = at(500, 500);
                let (_, v) = spawn_card(&mut w, 1, card, c, f)?;
                no_ammo(&mut w, v);
                let side = FxVec2::new(-f.y, f.x);
                let (gsq, _) = spawn_card(
                    &mut w,
                    0,
                    "ww2:ger_grenadiergruppe",
                    c + side * Fx::from_int(15),
                    -side,
                )?;
                for m in members(&w, gsq) {
                    w.edit_soldier(m, |k, c| {
                        k.items[ItemKind::SingleShotAt.index()] = 0;
                        k.rounds.iter_mut().for_each(|n| *n = 0);
                        c.mag = 0;
                    });
                }
                let Some(man) = members(&w, gsq)
                    .into_iter()
                    .find(|m| w.entities().kit()[m.index()].items[ItemKind::Frag.index()] > 0)
                else {
                    continue;
                };
                for _ in 0..hz(&w) {
                    w.step(&[]);
                }
                let target = w
                    .game()
                    .and_then(|g| g.vehicle_derived().frame(v))
                    .map(|f| f.boxes[1].center.xy())
                    .unwrap_or(c);
                let before = crew_hp(&w, v);
                order(
                    &mut w,
                    0,
                    vec![CommandKind::Throw {
                        units: vec![man],
                        item: ItemKind::Frag,
                        target,
                    }],
                );
                let mut inside = false;
                for _ in 0..8 * hz(&w) {
                    w.step(&[]);
                    inside |= w
                        .game()
                        .expect("match")
                        .thrown()
                        .iter()
                        .any(|t| t.inside.is_some_and(|i| i.vehicle == v));
                }
                if screened {
                    scr_n += 1;
                    scr_in += u32::from(inside);
                    let after = crew_hp(&w, v);
                    lost += dead(&after).saturating_sub(dead(&before));
                } else {
                    m8_n += 1;
                    m8_in += u32::from(inside);
                }
                if env.verbose {
                    eprintln!(
                        "case 30 seed {seed} k {k} {card}: inside {inside}, {before:?} -> {:?}",
                        crew_hp(&w, v)
                    );
                }
            }
        }
        // A live frag dropped from 6 m onto the screen over the turret.
        let mut w = open_top_world(env, seed * 32 + 9 + flip as u64)?;
        let (_, v) = spawn_card(&mut w, 1, GER, at(500, 500), dir_deg(90))?;
        no_ammo(&mut w, v);
        let (gsq, _) = spawn_card(
            &mut w,
            0,
            "ww2:ger_grenadiergruppe",
            at(480, 500),
            dir_deg(0),
        )?;
        let man = members(&w, gsq)[0];
        for m in members(&w, gsq) {
            w.edit_soldier(m, |k, c| {
                k.items[ItemKind::SingleShotAt.index()] = 0;
                k.rounds.iter_mut().for_each(|n| *n = 0);
                c.mag = 0;
            });
        }
        w.step(&[]);
        let item = w
            .match_setup()
            .and_then(|s| s.items.iter().position(|x| x.kind == ItemKind::Frag))
            .ok_or_else(|| anyhow!("no frag"))? as u16;
        let spot = w
            .game()
            .and_then(|g| g.vehicle_derived().frame(v))
            .map(|f| f.boxes[1].center.xy())
            .ok_or_else(|| anyhow!("frame"))?;
        let before = crew_hp(&w, v);
        if !w.drop_item_now(man, item, spot, Fx::from_int(6)) {
            return Err(anyhow!("drop_item_now"));
        }
        let mut rest: Option<FxVec3> = None;
        let mut inside = false;
        for _ in 0..4 * hz(&w) {
            w.step(&[]);
            let g = w.game().expect("match");
            inside |= g.thrown().iter().any(|t| t.inside.is_some());
            if rest.is_none() {
                rest = g.thrown().iter().find(|t| t.landed).map(|t| t.pos);
            }
        }
        drops += 1;
        scr_in += u32::from(inside);
        scr_n += 1;
        let next_to = rest.is_some_and(|q| beside(&w, v, q, Fx::from_int(3)));
        beside_n += u32::from(next_to);
        if env.verbose {
            eprintln!(
                "case 30 drop: crew {before:?} -> {:?}, rest {rest:?}",
                crew_hp(&w, v)
            );
        }
        lost += dead(&crew_hp(&w, v)).saturating_sub(dead(&before));
        // Rounds from above go through the screen into the gunner.
        let mut w = open_top_world(env, seed * 32 + 11 + flip as u64)?;
        let (_, v) = spawn_card(&mut w, 1, GER, at(500, 500), dir_deg(90))?;
        no_ammo(&mut w, v);
        let gun = weapon_idx(&w, "ww2:mg42_bipod")?;
        w.step(&[]);
        let k = w
            .match_setup()
            .and_then(|s| {
                let vt = vstate(&w, v).map(|x| &s.vehicles[x.type_idx as usize])?;
                vt.seats
                    .iter()
                    .position(|x| x.role == frontline_content::SeatRole::Gunner)
            })
            .ok_or_else(|| anyhow!("gunner"))?;
        for _ in 0..6 {
            let (part, f) = {
                let s = w.match_setup().ok_or_else(|| anyhow!("match"))?;
                let vt = &s.vehicles[vstate(&w, v).map_or(0, |x| x.type_idx) as usize];
                let f = *w
                    .game()
                    .and_then(|g| g.vehicle_derived().frame(v))
                    .ok_or_else(|| anyhow!("frame"))?;
                (vt.seats[k].part, f)
            };
            let sb = frontline_sim::damage::part_box(&f, &part);
            let up = FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::from_int(25));
            let before = crew_hp(&w, v).get(k).copied().unwrap_or(0);
            if !w.fire_round_now(0, gun, sb.center + up, sb.center) {
                continue;
            }
            shots += 1;
            for _ in 0..hz(&w) / 2 {
                w.step(&[]);
            }
            hits += u32::from(crew_hp(&w, v).get(k).copied().unwrap_or(0) < before);
            w.edit_vehicle(v, |vs| vs.seats[k].hp = 100);
        }
    }
    let pct = |a: u32, b: u32| round3(f64::from(a) * 100.0 / f64::from(b.max(1)));
    r.check_v(
        "screened_inside_pct",
        scr_in == 0,
        pct(scr_in, scr_n),
        "0",
        format!("{scr_in} of {scr_n}"),
    );
    r.check_v(
        "screened_crew_lost",
        lost == 0,
        lost as f64,
        "0",
        format!("crew killed in {scr_n} grenades"),
    );
    r.check_v(
        "landed_beside_pct",
        beside_n * 5 >= drops * 4,
        pct(beside_n, drops),
        ">= 80",
        format!("{beside_n} of {drops} dropped on the screen"),
    );
    r.check_v(
        "unscreened_m8_inside_pct",
        m8_in * 10 >= m8_n * 3,
        pct(m8_in, m8_n),
        ">= 30",
        format!("{m8_in} of {m8_n}"),
    );
    r.check_v(
        "bullets_pass_screen",
        hits > 0,
        f64::from(u8::from(hits > 0)),
        "1",
        format!("{hits} of {shots} rounds"),
    );
    Ok(r)
}

/// The penetration chance (%) of `ammo` at `vehicle`'s zone at `range` m.
fn pen_pct(w: &World, ammo: &str, vehicle: &str, zone: u8, range: i32) -> Result<f64> {
    let s = w.match_setup().ok_or_else(|| anyhow!("match"))?;
    let a = s
        .ammo
        .iter()
        .position(|x| x.id == ammo)
        .ok_or_else(|| anyhow!("no {ammo}"))?;
    let v = s
        .vehicles
        .iter()
        .position(|x| x.id == vehicle)
        .ok_or_else(|| anyhow!("no {vehicle}"))?;
    let t = &w.game().expect("match").armour_tables();
    let shot = plate_shot(
        &t.ammo[a],
        &t.vehicles[v],
        zone,
        Fx::from_int(range),
        deg(0),
        Fx::ZERO,
        false,
    );
    Ok(f64::from(shot.chance_ppm) * 100.0 / f64::from(armour::PPM))
}

/// Case 31, the car parts (trucks in Build B): the new guns against plates,
/// a rifle at the M8's front, and the optics' sight gain measured by a
/// range ladder on real spotting.
pub(super) fn case31(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(31, "recon guns, thin plates, stationary sight");
    let mut w = open_top_world(env, 1)?;
    w.step(&[]);
    let front = PlateGroup::Front.zone(0);
    let p2 = pen_pct(&w, "ww2:2cm_pzgr39", "ww2:usa_m3a1", front, 100)?;
    r.check_v(
        "pen_2cm_m3a1_front_100_pct",
        p2 >= 80.0,
        round3(p2),
        ">= 80",
        "2 cm PzGr 39 at 12.7 mm, 100 m".to_string(),
    );
    let p37 = pen_pct(&w, "ww2:37mm_m51", "ww2:ger_pz4_h", front, 500)?;
    r.check_v(
        "pen_37mm_pz4_front_pct",
        p37 <= 5.0,
        round3(p37),
        "<= 5",
        "37 mm M51 at the Pz IV front, 500 m".to_string(),
    );
    // A rifle at the M8's front from 50 m.
    let gun = weapon_idx(&w, "ww2:kar98k")?;
    let (mut pens, mut struck) = (0u32, 0u32);
    for (seed, flip) in env.runs() {
        r.runs += 1;
        let mut w = open_top_world(env, seed * 64 + flip as u64)?;
        let (_, v) = spawn_card(&mut w, 1, USA, at(500, 500), dir_deg(0))?;
        no_ammo(&mut w, v);
        w.step(&[]);
        for k in 0..super::CASE21_SHOTS {
            let f = *w
                .game()
                .and_then(|g| g.vehicle_derived().frame(v))
                .ok_or_else(|| anyhow!("frame"))?;
            let b = &f.boxes[0];
            let z = b.center + b.axes.x * (b.half.x - Fx::from_ratio(1, 10));
            let up = FxVec3::new(
                Fx::ZERO,
                Fx::from_ratio(k as i64 * 10 - 25, 100),
                Fx::from_ratio(10, 100),
            );
            if let Some((hit, pen)) = super::small_shot(&mut w, 0, gun, z + up, 0, 50) {
                struck += u32::from(hit);
                pens += u32::from(pen);
            }
        }
    }
    let rifle = f64::from(pens) * 100.0 / f64::from(struck.max(1));
    r.check_v(
        "rifle_m8_front_pen_pct",
        pens == 0,
        round3(rifle),
        "0",
        format!("{pens} of {struck} rounds that struck"),
    );
    // MG 42 at a truck's side from 150 m (7.3): nearly every round through;
    // how long until the driver or engine goes.
    let mg = weapon_idx(&w, "ww2:mg42")?;
    let interval_s = w.match_setup().map_or(0.05, |s| {
        f64::from(s.weapons[mg as usize].interval_milli) / 30_000.0
    });
    let (mut mg_pens, mut mg_struck) = (0u32, 0u32);
    let mut stop_shots: Vec<u32> = Vec::new();
    for (seed, flip) in env.runs() {
        r.runs += 1;
        let (p, tc, _) = side(flip);
        let shooter = 1 - p;
        let mut w = open_top_world(env, seed * 128 + flip as u64)?;
        let v = truck_at(&mut w, p, tc, 500, 500, 0)?;
        w.step(&[]);
        for k in 0..40u32 {
            let f = *w
                .game()
                .and_then(|g| g.vehicle_derived().frame(v))
                .ok_or_else(|| anyhow!("frame"))?;
            let b = &f.boxes[0];
            // The cab's right side, over the door by the driver.
            let z = b.center + b.axes.y * (b.half.y - Fx::from_ratio(1, 10));
            let off = b.axes.x * Fx::from_ratio((k as i64 % 5) * 20 - 80, 100)
                + b.axes.z * Fx::from_ratio((k as i64 % 3) * 15 - 15, 100);
            if let Some((hit, pen)) = super::small_shot(&mut w, shooter, mg, z + off, 90, 150) {
                mg_struck += u32::from(hit);
                mg_pens += u32::from(pen);
            }
            let Some(vs) = vstate(&w, v) else { break };
            let engine = vs.components[ComponentKind::Engine.index()];
            let driver = vs.seats[0].hp;
            if engine == 0 || driver == 0 {
                stop_shots.push(k + 1);
                break;
            }
        }
    }
    let mg_pct = f64::from(mg_pens) * 100.0 / f64::from(mg_struck.max(1));
    r.check_v(
        "mg42_truck_pen_pct",
        mg_pct >= 90.0,
        round3(mg_pct),
        ">= 90",
        format!("{mg_pens} of {mg_struck} that struck"),
    );
    stop_shots.sort_unstable();
    let med = stop_shots
        .get(stop_shots.len() / 2)
        .copied()
        .map_or(f64::INFINITY, f64::from);
    let stop_s = med * interval_s;
    r.check_v(
        "mg42_stops_truck_s",
        stop_s <= 60.0,
        round3(stop_s),
        "<= 60",
        format!(
            "median {med} rounds through the cab, {} of the runs stopped",
            stop_shots.len()
        ),
    );
    // The farthest tank (spot size 200 %) the car spots settled, against a
    // car with no stationary bonus: a ladder 400-660 m.
    let (mut base, mut settled) = (0i32, 0i32);
    for hold in [false, true] {
        let mut best = 0;
        let mut d = 400;
        while d <= 660 {
            let mut setup = env.setup(77)?;
            let mut w = super::open_top_from(&mut setup, env.threads, |s| {
                if hold {
                    s.vehicles
                        .iter_mut()
                        .for_each(|x| x.stationary_sight_pct = 100);
                }
            })?;
            let (_, v) = spawn_card(&mut w, 0, GER, at(300, 500), dir_deg(0))?;
            no_ammo(&mut w, v);
            let (_, m) = spawn_card(
                &mut w,
                1,
                "ww2:usa_m4a3_75_squad",
                at(300 + d, 500),
                dir_deg(90),
            )?;
            no_ammo(&mut w, m);
            for _ in 0..8 * hz(&w) {
                w.step(&[]);
            }
            if w.entities().combat()[m.index()].seen_by & 1 != 0 {
                best = d;
            }
            d += 20;
        }
        if hold {
            base = best;
        } else {
            settled = best;
        }
    }
    let gain = f64::from(settled - base) * 100.0 / f64::from(base.max(1));
    r.row(
        "sight ladder",
        format!("no bonus {base} m, settled {settled} m"),
    );
    r.check_v(
        "stationary_sight_gain_pct",
        gain >= 30.0,
        round3(gain),
        ">= 30",
        format!("{base} m to {settled} m"),
    );
    Ok(r)
}

/// Case 32 (owner request 2026-11-01, spec 9): the AI brings supply to a
/// stranded tank. The tank stands immobile with 10 % of its rounds, 350 m
/// from a base holding a loaded truck; no enemy on the map.
pub(super) fn case32(env: &Env) -> Result<CaseReport> {
    let mut r = CaseReport::new(32, "AI truck serves a stranded tank (7.5)");
    let (mut served, mut gained) = (0u32, 0u64);
    for (seed, flip) in env.runs() {
        r.runs += 1;
        let (p, tc, tank) = side(flip);
        let mut s = env.setup(seed * 37 + flip as u64)?;
        let mut w = super::open_top_from(&mut s, env.threads, |s| {
            s.players[p as usize].is_ai = true;
        })?;
        let setup = w.match_setup().ok_or_else(|| anyhow!("setup"))?.clone();
        let b = &setup.map.bases[setup.players[p as usize].base as usize];
        let base: FxVec2 = b.center.into();
        let mid = FxVec2::from_int(setup.map.size_m / 2, setup.map.size_m / 2);
        let toward = (mid - base).normalize_or_zero();
        let to = base + toward * Fx::from_int(350);
        let v = tank_at_tenth(&mut w, p, tank, to.x.floor_int(), to.y.floor_int())?;
        w.edit_vehicle(v, |vs| {
            for c in [ComponentKind::TrackLeft, ComponentKind::TrackRight] {
                vs.components[c.index()] = 0;
            }
        });
        let (_, t) = spawn_card(&mut w, p, tc, base, toward)?;
        w.edit_vehicle(t, |vs| {
            vs.cargo_g = [120_000, 40_000, 320_000];
        });
        let n0 = held_rounds(&w, v)?;
        for _ in 0..150 * hz(&w) {
            w.step(&[]);
        }
        let g = held_rounds(&w, v)?.saturating_sub(n0) as u64;
        gained += g;
        let serves = w
            .game()
            .and_then(|g| g.ai(PlayerId(p)))
            .map_or(0, |a| a.counters.truck_serves);
        served += u32::from(serves > 0 && g >= 1);
    }
    let runs = f64::from(r.runs);
    let pct_served = round3(f64::from(served) * 100.0 / runs);
    r.check_v(
        "stranded_served_pct",
        pct_served >= 80.0,
        pct_served,
        ">= 80",
        format!("{served} of {} runs", r.runs),
    );
    let mean = round3(gained as f64 / runs);
    r.check_v(
        "rounds_gained_mean",
        mean >= 5.0,
        mean,
        ">= 5",
        format!("{gained} rounds in all"),
    );
    Ok(r)
}
