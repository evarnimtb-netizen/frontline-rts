//! Combat (S1 stage 3): spotting, firing, hits, suppression, mortars,
//! deaths and corpses on Kessel Ridge, trench cover against rifles and
//! mortars, the projectile pool under load, and determinism of all of it
//! (thread counts, incremental hash, save → load → continue mid-fight).
//!
//! Squads are staged with `World::place_squad` / `World::dig_trench_now`
//! (setup tools, not commands) on open ground east of the map centre, where
//! the map seed gives clear lines of sight at these ranges.

use std::path::Path;

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::{
    Command, CommandKind, EntityKind, Fx, FxVec2, NavPhase, Order, OrderKind, PlayerId, SquadId,
    Stance, World,
};

/// The S1-S3c content frozen before the S4 content switch: these mechanics
/// tests were written for its squads (8-man rifle squads, 3-man MG teams)
/// and keep running on it; the S4 cards are covered by `s4_members.rs`,
/// `variants.rs` and the AI-vs-AI runs.
const S3C_MOD: &str = "../../sdk/crates/content/tests/fixtures/ww2_s3c";

fn setup(seed: u64) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(S3C_MOD);
    let defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    defs.resolve(&MatchRequest::skirmish(
        seed,
        "ww2:germany",
        "ww2:usa",
        Difficulty::Normal,
    ))
    .map(|mut s| {
        // These tests drive both sides by hand, and call in several squads
        // of one type at once (S3b re-call cooldowns off).
        s.players.iter_mut().for_each(|p| p.is_ai = false);

        s.squad_types
            .iter_mut()
            .filter(|t| t.vehicle.is_none())
            .for_each(|t| t.recall_cooldown_ticks = 0);
        s
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

fn world(seed: u64, threads: usize) -> World {
    World::new_match(setup(seed), threads).unwrap()
}

fn cmd(player: u8, seq: u32, kind: CommandKind) -> Command {
    Command {
        player: PlayerId(player),
        seq,
        kind,
    }
}

const EAST: FxVec2 = FxVec2::new(Fx::ONE, Fx::ZERO);
const WEST: FxVec2 = FxVec2::new(Fx::NEG_ONE, Fx::ZERO);

/// Player `p`'s first squad of type `ty`.
fn squad(w: &World, p: u8, ty: &str) -> SquadId {
    let t = w.match_setup().unwrap().squad_type_index(ty).unwrap();
    *w.game()
        .unwrap()
        .squads()
        .iter()
        .find(|(_, s)| s.owner == PlayerId(p) && s.type_idx == t)
        .unwrap()
        .0
}

/// Buys a squad of type `ty` for player `p` and steps until it arrives.
fn call_in(w: &mut World, p: u8, ty: &str) -> SquadId {
    let t = w.match_setup().unwrap().squad_type_index(ty).unwrap();
    let known: Vec<SquadId> = w.game().unwrap().squads().keys().copied().collect();
    let seq = w.tick() as u32;
    w.step(&[cmd(p, seq, CommandKind::CallIn { squad_type: t })]);
    for _ in 0..30 * 60 {
        if let Some(id) = w
            .game()
            .unwrap()
            .squads()
            .keys()
            .find(|id| !known.contains(id))
        {
            return *id;
        }
        w.step(&[]);
    }
    panic!("call-in never arrived");
}

fn losses(w: &World, p: usize) -> u32 {
    w.game().unwrap().players()[p].losses
}

fn alive(w: &World, sq: SquadId) -> bool {
    w.game().unwrap().squad(sq).is_some()
}

#[test]
fn squads_spot_fight_and_leave_corpses() {
    let mut w = world(1, 2);
    let a = squad(&w, 0, "ww2:ger_rifle_squad");
    let b = squad(&w, 1, "ww2:usa_rifle_squad");
    w.place_squad(a, FxVec2::from_int(430, 420), EAST);
    w.place_squad(b, FxVec2::from_int(550, 420), WEST);
    let corpse_ticks = w.match_setup().unwrap().rules.combat.corpse_ticks as u64;
    let (mut seen_both, mut shots_seen, mut first_corpse) = (false, false, None);
    // Corpses stay 30 s, then vanish.
    let corpse_check = |w: &World, first: Option<(frontline_sim::SimId, u64)>| {
        if let Some((id, t0)) = first {
            let kind = w.entities().kind()[id.index()];
            let listed = w.snapshot().corpses.iter().any(|c| c.id == id);
            let expect = w.tick() < t0 + corpse_ticks - 1;
            assert_eq!(kind == EntityKind::Corpse, expect, "tick {}", w.tick());
            assert_eq!(listed, expect);
        }
    };
    while alive(&w, a) && alive(&w, b) {
        w.step(&[]);
        corpse_check(&w, first_corpse);
        assert!(w.tick() < 30 * 180, "the fight never ended");
        let s = w.snapshot();
        let n = s.soldiers.len();
        assert!(
            [
                s.soldier_stance.len(),
                s.soldier_hp.len(),
                s.soldier_suppression.len(),
                s.soldier_pinned.len(),
                s.soldier_visible.len()
            ]
            .iter()
            .all(|&l| l == n)
        );
        // Enemies see each other across the 120 m gap.
        let vis = |p: u8| {
            s.soldier_owner
                .iter()
                .zip(&s.soldier_visible)
                .any(|(o, v)| *o != p && v & (1 << p) != 0)
        };
        seen_both |= vis(0) && vis(1);
        shots_seen |= !s.shots.is_empty();
        if first_corpse.is_none()
            && let Some(c) = s.corpses.first()
        {
            assert_eq!(c.ticks_left as u64, corpse_ticks - 1);
            first_corpse = Some((c.id, w.tick()));
        }
    }
    assert!(seen_both && shots_seen);
    let st = w.stats();
    assert!(st.soldier_hits > 0 && st.near_misses > 0 && st.deaths >= 8);
    assert_eq!(st.deaths as u32, losses(&w, 0) + losses(&w, 1));
    let g = w.game().unwrap();
    assert_eq!(
        g.players()[0].kills + g.players()[1].kills,
        st.deaths as u32
    );
    let (_, t0) = first_corpse.expect("someone died");
    while w.tick() < t0 + corpse_ticks {
        w.step(&[]);
        corpse_check(&w, first_corpse);
    }
}

#[test]
fn entrenched_rifles_beat_rifles_in_the_open() {
    let (mut dug, mut open) = (0, 0);
    for seed in 1..=3 {
        let mut w = world(seed, 2);
        let a = squad(&w, 0, "ww2:ger_rifle_squad");
        let b = squad(&w, 1, "ww2:usa_rifle_squad");
        // A trench across the line of fire, parapet to the east.
        w.dig_trench_now(
            &[FxVec2::from_int(420, 404), FxVec2::from_int(420, 436)],
            -1,
        );
        w.place_squad(a, FxVec2::from_int(420, 420), EAST);
        w.place_squad(b, FxVec2::from_int(570, 420), WEST);
        // Crouched in the trench at the start: hidden.
        w.step(&[]);
        let s = w.snapshot();
        let members = &w.game().unwrap().squad(a).unwrap().members;
        for (k, id) in s.soldier_ids.iter().enumerate() {
            if members.contains(id) {
                assert_eq!(s.soldier_stance[k], Stance::Crouch);
            }
        }
        while alive(&w, a) && alive(&w, b) && w.tick() < 30 * 180 {
            w.step(&[]);
        }
        dug += losses(&w, 0);
        open += losses(&w, 1);
    }
    assert!(open >= 8, "the open squad should suffer: {open}");
    assert!(dug * 2 <= open, "trench {dug} vs open {open}");
}

#[test]
fn spotting_follows_range_cover_and_woods() {
    let mut s = setup(2);
    s.rules.start_manpower = 10_000;
    let mut w = World::new_match(s, 2).unwrap();
    let rifle = squad(&w, 1, "ww2:usa_rifle_squad");
    let eng = squad(&w, 1, "ww2:usa_engineer_squad");
    // Mortar crews never use small arms, so they stay as placed.
    let near = call_in(&mut w, 0, "ww2:ger_mortar_team");
    let dug = call_in(&mut w, 0, "ww2:ger_mortar_team");
    let far = call_in(&mut w, 0, "ww2:ger_mortar_team");
    let wood = call_in(&mut w, 0, "ww2:ger_mortar_team");
    w.dig_trench_now(
        &[FxVec2::from_int(480, 440), FxVec2::from_int(480, 460)],
        -1,
    );
    w.place_squad(rifle, FxVec2::from_int(580, 420), WEST);
    w.place_squad(near, FxVec2::from_int(480, 400), EAST); // 100 m, open
    w.place_squad(dug, FxVec2::from_int(480, 450), EAST); // ~105 m, in the trench
    w.place_squad(far, FxVec2::from_int(400, 380), EAST); // ~185 m
    // In the woods at (690, 470), 85 m from the engineers: inside their
    // sight in the open, beyond it at the woods' 50 %.
    w.place_squad(wood, FxVec2::from_int(680, 460), WEST);
    w.place_squad(eng, FxVec2::from_int(765, 460), WEST);
    for _ in 0..30 {
        w.step(&[]);
    }
    let seen = |w: &World, sq: SquadId| {
        let m = &w.game().unwrap().squad(sq).unwrap().members;
        let s = w.snapshot();
        s.soldier_ids
            .iter()
            .zip(&s.soldier_visible)
            .filter(|(id, v)| m.contains(id) && *v & 2 != 0)
            .count()
    };
    // The rifle squad opens fire at once and may already have killed one
    // (S3b: staged within the first second of the match, so a different
    // tick's rolls than when four call-ins took two minutes): every man
    // still alive is seen.
    let alive = w.game().unwrap().squad(near).unwrap().members.len();
    assert!(alive >= 2, "{alive}");
    assert_eq!(seen(&w, near), alive, "open ground at 100 m");
    assert_eq!(seen(&w, dug), 0, "crouched in a trench");
    assert_eq!(seen(&w, far), 0, "beyond sight range");
    assert_eq!(seen(&w, wood), 0, "concealed by woods");
    // ... though nothing but the woods hides it: the sight line is clear.
    let t = w.terrain();
    let eye = |xy: FxVec2| xy.extend(t.height_at(xy) + Fx::from_milli(1650));
    assert!(t.line_of_sight(
        eye(FxVec2::from_int(765, 460)),
        eye(FxVec2::from_int(680, 460))
    ));
    // Own soldiers are always visible to their owner.
    assert!(w.snapshot().soldier_visible.iter().all(|v| *v != 0));
    // Firing gives the crew in the woods away.
    let target = FxVec2::from_int(690, 700);
    w.step(&[cmd(
        0,
        1,
        CommandKind::FireMission {
            squad: wood,
            target,
            rounds: 2,
        },
    )]);
    let fired = w.stats().shots_fired;
    while w.stats().shots_fired == fired {
        w.step(&[]);
        assert!(w.tick() < 30 * 300);
    }
    for _ in 0..10 {
        w.step(&[]);
    }
    assert!(seen(&w, wood) > 0, "a shot reveals the shooter");
}

#[test]
fn mg_fire_pins_infantry() {
    let mut w = world(1, 2);
    let mg = call_in(&mut w, 0, "ww2:ger_mg_team");
    let b = squad(&w, 1, "ww2:usa_rifle_squad");
    w.place_squad(mg, FxVec2::from_int(430, 420), EAST);
    w.place_squad(b, FxVec2::from_int(550, 420), WEST);
    let mut pinned = 0;
    let mut gunner_prone = false;
    let gunner = w.game().unwrap().squad(mg).unwrap().members[0];
    for _ in 0..30 * 30 {
        w.step(&[]);
        let s = w.snapshot();
        pinned = pinned.max(s.soldier_pinned.iter().filter(|p| **p).count());
        if let Some(k) = s.soldier_ids.iter().position(|id| *id == gunner) {
            gunner_prone |= s.soldier_stance[k] == Stance::Prone && !s.soldier_pinned[k];
        }
    }
    assert!(pinned >= 2, "MG fire pinned {pinned}");
    assert!(gunner_prone, "a set-up MG fires lying down");
    assert!(w.stats().near_misses > 50);
}

#[test]
fn barrage_fires_shells_on_an_arc_and_craters() {
    let mut w = world(3, 2);
    let mortar = call_in(&mut w, 0, "ww2:ger_mortar_team");
    let b = squad(&w, 1, "ww2:usa_rifle_squad");
    w.place_squad(mortar, FxVec2::from_int(300, 420), EAST);
    w.place_squad(b, FxVec2::from_int(560, 420), WEST);
    // Closer than the minimum range: rejected.
    let rejected = w.stats().commands_rejected;
    let target = FxVec2::from_int(560, 420);
    w.step(&[cmd(
        0,
        1,
        CommandKind::FireMission {
            squad: mortar,
            target: FxVec2::from_int(330, 420),
            rounds: 3,
        },
    )]);
    assert_eq!(w.stats().commands_rejected, rejected + 1);
    w.step(&[cmd(
        0,
        2,
        CommandKind::FireMission {
            squad: mortar,
            target,
            rounds: 6,
        },
    )]);
    let tiles = w.terrain().detail().tiles().len();
    let (mut apex, mut bursts) = (Fx::ZERO, 0);
    let start_z = w.terrain().height_at(FxVec2::from_int(300, 420));
    while w.game().unwrap().squad(mortar).unwrap().order.kind() == OrderKind::FireMission {
        w.step(&[]);
        assert!(w.tick() < 30 * 120, "barrage never finished");
        for p in w.projectiles().slots().iter().flatten() {
            apex = apex.max(p.pos.z - start_z);
        }
        bursts += w.snapshot().explosions.len();
    }
    for _ in 0..30 * 20 {
        w.step(&[]);
        bursts += w.snapshot().explosions.len();
    }
    assert_eq!(bursts, 6);
    assert_eq!(w.stats().explosions, 6);
    // A steep arc: 260 m out, well over 100 m up.
    assert!(apex > Fx::from_int(100), "apex {apex:?}");
    assert!(w.terrain().detail().tiles().len() > tiles, "no craters");
    assert!(losses(&w, 1) > 0, "a barrage on a squad in the open hurts");
    // Out of range: the team first walks closer.
    w.step(&[cmd(
        0,
        3,
        CommandKind::FireMission {
            squad: mortar,
            target: FxVec2::from_int(990, 420),
            rounds: 2,
        },
    )]);
    let s = w.game().unwrap().squad(mortar).unwrap();
    assert!(matches!(s.order, Order::FireMission { rounds: 2, .. }));
    assert_ne!(s.nav.phase, NavPhase::Holding);
}

#[test]
fn entrenched_men_survive_mortars_better() {
    let (mut dug, mut open) = (0, 0);
    for seed in 1..=6 {
        for trench in [false, true] {
            let mut w = world(seed, 2);
            let rifle = squad(&w, 0, "ww2:ger_rifle_squad");
            if trench {
                w.dig_trench_now(
                    &[FxVec2::from_int(420, 400), FxVec2::from_int(420, 440)],
                    -1,
                );
            }
            w.place_squad(rifle, FxVec2::from_int(420, 420), EAST);
            let m = call_in(&mut w, 1, "ww2:usa_mortar_team");
            w.place_squad(m, FxVec2::from_int(670, 420), WEST);
            let target = FxVec2::from_int(420, 420);
            w.step(&[cmd(
                1,
                1,
                CommandKind::FireMission {
                    squad: m,
                    target,
                    rounds: 10,
                },
            )]);
            for _ in 0..30 * 90 {
                w.step(&[]);
            }
            assert_eq!(w.stats().explosions, 10);
            *(if trench { &mut dug } else { &mut open }) += losses(&w, 0);
        }
    }
    assert!(open >= 20, "a barrage on men in the open: {open}");
    assert!(dug * 100 <= open * 45, "trench {dug} vs open {open}");
}

#[test]
fn attack_move_halts_to_fight_and_move_does_not_shoot_on_the_walk() {
    let mut w = world(4, 2);
    let a = squad(&w, 0, "ww2:ger_rifle_squad");
    let b = squad(&w, 1, "ww2:usa_rifle_squad");
    w.place_squad(a, FxVec2::from_int(420, 420), EAST);
    w.place_squad(b, FxVec2::from_int(700, 420), WEST);
    // US walks past the Germans (Move): its leader never fires while
    // walking.
    let lead_b = w.game().unwrap().squad(b).unwrap().members[0];
    w.step(&[cmd(
        1,
        0,
        CommandKind::Move {
            squad: b,
            target: FxVec2::from_int(480, 300),
        },
    )]);
    while alive(&w, b)
        && w.game().unwrap().squad(b).unwrap().nav.phase != NavPhase::Holding
        && w.tick() < 30 * 200
    {
        let shots = w.entities().weapon()[lead_b.index()].shots;
        w.step(&[]);
        if !alive(&w, b) || w.game().unwrap().squad(b).unwrap().members[0] != lead_b {
            break;
        }
        let moving = w.entities().velocity()[lead_b.index()].lin.length() > Fx::from_milli(500);
        if moving {
            assert_eq!(w.entities().weapon()[lead_b.index()].shots, shots);
        }
    }
    // German AttackMove toward the Americans: it halts and fights.
    let mut w = world(4, 2);
    w.place_squad(a, FxVec2::from_int(300, 420), EAST);
    w.place_squad(b, FxVec2::from_int(560, 420), WEST);
    w.step(&[cmd(
        0,
        0,
        CommandKind::AttackMove {
            squad: a,
            target: FxVec2::from_int(700, 420),
        },
    )]);
    let mut halted = false;
    while alive(&w, a) && alive(&w, b) && w.tick() < 30 * 200 {
        w.step(&[]);
        let Some(s) = w.game().unwrap().squad(a) else {
            break;
        };
        if s.nav.phase == NavPhase::Moving && s.focus.is_some() {
            let v = w.entities().velocity()[s.members[0].index()].lin.length();
            halted |= v < Fx::from_milli(300);
        }
    }
    assert!(halted, "attack-move halts when it meets the enemy");
    assert!(losses(&w, 1) > 0);
}

#[test]
fn saturated_pool_drops_rounds_but_never_shells() {
    let mut s = setup(5);
    s.rules.projectile_cap = 40;
    s.rules.start_manpower = 10_000;
    let mut w = World::new_match(s, 2).unwrap();
    let mg1 = call_in(&mut w, 0, "ww2:ger_mg_team");
    let mg2 = call_in(&mut w, 0, "ww2:ger_mg_team");
    let mortar = call_in(&mut w, 0, "ww2:ger_mortar_team");
    let b = squad(&w, 1, "ww2:usa_rifle_squad");
    let c = squad(&w, 1, "ww2:usa_engineer_squad");
    w.place_squad(mg1, FxVec2::from_int(430, 410), EAST);
    w.place_squad(mg2, FxVec2::from_int(430, 430), EAST);
    w.place_squad(mortar, FxVec2::from_int(330, 420), EAST);
    w.place_squad(b, FxVec2::from_int(540, 410), WEST);
    w.place_squad(c, FxVec2::from_int(540, 430), WEST);
    let target = FxVec2::from_int(540, 420);
    w.step(&[cmd(
        0,
        1,
        CommandKind::FireMission {
            squad: mortar,
            target,
            rounds: 5,
        },
    )]);
    for _ in 0..30 * 60 {
        w.step(&[]);
        assert!(w.projectiles().live() <= 40);
    }
    assert!(w.stats().shots_dropped > 0, "the pool should saturate");
    assert_eq!(w.stats().explosions, 5, "every shell flies");
}

// ------------------------------------------------------------ determinism --

/// A staged mixed fight: rifles and engineers on both sides, an MG, a
/// mortar barrage, a dig under fire and an attack-move.
fn battle(seed: u64, threads: usize) -> World {
    let mut w = world(seed, threads);
    let mg = call_in(&mut w, 0, "ww2:ger_mg_team");
    let mortar = call_in(&mut w, 1, "ww2:usa_mortar_team");
    let ger: Vec<SquadId> = ["ww2:ger_rifle_squad", "ww2:ger_engineer_squad"]
        .iter()
        .map(|t| squad(&w, 0, t))
        .collect();
    let usa: Vec<SquadId> = ["ww2:usa_rifle_squad", "ww2:usa_engineer_squad"]
        .iter()
        .map(|t| squad(&w, 1, t))
        .collect();
    w.dig_trench_now(
        &[FxVec2::from_int(420, 404), FxVec2::from_int(420, 436)],
        -1,
    );
    w.place_squad(ger[0], FxVec2::from_int(420, 420), EAST);
    w.place_squad(ger[1], FxVec2::from_int(410, 450), EAST);
    w.place_squad(mg, FxVec2::from_int(415, 390), EAST);
    w.place_squad(usa[0], FxVec2::from_int(560, 420), WEST);
    w.place_squad(usa[1], FxVec2::from_int(600, 460), WEST);
    w.place_squad(mortar, FxVec2::from_int(680, 400), WEST);
    w
}

fn battle_orders(w: &World, t0: u64) -> Vec<Command> {
    let dt = w.tick() - t0;
    let mut out = Vec::new();
    if dt == 2 {
        let mortar = squad(w, 1, "ww2:usa_mortar_team");
        out.push(cmd(
            1,
            1000,
            CommandKind::FireMission {
                squad: mortar,
                target: FxVec2::from_int(420, 420),
                rounds: 8,
            },
        ));
        let eng = squad(w, 0, "ww2:ger_engineer_squad");
        let line = vec![FxVec2::from_int(405, 440), FxVec2::from_int(405, 460)];
        out.push(cmd(
            0,
            1000,
            CommandKind::Dig {
                squad: eng,
                line,
                profile: Default::default(),
            },
        ));
    }
    if dt == 150
        && let Some((&id, _)) = w
            .game()
            .unwrap()
            .squads()
            .iter()
            .find(|(_, s)| s.owner == PlayerId(1) && s.members.len() == 5)
    {
        out.push(cmd(
            1,
            1001,
            CommandKind::AttackMove {
                squad: id,
                target: FxVec2::from_int(430, 450),
            },
        ));
    }
    out
}

fn run_battle(w: &mut World, t0: u64, until: u64, check_full: bool) -> Vec<(u64, u64)> {
    let mut out = Vec::new();
    while w.tick() < until {
        let cmds = battle_orders(w, t0);
        let published = w.step(&cmds);
        if check_full {
            assert_eq!(w.state_hash(), w.full_state_hash(), "tick {}", w.tick());
        }
        if let Some(h) = published {
            out.push((w.tick(), h));
        }
    }
    out
}

#[test]
fn combat_is_identical_across_thread_counts() {
    let mut reference_world = battle(41, 1);
    let t0 = reference_world.tick();
    let reference = run_battle(&mut reference_world, t0, t0 + 1800, true);
    let st = reference_world.stats();
    assert!(st.deaths > 0 && st.explosions > 0 && st.soldier_hits > 0);
    for threads in [2, 3, 8] {
        let mut w = battle(41, threads);
        assert_eq!(w.tick(), t0);
        let got = run_battle(&mut w, t0, t0 + 1800, false);
        assert_eq!(got, reference, "hashes differ with {threads} threads");
    }
}

#[test]
fn combat_survives_save_load_continue() {
    let mut straight = battle(43, 2);
    let t0 = straight.tick();
    let end = t0 + 1500;
    let straight_hashes = run_battle(&mut straight, t0, end, false);

    let mut first = battle(43, 2);
    // Mid-fight: rounds and shells in flight, corpses on the ground.
    let mut mid = t0 + 400;
    run_battle(&mut first, t0, mid, false);
    while first.projectiles().live() == 0
        || first.snapshot().corpses.is_empty()
        || !first
            .projectiles()
            .slots()
            .iter()
            .flatten()
            .any(|p| p.pos.z > first.terrain().height_at(p.pos.xy()) + Fx::from_int(20))
    {
        mid += 1;
        run_battle(&mut first, t0, mid, false);
        assert!(mid < end - 300, "no busy moment to save in");
    }
    let bytes = first.save();
    let mut resumed = World::load(&bytes, 3).unwrap();
    assert_eq!(resumed.state(), first.state());
    assert_eq!(resumed.state_hash(), first.state_hash());
    drop(first);
    let resumed_hashes = run_battle(&mut resumed, t0, end, true);
    let tail: Vec<_> = straight_hashes
        .into_iter()
        .filter(|(t, _)| *t > mid)
        .collect();
    assert_eq!(resumed_hashes, tail);
    assert_eq!(resumed.save(), straight.save(), "final save bytes differ");
}

#[test]
fn combat_runs_are_seed_sensitive() {
    let mut a = battle(51, 2);
    let mut b = battle(52, 2);
    let (ta, tb) = (a.tick(), b.tick());
    let ha = run_battle(&mut a, ta, ta + 600, false);
    let hb = run_battle(&mut b, tb, tb + 600, false);
    assert_ne!(ha.last(), hb.last());
}
