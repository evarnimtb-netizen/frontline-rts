//! S3 AI with vehicles ("Done when" 1): AI-vs-AI skirmishes under the test
//! unlock rules buy, escort and fight with tanks and AT teams; the same
//! hashes at 1, 2, 3 and 8 threads; and save → load → continue at the
//! staged moments: a shell in flight, a burning vehicle, a bogged vehicle,
//! a crew swap in progress and a pending vehicle path request. A moment the
//! match does not reach by itself is staged with the setup tools (in the
//! run that is saved, so the loaded copy continues the same history).

use std::path::Path;

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::{EntityKind, PathReq, PlayerId, SimId, World};

fn setup(seed: u64) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let mut defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let rules = defs
        .add_rules_override(&root.join("tests/rules_test_unlock.toml"))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut req = MatchRequest::skirmish(seed, "ww2:germany", "ww2:usa", Difficulty::Normal);
    req.rules = rules;
    let mut s = defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
    s.players.iter_mut().for_each(|p| p.is_ai = true);
    // Tanks early on both sides.
    s.rules.start_manpower = 1500;
    s
}

fn run(w: &mut World, ticks: u64) -> Vec<u64> {
    let mut h = Vec::new();
    for _ in 0..ticks {
        w.step(&[]);
        if w.tick().is_multiple_of(30) {
            h.push(w.state_hash());
        }
    }
    h
}

fn live_vehicles(w: &World) -> Vec<SimId> {
    w.game()
        .unwrap()
        .vehicles()
        .keys()
        .copied()
        .filter(|v| w.entities().kind()[v.index()] == EntityKind::Vehicle)
        .collect()
}

#[test]
fn ai_vs_ai_with_vehicles_is_identical_across_thread_counts() {
    let ticks = 30 * 300;
    let mut base = World::new_match(setup(3), 1).unwrap();
    let want = run(&mut base, ticks);
    let g = base.game().unwrap();
    for p in 0..2u8 {
        let ai = g.ai(PlayerId(p)).unwrap();
        assert!(
            ai.counters.vehicles_bought >= 1,
            "player {p} bought no vehicle: {:?}",
            ai.counters
        );
    }
    assert!(base.stats().armour_box_tests > 0, "shells met armour");
    let c = |p: u8| g.ai(PlayerId(p)).unwrap().counters.clone();
    println!("counters: {:?} / {:?}", c(0), c(1));
    assert!(
        c(0).escorts_started + c(1).escorts_started >= 1,
        "an escorted assault started"
    );
    assert!(c(0).at_bought + c(1).at_bought >= 1, "an AT team bought");
    for threads in [2, 3, 8] {
        let mut w = World::new_match(setup(3), threads).unwrap();
        assert_eq!(run(&mut w, ticks), want, "{threads} threads");
    }
}

/// Saves `w` now, continues both the original and the loaded copy (at
/// `threads`) for `ticks`, and compares every tick's hash.
fn save_load_continue(w: &mut World, threads: usize, ticks: u64, what: &str) {
    let bytes = w.save();
    let mut b = World::load(&bytes, threads).unwrap_or_else(|e| panic!("{what}: {e}"));
    assert_eq!(b.state_hash(), w.state_hash(), "{what}: hash after load");
    for _ in 0..ticks {
        w.step(&[]);
        b.step(&[]);
        assert_eq!(b.state_hash(), w.state_hash(), "{what}: tick {}", w.tick());
    }
}

#[test]
fn ai_vs_ai_with_vehicles_survives_save_load_at_the_staged_moments() {
    let mut w = World::new_match(setup(3), 2).unwrap();
    // Until both sides have tanks out.
    for _ in 0..30 * 240 {
        w.step(&[]);
        let owners: Vec<u8> = live_vehicles(&w)
            .iter()
            .map(|v| w.entities().owner()[v.index()].0)
            .collect();
        if owners.contains(&0) && owners.contains(&1) {
            break;
        }
    }
    assert!(live_vehicles(&w).len() >= 2, "both AIs field a tank");

    // A shell in flight (found by running on, up to 6 game minutes).
    let mut found = false;
    for _ in 0..30 * 360 {
        if !w.snapshot().shells.is_empty() {
            found = true;
            break;
        }
        w.step(&[]);
    }
    assert!(found, "a shell in flight within 6 minutes");
    println!("shell in flight at tick {}", w.tick());
    save_load_continue(&mut w, 1, 300, "shell in flight");

    // A burning vehicle, a bogged vehicle and a crew swap in progress:
    // staged on live tanks where the match has none.
    let v = live_vehicles(&w);
    assert!(!v.is_empty(), "a live vehicle to stage on");
    if !w.game().unwrap().vehicles().values().any(|s| s.burn > 0) {
        println!("burning: staged at tick {}", w.tick());
        w.edit_vehicle(v[0], |s| s.burn = 1);
    }
    save_load_continue(&mut w, 3, 300, "burning vehicle");
    let v = live_vehicles(&w);
    if !w.game().unwrap().vehicles().values().any(|s| s.bog > 0) {
        println!("bogged: staged at tick {}", w.tick());
        w.edit_vehicle(*v.last().unwrap(), |s| {
            s.bog = 450;
            s.crossing = true;
        });
    }
    save_load_continue(&mut w, 8, 200, "bogged vehicle");
    let v = live_vehicles(&w);
    if !w
        .game()
        .unwrap()
        .vehicles()
        .values()
        .any(|s| s.seats.iter().any(|x| x.swap > 0))
    {
        // A dead gunner: the next Status stage starts the swap. S3c: on
        // the best-manned tank, so the loss does not make its crew bail out.
        println!("crew swap: staged at tick {}", w.tick());
        let best = *v
            .iter()
            .max_by_key(|id| {
                let s = &w.game().unwrap().vehicles()[id];
                (
                    s.seats.iter().filter(|x| x.hp > 0).count(),
                    std::cmp::Reverse(**id),
                )
            })
            .unwrap();
        w.edit_vehicle(best, |s| s.seats[1].hp = 0);
        w.step(&[]);
    }
    assert!(
        w.game()
            .unwrap()
            .vehicles()
            .values()
            .any(|s| s.seats.iter().any(|x| x.swap > 0)),
        "a crew swap in progress"
    );
    save_load_continue(&mut w, 2, 200, "crew swap");

    // A pending vehicle path request: the AI re-paths its tanks every few
    // seconds; run until one waits in the queue at the end of a tick (S3b:
    // with re-call cooldowns the AI fields fewer squads, so path load is
    // lighter and the search window is 10 minutes).
    let mut pending = false;
    for _ in 0..30 * 600 {
        w.step(&[]);
        pending = w
            .game()
            .unwrap()
            .path_queue()
            .queues
            .iter()
            .flatten()
            .any(|r| matches!(r, PathReq::Vehicle { .. }));
        if pending {
            break;
        }
    }
    println!("pending vehicle path found by itself: {pending}");
    assert!(pending, "a pending vehicle path request");
    save_load_continue(&mut w, 3, 300, "pending vehicle path");
}

/// Flat staging ground with player 1 as the AI (`scenarios::staging`).
fn staged_ai(seed: u64) -> World {
    staged_ai_with(seed, |_| {})
}

/// [`staged_ai`] with `edit` applied to the setup first.
fn staged_ai_with(seed: u64, edit: impl FnOnce(&mut frontline_content::MatchSetup)) -> World {
    let mut s = setup(seed);
    frontline_sim::scenarios::staging(&mut s, true);
    s.players[1].is_ai = true;
    edit(&mut s);
    World::new_match(s, 1).unwrap()
}

#[test]
fn ai_repairs_an_immobilised_tank_and_retreats_a_beaten_one() {
    use frontline_content::ComponentKind;
    use frontline_sim::scenarios::{EAST, at, tank};
    let mut w = staged_ai(1);
    let (_, lame) = tank(&mut w, 1, at(500, 500), EAST).unwrap();
    let (beaten_sq, beaten) = tank(&mut w, 1, at(500, 560), EAST).unwrap();
    w.edit_vehicle(lame, |v| v.components[ComponentKind::TrackLeft.index()] = 0);
    w.edit_vehicle(beaten, |v| v.components[ComponentKind::Gun.index()] = 0);
    let mut repairing = false;
    let mut retreating = false;
    for _ in 0..30 * 20 {
        w.step(&[]);
        let g = w.game().unwrap();
        repairing |= g.vehicle(lame).is_some_and(|v| v.repairing.is_some());
        retreating |= g
            .squad(beaten_sq)
            .is_some_and(|q| q.order == frontline_sim::Order::Retreat);
    }
    let c = &w.game().unwrap().ai(PlayerId(1)).unwrap().counters;
    assert!(repairing && c.repairs >= 1, "{c:?}");
    assert!(retreating && c.retreats >= 1, "{c:?}");
}

#[test]
fn ai_engineers_satchel_a_stopped_tank_beside_its_track() {
    use frontline_content::{ComponentKind, SquadRole};
    use frontline_sim::scenarios::{NORTH, WEST, at, spawn, tank};
    // S3c: no bail-out for crew losses, so the driver-only target stays a
    // live (stopped) tank.
    let mut w = staged_ai_with(2, |s| s.rules.vehicles.bail.crew_below = 0);
    // A stopped human tank facing north; AI engineers 6 m east of its
    // right side, leader first.
    let (_, t) = tank(&mut w, 0, at(500, 500), NORTH).unwrap();
    // Its guns are out, so it cannot shoot the engineers first.
    w.edit_vehicle(t, |v| v.seats.iter_mut().skip(1).for_each(|s| s.hp = 0));
    let eng = spawn(&mut w, 1, SquadRole::Engineer, at(507, 506), WEST).unwrap();
    let lead = w.game().unwrap().squad(eng).unwrap().members[0];
    let ly = w.entities().transform()[lead.index()].pos.xy().y;
    let dy = ly - frontline_sim::Fx::from_int(500);
    w.place_squad(
        eng,
        frontline_sim::FxVec2::new(
            frontline_sim::Fx::from_int(507),
            frontline_sim::Fx::from_int(506) - dy,
        ),
        WEST,
    );
    let mut cut = false;
    for _ in 0..30 * 20 {
        w.step(&[]);
        let v = w.game().unwrap().vehicle(t).unwrap();
        cut |= v.component(ComponentKind::TrackRight) == 0
            || v.component(ComponentKind::TrackLeft) == 0;
    }
    let c = &w.game().unwrap().ai(PlayerId(1)).unwrap().counters;
    assert!(c.satchels_at_vehicles >= 1, "{c:?}");
    assert!(cut, "a track destroyed by the satchel");
}
