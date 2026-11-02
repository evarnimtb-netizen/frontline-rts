//! S3b AI (docs/slices/S3b-balance.md B3–B5) and S3c Part B: AI-vs-AI
//! skirmishes under the test unlock rules use the new rules (bases of fire
//! on `SuppressUnits`, AT-team hunts and rockets, satchel hunts; S3c second
//! bases, sticky hunts, re-crews, crews walking back, denial) and stay
//! deterministic: the
//! same hashes at 1, 2, 3 and 8 threads, and save → load → continue in the
//! middle of the fighting gives the same history.

use std::path::Path;

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::{PlayerId, World};

/// The S1-S3c content frozen before the S4 content switch: these mechanics
/// tests were written for its squads (8-man rifle squads, 3-man MG teams)
/// and keep running on it; the S4 cards are covered by `s4_members.rs`,
/// `variants.rs` and the AI-vs-AI runs.
const S3C_MOD: &str = "../../sdk/crates/content/tests/fixtures/ww2_s3c";

fn setup(seed: u64) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(S3C_MOD);
    let mut defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let rules = defs
        .add_rules_override(&root.join("tests/rules_test_unlock.toml"))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut req = MatchRequest::skirmish(seed, "ww2:germany", "ww2:usa", Difficulty::Normal);
    req.rules = rules;
    let mut s = defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
    s.players.iter_mut().for_each(|p| p.is_ai = true);
    s.rules.start_manpower = 1500;
    s
}

/// Steps `ticks`, one hash per game second.
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

// S3b Fix: seed 2 (seed 1 fires no rocket in the 10 minutes on the Fix
// data; seeds 1-8 give 0, 4, 1, 0, 0, 0, 6, 0 rockets). S3c Part B (map,
// AI, shells flying their full range): seed 18, the first of seeds 1-24
// with rockets, a satchel hunt ended, second bases, re-crews, crews
// walking back and denial fire all in the ten minutes. S3c Tune (AT mix 20,
// at_hunt_clear_m 20, satchel_hunt_m 250): seed 50, the only one of seeds
// 1-60 with all of that in the ten minutes.
const SEED: u64 = 50;
const TICKS: u64 = 30 * 600;
const SAVE_AT: u64 = 30 * 330;

#[test]
fn ai_assault_rules_fire_and_stay_deterministic() {
    let mut base = World::new_match(setup(SEED), 1).unwrap();
    let want = run(&mut base, TICKS);
    let g = base.game().unwrap();
    let c = |p: u8| g.ai(PlayerId(p)).unwrap().counters.clone();
    let rockets: u32 = g.players().iter().map(|p| p.rockets).sum();
    println!("counters: {:?} / {:?}; rockets {rockets}", c(0), c(1));
    let sum = |f: fn(&frontline_sim::ai::AiCounters) -> u32| f(&c(0)) + f(&c(1));
    assert!(
        sum(|c| c.suppress_orders) >= 1,
        "a base of fire or a tank suppressed"
    );
    assert!(sum(|c| c.at_hunts) >= 1, "an AT team hunted");
    assert!(rockets >= 1, "an AT rocket was fired");
    // S3c Part B (docs/slices/S3c-armour.md B2, B4, B5) in the same match,
    // so the determinism runs below cover them: second bases of fire, a
    // satchel hunt (ended and counted), AI re-crews, crews walking back
    // and denial fire. (Rearm is staged in `ai_armour.rs`: tanks rarely run
    // low in ten minutes.)
    assert!(sum(|c| c.second_bases) >= 1, "a second base of fire");
    assert!(sum(|c| c.satchel_hunts) >= 1, "a satchel hunt");
    let ends: u32 = c(0).hunt_ends.iter().chain(&c(1).hunt_ends).sum();
    assert!(ends >= 1, "a sticky hunt ended and was counted");
    assert!(sum(|c| c.recrews) >= 1, "an AI crew re-crewed");
    assert!(sum(|c| c.crew_retreats) >= 1, "an AI crew walked back");
    assert!(
        sum(|c| c.denials) >= 1,
        "an AI tank fired at an enemy abandoned tank"
    );
    for threads in [2, 3, 8] {
        let mut w = World::new_match(setup(SEED), threads).unwrap();
        assert_eq!(run(&mut w, TICKS), want, "{threads} threads");
    }
    // Save in the middle of the fighting, load at another thread count,
    // continue: the same hashes to the end.
    let mut first = World::new_match(setup(SEED), 2).unwrap();
    let head = run(&mut first, SAVE_AT);
    let mut resumed = World::load(&first.save(), 3).unwrap();
    assert_eq!(resumed.state_hash(), first.state_hash());
    drop(first);
    let tail = run(&mut resumed, TICKS - SAVE_AT);
    assert_eq!([head, tail].concat(), want, "save, load, continue");
    assert_eq!(resumed.save(), base.save(), "final save bytes differ");
}
