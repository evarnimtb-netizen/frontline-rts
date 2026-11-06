//! S3 gunnery and vehicle damage (sim stage 3, spec step 4): aim-point
//! choice, laying and lead, main guns and MGs firing, HE against men,
//! penetration and knock-out in a duel, crew swap, fire, field and base
//! repair, attack-move halts, and determinism with shells in flight,
//! burning vehicles and crew swaps across save → load → continue.

use std::path::Path;

use frontline_content::{ComponentKind, Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::armour::{BoxPart, deg};
use frontline_sim::gunnery::{aim_choice, lay_gun, lead_estimate, mass_centre, solution};
use frontline_sim::{
    ArmourEventKind, Command, CommandKind, EntityKind, Fx, FxVec2, FxVec3, PlayerId, SimId,
    SquadId, World,
};

fn setup(seed: u64) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let mut defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let rules = defs
        .add_rules_override(&root.join("tests/rules_test_unlock.toml"))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut req = MatchRequest::skirmish(seed, "ww2:germany", "ww2:usa", Difficulty::Normal);
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

fn squad_type(w: &World, id: &str) -> u16 {
    w.match_setup().unwrap().squad_type_index(id).unwrap()
}

/// A tank for `player` (0: Panzer IV, 1: Sherman) at `at` facing `f`.
fn tank(w: &mut World, player: u8, at: FxVec2, f: FxVec2) -> (SquadId, SimId) {
    let id = if player == 0 {
        "ww2:ger_pz4_h_squad"
    } else {
        "ww2:usa_m4a3_75_squad"
    };
    let t = squad_type(w, id);
    let sq = w.spawn_squad_at(PlayerId(player), t, at, f).unwrap();
    (sq, w.game().unwrap().squad(sq).unwrap().members[0])
}

fn rifles(w: &mut World, player: u8, at: FxVec2, f: FxVec2) -> SquadId {
    let id = if player == 0 {
        "ww2:ger_grenadiergruppe"
    } else {
        "ww2:usa_rifle_squad"
    };
    let t = squad_type(w, id);
    w.spawn_squad_at(PlayerId(player), t, at, f).unwrap()
}

fn yaw(f: FxVec2) -> Fx {
    Fx::atan2(f.y, f.x)
}

fn events(w: &World, kind: ArmourEventKind) -> usize {
    w.snapshot()
        .armour_events
        .iter()
        .filter(|e| e.kind == kind as u8)
        .count()
}

#[test]
fn lay_gun_turns_at_the_def_rates_within_the_elevation_limits() {
    let mut w = World::new_match(setup(1), 1).unwrap();
    let (_, v) = tank(&mut w, 0, xy(500, 500), EAST);
    let g = w.game().unwrap();
    let def = &g.setup().vehicles[g.vehicle(v).unwrap().type_idx as usize];
    let mut vs = g.vehicle(v).unwrap().clone();
    // 14°/s at 30 Hz: 90° takes about 6.4 s.
    let mut ticks = 0;
    while !lay_gun(&mut vs, def, deg(90), Fx::ZERO, 30) {
        ticks += 1;
        assert!(ticks < 1000);
    }
    assert!((190..=195).contains(&ticks), "{ticks}");
    assert_eq!(vs.turret_yaw, deg(90));
    // Elevation clamps to +20° and −8°.
    for _ in 0..300 {
        lay_gun(&mut vs, def, deg(90), deg(45), 30);
    }
    assert_eq!(vs.gun_pitch, deg(20));
    for _ in 0..300 {
        lay_gun(&mut vs, def, deg(90), deg(-45), 30);
    }
    assert_eq!(vs.gun_pitch, deg(-8));
    // A damaged ring halves the traverse; a destroyed one fixes the turret.
    vs.components[ComponentKind::TurretRing as usize] = 30;
    let before = vs.turret_yaw;
    lay_gun(&mut vs, def, Fx::ZERO, Fx::ZERO, 30);
    let half = (before - vs.turret_yaw).abs();
    assert!(
        (half - deg(14).div_int(60)).abs() <= Fx::from_raw(2),
        "{half:?}"
    );
    vs.components[ComponentKind::TurretRing as usize] = 0;
    let fixed = vs.turret_yaw;
    lay_gun(&mut vs, def, Fx::ZERO, Fx::ZERO, 30);
    assert_eq!(vs.turret_yaw, fixed);
    // The shortest way round.
    vs.components[ComponentKind::TurretRing as usize] = 80;
    vs.turret_yaw = deg(170);
    lay_gun(&mut vs, def, deg(-170), Fx::ZERO, 30);
    assert!(vs.turret_yaw > deg(170) || vs.turret_yaw < deg(-170));
}

#[test]
fn the_solution_leads_a_crossing_target_and_compensates_drop() {
    let from = FxVec3::new(Fx::from_int(100), Fx::from_int(100), Fx::from_int(20));
    let at = FxVec3::new(Fx::from_int(400), Fx::from_int(100), Fx::from_int(20));
    let (still, t) = solution(from, at, FxVec2::ZERO, Fx::from_int(790));
    // 300 m at 790 m/s: 0.38 s; aimed slightly up, straight on.
    assert!((t - Fx::from_milli(380)).abs() < Fx::from_milli(3), "{t:?}");
    assert!(still.y.is_zero() && still.z > Fx::ZERO);
    // A target crossing north at 6 m/s: lead about 2.3 m, 7.6 mrad.
    let (lead, _) = solution(
        from,
        at,
        FxVec2::new(Fx::ZERO, Fx::from_int(6)),
        Fx::from_int(790),
    );
    let mrad = lead.y.div(lead.x);
    assert!(
        (mrad - Fx::from_milli(7)).abs() < Fx::from_milli(2),
        "{mrad:?}"
    );
}

#[test]
fn the_gunner_aims_at_the_centre_of_visible_mass() {
    let mut w = World::new_match(setup(1), 1).unwrap();
    // Sherman 300 m east of a Panzer IV; the Panzer IV faces it squarely,
    // then angled 30°.
    let (_, pz) = tank(&mut w, 0, xy(400, 500), EAST);
    let (_, sh) = tank(&mut w, 1, xy(700, 500), WEST);
    let look = |w: &World, from: SimId, at: SimId| {
        let g = w.game().unwrap();
        let d = g.vehicle_derived();
        let (fa, ft) = (d.frame(from).unwrap(), d.frame(at).unwrap());
        let muzzle = fa.turret.center + FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::HALF);
        (
            aim_choice(muzzle, ft, w.terrain()),
            mass_centre(muzzle, ft),
            *ft,
        )
    };
    // Squarely: both parts in view, so the centre of mass, whatever the
    // round (S3b: no more "the weakest plate"). The Panzer IV's frontal
    // hull (2.8 × 1.36 m) outweighs its turret (1.9 × 0.9 m): the point
    // sits about 1.4 m up, between the hull centre (1.08 m) and the
    // turret centre (2.21 m), so most shots strike the upper hull front.
    let (part, c, f) = look(&w, sh, pz);
    assert_eq!(part, Some(BoxPart::HULL));
    let h = c.z - f.origin.z;
    assert!(
        h > Fx::from_milli(1350) && h < Fx::from_milli(1500),
        "aim height {h:?}"
    );
    assert!((c.xy() - f.hull.center.xy()).length() < Fx::from_milli(300));
    let (part, _, _) = look(&w, pz, sh);
    assert_eq!(part, Some(BoxPart::HULL));
    // Angled 30°: still the centre of mass (the angle changes the weights
    // a little, not the rule).
    w.place_vehicle(pz, xy(400, 500), deg(30));
    let (part, c, f) = look(&w, sh, pz);
    assert_eq!(part, Some(BoxPart::HULL));
    assert!(c.z - f.origin.z > Fx::from_milli(1300));
    // The gunnery pass itself: the Sherman's gunner aims at the centre of
    // mass (the Panzer IV's gun broken, so it does not shoot first).
    w.edit_vehicle(pz, |vs| vs.components[ComponentKind::Gun as usize] = 0);
    let mut first = None;
    for _ in 0..40 {
        w.step(&[]);
        let vs = w.game().unwrap().vehicle(sh).unwrap();
        if first.is_none() && vs.target.is_some() {
            first = Some((vs.target, vs.aim_turret));
        }
    }
    assert_eq!(first, Some((Some(pz), false)));
    let _ = yaw;
}

#[test]
fn moving_target_lead_estimate_errs_and_shrinks_with_ranging() {
    let vel = FxVec2::new(Fx::ZERO, Fx::from_int(6));
    let est = |ranging: u8, target: u32| {
        lead_estimate(vel, 7, SimId(3), SimId(target), ranging, 40, 20, 50)
    };
    // The same target and ranging step: the same estimate (no state).
    assert_eq!(est(0, 9), est(0, 9));
    // Within ±40 % at the first shot, and never a different direction.
    let mut errs = Vec::new();
    for t in 0..200 {
        let e = est(0, t);
        assert!(e.x.is_zero() && e.y > Fx::ZERO);
        let err = (e.y - vel.y).abs();
        assert!(err <= Fx::from_int(6).mul_int(40).div_int(100) + Fx::EPSILON);
        errs.push(err);
    }
    // Errors spread out (not all zero) ...
    assert!(errs.iter().filter(|e| **e > Fx::from_milli(300)).count() > 50);
    // ... and shrink as the gunner ranges in, to half at the floor.
    for t in 0..200 {
        let first = (est(0, t).y - vel.y).abs();
        let fourth = (est(3, t).y - vel.y).abs();
        assert!(fourth <= Fx::from_int(6).mul_int(20).div_int(100) + Fx::EPSILON);
        let _ = first;
    }
    // No error with the key at 0, or on a still target.
    assert_eq!(lead_estimate(vel, 7, SimId(3), SimId(9), 0, 0, 20, 50), vel);
    assert_eq!(
        lead_estimate(FxVec2::ZERO, 7, SimId(3), SimId(9), 0, 40, 20, 50),
        FxVec2::ZERO
    );
}

/// Runs a Panzer IV vs Sherman duel at 300 m until one is knocked out;
/// returns (ticks, winner, hashes every second).
fn duel(seed: u64, threads: usize) -> (u64, Option<SimId>, Vec<u64>, World) {
    let mut w = World::new_match(setup(seed), threads).unwrap();
    let (_, pz) = tank(&mut w, 0, xy(350, 500), EAST);
    let (_, sh) = tank(&mut w, 1, xy(650, 500), WEST);
    let mut hashes = Vec::new();
    for _ in 0..30 * 120 {
        w.step(&[]);
        if w.tick().is_multiple_of(30) {
            hashes.push(w.state_hash());
        }
        let g = w.game().unwrap();
        let ko = |v| g.vehicle(v).is_none_or(|s| s.wreck_tick.is_some());
        if ko(pz) || ko(sh) {
            let winner = if ko(pz) { sh } else { pz };
            return (w.tick(), Some(winner), hashes, w);
        }
    }
    (w.tick(), None, hashes, w)
}

#[test]
fn a_tank_duel_ends_in_a_knock_out_and_is_deterministic() {
    let mut wins = [0u32; 2];
    let mut draws = 0;
    for seed in 1..=6 {
        let (ticks, winner, hashes, w) = duel(seed, 1);
        if seed <= 2 {
            let (t3, w3, h3, _) = duel(seed, 3);
            assert_eq!((t3, w3), (ticks, winner));
            assert_eq!(h3, hashes, "seed {seed}: 1 vs 3 threads");
        }
        // S3b: hits no longer wipe a turret out at once; a duel can end with
        // both tanks immobilised and blind (commander lost, the other out of
        // the reduced sight), which is a result too.
        let Some(winner) = winner else {
            draws += 1;
            assert!(w.stats().penetrations >= 1);
            continue;
        };
        wins[(winner.0 % 2) as usize] += 1;
        assert!(w.stats().penetrations >= 1);
        assert_eq!(w.stats().knockouts, 1);
        let g = w.game().unwrap();
        let wrecks: Vec<_> = g
            .vehicles()
            .iter()
            .filter(|(_, v)| v.wreck_tick.is_some())
            .collect();
        assert_eq!(wrecks.len(), 1);
        let (&wid, _) = wrecks[0];
        assert_eq!(w.entities().kind()[wid.index()], EntityKind::Wreck);
        assert_eq!(w.snapshot().vehicles.len(), 2);
        // The survivor stops aiming at the wreck.
        let alive = g.vehicles().get(&winner).unwrap();
        assert_ne!(alive.target, Some(wid));
    }
    assert!(draws <= 2, "{draws} of 6 duels without a knock-out");
    println!("duel wins (by id parity): {wins:?}, draws {draws}");
}

#[test]
fn shells_burning_vehicles_and_crew_swaps_survive_save_load_continue() {
    let mut w = World::new_match(setup(3), 2).unwrap();
    let (_, pz) = tank(&mut w, 0, xy(350, 500), EAST);
    let (_, sh) = tank(&mut w, 1, xy(650, 500), WEST);
    // A burning Panzer IV with a dead gunner (the swap starts next tick).
    w.edit_vehicle(pz, |vs| {
        vs.burn = 1;
        vs.seats[1].hp = 0;
    });
    let _ = sh;
    let mut saved = None;
    for _ in 0..400 {
        w.step(&[]);
        let shell = !w.snapshot().shells.is_empty();
        let swapping = w
            .game()
            .unwrap()
            .vehicle(pz)
            .is_some_and(|v| v.seats.iter().any(|s| s.swap > 0));
        if shell && swapping && saved.is_none() {
            saved = Some(w.save());
            break;
        }
    }
    let bytes = saved.expect("a moment with a shell in flight, a fire and a swap");
    let mut a = World::load(&bytes, 1).unwrap();
    let mut b = World::load(&bytes, 3).unwrap();
    assert!(a.game().unwrap().vehicle(pz).unwrap().burn > 0);
    for _ in 0..30 * 30 {
        a.step(&[]);
        b.step(&[]);
        w.step(&[]);
        assert_eq!(a.state_hash(), w.state_hash());
        assert_eq!(b.state_hash(), w.state_hash());
    }
}

#[test]
fn an_empty_gunner_seat_is_refilled_by_the_radio_operator() {
    // S3c: no bail-out for crew losses (the last step leaves two men).
    let mut s = setup(1);
    s.rules.vehicles.bail.crew_below = 0;
    let mut w = World::new_match(s, 1).unwrap();
    let (_, v) = tank(&mut w, 0, xy(500, 500), EAST);
    w.edit_vehicle(v, |vs| vs.seats[1].hp = 0);
    w.step(&[]);
    let def = &w.match_setup().unwrap().vehicles[0].clone();
    let vs = w.game().unwrap().vehicle(v).unwrap();
    // Seat order: driver, gunner, commander, loader, radio_mg.
    assert_eq!(def.seats[4].role, frontline_content::SeatRole::RadioMg);
    assert!(vs.seats[1].swap > 0 && vs.seats[1].hp == 100);
    assert_eq!(vs.seats[1].swap_from, 4);
    assert_eq!(vs.seats[4].hp, 0);
    assert!(!vs.manned(def, frontline_content::SeatRole::Gunner));
    assert_eq!(w.snapshot().vehicles[0].seats[1], 4, "swapping");
    for _ in 0..4 * 30 {
        w.step(&[]);
    }
    let vs = w.game().unwrap().vehicle(v).unwrap();
    assert!(vs.manned(def, frontline_content::SeatRole::Gunner));
    // Then the driver: the loader is next.
    w.edit_vehicle(v, |vs| vs.seats[0].hp = 0);
    w.step(&[]);
    let vs = w.game().unwrap().vehicle(v).unwrap();
    assert_eq!(vs.seats[0].swap_from, 3);
    // Driver gone with nobody to swap: immobile.
    w.edit_vehicle(v, |vs| {
        for s in &mut vs.seats {
            *s = Default::default();
        }
        vs.seats[2].hp = 100;
        vs.seats[1].hp = 100;
    });
    w.step(&[]);
    let vs = w.game().unwrap().vehicle(v).unwrap().clone();
    // The commander moves to the driver's seat.
    assert_eq!(vs.seats[0].swap_from, 2);
    assert!(vs.immobile(def));
}

#[test]
fn a_fire_burns_the_engine_and_ends_out_or_in_a_knock_out() {
    let (mut out, mut ko) = (0, 0);
    for seed in 1..=40 {
        // S3c: the crew stays and fights the fire (no engine-fire
        // bail-out), the S3 fire rule alone.
        let mut s = setup(seed);
        s.rules.vehicles.bail.engine_fire_bail_s = 0;
        let mut w = World::new_match(s, 1).unwrap();
        let (_, v) = tank(&mut w, 0, xy(500, 500), EAST);
        w.edit_vehicle(v, |vs| vs.burn = 1);
        for _ in 0..30 {
            w.step(&[]);
        }
        let e = w.game().unwrap().vehicle(v).unwrap().components[0];
        assert_eq!(e, 95, "5 HP/s");
        for _ in 0..30 * 21 {
            w.step(&[]);
        }
        let vs = w.game().unwrap().vehicle(v).unwrap();
        if vs.wreck_tick.is_some() {
            ko += 1;
        } else {
            assert_eq!(vs.burn, 0);
            out += 1;
        }
    }
    // Four 25 % rolls (at 4, 8, 12, 16 s): knocked out with 0.75⁴ = 31.6 %.
    let p = 0.75f64.powi(4);
    let sigma = (40.0 * p * (1.0 - p)).sqrt();
    assert!(
        ((ko as f64) - 40.0 * p).abs() <= 3.0 * sigma + 1.0,
        "{ko} knocked out, {out} put out"
    );
}

#[test]
fn field_repair_fixes_the_worst_part_first_and_a_hit_stops_it() {
    let mut w = World::new_match(setup(1), 1).unwrap();
    let (_, v) = tank(&mut w, 0, xy(500, 500), EAST);
    w.edit_vehicle(v, |vs| {
        vs.components[ComponentKind::TrackLeft as usize] = 0;
        vs.components[ComponentKind::Engine as usize] = 20;
    });
    let def = w.match_setup().unwrap().vehicles[0].clone();
    assert!(w.game().unwrap().vehicle(v).unwrap().immobilised(&def));
    assert!(w.start_repair(v));
    let vs = w.game().unwrap().vehicle(v).unwrap();
    assert_eq!(vs.repairing, Some(ComponentKind::TrackLeft as u8));
    assert_eq!(
        w.snapshot().vehicles[0].repairing,
        ComponentKind::TrackLeft as u8
    );
    // 100 HP in 30 s.
    for _ in 0..30 * 30 {
        w.step(&[]);
    }
    let vs = w.game().unwrap().vehicle(v).unwrap();
    assert_eq!(vs.components[ComponentKind::TrackLeft as usize], 100);
    assert_eq!(vs.repairing, Some(ComponentKind::Engine as u8));
    // The engine to 50 at 50 HP per 60 s: 30 HP in 36 s.
    for _ in 0..30 * 37 {
        w.step(&[]);
    }
    let vs = w.game().unwrap().vehicle(v).unwrap();
    assert_eq!(vs.components[ComponentKind::Engine as usize], 50);
    assert_eq!(vs.repairing, None);
    assert!(!w.start_repair(v), "nothing left below its repair target");
    // Nothing repairs the gun in the field.
    w.edit_vehicle(v, |vs| vs.components[ComponentKind::Gun as usize] = 10);
    assert!(!w.start_repair(v));

    // A hit stops a repair: a Sherman 200 m away opens fire.
    w.edit_vehicle(v, |vs| {
        vs.components[ComponentKind::TrackRight as usize] = 10
    });
    assert!(w.start_repair(v));
    let (_, _sh) = tank(&mut w, 1, xy(700, 500), WEST);
    let stopped = (0..30 * 30).any(|_| {
        w.step(&[]);
        let hit = w
            .snapshot()
            .armour_events
            .iter()
            .any(|e| e.vehicle == v.0 && matches!(e.kind, 1..=3));
        hit && w
            .game()
            .unwrap()
            .vehicle(v)
            .is_none_or(|s| s.repairing.is_none())
    });
    assert!(stopped);
}

#[test]
fn base_repair_restores_parts_rounds_and_crew() {
    let mut w = World::new_match(setup(1), 1).unwrap();
    let base = w.game().unwrap().base_of(PlayerId(0));
    let (_, v) = tank(&mut w, 0, base, EAST);
    w.edit_vehicle(v, |vs| {
        vs.components = [10, 50, 100, 0, 80, 60, 100];
        vs.ammo = vec![20, 25, 2];
        vs.seats[3].hp = 0;
    });
    for _ in 0..30 * 20 {
        w.step(&[]);
    }
    let vs = w.game().unwrap().vehicle(v).unwrap();
    // 2 % per second for 20 s: +40 % of each part's HP.
    assert_eq!(vs.components[0], 50);
    assert_eq!(vs.components[1], 90);
    assert_eq!(vs.components[3], 32, "the gun too: 40 % of 80");
    let rounds = vs.ammo.iter().sum::<u16>() + vs.loaded.is_some() as u16;
    // S5a: one standard round per 2 s, the rare PzGr 40 on its own timer
    // (one per 10 s, up to rare_cap 3: one more here), each paid.
    assert_eq!(
        rounds,
        47 + 10 + 1,
        "one round per 2 s, a rare one per 10 s"
    );
    assert!(
        w.game().unwrap().players()[0].ammo.paid[2] > 0,
        "paid at the shells price"
    );
    assert_eq!(vs.seats[3].hp, 100, "one dead seat per 20 s");
}

#[test]
fn a_tank_fires_he_and_its_mgs_at_men_and_never_near_its_own() {
    let mut w = World::new_match(setup(2), 1).unwrap();
    let (_, v) = tank(&mut w, 0, xy(350, 500), EAST);
    let us = rifles(&mut w, 1, xy(480, 500), WEST);
    let (mut he, mut mg) = (0, 0);
    let mg34 = w
        .match_setup()
        .unwrap()
        .weapons
        .iter()
        .position(|x| x.id == "ww2:mg34")
        .unwrap() as u16;
    for _ in 0..30 * 60 {
        w.step(&[]);
        he += w
            .snapshot()
            .armour_events
            .iter()
            .filter(|e| e.kind == ArmourEventKind::Fired as u8 && e.component == 2)
            .count();
        mg += w
            .snapshot()
            .shots
            .iter()
            .filter(|s| s.shooter == v && s.weapon == mg34)
            .count();
    }
    assert!(he >= 1, "HE at the squad");
    assert!(mg >= 20, "MG rounds: {mg}");
    let left = w.game().unwrap().squad(us).map_or(0, |s| s.members.len());
    assert!(left < 8, "the squad took losses: {left} left");
    let vs = w.game().unwrap().vehicle(v).unwrap();
    // S5a: 42 Sprgr 34 stowed.
    assert!(vs.ammo[1] < 42);

    // Own men within 30 m of the enemy: no HE.
    let mut w = World::new_match(setup(2), 1).unwrap();
    tank(&mut w, 0, xy(350, 500), EAST);
    rifles(&mut w, 1, xy(480, 500), WEST);
    rifles(&mut w, 0, xy(478, 488), EAST);
    let mut he = 0;
    for _ in 0..30 * 20 {
        w.step(&[]);
        he += w
            .snapshot()
            .armour_events
            .iter()
            .filter(|e| e.kind == ArmourEventKind::Fired as u8 && e.component == 2)
            .count();
    }
    assert_eq!(he, 0);
}

/// S3b Fix (review M4): a vehicle MG spends its stowed rounds, falls
/// silent when they are gone, and base rearm refills a belt per step.
#[test]
fn vehicle_mgs_spend_stowed_rounds_and_rearm_in_the_base() {
    let mut w = World::new_match(setup(2), 1).unwrap();
    let (_, v) = tank(&mut w, 0, xy(350, 500), EAST);
    rifles(&mut w, 1, xy(480, 500), WEST);
    let stowed: Vec<u32> = {
        let s = w.match_setup().unwrap();
        let vt = &s.vehicles[w.game().unwrap().vehicle(v).unwrap().type_idx as usize];
        vt.mounts
            .iter()
            .skip(1)
            .map(|m| m.rounds.unwrap())
            .collect()
    };
    assert_eq!(stowed, vec![1575, 1575], "Panzer IV H: 3,150 rounds");
    // Both MGs dry: no MG round in a minute.
    w.edit_vehicle(v, |vs| {
        for (m, n) in vs.mgs.iter_mut().zip(&stowed) {
            m.spent = *n;
        }
    });
    let mg34 = w
        .match_setup()
        .unwrap()
        .weapons
        .iter()
        .position(|x| x.id == "ww2:mg34")
        .unwrap() as u16;
    let mut mg = 0;
    for _ in 0..30 * 30 {
        w.step(&[]);
        mg += w
            .snapshot()
            .shots
            .iter()
            .filter(|s| s.shooter == v && s.weapon == mg34)
            .count();
    }
    assert_eq!(mg, 0, "dry MGs fire nothing");
    // One round short: exactly one more MG round.
    let mut w = World::new_match(setup(2), 1).unwrap();
    let (_, v) = tank(&mut w, 0, xy(350, 500), EAST);
    rifles(&mut w, 1, xy(480, 500), WEST);
    w.edit_vehicle(v, |vs| {
        vs.mgs[0].spent = 1574;
        vs.mgs[1].spent = 1575;
    });
    for _ in 0..30 * 30 {
        w.step(&[]);
    }
    let vs = w.game().unwrap().vehicle(v).unwrap();
    assert_eq!(vs.mgs[0].spent, 1575, "fired its last round");
    assert_eq!(vs.mgs[0].shots, 1);
    // In the base: a 150-round belt per MG per rearm step (2 s).
    let mut w = World::new_match(setup(1), 1).unwrap();
    let base = w.game().unwrap().base_of(PlayerId(0));
    let (_, v) = tank(&mut w, 0, base, EAST);
    w.edit_vehicle(v, |vs| {
        vs.mgs[0].spent = 1575;
        vs.mgs[1].spent = 200;
    });
    for _ in 0..30 * 4 {
        w.step(&[]);
    }
    let vs = w.game().unwrap().vehicle(v).unwrap();
    assert_eq!(vs.mgs[0].spent, 1575 - 2 * 150);
    assert_eq!(vs.mgs[1].spent, 0);
}

#[test]
fn an_attack_moving_tank_halts_to_fire() {
    let mut w = World::new_match(setup(1), 1).unwrap();
    let (sq, v) = tank(&mut w, 0, xy(300, 500), EAST);
    let (_, sh) = tank(&mut w, 1, xy(640, 500), WEST);
    // Keep the Sherman from shooting back.
    w.edit_vehicle(sh, |vs| vs.components[ComponentKind::Gun as usize] = 0);
    w.step(&[Command {
        player: PlayerId(0),
        seq: 0,
        kind: CommandKind::AttackMove {
            squad: sq,
            target: xy(700, 500),
        },
    }]);
    let mut halted = false;
    let mut fired = 0;
    for _ in 0..30 * 40 {
        w.step(&[]);
        fired += events(&w, ArmourEventKind::Fired);
        let vs = w.game().unwrap().vehicle(v).unwrap();
        if vs.target == Some(sh) && vs.speed.is_zero() {
            halted = true;
        }
    }
    assert!(halted && fired >= 1, "halted {halted}, fired {fired}");
}
