//! S15a Build A tests (docs/slices/S15-ussr.md 3, 5): the SAVE 34 layout
//! and its load checks, then the rules R (riders), P (plates), X (captured
//! bombs) and B (bayonets). Every test turns its switch on in the test
//! only, with test-made stand-in defs (no USSR def in the mod yet). Each
//! test names its control: the edit that makes it fail.

use frontline_content::MatchSetup;
use frontline_content::vehicle::{BoxSetup, TransportSetup};

use super::s6_tests::{flat_world_with, members, run, spawn, steps};
use super::*;
use crate::command::CommandKind;
use crate::fx::{FxVec2, FxVec3};
use crate::s15::{
    AiRiders, RIDER_BOARD, RIDER_DROP, RIDER_RIDE, RiderJob, RiderOff, RiderRefused, rider_exit,
};

pub(super) const RIFLES: &str = "ww2:ger_grenadiergruppe";
pub(super) const US_RIFLES: &str = "ww2:usa_rifle_squad";
/// The stand-in rider tank (plan R1): a Panzer IV with six deck slots.
pub(super) const TANK: &str = "ww2:ger_pz4_h_squad";
const TANK_DEF: &str = "ww2:ger_pz4_h";

pub(super) fn game_mut(s: &mut SimState) -> &mut MatchState {
    s.game.as_mut().unwrap()
}

fn ai(s: &mut SimState) -> &mut crate::ai::AiState {
    game_mut(s).ai[1].as_mut().unwrap()
}

/// A deck box (hull frame, cm), as the def's `[[rider_slots]] box`.
fn deck_box(x: i32, y: i32) -> BoxSetup {
    BoxSetup {
        center: frontline_content::vehicle::FixedVec3::from_cm([x, y, 176 + 45]),
        half: frontline_content::vehicle::FixedVec3::from_cm([22, 20, 45]),
    }
}

/// Plan R1 stand-in: the Panzer IV H gets ten rider slots on its engine
/// deck (outside every plate: the hull box tops out at 176 cm), entry at
/// the hull's rear, the rules' exit times.
pub(super) fn rider_tank(s: &mut MatchSetup) {
    let hz = s.tick_hz;
    let v = s
        .vehicles
        .iter_mut()
        .find(|v| v.id == TANK_DEF)
        .expect("the Panzer IV");
    let ticks = |ms: u32| (ms as u64 * hz as u64).div_ceil(1000) as u32;
    v.transport = Some(TransportSetup {
        entry: frontline_content::vehicle::FixedVec3::from_cm([-300, 0, 60]),
        board_ticks: ticks(1000),
        exit_ticks: ticks(600),
        exit_over_sides_ticks: Some(ticks(600)),
        exit_raise: frontline_content::setup::Fixed(0),
        slots: [-120, -170, -220, -265]
            .into_iter()
            .flat_map(|x| [(x, 60), (x, -60)])
            .chain([(-145, 0), (-245, 0)])
            .map(|(x, y)| deck_box(x, y))
            .collect(),
        exposed: true,
    });
}

/// The S15 staged save: player 1 is an AI; a German tank with rider slots
/// carrying a rifle squad, its riders getting off (R3), a rider job in
/// each phase and a failed pairing, S15 counters and the S15 AI counters.
/// Returns `(world, tank, riding squad)`.
pub(super) fn staged() -> (World, SimId, SquadId) {
    let mut w = flat_world_with(1, |s| {
        s.players[1].is_ai = true;
        rider_tank(s);
    });
    let tank_sq = spawn(&mut w, 0, TANK, 300, 300);
    let tank = members(&w, tank_sq)[0];
    let inf = spawn(&mut w, 0, RIFLES, 290, 300);
    spawn(&mut w, 1, "ww2:usa_m4a3_75_squad", 700, 700);
    spawn(&mut w, 1, US_RIFLES, 700, 690);
    steps(&mut w, 3);
    assert!(w.seat_now(inf, tank), "the squad boards the deck");
    steps(&mut w, 1);
    let s = &mut w.state;
    let g = game_mut(s);
    g.vehicles.get_mut(&tank).unwrap().rider_off = Some(RiderOff {
        ticks: 5,
        why: rider_exit::HIT,
    });
    g.players[0].s15.rider_boards = 6;
    g.players[0].s15.rider_exits[rider_exit::SUPPRESSED as usize] = 2;
    g.players[1].s15.plate_stops = 3;
    g.players[1].s15.bayonet_hits = 1;
    let k = &mut g.ai[1].as_mut().unwrap().counters;
    k.rider_jobs = 2;
    k.rider_job_ends[3] = 1;
    w.refresh_hashes();
    (w, tank, inf)
}

/// [`staged`] with AI rider jobs: player 1's own rider tanks, one job per
/// phase. Returns `(world, the AI's tanks)`.
pub(super) fn staged_jobs() -> (World, Vec<SimId>) {
    let (mut w, _, _) = staged();
    let mut tanks = Vec::new();
    let mut squads = Vec::new();
    for k in 0..3 {
        let t = spawn(&mut w, 1, TANK, 600 + 20 * k, 700);
        tanks.push(members(&w, t)[0]);
        squads.push(spawn(&mut w, 1, US_RIFLES, 600 + 20 * k, 680));
    }
    steps(&mut w, 2);
    let tick = w.tick();
    let s = &mut w.state;
    let a = ai(s);
    a.riders = AiRiders {
        jobs: [RIDER_BOARD, RIDER_RIDE, RIDER_DROP]
            .into_iter()
            .enumerate()
            .map(|(k, phase)| RiderJob {
                tank: tanks[k],
                squad: squads[k],
                point: 0,
                phase,
                phase_at: tick - 1,
            })
            .collect(),
        refused: vec![RiderRefused {
            tank: tanks[0],
            squad: squads[2],
            at: tick - 2,
        }],
    };
    w.refresh_hashes();
    (w, tanks)
}

/// Plan 3: the S15 state saves, loads and hashes. Control: an unedited
/// reload keeps the hash (each edit must change it).
#[test]
fn s15_layout_roundtrips_and_hashes() {
    let (w, _) = staged_jobs();
    let tank = *w
        .game()
        .unwrap()
        .vehicles
        .iter()
        .find(|(_, v)| v.rider_off.is_some())
        .unwrap()
        .0;
    let rider = w.game().unwrap().riders_of(tank)[0];
    let bytes = w.save();
    let back = World::load(&bytes, 1).expect("the S15 state loads");
    assert_eq!(back.state(), w.state());
    assert_eq!(back.full_state_hash(), w.full_state_hash());
    assert_eq!(back.save(), bytes);
    let h = back.full_state_hash();
    assert_eq!(World::load(&back.save(), 1).unwrap().full_state_hash(), h);
    type Edit = Box<dyn Fn(&mut SimState)>;
    let edits: Vec<Edit> = vec![
        Box::new(move |s| {
            game_mut(s)
                .vehicles
                .get_mut(&tank)
                .unwrap()
                .rider_off
                .as_mut()
                .unwrap()
                .ticks -= 1
        }),
        Box::new(move |s| {
            game_mut(s)
                .vehicles
                .get_mut(&tank)
                .unwrap()
                .rider_off
                .as_mut()
                .unwrap()
                .why = rider_exit::GUN_FIRED
        }),
        Box::new(move |s| game_mut(s).vehicles.get_mut(&tank).unwrap().rider_off = None),
        Box::new(|s| game_mut(s).players[0].s15.plate_hits += 1),
        Box::new(|s| game_mut(s).players[0].s15.plate_stops_frag += 1),
        Box::new(|s| game_mut(s).players[0].s15.plate_stops_melee += 1),
        Box::new(|s| game_mut(s).players[0].s15.rider_boards += 1),
        Box::new(|s| game_mut(s).players[0].s15.rider_exits[4] += 1),
        Box::new(|s| game_mut(s).players[0].s15.rider_casualties += 1),
        Box::new(|s| game_mut(s).players[1].s15.foreign_drawn += 1),
        Box::new(|s| game_mut(s).players[1].s15.foreign_fired += 1),
        Box::new(|s| game_mut(s).players[1].s15.bayonet_strikes += 1),
        Box::new(|s| game_mut(s).players[1].s15.bayonet_hits += 1),
        Box::new(|s| ai(s).counters.rider_jobs += 1),
        Box::new(|s| ai(s).counters.rider_job_ends[0] += 1),
        Box::new(|s| ai(s).riders.jobs[1].phase = RIDER_DROP),
        Box::new(|s| ai(s).riders.jobs[0].phase_at -= 1),
        Box::new(|s| ai(s).riders.jobs[2].point = 1),
        Box::new(|s| ai(s).riders.refused[0].at -= 1),
        Box::new(|s| ai(s).riders.refused.clear()),
    ];
    for (k, e) in edits.into_iter().enumerate() {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        let y = World::load(&x.save(), 1).unwrap_or_else(|e| panic!("edit {k}: {e:?}"));
        assert_ne!(y.full_state_hash(), h, "edit {k} is not hashed");
    }
    // `Kit::foreign` is hashed with the man's chunk (it cannot load before
    // a weapon has a foreign feed: `load_rejects_bad_s15_state`).
    let mut x = World::load(&bytes, 1).unwrap();
    x.state.entities.kit[rider.index()].foreign = 1;
    x.hash.mark_entity_chunk(rider.chunk());
    x.refresh_hashes();
    assert_ne!(x.full_state_hash(), h, "Kit::foreign is not hashed");
}

/// Plan 3 N3 load validation, one case per check. Control: the staged
/// save itself loads (checked first), so each failure is the edit's.
#[test]
fn load_rejects_bad_s15_state() {
    let (w, tanks) = staged_jobs();
    let bytes = w.save();
    World::load(&bytes, 1).expect("control: the staged save loads");
    let tick = w.tick();
    let g = w.game().unwrap();
    let (tank, _) = g
        .vehicles
        .iter()
        .find(|(_, v)| v.rider_off.is_some())
        .unwrap();
    let tank = *tank;
    let rider = g.riders_of(tank)[0];
    let walker = g
        .squads
        .values()
        .find(|q| q.owner == PlayerId(1) && q.members.len() > 1)
        .unwrap()
        .members[0];
    // A tank without rider slots: player 1's Sherman.
    let plain = *g
        .vehicles
        .iter()
        .find(|(_, v)| v.riders.is_empty())
        .unwrap()
        .0;
    let _ = tanks;
    type Edit = Box<dyn Fn(&mut SimState)>;
    let cases: Vec<(&str, Edit)> = vec![
        (
            "rider job on a tank without slots",
            Box::new(move |s| ai(s).riders.jobs[0].tank = plain),
        ),
        (
            "rider job phase out of range",
            Box::new(|s| ai(s).riders.jobs[1].phase = RIDER_DROP + 1),
        ),
        (
            "rider job in the future",
            Box::new(move |s| ai(s).riders.jobs[0].phase_at = tick + 1),
        ),
        (
            "rider jobs not ascending",
            Box::new(|s| ai(s).riders.jobs.swap(0, 2)),
        ),
        (
            "rider job of an unknown point",
            Box::new(|s| ai(s).riders.jobs[0].point = 99),
        ),
        (
            "rider job of an enemy squad",
            Box::new(|s| {
                let g = game_mut(s);
                let sq = *g
                    .squads
                    .iter()
                    .find(|(_, q)| q.owner == PlayerId(0))
                    .unwrap()
                    .0;
                g.ai[1].as_mut().unwrap().riders.jobs[0].squad = sq;
            }),
        ),
        (
            "refused pairing in the future",
            Box::new(move |s| ai(s).riders.refused[0].at = tick + 1),
        ),
        (
            "rider timer at zero",
            Box::new(move |s| {
                game_mut(s)
                    .vehicles
                    .get_mut(&tank)
                    .unwrap()
                    .rider_off
                    .as_mut()
                    .unwrap()
                    .ticks = 0
            }),
        ),
        (
            "rider timer over the exit time",
            Box::new(move |s| {
                game_mut(s)
                    .vehicles
                    .get_mut(&tank)
                    .unwrap()
                    .rider_off
                    .as_mut()
                    .unwrap()
                    .ticks = 19
            }),
        ),
        (
            "rider timer with an ordered reason",
            Box::new(move |s| {
                game_mut(s)
                    .vehicles
                    .get_mut(&tank)
                    .unwrap()
                    .rider_off
                    .as_mut()
                    .unwrap()
                    .why = rider_exit::ORDERED
            }),
        ),
        (
            "rider timer on a tank without slots",
            Box::new(move |s| {
                game_mut(s).vehicles.get_mut(&plain).unwrap().rider_off = Some(RiderOff {
                    ticks: 3,
                    why: rider_exit::HIT,
                })
            }),
        ),
        (
            "foreign rounds without a foreign feed",
            Box::new(move |s| s.entities.kit[rider.index()].foreign = 1),
        ),
        (
            "foreign rounds on a walker without a feed",
            Box::new(move |s| s.entities.kit[walker.index()].foreign = 3),
        ),
        (
            "s15 counter out of range",
            Box::new(|s| game_mut(s).players[0].s15.plate_stops = u32::MAX),
        ),
        (
            "s15 AI counter out of range",
            Box::new(|s| ai(s).counters.rider_job_ends[2] = u32::MAX),
        ),
    ];
    for (name, e) in cases {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        assert!(World::load(&x.save(), 1).is_err(), "{name} loads");
    }
}

/// Plan 2: every S15 key is part of the setup (saved, hashed) and checked
/// against its range. Control: the unedited setup is valid, and each
/// in-range edit is valid too.
#[test]
fn load_rejects_bad_s15_keys() {
    let w = flat_world_with(1, |_| {});
    let base = w.game().unwrap().setup.clone();
    let h0 = postcard::to_allocvec(&base).unwrap();
    assert!(
        base.validate().is_empty(),
        "control: the unedited setup is valid"
    );
    type Edit = Box<dyn Fn(&mut MatchSetup)>;
    let good: Vec<Edit> = vec![
        Box::new(|s| s.rules.riders.enabled = 1),
        Box::new(|s| s.rules.riders.off_on_suppression = 0),
        Box::new(|s| s.rules.riders.suppression_pct = 150),
        Box::new(|s| s.rules.riders.off_on_hit = 0),
        Box::new(|s| s.rules.riders.off_on_fire = 0),
        Box::new(|s| s.rules.riders.exit_ms = 900),
        Box::new(|s| s.rules.riders.stun_ms = 2000),
        Box::new(|s| s.rules.body_armour.enabled = 1),
        Box::new(|s| s.rules.body_armour.soft_pen_tenths_mm = 25),
        Box::new(|s| s.rules.body_armour.stopped_damage_pct = 10),
        Box::new(|s| s.rules.body_armour.stops_melee = 0),
        Box::new(|s| s.rules.body_armour.prone = 1),
        Box::new(|s| s.rules.melee.bayonets = 1),
        Box::new(|s| s.rules.supply.as_mut().unwrap().foreign_ammo = 1),
        Box::new(|s| s.rules.ai.riders.jobs = 1),
        Box::new(|s| s.rules.ai.riders.squads_max = 3),
        Box::new(|s| s.rules.ai.riders.drop_m = 100),
        Box::new(|s| s.rules.ai.riders.job_max_s = 200),
        Box::new(|s| s.rules.ai.riders.stall_s = 30),
        Box::new(|s| s.rules.ai.riders.recheck_s = 90),
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
        ("riders enabled 2", Box::new(|s| s.rules.riders.enabled = 2)),
        (
            "suppression_pct 401",
            Box::new(|s| s.rules.riders.suppression_pct = 401),
        ),
        ("off_on_hit 2", Box::new(|s| s.rules.riders.off_on_hit = 2)),
        ("exit_ms 99", Box::new(|s| s.rules.riders.exit_ms = 99)),
        (
            "stun_ms 30001",
            Box::new(|s| s.rules.riders.stun_ms = 30_001),
        ),
        (
            "body_armour enabled 2",
            Box::new(|s| s.rules.body_armour.enabled = 2),
        ),
        (
            "soft_pen_tenths_mm 1001",
            Box::new(|s| s.rules.body_armour.soft_pen_tenths_mm = 1001),
        ),
        (
            "stopped_damage_pct 401",
            Box::new(|s| s.rules.body_armour.stopped_damage_pct = 401),
        ),
        ("prone 2", Box::new(|s| s.rules.body_armour.prone = 2)),
        ("bayonets 2", Box::new(|s| s.rules.melee.bayonets = 2)),
        (
            "foreign_ammo 2",
            Box::new(|s| s.rules.supply.as_mut().unwrap().foreign_ammo = 2),
        ),
        ("ai.riders jobs 2", Box::new(|s| s.rules.ai.riders.jobs = 2)),
        (
            "ai.riders squads_max 9",
            Box::new(|s| s.rules.ai.riders.squads_max = 9),
        ),
        (
            "ai.riders drop_m 0",
            Box::new(|s| s.rules.ai.riders.drop_m = 0),
        ),
        (
            "ai.riders recheck_s 3601",
            Box::new(|s| s.rules.ai.riders.recheck_s = 3601),
        ),
    ];
    for (name, e) in bad {
        let mut s = base.clone();
        e(&mut s);
        assert!(!s.validate().is_empty(), "{name} is accepted");
    }
}

/// Plan 2 M1-M4: `[matchups]` shape checks. Control: the shipped table
/// is valid.
#[test]
fn matchups_table_is_checked() {
    use frontline_content::MatchupsDef;
    let ok = MatchupsDef {
        factions: vec!["ww2:germany".into(), "ww2:usa".into()],
        pairs: vec![["ww2:germany".into(), "ww2:usa".into()]],
        free: 0,
        mirror: 0,
    };
    let mut out = Vec::new();
    ok.validate(&mut out);
    assert!(out.is_empty(), "control: {out:?}");
    let bad: Vec<(&str, MatchupsDef)> = vec![
        (
            "free 2",
            MatchupsDef {
                free: 2,
                ..ok.clone()
            },
        ),
        (
            "mirror 2",
            MatchupsDef {
                mirror: 2,
                ..ok.clone()
            },
        ),
        (
            "a pair of one faction",
            MatchupsDef {
                pairs: vec![["ww2:usa".into(), "ww2:usa".into()]],
                ..ok.clone()
            },
        ),
        (
            "a pair outside the list",
            MatchupsDef {
                pairs: vec![["ww2:germany".into(), "ww2:ussr".into()]],
                ..ok.clone()
            },
        ),
        (
            "a pair twice",
            MatchupsDef {
                pairs: vec![
                    ["ww2:germany".into(), "ww2:usa".into()],
                    ["ww2:usa".into(), "ww2:germany".into()],
                ],
                ..ok.clone()
            },
        ),
        (
            "a faction twice",
            MatchupsDef {
                factions: vec!["ww2:usa".into(), "ww2:usa".into()],
                pairs: vec![],
                ..ok.clone()
            },
        ),
    ];
    for (name, m) in bad {
        let mut out = Vec::new();
        m.validate(&mut out);
        assert!(!out.is_empty(), "{name} is accepted");
    }
}

// ------------------------------------------------------------ R1-R6 --

/// A world with the stand-in rider tank, `[riders] enabled` = `on` (the
/// other R3 switches as `edit` leaves them).
fn rider_world(on: u32, edit: impl FnOnce(&mut MatchSetup)) -> World {
    flat_world_with(1, |s| {
        rider_tank(s);
        s.rules.riders.enabled = on;
        edit(s);
    })
}

fn weapon(w: &World, id: &str) -> u16 {
    w.match_setup()
        .unwrap()
        .weapons
        .iter()
        .position(|x| x.id == id)
        .unwrap_or_else(|| panic!("{id}")) as u16
}

fn s15(w: &World, p: usize) -> crate::s15::S15Counters {
    w.game().unwrap().players[p].s15.clone()
}

/// A tank of player 0 at (500, 500) facing east with a rifle squad seated
/// on its deck. Returns `(tank, squad)`.
fn ride(w: &mut World) -> (SimId, SquadId) {
    let t = spawn(w, 0, TANK, 500, 500);
    let tank = members(w, t)[0];
    let inf = spawn(w, 0, RIFLES, 490, 500);
    steps(w, 2);
    assert!(w.seat_now(inf, tank));
    steps(w, 1);
    (tank, inf)
}

fn exit_ticks(w: &World) -> u64 {
    let s = w.match_setup().unwrap();
    (s.rules.riders.exit_ms as u64 * s.tick_hz as u64).div_ceil(1000)
}

/// R1-R2: `Embark` on a tank with rider slots: the squad walks to the rear
/// and climbs on; every man is counted. Control: the same tank without
/// slots refuses the order (nobody boards).
#[test]
fn riders_board_a_tank() {
    for slots in [true, false] {
        let mut w = flat_world_with(1, |s| {
            if slots {
                rider_tank(s);
            }
        });
        let t = spawn(&mut w, 0, TANK, 500, 500);
        let tank = members(&w, t)[0];
        let inf = spawn(&mut w, 0, RIFLES, 480, 510);
        steps(&mut w, 2);
        run(
            &mut w,
            0,
            1,
            CommandKind::Embark {
                squads: vec![inf],
                vehicle: tank,
            },
        );
        let n = members(&w, inf).len();
        for _ in 0..900 {
            steps(&mut w, 1);
            if w.game().unwrap().riders_of(tank).len() == n {
                break;
            }
        }
        let aboard = w.game().unwrap().riders_of(tank).len();
        if slots {
            assert_eq!(aboard, n, "the squad is on the deck");
            assert_eq!(s15(&w, 0).rider_boards, n as u32);
        } else {
            assert_eq!(aboard, 0, "control: no slots, no riders");
        }
    }
}

/// R4: a rifle round at a rider on the deck hits him (his box is outside
/// every plate). Control: the same round with the deck empty hurts no
/// man of player 0.
#[test]
fn rider_hit_on_the_deck() {
    for seated in [true, false] {
        let mut w = rider_world(1, |s| s.rules.riders.off_on_suppression = 0);
        let t = spawn(&mut w, 0, TANK, 500, 500);
        let tank = members(&w, t)[0];
        let inf = spawn(&mut w, 0, RIFLES, 470, 520);
        steps(&mut w, 2);
        if seated {
            assert!(w.seat_now(inf, tank));
            steps(&mut w, 1);
        }
        let g = w.game().unwrap();
        // The front rider's slot centre, in the world.
        let geo = &g.vderived.geo[g.vehicles[&tank].type_idx as usize];
        let f = g.vderived.frame(tank).unwrap();
        let c = f.origin + f.hull.axes.apply(geo.slots[0].0);
        let hp0: u32 = members(&w, inf)
            .iter()
            .map(|m| w.state.entities.combat[m.index()].hp as u32)
            .sum();
        let kar = weapon(&w, "ww2:kar98k");
        let from = FxVec3::new(c.x, c.y + Fx::from_int(30), c.z);
        assert!(w.fire_round_now(1, kar, from, c));
        steps(&mut w, 30);
        let hp1: u32 = members(&w, inf)
            .iter()
            .map(|m| {
                let e = &w.state.entities;
                if e.kind[m.index()] == EntityKind::Soldier {
                    e.combat[m.index()].hp as u32
                } else {
                    0
                }
            })
            .sum();
        if seated {
            assert!(hp1 < hp0, "the rider is hit ({hp0} -> {hp1})");
        } else {
            assert_eq!(hp1, hp0, "control: an empty deck");
        }
    }
}

/// R3a: a rider past the threshold gets everyone off over the sides
/// within `exit_ms`, stunned; the tank keeps its order. Control:
/// `[riders] enabled` 0 leaves them on.
#[test]
fn riders_off_when_suppressed() {
    for on in [1, 0] {
        let mut w = rider_world(on, |_| {});
        let (tank, inf) = ride(&mut w);
        let n = members(&w, inf).len();
        let m0 = members(&w, inf)[0];
        let at = {
            let s = w.match_setup().unwrap();
            s.rules.combat.crouch_suppression * 1000
        };
        assert!(w.edit_soldier(m0, |_, c| c.suppression = at + 1000));
        let n_ex = exit_ticks(&w) + 2;
        steps(&mut w, n_ex);
        let left = w.game().unwrap().riders_of(tank).len();
        if on == 1 {
            assert_eq!(left, 0, "everyone is off");
            assert_eq!(
                s15(&w, 0).rider_exits[rider_exit::SUPPRESSED as usize],
                n as u32
            );
            let m1 = members(&w, inf)[1];
            assert!(w.state.entities.weapon[m1.index()].cooldown > 0, "stunned");
        } else {
            assert_eq!(left, n, "control: switch off, they stay");
        }
    }
}

/// R3a (S15a Build A, W2): an enemy mortar burst 10 m off the tank
/// suppresses its riders (they are in no spatial hash; no fragment
/// reaches them at 10 m). Control: `[riders] enabled` 0: the same burst
/// leaves them unsuppressed and all on the deck. (The exit past the
/// threshold is `riders_off_when_suppressed`.)
#[test]
fn riders_suppressed_by_a_burst() {
    for on in [1, 0] {
        let mut w = rider_world(on, |_| {});
        let (tank, inf) = ride(&mut w);
        let n = members(&w, inf).len();
        let p = w.state.entities.transform[tank.index()].pos.xy();
        let grw = weapon(&w, "ww2:grw34");
        assert!(w.drop_shell_now(1, grw, p + FxVec2::new(Fx::ZERO, Fx::from_int(10))));
        // The shell falls 20 m (about 10 ticks): the most any man has.
        let mut supp = 0;
        for _ in 0..30 {
            steps(&mut w, 1);
            for m in members(&w, inf) {
                supp = supp.max(w.state.entities.combat[m.index()].suppression);
            }
        }
        if on == 1 {
            assert!(supp > 0, "the burst suppresses the riders");
        } else {
            assert_eq!(supp, 0, "control: switch off, no rider suppressed");
            let left = w.game().unwrap().riders_of(tank).len();
            assert_eq!(left, n, "control: they stay");
        }
    }
}

/// S15a Build A step 6: the snapshot marks plated men, a rider deck and
/// the riders' exit under way. Control: both switches 0 (the same plates
/// and slots left in the setup) mark nothing.
#[test]
fn snapshot_shows_plates_and_deck() {
    use crate::snapshot::vehicle_flags::{DECK, RIDERS_OFF};
    for on in [1, 0] {
        let mut w = rider_world(on, |s| plate_all(s, on, 120));
        let (tank, inf) = ride(&mut w);
        let flags = |w: &World| {
            let snap = w.snapshot();
            snap.vehicles.iter().find(|v| v.id == tank.0).unwrap().flags
        };
        let snap = w.snapshot();
        let plated = members(&w, inf)
            .iter()
            .filter(|m| {
                let k = snap.soldier_ids.iter().position(|x| x == *m).unwrap();
                snap.soldier_plate[k]
            })
            .count();
        assert_eq!(snap.soldier_plate.len(), snap.soldier_ids.len());
        let f0 = flags(&w);
        let m0 = members(&w, inf)[0];
        let at = w.match_setup().unwrap().rules.combat.crouch_suppression * 1000;
        assert!(w.edit_soldier(m0, |_, c| c.suppression = at + 1000));
        steps(&mut w, 1);
        let f1 = flags(&w);
        if on == 1 {
            assert_eq!(plated, members(&w, inf).len(), "every rider is plated");
            assert!(f0 & DECK != 0 && f0 & RIDERS_OFF == 0);
            assert!(f1 & RIDERS_OFF != 0, "the exit runs");
        } else {
            assert_eq!(plated, 0, "control: no plate shown");
            assert_eq!((f0 | f1) & (DECK | RIDERS_OFF), 0, "control: no deck");
        }
    }
}

/// R3b: an AT rocket striking the tank gets its riders off. Control:
/// `off_on_hit` 0 (R3a off in both runs: the strike's suppression alone
/// does not count here).
#[test]
fn riders_off_when_tank_is_hit() {
    for hit in [1, 0] {
        let mut w = rider_world(1, |s| {
            s.rules.riders.off_on_suppression = 0;
            s.rules.riders.off_on_hit = hit;
        });
        let (tank, inf) = ride(&mut w);
        let n = members(&w, inf).len();
        let p = w.state.entities.transform[tank.index()].pos;
        let aim = FxVec3::new(p.x, p.y, p.z + Fx::ONE);
        let from = FxVec3::new(p.x + Fx::from_int(40), p.y, aim.z);
        let rpzb = weapon(&w, "ww2:rpzb54");
        assert!(w.fire_rocket_now(1, rpzb, from, aim));
        let n_ex = 60 + exit_ticks(&w);
        steps(&mut w, n_ex);
        let left = w.game().unwrap().riders_of(tank).len();
        let hits = s15(&w, 0).rider_exits[rider_exit::HIT as usize];
        assert!(
            w.game().unwrap().vehicles[&tank].last_attacker == Some(1),
            "the rocket struck the tank"
        );
        if hit == 1 {
            assert_eq!(left, 0, "everyone is off");
            assert!(hits > 0 && hits <= n as u32);
        } else {
            assert_eq!(hits, 0, "control: off_on_hit 0");
        }
    }
}

/// R3c: the tank's main gun firing gets its riders off. Control:
/// `off_on_fire` 0.
#[test]
fn riders_off_when_gun_fires() {
    for fire in [1, 0] {
        let mut w = rider_world(1, |s| {
            s.rules.riders.off_on_suppression = 0;
            s.rules.riders.off_on_hit = 0;
            s.rules.riders.off_on_fire = fire;
        });
        let (tank, _) = ride(&mut w);
        spawn(&mut w, 1, "ww2:usa_m4a3_75_squad", 700, 500);
        for _ in 0..1800 {
            steps(&mut w, 1);
            if w.game().unwrap().vehicles[&tank].since_fire < 5 {
                break;
            }
        }
        assert!(
            w.game().unwrap().vehicles[&tank].since_fire < u16::MAX,
            "the gun fired"
        );
        let n_ex = exit_ticks(&w) + 2;
        steps(&mut w, n_ex);
        let fired = s15(&w, 0).rider_exits[rider_exit::GUN_FIRED as usize];
        if fire == 1 {
            assert!(fired > 0, "riders off when the gun fired");
        } else {
            assert_eq!(fired, 0, "control: off_on_fire 0");
        }
    }
}

/// R5: riders neither capture nor contest a point (S8a). Control: the
/// same men standing on the point capture it.
#[test]
fn riders_do_not_capture() {
    for riding in [true, false] {
        let mut w = rider_world(1, |s| s.rules.riders.off_on_suppression = 0);
        let at = {
            let p = w.match_setup().unwrap().map.points[2].pos;
            (Fx::from(p.x).floor_int(), Fx::from(p.y).floor_int())
        };
        let inf = spawn(&mut w, 0, RIFLES, at.0, at.1 + 4);
        if riding {
            let t = spawn(&mut w, 0, TANK, at.0, at.1);
            let tank = members(&w, t)[0];
            steps(&mut w, 2);
            assert!(w.seat_now(inf, tank));
        }
        steps(&mut w, 600);
        let pt = &w.game().unwrap().points()[2];
        let took = pt.capturer == Some(PlayerId(0)) || pt.owner == Some(PlayerId(0));
        assert_eq!(took, !riding, "riding {riding}: {pt:?}");
    }
}

/// R3 save/load mid-exit: a save taken while the timer runs loads and
/// plays on tick for tick. Control: the unedited run.
#[test]
fn riders_save_and_load_mid_exit() {
    let mut w = rider_world(1, |_| {});
    let (tank, inf) = ride(&mut w);
    let m0 = members(&w, inf)[0];
    assert!(w.edit_soldier(m0, |_, c| c.suppression = 100_000));
    steps(&mut w, 3);
    assert!(
        w.game().unwrap().vehicles[&tank].rider_off.is_some(),
        "mid-exit"
    );
    let mut back = World::load(&w.save(), 1).expect("mid-exit loads");
    for _ in 0..40 {
        steps(&mut w, 1);
        steps(&mut back, 1);
        assert_eq!(w.full_state_hash(), back.full_state_hash());
    }
    assert!(w.game().unwrap().riders_of(tank).is_empty());
}

// ------------------------------------------------------------ P1-P5 --

/// Every member of every squad type wears a 2.0 mm torso plate over a
/// front arc of `arc` deg; `[body_armour] enabled` = `on`, wounds on (the
/// zones).
fn plate_all(s: &mut MatchSetup, on: u32, arc: u16) {
    s.rules.body_armour.enabled = on;
    s.rules.wounds.enabled = 1;
    for t in &mut s.squad_types {
        for m in &mut t.members {
            m.plate = Some(frontline_content::setup::PlateSetup {
                tenths_mm: 20,
                zone: crate::wounds::ZONE_TORSO,
                arc_deg: arc,
            });
        }
    }
}

/// Where a zone's middle is on a standing man, as a share of his height
/// (%): head, torso, legs by `[wounds]`.
fn zone_pct(w: &World, zone: u8) -> i64 {
    let t = &w.match_setup().unwrap().rules.wounds;
    let (head, legs) = (t.head_top_pct as i64, t.legs_bottom_pct as i64);
    match zone {
        crate::wounds::ZONE_HEAD => 100 - head / 2,
        crate::wounds::ZONE_LEGS => legs / 2,
        _ => (legs + 100 - head) / 2,
    }
}

/// One round of weapon `wpn` (def id) from `dist` m at a lone plated man
/// of player 0 held at (500, 500), fired from where he faces (`back`:
/// from behind) at the middle of `zone`. (He faces the way his squad
/// walks him, so his facing is read after a held tick.) Returns (HP lost,
/// his suppression after, player 0's counters).
fn plate_shot(
    on: u32,
    wpn: &str,
    dist: i32,
    back: bool,
    zone: u8,
) -> (u16, u32, crate::s15::S15Counters) {
    let mut w = flat_world_with(1, |s| plate_all(s, on, 120));
    let m = super::s11a2_tests::lone(&mut w, 0, 500, 500);
    let i = m.index();
    let hold = |w: &mut World| {
        w.place_soldier(m, FxVec2::from_int(500, 500));
        steps(w, 1);
    };
    hold(&mut w);
    hold(&mut w);
    let t = w.state.entities.transform[i];
    let h = w.state.entities.combat[i].height;
    let z = t.pos.z + Fx::from_raw((h.raw() as i64 * zone_pct(&w, zone) / 100) as i32);
    let (s, c) = t.yaw.sin_cos();
    let d = Fx::from_int(if back { -dist } else { dist });
    let from = FxVec3::new(t.pos.x + c.mul(d), t.pos.y + s.mul(d), z);
    let aim = FxVec3::new(t.pos.x, t.pos.y, z);
    let hp0 = w.state.entities.combat[i].hp;
    let wi = weapon(&w, wpn);
    assert!(w.fire_round_now(1, wi, from, aim));
    for _ in 0..60 {
        hold(&mut w);
        if s15(&w, 0).plate_hits > 0 || w.state.entities.combat[i].hp < hp0 {
            break;
        }
    }
    let c = &w.state.entities.combat[i];
    (hp0 - c.hp.min(hp0), c.suppression, s15(&w, 0))
}

/// P1-P3: an MP 40 round at 50 m at the chest of a standing plated man
/// from his front is stopped (no damage, the hit's suppression kept).
/// Control: `[body_armour] enabled` 0 (the same plate in the setup)
/// lets it through.
#[test]
fn plate_stops_smg_from_the_front() {
    let (lost, supp, k) = plate_shot(1, "ww2:mp40", 50, false, crate::wounds::ZONE_TORSO);
    assert_eq!((k.plate_hits, k.plate_stops), (1, 1));
    assert_eq!(lost, 0, "stopped: no damage");
    assert!(supp > 0, "P3: the hit still suppresses");
    let (lost, _, k) = plate_shot(0, "ww2:mp40", 50, false, crate::wounds::ZONE_TORSO);
    assert_eq!(k.plate_hits, 0);
    assert!(lost > 0, "control: plates off, the round hurts");
}

/// P2: a Kar 98k round (5 mm at 100 m) beats a 2 mm plate: met, not
/// stopped, full damage. Control: the MP 40 (no table: 1.0 mm) is
/// stopped at the same range.
#[test]
fn plate_does_not_stop_a_rifle() {
    let (lost, _, k) = plate_shot(1, "ww2:kar98k", 50, false, crate::wounds::ZONE_TORSO);
    assert_eq!((k.plate_hits, k.plate_stops), (1, 0));
    assert!(lost > 0);
    let (lost, _, k) = plate_shot(1, "ww2:mp40", 50, false, crate::wounds::ZONE_TORSO);
    assert_eq!((k.plate_stops, lost), (1, 0), "control");
}

/// P1: a round from behind (outside the 120 deg front arc) does not meet
/// the plate. Control: from the front it is stopped.
#[test]
fn plate_does_not_cover_the_back() {
    let (lost, _, k) = plate_shot(1, "ww2:mp40", 50, true, crate::wounds::ZONE_TORSO);
    assert_eq!(k.plate_hits, 0);
    assert!(lost > 0, "from behind: hurt");
    let (_, _, k) = plate_shot(1, "ww2:mp40", 50, false, crate::wounds::ZONE_TORSO);
    assert_eq!(k.plate_stops, 1, "control: the front");
}

/// P1: the plate covers the torso only: head and leg hits from the front
/// go through untouched. Control: the torso hit is stopped.
#[test]
fn plate_does_not_cover_head_or_legs() {
    for zone in [crate::wounds::ZONE_HEAD, crate::wounds::ZONE_LEGS] {
        let (lost, _, k) = plate_shot(1, "ww2:mp40", 50, false, zone);
        assert_eq!(k.plate_hits, 0, "zone {zone}");
        assert!(lost > 0, "zone {zone}: hurt");
    }
    let (lost, _, _) = plate_shot(1, "ww2:mp40", 50, false, crate::wounds::ZONE_TORSO);
    assert_eq!(lost, 0, "control: the torso");
}

/// P2: mortar bursts east of a plated man facing them: fragments with
/// `frag_pen_mm` 1 (under the plate) are stopped; control: the mod's 9
/// mm fragments go through the plate (met, none stopped).
#[test]
fn plate_stops_a_light_fragment_only() {
    let mut out = Vec::new();
    for pen in [1, 9] {
        let mut w = flat_world_with(1, |s| {
            plate_all(s, 1, 120);
            let k = s.weapons.iter().position(|x| x.id == "ww2:grw34").unwrap();
            s.weapons[k].he.as_mut().unwrap().frag_pen_mm = pen;
        });
        let m = super::s11a2_tests::lone(&mut w, 0, 500, 500);
        w.place_soldier(m, FxVec2::from_int(500, 500));
        w.state.entities.transform[m.index()].yaw = Fx::ZERO;
        let mortar = weapon(&w, "ww2:grw34");
        let ground = w.state.terrain.clone();
        for k in 0..40 {
            w.state.entities.combat[m.index()].hp = 100;
            super::s11a2_tests::burst_now(
                &mut w,
                1,
                mortar,
                0,
                FxVec2::from_int(508, 500),
                1000 + k,
            );
            w.state.terrain = ground.clone();
        }
        out.push(s15(&w, 0));
    }
    assert!(out[0].plate_stops_frag > 0, "light fragments stopped");
    assert!(out[1].plate_hits > 0, "control: heavy fragments meet it");
    assert_eq!(out[1].plate_stops_frag, 0, "control: and go through");
}

/// P4: a butt stroke at a plated US man is stopped (`stops_melee` 1;
/// the plate's arc 360 deg here: the facing is P1's, tested above).
/// Control: `stops_melee` 0 lets it hurt.
#[test]
fn plate_stops_a_strike() {
    for stops in [1, 0] {
        let (mut w, a, b) =
            super::s14c_tests::duel(1, "ww2:kar98k", "ww2:m1_garand", (80, 0), |s| {
                plate_all(s, 1, 360);
                s.rules.body_armour.stops_melee = stops;
            });
        use super::s14c_tests::{hp, off, tick_held, wind_up};
        let full = hp(&w, b);
        wind_up(&mut w, a, off(200, 0));
        tick_held(&mut w, (a, off(0, 0)), (b, off(80, 0)), true);
        let k = s15(&w, 1);
        assert_eq!(w.game().unwrap().players[0].s14.strike_hits, 1);
        if stops == 1 {
            assert_eq!((k.plate_stops_melee, hp(&w, b)), (1, full));
        } else {
            assert_eq!(k.plate_stops_melee, 0);
            assert!(hp(&w, b) < full, "control: stops_melee 0");
        }
    }
}

/// P5: a flame particle at a plated man's chest from his front ignores
/// the plate (no meeting, damage). Control: an MP 40 round on the same
/// line is stopped.
#[test]
fn flame_ignores_the_plate() {
    let (lost, _, k) = plate_shot(1, "ww2:fmw41", 8, false, crate::wounds::ZONE_TORSO);
    assert_eq!(k.plate_hits, 0);
    assert!(lost > 0, "the flame burns through");
    let (lost, _, k) = plate_shot(1, "ww2:mp40", 8, false, crate::wounds::ZONE_TORSO);
    assert_eq!((k.plate_stops, lost), (1, 0), "control");
}

/// N2 for plates: with `[body_armour] enabled` 0 a match whose men carry
/// plates in the setup runs as one without them (same hash, tick by
/// tick, through a rifle fight). Control: switched on, the hashes part.
#[test]
fn plates_off_is_base() {
    let fight = |edit: &dyn Fn(&mut MatchSetup)| {
        let mut w = flat_world_with(1, |s| {
            s.rules.wounds.enabled = 1;
            edit(s);
        });
        spawn(&mut w, 0, RIFLES, 500, 500);
        spawn(&mut w, 1, US_RIFLES, 560, 500);
        let mut hs = Vec::new();
        for _ in 0..40 {
            steps(&mut w, 30);
            // The state without the setup bytes (the setups differ).
            hs.push(crate::hash::full_hash(&w.state, &w.config_bytes, &[]));
        }
        (hs, s15(&w, 0).plate_hits + s15(&w, 1).plate_hits)
    };
    let (base, _) = fight(&|_| {});
    let (off, n_off) = fight(&|s| plate_all(s, 0, 360));
    assert_eq!(n_off, 0);
    assert_eq!(off, base, "plates in the setup, switch 0: base");
    let (on, n_on) = fight(&|s| plate_all(s, 1, 360));
    assert!(n_on > 0, "control: plates met");
    assert_ne!(on, base, "control: switched on, the match differs");
}

// ------------------------------------------------------------ X1-X3 --

const US_MORTAR: &str = "ww2:usa_mortar_team";
const GER_MORTAR: &str = "ww2:ger_mortar_team";

/// The stand-in Soviet mortar (plan X3): the US 81 mm mortar given the
/// GrW 34's bomb as a German foreign feed at `spread` %, with `[supply]
/// foreign_ammo` 1 (`feed` false: the switch and the feed left off).
fn foreign_setup(s: &mut MatchSetup, feed: bool, spread: u16) {
    if !feed {
        return;
    }
    if let Some(sp) = s.rules.supply.as_mut() {
        sp.foreign_ammo = 1;
    }
    let grw = s.weapons.iter().position(|w| w.id == "ww2:grw34").unwrap();
    let bomb = s.weapons[grw].feed.unwrap();
    let ger = s
        .factions
        .iter()
        .position(|f| f.id == "ww2:germany")
        .unwrap() as u16;
    let m1 = s
        .weapons
        .iter()
        .position(|w| w.id == "ww2:m1_mortar")
        .unwrap();
    s.weapons[m1].foreign_feed = Some(frontline_content::setup::ForeignFeedSetup {
        feed: bomb,
        faction: ger,
        spread_pct: spread,
    });
}

/// Empties every man's bombs and magazine (own and foreign).
fn dry(w: &mut World, q: SquadId) {
    for m in members(w, q) {
        w.edit_soldier(m, |k, c| {
            k.rounds = [0; frontline_content::setup::MAX_FEEDS];
            k.foreign = 0;
            c.mag = 0;
        });
    }
}

/// The man carrying the squad's mortar.
fn tube(w: &World, q: SquadId) -> SimId {
    let g = w.game().unwrap();
    let wi = g.setup().squad_types[g.squads[&q].type_idx as usize].weapon;
    members(w, q)
        .into_iter()
        .find(|m| w.state.entities.kit[m.index()].weapon == wi)
        .unwrap()
}

fn bombs_of(w: &World, q: SquadId) -> (u32, u32) {
    let e = &w.state.entities;
    let g = w.game().unwrap();
    let ty = &g.setup().squad_types[g.squads[&q].type_idx as usize];
    let k = ty
        .feed_slot(&g.setup().weapons[ty.weapon as usize])
        .unwrap();
    members(w, q).iter().fold((0, 0), |(o, f), m| {
        let i = m.index();
        (
            o + e.kit[i].rounds[k] as u32 + e.combat[i].mag as u32,
            f + e.kit[i].foreign as u32,
        )
    })
}

/// A dry mortar squad of player `p` (`card`) at (500, 500) with a full
/// set-down shells crate of faction `crate_faction` held by `p` 3 m from
/// its tube, after `secs` seconds: (own, foreign) bombs and `p`'s
/// counters.
fn draw_at_crate(
    feed: bool,
    p: u8,
    card: &str,
    crate_faction: &str,
    secs: u64,
) -> ((u32, u32), crate::s15::S15Counters) {
    let mut w = flat_world_with(1, |s| foreign_setup(s, feed, 130));
    let q = spawn(&mut w, p, card, 500, 500);
    steps(&mut w, 60);
    dry(&mut w, q);
    let at = w.state.entities.transform[tube(&w, q).index()].pos.xy();
    let s = w.match_setup().unwrap();
    let f = s
        .factions
        .iter()
        .position(|x| x.id == crate_faction)
        .unwrap() as u16;
    let d = s
        .stock_of(
            f,
            frontline_content::setup::StockKind::Crate,
            Some(frontline_content::setup::SupplyKind::Shells),
        )
        .unwrap();
    w.add_crate_now(PlayerId(p), d, at + FxVec2::from_int(3, 0))
        .unwrap();
    steps(&mut w, secs * 30);
    (bombs_of(&w, q), s15(&w, p as usize))
}

/// X1: a dry stand-in Soviet mortar at a German shells crate its player
/// holds draws foreign bombs (no own bomb fits the crate). Control: the
/// same mortar without the foreign feed (switch 0) draws nothing.
#[test]
fn soviet_mortar_draws_german_bombs() {
    let ((own, foreign), k) = draw_at_crate(true, 1, US_MORTAR, "ww2:germany", 30);
    assert_eq!(own, 0);
    assert!(foreign > 0, "foreign bombs drawn");
    assert_eq!(k.foreign_drawn, foreign);
    let ((own, foreign), k) = draw_at_crate(false, 1, US_MORTAR, "ww2:germany", 30);
    assert_eq!((own, foreign, k.foreign_drawn), (0, 0, 0), "control");
}

/// X3: the GrW 34 has no foreign feed: at a US shells crate its player
/// holds it draws nothing (with the stand-in feed on). Control: at a
/// German crate it draws its own bombs.
#[test]
fn german_mortar_draws_no_soviet_bombs() {
    let ((own, foreign), k) = draw_at_crate(true, 0, GER_MORTAR, "ww2:usa", 30);
    assert_eq!((own, foreign, k.foreign_drawn), (0, 0, 0));
    let ((own, _), _) = draw_at_crate(true, 0, GER_MORTAR, "ww2:germany", 30);
    assert!(own > 0, "control: own bombs at an own-faction crate");
}

/// A stand-in Soviet mortar squad of player 1 at (500, 500) with `own`
/// own bombs on its tube man and `foreign` foreign ones.
fn armed_mortar(spread: u16, own: u16, foreign: u16) -> (World, SquadId) {
    let mut w = flat_world_with(1, |s| foreign_setup(s, true, spread));
    let q = spawn(&mut w, 1, US_MORTAR, 500, 500);
    steps(&mut w, 60);
    dry(&mut w, q);
    let t = tube(&w, q);
    let g = w.game().unwrap();
    let ty = &g.setup().squad_types[g.squads[&q].type_idx as usize];
    let k = ty
        .feed_slot(&g.setup().weapons[ty.weapon as usize])
        .unwrap();
    w.edit_soldier(t, |kit, _| {
        kit.rounds[k] = own;
        kit.foreign = foreign;
    });
    (w, q)
}

/// Orders a mission of `n` rounds at (500, 900) for player 1 and runs it
/// out.
fn shoot(w: &mut World, q: SquadId, n: u16) {
    w.step(&[Command {
        player: PlayerId(1),
        seq: 1,
        kind: CommandKind::FireMission {
            squad: q,
            target: FxVec2::from_int(500, 900),
            rounds: n,
        },
    }]);
    let r = w
        .snapshot()
        .fire_mission_refusals
        .iter()
        .find(|e| e.squad == q)
        .map(|e| e.refusal);
    assert_eq!(r, None, "mission accepted");
    super::s11a1_tests::run_out(w, q, 240);
}

/// X2: own bombs go first: a squad with 2 own and 2 foreign fires a
/// 2-round mission from its own (foreign kept, none counted fired); the
/// next 2 rounds are the foreign ones. Control: the counter moves only
/// then.
#[test]
fn own_bombs_first() {
    let (mut w, q) = armed_mortar(130, 2, 2);
    shoot(&mut w, q, 2);
    assert_eq!(bombs_of(&w, q), (0, 2), "own fired first");
    assert_eq!(s15(&w, 1).foreign_fired, 0);
    shoot(&mut w, q, 2);
    assert_eq!(bombs_of(&w, q), (0, 0));
    assert_eq!(s15(&w, 1).foreign_fired, 2, "control: then the foreign");
}

/// X2: the same mission fired with foreign bombs (spread 200 %) misses by
/// more than with own bombs (same aim draws, the spread scaled).
/// Control: at 100 % the miss is the same.
#[test]
fn foreign_bombs_fly_wider() {
    let miss = |spread: u16, foreign: bool| {
        let (own, f) = if foreign { (0, 4) } else { (4, 0) };
        let (mut w, q) = armed_mortar(spread, own, f);
        shoot(&mut w, q, 4);
        let fm = &w.game().unwrap().players[1].fm;
        assert_eq!(fm.bombs.iter().sum::<u32>(), 4, "four fired");
        fm.miss_mm.iter().sum::<u64>()
    };
    let own = miss(200, false);
    assert!(miss(200, true) > own, "foreign wider");
    assert_eq!(miss(100, true), own, "control: 100 %");
}

/// N2 for foreign feeds: with `[supply] foreign_ammo` 0 the setup build
/// drops the feed and a match runs as base. Control: with the feed on, a
/// dry mortar at a foreign crate differs (draws).
#[test]
fn foreign_off_is_base() {
    let run = |feed: bool, off: bool| {
        let mut w = flat_world_with(1, |s| {
            foreign_setup(s, feed, 130);
            if off {
                s.s15_off();
            }
        });
        let q = spawn(&mut w, 1, US_MORTAR, 500, 500);
        steps(&mut w, 60);
        dry(&mut w, q);
        let at = w.state.entities.transform[tube(&w, q).index()].pos.xy();
        let d = w
            .match_setup()
            .unwrap()
            .stock_of(
                0,
                frontline_content::setup::StockKind::Crate,
                Some(frontline_content::setup::SupplyKind::Shells),
            )
            .unwrap();
        w.add_crate_now(PlayerId(1), d, at + FxVec2::from_int(3, 0));
        let mut hs = Vec::new();
        for _ in 0..20 {
            steps(&mut w, 30);
            hs.push(crate::hash::full_hash(&w.state, &w.config_bytes, &[]));
        }
        (
            hs,
            w.match_setup()
                .unwrap()
                .weapons
                .iter()
                .any(|x| x.foreign_feed.is_some()),
        )
    };
    let (base, _) = run(false, false);
    let (off, any) = run(true, true);
    assert!(!any, "s15_off drops the feed");
    assert_eq!(off, base);
    let (on, any) = run(true, false);
    assert!(any);
    assert_ne!(on, base, "control: the feed on draws");
}

// --------------------------------------------------------------- B1 --

/// The stand-in Mosin (plan B1): the Kar 98k with a bayonet block (reach
/// 140 cm), `[melee] bayonets` = `on`.
fn bayonet_setup(s: &mut MatchSetup, on: u32) {
    s.rules.melee.bayonets = on;
    let k = s.weapons.iter().position(|w| w.id == "ww2:kar98k").unwrap();
    let butt = s.weapons[k].melee.unwrap();
    s.weapons[k].bayonet = Some(frontline_content::defs::MeleeDef {
        reach_cm: 140,
        ..butt
    });
}

/// One strike by the German Kar 98k man at a passive US man `dx` cm east
/// (`bayonets` = `on`): whether it hit.
fn bayonet_strike(on: u32, dx: i32) -> bool {
    use super::s14c_tests::{duel, off, tick_held, wind_up};
    let (mut w, a, b) = duel(1, "ww2:kar98k", "ww2:m1_garand", (dx, 0), |s| {
        bayonet_setup(s, on)
    });
    wind_up(&mut w, a, off(300, 0));
    tick_held(&mut w, (a, off(0, 0)), (b, off(dx, 0)), true);
    let k = &w.game().unwrap().players[0].s14;
    assert_eq!(k.strikes, 1, "one strike");
    let b = s15(&w, 0);
    assert_eq!(b.bayonet_strikes, on, "a bayonet strike counted");
    assert_eq!(b.bayonet_hits, on * k.strike_hits);
    k.strike_hits == 1
}

/// B1: the bayonet reaches a man 1.5 m ahead. Control: the butt (no
/// bayonet block on the weapon's strike) reaches 1.0 m only, so at
/// 1.5 m it misses (`bayonets_off_uses_the_butt`).
#[test]
fn bayonet_reaches_150_cm() {
    assert!(bayonet_strike(1, 150));
    assert!(bayonet_strike(1, 100), "and nearer");
}

/// B1: 1.9 m is beyond the bayonet. Control: 1.5 m is hit (above).
#[test]
fn bayonet_misses_at_190_cm() {
    assert!(!bayonet_strike(1, 190));
    assert!(bayonet_strike(1, 150), "control");
}

/// N2 for bayonets: with `bayonets` 0 the weapon strikes with its butt
/// (1.5 m: a miss) and the setup build drops the block. Control: at 1.0 m
/// the butt hits.
#[test]
fn bayonets_off_uses_the_butt() {
    assert!(!bayonet_strike(0, 150));
    assert!(bayonet_strike(0, 90), "control: the butt's reach");
    let mut s = flat_world_with(1, |s| bayonet_setup(s, 1))
        .match_setup()
        .unwrap()
        .clone();
    s.s15_off();
    assert!(s.weapons.iter().all(|w| w.bayonet.is_none()));
}
