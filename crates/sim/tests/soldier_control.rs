//! Soldier-level orders (S1b "Orders on a soldier set"): validation of the
//! men a command names, detaching and regrouping, stance orders, `DigUnits`
//! splitting its line, and determinism with unit orders mixed into an
//! AI-vs-AI match (thread counts, save → load → continue).
//!
//! Squads are staged with `World::place_squad` (a setup tool) on the open
//! ground west of the map centre that `tests/combat.rs` uses.

use std::path::Path;

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::{
    Command, CommandKind, Fx, FxVec2, NavPhase, Order, OrderKind, PathReq, PlayerId, SimId,
    SquadId, Stance, StanceOrder, UnitOrder, World,
};

/// The S1-S3c content frozen before the S4 content switch: these mechanics
/// tests were written for its squads (8-man rifle squads, 3-man MG teams)
/// and keep running on it; the S4 cards are covered by `s4_members.rs`,
/// `variants.rs` and the AI-vs-AI runs.
const S3C_MOD: &str = "../../sdk/crates/content/tests/fixtures/ww2_s3c";

fn setup(seed: u64, ai: bool) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(S3C_MOD);
    let defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    defs.resolve(&MatchRequest::skirmish(
        seed,
        "ww2:germany",
        "ww2:usa",
        Difficulty::Normal,
    ))
    .map(|mut s| {
        s.players.iter_mut().for_each(|p| p.is_ai = ai);

        s
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

/// Both sides driven by hand.
fn world(seed: u64, threads: usize) -> World {
    World::new_match(setup(seed, false), threads).unwrap()
}

fn cmd(player: u8, seq: u32, kind: CommandKind) -> Command {
    Command {
        player: PlayerId(player),
        seq,
        kind,
    }
}

const EAST: FxVec2 = FxVec2::new(Fx::ONE, Fx::ZERO);
const WEST: FxVec2 = FxVec2::new(Fx::NEG_ONE, Fx::ZERO);

/// Player `p`'s squads of type `ty`.
fn squads(w: &World, p: u8, ty: &str) -> Vec<SquadId> {
    let t = w.match_setup().unwrap().squad_type_index(ty).unwrap();
    w.game()
        .unwrap()
        .squads()
        .iter()
        .filter(|(_, s)| s.owner == PlayerId(p) && s.type_idx == t)
        .map(|(id, _)| *id)
        .collect()
}

fn members(w: &World, sq: SquadId) -> Vec<SimId> {
    w.game().unwrap().squad(sq).unwrap().members.clone()
}

fn pos(w: &World, id: SimId) -> FxVec2 {
    w.entities().transform()[id.index()].pos.xy()
}

fn dist(a: FxVec2, b: FxVec2) -> Fx {
    (a - b).length()
}

fn detached(w: &World, id: SimId) -> bool {
    w.entities().unit()[id.index()].detached
}

fn unit_order(w: &World, id: SimId) -> UnitOrder {
    w.entities().unit()[id.index()].order.clone()
}

/// Steps until `done(w)` or `max_ticks`; returns whether `done` held.
fn run_until(w: &mut World, max_ticks: u64, done: impl Fn(&World) -> bool) -> bool {
    for _ in 0..max_ticks {
        if done(w) {
            return true;
        }
        w.step(&[]);
    }
    done(w)
}

/// Steps one tick with `kind` from player `p`; returns whether it applied.
fn order(w: &mut World, p: u8, kind: CommandKind) -> bool {
    let before = w.stats().commands_applied;
    let seq = w.tick() as u32;
    w.step(&[cmd(p, seq, kind)]);
    w.stats().commands_applied == before + 1
}

/// Buys a squad of type `ty` for player `p` and steps until it arrives.
fn call_in(w: &mut World, p: u8, ty: &str) -> SquadId {
    let t = w.match_setup().unwrap().squad_type_index(ty).unwrap();
    let known: Vec<SquadId> = w.game().unwrap().squads().keys().copied().collect();
    assert!(order(w, p, CommandKind::CallIn { squad_type: t }));
    for _ in 0..30 * 60 {
        if let Some(id) = w
            .game()
            .unwrap()
            .squads()
            .keys()
            .find(|id| !known.contains(id))
        {
            return *id;
        }
        w.step(&[]);
    }
    panic!("call-in never arrived");
}

#[test]
fn unit_commands_drop_bad_men_and_reject_empty_sets() {
    let mut w = world(1, 2);
    let a = squads(&w, 0, "ww2:ger_rifle_squad")[0];
    let b = squads(&w, 1, "ww2:usa_rifle_squad")[0];
    // A firefight leaves a corpse to name.
    w.place_squad(a, FxVec2::from_int(430, 420), EAST);
    w.place_squad(b, FxVec2::from_int(550, 420), WEST);
    assert!(run_until(&mut w, 30 * 120, |w| !w
        .snapshot()
        .corpses
        .is_empty()));
    let corpse = w.snapshot().corpses[0].id;
    let mine = |w: &World| -> Vec<SimId> {
        let sq = squads(w, 0, "ww2:ger_rifle_squad")[1];
        members(w, sq)
    };
    let enemy = members(&w, squads(&w, 1, "ww2:usa_rifle_squad")[1]);
    let target = FxVec2::from_int(200, 200);
    let mv = |units: Vec<SimId>| CommandKind::MoveUnits { units, target };

    // Empty, too many (even if they are all one man), foreign only, dead
    // only and unknown only: ignored.
    let man = mine(&w)[0];
    let rejected_before = w.stats().commands_rejected;
    assert!(!order(&mut w, 0, mv(vec![])));
    assert!(!order(
        &mut w,
        0,
        mv(vec![man; frontline_sim::MAX_UNITS + 1])
    ));
    assert!(!order(&mut w, 0, mv(enemy.clone())));
    assert!(!order(&mut w, 0, mv(vec![corpse])));
    assert!(!order(&mut w, 0, mv(vec![SimId(1 << 20)])));
    assert!(!order(
        &mut w,
        0,
        CommandKind::SetStance {
            units: enemy.clone(),
            stance: StanceOrder::Prone,
        }
    ));
    assert!(!order(&mut w, 0, CommandKind::StopUnits { units: vec![] }));
    assert_eq!(w.stats().commands_rejected, rejected_before + 7);
    assert!(enemy.iter().all(|&e| !detached(&w, e)));
    assert!(mine(&w).iter().all(|&m| !detached(&w, m)));
    assert!(
        enemy
            .iter()
            .all(|e| w.entities().unit()[e.index()].stance == StanceOrder::Auto)
    );

    // Exactly 120 duplicates of one man, plus a foreign, a dead and an
    // unknown id: only he is ordered, and queued for one path.
    let mut units = vec![man; frontline_sim::MAX_UNITS - 3];
    units.extend([enemy[0], corpse, SimId(1 << 20)]);
    assert!(order(&mut w, 0, mv(units)));
    assert!(detached(&w, man));
    assert!(!detached(&w, enemy[0]));
    assert!(matches!(unit_order(&w, man), UnitOrder::Move { .. }));
    let queued = w.game().unwrap().path_queue().queues[0]
        .iter()
        .filter(|r| **r == PathReq::Unit(man))
        .count();
    assert!(queued <= 1);
    assert!(mine(&w)[1..].iter().all(|&m| !detached(&w, m)));

    // Another player cannot order him, nor change his stance.
    assert!(!order(
        &mut w,
        1,
        CommandKind::StopUnits { units: vec![man] }
    ));
    assert!(!order(
        &mut w,
        1,
        CommandKind::SetStance {
            units: vec![man],
            stance: StanceOrder::Prone,
        }
    ));
    assert_eq!(w.entities().unit()[man.index()].stance, StanceOrder::Auto);

    // DigUnits with only men who cannot dig: ignored.
    let mg = call_in(&mut w, 0, "ww2:ger_mg_team");
    let line = vec![FxVec2::from_int(300, 300), FxVec2::from_int(306, 300)];
    let digs = |units: Vec<SimId>, line: Vec<FxVec2>| CommandKind::DigUnits {
        units,
        line,
        profile: Default::default(),
    };
    let mg_men = members(&w, mg);
    assert!(!order(&mut w, 0, digs(mg_men, line.clone())));
    // A bad line is refused like a squad Dig.
    let far = vec![FxVec2::from_int(300, 300), FxVec2::from_int(400, 300)];
    let two = mine(&w)[2..4].to_vec();
    assert!(!order(&mut w, 0, digs(two.clone(), far)));
    assert!(!order(&mut w, 0, digs(two, vec![line[0]])));
    assert!(mine(&w)[2..4].iter().all(|&m| !detached(&w, m)));
    assert!(w.game().unwrap().unit_digs().is_empty());
}

#[test]
fn men_sent_alone_detach_spread_and_regroup() {
    let mut w = world(2, 2);
    let sq = squads(&w, 0, "ww2:ger_rifle_squad")[0];
    w.place_squad(sq, FxVec2::from_int(400, 420), EAST);
    let men = members(&w, sq);
    let home: Vec<FxVec2> = men.iter().map(|&m| pos(&w, m)).collect();
    let target = FxVec2::from_int(430, 450);
    let pair = vec![men[5], men[3]];
    assert!(order(
        &mut w,
        0,
        CommandKind::MoveUnits {
            units: pair.clone(),
            target,
        }
    ));
    // The pair is detached; the squad keeps its (idle) order and counts them.
    let g = w.game().unwrap();
    assert_eq!(g.squad(sq).unwrap().order, Order::Idle);
    assert_eq!(g.squad(sq).unwrap().members.len(), 8);
    let snap = w.snapshot();
    let card = snap.squads.iter().find(|s| s.id == sq.0).unwrap();
    assert_eq!((card.members, card.detached), (8, 2));
    assert_eq!(snap.units.len(), 2);
    assert!(snap.units.iter().all(|u| u.order == OrderKind::Move
        && u.target == target.to_f32()
        && pair.contains(&u.id)));
    let n = snap.soldiers.len();
    assert_eq!(snap.soldier_detached.len(), n);
    assert_eq!(snap.soldier_stance_order.len(), n);
    assert_eq!(snap.soldier_detached.iter().filter(|d| **d).count(), 2);

    // They walk there on their own and stand a squad spacing apart in a
    // line across the way they came; the rest of the squad stays put.
    let there = |w: &World| {
        pair.iter().all(|&m| {
            !matches!(unit_order(w, m), UnitOrder::Move { .. })
                && w.entities().unit()[m.index()].nav.phase == NavPhase::Holding
        })
    };
    assert!(run_until(&mut w, 30 * 60, there), "the pair never arrived");
    for &m in &pair {
        assert!(detached(&w, m), "an arrived man stays detached");
        assert_eq!(unit_order(&w, m), UnitOrder::Idle);
        assert!(
            dist(pos(&w, m), target) < Fx::from_int(4),
            "{:?}",
            pos(&w, m)
        );
    }
    let gap = dist(pos(&w, pair[0]), pos(&w, pair[1]));
    assert!(gap > Fx::from_int(2) && gap < Fx::from_int(4), "{gap:?}");
    for (k, &m) in men.iter().enumerate() {
        if !pair.contains(&m) {
            assert!(dist(pos(&w, m), home[k]) < Fx::from_int(1));
        }
    }
    // Their own spots are reported for markers.
    let snap = w.snapshot();
    for u in &snap.units {
        let goal = w.entities().unit()[u.id.index()].nav.goal;
        assert_eq!(u.goal, goal.to_f32());
        assert_eq!(u.phase, NavPhase::Holding);
    }

    // A squad order clears `detached` and the whole squad walks together.
    let goal = FxVec2::from_int(460, 400);
    assert!(order(
        &mut w,
        0,
        CommandKind::Move {
            squad: sq,
            target: goal
        }
    ));
    assert!(men.iter().all(|&m| !detached(&w, m)));
    assert!(w.snapshot().units.is_empty());
    assert!(run_until(&mut w, 30 * 90, |w| {
        w.game().unwrap().squad(sq).unwrap().order == Order::Idle
    }));
    for &m in &men {
        assert!(
            dist(pos(&w, m), goal) < Fx::from_int(15),
            "{:?}",
            pos(&w, m)
        );
    }
}

#[test]
fn unit_orders_naming_a_whole_squad_are_squad_orders() {
    let mut w = world(3, 1);
    let rifles = squads(&w, 0, "ww2:ger_rifle_squad");
    let (a, b) = (rifles[0], rifles[1]);
    w.place_squad(a, FxVec2::from_int(400, 420), EAST);
    w.place_squad(b, FxVec2::from_int(400, 440), EAST);
    // All of A plus two of B: A gets a squad Move, the two are detached.
    let mut units = members(&w, a);
    units.extend(&members(&w, b)[..2]);
    let target = FxVec2::from_int(450, 430);
    assert!(order(
        &mut w,
        0,
        CommandKind::AttackMoveUnits {
            units: units.clone(),
            target,
        }
    ));
    let g = w.game().unwrap();
    assert_eq!(g.squad(a).unwrap().order, Order::AttackMove { target });
    assert_eq!(g.squad(b).unwrap().order, Order::Idle);
    assert!(members(&w, a).iter().all(|&m| !detached(&w, m)));
    let two = members(&w, b)[..2].to_vec();
    assert!(
        two.iter()
            .all(|&m| detached(&w, m) && matches!(unit_order(&w, m), UnitOrder::AttackMove { .. }))
    );
    // Stopping all of B by its men is a squad Stop: the two rejoin and the
    // squad forms up around its leader.
    let units = members(&w, b);
    assert!(order(&mut w, 0, CommandKind::StopUnits { units }));
    assert!(members(&w, b).iter().all(|&m| !detached(&w, m)));
    assert_eq!(w.game().unwrap().squad(b).unwrap().order, Order::Idle);
    // Retreating all of A by its men is a squad Retreat.
    let units = members(&w, a);
    assert!(order(&mut w, 0, CommandKind::RetreatUnits { units }));
    assert_eq!(w.game().unwrap().squad(a).unwrap().order, Order::Retreat);
}

#[test]
fn stop_and_regroup_bring_detached_men_back() {
    let mut w = world(4, 2);
    let sq = squads(&w, 0, "ww2:ger_rifle_squad")[0];
    w.place_squad(sq, FxVec2::from_int(400, 420), EAST);
    let men = members(&w, sq);
    // The leader and one more man walk off; the squad's next man leads.
    assert!(order(
        &mut w,
        0,
        CommandKind::MoveUnits {
            units: vec![men[0], men[1]],
            target: FxVec2::from_int(400, 460),
        }
    ));
    assert!(run_until(&mut w, 30 * 60, |w| {
        dist(pos(w, men[0]), FxVec2::from_int(400, 460)) < Fx::from_int(5)
    }));
    let snap = w.snapshot();
    let card = snap.squads.iter().find(|s| s.id == sq.0).unwrap();
    let anchor = pos(&w, men[2]).to_f32();
    assert_eq!(
        [card.anchor[0], card.anchor[1]],
        anchor,
        "anchor on the leader in formation"
    );
    // One stops where he stands; he stays detached.
    assert!(order(
        &mut w,
        0,
        CommandKind::StopUnits {
            units: vec![men[1]]
        }
    ));
    assert!(detached(&w, men[1]));
    let nav = &w.entities().unit()[men[1].index()].nav;
    assert_eq!(nav.phase, NavPhase::Holding);
    assert!(dist(nav.goal, pos(&w, men[1])) < Fx::from_milli(200));
    // A squad Stop regroups: the man still in formation leads, and the
    // detached two walk back into a line around him.
    assert!(order(&mut w, 0, CommandKind::Stop { squad: sq }));
    let g = w.game().unwrap();
    let squad = g.squad(sq).unwrap();
    assert_eq!(squad.members[0], men[2]);
    assert_eq!(squad.nav.phase, NavPhase::Arriving);
    assert!(men.iter().all(|&m| !detached(&w, m)));
    let lead = pos(&w, men[2]);
    assert!(run_until(&mut w, 30 * 60, |w| {
        w.game().unwrap().squad(sq).unwrap().nav.phase == NavPhase::Holding
    }));
    for &m in &men {
        assert!(
            dist(pos(&w, m), lead) < Fx::from_int(15),
            "{:?}",
            pos(&w, m)
        );
    }

    // Regroup only rejoins: a walking squad keeps its order.
    assert!(order(
        &mut w,
        0,
        CommandKind::MoveUnits {
            units: vec![men[4]],
            target: FxVec2::from_int(380, 380),
        }
    ));
    let goal = FxVec2::from_int(470, 420);
    assert!(order(
        &mut w,
        0,
        CommandKind::Move {
            squad: sq,
            target: goal
        }
    ));
    assert!(!detached(&w, men[4]), "a squad order clears detached");
    assert!(order(
        &mut w,
        0,
        CommandKind::MoveUnits {
            units: vec![men[4]],
            target: FxVec2::from_int(380, 380),
        }
    ));
    assert!(order(&mut w, 0, CommandKind::Regroup { squad: sq }));
    assert!(!detached(&w, men[4]));
    assert_eq!(
        w.game().unwrap().squad(sq).unwrap().order,
        Order::Move { target: goal }
    );
    // Regroup of a squad that is not this player's: refused.
    let enemy = squads(&w, 1, "ww2:usa_rifle_squad")[0];
    assert!(!order(&mut w, 0, CommandKind::Regroup { squad: enemy }));
}

#[test]
fn stance_orders_keep_formation_and_prone_men_crawl() {
    let mut w = world(5, 2);
    let sq = squads(&w, 0, "ww2:ger_rifle_squad")[0];
    w.place_squad(sq, FxVec2::from_int(400, 420), EAST);
    let men = members(&w, sq);
    assert!(order(
        &mut w,
        0,
        CommandKind::SetStance {
            units: vec![men[1], men[2]],
            stance: StanceOrder::Prone,
        }
    ));
    w.step(&[]);
    let stance = |w: &World, m: SimId| w.entities().combat()[m.index()].stance;
    assert_eq!(stance(&w, men[1]), Stance::Prone);
    assert_eq!(stance(&w, men[0]), Stance::Stand);
    // A stance order neither detaches nor touches the squad's order.
    assert!(men.iter().all(|&m| !detached(&w, m)));
    let snap = w.snapshot();
    let k = snap
        .soldier_ids
        .iter()
        .position(|id| *id == men[1])
        .unwrap();
    assert_eq!(snap.soldier_stance_order[k], StanceOrder::Prone);
    assert_eq!(snap.soldier_stance[k], Stance::Prone);
    // Crouch and Auto again.
    assert!(order(
        &mut w,
        0,
        CommandKind::SetStance {
            units: vec![men[2]],
            stance: StanceOrder::Crouch,
        }
    ));
    w.step(&[]);
    assert_eq!(stance(&w, men[2]), Stance::Crouch);

    // Prone men crawl: one standing and one prone man walk 30 m each.
    let (stand, prone) = (men[0], men[1]);
    let go = |m: SimId, y: i32| CommandKind::MoveUnits {
        units: vec![m],
        target: FxVec2::from_int(430, y),
    };
    let s0 = pos(&w, stand);
    let p0 = pos(&w, prone);
    let seq = w.tick() as u32;
    w.step(&[cmd(0, seq, go(stand, 410)), cmd(0, seq + 1, go(prone, 430))]);
    for _ in 0..30 * 6 {
        w.step(&[]);
    }
    let walked = dist(pos(&w, stand), s0);
    let crawled = dist(pos(&w, prone), p0);
    let pct = w.match_setup().unwrap().rules.movement.prone_speed_pct as i32;
    assert!(walked > Fx::from_int(10), "{walked:?}");
    assert!(
        crawled < walked.mul_int(pct + 10).div_int(100)
            && crawled > walked.mul_int(pct - 10).div_int(100),
        "walked {walked:?}, crawled {crawled:?}"
    );
    assert_eq!(stance(&w, prone), Stance::Prone);
    // Back to Auto: he stands up.
    assert!(order(
        &mut w,
        0,
        CommandKind::SetStance {
            units: vec![prone],
            stance: StanceOrder::Auto,
        }
    ));
    w.step(&[]);
    assert_eq!(stance(&w, prone), Stance::Stand);
}

#[test]
fn detached_men_fight_and_fall_back_by_their_own_order() {
    // The squads of `tests/combat.rs`, 120 m apart with clear lines.
    let mut w = world(7, 2);
    let a = squads(&w, 0, "ww2:ger_rifle_squad")[0];
    let b = squads(&w, 1, "ww2:usa_rifle_squad")[0];
    w.place_squad(a, FxVec2::from_int(430, 420), EAST);
    w.place_squad(b, FxVec2::from_int(550, 420), WEST);
    let men = members(&w, a);
    let (scout, walker, runner) = (men[0], men[2], men[1]);
    // One man attack-moves at the enemy, one walks across the field and one
    // falls back to base.
    let seq = w.tick() as u32;
    w.step(&[
        cmd(
            0,
            seq,
            CommandKind::AttackMoveUnits {
                units: vec![scout],
                target: FxVec2::from_int(540, 420),
            },
        ),
        cmd(
            0,
            seq + 1,
            CommandKind::MoveUnits {
                units: vec![walker],
                target: FxVec2::from_int(470, 460),
            },
        ),
        cmd(
            0,
            seq + 2,
            CommandKind::RetreatUnits {
                units: vec![runner],
            },
        ),
    ]);
    let base = w.game().unwrap().base_of(PlayerId(0));
    let nav = &w.entities().unit()[runner.index()].nav;
    assert!(dist(nav.goal, base) < Fx::from_int(20));
    let from_base = dist(pos(&w, runner), base);
    let alive =
        |w: &World, m: SimId| w.entities().kind()[m.index()] == frontline_sim::EntityKind::Soldier;
    let mut halted = false;
    for _ in 0..30 * 20 {
        let shots = w.entities().weapon()[walker.index()].shots;
        w.step(&[]);
        if alive(&w, scout) && w.entities().combat()[scout.index()].target.is_some() {
            // He halts to fight where he stands, with his own order kept.
            let wp = w.entities().mover()[scout.index()].waypoint;
            assert!(dist(wp, pos(&w, scout)) < Fx::from_int(1));
            assert!(matches!(
                unit_order(&w, scout),
                UnitOrder::AttackMove { .. }
            ));
            halted = true;
        }
        // A man on a Move fires only while standing still.
        if alive(&w, walker) && w.entities().weapon()[walker.index()].shots > shots {
            let v = w.entities().velocity()[walker.index()].lin.length();
            assert!(v <= Fx::from_milli(300), "fired while walking at {v:?}");
        }
    }
    assert!(halted, "the scout never engaged");
    // The man falling back went the other way and never fired.
    assert!(alive(&w, runner));
    assert!(dist(pos(&w, runner), base) < from_base - Fx::from_int(30));
    assert_eq!(w.entities().weapon()[runner.index()].shots, 0);
    assert!(!w.entities().combat()[runner.index()].pinned);
}

#[test]
fn dig_units_split_the_line_among_only_those_men() {
    let mut w = world(6, 2);
    let sq = squads(&w, 0, "ww2:ger_rifle_squad")[0];
    w.place_squad(sq, FxVec2::from_int(400, 420), EAST);
    let mg = call_in(&mut w, 0, "ww2:ger_mg_team");
    w.place_squad(mg, FxVec2::from_int(400, 440), EAST);
    let men = members(&w, sq);
    let line = vec![FxVec2::from_int(410, 410), FxVec2::from_int(410, 419)];
    // Three riflemen and an MG man (who cannot dig and is dropped).
    let diggers = vec![men[6], men[1], men[3]];
    let mut units = diggers.clone();
    units.push(members(&w, mg)[0]);
    assert!(order(
        &mut w,
        0,
        CommandKind::DigUnits {
            units,
            line: line.clone(),
            profile: Default::default(),
        }
    ));
    let g = w.game().unwrap();
    assert_eq!(g.squad(sq).unwrap().order, Order::Idle);
    assert!(!detached(&w, members(&w, mg)[0]));
    let (&job_id, job) = g.unit_digs().iter().next().unwrap();
    assert_eq!(g.unit_digs().len(), 1);
    assert_eq!(job.owner, PlayerId(0));
    // Three equal segments along the line, one man each, in order along it
    // by where the men stand (the line runs north, so by y).
    let segs = &job.job.segments;
    assert_eq!(segs.len(), 3);
    assert_eq!(segs[0].from, Fx::ZERO);
    assert_eq!(segs[2].to, Fx::from_int(9));
    assert!(segs.windows(2).all(|s| s[0].to == s[1].from));
    let mut by_y = diggers.clone();
    by_y.sort_by_key(|&m| (pos(&w, m).y, m));
    let assigned: Vec<SimId> = segs.iter().map(|s| s.digger.unwrap()).collect();
    assert_eq!(assigned, by_y);
    for &m in &diggers {
        assert!(detached(&w, m));
        assert_eq!(unit_order(&w, m), UnitOrder::Dig { job: job_id });
    }
    let snap = w.snapshot();
    assert_eq!(snap.units.len(), 3);
    assert!(snap.units.iter().all(|u| u.order == OrderKind::Dig));

    // Halfway, the southern digger is stopped: his segment is let go and
    // the others finish the whole line between them.
    let quitter = by_y[0];
    assert!(run_until(&mut w, 30 * 120, |w| {
        w.game().unwrap().unit_digs()[&job_id].job.segments[0].passes >= 2
    }));
    assert!(order(
        &mut w,
        0,
        CommandKind::StopUnits {
            units: vec![quitter]
        }
    ));
    w.step(&[]);
    let seg0 = &w.game().unwrap().unit_digs()[&job_id].job.segments[0];
    assert_ne!(seg0.digger, Some(quitter));
    assert!(run_until(&mut w, 30 * 600, |w| w
        .game()
        .unwrap()
        .unit_digs()
        .is_empty()));
    for &m in &by_y[1..] {
        assert!(detached(&w, m));
        assert_eq!(unit_order(&w, m), UnitOrder::Idle);
    }
    // Dug to full depth along the whole line, the stopped man's part too.
    let t = w.terrain();
    let depth: Fx = w.match_setup().unwrap().rules.dig.depth.into();
    for y in [411, 414, 418] {
        let p = FxVec2::from_int(410, y);
        let dug = t.base_height_at(p) - t.height_at(p);
        assert!(dug >= depth - Fx::from_milli(50), "y {y}: {dug:?}");
    }
    // The men left in formation never dug: the squad has no dig order.
    assert_eq!(w.game().unwrap().squad(sq).unwrap().order, Order::Idle);

    // Naming exactly the whole squad gives it a squad Dig order.
    let line2 = vec![FxVec2::from_int(420, 400), FxVec2::from_int(430, 400)];
    let units = members(&w, sq);
    assert!(order(
        &mut w,
        0,
        CommandKind::DigUnits {
            units,
            line: line2,
            profile: Default::default()
        }
    ));
    assert!(matches!(
        w.game().unwrap().squad(sq).unwrap().order,
        Order::Dig { .. }
    ));
    assert!(members(&w, sq).iter().all(|&m| !detached(&w, m)));
    assert!(w.game().unwrap().unit_digs().is_empty());
}

// ------------------------------------------------------------ determinism --

/// Scripted human orders for AI player 0's men, derived from the state so
/// every run sends the same ones: every few seconds some of its soldiers
/// get a unit order of each kind (and some squads a regroup).
fn script(w: &World) -> Vec<Command> {
    let t = w.tick();
    if t < 300 || !t.is_multiple_of(97) {
        return Vec::new();
    }
    let ents = w.entities();
    let mine: Vec<SimId> = ents
        .soldiers()
        .filter(|id| ents.owner()[id.index()] == PlayerId(0))
        .collect();
    if mine.is_empty() {
        return Vec::new();
    }
    let k = (t / 97) as usize;
    let pick =
        |n: usize| -> Vec<SimId> { (0..n).map(|j| mine[(k * 7 + j * 5) % mine.len()]).collect() };
    let at = pos(w, mine[k % mine.len()]);
    let off = FxVec2::from_int((k % 5) as i32 * 10 - 20, (k % 3) as i32 * 10 - 10);
    let target = w.terrain().clamp(at + off);
    let kind = match k % 7 {
        0 => CommandKind::MoveUnits {
            units: pick(3),
            target,
        },
        1 => CommandKind::AttackMoveUnits {
            units: pick(2),
            target,
        },
        2 => CommandKind::SetStance {
            units: pick(4),
            stance: [
                StanceOrder::Prone,
                StanceOrder::Crouch,
                StanceOrder::Stand,
                StanceOrder::Auto,
            ][k / 7 % 4],
        },
        3 => CommandKind::StopUnits { units: pick(1) },
        4 => CommandKind::DigUnits {
            units: pick(3),
            line: vec![target, w.terrain().clamp(target + FxVec2::from_int(8, 3))],
            profile: Default::default(),
        },
        5 => CommandKind::RetreatUnits { units: pick(1) },
        _ => {
            let sq = ents.squad()[mine[k % mine.len()].index()].unwrap();
            CommandKind::Regroup { squad: sq }
        }
    };
    // Plus one that is always refused (player 1's man), sent unsorted.
    let theirs = ents
        .soldiers()
        .find(|id| ents.owner()[id.index()] == PlayerId(1));
    let mut cmds = vec![cmd(0, 1_000_000 + k as u32, kind)];
    if let Some(e) = theirs {
        cmds.insert(
            0,
            cmd(
                0,
                2_000_000 + k as u32,
                CommandKind::StopUnits { units: vec![e] },
            ),
        );
    }
    cmds
}

fn ai_match(seed: u64, threads: usize) -> World {
    World::new_match(setup(seed, true), threads).unwrap()
}

/// Runs to `until` with the script, collecting published hashes; checks
/// the incremental hash against a full recompute every `full_every` ticks.
fn run(w: &mut World, until: u64, full_every: u64) -> Vec<(u64, u64)> {
    let mut out = Vec::new();
    while w.tick() < until {
        let cmds = script(w);
        if let Some(h) = w.step(&cmds) {
            out.push((w.tick(), h));
        }
        if full_every > 0 && w.tick().is_multiple_of(full_every) {
            assert_eq!(w.state_hash(), w.full_state_hash(), "tick {}", w.tick());
        }
    }
    out
}

#[test]
fn unit_orders_in_an_ai_match_are_identical_across_thread_counts() {
    let end = 4 * 60 * 30;
    let mut reference_world = ai_match(13, 1);
    let reference = run(&mut reference_world, end, 89);
    let st = reference_world.stats();
    assert!(st.deaths > 0 && st.commands_applied > 50, "{st:?}");
    // The script really detached men and the AI kept playing.
    let g = reference_world.game().unwrap();
    assert!(g.ai(PlayerId(0)).unwrap().issued > 10);
    assert_eq!(g.ai(PlayerId(0)).unwrap().rejected, 0);
    for threads in [2, 3, 8] {
        let mut w = ai_match(13, threads);
        assert_eq!(run(&mut w, end, 0), reference, "{threads} threads");
    }
}

#[test]
fn unit_orders_survive_save_load_continue() {
    let end = 4 * 60 * 30;
    let mut straight = ai_match(14, 2);
    let mut seen_detached = false;
    let mut seen_dig = false;
    let mut reference = Vec::new();
    while straight.tick() < end {
        let cmds = script(&straight);
        if let Some(h) = straight.step(&cmds) {
            reference.push((straight.tick(), h));
        }
        seen_detached |= !straight.snapshot().units.is_empty();
        seen_dig |= !straight.game().unwrap().unit_digs().is_empty();
    }
    assert!(seen_detached && seen_dig);

    // Save at a moment with detached men walking and a unit dig going on.
    let mut first = ai_match(14, 2);
    let mut mid = 2 * 60 * 30;
    run(&mut first, mid, 0);
    let busy = |w: &World| {
        let walking =
            w.entities().unit().iter().any(|u| {
                u.detached && matches!(u.nav.phase, NavPhase::Moving | NavPhase::PathPending)
            });
        walking && !w.game().unwrap().unit_digs().is_empty()
    };
    while !busy(&first) {
        mid += 1;
        run(&mut first, mid, 0);
        assert!(mid < end - 600, "no busy moment to save in");
    }
    let bytes = first.save();
    let mut resumed = World::load(&bytes, 3).unwrap();
    assert_eq!(resumed.state(), first.state());
    assert_eq!(resumed.state_hash(), first.state_hash());
    drop(first);
    let tail: Vec<_> = reference.into_iter().filter(|(t, _)| *t > mid).collect();
    assert_eq!(run(&mut resumed, end, 31), tail);
    assert_eq!(resumed.save(), straight.save(), "final save bytes differ");
    assert_eq!(resumed.snapshot(), straight.snapshot());
}
