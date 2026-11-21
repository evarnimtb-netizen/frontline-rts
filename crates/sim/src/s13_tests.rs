//! S13a Build A tests (docs/slices/S13-heavy-wave.md 3, 5): the SAVE 31
//! layout and its load checks, rule O1 and `special_min_pct`. Each test
//! names its control: the edit that makes it fail.

use super::s6_tests::{flat_world_with, spawn, steps};
use super::*;
use frontline_content::{AmmoMechanism, SlopeClass};

/// A flat match whose player 1 is an AI, with the S13 AI fields staged.
fn staged() -> World {
    let mut w = flat_world_with(1, |s| s.players[1].is_ai = true);
    steps(&mut w, 3);
    spawn(&mut w, 1, "ww2:usa_rifle_squad", 300, 300);
    steps(&mut w, 30);
    let tick = w.tick();
    assert!(w.with_ai_state(1, |ai| {
        ai.armour_seen_at = tick - 5;
        ai.heavy_plan_since = tick - 2;
        let k = &mut ai.counters;
        (k.heavy_plans, k.heavy_buys, k.heavy_drops, k.push_turnbacks) = (2, 1, 1, 3);
    }));
    w
}

fn ai(s: &mut SimState) -> &mut crate::ai::AiState {
    s.game.as_mut().unwrap().ai[1].as_mut().unwrap()
}

/// Plan 5: the S13 AI fields save, load and hash. Control: an unedited
/// reload keeps the hash (each edit must change it).
#[test]
fn s13_layout_roundtrips_and_hashes() {
    let w = staged();
    let bytes = w.save();
    let back = World::load(&bytes, 1).expect("the S13 state loads");
    assert_eq!(back.game().unwrap().ai[1], w.game().unwrap().ai[1]);
    assert_eq!(back.full_state_hash(), w.full_state_hash());
    assert_eq!(back.save(), bytes);
    let h = back.full_state_hash();
    assert_eq!(World::load(&back.save(), 1).unwrap().full_state_hash(), h);
    type Edit = Box<dyn Fn(&mut SimState)>;
    let edits: Vec<Edit> = vec![
        Box::new(|s| ai(s).armour_seen_at -= 1),
        Box::new(|s| ai(s).heavy_plan_since = 0),
        Box::new(|s| ai(s).counters.heavy_plans += 1),
        Box::new(|s| ai(s).counters.heavy_buys += 1),
        Box::new(|s| ai(s).counters.heavy_drops += 1),
        Box::new(|s| ai(s).counters.push_turnbacks += 1),
    ];
    for (k, e) in edits.into_iter().enumerate() {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        let y = World::load(&x.save(), 1).unwrap_or_else(|e| panic!("edit {k}: {e:?}"));
        assert_ne!(y.full_state_hash(), h, "edit {k} is not hashed");
    }
}

/// Plan 5 load validation, one case per check. Control: the staged save
/// itself loads (checked first), so each failure is the edit's.
#[test]
fn load_rejects_bad_s13_state() {
    let w = staged();
    let bytes = w.save();
    World::load(&bytes, 1).expect("control: the staged save loads");
    let tick = w.tick();
    type Edit = Box<dyn Fn(&mut SimState)>;
    let cases: Vec<(&str, Edit)> = vec![
        (
            "armour_seen_at in the future",
            Box::new(move |s| ai(s).armour_seen_at = tick + 1),
        ),
        (
            "heavy_plan_since in the future",
            Box::new(move |s| ai(s).heavy_plan_since = tick + 1),
        ),
    ];
    for (name, e) in cases {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        assert!(World::load(&x.save(), 1).is_err(), "{name} loads");
    }
}

/// Plan 3.2, 5: the S13 setup keys are part of the setup (saved, hashed),
/// and a slope class on an HE or HEAT round, or a heavy flag on a soldier
/// card, is refused. Control: the unedited setup starts a match.
#[test]
fn s13_setup_keys_are_hashed_and_checked() {
    let w = flat_world_with(1, |_| {});
    let base = w.game().unwrap().setup.clone();
    let h0 = postcard::to_allocvec(&base).unwrap();
    let ap = base
        .ammo
        .iter()
        .position(|a| a.mechanism == AmmoMechanism::Aphe)
        .expect("an aphe round");
    let he = base
        .ammo
        .iter()
        .position(|a| a.mechanism == AmmoMechanism::He)
        .expect("an he round");
    let veh_card = base
        .squad_types
        .iter()
        .position(|t| t.vehicle.is_some())
        .unwrap();
    let man_card = base
        .squad_types
        .iter()
        .position(|t| t.vehicle.is_none())
        .unwrap();
    type Edit = Box<dyn Fn(&mut frontline_content::MatchSetup)>;
    let good: Vec<Edit> = vec![
        Box::new(move |s| s.ammo[ap].slope_class = Some(SlopeClass::Apcbc)),
        Box::new(move |s| s.ammo[ap].slope_class = Some(SlopeClass::Apcr)),
        Box::new(move |s| s.squad_types[veh_card].ai_heavy = true),
        Box::new(|s| s.vehicles[0].ai_overwatch = true),
        Box::new(|s| s.rules.armour.obliquity = 0),
        Box::new(|s| s.rules.armour.td_lo_pct = 30),
        Box::new(|s| s.rules.armour.td_hi_pct = 100),
        Box::new(|s| s.rules.armour.apcbc_w_pct = 50),
        Box::new(|s| s.rules.armour.apcr_w_pct = 50),
        Box::new(|s| s.rules.vehicles.gunnery.special_min_pct = 25),
        Box::new(|s| s.rules.ai.armour.heavy_buy = 1),
        Box::new(|s| s.rules.ai.armour.heavy_seen_s = 200),
        Box::new(|s| s.rules.ai.armour.heavy_save_max_s = 200),
        Box::new(|s| s.rules.ai.armour.heavy_overwatch = 1),
        Box::new(|s| s.rules.ai.armour.push_limit = 1),
        Box::new(|s| s.rules.ai.armour.push_without_supply_m = 200),
        Box::new(|s| s.rules.ai.armour.tank_reserve_rounds = 5),
    ];
    for (k, e) in good.into_iter().enumerate() {
        let mut s = base.clone();
        e(&mut s);
        assert_ne!(
            postcard::to_allocvec(&s).unwrap(),
            h0,
            "edit {k} is not in the setup bytes"
        );
        assert!(s.validate().is_empty(), "edit {k}: {:?}", s.validate());
    }
    let bad: Vec<(&str, Edit)> = vec![
        (
            "an he round with a class",
            Box::new(move |s| s.ammo[he].slope_class = Some(SlopeClass::Apcbc)),
        ),
        (
            "a soldier card marked heavy",
            Box::new(move |s| s.squad_types[man_card].ai_heavy = true),
        ),
        (
            "td_lo_pct not below td_hi_pct",
            Box::new(|s| s.rules.armour.td_lo_pct = 90),
        ),
        ("obliquity 2", Box::new(|s| s.rules.armour.obliquity = 2)),
        (
            "apcr_w_pct over 100",
            Box::new(|s| s.rules.armour.apcr_w_pct = 101),
        ),
        (
            "special_min_pct over 100",
            Box::new(|s| s.rules.vehicles.gunnery.special_min_pct = 101),
        ),
        ("heavy_buy 2", Box::new(|s| s.rules.ai.armour.heavy_buy = 2)),
    ];
    assert!(
        base.validate().is_empty(),
        "control: the unedited setup is valid"
    );
    for (name, e) in bad {
        let mut s = base.clone();
        e(&mut s);
        assert!(!s.validate().is_empty(), "{name} is accepted");
    }
}
