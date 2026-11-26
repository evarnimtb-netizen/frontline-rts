//! S15a Build A tests (docs/slices/S15-ussr.md 3, 5): the SAVE 34 layout
//! and its load checks, then the rules R (riders), P (plates), X (captured
//! bombs) and B (bayonets). Every test turns its switch on in the test
//! only, with test-made stand-in defs (no USSR def in the mod yet). Each
//! test names its control: the edit that makes it fail.

use frontline_content::MatchSetup;
use frontline_content::vehicle::{BoxSetup, TransportSetup};

use super::s6_tests::{flat_world_with, members, spawn, steps};
use super::*;
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
