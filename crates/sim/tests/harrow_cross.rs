//! S17b Build B (`docs/slices/S17-team-play.md` 5, 9): Harrow Cross
//! (`ww2:harrow_cross`), the four-base team map. Its hand-placed items
//! mirror through the centre (base i onto base (i + 2) % 4), its terrain is
//! exactly point-symmetric, its sides are two of two, and a vehicle path
//! search from each base reaches the farthest point and the mirror base for
//! every drive type.

use std::path::Path;

use frontline_content::vehicle::MobilityKind;
use frontline_content::{Difficulty, MapFileDef, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::path::{PathCost, PathScratch, VehiclePathCost, find_vehicle_path};
use frontline_sim::{BlockLayer, FxVec2, Heightfield};

const MAP: &str = "ww2:harrow_cross";

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2")
}

fn defs() -> ModDefs {
    ModDefs::load(&root()).unwrap_or_else(|e| panic!("{e}"))
}

fn map_file() -> MapFileDef {
    defs().maps[MAP].clone()
}

/// The 2v2 on Harrow Cross: players 0, 1 (side 0) against 2, 3 (side 1).
fn setup_2v2() -> MatchSetup {
    let mut req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    req.map = MAP.into();
    let p = req.players.clone();
    req.players = vec![p[0].clone(), p[1].clone(), p[0].clone(), p[1].clone()];
    for (i, p) in req.players.iter_mut().enumerate() {
        p.team = Some(i as u8 / 2);
    }
    defs().resolve(&req).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn harrow_cross_mirrors_every_item_through_the_centre() {
    let m = map_file();
    assert_eq!((m.id.as_str(), m.size_m), (MAP, 1536));
    assert!(m.validate().is_empty(), "{:?}", m.validate());
    assert!(m.mirror_warnings().is_empty(), "{:?}", m.mirror_warnings());
    assert!(m.side_warnings().is_empty(), "{:?}", m.side_warnings());
    let s = m.size_m;
    let mirror = |p: [u32; 2]| [s - p[0], s - p[1]];
    let ends = |a: [u32; 2], b: [u32; 2], f: [u32; 2], t: [u32; 2]| {
        (f, t) == (mirror(a), mirror(b)) || (f, t) == (mirror(b), mirror(a))
    };
    for r in &m.terrain.ridges {
        assert!(
            m.terrain
                .ridges
                .iter()
                .any(|q| ends(r.from, r.to, q.from, q.to)
                    && (q.height_m, q.width_m) == (r.height_m, r.width_m)),
            "ridge {r:?} has no mirror"
        );
    }
    for v in &m.terrain.valleys {
        assert!(
            m.terrain
                .valleys
                .iter()
                .any(|q| ends(v.from, v.to, q.from, q.to)
                    && (q.depth_m, q.width_m) == (v.depth_m, v.width_m)),
            "valley {v:?} has no mirror"
        );
    }
    for w in &m.woods {
        assert!(
            m.woods
                .iter()
                .any(|q| q.center == mirror(w.center) && q.radius_m == w.radius_m),
            "woods {w:?}"
        );
    }
    // Plan 5: four bases, base i mirrors base (i + 2) % 4 on the other
    // side; seven points, D (worth 2) at the centre.
    assert_eq!(m.bases.len(), 4);
    for (i, b) in m.bases.iter().enumerate() {
        let q = &m.bases[(i + 2) % 4];
        assert_eq!(q.pos, mirror(b.pos), "base {i}");
        assert_eq!(q.radius_m, b.radius_m);
        assert_eq!(b.side, Some(i as u8 / 2), "base {i}");
    }
    assert_eq!(m.capture_points.len(), 7);
    let d = m.capture_points.iter().find(|c| c.name == "D").unwrap();
    assert_eq!((d.pos, d.value), ([s / 2, s / 2], 2));
    // Control: moving one point 3 m off its mirror is warned about.
    let mut bad = m.clone();
    bad.capture_points[0].pos[0] += 3;
    assert_eq!(bad.mirror_warnings().len(), 2, "the point and its partner");
}

#[test]
fn harrow_cross_terrain_is_point_symmetric() {
    let s = setup_2v2();
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

#[test]
fn harrow_cross_resolves_a_2v2_with_each_player_on_his_base() {
    let s = setup_2v2();
    let team: Vec<u8> = s.players.iter().map(|p| p.team).collect();
    let base: Vec<u8> = s.players.iter().map(|p| p.base).collect();
    assert_eq!(team, [0, 0, 1, 1]);
    assert_eq!(base, [0, 1, 2, 3]);
    let w = frontline_sim::World::new_match(s, 1).unwrap();
    let g = w.game().unwrap();
    // A1: each AI's foe is the hostile base nearest his own: across the
    // map on his own edge row (0 faces 3, 1 faces 2).
    let foe: Vec<u8> = (0..4)
        .map(|p| g.main_foe(frontline_sim::PlayerId(p)).unwrap().0)
        .collect();
    assert_eq!(foe, [3, 2, 1, 0]);
}

#[test]
fn every_drive_type_reaches_the_farthest_point_and_the_mirror_base_under_the_cap() {
    let s = setup_2v2();
    let t = Heightfield::generate_map(&s.map);
    let mv = &s.rules.movement;
    let cap = mv.path_max_expansions;
    let block = BlockLayer::new(t.dim(), u16::MAX);
    let mut kinds = std::collections::BTreeSet::new();
    for (b, base) in s.map.bases.iter().enumerate() {
        let from: FxVec2 = base.center.into();
        let far_point: FxVec2 = s
            .map
            .points
            .iter()
            .map(|c| FxVec2::from(c.pos))
            .max_by_key(|p| (*p - from).length().raw())
            .unwrap();
        let far_base: FxVec2 = s.map.bases[(b + 2) % 4].center.into();
        for v in &s.vehicles {
            if v.mobility.kind == MobilityKind::Towed {
                continue;
            }
            kinds.insert(format!("{:?}", v.mobility.kind));
            let cost = PathCost {
                slope_cost_pct: mv.slope_cost_pct,
                max_slope_pct: v.mobility.max_slope_pct,
                trench_cost_pct: 100,
                ditch_cost_pct: mv.ditch_cost_pct,
            };
            let veh = VehiclePathCost {
                block: &block,
                avoid: None,
                costs: &s.rules.vehicles.path,
                mines: None,
                road_pct: VehiclePathCost::road_for(&s.map, &s.rules.vehicles.road, &v.mobility),
            };
            for to in [far_point, far_base] {
                let mut scratch = PathScratch::default();
                let r = find_vehicle_path(&t, from, to, &cost, &veh, cap, &mut scratch);
                assert!(
                    r.reached && r.expansions < cap,
                    "{} base {b}: {}",
                    v.id,
                    r.expansions
                );
            }
        }
    }
    assert_eq!(kinds.len(), 3, "wheeled, half-track and tracked: {kinds:?}");
}
