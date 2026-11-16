//! S10b-ai (docs/slices/S10b-ai.md 2): the `[ai.obstacles]` table, the
//! `[fortify]` mine path keys and `[ai.truck] refusal_memory` parse from the
//! ww2 rules, reach the setup, work with overrides (`--set`) and are
//! range-checked.

use std::path::{Path, PathBuf};

use frontline_content::tuning::{AiObstacleTuning, FortTuning};
use frontline_content::{Difficulty, MatchRequest, ModDefs};

fn ww2_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mods/ww2")
}

fn write(name: &str, text: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "frontline-s10b-ai-{name}-{}.toml",
        std::process::id()
    ));
    std::fs::write(&p, text).unwrap();
    p
}

/// Control: widen `jobs_max` to `0..=16` in `AiObstacleTuning` and the
/// `jobs_max = 0` override is accepted (the test fails there).
#[test]
fn ai_obstacle_tuning_parses_and_validates() {
    let mut defs = ModDefs::load(&ww2_root()).unwrap_or_else(|e| panic!("{e}"));
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let s = defs.resolve(&req).unwrap();
    // The mod restates the starting values, every switch on (S10c-ai
    // Build B chunk 2: `front` on too, its starting value 0).
    assert_eq!(
        s.rules.ai.obstacles,
        AiObstacleTuning {
            front: 1,
            ..AiObstacleTuning::default()
        }
    );
    let o = &s.rules.ai.obstacles;
    assert_eq!(
        (
            o.mines,
            o.at_mines,
            o.obstacles,
            o.bunker,
            o.sweep,
            o.demolish
        ),
        (1, 1, 1, 1, 1, 1)
    );
    assert_eq!(
        (
            o.jobs_max,
            o.materials_max_pct,
            o.job_max_s,
            o.refused_s,
            o.refused_m
        ),
        (1, 3, 900, 300, 20)
    );
    // Build E: `bunker_assign`, `bunker_truck_first` (switches, on) and
    // `barrier_job_max_s` 1200.
    assert_eq!(
        (o.bunker_assign, o.bunker_truck_first, o.barrier_job_max_s),
        (1, 1, 1200)
    );
    // Build E chunk 2: `lane_file` (switch, on) and `at_rows` 2.
    assert_eq!((o.lane_file, o.at_rows), (1, 2));
    // B1-0: `demolish_handover` (switch, on).
    assert_eq!(o.demolish_handover, 1);
    // B1-0: `bunker_yields` (switch, on).
    assert_eq!(o.bunker_yields, 1);
    // S10c-ai: fourteen more keys (s10c_ai_content.rs checks them).
    assert_eq!(AiObstacleTuning::KEYS.len(), 51);
    let tables = frontline_content::tuning::TABLES;
    assert!(
        tables
            .iter()
            .any(|(t, k)| *t == "ai.obstacles" && k.len() == 51)
    );
    let f = s.rules.fortify.as_ref().unwrap();
    assert_eq!(
        (
            f.mine_paths,
            f.mine_path_cost_pct,
            f.mine_band_foot_cm,
            f.mine_band_vehicle_cm
        ),
        (1, 2000, 300, 400)
    );
    assert!(FortTuning::KEYS.iter().any(|k| k.0 == "mine_paths"));
    assert_eq!(s.rules.ai.truck.refusal_memory, 1);
    // An override reaches the setup (the ablation switches and the loose
    // control's keys).
    let off = write(
        "off",
        "[ai.obstacles]\nmines = 0\nat_mines = 0\nobstacles = 0\nbunker = 0\nsweep = 0\n\
         demolish = 0\nrefused_s = 0\njob_max_s = 36000\nmaterials_max_pct = 100\n\
         [fortify]\nmine_paths = 0\n[ai.truck]\nrefusal_memory = 0\n",
    );
    defs.apply_override(&off, "ww2:skirmish")
        .unwrap_or_else(|e| panic!("{e}"));
    let s = defs.resolve(&req).unwrap();
    let o = &s.rules.ai.obstacles;
    assert_eq!(
        (
            o.mines,
            o.at_mines,
            o.obstacles,
            o.bunker,
            o.sweep,
            o.demolish
        ),
        (0, 0, 0, 0, 0, 0)
    );
    assert_eq!(
        (o.refused_s, o.job_max_s, o.materials_max_pct),
        (0, 36000, 100)
    );
    assert_eq!(o.at_out_m, 60, "the other keys stay");
    assert_eq!(s.rules.fortify.as_ref().unwrap().mine_paths, 0);
    assert_eq!(s.rules.ai.truck.refusal_memory, 0);
    // Out of range: the issue names the key and a fix.
    for (name, text, key) in [
        ("sw", "[ai.obstacles]\nmines = 2\n", "ai.obstacles.mines"),
        (
            "jobs",
            "[ai.obstacles]\njobs_max = 0\n",
            "ai.obstacles.jobs_max",
        ),
        ("paths", "[fortify]\nmine_paths = 2\n", "fortify.mine_paths"),
        (
            "cost",
            "[fortify]\nmine_path_cost_pct = 99\n",
            "fortify.mine_path_cost_pct",
        ),
        (
            "truck",
            "[ai.truck]\nrefusal_memory = 2\n",
            "ai.truck.refusal_memory",
        ),
    ] {
        let bad = write(name, text);
        let e = defs
            .apply_override(&bad, "ww2:skirmish")
            .err()
            .unwrap_or_else(|| panic!("{key} out of range was accepted"))
            .to_string();
        assert!(e.contains(key), "{e}");
    }
    // An unknown key is refused.
    let typo = write("typo", "[ai.obstacles]\nmine = 1\n");
    let e = defs
        .apply_override(&typo, "ww2:skirmish")
        .unwrap_err()
        .to_string();
    assert!(e.contains("mine"), "{e}");
    // The setup is checked too (saves are untrusted).
    let mut s = defs.resolve(&req).unwrap();
    assert!(s.validate().is_empty());
    s.rules.ai.obstacles.demolish_pieces_max = 4;
    assert!(
        s.validate()
            .iter()
            .any(|i| i.path.contains("ai.obstacles.demolish_pieces_max")
                || i.message.contains("ai.obstacles.demolish_pieces_max")),
        "{:?}",
        s.validate()
    );
}
