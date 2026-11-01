//! The relay on a real socket (S2 "Threads"): one thread owns the socket
//! and the [`Relay`], both inside the host's game and in the
//! `frontline-relay` binary. Whoever starts it only reads a status
//! snapshot, so a frame hitch on the host stalls nobody.
//!
//! Deadlines: the thread sleeps in `recv` until 1 ms before the next turn
//! closes, then spins. Windows timers tick at 15.6 ms unless a process
//! calls `timeBeginPeriod(1)`, which needs `unsafe` FFI this workspace
//! forbids; there the relay spins for the last 16 ms of each period
//! instead (about half a core). S2 ships no Windows game build; this only
//! matters for a Windows-hosted `frontline-relay`.

use std::io::{self, Write};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::relay::{Relay, RelayConfig, RelayStatus};
use crate::rng::TokenSource;
use crate::transport::{RECV_BUF, Transport, UdpTransport};

/// Sleep no closer than this to a turn deadline; spin after.
const SPIN_MARGIN_US: u64 = if cfg!(windows) { 16_000 } else { 1_000 };
/// Longest sleep, so control resends and timeouts run on time.
const MAX_SLEEP_US: u64 = 5_000;
/// How long a stopping relay waits for its `Bye(Shutdown)` acks.
const SHUTDOWN_US: u64 = 500_000;

/// Where the loop writes its JSON-lines log.
pub type LogSink = Box<dyn Write + Send>;

/// Runs `relay` on `sock` until `stop` is set (then it sends every player
/// `Bye(Shutdown)` and waits briefly for the acks), or (with
/// `exit_when_finished`) until the match ended and the goodbyes went out.
/// `clock` is the relay clock (µs since some origin).
pub fn run_relay(
    relay: &mut Relay,
    sock: &mut impl Transport,
    stop: &AtomicBool,
    clock: impl Fn() -> u64,
    mut log: Option<&mut LogSink>,
    status: Option<&Mutex<RelayStatus>>,
    exit_when_finished: bool,
) -> io::Result<()> {
    let mut buf = vec![0u8; RECV_BUF];
    let mut status_at = 0u64;
    let mut stop_at: Option<u64> = None;
    loop {
        // On stop, say `Bye(Shutdown)` to everyone still here and give
        // the acks up to half a second.
        if stop.load(Ordering::Relaxed) && stop_at.is_none() {
            let now = clock();
            relay.shutdown(now);
            stop_at = Some(now);
        }
        if let Some(t) = stop_at {
            let now = clock();
            if relay.is_finished(now) || now.saturating_sub(t) > SHUTDOWN_US {
                break;
            }
        }
        let now = clock();
        let until = relay
            .next_deadline()
            .map_or(MAX_SLEEP_US, |d| d.saturating_sub(now))
            .min(MAX_SLEEP_US);
        let first = if until > SPIN_MARGIN_US {
            sock.recv_wait(&mut buf, Duration::from_micros(until - SPIN_MARGIN_US))?
        } else {
            std::hint::spin_loop();
            sock.recv_from(&mut buf)?
        };
        if let Some((n, from)) = first {
            relay.on_datagram(from, &buf[..n], clock());
            while let Some((n, from)) = sock.recv_from(&mut buf)? {
                relay.on_datagram(from, &buf[..n], clock());
            }
        }
        let now = clock();
        relay.poll(now);
        for (to, bytes) in relay.drain_out() {
            // A send error is one lost datagram (an unreachable peer); the
            // protocol resends what matters.
            let _ = sock.send_to(&bytes, to);
        }
        if let Some(w) = log.as_deref_mut() {
            for rec in relay.drain_log() {
                if let Ok(line) = serde_json::to_string(&rec) {
                    let _ = writeln!(w, "{line}");
                }
            }
        } else {
            relay.drain_log().for_each(drop);
        }
        if let Some(s) = status
            && now.saturating_sub(status_at) >= 100_000
        {
            status_at = now;
            if let Ok(mut g) = s.lock() {
                *g = relay.status(now);
            }
        }
        if exit_when_finished && relay.is_finished(now) {
            break;
        }
    }
    if let Some(w) = log {
        let _ = w.flush();
    }
    Ok(())
}

/// A relay on its own thread (the host's in-game relay).
pub struct RelayServer {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<io::Result<()>>>,
    status: Arc<Mutex<RelayStatus>>,
    addr: SocketAddr,
}

impl RelayServer {
    /// Binds `bind` and starts the relay thread. The thread ends when the
    /// match is over (every player left) or on [`RelayServer::stop`].
    pub fn spawn(
        cfg: RelayConfig,
        bind: SocketAddr,
        log: Option<LogSink>,
    ) -> io::Result<RelayServer> {
        RelayServer::spawn_with(cfg, bind, log, true)
    }

    /// Like [`RelayServer::spawn`]; `log_turns` puts every non-empty closed
    /// turn in the log (the bulk of it).
    pub fn spawn_with(
        cfg: RelayConfig,
        bind: SocketAddr,
        log: Option<LogSink>,
        log_turns: bool,
    ) -> io::Result<RelayServer> {
        let mut sock = UdpTransport::bind(bind)?;
        let addr = sock.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new(RelayStatus::default()));
        let thread = {
            let stop = Arc::clone(&stop);
            let status = Arc::clone(&status);
            std::thread::Builder::new()
                .name("frontline-relay".into())
                .spawn(move || {
                    let mut relay = Relay::new(cfg, TokenSource::os());
                    relay.set_log_turns(log_turns);
                    let origin = Instant::now();
                    let clock = move || origin.elapsed().as_micros() as u64;
                    let mut log = log;
                    run_relay(
                        &mut relay,
                        &mut sock,
                        &stop,
                        clock,
                        log.as_mut(),
                        Some(&status),
                        true,
                    )
                })?
        };
        Ok(RelayServer {
            stop,
            thread: Some(thread),
            status,
            addr,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// The newest status snapshot (refreshed every 100 ms).
    pub fn status(&self) -> RelayStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn is_running(&self) -> bool {
        self.thread.as_ref().is_some_and(|t| !t.is_finished())
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for RelayServer {
    fn drop(&mut self) {
        self.stop();
    }
}
