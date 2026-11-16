//! S10c-ai Build B chunk 1 tests (docs/slices/S10c-ai.md 4 F1-F4, F7, F10;
//! `S10c-ai-build-B.md` items 1-5): front lanes, their site lines, the front
//! wants before the point's other works, the caps and the carry rule.

use super::s6_tests::{flat_world_with, members, run, spawn, steps};
use super::s10b_ai_lay_tests::{held, squad_at};
use super::*;
use crate::ai::fort::FRONT_WHY;
use crate::ai::{AiState, FortPhase, FrontLane};
use crate::command::CommandKind;
use crate::game::SquadId;

const ENG: &str = "ww2:usa_engineer_squad";

fn c(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

fn fort(w: &World, id: &str) -> u16 {
    w.match_setup()
        .unwrap()
        .forts
        .iter()
        .position(|f| f.id == id)
        .unwrap() as u16
}

fn ai_ro(w: &World) -> &AiState {
    w.game().unwrap().ai[1].as_ref().unwrap()
}

fn ai_mut(w: &mut World) -> &mut AiState {
    w.state.game.as_mut().unwrap().ai[1].as_mut().unwrap()
}

fn play(w: &mut World, secs: u64) {
    steps(w, secs * 30);
}

/// Plays up to `secs`, stopping once `done` holds; whether it did.
fn play_until(w: &mut World, secs: u64, mut done: impl FnMut(&World) -> bool) -> bool {
    for _ in 0..secs {
        if done(w) {
            return true;
        }
        play(w, 1);
    }
    done(w)
}

fn point(w: &World, name: &str) -> usize {
    w.match_setup()
        .unwrap()
        .map
        .points
        .iter()
        .position(|p| p.name == name)
        .unwrap_or(usize::MAX)
}

fn pos_of(w: &World, i: usize) -> FxVec2 {
    w.match_setup().unwrap().map.points[i].pos.into()
}

fn why(name: &str) -> usize {
    FRONT_WHY.iter().position(|n| *n == name).unwrap()
}

/// The staged front: AI player 1 (USA, no start squads, no buys, S10a
/// `works` off, `front` on) owns C; `near` is the one of B and D on the
/// AI's own side (300 m from C), `far` is E (346 m from C).
struct Front {
    own: usize,
    pos: FxVec2,
    near: usize,
    far: usize,
}

fn scene(edit: impl FnOnce(&mut frontline_content::MatchSetup)) -> (World, Front) {
    let w = flat_world_with(1, |s| {
        s.players[1].is_ai = true;
        let f = s.players[1].faction as usize;
        s.factions[f].start_squads.clear();
        s.squad_types.iter_mut().for_each(|t| t.ai_weight = 0);
        s.rules.ai.fort.works = 0;
        s.rules.ai.obstacles.front = 1;
        edit(s);
    });
    let own = point(&w, "C");
    let eb = w.game().unwrap().base_of(PlayerId(0));
    // The one of B and D farther from the enemy base: on the AI's side.
    let (b, d) = (point(&w, "B"), point(&w, "D"));
    let near = if b == usize::MAX || d == usize::MAX {
        usize::MAX
    } else if (pos_of(&w, b) - eb).length_sq_raw() > (pos_of(&w, d) - eb).length_sq_raw() {
        b
    } else {
        d
    };
    let far = point(&w, "E");
    let pos = pos_of(&w, own);
    let mut w = w;
    w.state.game.as_mut().unwrap().points[own].owner = Some(PlayerId(1));
    (
        w,
        Front {
            own,
            pos,
            near,
            far,
        },
    )
}

fn own_by(w: &mut World, i: usize, p: Option<u8>) {
    w.state.game.as_mut().unwrap().points[i].owner = p.map(PlayerId);
}

fn lane_of(w: &World, i: usize) -> Option<FrontLane> {
    ai_ro(w)
        .front_lanes
        .iter()
        .find(|l| l.point as usize == i)
        .copied()
}

/// Whether `line` lies across the foot path from `from` to `to` (wire cost
/// off): its middle within 2 m of the path, `out` (-6..=+1) m straight from
/// `to` (the path's last `out` m may bend), and across the path's leg
/// nearest it (within ~6 degrees of the perpendicular).
fn across(w: &World, line: [FxVec2; 2], to: FxVec2, from: FxVec2, out: i32) -> bool {
    let way = foot_path(w, from, to, false);
    let mid = (line[0] + line[1]).div_int(2);
    let two = Fx::from_int(2);
    let Some(leg) = way
        .windows(2)
        .find(|p| crate::fort::near_segment(mid, p[0], p[1], two))
    else {
        return false;
    };
    let dir = (leg[1] - leg[0]).normalize_or_zero();
    let l = (line[1] - line[0]).normalize_or_zero();
    let cos = (l.x * dir.x + l.y * dir.y).abs();
    let d = (mid - to).length().floor_int();
    (out - 6..=out + 1).contains(&d) && cos <= Fx::from_milli(100)
}

/// Player 1's mines of blueprint `f` within 5 m of `line`.
fn mines_on(w: &World, f: u16, line: [FxVec2; 2]) -> usize {
    w.game()
        .unwrap()
        .fort()
        .mines
        .values()
        .filter(|m| {
            m.owner == PlayerId(1)
                && m.fort == f
                && crate::fort::near_segment(m.pos, line[0], line[1], Fx::from_int(5))
        })
        .count()
}

/// Player 1's live fort jobs on blueprint `f` at point `i`.
fn live_jobs(w: &World, f: u16, i: usize) -> usize {
    ai_ro(w)
        .forts
        .values()
        .filter(|j| j.fort == f && j.point as usize == i && j.phase != FortPhase::Done)
        .count()
}

/// F2: the lane of C runs from the enemy-owned point nearest C. Enemy E
/// alone: both lines lie across the way C -> E (wire 50 m out, 20 m long;
/// AP 55 m out, 30 m long). Enemy E and `near` (300 m < 346 m): from
/// `near`. Control: no enemy point -> from the enemy base (seen failing
/// the E assertion there: the lane is not across C -> E).
#[test]
fn ai_front_lane_runs_from_the_nearest_enemy_point() {
    let lane = |owned: &[usize]| {
        let (mut w, f) = scene(|_| {});
        for &i in owned {
            own_by(&mut w, i, Some(0));
        }
        play(&mut w, 3);
        (lane_of(&w, f.own).expect("a lane at C"), w, f)
    };
    let (_, _, f0) = lane(&[]);
    let (l, w, f) = lane(&[f0.far]);
    let e = pos_of(&w, f.far);
    assert!(
        across(&w, l.wire, f.pos, e, 50),
        "wire across C -> E: {l:?}"
    );
    assert!(across(&w, l.ap, f.pos, e, 55), "AP across C -> E: {l:?}");
    let len = |a: [FxVec2; 2]| (a[1] - a[0]).length().floor_int();
    assert!((19..=20).contains(&len(l.wire)), "wire 20 m: {l:?}");
    assert!((29..=30).contains(&len(l.ap)), "AP 30 m: {l:?}");
    assert!(!l.ap_done && !l.wire_done);
    let (l, w, f) = lane(&[f0.far, f0.near]);
    assert!(
        across(&w, l.wire, f.pos, pos_of(&w, f.near), 50),
        "from near: {l:?}"
    );
    assert!(
        !across(&w, l.wire, f.pos, pos_of(&w, f.far), 50),
        "not from E: {l:?}"
    );
    // Control: no enemy point: from the enemy base, not across C -> E.
    let (l, w, f) = lane(&[]);
    let eb = w.game().unwrap().base_of(PlayerId(0));
    assert!(
        across(&w, l.wire, f.pos, eb, 50),
        "control: from the base {l:?}"
    );
    assert!(!across(&w, l.wire, f.pos, pos_of(&w, f.far), 50), "control");
}

/// F3, F4 (and item 5): at an own UNtrenched front point (enemy point 300 m
/// away) a US engineer squad carrying 120 kg lays 15 AP mines within 5 m of
/// the saved AP line across the lane 55 m out; the held builder takes the
/// job before any trench is dug. Control: `front` 0 -> no mine on that
/// line in the same time (seen failing as "15 mines").
#[test]
fn ai_lays_front_field_without_a_trench() {
    let lay = |front: u32| {
        let (mut w, f) = scene(|s| s.rules.ai.obstacles.front = front);
        own_by(&mut w, f.near, Some(0));
        squad_at(&mut w, ENG, f.pos, 120);
        let ap = fort(&w, "ww2:ap_minefield");
        let mut dug_first = false;
        let mut seen = false;
        play_until(&mut w, 600, |w| {
            if !seen && live_jobs(w, ap, f.own) > 0 {
                seen = true;
                dug_first = !ai_ro(w).trenches.is_empty();
            }
            ai_ro(w).front_lanes.first().is_some_and(|l| l.ap_done)
        });
        // The expected line (the lane is straight on the flat map).
        let to = pos_of(&w, f.near);
        let dir = (to - f.pos).normalize_or_zero();
        let mid = f.pos + dir * Fx::from_int(55);
        let line = [
            mid - dir.perp() * Fx::from_int(15),
            mid + dir.perp() * Fx::from_int(15),
        ];
        let n = mines_on(&w, ap, line);
        (n, dug_first, lane_of(&w, f.own), w.state.tick)
    };
    let (n, dug_first, lane, _) = lay(1);
    let lane = lane.expect("a lane");
    assert!(lane.ap_done, "ap_done set");
    assert_eq!(n, 15, "15 mines on the front line");
    assert!(!dug_first, "the held builder starts the job, not a trench");
    let (n, _, lane, _) = lay(0);
    assert_eq!(n, 0, "control: front 0");
    assert!(lane.is_none(), "control: no lane");
}

/// F4: at a trenched front point with no nest (S10a works on), the first
/// job is FrontAp. Control: `front` 0 -> the nest (seen failing the first
/// assertion there).
#[test]
fn ai_front_wants_come_before_the_nest() {
    let first = |front: u32| {
        // Build C (F1): the lone held point is the home point; plain F1
        // (`front_home` 1) keeps it a front point.
        let (mut w, h) = held(|s| {
            s.rules.ai.fort.works = 1;
            s.rules.ai.obstacles.front = front;
            s.rules.ai.obstacles.front_home = 1;
        });
        squad_at(&mut w, ENG, h.pos, 120);
        play_until(&mut w, 30, |w| !ai_ro(w).forts.is_empty());
        let j = ai_ro(&w).forts.values().next().cloned().expect("a job");
        let s = w.match_setup().unwrap();
        let lane = lane_of(&w, 0);
        (s.forts[j.fort as usize].id.clone(), j, lane)
    };
    let (id, j, lane) = first(1);
    assert_eq!(id, "ww2:ap_minefield", "FrontAp first");
    let lane = lane.expect("a lane");
    assert_eq!(
        j.line.last().copied(),
        Some(lane.ap[1]),
        "on the lane's AP line"
    );
    let (id, _, _) = first(0);
    assert_eq!(id, "ww2:mg_nest", "control: the nest");
}

/// F7: with two saved lanes holding a field (`ap_done`), a third front
/// point gets no FrontAp (`front_fields_max` 2; `front_why` cap). With
/// `front_fields_max` 9 the blueprint cap holds instead: 60 own AP mines
/// stand -> no FrontAp, `front_why` cap. Control: `front_fields_max` 9, no
/// mines -> the FrontAp job starts (seen failing as "no FrontAp").
#[test]
fn ai_front_caps_hold() {
    let jobs = |max: u32, full: bool| {
        let (mut w, f) = scene(|s| {
            s.rules.ai.obstacles.front_points = 3;
            s.rules.ai.obstacles.front_fields_max = max;
            s.rules.ai.obstacles.at_mines = 0;
            s.rules.ai.fort.wire = 0;
        });
        let (a, e) = (point(&w, "A"), point(&w, "E"));
        own_by(&mut w, a, Some(1));
        own_by(&mut w, e, Some(1));
        play(&mut w, 3);
        assert_eq!(ai_ro(&w).front_lanes.len(), 3, "three lanes");
        for l in ai_mut(&mut w).front_lanes.iter_mut() {
            if l.point as usize != f.own {
                l.ap_done = true;
            }
        }
        let ap = fort(&w, "ww2:ap_minefield");
        if full {
            let base = w.game().unwrap().base_of(PlayerId(1));
            for k in 0..4 {
                let at = base + c(-60 + 10 * k, -20);
                assert!(w.build_now(ap, &[at, at + c(0, -29)], PlayerId(1)));
            }
            let n = w.game().unwrap().fort().mines.len();
            assert_eq!(n, 60, "the blueprint cap stands");
        }
        squad_at(&mut w, ENG, f.pos, 120);
        let mut most = 0;
        for _ in 0..30 {
            play(&mut w, 1);
            most = most.max(live_jobs(&w, ap, f.own));
        }
        (most, ai_ro(&w).counters.front_why)
    };
    let (n, fw) = jobs(2, false);
    assert_eq!(n, 0, "front_fields_max 2: no third field");
    assert!(fw[why("cap")] >= 1, "{fw:?}");
    let (n, fw) = jobs(9, true);
    assert_eq!(n, 0, "the blueprint cap: no field");
    assert!(fw[why("cap")] >= 1, "{fw:?}");
    let (n, _) = jobs(9, false);
    assert_eq!(n, 1, "control: front_fields_max 9 -> the FrontAp job");
}

/// Job table [B] "another squad on it": two builders at the front point,
/// `jobs_max` 2: never two live jobs on one front site; both FrontAp and
/// FrontWire run. Control: `front_wants` blind to a live job of its kind
/// -> both builders take the AP site (seen failing, code edit).
#[test]
fn ai_two_builders_never_share_a_front_site() {
    let (mut w, f) = scene(|s| {
        s.rules.ai.obstacles.jobs_max = 2;
        s.rules.ai.obstacles.at_mines = 0;
        s.rules.ai.fort.builders_max = 4;
    });
    own_by(&mut w, f.near, Some(0));
    squad_at(&mut w, ENG, f.pos, 120);
    squad_at(&mut w, ENG, f.pos + c(4, 0), 120);
    let (ap, wire) = (fort(&w, "ww2:ap_minefield"), fort(&w, "ww2:barbed_wire"));
    let (mut ap_ran, mut wire_ran) = (false, false);
    for _ in 0..240 {
        play(&mut w, 1);
        let (a, b) = (live_jobs(&w, ap, f.own), live_jobs(&w, wire, f.own));
        assert!(
            a <= 1 && b <= 1,
            "two jobs on one front site: {a} AP, {b} wire"
        );
        ap_ran |= a == 1;
        wire_ran |= b == 1;
    }
    assert!(ap_ran && wire_ran, "FrontAp {ap_ran}, FrontWire {wire_ran}");
}

/// F2: the lane saved on the first (recheck) plan makes the want show on
/// later plans: a builder that arrives at 5 s gets its FrontAp job by 15 s.
/// Control: the entry removed at 5 s -> no front job before the next
/// recheck plan (60 s), one after it (seen failing as "by 15 s").
#[test]
fn ai_front_wants_show_on_every_plan() {
    let run = |remove: bool| {
        let (mut w, f) = scene(|s| s.rules.ai.obstacles.at_mines = 0);
        own_by(&mut w, f.near, Some(0));
        play(&mut w, 5);
        assert!(lane_of(&w, f.own).is_some(), "found on the first plan");
        if remove {
            ai_mut(&mut w).front_lanes.clear();
        }
        squad_at(&mut w, ENG, f.pos, 120);
        let ap = fort(&w, "ww2:ap_minefield");
        let early = play_until(&mut w, 10, |w| live_jobs(w, ap, f.own) > 0);
        let late = play_until(&mut w, 60, |w| live_jobs(w, ap, f.own) > 0);
        (early, late, w.state.tick / 30)
    };
    let (early, _, _) = run(false);
    assert!(early, "the FrontAp job by 15 s");
    let (early, late, at) = run(true);
    assert!(!early, "control: no lane, no want before the recheck");
    assert!(
        late && at >= 60,
        "control: the job after the recheck ({at} s)"
    );
}

/// F10: a builder at the front point (far from its base) carrying nothing
/// takes no front job (`front_why` no_carry). Control: carrying 60 kg ->
/// the FrontAp job (seen failing as "no front job" with the carry rule
/// off).
#[test]
fn ai_empty_builder_takes_no_front_job() {
    let run = |kg: u32| {
        let (mut w, f) = scene(|s| s.rules.ai.obstacles.at_mines = 0);
        own_by(&mut w, f.near, Some(0));
        squad_at(&mut w, ENG, f.pos, kg);
        let ap = fort(&w, "ww2:ap_minefield");
        let wire = fort(&w, "ww2:barbed_wire");
        let mut most = 0;
        for _ in 0..60 {
            play(&mut w, 1);
            most = most.max(live_jobs(&w, ap, f.own) + live_jobs(&w, wire, f.own));
        }
        (
            most,
            ai_ro(&w).counters.front_why,
            ai_ro(&w).counters.front_jobs,
        )
    };
    let (n, fw, jobs) = run(0);
    assert_eq!((n, jobs), (0, 0), "no front job with nothing carried");
    assert!(fw[why("no_carry")] >= 10, "{fw:?}");
    let (n, _, jobs) = run(60);
    assert!(n >= 1 && jobs >= 1, "control: 60 kg -> a front job");
}

/// F10, L1: a builder spawned in its base (empty) about 600 m from the
/// front point loads there, walks out and lays the 15 mines; the job ends
/// laid inside `job_max_s` 900 s (`jobs_overdue` 0). Control: `job_max_s`
/// 120 -> the job ends by the deadline before it is laid (seen failing as
/// "laid").
#[test]
fn ai_front_job_from_base_ends_inside_its_deadline() {
    let run = |max_s: u32| {
        // C alone on the map: the builder has no other point to go to.
        // Build C (F1): C is then the home point, so plain F1 makes it front.
        let (mut w, f) = scene(|s| {
            s.map.points.retain(|p| p.name == "C");
            s.rules.ai.obstacles.front_home = 1;
            s.rules.ai.obstacles.at_mines = 0;
            s.rules.ai.fort.wire = 0;
            s.rules.ai.obstacles.job_max_s = max_s;
        });
        let base = w.game().unwrap().base_of(PlayerId(1));
        let far = (f.pos - base).length().floor_int();
        assert!(far >= 550, "the base {far} m away");
        let eng = squad_at(&mut w, ENG, base, 0);
        let ap = fort(&w, "ww2:ap_minefield");
        let mut start = None;
        let mut end = None;
        for s in 0..1000u64 {
            play(&mut w, 1);
            let live = ai_ro(&w).forts.get(&eng).is_some_and(|j| j.fort == ap);
            if live && start.is_none() {
                start = Some(s);
            }
            if !live && start.is_some() {
                end = Some(s);
                break;
            }
        }
        let laid = lane_of(&w, f.own).is_some_and(|l| l.ap_done);
        (
            laid,
            start,
            end,
            ai_ro(&w).counters.jobs_overdue,
            mines_on(&w, ap, lane_of(&w, f.own).unwrap().ap),
        )
    };
    let (laid, start, end, overdue, n) = run(900);
    let took = end.zip(start).map(|(e, s)| e - s);
    assert!(laid && n == 15, "laid: {laid}, {n} mines, took {took:?} s");
    assert!(took.is_some_and(|t| t < 900), "inside 900 s: {took:?}");
    assert_eq!(overdue, 0);
    let (laid, _, _, _, _) = run(120);
    assert!(!laid, "control: job_max_s 120");
}

/// The lane of C from `near` with the front wire and AP field standing on
/// its saved lines (`edit` on the setup first).
fn staged_lines(edit: fn(&mut frontline_content::MatchSetup)) -> (World, Front, FrontLane) {
    let (mut w, f) = scene(edit);
    own_by(&mut w, f.near, Some(0));
    play(&mut w, 3);
    let lane = lane_of(&w, f.own).expect("a lane");
    let (ap, wire) = (fort(&w, "ww2:ap_minefield"), fort(&w, "ww2:barbed_wire"));
    assert!(w.build_now(wire, &lane.wire, PlayerId(1)), "wire {lane:?}");
    assert!(w.build_now(ap, &lane.ap, PlayerId(1)), "AP {lane:?}");
    (w, f, lane)
}

/// F3: an enemy rifle squad ordered along the lane goes round the 20 m wire
/// (500 % wire cost) and its leader crosses the AP row within 15 m of the
/// lane: a mine goes off. Control: `front_wire_len_m` 40 -> the leader
/// crosses the row's line more than 15 m off the lane (outside the row;
/// seen failing as "within 15 m").
#[test]
fn front_wire_detour_crosses_the_field() {
    let walk = |edit: fn(&mut frontline_content::MatchSetup)| {
        let (mut w, f, _) = staged_lines(edit);
        let to = pos_of(&w, f.near);
        let dir = (to - f.pos).normalize_or_zero();
        let from = f.pos + dir * Fx::from_int(150);
        let card = "ww2:ger_grenadiergruppe";
        let sq: SquadId = spawn(&mut w, 0, card, from.x.floor_int(), from.y.floor_int());
        run(
            &mut w,
            0,
            1,
            CommandKind::Move {
                squad: sq,
                target: f.pos,
            },
        );
        let lead = members(&w, sq)[0];
        let mut off_at_row = None;
        for _ in 0..240 {
            steps(&mut w, 10);
            if !w.state.entities.is_alive(lead) {
                break;
            }
            let p = w.state.entities.transform[lead.index()].pos.xy();
            let v = p - f.pos;
            let along = (v.x * dir.x + v.y * dir.y).floor_int();
            if off_at_row.is_none() && along <= 55 {
                off_at_row = Some((v.x * dir.y - v.y * dir.x).abs().floor_int());
            }
            if along < 30 {
                break;
            }
        }
        let set_off = w.game().unwrap().fort().accounts[1].mines_set_off;
        (off_at_row, set_off)
    };
    let (off, set_off) = walk(|_| {});
    assert!(
        off.is_some_and(|o| o <= 15),
        "within 15 m of the lane at the row: {off:?}"
    );
    assert!(set_off >= 1, "a mine goes off");
    let (off, _) = walk(|s| s.rules.ai.obstacles.front_wire_len_m = 40);
    assert!(
        off.is_some_and(|o| o > 15),
        "control: wire 40 m, outside the row: {off:?}"
    );
}

/// The foot path (as `breach_path`; wire cost on or off) from `a` to `b`,
/// `a` first.
fn foot_path(w: &World, a: FxVec2, b: FxVec2, with_wire: bool) -> Vec<FxVec2> {
    let g = w.game().unwrap();
    let terrain = &w.state.terrain;
    let mv = &g.setup.rules.movement;
    let cost = crate::path::PathCost {
        slope_cost_pct: mv.slope_cost_pct,
        max_slope_pct: mv.max_slope_pct,
        trench_cost_pct: mv.trench_cost_pct,
        ditch_cost_pct: mv.ditch_cost_pct,
    };
    let (fs, forts) = (&g.fort, &g.setup.forts);
    let nodes = fs.path_nodes(forts, terrain.dim());
    let crosses = |p: FxVec2, q: FxVec2| fs.step_in_wire(forts, p, q);
    let wire = crate::path::WireCost {
        nodes: &nodes,
        cost_pct: g
            .setup
            .rules
            .fortify
            .as_ref()
            .map_or(100, |f| f.wire_path_cost_pct),
        crosses: &crosses,
    };
    let mut scratch = crate::path::PathScratch::default();
    let r = crate::path::find_foot_path(
        terrain,
        a,
        b,
        &cost,
        None,
        with_wire.then_some(wire),
        None,
        mv.path_max_expansions,
        &mut scratch,
    );
    assert!(r.reached);
    let mut way = vec![a];
    way.extend(r.points);
    way
}

/// The foot path with wire cost: its length (m) and whether a leg crosses
/// wire.
fn foot_way(w: &World, a: FxVec2, b: FxVec2) -> (i32, bool) {
    let g = w.game().unwrap();
    let way = foot_path(w, a, b, true);
    let len = crate::squad::polyline_length(&way).floor_int();
    let cross = way
        .windows(2)
        .any(|p| g.fort.step_in_wire(&g.setup.forts, p[0], p[1]));
    (len, cross)
}

/// F3: the own foot route from the front point to the nearest point it
/// does not hold (`route_out`) is at most 15 m longer with the front wire
/// standing, and does not cross it. Control: a 60 m wire 10 m out
/// (`front_wire_len_m` 60, `front_out_m` 10) -> longer by more than 15 m
/// or through the wire (seen failing).
#[test]
fn ai_front_wire_leaves_the_own_way_out() {
    let way = |edit: fn(&mut frontline_content::MatchSetup)| {
        let (mut w, f) = scene(edit);
        own_by(&mut w, f.near, Some(0));
        // The other of B and D (as far as `near`) is the AI's, so `near`
        // is the nearest point it does not hold.
        let other = if f.near == point(&w, "B") {
            point(&w, "D")
        } else {
            point(&w, "B")
        };
        own_by(&mut w, other, Some(1));
        play(&mut w, 3);
        let lane = lane_of(&w, f.own).expect("a lane");
        // `route_out`: the nearest other point not held by the AI.
        let g = w.game().unwrap();
        let to = (0..g.points.len())
            .filter(|&j| j != f.own && g.points[j].owner != Some(PlayerId(1)))
            .map(|j| pos_of(&w, j))
            .min_by_key(|&p| ((p - f.pos).length_sq_raw(), p.x.raw(), p.y.raw()))
            .unwrap();
        let (before, _) = foot_way(&w, f.pos, to);
        let wire = fort(&w, "ww2:barbed_wire");
        assert!(w.build_now(wire, &lane.wire, PlayerId(1)));
        let (after, cross) = foot_way(&w, f.pos, to);
        (before, after, cross)
    };
    let (before, after, cross) = way(|_| {});
    assert!(
        after <= before + 15 && !cross,
        "{before} -> {after} m, crosses {cross}"
    );
    let (before, after, cross) = way(|s| {
        s.rules.ai.obstacles.front_wire_len_m = 60;
        s.rules.ai.obstacles.front_out_m = 10;
    });
    assert!(
        after > before + 15 || cross,
        "control: {before} -> {after} m, crosses {cross}"
    );
}

// ------------------------------------------------- Build B chunk 2 --
// F5 (threat), F6 (drop and resume), F8 and the job ends (spec 5, first
// row), determinism. The enemy that threatens a front site is a player-0
// truck (a known enemy vehicle with no gun), so the builders are not shot
// and only the threat rule is seen.

const TRUCK: &str = "ww2:ger_opel_blitz_squad";

/// A player-0 truck parked in its own base; [`show`] moves it.
fn truck(w: &mut World) -> SimId {
    let eb = w.game().unwrap().base_of(PlayerId(0));
    let t = spawn(w, 0, TRUCK, eb.x.floor_int(), eb.y.floor_int());
    members(w, t)[0]
}

fn show(w: &mut World, v: SimId, at: FxVec2) {
    assert!(w.place_vehicle(v, at, Fx::ZERO));
}

/// Takes the truck away (to its base); a truck the builders shot to a
/// wreck is gone already.
fn hide(w: &mut World, v: SimId) {
    let eb = w.game().unwrap().base_of(PlayerId(0));
    let _ = w.place_vehicle(v, eb, Fx::ZERO);
}

/// Where squad `id`'s first living man stands.
fn lead_of(w: &World, id: SquadId) -> Option<FxVec2> {
    let g = w.game()?;
    let q = g.squads.get(&id)?;
    let m = q.members.iter().find(|m| w.state.entities.is_alive(**m))?;
    Some(w.state.entities.transform[m.index()].pos.xy())
}

/// The middle of a saved line.
fn mid2(l: [FxVec2; 2]) -> FxVec2 {
    (l[0] + l[1]).div_int(2)
}

/// Whether squad `id` holds a live FrontAp job (blueprint `ap`).
fn on_ap(w: &World, id: SquadId, ap: u16) -> bool {
    ai_ro(w)
        .forts
        .get(&id)
        .is_some_and(|j| j.fort == ap && j.phase != FortPhase::Done)
}

/// The front scene of C with `near` the enemy's, a builder with 120 kg at
/// C; plays until its FrontAp job runs and its leader is within 20 m of
/// the AP line's middle.
fn front_at_site(
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> (World, Front, FrontLane, SquadId, u16) {
    let (mut w, f) = scene(|s| {
        s.rules.ai.obstacles.at_mines = 0;
        s.rules.ai.fort.wire = 0;
        edit(s);
    });
    own_by(&mut w, f.near, Some(0));
    play(&mut w, 3);
    let lane = lane_of(&w, f.own).expect("a lane");
    let eng = squad_at(&mut w, ENG, f.pos, 120);
    let ap = fort(&w, "ww2:ap_minefield");
    let site = mid2(lane.ap);
    let at = play_until(&mut w, 120, |w| {
        on_ap(w, eng, ap) && lead_of(w, eng).is_some_and(|p| (p - site).length() < Fx::from_int(20))
    });
    assert!(at, "the builder reached the AP site on its job");
    (w, f, lane, eng, ap)
}

/// F5 (S10c-1 (A)): an enemy (a truck) 60 m beyond the AP site, 115 m from
/// the point, shown as the builder arrives: the FrontAp job is alive 60 s
/// later and has laid mines, `front_drops` 0. The same enemy there from
/// the start does not keep the job from starting (the builder's 80 m on
/// the walk stays clear of it). Control: `front_site_threat_m` 80 -> the
/// job is dropped (no refusal, `front_drops` 1; seen failing "alive").
#[test]
fn ai_front_job_survives_an_enemy_60_m_from_the_site() {
    let run = |site_m: u32| {
        let (mut w, f, lane, eng, ap) =
            front_at_site(|s| s.rules.ai.obstacles.front_site_threat_m = site_m);
        let site = mid2(lane.ap);
        let out = (site - f.pos).normalize_or_zero();
        let v = truck(&mut w);
        show(&mut w, v, site + out * Fx::from_int(60));
        let before = mines_on(&w, ap, lane.ap);
        play(&mut w, 60);
        let c = &ai_ro(&w).counters;
        (
            on_ap(&w, eng, ap) || lane_of(&w, f.own).is_some_and(|l| l.ap_done),
            mines_on(&w, ap, lane.ap) - before,
            c.front_drops,
            c.fort_job_fails,
        )
    };
    let (alive, laid, drops, fails) = run(50);
    assert!(alive, "the job is alive (or done) 60 s later");
    assert!(laid >= 3, "it laid {laid} mines meanwhile");
    assert_eq!((drops, fails), (0, 0));
    let (alive, _, drops, fails) = run(80);
    assert!(!alive, "control: site 80 m -> dropped");
    assert_eq!((drops, fails), (1, 0), "control: a drop, no refusal");
    // The enemy there from the start: the job still starts.
    let (mut w, f) = scene(|s| {
        s.rules.ai.obstacles.at_mines = 0;
        s.rules.ai.fort.wire = 0;
    });
    own_by(&mut w, f.near, Some(0));
    play(&mut w, 3);
    let lane = lane_of(&w, f.own).expect("a lane");
    let site = mid2(lane.ap);
    let v = truck(&mut w);
    show(
        &mut w,
        v,
        site + (site - f.pos).normalize_or_zero() * Fx::from_int(60),
    );
    let eng = squad_at(&mut w, ENG, f.pos, 120);
    let ap = fort(&w, "ww2:ap_minefield");
    assert!(
        play_until(&mut w, 30, |w| on_ap(w, eng, ap)),
        "the job starts with the enemy 60 m beyond the site"
    );
}

/// F6: an enemy shown 40 m from the site when the 6th mine is laid drops
/// the job (no refusal); it is taken away 60 s later and the builder takes
/// the rest of the line: 15 mines in the end, `front_drops` 1,
/// `front_resumes` 1, `fort_job_fails` 0. Control: the resume want taken
/// away (the lane marked `ap_done` at the drop, as a want blind to the
/// laid part) -> the field stays at 5-7 mines (seen failing "15").
#[test]
fn ai_front_job_resumes_after_a_threat_drop() {
    let run = |resume: bool| {
        let (mut w, f, lane, eng, ap) = front_at_site(|_| {});
        let site = mid2(lane.ap);
        let out = (site - f.pos).normalize_or_zero();
        let v = truck(&mut w);
        assert!(play_until(&mut w, 200, |w| mines_on(w, ap, lane.ap) >= 6));
        show(&mut w, v, site + out * Fx::from_int(40));
        assert!(
            play_until(&mut w, 10, |w| !on_ap(w, eng, ap)),
            "dropped by the enemy at 40 m"
        );
        let at_drop = mines_on(&w, ap, lane.ap);
        if !resume {
            ai_mut(&mut w)
                .front_lanes
                .iter_mut()
                .for_each(|l| l.ap_done = true);
        }
        play(&mut w, 60);
        hide(&mut w, v);
        play_until(&mut w, 400, |w| {
            lane_of(w, f.own).is_some_and(|l| l.ap_done) && !on_ap(w, eng, ap)
        });
        let c = &ai_ro(&w).counters;
        (
            at_drop,
            mines_on(&w, ap, lane.ap),
            c.front_drops,
            c.front_resumes,
            c.fort_job_fails,
        )
    };
    let (at_drop, end, drops, resumes, fails) = run(true);
    assert!((6..=7).contains(&at_drop), "{at_drop} mines at the drop");
    assert_eq!(end, 15, "the field finished");
    assert_eq!((drops, resumes, fails), (1, 1, 0));
    let (at_drop, end, _, resumes, _) = run(false);
    // The dropped squad's order may finish the mine in hand.
    assert!(
        end <= at_drop + 1 && end < 15,
        "control: no resume, {at_drop} -> {end} mines"
    );
    assert_eq!(resumes, 0);
}

/// F6: a laid front field (every site laid -> `ap_done`) whose mines are
/// set off later is not laid again: no FrontAp job in 60 s with a builder
/// carrying 120 kg at the point. Control: `ap_done` cleared after the
/// blasts -> the FrontAp job lays the three gaps (seen failing "no job").
#[test]
fn ai_front_field_set_off_is_not_relaid() {
    let run = |clear: bool| {
        let (mut w, f) = scene(|s| {
            s.rules.ai.obstacles.at_mines = 0;
            s.rules.ai.fort.wire = 0;
        });
        own_by(&mut w, f.near, Some(0));
        play(&mut w, 3);
        let lane = lane_of(&w, f.own).expect("a lane");
        let ap = fort(&w, "ww2:ap_minefield");
        assert!(w.build_now(ap, &lane.ap, PlayerId(1)));
        play(&mut w, 3);
        assert!(lane_of(&w, f.own).unwrap().ap_done, "every site laid");
        // The last three mines of the line go off (removed, as
        // `take_mines` leaves them), so a resume would lay just those.
        let mut ids: Vec<(i64, u32)> = w
            .game()
            .unwrap()
            .fort()
            .mines
            .iter()
            .filter(|(_, m)| m.owner == PlayerId(1))
            .map(|(&k, m)| ((m.pos - lane.ap[0]).length_sq_raw() as i64, k))
            .collect();
        ids.sort_unstable();
        let ids: Vec<u32> = ids.iter().rev().take(3).map(|&(_, k)| k).collect();
        w.edit_fort(|f| {
            for k in &ids {
                f.mines.remove(k);
            }
        });
        if clear {
            ai_mut(&mut w)
                .front_lanes
                .iter_mut()
                .for_each(|l| l.ap_done = false);
        }
        let eng = squad_at(&mut w, ENG, f.pos, 120);
        let mut job = false;
        for _ in 0..60 {
            play(&mut w, 1);
            job |= on_ap(&w, eng, ap);
        }
        play_until(&mut w, 120, |w| mines_on(w, ap, lane.ap) >= 15);
        (job, mines_on(&w, ap, lane.ap))
    };
    let (job, n) = run(false);
    assert!(!job, "no FrontAp job on a set-off field");
    assert_eq!(n, 12);
    let (job, n) = run(true);
    assert!(job && n == 15, "control: ap_done cleared -> relaid ({n})");
}

/// Spec 5, FrontAp "target gone": the point is lost mid-field: the job
/// ends, no refusal, the laid mines stay. Control: the point kept -> the
/// job is alive (seen failing "ended").
#[test]
fn ai_front_job_ends_when_the_point_is_lost() {
    let run = |lose: bool| {
        let (mut w, f, lane, eng, ap) = front_at_site(|_| {});
        assert!(play_until(&mut w, 200, |w| mines_on(w, ap, lane.ap) >= 4));
        let laid = mines_on(&w, ap, lane.ap);
        if lose {
            own_by(&mut w, f.own, Some(0));
        }
        play(&mut w, 8);
        let c = &ai_ro(&w).counters;
        (
            on_ap(&w, eng, ap),
            laid,
            mines_on(&w, ap, lane.ap),
            c.fort_job_fails,
            ai_ro(&w).refused.len(),
        )
    };
    let (alive, laid, now, fails, refused) = run(true);
    assert!(!alive, "the job ended");
    assert!(now >= laid, "the laid part stays: {laid} -> {now}");
    assert_eq!((fails, refused), (0, 0), "no refusal");
    let (alive, ..) = run(false);
    assert!(alive, "control: the point kept");
}

/// Spec 5, FrontAp "squad dead": the builder is wiped out mid-field: the
/// job ends with no refusal, `front_lost` 1, the mines stay, and a second
/// builder takes the rest (`front_resumes` 1, 15 mines). Control: one man
/// killed (strength over 60 %) -> the job goes on, `front_lost` 0 (seen
/// failing "ended").
#[test]
fn ai_front_builder_dies_mid_field() {
    let run = |kill_all: bool| {
        let (mut w, f, lane, eng, ap) = front_at_site(|_| {});
        assert!(play_until(&mut w, 200, |w| mines_on(w, ap, lane.ap) >= 5));
        let men = members(&w, eng);
        let n = if kill_all { men.len() } else { 1 };
        for &m in men.iter().take(n) {
            assert!(w.kill_soldier(m));
        }
        play(&mut w, 8);
        let alive = on_ap(&w, eng, ap);
        let lost = ai_ro(&w).counters.front_lost;
        let laid = mines_on(&w, ap, lane.ap);
        if kill_all {
            squad_at(&mut w, ENG, f.pos, 120);
            play_until(&mut w, 400, |w| {
                lane_of(w, f.own).is_some_and(|l| l.ap_done)
            });
        }
        let c = &ai_ro(&w).counters;
        (
            alive,
            lost,
            laid,
            mines_on(&w, ap, lane.ap),
            c.front_resumes,
            c.fort_job_fails,
        )
    };
    let (alive, lost, laid, end, resumes, fails) = run(true);
    assert!(!alive && lost == 1, "ended, front_lost {lost}");
    assert!(laid >= 5, "the laid part stays: {laid}");
    assert_eq!((end, resumes, fails), (15, 1, 0));
    let (alive, lost, ..) = run(false);
    assert!(alive && lost == 0, "control: one man lost, the job goes on");
}

/// F8: with `front_fields_max` 1 and `front_points` 1, the AI holds every
/// point but B (on the enemy's side, nearest the enemy base); C (the
/// front) holds a laid field. The AI takes B: B is now the front and C no
/// longer is, so C's lane entry goes and B's FrontAp is wanted: the first
/// check that keeps it from starting is `no_builder` (no squad there), not
/// `cap`; C's 15 mines stay. (A lost point is the same rule; the entry is
/// kept only while a front job lives.) Control: `front_points` 2 (B and C
/// both front) -> C's field fills the cap: `front_why` cap at B (seen
/// failing "no cap").
///
/// Deviation: the spec's "a new front point still gets its field" is shown
/// by the want, not a started job: with a builder placed at B, assign sent
/// it to C (the more central point, priority over distance) where it took
/// C's rear R1 field, whatever B's hold; the F8 rule itself is the lane
/// drop and the freed cap.
#[test]
fn ai_front_fields_survive_a_lost_point() {
    let run = |one: bool| {
        let (mut w, f) = scene(|s| {
            s.rules.ai.obstacles.at_mines = 0;
            s.rules.ai.fort.wire = 0;
            s.rules.ai.obstacles.front_fields_max = 1;
            s.rules.ai.obstacles.front_points = if one { 1 } else { 2 };
        });
        let b = point(&w, "B");
        for n in ["A", "D", "E"] {
            let i = point(&w, n);
            own_by(&mut w, i, Some(1));
        }
        play(&mut w, 3);
        let lane = lane_of(&w, f.own).expect("C's lane");
        let ap = fort(&w, "ww2:ap_minefield");
        assert!(w.build_now(ap, &lane.ap, PlayerId(1)));
        // Just before the next recheck plan (60 s).
        play(&mut w, 54);
        assert!(lane_of(&w, f.own).unwrap().ap_done);
        own_by(&mut w, b, Some(1));
        play(&mut w, 6);
        let before = ai_ro(&w).counters.front_why;
        play(&mut w, 30);
        let after = ai_ro(&w).counters.front_why;
        let grew = |k: &str| after[why(k)] - before[why(k)];
        (
            lane_of(&w, b).is_some_and(|l| !l.ap_done),
            grew("cap"),
            grew("no_builder"),
            mines_on(&w, ap, lane.ap),
            lane_of(&w, f.own).is_some(),
        )
    };
    let (b_lane, cap, none, n, c_lane) = run(true);
    assert!(b_lane, "B has its lane");
    assert!(!c_lane, "C's lane entry dropped");
    assert_eq!(cap, 0, "no cap at B");
    assert!(none >= 5, "B's field wanted, no builder there ({none})");
    assert_eq!(n, 15, "C's mines stay");
    let (b_lane, cap, _, _, c_lane) = run(false);
    assert!(b_lane && c_lane, "control: both lanes");
    assert!(
        cap >= 5,
        "control: C's field fills front_fields_max 1 ({cap})"
    );
}

/// The default skirmish (Kessel Ridge, both sides AI, `front` 1) at
/// `threads`.
fn skirmish(threads: usize) -> World {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let defs = frontline_content::ModDefs::load(&root).unwrap();
    let req = frontline_content::MatchRequest::skirmish(
        1,
        "ww2:germany",
        "ww2:usa",
        frontline_content::Difficulty::Normal,
    );
    let mut s = defs.resolve(&req).unwrap();
    s.players.iter_mut().for_each(|p| p.is_ai = true);
    assert_eq!(s.rules.ai.obstacles.front, 1, "front on by default");
    World::new_match(s, threads).unwrap()
}

fn hashes(w: &mut World, ticks: u64) -> Vec<u64> {
    let mut h = Vec::new();
    for _ in 0..ticks {
        w.step(&[]);
        if w.state.tick.is_multiple_of(30) {
            h.push(w.state_hash());
        }
    }
    h
}

/// Whether a live front job (FrontAp / FrontWire on its lane, or an AT
/// field at a lane's point) runs for either side.
fn front_job_live(w: &World) -> bool {
    let g = w.game().unwrap();
    g.ai.iter().flatten().any(|ai| {
        ai.forts.values().any(|j| {
            j.phase != FortPhase::Done
                && ai.front_lanes.iter().any(|l| {
                    l.point == j.point
                        && (j.line.last() == Some(&l.ap[1]) || j.line.last() == Some(&l.wire[1]))
                })
        })
    })
}

/// Determinism (spec 5): the default skirmish with front jobs gives the
/// same hashes at 1, 2, 3 and 8 threads over 12:30, and a save at the
/// first second from 5:00 with a front job live and a second one from the
/// end of its 30 s replay (Build C: seed 1 has none from 9:52 to 24:52;
/// both with lanes saved) loads and
/// plays on alike. Control: a loaded copy whose one saved lane is edited
/// (`ap_done` flipped) hashes differently at once (the comparison sees
/// the front state; seen).
#[test]
fn ai_front_deterministic_threads_and_save_load() {
    let five = 30 * 300;
    let twelve = 30 * 720;
    let mut base = skirmish(1);
    let mut want = hashes(&mut base, five);
    want.extend(hashes(&mut base, twelve - five + 30 * 30));
    let jobs: u32 = base
        .game()
        .unwrap()
        .ai
        .iter()
        .flatten()
        .map(|a| a.counters.front_jobs)
        .sum();
    assert!(jobs >= 2, "front jobs ran: {jobs}");
    for threads in [2, 3, 8] {
        let mut w = skirmish(threads);
        let mut h = hashes(&mut w, five);
        h.extend(hashes(&mut w, twelve - five + 30 * 30));
        assert_eq!(h, want, "at {threads} threads");
    }
    // The saves: at the first second from 5:00 (then from the end of the
    // first save's replay) with a front job live, within 15 minutes.
    let mut a = skirmish(1);
    let mut at = 0;
    // Build C: the second save looks from the end of the first one's 30 s
    // replay (seed 1 has no live front job from 9:52 to 24:52 now, so it
    // may be the same job, later).
    let mut stop = five;
    for _ in 0..2 {
        hashes(&mut a, stop.saturating_sub(at));
        at = at.max(stop);
        let mut s = 0;
        // Build C: the home point is no front point before the side is
        // pushed back (F1), so front jobs are rarer: 15 minutes to look.
        while !front_job_live(&a) && s < 900 {
            hashes(&mut a, 30);
            at += 30;
            s += 1;
        }
        assert!(front_job_live(&a), "a front job live from {} s", stop / 30);
        eprintln!("save at {} s", at / 30);
        let mut b = World::load(&a.save(), 1).unwrap();
        assert_eq!(a.state_hash(), b.state_hash(), "at the load");
        assert_eq!(ai_of(&a), ai_of(&b));
        assert!(ai_of(&b).iter().any(|ai| !ai.front_lanes.is_empty()));
        // Control: one lane edited -> another hash.
        let mut c = World::load(&a.save(), 1).unwrap();
        c.state
            .game
            .as_mut()
            .unwrap()
            .ai
            .iter_mut()
            .flatten()
            .find(|ai| !ai.front_lanes.is_empty())
            .unwrap()
            .front_lanes[0]
            .ap_done ^= true;
        assert_ne!(c.state_hash(), a.state_hash(), "control: a lane edit");
        let (ha, hb) = (hashes(&mut a, 30 * 30), hashes(&mut b, 30 * 30));
        assert_eq!(ha, hb, "after the load at {} s", stop / 30);
        at += 30 * 30;
        stop = at;
    }
}

fn ai_of(w: &World) -> Vec<AiState> {
    w.game().unwrap().ai.iter().flatten().cloned().collect()
}

// ------------------------------------------------ Build C chunk 1 (C1-C3) --

/// Player 1's live S10b mine jobs (any blueprint with a mine item).
fn mine_jobs(w: &World) -> usize {
    let s = w.match_setup().unwrap();
    ai_ro(w)
        .forts
        .values()
        .filter(|j| j.phase != FortPhase::Done && s.forts[j.fort as usize].mine_item.is_some())
        .count()
}

/// C1: `jobs_max` 2: at a held, trenched point (front off, wire off) two
/// builders lay the R1 AP field and the R2 AT field at once. Control:
/// `jobs_max` 1 -> never two mine jobs at once.
#[test]
fn ai_two_s10b_jobs_run_at_once() {
    let most = |jobs_max: u32| {
        let (mut w, h) = held(|s| {
            s.rules.ai.fort.wire = 0;
            s.rules.ai.fort.builders_max = 4;
            s.rules.ai.obstacles.jobs_max = jobs_max;
        });
        squad_at(&mut w, ENG, h.pos, 120);
        squad_at(&mut w, ENG, h.pos + c(4, 0), 120);
        let mut most = 0;
        for _ in 0..180 {
            play(&mut w, 1);
            most = most.max(mine_jobs(&w));
        }
        most
    };
    assert_eq!(most(2), 2, "jobs_max 2: two mine jobs at once");
    assert_eq!(most(1), 1, "control: jobs_max 1");
}

/// The front jobs (by their lane's lines, as `front_kind`) and the other
/// live jobs of player 1 at point `i`.
fn front_and_home(w: &World, i: usize) -> (usize, usize) {
    let Some(lane) = lane_of(w, i) else {
        return (0, 0);
    };
    let s = w.match_setup().unwrap();
    let (ap, wire) = (fort(w, "ww2:ap_minefield"), fort(w, "ww2:barbed_wire"));
    let r = Fx::from_int(2);
    let (mut front, mut home) = (0, 0);
    for j in ai_ro(w)
        .forts
        .values()
        .filter(|j| j.phase != FortPhase::Done)
    {
        let end = j.line.last().copied().unwrap_or_default();
        let is_front = (j.fort == ap && (end - lane.ap[1]).length() <= r)
            || (j.fort == wire && (end - lane.wire[1]).length() <= r)
            || s.forts[j.fort as usize].id == "ww2:at_minefield";
        if is_front {
            front += 1;
        } else {
            home += 1;
        }
    }
    (front, home)
}

/// C8: two builders at a held, trenched point that is a front point
/// (`front_home` 1), S10a nest on, `jobs_max` 2: with `front_slots` 1 one
/// takes a front job and the other the nest (a home work) while it runs;
/// never two front jobs. Control: `front_slots` 2 -> both slots at the
/// front (FrontAp and FrontWire at once).
#[test]
fn ai_front_jobs_leave_a_slot_for_home_works() {
    let run = |slots: u32| {
        let (mut w, h) = held(|s| {
            s.rules.ai.obstacles.front = 1;
            s.rules.ai.obstacles.front_home = 1;
            s.rules.ai.obstacles.front_slots = slots;
            s.rules.ai.obstacles.at_mines = 0;
            s.rules.ai.obstacles.jobs_max = 2;
            s.rules.ai.fort.builders_max = 4;
            let f = &mut s.rules.ai.fort;
            f.works = 1;
            f.nest = 1;
            f.dugout = 0;
            f.roof = 0;
            f.sandbags = 0;
        });
        squad_at(&mut w, ENG, h.pos, 120);
        squad_at(&mut w, ENG, h.pos + c(4, 0), 120);
        let (mut most_front, mut both) = (0, false);
        for _ in 0..120 {
            play(&mut w, 1);
            let (front, home) = front_and_home(&w, 0);
            most_front = most_front.max(front);
            both |= front == 1 && home >= 1;
        }
        (most_front, both)
    };
    let (most, both) = run(1);
    assert_eq!(most, 1, "front_slots 1: one front job at a time");
    assert!(both, "a home work runs beside the front job");
    let (most, _) = run(2);
    assert_eq!(most, 2, "control: front_slots 2 -> both slots at the front");
}

/// C2: AI player 1 owns C (front) and `near` (its home point, not front:
/// the enemy owns no point). A builder at `near` is assigned there (the
/// dig bonus at every own untrenched point, and `near` is 300 m closer);
/// with `mine_assign` 1 it takes the FrontAp job at C anyway. Control:
/// `mine_assign` 0 -> no front job (seen failing as "the FrontAp job").
#[test]
fn ai_mine_assign_sends_a_builder_to_the_front() {
    let run = |on: u32| {
        let (mut w, f) = scene(|s| {
            s.rules.ai.obstacles.at_mines = 0;
            s.rules.ai.obstacles.mine_assign = on;
        });
        own_by(&mut w, f.near, Some(1));
        let at = pos_of(&w, f.near);
        let eng = squad_at(&mut w, ENG, at, 120);
        let ap = fort(&w, "ww2:ap_minefield");
        let ok = play_until(&mut w, 60, |w| {
            ai_ro(w)
                .forts
                .get(&eng)
                .is_some_and(|j| j.fort == ap && j.point as usize == f.own)
        });
        let task = ai_ro(&w).tasks.get(&eng).map(|t| t.point as usize);
        (ok, task, ai_ro(&w).counters.front_why, f)
    };
    let (ok, _, fw, _) = run(1);
    assert!(ok, "mine_assign 1: the FrontAp job at C ({fw:?})");
    let (ok, task, fw, f) = run(0);
    assert!(!ok, "control: mine_assign 0 -> no front job");
    assert_eq!(task, Some(f.near), "control: assigned to the home point");
    assert!(fw[why("no_builder")] > 0, "control: no_builder {fw:?}");
}

/// AI player 1 (USA, buys on: rifle squads weight 100, engineers 1; the
/// mix keeps only the `line` slot, so the weighted pick never buys an
/// engineer) owns C, no start squads; `min` = `builder_cards_min`.
fn buy_world(min: u32) -> World {
    let mut w = flat_world_with(1, |s| {
        let line = s.tag("line").unwrap();
        s.rules.ai.mix.retain(|m| m.tag == line);
        s.players[1].is_ai = true;
        let f = s.players[1].faction as usize;
        s.factions[f].start_squads.clear();
        for t in s.squad_types.iter_mut() {
            t.ai_weight = match t.id.as_str() {
                "ww2:usa_rifle_squad" => 100,
                ENG => 1,
                _ => 0,
            };
        }
        s.rules.ai.obstacles.builder_cards_min = min;
    });
    let cc = point(&w, "C");
    w.state.game.as_mut().unwrap().points[cc].owner = Some(PlayerId(1));
    w
}

fn bought(w: &World, card: &str) -> u32 {
    let t = w.match_setup().unwrap().squad_type_index(card).unwrap();
    ai_ro(w)
        .counters
        .bought
        .get(t as usize)
        .copied()
        .unwrap_or(0)
}

/// Player 1's living engineer squads.
fn engineers(w: &World) -> Vec<SquadId> {
    let t = w.match_setup().unwrap().squad_type_index(ENG).unwrap();
    w.game()
        .unwrap()
        .squads
        .iter()
        .filter(|(_, q)| q.owner == PlayerId(1) && q.type_idx == t && !q.members.is_empty())
        .map(|(&id, _)| id)
        .collect()
}

/// C3: `builder_cards_min` 2 with no builder alive: the first buys are
/// two engineer squads (`builder_buys` 2) before any weighted pick.
/// Control: 0 -> the weighted pick (rifle squads; no builder buy).
#[test]
fn ai_buys_a_builder_under_the_floor() {
    let any = |w: &World| ai_ro(w).counters.bought.iter().sum::<u32>() > 0;
    let mut w = buy_world(2);
    assert!(play_until(&mut w, 30, any), "a buy");
    assert_eq!(bought(&w, ENG), 1, "the first buy is the engineer card");
    // The card's cooldown: the second engineer when it is ready.
    play_until(&mut w, 300, |w| bought(w, ENG) >= 2);
    assert_eq!(bought(&w, ENG), 2, "two engineer buys");
    assert_eq!(ai_ro(&w).counters.builder_buys, 2);
    let mut w = buy_world(0);
    assert!(play_until(&mut w, 30, any), "control: a buy");
    assert_eq!(
        (bought(&w, ENG), ai_ro(&w).counters.builder_buys),
        (0, 0),
        "control: builder_cards_min 0 -> the weighted pick"
    );
    assert!(bought(&w, "ww2:usa_rifle_squad") > 0, "control: rifles");
}

/// C3, job table "Builder buy": a builder squad killed after the buy -> one
/// more buy; living and pending engineers never above the floor (2).
#[test]
fn ai_builder_floor_never_overbuys() {
    let mut w = buy_world(2);
    let mut most = 0;
    let arrived = play_until(&mut w, 300, |w| engineers(w).len() >= 2);
    assert!(arrived, "two engineers arrived");
    for _ in 0..60 {
        play(&mut w, 1);
        most = most.max(engineers(&w).len());
    }
    assert_eq!(bought(&w, ENG), 2, "no buy over the floor");
    let dead = engineers(&w)[0];
    for m in members(&w, dead) {
        w.kill_soldier(m);
    }
    let again = play_until(&mut w, 300, |w| bought(w, ENG) >= 3);
    assert!(again, "one more buy after a loss");
    for _ in 0..300 {
        play(&mut w, 1);
        most = most.max(engineers(&w).len());
    }
    assert_eq!(bought(&w, ENG), 3, "exactly one more");
    assert!(most <= 2, "never above the floor: {most}");
    assert_eq!(ai_ro(&w).counters.builder_buys, 3);
}
