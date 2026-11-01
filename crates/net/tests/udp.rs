//! The real-socket path: the relay thread and two client net threads over
//! UDP loopback in real time (short, so `cargo test` stays quick). Sockets
//! bind loopback only, so no macOS firewall prompt appears.

use std::time::{Duration, Instant};

use frontline_net::client::{ClientConfig, NetEvent};
use frontline_net::client_io::NetHandle;
use frontline_net::harness::{self, VirtualSpec};
use frontline_net::relay::RelayConfig;
use frontline_net::server::RelayServer;
use frontline_net::transport::LinkParams;

#[test]
fn ipv4_loopback_match_agrees() {
    let defs = harness::load_defs(&harness::mods_dir(None).unwrap()).unwrap();
    let setup = harness::net_setup(&defs, 11, false, None).unwrap();
    let spec = VirtualSpec {
        seed: 11,
        seconds: 6,
        quiet_tail_us: 1_500_000,
        link: LinkParams {
            delay_us: 3_000,
            jitter_us: 5_000,
            drop: 0.02,
            ..LinkParams::default()
        },
        relay: RelayConfig {
            countdown_us: 500_000,
            ..RelayConfig::default()
        },
        ..VirtualSpec::default()
    };
    let r = harness::run_udp(&setup, &spec).unwrap();
    eprintln!(
        "D={} ticks={:?} checks={} mismatches={} stamped={:?} lost={:?} restamps={}",
        r.d, r.ticks, r.hash_checks, r.hash_mismatches, r.stamped, r.lost, r.relay_restamps
    );
    assert!(r.ticks.iter().all(|&t| t >= 150), "{:?}", r.ticks);
    assert!(r.hash_checks >= 4);
    assert_eq!(r.hash_mismatches, 0);
    assert_eq!(r.order_errors, 0);
    assert_eq!(r.lost, [0, 0]);
}

#[test]
fn ipv6_loopback_handshake() {
    let mut server = RelayServer::spawn(RelayConfig::default(), "[::1]:0".parse().unwrap(), None)
        .expect("IPv6 loopback is available");
    let mut client = NetHandle::spawn(ClientConfig {
        relay: server.local_addr(),
        name: "six".into(),
        build_id: frontline_net::build_id(),
        content_hash: 0,
        nonce: 6,
    })
    .unwrap();
    let t0 = Instant::now();
    let mut evs = Vec::new();
    let mut welcomed = false;
    while t0.elapsed() < Duration::from_secs(5) && !welcomed {
        std::thread::sleep(Duration::from_millis(5));
        client.drain_events(&mut evs);
        welcomed = evs
            .iter()
            .any(|e| matches!(e, NetEvent::Welcome { slot: 0 }));
    }
    assert!(welcomed, "no Welcome over ::1: {evs:?}");
    // The relay measures the client's RTT from its pings.
    std::thread::sleep(Duration::from_millis(1200));
    let s = server.status();
    assert_eq!(s.slots[0].as_ref().map(|s| s.name.as_str()), Some("six"));
    client.stop();
    server.stop();
}

/// With the match events taken, bundles reach only the sim thread's
/// receiver, and `Start` reaches both.
#[test]
fn match_events_split_off() {
    use frontline_net::client_io::ToNet;
    use frontline_net::protocol::LobbySet;
    let cfg = RelayConfig {
        countdown_us: 200_000,
        ..RelayConfig::default()
    };
    let mut server = RelayServer::spawn(cfg, "127.0.0.1:0".parse().unwrap(), None).unwrap();
    let mut hs: Vec<NetHandle> = (0..2)
        .map(|i| {
            NetHandle::spawn(ClientConfig {
                relay: server.local_addr(),
                name: format!("p{i}"),
                build_id: frontline_net::build_id(),
                content_hash: 0,
                nonce: 50 + i,
            })
            .unwrap()
        })
        .collect();
    let sim_rx = hs[0].take_match_events().unwrap();
    let factions = ["ww2:germany", "ww2:usa"];
    let mut ui = [Vec::new(), Vec::new()];
    let t0 = Instant::now();
    let mut chosen = [false; 2];
    let mut readied = false;
    let mut sim_bundles = 0;
    while t0.elapsed() < Duration::from_secs(8) && sim_bundles < 30 {
        std::thread::sleep(Duration::from_millis(5));
        for (i, h) in hs.iter().enumerate() {
            let before = ui[i].len();
            h.drain_events(&mut ui[i]);
            for ev in &ui[i][before..] {
                if let NetEvent::Welcome { slot } = *ev {
                    h.send(ToNet::Lobby(LobbySet {
                        faction: factions[slot as usize].into(),
                        ..LobbySet::default()
                    }));
                    chosen[i] = true;
                }
            }
        }
        if chosen == [true, true] && !readied && t0.elapsed() > Duration::from_millis(800) {
            readied = true;
            for h in &hs {
                h.send(ToNet::Ready(7));
            }
        }
        sim_bundles += sim_rx
            .try_iter()
            .filter(|e| matches!(e, NetEvent::Bundle { .. }))
            .count();
    }
    assert!(sim_bundles >= 30, "sim thread got {sim_bundles} bundles");
    let has = |v: &[NetEvent], f: fn(&NetEvent) -> bool| v.iter().any(f);
    assert!(!has(&ui[0], |e| matches!(e, NetEvent::Bundle { .. })));
    assert!(has(&ui[0], |e| matches!(e, NetEvent::Start(_))));
    assert!(
        has(&ui[1], |e| matches!(e, NetEvent::Bundle { .. })),
        "unsplit handle keeps bundles"
    );
    for h in &mut hs {
        h.stop();
    }
    server.stop();
}
