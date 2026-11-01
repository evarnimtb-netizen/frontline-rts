//! S3 vehicle orders, escorts and AT ditches (sim stage 4, spec steps 5–6):
//! the new `*Units` commands and their validation with vehicle ids,
//! reversing, facing, the `AttackUnit` approach, ammo choice, field repair,
//! escorts, the AT-ditch dig profile (engineers, blocking, infantry
//! crossing, no cover in it), bogging and unbogging, and wrecks as
//! obstacles; determinism across threads and save → load → continue with
//! escorts, launchers and an AT ditch being dug.

use std::path::Path;

use frontline_content::{
    ComponentKind, Difficulty, DigProfile, MatchRequest, MatchSetup, ModDefs, SeatRole,
};
use frontline_sim::vehicle::{AmmoChoice, node_at, sample_ground};
use frontline_sim::{
    ArmourEventKind, Command, CommandKind, EntityKind, Fx, FxVec2, Order, PlayerId, SimId, SquadId,
    World,
};

/// Flat open ground with the test unlock, both sides human.
fn setup() -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let mut defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let rules = defs
        .add_rules_override(&root.join("tests/rules_test_unlock.toml"))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    req.rules = rules;
    let mut s = defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
    s.players.iter_mut().for_each(|p| p.is_ai = false);
    s.rules.start_manpower = 100_000;
    s.map.terrain.octaves.clear();
    s.map.terrain.bands.clear();
    s.map.woods.clear();
    // S3b: `disarm` keeps only a driver; the old crew rule (knocked out
    // only when empty) keeps such a target alive under fire.
    s.rules.vehicles.damage.crew_ko_below = 0;
    s
}

fn xy(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

const EAST: FxVec2 = FxVec2::new(Fx::ONE, Fx::ZERO);
const WEST: FxVec2 = FxVec2::new(Fx::NEG_ONE, Fx::ZERO);
const NORTH: FxVec2 = FxVec2::new(Fx::ZERO, Fx::ONE);

fn cmd(player: u8, seq: u32, kind: CommandKind) -> Command {
    Command {
        player: PlayerId(player),
        seq,
        kind,
    }
}

fn spawn(w: &mut World, player: u8, id: &str, at: FxVec2, f: FxVec2) -> SquadId {
    let t = w.match_setup().unwrap().squad_type_index(id).unwrap();
    w.spawn_squad_at(PlayerId(player), t, at, f).unwrap()
}

fn members(w: &World, sq: SquadId) -> Vec<SimId> {
    w.game().unwrap().squad(sq).unwrap().members.clone()
}

fn tank(w: &mut World, player: u8, at: FxVec2, f: FxVec2) -> (SquadId, SimId) {
    let id = if player == 0 {
        "ww2:ger_pz4_h_squad"
    } else {
        "ww2:usa_m4a3_75_squad"
    };
    let sq = spawn(w, player, id, at, f);
    (sq, members(w, sq)[0])
}

/// Only the driver left: drives, never fires.
fn disarm(w: &mut World, v: SimId) {
    let s = w.match_setup().unwrap().clone();
    let vt = &s.vehicles[w.game().unwrap().vehicle(v).unwrap().type_idx as usize];
    let roles: Vec<SeatRole> = vt.seats.iter().map(|s| s.role).collect();
    w.edit_vehicle(v, |vs| {
        for (seat, role) in vs.seats.iter_mut().zip(&roles) {
            if *role != SeatRole::Driver {
                seat.hp = 0;
            }
        }
    });
}

fn pos(w: &World, id: SimId) -> FxVec2 {
    w.entities().transform()[id.index()].pos.xy()
}

fn dist(a: FxVec2, b: FxVec2) -> Fx {
    (a - b).length()
}

fn rejected(w: &World) -> u64 {
    w.stats().commands_rejected
}

fn order(w: &World, sq: SquadId) -> Order {
    w.game().unwrap().squad(sq).unwrap().order.clone()
}

fn run_until(w: &mut World, max_ticks: u64, mut done: impl FnMut(&World) -> bool) -> bool {
    for _ in 0..max_ticks {
        if done(w) {
            return true;
        }
        w.step(&[]);
    }
    done(w)
}

#[test]
fn the_new_unit_commands_validate_their_ids() {
    let mut w = World::new_match(setup(), 1).unwrap();
    let (_, v) = tank(&mut w, 0, xy(500, 500), EAST);
    let (_, enemy) = tank(&mut w, 1, xy(700, 500), WEST);
    disarm(&mut w, enemy);
    disarm(&mut w, v);
    let rifles = spawn(&mut w, 0, "ww2:ger_rifle_squad", xy(480, 530), NORTH);
    let men = members(&w, rifles);
    // Let the enemy tank be seen.
    assert!(run_until(&mut w, 300, |w| {
        w.entities().combat()[enemy.index()].seen_by & 1 != 0
    }));
    let all = |units: Vec<SimId>| -> Vec<CommandKind> {
        vec![
            CommandKind::ReverseUnits {
                units: units.clone(),
                target: xy(450, 500),
            },
            CommandKind::FaceUnits {
                units: units.clone(),
                toward: xy(500, 600),
            },
            CommandKind::AttackUnit {
                units: units.clone(),
                target: enemy,
            },
            CommandKind::SetAmmo {
                units: units.clone(),
                ammo: AmmoChoice::He,
            },
            CommandKind::RepairUnits {
                units: units.clone(),
            },
            CommandKind::FollowUnits { units, vehicle: v },
        ]
    };
    // Empty, 121 ids, the enemy's vehicle, a vacant id: all rejected.
    let mut seq = 0u32;
    for units in [vec![], vec![v; 121], vec![enemy], vec![SimId(60_000)]] {
        for k in all(units) {
            let r0 = rejected(&w);
            w.step(&[cmd(0, seq, k.clone())]);
            seq += 1;
            assert_eq!(rejected(&w), r0 + 1, "accepted {k:?}");
        }
    }
    // Player 1 may not order player 0's vehicle.
    for k in all(vec![v]) {
        let r0 = rejected(&w);
        w.step(&[cmd(1, seq, k.clone())]);
        seq += 1;
        assert_eq!(rejected(&w), r0 + 1, "accepted {k:?}");
    }
    // Vehicle-only commands with only soldiers: rejected; FollowUnits
    // with only the vehicle: rejected (vehicles are dropped).
    for k in [
        CommandKind::FaceUnits {
            units: men.clone(),
            toward: xy(500, 600),
        },
        CommandKind::SetAmmo {
            units: men.clone(),
            ammo: AmmoChoice::Ap,
        },
        CommandKind::RepairUnits { units: men.clone() },
        CommandKind::AttackUnit {
            units: men.clone(),
            target: enemy,
        },
        CommandKind::FollowUnits {
            units: vec![v],
            vehicle: v,
        },
        // Following an enemy vehicle, or a soldier.
        CommandKind::FollowUnits {
            units: men.clone(),
            vehicle: enemy,
        },
        CommandKind::FollowUnits {
            units: men.clone(),
            vehicle: men[0],
        },
        // Attacking an own vehicle, or a soldier.
        CommandKind::AttackUnit {
            units: vec![v],
            target: v,
        },
        CommandKind::AttackUnit {
            units: vec![v],
            target: men[0],
        },
        // Facing a point off the map.
        CommandKind::FaceUnits {
            units: vec![v],
            toward: xy(-5, 500),
        },
        // Nothing to repair.
        CommandKind::RepairUnits { units: vec![v] },
    ] {
        let r0 = rejected(&w);
        w.step(&[cmd(0, seq, k.clone())]);
        seq += 1;
        assert_eq!(rejected(&w), r0 + 1, "accepted {k:?}");
    }
    // Own vehicle ids: accepted (RepairUnits once there is damage).
    w.edit_vehicle(v, |vs| vs.components[ComponentKind::TrackLeft.index()] = 0);
    for k in all(vec![v]) {
        if matches!(k, CommandKind::FollowUnits { .. }) {
            continue;
        }
        let r0 = rejected(&w);
        w.step(&[cmd(0, seq, k.clone())]);
        seq += 1;
        assert_eq!(rejected(&w), r0, "rejected {k:?}");
    }
    let vs = w.game().unwrap().vehicle(v).unwrap();
    assert_eq!(vs.ammo_choice, AmmoChoice::He);
    assert_eq!(vs.attack, Some(enemy));
    assert_eq!(vs.repairing, Some(ComponentKind::TrackLeft.index() as u8));
    // Duplicates are fine; soldiers follow.
    let mut dup = men.clone();
    dup.extend(&men);
    dup.push(v);
    let r0 = rejected(&w);
    w.step(&[cmd(
        0,
        seq,
        CommandKind::FollowUnits {
            units: dup,
            vehicle: v,
        },
    )]);
    assert_eq!(rejected(&w), r0);
    assert_eq!(order(&w, rifles), Order::Follow { vehicle: v });
    // A move order ends the repair and the attack.
    w.step(&[cmd(
        0,
        seq + 1,
        CommandKind::MoveUnits {
            units: vec![v],
            target: xy(520, 500),
        },
    )]);
    let vs = w.game().unwrap().vehicle(v).unwrap();
    assert_eq!(vs.repairing, None);
    assert_eq!(vs.attack, None);
}

#[test]
fn soldier_only_and_engineer_only_orders_reject_the_wrong_squads() {
    let mut w = World::new_match(setup(), 1).unwrap();
    let (tsq, v) = tank(&mut w, 0, xy(500, 500), EAST);
    let rifles = spawn(&mut w, 0, "ww2:ger_rifle_squad", xy(400, 400), NORTH);
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", xy(400, 300), NORTH);
    let line = vec![xy(380, 420), xy(400, 420)];
    let long = vec![xy(380, 320), xy(420, 320)];
    let mut seq = 0;
    let mut expect = |w: &mut World, k: CommandKind, ok: bool| {
        let r0 = rejected(w);
        w.step(&[cmd(0, seq, k.clone())]);
        seq += 1;
        assert_eq!(rejected(w) == r0, ok, "{k:?}");
    };
    // Vehicle squads: no dig, barrage, regroup or stance.
    for k in [
        CommandKind::Dig {
            squad: tsq,
            line: line.clone(),
            profile: DigProfile::AtDitch,
        },
        CommandKind::Barrage {
            squad: tsq,
            target: xy(800, 800),
            rounds: 2,
        },
        CommandKind::Regroup { squad: tsq },
        CommandKind::SetStance {
            units: vec![v],
            stance: frontline_sim::StanceOrder::Crouch,
        },
        CommandKind::DigUnits {
            units: vec![v],
            line: line.clone(),
            profile: DigProfile::Trench,
        },
    ] {
        expect(&mut w, k, false);
    }
    // The AT ditch is the engineers' (and at most 30 m).
    expect(
        &mut w,
        CommandKind::Dig {
            squad: rifles,
            line: line.clone(),
            profile: DigProfile::AtDitch,
        },
        false,
    );
    let rifle_men = members(&w, rifles);
    expect(
        &mut w,
        CommandKind::DigUnits {
            units: rifle_men,
            line: line.clone(),
            profile: DigProfile::AtDitch,
        },
        false,
    );
    expect(
        &mut w,
        CommandKind::Dig {
            squad: eng,
            line: long.clone(),
            profile: DigProfile::AtDitch,
        },
        false,
    );
    // 40 m is fine for a trench.
    expect(
        &mut w,
        CommandKind::Dig {
            squad: rifles,
            line: long,
            profile: DigProfile::Trench,
        },
        true,
    );
    assert!(w.game().unwrap().at_ditches().is_empty());
    expect(
        &mut w,
        CommandKind::Dig {
            squad: eng,
            line: line.clone(),
            profile: DigProfile::AtDitch,
        },
        true,
    );
    // Sections are recorded as they are dug (first pass), not on the order.
    assert!(w.game().unwrap().at_ditches().is_empty());
    let two = members(&w, eng)[..2].to_vec();
    expect(
        &mut w,
        CommandKind::DigUnits {
            units: two,
            line: line.clone(),
            profile: DigProfile::AtDitch,
        },
        true,
    );
    assert!(w.game().unwrap().at_ditches().is_empty());
}

#[test]
fn engineers_dig_an_at_ditch_that_blocks_tanks_but_not_men() {
    let mut w = World::new_match(setup(), 1).unwrap();
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", xy(500, 490), NORTH);
    let line = vec![xy(495, 500), xy(505, 500)];
    w.step(&[cmd(
        0,
        0,
        CommandKind::Dig {
            squad: eng,
            line: line.clone(),
            profile: DigProfile::AtDitch,
        },
    )]);
    let t0 = w.tick();
    let done = run_until(&mut w, 30 * 600, |w| order(w, eng) == Order::Idle);
    let secs = (w.tick() - t0) / 30;
    assert!(done, "10 m of AT ditch unfinished after 10 min");
    // §5.10.9 pace: 5 men × 200 % × 100 work/s against 220,000: about 3.7
    // min of digging plus the walk.
    assert!((180..=330).contains(&secs), "dug in {secs} s");
    let mid = xy(500, 500);
    let depth = w.terrain().base_height_at(mid) - w.terrain().height_at(mid);
    let want: Fx = w.match_setup().unwrap().rules.at_ditch.depth.into();
    assert!((depth - want).abs() < Fx::from_milli(50), "{depth:?}");
    let block = &w.game().unwrap().vehicle_derived().block;
    assert!(
        block.blocked_at(w.terrain(), mid),
        "2.3 m deep: blocks tanks"
    );
    assert!(block.is_ditch(node_at(w.terrain(), mid)));

    // A rifle squad ordered across arrives.
    let rifles = spawn(&mut w, 0, "ww2:ger_rifle_squad", xy(500, 480), EAST);
    w.step(&[cmd(
        0,
        1,
        CommandKind::Move {
            squad: rifles,
            target: xy(500, 520),
        },
    )]);
    let lead = members(&w, rifles)[0];
    assert!(
        run_until(&mut w, 30 * 90, |w| order(w, rifles) == Order::Idle),
        "never arrived"
    );
    assert!(dist(pos(&w, lead), xy(500, 520)) < Fx::from_int(8));
}

#[test]
fn men_take_cover_in_a_trench_but_never_in_an_at_ditch() {
    for at_ditch in [false, true] {
        let mut w = World::new_match(setup(), 1).unwrap();
        let line = [xy(480, 500), xy(510, 500)];
        if at_ditch {
            assert!(w.dig_at_ditch_now(&line, 1));
        } else {
            assert!(w.dig_trench_now(&line, 1));
        }
        // From the north, so nobody crosses it: the men stand 7 m from the
        // line; cover seeking pulls them into a trench, never into a ditch.
        let rifles = spawn(&mut w, 0, "ww2:ger_rifle_squad", xy(495, 540), WEST);
        w.step(&[cmd(
            0,
            0,
            CommandKind::Move {
                squad: rifles,
                target: xy(495, 507),
            },
        )]);
        assert!(run_until(&mut w, 30 * 60, |w| order(w, rifles) == Order::Idle));
        for _ in 0..90 {
            w.step(&[]);
        }
        let in_dug = members(&w, rifles)
            .iter()
            .filter(|m| w.entities().combat()[m.index()].in_trench)
            .count();
        if at_ditch {
            assert_eq!(in_dug, 0, "a man took cover in the AT ditch");
        } else {
            assert!(in_dug >= 3, "only {in_dug} in the trench");
        }
    }
}

#[test]
fn reverse_backs_up_with_the_front_kept() {
    let mut w = World::new_match(setup(), 1).unwrap();
    let (sq, v) = tank(&mut w, 0, xy(500, 500), EAST);
    let yaw0 = w.entities().transform()[v.index()].yaw;
    w.step(&[cmd(
        0,
        0,
        CommandKind::ReverseUnits {
            units: vec![v],
            target: xy(420, 500),
        },
    )]);
    let mut backwards = false;
    assert!(run_until(&mut w, 30 * 90, |w| {
        backwards |= w.game().unwrap().vehicle(v).unwrap().speed < Fx::ZERO;
        order(w, sq) == Order::Idle
    }));
    assert!(backwards);
    assert!(dist(pos(&w, v), xy(420, 500)) < Fx::from_int(4));
    let yaw = w.entities().transform()[v.index()].yaw;
    assert!((yaw - yaw0).wrap_angle().abs() < frontline_sim::armour::deg(10));
}

#[test]
fn face_turns_the_hull_to_the_point_and_holds() {
    let mut w = World::new_match(setup(), 1).unwrap();
    let (sq, v) = tank(&mut w, 0, xy(500, 500), EAST);
    w.step(&[cmd(
        0,
        0,
        CommandKind::FaceUnits {
            units: vec![v],
            toward: xy(500, 700),
        },
    )]);
    assert!(run_until(&mut w, 30 * 10, |w| {
        w.game().unwrap().vehicle(v).unwrap().face.is_none()
    }));
    let yaw = w.entities().transform()[v.index()].yaw;
    let err = (yaw - Fx::FRAC_PI_2).wrap_angle().abs();
    assert!(err <= frontline_sim::armour::deg(3), "{err:?}");
    assert!(dist(pos(&w, v), xy(500, 500)) < Fx::ONE, "turned in place");
    assert_eq!(order(&w, sq), Order::Idle);
}

#[test]
fn attack_unit_closes_in_until_the_gun_has_the_target() {
    // The KwK's reach is cut to 120 m so a seen tank can be out of range.
    let mut s = setup();
    for wp in s.weapons.iter_mut().filter(|w| w.id == "ww2:kwk40_l48") {
        wp.range_max = frontline_content::Fixed::from_int(120);
        wp.engage_still = wp.range_max;
        wp.engage_moving = wp.range_max;
    }
    let mut w = World::new_match(s, 1).unwrap();
    let (sq, v) = tank(&mut w, 0, xy(300, 500), EAST);
    let (_, enemy) = tank(&mut w, 1, xy(560, 500), WEST);
    disarm(&mut w, enemy);
    assert!(run_until(&mut w, 300, |w| {
        w.entities().combat()[enemy.index()].seen_by & 1 != 0
    }));
    w.step(&[cmd(
        0,
        0,
        CommandKind::AttackUnit {
            units: vec![v],
            target: enemy,
        },
    )]);
    w.step(&[]);
    assert!(
        matches!(order(&w, sq), Order::Move { .. }),
        "{:?}",
        order(&w, sq)
    );
    let fired = w.stats().shots_fired;
    let got = run_until(&mut w, 30 * 120, |w| {
        w.game().unwrap().vehicle(v).unwrap().target == Some(enemy) && order(w, sq) == Order::Idle
    });
    assert!(got, "never closed in");
    let d = dist(pos(&w, v), pos(&w, enemy));
    assert!(d <= Fx::from_int(121), "{d:?}");
    assert!(d >= Fx::from_int(60), "drove too close: {d:?}");
    // It fires at it.
    // It fires at it (S3b: the first round may already have knocked the
    // disarmed tank out, in the tick it came into range).
    for _ in 0..30 * 10 {
        w.step(&[]);
    }
    assert!(w.stats().shots_fired > fired);
}

#[test]
fn set_ammo_he_loads_he_even_at_a_tank() {
    let mut w = World::new_match(setup(), 1).unwrap();
    let (_, v) = tank(&mut w, 0, xy(400, 500), EAST);
    let (_, enemy) = tank(&mut w, 1, xy(600, 500), WEST);
    disarm(&mut w, enemy);
    w.step(&[cmd(
        0,
        0,
        CommandKind::SetAmmo {
            units: vec![v],
            ammo: AmmoChoice::He,
        },
    )]);
    let he = frontline_sim::gunnery::HE;
    assert!(run_until(&mut w, 30 * 30, |w| {
        w.game().unwrap().vehicle(v).unwrap().loaded == Some(he)
    }));
}

/// S3b: the Panzer IV's PzGr 40 APCR (the special third round). `Apcr`
/// loads it; `Auto` loads it only against a target the PzGr 39 likely
/// fails on, where the APCR does better; a Sherman (no special round)
/// loads its AP for `Apcr`.
#[test]
fn apcr_goes_only_where_ap_would_likely_fail() {
    use frontline_sim::gunnery::{AP, SPECIAL};
    let loads = |thick_mm: i32, choice: Option<AmmoChoice>| {
        let mut s = setup();
        let sh = s
            .vehicles
            .iter()
            .position(|v| v.id == "ww2:usa_m4a3_75")
            .unwrap();
        s.vehicles[sh].armour[frontline_content::ArmourZone::HullFront.index()].thickness =
            frontline_content::Fixed::from_int(thick_mm);
        let mut w = World::new_match(s, 1).unwrap();
        let (_, v) = tank(&mut w, 0, xy(350, 500), EAST);
        let (_, enemy) = tank(&mut w, 1, xy(650, 500), WEST);
        disarm(&mut w, enemy);
        if let Some(c) = choice {
            w.step(&[cmd(
                0,
                0,
                CommandKind::SetAmmo {
                    units: vec![v],
                    ammo: c,
                },
            )]);
        }
        let mut fired = Vec::new();
        for _ in 0..30 * 20 {
            w.step(&[]);
            for e in &w.snapshot().armour_events {
                if e.kind == ArmourEventKind::Fired as u8 && e.vehicle == v.0 {
                    fired.push(e.component);
                }
            }
            if fired.len() >= 2 {
                break;
            }
        }
        fired
    };
    // The real Sherman (63.5 mm at 47°): the PzGr 39 goes through; Auto
    // keeps AP (event kind 1).
    let f = loads(64, None);
    assert!(!f.is_empty() && f.iter().all(|&k| k == 1), "{f:?}");
    // An 82 mm glacis (about 120 mm effective): the PzGr 39 at 300 m has a
    // poor chance, the PzGr 40 a better one: Auto swaps to it (kind 3).
    let f = loads(82, None);
    assert!(f.contains(&3), "{f:?}");
    // So thick that neither gets through: Auto stays with AP.
    let f = loads(150, None);
    assert!(!f.is_empty() && f.iter().all(|&k| k == 1), "{f:?}");
    // Ordered: APCR at the real Sherman too.
    let f = loads(64, Some(AmmoChoice::Apcr));
    assert!(f.contains(&3), "{f:?}");
    let _ = (AP, SPECIAL);
}

/// S3b: `Apcr` on a gun without a special round loads its standard one.
#[test]
fn apcr_on_a_gun_without_it_loads_ap() {
    let mut w = World::new_match(setup(), 1).unwrap();
    let (_, v) = tank(&mut w, 1, xy(600, 500), WEST);
    w.step(&[cmd(
        1,
        0,
        CommandKind::SetAmmo {
            units: vec![v],
            ammo: AmmoChoice::Apcr,
        },
    )]);
    assert!(run_until(&mut w, 30 * 10, |w| {
        w.game().unwrap().vehicle(v).unwrap().loaded == Some(frontline_sim::gunnery::AP)
    }));
    assert_eq!(w.game().unwrap().vehicle(v).unwrap().ammo.len(), 2);
}

#[test]
fn repair_units_fixes_a_broken_track_in_place() {
    let mut w = World::new_match(setup(), 1).unwrap();
    let (_, v) = tank(&mut w, 0, xy(500, 500), EAST);
    w.edit_vehicle(v, |vs| vs.components[ComponentKind::TrackRight.index()] = 0);
    w.step(&[cmd(0, 0, CommandKind::RepairUnits { units: vec![v] })]);
    assert!(w.game().unwrap().vehicle(v).unwrap().repairing.is_some());
    assert!(run_until(&mut w, 30 * 40, |w| {
        w.game()
            .unwrap()
            .vehicle(v)
            .unwrap()
            .component(ComponentKind::TrackRight)
            == 100
    }));
    assert!(dist(pos(&w, v), xy(500, 500)) < Fx::ONE);
}

#[test]
fn escorts_walk_in_file_behind_the_tank_and_cap_its_speed() {
    let mut w = World::new_match(setup(), 1).unwrap();
    let (tsq, v) = tank(&mut w, 0, xy(300, 500), EAST);
    let rifles = spawn(&mut w, 0, "ww2:ger_rifle_squad", xy(280, 530), NORTH);
    let men = members(&w, rifles);
    w.step(&[cmd(
        0,
        0,
        CommandKind::FollowUnits {
            units: men.clone(),
            vehicle: v,
        },
    )]);
    assert_eq!(order(&w, rifles), Order::Follow { vehicle: v });
    w.step(&[cmd(
        0,
        1,
        CommandKind::Move {
            squad: tsq,
            target: xy(450, 500),
        },
    )]);
    let walk: Fx = w.match_setup().unwrap().squad_types
        [w.game().unwrap().squad(rifles).unwrap().type_idx as usize]
        .speed
        .into();
    let mut top = Fx::ZERO;
    assert!(run_until(&mut w, 30 * 120, |w| {
        top = top.max(w.game().unwrap().vehicle(v).unwrap().speed);
        order(w, tsq) == Order::Idle
    }));
    assert!(top <= walk + Fx::from_milli(100), "{top:?} > {walk:?}");
    assert_eq!(
        w.game().unwrap().vehicle(v).unwrap().followers.len(),
        men.len()
    );
    for _ in 0..30 * 10 {
        w.step(&[]);
    }
    // All behind the tank (west of it), in a file within 20 m.
    let tp = pos(&w, v);
    for &m in &men {
        let p = pos(&w, m);
        assert!(p.x < tp.x - Fx::from_int(3), "{p:?} not behind {tp:?}");
        assert!(dist(p, tp) < Fx::from_int(20), "{p:?} far from {tp:?}");
    }
    // The tank is knocked out: they stop.
    w.wreck_vehicle(v);
    w.step(&[]);
    assert_eq!(order(&w, rifles), Order::Idle);
}

#[test]
fn a_bogged_tank_waits_out_the_unbog_time_then_drives_on() {
    let mut w = World::new_match(setup(), 1).unwrap();
    let (sq, v) = tank(&mut w, 0, xy(500, 500), EAST);
    w.edit_vehicle(v, |vs| vs.bog = 90);
    w.step(&[cmd(
        0,
        0,
        CommandKind::Move {
            squad: sq,
            target: xy(560, 500),
        },
    )]);
    let flags = |w: &World| {
        w.snapshot()
            .vehicles
            .iter()
            .find(|x| x.id == v.0)
            .unwrap()
            .flags
    };
    for _ in 0..85 {
        w.step(&[]);
        assert!(dist(pos(&w, v), xy(500, 500)) < Fx::from_milli(10));
        assert!(flags(&w) & frontline_sim::vehicle_flags::BOGGED != 0);
    }
    assert!(run_until(&mut w, 30 * 30, |w| order(w, sq) == Order::Idle));
    assert!(flags(&w) & frontline_sim::vehicle_flags::BOGGED == 0);
}

#[test]
fn tanks_drive_round_a_wreck_and_soldiers_round_it() {
    let mut w = World::new_match(setup(), 1).unwrap();
    let (_, wreck) = tank(&mut w, 1, xy(520, 500), NORTH);
    w.wreck_vehicle(wreck);
    let (sq, v) = tank(&mut w, 0, xy(480, 500), EAST);
    w.step(&[cmd(
        0,
        0,
        CommandKind::Move {
            squad: sq,
            target: xy(570, 500),
        },
    )]);
    let geo = w.game().unwrap().vehicle_derived().geo[0];
    let arrived = run_until(&mut w, 30 * 90, |w| {
        let t = w.entities().transform()[v.index()];
        let g = sample_ground(w.terrain(), t.pos.xy(), t.yaw, &geo);
        let b = &w.game().unwrap().vehicle_derived().block;
        assert!(
            g.xy.iter()
                .flatten()
                .all(|p| !b.blocked_at(w.terrain(), *p))
        );
        order(w, sq) == Order::Idle
    });
    assert!(arrived);
    assert_eq!(w.entities().kind()[wreck.index()], EntityKind::Wreck);
    // Riflemen walking through it go round it: nobody inside the hull.
    let rifles = spawn(&mut w, 0, "ww2:ger_rifle_squad", xy(490, 500), NORTH);
    w.step(&[cmd(
        0,
        1,
        CommandKind::Move {
            squad: rifles,
            target: xy(550, 500),
        },
    )]);
    let wp = pos(&w, wreck);
    let men = members(&w, rifles);
    assert!(run_until(&mut w, 30 * 60, |w| {
        for m in &men {
            let rel = pos(w, *m) - wp;
            // The Sherman wreck faces north: hull 2.9 m long, 1.3 m wide
            // (half sizes, rounded down).
            assert!(
                rel.x.abs() > Fx::from_milli(1_300) || rel.y.abs() > Fx::from_milli(2_900),
                "a man inside the wreck at {rel:?}"
            );
        }
        order(w, rifles) == Order::Idle
    }));
}

/// A scripted match with AT teams, escorts, a vehicle attack order and an
/// AT ditch being dug.
fn order_match(threads: usize) -> World {
    let mut w = World::new_match(setup(), threads).unwrap();
    let (_, a) = tank(&mut w, 0, xy(350, 500), EAST);
    disarm(&mut w, a);
    let (bsq, b) = tank(&mut w, 1, xy(650, 520), WEST);
    disarm(&mut w, b);
    let rifles = spawn(&mut w, 0, "ww2:ger_rifle_squad", xy(330, 530), NORTH);
    // S3b: 20 m further east than in S3. Near misses now count to the
    // whole body, so the escort's fire suppresses the gunner more and he
    // fell at 66 m, just outside the moving-target reach (60 m).
    let team = spawn(&mut w, 1, "ww2:usa_bazooka_team", xy(470, 470), WEST);
    let eng = spawn(&mut w, 1, "ww2:usa_engineer_squad", xy(700, 400), NORTH);
    let men = members(&w, rifles);
    let team_men = members(&w, team);
    w.step(&[
        cmd(
            0,
            0,
            CommandKind::FollowUnits {
                units: men,
                vehicle: a,
            },
        ),
        cmd(
            1,
            0,
            CommandKind::Dig {
                squad: eng,
                line: vec![xy(690, 420), xy(710, 420)],
                profile: DigProfile::AtDitch,
            },
        ),
        cmd(
            0,
            1,
            CommandKind::AttackMoveUnits {
                units: vec![a],
                target: xy(600, 500),
            },
        ),
        cmd(
            1,
            1,
            CommandKind::FaceUnits {
                units: vec![b],
                toward: xy(350, 500),
            },
        ),
    ]);
    for _ in 0..90 {
        w.step(&[]);
    }
    w.step(&[cmd(
        1,
        2,
        CommandKind::AttackUnit {
            units: team_men,
            target: a,
        },
    )]);
    let _ = bsq;
    w
}

#[test]
fn orders_escorts_launchers_and_ditches_are_deterministic_and_survive_save_load() {
    let mut runs: Vec<Vec<u64>> = Vec::new();
    for threads in [1, 2, 3, 8] {
        let mut w = order_match(threads);
        let mut hashes = Vec::new();
        for _ in 0..30 * 40 {
            w.step(&[]);
            hashes.push(w.state_hash());
        }
        runs.push(hashes);
    }
    for r in &runs[1..] {
        assert_eq!(r, &runs[0], "thread counts disagree");
    }
    // Save at 20 s, load, continue for 60 s (past the first AT-ditch
    // sections being recorded): the same hashes.
    let mut w = order_match(2);
    for _ in 0..30 * 20 {
        w.step(&[]);
    }
    assert!(w.game().unwrap().at_ditches().is_empty());
    let mut loaded = World::load(&w.save(), 3).unwrap();
    for _ in 0..30 * 60 {
        w.step(&[]);
        loaded.step(&[]);
        assert_eq!(w.state_hash(), loaded.state_hash());
        assert_eq!(w.full_state_hash(), w.state_hash());
    }
    assert!(w.stats().shots_fired > 0);
    assert!(w.stats().rockets_fired > 0, "the bazooka team fired");
    let ditches = w.game().unwrap().at_ditches();
    assert!(!ditches.is_empty(), "no AT-ditch section dug in 80 s");
    assert!(ditches.iter().flatten().all(|q| q.y == Fx::from_int(420)));
}
