//! S11a2 tests (docs/slices/S11a2.md 3, 4; Build A): the SAVE 28 layout
//! and its load checks, friendly fire of mission rounds (F1-F4) and
//! counter-battery by sound (K1-K4). Each test names its control: the edit
//! that makes it fail.

use super::s6_tests::{spawn, steps};
use super::s11a1_tests::{Scene, TARGET, mission, order, scene_with};
use super::*;
use crate::ai::AiArty;
use crate::counter_battery::{CbSpot, MAX_CB_SPOTS};

fn c(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

// ------------------------------------------------------------ layout --

/// The S11a2 staged save: the S11a1 gun scene (player 0's test gun on a
/// mission flagged `cb`), player 1 an AI holding a counter-fire record,
/// player 1 holding one marked and one pending spot of player 0's gun, and
/// a few S11a2 counters set. Returns `(world, gun squad, AI squad)`.
fn staged() -> (World, SquadId, SquadId) {
    let Scene { mut w, sq, .. } = scene_with(1, 0, |s| s.players[1].is_ai = true);
    assert_eq!(order(&mut w, sq, c(TARGET.0, TARGET.1), 4), None);
    let q1 = spawn(&mut w, 1, "ww2:usa_rifle_squad", 900, 900);
    steps(&mut w, 5);
    assert!(w.edit_squad(sq, |q| {
        if let Order::FireMission { m, .. } = &mut q.order {
            m.cb = true;
        }
    }));
    assert!(mission(&w, sq).unwrap().cb);
    let tick = w.tick();
    let hz = 30;
    let sec = tick.div_ceil(hz) * hz;
    assert!(w.with_ai_state(1, |ai| {
        ai.arty.insert(
            q1,
            AiArty {
                since: tick,
                last_order: tick,
                cb: true,
                cb_shots: 4,
                ..AiArty::default()
            },
        );
        ai.counters.ff_lifts = 1;
        ai.counters.cb_masked = 2;
    }));
    assert!(w.edit_player(PlayerId(1), |p| {
        p.cb = vec![
            CbSpot {
                of: 0,
                pos: c(300, 500),
                at: c(330, 520),
                err_mm: 120_000,
                heard_mm: 800_000,
                evidence: 4,
                shots: 4,
                decay_at: sec + 300 * hz,
                mark_at: 0,
                marked: true,
            },
            CbSpot {
                of: 0,
                pos: c(500, 200),
                at: c(500, 200),
                err_mm: 0,
                heard_mm: 0,
                evidence: 3,
                shots: 3,
                decay_at: sec + 280 * hz,
                mark_at: sec + 15 * hz,
                marked: false,
            },
        ];
        p.s11a2.cb_marks = 1;
        p.s11a2.cb_err_first_mm = 120_000;
    }));
    assert!(w.edit_player(PlayerId(0), |p| {
        p.s11a2.ff_kills = [1, 2];
        p.s11a2.cb_rounds[2] = 3;
    }));
    (w, sq, q1)
}

fn game_mut(s: &mut SimState) -> &mut MatchState {
    s.game.as_mut().unwrap()
}

fn ai1(s: &mut SimState) -> &mut crate::ai::AiState {
    game_mut(s).ai[1].as_mut().unwrap()
}

fn mission_mut(s: &mut SimState, q: SquadId) -> &mut crate::fire_mission::Mission {
    match &mut game_mut(s).squads.get_mut(&q).unwrap().order {
        Order::FireMission { m, .. } => m,
        _ => panic!("not on a mission"),
    }
}

/// Spec 3: the S11a2 state saves, loads and hashes. Control: an unedited
/// reload keeps the hash (each edit must change it).
#[test]
fn s11a2_layout_roundtrips_and_hashes() {
    let (w, sq, q1) = staged();
    let bytes = w.save();
    let back = World::load(&bytes, 1).expect("the S11a2 state loads");
    assert_eq!(back.game().unwrap().players(), w.game().unwrap().players());
    assert_eq!(back.game().unwrap().ai[1], w.game().unwrap().ai[1]);
    assert_eq!(back.full_state_hash(), w.full_state_hash());
    assert_eq!(back.save(), bytes);
    let h = back.full_state_hash();
    type Edit = Box<dyn Fn(&mut SimState)>;
    let edits: Vec<Edit> = vec![
        Box::new(move |s| mission_mut(s, sq).cb = false),
        Box::new(move |s| ai1(s).arty.get_mut(&q1).unwrap().cb = false),
        Box::new(move |s| ai1(s).arty.get_mut(&q1).unwrap().cb_shots += 1),
        Box::new(|s| ai1(s).counters.ff_holds += 1),
        Box::new(|s| ai1(s).counters.cb_stale_holds += 1),
        Box::new(|s| game_mut(s).players[0].s11a2.ff_wounds += 1),
        Box::new(|s| game_mut(s).players[0].s11a2.cb_near_bursts += 1),
        Box::new(|s| game_mut(s).players[1].cb[0].err_mm -= 1),
        Box::new(|s| game_mut(s).players[1].cb[0].at.x += Fx::ONE),
        Box::new(|s| game_mut(s).players[1].cb[1].evidence -= 1),
        Box::new(|s| game_mut(s).players[1].cb[1].mark_at -= 30),
        Box::new(|s| {
            game_mut(s).players[1].cb.pop();
        }),
    ];
    assert_eq!(World::load(&back.save(), 1).unwrap().full_state_hash(), h);
    for (k, e) in edits.into_iter().enumerate() {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        let y = World::load(&x.save(), 1).unwrap_or_else(|e| panic!("edit {k}: {e:?}"));
        assert_ne!(y.full_state_hash(), h, "edit {k} is not hashed");
    }
}

/// Spec 3 load validation, one case per check. Control: the staged save
/// itself loads (checked first), so each failure is the edit's.
#[test]
fn load_rejects_bad_s11a2_state() {
    let (mut w, _sq, _) = staged();
    // A mortar team on its own mission (for the `Mission::cb` check).
    let mortar = spawn(&mut w, 0, "ww2:ger_mortar_team", 320, 520);
    steps(&mut w, 30);
    w.mark_intel(PlayerId(0), c(600, 520));
    assert_eq!(order(&mut w, mortar, c(600, 520), 4), None);
    assert!(mission(&w, mortar).is_some(), "the mortar is on a mission");
    let bytes = w.save();
    World::load(&bytes, 1).expect("control: the staged save loads");
    let tick = w.tick();
    let hz = 30u64;
    let size_mm = 1_024_000 + 1;
    type Edit = Box<dyn Fn(&mut SimState)>;
    fn sp(s: &mut SimState, k: usize) -> &mut CbSpot {
        &mut s.game.as_mut().unwrap().players[1].cb[k]
    }
    let cases: Vec<(&str, Edit)> = vec![
        (
            "more spots than the cap",
            Box::new(|s| {
                let c0 = *sp(s, 0);
                let v = &mut game_mut(s).players[1].cb;
                while v.len() <= MAX_CB_SPOTS {
                    v.push(c0);
                }
            }),
        ),
        ("of names no player", Box::new(|s| sp(s, 0).of = 2)),
        ("of is the holder", Box::new(|s| sp(s, 0).of = 1)),
        ("pos off the map", Box::new(|s| sp(s, 0).pos = c(-1, 10))),
        ("at off the map", Box::new(|s| sp(s, 0).at = c(10, 2000))),
        ("err_mm past the map", Box::new(move |s| sp(s, 0).err_mm = size_mm)),
        (
            "heard_mm past the map",
            Box::new(move |s| sp(s, 0).heard_mm = size_mm),
        ),
        ("no evidence", Box::new(|s| sp(s, 0).evidence = 0)),
        (
            "decay_at a second past",
            Box::new(move |s| sp(s, 0).decay_at = tick - hz),
        ),
        (
            "decay_at too far ahead",
            Box::new(move |s| sp(s, 0).decay_at = tick + 301 * hz + 1),
        ),
        (
            "mark_at a second past",
            Box::new(move |s| sp(s, 1).mark_at = tick - hz),
        ),
        (
            "mark_at too far ahead",
            Box::new(move |s| sp(s, 1).mark_at = tick + 16 * hz + 1),
        ),
        ("pending and marked", Box::new(|s| sp(s, 1).marked = true)),
        (
            "marked with no shot",
            Box::new(|s| {
                sp(s, 0).shots = 0;
            }),
        ),
        (
            "s11a2 counter out of range",
            Box::new(|s| game_mut(s).players[0].s11a2.ff_wounds = u32::MAX),
        ),
        (
            "AI counter out of range",
            Box::new(|s| ai1(s).counters.cb_missions = u32::MAX),
        ),
        (
            "cb on a mortar's mission",
            Box::new(move |s| mission_mut(s, mortar).cb = true),
        ),
    ];
    for (name, e) in cases {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        assert!(World::load(&x.save(), 1).is_err(), "{name}: loaded");
    }
    // The one-second slack (spec 3): a spot whose times are due this very
    // second still loads.
    let mut x = World::load(&bytes, 1).unwrap();
    sp(&mut x.state, 0).decay_at = tick + 1 - hz;
    sp(&mut x.state, 1).mark_at = tick + 1 - hz;
    World::load(&x.save(), 1).expect("times inside the last second load");
}
