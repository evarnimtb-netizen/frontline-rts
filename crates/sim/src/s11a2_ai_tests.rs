//! S11a2 Build B tests (docs/slices/S11a2.md 4 S1-S3, R1-R3, 5): the AI's
//! own-men rule under friendly fire and its counter-fire. Each test names
//! its control: the edit that makes it fail.

use super::s6_tests::{flat_world_with, members, spawn, steps};
use super::s11a1_tests::{mission, order};
use super::*;
use crate::game::Order;

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
    ai_world_of(threads, 0, ff_safe, edit)
}

/// [`ai_world`] with player `p` the AI.
fn ai_world_of(
    threads: usize,
    p: u8,
    ff_safe: u32,
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> World {
    let mut w = flat_world_with(threads, |s| {
        s.players[p as usize].is_ai = true;
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
    w.state.game.as_mut().unwrap().players[p as usize].manpower = 0;
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
    counters_of(w, 0)
}

fn counters_of(w: &World, p: usize) -> crate::ai::AiCounters {
    w.game().unwrap().ai[p].as_ref().unwrap().counters.clone()
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
        self.look_of(w, 0);
    }

    fn look_of(&mut self, w: &World, p: usize) {
        for c in &w.game().unwrap().ai[p].as_ref().unwrap().pending {
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
    plans_of(w, 0, n, sent);
}

fn plans_of(w: &mut World, p: usize, n: u32, sent: &mut Sent) {
    let p0 = counters_of(w, p).plans;
    let t0 = w.tick();
    while counters_of(w, p).plans < p0 + n {
        assert!(w.tick() < t0 + 30 * 600, "no plan for 10 minutes");
        w.step(&[]);
        sent.look_of(w, p);
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

// ------------------------------------------------- counter-fire (R) --

/// Gives player `p` one marker of player `1 - p`'s gun at `at` with
/// error `err_m` and `shots` rounds of evidence.
fn give_marker(w: &mut World, p: u8, at: FxVec2, err_m: u32, shots: u32) {
    let hz = 30;
    let sec = w.tick().div_ceil(hz) * hz;
    assert!(w.edit_player(PlayerId(p), |pl| {
        pl.cb = vec![crate::counter_battery::CbSpot {
            of: 1 - p,
            pos: at,
            at,
            err_mm: err_m * 1000,
            heard_mm: 800_000,
            evidence: 3,
            shots,
            decay_at: sec + 300 * hz,
            mark_at: 0,
            marked: true,
        }];
    }));
}

fn add_shot(w: &mut World, p: u8) {
    assert!(w.edit_player(PlayerId(p), |pl| pl.cb[0].shots += 1));
}

const MARK: (i32, i32) = (500, 200);

/// The R scene: AI player `p`'s gun (`card`, with a record) at
/// (`gun_x`, 500), a marker of the enemy's gun at `mark` (40 m wide), and
/// with `observed` the S11a1 A4 scene too (an own VB-Trupp at (600, 500),
/// an unarmed US rifle squad it sees at (700, 500)). `counter_fire` 1.
fn cf_scene(
    threads: usize,
    p: u8,
    card: &str,
    gun_x: i32,
    mark: (i32, i32),
    observed: bool,
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> (World, SquadId) {
    let mut w = ai_world_of(threads, p, 1, |s| {
        s.rules.fire_mission.as_mut().unwrap().counter_battery = 1;
        let a = &mut s.rules.ai.arty;
        a.counter_fire = 1;
        a.buy = 1;
        a.max_guns = 0;
        edit(s);
    });
    let gun = spawn(&mut w, p, card, gun_x, 500);
    if observed {
        spawn(&mut w, p, WALKER, 600, 500);
        let e = spawn(&mut w, 1 - p, "ww2:usa_rifle_squad", 700, 500);
        unarm(&mut w, e);
        assert!(w.mark_intel(PlayerId(p), c(700, 500)));
    }
    give_marker(&mut w, p, c(mark.0, mark.1), 40, 3);
    (w, gun)
}

/// R1, R3: an AI gun answers a marker with map fire before its observed
/// target (`cb_missions`, the sim flags the mission `cb`). Control
/// `counter_fire` 0: the A4 target instead.
#[test]
fn ai_gun_answers_a_marker() {
    let run = |cf: u32| {
        let (mut w, gun) = cf_scene(1, 0, LEIG, 100, MARK, true, |s| {
            s.rules.ai.arty.counter_fire = cf
        });
        let mut sent = Sent::default();
        let t0 = w.tick();
        while mission(&w, gun).is_none() && w.tick() < t0 + 30 * 60 {
            w.step(&[]);
            sent.look(&w);
        }
        let m = mission(&w, gun).expect("a mission");
        let Order::FireMission { target, .. } = w.game().unwrap().squads[&gun].order else {
            unreachable!()
        };
        (target, m.cb, counters(&w).cb_missions)
    };
    let (t, cb, n) = run(1);
    assert_eq!(t, c(MARK.0, MARK.1), "on the marker");
    assert!(cb, "the sim flags it counter-fire");
    assert_eq!(n, 1);
    let (t, cb, n) = run(0);
    assert!(
        (t - c(700, 500)).length() < Fx::from_int(30),
        "control: the A4 target"
    );
    assert_eq!((cb, n), (false, 0));
}

/// R2: one mission per new evidence: 40 plans with no new enemy round
/// give one counter-fire mission (`cb_stale_holds` after it). Control: one
/// more round heard: a second mission.
#[test]
fn ai_counter_fire_waits_for_new_evidence() {
    let (mut w, gun) = cf_scene(1, 0, LEIG, 100, MARK, false, |_| {});
    let mut sent = Sent::default();
    plans(&mut w, 40, &mut sent);
    let k = counters(&w);
    assert_eq!(
        sent.missions_near(gun, c(MARK.0, MARK.1), 1),
        1,
        "one mission"
    );
    assert_eq!(k.cb_missions, 1);
    assert!(k.cb_stale_holds > 0, "held for no new evidence");
    add_shot(&mut w, 0);
    plans(&mut w, 40, &mut sent);
    assert_eq!(
        counters(&w).cb_missions,
        2,
        "control: new evidence, a new mission"
    );
}

/// Fix B-3: the evidence answered is tied to no marker, so a switch to a
/// sharper marker with fewer shots than the last one answered is new
/// evidence (`shots != cb_shots`), not stale. Control: no second marker:
/// no second mission.
#[test]
fn ai_counter_fire_answers_a_new_marker_with_fewer_shots() {
    const B: (i32, i32) = (450, 750);
    let run = |second: bool| {
        let (mut w, gun) = cf_scene(1, 0, LEIG, 100, MARK, false, |_| {});
        give_marker(&mut w, 0, c(MARK.0, MARK.1), 40, 5);
        let mut sent = Sent::default();
        plans(&mut w, 20, &mut sent);
        assert_eq!(sent.missions_near(gun, c(MARK.0, MARK.1), 1), 1);
        if second {
            let hz = 30;
            let sec = w.tick().div_ceil(hz) * hz;
            assert!(w.edit_player(PlayerId(0), |pl| {
                pl.cb.push(crate::counter_battery::CbSpot {
                    of: 1,
                    pos: c(B.0, B.1),
                    at: c(B.0, B.1),
                    err_mm: 20_000,
                    heard_mm: 800_000,
                    evidence: 3,
                    shots: 2,
                    decay_at: sec + 300 * hz,
                    mark_at: 0,
                    marked: true,
                });
            }));
        }
        plans(&mut w, 40, &mut sent);
        (
            sent.missions_near(gun, c(B.0, B.1), 1),
            counters(&w).cb_missions,
        )
    };
    assert_eq!(run(true), (1, 2), "the sharper marker is answered");
    assert_eq!(run(false), (0, 1), "control: no new marker, no new mission");
}

/// R2: the flat M3 behind a crest cannot answer a marker beyond it: the
/// AI sends no order (`cb_masked` 1, the evidence spent: once), the sim
/// refuses nothing. Control: no crest: the mission.
#[test]
fn ai_does_not_order_a_masked_counter_fire() {
    let run = |crest: bool| {
        let (mut w, gun) = cf_scene(1, 1, "ww2:usa_105mm_m3_squad", 50, (580, 500), false, |s| {
            if crest {
                s.map
                    .terrain
                    .bands
                    .push(frontline_content::setup::BandSetup {
                        from: frontline_content::setup::FixedVec2::from_m([500, 0]),
                        to: frontline_content::setup::FixedVec2::from_m([500, 1024]),
                        height: frontline_content::Fixed::from_int(25),
                        half_width: frontline_content::Fixed::from_int(40),
                    });
            }
        });
        let mut sent = Sent::default();
        plans_of(&mut w, 1, 20, &mut sent);
        let pl = &w.game().unwrap().players[1];
        (
            counters_of(&w, 1),
            sent.missions_near(gun, c(580, 500), 1),
            pl.s11.masked + pl.s11.not_ready + pl.fm.refused.iter().sum::<u32>(),
        )
    };
    let (k, orders, refused) = run(true);
    assert_eq!((k.cb_masked, orders, refused), (1, 0, 0), "{k:?}");
    let (k, orders, _) = run(false);
    assert_eq!(
        (k.cb_masked, orders, k.cb_missions),
        (0, 1, 1),
        "control: flat"
    );
}

/// R2: a gun at or under `reserve_pct` of its HE keeps it (the A4 hold):
/// no counter-fire. Control `reserve_pct` 0: it fires.
#[test]
fn ai_counter_fire_keeps_the_reserve() {
    let run = |reserve: u32| {
        let (mut w, gun) = cf_scene(1, 0, LEIG, 100, MARK, false, |s| {
            s.rules.ai.arty.reserve_pct = reserve
        });
        let v = members(&w, gun)[0];
        assert!(w.edit_vehicle(v, |vs| {
            for n in vs.ammo.iter_mut() {
                *n = (*n).min(1);
            }
            vs.loaded = None;
        }));
        let mut sent = Sent::default();
        plans(&mut w, 10, &mut sent);
        counters(&w)
    };
    let k = run(30);
    assert_eq!(k.cb_missions, 0);
    assert!(k.arty_reserve_holds > 0);
    assert_eq!(run(0).cb_missions, 1, "control: reserve 0");
}

/// The combined scene of the next two tests: the AI's mortar on the
/// unarmed enemy with the walker 120 m off sent onto the point at
/// `walk_at`, and the AI's leIG with a marker. Switches: `on` all four.
fn all_scene(threads: usize, on: u32) -> (World, SquadId, SquadId, SquadId) {
    let mut w = ai_world(threads, on, |s| {
        let fm = s.rules.fire_mission.as_mut().unwrap();
        fm.friendly_fire = on;
        fm.counter_battery = on;
        let a = &mut s.rules.ai.arty;
        a.counter_fire = on;
        a.buy = 1;
        a.max_guns = 0;
        s.rules.ai.mortar.mission_rounds = 12;
        s.rules.ai.barrage_rounds = 12;
    });
    let mortar = spawn(&mut w, 0, MORTAR, 100, 500);
    let enemy = spawn(&mut w, 1, "ww2:usa_rifle_squad", ENEMY.0, ENEMY.1);
    let own = spawn(&mut w, 0, WALKER, ENEMY.0, ENEMY.1 + 120);
    unarm(&mut w, enemy);
    unarm(&mut w, own);
    let gun = spawn(&mut w, 0, LEIG, 100, 800);
    give_marker(&mut w, 0, c(MARK.0, MARK.1), 40, 3);
    (w, mortar, own, gun)
}

/// The walker is sent onto the point at this tick (after the AI's mortar
/// mission began, about 65 s in).
const WALK_AT: u64 = 30 * 75;

/// Steps `all_scene` to `end`, sending the walker at [`WALK_AT`].
fn all_steps(w: &mut World, own: SquadId, end: u64, sent: &mut Sent) {
    while w.tick() < end {
        if w.tick() == WALK_AT {
            walk(w, own, c(ENEMY.0, ENEMY.1 + 5));
        } else {
            w.step(&[]);
        }
        sent.look(w);
    }
}

/// Neutrality of the switches (spec 4): with all four S11a2 switches 0,
/// 40+ plans of a scene with a marker and own men walking onto a mortar
/// target bring no S3 `Stop`, no counter-fire order and every S11a2 AI
/// counter 0. Control: the switches 1: a lift and a counter-fire mission.
#[test]
fn ai_all_s11a2_switches_off_emit_nothing_new() {
    let run = |on: u32| {
        let (mut w, mortar, own, gun) = all_scene(1, on);
        let mut sent = Sent::default();
        all_steps(&mut w, own, 30 * 150, &mut sent);
        let k = counters(&w);
        assert!(k.plans >= 40, "plans {}", k.plans);
        (
            sent.stops(mortar),
            sent.missions_near(gun, c(MARK.0, MARK.1), 1),
            [
                k.ff_lifts,
                k.ff_holds,
                k.cb_missions,
                k.cb_wide_holds,
                k.cb_stale_holds,
                k.cb_masked,
            ],
        )
    };
    let (stops, cf, k) = run(0);
    assert_eq!((stops, cf, k), (0, 0, [0; 6]), "nothing new");
    let (stops, cf, k) = run(1);
    assert!(
        stops == 1 && cf == 1,
        "control: {stops} stops, {cf} counter-fire, {k:?}"
    );
}

/// Determinism (spec 5): the combined scene on 1 and 4 threads, and saves
/// taken with a counter-fire mission in Laying, with a lift pending and
/// later play on to the same hash. Control: the walker never sent.
#[test]
fn ai_s11a2_deterministic_threads_and_save_load() {
    use crate::fire_mission::phase;
    let end = 30 * 150;
    // The probe: find the two save ticks.
    let (mut w, mortar, own, gun) = all_scene(1, 1);
    let (mut laying, mut lift) = (None, None);
    let mut sent = Sent::default();
    while w.tick() < end {
        let next = w.tick() + 1;
        all_steps(&mut w, own, next, &mut sent);
        if laying.is_none() && mission(&w, gun).is_some_and(|m| m.cb && m.phase == phase::LAYING) {
            laying = Some(w.tick());
        }
        if lift.is_none()
            && ai(&w)
                .pending
                .iter()
                .any(|c| matches!(c.kind, CommandKind::Stop { squad } if squad == mortar))
        {
            lift = Some(w.tick());
        }
    }
    let h1 = w.full_state_hash();
    let at = [
        laying.expect("a counter-fire mission in Laying"),
        lift.expect("a lift pending"),
        end - 30 * 20,
    ];
    let go = |threads: usize| {
        let (mut w, _, own, _) = all_scene(threads, 1);
        let mut saves = Vec::new();
        let mut sent = Sent::default();
        while w.tick() < end {
            if at.contains(&w.tick()) {
                saves.push((w.tick(), w.save()));
            }
            let next = w.tick() + 1;
            all_steps(&mut w, own, next, &mut sent);
        }
        (w.full_state_hash(), saves, own)
    };
    let (h, saves, own) = go(4);
    assert_eq!(h, h1, "1 and 4 threads");
    assert_eq!(saves.len(), 3);
    for (t, bytes) in saves {
        let mut w = World::load(&bytes, 1).unwrap_or_else(|e| panic!("save at {t}: {e:?}"));
        all_steps(&mut w, own, end, &mut Sent::default());
        assert_eq!(w.full_state_hash(), h1, "the save at tick {t}");
    }
    // Control: the walker never sent.
    let (mut w, ..) = all_scene(1, 1);
    while w.tick() < end {
        w.step(&[]);
    }
    assert_ne!(w.full_state_hash(), h1, "control");
}
