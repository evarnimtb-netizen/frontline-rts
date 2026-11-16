//! S10c-ai tests (docs/slices/S10c-ai.md 3, 6; Build A chunk 1): the saved
//! counters, the refusal split, the front points (F1) with their plan
//! counters, and the mine-want reasons (review finding 12). Nothing here
//! changes what the AI does.

use super::s6_tests::{flat_world_with, spawn, steps};
use super::s10b_ai_lay_tests::{Held, held, squad_at};
use super::s10b_ai_tests::{Stage, fort_row, truck_row};
use super::*;
use crate::ai::fort::front_points_of;
use crate::ai::{AiCounters, AiState, FrontLane, MAX_FRONT_LANES, MINE_WANT_WHY};

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

/// The three counters of the split and `s10b_refused`.
fn split(c: &AiCounters) -> [u32; 4] {
    [
        c.fort_job_fails,
        c.truck_give_ups,
        c.route_refusals,
        c.s10b_refused,
    ]
}

/// Drives a liveness row's habit on its caller-kept AI state until `done`
/// or 20 min; returns the state.
fn drive_until(w: &mut World, stage: &Stage, done: impl Fn(&AiCounters) -> bool) -> AiState {
    let mut ai = stage
        .ai
        .clone()
        .unwrap_or_else(|| AiState::new(w.state.tick));
    for _ in 0..20 * 60 * 30 {
        w.drive_ai_habit(&mut ai, PlayerId(1), stage.habit, &stage.args);
        if done(&ai.counters) {
            break;
        }
    }
    ai
}

/// A failed S10b fort job (the Fort liveness row), a remembered truck
/// give-up (the TruckGoal row, L5) and an R6 own-route reject (the scene
/// of `ai_never_blocks_own_route`) each count in their own field and once
/// in `s10b_refused`: 1/1/1 and 3. (Each needs its own staged world; the
/// sum is over the three.) Control: `refusal_memory` 0 -> `truck_give_ups`
/// 0, so the sum is 1/0/1 and 2 (the main assertion fails, seen).
#[test]
fn s10c_refusal_split_adds_up() {
    let scenes = |memory: u32| {
        // A failed fort job.
        let (mut w, stage) = fort_row();
        let fort = drive_until(&mut w, &stage, |c| c.fort_job_fails >= 1);
        // A truck give-up.
        let (mut w, stage) = truck_row();
        w.state
            .game
            .as_mut()
            .unwrap()
            .setup
            .rules
            .ai
            .truck
            .refusal_memory = memory;
        let truck = drive_until(&mut w, &stage, |c| c.truck_give_ups >= 1);
        // An R6 reject: a log barrier site on the own vehicle route.
        let (mut w, h) = super::s10b_ai_obst_tests::obst_world(|_| {});
        let at = h.pos + c(60, 0);
        super::s10b_ai_obst_tests::gun_in_hold(&mut w, at, h.pos);
        squad_at(&mut w, "ww2:usa_engineer_squad", at + c(0, 10), 120);
        steps(&mut w, 60 * 30);
        let route = ai_ro(&w).clone();
        [
            split(&fort.counters),
            split(&truck.counters),
            split(&route.counters),
        ]
    };
    let [f, t, r] = scenes(1);
    assert_eq!(f, [1, 0, 0, 1], "the fort scene");
    assert_eq!(t, [0, 1, 0, 1], "the truck scene");
    assert_eq!(r, [0, 0, 1, 1], "the route scene");
    let sum: Vec<u32> = (0..4).map(|k| f[k] + t[k] + r[k]).collect();
    assert_eq!(sum, [1, 1, 1, 3], "three fields 1/1/1, s10b_refused 3");
    // Control: no truck memory, no truck give-up counted (nor refused).
    let [f, t, r] = scenes(0);
    assert_eq!(t, [0, 0, 0, 0], "control: refusal_memory 0");
    let sum: Vec<u32> = (0..4).map(|k| f[k] + t[k] + r[k]).collect();
    assert_eq!(sum, [1, 0, 1, 2]);
}

/// The player whose enemy base is the map's (928, 928) one, so on Kessel
/// Ridge C (512, 512) is nearer the enemy than B (300, 300).
fn front_world(edit: impl FnOnce(&mut frontline_content::MatchSetup)) -> (World, u8) {
    let w = flat_world_with(1, edit);
    let g = w.game().unwrap();
    let me = (0..2u8)
        .find(|&p| g.base_of(PlayerId(1 - p)) == c(928, 928))
        .unwrap();
    (w, me)
}

fn point(w: &World, name: &str) -> usize {
    w.match_setup()
        .unwrap()
        .map
        .points
        .iter()
        .position(|p| p.name == name)
        .unwrap()
}

/// F1 (orchestrator 2026-11-16, Build C meaning, `front` 1): B is this
/// player's home point (nearest its base). `front_home` 0: own B and C ->
/// [C]; own A, B and C -> [C, A]; own B only -> none (the home point is
/// front only while the enemy owns more than half of the points); enemy D
/// and E (2 of 5) -> still none; enemy A, D, E (3 of 5) -> [B], and with C
/// own too [C, B]. Control `front_home` 1 (plain F1): own B and C -> [C, B]
/// (seen failing the `front_home` 0 assertion there). And plain F1 below (with `front`
/// 0 the home rule is not applied, so the Build A counters stay):
/// own B and C -> [C, B] (C nearer the enemy base); own B only -> [B];
/// none own -> none. The plan counters: an AI owning C counts front plans,
/// all calm while no enemy is near, and a known enemy at C makes a plan
/// not calm. Control: `front_points` 0 -> no front point and no front plan
/// (the first assertion fails there, seen).
#[test]
fn ai_front_points_are_nearest_the_enemy_base() {
    let home = |front_home: u32| {
        let (mut w, me) = front_world(|s| {
            s.rules.ai.obstacles.front = 1;
            s.rules.ai.obstacles.front_home = front_home;
        });
        let (a, b, cc) = (point(&w, "A"), point(&w, "B"), point(&w, "C"));
        let (d, e) = (point(&w, "D"), point(&w, "E"));
        let g = w.state.game.as_mut().unwrap();
        assert_eq!(g.base_of(PlayerId(me)), c(96, 96), "B is the home point");
        g.points[b].owner = Some(PlayerId(me));
        g.points[cc].owner = Some(PlayerId(me));
        let two = front_points_of(g, PlayerId(me));
        g.points[a].owner = Some(PlayerId(me));
        let three = front_points_of(g, PlayerId(me));
        g.points[a].owner = None;
        g.points[cc].owner = None;
        let only = front_points_of(g, PlayerId(me));
        // Build C (orchestrator 2026-11-16): pushed back, the enemy owns
        // more than half of the points (A, D, E of 5): the home point is
        // front again; with two of 5 it is not.
        g.points[d].owner = Some(PlayerId(1 - me));
        g.points[e].owner = Some(PlayerId(1 - me));
        let half = front_points_of(g, PlayerId(me));
        g.points[a].owner = Some(PlayerId(1 - me));
        let pushed = front_points_of(g, PlayerId(me));
        g.points[cc].owner = Some(PlayerId(me));
        let pushed2 = front_points_of(g, PlayerId(me));
        (two, three, only, half, pushed, pushed2, [a, b, cc])
    };
    let (two, three, only, half, pushed, pushed2, [a, b, cc]) = home(0);
    assert_eq!(two, vec![cc], "front_home 0: the home point B is not front");
    assert_eq!(three, vec![cc, a]);
    assert!(
        only.is_empty(),
        "the home point alone is no front: {only:?}"
    );
    assert!(half.is_empty(), "enemy owns 2 of 5: not pushed: {half:?}");
    assert_eq!(
        pushed,
        vec![b],
        "enemy owns 3 of 5: the home point is front"
    );
    assert_eq!(
        pushed2,
        vec![cc, b],
        "pushed: B counts like any other point"
    );
    let (two, three, only, half, pushed, pushed2, _) = home(1);
    assert_eq!(two, vec![cc, b], "control: front_home 1");
    assert_eq!((three, only, half), (vec![cc, a], vec![b], vec![b]));
    assert_eq!((pushed, pushed2), (vec![b], vec![cc, b]));
    let run = |n: u32| {
        let (mut w, me) = front_world(|s| {
            s.rules.ai.obstacles.front = 0;
            s.rules.ai.obstacles.front_points = n;
        });
        let (b, cc) = (point(&w, "B"), point(&w, "C"));
        let g = w.state.game.as_mut().unwrap();
        g.points[b].owner = Some(PlayerId(me));
        g.points[cc].owner = Some(PlayerId(me));
        let both = front_points_of(g, PlayerId(me));
        g.points[cc].owner = None;
        let one = front_points_of(g, PlayerId(me));
        g.points[b].owner = None;
        let none = front_points_of(g, PlayerId(me));
        (both, one, none, b, cc)
    };
    let (both, one, none, b, cc) = run(2);
    assert_eq!(both, vec![cc, b], "C first");
    assert_eq!(one, vec![b]);
    assert!(none.is_empty());
    let (both, one, _, _, cc) = run(1);
    assert_eq!((both, one), (vec![cc], vec![b]));
    // Control: no front points.
    let (both, one, _, _, _) = run(0);
    assert!(both.is_empty() && one.is_empty(), "control: front_points 0");

    // The plan counters on an AI that owns C (no buys, no start squads).
    let plans = |n: u32, enemy: bool| {
        let (mut w, me) = front_world(|s| {
            s.rules.ai.obstacles.front = 0;
            s.rules.ai.obstacles.front_points = n;
            for p in s.players.iter_mut() {
                p.is_ai = false;
            }
            s.squad_types.iter_mut().for_each(|t| t.ai_weight = 0);
            for f in s.factions.iter_mut() {
                f.start_squads.clear();
            }
        });
        let cc = point(&w, "C");
        let at: FxVec2 = w.match_setup().unwrap().map.points[cc].pos.into();
        {
            let g = w.state.game.as_mut().unwrap();
            g.points[cc].owner = Some(PlayerId(me));
            g.ai[me as usize] = Some(AiState::new(w.state.tick));
        }
        let card = |p: u8| {
            let f = w.match_setup().unwrap().players[p as usize].faction as usize;
            if w.match_setup().unwrap().factions[f].id.contains("germany") {
                "ww2:ger_grenadiergruppe"
            } else {
                "ww2:usa_rifle_squad"
            }
        };
        let (mine, theirs) = (card(me), card(1 - me));
        spawn(&mut w, me, mine, at.x.floor_int(), at.y.floor_int());
        if enemy {
            let x = at.x.floor_int() + 15;
            spawn(&mut w, 1 - me, theirs, x, at.y.floor_int());
        }
        steps(&mut w, 30 * 30);
        let c = &w.game().unwrap().ai[me as usize].as_ref().unwrap().counters;
        (c.front_plans, c.front_calm_plans)
    };
    let (p, calm) = plans(2, false);
    assert!(p >= 2 && calm == p, "calm: {calm} of {p}");
    let (p, calm) = plans(2, true);
    assert!(p >= 2 && calm < p, "an enemy at C: {calm} of {p}");
    assert_eq!(plans(0, false), (0, 0), "control: front_points 0");
}

/// Finding 12: at a trenched own point with the AP field on and no wire,
/// every plan's reason is `wire_first` (no builder is there either: the
/// wire gate comes first). Control: the nest and wire built -> the next
/// reason, `no_builder`, and no `wire_first` (seen failing the first
/// assertion).
#[test]
fn mine_want_why_names_the_wire_gate() {
    let run = |wire: bool| {
        let (mut w, h): (World, Held) = held(|s| s.rules.ai.obstacles.at_mines = 0);
        if wire {
            let o = w.match_setup().unwrap().rules.ai.fort.wire_out_m as i32;
            let nest = h.line[1] + h.along * Fx::from_int(6);
            let nest_f = fort(&w, "ww2:mg_nest");
            assert!(w.build_now(
                nest_f,
                &[nest, nest + h.enemy * Fx::from_int(10)],
                PlayerId(1)
            ));
            let wc = h.mid() + h.enemy * Fx::from_int(o);
            let wire_f = fort(&w, "ww2:barbed_wire");
            let line = [
                wc - h.along * Fx::from_int(30),
                wc + h.along * Fx::from_int(30),
            ];
            assert!(w.build_now(wire_f, &line, PlayerId(1)));
        }
        steps(&mut w, 30 * 30);
        ai_ro(&w).counters.mine_want_why
    };
    let at = |name: &str| MINE_WANT_WHY.iter().position(|n| *n == name).unwrap();
    let why = run(false);
    assert!(why[at("wire_first")] >= 2, "{why:?}");
    assert_eq!(why.iter().sum::<u32>(), why[at("wire_first")], "{why:?}");
    // Control: the wire stands.
    let why = run(true);
    assert_eq!(why[at("wire_first")], 0, "control: {why:?}");
    assert!(why[at("no_builder")] >= 2, "control: {why:?}");
    assert_eq!(why.iter().sum::<u32>(), why[at("no_builder")], "{why:?}");
}

/// Spec 6: the new counters and setup keys save, load and hash. Control:
/// an unedited reload keeps the hash (each edit must change it; an edit
/// that is not saved would fail the `assert_ne`).
#[test]
fn s10c_ai_layout_roundtrips_and_hashes() {
    let mut w = flat_world_with(1, |s| s.players[1].is_ai = true);
    steps(&mut w, 3);
    assert!(w.with_ai_state(1, |ai| {
        let c = &mut ai.counters;
        c.fort_job_fails = 1;
        c.truck_give_ups = 2;
        c.route_refusals = 3;
        c.front_plans = 4;
        c.front_calm_plans = 5;
        c.front_jobs = 6;
        c.front_drops = 7;
        c.front_lost = 8;
        c.front_resumes = 9;
        c.front_why[11] = 10;
        c.mine_want_why[7] = 11;
        c.belt_jobs = 12;
        c.belt_route_rejects = 13;
        c.builder_buys = 14;
        // Addendum 1: two saved lanes (Build A leaves them empty in play).
        ai.front_lanes = vec![lane(1, 0), lane(3, 1)];
        // Fix pass 3 (C-5): the second lane holds a saved AT row line.
        ai.front_lanes[1].at = Some([FxVec2::from_int(280, 330), FxVec2::from_int(320, 330)]);
        // Fix pass 3: the thirteenth `front_why` entry (`at_wait`).
        ai.counters.front_why[12] = 15;
    }));
    let bytes = w.save();
    let back = World::load(&bytes, 1).expect("the S10c-ai state loads");
    assert_eq!(back.game().unwrap().ai[1], w.game().unwrap().ai[1]);
    assert_eq!(back.full_state_hash(), w.full_state_hash());
    assert_eq!(back.save(), bytes);
    let h = back.full_state_hash();
    type Edit = Box<dyn Fn(&mut SimState)>;
    fn ai(s: &mut SimState) -> &mut AiCounters {
        &mut s.game.as_mut().unwrap().ai[1].as_mut().unwrap().counters
    }
    fn o(s: &mut SimState) -> &mut frontline_content::tuning::AiObstacleTuning {
        &mut s.game.as_mut().unwrap().setup.rules.ai.obstacles
    }
    fn lanes(s: &mut SimState) -> &mut Vec<FrontLane> {
        &mut s.game.as_mut().unwrap().ai[1].as_mut().unwrap().front_lanes
    }
    let edits: Vec<Edit> = vec![
        Box::new(|s| ai(s).fort_job_fails += 1),
        Box::new(|s| ai(s).truck_give_ups += 1),
        Box::new(|s| ai(s).route_refusals += 1),
        Box::new(|s| ai(s).front_calm_plans += 1),
        Box::new(|s| ai(s).front_why[0] += 1),
        Box::new(|s| ai(s).mine_want_why[0] += 1),
        Box::new(|s| ai(s).builder_buys += 1),
        // Build B chunk 2: `front` is on by default; `front_home` new.
        // Fix pass 1 (B-5): `front` 0 with saved lanes no longer loads, so
        // the switch edit clears them too (`mine_assign` alone below).
        Box::new(|s| {
            o(s).front = 0;
            lanes(s).clear();
        }),
        Box::new(|s| o(s).mine_assign = 0),
        Box::new(|s| o(s).front_home = 1),
        Box::new(|s| o(s).front_points = 3),
        Box::new(|s| o(s).belt_detour_pct = 200),
        // Build C chunk 2: the default is 2, so the edit sets 3.
        Box::new(|s| o(s).builder_cards_min = 3),
        Box::new(|s| o(s).front_slots = 2),
        Box::new(|s| o(s).front_wire_len_m = 30),
        Box::new(|s| lanes(s)[0].ap_done = true),
        Box::new(|s| lanes(s)[1].wire[1] = c(301, 300)),
        Box::new(|s| lanes(s)[1].since = 0),
        // Fix pass 3: the saved AT line (value and tag), `at_wait`, and the
        // key `mine_assign_reach_m`.
        Box::new(|s| lanes(s)[1].at = Some([c(281, 330), c(320, 330)])),
        Box::new(|s| lanes(s)[1].at = None),
        Box::new(|s| lanes(s)[0].at = Some([c(280, 330), c(320, 330)])),
        Box::new(|s| ai(s).front_why[12] += 1),
        Box::new(|s| o(s).mine_assign_reach_m = 600),
    ];
    let same = World::load(&back.save(), 1).unwrap();
    assert_eq!(same.full_state_hash(), h);
    for (k, e) in edits.into_iter().enumerate() {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        let y = World::load(&x.save(), 1).unwrap_or_else(|e| panic!("edit {k}: {e:?}"));
        assert_ne!(y.full_state_hash(), h, "edit {k} is not hashed");
    }
}

/// A front lane at `point` found at tick `since` (sites near (300, 300)).
fn lane(point: u8, since: u64) -> FrontLane {
    FrontLane {
        point,
        since,
        ap: [c(290, 310), c(310, 310)],
        wire: [c(295, 300), c(305, 300)],
        ap_done: false,
        wire_done: false,
        at: None,
    }
}

/// Addendum 1: `front_lanes` loads only within its rules (spec 3): at most
/// `MAX_FRONT_LANES`, points in range, unique and ascending, `since` not in
/// the future, sites on the map. Control: the good lanes load (first
/// assert); the S10c lines of `validate_fort` replaced by `true` -> the
/// first case loads (seen).
#[test]
fn load_rejects_bad_front_lanes() {
    let mut w = flat_world_with(1, |s| s.players[1].is_ai = true);
    steps(&mut w, 3);
    assert!(w.with_ai_state(1, |ai| {
        ai.front_lanes = vec![lane(0, 1), lane(2, 2)];
        ai.front_lanes[1].at = Some([c(280, 330), c(320, 330)]);
    }));
    let bytes = w.save();
    let back = World::load(&bytes, 1).expect("good front lanes load");
    assert_eq!(ai_ro(&back).front_lanes.len(), 2);
    fn lanes(s: &mut SimState) -> &mut Vec<FrontLane> {
        &mut s.game.as_mut().unwrap().ai[1].as_mut().unwrap().front_lanes
    }
    type Edit = Box<dyn Fn(&mut SimState)>;
    let bad: Vec<(&str, Edit)> = vec![
        (
            "over the cap",
            Box::new(|s| {
                // Kessel Ridge has 5 points, so more than 8 lanes also
                // repeat a point: this case is caught by the cap and the
                // order check together.
                *lanes(s) = (0..=MAX_FRONT_LANES as u8)
                    .map(|p| lane(p % 5, 1))
                    .collect();
            }),
        ),
        ("point out of range", Box::new(|s| lanes(s)[1].point = 9)),
        ("points not unique", Box::new(|s| lanes(s)[1].point = 0)),
        ("points not ascending", Box::new(|s| lanes(s).reverse())),
        (
            "lane found in the future",
            Box::new(|s| {
                let t = s.tick;
                lanes(s)[0].since = t + 1;
            }),
        ),
        (
            "ap site off the map",
            Box::new(|s| lanes(s)[0].ap[0] = c(-1, 300)),
        ),
        (
            "wire site off the map",
            Box::new(|s| lanes(s)[1].wire[1] = c(300, 5000)),
        ),
        (
            // Fix pass 3 (C-5): the saved AT row line is on the map too.
            "at site off the map",
            Box::new(|s| lanes(s)[1].at = Some([c(300, 300), c(-1, 300)])),
        ),
    ];
    for (what, e) in bad {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        assert!(
            World::load(&x.save(), 1).is_err(),
            "bad front lanes loaded: {what}"
        );
    }
}

// ------------------------------------------------------- Fix pass 1 --

/// Finding A-1: a laid R1 field at a threatened point (here every point:
/// `build_threat_max` 0) is not counted as `not_own_or_trench`; only the
/// AP field is switched on. Control: no field laid -> one count each plan
/// (seen: the main assertion fails with the old order, where the threat
/// check came before "laid").
#[test]
fn mine_want_why_skips_a_laid_field_at_a_threatened_point() {
    let run = |laid: bool| {
        let (mut w, h): (World, Held) = held(|s| {
            s.rules.ai.obstacles.at_mines = 0;
            s.rules.ai.fort.wire = 0;
            s.rules.ai.fort.build_threat_max = 0;
        });
        if laid {
            let out = w.match_setup().unwrap().rules.ai.obstacles.ap_out_m as i32;
            let at = h.mid() + h.enemy * Fx::from_int(out);
            let ap = fort(&w, "ww2:ap_minefield");
            w.lay_mine_now(ap, at, PlayerId(1)).unwrap();
        }
        steps(&mut w, 30 * 30);
        ai_ro(&w).counters.mine_want_why
    };
    let at = |name: &str| MINE_WANT_WHY.iter().position(|n| *n == name).unwrap();
    let why = run(true);
    assert_eq!(why.iter().sum::<u32>(), 0, "laid field counted: {why:?}");
    let why = run(false);
    assert!(why[at("not_own_or_trench")] >= 2, "control: {why:?}");
}

/// Finding A-4: the lane cap on its own: on a map with ten points, nine
/// lanes with unique, ascending points in range are refused by
/// `MAX_FRONT_LANES` alone. Control: eight such lanes load.
#[test]
fn load_rejects_lanes_over_the_cap_alone() {
    let mut w = flat_world_with(1, |s| {
        s.players[1].is_ai = true;
        let p0 = s.map.points[0].clone();
        s.map.points = (0..10)
            .map(|k| {
                let mut p = p0.clone();
                p.pos = frontline_content::setup::FixedVec2::from_m([100 + 80 * k, 500]);
                p
            })
            .collect();
    });
    steps(&mut w, 3);
    let n = MAX_FRONT_LANES as u8;
    let with = |w: &mut World, k: u8| {
        assert!(w.with_ai_state(1, |ai| ai.front_lanes =
            (0..k).map(|p| lane(p, 1)).collect()));
        World::load(&w.save(), 1)
    };
    assert!(with(&mut w, n).is_ok(), "control: {n} lanes load");
    assert!(with(&mut w, n + 1).is_err(), "{} lanes loaded", n + 1);
}

/// Finding B-5: saved lanes with `front` 0 are refused on load (the front
/// rules would apply on a crafted save). Control: the same lanes with
/// `front` 1 load.
#[test]
fn load_rejects_front_lanes_with_front_off() {
    for front in [1u32, 0] {
        let mut w = flat_world_with(1, |s| {
            s.players[1].is_ai = true;
            s.rules.ai.obstacles.front = front;
        });
        steps(&mut w, 3);
        assert!(w.with_ai_state(1, |ai| ai.front_lanes = vec![lane(0, 1)]));
        let r = World::load(&w.save(), 1);
        assert_eq!(r.is_ok(), front == 1, "front {front}");
    }
}

/// Finding B-2: a front AP field (on the saved lane's FrontAp line, here
/// 10 m beyond the trenched point's R1 site and inside its 21 m radius)
/// does not stand in for the point's R1 field: an R1 job starts. Control
/// (code edit: `field_has` back to `fort_near`, the lane's mines counted)
/// -> no R1 job (seen).
#[test]
fn front_field_does_not_stand_in_for_the_r1_field() {
    let (mut w, h) = held(|s| {
        let o = &mut s.rules.ai.obstacles;
        s.rules.ai.fort.wire = 0;
        o.at_mines = 0;
        o.front = 1;
        o.front_home = 1;
    });
    steps(&mut w, 3 * 30);
    let out = w.match_setup().unwrap().rules.ai.obstacles.ap_out_m as i32;
    let r1 = h.mid() + h.enemy * Fx::from_int(out);
    let far = r1 + h.enemy * Fx::from_int(10);
    let line = [
        far - h.along * Fx::from_int(15),
        far + h.along * Fx::from_int(15),
    ];
    assert!(w.with_ai_state(1, |ai| {
        let l = &mut ai.front_lanes[0];
        l.ap = line;
        l.ap_done = true;
        l.wire_done = true;
    }));
    let ap = fort(&w, "ww2:ap_minefield");
    for k in 0..8 {
        let p = line[0] + h.along * Fx::from_int(4 * k);
        w.lay_mine_now(ap, p, PlayerId(1)).unwrap();
    }
    squad_at(&mut w, "ww2:usa_engineer_squad", h.pos, 120);
    let mut r1_job = false;
    for _ in 0..60 {
        steps(&mut w, 30);
        r1_job |= ai_ro(&w).forts.values().any(|j| {
            j.fort == ap
                && j.line
                    .last()
                    .is_some_and(|e| (*e - line[1]).length() > Fx::from_int(5))
        });
    }
    assert!(r1_job, "no R1 field job beside the front field");
}

/// Finding B-3: a front work under a young refusal (L4) holds no builder:
/// the held point's only want is its FrontAp line, refused at its middle
/// a moment ago; over 60 s no plan holds the engineer for it
/// (`fort_build_plans` 0, S10a on through the wire habit). Control (code
/// edit: the B-3 check in `fort_holds` removed) -> the engineer is held
/// every plan (seen).
#[test]
fn a_refused_front_work_holds_no_builder() {
    let (mut w, h) = held(|s| {
        let o = &mut s.rules.ai.obstacles;
        o.at_mines = 0;
        o.fields_per_point = 0;
        o.front = 1;
        o.front_home = 1;
    });
    steps(&mut w, 3 * 30);
    let tick = w.state.tick;
    assert!(w.with_ai_state(1, |ai| {
        let l = &mut ai.front_lanes[0];
        l.wire_done = true;
        let mid = (l.ap[0] + l.ap[1]).div_int(2);
        ai.refuse(crate::ai::JobKind::Fort, mid, tick);
    }));
    squad_at(&mut w, "ww2:usa_engineer_squad", h.pos, 120);
    steps(&mut w, 60 * 30);
    let c = &ai_ro(&w).counters;
    assert_eq!(c.fort_build_plans, 0, "held for a refused front work");
    assert!(ai_ro(&w).forts.is_empty(), "a job started");
}

/// Finding B-4: F5's "at the site" is measured from the nearest point of
/// the work's line: a builder 15 m beyond the end of a 40 m AT row (35 m
/// from its middle) is at the site, so an enemy 70 m from it (78 m from the
/// middle) is no threat (50 m), where the old middle rule gave it the walk's
/// 80 m. Control: the builder 25 m beyond the end is walking -> threat
/// (`Some(false)`); and the old rule (code edit: `near(lead, site, 20)`)
/// fails the main assertion (seen).
#[test]
fn front_threat_at_site_is_measured_from_the_line() {
    let w = flat_world_with(1, |s| s.players[1].is_ai = true);
    let g = w.game().unwrap();
    let mut ai = AiState::new(0);
    let line = [c(300, 300), c(340, 300)];
    let probe = |ai: &mut AiState, lead: FxVec2| {
        let enemy = lead + c(0, 70);
        g.front_threat_probe(
            ai,
            &w.state.entities,
            &w.state.terrain,
            PlayerId(1),
            lead,
            &line,
            enemy,
        )
    };
    assert_eq!(probe(&mut ai, c(285, 300)), None, "at the row's end");
    assert_eq!(probe(&mut ai, c(275, 300)), Some(false), "control: walking");
}
