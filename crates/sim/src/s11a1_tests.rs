//! S11a1 tests (docs/slices/S11a1.md 3, 4; Build A): the SAVE 27 layout
//! and its load checks, the widened kill class, and the sim rules G1-G7.
//! Each test names its control: the edit that makes it fail.

use super::s6_tests::{flat_world_with, spawn, steps};
use super::*;
use crate::ai::{AiArty, AiBreach, AiFo, ArtyPhase, BreachPhase, MAX_ARTY, MAX_FO};
use crate::game::kill_class;

fn c(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

/// A flat match whose player 1 is an AI, with one valid record of each
/// S11a1 kind staged for player 1 (`(gun squad, observer squad)`).
fn staged() -> (World, SquadId, SquadId) {
    let mut w = flat_world_with(1, |s| s.players[1].is_ai = true);
    steps(&mut w, 3);
    let a = spawn(&mut w, 1, "ww2:usa_rifle_squad", 300, 300);
    let b = spawn(&mut w, 1, "ww2:usa_rifle_squad", 320, 300);
    steps(&mut w, 30);
    let tick = w.tick();
    assert!(w.with_ai_state(1, |ai| {
        ai.arty.insert(
            a,
            AiArty {
                phase: ArtyPhase::Ready,
                since: tick,
                last_order: tick,
                missions: 3,
                prep: true,
            },
        );
        ai.fo.insert(
            b,
            AiFo {
                point: 2,
                post: Some(c(400, 400)),
                cursor: 4,
                since: tick,
                arrived: true,
                retry_at: tick + 60,
            },
        );
        ai.fo_lost_at = tick;
        ai.breaches.insert(
            b,
            AiBreach {
                point: 1,
                crossing: c(500, 500),
                section: 1,
                phase: BreachPhase::Stage,
                since: tick,
                tries: 0,
                suppress_at: None,
                smoke_at: None,
                prep_at: Some(tick),
            },
        );
        let k = &mut ai.counters;
        k.arty_buys = 1;
        k.breach_prep_timeouts = 2;
    }));
    let g = w.state.game.as_mut().unwrap();
    g.players[0].s11.gun_rounds[1] = 5;
    g.players[0].s11.gun_shell_micro = 7;
    (w, a, b)
}

/// Spec 3: the S11a1 records save, load and hash. Control: an unedited
/// reload keeps the hash (each edit must change it).
#[test]
fn s11a1_layout_roundtrips_and_hashes() {
    let (w, a, b) = staged();
    let bytes = w.save();
    let back = World::load(&bytes, 1).expect("the S11a1 state loads");
    assert_eq!(back.game().unwrap().ai[1], w.game().unwrap().ai[1]);
    assert_eq!(back.full_state_hash(), w.full_state_hash());
    assert_eq!(back.save(), bytes);
    let h = back.full_state_hash();
    type Edit = Box<dyn Fn(&mut SimState)>;
    fn ai(s: &mut SimState) -> &mut crate::ai::AiState {
        s.game.as_mut().unwrap().ai[1].as_mut().unwrap()
    }
    let edits: Vec<Edit> = vec![
        Box::new(move |s| ai(s).arty.get_mut(&a).unwrap().missions += 1),
        Box::new(move |s| ai(s).arty.get_mut(&a).unwrap().phase = ArtyPhase::Emplace),
        Box::new(move |s| ai(s).fo.get_mut(&b).unwrap().cursor += 1),
        Box::new(move |s| ai(s).fo.get_mut(&b).unwrap().post = None),
        Box::new(|s| ai(s).fo_lost_at -= 1),
        Box::new(move |s| ai(s).breaches.get_mut(&b).unwrap().prep_at = None),
        Box::new(|s| ai(s).counters.arty_lifts += 1),
        Box::new(|s| ai(s).counters.fo_post_fails += 1),
        Box::new(|s| s.game.as_mut().unwrap().players[0].s11.gun_breaks += 1),
        Box::new(|s| s.game.as_mut().unwrap().players[1].s11.gun_miss_mm[2] += 1),
        Box::new(|s| s.game.as_mut().unwrap().players[1].fm.refused[8] += 1),
    ];
    assert_eq!(World::load(&back.save(), 1).unwrap().full_state_hash(), h);
    for (k, e) in edits.into_iter().enumerate() {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        let y = World::load(&x.save(), 1).unwrap_or_else(|e| panic!("edit {k}: {e:?}"));
        assert_ne!(y.full_state_hash(), h, "edit {k} is not hashed");
    }
}

/// Spec 3 load validation, one case per check. Control: the staged save
/// itself loads (checked first), so each failure is the edit's.
#[test]
fn load_rejects_bad_s11a1_state() {
    let (w, a, b) = staged();
    let bytes = w.save();
    World::load(&bytes, 1).expect("control: the staged save loads");
    let tick = w.tick();
    let other = w
        .game()
        .unwrap()
        .squads
        .iter()
        .find(|(_, q)| q.owner == PlayerId(0))
        .map(|(id, _)| *id);
    type Edit = Box<dyn Fn(&mut SimState)>;
    fn ai(s: &mut SimState) -> &mut crate::ai::AiState {
        s.game.as_mut().unwrap().ai[1].as_mut().unwrap()
    }
    let mut cases: Vec<(&str, Edit)> = vec![
        (
            "arty over its cap",
            Box::new(move |s| {
                let r = ai(s).arty[&a];
                for k in 0..=MAX_ARTY as u32 {
                    ai(s).arty.insert(SquadId(k), r);
                }
            }),
        ),
        (
            "fo over its cap",
            Box::new(move |s| {
                let r = ai(s).fo[&b];
                for k in 0..=MAX_FO as u32 {
                    ai(s).fo.insert(SquadId(k), r);
                }
            }),
        ),
        ("fo_lost_at in the future", Box::new(move |s| ai(s).fo_lost_at = tick + 1)),
        (
            "arty key never handed out",
            Box::new(move |s| {
                let r = ai(s).arty[&a];
                ai(s).arty.insert(SquadId(1 << 30), r);
            }),
        ),
        (
            "arty since in the future",
            Box::new(move |s| ai(s).arty.get_mut(&a).unwrap().since = tick + 1),
        ),
        (
            "arty last_order in the future",
            Box::new(move |s| ai(s).arty.get_mut(&a).unwrap().last_order = tick + 1),
        ),
        (
            "fo key never handed out",
            Box::new(move |s| {
                let r = ai(s).fo[&b];
                ai(s).fo.insert(SquadId(1 << 30), r);
            }),
        ),
        (
            "fo point out of range",
            Box::new(move |s| ai(s).fo.get_mut(&b).unwrap().point = 99),
        ),
        (
            "fo post off the map",
            Box::new(move |s| ai(s).fo.get_mut(&b).unwrap().post = Some(c(-5, 10))),
        ),
        (
            "fo cursor past the candidates",
            Box::new(move |s| ai(s).fo.get_mut(&b).unwrap().cursor = 16),
        ),
        (
            "fo since in the future",
            Box::new(move |s| ai(s).fo.get_mut(&b).unwrap().since = tick + 1),
        ),
        (
            "fo retry_at too far ahead",
            Box::new(move |s| ai(s).fo.get_mut(&b).unwrap().retry_at = tick + 1_000_000),
        ),
        (
            "breach prep_at in the future",
            Box::new(move |s| ai(s).breaches.get_mut(&b).unwrap().prep_at = Some(tick + 1)),
        ),
        (
            "s11 counter out of range",
            Box::new(|s| s.game.as_mut().unwrap().players[0].s11.gun_kills = u32::MAX),
        ),
        (
            "AI counter out of range",
            Box::new(|s| ai(s).counters.fo_buys = u32::MAX),
        ),
    ];
    if let Some(o) = other {
        cases.push((
            "arty key of another player",
            Box::new(move |s| {
                let r = ai(s).arty[&a];
                ai(s).arty.insert(o, r);
            }),
        ));
        cases.push((
            "fo key of another player",
            Box::new(move |s| {
                let r = ai(s).fo[&b];
                ai(s).fo.insert(o, r);
            }),
        ));
    }
    for (name, e) in cases {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        assert!(World::load(&x.save(), 1).is_err(), "{name}: loaded");
    }
}

/// Spec 3 "Vehicle type cap": with 17 vehicle types the 17th type's rounds
/// carry their type and their class. Control: the old 4-bit pack in a
/// `u8` (`class | (t + 1) << 4`) loses type 16 (17 << 4 wraps to 16 = type 0).
#[test]
fn seventeen_vehicle_types_keep_their_kill_rows() {
    let mut w = flat_world_with(1, |s| {
        let pz = s
            .squad_types
            .iter()
            .find(|t| t.id == "ww2:ger_pz4_h_squad")
            .and_then(|t| t.vehicle)
            .unwrap() as usize;
        assert_eq!(s.vehicles.len(), 15, "ww2 has 15 vehicle defs at the base");
        while s.vehicles.len() < 17 {
            let v = s.vehicles[pz].clone();
            s.vehicles.push(v);
        }
        let t = s
            .squad_types
            .iter_mut()
            .find(|t| t.id == "ww2:ger_pz4_h_squad")
            .unwrap();
        t.vehicle = Some(16);
    });
    let sq = spawn(&mut w, 0, "ww2:ger_pz4_h_squad", 300, 300);
    steps(&mut w, 2);
    let g = w.game().unwrap();
    let v = g.squads[&sq].members[0];
    assert_eq!(g.vehicles()[&v].type_idx, 16);
    assert_eq!(g.players[1].s7.kills_by_vehicle.len(), 17);
    let cannon = g
        .setup()
        .weapons
        .iter()
        .position(|x| x.class == frontline_content::WeaponClass::Cannon)
        .unwrap() as u16;
    let class = g.round_class(&w.state.entities, cannon, u16::MAX, v);
    assert_eq!(kill_class::base(class), kill_class::TANK_AP);
    assert_eq!(kill_class::vehicle(class), Some(16));
    // Every type up to the cap keeps both parts.
    for t in [0u16, 14, 15, 16, 4094] {
        let k = kill_class::with_vehicle(kill_class::FRAG, t);
        assert_eq!((kill_class::base(k), kill_class::vehicle(k)), (kill_class::FRAG, Some(t as usize)));
    }
    // Control: the old pack.
    let old = |c: u8, t: u8| (c & 0x0F) | (t.wrapping_add(1) << 4);
    let o = old(kill_class::TANK_AP as u8, 16);
    assert_ne!(o >> 4, 17, "the old u8 pack keeps type 16");
}

/// Adds a test-only infantry gun to a setup: the Pak 38 vehicle and squad
/// cloned as `test:inf_gun` with role `role` (Build B brings the real
/// leIG 18 and M3). Returns nothing; spawn it by the id.
pub(super) fn add_test_gun(s: &mut frontline_content::MatchSetup, role: frontline_content::VehicleRole) {
    let src = s
        .squad_types
        .iter()
        .position(|t| t.id == "ww2:ger_pak38_squad")
        .unwrap();
    let vi = s.squad_types[src].vehicle.unwrap() as usize;
    let mut v = s.vehicles[vi].clone();
    v.role = role;
    s.vehicles.push(v);
    let mut t = s.squad_types[src].clone();
    t.id = "test:inf_gun".into();
    t.vehicle = Some((s.vehicles.len() - 1) as u16);
    s.squad_types.push(t);
}

/// Spec A1 / review 2: the AT-gun habits take only role `at_gun`; an
/// `inf_gun` gets no `AiGun` job and no order. Control: the same clone
/// with role `at_gun` gets a job.
#[test]
fn at_gun_ai_ignores_infantry_guns() {
    use frontline_content::VehicleRole;
    let run = |role: VehicleRole| {
        let mut w = flat_world_with(1, |s| {
            s.players[1].is_ai = true;
            add_test_gun(s, role);
        });
        steps(&mut w, 2);
        let q = spawn(&mut w, 1, "test:inf_gun", 400, 400);
        let order0 = w.game().unwrap().squads[&q].order.kind();
        steps(&mut w, 30 * 40);
        let g = w.game().unwrap();
        let ai = g.ai[1].as_ref().unwrap();
        (ai.guns.contains_key(&q), g.squads[&q].order.kind() == order0)
    };
    assert_eq!(run(VehicleRole::InfGun), (false, true), "an inf_gun is left alone");
    assert!(run(VehicleRole::AtGun).0, "control: an at_gun clone gets a job");
}
