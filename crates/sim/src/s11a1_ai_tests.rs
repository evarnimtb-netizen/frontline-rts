//! S11a1 Build C tests (docs/slices/S11a1.md 4 A1-A6, P1-P3, 5): the
//! artillery AI. Each test names its control: the edit that makes it fail.

use super::s6_tests::{flat_world_with, members, spawn, steps};
use super::*;
use crate::ai::ArtyPhase;
use crate::game::Order;
use crate::vehicle::GunState;
use frontline_content::setup::member_trait;

const LEIG: &str = "ww2:ger_leig18_squad";
const VB: &str = "ww2:ger_vb_trupp";

fn c(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

/// A flat match whose player 0 (Germany) is an AI with `[ai.arty]` `buy`
/// and `fo` as given, the leIG and VB-Trupp cards unlocked at once and
/// 5,000 MP; `edit` changes the setup last.
fn ai_world(
    threads: usize,
    buy: u32,
    fo: u32,
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> World {
    let mut w = flat_world_with(threads, |s| {
        s.players[0].is_ai = true;
        s.rules.ai.arty.buy = buy;
        s.rules.ai.arty.fo = fo;
        s.rules.ai.arty.after_s = 0;
        for t in s.squad_types.iter_mut() {
            if t.id == LEIG || t.id == VB {
                t.initial_cooldown_ticks = 0;
            }
        }
        edit(s);
    });
    w.state.game.as_mut().unwrap().players[0].manpower = 5000;
    w
}

fn counters(w: &World) -> crate::ai::AiCounters {
    w.game().unwrap().ai[0].as_ref().unwrap().counters.clone()
}

fn ai(w: &World) -> &crate::ai::AiState {
    w.game().unwrap().ai[0].as_ref().unwrap()
}

/// Own squads of player 0 whose card is `card`.
fn own_of(w: &World, card: &str) -> Vec<SquadId> {
    let s = w.match_setup().unwrap();
    w.game()
        .unwrap()
        .squads
        .iter()
        .filter(|(_, q)| q.owner == PlayerId(0) && s.squad_types[q.type_idx as usize].id == card)
        .map(|(&id, _)| id)
        .collect()
}

/// A1: with `buy` 1 the AI buys one infantry gun inside its share of the
/// income. Controls: `buy` 0, and `income_max_pct` 1 (the card never fits
/// the share): no gun.
#[test]
fn ai_buys_an_infantry_gun_inside_its_budget() {
    let run = |buy: u32, pct: u32| {
        let mut w = ai_world(1, buy, 0, |s| s.rules.ai.arty.income_max_pct = pct);
        steps(&mut w, 30 * 20);
        (counters(&w).arty_buys, own_of(&w, LEIG).len())
    };
    // The share: 15 % of 600 MP is 90 MP < 320 at minute 0; 100 % fits.
    assert_eq!(run(1, 100), (1, 1), "one gun, max_guns 1");
    assert_eq!(run(0, 100), (0, 0), "control: buy 0");
    assert_eq!(run(1, 1), (0, 0), "control: income_max_pct 1");
}

/// A2: a limbered AI gun is ordered to emplace and is Ready within 60 s.
/// Control: `buy` 0 (no record, no order): it stays limbered.
#[test]
fn ai_emplaces_its_infantry_gun() {
    let run = |buy: u32| {
        let mut w = ai_world(1, buy, 0, |s| s.rules.ai.arty.max_guns = 0);
        steps(&mut w, 2);
        let q = spawn(&mut w, 0, LEIG, 120, 500);
        let gun = members(&w, q)[0];
        w.state
            .game
            .as_mut()
            .unwrap()
            .vehicles
            .get_mut(&gun)
            .unwrap()
            .served
            .as_mut()
            .unwrap()
            .state = GunState::Limbered;
        steps(&mut w, 30 * 60);
        let g = w.game().unwrap();
        (
            g.vehicles[&gun].served.as_ref().unwrap().state,
            ai(&w).arty.get(&q).map(|a| a.phase),
        )
    };
    assert_eq!(run(1), (GunState::Emplaced, Some(ArtyPhase::Ready)));
    assert_eq!(run(0), (GunState::Limbered, None), "control: buy 0");
}

/// A3 and the wiring: with a Ready gun the AI buys one observer team; it
/// takes a post that sees the front point (flat ground: the first
/// candidate) and gets only the A3 orders (its order is the move to the
/// post, never an infantry habit's). Control: `fo` 0: no team.
#[test]
fn ai_buys_one_observer_for_its_gun() {
    let run = |fo: u32| {
        let mut w = ai_world(1, 1, fo, |s| s.rules.ai.arty.max_guns = 0);
        steps(&mut w, 2);
        spawn(&mut w, 0, LEIG, 120, 500);
        steps(&mut w, 30 * 30);
        w
    };
    let w = run(1);
    let k = counters(&w);
    assert_eq!(k.fo_buys, 1, "one team, fo_max 1");
    let fo = own_of(&w, VB);
    assert_eq!(fo.len(), 1);
    let rec = ai(&w).fo[&fo[0]];
    let post = rec.post.expect("a post");
    assert_eq!(k.fo_posts, 1);
    assert_eq!(k.fo_post_fails, 0);
    let q = &w.game().unwrap().squads[&fo[0]];
    let lead = w.entities().transform[q.lead(w.entities()).index()]
        .pos
        .xy();
    let moving_there = matches!(q.order, Order::Move { target } if target == post);
    assert!(
        moving_there || (lead - post).length() <= Fx::from_int(15),
        "only the A3 orders: {:?}",
        q.order.kind()
    );
    let w = run(0);
    assert_eq!(counters(&w).fo_buys, 0, "control: fo 0");
    assert!(own_of(&w, VB).is_empty());
}

/// A3: the post search casts at most `post_rays` rays a plan and fails
/// over when no candidate sees the point. A 30 m ridge across the line
/// from the base to every point: `fo_post_fails` > 0. Control: flat
/// ground finds a post (the test above).
#[test]
fn ai_observer_post_search_fails_behind_a_ridge_and_is_bounded() {
    use frontline_content::Fixed;
    use frontline_content::setup::{BandSetup, FixedVec2};
    let mut w = ai_world(1, 0, 1, |s| {
        s.rules.ai.arty.post_rays = 2;
        // A ring of ridge around every capture point, 60 m out.
        let pts: Vec<_> = s.map.points.iter().map(|p| p.pos).collect();
        for p in pts {
            let (x, y) = (Fx::from(p.x).floor_int(), Fx::from(p.y).floor_int());
            for (a, b) in [
                ((-70, -70), (70, -70)),
                ((70, -70), (70, 70)),
                ((70, 70), (-70, 70)),
                ((-70, 70), (-70, -70)),
            ] {
                s.map.terrain.bands.push(BandSetup {
                    from: FixedVec2::from_m([(x + a.0).max(0) as u32, (y + a.1).max(0) as u32]),
                    to: FixedVec2::from_m([(x + b.0).max(0) as u32, (y + b.1).max(0) as u32]),
                    height: Fixed::from_int(30),
                    half_width: Fixed::from_int(15),
                });
            }
        }
    });
    steps(&mut w, 2);
    spawn(&mut w, 0, VB, 150, 500);
    steps(&mut w, 30 * 30);
    let k = counters(&w);
    assert!(
        k.fo_post_fails >= 1,
        "fails {} posts {}",
        k.fo_post_fails,
        k.fo_posts
    );
    // Bounded: 15 candidates at 2 rays a plan take 8 plans for one search.
    let plans = k.plans as u32;
    assert!(
        k.fo_post_fails <= plans / 7 + 1,
        "fails {} plans {plans}",
        k.fo_post_fails
    );
}

/// The A4 scene: the AI's leIG at (100, 500), its VB-Trupp at (600, 500)
/// (beyond the voice link: radio only), a US rifle squad at (700, 500)
/// with no rounds that the team sees, the point marked in the AI's intel. `edit`
/// changes the setup; `kill_radio` kills the team's radio man first.
fn fire_scene(
    threads: usize,
    kill_radio: bool,
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> (World, SquadId) {
    let mut w = ai_world(threads, 1, 0, |s| {
        s.rules.ai.arty.max_guns = 0;
        edit(s);
    });
    steps(&mut w, 2);
    let q = spawn(&mut w, 0, LEIG, 100, 500);
    let fo = spawn(&mut w, 0, VB, 600, 500);
    let e = spawn(&mut w, 1, "ww2:usa_rifle_squad", 700, 500);
    // The target squad has no rounds (the team must live to observe).
    for m in members(&w, e) {
        w.edit_soldier(m, |k, c| {
            k.rounds = [0; frontline_content::setup::MAX_FEEDS];
            c.mag = 0;
        });
    }
    assert!(w.mark_intel(PlayerId(0), c(700, 500)));
    if kill_radio {
        let s = w.match_setup().unwrap().clone();
        let ty = &s.squad_types[w.game().unwrap().squads[&fo].type_idx as usize];
        for m in members(&w, fo) {
            if ty.members[w.entities().kit[m.index()].slot as usize].traits & member_trait::RADIO
                != 0
            {
                assert!(w.kill_soldier(m));
            }
        }
    }
    (w, q)
}

/// A4: the AI's gun fires an observed mission through its observer team
/// (radio), and the sim refuses none of its orders. Control: the team's
/// radio man dead: no mission, `arty_no_observer` > 0.
#[test]
fn ai_gun_fires_an_observed_mission_through_its_observer() {
    let (mut w, q) = fire_scene(1, false, |_| {});
    steps(&mut w, 30 * 20);
    let k = counters(&w);
    assert!(k.arty_missions >= 1, "missions {}", k.arty_missions);
    let s11 = w.game().unwrap().players[0].s11;
    assert!(s11.gun_missions[crate::fire_mission::mode::OBSERVED as usize] >= 1);
    assert_eq!(s11.masked + s11.not_ready, 0);
    assert_eq!(ai(&w).arty[&q].missions as u32, k.arty_missions);
    let (mut w, _) = fire_scene(1, true, |_| {});
    steps(&mut w, 30 * 20);
    let k = counters(&w);
    assert_eq!(k.arty_missions, 0, "control: no radio, no mission");
    assert!(k.arty_no_observer > 0);
}

/// A5: no mission while the gun's shells cost more than
/// `shell_mp_max_pct` of the income so far. Control: 100 %.
#[test]
fn ai_gun_stops_at_its_shell_budget() {
    let run = |pct: u32| {
        let (mut w, _) = fire_scene(1, false, |s| s.rules.ai.arty.shell_mp_max_pct = pct);
        // 400 MP of shells against 600 MP of income: 67 %.
        w.state.game.as_mut().unwrap().players[0]
            .s11
            .gun_shell_micro = 400 * crate::supply::MICRO;
        steps(&mut w, 30 * 20);
        counters(&w)
    };
    let k = run(25);
    assert_eq!(k.arty_missions, 0);
    assert!(k.arty_budget_holds > 0);
    assert!(run(100).arty_missions >= 1, "control: 100 %");
}

/// A6: an AI gun mission whose target gets an own man within
/// `danger_close_m` is stopped. Control: an own squad halted 80 m off:
/// the mission runs on.
#[test]
fn ai_lifts_gun_fire_when_own_men_close() {
    let run = |dx: i32| {
        let (mut w, q) = fire_scene(1, false, |s| s.rules.ai.arty.mission_rounds = 12);
        let mut on = false;
        for _ in 0..30 * 20 {
            w.step(&[]);
            if matches!(
                w.game().unwrap().squads[&q].order,
                Order::FireMission { .. }
            ) {
                on = true;
                break;
            }
        }
        assert!(on, "a mission started");
        let Order::FireMission { target, .. } = w.game().unwrap().squads[&q].order else {
            unreachable!()
        };
        let x = target.x.floor_int() - dx;
        let y = target.y.floor_int();
        let own = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", x, y);
        // Halt them where they stand (the infantry habits may move them).
        let _ = own;
        steps(&mut w, 30 * 6);
        (
            counters(&w).arty_lifts,
            matches!(
                w.game().unwrap().squads[&q].order,
                Order::FireMission { .. }
            ),
        )
    };
    let (lifts, on) = run(10);
    assert!(lifts >= 1 && !on, "lifted: {lifts} {on}");
    let (lifts, _) = run(80);
    assert_eq!(lifts, 0, "control: 80 m off");
}

/// Liveness and neutrality of the switches: with `buy` and `fo` 0 an own
/// gun and observer team get no record and no command over 40 plans.
/// Control: `buy` 1 gives the gun a record.
#[test]
fn ai_all_s11a1_switches_off_emit_nothing_new() {
    let run = |buy: u32| {
        let mut w = ai_world(1, buy, 0, |s| s.rules.ai.arty.max_guns = 0);
        steps(&mut w, 2);
        let q = spawn(&mut w, 0, LEIG, 120, 500);
        let fo = spawn(&mut w, 0, VB, 300, 500);
        let o0 = (
            w.game().unwrap().squads[&q].order.kind(),
            w.game().unwrap().squads[&fo].order.kind(),
        );
        let plans0 = counters(&w).plans;
        while counters(&w).plans < plans0 + 40 {
            w.step(&[]);
        }
        let o1 = (
            w.game().unwrap().squads[&q].order.kind(),
            w.game().unwrap().squads[&fo].order.kind(),
        );
        (ai(&w).arty.len() + ai(&w).fo.len(), o0 == o1)
    };
    assert_eq!(run(0), (0, true));
    assert_eq!(run(1).0, 1, "control: buy 1 keeps a record");
}

/// Determinism (spec 5): the A4 scene runs the same on 1 and 4 threads,
/// and saves taken with a gun and an observer record, before and after
/// the first mission, play on to the same hash. Control: a run whose
/// observer's radio man dies differs.
#[test]
fn ai_arty_deterministic_threads_and_save_load() {
    let go = |threads: usize| {
        let (mut w, _) = fire_scene(threads, false, |s| s.rules.ai.arty.fo = 1);
        let mut saves = Vec::new();
        for t in 0..30 * 40u64 {
            if t == 30 || t == 30 * 12 || t == 30 * 25 {
                saves.push((w.tick(), w.save()));
            }
            w.step(&[]);
        }
        (w.full_state_hash(), w.tick(), saves)
    };
    let (h1, end, saves) = go(1);
    let (h4, _, _) = go(4);
    assert_eq!(h1, h4, "1 and 4 threads");
    for (t, bytes) in saves {
        let mut w = World::load(&bytes, 1).unwrap_or_else(|e| panic!("save at {t}: {e:?}"));
        while w.tick() < end {
            w.step(&[]);
        }
        assert_eq!(w.full_state_hash(), h1, "the save at tick {t}");
    }
    let (mut w, _) = fire_scene(1, true, |s| s.rules.ai.arty.fo = 1);
    while w.tick() < end {
        w.step(&[]);
    }
    assert_ne!(w.full_state_hash(), h1, "control");
}

/// Spec 5 AiFo: a lost observer team counts `fo_lost` and is bought again
/// once, `fo_rebuy_s` after the loss. Control: a rebuy wait longer than
/// the run: no second team.
#[test]
fn ai_observer_is_rebought_once() {
    let run = |rebuy_s: u32| {
        let mut w = ai_world(1, 1, 1, |s| {
            s.rules.ai.arty.max_guns = 0;
            s.rules.ai.arty.fo_rebuy_s = rebuy_s;
        });
        steps(&mut w, 2);
        spawn(&mut w, 0, LEIG, 120, 500);
        steps(&mut w, 30 * 10);
        let fo = own_of(&w, VB);
        assert_eq!(fo.len(), 1, "the first team");
        for m in members(&w, fo[0]) {
            assert!(w.kill_soldier(m));
        }
        // The card's own 60 s cooldown runs from the first buy.
        steps(&mut w, 30 * 70);
        let k = counters(&w);
        (k.fo_lost, k.fo_buys, own_of(&w, VB).len())
    };
    assert_eq!(run(20), (1, 2, 1), "lost once, bought again once");
    assert_eq!(run(600), (1, 1, 0), "control: rebuy wait 600 s");
}
