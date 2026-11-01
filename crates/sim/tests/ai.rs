//! AI commander (S1 stage 4): commands go through the normal command path
//! after the reaction delay, AI-vs-AI matches obey the rules the AI is meant
//! to follow (retreat only weak squads, engineers dig at points, mortars
//! fire), finish with a winner, and are deterministic across thread counts
//! and save → load → continue.

use std::collections::BTreeSet;
use std::path::Path;

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs, PlayerRequest, SquadRole};
use frontline_sim::{CommandKind, Fx, FxVec2, Order, Outcome, PlayerId, SquadId, World};

fn setup(seed: u64, d0: Difficulty, d1: Difficulty, ai0: bool) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let req = MatchRequest {
        rules: "ww2:skirmish".into(),
        map: "ww2:kessel_ridge".into(),
        seed,
        players: vec![
            PlayerRequest {
                faction: "ww2:germany".into(),
                ai: ai0,
                difficulty: d0,
            },
            PlayerRequest {
                faction: "ww2:usa".into(),
                ai: true,
                difficulty: d1,
            },
        ],
    };
    defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"))
}

fn ai_vs_ai(seed: u64, threads: usize) -> World {
    World::new_match(
        setup(seed, Difficulty::Normal, Difficulty::Normal, true),
        threads,
    )
    .unwrap()
}

#[test]
fn ai_commands_wait_for_the_reaction_delay() {
    for (d, reaction) in [
        (Difficulty::Easy, 120u64),
        (Difficulty::Normal, 60),
        (Difficulty::Hard, 30),
    ] {
        // Player 0 plans at tick 0. Player 1 (Easy: 3 s re-plan, 4 s
        // reaction) plans at tick 45, so none of its orders land before 165.
        let mut w = World::new_match(setup(1, d, Difficulty::Easy, true), 1).unwrap();
        w.step(&[]);
        let g = w.game().unwrap();
        let ai = g.ai(PlayerId(0)).unwrap();
        assert!(!ai.pending.is_empty(), "{d:?}: no plan at tick 0");
        assert!(ai.pending.iter().all(|c| c.due == reaction));
        let seqs: Vec<u32> = ai.pending.iter().map(|c| c.seq).collect();
        assert_eq!(seqs, (0..seqs.len() as u32).collect::<Vec<_>>());
        assert!(g.ai(PlayerId(1)).unwrap().pending.is_empty());
        let n = ai.pending.len() as u64;
        while w.tick() < reaction {
            w.step(&[]);
        }
        assert_eq!(w.stats().commands_applied, 0);
        w.step(&[]);
        // All of them applied on the due tick, through the normal path.
        assert_eq!(w.stats().commands_applied, n, "{d:?}");
        assert_eq!(w.stats().commands_rejected, 0);
        assert!(
            w.game()
                .unwrap()
                .ai(PlayerId(0))
                .unwrap()
                .pending
                .iter()
                .all(|c| c.due > reaction)
        );
        // Its first orders send the starting squads out of the base.
        let moving = w
            .game()
            .unwrap()
            .squads()
            .values()
            .filter(|s| s.owner == PlayerId(0) && s.order != Order::Idle)
            .count();
        assert!(moving >= 2, "{d:?}: {moving} squads ordered");
    }
}

#[test]
fn ai_takes_the_map_from_an_idle_human() {
    let mut w =
        World::new_match(setup(2, Difficulty::Normal, Difficulty::Normal, false), 2).unwrap();
    while w.tick() < 6 * 60 * 30 {
        w.step(&[]);
    }
    let g = w.game().unwrap();
    let held = g
        .points()
        .iter()
        .filter(|c| c.owner == Some(PlayerId(1)))
        .count();
    assert!(held >= 4, "AI holds {held} points");
    assert!(g.players()[0].tickets < 500);
    // It bought reinforcements and never had a command refused.
    let ai = g.ai(PlayerId(1)).unwrap();
    assert!(
        g.squads()
            .values()
            .filter(|s| s.owner == PlayerId(1))
            .count()
            > 3
    );
    assert_eq!(ai.rejected, 0);
    assert!(ai.issued > 10);
}

/// What an AI-vs-AI match showed, checked as it ran.
#[derive(Default)]
struct Seen {
    retreats: BTreeSet<SquadId>,
    digs: BTreeSet<SquadId>,
    barrages: BTreeSet<SquadId>,
}

#[test]
fn ai_vs_ai_plays_by_the_rules() {
    let mut w = ai_vs_ai(3, 2);
    let s = w.match_setup().unwrap().clone();
    let pct = s.rules.ai.retreat_strength_pct as usize;
    let mut seen = Seen::default();
    while w.tick() < 12 * 60 * 30 && w.game().unwrap().outcome() == Outcome::Running {
        w.step(&[]);
        if !w.tick().is_multiple_of(15) {
            continue;
        }
        let g = w.game().unwrap();
        for (&id, sq) in g.squads() {
            let ty = &s.squad_types[sq.type_idx as usize];
            let lead = w.entities().transform()[sq.members[0].index()].pos.xy();
            match &sq.order {
                // Only weak squads fall back (strength only drops later).
                // S3 vehicles fall back on damage instead (`ai_vehicles.rs`).
                // S3c: crews walk back to base at any strength.
                Order::Retreat
                    if ty.vehicle.is_none()
                        && ty.role != SquadRole::Crew
                        && seen.retreats.insert(id) =>
                {
                    assert!(
                        sq.members.len() * 100 < ty.men as usize * pct + 100,
                        "squad {id:?} retreated with {}/{} men",
                        sq.members.len(),
                        ty.men
                    );
                }
                // Engineers dig, next to a capture point.
                Order::Dig { line, .. } if seen.digs.insert(id) => {
                    assert_eq!(ty.role, SquadRole::Engineer);
                    let mid = (line[0] + line[line.len() - 1]).div_int(2);
                    let near = s
                        .map
                        .points
                        .iter()
                        .any(|p| (FxVec2::from(p.pos) - mid).length() < Fx::from_int(30));
                    assert!(near, "trench at {mid:?} is not at a point");
                }
                // Mortars fire at targets inside their range band.
                Order::Barrage { target, .. } if seen.barrages.insert(id) => {
                    assert_eq!(ty.role, SquadRole::Mortar);
                    let wpn = &s.weapons[ty.weapon as usize];
                    let d = (*target - lead).length();
                    assert!(d >= Fx::from(wpn.range_min), "barrage at {d:?}");
                }
                _ => {}
            }
        }
    }
    let g = w.game().unwrap();
    for p in 0..2u8 {
        let ai = g.ai(PlayerId(p)).unwrap();
        assert!(ai.issued > 50, "player {p} issued {}", ai.issued);
        assert!(
            ai.rejected * 50 <= ai.issued,
            "player {p}: {} of {} commands rejected",
            ai.rejected,
            ai.issued
        );
        // Trenches the AI planned sit in front of the point, across it.
        for t in &ai.trenches {
            let at: FxVec2 = s.map.points[t.point as usize].pos.into();
            let len = (t.line[1] - t.line[0]).length();
            assert!(len <= Fx::from(s.rules.ai.dig_length) + Fx::ONE);
            let mid = (t.line[0] + t.line[1]).div_int(2);
            assert!((mid - at).length() <= Fx::from(s.rules.ai.dig_offset) + Fx::ONE);
        }
    }
    assert!(!seen.retreats.is_empty(), "nobody retreated");
    assert!(!seen.digs.is_empty(), "nobody dug");
    assert!(!seen.barrages.is_empty(), "no mortar fired");
    assert!(w.stats().deaths > 20 && w.stats().explosions > 0);
    assert!(w.terrain().detail().tiles().len() > 2, "no trench was dug");
    // Both sides bought squads beyond their three starters.
    for p in 0..2u8 {
        let bought = g
            .squads()
            .values()
            .filter(|sq| sq.owner == PlayerId(p))
            .count()
            + g.players()[p as usize].losses as usize;
        assert!(bought > 6, "player {p}");
    }
}

/// Runs to `until`, collecting published hashes; checks the incremental
/// hash against a full recompute every `full_every` ticks (0: never).
fn run(w: &mut World, until: u64, full_every: u64) -> Vec<(u64, u64)> {
    let mut out = Vec::new();
    while w.tick() < until {
        if let Some(h) = w.step(&[]) {
            out.push((w.tick(), h));
        }
        if full_every > 0 && w.tick().is_multiple_of(full_every) {
            assert_eq!(w.state_hash(), w.full_state_hash(), "tick {}", w.tick());
        }
    }
    out
}

#[test]
fn ai_vs_ai_is_identical_across_thread_counts() {
    let end = 5 * 60 * 30;
    let mut reference_world = ai_vs_ai(11, 1);
    let reference = run(&mut reference_world, end, 97);
    let st = reference_world.stats();
    assert!(st.deaths > 0 && st.commands_applied > 50);
    for threads in [2, 3, 8] {
        let mut w = ai_vs_ai(11, threads);
        assert_eq!(run(&mut w, end, 0), reference, "{threads} threads");
    }
}

#[test]
fn ai_vs_ai_survives_save_load_continue() {
    let end = 5 * 60 * 30;
    let mut straight = ai_vs_ai(12, 2);
    let reference = run(&mut straight, end, 0);

    // Save mid-fight, with AI commands waiting for their delay and rounds
    // in flight.
    let mut first = ai_vs_ai(12, 2);
    let mut mid = 4 * 60 * 30;
    run(&mut first, mid, 0);
    let busy = |w: &World| {
        let g = w.game().unwrap();
        w.projectiles().live() > 0 && (0..2).all(|p| !g.ai(PlayerId(p)).unwrap().pending.is_empty())
    };
    while !busy(&first) {
        mid += 1;
        run(&mut first, mid, 0);
        assert!(mid < end - 600, "no busy moment to save in");
    }
    let bytes = first.save();
    let mut resumed = World::load(&bytes, 3).unwrap();
    assert_eq!(resumed.state(), first.state());
    assert_eq!(resumed.state_hash(), first.state_hash());
    drop(first);
    let tail: Vec<_> = reference.into_iter().filter(|(t, _)| *t > mid).collect();
    assert_eq!(run(&mut resumed, end, 31), tail);
    assert_eq!(resumed.save(), straight.save(), "final save bytes differ");
}

#[test]
fn ai_vs_ai_runs_are_seed_sensitive() {
    let mut a = ai_vs_ai(21, 2);
    let mut b = ai_vs_ai(22, 2);
    let ha = run(&mut a, 3 * 60 * 30, 0);
    let hb = run(&mut b, 3 * 60 * 30, 0);
    assert_ne!(ha.last(), hb.last());
}

/// The S1 "done" bar: AI-vs-AI ends with a winner inside 30 game minutes.
/// Slow in debug builds; `cargo test --release` runs it.
#[test]
#[cfg_attr(debug_assertions, ignore = "long; run with --release")]
fn ai_vs_ai_matches_end_with_a_winner() {
    for seed in 1..=5 {
        let mut w = ai_vs_ai(seed, 2);
        while w.tick() < 30 * 60 * 30 && w.game().unwrap().outcome() == Outcome::Running {
            w.step(&[]);
        }
        let g = w.game().unwrap();
        assert!(
            matches!(g.outcome(), Outcome::Won { .. }),
            "seed {seed}: {:?} at tick {}",
            g.outcome(),
            w.tick()
        );
        // The loser ran out of tickets (or men); stats are filled in.
        let snap = w.snapshot();
        assert!(snap.players.iter().all(|p| p.kills + p.losses > 0));
        assert!(snap.players.iter().any(|p| p.hold_s > 0));
    }
}

#[test]
fn ai_commands_are_ordinary_commands() {
    // A human command for the AI's player slot is applied alongside the
    // AI's own; nothing in the AI path bypasses validation.
    let mut w = ai_vs_ai(4, 1);
    let sq = *w
        .game()
        .unwrap()
        .squads()
        .iter()
        .find(|(_, s)| s.owner == PlayerId(1))
        .unwrap()
        .0;
    w.step(&[frontline_sim::Command {
        player: PlayerId(1),
        seq: 1_000_000,
        kind: CommandKind::Retreat { squad: sq },
    }]);
    assert_eq!(w.game().unwrap().squad(sq).unwrap().order, Order::Retreat);
    // A command for the AI's slot naming a squad it does not own is refused
    // like any other.
    let enemy = *w
        .game()
        .unwrap()
        .squads()
        .iter()
        .find(|(_, s)| s.owner == PlayerId(0))
        .unwrap()
        .0;
    let before = w.stats().commands_rejected;
    w.step(&[frontline_sim::Command {
        player: PlayerId(1),
        seq: 1_000_001,
        kind: CommandKind::Stop { squad: enemy },
    }]);
    assert_eq!(w.stats().commands_rejected, before + 1);
}

#[test]
fn human_attack_move_over_the_ridge_reaches_a_firefight() {
    // Regression: squads halted on a focus they saw over the C ridge crest
    // but could not shoot, and both sides froze there for the whole match.
    // A scripted human sends every idle squad to C; small arms must fire
    // within five minutes.
    let mut w =
        World::new_match(setup(1, Difficulty::Normal, Difficulty::Normal, false), 2).unwrap();
    let target = FxVec2::from_int(512, 512);
    let mut seq = 0;
    while w.tick() < 5 * 60 * 30 && w.stats().shots_fired == 0 {
        let mut cmds = Vec::new();
        if w.tick().is_multiple_of(60) {
            for (id, sq) in w.game().unwrap().squads() {
                if sq.owner == PlayerId(0) && sq.order == Order::Idle {
                    cmds.push(frontline_sim::Command {
                        player: PlayerId(0),
                        seq,
                        kind: CommandKind::AttackMove { squad: *id, target },
                    });
                    seq += 1;
                }
            }
        }
        w.step(&cmds);
    }
    assert!(w.stats().shots_fired > 0, "no shots in five minutes");
}
