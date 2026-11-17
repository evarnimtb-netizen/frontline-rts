//! S11a1 (docs/slices/S11a1.md 2): the new `[fire_mission]`, `[supply]` and
//! `[ai.arty]` keys parse from the ww2 rules at their defaults, the `a_off`
//! control file reaches the setup, every key is range-checked (overrides
//! and untrusted saves), and the `[indirect]` gun keys, the `radio` trait
//! and the `inf_gun` role load. Build B adds the content tests here.

use std::path::{Path, PathBuf};

use frontline_content::tuning::{AiArtyTuning, FireMissionTuning};
use frontline_content::{Difficulty, MatchRequest, ModDefs};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn write(name: &str, text: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "frontline-s11a1-{name}-{}.toml",
        std::process::id()
    ));
    std::fs::write(&p, text).unwrap();
    p
}

/// One out-of-range case per new key (spec 2 ranges). Control: widen one
/// range (seen with `gun_break_m: 0..=2001`) and that case is accepted, so
/// the test fails.
#[test]
fn load_rejects_bad_s11a1_keys() {
    let mut defs = ModDefs::load(&root().join("mods/ww2")).unwrap_or_else(|e| panic!("{e}"));
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let s = defs.resolve(&req).unwrap();
    let fm = s.rules.fire_mission.as_ref().unwrap();
    assert_eq!(
        (
            fm.gun_missions,
            fm.radio_link,
            fm.mask,
            fm.gun_fire_direction_s,
            fm.gun_adjust_rounds_max
        ),
        (1, 1, 1, 20, 1)
    );
    assert_eq!(
        (
            fm.gun_first_round_error_pct,
            fm.gun_first_round_error_min_m,
            fm.gun_map_fire_error_pct,
            fm.gun_map_fire_error_min_m,
            fm.gun_break_m
        ),
        (3, 25, 6, 50, 150)
    );
    assert_eq!(s.rules.supply.as_ref().unwrap().arty_base_rearm, 1);
    assert_eq!(s.rules.ai.arty, AiArtyTuning::default());
    assert_eq!(FireMissionTuning::KEYS.len(), 35);
    assert_eq!(AiArtyTuning::KEYS.len(), 21);
    assert!(
        frontline_content::tuning::TABLES
            .iter()
            .any(|(t, k)| *t == "ai.arty" && k.len() == 21)
    );
    // The control file applies; the switches go off.
    let a_off = root().join("docs/slices/targets/S11a1-ablations/a_off.toml");
    defs.apply_override(&a_off, "ww2:skirmish")
        .unwrap_or_else(|e| panic!("{e}"));
    let s = defs.resolve(&req).unwrap();
    let fm = s.rules.fire_mission.as_ref().unwrap();
    assert_eq!((fm.gun_missions, fm.radio_link, fm.mask), (0, 0, 0));
    assert_eq!(s.rules.supply.as_ref().unwrap().arty_base_rearm, 1);
    assert_eq!(s.rules.ai.arty, AiArtyTuning::default());
    // An override reaches the setup.
    let on = write("on", "[ai.arty]\nbuy = 1\npost_rays = 48\n");
    defs.apply_override(&on, "ww2:skirmish")
        .unwrap_or_else(|e| panic!("{e}"));
    let s = defs.resolve(&req).unwrap();
    assert_eq!((s.rules.ai.arty.buy, s.rules.ai.arty.post_rays), (1, 48));
    // Out of range: one case per key; the issue names the key.
    let cases: Vec<(&str, &str, u32)> = vec![
        ("fire_mission", "gun_missions", 2),
        ("fire_mission", "radio_link", 2),
        ("fire_mission", "mask", 2),
        ("fire_mission", "gun_fire_direction_s", 3601),
        ("fire_mission", "gun_adjust_rounds_max", 0),
        ("fire_mission", "gun_first_round_error_pct", 51),
        ("fire_mission", "gun_first_round_error_min_m", 2001),
        ("fire_mission", "gun_map_fire_error_pct", 51),
        ("fire_mission", "gun_map_fire_error_min_m", 2001),
        ("fire_mission", "gun_break_m", 2001),
        ("supply", "arty_base_rearm", 2),
        ("ai.arty", "buy", 2),
        ("ai.arty", "fo", 2),
        ("ai.arty", "prep", 2),
        ("ai.arty", "after_s", 3601),
        ("ai.arty", "income_max_pct", 101),
        ("ai.arty", "reserve_mp", 10001),
        ("ai.arty", "max_guns", 9),
        ("ai.arty", "fo_max", 9),
        ("ai.arty", "fo_rebuy_s", 3601),
        ("ai.arty", "mission_rounds", 51),
        ("ai.arty", "reserve_pct", 101),
        ("ai.arty", "shell_mp_max_pct", 101),
        ("ai.arty", "danger_close_m", 2001),
        ("ai.arty", "post_m", 2001),
        ("ai.arty", "post_side_m", 2001),
        ("ai.arty", "post_rays", 49),
        ("ai.arty", "post_clear_m", 2001),
        ("ai.arty", "post_retry_s", 3601),
        ("ai.arty", "prep_near_m", 2001),
        ("ai.arty", "prep_rounds_min", 0),
        ("ai.arty", "prep_wait_s", 3601),
    ];
    for (table, key, bad) in cases {
        let f = write(key, &format!("[{table}]\n{key} = {bad}\n"));
        let e = defs
            .apply_override(&f, "ww2:skirmish")
            .err()
            .unwrap_or_else(|| panic!("{table}.{key} = {bad} was accepted"))
            .to_string();
        assert!(e.contains(&format!("{table}.{key}")), "{e}");
    }
    // The gun error floors keep their order (observed <= map).
    let f = write(
        "order",
        "[fire_mission]\ngun_first_round_error_min_m = 60\ngun_map_fire_error_min_m = 50\n",
    );
    assert!(defs.apply_override(&f, "ww2:skirmish").is_err());
    // The setup is checked too (saves are untrusted).
    let mut s = defs.resolve(&req).unwrap();
    assert!(s.validate().is_empty());
    s.rules.ai.arty.post_rays = 49;
    assert!(
        s.validate().iter().any(
            |i| i.path.contains("ai.arty.post_rays") || i.message.contains("ai.arty.post_rays")
        ),
        "{:?}",
        s.validate()
    );
    let mut s = defs.resolve(&req).unwrap();
    s.rules.fire_mission.as_mut().unwrap().gun_break_m = 2001;
    assert!(!s.validate().is_empty());
}

/// The `[indirect]` gun keys and the `radio` trait (schema in Build A,
/// used in Build B). Control: a `low_angle` table with a band reaching 90
/// is refused.
#[test]
fn indirect_gun_keys_and_radio_trait_parse() {
    use frontline_content::defs::IndirectDef;
    use frontline_content::setup::{IndirectSetup, member_trait};
    let d: IndirectDef = toml::from_str(
        "charges_m_s = [92, 120, 150, 180, 210]\nelevation_deg = [0, 73]\nlow_angle = true\nrange_m = 3550\nspread_mrad = 8\n",
    )
    .unwrap();
    assert!(d.low_angle);
    let s = IndirectSetup::lower(&d);
    assert!(s.low_angle && s.ok());
    assert_eq!(s.range.0, 3550 << 16);
    // A mortar table is unchanged: no low angle, defaults left to the weapon.
    let m: IndirectDef =
        toml::from_str("charges_m_s = [75, 105, 130, 152, 174]\nelevation_deg = [45, 87]\n")
            .unwrap();
    assert!(!m.low_angle && m.range_m.is_none() && m.spread_mrad.is_none());
    let l = IndirectSetup::lower_for(&m, 1100, frontline_content::Fixed::from_ratio(3, 100));
    assert!(!l.low_angle && l.ok());
    assert_eq!(l.range.0, 1100 << 16);
    // The bad band (control).
    let mut bad = IndirectSetup::lower(&d);
    bad.tan_max = frontline_content::Fixed::from_int(60);
    assert!(!bad.ok());
    // A mortar setup (not low_angle) still needs 45 deg at least.
    let mut low = l.clone();
    low.tan_min = frontline_content::Fixed::ZERO;
    assert!(!low.ok());
    assert_eq!(member_trait::RADIO, 32);
    assert!(
        member_trait::NAMES
            .iter()
            .any(|&(n, b)| n == "radio" && b == 32)
    );
    assert_eq!(member_trait::ALL & member_trait::RADIO, member_trait::RADIO);
    assert!(frontline_content::VehicleRole::InfGun.outside_limit());
    assert_eq!(frontline_content::VehicleRole::InfGun.name(), "inf_gun");
}
