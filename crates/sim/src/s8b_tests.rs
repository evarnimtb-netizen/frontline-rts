//! S8b Build B tests (docs/slices/S8b-trucks-recon.md 7, 12): supply trucks
//! through the one forward source function, loading in base, `DropCrates`,
//! the cargo box, validation of the new state.

use frontline_content::setup::{StockKind, SupplyKind};

use super::s6_tests::{
    HZ, acc, add_built, add_crate_at, def, flat_world, flat_world_with, members, run, spawn, steps,
};
use super::*;
use crate::damage::{Dealt, FireRolls, Inner, KO_RACK};

const BLITZ: &str = "ww2:ger_opel_blitz_squad";
const TANK: &str = "ww2:ger_pz4_h_squad";
const SHELLS: usize = 2;

fn truck(w: &mut World, p: u8, x: i32, y: i32, kg: [u32; 3]) -> (SquadId, SimId) {
    let sq = spawn(w, p, BLITZ, x, y);
    let v = members(w, sq)[0];
    w.edit_vehicle(v, |vs| {
        vs.cargo_g = [kg[0] * 1000, kg[1] * 1000, kg[2] * 1000];
    });
    (sq, v)
}

/// A tank at (x, y) with an emptied rack (rounds in `ammo` from the def).
fn dry_tank(w: &mut World, p: u8, card: &str, x: i32, y: i32) -> SimId {
    let sq = spawn(w, p, card, x, y);
    let v = members(w, sq)[0];
    w.edit_vehicle(v, |vs| {
        vs.ammo.iter_mut().for_each(|a| *a = 0);
        vs.loaded = None;
    });
    v
}

fn cargo(w: &World, v: SimId) -> [u32; 3] {
    w.game().unwrap().vehicles[&v].cargo_g
}

/// Rounds a tank holds and their grams.
fn rack(w: &World, v: SimId) -> (u32, u32) {
    let g = w.game().unwrap();
    let vs = &g.vehicles[&v];
    let vt = &g.setup().vehicles[vs.type_idx as usize];
    let ammo = vt.main_ammo();
    let mut grams = 0u32;
    let mut held = 0u32;
    for (k, &n) in vs.ammo.iter().enumerate() {
        let n = n as u32 + (vs.loaded == Some(k as u8)) as u32;
        held += n;
        grams += n * g.setup().ammo[ammo[k].ammo as usize].supply.unwrap().g;
    }
    (held, grams)
}

#[test]
fn a_tank_at_15m_of_a_truck_rearms_at_one_round_per_4s_and_the_stock_falls() {
    let mut w = flat_world(1);
    let (_, t) = truck(&mut w, 0, 515, 500, [0, 0, 600]);
    let tank = dry_tank(&mut w, 0, TANK, 500, 500);
    let t0 = w.tick();
    let n = 41 * HZ;
    steps(&mut w, n);
    let due = (t0..t0 + n)
        .filter(|t| (t + tank.0 as u64).is_multiple_of(4 * HZ))
        .count() as u32;
    let a = acc(&w, 0);
    assert_eq!(a.tank_rounds_fwd, due, "one standard round per 4 s");
    assert_eq!(a.truck_tank_rounds, a.tank_rounds_fwd + a.tank_rare_fwd);
    let (held, grams) = rack(&w, tank);
    assert_eq!(held, a.tank_rounds_fwd + a.tank_rare_fwd);
    assert_eq!(cargo(&w, t)[SHELLS], 600_000 - grams, "the stock falls by the kg");
    assert_eq!(a.truck_out_g, grams as u64);
}

#[test]
fn a_tank_between_an_hq_and_two_trucks_rearms_no_faster_than_at_one_source() {
    let mut w = flat_world(1);
    let hq = def(&w, 0, StockKind::Hq, None);
    let hid = add_built(&mut w, 0, hq, FxVec2::from_int(505, 500), [0, 0, 900]);
    let (_, a) = truck(&mut w, 0, 490, 500, [0, 0, 900]);
    let (_, b) = truck(&mut w, 0, 510, 510, [0, 0, 900]);
    let tank = dry_tank(&mut w, 0, TANK, 500, 500);
    let t0 = w.tick();
    let n = 41 * HZ;
    steps(&mut w, n);
    let due = (t0..t0 + n)
        .filter(|t| (t + tank.0 as u64).is_multiple_of(4 * HZ))
        .count() as u32;
    let acct = acc(&w, 0);
    assert_eq!(acct.tank_rounds_fwd, due, "rates never stack");
    assert_eq!(cargo(&w, a)[SHELLS], 900_000, "a tie goes to the structure");
    assert_eq!(cargo(&w, b)[SHELLS], 900_000);
    let (_, grams) = rack(&w, tank);
    let s = &w.game().unwrap().supply6().structures[&hid];
    assert_eq!(s.stock_g[SHELLS], 900_000 - grams);
}

#[test]
fn the_faster_source_wins_over_a_structure() {
    // A truck with a shorter interval beats the structure tie.
    let mut w = flat_world_with(1, |s| {
        for v in s.vehicles.iter_mut() {
            if let Some(t) = v.supply.as_mut() {
                t.round_interval_ticks = 2 * 30;
            }
        }
    });
    let hq = def(&w, 0, StockKind::Hq, None);
    add_built(&mut w, 0, hq, FxVec2::from_int(505, 500), [0, 0, 900]);
    let (_, a) = truck(&mut w, 0, 490, 500, [0, 0, 900]);
    let tank = dry_tank(&mut w, 0, TANK, 500, 500);
    let t0 = w.tick();
    let n = 41 * HZ;
    steps(&mut w, n);
    let due = (t0..t0 + n)
        .filter(|t| (t + tank.0 as u64).is_multiple_of(2 * HZ))
        .count() as u32;
    assert_eq!(acc(&w, 0).tank_rounds_fwd, due);
    assert!(cargo(&w, a)[SHELLS] < 900_000, "the truck served");
}

#[test]
fn a_truck_loads_stopped_in_base_paying_per_kg_and_pauses_without_mp() {
    let mut w = flat_world_with(1, |s| {
        s.rules.start_manpower = 1_000;
        s.rules.income_per_min = 0;
        s.rules.point_income_per_min = 0;
    });
    let b = w.game().unwrap().base_of(PlayerId(0));
    let (x, y) = (b.x.floor_int(), b.y.floor_int());
    let sq = spawn(&mut w, 0, BLITZ, x, y);
    let v = members(&w, sq)[0];
    let mp0 = w.game().unwrap().players()[0].manpower;
    steps(&mut w, 60 * HZ);
    let c = cargo(&w, v);
    let g = w.game().unwrap().setup();
    let d = g.vehicles[w.game().unwrap().vehicles[&v].type_idx as usize]
        .supply
        .unwrap();
    assert_eq!(c[0], d.load_g[0], "small arms loaded");
    assert_eq!(c[1], d.load_g[1], "explosives loaded");
    assert!(c[2] >= 32_000 * 5, "shells on the crate queue: {}", c[2]);
    let sp = g.rules.supply.as_ref().unwrap();
    let paid: u64 = SupplyKind::ALL
        .iter()
        .map(|k| crate::supply::micro_price(g, *k, c[k.index()]))
        .sum();
    let a = acc(&w, 0);
    assert_eq!(a.truck_load_micro, paid, "each gram paid once");
    assert!(sp.small_arms_mp_per_t > 0);
    let mp1 = w.game().unwrap().players()[0].manpower;
    assert!(mp1 < mp0, "paid from manpower");
    // Out of manpower: loading pauses, nothing goes into debt.
    w.edit_vehicle(v, |vs| vs.cargo_g = [0; 3]);
    w.edit_player(PlayerId(0), |p| {
        p.manpower = 0;
        p.ammo.credit = 0;
    });
    let before = cargo(&w, v);
    steps(&mut w, 15 * HZ);
    assert_eq!(cargo(&w, v), before);
    assert_eq!(w.game().unwrap().players()[0].manpower, 0);
}

#[test]
fn a_truck_does_not_load_outside_the_base_or_rolling() {
    let mut w = flat_world(1);
    let (_, v) = truck(&mut w, 0, 500, 500, [0, 0, 0]);
    steps(&mut w, 30 * HZ);
    assert_eq!(cargo(&w, v), [0; 3], "outside the base");
}

#[test]
fn a_tank_in_base_rearms_from_the_base_not_a_truck() {
    let mut w = flat_world(1);
    let b = w.game().unwrap().base_of(PlayerId(0));
    let (x, y) = (b.x.floor_int(), b.y.floor_int());
    let (_, t) = truck(&mut w, 0, x + 6, y, [0, 0, 300]);
    let tank = dry_tank(&mut w, 0, TANK, x, y);
    steps(&mut w, 30 * HZ);
    assert!(rack(&w, tank).0 > 0, "the base rearmed it");
    assert_eq!(acc(&w, 0).truck_tank_rounds, 0);
    assert!(cargo(&w, t)[SHELLS] >= 300_000, "never drawn");
}

#[test]
fn men_within_8m_take_small_arms_from_a_truck_not_at_12m() {
    let mut w = flat_world(1);
    let (_, t) = truck(&mut w, 0, 500, 500, [30, 0, 0]);
    let near = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 504, 500);
    let far = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 512, 520);
    for sq in [near, far] {
        for m in members(&w, sq) {
            w.edit_soldier(m, |k, _| k.rounds = [0; frontline_content::setup::MAX_FEEDS]);
        }
    }
    steps(&mut w, 10 * HZ);
    let a = acc(&w, 0);
    assert!(a.truck_men_rounds > 0, "served");
    assert!(cargo(&w, t)[0] < 30_000, "drawn");
    for m in members(&w, far) {
        assert_eq!(w.entities().kit()[m.index()].rounds[0], 0, "out of reach");
    }
}

#[test]
fn a_mortar_never_takes_bombs_from_a_truck() {
    let mut w = flat_world(1);
    let (_, t) = truck(&mut w, 0, 500, 500, [0, 0, 300]);
    let m = spawn(&mut w, 0, "ww2:ger_mortar_team", 503, 500);
    for k in members(&w, m) {
        w.edit_soldier(k, |kit, c| {
            kit.rounds = [0; frontline_content::setup::MAX_FEEDS];
            c.mag = 0;
        });
    }
    steps(&mut w, 30 * HZ);
    assert_eq!(cargo(&w, t)[SHELLS], 300_000, "S8b-4");
    assert_eq!(acc(&w, 0).structure_bombs + acc(&w, 0).crate_bombs, 0);
    // A crate it drops does feed the mortar.
    let c = def(&w, 0, StockKind::Crate, Some(SupplyKind::Shells));
    add_crate_at(&mut w, 0, c, FxVec2::from_int(503, 501));
    steps(&mut w, 30 * HZ);
    assert!(acc(&w, 0).crate_bombs > 0);
}

#[test]
fn belts_fill_at_fifty_rounds_a_second_from_a_truck() {
    let mut w = flat_world(1);
    let (_, t) = truck(&mut w, 0, 515, 500, [40, 0, 0]);
    let sq = spawn(&mut w, 0, TANK, 500, 500);
    let v = members(&w, sq)[0];
    w.edit_vehicle(v, |vs| {
        vs.mgs[0].spent = 500;
        vs.mgs[1].spent = 100;
    });
    steps(&mut w, 2 * HZ);
    let vs = &w.game().unwrap().vehicles[&v];
    let left = vs.mgs[0].spent + vs.mgs[1].spent;
    assert!((600 - 100..600).contains(&left), "at most 2 s of 50: left {left}");
    assert_eq!(acc(&w, 0).fwd_belt_rounds, 600 - left);
    assert!(cargo(&w, t)[0] < 40_000);
    steps(&mut w, 11 * HZ);
    let vs = &w.game().unwrap().vehicles[&v];
    assert_eq!(vs.mgs[0].spent + vs.mgs[1].spent, 0, "filled");
}

#[test]
fn drop_crates_moves_grams_into_crates_and_charges_nothing() {
    let mut w = flat_world(1);
    let (_, v) = truck(&mut w, 0, 500, 500, [0, 0, 96]);
    let mp0 = w.game().unwrap().players()[0].manpower;
    let credit0 = w.game().unwrap().players()[0].ammo.credit;
    let point = FxVec2::from_int(540, 500);
    run(
        &mut w,
        0,
        1,
        CommandKind::DropCrates {
            vehicle: v,
            point,
            count: 2,
            kind: SupplyKind::Shells,
        },
    );
    assert!(w.game().unwrap().vehicles[&v].drop.is_some());
    steps(&mut w, 40 * HZ);
    let g = w.game().unwrap();
    assert_eq!(g.vehicles[&v].drop, None, "done");
    assert!(
        (w.entities().transform()[v.index()].pos.xy() - point).length() <= Fx::from_int(8),
        "drove to the point"
    );
    let a = acc(&w, 0);
    assert_eq!(a.truck_crates_dropped, 2);
    assert_eq!(cargo(&w, v)[SHELLS], 96_000 - a.truck_dropped_g as u32);
    assert_eq!(a.truck_dropped_g, 64_000);
    let crates = &g.supply6().crates;
    assert_eq!(crates.len(), 2);
    assert!(crates.values().all(|c| c.carrier.is_none() && c.g == 32_000));
    assert!(g.players()[0].manpower >= mp0 && g.players()[0].ammo.credit == credit0);
}

#[test]
fn drop_crates_refuses_what_it_cannot_do() {
    let mut w = flat_world(1);
    let (_, v) = truck(&mut w, 0, 500, 500, [0, 0, 0]);
    let drop = |vehicle, count, kind, x| CommandKind::DropCrates {
        vehicle,
        point: FxVec2::from_int(x, 500),
        count,
        kind,
    };
    // Empty cargo.
    run(&mut w, 0, 1, drop(v, 1, SupplyKind::Shells, 540));
    let g = w.game().unwrap();
    assert_eq!(g.vehicles[&v].drop, None);
    let tank = spawn(&mut w, 0, TANK, 480, 500);
    let tv = members(&w, tank)[0];
    run(&mut w, 0, 2, drop(tv, 1, SupplyKind::Shells, 540));
    assert_eq!(w.game().unwrap().vehicles[&tv].drop, None, "not a truck");
    w.edit_vehicle(v, |vs| vs.cargo_g[SHELLS] = 96_000);
    run(&mut w, 1, 3, drop(v, 1, SupplyKind::Shells, 540));
    assert_eq!(w.game().unwrap().vehicles[&v].drop, None, "not the enemy's");
    run(&mut w, 0, 4, drop(v, 0, SupplyKind::Shells, 540));
    assert_eq!(w.game().unwrap().vehicles[&v].drop, None, "no crates");
    run(&mut w, 0, 5, drop(v, 9, SupplyKind::Shells, 540));
    assert_eq!(w.game().unwrap().vehicles[&v].drop, None, "over MAX_DROP");
    run(&mut w, 0, 6, drop(v, 1, SupplyKind::Shells, 99_999));
    assert_eq!(w.game().unwrap().vehicles[&v].drop, None, "off the map");
    assert!(w.game().unwrap().supply6().crates.is_empty());
    // The cap.
    let cap = w.game().unwrap().setup().rules.supply.as_ref().unwrap().crate_cap;
    let c = def(&w, 0, StockKind::Crate, Some(SupplyKind::SmallArms));
    for k in 0..cap {
        add_crate_at(&mut w, 0, c, FxVec2::from_int(300 + k as i32, 300));
    }
    run(&mut w, 0, 7, drop(v, 1, SupplyKind::Shells, 540));
    assert_eq!(w.game().unwrap().vehicles[&v].drop, None, "at the crate cap");
}

#[test]
fn another_order_ends_a_drop() {
    let mut w = flat_world(1);
    let (sq, v) = truck(&mut w, 0, 500, 500, [0, 0, 96]);
    run(
        &mut w,
        0,
        1,
        CommandKind::DropCrates {
            vehicle: v,
            point: FxVec2::from_int(600, 500),
            count: 1,
            kind: SupplyKind::Shells,
        },
    );
    steps(&mut w, HZ);
    run(
        &mut w,
        0,
        2,
        CommandKind::Move {
            squad: sq,
            target: FxVec2::from_int(500, 600),
        },
    );
    assert_eq!(w.game().unwrap().vehicles[&v].drop, None);
    steps(&mut w, 30 * HZ);
    assert_eq!(acc(&w, 0).truck_crates_dropped, 0);
}

/// `count` deal attempts on `v`'s cargo box for `dmg`; the KO cause if any.
fn deal_cargo(w: &mut World, v: SimId, dmg: u32) -> Option<u8> {
    let d = [Dealt {
        inner: Inner::Part(frontline_content::ComponentKind::Cargo),
        dmg,
    }];
    let mut f = FireRolls::default();
    super::tests::deal_now(w, v, &d, 7, 3, &mut f)
}

#[test]
fn a_hit_on_a_loaded_cargo_box_detonates_and_cooks_off_an_empty_one_does_not() {
    let mut blew = 0;
    for seed in 0..40u64 {
        let mut w = flat_world(1);
        let (_, v) = truck(&mut w, 0, 500, 500, [0, 0, 320]);
        let k = w.tick() + seed;
        let d = [Dealt {
            inner: Inner::Part(frontline_content::ComponentKind::Cargo),
            dmg: 200,
        }];
        let mut f = FireRolls::default();
        let ko = super::tests::deal_now(&mut w, v, &d, k, seed as u32, &mut f);
        if ko == Some(KO_RACK) {
            blew += 1;
            // The Status stage's knock-out (the damage stage returns the
            // cause): the cook-off from the cargo.
            let pos = w.entities().transform()[v.index()].pos;
            let tick = w.tick();
            w.state
                .game
                .as_mut()
                .unwrap()
                .truck_wrecked(v, 0, tick, pos, true, 1, 0);
            let a = acc(&w, 0);
            assert_eq!(a.cargo_detonations, 1);
            assert_eq!(a.cookoffs, 1, "a cook-off started");
            assert_eq!(a.truck_lost_g, 320_000);
            assert_eq!(cargo(&w, v), [0; 3], "a wreck's cargo is lost");
        }
    }
    assert!(blew > 0, "the rack rule detonates some");
    // Empty: no detonation and no fire.
    let mut w = flat_world(1);
    let (_, v) = truck(&mut w, 0, 500, 500, [0; 3]);
    assert_eq!(deal_cargo(&mut w, v, 200), None);
    assert_eq!(w.game().unwrap().vehicles[&v].ammo_fire, 0);
    // A small load below the minimum burns but never detonates.
    for seed in 0..30u64 {
        let mut w = flat_world(1);
        let (_, v) = truck(&mut w, 0, 500, 500, [100, 0, 10]);
        let d = [Dealt {
            inner: Inner::Part(frontline_content::ComponentKind::Cargo),
            dmg: 200,
        }];
        let mut f = FireRolls::default();
        let k = w.tick() + seed;
        assert_eq!(
            super::tests::deal_now(&mut w, v, &d, k, seed as u32, &mut f),
            None,
            "below the minimum"
        );
    }
}

#[test]
fn the_new_state_is_validated_on_load() {
    let mut w = flat_world(1);
    let (_, t) = truck(&mut w, 0, 500, 500, [10, 0, 10]);
    let tank = spawn(&mut w, 0, TANK, 480, 500);
    let tv = members(&w, tank)[0];
    steps(&mut w, 2);
    let bad: Vec<(&str, Box<dyn Fn(&mut World)>)> = vec![
        (
            "cargo above the cap",
            Box::new(move |w| {
                w.edit_vehicle(t, |v| v.cargo_g[0] = u32::MAX);
            }),
        ),
        (
            "cargo on a tank",
            Box::new(move |w| {
                w.edit_vehicle(tv, |v| v.cargo_g[0] = 1);
            }),
        ),
        (
            "drop on a tank",
            Box::new(move |w| {
                w.edit_vehicle(tv, |v| {
                    v.drop = Some(crate::vehicle::CrateDrop {
                        point: FxVec2::from_int(1, 1),
                        count: 1,
                        kind: SupplyKind::Shells,
                        next_tick: 0,
                    })
                });
            }),
        ),
        (
            "drop count 0",
            Box::new(move |w| {
                w.edit_vehicle(t, |v| {
                    v.drop = Some(crate::vehicle::CrateDrop {
                        point: FxVec2::from_int(1, 1),
                        count: 0,
                        kind: SupplyKind::Shells,
                        next_tick: 0,
                    })
                });
            }),
        ),
        (
            "drop point off the map",
            Box::new(move |w| {
                w.edit_vehicle(t, |v| {
                    v.drop = Some(crate::vehicle::CrateDrop {
                        point: FxVec2::from_int(-1, 1),
                        count: 1,
                        kind: SupplyKind::Shells,
                        next_tick: 0,
                    })
                });
            }),
        ),
    ];
    for (name, edit) in bad {
        let mut e = World::load(&w.save(), 1).unwrap();
        edit(&mut e);
        assert!(World::load(&e.save(), 1).is_err(), "{name} loaded");
    }
}
