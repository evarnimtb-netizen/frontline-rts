//! S3c Part B AI armour habits (`docs/slices/S3c-armour.md` B1, B2, B5),
//! staged on flat ground with player 1 as the AI and no manpower to buy
//! with: a tank with dry MGs drives back to rearm and comes out again
//! (identical at 1, 2, 3 and 8 threads and across save → load → continue
//! mid-trip); an AI crew re-crews its abandoned tank while the crew of a
//! burning one walks back; an AI tank fires at an enemy abandoned tank;
//! a satchel hunt sticks to a tank that stops and starts and ends once it
//! drives off. The AI-vs-AI match in `ai_assault.rs` covers second bases,
//! hunts, re-crews and denial under determinism too.

use std::path::Path;

use frontline_content::{ComponentKind, Difficulty, MatchRequest, MatchSetup, ModDefs, SquadRole};
use frontline_sim::ai::TaskKind;
use frontline_sim::ai::{AiCounters, END_FAST, END_LEASH};
use frontline_sim::scenarios::{EAST, NORTH, WEST, at, cmd, spawn, tank};
use frontline_sim::{CommandKind, EntityKind, Order, PlayerId, SimId, SquadId, World};

/// Flat staging ground, player 1 the AI with no manpower (it buys
/// nothing), player 0 human.
fn setup() -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let mut defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let rules = defs
        .add_rules_override(&root.join("tests/rules_test_unlock.toml"))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    req.rules = rules;
    let mut s = defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
    frontline_sim::scenarios::staging(&mut s, true);
    s.players[1].is_ai = true;
    s.rules.start_manpower = 0;
    s
}

fn counters(w: &World) -> AiCounters {
    w.game().unwrap().ai(PlayerId(1)).unwrap().counters.clone()
}

fn task(w: &World, sq: SquadId) -> Option<TaskKind> {
    w.game()
        .unwrap()
        .ai(PlayerId(1))
        .unwrap()
        .tasks
        .get(&sq)
        .map(|t| t.kind)
}

/// Every limited MG mount of AI tank `v` dry.
fn dry_mgs(w: &mut World, v: SimId) {
    let s = w.match_setup().unwrap().clone();
    let ty = w.game().unwrap().vehicle(v).unwrap().type_idx as usize;
    let rounds: Vec<u32> = s.vehicles[ty]
        .mounts
        .iter()
        .skip(1)
        .map(|m| m.rounds.unwrap_or(0))
        .collect();
    w.edit_vehicle(v, |vs| {
        for (st, n) in vs.mgs.iter_mut().zip(&rounds) {
            st.spent = *n;
        }
    });
}

/// The rearm trip: `(squad, vehicle, world)` with the trip started, and
/// one hash per second for `secs` seconds after it.
fn rearm_run(threads: usize, save_at: Option<u64>, secs: u64) -> (Vec<u64>, AiCounters, bool) {
    let mut w = World::new_match(setup(), 1).unwrap();
    let (sq, v) = tank(&mut w, 1, at(700, 700), EAST).unwrap();
    dry_mgs(&mut w, v);
    if threads != 1 {
        w = World::load(&w.save(), threads).unwrap();
    }
    let mut hashes = Vec::new();
    let mut in_base = false;
    for k in 0..secs * 30 {
        if save_at == Some(k) {
            w = World::load(&w.save(), 3).unwrap();
        }
        w.step(&[]);
        let g = w.game().unwrap();
        in_base |= g.vehicle(v).is_some_and(|vs| vs.base_ticks > 0);
        if w.tick().is_multiple_of(30) {
            hashes.push(w.state_hash());
        }
        let _ = g.squad(sq);
    }
    (hashes, counters(&w), in_base)
}

#[test]
fn a_tank_with_dry_mgs_drives_back_rearms_and_returns() {
    let mut w = World::new_match(setup(), 1).unwrap();
    let (sq, v) = tank(&mut w, 1, at(700, 700), EAST).unwrap();
    dry_mgs(&mut w, v);
    let mut started = false;
    let mut arrived = None;
    for _ in 0..30 * 60 {
        w.step(&[]);
        started |= task(&w, sq) == Some(TaskKind::Rearm)
            && w.game().unwrap().squad(sq).unwrap().order == Order::Retreat;
        if arrived.is_none() && w.game().unwrap().vehicle(v).unwrap().base_ticks > 0 {
            arrived = Some(w.tick());
        }
    }
    assert!(started, "a rearm trip ({:?})", counters(&w));
    assert_eq!(counters(&w).rearms, 1);
    let arrived = arrived.expect("the tank reached its base");
    // It waits in base until rearmed (`rearm_full_pct`) or `rearm_max_s`,
    // then takes a task again.
    let max_s = w.match_setup().unwrap().rules.ai.armour.rearm_max_s as u64;
    let mut left = false;
    while w.tick() < arrived + 30 * (max_s + 30) {
        w.step(&[]);
        if task(&w, sq) != Some(TaskKind::Rearm) {
            left = true;
            break;
        }
    }
    assert!(left, "the rearm task ended within rearm_max_s");
    let vs = w.game().unwrap().vehicle(v).unwrap();
    let s = w.match_setup().unwrap();
    let ty = &s.vehicles[vs.type_idx as usize];
    // Every MG has rounds again (`dry_mgs` emptied them all).
    let stowed: Vec<(u32, u32)> = ty
        .mounts
        .iter()
        .skip(1)
        .zip(&vs.mgs)
        .filter_map(|(m, st)| m.rounds.map(|n| (n, st.spent)))
        .collect();
    assert!(!stowed.is_empty());
    assert!(
        stowed.iter().all(|&(n, spent)| spent < n),
        "belts were refilled in base: {stowed:?}"
    );
    assert_eq!(counters(&w).rearms, 1, "one trip, no new one once rearmed");
    // And it goes back out: it leaves the base within a minute.
    let p = w.match_setup().unwrap().players[1].base as usize;
    let base = w.match_setup().unwrap().map.bases[p].clone();
    let r: frontline_sim::Fx = base.radius.into();
    let c = frontline_sim::FxVec2::from(base.center);
    let out = (0..30 * 60).any(|_| {
        w.step(&[]);
        let pos = w.entities().transform()[v.index()].pos.xy();
        (pos - c).length() > r
    });
    assert!(out, "the tank returns to the field after rearming");
}

#[test]
fn rearm_trips_are_deterministic_across_threads_and_save_load() {
    let (want, c, in_base) = rearm_run(1, None, 70);
    assert!(c.rearms == 1 && in_base, "{c:?}");
    for threads in [2, 3, 8] {
        assert_eq!(rearm_run(threads, None, 70).0, want, "{threads} threads");
    }
    // Saved mid-trip (driving back) and loaded at 3 threads.
    assert_eq!(
        rearm_run(1, Some(30 * 20), 70).0,
        want,
        "save, load, continue"
    );
}

/// Abandons AI tank `v` (the owner's order) and waits for its crew squad.
fn bail(w: &mut World, v: SimId) -> SquadId {
    w.edit_vehicle(v, |vs| vs.bail_order = true);
    for _ in 0..30 * 10 {
        w.step(&[]);
        let g = w.game().unwrap();
        if let Some(c) = g.vehicle(v).unwrap().crew_squad
            && g.vehicle(v).unwrap().exit.iter().all(|&e| e == 0)
        {
            return c;
        }
    }
    panic!("no crew squad");
}

#[test]
fn an_ai_crew_re_crews_its_tank_and_a_burning_tanks_crew_walks_back() {
    let mut s = setup();
    // The ammo fire must not end the second tank before its crew is out.
    s.rules.vehicles.damage.ammo_fire_s = 60;
    let mut w = World::new_match(s, 1).unwrap();
    let (_, v) = tank(&mut w, 1, at(600, 600), EAST).unwrap();
    let (_, hot) = tank(&mut w, 1, at(600, 700), EAST).unwrap();
    let crew = bail(&mut w, v);
    w.edit_vehicle(hot, |vs| vs.ammo_fire = 1);
    let mut hot_crew = None;
    let mut recrewed = false;
    for _ in 0..30 * 60 {
        w.step(&[]);
        let g = w.game().unwrap();
        hot_crew = hot_crew.or(g.vehicle(hot).and_then(|vs| vs.crew_squad));
        if w.entities().kind()[v.index()] == EntityKind::Vehicle {
            recrewed = true;
        }
    }
    let c = counters(&w);
    assert!(recrewed, "the AI crew re-crewed its tank ({c:?})");
    assert_eq!(w.entities().owner()[v.index()], PlayerId(1));
    assert!(w.game().unwrap().squad(crew).is_none(), "all five boarded");
    assert!(c.recrews >= 1 && c.crew_retreats >= 1, "{c:?}");
    // The burning tank's crew walks back, never to it.
    let hc = hot_crew.expect("the burning tank's crew got out");
    if let Some(q) = w.game().unwrap().squad(hc) {
        assert_eq!(q.order, Order::Retreat);
    }
    assert_ne!(w.entities().kind()[hot.index()], EntityKind::Vehicle);
}

#[test]
fn an_ai_tank_fires_at_an_enemy_abandoned_tank() {
    let mut w = World::new_match(setup(), 1).unwrap();
    let (_, mine) = tank(&mut w, 1, at(500, 500), WEST).unwrap();
    // The human tank 200 m west, its crew all dead: abandoned intact
    // (S3c-12 (A)), nobody gets out.
    let (_, theirs) = tank(&mut w, 0, at(300, 500), EAST).unwrap();
    w.edit_vehicle(theirs, |vs| vs.seats.iter_mut().for_each(|s| s.hp = 0));
    let mut ordered = false;
    for _ in 0..30 * 30 {
        w.step(&[]);
        ordered |= w.game().unwrap().vehicle(mine).unwrap().attack == Some(theirs);
    }
    let c = counters(&w);
    assert!(ordered && c.denials >= 1, "{c:?}");
}

#[test]
fn a_satchel_hunt_sticks_through_a_stop_and_start_and_ends_when_it_drives_off() {
    let mut s = setup();
    // The human tank must not bail out with its gunners dead.
    s.rules.vehicles.bail.crew_below = 0;
    let mut w = World::new_match(s, 1).unwrap();
    let (tsq, t) = tank(&mut w, 0, at(500, 500), NORTH).unwrap();
    // Driver only: it can move but not shoot.
    w.edit_vehicle(t, |v| v.seats.iter_mut().skip(1).for_each(|s| s.hp = 0));
    let eng = spawn(&mut w, 1, SquadRole::Engineer, at(580, 500), WEST).unwrap();
    let hunt = |w: &World| {
        w.game()
            .unwrap()
            .ai(PlayerId(1))
            .unwrap()
            .hunts
            .get(&eng)
            .map(|h| h.vehicle)
    };
    // It starts once the tank has been still for `satchel_slow_s`.
    let mut started = false;
    for _ in 0..30 * 8 {
        w.step(&[]);
        started |= hunt(&w) == Some(t);
    }
    assert!(started, "a hunt started ({:?})", counters(&w));
    // A short drive (under `satchel_chase_s`) and a stop: still hunted.
    let mut seq = 0;
    let mut go = |w: &mut World, y: i32| {
        seq += 1;
        w.step(&[cmd(
            0,
            seq,
            CommandKind::Move {
                squad: tsq,
                target: at(500, y),
            },
        )]);
    };
    go(&mut w, 520);
    for _ in 0..30 * 4 {
        w.step(&[]);
    }
    assert_eq!(hunt(&w), Some(t), "kept through a short drive");
    // Unless a satchel got it meanwhile, it drives off for good: the hunt
    // ends (too fast too long, or out of the leash).
    let tracks_ok = |w: &World| {
        let v = w.game().unwrap().vehicle(t).unwrap();
        v.component(ComponentKind::TrackLeft) > 0 && v.component(ComponentKind::TrackRight) > 0
    };
    if !tracks_ok(&w) {
        return;
    }
    go(&mut w, 1000);
    for _ in 0..30 * 30 {
        w.step(&[]);
        if hunt(&w).is_none() {
            break;
        }
    }
    let c = counters(&w);
    if tracks_ok(&w) {
        assert_eq!(hunt(&w), None, "{c:?}");
        assert!(c.hunt_ends[END_FAST] + c.hunt_ends[END_LEASH] >= 1, "{c:?}");
    }
}
