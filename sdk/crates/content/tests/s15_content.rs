//! S15 M1-M4 (docs/slices/S15-ussr.md 4): `[matchups]` in `resolve`. No
//! USSR def is in the mod before Build B, so the tests add a stand-in
//! faction `ww2:ussr` (Germany's roster under another id) to loaded defs.
//! Each test names its control.

use std::path::{Path, PathBuf};

use frontline_content::{Difficulty, MatchRequest, MatchupsDef, ModDefs};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

const RULES: &str = "ww2:skirmish";

/// The ww2 defs plus a stand-in `ww2:ussr`, the rules' `[matchups]` set
/// by `edit`.
fn defs_with(edit: impl FnOnce(&mut MatchupsDef)) -> ModDefs {
    let mut defs = ModDefs::load(&root().join("mods/ww2")).unwrap_or_else(|e| panic!("{e}"));
    let mut ussr = defs.factions["ww2:germany"].clone();
    ussr.id = "ww2:ussr".into();
    ussr.name = "USSR".into();
    defs.factions.insert(ussr.id.clone(), ussr);
    let m = defs
        .rules
        .get_mut(RULES)
        .unwrap()
        .matchups
        .as_mut()
        .expect("the ww2 rules have [matchups]");
    edit(m);
    defs
}

fn req(a: &str, b: &str) -> MatchRequest {
    MatchRequest::skirmish(1, a, b, Difficulty::Normal)
}

/// The plan's 1944 table: the USSR enabled, Germany-USA and
/// Germany-USSR.
fn east(m: &mut MatchupsDef) {
    m.factions.push("ww2:ussr".into());
    m.pairs.push(["ww2:germany".into(), "ww2:ussr".into()]);
}

/// M1: a faction outside `factions` is refused. Control: with it listed
/// (and its pair) the same request resolves.
#[test]
fn matchup_refuses_a_faction_not_enabled() {
    let defs = defs_with(|_| {});
    let e = defs.resolve(&req("ww2:germany", "ww2:ussr")).unwrap_err();
    assert!(e.to_string().contains("not enabled"), "{e}");
    let defs = defs_with(east);
    defs.resolve(&req("ww2:germany", "ww2:ussr"))
        .expect("control: enabled and paired");
}

/// M2: USSR against the USA is not a pair. Control: Germany-USSR is.
#[test]
fn matchup_refuses_usa_ussr_without_free() {
    let defs = defs_with(east);
    let e = defs.resolve(&req("ww2:usa", "ww2:ussr")).unwrap_err();
    assert!(e.to_string().contains("allows no match"), "{e}");
    defs.resolve(&req("ww2:ussr", "ww2:germany"))
        .expect("control: a listed pair in either order");
}

/// M2 `free` 1: any two enabled factions. Control: `free` 0 refuses
/// (above); a faction not enabled is still refused.
#[test]
fn free_allows_any_two() {
    let defs = defs_with(|m| {
        east(m);
        m.free = 1;
    });
    defs.resolve(&req("ww2:usa", "ww2:ussr"))
        .expect("free skirmish");
    let defs = defs_with(|m| {
        m.free = 1;
    });
    assert!(
        defs.resolve(&req("ww2:usa", "ww2:ussr")).is_err(),
        "control: the USSR not enabled"
    );
}

/// M3: the default opponent is the other member of the first pair
/// holding the faction. Control: a faction in no pair has none.
#[test]
fn default_opponent_is_first_pair() {
    let defs = defs_with(east);
    let m = defs.rules[RULES].matchups.as_ref().unwrap();
    assert_eq!(m.default_opponent("ww2:germany"), Some("ww2:usa"));
    assert_eq!(m.default_opponent("ww2:usa"), Some("ww2:germany"));
    assert_eq!(m.default_opponent("ww2:ussr"), Some("ww2:germany"));
    let all: Vec<String> = defs.factions.keys().cloned().collect();
    assert_eq!(m.opponents("ww2:germany", &all), ["ww2:usa", "ww2:ussr"]);
    let shipped = defs_with(|_| {});
    let m0 = shipped.rules[RULES].matchups.as_ref().unwrap();
    assert_eq!(m0.default_opponent("ww2:ussr"), None, "control");
}

/// The shipped table changes nothing: Germany-USA and the mirror (the
/// CLI's mirror runs) resolve as before; the shipped rules name only
/// existing factions. Control: an unknown faction in the list is a
/// load issue.
#[test]
fn shipped_matchups_keep_the_default_pair() {
    let defs = ModDefs::load(&root().join("mods/ww2")).unwrap();
    defs.resolve(&req("ww2:germany", "ww2:usa")).unwrap();
    defs.resolve(&req("ww2:usa", "ww2:germany")).unwrap();
    defs.resolve(&req("ww2:usa", "ww2:usa")).unwrap();
    let dir = std::env::temp_dir().join(format!("frontline-s15-m-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let text = |f: &str| {
        format!(
            "id = \"ww2:s15_m\"\nname = \"S15 M\"\nbase = \"ww2:skirmish\"\n[matchups]\nfactions = [\"ww2:germany\", \"{f}\"]\n"
        )
    };
    let good = dir.join("good.toml");
    std::fs::write(&good, text("ww2:usa")).unwrap();
    defs.clone()
        .add_rules_override(&good)
        .expect("control: known factions");
    let p = dir.join("bad.toml");
    std::fs::write(&p, text("ww2:prussia")).unwrap();
    let mut d = defs.clone();
    assert!(
        d.add_rules_override(&p).is_err(),
        "unknown faction accepted"
    );
}
