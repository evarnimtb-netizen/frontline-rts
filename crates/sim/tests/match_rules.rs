//! Match rules (S1 stage 4): capture timing, contested and empty points,
//! income, ticket drain in proportion to the point difference, and the three
//! ways to lose. Both sides are driven by hand (AI off); squads are put on
//! points with the `place_squad` setup tool.

use std::path::Path;

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::{Command, CommandKind, Defeat, Fx, FxVec2, Outcome, PlayerId, SquadId, World};

fn setup(seed: u64) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let mut s = defs
        .resolve(&MatchRequest::skirmish(
            seed,
            "ww2:germany",
            "ww2:usa",
            Difficulty::Normal,
        ))
        .unwrap_or_else(|e| panic!("{e}"));
    s.players.iter_mut().for_each(|p| p.is_ai = false);
    s
}

fn squad(w: &World, player: u8, ty: &str) -> SquadId {
    let t = w.match_setup().unwrap().squad_type_index(ty).unwrap();
    *w.game()
        .unwrap()
        .squads()
        .iter()
        .find(|(_, s)| s.owner == PlayerId(player) && s.type_idx == t)
        .unwrap()
        .0
}

fn point(w: &World, name: &str) -> (usize, FxVec2) {
    let pts = &w.match_setup().unwrap().map.points;
    let k = pts.iter().position(|p| p.name == name).unwrap();
    (k, pts[k].pos.into())
}

fn place(w: &mut World, id: SquadId, at: FxVec2) {
    assert!(w.place_squad(id, at, FxVec2::new(Fx::ONE, Fx::ZERO)));
}

fn steps(w: &mut World, n: u32) {
    for _ in 0..n {
        w.step(&[]);
    }
}

#[test]
fn capture_takes_20_s_alone_and_less_with_more_men() {
    let mut s = setup(1);
    // A one-man engineer squad, so the base rate shows.
    let eng = s.squad_type_index("ww2:ger_engineer_squad").unwrap();
    // S4: one man is one member slot of one.
    let t = &mut s.squad_types[eng as usize];
    t.men = 1;
    t.members.truncate(1);
    t.members[0].count = 1;
    let mut w = World::new_match(s, 2).unwrap();
    let (b, b_pos) = point(&w, "B");
    let (c, c_pos) = point(&w, "C");
    let one = squad(&w, 0, "ww2:ger_engineer_squad");
    let eight = squad(&w, 0, "ww2:ger_grenadiergruppe");
    place(&mut w, one, b_pos);
    place(&mut w, eight, c_pos);

    // Eight men capture at the cap (4 men: 250 % speed): 240 ticks.
    steps(&mut w, 239);
    let g = w.game().unwrap();
    assert_eq!(g.points()[c].owner, None);
    assert_eq!(g.points()[c].capturer, Some(PlayerId(0)));
    assert_eq!(g.points()[c].progress, 239 * 250);
    assert_eq!(w.snapshot().points[c].progress_pct, 99);
    steps(&mut w, 1);
    let g = w.game().unwrap();
    assert_eq!(g.points()[c].owner, Some(PlayerId(0)));
    assert_eq!((g.points()[c].capturer, g.points()[c].progress), (None, 0));
    // One man takes the full 20 s.
    assert_eq!(g.points()[b].owner, None);
    steps(&mut w, 600 - 240 - 1);
    assert_eq!(w.game().unwrap().points()[b].owner, None);
    steps(&mut w, 1);
    assert_eq!(w.game().unwrap().points()[b].owner, Some(PlayerId(0)));
    assert_eq!(w.tick(), 600);

    // An enemy squad alone on an owned point flips it in one capture time.
    let us = squad(&w, 1, "ww2:usa_rifle_squad");
    place(&mut w, one, FxVec2::from_int(96, 96));
    place(&mut w, us, b_pos);
    steps(&mut w, 239);
    assert_eq!(w.game().unwrap().points()[b].owner, Some(PlayerId(0)));
    steps(&mut w, 1);
    assert_eq!(w.game().unwrap().points()[b].owner, Some(PlayerId(1)));
    let snap = w.snapshot();
    assert_eq!(snap.points[b].owner, Some(1));
    assert_eq!(snap.points[c].owner, Some(0));
    assert_eq!(snap.players[0].points_held, 1);
    assert_eq!(snap.players[1].points_held, 1);
}

#[test]
fn contested_points_freeze_and_empty_points_fade() {
    let mut w = World::new_match(setup(2), 2).unwrap();
    let (b, b_pos) = point(&w, "B");
    let ger = squad(&w, 0, "ww2:ger_grenadiergruppe");
    let us = squad(&w, 1, "ww2:usa_rifle_squad");
    place(&mut w, ger, b_pos);
    steps(&mut w, 100);
    let p = w.game().unwrap().points()[b].progress;
    assert_eq!(p, 100 * 250);

    // Both sides on the point: frozen (whatever the shooting does).
    place(&mut w, us, b_pos + FxVec2::from_int(0, 6));
    steps(&mut w, 30);
    let c = &w.game().unwrap().points()[b];
    assert!(c.contested && w.snapshot().points[b].contested);
    assert_eq!((c.progress, c.capturer), (p, Some(PlayerId(0))));

    // The enemy leaves: capture resumes.
    place(&mut w, us, FxVec2::from_int(928, 928));
    steps(&mut w, 1);
    let c = &w.game().unwrap().points()[b];
    assert!(!c.contested && c.progress > p);

    // Everyone leaves: progress fades at the one-man rate, then the
    // capturer is forgotten.
    place(&mut w, ger, FxVec2::from_int(96, 96));
    let p = w.game().unwrap().points()[b].progress;
    steps(&mut w, 10);
    assert_eq!(w.game().unwrap().points()[b].progress, p - 10 * 100);
    steps(&mut w, p / 100);
    let c = &w.game().unwrap().points()[b];
    assert_eq!((c.progress, c.capturer, c.owner), (0, None, None));
}

#[test]
fn income_accrues_every_tick_and_grows_with_points() {
    let mut w = World::new_match(setup(3), 1).unwrap();
    steps(&mut w, 900);
    // 240 per minute: 120 in 30 s, exactly (fractions carried).
    assert_eq!(w.game().unwrap().players()[0].manpower, 600 + 120);
    assert_eq!(w.snapshot().players[0].income_per_min, 240);
    let (_, c_pos) = point(&w, "C");
    let ger = squad(&w, 0, "ww2:ger_grenadiergruppe");
    place(&mut w, ger, c_pos);
    steps(&mut w, 240);
    // C is worth 2: 240 + 2 × 60.
    let g = w.game().unwrap();
    assert_eq!(g.income_per_min(PlayerId(0)), 360);
    assert_eq!(g.income_per_min(PlayerId(1)), 240);
    assert_eq!(w.snapshot().players[0].income_per_min, 360);

    // AI players' income follows their difficulty.
    let mut s = setup(3);
    s.players[1].is_ai = true;
    for (d, want) in [
        (Difficulty::Easy, 192),
        (Difficulty::Normal, 240),
        (Difficulty::Hard, 300),
    ] {
        s.players[1].difficulty = d;
        let w = World::new_match(s.clone(), 1).unwrap();
        let g = w.game().unwrap();
        assert_eq!(g.income_per_min(PlayerId(1)), want, "{d:?}");
        assert_eq!(g.income_per_min(PlayerId(0)), 240);
    }
}

#[test]
fn tickets_drain_in_proportion_to_the_point_difference() {
    let mut w = World::new_match(setup(4), 2).unwrap();
    let (_, b_pos) = point(&w, "B");
    let (_, c_pos) = point(&w, "C");
    let squads: Vec<SquadId> = w
        .game()
        .unwrap()
        .squads()
        .iter()
        .filter(|(_, s)| s.owner == PlayerId(0))
        .map(|(id, _)| *id)
        .collect();
    place(&mut w, squads[0], b_pos);
    place(&mut w, squads[1], c_pos);
    steps(&mut w, 239);
    assert_eq!(w.game().unwrap().drain_per_min(PlayerId(1)), 0);
    // Both flip on this tick: B (1) + C (2) = 3 points of difference.
    steps(&mut w, 1);
    let g = w.game().unwrap();
    assert_eq!(g.held_value(PlayerId(0)), 3);
    assert_eq!(g.drain_per_min(PlayerId(1)), 3 * 20);
    assert_eq!(g.drain_per_min(PlayerId(0)), 0);
    assert_eq!(g.players()[1].tickets, 500);
    // 60 tickets per minute, counted from the flip tick.
    steps(&mut w, 1799);
    let g = w.game().unwrap();
    assert_eq!(g.players()[1].tickets, 440);
    assert_eq!(g.players()[0].tickets, 500);
    assert_eq!(w.snapshot().players[1].drain_per_min, 60);
    // Time holding points: two points for 1800 ticks.
    assert_eq!(g.players()[0].point_ticks, 2 * 1800);
    assert_eq!(w.snapshot().players[0].hold_s, 120);

    // Half the difference, half the drain.
    place(&mut w, squads[1], FxVec2::from_int(96, 96));
    let us = squad(&w, 1, "ww2:usa_rifle_squad");
    place(&mut w, us, c_pos);
    steps(&mut w, 240);
    let g = w.game().unwrap();
    assert_eq!(g.points()[2].owner, Some(PlayerId(1)));
    assert_eq!(g.drain_per_min(PlayerId(0)), 20);
    assert_eq!(g.drain_per_min(PlayerId(1)), 0);
}

#[test]
fn running_out_of_tickets_loses_and_ends_the_match() {
    let mut s = setup(5);
    s.rules.tickets = 2;
    let mut w = World::new_match(s, 1).unwrap();
    let (_, c_pos) = point(&w, "C");
    let ger = squad(&w, 0, "ww2:ger_grenadiergruppe");
    place(&mut w, ger, c_pos);
    // Flip at 240, then 40 tickets a minute: one per 45 ticks.
    steps(&mut w, 240 + 45);
    assert_eq!(w.game().unwrap().players()[1].tickets, 1);
    assert_eq!(w.game().unwrap().outcome(), Outcome::Running);
    steps(&mut w, 45);
    let g = w.game().unwrap();
    assert_eq!(g.players()[1].tickets, 0);
    assert_eq!(g.players()[1].defeat, Some(Defeat::Tickets));
    assert_eq!(
        g.outcome(),
        Outcome::Won {
            winner: PlayerId(0)
        }
    );
    let snap = w.snapshot();
    assert_eq!(snap.winner, Some(0));
    assert_eq!(snap.players[1].defeat, Some(Defeat::Tickets));
    // Nothing moves on after the end: orders are refused, tickets and
    // manpower stay put.
    let mp = g.players()[0].manpower;
    let rejected = w.stats().commands_rejected;
    w.step(&[Command {
        player: PlayerId(0),
        seq: 0,
        kind: CommandKind::Stop { squad: ger },
    }]);
    assert_eq!(w.stats().commands_rejected, rejected + 1);
    steps(&mut w, 100);
    assert_eq!(w.game().unwrap().players()[0].manpower, mp);
}

#[test]
fn no_soldiers_and_no_manpower_loses() {
    let usa = |s: &MatchSetup| s.players[1].faction as usize;
    // No soldiers, but manpower to buy: still in.
    let mut s = setup(6);
    let f = usa(&s);
    s.factions[f].start_squads.clear();
    let mut w = World::new_match(s.clone(), 1).unwrap();
    steps(&mut w, 30);
    assert_eq!(w.game().unwrap().outcome(), Outcome::Running);
    assert_eq!(w.game().unwrap().players()[1].defeat, None);

    // No soldiers and nothing to buy with: out at once.
    s.rules.start_manpower = 0;
    let mut w = World::new_match(s.clone(), 1).unwrap();
    w.step(&[]);
    let g = w.game().unwrap();
    assert_eq!(g.players()[1].defeat, Some(Defeat::Wiped));
    assert_eq!(
        g.outcome(),
        Outcome::Won {
            winner: PlayerId(0)
        }
    );

    // Both sides wiped in the same tick: a draw.
    for f in &mut s.factions {
        f.start_squads.clear();
    }
    let mut w = World::new_match(s, 1).unwrap();
    w.step(&[]);
    assert_eq!(w.game().unwrap().outcome(), Outcome::Draw);
    assert_eq!(w.snapshot().outcome, Outcome::Draw);
    assert_eq!(w.snapshot().winner, None);

    // Surrender is a defeat too.
    let mut w = World::new_match(setup(6), 1).unwrap();
    w.step(&[Command {
        player: PlayerId(1),
        seq: 0,
        kind: CommandKind::Surrender,
    }]);
    let g = w.game().unwrap();
    assert_eq!(g.players()[1].defeat, Some(Defeat::Surrendered));
    assert!(g.players()[1].surrendered);
}

#[test]
fn rules_state_survives_save_load_mid_capture() {
    let run = |w: &mut World, until: u64| {
        let mut out = Vec::new();
        while w.tick() < until {
            if let Some(h) = w.step(&[]) {
                out.push((w.tick(), h));
            }
        }
        out
    };
    let start = |threads| {
        let mut w = World::new_match(setup(7), threads).unwrap();
        let (_, c_pos) = point(&w, "C");
        let (_, b_pos) = point(&w, "B");
        let ger = squad(&w, 0, "ww2:ger_grenadiergruppe");
        let eng = squad(&w, 0, "ww2:ger_engineer_squad");
        place(&mut w, ger, c_pos);
        place(&mut w, eng, b_pos);
        w
    };
    let mut straight = start(1);
    let reference = run(&mut straight, 2000);
    let mut first = start(2);
    run(&mut first, 120);
    let mut resumed = World::load(&first.save(), 3).unwrap();
    assert_eq!(resumed.state(), first.state());
    assert_eq!(resumed.full_state_hash(), first.state_hash());
    let tail: Vec<_> = reference.into_iter().filter(|(t, _)| *t > 120).collect();
    assert_eq!(run(&mut resumed, 2000), tail);
    assert_eq!(resumed.full_state_hash(), resumed.state_hash());
    assert_eq!(resumed.save(), straight.save());
}
