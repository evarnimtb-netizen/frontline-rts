//! S4 Part A migration guards (docs/slices/S4-variants.md, "Migration plan").
//!
//! * `setup_bytes_baseline`: the `MatchSetup` the shipped `ww2` mod resolves
//!   to is byte-identical to the one recorded before the variant system
//!   (`63c3e36`): its postcard bytes are part of the state hash, so equal
//!   bytes here plus equal match hashes prove Part A changed no behaviour.
//! * `legacy_view_matches_snapshot`: every resolved def of `mods/ww2`, as
//!   JSON, still carries every key and value the old loader produced
//!   (`snapshots/ww2_resolved.json`, written by the old loader; never
//!   re-blessed in S4).
//! * `canonical_view_matches_snapshot`: the full resolved view (members,
//!   tags, capabilities, card keys) equals
//!   `snapshots/ww2_canonical.json`. Content changes update it on purpose:
//!   `BLESS=1 cargo test -p frontline-content --test resolved_defs`.

use std::path::{Path, PathBuf};

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs, PlayerRequest};
use serde_json::Value;

fn ww2_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mods/ww2")
}

/// The S3c content frozen before the S4 content switch (`fixtures/
/// ww2_s3c/README.md`): the legacy unit files the legacy guards load.
fn s3c_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ww2_s3c")
}

fn snapshots() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

/// The frozen S3c content (legacy guards).
fn load() -> ModDefs {
    ModDefs::load(&s3c_root()).unwrap_or_else(|e| panic!("{e}"))
}

/// The live `mods/ww2` (the canonical view).
fn live() -> ModDefs {
    ModDefs::load(&ww2_root()).unwrap_or_else(|e| panic!("{e}"))
}

fn ai_vs_ai(rules: &str, a: &str, b: &str, seed: u64, difficulty: Difficulty) -> MatchRequest {
    MatchRequest {
        rules: rules.into(),
        map: "ww2:kessel_ridge".into(),
        seed,
        players: vec![
            PlayerRequest {
                faction: a.into(),
                ai: true,
                difficulty,
            },
            PlayerRequest {
                faction: b.into(),
                ai: true,
                difficulty,
            },
        ],
    }
}

fn setup_hash(s: &MatchSetup) -> u64 {
    let bytes = postcard::to_allocvec(s).expect("encoding a setup cannot fail");
    xxhash_rust::xxh3::xxh3_64(&bytes)
}

/// xxh3 of the setups of the frozen S3c content (`fixtures/ww2_s3c`).
/// S4 Part A kept the `63c3e36` values (0x5871bd969965f5a6,
/// 0x423d6bd094b287d8, 0x8b78652432150cb0, 0x5871bd969965f5a6,
/// 0x59a3d2447901500a, 0x35fe520d10f8ce52); Part B (B1) changed the setup
/// layout on purpose (members, tags, capabilities, training, card limits,
/// the AI mix by tag, the fourth item kind). The S4 Fix stage added one
/// rules field to the setup (`items.crew_pickup_reach_m`, default 30 m,
/// review L5); the Part B values were 0x61dcc7e06ca47ff6,
/// 0xe66560dffba1e680, 0xe1faafbe66546d9f, 0x61dcc7e06ca47ff6,
/// 0x609ba8ba1224583f, 0x672de1cdc6cb85c6. S5a (A2) changed the layout on
/// purpose (cartridges, supply weights, feeds and member rounds, `rare`,
/// `rare_cap` and the rack factor, the `[supply]` and `[combat.share]`
/// rules); the S4 Fix values were 0x10e500c23953fb93, 0x3689d883fe85dd98,
/// 0xe43938148a1d867e, 0x10e500c23953fb93, 0xdc46d00b47f91e5c,
/// 0xb73ebf70d33b818b. S5a Part B changed it again on purpose (the
/// `[combat.fire]` and `[ai.ammo]` rules, the weapon `hold_beyond_pct`);
/// the Part A values were 0x981be222d89dc533, 0xa361b2c3d8ab0501,
/// 0x8c8da20bb09e1c4a, 0x981be222d89dc533, 0xbf2dcec35942ab25,
/// 0x8a441c7150adde30. The S5a Fix added three rules keys
/// (`hold_spring_points`, `ambush_release_m`, `base_bomb_after_fire_s`);
/// the Part B values were 0xc75149efb1159f88, 0xa2342d0679d92657,
/// 0x8fa980212ecdf1f2, 0xc75149efb1159f88, 0xcd0a7c0c5ede5b2a,
/// 0xf7f2ebfeb6842321. S5b (Part A) changed the layout on purpose (the
/// weapon `indirect` table, the `[fire_mission]` rules and `[ai.mortar]`,
/// all absent or legacy defaults in this content); the S5a Fix values were
/// 0xbb3b8dae2e8c822a, 0x69d3ee2a3c5151fe, 0xb46ab44ea1747786,
/// 0xbb3b8dae2e8c822a, 0x46802df03f79e372, 0x3940489958d5181a. S6 changed
/// the layout on purpose (the `stocks` list, the pit profiles in the
/// rules, the S6 `[supply]` keys with their defaults and `[ai.supply]`);
/// the S5b values were 0x41265dc0ea30e6f4, 0x09ac37c094fbee1b,
/// 0x1183b57b79b09e46, 0x41265dc0ea30e6f4, 0x1c4ee9689e2794e7,
/// 0x855841c1470238ab, and these are the new ones. The live `mods/ww2` is
/// guarded by the canonical snapshot instead.
const BASELINE: [(&str, u64); 6] = [
    ("skirmish ger-usa seed 1 normal", 0xb05ebf261ab0a6d0),
    ("skirmish usa-ger seed 7 hard", 0x37e505284272d288),
    ("test_unlock ger-usa seed 1 normal", 0xb923a689252fb652),
    ("example override ger-usa seed 1 normal", 0xb05ebf261ab0a6d0),
    ("human ger vs ai usa seed 3 easy", 0x1fb482f543103c0a),
    (
        "unit and weapon patch ger-usa seed 1 normal",
        0x796a5341fe2d7f00,
    ),
];

/// An override that changes a unit, a weapon and the rules (the patch path
/// must lower exactly as before too).
const PATCH: &str = r#"
[ai]
mix_mg_pct = 30
[defs.units."ww2:usa_rifle_squad"]
cost = 300
men = 5
[defs.units."ww2:ger_mg_team"]
recall_cooldown_s = 50
[defs.weapons."ww2:kar98k"]
range_m = 400
"#;

fn baseline_setups() -> Vec<(&'static str, MatchSetup)> {
    let root = s3c_root();
    let defs = load();
    let mut unlock = defs.clone();
    unlock
        .add_rules_override(&root.join("tests/rules_test_unlock.toml"))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut example = defs.clone();
    example
        .apply_override(&root.join("tests/overrides/example.toml"), "ww2:skirmish")
        .unwrap_or_else(|e| panic!("{e}"));
    let mut patched = defs.clone();
    let patch =
        std::env::temp_dir().join(format!("frontline-s4-patch-{}.toml", std::process::id()));
    std::fs::write(&patch, PATCH).unwrap();
    patched
        .apply_override(&patch, "ww2:skirmish")
        .unwrap_or_else(|e| panic!("{e}"));
    let _ = std::fs::remove_file(&patch);
    let (ger, usa) = ("ww2:germany", "ww2:usa");
    let r = |d: &ModDefs, req: MatchRequest| d.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
    vec![
        (
            BASELINE[0].0,
            r(
                &defs,
                ai_vs_ai("ww2:skirmish", ger, usa, 1, Difficulty::Normal),
            ),
        ),
        (
            BASELINE[1].0,
            r(
                &defs,
                ai_vs_ai("ww2:skirmish", usa, ger, 7, Difficulty::Hard),
            ),
        ),
        (
            BASELINE[2].0,
            r(
                &unlock,
                ai_vs_ai("ww2:skirmish_test_unlock", ger, usa, 1, Difficulty::Normal),
            ),
        ),
        (
            BASELINE[3].0,
            r(
                &example,
                ai_vs_ai("ww2:skirmish", ger, usa, 1, Difficulty::Normal),
            ),
        ),
        (
            BASELINE[4].0,
            r(&defs, MatchRequest::skirmish(3, ger, usa, Difficulty::Easy)),
        ),
        (
            BASELINE[5].0,
            r(
                &patched,
                ai_vs_ai("ww2:skirmish", ger, usa, 1, Difficulty::Normal),
            ),
        ),
    ]
}

#[test]
fn setup_bytes_baseline() {
    let got: Vec<(&str, u64)> = baseline_setups()
        .iter()
        .map(|(n, s)| (*n, setup_hash(s)))
        .collect();
    let show: String = got
        .iter()
        .map(|(n, h)| format!("    (\"{n}\", 0x{h:016x}),\n"))
        .collect();
    assert_eq!(got, BASELINE, "setup bytes changed; now:\n{show}");
}

/// Every def of every kind as one JSON object `{kind: {id: def}}` (keys
/// sorted: serde_json's map is a `BTreeMap`).
fn resolved_json(d: &ModDefs) -> Value {
    fn kind<T: serde::Serialize>(m: &std::collections::BTreeMap<String, T>) -> Value {
        serde_json::to_value(m).expect("defs serialise")
    }
    serde_json::json!({
        "units": kind(&d.squads),
        "weapons": kind(&d.weapons),
        "items": kind(&d.items),
        "factions": kind(&d.factions),
        "maps": kind(&d.maps),
        "rules": kind(&d.rules),
        "vehicles": kind(&d.vehicles),
        "ammo": kind(&d.ammo),
    })
}

/// The S4 canonical view: every kind including training profiles.
fn canonical_json(d: &ModDefs) -> Value {
    let mut v = resolved_json(d);
    v["training"] = serde_json::to_value(&d.training).expect("defs serialise");
    v
}

fn bless() -> bool {
    std::env::var_os("BLESS").is_some()
}

fn read_snapshot(name: &str) -> Option<Value> {
    let text = std::fs::read_to_string(snapshots().join(name)).ok()?;
    Some(serde_json::from_str(&text).expect("snapshot is JSON"))
}

fn write_snapshot(name: &str, v: &Value) {
    std::fs::create_dir_all(snapshots()).unwrap();
    let mut text = serde_json::to_string_pretty(v).unwrap();
    text.push('\n');
    std::fs::write(snapshots().join(name), text).unwrap();
}

/// Paths where `want` has a value `got` lacks or differs from (objects:
/// every key of `want`; anything else: equal).
fn missing(path: &str, want: &Value, got: &Value, out: &mut Vec<String>) {
    match (want, got) {
        (Value::Object(w), Value::Object(g)) => {
            for (k, wv) in w {
                let p = format!("{path}.{k}");
                match g.get(k) {
                    Some(gv) => missing(&p, wv, gv, out),
                    None => out.push(format!("{p}: missing (was {wv})")),
                }
            }
        }
        _ if want == got => {}
        _ => out.push(format!("{path}: {want} -> {got}")),
    }
}

#[test]
fn legacy_view_matches_snapshot() {
    let got = resolved_json(&load());
    let name = "ww2_resolved.json";
    let Some(want) = read_snapshot(name) else {
        // Written once, by the old loader (S4 Part A step 1).
        write_snapshot(name, &got);
        panic!("wrote tests/snapshots/{name}; commit it and run again");
    };
    let mut diffs = Vec::new();
    missing("", &want, &got, &mut diffs);
    // The same defs, no more and no fewer, per kind.
    for (k, w) in want.as_object().unwrap() {
        let ids = |v: &Value| v.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>());
        if ids(w) != ids(&got[k]) {
            diffs.push(format!("{k}: ids {:?} -> {:?}", ids(w), ids(&got[k])));
        }
    }
    assert!(
        diffs.is_empty(),
        "the legacy view changed:\n{}",
        diffs.join("\n")
    );
}

#[test]
fn canonical_view_matches_snapshot() {
    let got = canonical_json(&live());
    let name = "ww2_canonical.json";
    match read_snapshot(name) {
        Some(want) if !bless() => {
            let mut diffs = Vec::new();
            missing("", &want, &got, &mut diffs);
            missing("", &got, &want, &mut diffs);
            assert!(
                diffs.is_empty(),
                "the canonical view changed (BLESS=1 to accept):\n{}",
                diffs.join("\n")
            );
        }
        _ => {
            write_snapshot(name, &got);
            if !bless() {
                panic!("wrote tests/snapshots/{name}; commit it and run again");
            }
        }
    }
}

/// The legacy expansion keeps today's men exactly (S4-2): an MG team is
/// three MG gunners, an AT team a launcher and a rifleman, a crew one SMG
/// and pistols; every soldier card has its role's tag and capabilities.
#[test]
fn legacy_cards_expand_to_todays_men() {
    let d = load();
    let slots = |id: &str| -> Vec<(String, u8, String)> {
        d.squads[id]
            .members
            .iter()
            .map(|m| (m.slot.clone(), m.count, m.weapon.clone()))
            .collect()
    };
    let s = |a: &str, n: u8, w: &str| (a.to_owned(), n, w.to_owned());
    assert_eq!(
        slots("ww2:ger_mg_team"),
        [s("leader", 1, "ww2:mg42"), s("weapon", 2, "ww2:mg42")]
    );
    assert_eq!(
        slots("ww2:usa_bazooka_team"),
        [
            s("leader", 1, "ww2:m9_bazooka"),
            s("sidearm", 1, "ww2:m1_garand")
        ]
    );
    assert_eq!(
        slots("ww2:ger_tank_crew"),
        [s("leader", 1, "ww2:mp40"), s("sidearm", 4, "ww2:p38")]
    );
    let rifle = &d.squads["ww2:ger_rifle_squad"];
    assert_eq!(rifle.tags, ["line"]);
    assert_eq!(rifle.capabilities, ["dig_basic", "recrew"]);
    assert_eq!(rifle.members[0].items.get("ww2:stielhandgranate"), Some(&2));
    assert_eq!((rifle.cooldown_s, rifle.unlock_s), (Some(24), Some(0)));
    assert!(d.squads["ww2:ger_mortar_team"].has_capability("fire_mission"));
    assert!(d.squads["ww2:usa_bazooka_team"].has_capability("at_hunt"));
    assert!(d.squads["ww2:usa_engineer_squad"].has_capability("dig_full"));
    assert!(d.squads["ww2:ger_tank_crew"].has_capability("vehicle_crew"));
    let tank = &d.squads["ww2:ger_pz4_h_squad"];
    assert_eq!(tank.tags, ["vehicle"]);
    assert!(tank.members.is_empty() && tank.capabilities.is_empty());
    assert_eq!(tank.cooldown_s, None);
    assert_eq!(d.training.len(), 4);
    assert!(d.training["ww2:training_regular"].is_regular());
    assert!(!d.training["ww2:training_conscript"].is_regular());
}
