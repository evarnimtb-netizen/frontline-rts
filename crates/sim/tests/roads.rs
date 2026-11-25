//! S16a Build A (`docs/slices/S16-second-map.md` 4, 9 "S16a"): roads on a
//! small test map built here (Linden Brook is S16b): road speed by drive
//! type (R1) and the drive controller's top, path cost by time and the
//! smoothing rule (R3), the `[vehicles.road] enabled` switch, the
//! point-symmetry blend, and determinism and save/load on a road map.

use std::path::Path;

use frontline_content::setup::{FixedVec2, MapSetup, RoadSetup};
use frontline_content::{Difficulty, Fixed, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::path::{PathCost, PathScratch, VehiclePathCost, find_vehicle_path};
use frontline_sim::terrain::{FLAG_ROAD, FLAG_WOODS};
use frontline_sim::{
    BlockLayer, CommandKind, Fx, FxVec2, Heightfield, PlayerId, SimId, SquadId, World,
};

const TRUCK: &str = "ww2:ger_opel_blitz_squad";
const TRUCK_ID: &str = "ww2:ger_opel_blitz";
const EAST: FxVec2 = FxVec2::new(Fx::ONE, Fx::ZERO);

fn defs() -> (ModDefs, std::path::PathBuf) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    (ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}")), root)
}

/// Kessel Ridge flattened, no woods, human players (no AI), plus `roads`.
fn setup(roads: &[(&[[u32; 2]], u32)]) -> MatchSetup {
    let (mut defs, root) = defs();
    let mut req = MatchRequest::skirmish(5, "ww2:germany", "ww2:usa", Difficulty::Normal);
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
    s.map.roads = roads
        .iter()
        .map(|(pts, w)| RoadSetup {
            points: pts.iter().map(|&p| FixedVec2::from_m(p)).collect(),
            half_width: Fixed::from_ratio(*w as i64, 2),
        })
        .collect();
    let issues = s.validate();
    assert!(issues.is_empty(), "{issues:?}");
    s
}

fn woods(s: &mut MatchSetup, c: [u32; 2], r: i32) {
    s.map.woods.push(frontline_content::setup::CircleSetup {
        center: FixedVec2::from_m(c),
        radius: Fixed::from_int(r),
    });
}

fn xy(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

fn spawn(w: &mut World, card: &str, at: FxVec2) -> (SquadId, SimId) {
    let t = w.match_setup().unwrap().squad_type_index(card).unwrap();
    let sq = w.spawn_squad_at(PlayerId(0), t, at, EAST).unwrap();
    (sq, w.game().unwrap().squad(sq).unwrap().members[0])
}

fn mv(w: &mut World, squad: SquadId, target: FxVec2) {
    let seq = w.tick() as u32;
    w.step(&[frontline_sim::scenarios::cmd(
        0,
        seq,
        CommandKind::Move { squad, target },
    )]);
}

fn pos(w: &World, v: SimId) -> FxVec2 {
    w.entities().transform()[v.index()].pos.xy()
}

fn speed(w: &World, v: SimId) -> Fx {
    w.game().unwrap().vehicle(v).unwrap().speed
}

fn top_speed(s: &MatchSetup, id: &str) -> Fx {
    s.vehicles
        .iter()
        .find(|v| v.id == id)
        .unwrap()
        .mobility
        .speed
        .into()
}

/// Drives `card` east from x 150 to x 850 along y 500 for `ticks`; the
/// highest speed seen (in the window `x_lo..x_hi`) and the hull's track.
fn drive_east(s: MatchSetup, card: &str, ticks: u32, x_lo: i32, x_hi: i32) -> (Fx, World, SimId) {
    let mut w = World::new_match(s, 1).unwrap();
    let (sq, v) = spawn(&mut w, card, xy(150, 500));
    mv(&mut w, sq, xy(850, 500));
    let mut best = Fx::ZERO;
    for _ in 0..ticks {
        w.step(&[]);
        let p = pos(&w, v);
        if p.x >= Fx::from_int(x_lo) && p.x <= Fx::from_int(x_hi) {
            best = best.max(speed(&w, v));
        }
    }
    (best, w, v)
}

const ROAD_EW: &[[u32; 2]] = &[[60, 500], [960, 500]];

#[test]
fn truck_on_a_straight_road_reaches_wheeled_pct_and_off_road_100() {
    let s = setup(&[(ROAD_EW, 8)]);
    let top = top_speed(&s, TRUCK_ID);
    let pct = s.rules.vehicles.road.wheeled_pct as i32;
    assert_eq!(pct, 175, "the shipped wheeled_pct");
    let want = top.mul_int(pct).div_int(100);
    let (on, _, _) = drive_east(s, TRUCK, 30 * 50, 150, 850);
    assert!(
        on >= want - Fx::from_milli(50) && on <= want + Fx::from_milli(1),
        "road top {} want {}",
        on.to_f32(),
        want.to_f32()
    );
    // Control: the same drive without a road is at 100 %.
    let (off, _, _) = drive_east(setup(&[]), TRUCK, 30 * 50, 150, 850);
    assert!(
        off >= top - Fx::from_milli(50) && off <= top + Fx::from_milli(1),
        "off-road top {} want {}",
        off.to_f32(),
        top.to_f32()
    );
}

#[test]
fn per_vehicle_road_speed_pct_overrides_the_drive_type() {
    let mut s = setup(&[(ROAD_EW, 8)]);
    let k = s.vehicles.iter().position(|v| v.id == TRUCK_ID).unwrap();
    s.vehicles[k].mobility.road_speed_pct = Some(120);
    let top = top_speed(&s, TRUCK_ID);
    let want = top.mul_int(120).div_int(100);
    let (on, _, _) = drive_east(s, TRUCK, 30 * 50, 150, 850);
    assert!(
        on >= want - Fx::from_milli(50) && on <= want + Fx::from_milli(1),
        "{} want {}",
        on.to_f32(),
        want.to_f32()
    );
}

#[test]
fn woods_on_a_road_do_not_slow_a_truck() {
    // Woods over the road's middle third: on the road the truck keeps its
    // road speed through them.
    let mut s = setup(&[(ROAD_EW, 8)]);
    woods(&mut s, [500, 500], 80);
    let top = top_speed(&s, TRUCK_ID);
    let want = top.mul_int(175).div_int(100);
    let (best, w, _) = drive_east(s, TRUCK, 30 * 50, 470, 530);
    let t = w.terrain();
    let n = |x: u32| {
        let (gx, gy) = t.nearest_grid(xy(x as i32, 500));
        t.flags()[(gy * t.dim() + gx) as usize]
    };
    assert_eq!(n(500), FLAG_WOODS | FLAG_ROAD, "both flags on the node");
    assert!(best >= want - Fx::from_milli(50), "{}", best.to_f32());
    // Control: with no road the woods cap it at woods_speed_pct (its goal
    // inside them, so it cannot drive round).
    let mut s = setup(&[]);
    woods(&mut s, [500, 500], 150);
    let k = s.vehicles.iter().find(|v| v.id == TRUCK_ID).unwrap();
    let cap = top.mul_int(k.mobility.woods_speed_pct as i32).div_int(100);
    let mut w = World::new_match(s, 1).unwrap();
    let (sq, v) = spawn(&mut w, TRUCK, xy(150, 500));
    mv(&mut w, sq, xy(520, 500));
    let mut best = Fx::ZERO;
    for _ in 0..30 * 120 {
        w.step(&[]);
        let p = pos(&w, v);
        if p.x >= Fx::from_int(430) && p.x <= Fx::from_int(470) {
            best = best.max(speed(&w, v));
        }
    }
    assert!(best > Fx::ZERO, "the control truck reached the woods");
    assert!(
        best <= cap + Fx::from_milli(1),
        "{} cap {}",
        best.to_f32(),
        cap.to_f32()
    );
}

#[test]
fn enabled_0_restores_cross_country_on_a_road_map() {
    let mut s = setup(&[(ROAD_EW, 8)]);
    s.rules.vehicles.road.enabled = 0;
    let top = top_speed(&s, TRUCK_ID);
    let (on, _, _) = drive_east(s, TRUCK, 30 * 50, 150, 850);
    assert!(
        on <= top + Fx::from_milli(1),
        "{} top {}",
        on.to_f32(),
        top.to_f32()
    );
    assert!(on >= top - Fx::from_milli(50), "{}", on.to_f32());
}

#[test]
fn drive_types_take_their_road_pct() {
    // Half-track 150 %, tracked 140 % (the shipped values).
    for (card, id, pct) in [
        ("ww2:ger_sdkfz251_1_squad", "ww2:ger_sdkfz251_1", 150),
        ("ww2:ger_pz4_h_squad", "ww2:ger_pz4_h", 140),
    ] {
        let s = setup(&[(ROAD_EW, 8)]);
        let want = top_speed(&s, id).mul_int(pct).div_int(100);
        let (on, _, _) = drive_east(s, card, 30 * 60, 150, 850);
        assert!(
            on >= want - Fx::from_milli(50) && on <= want + Fx::from_milli(1),
            "{id}: {} want {}",
            on.to_f32(),
            want.to_f32()
        );
    }
}

#[test]
fn a_truck_stops_on_its_last_road_waypoint() {
    // R1 `top`: braking for the last waypoint uses the road speed, so a
    // truck at 175 % stops at its goal instead of overrunning it.
    let s = setup(&[(ROAD_EW, 8)]);
    let mut w = World::new_match(s, 1).unwrap();
    let (sq, v) = spawn(&mut w, TRUCK, xy(150, 500));
    let goal = xy(600, 500);
    mv(&mut w, sq, goal);
    let mut far = Fx::ZERO;
    let mut fast = Fx::ZERO;
    for _ in 0..30 * 60 {
        w.step(&[]);
        far = far.max(pos(&w, v).x);
        fast = fast.max(speed(&w, v));
    }
    let top = top_speed(w.match_setup().unwrap(), TRUCK_ID);
    assert!(fast > top, "it drove at road speed ({})", fast.to_f32());
    assert!(speed(&w, v).abs() < Fx::from_milli(50), "stopped");
    assert!(
        far <= goal.x + Fx::from_int(2),
        "overran to x {}",
        far.to_f32()
    );
    assert!(
        (pos(&w, v) - goal).length() < Fx::from_int(3),
        "stopped at {:?}",
        pos(&w, v)
    );
}

// ------------------------------------------------------------- R3 path --

fn terrain(s: &MatchSetup) -> Heightfield {
    Heightfield::generate_map(&s.map)
}

fn path_of(
    s: &MatchSetup,
    t: &Heightfield,
    id: &str,
    from: FxVec2,
    to: FxVec2,
) -> (Vec<FxVec2>, u32) {
    let ty = s.vehicles.iter().find(|v| v.id == id).unwrap();
    let mv = &s.rules.movement;
    let cost = PathCost {
        slope_cost_pct: mv.slope_cost_pct,
        max_slope_pct: ty.mobility.max_slope_pct,
        trench_cost_pct: 100,
        ditch_cost_pct: mv.ditch_cost_pct,
    };
    let block = BlockLayer::new(t.dim(), u16::MAX);
    let veh = VehiclePathCost {
        block: &block,
        avoid: None,
        costs: &s.rules.vehicles.path,
        mines: None,
        road_pct: VehiclePathCost::road_for(&s.map, &s.rules.vehicles.road, &ty.mobility),
    };
    let mut scratch = PathScratch::default();
    let r = find_vehicle_path(
        t,
        from,
        to,
        &cost,
        &veh,
        mv.path_max_expansions,
        &mut scratch,
    );
    assert!(r.reached, "{id}: no path");
    let mut way = vec![from];
    way.extend(r.points);
    (way, r.expansions)
}

/// Share (percent) of the path's 2 m samples on road nodes.
fn road_share(t: &Heightfield, way: &[FxVec2]) -> u32 {
    let (mut on, mut all) = (0u32, 0u32);
    for w in way.windows(2) {
        let d = w[1] - w[0];
        let steps = (d.length().raw() >> (Fx::FRAC_BITS + 1)).max(1);
        for k in 0..=steps {
            let p =
                w[0] + FxVec2::new(d.x.mul_int(k).div_int(steps), d.y.mul_int(k).div_int(steps));
            all += 1;
            on += t.road_at(p) as u32;
        }
    }
    on * 100 / all.max(1)
}

#[test]
fn a_path_follows_a_road_when_faster_and_leaves_it_when_not() {
    // A road through woods (x2 off it): 577 m of road against 400 m
    // straight through the trees, so by time (175 %, no woods factor on
    // the road) the road is the cheapest way for a truck. (On open ground
    // the R3 heuristic, never below plain A*, keeps a search near the
    // straight line: its paths are within 1.75x of the cheapest.)
    let (a, b) = (xy(300, 300), xy(700, 300));
    let detour: &[[u32; 2]] = &[[300, 300], [400, 460], [600, 460], [700, 300]];
    let mut s = setup(&[(detour, 8)]);
    woods(&mut s, [500, 350], 350);
    let t = terrain(&s);
    let (way, _) = path_of(&s, &t, TRUCK_ID, a, b);
    assert!(
        road_share(&t, &way) >= 90,
        "on road {} %: {way:?}",
        road_share(&t, &way)
    );
    // Control: with `[vehicles.road] enabled` 0 the same map is driven
    // straight through the woods.
    s.rules.vehicles.road.enabled = 0;
    let (way, _) = path_of(&s, &t, TRUCK_ID, a, b);
    assert!(
        road_share(&t, &way) <= 40,
        "on road {} %: {way:?}",
        road_share(&t, &way)
    );
    // A long detour (1200 m of road) on open ground: straight across is
    // cheaper, and the path leaves the road.
    let s = setup(&[(&[[300, 300], [300, 700], [700, 700], [700, 300]], 8)]);
    let t = terrain(&s);
    let (way, _) = path_of(&s, &t, TRUCK_ID, a, b);
    assert!(
        road_share(&t, &way) <= 20,
        "on road {} %: {way:?}",
        road_share(&t, &way)
    );
}

#[test]
fn road_free_maps_keep_plain_weights_and_road_maps_stay_bounded() {
    // Kessel Ridge as shipped (no roads): `road_for` is None, so searches
    // are the pre-S16 ones.
    let (defs, _) = defs();
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let s = defs.resolve(&req).unwrap();
    assert!(s.map.roads.is_empty());
    let ty = s.vehicles.iter().find(|v| v.id == TRUCK_ID).unwrap();
    assert!(VehiclePathCost::road_for(&s.map, &s.rules.vehicles.road, &ty.mobility).is_none());
    let t = terrain(&s);
    assert!(
        t.flags().iter().all(|f| f & FLAG_ROAD == 0),
        "no road bit on Kessel Ridge"
    );
    // A road map: a cross-map search for every drive type stays under the
    // expansion cap (R3: heuristic weight never below plain A*).
    let s = setup(&[(&[[72, 72], [512, 400], [952, 952]], 6)]);
    let t = terrain(&s);
    for id in [TRUCK_ID, "ww2:ger_sdkfz251_1", "ww2:ger_pz4_h"] {
        let (_, exp) = path_of(&s, &t, id, xy(72, 72), xy(952, 952));
        assert!(exp < s.rules.movement.path_max_expansions, "{id}: {exp}");
    }
}

// ---------------------------------------------------------- symmetry --

fn kessel_point_symmetric() -> MapSetup {
    let (defs, _) = defs();
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let mut m = defs.resolve(&req).unwrap().map;
    m.terrain.point_symmetric = true;
    m
}

#[test]
fn a_blended_point_symmetric_map_has_no_step_on_the_anti_diagonal() {
    let m = kessel_point_symmetric();
    let t = Heightfield::generate_map(&m);
    let dim = t.dim();
    let last = dim - 1;
    for gy in 0..dim {
        for gx in 0..dim {
            assert_eq!(
                t.grid_height(gx, gy),
                t.grid_height(last - gx, last - gy),
                "not point-symmetric at ({gx}, {gy})"
            );
        }
    }
    // Greatest step between neighbours across x + y = size against the
    // greatest anywhere else.
    let (mut across, mut elsewhere) = (0i64, 0i64);
    for gy in 0..dim {
        for gx in 0..dim {
            for (nx, ny) in [(gx + 1, gy), (gx, gy + 1)] {
                if nx > last || ny > last {
                    continue;
                }
                let d =
                    (t.grid_height(gx, gy).raw() as i64 - t.grid_height(nx, ny).raw() as i64).abs();
                let crosses = (gx + gy <= last) != (nx + ny <= last);
                if crosses {
                    across = across.max(d);
                } else {
                    elsewhere = elsewhere.max(d);
                }
            }
        }
    }
    assert!(across > 0, "the test samples the anti-diagonal");
    assert!(
        across <= elsewhere,
        "step {across} across, {elsewhere} elsewhere"
    );
    // Control: without symmetry the field is not point-symmetric.
    let mut m = m;
    m.terrain.point_symmetric = false;
    let t = Heightfield::generate_map(&m);
    assert!((0..dim).any(|g| t.grid_height(g, 3) != t.grid_height(last - g, last - 3)));
}

// ------------------------------------------------ determinism, saves --

fn road_world(threads: usize) -> (World, [SimId; 3]) {
    let mut s = setup(&[(ROAD_EW, 8), (&[[500, 100], [500, 900]], 6)]);
    woods(&mut s, [500, 500], 40);
    let mut w = World::new_match(s, threads).unwrap();
    let (a, va) = spawn(&mut w, TRUCK, xy(150, 500));
    let (b, vb) = spawn(&mut w, "ww2:ger_pz4_h_squad", xy(500, 150));
    let (c, vc) = spawn(&mut w, "ww2:ger_sdkfz251_1_squad", xy(150, 520));
    mv(&mut w, a, xy(850, 500));
    mv(&mut w, b, xy(500, 850));
    mv(&mut w, c, xy(700, 800));
    (w, [va, vb, vc])
}

#[test]
fn road_map_hashes_equal_across_threads_and_save_load() {
    let run = |threads: usize| {
        let (mut w, _) = road_world(threads);
        for _ in 0..30 * 20 {
            w.step(&[]);
        }
        let bytes = w.save();
        for _ in 0..30 * 10 {
            w.step(&[]);
        }
        (w.state_hash(), bytes)
    };
    let (h1, bytes) = run(1);
    for t in [2, 3, 8] {
        assert_eq!(run(t).0, h1, "{t} threads");
    }
    let mut l = World::load(&bytes, 2).unwrap();
    for _ in 0..30 * 10 {
        l.step(&[]);
    }
    assert_eq!(l.state_hash(), h1, "save, load, continue");
    // The road flags survive the save.
    assert!(l.terrain().flags().iter().any(|f| f & FLAG_ROAD != 0));
}

#[test]
fn road_setup_is_hashed_and_checked() {
    let a = setup(&[(ROAD_EW, 8)]);
    let mut b = a.clone();
    b.map.roads[0].half_width = Fixed::from_int(3);
    let wa = World::new_match(a.clone(), 1).unwrap();
    let wb = World::new_match(b, 1).unwrap();
    assert_ne!(
        wa.state_hash(),
        wb.state_hash(),
        "road width reaches the hash"
    );
    for bad in [
        |s: &mut MatchSetup| s.map.roads[0].points.truncate(1),
        |s: &mut MatchSetup| s.map.roads[0].half_width = Fixed::from_int(7),
        |s: &mut MatchSetup| {
            let p = s.map.roads[0].points[0];
            s.map.roads[0].points.insert(0, p);
        },
        |s: &mut MatchSetup| s.map.roads[0].points[1] = FixedVec2::from_m([2000, 500]),
        |s: &mut MatchSetup| s.rules.vehicles.road.wheeled_pct = 201,
        |s: &mut MatchSetup| s.rules.vehicles.road.enabled = 2,
        |s: &mut MatchSetup| {
            s.vehicles[0].mobility.road_speed_pct = Some(99);
        },
    ] {
        let mut s = a.clone();
        bad(&mut s);
        assert!(!s.validate().is_empty(), "accepted a bad road setup");
    }
}
