//! S11a2 (docs/slices/S11a2.md 2): the new `[fire_mission]` and `[ai.arty]`
//! keys parse from the ww2 rules with all four switches 0 (Build A), the
//! `b_off` control file applies, every key is range-checked (overrides and
//! untrusted saves), and the error floor keeps its order.

use std::path::{Path, PathBuf};

use frontline_content::tuning::{AiArtyTuning, FireMissionTuning};
use frontline_content::{Difficulty, MatchRequest, ModDefs};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn write(name: &str, text: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "frontline-s11a2-{name}-{}.toml",
        std::process::id()
    ));
    std::fs::write(&p, text).unwrap();
    p
}

/// One out-of-range case per new key (spec 2 ranges). Control: widen one
/// range (seen with `cb_near_m: 0..=2001`) and that case is accepted, so
/// the test fails.
#[test]
fn load_rejects_bad_s11a2_keys() {
    let mut defs = ModDefs::load(&root().join("mods/ww2")).unwrap_or_else(|e| panic!("{e}"));
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let s = defs.resolve(&req).unwrap();
    let fm = s.rules.fire_mission.as_ref().unwrap();
    // Build A: both sim switches 0, the other keys at the spec values.
    assert_eq!(
        (
            fm.friendly_fire,
            fm.counter_battery,
            fm.cb_rounds_to_mark,
            fm.cb_decay_s,
            fm.cb_delay_s
        ),
        (0, 0, 3, 300, 15)
    );
    assert_eq!(
        (
            fm.cb_error_pct,
            fm.cb_error_step_pct,
            fm.cb_error_floor_pct,
            fm.cb_reset_m,
            fm.cb_near_m
        ),
        (15, 80, 5, 75, 20)
    );
    let a = &s.rules.ai.arty;
    assert_eq!(
        (a.ff_safe, a.counter_fire, a.lift_m, a.cb_max_error_m),
        (0, 0, 100, 60)
    );
    let d = AiArtyTuning::default();
    assert_eq!(
        (d.ff_safe, d.counter_fire, d.lift_m, d.cb_max_error_m),
        (0, 0, 100, 60)
    );
    assert_eq!(FireMissionTuning::KEYS.len(), 45);
    assert_eq!(AiArtyTuning::KEYS.len(), 25);
    // The control file applies (its keys exist and are in range).
    let b_off = root().join("docs/slices/targets/S11a2-ablations/b_off.toml");
    defs.apply_override(&b_off, "ww2:skirmish")
        .unwrap_or_else(|e| panic!("{e}"));
    // An override reaches the setup.
    let on = write(
        "on",
        "[fire_mission]\nfriendly_fire = 1\ncb_reset_m = 2000\n[ai.arty]\ncounter_fire = 1\n",
    );
    defs.apply_override(&on, "ww2:skirmish")
        .unwrap_or_else(|e| panic!("{e}"));
    let s = defs.resolve(&req).unwrap();
    let fm = s.rules.fire_mission.as_ref().unwrap();
    assert_eq!((fm.friendly_fire, fm.cb_reset_m), (1, 2000));
    assert_eq!(s.rules.ai.arty.counter_fire, 1);
    // Out of range: one case per key; the issue names the key.
    let cases: Vec<(&str, &str, u32)> = vec![
        ("fire_mission", "friendly_fire", 2),
        ("fire_mission", "counter_battery", 2),
        ("fire_mission", "cb_rounds_to_mark", 0),
        ("fire_mission", "cb_rounds_to_mark", 51),
        ("fire_mission", "cb_decay_s", 0),
        ("fire_mission", "cb_decay_s", 3601),
        ("fire_mission", "cb_delay_s", 0),
        ("fire_mission", "cb_delay_s", 3601),
        ("fire_mission", "cb_error_pct", 0),
        ("fire_mission", "cb_error_pct", 101),
        ("fire_mission", "cb_error_step_pct", 0),
        ("fire_mission", "cb_error_step_pct", 101),
        ("fire_mission", "cb_error_floor_pct", 0),
        ("fire_mission", "cb_reset_m", 2001),
        ("fire_mission", "cb_near_m", 2001),
        ("ai.arty", "ff_safe", 2),
        ("ai.arty", "counter_fire", 2),
        ("ai.arty", "lift_m", 2001),
        ("ai.arty", "cb_max_error_m", 2001),
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
    // The error floor is at most the first error.
    let f = write(
        "order",
        "[fire_mission]\ncb_error_pct = 10\ncb_error_floor_pct = 11\n",
    );
    assert!(defs.apply_override(&f, "ww2:skirmish").is_err());
    // The setup is checked too (saves are untrusted).
    let mut s = defs.resolve(&req).unwrap();
    assert!(s.validate().is_empty());
    s.rules.fire_mission.as_mut().unwrap().cb_near_m = 2001;
    assert!(!s.validate().is_empty());
    let mut s = defs.resolve(&req).unwrap();
    s.rules.ai.arty.lift_m = 2001;
    assert!(!s.validate().is_empty());
}
