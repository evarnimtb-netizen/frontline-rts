//! Relay rules from the S2 netcode review, on a virtual clock with no sim:
//! one relay and two clients over a zero-latency link that a test can cut
//! one way or both. Each test is a hostile or broken case that once left
//! a match stuck or slow.

use std::net::SocketAddr;

use frontline_net::client::{ClientConfig, ClientPhase, NetClient, NetEvent};
use frontline_net::protocol::{ByeReason, LobbySet, PauseCause};
use frontline_net::relay::{LogRecord, Phase, Relay, RelayConfig, SlotState};
use frontline_net::rng::TokenSource;
use frontline_sim::CommandKind;

const RELAY: &str = "10.0.0.1:47015";

struct Net {
    relay: Relay,
    clients: Vec<NetClient>,
    addrs: [SocketAddr; 2],
    now: u64,
    events: [Vec<NetEvent>; 2],
    /// Cut relay → client, and client → relay.
    down_cut: [bool; 2],
    up_cut: [bool; 2],
    log: Vec<LogRecord>,
}

impl Net {
    fn new(names: [&str; 2]) -> Net {
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
            clients: vec![client(0), client(1)],
            addrs: [
                "10.0.0.2:5000".parse().unwrap(),
                "10.0.0.3:5000".parse().unwrap(),
            ],
            now: 1_000_000,
            events: [Vec::new(), Vec::new()],
            down_cut: [false; 2],
            up_cut: [false; 2],
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
            for i in 0..2 {
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
    });
    n.down_cut[0] = true;
    for k in 0..2000 {
        n.clients[1].set_lobby(LobbySet {
            faction: if k % 2 == 0 { "usa" } else { "ussr" }.into(),
            seed: None,
            map: None,
            d: None,
        });
        n.run(1000);
    }
    n.down_cut[0] = false;
    n.clients[1].set_lobby(LobbySet {
        faction: "usa".into(),
        seed: None,
        map: None,
        d: None,
    });
    n.run(300_000);
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
