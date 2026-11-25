//! S9 Build A tests (docs/slices/S9-at-guns.md 4, 5, 15): the gun crew as
//! soldiers in service slots, the seat mirror, pinned gunner, auto-swap,
//! the AP core reaching a man behind the shield, hold fire, and the save
//! and validation checks of the new state.

use super::s6_tests::{flat_world, flat_world_with, members, spawn, steps};
use super::*;
use crate::FxVec3;
use crate::vehicle::Slot;

const PAK: &str = "ww2:ger_pak40_squad";

fn gun(w: &mut World, p: u8, x: i32, y: i32) -> SimId {
    let sq = spawn(w, p, PAK, x, y);
    members(w, sq)[0]
}

fn slots(w: &World, g: SimId) -> Vec<Slot> {
    w.game().unwrap().vehicles[&g]
        .served
        .as_ref()
        .unwrap()
        .slots
        .clone()
}

#[test]
fn a_served_gun_is_a_vehicle_with_soldiers_in_its_slots() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    let st = slots(&w, g);
    assert_eq!(st.len(), 6);
    for (k, s) in st.iter().enumerate() {
        let m = s.man.expect("spawned at the slot");
        assert_eq!(w.entities().kind[m.index()], EntityKind::Soldier);
        let u = w.entities().unit[m.index()].serving.unwrap();
        assert_eq!((u.gun, u.slot), (g, k as u8));
    }
    // Soldiers stand at the service slots, not on the gun.
    let gpos = w.entities().transform[g.index()].pos.xy();
    let first = st[0].man.unwrap();
    let d = (w.entities().transform[first.index()].pos.xy() - gpos).length();
    assert!(d > Fx::from_ratio(1, 2), "gunner at the slot, {d:?}");
    steps(&mut w, 30);
    assert!(World::from_state(w.state.clone(), 1).is_ok());
    // Saves round-trip and replay identically.
    let bytes = w.save();
    let mut w2 = World::load(&bytes, 3).unwrap();
    assert_eq!(w2.full_state_hash(), w.full_state_hash());
    steps(&mut w, 15);
    steps(&mut w2, 15);
    assert_eq!(w2.full_state_hash(), w.full_state_hash());
}

#[test]
fn the_seat_mirror_reads_the_men_and_a_pinned_gunner_is_not_manned() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    steps(&mut w, 2);
    let gunner = slots(&w, g)[0].man.unwrap();
    let vt = w.game().unwrap().vehicles[&g].type_idx as usize;
    let setup = w.match_setup().unwrap().clone();
    let v = setup.vehicles[vt].clone();
    let manned =
        |w: &World| w.game().unwrap().vehicles[&g].manned(&v, frontline_content::SeatRole::Gunner);
    assert!(manned(&w));
    // Pinned: suppression above the threshold reads as not manned.
    w.edit_soldier(gunner, |_, c| c.suppression = 1_000_000);
    steps(&mut w, 2);
    assert!(!manned(&w));
    w.edit_soldier(gunner, |_, c| {
        c.suppression = 0;
        c.pinned = false;
    });
    steps(&mut w, 2);
    assert!(manned(&w));
}

#[test]
fn a_dead_gunner_is_replaced_after_the_slot_swap() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    steps(&mut w, 2);
    let st0 = slots(&w, g);
    let (gunner, last) = (st0[0].man.unwrap(), st0[5].man.unwrap());
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let vt = w.game().unwrap().vehicles[&g].type_idx as usize;
    let v = w.match_setup().unwrap().vehicles[vt].clone();
    // He leaves his slot (killed or walked off); the last-listed living man
    // moves up, as the slot check and auto-swap run.
    w.edit_unit(gunner, |u| u.serving = None);
    steps(&mut w, 1);
    let st = slots(&w, g);
    assert_eq!(st[0].man, Some(last));
    assert_eq!(st[5].man, None);
    let cap = w.match_setup().unwrap().rules.vehicles.guns.slot_swap_s as u64 * hz + 10;
    steps(&mut w, cap);
    let st = slots(&w, g);
    assert_eq!(st[0].man, Some(last), "the last man moved up to gunner");
    assert_eq!(st[0].swap, 0);
    assert!(w.game().unwrap().vehicles[&g].manned(&v, frontline_content::SeatRole::Gunner));
}

#[test]
fn hold_fire_keeps_the_gun_silent_until_it_is_struck() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 200, 500);
    let t = spawn(&mut w, 1, "ww2:usa_m4a3_75_squad", 620, 500);
    let tank = members(&w, t)[0];
    // Tank 420 m out, stationary; the gun holds to 300 m.
    w.edit_vehicle(g, |vs| {
        vs.served.as_mut().unwrap().hold_fire_m = 300;
        vs.loaded = Some(0);
    });
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let fired = |w: &World| w.game().unwrap().vehicles[&g].since_fire != u16::MAX;
    for _ in 0..(40 * hz) {
        w.step(&[]);
        if w.entities().kind[tank.index()] != EntityKind::Vehicle {
            break;
        }
    }
    assert!(!fired(&w), "beyond the hold range it does not fire");
    // Struck: the ambush is over.
    w.edit_vehicle(g, |vs| vs.last_attacker = Some(1));
    for _ in 0..(40 * hz) {
        w.step(&[]);
        if fired(&w) {
            break;
        }
    }
    assert!(fired(&w), "once struck it fires");
}

#[test]
fn an_ap_round_through_the_shield_strikes_the_man_on_its_line() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    steps(&mut w, 2);
    let gunner = slots(&w, g)[0].man.unwrap();
    let hp0 = w.entities().combat[gunner.index()].hp;
    let (kwk, ammo) = {
        let s = w.match_setup().unwrap();
        (
            s.weapons
                .iter()
                .position(|w| w.id == "ww2:kwk40_l48")
                .unwrap() as u16,
            s.ammo
                .iter()
                .position(|a| a.id == "ww2:75mm_pzgr39")
                .unwrap() as u16,
        )
    };
    // From the front, at the gunner's y through the shield: the shell
    // strikes the shield plate; the core reaches him 1 m behind it.
    let p = w.entities().transform[gunner.index()].pos;
    let aim = FxVec3::new(p.x, p.y, p.z + Fx::from_ratio(8, 10));
    let from = FxVec3::new(p.x + Fx::from_int(100), p.y, aim.z);
    assert!(w.fire_shell_now(1, kwk, ammo, from, aim));
    steps(&mut w, 8);
    let c = &w.entities().combat[gunner.index()];
    assert!(
        c.hp < hp0 || w.entities().kind[gunner.index()] != EntityKind::Soldier,
        "the core through the shield struck him ({hp0} -> {})",
        c.hp
    );
}

#[test]
fn every_new_field_changes_the_hash() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    steps(&mut w, 2);
    let m = slots(&w, g)[0].man.unwrap();
    let base = w.full_state_hash();
    w.edit_unit(m, |u| u.serving.as_mut().unwrap().slot = 1);
    assert_ne!(w.full_state_hash(), base, "Unit::serving");
    w.edit_unit(m, |u| u.serving.as_mut().unwrap().slot = 0);
    assert_eq!(w.full_state_hash(), base, "Unit::serving restored");
    type Edit = Box<dyn Fn(&mut VehicleState)>;
    let edits: Vec<(&str, Edit)> = vec![
        (
            "hold_fire_m",
            Box::new(|v| v.served.as_mut().unwrap().hold_fire_m = 7),
        ),
        ("timer", Box::new(|v| v.served.as_mut().unwrap().timer = 7)),
        (
            "hitched",
            Box::new(|v| v.served.as_mut().unwrap().hitched = Some(SimId(1))),
        ),
        (
            "abandon_ticks",
            Box::new(|v| v.served.as_mut().unwrap().abandon_ticks = 7),
        ),
        (
            "relay_ticks",
            Box::new(|v| v.served.as_mut().unwrap().relay_ticks = 7),
        ),
        (
            "slot.swap",
            Box::new(|v| v.served.as_mut().unwrap().slots[1].swap = 7),
        ),
        (
            "state",
            Box::new(|v| v.served.as_mut().unwrap().state = crate::vehicle::GunState::Limbered),
        ),
        ("towing", Box::new(|v| v.towing = Some(SimId(1)))),
    ];
    for (name, e) in edits {
        let orig = w.game().unwrap().vehicles[&g].clone();
        w.edit_vehicle(g, |v| e(v));
        assert_ne!(w.full_state_hash(), base, "{name}");
        w.edit_vehicle(g, |v| *v = orig);
        assert_eq!(w.full_state_hash(), base, "{name} restored");
    }
}

#[test]
fn validation_refuses_a_serving_man_without_a_slot() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    steps(&mut w, 2);
    let m = slots(&w, g)[0].man.unwrap();
    // A man names the gun; the gun's slot names someone else.
    w.edit_unit(m, |u| u.serving.as_mut().unwrap().slot = 3);
    assert!(World::load(&w.save(), 1).is_err());
    w.edit_unit(m, |u| u.serving.as_mut().unwrap().slot = 0);
    assert!(World::load(&w.save(), 1).is_ok());
    // A gun slot that names a man who does not serve it.
    let other = slots(&w, g)[1].man.unwrap();
    w.edit_vehicle(g, |vs| {
        vs.served.as_mut().unwrap().slots[0].man = Some(other)
    });
    assert!(World::load(&w.save(), 1).is_err());
}

// ---- Build B: mobility, tow, neutral, capture, crates (spec 6, 9, 10).

use crate::command::{Command, CommandKind};
use crate::vehicle::GunState;

const TOW: &str = "ww2:ger_sdkfz11_squad";

fn send(w: &mut World, p: u8, kind: CommandKind) {
    w.step(&[Command {
        player: PlayerId(p),
        seq: 1,
        kind,
    }]);
}

fn state(w: &World, g: SimId) -> GunState {
    w.game().unwrap().vehicles[&g]
        .served
        .as_ref()
        .unwrap()
        .state
}

fn limber(w: &mut World, g: SimId) {
    w.edit_vehicle(g, |vs| {
        vs.served.as_mut().unwrap().state = GunState::Limbered;
    });
}

fn secs(w: &World, s: u64) -> u64 {
    s * w.match_setup().unwrap().tick_hz as u64
}

fn xy(w: &World, id: SimId) -> FxVec2 {
    w.entities().transform[id.index()].pos.xy()
}

/// A tow behind the trail of a gun at `(gx, gy)` facing +x: its rear is
/// toward the gun, about 2 m from the trail tip.
fn tow_for(w: &mut World, p: u8, gx: i32, gy: i32) -> (SquadId, SimId) {
    let t = w.match_setup().unwrap().squad_type_index(TOW).unwrap();
    let q = w
        .spawn_squad_at(
            PlayerId(p),
            t,
            FxVec2::from_int(gx - 8, gy),
            FxVec2::new(-Fx::ONE, Fx::ZERO),
        )
        .unwrap();
    (q, members(w, q)[0])
}

/// The shells crate def of player `p`'s faction.
fn shells(w: &World, p: u8) -> u16 {
    let f = w.match_setup().unwrap().players[p as usize].faction;
    super::s6_tests::def(
        w,
        f,
        frontline_content::setup::StockKind::Crate,
        Some(frontline_content::setup::SupplyKind::Shells),
    )
}

/// Kills every man at `g`'s slots.
fn kill_crew(w: &mut World, g: SimId) {
    for s in slots(w, g) {
        if let Some(m) = s.man {
            assert!(w.kill_soldier(m));
        }
    }
}

/// Whether `a` is within `eps_cm` of `b_cm` (centimetres).
fn near_cm(a: Fx, b_cm: i32, eps_cm: i32) -> bool {
    (a.mul_int(100).floor_int() - b_cm).abs() <= eps_cm
}

#[test]
fn emplace_and_pack_take_their_times_and_only_an_emplaced_gun_is_ready() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    steps(&mut w, 2);
    limber(&mut w, g);
    assert!(!w.game().unwrap().vehicles[&g].gun_ready());
    send(
        &mut w,
        0,
        CommandKind::Emplace {
            guns: vec![g],
            facing: FxVec2::from_int(500, 800),
        },
    );
    assert_eq!(state(&w, g), GunState::Emplacing);
    let (t29, t2) = (secs(&w, 29), secs(&w, 2));
    steps(&mut w, t29);
    assert_eq!(state(&w, g), GunState::Emplacing, "30 s of work");
    steps(&mut w, t2);
    assert_eq!(state(&w, g), GunState::Emplaced);
    assert!(w.game().unwrap().vehicles[&g].gun_ready());
    assert_eq!(
        w.entities().transform[g.index()].yaw,
        Fx::atan2(Fx::from_int(300), Fx::ZERO),
        "faces the point"
    );
    assert_eq!(w.game().unwrap().players[0].s9.guns_emplaced, 1);
    send(&mut w, 0, CommandKind::PackGun { guns: vec![g] });
    assert_eq!(state(&w, g), GunState::Packing);
    let (t17, t2) = (secs(&w, 17), secs(&w, 2));
    steps(&mut w, t17);
    assert_eq!(state(&w, g), GunState::Packing, "18 s");
    steps(&mut w, t2);
    assert_eq!(state(&w, g), GunState::Limbered);
}

#[test]
fn work_needs_two_men_at_the_slots() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    steps(&mut w, 2);
    limber(&mut w, g);
    send(
        &mut w,
        0,
        CommandKind::Emplace {
            guns: vec![g],
            facing: FxVec2::from_int(800, 500),
        },
    );
    let men = slots(&w, g);
    for s in men.iter().skip(1) {
        w.kill_soldier(s.man.unwrap());
    }
    let t = secs(&w, 40);
    steps(&mut w, t);
    assert_eq!(
        state(&w, g),
        GunState::Emplacing,
        "one man cannot emplace it"
    );
}

#[test]
fn a_pushed_gun_moves_at_its_push_speed_scaled_by_the_men() {
    let run = |kill: usize| {
        let mut w = flat_world(1);
        let g = gun(&mut w, 0, 500, 500);
        steps(&mut w, 2);
        limber(&mut w, g);
        let men = slots(&w, g);
        for s in men.iter().rev().take(kill) {
            w.kill_soldier(s.man.unwrap());
        }
        send(
            &mut w,
            0,
            CommandKind::PushGun {
                guns: vec![g],
                point: FxVec2::from_int(600, 500),
            },
        );
        assert_eq!(state(&w, g), GunState::Pushing);
        let t = secs(&w, 2);
        steps(&mut w, t);
        let a = xy(&w, g);
        let t = secs(&w, 10);
        steps(&mut w, t);
        // cm/s over the 10 s.
        (xy(&w, g) - a).length().mul_int(10).floor_int()
    };
    let full = run(0);
    assert!((30..=40).contains(&full), "full crew {full} cm/s");
    let two = run(4);
    assert!((8..=20).contains(&two), "two men {two} cm/s");
}

/// S16b (checker after S16a, R1): a pushed gun on a road node has road
/// factor 100 and is not slowed by woods there, as R3 prices every road
/// node without the woods factor. Control: the same woods off the road
/// cap it at its `woods_speed_pct`.
#[test]
fn a_pushed_gun_on_a_road_in_woods_keeps_its_push_speed() {
    use frontline_content::Fixed;
    use frontline_content::setup::{CircleSetup, FixedVec2, RoadSetup};
    let run = |road: bool| {
        let mut w = flat_world_with(1, |s| {
            s.map.woods.push(CircleSetup {
                center: FixedVec2::from_m([560, 500]),
                radius: Fixed::from_int(120),
            });
            if road {
                s.map.roads.push(RoadSetup {
                    points: vec![FixedVec2::from_m([400, 500]), FixedVec2::from_m([700, 500])],
                    half_width: Fixed::from_int(4),
                });
            }
        });
        let g = gun(&mut w, 0, 500, 500);
        let ty =
            &w.match_setup().unwrap().vehicles[w.game().unwrap().vehicles[&g].type_idx as usize];
        assert_eq!(ty.mobility.woods_speed_pct, 50, "the default for a gun");
        assert_eq!(w.terrain().road_at(FxVec2::from_int(520, 500)), road);
        assert!(w.terrain().woods_at(FxVec2::from_int(520, 500)));
        steps(&mut w, 2);
        limber(&mut w, g);
        send(
            &mut w,
            0,
            CommandKind::PushGun {
                guns: vec![g],
                point: FxVec2::from_int(600, 500),
            },
        );
        let t = secs(&w, 2);
        steps(&mut w, t);
        let a = xy(&w, g);
        let t = secs(&w, 10);
        steps(&mut w, t);
        (xy(&w, g) - a).length().mul_int(10).floor_int()
    };
    let on = run(true);
    assert!((30..=40).contains(&on), "on the road {on} cm/s");
    let off = run(false);
    assert!((12..=20).contains(&off), "woods off a road {off} cm/s");
}

#[test]
fn a_gun_too_heavy_to_push_refuses_the_order_and_a_pushed_gun_arrives_limbered() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    steps(&mut w, 2);
    limber(&mut w, g);
    send(
        &mut w,
        0,
        CommandKind::PushGun {
            guns: vec![g],
            point: FxVec2::from_int(502, 500),
        },
    );
    let t = secs(&w, 20);
    steps(&mut w, t);
    assert_eq!(state(&w, g), GunState::Limbered, "arrived");
    let mut w2 = super::s6_tests::flat_world_with(1, |s| {
        s.vehicles.iter_mut().for_each(|v| {
            if v.served() {
                v.mobility.speed = frontline_content::Fixed::ZERO;
                v.mobility.reverse = frontline_content::Fixed::ZERO;
            }
        });
    });
    let g2 = gun(&mut w2, 0, 500, 500);
    steps(&mut w2, 2);
    limber(&mut w2, g2);
    send(
        &mut w2,
        0,
        CommandKind::PushGun {
            guns: vec![g2],
            point: FxVec2::from_int(600, 500),
        },
    );
    assert_eq!(state(&w2, g2), GunState::Limbered);
}

#[test]
fn a_tow_hitches_carries_the_crew_and_the_gun_follows_on_a_bar() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    let (tq, tow) = tow_for(&mut w, 0, 500, 500);
    steps(&mut w, 2);
    limber(&mut w, g);
    send(&mut w, 0, CommandKind::Hitch { guns: vec![g], tow });
    assert_eq!(state(&w, g), GunState::Hitching);
    let t = secs(&w, 21);
    steps(&mut w, t);
    assert_eq!(state(&w, g), GunState::Hitched);
    let t = secs(&w, 25);
    steps(&mut w, t);
    assert_eq!(w.game().unwrap().riders_of(tow).len(), 6, "the crew rides");
    assert_eq!(
        w.game().unwrap().vehicles[&tow].towing,
        Some(g),
        "the tow names it"
    );
    send(
        &mut w,
        0,
        CommandKind::Move {
            squad: tq,
            target: FxVec2::from_int(200, 500),
        },
    );
    let t = secs(&w, 25);
    steps(&mut w, t);
    let sp = w.game().unwrap().vehicles[&tow]
        .speed
        .mul_int(100)
        .floor_int();
    assert!((400..=650).contains(&sp), "towed speed {sp} cm/s");
    let (gx, tx) = (xy(&w, g), xy(&w, tow));
    assert!(tx.x < gx.x, "the tow leads");
    // Tow half length 277 cm plus the bar 310 cm.
    assert!(
        near_cm((gx - tx).length(), 587, 30),
        "bar {:?}",
        (gx - tx).length()
    );
    assert!(!w.game().unwrap().vehicles[&g].gun_ready());
    // Stop, unhitch: the crew dismounts and is back at the slots.
    send(&mut w, 0, CommandKind::Stop { squad: tq });
    let t = secs(&w, 5);
    steps(&mut w, t);
    send(&mut w, 0, CommandKind::Unhitch { guns: vec![g] });
    assert_eq!(state(&w, g), GunState::Unhitching);
    let t = secs(&w, 21);
    steps(&mut w, t);
    assert_eq!(state(&w, g), GunState::Limbered);
    assert_eq!(w.game().unwrap().vehicles[&tow].towing, None);
    let t = secs(&w, 25);
    steps(&mut w, t);
    assert_eq!(
        slots(&w, g).iter().filter(|s| s.man.is_some()).count(),
        6,
        "the crew serves again"
    );
    assert!(w.game().unwrap().riders_of(tow).is_empty());
}

#[test]
fn a_gun_card_pays_for_its_tow_and_arrives_hitched_with_the_crew_aboard() {
    let mut w = flat_world(1);
    let t = w
        .match_setup()
        .unwrap()
        .squad_type_index("ww2:ger_pak40_squad")
        .unwrap();
    assert_eq!(w.match_setup().unwrap().squad_types[t as usize].cost, 560);
    let mp0 = w.game().unwrap().players[0].manpower;
    let before: Vec<SimId> = w.game().unwrap().vehicles.keys().copied().collect();
    send(&mut w, 0, CommandKind::CallIn { squad_type: t });
    assert_eq!(w.game().unwrap().players[0].manpower, mp0 - 560);
    let added: Vec<SimId> = w
        .game()
        .unwrap()
        .vehicles
        .keys()
        .copied()
        .filter(|id| !before.contains(id))
        .collect();
    assert_eq!(added.len(), 2, "gun and tow");
    let g = *added
        .iter()
        .find(|id| w.game().unwrap().vehicles[id].served.is_some())
        .unwrap();
    assert_eq!(state(&w, g), GunState::Hitched);
    let tow = *added.iter().find(|&&id| id != g).unwrap();
    assert_eq!(w.game().unwrap().vehicles[&tow].towing, Some(g));
    assert_eq!(w.game().unwrap().riders_of(tow).len(), 6);
    assert_eq!(w.game().unwrap().players[0].s9.guns_bought, 1);
}

#[test]
fn the_tow_lost_leaves_the_gun_limbered_and_the_crew_walks_back() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    let (_, tow) = tow_for(&mut w, 0, 500, 500);
    steps(&mut w, 2);
    assert!(w.hitch_gun_now(g, tow));
    steps(&mut w, 2);
    assert_eq!(state(&w, g), GunState::Hitched);
    assert_eq!(w.game().unwrap().riders_of(tow).len(), 6);
    w.edit_vehicle(tow, |vs| vs.bail_order = true);
    let t = secs(&w, 2);
    steps(&mut w, t);
    assert_eq!(state(&w, g), GunState::Limbered, "the tow bailed out");
    assert_eq!(w.game().unwrap().players[0].s9.tows_lost, 1);
    let t = secs(&w, 30);
    steps(&mut w, t);
    assert_eq!(
        slots(&w, g).iter().filter(|s| s.man.is_some()).count(),
        6,
        "the crew serves it where it stands"
    );
}

#[test]
fn a_gun_with_nobody_near_turns_neutral_and_an_enemy_squad_takes_it() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    steps(&mut w, 2);
    let tick0 = w.state.tick;
    kill_crew(&mut w, g);
    let need = secs(&w, 10);
    steps(&mut w, need - 5);
    assert_eq!(w.entities().kind[g.index()], EntityKind::Vehicle);
    steps(&mut w, 10);
    assert_eq!(w.entities().kind[g.index()], EntityKind::Abandoned);
    assert!(w.state.tick - tick0 <= need + 10);
    assert_eq!(w.game().unwrap().players[0].s9.guns_neutral, 1);
    let vs = &w.game().unwrap().vehicles[&g];
    assert_eq!(vs.abandon_cause, crate::bail::AB_GUN);
    assert_eq!(w.entities().combat[g.index()].seen_by, 0b11, "seen by all");
    let pop0 = w.game().unwrap().population(w.entities(), PlayerId(0));
    // An enemy rifle squad comes and serves it.
    let q = spawn(&mut w, 1, "ww2:usa_rifle_squad", 505, 500);
    let hash0 = w.full_state_hash();
    send(
        &mut w,
        1,
        CommandKind::Recrew {
            squad: q,
            vehicle: g,
        },
    );
    assert_ne!(w.full_state_hash(), hash0);
    let t = secs(&w, 40);
    for _ in 0..t {
        w.step(&[]);
        if w.entities().kind[g.index()] == EntityKind::Vehicle {
            break;
        }
    }
    assert_eq!(w.entities().kind[g.index()], EntityKind::Vehicle, "served");
    assert_eq!(w.entities().owner[g.index()], PlayerId(1), "taken");
    assert_eq!(w.game().unwrap().players[1].s9.guns_captured, 1);
    assert_eq!(w.game().unwrap().players[1].crew.captured, 1);
    assert_eq!(
        w.game().unwrap().population(w.entities(), PlayerId(0)),
        pop0
    );
    let n = slots(&w, g).iter().filter(|s| s.man.is_some()).count();
    assert_eq!(n, 6, "six of the rifle squad at the slots");
    // Rounds and parts stayed; the save holds it.
    let bytes = w.save();
    let w2 = World::load(&bytes, 1).unwrap();
    assert_eq!(w2.full_state_hash(), w.full_state_hash());
}

#[test]
fn a_captured_gun_takes_only_rounds_of_its_own_faction() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 400, 300);
    steps(&mut w, 2);
    let q = spawn(&mut w, 1, "ww2:usa_rifle_squad", 405, 300);
    kill_crew(&mut w, g);
    let need = secs(&w, 11);
    steps(&mut w, need);
    send(
        &mut w,
        1,
        CommandKind::Recrew {
            squad: q,
            vehicle: g,
        },
    );
    let t = secs(&w, 40);
    steps(&mut w, t);
    assert_eq!(w.entities().owner[g.index()], PlayerId(1));
    // No crates at all: it keeps what was left (and may not refill).
    let n0 = w.game().unwrap().vehicles[&g].ammo[0];
    w.edit_vehicle(g, |vs| vs.ammo[0] = n0.saturating_sub(5));
    let spent = w.game().unwrap().vehicles[&g].ammo[0];
    let usa = shells(&w, 1);
    let ger = shells(&w, 0);
    // Its captor's own crate does not fit.
    super::s6_tests::add_crate_at(&mut w, 1, usa, FxVec2::from_int(401, 300));
    let t = secs(&w, 30);
    steps(&mut w, t);
    assert_eq!(
        w.game().unwrap().vehicles[&g].ammo[0],
        spent,
        "own crate refused"
    );
    // A captured crate of the gun's faction does; validation accepts it.
    super::s6_tests::add_crate_at(&mut w, 1, ger, FxVec2::from_int(399, 300));
    assert!(World::load(&w.save(), 1).is_ok());
    let t = secs(&w, 60);
    steps(&mut w, t);
    assert!(
        w.game().unwrap().vehicles[&g].ammo[0] > spent,
        "the gun's own crate"
    );
}

#[test]
fn an_order_off_the_gun_takes_the_men_away_and_breaks_the_crew_link() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    steps(&mut w, 2);
    let crew = w.game().unwrap().vehicles[&g]
        .served
        .as_ref()
        .unwrap()
        .crew
        .unwrap();
    send(
        &mut w,
        0,
        CommandKind::Move {
            squad: crew,
            target: FxVec2::from_int(520, 500),
        },
    );
    assert!(slots(&w, g).iter().all(|s| s.man.is_none()));
    let vs = &w.game().unwrap().vehicles[&g];
    assert_eq!(vs.served.as_ref().unwrap().crew, None);
    let t = secs(&w, 25);
    steps(&mut w, t);
    assert_eq!(
        w.entities().kind[g.index()],
        EntityKind::Abandoned,
        "nobody came back, nobody within 15 m"
    );
}

#[test]
fn a_target_outside_the_arc_re_lays_the_trail_and_the_gun_cannot_fire_meanwhile() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    steps(&mut w, 2);
    // A FaceUnits order re-lays it to the point.
    send(
        &mut w,
        0,
        CommandKind::FaceUnits {
            units: vec![g],
            toward: FxVec2::from_int(500, 800),
        },
    );
    steps(&mut w, 1);
    let want = Fx::atan2(Fx::from_int(300), Fx::ZERO);
    let sv = w.game().unwrap().vehicles[&g].served.clone().unwrap();
    assert!(sv.relay_ticks > 0, "re-laying");
    assert_eq!(sv.relay_to, Some(want));
    assert!(!w.game().unwrap().vehicles[&g].gun_ready());
    let t = secs(&w, 16);
    steps(&mut w, t);
    assert_eq!(w.entities().transform[g.index()].yaw, want);
    assert!(w.game().unwrap().vehicles[&g].gun_ready());
    // A tank well outside the arc: gunnery asks for the turn on its own.
    let t2 = spawn(&mut w, 1, "ww2:usa_m4a3_75_squad", 780, 500);
    let _ = members(&w, t2);
    let mut laid = false;
    let t = secs(&w, 10);
    for _ in 0..t {
        w.step(&[]);
        if w.game().unwrap().vehicles[&g]
            .served
            .as_ref()
            .unwrap()
            .relay_ticks
            > 0
        {
            laid = true;
            break;
        }
    }
    assert!(laid, "a seen tank outside the arc re-lays the trail");
}

#[test]
fn engineers_dig_a_pit_around_a_gun_from_its_rim() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    let q = spawn(&mut w, 0, "ww2:ger_engineer_squad", 500, 515);
    steps(&mut w, 2);
    let z0 = w.entities().transform[g.index()].pos.z;
    let line = vec![FxVec2::from_int(498, 500), FxVec2::from_int(502, 500)];
    send(
        &mut w,
        0,
        CommandKind::Dig {
            squad: q,
            line,
            profile: frontline_content::DigProfile::GunPit,
        },
    );
    let t = secs(&w, 240);
    steps(&mut w, t);
    assert_eq!(w.game().unwrap().squads[&q].order, crate::game::Order::Idle);
    let dz = (z0 - w.entities().transform[g.index()].pos.z).to_f32();
    assert!(dz > 0.4, "the gun settled into the pit: {dz}");
}

#[test]
fn the_gun_pit_lowers_the_ground_under_the_gun_and_does_not_block_its_wheels() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    steps(&mut w, 2);
    let z0 = w.entities().transform[g.index()].pos.z;
    let line = [FxVec2::from_int(498, 500), FxVec2::from_int(502, 500)];
    assert!(w.dig_pit_now(&line, frontline_content::DigProfile::GunPit));
    steps(&mut w, 2);
    let z1 = w.entities().transform[g.index()].pos.z;
    assert!(
        (z0 - z1).to_f32() > 0.3,
        "settled into the pit {z0:?} -> {z1:?}"
    );
    assert!(!w.game().unwrap().vehicles[&g].on_blocked);
    limber(&mut w, g);
    send(
        &mut w,
        0,
        CommandKind::PushGun {
            guns: vec![g],
            point: FxVec2::from_int(530, 500),
        },
    );
    assert_eq!(state(&w, g), GunState::Pushing);
    let g0 = xy(&w, g);
    let t = secs(&w, 10);
    steps(&mut w, t);
    assert!(xy(&w, g) != g0, "wheels roll out of it");
}

#[test]
fn every_build_b_field_changes_the_hash_and_validation_catches_the_tow_link() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    let (_, tow) = tow_for(&mut w, 0, 500, 500);
    steps(&mut w, 2);
    let base = w.full_state_hash();
    let orig = w.game().unwrap().vehicles[&g].clone();
    w.edit_vehicle(g, |v| v.served.as_mut().unwrap().relay_to = Some(Fx::ONE));
    assert_ne!(w.full_state_hash(), base, "relay_to");
    w.edit_vehicle(g, |v| *v = orig);
    assert_eq!(w.full_state_hash(), base);
    w.state.game.as_mut().unwrap().players[0].s9.guns_captured = 9;
    assert_ne!(w.full_state_hash(), base, "s9 counters");
    w.state.game.as_mut().unwrap().players[0].s9.guns_captured = 0;
    assert_eq!(w.full_state_hash(), base);
    // The links.
    w.edit_vehicle(tow, |vs| vs.towing = Some(g));
    assert!(
        World::load(&w.save(), 1).is_err(),
        "gun does not name the tow"
    );
    w.edit_vehicle(tow, |vs| vs.towing = None);
    w.edit_vehicle(g, |v| {
        let s = v.served.as_mut().unwrap();
        s.hitched = Some(tow);
    });
    assert!(World::load(&w.save(), 1).is_err(), "hitched while emplaced");
    w.edit_vehicle(g, |v| {
        let s = v.served.as_mut().unwrap();
        s.hitched = None;
        s.relay_to = Some(Fx::ONE);
    });
    assert!(World::load(&w.save(), 1).is_err(), "a turn with no turning");
    w.edit_vehicle(g, |v| v.served.as_mut().unwrap().relay_to = None);
    assert!(World::load(&w.save(), 1).is_ok());
    // A captured crate's owner stays in range and loads.
    let ger = shells(&w, 0);
    super::s6_tests::add_crate_at(&mut w, 1, ger, FxVec2::from_int(450, 450));
    assert!(World::load(&w.save(), 1).is_ok());
}

/// A gun's whole life through commands, for the determinism check.
fn gun_scene(w: &mut World, t: u64, g: SimId, tow: SimId, tq: SquadId) -> Vec<Command> {
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let mk = |kind| Command {
        player: PlayerId(0),
        seq: t as u32,
        kind,
    };
    match t {
        x if x == hz => vec![mk(CommandKind::SetAmbush {
            guns: vec![g],
            range_m: 300,
        })],
        x if x == 2 * hz => vec![mk(CommandKind::PackGun { guns: vec![g] })],
        x if x == 22 * hz => vec![mk(CommandKind::PushGun {
            guns: vec![g],
            point: FxVec2::from_int(495, 505),
        })],
        x if x == 60 * hz => vec![mk(CommandKind::Hitch { guns: vec![g], tow })],
        x if x == 100 * hz => vec![mk(CommandKind::Move {
            squad: tq,
            target: FxVec2::from_int(460, 500),
        })],
        x if x == 130 * hz => vec![mk(CommandKind::Unhitch { guns: vec![g] })],
        x if x == 170 * hz => vec![mk(CommandKind::Emplace {
            guns: vec![g],
            facing: FxVec2::from_int(700, 500),
        })],
        x if x == 210 * hz => vec![mk(CommandKind::FaceUnits {
            units: vec![g],
            toward: FxVec2::from_int(600, 700),
        })],
        _ => vec![],
    }
}

#[test]
fn guns_are_deterministic_through_their_whole_life_at_every_thread_count() {
    let mut s0 = flat_world(1);
    let g = gun(&mut s0, 0, 500, 500);
    let (tq, tow) = tow_for(&mut s0, 0, 500, 500);
    let rifles = spawn(&mut s0, 1, "ww2:usa_rifle_squad", 790, 505);
    steps(&mut s0, 5);
    let save = s0.save();
    let hz = 30u64;
    let run = |threads: usize, split: bool| {
        let mut w = World::load(&save, threads).unwrap();
        let mut hashes = Vec::new();
        for t in 0..420 * hz {
            if split && t % (20 * hz) == 3 {
                w = World::load(&w.save(), threads).unwrap();
            }
            let mut cmds = gun_scene(&mut w, t, g, tow, tq);
            // The crew dies, the gun goes neutral, an enemy squad takes it.
            if t == 240 * hz {
                kill_crew(&mut w, g);
            }
            if t == 252 * hz {
                cmds.push(Command {
                    player: PlayerId(1),
                    seq: t as u32,
                    kind: CommandKind::Recrew {
                        squad: rifles,
                        vehicle: g,
                    },
                });
            }
            w.step(&cmds);
            if !split {
                if t == 95 * hz {
                    assert_eq!(state(&w, g), GunState::Hitched, "hitched by 95 s");
                }
                if t == 128 * hz {
                    assert!(xy(&w, g).x < Fx::from_int(490), "the tow pulled it");
                }
                if t == 220 * hz {
                    assert_eq!(state(&w, g), GunState::Emplaced, "emplaced");
                }
                if t == 254 * hz {
                    assert_eq!(
                        w.entities().kind[g.index()],
                        EntityKind::Abandoned,
                        "neutral"
                    );
                }
                if t == 419 * hz {
                    assert_eq!(w.entities().owner[g.index()], PlayerId(1), "captured");
                }
            }
            if t % 90 == 0 {
                hashes.push(w.state_hash());
            }
        }
        (hashes, w.full_state_hash())
    };
    let reference = run(1, false);
    for threads in [1, 2, 3, 8] {
        assert_eq!(run(threads, true), reference, "threads {threads}");
    }
}

#[test]
fn own_infantry_serve_an_unmanned_gun_before_it_turns_neutral() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    let q = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 505, 500);
    steps(&mut w, 2);
    kill_crew(&mut w, g);
    steps(&mut w, 2);
    assert_eq!(w.entities().kind[g.index()], EntityKind::Vehicle, "held");
    send(
        &mut w,
        0,
        CommandKind::Recrew {
            squad: q,
            vehicle: g,
        },
    );
    let t = secs(&w, 40);
    for _ in 0..t {
        w.step(&[]);
        if slots(&w, g).iter().any(|s| s.man.is_some()) {
            break;
        }
    }
    assert!(
        slots(&w, g).iter().any(|s| s.man.is_some()),
        "own men took the slots"
    );
    assert_eq!(w.entities().owner[g.index()], PlayerId(0));
    let sv = w.game().unwrap().vehicles[&g].served.clone().unwrap();
    assert_eq!(sv.crew, Some(q));
    // Riflemen are untrained at the gun.
    let vs = &w.game().unwrap().vehicles[&g];
    let v = w.match_setup().unwrap().vehicles[vs.type_idx as usize].clone();
    let t = secs(&w, 2);
    steps(&mut w, t);
    assert!(
        w.game().unwrap().vehicles[&g]
            .seat(&v, frontline_content::SeatRole::Gunner)
            .is_some_and(|s| s.untrained)
    );
}

// ---- S9 Fix: review findings ----
const RIFLE_DE: &str = "ww2:ger_grenadiergruppe";
#[test]
fn fix_a_hitched_gun_wrecked_still_saves_and_loads() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    let (_, tow) = tow_for(&mut w, 0, 500, 500);
    steps(&mut w, 2);
    assert!(w.hitch_gun_now(g, tow));
    steps(&mut w, 2);
    assert!(w.wreck_vehicle(g));
    steps(&mut w, 3);
    let bytes = w.save();
    assert_eq!(state(&w, g), GunState::Limbered);
    assert!(World::load(&bytes, 1).is_ok());
}

#[test]
fn fix_a_gun_taken_first_is_not_boarded_by_another_player() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    steps(&mut w, 2);
    kill_crew(&mut w, g);
    let t = secs(&w, 11);
    steps(&mut w, t);
    assert_eq!(w.entities().kind[g.index()], EntityKind::Abandoned);
    let a = spawn(&mut w, 1, "ww2:usa_rifle_squad", 505, 500);
    let b = spawn(&mut w, 0, RIFLE_DE, 495, 500);
    // A keeps three men, so three slots stay free.
    let am = members(&w, a);
    for m in am.iter().skip(3) {
        assert!(w.kill_soldier(*m));
    }
    steps(&mut w, 1);
    w.step(&[
        Command {
            player: PlayerId(1),
            seq: 1,
            kind: CommandKind::Recrew {
                squad: a,
                vehicle: g,
            },
        },
        Command {
            player: PlayerId(0),
            seq: 1,
            kind: CommandKind::Recrew {
                squad: b,
                vehicle: g,
            },
        },
    ]);
    let t = secs(&w, 60);
    let mut first = None;
    for k in 0..t {
        w.step(&[]);
        if first.is_none() && w.entities().kind[g.index()] == EntityKind::Vehicle {
            first = Some(k);
        }
    }
    let sv = w.game().unwrap().vehicles[&g].served.clone().unwrap();
    let owners: Vec<u8> = sv
        .slots
        .iter()
        .filter_map(|s| s.man)
        .map(|m| w.entities().owner[m.index()].0)
        .collect();
    let r = World::load(&w.save(), 1);
    assert!(r.is_ok());
    assert!(owners.iter().all(|&o| o == 1));
    assert_eq!(w.game().unwrap().squads[&b].order, Order::Idle);
}

#[test]
fn fix_embark_takes_the_crew_off_the_gun() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    let (_, tow) = tow_for(&mut w, 0, 500, 500);
    steps(&mut w, 2);
    let crew = w.game().unwrap().vehicles[&g]
        .served
        .as_ref()
        .unwrap()
        .crew
        .unwrap();
    send(
        &mut w,
        0,
        CommandKind::Embark {
            squads: vec![crew],
            vehicle: tow,
        },
    );
    let t = secs(&w, 30);
    steps(&mut w, t);
    let e = w.entities();
    let sv = w.game().unwrap().vehicles[&g].served.clone().unwrap();
    let both = w.game().unwrap().squads[&crew]
        .members
        .iter()
        .filter(|m| e.unit[m.index()].serving.is_some() && e.unit[m.index()].riding.is_some())
        .count();
    let serving = sv.slots.iter().filter(|s| s.man.is_some()).count();
    assert_eq!(both, 0);
    assert_eq!(serving, 0, "off the slots");
    assert_eq!(sv.crew, None, "the gun no longer names them");
    assert!(World::load(&w.save(), 1).is_ok());
}

/// Their own tow is where a hitched gun's crew rides: still its crew.
#[test]
fn fix_embark_on_the_guns_own_tow_keeps_the_crew() {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    let (_, tow) = tow_for(&mut w, 0, 500, 500);
    steps(&mut w, 2);
    assert!(w.hitch_gun_now(g, tow));
    steps(&mut w, 2);
    let crew = w.game().unwrap().vehicles[&g]
        .served
        .as_ref()
        .unwrap()
        .crew
        .unwrap();
    send(
        &mut w,
        0,
        CommandKind::Embark {
            squads: vec![crew],
            vehicle: tow,
        },
    );
    steps(&mut w, 2);
    assert_eq!(
        w.game().unwrap().vehicles[&g].served.as_ref().unwrap().crew,
        Some(crew)
    );
}

/// A gun card near the cap counts its crew.
#[test]
fn fix_a_gun_card_counts_its_crew_against_the_pop_cap() {
    let refused = |extra: u32| {
        let cap = std::cell::Cell::new(0);
        let probe = flat_world(1);
        let g = probe.game().unwrap();
        let s = g.setup();
        let ty = s.squad_types.iter().position(|t| t.id == PAK).unwrap() as u16;
        let card = &s.squad_types[ty as usize];
        let tow = card
            .tow_card
            .map_or(0, |t| s.squad_types[t as usize].pop(&s.vehicles));
        cap.set(g.population(probe.entities(), PlayerId(0)) + card.pop(&s.vehicles) + tow + extra);
        let w = flat_world_with(1, |s| s.rules.pop_cap = cap.get());
        let g = w.game().unwrap();
        g.call_in_check(w.entities(), PlayerId(0), ty, w.tick())
    };
    assert!(matches!(
        refused(5),
        Err(crate::game::CallInRefusal::PopCap)
    ));
    assert!(!matches!(
        refused(6),
        Err(crate::game::CallInRefusal::PopCap)
    ));
}

fn ambush_run(with_near: bool) -> (Option<SimId>, u32, u16, Vec<SimId>) {
    let mut w = flat_world(1);
    let g = gun(&mut w, 0, 500, 500);
    steps(&mut w, 2);
    send(
        &mut w,
        0,
        CommandKind::FaceUnits {
            units: vec![g],
            toward: FxVec2::from_int(500, 800),
        },
    );
    let t = secs(&w, 16);
    steps(&mut w, t);
    send(
        &mut w,
        0,
        CommandKind::SetAmbush {
            guns: vec![g],
            range_m: 400,
        },
    );
    let t2 = spawn(&mut w, 1, "ww2:usa_cckw_squad", 500, 850);
    let t2id = members(&w, t2)[0];
    let mut ids = vec![t2id];
    if with_near {
        let t1 = spawn(&mut w, 1, "ww2:usa_cckw_squad", 650, 500);
        ids.push(members(&w, t1)[0]);
    }
    let t = secs(&w, 45);
    steps(&mut w, t);
    let vs = &w.game().unwrap().vehicles[&g];
    (
        vs.target,
        w.game().unwrap().players[0].s9.gun_rounds,
        vs.served.as_ref().unwrap().relay_ticks,
        ids,
    )
}

#[test]
fn fix_a_nearer_vehicle_outside_the_arc_does_not_freeze_an_ambush() {
    let (_, control, _, _) = ambush_run(false);
    assert!(control > 0, "the control fires");
    let (_, rounds, _, _) = ambush_run(true);
    assert!(
        rounds > 0,
        "a truck outside the arc at 150 m does not freeze it"
    );
}

#[test]
fn fix_a_captured_crate_stays_out_of_the_captors_stock() {
    use super::s6_tests::{add_crate_at, add_structure, built, def};
    use frontline_content::setup::StockKind;
    let mut w = flat_world(1);
    let f = w.match_setup().unwrap().players[0].faction;
    let dump = def(&w, f, StockKind::Hq, None);
    let s = built(&w, 0, dump, FxVec2::from_int(400, 300), [0, 0, 0]);
    let id = add_structure(&mut w, s);
    let usa = shells(&w, 1);
    // A US crate player 0 has captured, set down at his own German dump.
    add_crate_at(&mut w, 0, usa, FxVec2::from_int(401, 300));
    let t = secs(&w, 5);
    steps(&mut w, t);
    let st = w.game().unwrap().supply6();
    assert_eq!(st.structures[&id].stock_g[2], 0);
}
