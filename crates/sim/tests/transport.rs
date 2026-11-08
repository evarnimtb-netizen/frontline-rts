//! S8a Build B (`docs/slices/S8a-halftracks.md` 5, 9, 10): riding
//! half-tracks. Boarding at the door, riders in their slots, the mounted
//! squad's orders, door and side exits, the forced dismount, no capture
//! while aboard, the riding state across save, load and threads.

use std::path::Path;

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::game::Order;
use frontline_sim::vehicle::Unload;
use frontline_sim::{Command, CommandKind, Fx, FxVec2, PlayerId, SimId, SquadId, World};

fn setup(seed: u64) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let mut defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let mut req = MatchRequest::skirmish(seed, "ww2:germany", "ww2:usa", Difficulty::Normal);
    req.rules = defs
        .add_rules_override(&root.join("tests/rules_test_unlock.toml"))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut s = defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
    s.players.iter_mut().for_each(|p| p.is_ai = false);
    s.rules.start_manpower = 100_000;
    s.map.terrain.octaves.clear();
    s.map.terrain.bands.clear();
    s.map.woods.clear();
    s
}

fn xy(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

const EAST: FxVec2 = FxVec2::new(Fx::ONE, Fx::ZERO);
const WEST: FxVec2 = FxVec2::new(Fx::NEG_ONE, Fx::ZERO);

fn spawn(w: &mut World, player: u8, card: &str, at: FxVec2, f: FxVec2) -> (SquadId, SimId) {
    let t = w.match_setup().unwrap().squad_type_index(card).unwrap();
    let sq = w.spawn_squad_at(PlayerId(player), t, at, f).unwrap();
    (sq, w.game().unwrap().squad(sq).unwrap().members[0])
}

fn cmd(player: u8, seq: u32, kind: CommandKind) -> Command {
    Command {
        player: PlayerId(player),
        seq,
        kind,
    }
}

/// Steps one tick with `c`; whether the sim applied it.
fn accepts(w: &mut World, c: Command) -> bool {
    let before = w.stats().commands_applied;
    w.step(&[c]);
    w.stats().commands_applied > before
}

fn members(w: &World, sq: SquadId) -> Vec<SimId> {
    w.game()
        .unwrap()
        .squad(sq)
        .map_or_else(Vec::new, |s| s.members.clone())
}

fn riders(w: &World, v: SimId) -> usize {
    w.game().unwrap().riders_of(v).len()
}

/// A 251 at (500, 500) facing east and a ten-man Grenadiergruppe just
/// behind its rear door.
fn loaded_world(seed: u64) -> (World, SimId, SquadId, SquadId) {
    let mut w = World::new_match(setup(seed), 1).unwrap();
    let (vsq, v) = spawn(&mut w, 0, "ww2:ger_sdkfz251_1_squad", xy(500, 500), EAST);
    let (inf, _) = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", xy(488, 500), EAST);
    (w, v, vsq, inf)
}

/// Embark and step until everyone is aboard (at most 60 s); returns the
/// ticks at which the rider count rose.
fn board_all(w: &mut World, v: SimId, inf: SquadId) -> Vec<u64> {
    let n = members(w, inf).len();
    w.step(&[cmd(
        0,
        1,
        CommandKind::Embark {
            squads: vec![inf],
            vehicle: v,
        },
    )]);
    let mut ups = Vec::new();
    let mut last = 0;
    for _ in 0..30 * 60 {
        w.step(&[]);
        let r = riders(w, v);
        if r > last {
            ups.push(w.tick());
            last = r;
        }
        if r == n {
            break;
        }
    }
    ups
}

#[test]
fn a_squad_boards_one_man_per_board_time_and_rides_in_its_slots() {
    let (mut w, v, vsq, inf) = loaded_world(1);
    let ups = board_all(&mut w, v, inf);
    assert_eq!(ups.len(), 10, "all ten aboard: {ups:?}");
    let board = w
        .match_setup()
        .unwrap()
        .vehicles
        .iter()
        .find(|x| x.id == "ww2:ger_sdkfz251_1")
        .unwrap()
        .transport
        .as_ref()
        .unwrap()
        .board_ticks as u64;
    for p in ups.windows(2) {
        assert!(p[1] - p[0] >= board, "one man per board time: {ups:?}");
    }
    let g = w.game().unwrap();
    assert!(g.squad_rides(w.entities(), inf));
    assert_eq!(g.squad(inf).unwrap().order, Order::Embark { vehicle: v });
    // A move order to the mounted squad is ignored; the vehicle drives.
    w.step(&[cmd(
        0,
        2,
        CommandKind::Move {
            squad: inf,
            target: xy(400, 400),
        },
    )]);
    assert_eq!(
        w.game().unwrap().squad(inf).unwrap().order,
        Order::Embark { vehicle: v }
    );
    w.step(&[cmd(
        0,
        3,
        CommandKind::Move {
            squad: vsq,
            target: xy(560, 500),
        },
    )]);
    for _ in 0..30 * 15 {
        w.step(&[]);
    }
    let vp = w.entities().transform()[v.index()].pos;
    assert!(vp.x > Fx::from_int(530), "it drove: {vp:?}");
    for m in members(&w, inf) {
        let p = w.entities().transform()[m.index()].pos;
        let d = (p.xy() - vp.xy()).length();
        assert!(d < Fx::from_int(4), "rider {m:?} rides along ({d:?})");
        assert!(p.z > vp.z + Fx::ONE, "seated on the bench, not the ground");
    }
    // No capture or soldier orders while aboard: soldier commands drop him.
    let man = members(&w, inf)[0];
    assert!(!accepts(
        &mut w,
        cmd(
            0,
            4,
            CommandKind::MoveUnits {
                units: vec![man],
                target: xy(100, 100),
            },
        )
    ));
}

#[test]
fn a_squad_that_does_not_fit_is_refused() {
    let mut w = World::new_match(setup(2), 1).unwrap();
    let (_, v) = spawn(&mut w, 1, "ww2:usa_m3a1_squad", xy(500, 500), WEST);
    let (big, _) = spawn(&mut w, 1, "ww2:usa_rifle_squad", xy(512, 500), WEST);
    let (small, _) = spawn(&mut w, 1, "ww2:usa_rifle_squad_9", xy(512, 510), WEST);
    assert_eq!(members(&w, big).len(), 12);
    // S8b: capacity 12, so the full 12-man squad fits alone (S8a-2 (B)); the
    // refusal is for a pair that together exceeds the seats.
    assert!(accepts(
        &mut w,
        cmd(
            1,
            2,
            CommandKind::Embark {
                squads: vec![big, small],
                vehicle: v,
            },
        )
    ));
    let g = w.game().unwrap();
    assert_eq!(g.squad(big).unwrap().order, Order::Embark { vehicle: v });
    assert_ne!(g.squad(small).unwrap().order, Order::Embark { vehicle: v });
}

/// Ticks from the order until the last man is out, and where they stand.
fn unload(seed: u64, over_sides: bool) -> (u64, Vec<FxVec2>, FxVec2) {
    let (mut w, v, _, inf) = loaded_world(seed);
    board_all(&mut w, v, inf);
    let t0 = w.tick();
    w.step(&[cmd(
        0,
        9,
        CommandKind::Disembark {
            vehicles: vec![v],
            over_sides,
        },
    )]);
    while riders(&w, v) > 0 && w.tick() < t0 + 30 * 30 {
        w.step(&[]);
    }
    let g = w.game().unwrap();
    assert_eq!(g.squad(inf).unwrap().order, Order::Idle, "the squad holds");
    let spots = members(&w, inf)
        .iter()
        .map(|m| w.entities().transform()[m.index()].pos.xy())
        .collect();
    (
        w.tick() - t0,
        spots,
        w.entities().transform()[v.index()].pos.xy(),
    )
}

#[test]
fn men_get_out_faster_over_the_sides_than_by_the_door() {
    let (door, at_door, vp) = unload(3, false);
    let (sides, at_sides, _) = unload(3, true);
    assert!(sides < door, "sides {sides} < door {door}");
    // 10 x 0.8 s and 10 x 0.5 s at 30 Hz.
    assert!((235..=250).contains(&door), "door {door}");
    assert!((145..=160).contains(&sides), "sides {sides}");
    // Door: behind the hull; sides: beside it.
    assert!(at_door.iter().all(|p| p.x < vp.x - Fx::from_int(2)));
    // Sides: beside the hull, on the left (no round has struck it).
    assert!(
        at_sides.iter().all(|p| p.y > vp.y + Fx::from_ratio(1, 2)),
        "{at_sides:?} {vp:?}"
    );
}

#[test]
fn a_knocked_out_transport_drops_everyone_at_once_stunned() {
    let (mut w, v, _, inf) = loaded_world(4);
    board_all(&mut w, v, inf);
    assert!(w.wreck_vehicle(v));
    w.step(&[]);
    assert_eq!(riders(&w, v), 0);
    let g = w.game().unwrap();
    assert_eq!(g.squad(inf).unwrap().order, Order::Idle);
    for m in members(&w, inf) {
        let i = m.index();
        assert!(w.entities().unit()[i].riding.is_none());
        assert!(w.entities().weapon()[i].cooldown > 0, "stunned");
        assert!(w.entities().combat()[i].suppression > 0);
    }
}

#[test]
fn riders_neither_capture_nor_contest() {
    let mut w = World::new_match(setup(5), 1).unwrap();
    let p = FxVec2::from(w.match_setup().unwrap().map.points[2].pos);
    let (_, v) = spawn(&mut w, 0, "ww2:ger_sdkfz251_1_squad", p + xy(30, 0), EAST);
    let (inf, _) = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", p + xy(18, 0), EAST);
    board_all(&mut w, v, inf);
    w.place_vehicle(v, p, Fx::ZERO);
    for _ in 0..30 * 5 {
        w.step(&[]);
    }
    let progress = |w: &World| w.game().unwrap().points()[2].progress;
    assert_eq!(progress(&w), 0, "nobody on foot at the point");
    w.step(&[cmd(
        0,
        9,
        CommandKind::Disembark {
            vehicles: vec![v],
            over_sides: true,
        },
    )]);
    for _ in 0..30 * 10 {
        w.step(&[]);
    }
    let pt = &w.game().unwrap().points()[2];
    assert!(
        pt.progress > 0 || pt.owner == Some(PlayerId(0)),
        "the men on foot capture: {pt:?}"
    );
}

#[test]
fn riding_state_is_saved_hashed_and_checked_on_load() {
    let (mut w, v, _, inf) = loaded_world(6);
    board_all(&mut w, v, inf);
    let base = w.full_state_hash();
    let loaded = World::load(&w.save(), 2).unwrap();
    assert_eq!(loaded.full_state_hash(), base);
    assert_eq!(loaded.state(), w.state());
    let man = members(&w, inf)[3];
    type Edit = Box<dyn Fn(&mut World) -> bool>;
    let edits: Vec<(&str, Edit)> = vec![
        (
            "riding.slot",
            Box::new(move |w| {
                w.edit_unit(man, |u| {
                    u.riding.as_mut().unwrap().slot ^= 1;
                })
            }),
        ),
        (
            "riders",
            Box::new(move |w| w.edit_vehicle(v, |vs| vs.riders.swap(0, 1))),
        ),
        (
            "door_ticks",
            Box::new(move |w| w.edit_vehicle(v, |vs| vs.door_ticks = 3)),
        ),
        (
            "unload",
            Box::new(move |w| w.edit_vehicle(v, |vs| vs.unload = Some(Unload::default()))),
        ),
        (
            "trip",
            Box::new(move |w| w.edit_vehicle(v, |vs| vs.trip = None)),
        ),
        (
            "s8a",
            Box::new(|w| w.edit_player(PlayerId(0), |p| p.s8a.rides += 1)),
        ),
    ];
    for (name, edit) in &edits {
        let mut e = World::load(&w.save(), 1).unwrap();
        assert!(edit(&mut e), "{name}");
        assert_ne!(e.full_state_hash(), base, "{name} is not hashed");
        let round = World::load(&e.save(), 1);
        match *name {
            // A slot that names someone else, or a rider his slot does not.
            "riding.slot" | "riders" => assert!(round.is_err(), "{name} accepted"),
            _ => assert_eq!(round.unwrap().full_state_hash(), e.full_state_hash()),
        }
    }
    let mut bad = World::load(&w.save(), 1).unwrap();
    bad.edit_vehicle(v, |vs| vs.door_ticks = u16::MAX);
    assert!(World::load(&bad.save(), 1).is_err(), "door timer too long");
    let mut bad = World::load(&w.save(), 1).unwrap();
    bad.edit_vehicle(v, |vs| vs.riders.push(None));
    assert!(World::load(&bad.save(), 1).is_err(), "slot count");
    let mut bad = World::load(&w.save(), 1).unwrap();
    bad.edit_unit(man, |u| u.riding = None);
    assert!(
        World::load(&bad.save(), 1).is_err(),
        "a slot names a man on foot"
    );
}

/// Embark, drive toward an enemy squad, unload under fire: the same at
/// 1/2/3/8 threads and across a save, load and continue while embarked.
#[test]
fn riding_is_deterministic_across_threads_and_save_load() {
    let start = || {
        let (mut w, v, vsq, inf) = loaded_world(7);
        spawn(&mut w, 1, "ww2:usa_rifle_squad", xy(650, 520), WEST);
        (w, v, vsq, inf)
    };
    let script = |t: u64, v: SimId, vsq: SquadId, inf: SquadId| -> Vec<Command> {
        match t {
            1 => vec![cmd(
                0,
                1,
                CommandKind::Embark {
                    squads: vec![inf],
                    vehicle: v,
                },
            )],
            400 => vec![cmd(
                0,
                2,
                CommandKind::Move {
                    squad: vsq,
                    target: xy(580, 505),
                },
            )],
            800 => vec![cmd(
                0,
                3,
                CommandKind::Disembark {
                    vehicles: vec![v],
                    over_sides: true,
                },
            )],
            _ => Vec::new(),
        }
    };
    let run = |threads: usize, split: Option<u64>| {
        let (w0, v, vsq, inf) = start();
        let mut w = World::load(&w0.save(), threads).unwrap();
        let mut hashes = Vec::new();
        let mut rode = 0usize;
        for t in 0..1200u64 {
            if Some(t) == split {
                w = World::load(&w.save(), threads).unwrap();
            }
            w.step(&script(t, v, vsq, inf));
            rode = rode.max(riders(&w, v));
            if t % 20 == 0 {
                hashes.push(w.state_hash());
            }
        }
        (hashes, w.full_state_hash(), rode)
    };
    let base = run(1, None);
    assert!(base.2 >= 8, "the squad rode: {}", base.2);
    for threads in [2, 3, 8] {
        assert_eq!(run(threads, None), base, "{threads} threads");
    }
    assert_eq!(run(3, Some(600)), base, "save/load/continue while embarked");
}

/// S8a AI (`ai::transport`): in an AI-vs-AI match a side buys a
/// half-track, loads a squad, rides and unloads it, and its transport moves
/// do not loop (few failed moves, almost no refused commands). Scans seeds
/// 1-4 for a match where a ride happens by 15 minutes. Slow in debug: run
/// with `--release -- --include-ignored`.
#[test]
#[ignore]
fn the_ai_rides_its_half_tracks_without_move_loops() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let defs = ModDefs::load(&root).unwrap();
    let mut found = None;
    for seed in 1..=4u64 {
        let req = MatchRequest::skirmish(seed, "ww2:germany", "ww2:usa", Difficulty::Normal);
        let mut s = defs.resolve(&req).unwrap();
        s.players.iter_mut().for_each(|p| p.is_ai = true);
        let mut w = World::new_match(s, 2).unwrap();
        let mut rode = [false; 2];
        for _ in 0..30 * 60 * 15 {
            w.step(&[]);
            if w.tick().is_multiple_of(30) {
                let g = w.game().unwrap();
                for (id, vs) in g.vehicles() {
                    if vs.riders.iter().any(|r| r.is_some()) {
                        rode[w.entities().owner()[id.index()].0 as usize] = true;
                    }
                }
            }
        }
        let g = w.game().unwrap();
        let rides: u32 = g.players().iter().map(|p| p.s8a.rides).sum();
        if rides == 0 {
            continue;
        }
        for p in 0..2u8 {
            let ai = g.ai(PlayerId(p)).unwrap();
            let c = &ai.counters;
            assert!(
                c.transport_fails <= 2 * c.transport_rides + 4,
                "seed {seed} player {p}: {} failed moves for {} rides",
                c.transport_fails,
                c.transport_rides
            );
            assert!(
                ai.rejected * 50 <= ai.issued,
                "seed {seed} player {p}: {} of {} refused",
                ai.rejected,
                ai.issued
            );
        }
        // Review M2: the AI measured the rides (order to the point).
        let measured: u32 = (0..2u8)
            .map(|p| {
                let ai = g.ai(PlayerId(p)).unwrap();
                let c = &ai.counters;
                c.obj_rides + c.obj_lost + ai.ride_meas.values().filter(|m| m.rode).count() as u32
            })
            .sum();
        assert!(measured > 0, "seed {seed}: rides but no ride measurement");
        found = Some((seed, rode, rides));
        break;
    }
    let (_, rode, rides) = found.expect("no AI ride in seeds 1-4");
    assert!(rides > 0 && rode.iter().any(|&r| r));
}

/// S8b: recon and truck AI in an AI-vs-AI match: both are bought, the moves
/// do not loop, almost nothing is refused; the whole match (AI jobs, drops,
/// Meet phases, ride locks) is deterministic at 1, 2, 3 and 8 threads and
/// across a save and load. Slow in debug: run with `--release --
/// --include-ignored`. Scans seeds 1-3 for a match with a truck drop.
#[test]
#[ignore]
fn s8b_ai_recon_and_trucks_are_deterministic_and_do_not_loop() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let defs = ModDefs::load(&root).unwrap();
    let ticks = 30 * 60 * 12u64;
    let run = |seed: u64, threads: usize, split: Option<u64>| {
        let req = MatchRequest::skirmish(seed, "ww2:germany", "ww2:usa", Difficulty::Normal);
        let mut s = defs.resolve(&req).unwrap();
        s.players.iter_mut().for_each(|p| p.is_ai = true);
        let mut w = World::new_match(s, threads).unwrap();
        for t in 0..ticks {
            if Some(t) == split {
                w = World::load(&w.save(), threads).unwrap();
            }
            w.step(&[]);
        }
        w
    };
    let mut found = None;
    for seed in 1..=3u64 {
        let w = run(seed, 1, None);
        let g = w.game().unwrap();
        let (mut drops, mut cars) = (0u32, 0u32);
        for p in 0..2u8 {
            let ai = g.ai(PlayerId(p)).unwrap();
            let c = &ai.counters;
            cars += c.recon_stops;
            drops += c.truck_drops_ordered;
            assert!(
                c.truck_fails + c.recon_fails <= 20,
                "seed {seed} p{p}: fails"
            );
            assert!(
                ai.rejected * 50 <= ai.issued,
                "seed {seed} p{p}: {} of {} refused",
                ai.rejected,
                ai.issued
            );
        }
        if drops > 0 && cars > 0 {
            found = Some((seed, w.full_state_hash()));
            break;
        }
    }
    let (seed, base) = found.expect("no truck drop and recon stop in seeds 1-3");
    for threads in [2, 3, 8] {
        assert_eq!(
            run(seed, threads, None).full_state_hash(),
            base,
            "{threads} threads"
        );
    }
    assert_eq!(
        run(seed, 3, Some(ticks / 2)).full_state_hash(),
        base,
        "save/load/continue"
    );
}

/// S8a Fix (review L3): the refusals, one case per row. Each starts from
/// `loaded_world` (a 251 with a Grenadiergruppe behind it), sets the case
/// up and sends one command that the sim must not apply.
#[test]
fn transport_orders_that_must_be_refused() {
    type Setup = Box<dyn Fn(&mut World, SimId, SquadId) -> Command>;
    let embark = |inf: SquadId, v: SimId| {
        cmd(
            0,
            9,
            CommandKind::Embark {
                squads: vec![inf],
                vehicle: v,
            },
        )
    };
    let cases: Vec<(&str, Setup)> = vec![
        (
            "Embark into an enemy vehicle",
            Box::new(move |w, _, inf| {
                let (_, e) = spawn(w, 1, "ww2:usa_m3a1_squad", xy(470, 500), EAST);
                embark(inf, e)
            }),
        ),
        (
            "Embark into a wreck",
            Box::new(move |w, v, inf| {
                assert!(w.wreck_vehicle(v));
                w.step(&[]);
                embark(inf, v)
            }),
        ),
        (
            "Embark into a burning vehicle",
            Box::new(move |w, v, inf| {
                assert!(w.edit_vehicle(v, |vs| vs.burn = 30));
                embark(inf, v)
            }),
        ),
        (
            "Embark into an unloading vehicle",
            Box::new(move |w, v, inf| {
                board_all(w, v, inf);
                assert!(accepts(
                    w,
                    cmd(
                        0,
                        2,
                        CommandKind::Disembark {
                            vehicles: vec![v],
                            over_sides: false,
                        },
                    )
                ));
                assert!(w.game().unwrap().vehicle(v).unwrap().unload.is_some());
                let (other, _) = spawn(w, 0, "ww2:ger_grenadiergruppe", xy(480, 510), EAST);
                embark(other, v)
            }),
        ),
        (
            "Disembark of an empty vehicle",
            Box::new(move |_, v, _| {
                cmd(
                    0,
                    9,
                    CommandKind::Disembark {
                        vehicles: vec![v],
                        over_sides: true,
                    },
                )
            }),
        ),
        (
            "Move to a mounted squad",
            Box::new(move |w, v, inf| {
                board_all(w, v, inf);
                cmd(
                    0,
                    9,
                    CommandKind::Move {
                        squad: inf,
                        target: xy(600, 600),
                    },
                )
            }),
        ),
        (
            "Retreat to a mounted squad",
            Box::new(move |w, v, inf| {
                board_all(w, v, inf);
                cmd(0, 9, CommandKind::Retreat { squad: inf })
            }),
        ),
    ];
    // Control: the same command into its own idle vehicle is applied.
    let (mut w, v, _, inf) = loaded_world(7);
    assert!(accepts(&mut w, embark(inf, v)), "control refused");
    for (name, case) in &cases {
        let (mut w, v, _, inf) = loaded_world(7);
        let c = case(&mut w, v, inf);
        assert!(!accepts(&mut w, c), "{name}: accepted");
    }
}
