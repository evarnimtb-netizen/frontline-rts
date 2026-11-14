//! S10b-ai tests (docs/slices/S10b-ai.md R7; Build C chunk 2): the AI builds
//! one bunker at its held, trenched point nearest the enemy base, at the
//! trench end opposite the nest, and its MG team defends from it.

use super::s6_tests::steps;
use super::s10b_ai_lay_tests::{Held, held, squad_at};
use super::*;
use crate::ai::{AiState, FortPhase, TaskKind};
use crate::game::SquadId;
use frontline_content::setup::Capability;

const BAU: &str = "ww2:ger_baupioniergruppe";
const ENG: &str = "ww2:usa_engineer_squad";
const LMG: &str = "ww2:usa_lmg_squad";

fn c(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

fn bunker(w: &World) -> u16 {
    w.match_setup()
        .unwrap()
        .forts
        .iter()
        .position(|f| f.id == "ww2:bunker")
        .unwrap() as u16
}

fn play(w: &mut World, secs: u64) {
    steps(w, secs * 30);
}

fn ai_ro(w: &World) -> &AiState {
    w.game().unwrap().ai[1].as_ref().unwrap()
}

/// The held point with only the bunker habit on (no mines, obstacles, S10a
/// works or wire); `bunker` the switch.
pub(crate) fn bunker_world(
    bunker: u32,
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> (World, Held) {
    held(|s| {
        s.rules.ai.fort.wire = 0;
        s.rules.ai.obstacles.mines = 0;
        s.rules.ai.obstacles.obstacles = 0;
        s.rules.ai.obstacles.bunker = bunker;
        edit(s);
    })
}

/// R7's site at point 0 (even: the nest takes trench end `line[1]`, the
/// bunker `line[0]`, `nest_offset_m` beyond it).
pub(crate) fn bunker_site(w: &World, h: &Held) -> FxVec2 {
    let off = w.match_setup().unwrap().rules.ai.fort.nest_offset_m as i32;
    let [a, b] = h.line;
    a + (a - b).normalize_or_zero() * Fx::from_int(off)
}

/// A player 1 builder of `card` at the point carrying its full carry, and
/// three 40 kg construction crates at the bunker site (250 kg is over any
/// squad's carry: the kg at hand, `at_hand`).
pub(crate) fn builder(w: &mut World, h: &Held, card: &str, at: FxVec2) -> SquadId {
    let site = bunker_site(w, h);
    let def = w
        .match_setup()
        .unwrap()
        .stocks
        .iter()
        .position(|d| d.id == "ww2:usa_construction_crate")
        .unwrap() as u16;
    for k in 0..3 {
        w.add_crate_now(
            PlayerId(1),
            def,
            site + h.enemy * Fx::from_int(-6) + c(k, 0),
        )
        .unwrap();
    }
    squad_at(w, card, at, 250)
}

/// Player 1's built bunker centres (roof pieces).
fn built(w: &World) -> Vec<FxVec2> {
    let b = bunker(w);
    w.game()
        .unwrap()
        .fort()
        .pieces
        .values()
        .filter(|q| q.owner == PlayerId(1) && q.fort == b && q.part == 0)
        .map(|q| q.pos)
        .collect()
}

/// Player 1's bunker jobs (squad, line start).
fn jobs(w: &World) -> Vec<(SquadId, FxVec2)> {
    let b = bunker(w);
    ai_ro(w)
        .forts
        .iter()
        .filter(|(_, j)| j.fort == b && j.phase != FortPhase::Done)
        .map(|(&s, j)| (s, j.line[0]))
        .collect()
}

/// Plays up to `secs` until a bunker stands; (built, the most bunker jobs
/// alive at once, seconds).
fn build(w: &mut World, secs: u64) -> (Vec<FxVec2>, usize, u64) {
    let mut most = 0;
    for t in 0..secs {
        most = most.max(jobs(w).len());
        if !built(w).is_empty() {
            return (built(w), most, t);
        }
        play(w, 1);
    }
    (built(w), most, secs)
}

/// R7: Baupioniere with 250 kg build the one bunker at the trench end
/// opposite the nest, facing the enemy; afterwards an MG team at the point
/// gets the bunker centre as its Defend target and no second bunker starts
/// (`bunkers_max` 1). Control: `bunker` 0 -> no bunker job (seen failing as
/// "a bunker" when asserted to build); the manning arm in `order_infantry`
/// dropped -> "mans the bunker" fails (seen).
#[test]
fn ai_builds_and_mans_one_bunker() {
    let (mut w, h) = bunker_world(1, |_| {});
    let site = bunker_site(&w, &h);
    builder(&mut w, &h, BAU, h.pos);
    let (got, most, secs) = build(&mut w, 1500);
    println!("bunker built after {secs} s");
    assert_eq!(got.len(), 1, "a bunker: {got:?}");
    assert!(
        (got[0] - site).length() < Fx::from_int(2),
        "at the site: {:?} vs {site:?}",
        got[0]
    );
    assert_eq!(most, 1, "one job");
    // Manning: the MG team defends from the bunker.
    let lmg = squad_at(&mut w, LMG, h.pos + c(5, 0), 0);
    squad_at(&mut w, BAU, h.pos + c(-5, 0), 250);
    play(&mut w, 30);
    let t = ai_ro(&w).tasks[&lmg];
    assert_eq!(t.kind, TaskKind::Defend);
    assert!(
        (t.target - got[0]).length() < Fx::from_int(1),
        "mans the bunker: {:?}",
        t.target
    );
    play(&mut w, 120);
    assert!(
        jobs(&w).is_empty() && built(&w).len() == 1,
        "no second bunker"
    );
    // Control.
    let (mut w, h) = bunker_world(0, |_| {});
    builder(&mut w, &h, BAU, h.pos);
    let (got, most, _) = build(&mut w, 120);
    assert!(got.is_empty() && most == 0, "control: bunker 0 -> none");
}

/// R7 builders (S10b R21): with no `build_heavy` squad, US engineers
/// (`build_structure`) build the bunker at `heavy_by_field_pct`; with both
/// at the point the Baupioniere take it though the engineers have the
/// lower id. Control: the engineers without `build_structure` (a heavy-only
/// rule) -> no bunker for the USA; the `heavy_by_field` key dropped from
/// `point_starts`' pick -> "Baupioniere first" fails (seen).
#[test]
fn ai_bunker_needs_heavy_or_field_builder() {
    let (mut w, h) = bunker_world(1, |_| {});
    let eng = builder(&mut w, &h, ENG, h.pos);
    let (got, _, secs) = build(&mut w, 1500);
    println!("US engineers built it after {secs} s");
    assert_eq!(got.len(), 1, "engineers build at 50 %");
    assert!(
        w.game()
            .unwrap()
            .fort()
            .pieces
            .values()
            .any(|q| q.fort == bunker(&w))
    );
    let _ = eng;
    // Both: build_heavy first.
    let (mut w, h) = bunker_world(1, |_| {});
    let eng = builder(&mut w, &h, ENG, h.pos);
    let bau = squad_at(&mut w, BAU, h.pos + c(3, 0), 250);
    assert!(eng < bau);
    play(&mut w, 30);
    let j = jobs(&w);
    assert_eq!(j.len(), 1, "a job: {j:?}");
    assert_eq!(j[0].0, bau, "Baupioniere first");
    // Control: no field builder.
    let (mut w, h) = bunker_world(1, |s| {
        for t in s.squad_types.iter_mut().filter(|t| t.id == ENG) {
            t.caps &= !Capability::BuildStructure.bit();
        }
    });
    builder(&mut w, &h, ENG, h.pos);
    let (got, most, _) = build(&mut w, 120);
    assert!(
        got.is_empty() && most == 0,
        "control: no field builder -> none"
    );
}

/// Build D (the bunker never built in 40 default matches: every started
/// job ended `end_failed` the second its pit was dug): the 250 kg step comes
/// only by truck, and the one truck is busy (Return) when the step starts
/// waiting; the job waits while a truck job lives (L1 bounds it), takes
/// two 200 kg runs and the bunker stands. Control: the Watch wait for a
/// truck not yet free dropped (`heavy` false) -> the job ends failed at
/// once, "the bunker stands" fails (seen).
#[test]
fn ai_bunker_waits_for_a_busy_truck() {
    let (mut w, h) = bunker_world(1, |s| {
        // S10a on (its truck runs) with no S10a work of its own.
        let f = &mut s.rules.ai.fort;
        f.works = 1;
        f.nest = 0;
        f.dugout = 0;
        f.roof = 0;
        f.sandbags = 0;
    });
    let base = w.game().unwrap().base_of(PlayerId(1));
    let truck = squad_at(&mut w, "ww2:usa_cckw_squad", base, 0);
    squad_at(&mut w, BAU, h.pos, 40);
    let b = bunker(&w);
    let mut busy = false;
    let mut done = false;
    for _ in 0..30 * 1500 {
        steps(&mut w, 1);
        let g = w.game().unwrap();
        let waiting = g
            .fort()
            .works
            .values()
            .any(|k| k.fort == b && k.step == crate::fort::WorkStep::Wait);
        if waiting && !busy {
            busy = true;
            let t = w.state.tick;
            let ai = w.state.game.as_mut().unwrap().ai[1].as_mut().unwrap();
            let j = ai.trucks.get_mut(&truck).expect("a truck job");
            j.phase = crate::ai::TruckPhase::Return;
            j.since = t;
        }
        if !built(&w).is_empty() {
            done = true;
            break;
        }
    }
    let c = &ai_ro(&w).counters;
    println!(
        "busy {busy}, runs {:?}, why {:?}",
        c.material_runs, c.bunker_why
    );
    assert!(busy, "the step waited for kg");
    assert!(done, "the bunker stands");
    assert_eq!(c.bunker_why[14], 0, "no failed bunker job");
}

/// The bunker world with a second own point (no trench) 120 m from the
/// bunker point; Baupioniere with the kg there are assigned to it, not to
/// the bunker point.
fn assign_world(assign: u32) -> (World, Held, SquadId) {
    let (mut w, h) = bunker_world(1, |s| {
        s.rules.ai.obstacles.bunker_assign = assign;
        let mut p = s.map.points[0].clone();
        p.pos.x.0 += 120 << 16;
        s.map.points.push(p);
    });
    w.state.game.as_mut().unwrap().points[1].owner = Some(PlayerId(1));
    let far: FxVec2 = w.match_setup().unwrap().map.points[1].pos.into();
    let bau = builder(&mut w, &h, BAU, far + c(0, 5));
    (w, h, bau)
}

/// Build E item 1 (`bunker_assign`, S10b-ai-D1): the only fit builder is
/// assigned to another own point (the 40 set's `no_squad` / `no_builder`);
/// the AI sends it to the bunker point for the bunker job. Control:
/// `bunker_assign` 0 -> no bunker job, `bunker_why` counts `no_squad`
/// (seen failing as "a bunker job" with the control asserted to start).
#[test]
fn ai_bunker_assigns_a_free_builder() {
    let (mut w, _h, bau) = assign_world(1);
    let mut got = false;
    for _ in 0..90 {
        play(&mut w, 1);
        if jobs(&w).iter().any(|j| j.0 == bau) {
            got = true;
            break;
        }
    }
    let why = ai_ro(&w).counters.bunker_why;
    println!("assign 1: job {got}, why {why:?}");
    assert!(got, "a bunker job for the Baupioniere");
    let (mut w, _h, bau) = assign_world(0);
    play(&mut w, 90);
    let why = ai_ro(&w).counters.bunker_why;
    println!("assign 0: why {why:?}");
    assert!(
        !jobs(&w).iter().any(|j| j.0 == bau),
        "control: bunker_assign 0 -> no bunker job"
    );
    assert!(why[6] + why[7] > 0, "control: no_squad or no_builder");
}

/// Plays the bunker with one loaded truck in tank service (Follow) from the
/// moment the bunker step waits for kg; `first` the `bunker_truck_first`
/// key. (the truck was sent back for the run, the bunker stands, counters).
fn truck_first_run(first: u32) -> (bool, bool, crate::ai::AiCounters) {
    let (mut w, h) = bunker_world(1, |s| {
        s.rules.ai.obstacles.bunker_truck_first = first;
        let f = &mut s.rules.ai.fort;
        f.works = 1;
        f.nest = 0;
        f.dugout = 0;
        f.roof = 0;
        f.sandbags = 0;
    });
    let base = w.game().unwrap().base_of(PlayerId(1));
    let truck = squad_at(&mut w, "ww2:usa_cckw_squad", base, 0);
    let tv = w.game().unwrap().squads[&truck].members[0];
    squad_at(&mut w, BAU, h.pos, 40);
    let b = bunker(&w);
    let (mut serving, mut sent, mut done) = (false, false, false);
    for _ in 0..30 * 1500 {
        steps(&mut w, 1);
        let g = w.game().unwrap();
        let waiting = g
            .fort()
            .works
            .values()
            .any(|k| k.fort == b && k.step == crate::fort::WorkStep::Wait);
        if waiting && !serving {
            serving = true;
            let t = w.state.tick;
            let g = w.state.game.as_mut().unwrap();
            // A full load of shells: Follow never returns on its own.
            g.vehicles.get_mut(&tv).unwrap().cargo_g = [320_000, 120_000, 40_000];
            let j = g.ai[1]
                .as_mut()
                .unwrap()
                .trucks
                .get_mut(&truck)
                .expect("a truck job");
            j.phase = crate::ai::TruckPhase::Follow;
            j.since = t;
        } else if serving && !sent {
            sent = ai_ro(&w)
                .trucks
                .get(&truck)
                .is_some_and(|j| j.phase == crate::ai::TruckPhase::Return);
        }
        if !built(&w).is_empty() {
            done = true;
            break;
        }
    }
    assert!(serving, "the step waited for kg");
    (sent, done, ai_ro(&w).counters.clone())
}

/// Build E item 1 (`bunker_truck_first`, S10b-ai-D1): the bunker's 250 kg
/// step waits while the only truck serves tanks (Follow, loaded); it is sent
/// back to base for one run and the bunker stands. Control:
/// `bunker_truck_first` 0 -> the truck stays in service, the job ends at its
/// deadline (`end_deadline`), no bunker (seen failing as "the truck was
/// sent" with the control asserted to send).
#[test]
fn ai_bunker_takes_a_truck_from_service() {
    let (sent, done, c) = truck_first_run(1);
    println!(
        "first 1: sent {sent}, built {done}, runs {:?}, why {:?}",
        c.material_runs, c.bunker_why
    );
    assert!(sent, "the truck was sent back for the run");
    assert!(done, "the bunker stands");
    assert_eq!(c.bunker_why[14], 0, "no failed bunker job");
    let (sent, done, c) = truck_first_run(0);
    println!(
        "first 0: sent {sent}, built {done}, runs {:?}, why {:?}",
        c.material_runs, c.bunker_why
    );
    assert!(
        !sent && !done,
        "control: bunker_truck_first 0 -> no run, no bunker"
    );
}
