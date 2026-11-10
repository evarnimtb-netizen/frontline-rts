//! Garbage in, no panic out (S2 "Security": all network input is
//! untrusted). Random bytes, valid headers with random bodies, and
//! genuine datagrams with flipped bits, cut short or extended (including
//! ones that carry real tokens) go to a relay in the lobby and in a running
//! match and to both clients.

use std::net::SocketAddr;

use frontline_net::client::{ClientConfig, NetClient, NetEvent};
use frontline_net::protocol::{self, LobbySet, MAX_DATAGRAM};
use frontline_net::relay::{Relay, RelayConfig};
use frontline_net::rng::{SplitMix, TokenSource};
use frontline_sim::{CommandKind, FxVec2, game::SquadId};

struct Rig {
    relay: Relay,
    clients: Vec<NetClient>,
    addrs: Vec<SocketAddr>,
    relay_addr: SocketAddr,
    now: u64,
    /// Genuine datagrams seen, both directions: (to relay?, bytes).
    seen: Vec<(bool, Vec<u8>)>,
    /// Commands the clients got in bundles: (client, command).
    bundled: Vec<(usize, frontline_sim::Command)>,
}

impl Rig {
    fn new() -> Rig {
        let relay_addr: SocketAddr = "10.0.0.1:47015".parse().unwrap();
        let addrs: Vec<SocketAddr> = vec![
            "10.0.0.2:5000".parse().unwrap(),
            "[fd00::3]:5000".parse().unwrap(),
        ];
        let clients = (0..2)
            .map(|i| {
                NetClient::new(ClientConfig {
                    relay: relay_addr,
                    name: format!("p{i}"),
                    build_id: "b".into(),
                    content_hash: 1,
                    nonce: 100 + i as u64,
                })
            })
            .collect();
        Rig {
            relay: Relay::new(RelayConfig::default(), TokenSource::seeded(3)),
            clients,
            addrs,
            relay_addr,
            now: 0,
            seen: Vec::new(),
            bundled: Vec::new(),
        }
    }

    /// One ms: poll everything and deliver every datagram (no loss).
    fn step(&mut self) {
        self.now += 1000;
        let now = self.now;
        self.relay.poll(now);
        let down: Vec<_> = self.relay.drain_out().collect();
        for (to, bytes) in down {
            if let Some(i) = self.addrs.iter().position(|a| *a == to) {
                self.clients[i].on_datagram(self.relay_addr, &bytes, now);
            }
            if self.seen.len() < 4000 {
                self.seen.push((false, bytes));
            }
        }
        for i in 0..2 {
            self.clients[i].poll(now);
            let up: Vec<_> = self.clients[i].drain_out().collect();
            for (_, bytes) in up {
                self.relay.on_datagram(self.addrs[i], &bytes, now);
                if self.seen.len() < 4000 {
                    self.seen.push((true, bytes));
                }
            }
            let evs: Vec<_> = self.clients[i].drain_events().collect();
            for ev in evs {
                if let NetEvent::Bundle { cmds, .. } = &ev {
                    self.bundled.extend(cmds.iter().map(|c| (i, c.clone())));
                }
                if let NetEvent::Welcome { slot } = ev {
                    let faction = if slot == 0 { "ww2:germany" } else { "ww2:usa" };
                    self.clients[i].set_lobby(LobbySet {
                        faction: faction.into(),
                        ..LobbySet::default()
                    });
                }
            }
        }
        // Ready 1 s in, once both chose (any lobby change clears Ready).
        if now == 1_000_000 {
            for c in &mut self.clients {
                c.ready(42);
            }
        }
    }

    /// Feeds one junk datagram everywhere.
    fn junk(&mut self, r: &mut SplitMix) {
        let bytes = junk(r, &self.seen);
        assert!(protocol::decode(&bytes).is_ok() || bytes.len() <= 2000);
        let from = match r.below(3) {
            0 => self.addrs[0],
            1 => self.addrs[1],
            _ => "203.0.113.9:1234".parse().unwrap(),
        };
        let now = self.now;
        self.relay.on_datagram(from, &bytes, now);
        for c in &mut self.clients {
            c.on_datagram(self.relay_addr, &bytes, now);
        }
    }
}

fn junk(r: &mut SplitMix, seen: &[(bool, Vec<u8>)]) -> Vec<u8> {
    let random =
        |r: &mut SplitMix, n: usize| -> Vec<u8> { (0..n).map(|_| r.next_u64() as u8).collect() };
    match r.below(6) {
        0 => {
            let n = r.below(1500) as usize;
            random(r, n)
        }
        1 => {
            // Valid header, random kind, random token, random body.
            let mut v = b"FLNT".to_vec();
            v.extend_from_slice(&protocol::PROTOCOL_VERSION.to_le_bytes());
            v.push(r.below(16) as u8);
            v.extend_from_slice(&r.next_u64().to_le_bytes());
            let n = r.below(1300) as usize;
            v.extend(random(r, n));
            v
        }
        _ if seen.is_empty() => random(r, 20),
        2 => {
            // A genuine datagram (real token) with a few flipped bits.
            let mut v = seen[r.below(seen.len() as u64) as usize].1.clone();
            for _ in 0..1 + r.below(4) {
                if v.is_empty() {
                    break;
                }
                let i = r.below(v.len() as u64) as usize;
                v[i] ^= 1 << r.below(8);
            }
            v
        }
        3 => {
            let mut v = seen[r.below(seen.len() as u64) as usize].1.clone();
            let n = r.below(v.len() as u64 + 1) as usize;
            v.truncate(n);
            v
        }
        4 => {
            // Genuine header, random body.
            let mut v = seen[r.below(seen.len() as u64) as usize].1.clone();
            v.truncate(protocol::HEADER_LEN.min(v.len()));
            let n = r.below(MAX_DATAGRAM as u64 + 200) as usize;
            v.extend(random(r, n));
            v
        }
        _ => {
            let mut v = seen[r.below(seen.len() as u64) as usize].1.clone();
            let n = r.below(400) as usize;
            v.extend(random(r, n));
            v
        }
    }
}

#[test]
fn garbage_never_panics() {
    let mut rig = Rig::new();
    let mut r = SplitMix::new(77);
    // Lobby phase under fire.
    for i in 0..30_000 {
        rig.junk(&mut r);
        if i % 20 == 0 {
            rig.step();
        }
    }
    // Into a running match (a fresh rig: the junk above may have taken
    // slots or forged control acks, which is fine but not a match).
    let mut rig = Rig::new();
    for _ in 0..5_000 {
        rig.step();
    }
    assert!(
        rig.relay.phase() == frontline_net::relay::Phase::Running,
        "the clean rig started a match"
    );
    for k in 0..200_000u32 {
        rig.junk(&mut r);
        if k % 50 == 0 {
            rig.clients[(k % 2) as usize].send_command(CommandKind::Move {
                squad: SquadId(k % 7),
                target: FxVec2::from_int(10, 20),
            });
            rig.step();
        }
    }
    // Plain decode of junk also never panics.
    for _ in 0..200_000 {
        let bytes = junk(&mut r, &rig.seen);
        let _ = protocol::decode(&bytes);
        let _ = protocol::reject_version(&bytes);
    }
}

/// S10a: the four fortification commands (and a construction crate) go
/// through the relay to both clients unchanged, and junk built from their
/// genuine datagrams never panics.
#[test]
fn fortify_commands_roundtrip_under_garbage() {
    use frontline_content::setup::SupplyKind;
    use frontline_sim::SimId;
    let mut rig = Rig::new();
    for _ in 0..5_000 {
        rig.step();
    }
    assert!(rig.relay.phase() == frontline_net::relay::Phase::Running);
    rig.seen.clear();
    let sent = [
        CommandKind::Fortify {
            squad: SquadId(3),
            fort: 1,
            line: (0..16).map(|i| FxVec2::from_int(100 + 2 * i, 50)).collect(),
        },
        CommandKind::LoadMaterials {
            squad: SquadId(3),
            kg: 160,
        },
        CommandKind::CutWire {
            squad: SquadId(5),
            at: FxVec2::from_int(300, 310),
        },
        CommandKind::PlaceBangalore {
            units: (0..12).map(SimId).collect(),
            target: FxVec2::from_int(305, 320),
        },
        CommandKind::IssueCrate {
            squad: SquadId(3),
            kind: SupplyKind::Construction,
        },
    ];
    for (k, cmd) in sent.iter().enumerate() {
        assert!(rig.clients[k % 2].send_command(cmd.clone()), "{cmd:?}");
        for _ in 0..50 {
            rig.step();
        }
    }
    for _ in 0..2_000 {
        rig.step();
    }
    for (k, cmd) in sent.iter().enumerate() {
        let from = rig.clients[k % 2].slot().expect("welcomed");
        for client in 0..2 {
            assert!(
                rig.bundled
                    .iter()
                    .any(|(i, c)| *i == client && c.player.0 == from && c.kind == *cmd),
                "client {client} got {cmd:?}"
            );
        }
    }
    // Junk from those datagrams, into the running match and plain decode.
    let mut r = SplitMix::new(1010);
    for k in 0..50_000u32 {
        rig.junk(&mut r);
        if k % 50 == 0 {
            rig.step();
        }
    }
    for _ in 0..50_000 {
        let bytes = junk(&mut r, &rig.seen);
        let _ = protocol::decode(&bytes);
    }
}
