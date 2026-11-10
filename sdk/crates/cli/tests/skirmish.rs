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
    // S3c report-only counters: the map's points, each player's base, and
    // the near point (B for base 0, D for base 1) taken within 2 minutes.
    assert_eq!(r.point_names, ["A", "B", "C", "D", "E"]);
    assert_eq!((r.players[0].base, r.players[1].base), (1, 0));
    for p in &r.players {
        let s = &p.s3c;
        assert_eq!(s.first_capture_s.len(), 5);
        assert_eq!(s.points_at.len(), 4);
        assert!(s.points_at.iter().all(Option::is_none), "match under 5 min");
        let near = if p.base == 0 { 1 } else { 3 };
        assert!(s.first_capture_s[near].is_some_and(|t| t <= 120), "{s:?}");
        assert!(s.point_hold_s.iter().all(|&h| h <= 120));
        assert_eq!(s.tank_alive_s, 0, "no tanks before the unlock");
    }
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
    // S3b Fix (review M5): supported, unsupported, and supported against a
    // trench with its own MG team, per side.
    assert_eq!(r.suppress.runs.len(), 8);
    assert_eq!(r.suppress.runs.iter().filter(|r| r.second).count(), 2);
    let defended: Vec<_> = r.suppress.runs.iter().filter(|r| r.def_mg).collect();
    assert_eq!(defended.len(), 2);
    for run in defended {
        assert!(run.mg, "{run:?}");
        assert!(run.defender_men > 8, "the MG team counts: {run:?}");
    }
    assert!(r.suppress.defended.defender_men > r.suppress.supported.defender_men);
    let text = frontline_cli::fortify::summary_text(&r);
    assert!(text.contains("fortify-test: "), "{text}");
    assert!(text.contains("T10, ww2:germany entrenched"), "{text}");
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
    // A real change gives another match: more starting manpower, more
    // men in the field by minute 2 (not just another setup hash).
    let path = tmp("manpower.toml");
    std::fs::write(&path, "[economy]\nstart_manpower = 1200\n").unwrap();
    let changed = run(SkirmishArgs {
        rules_override: vec![path.clone()],
        ..short(2, 2, 2)
    });
    assert_ne!(changed.final_hash, base.final_hash);
    assert!(
        changed.totals.peak_soldiers > base.totals.peak_soldiers,
        "{} vs {}",
        changed.totals.peak_soldiers,
        base.totals.peak_soldiers
    );
    // Overrides apply in order: the second restores the value.
    let back = tmp("manpower_back.toml");
    std::fs::write(&back, "[economy]\nstart_manpower = 600\n").unwrap();
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

/// `--json FILE` (infra step): the file parses and every number in it equals
/// the one in the `--summary` text; the normal output is unchanged by it.
#[test]
fn json_file_matches_the_summary_text_and_leaves_stdout_alone() {
    let exe = env!("CARGO_BIN_EXE_frontline-cli");
    let file = tmp("many.json");
    let _ = std::fs::remove_file(&file);
    let args = [
        "skirmish",
        "--ai-vs-ai",
        "--seed",
        "7",
        "--seeds",
        "2",
        "--both-bases",
        "--minutes",
        "2",
        "--jobs",
        "2",
        "--summary",
    ];
    let plain = Command::new(exe).args(args).output().unwrap();
    let with = Command::new(exe)
        .args(args)
        .arg("--json")
        .arg(&file)
        .output()
        .unwrap();
    assert!(plain.status.success() && with.status.success());
    // Same text (the tick timing line varies with the machine's load).
    let strip = |b: &[u8]| {
        String::from_utf8_lossy(b)
            .lines()
            .filter(|l| !l.starts_with("tick p99"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(strip(&plain.stdout), strip(&with.stdout));

    let j: serde_json::Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(j["schema"], 1);
    assert_eq!(j["kind"], "skirmish");
    assert_eq!(j["matches"], 4);
    let text = String::from_utf8_lossy(&with.stdout).into_owned();

    // Per match: seed, side, winner, winning base, length, HE share, hash.
    let rows: Vec<&str> = text
        .lines()
        .filter(|l| l.contains(" normal ") || l.contains(" swapped "))
        .filter(|l| l.contains("0x"))
        .collect();
    let per = j["per_match"].as_array().unwrap();
    assert_eq!(rows.len(), per.len());
    for (row, m) in rows.iter().zip(per) {
        let f: Vec<&str> = row.split_whitespace().collect();
        assert_eq!(f[0], m["seed"].to_string());
        assert_eq!(f[1], m["side"].as_str().unwrap());
        assert_eq!(f[2], m["winner_faction"].as_str().unwrap_or("time_limit"));
        let base = m["winner_base"]
            .as_u64()
            .map_or("-".into(), |b| b.to_string());
        assert_eq!(f[3], base);
        let s = m["length_s"].as_f64().unwrap() as u64;
        assert_eq!(f[4], format!("{}:{:02}", s / 60, s % 60));
        let he: f64 = f[f.len() - 2].parse().unwrap();
        assert!((he - m["he_share_pct"].as_f64().unwrap()).abs() < 0.051);
        assert_eq!(f[f.len() - 1], m["hash"].as_str().unwrap());
    }

    // Aggregates against the text lines.
    let line = |p: &str| text.lines().find(|l| l.starts_with(p)).unwrap().to_string();
    let wb = line("wins by base:");
    assert!(
        wb.starts_with(&format!(
            "wins by base: base 0 {}, base 1 {}; no winner {}",
            j["wins"]["base0"], j["wins"]["base1"], j["wins"]["no_winner"]
        )),
        "{wb}"
    );
    let he = line("HE share of kills:");
    assert!(he.contains(&format!(
        "mean {:.1} %, max {:.1} %",
        j["he_share_pct"]["mean"].as_f64().unwrap(),
        j["he_share_pct"]["max"].as_f64().unwrap()
    )));
    let len = line("length min:");
    assert!(len.contains(&format!(
        "mean {:.1}, {:.1}-{:.1}, {} over 25",
        j["length_min"]["mean"].as_f64().unwrap(),
        j["length_min"]["min"].as_f64().unwrap(),
        j["length_min"]["max"].as_f64().unwrap(),
        j["length_min"]["over_25"]
    )));
    for f in ["germany", "usa"] {
        let row = text
            .lines()
            .find(|l| l.starts_with(&format!("ww2:{f} ")) && l.contains('/'))
            .unwrap();
        let c: Vec<&str> = row.split_whitespace().collect();
        let jf = &j["factions"][f];
        assert_eq!(c[1], jf["wins"].to_string());
        assert_eq!(j["wins"][f], jf["wins"]);
        assert_eq!(
            c[2],
            format!("{}/{}", jf["wins_by_base"][0], jf["wins_by_base"][1])
        );
        assert_eq!(
            c.last().unwrap().to_string(),
            format!("{}/{}", jf["kills"], jf["losses"])
        );
        // Ammunition per kind (the S5a line).
        let a = text
            .lines()
            .find(|l| l.starts_with(&format!("S5a ww2:{f}: MP on ammo")))
            .unwrap();
        let m = &jf["ammo"]["spend_mp"];
        assert!(
            a.contains(&format!(
                "small arms {:.0}, explosives {:.0}, shells {:.0} (total {:.0}, {:.1}/match)",
                m["small_arms"].as_f64().unwrap(),
                m["explosives"].as_f64().unwrap(),
                m["shells"].as_f64().unwrap(),
                m["total"].as_f64().unwrap(),
                m["per_match"].as_f64().unwrap()
            )),
            "{a}"
        );
    }
    // The tick line is timing; compare it with the same rounding.
    let tick = line("tick p99 ms");
    assert!(tick.contains(&format!(
        "peak {:.3}, all {:.3}",
        j["tick_p99_ms"]["peak"].as_f64().unwrap(),
        j["tick_p99_ms"]["all"].as_f64().unwrap()
    )));

    // One match writes the same shape.
    let one = tmp("one.json");
    let out = Command::new(exe)
        .args(["skirmish", "--ai-vs-ai", "--seed", "3", "--minutes", "1"])
        .arg("--json")
        .arg(&one)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3));
    let j1: serde_json::Value = serde_json::from_slice(&std::fs::read(&one).unwrap()).unwrap();
    assert_eq!(j1["matches"], 1);
    assert_eq!(j1["per_match"][0]["seed"], 3);
    assert!(
        j1["per_match"][0]["hash"]
            .as_str()
            .unwrap()
            .starts_with("0x")
    );
}

/// `targets` plays the runs it needs, prints one line per target and exits 1
/// on a hard miss; `--from-json` reads the saved runs back to the same lines.
#[test]
fn targets_command_prints_lines_and_exits_by_hard_misses() {
    let exe = env!("CARGO_BIN_EXE_frontline-cli");
    let dir = tmp("targets");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("t.toml");
    std::fs::write(
        &file,
        r#"
[runs.s2]
kind = "skirmish"
seeds = 2
both_bases = true
minutes = 2
[runs.other]
kind = "fortify-test"
seeds = 1

[[target]]
id = "A1"
metric = "s2.matches"
min = 4
max = 4
[[target]]
id = "A2"
metric = "s2.wins.base_max_pct"
min = 0
max = 100
kind = "report"
[[target]]
id = "A3"
metric = "other.pass"
min = 1
"#,
    )
    .unwrap();
    let saved = dir.join("saved");
    let run = Command::new(exe)
        .arg("targets")
        .arg(&file)
        .args(["--jobs", "2", "--runs", "s2"])
        .arg("--out")
        .arg(&saved)
        .output()
        .unwrap();
    assert_eq!(
        run.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let text = String::from_utf8_lossy(&run.stdout).into_owned();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "A1 s2.matches 4 [4,4] PASS", "{text}");
    assert!(lines[1].starts_with("A2 s2.wins.base_max_pct "), "{text}");
    assert!(lines[2].ends_with("SKIP (run other not played)"), "{text}");
    assert!(
        lines[3].starts_with("2 pass, 0 miss (hard: 0), 1 skipped"),
        "{text}"
    );
    assert!(saved.join("s2.json").exists());

    // Reading the saved run gives the same lines; a target the data cannot
    // meet is a hard miss: exit code 1.
    let strict = std::fs::read_to_string(&file)
        .unwrap()
        .replace("min = 4\nmax = 4", "min = 5\nmax = 6");
    let file2 = dir.join("t2.toml");
    std::fs::write(&file2, strict).unwrap();
    let again = Command::new(exe)
        .arg("targets")
        .arg(&file2)
        .arg("--from-json")
        .arg(&saved)
        .output()
        .unwrap();
    assert_eq!(again.status.code(), Some(1));
    let t2 = String::from_utf8_lossy(&again.stdout).into_owned();
    assert!(t2.starts_with("A1 s2.matches 4 [5,6] MISS\n"), "{t2}");
    assert!(
        t2.trim_end()
            .ends_with("1 pass, 1 miss (hard: 1), 1 skipped"),
        "{t2}"
    );
}

/// S10a (spec 7): the AI uses nothing new, so every S10a counter stays
/// zero, per player and per faction in the `--json` document.
#[test]
fn s10_counters_zero_without_fortifications() {
    let r = run(short(3, 4, 2));
    for p in &r.players {
        let a = &p.s10.fort;
        assert_eq!(*a, frontline_sim::fort::FortAccount::default(), "{p:?}");
        assert_eq!(p.s10.new_cards_bought, 0);
    }
    let exe = env!("CARGO_BIN_EXE_frontline-cli");
    let file = tmp("s10.json");
    let _ = std::fs::remove_file(&file);
    let out = Command::new(exe)
        .args(["skirmish", "--ai-vs-ai", "--seed", "3", "--seeds", "1"])
        .args(["--both-bases", "--minutes", "3", "--jobs", "2", "--summary"])
        .arg("--json")
        .arg(&file)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let j: serde_json::Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    for f in ["germany", "usa"] {
        let s10 = j["factions"][f]["s10"].as_object().expect("an s10 block");
        for k in [
            "works_started",
            "new_cards_bought",
            "wire_built",
            "bangalores",
        ] {
            assert!(s10.contains_key(k), "{f}.s10.{k}");
        }
        for (k, v) in s10 {
            assert_eq!(v.as_f64(), Some(0.0), "{f}.s10.{k}");
        }
    }
}
