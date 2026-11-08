//! S8b vehicle schema (`docs/slices/S8b-trucks-recon.md` 3): wheeled
//! mobility, wheels as components, the grenade screen, optics; broken copies
//! of the live `mods/ww2` give file-qualified errors for each rule.

use std::path::{Path, PathBuf};

use frontline_content::{ContentError, ModDefs};

fn live_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mods/ww2")
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        let to = dst.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &to);
        } else {
            std::fs::copy(e.path(), to).unwrap();
        }
    }
}

fn issues_of(name: &str, file: &str, from: &str, to: &str) -> String {
    let root = std::env::temp_dir().join(format!("frontline-s8b-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    copy_dir(&live_root(), &root);
    let p = root.join(file);
    let s = std::fs::read_to_string(&p).unwrap();
    assert!(s.contains(from), "{} lacks {from:?}", p.display());
    std::fs::write(&p, s.replacen(from, to, 1)).unwrap();
    let e = ModDefs::load(&root).unwrap_err();
    let _ = std::fs::remove_dir_all(&root);
    match e {
        ContentError::Invalid { issues, .. } => issues
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
        ContentError::Parse { origin, message } => format!("{origin}: {message}"),
        other => panic!("expected validation issues, got {other}"),
    }
}

const CAR: &str = "defs/vehicles/ger_sdkfz234_1.toml";
const M8: &str = "defs/vehicles/usa_m8.toml";

#[test]
fn wheeled_needs_a_radius_and_no_turn_rate() {
    let e = issues_of("radius", CAR, "turn_radius_cm = 700", "");
    assert!(e.contains("turn_radius_cm"), "{e}");
    let e = issues_of(
        "turn_rate",
        CAR,
        "turn_radius_cm = 700",
        "turn_radius_cm = 700\nturn_deg_s = 30",
    );
    assert!(e.contains("turn_deg_s"), "{e}");
    let e = issues_of(
        "small_radius",
        CAR,
        "turn_radius_cm = 700",
        "turn_radius_cm = 100",
    );
    assert!(e.contains("turn_radius_cm"), "{e}");
    let e = issues_of(
        "tracked_radius",
        "defs/vehicles/usa_m10.toml",
        "turn_deg_s = 32",
        "turn_deg_s = 32\nturn_radius_cm = 700",
    );
    assert!(e.contains("turn_radius_cm"), "{e}");
}

#[test]
fn wheels_exactly_on_wheeled_vehicles() {
    let e = issues_of(
        "wheels_on_tank",
        "defs/vehicles/usa_m10.toml",
        "type = \"track_left\"",
        "type = \"wheels_front\"",
    );
    assert!(
        e.contains("track_left") || e.contains("wheels_front"),
        "{e}"
    );
    let e = issues_of(
        "tracks_on_car",
        CAR,
        "type = \"wheels_front\"",
        "type = \"track_left\"",
    );
    assert!(
        e.contains("track_left") || e.contains("wheels_front"),
        "{e}"
    );
}

#[test]
fn a_grenade_screen_only_on_an_open_face() {
    let e = issues_of(
        "screen_closed",
        CAR,
        "grenade_screen = [\"top\"]",
        "grenade_screen = [\"front\"]",
    );
    assert!(e.contains("grenade_screen"), "{e}");
}

#[test]
fn optics_have_a_range() {
    let e = issues_of(
        "optics",
        M8,
        "stationary_sight_pct = 140",
        "stationary_sight_pct = 50",
    );
    assert!(e.contains("stationary_sight_pct"), "{e}");
}
