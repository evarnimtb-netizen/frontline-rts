//! S12b Build C tests (docs/slices/S12-wounds-medics.md 5 A1-A5, liveness,
//! 6): the medic AI (`ai::medic`). Player 0 (Germany) is an AI that buys
//! nothing from its mix (every card `ai_weight` 0 but the medic card's);
//! `[wounds] enabled`, `reinforce` and every `[ai.medic]` switch are on in
//! the test only. Each test names its control: the edit that makes it
//! fail.

use super::s6_tests::{add_built, def, flat_world_with, members, run, spawn, steps};
use super::s12_tests::stage_downed;
use super::*;
use crate::ecs::EntityKind;
use crate::unit::UnitOrder;
use crate::wounds::WOUNDED;
use frontline_content::setup::StockKind;

const RIFLES: &str = "ww2:ger_grenadiergruppe";
const ENEMY: &str = "ww2:usa_rifle_squad";
const MEDICS: &str = "ww2:ger_sanitaetstrupp";

/// The medic AI world: `edit` changes the setup last.
fn med_world(edit: impl FnOnce(&mut frontline_content::MatchSetup)) -> World {
    let mut w = flat_world_with(1, |s| {
        s.players[0].is_ai = true;
        s.rules.wounds.enabled = 1;
        s.rules.wounds.reinforce = 1;
        let m = &mut s.rules.ai.medic;
        m.buy = 1;
        m.field = 1;
        m.post = 1;
        m.reinforce = 1;
        s.rules.start_manpower = 4000;
        for t in s.squad_types.iter_mut() {
            if t.id == MEDICS {
                t.ai_weight = 10;
                t.initial_cooldown_ticks = 0;
            } else {
                t.ai_weight = 0;
            }
        }
        edit(s);
    });
    w.state.game.as_mut().unwrap().players[0].manpower = 3000;
    w
}

fn counters(w: &World) -> crate::ai::AiCounters {
    w.game().unwrap().ai[0].as_ref().unwrap().counters.clone()
}

fn pos(w: &World, m: SimId) -> FxVec2 {
    w.state.entities.transform[m.index()].pos.xy()
}

/// The own base's centre and the unit vector from it to the map centre.
fn axis(w: &World) -> (FxVec2, FxVec2) {
    let b = w.game().unwrap().base_of(PlayerId(0));
    (b, (FxVec2::from_int(512, 512) - b).normalize_or_zero())
}

/// The point `k` metres from the own base toward the map centre.
fn out(w: &World, k: i32) -> FxVec2 {
    let (b, d) = axis(w);
    b + d * Fx::from_int(k)
}

fn at(w: &mut World, p: u8, card: &str, q: FxVec2) -> SquadId {
    spawn(w, p, card, q.x.floor_int(), q.y.floor_int())
}

fn atk(w: &mut World, p: u8, card: &str, k: i32) -> SquadId {
    let q = out(w, k);
    at(w, p, card, q)
}

fn downk(w: &mut World, sq: SquadId, n: usize, k: i32, bleed_s: u64) -> SimId {
    let q = out(w, k);
    down_at(w, sq, n, q, bleed_s)
}

fn own_of(w: &World, id: &str) -> Vec<SquadId> {
    let s = w.match_setup().unwrap();
    w.game()
        .unwrap()
        .squads
        .iter()
        .filter(|(_, q)| q.owner == PlayerId(0) && s.squad_types[q.type_idx as usize].id == id)
        .map(|(&id, _)| id)
        .collect()
}

fn cas(w: &World, m: SimId) -> Option<crate::wounds::Casualty> {
    w.game().unwrap().casualties.get(&m).cloned()
}

fn detached(w: &World, m: SimId) -> bool {
    w.state.entities.unit[m.index()].detached
}

/// Stages a downed man (bleeding out in `bleed_s`; 0: stabilised) from
/// squad `sq`'s member `k`, moved to `q`.
fn down_at(w: &mut World, sq: SquadId, k: usize, q: FxVec2, bleed_s: u64) -> SimId {
    let m = members(w, sq)[k];
    w.state.entities.transform[m.index()].pos = q.extend(Fx::ZERO);
    let t = w.tick();
    stage_downed(w, m, t + bleed_s.max(1) * 30);
    if bleed_s == 0 {
        w.state
            .game
            .as_mut()
            .unwrap()
            .casualties
            .get_mut(&m)
            .unwrap()
            .bleed_out_at = 0;
    }
    m
}

fn aid_post(w: &mut World, q: FxVec2) -> u32 {
    let d = def(w, 0, StockKind::Aid, None);
    add_built(w, 0, d, q, [0; 3])
}

fn post_of(w: &World) -> Option<crate::structures::Structure> {
    let g = w.game().unwrap();
    g.supply6
        .structures
        .values()
        .find(|st| {
            st.owner == PlayerId(0) && g.setup().stocks[st.def as usize].kind == StockKind::Aid
        })
        .cloned()
}

/// A1: with four infantry squads alive and three own men down, the AI
/// buys one medic team. Controls: two men down (no buy); `buy` 0 (no buy).
#[test]
fn a1_buys_a_team_after_three_downs() {
    let run_with = |downs: usize, buy: u32| {
        let mut w = med_world(|s| s.rules.ai.medic.buy = buy);
        let sqs: Vec<SquadId> = (0..4)
            .map(|k| atk(&mut w, 0, RIFLES, 30 + 10 * k))
            .collect();
        steps(&mut w, 2);
        for (k, &sq) in sqs.iter().enumerate().take(downs) {
            let q = out(&w, 40 + 5 * k as i32);
            down_at(&mut w, sq, 3, q, 170);
        }
        steps(&mut w, 30 * 10);
        (counters(&w).medic_buys, own_of(&w, MEDICS).len())
    };
    assert_eq!(run_with(3, 1), (1, 1), "one team");
    assert_eq!(run_with(2, 1), (0, 0), "control: two downs");
    assert_eq!(run_with(3, 0), (0, 0), "control: buy 0");
}

/// A1: a second team once the first staffs a post (men wait there).
/// Control: nobody waits at the post, so the first team does not staff
/// it: no second buy.
#[test]
fn a1_second_team_once_the_first_staffs_a_post() {
    let run_with = |patient: bool| {
        let mut w = med_world(|_| {});
        for k in 0..4 {
            atk(&mut w, 0, RIFLES, 20 + 8 * k);
        }
        let p = out(&w, 90);
        aid_post(&mut w, p);
        at(&mut w, 0, MEDICS, p);
        let sq = at(&mut w, 0, RIFLES, p + FxVec2::from_int(3, 0));
        steps(&mut w, 30 * 3);
        if patient {
            let m = members(&w, sq)[5];
            w.edit_soldier(m, |_, c| {
                c.hp = 40;
                c.wound.state = WOUNDED;
                c.wound.bandaged = true;
            });
            run(&mut w, 0, 9000, CommandKind::StopUnits { units: vec![m] });
        }
        steps(&mut w, 30 * 8);
        counters(&w).medic_buys
    };
    assert_eq!(run_with(true), 1, "the second team");
    assert_eq!(run_with(false), 0, "control: no one waits");
}

/// A2: a free team builds the post in the own HQ's radius on its base
/// side. Control: without an HQ it goes on the own base's edge toward the
/// nearest non-own point.
#[test]
fn a2_post_at_the_hq_else_at_the_base_edge() {
    for hq in [true, false] {
        let mut w = med_world(|_| {});
        let hq_at = out(&w, 200);
        if hq {
            let d = def(&w, 0, StockKind::Hq, None);
            add_built(&mut w, 0, d, hq_at, [0; 3]);
        }
        atk(&mut w, 0, MEDICS, 20);
        steps(&mut w, 30 * 6);
        assert_eq!(counters(&w).post_builds, 1, "hq {hq}");
        let p = post_of(&w).expect("a post");
        let (b, _) = axis(&w);
        let s = w.match_setup().unwrap();
        if hq {
            let r: Fx = s.stocks[def(&w, 0, StockKind::Hq, None) as usize]
                .radius
                .into();
            assert!((p.pos - hq_at).length() <= r, "in the HQ radius");
            assert!(
                (p.pos - b).length() < (hq_at - b).length(),
                "toward the base"
            );
        } else {
            let r: Fx = s.map.bases[s.players[0].base as usize].radius.into();
            let d = (p.pos - b).length() - r;
            assert!(d.abs() <= Fx::from_int(2), "on the base edge: {d:?}");
        }
    }
}

/// A3: a free team sends a medic to a downed man 60 m away (beyond the
/// sim's own 25 m auto-tend) with no enemy near him; he is stabilised.
/// Control: a spotted enemy squad 25 m from him: the team holds
/// (`medic_holds`), no tend.
#[test]
fn a3_tends_only_with_no_enemy_near() {
    for enemy in [false, true] {
        let mut w = med_world(|s| s.rules.ai.medic.post = 0);
        let sq = atk(&mut w, 0, RIFLES, 150);
        atk(&mut w, 0, MEDICS, 90);
        steps(&mut w, 2);
        let d = downk(&mut w, sq, 3, 150, 170);
        if enemy {
            let e = out(&w, 175);
            at(&mut w, 1, ENEMY, e);
        }
        steps(&mut w, 30 * 6);
        let k = counters(&w);
        if enemy {
            assert_eq!(k.medic_tends, 0, "control: enemy near");
            assert!(k.medic_holds > 0);
        } else {
            assert!(k.medic_tends >= 1);
            steps(&mut w, 30 * 60);
            assert_eq!(cas(&w, d).map(|c| c.bleed_out_at), Some(0), "stabilised");
        }
    }
}

/// A3: a free team carries a stabilised man to the post (both men; the
/// post then evacuates him). Control: another own man bleeds 20 m from
/// him: the team tends that man first, no carry.
#[test]
fn a3_carries_a_stabilised_man_to_the_post() {
    for bleeding in [false, true] {
        let mut w = med_world(|s| s.rules.ai.medic.buy = 0);
        let p = out(&w, 40);
        aid_post(&mut w, p);
        let sq = atk(&mut w, 0, RIFLES, 160);
        let mt = atk(&mut w, 0, MEDICS, 110);
        steps(&mut w, 2);
        let d = downk(&mut w, sq, 3, 150, 0);
        if bleeding {
            let q = out(&w, 150) + FxVec2::from_int(0, 20);
            down_at(&mut w, sq, 4, q, 170);
        }
        steps(&mut w, 30 * 6);
        let k = counters(&w);
        if bleeding {
            assert_eq!(
                (k.medic_carries, k.medic_tends),
                (0, 1),
                "control: tend first"
            );
            continue;
        }
        assert_eq!(k.medic_carries, 1);
        let c = cas(&w, d).expect("downed");
        let men = members(&w, mt);
        assert!(c.carriers.iter().flatten().all(|m| men.contains(m)));
        assert!(c.carriers[1].is_some(), "a stretcher");
        // Carried to the post; a staffed post evacuates him.
        let mut gone = false;
        for _ in 0..(30 * 240) {
            w.step(&[]);
            if cas(&w, d).is_none() {
                gone = true;
                break;
            }
        }
        assert!(gone, "evacuated");
        assert_eq!(w.game().unwrap().players[0].s12.evacuated, 1);
    }
}

/// A3: a free team follows behind the lead squad of the assault (the
/// AI's own capture or attack squads). Control: `field` 0: it stays.
#[test]
fn a3_follows_behind_the_assault() {
    for field in [1, 0] {
        let mut w = med_world(|s| {
            s.rules.ai.medic.field = field;
            s.rules.ai.medic.post = 0;
            s.rules.ai.medic.buy = 0;
        });
        for k in 0..3 {
            atk(&mut w, 0, RIFLES, 20 + 8 * k);
        }
        let mt = atk(&mut w, 0, MEDICS, 10);
        steps(&mut w, 2);
        let from = pos(&w, members(&w, mt)[0]);
        steps(&mut w, 30 * 60);
        let moved = (pos(&w, members(&w, mt)[0]) - from).length();
        if field == 1 {
            assert!(moved > Fx::from_int(40), "it followed: {moved:?}");
        } else {
            assert!(moved < Fx::from_int(10), "control: it stayed: {moved:?}");
        }
    }
}

/// A4 and liveness ("no team waits on an empty post"): a wounded man
/// waits halted in the post's radius; the nearest free team walks there,
/// staffs it and he is treated; once nobody waits the team leaves.
/// Control: `post` 0: no team comes, he is not treated.
#[test]
fn a4_staffs_a_post_while_men_wait_then_leaves() {
    for post in [1, 0] {
        let mut w = med_world(|s| {
            s.rules.ai.medic.post = post;
            s.rules.ai.medic.buy = 0;
        });
        let p = out(&w, 60);
        let pid = aid_post(&mut w, p);
        for k in 0..3 {
            atk(&mut w, 0, RIFLES, 20 + 8 * k);
        }
        let mt = atk(&mut w, 0, MEDICS, 140);
        let sq = at(&mut w, 0, RIFLES, p + FxVec2::from_int(4, 0));
        // The AI's first orders (its squads attack) come first.
        steps(&mut w, 30 * 3);
        let m = members(&w, sq)[5];
        w.edit_soldier(m, |_, c| {
            c.hp = 40;
            c.wound.state = WOUNDED;
            c.wound.bandaged = true;
        });
        run(&mut w, 0, 9000, CommandKind::StopUnits { units: vec![m] });
        let staffed = |w: &World| {
            let g = w.game().unwrap();
            g.post_staff(&w.state.entities, &g.supply6.structures[&pid]) >= 2
        };
        let mut was_staffed = false;
        for _ in 0..(30 * 150) {
            w.step(&[]);
            was_staffed |= staffed(&w);
            if w.game().unwrap().players[0].s12.treated > 0 {
                break;
            }
        }
        if post == 0 {
            assert!(!was_staffed, "control: no team");
            assert_eq!(w.game().unwrap().players[0].s12.treated, 0);
            continue;
        }
        assert!(was_staffed, "staffed");
        assert_eq!(w.game().unwrap().players[0].s12.treated, 1, "treated");
        // Nobody waits: the team does not stay.
        steps(&mut w, 30 * 30);
        let r: Fx = w.match_setup().unwrap().stocks[def(&w, 0, StockKind::Aid, None) as usize]
            .radius
            .into();
        let lead = members(&w, mt)[0];
        let leaving = match w.game().unwrap().squads[&mt].order {
            crate::game::Order::Move { target } => (target - p).length() > r,
            _ => (pos(&w, lead) - p).length() > r,
        };
        assert!(leaving, "no team waits on an empty post");
    }
}

/// A4: a retreating squad sends its untreated wounded men to the post on
/// its way. Control: no post: they stay in the squad.
#[test]
fn a4_retreating_squad_sends_its_wounded_to_the_post() {
    for with_post in [true, false] {
        let mut w = med_world(|s| s.rules.ai.medic.buy = 0);
        let p = out(&w, 120);
        if with_post {
            aid_post(&mut w, p);
        }
        let sq = atk(&mut w, 0, RIFLES, 260);
        steps(&mut w, 2);
        let hurt: Vec<SimId> = members(&w, sq)[4..6].to_vec();
        for &m in &hurt {
            w.edit_soldier(m, |_, c| {
                c.hp = 40;
                c.wound.state = WOUNDED;
                c.wound.bandaged = true;
            });
        }
        steps(&mut w, 30 * 3);
        run(&mut w, 0, 9000, CommandKind::Retreat { squad: sq });
        steps(&mut w, 30 * 8);
        for &m in &hurt {
            let u = &w.state.entities.unit[m.index()];
            if with_post {
                assert!(u.detached, "sent off");
                assert_eq!(u.order, UnitOrder::Move { target: p }, "to the post");
            } else {
                assert!(!u.detached, "control: no post");
            }
        }
    }
}

/// A5: a squad at half strength in the base is reinforced (its gaps
/// filled from the base) and a man standing halted there is folded in.
/// Controls: the same squad out of the base; `reinforce` 0.
#[test]
fn a5_reinforces_a_weak_squad_in_base() {
    let run_with = |dist: i32, sw: u32| {
        let mut w = med_world(|s| {
            s.rules.ai.medic.reinforce = sw;
            s.rules.ai.medic.buy = 0;
        });
        let sq = atk(&mut w, 0, RIFLES, dist);
        steps(&mut w, 2);
        let ms = members(&w, sq);
        for &m in &ms[5..] {
            let g = w.state.game.as_mut().unwrap();
            g.kill(&mut w.state.entities, &mut w.hash, m, 30);
        }
        steps(&mut w, 30 * 6);
        let k = counters(&w).reinforce_orders;
        (k, members(&w, sq).len())
    };
    let (k, n) = run_with(5, 1);
    assert_eq!(k, 1);
    assert_eq!(n, 10, "filled");
    assert_eq!(run_with(300, 1), (0, 5), "control: out of the base");
    assert_eq!(run_with(5, 0), (0, 5), "control: reinforce 0");
}

/// A5: a man standing halted, detached, in the base (as a reinforcement
/// that walked to his squad) is folded in by the AI's `Regroup` while the
/// squad is back in the base. Control: `reinforce` 0 sends none.
#[test]
fn a5_folds_a_halted_reinforcement_in() {
    for sw in [1, 0] {
        let mut w = med_world(|s| {
            s.rules.ai.medic.reinforce = sw;
            s.rules.ai.medic.buy = 0;
        });
        let sq = atk(&mut w, 0, RIFLES, 5);
        steps(&mut w, 2);
        // Back to the base, out of the AI's attack orders.
        steps(&mut w, 30 * 3);
        run(&mut w, 0, 8999, CommandKind::Retreat { squad: sq });
        let m = members(&w, sq)[9];
        run(&mut w, 0, 9000, CommandKind::StopUnits { units: vec![m] });
        assert!(detached(&w, m));
        // (The AI's attack orders fold him in later too: read its Regroup.)
        let mut regroup = false;
        for _ in 0..(30 * 6) {
            w.step(&[]);
            let ai = w.game().unwrap().ai[0].as_ref().unwrap();
            regroup |= ai
                .pending
                .iter()
                .any(|c| c.kind == CommandKind::Regroup { squad: sq });
        }
        assert_eq!(regroup, sw == 1, "reinforce {sw}");
    }
}

/// Liveness (plan 6 "carrier hit: set down"): a carry set down leaves the
/// other carrier halted, detached, by a stabilised man: the AI regroups
/// the team. Control: an AI with every medic switch off leaves him.
#[test]
fn l_team_released_when_its_carry_ends() {
    for on in [true, false] {
        let mut w = med_world(|s| {
            let m = &mut s.rules.ai.medic;
            m.buy = 0;
            m.field = 0;
            m.post = on as u32;
            m.reinforce = 0;
        });
        let sq = atk(&mut w, 0, RIFLES, 160);
        let mt = atk(&mut w, 0, MEDICS, 150);
        steps(&mut w, 2);
        let d = downk(&mut w, sq, 3, 152, 0);
        let men = members(&w, mt);
        let to = out(&w, 10);
        run(
            &mut w,
            0,
            9000,
            CommandKind::Carry {
                units: men.clone(),
                target: d,
                to,
            },
        );
        // Both lift him, then the first is hit: set down.
        for _ in 0..(30 * 30) {
            w.step(&[]);
            if cas(&w, d).is_some_and(|c| {
                c.dest.is_some_and(|q| {
                    w.state.entities.unit[men[0].index()].order == UnitOrder::Move { target: q }
                })
            }) {
                break;
            }
        }
        w.edit_soldier(men[0], |_, c| {
            c.hp = 40;
            c.wound.state = WOUNDED;
        });
        steps(&mut w, 3);
        assert!(cas(&w, d).unwrap().carriers[0].is_none(), "set down");
        assert!(detached(&w, men[1]), "the other carrier halted");
        steps(&mut w, 30 * 6);
        assert_eq!(!detached(&w, men[1]), on, "regrouped: {on}");
    }
}

/// Liveness: a team man walking to a downed man for `STALL_S` with no
/// new phase is regrouped (the job dropped). Control: a fresh walk.
#[test]
fn l_stalled_job_is_dropped_after_120_s() {
    for stalled in [true, false] {
        let mut w = med_world(|s| {
            let m = &mut s.rules.ai.medic;
            m.buy = 0;
            m.field = 0;
        });
        // No manpower: no post is built (its Dig would regroup the team).
        w.state.game.as_mut().unwrap().players[0].manpower = 0;
        let sq = atk(&mut w, 0, RIFLES, 200);
        let mt = atk(&mut w, 0, MEDICS, 100);
        steps(&mut w, 2);
        let d = downk(&mut w, sq, 3, 200, 175);
        let m = members(&w, mt)[0];
        let target = pos(&w, d);
        run(
            &mut w,
            0,
            9000,
            CommandKind::Tend {
                units: vec![m],
                target: d,
            },
        );
        steps(&mut w, 2);
        assert_eq!(
            w.state.entities.unit[m.index()].order,
            UnitOrder::Move { target }
        );
        if stalled {
            let u = &mut w.state.entities.unit[m.index()];
            u.nav.phase_ticks = (crate::ai::medic::STALL_S * 30) as u32;
            // Kept stalled: no path served.
            u.nav.phase = crate::game::NavPhase::PathPending;
        }
        steps(&mut w, 30 * 5);
        assert_eq!(!detached(&w, m), stalled, "stalled {stalled}");
    }
}

/// Liveness ("taken by another team"): a man walking to a downed man
/// another medic already tends is regrouped. Control: nobody tends him.
#[test]
fn l_casualty_taken_by_another_releases_the_walker() {
    for taken in [true, false] {
        let mut w = med_world(|s| {
            let m = &mut s.rules.ai.medic;
            m.buy = 0;
            m.field = 0;
        });
        // No manpower: no post is built (its Dig would regroup the team).
        w.state.game.as_mut().unwrap().players[0].manpower = 0;
        let sq = atk(&mut w, 0, RIFLES, 200);
        let a = atk(&mut w, 0, MEDICS, 90);
        let b = atk(&mut w, 0, MEDICS, 196);
        steps(&mut w, 2);
        let d = downk(&mut w, sq, 3, 200, 175);
        let bm = members(&w, b)[0];
        if taken {
            run(
                &mut w,
                0,
                9000,
                CommandKind::Tend {
                    units: vec![bm],
                    target: d,
                },
            );
            for _ in 0..(30 * 10) {
                w.step(&[]);
                if cas(&w, d).is_some_and(|c| c.tend.is_some()) {
                    break;
                }
            }
            assert_eq!(cas(&w, d).unwrap().tend.map(|t| t.0), Some(bm));
        }
        let am = members(&w, a)[0];
        let to = pos(&w, d);
        run(
            &mut w,
            0,
            9001,
            CommandKind::MoveUnits {
                units: vec![am],
                target: to,
            },
        );
        steps(&mut w, 30 * 5);
        assert_eq!(!detached(&w, am), taken, "taken {taken}");
    }
}

/// Liveness: a job ends when its casualty dies: the walking medic is no
/// longer on a job and rejoins (the sim releases him; the AI sends no new
/// tend to the dead man). Control: alive, he is still walking.
#[test]
fn l_job_ends_when_the_casualty_dies() {
    for dies in [true, false] {
        let mut w = med_world(|s| {
            let m = &mut s.rules.ai.medic;
            m.buy = 0;
            m.post = 0;
        });
        let sq = atk(&mut w, 0, RIFLES, 200);
        let mt = atk(&mut w, 0, MEDICS, 110);
        steps(&mut w, 2);
        let d = downk(&mut w, sq, 3, 200, 175);
        let mut walker = None;
        for _ in 0..(30 * 8) {
            w.step(&[]);
            walker = members(&w, mt).into_iter().find(|&m| detached(&w, m));
            if walker.is_some() {
                break;
            }
        }
        let m = walker.expect("the AI sent a medic");
        if dies {
            let t = w.tick();
            w.state
                .game
                .as_mut()
                .unwrap()
                .casualties
                .get_mut(&d)
                .unwrap()
                .bleed_out_at = t + 1;
        }
        steps(&mut w, 30 * 3);
        assert_eq!(!detached(&w, m), dies, "dies {dies}");
        if dies {
            assert_eq!(w.state.entities.kind[d.index()], EntityKind::Corpse);
        }
    }
}

/// Every `[ai.medic]` switch 0: a team, three downs and a post bring no
/// medic order. Control: the default on (A3 tends).
#[test]
fn switches_off_the_medic_ai_does_nothing() {
    for on in [false, true] {
        let mut w = med_world(|s| {
            let m = &mut s.rules.ai.medic;
            let v = on as u32;
            (m.buy, m.field, m.post, m.reinforce) = (v, v, v, v);
        });
        let p = out(&w, 40);
        aid_post(&mut w, p);
        let sq = atk(&mut w, 0, RIFLES, 150);
        atk(&mut w, 0, MEDICS, 100);
        steps(&mut w, 2);
        for k in 3..6 {
            let q = out(&w, 150) + FxVec2::from_int(0, 4 * k as i32);
            down_at(&mut w, sq, k, q, 170);
        }
        steps(&mut w, 30 * 10);
        let k = counters(&w);
        let n = k.medic_buys + k.medic_tends + k.medic_carries + k.post_builds + k.reinforce_orders;
        assert_eq!(n > 0, on, "switches {on}");
    }
}

/// Save/load mid-job (plan 6): the medic AI keeps no state of its own, so
/// a save while its team carries a man (and a second team walks to a
/// post) loads and plays on to the same hash, at 1 and 3 threads.
/// Control: the unsaved run is the reference.
#[test]
fn save_load_mid_ai_carry_keeps_the_hash() {
    let mut w = med_world(|s| s.rules.ai.medic.buy = 0);
    let p = out(&w, 40);
    aid_post(&mut w, p);
    let sq = atk(&mut w, 0, RIFLES, 160);
    atk(&mut w, 0, MEDICS, 110);
    steps(&mut w, 2);
    let d = downk(&mut w, sq, 3, 150, 0);
    for _ in 0..(30 * 20) {
        w.step(&[]);
        if cas(&w, d).is_some_and(|c| c.carriers[0].is_some()) {
            break;
        }
    }
    assert!(
        cas(&w, d).is_some_and(|c| c.carriers[0].is_some()),
        "carried"
    );
    steps(&mut w, 30 * 5);
    let save = w.save();
    let mut x = World::load(&save, 1).unwrap();
    let mut y = World::load(&save, 3).unwrap();
    assert_eq!(x.full_state_hash(), w.full_state_hash());
    for _ in 0..(30 * 30) {
        w.step(&[]);
        x.step(&[]);
        y.step(&[]);
    }
    assert_eq!(x.full_state_hash(), w.full_state_hash(), "loaded");
    assert_eq!(y.full_state_hash(), w.full_state_hash(), "3 threads");
}

/// Build C fix (found by the shipped-switch save round trips): a downed
/// man's `wound.by` keeps the hitter's full kill class, with the shooting
/// vehicle's type above its low nibble (a hull MG's `VEHICLE_MG | (type +
/// 1) << 4`), and the load check accepts it. Control: a class whose low
/// nibble is past `kill_class::COUNT` is still refused.
#[test]
fn wound_by_with_a_vehicle_class_loads() {
    use crate::game::kill_class;
    let w = crate::wounds::Wound {
        state: crate::wounds::DOWNED,
        by: (1, kill_class::VEHICLE_MG | (17 + 1) << 4),
        ..Default::default()
    };
    assert!(crate::wounds::wound_ok(EntityKind::Downed, &w, 2, 18, 300));
    let bad = crate::wounds::Wound {
        by: (1, kill_class::COUNT as u16 | (17 + 1) << 4),
        ..w
    };
    assert!(!crate::wounds::wound_ok(
        EntityKind::Downed,
        &bad,
        2,
        18,
        300
    ));
    // S12 F2 (review R10): the type bits name one of the setup's
    // vehicle types (type 17 needs 18).
    assert!(!crate::wounds::wound_ok(EntityKind::Downed, &w, 2, 17, 300));
}
