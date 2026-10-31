//! Determinism tests (§5.11, §14): incremental vs full hash, thread-count
//! fuzz, save → load → continue, repeat runs, and M0 system sanity checks.

use frontline_sim::{
    Command, CommandKind, Fx, FxVec2, PlayerId, SimConfig, SimId, TerrainKind, TickRate, World,
};

/// Small but busy scenario: dense enough for ORCA contacts, pool cap low
/// enough that some shots are dropped.
fn config(seed: u64) -> SimConfig {
    SimConfig {
        seed,
        soldiers: 400,
        map_size_m: 256,
        projectile_cap: 400,
        ..SimConfig::default()
    }
}

/// Scripted command stream: moves, spawns, despawns and some invalid
/// commands, delivered in reverse (unsorted) order.
fn commands_for(tick: u64) -> Vec<Command> {
    let mut cmds = Vec::new();
    let t = tick as u32;
    if tick.is_multiple_of(7) {
        let unit = SimId((t * 13) % 400);
        let target = FxVec2::from_int(((t * 31) % 256) as i32, ((t * 17) % 256) as i32);
        cmds.push(Command {
            player: PlayerId((unit.0 % 2) as u8),
            seq: t,
            kind: CommandKind::MoveSoldier { unit, target },
        });
        // Wrong owner: rejected.
        cmds.push(Command {
            player: PlayerId(((unit.0 + 1) % 2) as u8),
            seq: t,
            kind: CommandKind::MoveSoldier { unit, target },
        });
    }
    if tick % 50 == 10 {
        let unit = SimId((t / 50 * 37) % 400);
        cmds.push(Command {
            player: PlayerId((unit.0 % 2) as u8),
            seq: t + 1,
            kind: CommandKind::Despawn { unit },
        });
    }
    if tick % 50 == 35 {
        let pos = FxVec2::from_int(((t * 7) % 256) as i32, ((t * 11) % 256) as i32);
        cmds.push(Command {
            player: PlayerId((t / 50 % 2) as u8),
            seq: t + 2,
            kind: CommandKind::Spawn { pos },
        });
        // Player out of range: rejected.
        cmds.push(Command {
            player: PlayerId(9),
            seq: 0,
            kind: CommandKind::Spawn { pos },
        });
    }
    cmds.reverse();
    cmds
}

/// Runs `world` up to `until` ticks, returning every published `(tick, hash)`.
fn run_to(world: &mut World, until: u64) -> Vec<(u64, u64)> {
    let mut out = Vec::new();
    while world.tick() < until {
        let cmds = commands_for(world.tick());
        if let Some(h) = world.step(&cmds) {
            out.push((world.tick(), h));
        }
    }
    out
}

#[test]
fn incremental_hash_matches_full_recompute() {
    let mut w = World::new(config(11), 4).unwrap();
    assert_eq!(w.state_hash(), w.full_state_hash());
    for _ in 0..900 {
        let cmds = commands_for(w.tick());
        let published = w.step(&cmds);
        let inc = w.state_hash();
        assert_eq!(
            inc,
            w.full_state_hash(),
            "incremental hash diverged at tick {}",
            w.tick()
        );
        if let Some(h) = published {
            assert_eq!(h, inc);
        }
    }
    let s = w.stats();
    assert!(
        s.shots_fired > 0 && s.ground_hits + s.expired + s.out_of_bounds > 0,
        "{s:?}"
    );
    assert!(s.commands_applied > 0 && s.commands_rejected > 0, "{s:?}");
}

#[test]
fn thread_count_fuzz_identical_hashes() {
    let reference = run_to(&mut World::new(config(7), 1).unwrap(), 600);
    assert_eq!(reference.len(), 20, "one publish every 30 ticks");
    for threads in [2, 3, 8] {
        let got = run_to(&mut World::new(config(7), threads).unwrap(), 600);
        assert_eq!(got, reference, "hashes differ with {threads} threads");
    }
}

#[test]
fn save_load_continue_matches_straight_run() {
    let mut straight = World::new(config(3), 2).unwrap();
    let straight_hashes = run_to(&mut straight, 900);

    let mut first = World::new(config(3), 2).unwrap();
    run_to(&mut first, 300);
    let bytes = first.save();
    drop(first);
    // Load with a different worker count; that must not matter either.
    let mut resumed = World::load(&bytes, 3).unwrap();
    assert_eq!(resumed.tick(), 300);
    let resumed_hashes = run_to(&mut resumed, 900);

    let tail: Vec<_> = straight_hashes
        .into_iter()
        .filter(|(t, _)| *t > 300)
        .collect();
    assert_eq!(resumed_hashes, tail);
    assert_eq!(resumed.full_state_hash(), straight.full_state_hash());
    assert_eq!(resumed.save(), straight.save(), "final save bytes differ");
    assert_eq!(resumed.snapshot(), straight.snapshot());
}

/// Committed final hash of `config(7)` with `commands_for` at tick 600.
///
/// CI runs this test on every OS, so each platform is checked against the
/// same constant (not just against itself), with the command path (move,
/// spawn, despawn, id reuse, rejections) included. A change that alters
/// results on purpose must update this value in the same commit and say so.
const GOLDEN_CONFIG7_TICK600: u64 = 0xcb92_37c8_181f_bf35;

#[test]
fn golden_hash_with_commands() {
    let mut w = World::new(config(7), 3).unwrap();
    run_to(&mut w, 600);
    assert_eq!(
        w.state_hash(),
        GOLDEN_CONFIG7_TICK600,
        "sim results changed: got {:#018x}",
        w.state_hash()
    );
}

#[test]
fn save_off_publish_tick_then_continue() {
    // Tick 437 is not a multiple of hash_interval (30), with projectiles in
    // flight and commands both before and after the save.
    let mut straight = World::new(config(6), 2).unwrap();
    let straight_hashes = run_to(&mut straight, 720);

    let mut first = World::new(config(6), 2).unwrap();
    run_to(&mut first, 437);
    assert!(first.projectiles().live() > 0);
    let mut resumed = World::load(&first.save(), 5).unwrap();
    drop(first);
    let resumed_hashes = run_to(&mut resumed, 720);

    let tail: Vec<_> = straight_hashes
        .into_iter()
        .filter(|(t, _)| *t > 437)
        .collect();
    assert_eq!(resumed_hashes, tail);
    assert_eq!(resumed.save(), straight.save());
}

#[test]
fn thread_count_fuzz_many_chunks() {
    // 2,600 soldiers and 2,600 pool slots give 11 chunks each, so 3 and 8
    // workers really run chunks in parallel (and out of order).
    let cfg = SimConfig {
        soldiers: 2600,
        projectile_cap: 2600,
        map_size_m: 512,
        spawn_area_m: 256,
        ..config(12)
    };
    let run = |threads| {
        let mut w = World::new(cfg.clone(), threads).unwrap();
        let hashes = run_to(&mut w, 240);
        assert!(w.entities().chunk_count() >= 10 && w.projectiles().chunk_count() >= 10);
        hashes
    };
    let reference = run(1);
    for threads in [3, 8] {
        assert_eq!(
            run(threads),
            reference,
            "hashes differ with {threads} threads"
        );
    }
}

#[test]
fn save_mid_flight_projectiles_round_trips() {
    let mut w = World::new(config(5), 2).unwrap();
    run_to(&mut w, 137);
    assert!(
        w.projectiles().live() > 0,
        "want projectiles in flight at save time"
    );
    let loaded = World::load(&w.save(), 1).unwrap();
    assert_eq!(loaded.state(), w.state());
    assert_eq!(loaded.state_hash(), w.state_hash());
}

#[test]
fn separate_runs_identical_and_seed_sensitive() {
    let a = run_to(&mut World::new(config(42), 3).unwrap(), 450);
    let b = run_to(&mut World::new(config(42), 3).unwrap(), 450);
    assert_eq!(a, b);
    let c = run_to(&mut World::new(config(43), 3).unwrap(), 450);
    assert!(
        a.iter().zip(&c).all(|(x, y)| x.1 != y.1),
        "different seeds should never collide"
    );
}

#[test]
fn load_rejects_garbage() {
    assert!(World::load(b"not a save", 1).is_err());
    let mut bytes = World::new(config(1), 1).unwrap().save();
    bytes[0] ^= 0xFF;
    assert!(World::load(&bytes, 1).is_err());
}

#[test]
fn tick_rate_20hz_uses_three_substeps() {
    assert_eq!(TickRate::Hz30.substeps(), 2);
    assert_eq!(TickRate::Hz20.substeps(), 3);
    let cfg = SimConfig {
        tick_rate: TickRate::Hz20,
        hash_interval: 20,
        ..config(9)
    };
    let a = run_to(&mut World::new(cfg.clone(), 1).unwrap(), 400);
    let b = run_to(&mut World::new(cfg, 8).unwrap(), 400);
    assert_eq!(a.len(), 20);
    assert_eq!(a, b);
}

#[test]
fn projectile_pool_is_capped() {
    let cfg = SimConfig {
        projectile_cap: 64,
        ..config(2)
    };
    let mut w = World::new(cfg, 2).unwrap();
    for _ in 0..200 {
        w.step(&[]);
        assert!(w.projectiles().live() <= 64);
        assert_eq!(w.snapshot().projectiles.len(), w.projectiles().live());
    }
    assert!(w.stats().shots_dropped > 0);
}

#[test]
fn snapshot_matches_state() {
    let mut w = World::new(config(4), 2).unwrap();
    run_to(&mut w, 60);
    let snap = w.snapshot();
    assert_eq!(snap.tick, 60);
    assert_eq!(snap.soldiers.len(), w.entities().soldier_count());
    assert_eq!(snap.soldier_ids.len(), snap.soldiers.len());
    let id = snap.soldier_ids[5];
    let t = w.entities().transform()[id.index()];
    assert_eq!(
        snap.soldiers[5],
        [
            t.pos.x.to_f32(),
            t.pos.y.to_f32(),
            t.pos.z.to_f32(),
            t.yaw.to_f32()
        ]
    );
}

#[test]
fn id_allocator_reuses_freed_ids() {
    let cfg = SimConfig {
        soldiers: 4,
        projectile_cap: 0,
        ..config(8)
    };
    let mut w = World::new(cfg, 1).unwrap();
    let despawn = Command {
        player: PlayerId(1),
        seq: 0,
        kind: CommandKind::Despawn { unit: SimId(1) },
    };
    w.step(&[despawn]);
    assert!(!w.entities().is_alive(SimId(1)));
    assert_eq!(w.entities().allocator().free_list(), &[1]);
    let spawn = Command {
        player: PlayerId(0),
        seq: 0,
        kind: CommandKind::Spawn {
            pos: FxVec2::from_int(10, 10),
        },
    };
    w.step(&[spawn]);
    assert!(w.entities().is_alive(SimId(1)));
    assert_eq!(w.entities().owner()[1], PlayerId(0));
    assert_eq!(w.entities().allocator().high_water(), 4);
}

#[test]
fn soldiers_walk_and_avoid_each_other() {
    // Two soldiers on flat ground ordered through each other head-on.
    let cfg = SimConfig {
        soldiers: 0,
        terrain: TerrainKind::Flat,
        projectile_cap: 0,
        ..config(1)
    };
    let mut w = World::new(cfg, 1).unwrap();
    let spawn = |player: u8, x: i32| Command {
        player: PlayerId(player),
        seq: 0,
        kind: CommandKind::Spawn {
            pos: FxVec2::from_int(x, 100),
        },
    };
    w.step(&[spawn(0, 90), spawn(1, 110)]);
    let order = |player: u8, unit: u32, x: i32| Command {
        player: PlayerId(player),
        seq: 1,
        kind: CommandKind::MoveSoldier {
            unit: SimId(unit),
            target: FxVec2::from_int(x, 100),
        },
    };
    w.step(&[order(0, 0, 112), order(1, 1, 88)]);
    let mut min_dist = Fx::MAX;
    for _ in 0..30 * 25 {
        w.step(&[]);
        let t = w.entities().transform();
        min_dist = min_dist.min((t[0].pos.xy() - t[1].pos.xy()).length());
        let m = w.entities().mover();
        if !m[0].ordered && !m[1].ordered {
            break;
        }
    }
    let t = w.entities().transform();
    let m = w.entities().mover();
    assert!(
        !m[0].ordered && !m[1].ordered,
        "both should reach their targets: {:?} {:?}",
        t[0].pos,
        t[1].pos
    );
    // Combined radius is 0.8 m; allow a little discretisation slack.
    assert!(
        min_dist >= Fx::from_milli(700),
        "soldiers passed through each other (min {min_dist})"
    );
}
