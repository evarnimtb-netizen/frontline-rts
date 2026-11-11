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

/// One infantry path search as `serve_paths` makes it (the index
/// refreshed first, as there).
fn foot_path(w: &mut World, from: FxVec2, to: FxVec2) -> PathResult {
    let dim = w.state.terrain.dim();
    {
        let g = w.state.game.as_mut().unwrap();
        g.fort.refresh_wire_index(&g.setup.forts, dim);
    }
    foot_path_ro(w, from, to)
}

fn foot_path_ro(w: &World, from: FxVec2, to: FxVec2) -> PathResult {
    let g = w.game().unwrap();
    let terrain = &w.state.terrain;
    let mv = &g.setup().rules.movement;
    let cost = crate::path::PathCost {
        slope_cost_pct: mv.slope_cost_pct,
        max_slope_pct: mv.max_slope_pct,
        trench_cost_pct: mv.trench_cost_pct,
        ditch_cost_pct: mv.ditch_cost_pct,
    };
    let (fort, forts) = (g.fort(), &g.setup().forts);
    let nodes = fort.path_nodes(forts, terrain.dim());
    let crosses = |a: FxVec2, b: FxVec2| fort.step_in_wire(forts, a, b);
    let wire = crate::path::WireCost {
        nodes: &nodes,
        cost_pct: g.setup().rules.fortify.as_ref().unwrap().wire_path_cost_pct,
        crosses: &crosses,
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

/// Each leg of a smoothed path from `from` that meets a wire band.
fn wired_legs(w: &World, from: FxVec2, r: &PathResult) -> Vec<(FxVec2, FxVec2)> {
    let g = w.game().unwrap();
    let mut prev = from;
    let mut out = Vec::new();
    for &p in &r.points {
        if g.fort().step_in_wire(&g.setup().forts, prev, p) {
            out.push((prev, p));
        }
        prev = p;
    }
    out
}

/// Where the path from `from` crosses the line y = `y` (x values).
fn crossings_at(from: FxVec2, r: &PathResult, y: Fx) -> Vec<Fx> {
    let mut prev = from;
    let mut out = Vec::new();
    for &p in &r.points {
        if (prev.y - y).raw().signum() != (p.y - y).raw().signum() && prev.y != p.y {
            let t = (y - prev.y) / (p.y - prev.y);
            out.push(prev.x + (p.x - prev.x) * t);
        }
        prev = p;
    }
    out
}

#[test]
fn path_finds_a_2m_and_a_4m_wire_gap() {
    // A 2 m gap (one section, 0.5 m clear of both bands) on a node
    // column, and a 4 m gap (two sections) off the node columns, each in
    // an 80 m line 2 m off a node row. The S10a layer (every node within
    // half a band + 2 m) priced both gaps and the path went round the line.
    for (gap0, gap1, label) in [(499, 501, "2 m"), (501, 505, "4 m")] {
        let mut w = flat_world_with(1, |_| {});
        steps(&mut w, 1);
        wire_with_gap(
            &mut w,
            c(461, 502),
            c(541, 502),
            Fx::from_int(gap0),
            Fx::from_int(gap1),
        );
        let n = w.game().unwrap().fort().wire.len();
        assert_eq!(n, 40 - (gap1 - gap0) as usize / 2, "{label}");
        // From 12 m beside the gap: straight across costs two wired steps
        // (500 %), the gap a short detour, so only a free gap draws it.
        let x = (gap0 + gap1) / 2 - 12;
        let from = c(x, 470);
        let r = foot_path(&mut w, from, c(x, 534));
        assert!(r.reached, "{label}");
        let xs = crossings_at(from, &r, Fx::from_int(502));
        assert_eq!(xs.len(), 1, "{label}: {:?}", r.points);
        assert!(
            xs[0] > Fx::from_int(gap0) && xs[0] < Fx::from_int(gap1),
            "{label} gap not taken: crossed at {} ({:?})",
            xs[0],
            r.points
        );
        assert!(wired_legs(&w, from, &r).is_empty(), "{label}: touches wire");
    }
}

#[test]
fn path_never_steps_across_standing_wire_beside_a_gap() {
    // (a) One 2 m section across the middle of a diagonal grid step
    // (416,416)-(420,420): both nodes lie 2.83 m from it, outside the S10a
    // layer (half a band + 2 m = 2.75 m), so that step crossed it unpriced.
    let mut w = flat_world_with(1, |_| {});
    steps(&mut w, 1);
    let f = fort(&w, "ww2:barbed_wire");
    let half = Fx::from_ratio(707, 1000);
    w.edit_fort(|s| {
        let id = s.next_wire;
        s.next_wire += 1;
        s.wire.insert(
            id,
            crate::fort::WireSection {
                owner: PlayerId(1),
                fort: f,
                a: FxVec2::new(Fx::from_int(418) - half, Fx::from_int(418) + half),
                b: FxVec2::new(Fx::from_int(418) + half, Fx::from_int(418) - half),
                hp: 200,
            },
        );
    });
    let from = c(400, 400);
    let r = foot_path(&mut w, from, c(440, 440));
    assert!(r.reached);
    assert_eq!(wired_legs(&w, from, &r), vec![], "path {:?}", r.points);
    // Control: the straight line does meet the section.
    let g = w.game().unwrap();
    assert!(g.fort().step_in_wire(&g.setup().forts, from, c(440, 440)));
    // (b) A 20 m line with a 2 m gap between node columns (no grid step
    // passes it 0.75 m clear): the path goes round the line, never
    // through the gap's bands.
    let mut w = flat_world_with(1, |_| {});
    steps(&mut w, 1);
    wire_with_gap(
        &mut w,
        c(491, 502),
        c(511, 502),
        Fx::from_int(501),
        Fx::from_int(503),
    );
    assert_eq!(w.game().unwrap().fort().wire.len(), 9);
    let from = c(502, 470);
    let r = foot_path(&mut w, from, c(502, 534));
    assert!(r.reached);
    assert_eq!(wired_legs(&w, from, &r), vec![], "path {:?}", r.points);
    let xs = crossings_at(from, &r, Fx::from_int(502));
    assert!(
        xs.iter()
            .all(|&x| x < Fx::from_int(491) || x > Fx::from_int(511)),
        "went through: {xs:?}"
    );
}

/// A tiny deterministic generator for the index test.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self, n: u32) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) % n as u64) as u32
    }
}

#[test]
fn wire_index_matches_a_full_scan() {
    let mut w = flat_world_with(1, |_| {});
    steps(&mut w, 1);
    let f = fort(&w, "ww2:barbed_wire");
    let dim = w.state.terrain.dim();
    let mut rng = Lcg(7);
    // Every query of the index against the full scan (a clone whose index
    // is stale); returns how many answers were true.
    let check = |w: &World, rng: &mut Lcg| -> u32 {
        let g = w.game().unwrap();
        let forts = &g.setup().forts;
        let fresh = g.fort();
        assert!(fresh.index_fresh(), "the index was not rebuilt");
        let mut scan = fresh.clone();
        scan.wire_index = Default::default();
        assert!(!scan.index_fresh());
        assert_eq!(fresh.path_nodes(forts, dim), scan.path_nodes(forts, dim));
        let mut hits = 0;
        for _ in 0..3000 {
            // Points near the sections (a 64 m square at the map's middle).
            let p = |r: &mut Lcg| {
                FxVec2::new(
                    Fx::from_raw((480 << 16) + r.next(64 << 16) as i32),
                    Fx::from_raw((480 << 16) + r.next(64 << 16) as i32),
                )
            };
            let (a, b) = (p(rng), p(rng));
            let b = a + (b - a).div_int(8);
            let pl = PlayerId(rng.next(2) as u8);
            let q = [
                fresh.in_wire(forts, a),
                fresh.in_enemy_wire(forts, a, pl),
                fresh.step_in_wire(forts, a, b),
            ];
            let s = [
                scan.in_wire(forts, a),
                scan.in_enemy_wire(forts, a, pl),
                scan.step_in_wire(forts, a, b),
            ];
            assert_eq!(q, s, "at {a:?} -> {b:?}");
            hits += q.iter().filter(|&&x| x).count() as u32;
        }
        hits
    };
    let refresh = |w: &mut World| {
        let g = w.state.game.as_mut().unwrap();
        g.fort.refresh_wire_index(&g.setup.forts, dim);
    };
    // Random sections of both owners, any direction, some across cell
    // edges (16 m).
    let add = |w: &mut World, rng: &mut Lcg, n: u32| {
        w.edit_fort(|s| {
            for _ in 0..n {
                let a = FxVec2::new(
                    Fx::from_raw((480 << 16) + rng.next(64 << 16) as i32),
                    Fx::from_raw((480 << 16) + rng.next(64 << 16) as i32),
                );
                let d = FxVec2::new(
                    Fx::from_raw(rng.next(4 << 16) as i32 - (2 << 16)),
                    Fx::from_raw(rng.next(4 << 16) as i32 - (2 << 16)),
                );
                let id = s.next_wire;
                s.next_wire += 1;
                s.wire.insert(
                    id,
                    crate::fort::WireSection {
                        owner: PlayerId((id % 2) as u8),
                        fort: f,
                        a,
                        b: a + d,
                        hp: 200,
                    },
                );
            }
        });
    };
    add(&mut w, &mut rng, 60);
    refresh(&mut w);
    let hits = check(&w, &mut rng);
    assert!(hits > 100, "a vacuous test: {hits} hits");
    // Adds.
    add(&mut w, &mut rng, 30);
    assert!(
        !w.game().unwrap().fort().index_fresh(),
        "an add makes it stale"
    );
    refresh(&mut w);
    check(&w, &mut rng);
    // Removals.
    w.edit_fort(|s| {
        let ids: Vec<u32> = s.wire.keys().copied().step_by(3).collect();
        for id in ids {
            s.remove_wire(id, None, crate::fort::WireGone::Broken);
        }
    });
    assert!(
        !w.game().unwrap().fort().index_fresh(),
        "a removal makes it stale"
    );
    refresh(&mut w);
    check(&w, &mut rng);
    // After load: stale at first (not saved), the same answers once the
    // first tick's path service rebuilt it.
    let mut back = World::load(&w.save(), 1).unwrap();
    assert!(!back.game().unwrap().fort().index_fresh());
    steps(&mut back, 1);
    assert_eq!(back.full_state_hash(), {
        steps(&mut w, 1);
        w.full_state_hash()
    });
    refresh(&mut back);
    check(&back, &mut rng);
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
    let r = foot_path(&mut w, from, to);
    let n = 2000;
    let t0 = std::time::Instant::now();
    for _ in 0..n {
        std::hint::black_box(foot_path_ro(&w, from, to));
    }
    let us = t0.elapsed().as_secs_f64() * 1e6 / n as f64;
    println!(
        "wire path cost: {} expansions, reached {}, {:.1} us per search, points {:?}",
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

// ============================================ Build B: buys and building --

use crate::ai::AiTrench;
use crate::command::CommandKind;

/// Seconds of play.
fn play(w: &mut World, secs: u64) {
    steps(w, secs * 30);
}

/// A world where AI player 1 (USA, no start squads, no buys) holds the
/// map's only point with a finished AI trench across the way to player 0's
/// base. Returns the world, the point and the trench line.
fn held(edit: impl FnOnce(&mut frontline_content::MatchSetup)) -> (World, FxVec2, [FxVec2; 2]) {
    held_dug(true, edit)
}

/// [`held`], the trench finished only when `dug`.
fn held_dug(
    dug: bool,
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> (World, FxVec2, [FxVec2; 2]) {
    let mut w = flat_world_with(1, |s| {
        s.players[1].is_ai = true;
        s.map.points.truncate(1);
        let f = s.players[1].faction as usize;
        s.factions[f].start_squads.clear();
        // No buys: no card has a weight.
        s.squad_types.iter_mut().for_each(|t| t.ai_weight = 0);
        edit(s);
    });
    let (pos, line) = {
        let g = w.state.game.as_mut().unwrap();
        g.points[0].owner = Some(PlayerId(1));
        let s = &g.setup;
        let pos: FxVec2 = s.map.points[0].pos.into();
        let dir = (g.base_of(PlayerId(0)) - pos).normalize_or_zero();
        let mid = pos + dir * Fx::from(s.rules.ai.dig_offset);
        let half = dir.perp() * Fx::from(s.rules.ai.dig_length).div_int(2);
        let line = [mid - half, mid + half];
        g.ai[1]
            .as_mut()
            .unwrap()
            .trenches
            .push(AiTrench { point: 0, line });
        (pos, line)
    };
    if dug {
        trench_done(&mut w, line);
    }
    (w, pos, line)
}

/// The trench's middle dug to depth (what `evaluate_points` reads).
fn trench_done(w: &mut World, line: [FxVec2; 2]) {
    let depth = Fx::from(w.match_setup().unwrap().rules.dig.depth) + Fx::from_milli(300);
    let tick = w.state.tick;
    let mid = (line[0] + line[1]).div_int(2);
    w.state
        .terrain
        .crater_cut(mid, Fx::from_int(3), depth, tick);
}

/// A player 1 squad at `at` carrying `kg`.
fn squad_at(w: &mut World, card: &str, at: FxVec2, kg: u32) -> SquadId {
    let (x, y) = (at.x.floor_int(), at.y.floor_int());
    let id = spawn(w, 1, card, x, y);
    if kg > 0 {
        w.edit_fort(|f| {
            f.carried_g.insert(id, kg * 1000);
        });
    }
    id
}

fn ai_ro(w: &World) -> &crate::ai::AiState {
    w.game().unwrap().ai[1].as_ref().unwrap()
}

/// Player 1's own pieces (part 0) and wire sections of blueprint `f`.
fn built(w: &World, f: u16) -> Vec<FxVec2> {
    let fs = w.game().unwrap().fort();
    fs.pieces
        .values()
        .filter(|q| q.owner == PlayerId(1) && q.fort == f && q.part == 0)
        .map(|q| q.pos)
        .chain(
            fs.wire
                .values()
                .filter(|s| s.owner == PlayerId(1) && s.fort == f)
                .map(|s| (s.a + s.b).div_int(2)),
        )
        .collect()
}

/// The nest site of point 0 (index 0: the trench's second end, spec 3.3).
fn nest_site(w: &World, line: [FxVec2; 2]) -> FxVec2 {
    let off = w.match_setup().unwrap().rules.ai.fort.nest_offset_m as i32;
    line[1] + (line[1] - line[0]).normalize_or_zero() * Fx::from_int(off)
}

/// Plays up to `secs`, stopping once `done` holds; whether it did.
fn play_until(w: &mut World, secs: u64, mut done: impl FnMut(&World) -> bool) -> bool {
    for _ in 0..secs {
        if done(w) {
            return true;
        }
        play(w, 1);
    }
    done(w)
}

/// Every AI command player 1 has pending for squad `id` (sampled).
fn pending_for(w: &World, id: SquadId) -> Vec<CommandKind> {
    ai_ro(w)
        .pending
        .iter()
        .filter(|c| match &c.kind {
            CommandKind::Move { squad, .. }
            | CommandKind::AttackMove { squad, .. }
            | CommandKind::Dig { squad, .. }
            | CommandKind::Fortify { squad, .. }
            | CommandKind::LoadMaterials { squad, .. } => *squad == id,
            _ => false,
        })
        .map(|c| c.kind.clone())
        .collect()
}

fn type_buys(w: &World, p: u8, card: &str) -> u32 {
    let g = w.game().unwrap();
    let t = g.setup().squad_type_index(card).unwrap() as usize;
    g.ai[p as usize]
        .as_ref()
        .unwrap()
        .counters
        .bought
        .get(t)
        .copied()
        .unwrap_or(0)
}

/// Both AIs buying only engineer cards, rich, no cooldowns: the buys of
/// each faction's engineer cards after `secs`.
fn engineer_buys(cards: u32, secs: u64) -> [[u32; 3]; 2] {
    let mut w = flat_world_with(1, |s| {
        s.players.iter_mut().for_each(|p| p.is_ai = true);
        let eng = s.tag("engineer").unwrap();
        s.rules.ai.mix.retain(|m| m.tag == eng);
        s.rules.ai.fort.cards = cards;
        s.rules.ai.fort.works = 0;
        s.rules.ai.fort.wire = 0;
        s.rules.start_manpower = 100_000;
        s.rules.pop_cap = 1000;
        // One buy a plan: a plan never buys one card twice.
        s.rules.ai.infantry.buys_per_plan = 1;
        for t in s.squad_types.iter_mut() {
            t.recall_cooldown_ticks = 0;
            t.initial_cooldown_ticks = 0;
        }
    });
    play(&mut w, secs);
    [
        [
            type_buys(&w, 0, "ww2:ger_engineer_squad"),
            type_buys(&w, 0, "ww2:ger_baupioniergruppe"),
            type_buys(&w, 0, "ww2:ger_sturmpioniergruppe"),
        ],
        [
            type_buys(&w, 1, "ww2:usa_engineer_squad"),
            0,
            type_buys(&w, 1, "ww2:usa_assault_section"),
        ],
    ]
}

#[test]
fn ai_buys_engineer_cards_by_weight() {
    let [ger, usa] = engineer_buys(1, 240);
    // Weights 40 / 100 / 60 (Germany: Pioniere, Baupioniere, Sturmpioniere;
    // stage D) and 100 / 35 (USA): the buys come in that ratio, each within
    // one buy of its share.
    let n: u32 = ger.iter().sum();
    assert!(n >= 10, "enough German buys to read a ratio: {ger:?}");
    for (k, w) in [40u32, 100, 60].iter().enumerate() {
        let share = n * w / 200;
        assert!(
            ger[k].abs_diff(share) <= n / 20 + 1,
            "Germany {ger:?}, card {k}"
        );
    }
    let n: u32 = usa.iter().sum();
    assert!(n >= 8, "enough US buys: {usa:?}");
    assert!(usa[0].abs_diff(n * 100 / 135) <= n / 20 + 1, "USA {usa:?}");
    assert!(ger[1] > ger[2] && ger[2] > ger[0], "{ger:?}");
    assert!(usa[2] >= 1, "the Assault Section is bought: {usa:?}");
}

#[test]
fn ai_cards_switch_off_buys_none() {
    let [ger, usa] = engineer_buys(0, 240);
    assert_eq!((ger[1], ger[2], usa[2]), (0, 0, 0), "{ger:?} {usa:?}");
    // Positive control: the engineer slot still buys its old card.
    assert!(ger[0] >= 4 && usa[0] >= 4, "{ger:?} {usa:?}");
}

#[test]
fn ai_builders_load_materials_in_base() {
    let (mut w, pos, _) = held(|_| {});
    let base = w.game().unwrap().base_of(PlayerId(1));
    let eng = squad_at(&mut w, "ww2:usa_engineer_squad", base, 0);
    let rifle = squad_at(&mut w, "ww2:usa_rifle_squad", base + c(6, 0), 0);
    // Control: a builder outside the base loads nothing.
    let away = squad_at(&mut w, "ww2:usa_engineer_squad", pos + c(0, -40), 0);
    play(&mut w, 8);
    let carried = |w: &World, s| {
        w.game()
            .unwrap()
            .fort()
            .carried_g
            .get(&s)
            .copied()
            .unwrap_or(0)
    };
    let men = members(&w, eng).len() as u32;
    assert_eq!(carried(&w, eng), men * 20_000, "full carry: 20 kg a man");
    assert_eq!(carried(&w, rifle), 0, "no build capability, no load");
    assert_eq!(carried(&w, away), 0, "not in base, no load");
    assert_eq!(acc(&w, 1).materials_loaded_g, (men * 20_000) as u64);
}

#[test]
fn ai_materials_budget_stops_loading() {
    // 1 % of 600 MP = 6 MP = 60 kg at 100 MP/t.
    let (mut w, _, _) = held(|s| s.rules.ai.fort.materials_max_pct = 1);
    let base = w.game().unwrap().base_of(PlayerId(1));
    let a = squad_at(&mut w, "ww2:usa_engineer_squad", base, 0);
    let b = squad_at(&mut w, "ww2:usa_engineer_squad", base + c(8, 0), 0);
    play(&mut w, 20);
    let fs = w.game().unwrap().fort();
    let got = |s| fs.carried_g.get(&s).copied().unwrap_or(0);
    assert_eq!(got(a) + got(b), 60_000, "loads stop at the budget");
    assert!(acc(&w, 1).materials_micro <= 6 * crate::supply::MICRO);
    // Control: at 3 % both load in full.
    let (mut w, _, _) = held(|_| {});
    let a = squad_at(&mut w, "ww2:usa_engineer_squad", base, 0);
    let b = squad_at(&mut w, "ww2:usa_engineer_squad", base + c(8, 0), 0);
    play(&mut w, 20);
    let fs = w.game().unwrap().fort();
    let got = |s| fs.carried_g.get(&s).copied().unwrap_or(0);
    assert!(got(a) > 0 && got(b) > 0 && got(a) + got(b) > 60_000);
}

/// The trench middle and the unit direction to the enemy (point 0).
fn frame(pos: FxVec2, line: [FxVec2; 2]) -> (FxVec2, FxVec2) {
    let mid = (line[0] + line[1]).div_int(2);
    (mid, (mid - pos).normalize_or_zero())
}

/// Builds blueprint `id` for player 1 at a point site facing the enemy.
fn prebuild(w: &mut World, id: &str, at: FxVec2, enemy: FxVec2) {
    let f = fort(w, id);
    let line = [at, at + enemy * Fx::from_int(10)];
    assert!(w.build_now(f, &line, PlayerId(1)));
}

fn jobs_of(w: &World, f: u16) -> usize {
    ai_ro(w).forts.values().filter(|j| j.fort == f).count()
}

fn near_m(a: FxVec2, b: FxVec2, m: i32) -> bool {
    (a - b).length() <= Fx::from_int(m)
}

const ENG: &str = "ww2:usa_engineer_squad";

#[test]
fn ai_builds_mg_nest_after_trench() {
    let (mut w, pos, line) = held(|_| {});
    squad_at(&mut w, ENG, pos, 120);
    let nest = fort(&w, "ww2:mg_nest");
    let site = nest_site(&w, line);
    let ok = play_until(&mut w, 400, |w| {
        built(w, nest).iter().any(|p| near_m(*p, site, 6))
    });
    assert!(ok, "the nest stands at its site: {:?}", built(&w, nest));
    assert!(ai_ro(&w).counters.fort_jobs >= 1);
    // Control: the trench not finished: the engineers dig it, no nest job.
    let (mut w, pos, _) = held_dug(false, |_| {});
    let eng2 = squad_at(&mut w, ENG, pos, 120);
    play(&mut w, 30);
    assert_eq!(jobs_of(&w, nest), 0);
    assert!(matches!(
        w.game().unwrap().squads[&eng2].order,
        Order::Dig { .. }
    ));
}

#[test]
fn ai_lays_wire_across_the_approach() {
    let (mut w, pos, line) = held(|_| {});
    let (mid, enemy) = frame(pos, line);
    let wire = fort(&w, "ww2:barbed_wire");
    let site = nest_site(&w, line);
    prebuild(&mut w, "ww2:mg_nest", site, enemy);
    squad_at(&mut w, ENG, pos, 120);
    // Stage D: a 60 m line (30 sections; was 30 m).
    assert!(play_until(&mut w, 600, |w| built(w, wire).len() >= 30));
    let along = (line[1] - line[0]).normalize_or_zero();
    let fs = w.game().unwrap().fort();
    for s in fs.wire.values().filter(|s| s.owner == PlayerId(1)) {
        let m = (s.a + s.b).div_int(2) - mid;
        let out = m.x * enemy.x + m.y * enemy.y;
        let side = m.x * along.x + m.y * along.y;
        assert!(
            (out - Fx::from_int(35)).abs() <= Fx::from_int(2),
            "35 m out: {out:?}"
        );
        assert!(
            side.abs() <= Fx::from_int(31),
            "within the 60 m line: {side:?}"
        );
        let d = s.b - s.a;
        let across = (d.x * enemy.x + d.y * enemy.y).abs();
        assert!(across <= Fx::from_milli(200), "parallel to the trench");
    }
}

/// Stage D: [`held`] with a second capture point (not held by player 1)
/// 200 m from the first, `cs`, `sn` = (cos, sin) x 1000 of its bearing off
/// the enemy direction; player 1 lays its wire (nest prebuilt). Returns the
/// world, the point, the other point and the trench line.
fn held_two(cs: i32, sn: i32) -> (World, FxVec2, FxVec2, [FxVec2; 2]) {
    let mut other = FxVec2::ZERO;
    let (mut w, pos, line) = held(|s| {
        let p0: FxVec2 = s.map.points[0].pos.into();
        let b0: FxVec2 = s.map.bases[s.players[0].base as usize].center.into();
        let d = (b0 - p0).normalize_or_zero();
        let dir = d * Fx::from_milli(cs) + d.perp() * Fx::from_milli(sn);
        other = p0 + dir * Fx::from_int(200);
        let mut q = s.map.points[0].clone();
        q.pos = frontline_content::FixedVec2 {
            x: frontline_content::Fixed(other.x.raw()),
            y: frontline_content::Fixed(other.y.raw()),
        };
        s.map.points.push(q);
    });
    let (_, enemy) = frame(pos, line);
    let site = nest_site(&w, line);
    prebuild(&mut w, "ww2:mg_nest", site, enemy);
    squad_at(&mut w, ENG, pos, 120);
    (w, pos, other, line)
}

/// Player 1's wire sections whose band reaches within 2 m of the straight
/// route from `pos` to `to`, and all its sections.
fn wire_on_route(w: &World, pos: FxVec2, to: FxVec2) -> (usize, usize) {
    let fs = w.game().unwrap().fort();
    let own: Vec<_> = fs
        .wire
        .values()
        .filter(|s| s.owner == PlayerId(1))
        .collect();
    let on = own
        .iter()
        .filter(|s| {
            let m = (s.a + s.b).div_int(2);
            crate::fort::near_segment(m, pos, to, Fx::from_int(3))
        })
        .count();
    (on, own.len())
}

#[test]
fn ai_wire_keeps_off_its_route_out() {
    // The next point lies 25 degrees off the wire's facing: its straight
    // route would cross the 60 m line 16 m from the middle, so the line
    // slides along itself and the route stays clear; still 60 m.
    let (mut w, pos, other, _) = held_two(906, 423);
    let wire = fort(&w, "ww2:barbed_wire");
    assert!(play_until(&mut w, 600, |w| built(w, wire).len() >= 30));
    let (on, all) = wire_on_route(&w, pos, other);
    assert_eq!(on, 0, "no section on the route out ({all} sections)");
    // Control: the next point straight ahead (the approach the wire
    // faces): the line stays across it.
    let (mut w, pos, other, _) = held_two(1000, 0);
    assert!(play_until(&mut w, 600, |w| built(w, wire).len() >= 30));
    let (on, _) = wire_on_route(&w, pos, other);
    assert!(on >= 1, "the faced approach keeps its wire");
}

#[test]
fn ai_does_not_build_under_threat() {
    let nest_job = |enemies: bool| {
        let (mut w, pos, line) = held(|_| {});
        let (mid, enemy) = frame(pos, line);
        squad_at(&mut w, ENG, pos, 120);
        if enemies {
            let at = mid + enemy * Fx::from_int(50);
            spawn(
                &mut w,
                0,
                "ww2:ger_grenadiergruppe",
                at.x.floor_int(),
                at.y.floor_int(),
            );
        }
        let nest = fort(&w, "ww2:mg_nest");
        let mut most = 0;
        for _ in 0..40 {
            play(&mut w, 1);
            most = most.max(jobs_of(&w, nest));
        }
        (most, acc(&w, 1).works_started)
    };
    assert_eq!(nest_job(true), (0, 0), "enemies at the point: no work");
    // Positive control: the same scene without them builds.
    assert!(nest_job(false).0 >= 1);
}

#[test]
fn ai_builder_limits_hold() {
    let most_jobs = |edit: fn(&mut frontline_content::MatchSetup)| {
        let (mut w, pos, _) = held(edit);
        for k in 0..3 {
            squad_at(&mut w, ENG, pos + c(4 * k, 0), 120);
        }
        let mut most = 0;
        for _ in 0..60 {
            play(&mut w, 1);
            most = most.max(ai_ro(&w).forts.len());
        }
        most
    };
    assert_eq!(most_jobs(|s| s.rules.ai.fort.builders_max = 1), 1);
    assert_eq!(most_jobs(|_| {}), 2, "builders_max 2");
    // works_per_point 1 with the nest standing: no wire job.
    let wire_jobs = |per_point: u32| {
        let (mut w, pos, line) = held(|s| s.rules.ai.fort.works_per_point = per_point);
        let (_, enemy) = frame(pos, line);
        let site = nest_site(&w, line);
        prebuild(&mut w, "ww2:mg_nest", site, enemy);
        squad_at(&mut w, ENG, pos, 120);
        play(&mut w, 30);
        jobs_of(&w, fort(&w, "ww2:barbed_wire")) + built(&w, fort(&w, "ww2:barbed_wire")).len()
    };
    assert_eq!(wire_jobs(1), 0);
    assert!(wire_jobs(3) > 0, "control: room for the wire");
}

#[test]
fn ai_builder_is_not_reordered_mid_work() {
    let (mut w, pos, line) = held(|s| s.rules.ai.fort.wire = 0);
    let eng = squad_at(&mut w, ENG, pos, 120);
    let nest = fort(&w, "ww2:mg_nest");
    let site = nest_site(&w, line);
    let mut watched = 0;
    for _ in 0..(400 * 30) {
        w.step(&[]);
        let Some(j) = ai_ro(&w).forts.get(&eng) else {
            if watched > 0 {
                break;
            }
            continue;
        };
        if matches!(j.phase, FortPhase::Order | FortPhase::Watch) {
            watched += 1;
            let p = pending_for(&w, eng);
            assert!(
                !p.iter().any(|k| matches!(
                    k,
                    CommandKind::Move { .. }
                        | CommandKind::AttackMove { .. }
                        | CommandKind::Dig { .. }
                )),
                "re-ordered mid work: {p:?}"
            );
        }
    }
    assert!(watched > 0);
    assert!(
        built(&w, nest).iter().any(|p| near_m(*p, site, 6)),
        "the work was finished"
    );
    // Positive control: without its job the squad is re-ordered at once.
    let (mut w, pos, _) = held(|s| s.rules.ai.fort.wire = 0);
    let eng = squad_at(&mut w, ENG, pos, 120);
    assert!(play_until(&mut w, 60, |w| ai_ro(w)
        .forts
        .get(&eng)
        .is_some_and(|j| j.phase == FortPhase::Watch)));
    ai1(&mut w).forts.clear();
    let mut moved = false;
    for _ in 0..(10 * 30) {
        w.step(&[]);
        ai1(&mut w).forts.clear();
        moved |= pending_for(&w, eng)
            .iter()
            .any(|k| matches!(k, CommandKind::Move { .. }));
    }
    assert!(moved, "the check sees a re-order");
}

#[test]
fn ai_never_orders_without_the_capability() {
    let (mut w, pos, line) = held(|_| {});
    let (_, enemy) = frame(pos, line);
    let rifle = squad_at(&mut w, "ww2:usa_rifle_squad", pos, 120);
    let lmg = squad_at(&mut w, "ww2:usa_lmg_squad", pos + c(5, 0), 0);
    let base = w.game().unwrap().base_of(PlayerId(1));
    let rifle_b = squad_at(&mut w, "ww2:usa_rifle_squad", base, 0);
    for _ in 0..(60 * 30) {
        w.step(&[]);
        for s in [rifle, lmg, rifle_b] {
            let p = pending_for(&w, s);
            assert!(
                !p.iter().any(|k| matches!(
                    k,
                    CommandKind::Fortify { .. } | CommandKind::LoadMaterials { .. }
                )),
                "{s:?}: {p:?}"
            );
        }
    }
    assert!(ai_ro(&w).forts.is_empty() && acc(&w, 1).works_started == 0);
    // `build_structure` without `place_wire`: the nest standing, no wire.
    let wire_jobs = |strip: bool| {
        let (mut w, pos, line) = held(|s| {
            if strip {
                for t in s.squad_types.iter_mut().filter(|t| t.id == ENG) {
                    t.caps &= !Capability::PlaceWire.bit();
                }
            }
        });
        let site = nest_site(&w, line);
        prebuild(&mut w, "ww2:mg_nest", site, enemy);
        squad_at(&mut w, ENG, pos, 120);
        play(&mut w, 30);
        acc(&w, 1).works_started
    };
    assert_eq!(wire_jobs(true), 0);
    assert!(
        wire_jobs(false) > 0,
        "control: with place_wire it lays wire"
    );
}

#[test]
fn ai_mg_team_mans_its_nest() {
    let target = |with_nest: bool| {
        let (mut w, pos, line) = held(|s| {
            s.rules.ai.fort.works = 1;
            s.rules.ai.fort.wire = 0;
        });
        let (_, enemy) = frame(pos, line);
        let site = nest_site(&w, line);
        if with_nest {
            prebuild(&mut w, "ww2:mg_nest", site, enemy);
        }
        let lmg = squad_at(&mut w, "ww2:usa_lmg_squad", pos, 0);
        play(&mut w, 15);
        let t = ai_ro(&w).tasks[&lmg];
        (t.kind, near_m(t.target, site, 1))
    };
    assert_eq!(target(true), (crate::ai::TaskKind::Defend, true));
    assert_eq!(
        target(false),
        (crate::ai::TaskKind::Defend, false),
        "control"
    );
}

/// The dugout site of point 0.
fn dugout_site(w: &World, pos: FxVec2, line: [FxVec2; 2]) -> FxVec2 {
    let (mid, enemy) = frame(pos, line);
    let back = w.match_setup().unwrap().rules.ai.fort.dugout_back_m as i32;
    mid - enemy * Fx::from_int(back)
}

#[test]
fn ai_marks_a_shelled_point_and_digs_a_dugout() {
    let run = |pin: bool| {
        // (Stage D: wire off, the 60 m line would hold the one builder for
        // the whole run; the dugout comes after the wire.)
        let (mut w, pos, line) = held(|s| s.rules.ai.fort.wire = 0);
        let (_, enemy) = frame(pos, line);
        let site = nest_site(&w, line);
        prebuild(&mut w, "ww2:mg_nest", site, enemy);
        let rifle = squad_at(&mut w, "ww2:usa_rifle_squad", pos, 0);
        squad_at(&mut w, ENG, pos + c(3, 3), 120);
        let dug = fort(&w, "ww2:dugout");
        let mut marked = false;
        let mut dugout_job = false;
        for _ in 0..90 {
            play(&mut w, 1);
            if pin {
                for m in members(&w, rifle) {
                    w.edit_soldier(m, |_, c| c.suppression = 99_000);
                }
            }
            marked |= ai_ro(&w).shelled.contains_key(&0);
            dugout_job |= jobs_of(&w, dug) > 0;
        }
        (marked, dugout_job, ai_ro(&w).enemy_mortar_seen)
    };
    assert_eq!(
        run(true),
        (true, true, false),
        "pinned with nobody near: shelled"
    );
    assert_eq!(run(false), (false, false, false), "control: quiet");
}

/// Player 1's men of squad `id` under the roof box of the dugout piece
/// at `site` (inside the box in plan, below its bottom).
fn under_roof(w: &World, id: SquadId, site: FxVec2) -> usize {
    let f = fort(w, "ww2:dugout");
    let half = w.match_setup().unwrap().forts[f as usize].pieces[0].half;
    let roof = w
        .game()
        .unwrap()
        .fort()
        .pieces
        .values()
        .find(|q| q.fort == f && near_m(q.pos, site, 2))
        .cloned()
        .expect("a roof");
    let (sn, cs) = roof.yaw.sin_cos();
    let along = FxVec2::new(cs, sn);
    members(w, id)
        .iter()
        .filter(|m| {
            let p = w.state.entities.transform[m.index()].pos;
            let d = p.xy() - roof.pos;
            d.dot(along).abs() <= Fx::from(half[0])
                && d.dot(along.perp()).abs() <= Fx::from(half[1])
                && p.z + Fx::from_milli(1500) <= roof.z
        })
        .count()
}

#[test]
fn ai_shelters_in_the_dugout_and_returns() {
    let (mut w, pos, line) = held(|s| s.rules.ai.fort.works_per_point = 0);
    let (_, enemy) = frame(pos, line);
    let site = dugout_site(&w, pos, line);
    // The pit as the work digs it (along the facing), then the roof.
    let half = Fx::from(w.match_setup().unwrap().rules.dugout_pit.pit_line).div_int(2);
    assert!(w.dig_pit_now(
        &[site - enemy * half, site + enemy * half],
        frontline_content::DigProfile::DugoutPit
    ));
    prebuild(&mut w, "ww2:dugout", site, enemy);
    let rifle = squad_at(&mut w, "ww2:usa_rifle_squad", pos, 0);
    play(&mut w, 10);
    assert_eq!(under_roof(&w, rifle, site), 0, "not before a mark");
    let tick = w.state.tick;
    ai1(&mut w).shelled.insert(0, tick);
    // The men the roof holds (trench spots `cover.gap_mm` apart) go in
    // one by one and stay under it while the mark is fresh.
    assert!(play_until(&mut w, 15, |w| under_roof(w, rifle, site) >= 2));
    assert!(ai_ro(&w).counters.shelter_moves >= 1);
    play(&mut w, 4);
    assert!(under_roof(&w, rifle, site) >= 2, "they stay under the roof");
    // The mark gets old (shelter_s 20): back out with the squad order
    // (the cover rule may still pick the pit for a man whose formation
    // slot lies near it: the order, not the spot, is checked here).
    assert!(play_until(&mut w, 40, |w| {
        let t = ai_ro(w).tasks[&rifle].target;
        let to_site = match w.game().unwrap().squads[&rifle].order {
            Order::Move { target } => near_m(target, site, 1),
            _ => false,
        };
        let detached = members(w, rifle)
            .iter()
            .any(|m| w.state.entities.unit[m.index()].detached);
        !near_m(t, site, 1) && !to_site && !detached
    }));
}

#[test]
fn ai_roofs_its_hq_once_mortars_are_seen() {
    let (mut w, pos, _) = held(|s| s.rules.ai.fort.works_per_point = 0);
    let hq_at = pos + c(-20, 0);
    let def = {
        let s = w.match_setup().unwrap();
        s.stock_of(
            s.players[1].faction,
            frontline_content::setup::StockKind::Hq,
            None,
        )
        .unwrap()
    };
    w.edit_supply6(|s| {
        s.structures.insert(
            77,
            crate::structures::Structure {
                owner: PlayerId(1),
                def,
                pos: hq_at,
                yaw: Fx::ZERO,
                builder: None,
                built: true,
                box_hp: [1000; crate::structures::MAX_BOXES],
                radio: true,
                stock_g: [0; 3],
            },
        );
        s.next_structure = s.next_structure.max(78);
    });
    squad_at(&mut w, ENG, hq_at + c(10, 0), 120);
    let roof = fort(&w, "ww2:hq_dugout");
    play(&mut w, 30);
    assert_eq!(jobs_of(&w, roof), 0, "no mortar seen: no roof");
    ai1(&mut w).enemy_mortar_seen = true;
    assert!(play_until(&mut w, 30, |w| {
        w.game()
            .unwrap()
            .fort()
            .works
            .values()
            .any(|k| k.hq == Some(77))
    }));
}

#[test]
fn ai_sandbags_an_emplaced_gun() {
    let run = |hold_s: u32| {
        let (mut w, pos, line) = held(|s| {
            s.rules.ai.fort.works_per_point = 0;
            s.rules.ai.fort.gun_hold_s = hold_s;
        });
        let (_, enemy) = frame(pos, line);
        let at = pos + c(20, 0);
        let gsq = squad_at(&mut w, "ww2:usa_57mm_m1_squad", at, 0);
        let gun = members(&w, gsq)[0];
        let gpos = w.state.entities.transform[gun.index()].pos.xy();
        let tick = w.state.tick;
        ai1(&mut w).guns.insert(
            gsq,
            crate::ai::AiGun {
                phase: crate::ai::GunPhase::Hold,
                site: gpos,
                facing: gpos + enemy * Fx::from_int(100),
                moved_to: gpos,
                moved_at: tick,
                quiet_at: tick,
                // (No S9 pit: that would take the engineers.)
                pit: true,
                ..Default::default()
            },
        );
        squad_at(&mut w, ENG, gpos + c(-15, 0), 120);
        let wall = fort(&w, "ww2:sandbag_wall");
        let ok = play_until(&mut w, 30, |w| {
            jobs_of(w, wall) > 0 || !built(w, wall).is_empty()
        });
        let (gp, fc) = {
            let j = &ai_ro(&w).guns[&gsq];
            (j.site, j.facing)
        };
        let want = gp + (fc - gp).normalize_or_zero() * Fx::from_int(4);
        let at_gun = ai_ro(&w)
            .forts
            .values()
            .any(|j| j.fort == wall && near_m((j.line[0] + j.line[1]).div_int(2), want, 1));
        (ok, at_gun)
    };
    assert_eq!(run(0), (true, true), "a wall 4 m in front of the gun");
    assert!(!run(3600).0, "control: not in Hold long enough");
}

/// Stage D: switch `k` of the per-work four (0 nest, 1 dugout, 2 roof,
/// 3 sandbags) set to 0; `k` >= 4: all on.
fn work_switch_off(s: &mut frontline_content::MatchSetup, k: usize) {
    let f = &mut s.rules.ai.fort;
    match k {
        0 => f.nest = 0,
        1 => f.dugout = 0,
        2 => f.roof = 0,
        3 => f.sandbags = 0,
        _ => {}
    }
}

/// Whether the AI starts (a job) or builds work kind `kind` (0 nest, 1
/// dugout, 2 roof, 3 sandbags) within 40 s in that kind's scene, with
/// switch `off` (as [`work_switch_off`]) set to 0. Wire is off throughout.
fn work_started(kind: usize, off: usize) -> bool {
    let (mut w, pos, line) = held(|s| {
        s.rules.ai.fort.wire = 0;
        if kind >= 2 {
            // Roof and wall scenes: no point works take the builder.
            s.rules.ai.fort.works_per_point = 0;
            // A gun counts as held at once.
            s.rules.ai.fort.gun_hold_s = 0;
        }
        work_switch_off(s, off);
    });
    let (_, enemy) = frame(pos, line);
    match kind {
        0 => {
            squad_at(&mut w, ENG, pos, 120);
            let f = fort(&w, "ww2:mg_nest");
            play_until(&mut w, 40, |w| jobs_of(w, f) > 0 || !built(w, f).is_empty())
        }
        1 => {
            let site = nest_site(&w, line);
            prebuild(&mut w, "ww2:mg_nest", site, enemy);
            squad_at(&mut w, ENG, pos, 120);
            ai1(&mut w).enemy_mortar_seen = true;
            let f = fort(&w, "ww2:dugout");
            play_until(&mut w, 40, |w| jobs_of(w, f) > 0 || !built(w, f).is_empty())
        }
        2 => {
            let hq_at = pos + c(-20, 0);
            let def = {
                let s = w.match_setup().unwrap();
                s.stock_of(
                    s.players[1].faction,
                    frontline_content::setup::StockKind::Hq,
                    None,
                )
                .unwrap()
            };
            w.edit_supply6(|s| {
                s.structures.insert(
                    77,
                    crate::structures::Structure {
                        owner: PlayerId(1),
                        def,
                        pos: hq_at,
                        yaw: Fx::ZERO,
                        builder: None,
                        built: true,
                        box_hp: [1000; crate::structures::MAX_BOXES],
                        radio: true,
                        stock_g: [0; 3],
                    },
                );
                s.next_structure = s.next_structure.max(78);
            });
            squad_at(&mut w, ENG, hq_at + c(10, 0), 120);
            ai1(&mut w).enemy_mortar_seen = true;
            let f = fort(&w, "ww2:hq_dugout");
            play_until(&mut w, 40, |w| {
                jobs_of(w, f) > 0
                    || w.game()
                        .unwrap()
                        .fort()
                        .works
                        .values()
                        .any(|k| k.hq == Some(77))
            })
        }
        _ => {
            let at = pos + c(20, 0);
            let gsq = squad_at(&mut w, "ww2:usa_57mm_m1_squad", at, 0);
            let gun = members(&w, gsq)[0];
            let gpos = w.state.entities.transform[gun.index()].pos.xy();
            let tick = w.state.tick;
            ai1(&mut w).guns.insert(
                gsq,
                crate::ai::AiGun {
                    phase: crate::ai::GunPhase::Hold,
                    site: gpos,
                    facing: gpos + enemy * Fx::from_int(100),
                    moved_to: gpos,
                    moved_at: tick,
                    quiet_at: tick,
                    pit: true,
                    ..Default::default()
                },
            );
            squad_at(&mut w, ENG, gpos + c(-15, 0), 120);
            let f = fort(&w, "ww2:sandbag_wall");
            play_until(&mut w, 40, |w| jobs_of(w, f) > 0 || !built(w, f).is_empty())
        }
    }
}

#[test]
fn ai_per_work_switches_stop_exactly_their_kind() {
    // All on (positive control): every kind is started in its scene.
    for kind in 0..4 {
        assert!(work_started(kind, 4), "all on: kind {kind} started");
    }
    // `nest` off alone: the wire does not wait for a nest (it does with
    // `works` 0, the measured works_off: no point wire either).
    let wire_laid = |edit: fn(&mut frontline_content::MatchSetup)| {
        let (mut w, pos, _) = held(edit);
        squad_at(&mut w, ENG, pos, 120);
        let f = fort(&w, "ww2:barbed_wire");
        play_until(&mut w, 60, |w| jobs_of(w, f) > 0 || !built(w, f).is_empty())
    };
    assert!(wire_laid(|s| s.rules.ai.fort.nest = 0), "nest off: wire");
    assert!(
        !wire_laid(|s| s.rules.ai.fort.works = 0),
        "works off: no wire"
    );
    // Each switch off: its own kind is not started, the other three are.
    for off in 0..4 {
        for kind in 0..4 {
            assert_eq!(
                work_started(kind, off),
                kind != off,
                "switch {off} off, kind {kind}"
            );
        }
    }
}

#[test]
fn ai_truck_brings_material_to_a_waiting_work() {
    let (mut w, pos, line) = held(|_| {});
    let (_, enemy) = frame(pos, line);
    let site = nest_site(&w, line);
    prebuild(&mut w, "ww2:mg_nest", site, enemy);
    let base = w.game().unwrap().base_of(PlayerId(1));
    squad_at(&mut w, "ww2:usa_cckw_squad", base, 0);
    squad_at(&mut w, ENG, pos, 0);
    let ok = play_until(&mut w, 400, |w| acc(w, 1).materials_used_truck_g > 0);
    let c = &ai_ro(&w).counters;
    assert!(ok, "truck kg drawn: {:?} {:?}", c.material_runs, acc(&w, 1));
    assert!(c.material_runs[0] >= 1);
    assert_eq!(c.material_runs[2], 0, "nobody walked back");
}

#[test]
fn ai_builder_walks_back_for_materials() {
    let (mut w, pos, line) = held(|_| {});
    let (_, enemy) = frame(pos, line);
    let site = nest_site(&w, line);
    prebuild(&mut w, "ww2:mg_nest", site, enemy);
    let eng = squad_at(&mut w, ENG, pos, 0);
    let wire = fort(&w, "ww2:barbed_wire");
    assert!(play_until(&mut w, 30, |w| ai_ro(w).counters.material_runs
        [2]
        >= 1));
    let carried = |w: &World| {
        w.game()
            .unwrap()
            .fort()
            .carried_g
            .get(&eng)
            .copied()
            .unwrap_or(0)
    };
    assert!(
        play_until(&mut w, 400, |w| carried(w) > 0),
        "loaded in base"
    );
    assert!(
        play_until(&mut w, 600, |w| built(w, wire).len() >= 15),
        "and laid the wire"
    );
}

// ================================================== Build C: breaching --

const ASSAULT: &str = "ww2:usa_assault_section";
const WIRE_OUT: i32 = 35;

/// A world (`threads`) where AI player 1 (USA, no start squads, no buys)
/// attacks the map's only point, held by player 0, across `wire_len` m of
/// player 0's wire [`WIRE_OUT`] m in front of it (0: no wire). Returns
/// the world, the point and the unit direction from it toward player 1.
fn attacked_t(
    threads: usize,
    wire_len: i32,
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> (World, FxVec2, FxVec2) {
    let mut w = flat_world_with(threads, |s| {
        s.players[1].is_ai = true;
        s.map.points.truncate(1);
        s.factions.iter_mut().for_each(|f| f.start_squads.clear());
        s.squad_types.iter_mut().for_each(|t| t.ai_weight = 0);
        edit(s);
    });
    let (pos, dir) = {
        let g = w.state.game.as_mut().unwrap();
        g.points[0].owner = Some(PlayerId(0));
        let pos: FxVec2 = g.setup.map.points[0].pos.into();
        (pos, (g.base_of(PlayerId(1)) - pos).normalize_or_zero())
    };
    if wire_len > 0 {
        let f = fort(&w, "ww2:barbed_wire");
        let mid = pos + dir * Fx::from_int(WIRE_OUT);
        let half = dir.perp() * Fx::from_int(wire_len).div_int(2);
        assert!(w.build_now(f, &[mid - half, mid + half], PlayerId(0)));
    }
    (w, pos, dir)
}

fn attacked(
    wire_len: i32,
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> (World, FxVec2, FxVec2) {
    attacked_t(1, wire_len, edit)
}

/// A squad of player `p` at `at`.
fn squad_of(w: &mut World, p: u8, card: &str, at: FxVec2) -> SquadId {
    spawn(w, p, card, at.x.floor_int(), at.y.floor_int())
}

/// Player 0's defenders: a rifle squad on the point and an MG team 15 m
/// to its side; `armed` false: no rounds (they cannot hurt the attackers).
fn defenders(w: &mut World, pos: FxVec2, dir: FxVec2, armed: bool) -> (SquadId, SquadId) {
    let rifle = squad_of(w, 0, "ww2:ger_grenadiergruppe", pos);
    let mg = squad_of(
        w,
        0,
        "ww2:ger_smg_gruppe",
        pos + dir.perp() * Fx::from_int(15),
    );
    if !armed {
        for m in members(w, rifle).into_iter().chain(members(w, mg)) {
            w.edit_soldier(m, |k, c| {
                k.rounds.iter_mut().for_each(|r| *r = 0);
                k.items.iter_mut().for_each(|n| *n = 0);
                c.mag = 0;
            });
        }
    }
    (rifle, mg)
}

/// Player 1's attackers `out` m from the point: the card, an MG team and
/// a rifle squad 20 m to either side.
fn attackers(w: &mut World, pos: FxVec2, dir: FxVec2, out: i32, card: &str) -> [SquadId; 3] {
    let at = pos + dir * Fx::from_int(out);
    let side = dir.perp() * Fx::from_int(20);
    [
        squad_of(w, 1, card, at),
        squad_of(w, 1, "ww2:usa_lmg_squad", at - side),
        squad_of(w, 1, "ww2:usa_rifle_squad", at + side),
    ]
}

/// The pending AI commands of player 1 (sampled).
fn pending1(w: &World) -> Vec<CommandKind> {
    ai_ro(w).pending.iter().map(|c| c.kind.clone()).collect()
}

#[test]
fn ai_learns_enemy_wire_only_by_sight() {
    // In range in the open: known; its own wire never.
    let (mut w, pos, dir) = attacked(30, |_| {});
    let own = fort(&w, "ww2:barbed_wire");
    let mid = pos + dir * Fx::from_int(80);
    assert!(w.build_now(
        own,
        &[mid, mid + dir.perp() * Fx::from_int(10)],
        PlayerId(1)
    ));
    squad_of(
        &mut w,
        1,
        "ww2:usa_rifle_squad",
        pos + dir * Fx::from_int(130),
    );
    play(&mut w, 5);
    let enemy: Vec<u32> = {
        let f = w.game().unwrap().fort();
        f.wire
            .iter()
            .filter(|(_, s)| s.owner == PlayerId(0))
            .map(|(&id, _)| id)
            .collect()
    };
    assert!(!enemy.is_empty());
    assert_eq!(
        ai_ro(&w).wire_known,
        enemy,
        "every enemy section, sorted, no own one"
    );
    // A section that no longer stands is forgotten.
    let gone = enemy[0];
    w.edit_fort(|f| {
        f.wire.remove(&gone);
    });
    play(&mut w, 3);
    assert!(!ai_ro(&w).wire_known.contains(&gone));

    // Out of `wire_seen_m`: unknown.
    let (mut w, pos, dir) = attacked(30, |_| {});
    squad_of(
        &mut w,
        1,
        "ww2:usa_rifle_squad",
        pos + dir * Fx::from_int(200),
    );
    play(&mut w, 5);
    assert!(ai_ro(&w).wire_known.is_empty(), "165 m: not seen");

    // In range but at the bottom of a hollow 4 m deep: the earth hides it
    // (read at the first plans, before the squad walks out).
    let (mut w, pos, dir) = attacked(30, |_| {});
    let hole = pos + dir * Fx::from_int(130);
    let tick = w.state.tick;
    w.state
        .terrain
        .crater_cut(hole, Fx::from_int(30), Fx::from_int(4), tick);
    w.refresh_hashes();
    let sq = squad_of(&mut w, 1, "ww2:usa_rifle_squad", hole);
    play(&mut w, 2);
    assert!(ai_ro(&w).next_plan > 30, "a plan ran");
    assert!(
        ai_ro(&w).wire_known.is_empty(),
        "no line of sight: not known"
    );
    // Control: the same men lifted out of the hollow see it.
    let up = hole - dir * Fx::from_int(40);
    w.place_squad(sq, up, -dir);
    play(&mut w, 2);
    assert!(!ai_ro(&w).wire_known.is_empty(), "in the open: known");

    // Switch off: nothing learnt.
    let (mut w, pos, dir) = attacked(30, |s| s.rules.ai.fort.breach = 0);
    squad_of(
        &mut w,
        1,
        "ww2:usa_rifle_squad",
        pos + dir * Fx::from_int(130),
    );
    play(&mut w, 5);
    assert!(ai_ro(&w).wire_known.is_empty());
}

#[test]
fn ai_goes_round_wire_when_the_path_avoids_it() {
    // 10 m of wire across the approach: the path goes round, no job.
    let (mut w, pos, dir) = attacked(10, |_| {});
    attackers(&mut w, pos, dir, 130, ASSAULT);
    play(&mut w, 20);
    let c = &ai_ro(&w).counters;
    assert!(c.wire_bypassed >= 1, "went round");
    assert_eq!(c.breaches_started, 0);
    assert!(ai_ro(&w).breaches.is_empty());
    // Control: 100 m across the whole approach: a breach job.
    let (mut w, pos, dir) = attacked(100, |_| {});
    attackers(&mut w, pos, dir, 130, ASSAULT);
    play(&mut w, 20);
    let c = &ai_ro(&w).counters;
    assert_eq!(c.wire_bypassed, 0);
    assert_eq!(c.breaches_started, 1);
    assert_eq!(ai_ro(&w).breaches.len(), 1);
}

/// Ticks at which player 1 first had each kind of breach command pending
/// (suppression aimed within 6 m of `aim`, smoke, the carrier's order),
/// sampled every tick for `secs`.
#[derive(Debug, Default)]
struct BreachTicks {
    suppress: Option<u64>,
    suppress_elsewhere: Option<u64>,
    smoke: Option<u64>,
    bangalore: Option<u64>,
    cut: Option<u64>,
}

fn watch_breach(w: &mut World, secs: u64, aim: &dyn Fn(&World) -> Vec<FxVec2>) -> BreachTicks {
    let mut t = BreachTicks::default();
    for _ in 0..secs * 30 {
        steps(w, 1);
        let tick = w.state.tick;
        let aims = aim(w);
        for k in pending1(w) {
            match k {
                CommandKind::SuppressUnits { line, .. } => {
                    let mid = (line[0] + line[line.len() - 1]).div_int(2);
                    if aims.iter().any(|&a| near_m(mid, a, 6)) {
                        t.suppress.get_or_insert(tick);
                    } else {
                        t.suppress_elsewhere.get_or_insert(tick);
                    }
                }
                CommandKind::Throw {
                    item: frontline_content::ItemKind::Smoke,
                    ..
                } => {
                    t.smoke.get_or_insert(tick);
                }
                CommandKind::PlaceBangalore { .. } => {
                    t.bangalore.get_or_insert(tick);
                }
                CommandKind::CutWire { .. } => {
                    t.cut.get_or_insert(tick);
                }
                _ => {}
            }
        }
    }
    t
}

/// Positions of squad `id`'s live men.
fn men_at(w: &World, id: SquadId) -> Vec<FxVec2> {
    w.game()
        .unwrap()
        .squads
        .get(&id)
        .map(|q| {
            q.members
                .iter()
                .map(|m| w.state.entities.transform[m.index()].pos.xy())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn ai_breach_suppresses_the_nest_and_smokes_before_the_carrier_goes() {
    let (mut w, pos, dir) = attacked(100, |_| {});
    let (_, mg) = defenders(&mut w, pos, dir, false);
    // The MG team in a built nest.
    let nest = fort(&w, "ww2:mg_nest");
    let at = pos + dir.perp() * Fx::from_int(15);
    assert!(w.build_now(nest, &[at, at + dir], PlayerId(0)));
    attackers(&mut w, pos, dir, 130, ASSAULT);
    let t = watch_breach(&mut w, 150, &|w: &World| men_at(w, mg));
    let c = &ai_ro(&w).counters;
    assert_eq!(c.breaches_started, 1, "{t:?}");
    let sup = t.suppress.expect("the base aims at the MG team");
    let smoke = t.smoke.expect("smoke thrown");
    let go = t.bangalore.or(t.cut).expect("the carrier goes");
    assert!(sup <= smoke, "fire before smoke: {t:?}");
    let lead = w.match_setup().unwrap().rules.ai.fort.suppress_lead_s as u64 * 30;
    assert!(
        go >= smoke + lead,
        "the carrier waits the lead after the smoke: {t:?}"
    );
    assert!(c.breach_suppress >= 1 && c.breach_smokes >= 1);
    // Control: the same attack with the switch off never aims at the MG,
    // smokes for a breach or sends a carrier.
    let (mut w, pos, dir) = attacked(100, |s| s.rules.ai.fort.breach = 0);
    let (_, mg) = defenders(&mut w, pos, dir, false);
    assert!(w.build_now(nest, &[at, at + dir], PlayerId(0)));
    attackers(&mut w, pos, dir, 130, ASSAULT);
    let t = watch_breach(&mut w, 150, &|w: &World| men_at(w, mg));
    assert_eq!((t.bangalore, t.cut), (None, None), "{t:?}");
    assert_eq!(ai_ro(&w).counters.breach_smokes, 0);
}

/// Stage D (owner "fix smoke"): at the tick the carrier is sent (the first
/// `PlaceBangalore` or `CutWire` pending), the distance from the crossing
/// to the nearest smoke cloud's centre and that cloud's full radius (m,
/// floored); `None` when no carrier went or no cloud stood.
fn cloud_at_go(w: &mut World, secs: u64) -> Option<(i32, i32)> {
    for _ in 0..secs * 30 {
        steps(w, 1);
        let go = pending1(w).iter().any(|k| {
            matches!(
                k,
                CommandKind::PlaceBangalore { .. } | CommandKind::CutWire { .. }
            )
        });
        if !go {
            continue;
        }
        let crossing = ai_ro(w).breaches.values().next()?.crossing;
        let g = w.game().unwrap();
        return g
            .smoke
            .iter()
            .map(|c| {
                let r = g.setup.items[c.item as usize]
                    .smoke
                    .as_ref()
                    .map_or(0, |s| Fx::from(s.radius).floor_int());
                ((c.pos.xy() - crossing).length().floor_int(), r)
            })
            .min();
    }
    None
}

#[test]
fn ai_breach_smoke_covers_the_crossing_when_the_carrier_goes() {
    // Defenders who cannot hurt (the carrier lives to go), an MG team in a
    // nest 15 m to the point's side: the cloud stands within its radius of
    // the crossing when the carrier is sent.
    let scene = |edit: fn(&mut frontline_content::MatchSetup)| {
        let (mut w, pos, dir) = attacked(100, edit);
        let (_, _mg) = defenders(&mut w, pos, dir, false);
        let nest = fort(&w, "ww2:mg_nest");
        let at = pos + dir.perp() * Fx::from_int(15);
        assert!(w.build_now(nest, &[at, at + dir], PlayerId(0)));
        attackers(&mut w, pos, dir, 130, ASSAULT);
        w
    };
    let mut w = scene(|_| {});
    let (d, r) = cloud_at_go(&mut w, 150).expect("a carrier went with a cloud standing");
    assert!(
        r > 0 && d <= r,
        "cloud centre {d} m from the crossing, radius {r} m"
    );
    assert!(ai_ro(&w).counters.breach_smokes >= 1);
    // Control: the same attack with no smoke on any attacker: the carrier
    // goes with no cloud at the crossing (the check can fail).
    let mut w = scene(|s| {
        let smoke = frontline_content::ItemKind::Smoke.index();
        for t in s.squad_types.iter_mut() {
            t.members.iter_mut().for_each(|m| m.items[smoke] = 0);
        }
    });
    let near = cloud_at_go(&mut w, 150);
    assert!(
        near.is_none_or(|(d, r)| d > r),
        "no smoke: no cloud at the crossing, got {near:?}"
    );
}

#[test]
fn ai_breach_blows_with_a_bangalore_else_cuts() {
    // Defenders known near the crossing: the assault section lays a
    // Bangalore, which blows the gap (the job ends opened).
    let (mut w, pos, dir) = attacked(100, |_| {});
    defenders(&mut w, pos, dir, false);
    attackers(&mut w, pos, dir, 130, ASSAULT);
    let t = watch_breach(&mut w, 150, &|_: &World| Vec::new());
    assert!(t.bangalore.is_some() && t.cut.is_none(), "{t:?}");
    assert!(play_until(&mut w, 60, |w| ai_ro(w)
        .counters
        .breaches_opened
        >= 1));
    let a = acc(&w, 1);
    assert!(a.bangalores >= 1 && a.wire_blown >= 1, "{a:?}");
    // Nobody near the crossing and the squad unhurt: it cuts (alone, so
    // no other squad walks onto the empty point first).
    let (mut w, pos, dir) = attacked(100, |_| {});
    squad_of(&mut w, 1, ASSAULT, pos + dir * Fx::from_int(130));
    let t = watch_breach(&mut w, 150, &|_: &World| Vec::new());
    assert!(t.cut.is_some() && t.bangalore.is_none(), "{t:?}");
    assert!(play_until(&mut w, 90, |w| acc(w, 1).wire_cut >= 1));
    // Field engineers (no Bangalore) cut even with defenders near.
    let (mut w, pos, dir) = attacked(100, |_| {});
    defenders(&mut w, pos, dir, false);
    attackers(&mut w, pos, dir, 130, "ww2:usa_engineer_squad");
    let t = watch_breach(&mut w, 150, &|_: &World| Vec::new());
    assert!(t.cut.is_some() && t.bangalore.is_none(), "{t:?}");
}

#[test]
fn ai_breach_retries_a_pinned_carrier_then_gives_up() {
    let (mut w, pos, dir) = attacked(100, |s| s.rules.ai.fort.breach_tries = 3);
    defenders(&mut w, pos, dir, false);
    let [card, ..] = attackers(&mut w, pos, dir, 130, ASSAULT);
    // Every carrier is stopped as he sets off (as a pinned one is called
    // off): each try fails, three in all, then the job is given up. (Stage
    // D: with the closer stage spot the third try here has no defender
    // known and no base firing, so its carrier goes to cut; it counts.)
    let mut sent = 0;
    let mut was = false;
    for _ in 0..30 * 400 {
        steps(&mut w, 1);
        let now = pending1(&w).iter().any(|k| {
            matches!(
                k,
                CommandKind::PlaceBangalore { .. } | CommandKind::CutWire { .. }
            )
        });
        sent += (now && !was) as u32;
        was = now;
        for m in members(&w, card) {
            let i = m.index();
            if w.state.entities.kit[i]
                .windup
                .is_some_and(|x| x.kind == frontline_content::ItemKind::Bangalore)
            {
                // Called off with his charge still on him.
                let b = frontline_content::ItemKind::Bangalore.index();
                w.edit_soldier(m, |k, _| {
                    k.windup = None;
                    k.items[b] += 1;
                });
            }
        }
        if ai_ro(&w).counters.breaches_given_up >= 1 {
            break;
        }
    }
    let c = &ai_ro(&w).counters;
    assert_eq!(c.breaches_given_up, 1);
    assert_eq!(c.breaches_opened, 0);
    assert_eq!(sent, 3, "one carrier per try");
    assert!(c.breach_smokes >= 1);
    assert_eq!(acc(&w, 1).bangalores, 0);
    // Control: the same scene, carriers left alone: the charge is laid.
    let (mut w, pos, dir) = attacked(100, |s| s.rules.ai.fort.breach_tries = 3);
    defenders(&mut w, pos, dir, false);
    attackers(&mut w, pos, dir, 130, ASSAULT);
    assert!(play_until(&mut w, 300, |w| acc(w, 1).bangalores >= 1));
}

/// Plays `secs`; returns (whether a player 1 leader got within 10 m of
/// the point, leader-ticks inside a standing wire band, and how far along
/// the wire line from its middle each leader crossed it, m).
fn leaders_through(w: &mut World, secs: u64, pos: FxVec2, dir: FxVec2) -> (bool, u32, Vec<i32>) {
    let (mut reached, mut inside, mut crossed) = (false, 0, Vec::new());
    let mut side: std::collections::BTreeMap<SquadId, bool> = Default::default();
    for _ in 0..secs * 30 {
        steps(w, 1);
        let g = w.game().unwrap();
        for (&id, q) in g.squads.iter().filter(|(_, q)| q.owner == PlayerId(1)) {
            let p = w.state.entities.transform[q.lead(&w.state.entities).index()]
                .pos
                .xy();
            reached |= near_m(p, pos, 10);
            inside += g.fort().in_wire(&g.setup().forts, p) as u32;
            let out = (p - pos).dot(dir) > Fx::from_int(WIRE_OUT);
            if side.insert(id, out) == Some(true) && !out {
                crossed.push((p - pos).dot(dir.perp()).abs().floor_int());
            }
        }
    }
    (reached, inside, crossed)
}

#[test]
fn ai_assault_goes_through_the_gap() {
    // Control first: no breach habit, the attack walks through the wire.
    let (mut w, pos, dir) = attacked(100, |s| s.rules.ai.fort.breach = 0);
    defenders(&mut w, pos, dir, false);
    attackers(&mut w, pos, dir, 130, ASSAULT);
    let (_, wired, _) = leaders_through(&mut w, 300, pos, dir);
    assert!(wired > 100, "no gap: leaders in the band ({wired} ticks)");
    let (mut w, pos, dir) = attacked(100, |_| {});
    defenders(&mut w, pos, dir, false);
    attackers(&mut w, pos, dir, 130, ASSAULT);
    assert!(play_until(&mut w, 200, |w| ai_ro(w)
        .counters
        .breaches_opened
        >= 1));
    assert!(ai_ro(&w).breaches.is_empty(), "the job ended opened");
    let crossing = w.game().unwrap().fort().charges.len();
    assert_eq!(crossing, 0);
    let (reached, inside, crossed) = leaders_through(&mut w, 120, pos, dir);
    assert!(reached, "the attack reached the point");
    // The gap is about 4 m: a leader may brush its edge (the path's
    // corner clips the next section's band, about 1.5 m at wire pace),
    // never walk the band as without it.
    assert!(
        inside * 2 < wired,
        "through the gap: {inside} ticks in a band, {wired} without"
    );
    assert!(!crossed.is_empty());
    assert!(
        // (the squads start 20 m to either side; the blown gap spans a
        // few sections round the crossing)
        crossed.iter().all(|&d| d <= 12),
        "every leader crossed at the gap (m from the middle): {crossed:?}"
    );
}

/// [`attacked`] where the only card the AI may buy is the assault section
/// (weight 35), the engineer mix slot is empty and decisions are exact (no
/// buy by weight or by a random pick).
fn breach_buy_world(
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> (World, FxVec2, FxVec2) {
    attacked(100, |s| {
        s.rules.start_manpower = 5_000;
        s.rules.difficulty.normal.decision_quality_pct = 100;
        let eng = s.tag("engineer").unwrap();
        for m in s.rules.ai.mix.iter_mut().filter(|m| m.tag == eng) {
            m.pct = 0;
        }
        for t in s.squad_types.iter_mut().filter(|t| t.id == ASSAULT) {
            t.ai_weight = 35;
        }
        edit(s);
    })
}

fn assault_buys(w: &World) -> u32 {
    type_buys(w, 1, ASSAULT)
}

#[test]
fn ai_buys_an_assault_card_when_a_breach_is_wanted() {
    // Rifle squads attack across known wire: the next buy is the assault
    // section, once (`breach_cards_max` 1).
    let (mut w, pos, dir) = breach_buy_world(|_| {});
    squad_of(
        &mut w,
        1,
        "ww2:usa_rifle_squad",
        pos + dir * Fx::from_int(130),
    );
    assert!(play_until(&mut w, 30, |w| assault_buys(w) >= 1));
    play(&mut w, 120);
    assert_eq!(assault_buys(&w), 1, "at most one alive");
    let ty = w.match_setup().unwrap().squad_type_index(ASSAULT).unwrap();
    assert!(
        w.game()
            .unwrap()
            .squads
            .values()
            .any(|q| q.owner == PlayerId(1) && q.type_idx == ty)
    );
    // No wire: no buy.
    let (mut w, pos, dir) = {
        let (mut w, pos, dir) = breach_buy_world(|_| {});
        w.edit_fort(|f| f.wire.clear());
        (w, pos, dir)
    };
    squad_of(
        &mut w,
        1,
        "ww2:usa_rifle_squad",
        pos + dir * Fx::from_int(130),
    );
    play(&mut w, 40);
    assert_eq!(assault_buys(&w), 0, "no wire");
    // A squad that can cut is alive: no buy.
    let (mut w, pos, dir) = breach_buy_world(|_| {});
    squad_of(
        &mut w,
        1,
        "ww2:usa_rifle_squad",
        pos + dir * Fx::from_int(130),
    );
    squad_of(
        &mut w,
        1,
        "ww2:usa_engineer_squad",
        pos + dir * Fx::from_int(150),
    );
    play(&mut w, 40);
    assert_eq!(assault_buys(&w), 0, "engineers can cut");
    // Switches off: no buy.
    for off in [0, 1] {
        let (mut w, pos, dir) = breach_buy_world(|s| {
            if off == 0 {
                s.rules.ai.fort.breach = 0;
            } else {
                s.rules.ai.fort.cards = 0;
            }
        });
        squad_of(
            &mut w,
            1,
            "ww2:usa_rifle_squad",
            pos + dir * Fx::from_int(130),
        );
        play(&mut w, 40);
        assert_eq!(assault_buys(&w), 0, "switch {off} off");
    }
}

/// The armed breach scene at `threads`.
fn breach_scene(threads: usize) -> World {
    let (mut w, pos, dir) = attacked_t(threads, 100, |_| {});
    defenders(&mut w, pos, dir, true);
    attackers(&mut w, pos, dir, 130, ASSAULT);
    w
}

fn hashes(w: &mut World, ticks: u64) -> Vec<u64> {
    let mut h = Vec::new();
    for _ in 0..ticks {
        w.step(&[]);
        if w.state.tick.is_multiple_of(15) {
            h.push(w.state_hash());
        }
    }
    h
}

#[test]
fn ai_breach_is_deterministic_across_threads_and_save_load() {
    let ticks = 30 * 150;
    let mut base = breach_scene(1);
    let want = hashes(&mut base, ticks);
    let c = &ai_ro(&base).counters;
    assert!(
        c.breaches_started >= 1 && c.breach_suppress >= 1,
        "a breach ran"
    );
    for threads in [2, 3, 8] {
        let mut w = breach_scene(threads);
        assert_eq!(hashes(&mut w, ticks), want, "at {threads} threads");
    }
    // Save while a job runs, load, continue alike.
    let mut a = breach_scene(1);
    assert!(play_until(&mut a, 120, |w| !ai_ro(w).breaches.is_empty()));
    play(&mut a, 3);
    let mut b = World::load(&a.save(), 1).unwrap();
    assert_eq!(a.state_hash(), b.state_hash(), "at the load");
    assert_eq!(ai_ro(&a), ai_ro(&b));
    assert!(!ai_ro(&b).wire_known.is_empty());
    assert_eq!(
        hashes(&mut a, 30 * 60),
        hashes(&mut b, 30 * 60),
        "after the load"
    );
    assert_eq!(ai_ro(&a), ai_ro(&b));
}
