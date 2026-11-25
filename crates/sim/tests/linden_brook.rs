//! S16b Build B (`docs/slices/S16-second-map.md` 5, 9): Linden Brook
//! (`ww2:linden_brook`). Its hand-placed items mirror through the centre
//! (R5), its terrain is exactly point-symmetric, a vehicle path search
//! from each base to the farthest point reaches its goal under
//! `path_max_expansions` for every drive type, and trucks drive its roads
//! (finding F1).

use std::path::Path;

use frontline_content::vehicle::MobilityKind;
use frontline_content::{Difficulty, MapFileDef, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::path::{PathCost, PathScratch, VehiclePathCost, find_vehicle_path};
use frontline_sim::{BlockLayer, Fx, FxVec2, Heightfield};

const MAP: &str = "ww2:linden_brook";

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2")
}

fn map_file() -> MapFileDef {
    let defs = ModDefs::load(&root()).unwrap_or_else(|e| panic!("{e}"));
    defs.maps[MAP].clone()
}

fn setup(f0: &str, f1: &str) -> MatchSetup {
    let defs = ModDefs::load(&root()).unwrap_or_else(|e| panic!("{e}"));
    let mut req = MatchRequest::skirmish(1, f0, f1, Difficulty::Normal);
    req.map = MAP.into();
    defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn linden_brook_mirrors_every_item_through_the_centre() {
    let m = map_file();
    assert_eq!(m.id, MAP);
    assert!(m.validate().is_empty(), "{:?}", m.validate());
    assert!(
        m.mirror_warnings().is_empty(),
        "R5: {:?}",
        m.mirror_warnings()
    );
    // The bands are listed with their mirrors too (the map reads the same
    // with symmetry off): each ridge and valley has a partner whose ends
    // mirror its ends (a band reads the same in either direction).
    let s = m.size_m;
    let mirror = |p: [u32; 2]| [s - p[0], s - p[1]];
    let ends = |a: [u32; 2], b: [u32; 2], f: [u32; 2], t: [u32; 2]| {
        (f, t) == (mirror(a), mirror(b)) || (f, t) == (mirror(b), mirror(a))
    };
    for r in &m.terrain.ridges {
        assert!(
            m.terrain.ridges.iter().any(|q| ends(r.from, r.to, q.from, q.to)
                && (q.height_m, q.width_m) == (r.height_m, r.width_m)),
            "ridge {r:?} has no mirror"
        );
    }
    for v in &m.terrain.valleys {
        assert!(
            m.terrain.valleys.iter().any(|q| ends(v.from, v.to, q.from, q.to)
                && (q.depth_m, q.width_m) == (v.depth_m, v.width_m)),
            "valley {v:?} has no mirror"
        );
    }
    // Woods keep their radius in the mirror; C (worth 2) is the centre.
    for w in &m.woods {
        assert!(
            m.woods
                .iter()
                .any(|q| q.center == mirror(w.center) && q.radius_m == w.radius_m),
            "woods {w:?}"
        );
    }
    let c = m.capture_points.iter().find(|c| c.name == "C").unwrap();
    assert_eq!((c.pos, c.value), ([s / 2, s / 2], 2));
    // Control: moving one point 3 m off its mirror is warned about.
    let mut bad = m.clone();
    bad.capture_points[0].pos[0] += 3;
    assert_eq!(bad.mirror_warnings().len(), 2, "the point and its partner");
}

#[test]
fn linden_brook_terrain_is_point_symmetric() {
    let s = setup("ww2:germany", "ww2:usa");
    assert!(s.map.terrain.point_symmetric);
    let t = Heightfield::generate_map(&s.map);
    let last = t.dim() - 1;
    for gy in 0..t.dim() {
        for gx in 0..t.dim() {
            let k = (gy * t.dim() + gx) as usize;
            let m = ((last - gy) * t.dim() + (last - gx)) as usize;
            assert_eq!(
                t.grid_height(gx, gy),
                t.grid_height(last - gx, last - gy),
                "height at ({gx}, {gy})"
            );
            assert_eq!(t.flags()[k], t.flags()[m], "flags at ({gx}, {gy})");
        }
    }
}

fn path(
    s: &MatchSetup,
    t: &Heightfield,
    mobility: &frontline_content::vehicle::MobilitySetup,
    from: FxVec2,
    to: FxVec2,
) -> (bool, u32, Vec<FxVec2>) {
    let mv = &s.rules.movement;
    let cost = PathCost {
        slope_cost_pct: mv.slope_cost_pct,
        max_slope_pct: mobility.max_slope_pct,
        trench_cost_pct: 100,
        ditch_cost_pct: mv.ditch_cost_pct,
    };
    let block = BlockLayer::new(t.dim(), u16::MAX);
    let veh = VehiclePathCost {
        block: &block,
        avoid: None,
        costs: &s.rules.vehicles.path,
        mines: None,
        road_pct: VehiclePathCost::road_for(&s.map, &s.rules.vehicles.road, mobility),
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
    let mut way = vec![from];
    way.extend(r.points);
    (r.reached, r.expansions, way)
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

fn at(p: frontline_content::setup::FixedVec2) -> FxVec2 {
    p.into()
}

#[test]
fn every_drive_type_reaches_the_farthest_point_from_each_base_under_the_cap() {
    // Plan 9 "S16a", deferred to S16b: base to the farthest point on
    // Linden Brook, each drive type, under `path_max_expansions` (8000).
    // Every self-propelled vehicle of both factions (their slope limits
    // differ); the targets are the farthest capture point and the far base.
    let s = setup("ww2:germany", "ww2:usa");
    let t = Heightfield::generate_map(&s.map);
    let cap = s.rules.movement.path_max_expansions;
    assert_eq!(cap, 8000);
    let mut kinds = std::collections::BTreeSet::new();
    for (b, base) in s.map.bases.iter().enumerate() {
        let from = at(base.center);
        let far_point = s
            .map
            .points
            .iter()
            .map(|c| at(c.pos))
            .max_by_key(|p| (*p - from).length().raw())
            .unwrap();
        let far_base = at(s.map.bases[1 - b].center);
        for v in &s.vehicles {
            if v.mobility.kind == MobilityKind::Towed {
                continue;
            }
            kinds.insert(format!("{:?}", v.mobility.kind));
            for to in [far_point, far_base] {
                let (reached, exp, _) = path(&s, &t, &v.mobility, from, to);
                assert!(reached && exp < cap, "{} base {b}: {exp}", v.id);
            }
        }
    }
    assert_eq!(kinds.len(), 3, "wheeled, half-track and tracked: {kinds:?}");
}

/// Drives a German truck (no AI) from just inside base 0 to `goal`; the
/// metres it drove and the share of them on road nodes (hull centre).
fn drive_truck(s: &MatchSetup, goal: [i32; 2]) -> (f64, u32) {
    use frontline_sim::{CommandKind, PlayerId, World};
    let mut w = World::new_match(s.clone(), 1).unwrap();
    let card = w
        .match_setup()
        .unwrap()
        .squad_type_index("ww2:ger_opel_blitz_squad")
        .unwrap();
    let east = FxVec2::new(Fx::ONE, Fx::ZERO);
    let sq = w
        .spawn_squad_at(PlayerId(0), card, FxVec2::from_int(80, 300), east)
        .unwrap();
    let v = w.game().unwrap().squad(sq).unwrap().members[0];
    let target = FxVec2::from_int(goal[0], goal[1]);
    let seq = w.tick() as u32;
    w.step(&[frontline_sim::scenarios::cmd(
        0,
        seq,
        CommandKind::Move { squad: sq, target },
    )]);
    let (mut on, mut all) = (Fx::ZERO, Fx::ZERO);
    for _ in 0..30 * 240 {
        w.step(&[]);
        let m = w.game().unwrap().vehicle(v).unwrap().speed.abs().div_int(30);
        all += m;
        if w.terrain().road_at(w.entities().transform()[v.index()].pos.xy()) {
            on += m;
        }
    }
    let pct = (on.to_f32() * 100.0 / all.to_f32().max(1e-3)) as u32;
    (all.to_f32() as f64, pct)
}

#[test]
fn a_truck_drives_the_roads_from_its_base_to_the_points() {
    // Finding F1 (S16 log): the R3 heuristic keeps a search within 1.75x
    // of the cheapest path and can leave a bent road on open ground. On
    // Linden Brook the roads run from the bases to the points, and a truck
    // ordered there drives them (94-96 % of its metres on road nodes when
    // this test was written; C over B, D over B and C, E over (300, 200)).
    let mut s = setup("ww2:germany", "ww2:usa");
    s.players.iter_mut().for_each(|p| p.is_ai = false);
    for goal in [[512, 512], [764, 604], [512, 164]] {
        let (m, pct) = drive_truck(&s, goal);
        assert!(m > 400.0, "{goal:?}: drove {m} m");
        assert!(pct >= 85, "{goal:?}: {pct} % on road");
    }
    // Control: with `[vehicles.road] enabled` 0 the truck drives straight
    // to C across the fields.
    s.rules.vehicles.road.enabled = 0;
    let (_, pct) = drive_truck(&s, [512, 512]);
    assert!(pct <= 30, "{pct} % on road with roads off");
}
