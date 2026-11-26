//! S15 M1-M4 (docs/slices/S15-ussr.md 4): `[matchups]` in `resolve`. No
//! USSR def is in the mod before Build B, so the tests add a stand-in
//! faction `ww2:ussr` (Germany's roster under another id) to loaded defs.
//! Each test names its control.

use std::path::{Path, PathBuf};

use frontline_content::defs::ForeignFeedDef;
use frontline_content::variant::BodyArmourDef;
use frontline_content::vehicle::TenthsMm;
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
    plate_grenadiers(&mut defs);
    foreign_m1(&mut defs);
    bayonet_kar(&mut defs);
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
    let gren = |s: &frontline_content::MatchSetup| {
        let k = s.squad_type_index(GREN).unwrap() as usize;
        s.squad_types[k].members.iter().any(|m| m.plate.is_some())
    };
    assert!(!gren(&on), "plates still off");
    defs.rules
        .get_mut(RULES)
        .unwrap()
        .body_armour
        .as_mut()
        .unwrap()
        .enabled = 1;
    let on = defs.resolve(&r).unwrap();
    assert!(gren(&on), "control: plates with `[body_armour] enabled` 1");
    assert!(on.validate().is_empty(), "{:?}", on.validate());
    let feeds = |s: &frontline_content::MatchSetup| {
        s.weapons
            .iter()
            .filter(|w| w.foreign_feed.is_some())
            .count()
    };
    assert_eq!(feeds(&on), 0, "foreign feed still off");
    defs.rules
        .get_mut(RULES)
        .unwrap()
        .supply
        .as_mut()
        .unwrap()
        .foreign_ammo = 1;
    let on = defs.resolve(&r).unwrap();
    assert_eq!(feeds(&on), 1, "control: the feed with `foreign_ammo` 1");
    assert!(on.validate().is_empty(), "{:?}", on.validate());
    let bayonets = |s: &frontline_content::MatchSetup| {
        s.weapons.iter().filter(|w| w.bayonet.is_some()).count()
    };
    assert_eq!(bayonets(&on), 0, "bayonets still off");
    defs.rules
        .get_mut(RULES)
        .unwrap()
        .melee
        .as_mut()
        .unwrap()
        .bayonets = 1;
    let on = defs.resolve(&r).unwrap();
    assert_eq!(bayonets(&on), 1, "control: with `bayonets` 1");
    assert!(on.validate().is_empty(), "{:?}", on.validate());
    let mut off = on.clone();
    off.s15_off();
    assert!(!gren(&off), "s15_off drops the plates");
    assert_eq!(feeds(&off), 0, "s15_off drops the feed");
    assert_eq!(bayonets(&off), 0, "s15_off drops the bayonet");
}

/// The stand-in Mosin's bayonet (plan B1) on the Kar 98k: reach 140 cm.
fn bayonet_kar(defs: &mut ModDefs) {
    let m = defs
        .weapons
        .get_mut("ww2:kar98k")
        .unwrap()
        .melee
        .as_mut()
        .unwrap();
    let mut b = m.strike();
    b.reach_cm = 140;
    m.bayonet = Some(b);
}

/// B1 def checks: the bayonet block in the `[melee]` ranges. Control:
/// the stand-in is valid and `[melee.bayonet]` parses.
#[test]
fn bayonet_block_is_checked() {
    let mut defs = ModDefs::load(&root().join("mods/ww2")).unwrap();
    bayonet_kar(&mut defs);
    let ok = defs.weapons["ww2:kar98k"].clone();
    assert!(ok.validate().is_empty(), "control: {:?}", ok.validate());
    let mut far = ok.clone();
    far.melee
        .as_mut()
        .unwrap()
        .bayonet
        .as_mut()
        .unwrap()
        .reach_cm = 1001;
    assert!(!far.validate().is_empty(), "reach 1001");
    let m: frontline_content::defs::WeaponMeleeDef = toml::from_str(
        "reach_cm = 100\ndamage = 50\nwindup_ms = 600\nrecover_ms = 1000\n\
         [bayonet]\nreach_cm = 140\ndamage = 50\nwindup_ms = 600\nrecover_ms = 1000\n",
    )
    .unwrap();
    assert_eq!(m.bayonet.map(|b| b.reach_cm), Some(140));
}

/// The stand-in Soviet mortar's feed (plan X3) on the US 81 mm mortar.
fn foreign_m1(defs: &mut ModDefs) {
    defs.weapons.get_mut("ww2:m1_mortar").unwrap().foreign_feed = Some(ForeignFeedDef {
        cartridge: "ww2:8cm_wgr34".into(),
        faction: "ww2:germany".into(),
        spread_pct: 130,
    });
}

/// X1 def checks: spread 100..=400, a mortar only, a known cartridge and
/// faction. Control: the stand-in loads clean.
#[test]
fn foreign_feed_is_checked() {
    let mut defs = ModDefs::load(&root().join("mods/ww2")).unwrap();
    foreign_m1(&mut defs);
    let ok = defs.weapons["ww2:m1_mortar"].clone();
    assert!(ok.validate().is_empty(), "control: {:?}", ok.validate());
    let mut wide = ok.clone();
    wide.foreign_feed.as_mut().unwrap().spread_pct = 401;
    assert!(!wide.validate().is_empty(), "spread 401");
    let mut rifle = defs.weapons["ww2:kar98k"].clone();
    rifle.foreign_feed = ok.foreign_feed.clone();
    assert!(!rifle.validate().is_empty(), "a rifle");
    // Cross-file names: the whole set's check.
    let issues = |d: &ModDefs| d.cross_check();
    assert!(issues(&defs).is_empty(), "control: {:?}", issues(&defs));
    let mut bad = defs.clone();
    bad.weapons
        .get_mut("ww2:m1_mortar")
        .unwrap()
        .foreign_feed
        .as_mut()
        .unwrap()
        .cartridge = "ww2:8cm_wgr99".into();
    assert!(!issues(&bad).is_empty(), "unknown cartridge");
    let mut bad = defs.clone();
    bad.weapons
        .get_mut("ww2:m1_mortar")
        .unwrap()
        .foreign_feed
        .as_mut()
        .unwrap()
        .faction = "ww2:prussia".into();
    assert!(!issues(&bad).is_empty(), "unknown faction");
}

const GREN: &str = "ww2:ger_grenadiergruppe";

/// A stand-in 2.0 mm torso plate (120 deg) on every Grenadiergruppe man.
fn plate_grenadiers(defs: &mut ModDefs) {
    for m in &mut defs.squads.get_mut(GREN).unwrap().members {
        m.armour = Some(BodyArmourDef {
            mm: TenthsMm(20),
            zone: "torso".into(),
            arc_deg: 120,
        });
    }
}

/// P1 def checks: thickness 0.5..=10 mm, the torso only, arc 1..=360.
/// Control: the stand-in plate is valid.
#[test]
fn member_plates_are_checked() {
    let mut defs = ModDefs::load(&root().join("mods/ww2")).unwrap();
    plate_grenadiers(&mut defs);
    let ok = defs.squads[GREN].clone();
    assert!(ok.validate().is_empty(), "control: {:?}", ok.validate());
    let bad = |edit: &dyn Fn(&mut BodyArmourDef)| {
        let mut s = ok.clone();
        edit(s.members[0].armour.as_mut().unwrap());
        !s.validate().is_empty()
    };
    assert!(bad(&|a| a.mm = TenthsMm(4)), "0.4 mm");
    assert!(bad(&|a| a.mm = TenthsMm(101)), "10.1 mm");
    assert!(bad(&|a| a.zone = "head".into()), "head");
    assert!(bad(&|a| a.arc_deg = 0), "arc 0");
    assert!(bad(&|a| a.arc_deg = 361), "arc 361");
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
