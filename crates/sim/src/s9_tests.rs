//! S9 Build A tests (docs/slices/S9-at-guns.md 4, 5, 15): the gun crew as
//! soldiers in service slots, the seat mirror, pinned gunner, auto-swap,
//! the AP core reaching a man behind the shield, hold fire, and the save
//! and validation checks of the new state.

use super::s6_tests::{flat_world, members, spawn, steps};
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
