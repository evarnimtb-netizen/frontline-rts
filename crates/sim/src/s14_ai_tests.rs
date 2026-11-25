//! S14-ai Build D tests (docs/slices/S14-assault-kit.md 7, A1-A4 and
//! liveness): the flame AI (`ai::flame`). Player 1 (USA) is an AI that buys
//! nothing from its mix; its Assault Section carries the M1A1 (`[flame]
//! enabled` 1 at resolve time). Player 0 (Germany) holds the one point with
//! unarmed men in a trench (or a bunker). Every `[ai.flame]` switch and
//! `[clear] enabled` are on in the test only. Each test names its control.

use super::s6_tests::{members, spawn};
use super::*;
use crate::ai::{AiFlame, AiState, FlamePhase};
use crate::unit::UnitOrder;
use frontline_content::{ItemKind, MatchSetup};

const ASSAULT: &str = "ww2:usa_assault_section";
const US_RIFLES: &str = "ww2:usa_rifle_squad";
const GER: &str = "ww2:ger_grenadiergruppe";
/// The attackers start this far out from the point (m).
const OUT_M: i32 = 60;

/// The S14-ai scene's world: one point, held by player 0; `edit` last.
fn flame_ai_world(threads: usize, edit: impl FnOnce(&mut MatchSetup)) -> World {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let mut defs = frontline_content::ModDefs::load(&root).unwrap();
    for r in defs.rules.values_mut() {
        r.flame.get_or_insert_with(Default::default).enabled = 1;
    }
    let req = frontline_content::MatchRequest::skirmish(
        1,
        "ww2:germany",
        "ww2:usa",
        frontline_content::Difficulty::Normal,
    );
    let mut s = defs.resolve(&req).unwrap();
    s.players.iter_mut().for_each(|p| p.is_ai = false);
    s.players[1].is_ai = true;
    s.map.terrain.octaves.clear();
    s.map.terrain.bands.clear();
    s.map.woods.clear();
    s.s11b_off();
    s.s12_off();
    s.s13_off();
    super::s10a_ai_tests::s10b_ai_off(&mut s);
    s.map.points.truncate(1);
    s.factions.iter_mut().for_each(|f| f.start_squads.clear());
    s.squad_types.iter_mut().for_each(|t| t.ai_weight = 0);
    s.rules.clear.enabled = 1;
    // Every gun but the flamethrowers hurts and frightens nobody: the
    // defenders fire (a man in a trench is seen when he rises to fire)
    // and the scene lasts until the flame decides it.
    for wp in s.weapons.iter_mut() {
        if wp.class != frontline_content::WeaponClass::Flame {
            wp.damage = 0;
            wp.suppression = 0;
        }
    }
    let f = &mut s.rules.ai.flame;
    (f.jobs, f.refuel, f.buy, f.clear) = (1, 1, 1, 1);
    edit(&mut s);
    let mut w = World::new_match(s, threads).unwrap();
    w.state.game.as_mut().unwrap().points[0].owner = Some(PlayerId(0));
    w
}

/// The point and the unit vector from it toward player 1's base.
fn axis(w: &World) -> (FxVec2, FxVec2) {
    let g = w.game().unwrap();
    let pos: FxVec2 = g.setup.map.points[0].pos.into();
    (pos, (g.base_of(PlayerId(1)) - pos).normalize_or_zero())
}

fn at(w: &mut World, p: u8, card: &str, q: FxVec2) -> SquadId {
    spawn(w, p, card, q.x.floor_int(), q.y.floor_int())
}

/// Player 0's men (harmless guns, no frags) on the point; `trench` digs a trench across it
/// first.
fn defenders(w: &mut World, trench: bool) -> SquadId {
    let (pos, dir) = axis(w);
    if trench {
        let half = dir.perp() * Fx::from_int(12);
        assert!(w.dig_trench_now(&[pos - half, pos + half], -1));
    }
    let sq = at(w, 0, GER, pos);
    let items = ItemKind::Frag.index();
    for m in members(w, sq) {
        w.edit_soldier(m, |k, _| k.items[items] = 0);
    }
    sq
}

fn unarm(w: &mut World, sq: SquadId) {
    for m in members(w, sq) {
        w.edit_soldier(m, |k, c| {
            k.rounds.iter_mut().for_each(|r| *r = 0);
            k.items.iter_mut().for_each(|n| *n = 0);
            c.mag = 0;
        });
    }
}

/// Player 1's Assault Section `OUT_M` out.
fn attacker(w: &mut World) -> SquadId {
    let (pos, dir) = axis(w);
    at(w, 1, ASSAULT, pos + dir * Fx::from_int(OUT_M))
}

fn ai_ro(w: &World) -> &AiState {
    w.game().unwrap().ai[1].as_ref().unwrap()
}

fn ai_mut(w: &mut World) -> &mut AiState {
    w.state.game.as_mut().unwrap().ai[1].as_mut().unwrap()
}

fn job(w: &World) -> Option<(SquadId, AiFlame)> {
    ai_ro(w)
        .flame
        .jobs
        .iter()
        .next()
        .map(|(&k, j)| (k, j.clone()))
}

fn ends(w: &World) -> [u32; 6] {
    ai_ro(w).counters.flame_job_ends
}

/// Steps up to `secs` s; whether `done` held at some tick.
fn play_until(w: &mut World, secs: u64, mut done: impl FnMut(&World) -> bool) -> bool {
    for _ in 0..secs * 30 {
        w.step(&[]);
        if done(w) {
            return true;
        }
    }
    false
}

fn trench_scene(threads: usize, edit: impl FnOnce(&mut MatchSetup)) -> (World, SquadId, SquadId) {
    let mut w = flame_ai_world(threads, edit);
    let d = defenders(&mut w, true);
    let a = attacker(&mut w);
    (w, a, d)
}

/// A1: the job starts on a trench target, closes, goes and burns: the
/// flame man's `SuppressUnits` point is a German man's trench cell.
/// Control: `jobs` 0 starts none.
#[test]
fn ai_flame_job_runs_its_phases() {
    let (mut w, a, d) = trench_scene(1, |_| {});
    assert!(
        play_until(&mut w, 150, |w| job(w)
            .is_some_and(|(_, j)| j.phase == FlamePhase::Burn)),
        "a job burns: {:?} {:?}",
        job(&w),
        ai_ro(&w).counters.flame_jobs
    );
    let (sq, j) = job(&w).unwrap();
    assert_eq!(sq, a);
    assert!(!j.bunker);
    let men = members(&w, d);
    let at = men
        .iter()
        .map(|m| w.state.entities.transform[m.index()].pos.xy())
        .map(|p| (p - j.aim).length())
        .min()
        .unwrap();
    assert!(at <= Fx::from_int(4), "the aim is at a defender: {at:?}");
    play_until(&mut w, 3, |_| false);
    let u = &w.state.entities.unit[j.man.index()];
    assert!(
        matches!(u.order, UnitOrder::Suppress { line } if line[0] == j.aim),
        "{:?}",
        u.order
    );
    let (mut c, _, _) = trench_scene(1, |s| s.rules.ai.flame.jobs = 0);
    play_until(&mut c, 150, |_| false);
    assert_eq!(ai_ro(&c).counters.flame_jobs, 0, "control: jobs off");
}

/// L (dead): the operator killed with nobody to pick his flamethrower up
/// ends the job (`dead`). Control: with the drop left, the assistant picks
/// it up and the job runs on with him.
#[test]
fn flame_job_ends_when_operator_dies() {
    for pickup in [true, false] {
        let (mut w, a, _) = trench_scene(1, |_| {});
        assert!(play_until(&mut w, 60, |w| job(w).is_some()));
        let (_, j) = job(&w).unwrap();
        assert!(w.kill_soldier(j.man));
        if !pickup {
            w.state
                .game
                .as_mut()
                .unwrap()
                .squads
                .get_mut(&a)
                .unwrap()
                .drops
                .clear();
        }
        let ended = play_until(&mut w, 20, |w| ends(w)[2] > 0);
        if pickup {
            assert!(!ended, "control: picked up, {:?}", ends(&w));
            let (_, k) = job(&w).expect("runs on");
            assert_ne!(k.man, j.man, "the new carrier");
        } else {
            assert!(ended, "dead: {:?} {:?}", ends(&w), job(&w));
            assert!(job(&w).is_none());
        }
    }
}

/// L1: `job_max_s` ends a job (`deadline`). Control: 180 s does not in
/// the same time.
#[test]
fn flame_job_ends_on_deadline() {
    for max in [3u32, 180] {
        let (mut w, _, _) = trench_scene(1, |s| s.rules.ai.flame.job_max_s = max);
        assert!(play_until(&mut w, 60, |w| job(w).is_some()));
        let hit = play_until(&mut w, 10, |w| ends(w)[4] > 0);
        assert_eq!(hit, max == 3, "max {max}: {:?}", ends(&w));
    }
}

/// L2: a squad held still makes no progress and the job ends (`stall`)
/// after `stall_s`; the failed target is not taken again for `recheck_s`
/// (L3). Control: walking freely, no stall in the same time.
#[test]
fn flame_job_ends_on_stall() {
    for held in [true, false] {
        let (mut w, a, _) = trench_scene(1, |s| s.rules.ai.flame.stall_s = 5);
        assert!(play_until(&mut w, 60, |w| job(w).is_some()));
        let men = members(&w, a);
        if held {
            super::s10b_tests::hold(&mut w, &men, 30 * 12);
            assert!(ends(&w)[5] > 0, "stall: {:?}", ends(&w));
            assert_eq!(ai_ro(&w).flame.failed.len(), 1, "the target waits");
            let started = ai_ro(&w).counters.flame_jobs;
            super::s10b_tests::hold(&mut w, &men, 30 * 10);
            assert_eq!(ai_ro(&w).counters.flame_jobs, started, "not retried (L3)");
        } else {
            play_until(&mut w, 8, |_| false);
            assert_eq!(ends(&w)[5], 0, "control: {:?}", ends(&w));
        }
    }
}

/// A2: a flame squad whose tank is dry walks back to refill while the S5a
/// rule would keep it (`dry_pct` and `return_below_pct` 0 here; in the
/// mod's rules a tank at the reserve under threat). Control: `refuel` 0,
/// it does not.
#[test]
fn refuel_when_tank_dry() {
    for refuel in [1u32, 0] {
        let mut w = flame_ai_world(1, |s| {
            s.rules.ai.flame.jobs = 0;
            s.rules.ai.flame.refuel = refuel;
            // The S5a trip alone never starts (the flamethrower is the
            // squad's crew weapon: its share is the tank's).
            (s.rules.ai.ammo.dry_pct, s.rules.ai.ammo.return_below_pct) = (0, 0);
        });
        let a = attacker(&mut w);
        let s = w.match_setup().unwrap();
        let flamer = members(&w, a)
            .into_iter()
            .find(|m| {
                let k = w.state.entities.kit[m.index()].weapon as usize;
                s.weapons[k].class == frontline_content::WeaponClass::Flame
            })
            .expect("a flame man");
        w.edit_soldier(flamer, |k, c| {
            k.rounds.iter_mut().for_each(|r| *r = 0);
            c.mag = 0;
        });
        play_until(&mut w, 30, |_| false);
        let trip = ai_ro(&w).tasks.get(&a).map(|t| t.kind) == Some(crate::ai::TaskKind::Refill);
        assert_eq!(trip, refuel == 1, "refuel {refuel}");
        assert_eq!(ai_ro(&w).counters.refuels > 0, refuel == 1);
    }
}

/// A3: a rifle squad attacks a held trench and no flame squad does: the
/// urgent buy takes the Assault Section, once while it lives (`cards_max`
/// 1). Control: `cards_max` 0 buys none.
#[test]
fn flame_buy_once_per_cap() {
    for cap in [1u32, 0] {
        let mut w = flame_ai_world(1, |s| {
            s.rules.ai.flame.cards_max = cap;
            s.rules.start_manpower = 4000;
            // No mix slot (no tag): only the urgent buy calls it in.
            for t in s.squad_types.iter_mut() {
                if t.id == ASSAULT {
                    t.ai_weight = 1;
                    t.initial_cooldown_ticks = 0;
                    t.tags.clear();
                }
            }
        });
        w.state.game.as_mut().unwrap().players[1].manpower = 4000;
        defenders(&mut w, true);
        let (pos, dir) = axis(&w);
        at(&mut w, 1, US_RIFLES, pos + dir * Fx::from_int(25));
        play_until(&mut w, 120, |_| false);
        assert_eq!(ai_ro(&w).counters.flame_buys, cap, "cap {cap}");
    }
}

/// A1: a manned enemy bunker is flamed at its slit: the aim is the roof's
/// centre plus its facing times the roof's half depth, which is the gap
/// between the two front timbers. Control: the aim is not the centre.
#[test]
fn ai_flames_the_slit() {
    let mut w = flame_ai_world(1, |_| {});
    let (pos, dir) = axis(&w);
    let f = w
        .match_setup()
        .unwrap()
        .forts
        .iter()
        .position(|f| f.id == "ww2:bunker")
        .unwrap() as u16;
    assert!(w.build_now(f, &[pos, pos + dir * Fx::from_int(10)], PlayerId(0)));
    let d = at(&mut w, 0, GER, pos);
    unarm(&mut w, d);
    for (k, m) in members(&w, d).into_iter().enumerate().take(3) {
        w.place_soldier(m, pos + dir.perp() * Fx::from_int(k as i32 - 1));
    }
    attacker(&mut w);
    let ok = play_until(&mut w, 150, |w| {
        job(w).is_some_and(|(_, j)| j.phase == FlamePhase::Burn)
    });
    assert!(ok, "a slit burn: {:?}", job(&w));
    let (_, j) = job(&w).unwrap();
    assert!(j.bunker);
    let pieces: Vec<_> = w.game().unwrap().fort.pieces.values().cloned().collect();
    let roof = pieces.iter().find(|q| q.part == 0).unwrap();
    assert_eq!(j.target, roof.pos);
    assert!(
        (j.aim - roof.pos).length() > Fx::from_int(2),
        "control: not the centre"
    );
    let front: Vec<FxVec2> = pieces
        .iter()
        .filter(|q| q.part != 0 && (q.pos - roof.pos).dot(j.aim - roof.pos) > Fx::from_int(2))
        .map(|q| q.pos)
        .collect();
    assert_eq!(front.len(), 2, "two front timbers");
    let gap = (front[0] + front[1]).div_int(2);
    assert!(
        (gap - j.aim).length() <= Fx::from_milli(300),
        "{gap:?} {:?}",
        j.aim
    );
    play_until(&mut w, 3, |_| false);
    let u = &w.state.entities.unit[j.man.index()];
    assert!(matches!(u.order, UnitOrder::Suppress { line } if line[0] == j.aim));
}

/// A1 Clear: with the trench burnt out (its men gone), the job orders
/// `ClearTrench` at it and the squad clears. Control: `clear` 0 ends the
/// job done instead.
#[test]
fn ai_clears_after_burn() {
    for clear in [1u32, 0] {
        let (mut w, a, d) = trench_scene(1, |s| s.rules.ai.flame.clear = clear);
        assert!(play_until(&mut w, 150, |w| job(w)
            .is_some_and(|(_, j)| j.phase == FlamePhase::Burn)));
        for m in members(&w, d) {
            w.kill_soldier(m);
        }
        let ok = play_until(&mut w, 30, |w| w.game().unwrap().clears().contains_key(&a));
        assert_eq!(
            ok,
            clear == 1,
            "clear {clear}: {:?} {:?}",
            job(&w),
            ends(&w)
        );
        if clear == 0 {
            assert!(ends(&w)[0] > 0, "done");
        } else {
            assert!(ai_ro(&w).counters.clear_orders >= 1);
        }
    }
}

/// A4: a rifle squad near a known enemy in a trench is ordered to clear it
/// (once per `recheck_s`). Control: `clear` 0 orders none.
#[test]
fn ai_clear_orders_near_a_trench() {
    for clear in [1u32, 0] {
        let mut w = flame_ai_world(1, |s| {
            s.rules.ai.flame.jobs = 0;
            s.rules.ai.flame.clear = clear;
        });
        defenders(&mut w, true);
        let (pos, dir) = axis(&w);
        let r = at(&mut w, 1, US_RIFLES, pos + dir * Fx::from_int(25));
        let frag = ItemKind::Frag.index();
        for m in members(&w, r) {
            w.edit_soldier(m, |k, _| k.items[frag] = 0);
        }
        // Two men with two frags each (the throws habit may spend some).
        for m in members(&w, r).into_iter().take(2) {
            w.edit_soldier(m, |k, _| k.items[frag] = 2);
        }
        let ok = play_until(&mut w, 60, |w| w.game().unwrap().clears().contains_key(&r));
        assert_eq!(
            ok,
            clear == 1,
            "clear {clear}: {}",
            ai_ro(&w).counters.clear_orders
        );
        assert_eq!(ai_ro(&w).counters.clear_orders, clear, "once per recheck_s");
    }
}

/// Liveness, determinism: the scene's hashes are equal at 1 and 4
/// threads, and through a save and load in each job phase reached.
#[test]
fn flame_ai_hash_is_thread_count_free() {
    let ticks = 30 * 90;
    let run = |threads: usize| {
        let (mut w, _, _) = trench_scene(threads, |_| {});
        let mut h = Vec::new();
        let mut phases = Vec::new();
        for _ in 0..ticks {
            w.step(&[]);
            if let Some((_, j)) = job(&w)
                && !phases.contains(&j.phase)
            {
                phases.push(j.phase);
                let b = World::load(&w.save(), threads).unwrap();
                assert_eq!(b.full_state_hash(), w.full_state_hash(), "{:?}", j.phase);
            }
            if w.state.tick.is_multiple_of(15) {
                h.push(w.state_hash());
            }
        }
        (h, phases)
    };
    let (one, phases) = run(1);
    assert!(phases.len() >= 2, "{phases:?}");
    assert_eq!(run(4).0, one);
}

/// The S14-ai state is saved, hashed and checked on load.
#[test]
fn flame_ai_state_saves_and_is_checked() {
    let (mut w, a, _) = trench_scene(1, |_| {});
    assert!(play_until(&mut w, 60, |w| job(w).is_some()));
    let tick = w.tick();
    ai_mut(&mut w)
        .flame
        .failed
        .push((FxVec2::from_int(10, 10), tick));
    ai_mut(&mut w).flame.cleared.insert(a, tick);
    let bytes = w.save();
    let h = w.full_state_hash();
    assert_eq!(World::load(&bytes, 1).unwrap().full_state_hash(), h);
    type Edit = Box<dyn Fn(&mut AiState)>;
    let hashed: Vec<Edit> = vec![
        Box::new(|s| s.flame.jobs.values_mut().next().unwrap().best += 1),
        Box::new(|s| s.flame.failed[0].1 -= 1),
        Box::new(|s| *s.flame.cleared.values_mut().next().unwrap() -= 1),
    ];
    for (k, e) in hashed.into_iter().enumerate() {
        let mut x = World::load(&bytes, 1).unwrap();
        e(ai_mut(&mut x));
        let y = World::load(&x.save(), 1).unwrap_or_else(|e| panic!("edit {k}: {e:?}"));
        assert_ne!(y.full_state_hash(), h, "edit {k} is not hashed");
    }
    let bad: Vec<Edit> = vec![
        Box::new(|s| s.flame.jobs.values_mut().next().unwrap().point = 9),
        Box::new(|s| s.flame.jobs.values_mut().next().unwrap().phase_at = u64::MAX),
        Box::new(|s| s.flame.jobs.values_mut().next().unwrap().aim.x = Fx::from_int(-1)),
        Box::new(|s| s.flame.failed[0].1 = u64::MAX),
        Box::new(|s| {
            let j = s.flame.jobs.values().next().unwrap().clone();
            for k in 0..5 {
                s.flame.jobs.insert(SquadId(k), j.clone());
            }
        }),
    ];
    for (k, e) in bad.into_iter().enumerate() {
        let mut x = World::load(&bytes, 1).unwrap();
        e(ai_mut(&mut x));
        assert!(World::load(&x.save(), 1).is_err(), "bad edit {k} loads");
    }
}
