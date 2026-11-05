//! Two worlds, an in-process relay and two clients over a simulated
//! network on a virtual clock (S2 "Testing"). Every run checks identical
//! hashes, that every stamped `seq` ran exactly once in order on both
//! worlds, and that restamps are counted the same by the relay and the
//! clients.

use frontline_content::MatchSetup;
use frontline_net::harness::{self, VirtualReport, VirtualSpec};
use frontline_net::protocol::ByeReason;
use frontline_net::relay::LogRecord;
use frontline_net::transport::LinkParams;
use frontline_sim::{Outcome, PlayerId};

fn setup(seed: u64, tickets: Option<u32>) -> MatchSetup {
    let defs = harness::load_defs(&harness::mods_dir(None).unwrap()).unwrap();
    harness::net_setup(&defs, seed, true, tickets).unwrap()
}

/// S3: the test unlock rules (vehicles at once) with 4000 starting
/// manpower, so both AIs field tanks early. S3b: the vehicles' 120 s
/// re-call cooldown is off, so a 3-minute run fields two per side (other
/// call-ins keep their cooldowns, and the scripted ones meet them).
fn vehicle_setup(seed: u64, tickets: Option<u32>) -> MatchSetup {
    let (defs, rules) = harness::load_defs_test_unlock(&harness::mods_dir(None).unwrap()).unwrap();
    let mut s = harness::net_setup_rules(&defs, &rules, seed, true, tickets).unwrap();
    s.rules.start_manpower = 4000;
    for t in s.squad_types.iter_mut().filter(|t| t.vehicle.is_some()) {
        t.recall_cooldown_ticks = 0;
    }
    for v in &mut s.vehicles {
        v.recall_cooldown_ticks = 0;
    }
    s
}

/// The S3 checks on top of [`assert_clean`]: at least two vehicles per
/// side, and every vehicle order scripted by both slots.
fn assert_vehicles(r: &VirtualReport) {
    let s = summary(r);
    assert!(
        r.vehicles_spawned.iter().all(|&n| n >= 2),
        "vehicles spawned {:?}: {s}",
        r.vehicles_spawned
    );
    for (slot, kinds) in r.script_kinds.iter().enumerate() {
        for k in harness::VEHICLE_COMMANDS {
            assert!(
                kinds.get(k).copied().unwrap_or(0) > 0,
                "slot {slot} never scripted {k}: {kinds:?}"
            );
        }
    }
}

/// The lossy internet path of the S2 tests: 15 ms one-way plus 0–80 ms
/// jitter (so datagrams reorder), 5 % loss, 2 % duplicates, and 5 % held
/// back another 40 ms.
fn rough() -> LinkParams {
    LinkParams {
        delay_us: 15_000,
        jitter_us: 80_000,
        drop: 0.05,
        dup: 0.02,
        reorder: 0.05,
        reorder_us: 40_000,
    }
}

/// Game seconds for the default runs (S2: 3 game minutes per seed).
fn minutes3() -> u64 {
    180
}

fn summary(r: &VirtualReport) -> String {
    format!(
        "D={} ticks={:?} hash_checks={} mismatches={} stamped={:?} queued={:?} lost={:?} \
         order_errors={} streams_differ={} restamps={} of {} ({:.3} %) own_restamps={:?} \
         stalls={:?} pauses={} resumes={} desync={:?} byes={:?} outcomes={:?} rtt_p99={:?} \
         net={:?} step_p99={} us step_max={} us",
        r.d,
        r.ticks,
        r.hash_checks,
        r.hash_mismatches,
        r.stamped,
        r.queued,
        r.lost,
        r.order_errors,
        r.streams_differ,
        r.relay_restamps,
        r.relay_commands,
        r.restamp_rate() * 100.0,
        r.own_restamps,
        r.stall_ticks,
        r.pauses,
        r.resumes,
        r.desync,
        r.byes,
        r.outcomes,
        r.rtt_p99_ms,
        r.net,
        r.step_p99_us,
        r.step_max_us,
    )
}

/// The checks every healthy run must pass.
fn assert_clean(r: &VirtualReport, min_ticks: u64) {
    let s = summary(r);
    assert!(
        r.ticks.iter().all(|&t| t >= min_ticks),
        "too few ticks: {s}"
    );
    assert!(
        r.hash_checks * 30 + 60 >= min_ticks,
        "too few hash checks: {s}"
    );
    assert_eq!(r.hash_mismatches, 0, "{s}");
    assert_eq!(r.relay_desync, None, "{s}");
    assert!(!r.streams_differ, "{s}");
    assert_eq!(r.order_errors, 0, "{s}");
    assert_eq!(r.lost, [0, 0], "{s}");
    assert_eq!(r.stamped, r.queued, "every queued order was stamped: {s}");
    assert!(r.stamped.iter().all(|&n| n > 20), "the script ran: {s}");
    assert_eq!(
        r.own_restamps[0] + r.own_restamps[1],
        r.relay_restamps,
        "clients count the relay's restamps: {s}"
    );
}

#[test]
fn loopback_rough_path_seeds_1_to_3() {
    for seed in 1..=3 {
        let spec = VirtualSpec {
            seed,
            seconds: minutes3(),
            link: rough(),
            garbage_per_s: 20,
            ..VirtualSpec::default()
        };
        let r = harness::run_virtual(&setup(seed, None), &spec);
        eprintln!("seed {seed}: {}", summary(&r));
        assert_clean(&r, (minutes3() - 2) * 30);
        assert!(r.relay_commands > 100, "{}", summary(&r));
    }
}

/// S3 "Done when" 4: the rough path with vehicles (test unlock rules):
/// both AIs buy and fight with tanks while the scripts send every vehicle
/// order. Identical hashes, every `seq` executed once, at least two
/// vehicles per side.
#[test]
fn loopback_vehicles_seeds_1_to_3() {
    for seed in 1..=3 {
        let spec = VirtualSpec {
            seed,
            seconds: minutes3(),
            link: rough(),
            garbage_per_s: 20,
            vehicle_orders: true,
            ..VirtualSpec::default()
        };
        let r = harness::run_virtual(&vehicle_setup(seed, None), &spec);
        eprintln!(
            "vehicles seed {seed}: spawned {:?}, scripted {:?} / {:?}: {}",
            r.vehicles_spawned,
            r.script_kinds[0],
            r.script_kinds[1],
            summary(&r)
        );
        assert_clean(&r, (minutes3() - 2) * 30);
        assert_vehicles(&r);
    }
}

/// D forced to 2 on the rough path with 0–120 ms jitter: many commands
/// miss their turn and are restamped, and still none is lost or
/// reordered.
#[test]
fn restamps_keep_every_command() {
    let spec = VirtualSpec {
        seed: 4,
        seconds: 60,
        link: LinkParams {
            jitter_us: 120_000,
            ..rough()
        },
        d_override: Some(2),
        ..VirtualSpec::default()
    };
    let r = harness::run_virtual(&setup(4, None), &spec);
    eprintln!("{}", summary(&r));
    assert_eq!(r.d, 2);
    assert_clean(&r, 58 * 30);
    assert!(r.relay_restamps > 10, "restamps happen: {}", summary(&r));
}

/// A 1 s total drop of slot 1: turns keep closing, its input is restamped,
/// it catches up, and the relay never pauses.
#[test]
fn one_second_drop_resyncs() {
    let spec = VirtualSpec {
        seed: 5,
        seconds: 40,
        blackouts: vec![(1, 20_000_000, 1_000_000)],
        ..VirtualSpec::default()
    };
    let r = harness::run_virtual(&setup(5, None), &spec);
    eprintln!("{}", summary(&r));
    assert_clean(&r, 38 * 30);
    assert_eq!(r.pauses, 0, "{}", summary(&r));
    assert!(
        r.stall_ticks[1] >= 20,
        "slot 1 waited for bundles: {}",
        summary(&r)
    );
}

/// A 10 s blackout of slot 1: the relay pauses at 10 s of silence, resends
/// the missed bundles oldest first when it is back, and resumes once it
/// holds them. No command is lost.
#[test]
fn ten_second_blackout_resyncs() {
    let spec = VirtualSpec {
        seed: 6,
        seconds: 60,
        link: LinkParams {
            delay_us: 20_000,
            jitter_us: 10_000,
            drop: 0.02,
            ..LinkParams::default()
        },
        blackouts: vec![(1, 15_000_000, 10_000_000)],
        ..VirtualSpec::default()
    };
    let r = harness::run_virtual(&setup(6, None), &spec);
    eprintln!("{}", summary(&r));
    // The pause stops the clock for a moment, so a little less game time.
    assert_clean(&r, 55 * 30);
    let resumed = r
        .relay_log
        .iter()
        .any(|l| matches!(l, LogRecord::Resumed { .. }));
    let paused = r
        .relay_log
        .iter()
        .any(|l| matches!(l, LogRecord::Paused { .. }));
    assert_eq!(
        paused,
        resumed,
        "a pause is always resumed: {}",
        summary(&r)
    );
}

/// A 70 s blackout of slot 1: pause at 10 s, then after the 60 s timeout
/// the relay closes a `Surrender` for slot 1 and resumes; slot 0 wins, and
/// slot 1 hears `Bye(TimedOut)` when it comes back.
#[test]
fn seventy_second_blackout_ends_in_surrender() {
    let spec = VirtualSpec {
        seed: 7,
        seconds: 100,
        blackouts: vec![(1, 10_000_000, 70_000_000)],
        // Slot 0 stays after its win, so the relay closes every command it
        // stamped and none counts as lost.
        bye_on_end: false,
        ..VirtualSpec::default()
    };
    let r = harness::run_virtual(&setup(7, None), &spec);
    eprintln!("{}", summary(&r));
    assert!(
        r.relay_log
            .iter()
            .any(|l| matches!(l, LogRecord::TimedOut { slot: 1, .. })),
        "{}",
        summary(&r)
    );
    assert_eq!(r.injected_surrenders, vec![1], "{}", summary(&r));
    assert_eq!(
        r.outcomes[0],
        Outcome::Won {
            winner: PlayerId(0)
        },
        "{}",
        summary(&r)
    );
    assert_eq!(r.byes[1], Some(ByeReason::TimedOut), "{}", summary(&r));
    assert_eq!(r.lost[0], 0, "{}", summary(&r));
    assert_eq!(r.order_errors, 0, "{}", summary(&r));
    assert_eq!(r.hash_mismatches, 0, "{}", summary(&r));
}

/// Slot 0's source address changes mid-match (NAT rebinding): the token
/// keeps the connection, nothing pauses, nothing is lost.
#[test]
fn address_change_keeps_the_connection() {
    let spec = VirtualSpec {
        seed: 8,
        seconds: 40,
        link: rough(),
        rebind: Some((0, 15_000_000)),
        ..VirtualSpec::default()
    };
    let r = harness::run_virtual(&setup(8, None), &spec);
    eprintln!("{}", summary(&r));
    assert_clean(&r, 38 * 30);
    assert_eq!(r.pauses, 0, "{}", summary(&r));
    assert!(
        r.relay_log
            .iter()
            .any(|l| matches!(l, LogRecord::Moved { slot: 0, .. })),
        "{}",
        summary(&r)
    );
}

/// The dev hook makes slot 1 step tick 1000 with one extra local `Stop`.
/// Both clients learn of the desync within 30 ticks, stop at the same
/// turn, and `desync-diff` of their saves names fields.
#[test]
fn injected_desync_is_detected_and_diffed() {
    let dump = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("desync-dump");
    let _ = std::fs::remove_dir_all(&dump);
    let spec = VirtualSpec {
        seed: 9,
        seconds: 60,
        link: rough(),
        inject_desync: Some((1, 1000)),
        dump_dir: Some(dump.clone()),
        ..VirtualSpec::default()
    };
    let r = harness::run_virtual(&setup(9, None), &spec);
    eprintln!("{}", summary(&r));
    let (tick, at) = r.desync[0].expect("slot 0 heard of the desync");
    assert_eq!(r.desync[1], Some((tick, at)), "{}", summary(&r));
    assert_eq!(r.relay_desync, Some(tick));
    assert!(tick > 1000 && tick <= 1030, "detected at {tick}");
    assert_eq!(r.ticks, [at, at], "both stopped at the pause turn");
    // The dumps: both clients' files and the relay log.
    for f in [
        "manifest.json",
        "paused.sav",
        "checkpoint.sav",
        "bundles.jsonl",
    ] {
        for s in ["slot0", "slot1"] {
            assert!(dump.join(s).join(f).is_file(), "missing {s}/{f}");
        }
    }
    assert!(dump.join("relay.jsonl").is_file());
    let manifest = std::fs::read_to_string(dump.join("slot1/manifest.json")).unwrap();
    assert!(
        manifest.contains(&format!("\"desync_tick\": {tick}")),
        "{manifest}"
    );
    // The confirmed checkpoint is from before the divergence (tick 900).
    assert!(manifest.contains("\"checkpoint_tick\": 900"), "{manifest}");
    let bundles = std::fs::read_to_string(dump.join("slot1/bundles.jsonl")).unwrap();
    assert!(
        bundles
            .lines()
            .next()
            .unwrap()
            .starts_with("{\"tick\":900,"),
        "{bundles}"
    );
    let read = |s: &str| std::fs::read(dump.join(s).join("paused.sav")).unwrap();
    let (a, b) = (read("slot0"), read("slot1"));
    let d = frontline_cli::desync_diff::diff_saves(&a, &b, 50).unwrap();
    let text = frontline_cli::desync_diff::render(&d);
    eprintln!("{text}");
    assert!(d.differs() && d.total > 0, "{text}");
    assert!(
        d.diffs
            .iter()
            .any(|f| f.name.starts_with("entity ") || f.name.starts_with("squad ")),
        "names a unit or squad field: {text}"
    );
    assert!(
        d.diffs
            .iter()
            .any(|f| f.name == "stats.commands_applied" || f.name == "stats.commands_rejected"),
        "names the command counter: {text}"
    );
    // The hash rings show where it went wrong: they agree before tick 1001.
    let ring =
        |i: usize| -> std::collections::BTreeMap<u64, u64> { r.rings[i].iter().copied().collect() };
    let (r0, r1) = (ring(0), ring(1));
    assert_eq!(r0.get(&1000), r1.get(&1000));
    assert_ne!(r0.get(&1001), r1.get(&1001));
}

/// The desync hook stops a vehicle (S3): `desync-diff` of the two saves
/// names vehicle fields.
#[test]
fn injected_vehicle_desync_names_vehicle_fields() {
    let dump = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("desync-dump-vehicle");
    let _ = std::fs::remove_dir_all(&dump);
    let spec = VirtualSpec {
        seed: 9,
        seconds: 90,
        link: rough(),
        vehicle_orders: true,
        inject_desync: Some((1, 1800)),
        inject_vehicle: true,
        dump_dir: Some(dump.clone()),
        ..VirtualSpec::default()
    };
    let r = harness::run_virtual(&vehicle_setup(9, None), &spec);
    eprintln!("{}", summary(&r));
    let (tick, _) = r.desync[0].expect("slot 0 heard of the desync");
    assert!(tick > 1800 && tick <= 1830, "detected at {tick}");
    let read = |s: &str| std::fs::read(dump.join(s).join("paused.sav")).unwrap();
    let d = frontline_cli::desync_diff::diff_saves(&read("slot0"), &read("slot1"), 500).unwrap();
    let text = frontline_cli::desync_diff::render(&d);
    let named: Vec<&str> = d
        .diffs
        .iter()
        .map(|f| f.name.as_str())
        .filter(|n| n.starts_with("vehicle "))
        .collect();
    eprintln!("{} differences; vehicle fields: {named:?}", d.total);
    assert!(!named.is_empty(), "names a vehicle field: {text}");
    let _ = std::fs::remove_dir_all(&dump);
}

/// The S3 soak: 20 game minutes (36,000 ticks) with vehicles, once.
/// `cargo test -p frontline-net --release -- --ignored soak_with_vehicles`.
#[test]
#[ignore]
fn soak_with_vehicles_36000_ticks() {
    let spec = VirtualSpec {
        seed: 1,
        seconds: 1200,
        link: rough(),
        garbage_per_s: 5,
        vehicle_orders: true,
        ..VirtualSpec::default()
    };
    let r = harness::run_virtual(&vehicle_setup(1, Some(100_000)), &spec);
    eprintln!(
        "vehicle soak: spawned {:?}, scripted {:?} / {:?}: {}",
        r.vehicles_spawned,
        r.script_kinds[0],
        r.script_kinds[1],
        summary(&r)
    );
    assert_clean(&r, 1198 * 30);
    assert_vehicles(&r);
}

/// 20 game minutes per seed (S2 "Long soak"), with tickets high enough
/// that nobody wins early. `cargo test -p frontline-net --release -- --ignored`.
#[test]
#[ignore]
fn soak_20_minutes_seeds_1_to_3() {
    // Three seeds that run the full 20 minutes. 100 000 tickets stop a
    // ticket win, but a side can still be wiped (no units and less
    // manpower than its cheapest card): since S6b's mortar AI, seed 3 ends
    // that way at ~17.6 min. Such a run must still be clean to its end with
    // both peers on the same outcome; the next seed (up to 8) replaces it.
    let mut full = 0;
    for seed in 1..=8 {
        let spec = VirtualSpec {
            seed,
            seconds: 1200,
            link: rough(),
            garbage_per_s: 5,
            ..VirtualSpec::default()
        };
        let r = harness::run_virtual(&setup(seed, Some(100_000)), &spec);
        eprintln!("soak seed {seed}: {}", summary(&r));
        if r.outcomes[0] != Outcome::Running {
            assert_eq!(r.outcomes[0], r.outcomes[1], "{}", summary(&r));
            assert_clean(&r, r.ticks[0].min(r.ticks[1]));
            continue;
        }
        assert_clean(&r, 1198 * 30);
        assert_eq!(
            r.outcomes,
            [Outcome::Running, Outcome::Running],
            "{}",
            summary(&r)
        );
        full += 1;
        if full == 3 {
            return;
        }
    }
    panic!("fewer than three of seeds 1-8 ran the full 20 minutes");
}
