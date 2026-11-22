//! B3 code tests (docs/slices/B3-log.md): `[ai.arty] direct_in_budget`,
//! an AI gun's direct fire inside the A5 shell budget, and the SAVE 32
//! layout. Each test names its control: the edit that makes it fail.

use super::s6_tests::{flat_world_with, members, spawn, steps};
use super::*;

const LEIG: &str = "ww2:ger_leig18_squad";

/// A flat match whose player 0 (Germany) is an AI with an emplaced leIG at
/// (100, 500) facing +x; `[ai.arty] direct_in_budget` is `on`,
/// `shell_mp_max_pct` `pct`, and 400 MP of gun shells are already bought
/// (67 % of the 600 MP income). After 10 s (the AI's plans have run) a US
/// rifle squad with no rounds appears `dx` m in front of the
/// gun. Returns the world 30 s later and the gun.
fn scene(on: u32, pct: u32, dx: i32) -> (World, SimId) {
    let mut w = flat_world_with(1, |s| {
        s.players[0].is_ai = true;
        let a = &mut s.rules.ai.arty;
        (a.buy, a.fo, a.max_guns, a.after_s) = (1, 0, 0, 0);
        a.direct_in_budget = on;
        a.shell_mp_max_pct = pct;
    });
    w.state.game.as_mut().unwrap().players[0].manpower = 5000;
    steps(&mut w, 2);
    let q = spawn(&mut w, 0, LEIG, 100, 500);
    let gun = members(&w, q)[0];
    w.state.game.as_mut().unwrap().players[0]
        .s11
        .gun_shell_micro = 400 * crate::supply::MICRO;
    steps(&mut w, 30 * 10);
    let e = spawn(&mut w, 1, "ww2:usa_rifle_squad", 100 + dx, 500);
    for m in members(&w, e) {
        w.edit_soldier(m, |k, c| {
            k.rounds = [0; frontline_content::setup::MAX_FEEDS];
            c.mag = 0;
        });
    }
    steps(&mut w, 30 * 30);
    (w, gun)
}

/// (direct rounds of player 0's guns, `arty_direct_holds`, the gun's
/// `hold_fire_m`).
fn read(w: &World, gun: SimId) -> (u32, u32, u16) {
    let g = w.game().unwrap();
    (
        g.players[0].s11.gun_direct_rounds,
        g.ai[0].as_ref().unwrap().counters.arty_direct_holds,
        g.vehicles
            .get(&gun)
            .and_then(|v| v.served.as_ref())
            .map_or(0, |s| s.hold_fire_m),
    )
}

/// On: over the budget the AI gun holds its direct fire at men 100 m off
/// (no direct round, a hold of `self_defence_m`, holds counted). Controls:
/// the switch 0 (the gun fires over open sights, no hold, no count), and
/// the switch 1 inside the budget (`shell_mp_max_pct` 100: no hold).
#[test]
fn direct_budget_holds_ai_gun_direct_fire_over_budget() {
    let (w, gun) = scene(1, 25, 100);
    let sd = w.match_setup().unwrap().rules.vehicles.guns.self_defence_m as u16;
    let (rounds, holds, hold) = read(&w, gun);
    assert_eq!(rounds, 0, "held: no direct round");
    assert!(holds > 0);
    assert_eq!(hold, sd.max(1));
    let (w, gun) = scene(0, 25, 100);
    let (rounds, holds, hold) = read(&w, gun);
    assert!(rounds > 0, "control: switch 0 fires over open sights");
    assert_eq!((holds, hold), (0, 0), "control: switch 0 holds nothing");
    let (w, gun) = scene(1, 100, 100);
    let (_, holds, hold) = read(&w, gun);
    assert_eq!((holds, hold), (0, 0), "control: inside the budget no hold");
}

/// On and over the budget, men inside `self_defence_m` (40 m) are still
/// fired at. Control: the same men at 100 m are not (the hold is on).
#[test]
fn direct_budget_keeps_self_defence_fire() {
    let (w, gun) = scene(1, 25, 36);
    let (rounds, holds, _) = read(&w, gun);
    assert!(holds > 0, "the gun is under the hold");
    assert!(rounds > 0, "self-defence at 36 m fires");
    let (w, gun) = scene(1, 25, 100);
    assert_eq!(read(&w, gun).0, 0, "control: 100 m is held");
}

/// SAVE 32: `arty_direct_holds` saves, loads and is hashed; the setup key
/// is in the setup bytes and 2 is refused. Control: an unedited reload
/// keeps the hash, and `direct_in_budget` 1 is valid.
#[test]
fn direct_budget_layout_roundtrips() {
    let (w, _) = scene(1, 25, 100);
    let bytes = w.save();
    let back = World::load(&bytes, 1).expect("the B3 state loads");
    assert_eq!(back.game().unwrap().ai[0], w.game().unwrap().ai[0]);
    assert_eq!(back.full_state_hash(), w.full_state_hash());
    assert_eq!(back.save(), bytes);
    let h = back.full_state_hash();
    let mut x = World::load(&bytes, 1).unwrap();
    x.state.game.as_mut().unwrap().ai[0]
        .as_mut()
        .unwrap()
        .counters
        .arty_direct_holds += 1;
    let y = World::load(&x.save(), 1).unwrap();
    assert_ne!(y.full_state_hash(), h, "arty_direct_holds is not hashed");
    let base = flat_world_with(1, |_| {}).game().unwrap().setup.clone();
    assert_eq!(base.rules.ai.arty.direct_in_budget, 0, "ww2 ships it 0");
    let h0 = postcard::to_allocvec(&base).unwrap();
    let mut s = base.clone();
    s.rules.ai.arty.direct_in_budget = 1;
    assert_ne!(
        postcard::to_allocvec(&s).unwrap(),
        h0,
        "not in the setup bytes"
    );
    assert!(s.validate().is_empty(), "control: 1 is valid");
    s.rules.ai.arty.direct_in_budget = 2;
    assert!(!s.validate().is_empty(), "direct_in_budget 2 is accepted");
}
