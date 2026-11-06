//! S3 content: vehicle and ammo defs, AT teams, dig profiles and the rules
//! test override load, lower and validate; broken copies give actionable,
//! file-qualified errors (S3 "Done when" 3).

use std::path::{Path, PathBuf};

use frontline_content::setup::{DigProfile, Fixed};
use frontline_content::vehicle::{AmmoMechanism, ComponentKind, MountKind, SeatRole};
use frontline_content::{ContentError, Difficulty, MatchRequest, ModDefs};

/// S4: the S1-S3c content frozen before the content switch
/// (`fixtures/ww2_s3c`): these tests check its legacy unit files, rules and
/// error reports value by value. The live `mods/ww2` is checked by
/// `validate`, the canonical snapshot (`resolved_defs.rs`) and
/// `variants.rs`.
fn ww2_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ww2_s3c")
}

fn load() -> ModDefs {
    ModDefs::load(&ww2_root()).unwrap_or_else(|e| panic!("{e}"))
}

fn broken_copy(name: &str, edit: impl FnOnce(&Path)) -> PathBuf {
    let dst = std::env::temp_dir().join(format!("frontline-s3-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(&ww2_root(), &dst);
    edit(&dst);
    dst
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

/// Loads a broken copy and returns every issue as `path: message (fix)`.
fn issues_of(name: &str, edit: impl FnOnce(&Path)) -> Vec<String> {
    let root = broken_copy(name, edit);
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

fn skirmish(d: &ModDefs, rules: &str) -> frontline_content::MatchSetup {
    let mut req = MatchRequest::skirmish(3, "ww2:germany", "ww2:usa", Difficulty::Normal);
    req.rules = rules.into();
    d.resolve(&req).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn vehicles_and_ammo_lower_into_the_setup() {
    let d = load();
    let s = skirmish(&d, "ww2:skirmish");
    assert!(s.validate().is_empty(), "{:?}", s.validate());
    assert_eq!(
        s.vehicles.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(),
        ["ww2:ger_pz4_h", "ww2:usa_m4a3_75"]
    );
    let pz = &s.vehicles[0];
    let sh = &s.vehicles[1];
    assert_eq!(s.factions[pz.faction as usize].id, "ww2:germany");
    // S3b: the owner's 120 s re-call cooldown (2026-11-01), no start
    // cooldown (the 4:00 vehicle unlock applies).
    assert_eq!(
        (
            pz.cost,
            pz.pop,
            pz.recall_cooldown_ticks,
            pz.initial_cooldown_ticks
        ),
        (620, 12, 120 * 30, 0) // S3b Fix: cost 640 -> 620
    );
    assert_eq!((sh.cost, sh.pop), (590, 12)); // S3b Fix: 560 -> 590
    assert_eq!(pz.mobility.speed, Fixed::from_cm(600));
    assert_eq!(sh.mobility.reverse, Fixed::from_cm(250));
    assert_eq!(pz.mobility.ditch_block, Fixed::from_ratio(19, 10));
    assert_eq!(pz.mobility.unbog_ticks, [600, 1200]);
    assert_eq!((pz.traverse_deg_s, sh.traverse_deg_s), (14, 24));
    // Armour by zone; the Sherman glacis keeps its half millimetre. S7:
    // zone id = box × 4 + group, the S3 numbers and names (S7-9).
    let plate = |v: &frontline_content::vehicle::VehicleSetup, z: &str| {
        let p = v.plate(v.zone_by_name(z).unwrap()).unwrap();
        (p.thickness, p.slope_deg)
    };
    assert_eq!(plate(pz, "hull_front"), (Fixed::from_int(80), 10));
    assert_eq!(plate(pz, "turret_side"), (Fixed::from_int(30), 25));
    assert_eq!(plate(sh, "hull_front"), (Fixed::from_ratio(635, 10), 47));
    assert_eq!(plate(sh, "turret_front"), (Fixed::from_int(76), 30));
    assert_eq!(plate(sh, "turret_top"), (Fixed::from_int(25), 0));
    for v in [pz, sh] {
        assert_eq!(
            (0..8).map(|z| v.zone_name(z)).collect::<Vec<_>>(),
            [
                "hull_front",
                "hull_side",
                "hull_rear",
                "hull_top",
                "turret_front",
                "turret_side",
                "turret_rear",
                "turret_top"
            ]
        );
        assert!(v.has_turret && v.gun_arc_deg.is_none());
        assert_eq!(v.upper_box(), 1);
        assert_eq!(v.boxes[0].bounds, v.hull);
        assert_eq!(v.boxes[1].bounds, v.turret);
        assert!(v.boxes.iter().all(|b| b.open == 0));
    }
    // Five seats in def order; six components in snapshot order.
    assert_eq!(pz.seats.len(), 5);
    assert!(pz.seat(SeatRole::RadioMg).is_some());
    assert_eq!(
        pz.components.iter().map(|c| c.kind).collect::<Vec<_>>(),
        ComponentKind::ALL
    );
    let track = &pz.components[ComponentKind::TrackLeft.index()];
    assert_eq!(track.anchors.len(), 3);
    assert!(track.parts.is_empty());
    assert_eq!(track.repair.as_ref().unwrap().ticks, 30 * 30);
    assert!(pz.components[ComponentKind::Gun.index()].repair.is_none());
    // Main gun first, penetrator before HE.
    let main = pz.main_gun().unwrap();
    assert_eq!(main.kind, MountKind::TurretMain);
    assert_eq!(s.weapons[main.weapon as usize].id, "ww2:kwk40_l48");
    assert_eq!(main.elevation_deg, [-8, 20]);
    let rounds: Vec<_> = main
        .ammo
        .iter()
        .map(|l| (s.ammo[l.ammo as usize].id.as_str(), l.count))
        .collect();
    assert_eq!(
        rounds,
        [
            ("ww2:75mm_pzgr39", 30),
            ("ww2:75mm_sprgr34", 30),
            ("ww2:75mm_pzgr40", 6)
        ],
        "standard penetrator, HE, then the special one (S3b)"
    );
    let hull_mg = pz
        .mounts
        .iter()
        .find(|m| m.kind == MountKind::Hull)
        .unwrap();
    assert_eq!((hull_mg.arc_deg, hull_mg.seat), (15, SeatRole::RadioMg));
    // The cannon's spread is in microradians: 2.5 mrad.
    assert_eq!(
        s.weapons[main.weapon as usize].spread,
        Fixed::from_ratio(25, 10_000)
    );

    // Ammo: the AP table at 0° (S3b Fix: Bird & Livingston, was 30°), HE
    // flat, HEAT flat at 0°.
    let ammo = |id: &str| s.ammo.iter().find(|a| a.id == id).unwrap();
    let pzgr = ammo("ww2:75mm_pzgr39");
    assert_eq!(pzgr.mechanism, AmmoMechanism::Aphe);
    assert_eq!((pzgr.pen_ref_deg, pzgr.ricochet_deg), (0, 70));
    assert_eq!(pzgr.penetration.len(), 5);
    assert_eq!(pzgr.penetration[1].range, Fixed::from_int(500));
    assert_eq!(pzgr.penetration[1].mm, 123);
    assert!(pzgr.filler.is_some() && pzgr.blast.is_none());
    assert_eq!(pzgr.spall.unwrap().rays, 6);
    let he = ammo("ww2:75mm_m48");
    assert_eq!(he.blast.as_ref().unwrap().blast.pen_mm, 10);
    assert!(he.core.is_none());
    let heat = ammo("ww2:rpzb54_heat");
    assert_eq!((heat.penetration[0].mm, heat.pen_ref_deg), (160, 0));
    assert_eq!(heat.blast.as_ref().unwrap().blast.pen_mm, 0);
    assert_eq!(heat.spall.unwrap().rays, 7); // S3b Fix (review M1): 3 -> 7

    // Vehicle squads take their numbers from the vehicle; AT teams carry
    // one launcher and a rifle.
    let ty = |id: &str| &s.squad_types[s.squad_type_index(id).unwrap() as usize];
    let tank = ty("ww2:ger_pz4_h_squad");
    assert!(s.type_has_tag(
        s.squad_type_index("ww2:ger_pz4_h_squad").unwrap(),
        "vehicle"
    ));
    assert_eq!(tank.vehicle, Some(0));
    assert_eq!((tank.men, tank.cost, tank.pop(&s.vehicles)), (0, 620, 12)); // S3b Fix: 640 -> 620
    let at = ty("ww2:usa_bazooka_team");
    assert!(s.type_has_tag(s.squad_type_index("ww2:usa_bazooka_team").unwrap(), "at"));
    assert_eq!(
        at.members.iter().map(|m| m.count).collect::<Vec<_>>(),
        [1, 1]
    );
    assert_eq!(s.weapons[at.weapon as usize].id, "ww2:m9_bazooka");
    assert_eq!(s.weapons[at.members[1].weapon as usize].id, "ww2:m1_garand");
    assert_eq!(s.weapons[at.small_arms as usize].id, "ww2:m1_garand");
    assert_eq!(at.pop(&s.vehicles), 2);
    let launcher = &s.weapons[at.weapon as usize];
    assert_eq!(s.ammo[launcher.ammo.unwrap() as usize].id, "ww2:m6a3_heat");
    assert_eq!(
        (
            launcher.engage_still,
            launcher.engage_moving,
            launcher.rounds_carried
        ),
        (Fixed::from_int(100), Fixed::from_int(60), 6)
    );
    assert_eq!(
        (launcher.fire_reveal_pct, launcher.fire_reveal_ticks),
        (300, 90)
    );
    // Weapons without their own reveal use the rules' (150 %, 2 s).
    let rifle = s.weapons.iter().find(|w| w.id == "ww2:kar98k").unwrap();
    assert_eq!((rifle.fire_reveal_pct, rifle.fire_reveal_ticks), (150, 60));
    // Every HE block now meets vehicles.
    let mortar = s.weapons.iter().find(|w| w.id == "ww2:grw34").unwrap();
    let mhe = mortar.he.as_ref().unwrap();
    assert_eq!((mhe.pen_mm, mhe.vs_vehicle_pct), (8, 40));
    let satchel = s
        .items
        .iter()
        .find(|i| i.id == "ww2:geballte_ladung")
        .unwrap();
    assert_eq!(satchel.he.as_ref().unwrap().blast.vs_vehicle_pct, 100);

    // Rules: vehicles from 4:00, two dig profiles.
    let r = &s.rules;
    assert_eq!((r.vehicles.unlock_ticks, r.vehicles.limit), (240 * 30, 4));
    assert_eq!(r.vehicles.he_area_spread, Fixed::from_ratio(6, 1000));
    assert_eq!(r.dig.depth, Fixed::from_cm(160));
    assert_eq!(r.dig.wall_run, Fixed::from_cm(40));
    let ditch = r.dig_profile(DigProfile::AtDitch);
    assert_eq!(
        (ditch.depth, ditch.wall_run),
        (Fixed::from_cm(230), Fixed::from_cm(120))
    );
    assert!(ditch.engineers_only && !r.dig.engineers_only);
    assert_eq!(ditch.max_length, Fixed::from_int(30));
    assert_eq!(r.ai.tank_standoff, Fixed::from_int(160));
}

#[test]
fn the_unlock_override_changes_one_rule() {
    let mut d = load();
    let base_hash = d.content_hash();
    let id = d
        .add_rules_override(&ww2_root().join("tests/rules_test_unlock.toml"))
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(id, "ww2:skirmish_test_unlock");
    assert_ne!(
        d.content_hash(),
        base_hash,
        "the override is part of the content"
    );
    let a = skirmish(&d, "ww2:skirmish");
    let b = skirmish(&d, &id);
    assert_eq!(a.rules.vehicles.unlock_ticks, 7200);
    assert_eq!(b.rules.vehicles.unlock_ticks, 0);
    // Everything else is the base's.
    let mut b_rules = b.rules.clone();
    b_rules.vehicles.unlock_ticks = a.rules.vehicles.unlock_ticks;
    assert_eq!(b_rules, a.rules);
    // Loading it twice is an error (the id exists).
    assert!(
        d.add_rules_override(&ww2_root().join("tests/rules_test_unlock.toml"))
            .is_err()
    );
}

#[test]
fn bad_overrides_are_reported() {
    let dir = std::env::temp_dir().join(format!("frontline-s3-override-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let write = |name: &str, body: &str| {
        let p = dir.join(name);
        std::fs::write(&p, body).unwrap();
        p
    };
    let mut d = load();
    let e = d
        .add_rules_override(&write("nobase.toml", "id = \"ww2:x\"\nname = \"x\"\n"))
        .unwrap_err()
        .to_string();
    assert!(e.contains("base") && e.contains("fix"), "{e}");
    let e = d
        .add_rules_override(&write(
            "typo.toml",
            "base = \"ww2:skirmish\"\nid = \"ww2:x\"\nname = \"x\"\n[vehicles]\nunlock = 0\n",
        ))
        .unwrap_err()
        .to_string();
    assert!(e.contains("unlock"), "{e}");
    let e = d
        .add_rules_override(&write(
            "range.toml",
            "base = \"ww2:skirmish\"\nid = \"ww2:x\"\nname = \"x\"\n[vehicles]\nlimit = 99\n",
        ))
        .unwrap_err()
        .to_string();
    assert!(e.contains("vehicles.limit"), "{e}");
    let e = d
        .add_rules_override(&write(
            "unknown.toml",
            "base = \"ww2:skirmsh\"\nid = \"ww2:x\"\nname = \"x\"\n",
        ))
        .unwrap_err()
        .to_string();
    assert!(e.contains("did you mean `ww2:skirmish`"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn broken_armour_zone_is_reported() {
    // A misspelled group (the missing `front` and the unknown key).
    let i = issues_of("zone-name", |r| {
        replace_in(
            &r.join("defs/vehicles/ger_pz4_h.toml"),
            "front = { thickness_mm = 80, slope_deg = 10 }",
            "frnt = { thickness_mm = 80, slope_deg = 10 }",
        );
    });
    has(&i, "defs/vehicles/ger_pz4_h.toml", &["frnt"]);
    // A zero plate, a sloped top, and a slope past 80°.
    let i = issues_of("zone-values", |r| {
        let f = r.join("defs/vehicles/usa_m4a3_75.toml");
        replace_in(
            &f,
            "rear = { thickness_mm = 51, slope_deg = 0 }",
            "rear = { thickness_mm = 51.25, slope_deg = 0 }",
        );
    });
    let f = "defs/vehicles/usa_m4a3_75.toml";
    // The quarter millimetre fails at parse time, before the other checks.
    has(&i, f, &["51.25", "one decimal"]);
    let i = issues_of("zone-values2", |r| {
        let f = r.join("defs/vehicles/usa_m4a3_75.toml");
        replace_in(
            &f,
            "side = { thickness_mm = 38, slope_deg = 0 }",
            "side = { thickness_mm = 0, slope_deg = 0 }",
        );
        replace_in(
            &f,
            "top = { thickness_mm = 19 }",
            "top = { thickness_mm = 19, slope_deg = 5 }",
        );
        replace_in(&f, "slope_deg = 47", "slope_deg = 85");
    });
    has(
        &i,
        &format!("{f}: boxes[0].armor.side.thickness_mm"),
        &["1..=500"],
    );
    has(
        &i,
        &format!("{f}: boxes[0].armor.top.slope_deg"),
        &["flat", "fix: remove slope_deg"],
    );
    has(
        &i,
        &format!("{f}: boxes[0].armor.front.slope_deg"),
        &["0..=80", "85"],
    );
}

#[test]
fn seat_box_outside_its_frame_is_reported() {
    let i = issues_of("seat-box", |r| {
        let f = r.join("defs/vehicles/ger_pz4_h.toml");
        // The driver's box pokes out of the hull's front.
        replace_in(&f, "center = [190, 60, 115]", "center = [280, 60, 115]");
        // The gunner sits in the turret frame but below the turret basket
        // (S3b Tune: a turret-frame box may reach down to the hull floor,
        // 136 cm below the pivot, not below it).
        replace_in(&f, "center = [10, 45, 0]", "center = [10, 45, -100]");
        // An engine box in the turret frame lies outside the turret.
        replace_in(
            &f,
            "box = { frame = \"hull\", center = [-200, 0, 100]",
            "box = { frame = \"turret\", center = [-200, 0, 100]",
        );
    });
    let f = "defs/vehicles/ger_pz4_h.toml";
    has(
        &i,
        &format!("{f}: seats[0].box"),
        &["outside its frame", "hull boxes span x -290..290"],
    );
    has(
        &i,
        &format!("{f}: seats[1].box"),
        &["outside its frame", "turret boxes"],
    );
    has(
        &i,
        &format!("{f}: components[0].box"),
        &["outside its frame", "frame = \"hull\""],
    );
}

#[test]
fn vehicle_structure_errors_are_reported() {
    let i = issues_of("structure", |r| {
        let f = r.join("defs/vehicles/usa_m4a3_75.toml");
        replace_in(
            &f,
            "id = \"gunner\"\nrole = \"gunner\"",
            "id = \"gunner\"\nrole = \"loader\"",
        );
        replace_in(&f, "type = \"ammo_rack\"", "type = \"engine\"");
        replace_in(
            &f,
            "def = \"ww2:m1919\"\nmount = \"coax\"",
            "def = \"ww2:m3_75mm\"\nmount = \"coax\"",
        );
        replace_in(&f, "max_slope_pct = 50", "max_slope_pct = 70");
        replace_in(
            &r.join("defs/units/usa_m4a3_75_squad.toml"),
            "vehicle = \"ww2:usa_m4a3_75\"",
            "vehicle = \"ww2:usa_m4a3_75\"\nmen = 5",
        );
    });
    let f = "defs/vehicles/usa_m4a3_75.toml";
    has(&i, &format!("{f}: seats"), &["needs a `gunner` seat"]);
    has(&i, &format!("{f}: seats[3].role"), &["listed twice"]);
    has(
        &i,
        &format!("{f}: components"),
        &["exactly one `ammo_rack`"],
    );
    has(
        &i,
        &format!("{f}: components"),
        &["exactly one `engine`", "got 2"],
    );
    has(
        &i,
        &format!("{f}: mobility.max_slope_pct"),
        &["30° pitch clamp"],
    );
    has(
        &i,
        "vehicle `ww2:usa_m4a3_75`: weapons[1].def",
        &["Mg weapon"],
    );
    has(
        &i,
        "defs/units/usa_m4a3_75_squad.toml: men",
        &["not allowed on a vehicle squad"],
    );
}

#[test]
fn ammo_and_at_team_errors_are_reported() {
    let i = issues_of("ammo", |r| {
        let a = r.join("defs/ammo/75mm_m61.toml");
        replace_in(&a, "[[100, 88], [500, 81]", "[[500, 88], [100, 81]"); // S3b Fix: B&L row
        replace_in(&a, "ricochet_angle_deg = 70", "ricochet_angle_deg = 85");
        replace_in(&a, "[filler]", "[fillr]");
        let h = r.join("defs/ammo/75mm_sprgr34.toml");
        replace_in(&h, "pen_mm = 10", "pen_mm = 12");
        let t = r.join("defs/units/ger_panzerschreck_team.toml");
        replace_in(&t, "sidearm = \"ww2:kar98k\"\n", "");
        let w = r.join("defs/weapons/rpzb54.toml");
        replace_in(&w, "muzzle_velocity_m_s = 110", "muzzle_velocity_m_s = 120");
    });
    // `[fillr]` is an unknown key: a parse error for that file.
    has(&i, "75mm_m61.toml", &["fillr"]);
    has(
        &i,
        "defs/ammo/75mm_sprgr34.toml: he.pen_mm",
        &["must be 10", "fix: pen_mm = 10"],
    );
    has(
        &i,
        "defs/units/ger_panzerschreck_team.toml: sidearm",
        &["need a sidearm"],
    );
    has(
        &i,
        "weapon `ww2:rpzb54`: muzzle_velocity_m_s",
        &["flies at 110", "fix: muzzle_velocity_m_s = 110"],
    );
    let i = issues_of("ammo2", |r| {
        let a = r.join("defs/ammo/75mm_m61.toml");
        replace_in(&a, "[[100, 88], [500, 81]", "[[500, 88], [100, 81]"); // S3b Fix: B&L row
        replace_in(&a, "ricochet_angle_deg = 70", "ricochet_angle_deg = 85");
    });
    has(
        &i,
        "defs/ammo/75mm_m61.toml: penetration[1][0]",
        &["ascending"],
    );
    has(
        &i,
        "defs/ammo/75mm_m61.toml: ricochet_angle_deg",
        &["1..=80"],
    );
}

/// S3b: multi-box components, the barrel box, the filler rays and the
/// fragment keys are checked with actionable messages.
#[test]
fn s3b_damage_geometry_errors_are_reported() {
    let i = issues_of("s3b-geometry", |r| {
        let f = r.join("defs/vehicles/ger_pz4_h.toml");
        // A rack bin poking out of the hull's side.
        replace_in(
            &f,
            "{ frame = \"hull\", center = [-20, 95, 70], half = [55, 30, 25] }",
            "{ frame = \"hull\", center = [-20, 125, 70], half = [55, 30, 25] }",
        );
        // The transmission with both `box` and `boxes`.
        replace_in(
            &f,
            "box = { frame = \"hull\", center = [200, 0, 85], half = [60, 30, 40] }",
            "box = { frame = \"hull\", center = [200, 0, 85], half = [60, 30, 40] }\nboxes = [{ frame = \"hull\", center = [200, 0, 85], half = [60, 30, 40] }]",
        );
        // No barrel on the main gun.
        replace_in(&f, "barrel = { length_cm = 335, radius_cm = 8 }", "");
        // A filler with too many rays.
        replace_in(
            &r.join("defs/ammo/75mm_pzgr39.toml"),
            "rays = 8",
            "rays = 40",
        );
        // S3b Fix: more spall rays than the sim traces (7).
        replace_in(
            &r.join("defs/ammo/75mm_pzgr40.toml"),
            "rays = 6",
            "rays = 8",
        );
        // S3b Fix: MG stowage on the main gun, and an empty MG stowage.
        replace_in(
            &f,
            "muzzle = [450, 0, 45]",
            "muzzle = [450, 0, 45]\nrounds = 100",
        );
        replace_in(&f, "rounds = 1575", "rounds = 0");
        // A fragment band upside down.
        replace_in(
            &r.join("defs/weapons/grw34.toml"),
            "frag_elev_deg = [2, 50]",
            "frag_elev_deg = [50, 2]",
        );
    });
    let f = "defs/vehicles/ger_pz4_h.toml";
    has(
        &i,
        &format!("{f}: components[5].boxes[0]"),
        &["outside its frame", "hull boxes span"],
    );
    has(&i, &format!("{f}: components[6].boxes"), &["not both"]);
    has(
        &i,
        &format!("{f}: weapons[0].barrel"),
        &["needs a barrel box"],
    );
    has(&i, "defs/ammo/75mm_pzgr39.toml: filler.rays", &["0..=16"]);
    has(&i, "defs/ammo/75mm_pzgr40.toml: spall.rays", &["0..=7"]);
    has(
        &i,
        &format!("{f}: weapons[0].rounds"),
        &["only an MG mount"],
    );
    has(&i, &format!("{f}: weapons[1].rounds"), &["1..=20000"]);
    has(
        &i,
        "defs/weapons/grw34.toml: he.frag_elev_deg",
        &["not [min, max]"],
    );
}

/// S3b Fix: the sim tests at most MAX_INNER_BOXES (S8a: 24) seat, passenger
/// slot and component boxes; more is a load error, not a silent cut.
#[test]
fn too_many_inner_boxes_are_reported() {
    let i = issues_of("inner-boxes", |r| {
        // The Panzer IV has 12 seat and component boxes: 13 passenger
        // slots (also refused on a non-transport) make 25.
        let slot = "[[passenger_slots]]\nbox = { center = [0, 0, 100], half = [20, 20, 40] }\n";
        let f = r.join("defs/vehicles/ger_pz4_h.toml");
        let s = std::fs::read_to_string(&f).unwrap();
        std::fs::write(&f, s + &slot.repeat(13)).unwrap();
    });
    has(
        &i,
        "defs/vehicles/ger_pz4_h.toml: components",
        &["at most 24 inner boxes", "got 25"],
    );
}

/// S3b: a main gun may carry a special second penetrator, only with its
/// HE round (indices: standard 0, HE 1, special 2).
#[test]
fn a_special_round_needs_the_he_round() {
    let i = issues_of("special-round", |r| {
        let f = r.join("defs/vehicles/ger_pz4_h.toml");
        replace_in(&f, "{ id = \"ww2:75mm_sprgr34\", count = 30 },", "");
    });
    has(&i, "ammo", &["special armour-piercing"]);
}

#[test]
fn setup_validate_rejects_broken_vehicles_and_ammo() {
    let d = load();
    let s = skirmish(&d, "ww2:skirmish");
    let bad = |edit: &dyn Fn(&mut frontline_content::MatchSetup), want: &str| {
        let mut t = s.clone();
        edit(&mut t);
        let issues = t.validate();
        assert!(
            issues.iter().any(|i| i.to_string().contains(want)),
            "{want:?} not in {issues:?}"
        );
    };
    bad(&|t| t.vehicles[0].seats.clear(), "vehicles.seats");
    bad(
        &|t| t.vehicles[1].seats[0].part.bounds.center.x = Fixed::from_int(9),
        "vehicles.seats",
    );
    bad(
        &|t| t.vehicles[0].mounts[0].weapon = 999,
        "vehicles.mounts.weapon",
    );
    bad(&|t| t.vehicles[0].mounts.swap(0, 1), "main gun not first");
    bad(
        &|t| t.vehicles[0].mounts[0].ammo[0].ammo = 99,
        "vehicles.mounts",
    );
    bad(
        &|t| t.vehicles[0].components.pop().map(drop).unwrap_or(()),
        "vehicles.components",
    );
    bad(
        &|t| t.vehicles[0].boxes[0].plates[0].slope_deg = 90,
        "vehicles.armour",
    );
    bad(&|t| t.vehicles[0].faction = 9, "vehicles.faction");
    // S3b: the barrel box, the component boxes and the special round.
    bad(
        &|t| t.vehicles[0].mounts[0].barrel_length = Fixed::ZERO,
        "vehicles.mounts",
    );
    bad(
        &|t| t.vehicles[0].components[5].parts.clear(),
        "vehicles.components",
    );
    bad(
        &|t| {
            let p = t.vehicles[0].components[5].parts[0];
            t.vehicles[0].components[5].parts.extend([p; 4]);
        },
        "vehicles.components",
    );
    // S3b Fix: more inner boxes than the cap (S8a: 24), each component
    // within its own cap (5 seats + 5 boxed components x 4 = 25).
    bad(
        &|t| {
            for c in t.vehicles[0].components.iter_mut() {
                if let Some(&p) = c.parts.first() {
                    c.parts.resize(4, p);
                }
            }
        },
        "MAX_INNER_BOXES",
    );
    // S3b Fix: MG stowage only on an MG mount, and not 0.
    bad(
        &|t| t.vehicles[0].mounts[0].rounds = Some(100),
        "vehicles.mounts",
    );
    bad(
        &|t| t.vehicles[0].mounts[1].rounds = Some(0),
        "vehicles.mounts",
    );
    bad(
        &|t| {
            let m = &mut t.vehicles[0].mounts[0];
            let first = m.ammo[0].clone();
            m.ammo.swap(0, 1);
            m.ammo.push(first);
        },
        "vehicles.mounts",
    );
    let ap = s
        .ammo
        .iter()
        .position(|a| a.id == "ww2:75mm_pzgr39")
        .unwrap();
    let he = s
        .ammo
        .iter()
        .position(|a| a.id == "ww2:75mm_sprgr34")
        .unwrap();
    bad(&|t| t.ammo[ap].penetration.clear(), "ammo.penetration");
    bad(&|t| t.ammo[ap].ricochet_deg = 89, "ammo.values");
    bad(&|t| t.ammo[ap].spall = None, "ammo.mechanism");
    bad(
        &|t| t.ammo[he].blast.as_mut().unwrap().blast.pen_mm = 11,
        "ammo.blast",
    );
    let tank = s.squad_type_index("ww2:ger_pz4_h_squad").unwrap() as usize;
    bad(&|t| t.squad_types[tank].men = 4, "squad_types.vehicle");
    bad(
        &|t| t.squad_types[tank].vehicle = Some(1),
        "squad_types.vehicle",
    );
    let at = s.squad_type_index("ww2:ger_panzerschreck_team").unwrap() as usize;
    bad(
        &|t| t.squad_types[at].members[1].count = 3,
        "squad_types.men",
    );
    bad(&|t| t.rules.vehicles.wreck_cap = 0, "rules.vehicles");
    bad(
        &|t| t.rules.at_ditch.wall_run = Fixed::ZERO,
        "rules.dig.profile",
    );
}
