//! S11b Build A (docs/slices/S11b.md 2, 4 G1): the new `[ai.arty]` and
//! `[items]` keys parse from the ww2 rules with every switch 0, the
//! `s11b_off` control file applies, every key is range-checked (overrides
//! and untrusted saves), and the rifle-grenade item def needs an HE
//! launcher.

use std::path::{Path, PathBuf};

use frontline_content::tuning::AiArtyTuning;
use frontline_content::{Difficulty, ItemKind, MatchRequest, ModDefs};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn write(name: &str, text: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("frontline-s11b-{name}-{}.toml", std::process::id()));
    std::fs::write(&p, text).unwrap();
    p
}

/// One out-of-range case per new key (spec 2 ranges). Control: widen one
/// range (seen with `how_max: 0..=9`) and that case is accepted, so the
/// test fails.
#[test]
fn load_rejects_bad_s11b_keys() {
    let mut defs = ModDefs::load(&root().join("mods/ww2")).unwrap_or_else(|e| panic!("{e}"));
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let s = defs.resolve(&req).unwrap();
    let a = &s.rules.ai.arty;
    // Build C: ww2 turns every switch on, the other keys at the spec values;
    // the fix stage turns the AI's light mortars off (S11b-14).
    assert_eq!(
        (a.cb_alternate, a.buy_pending, a.how, a.light),
        (1, 1, 1, 0)
    );
    assert_eq!(
        (a.how_after_s, a.how_max, a.how_income_pct, a.how_reserve_mp),
        (480, 1, 15, 300)
    );
    assert_eq!(
        (
            s.rules.items.rifle_grenades,
            s.rules.items.rifle_grenade_gap_ticks
        ),
        (1, 25 * 30)
    );
    // Absent keys read the same values.
    let d = AiArtyTuning::default();
    assert_eq!(
        (d.cb_alternate, d.buy_pending, d.how, d.light),
        (0, 0, 0, 0)
    );
    assert_eq!(
        (d.how_after_s, d.how_max, d.how_income_pct, d.how_reserve_mp),
        (480, 1, 15, 300)
    );
    // The control file applies (its keys exist and are in range).
    let off = root().join("docs/slices/targets/S11b-ablations/s11b_off.toml");
    defs.apply_override(&off, "ww2:skirmish")
        .unwrap_or_else(|e| panic!("{e}"));
    // An override reaches the setup.
    let on = write(
        "on",
        "[ai.arty]\ncb_alternate = 1\nhow_reserve_mp = 100000\n[items]\nrifle_grenades = 1\nrifle_grenade_gap_s = 600\n",
    );
    defs.apply_override(&on, "ww2:skirmish")
        .unwrap_or_else(|e| panic!("{e}"));
    let s = defs.resolve(&req).unwrap();
    assert_eq!(
        (s.rules.ai.arty.cb_alternate, s.rules.ai.arty.how_reserve_mp),
        (1, 100_000)
    );
    assert_eq!(
        (
            s.rules.items.rifle_grenades,
            s.rules.items.rifle_grenade_gap_ticks
        ),
        (1, 600 * 30)
    );
    // Out of range: one case per key; the issue names the key.
    let cases: Vec<(&str, &str, u32)> = vec![
        ("ai.arty", "cb_alternate", 2),
        ("ai.arty", "buy_pending", 2),
        ("ai.arty", "how", 2),
        ("ai.arty", "how_after_s", 3601),
        ("ai.arty", "how_max", 9),
        ("ai.arty", "how_income_pct", 101),
        ("ai.arty", "how_reserve_mp", 100_001),
        ("ai.arty", "light", 2),
        ("items", "rifle_grenades", 2),
        ("items", "rifle_grenade_gap_s", 601),
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
    // The setup is checked too (saves are untrusted).
    let mut s = defs.resolve(&req).unwrap();
    assert!(s.validate().is_empty());
    s.rules.ai.arty.how_max = 9;
    assert!(!s.validate().is_empty());
    let mut s = defs.resolve(&req).unwrap();
    s.rules.items.rifle_grenades = 2;
    assert!(!s.validate().is_empty());
    let mut s = defs.resolve(&req).unwrap();
    s.rules.items.rifle_grenade_gap_ticks = 600 * 30 + 1;
    assert!(!s.validate().is_empty());
}

/// G1: a `rifle_grenade` item names a launcher whose round is HE (with an
/// `[he]` table) and is refilled as shells; the item itself is never
/// thrown. Control: the ww2 def (M7, M17 HE) passes; pointed at the
/// Panzerfaust (a HEAT round) it is refused, and with no `weapon` too.
#[test]
fn rifle_grenade_item_needs_an_he_launcher() {
    const IT: &str = "ww2:m7_rifle_grenade";
    let defs = ModDefs::load(&root().join("mods/ww2")).unwrap_or_else(|e| panic!("{e}"));
    let it = &defs.items[IT];
    assert_eq!(it.kind, ItemKind::RifleGrenade);
    assert!(!ItemKind::RifleGrenade.thrown());
    assert_eq!(
        (it.range_m, it.min_range_m, it.windup_ms, it.stow_ms),
        (180, 30, 3000, 3000)
    );
    let mine = |d: &ModDefs| {
        d.cross_check()
            .into_iter()
            .filter(|i| i.path.contains("m7_rifle_grenade"))
            .count()
    };
    assert_eq!(mine(&defs), 0, "control: the M7 with the M17 HE round");
    // A HEAT launcher is refused.
    let mut heat = defs.clone();
    heat.items.get_mut(IT).unwrap().weapon = Some("ww2:pzf60".into());
    assert_eq!(mine(&heat), 1, "a HEAT launcher");
    // A round refilled as anything but shells is refused.
    let mut kind = defs.clone();
    kind.items
        .get_mut(IT)
        .unwrap()
        .supply
        .as_mut()
        .unwrap()
        .kind = "explosives".into();
    assert_eq!(mine(&kind), 1, "explosives");
    // No weapon at all: the def check refuses it.
    let mut none = defs.items[IT].clone();
    none.weapon = None;
    assert!(none.validate().iter().any(|i| i.path == "weapon"));
    assert!(defs.items[IT].validate().is_empty());
    // A range beyond 300 m or a stow time on another kind are refused.
    let mut far = defs.items[IT].clone();
    far.range_m = 301;
    assert!(far.validate().iter().any(|i| i.path == "range_m"));
    let mut frag = defs.items["ww2:mk2_grenade"].clone();
    frag.stow_ms = 1000;
    assert!(frag.validate().iter().any(|i| i.path == "stow_ms"));
}

/// G1 issue rule: with `[items] rifle_grenades` 0 the setup build drops the
/// `rifle_grenade` kind from every squad type (no item index, member
/// counts 0). Control 1: the slot's kit holds 3. (No card carries the
/// grenade before Build B: the test gives the US riflemen three.)
#[test]
fn switch_off_issues_no_rifle_grenades() {
    let mut defs = ModDefs::load(&root().join("mods/ww2")).unwrap_or_else(|e| panic!("{e}"));
    let sq = defs.squads.get_mut("ww2:usa_rifle_squad").unwrap();
    let slot = sq
        .members
        .iter_mut()
        .find(|m| m.slot == "rifleman")
        .expect("a rifleman slot");
    slot.items.insert("ww2:m7_rifle_grenade".into(), 3);
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let kit = |rg: u32| {
        let mut d = defs.clone();
        d.rules
            .get_mut("ww2:skirmish")
            .unwrap()
            .items
            .rifle_grenades = rg;
        let s = d.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
        let t = &s.squad_types[s.squad_type_index("ww2:usa_rifle_squad").unwrap() as usize];
        let k = t.members.iter().position(|m| m.slot == "rifleman").unwrap() as u8;
        (
            t.items[ItemKind::RifleGrenade.index()].is_some(),
            t.full_kit(k)[ItemKind::RifleGrenade.index()],
            t.squad_items(ItemKind::RifleGrenade),
        )
    };
    assert_eq!(kit(0), (false, 0, 0), "switch 0: no grenade issued");
    assert_eq!(kit(1), (true, 3, 15), "control: five riflemen, three each");
}

// ------------------------------------------------------ Build B content --

fn ww2() -> ModDefs {
    ModDefs::load(&root().join("mods/ww2")).unwrap_or_else(|e| panic!("{e}"))
}

/// C1-C4 (and C6, C7 when built): every new def loads at the figure spec 4
/// cites; one assert per cited design line (D = artillery-on-map.md, GA =
/// guns-artillery.md, RE = roster-expansion.md). Control: any def off its
/// cited figure fails the matching line.
#[test]
fn s11b_defs_load_with_the_cited_figures() {
    use frontline_content::VehicleRole;
    let d = ww2();
    assert!(d.cross_check().is_empty(), "{:?}", d.cross_check());
    // C1 leFH 18 (D 143, 395, 721, 726-727, 809, 812, 837, 842).
    let v = &d.vehicles["ww2:ger_lefh18"];
    assert!(v.validate().is_empty(), "{:?}", v.validate());
    assert_eq!(
        (v.role, v.mass_kg),
        (VehicleRole::InfGun, Some(1985)),
        "D 143"
    );
    assert_eq!(v.weapons[0].elevation_deg, Some([-5, 42]), "D 143");
    assert_eq!(
        (v.weapons[0].ammo[0].id.as_str(), v.weapons[0].ammo[0].count),
        ("ww2:105mm_fhgr38", 12),
        "D 395"
    );
    let m = &v.mobility;
    assert_eq!(
        (m.emplace_s, m.pack_s, m.hitch_s),
        (Some(90), Some(54), Some(25)),
        "D 721"
    );
    assert_eq!(m.relay_trail_s, Some(25), "D 726-727");
    assert!(v.boxes.iter().any(|b| b.id == "shield"), "D 809");
    assert_eq!(v.spot_size_pct, 130, "D 812");
    assert_eq!(
        (v.cost, v.recall_cooldown_s, v.initial_cooldown_s),
        (480, 180, 480),
        "D 837"
    );
    assert_eq!(v.pop, 2, "D 842");
    let w = &d.weapons["ww2:lefh18"];
    assert_eq!((w.rounds_per_min, w.reload_ms), (4, 15000), "D 143");
    let ind = w.indirect.as_ref().unwrap();
    assert_eq!(
        (ind.range_m, ind.elevation_deg),
        (Some(10700), [0, 42]),
        "D 143"
    );
    assert_eq!(ind.charges_m_s[0], 200, "D 757");
    let crew = &d.squads["ww2:ger_lefh18_crew"];
    assert_eq!(
        crew.members.iter().map(|m| m.count as u32).sum::<u32>(),
        6,
        "D 143"
    );
    assert_eq!(d.squads["ww2:ger_lefh18_squad"].max_alive, 1, "D 837");
    // C2 105 mm M2A1 (D 156, 396, 722, 750, 809, 812, 838, 842).
    let v = &d.vehicles["ww2:usa_105mm_m2a1"];
    assert!(v.validate().is_empty(), "{:?}", v.validate());
    assert_eq!(
        (v.role, v.mass_kg),
        (VehicleRole::InfGun, Some(2260)),
        "D 156"
    );
    assert_eq!(v.weapons[0].elevation_deg, Some([-5, 65]), "D 156");
    assert_eq!(
        (v.weapons[0].ammo[0].id.as_str(), v.weapons[0].ammo[0].count),
        ("ww2:105mm_m1_he", 12),
        "D 396"
    );
    let m = &v.mobility;
    assert_eq!(
        (m.emplace_s, m.pack_s, m.hitch_s, m.relay_trail_s),
        (Some(90), Some(54), Some(25), Some(25)),
        "D 722"
    );
    assert!(v.boxes.iter().any(|b| b.id == "shield"), "D 809");
    assert_eq!(v.spot_size_pct, 130, "D 812");
    assert_eq!(
        (v.cost, v.recall_cooldown_s, v.initial_cooldown_s),
        (500, 180, 480),
        "D 838"
    );
    assert_eq!(v.pop, 2, "D 842");
    let w = &d.weapons["ww2:m2a1_105mm"];
    assert_eq!((w.rounds_per_min, w.reload_ms), (3, 20000), "D 156");
    let ind = w.indirect.as_ref().unwrap();
    assert_eq!(
        (ind.range_m, ind.elevation_deg),
        (Some(11300), [0, 65]),
        "D 156"
    );
    assert_eq!(
        ind.charges_m_s,
        [205, 220, 240, 270, 311, 333],
        "D 750 (charge 1); the M3 table plus 333"
    );
    let crew = &d.squads["ww2:usa_105mm_m2a1_crew"];
    assert_eq!(
        crew.members.iter().map(|m| m.count as u32).sum::<u32>(),
        8,
        "D 156"
    );
    assert_eq!(d.squads["ww2:usa_105mm_m2a1_squad"].max_alive, 1, "D 838");
    // No push (D 721-722): the smallest speed the setup check allows, and
    // more men needed than the crew has.
    for (id, crew) in [("ww2:ger_lefh18", 6), ("ww2:usa_105mm_m2a1", 8)] {
        let m = &d.vehicles[id].mobility;
        assert_eq!((m.speed_cm_s, m.reverse_cm_s), (1, 1), "{id}");
        assert!(m.push_men.unwrap() > crew, "{id}");
    }
    // C3 FH Gr 38 (D 143 filler, D 509 complete round, D 1051 rays).
    let a = &d.ammo["ww2:105mm_fhgr38"];
    let he = a.he.as_ref().unwrap();
    assert_eq!(a.supply.as_ref().map(|s| s.g), Some(19000), "D 509");
    assert_eq!(he.frag_count, 160, "D 1051");
    assert_eq!(
        (
            he.blast_radius_cm,
            he.frag_reach_cm,
            he.suppression_radius_m,
            he.crater_radius_cm,
            he.crater_depth_cm
        ),
        (170, 1700, 17, 85, 43),
        "D 143: Sprgr 34 x 1.42"
    );
    // C4 light mortars (GA 591-592, 726, 730-731; RE 135, 179).
    let kz = &d.weapons["ww2:kz_grw42"];
    assert_eq!(
        (kz.rounds_per_min, kz.min_range_m, kz.range_m),
        (15, 60, 275),
        "GA 726"
    );
    assert_eq!(kz.feed.as_deref(), Some("ww2:8cm_wgr34"), "GA 726");
    let m2 = &d.weapons["ww2:m2_60mm"];
    assert_eq!(
        (m2.rounds_per_min, m2.min_range_m, m2.range_m),
        (18, 90, 370),
        "GA 730"
    );
    assert_eq!(d.cartridges["ww2:60mm_m49a2"].supply.g, 1330, "GA 730");
    let he = m2.he.as_ref().unwrap();
    assert_eq!(
        (
            he.blast_radius_cm,
            he.frag_count,
            he.frag_reach_cm,
            he.suppression_radius_m
        ),
        (113, 90, 1130, 11),
        "GA 730-731: the 81 mm block x 0.75"
    );
    let bombs = |id: &str, feed: &str| -> (u32, u32) {
        let s = &d.squads[id];
        (
            s.members.iter().map(|m| m.count as u32).sum(),
            s.members
                .iter()
                .map(|m| m.count as u32 * m.rounds.get(feed).copied().unwrap_or(0))
                .sum(),
        )
    };
    assert_eq!(
        bombs("ww2:ger_kz_grw42_team", "ww2:8cm_wgr34"),
        (4, 18),
        "GA 592"
    );
    assert_eq!(
        bombs("ww2:usa_60mm_mortar_squad", "ww2:60mm_m49a2"),
        (5, 30),
        "GA 591"
    );
    let kz = &d.squads["ww2:ger_kz_grw42_team"];
    assert_eq!(
        (kz.cost, kz.cooldown_s),
        (181, Some(45)),
        "RE 135 (x 0.8 x 1.03)"
    );
    let us = &d.squads["ww2:usa_60mm_mortar_squad"];
    assert_eq!((us.cost, us.cooldown_s), (240, Some(45)), "RE 179");
    // C6 75 mm pack howitzer, data only (D 157, D 720): in no roster.
    let v = &d.vehicles["ww2:usa_75mm_m1a1"];
    assert!(v.validate().is_empty(), "{:?}", v.validate());
    assert_eq!(
        (v.mass_kg, v.weapons[0].elevation_deg, v.mobility.speed_cm_s),
        (Some(653), Some([-5, 45]), 90),
        "D 157, D 720"
    );
    let w = &d.weapons["ww2:m1a1_75mm"];
    assert_eq!(
        (w.rounds_per_min, w.indirect.as_ref().unwrap().range_m),
        (3, Some(8800)),
        "D 157"
    );
    assert!(
        !d.squads
            .values()
            .any(|s| s.vehicle.as_deref() == Some("ww2:usa_75mm_m1a1")),
        "no card"
    );
    // C7 (D 848): the artillery group of 2 holds the four gun cards.
    let r = &d.rules["ww2:skirmish"];
    assert_eq!(
        r.limit_groups.get("ww2:artillery").copied(),
        Some(2),
        "D 848"
    );
    for card in [
        "ww2:ger_leig18_squad",
        "ww2:usa_105mm_m3_squad",
        "ww2:ger_lefh18_squad",
        "ww2:usa_105mm_m2a1_squad",
    ] {
        assert_eq!(
            d.squads[card].limit_group.as_deref(),
            Some("ww2:artillery"),
            "{card}"
        );
    }
}

/// C1-C2: both howitzers are `inf_gun` cards with a low-angle `[indirect]`
/// table, tagged `field_howitzer`, at weight 0, last but one in each roster
/// before the light mortar. Control: without `low_angle` the leFH 18
/// (+42) still loads and the M2A1 (+65) is refused (S11a1 review A-1).
#[test]
fn howitzers_are_inf_guns_with_a_low_angle_table() {
    let d = ww2();
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let s = d.resolve(&req).unwrap();
    assert!(s.validate().is_empty(), "{:?}", s.validate());
    let tag = s.tags.iter().position(|t| t == "field_howitzer").unwrap() as u16;
    for (card, w) in [
        ("ww2:ger_lefh18_squad", "ww2:lefh18"),
        ("ww2:usa_105mm_m2a1_squad", "ww2:m2a1_105mm"),
    ] {
        let t = &s.squad_types[s.squad_type_index(card).unwrap() as usize];
        let v = &s.vehicles[t.vehicle.unwrap() as usize];
        assert_eq!(v.role, frontline_content::VehicleRole::InfGun, "{card}");
        assert!(t.tags.contains(&tag), "{card}");
        // Build C (H1): the S11a1 gun cards' weight.
        assert_eq!((t.ai_weight, t.max_alive), (5, 1), "{card}");
        let wi = &s.weapons[v.mounts[0].weapon as usize];
        assert_eq!(wi.id, w);
        assert!(wi.indirect.as_ref().unwrap().low_angle, "{card}");
        let f = &s.factions[t.faction as usize];
        let n = f.roster.len();
        assert_eq!(
            s.squad_types[f.roster[n - 2] as usize].id,
            card,
            "appended before the light mortar"
        );
    }
    let off = |w: &str| {
        let mut d = ww2();
        d.weapons
            .get_mut(w)
            .unwrap()
            .indirect
            .as_mut()
            .unwrap()
            .low_angle = false;
        d.cross_check()
            .into_iter()
            .filter(|i| i.path.contains("elevation_deg"))
            .count()
    };
    assert_eq!(off("ww2:lefh18"), 0, "+42 needs no low-angle table");
    assert_eq!(off("ww2:m2a1_105mm"), 1, "+65 does");
}

/// C5: with `[items] rifle_grenades` 1 the US Rifle Squad's two rifle
/// grenadiers and the Grenadiergruppe's one carry three each (M17 on the
/// M7, Gewehr-Sprenggranate on the Schiessbecher); the men, their order,
/// price and pop are unchanged. The variants list their own members and
/// inherit no grenadier. Control: with 0 no squad carries one.
#[test]
fn rifle_grenadiers_carry_three() {
    let d = ww2();
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let kit = |rg: u32, card: &str| {
        let mut d = d.clone();
        d.rules
            .get_mut("ww2:skirmish")
            .unwrap()
            .items
            .rifle_grenades = rg;
        let s = d.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
        let t = &s.squad_types[s.squad_type_index(card).unwrap() as usize];
        let item = t.items[ItemKind::RifleGrenade.index()].map(|i| s.items[i as usize].id.clone());
        let per: Vec<(String, u8, u8)> = t
            .members
            .iter()
            .map(|m| {
                (
                    m.slot.clone(),
                    m.count,
                    m.items[ItemKind::RifleGrenade.index()],
                )
            })
            .filter(|(_, _, n)| *n > 0)
            .collect();
        (item, per, t.men, t.cost)
    };
    let (item, per, men, cost) = kit(1, "ww2:usa_rifle_squad");
    assert_eq!(item.as_deref(), Some("ww2:m7_rifle_grenade"));
    assert_eq!(per, [("rifle_grenadier".to_string(), 2, 3)]);
    assert_eq!((men, cost), (12, 360), "men and price unchanged");
    let (item, per, men, cost) = kit(1, "ww2:ger_grenadiergruppe");
    assert_eq!(item.as_deref(), Some("ww2:schiessbecher_sprgr"));
    assert_eq!(per, [("rifle_grenadier".to_string(), 1, 3)]);
    assert_eq!((men, cost), (10, 232), "men and price unchanged");
    for v in [
        "ww2:usa_rifle_squad_9",
        "ww2:usa_rifle_squad_2bar",
        "ww2:ger_grenadiergruppe_7",
    ] {
        assert_eq!(kit(1, v).1, [], "{v} inherits no grenadier");
    }
    for card in ["ww2:usa_rifle_squad", "ww2:ger_grenadiergruppe"] {
        let (item, per, _, _) = kit(0, card);
        assert_eq!((item, per), (None, vec![]), "control: switch 0, {card}");
    }
    // The German round (spec C5): 250 g, 250 m, 60 m/s, the Mk 2 block at
    // about half.
    let it = &d.items["ww2:schiessbecher_sprgr"];
    assert_eq!((it.range_m, it.min_range_m), (250, 30), "S11b-7");
    assert_eq!(
        d.weapons["ww2:schiessbecher"].muzzle_velocity_m_s, 60,
        "S11b-7"
    );
    let he = d.ammo["ww2:gsprgr30"].he.as_ref().unwrap();
    assert_eq!(
        (he.frag_count, he.blast_radius_cm, he.frag_reach_cm),
        (48, 160, 800),
        "Mk 2 x 0.5 rays, x 0.8 reach"
    );
}
