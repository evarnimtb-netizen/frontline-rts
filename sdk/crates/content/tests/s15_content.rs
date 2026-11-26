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
/// to the S15a table (Germany and the USA only; S15-ai ships the USSR
/// enabled) and then by `edit`.
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
    m.factions = vec!["ww2:germany".into(), "ww2:usa".into()];
    m.pairs = vec![["ww2:germany".into(), "ww2:usa".into()]];
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
/// existing factions. S15-ai: the shipped table also enables the USSR
/// against Germany (Germany's default opponent stays the USA) and still
/// refuses the USSR against the USA. Control: an unknown faction in the
/// list is a load issue.
#[test]
fn shipped_matchups_keep_the_default_pair() {
    let defs = ModDefs::load(&root().join("mods/ww2")).unwrap();
    defs.resolve(&req("ww2:germany", "ww2:usa")).unwrap();
    defs.resolve(&req("ww2:usa", "ww2:germany")).unwrap();
    defs.resolve(&req("ww2:usa", "ww2:usa")).unwrap();
    defs.resolve(&req("ww2:germany", "ww2:ussr")).unwrap();
    assert!(defs.resolve(&req("ww2:usa", "ww2:ussr")).is_err());
    let m = defs.rules[RULES].matchups.as_ref().unwrap();
    assert_eq!(m.default_opponent("ww2:germany"), Some("ww2:usa"));
    assert_eq!(m.default_opponent("ww2:ussr"), Some("ww2:germany"));
    let dir = std::env::temp_dir().join(format!("frontline-s15-m-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let text = |f: &str| {
        format!(
            "id = \"ww2:s15_m\"\nname = \"S15 M\"\nbase = \"ww2:skirmish\"\n[matchups]\nfactions = [\"ww2:germany\", \"{f}\"]\npairs = [[\"ww2:germany\", \"{f}\"]]\n"
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
    let mut base_defs = ModDefs::load(&root().join("mods/ww2")).unwrap();
    // S15-ai ships the switches on: the test turns them off first.
    s15_switches(&mut base_defs, 0);
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
    // (The real USSR defs, in every setup since S15 Build B, have their own
    // feed and bayonet; the stand-ins on the German and US defs are counted.)
    let feeds = |s: &frontline_content::MatchSetup| {
        s.weapons
            .iter()
            .filter(|w| w.foreign_feed.is_some() && !w.id.starts_with("ww2:ussr_"))
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
        s.weapons
            .iter()
            .filter(|w| w.bayonet.is_some() && !w.id.starts_with("ww2:ussr_"))
            .count()
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

// ------------------------------------------- Build B: the real USSR defs --

/// Sets every S15 switch of every rules def to 1 (or 0).
fn s15_switches(defs: &mut ModDefs, on: u32) {
    for r in defs.rules.values_mut() {
        r.riders.get_or_insert_with(Default::default).enabled = on;
        r.body_armour.get_or_insert_with(Default::default).enabled = on;
        r.supply.as_mut().expect("[supply]").foreign_ammo = on;
        r.melee.get_or_insert_with(Default::default).bayonets = on;
    }
}

/// Plan N1 (the Append rule): every id S15 adds starts `ww2:ussr` and
/// sorts after every older id, so the Germany-USA setup is the setup
/// without the USSR defs with the USSR entries appended at the end of each
/// list: nothing is renumbered, no German or US def gained a key. Checked
/// with the S15 switches off and with them on (T210, T211 in the small).
/// Control: the USSR entries exist in each list (the check is not empty).
#[test]
fn s15_defs_sort_last() {
    for on in [0, 1] {
        let mut full = ModDefs::load(&root().join("mods/ww2")).unwrap();
        s15_switches(&mut full, on);
        let mut base = full.clone();
        base.squads.retain(|k, _| !k.starts_with("ww2:ussr"));
        base.weapons.retain(|k, _| !k.starts_with("ww2:ussr"));
        base.items.retain(|k, _| !k.starts_with("ww2:ussr"));
        base.vehicles.retain(|k, _| !k.starts_with("ww2:ussr"));
        base.ammo.retain(|k, _| !k.starts_with("ww2:ussr"));
        base.cartridges.retain(|k, _| !k.starts_with("ww2:ussr"));
        base.supply.retain(|k, _| !k.starts_with("ww2:ussr"));
        base.factions.retain(|k, _| !k.starts_with("ww2:ussr"));
        let r = req("ww2:germany", "ww2:usa");
        let mut got = full.resolve(&r).unwrap();
        let want = base.resolve(&r).unwrap();
        let mut seen = Vec::new();
        macro_rules! list {
            ($f:ident) => {{
                let ids: Vec<String> = got.$f.iter().map(|d| d.id.clone()).collect();
                let first = ids.iter().position(|i| i.starts_with("ww2:ussr"));
                let old = ids.iter().filter(|i| !i.starts_with("ww2:ussr")).count();
                assert_eq!(
                    first.unwrap_or(old),
                    old,
                    "{}: USSR ids come last",
                    stringify!($f)
                );
                assert!(
                    ids[old..].iter().all(|i| i.starts_with("ww2:ussr")),
                    "{}",
                    stringify!($f)
                );
                seen.push((stringify!($f), ids.len() - old));
                got.$f.truncate(old);
            }};
        }
        list!(squad_types);
        list!(weapons);
        list!(items);
        list!(vehicles);
        list!(ammo);
        list!(cartridges);
        list!(stocks);
        list!(factions);
        // S15 Build C: the vehicles and ammo lists gain their USSR entries too.
        for (name, n) in &seen {
            assert!(*n > 0, "{name}: no USSR entry");
        }
        // The tag list is sorted: the one USSR tag (`vehicle_riders`) sorts
        // after every older one, so the list only grows at its end.
        assert!(got.tags.starts_with(&want.tags), "no tag index moved");
        assert!(
            got.tags[want.tags.len()..]
                .iter()
                .all(|t| t == "vehicle_riders")
        );
        got.tags.truncate(want.tags.len());
        macro_rules! same {
            ($($f:ident),*) => {$( assert!(got.$f == want.$f, "switches {on}: {} changed", stringify!($f)); )*};
        }
        same!(
            weapons,
            items,
            squad_types,
            factions,
            ammo,
            vehicles,
            tags,
            training,
            cartridges,
            stocks,
            forts,
            rules,
            map,
            players
        );
        assert!(got == want, "switches {on}: the old entries changed");
    }
}

fn assault_group(s: &frontline_content::MatchSetup) -> &frontline_content::setup::SquadTypeSetup {
    &s.squad_types[s.squad_type_index("ww2:ussr_shturmovaya_gruppa").unwrap() as usize]
}

/// P1 data: with `[body_armour] enabled` 1 every man of the assault group
/// wears the 2 mm torso plate (the SN-42) and no man of any other card
/// does; with 0 nobody does (control: the plate is in the def).
#[test]
fn only_the_assault_group_wears_a_plate() {
    let mut defs = ModDefs::load(&root().join("mods/ww2")).unwrap();
    s15_switches(&mut defs, 1);
    let on = defs.resolve(&req("ww2:germany", "ww2:usa")).unwrap();
    for t in &on.squad_types {
        let plated = t.members.iter().filter(|m| m.plate.is_some()).count();
        if t.id == "ww2:ussr_shturmovaya_gruppa" {
            assert_eq!(plated, t.members.len(), "every man of the group");
            assert!(t.members.iter().all(|m| m.plate.unwrap().tenths_mm == 20));
        } else {
            assert_eq!(plated, 0, "{}", t.id);
        }
    }
    s15_switches(&mut defs, 0);
    let off = defs.resolve(&req("ww2:germany", "ww2:usa")).unwrap();
    assert!(
        assault_group(&off)
            .members
            .iter()
            .all(|m| m.plate.is_none()),
        "control: switch 0 drops the plates"
    );
}

/// S14 4.2 on the ROKS-3 pair: with `[flame] enabled` 0 the assault group
/// has no flame man and no `flame` capability (every man a PPSh-41 man);
/// with 1 it has one ROKS-3 operator and a `crew_assistant`. Control: the
/// flame-on group differs.
#[test]
fn roks3_pair_off_is_no_flame_man() {
    use frontline_content::setup::Capability;
    let roks = |defs: &ModDefs, on: u32| {
        let mut d = defs.clone();
        d.rules
            .values_mut()
            .for_each(|r| r.flame.get_or_insert_with(Default::default).enabled = on);
        let s = d.resolve(&req("ww2:germany", "ww2:usa")).unwrap();
        let w = s
            .weapons
            .iter()
            .position(|w| w.id == "ww2:ussr_roks3")
            .unwrap();
        let t = assault_group(&s);
        let n = t.members.iter().filter(|m| m.weapon as usize == w).count();
        (n, t.men as usize, t.has(Capability::Flame))
    };
    let defs = ModDefs::load(&root().join("mods/ww2")).unwrap();
    assert_eq!(roks(&defs, 0), (0, 8, false), "flame off: eight PPSh men");
    assert_eq!(roks(&defs, 1), (1, 8, true), "control: flame on");
}

/// The Mosin carries both strikes: the butt always, the bayonet only with
/// `[melee] bayonets` 1 (the setup drops it at 0).
#[test]
fn mosin_has_butt_and_bayonet_blocks() {
    let mut defs = ModDefs::load(&root().join("mods/ww2")).unwrap();
    let m = defs.weapons["ww2:ussr_mosin_9130"].melee.unwrap();
    assert_eq!((m.reach_cm, m.bayonet.unwrap().reach_cm), (100, 180));
    s15_switches(&mut defs, 1);
    let on = defs.resolve(&req("ww2:germany", "ww2:usa")).unwrap();
    let k = on
        .weapons
        .iter()
        .position(|w| w.id == "ww2:ussr_mosin_9130")
        .unwrap();
    assert!(on.weapons[k].bayonet.is_some());
    let kar = on
        .weapons
        .iter()
        .position(|w| w.id == "ww2:kar98k")
        .unwrap();
    assert!(on.weapons[kar].bayonet.is_none(), "no German bayonet");
    let sn = on
        .weapons
        .iter()
        .position(|w| w.id == "ww2:ussr_82mm_bm37")
        .unwrap();
    assert!(on.weapons[sn].foreign_feed.is_some());
    let grw = on.weapons.iter().position(|w| w.id == "ww2:grw34").unwrap();
    assert!(on.weapons[grw].foreign_feed.is_none(), "X3: one way only");
}

/// S15 Build C (plan 6.3, 6.4) on the real defs: the nine Soviet AP and
/// APCR rounds carry the S13 slope class (HE rounds none); each gun card
/// comes with the US6 as its tow; the vehicle cards carry the plan's AI
/// weights, the IS-2 is the one Soviet heavy card; every Soviet supply
/// def cooks off with the Soviet HE round. Control: the German and US
/// lists the S13 test pins are unchanged (that test filters `ussr_`).
#[test]
fn s15c_rounds_cards_and_tows() {
    use frontline_content::vehicle::SlopeClass;
    let defs = ModDefs::load(&root().join("mods/ww2")).unwrap();
    let s = defs.resolve(&req("ww2:germany", "ww2:usa")).unwrap();
    let mut classed: Vec<(&str, SlopeClass)> = s
        .ammo
        .iter()
        .filter(|a| a.id.starts_with("ww2:ussr_"))
        .filter_map(|a| a.slope_class.map(|c| (a.id.as_str(), c)))
        .collect();
    classed.sort_by_key(|&(id, _)| id);
    assert_eq!(
        classed,
        [
            ("ww2:ussr_122mm_br471", SlopeClass::Apcbc),
            ("ww2:ussr_45mm_br240", SlopeClass::Apcbc),
            ("ww2:ussr_45mm_br240p", SlopeClass::Apcr),
            ("ww2:ussr_57mm_br271", SlopeClass::Apcbc),
            ("ww2:ussr_57mm_br271p", SlopeClass::Apcr),
            ("ww2:ussr_76mm_br350a", SlopeClass::Apcbc),
            ("ww2:ussr_76mm_br350p", SlopeClass::Apcr),
            ("ww2:ussr_85mm_br365", SlopeClass::Apcbc),
            ("ww2:ussr_85mm_br365p", SlopeClass::Apcr),
        ][..]
    );
    let card = |id: &str| {
        let k = s.squad_type_index(id).unwrap_or_else(|| panic!("{id}")) as usize;
        &s.squad_types[k]
    };
    let us6 = s.squad_type_index("ww2:ussr_us6_squad").unwrap();
    for g in [
        "ww2:ussr_45mm_m42_squad",
        "ww2:ussr_57mm_zis2_squad",
        "ww2:ussr_76mm_zis3_squad",
        "ww2:ussr_76mm_m1943_squad",
        "ww2:ussr_122mm_m30_squad",
    ] {
        assert_eq!(card(g).tow_card, Some(us6), "{g}: towed by the US6");
    }
    for (id, wt) in [
        ("ww2:ussr_t34_85_squad", 100),
        ("ww2:ussr_su85_squad", 100),
        ("ww2:ussr_su76m_squad", 50),
        ("ww2:ussr_is2_squad", 0),
        ("ww2:ussr_ba64b_squad", 5),
        ("ww2:ussr_us6_squad", 5),
        ("ww2:ussr_m3a1_scout_squad", 100),
    ] {
        assert_eq!(card(id).ai_weight, wt, "{id}");
    }
    let heavy: Vec<&str> = s
        .squad_types
        .iter()
        .filter(|t| t.ai_heavy && t.id.starts_with("ww2:ussr_"))
        .map(|t| t.id.as_str())
        .collect();
    assert_eq!(heavy, ["ww2:ussr_is2_squad"]);
    for (id, d) in &defs.supply {
        if id.starts_with("ww2:ussr_")
            && let Some(c) = &d.cookoff
        {
            assert_eq!(c, "ww2:ussr_76mm_of350", "{id}");
        }
    }
    for (id, v) in &defs.vehicles {
        if !id.starts_with("ww2:ussr_") {
            assert!(v.riders.is_none() && v.rider_slots.is_empty(), "{id}");
        }
    }
}
