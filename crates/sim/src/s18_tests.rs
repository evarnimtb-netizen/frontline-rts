//! S18 Build B tests (docs/slices/S18-online-teams.md 3 H1, 4 "S18b"):
//! the hand-over of a surrendered player's troops to his ally. Each test
//! names its control.

use super::s6_tests::{add_built, add_crate_at, def, members, run, spawn, steps};
use super::s17_tests::team_setup;
use super::*;
use crate::ecs::EntityKind;
use crate::game::{Outcome, SquadId};
use frontline_content::setup::{StockKind, SupplyKind};

const GER: &str = "ww2:ger_grenadiergruppe";
const PAK: &str = "ww2:ger_pak40_squad";

fn game(w: &World) -> &MatchState {
    w.game().unwrap()
}

fn fort(w: &World, id: &str) -> u16 {
    w.match_setup()
        .unwrap()
        .forts
        .iter()
        .position(|f| f.id == id)
        .unwrap() as u16
}

/// What player `p` owns in play, by kind (ids ascending).
#[derive(Debug, Default, PartialEq, Eq)]
struct Owned {
    ents: Vec<(u32, EntityKind)>,
    squads: Vec<u32>,
    structures: Vec<u32>,
    crates: Vec<u32>,
    pieces: Vec<u32>,
    wire: Vec<u32>,
    mines: Vec<u32>,
}

fn owned(w: &World, p: u8) -> Owned {
    let g = game(w);
    let e = w.entities();
    let me = PlayerId(p);
    Owned {
        ents: (0..e.len())
            .filter(|&i| {
                e.owner[i] == me
                    && matches!(
                        e.kind[i],
                        EntityKind::Soldier
                            | EntityKind::Vehicle
                            | EntityKind::Abandoned
                            | EntityKind::Downed
                    )
            })
            .map(|i| (i as u32, e.kind[i]))
            .collect(),
        squads: g
            .squads
            .iter()
            .filter(|(_, q)| q.owner == me)
            .map(|(id, _)| id.0)
            .collect(),
        structures: g
            .supply6()
            .structures
            .iter()
            .filter(|(_, s)| s.owner == me)
            .map(|(&id, _)| id)
            .collect(),
        crates: g
            .supply6()
            .crates
            .iter()
            .filter(|(_, c)| c.owner == me)
            .map(|(&id, _)| id)
            .collect(),
        pieces: g
            .fort()
            .pieces
            .iter()
            .filter(|(_, x)| x.owner == me)
            .map(|(&id, _)| id)
            .collect(),
        wire: g
            .fort()
            .wire
            .iter()
            .filter(|(_, x)| x.owner == me)
            .map(|(&id, _)| id)
            .collect(),
        mines: g
            .fort()
            .mines
            .iter()
            .filter(|(_, x)| x.owner == me)
            .map(|(&id, _)| id)
            .collect(),
    }
}

/// A 2v2 (slot 0 Germany and slot 1 USA on team 0, against slots 2 and 3)
/// where player 0 holds one of every kind H1 hands over: an infantry squad,
/// a served gun, an abandoned gun, a downed man, an HQ, a crate, sandbag
/// pieces, wire and a mine. `handover` and `handover_pop` as given.
fn staged(threads: usize, handover: u32, handover_pop: u32) -> World {
    let s = team_setup(&[0, 0, 1, 1], true, false, |s| {
        s.rules.teams.handover = handover;
        s.rules.teams.handover_pop = handover_pop;
        s.rules.wounds.enabled = 1;
    });
    let mut w = World::new_match(s, threads).unwrap();
    let inf = spawn(&mut w, 0, GER, 400, 400);
    spawn(&mut w, 0, PAK, 420, 400);
    // A second gun whose crew is killed: abandoned after the bail delay.
    let lost = spawn(&mut w, 0, PAK, 440, 420);
    let gun = members(&w, lost)[0];
    steps(&mut w, 2);
    let crew: Vec<SimId> = game(&w).vehicles[&gun]
        .served
        .as_ref()
        .unwrap()
        .slots
        .iter()
        .filter_map(|s| s.man)
        .collect();
    for m in crew {
        assert!(w.kill_soldier(m));
    }
    let hz = w.match_setup().unwrap().tick_hz as u64;
    steps(&mut w, 15 * hz);
    assert_eq!(w.entities().kind[gun.index()], EntityKind::Abandoned);
    // A downed man of the infantry squad.
    let man = members(&w, inf)[3];
    let until = w.tick() + 3000;
    super::s12_tests::stage_downed(&mut w, man, until);
    let f0 = game(&w).setup().players[0].faction;
    let hq = def(&w, f0, StockKind::Hq, None);
    add_built(&mut w, 0, hq, FxVec2::from_int(120, 120), [10, 10, 10]);
    let cr = def(&w, f0, StockKind::Crate, Some(SupplyKind::SmallArms));
    add_crate_at(&mut w, 0, cr, FxVec2::from_int(430, 380));
    let sb = fort(&w, "ww2:sandbag_wall");
    assert!(w.build_now(
        sb,
        &[FxVec2::from_int(450, 450), FxVec2::from_int(456, 450)],
        PlayerId(0)
    ));
    let bw = fort(&w, "ww2:barbed_wire");
    assert!(w.build_now(
        bw,
        &[FxVec2::from_int(460, 470), FxVec2::from_int(480, 470)],
        PlayerId(0)
    ));
    let mf = fort(&w, "ww2:ap_minefield");
    w.lay_mine_now(mf, FxVec2::from_int(500, 520), PlayerId(0))
        .unwrap();
    w.step(&[]);
    w
}

/// Plan 4 S18b, H1: player 0 surrenders and every kind he owns changes
/// owner to his ally (slot 1), in the same tick; his manpower goes with it;
/// the ally records the squads. A save made in that tick loads with the
/// same hash (the ally owning a German HQ and gun is valid), and the
/// handed troops then play on. Control: `handover` 0 leaves all of it with
/// player 0.
#[test]
fn a_surrender_hands_every_kind_to_the_ally() {
    let mut w = staged(1, 1, 0);
    let before = owned(&w, 0);
    assert!(!before.ents.is_empty());
    let kinds: std::collections::BTreeSet<u8> = before.ents.iter().map(|e| e.1 as u8).collect();
    for k in [
        EntityKind::Soldier,
        EntityKind::Vehicle,
        EntityKind::Abandoned,
        EntityKind::Downed,
    ] {
        assert!(kinds.contains(&(k as u8)), "staged no {k:?}");
    }
    for (n, v) in [
        ("squads", &before.squads),
        ("structures", &before.structures),
        ("crates", &before.crates),
        ("pieces", &before.pieces),
        ("wire", &before.wire),
        ("mines", &before.mines),
    ] {
        assert!(!v.is_empty(), "staged no {n}");
    }
    let ally_before = owned(&w, 1);
    let mp = (game(&w).players[0].manpower, game(&w).players[1].manpower);
    let tick = w.tick();
    run(&mut w, 0, 0, CommandKind::Surrender);
    assert_eq!(w.tick(), tick + 1);
    assert_eq!(owned(&w, 0), Owned::default(), "player 0 owns nothing");
    let after = owned(&w, 1);
    let merged = |a: &Vec<u32>, b: &Vec<u32>| {
        let mut v: Vec<u32> = a.iter().chain(b).copied().collect();
        v.sort_unstable();
        v
    };
    assert_eq!(after.squads, merged(&before.squads, &ally_before.squads));
    assert_eq!(after.structures, before.structures);
    assert_eq!(after.crates, before.crates);
    assert_eq!(after.pieces, before.pieces);
    assert_eq!(after.wire, before.wire);
    assert_eq!(after.mines, before.mines);
    for (id, _) in &before.ents {
        assert_eq!(w.entities().owner[*id as usize], PlayerId(1), "entity {id}");
    }
    let g = game(&w);
    assert_eq!(g.players[0].manpower, 0);
    assert_eq!(g.players[1].manpower, mp.0 + mp.1);
    assert_eq!(
        g.players[1].handed,
        before
            .squads
            .iter()
            .map(|&q| SquadId(q))
            .collect::<Vec<_>>()
    );
    assert_eq!(g.players[0].handed_to, Some(PlayerId(1)));
    assert_eq!(g.outcome(), Outcome::Running, "the ally plays on");
    // A mine its new owner knows.
    for id in &before.mines {
        assert_ne!(g.fort().mines[id].known & 0b10, 0);
    }
    // Saved in the hand-over tick: loads, same hash, same bytes again.
    let bytes = w.save();
    let back = World::load(&bytes, 1).expect("the hand-over state loads");
    assert_eq!(back.full_state_hash(), w.full_state_hash());
    assert_eq!(back.state_hash(), w.state_hash());
    assert_eq!(back.save(), bytes);
    // The handed squads take the ally's orders.
    let sq = SquadId(before.squads[0]);
    let n = w.stats().commands_rejected;
    run(
        &mut w,
        1,
        0,
        CommandKind::Move {
            squad: sq,
            target: FxVec2::from_int(300, 300),
        },
    );
    assert_eq!(w.stats().commands_rejected, n, "the ally orders the squad");
    steps(&mut w, 300);
    // Control: with the switch off everything stays with player 0.
    let mut c = staged(1, 0, 0);
    let before = owned(&c, 0);
    run(&mut c, 0, 0, CommandKind::Surrender);
    assert_eq!(owned(&c, 0), before);
    assert!(game(&c).players[1].handed.is_empty());
}

/// S18-6: handed troops never count against the ally's pop cap with
/// `handover_pop` 0, and do with 1. Control: before the hand-over the
/// ally's population is his own.
#[test]
fn handed_troops_do_not_block_the_allys_cap() {
    for pop in [0, 1] {
        let mut w = staged(1, 1, pop);
        let ally = |w: &World| game(w).population(w.entities(), PlayerId(1));
        let giver = game(&w).population(w.entities(), PlayerId(0));
        let own = ally(&w);
        assert!(giver > 0);
        run(&mut w, 0, 0, CommandKind::Surrender);
        if pop == 0 {
            assert_eq!(ally(&w), own, "handed troops are free");
        } else {
            assert_eq!(ally(&w), own + giver, "control: they count");
        }
    }
}

/// H1: the target is the lowest-slot standing ally; with none standing
/// nothing moves and the team is out (S17 T7). Control: a 1v1 surrender
/// hands nothing over.
#[test]
fn with_no_standing_ally_nothing_moves() {
    let mut w = World::new_match(team_setup(&[0, 0, 1, 1], true, false, |_| {}), 1).unwrap();
    for (p, x) in [(0u8, 300), (1, 340), (2, 700), (3, 740)] {
        spawn(&mut w, p, GER, x, 400);
    }
    w.step(&[]);
    assert_eq!(game(&w).handover_target(PlayerId(0)), Some(PlayerId(1)));
    assert_eq!(game(&w).handover_target(PlayerId(3)), Some(PlayerId(2)));
    run(&mut w, 1, 0, CommandKind::Surrender);
    assert_eq!(game(&w).players[1].handed_to, Some(PlayerId(0)));
    assert_eq!(game(&w).handover_target(PlayerId(0)), None);
    let left = owned(&w, 0);
    run(&mut w, 0, 0, CommandKind::Surrender);
    assert_eq!(owned(&w, 0), left, "no ally: his troops hold");
    assert_eq!(game(&w).players[0].handed_to, None);
    assert_eq!(
        game(&w).outcome(),
        Outcome::Won {
            winner: PlayerId(2)
        }
    );
    // Player 1 surrenders again (his own order, then the relay's as he
    // quits): nothing moves again and his hand-over target stays, so the
    // structure check on load still holds.
    run(&mut w, 1, 1, CommandKind::Surrender);
    assert_eq!(game(&w).players[1].handed_to, Some(PlayerId(0)));
    let h = w.state_hash();
    let back = World::load(&w.save(), 1).unwrap();
    assert_eq!(back.state_hash(), h);
    // Control: 1v1.
    let mut w = super::s6_tests::flat_world(1);
    spawn(&mut w, 0, GER, 300, 300);
    w.step(&[]);
    let left = owned(&w, 0);
    run(&mut w, 0, 0, CommandKind::Surrender);
    assert_eq!(owned(&w, 0), left);
}

/// Plan 4 S18b: a 2v2 of AIs (mixed factions) where player 0 surrenders
/// at 1 minute and player 2 at 2 minutes runs 5 minutes: no panic, the
/// AI allies play the handed troops, hashes equal at 1, 2, 3 and 8 threads
/// and through a save and load after the hand-over. Control: the troops
/// changed hands.
#[test]
fn a_2v2_with_hand_overs_agrees_across_threads_and_saves() {
    let s = team_setup(&[0, 0, 1, 1], false, true, |_| {});
    let surrender = |w: &mut World, p: u8| {
        w.step(&[Command {
            player: PlayerId(p),
            seq: 0,
            kind: CommandKind::Surrender,
        }])
    };
    let run_one = |threads: usize| -> (Vec<u64>, World) {
        let mut w = World::new_match(s.clone(), threads).unwrap();
        let mut h = Vec::new();
        while w.tick() < 5 * 60 * 30 {
            match w.tick() {
                1800 => {
                    surrender(&mut w, 0);
                }
                3600 => {
                    surrender(&mut w, 2);
                }
                _ => {
                    w.step(&[]);
                }
            }
            if w.tick().is_multiple_of(30) {
                h.push(w.state_hash());
            }
            if w.tick() == 2400 {
                let back = World::load(&w.save(), threads).unwrap();
                assert_eq!(back.full_state_hash(), w.full_state_hash());
                w = back;
            }
        }
        (h, w)
    };
    let (h1, w1) = run_one(1);
    let g = game(&w1);
    assert_eq!(g.players[0].handed_to, Some(PlayerId(1)));
    assert_eq!(g.players[2].handed_to, Some(PlayerId(3)));
    assert!(!g.players[1].handed.is_empty(), "control: troops changed");
    assert!(
        g.squads.values().all(|q| q.owner.0 == 1 || q.owner.0 == 3),
        "the surrendered own no squad"
    );
    for t in [2, 3, 8] {
        let (h, _) = run_one(t);
        assert_eq!(h, h1, "hashes differ at {t} threads");
    }
}
