//! S11a2 fortify-test rows (docs/slices/S11a2.md 4 M2, M3; Build B):
//! `s11a2.ff`, `s11a2.cb` (with `answer`) and `s11a2.ff_lift` (three-part
//! rows: value, exposure, control). Every row is driven by player commands
//! on a flat map; the start squads are moved out of the way first. Each
//! row sets its own switches (value and control), so it reads the same
//! whatever the rules file holds.
//!
//! - `ff` (T139): a mortar team fires 8 bombs Direct at a point 300 m
//!   off where a halted rifle squad stands, once owned by the firing
//!   player, once by the enemy; each faction's mortar in turn.
//!   `own_loss_pct`, `enemy_loss_pct` (men lost of the squad),
//!   `own_vs_enemy_pct`; exposure `rounds_fired` (bombs of the own runs).
//!   Control: `friendly_fire` 0 (the same two runs).
//! - `cb` (T140, T141): a leIG 18 (Germany) and a 105 mm M3 (USA) each
//!   fire 8 rounds of map fire; a rifle squad of the enemy stands 800 m from
//!   the gun and listens. `marked_pct` (runs marked by the third round's
//!   tick plus `cb_delay_s` and one second), `mean_error_pct` /
//!   `worst_error_pct` (the first marker's miss as % of its spot's
//!   `heard_mm`), `last_error_m` (the miss after the 8 rounds), exposure
//!   `rounds_fired`. `answer`: in the M3 runs a German leIG 18 beside the
//!   listener fires 8 rounds of map fire at the marker: `near_burst_pct`
//!   (bursts within `cb_near_m` of the M3), `crew_loss_pct` (the M3's crew
//!   lost), `rounds_fired`. Control: `counter_battery` 0 (no marker; the
//!   answer is aimed where the marker would be, the gun itself, and the
//!   sim refuses it: unknown ground).
//! - `ff_lift` (T142): see [`ff_lift_row`].
use anyhow::{Result, anyhow, bail};
use frontline_content::MatchSetup;
use frontline_sim::{
    Command, CommandKind, EntityKind, Fx, FxVec2, Mission, Order, PlayerId, SquadId, World,
};
use serde::Serialize;

use super::s10b::{fields, pct};
use super::{EAST, FortifyArgs, at, members, world_with};

const SOUTH: FxVec2 = FxVec2::new(Fx::ZERO, Fx::NEG_ONE);
/// Mortar teams, rifle squads and guns by player (Germany, USA).
const MORTARS: [&str; 2] = ["ww2:ger_mortar_team", "ww2:usa_mortar_team"];
const RIFLES: [&str; 2] = ["ww2:ger_grenadiergruppe", "ww2:usa_rifle_squad"];
const GUNS: [&str; 2] = ["ww2:ger_leig18_squad", "ww2:usa_105mm_m3_squad"];
/// `ff`: bombs, the mortar, the point (300 m east).
pub const FF_ROUNDS: u16 = 8;
const FF_MORTAR: (i32, i32) = (200, 500);
const FF_POINT: (i32, i32) = (500, 500);
/// `cb`: rounds, the gun, its map-fire point, the listener (800 m north
/// of the gun), the answering leIG beside the listener.
pub const CB_ROUNDS: u16 = 8;
const CB_GUN: (i32, i32) = (100, 150);
const CB_POINT: (i32, i32) = (600, 150);
const CB_EAR: (i32, i32) = (100, 950);
const CB_ANSWER: (i32, i32) = (140, 950);
/// The longest mission (s) before a run fails.
const MISSION_MAX_S: u64 = 600;

#[derive(Clone, Debug, Default, Serialize)]
pub struct FfControl {
    pub own_loss_pct: Option<f64>,
    pub enemy_loss_pct: Option<f64>,
    pub own_vs_enemy_pct: Option<f64>,
}

/// T139: own men under an own mission lose as enemy men do.
#[derive(Clone, Debug, Default, Serialize)]
pub struct FfRow {
    pub own_loss_pct: Option<f64>,
    pub enemy_loss_pct: Option<f64>,
    pub own_vs_enemy_pct: Option<f64>,
    /// Exposure: bombs that burst in the own runs.
    pub rounds_fired: Option<f64>,
    pub control: FfControl,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct CbAnswer {
    pub near_burst_pct: Option<f64>,
    pub crew_loss_pct: Option<f64>,
    /// Answering rounds that burst.
    pub rounds_fired: Option<f64>,
    /// Report: answering orders the sim refused.
    pub refused: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct CbControl {
    pub marked_pct: Option<f64>,
    pub mean_error_pct: Option<f64>,
    pub worst_error_pct: Option<f64>,
    pub last_error_m: Option<f64>,
    pub answer: CbAnswer,
}

/// T140, T141: a gun found by sound, and the answer.
#[derive(Clone, Debug, Default, Serialize)]
pub struct CbRow {
    pub marked_pct: Option<f64>,
    pub mean_error_pct: Option<f64>,
    pub worst_error_pct: Option<f64>,
    pub last_error_m: Option<f64>,
    /// Exposure: gun mission rounds fired (`S11Counters::gun_rounds`).
    pub rounds_fired: Option<f64>,
    /// Report, per gun (leIG, M3): runs marked in time (%).
    pub leig_marked_pct: Option<f64>,
    pub m3_marked_pct: Option<f64>,
    pub answer: CbAnswer,
    pub control: CbControl,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct FfLiftControl {
    pub clean_runs_pct: Option<f64>,
    pub lifted_pct: Option<f64>,
    pub rounds_fired: Option<f64>,
}

/// T142: the AI lifts its fire before own men walk into it.
#[derive(Clone, Debug, Default, Serialize)]
pub struct FfLiftRow {
    pub clean_runs_pct: Option<f64>,
    pub lifted_pct: Option<f64>,
    pub rounds_fired: Option<f64>,
    /// The scene used: the squad's start beyond the point (m), bombs.
    pub start_m: Option<f64>,
    pub bombs: Option<f64>,
    pub control: FfLiftControl,
}

/// The S11a2 rows (`fortify10.s11a2`).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S11a2Report {
    pub ff: FfRow,
    pub cb: CbRow,
    pub ff_lift: FfLiftRow,
}

/// Summary lines, one per row.
pub fn summary_text(r: &S11a2Report) -> String {
    let v = serde_json::to_value(r).unwrap_or_default();
    ["ff", "cb", "ff_lift"]
        .iter()
        .map(|k| format!("  S11a2 {k}: {}\n", fields(&v[*k])))
        .collect()
}

fn metres(f: Fx) -> f64 {
    f64::from(f.raw()) / 65536.0
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn mean(v: &[f64]) -> Option<f64> {
    (!v.is_empty()).then(|| round1(v.iter().sum::<f64>() / v.len() as f64))
}

fn ratio(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(a), Some(b)) if b > 0.0 => Some(round1(a * 100.0 / b)),
        _ => None,
    }
}

fn mission(w: &World, sq: SquadId) -> Option<Mission> {
    match &w.game()?.squad(sq)?.order {
        Order::FireMission { m, .. } => Some(*m),
        _ => None,
    }
}

/// A flat map with no ticket drain (minutes of firing end no match).
fn flat(s: &mut MatchSetup) {
    s.map.terrain.octaves.clear();
    s.map.terrain.bands.clear();
    s.map.woods.clear();
    s.rules.ticket_drain_per_min_per_point = 0;
}

/// Sets the two sim switches of the slice.
fn switches(s: &mut MatchSetup, ff: u32, cb: u32) {
    let fm = s
        .rules
        .fire_mission
        .as_mut()
        .expect("ww2 has [fire_mission]");
    fm.friendly_fire = ff;
    fm.counter_battery = cb;
}

fn spawn(w: &mut World, p: u8, card: &str, xy: FxVec2, facing: FxVec2) -> Result<SquadId> {
    let t = w
        .match_setup()
        .unwrap()
        .squad_type_index(card)
        .ok_or_else(|| anyhow!("no {card} card"))?;
    w.spawn_squad_at(PlayerId(p), t, xy, facing)
        .ok_or_else(|| anyhow!("could not place {card}"))
}

/// Moves every start squad of player `p` to `xy` (out of the scene).
fn park(w: &mut World, p: u8, xy: FxVec2) {
    let ids: Vec<SquadId> = w
        .game()
        .unwrap()
        .squads()
        .iter()
        .filter(|(_, q)| q.owner == PlayerId(p))
        .map(|(id, _)| *id)
        .collect();
    for id in ids {
        w.place_squad(id, xy, EAST);
    }
}

/// Orders the mission; the refusal of this tick, if any.
fn order(w: &mut World, p: u8, sq: SquadId, target: FxVec2, rounds: u16) -> Option<String> {
    let seq = w.tick() as u32;
    w.step(&[Command {
        player: PlayerId(p),
        seq,
        kind: CommandKind::FireMission {
            squad: sq,
            target,
            rounds,
        },
    }]);
    w.snapshot()
        .fire_mission_refusals
        .iter()
        .find(|e| e.squad == sq)
        .map(|e| frontline_sim::FireMissionRefusal::NAMES[e.refusal as usize].to_string())
}

/// Runs the mission of `sq` out (and 30 s for the last bursts); the
/// bursts of mission `id`.
fn run_out(
    w: &mut World,
    sq: SquadId,
    id: u32,
    what: &str,
) -> Result<Vec<frontline_sim::fire_mission::BurstRecord>> {
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let t0 = w.tick();
    while mission(w, sq).is_some() {
        if w.tick() > t0 + MISSION_MAX_S * hz {
            bail!("{what}: the mission never ended");
        }
        w.step(&[]);
    }
    for _ in 0..30 * hz {
        w.step(&[]);
    }
    Ok(w.take_burst_log()
        .into_iter()
        .filter(|b| b.mission == id)
        .collect())
}

/// Runs `f` for each seed on its own thread (each world is separate and
/// deterministic) and returns the results in seed order.
pub(super) fn par_seeds<T: Send>(
    seeds: std::ops::Range<u64>,
    f: impl Fn(u64) -> Result<T> + Sync,
) -> Result<Vec<T>> {
    let f = &f;
    std::thread::scope(|sc| {
        let hs: Vec<_> = seeds.map(|seed| sc.spawn(move || f(seed))).collect();
        hs.into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err(anyhow!("a row thread panicked")))
            })
            .collect()
    })
}

/// Living soldiers (not the gun) of squad `q`.
fn soldiers(w: &World, q: SquadId) -> u32 {
    members(w, q)
        .iter()
        .filter(|m| {
            w.entities().kind()[m.index()] == EntityKind::Soldier
                && w.entities().combat()[m.index()].hp > 0
        })
        .count() as u32
}

/// Living soldiers of player `p` within 20 m of `xy` (a served gun's
/// crew is a squad of its own standing at the gun's slots).
fn crew_near(w: &World, p: u8, xy: FxVec2) -> u32 {
    let g = w.game().unwrap();
    g.squads()
        .values()
        .filter(|q| q.owner == PlayerId(p))
        .flat_map(|q| q.members.iter())
        .filter(|m| {
            w.entities().kind()[m.index()] == EntityKind::Soldier
                && w.entities().combat()[m.index()].hp > 0
                && (super::pos(w, **m) - xy).length() <= Fx::from_int(20)
        })
        .count() as u32
}

/// One `ff` run: the mortar of player `p`, the squad owned by `p` (`own`)
/// or the enemy. (men lost, men, bombs burst).
fn ff_once(args: &FortifyArgs, seed: u64, p: u8, own: bool, ff: u32) -> Result<(u32, u32, u32)> {
    let mut w = world_with(args, seed, |s| {
        flat(s);
        switches(s, ff, 0);
    })?;
    let what = format!("s11a2.ff seed {seed} player {p} own {own} ff {ff}");
    let e = 1 - p;
    park(&mut w, p, at(60, 950));
    park(&mut w, e, at(1000, 60));
    let victim = if own { p } else { e };
    let point = at(FF_POINT.0, FF_POINT.1);
    let mortar = spawn(
        &mut w,
        p,
        MORTARS[p as usize],
        at(FF_MORTAR.0, FF_MORTAR.1),
        EAST,
    )?;
    let sq = spawn(&mut w, victim, RIFLES[victim as usize], point, EAST)?;
    for _ in 0..2 * w.match_setup().unwrap().tick_hz {
        w.step(&[]);
    }
    let men = soldiers(&w, sq);
    w.enable_burst_log();
    if let Some(r) = order(&mut w, p, mortar, point, FF_ROUNDS) {
        bail!("{what}: refused {r}");
    }
    let id = mission(&w, mortar)
        .ok_or_else(|| anyhow!("{what}: no mission"))?
        .id;
    let bursts = run_out(&mut w, mortar, id, &what)?.len() as u32;
    Ok((men - soldiers(&w, sq), men, bursts))
}

/// The `ff` row (module docs).
pub fn ff_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<FfRow> {
    // [ff 1, ff 0][own, enemy] = (lost, men); bombs of the own runs (ff 1).
    let all = par_seeds(seeds, |seed| {
        let mut v = Vec::new();
        for p in [0u8, 1] {
            let mut r = [[(0u32, 0u32, 0u32); 2]; 2];
            for (i, ff) in [1u32, 0].into_iter().enumerate() {
                for (j, own) in [true, false].into_iter().enumerate() {
                    r[i][j] = ff_once(args, seed, p, own, ff)?;
                }
            }
            v.push(r);
        }
        Ok(v)
    })?;
    let mut t = [[(0u32, 0u32); 2]; 2];
    let mut bombs = 0u32;
    let mut runs = 0u32;
    for r in all.into_iter().flatten() {
        runs += 1;
        bombs += r[0][0].2;
        for i in 0..2 {
            for j in 0..2 {
                t[i][j].0 += r[i][j].0;
                t[i][j].1 += r[i][j].1;
            }
        }
    }
    let loss = |i: usize, j: usize| pct(t[i][j].0, t[i][j].1);
    Ok(FfRow {
        own_loss_pct: loss(0, 0),
        enemy_loss_pct: loss(0, 1),
        own_vs_enemy_pct: ratio(loss(0, 0), loss(0, 1)),
        rounds_fired: (runs > 0).then_some(bombs as f64),
        control: FfControl {
            own_loss_pct: loss(1, 0),
            enemy_loss_pct: loss(1, 1),
            own_vs_enemy_pct: ratio(loss(1, 0), loss(1, 1)),
        },
    })
}

/// What one `cb` run saw.
#[derive(Clone, Copy, Debug, Default)]
struct CbRun {
    marked_in_time: bool,
    /// The first marker's miss as % of its `heard_mm`.
    first_pct: Option<f64>,
    /// The marker's miss after the mission (m).
    last_m: Option<f64>,
    rounds: u32,
    /// The answer (M3 runs): refused, bursts, bursts near, crew, lost.
    answered: bool,
    refused: bool,
    a_bursts: u32,
    a_near: u32,
    crew: u32,
    crew_lost: u32,
}

/// One `cb` run: gun of player `p`, `cb` the switch. With `answer` (the
/// M3 run) the listener's leIG then fires at the marker (with no marker,
/// at the gun: the control).
fn cb_once(args: &FortifyArgs, seed: u64, p: u8, cb: u32, answer: bool) -> Result<CbRun> {
    let mut near_m = 20;
    let mut delay_s = 15;
    let mut w = world_with(args, seed, |s| {
        flat(s);
        switches(s, 0, cb);
        let fm = s.rules.fire_mission.as_ref().unwrap();
        near_m = fm.cb_near_m as i32;
        delay_s = fm.cb_delay_s as u64;
    })?;
    let what = format!("s11a2.cb seed {seed} player {p} cb {cb}");
    let e = 1 - p;
    park(&mut w, p, at(20, 20));
    park(&mut w, e, at(60, 1010));
    let gun_xy = at(CB_GUN.0, CB_GUN.1);
    let point = at(CB_POINT.0, CB_POINT.1);
    let gun = spawn(&mut w, p, GUNS[p as usize], gun_xy, EAST)?;
    spawn(&mut w, e, RIFLES[e as usize], at(CB_EAR.0, CB_EAR.1), SOUTH)?;
    let hz = w.match_setup().unwrap().tick_hz as u64;
    for _ in 0..2 * hz {
        w.step(&[]);
    }
    if !w.mark_intel(PlayerId(p), point) {
        bail!("{what}: could not mark the point");
    }
    w.enable_burst_log();
    if let Some(r) = order(&mut w, p, gun, point, CB_ROUNDS) {
        bail!("{what}: refused {r}");
    }
    if mission(&w, gun).is_none() {
        bail!("{what}: no mission");
    }
    let gun_at = |w: &World| {
        members(w, gun)
            .iter()
            .find(|m| w.entities().kind()[m.index()] != EntityKind::Soldier)
            .map_or(gun_xy, |m| super::pos(w, *m))
    };
    let spot = |w: &World| {
        w.game().unwrap().players()[e as usize]
            .cb
            .iter()
            .find(|s| s.of == p && s.marked)
            .copied()
    };
    let mut run = CbRun::default();
    let (mut third, mut marked_at) = (None, None);
    let t0 = w.tick();
    let mut end = None;
    let gun_rounds = |w: &World| -> u32 {
        w.game().unwrap().players()[p as usize]
            .s11
            .gun_rounds
            .iter()
            .sum()
    };
    let fired0 = gun_rounds(&w);
    // The mission, then the delay and a second more for the last mark.
    while end.is_none_or(|t| w.tick() < t + (delay_s + 2) * hz) {
        if w.tick() > t0 + MISSION_MAX_S * hz {
            bail!("{what}: the mission never ended");
        }
        w.step(&[]);
        let m = mission(&w, gun);
        if third.is_none() && gun_rounds(&w) - fired0 >= 3 {
            third = Some(w.tick());
        }
        if end.is_none() && m.is_none() {
            end = Some(w.tick());
        }
        if marked_at.is_none()
            && let Some(s) = spot(&w)
        {
            marked_at = Some(w.tick());
            let miss = metres((s.at - gun_at(&w)).length());
            run.first_pct = Some(miss * 100_000.0 / f64::from(s.heard_mm.max(1)));
        }
    }
    // Rounds fired (the player's gun mission rounds; the last may still fly).
    w.take_burst_log();
    run.rounds = gun_rounds(&w) - fired0;
    run.marked_in_time = match (third, marked_at) {
        (Some(t), Some(m)) => m <= t + (delay_s + 1) * hz,
        _ => false,
    };
    let marker = spot(&w);
    run.last_m = marker.map(|s| metres((s.at - gun_at(&w)).length()));
    if answer {
        run.answered = true;
        let target = marker.map_or_else(|| gun_at(&w), |s| s.at);
        let leig = spawn(
            &mut w,
            e,
            GUNS[e as usize],
            at(CB_ANSWER.0, CB_ANSWER.1),
            SOUTH,
        )?;
        for _ in 0..2 * hz {
            w.step(&[]);
        }
        let g = gun_at(&w);
        run.crew = crew_near(&w, p, g);
        w.enable_burst_log();
        match order(&mut w, e, leig, target, CB_ROUNDS) {
            Some(_) => run.refused = true,
            None => {
                let id = mission(&w, leig)
                    .ok_or_else(|| anyhow!("{what}: no answer mission"))?
                    .id;
                let b = run_out(&mut w, leig, id, &what)?;
                run.a_bursts = b.len() as u32;
                run.a_near = b
                    .iter()
                    .filter(|b| (b.pos.xy() - g).length() <= Fx::from_int(near_m))
                    .count() as u32;
            }
        }
        run.crew_lost = run.crew - crew_near(&w, p, g).min(run.crew);
    }
    Ok(run)
}

/// The `cb` row (module docs).
pub fn cb_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<CbRow> {
    let all = par_seeds(seeds, |seed| {
        let mut v = Vec::new();
        for cb in [1u32, 0] {
            for p in [0u8, 1] {
                v.push((cb, p, cb_once(args, seed, p, cb, p == 1)?));
            }
        }
        Ok(v)
    })?;
    let side = |cb: u32| -> (CbRow, CbAnswer) {
        let runs: Vec<(u8, CbRun)> = all
            .iter()
            .flatten()
            .filter(|(c, _, _)| *c == cb)
            .map(|(_, p, r)| (*p, *r))
            .collect();
        let n = runs.len() as u32;
        let marked = runs.iter().filter(|(_, r)| r.marked_in_time).count() as u32;
        let by = |p: u8| {
            let v: Vec<&CbRun> = runs
                .iter()
                .filter(|(q, _)| *q == p)
                .map(|(_, r)| r)
                .collect();
            pct(
                v.iter().filter(|r| r.marked_in_time).count() as u32,
                v.len() as u32,
            )
        };
        let first: Vec<f64> = runs.iter().filter_map(|(_, r)| r.first_pct).collect();
        let last: Vec<f64> = runs.iter().filter_map(|(_, r)| r.last_m).collect();
        let a: Vec<&CbRun> = runs.iter().map(|(_, r)| r).filter(|r| r.answered).collect();
        let (bursts, near, crew, lost, refused) = a.iter().fold((0, 0, 0, 0, 0), |t, r| {
            (
                t.0 + r.a_bursts,
                t.1 + r.a_near,
                t.2 + if r.refused { 0 } else { r.crew },
                t.3 + r.crew_lost,
                t.4 + r.refused as u32,
            )
        });
        let answer = CbAnswer {
            near_burst_pct: pct(near, bursts),
            crew_loss_pct: pct(lost, crew),
            rounds_fired: (!a.is_empty()).then_some(bursts as f64),
            refused: (!a.is_empty()).then_some(refused as f64),
        };
        (
            CbRow {
                marked_pct: pct(marked, n),
                mean_error_pct: mean(&first),
                worst_error_pct: first.iter().copied().reduce(f64::max).map(round1),
                last_error_m: mean(&last),
                rounds_fired: (n > 0)
                    .then_some(runs.iter().map(|(_, r)| r.rounds).sum::<u32>() as f64),
                leig_marked_pct: by(0),
                m3_marked_pct: by(1),
                answer: CbAnswer::default(),
                control: CbControl::default(),
            },
            answer,
        )
    };
    let (mut row, answer) = side(1);
    let (c, c_answer) = side(0);
    row.answer = answer;
    row.control = CbControl {
        marked_pct: c.marked_pct,
        mean_error_pct: c.mean_error_pct,
        worst_error_pct: c.worst_error_pct,
        last_error_m: c.last_error_m,
        answer: c_answer,
    };
    Ok(row)
}

/// `ff_lift`: bombs, the squad's start beyond the point (m).
pub const LIFT_ROUNDS: u16 = 12;
pub const LIFT_START_M: i32 = 130;

/// One `ff_lift` run of player `p` (`ff_safe` the switch): (no own man
/// hurt by own fire, the mission lifted, bombs fired).
fn ff_lift_once(args: &FortifyArgs, seed: u64, p: u8, safe: u32) -> Result<(bool, bool, u32)> {
    let mut w = world_with(args, seed, |s| {
        flat(s);
        switches(s, 1, 0);
        s.rules.ai.arty.ff_safe = safe;
    })?;
    let what = format!("s11a2.ff_lift seed {seed} player {p} ff_safe {safe}");
    let e = 1 - p;
    park(&mut w, p, at(60, 950));
    park(&mut w, e, at(1000, 60));
    let point = at(FF_POINT.0, FF_POINT.1);
    let mortar = spawn(
        &mut w,
        p,
        MORTARS[p as usize],
        at(FF_MORTAR.0, FF_MORTAR.1),
        EAST,
    )?;
    let sq = spawn(
        &mut w,
        p,
        RIFLES[p as usize],
        at(FF_POINT.0 + LIFT_START_M, FF_POINT.1),
        EAST,
    )?;
    let hz = w.match_setup().unwrap().tick_hz as u64;
    for _ in 0..2 * hz {
        w.step(&[]);
    }
    // Fix B-5: the squad starts outside `lift_m`, so the rule is not met
    // on the first plan (it must see the squad walk in).
    let lift = w.match_setup().unwrap().rules.ai.arty.lift_m as i32;
    if LIFT_START_M <= lift {
        bail!("{what}: the squad starts inside lift_m {lift}");
    }
    let bombs = |w: &World| -> u32 {
        w.game().unwrap().players()[p as usize]
            .fm
            .bombs
            .iter()
            .sum()
    };
    let hurt = |w: &World| {
        let c = &w.game().unwrap().players()[p as usize].s11a2;
        c.ff_bursts
    };
    let (b0, h0) = (bombs(&w), hurt(&w));
    if let Some(r) = order(&mut w, p, mortar, point, LIFT_ROUNDS) {
        bail!("{what}: refused {r}");
    }
    if mission(&w, mortar).is_none() {
        bail!("{what}: no mission");
    }
    let mut ai = frontline_sim::ai::AiState::new(w.tick());
    let t0 = w.tick();
    let mut sent = false;
    let mut end = None;
    while end.is_none_or(|t| w.tick() < t + 40 * hz) {
        if w.tick() > t0 + MISSION_MAX_S * hz {
            bail!("{what}: the mission never ended");
        }
        // The squad walks onto the point as the first bomb leaves.
        if !sent && bombs(&w) > b0 {
            sent = true;
            let seq = w.tick() as u32;
            w.step(&[Command {
                player: PlayerId(p),
                seq,
                kind: CommandKind::Move {
                    squad: sq,
                    target: point,
                },
            }]);
            continue;
        }
        w.drive_ff_lift(&mut ai, PlayerId(p));
        if end.is_none() && mission(&w, mortar).is_none() {
            end = Some(w.tick());
        }
    }
    Ok((hurt(&w) == h0, ai.counters.ff_lifts > 0, bombs(&w) - b0))
}

/// The `ff_lift` row (spec 4 M3): a mortar team's 12-bomb Direct mission
/// on a point 300 m off; an own rifle squad 130 m beyond the point (outside
/// `lift_m` 100; fix B-5) is sent
/// onto it as the first bomb leaves; the owner's S3 lift step is driven
/// each plan (`World::drive_ff_lift`); each faction's mortar in turn.
/// `clean_runs_pct` (no own man hurt by own fire), `lifted_pct`,
/// `rounds_fired` (bombs, all runs). Control: `ff_safe` 0 (the step lifts
/// nothing, the mission runs out).
pub fn ff_lift_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<FfLiftRow> {
    let all = par_seeds(seeds, |seed| {
        let mut v = Vec::new();
        for safe in [1u32, 0] {
            for p in [0u8, 1] {
                v.push((safe, ff_lift_once(args, seed, p, safe)?));
            }
        }
        Ok(v)
    })?;
    let side = |safe: u32| {
        let runs: Vec<(bool, bool, u32)> = all
            .iter()
            .flatten()
            .filter(|(s, _)| *s == safe)
            .map(|(_, r)| *r)
            .collect();
        let n = runs.len() as u32;
        (
            pct(runs.iter().filter(|r| r.0).count() as u32, n),
            pct(runs.iter().filter(|r| r.1).count() as u32, n),
            (n > 0).then_some(runs.iter().map(|r| r.2).sum::<u32>() as f64),
        )
    };
    let (clean, lifted, rounds) = side(1);
    let (c_clean, c_lifted, c_rounds) = side(0);
    Ok(FfLiftRow {
        clean_runs_pct: clean,
        lifted_pct: lifted,
        rounds_fired: rounds,
        start_m: Some(LIFT_START_M as f64),
        bombs: Some(LIFT_ROUNDS as f64),
        control: FfLiftControl {
            clean_runs_pct: c_clean,
            lifted_pct: c_lifted,
            rounds_fired: c_rounds,
        },
    })
}

/// Every S11a2 row.
pub fn s11a2_rows(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<S11a2Report> {
    Ok(S11a2Report {
        ff: ff_row(args, seeds.clone())?,
        cb: cb_row(args, seeds.clone())?,
        ff_lift: ff_lift_row(args, seeds)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// M2 (Build B): each row runs on one seed and its control differs from
    /// its value (the switch makes a difference).
    #[test]
    fn fortify_s11a2_rows_run() {
        let a = FortifyArgs::default();
        let r = s11a2_rows(&a, 1..2).unwrap();
        eprintln!("{}", serde_json::to_string(&r).unwrap());
        let f = &r.ff;
        assert!(f.rounds_fired.unwrap() > 0.0);
        assert_ne!(f.own_loss_pct, f.control.own_loss_pct, "ff control");
        assert_eq!(
            f.control.own_loss_pct,
            Some(0.0),
            "no own loss with the switch 0"
        );
        let c = &r.cb;
        assert!(c.rounds_fired.unwrap() > 0.0);
        assert_ne!(c.marked_pct, c.control.marked_pct, "cb control");
        assert_eq!(c.control.marked_pct, Some(0.0));
        assert!(c.answer.rounds_fired.unwrap() > 0.0, "the answer fired");
        assert_eq!(
            c.control.answer.refused,
            Some(1.0),
            "control answer refused"
        );
        // Fix B-5: the lift row measures (the squad starts outside
        // `lift_m`) and its control bites.
        let l = &r.ff_lift;
        assert_eq!(l.start_m, Some(LIFT_START_M as f64));
        assert_eq!(l.lifted_pct, Some(100.0), "lifted");
        assert_eq!(l.clean_runs_pct, Some(100.0), "clean");
        assert_eq!(l.control.clean_runs_pct, Some(0.0), "control: hurt");
    }

    /// One-off: the S11a2 rows on seeds 1-10 (as `fortify-test --seeds
    /// 10`), with their wall time; run by hand in release.
    #[test]
    #[ignore]
    fn s11a2_rows_one_off() {
        let t = std::time::Instant::now();
        let r = s11a2_rows(&FortifyArgs::default(), 1..11).unwrap();
        eprintln!("{}", serde_json::to_string(&r).unwrap());
        eprintln!("s11a2 rows wall s: {:.1}", t.elapsed().as_secs_f64());
    }
}
