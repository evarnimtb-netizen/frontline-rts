//! S13 (docs/slices/S13-heavy-wave.md 3.2, 3.6, 5): the `[armour]`,
//! `[vehicles.gunnery] special_min_pct` and S13 `[ai.armour]` keys parse
//! from the ww2 rules with every S13 switch off, the ablation files apply,
//! every key is range-checked, and the def keys `slope_class`, `ai_heavy`
//! and `ai_overwatch` are checked.

use std::path::{Path, PathBuf};

use frontline_content::tuning::ArmourTuning;
use frontline_content::{Difficulty, MatchRequest, ModDefs, SlopeClass};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn write(name: &str, text: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("frontline-s13-{name}-{}.toml", std::process::id()));
    std::fs::write(&p, text).unwrap();
    p
}

/// The S13 keys of `[ai.armour]` with their ranges (plan 5).
const AI_KEYS: [(&str, u32, u32); 7] = [
    ("heavy_buy", 0, 1),
    ("heavy_seen_s", 0, 3600),
    ("heavy_save_max_s", 0, 3600),
    ("heavy_overwatch", 0, 1),
    ("push_limit", 0, 1),
    ("push_without_supply_m", 0, 5000),
    ("tank_reserve_rounds", 0, 200),
];

/// One out-of-range case per key bound (plan 3.2 ranges: switch 0..=1,
/// `td_*` 0..=300, weights 0..=100). Control: widen one range (seen with
/// `apcr_w_pct: 0..=101`) and that case is accepted, so the test fails.
#[test]
fn load_rejects_bad_s13_keys() {
    let defs0 = ModDefs::load(&root().join("mods/ww2")).unwrap_or_else(|e| panic!("{e}"));
    let mut defs = defs0.clone();
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let s = defs.resolve(&req).unwrap();
    // The ww2 rules state the defaults; S13-ai Build C turns H1-H3 on.
    assert_eq!(s.rules.armour, ArmourTuning::default());
    assert_eq!(ArmourTuning::KEYS.len(), 5);
    let a = &s.rules.armour;
    assert_eq!(
        (
            a.obliquity,
            a.td_lo_pct,
            a.td_hi_pct,
            a.apcbc_w_pct,
            a.apcr_w_pct
        ),
        (1, 40, 90, 100, 100)
    );
    assert_eq!(s.rules.vehicles.gunnery.special_min_pct, 0);
    let ar = &s.rules.ai.armour;
    assert_eq!(
        (
            ar.heavy_buy,
            ar.heavy_seen_s,
            ar.heavy_save_max_s,
            ar.heavy_overwatch
        ),
        (1, 300, 240, 1)
    );
    assert_eq!(
        (
            ar.push_limit,
            ar.push_without_supply_m,
            ar.tank_reserve_rounds
        ),
        (1, 300, 10)
    );
    // S13a Build B: exactly the seven AP / APCBC / APCR rounds of the eleven
    // new ones carry a class (the four HE rounds refuse one); of the old
    // rounds only the two twins B3 run 2 classes (B3-4 (A)).
    let mut classed: Vec<(&str, SlopeClass)> = s
        .ammo
        .iter()
        .filter_map(|x| x.slope_class.map(|c| (x.id.as_str(), c)))
        .collect();
    classed.sort_by_key(|&(id, _)| id);
    assert_eq!(
        classed,
        [
            ("ww2:3in_m62", SlopeClass::Apcbc),
            ("ww2:75mm_pzgr3942", SlopeClass::Apcbc),
            ("ww2:75mm_pzgr40", SlopeClass::Apcr),
            ("ww2:75mm_pzgr4042", SlopeClass::Apcr),
            ("ww2:76mm_m62", SlopeClass::Apcbc),
            ("ww2:76mm_m93", SlopeClass::Apcr),
            ("ww2:88mm_pzgr39", SlopeClass::Apcbc),
            ("ww2:88mm_pzgr40", SlopeClass::Apcr),
            ("ww2:90mm_m82", SlopeClass::Apcbc),
        ][..]
    );
    let mut heavy: Vec<&str> = s
        .squad_types
        .iter()
        .filter(|t| t.ai_heavy)
        .map(|t| t.id.as_str())
        .collect();
    heavy.sort();
    assert_eq!(
        heavy,
        [
            "ww2:ger_panther_g_squad",
            "ww2:ger_tiger_1_squad",
            // B3 run 2 (B3-3 (A)).
            "ww2:usa_m36_squad",
            "ww2:usa_m4a3e2_squad"
        ]
    );
    let mut watch: Vec<&str> = s
        .vehicles
        .iter()
        .filter(|v| v.ai_overwatch)
        .map(|v| v.id.as_str())
        .collect();
    watch.sort();
    assert_eq!(watch, ["ww2:ger_panther_g", "ww2:ger_tiger_1"]);
    // S13-ai Build C: the heavy cards stay at weight 0 (H1 buys them), the
    // 76 W and the M36 at 100 (plan 4.3); all five carry `heavy_wave`.
    for (id, wt) in [
        ("ww2:ger_panther_g_squad", 0),
        ("ww2:ger_tiger_1_squad", 0),
        ("ww2:usa_m4a3_76w_squad", 100),
        ("ww2:usa_m36_squad", 100),
        ("ww2:usa_m4a3e2_squad", 0),
    ] {
        let k = s.squad_types.iter().position(|t| t.id == id).unwrap();
        assert_eq!(s.squad_types[k].ai_weight, wt, "{id}");
        assert!(s.type_has_tag(k as u16, "heavy_wave"), "{id}");
    }
    // The ablation files apply.
    for f in ["s13_off", "o1_off", "heavy_off", "push_off"] {
        let mut d = defs0.clone();
        let p = root().join(format!("docs/slices/targets/S13-ablations/{f}.toml"));
        d.apply_override(&p, "ww2:skirmish")
            .unwrap_or_else(|e| panic!("{f}: {e}"));
        let s = d.resolve(&req).unwrap();
        let off = matches!(f, "s13_off" | "o1_off");
        assert_eq!(s.rules.armour.obliquity, u32::from(!off), "{f}");
    }
    // An override reaches the setup.
    let on = write(
        "on",
        "[armour]\ntd_lo_pct = 25\ntd_hi_pct = 100\n[vehicles.gunnery]\nspecial_min_pct = 25\n[ai.armour]\nheavy_buy = 1\n",
    );
    defs.apply_override(&on, "ww2:skirmish")
        .unwrap_or_else(|e| panic!("{e}"));
    let s = defs.resolve(&req).unwrap();
    assert_eq!(
        (s.rules.armour.td_lo_pct, s.rules.armour.td_hi_pct),
        (25, 100)
    );
    assert_eq!(s.rules.vehicles.gunnery.special_min_pct, 25);
    assert_eq!(s.rules.ai.armour.heavy_buy, 1);
    // Out of range: one case per bound; the issue names the key.
    let mut cases: Vec<(&str, &str, u32)> = Vec::new();
    for &(key, lo, hi) in ArmourTuning::KEYS {
        if lo > 0 {
            cases.push(("armour", key, lo - 1));
        }
        cases.push(("armour", key, hi + 1));
    }
    for (key, _, hi) in AI_KEYS {
        cases.push(("ai.armour", key, hi + 1));
    }
    cases.push(("vehicles.gunnery", "special_min_pct", 101));
    for (table, key, bad) in cases {
        let f = write(key, &format!("[{table}]\n{key} = {bad}\n"));
        let e = defs0
            .clone()
            .apply_override(&f, "ww2:skirmish")
            .err()
            .unwrap_or_else(|| panic!("{table}.{key} = {bad} was accepted"))
            .to_string();
        assert!(e.contains(&format!("{table}.{key}")), "{e}");
    }
    // The ramp's order: lo below hi.
    let f = write("order", "[armour]\ntd_lo_pct = 90\ntd_hi_pct = 90\n");
    let e = defs0
        .clone()
        .apply_override(&f, "ww2:skirmish")
        .err()
        .map(|e| e.to_string())
        .unwrap_or_else(|| panic!("td_lo_pct = td_hi_pct was accepted"));
    assert!(e.contains("armour.td_lo_pct"), "{e}");
    // The plan's window of ramps (3.2): every one is accepted.
    for lo in [25, 30, 40, 50] {
        for hi in [75, 80, 90, 100] {
            let f = write(
                "window",
                &format!("[armour]\ntd_lo_pct = {lo}\ntd_hi_pct = {hi}\n"),
            );
            defs0
                .clone()
                .apply_override(&f, "ww2:skirmish")
                .unwrap_or_else(|e| panic!("({lo}, {hi}): {e}"));
        }
    }
}

/// Plan 3.2: `slope_class` only on `ap` and `aphe` rounds; plan 5:
/// `ai_heavy` only on a vehicle card. Control: the unedited defs validate.
#[test]
fn s13_def_keys_are_checked() {
    let defs = ModDefs::load(&root().join("mods/ww2")).unwrap_or_else(|e| panic!("{e}"));
    let named = |v: &[frontline_content::Issue], k: &str| v.iter().any(|i| i.path.ends_with(k));
    for (id, a) in &defs.ammo {
        assert!(a.validate().is_empty(), "control: {id}");
        let mut x = a.clone();
        x.slope_class = Some(SlopeClass::Apcr);
        let ok = matches!(
            a.mechanism,
            frontline_content::AmmoMechanism::Ap | frontline_content::AmmoMechanism::Aphe
        );
        assert_eq!(!named(&x.validate(), "slope_class"), ok, "{id}");
    }
    let mut seen = (false, false);
    for (id, q) in &defs.squads {
        let mut x = q.clone();
        x.ai_heavy = true;
        let bad = named(&x.validate(), "ai_heavy");
        assert_eq!(bad, q.vehicle.is_none(), "{id}");
        seen.0 |= bad;
        seen.1 |= !bad;
    }
    assert_eq!(seen, (true, true), "both kinds of card were tried");
    // `ai_overwatch` is a key of a vehicle def (deny_unknown_fields).
    let (id, v) = defs.vehicles.iter().next().unwrap();
    let mut j = serde_json::to_value(v).unwrap();
    assert!(
        j.get("ai_overwatch").is_none(),
        "{id}: false is not written"
    );
    j["ai_overwatch"] = serde_json::Value::Bool(true);
    let back: frontline_content::VehicleDef = serde_json::from_value(j).unwrap();
    assert!(back.ai_overwatch);
}
