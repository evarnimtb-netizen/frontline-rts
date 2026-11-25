//! S14a Build C tests (docs/slices/S14-assault-kit.md 5, M1-M5): the
//! melee strike. Every test turns `[melee] enabled` on in the test only,
//! on the mod's data (the rifles' and SMGs' `[melee]` blocks, the rules'
//! spade). Each test names its control: the edit that makes it fail.

use super::s6_tests::{flat_world_with, members, run, spawn, steps};
use super::*;
use crate::flame::{S14Counters, STRIKE_NONE, STRIKE_RECOVER, STRIKE_WINDUP};
use crate::game::kill_class;

const GER: &str = "ww2:ger_grenadiergruppe";
const USA: &str = "ww2:usa_rifle_squad";
/// Where the German man stands (m).
const AT: i32 = 300;

fn s14_of(w: &World, p: usize) -> S14Counters {
    w.game().unwrap().players[p].s14.clone()
}

/// The first man of squad `sq` whose main weapon is `weapon` (def id).
fn man_with(w: &World, sq: SquadId, weapon: &str) -> SimId {
    let g = w.game().unwrap();
    members(w, sq)
        .into_iter()
        .find(|m| g.setup().weapons[w.state.entities.kit[m.index()].weapon as usize].id == weapon)
        .unwrap_or_else(|| panic!("no man with {weapon}"))
}

/// A point `dx`, `dy` cm from (`AT`, `AT`).
fn off(dx: i32, dy: i32) -> FxVec2 {
    FxVec2::new(
        Fx::from_int(AT) + Fx::from_ratio(dx as i64, 100),
        Fx::from_int(AT) + Fx::from_ratio(dy as i64, 100),
    )
}

/// Two lone men: the German man with `ger_w` at (`AT`, `AT`) and the US
/// man with `us_w` `dx`, `dy` cm from him; every other man of both squads
/// is dead. `edit` runs on the setup after `[melee] enabled` is set to
/// `on`. Returns `(world, German man, US man)`.
fn duel(
    on: u32,
    ger_w: &str,
    us_w: &str,
    (dx, dy): (i32, i32),
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> (World, SimId, SimId) {
    let mut w = flat_world_with(1, |s| {
        s.rules.melee.enabled = on;
        s.rules.wounds.enabled = 1;
        edit(s);
    });
    let g = spawn(&mut w, 0, GER, AT, AT);
    let u = spawn(&mut w, 1, USA, AT + 200, AT + 200);
    steps(&mut w, 2);
    let a = man_with(&w, g, ger_w);
    let b = man_with(&w, u, us_w);
    for m in members(&w, g).into_iter().chain(members(&w, u)) {
        if m != a && m != b {
            assert!(w.kill_soldier(m));
        }
    }
    steps(&mut w, 1);
    w.place_soldier(a, off(0, 0));
    w.place_soldier(b, off(dx, dy));
    (w, a, b)
}

/// Keeps man `m` in a long recovery: he neither aims, fires nor starts
/// a strike (a staged passive target).
fn passive(w: &mut World, m: SimId) {
    w.edit_soldier(m, |_, c| {
        c.close.strike_phase = STRIKE_RECOVER;
        c.close.strike = 200;
        c.close.strike_at = off(-500, 0);
    });
}

/// One tick with `a` at `pa` and `b` at `pb` (held on their spots) and
/// `b` passive.
fn tick_held(w: &mut World, (a, pa): (SimId, FxVec2), (b, pb): (SimId, FxVec2), b_passive: bool) {
    w.place_soldier(a, pa);
    w.place_soldier(b, pb);
    if b_passive {
        passive(w, b);
    }
    w.step(&[]);
}

/// Stages `m` one tick from the end of his wind-up, striking toward `at`.
fn wind_up(w: &mut World, m: SimId, at: FxVec2) {
    assert!(w.edit_soldier(m, |_, c| {
        c.close.strike_phase = STRIKE_WINDUP;
        c.close.strike = 1;
        c.close.strike_at = at;
    }));
}

fn hp(w: &World, m: SimId) -> u16 {
    w.state.entities.combat[m.index()].hp
}

fn phase(w: &World, m: SimId) -> u8 {
    w.state.entities.combat[m.index()].close.strike_phase
}

/// M1: a strike toward a point 2 m east touches the capsule of a man 0.9
/// m east (one strike, one hit, damage, class MELEE wound or down);
/// control: a man 0.9 m north of the same segment is not touched (one
/// strike, no hit, no damage).
#[test]
fn strike_hits_only_a_touched_capsule() {
    for (dx, dy, touched) in [(90, 0, true), (0, 90, false)] {
        let (mut w, a, b) = duel(1, "ww2:kar98k", "ww2:m1_garand", (dx, dy), |_| {});
        let full = hp(&w, b);
        wind_up(&mut w, a, off(200, 0));
        tick_held(&mut w, (a, off(0, 0)), (b, off(dx, dy)), true);
        let k = s14_of(&w, 0);
        assert_eq!(k.strikes, 1, "one strike made");
        assert_eq!(phase(&w, a), STRIKE_RECOVER, "he recovers");
        if touched {
            assert_eq!(k.strike_hits, 1);
            assert!(hp(&w, b) < full, "the butt stroke hurts");
        } else {
            assert_eq!(k.strike_hits, 0, "control: aside, no hit");
            assert_eq!(hp(&w, b), full, "control: no damage");
        }
    }
}

/// M2, M3: `Strike` starts a wind-up at the nearest enemy in reach; the
/// strike goes where he stood then. A target that steps back to 1.8 m
/// during the wind-up is missed; control: one that stays is hit.
#[test]
fn target_steps_back_strike_misses() {
    for back in [true, false] {
        let (mut w, a, b) = duel(1, "ww2:kar98k", "ww2:m1_garand", (90, 0), |_| {});
        passive(&mut w, b);
        run(&mut w, 0, 1, CommandKind::Strike { units: vec![a] });
        assert_eq!(phase(&w, a), STRIKE_WINDUP, "Strike accepted");
        let pb = if back { off(180, 0) } else { off(90, 0) };
        for _ in 0..60 {
            if phase(&w, a) != STRIKE_WINDUP {
                break;
            }
            tick_held(&mut w, (a, off(0, 0)), (b, pb), true);
        }
        let k = s14_of(&w, 0);
        assert_eq!(k.strikes, 1);
        assert_eq!(k.strike_hits, u32::from(!back), "back {back}");
    }
}

/// M2: a man with a loaded SMG and an enemy at 1 m shoots (no strike);
/// control: the same man with an empty magazine (reloading) strikes.
#[test]
fn loaded_smg_shoots_not_strikes() {
    for dry in [false, true] {
        let (mut w, a, b) = duel(1, "ww2:mp40", "ww2:m1_garand", (100, 0), |_| {});
        if dry {
            w.edit_soldier(a, |_, c| c.mag = 0);
        }
        let full = hp(&w, b);
        let mut struck = false;
        for _ in 0..60 {
            tick_held(&mut w, (a, off(0, 0)), (b, off(100, 0)), true);
            struck |= phase(&w, a) != STRIKE_NONE;
        }
        if dry {
            assert!(struck, "control: reloading, he strikes");
        } else {
            assert!(!struck, "loaded: no strike");
            assert_eq!(s14_of(&w, 0).strikes, 0);
            assert!(hp(&w, b) < full, "loaded: he shoots");
        }
    }
}

/// M2: a rifleman reloading (empty magazine, rounds in his pouches) with
/// an enemy at 1 m starts a strike within a tick and lands it; control:
/// `[melee] enabled` 0, no strike.
#[test]
fn reloading_rifle_strikes() {
    for on in [1, 0] {
        let (mut w, a, b) = duel(on, "ww2:kar98k", "ww2:m1_garand", (100, 0), |_| {});
        w.edit_soldier(a, |_, c| c.mag = 0);
        let mut started = None;
        for t in 0..60 {
            tick_held(&mut w, (a, off(0, 0)), (b, off(100, 0)), true);
            if started.is_none() && phase(&w, a) != STRIKE_NONE {
                started = Some(t);
            }
        }
        let k = s14_of(&w, 0);
        if on == 1 {
            assert!(started.is_some_and(|t| t <= 1), "started {started:?}");
            assert!(k.strikes >= 1 && k.strike_hits >= 1, "{k:?}");
        } else {
            assert_eq!(started, None, "control: off");
            assert_eq!(k, S14Counters::default());
        }
    }
}

/// M2: a pinned man does not strike; control: the same man not pinned
/// does.
#[test]
fn pinned_man_does_not_strike() {
    for pinned in [true, false] {
        let (mut w, a, b) = duel(1, "ww2:kar98k", "ww2:m1_garand", (100, 0), |_| {});
        let mut struck = false;
        for _ in 0..30 {
            w.edit_soldier(a, |_, c| {
                c.mag = 0;
                if pinned {
                    c.pinned = true;
                    c.suppression = 100_000;
                }
            });
            tick_held(&mut w, (a, off(0, 0)), (b, off(100, 0)), true);
            struck |= phase(&w, a) != STRIKE_NONE;
        }
        assert_eq!(struck, !pinned, "pinned {pinned}");
    }
}

/// M1, M2 (W12): a downed enemy at 0.9 m starts no strike, and a strike
/// staged at him passes over him (no hit, no HP lost); control: the same
/// man standing is struck.
#[test]
fn no_strike_at_downed() {
    for down in [true, false] {
        let (mut w, a, b) = duel(1, "ww2:kar98k", "ww2:m1_garand", (90, 0), |_| {});
        if down {
            assert!(w.down_soldier(b, (0, kill_class::RIFLE)));
        }
        let mut started = false;
        for _ in 0..20 {
            w.edit_soldier(a, |_, c| c.mag = 0);
            w.place_soldier(a, off(0, 0));
            if !down {
                tick_held(&mut w, (a, off(0, 0)), (b, off(90, 0)), true);
            } else {
                w.step(&[]);
            }
            started |= phase(&w, a) != STRIKE_NONE;
        }
        if down {
            assert!(!started, "no strike starts at a downed man");
            let before = hp(&w, b);
            wind_up(&mut w, a, off(90, 0));
            w.place_soldier(a, off(0, 0));
            w.step(&[]);
            let k = s14_of(&w, 0);
            assert_eq!((k.strikes, k.strike_hits), (1, 0));
            assert_eq!(hp(&w, b), before, "the downed man is not struck");
        } else {
            assert!(started, "control: standing, he is struck at");
            assert!(s14_of(&w, 0).strike_hits >= 1);
        }
    }
}

/// M3: two strikers whose wind-ups end in the same tick resolve in
/// ascending id: 0.5 m apart, with a strike that downs at once (`damage`
/// 1000, every weapon on the rules' 0.6 m spade), the lower id (the German, spawned first)
/// strikes first and the US man, down, does not strike; the class is
/// MELEE. Control: `damage` 10, both strike.
#[test]
fn two_strikers_resolve_in_id_order() {
    for dmg in [1000, 10] {
        let (mut w, a, b) = duel(1, "ww2:kar98k", "ww2:m1_garand", (50, 0), |s| {
            s.rules.melee.damage = dmg;
            for wp in &mut s.weapons {
                wp.melee = None;
            }
        });
        assert!(a < b, "the German man has the lower id");
        let full_a = hp(&w, a);
        wind_up(&mut w, a, off(50, 0));
        wind_up(&mut w, b, off(0, 0));
        tick_held(&mut w, (a, off(0, 0)), (b, off(50, 0)), false);
        let (ka, kb) = (s14_of(&w, 0), s14_of(&w, 1));
        assert_eq!((ka.strikes, ka.strike_hits), (1, 1));
        if dmg == 1000 {
            assert_eq!(ka.strike_kills, 1);
            assert_eq!(kb.strikes, 0, "the downed man does not strike");
            assert_eq!(hp(&w, a), full_a);
            let p = &w.game().unwrap().players[0];
            let out = p.kills_by[kill_class::MELEE as usize]
                + p.s12.downed_by_class[kill_class::MELEE as usize];
            assert_eq!(out, 1, "credited to MELEE");
        } else {
            assert_eq!((kb.strikes, kb.strike_hits), (1, 1), "control: both");
            assert!(hp(&w, a) < full_a);
        }
    }
}

/// The two full squads spawned on one spot (the men push apart and fight
/// at arm's length) for 300 ticks: every tick's entity store.
fn close_fight(edit: impl FnOnce(&mut frontline_content::MatchSetup)) -> (Vec<Entities>, World) {
    let mut w = flat_world_with(1, edit);
    spawn(&mut w, 0, GER, AT, AT);
    spawn(&mut w, 1, USA, AT, AT);
    let mut out = Vec::new();
    for _ in 0..300 {
        w.step(&[]);
        out.push(w.state.entities.clone());
    }
    (out, w)
}

/// M1-M5 off: with `[melee] enabled` 0 a close fight runs exactly as with
/// every weapon's `[melee]` block removed and the rules' strike numbers
/// changed (they are inert), tick for tick, and no man ever holds a
/// strike. Control: the switch on, strikes happen and the fight differs.
/// (The 40-match half of this check is T185, `tools/neutral.py`.)
#[test]
fn melee_off_is_base() {
    let strip = |s: &mut frontline_content::MatchSetup| {
        for wp in &mut s.weapons {
            wp.melee = None;
        }
        s.rules.melee.reach_cm = 900;
        s.rules.melee.damage = 999;
        s.rules.melee.windup_ms = 100;
    };
    let (off_a, w) = close_fight(|_| {});
    let (off_b, _) = close_fight(strip);
    assert!(off_a == off_b, "melee off: the data is inert");
    assert!(
        off_a.iter().all(
            |e| e.combat.iter().all(|c| c.close.strike_phase == STRIKE_NONE
                && c.close.strike == 0
                && c.close.strike_at == FxVec2::default())
        ),
        "no strike state with the switch off"
    );
    assert_eq!(s14_of(&w, 0), S14Counters::default());
    let (on, w_on) = close_fight(|s| s.rules.melee.enabled = 1);
    let made = s14_of(&w_on, 0).strikes + s14_of(&w_on, 1).strikes;
    assert!(made > 0, "control: strikes with the switch on");
    assert!(on != off_a, "control: the fight differs");
}

/// M2 `Strike`: refused (`NotAllowed`) with nobody in reach plus pad;
/// accepted at 0.9 m, and the snapshot shows his phase. Control: the same
/// order at 0.9 m with `[melee] enabled` 0 is refused.
#[test]
fn strike_command_needs_a_man_in_reach() {
    for (on, dx, ok) in [(1, 300, false), (1, 90, true), (0, 90, false)] {
        let (mut w, a, b) = duel(on, "ww2:kar98k", "ww2:m1_garand", (dx, 0), |_| {});
        passive(&mut w, b);
        run(&mut w, 0, 1, CommandKind::Strike { units: vec![a] });
        let refused = w
            .fort_refusals()
            .iter()
            .any(|r| r.refusal == crate::fort::FortRefusal::NotAllowed);
        assert_eq!(refused, !ok, "on {on} at {dx} cm");
        assert_eq!(phase(&w, a) == STRIKE_WINDUP, ok);
        let s = w.snapshot();
        let k = s
            .soldier_ids
            .iter()
            .position(|&x| x == a)
            .expect("in the snapshot");
        assert_eq!(s.soldier_strike.len(), s.soldiers.len());
        assert_eq!(
            s.soldier_strike[k],
            if ok { STRIKE_WINDUP } else { STRIKE_NONE }
        );
    }
}

/// M3 through save/load: a world saved mid-wind-up (and a copy loaded
/// with 2 threads) runs on through the strike and the recovery with the
/// same hash every tick. Control: the strike happens in both.
#[test]
fn strike_saves_and_loads_mid_windup() {
    let (mut w, a, b) = duel(1, "ww2:kar98k", "ww2:m1_garand", (90, 0), |_| {});
    passive(&mut w, b);
    run(&mut w, 0, 1, CommandKind::Strike { units: vec![a] });
    steps(&mut w, 3);
    assert_eq!(phase(&w, a), STRIKE_WINDUP, "mid-wind-up");
    let mut back = World::load(&w.save(), 2).expect("loads");
    assert_eq!(
        back.state.entities.combat[a.index()].close,
        w.state.entities.combat[a.index()].close,
        "control: saved"
    );
    for t in 0..90 {
        w.step(&[]);
        back.step(&[]);
        assert_eq!(w.full_state_hash(), back.full_state_hash(), "tick {t}");
    }
    assert_eq!(s14_of(&w, 0).strikes, 1);
    assert_eq!(s14_of(&back, 0), s14_of(&w, 0));
}

/// Spec 8: a close fight with `[melee] enabled` 1 hashes the same at 1,
/// 2, 3 and 8 threads, tick for tick. Control: strikes happen in it.
#[test]
fn melee_scene_hash_is_thread_count_free() {
    let hashes = |threads: usize| {
        let mut w = flat_world_with(threads, |s| s.rules.melee.enabled = 1);
        spawn(&mut w, 0, GER, AT, AT);
        spawn(&mut w, 1, USA, AT, AT);
        let mut v = Vec::new();
        for _ in 0..300 {
            w.step(&[]);
            v.push(w.state_hash());
        }
        (v, s14_of(&w, 0).strikes + s14_of(&w, 1).strikes)
    };
    let (one, made) = hashes(1);
    assert!(made > 0, "control: strikes happen");
    for n in [2, 3, 8] {
        assert_eq!(hashes(n).0, one, "{n} threads");
    }
}
