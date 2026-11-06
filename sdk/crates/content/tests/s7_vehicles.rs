//! S7 vehicle schema (`docs/slices/S7-casemate-td.md` 1): the StuG III G
//! and the M10 lower with their armour boxes, zones, casemate arc and open
//! top; broken copies of the live `mods/ww2` give file-qualified errors
//! for each schema rule (casemate xor turret, arc bounds, open faces, seat
//! boxes, turret ring iff turret).

use std::path::{Path, PathBuf};

use frontline_content::vehicle::{MountKind, PlateGroup, VehicleRole};
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

fn replace_in(path: &Path, from: &str, to: &str) {
    let s = std::fs::read_to_string(path).unwrap();
    assert!(s.contains(from), "{} lacks {from:?}", path.display());
    std::fs::write(path, s.replacen(from, to, 1)).unwrap();
}

fn issues_of(name: &str, edit: impl FnOnce(&Path)) -> Vec<String> {
    let root = std::env::temp_dir().join(format!("frontline-s7-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    copy_dir(&live_root(), &root);
    edit(&root);
    let e = ModDefs::load(&root).unwrap_err();
    let _ = std::fs::remove_dir_all(&root);
    match e {
        ContentError::Invalid { issues, .. } => issues.iter().map(|i| i.to_string()).collect(),
        ContentError::Parse { origin, message } => vec![format!("{origin}: {message}")],
        other => panic!("expected validation issues, got {other}"),
    }
}

#[track_caller]
fn has(issues: &[String], path: &str, words: &[&str]) {
    assert!(
        issues
            .iter()
            .any(|i| i.contains(path) && words.iter().all(|w| i.contains(w))),
        "no issue at {path:?} with {words:?} in {issues:#?}"
    );
}

#[test]
fn the_stug_and_the_m10_load_with_the_s7_schema() {
    let d = ModDefs::load(&live_root()).unwrap_or_else(|e| panic!("{e}"));
    let stug = d.vehicles.get("ww2:ger_stug3_g").expect("StuG");
    assert_eq!(stug.role, VehicleRole::AssaultGun);
    assert!(stug.turret.is_none());
    let ids: Vec<&str> = stug.boxes.iter().map(|b| b.id.as_str()).collect();
    assert_eq!(ids, ["hull", "casemate"]);
    let main = stug
        .weapons
        .iter()
        .find(|w| w.mount == MountKind::Casemate)
        .expect("a casemate gun");
    assert_eq!(main.traverse_deg, Some([-10, 10]));
    assert!(stug.seats.iter().all(|s| s.in_box.is_some()));
    let m10 = d.vehicles.get("ww2:usa_m10").expect("M10");
    assert_eq!(m10.role, VehicleRole::TankDestroyer);
    assert!(m10.turret.is_some());
    assert_eq!(m10.boxes[1].open, [PlateGroup::Top]);
    assert!(m10.boxes[0].open.is_empty());
    assert_eq!(m10.seats.len(), 5);
}

#[test]
fn s7_schema_errors_are_reported() {
    let f = "defs/vehicles/ger_stug3_g.toml";
    let i = issues_of("schema", |r| {
        let stug = r.join(f);
        // A turret as well as the casemate gun.
        replace_in(
            &stug,
            "[mobility]",
            "[turret]\npivot = [0, 0, 150]\ntraverse_deg_s = 10\nelevate_deg_s = 4\n\n[mobility]",
        );
        // An arc that leaves out straight ahead.
        replace_in(&stug, "traverse_deg = [-10, 10]", "traverse_deg = [5, 10]");
        // The running-gear hull cannot be open.
        replace_in(
            &stug,
            "id = \"hull\"\nframe = \"hull\"",
            "id = \"hull\"\nframe = \"hull\"\nopen = [\"top\"]",
        );
        // A seat in a box that does not exist.
        replace_in(&stug, "in = \"casemate\"", "in = \"cupola\"");
        let m10 = r.join("defs/vehicles/usa_m10.toml");
        // An open face keeps a plate; the turret ring is gone.
        replace_in(
            &m10,
            "rear = { thickness_mm = 25, slope_deg = 0 }",
            "rear = { thickness_mm = 25, slope_deg = 0 }\ntop = { thickness_mm = 10 }",
        );
        replace_in(&m10, "type = \"turret_ring\"", "type = \"engine\"");
    });
    has(&i, &format!("{f}: turret"), &["turret or a casemate gun"]);
    has(
        &i,
        &format!("{f}: boxes"),
        &["exactly one frame = \"turret\" box"],
    );
    has(&i, &format!("{f}: weapons[0].traverse_deg"), &["[5, 10]"]);
    has(&i, &format!("{f}: boxes[0].open"), &["no open face"]);
    has(
        &i,
        &format!("{f}: seats[1].in"),
        &["no armour box `cupola`"],
    );
    let m = "defs/vehicles/usa_m10.toml";
    has(
        &i,
        &format!("{m}: boxes[1].armor.top"),
        &["an open face has no plate"],
    );
    has(
        &i,
        &format!("{m}: components"),
        &["exactly one `turret_ring`", "got 0"],
    );
}

#[test]
fn a_turret_ring_without_a_turret_is_reported() {
    let f = "defs/vehicles/ger_stug3_g.toml";
    let i = issues_of("ring", |r| {
        let stug = r.join(f);
        let s = std::fs::read_to_string(&stug).unwrap();
        std::fs::write(
            &stug,
            s + "\n[[components]]\ntype = \"turret_ring\"\nhp = 80\n\
                 box = { frame = \"hull\", center = [40, 0, 200], half = [60, 60, 8] }\n",
        )
        .unwrap();
    });
    has(
        &i,
        &format!("{f}: components"),
        &["no `turret_ring`", "got 1"],
    );
}
