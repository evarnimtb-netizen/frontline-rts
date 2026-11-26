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

/// Stand-in rider slots on the Panzer IV H's def (ten deck boxes).
fn pz4_riders(defs: &mut ModDefs) {
    use frontline_content::vehicle::{BoxDef, PassengerSlotDef, RidersDef};
    let v = defs.vehicles.get_mut("ww2:ger_pz4_h").unwrap();
    v.riders = Some(RidersDef {
        capacity: 10,
        entry: [-300, 0, 60],
        board_ms: 1000,
    });
    v.rider_slots = [-120, -170, -220, -265]
        .into_iter()
        .flat_map(|x| [(x, 60), (x, -60)])
        .chain([(-145, 0), (-245, 0)])
        .map(|(x, y)| PassengerSlotDef {
            bounds: BoxDef {
                center: [x, y, 221],
                half: [22, 20, 45],
            },
        })
        .collect();
}

/// Plan N2: with every S15 switch 0 the setup build drops the S15 def
/// parts before anything reads them: a mod whose Panzer IV has rider
/// slots resolves to exactly today's setup. Control: `[riders] enabled` 1
/// gives the tank its (exposed) slots.
#[test]
fn s15_off_setup_has_no_s15_field() {
    let base_defs = ModDefs::load(&root().join("mods/ww2")).unwrap();
    let r = req("ww2:germany", "ww2:usa");
    let base = base_defs.resolve(&r).unwrap();
    let mut defs = base_defs.clone();
    pz4_riders(&mut defs);
    for (id, v) in &defs.vehicles {
        assert!(v.validate().is_empty(), "{id}: {:?}", v.validate());
    }
    let off = defs.resolve(&r).unwrap();
    assert_eq!(off, base, "switches off: today's setup");
    defs.rules
        .get_mut(RULES)
        .unwrap()
        .riders
        .as_mut()
        .unwrap()
        .enabled = 1;
    let on = defs.resolve(&r).unwrap();
    let pz = on
        .vehicles
        .iter()
        .find(|v| v.id == "ww2:ger_pz4_h")
        .unwrap();
    let t = pz
        .transport
        .as_ref()
        .expect("control: rider slots with the switch on");
    assert!(t.exposed && t.slots.len() == 10);
    assert!(on.validate().is_empty(), "{:?}", on.validate());
}

/// R1 def checks: `[riders]` only on a tank, assault gun or TD, one box
/// per rider, each box on the deck. Control: the stand-in is valid.
#[test]
fn rider_slots_are_checked() {
    let mut defs = ModDefs::load(&root().join("mods/ww2")).unwrap();
    pz4_riders(&mut defs);
    let ok = defs.vehicles["ww2:ger_pz4_h"].clone();
    assert!(ok.validate().is_empty(), "control");
    let mut few = ok.clone();
    few.rider_slots.pop();
    assert!(!few.validate().is_empty(), "nine boxes for ten riders");
    let mut inside = ok.clone();
    inside.rider_slots[0].bounds.center = [0, 0, 108];
    assert!(!inside.validate().is_empty(), "a box inside the hull");
    let mut truck = defs.vehicles["ww2:ger_opel_blitz"].clone();
    truck.riders = ok.riders.clone();
    truck.rider_slots = ok.rider_slots.clone();
    assert!(!truck.validate().is_empty(), "riders on a truck");
}
