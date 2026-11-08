//! S8b Build A (`docs/slices/S8b-trucks-recon.md` 5, 6): wheeled mobility,
//! the wheeled block layer, recon optics, grenade screens, and the new
//! state across save, load and threads.

use std::path::Path;

use frontline_content::vehicle::VehicleRole;
use frontline_content::{ComponentKind, Difficulty, ItemKind, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::game::CallInRefusal;
use frontline_sim::{BlockLayer, CommandKind, Fx, FxVec2, PlayerId, SimId, SquadId, World};

fn setup(seed: u64) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let mut defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let mut req = MatchRequest::skirmish(seed, "ww2:germany", "ww2:usa", Difficulty::Normal);
    req.rules = defs
        .add_rules_override(&root.join("tests/rules_test_unlock.toml"))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut s = defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
    s.players.iter_mut().for_each(|p| p.is_ai = false);
    s.rules.start_manpower = 100_000;
    s.map.terrain.octaves.clear();
    s.map.terrain.bands.clear();
    s.map.woods.clear();
    s.rules.vehicles.bail.crew_below = 0;
    s
}

fn xy(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

const EAST: FxVec2 = FxVec2::new(Fx::ONE, Fx::ZERO);
const CAR: &str = "ww2:ger_sdkfz234_1_squad";
const M8: &str = "ww2:usa_m8_squad";

fn spawn(w: &mut World, p: u8, card: &str, at: FxVec2) -> (SquadId, SimId) {
    let t = w.match_setup().unwrap().squad_type_index(card).unwrap();
    let sq = w.spawn_squad_at(PlayerId(p), t, at, EAST).unwrap();
    (sq, w.game().unwrap().squad(sq).unwrap().members[0])
}

fn step_n(w: &mut World, n: u32) {
    for _ in 0..n {
        w.step(&[]);
    }
}

fn cmd(p: u8, seq: u32, kind: CommandKind) -> frontline_sim::Command {
    frontline_sim::scenarios::cmd(p, seq, kind)
}

fn mv(w: &mut World, p: u8, squad: SquadId, target: FxVec2) {
    let seq = w.tick() as u32;
    w.step(&[cmd(p, seq, CommandKind::Move { squad, target })]);
}

fn at(w: &World, v: SimId) -> FxVec2 {
    w.entities().transform()[v.index()].pos.xy()
}

#[test]
fn the_cars_are_recon_with_wheels_and_no_tracks() {
    let s = setup(1);
    for id in ["ww2:ger_sdkfz234_1", "ww2:usa_m8"] {
        let v = s.vehicles.iter().find(|x| x.id == id).unwrap();
        assert_eq!(v.role, VehicleRole::Recon, "{id}");
        assert!(v.has_part(ComponentKind::WheelsFront), "{id}");
        assert!(v.has_part(ComponentKind::WheelsRear), "{id}");
        assert!(!v.has_part(ComponentKind::TrackLeft), "{id}");
        assert!(!v.has_part(ComponentKind::Cargo), "{id}");
        assert_eq!(v.stationary_sight_pct, 140, "{id}");
        assert_eq!(v.settle_ticks, 120, "{id}");
    }
}

#[test]
fn the_ditch_depth_ignores_wheeled_defs() {
    let s = setup(1);
    assert_eq!(BlockLayer::wheeled_dm_of(&s), Some(10));
    // The tracked defs' shallowest depth, not the wheeled 10 dm (which would
    // make every AT ditch dug 1 m a wall for tanks).
    let tanks = s
        .vehicles
        .iter()
        .filter(|v| {
            !matches!(
                v.mobility.kind,
                frontline_content::MobilityKind::Wheeled | frontline_content::MobilityKind::Towed
            )
        })
        .map(|v| v.mobility.ditch_block)
        .min()
        .map(|d| (Fx::from(d).mul_int(10)).round_int() as u16)
        .unwrap();
    assert_eq!(BlockLayer::ditch_dm_of(&s), tanks);
    assert!(BlockLayer::ditch_dm_of(&s) > 10);
}

#[test]
fn missing_tracks_do_not_immobilise_a_car_and_a_lost_axle_does() {
    let mut w = World::new_match(setup(2), 1).unwrap();
    let (_, v) = spawn(&mut w, 0, CAR, xy(500, 500));
    step_n(&mut w, 2);
    let g = w.game().unwrap();
    let vs = g.vehicle(v).unwrap();
    let def = &w.match_setup().unwrap().vehicles[vs.type_idx as usize];
    assert!(!vs.immobilised(def), "no tracks to lose");
    assert!(!vs.gun_out(def));
    w.edit_vehicle(v, |vs| vs.components[ComponentKind::WheelsRear.index()] = 0);
    let g = w.game().unwrap();
    let vs = g.vehicle(v).unwrap();
    let def = &w.match_setup().unwrap().vehicles[vs.type_idx as usize];
    assert!(vs.immobilised(def), "a lost axle stops it");
}

#[test]
fn a_def_with_no_mounts_is_not_gun_out() {
    let s = setup(1);
    let mut w = World::new_match(s, 1).unwrap();
    let (_, v) = spawn(&mut w, 0, "ww2:ger_sdkfz251_1_squad", xy(500, 500));
    step_n(&mut w, 1);
    let g = w.game().unwrap();
    let vs = g.vehicle(v).unwrap();
    let mut def = w.match_setup().unwrap().vehicles[vs.type_idx as usize].clone();
    def.mounts.clear();
    assert!(!vs.gun_out(&def), "a truck has no armament to be out of");
}

/// A trench beside a parked car must not strand it.
#[test]
fn a_new_trench_beside_a_parked_car_does_not_strand_it() {
    let mut w = World::new_match(setup(3), 1).unwrap();
    let (sq, v) = spawn(&mut w, 0, CAR, xy(500, 500));
    step_n(&mut w, 2);
    // 5 m to the south, end to end, along the car.
    assert!(w.dig_trench_now(&[xy(470, 495), xy(530, 495)], 1));
    step_n(&mut w, 30);
    assert!(!w.game().unwrap().vehicle(v).unwrap().on_blocked);
    let to = xy(540, 520);
    mv(&mut w, 0, sq, to);
    step_n(&mut w, 30 * 20);
    let d = (at(&w, v) - to).length();
    assert!(d < Fx::from_int(6), "it drove away; {d:?} m left");
}

#[test]
fn recon_and_truck_roles_are_outside_the_limit_on_their_own_unlock() {
    let mut s = setup(4);
    s.rules.vehicles.limit = 0;
    let mut w = World::new_match(s, 1).unwrap();
    let g = w.game().unwrap();
    let card = |c: &str| w.match_setup().unwrap().squad_type_index(c).unwrap();
    let (recon, tank) = (card(CAR), card("ww2:ger_pz4_h_squad"));
    assert!(g.is_outside_limit_type(recon));
    assert!(!g.is_outside_limit_type(tank));
    assert!(matches!(
        g.call_in_check(w.entities(), PlayerId(0), recon, 0),
        Err(CallInRefusal::Locked { .. })
    ));
    step_n(&mut w, 30 * 91);
    let g = w.game().unwrap();
    g.call_in_check(w.entities(), PlayerId(0), recon, w.tick())
        .unwrap_or_else(|e| panic!("{e:?}"));
    assert!(matches!(
        g.call_in_check(w.entities(), PlayerId(0), tank, w.tick()),
        Err(CallInRefusal::VehicleLimit)
    ));
}

#[test]
fn a_car_turns_only_when_it_rolls_and_still_ticks_count_standing() {
    let mut w = World::new_match(setup(5), 1).unwrap();
    let (sq, v) = spawn(&mut w, 0, M8, xy(500, 500));
    step_n(&mut w, 2);
    let yaw = |w: &World| w.entities().transform()[v.index()].yaw;
    let y0 = yaw(&w);
    let seq = w.tick() as u32;
    w.step(&[cmd(
        0,
        seq,
        CommandKind::FaceUnits {
            units: vec![v],
            toward: xy(500, 600),
        },
    )]);
    step_n(&mut w, 30 * 3);
    assert_eq!(yaw(&w), y0, "no neutral pivot");
    assert!(w.game().unwrap().vehicle(v).unwrap().still_ticks > 60);
    // A point 90 deg to the side, close: reached by rolling.
    let to = xy(500, 540);
    mv(&mut w, 0, sq, to);
    step_n(&mut w, 30 * 30);
    assert!((at(&w, v) - to).length() < Fx::from_int(6), "it got there");
    assert_ne!(yaw(&w), y0);
}

fn frag(w: &World) -> u16 {
    w.match_setup()
        .unwrap()
        .items
        .iter()
        .position(|x| x.kind == ItemKind::Frag)
        .unwrap() as u16
}

#[test]
fn a_screen_turns_a_grenade_away_and_an_open_turret_takes_it() {
    for (card, screened) in [(CAR, true), (M8, false)] {
        let mut w = World::new_match(setup(6), 1).unwrap();
        let (_, v) = spawn(&mut w, 1, card, xy(500, 500));
        let (gsq, _) = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", xy(470, 500));
        let man = w.game().unwrap().squad(gsq).unwrap().members[0];
        step_n(&mut w, 2);
        let turret = w.game().unwrap().vehicle_derived().frame(v).unwrap().boxes[1]
            .center
            .xy();
        assert!(w.drop_item_now(man, frag(&w), turret, Fx::from_int(6)));
        step_n(&mut w, 30 * 2);
        let g = w.game().unwrap();
        let t = g.thrown().first().expect("still in the air or at rest");
        assert!(t.landed);
        assert_eq!(t.inside.is_some(), !screened, "{card}");
        if screened {
            // Beside the hull (not under or on it).
            let f = g.vehicle_derived().frame(v).unwrap();
            let b = &f.boxes[0];
            let d = t.pos.xy() - b.center.xy();
            let (lx, ly) = (b.axes.x.xy().dot(d).abs(), b.axes.y.xy().dot(d).abs());
            assert!(lx > b.half.x || ly > b.half.y, "outside the footprint");
        }
    }
}

#[test]
fn wheeled_state_is_saved_hashed_and_deterministic() {
    let start = || {
        let mut w = World::new_match(setup(9), 1).unwrap();
        let (sq, car) = spawn(&mut w, 0, CAR, xy(500, 500));
        spawn(&mut w, 1, M8, xy(640, 470));
        spawn(&mut w, 1, "ww2:usa_rifle_squad", xy(620, 530));
        let seq = w.tick() as u32;
        // A move behind it, so the car reverses first.
        w.step(&[cmd(
            0,
            seq,
            CommandKind::Move {
                squad: sq,
                target: xy(440, 520),
            },
        )]);
        (w, car)
    };
    let (mut w, car) = start();
    step_n(&mut w, 30 * 4);
    let base = w.full_state_hash();
    let loaded = World::load(&w.save(), 2).unwrap();
    assert_eq!(loaded.full_state_hash(), base);
    let mut e = World::load(&w.save(), 1).unwrap();
    assert!(e.edit_vehicle(car, |v| v.still_ticks ^= 1));
    assert_ne!(e.full_state_hash(), base, "still_ticks is not hashed");
    let run = |threads: usize, split: Option<u64>| {
        let (w0, _) = start();
        let mut w = World::load(&w0.save(), threads).unwrap();
        let mut hashes = Vec::new();
        for t in 0..30 * 12u64 {
            if Some(t) == split {
                w = World::load(&w.save(), threads).unwrap();
            }
            w.step(&[]);
            if t % 15 == 0 {
                hashes.push(w.state_hash());
            }
        }
        (hashes, w.full_state_hash())
    };
    let base = run(1, None);
    for threads in [2, 3, 8] {
        assert_eq!(run(threads, None), base, "{threads} threads");
    }
    assert_eq!(run(3, Some(30 * 6)), base, "save, load, continue");
}
