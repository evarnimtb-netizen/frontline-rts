//! S10a-ai tests (docs/slices/S10a-ai.md 7): AI fort state, lost
//! materials, the S17 wire path step test, the wire cell index.

use frontline_content::setup::Capability;

use super::s6_tests::{flat_world_with, spawn, steps};
use super::*;
use crate::ai::{AiBreach, AiFortJob, BreachPhase, FortPhase, NO_POINT};
use crate::game::SquadId;
use crate::path::PathResult;

fn c(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

fn fort(w: &World, id: &str) -> u16 {
    w.match_setup()
        .unwrap()
        .forts
        .iter()
        .position(|f| f.id == id)
        .unwrap() as u16
}

/// Player 1 wire from `a` to `b`, then the sections whose middle lies in
/// `[x0, x1]` removed (a gap).
fn wire_with_gap(w: &mut World, a: FxVec2, b: FxVec2, x0: Fx, x1: Fx) {
    let f = fort(w, "ww2:barbed_wire");
    assert!(w.build_now(f, &[a, b], PlayerId(1)));
    w.edit_fort(|s| {
        s.wire.retain(|_, q| {
            let mx = (q.a.x + q.b.x).div_int(2);
            mx < x0 || mx > x1
        })
    });
}

/// One infantry path search as `serve_paths` makes it.
fn foot_path(w: &World, from: FxVec2, to: FxVec2) -> PathResult {
    let g = w.game().unwrap();
    let terrain = &w.state.terrain;
    let mv = &g.setup().rules.movement;
    let cost = crate::path::PathCost {
        slope_cost_pct: mv.slope_cost_pct,
        max_slope_pct: mv.max_slope_pct,
        trench_cost_pct: mv.trench_cost_pct,
        ditch_cost_pct: mv.ditch_cost_pct,
    };
    let nodes = g.fort().wire_nodes(&g.setup().forts, terrain.dim());
    let wire = crate::path::WireCost {
        nodes: &nodes,
        cost_pct: g.setup().rules.fortify.as_ref().unwrap().wire_path_cost_pct,
    };
    let mut s = crate::path::PathScratch::default();
    crate::path::find_foot_path(
        terrain,
        from,
        to,
        &cost,
        None,
        Some(wire),
        mv.path_max_expansions,
        &mut s,
    )
}

/// The path-search cost the Build A hand-over reports (a 20 m line with a
/// 4 m gap): expansions and the time of one search (`--ignored
/// --nocapture`; report only).
#[test]
#[ignore]
#[allow(clippy::disallowed_methods)] // report timing only, never fed to the sim
fn wire_path_cost_report() {
    let mut w = flat_world_with(1, |_| {});
    steps(&mut w, 1);
    wire_with_gap(
        &mut w,
        c(490, 500),
        c(510, 500),
        Fx::from_int(498),
        Fx::from_int(502),
    );
    let (from, to) = (c(500, 470), c(500, 530));
    let r = foot_path(&w, from, to);
    let n = 2000;
    let t0 = std::time::Instant::now();
    for _ in 0..n {
        std::hint::black_box(foot_path(&w, from, to));
    }
    let us = t0.elapsed().as_secs_f64() * 1e6 / n as f64;
    println!(
        "wire path cost: {} expansions, reached {}, {:.1} us per search (layer build included), points {:?}",
        r.expansions, r.reached, us, r.points
    );
}

// ------------------------------------------------- layout: save, hash --

/// The flat ww2 world with player 1 an AI; engineer squads get the field
/// engineer caps (as `s10a_tests::world`).
fn ai_world() -> World {
    flat_world_with(1, |s| {
        s.players[1].is_ai = true;
        for t in s.squad_types.iter_mut() {
            if t.id.contains("engineer") {
                t.caps |= Capability::BuildStructure.bit()
                    | Capability::PlaceWire.bit()
                    | Capability::ClearWire.bit();
            }
        }
    })
}

fn ai1(w: &mut World) -> &mut crate::ai::AiState {
    w.state.game.as_mut().unwrap().ai[1].as_mut().unwrap()
}

/// An AI world with every S10a-ai field set: a fort job and a breach job
/// keyed by player 1 squads, known wire, marks, a material run, counters
/// and the new account fields. Returns it with the two squads.
fn ai_busy() -> (World, SquadId, SquadId) {
    let mut w = ai_world();
    steps(&mut w, 3);
    let a = spawn(&mut w, 1, "ww2:usa_engineer_squad", 600, 600);
    let b = spawn(&mut w, 1, "ww2:usa_rifle_squad", 620, 600);
    let wall = fort(&w, "ww2:sandbag_wall");
    let tick = w.state.tick;
    let ai = ai1(&mut w);
    ai.forts.insert(
        a,
        AiFortJob {
            fort: wall,
            point: 0,
            line: vec![c(600, 610), c(604, 610)],
            phase: FortPhase::Go,
            since: tick,
            work: Some(3),
            tries: 1,
        },
    );
    ai.breaches.insert(
        b,
        AiBreach {
            point: 1,
            crossing: c(400, 400),
            section: 7,
            phase: BreachPhase::Suppress,
            since: tick,
            tries: 2,
            suppress_at: Some(tick),
            smoke_at: None,
        },
    );
    ai.wire_known = vec![2, 5, 9];
    ai.wire_cursor = 6;
    ai.shelled.insert(0, tick);
    ai.enemy_mortar_seen = true;
    ai.bypass.insert(1, tick);
    ai.material_run = Some((a, 3));
    ai.counters.fort_jobs = 1;
    ai.counters.material_runs = [1, 2, 3];
    ai.counters.wire_bypassed = 4;
    w.edit_fort(|f| {
        let acc = &mut f.accounts[1];
        acc.materials_lost_g = 5_000;
        acc.men_lost_in_wire = 2;
        acc.works_done = vec![1, 0, 2];
        acc.materials_used_g = 9_000;
        acc.materials_used_crate_g = 4_000;
        acc.materials_used_truck_g = 3_000;
    });
    (w, a, b)
}

/// An edit of the whole saved state (hash and load tests).
type StateEdit = Box<dyn Fn(&mut SimState)>;

fn ai_of(s: &mut SimState) -> &mut crate::ai::AiState {
    s.game.as_mut().unwrap().ai[1].as_mut().unwrap()
}

fn acc_of(s: &mut SimState) -> &mut crate::fort::FortAccount {
    &mut s.game.as_mut().unwrap().fort.accounts[1]
}

#[test]
fn ai_fort_state_saves_loads_and_hashes() {
    let (w, _, _) = ai_busy();
    let bytes = w.save();
    let back = World::load(&bytes, 1).expect("the full S10a-ai state loads");
    assert_eq!(back.full_state_hash(), w.full_state_hash());
    assert_eq!(
        back.game().unwrap().ai[1],
        w.game().unwrap().ai[1],
        "AI state round trip"
    );
    assert_eq!(back.game().unwrap().fort(), w.game().unwrap().fort());
    // Each new field is in the hash.
    let h = back.full_state_hash();
    let edits: Vec<StateEdit> = vec![
        Box::new(|s| ai_of(s).forts.values_mut().next().unwrap().tries += 1),
        Box::new(|s| ai_of(s).forts.values_mut().next().unwrap().phase = FortPhase::Watch),
        Box::new(|s| ai_of(s).breaches.values_mut().next().unwrap().section += 1),
        Box::new(|s| ai_of(s).breaches.values_mut().next().unwrap().smoke_at = Some(1)),
        Box::new(|s| ai_of(s).wire_known.push(11)),
        Box::new(|s| ai_of(s).wire_cursor += 1),
        Box::new(|s| {
            ai_of(s).shelled.insert(1, 1);
        }),
        Box::new(|s| ai_of(s).enemy_mortar_seen = false),
        Box::new(|s| {
            ai_of(s).bypass.insert(0, 1);
        }),
        Box::new(|s| ai_of(s).material_run = None),
        Box::new(|s| ai_of(s).counters.breaches_opened += 1),
        Box::new(|s| ai_of(s).counters.material_runs[2] += 1),
        Box::new(|s| acc_of(s).materials_lost_g += 1),
        Box::new(|s| acc_of(s).men_lost_in_wire += 1),
        Box::new(|s| acc_of(s).works_done[1] += 1),
        Box::new(|s| acc_of(s).materials_used_crate_g += 1),
        Box::new(|s| acc_of(s).materials_used_truck_g += 1),
        Box::new(|s| s.game.as_mut().unwrap().setup.rules.ai.fort.wire_out_m += 1),
    ];
    for (k, e) in edits.into_iter().enumerate() {
        // Through a save and load: the setup's hash bytes are made there.
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        let y = World::load(&x.save(), 1).unwrap_or_else(|e| panic!("edit {k}: {e:?}"));
        assert_ne!(y.full_state_hash(), h, "edit {k} is not hashed");
    }
}

/// Player 1's squads, ascending.
fn own_squads(s: &mut SimState) -> Vec<SquadId> {
    let g = s.game.as_ref().unwrap();
    g.squads
        .iter()
        .filter(|(_, q)| q.owner == PlayerId(1))
        .map(|(&id, _)| id)
        .collect()
}

#[test]
fn load_rejects_bad_ai_fort_state() {
    let (w, a, _) = ai_busy();
    let bytes = w.save();
    // Positive control: the busy state itself loads (above), and a job
    // whose squad is gone since the last plan still loads.
    let mut gone = World::load(&bytes, 1).unwrap();
    let men = gone.game().unwrap().squads[&a].members.clone();
    for m in men {
        assert!(gone.kill_soldier(m));
    }
    assert!(!gone.game().unwrap().squads.contains_key(&a));
    assert!(
        World::load(&gone.save(), 1).is_ok(),
        "a job of a squad just lost"
    );
    let enemy = spawn(
        &mut World::load(&bytes, 1).unwrap(),
        0,
        "ww2:ger_grenadiergruppe",
        300,
        300,
    );
    let bad: Vec<StateEdit> = vec![
        Box::new(|s| {
            let j = ai_of(s).forts.values().next().unwrap().clone();
            for k in own_squads(s) {
                ai_of(s).forts.insert(k, j.clone());
            }
        }),
        Box::new(|s| ai_of(s).forts.values_mut().next().unwrap().fort = 999),
        Box::new(|s| ai_of(s).forts.values_mut().next().unwrap().point = 9),
        Box::new(|s| ai_of(s).forts.values_mut().next().unwrap().line.clear()),
        Box::new(|s| ai_of(s).forts.values_mut().next().unwrap().line = vec![c(5000, 1); 2]),
        Box::new(|s| ai_of(s).forts.values_mut().next().unwrap().line = vec![c(1, 1); 17]),
        Box::new(|s| ai_of(s).forts.values_mut().next().unwrap().since = u64::MAX),
        Box::new(|s| ai_of(s).forts.values_mut().next().unwrap().tries = 200),
        Box::new(move |s| {
            let j = ai_of(s).forts.values().next().unwrap().clone();
            ai_of(s).forts.insert(enemy, j);
        }),
        Box::new(|s| {
            let j = ai_of(s).forts.values().next().unwrap().clone();
            ai_of(s).forts.insert(SquadId(100_000), j);
        }),
        Box::new(|s| ai_of(s).breaches.values_mut().next().unwrap().point = NO_POINT),
        Box::new(|s| ai_of(s).breaches.values_mut().next().unwrap().crossing = c(-1, 5)),
        Box::new(|s| ai_of(s).breaches.values_mut().next().unwrap().tries = 11),
        Box::new(|s| ai_of(s).breaches.values_mut().next().unwrap().suppress_at = Some(u64::MAX)),
        Box::new(|s| {
            let b = ai_of(s).breaches.values().next().unwrap().clone();
            for k in own_squads(s) {
                ai_of(s).breaches.insert(k, b.clone());
            }
        }),
        Box::new(|s| ai_of(s).wire_known = vec![5, 2]),
        Box::new(|s| ai_of(s).wire_known = vec![3, 3]),
        Box::new(|s| ai_of(s).wire_known = (0..257).collect()),
        Box::new(|s| {
            ai_of(s).shelled.insert(9, 1);
        }),
        Box::new(|s| {
            ai_of(s).shelled.insert(0, u64::MAX);
        }),
        Box::new(|s| {
            ai_of(s).bypass.insert(9, 1);
        }),
        Box::new(move |s| ai_of(s).material_run = Some((enemy, 1))),
        Box::new(|s| acc_of(s).works_done = vec![0; 99]),
        Box::new(|s| acc_of(s).materials_used_truck_g = 7_000),
    ];
    for (k, e) in bad.into_iter().enumerate() {
        let mut x = World::load(&bytes, 1).unwrap();
        if k == 8 || k == 21 {
            // The enemy squad must exist in the edited world too.
            spawn(&mut x, 0, "ww2:ger_grenadiergruppe", 300, 300);
        }
        if k == 0 || k == 14 {
            // Enough own squads that only the cap is broken.
            for n in 0..17 {
                spawn(&mut x, 1, "ww2:usa_rifle_squad", 700 + 4 * n, 700);
            }
            assert!(own_squads(&mut x.state).len() > 17);
        }
        e(&mut x.state);
        assert!(World::load(&x.save(), 1).is_err(), "bad state {k} loaded");
    }
}

// ------------------------------------------------ lost and by-kind kg --

fn acc(w: &World, p: usize) -> crate::fort::FortAccount {
    w.game().unwrap().fort().accounts[p].clone()
}

fn members(w: &World, s: SquadId) -> Vec<SimId> {
    w.game().unwrap().squads[&s].members.clone()
}

#[test]
fn fort_account_counts_lost_materials() {
    let mut w = ai_world();
    // Dead carriers: one man's share, then the rest with the squad.
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 500, 500);
    let men = members(&w, eng);
    let per = 20_000u64;
    let full = per * men.len() as u64;
    w.edit_fort(|f| {
        f.carried_g.insert(eng, full as u32);
    });
    assert_eq!(acc(&w, 0).materials_lost_g, 0);
    assert!(w.kill_soldier(men[0]));
    assert_eq!(acc(&w, 0).materials_lost_g, per, "one man's share");
    for &m in &men[1..] {
        assert!(w.kill_soldier(m));
    }
    assert_eq!(acc(&w, 0).materials_lost_g, full, "all of it");
    assert_eq!(acc(&w, 1).materials_lost_g, 0, "never the enemy's");
    // A wrecked truck.
    let truck = spawn(&mut w, 0, "ww2:ger_opel_blitz_squad", 510, 500);
    let tv = members(&w, truck)[0];
    w.edit_vehicle(tv, |v| v.material_g = 80_000);
    assert!(w.wreck_vehicle(tv));
    assert_eq!(acc(&w, 0).materials_lost_g, full + 80_000);
    // A removed work: the kg it drew.
    let wall = fort(&w, "ww2:sandbag_wall");
    let eng2 = spawn(&mut w, 0, "ww2:ger_engineer_squad", 520, 520);
    w.edit_fort(|f| {
        f.carried_g.insert(eng2, 30_000);
    });
    super::s6_tests::run(
        &mut w,
        0,
        0,
        CommandKind::Fortify {
            squad: eng2,
            fort: wall,
            line: vec![c(520, 525), c(522, 525)],
        },
    );
    steps(&mut w, 5 * 30);
    let got = w
        .game()
        .unwrap()
        .fort()
        .works
        .values()
        .next()
        .expect("building")
        .got_g as u64;
    assert!(got > 0);
    let before = acc(&w, 0).materials_lost_g;
    super::s6_tests::run(
        &mut w,
        0,
        1,
        CommandKind::Move {
            squad: eng2,
            target: c(540, 540),
        },
    );
    steps(&mut w, 1);
    assert!(w.game().unwrap().fort().works.is_empty());
    let left = w
        .game()
        .unwrap()
        .fort()
        .carried_g
        .get(&eng2)
        .copied()
        .unwrap_or(0) as u64;
    assert_eq!(
        acc(&w, 0).materials_lost_g,
        before + got,
        "drawn kg lost; {left} g still carried"
    );
    assert_eq!(acc(&w, 0).materials_used_g, got);
    assert_eq!(
        acc(&w, 0).materials_used_crate_g + acc(&w, 0).materials_used_truck_g,
        0,
        "carried"
    );
}

#[test]
fn works_done_and_kg_by_source_are_counted() {
    let mut w = ai_world();
    let wall = fort(&w, "ww2:sandbag_wall");
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 500, 500);
    let truck = spawn(&mut w, 0, "ww2:ger_opel_blitz_squad", 505, 500);
    let tv = members(&w, truck)[0];
    w.edit_vehicle(tv, |v| v.material_g = 80_000);
    super::s6_tests::run(
        &mut w,
        0,
        0,
        CommandKind::Fortify {
            squad: eng,
            fort: wall,
            line: vec![c(500, 505), c(502, 505)],
        },
    );
    steps(&mut w, 90 * 30);
    let a = acc(&w, 0);
    assert!(
        a.works_done.get(wall as usize).copied() == Some(1),
        "{:?}",
        a.works_done
    );
    assert!(a.materials_used_g > 0);
    assert_eq!(
        a.materials_used_truck_g, a.materials_used_g,
        "all from the truck"
    );
    assert_eq!(a.materials_used_crate_g, 0);
}

#[test]
fn men_killed_in_enemy_wire_are_counted() {
    let mut w = ai_world();
    let f = fort(&w, "ww2:barbed_wire");
    assert!(w.build_now(f, &[c(480, 500), c(520, 500)], PlayerId(1)));
    let ger = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 500, 500);
    let usa = spawn(&mut w, 1, "ww2:usa_rifle_squad", 500, 500);
    let place = |w: &mut World, m: SimId, x: i32, y: i32| {
        w.state.entities.transform[m.index()].pos.x = Fx::from_int(x);
        w.state.entities.transform[m.index()].pos.y = Fx::from_int(y);
    };
    let g = members(&w, ger);
    let u = members(&w, usa);
    // Outside the band (0.75 m half width): no count.
    place(&mut w, g[0], 500, 502);
    assert!(w.kill_soldier(g[0]));
    assert_eq!(acc(&w, 0).men_lost_in_wire, 0);
    // In his own side's wire: no count.
    place(&mut w, u[0], 500, 500);
    assert!(w.kill_soldier(u[0]));
    assert_eq!(acc(&w, 1).men_lost_in_wire, 0);
    // Inside the enemy band: counted.
    place(&mut w, g[1], 501, 500);
    assert!(w.kill_soldier(g[1]));
    assert_eq!(acc(&w, 0).men_lost_in_wire, 1);
    // Past the line's end: no count.
    place(&mut w, g[2], 530, 500);
    assert!(w.kill_soldier(g[2]));
    assert_eq!(acc(&w, 0).men_lost_in_wire, 1);
}
