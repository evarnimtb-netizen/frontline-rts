//! S5b Part B: AI fire missions (docs/slices/S5b-mortars.md "AI mission
//! logic", targets T4 no blind fire, T11 missions through observers, T12
//! determinism with a spotter out). Flat staging ground, player 1 the AI
//! with no manpower (it buys nothing), player 0 human.
//!
//! The AI's mortar stands where its planner wants it (its support spot, read
//! from a first run), so it neither walks nor moves its tube; an enemy
//! squad holding fire is placed 130 m from the gunner, square to the line
//! toward the front point (away from the AI's own squads). The mortar
//! team's own sight (150 m) spots it; the mortar's leader observes only
//! within 100 m here (`observe_leader_m`), so from the tube the mission
//! would be map fire.

use std::path::Path;

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::ai::{AiCounters, TaskKind};
use frontline_sim::fire_mission::mode;
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

fn mortar_type(w: &World) -> u16 {
    w.match_setup()
        .unwrap()
        .squad_type_index("ww2:usa_mortar_team")
        .unwrap()
}

/// Where the AI wants its mortar (its support spot) and toward which point.
fn support_spot(edit: fn(&mut MatchSetup)) -> (FxVec2, FxVec2) {
    let mut w = World::new_match(setup(edit), 1).unwrap();
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
            return (task.target, base);
        }
    }
    panic!("the AI never gave its mortar a support spot");
}

/// The staged world: the AI mortar on its support spot, an enemy rifle
/// squad holding fire 130 m from it, square to the line to the AI's base.
fn staged(edit: fn(&mut MatchSetup), threads: usize) -> (World, SquadId, SquadId) {
    let (spot, base) = support_spot(edit);
    let mut w = World::new_match(setup(edit), threads).unwrap();
    let back = (base - spot).normalize_or_zero();
    let side = FxVec2::new(-back.y, back.x);
    let t = mortar_type(&w);
    let mortar = w.spawn_squad_at(PlayerId(1), t, spot, side).unwrap();
    let rifle = w
        .match_setup()
        .unwrap()
        .squad_type_index("ww2:ger_grenadiergruppe")
        .unwrap();
    let target = w.terrain().clamp(spot + side * Fx::from_int(130));
    let enemy = w.spawn_squad_at(PlayerId(0), rifle, target, -side).unwrap();
    let men = w.game().unwrap().squad(enemy).unwrap().members.clone();
    w.step(&[cmd(
        0,
        0,
        CommandKind::SetFire {
            units: men,
            mode: FireMode::Hold,
        },
    )]);
    (w, mortar, enemy)
}

fn no_edit(_: &mut MatchSetup) {}

/// The spotter's man, while one is out.
fn spotter(w: &World, sq: SquadId) -> Option<SimId> {
    w.game()
        .unwrap()
        .ai(PlayerId(1))
        .unwrap()
        .spotters
        .get(&sq)
        .filter(|s| s.out())
        .map(|s| s.man)
}

fn mission_mode(w: &World, sq: SquadId) -> Option<(u8, Option<SimId>)> {
    match &w.game().unwrap().squad(sq)?.order {
        Order::FireMission { m, .. } => Some((m.mode, m.observer)),
        _ => None,
    }
}

/// T11: the tube team cannot observe the target (its leader observes only
/// within 100 m here), so the AI sends the leader forward on the line to
/// the target, inside the voice link; once he is there the mission is
/// Observed, through him; the sim refuses nothing.
#[test]
#[cfg_attr(debug_assertions, ignore = "long; run with --release")]
fn the_ai_sends_a_spotter_and_fires_an_observed_mission_through_him() {
    spotter_case(no_edit);
}

/// S11a2 (fix B-7): the same with `[ai.arty] ff_safe` 1, the spotter's
/// stand-off then `danger_close_m + 20` (70 m) short of the target; the
/// leader observes to 150 m here so a stand-off exists (70-140 m).
#[test]
#[cfg_attr(debug_assertions, ignore = "long; run with --release")]
fn the_ai_sends_a_spotter_with_ff_safe() {
    spotter_case(ff_safe_on);
}

fn ff_safe_on(s: &mut MatchSetup) {
    s.rules.ai.arty.ff_safe = 1;
    s.rules.fire_mission.as_mut().unwrap().observe_leader_m = 150;
}

fn spotter_case(edit: fn(&mut MatchSetup)) {
    let (mut w, mortar, _) = staged(edit, 1);
    let mut sent = None;
    let mut observed = None;
    for _ in 0..120 * 30 {
        w.step(&[]);
        if sent.is_none() {
            sent = spotter(&w, mortar);
            // Before he goes, nothing is fired: no map fire while a spotter
            // can be sent.
            if sent.is_none() {
                assert!(
                    mission_mode(&w, mortar).is_none(),
                    "fired before the spotter went"
                );
            }
        }
        if let Some((md, obs)) = mission_mode(&w, mortar) {
            observed = Some((md, obs));
            break;
        }
    }
    let man = sent.expect("no spotter was sent");
    let (md, obs) = observed.expect("no mission after the spotter went out");
    assert_eq!(md, mode::OBSERVED, "the mission is not observed");
    assert_eq!(obs, Some(man), "observed by someone else");
    let ents = w.entities();
    assert!(ents.unit()[man.index()].detached, "the spotter stays out");
    let c = counters(&w);
    assert_eq!(c.fm_spotters, 1);
    assert_eq!(c.fm_spotter_missions, 1);
    assert_eq!(c.fm_missions, [0, 1, 0]);
    let g = w.game().unwrap();
    assert_eq!(g.players()[1].fm.refused.iter().sum::<u32>(), 0);
    // The spotter stood inside the voice link of the gunner.
    let sq = g.squad(mortar).unwrap();
    let gunner = g.mortar_gunner(ents, sq).unwrap();
    let d = (ents.transform()[man.index()].pos.xy() - ents.transform()[gunner.index()].pos.xy())
        .length();
    let link = w
        .match_setup()
        .unwrap()
        .rules
        .fire_mission
        .as_ref()
        .unwrap()
        .observer_link_m;
    assert!(
        d <= Fx::from_int(link as i32),
        "spotter {d:?} from the gunner"
    );
}

/// No spotter can be sent (`spot_samples` 0): the AI falls back to map fire
/// while it holds more than `map_fire_min_pct` of its bombs ...
#[test]
#[cfg_attr(debug_assertions, ignore = "long; run with --release")]
fn without_a_spotter_the_ai_falls_back_to_map_fire_above_its_reserve() {
    fn no_spots(s: &mut MatchSetup) {
        s.rules.ai.mortar.spot_samples = 0;
        // S6b: the S5b map-fire rules (no miss limit).
        s.rules.ai.mortar.map_fire_max_miss_m = 1000;
    }
    let (mut w, mortar, _) = staged(no_spots, 1);
    let mut fired = None;
    for _ in 0..60 * 30 {
        w.step(&[]);
        if let Some(m) = mission_mode(&w, mortar) {
            fired = Some(m);
            break;
        }
    }
    assert_eq!(fired.map(|m| m.0), Some(mode::MAP));
    assert_eq!(counters(&w).fm_missions, [0, 0, 1]);
    assert_eq!(counters(&w).fm_spotters, 0);
    assert_eq!(
        w.game().unwrap().players()[1]
            .fm
            .refused
            .iter()
            .sum::<u32>(),
        0
    );
}

/// ... and holds it (no mission at all) at or below that share; and a
/// mortar that knows of no enemy fires at nothing (no blind fire).
#[test]
#[cfg_attr(debug_assertions, ignore = "long; run with --release")]
fn the_ai_keeps_its_map_fire_reserve_and_never_fires_blind() {
    fn no_spots_high_reserve(s: &mut MatchSetup) {
        s.rules.ai.mortar.spot_samples = 0;
        // 20 bombs held = 100 %: hold map fire at every share.
        s.rules.ai.mortar.map_fire_min_pct = 100;
        s.rules.ai.mortar.map_fire_max_miss_m = 1000;
    }
    let (mut w, mortar, _) = staged(no_spots_high_reserve, 1);
    for _ in 0..60 * 30 {
        w.step(&[]);
        assert!(
            mission_mode(&w, mortar).is_none(),
            "map fire below the reserve"
        );
    }
    assert!(counters(&w).fm_map_holds > 0);
    assert_eq!(counters(&w).fm_missions, [0, 0, 0]);
    // No enemy known: nothing at all, for two minutes.
    let (spot, base) = support_spot(no_edit);
    let mut w = World::new_match(setup(no_edit), 1).unwrap();
    let back = (base - spot).normalize_or_zero();
    let t = mortar_type(&w);
    let mortar = w
        .spawn_squad_at(PlayerId(1), t, spot, FxVec2::new(-back.y, back.x))
        .unwrap();
    for _ in 0..120 * 30 {
        w.step(&[]);
        assert!(
            mission_mode(&w, mortar).is_none(),
            "fired at unknown ground"
        );
    }
    let c = counters(&w);
    assert_eq!(c.fm_missions, [0, 0, 0]);
    assert_eq!(c.fm_spotters, 0);
    assert_eq!(
        w.game().unwrap().players()[1]
            .fm
            .refused
            .iter()
            .sum::<u32>(),
        0
    );
}

/// T12: with a spotter out (walking, then at his spot), the run is the same
/// at 1, 2, 3 and 8 threads and through save → load → continue.
#[test]
#[cfg_attr(debug_assertions, ignore = "long; run with --release")]
fn ai_spotters_are_deterministic_across_threads_and_saves() {
    let run = |threads: usize, save_when_out: bool| -> (Vec<u64>, AiCounters) {
        let (mut w, mortar, _) = staged(no_edit, threads);
        let mut hashes = Vec::new();
        let mut saved = false;
        for k in 0..90 * 30u64 {
            w.step(&[]);
            if save_when_out && !saved && spotter(&w, mortar).is_some() {
                w = World::load(&w.save(), 3).unwrap();
                saved = true;
            }
            if k % 30 == 0 {
                hashes.push(w.state_hash());
            }
        }
        assert!(!save_when_out || saved, "no spotter went out");
        assert_eq!(w.state_hash(), w.full_state_hash());
        (hashes, counters(&w))
    };
    let (reference, c) = run(1, false);
    assert!(c.fm_spotters >= 1 && c.fm_spotter_missions >= 1);
    for threads in [2, 3, 8] {
        assert_eq!(run(threads, false).0, reference, "{threads} threads");
    }
    assert_eq!(
        run(2, true).0,
        reference,
        "save and load with a spotter out"
    );
}
