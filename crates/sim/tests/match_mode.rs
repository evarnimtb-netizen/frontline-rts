//! Match mode (S1 stage 1): the ww2 mod's Kessel Ridge skirmish, squad
//! orders, path queue, formations, call-ins and surrender, plus determinism
//! across thread counts and save → load → continue.

use std::path::Path;

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::{
    CallInEvent, CallInRefusal, Command, CommandKind, Fx, FxVec2, NavPhase, Order, OrderKind,
    Outcome, PlayerId, SimId, SquadId, World,
};

fn setup(seed: u64) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    defs.resolve(&MatchRequest::skirmish(
        seed,
        "ww2:germany",
        "ww2:usa",
        Difficulty::Normal,
    ))
    .map(|mut s| {
        // These tests drive both sides by hand.
        s.players.iter_mut().for_each(|p| p.is_ai = false);
        s
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

fn world(seed: u64, threads: usize) -> World {
    World::new_match(setup(seed), threads).unwrap()
}

/// The smallest path budget the rules allow: most searches overrun it
/// (debt) and long ones hit the per-search cap (partial paths, re-paths).
fn tight_world(seed: u64, threads: usize) -> World {
    let mut s = setup(seed);
    s.rules.movement.path_budget_per_tick = 64;
    s.rules.movement.path_max_expansions = 64;
    World::new_match(s, threads).unwrap()
}

fn cmd(player: u8, seq: u32, kind: CommandKind) -> Command {
    Command {
        player: PlayerId(player),
        seq,
        kind,
    }
}

fn squads_of(w: &World, player: u8) -> Vec<SquadId> {
    w.game()
        .unwrap()
        .squads()
        .iter()
        .filter(|(_, s)| s.owner == PlayerId(player))
        .map(|(id, _)| *id)
        .collect()
}

fn type_index(w: &World, id: &str) -> u16 {
    w.match_setup().unwrap().squad_type_index(id).unwrap()
}

fn pos(w: &World, id: SimId) -> FxVec2 {
    w.entities().transform()[id.index()].pos.xy()
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

#[test]
fn match_starts_with_squads_at_bases() {
    let w = world(1, 2);
    let g = w.game().unwrap();
    assert_eq!(g.players().len(), 2);
    assert_eq!(g.squads().len(), 6);
    // S4: 2 line squads + 1 engineer squad each: Germany 10 + 10 + 8,
    // USA 12 + 12 + 8.
    assert_eq!(w.entities().soldier_count(), 60);
    for (&id, sq) in g.squads() {
        let base = g.base_of(sq.owner);
        for &m in &sq.members {
            assert_eq!(w.entities().squad()[m.index()], Some(id));
            assert!((pos(&w, m) - base).length() < Fx::from_int(45));
        }
        assert_eq!(sq.nav.phase, NavPhase::Holding);
    }
    let snap = w.snapshot();
    assert_eq!(snap.squads.len(), 6);
    assert_eq!(snap.soldier_squad.len(), 60);
    assert_eq!(snap.soldier_owner.len(), 60);
    assert_eq!(snap.players.len(), 2);
    assert_eq!(snap.players[0].pop, 28);
    assert_eq!(snap.players[0].manpower, 600);
    // The map is real terrain: a ridge at C well above the valley at B.
    let t = w.terrain();
    let c = t.height_at(FxVec2::from_int(512, 512));
    let b = t.height_at(FxVec2::from_int(300, 300));
    assert!(c > b + Fx::from_int(8), "ridge {c} vs valley {b}");
    assert!(t.woods_at(FxVec2::from_int(330, 560)));
    assert!(!t.woods_at(FxVec2::from_int(512, 512)));
}

#[test]
fn move_order_walks_there_and_spreads_in_a_line() {
    let mut w = world(2, 2);
    let sq = squads_of(&w, 0)[0];
    let target = FxVec2::from_int(300, 300); // point B
    w.step(&[cmd(0, 0, CommandKind::Move { squad: sq, target })]);
    {
        let s = w.game().unwrap().squad(sq).unwrap();
        assert_eq!(s.order.kind(), OrderKind::Move);
    }
    let arrived = run_until(&mut w, 30 * 240, |w| {
        w.game().unwrap().squad(sq).unwrap().nav.phase == NavPhase::Holding
    });
    assert!(arrived, "squad never arrived");
    let s = w.game().unwrap().squad(sq).unwrap().clone();
    assert_eq!(s.order, Order::Idle, "move order completes");
    // Men stand on a line through the target, across the move direction,
    // about 3 m apart.
    let facing = s.nav.facing;
    let right = -facing.perp();
    let mut along: Vec<Fx> = Vec::new();
    for &m in &s.members {
        let d = pos(&w, m) - target;
        assert!(d.dot(facing).abs() < Fx::from_int(2), "off the line: {d:?}");
        along.push(d.dot(right));
    }
    along.sort();
    for pair in along.windows(2) {
        let gap = pair[1] - pair[0];
        assert!(gap > Fx::from_int(2) && gap < Fx::from_int(4), "{along:?}");
    }
    // They face the move direction (from the base toward B).
    let base = w.game().unwrap().base_of(PlayerId(0));
    assert!(facing.dot((target - base).normalize_or_zero()) > Fx::from_milli(700));
    let yaw = w.entities().transform()[s.members[3].index()].yaw;
    assert_eq!(yaw, Fx::atan2(facing.y, facing.x));
}

#[test]
fn bad_orders_are_rejected() {
    let mut w = world(3, 1);
    let mine = squads_of(&w, 0);
    let theirs = squads_of(&w, 1);
    let usa_mg = type_index(&w, "ww2:usa_lmg_squad");
    let ger_mg = type_index(&w, "ww2:ger_smg_gruppe");
    let line = vec![FxVec2::from_int(200, 200), FxVec2::from_int(220, 200)];
    let too_long = vec![FxVec2::from_int(200, 200), FxVec2::from_int(270, 200)];
    let target = FxVec2::from_int(500, 500);
    let bad = [
        // Not my squad.
        cmd(
            0,
            0,
            CommandKind::Move {
                squad: theirs[0],
                target,
            },
        ),
        // No such squad.
        cmd(0, 1, CommandKind::Stop { squad: SquadId(99) }),
        // Bench commands.
        cmd(0, 2, CommandKind::Spawn { pos: target }),
        cmd(
            0,
            3,
            CommandKind::MoveSoldier {
                unit: SimId(0),
                target,
            },
        ),
        // Rifles cannot fire a barrage.
        cmd(
            0,
            4,
            CommandKind::Barrage {
                squad: mine[0],
                target,
                rounds: 4,
            },
        ),
        // Trench too long, too few points.
        cmd(
            0,
            5,
            CommandKind::Dig {
                squad: mine[0],
                line: too_long,
                profile: Default::default(),
            },
        ),
        cmd(
            0,
            6,
            CommandKind::Dig {
                squad: mine[0],
                line: vec![line[0]],
                profile: Default::default(),
            },
        ),
        // Another faction's squad type.
        cmd(0, 7, CommandKind::CallIn { squad_type: usa_mg }),
        // Unknown player.
        cmd(5, 0, CommandKind::Surrender),
    ];
    let before = w.stats().commands_rejected;
    w.step(&bad);
    assert_eq!(w.stats().commands_rejected - before, bad.len() as u64);
    assert_eq!(w.stats().commands_applied, 0);
    // The same squads with valid orders are fine.
    w.step(&[
        cmd(
            0,
            8,
            CommandKind::Dig {
                squad: mine[0],
                line,
                profile: Default::default(),
            },
        ),
        cmd(0, 9, CommandKind::CallIn { squad_type: ger_mg }),
    ]);
    assert_eq!(w.stats().commands_applied, 2);
}

#[test]
fn call_in_spawns_at_once_and_starts_its_cooldown() {
    // S3b (owner 2026-11-01): no queue, no delivery time. Manpower is spent
    // and the squad stands in the base in the same tick.
    let mut w = world(4, 2);
    let s = w.match_setup().unwrap().clone();
    let rifle = type_index(&w, "ww2:ger_grenadiergruppe");
    let ty = &s.squad_types[rifle as usize];
    w.step(&[cmd(0, 0, CommandKind::CallIn { squad_type: rifle })]);
    let g = w.game().unwrap();
    assert_eq!(g.players()[0].manpower, s.rules.start_manpower - ty.cost);
    assert_eq!(g.squads().len(), 7);
    assert_eq!(w.entities().soldier_count(), 60 + 10);
    let (&new, sq) = g.squads().last_key_value().unwrap();
    assert_eq!((sq.owner, sq.type_idx), (PlayerId(0), rifle));
    assert_eq!(w.snapshot().players[0].pop, 28 + 10);
    assert_eq!(
        w.snapshot().call_ins,
        [CallInEvent {
            player: PlayerId(0),
            squad_type: rifle,
            result: Ok(new),
        }]
    );
    // Its cooldown runs from the call-in tick; other types are ready.
    assert_eq!(
        g.players()[0].ready_at[rifle as usize],
        ty.recall_cooldown_ticks as u64
    );
    assert_eq!(
        w.snapshot().players[0].cooldowns[rifle as usize],
        ty.recall_cooldown_ticks - 1
    );
    let eng = type_index(&w, "ww2:ger_engineer_squad");
    assert_eq!(w.snapshot().players[0].cooldowns[eng as usize], 0);
    // The members stand inside the base (radius 40 m).
    let base = w.game().unwrap().base_of(PlayerId(0));
    for &m in &w.game().unwrap().squad(new).unwrap().members {
        assert!((pos(&w, m) - base).length() < Fx::from_int(40));
    }
}

#[test]
fn call_ins_are_refused_with_a_reason() {
    let mut w = world(4, 1);
    let s = w.match_setup().unwrap().clone();
    let rifle = type_index(&w, "ww2:ger_grenadiergruppe");
    let eng = type_index(&w, "ww2:ger_engineer_squad");
    let mg = type_index(&w, "ww2:ger_smg_gruppe");
    let usa_mg = type_index(&w, "ww2:usa_lmg_squad");
    let mortar = type_index(&w, "ww2:ger_mortar_team");
    let result = |w: &World| {
        let c = w.snapshot().call_ins.last().copied().unwrap();
        assert_eq!(c.player, PlayerId(0));
        c.result
    };
    // 600 MP: a rifle squad (225) and engineers (200) leave 175 (S5a Tune
    // prices; the sMG-Gruppe at 155 is now affordable, the mortar team at
    // 225 is not).
    w.step(&[
        cmd(0, 0, CommandKind::CallIn { squad_type: rifle }),
        cmd(0, 1, CommandKind::CallIn { squad_type: eng }),
    ]);
    assert_eq!(w.snapshot().call_ins.len(), 2);
    assert!(w.snapshot().call_ins.iter().all(|c| c.result.is_ok()));
    // Same type again: its cooldown, to the tick.
    w.step(&[cmd(0, 2, CommandKind::CallIn { squad_type: rifle })]);
    let cd = s.squad_types[rifle as usize].recall_cooldown_ticks;
    assert_eq!(result(&w), Err(CallInRefusal::Cooldown { ticks: cd - 1 }));
    // Short of manpower: the shortfall.
    w.step(&[cmd(0, 3, CommandKind::CallIn { squad_type: mortar })]);
    let short = s.squad_types[mortar as usize].cost - w.game().unwrap().players()[0].manpower;
    assert_eq!(result(&w), Err(CallInRefusal::Manpower { short }));
    // Another faction's type.
    w.step(&[cmd(0, 4, CommandKind::CallIn { squad_type: usa_mg })]);
    assert_eq!(result(&w), Err(CallInRefusal::NotInRoster));
    // Refusals spend nothing and spawn nothing.
    assert_eq!(w.game().unwrap().squads().len(), 8);
    assert_eq!(w.stats().commands_rejected, 3);

    // Pop cap: 21 men alive + 8 + 5, so a 3-man MG team does not fit 36.
    let mut s2 = setup(4);
    s2.rules.pop_cap = 36;
    s2.rules.start_manpower = 10_000;
    let mut w = World::new_match(s2, 1).unwrap();
    w.step(&[
        cmd(0, 0, CommandKind::CallIn { squad_type: rifle }),
        cmd(0, 1, CommandKind::CallIn { squad_type: eng }),
        cmd(0, 2, CommandKind::CallIn { squad_type: mg }),
    ]);
    assert_eq!(result(&w), Err(CallInRefusal::PopCap));
}

#[test]
fn cooldowns_end_on_their_tick_and_groups_and_start_cooldowns_apply() {
    let mut s = setup(4);
    s.rules.start_manpower = 100_000;
    let rifle = s.squad_type_index("ww2:ger_grenadiergruppe").unwrap();
    let mg = s.squad_type_index("ww2:ger_smg_gruppe").unwrap();
    let mortar = s.squad_type_index("ww2:ger_mortar_team").unwrap();
    // MG and mortar share a timer; the mortar also waits 90 ticks at the
    // start.
    s.squad_types[mg as usize].cooldown_group = Some(0);
    s.squad_types[mortar as usize].cooldown_group = Some(0);
    s.squad_types[mortar as usize].initial_cooldown_ticks = 90;
    let cd = s.squad_types[rifle as usize].recall_cooldown_ticks as u64;
    let mg_cd = s.squad_types[mg as usize].recall_cooldown_ticks as u64;
    let mut w = World::new_match(s, 1).unwrap();
    w.step(&[cmd(0, 0, CommandKind::CallIn { squad_type: mortar })]);
    // S4: a card's own unlock is refused as "locked" (before the cooldown
    // check).
    assert_eq!(
        w.snapshot().call_ins[0].result,
        Err(CallInRefusal::Locked { ticks: 90 })
    );
    w.step(&[
        cmd(0, 1, CommandKind::CallIn { squad_type: rifle }),
        cmd(0, 2, CommandKind::CallIn { squad_type: mg }),
    ]);
    let ready = &w.game().unwrap().players()[0].ready_at;
    assert_eq!(ready[rifle as usize], 1 + cd);
    assert_eq!(ready[mg as usize], 1 + mg_cd);
    assert_eq!(
        ready[mortar as usize],
        1 + mg_cd,
        "the group shares the timer"
    );
    // One tick early: refused; on the tick: bought.
    while w.tick() < cd {
        w.step(&[]);
    }
    w.step(&[cmd(0, 3, CommandKind::CallIn { squad_type: rifle })]);
    assert_eq!(
        w.snapshot().call_ins[0].result,
        Err(CallInRefusal::Cooldown { ticks: 1 })
    );
    w.step(&[cmd(0, 4, CommandKind::CallIn { squad_type: rifle })]);
    assert!(w.snapshot().call_ins[0].result.is_ok());
}

#[test]
fn squads_bought_in_quick_succession_spread_over_the_base() {
    let mut s = setup(4);
    s.rules.start_manpower = 100_000;
    for t in s.squad_types.iter_mut().filter(|t| t.vehicle.is_none()) {
        t.recall_cooldown_ticks = 0;
    }
    let rifle = s.squad_type_index("ww2:ger_grenadiergruppe").unwrap();
    let clear = Fx::from_int(s.rules.call_in.spawn_clear_m as i32);
    let mut w = World::new_match(s, 1).unwrap();
    // The start squads leave the base.
    for (k, id) in squads_of(&w, 0).into_iter().enumerate() {
        w.place_squad(
            id,
            FxVec2::from_int(300 + 30 * k as i32, 300),
            FxVec2::new(Fx::ONE, Fx::ZERO),
        );
    }
    let base = w.game().unwrap().base_of(PlayerId(0));
    let centre = |w: &World, id: SquadId| w.game().unwrap().squad(id).unwrap().nav.goal;
    // Seven rifle squads, one per tick: seven different spots, each clear
    // of the others, all inside the base.
    let mut spots: Vec<FxVec2> = Vec::new();
    for k in 0..7 {
        w.step(&[cmd(0, k, CommandKind::CallIn { squad_type: rifle })]);
        let id = *w.game().unwrap().squads().last_key_value().unwrap().0;
        let at = centre(&w, id);
        // The ring is at half the 40 m base radius (fixed-point sin/cos).
        assert!((at - base).length() <= Fx::from_ratio(201, 10), "{at:?}");
        for &o in &spots {
            assert!((at - o).length() > clear, "squad {k} at {at:?} on {o:?}");
        }
        spots.push(at);
    }
    // An eighth: every spot has men, so it takes one with the fewest.
    w.step(&[cmd(0, 7, CommandKind::CallIn { squad_type: rifle })]);
    let id = *w.game().unwrap().squads().last_key_value().unwrap().0;
    assert!(spots.contains(&centre(&w, id)));
}

#[test]
fn retreat_returns_to_base_and_stop_holds() {
    let mut w = world(5, 2);
    let sq = squads_of(&w, 1)[0];
    w.step(&[cmd(
        1,
        0,
        CommandKind::Move {
            squad: sq,
            target: FxVec2::from_int(724, 724),
        },
    )]);
    for _ in 0..30 * 20 {
        w.step(&[]);
    }
    w.step(&[cmd(1, 1, CommandKind::Stop { squad: sq })]);
    let held: Vec<FxVec2> = w.game().unwrap().squad(sq).unwrap().nav.slots.clone();
    assert_eq!(
        w.game().unwrap().squad(sq).unwrap().nav.phase,
        NavPhase::Holding
    );
    for _ in 0..60 {
        w.step(&[]);
    }
    let s = w.game().unwrap().squad(sq).unwrap();
    for (k, &m) in s.members.iter().enumerate() {
        assert!((pos(&w, m) - held[k]).length() < Fx::from_int(2));
    }
    w.step(&[cmd(1, 2, CommandKind::Retreat { squad: sq })]);
    assert_eq!(w.game().unwrap().squad(sq).unwrap().order, Order::Retreat);
    let base = w.game().unwrap().base_of(PlayerId(1));
    let back = run_until(&mut w, 30 * 200, |w| {
        w.game().unwrap().squad(sq).unwrap().order == Order::Idle
    });
    assert!(back);
    let lead = w.game().unwrap().squad(sq).unwrap().members[0];
    assert!((pos(&w, lead) - base).length() < Fx::from_int(20));
}

#[test]
fn dig_and_barrage_orders_are_stored() {
    let mut w = world(6, 1);
    let eng = squads_of(&w, 0)
        .into_iter()
        .find(|id| {
            let t = w.game().unwrap().squad(*id).unwrap().type_idx;
            w.match_setup().unwrap().squad_types[t as usize].dig_rate_pct > 100
        })
        .unwrap();
    let line = vec![
        FxVec2::from_int(150, 180),
        FxVec2::from_int(165, 185),
        FxVec2::from_int(180, 180),
    ];
    w.step(&[cmd(
        0,
        0,
        CommandKind::Dig {
            squad: eng,
            line: line.clone(),
            profile: Default::default(),
        },
    )]);
    let arrived = run_until(&mut w, 30 * 120, |w| {
        w.game().unwrap().squad(eng).unwrap().nav.phase == NavPhase::Holding
    });
    assert!(arrived);
    let s = w.game().unwrap().squad(eng).unwrap();
    assert!(matches!(&s.order, Order::Dig { line: l, .. } if *l == line));
    // Men stand along the line, facing the nearest enemy (north-east).
    for &m in &s.members {
        let p = pos(&w, m);
        assert!(p.y > Fx::from_int(175) && p.y < Fx::from_int(190), "{p:?}");
    }
    assert!(s.nav.facing.y > Fx::ZERO);

    // Call in a mortar team and give it a barrage.
    let mortar = type_index(&w, "ww2:ger_mortar_team");
    w.step(&[cmd(0, 1, CommandKind::CallIn { squad_type: mortar })]);
    let m = *w.game().unwrap().squads().last_key_value().unwrap().0;
    let target = FxVec2::from_int(400, 400);
    w.step(&[cmd(
        0,
        2,
        CommandKind::Barrage {
            squad: m,
            target,
            rounds: 6,
        },
    )]);
    let s = w.game().unwrap().squad(m).unwrap();
    assert_eq!(s.order, Order::Barrage { target, rounds: 6 });
    assert_eq!(
        w.snapshot().squads.last().unwrap().order,
        OrderKind::Barrage
    );
}

#[test]
fn surrender_ends_the_match() {
    let mut w = world(7, 1);
    let sq = squads_of(&w, 0)[0];
    w.step(&[cmd(1, 0, CommandKind::Surrender)]);
    assert_eq!(
        w.game().unwrap().outcome(),
        Outcome::Won {
            winner: PlayerId(0)
        }
    );
    assert_eq!(w.snapshot().winner, Some(0));
    // No orders after the end.
    w.step(&[cmd(0, 1, CommandKind::Stop { squad: sq })]);
    assert_eq!(w.stats().commands_rejected, 1);
}

#[test]
fn path_budget_spreads_searches_over_ticks() {
    let mut w = tight_world(8, 1);
    // Six long moves in one tick cost more than one tick's budget.
    let mut cmds = Vec::new();
    for p in 0..2u8 {
        let target = if p == 0 {
            FxVec2::from_int(900, 860)
        } else {
            FxVec2::from_int(120, 160)
        };
        for (k, sq) in squads_of(&w, p).into_iter().enumerate() {
            cmds.push(cmd(p, k as u32, CommandKind::Move { squad: sq, target }));
        }
    }
    w.step(&cmds);
    let g = w.game().unwrap();
    let pending = g
        .squads()
        .values()
        .filter(|s| s.nav.phase == NavPhase::PathPending)
        .count();
    let budget = g.setup().rules.movement.path_budget_per_tick;
    assert!(
        pending > 0 || g.path_queue().debt > 0,
        "a {budget}-expansion budget should not serve six cross-map searches in one tick"
    );
    let all = run_until(&mut w, 120, |w| {
        w.game()
            .unwrap()
            .squads()
            .values()
            .all(|s| s.nav.phase == NavPhase::Moving)
    });
    assert!(all, "every request is served within a few seconds");
    // Capped searches give partial paths; squads re-plan at their ends and
    // still arrive.
    assert!(w.game().unwrap().squads().values().any(|s| s.nav.partial));
    let arrived = run_until(&mut w, 30 * 600, |w| {
        w.game()
            .unwrap()
            .squads()
            .values()
            .all(|s| s.nav.phase == NavPhase::Holding)
    });
    assert!(arrived, "partial paths must still lead to the goal");
    for s in w.game().unwrap().squads().values() {
        let lead = pos(&w, s.members[0]);
        assert!(
            (lead - s.nav.goal).length() < Fx::from_int(20),
            "{lead:?} vs {:?}",
            s.nav.goal
        );
    }
}

// ------------------------------------------------------------ determinism --

/// Scripted orders for both players: moves, attack-moves, stops, retreats,
/// a dig, call-ins, and some rejected commands, delivered unsorted.
fn orders_for(w: &World, tick: u64) -> Vec<Command> {
    let mut out = Vec::new();
    let t = tick as u32;
    let points = [
        FxVec2::from_int(220, 700),
        FxVec2::from_int(300, 300),
        FxVec2::from_int(512, 512),
        FxVec2::from_int(724, 724),
        FxVec2::from_int(800, 320),
    ];
    if tick % 90 == 3 {
        for p in 0..2u8 {
            for (k, sq) in squads_of(w, p).into_iter().enumerate() {
                let target = points[(t as usize / 90 + k + p as usize) % points.len()];
                let kind = if k % 2 == 0 {
                    CommandKind::Move { squad: sq, target }
                } else {
                    CommandKind::AttackMove { squad: sq, target }
                };
                out.push(cmd(p, t * 16 + k as u32, kind));
            }
        }
    }
    if tick % 150 == 77 {
        let sq = squads_of(w, 1)[0];
        out.push(cmd(1, t * 16 + 9, CommandKind::Stop { squad: sq }));
        out.push(cmd(0, t * 16 + 9, CommandKind::Stop { squad: sq })); // not owner
    }
    if tick == 200 {
        let sq = squads_of(w, 0)[1];
        out.push(cmd(0, 1, CommandKind::Retreat { squad: sq }));
        let rifle = w
            .match_setup()
            .unwrap()
            .squad_type_index("ww2:usa_rifle_squad")
            .unwrap();
        out.push(cmd(1, 1, CommandKind::CallIn { squad_type: rifle }));
    }
    if tick == 260 {
        let eng = squads_of(w, 1)[2];
        let line = vec![FxVec2::from_int(700, 700), FxVec2::from_int(730, 690)];
        out.push(cmd(
            1,
            2,
            CommandKind::Dig {
                squad: eng,
                line,
                profile: Default::default(),
            },
        ));
    }
    out.reverse();
    out
}

fn run_to(w: &mut World, until: u64) -> Vec<(u64, u64)> {
    let mut out = Vec::new();
    while w.tick() < until {
        let cmds = orders_for(w, w.tick());
        if let Some(h) = w.step(&cmds) {
            out.push((w.tick(), h));
        }
    }
    out
}

#[test]
fn match_incremental_hash_matches_full_recompute() {
    let mut w = world(11, 3);
    assert_eq!(w.state_hash(), w.full_state_hash());
    for _ in 0..400 {
        let cmds = orders_for(&w, w.tick());
        w.step(&cmds);
        assert_eq!(
            w.state_hash(),
            w.full_state_hash(),
            "diverged at tick {}",
            w.tick()
        );
    }
    assert!(w.stats().commands_applied > 10 && w.stats().commands_rejected > 0);
}

#[test]
fn match_thread_count_fuzz_identical_hashes() {
    for make in [world, tight_world] {
        let reference = run_to(&mut make(21, 1), 900);
        assert_eq!(reference.len(), 30);
        for threads in [2, 3, 8] {
            let got = run_to(&mut make(21, threads), 900);
            assert_eq!(got, reference, "hashes differ with {threads} threads");
        }
    }
}

#[test]
fn match_save_load_continue_matches_straight_run() {
    let mut straight = tight_world(31, 2);
    let straight_hashes = run_to(&mut straight, 900);

    let mut first = tight_world(31, 2);
    // Tick 94: right after the tick-93 orders, with path requests still
    // queued or paid for in debt, squads mid-move, and a call-in queued
    // later on.
    run_to(&mut first, 94);
    {
        let g = first.game().unwrap();
        let busy = g.path_queue().debt > 0
            || g.squads()
                .values()
                .any(|s| s.nav.phase == NavPhase::PathPending);
        assert!(busy, "want pending path work at save time");
    }
    let bytes = first.save();
    let mut resumed = World::load(&bytes, 3).unwrap();
    assert_eq!(resumed.state(), first.state());
    assert_eq!(resumed.state_hash(), first.state_hash());
    drop(first);
    let resumed_hashes = run_to(&mut resumed, 900);
    let tail: Vec<_> = straight_hashes
        .into_iter()
        .filter(|(t, _)| *t > 94)
        .collect();
    assert_eq!(resumed_hashes, tail);
    assert_eq!(resumed.save(), straight.save(), "final save bytes differ");
    assert_eq!(resumed.snapshot(), straight.snapshot());
}

#[test]
fn match_runs_are_seed_sensitive() {
    let a = run_to(&mut world(41, 2), 300);
    let b = run_to(&mut world(41, 2), 300);
    let c = run_to(&mut world(42, 2), 300);
    assert_eq!(a, b);
    assert!(a.iter().zip(&c).all(|(x, y)| x.1 != y.1));
}

/// S3b: the moved constants are read from the setup's tuning tables (a
/// changed value changes the match, not just the setup hash).
#[test]
fn tuning_tables_drive_the_sim() {
    // [combat.body] stand_mm: the capsule a soldier spawns with.
    let mut s = setup(4);
    s.rules.combat.body.stand_mm = 1600;
    let w = World::new_match(s, 1).unwrap();
    let id = w.entities().soldiers().next().unwrap();
    assert_eq!(
        w.entities().combat()[id.index()].height,
        Fx::from_milli(1600)
    );
    // [movement.formation] slot_lead_mm: where members walk on a move.
    let walk = |lead_mm: u32| {
        let mut s = setup(4);
        s.rules.movement.formation.slot_lead_mm = lead_mm;
        let mut w = World::new_match(s, 1).unwrap();
        let sq = squads_of(&w, 0)[0];
        let target = FxVec2::from_int(500, 300);
        w.step(&[cmd(0, 0, CommandKind::Move { squad: sq, target })]);
        for _ in 0..300 {
            w.step(&[]);
        }
        let members = w.game().unwrap().squad(sq).unwrap().members.clone();
        members.iter().map(|m| pos(&w, *m)).collect::<Vec<_>>()
    };
    assert_eq!(walk(1500), walk(1500));
    assert_ne!(walk(1500), walk(500));
}

#[test]
fn ai_vs_ai_continues_identically_after_a_save_mid_cooldown() {
    // S3b: re-call cooldowns are match state. An AI-vs-AI match saved while
    // both players wait on a cooldown continues with the same hashes at
    // another thread count, and the same as an unbroken run at 1 and 8.
    let mut s = setup(8);
    s.players.iter_mut().for_each(|p| p.is_ai = true);
    let mut w = World::new_match(s.clone(), 2).unwrap();
    let waiting = |w: &World| {
        let t = w.tick();
        w.game()
            .unwrap()
            .players()
            .iter()
            .all(|p| p.ready_at.iter().any(|&r| r > t + 30))
    };
    assert!(run_until(&mut w, 30 * 120, waiting));
    let at = w.tick();
    let mut loaded = World::load(&w.save(), 3).unwrap();
    assert_eq!(loaded.full_state_hash(), w.full_state_hash());
    let mut bought = 0;
    for _ in 0..30 * 90 {
        assert_eq!(w.step(&[]), loaded.step(&[]), "tick {}", w.tick());
        bought += w
            .snapshot()
            .call_ins
            .iter()
            .filter(|c| c.result.is_ok())
            .count();
        assert_eq!(w.snapshot().call_ins, loaded.snapshot().call_ins);
    }
    assert_eq!(w.full_state_hash(), loaded.full_state_hash());
    assert!(bought > 0, "the AI bought again after its cooldowns");
    for threads in [1, 8] {
        let mut other = World::new_match(s.clone(), threads).unwrap();
        while other.tick() < at + 30 * 90 {
            other.step(&[]);
        }
        assert_eq!(
            other.full_state_hash(),
            w.full_state_hash(),
            "{threads} threads"
        );
    }
}
