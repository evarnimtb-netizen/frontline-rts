//! S11a2 tests (docs/slices/S11a2.md 3, 4; Build A): the SAVE 28 layout
//! and its load checks, friendly fire of mission rounds (F1-F4) and
//! counter-battery by sound (K1-K4). Each test names its control: the edit
//! that makes it fail.

use super::s6_tests::{spawn, steps};
use super::s11a1_tests::{Scene, TARGET, mission, order, run_out, scene_with};
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
        (
            "err_mm past the map",
            Box::new(move |s| sp(s, 0).err_mm = size_mm),
        ),
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

// ------------------------------------------------- friendly fire (F) --

use super::s6_tests::{flat_world_with, members};
use crate::combat::{BallisticEvent, Blow, DamageCtx};
use crate::ecs::Stance;
use crate::projectile::NO_AMMO;

/// A flat match with `[fire_mission] friendly_fire` = `ff`.
fn ff_world(ff: u32) -> World {
    let mut w = flat_world_with(1, |s| {
        s.rules.fire_mission.as_mut().unwrap().friendly_fire = ff;
    });
    steps(&mut w, 2);
    w
}

fn weapon(w: &World, id: &str) -> u16 {
    w.match_setup()
        .unwrap()
        .weapons
        .iter()
        .position(|x| x.id == id)
        .unwrap() as u16
}

/// A lone standing man of player `p` at `(x, y)` (a squad with every other
/// member killed), the spatial hash rebuilt.
fn lone(w: &mut World, p: u8, x: i32, y: i32) -> SimId {
    lone_at(w, p, c(x, y))
}

fn lone_at(w: &mut World, p: u8, at: FxVec2) -> SimId {
    let (x, y) = (at.x.floor_int(), at.y.floor_int());
    let card = if p == 0 {
        "ww2:ger_grenadiergruppe"
    } else {
        "ww2:usa_rifle_squad"
    };
    let q = spawn(w, p, card, x, y);
    steps(w, 2);
    let ms = members(w, q);
    for &m in &ms[1..] {
        assert!(w.kill_soldier(m));
    }
    steps(w, 1);
    let m = ms[0];
    let z = w.state.terrain.height_at(at);
    w.state.entities.transform[m.index()].pos = at.extend(z);
    w.spatial.rebuild(&w.state.entities);
    m
}

/// One shell burst of `weapon` for `player` on the ground at `at`, dealt
/// as the Damage stage would at `tick` (`mission` 0: not a mission round).
fn burst_now(w: &mut World, player: u8, weapon: u16, mission: u32, at: FxVec2, tick: u64) {
    let World {
        state,
        hash,
        spatial,
        ..
    } = w;
    let SimState {
        entities,
        terrain,
        stats,
        game,
        ..
    } = state;
    let pos = at.extend(terrain.height_at(at));
    let mut ctx = DamageCtx {
        ents: entities,
        terrain,
        spatial,
        hash,
        stats,
        seed: 1,
        tick,
        fallen: Vec::new(),
        ff_dead: Vec::new(),
        ff_hit: Vec::new(),
    };
    let ev = [BallisticEvent {
        weapon,
        player,
        ammo: NO_AMMO,
        slot: 0,
        shooter: SimId(u32::MAX),
        blow: Blow::Burst(pos),
        mission,
    }];
    game.as_mut().unwrap().apply_blows(
        &mut ctx,
        &ev,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut Vec::new(),
    );
    w.refresh_hashes();
}

/// Lays man `m` down now (stance and capsule height, as the Status stage
/// would).
fn prone(w: &mut World, m: SimId) {
    pose(w, m, Stance::Prone);
}

/// Puts man `m` in `stance` now (stance and capsule height).
fn pose(w: &mut World, m: SimId, stance: Stance) {
    let b = &w.match_setup().unwrap().rules.combat.body;
    let mm = match stance {
        Stance::Prone => b.prone_mm,
        Stance::Crouch => b.crouch_mm,
        _ => b.stand_mm,
    } as i32;
    let c = &mut w.state.entities.combat[m.index()];
    c.stance = stance;
    c.height = Fx::from_milli(mm);
}

fn hp(w: &World, m: SimId) -> u16 {
    w.state.entities.combat[m.index()].hp
}

fn supp(w: &World, m: SimId) -> u32 {
    w.state.entities.combat[m.index()].suppression
}

/// Hit points player 0's men `ms` lost to `n` mortar bursts at `at` of
/// player 0 (`mission` as given), each keyed by its own tick.
fn own_loss(w: &mut World, ms: &[SimId], mission: u32, at: FxVec2, n: u64) -> u32 {
    let mortar = weapon(w, "ww2:grw34");
    let before: u32 = ms.iter().map(|&m| hp(w, m) as u32).sum();
    // Each burst on the same unbroken ground (no crater of the last).
    let ground = w.state.terrain.clone();
    for k in 0..n {
        burst_now(w, 0, mortar, mission, at, 1000 + k);
        w.state.terrain = ground.clone();
    }
    before - ms.iter().map(|&m| hp(w, m) as u32).sum::<u32>()
}

/// F1: only mission rounds' bursts are friendly-fire capable. With the
/// switch 1, a burst of the same mortar with no mission (as direct HE,
/// a cook-off or an item) on own men hurts none and suppresses none.
/// Control: the same burst from a mission round hurts.
#[test]
fn direct_he_still_spares_own_men() {
    let mut w = ff_world(1);
    let ms: Vec<SimId> = (0..4).map(|k| lone(&mut w, 0, 501 + 2 * k, 500)).collect();
    assert_eq!(own_loss(&mut w, &ms, 0, c(500, 500), 6), 0);
    assert!(
        ms.iter().all(|&m| supp(&w, m) == 0),
        "no suppression either"
    );
    // Control: a mission round's burst.
    let mut w = ff_world(1);
    let ms: Vec<SimId> = (0..4).map(|k| lone(&mut w, 0, 501 + 2 * k, 500)).collect();
    assert!(own_loss(&mut w, &ms, 7, c(500, 500), 6) > 0);
}

/// F2: an own mission burst's overpressure and suppression reach own men.
/// A man 1 m from the burst (inside the 1.5 m blast radius) is hurt and a
/// man 5 m off is suppressed. Control: `friendly_fire` 0: untouched.
#[test]
fn own_mission_burst_blast_hurts_own_men() {
    let run = |ff: u32| {
        let mut w = ff_world(ff);
        // One man inside the blast radius, one behind him for the
        // suppression (a dead man's is cleared).
        let m = lone(&mut w, 0, 501, 500);
        let far = lone(&mut w, 0, 500, 505);
        let full = hp(&w, m);
        let mortar = weapon(&w, "ww2:grw34");
        burst_now(&mut w, 0, mortar, 3, c(500, 500), 1000);
        (full - hp(&w, m), supp(&w, far))
    };
    let (lost, s) = run(1);
    assert!(lost > 0 && s > 0, "lost {lost}, suppression {s}");
    assert_eq!(run(0), (0, 0), "control: switch 0");
}

/// F2: overhead cover for an own victim counts his own pieces
/// (`piece_between` asked with no excluded player): an own man 1.4 m from
/// an own mission burst (inside the 1.5 m blast radius) with an own
/// sandbag wall between takes no overpressure, as behind the enemy's
/// wall. Controls: no wall (the blast hurts him), and the old exclusion
/// (the burst owner's pieces left out: the own wall does not cover).
#[test]
fn own_roof_shields_own_men_from_own_burst() {
    let run = |owner: Option<u8>| {
        let mut w = ff_world(1);
        let f = w
            .match_setup()
            .unwrap()
            .forts
            .iter()
            .position(|f| f.id == "ww2:sandbag_wall")
            .unwrap() as u16;
        if let Some(o) = owner {
            let x = Fx::from_milli(500_800);
            let line = [
                FxVec2::new(x, Fx::from_int(490)),
                FxVec2::new(x, Fx::from_int(510)),
            ];
            assert!(w.build_now(f, &line, PlayerId(o)));
        }
        let m = lone_at(
            &mut w,
            0,
            FxVec2::new(Fx::from_milli(501_400), Fx::from_int(500)),
        );
        prone(&mut w, m);
        let full = hp(&w, m);
        let mortar = weapon(&w, "ww2:grw34");
        burst_now(&mut w, 0, mortar, 3, c(500, 500), 1000);
        full - hp(&w, m)
    };
    let (own, enemy, open) = (run(Some(0)), run(Some(1)), run(None));
    assert_eq!(own, enemy, "own cover counts as the enemy's");
    assert!(own < open, "control: no wall ({own} vs {open})");
}

/// F3: fragments of an own mission burst hit own men 8 m off (outside the
/// blast radius). Control: switch 0: none.
#[test]
fn own_mission_fragments_hit_own_men() {
    let run = |ff: u32| {
        let mut w = ff_world(ff);
        let ms: Vec<SimId> = (0..5).map(|k| lone(&mut w, 0, 508, 496 + 2 * k)).collect();
        own_loss(&mut w, &ms, 3, c(500, 500), 8)
    };
    assert!(run(1) > 0);
    assert_eq!(run(0), 0, "control: switch 0");
}

/// F3: own men in a trench (on its floor) beside an own mission burst on
/// the surface are shielded as enemy men would be. Control: the same men
/// on open ground are hit.
#[test]
fn own_trench_shields_own_men_from_own_fragments() {
    let run = |trench: bool| {
        let mut w = ff_world(1);
        if trench {
            assert!(w.dig_trench_now(&[c(508, 480), c(508, 520)], -1));
        }
        let ms: Vec<SimId> = (0..5).map(|k| lone(&mut w, 0, 508, 496 + 2 * k)).collect();
        if trench {
            // On the floor, crouched (the Status stage puts them there).
            steps(&mut w, 2);
            for &m in &ms {
                assert!(
                    w.state.entities.combat[m.index()].in_trench,
                    "in the trench"
                );
            }
            w.spatial.rebuild(&w.state.entities);
        }
        own_loss(&mut w, &ms, 3, c(500, 500), 8)
    };
    assert_eq!(run(true), 0, "the trench shields own men");
    assert!(run(false) > 0, "control: in the open they are hit");
}

/// F3: an own sandbag wall and an own tank between an own mission burst
/// and own men stop its fragments as the same cover of the enemy would,
/// and take nothing. Controls: the cover removed (the men lose more), and
/// the F3 rule undone (own boxes out of the ray test: the own wall's men
/// lose as much as with no wall, more than behind the enemy's).
#[test]
fn own_bunker_and_own_hull_stop_own_fragments() {
    // Behind a wall of `owner` (None: no wall).
    let wall_run = |owner: Option<u8>| {
        let mut w = ff_world(1);
        let f = w
            .match_setup()
            .unwrap()
            .forts
            .iter()
            .position(|f| f.id == "ww2:sandbag_wall")
            .unwrap() as u16;
        if let Some(o) = owner {
            let x = Fx::from_milli(501_900);
            let line = [
                FxVec2::new(x, Fx::from_int(490)),
                FxVec2::new(x, Fx::from_int(510)),
            ];
            assert!(w.build_now(f, &line, PlayerId(o)));
        }
        // Crouched 0.4 m behind the 0.9 m wall, 2.6 m from the burst.
        let x = Fx::from_milli(502_600);
        let ms: Vec<SimId> = (0..5)
            .map(|k| lone_at(&mut w, 0, FxVec2::new(x, Fx::from_int(496 + 2 * k))))
            .collect();
        for &m in &ms {
            pose(&mut w, m, Stance::Crouch);
        }
        let hp0: Vec<u32> = w
            .game()
            .unwrap()
            .fort()
            .pieces
            .values()
            .map(|p| p.hp)
            .collect();
        let lost = own_loss(&mut w, &ms, 3, c(500, 500), 8);
        let hp1: Vec<u32> = w
            .game()
            .unwrap()
            .fort()
            .pieces
            .values()
            .map(|p| p.hp)
            .collect();
        if owner == Some(0) {
            assert_eq!(hp0, hp1, "the own wall takes nothing");
        }
        lost
    };
    let (own, enemy, open) = (wall_run(Some(0)), wall_run(Some(1)), wall_run(None));
    assert_eq!(own, enemy, "the own wall shields as the enemy's does");
    assert!(own < open, "control: no wall ({own} vs {open})");
    // Behind a tank of `owner`.
    let tank_run = |owner: Option<u8>| {
        let mut w = ff_world(1);
        let tank = owner.map(|o| {
            let card = if o == 0 {
                "ww2:ger_pz4_h_squad"
            } else {
                "ww2:usa_m4a3_75_squad"
            };
            let q = spawn(&mut w, o, card, 505, 500);
            steps(&mut w, 2);
            members(&w, q)[0]
        });
        // Standing in the hull's shadow, 9 m from the burst.
        let ms: Vec<SimId> = (0..3).map(|k| lone(&mut w, 0, 509, 499 + k)).collect();
        let vs0 = tank.map(|t| w.game().unwrap().vehicles[&t].clone());
        let lost = own_loss(&mut w, &ms, 3, c(500, 500), 8);
        if owner == Some(0) {
            let t = tank.unwrap();
            assert_eq!(
                Some(w.game().unwrap().vehicles[&t].clone()),
                vs0,
                "the own tank takes nothing"
            );
        }
        lost
    };
    let (own, open) = (tank_run(Some(0)), tank_run(None));
    assert!(own < open, "control: no tank ({own} vs {open})");
    assert_eq!(own, 0, "the own hull stops own fragments");
}

/// The F scene of the contract: player 0's mortar team at (200, 500) fires
/// a Direct mission of `rounds` bombs on (500, 500), where a rifle squad of
/// player `victim` stands halted. Returns `(world, mortar, squad)` with
/// the mission ordered.
fn mission_scene(threads: usize, ff: u32, victim: u8, rounds: u16) -> (World, SquadId, SquadId) {
    let mut w = flat_world_with(threads, |s| {
        s.rules.fire_mission.as_mut().unwrap().friendly_fire = ff;
    });
    steps(&mut w, 2);
    let mortar = spawn(&mut w, 0, "ww2:ger_mortar_team", 200, 500);
    let card = if victim == 0 {
        "ww2:ger_grenadiergruppe"
    } else {
        "ww2:usa_rifle_squad"
    };
    let sq = spawn(&mut w, victim, card, 500, 500);
    steps(&mut w, 30);
    assert_eq!(order(&mut w, mortar, c(500, 500), rounds), None);
    (w, mortar, sq)
}

/// Runs the mission out and 40 s more (the last bombs land).
fn finish(w: &mut World, mortar: SquadId) {
    run_out(w, mortar, 400);
    steps(w, 40 * 30);
}

fn pl(w: &World, p: usize) -> &crate::game::PlayerState {
    &w.game().unwrap().players()[p]
}

/// F2 (spec 4): 8 bombs of an own Direct mission on an own squad in the
/// open: men lost, all of them own-fire deaths. Control: `friendly_fire`
/// 0: none lost, none hurt.
#[test]
fn own_mission_burst_hurts_own_men() {
    let (mut w, mortar, sq) = mission_scene(1, 1, 0, 8);
    let men0 = members(&w, sq).len();
    finish(&mut w, mortar);
    let p = pl(&w, 0);
    assert!(p.losses > 0, "own men lost");
    assert_eq!(p.s11a2.ff_kills, [p.losses, 0], "all by own mortar fire");
    assert!(p.s11a2.ff_bursts > 0);
    assert!(members(&w, sq).len() < men0);
    // Control.
    let (mut w, mortar, sq) = mission_scene(1, 0, 0, 8);
    let ms = members(&w, sq);
    finish(&mut w, mortar);
    assert_eq!(pl(&w, 0).losses, 0);
    assert!(ms.iter().all(|&m| hp(&w, m) == 100), "control: nobody hurt");
    let c = pl(&w, 0).s11a2;
    assert_eq!((c.ff_kills, c.ff_wounds, c.ff_bursts), ([0, 0], 0, 0));
    assert!(c.mortar_near_own_bursts > 0, "the report counts either way");
}

/// F4: own-fire deaths are not kills (`kills`, `kills_by`, the per-type
/// rows stay 0) and are losses. Control: the squad owned by the enemy:
/// the same mission's deaths are kills.
#[test]
fn friendly_fire_kills_are_not_kills() {
    let (mut w, mortar, _) = mission_scene(1, 1, 0, 8);
    finish(&mut w, mortar);
    let p = pl(&w, 0);
    assert!(p.losses > 0);
    assert_eq!(p.kills, 0);
    assert_eq!(p.kills_by.iter().sum::<u32>(), 0);
    assert_eq!(p.s7.kills_by_vehicle.iter().sum::<u32>(), 0);
    assert_eq!(p.s11.gun_kills, 0);
    assert_eq!(p.s11a2.ff_kills[0], p.losses);
    // Control: the enemy's squad.
    let (mut w, mortar, _) = mission_scene(1, 1, 1, 8);
    finish(&mut w, mortar);
    let (p0, p1) = (pl(&w, 0), pl(&w, 1));
    assert!(p0.kills > 0 && p0.kills == p1.losses, "control: kills");
    assert_eq!(p0.s11a2.ff_kills, [0, 0]);
}

/// F4: a man wounded by his own side's mission burst and then killed by
/// the enemy is the enemy's kill (the own-fire mark is made on the killing
/// hit only). Control: the killing burst is the own one: no kill.
#[test]
fn enemy_kill_of_a_man_wounded_by_own_fire_is_a_kill() {
    let run = |finisher: u8| {
        let mut w = ff_world(1);
        let m = lone(&mut w, 0, 508, 500);
        let lost0 = pl(&w, 0).losses;
        let mortar = weapon(&w, "ww2:grw34");
        // Own fragments until he is wounded, not dead.
        let ground = w.state.terrain.clone();
        let mut k = 0;
        while hp(&w, m) == 100 {
            burst_now(&mut w, 0, mortar, 3, c(500, 500), 2000 + k);
            w.state.terrain = ground.clone();
            k += 1;
            assert!(k < 200, "never hit");
        }
        assert!(hp(&w, m) > 0, "wounded first");
        assert_eq!(pl(&w, 0).s11a2.ff_wounds, 1);
        // Then bursts on him until he dies.
        let mut k = 0;
        while hp(&w, m) > 0 {
            let mission = if finisher == 0 { 3 } else { 0 };
            burst_now(&mut w, finisher, mortar, mission, c(508, 500), 3000 + k);
            w.state.terrain = ground.clone();
            k += 1;
            assert!(k < 50, "never killed");
        }
        (
            pl(&w, 1).kills,
            pl(&w, 0).s11a2.ff_kills,
            pl(&w, 0).losses - lost0,
        )
    };
    assert_eq!(run(1), (1, [0, 0], 1), "the enemy's kill");
    assert_eq!(run(0), (0, [1, 0], 1), "control: own fire kills him");
}

/// Spec 5 "Own burst": the firing squad dies with bombs in the air; the
/// bursts still hurt own men and count (mortar, from the burst's class).
/// Control: before the deaths nothing marks them (the counters are read
/// as a difference).
#[test]
fn bombs_in_the_air_still_hurt_after_the_tube_dies() {
    let (mut w, mortar, sq) = mission_scene(1, 1, 0, 12);
    // Into fire for effect (several bombs in the air), then kill the team.
    let fired = |w: &World| mission(w, mortar).map_or(0, |m| m.fired);
    let end = w.tick() + 300 * 30;
    while fired(&w) < 6 {
        assert!(w.tick() < end, "the mortar never fired");
        w.step(&[]);
    }
    let ff0 = pl(&w, 0).s11a2;
    let ms = members(&w, sq);
    let hp0: u32 = ms.iter().map(|&m| hp(&w, m) as u32).sum();
    for m in members(&w, mortar) {
        assert!(w.kill_soldier(m));
    }
    steps(&mut w, 40 * 30);
    let p = pl(&w, 0);
    let hp1: u32 = ms.iter().map(|&m| hp(&w, m) as u32).sum();
    assert!(hp1 < hp0, "bombs in the air still hurt own men");
    assert!(p.s11a2.ff_bursts > ff0.ff_bursts, "and count");
    assert_eq!(p.kills, 0);
    assert_eq!(p.s11a2.ff_kills[1], 0, "mortar, not gun");
}

/// F4: friendly fire is deterministic across thread counts and a
/// save/load with bombs in the air. Control: the run with `friendly_fire`
/// 0 ends with another hash (the rule did something).
#[test]
fn friendly_fire_deterministic_threads_and_save_load() {
    let run = |threads: usize, ff: u32, reload: bool| {
        let (mut w, mortar, _) = mission_scene(threads, ff, 0, 8);
        let end = w.tick() + 300 * 30;
        while mission(&w, mortar).map_or(0, |m| m.fired) < 2 {
            assert!(w.tick() < end);
            w.step(&[]);
        }
        if reload {
            w = World::load(&w.save(), threads).unwrap();
        }
        finish(&mut w, mortar);
        (w.full_state_hash(), pl(&w, 0).s11a2)
    };
    let a = run(1, 1, false);
    assert!(a.1.ff_bursts > 0);
    assert_eq!(run(4, 1, false), a, "4 threads");
    assert_eq!(run(1, 1, true), a, "save/load mid-mission");
    assert_ne!(run(1, 0, false).0, a.0, "control: switch 0");
}

// ------------------------------------------- counter-battery (K) --

use crate::counter_battery::whole_second;

const GUN: (i32, i32) = (100, 500);
const MAP_TARGET: (i32, i32) = (500, 250);

/// The K scene: player 0's real leIG 18 at (100, 500), a lone listener of
/// player 1 800 m east, `counter_battery` 1; `edit` changes the setup.
fn cb_scene(
    threads: usize,
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> (World, SquadId, SimId) {
    let mut w = flat_world_with(threads, |s| {
        s.rules.fire_mission.as_mut().unwrap().counter_battery = 1;
        edit(s);
    });
    steps(&mut w, 2);
    let gun = spawn(&mut w, 0, "ww2:ger_leig18_squad", GUN.0, GUN.1);
    let ear = lone(&mut w, 1, GUN.0 + 800, GUN.1);
    steps(&mut w, 30);
    (w, gun, ear)
}

/// Orders `n` rounds of map fire on [`MAP_TARGET`] (marked known first).
fn map_fire(w: &mut World, gun: SquadId, n: u16) {
    assert!(w.mark_intel(PlayerId(0), c(MAP_TARGET.0, MAP_TARGET.1)));
    assert_eq!(order(w, gun, c(MAP_TARGET.0, MAP_TARGET.1), n), None);
}

fn fired(w: &World, gun: SquadId) -> u16 {
    mission(w, gun).map_or(0, |m| m.fired)
}

fn gun_pos(w: &World, gun: SquadId) -> FxVec2 {
    let v = w.game().unwrap().squads[&gun].members[0];
    w.state.entities.transform[v.index()].pos.xy()
}

fn spots(w: &World, p: usize) -> Vec<CbSpot> {
    pl(w, p).cb.clone()
}

/// [`MatchState::cb_note`] for player 0's gun at `at` now.
fn note(w: &mut World, at: FxVec2, tick: u64) {
    let SimState { entities, game, .. } = &mut w.state;
    game.as_mut()
        .unwrap()
        .cb_note(entities, PlayerId(0), at, tick);
}

/// [`MatchState::cb_tick`] on every whole second of `from..=to`.
fn cb_ticks(w: &mut World, from: u64, to: u64) {
    let SimState { entities, game, .. } = &mut w.state;
    let g = game.as_mut().unwrap();
    let mut t = whole_second(from, 30);
    while t <= to {
        g.cb_tick(entities, t);
        t += 30;
    }
}

/// K1: each mission round of an infantry gun counts with the enemy: one
/// spot of player 0 at the gun, with as much evidence as rounds. Controls:
/// `counter_battery` 0 (no spot), a mortar's mission (no spot).
#[test]
fn gun_rounds_leave_evidence_with_the_enemy() {
    let (mut w, gun, _) = cb_scene(1, |_| {});
    map_fire(&mut w, gun, 2);
    run_out(&mut w, gun, 300);
    let sp = spots(&w, 1);
    assert_eq!(sp.len(), 1, "{sp:?}");
    assert_eq!((sp[0].of, sp[0].evidence, sp[0].shots), (0, 2, 2));
    assert!((sp[0].pos - gun_pos(&w, gun)).length() < Fx::ONE);
    assert_eq!(pl(&w, 0).s11a2.cb_heard, 2);
    assert!(spots(&w, 0).is_empty(), "nothing heard of player 1");
    // Control: switch 0.
    let (mut w, gun, _) = cb_scene(1, |s| {
        s.rules.fire_mission.as_mut().unwrap().counter_battery = 0;
    });
    map_fire(&mut w, gun, 2);
    run_out(&mut w, gun, 300);
    assert!(spots(&w, 1).is_empty(), "control: switch 0");
    assert_eq!(pl(&w, 0).s11a2.cb_heard, 0);
    // Control: a mortar mission.
    let (mut w, _, _) = cb_scene(1, |_| {});
    let mortar = spawn(&mut w, 0, "ww2:ger_mortar_team", 150, 500);
    steps(&mut w, 30);
    assert!(w.mark_intel(PlayerId(0), c(450, 500)));
    assert_eq!(order(&mut w, mortar, c(450, 500), 3), None);
    run_out(&mut w, mortar, 300);
    assert!(spots(&w, 1).is_empty(), "control: mortars leave none");
}

/// K1 (D 10 test 5): a gun that hops 60 m keeps its spot; 80 m (beyond
/// `cb_reset_m` 75) starts a new one, unless the spot's marker error is
/// wider than the hop.
#[test]
fn a_short_hop_keeps_the_spot() {
    let (mut w, _, _) = cb_scene(1, |_| {});
    let t = w.tick();
    let p = c(GUN.0, GUN.1);
    note(&mut w, p, t);
    note(&mut w, p + c(0, 60), t);
    assert_eq!(spots(&w, 1).len(), 1, "60 m: the same spot");
    assert_eq!(spots(&w, 1)[0].evidence, 2);
    note(&mut w, p + c(0, 80), t);
    assert_eq!(spots(&w, 1).len(), 2, "80 m unmarked: a new spot");
    // A marked spot with a 90 m error takes the 80 m hop.
    let (mut w, _, _) = cb_scene(1, |_| {});
    note(&mut w, p, t);
    assert!(w.edit_player(PlayerId(1), |pl| {
        let s = &mut pl.cb[0];
        s.marked = true;
        s.err_mm = 90_000;
        s.heard_mm = 800_000;
    }));
    note(&mut w, p + c(0, 80), t);
    assert_eq!(spots(&w, 1).len(), 1, "inside the marker's error");
}

/// Spec 5 "Spot / marker": two guns of one player within `cb_reset_m`
/// share one spot with twice the evidence. Control: 200 m apart: two.
#[test]
fn two_guns_within_reset_share_a_spot() {
    let (mut w, _, _) = cb_scene(1, |_| {});
    let t = w.tick();
    for _ in 0..2 {
        note(&mut w, c(100, 500), t);
        note(&mut w, c(100, 540), t);
    }
    let sp = spots(&w, 1);
    assert_eq!((sp.len(), sp[0].evidence), (1, 4));
    let (mut w, _, _) = cb_scene(1, |_| {});
    note(&mut w, c(100, 500), t);
    note(&mut w, c(100, 700), t);
    assert_eq!(spots(&w, 1).len(), 2, "control: 200 m apart");
}

/// K2: the third round marks the gun `cb_delay_s` later, within 15 % of
/// the listener's 800 m. Control: `cb_rounds_to_mark` 9: no marker from
/// 8 rounds.
#[test]
fn third_round_marks_the_gun_within_15_pct() {
    let (mut w, gun, _) = cb_scene(1, |_| {});
    map_fire(&mut w, gun, 8);
    let end = w.tick() + 600 * 30;
    let mut t3 = None;
    while mission(&w, gun).is_some() || t3.is_none_or(|t| w.tick() < t + 20 * 30) {
        assert!(w.tick() < end);
        w.step(&[]);
        let sp = spots(&w, 1);
        if t3.is_none() && sp.first().is_some_and(|s| s.evidence >= 3) {
            t3 = Some(w.tick());
        }
        if let (Some(t), Some(s)) = (t3, sp.first()) {
            if w.tick() < t + 15 * 30 - 1 {
                assert!(!s.marked, "no marker before round 3 + 15 s");
            }
        } else {
            assert!(sp.iter().all(|s| !s.marked));
        }
    }
    let s = spots(&w, 1)[0];
    assert!(s.marked, "marked");
    let miss = (s.at - gun_pos(&w, gun)).length();
    assert!(miss <= Fx::from_int(120), "miss {miss:?}");
    assert!(
        s.heard_mm > 790_000 && s.heard_mm < 810_000,
        "{}",
        s.heard_mm
    );
    assert_eq!(pl(&w, 1).s11a2.cb_marks, 1);
    // Control.
    let (mut w, gun, _) = cb_scene(1, |s| {
        s.rules.fire_mission.as_mut().unwrap().cb_rounds_to_mark = 9;
    });
    map_fire(&mut w, gun, 8);
    run_out(&mut w, gun, 600);
    steps(&mut w, 30 * 30);
    assert!(spots(&w, 1).iter().all(|s| !s.marked), "control: 9 rounds");
}

/// The first misses of 40 markers (each from 3 rounds at the same gun, the
/// draws keyed by their own ticks) as tenths of a % of `heard_mm`.
fn first_misses(w: &mut World, fixed_len: bool) -> Vec<u64> {
    let p = c(GUN.0, GUN.1);
    let base = whole_second(w.tick(), 30);
    let mut out = Vec::new();
    for k in 0..40u64 {
        assert!(w.edit_player(PlayerId(1), |pl| pl.cb.clear()));
        let t = base + k * 3600 * 30;
        for _ in 0..3 {
            note(w, p, t);
        }
        let due = spots(w, 1)[0].mark_at;
        cb_ticks(w, due, due);
        let mut s = spots(w, 1)[0];
        assert!(s.marked);
        if fixed_len {
            // The control: the offset pushed out to the full error.
            let off = s.at - s.pos;
            let e = Fx::from_raw((((s.err_mm as u64) << 16) / 1000) as i32);
            s.at = s.pos + off.normalize_or_zero() * e;
        }
        let miss = (((s.at - s.pos).length().raw() as u64) * 1000) >> 16;
        out.push(miss * 1000 / s.heard_mm as u64);
    }
    out
}

/// K2: marker misses are spread inside the error (uniform length): mean
/// 5-10 % of the listener distance, at least one under 4 %, none over
/// 15 %. Control: a fixed-length offset (the mission aim's rule) fails the
/// mean.
#[test]
fn marker_offsets_are_spread_inside_the_error() {
    let (mut w, _, _) = cb_scene(1, |_| {});
    let m = first_misses(&mut w, false);
    let mean = m.iter().sum::<u64>() / m.len() as u64;
    assert!((50..=100).contains(&mean), "mean {mean} / 1000: {m:?}");
    assert!(m.iter().any(|&x| x < 40), "{m:?}");
    assert!(m.iter().all(|&x| x <= 150), "{m:?}");
    let (mut w, _, _) = cb_scene(1, |_| {});
    let m = first_misses(&mut w, true);
    let mean = m.iter().sum::<u64>() / m.len() as u64;
    assert!(mean > 100, "control: fixed length, mean {mean}");
}

/// K2, K3: a gun firing one round a minute is marked by its fourth round
/// (evidence decays one per 300 s). Control: `cb_decay_s` 30: never.
#[test]
fn slow_gun_is_marked_by_its_fourth_round() {
    let run = |decay: u32| {
        let (mut w, _, _) = cb_scene(1, |s| {
            s.rules.fire_mission.as_mut().unwrap().cb_decay_s = decay;
        });
        let t0 = whole_second(w.tick(), 30);
        let p = c(GUN.0, GUN.1);
        for k in 0..4u64 {
            let t = t0 + k * 60 * 30;
            cb_ticks(&mut w, t.saturating_sub(59 * 30), t);
            note(&mut w, p, t);
        }
        let t4 = t0 + 3 * 60 * 30;
        cb_ticks(&mut w, t4 + 1, t4 + 16 * 30);
        spots(&w, 1).iter().any(|s| s.marked)
    };
    assert!(run(300), "marked by the fourth round");
    assert!(!run(30), "control: decay 30 s");
}

/// K3: five more rounds at 800 m sharpen the first 120 m error to the
/// 40 m floor (5 %), the marker moving toward the gun along its offset.
/// Control: `cb_error_step_pct` 100: unchanged.
#[test]
fn marker_sharpens_to_the_floor() {
    let run = |step: u32| {
        let (mut w, _, _) = cb_scene(1, |s| {
            s.rules.fire_mission.as_mut().unwrap().cb_error_step_pct = step;
        });
        let t = whole_second(w.tick(), 30);
        let p = c(GUN.0, GUN.1);
        for _ in 0..3 {
            note(&mut w, p, t);
        }
        let due = spots(&w, 1)[0].mark_at;
        cb_ticks(&mut w, due, due);
        let s0 = spots(&w, 1)[0];
        for _ in 0..5 {
            note(&mut w, p, due + 30);
        }
        (s0, spots(&w, 1)[0])
    };
    let (s0, s1) = run(80);
    assert!((119_000..=121_000).contains(&s0.err_mm), "{}", s0.err_mm);
    assert!((39_000..=41_000).contains(&s1.err_mm), "{}", s1.err_mm);
    let p = c(GUN.0, GUN.1);
    assert!((s1.at - p).length() <= (s0.at - p).length());
    let (s0, s1) = run(100);
    assert_eq!((s0.err_mm, s0.at), (s1.err_mm, s1.at), "control: step 100");
}

/// K3: three rounds, then 900 s: the spot and its marker are gone (one
/// piece of evidence per 300 s). Control: 600 s: still there.
#[test]
fn evidence_decays_and_the_marker_goes() {
    let run = |wait_s: u64| {
        let (mut w, _, _) = cb_scene(1, |_| {});
        let t = whole_second(w.tick(), 30);
        for _ in 0..3 {
            note(&mut w, c(GUN.0, GUN.1), t);
        }
        cb_ticks(&mut w, t, t + wait_s * 30);
        (spots(&w, 1).len(), pl(&w, 1).s11a2.cb_dropped)
    };
    assert_eq!(run(900), (0, 1), "gone");
    assert_eq!(run(600).0, 1, "control: 600 s");
}

/// Spec 5 "Spot / marker": the gun dies; its marker stays until decay.
/// Control: the decay (as above) removes it.
#[test]
fn marker_outlives_the_gun_until_decay() {
    let (mut w, gun, _) = cb_scene(1, |_| {});
    map_fire(&mut w, gun, 4);
    run_out(&mut w, gun, 300);
    steps(&mut w, 20 * 30);
    assert!(spots(&w, 1)[0].marked);
    for m in members(&w, gun) {
        let _ = w.kill_soldier(m);
    }
    let crew: Vec<SimId> = w
        .game()
        .unwrap()
        .squads
        .values()
        .filter(|q| q.owner == PlayerId(0))
        .flat_map(|q| q.members.clone())
        .collect();
    for m in crew {
        let _ = w.kill_soldier(m);
    }
    steps(&mut w, 60 * 30);
    assert!(spots(&w, 1)[0].marked, "the marker outlives the gun");
    steps(&mut w, 1200 * 30);
    assert!(spots(&w, 1).is_empty(), "then decays");
}

/// Spec 3: a save between a round (or the marking) and the next whole
/// second loads (K1, K2 times rounded up). Control: the same save with
/// `decay_at` a second further back is refused.
#[test]
fn save_between_the_round_and_the_next_second_loads() {
    let (mut w, gun, _) = cb_scene(1, |_| {});
    map_fire(&mut w, gun, 4);
    let end = w.tick() + 300 * 30;
    let mut saved = 0;
    while mission(&w, gun).is_some() {
        assert!(w.tick() < end);
        let n0 = fired(&w, gun);
        w.step(&[]);
        if fired(&w, gun) > n0 && !w.tick().is_multiple_of(30) {
            let b = w.save();
            let back = World::load(&b, 1).expect("a save just after a round loads");
            assert_eq!(back.full_state_hash(), w.full_state_hash());
            saved += 1;
        }
    }
    assert!(saved > 0);
    // Pending marker: a save mid-second loads too.
    while !w.tick().is_multiple_of(30) || spots(&w, 1)[0].mark_at == 0 {
        w.step(&[]);
        if spots(&w, 1)[0].marked {
            break;
        }
    }
    w.step(&[]);
    World::load(&w.save(), 1).expect("a pending marker loads");
    let mut x = World::load(&w.save(), 1).unwrap();
    let t = x.tick();
    x.state.game.as_mut().unwrap().players[1].cb[0].decay_at = t - 30;
    assert!(World::load(&x.save(), 1).is_err(), "control");
}

/// K1-K3: deterministic across threads and a save/load with a pending
/// and then a sharpening marker. Control: `counter_battery` 0 ends with
/// another hash.
#[test]
fn counter_battery_deterministic_threads_and_save_load() {
    let run = |threads: usize, on: u32, reload: &[u16]| {
        let (mut w, gun, _) = cb_scene(threads, |s| {
            s.rules.fire_mission.as_mut().unwrap().counter_battery = on;
        });
        map_fire(&mut w, gun, 8);
        let mut done = Vec::new();
        let end = w.tick() + 600 * 30;
        while mission(&w, gun).is_some() {
            assert!(w.tick() < end);
            w.step(&[]);
            let f = fired(&w, gun);
            if reload.contains(&f) && !done.contains(&f) {
                done.push(f);
                w = World::load(&w.save(), threads).unwrap();
            }
        }
        steps(&mut w, 30 * 30);
        (w.full_state_hash(), spots(&w, 1))
    };
    let a = run(1, 1, &[]);
    assert!(a.1.first().is_some_and(|s| s.marked && s.evidence >= 6));
    assert_eq!(run(4, 1, &[]), a, "4 threads");
    // Round 3 (marker pending) and round 6 (sharpening).
    assert_eq!(run(1, 1, &[3, 6]), a, "save/load");
    assert_ne!(run(1, 0, &[]).0, a.0, "control: switch 0");
}

/// K4: map fire on an own marker is known ground. Control:
/// `counter_battery` 0 (no marker): `UnknownGround`.
#[test]
fn map_fire_on_a_marker_is_accepted() {
    let run = |on: u32| {
        let (mut w, gun, _) = cb_scene(1, |s| {
            s.rules.fire_mission.as_mut().unwrap().counter_battery = on;
        });
        // Player 1's own gun, 900 m from player 0's.
        let us = spawn(&mut w, 1, "ww2:usa_105mm_m3_squad", 950, 300);
        steps(&mut w, 30);
        map_fire(&mut w, gun, 3);
        run_out(&mut w, gun, 300);
        steps(&mut w, 20 * 30);
        let at = spots(&w, 1).first().map_or(c(GUN.0, GUN.1), |s| s.at);
        w.step(&[crate::command::Command {
            player: PlayerId(1),
            seq: 7,
            kind: crate::command::CommandKind::FireMission {
                squad: us,
                target: at,
                rounds: 2,
            },
        }]);
        let refusal = w
            .snapshot()
            .fire_mission_refusals
            .iter()
            .find(|e| e.squad == us)
            .map(|e| e.refusal);
        (refusal, mission(&w, us).map(|m| m.cb))
    };
    assert_eq!(run(1), (None, Some(true)), "accepted, flagged");
    assert_eq!(
        run(0).0,
        Some(crate::fire_mission::FireMissionRefusal::UnknownGround),
        "control: no marker"
    );
}

/// K4: a marker reveals nothing: the gun's `seen_by` and player 1's
/// knowledge map are as with the switch off. Control: the marker exists
/// (the runs differ in that only).
#[test]
fn marker_does_not_reveal_the_gun() {
    let run = |on: u32| {
        let (mut w, gun, _) = cb_scene(1, |s| {
            s.rules.fire_mission.as_mut().unwrap().counter_battery = on;
        });
        map_fire(&mut w, gun, 4);
        run_out(&mut w, gun, 300);
        steps(&mut w, 20 * 30);
        let v = w.game().unwrap().squads[&gun].members[0];
        (
            w.state.entities.combat[v.index()].seen_by,
            pl(&w, 1).intel.clone(),
            spots(&w, 1).iter().any(|s| s.marked),
        )
    };
    let (on, off) = (run(1), run(0));
    assert!(on.2 && !off.2, "control: only the marker differs");
    assert_eq!((on.0, on.1), (off.0, off.1));
}

/// K4: a gun mission within `intel_snap_m` of an own marker is flagged
/// counter-fire (`cb_missions`). Control: 60 m off the marker (known by
/// intel there): not flagged.
#[test]
fn counter_fire_mission_is_flagged() {
    let run = |off: i32| {
        let (mut w, gun, _) = cb_scene(1, |_| {});
        let us = spawn(&mut w, 1, "ww2:usa_105mm_m3_squad", 950, 300);
        steps(&mut w, 30);
        map_fire(&mut w, gun, 3);
        run_out(&mut w, gun, 300);
        steps(&mut w, 20 * 30);
        let at = spots(&w, 1)[0].at + c(0, off);
        assert!(w.mark_intel(PlayerId(1), at));
        w.step(&[crate::command::Command {
            player: PlayerId(1),
            seq: 7,
            kind: crate::command::CommandKind::FireMission {
                squad: us,
                target: at,
                rounds: 2,
            },
        }]);
        let m = mission(&w, us).expect("accepted");
        (m.cb, pl(&w, 1).s11a2.cb_missions)
    };
    assert_eq!(run(0), (true, 1));
    assert_eq!(run(60), (false, 0), "control: 60 m off");
}
