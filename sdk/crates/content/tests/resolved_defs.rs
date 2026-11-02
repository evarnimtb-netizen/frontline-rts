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

fn snapshots() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

fn load() -> ModDefs {
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

/// xxh3 of the setups recorded at `63c3e36` (S4 Plan baseline).
const BASELINE: [(&str, u64); 6] = [
    ("skirmish ger-usa seed 1 normal", 0x5871bd969965f5a6),
    ("skirmish usa-ger seed 7 hard", 0x423d6bd094b287d8),
    ("test_unlock ger-usa seed 1 normal", 0x8b78652432150cb0),
    ("example override ger-usa seed 1 normal", 0x5871bd969965f5a6),
    ("human ger vs ai usa seed 3 easy", 0x59a3d2447901500a),
    ("unit and weapon patch ger-usa seed 1 normal", 0x35fe520d10f8ce52),
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
    let root = ww2_root();
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
    let patch = std::env::temp_dir().join(format!("frontline-s4-patch-{}.toml", std::process::id()));
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
            r(&defs, ai_vs_ai("ww2:skirmish", usa, ger, 7, Difficulty::Hard)),
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
