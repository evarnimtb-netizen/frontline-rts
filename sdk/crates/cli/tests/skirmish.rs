//! `skirmish` and `fortify-test` (S1 "Done when" 1–3) through the library
//! API and the real binary: an AI-vs-AI match gives the same hashes at any
//! thread count and through save → load → continue, its report is filled
//! in, and the trench checks pass.
//!
//! Matches here are a few game minutes long so debug test runs stay quick;
//! whole matches are run with the CLI (README, bench/results/S1-*).

use std::path::{Path, PathBuf};
use std::process::Command;

use frontline_cli::skirmish::SkirmishReport;
use frontline_cli::{FortifyArgs, SkirmishArgs, fortify_test, skirmish};

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("frontline-cli-tests");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn run(args: SkirmishArgs) -> SkirmishReport {
    skirmish(&args).unwrap_or_else(|e| panic!("skirmish failed: {e:#}"))
}

fn short(seed: u64, minutes: u32, threads: usize) -> SkirmishArgs {
    SkirmishArgs {
        seed,
        minutes,
        threads: Some(threads),
        ..SkirmishArgs::default()
    }
}

#[test]
fn skirmish_hashes_match_across_threads_and_save_load() {
    let minutes = 5;
    let reference = run(short(5, minutes, 1));
    assert_eq!(reference.ticks, minutes as u64 * 60 * 30);
    assert_eq!(reference.result, "time_limit");
    assert!(reference.totals.shots_fired > 0, "no fighting yet");
    for threads in [2, 3, 8] {
        let r = run(short(5, minutes, threads));
        assert_eq!(r.final_hash, reference.final_hash, "{threads} threads");
        assert_eq!(r.totals.deaths, reference.totals.deaths);
    }

    // Save mid-match, continue on another thread count.
    let save = tmp("skirmish_mid.sav");
    let first = run(SkirmishArgs {
        save: Some(save.clone()),
        save_at: Some(4 * 60 * 30 + 7),
        ..short(5, minutes, 2)
    });
    assert_eq!(first.final_hash, reference.final_hash);
    let sv = first.saved.as_ref().unwrap();
    assert_eq!(sv.tick, 4 * 60 * 30 + 7);
    let resumed = run(SkirmishArgs {
        ai_vs_ai: false,
        load: Some(save),
        ..short(999, minutes, 3)
    });
    assert_eq!(resumed.start_tick, sv.tick);
    assert_eq!(resumed.seed, 5, "the seed comes from the save");
    assert_eq!(resumed.final_hash, reference.final_hash);
    assert_eq!(resumed.players[0].kills, reference.players[0].kills);
}

#[test]
fn skirmish_report_is_filled_in() {
    let r = run(SkirmishArgs {
        swap_bases: true,
        ..short(2, 2, 2)
    });
    assert_eq!(r.players.len(), 2);
    assert_eq!(r.players[0].faction, "ww2:germany");
    assert_eq!(r.players[1].faction, "ww2:usa");
    for p in &r.players {
        assert_eq!(p.difficulty, "normal");
        assert!(p.ai_commands > 0 && p.squads_fielded >= 3);
        assert!(p.soldiers_alive > 0 && p.defeat.is_none());
    }
    assert!(r.totals.peak_soldiers >= 2 * 19);
    assert!(r.timing.peak_ticks > 0 && r.timing.all.p99_ms > 0.0);
    let text = frontline_cli::skirmish::summary_text(&r);
    assert!(text.contains("no winner within 2 min"), "{text}");
    // Swapping bases changes the match.
    assert_ne!(run(short(2, 2, 2)).final_hash, r.final_hash);
}

#[test]
fn binary_runs_a_skirmish_and_prints_json() {
    let exe = env!("CARGO_BIN_EXE_frontline-cli");
    let out = Command::new(exe)
        .args(["skirmish", "--ai-vs-ai", "--seed", "4", "--minutes", "1"])
        .args(["--difficulty", "hard", "--difficulty1", "easy"])
        .args(["--threads", "2", "--report", "json"])
        .output()
        .unwrap();
    // No winner after one minute: exit code 3, report still printed.
    assert_eq!(
        out.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["seed"], 4);
    assert_eq!(json["ticks"], 1800);
    assert_eq!(json["result"], "time_limit");
    assert_eq!(json["players"][0]["difficulty"], "hard");
    assert_eq!(json["players"][1]["difficulty"], "easy");
    assert!(json["final_hash"].as_str().unwrap().starts_with("0x"));
    assert!(json["timing"]["peak"]["p99_ms"].as_f64().unwrap() > 0.0);

    // A human-vs-AI match is not something the headless CLI can play.
    let out = Command::new(exe).args(["skirmish"]).output().unwrap();
    assert!(!out.status.success());
}

/// The S1 cover bar over the default five seeds, both factions entrenched
/// in turn, plus the S1b grenade assault and smoke screen. Slow in debug
/// builds; `cargo test --release` runs it.
#[test]
#[cfg_attr(debug_assertions, ignore = "long; run with --release")]
fn fortify_test_passes() {
    let r = fortify_test(&FortifyArgs::default()).unwrap();
    assert_eq!(r.rifles.runs.len(), 10);
    assert_eq!(r.mortar.runs.len(), 20);
    assert!(r.rifles.open_losses >= 40, "{}", r.rifles.open_losses);
    assert!(r.mortar.open_losses >= 40, "{}", r.mortar.open_losses);
    assert!(r.rifles.pass, "{r:#?}");
    assert!(r.mortar.pass, "{r:#?}");
    assert!(r.smoke.pass, "{:#?}", r.smoke);
    // From 60 m, from 25 m, and from 60 m behind smoke; each with and
    // without frags.
    assert_eq!(r.grenade.runs.len(), 60);
    assert_eq!(r.smoke.runs.len(), 20);
    // Frags thrown from 25 m cost the defenders clearly more than rifles.
    let close = &r.grenade.variants[1];
    assert_eq!(close.start_m, 25);
    assert!(close.grenades.frags_thrown >= 40, "{close:?}");
    assert!(
        close.grenades.defender_losses >= 2 * close.rifle_only.defender_losses.max(1),
        "{close:?}"
    );
    // Behind their own smoke the attackers get close enough to throw.
    let screened = &r.grenade.variants[2];
    assert!(screened.smoke && screened.start_m == 60, "{screened:?}");
    assert!(screened.grenades.frags_thrown >= 20, "{screened:?}");
    assert!(r.pass);
}

#[test]
fn fortify_test_reports_each_run() {
    let r = fortify_test(&FortifyArgs {
        seeds: 1,
        first_seed: 3,
        ..FortifyArgs::default()
    })
    .unwrap();
    assert_eq!(r.rifles.runs.len(), 2);
    assert_eq!(r.mortar.runs.len(), 4);
    assert_eq!(r.grenade.runs.len(), 12);
    assert_eq!(r.grenade.variants.len(), 3);
    assert_eq!(r.smoke.runs.len(), 4);
    // Behind smoke: never seen or fired on; the control is.
    for run in &r.smoke.runs {
        assert_eq!(run.smoke, run.seen_ticks == 0, "{run:?}");
    }
    assert_eq!(r.rifles.runs[0].entrenched, "ww2:germany");
    assert_eq!(r.rifles.runs[1].entrenched, "ww2:usa");
    // The open squad is the one that suffers.
    assert!(r.rifles.open_losses > r.rifles.trench_losses);
    let text = frontline_cli::fortify::summary_text(&r);
    assert!(text.contains("fortify-test: "), "{text}");
}

fn ww2_override(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../mods/ww2/tests/overrides")
        .join(name)
}

#[test]
fn rules_overrides_reach_the_match_and_restating_the_mod_changes_nothing() {
    // S3b: the example override restates the mod's values: same match.
    let base = run(short(2, 2, 2));
    let same = run(SkirmishArgs {
        rules_override: vec![ww2_override("example.toml")],
        ..short(2, 2, 2)
    });
    assert_eq!(same.final_hash, base.final_hash);
    // A real change (members walk closer behind their slots) gives another
    // match.
    let path = tmp("slot_lead.toml");
    std::fs::write(&path, "[movement.formation]\nslot_lead_mm = 1000\n").unwrap();
    let changed = run(SkirmishArgs {
        rules_override: vec![path.clone()],
        ..short(2, 2, 2)
    });
    assert_ne!(changed.final_hash, base.final_hash);
    // Overrides apply in order: the second restores the value.
    let back = tmp("slot_lead_back.toml");
    std::fs::write(&back, "[movement.formation]\nslot_lead_mm = 1500\n").unwrap();
    let restored = run(SkirmishArgs {
        rules_override: vec![path, back],
        ..short(2, 2, 2)
    });
    assert_eq!(restored.final_hash, base.final_hash);
}

#[test]
fn many_seeds_run_in_parallel_give_the_single_runs_hashes() {
    let many = frontline_cli::skirmish::skirmish_many(&SkirmishArgs {
        seeds: Some(2),
        both_bases: true,
        summary: true,
        jobs: Some(3),
        threads: Some(1),
        ..short(7, 1, 1)
    })
    .unwrap();
    assert_eq!(many.matches.len(), 4);
    assert_eq!(many.summary.matches, 4);
    let order: Vec<(u64, bool)> = many.matches.iter().map(|m| (m.seed, m.swapped)).collect();
    assert_eq!(order, [(7, false), (7, true), (8, false), (8, true)]);
    for m in &many.matches {
        let single = run(SkirmishArgs {
            swap_bases: m.swapped,
            ..short(m.seed, 1, 2)
        });
        assert_eq!(m.report.final_hash, single.final_hash, "seed {}", m.seed);
    }
    let text = frontline_cli::skirmish::many_text(&many);
    assert!(
        text.contains("ww2:germany") && text.contains("wins by base"),
        "{text}"
    );
}
