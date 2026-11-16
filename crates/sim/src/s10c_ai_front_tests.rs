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
        let (mut w, h) = held(|s| {
            s.rules.ai.fort.works = 1;
            s.rules.ai.obstacles.front = front;
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
        let (mut w, f) = scene(|s| {
            s.map.points.retain(|p| p.name == "C");
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
