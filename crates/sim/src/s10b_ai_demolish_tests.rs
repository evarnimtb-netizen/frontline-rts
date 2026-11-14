//! S10b-ai Build D tests (docs/slices/S10b-ai.md R13, R14, spec 5
//! "Demolish"): the AI blows enemy obstacles in its blocked tank's way
//! until the tank's path goes through, and its demolition jobs end on
//! every path.
//!
//! STAGED SATCHELS (review finding 13, owner question S10b-ai-E3): every
//! test here built on [`blocked`] gives the engineer leader 3 satchels. The
//! engineer cards carry 1, and a 4 m ring opens only after 2 adjacent
//! pieces, so in a match an AI demolition opens one piece, the second
//! Demolish is refused and the job fails (fortify `s10b_ai.demolish`:
//! `pieces_used` 1, `gap_opened_pct` 0). These tests check the job logic
//! with enough satchels, not what a card squad can do.

use super::s6_tests::{run, spawn, steps};
use super::s10b_ai_sweep_tests::{Attack, attack_world};
use super::*;
use crate::ai::{AiHabit, AiState, DriveArgs, HabitEnd};
use crate::game::SquadId;
use frontline_content::ItemKind;

const ENG: &str = "ww2:usa_engineer_squad";
const TANK: &str = "ww2:usa_m4a3_75_squad";
/// The ring's radius and hedgehog step (m). S10b-ai E3: a piece blocks the
/// vehicle nodes within its `piece_half` box (at least 2 m each way, no
/// pad), so a 4 m ring is closed and opens for a medium tank after 2
/// adjacent pieces (about 12 m between the standing centres).
const RING_M: i32 = 20;
const STEP_M: i32 = 4;

/// A blocked tank: AI player 1's M4 150 m short of the point, inside a
/// closed ring of player 0 hedgehogs, ordered 90 m on toward the point (it
/// stops at the ring: no way round). AI engineers (leader STAGED with 3
/// satchels; the card has 1, see the module docs) 40 m aside. `edit` runs last on the setup.
pub(crate) struct Blocked {
    pub w: World,
    pub a: Attack,
    pub tank: SquadId,
    pub eng: SquadId,
    pub ring: Vec<u32>,
}

pub(crate) fn blocked(edit: impl FnOnce(&mut frontline_content::MatchSetup)) -> Blocked {
    let (mut w, a) = attack_world(|s| {
        let k = ItemKind::Satchel.index();
        for t in s.squad_types.iter_mut().filter(|t| t.id == ENG) {
            t.members[0].items[k] = 3;
        }
        edit(s)
    });
    let at = a.short(150);
    let tank = spawn(&mut w, 1, TANK, at.x.floor_int(), at.y.floor_int());
    let v = w.game().unwrap().squads[&tank].members[0];
    let centre = w.state.entities.transform[v.index()].pos.xy();
    let ring = ring(&mut w, centre);
    let e = a.short(150) + a.side * Fx::from_int(40);
    let eng = spawn(&mut w, 1, ENG, e.x.floor_int(), e.y.floor_int());
    run(
        &mut w,
        1,
        1,
        CommandKind::Move {
            squad: tank,
            target: a.short(60),
        },
    );
    let h = hz(&w);
    steps(&mut w, 20 * h);
    Blocked {
        w,
        a,
        tank,
        eng,
        ring,
    }
}

fn hz(w: &World) -> u64 {
    w.match_setup().unwrap().tick_hz as u64
}

/// A closed ring of player 0 hedgehogs around `c`; their piece ids.
fn ring(w: &mut World, c: FxVec2) -> Vec<u32> {
    let f = w
        .match_setup()
        .unwrap()
        .forts
        .iter()
        .position(|f| f.id == "ww2:hedgehog")
        .unwrap() as u16;
    let before: Vec<u32> = w.game().unwrap().fort().pieces.keys().copied().collect();
    // Circumference / step, rounded up.
    let n = (2 * 314 * RING_M) / (100 * STEP_M) + 1;
    for k in 0..n {
        let ang = Fx::from_int(2 * 314 * k) / Fx::from_int(100 * n);
        let (s, co) = (ang.sin(), ang.cos());
        let r = Fx::from_int(RING_M);
        let p = c + FxVec2::new(co * r, s * r);
        let t = FxVec2::new(-s, co);
        assert!(w.build_now(f, &[p, p + t], PlayerId(0)));
    }
    w.game()
        .unwrap()
        .fort()
        .pieces
        .keys()
        .copied()
        .filter(|id| !before.contains(id))
        .collect()
}

fn args(b: &Blocked) -> DriveArgs {
    DriveArgs {
        squad: Some(b.eng),
        point: 0,
        others: vec![b.tank],
        at: None,
    }
}

/// Drives the Demolish habit for up to `ticks`; the end, if any.
fn drive(w: &mut World, ai: &mut AiState, a: &DriveArgs, ticks: u64) -> Option<HabitEnd> {
    (0..ticks).find_map(|_| w.drive_ai_habit(ai, PlayerId(1), AiHabit::Demolish, a))
}

fn tank_pos(w: &World, tank: SquadId) -> FxVec2 {
    let v = w.game().unwrap().squads[&tank].members[0];
    w.state.entities.transform[v.index()].pos.xy()
}

fn charges_laid(w: &World) -> u32 {
    w.game().unwrap().fort().accounts[1].demolitions
}

/// R14 (T106b): the blocked tank's engineers blow the ring piece on its
/// line, then a neighbour, until its path goes through: the job ends
/// `opened` after 2-3 pieces and the tank leaves the ring. Control:
/// `demolish_pieces_max` 1 -> one piece, the path still short, `gave_up`
/// (seen).
#[test]
fn ai_demolishes_until_a_tank_gap_opens() {
    for control in [false, true] {
        let mut b = blocked(|s| {
            if control {
                s.rules.ai.obstacles.demolish_pieces_max = 1;
            }
        });
        let start = tank_pos(&b.w, b.tank);
        let mut ai = AiState::new(b.w.state.tick);
        let args = args(&b);
        let h = hz(&b.w);
        let end = drive(&mut b.w, &mut ai, &args, 900 * h);
        let used = charges_laid(&b.w);
        let c = &ai.counters;
        if control {
            assert_eq!(end, Some(HabitEnd::Failed));
            assert_eq!((used, c.demolish_given_up, c.gaps_opened), (1, 1, 0));
            continue;
        }
        assert_eq!(end, Some(HabitEnd::Done), "{:?}", ai.counters);
        let gone = {
            let g = b.w.game().unwrap();
            b.ring
                .iter()
                .filter(|p| !g.fort().pieces.contains_key(p))
                .count()
        };
        assert!((2..=3).contains(&used), "pieces used {used}, gone {gone}");
        assert_eq!((c.demolitions_started, c.gaps_opened), (1, 1));
        assert!(c.demolish_wanted >= 1);
        drive(&mut b.w, &mut ai, &args, 60 * h);
        let moved = (tank_pos(&b.w, b.tank) - start).length();
        assert!(moved > Fx::from_int(RING_M + 5), "moved {moved:?}");
    }
}

/// Spec 5 "Demolish / target gone": the job's first piece removed (a shell)
/// before its charge: the job skips it, blows the next and still opens.
/// Control: `next_piece` without the standing check -> the `Demolish` at
/// the gone piece is refused and the job fails (seen).
#[test]
fn ai_demolish_skips_gone_piece() {
    let mut b = blocked(|_| {});
    let mut ai = AiState::new(b.w.state.tick);
    let args = args(&b);
    let h = hz(&b.w);
    drive(&mut b.w, &mut ai, &args, 2 * h);
    let first = ai.demolitions[&b.eng].pieces[0].unwrap();
    b.w.edit_fort(|f| {
        f.pieces.remove(&first);
    });
    let end = drive(&mut b.w, &mut ai, &args, 900 * h);
    assert_eq!(end, Some(HabitEnd::Done), "{:?}", ai.counters);
    assert_eq!(ai.counters.s10b_refused, 0);
    assert!(charges_laid(&b.w) >= 1);
}

/// Spec 5 "Demolish / another squad on it": two free engineer squads and
/// one blocked tank: one job only, every plan. Control: no
/// one-job-per-vehicle filter in `demolish_start` and the vehicle kept in
/// the list by `demolish_continue` -> a second job (seen).
#[test]
fn ai_one_demolition_per_vehicle() {
    let mut b = blocked(|_| {});
    let e = b.a.short(150) - b.a.side * Fx::from_int(40);
    let eng2 = spawn(&mut b.w, 1, ENG, e.x.floor_int(), e.y.floor_int());
    let mut ai = AiState::new(b.w.state.tick);
    let mut args = args(&b);
    args.others.push(eng2);
    let h = hz(&b.w);
    for _ in 0..120 {
        drive(&mut b.w, &mut ai, &args, h);
        assert!(ai.demolitions.len() <= 1, "{:?}", ai.demolitions);
    }
    assert_eq!(ai.counters.demolitions_started, 1);
}

/// Spec 5 "Demolish / squad dead or vehicle lost": the engineers killed,
/// or the tank wrecked, mid-job end it with no refusal. Control: one
/// engineer killed (not the satchel man) -> the job lives (seen).
#[test]
fn ai_demolish_ends_when_vehicle_or_squad_lost() {
    for case in 0..3 {
        let mut b = blocked(|_| {});
        let mut ai = AiState::new(b.w.state.tick);
        let args = args(&b);
        let h = hz(&b.w);
        drive(&mut b.w, &mut ai, &args, 2 * h);
        assert!(ai.demolitions.contains_key(&b.eng));
        let men = b.w.game().unwrap().squads[&b.eng].members.clone();
        match case {
            0 => {
                for m in men {
                    assert!(b.w.kill_soldier(m));
                }
            }
            1 => {
                let v = b.w.game().unwrap().squads[&b.tank].members[0];
                let t = b.w.state.tick;
                assert!(b.w.edit_vehicle(v, |s| s.wreck_tick = Some(t)));
            }
            _ => assert!(b.w.kill_soldier(*men.last().unwrap())),
        }
        drive(&mut b.w, &mut ai, &args, 5 * h);
        if case == 2 {
            assert!(ai.demolitions.contains_key(&b.eng), "control");
            continue;
        }
        assert!(!ai.demolitions.contains_key(&b.eng), "case {case}");
        assert!(ai.refused.is_empty());
        assert_eq!(ai.counters.s10b_refused, 0);
    }
}

/// Spec 5 "Demolish / vehicle in footprint" and "save/load mid-job": the
/// own tank stands beside the piece when its charge bursts; it takes no
/// harm (S10b R7), and a save loaded mid-job runs on the same. Control:
/// `demolish` 0 -> no charge, the ring stands whole (seen).
#[test]
fn ai_demolish_charge_spares_own_tank() {
    for control in [false, true] {
        let mut b = blocked(|s| {
            if control {
                s.rules.ai.obstacles.demolish = 0;
            }
        });
        let v = b.w.game().unwrap().squads[&b.tank].members[0];
        let before = b.w.game().unwrap().vehicles[&v].clone();
        let mut ai = AiState::new(b.w.state.tick);
        let args = args(&b);
        let h = hz(&b.w);
        let ran = drive(&mut b.w, &mut ai, &args, 20 * h);
        if !control {
            assert!(ran.is_none() && ai.demolitions.contains_key(&b.eng));
            let mut w2 = World::load(&b.w.save(), 1).unwrap();
            let mut ai2 = ai.clone();
            drive(&mut b.w, &mut ai, &args, 60 * h);
            drive(&mut w2, &mut ai2, &args, 60 * h);
            assert_eq!(b.w.state_hash(), w2.state_hash());
            assert_eq!(ai, ai2);
        }
        // Until the first charge has burst (or 400 s).
        for _ in 0..400 {
            drive(&mut b.w, &mut ai, &args, h);
            let g = b.w.game().unwrap();
            if charges_laid(&b.w) > 0 && g.fort().charges.is_empty() {
                break;
            }
        }
        let g = b.w.game().unwrap();
        let standing = b
            .ring
            .iter()
            .filter(|p| g.fort().pieces.contains_key(p))
            .count();
        if control {
            assert_eq!(standing, b.ring.len());
            assert_eq!(charges_laid(&b.w), 0);
            continue;
        }
        assert!(standing < b.ring.len());
        let after = &g.vehicles[&v];
        assert!(!after.immobile(&g.setup.vehicles[after.type_idx as usize]));
        assert_eq!(after.components, before.components, "own tank harmed");
        assert_eq!(after.seats, before.seats, "crew harmed");
    }
}
