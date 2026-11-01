//! `frontline-cli ballistics` (S3 "Done when" 2) and mod validation ("Done
//! when" 3): the golden file runs through the sim's own armour functions.

use std::path::{Path, PathBuf};
use std::process::Command;

use frontline_cli::ballistics::{self, Armoury, BallisticsArgs, Descent};
use frontline_cli::validate;
use frontline_content::ArmourZone;

fn ww2() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mods/ww2")
}

fn armoury() -> Armoury {
    Armoury::load(Some(&ww2())).unwrap()
}

/// The spec's 27 golden rows, in order: (ammo, target, zone, range, angle).
const SPEC_ROWS: [(&str, &str, &str, i32, i32); 27] = [
    ("75mm_pzgr39", "usa_m4a3_75", "hull_front", 150, 0),
    ("75mm_pzgr39", "usa_m4a3_75", "hull_front", 300, 0),
    ("75mm_pzgr39", "usa_m4a3_75", "hull_front", 300, 30),
    ("75mm_pzgr39", "usa_m4a3_75", "hull_front", 450, 30),
    ("75mm_pzgr39", "usa_m4a3_75", "hull_front", 1000, 0),
    ("75mm_pzgr39", "usa_m4a3_75", "hull_front", 1000, 0),
    ("75mm_pzgr39", "usa_m4a3_75", "hull_front", 1500, 0),
    ("75mm_pzgr39", "usa_m4a3_75", "hull_front", 300, 65),
    ("75mm_pzgr39", "usa_m4a3_75", "turret_front", 300, 0),
    ("75mm_pzgr39", "usa_m4a3_75", "turret_front", 1500, 0),
    ("75mm_pzgr39", "usa_m4a3_75", "hull_side", 300, 0),
    ("75mm_m61", "ger_pz4_h", "hull_front", 100, 0),
    ("75mm_m61", "ger_pz4_h", "hull_front", 150, 0),
    ("75mm_m61", "ger_pz4_h", "hull_front", 300, 0),
    ("75mm_m61", "ger_pz4_h", "hull_front", 450, 0),
    ("75mm_m61", "ger_pz4_h", "hull_front", 500, 0),
    ("75mm_m61", "ger_pz4_h", "hull_front", 1000, 0),
    ("75mm_m61", "ger_pz4_h", "hull_front", 150, 20),
    ("75mm_m61", "ger_pz4_h", "hull_front", 300, 20),
    ("75mm_m61", "ger_pz4_h", "hull_front", 150, 30),
    ("75mm_m61", "ger_pz4_h", "turret_front", 300, 0),
    ("75mm_m61", "ger_pz4_h", "hull_side", 150, 0),
    ("m6a3_heat", "ger_pz4_h", "hull_front", 100, 0),
    ("m6a3_heat", "ger_pz4_h", "hull_front", 100, 40),
    ("rpzb54_heat", "usa_m4a3_75", "hull_front", 100, 0),
    ("rpzb54_heat", "usa_m4a3_75", "hull_front", 100, 40),
    ("75mm_sprgr34", "usa_m4a3_75", "hull_side", 300, 0),
];

#[test]
fn golden_rows_pass_through_the_sims_functions() {
    let a = armoury();
    let path = ww2().join("tests/ballistics.toml");
    let g = ballistics::golden(&a, &path).unwrap();
    assert!(g.rows >= 35, "only {} golden rows", g.rows);
    let failed: Vec<_> = g.results.iter().filter(|r| !r.pass).collect();
    assert!(failed.is_empty(), "{}", ballistics::golden_text(&g));
    for (i, (ammo, target, zone, range, angle)) in SPEC_ROWS.iter().enumerate() {
        let s = &g.results[i].shot;
        assert_eq!(
            (
                s.ammo.as_str(),
                s.target.as_str(),
                s.zone.as_str(),
                s.range_m,
                s.angle_deg
            ),
            (
                format!("ww2:{ammo}").as_str(),
                format!("ww2:{target}").as_str(),
                *zone,
                *range,
                *angle
            ),
            "row {} is not the spec's",
            i + 1
        );
    }
    // Row 6 uses the aimed descent, row 22 the track band.
    assert!(g.results[5].shot.descent_deg > 0.4);
    assert!(g.results[21].shot.track_mm > 0.0);
}

#[test]
fn one_shot_matches_the_spec_numbers() {
    let a = armoury();
    let pz = a.ammo_index("75mm_pzgr39").unwrap();
    let sherman = a.vehicle_index("ww2:usa_m4a3_75").unwrap();
    let r = ballistics::shot(
        &a,
        pz,
        sherman,
        ArmourZone::HullFront,
        300,
        30,
        Descent::Deg(0),
        false,
    );
    assert!((r.impact_deg - 53.8).abs() < 0.1, "{}", r.impact_deg);
    assert!((r.effective_mm - 107.5).abs() < 0.1, "{}", r.effective_mm);
    assert_eq!(r.result, "penetrates");
    let text = ballistics::shot_text(&r);
    for want in ["63.5 mm at 47°", "53.8°", "107.5 mm", "penetrates (100 %)"] {
        assert!(text.contains(want), "{want:?} not in\n{text}");
    }
    let r = ballistics::shot(
        &a,
        pz,
        sherman,
        ArmourZone::HullFront,
        450,
        30,
        Descent::Deg(0),
        false,
    );
    assert_eq!(r.result, "borderline");
    assert!(ballistics::shot_text(&r).contains("borderline: 92.3 % penetrate"));
    let r = ballistics::shot(
        &a,
        pz,
        sherman,
        ArmourZone::HullFront,
        300,
        65,
        Descent::Deg(0),
        false,
    );
    assert_eq!(r.result, "ricochet");
    // Unknown ids and zones say what exists.
    let e = a.ammo_index("ww2:nope").unwrap_err().to_string();
    assert!(e.contains("ww2:75mm_m61"), "{e}");
    let e = ballistics::parse_zone("hull_frnt").unwrap_err().to_string();
    assert!(e.contains("hull_front"), "{e}");
}

#[test]
fn table_json_and_golden_through_run() {
    let base = BallisticsArgs {
        ammo: Some("ww2:75mm_m61".into()),
        target: Some("ww2:ger_pz4_h".into()),
        descent: "0".into(),
        mods: Some(ww2()),
        ..Default::default()
    };
    let (out, ok) = ballistics::run(&BallisticsArgs {
        table: true,
        ..base.clone()
    })
    .unwrap();
    assert!(ok);
    assert!(
        out.contains("hull_front") && out.contains("1500 m") && out.contains(" R"),
        "{out}"
    );
    let (json, _) = ballistics::run(&BallisticsArgs {
        table: true,
        json: true,
        ..base.clone()
    })
    .unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    // 8 zones × 4 angles, 7 ranges each.
    assert_eq!(v["rows"].as_array().unwrap().len(), 32);
    assert_eq!(v["ranges_m"].as_array().unwrap().len(), 7);
    let (json, ok) = ballistics::run(&BallisticsArgs {
        zone: Some("hull_front".into()),
        range: 300,
        json: true,
        ..base.clone()
    })
    .unwrap();
    assert!(ok);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["result"], "borderline");
    assert!((v["pen_pct"].as_f64().unwrap() - 88.4).abs() < 0.1);
    let (_, ok) = ballistics::run(&BallisticsArgs {
        golden: Some(ww2().join("tests/ballistics.toml")),
        ammo: None,
        target: None,
        ..base
    })
    .unwrap();
    assert!(ok);
}

#[test]
fn the_binary_has_help_and_exit_codes() {
    let bin = env!("CARGO_BIN_EXE_frontline-cli");
    let help = Command::new(bin)
        .args(["ballistics", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
    let text = String::from_utf8_lossy(&help.stdout);
    for flag in [
        "--ammo",
        "--target",
        "--zone",
        "--range",
        "--angle",
        "--descent",
        "--track-band",
        "--table",
        "--golden",
        "--json",
    ] {
        assert!(text.contains(flag), "{flag} not in help:\n{text}");
    }
    let run = Command::new(bin)
        .args(["ballistics", "--mods"])
        .arg(ww2())
        .args([
            "--ammo",
            "ww2:75mm_pzgr39",
            "--target",
            "ww2:usa_m4a3_75",
            "--zone",
            "hull_front",
            "--range",
            "300",
            "--angle",
            "30",
        ])
        .output()
        .unwrap();
    assert!(run.status.success());
    assert!(String::from_utf8_lossy(&run.stdout).contains("penetrates"));
    // A failing golden row exits 1.
    let dir = std::env::temp_dir().join(format!("frontline-golden-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let bad = dir.join("bad.toml");
    std::fs::write(
        &bad,
        "[[row]]\nammo = \"ww2:75mm_m61\"\ntarget = \"ww2:ger_pz4_h\"\nzone = \"hull_front\"\nrange = 1000\nexpect = \"pen\"\n",
    )
    .unwrap();
    let out = Command::new(bin)
        .args(["ballistics", "--mods"])
        .arg(ww2())
        .arg("--golden")
        .arg(&bad)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stdout).contains("FAIL #1"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn validate_checks_the_mod_folder() {
    let r = validate::path(&ww2());
    assert!(r.ok, "{:?}", r.problems);
    // A broken copy: a non-zero drag and a seat outside its frame.
    let dst = std::env::temp_dir().join(format!("frontline-validate-s3-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(&ww2(), &dst);
    let edit = |f: &str, from: &str, to: &str| {
        let p = dst.join(f);
        let s = std::fs::read_to_string(&p).unwrap();
        assert!(s.contains(from));
        std::fs::write(&p, s.replacen(from, to, 1)).unwrap();
    };
    edit(
        "defs/ammo/75mm_m48.toml",
        "drag_coeff = 0",
        "drag_coeff = 0.3",
    );
    edit(
        "defs/vehicles/ger_pz4_h.toml",
        "center = [150, 60, 110]",
        "center = [150, 160, 110]",
    );
    let r = validate::path(&dst);
    assert!(!r.ok);
    let all: Vec<String> = r
        .problems
        .iter()
        .map(|p| format!("{}: {} ({:?})", p.path, p.message, p.fix))
        .collect();
    let has = |path: &str, word: &str| {
        assert!(
            all.iter().any(|p| p.contains(path) && p.contains(word)),
            "{path:?}/{word:?} not in {all:#?}"
        )
    };
    has("defs/ammo/75mm_m48.toml: drag_coeff", "no drag");
    has(
        "defs/vehicles/ger_pz4_h.toml: crew.boxes.driver",
        "outside its frame",
    );
    // With the defs fixed, a typo in a rules override is reported too.
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(&ww2(), &dst);
    edit(
        "tests/rules_test_unlock.toml",
        "unlock_s = 0",
        "unlock_s = 0\nunlok = 1",
    );
    let r = validate::path(&dst);
    assert!(
        r.problems
            .iter()
            .any(|p| p.path.contains("rules_test_unlock.toml") && p.message.contains("unlok")),
        "{:?}",
        r.problems
    );
    let _ = std::fs::remove_dir_all(&dst);
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
