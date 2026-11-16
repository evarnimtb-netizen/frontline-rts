//! S10b-ai tests (docs/slices/S10b-ai.md R5, R6, spec 5 "site taken";
//! Build C chunk 1): the AI builds a log barrier before a gun in Hold and a
//! hedgehog row before a built nest, never across its own vehicle route.

use super::s6_tests::{run, spawn, steps};
use super::s10b_ai_lay_tests::{Held, held, squad_at};
use super::*;
use crate::ai::{AiGun, AiState, FortPhase, GunPhase};
use frontline_content::setup::Capability;

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

fn play(w: &mut World, secs: u64) {
    steps(w, secs * 30);
}

fn ai_ro(w: &World) -> &AiState {
    w.game().unwrap().ai[1].as_ref().unwrap()
}

fn ai_mut(w: &mut World) -> &mut AiState {
    w.state.game.as_mut().unwrap().ai[1].as_mut().unwrap()
}

fn play_until(w: &mut World, secs: u64, mut done: impl FnMut(&World) -> bool) -> bool {
    for _ in 0..secs {
        if done(w) {
            return true;
        }
        play(w, 1);
    }
    done(w)
}

/// Lines of player 1's AI jobs on blueprint `f`.
fn job_lines(w: &World, f: u16) -> Vec<Vec<FxVec2>> {
    ai_ro(w)
        .forts
        .values()
        .filter(|j| j.fort == f)
        .map(|j| j.line.clone())
        .collect()
}

/// Distance (m, rounded down) of `p` from the line through `a` along unit `d`.
fn off_m(p: FxVec2, a: FxVec2, d: FxVec2) -> i32 {
    let v = p - a;
    (v.x * d.y - v.y * d.x).abs().floor_int()
}

/// Puts a player 1 AT gun in Hold at `at` facing `facing` (as the S10a
/// sandbag test).
pub(crate) fn gun_in_hold(w: &mut World, at: FxVec2, facing: FxVec2) -> crate::game::SquadId {
    let gsq = squad_at(w, "ww2:usa_57mm_m1_squad", at, 0);
    let tick = w.state.tick;
    let gpos = {
        let gun = w.game().unwrap().squads[&gsq].members[0];
        w.state.entities.transform[gun.index()].pos.xy()
    };
    ai_mut(w).guns.insert(
        gsq,
        AiGun {
            phase: GunPhase::Hold,
            site: gpos,
            facing: gpos + (facing - at),
            moved_to: gpos,
            moved_at: tick,
            quiet_at: tick,
            pit: true,
            ..Default::default()
        },
    );
    gsq
}

/// The held point with only the obstacle habits on (no mines, no S10a
/// works or wire), a gun in Hold at once.
pub(crate) fn obst_world(edit: impl FnOnce(&mut frontline_content::MatchSetup)) -> (World, Held) {
    held(|s| {
        s.rules.ai.fort.wire = 0;
        s.rules.ai.fort.gun_hold_s = 0;
        s.rules.ai.obstacles.mines = 0;
        edit(s);
    })
}

/// R5: an own gun in Hold gets a 20 m log barrier across its arc
/// `obst_gun_m` (40) out. Control: `obstacles` 0 -> no barrier job (seen
/// failing as "a barrier job" when asserted with the switch off).
#[test]
fn ai_builds_barrier_before_gun() {
    let run = |obstacles: u32| {
        let (mut w, h) = obst_world(|s| s.rules.ai.obstacles.obstacles = obstacles);
        let at = h.pos + c(20, 0);
        let gsq = gun_in_hold(&mut w, at, at + h.enemy * Fx::from_int(100));
        squad_at(&mut w, ENG, at + c(-15, 0), 120);
        let bar = fort(&w, "ww2:log_barrier");
        let ok = play_until(&mut w, 60, |w| !job_lines(w, bar).is_empty());
        let j = ai_ro(&w).guns[&gsq];
        (ok, job_lines(&w, bar), j.site, j.facing)
    };
    let (ok, lines, site, facing) = run(1);
    assert!(ok, "a barrier job");
    let fwd = (facing - site).normalize_or_zero();
    let l = &lines[0];
    let mid = (l[0] + l[1]).div_int(2);
    let out = (mid - site).dot(fwd).floor_int();
    assert!((39..=40).contains(&out), "40 m out: {out}");
    assert_eq!(off_m(mid, site, fwd), 0, "centred on the arc");
    let len = (l[1] - l[0]).length().floor_int();
    assert!((19..=20).contains(&len), "20 m long: {len}");
    assert!(
        (l[1] - l[0]).normalize_or_zero().dot(fwd).abs() < Fx::from_milli(50),
        "across the arc"
    );
    let (ok, ..) = run(0);
    assert!(!ok, "control: obstacles 0 -> no barrier job");
}

/// R5: with the point's nest built, a builder (US engineers given
/// `build_heavy`) builds the `hedgehogs_row` (3) hedgehogs 4 m apart, 20 m
/// in front of the nest, one job at a time; the row's sites stay fixed.
/// Control: no nest built -> no hedgehog (spec's `hedgehogs_row` 0 is out
/// of the key's range 1..=16; seen failing as "the row stands" with the
/// control asserted to build).
#[test]
fn ai_builds_hedgehogs_before_nest() {
    let run = |with_nest: bool| {
        let (mut w, h) = obst_world(|s| {
            for t in s.squad_types.iter_mut().filter(|t| t.id == ENG) {
                t.caps |= Capability::BuildHeavy.bit();
            }
        });
        let s = w.match_setup().unwrap().clone();
        let off = Fx::from_int(s.rules.ai.fort.nest_offset_m as i32);
        let [a, b] = h.line;
        let nest = b + (b - a).normalize_or_zero() * off;
        let nf = fort(&w, "ww2:mg_nest");
        if with_nest {
            assert!(w.build_now(nf, &[nest, nest + h.enemy * Fx::from_int(10)], PlayerId(1)));
        }
        squad_at(&mut w, ENG, h.pos, 120);
        let hh = fort(&w, "ww2:hedgehog");
        let first = nest + h.enemy * Fx::from_int(s.rules.ai.obstacles.obst_work_m as i32);
        let sites: Vec<FxVec2> = (0..3)
            .map(|k| first + h.along * Fx::from_int(4 * k))
            .collect();
        let mut max_jobs = 0;
        let built = |w: &World| -> Vec<FxVec2> {
            w.game()
                .unwrap()
                .fort()
                .pieces
                .values()
                .filter(|q| q.owner == PlayerId(1) && q.fort == hh)
                .map(|q| q.pos)
                .collect()
        };
        for _ in 0..1200 {
            play(&mut w, 1);
            let live = ai_ro(&w)
                .forts
                .values()
                .filter(|j| j.fort == hh && j.phase != FortPhase::Done)
                .count();
            max_jobs = max_jobs.max(live);
            for l in job_lines(&w, hh) {
                assert!(
                    sites.iter().any(|p| (l[0] - *p).length() < Fx::from_int(1)),
                    "a job off the row: {:?} (row {sites:?})",
                    l[0]
                );
            }
            if built(&w).len() >= 3 {
                break;
            }
        }
        (built(&w), sites, max_jobs)
    };
    let (got, sites, max_jobs) = run(true);
    assert_eq!(got.len(), 3, "the row stands: {got:?}");
    for p in &sites {
        assert!(
            got.iter().any(|q| (*q - *p).length() < Fx::from_int(1)),
            "a hedgehog at {p:?}: {got:?}"
        );
    }
    assert_eq!(max_jobs, 1, "one job at a time");
    let (got, _, max_jobs) = run(false);
    assert!(got.is_empty() && max_jobs == 0, "control: no nest, no row");
}

/// Fix 15: R6's mirrored side keeps its own sites. A hedgehog already on
/// side 1's first site (one step toward the trench's middle) fixes the row
/// to side 1: every later piece goes on that side, none at or beyond side
/// 0's first site. Control: before the fix side 1 started at side 0's
/// first site, so the row went on along side 0 (seen failing as "a piece
/// on side 0").
#[test]
fn hedgehog_row_keeps_its_mirrored_side() {
    let (mut w, h) = obst_world(|s| {
        for t in s.squad_types.iter_mut().filter(|t| t.id == ENG) {
            t.caps |= Capability::BuildHeavy.bit();
        }
    });
    let s = w.match_setup().unwrap().clone();
    let off = Fx::from_int(s.rules.ai.fort.nest_offset_m as i32);
    let [a, b] = h.line;
    let nest = b + (b - a).normalize_or_zero() * off;
    let nf = fort(&w, "ww2:mg_nest");
    assert!(w.build_now(nf, &[nest, nest + h.enemy * Fx::from_int(10)], PlayerId(1)));
    let hh = fort(&w, "ww2:hedgehog");
    let first = nest + h.enemy * Fx::from_int(s.rules.ai.obstacles.obst_work_m as i32);
    let s1 = first - h.along * Fx::from_int(4);
    assert!(w.build_now(hh, &[s1, s1 + h.enemy], PlayerId(1)));
    squad_at(&mut w, ENG, h.pos, 120);
    let built = |w: &World| -> Vec<FxVec2> {
        w.game()
            .unwrap()
            .fort()
            .pieces
            .values()
            .filter(|q| q.owner == PlayerId(1) && q.fort == hh)
            .map(|q| q.pos)
            .collect()
    };
    for _ in 0..1200 {
        play(&mut w, 1);
        for l in job_lines(&w, hh) {
            assert!(
                (l[0] - first).dot(h.along) < Fx::from_int(-3),
                "a piece on side 0: {:?}",
                l[0]
            );
        }
        if built(&w).len() >= 3 {
            break;
        }
    }
    let got = built(&w);
    assert_eq!(got.len(), 3, "the row stands: {got:?}");
    for q in &got {
        assert!(
            (*q - first).dot(h.along) < Fx::from_int(-3),
            "a piece on side 0: {q:?}"
        );
    }
}

/// R6: a gun on the own route 60 m before the point, its arc along the
/// route: the barrier across the route is rejected, the mirrored one (20 m
/// aside) is still within `own_route_m` + `wreck_pad_m` -> refused
/// (`own_route_rejects`, an L3 refusal, no job). Control: `own_route_m` 0
/// -> the barrier job lies across the route (seen failing as "refused").
#[test]
fn ai_never_blocks_own_route() {
    let run = |route_m: u32| {
        let (mut w, h) = obst_world(|s| s.rules.ai.obstacles.own_route_m = route_m);
        // The own vehicle route's last leg runs along x into the point
        // on this flat map (base (928, 928), point (220, 700)).
        let (base, dir) = (h.pos, c(1, 0));
        let at = h.pos + c(60, 0);
        gun_in_hold(&mut w, at, h.pos);
        squad_at(&mut w, ENG, at + c(0, 10), 120);
        let bar = fort(&w, "ww2:log_barrier");
        play(&mut w, 60);
        let ai = ai_ro(&w);
        let on_route = job_lines(&w, bar).iter().any(|l| {
            off_m(l[0], base, dir) <= route_m as i32 + 2
                || off_m(l[1], base, dir) <= route_m as i32 + 2
                || (off_m(l[0], base, dir) + off_m(l[1], base, dir)
                    > off_m((l[0] + l[1]).div_int(2), base, dir) * 2 + 2)
        });
        (
            ai.counters.own_route_rejects,
            job_lines(&w, bar).len(),
            on_route,
            ai.refused.len(),
        )
    };
    let (rejects, jobs, on_route, refused) = run(10);
    assert!(rejects >= 1 && refused >= 1, "refused: {rejects} {refused}");
    assert!(jobs == 0 || !on_route, "no barrier on the own route");
    assert_eq!(rejects, 1, "one rejection in 60 s (L4)");
    let (rejects, jobs, on_route, _) = run(0);
    assert!(
        rejects == 0 && jobs == 1 && on_route,
        "control: own_route_m 0 -> on the route ({rejects}, {jobs}, {on_route})"
    );
}

/// T115 `own_blocked`: an own tank ordered onto its own barrier (the goal
/// inside the blocking piece) fails its move beside it -> counted once per
/// (vehicle, piece). Control: the barrier is player 0's -> 0 (seen failing
/// with the owner check removed).
#[test]
fn ai_counts_own_blocked() {
    let go = |owner: u8| {
        let (mut w, h) = obst_world(|_| {});
        let bar = fort(&w, "ww2:log_barrier");
        let p = h.pos + c(-60, 30);
        assert!(w.build_now(bar, &[p, p + c(4, 0)], PlayerId(owner)));
        let t = spawn(
            &mut w,
            1,
            "ww2:usa_m4a3_75_squad",
            p.x.floor_int() + 9,
            p.y.floor_int(),
        );
        let goal = p + c(2, 0);
        run(
            &mut w,
            1,
            1,
            crate::command::CommandKind::Move {
                squad: t,
                target: goal,
            },
        );
        play(&mut w, 30);
        ai_ro(&w).counters.own_blocked
    };
    let n = go(1);
    assert!(n >= 1, "own_blocked counted: {n}");
    assert_eq!(go(0), 0, "control: an enemy barrier");
}

/// Fix 6/16: a tank halted beside its own barrier by a Stop (as the AI's
/// own Stops and a target acquired on a Move do: path cleared, goal kept)
/// is not blocked: its goal is still reachable -> 0. Control: the same tank
/// ordered into the barrier -> counted (`ai_counts_own_blocked`). Before
/// the fix this read 1.
#[test]
fn own_blocked_ignores_a_halt_beside_an_own_piece() {
    for dy in [5, -6] {
        let (mut w, h) = obst_world(|_| {});
        let bar = fort(&w, "ww2:log_barrier");
        let p = h.pos + c(-60, 30);
        assert!(w.build_now(bar, &[p, p + c(4, 0)], PlayerId(1)));
        let t = spawn(
            &mut w,
            1,
            "ww2:usa_m4a3_75_squad",
            p.x.floor_int() + 2,
            p.y.floor_int() + dy,
        );
        let mv = crate::command::CommandKind::Move {
            squad: t,
            target: p + c(70, dy),
        };
        run(&mut w, 1, 1, mv);
        play(&mut w, 1);
        run(&mut w, 1, 1, crate::command::CommandKind::Stop { squad: t });
        play(&mut w, 30);
        assert_eq!(
            ai_ro(&w).counters.own_blocked,
            0,
            "a halt is no block ({dy})"
        );
    }
}

/// Build D: a pair stuck at the save is not counted again after the load
/// (`AiState::blocked_pairs` is saved). The tank of `ai_counts_own_blocked`
/// stays stuck by its own barrier; saved after its first count, the loaded
/// world counts what the unsaved one counts. Control: `#[serde(skip)]` on
/// `blocked_pairs` -> the loaded world counts the pair again (seen).
#[test]
fn own_blocked_counts_once_across_save_load() {
    let (mut w, h) = obst_world(|_| {});
    let bar = fort(&w, "ww2:log_barrier");
    let p = h.pos + c(-60, 30);
    assert!(w.build_now(bar, &[p, p + c(4, 0)], PlayerId(1)));
    let t = spawn(
        &mut w,
        1,
        "ww2:usa_m4a3_75_squad",
        p.x.floor_int() + 9,
        p.y.floor_int(),
    );
    run(
        &mut w,
        1,
        1,
        crate::command::CommandKind::Move {
            squad: t,
            target: p + c(2, 0),
        },
    );
    for _ in 0..60 {
        if ai_ro(&w).counters.own_blocked > 0 {
            break;
        }
        play(&mut w, 1);
    }
    let n = ai_ro(&w).counters.own_blocked;
    assert!(
        n >= 1 && !ai_ro(&w).blocked_pairs.is_empty(),
        "stuck at the save: {n}"
    );
    let mut b = World::load(&w.save(), 1).unwrap();
    assert_eq!(ai_ro(&b), ai_ro(&w), "loaded AI state");
    play(&mut w, 20);
    play(&mut b, 20);
    assert_eq!(
        ai_ro(&b).counters.own_blocked,
        ai_ro(&w).counters.own_blocked,
        "counted alike after the load"
    );
}

/// Job table [C] "site taken": another work (player 0's hedgehog, built at
/// once) lands on a watched AP field's site -> the job ends. Control: the site check in Watch skipped -> the
/// job goes on (seen failing with the check commented out).
#[test]
fn ai_s10b_site_taken_ends_job() {
    let (mut w, h) = held(|s| {
        s.rules.ai.fort.wire = 0;
        s.rules.ai.obstacles.at_mines = 0;
    });
    let eng = squad_at(&mut w, ENG, h.pos, 120);
    let ap = fort(&w, "ww2:ap_minefield");
    let watching = |w: &World| {
        ai_ro(w)
            .forts
            .get(&eng)
            .is_some_and(|j| j.phase == FortPhase::Watch && j.fort == ap)
    };
    assert!(
        play_until(&mut w, 300, watching),
        "the job watches its work"
    );
    let l = ai_ro(&w).forts[&eng].line.clone();
    let mid = (l[0] + l[l.len() - 1]).div_int(2);
    let hh = fort(&w, "ww2:hedgehog");
    assert!(
        w.build_now(hh, &[mid, mid + c(1, 0)], PlayerId(0)),
        "a hedgehog lands"
    );
    play(&mut w, 10);
    assert!(
        !ai_ro(&w)
            .forts
            .get(&eng)
            .is_some_and(|j| j.phase != FortPhase::Done),
        "the job ended when its site was taken"
    );
}

/// Build E item 2 (S10b-ai-C1): a log barrier job's L1 deadline is
/// `barrier_job_max_s`, not `job_max_s`: with `job_max_s` 40 and
/// `barrier_job_max_s` 1200 the barrier job lives 60 s on; with them the
/// other way round it ends by then (a failure: one refusal). Control: both
/// 40 (the barrier on `job_max_s` as before) -> it ends (seen failing as
/// "lives past job_max_s" with the control asserted to live).
#[test]
fn barrier_job_deadline_is_its_own() {
    let run = |job_s: u32, barrier_s: u32| {
        let (mut w, h) = obst_world(|s| {
            s.rules.ai.obstacles.job_max_s = job_s;
            s.rules.ai.obstacles.barrier_job_max_s = barrier_s;
        });
        let at = h.pos + c(20, 0);
        gun_in_hold(&mut w, at, at + h.enemy * Fx::from_int(100));
        squad_at(&mut w, ENG, at + c(-15, 0), 120);
        let bar = fort(&w, "ww2:log_barrier");
        assert!(
            play_until(&mut w, 60, |w| !job_lines(w, bar).is_empty()),
            "a barrier job"
        );
        let refused = ai_ro(&w).counters.s10b_refused;
        play(&mut w, 60);
        let alive = !job_lines(&w, bar).is_empty();
        (alive, ai_ro(&w).counters.s10b_refused - refused)
    };
    let (alive, refused) = run(40, 1200);
    assert!(alive, "the barrier job lives past job_max_s");
    assert_eq!(refused, 0);
    let (alive, refused) = run(1200, 40);
    assert!(!alive, "the barrier job ends at barrier_job_max_s");
    assert_eq!(refused, 1, "one refusal");
    let (alive, _) = run(40, 40);
    assert!(!alive, "control: both 40 -> it ends");
}

/// S10c-ai C6 (B1-2): the tank of `ai_counts_own_blocked` stuck at its own
/// barrier (counted once); then taken 60 m off (placed there, holding at
/// its goal: the pair clears) for 5 s, and put back stuck as it was (its
/// position and move restored): the same (vehicle, piece) pair is not
/// counted again (`own_blocked` 1). Control: the old code (the saved list
/// replaced by each plan's pairs) counts it again, 2 (seen failing, code
/// edit).
#[test]
fn own_blocked_pair_is_counted_once() {
    let (mut w, h) = obst_world(|_| {});
    let bar = fort(&w, "ww2:log_barrier");
    let p = h.pos + c(-60, 30);
    assert!(w.build_now(bar, &[p, p + c(4, 0)], PlayerId(1)));
    let t = spawn(
        &mut w,
        1,
        "ww2:usa_m4a3_75_squad",
        p.x.floor_int() + 9,
        p.y.floor_int(),
    );
    let mv = crate::command::CommandKind::Move {
        squad: t,
        target: p + c(2, 0),
    };
    run(&mut w, 1, 1, mv);
    play_until(&mut w, 30, |w| ai_ro(w).counters.own_blocked > 0);
    assert_eq!(ai_ro(&w).counters.own_blocked, 1, "stuck: counted");
    assert!(!ai_ro(&w).blocked_pairs.is_empty());
    let v = w.game().unwrap().squads[&t].members[0];
    let (at, yaw) = {
        let tr = &w.state.entities.transform[v.index()];
        (tr.pos.xy(), tr.yaw)
    };
    let nav = w.game().unwrap().squads[&t].nav.clone();
    let away = at + c(60, 0);
    assert!(w.place_vehicle(v, away, yaw));
    {
        let q = w.state.game.as_mut().unwrap().squads.get_mut(&t).unwrap();
        q.nav.goal = away;
        q.nav.path.clear();
    }
    play(&mut w, 5);
    assert!(w.place_vehicle(v, at, yaw));
    w.state
        .game
        .as_mut()
        .unwrap()
        .squads
        .get_mut(&t)
        .unwrap()
        .nav = nav;
    play(&mut w, 5);
    assert_eq!(
        ai_ro(&w).counters.own_blocked,
        1,
        "the same pair again is not new"
    );
}
