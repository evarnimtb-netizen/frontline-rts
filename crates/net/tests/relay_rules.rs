//! Relay rules from the S2 netcode review, on a virtual clock with no sim:
//! one relay and two clients (S18: up to four) over a zero-latency link
//! that a test can cut one way or both. Each test is a hostile or broken
//! case that once left a match stuck or slow, or an S18 lobby rule.

use std::net::SocketAddr;

use frontline_net::client::{ClientConfig, ClientPhase, NetClient, NetEvent};
use frontline_net::protocol::{AiPick, ByeReason, LobbySet, PauseCause, RejectReason};
use frontline_net::relay::{LogRecord, Phase, Relay, RelayConfig, SlotState};
use frontline_net::rng::TokenSource;
use frontline_sim::CommandKind;

const RELAY: &str = "10.0.0.1:47015";

struct Net {
    relay: Relay,
    clients: Vec<NetClient>,
    addrs: Vec<SocketAddr>,
    now: u64,
    events: Vec<Vec<NetEvent>>,
    /// Cut relay → client, and client → relay.
    down_cut: Vec<bool>,
    up_cut: Vec<bool>,
    /// Clients that do not poll yet (S18: join later).
    asleep: Vec<bool>,
    log: Vec<LogRecord>,
}

impl Net {
    fn new<const N: usize>(names: [&str; N]) -> Net {
        let relay_addr: SocketAddr = RELAY.parse().unwrap();
        let client = |i: usize| {
            NetClient::new(ClientConfig {
                relay: relay_addr,
                name: names[i].into(),
                build_id: "test".into(),
                content_hash: 7,
                nonce: 100 + i as u64,
            })
        };
        Net {
            relay: Relay::new(
                RelayConfig {
                    countdown_us: 200_000,
                    ..RelayConfig::default()
                },
                TokenSource::seeded(1),
            ),
            clients: (0..N).map(client).collect(),
            addrs: (0..N)
                .map(|i| format!("10.0.0.{}:5000", i + 2).parse().unwrap())
                .collect(),
            now: 1_000_000,
            events: vec![Vec::new(); N],
            down_cut: vec![false; N],
            up_cut: vec![false; N],
            asleep: vec![false; N],
            log: Vec::new(),
        }
    }

    /// Runs `us` of virtual time in 1 ms steps.
    fn run(&mut self, us: u64) {
        let end = self.now + us;
        while self.now < end {
            self.now += 1000;
            let now = self.now;
            self.relay.poll(now);
            let out: Vec<_> = self.relay.drain_out().collect();
            for (to, bytes) in out {
                if let Some(i) = self.addrs.iter().position(|a| *a == to)
                    && !self.down_cut[i]
                {
                    self.clients[i].on_datagram(RELAY.parse().unwrap(), &bytes, now);
                }
            }
            self.log.extend(self.relay.drain_log());
            for i in 0..self.clients.len() {
                if self.asleep[i] {
                    continue;
                }
                self.clients[i].poll(now);
                self.events[i].extend(self.clients[i].drain_events());
                let out: Vec<_> = self.clients[i].drain_out().collect();
                if !self.up_cut[i] {
                    for (_, bytes) in out {
                        self.relay.on_datagram(self.addrs[i], &bytes, now);
                    }
                }
            }
        }
    }

    /// Both join, pick factions, Ready, and the match starts.
    fn start(&mut self) {
        self.run(50_000);
        for (i, f) in ["ger", "usa"].iter().enumerate() {
            self.clients[i].set_lobby(LobbySet {
                faction: (*f).into(),
                seed: (i == 0).then_some(3),
                map: None,
                d: None,
                ..LobbySet::default()
            });
        }
        self.run(100_000);
        for c in &mut self.clients {
            c.ready(42);
        }
        self.run(600_000);
        assert_eq!(self.relay.phase(), Phase::Running);
    }

    fn bundled(&self, i: usize, pred: impl Fn(u64, &frontline_sim::Command) -> bool) -> bool {
        self.events[i].iter().any(|e| match e {
            NetEvent::Bundle { tick, cmds } => cmds.iter().any(|c| pred(*tick, c)),
            _ => false,
        })
    }
}

/// The host quits mid-match and stops its relay at once: the guest still
/// gets the turn with the host's `Surrender`, before `Bye(Shutdown)`.
#[test]
fn host_quit_then_shutdown_delivers_the_surrender() {
    let mut n = Net::new(["Host", "Guest"]);
    n.start();
    n.run(1_000_000);
    n.clients[0].bye(ByeReason::Quit);
    n.run(5_000);
    n.relay.shutdown(n.now);
    n.run(600_000);
    let surrender = n.bundled(1, |_, c| {
        c.player.0 == 0 && c.kind == CommandKind::Surrender
    });
    assert!(surrender, "the guest never got the host's surrender");
    let bye_after_bundles = n.events[1]
        .iter()
        .position(|e| matches!(e, NetEvent::Bye(ByeReason::Shutdown)))
        .expect("the guest heard the goodbye");
    let last_bundle = n.events[1]
        .iter()
        .rposition(|e| matches!(e, NetEvent::Bundle { .. }))
        .unwrap();
    assert!(last_bundle < bye_after_bundles);
    assert!(n.relay.is_finished(n.now));
}

/// A slot whose uplink works but whose downlink is dead after a blackout
/// is heard again, but never catches up to the pause turn: the waiting
/// pause still times out after 60 s instead of lasting forever.
#[test]
fn waiting_pause_times_out_on_a_one_way_link() {
    let mut n = Net::new(["A", "B"]);
    n.start();
    n.run(1_000_000);
    n.down_cut[1] = true;
    n.up_cut[1] = true;
    n.run(12_000_000);
    assert!(matches!(
        n.relay.paused(),
        Some((_, frontline_net::relay::PauseKind::Waiting(1)))
    ));
    // Its input comes through again; bundles still do not reach it.
    n.up_cut[1] = false;
    n.run(65_000_000);
    assert_eq!(n.relay.slot_state(1), Some(SlotState::TimedOut));
    assert_eq!(n.relay.paused(), None);
    assert!(n.bundled(0, |_, c| c.player.0 == 1
        && c.kind == CommandKind::Surrender));
}

/// A player flooding lobby changes while the other's downlink drops for a
/// moment: lobby broadcasts are coalesced, so the other's control stream
/// never backs up, and it still sees `Start`.
#[test]
fn lobby_flood_does_not_jam_the_other_stream() {
    let mut n = Net::new(["A", "B"]);
    n.run(50_000);
    n.clients[0].set_lobby(LobbySet {
        faction: "ger".into(),
        seed: Some(3),
        map: None,
        d: None,
        ..LobbySet::default()
    });
    n.down_cut[0] = true;
    for k in 0..2000 {
        n.clients[1].set_lobby(LobbySet {
            faction: if k % 2 == 0 { "usa" } else { "ussr" }.into(),
            seed: None,
            map: None,
            d: None,
            ..LobbySet::default()
        });
        n.run(1000);
    }
    n.down_cut[0] = false;
    n.clients[1].set_lobby(LobbySet {
        faction: "usa".into(),
        seed: None,
        map: None,
        d: None,
        ..LobbySet::default()
    });
    // S18: a Ready names the lobby revision it was made for, so A readies
    // once its control stream (resent 4 messages per 100 ms after the cut)
    // caught up with the last change.
    let rev = |n: &Net, i: usize| {
        n.events[i].iter().rev().find_map(|e| match e {
            NetEvent::Lobby(l) => Some(l.rev),
            _ => None,
        })
    };
    n.run(300_000);
    for _ in 0..100 {
        if rev(&n, 0) == rev(&n, 1) {
            break;
        }
        n.run(100_000);
    }
    for c in &mut n.clients {
        c.ready(42);
    }
    n.run(600_000);
    assert_eq!(n.relay.phase(), Phase::Running);
    assert!(n.events[0].iter().any(|e| matches!(e, NetEvent::Start(_))));
    assert_eq!(n.clients[0].phase(), ClientPhase::Running);
    let lobbies = n.events[0]
        .iter()
        .filter(|e| matches!(e, NetEvent::Lobby(_)))
        .count();
    assert!(lobbies < 200, "{lobbies} lobby updates for 2 s of flood");
}

/// Pause and resume requests faster than one per 250 ms are ignored, so
/// the other player's stream gets at most a few `Paused`/`Resume` a second.
#[test]
fn pause_toggling_is_rate_limited() {
    let mut n = Net::new(["A", "B"]);
    n.start();
    n.run(500_000);
    for _ in 0..200 {
        n.clients[0].request_pause();
        n.clients[0].request_resume();
        n.run(5_000);
    }
    n.run(500_000);
    let pauses = n.events[1]
        .iter()
        .filter(|e| {
            matches!(
                e,
                NetEvent::Paused {
                    cause: PauseCause::Player(0),
                    ..
                }
            )
        })
        .count();
    assert!((1..=5).contains(&pauses), "{pauses} pauses in 1 s");
}

/// Two players with the same name: the second gets a suffix, so the
/// overlays and end screen can tell them apart.
#[test]
fn duplicate_names_are_suffixed() {
    let mut n = Net::new(["Player", "Player"]);
    n.run(200_000);
    let names: Vec<String> = n.events[0]
        .iter()
        .rev()
        .find_map(|e| match e {
            NetEvent::Lobby(l) => Some(l.slots.iter().flatten().map(|s| s.name.clone()).collect()),
            _ => None,
        })
        .unwrap();
    assert_eq!(names, ["Player", "Player (2)"]);
}

/// After a blackout the client stamped nothing while it heard no relay,
/// so its first orders after the resume run about D ticks after they are
/// sent, not a second later.
#[test]
fn orders_after_a_long_blackout_are_not_far_ahead() {
    let mut n = Net::new(["A", "B"]);
    n.start();
    n.run(1_000_000);
    n.down_cut[1] = true;
    n.up_cut[1] = true;
    // Orders keep coming while the link is dead.
    for _ in 0..40 {
        n.clients[1].send_command(CommandKind::StopUnits { units: Vec::new() });
        n.run(1_000_000);
    }
    n.down_cut[1] = false;
    n.up_cut[1] = false;
    n.run(3_000_000);
    assert_eq!(n.relay.paused(), None, "resumed");
    let before = n.events[1].len();
    let sent_at_tick = n.relay.next_turn();
    n.clients[1].send_command(CommandKind::Regroup {
        squad: frontline_sim::SquadId(1),
    });
    n.run(2_000_000);
    let ran = n.events[1][before..].iter().find_map(|e| match e {
        NetEvent::Bundle { tick, cmds }
            if cmds
                .iter()
                .any(|c| matches!(c.kind, CommandKind::Regroup { .. })) =>
        {
            Some(*tick)
        }
        _ => None,
    });
    let ran = ran.expect("the order ran");
    assert!(
        ran <= sent_at_tick + 2 + 6,
        "order sent at turn {sent_at_tick} ran at {ran}"
    );
    assert!(n.log.iter().any(|l| matches!(l, LogRecord::Resumed { .. })));
}

/// The last lobby state client `i` heard.
fn last_lobby(n: &Net, i: usize) -> frontline_net::protocol::LobbyState {
    n.events[i]
        .iter()
        .rev()
        .find_map(|e| match e {
            NetEvent::Lobby(l) => Some(l.clone()),
            _ => None,
        })
        .expect("a lobby state")
}

/// S18: a third player is refused while the lobby is a 1v1 (the default
/// size, so a 1v1 relay behaves as before); once the host opens four
/// slots and fills slot 3 with AI, he joins slot 2, can move to an open
/// slot but not to the host's or the AI's.
#[test]
fn lobby_size_ai_fill_and_slot_moves() {
    let mut n = Net::new(["Host", "B", "C", "D"]);
    n.asleep[3] = true;
    n.run(50_000);
    assert!(
        n.events[2]
            .iter()
            .any(|e| matches!(e, NetEvent::Rejected(RejectReason::Full))),
        "{:?}",
        n.events[2]
    );
    n.clients[0].set_lobby(LobbySet {
        faction: "ger".into(),
        players: Some(4),
        ai: Some(vec![AiPick {
            slot: 3,
            faction: "usa".into(),
            team: Some(1),
            difficulty: 2,
        }]),
        ..LobbySet::default()
    });
    n.run(100_000);
    n.asleep[3] = false;
    n.run(100_000);
    assert!(n.events[3].contains(&NetEvent::Welcome { slot: 2 }));
    let l = last_lobby(&n, 0);
    assert_eq!(l.players, 4);
    assert_eq!(l.slots[3].as_ref().and_then(|s| s.ai), Some(2));
    // To the AI's slot, or the host's: refused. Slot 1 is B's.
    n.clients[3].take_slot(3);
    n.clients[3].take_slot(0);
    n.clients[3].take_slot(1);
    n.run(100_000);
    assert!(
        !n.events[3]
            .iter()
            .any(|e| matches!(e, NetEvent::Slot { .. }))
    );
    // B leaves; D takes slot 1.
    n.clients[1].bye(ByeReason::Quit);
    n.run(100_000);
    n.clients[3].take_slot(1);
    n.run(100_000);
    assert!(n.events[3].contains(&NetEvent::Slot { slot: 1 }));
    assert_eq!(n.clients[3].slot(), Some(1));
    let l = last_lobby(&n, 0);
    assert_eq!(l.slots[1].as_ref().map(|s| s.name.as_str()), Some("D"));
    assert!(l.slots[2].is_none());
    // The host shrinks the match to two: the AI slot past it goes.
    n.clients[0].set_lobby(LobbySet {
        faction: "ger".into(),
        players: Some(2),
        ..LobbySet::default()
    });
    n.run(100_000);
    let l = last_lobby(&n, 0);
    assert_eq!(l.players, 2);
    assert!(l.slots[3].is_none());
}

/// S18: a Ready made for an older lobby revision does not count: B's
/// Ready is held on the way while the host changes the seed, so B is not
/// ready and the match does not start until B readies again.
#[test]
fn a_ready_for_an_older_lobby_is_dropped() {
    let mut n = Net::new(["A", "B"]);
    n.run(50_000);
    for (i, f) in ["ger", "usa"].iter().enumerate() {
        n.clients[i].set_lobby(LobbySet {
            faction: (*f).into(),
            seed: (i == 0).then_some(3),
            ..LobbySet::default()
        });
    }
    n.run(100_000);
    n.up_cut[1] = true;
    n.clients[1].ready(42);
    n.run(10_000);
    n.clients[0].set_lobby(LobbySet {
        faction: "ger".into(),
        seed: Some(4),
        ..LobbySet::default()
    });
    n.run(50_000);
    n.up_cut[1] = false;
    n.clients[0].ready(42);
    n.run(300_000);
    assert_eq!(n.relay.phase(), Phase::Lobby);
    let l = last_lobby(&n, 0);
    assert_eq!(l.seed, 4);
    assert!(!l.slots[1].as_ref().unwrap().ready);
    n.clients[1].ready(42);
    n.run(300_000);
    assert_eq!(n.relay.phase(), Phase::Running);
}

/// S18 H5: four players on teams 0 0 1 1. A map ping goes to the
/// sender's connected allies only, at most one per `ping_gap_us`.
#[test]
fn map_pings_reach_allies_only_and_are_spaced() {
    let mut n = Net::new(["A", "B", "C", "D"]);
    n.asleep[1..].iter_mut().for_each(|a| *a = true);
    n.run(50_000);
    n.clients[0].set_lobby(LobbySet {
        faction: "ger".into(),
        team: Some(0),
        players: Some(4),
        ..LobbySet::default()
    });
    n.run(100_000);
    n.asleep.iter_mut().for_each(|a| *a = false);
    n.run(100_000);
    for (i, (f, t)) in [("usa", 0), ("ger", 1), ("usa", 1)].iter().enumerate() {
        n.clients[i + 1].set_lobby(LobbySet {
            faction: (*f).into(),
            team: Some(*t),
            ..LobbySet::default()
        });
    }
    n.run(300_000);
    for c in &mut n.clients {
        c.ready(42);
    }
    n.run(600_000);
    assert_eq!(n.relay.phase(), Phase::Running);
    let start = n.events[2]
        .iter()
        .find_map(|e| match e {
            NetEvent::Start(s) => Some(s.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(start.players.len(), 4);
    assert_eq!(
        start.players.iter().map(|p| p.team).collect::<Vec<_>>(),
        [Some(0), Some(0), Some(1), Some(1)]
    );
    n.clients[0].map_ping(10, 20);
    n.run(10_000);
    n.clients[0].map_ping(11, 21);
    n.run(100_000);
    let pings = |n: &Net, i: usize| -> Vec<(u8, i32, i32)> {
        n.events[i]
            .iter()
            .filter_map(|e| match e {
                NetEvent::MapPing { slot, x, y } => Some((*slot, *x, *y)),
                _ => None,
            })
            .collect()
    };
    assert_eq!(pings(&n, 1), [(0, 10, 20)]);
    assert!(pings(&n, 0).is_empty());
    assert!(pings(&n, 2).is_empty() && pings(&n, 3).is_empty());
    n.run(500_000);
    n.clients[0].map_ping(12, 22);
    n.run(100_000);
    assert_eq!(pings(&n, 1), [(0, 10, 20), (0, 12, 22)]);
    // Review: a ping off the map (the cursor past its edge) is clamped
    // into the wire range; unclamped it failed the relay's decode and
    // jammed slot 0's reliable control stream (no later ping, hash or
    // bye got through).
    n.run(500_000);
    n.clients[0].map_ping(-40, 70_000);
    n.run(600_000);
    n.clients[0].map_ping(13, 23);
    n.run(100_000);
    assert_eq!(
        pings(&n, 1),
        [(0, 10, 20), (0, 12, 22), (0, 0, 65_535), (0, 13, 23)]
    );
}

/// Review: a `Ready` carries the lobby revision its hash was made from
/// (`ready_for`), not the newest one the client thread heard: a hash made
/// from revision `r` is dropped once the lobby moved on, even if the
/// client already holds the newer state.
#[test]
fn a_ready_names_the_revision_its_hash_was_made_from() {
    let mut n = Net::new(["A", "B"]);
    n.run(50_000);
    for (i, f) in ["ger", "usa"].iter().enumerate() {
        n.clients[i].set_lobby(LobbySet {
            faction: (*f).into(),
            seed: (i == 0).then_some(3),
            ..LobbySet::default()
        });
    }
    n.run(100_000);
    let old = last_lobby(&n, 1).rev;
    n.clients[0].set_lobby(LobbySet {
        faction: "ger".into(),
        seed: Some(4),
        ..LobbySet::default()
    });
    n.run(100_000);
    let new = last_lobby(&n, 1).rev;
    assert_ne!(old, new);
    n.clients[0].ready(42);
    n.clients[1].ready_for(42, old);
    n.run(300_000);
    assert_eq!(n.relay.phase(), Phase::Lobby);
    assert!(!last_lobby(&n, 0).slots[1].as_ref().unwrap().ready);
    n.clients[1].ready_for(42, new);
    n.run(300_000);
    assert_eq!(n.relay.phase(), Phase::Running);
}

/// Review: a client of the S17 protocol (version 17) is told the relay's
/// version in a `Reject`, and nothing else of it is read.
#[test]
fn an_older_protocol_is_rejected_with_the_relay_version() {
    use frontline_net::protocol::{
        Hello, PROTOCOL_VERSION, Packet, decode, encode, reject_version,
    };
    let mut relay = Relay::new(RelayConfig::default(), TokenSource::seeded(1));
    let from: SocketAddr = "10.0.0.9:5000".parse().unwrap();
    let mut bytes = Vec::new();
    encode(
        0,
        &Packet::Hello(Hello {
            nonce: 1,
            build_id: "test".into(),
            content_hash: 7,
            name: "old".into(),
        }),
        &mut bytes,
    )
    .unwrap();
    bytes[4..6].copy_from_slice(&17u16.to_le_bytes());
    relay.on_datagram(from, &bytes, 1_000_000);
    let out: Vec<_> = relay.drain_out().collect();
    assert_eq!(out.len(), 1);
    assert_eq!(reject_version(&out[0].1), Some(PROTOCOL_VERSION));
    match decode(&out[0].1) {
        Ok((_, Packet::Reject(r))) => assert_eq!(r.reason, RejectReason::Version),
        other => panic!("not a reject: {other:?}"),
    }
    // No slot was taken: the lobby is still empty.
    assert!(relay.status(1_000_000).slots.iter().all(|s| s.is_none()));
}
