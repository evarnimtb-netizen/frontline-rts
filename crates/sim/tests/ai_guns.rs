//! S9 AI with AT guns (Build C): an AI-vs-AI skirmish on the shipped mod buys,
//! tows and emplaces its guns; the same hashes at 1, 2, 3 and 8 threads;
//! save, load and continue with the gun jobs and tows live.

use std::path::Path;

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::{PlayerId, World};

fn setup(seed: u64) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let req = MatchRequest::skirmish(seed, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let mut s = defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
    s.players.iter_mut().for_each(|p| p.is_ai = true);
    // A gun early on both sides.
    s.rules.start_manpower = 2500;
    s.rules.ai.guns.income_max_pct = 100;
    s.rules.ai.guns.after_s = 0;
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

#[test]
#[ignore = "slow in debug: runs in the release test pass"]
fn ai_guns_emplace_and_are_deterministic() {
    let ticks = 30 * 360;
    // A seed in which both sides have a gun in place by the save point.
    let seed = (11..31)
        .find(|&s| {
            let mut w = World::new_match(setup(s), 1).unwrap();
            run(&mut w, 30 * 300);
            let g = w.game().unwrap();
            (0..2u8).all(|p| {
                let a = g.ai(PlayerId(p)).unwrap();
                g.players()[p as usize].s9.guns_emplaced >= 1 && !a.guns.is_empty()
            })
        })
        .expect("a seed with guns emplaced on both sides by 5:00");
    let mut base = World::new_match(setup(seed), 1).unwrap();
    let want = run(&mut base, ticks);
    let g = base.game().unwrap();
    for p in 0..2u8 {
        let c = &g.ai(PlayerId(p)).unwrap().counters;
        assert!(c.gun_unhitches >= 1, "player {p}: {c:?}");
        assert!(
            c.gun_ai_fails <= 4,
            "player {p}: no gun move loops: {}",
            c.gun_ai_fails
        );
    }
    for threads in [2, 3, 8] {
        let mut w = World::new_match(setup(seed), threads).unwrap();
        assert_eq!(run(&mut w, ticks), want, "at {threads} threads");
    }
    // Save at 5:00, load, continue: the same history as the run saved.
    let mut a = World::new_match(setup(seed), 1).unwrap();
    run(&mut a, 30 * 300);
    let bytes = a.save();
    let mut b = World::load(&bytes, 1).unwrap();
    assert_eq!(a.state_hash(), b.state_hash(), "at the load");
    assert_eq!(run(&mut a, 30 * 60), run(&mut b, 30 * 60), "after the load");
}

/// S9 Fix: a gun left idle packs, its tow comes for it, it moves and goes
/// back into action; a save taken while the tow is coming continues alike.
#[test]
#[ignore = "slow in debug: runs in the release test pass"]
fn ai_guns_resite_with_the_tow_and_survive_a_save() {
    use frontline_sim::ai::GunPhase;
    let fetching = |w: &World| {
        let g = w.game().unwrap();
        (0..2u8).find(|&p| {
            g.ai(PlayerId(p))
                .is_some_and(|a| a.guns.values().any(|j| j.phase == GunPhase::Fetch))
        })
    };
    let moved = (11..51).any(|seed| {
        let mut w = World::new_match(setup(seed), 1).unwrap();
        let mut left = 30 * 1500u32;
        while left > 0 && fetching(&w).is_none() {
            w.step(&[]);
            left -= 1;
        }
        let Some(p) = fetching(&w) else { return false };
        let mut b = World::load(&w.save(), 1).unwrap();
        assert_eq!(w.state_hash(), b.state_hash(), "at the load");
        assert_eq!(run(&mut w, 30 * 30), run(&mut b, 30 * 30), "after the load");
        let c = &w.game().unwrap().ai(PlayerId(p)).unwrap().counters;
        let before = c.gun_unhitches;
        run(&mut w, 30 * 200);
        let g = w.game().unwrap();
        g.ai(PlayerId(p)).unwrap().counters.gun_unhitches > before
    });
    assert!(moved, "a seed where a re-sited gun reaches a new place");
}
