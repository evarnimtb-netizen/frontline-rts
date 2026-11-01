//! S3b tuning overrides (`ModDefs::apply_override`) and the tuning tables:
//! an override that restates the mod's values changes nothing, real edits
//! reach the setup, mistakes give actionable errors.

use std::path::{Path, PathBuf};

use frontline_content::{ContentError, Difficulty, MatchRequest, ModDefs};

fn ww2_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mods/ww2")
}

fn load() -> ModDefs {
    ModDefs::load(&ww2_root()).unwrap_or_else(|e| panic!("{e}"))
}

fn write(name: &str, text: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("frontline-s3b-{name}-{}.toml", std::process::id()));
    std::fs::write(&p, text).unwrap();
    p
}

fn issues(e: ContentError) -> String {
    e.to_string()
}

#[test]
fn the_example_override_restates_the_mod_and_changes_nothing() {
    let mut defs = load();
    let before = defs.clone();
    let path = ww2_root().join("tests/overrides/example.toml");
    let changes = defs
        .apply_override(&path, "ww2:skirmish")
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(changes.is_empty(), "{changes:?}");
    assert_eq!(defs, before);
    assert_eq!(defs.content_hash(), before.content_hash());
}

#[test]
fn every_def_survives_an_empty_patch() {
    // The TOML round trip the override uses must be lossless for every def
    // the mod has, or a patch would change more than it names.
    let base = load();
    let mut text = String::new();
    for id in base.squads.keys() {
        text += &format!("[defs.units.\"{id}\"]\n");
    }
    for (kind, ids) in [
        ("weapons", base.weapons.keys().collect::<Vec<_>>()),
        ("items", base.items.keys().collect()),
        ("factions", base.factions.keys().collect()),
        ("maps", base.maps.keys().collect()),
        ("vehicles", base.vehicles.keys().collect()),
        ("ammo", base.ammo.keys().collect()),
    ] {
        for id in ids {
            text += &format!("[defs.{kind}.\"{id}\"]\n");
        }
    }
    let path = write("empty", &text);
    let mut defs = base.clone();
    let changes = defs
        .apply_override(&path, "ww2:skirmish")
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(changes.is_empty(), "{changes:?}");
    assert_eq!(defs, base);
}

#[test]
fn rules_and_def_edits_reach_the_setup_and_are_listed() {
    let mut defs = load();
    let path = write(
        "edit",
        "[combat]\nnear_miss_cm = 250\n[ai.armour]\nbackup_m = 45\n\
         [defs.vehicles.\"ww2:ger_pz4_h\"]\ncost = 600\n",
    );
    let changes = defs.apply_override(&path, "ww2:skirmish").unwrap();
    let shown: Vec<String> = changes.iter().map(|c| c.to_string()).collect();
    assert_eq!(
        shown,
        [
            "defs.vehicles.\"ww2:ger_pz4_h\": cost: 640 -> 600",
            "rules \"ww2:skirmish\": ai.armour.backup_m: 30 -> 45",
            "rules \"ww2:skirmish\": combat.near_miss_cm: 200 -> 250",
        ]
    );
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let s = defs.resolve(&req).unwrap();
    assert_eq!(s.rules.ai.armour.backup_m, 45);
    assert_eq!(s.rules.combat.near_miss.raw(), 5 * 65536 / 2);
    let pz = s.vehicles.iter().find(|v| v.id == "ww2:ger_pz4_h").unwrap();
    assert_eq!(pz.cost, 600);
}

#[test]
fn override_mistakes_are_actionable() {
    let mut defs = load();
    // A typo in a key: the parser names the table and the known keys.
    let typo = write("typo", "[combat]\nnear_mis_cm = 250\n");
    let e = issues(defs.apply_override(&typo, "ww2:skirmish").unwrap_err());
    assert!(
        e.contains("near_mis_cm") && e.contains("near_miss_cm"),
        "{e}"
    );
    // Out of range: the issue names the key and a fix.
    let range = write("range", "[combat.mortar]\nflight_milli = 0\n");
    let e = issues(defs.apply_override(&range, "ww2:skirmish").unwrap_err());
    assert!(
        e.contains("combat.mortar.flight_milli") && e.contains("[combat.mortar] flight_milli = 1"),
        "{e}"
    );
    // An unknown def id suggests the closest.
    let id = write("id", "[defs.vehicles.\"ww2:ger_pz4\"]\ncost = 1\n");
    let e = issues(defs.apply_override(&id, "ww2:skirmish").unwrap_err());
    assert!(e.contains("did you mean `ww2:ger_pz4_h`"), "{e}");
    // A rules def of its own is not an override.
    let own = write("own", "base = \"ww2:skirmish\"\nid = \"ww2:x\"\n");
    let e = issues(defs.apply_override(&own, "ww2:skirmish").unwrap_err());
    assert!(e.contains("remove `base`"), "{e}");
    // A failed override leaves the defs as they were.
    assert_eq!(defs, load());
}

#[test]
fn tuning_tables_in_the_rules_check_their_ranges_on_load() {
    let mut defs = load();
    let path = write("swap", "[combat.body]\nprone_mm = 1300\n");
    let e = issues(defs.apply_override(&path, "ww2:skirmish").unwrap_err());
    assert!(e.contains("combat.body.prone_mm"), "{e}");
    // The setup re-checks them (saves are untrusted).
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let mut s = load().resolve(&req).unwrap();
    assert!(s.validate().is_empty());
    s.rules.vehicles.driving.walk_mm_s = 0;
    s.rules.dig.common.parapet_rise_mm = 0;
    let found: Vec<String> = s.validate().iter().map(|i| i.path.clone()).collect();
    assert!(
        found.contains(&"rules.vehicles.driving.walk_mm_s".to_owned())
            && found.contains(&"rules.dig.common.parapet_rise_mm".to_owned()),
        "{found:?}"
    );
}

/// The value of `table.key` in the base rules TOML (as resolved).
fn rules_value(defs: &ModDefs, table: &str, key: &str) -> u32 {
    let v = toml::Value::try_from(&defs.rules["ww2:skirmish"]).unwrap();
    let mut t = &v;
    for part in table.split('.') {
        t = &t[part];
    }
    t[key].as_integer().unwrap() as u32
}

#[test]
fn every_tuning_key_is_in_the_mod_checked_and_reaches_the_setup() {
    // For each moved constant: the mod has it; a value outside its range is
    // refused naming the key; a value 1 away changes the resolved setup
    // (so it is saved and hashed with the match).
    let base = load();
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let setup_bytes = |d: &ModDefs| postcard::to_allocvec(&d.resolve(&req).unwrap()).unwrap();
    let before = setup_bytes(&base);
    let mut n = 0;
    for (table, keys) in frontline_content::tuning::TABLES {
        for &(key, lo, hi) in *keys {
            n += 1;
            let cur = rules_value(&base, table, key);
            assert!((lo..=hi).contains(&cur), "{table}.{key} = {cur}");
            let bad = write("bad", &format!("[{table}]\n{key} = {}\n", hi as u64 + 1));
            let e = issues(
                base.clone()
                    .apply_override(&bad, "ww2:skirmish")
                    .unwrap_err(),
            );
            assert!(e.contains(&format!("{table}.{key}")), "{table}.{key}: {e}");
            let next = if cur < hi { cur + 1 } else { cur - 1 };
            let good = write("good", &format!("[{table}]\n{key} = {next}\n"));
            let mut d = base.clone();
            d.apply_override(&good, "ww2:skirmish")
                .unwrap_or_else(|e| panic!("{table}.{key} = {next}: {e}"));
            assert_ne!(
                setup_bytes(&d),
                before,
                "{table}.{key} does not reach the setup"
            );
        }
    }
    assert!(n >= 100, "{n} keys");
}
