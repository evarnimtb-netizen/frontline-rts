//! S10a-ai Build B (docs/slices/S10a-ai.md 7): an AI-vs-AI skirmish on the
//! shipped mod with fort jobs under way gives the same hashes at 1, 2, 3 and
//! 8 threads, and a save at 5:00 loads and continues alike.

use std::path::Path;

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::{PlayerId, World};

fn setup(seed: u64) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let req = MatchRequest::skirmish(seed, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let mut s = defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
    s.players.iter_mut().for_each(|p| p.is_ai = true);
    s
}

fn run(w: &mut World, ticks: u64) -> Vec<u64> {
    let mut h = Vec::new();
    for _ in 0..ticks {
        w.step(&[]);
        if w.tick().is_multiple_of(30) {
            h.push(w.state_hash());
        }
    }
    h
}

/// Fort jobs live and works started on both sides.
fn building(w: &World) -> bool {
    let g = w.game().unwrap();
    (0..2u8).all(|p| {
        let a = g.ai(PlayerId(p)).unwrap();
        a.counters.fort_jobs >= 1 && g.fort().accounts[p as usize].works_started >= 1
    })
}

/// Found by the search below; tried first.
const PINNED_SEED: u64 = 1;

#[test]
#[ignore = "slow in debug: runs in the release test pass"]
fn ai_fort_is_deterministic_across_threads_and_save_load() {
    let save_at = 30 * 300;
    let qualifies = |s: u64| {
        let mut w = World::new_match(setup(s), 1).unwrap();
        run(&mut w, save_at);
        building(&w)
            && w.game()
                .unwrap()
                .ai(PlayerId(0))
                .is_some_and(|a| !a.forts.is_empty())
    };
    // The known seed first; the full search only if it stops qualifying.
    let seed = if qualifies(PINNED_SEED) {
        PINNED_SEED
    } else {
        println!("pinned seed {PINNED_SEED} no longer qualifies; searching 1..21");
        (1..21)
            .find(|&s| qualifies(s))
            .expect("a seed with fort jobs on both sides and one live at 5:00")
    };
    println!("seed {seed}");
    let ticks = save_at + 30 * 60;
    let mut base = World::new_match(setup(seed), 1).unwrap();
    let want = run(&mut base, ticks);
    for threads in [2, 3, 8] {
        let mut w = World::new_match(setup(seed), threads).unwrap();
        assert_eq!(run(&mut w, ticks), want, "at {threads} threads");
    }
    let mut a = World::new_match(setup(seed), 1).unwrap();
    run(&mut a, save_at);
    let mut b = World::load(&a.save(), 1).unwrap();
    assert_eq!(a.state_hash(), b.state_hash(), "at the load");
    assert_eq!(
        a.game().unwrap().ai(PlayerId(0)),
        b.game().unwrap().ai(PlayerId(0))
    );
    assert_eq!(run(&mut a, 30 * 60), run(&mut b, 30 * 60), "after the load");
    assert_eq!(a.game().unwrap().fort(), b.game().unwrap().fort());
}

/// Every S10a-ai trace in a match: AI fort and breach state, the AI's fort
/// counters, the fort state and the S10a cards bought (by name, for the
/// failure message).
fn traces(w: &World) -> Vec<String> {
    let g = w.game().unwrap();
    let s = w.match_setup().unwrap();
    let mut out = Vec::new();
    for p in 0..2u8 {
        let a = g.ai(PlayerId(p)).unwrap();
        let c = &a.counters;
        let set = [
            ("forts", !a.forts.is_empty()),
            ("breaches", !a.breaches.is_empty()),
            ("wire_known", !a.wire_known.is_empty()),
            ("wire_cursor", a.wire_cursor != 0),
            ("shelled", !a.shelled.is_empty()),
            ("enemy_mortar_seen", a.enemy_mortar_seen),
            ("bypass", !a.bypass.is_empty()),
            ("material_run", a.material_run.is_some()),
            ("fort_jobs", c.fort_jobs != 0),
            ("fort_refused", c.fort_refused != 0),
            ("fort_build_plans", c.fort_build_plans != 0),
            ("infantry_plans", c.infantry_plans != 0),
            ("shelter_moves", c.shelter_moves != 0),
            ("material_runs", c.material_runs != [0; 3]),
            ("breaches_started", c.breaches_started != 0),
            ("breaches_opened", c.breaches_opened != 0),
            ("breaches_given_up", c.breaches_given_up != 0),
            ("breach_smokes", c.breach_smokes != 0),
            ("breach_suppress", c.breach_suppress != 0),
            ("wire_bypassed", c.wire_bypassed != 0),
        ];
        out.extend(set.iter().filter(|x| x.1).map(|x| format!("p{p} {}", x.0)));
        for (t, ty) in s.squad_types.iter().enumerate() {
            let s10a = ty.has(frontline_content::setup::Capability::BuildHeavy)
                || ty.has(frontline_content::setup::Capability::Breach);
            if s10a && c.bought.get(t).copied().unwrap_or(0) > 0 {
                out.push(format!("p{p} bought {}", ty.id));
            }
        }
    }
    let f = g.fort();
    let fort = [
        ("works", !f.works.is_empty()),
        ("pieces", !f.pieces.is_empty()),
        ("wire", !f.wire.is_empty()),
        ("charges", !f.charges.is_empty()),
        ("carried", !f.carried_g.is_empty()),
        (
            "accounts",
            f.accounts
                .iter()
                .any(|a| *a != frontline_sim::fort::FortAccount::default()),
        ),
    ];
    out.extend(fort.iter().filter(|x| x.1).map(|x| format!("fort {}", x.0)));
    out
}

/// Spec 7 (Build C): with `works`, `wire`, `breach` and `cards` all 0 and
/// the S10b-ai `[ai.obstacles]` habits (`mines`, `obstacles`, `bunker`,
/// `sweep`, `demolish`) all 0 a 15-minute AI-vs-AI match leaves no S10a-ai
/// or S10b-ai trace at all; the same match with the switches on leaves some
/// (the control). With only the S10a switches off (S10b-ai on) the match
/// leaves S10b traces only: fort jobs and works, carried grams, accounts and
/// walk-back runs (`material_runs[2]`: an S10b builder walks home for mines),
/// never an S10a trace (truck runs, breaches, wire, cards, plans).
/// Control: drop the S10b override from the first match -> "all off" fails
/// with the AI's mine jobs (seen: fort_jobs, forts, works, carried, accounts).
#[test]
#[ignore = "three 15-minute matches: runs in the release test pass"]
fn ai_all_switches_off_emit_nothing_new() {
    let ticks = 30 * 60 * 15;
    let s10a_off = |s: &mut MatchSetup| {
        let f = &mut s.rules.ai.fort;
        (f.works, f.wire, f.breach, f.cards) = (0, 0, 0, 0);
    };
    let mut off = setup(3);
    s10a_off(&mut off);
    let o = &mut off.rules.ai.obstacles;
    (o.mines, o.obstacles, o.bunker, o.sweep, o.demolish) = (0, 0, 0, 0, 0);
    let mut w = World::new_match(off, 1).unwrap();
    run(&mut w, ticks);
    assert_eq!(traces(&w), Vec::<String>::new(), "all off");
    let mut only_b = setup(3);
    s10a_off(&mut only_b);
    let mut w = World::new_match(only_b, 1).unwrap();
    run(&mut w, ticks);
    let s10b = [
        "forts",
        "fort_jobs",
        "fort_refused",
        "fort works",
        "fort pieces",
        "fort carried",
        "fort accounts",
        "material_runs",
    ];
    let not_b: Vec<String> = traces(&w)
        .into_iter()
        .filter(|t| !s10b.contains(&t.trim_start_matches(['p', '0', '1', ' '])))
        .collect();
    assert_eq!(not_b, Vec::<String>::new(), "S10a off, S10b-ai on");
    for p in 0..2u8 {
        let runs = w
            .game()
            .unwrap()
            .ai(PlayerId(p))
            .unwrap()
            .counters
            .material_runs;
        assert_eq!(runs[..2], [0, 0], "p{p}: S10a off, no truck run");
    }
    let mut w = World::new_match(setup(3), 1).unwrap();
    run(&mut w, ticks);
    assert!(!traces(&w).is_empty(), "switches on: the AI fortifies");
}

/// Blueprint ids of the live S10b-ai fort jobs (minefield, obstacle,
/// bunker blueprints) of both players.
fn s10b_live(w: &World) -> Vec<String> {
    let g = w.game().unwrap();
    let s = w.match_setup().unwrap();
    let mut out = Vec::new();
    for p in 0..2u8 {
        for j in g.ai(PlayerId(p)).unwrap().forts.values() {
            let d = &s.forts[j.fort as usize];
            let s10b = d.mine_item.is_some()
                || d.blocks_vehicles
                || d.pit == Some(frontline_content::setup::DigProfile::BunkerPit);
            if s10b && !j.parked() && j.phase != frontline_sim::ai::FortPhase::Done {
                out.push(format!("p{p} {}", d.id));
            }
        }
    }
    out
}

/// S10b-ai (spec 5, Build C): an AI-vs-AI skirmish with S10b-ai jobs live
/// gives the same hashes at 1, 2, 3 and 8 threads, and saves at 5:00 and
/// 12:00 (each moved to the first second after it with an S10b-ai job
/// live, at most 6 min later) load to the same state and continue alike.
/// Control: `#[serde(skip)]` on `AiFortJob::start` -> the loaded state
/// differs (seen, see the log).
#[test]
#[ignore = "slow in debug: runs in the release test pass"]
fn ai_s10b_deterministic_threads_and_save_load() {
    let marks: [u64; 2] = [30 * 300, 30 * 720];
    let saves_of = |s: u64| -> Option<[u64; 2]> {
        let mut w = World::new_match(setup(s), 1).unwrap();
        let mut out = [0; 2];
        for (k, &m) in marks.iter().enumerate() {
            let from = m.max(out[0] + 30 * 60);
            let n = from.saturating_sub(w.tick());
            run(&mut w, n);
            while s10b_live(&w).is_empty() && w.tick() < from + 30 * 360 {
                run(&mut w, 30);
            }
            let live = s10b_live(&w);
            println!("seed {s}: at {} s {live:?}", w.tick() / 30);
            if live.is_empty() {
                return None;
            }
            out[k] = w.tick();
        }
        Some(out)
    };
    let (seed, saves) = (1..11)
        .find_map(|s| saves_of(s).map(|v| (s, v)))
        .expect("a seed with S10b-ai jobs live after 5:00 and 12:00");
    println!("seed {seed}, saves at {saves:?} ticks");
    let ticks = saves[1] + 30 * 60;
    let mut base = World::new_match(setup(seed), 1).unwrap();
    let want = run(&mut base, ticks);
    for threads in [2, 3, 8] {
        let mut w = World::new_match(setup(seed), threads).unwrap();
        assert_eq!(run(&mut w, ticks), want, "at {threads} threads");
    }
    let mut a = World::new_match(setup(seed), 1).unwrap();
    for save in saves {
        let n = save - a.tick();
        run(&mut a, n);
        let mut b = World::load(&a.save(), 1).unwrap();
        assert_eq!(a.state_hash(), b.state_hash(), "at the load {save}");
        for p in 0..2u8 {
            assert_eq!(
                a.game().unwrap().ai(PlayerId(p)),
                b.game().unwrap().ai(PlayerId(p)),
                "p{p} AI state at {save}"
            );
        }
        let mut c = World::load(&a.save(), 1).unwrap();
        assert_eq!(
            run(&mut c, 30 * 60),
            run(&mut b, 30 * 60),
            "after the load {save}"
        );
        assert_eq!(c.game().unwrap().fort(), b.game().unwrap().fort());
    }
}
