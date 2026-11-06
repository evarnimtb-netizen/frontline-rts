//! S7 AI habits (`docs/slices/S7-casemate-td.md` 6, `ai::overwatch`): an
//! assault gun holds an overwatch spot, fires and relocates; AI infantry
//! throws into a stopped open-topped enemy vehicle; an AI open-topped
//! vehicle reverses away from enemy infantry. Release only (AI plans are
//! slow in debug builds).

use std::path::Path;

use frontline_content::{Difficulty, ItemKind, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::{ArmourEventKind, Fx, FxVec2, PlayerId, SimId, SquadId, World};

/// Flat ground, the test unlock; player `ai` is the AI, it cannot buy.
fn setup(seed: u64, ai: u8) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let mut defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let rules = defs
        .add_rules_override(&root.join("tests/rules_test_unlock.toml"))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut req = MatchRequest::skirmish(seed, "ww2:germany", "ww2:usa", Difficulty::Normal);
    req.rules = rules;
    let mut s = defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
    for (k, p) in s.players.iter_mut().enumerate() {
        p.is_ai = k == ai as usize;
    }
    s.rules.start_manpower = 0;
    s.map.terrain.octaves.clear();
    s.map.terrain.bands.clear();
    s.map.woods.clear();
    s
}

fn xy(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

const EAST: FxVec2 = FxVec2::new(Fx::ONE, Fx::ZERO);
const WEST: FxVec2 = FxVec2::new(Fx::NEG_ONE, Fx::ZERO);

fn spawn(w: &mut World, player: u8, card: &str, at: FxVec2, f: FxVec2) -> (SquadId, SimId) {
    let t = w.match_setup().unwrap().squad_type_index(card).unwrap();
    let sq = w.spawn_squad_at(PlayerId(player), t, at, f).unwrap();
    (sq, w.game().unwrap().squad(sq).unwrap().members[0])
}

fn disarm(w: &mut World, id: SimId) {
    assert!(w.edit_vehicle(id, |v| {
        v.ammo.iter_mut().for_each(|n| *n = 0);
        v.loaded = None;
    }));
}

fn counters(w: &World, p: usize) -> frontline_sim::AiCounters {
    w.game()
        .unwrap()
        .ai(PlayerId(p as u8))
        .unwrap()
        .counters
        .clone()
}

fn pos(w: &World, id: SimId) -> FxVec2 {
    w.entities().transform()[id.index()].pos.xy()
}

#[test]
#[cfg_attr(debug_assertions, ignore = "AI plans: release only")]
fn an_assault_gun_holds_overwatch_fires_and_relocates() {
    let mut w = World::new_match(setup(3, 0), 1).unwrap();
    let (_, stug) = spawn(&mut w, 0, "ww2:ger_stug3_g_squad", xy(300, 500), EAST);
    let (_, sh) = spawn(&mut w, 1, "ww2:usa_m4a3_75_squad", xy(520, 520), WEST);
    disarm(&mut w, sh);
    // The target soaks hits: its crew and parts are far tougher than real.
    assert!(w.edit_vehicle(sh, |v| {
        v.seats.iter_mut().for_each(|s| s.hp = u16::MAX);
        v.components.iter_mut().for_each(|c| *c = u16::MAX);
    }));
    let mut fired = 0;
    for _ in 0..30 * 120 {
        w.step(&[]);
        fired += w
            .snapshot()
            .armour_events
            .iter()
            .filter(|e| e.kind == ArmourEventKind::Fired as u8 && e.vehicle == stug.0)
            .count();
        let c = counters(&w, 0);
        if c.relocations >= 1 && c.overwatch_holds >= 2 {
            break;
        }
    }
    let c = counters(&w, 0);
    let ow = w.game().unwrap().ai(PlayerId(0)).unwrap().overwatch.len();
    assert!(fired >= 2, "the StuG fired {fired} rounds");
    assert!(c.overwatch_holds >= 1, "never held a spot");
    assert!(c.relocations >= 1, "never relocated after {fired} shots");
    assert_eq!(ow, 1);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "AI plans: release only")]
fn ai_infantry_throws_into_a_stopped_open_top() {
    let mut w = World::new_match(setup(4, 0), 1).unwrap();
    let (_, m10) = spawn(&mut w, 1, "ww2:usa_m10_squad", xy(420, 500), WEST);
    disarm(&mut w, m10);
    let (sq, _) = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", xy(400, 500), EAST);
    // Grenades only (the Panzerfaust would make the crew bail).
    for m in w.game().unwrap().squad(sq).unwrap().members.clone() {
        w.edit_soldier(m, |k, _| k.items[ItemKind::SingleShotAt.index()] = 0);
    }
    for _ in 0..30 * 40 {
        w.step(&[]);
        if w.game().unwrap().players()[0].s7.open_top_throws > 0 {
            break;
        }
    }
    assert!(counters(&w, 0).open_top_throws >= 1, "no throw ordered");
    assert!(
        w.game().unwrap().players()[0].s7.open_top_throws >= 1,
        "nothing dropped in"
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore = "AI plans: release only")]
fn an_ai_open_top_backs_off_from_infantry() {
    let mut w = World::new_match(setup(5, 1), 1).unwrap();
    let (_, m10) = spawn(&mut w, 1, "ww2:usa_m10_squad", xy(500, 500), WEST);
    let (sq, man) = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", xy(455, 500), EAST);
    for m in w.game().unwrap().squad(sq).unwrap().members.clone() {
        w.edit_soldier(m, |k, _| k.items[ItemKind::SingleShotAt.index()] = 0);
    }
    let start = (pos(&w, m10) - pos(&w, man)).length();
    for _ in 0..30 * 20 {
        w.step(&[]);
    }
    let alive = w.game().unwrap().vehicle(m10).is_some();
    assert!(counters(&w, 1).open_top_backoffs >= 1, "never backed off");
    if alive {
        let now = (pos(&w, m10) - pos(&w, man)).length();
        assert!(now > start + Fx::from_int(10), "{start:?} -> {now:?}");
    }
}
