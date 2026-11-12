//! S10b-ai tests (docs/slices/S10b-ai.md R11, R12; Build B chunk 1): paths
//! round known enemy mines, the derived mine layer, the blast ring and the
//! sim counters.

use super::s6_tests::{flat_world_with, members, run, spawn, steps};
use super::*;
use crate::fort::{FortState, MAX_BLASTS, MineSet};
use crate::game::{NavPhase, SquadId};

const HZ: u64 = 30;

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

fn fs(w: &World) -> &FortState {
    w.game().unwrap().fort()
}

/// A flat world with `mine_paths` set.
fn world(mine_paths: u32) -> World {
    flat_world_with(1, |s| {
        s.rules.fortify.as_mut().unwrap().mine_paths = mine_paths;
    })
}

/// `owner`'s field of blueprint `field` along y = `y`, x = `x0..=x1`
/// every `step` m; `known_to` players also know each mine.
fn field(
    w: &mut World,
    field: &str,
    owner: u8,
    (x0, x1, y): (i32, i32, i32),
    step: usize,
    known_to: u8,
) {
    let f = fort(w, field);
    let mut ids = Vec::new();
    for x in (x0..=x1).step_by(step) {
        ids.push(w.lay_mine_now(f, c(x, y), PlayerId(owner)).unwrap());
    }
    w.edit_fort(|s| {
        for id in ids {
            s.mines.get_mut(&id).unwrap().known |= known_to;
        }
    });
}

/// The bands (cm) of the match: (foot, vehicle).
fn bands(w: &World) -> (u32, u32) {
    let f = w.match_setup().unwrap().rules.fortify.as_ref().unwrap();
    (f.mine_band_foot_cm, f.mine_band_vehicle_cm)
}

/// The full-scan set of player `p` (every mine checked, no layer).
fn scan(w: &World, p: u8, vehicle: bool) -> MineSet {
    let b = bands(w);
    fs(w).scan_mine_set(
        &w.match_setup().unwrap().items,
        w.state.terrain.dim(),
        PlayerId(p),
        vehicle,
        if vehicle { b.1 } else { b.0 },
    )
}

/// Orders squad `sq` of `p` to `to` and returns its lead's position and
/// the served path (the lead's legs).
fn route(w: &mut World, p: u8, sq: SquadId, to: FxVec2) -> Vec<FxVec2> {
    run(
        w,
        p,
        3,
        CommandKind::Move {
            squad: sq,
            target: to,
        },
    );
    for _ in 0..HZ {
        let g = w.game().unwrap();
        let s = &g.squads[&sq];
        if s.nav.phase != NavPhase::PathPending && !s.nav.path.is_empty() {
            let lead = w.state.entities.transform[s.lead(&w.state.entities).index()]
                .pos
                .xy();
            let mut out = vec![lead];
            out.extend(s.nav.path[s.nav.next as usize..].iter().copied());
            return out;
        }
        w.step(&[]);
    }
    panic!("no path served");
}

/// Legs of `pts` that come within the band of a mine of `set`.
fn mined_legs(set: &MineSet, pts: &[FxVec2]) -> usize {
    pts.windows(2).filter(|l| set.crosses(l[0], l[1])).count()
}

/// The greatest distance (m) of a route point from the line x = `x`.
fn off_line(pts: &[FxVec2], x: i32) -> i32 {
    pts.iter()
        .map(|p| (p.x - Fx::from_int(x)).abs().floor_int())
        .max()
        .unwrap_or(0)
}

fn length(pts: &[FxVec2]) -> i32 {
    pts.windows(2)
        .map(|l| (l[1] - l[0]).length().floor_int())
        .sum()
}

/// Prints the nodes one foot search (470 -> 530 across the field)
/// expands without and with the mine layer (the log's numbers).
fn expansions(w: &World, set: MineSet) {
    let g = w.game().unwrap();
    let mv = &g.setup().rules.movement;
    let cost = crate::path::PathCost {
        slope_cost_pct: mv.slope_cost_pct,
        max_slope_pct: mv.max_slope_pct,
        trench_cost_pct: mv.trench_cost_pct,
        ditch_cost_pct: mv.ditch_cost_pct,
    };
    let pct = g.setup().rules.fortify.as_ref().unwrap().mine_path_cost_pct;
    let cross = |a: FxVec2, b: FxVec2| set.crosses(a, b);
    let mines = crate::path::MineCost {
        nodes: &set.nodes,
        cost_pct: pct,
        crosses: &cross,
    };
    let mut s = crate::path::PathScratch::default();
    let t = &w.state.terrain;
    for m in [None, Some(mines)] {
        let r = crate::path::find_foot_path(
            t,
            c(500, 470),
            c(500, 530),
            &cost,
            None,
            None,
            m,
            mv.path_max_expansions,
            &mut s,
        );
        eprintln!(
            "  search mines {}: {} expanded, {} m",
            m.is_some(),
            r.expansions,
            length(&[vec![c(500, 470)], r.points].concat())
        );
    }
}

/// R11: a rifle squad ordered across a known enemy AP field (30 m) and a
/// tank across a known enemy AT field (40 m) go round it. Control:
/// `mine_paths` 0 -> both routes go straight through (asserted below).
#[test]
fn paths_avoid_known_enemy_mines() {
    for on in [1u32, 0] {
        // Foot.
        let mut w = world(on);
        field(&mut w, "ww2:ap_minefield", 1, (485, 515, 500), 2, 1);
        let sq = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 500, 470);
        let pts = route(&mut w, 0, sq, c(500, 530));
        let set = scan(&w, 0, false);
        assert_eq!(set.pos.len(), 16);
        let hit = mined_legs(&set, &pts);
        eprintln!(
            "foot mine_paths {on}: {} m, {} m off the line, mined legs {hit}",
            length(&pts),
            off_line(&pts, 500)
        );
        expansions(&w, set);
        if on == 1 {
            assert_eq!(hit, 0, "foot route {pts:?}");
            assert!(off_line(&pts, 500) > 15, "went round: {pts:?}");
        } else {
            assert!(hit > 0, "control: straight through {pts:?}");
        }
        // Tank.
        let mut w = world(on);
        field(&mut w, "ww2:at_minefield", 1, (480, 520, 500), 4, 1);
        let tank = spawn(&mut w, 0, "ww2:ger_pz4_h_squad", 500, 440);
        let pts = route(&mut w, 0, tank, c(500, 560));
        let set = scan(&w, 0, true);
        assert_eq!(set.pos.len(), 11);
        let hit = mined_legs(&set, &pts);
        eprintln!(
            "tank mine_paths {on}: {} m, {} m off the line, mined legs {hit}",
            length(&pts),
            off_line(&pts, 500)
        );
        if on == 1 {
            assert_eq!(hit, 0, "tank route {pts:?}");
        } else {
            assert!(hit > 0, "control: straight through {pts:?}");
        }
    }
}

/// R11: an enemy field the player does not know and the player's own field
/// cost nothing: the route stays straight. Control: price every mine
/// (drop the `owner != p` and `known` filters of `scan_mine_set`) -> both
/// detour (seen failing).
#[test]
fn paths_ignore_unknown_and_own_mines() {
    for (owner, known_to) in [(1u8, 0u8), (0, 1)] {
        let mut w = world(1);
        field(
            &mut w,
            "ww2:ap_minefield",
            owner,
            (485, 515, 500),
            2,
            known_to,
        );
        let sq = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 500, 470);
        let pts = route(&mut w, 0, sq, c(500, 530));
        assert!(off_line(&pts, 500) <= 2, "owner {owner}: detoured {pts:?}");
        let mut w = world(1);
        field(
            &mut w,
            "ww2:at_minefield",
            owner,
            (480, 520, 500),
            4,
            known_to,
        );
        let tank = spawn(&mut w, 0, "ww2:ger_pz4_h_squad", 500, 440);
        let pts = route(&mut w, 0, tank, c(500, 560));
        assert!(
            off_line(&pts, 500) <= 2,
            "owner {owner}: tank detoured {pts:?}"
        );
    }
}

/// A tiny deterministic generator.
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

/// The derived layer equals the full scan after adds, new known bits,
/// removals and a load; every grid step the exact test prices has an end
/// among the layer's nodes (the prefilter loses nothing). Control: skip
/// the rebuild in `serve_paths` -> the loaded layer stays stale ("the
/// layer was not rebuilt"; seen failing).
#[test]
fn mine_layer_matches_full_scan_after_load() {
    let mut w = world(1);
    let dim = w.state.terrain.dim();
    let b = bands(&w);
    let mut rng = Lcg(11);
    let ap = fort(&w, "ww2:ap_minefield");
    let at = fort(&w, "ww2:at_minefield");
    let add = |w: &mut World, rng: &mut Lcg, n: u32| {
        for k in 0..n {
            let p = FxVec2::new(
                Fx::from_raw((470 << 16) + rng.next(60 << 16) as i32),
                Fx::from_raw((470 << 16) + rng.next(60 << 16) as i32),
            );
            let f = if k % 3 == 0 { at } else { ap };
            let id = w.lay_mine_now(f, p, PlayerId(rng.next(2) as u8)).unwrap();
            let bits = rng.next(4) as u8;
            w.edit_fort(|s| s.mines.get_mut(&id).unwrap().known |= bits);
        }
    };
    let refresh = |w: &mut World| {
        let g = w.state.game.as_mut().unwrap();
        g.fort.refresh_mine_layer(&g.setup.items, dim, b);
    };
    let check = |w: &World, rng: &mut Lcg| -> u32 {
        let f = fs(w);
        assert!(f.mine_layer_fresh(dim, b), "the layer was not rebuilt");
        let mut hits = 0;
        for p in 0..2u8 {
            for veh in [false, true] {
                let full = scan(w, p, veh);
                let got = f.mine_set(PlayerId(p), veh);
                if full.pos.is_empty() {
                    assert!(got.is_none());
                    continue;
                }
                let got = got.expect("a set for known mines");
                assert_eq!(got, &full, "player {p} vehicle {veh}");
                for _ in 0..2000 {
                    let (x, y) = (115 + rng.next(20), 115 + rng.next(20));
                    let (dx, dy) = (rng.next(3) as i32 - 1, rng.next(3) as i32 - 1);
                    let (mx, my) = ((x as i32 + dx) as u32, (y as i32 + dy) as u32);
                    let pa = c(x as i32 * 4, y as i32 * 4);
                    let pb = c(mx as i32 * 4, my as i32 * 4);
                    if got.crosses(pa, pb) {
                        hits += 1;
                        let (n, m) = (y * dim + x, my * dim + mx);
                        assert!(
                            got.nodes.binary_search(&n).is_ok()
                                || got.nodes.binary_search(&m).is_ok(),
                            "a priced step with no end in the layer: {pa:?} -> {pb:?}"
                        );
                    }
                }
            }
        }
        hits
    };
    add(&mut w, &mut rng, 30);
    refresh(&mut w);
    assert!(check(&w, &mut rng) > 50, "a vacuous test");
    // New known bits only.
    let ids: Vec<u32> = fs(&w).mines.keys().copied().step_by(4).collect();
    w.edit_fort(|s| {
        ids.iter()
            .for_each(|id| s.mines.get_mut(id).unwrap().known |= 3)
    });
    assert!(
        !fs(&w).mine_layer_fresh(dim, b),
        "a known bit makes it stale"
    );
    refresh(&mut w);
    check(&w, &mut rng);
    // Removals and adds.
    w.edit_fort(|s| {
        let ids: Vec<u32> = s.mines.keys().copied().step_by(3).collect();
        ids.iter().for_each(|id| {
            s.mines.remove(id);
        });
    });
    assert!(!fs(&w).mine_layer_fresh(dim, b), "a removal makes it stale");
    add(&mut w, &mut rng, 10);
    refresh(&mut w);
    check(&w, &mut rng);
    // After load: not saved, stale until a path request rebuilds it.
    let mut back = World::load(&w.save(), 1).unwrap();
    assert!(!fs(&back).mine_layer_fresh(dim, b));
    let sq = spawn(&mut back, 0, "ww2:ger_grenadiergruppe", 300, 300);
    run(
        &mut back,
        0,
        3,
        CommandKind::Move {
            squad: sq,
            target: c(320, 300),
        },
    );
    check(&back, &mut rng);
    assert_eq!(back.full_state_hash(), {
        let sq2 = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 300, 300);
        assert_eq!(sq2, sq);
        run(
            &mut w,
            0,
            3,
            CommandKind::Move {
                squad: sq,
                target: c(320, 300),
            },
        );
        w.full_state_hash()
    });
}

/// Blasts: every mine that goes off is remembered, at most `MAX_BLASTS`
/// (the newest), each for `BLAST_KEEP_S`; a man who sets off a mine his
/// player knew counts in `lost_on_known`. Control: cap the ring at
/// `MAX_BLASTS - 1` (the `>=` test in `take_mines` made `>= MAX_BLASTS - 1`)
/// -> 31 kept (seen failing).
#[test]
fn blast_ring_keeps_32_newest() {
    let mut w = world(1);
    let f = fort(&w, "ww2:ap_minefield");
    let ids: Vec<u32> = (0..40)
        .map(|k| w.lay_mine_now(f, c(300 + 2 * k, 300), PlayerId(1)).unwrap())
        .collect();
    let now = w.state.tick;
    w.edit_fort(|s| {
        for id in &ids {
            s.mines.get_mut(id).unwrap().boom_tick = Some(now);
        }
    });
    w.step(&[]);
    let bl = &fs(&w).blasts;
    assert_eq!(bl.len(), MAX_BLASTS);
    assert_eq!(bl[0].pos, c(300 + 2 * 8, 300), "the oldest 8 dropped");
    assert_eq!(bl[MAX_BLASTS - 1].pos, c(300 + 2 * 39, 300));
    assert!(bl.iter().all(|b| b.owner == PlayerId(1) && b.tick == now));
    // Known: a player-0 squad on a player-1 mine its player knows; then
    // one it does not know.
    for (known, lost) in [(1u8, 1u32), (0, 0)] {
        let mut w = world(1);
        let sq = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 500, 500);
        let man = members(&w, sq)[1];
        let at = w.state.entities.transform[man.index()].pos.xy();
        let id = w.lay_mine_now(f, at, PlayerId(1)).unwrap();
        w.edit_fort(|s| s.mines.get_mut(&id).unwrap().known |= known);
        steps(&mut w, 3);
        assert_eq!(fs(&w).accounts[1].mines_set_off, 1);
        assert_eq!(fs(&w).blasts.len(), 1);
        assert_eq!(fs(&w).accounts[0].lost_on_known, lost, "known {known}");
        assert_eq!(
            fs(&w).accounts[0].own_mine_losses + fs(&w).accounts[1].own_mine_losses,
            0
        );
        // Expiry: a blast older than `BLAST_KEEP_S` goes on the next tick.
        let keep = crate::fort::BLAST_KEEP_S * HZ;
        let b0 = fs(&w).blasts[0].tick;
        w.state.tick = b0 + keep - 3;
        w.step(&[]);
        assert_eq!(fs(&w).blasts.len(), 1, "younger than the keep");
        steps(&mut w, 4);
        assert!(fs(&w).blasts.is_empty(), "dropped at the keep");
    }
}

/// `at_exposure_ticks`: an enemy tank standing 4 m from an AT mine (beside
/// it, tracks clear) adds one per tick to the mine owner's count; an own
/// tank adds nothing. Control: the own tank (owner 1) -> 0, asserted.
#[test]
fn at_exposure_counts_enemy_vehicle_ticks() {
    for (p, want) in [(0u8, 20u64), (1, 0)] {
        let mut w = world(1);
        let card = if p == 0 {
            "ww2:ger_pz4_h_squad"
        } else {
            "ww2:usa_m4a3_75_squad"
        };
        let tank = spawn(&mut w, p, card, 500, 500);
        let v = members(&w, tank)[0];
        steps(&mut w, 2);
        let at = w.state.entities.transform[v.index()].pos.xy() + FxVec2::from_int(0, 4);
        let f = fort(&w, "ww2:at_minefield");
        w.lay_mine_now(f, at, PlayerId(1)).unwrap();
        steps(&mut w, 20);
        assert_eq!(fs(&w).mines.len(), 1, "not set off");
        assert_eq!(fs(&w).accounts[1].at_exposure_ticks, want, "player {p}");
    }
}
