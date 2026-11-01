//! The client net thread (S2 "Threads"): owns the socket and the
//! [`NetClient`], so the sim thread and the Godot main thread never touch
//! a socket. The owner talks to it over channels: [`ToNet`] in,
//! [`NetEvent`]s out, and a [`ClientStatus`] snapshot (with the schedule
//! and clock offset that map ticks to step slots) refreshed every ms.
//!
//! Local time is µs since the handle's origin; [`NetHandle::now_us`] reads
//! the same clock, so step slots from the status compare directly.

use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use frontline_sim::CommandKind;

use crate::client::{ClientConfig, ClientPhase, ClientStatus, NetClient, NetEvent};
use crate::protocol::{ByeReason, LobbySet};
use crate::transport::{RECV_BUF, Transport, UdpTransport};

/// Messages to the net thread.
#[derive(Clone, Debug)]
pub enum ToNet {
    Command(CommandKind),
    Hash { tick: u64, hash: u64 },
    Lobby(LobbySet),
    Ready(u64),
    Pause,
    Resume,
    Bye(ByeReason),
}

/// The local address a client binds to reach `relay`: loopback for a
/// loopback relay (no firewall prompt), else any address of its family.
pub fn client_bind_for(relay: SocketAddr) -> SocketAddr {
    let ip = match relay.ip() {
        IpAddr::V4(v) if v.is_loopback() => IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        IpAddr::V6(v) if v.is_loopback() => IpAddr::V6(Ipv6Addr::LOCALHOST),
        IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
    };
    SocketAddr::new(ip, 0)
}

pub struct NetHandle {
    tx: Sender<ToNet>,
    events: Receiver<NetEvent>,
    status: Arc<Mutex<ClientStatus>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<io::Result<()>>>,
    origin: Instant,
    local_addr: SocketAddr,
}

impl NetHandle {
    /// Binds a socket for `cfg.relay` and starts the net thread, which
    /// connects (sends `Hello`) at once.
    pub fn spawn(cfg: ClientConfig) -> io::Result<NetHandle> {
        let mut sock = UdpTransport::bind(client_bind_for(cfg.relay))?;
        let local_addr = sock.local_addr()?;
        let (tx, rx) = channel::<ToNet>();
        let (ev_tx, ev_rx) = channel::<NetEvent>();
        let status = Arc::new(Mutex::new(ClientStatus::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let origin = Instant::now();
        let thread = {
            let status = Arc::clone(&status);
            let stop = Arc::clone(&stop);
            std::thread::Builder::new()
                .name("frontline-net".into())
                .spawn(move || {
                    let mut client = NetClient::new(cfg);
                    let clock = || origin.elapsed().as_micros() as u64;
                    let mut buf = vec![0u8; RECV_BUF];
                    let mut stop_at: Option<u64> = None;
                    loop {
                        if stop.load(Ordering::Relaxed) && stop_at.is_none() {
                            stop_at = Some(clock());
                            if client.phase() != ClientPhase::Ended {
                                client.bye(ByeReason::Quit);
                            }
                        }
                        // Leave once the goodbye is acked, or after 1 s.
                        if let Some(t) = stop_at
                            && (client.bye_acked() || clock().saturating_sub(t) > 1_000_000)
                        {
                            return Ok(());
                        }
                        if let Some((n, from)) =
                            sock.recv_wait(&mut buf, Duration::from_millis(1))?
                        {
                            client.on_datagram(from, &buf[..n], clock());
                            while let Some((n, from)) = sock.recv_from(&mut buf)? {
                                client.on_datagram(from, &buf[..n], clock());
                            }
                        }
                        for m in rx.try_iter() {
                            match m {
                                ToNet::Command(k) => {
                                    client.send_command(k);
                                }
                                ToNet::Hash { tick, hash } => client.send_hash(tick, hash),
                                ToNet::Lobby(l) => client.set_lobby(l),
                                ToNet::Ready(h) => client.ready(h),
                                ToNet::Pause => client.request_pause(),
                                ToNet::Resume => client.request_resume(),
                                ToNet::Bye(r) => client.bye(r),
                            }
                        }
                        client.poll(clock());
                        for (to, bytes) in client.drain_out() {
                            let _ = sock.send_to(&bytes, to);
                        }
                        for ev in client.drain_events() {
                            // The owner may have gone; keep serving the
                            // relay until told to stop.
                            let _ = ev_tx.send(ev);
                        }
                        if let Ok(mut s) = status.lock() {
                            *s = client.status();
                        }
                    }
                })?
        };
        Ok(NetHandle {
            tx,
            events: ev_rx,
            status,
            stop,
            thread: Some(thread),
            origin,
            local_addr,
        })
    }

    pub fn send(&self, m: ToNet) {
        let _ = self.tx.send(m);
    }

    /// Every event since the last call, oldest first.
    pub fn drain_events(&self, out: &mut Vec<NetEvent>) {
        out.extend(self.events.try_iter());
    }

    pub fn status(&self) -> ClientStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// The net thread's local clock (µs), for comparing with step slots.
    pub fn now_us(&self) -> u64 {
        self.origin.elapsed().as_micros() as u64
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Says `Bye(Quit)` unless the match already ended, waits up to 1 s
    /// for the relay's ack, and joins the thread.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for NetHandle {
    fn drop(&mut self) {
        self.stop();
    }
}
