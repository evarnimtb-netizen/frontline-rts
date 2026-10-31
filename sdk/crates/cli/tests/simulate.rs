//! End-to-end checks of `frontline-cli` through its library API: thread-count
//! determinism, save → load → continue, schema export and the bench
//! scenarios. One test drives the real binary to cover argument parsing and
//! the report file.

use std::path::{Path, PathBuf};
use std::process::Command;

use frontline_cli::report::{Report, TickHash};
use frontline_cli::{SimulateArgs, lower, schema, simulate, validate};
use frontline_content::Scenario;

/// 600 soldiers (3 id chunks) on a small noise map, firing fast into a
/// smallish pool so shots are also dropped: every stage has work.
const BUSY: &str = r#"
name = "busy"
seed = 7
hash_interval = 15

[map]
size_m = 256
heightfield = { kind = "noise" }

[forces]
players = 3
soldiers = 600

[projectiles]
pool_cap = 150
fire_interval_ms = { min = 100, max = 600 }
"#;

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("frontline-cli-tests");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn busy_scenario(file: &str) -> PathBuf {
    let path = tmp(file);
    std::fs::write(&path, BUSY).unwrap();
    path
}

fn run(args: SimulateArgs) -> Report {
    simulate(&args).unwrap_or_else(|e| panic!("simulate failed: {e:#}"))
}

#[test]
fn thread_count_never_changes_hashes() {
    let scenario = busy_scenario("threads.toml");
    let reports: Vec<Report> = [1, 2, 3, 8]
        .into_iter()
        .map(|threads| {
            run(SimulateArgs {
                scenario: Some(scenario.clone()),
                ticks: 300,
                seed: Some(11),
                threads: Some(threads),
                ..Default::default()
            })
        })
        .collect();
    let base = &reports[0];
    assert_eq!(base.hashes.len(), 20, "one hash every 15 ticks");
    assert!(base.stats.shots_fired > 0 && base.stats.shots_dropped > 0);
    assert!(base.entities.projectiles_live > 0);
    for r in &reports[1..] {
        assert_eq!(r.hashes, base.hashes, "threads {}", r.run.threads);
        assert_eq!(r.final_hash, base.final_hash, "threads {}", r.run.threads);
    }

    // The seed flag does matter.
    let other = run(SimulateArgs {
        scenario: Some(scenario),
        ticks: 300,
        seed: Some(12),
        threads: Some(2),
        ..Default::default()
    });
    assert_ne!(other.final_hash, base.final_hash);
}

#[test]
fn save_load_continue_matches_straight_run() {
    let scenario = busy_scenario("saveload.toml");
    let save = tmp("mid_barrage.flsave");
    let _ = std::fs::remove_file(&save);

    let straight = run(SimulateArgs {
        scenario: Some(scenario.clone()),
        ticks: 450,
        threads: Some(3),
        ..Default::default()
    });

    // Same run, saving mid-barrage: saving must not perturb anything.
    let first = run(SimulateArgs {
        scenario: Some(scenario.clone()),
        ticks: 450,
        threads: Some(2),
        save_at: Some(200),
        save: Some(save.clone()),
        ..Default::default()
    });
    assert_eq!(first.final_hash, straight.final_hash);
    let saved = first.run.saved.as_ref().expect("save info");
    assert_eq!(saved.tick, 200);
    assert!(saved.projectiles_live > 0, "save should be mid-barrage");

    // Continue from the save with a different thread count, and with the
    // scenario given as a cross-check.
    let cont = run(SimulateArgs {
        scenario: Some(scenario.clone()),
        ticks: 250,
        threads: Some(8),
        load: Some(save.clone()),
        ..Default::default()
    });
    assert_eq!(cont.run.start_tick, 200);
    assert_eq!(cont.final_tick, 450);
    assert_eq!(cont.final_hash, straight.final_hash);
    let tail: Vec<TickHash> = straight
        .hashes
        .iter()
        .filter(|h| h.tick > 200)
        .cloned()
        .collect();
    assert_eq!(cont.hashes, tail);

    // A scenario that does not match the save is refused with the reason.
    let other = tmp("saveload_other.toml");
    std::fs::write(&other, BUSY.replace("soldiers = 600", "soldiers = 500")).unwrap();
    let err = simulate(&SimulateArgs {
        scenario: Some(other),
        ticks: 1,
        load: Some(save.clone()),
        ..Default::default()
    })
    .unwrap_err()
    .to_string();
    assert!(err.contains("soldiers: scenario 500, save 600"), "{err}");

    // --save-at outside the run is rejected.
    let err = simulate(&SimulateArgs {
        scenario: Some(scenario),
        ticks: 10,
        save_at: Some(11),
        save: Some(tmp("never.flsave")),
        ..Default::default()
    })
    .unwrap_err()
    .to_string();
    assert!(err.contains("outside"), "{err}");
}

#[test]
fn twenty_hz_and_paced_runs_report_tick_stats() {
    let path = tmp("paced.toml");
    std::fs::write(&path, BUSY.replace("seed = 7", "seed = 7\ntick_hz = 20")).unwrap();
    let r = run(SimulateArgs {
        scenario: Some(path),
        ticks: 10,
        threads: Some(2),
        paced: true,
        ..Default::default()
    });
    assert_eq!(r.config.tick_hz, 20);
    assert_eq!(r.config.substeps, 3);
    let p = r.paced.as_ref().expect("paced stats");
    assert_eq!(p.period_ms, 50.0);
    // Ten ticks at 50 ms: the loop must actually wait (>= 9 periods).
    assert!(r.run.wall_ms >= 440.0, "wall {} ms", r.run.wall_ms);
    assert!(r.timing.tick.max_ms >= r.timing.tick.p99_ms);
    assert!(r.timing.tick.p99_ms >= r.timing.tick.p95_ms);
}

#[test]
fn schema_export_writes_valid_json() {
    let dir = tmp("schemas");
    let _ = std::fs::remove_dir_all(&dir);
    let written = schema::export(&dir).unwrap();
    assert_eq!(written, vec![dir.join("scenario.schema.json")]);
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&written[0]).unwrap()).unwrap();
    assert_eq!(json["title"], "Scenario");
    assert!(
        json["$schema"]
            .as_str()
            .unwrap()
            .contains("json-schema.org")
    );
    for key in [
        "name",
        "tick_hz",
        "hash_interval",
        "map",
        "forces",
        "projectiles",
    ] {
        assert!(json["properties"].get(key).is_some(), "missing {key}");
    }
}

#[test]
fn bench_scenarios_are_valid_and_match_the_tiers() {
    let dir = workspace_root().join("bench/scenarios");
    let tiers = [
        ("empty", None),
        ("battalion", Some((700, 1800, 3072))),
        ("regiment", Some((2000, 5000, 4096))),
    ];
    for (name, caps) in tiers {
        let path = dir.join(format!("{name}.toml"));
        let v = validate::scenario_file(&path);
        assert!(v.ok, "{name}: {:?}", v.problems);
        let s = Scenario::load(&path).unwrap();
        assert_eq!(s.name, name);
        let config = lower::sim_config(&s, s.seed).unwrap();
        if let Some((soldiers, pool, map)) = caps {
            assert_eq!(config.soldiers, soldiers);
            assert_eq!(config.projectile_cap, pool);
            assert_eq!(config.map_size_m, map);
            assert_eq!(config.tick_rate.hz(), 30);
        }
    }
}

#[test]
fn lowering_reports_what_the_m0_sim_cannot_do() {
    let src = BUSY.replace(
        "heightfield = { kind = \"noise\" }",
        "heightfield = { kind = \"noise\", seed = 3, amplitude_m = 30 }",
    );
    let s = Scenario::from_toml_str(&src, "x").unwrap();
    let issues = lower::sim_config(&s, 7).unwrap_err();
    let paths: Vec<_> = issues.iter().map(|i| i.path.as_str()).collect();
    assert_eq!(
        paths,
        ["map.heightfield.seed", "map.heightfield.amplitude_m"]
    );
    assert!(issues.iter().all(|i| i.hint.is_some()));
    // A terrain seed equal to the world seed is fine.
    let src = BUSY.replace(
        "heightfield = { kind = \"noise\" }",
        "heightfield = { kind = \"noise\", seed = 7 }",
    );
    let s = Scenario::from_toml_str(&src, "x").unwrap();
    assert!(lower::sim_config(&s, 7).is_ok());
}

#[test]
fn binary_writes_report_and_rejects_bad_files() {
    let exe = env!("CARGO_BIN_EXE_frontline-cli");
    let report = tmp("empty_report.json");
    let _ = std::fs::remove_file(&report);
    let scenario = workspace_root().join("bench/scenarios/empty.toml");
    let out = Command::new(exe)
        .args(["simulate", "--ticks", "90", "--seed", "3", "--threads", "2"])
        .arg("--scenario")
        .arg(&scenario)
        .arg("--report")
        .arg(&report)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&report).unwrap()).unwrap();
    assert_eq!(json["final_tick"], 90);
    assert_eq!(json["config"]["seed"], 3);
    assert_eq!(json["scenario"]["name"], "empty");
    assert_eq!(json["hashes"].as_array().unwrap().len(), 3);
    assert!(json["final_hash"].as_str().unwrap().starts_with("0x"));
    for stage in [
        "intent",
        "movement",
        "weapons",
        "ballistics",
        "world_hash",
        "total",
    ] {
        assert!(
            json["timing"]["stages"][stage]["p99_ms"].is_number(),
            "{stage}"
        );
    }
    assert_eq!(json["entities"]["soldiers"], 16);
    assert!(json["paced"].is_null());

    // A broken scenario fails with every problem listed: content rules and
    // what the M0 sim cannot run, together.
    let bad = tmp("bad.toml");
    std::fs::write(
        &bad,
        BUSY.replace("size_m = 256", "size_m = 300")
            .replace("players = 3", "players = 11")
            .replace("kind = \"noise\"", "kind = \"noise\", amplitude_m = 20"),
    )
    .unwrap();
    let out = Command::new(exe)
        .args(["simulate", "--ticks", "1", "--scenario"])
        .arg(&bad)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("map.size_m") && err.contains("size_m = 256 or size_m = 320"),
        "{err}"
    );
    assert!(err.contains("forces.players"), "{err}");
    assert!(err.contains("map.heightfield.amplitude_m"), "{err}");
    assert!(err.contains("3 problem(s)"), "{err}");

    let out = Command::new(exe)
        .args(["validate", "--json"])
        .arg(&bad)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json[0]["problems"].as_array().unwrap().len(), 3);
}
