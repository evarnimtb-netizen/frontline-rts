//! S10c-ai (docs/slices/S10c-ai.md 2, 6): the fourteen new `[ai.obstacles]`
//! keys parse from the ww2 rules (at their off values but `front` 1, Build
//! B item 10), the `c_off` control file reaches the setup, and every key is
//! range-checked (overrides and untrusted saves). `front_home` (0/1, off 0)
//! is the orchestrator's key of 2026-11-16.

use std::path::{Path, PathBuf};

use frontline_content::tuning::AiObstacleTuning;
use frontline_content::{Difficulty, MatchRequest, ModDefs};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn write(name: &str, text: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "frontline-s10c-ai-{name}-{}.toml",
        std::process::id()
    ));
    std::fs::write(&p, text).unwrap();
    p
}

/// One out-of-range case per new key (spec 6). Control: widen one range in
/// `AiObstacleTuning` (seen with `front_points: 0..=9`) and that case is
/// accepted, so the test fails.
#[test]
fn load_rejects_bad_s10c_keys() {
    let mut defs = ModDefs::load(&root().join("mods/ww2")).unwrap_or_else(|e| panic!("{e}"));
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let s = defs.resolve(&req).unwrap();
    let o = &s.rules.ai.obstacles;
    // The mod states the off values (spec 2): switches 0, the floor 0;
    // Build B item 10 turns `front` on.
    assert_eq!(
        (
            o.front,
            o.mine_assign,
            o.belt,
            o.builder_cards_min,
            o.jobs_max
        ),
        (1, 0, 0, 0, 1)
    );
    assert_eq!(
        (
            o.front_points,
            o.front_out_m,
            o.front_wire_len_m,
            o.front_site_threat_m,
            o.front_fields_max,
            o.front_slots,
            o.front_home
        ),
        (2, 50, 20, 50, 2, 1, 0)
    );
    assert_eq!(
        (o.belt_pieces, o.belt_gap_m, o.belt_detour_pct),
        (5, 10, 130)
    );
    assert_eq!(
        *o,
        AiObstacleTuning {
            front: 1,
            ..AiObstacleTuning::default()
        }
    );
    // The control file applies and changes nothing (all keys at off).
    let c_off = root().join("docs/slices/targets/S10c-ai-ablations/c_off.toml");
    defs.apply_override(&c_off, "ww2:skirmish")
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        defs.resolve(&req).unwrap().rules.ai.obstacles,
        AiObstacleTuning::default()
    );
    // An override reaches the setup.
    let on = write(
        "on",
        "[ai.obstacles]\nfront = 1\nfront_points = 8\nbelt_detour_pct = 1000\n",
    );
    defs.apply_override(&on, "ww2:skirmish")
        .unwrap_or_else(|e| panic!("{e}"));
    let s = defs.resolve(&req).unwrap();
    let o = &s.rules.ai.obstacles;
    assert_eq!((o.front, o.front_points, o.belt_detour_pct), (1, 8, 1000));
    // Out of range: one case per key; the issue names the key.
    for (key, bad) in [
        ("front", 2),
        ("mine_assign", 2),
        ("belt", 2),
        ("front_points", 9),
        ("front_out_m", 501),
        ("front_wire_len_m", 501),
        ("front_site_threat_m", 501),
        ("front_fields_max", 17),
        ("front_slots", 17),
        ("front_home", 2),
        ("belt_pieces", 21),
        ("belt_gap_m", 501),
        ("belt_detour_pct", 1001),
        ("builder_cards_min", 9),
    ] {
        let f = write(key, &format!("[ai.obstacles]\n{key} = {bad}\n"));
        let e = defs
            .apply_override(&f, "ww2:skirmish")
            .err()
            .unwrap_or_else(|| panic!("{key} = {bad} was accepted"))
            .to_string();
        assert!(e.contains(&format!("ai.obstacles.{key}")), "{e}");
    }
    // The setup is checked too (saves are untrusted).
    let mut s = defs.resolve(&req).unwrap();
    assert!(s.validate().is_empty());
    s.rules.ai.obstacles.front_points = 9;
    assert!(
        s.validate()
            .iter()
            .any(|i| i.path.contains("ai.obstacles.front_points")
                || i.message.contains("ai.obstacles.front_points")),
        "{:?}",
        s.validate()
    );
}
