//! S11a1 fortify-test rows (docs/slices/S11a1.md 4 C5, 8; Build B item 5):
//! `s11a1.gun_fm`, `s11a1.mask`, `s11a1.barrage` (three-part rows: value,
//! exposure, control) and the `breach_prep` keys Build C fills (null here,
//! so the targets print SKIP). Every row is driven by player commands
//! (`CommandKind::FireMission` on the gun's own squad); no AI code runs.
//! A gun card arrives emplaced and crewed at its spawn point.
//!
//! - `gun_fm` (T128): on flat ground each faction's gun (Germany the leIG
//!   18, the USA the 105 mm M3) fires 8 rounds at a point 600 m east. The
//!   observed run: the faction's observer team stands 300 m short of the
//!   point (beyond `observer_link_m` of the gun: linked by radio only).
//!   The map run: no observer, observation ranges 0, the point marked in
//!   the knowledge map. `observed_miss_m` / `map_miss_m`: mean distance of
//!   the mission's bursts from the point; `ratio_pct` = map / observed.
//!   Exposure `rounds_fired` (observed runs). Control: the observer's
//!   radio man killed before the order (the mission is map fire, about 100).
//! - `mask` (T129): a 25 m crest band across x = 500 (the G2 crest), the gun
//!   at (50, 500), the point at (580, 500), marked; one round. `m3_refused_pct`
//!   (the M3 refused `masked`), `leig_fired_pct` (the leIG's order accepted
//!   and its round burst within 10 m of its aim point, the point plus the
//!   mission's aim error: it flew its solution over the crest). Exposure
//!   `orders`. Control: flat ground.
//! - `barrage` (T130): 12 M1 HE from the M3 500 m east, observed by the FO
//!   Party 350 m from the target, on a German rifle squad in the open and
//!   (another run) in a dug trench: `open_loss_pct`, `trench_loss_pct`
//!   (men lost of the squad), `trench_vs_open_pct`, `trench_suppressed_pct`
//!   (the trench squad's living men ducked or pinned one tick after the last burst).
//!   Control: the trench dug but the squad standing 8 m in front of it, in
//!   the open (with a fixed-seed sim, "both squads in the open" at one spot
//!   would be the open run itself).
use anyhow::{Result, anyhow, bail};
use frontline_content::MatchSetup;
use frontline_content::setup::{BandSetup, FixedVec2, member_trait};
use frontline_sim::{Command, CommandKind, Fx, FxVec2, Mission, Order, PlayerId, SquadId, World};
use serde::Serialize;

use super::s10b::{fields, pct};
use super::{EAST, FortifyArgs, WEST, at, members, world_with};

/// The gun cards and observer cards, by player (Germany, USA).
const GUNS: [&str; 2] = ["ww2:ger_leig18_squad", "ww2:usa_105mm_m3_squad"];
const FOS: [&str; 2] = ["ww2:ger_vb_trupp", "ww2:usa_fo_party"];
/// `gun_fm`: rounds a mission, the gun, the point, the observer.
pub const FM_ROUNDS: u16 = 8;
const FM_GUN: (i32, i32) = (150, 500);
const FM_RANGE_M: i32 = 600;
const FM_FO_SHORT_M: i32 = 300;
/// `barrage`: rounds, the M3's distance from the target squad, the FO's.
pub const BARRAGE_ROUNDS: u16 = 12;
const BARRAGE_RANGE_M: i32 = 500;
const BARRAGE_FO_M: i32 = 350;
const BARRAGE_TARGET: (i32, i32) = (400, 500);
/// `mask`: the leIG's round lands within this many metres of its aim point.
const MASK_LAND_M: i32 = 10;
/// The longest mission (s) before a run fails.
const MISSION_MAX_S: u64 = 600;

#[derive(Clone, Debug, Default, Serialize)]
pub struct GunFmControl {
    pub observed_miss_m: Option<f64>,
    pub ratio_pct: Option<f64>,
}

/// T128: map fire against observed fire.
#[derive(Clone, Debug, Default, Serialize)]
pub struct GunFmRow {
    pub observed_miss_m: Option<f64>,
    pub map_miss_m: Option<f64>,
    pub ratio_pct: Option<f64>,
    /// Exposure: mission rounds that burst in the observed runs.
    pub rounds_fired: Option<f64>,
    /// Report: observed runs whose mission started Observed (%).
    pub observed_mode_pct: Option<f64>,
    /// Report, per gun: (observed, map) mean miss, m.
    pub leig_miss_m: [Option<f64>; 2],
    pub m3_miss_m: [Option<f64>; 2],
    pub control: GunFmControl,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct MaskControl {
    pub m3_refused_pct: Option<f64>,
    pub leig_fired_pct: Option<f64>,
}

/// T129: the crest masks the low-angle M3, not the high-angle leIG.
#[derive(Clone, Debug, Default, Serialize)]
pub struct MaskRow {
    pub m3_refused_pct: Option<f64>,
    pub leig_fired_pct: Option<f64>,
    /// Exposure: orders given (both guns).
    pub orders: Option<f64>,
    pub control: MaskControl,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct BarrageControl {
    pub trench_vs_open_pct: Option<f64>,
    pub trench_suppressed_pct: Option<f64>,
}

/// T130: 12 M1 HE on a trench and in the open.
#[derive(Clone, Debug, Default, Serialize)]
pub struct BarrageRow {
    pub open_loss_pct: Option<f64>,
    pub trench_loss_pct: Option<f64>,
    pub trench_vs_open_pct: Option<f64>,
    pub trench_suppressed_pct: Option<f64>,
    /// Report: mission rounds that burst (all runs).
    pub rounds_fired: Option<f64>,
    pub control: BarrageControl,
}

/// Build C's breach row keys (null until then).
#[derive(Clone, Debug, Default, Serialize)]
pub struct BreachPrepControl {
    pub gap_opened_pct: Option<f64>,
    pub placed_pct: Option<f64>,
    pub attacker_loss_pct: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct BreachPrepRow {
    pub gap_opened_pct: Option<f64>,
    pub placed_pct: Option<f64>,
    pub attacker_loss_pct: Option<f64>,
    pub prep_fired_pct: Option<f64>,
    pub control: BreachPrepControl,
}

/// The S11a1 rows (`fortify10.s11a1`).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S11a1Report {
    pub gun_fm: GunFmRow,
    pub mask: MaskRow,
    pub barrage: BarrageRow,
    pub breach_prep: BreachPrepRow,
    pub breach_prep_gun: BreachPrepRow,
}

/// Summary lines, one per row.
pub fn summary_text(r: &S11a1Report) -> String {
    let v = serde_json::to_value(r).unwrap_or_default();
    ["gun_fm", "mask", "barrage"]
        .iter()
        .map(|k| format!("  S11a1 {k}: {}\n", fields(&v[*k])))
        .collect()
}

fn metres(f: Fx) -> f64 {
    f64::from(f.raw()) / 65536.0
}

fn mean(v: &[f64]) -> Option<f64> {
    (!v.is_empty()).then(|| (v.iter().sum::<f64>() / v.len() as f64 * 10.0).round() / 10.0)
}

fn ratio(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(a), Some(b)) if b > 0.0 => Some((a * 1000.0 / b).round() / 10.0),
        _ => None,
    }
}

fn mission(w: &World, sq: SquadId) -> Option<Mission> {
    match &w.game()?.squad(sq)?.order {
        Order::FireMission { m, .. } => Some(*m),
        _ => None,
    }
}

fn flat(s: &mut MatchSetup) {
    s.map.terrain.octaves.clear();
    s.map.terrain.bands.clear();
    s.map.woods.clear();
    // Minutes of firing: no ticket drain ends the match.
    s.rules.ticket_drain_per_min_per_point = 0;
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

/// The living men of squad `q` whose slot carries trait `bit`.
fn with_trait(w: &World, q: SquadId, bit: u8) -> Vec<frontline_sim::SimId> {
    let g = w.game().unwrap();
    let Some(sq) = g.squad(q) else {
        return Vec::new();
    };
    let ty = &g.setup().squad_types[sq.type_idx as usize];
    sq.members
        .iter()
        .copied()
        .filter(|m| {
            ty.members
                .get(w.entities().kit()[m.index()].slot as usize)
                .is_some_and(|s| s.traits & bit != 0)
        })
        .collect()
}

/// One `gun_fm` run of player `p`: `kind` 0 observed, 1 map, 2 control
/// (the observer's radio man dead). (mean miss m, bursts, started observed).
fn gun_fm_once(args: &FortifyArgs, seed: u64, p: u8, kind: u8) -> Result<(f64, u32, bool)> {
    use frontline_sim::fire_mission::mode;
    let mut w = world_with(args, seed, |s| {
        flat(s);
        if kind == 1 {
            let fm = s
                .rules
                .fire_mission
                .as_mut()
                .expect("ww2 has [fire_mission]");
            fm.observe_leader_m = 0;
            fm.observe_observer_m = 0;
        }
    })?;
    let what = format!("s11a1.gun_fm seed {seed} {} kind {kind}", GUNS[p as usize]);
    let gun = at(FM_GUN.0, FM_GUN.1);
    let target = at(FM_GUN.0 + FM_RANGE_M, FM_GUN.1);
    let sq = spawn(&mut w, p, GUNS[p as usize], gun, EAST)?;
    if kind == 1 {
        if !w.mark_intel(PlayerId(p), target) {
            bail!("{what}: could not mark the target");
        }
    } else {
        let fo_at = at(FM_GUN.0 + FM_RANGE_M - FM_FO_SHORT_M, FM_GUN.1 + 20);
        let fo = spawn(&mut w, p, FOS[p as usize], fo_at, EAST)?;
        if kind == 2 {
            for m in with_trait(&w, fo, member_trait::RADIO) {
                w.kill_soldier(m);
            }
        }
    }
    for _ in 0..2 * w.match_setup().unwrap().tick_hz {
        w.step(&[]);
    }
    w.enable_burst_log();
    if let Some(r) = order(&mut w, p, sq, target, FM_ROUNDS) {
        bail!("{what}: refused {r}");
    }
    let m0 = mission(&w, sq).ok_or_else(|| anyhow!("{what}: no mission"))?;
    let bursts = run_out(&mut w, sq, m0.id, &what)?;
    let misses: Vec<f64> = bursts
        .iter()
        .map(|b| metres((b.pos.xy() - target).length()))
        .collect();
    let n = misses.len() as u32;
    Ok((mean(&misses).unwrap_or(0.0), n, m0.mode == mode::OBSERVED))
}

/// Runs `f` for each seed on its own thread (each world is separate and
/// deterministic, so the results do not depend on it) and returns the
/// results in seed order; the first error wins.
fn par_seeds<T: Send>(
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

/// The `gun_fm` row (module docs).
pub fn gun_fm_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<GunFmRow> {
    let (mut obs, mut map, mut ctl) = (Vec::new(), Vec::new(), Vec::new());
    let mut per = [[Vec::new(), Vec::new()], [Vec::new(), Vec::new()]];
    let (mut rounds, mut runs, mut started) = (0u32, 0u32, 0u32);
    let all = par_seeds(seeds, |seed| {
        let mut v = Vec::new();
        for p in [0u8, 1] {
            v.push((
                gun_fm_once(args, seed, p, 0)?,
                gun_fm_once(args, seed, p, 1)?.0,
                gun_fm_once(args, seed, p, 2)?.0,
            ));
        }
        Ok(v)
    })?;
    for v in all {
        for (p, ((o, n, s), m, c)) in v.into_iter().enumerate() {
            obs.push(o);
            per[p][0].push(o);
            rounds += n;
            runs += 1;
            started += s as u32;
            map.push(m);
            per[p][1].push(m);
            ctl.push(c);
        }
    }
    let (o, m, c) = (mean(&obs), mean(&map), mean(&ctl));
    Ok(GunFmRow {
        observed_miss_m: o,
        map_miss_m: m,
        ratio_pct: ratio(m, o),
        rounds_fired: (runs > 0).then_some(rounds as f64),
        observed_mode_pct: pct(started, runs),
        leig_miss_m: [mean(&per[0][0]), mean(&per[0][1])],
        m3_miss_m: [mean(&per[1][0]), mean(&per[1][1])],
        control: GunFmControl {
            observed_miss_m: c,
            ratio_pct: ratio(m, c),
        },
    })
}

/// One `mask` order of gun `p` (crest or flat): (refused masked, fired over).
fn mask_once(args: &FortifyArgs, seed: u64, p: u8, crest: bool) -> Result<(bool, bool)> {
    let mut w = world_with(args, seed, |s| {
        flat(s);
        if crest {
            s.map.terrain.bands.push(BandSetup {
                from: FixedVec2::from_m([500, 0]),
                to: FixedVec2::from_m([500, 1024]),
                height: frontline_content::Fixed::from_int(25),
                half_width: frontline_content::Fixed::from_int(40),
            });
        }
    })?;
    let what = format!("s11a1.mask seed {seed} {}", GUNS[p as usize]);
    let target = at(580, 500);
    let sq = spawn(&mut w, p, GUNS[p as usize], at(50, 500), EAST)?;
    if !w.mark_intel(PlayerId(p), target) {
        bail!("{what}: could not mark the target");
    }
    for _ in 0..2 * w.match_setup().unwrap().tick_hz {
        w.step(&[]);
    }
    w.enable_burst_log();
    match order(&mut w, p, sq, target, 1) {
        Some(r) => Ok((r == "masked", false)),
        None => {
            // Review B-2: an aim point (the point plus the mission's aim
            // error) in the crest's shadow has no clear arc: the mission
            // ends unfired (counted as not fired over), where before the
            // round burst on the far slope.
            let Some(m0) = mission(&w, sq) else {
                return Ok((false, false));
            };
            // The aim point of the round (the mission's aim error added):
            // a burst within 10 m of it flew its solution over the crest.
            let aim = target + m0.offset;
            let b = run_out(&mut w, sq, m0.id, &what)?;
            Ok((
                false,
                b.iter()
                    .any(|b| (b.pos.xy() - aim).length() <= Fx::from_int(MASK_LAND_M)),
            ))
        }
    }
}

/// The `mask` row (module docs).
pub fn mask_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<MaskRow> {
    let mut n = 0u32;
    let mut v = [[0u32; 2]; 2]; // [crest, flat][m3 refused, leig fired]
    let all = par_seeds(seeds, |seed| {
        let mut r = [[false; 2]; 2];
        for (k, crest) in [true, false].into_iter().enumerate() {
            r[k] = [
                mask_once(args, seed, 1, crest)?.0,
                mask_once(args, seed, 0, crest)?.1,
            ];
        }
        Ok(r)
    })?;
    for r in all {
        n += 1;
        for k in 0..2 {
            v[k][0] += r[k][0] as u32;
            v[k][1] += r[k][1] as u32;
        }
    }
    Ok(MaskRow {
        m3_refused_pct: pct(v[0][0], n),
        leig_fired_pct: pct(v[0][1], n),
        orders: (n > 0).then_some(2.0 * n as f64),
        control: MaskControl {
            m3_refused_pct: pct(v[1][0], n),
            leig_fired_pct: pct(v[1][1], n),
        },
    })
}

/// One `barrage` run: `kind` 0 open, 1 trench, 2 control (dug, squad 8 m
/// in front of it). (men lost, men, men ducked or pinned at the end, bursts).
fn barrage_once(args: &FortifyArgs, seed: u64, kind: u8) -> Result<(u32, u32, u32, u32)> {
    let mut w = world_with(args, seed, flat)?;
    let what = format!("s11a1.barrage seed {seed} kind {kind}");
    let (tx, ty) = BARRAGE_TARGET;
    let target = at(tx, ty);
    // Player 0's first line squad; the other start squads go far away.
    let rifle = super::squad(&w, 0, "line")?;
    for (id, q) in w.game().unwrap().squads().clone() {
        if id != rifle && q.owner == PlayerId(0) {
            w.place_squad(id, at(60, 950), EAST);
        }
    }
    if kind != 0 {
        w.dig_trench_now(&[at(tx, ty - 16), at(tx, ty + 16)], -1);
    }
    let spot = if kind == 2 { at(tx + 8, ty) } else { target };
    w.place_squad(rifle, spot, EAST);
    let men = members(&w, rifle).len() as u32;
    let gun = spawn(&mut w, 1, GUNS[1], at(tx + BARRAGE_RANGE_M, ty), WEST)?;
    spawn(&mut w, 1, FOS[1], at(tx + BARRAGE_FO_M, ty + 30), WEST)?;
    let g = w.game().unwrap().squad(gun).unwrap().members[0];
    w.edit_vehicle(g, |vs| {
        for n in vs.ammo.iter_mut() {
            *n = (*n).max(BARRAGE_ROUNDS);
        }
    });
    for _ in 0..2 * w.match_setup().unwrap().tick_hz {
        w.step(&[]);
    }
    let before = w.game().unwrap().players()[0].losses;
    w.enable_burst_log();
    if let Some(r) = order(&mut w, 1, gun, target, BARRAGE_ROUNDS) {
        bail!("{what}: refused {r}");
    }
    let id = mission(&w, gun)
        .ok_or_else(|| anyhow!("{what}: no mission"))?
        .id;
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let t0 = w.tick();
    let mut down = 0u32;
    let mut bursts = 0u32;
    let mut end = None;
    let mut pending = false;
    while end.is_none_or(|e| w.tick() < e + 20 * hz) {
        if end.is_none() && mission(&w, gun).is_none() {
            end = Some(w.tick());
        }
        if w.tick() > t0 + MISSION_MAX_S * hz {
            bail!("{what}: the mission never ended");
        }
        w.step(&[]);
        // The men's state one tick after a burst (they duck on the next tick).
        let read = pending;
        let n = w
            .take_burst_log()
            .iter()
            .filter(|b| b.mission == id)
            .count() as u32;
        bursts += n;
        pending = n > 0;
        if read {
            down = members(&w, rifle)
                .iter()
                .filter(|m| {
                    let c = &w.entities().combat()[m.index()];
                    c.hp > 0 && (c.ducked || c.pinned)
                })
                .count() as u32;
        }
    }
    let lost = w.game().unwrap().players()[0].losses - before;
    Ok((lost, men, down, bursts))
}

/// The `barrage` row (module docs).
pub fn barrage_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<BarrageRow> {
    let mut t = [[0u32; 4]; 3];
    let all = par_seeds(seeds, |seed| {
        (0..3u8)
            .map(|kind| barrage_once(args, seed, kind))
            .collect::<Result<Vec<_>>>()
    })?;
    for runs in all {
        for (kind, (l, m, d, b)) in runs.into_iter().enumerate() {
            let r = &mut t[kind];
            r[0] += l;
            r[1] += m;
            r[2] += d;
            r[3] += b;
        }
    }
    let loss = |k: usize| pct(t[k][0], t[k][1]);
    let alive = |k: usize| t[k][1].saturating_sub(t[k][0]);
    Ok(BarrageRow {
        open_loss_pct: loss(0),
        trench_loss_pct: loss(1),
        trench_vs_open_pct: ratio(loss(1), loss(0)),
        trench_suppressed_pct: pct(t[1][2], alive(1)),
        rounds_fired: (t[0][1] > 0).then_some((t[0][3] + t[1][3] + t[2][3]) as f64),
        control: BarrageControl {
            trench_vs_open_pct: ratio(loss(2), loss(0)),
            trench_suppressed_pct: pct(t[2][2], alive(2)),
        },
    })
}

/// Every S11a1 row (`breach_prep` and `breach_prep_gun`: Build C).
pub fn s11a1_rows(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<S11a1Report> {
    Ok(S11a1Report {
        gun_fm: gun_fm_row(args, seeds.clone())?,
        mask: mask_row(args, seeds.clone())?,
        barrage: barrage_row(args, seeds)?,
        breach_prep: BreachPrepRow::default(),
        breach_prep_gun: BreachPrepRow::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// C5 (Build B): each row runs on one seed and its control differs from
    /// its value (the radio man, the crest, the trench make a difference).
    #[test]
    fn fortify_s11a1_rows_run() {
        let a = FortifyArgs::default();
        let r = s11a1_rows(&a, 1..2).unwrap();
        eprintln!("{}", serde_json::to_string(&r).unwrap());
        let g = &r.gun_fm;
        assert!(g.rounds_fired.unwrap() > 0.0);
        assert_ne!(g.ratio_pct, g.control.ratio_pct, "gun_fm control");
        let m = &r.mask;
        assert_ne!(m.m3_refused_pct, m.control.m3_refused_pct, "mask control");
        let b = &r.barrage;
        assert!(b.rounds_fired.unwrap() > 0.0);
        assert_ne!(
            b.trench_vs_open_pct, b.control.trench_vs_open_pct,
            "barrage control"
        );
        assert!(r.breach_prep.gap_opened_pct.is_none());
    }

    /// One-off: the S11a1 rows on seeds 1-10 (as `fortify-test --seeds
    /// 10`), with their wall time; run by hand in release.
    #[test]
    #[ignore]
    fn s11a1_rows_one_off() {
        let t = std::time::Instant::now();
        let r = s11a1_rows(&FortifyArgs::default(), 1..11).unwrap();
        eprintln!("{}", serde_json::to_string(&r).unwrap());
        eprintln!("s11a1 rows wall s: {:.1}", t.elapsed().as_secs_f64());
    }
}
