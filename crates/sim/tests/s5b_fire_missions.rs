//! S5b Part A: mortar fire missions (docs/slices/S5b-mortars.md targets
//! T2 ranging only on seen bursts, T4 the knowledge rule, T10 the charge
//! solver in the sim, T12 determinism and saves; T3, the bearers' bombs, is
//! in `s5a_ammo.rs`). Flat open ground, both sides human.

use std::path::Path;

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::fire_mission::{mode, phase};
use frontline_sim::{
    Command, CommandKind, FireMissionRefusal, Fx, FxVec2, Mission, Order, PlayerId, SimId, SquadId,
    World,
};

fn setup_with(edit: impl FnOnce(&mut MatchSetup)) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let mut defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let rules = defs
        .add_rules_override(&root.join("tests/rules_test_unlock.toml"))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    req.rules = rules;
    let mut s = defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
    s.players.iter_mut().for_each(|p| p.is_ai = false);
    s.rules.start_manpower = 100_000;
    s.map.terrain.octaves.clear();
    s.map.terrain.bands.clear();
    s.map.woods.clear();
    edit(&mut s);
    s
}

fn xy(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

const EAST: FxVec2 = FxVec2::new(Fx::ONE, Fx::ZERO);
/// Where the mortar stands and what it fires at in most tests: 300 m of
/// flat ground, inside the leader's 400 m observe range.
const TUBE: (i32, i32) = (600, 500);
const TARGET: (i32, i32) = (900, 500);

fn spawn(w: &mut World, player: u8, id: &str, at: FxVec2, f: FxVec2) -> SquadId {
    let t = w.match_setup().unwrap().squad_type_index(id).unwrap();
    w.spawn_squad_at(PlayerId(player), t, at, f).unwrap()
}

fn mortar(w: &mut World) -> SquadId {
    spawn(w, 0, "ww2:ger_mortar_team", xy(TUBE.0, TUBE.1), EAST)
}

/// Orders a mission and returns this tick's refusal, if any.
fn order(w: &mut World, sq: SquadId, target: FxVec2, rounds: u16) -> Option<FireMissionRefusal> {
    w.step(&[Command {
        player: PlayerId(0),
        seq: w.tick() as u32,
        kind: CommandKind::FireMission {
            squad: sq,
            target,
            rounds,
        },
    }]);
    let r = w
        .snapshot()
        .fire_mission_refusals
        .first()
        .map(|e| e.refusal);
    if r.is_some() {
        assert_eq!(w.snapshot().fire_mission_refusals[0].squad, sq);
    }
    r
}

fn mission(w: &World, sq: SquadId) -> Option<Mission> {
    match &w.game().unwrap().squad(sq)?.order {
        Order::FireMission { m, .. } => Some(*m),
        _ => None,
    }
}

fn metres(f: Fx) -> f64 {
    f.raw() as f64 / 65536.0
}

/// Runs until the squad's mission ended, then `after_s` more (its last
/// bombs land).
fn run_out(w: &mut World, sq: SquadId, after_s: u64) {
    let t0 = w.tick();
    while mission(w, sq).is_some() {
        w.step(&[]);
        assert!(w.tick() < t0 + 30 * 300, "the mission never ended");
    }
    for _ in 0..after_s * 30 {
        w.step(&[]);
    }
}

// ------------------------------------------------------------- T2, T10 --

/// T2 and T10: the mortar's own leader sees the target, so the mission is
/// Direct: it starts at 3 % of the range (10 m floor), lays for `direct_s`,
/// fires one adjusting round on charge I (about 14.6 s of flight at 300 m),
/// waits for its burst, which the leader sees, halving the aim error, then
/// fires for effect; each later burst he sees halves it again. The
/// ranging memory keeps the result.
#[test]
fn a_direct_mission_ranges_in_on_the_bursts_its_leader_sees() {
    let mut w = World::new_match(setup_with(|_| {}), 1).unwrap();
    let sq = mortar(&mut w);
    w.enable_burst_log();
    let t0 = w.tick();
    assert_eq!(order(&mut w, sq, xy(TARGET.0, TARGET.1), 6), None);
    let m = mission(&w, sq).expect("accepted");
    assert_eq!(m.mode, mode::DIRECT);
    assert_eq!(m.phase, phase::LAYING);
    assert!(m.observer.is_some());
    let err0 = metres(m.err0);
    assert!((9.9..=10.1).contains(&err0), "first-round error {err0} m");
    // Laying: nothing fired for direct_s (4 s).
    for _ in 0..3 * 30 {
        w.step(&[]);
    }
    assert_eq!(mission(&w, sq).unwrap().fired, 0);
    // The adjusting round: one bomb, then nothing until its burst.
    while mission(&w, sq).unwrap().fired == 0 {
        w.step(&[]);
    }
    let m = mission(&w, sq).unwrap();
    assert_eq!(m.phase, phase::ADJUST);
    assert!(m.wait_burst);
    assert_eq!(m.charge, 0, "charge I at 300 m");
    let flight = m.flight as f64 / 30.0;
    assert!((14.0..15.2).contains(&flight), "flight {flight} s");
    let fired_at = w.tick();
    while w.tick() < fired_at + 12 * 30 {
        w.step(&[]);
        assert_eq!(mission(&w, sq).unwrap().fired, 1, "waits for the burst");
    }
    run_out(&mut w, sq, 25);
    let log = w.take_burst_log();
    assert_eq!(log.len(), 6, "{log:?}");
    let first = log[0];
    assert!(first.live && first.seen);
    // The first burst lands about a flight time after the shot.
    let t = (first.tick - fired_at) as f64 / 30.0;
    assert!(
        (t - flight).abs() < 0.5,
        "burst after {t} s, flight {flight}"
    );
    assert!(first.tick - t0 < 25 * 30);
    // Each seen burst of the live mission halves the error.
    let mut e = err0;
    for b in log.iter().filter(|b| b.live) {
        assert!(b.seen, "{b:?}");
        e /= 2.0;
        assert!(
            (metres(b.offset) - e).abs() < 0.01,
            "{} vs {e}",
            metres(b.offset)
        );
    }
    let g = w.game().unwrap();
    let fm = &g.players()[0].fm;
    assert_eq!(fm.missions[mode::DIRECT as usize], 1);
    assert_eq!(fm.bombs[mode::DIRECT as usize], 6);
    assert_eq!(
        fm.adjust, 1,
        "one adjusting round: 10 m -> 5 m is within ffe_error_m"
    );
    assert_eq!(fm.unseen, 0);
    let cells = &g.squad(sq).unwrap().ranging;
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].target, xy(TARGET.0, TARGET.1));
    assert!(cells[0].seen >= 1);
    // A new mission at the same area starts from the remembered error and
    // fires for effect without an adjusting round.
    assert_eq!(order(&mut w, sq, xy(TARGET.0 + 20, TARGET.1), 2), None);
    let m = mission(&w, sq).unwrap();
    assert!(m.ranged);
    assert!(metres(m.err0) < 5.0);
    run_out(&mut w, sq, 20);
    assert_eq!(w.game().unwrap().players()[0].fm.adjust, 1);
}

/// T2: bursts the observer cannot see (smoke on his sight line, laid once
/// the adjusting round is in the air) teach nothing: the aim error stays
/// where it started and the mission, its observer lost, fires on the last
/// data as map fire.
#[test]
fn hidden_bursts_teach_nothing() {
    let mut w = World::new_match(setup_with(|_| {}), 1).unwrap();
    let sq = mortar(&mut w);
    w.enable_burst_log();
    assert_eq!(order(&mut w, sq, xy(TARGET.0, TARGET.1), 6), None);
    let err0 = mission(&w, sq).unwrap().err0;
    while mission(&w, sq).unwrap().fired == 0 {
        w.step(&[]);
    }
    // A cloud halfway along the sight line, laid again before it thins.
    let mid = xy((TUBE.0 + TARGET.0) / 2, TUBE.1);
    let t0 = w.tick();
    while mission(&w, sq).is_some() || w.tick() < t0 + 60 * 30 {
        if (w.tick() - t0).is_multiple_of(20 * 30) {
            assert!(w.smoke_now(mid));
            assert!(w.smoke_now(mid + xy(0, 6)));
            assert!(w.smoke_now(mid - xy(0, 6)));
        }
        w.step(&[]);
        if let Some(m) = mission(&w, sq) {
            assert_eq!(m.offset.length(), err0, "unseen bursts never range");
            assert_eq!(m.seen, 0);
        }
        assert!(w.tick() < t0 + 300 * 30);
    }
    let log = w.take_burst_log();
    assert_eq!(log.len(), 6);
    assert!(log.iter().all(|b| !b.seen));
    let fm = &w.game().unwrap().players()[0].fm;
    assert_eq!(fm.seen, 0);
    assert!(
        fm.downgrades >= 1,
        "the observer was lost: map fire on the last data"
    );
    assert_eq!(fm.adjust, 1, "the one adjusting round, never seen");
    assert!(
        w.game().unwrap().squad(sq).unwrap().ranging.is_empty(),
        "nothing seen, nothing remembered"
    );
}

// ------------------------------------------------------------------ T4 --

/// T4: no fire into ground the player has never seen. With the leader's
/// observe range cut to 100 m nobody sees the target 300 m off: refused as
/// unknown ground. A trench-marked intel cell, a capture point, and a cell
/// where an enemy was last seen (until it expires) make it known; such a
/// mission is map fire at the 3x error.
#[test]
fn unknown_ground_is_refused_and_known_ground_is_map_fire() {
    let mut w = World::new_match(
        setup_with(|s| s.rules.fire_mission.as_mut().unwrap().observe_leader_m = 100),
        1,
    )
    .unwrap();
    let sq = mortar(&mut w);
    let target = xy(TARGET.0, TARGET.1);
    assert_eq!(
        order(&mut w, sq, target, 6),
        Some(FireMissionRefusal::UnknownGround)
    );
    assert!(mission(&w, sq).is_none());
    assert_eq!(
        w.game().unwrap().players()[0].fm.refused[FireMissionRefusal::UnknownGround as usize],
        1
    );
    // Known: an enemy was seen there in a trench (kept for the match).
    assert!(w.mark_intel(PlayerId(0), target));
    assert_eq!(order(&mut w, sq, target, 1), None);
    let m = mission(&w, sq).unwrap();
    assert_eq!(m.mode, mode::MAP);
    let e = metres(m.err0);
    assert!(
        (26.9..=27.1).contains(&e),
        "map-fire error {e} m (8 % of 300 m, 27 m floor)"
    );
    run_out(&mut w, sq, 20);
    assert_eq!(
        w.game().unwrap().players()[0].fm.adjust,
        0,
        "map fire never adjusts"
    );
    for _ in 0..400 * 30 {
        w.step(&[]);
    }
    assert_eq!(
        order(&mut w, sq, target, 1),
        None,
        "a trench cell stays known"
    );
    run_out(&mut w, sq, 20);

    // Known: a capture point (its area grown by intel_snap_m).
    let cp: FxVec2 = w.match_setup().unwrap().map.points[0].pos.into();
    let tube = cp - xy(300, 0);
    assert!(w.place_squad(sq, w.terrain().clamp(tube), EAST));
    assert_eq!(order(&mut w, sq, cp + xy(0, 30), 1), None);
    assert_eq!(mission(&w, sq).unwrap().mode, mode::MAP);
    run_out(&mut w, sq, 20);
    assert!(w.place_squad(sq, xy(TUBE.0, TUBE.1), EAST));

    // Known while fresh: a cell where an enemy was last seen. A US rifle
    // squad is spotted by a German one 60 m off, then both leave.
    let seen_at = xy(TARGET.0, TARGET.1 + 150);
    let enemy = spawn(&mut w, 1, "ww2:usa_rifle_squad", seen_at, EAST);
    let watcher = spawn(
        &mut w,
        0,
        "ww2:ger_grenadiergruppe",
        seen_at - xy(60, 0),
        EAST,
    );
    for _ in 0..3 * 30 {
        w.step(&[]);
    }
    assert!(
        !w.game().unwrap().players()[0].intel.is_empty(),
        "the sighting is on the map"
    );
    assert!(w.place_squad(enemy, xy(1500, 1500), EAST));
    assert!(w.place_squad(watcher, xy(200, 200), EAST));
    for _ in 0..3 * 30 {
        w.step(&[]);
    }
    assert_eq!(order(&mut w, sq, seen_at, 1), None);
    assert_eq!(mission(&w, sq).unwrap().mode, mode::MAP);
    run_out(&mut w, sq, 20);
    // ... and forgotten after intel_keep_s (300 s).
    for _ in 0..300 * 30 {
        w.step(&[]);
    }
    assert_eq!(
        order(&mut w, sq, seen_at, 1),
        Some(FireMissionRefusal::UnknownGround)
    );
}

/// Review L2: a mission ordered beyond range walks the team to 90 % of the
/// range and fires from there, so its start error is that of 540 m (map
/// fire: 8 % = 43.2 m), not of the 800 m at order time (64 m).
#[test]
fn a_mission_beyond_range_starts_with_the_error_of_its_firing_range() {
    let mut w = World::new_match(
        setup_with(|s| s.rules.fire_mission.as_mut().unwrap().observe_leader_m = 100),
        1,
    )
    .unwrap();
    let sq = mortar(&mut w);
    assert!(w.place_squad(sq, xy(150, TUBE.1), EAST));
    let target = xy(950, TUBE.1);
    assert!(w.mark_intel(PlayerId(0), target));
    assert_eq!(order(&mut w, sq, target, 1), None);
    let m = mission(&w, sq).unwrap();
    assert_eq!(m.mode, mode::MAP);
    let e = metres(m.err0);
    assert!((42.9..=43.5).contains(&e), "start error {e} m");
}

/// The other refusals: inside the minimum range, off the map, no bombs, a
/// squad without a mortar.
#[test]
fn missions_are_refused_with_a_reason() {
    let mut w = World::new_match(setup_with(|_| {}), 1).unwrap();
    let sq = mortar(&mut w);
    assert_eq!(
        order(&mut w, sq, xy(TUBE.0 + 30, TUBE.1), 4),
        Some(FireMissionRefusal::InsideMinimum)
    );
    assert_eq!(
        order(&mut w, sq, xy(-50, TUBE.1), 4),
        Some(FireMissionRefusal::OffMap)
    );
    assert_eq!(
        order(&mut w, sq, xy(TARGET.0, TARGET.1), 0),
        Some(FireMissionRefusal::Rounds)
    );
    let rifles = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", xy(500, 300), EAST);
    assert_eq!(
        order(&mut w, rifles, xy(TARGET.0, TARGET.1), 4),
        Some(FireMissionRefusal::NotMortar)
    );
    let men = w.game().unwrap().squad(sq).unwrap().members.clone();
    for m in men {
        assert!(w.edit_soldier(m, |k, c| {
            k.rounds.iter_mut().for_each(|r| *r = 0);
            c.mag = 0;
        }));
    }
    assert_eq!(
        order(&mut w, sq, xy(TARGET.0, TARGET.1), 4),
        Some(FireMissionRefusal::NoBombs)
    );
}

// --------------------------------------------------- observers and link --

/// The mortar's own leader walks out as a spotter: the mission is Observed
/// (not Direct, he is detached), he is not called back by the order, and
/// beyond the voice link (150 m from the gunner) his corrections no longer
/// reach the tube: the mission is map fire.
#[test]
fn a_detached_leader_spots_within_the_voice_link() {
    let mut w = World::new_match(setup_with(|_| {}), 1).unwrap();
    let sq = mortar(&mut w);
    let leader = {
        let g = w.game().unwrap();
        g.squad_leader(w.entities(), g.squad(sq).unwrap()).unwrap()
    };
    let walk = |w: &mut World, to: FxVec2| {
        w.step(&[Command {
            player: PlayerId(0),
            seq: w.tick() as u32,
            kind: CommandKind::MoveUnits {
                units: vec![leader],
                target: to,
            },
        }]);
        for _ in 0..90 * 30 {
            w.step(&[]);
        }
    };
    walk(&mut w, xy(TUBE.0 + 100, TUBE.1 + 40));
    assert!(w.entities().unit()[leader.index()].detached);
    assert_eq!(order(&mut w, sq, xy(TARGET.0, TARGET.1), 3), None);
    let m = mission(&w, sq).unwrap();
    assert_eq!(m.mode, mode::OBSERVED);
    assert_eq!(m.observer, Some(leader));
    assert!(
        w.entities().unit()[leader.index()].detached,
        "the spotter stays out"
    );
    run_out(&mut w, sq, 20);
    assert!(w.game().unwrap().players()[0].fm.seen >= 1);
    // 250 m out: he still sees the point (known), but out of voice range.
    walk(&mut w, xy(TUBE.0 + 250, TUBE.1 + 40));
    assert_eq!(order(&mut w, sq, xy(TARGET.0 + 50, TARGET.1), 1), None);
    assert_eq!(mission(&w, sq).unwrap().mode, mode::MAP);
}

// ----------------------------------------------------------------- T12 --

fn determinism_world(threads: usize) -> (World, SquadId) {
    let mut w = World::new_match(setup_with(|_| {}), threads).unwrap();
    let sq = mortar(&mut w);
    spawn(
        &mut w,
        1,
        "ww2:usa_rifle_squad",
        xy(TARGET.0, TARGET.1),
        EAST,
    );
    let leader = {
        let g = w.game().unwrap();
        g.squad_leader(w.entities(), g.squad(sq).unwrap()).unwrap()
    };
    w.step(&[Command {
        player: PlayerId(0),
        seq: 1,
        kind: CommandKind::MoveUnits {
            units: vec![leader],
            target: xy(TUBE.0 + 80, TUBE.1 + 30),
        },
    }]);
    for _ in 0..40 * 30 {
        w.step(&[]);
    }
    assert_eq!(order(&mut w, sq, xy(TARGET.0, TARGET.1), 8), None);
    (w, sq)
}

/// T12: a mission with a spotter out gives the same hashes at 1, 2, 3 and
/// 8 threads, and through save, load and continue mid-adjust (a round in
/// the air, `wait_burst` set) and mid-effect.
#[test]
fn fire_missions_are_deterministic_across_threads_and_saves() {
    let ticks = 90 * 30;
    let straight = |threads: usize| {
        let (mut w, _) = determinism_world(threads);
        let mut hashes = Vec::new();
        for t in 0..ticks {
            w.step(&[]);
            if t % 150 == 0 {
                hashes.push(w.state_hash());
            }
        }
        (hashes, w.state_hash())
    };
    let one = straight(1);
    for threads in [2, 3, 8] {
        assert_eq!(straight(threads), one, "{threads} threads");
    }
    // Save mid-adjust and mid-effect, load, continue.
    let (mut w, sq) = determinism_world(2);
    let mut saved_adjust = None;
    let mut saved_effect = None;
    for _ in 0..ticks {
        w.step(&[]);
        let Some(m) = mission(&w, sq) else { continue };
        if saved_adjust.is_none() && m.wait_burst {
            saved_adjust = Some((w.tick(), w.save()));
        }
        if saved_effect.is_none() && m.phase == phase::EFFECT && m.fired >= 2 {
            saved_effect = Some((w.tick(), w.save()));
        }
    }
    let end = w.tick();
    assert_eq!(w.state_hash(), one.1);
    for (what, saved) in [("mid-adjust", saved_adjust), ("mid-effect", saved_effect)] {
        let (at, bytes) = saved.unwrap_or_else(|| panic!("never {what}"));
        let mut l = World::load(&bytes, 3).unwrap();
        assert_eq!(l.tick(), at);
        while l.tick() < end {
            l.step(&[]);
        }
        assert_eq!(l.state_hash(), one.1, "{what}");
    }
}

// -------------------------------------------------------------- legacy --

/// S5b-1 (A): rules without `[fire_mission]` fire a mission as the old
/// barrage: no laying, no aim error, no knowledge rule (any point on the
/// map), no ranging memory and no intel.
#[test]
fn legacy_rules_fire_as_the_old_barrage() {
    let mut w = World::new_match(
        setup_with(|s| {
            s.rules.fire_mission = None;
        }),
        1,
    )
    .unwrap();
    let sq = spawn(&mut w, 0, "ww2:ger_mortar_team", xy(300, 500), EAST);
    // 550 m off, beyond the leader's observe range: accepted all the same.
    assert_eq!(order(&mut w, sq, xy(850, 500), 3), None);
    assert_eq!(mission(&w, sq), Some(Mission::default()));
    w.enable_burst_log();
    run_out(&mut w, sq, 25);
    let g = w.game().unwrap();
    assert_eq!(g.players()[0].fm, Default::default());
    assert!(g.players()[0].intel.is_empty());
    assert!(g.squad(sq).unwrap().ranging.is_empty());
    assert!(
        w.take_burst_log().is_empty(),
        "legacy bombs carry no mission"
    );
    let _ = SimId(0);
}
