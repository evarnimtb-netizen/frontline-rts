//! S11a2 Build B tests (docs/slices/S11a2.md 4 S1-S3, R1-R3, 5): the AI's
//! own-men rule under friendly fire and its counter-fire. Each test names
//! its control: the edit that makes it fail.

use super::s6_tests::{flat_world_with, members, spawn, steps};
use super::s11a1_tests::{mission, order};
use super::*;

const MORTAR: &str = "ww2:ger_mortar_team";
const LEIG: &str = "ww2:ger_leig18_squad";
/// An own squad the AI never orders (observer teams leave the lists
/// before `assign`; `[ai.arty] fo` 0): the test walks it.
const WALKER: &str = "ww2:ger_vb_trupp";

fn c(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

/// A flat match whose player 0 (Germany) is an AI with no manpower (it
/// buys nothing), every start squad dead (nobody walks into the scene by
/// itself), `friendly_fire` 1, `ff_safe` as given, `counter_fire` 0;
/// `edit` changes the setup last.
fn ai_world(
    threads: usize,
    ff_safe: u32,
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> World {
    let mut w = flat_world_with(threads, |s| {
        s.players[0].is_ai = true;
        let fm = s.rules.fire_mission.as_mut().unwrap();
        fm.friendly_fire = 1;
        fm.counter_battery = 0;
        let a = &mut s.rules.ai.arty;
        a.ff_safe = ff_safe;
        a.counter_fire = 0;
        a.buy = 0;
        a.fo = 0;
        a.prep = 0;
        // No income: the AI buys nothing that could walk into the scene.
        s.rules.income_per_min = 0;
        edit(s);
    });
    w.state.game.as_mut().unwrap().players[0].manpower = 0;
    let start: Vec<SimId> = w
        .game()
        .unwrap()
        .squads
        .values()
        .flat_map(|q| q.members.clone())
        .collect();
    for m in start {
        w.kill_soldier(m);
    }
    // No step here: with no man alive and no manpower the player is
    // wiped out at the next tick; the caller spawns its scene first.
    w
}

pub(super) fn counters(w: &World) -> crate::ai::AiCounters {
    w.game().unwrap().ai[0].as_ref().unwrap().counters.clone()
}

fn ai(w: &World) -> &crate::ai::AiState {
    w.game().unwrap().ai[0].as_ref().unwrap()
}

/// Strips a squad's rounds (it can neither shoot nor be the reason a
/// target dies early).
fn unarm(w: &mut World, q: SquadId) {
    for m in members(w, q) {
        w.edit_soldier(m, |k, c| {
            k.rounds = [0; frontline_content::setup::MAX_FEEDS];
            c.mag = 0;
        });
    }
}

/// Moves squad `q` of player 0 (sent as a player command).
fn walk(w: &mut World, q: SquadId, to: FxVec2) {
    w.step(&[Command {
        player: PlayerId(0),
        seq: 7,
        kind: CommandKind::Move {
            squad: q,
            target: to,
        },
    }]);
}

/// Every AI command of player 0 seen in `pending` (each seq once).
#[derive(Default)]
struct Sent {
    seen: std::collections::BTreeSet<u32>,
    cmds: Vec<CommandKind>,
}

impl Sent {
    fn look(&mut self, w: &World) {
        for c in &ai(w).pending {
            if self.seen.insert(c.seq) {
                self.cmds.push(c.kind.clone());
            }
        }
    }

    fn stops(&self, q: SquadId) -> usize {
        self.cmds
            .iter()
            .filter(|k| matches!(k, CommandKind::Stop { squad } if *squad == q))
            .count()
    }

    fn missions_near(&self, q: SquadId, at: FxVec2, m: i32) -> usize {
        self.cmds
            .iter()
            .filter(|k| {
                matches!(k, CommandKind::FireMission { squad, target, .. }
                    if *squad == q && (*target - at).length() <= Fx::from_int(m))
            })
            .count()
    }
}

/// Steps until `n` more plans ran, recording the AI's commands.
fn plans(w: &mut World, n: u32, sent: &mut Sent) {
    let p0 = counters(w).plans;
    let t0 = w.tick();
    while counters(w).plans < p0 + n {
        assert!(w.tick() < t0 + 30 * 600, "no plan for 10 minutes");
        w.step(&[]);
        sent.look(w);
    }
}

/// The S2 scene: the AI's mortar team at (100, 500), an unarmed US rifle
/// squad at (350, 500) it sees, the AI's walker at `walker` (unarmed).
fn mortar_scene(
    threads: usize,
    ff_safe: u32,
    walker: (i32, i32),
) -> (World, SquadId, SquadId, SquadId) {
    mortar_scene_with(threads, ff_safe, walker, |_| {})
}

fn mortar_scene_with(
    threads: usize,
    ff_safe: u32,
    walker: (i32, i32),
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> (World, SquadId, SquadId, SquadId) {
    let mut w = ai_world(threads, ff_safe, edit);
    let mortar = spawn(&mut w, 0, MORTAR, 100, 500);
    let enemy = spawn(&mut w, 1, "ww2:usa_rifle_squad", 350, 500);
    let own = spawn(&mut w, 0, WALKER, walker.0, walker.1);
    unarm(&mut w, enemy);
    unarm(&mut w, own);
    steps(&mut w, 2);
    (w, mortar, enemy, own)
}

const ENEMY: (i32, i32) = (350, 500);

/// S2: own men walking onto a target (goal within `danger_close_m`, lead
/// within `lift_m`) hold the mortar's order (`ff_holds`). Control
/// `ff_safe` 0: the 35 m filter lets it fire while the men are still
/// 90 m out.
#[test]
fn ai_mortar_passes_over_a_target_own_men_walk_to() {
    let run = |ff_safe: u32| {
        let (mut w, mortar, _, own) = mortar_scene(1, ff_safe, (ENEMY.0, ENEMY.1 + 90));
        walk(&mut w, own, c(ENEMY.0, ENEMY.1 + 10));
        let mut sent = Sent::default();
        plans(&mut w, 4, &mut sent);
        (
            sent.missions_near(mortar, c(ENEMY.0, ENEMY.1), 30),
            counters(&w).ff_holds,
        )
    };
    let (fm, holds) = run(1);
    assert_eq!(fm, 0, "no order on a target own men walk to");
    assert!(holds > 0, "ff_holds {holds}");
    let (fm, holds) = run(0);
    assert!(fm >= 1, "control ff_safe 0: it fires");
    assert_eq!(holds, 0, "control: no S11a2 hold");
}

/// S2: the mortar spotter's stand-off is `[ai.arty] danger_close_m + 20`
/// (70 m along the line of fire) with `ff_safe` 1. For a target 100 m out
/// the spot found stands at least 70 m short of it. Control `ff_safe` 0:
/// the first spot, 40 m out, 60 m short (the 35 m stand-off).
#[test]
fn ai_spotter_stands_outside_danger_close() {
    let spot = |ff_safe: u32| {
        let (w, mortar, _, _) = mortar_scene(1, ff_safe, (900, 100));
        let g = w.game().unwrap();
        let mut a = g.ai[0].clone().unwrap();
        g.test_spot_for(
            &mut a,
            PlayerId(0),
            &w.state.entities,
            &w.state.terrain,
            w.tick(),
            mortar,
            c(100, 500),
            c(200, 500),
        )
    };
    // Short of the target along the line of fire (east), m.
    let short = |p: FxVec2| 200 - p.x.floor_int();
    let p = spot(1).expect("a spot");
    assert!(short(p) >= 70, "spot {p:?}");
    let p = spot(0).expect("control: a spot");
    assert!((35..70).contains(&short(p)), "control spot {p:?}");
}

/// The S3 scene: the AI's own mortar mission of `rounds` bombs on the
/// unarmed enemy (`[ai.mortar] mission_rounds`); the walker 90 m beyond
/// the point is sent onto it as the first bomb leaves. Returns (lifts,
/// bombs fired, own men hurt or killed by own fire).
fn lift_run(ff_safe: u32, rounds: u32) -> (u32, u32, u32) {
    let (mut w, mortar, _, own) = mortar_scene_with(1, ff_safe, (ENEMY.0, ENEMY.1 + 90), |s| {
        s.rules.ai.mortar.mission_rounds = rounds;
        s.rules.ai.barrage_rounds = rounds as u16;
    });
    let bombs = |w: &World| w.game().unwrap().players[0].fm.bombs.iter().sum::<u32>();
    let mut sent_walk = false;
    let mut b0 = 0;
    for _ in 0..30 * 300 {
        w.step(&[]);
        match mission(&w, mortar) {
            Some(m) if m.fired >= 1 && !sent_walk => {
                b0 = bombs(&w) - 1;
                walk(&mut w, own, c(ENEMY.0, ENEMY.1 + 5));
                sent_walk = true;
            }
            None if sent_walk => break,
            _ => {}
        }
    }
    assert!(sent_walk, "the AI's mission started");
    steps(&mut w, 30 * 40);
    let p = &w.game().unwrap().players[0].s11a2;
    (
        counters(&w).ff_lifts,
        bombs(&w) - b0,
        p.ff_kills[0] + p.ff_wounds,
    )
}

/// S3: with `ff_safe` 1 the mission is stopped when own men walk in
/// (`ff_lifts`, bombs short of the order). Control `ff_safe` 0: the
/// mission runs out and own men are hurt by own fire.
#[test]
fn ai_lifts_a_mortar_mission_when_own_men_walk_in() {
    let (lifts, fired, _) = lift_run(1, 12);
    assert_eq!(lifts, 1, "one lift");
    assert!(fired < 12, "stopped after {fired} bombs");
    let (lifts, fired, hurt) = lift_run(0, 12);
    assert_eq!(lifts, 0, "control: no lift");
    assert!(fired >= 12, "control: ran out ({fired})");
    assert!(hurt > 0, "control: own men hurt by own fire");
}

/// Liveness (S2 + S3): the AI's own mortar mission on the target, then
/// own men walk in: over 20 plans at most one order and one `Stop` for it,
/// and at least one of each. Control: the walker stays away: no `Stop`.
#[test]
fn ai_lift_and_filter_do_not_loop() {
    let run = |walk_in: bool| {
        let (mut w, mortar, _, own) = mortar_scene_with(1, 1, (ENEMY.0, ENEMY.1 + 120), |s| {
            s.rules.ai.mortar.mission_rounds = 12;
            s.rules.ai.barrage_rounds = 12;
        });
        let mut sent = Sent::default();
        // The AI orders its mission (after its walk to a support spot)
        // while the walker stands 120 m off (beyond `lift_m`).
        for _ in 0..30 * 180 {
            w.step(&[]);
            sent.look(&w);
            if mission(&w, mortar).is_some() {
                break;
            }
        }
        assert!(mission(&w, mortar).is_some(), "the AI fired");
        if walk_in {
            walk(&mut w, own, c(ENEMY.0, ENEMY.1 + 10));
        }
        plans(&mut w, 20, &mut sent);
        (
            sent.missions_near(mortar, c(ENEMY.0, ENEMY.1), 30),
            sent.stops(mortar),
        )
    };
    let (fm, stops) = run(true);
    assert_eq!((fm, stops), (1, 1), "one order, one stop");
    let (_, stops) = run(false);
    assert_eq!(stops, 0, "control: no stop");
}

/// S3 with A6 gated: an AI gun on a mission whose target own men reach
/// gets one `Stop` (`ff_lifts` 1, `arty_lifts` 0). Control `ff_safe` 0:
/// A6 sends the one `Stop` (`arty_lifts` 1, `ff_lifts` 0).
#[test]
fn a_gun_mission_gets_one_stop_not_two() {
    let run = |ff_safe: u32| {
        let mut w = ai_world(1, ff_safe, |s| {
            s.rules.ai.arty.buy = 1;
            s.rules.ai.arty.max_guns = 0;
            s.rules.ai.arty.mission_rounds = 12;
        });
        let gun = spawn(&mut w, 0, LEIG, 100, 500);
        let mut sent = Sent::default();
        plans(&mut w, 2, &mut sent);
        assert!(ai(&w).arty.contains_key(&gun), "the gun has a record");
        assert!(w.mark_intel(PlayerId(0), c(600, 500)));
        assert_eq!(order(&mut w, gun, c(600, 500), 12), None);
        let own = spawn(&mut w, 0, WALKER, 600, 520);
        unarm(&mut w, own);
        plans(&mut w, 6, &mut sent);
        let k = counters(&w);
        (sent.stops(gun), k.ff_lifts, k.arty_lifts)
    };
    assert_eq!(run(1), (1, 1, 0), "one Stop, from S3");
    assert_eq!(run(0), (1, 0, 1), "control: one Stop, from A6");
}
