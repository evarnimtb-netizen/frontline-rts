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

#[test]
#[ignore = "slow in debug: runs in the release test pass"]
fn ai_fort_is_deterministic_across_threads_and_save_load() {
    let save_at = 30 * 300;
    let seed = (1..21)
        .find(|&s| {
            let mut w = World::new_match(setup(s), 1).unwrap();
            run(&mut w, save_at);
            building(&w)
                && w.game()
                    .unwrap()
                    .ai(PlayerId(0))
                    .is_some_and(|a| !a.forts.is_empty())
        })
        .expect("a seed with fort jobs on both sides and one live at 5:00");
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
