//! S7 casemate gunnery (`docs/slices/S7-casemate-td.md` 3): the gun lays
//! only within its arc; outside it the gunner asks the driver for a hull
//! turn (`VehicleState::lay_turn`), the hull pivots without overshoot, then
//! the gun lays and fires. Also the StuG and the M10 as content (boxes,
//! zones, roles) and a StuG / M10 fight across save, load and threads.

use std::path::Path;

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs, VehicleRole};
use frontline_sim::armour::deg;
use frontline_sim::gunnery::lay_gun;
use frontline_sim::{ArmourEventKind, Fx, FxVec2, PlayerId, SimId, SquadId, World};

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

fn spawn(w: &mut World, player: u8, card: &str, at: FxVec2, f: FxVec2) -> (SquadId, SimId) {
    let t = w.match_setup().unwrap().squad_type_index(card).unwrap();
    let sq = w.spawn_squad_at(PlayerId(player), t, at, f).unwrap();
    (sq, w.game().unwrap().squad(sq).unwrap().members[0])
}

fn yaw_of(w: &World, id: SimId) -> Fx {
    w.entities().transform()[id.index()].yaw
}

fn pos_of(w: &World, id: SimId) -> FxVec2 {
    w.entities().transform()[id.index()].pos.xy()
}

fn fired(w: &World, id: SimId) -> bool {
    w.snapshot()
        .armour_events
        .iter()
        .any(|e| e.kind == ArmourEventKind::Fired as u8 && e.vehicle == id.0)
}

/// A Sherman that cannot fire back (no rounds aboard).
fn disarm(w: &mut World, id: SimId) {
    assert!(w.edit_vehicle(id, |v| {
        v.ammo.iter_mut().for_each(|n| *n = 0);
        v.loaded = None;
    }));
}

#[test]
fn the_stug_and_the_m10_lower_with_their_boxes_and_roles() {
    let s = setup(1);
    let v = |id: &str| {
        s.vehicles
            .iter()
            .find(|v| v.id == id)
            .unwrap_or_else(|| panic!("{id}"))
    };
    let stug = v("ww2:ger_stug3_g");
    assert_eq!(stug.role, VehicleRole::AssaultGun);
    assert!(!stug.has_turret);
    assert_eq!(stug.gun_arc_deg, Some([-10, 10]));
    assert_eq!(stug.boxes.len(), 2);
    assert!(stug.boxes.iter().all(|b| b.open == 0));
    assert_eq!(stug.spot_size_pct, 160);
    let m10 = v("ww2:usa_m10");
    assert_eq!(m10.role, VehicleRole::TankDestroyer);
    assert!(m10.has_turret);
    assert_eq!(m10.gun_arc_deg, None);
    assert_eq!(m10.boxes.len(), 2);
    assert_eq!(m10.boxes[0].open, 0);
    assert_ne!(m10.boxes[1].open, 0, "the M10 turret is open-topped");
    assert_eq!(m10.seats.len(), 5);
    // The tanks keep their S3 shape: two closed boxes, a turret, no arc.
    for id in ["ww2:ger_pz4_h", "ww2:usa_m4a3_75"] {
        let t = v(id);
        assert_eq!(t.role, VehicleRole::Tank);
        assert!(t.has_turret && t.gun_arc_deg.is_none());
        assert_eq!(t.boxes.len(), 2);
    }
}

#[test]
fn a_casemate_gun_lays_only_within_its_arc() {
    let mut w = World::new_match(setup(1), 1).unwrap();
    let (_, v) = spawn(&mut w, 0, "ww2:ger_stug3_g_squad", xy(500, 500), EAST);
    let g = w.game().unwrap();
    let def = &g.setup().vehicles[g.vehicle(v).unwrap().type_idx as usize];
    let mut vs = g.vehicle(v).unwrap().clone();
    // 60° left: the gun stops at the arc edge and never reports laid.
    for _ in 0..600 {
        assert!(!lay_gun(&mut vs, def, deg(60), Fx::ZERO, 30));
    }
    assert_eq!(vs.turret_yaw, deg(10));
    for _ in 0..600 {
        assert!(!lay_gun(&mut vs, def, deg(-60), Fx::ZERO, 30));
    }
    assert_eq!(vs.turret_yaw, deg(-10));
    // 20° at 6°/s from -10° to +5°: 15° in 2.5 s, then laid.
    let mut ticks = 0;
    while !lay_gun(&mut vs, def, deg(5), Fx::ZERO, 30) {
        ticks += 1;
        assert!(ticks < 300);
    }
    assert!((74..=76).contains(&ticks), "{ticks}");
}

/// Vehicles.md 6.3: a target 40° off the nose. The gunner asks for a hull
/// turn, the hull error falls monotonically and never changes sign (no
/// overshoot), the turn ends inside the arc and the gun lays and fires.
#[test]
fn a_casemate_pivots_its_hull_to_lay_without_overshoot_and_fires() {
    for (left, seed) in [(true, 1), (false, 2)] {
        let mut w = World::new_match(setup(seed), 1).unwrap();
        let here = xy(400, 500);
        let off = if left { 129 } else { -129 };
        let there = here + FxVec2::from_int(153, off);
        let (_, stug) = spawn(&mut w, 0, "ww2:ger_stug3_g_squad", here, EAST);
        let (_, sh) = spawn(&mut w, 1, "ww2:usa_m4a3_75_squad", there, WEST);
        disarm(&mut w, sh);
        let bearing = |w: &World| {
            let d = pos_of(w, sh) - pos_of(w, stug);
            (Fx::atan2(d.y, d.x) - yaw_of(w, stug)).wrap_angle()
        };
        let sign = if left { Fx::ONE } else { Fx::NEG_ONE };
        let start = bearing(&w).mul(sign);
        assert!(start > deg(38) && start < deg(42), "{start:?}");
        let (mut prev, mut asked, mut shot) = (start, false, None);
        for t in 0..30 * 20 {
            w.step(&[]);
            let err = bearing(&w).mul(sign);
            assert!(err >= -deg(1), "overshot: {err:?} at tick {t}");
            assert!(err <= prev + deg(1).div_int(10), "error rose at tick {t}");
            prev = prev.min(err);
            asked |= w.game().unwrap().vehicle(stug).unwrap().lay_turn.is_some();
            if fired(&w, stug) {
                shot = Some(t);
                break;
            }
        }
        assert!(asked, "the gunner never asked for a hull turn");
        let t = shot.expect("the StuG never fired");
        // About 1.3 s to pivot 33° plus the laying: well under 8 s.
        assert!(t < 30 * 8, "fired only at tick {t}");
        assert!(bearing(&w).mul(sign) <= deg(10), "{:?}", bearing(&w));
    }
}

/// Determinism with a StuG and an M10 fighting: 1/2/3/8 threads agree,
/// and save → load → continue matches the straight run.
#[test]
fn a_stug_and_m10_fight_is_deterministic_across_threads_and_save_load() {
    let run = |threads: usize, split: Option<u64>| {
        let mut w = World::new_match(setup(7), threads).unwrap();
        spawn(&mut w, 0, "ww2:ger_stug3_g_squad", xy(400, 500), EAST);
        spawn(&mut w, 1, "ww2:usa_m10_squad", xy(620, 540), WEST);
        let mut hashes = Vec::new();
        for t in 0..30 * 40u64 {
            if Some(t) == split {
                w = World::load(&w.save(), threads).unwrap();
            }
            w.step(&[]);
            if t % 30 == 0 {
                hashes.push(w.state_hash());
            }
        }
        (hashes, w.full_state_hash(), w.stats().penetrations)
    };
    let base = run(1, None);
    assert!(base.2 > 0, "nobody penetrated: the fight did not happen");
    for threads in [2, 3, 8] {
        assert_eq!(run(threads, None), base, "{threads} threads");
    }
    assert_eq!(run(3, Some(30 * 15)), base, "save/load/continue");
}

#[test]
fn the_first_vehicle_of_each_role_is_found() {
    let s = setup(1);
    let roles = |p: u8| {
        let f = &s.factions[s.players[p as usize].faction as usize];
        f.roster
            .iter()
            .filter_map(|&t| s.squad_types[t as usize].vehicle)
            .map(|v| s.vehicles[v as usize].role)
            .collect::<Vec<_>>()
    };
    // The tank stays first (scenarios and AI tie-breaks pick it); S8a
    // appends the half-track.
    assert_eq!(
        roles(0),
        [
            VehicleRole::Tank,
            VehicleRole::AssaultGun,
            VehicleRole::Transport,
            VehicleRole::Recon,
            VehicleRole::Truck,
            VehicleRole::AtGun
        ]
    );
    assert_eq!(
        roles(1),
        [
            VehicleRole::Tank,
            VehicleRole::TankDestroyer,
            VehicleRole::Transport,
            VehicleRole::Recon,
            VehicleRole::Truck,
            VehicleRole::AtGun
        ]
    );
}

/// S7 save field `VehicleState::lay_turn`: saved, loaded, hashed, and
/// checked on load (only a casemate gun asks for a hull turn, |yaw| <= π,
/// none on a wreck).
#[test]
fn lay_turn_is_saved_hashed_and_checked_on_load() {
    let mut w = World::new_match(setup(1), 1).unwrap();
    let (_, stug) = spawn(&mut w, 0, "ww2:ger_stug3_g_squad", xy(400, 500), EAST);
    let (_, tank) = spawn(&mut w, 1, "ww2:usa_m4a3_75_squad", xy(700, 300), WEST);
    w.step(&[]);
    let base = w.full_state_hash();
    let save = w.save();
    assert!(w.edit_vehicle(stug, |v| v.lay_turn = Some(deg(30))));
    assert_ne!(w.full_state_hash(), base, "lay_turn is not hashed");
    assert_eq!(w.full_state_hash(), w.state_hash());
    let loaded = World::load(&w.save(), 2).unwrap();
    assert_eq!(
        loaded.game().unwrap().vehicle(stug).unwrap().lay_turn,
        Some(deg(30))
    );
    assert_eq!(loaded.full_state_hash(), w.full_state_hash());
    assert_eq!(loaded.state(), w.state());
    // Bad values are refused on load.
    let bad: [(SimId, Fx); 2] = [(stug, Fx::PI + Fx::ONE), (tank, deg(10))];
    for (id, y) in bad {
        let mut b = World::load(&save, 1).unwrap();
        assert!(b.edit_vehicle(id, |v| v.lay_turn = Some(y)));
        assert!(World::load(&b.save(), 1).is_err(), "{id:?} {y:?} accepted");
    }
    let mut b = World::load(&save, 1).unwrap();
    assert!(b.wreck_vehicle(stug));
    assert!(b.edit_vehicle(stug, |v| v.lay_turn = Some(deg(10))));
    assert!(
        World::load(&b.save(), 1).is_err(),
        "a wreck's hull turn accepted"
    );
}
