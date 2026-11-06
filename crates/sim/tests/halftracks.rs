//! S8a Build A (`docs/slices/S8a-halftracks.md` 3, 4, 10): half-tracks
//! without a main gun, pintle MGs and shields, small arms against thin
//! armour (penetration tables, the load-time reach table, vehicle targets),
//! the thin-plate fragment rule, the transport call-in rules, and the new
//! state across save, load and threads.

use std::path::Path;

use frontline_content::vehicle::{MountKind, VehicleRole};
use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::armour::{self, HitVia, deg};
use frontline_sim::game::CallInRefusal;
use frontline_sim::gunnery::pintle_turn;
use frontline_sim::{EntityKind, Fx, FxVec2, FxVec3, PlayerId, SimId, SquadId, World};

fn setup_with(seed: u64, test_unlock: bool) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let mut defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let mut req = MatchRequest::skirmish(seed, "ww2:germany", "ww2:usa", Difficulty::Normal);
    if test_unlock {
        req.rules = defs
            .add_rules_override(&root.join("tests/rules_test_unlock.toml"))
            .unwrap_or_else(|e| panic!("{e}"));
    }
    let mut s = defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
    s.players.iter_mut().for_each(|p| p.is_ai = false);
    s.rules.start_manpower = 100_000;
    s.map.terrain.octaves.clear();
    s.map.terrain.bands.clear();
    s.map.woods.clear();
    // Keep the crews in place (the tests count their hits).
    s.rules.vehicles.bail.crew_below = 0;
    s
}

fn setup(seed: u64) -> MatchSetup {
    setup_with(seed, true)
}

fn xy(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

const EAST: FxVec2 = FxVec2::new(Fx::ONE, Fx::ZERO);
const WEST: FxVec2 = FxVec2::new(Fx::NEG_ONE, Fx::ZERO);
const NORTH: FxVec2 = FxVec2::new(Fx::ZERO, Fx::ONE);

fn spawn(w: &mut World, player: u8, card: &str, at: FxVec2, f: FxVec2) -> (SquadId, SimId) {
    let t = w.match_setup().unwrap().squad_type_index(card).unwrap();
    let sq = w.spawn_squad_at(PlayerId(player), t, at, f).unwrap();
    (sq, w.game().unwrap().squad(sq).unwrap().members[0])
}

fn weapon(w: &World, id: &str) -> u16 {
    w.match_setup()
        .unwrap()
        .weapons
        .iter()
        .position(|x| x.id == id)
        .unwrap_or_else(|| panic!("no {id}")) as u16
}

fn vtype(w: &World, id: &str) -> usize {
    w.match_setup()
        .unwrap()
        .vehicles
        .iter()
        .position(|x| x.id == id)
        .unwrap()
}

fn pos3(w: &World, id: SimId) -> FxVec3 {
    w.entities().transform()[id.index()].pos
}

/// Empties a vehicle's MG stowage (it holds fire).
fn silence(w: &mut World, v: SimId) {
    let n: Vec<u32> = {
        let g = w.game().unwrap();
        let vs = g.vehicle(v).unwrap();
        let vt = &g.setup().vehicles[vs.type_idx as usize];
        vt.mg_mounts()
            .iter()
            .map(|m| m.rounds.unwrap_or(0))
            .collect()
    };
    assert!(w.edit_vehicle(v, |vs| {
        for (st, n) in vs.mgs.iter_mut().zip(n) {
            st.spent = n;
        }
        vs.ammo.iter_mut().for_each(|a| *a = 0);
        vs.loaded = None;
    }));
}

fn crew_hp(w: &World, v: SimId) -> Vec<u16> {
    let vs = w.game().unwrap().vehicle(v).unwrap();
    vs.seats.iter().map(|s| s.hp).collect()
}

#[test]
fn half_tracks_have_no_main_gun_and_one_pintle_mg() {
    let w = World::new_match(setup(1), 1).unwrap();
    let s = w.match_setup().unwrap();
    for (id, cap, shields) in [("ww2:ger_sdkfz251_1", 10, 1), ("ww2:usa_m3a1", 11, 0)] {
        let v = &s.vehicles[vtype(&w, id)];
        assert_eq!(v.role, VehicleRole::Transport, "{id}");
        assert!(v.main_gun().is_none(), "{id}");
        assert_eq!(v.mg_mounts().len(), 1, "{id}");
        assert_eq!(v.mg_mounts()[0].kind, MountKind::Pintle, "{id}");
        assert_eq!(v.transport.as_ref().unwrap().slots.len(), cap, "{id}");
        assert_eq!(v.shields.len(), shields, "{id}");
    }
}

/// A half-track never lays a gun or fires one; its pintle MG turns toward
/// men in its arc and fires.
#[test]
fn a_half_track_fires_only_its_mg_and_never_lays_a_gun() {
    for card in ["ww2:ger_sdkfz251_1_squad", "ww2:usa_m3a1_squad"] {
        let mut w = World::new_match(setup(2), 1).unwrap();
        let player = if card.starts_with("ww2:ger") { 0 } else { 1 };
        let (_, v) = spawn(&mut w, player, card, xy(500, 500), EAST);
        // Enemy riflemen 120 m ahead and 20 deg to the left (inside the
        // 251's +-35 deg arc).
        // (SMG men: no penetration table, so they never shoot back at the
        // exposed commander and the test sees his MG.)
        let foe = if player == 0 {
            "ww2:usa_rifle_squad"
        } else {
            "ww2:ger_smg_gruppe"
        };
        spawn(&mut w, 1 - player, foe, xy(613, 541), WEST);
        let (mut yaw_moved, mut fired) = (false, false);
        for _ in 0..30 * 15 {
            w.step(&[]);
            let vs = w.game().unwrap().vehicle(v).unwrap();
            assert_eq!(vs.turret_yaw, Fx::ZERO, "{card} laid a gun");
            assert!(vs.ammo.is_empty() && vs.loaded.is_none(), "{card}");
            assert!(vs.mgs[0].yaw.abs() <= deg(35) || card.contains("m3a1"));
            yaw_moved |= vs.mgs[0].yaw != Fx::ZERO;
            // (An abandoned vehicle's MG state is reset.)
            fired |= vs.mgs[0].shots > 0;
            assert!(
                w.snapshot()
                    .armour_events
                    .iter()
                    .all(|e| e.kind != frontline_sim::ArmourEventKind::Fired as u8),
                "{card} fired a main gun"
            );
        }
        assert!(fired, "{card}: the MG never fired");
        assert!(yaw_moved, "{card}: the pintle never turned");
    }
}

#[test]
fn pintle_turns_at_its_rate_within_its_arc() {
    let w = World::new_match(setup(1), 1).unwrap();
    let s = w.match_setup().unwrap();
    let hz = s.tick_hz;
    // The 251's MG: +-35 deg at 60 deg/s (2 deg a tick at 30 Hz).
    let m = &s.vehicles[vtype(&w, "ww2:ger_sdkfz251_1")].mg_mounts()[0];
    let (y1, laid) = pintle_turn(m, Fx::ZERO, deg(90), hz);
    assert!(!laid);
    assert!((y1 - deg(2)).abs() <= Fx::from_milli(2), "{y1:?}");
    let mut y = Fx::ZERO;
    for _ in 0..40 {
        y = pintle_turn(m, y, deg(90), hz).0;
    }
    assert_eq!(y, deg(35), "stops at the arc");
    let (y, laid) = pintle_turn(m, deg(34), deg(35), hz);
    assert!(laid && y == deg(35));
    // The M3A1's ring: the short way across the back (1 deg a tick).
    let r = &s.vehicles[vtype(&w, "ww2:usa_m3a1")].mg_mounts()[0];
    let (y, _) = pintle_turn(r, deg(-170), deg(170), hz);
    assert!(y < deg(-170) || y > deg(170), "went the long way: {y:?}");
    let mut y = deg(-170);
    let mut ticks = 0;
    while !pintle_turn(r, y, deg(170), hz).1 {
        y = pintle_turn(r, y, deg(170), hz).0;
        ticks += 1;
        assert!(ticks < 40);
    }
    assert!((19..=21).contains(&ticks), "{ticks} ticks for 20 deg");
}

/// The load-time reach table (vehicles.md 6.4): MG 42 against the M3A1's
/// face-hardened side to about 275 m; the .50 against the 251's side to
/// its full range; rifles beat no plate; nothing beats a Sherman.
#[test]
fn the_reach_table_follows_the_plates() {
    let w = World::new_match(setup(1), 1).unwrap();
    let t = w.game().unwrap().armour();
    let m3 = vtype(&w, "ww2:usa_m3a1") as u16;
    let ht = vtype(&w, "ww2:ger_sdkfz251_1") as u16;
    let sherman = vtype(&w, "ww2:usa_m4a3_75") as u16;
    let side = 1u8; // hull_side
    let front = 0u8;
    let mg42 = t.small_reach(weapon(&w, "ww2:mg42_bipod"), m3, side);
    assert!(
        mg42 > Fx::from_int(250) && mg42 < Fx::from_int(300),
        "{mg42:?}"
    );
    let m2 = t.small_reach(weapon(&w, "ww2:m2hb"), ht, side);
    assert_eq!(m2, Fx::from_int(600), "the .50 to its range");
    for rifle in ["ww2:kar98k", "ww2:m1_garand", "ww2:bar_m1918a2"] {
        for vt in [m3, ht] {
            for z in 0..12u8 {
                assert_eq!(t.small_reach(weapon(&w, rifle), vt, z), Fx::ZERO, "{rifle}");
            }
        }
    }
    assert_eq!(
        t.small_reach(weapon(&w, "ww2:mg42"), m3, front),
        Fx::from_int(0).max(t.small_reach(weapon(&w, "ww2:mg42"), m3, front)),
    );
    for z in 0..8u8 {
        assert_eq!(t.small_reach(weapon(&w, "ww2:m2hb"), sherman, z), Fx::ZERO);
        assert_eq!(t.small_reach(weapon(&w, "ww2:mg42"), sherman, z), Fx::ZERO);
    }
    // No table: an SMG never beats a plate.
    assert_eq!(t.small_reach(weapon(&w, "ww2:mp40"), m3, side), Fx::ZERO);
}

/// `n` rounds of `weapon` from `from` at `aim` (one every 3 ticks); the
/// small-arms penetrations they made.
fn volley(w: &mut World, player: u8, weapon: u16, from: FxVec3, aim: FxVec3, n: u32) -> u64 {
    let before = w.stats().small_arms_pens;
    for _ in 0..n {
        assert!(w.fire_round_now(player, weapon, from, aim));
        for _ in 0..3 {
            w.step(&[]);
        }
    }
    for _ in 0..30 {
        w.step(&[]);
    }
    w.stats().small_arms_pens - before
}

/// vehicles.md 10 (armour-test 21 in short): MG 42 against the M3A1's side
/// at 50 m mostly penetrates, the .50 against the 251's side at 300 m
/// mostly does, a rifle against either front never does.
#[test]
fn small_arms_beat_thin_side_plates_and_not_fronts() {
    let mut w = World::new_match(setup(4), 1).unwrap();
    let (_, m3) = spawn(&mut w, 1, "ww2:usa_m3a1_squad", xy(500, 500), NORTH);
    silence(&mut w, m3);
    w.step(&[]);
    let c = pos3(&w, m3) + FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::from_ratio(75, 100));
    let from = c + FxVec3::new(Fx::from_int(-50), Fx::ZERO, Fx::ZERO);
    let wi = weapon(&w, "ww2:mg42_bipod");
    let pens = volley(&mut w, 0, wi, from, c, 20);
    assert!(pens >= 15, "MG 42 at 50 m: {pens} of 20");

    let mut w = World::new_match(setup(5), 1).unwrap();
    let (_, ht) = spawn(&mut w, 0, "ww2:ger_sdkfz251_1_squad", xy(500, 500), NORTH);
    silence(&mut w, ht);
    w.step(&[]);
    let c = pos3(&w, ht) + FxVec3::new(Fx::ZERO, Fx::from_int(-1), Fx::from_ratio(70, 100));
    let from = c + FxVec3::new(Fx::from_int(300), Fx::ZERO, Fx::from_ratio(30, 100));
    // No drop compensation in the staging tool: aim 0.55 m high at 300 m.
    let c = c + FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::from_ratio(55, 100));
    let wi = weapon(&w, "ww2:m2hb");
    let pens = volley(&mut w, 1, wi, from, c, 20);
    assert!(pens >= 15, ".50 at 300 m: {pens} of 20");
    // A rifle at the 251's nose from 50 m: never.
    let nose = pos3(&w, ht) + FxVec3::new(Fx::ZERO, Fx::from_int(2), Fx::from_ratio(70, 100));
    let from = nose + FxVec3::new(Fx::ZERO, Fx::from_int(50), Fx::ZERO);
    let wi = weapon(&w, "ww2:m1_garand");
    let pens = volley(&mut w, 1, wi, from, nose, 20);
    assert_eq!(pens, 0, "a rifle beat the 251's front");
}

/// A shield in front of the commander (6.7): rifle rounds aimed at it never
/// reach him; MG 42 rounds at 30 m often go through.
#[test]
fn the_mg_shield_stops_rifles_and_not_every_mg_round() {
    let run = |gun: &str| {
        let mut w = World::new_match(setup(6), 1).unwrap();
        let (_, ht) = spawn(&mut w, 0, "ww2:ger_sdkfz251_1_squad", xy(500, 500), EAST);
        silence(&mut w, ht);
        w.step(&[]);
        let f = *w.game().unwrap().vehicle_derived().frame(ht).unwrap();
        assert_eq!(f.n_shields, 1);
        let at = f.shields[0].center;
        let from = at + FxVec3::new(Fx::from_int(30), Fx::ZERO, Fx::ZERO);
        let plates = &w.game().unwrap().armour().vehicles[f.type_idx as usize];
        let h = armour::hit_armour(from, at - from, &f, plates).unwrap();
        assert_eq!(h.via, HitVia::Shield(0), "the shield is struck first");
        let before = crew_hp(&w, ht)[1];
        let wi = weapon(&w, gun);
        volley(&mut w, 1, wi, from, at, 20);
        before - crew_hp(&w, ht)[1]
    };
    assert_eq!(run("ww2:m1_garand"), 0, "a rifle round went through");
    assert!(run("ww2:m1919") > 0, "no MG round went through");
}

/// Soldiers fire at a vehicle they can hurt (an exposed crew, S7-7, or a
/// plate in reach) and never at a closed tank.
#[test]
fn small_arms_pick_vehicles_they_can_hurt() {
    let target_of = |card: &str, foe: &str| {
        let mut w = World::new_match(setup(7), 1).unwrap();
        let (_, v) = spawn(&mut w, 1, card, xy(560, 500), WEST);
        silence(&mut w, v);
        let (sq, _) = spawn(&mut w, 0, foe, xy(500, 500), EAST);
        let mut aimed = false;
        for _ in 0..30 * 5 {
            w.step(&[]);
            let men = w.game().unwrap().squad(sq).unwrap().members.clone();
            aimed |= men
                .iter()
                .any(|m| w.entities().combat()[m.index()].target == Some(v));
        }
        aimed
    };
    assert!(target_of("ww2:usa_m10_squad", "ww2:ger_grenadiergruppe"));
    assert!(target_of("ww2:usa_m3a1_squad", "ww2:ger_grenadiergruppe"));
    assert!(!target_of(
        "ww2:usa_m4a3_75_squad",
        "ww2:ger_grenadiergruppe"
    ));
}

/// The thin-plate rule (S7-8): mortar bursts 1.5 m beside a vehicle pierce
/// the M3A1's 6.35 mm face-hardened side, not the 251's 8 mm sloped side
/// or a Sherman.
#[test]
fn mortar_fragments_pierce_only_the_thinnest_plates() {
    let run = |card: &str| {
        let mut n = 0;
        for k in 0..6i32 {
            let mut w = World::new_match(setup(20 + k as u64), 1).unwrap();
            let (_, v) = spawn(&mut w, 1, card, xy(500, 500), EAST);
            silence(&mut w, v);
            w.step(&[]);
            let side = if k % 2 == 0 { 1 } else { -1 };
            let at = pos3(&w, v).xy()
                + FxVec2::new(Fx::from_ratio(100 * (k - 2) as i64, 100), Fx::ZERO)
                + FxVec2::new(Fx::ZERO, Fx::from_ratio(side * 260, 100));
            let mortar = weapon(&w, "ww2:grw34");
            let before = w.stats().frag_plate_pierces;
            assert!(w.drop_shell_now(0, mortar, at));
            for _ in 0..30 {
                w.step(&[]);
            }
            n += w.stats().frag_plate_pierces - before;
        }
        n
    };
    assert!(
        run("ww2:usa_m3a1_squad") > 0,
        "no fragment pierced the M3A1"
    );
    assert_eq!(run("ww2:ger_sdkfz251_1_squad"), 0, "the 251's side");
    assert_eq!(run("ww2:usa_m4a3_75_squad"), 0, "a Sherman");
}

/// S8a-5: transports skip the vehicle limit, cap at two alive, and unlock
/// at [vehicles.transport] unlock_s (2:00) while tanks wait for 4:00.
#[test]
fn transports_skip_the_vehicle_limit_and_unlock_first() {
    let mut w = World::new_match(setup(8), 1).unwrap();
    for k in 0..4 {
        spawn(
            &mut w,
            0,
            "ww2:ger_pz4_h_squad",
            xy(200 + 20 * k, 200),
            EAST,
        );
    }
    let check = |w: &World, card: &str| {
        let t = w.match_setup().unwrap().squad_type_index(card).unwrap();
        let g = w.game().unwrap();
        g.call_in_check(w.entities(), PlayerId(0), t, w.tick() + 30 * 600)
    };
    assert!(matches!(
        check(&w, "ww2:ger_pz4_h_squad"),
        Err(CallInRefusal::VehicleLimit)
    ));
    assert!(check(&w, "ww2:ger_sdkfz251_1_squad").is_ok());
    spawn(&mut w, 0, "ww2:ger_sdkfz251_1_squad", xy(300, 300), EAST);
    spawn(&mut w, 0, "ww2:ger_sdkfz251_1_squad", xy(320, 300), EAST);
    assert!(matches!(
        check(&w, "ww2:ger_sdkfz251_1_squad"),
        Err(CallInRefusal::MaxAlive { max: 2 })
    ));
    // The real unlocks: 2:00 for transports, 4:00 for tanks.
    let w = World::new_match(setup_with(8, false), 1).unwrap();
    let at = |card: &str, tick: u64| {
        let t = w.match_setup().unwrap().squad_type_index(card).unwrap();
        w.game()
            .unwrap()
            .call_in_check(w.entities(), PlayerId(0), t, tick)
    };
    let hz = w.match_setup().unwrap().tick_hz as u64;
    assert!(matches!(
        at("ww2:ger_sdkfz251_1_squad", 120 * hz - 1),
        Err(CallInRefusal::Locked { .. })
    ));
    assert!(!matches!(
        at("ww2:ger_sdkfz251_1_squad", 120 * hz),
        Err(CallInRefusal::Locked { .. })
    ));
    assert!(matches!(
        at("ww2:ger_pz4_h_squad", 120 * hz),
        Err(CallInRefusal::Locked { .. })
    ));
}

/// `MgState::yaw`: saved, hashed, checked on load (inside the arc); a
/// fight with a turning pintle is the same at 1/2/3/8 threads and across
/// save, load and continue.
#[test]
fn pintle_yaw_is_saved_hashed_checked_and_deterministic() {
    let start = || {
        let mut w = World::new_match(setup(9), 1).unwrap();
        let (_, ht) = spawn(&mut w, 0, "ww2:ger_sdkfz251_1_squad", xy(500, 500), EAST);
        spawn(&mut w, 1, "ww2:usa_rifle_squad", xy(600, 540), WEST);
        spawn(&mut w, 1, "ww2:usa_m3a1_squad", xy(640, 470), WEST);
        (w, ht)
    };
    let (mut w, ht) = start();
    for _ in 0..30 * 4 {
        w.step(&[]);
    }
    let yaw = w.game().unwrap().vehicle(ht).unwrap().mgs[0].yaw;
    assert_ne!(yaw, Fx::ZERO, "the pintle turned");
    let base = w.full_state_hash();
    let loaded = World::load(&w.save(), 2).unwrap();
    assert_eq!(loaded.full_state_hash(), base);
    assert_eq!(loaded.game().unwrap().vehicle(ht).unwrap().mgs[0].yaw, yaw);
    let mut e = World::load(&w.save(), 1).unwrap();
    assert!(e.edit_vehicle(ht, |v| v.mgs[0].yaw += Fx::EPSILON));
    assert_ne!(e.full_state_hash(), base, "yaw is not hashed");
    let mut b = World::load(&w.save(), 1).unwrap();
    assert!(b.edit_vehicle(ht, |v| v.mgs[0].yaw = deg(40)));
    assert!(World::load(&b.save(), 1).is_err(), "a yaw outside the arc");
    let run = |threads: usize, split: Option<u64>| {
        let (w0, _) = start();
        let mut w = World::load(&w0.save(), threads).unwrap();
        let mut hashes = Vec::new();
        for t in 0..30 * 10u64 {
            if Some(t) == split {
                w = World::load(&w.save(), threads).unwrap();
            }
            w.step(&[]);
            if t % 15 == 0 {
                hashes.push(w.state_hash());
            }
        }
        (hashes, w.full_state_hash(), w.stats().small_arms_pens)
    };
    let base = run(1, None);
    for threads in [2, 3, 8] {
        assert_eq!(run(threads, None), base, "{threads} threads");
    }
    assert_eq!(run(3, Some(30 * 4)), base, "save/load/continue");
    let _ = EntityKind::Vehicle;
}
