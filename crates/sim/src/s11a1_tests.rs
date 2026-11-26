//! S11a1 tests (docs/slices/S11a1.md 3, 4; Build A): the SAVE 27 layout
//! and its load checks, the widened kill class, and the sim rules G1-G7.
//! Each test names its control: the edit that makes it fail.

use super::s6_tests::{flat_world_with, members, spawn, steps};
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
                ..AiArty::default()
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
    // Fix C-5: a new observer record (no point yet) loads; 99 below does
    // not.
    let mut x = World::load(&bytes, 1).unwrap();
    ai(&mut x.state).fo.get_mut(&b).unwrap().point = crate::ai::NO_POINT;
    World::load(&x.save(), 1).expect("a new observer record loads");
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
        (
            "fo_lost_at in the future",
            Box::new(move |s| ai(s).fo_lost_at = tick + 1),
        ),
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

/// Spec 3 "Vehicle type cap": with 18 vehicle types (ww2's 17 and a clone) the last type's rounds
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
        // Build B: 15 base defs + the leIG 18 and the M3; S11b Build B:
        // + the leFH 18, the M2A1 and the pack howitzer; S13a Build B: + the
        // Panther, Tiger, 76 mm Sherman, M36 and Jumbo; one clone more.
        // S15 Build C: + 12 Soviet defs (five guns, the US6, six vehicles);
        // S15 Fix: + the US6 gun tractor.
        assert_eq!(s.vehicles.len(), 38, "ww2 has 38 vehicle defs with S15");
        let v = s.vehicles[pz].clone();
        s.vehicles.push(v);
        let t = s
            .squad_types
            .iter_mut()
            .find(|t| t.id == "ww2:ger_pz4_h_squad")
            .unwrap();
        t.vehicle = Some(38);
    });
    let sq = spawn(&mut w, 0, "ww2:ger_pz4_h_squad", 300, 300);
    steps(&mut w, 2);
    let g = w.game().unwrap();
    let v = g.squads[&sq].members[0];
    assert_eq!(g.vehicles()[&v].type_idx, 38);
    assert_eq!(g.players[1].s7.kills_by_vehicle.len(), 39);
    let cannon = g
        .setup()
        .weapons
        .iter()
        .position(|x| x.class == frontline_content::WeaponClass::Cannon)
        .unwrap() as u16;
    let class = g.round_class(&w.state.entities, cannon, u16::MAX, v);
    assert_eq!(kill_class::base(class), kill_class::TANK_AP);
    assert_eq!(kill_class::vehicle(class), Some(38));
    // Every type up to the cap keeps both parts.
    for t in [0u16, 14, 15, 16, 4094] {
        let k = kill_class::with_vehicle(kill_class::FRAG, t);
        assert_eq!(
            (kill_class::base(k), kill_class::vehicle(k)),
            (kill_class::FRAG, Some(t as usize))
        );
    }
    // Control: the old pack.
    let old = |c: u8, t: u8| (c & 0x0F) | (t.wrapping_add(1) << 4);
    let o = old(kill_class::TANK_AP as u8, 16);
    assert_ne!(o >> 4, 17, "the old u8 pack keeps type 16");
}

/// Adds a test-only infantry gun to a setup: the Pak 38 vehicle and squad
/// cloned as `test:inf_gun` with role `role` (Build B brings the real
/// leIG 18 and M3). Returns nothing; spawn it by the id.
pub(super) fn add_test_gun(
    s: &mut frontline_content::MatchSetup,
    role: frontline_content::VehicleRole,
) {
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
        (
            ai.guns.contains_key(&q),
            g.squads[&q].order.kind() == order0,
        )
    };
    assert_eq!(
        run(VehicleRole::InfGun),
        (false, true),
        "an inf_gun is left alone"
    );
    assert!(
        run(VehicleRole::AtGun).0,
        "control: an at_gun clone gets a job"
    );
}

// ------------------------------------------------------- gun missions --

use crate::command::{Command, CommandKind};
use crate::fire_mission::{FireMissionRefusal, mode, phase};
use crate::vehicle::GunState;
use frontline_content::{SeatRole, VehicleRole};

/// The test-only indirect gun: [`add_test_gun`] (role `inf_gun`) whose
/// main weapon gets an `[indirect]` table (the leIG 18's charges, band
/// `lo..=hi` deg, `low_angle`, 3550 m, 8 mrad), minimum range `min_m`, and
/// a mount elevation of -10..`hi` (Build B brings the real leIG 18).
pub(super) fn add_indirect_gun(
    s: &mut frontline_content::MatchSetup,
    lo: u32,
    hi: u32,
    min_m: i32,
) {
    use frontline_content::Fixed;
    use frontline_content::defs::IndirectDef;
    use frontline_content::setup::IndirectSetup;
    add_test_gun(s, VehicleRole::InfGun);
    let vi = s.vehicles.len() - 1;
    let wi = s.vehicles[vi].mounts[0].weapon as usize;
    let mut w = s.weapons[wi].clone();
    w.id = "test:inf_gun_main".into();
    w.indirect = Some(IndirectSetup::lower(&IndirectDef {
        charges_m_s: vec![92, 120, 150, 180, 210],
        elevation_deg: [lo, hi],
        low_angle: true,
        range_m: Some(3550),
        spread_mrad: Some(8),
    }));
    w.range_min = Fixed::from_int(min_m);
    w.range_max = Fixed::from_int(3550);
    s.weapons.push(w);
    let nw = (s.weapons.len() - 1) as u16;
    let m = &mut s.vehicles[vi].mounts[0];
    m.weapon = nw;
    m.elevation_deg = [-10, hi as i32];
    let t = s.squad_types.last_mut().unwrap();
    t.weapon = nw;
    t.small_arms = nw;
}

/// A gun mission scene on flat ground: player 0's test gun at (300, 500)
/// facing +x and a Grenadier squad at (330, 500) whose leader observes
/// (by voice: within `observer_link_m` of the tube). `edit` changes the
/// setup first.
pub(super) struct Scene {
    pub(super) w: World,
    pub(super) sq: SquadId,
    pub(super) gun: SimId,
}

pub(super) fn scene_with(
    threads: usize,
    lo: u32,
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> Scene {
    let mut w = flat_world_with(threads, |s| {
        add_indirect_gun(s, lo, 73, 0);
        edit(s);
    });
    steps(&mut w, 2);
    let sq = spawn(&mut w, 0, "test:inf_gun", 300, 500);
    spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 330, 500);
    steps(&mut w, 30);
    let gun = w.game().unwrap().squads[&sq].members[0];
    Scene { w, sq, gun }
}

pub(super) fn scene() -> Scene {
    scene_with(1, 0, |_| {})
}

pub(super) const TARGET: (i32, i32) = (650, 500);

/// Orders the mission; the refusal of this tick, if any.
pub(super) fn order(
    w: &mut World,
    sq: SquadId,
    at: FxVec2,
    rounds: u16,
) -> Option<FireMissionRefusal> {
    w.step(&[Command {
        player: PlayerId(0),
        seq: 1,
        kind: CommandKind::FireMission {
            squad: sq,
            target: at,
            rounds,
        },
    }]);
    w.snapshot()
        .fire_mission_refusals
        .iter()
        .find(|e| e.squad == sq)
        .map(|e| e.refusal)
}

pub(super) fn mission(w: &World, sq: SquadId) -> Option<crate::fire_mission::Mission> {
    match &w.game().unwrap().squads[&sq].order {
        Order::FireMission { m, .. } => Some(*m),
        _ => None,
    }
}

/// The gun's HE rounds, the one in the breech included.
pub(super) fn he_rounds(w: &World, gun: SimId) -> u16 {
    let vs = &w.game().unwrap().vehicles[&gun];
    vs.ammo[1] + (vs.loaded == Some(1)) as u16
}

fn s11(w: &World) -> crate::fire_mission::S11Counters {
    w.game().unwrap().players[0].s11
}

/// Steps until the squad's mission ends (at most `s` seconds).
pub(super) fn run_out(w: &mut World, sq: SquadId, s: u64) {
    let end = w.tick() + s * 30;
    while mission(w, sq).is_some() {
        assert!(w.tick() < end, "the mission did not end within {s} s");
        w.step(&[]);
    }
}

/// G1: an emplaced, manned gun with an `[indirect]` main weapon takes a
/// fire mission (Direct: the crew's leader sees the point, by voice).
/// Control: `gun_missions` 0 refuses it `NotMortar`.
#[test]
fn gun_takes_a_fire_mission() {
    let mut s = scene();
    assert_eq!(order(&mut s.w, s.sq, c(TARGET.0, TARGET.1), 4), None);
    let m = mission(&s.w, s.sq).expect("the gun is on the mission");
    assert_eq!((m.mode, m.phase), (mode::DIRECT, phase::LAYING));
    assert_eq!(s11(&s.w).gun_missions, [1, 0, 0]);
    let g = s.w.game().unwrap();
    assert_eq!(
        g.players[0].fm.missions,
        [0, 0, 0],
        "a gun counts in s11 only"
    );
    // Control.
    let mut s = scene_with(1, 0, |s| {
        s.rules.fire_mission.as_mut().unwrap().gun_missions = 0
    });
    assert_eq!(
        order(&mut s.w, s.sq, c(TARGET.0, TARGET.1), 4),
        Some(FireMissionRefusal::NotMortar)
    );
    assert!(mission(&s.w, s.sq).is_none());
}

/// G1 refusals in the spec's order, one scene each. Control: the same gun
/// emplaced and manned accepts (last). `Masked` is the solver's
/// (`indirect.rs` `flat_gun_is_masked_behind_a_crest`).
#[test]
fn gun_mission_refusals_in_order() {
    let mut s = scene();
    let (w, sq, gun) = (&mut s.w, s.sq, s.gun);
    let t = c(TARGET.0, TARGET.1);
    let size = w.match_setup().unwrap().map.size_m;
    assert_eq!(
        order(w, sq, c(-5, 500), 4),
        Some(FireMissionRefusal::OffMap)
    );
    assert_eq!(order(w, sq, t, 0), Some(FireMissionRefusal::Rounds));
    w.edit_vehicle(gun, |vs| {
        vs.served.as_mut().unwrap().state = GunState::Limbered
    });
    assert_eq!(order(w, sq, t, 4), Some(FireMissionRefusal::NotReady));
    w.edit_vehicle(gun, |vs| {
        vs.served.as_mut().unwrap().state = GunState::Emplaced
    });
    let (ammo, loaded) = {
        let vs = &w.game().unwrap().vehicles[&gun];
        (vs.ammo.clone(), vs.loaded)
    };
    w.edit_vehicle(gun, |vs| {
        vs.ammo[1] = 0;
        if vs.loaded == Some(1) {
            vs.loaded = None;
        }
    });
    assert_eq!(order(w, sq, t, 4), Some(FireMissionRefusal::NoBombs));
    w.edit_vehicle(gun, |vs| {
        vs.ammo = ammo.clone();
        vs.loaded = loaded;
    });
    assert_eq!(
        order(w, sq, c(300, size - 30), 4),
        Some(FireMissionRefusal::UnknownGround)
    );
    // A high-angle-only gun (60..73 deg) cannot fire 40 m; with a minimum
    // range it is inside it.
    for (min, want) in [
        (0, FireMissionRefusal::CannotReach),
        (100, FireMissionRefusal::InsideMinimum),
    ] {
        let mut h = flat_world_with(1, |s| add_indirect_gun(s, 60, 73, min));
        steps(&mut h, 2);
        let q = spawn(&mut h, 0, "test:inf_gun", 300, 500);
        spawn(&mut h, 0, "ww2:ger_grenadiergruppe", 330, 500);
        steps(&mut h, 30);
        assert_eq!(order(&mut h, q, c(340, 500), 4), Some(want));
    }
    // Control.
    assert_eq!(order(w, sq, t, 4), None);
}

/// G3: rounds leave `vs.ammo`, the bursts carry the mission id, the rounds
/// count in `S11Counters` only. Control: no HE left refuses `NoBombs`.
#[test]
fn gun_mission_fires_counted_he() {
    let mut s = scene();
    s.w.enable_burst_log();
    let he0 = he_rounds(&s.w, s.gun);
    assert_eq!(order(&mut s.w, s.sq, c(TARGET.0, TARGET.1), 4), None);
    let id = mission(&s.w, s.sq).unwrap().id;
    run_out(&mut s.w, s.sq, 180);
    steps(&mut s.w, 30 * 10);
    assert_eq!(he_rounds(&s.w, s.gun), he0 - 4);
    let k = s11(&s.w);
    assert_eq!(k.gun_rounds.iter().sum::<u32>(), 4);
    assert_eq!(k.low_rounds + k.high_rounds, 4);
    let g = s.w.game().unwrap();
    assert_eq!(g.players[0].fm.bombs, [0, 0, 0], "not in FmCounters");
    let log = s.w.take_burst_log();
    assert_eq!(log.iter().filter(|b| b.mission == id).count(), 4, "{log:?}");
    // Control.
    let mut s = scene();
    s.w.edit_vehicle(s.gun, |vs| {
        vs.ammo[1] = 0;
        if vs.loaded == Some(1) {
            vs.loaded = None;
        }
    });
    assert_eq!(
        order(&mut s.w, s.sq, c(TARGET.0, TARGET.1), 4),
        Some(FireMissionRefusal::NoBombs)
    );
}

/// G3: with the loader gone the gun fires nothing until the S9 slot swap
/// fills his seat. Control: the full crew fires at the ready time
/// (`gun_mission_fires_counted_he`), and after the swap this gun fires.
#[test]
fn gun_mission_waits_for_the_crew() {
    let mut s = scene();
    assert_eq!(order(&mut s.w, s.sq, c(TARGET.0, TARGET.1), 4), None);
    let ready = mission(&s.w, s.sq).unwrap().ready_at;
    while s.w.tick() + 1 < ready {
        s.w.step(&[]);
    }
    let vt = s.w.game().unwrap().vehicles[&s.gun].type_idx as usize;
    let v = s.w.match_setup().unwrap().vehicles[vt].clone();
    let k = v
        .seats
        .iter()
        .position(|st| st.role == SeatRole::Loader)
        .unwrap();
    let slots = s.w.game().unwrap().vehicles[&s.gun]
        .served
        .clone()
        .unwrap()
        .slots;
    let loader = slots[k].man.unwrap();
    s.w.edit_unit(loader, |u| u.serving = None);
    s.w.step(&[]);
    let manned = |w: &World| w.game().unwrap().vehicles[&s.gun].manned(&v, SeatRole::Loader);
    assert!(!manned(&s.w));
    let mut waited = 0;
    while !manned(&s.w) {
        assert_eq!(
            s11(&s.w).gun_rounds.iter().sum::<u32>(),
            0,
            "fired without a loader"
        );
        s.w.step(&[]);
        waited += 1;
        assert!(waited < 30 * 60, "no slot swap");
    }
    assert!(waited > 30, "the swap took time");
    steps(&mut s.w, 30 * 20);
    assert!(
        s11(&s.w).gun_rounds.iter().sum::<u32>() > 0,
        "fires after the swap"
    );
}

/// Player 1's riflemen at `(x, y)`, held where they stand.
fn enemy(w: &mut World, x: i32, y: i32) -> SquadId {
    spawn(w, 1, "ww2:usa_rifle_squad", x, y)
}

/// An enemy tank for direct-fire scenes (seen at 300 m; riflemen at that
/// range are not).
const TANK: &str = "ww2:usa_m4a3_75_squad";

/// G3: a gun on a mission picks no main-gun target: a seen enemy tank
/// 300 m off draws no direct round. Control: Idle, it fires at them.
#[test]
fn gun_on_mission_fires_no_direct_round() {
    let run = |mission: bool| {
        let mut s = scene();
        spawn(&mut s.w, 1, TANK, 600, 540);
        if mission {
            assert_eq!(order(&mut s.w, s.sq, c(650, 420), 50), None);
        }
        steps(&mut s.w, 30 * 10);
        s11(&s.w).gun_direct_rounds
    };
    assert_eq!(run(true), 0);
    assert!(run(false) > 0, "control: the idle gun fires");
}

/// G3 counter split (review 6): an `inf_gun` fires direct rounds into
/// `gun_direct_rounds`, not `s9.gun_rounds`. Control: the same clone as an
/// `at_gun` counts in `s9`.
#[test]
fn infantry_gun_counts_only_in_s11() {
    let run = |role: VehicleRole| {
        let mut w = flat_world_with(1, |s| add_test_gun(s, role));
        steps(&mut w, 2);
        spawn(&mut w, 0, "test:inf_gun", 300, 500);
        spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 330, 500);
        spawn(&mut w, 1, TANK, 600, 540);
        steps(&mut w, 30 * 10);
        let p = &w.game().unwrap().players[0];
        (p.s9.gun_rounds, p.s11.gun_direct_rounds)
    };
    let (s9, s11) = run(VehicleRole::InfGun);
    assert!(s9 == 0 && s11 > 0, "inf_gun: s9 {s9}, s11 {s11}");
    let (s9, s11) = run(VehicleRole::AtGun);
    assert!(s9 > 0 && s11 == 0, "control at_gun: s9 {s9}, s11 {s11}");
}

/// G3 save and threads: saves in Laying, in Adjust with a round in the
/// air, and in Effect all load and go on as the unbroken run; 1 and 4
/// threads agree. Control: the hash moves with the mission (a mission
/// field edit changes it, `s5b_fire_mission_state_is_saved_hashed_and_checked`).
#[test]
fn gun_mission_deterministic_threads_and_save_load() {
    let go = |threads: usize| {
        let mut s = scene_with(threads, 0, |_| {});
        assert_eq!(order(&mut s.w, s.sq, c(TARGET.0, TARGET.1), 6), None);
        let mut saves = Vec::new();
        let mut seen = [false; 3];
        for _ in 0..30 * 120 {
            let k = match mission(&s.w, s.sq) {
                Some(m) if m.phase == phase::LAYING => Some(0),
                Some(m) if m.phase == phase::ADJUST && m.wait_burst => Some(1),
                Some(m) if m.phase == phase::EFFECT => Some(2),
                _ => None,
            };
            if let Some(k) = k
                && !seen[k]
            {
                seen[k] = true;
                saves.push((s.w.tick(), s.w.save()));
            }
            s.w.step(&[]);
        }
        assert_eq!(seen, [true; 3], "every phase was saved");
        (s.w.full_state_hash(), s.w.tick(), saves)
    };
    let (h1, end, saves) = go(1);
    let (h4, _, _) = go(4);
    assert_eq!(h1, h4, "1 and 4 threads");
    for (t, bytes) in saves {
        let mut w = World::load(&bytes, 1).unwrap_or_else(|e| panic!("save at {t}: {e:?}"));
        while w.tick() < end {
            w.step(&[]);
        }
        assert_eq!(
            w.full_state_hash(),
            h1,
            "the save at tick {t} goes on as the run"
        );
    }
}

/// G3 memory (review 3): a direct gun mission with seen rounds leaves
/// its offset in the squad's ranging memory, from the gun's position.
/// Control: a map-fire mission (observers see only 50 m; the point known
/// from a squad by the target, not linked) leaves none.
#[test]
fn gun_mission_leaves_ranging_memory() {
    let mut s = scene();
    assert_eq!(order(&mut s.w, s.sq, c(TARGET.0, TARGET.1), 4), None);
    run_out(&mut s.w, s.sq, 180);
    let r = &s.w.game().unwrap().squads[&s.sq].ranging;
    assert_eq!(r.len(), 1, "{r:?}");
    assert_eq!(r[0].from, c(300, 500));
    assert!(r[0].seen > 0);
    // Control.
    let mut s = scene_with(1, 0, |s| {
        s.rules.fire_mission.as_mut().unwrap().observe_leader_m = 50;
    });
    spawn(
        &mut s.w,
        0,
        "ww2:ger_grenadiergruppe",
        TARGET.0 - 10,
        TARGET.1,
    );
    steps(&mut s.w, 30);
    assert_eq!(order(&mut s.w, s.sq, c(TARGET.0, TARGET.1), 2), None);
    assert_eq!(mission(&s.w, s.sq).unwrap().mode, mode::MAP);
    run_out(&mut s.w, s.sq, 180);
    assert!(s.w.game().unwrap().squads[&s.sq].ranging.is_empty());
}

/// G4: a gun's map-fire error is twice the observed one at 600 m (36 m ->
/// the 50 m floor against the 25 m floor). Control: a mortar at the same
/// range keeps the mortar keys (48 m / 18 m).
#[test]
fn gun_map_error_is_twice_the_observed() {
    use crate::fire_mission::start_error_for;
    let w = flat_world_with(1, |_| {});
    let fm = w.match_setup().unwrap().rules.fire_mission.clone().unwrap();
    let d = Fx::from_int(600);
    assert_eq!(start_error_for(&fm, false, d, true), Fx::from_int(25));
    assert_eq!(start_error_for(&fm, true, d, true), Fx::from_int(50));
    let mortar = (
        start_error_for(&fm, true, d, false),
        start_error_for(&fm, false, d, false),
    );
    assert_eq!(mortar.0.round_int(), 48);
    assert_eq!(mortar.1.round_int(), 18);
}

/// G6: packing the gun ends its mission (order Idle). Control: left
/// alone it is still on it.
#[test]
fn gun_mission_ends_when_packed() {
    for pack in [true, false] {
        let mut s = scene();
        assert_eq!(order(&mut s.w, s.sq, c(TARGET.0, TARGET.1), 50), None);
        steps(&mut s.w, 30 * 5);
        if pack {
            s.w.step(&[Command {
                player: PlayerId(0),
                seq: 2,
                kind: CommandKind::PackGun { guns: vec![s.gun] },
            }]);
        }
        steps(&mut s.w, 5);
        assert_eq!(mission(&s.w, s.sq).is_none(), pack, "packed: {pack}");
        if pack {
            assert_eq!(s.w.game().unwrap().squads[&s.sq].order, Order::Idle);
        }
    }
}

/// G6: a seen enemy within `gun_break_m` (150 m) ends the mission and
/// counts a break. Control: at 200 m it goes on.
#[test]
fn gun_mission_breaks_for_self_defence() {
    for (dx, breaks) in [(100, true), (200, false)] {
        let mut s = scene();
        assert_eq!(order(&mut s.w, s.sq, c(650, 420), 50), None);
        steps(&mut s.w, 30);
        enemy(&mut s.w, 300 + dx, 560);
        steps(&mut s.w, 30 * 3);
        assert_eq!(mission(&s.w, s.sq).is_none(), breaks, "enemy at {dx} m");
        assert_eq!(s11(&s.w).gun_breaks, breaks as u32);
    }
}

/// A test observer team type (`test:fo`): the Grenadier squad with the
/// leader an `observer` and the next slot a `radio` man; and the gun
/// crews' first slot a `radio` man.
fn add_fo(s: &mut frontline_content::MatchSetup) {
    use frontline_content::setup::member_trait;
    let src = s
        .squad_types
        .iter()
        .position(|t| t.id == "ww2:ger_grenadiergruppe")
        .unwrap();
    let mut t = s.squad_types[src].clone();
    t.id = "test:fo".into();
    let lead = t
        .members
        .iter()
        .position(|m| m.traits & member_trait::LEADER != 0)
        .unwrap();
    t.members[lead].traits |= member_trait::OBSERVER;
    let r = (lead + 1) % t.members.len();
    t.members[r].traits |= member_trait::RADIO;
    s.squad_types.push(t);
    for t in s.squad_types.iter_mut().filter(|t| t.id.ends_with("_crew")) {
        t.members[0].traits |= member_trait::RADIO;
    }
}

/// The men of squad `q` whose slot has trait `bit`.
fn with_trait(w: &World, q: SquadId, bit: u8) -> Vec<SimId> {
    let g = w.game().unwrap();
    let ty = &w.match_setup().unwrap().squad_types[g.squads[&q].type_idx as usize];
    members(w, q)
        .into_iter()
        .filter(|m| ty.members[w.entities().kit[m.index()].slot as usize].traits & bit != 0)
        .collect()
}

/// G5: an observer 600 m from the gun who sees the point is linked by
/// radio: Observed. Controls: `radio_link` 0, the FO's radio man dead, the
/// gun crew's radio man dead: Map.
#[test]
fn radio_links_an_observer_across_the_map() {
    use frontline_content::setup::member_trait;
    let target = c(1000, 520);
    let run = |radio: bool, kill: u8| {
        let mut w = flat_world_with(1, |s| {
            add_indirect_gun(s, 0, 73, 0);
            add_fo(s);
            s.rules.fire_mission.as_mut().unwrap().radio_link = radio as u32;
        });
        steps(&mut w, 2);
        let sq = spawn(&mut w, 0, "test:inf_gun", 300, 500);
        let fo = spawn(&mut w, 0, "test:fo", 900, 500);
        steps(&mut w, 30);
        let gun = w.game().unwrap().squads[&sq].members[0];
        let crew = w.game().unwrap().vehicles[&gun]
            .served
            .as_ref()
            .unwrap()
            .crew
            .unwrap();
        let dead = match kill {
            1 => with_trait(&w, fo, member_trait::RADIO),
            2 => with_trait(&w, crew, member_trait::RADIO),
            _ => Vec::new(),
        };
        if kill != 0 {
            assert!(!dead.is_empty());
        }
        for m in dead {
            assert!(w.kill_soldier(m));
        }
        steps(&mut w, 2);
        let g = w.game().unwrap();
        let gp = w.entities().transform[gun.index()].pos.xy();
        g.mode_for(w.entities(), w.terrain(), sq, gp, target, w.tick())
            .0
    };
    assert_eq!(run(true, 0), mode::OBSERVED);
    assert_eq!(run(false, 0), mode::MAP, "control: radio_link 0");
    assert_eq!(run(true, 1), mode::MAP, "control: the FO's radio man dead");
    assert_eq!(run(true, 2), mode::MAP, "control: the gun's radio man dead");
}

/// G5: the radio reveals nothing: an enemy 700 m from the FO (inside his
/// observe range, beyond any sight) stays unseen. Control: one 100 m from
/// him is seen.
#[test]
fn radio_does_not_reveal() {
    let mut w = flat_world_with(1, |s| {
        add_indirect_gun(s, 0, 73, 0);
        add_fo(s);
    });
    steps(&mut w, 2);
    spawn(&mut w, 0, "test:inf_gun", 300, 500);
    spawn(&mut w, 0, "test:fo", 900, 500);
    let far = enemy(&mut w, 900, 1200);
    let near = enemy(&mut w, 1000, 500);
    steps(&mut w, 30 * 5);
    let seen = |q: SquadId| {
        members(&w, q)
            .iter()
            .any(|m| w.entities().combat[m.index()].seen_by & 1 != 0)
    };
    assert!(!seen(far), "the radio revealed an enemy");
    assert!(seen(near), "control: the near enemy is seen");
}

/// G7: with `arty_base_rearm` 0 an infantry gun in its base gets no
/// rounds back. Control 1: rounds return and their price (paid from the
/// shells stock) counts in `gun_shell_micro`.
#[test]
fn infantry_gun_does_not_rearm_in_base_when_switched_off() {
    let run = |on: u32| {
        let mut w = flat_world_with(1, |s| {
            add_test_gun(s, VehicleRole::InfGun);
            s.rules.supply.as_mut().unwrap().arty_base_rearm = on;
        });
        steps(&mut w, 2);
        let s = w.match_setup().unwrap();
        let base = s.map.bases[s.players[0].base as usize].center;
        let at = FxVec2::from(base);
        let (x, y) = (at.x.round_int(), at.y.round_int());
        let q = spawn(&mut w, 0, "test:inf_gun", x, y);
        let gun = members(&w, q)[0];
        steps(&mut w, 2);
        w.edit_vehicle(gun, |vs| vs.ammo[1] = 2);
        let mp0 = w.game().unwrap().players[0].manpower;
        steps(&mut w, 30 * 120);
        let g = w.game().unwrap();
        (
            g.vehicles[&gun].ammo[1],
            g.players[0].manpower < mp0,
            g.players[0].s11.gun_shell_micro,
        )
    };
    let (he, _, micro) = run(0);
    assert_eq!((he, micro), (2, 0), "switched off");
    let (he, _, micro) = run(1);
    assert!(he > 2 && micro > 0, "control: {he} {micro}");
}

// ------------------------------------------------------------ Build B --

/// C6 (spec 4): the real leIG 18 and 105 mm M3 tables solve 300 m and 900 m
/// on flat ground, and no M3 solution lays above 30 deg (tan 30 = 0.5774)
/// while the leIG has a high root above it at 900 m. Lives here, not in the content
/// crate's tests, because the solver is the sim's (`indirect::solve_gun`).
/// Control: the M3 at 9,000 m (past its top charge) has no candidate.
#[test]
fn real_infantry_guns_solve_300_and_900_m_on_flat_ground() {
    use crate::fx::FxVec3;
    use crate::indirect::{candidates, solve_gun};
    let w = flat_world_with(1, |_| {});
    let s = w.match_setup().unwrap();
    let ind = |id: &str| {
        s.weapons
            .iter()
            .find(|w| w.id == id)
            .and_then(|w| w.indirect.clone())
            .unwrap_or_else(|| panic!("{id}"))
    };
    let (leig, m3) = (ind("ww2:leig18"), ind("ww2:m3_105mm"));
    let t = crate::terrain::Heightfield::generate(crate::terrain::TerrainKind::Flat, 1024, 0);
    let from = FxVec3::new(Fx::from_int(100), Fx::from_int(500), Fx::ONE);
    // tan 30 deg = 0.57735; the table rounds within 0.0005.
    let tan30 = Fx::from_ratio(5780, 10000);
    for d in [300, 900] {
        let to = FxVec3::new(Fx::from_int(100 + d), Fx::from_int(500), Fx::ZERO);
        let a = solve_gun(&leig, from, to, &t, true, None, Fx::ZERO).expect("leIG solves");
        let b = solve_gun(&m3, from, to, &t, true, None, Fx::ZERO).expect("M3 solves");
        assert!(b.tan <= tan30, "M3 at {d} m lays {:?}", b.tan);
        assert!(a.flight > Fx::ZERO && b.flight > Fx::ZERO);
        assert!(candidates(&m3, from, to).all(|c| c.tan <= tan30));
        // At 300 m every high root of the leIG is above its 73 deg band
        // (charge I: 79.8 deg); at 900 m charge II's is 71 deg.
        assert!(
            d == 300 || candidates(&leig, from, to).any(|c| c.tan > tan30),
            "the leIG has a high root at {d} m"
        );
    }
    let far = FxVec3::new(Fx::from_int(9100), Fx::from_int(500), Fx::ZERO);
    assert!(
        candidates(&m3, from, far).next().is_none(),
        "control: past range"
    );
}

/// A flat world with a 25 m crest band across x = 500 (the G2 crest of
/// `indirect.rs`, as a map band), player 0's real gun `card` at (50, 500)
/// and a Grenadier squad at (600, 500) that sees the target ground.
pub(super) fn crest_gun(card: &str, crest: bool) -> (World, SquadId) {
    use frontline_content::Fixed;
    use frontline_content::setup::{BandSetup, FixedVec2};
    let mut w = flat_world_with(1, |s| {
        if crest {
            s.map.terrain.bands.push(BandSetup {
                from: FixedVec2::from_m([500, 0]),
                to: FixedVec2::from_m([500, 1024]),
                height: Fixed::from_int(25),
                half_width: Fixed::from_int(40),
            });
        }
    });
    steps(&mut w, 2);
    let sq = spawn(&mut w, 0, card, 50, 500);
    spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 600, 500);
    steps(&mut w, 30);
    (w, sq)
}

/// Review A-6: `Masked` and `s11.masked` reached through the sim (the real
/// 105 mm M3 behind a 25 m crest band), and (review A-3) a gun refusal
/// leaves the mortar row `fm.refused` alone. Controls: the real leIG 18
/// from the same spot is accepted; the M3 on flat ground is accepted.
#[test]
fn real_m3_is_masked_behind_a_crest_through_the_sim() {
    let t = c(580, 500);
    let (mut w, sq) = crest_gun("ww2:usa_105mm_m3_squad", true);
    assert_eq!(order(&mut w, sq, t, 4), Some(FireMissionRefusal::Masked));
    let p = &w.game().unwrap().players[0];
    assert_eq!(p.s11.masked, 1);
    assert_eq!(
        p.fm.refused.iter().sum::<u32>(),
        0,
        "A-3: not in the mortar row"
    );
    let (mut w, sq) = crest_gun("ww2:ger_leig18_squad", true);
    assert_eq!(
        order(&mut w, sq, t, 4),
        None,
        "control: the leIG clears the crest"
    );
    let (mut w, sq) = crest_gun("ww2:usa_105mm_m3_squad", false);
    assert_eq!(order(&mut w, sq, t, 4), None, "control: flat ground");
}

/// Review A-6: two broken rules at once give the earlier refusal of G1's
/// order (Rounds before NotReady before NoBombs before UnknownGround
/// before CannotReach). Control: each pair's later rule alone gives the
/// later refusal (the single-rule cases of `gun_mission_refusals_in_order`).
#[test]
fn gun_refusals_take_the_first_broken_rule() {
    let mut s = scene();
    let (w, sq, gun) = (&mut s.w, s.sq, s.gun);
    let t = c(TARGET.0, TARGET.1);
    let size = w.match_setup().unwrap().map.size_m;
    let unknown = c(300, size - 30);
    let limber = |w: &mut World, on: bool| {
        w.edit_vehicle(gun, |vs| {
            vs.served.as_mut().unwrap().state = if on {
                GunState::Limbered
            } else {
                GunState::Emplaced
            }
        });
    };
    let (ammo, loaded) = {
        let vs = &w.game().unwrap().vehicles[&gun];
        (vs.ammo.clone(), vs.loaded)
    };
    let empty = |w: &mut World, on: bool| {
        let (a, l) = (ammo.clone(), loaded);
        w.edit_vehicle(gun, move |vs| {
            if on {
                vs.ammo[1] = 0;
                if vs.loaded == Some(1) {
                    vs.loaded = None;
                }
            } else {
                vs.ammo = a;
                vs.loaded = l;
            }
        });
    };
    limber(w, true);
    assert_eq!(order(w, sq, t, 0), Some(FireMissionRefusal::Rounds));
    empty(w, true);
    assert_eq!(order(w, sq, t, 4), Some(FireMissionRefusal::NotReady));
    limber(w, false);
    assert_eq!(order(w, sq, unknown, 4), Some(FireMissionRefusal::NoBombs));
    empty(w, false);
    assert_eq!(
        order(w, sq, unknown, 4),
        Some(FireMissionRefusal::UnknownGround)
    );
    assert_eq!(order(w, sq, t, 4), None, "control");
}

/// Review A-2: the `[indirect] range_m` of the gun's table bounds the
/// order (`CannotReach` past it). Control: a target inside it is taken.
#[test]
fn gun_refuses_past_its_table_range() {
    let mut s = scene_with(1, 0, |s| {
        let w = s
            .weapons
            .iter_mut()
            .find(|w| w.id == "test:inf_gun_main")
            .unwrap();
        w.indirect.as_mut().unwrap().range = frontline_content::Fixed::from_int(300);
    });
    let (w, sq) = (&mut s.w, s.sq);
    assert_eq!(
        order(w, sq, c(650, 500), 4),
        Some(FireMissionRefusal::CannotReach)
    );
    assert_eq!(order(w, sq, c(550, 500), 4), None, "control: 250 m");
}
