//! S14 fortify-test rows (docs/slices/S14-assault-kit.md 9, rows R1-R4
//! and R6): the `fortify10.s14` scene. The S14 switches are on in the
//! row only; every row is driven by staged fire and player commands, no
//! AI code runs. The defs resolve with `[flame] enabled` 1
//! ([`flame_world`]), so the assault cards (Sturmpioniergruppe, Assault
//! Section) carry the mod's flame man (`flame_on` members: Flammenwerfer
//! 41, M1A1); a row's control sets the switch back after resolve.
//!
//! - `slit` (R1): the S10b bunker row's layout ([`super::bunker`]: the
//!   owner's bunker at (`TRENCH_X`, `LINE_Y`) facing west, five of his
//!   line men in the pit, held); the other side's assault card with its
//!   flame man held 20 m in front, `SuppressUnits` at the slit (a line
//!   across it on the front walls' centre line), the rest of his squad
//!   and the rest of the owner's squad held 300 m off (the five are the
//!   only men who can answer him). `out_pct` (dead or downed of the five)
//!   30 s after the order, `runs_4_out` (runs with at least 4 out) of
//!   `runs` (seeds x both owners), `flamer_out` (runs whose flame man is
//!   out at the count). Controls: `closed` (the front walls meet, as the
//!   S10b satchel row) and `back` (the flame man 20 m behind, at the back
//!   wall). Pass: `out_pct` >= 80, `runs_4_out` >= 80 % of runs, both
//!   controls <= 10. Report only (`hard` false; S14a Build B): five
//!   alert defenders kill the lone flame man before his fire tells; the
//!   owner's no-tuning rule leaves that to the later balance pass (log).
//! - `pool` (R2, hard): two flame men (one of each of two assault cards)
//!   `SuppressUnits` at empty ground 20 m aside while a staged rifle and
//!   MG volley (40 rounds a tick into empty ground, the pool cut to
//!   [`POOL_CAP`] slots in the row) saturates the pool and three mortar
//!   bombs land 2 s apart; the control is the same scene with `[flame]
//!   enabled` 0 after resolve (the same men; the flame men hold).
//!   `dropped` (pool drops with flame: the sim's `shots_dropped` plus
//!   staged rounds the full pool refused), `control.dropped`, `drop_diff`
//!   (|with - without| summed over runs), `pool_diff_ticks` (ticks whose
//!   pool differs), `over_cap` (ticks with more particles live than
//!   `particles_per_player`), `flame_fired`, `cap`. Pass: `drop_diff`,
//!   `pool_diff_ticks`, `over_cap` 0 with the pool saturated and
//!   particles fired.
//! - `trench` (R3, report): the S10a assault scene (the owner's line
//!   squad in the 32 m trench, the other side 60 m east) with the
//!   assault card walking into the trench: with flame (resolved with
//!   `[flame] enabled` 1) against the rifle baseline (resolved with 0:
//!   the cards' `flame_off` men). `defenders_out_pct`,
//!   `attackers_lost_pct`, `to_trench_s` (mean seconds until a living
//!   attacker stands within 2 m of the trench line, over the runs that
//!   got there), `reached` (those runs); `rifle.*` the same.
//! - `clear` (R4, report): an L trench held by four of the owner's men
//!   6 m apart (held on their spots), the other side's assault card
//!   ordered `ClearTrench` at its south end (`[clear] enabled` 1);
//!   control: the same squad by `AttackMove` to the same point.
//!   `steps`, `done` (clears ended), `defenders_out_pct`,
//!   `attackers_lost_pct`; `attack_move.*` the same.
//! - `under_fire` (R6, report): the owner's line squad in the trench
//!   under the other side's MG (`SuppressUnits` on the trench from 150 m,
//!   as S3b, 5 s ahead), then a flame man 25 m in front `SuppressUnits` on
//!   the trench; 30 s later `out_pct` of the line squad, `stepped_out`
//!   (the owner's F6 counter), `flame_fired`, `flame_in_trench_hits`.
use anyhow::{Result, bail};
use frontline_content::{MatchSetup, WeaponClass};
use frontline_sim::command::CommandKind;
use frontline_sim::{Fx, FxVec2, PlayerId, SimId, SquadId, World};
use serde::Serialize;

use super::bunker::{Site, bunker_facing, hold};
use super::s10b::{fields, pct};
use super::s11a1::par_seeds;
use super::{
    ASSAULT_S, ASSAULT_START_M, EAST, FortifyArgs, LINE_Y, SUPPRESS_LEAD_S, SUPPRESS_START_M,
    TRENCH_X, WEST, at, call_in, dig, members, pos, squad, step_with,
};

/// R1: the flame man's distance from the bunker's centre (m).
const SLIT_M: i32 = 20;
/// R1, R6: seconds from his order to the count.
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
/// R4: the clear's length (s).
const CLEAR_S: u64 = 120;
/// R6: the flame man's distance from the trench (m).
const UNDER_FIRE_M: i32 = 25;

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
    /// Runs whose flame man is dead or downed at the count.
    pub flamer_out: Option<f64>,
    /// Particles the flame men fired (all runs).
    pub flame_fired: Option<f64>,
    pub flame_hits: Option<f64>,
    pub flame_in_bunker_hits: Option<f64>,
    pub closed: SlitControl,
    pub back: SlitControl,
    pub pass: Option<bool>,
    /// Whether the row is a hard check (false: report only, module docs).
    pub hard: bool,
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

/// One side of `fortify10.s14.trench` (R3).
#[derive(Clone, Debug, Default, Serialize)]
pub struct TrenchSide {
    pub runs: Option<f64>,
    pub defenders_out_pct: Option<f64>,
    pub attackers_lost_pct: Option<f64>,
    pub to_trench_s: Option<f64>,
    pub reached: Option<f64>,
    pub flame_fired: Option<f64>,
}

/// `fortify10.s14.trench` (R3, report): with flame, and `rifle`.
#[derive(Clone, Debug, Default, Serialize)]
pub struct TrenchRow {
    #[serde(flatten)]
    pub flame: TrenchSide,
    pub rifle: TrenchSide,
}

/// One side of `fortify10.s14.clear` (R4).
#[derive(Clone, Debug, Default, Serialize)]
pub struct ClearSide {
    pub runs: Option<f64>,
    pub steps: Option<f64>,
    pub done: Option<f64>,
    pub defenders_out_pct: Option<f64>,
    pub attackers_lost_pct: Option<f64>,
}

/// `fortify10.s14.clear` (R4, report): `ClearTrench`, and `attack_move`.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ClearRow {
    #[serde(flatten)]
    pub clear: ClearSide,
    pub attack_move: ClearSide,
}

/// `fortify10.s14.under_fire` (R6, report).
#[derive(Clone, Debug, Default, Serialize)]
pub struct UnderFireRow {
    pub runs: Option<f64>,
    pub men: Option<f64>,
    pub out_pct: Option<f64>,
    pub stepped_out: Option<f64>,
    pub flame_fired: Option<f64>,
    pub flame_in_trench_hits: Option<f64>,
    /// Runs whose flame man is dead or downed at the count.
    pub flamer_out: Option<f64>,
}

/// The S14 rows (`fortify10.s14`).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S14Report {
    pub slit: SlitRow,
    pub pool: PoolRow,
    pub trench: TrenchRow,
    pub clear: ClearRow,
    pub under_fire: UnderFireRow,
}

/// Summary lines, one per row.
pub fn summary_text(r: &S14Report) -> String {
    let v = serde_json::to_value(r).unwrap_or_default();
    ["slit", "pool", "trench", "clear", "under_fire"]
        .iter()
        .map(|k| format!("  S14 {k}: {}\n", fields(&v[*k])))
        .collect()
}

/// The S14 rows.
pub fn s14_rows(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<S14Report> {
    Ok(S14Report {
        slit: slit_row(args, seeds.clone())?,
        pool: pool_row(args, seeds.clone())?,
        trench: trench_row(args, seeds.clone())?,
        clear: clear_row(args, seeds.clone())?,
        under_fire: under_fire_row(args, seeds)?,
    })
}

/// A row's world (module docs): the defs resolve with `[flame] enabled`
/// = `resolve_on` (which members the cards' `when` pairs keep), both
/// players human, plenty of manpower, then `edit`.
pub fn flame_world(
    args: &FortifyArgs,
    seed: u64,
    resolve_on: bool,
    edit: impl FnOnce(&mut MatchSetup),
) -> Result<World> {
    let sk = crate::skirmish::SkirmishArgs {
        seed,
        mods: args.mods.clone(),
        rules_override: args.rules_override.clone(),
        ..crate::skirmish::SkirmishArgs::default()
    };
    let mut setup = crate::skirmish::match_setup_with(&sk, |d| {
        for r in d.rules.values_mut() {
            r.flame.get_or_insert_with(Default::default).enabled = resolve_on as u32;
        }
    })?;
    setup.players.iter_mut().for_each(|p| p.is_ai = false);
    setup.rules.start_manpower = 10_000;
    edit(&mut setup);
    Ok(World::new_match(setup, args.threads)?)
}

/// Player `p`'s assault card spawned at `c` facing `f`: (squad, its men,
/// its flame man if the card resolved with one).
fn assault(
    w: &mut World,
    p: u8,
    c: FxVec2,
    f: FxVec2,
) -> Result<(SquadId, Vec<SimId>, Option<SimId>)> {
    let s = w.match_setup().unwrap();
    let id = if super::faction(s, p).ends_with("germany") {
        "ww2:ger_sturmpioniergruppe"
    } else {
        "ww2:usa_assault_section"
    };
    let Some(t) = s.squad_type_index(id) else {
        bail!("no card {id}");
    };
    let Some(sq) = w.spawn_squad_at(PlayerId(p), t, c, f) else {
        bail!("the {id} was not spawned");
    };
    let men = members(w, sq);
    let man = men.iter().copied().find(|&m| is_flame(w, m));
    Ok((sq, men, man))
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
/// (world, the five, every man to hold, the flame man).
fn slit_scene(
    args: &FortifyArgs,
    seed: u64,
    own: u8,
    v: Slit,
) -> Result<(World, Vec<SimId>, Vec<SimId>, SimId)> {
    let att = 1 - own;
    let mut w = flame_world(args, seed, true, |s| {
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
    let (_, all, man) = assault(&mut w, att, at(TRENCH_X - 300, LINE_Y + 300), EAST)?;
    let Some(man) = man else {
        bail!("seed {seed}: the assault card has no flame man");
    };
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
    Ok((w, five, held, man))
}

/// One R1 run: (out of the five, particles fired, hits, hits under the
/// roof, the flame man out).
fn slit_once(args: &FortifyArgs, seed: u64, own: u8, v: Slit) -> Result<[u32; 5]> {
    let att = 1 - own;
    let (mut w, five, held, man) = slit_scene(args, seed, own, v)?;
    hold(&mut w, &held, SLIT_S);
    let s14 = &w.game().unwrap().players()[att as usize].s14;
    Ok([
        five.iter().filter(|&&m| out(&w, m)).count() as u32,
        s14.flame_fired,
        s14.flame_hits,
        s14.flame_in_bunker_hits,
        out(&w, man) as u32,
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
    let runs: Vec<[[u32; 5]; 3]> = all.into_iter().flatten().collect();
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
        flamer_out: Some(sum(0, 4) as f64),
        flame_fired: Some(sum(0, 1) as f64),
        flame_hits: Some(sum(0, 2) as f64),
        flame_in_bunker_hits: Some(sum(0, 3) as f64),
        closed,
        back,
        pass,
        hard: SLIT_HARD,
    })
}

/// R1 is report only (module docs; S14a Build B).
pub const SLIT_HARD: bool = false;

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
    let mut w = flame_world(args, seed, true, |s| {
        super::s11a1::flat(s);
        s.rules.projectile_cap = POOL_CAP;
        s.rules.flame.enabled = on as u32;
        // No reveal: his fire must not change what the enemy sees.
        for x in s
            .weapons
            .iter_mut()
            .filter(|x| x.class == WeaponClass::Flame)
        {
            (x.fire_reveal_pct, x.fire_reveal_ticks) = (100, 0);
        }
    })?;
    // Everybody else far off: no fight but the staged one.
    for (id, q) in w.game().unwrap().squads().clone() {
        let corner = if q.owner == PlayerId(p) {
            at(60, 950)
        } else {
            at(964, 950)
        };
        w.place_squad(id, corner, EAST);
    }
    let mut men = Vec::new();
    for x in [496, 504] {
        let (_, _, man) = assault(&mut w, p, at(x, 300), EAST)?;
        let Some(man) = man else {
            bail!("seed {seed}: the assault card has no flame man");
        };
        // He walks last in his file: put him on the line, 20 m from the
        // aim point (he then holds there on the order).
        w.place_soldier(man, at(x, 300));
        men.push(man);
    }
    w.step(&[]);
    step_with(
        &mut w,
        p,
        vec![CommandKind::SuppressUnits {
            units: men.clone(),
            line: vec![at(498, 280), at(502, 280)],
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
        let w = flame_world(args, 1, true, |_| {})?;
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

/// Dead or downed men of `men`.
fn outs(w: &World, men: &[SimId]) -> u32 {
    men.iter().filter(|&&m| out(w, m)).count() as u32
}

/// One R3 run (module docs): [defenders out, defenders, attackers lost,
/// attackers, ticks to the trench (0: never), particles fired].
fn trench_once(args: &FortifyArgs, seed: u64, dug: u8, flame: bool) -> Result<[u64; 6]> {
    let att = 1 - dug;
    let mut w = flame_world(args, seed, flame, |_| {})?;
    let d = squad(&w, dug, "line")?;
    dig(&mut w);
    w.place_squad(d, at(TRENCH_X, LINE_Y), EAST);
    let start_at = at(TRENCH_X + ASSAULT_START_M, LINE_Y);
    let (sq, atts, _) = assault(&mut w, att, start_at, WEST)?;
    w.place_squad(sq, start_at, WEST);
    let defs = members(&w, d);
    let hz = w.match_setup().unwrap().tick_hz as u64;
    step_with(
        &mut w,
        att,
        vec![CommandKind::Move {
            squad: sq,
            target: at(TRENCH_X, LINE_Y),
        }],
    );
    let start = w.tick();
    let edge = Fx::from_int(TRENCH_X + 2);
    let mut reached = 0u64;
    while w.tick() < start + ASSAULT_S * hz {
        w.step(&[]);
        if reached == 0
            && atts
                .iter()
                .any(|&m| super::s10a::alive(&w, m) && pos(&w, m).x <= edge)
        {
            reached = w.tick() - start;
        }
    }
    Ok([
        outs(&w, &defs) as u64,
        defs.len() as u64,
        outs(&w, &atts) as u64,
        atts.len() as u64,
        reached,
        w.game().unwrap().players()[att as usize].s14.flame_fired as u64,
    ])
}

fn trench_side(runs: &[[u64; 6]], hz: u64) -> TrenchSide {
    let sum = |f: usize| runs.iter().map(|r| r[f]).sum::<u64>();
    let got: Vec<u64> = runs.iter().map(|r| r[4]).filter(|&t| t > 0).collect();
    TrenchSide {
        runs: Some(runs.len() as f64),
        defenders_out_pct: pct(sum(0) as u32, sum(1) as u32),
        attackers_lost_pct: pct(sum(2) as u32, sum(3) as u32),
        to_trench_s: (!got.is_empty()).then(|| {
            let s = got.iter().sum::<u64>() as f64 / got.len() as f64 / hz as f64;
            (s * 10.0).round() / 10.0
        }),
        reached: Some(got.len() as f64),
        flame_fired: Some(sum(5) as f64),
    }
}

/// The `trench` row (R3, module docs).
pub fn trench_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<TrenchRow> {
    let all = par_seeds(seeds, |seed| {
        let mut v = Vec::new();
        for dug in [0u8, 1] {
            v.push([
                trench_once(args, seed, dug, true)?,
                trench_once(args, seed, dug, false)?,
            ]);
        }
        Ok(v)
    })?;
    let runs: Vec<[[u64; 6]; 2]> = all.into_iter().flatten().collect();
    let hz = flame_world(args, 1, false, |_| {})?
        .match_setup()
        .unwrap()
        .tick_hz as u64;
    let side = |k: usize| trench_side(&runs.iter().map(|r| r[k]).collect::<Vec<_>>(), hz);
    Ok(TrenchRow {
        flame: side(0),
        rifle: side(1),
    })
}

/// The R4 L trench: south end, bend, east end.
fn clear_line() -> [FxVec2; 3] {
    [
        at(TRENCH_X, LINE_Y - 20),
        at(TRENCH_X, LINE_Y + 20),
        at(TRENCH_X + 40, LINE_Y + 20),
    ]
}

/// One R4 run (module docs): [steps, clears done, defenders out,
/// defenders, attackers lost, attackers].
fn clear_once(args: &FortifyArgs, seed: u64, own: u8, clear: bool) -> Result<[u32; 6]> {
    let att = 1 - own;
    let mut w = flame_world(args, seed, true, |s| s.rules.clear.enabled = 1)?;
    let line = clear_line();
    if !w.dig_trench_now(&line, -1) {
        bail!("seed {seed}: the L trench was not dug");
    }
    // Four of the owner's line men 6 m apart along the first leg, the
    // rest 300 m off; all held.
    let d = squad(&w, own, "line")?;
    let mut held = Vec::new();
    let mut defs = Vec::new();
    for (k, m) in members(&w, d).into_iter().enumerate() {
        let p = if k < 4 {
            defs.push(m);
            at(TRENCH_X, LINE_Y - 14 + 6 * k as i32)
        } else {
            at(TRENCH_X + 300 + k as i32, LINE_Y + 300)
        };
        w.place_soldier(m, p);
        held.push(m);
    }
    let from = at(TRENCH_X - 30, LINE_Y - 40);
    let (sq, atts, _) = assault(&mut w, att, from, EAST)?;
    w.place_squad(sq, from, EAST);
    w.step(&[]);
    let kind = if clear {
        CommandKind::ClearTrench {
            squad: sq,
            at: line[0],
        }
    } else {
        CommandKind::AttackMove {
            squad: sq,
            target: line[0],
        }
    };
    step_with(&mut w, att, vec![kind]);
    hold(&mut w, &held, CLEAR_S);
    let s14 = &w.game().unwrap().players()[att as usize].s14;
    Ok([
        s14.clear_steps,
        s14.clears_done,
        outs(&w, &defs),
        defs.len() as u32,
        outs(&w, &atts),
        atts.len() as u32,
    ])
}

fn clear_side(runs: &[[u32; 6]]) -> ClearSide {
    let sum = |f: usize| runs.iter().map(|r| r[f]).sum::<u32>();
    ClearSide {
        runs: Some(runs.len() as f64),
        steps: Some(sum(0) as f64),
        done: Some(sum(1) as f64),
        defenders_out_pct: pct(sum(2), sum(3)),
        attackers_lost_pct: pct(sum(4), sum(5)),
    }
}

/// The `clear` row (R4, module docs).
pub fn clear_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<ClearRow> {
    let all = par_seeds(seeds, |seed| {
        let mut v = Vec::new();
        for own in [0u8, 1] {
            v.push([
                clear_once(args, seed, own, true)?,
                clear_once(args, seed, own, false)?,
            ]);
        }
        Ok(v)
    })?;
    let runs: Vec<[[u32; 6]; 2]> = all.into_iter().flatten().collect();
    let side = |k: usize| clear_side(&runs.iter().map(|r| r[k]).collect::<Vec<_>>());
    Ok(ClearRow {
        clear: side(0),
        attack_move: side(1),
    })
}

/// One R6 run (module docs): [line men out, line men, stepped out,
/// particles fired, hits in the trench, the flame man out].
fn under_fire_once(args: &FortifyArgs, seed: u64, dug: u8) -> Result<[u32; 6]> {
    let att = 1 - dug;
    let mut w = flame_world(args, seed, true, |_| {})?;
    let d = squad(&w, dug, "line")?;
    dig(&mut w);
    w.place_squad(d, at(TRENCH_X, LINE_Y), EAST);
    let defs = members(&w, d);
    let trench = vec![at(TRENCH_X, LINE_Y - 15), at(TRENCH_X, LINE_Y + 15)];
    let mg = call_in(&mut w, att, "mg")?;
    w.place_squad(mg, at(TRENCH_X + SUPPRESS_START_M, LINE_Y - 20), WEST);
    let gunners = members(&w, mg);
    step_with(
        &mut w,
        att,
        vec![CommandKind::SuppressUnits {
            units: gunners,
            line: trench.clone(),
        }],
    );
    let hz = w.match_setup().unwrap().tick_hz as u64;
    for _ in 0..SUPPRESS_LEAD_S * hz {
        w.step(&[]);
    }
    let park = at(TRENCH_X + 300, LINE_Y + 300);
    let (_, all, man) = assault(&mut w, att, park, WEST)?;
    let Some(man) = man else {
        bail!("seed {seed}: the assault card has no flame man");
    };
    let mut held = Vec::new();
    for (k, &m) in all.iter().enumerate() {
        let p = if m == man {
            at(TRENCH_X + UNDER_FIRE_M, LINE_Y)
        } else {
            at(TRENCH_X + 300 + k as i32, LINE_Y + 300)
        };
        w.place_soldier(m, p);
        held.push(m);
    }
    step_with(
        &mut w,
        att,
        vec![CommandKind::SuppressUnits {
            units: vec![man],
            line: trench,
        }],
    );
    hold(&mut w, &held, SLIT_S);
    let g = w.game().unwrap();
    let (a, o) = (
        &g.players()[att as usize].s14,
        &g.players()[dug as usize].s14,
    );
    Ok([
        outs(&w, &defs),
        defs.len() as u32,
        o.stepped_out,
        a.flame_fired,
        a.flame_in_trench_hits,
        out(&w, man) as u32,
    ])
}

/// The `under_fire` row (R6, module docs).
pub fn under_fire_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<UnderFireRow> {
    let all = par_seeds(seeds, |seed| {
        let mut v = Vec::new();
        for dug in [0u8, 1] {
            v.push(under_fire_once(args, seed, dug)?);
        }
        Ok(v)
    })?;
    let runs: Vec<[u32; 6]> = all.into_iter().flatten().collect();
    let sum = |f: usize| runs.iter().map(|r| r[f]).sum::<u32>();
    Ok(UnderFireRow {
        runs: Some(runs.len() as f64),
        men: Some(sum(1) as f64),
        out_pct: pct(sum(0), sum(1)),
        stepped_out: Some(sum(2) as f64),
        flame_fired: Some(sum(3) as f64),
        flame_in_trench_hits: Some(sum(4) as f64),
        flamer_out: Some(sum(5) as f64),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R1-R4, R6 (Builds A, B): each row runs on one seed with the mod's
    /// flame men; the pool row's invariants hold (saturated, particles
    /// fired, no drop or slot differs, never over the cap); the slit row
    /// counts its men and its flame man fires (report only); the clear
    /// row's clears step and end, its AttackMove control takes no step;
    /// the trench and under-fire rows count their runs. (The R1 verdict
    /// is read off the fortify10 set, not one seed.)
    #[test]
    fn fortify_s14_rows_run() {
        let r = s14_rows(&FortifyArgs::default(), 1..2).unwrap();
        eprintln!("{}", serde_json::to_string(&r).unwrap());
        assert_eq!((r.slit.runs, r.slit.men), (Some(2.0), Some(10.0)));
        assert!(r.slit.flame_fired.unwrap() > 0.0, "{:?}", r.slit);
        assert!(!r.slit.hard);
        let p = &r.pool;
        assert!(p.dropped.unwrap() > 0.0, "the pool is saturated: {p:?}");
        assert!(p.flame_fired.unwrap() > 0.0, "control: flame fired");
        assert_eq!(p.drop_diff, Some(0.0), "{p:?}");
        assert_eq!(p.pool_diff_ticks, Some(0.0), "{p:?}");
        assert_eq!(p.over_cap, Some(0.0), "{p:?}");
        assert_eq!(p.pass, Some(true));
        let c = &r.clear;
        assert!(c.clear.steps.unwrap() >= 2.0, "{c:?}");
        assert_eq!(c.clear.done, Some(2.0), "{c:?}");
        assert_eq!(c.attack_move.steps, Some(0.0), "{c:?}");
        assert_eq!(
            (r.trench.flame.runs, r.trench.rifle.runs),
            (Some(2.0), Some(2.0))
        );
        assert_eq!(r.under_fire.runs, Some(2.0));
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
            let (mut w, _, held, _) = slit_scene(&args, 1, 0, Slit::Open).unwrap();
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
