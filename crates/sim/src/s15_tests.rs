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
use crate::fx::FxVec3;
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
