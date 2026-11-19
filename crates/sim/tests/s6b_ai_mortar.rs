//! S6b: mortar AI pass (docs/slices/S6b-mortar-ai.md): map fire only where
//! it can hit (C1), dry orders withdrawn (C3), determinism. Staged like `s5b_ai_mortar.rs`: flat
//! staging ground, player 1 the AI with no manpower (it buys nothing),
//! player 0 human; the AI mortar on its own support spot; the mortar's
//! leader observes only within 100 m (`observe_leader_m`).

use std::path::Path;

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::ai::{AiCounters, TaskKind};
use frontline_sim::scenarios::cmd;
use frontline_sim::unit::FireMode;
use frontline_sim::{CommandKind, Fx, FxVec2, Order, PlayerId, SimId, SquadId, World};

fn setup(edit: impl FnOnce(&mut MatchSetup)) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let mut defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let rules = defs
        .add_rules_override(&root.join("tests/rules_test_unlock.toml"))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    req.rules = rules;
    let mut s = defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
    // S11b Build C: written with every S11b switch off (shipped on).
    s.s11b_off();
    frontline_sim::scenarios::staging(&mut s, true);
    s.players[1].is_ai = true;
    s.rules.start_manpower = 0;
    // S11a2 Build B: the spotter scenes observe only to 100 m, inside the
    // `ff_safe` stand-off window (70-90 m); they keep the S5b stand-off.
    s.rules.ai.arty.ff_safe = 0;
    s.rules
        .fire_mission
        .as_mut()
        .expect("ww2 has [fire_mission]")
        .observe_leader_m = 100;
    edit(&mut s);
    s
}

fn counters(w: &World) -> AiCounters {
    w.game().unwrap().ai(PlayerId(1)).unwrap().counters.clone()
}

fn refused(w: &World) -> u32 {
    w.game().unwrap().players()[1].fm.refused.iter().sum()
}

fn mortar_type(w: &World) -> u16 {
    w.match_setup()
        .unwrap()
        .squad_type_index("ww2:usa_mortar_team")
        .unwrap()
}

/// The AI's support spot for a lone mortar, the AI's base and the point
/// the mortar supports (the spot lies `support_back_m` behind it).
fn support_spot(edit: fn(&mut MatchSetup)) -> (FxVec2, FxVec2, FxVec2) {
    let s = setup(edit);
    let back_m = s.rules.ai.infantry.support_back_m as i32;
    let mut w = World::new_match(s, 1).unwrap();
    let t = mortar_type(&w);
    let sq = w
        .spawn_squad_at(PlayerId(1), t, FxVec2::from_int(500, 500), FxVec2::ZERO)
        .unwrap();
    for _ in 0..120 {
        w.step(&[]);
        let ai = w.game().unwrap().ai(PlayerId(1)).unwrap();
        if let Some(task) = ai.tasks.get(&sq)
            && task.kind == TaskKind::Support
        {
            let base = w.game().unwrap().base_of(PlayerId(1));
            let back = (base - task.target).normalize_or_zero();
            let front = task.target - back * Fx::from_int(back_m);
            return (task.target, base, front);
        }
    }
    panic!("the AI never gave its mortar a support spot");
}

/// The AI mortar on its support spot (facing square to the line to its
/// base); with `enemy`, a rifle squad holding fire 130 m from it, square
/// to that line.
fn staged(
    edit: fn(&mut MatchSetup),
    threads: usize,
    enemy: bool,
) -> (World, SquadId, FxVec2, FxVec2) {
    let (spot, base, front) = support_spot(edit);
    let mut w = World::new_match(setup(edit), threads).unwrap();
    let back = (base - spot).normalize_or_zero();
    let side = FxVec2::new(-back.y, back.x);
    let t = mortar_type(&w);
    let mortar = w.spawn_squad_at(PlayerId(1), t, spot, side).unwrap();
    if enemy {
        let target = w.terrain().clamp(spot + side * Fx::from_int(130));
        hold_squad(&mut w, target, -side);
    }
    (w, mortar, spot, front)
}

/// Spawns a German rifle squad of player 0 at `at` and makes it hold fire.
fn hold_squad(w: &mut World, at: FxVec2, facing: FxVec2) -> SquadId {
    let rifle = w
        .match_setup()
        .unwrap()
        .squad_type_index("ww2:ger_grenadiergruppe")
        .unwrap();
    let enemy = w.spawn_squad_at(PlayerId(0), rifle, at, facing).unwrap();
    let men = w.game().unwrap().squad(enemy).unwrap().members.clone();
    w.step(&[cmd(
        0,
        0,
        CommandKind::SetFire {
            units: men,
            mode: FireMode::Hold,
        },
    )]);
    enemy
}

fn mission_mode(w: &World, sq: SquadId) -> Option<(u8, Option<SimId>)> {
    match &w.game().unwrap().squad(sq)?.order {
        Order::FireMission { m, .. } => Some((m.mode, m.observer)),
        _ => None,
    }
}

fn no_spots(s: &mut MatchSetup) {
    s.rules.ai.mortar.spot_samples = 0;
}

/// C1: no spotter can go and the target is map fire only; the ww2 limit
/// (`map_fire_max_miss_m` 20 m, map start error at least 27 m) holds it:
/// no mission, counted, nothing refused. (`s5b_ai_mortar.rs` checks that
/// map fire still goes with the limit off.)
#[test]
#[cfg_attr(debug_assertions, ignore = "long; run with --release")]
fn the_ai_orders_no_map_fire_that_would_miss_by_more_than_its_limit() {
    let (mut w, mortar, _, _) = staged(no_spots, 1, true);
    assert_eq!(
        w.match_setup().unwrap().rules.ai.mortar.map_fire_max_miss_m,
        20
    );
    for _ in 0..60 * 30 {
        w.step(&[]);
        assert!(
            mission_mode(&w, mortar).is_none(),
            "map fire past the limit"
        );
    }
    let c = counters(&w);
    assert!(c.fm_miss_holds > 0, "no miss hold counted");
    assert_eq!(c.fm_missions, [0, 0, 0]);
    assert_eq!(refused(&w), 0);
}

fn map_fire_on(s: &mut MatchSetup) {
    s.rules.ai.mortar.spot_samples = 0;
    s.rules.ai.mortar.map_fire_max_miss_m = 1000;
}

/// C3 (S5b-22): the AI orders a mission, then every man carrying bombs
/// falls during the reaction delay; the order is withdrawn when due, not
/// refused by the sim.
#[test]
#[cfg_attr(debug_assertions, ignore = "long; run with --release")]
fn a_fire_mission_whose_tube_ran_dry_while_waiting_is_withdrawn() {
    let (mut w, mortar, _, _) = staged(map_fire_on, 1, true);
    let mut killed = false;
    for _ in 0..60 * 30 {
        w.step(&[]);
        let waiting = w
            .game()
            .unwrap()
            .ai(PlayerId(1))
            .unwrap()
            .pending
            .iter()
            .any(|c| matches!(c.kind, CommandKind::FireMission { squad, .. } if squad == mortar));
        if waiting && !killed {
            let men = w.game().unwrap().squad(mortar).unwrap().members.clone();
            // Everyone but the leader (slot 0 carries no bombs).
            for m in men.into_iter().skip(1) {
                w.kill_soldier(m);
            }
            killed = true;
        }
        if killed && counters(&w).fm_dry_drops > 0 {
            break;
        }
    }
    assert!(killed, "the AI never ordered the mission");
    assert!(w.game().unwrap().squad(mortar).is_some(), "squad gone");
    assert_eq!(counters(&w).fm_dry_drops, 1);
    assert_eq!(refused(&w), 0, "the sim refused the AI's order");
    assert!(mission_mode(&w, mortar).is_none());
}

/// T12: with the S6b ww2 values (no reserve, map fire limit) and a
/// spotter out, the run is the same at 1, 2, 3 and 8 threads and through
/// save → load → continue.
#[test]
#[cfg_attr(debug_assertions, ignore = "long; run with --release")]
fn s6b_mortar_ai_is_deterministic_across_threads_and_saves() {
    fn ww2(_: &mut MatchSetup) {}
    let run = |threads: usize, save_at: Option<u64>| -> (Vec<u64>, AiCounters) {
        let (mut w, _, _, _) = staged(ww2, threads, true);
        let mut hashes = Vec::new();
        for k in 0..90 * 30u64 {
            w.step(&[]);
            if save_at == Some(k) {
                w = World::load(&w.save(), 3).unwrap();
            }
            if k % 30 == 0 {
                hashes.push(w.state_hash());
            }
        }
        assert_eq!(w.state_hash(), w.full_state_hash());
        (hashes, counters(&w))
    };
    let (reference, c) = run(1, None);
    assert!(c.fm_spotters >= 1 && c.fm_missions.iter().sum::<u32>() >= 1);
    for threads in [2, 3, 8] {
        assert_eq!(run(threads, None).0, reference, "{threads} threads");
    }
    assert_eq!(run(2, Some(40 * 30)).0, reference, "save, load, continue");
}

/// C4: the AI keeps one bearer squad per `mortars_per_bearers` mortar
/// squads (rounded up); 0 keeps the S6 rule (one). Three AI mortars, the
/// HQ squad, its HQ and the bearers free (no other card is affordable).
#[test]
#[cfg_attr(debug_assertions, ignore = "long; run with --release")]
fn the_ai_keeps_one_bearer_squad_per_two_mortars() {
    fn free_supply(s: &mut MatchSetup) {
        s.rules.ai.supply.hq_after_s = 0;
        for name in ["ww2:usa_ammo_bearers", "ww2:usa_platoon_hq"] {
            let t = s.squad_type_index(name).unwrap();
            s.squad_types[t as usize].cost = 0;
        }
        for st in &mut s.stocks {
            st.cost = 0;
        }
    }
    let bearers = |per: u32| -> usize {
        let mut s = setup(free_supply);
        s.rules.ai.supply.mortars_per_bearers = per;
        let mut w = World::new_match(s, 1).unwrap();
        let t = mortar_type(&w);
        for k in 0..3 {
            w.spawn_squad_at(
                PlayerId(1),
                t,
                FxVec2::from_int(500 + 20 * k, 500),
                FxVec2::ZERO,
            )
            .unwrap();
        }
        for _ in 0..120 * 30 {
            w.step(&[]);
        }
        let bt = w
            .match_setup()
            .unwrap()
            .squad_type_index("ww2:usa_ammo_bearers")
            .unwrap();
        w.game()
            .unwrap()
            .squads()
            .values()
            .filter(|q| q.owner == PlayerId(1) && q.type_idx == bt)
            .count()
    };
    assert_eq!(bearers(2), 2, "3 mortars, one bearer squad per 2");
    assert_eq!(bearers(0), 1, "0: one bearer squad (S6)");
}
