//! S3 infantry against tanks (sim stage 4, spec step 5): AT teams (the
//! launcher and the sidearm, engagement distances, backblast reveal,
//! hand-over, the fire step, resupply), blasts on tracks and ground
//! satchels.

use std::path::Path;

use frontline_content::{ComponentKind, Difficulty, MatchRequest, MatchSetup, ModDefs, SeatRole};
use frontline_sim::{
    ArmourEventKind, Command, CommandKind, Fx, FxVec2, ItemKind, PlayerId, SimId, SquadId, World,
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
    s
}

fn xy(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

const EAST: FxVec2 = FxVec2::new(Fx::ONE, Fx::ZERO);
const WEST: FxVec2 = FxVec2::new(Fx::NEG_ONE, Fx::ZERO);
/// Facing north: a squad stands on an east-west line.
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

/// A Sherman at `at` facing `f` whose crew is only its driver: it can
/// drive but neither its gun nor its MGs fire (nobody swaps into the
/// gunner's seat).
fn harmless_sherman(w: &mut World, at: FxVec2, f: FxVec2) -> (SquadId, SimId) {
    let sq = spawn(w, 1, "ww2:usa_m4a3_75_squad", at, f);
    let v = members(w, sq)[0];
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
    (sq, v)
}

fn launcher(w: &World, sq: SquadId) -> frontline_sim::Launcher {
    w.game()
        .unwrap()
        .squad(sq)
        .unwrap()
        .launcher
        .clone()
        .unwrap()
}

fn panzerschreck(w: &mut World, at: FxVec2, f: FxVec2) -> SquadId {
    spawn(w, 0, "ww2:ger_panzerschreck_team", at, f)
}

fn rpzb(w: &World) -> &frontline_content::setup::WeaponSetup {
    let s = w.match_setup().unwrap();
    s.weapons.iter().find(|x| x.id == "ww2:rpzb54").unwrap()
}

#[test]
fn an_at_team_carries_one_launcher_and_a_rifle() {
    let mut w = World::new_match(setup(), 1).unwrap();
    let sq = panzerschreck(&mut w, xy(300, 500), EAST);
    let men = members(&w, sq);
    assert_eq!(men.len(), 2);
    let l = launcher(&w, sq);
    assert_eq!(l.carrier, Some(men[0]));
    assert_eq!(l.rounds as u32, rpzb(&w).rounds_carried);
    assert_eq!(l.rounds, 6);
    // The rifleman's magazine is his rifle's.
    let s = w.match_setup().unwrap();
    let k98 = s.weapons.iter().find(|x| x.id == "ww2:kar98k").unwrap();
    assert_eq!(
        w.entities().combat()[men[1].index()].mag as u32,
        k98.magazine
    );
    // Rifle and MG squads have no launcher.
    let r = spawn(&mut w, 0, "ww2:ger_rifle_squad", xy(300, 520), EAST);
    assert!(w.game().unwrap().squad(r).unwrap().launcher.is_none());
}

#[test]
fn the_launcher_hits_a_still_tank_in_range_and_reloads() {
    let mut w = World::new_match(setup(), 2).unwrap();
    let team = panzerschreck(&mut w, xy(300, 500), EAST);
    let (_, tank) = harmless_sherman(&mut w, xy(400, 500), WEST);
    let carrier = members(&w, team)[0];
    let reload = rpzb(&w).reload_ticks as u64;
    let mut fired_at: Vec<u64> = Vec::new();
    for _ in 0..900 {
        let before = w.stats().rockets_fired;
        w.step(&[]);
        if w.stats().rockets_fired > before {
            fired_at.push(w.tick());
            // Up on his feet (the fire height) when the rocket leaves.
            let c = &w.entities().combat()[carrier.index()];
            assert_eq!(c.target, Some(tank));
        }
        if w.entities().kind()[tank.index()] != frontline_sim::EntityKind::Vehicle {
            break;
        }
    }
    assert!(!fired_at.is_empty(), "no rocket in 30 s");
    assert!(
        w.stats().penetrations >= 1,
        "a 160 mm HEAT rocket at 100 m goes through a Sherman anywhere: {:?}",
        w.stats()
    );
    for pair in fired_at.windows(2) {
        assert!(pair[1] - pair[0] >= reload, "{fired_at:?}");
    }
    let l = launcher(&w, team);
    assert_eq!(l.rounds as usize, 6 - fired_at.len());
    // The rifleman never fires at the tank.
    let rifleman = members(&w, team)[1];
    assert_eq!(w.entities().weapon()[rifleman.index()].shots, 0);
}

#[test]
fn launchers_respect_the_engagement_distances_and_ignore_infantry() {
    // A tank driving past at 90 m: inside the still distance (120 m) but
    // beyond the moving one (70 m). Then it stops: the team opens fire.
    let mut w = World::new_match(setup(), 1).unwrap();
    let team = panzerschreck(&mut w, xy(300, 400), EAST);
    let (tsq, tank) = harmless_sherman(&mut w, xy(390, 300), FxVec2::new(Fx::ZERO, Fx::ONE));
    let carrier = members(&w, team)[0];
    let mut seq = 0;
    w.step(&[cmd(
        1,
        seq,
        CommandKind::Move {
            squad: tsq,
            target: xy(390, 410),
        },
    )]);
    seq += 1;
    let moving: Fx = w.match_setup().unwrap().rules.vehicles.moving_speed.into();
    let mut fired_moving = 0;
    let mut rockets_far = 0;
    for _ in 0..1500 {
        let before = w.stats().rockets_fired;
        w.step(&[]);
        if w.stats().rockets_fired > before {
            let vs = w.game().unwrap().vehicle(tank).unwrap();
            let d = (w.entities().transform()[tank.index()].pos.xy()
                - w.entities().transform()[carrier.index()].pos.xy())
            .length();
            if vs.avg_vel.length() > moving {
                fired_moving += 1;
                assert!(d <= Fx::from_int(71), "a rocket at a moving tank at {d:?}");
            }
            if d > Fx::from_int(121) {
                rockets_far += 1;
            }
        }
        if w.game().unwrap().squad(tsq).unwrap().order == frontline_sim::Order::Idle {
            break;
        }
    }
    assert_eq!(fired_moving, 0, "never within 70 m while moving");
    assert_eq!(rockets_far, 0);
    // Parked at about 90 m: within the still distance.
    let stopped = w.stats().rockets_fired;
    for _ in 0..600 {
        w.step(&[]);
    }
    assert!(
        w.stats().rockets_fired > stopped,
        "no rocket at a still tank"
    );

    // Infantry only: the launcher stays silent, the rifle fires.
    let mut w = World::new_match(setup(), 1).unwrap();
    let team = panzerschreck(&mut w, xy(300, 400), EAST);
    let men = members(&w, team);
    spawn(&mut w, 1, "ww2:usa_rifle_squad", xy(380, 400), WEST);
    let _ = seq;
    for _ in 0..300 {
        w.step(&[]);
    }
    assert_eq!(w.stats().rockets_fired, 0);
    assert_eq!(w.entities().weapon()[men[0].index()].shots, 0);
    assert!(w.entities().weapon()[men[1].index()].shots > 0);
}

#[test]
fn the_next_man_takes_the_launcher_after_the_hand_over_and_reloads_alone() {
    let mut w = World::new_match(setup(), 1).unwrap();
    let team = panzerschreck(&mut w, xy(300, 500), EAST);
    let men = members(&w, team);
    let handover = rpzb(&w).handover_ticks as u64;
    assert_eq!(handover, 90, "3 s at 30 Hz");
    w.kill_soldier(men[0]);
    let l = launcher(&w, team);
    assert_eq!((l.carrier, l.handover as u64), (None, handover));
    w.step(&[]);
    // A save in the middle of the hand-over loads and continues alike.
    let mut loaded = World::load(&w.save(), 2).unwrap();
    assert_eq!(loaded.state_hash(), w.state_hash());
    let t0 = w.tick();
    while launcher(&w, team).carrier.is_none() {
        w.step(&[]);
        assert!(w.tick() - t0 <= handover + 1);
    }
    assert_eq!(launcher(&w, team).carrier, Some(men[1]));
    // 3 s after he fell (one tick passed before `t0`).
    assert_eq!(w.tick() - t0, handover - 1);
    while loaded.tick() < w.tick() {
        loaded.step(&[]);
    }
    assert_eq!(loaded.state_hash(), w.state_hash());
    // Alone, he reloads in twice the time.
    let (_, _tank) = harmless_sherman(&mut w, xy(400, 500), WEST);
    let reload = rpzb(&w).reload_ticks as u16;
    let mut shots = Vec::new();
    for _ in 0..1500 {
        let before = w.stats().rockets_fired;
        w.step(&[]);
        if w.stats().rockets_fired > before {
            shots.push(w.tick());
            assert_eq!(w.entities().weapon()[men[1].index()].cooldown, reload * 2);
        }
        if shots.len() == 2 {
            break;
        }
    }
    assert_eq!(shots.len(), 2, "{shots:?}");
    assert!(shots[1] - shots[0] >= 2 * reload as u64);
}

#[test]
fn in_a_trench_the_gunner_rises_to_fire_and_ducks_to_reload() {
    let mut w = World::new_match(setup(), 1).unwrap();
    assert!(w.dig_trench_now(&[xy(300, 490), xy(300, 510)], 1));
    let team = panzerschreck(&mut w, xy(300, 500), EAST);
    let carrier = members(&w, team)[0];
    for _ in 0..90 {
        w.step(&[]);
    }
    assert!(w.entities().combat()[carrier.index()].in_trench);
    let (_, _tank) = harmless_sherman(&mut w, xy(400, 500), WEST);
    let mut fired = None;
    for _ in 0..600 {
        let before = w.stats().rockets_fired;
        w.step(&[]);
        if w.stats().rockets_fired > before {
            fired = Some(w.tick());
            break;
        }
    }
    assert!(fired.is_some(), "no rocket from the trench");
    let c = w.entities().combat()[carrier.index()];
    assert_eq!(c.height, c.fire_height, "on the fire step when firing");
    for _ in 0..90 {
        w.step(&[]);
    }
    let c = w.entities().combat()[carrier.index()];
    assert!(
        c.height < c.fire_height,
        "down below the lip while reloading"
    );
}

#[test]
fn rockets_resupply_in_the_own_base() {
    let mut w = World::new_match(setup(), 1).unwrap();
    let base = w.game().unwrap().base_of(PlayerId(0));
    let team = panzerschreck(&mut w, base, EAST);
    w.edit_launcher(team, |l| l.rounds = 4);
    let every = w.match_setup().unwrap().rules.items.resupply_ticks as u64;
    for _ in 0..every {
        w.step(&[]);
    }
    assert_eq!(launcher(&w, team).rounds, 5);
    for _ in 0..every {
        w.step(&[]);
    }
    assert_eq!(launcher(&w, team).rounds, 6);
    for _ in 0..2 * every {
        w.step(&[]);
    }
    assert_eq!(
        launcher(&w, team).rounds,
        6,
        "never above the carried count"
    );
    // Outside the base: none.
    let mut w = World::new_match(setup(), 1).unwrap();
    let team = panzerschreck(&mut w, xy(500, 500), EAST);
    w.edit_launcher(team, |l| l.rounds = 4);
    for _ in 0..2 * every {
        w.step(&[]);
    }
    assert_eq!(launcher(&w, team).rounds, 4);
}

#[test]
fn the_backblast_gives_the_gunner_away() {
    // US riflemen 300 m west watch the team: beyond their 150 m sight for a
    // standing man and beyond a rifle's 150 % reveal, within the
    // launcher's 300 %. The team's target is a Sherman whose sight is cut
    // to 20 m, so it sees nobody.
    let mut s = setup();
    for v in s.vehicles.iter_mut() {
        v.sight = frontline_content::Fixed::from_int(20);
    }
    let mut w = World::new_match(s, 1).unwrap();
    let team = panzerschreck(&mut w, xy(500, 500), EAST);
    let carrier = members(&w, team)[0];
    spawn(&mut w, 1, "ww2:usa_rifle_squad", xy(200, 500), NORTH);
    let (_, _target) = harmless_sherman(&mut w, xy(600, 500), WEST);
    let seen = |w: &World| w.entities().combat()[carrier.index()].seen_by & 2 != 0;
    let mut fired = None;
    for _ in 0..600 {
        let before = w.stats().rockets_fired;
        w.step(&[]);
        if w.stats().rockets_fired > before {
            fired = Some(w.tick());
            break;
        }
        assert!(!seen(&w), "seen before firing");
    }
    let t0 = fired.expect("a rocket");
    let mut spotted = None;
    for _ in 0..90 {
        w.step(&[]);
        if seen(&w) {
            spotted = Some(w.tick() - t0);
            break;
        }
    }
    assert!(
        spotted.is_some(),
        "the backblast (300 % for 3 s) spots him at 300 m"
    );
}

// ------------------------------------------------- blasts on the tracks --

/// A German Panzer IV at (500, 500) facing east, its crew reduced to the
/// driver so it never fires.
fn parked_panzer(w: &mut World) -> SimId {
    let sq = spawn(w, 0, "ww2:ger_pz4_h_squad", xy(500, 500), EAST);
    let v = members(w, sq)[0];
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
    v
}

fn hp(w: &World, v: SimId, k: ComponentKind) -> u16 {
    w.game().unwrap().vehicle(v).unwrap().component(k)
}

#[test]
fn a_ground_satchel_beside_a_track_destroys_it_and_nothing_inside() {
    let mut w = World::new_match(setup(), 1).unwrap();
    let v = parked_panzer(&mut w);
    // US engineers 6 m south of the right (south) track.
    let eng = spawn(&mut w, 1, "ww2:usa_engineer_squad", xy(500, 494), NORTH);
    let men = members(&w, eng);
    let thrower = men[0];
    let engine = hp(&w, v, ComponentKind::Engine);
    let seats: Vec<u16> = w
        .game()
        .unwrap()
        .vehicle(v)
        .unwrap()
        .seats
        .iter()
        .map(|s| s.hp)
        .collect();
    w.step(&[cmd(
        1,
        0,
        CommandKind::Throw {
            units: vec![thrower],
            item: ItemKind::Satchel,
            target: FxVec2::new(Fx::from_int(500), Fx::from_milli(497_500)),
        },
    )]);
    let mut disabled = 0;
    for _ in 0..300 {
        w.step(&[]);
        disabled += w
            .snapshot()
            .armour_events
            .iter()
            .filter(|e| {
                e.kind == ArmourEventKind::Disabled as u8
                    && e.component == ComponentKind::TrackRight as u8
            })
            .count();
    }
    assert_eq!(w.stats().throws, 1);
    assert_eq!(hp(&w, v, ComponentKind::TrackRight), 0, "300 × 100 %");
    assert_eq!(disabled, 1);
    assert_eq!(
        hp(&w, v, ComponentKind::Engine),
        engine,
        "insides untouched"
    );
    let after: Vec<u16> = w
        .game()
        .unwrap()
        .vehicle(v)
        .unwrap()
        .seats
        .iter()
        .map(|s| s.hp)
        .collect();
    assert_eq!(after, seats);
    assert!(w.stats().blast_track_hits >= 1);
    let vs = w.game().unwrap().vehicle(v).unwrap();
    assert!(vs.immobilised(&w.match_setup().unwrap().vehicles[vs.type_idx as usize]));
    assert_eq!(w.stats().knockouts, 0);
}

#[test]
fn own_satchels_and_frags_spare_own_tracks_and_frags_barely_scratch() {
    // A German satchel beside a German tank: no friendly fire.
    let mut w = World::new_match(setup(), 1).unwrap();
    let v = parked_panzer(&mut w);
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", xy(500, 494), NORTH);
    let thrower = members(&w, eng)[0];
    w.step(&[cmd(
        0,
        0,
        CommandKind::Throw {
            units: vec![thrower],
            item: ItemKind::Satchel,
            target: FxVec2::new(Fx::from_int(500), Fx::from_milli(497_500)),
        },
    )]);
    for _ in 0..300 {
        w.step(&[]);
    }
    assert_eq!(w.stats().throws, 1);
    assert_eq!(hp(&w, v, ComponentKind::TrackRight), 100);
    assert_eq!(w.stats().blast_track_hits, 0);

    // A US frag beside it: 10 % of 150.
    let mut w = World::new_match(setup(), 1).unwrap();
    let v = parked_panzer(&mut w);
    let rifles = spawn(&mut w, 1, "ww2:usa_rifle_squad", xy(500, 480), NORTH);
    let thrower = members(&w, rifles)[0];
    w.step(&[cmd(
        1,
        0,
        CommandKind::Throw {
            units: vec![thrower],
            item: ItemKind::Frag,
            target: FxVec2::new(Fx::from_int(500), Fx::from_milli(497_500)),
        },
    )]);
    for _ in 0..300 {
        w.step(&[]);
    }
    assert_eq!(w.stats().throws, 1);
    let right = hp(&w, v, ComponentKind::TrackRight);
    assert!(
        (85..100).contains(&right),
        "a frag takes about 15 HP off a track: {right}"
    );
    assert_eq!(hp(&w, v, ComponentKind::Engine), 100);
}
