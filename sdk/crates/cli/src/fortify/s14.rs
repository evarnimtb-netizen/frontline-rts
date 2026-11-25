//! S14 fortify-test rows (docs/slices/S14-assault-kit.md 9, rows R1 and
//! R2; Build A): the `fortify10.s14` scene. The S14 switches are on in
//! the row only (`[flame] enabled` 1); every row is driven by staged fire
//! and player commands, no AI code runs. Build A has no flame def in the
//! mod: the rows make one ([`make_flamer`], as the sim's S14 tests do:
//! the line card's second man's weapon turned flame, 30 m, 25 m/s, 20
//! damage, 60 units a tank). Build B replaces it with the mod's
//! flamethrowers.
//!
//! - `slit` (R1, hard): the S10b bunker row's layout ([`super::bunker`]:
//!   the owner's bunker at (`TRENCH_X`, `LINE_Y`) facing west, five of
//!   his line men in the pit, held); the other side's flame man held 20 m
//!   in front, `SuppressUnits` at the slit (a line across it on the front
//!   walls' centre line), the rest of his squad and the rest of the
//!   owner's squad held 300 m off (the five are the only men who can
//!   answer him). `out_pct`
//!   (dead or downed of the five) 30 s after the order, `runs_4_out` (runs
//!   with at least 4 out) of `runs` (seeds x both owners). Controls:
//!   `closed` (the front walls meet, as the S10b satchel row) and `back`
//!   (the flame man 20 m behind, at the back wall). Pass: `out_pct` >= 80,
//!   `runs_4_out` >= 80 % of runs, both controls <= 10.
//! - `pool` (R2, hard): two flame men of one line squad `SuppressUnits` at
//!   empty ground 20 m aside (inside the test flamer's 30 m) while a staged rifle and MG volley (40 rounds
//!   a tick into empty ground, the pool cut to [`POOL_CAP`] slots in the
//!   row) saturates the pool and three mortar bombs land 2 s apart; the
//!   control is the same scene with `[flame] enabled` 0 (the flame men
//!   hold). `dropped` (pool drops with flame: the sim's `shots_dropped`
//!   plus staged rounds the full pool refused), `control.dropped`,
//!   `drop_diff` (|with - without| summed over runs), `pool_diff_ticks`
//!   (ticks whose pool differs), `over_cap` (ticks with more particles
//!   live than `particles_per_player`), `flame_fired`, `cap`. Pass:
//!   `drop_diff`, `pool_diff_ticks`, `over_cap` 0 with the pool saturated
//!   and particles fired.
use anyhow::{Result, bail};
use frontline_content::{MatchSetup, WeaponClass};
use frontline_sim::command::CommandKind;
use frontline_sim::{Fx, FxVec2, PlayerId, SimId, World};
use serde::Serialize;

use super::bunker::{Site, bunker_facing, hold};
use super::s10b::{fields, pct};
use super::s11a1::par_seeds;
use super::{EAST, FortifyArgs, LINE_Y, TRENCH_X, WEST, at, members, squad, step_with, world_with};

/// R1: the flame man's distance from the bunker's centre (m).
const SLIT_M: i32 = 20;
/// R1: seconds from his order to the count.
const SLIT_S: u64 = 30;
/// R1: the front walls' centre line from the bunker's centre (cm, the
/// def's front wall `offset_cm` x).
const FRONT_CM: i32 = 190;
/// R2: the pool's slots in the row (the rules' `projectile_cap`, cut so a
/// 40-round-a-tick volley fills it).
pub const POOL_CAP: u32 = 256;
/// R2: staged rounds a tick.
const POOL_ROUNDS: u32 = 40;
/// R2: the row's length (ticks).
const POOL_TICKS: u64 = 300;

/// `fortify10.s14.slit` controls.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SlitControl {
    pub out_pct: Option<f64>,
    pub runs_4_out: Option<f64>,
}

/// `fortify10.s14.slit` (R1).
#[derive(Clone, Debug, Default, Serialize)]
pub struct SlitRow {
    pub runs: Option<f64>,
    /// Exposure: men in the pit (five a run).
    pub men: Option<f64>,
    pub out_pct: Option<f64>,
    pub runs_4_out: Option<f64>,
    /// Particles the flame men fired (all runs).
    pub flame_fired: Option<f64>,
    pub flame_hits: Option<f64>,
    pub flame_in_bunker_hits: Option<f64>,
    pub closed: SlitControl,
    pub back: SlitControl,
    pub pass: Option<bool>,
}

/// `fortify10.s14.pool` control.
#[derive(Clone, Debug, Default, Serialize)]
pub struct PoolControl {
    pub dropped: Option<f64>,
}

/// `fortify10.s14.pool` (R2).
#[derive(Clone, Debug, Default, Serialize)]
pub struct PoolRow {
    pub runs: Option<f64>,
    pub dropped: Option<f64>,
    pub drop_diff: Option<f64>,
    pub pool_diff_ticks: Option<f64>,
    pub over_cap: Option<f64>,
    pub flame_fired: Option<f64>,
    pub cap: Option<f64>,
    pub control: PoolControl,
    pub pass: Option<bool>,
}

/// The S14 rows (`fortify10.s14`).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S14Report {
    pub slit: SlitRow,
    pub pool: PoolRow,
}

/// Summary lines, one per row.
pub fn summary_text(r: &S14Report) -> String {
    let v = serde_json::to_value(r).unwrap_or_default();
    ["slit", "pool"]
        .iter()
        .map(|k| format!("  S14 {k}: {}\n", fields(&v[*k])))
        .collect()
}

/// The S14 rows.
pub fn s14_rows(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<S14Report> {
    Ok(S14Report {
        slit: slit_row(args, seeds.clone())?,
        pool: pool_row(args, seeds)?,
    })
}

/// Test-made flame defs (module docs): every line card of player `p`'s
/// faction gets its second man's weapon as a flamethrower at `slots`
/// (member indices). `[flame] enabled` = `on`.
pub fn make_flamer(s: &mut MatchSetup, p: u8, slots: &[usize], on: bool) {
    s.rules.flame.enabled = on as u32;
    let f = s.players[p as usize].faction as usize;
    let types: Vec<u16> = s.factions[f]
        .roster
        .iter()
        .copied()
        .filter(|&t| s.type_has_tag(t, "line"))
        .collect();
    for t in types {
        let ty = &s.squad_types[t as usize];
        let Some(src) = ty.members.get(1).map(|m| m.weapon as usize) else {
            continue;
        };
        let mut w = s.weapons[src].clone();
        w.id = format!("test:flamer_{t}");
        w.class = WeaponClass::Flame;
        w.range_max = frontline_content::Fixed::from_int(30);
        w.muzzle_speed = frontline_content::Fixed::from_int(25);
        w.damage = 20;
        w.suppression = 35;
        w.magazine = 60;
        w.setup_ticks = 0;
        w.hold_beyond_pct = 100;
        w.pen.clear();
        w.core = None;
        w.engage_still = w.engage_still.min(w.range_max);
        w.engage_moving = w.engage_moving.min(w.range_max);
        s.weapons.push(w);
        let wi = (s.weapons.len() - 1) as u16;
        let ty = &mut s.squad_types[t as usize];
        for &k in slots {
            if let Some(m) = ty.members.get_mut(k) {
                m.weapon = wi;
            }
        }
    }
}

fn is_flame(w: &World, m: SimId) -> bool {
    let s = w.match_setup().unwrap();
    let wi = w.entities().kit()[m.index()].weapon as usize;
    s.weapons[wi].class == WeaponClass::Flame
}

/// Dead or downed.
fn out(w: &World, m: SimId) -> bool {
    !super::s10a::alive(w, m)
        || w.entities().combat()[m.index()].wound.state == frontline_sim::wounds::DOWNED
}

/// R1 variants.
#[derive(Clone, Copy, PartialEq)]
enum Slit {
    Open,
    Closed,
    Back,
}

/// One R1 scene with the flame man's order given and not yet run:
/// (world, the five, every man to hold).
fn slit_scene(
    args: &FortifyArgs,
    seed: u64,
    own: u8,
    v: Slit,
) -> Result<(World, Vec<SimId>, Vec<SimId>)> {
    let att = 1 - own;
    let mut w = world_with(args, seed, |s| {
        make_flamer(s, att, &[1], true);
        if v == Slit::Closed
            && let Some(f) = s.forts.iter_mut().find(|f| f.id == "ww2:bunker")
        {
            // The two front walls meet at y 0 (no slit), as the S10b
            // satchel row's control.
            for k in [4, 5] {
                f.pieces[k].half[1] = frontline_content::setup::Fixed::from_ratio(185, 200);
                f.pieces[k].offset[1] = frontline_content::setup::Fixed::from_ratio(
                    if k == 4 { -925 } else { 925 },
                    1000,
                );
            }
        }
    })?;
    let (_, five) = bunker_facing(&mut w, own, Site::Bunker, WEST)?;
    let sq = squad(&w, att, "line")?;
    let all = members(&w, sq);
    let Some(&man) = all.get(1) else {
        bail!("seed {seed}: no second man in the line squad");
    };
    if !is_flame(&w, man) {
        bail!("seed {seed}: the line card's second man has no flame weapon");
    }
    // Facing west: the front walls are west of the centre, the back wall
    // east.
    let c = at(TRENCH_X, LINE_Y);
    let sign = if v == Slit::Back { 1 } else { -1 };
    let wall = c.x + Fx::from_int(sign * FRONT_CM).div_int(100);
    let stand = FxVec2::new(c.x + Fx::from_int(sign * SLIT_M), c.y);
    let mut held = five.clone();
    for (k, &m) in all.iter().enumerate() {
        let p = if m == man {
            stand
        } else {
            at(TRENCH_X - 300 + k as i32, LINE_Y + 300)
        };
        w.place_soldier(m, p);
        held.push(m);
    }
    // The rest of the owner's line squad (60 m east in the bunker row)
    // goes 300 m off too: the five in the pit are the only men who can
    // answer the flame man (through the slit).
    let own_sq = squad(&w, own, "line")?;
    for (k, m) in members(&w, own_sq).into_iter().enumerate() {
        if !five.contains(&m) {
            w.place_soldier(m, at(TRENCH_X + 300 + k as i32, LINE_Y - 300));
            held.push(m);
        }
    }
    let half = Fx::from_milli(300);
    step_with(
        &mut w,
        att,
        vec![CommandKind::SuppressUnits {
            units: vec![man],
            line: vec![FxVec2::new(wall, c.y - half), FxVec2::new(wall, c.y + half)],
        }],
    );
    Ok((w, five, held))
}

/// One R1 run: (out of the five, particles fired, hits, hits under the
/// roof).
fn slit_once(args: &FortifyArgs, seed: u64, own: u8, v: Slit) -> Result<[u32; 4]> {
    let att = 1 - own;
    let (mut w, five, held) = slit_scene(args, seed, own, v)?;
    hold(&mut w, &held, SLIT_S);
    let s14 = &w.game().unwrap().players()[att as usize].s14;
    Ok([
        five.iter().filter(|&&m| out(&w, m)).count() as u32,
        s14.flame_fired,
        s14.flame_hits,
        s14.flame_in_bunker_hits,
    ])
}

/// The `slit` row (R1, module docs).
pub fn slit_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<SlitRow> {
    let all = par_seeds(seeds, |seed| {
        let mut v = Vec::new();
        for own in [0u8, 1] {
            v.push([
                slit_once(args, seed, own, Slit::Open)?,
                slit_once(args, seed, own, Slit::Closed)?,
                slit_once(args, seed, own, Slit::Back)?,
            ]);
        }
        Ok(v)
    })?;
    let runs: Vec<[[u32; 4]; 3]> = all.into_iter().flatten().collect();
    let n = runs.len() as u32;
    let men = 5 * n;
    let sum = |k: usize, f: usize| runs.iter().map(|r| r[k][f]).sum::<u32>();
    let four = |k: usize| runs.iter().filter(|r| r[k][0] >= 4).count() as f64;
    let ctl = |k: usize| SlitControl {
        out_pct: pct(sum(k, 0), men),
        runs_4_out: (n > 0).then(|| four(k)),
    };
    let (closed, back) = (ctl(1), ctl(2));
    let out_pct = pct(sum(0, 0), men);
    let pass = match (out_pct, closed.out_pct, back.out_pct) {
        (Some(o), Some(c), Some(b)) => {
            Some(o >= 80.0 && four(0) * 10.0 >= 8.0 * n as f64 && c <= 10.0 && b <= 10.0)
        }
        _ => None,
    };
    Ok(SlitRow {
        runs: (n > 0).then_some(n as f64),
        men: (n > 0).then_some(men as f64),
        out_pct,
        runs_4_out: (n > 0).then(|| four(0)),
        flame_fired: Some(sum(0, 1) as f64),
        flame_hits: Some(sum(0, 2) as f64),
        flame_in_bunker_hits: Some(sum(0, 3) as f64),
        closed,
        back,
        pass,
    })
}

/// The rifle and MG of player `p`'s line card (its first and second
/// men's weapons, before the flame edit).
fn guns(w: &World, p: u8) -> Result<[u16; 2]> {
    let s = w.match_setup().unwrap();
    let f = s.players[p as usize].faction as usize;
    let Some(&t) = s.factions[f]
        .roster
        .iter()
        .find(|&&t| s.type_has_tag(t, "mg") || s.type_has_tag(t, "line"))
    else {
        bail!("no line or mg card");
    };
    let ty = &s.squad_types[t as usize];
    let rifle = ty.members[0].weapon;
    let mg = ty
        .members
        .iter()
        .map(|m| m.weapon)
        .find(|&x| {
            s.weapons[x as usize].class != WeaponClass::Flame
                && x != rifle
                && s.weapons[x as usize].he.is_none()
        })
        .unwrap_or(rifle);
    Ok([rifle, mg])
}

/// The other faction's 81 mm mortar.
fn mortar(w: &World, p: u8) -> Result<u16> {
    let s = w.match_setup().unwrap();
    let germany = super::faction(s, p).ends_with("germany");
    let id = if germany {
        "ww2:grw34"
    } else {
        "ww2:m1_mortar"
    };
    s.weapons
        .iter()
        .position(|x| x.id == id)
        .map(|i| i as u16)
        .ok_or_else(|| anyhow::anyhow!("no weapon {id}"))
}

/// One R2 scene, set up and not yet run: (world, flame men).
fn pool_scene(args: &FortifyArgs, seed: u64, p: u8, on: bool) -> Result<(World, Vec<SimId>)> {
    let mut w = world_with(args, seed, |s| {
        super::s11a1::flat(s);
        s.rules.projectile_cap = POOL_CAP;
        make_flamer(s, p, &[1, 3], on);
        // No reveal: his fire must not change what the enemy sees.
        for x in s
            .weapons
            .iter_mut()
            .filter(|x| x.class == WeaponClass::Flame)
        {
            (x.fire_reveal_pct, x.fire_reveal_ticks) = (100, 0);
        }
    })?;
    let sq = squad(&w, p, "line")?;
    let all = members(&w, sq);
    let men: Vec<SimId> = [1, 3].iter().filter_map(|&k| all.get(k).copied()).collect();
    if men.len() != 2 || !men.iter().all(|&m| is_flame(&w, m)) {
        bail!("seed {seed}: the line squad has no two flame men");
    }
    // Everybody else far off: no fight but the staged one.
    for (id, q) in w.game().unwrap().squads().clone() {
        if id != sq {
            let corner = if q.owner == PlayerId(p) {
                at(60, 950)
            } else {
                at(964, 950)
            };
            w.place_squad(id, corner, EAST);
        }
    }
    w.place_squad(sq, at(500, 300), EAST);
    w.step(&[]);
    step_with(
        &mut w,
        p,
        vec![CommandKind::SuppressUnits {
            units: men.clone(),
            line: vec![at(500, 280), at(504, 280)],
        }],
    );
    Ok((w, men))
}

/// One R2 run, player `p`'s flame men: [drops with, drops without, ticks
/// the pool differs, ticks over the cap, particles fired].
fn pool_once(args: &FortifyArgs, seed: u64, p: u8) -> Result<[u32; 5]> {
    let (mut on, _) = pool_scene(args, seed, p, true)?;
    let (mut off, _) = pool_scene(args, seed, p, false)?;
    let g = guns(&on, p)?;
    let bomb = mortar(&on, p)?;
    let cap = on.match_setup().unwrap().rules.flame.particles_per_player as usize;
    let hz = on.match_setup().unwrap().tick_hz as u64;
    let (d_on, d_off) = (on.stats().shots_dropped, off.stats().shots_dropped);
    let (mut diff, mut over) = (0u32, 0u32);
    let mut refused = [0u32; 2];
    for t in 0..POOL_TICKS {
        for (r, w) in [&mut on, &mut off].into_iter().enumerate() {
            for k in 0..POOL_ROUNDS as i32 {
                let a = at(300, 600 + k);
                let z = w.terrain().height_at(a) + Fx::from_int(2);
                let from = a.extend(z);
                let to = at(700, 600 + k).extend(z);
                // A full pool refuses the staged round: a drop.
                if !w.fire_round_now(p, g[k as usize % 2], from, to) {
                    refused[r] += 1;
                }
            }
            if t % (2 * hz) == 0 && t < 6 * hz {
                let _ = w.drop_shell_now(p, bomb, at(700, 800));
            }
            w.step(&[]);
        }
        if on.projectiles().slots() != off.projectiles().slots() {
            diff += 1;
        }
        if on.game().unwrap().flame().len() > cap {
            over += 1;
        }
    }
    let fired = on.game().unwrap().players()[p as usize].s14.flame_fired;
    Ok([
        (on.stats().shots_dropped - d_on) as u32 + refused[0],
        (off.stats().shots_dropped - d_off) as u32 + refused[1],
        diff,
        over,
        fired,
    ])
}

/// The `pool` row (R2, module docs).
pub fn pool_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<PoolRow> {
    let all = par_seeds(seeds, |seed| {
        let mut v = Vec::new();
        for p in [0u8, 1] {
            v.push(pool_once(args, seed, p)?);
        }
        Ok(v)
    })?;
    let runs: Vec<[u32; 5]> = all.into_iter().flatten().collect();
    let sum = |f: usize| runs.iter().map(|r| r[f] as u64).sum::<u64>();
    let drop_diff: u64 = runs.iter().map(|r| r[0].abs_diff(r[1]) as u64).sum();
    let cap = {
        let w = world_with(args, 1, |_| {})?;
        w.match_setup().unwrap().rules.flame.particles_per_player
    };
    let pass = (!runs.is_empty())
        .then(|| drop_diff == 0 && sum(2) == 0 && sum(3) == 0 && sum(0) > 0 && sum(4) > 0);
    Ok(PoolRow {
        runs: Some(runs.len() as f64),
        dropped: Some(sum(0) as f64),
        drop_diff: Some(drop_diff as f64),
        pool_diff_ticks: Some(sum(2) as f64),
        over_cap: Some(sum(3) as f64),
        flame_fired: Some(sum(4) as f64),
        cap: Some(cap as f64),
        control: PoolControl {
            dropped: Some(sum(1) as f64),
        },
        pass,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R1/R2 (Build A): each row runs on one seed; the pool row's
    /// invariants hold (saturated, particles fired, no drop or slot
    /// differs, never over the cap); the slit row counts its men. (The R1
    /// verdict is read off the fortify10 set, not one seed.)
    #[test]
    fn fortify_s14_rows_run() {
        let r = s14_rows(&FortifyArgs::default(), 1..2).unwrap();
        eprintln!("{}", serde_json::to_string(&r).unwrap());
        assert_eq!((r.slit.runs, r.slit.men), (Some(2.0), Some(10.0)));
        let p = &r.pool;
        assert!(p.dropped.unwrap() > 0.0, "the pool is saturated: {p:?}");
        assert!(p.flame_fired.unwrap() > 0.0, "control: flame fired");
        assert_eq!(p.drop_diff, Some(0.0), "{p:?}");
        assert_eq!(p.pool_diff_ticks, Some(0.0), "{p:?}");
        assert_eq!(p.over_cap, Some(0.0), "{p:?}");
        assert_eq!(p.pass, Some(true));
    }

    /// The slit scene with `[flame] enabled` 1 plays the same at 1, 2, 3
    /// and 8 threads (state hashes every 30 ticks for 30 s). Control:
    /// particles were fired (flame came into play).
    #[test]
    fn s14_slit_scene_hash_is_thread_count_free() {
        let hashes = |threads: usize| {
            let args = FortifyArgs {
                threads,
                ..FortifyArgs::default()
            };
            let (mut w, _, held) = slit_scene(&args, 1, 0, Slit::Open).unwrap();
            let mut v = Vec::new();
            for _ in 0..30 {
                super::super::bunker::hold_ticks(&mut w, &held, 30);
                v.push(w.state_hash());
            }
            (v, w.game().unwrap().players()[1].s14.flame_fired)
        };
        let (one, fired) = hashes(1);
        assert!(fired > 0, "flame came into play");
        for n in [2, 3, 8] {
            assert_eq!(hashes(n).0, one, "{n} threads");
        }
    }

    /// One-off: the S14 rows on seeds 1..=`S14_SEEDS` (default 5; prints
    /// the values); run by hand in release.
    #[test]
    #[ignore]
    fn s14_rows_one_off() {
        let t = std::time::Instant::now();
        let n: u64 = std::env::var("S14_SEEDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);
        let r = s14_rows(&FortifyArgs::default(), 1..n + 1).unwrap();
        eprintln!("{}", serde_json::to_string(&r).unwrap());
        eprintln!("s14 rows wall s: {:.1}", t.elapsed().as_secs_f64());
    }
}
