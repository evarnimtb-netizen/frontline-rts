//! S11a1 (docs/slices/S11a1.md 2): the new `[fire_mission]`, `[supply]` and
//! `[ai.arty]` keys parse from the ww2 rules at their defaults, the `a_off`
//! control file reaches the setup, every key is range-checked (overrides
//! and untrusted saves), and the `[indirect]` gun keys, the `radio` trait
//! and the `inf_gun` role load. Build B adds the content tests here.

use std::path::{Path, PathBuf};

use frontline_content::tuning::{AiArtyTuning, FireMissionTuning};
use frontline_content::{Difficulty, MatchRequest, ModDefs};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn write(name: &str, text: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "frontline-s11a1-{name}-{}.toml",
        std::process::id()
    ));
    std::fs::write(&p, text).unwrap();
    p
}

/// One out-of-range case per new key (spec 2 ranges). Control: widen one
/// range (seen with `gun_break_m: 0..=2001`) and that case is accepted, so
/// the test fails.
#[test]
fn load_rejects_bad_s11a1_keys() {
    let mut defs = ModDefs::load(&root().join("mods/ww2")).unwrap_or_else(|e| panic!("{e}"));
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let s = defs.resolve(&req).unwrap();
    let fm = s.rules.fire_mission.as_ref().unwrap();
    assert_eq!(
        (
            fm.gun_missions,
            fm.radio_link,
            fm.mask,
            fm.gun_fire_direction_s,
            fm.gun_adjust_rounds_max
        ),
        (1, 1, 1, 20, 1)
    );
    assert_eq!(
        (
            fm.gun_first_round_error_pct,
            fm.gun_first_round_error_min_m,
            fm.gun_map_fire_error_pct,
            fm.gun_map_fire_error_min_m,
            fm.gun_break_m
        ),
        (3, 25, 6, 50, 150)
    );
    assert_eq!(s.rules.supply.as_ref().unwrap().arty_base_rearm, 1);
    // Build C: ww2 turns the three switches on; every other key is the
    // default.
    assert_eq!(
        s.rules.ai.arty,
        AiArtyTuning {
            buy: 1,
            fo: 1,
            prep: 1,
            ..AiArtyTuning::default()
        }
    );
    assert_eq!(FireMissionTuning::KEYS.len(), 35);
    assert_eq!(AiArtyTuning::KEYS.len(), 21);
    assert!(
        frontline_content::tuning::TABLES
            .iter()
            .any(|(t, k)| *t == "ai.arty" && k.len() == 21)
    );
    // The control file applies; the switches go off.
    let a_off = root().join("docs/slices/targets/S11a1-ablations/a_off.toml");
    defs.apply_override(&a_off, "ww2:skirmish")
        .unwrap_or_else(|e| panic!("{e}"));
    let s = defs.resolve(&req).unwrap();
    let fm = s.rules.fire_mission.as_ref().unwrap();
    assert_eq!((fm.gun_missions, fm.radio_link, fm.mask), (0, 0, 0));
    assert_eq!(s.rules.supply.as_ref().unwrap().arty_base_rearm, 1);
    // The switches off: every key the default.
    assert_eq!(s.rules.ai.arty, AiArtyTuning::default());
    // An override reaches the setup.
    let on = write("on", "[ai.arty]\nbuy = 1\npost_rays = 48\n");
    defs.apply_override(&on, "ww2:skirmish")
        .unwrap_or_else(|e| panic!("{e}"));
    let s = defs.resolve(&req).unwrap();
    assert_eq!((s.rules.ai.arty.buy, s.rules.ai.arty.post_rays), (1, 48));
    // Out of range: one case per key; the issue names the key.
    let cases: Vec<(&str, &str, u32)> = vec![
        ("fire_mission", "gun_missions", 2),
        ("fire_mission", "radio_link", 2),
        ("fire_mission", "mask", 2),
        ("fire_mission", "gun_fire_direction_s", 3601),
        ("fire_mission", "gun_adjust_rounds_max", 0),
        ("fire_mission", "gun_first_round_error_pct", 51),
        ("fire_mission", "gun_first_round_error_min_m", 2001),
        ("fire_mission", "gun_map_fire_error_pct", 51),
        ("fire_mission", "gun_map_fire_error_min_m", 2001),
        ("fire_mission", "gun_break_m", 2001),
        ("supply", "arty_base_rearm", 2),
        ("ai.arty", "buy", 2),
        ("ai.arty", "fo", 2),
        ("ai.arty", "prep", 2),
        ("ai.arty", "after_s", 3601),
        ("ai.arty", "income_max_pct", 101),
        ("ai.arty", "reserve_mp", 10001),
        ("ai.arty", "max_guns", 9),
        ("ai.arty", "fo_max", 9),
        ("ai.arty", "fo_rebuy_s", 3601),
        ("ai.arty", "mission_rounds", 51),
        ("ai.arty", "reserve_pct", 101),
        ("ai.arty", "shell_mp_max_pct", 101),
        ("ai.arty", "danger_close_m", 2001),
        ("ai.arty", "post_m", 2001),
        ("ai.arty", "post_side_m", 2001),
        ("ai.arty", "post_rays", 49),
        ("ai.arty", "post_clear_m", 2001),
        ("ai.arty", "post_retry_s", 3601),
        ("ai.arty", "prep_near_m", 2001),
        ("ai.arty", "prep_rounds_min", 0),
        ("ai.arty", "prep_wait_s", 3601),
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
    // The gun error floors keep their order (observed <= map).
    let f = write(
        "order",
        "[fire_mission]\ngun_first_round_error_min_m = 60\ngun_map_fire_error_min_m = 50\n",
    );
    assert!(defs.apply_override(&f, "ww2:skirmish").is_err());
    // The setup is checked too (saves are untrusted).
    let mut s = defs.resolve(&req).unwrap();
    assert!(s.validate().is_empty());
    s.rules.ai.arty.post_rays = 49;
    assert!(
        s.validate().iter().any(
            |i| i.path.contains("ai.arty.post_rays") || i.message.contains("ai.arty.post_rays")
        ),
        "{:?}",
        s.validate()
    );
    let mut s = defs.resolve(&req).unwrap();
    s.rules.fire_mission.as_mut().unwrap().gun_break_m = 2001;
    assert!(!s.validate().is_empty());
}

/// The `[indirect]` gun keys, the `radio` trait and the +73 deg mount,
/// parsed from the real ww2 defs (review A-1: the by-construction test did
/// not see that the def checks refused them). Controls: the same weapon
/// without `low_angle` is refused by the cross-check (one rule with the
/// setup check), a mount over 89 by the def check, and `radio` on a member
/// would be refused if it were reserved.
#[test]
fn indirect_gun_keys_and_radio_trait_parse() {
    use frontline_content::defs::{SquadDef, WeaponDef};
    use frontline_content::setup::member_trait;
    use frontline_content::vehicle::{VehicleDef, low_angle_top};
    let read = |f: &str| std::fs::read_to_string(root().join("mods/ww2/defs").join(f)).unwrap();
    let crew: SquadDef = toml::from_str(&read("units/ger_leig18_crew.toml")).unwrap();
    assert!(crew.validate().is_empty(), "{:?}", crew.validate());
    let fo: SquadDef = toml::from_str(&read("units/usa_fo_party.toml")).unwrap();
    assert!(fo.validate().is_empty(), "{:?}", fo.validate());
    let w: WeaponDef = toml::from_str(&read("weapons/leig18.toml")).unwrap();
    assert!(w.validate().is_empty(), "{:?}", w.validate());
    let ind = w.indirect.as_ref().unwrap();
    assert!(ind.low_angle && ind.range_m == Some(3550) && ind.spread_mrad == Some(8));
    let v: VehicleDef = toml::from_str(&read("vehicles/ger_leig18.toml")).unwrap();
    assert!(v.validate().is_empty(), "{:?}", v.validate());
    assert_eq!(v.weapons[0].elevation_deg, Some([-10, 73]));
    assert_eq!((low_angle_top(true), low_angle_top(false)), (89, 60));
    let mut bad = v.clone();
    bad.weapons[0].elevation_deg = Some([-10, 90]);
    assert!(
        !bad.validate().is_empty(),
        "control: 90 deg passes the def check"
    );
    // The mod loads and cross-checks clean; without `low_angle` the +73 mount
    // is refused (control).
    let mut d = ModDefs::load(&root().join("mods/ww2")).unwrap_or_else(|e| panic!("{e}"));
    assert!(d.cross_check().is_empty(), "{:?}", d.cross_check());
    d.weapons
        .get_mut("ww2:leig18")
        .unwrap()
        .indirect
        .as_mut()
        .unwrap()
        .low_angle = false;
    assert!(
        d.cross_check()
            .iter()
            .any(|i| i.path.contains("ger_leig18") && i.path.contains("elevation_deg")),
        "{:?}",
        d.cross_check()
    );
    // The resolved setup carries the trait bits.
    let d = ModDefs::load(&root().join("mods/ww2")).unwrap();
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let s = d.resolve(&req).unwrap();
    let t = s
        .squad_types
        .iter()
        .find(|t| t.id == "ww2:ger_leig18_crew")
        .unwrap();
    assert_eq!(
        t.members[2].traits & member_trait::RADIO,
        member_trait::RADIO
    );
    assert!(frontline_content::VehicleRole::InfGun.outside_limit());
    assert_eq!(frontline_content::VehicleRole::InfGun.name(), "inf_gun");
}

/// C6: the four cards and two rounds load and validate at the figures of
/// spec 4 C1-C4 (cost, pop, cooldown, unlock, crew, rounds, max alive,
/// `ai_weight` 0 in Build B); the M3's band never passes 30 deg; the leIG's
/// does. Control: a card at another price fails the table.
#[test]
fn s11a1_cards_load_at_the_spec_figures() {
    use frontline_content::Fixed;
    let d = ModDefs::load(&root().join("mods/ww2")).unwrap_or_else(|e| panic!("{e}"));
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let s = d.resolve(&req).unwrap();
    assert!(s.validate().is_empty(), "{:?}", s.validate());
    let hz = s.tick_hz;
    let card = |id: &str| {
        s.squad_types
            .iter()
            .find(|t| t.id == id)
            .unwrap_or_else(|| panic!("{id}"))
    };
    // (card, cost, pop, cooldown s, unlock s, max alive, men of the crew or squad)
    let table: [(&str, u32, u32, u32, u32, u16, u8); 4] = [
        ("ww2:ger_leig18_squad", 320, 2, 90, 180, 2, 5),
        ("ww2:usa_105mm_m3_squad", 380, 2, 120, 240, 2, 6),
        ("ww2:ger_vb_trupp", 80, 2, 60, 0, 2, 2),
        ("ww2:usa_fo_party", 120, 3, 60, 0, 2, 3),
    ];
    for (id, cost, pop, cd, unlock, alive, men) in table {
        let t = card(id);
        let crew = t
            .vehicle
            .and_then(|v| s.vehicles[v as usize].crew_squad)
            .map_or(t.men, |c| s.squad_types[c as usize].men);
        assert_eq!(
            (
                t.cost,
                t.pop(&s.vehicles),
                t.recall_cooldown_ticks / hz,
                t.initial_cooldown_ticks / hz,
                t.max_alive,
                crew,
                t.ai_weight
            ),
            // Build C: the gun cards weigh 5 (`buy_role_within`), the
            // observer cards 0 (A3 buys them by trait).
            (
                cost,
                pop,
                cd,
                unlock,
                alive,
                men,
                if t.vehicle.is_some() { 5 } else { 0 }
            ),
            "{id}"
        );
        let f = &s.factions[t.faction as usize];
        assert_eq!(
            f.roster
                .iter()
                .filter(|&&r| s.squad_types[r as usize].id == id)
                .count(),
            1
        );
    }
    // The four cards come last in each roster (after every older card).
    for f in &s.factions {
        let n = f.roster.len();
        let last: Vec<&str> = f.roster[n - 2..]
            .iter()
            .map(|&r| s.squad_types[r as usize].id.as_str())
            .collect();
        assert!(
            last == ["ww2:ger_leig18_squad", "ww2:ger_vb_trupp"]
                || last == ["ww2:usa_105mm_m3_squad", "ww2:usa_fo_party"],
            "{last:?}"
        );
    }
    // Rounds: HE, priced shells; the guns carry 12 and 10.
    let gun = |id: &str| {
        let v = card(id).vehicle.unwrap() as usize;
        let m = &s.vehicles[v].mounts[0];
        (
            s.vehicles[v].role,
            m.ammo.clone(),
            &s.weapons[m.weapon as usize],
            m.elevation_deg,
        )
    };
    let (role, ammo, w, elev) = gun("ww2:ger_leig18_squad");
    assert_eq!(role, frontline_content::VehicleRole::InfGun);
    assert_eq!(
        (
            ammo.len(),
            ammo[0].count,
            s.ammo[ammo[0].ammo as usize].id.as_str()
        ),
        (1, 12, "ww2:75mm_igr18")
    );
    let ind = w.indirect.as_ref().unwrap();
    assert!(
        ind.low_angle && ind.tan_max > Fixed::from_int(3),
        "leIG band reaches 73 deg"
    );
    assert_eq!(elev, [-10, 73]);
    let (_, ammo, w, elev) = gun("ww2:usa_105mm_m3_squad");
    assert_eq!(
        (ammo[0].count, s.ammo[ammo[0].ammo as usize].id.as_str()),
        (10, "ww2:105mm_m1_he")
    );
    let ind = w.indirect.as_ref().unwrap();
    // tan 30 deg = 0.577: the M3 never lays above 30 deg.
    assert!(
        ind.tan_max <= Fixed::from_ratio(578, 1000) && ind.tan_max > Fixed::from_ratio(57, 100)
    );
    assert_eq!(elev, [-9, 30]);
    assert_eq!(ind.range, Fixed::from_int(7600));
}

/// C6: `a_off.toml` holds every S11a1 switch (spec 2) at its off value.
/// Control: a switch missing from the file fails the list.
#[test]
fn a_off_holds_every_s11a1_switch() {
    let text =
        std::fs::read_to_string(root().join("docs/slices/targets/S11a1-ablations/a_off.toml"))
            .unwrap();
    let v: toml::Value = toml::from_str(&text).unwrap();
    let switches = [
        ("fire_mission", "gun_missions", 0),
        ("fire_mission", "radio_link", 0),
        ("fire_mission", "mask", 0),
        ("supply", "arty_base_rearm", 1),
        ("ai.arty", "buy", 0),
        ("ai.arty", "fo", 0),
        ("ai.arty", "prep", 0),
    ];
    for (table, key, off) in switches {
        let mut t = &v;
        for part in table.split('.') {
            t = &t[part];
        }
        assert_eq!(
            t.get(key).and_then(|x| x.as_integer()),
            Some(off),
            "{table}.{key}"
        );
    }
}
