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

/// F1: own B and C -> [C, B] (C nearer the enemy base); own B only -> [B];
/// none own -> none. The plan counters: an AI owning C counts front plans,
/// all calm while no enemy is near, and a known enemy at C makes a plan
/// not calm. Control: `front_points` 0 -> no front point and no front plan
/// (the first assertion fails there, seen).
#[test]
fn ai_front_points_are_nearest_the_enemy_base() {
    let run = |n: u32| {
        let (mut w, me) = front_world(|s| s.rules.ai.obstacles.front_points = n);
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
        Box::new(|s| o(s).front = 1),
        Box::new(|s| o(s).front_points = 3),
        Box::new(|s| o(s).belt_detour_pct = 200),
        Box::new(|s| o(s).builder_cards_min = 2),
        Box::new(|s| o(s).front_slots = 2),
        Box::new(|s| o(s).front_wire_len_m = 30),
        Box::new(|s| lanes(s)[0].ap_done = true),
        Box::new(|s| lanes(s)[1].wire[1] = c(301, 300)),
        Box::new(|s| lanes(s)[1].since = 0),
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
    assert!(w.with_ai_state(1, |ai| ai.front_lanes = vec![lane(0, 1), lane(2, 2)]));
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
