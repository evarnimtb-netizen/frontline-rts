//! S18: team matches over the relay on a virtual clock (plan 6 "Checks"):
//! a 2v2 on Harrow Cross with two people and two AI fill, and with four
//! people. Each runs thousands of ticks; every pair of clients must agree
//! on every hash, and one client drops (a 75 s blackout: the relay times
//! it out and surrenders for it), so its troops go to its ally (S18 H1)
//! on every world still running.

use frontline_content::MatchSetup;
use frontline_net::harness::{self, VirtualReport, VirtualSpec};
use frontline_net::protocol::ByeReason;
use frontline_net::relay::LogRecord;
use frontline_sim::{EntityKind, PlayerId, World};

/// Harrow Cross, teams `[0, 0, 1, 1]`, AI in the slots `ai`.
fn setup(seed: u64, ai: &[u8]) -> MatchSetup {
    let defs = harness::load_defs(&harness::mods_dir(None).unwrap()).unwrap();
    harness::net_setup_teams(
        &defs,
        "ww2:harrow_cross",
        seed,
        &["ww2:germany", "ww2:usa", "ww2:usa", "ww2:germany"],
        &[0, 0, 1, 1],
        ai,
        Some(5000),
    )
    .unwrap()
}

fn summary(r: &VirtualReport) -> String {
    format!(
        "slots={:?} D={} ticks={:?} hash_checks={} mismatches={} first={:?} stamped={:?} \
         queued={:?} lost={:?} order_errors={} streams_differ={} pauses={} resumes={} \
         desync={:?} byes={:?} outcomes={:?} injected={:?} pings={:?}",
        r.slots,
        r.d,
        r.ticks,
        r.hash_checks,
        r.hash_mismatches,
        r.first_mismatch,
        r.stamped,
        r.queued,
        r.lost,
        r.order_errors,
        r.streams_differ,
        r.pauses,
        r.resumes,
        r.desync,
        r.byes,
        r.outcomes,
        r.injected_surrenders,
        r.pings,
    )
}

/// Living things `p` owns in `w` (the kinds H1 hands over).
fn owned_living(w: &World, p: u8) -> usize {
    let e = w.entities();
    e.kind()
        .iter()
        .zip(e.owner())
        .filter(|(k, o)| {
            o.0 == p
                && matches!(
                    k,
                    EntityKind::Soldier
                        | EntityKind::Vehicle
                        | EntityKind::Abandoned
                        | EntityKind::Downed
                )
        })
        .count()
}

/// The checks of both runs: client `drop` (a slot) times out and its ally
/// `ally` holds everything it had, on every world that kept running.
fn assert_team_run(r: &VirtualReport, drop: u8, ally: u8, min_ticks: u64) {
    let s = summary(r);
    eprintln!("{s}");
    assert_eq!(r.hash_mismatches, 0, "{s}");
    assert_eq!(r.relay_desync, None, "{s}");
    assert!(!r.streams_differ, "{s}");
    assert_eq!(r.order_errors, 0, "{s}");
    assert!(r.hash_checks > 100, "too few hash checks: {s}");
    assert!(
        r.relay_log
            .iter()
            .any(|l| matches!(l, LogRecord::TimedOut { slot, .. } if *slot == drop)),
        "{s}"
    );
    assert_eq!(r.injected_surrenders, vec![drop], "{s}");
    for (k, &slot) in r.slots.iter().enumerate() {
        if slot == drop {
            assert_eq!(r.byes[k], Some(ByeReason::TimedOut), "{s}");
            continue;
        }
        assert!(r.ticks[k] >= min_ticks, "client {k} ran too few ticks: {s}");
        let w = &r.worlds[k];
        let g = w.game().expect("a match");
        let pl = g.players();
        assert!(pl[drop as usize].surrendered, "{s}");
        assert_eq!(pl[drop as usize].handed_to, Some(PlayerId(ally)), "{s}");
        assert_eq!(pl[drop as usize].manpower, 0, "{s}");
        assert!(
            !pl[ally as usize].handed.is_empty(),
            "the ally got squads: {s}"
        );
        assert_eq!(
            owned_living(w, drop),
            0,
            "slot {drop} still owns troops: {s}"
        );
        assert!(
            g.squads().values().all(|q| q.owner != PlayerId(drop)),
            "{s}"
        );
    }
}

/// Two people (slots 0 and 2) and two AI fill (slots 1 and 3, set by the
/// host in the lobby). Slot 2 drops; its troops go to the AI in slot 3.
#[test]
fn two_people_and_two_ai_fill_with_a_drop() {
    let s = setup(11, &[1, 3]);
    let spec = VirtualSpec {
        seed: 11,
        seconds: 300,
        blackouts: vec![(2, 150_000_000, 75_000_000)],
        bye_on_end: false,
        team: Some(vec![1, 3]),
        ..VirtualSpec::default()
    };
    let r = harness::run_virtual(&s, &spec);
    assert_eq!(r.slots, vec![0, 2], "{}", summary(&r));
    assert!(
        r.relay_log
            .iter()
            .any(|l| matches!(l, LogRecord::Started { players: 4, ai, .. } if *ai == vec![1, 3])),
        "{}",
        summary(&r)
    );
    assert_team_run(&r, 2, 3, 6000);
}

/// Four people, slot 0 the host. Slot 1 drops; its troops go to slot 0
/// and the three other clients agree on every hash before and after.
/// Slot 0's map ping reaches its ally (slot 1) only.
#[test]
fn four_people_with_a_drop_and_a_ping() {
    let s = setup(12, &[]);
    let spec = VirtualSpec {
        seed: 12,
        seconds: 240,
        // Slot 3 drops for 10 s and comes back (the relay waits for it,
        // then resumes); slot 1 drops for good.
        blackouts: vec![(3, 30_000_000, 10_000_000), (1, 60_000_000, 75_000_000)],
        bye_on_end: false,
        team: Some(Vec::new()),
        map_pings: vec![(0, 20_000_000), (2, 20_000_000)],
        ..VirtualSpec::default()
    };
    let r = harness::run_virtual(&s, &spec);
    assert_eq!(r.slots, vec![0, 1, 2, 3], "{}", summary(&r));
    let s1 = summary(&r);
    assert_eq!(r.pings[1], vec![(0, 100, 200)], "{s1}");
    assert_eq!(r.pings[3], vec![(2, 102, 200)], "{s1}");
    assert!(r.pings[0].is_empty(), "{s1}");
    assert!(r.pings[2].is_empty(), "{s1}");
    assert!(
        r.relay_log.iter().any(|l| matches!(
            l,
            LogRecord::Paused {
                cause: frontline_net::relay::PauseKind::Waiting(3),
                ..
            }
        )),
        "the relay waited for slot 3: {s1}"
    );
    assert!(r.resumes >= 2, "{s1}");
    assert_team_run(&r, 1, 0, 4000);
}

/// A forced desync in a four-player match (slot 2 runs one extra local
/// order): every client stops at the pause turn and each writes its dump,
/// whose manifest lists the hash of every slot.
#[test]
fn a_four_player_desync_is_dumped_with_every_hash() {
    let dump = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("desync-dump-4");
    let _ = std::fs::remove_dir_all(&dump);
    let s = setup(13, &[]);
    let spec = VirtualSpec {
        seed: 13,
        seconds: 60,
        inject_desync: Some((2, 1000)),
        dump_dir: Some(dump.clone()),
        team: Some(Vec::new()),
        ..VirtualSpec::default()
    };
    let r = harness::run_virtual(&s, &spec);
    let s1 = summary(&r);
    let tick = r.relay_desync.expect("the relay saw the desync");
    assert!(tick > 1000 && tick <= 1030, "detected at {tick}: {s1}");
    assert!(
        r.desync.iter().all(|d| d.is_some_and(|(t, _)| t == tick)),
        "{s1}"
    );
    for slot in 0..4 {
        let m = std::fs::read_to_string(dump.join(format!("slot{slot}/manifest.json")))
            .unwrap_or_else(|e| panic!("slot {slot}: {e}"));
        let v: serde_json::Value = serde_json::from_str(&m).unwrap();
        let hashes = v["hashes"].as_array().unwrap();
        assert_eq!(hashes.len(), 4, "{m}");
        assert!(hashes.iter().filter(|h| h.is_string()).count() >= 2, "{m}");
    }
    assert!(dump.join("relay.jsonl").is_file());
}
