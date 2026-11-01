//! S3 vehicles (sim stage 2, spec step 3): call-in with unlock, limit and
//! pop; the squad of one in squad and soldier-level orders; driving,
//! reversing, the block layer, push-out and avoidance; spotting of and by
//! vehicles; defeat with only vehicles left; determinism across threads and
//! save → load → continue with vehicles moving.

use std::path::Path;

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::vehicle::{node_at, sample_ground};
use frontline_sim::{
    Command, CommandKind, EntityKind, Fx, FxVec2, ItemKind, NavPhase, Order, PlayerId, SimId,
    SquadId, StanceOrder, World,
};

fn defs_and_rules(unlock: bool) -> (ModDefs, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let mut defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let rules = if unlock {
        defs.add_rules_override(&root.join("tests/rules_test_unlock.toml"))
            .unwrap_or_else(|e| panic!("{e}"))
    } else {
        "ww2:skirmish".to_owned()
    };
    (defs, rules)
}

/// Kessel Ridge with the test unlock override; both sides human.
fn setup(seed: u64, unlock: bool) -> MatchSetup {
    let (defs, rules) = defs_and_rules(unlock);
    let mut req = MatchRequest::skirmish(seed, "ww2:germany", "ww2:usa", Difficulty::Normal);
    req.rules = rules;
    let mut s = defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
    s.players.iter_mut().for_each(|p| p.is_ai = false);
    s.rules.start_manpower = 100_000;
    s
}

/// The same on flat, open ground (geometry tests).
fn flat_setup() -> MatchSetup {
    let mut s = setup(1, true);
    s.map.terrain.octaves.clear();
    s.map.terrain.bands.clear();
    s.map.woods.clear();
    s
}

fn cmd(player: u8, seq: u32, kind: CommandKind) -> Command {
    Command {
        player: PlayerId(player),
        seq,
        kind,
    }
}

fn type_index(w: &World, id: &str) -> u16 {
    w.match_setup().unwrap().squad_type_index(id).unwrap()
}

fn tank_type(w: &World, player: u8) -> u16 {
    type_index(
        w,
        if player == 0 {
            "ww2:ger_pz4_h_squad"
        } else {
            "ww2:usa_m4a3_75_squad"
        },
    )
}

fn xy(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

const EAST: FxVec2 = FxVec2::new(Fx::ONE, Fx::ZERO);

/// Spawns a tank for `player` at `at` facing `facing`; returns its squad
/// and vehicle id.
fn tank_at(w: &mut World, player: u8, at: FxVec2, facing: FxVec2) -> (SquadId, SimId) {
    let t = tank_type(w, player);
    let sq = w
        .spawn_squad_at(PlayerId(player), t, at, facing)
        .expect("spawned");
    let v = w.game().unwrap().squad(sq).unwrap().members[0];
    (sq, v)
}

fn pos(w: &World, id: SimId) -> FxVec2 {
    w.entities().transform()[id.index()].pos.xy()
}

fn dist(a: FxVec2, b: FxVec2) -> Fx {
    (a - b).length()
}

/// Steps until `done(w)` or `max_ticks`; returns whether `done` held.
fn run_until(w: &mut World, max_ticks: u64, mut done: impl FnMut(&World) -> bool) -> bool {
    for _ in 0..max_ticks {
        if done(w) {
            return true;
        }
        w.step(&[]);
    }
    done(w)
}

fn rejected(w: &World) -> u64 {
    w.stats().commands_rejected
}

#[test]
fn call_in_needs_the_unlock_time_respects_the_limit_and_counts_pop() {
    // Normal rules: locked for the first 4 minutes.
    let mut w = World::new_match(setup(1, false), 1).unwrap();
    let t = tank_type(&w, 0);
    w.step(&[cmd(0, 0, CommandKind::CallIn { squad_type: t })]);
    assert_eq!(rejected(&w), 1, "a tank before the unlock time");
    let unlock = w.match_setup().unwrap().rules.vehicles.unlock_ticks as u64;
    assert_eq!(unlock, 240 * 30);
    while w.tick() < unlock {
        w.step(&[]);
    }
    w.step(&[cmd(0, 1, CommandKind::CallIn { squad_type: t })]);
    assert_eq!(rejected(&w), 1, "accepted once unlocked");

    // Test override: unlocked at once; at most 4 alive or queued.
    let mut w = World::new_match(setup(1, true), 1).unwrap();
    let g = |w: &World| w.game().unwrap().population(w.entities(), PlayerId(0));
    let pop0 = g(&w);
    let cmds: Vec<Command> = (0..5)
        .map(|k| cmd(0, k, CommandKind::CallIn { squad_type: t }))
        .collect();
    w.step(&cmds);
    assert_eq!(rejected(&w), 1, "the fifth is over the limit");
    let game = w.game().unwrap();
    assert_eq!(game.vehicles_counted(w.entities(), PlayerId(0)), 4);
    assert_eq!(g(&w), pop0 + 4 * 12, "queued vehicles count their pop");
    assert_eq!(w.snapshot().players[0].vehicles_counted, 4);
    assert_eq!(w.snapshot().players[0].pop, pop0 + 48);
    // They arrive after the 45 s call-in and still count.
    for _ in 0..45 * 30 + 1 {
        w.step(&[]);
    }
    let game = w.game().unwrap();
    let alive = game
        .vehicles()
        .keys()
        .filter(|v| w.entities().kind()[v.index()] == EntityKind::Vehicle)
        .count();
    assert_eq!(alive, 4);
    assert_eq!(game.vehicles_counted(w.entities(), PlayerId(0)), 4);
    assert_eq!(g(&w), pop0 + 48);
    assert_eq!(w.snapshot().vehicles.len(), 4);
    let v = *game.vehicles().keys().next().unwrap();
    w.step(&[cmd(0, 9, CommandKind::CallIn { squad_type: t })]);
    assert_eq!(rejected(&w), 2, "four alive: still at the limit");
    // A wreck no longer counts.
    assert!(w.wreck_vehicle(v));
    w.step(&[cmd(0, 10, CommandKind::CallIn { squad_type: t })]);
    assert_eq!(rejected(&w), 2);

    // Pop cap: a tank needs 12 free.
    let mut s = setup(1, true);
    let start = {
        let w = World::new_match(s.clone(), 1).unwrap();
        w.game().unwrap().population(w.entities(), PlayerId(0))
    };
    s.rules.pop_cap = start + 11;
    let mut w = World::new_match(s, 1).unwrap();
    w.step(&[cmd(0, 0, CommandKind::CallIn { squad_type: t })]);
    assert_eq!(rejected(&w), 1, "over the pop cap");
}

#[test]
fn a_tank_drives_to_its_target_and_stops() {
    let mut w = World::new_match(flat_setup(), 2).unwrap();
    let (sq, v) = tank_at(&mut w, 0, xy(400, 500), EAST);
    let target = xy(600, 560);
    w.step(&[cmd(0, 0, CommandKind::Move { squad: sq, target })]);
    let mut top = Fx::ZERO;
    let arrived = run_until(&mut w, 30 * 90, |w| {
        let vs = w.game().unwrap().vehicle(v).unwrap();
        top = top.max(vs.speed);
        w.game().unwrap().squad(sq).unwrap().order == Order::Idle
    });
    assert!(arrived, "arrived: {:?}", pos(&w, v));
    assert!(dist(pos(&w, v), target) < Fx::from_int(4));
    let vs = w.game().unwrap().vehicle(v).unwrap().clone();
    assert!(top > Fx::from_int(5) && top <= Fx::from_int(6), "{top:?}");
    // It stops on arrival.
    for _ in 0..60 {
        w.step(&[]);
    }
    let vs2 = w.game().unwrap().vehicle(v).unwrap();
    assert_eq!(vs2.speed, Fx::ZERO, "{:?} → {:?}", vs.speed, vs2.speed);
    assert_eq!(
        w.game().unwrap().squad(sq).unwrap().nav.phase,
        NavPhase::Holding
    );
    // Flat ground: level hull on the ground.
    assert!(vs2.pitch.abs() < Fx::from_milli(20) && vs2.roll.abs() < Fx::from_milli(20));
    let z = w.entities().transform()[v.index()].pos.z;
    let ground = w.terrain().height_at(pos(&w, v));
    assert!((z - ground).abs() < Fx::from_milli(50));
    // The snapshot carries it.
    let snap = &w.snapshot().vehicles[0];
    assert_eq!(snap.id, v.0);
    assert_eq!(snap.owner, 0);
    assert_eq!(snap.ammo, [30, 30]);
    assert!(snap.squad == sq.0);
    assert!(w.snapshot().squads.iter().all(|s| s.id != sq.0));
}

#[test]
fn a_short_move_behind_reverses() {
    let mut w = World::new_match(flat_setup(), 1).unwrap();
    let (sq, v) = tank_at(&mut w, 0, xy(500, 500), EAST);
    let target = xy(470, 502);
    w.step(&[cmd(0, 0, CommandKind::Move { squad: sq, target })]);
    let mut slowest = Fx::ZERO;
    let arrived = run_until(&mut w, 30 * 40, |w| {
        slowest = slowest.min(w.game().unwrap().vehicle(v).unwrap().speed);
        w.game().unwrap().squad(sq).unwrap().order == Order::Idle
    });
    assert!(arrived);
    assert!(slowest < Fx::from_int(-1), "it backed up: {slowest:?}");
    assert!(slowest >= Fx::from_int(-2), "at most its reverse speed");
    let yaw = w.entities().transform()[v.index()].yaw;
    assert!(
        yaw.abs() < Fx::from_milli(400),
        "front kept toward east: {yaw:?}"
    );
    assert!(dist(pos(&w, v), target) < Fx::from_int(4));
}

#[test]
fn drive_never_puts_a_track_sample_on_an_at_ditch() {
    let mut w = World::new_match(flat_setup(), 1).unwrap();
    // A 40 m AT ditch across the way.
    assert!(w.dig_at_ditch_now(&[xy(540, 480), xy(540, 520)], 1));
    let block = |w: &World| w.game().unwrap().vehicle_derived().block.clone();
    assert!(block(&w).blocked_count() > 10);
    let (sq, v) = tank_at(&mut w, 0, xy(500, 500), EAST);
    w.step(&[cmd(
        0,
        0,
        CommandKind::Move {
            squad: sq,
            target: xy(590, 500),
        },
    )]);
    let geo = w.game().unwrap().vehicle_derived().geo[0];
    let arrived = run_until(&mut w, 30 * 120, |w| {
        let t = w.entities().transform()[v.index()];
        let g = sample_ground(w.terrain(), t.pos.xy(), t.yaw, &geo);
        let b = &w.game().unwrap().vehicle_derived().block;
        for p in g.xy.iter().flatten() {
            assert!(
                !b.is_blocked(node_at(w.terrain(), *p)),
                "track sample on a blocked node at tick {}",
                w.tick()
            );
        }
        w.game().unwrap().squad(sq).unwrap().order == Order::Idle
    });
    assert!(arrived, "drove round the ditch: {:?}", pos(&w, v));
    assert!(dist(pos(&w, v), xy(590, 500)) < Fx::from_int(4));

    // Facing the ditch at full throttle it stops short.
    let (sq2, v2) = tank_at(&mut w, 1, xy(522, 500), EAST);
    w.step(&[cmd(
        1,
        0,
        CommandKind::Move {
            squad: sq2,
            target: xy(560, 500),
        },
    )]);
    for _ in 0..30 * 20 {
        let t = w.entities().transform()[v2.index()];
        let g = sample_ground(w.terrain(), t.pos.xy(), t.yaw, &geo);
        let b = &w.game().unwrap().vehicle_derived().block;
        assert!(
            g.xy.iter()
                .flatten()
                .all(|p| !b.blocked_at(w.terrain(), *p))
        );
        w.step(&[]);
    }
}

#[test]
fn craters_and_trenches_never_block_vehicles() {
    let mut w = World::new_match(flat_setup(), 1).unwrap();
    w.dig_trench_now(&[xy(300, 300), xy(360, 300)], 1);
    assert_eq!(w.game().unwrap().vehicle_derived().block.blocked_count(), 0);
    // An AT ditch blocks only along its own line.
    w.dig_at_ditch_now(&[xy(600, 600), xy(630, 600)], 1);
    let b = w.game().unwrap().vehicle_derived().block.clone();
    assert!(b.blocked_at(w.terrain(), xy(615, 600)));
    assert!(b.blocked_at(w.terrain(), xy(615, 604)), "dilated one node");
    assert!(!b.blocked_at(w.terrain(), xy(615, 608)));
    assert!(!b.blocked_at(w.terrain(), xy(615, 616)));
    assert!(
        !b.blocked_at(w.terrain(), xy(330, 300)),
        "a trench never blocks"
    );
}

#[test]
fn soldiers_walk_round_a_parked_tank() {
    let mut w = World::new_match(flat_setup(), 1).unwrap();
    // A friendly tank (an enemy one would shoot them; avoidance is the same).
    let (_, v) = tank_at(&mut w, 0, xy(500, 500), EAST);
    let rifle = type_index(&w, "ww2:ger_rifle_squad");
    let sq = w
        .spawn_squad_at(
            PlayerId(0),
            rifle,
            xy(500, 470),
            FxVec2::new(Fx::ZERO, Fx::ONE),
        )
        .unwrap();
    let men = w.game().unwrap().squad(sq).unwrap().members.clone();
    // Walk straight through the hull to the far side.
    w.step(&[cmd(
        0,
        0,
        CommandKind::MoveUnits {
            units: men.clone(),
            target: xy(500, 530),
        },
    )]);
    let t = w.entities().transform()[v.index()];
    let geo = w.game().unwrap().vehicle_derived().geo[0];
    let (c, f) = (t.pos.xy(), FxVec2::new(Fx::ONE, Fx::ZERO));
    let mut worst = Fx::from_int(100);
    for _ in 0..30 * 40 {
        for m in &men {
            let rel = pos(&w, *m) - c;
            let (lx, ly) = (rel.dot(f).abs(), rel.det(f).abs());
            // Depth inside the footprint (negative: outside).
            let inside = (geo.hull_half.x - lx).min(geo.hull_half.y - ly);
            worst = worst.min(-inside);
        }
        w.step(&[]);
    }
    assert!(worst >= Fx::ZERO, "a man got {:?} m into the hull", -worst);
    let across = men
        .iter()
        .filter(|m| pos(&w, **m).y > Fx::from_int(515))
        .count();
    assert!(
        across >= men.len() - 1,
        "{across} of {} got round",
        men.len()
    );
}

#[test]
fn a_tank_crossing_an_occupied_trench_moves_nobody_in_it() {
    let mut s = flat_setup();
    for v in &mut s.vehicles {
        v.mobility.bog_pct = 0;
    }
    let mut w = World::new_match(s, 1).unwrap();
    // A trench across x = 500; a US rifle squad in it.
    w.dig_trench_now(&[xy(500, 470), xy(500, 530)], 1);
    let rifle = type_index(&w, "ww2:usa_rifle_squad");
    let sq = w
        .spawn_squad_at(
            PlayerId(1),
            rifle,
            xy(500, 500),
            FxVec2::new(Fx::NEG_ONE, Fx::ZERO),
        )
        .unwrap();
    w.step(&[cmd(
        1,
        0,
        CommandKind::SetStance {
            units: w.game().unwrap().squad(sq).unwrap().members.clone(),
            stance: StanceOrder::Crouch,
        },
    )]);
    for _ in 0..60 {
        w.step(&[]);
    }
    let men = w.game().unwrap().squad(sq).unwrap().members.clone();
    let cells: Vec<(u32, u32)> = men
        .iter()
        .map(|m| w.terrain().nearest_grid(pos(&w, *m)))
        .collect();
    assert!(
        men.iter()
            .all(|m| w.entities().combat()[m.index()].in_trench)
    );
    // A German tank drives across the trench right over them.
    let (tsq, v) = tank_at(&mut w, 0, xy(470, 500), EAST);
    w.step(&[cmd(
        0,
        0,
        CommandKind::Move {
            squad: tsq,
            target: xy(530, 500),
        },
    )]);
    let mut crossing = false;
    let arrived = run_until(&mut w, 30 * 120, |w| {
        crossing |= w.game().unwrap().vehicle(v).unwrap().crossing;
        w.game().unwrap().squad(tsq).unwrap().order == Order::Idle
    });
    assert!(arrived, "crossed: {:?}", pos(&w, v));
    assert!(crossing, "the tracks went over the trench");
    let after: Vec<(u32, u32)> = men
        .iter()
        .filter(|m| w.entities().kind()[m.index()] == EntityKind::Soldier)
        .map(|m| w.terrain().nearest_grid(pos(&w, *m)))
        .collect();
    assert_eq!(cells, after, "nobody in the trench changed cell");
    assert_eq!(w.game().unwrap().vehicle(v).unwrap().crossings, 1);
}

#[test]
fn a_standing_man_is_pushed_out_sideways() {
    let mut w = World::new_match(flat_setup(), 1).unwrap();
    let rifle = type_index(&w, "ww2:usa_rifle_squad");
    let sq = w
        .spawn_squad_at(
            PlayerId(1),
            rifle,
            xy(530, 500),
            FxVec2::new(Fx::ZERO, Fx::ONE),
        )
        .unwrap();
    // Men in its way step aside (ORCA against the hull circles) or are
    // pushed out sideways (the push-out unit test covers the push alone).
    let men = w.game().unwrap().squad(sq).unwrap().members.clone();
    let (tsq, v) = tank_at(&mut w, 0, xy(480, 500), EAST);
    w.step(&[cmd(
        0,
        0,
        CommandKind::Move {
            squad: tsq,
            target: xy(580, 500),
        },
    )]);
    let geo = w.game().unwrap().vehicle_derived().geo[0];
    let mut deepest = Fx::from_int(-100);
    run_until(&mut w, 30 * 60, |w| {
        let t = w.entities().transform()[v.index()];
        // Men the tank's MGs killed lie where they fell.
        for m in men
            .iter()
            .filter(|m| w.entities().kind()[m.index()] == EntityKind::Soldier)
        {
            let rel = pos(w, *m) - t.pos.xy();
            let (lx, ly) = (rel.dot(EAST).abs(), rel.det(EAST).abs());
            deepest = deepest.max((geo.hull_half.x - lx).min(geo.hull_half.y - ly));
        }
        w.game().unwrap().squad(tsq).unwrap().order == Order::Idle
    });
    assert!(
        deepest < Fx::from_milli(300),
        "nobody stays inside the moving hull: {deepest:?}"
    );
}

#[test]
fn vehicle_ids_take_unit_orders_and_soldier_only_orders_reject_them() {
    let mut w = World::new_match(flat_setup(), 1).unwrap();
    let (sq, v) = tank_at(&mut w, 0, xy(500, 500), EAST);
    let r0 = rejected(&w);
    // MoveUnits with the vehicle id: the squad of one gets a Move.
    w.step(&[cmd(
        0,
        0,
        CommandKind::MoveUnits {
            units: vec![v],
            target: xy(560, 500),
        },
    )]);
    assert_eq!(rejected(&w), r0);
    let g = w.game().unwrap();
    assert!(matches!(g.squad(sq).unwrap().order, Order::Move { .. }));
    // StopUnits.
    w.step(&[cmd(0, 1, CommandKind::StopUnits { units: vec![v] })]);
    assert_eq!(rejected(&w), r0);
    assert_eq!(w.game().unwrap().squad(sq).unwrap().order, Order::Idle);
    // A mixed set: the vehicle and the men.
    let men: Vec<SimId> = w
        .entities()
        .soldiers()
        .filter(|m| w.entities().owner()[m.index()] == PlayerId(0))
        .take(3)
        .collect();
    let mut mixed = men.clone();
    mixed.push(v);
    w.step(&[cmd(
        0,
        2,
        CommandKind::AttackMoveUnits {
            units: mixed,
            target: xy(560, 520),
        },
    )]);
    assert_eq!(rejected(&w), r0);
    assert!(matches!(
        w.game().unwrap().squad(sq).unwrap().order,
        Order::AttackMove { .. }
    ));
    // Soldier-only orders with only the vehicle: rejected.
    let line = vec![xy(500, 520), xy(520, 520)];
    let only: Vec<Command> = vec![
        cmd(
            0,
            3,
            CommandKind::DigUnits {
                units: vec![v],
                line: line.clone(),
            },
        ),
        cmd(
            0,
            4,
            CommandKind::SetStance {
                units: vec![v],
                stance: StanceOrder::Prone,
            },
        ),
        cmd(
            0,
            5,
            CommandKind::Throw {
                units: vec![v],
                item: ItemKind::Frag,
                target: xy(510, 510),
            },
        ),
        cmd(0, 6, CommandKind::Dig { squad: sq, line }),
        cmd(
            0,
            7,
            CommandKind::Barrage {
                squad: sq,
                target: xy(700, 700),
                rounds: 3,
            },
        ),
        cmd(0, 8, CommandKind::Regroup { squad: sq }),
        // Not his vehicle.
        cmd(
            1,
            0,
            CommandKind::MoveUnits {
                units: vec![v],
                target: xy(400, 400),
            },
        ),
        // 121 ids.
        cmd(
            0,
            9,
            CommandKind::MoveUnits {
                units: vec![v; 121],
                target: xy(400, 400),
            },
        ),
    ];
    let n = only.len() as u64;
    w.step(&only);
    assert_eq!(rejected(&w), r0 + n);
    // Squad-level orders.
    w.step(&[cmd(0, 10, CommandKind::Retreat { squad: sq })]);
    assert_eq!(rejected(&w), r0 + n);
    assert_eq!(w.game().unwrap().squad(sq).unwrap().order, Order::Retreat);
}

#[test]
fn a_player_with_only_a_vehicle_left_is_not_wiped_out() {
    let mut s = flat_setup();
    s.rules.start_manpower = 0;
    s.rules.income_per_min = 0;
    s.rules.point_income_per_min = 0;
    for f in &mut s.factions {
        f.start_squads.clear();
    }
    // Nobody: player 0 is out at once; give him a tank first.
    let mut w = World::new_match(s.clone(), 1).unwrap();
    let (_, v0) = tank_at(&mut w, 0, xy(400, 400), EAST);
    let (_, _v1) = tank_at(&mut w, 1, xy(600, 600), EAST);
    for _ in 0..30 {
        w.step(&[]);
    }
    assert!(
        w.game()
            .unwrap()
            .players()
            .iter()
            .all(|p| p.defeat.is_none())
    );
    assert!(w.wreck_vehicle(v0));
    w.step(&[]);
    let g = w.game().unwrap();
    assert!(g.players()[0].defeat.is_some(), "a wreck does not count");
    assert!(g.players()[1].defeat.is_none());
}

#[test]
fn vehicles_are_spotted_at_twice_sight_and_spot_as_observers() {
    let mut w = World::new_match(flat_setup(), 1).unwrap();
    // A German rifleman squad at (200, 500) facing east; sight 150 m.
    let rifle = type_index(&w, "ww2:ger_rifle_squad");
    let sight = w.match_setup().unwrap().squad_types[rifle as usize].sight;
    assert_eq!(Fx::from(sight), Fx::from_int(150));
    w.spawn_squad_at(
        PlayerId(0),
        rifle,
        xy(200, 500),
        FxVec2::new(Fx::ZERO, Fx::ONE),
    )
    .unwrap();
    // US tanks at 290 m (inside 2 × 150) and 320 m.
    let (_, near) = tank_at(&mut w, 1, xy(490, 500), EAST);
    let (_, far) = tank_at(&mut w, 1, xy(520, 500), EAST);
    let seen = |w: &World, v: SimId| w.entities().combat()[v.index()].seen_by & 1 != 0;
    for _ in 0..60 {
        w.step(&[]);
    }
    assert!(seen(&w, near), "290 m: inside 200 % of 150 m");
    assert!(!seen(&w, far), "320 m: outside");
    // The bridge's filter reads the snapshot's seen_by.
    let snap = w
        .snapshot()
        .vehicles
        .iter()
        .find(|s| s.id == near.0)
        .unwrap();
    assert!(snap.seen_by & 1 != 0);

    // A tank observes from its hatch with its own sight (175 m).
    let mut w = World::new_match(flat_setup(), 1).unwrap();
    tank_at(&mut w, 0, xy(200, 500), EAST);
    let us = type_index(&w, "ww2:usa_rifle_squad");
    let a = w
        .spawn_squad_at(
            PlayerId(1),
            us,
            xy(365, 500),
            FxVec2::new(Fx::ZERO, Fx::ONE),
        )
        .unwrap();
    let b = w
        .spawn_squad_at(
            PlayerId(1),
            us,
            xy(390, 300),
            FxVec2::new(Fx::ZERO, Fx::ONE),
        )
        .unwrap();
    // Once seen, its MGs suppress the near men into cover: count the most
    // seen at once.
    let seen_men = |w: &World, q: SquadId| {
        w.game().unwrap().squad(q).map_or(0, |s| {
            s.members
                .iter()
                .filter(|m| w.entities().combat()[m.index()].seen_by & 1 != 0)
                .count()
        })
    };
    let (mut most_a, mut most_b) = (0, 0);
    for _ in 0..60 {
        w.step(&[]);
        most_a = most_a.max(seen_men(&w, a));
        most_b = most_b.max(seen_men(&w, b));
    }
    assert!(most_a > 0, "men 165 m away are seen by the tank");
    assert_eq!(most_b, 0, "men 275 m away are not");

    // Near spotting: within 25 m an enemy soldier hears and sees it even
    // with no sight at all.
    let mut s = flat_setup();
    for t in &mut s.squad_types {
        t.sight = frontline_content::Fixed::from_int(1);
    }
    let mut w = World::new_match(s, 1).unwrap();
    w.spawn_squad_at(
        PlayerId(0),
        rifle,
        xy(200, 500),
        FxVec2::new(Fx::ZERO, Fx::ONE),
    )
    .unwrap();
    let (_, close) = tank_at(&mut w, 1, xy(215, 500), EAST);
    let (_, away) = tank_at(&mut w, 1, xy(260, 500), EAST);
    for _ in 0..60 {
        w.step(&[]);
    }
    assert!(seen(&w, close));
    assert!(!seen(&w, away));
}

#[test]
fn head_on_tanks_never_overlap_and_get_past() {
    let mut w = World::new_match(flat_setup(), 1).unwrap();
    let west = FxVec2::new(Fx::NEG_ONE, Fx::ZERO);
    let (a, va) = tank_at(&mut w, 0, xy(400, 500), EAST);
    let (b, vb) = tank_at(&mut w, 0, xy(520, 500), west);
    w.step(&[
        cmd(
            0,
            0,
            CommandKind::Move {
                squad: a,
                target: xy(560, 500),
            },
        ),
        cmd(
            0,
            1,
            CommandKind::Move {
                squad: b,
                target: xy(360, 500),
            },
        ),
    ]);
    let geo = w.game().unwrap().vehicle_derived().geo[0];
    let mut closest = Fx::from_int(1000);
    let done = run_until(&mut w, 30 * 90, |w| {
        // Hull overlap test on the two circle pairs (radius half width).
        let f = |v: SimId| {
            let t = w.entities().transform()[v.index()];
            let d = frontline_sim::vehicle::yaw_dir(t.yaw) * geo.hull_half.x.div_int(2);
            [t.pos.xy() + d, t.pos.xy() - d]
        };
        for p in f(va) {
            for q in f(vb) {
                closest = closest.min(dist(p, q));
            }
        }
        let g = w.game().unwrap();
        g.squad(a).unwrap().order == Order::Idle && g.squad(b).unwrap().order == Order::Idle
    });
    assert!(
        closest >= geo.hull_half.y.mul_int(2),
        "hulls overlapped: circles {closest:?} apart"
    );
    assert!(done, "both got past: {:?} {:?}", pos(&w, va), pos(&w, vb));
}

#[test]
fn a_tank_drives_from_its_base_to_a_point_on_kessel_ridge() {
    let mut w = World::new_match(setup(1, true), 2).unwrap();
    let base = w.game().unwrap().base_of(PlayerId(0));
    let (sq, v) = tank_at(&mut w, 0, base, EAST);
    let points: Vec<FxVec2> = w
        .match_setup()
        .unwrap()
        .map
        .points
        .iter()
        .map(|p| FxVec2::from(p.pos))
        .collect();
    for (k, &target) in points.iter().enumerate() {
        w.step(&[cmd(0, k as u32, CommandKind::Move { squad: sq, target })]);
        let from = pos(&w, v);
        let t0 = w.tick();
        let arrived = run_until(&mut w, 30 * 240, |w| {
            w.game().unwrap().squad(sq).unwrap().order == Order::Idle
        });
        let d = dist(pos(&w, v), target);
        println!(
            "point {k}: {:?} m in {} s, {:?} m short",
            dist(from, target),
            (w.tick() - t0) / 30,
            d
        );
        assert!(
            arrived && d < Fx::from_int(6),
            "point {k}: stuck at {:?}",
            pos(&w, v)
        );
    }
}

/// Scripted vehicle orders next to two AI commanders (test unlock rules).
fn vehicle_match(seed: u64, threads: usize) -> World {
    let mut s = setup(seed, true);
    s.players.iter_mut().for_each(|p| p.is_ai = true);
    World::new_match(s, threads).unwrap()
}

fn script(w: &World, k: u64) -> Vec<Command> {
    let t = |p: u8| tank_type(w, p);
    match k {
        1 => vec![
            cmd(0, 1000, CommandKind::CallIn { squad_type: t(0) }),
            cmd(0, 1001, CommandKind::CallIn { squad_type: t(0) }),
            cmd(1, 1000, CommandKind::CallIn { squad_type: t(1) }),
            cmd(1, 1001, CommandKind::CallIn { squad_type: t(1) }),
        ],
        _ => {
            // Every 10 s once they are out: send each player's vehicles
            // toward the map centre, back, or stop them.
            if !k.is_multiple_of(300) {
                return Vec::new();
            }
            let g = w.game().unwrap();
            let mut out = Vec::new();
            for (&sid, sq) in g.squads() {
                if !g.is_vehicle_squad(sid) {
                    continue;
                }
                let p = sq.owner.0;
                let seq = 2000 + (k / 300) as u32 * 8 + sid.0 % 8;
                let target = xy(
                    380 + 40 * (sid.0 as i32 % 7),
                    420 + 30 * ((k / 300) as i32 % 5),
                );
                let kind = match (k / 300 + sid.0 as u64) % 4 {
                    0 => CommandKind::Move { squad: sid, target },
                    1 => CommandKind::MoveUnits {
                        units: sq.members.clone(),
                        target,
                    },
                    2 => CommandKind::AttackMove { squad: sid, target },
                    _ => CommandKind::StopUnits {
                        units: sq.members.clone(),
                    },
                };
                out.push(cmd(p, seq, kind));
            }
            out
        }
    }
}

fn run_script(w: &mut World, ticks: u64) -> Vec<u64> {
    let mut hashes = Vec::new();
    for _ in 0..ticks {
        let k = w.tick() + 1;
        let cmds = script(w, k);
        w.step(&cmds);
        if w.tick().is_multiple_of(300) {
            hashes.push(w.full_state_hash());
        }
    }
    hashes
}

#[test]
fn vehicle_matches_are_identical_across_thread_counts() {
    let ticks = 30 * 150;
    let mut base = vehicle_match(3, 1);
    let want = run_script(&mut base, ticks);
    let g = base.game().unwrap();
    let alive = g
        .vehicles()
        .keys()
        .filter(|v| base.entities().kind()[v.index()] == EntityKind::Vehicle)
        .count();
    assert_eq!(alive, 4, "four tanks spawned");
    assert!(
        g.vehicles().iter().any(|(v, vs)| vs.speed != Fx::ZERO
            || dist(
                pos(&base, *v),
                base.game()
                    .unwrap()
                    .base_of(base.entities().owner()[v.index()])
            ) > Fx::from_int(60)),
        "they drove"
    );
    for threads in [2, 3, 8] {
        let mut w = vehicle_match(3, threads);
        assert_eq!(run_script(&mut w, ticks), want, "{threads} threads");
    }
}

#[test]
fn vehicle_state_survives_save_load_continue() {
    // A tight path budget, so a request can wait in the queue past a tick.
    let mut s = setup(2, true);
    s.players.iter_mut().for_each(|p| p.is_ai = true);
    s.rules.movement.path_budget_per_tick = 64;
    s.rules.movement.path_max_expansions = 64;
    let mut w = World::new_match(s, 2).unwrap();
    // Out at 45 s; driving from 50 s on.
    run_script(&mut w, 30 * 70);
    // A moving vehicle and a pending vehicle path request right now.
    let g = w.game().unwrap();
    let (sid, sq) = g
        .squads()
        .iter()
        .find(|(id, _)| g.is_vehicle_squad(**id))
        .map(|(id, s)| (*id, s.clone()))
        .unwrap();
    let p = sq.owner.0;
    let mut pending = false;
    for k in 0..90 {
        w.step(&[cmd(
            p,
            9000 + k,
            CommandKind::Move {
                squad: sid,
                target: xy(300 + 4 * k as i32, 512),
            },
        )]);
        if w.game().unwrap().squad(sid).unwrap().nav.phase == NavPhase::PathPending {
            pending = true;
            break;
        }
    }
    assert!(pending, "a vehicle path request waits in the queue");
    let v = sq.members[0];
    // A bogged one too (staged).
    let other = *w
        .game()
        .unwrap()
        .vehicles()
        .keys()
        .find(|k| **k != v)
        .unwrap();
    w.edit_vehicle(other, |vs| vs.bog = 300);
    let saved = w.save();
    let mut a = w;
    let mut b = World::load(&saved, 3).unwrap();
    assert_eq!(a.state_hash(), b.state_hash());
    let ha = run_script(&mut a, 30 * 40);
    let hb = run_script(&mut b, 30 * 40);
    assert_eq!(ha, hb);
    assert_eq!(a.full_state_hash(), b.full_state_hash());
}

/// Stress measurement (not a gate; `cargo test --release -- --ignored
/// --nocapture vehicle_stress`): 100 tanks and about 600 soldiers on
/// Kessel Ridge, the tanks re-ordered across the map every 20 s, two AI
/// commanders for the soldiers. Prints tick and stage times.
#[test]
#[ignore]
// Wall-clock time here only measures; it never feeds the sim.
#[allow(clippy::disallowed_methods)]
fn vehicle_stress_report() {
    use std::time::Instant;
    let mut s = setup(4, true);
    s.players.iter_mut().for_each(|p| p.is_ai = true);
    s.rules.vehicles.limit = 50;
    s.rules.pop_cap = 2000;
    let mut w = World::new_match(s, 3).unwrap();
    let rifle = [
        type_index(&w, "ww2:ger_rifle_squad"),
        type_index(&w, "ww2:usa_rifle_squad"),
    ];
    let mut tanks: Vec<(u8, SquadId)> = Vec::new();
    for p in 0..2u8 {
        let x0 = if p == 0 { 250 } else { 650 };
        for k in 0..50 {
            let at = xy(x0 + 10 * (k % 10), 300 + 40 * (k / 10));
            tanks.push((p, tank_at(&mut w, p, at, EAST).0));
        }
        for k in 0..42 {
            let at = xy(x0 + 12 * (k % 7), 520 + 25 * (k / 7));
            w.spawn_squad_at(PlayerId(p), rifle[p as usize], at, EAST);
        }
    }
    println!(
        "soldiers {}, vehicles {}",
        w.entities().soldier_count(),
        w.game().unwrap().vehicles().len()
    );
    let mut ticks = Vec::new();
    let mut moves = Vec::new();
    let mut spot = Vec::new();
    let (mut weapons, mut ballistics, mut damage) = (Vec::new(), Vec::new(), Vec::new());
    let mut seq = 0u32;
    for t in 0..30 * 120u64 {
        let mut cmds = Vec::new();
        if t % 600 == 0 {
            for (k, &(p, sq)) in tanks.iter().enumerate() {
                seq += 1;
                let tx = 200 + ((k as i32 * 37 + t as i32 / 6) % 600);
                let ty = 200 + ((k as i32 * 53 + t as i32 / 9) % 600);
                cmds.push(cmd(
                    p,
                    seq,
                    CommandKind::Move {
                        squad: sq,
                        target: xy(tx, ty),
                    },
                ));
            }
        }
        let t0 = Instant::now();
        w.step(&cmds);
        ticks.push(t0.elapsed().as_secs_f64() * 1000.0);
        moves.push(w.stage_times().movement.as_secs_f64() * 1000.0);
        if *moves.last().unwrap() > 2.0 {
            println!("slow movement at tick {t}: {:.2} ms", moves.last().unwrap());
        }
        spot.push(w.stage_times().perception.as_secs_f64() * 1000.0);
        weapons.push(w.stage_times().weapons.as_secs_f64() * 1000.0);
        ballistics.push(w.stage_times().ballistics.as_secs_f64() * 1000.0);
        damage.push(w.stage_times().damage.as_secs_f64() * 1000.0);
    }
    let p99 = |v: &mut Vec<f64>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        (v[v.len() / 2], v[v.len() * 99 / 100], v[v.len() - 1])
    };
    let moving = w
        .game()
        .unwrap()
        .vehicles()
        .values()
        .filter(|v| v.speed != Fx::ZERO)
        .count();
    println!("tick ms p50/p99/max {:?}", p99(&mut ticks));
    println!("movement ms p50/p99/max {:?}", p99(&mut moves));
    println!("perception ms p50/p99/max {:?}", p99(&mut spot));
    println!("weapons ms p50/p99/max {:?}", p99(&mut weapons));
    println!("ballistics ms p50/p99/max {:?}", p99(&mut ballistics));
    println!("damage ms p50/p99/max {:?}", p99(&mut damage));
    let st = w.stats();
    println!(
        "box tests {} (peak {} per tick), plate hits: {} ricochet, {} non-pen, {} pen; knocked out {}; small-arms rounds stopped {}",
        st.armour_box_tests,
        st.armour_box_peak,
        st.ricochets,
        st.non_penetrations,
        st.penetrations,
        st.knockouts,
        st.rounds_stopped
    );
    println!("vehicles moving at the end: {moving}");
}
