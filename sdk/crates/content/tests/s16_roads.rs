//! S16a Build A (`docs/slices/S16-second-map.md` 2-4, R5): `[[roads]]` in
//! map files, `[vehicles.road]`, `[mobility] road_speed_pct` and the
//! checks `frontline-cli validate` runs on them. Kessel Ridge loads
//! unchanged (no roads, no symmetry, no mirror warnings).

use std::path::Path;

use frontline_content::defs::{RoadDef, TerrainSymmetry};
use frontline_content::{Difficulty, MapFileDef, MatchRequest, ModDefs};

fn ww2() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mods/ww2")
}

fn kessel() -> MapFileDef {
    let src = std::fs::read_to_string(ww2().join("defs/maps/kessel_ridge.toml")).unwrap();
    toml::from_str(&src).unwrap()
}

fn issue_paths(m: &MapFileDef) -> Vec<String> {
    m.validate().into_iter().map(|i| i.path).collect()
}

#[test]
fn kessel_ridge_loads_unchanged_with_no_roads() {
    let m = kessel();
    assert!(m.roads.is_empty());
    assert!(m.validate().is_empty());
    assert!(
        m.mirror_warnings().is_empty(),
        "no symmetry: no mirror check"
    );
    let defs = ModDefs::load(&ww2()).unwrap_or_else(|e| panic!("{e}"));
    let s = defs
        .resolve(&MatchRequest::skirmish(
            1,
            "ww2:germany",
            "ww2:usa",
            Difficulty::Normal,
        ))
        .unwrap();
    assert!(s.map.roads.is_empty());
    // The shipped `[vehicles.road]` (every value in the rules TOML).
    let r = &s.rules.vehicles.road;
    assert_eq!(
        (r.enabled, r.wheeled_pct, r.halftrack_pct, r.tracked_pct),
        (1, 175, 150, 140)
    );
    assert_eq!((r.reverse_pct, r.path_cost, r.heuristic_pct), (100, 1, 100));
    assert!(
        s.vehicles
            .iter()
            .all(|v| v.mobility.road_speed_pct.is_none())
    );
}

#[test]
fn a_road_parses_and_resolves_to_half_width() {
    let mut src = std::fs::read_to_string(ww2().join("defs/maps/kessel_ridge.toml")).unwrap();
    src.push_str("\n[[roads]]\npoints = [[72, 72], [512, 512], [952, 952]]\nwidth_m = 6\n");
    let m: MapFileDef = toml::from_str(&src).unwrap();
    assert_eq!(m.roads.len(), 1);
    assert!(m.validate().is_empty(), "{:?}", m.validate());
    // An unknown key in a road is refused.
    let bad = src.replace("width_m = 6", "width_m = 6\nlanes = 2");
    assert!(toml::from_str::<MapFileDef>(&bad).is_err());
}

#[test]
fn validation_rejects_each_r5_case() {
    let road = |points: Vec<[u32; 2]>, width_m: u32| RoadDef { points, width_m };
    let cases: [(RoadDef, &str); 6] = [
        (road(vec![[10, 10]], 6), "roads[0].points"),
        (road(vec![[10, 10], [1100, 10]], 6), "roads[0].points[1]"),
        (
            road(vec![[10, 10], [10, 10], [50, 50]], 6),
            "roads[0].points[1]",
        ),
        (road(vec![[10, 10], [50, 50]], 3), "roads[0].width_m"),
        (road(vec![[10, 10], [50, 50]], 13), "roads[0].width_m"),
        (
            road((0..65).map(|k| [k * 10, 10]).collect(), 6),
            "roads[0].points",
        ),
    ];
    for (r, path) in cases {
        let mut m = kessel();
        m.roads = vec![r];
        let got = issue_paths(&m);
        assert!(got.iter().any(|p| p == path), "{path}: {got:?}");
    }
    // Control: widths 4 and 12 and two points are fine.
    for w in [4, 12] {
        let mut m = kessel();
        m.roads = vec![road(vec![[10, 10], [50, 50]], w)];
        assert!(m.validate().is_empty(), "{w}: {:?}", m.validate());
    }
}

#[test]
fn point_symmetric_maps_warn_on_unmirrored_items() {
    let mut m = kessel();
    m.terrain.symmetry = TerrainSymmetry::Point;
    // Kessel Ridge's points are not hand-mirrored: warnings (not errors).
    let w = m.mirror_warnings();
    assert!(!w.is_empty());
    assert!(m.validate().is_empty(), "warnings never fail validation");
    // A fully mirrored layout: no warning. Within 2 m is a mirror.
    let s = m.size_m;
    m.capture_points.truncate(1);
    m.capture_points[0].pos = [s / 2, s / 2];
    m.bases[0].pos = [100, 300];
    m.bases[1].pos = [s - 100, s - 301];
    m.woods = Vec::new();
    m.roads = vec![
        RoadDef {
            points: vec![[100, 300], [s / 2, s / 2], [s - 100, s - 300]],
            width_m: 6,
        },
        RoadDef {
            points: vec![[200, 100], [300, 200]],
            width_m: 6,
        },
        RoadDef {
            points: vec![[s - 300, s - 202], [s - 200, s - 100]],
            width_m: 6,
        },
    ];
    assert!(m.mirror_warnings().is_empty(), "{:?}", m.mirror_warnings());
    // A road point 3 m off its mirror warns, and names it.
    m.roads[2].points[0] = [s - 300, s - 203];
    let w: Vec<_> = m.mirror_warnings().into_iter().map(|i| i.path).collect();
    assert!(w.contains(&"roads[1].points[1]".to_string()), "{w:?}");
    assert!(w.contains(&"roads[2].points[0]".to_string()), "{w:?}");
}

#[test]
fn road_tuning_and_road_speed_pct_are_range_checked() {
    let root = ww2();
    let dir = std::env::temp_dir().join(format!("s16_roads_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (patch, path) in [
        (
            "[vehicles.road]\nwheeled_pct = 99\n",
            "vehicles.road.wheeled_pct",
        ),
        (
            "[vehicles.road]\ntracked_pct = 201\n",
            "vehicles.road.tracked_pct",
        ),
        ("[vehicles.road]\nenabled = 2\n", "vehicles.road.enabled"),
        (
            "[vehicles.road]\npath_cost = 2\n",
            "vehicles.road.path_cost",
        ),
        (
            "[vehicles.road]\nheuristic_pct = 0\n",
            "vehicles.road.heuristic_pct",
        ),
        (
            "[defs.vehicles.\"ww2:ger_opel_blitz\".mobility]\nroad_speed_pct = 250\n",
            "road_speed_pct",
        ),
        (
            "[defs.vehicles.\"ww2:ger_pak40\".mobility]\nroad_speed_pct = 150\n",
            "road_speed_pct",
        ),
    ] {
        let f = dir.join("o.toml");
        std::fs::write(&f, patch).unwrap();
        let defs = ModDefs::load(&root).unwrap();
        let e = defs
            .clone()
            .apply_override(&f, "ww2:skirmish")
            .err()
            .unwrap_or_else(|| panic!("accepted {patch}"));
        assert!(e.to_string().contains(path), "{patch}: {e}");
    }
    // Control: an in-range patch is accepted.
    let f = dir.join("o.toml");
    std::fs::write(&f, "[vehicles.road]\nwheeled_pct = 200\nenabled = 0\n").unwrap();
    let defs = ModDefs::load(&root).unwrap();
    defs.clone().apply_override(&f, "ww2:skirmish").unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
