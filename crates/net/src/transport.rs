//! Datagram transports (S2 "Transport"): the [`Transport`] trait, a real
//! [`UdpTransport`], and [`SimNet`], a seeded in-memory network for tests
//! on a virtual clock.
//!
//! The relay and the client never touch a transport themselves (they are
//! sans-IO); the loops in [`crate::server`], [`crate::client_io`] and
//! [`crate::harness`] move bytes between a transport and them. A later
//! move to QUIC datagrams (M7) replaces only this layer.

use std::cell::{Cell, RefCell};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, VecDeque};
use std::io;
use std::net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV6, UdpSocket};
use std::rc::Rc;
use std::time::Duration;

use socket2::{Domain, Protocol, Socket, Type};

use crate::rng::SplitMix;

/// A non-blocking datagram socket.
pub trait Transport {
    fn send_to(&mut self, bytes: &[u8], to: SocketAddr) -> io::Result<()>;
    /// Next waiting datagram, or `None` when there is none. Never blocks.
    fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>>;
    /// Like [`Transport::recv_from`], but may wait up to `timeout` for a
    /// datagram (the relay and the net thread sleep here between turns).
    /// The default does not wait.
    fn recv_wait(
        &mut self,
        buf: &mut [u8],
        timeout: Duration,
    ) -> io::Result<Option<(usize, SocketAddr)>> {
        let _ = timeout;
        self.recv_from(buf)
    }
    fn local_addr(&self) -> io::Result<SocketAddr>;
}

/// Receive buffer size for one datagram: larger than [`MAX_DATAGRAM`] so an
/// oversized datagram is seen whole and refused by the decoder rather than
/// silently truncated into something that might decode.
///
/// [`MAX_DATAGRAM`]: crate::protocol::MAX_DATAGRAM
pub const RECV_BUF: usize = 2048;

// ------------------------------------------------------------------ UDP --

/// A UDP socket from `socket2`. Bound to `[::]` it is dual-stack
/// (`IPV6_V6ONLY` off), so IPv4 and IPv6 peers both work; IPv4 peers then
/// appear as plain IPv4 addresses, never as `::ffff:a.b.c.d`.
pub struct UdpTransport {
    sock: UdpSocket,
    v6: bool,
    /// Read timeout currently set on the socket (`None`: non-blocking).
    wait: Option<Duration>,
}

impl UdpTransport {
    /// Binds `addr`. An unspecified IPv6 address (`[::]:port`) gives a
    /// dual-stack socket; tests bind `127.0.0.1:0` or `[::1]:0`, which
    /// never triggers the macOS incoming-connection prompt.
    pub fn bind(addr: SocketAddr) -> io::Result<UdpTransport> {
        let domain = if addr.is_ipv6() {
            Domain::IPV6
        } else {
            Domain::IPV4
        };
        let s = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;
        if addr.is_ipv6() && addr.ip().is_unspecified() {
            s.set_only_v6(false)?;
        }
        // Catch-up bursts reach a few hundred KB/s; a larger buffer keeps a
        // busy frame from dropping them. Best effort.
        let _ = s.set_recv_buffer_size(1 << 20);
        let _ = s.set_send_buffer_size(1 << 20);
        s.bind(&addr.into())?;
        s.set_nonblocking(true)?;
        Ok(UdpTransport {
            sock: s.into(),
            v6: addr.is_ipv6(),
            wait: None,
        })
    }

    fn recv_inner(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        loop {
            match self.sock.recv_from(buf) {
                Ok((n, from)) => return Ok(Some((n, canonical(from)))),
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    return Ok(None);
                }
                // Windows reports an ICMP port-unreachable from an earlier
                // send as a reset on the next receive, and an oversized
                // datagram as an error; both are only that one datagram.
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::ConnectionReset | io::ErrorKind::Interrupted
                    ) || e.raw_os_error() == Some(10040) =>
                {
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

/// IPv4-mapped IPv6 addresses (from a dual-stack socket) as plain IPv4.
fn canonical(a: SocketAddr) -> SocketAddr {
    SocketAddr::new(a.ip().to_canonical(), a.port())
}

impl Transport for UdpTransport {
    fn send_to(&mut self, bytes: &[u8], to: SocketAddr) -> io::Result<()> {
        let to = match (self.v6, to) {
            // A dual-stack socket reaches IPv4 peers through mapped
            // addresses (macOS refuses a plain AF_INET destination).
            (true, SocketAddr::V4(v4)) => {
                SocketAddr::V6(SocketAddrV6::new(v4.ip().to_ipv6_mapped(), v4.port(), 0, 0))
            }
            _ => to,
        };
        match self.sock.send_to(bytes, to) {
            Ok(_) => Ok(()),
            // A full send buffer drops the datagram, as the network might.
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(()),
            Err(e) => Err(e),
        }
    }

    fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        if self.wait.is_some() {
            self.sock.set_nonblocking(true)?;
            self.wait = None;
        }
        self.recv_inner(buf)
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.sock.local_addr()
    }

    fn recv_wait(
        &mut self,
        buf: &mut [u8],
        timeout: Duration,
    ) -> io::Result<Option<(usize, SocketAddr)>> {
        // A zero read timeout means "forever" on some platforms.
        let want = (timeout >= Duration::from_micros(200)).then_some(timeout);
        if want != self.wait {
            match want {
                Some(t) => {
                    self.sock.set_nonblocking(false)?;
                    self.sock.set_read_timeout(Some(t))?;
                }
                None => self.sock.set_nonblocking(true)?,
            }
            self.wait = want;
        }
        self.recv_inner(buf)
    }
}

/// The address to bind for hosting on `port`: dual-stack `[::]`.
pub fn any_addr(port: u16) -> SocketAddr {
    SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port)
}

// ------------------------------------------------------ impaired socket --

/// A real transport with a seeded bad link in front of it, both ways:
/// datagrams it sends and receives are delayed, jittered, dropped and
/// duplicated per [`LinkParams`], on the wall clock. For
/// `frontline-net-test --net udp`; apply it on the client side only.
pub struct Impaired<T> {
    inner: T,
    params: LinkParams,
    rng: SplitMix,
    origin: std::time::Instant,
    order: u64,
    /// `(due µs, order)` → `(peer, bytes, outgoing)`.
    queue: BTreeMap<(u64, u64), (SocketAddr, Vec<u8>, bool)>,
    buf: Vec<u8>,
}

impl<T: Transport> Impaired<T> {
    pub fn new(inner: T, params: LinkParams, seed: u64) -> Impaired<T> {
        Impaired {
            inner,
            params,
            rng: SplitMix::new(seed),
            origin: std::time::Instant::now(),
            order: 0,
            queue: BTreeMap::new(),
            buf: vec![0; RECV_BUF],
        }
    }

    fn now(&self) -> u64 {
        self.origin.elapsed().as_micros() as u64
    }

    fn enqueue(&mut self, peer: SocketAddr, bytes: &[u8], outgoing: bool) {
        let p = &self.params;
        if p.drop > 0.0 && self.rng.chance(p.drop) {
            return;
        }
        let copies = if p.dup > 0.0 && self.rng.chance(p.dup) {
            2
        } else {
            1
        };
        for _ in 0..copies {
            let p = &self.params;
            let mut at = self.now() + p.delay_us;
            if p.jitter_us > 0 {
                at += self.rng.below(p.jitter_us + 1);
            }
            if p.reorder > 0.0 && self.rng.chance(p.reorder) {
                at += p.reorder_us;
            }
            self.order += 1;
            self.queue
                .insert((at, self.order), (peer, bytes.to_vec(), outgoing));
        }
    }

    /// Sends due outgoing datagrams; returns the first due incoming one.
    fn pump(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        // Everything the socket holds enters the link now.
        while let Some((n, from)) = self.inner.recv_from(&mut self.buf)? {
            let b = self.buf[..n].to_vec();
            self.enqueue(from, &b, false);
        }
        let now = self.now();
        while let Some((&key, _)) = self.queue.first_key_value() {
            if key.0 > now {
                break;
            }
            let (peer, bytes, outgoing) = self.queue.remove(&key).expect("first key exists");
            if outgoing {
                self.inner.send_to(&bytes, peer)?;
            } else {
                let n = bytes.len().min(buf.len());
                buf[..n].copy_from_slice(&bytes[..n]);
                return Ok(Some((n, peer)));
            }
        }
        Ok(None)
    }
}

impl<T: Transport> Transport for Impaired<T> {
    fn send_to(&mut self, bytes: &[u8], to: SocketAddr) -> io::Result<()> {
        self.enqueue(to, bytes, true);
        self.pump(&mut []).map(|_| ())
    }

    fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        self.pump(buf)
    }

    fn recv_wait(
        &mut self,
        buf: &mut [u8],
        timeout: Duration,
    ) -> io::Result<Option<(usize, SocketAddr)>> {
        if let Some(got) = self.pump(buf)? {
            return Ok(Some(got));
        }
        // Wait for the socket, but not past the next delayed datagram.
        let next = self
            .queue
            .first_key_value()
            .map(|(k, _)| Duration::from_micros(k.0.saturating_sub(self.now())));
        let wait = next.map_or(timeout, |n| n.min(timeout));
        if let Some((n, from)) = self.inner.recv_wait(&mut self.buf, wait)? {
            let b = self.buf[..n].to_vec();
            self.enqueue(from, &b, false);
        }
        self.pump(buf)
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }
}

// -------------------------------------------------------- simulated net --

/// Link behaviour of a [`SimNet`], applied to every datagram in either
/// direction.
#[derive(Clone, Debug, Default)]
pub struct LinkParams {
    /// One-way base delay (µs).
    pub delay_us: u64,
    /// Extra one-way delay, uniform in `0..=jitter_us`. Reorders datagrams
    /// sent closer together than the jitter.
    pub jitter_us: u64,
    /// Chance each datagram is lost.
    pub drop: f64,
    /// Chance each datagram is delivered twice (the copy with its own
    /// delay).
    pub dup: f64,
    /// Chance a datagram is held back an extra `reorder_us`.
    pub reorder: f64,
    pub reorder_us: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SimNetStats {
    pub sent: u64,
    pub dropped: u64,
    pub blacked_out: u64,
    pub duplicated: u64,
    pub delivered: u64,
}

struct InFlight {
    from: SocketAddr,
    to: SocketAddr,
    bytes: Vec<u8>,
}

struct Inner {
    now: u64,
    rng: SplitMix,
    params: LinkParams,
    /// `(deliver_at, order)` → datagram; `order` keeps equal times stable.
    queue: BinaryHeap<Reverse<(u64, u64)>>,
    flights: BTreeMap<u64, InFlight>,
    next_order: u64,
    inbox: BTreeMap<SocketAddr, VecDeque<(SocketAddr, Vec<u8>)>>,
    /// `(addr, from, until)`: nothing reaches or leaves `addr` in
    /// `from..until`.
    blackouts: Vec<(SocketAddr, u64, u64)>,
    stats: SimNetStats,
}

impl Inner {
    fn dark(&self, addr: SocketAddr, t: u64) -> bool {
        self.blackouts
            .iter()
            .any(|&(a, from, until)| a == addr && (from..until).contains(&t))
    }

    fn schedule(&mut self, from: SocketAddr, to: SocketAddr, bytes: Vec<u8>) {
        let p = &self.params;
        let mut at = self.now + p.delay_us;
        if p.jitter_us > 0 {
            at += self.rng.below(p.jitter_us + 1);
        }
        if p.reorder > 0.0 && self.rng.chance(p.reorder) {
            at += p.reorder_us;
        }
        let order = self.next_order;
        self.next_order += 1;
        self.queue.push(Reverse((at, order)));
        self.flights.insert(order, InFlight { from, to, bytes });
    }
}

/// A seeded in-memory network on a virtual clock. Single-threaded: tests
/// drive it from one loop with [`SimNet::set_now`].
#[derive(Clone)]
pub struct SimNet {
    inner: Rc<RefCell<Inner>>,
}

impl SimNet {
    pub fn new(seed: u64, params: LinkParams) -> SimNet {
        SimNet {
            inner: Rc::new(RefCell::new(Inner {
                now: 0,
                rng: SplitMix::new(seed ^ 0x6e65_745f_7369_6d00),
                params,
                queue: BinaryHeap::new(),
                flights: BTreeMap::new(),
                next_order: 0,
                inbox: BTreeMap::new(),
                blackouts: Vec::new(),
                stats: SimNetStats::default(),
            })),
        }
    }

    /// A socket at `addr` (any address; nothing is bound on the host).
    pub fn socket(&self, addr: SocketAddr) -> SimSocket {
        self.inner.borrow_mut().inbox.entry(addr).or_default();
        SimSocket {
            inner: Rc::clone(&self.inner),
            addr: Rc::new(Cell::new(addr)),
        }
    }

    /// Advances the virtual clock and delivers every datagram due by `now`.
    pub fn set_now(&self, now: u64) {
        let mut g = self.inner.borrow_mut();
        g.now = now;
        while let Some(Reverse((at, order))) = g.queue.peek().copied() {
            if at > now {
                break;
            }
            g.queue.pop();
            let Some(f) = g.flights.remove(&order) else {
                continue;
            };
            if g.dark(f.to, at) || g.dark(f.from, at) {
                g.stats.blacked_out += 1;
                continue;
            }
            // A datagram to an address nobody holds any more (the sender's
            // old NAT mapping) is lost.
            if let Some(q) = g.inbox.get_mut(&f.to) {
                q.push_back((f.from, f.bytes));
                g.stats.delivered += 1;
            }
        }
    }

    pub fn now(&self) -> u64 {
        self.inner.borrow().now
    }

    /// Nothing reaches or leaves `addr` from `from_us` until `until_us`.
    pub fn blackout(&self, addr: SocketAddr, from_us: u64, until_us: u64) {
        self.inner
            .borrow_mut()
            .blackouts
            .push((addr, from_us, until_us));
    }

    /// Moves `sock` to `new_addr`, as a NAT rebinding or a switch from
    /// Wi-Fi to Ethernet would: later datagrams come from the new address
    /// and datagrams still addressed to the old one are lost.
    pub fn rebind(&self, sock: &SimSocket, new_addr: SocketAddr) {
        let mut g = self.inner.borrow_mut();
        g.inbox.remove(&sock.addr.get());
        g.inbox.entry(new_addr).or_default();
        sock.addr.set(new_addr);
    }

    pub fn set_params(&self, params: LinkParams) {
        self.inner.borrow_mut().params = params;
    }

    pub fn stats(&self) -> SimNetStats {
        self.inner.borrow().stats.clone()
    }
}

/// One endpoint of a [`SimNet`].
pub struct SimSocket {
    inner: Rc<RefCell<Inner>>,
    addr: Rc<Cell<SocketAddr>>,
}

impl SimSocket {
    pub fn addr(&self) -> SocketAddr {
        self.addr.get()
    }
}

impl Transport for SimSocket {
    fn send_to(&mut self, bytes: &[u8], to: SocketAddr) -> io::Result<()> {
        let from = self.addr.get();
        let mut g = self.inner.borrow_mut();
        g.stats.sent += 1;
        let now = g.now;
        if g.dark(from, now) || g.dark(to, now) {
            g.stats.blacked_out += 1;
            return Ok(());
        }
        let (drop, dup) = (g.params.drop, g.params.dup);
        if drop > 0.0 && g.rng.chance(drop) {
            g.stats.dropped += 1;
            return Ok(());
        }
        g.schedule(from, to, bytes.to_vec());
        if dup > 0.0 && g.rng.chance(dup) {
            g.stats.duplicated += 1;
            g.schedule(from, to, bytes.to_vec());
        }
        Ok(())
    }

    fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        let mut g = self.inner.borrow_mut();
        let Some(q) = g.inbox.get_mut(&self.addr.get()) else {
            return Ok(None);
        };
        let Some((from, bytes)) = q.pop_front() else {
            return Ok(None);
        };
        // Like a real socket: a too-small buffer truncates.
        let n = bytes.len().min(buf.len());
        buf[..n].copy_from_slice(&bytes[..n]);
        Ok(Some((n, from)))
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.addr.get())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    #[test]
    fn sim_net_delays_drops_and_blacks_out() {
        let net = SimNet::new(
            1,
            LinkParams {
                delay_us: 10_000,
                ..LinkParams::default()
            },
        );
        let mut a = net.socket(addr("10.0.0.1:1"));
        let mut b = net.socket(addr("10.0.0.2:1"));
        let mut buf = [0u8; 64];
        a.send_to(b"hi", b.addr()).unwrap();
        net.set_now(9_999);
        assert!(b.recv_from(&mut buf).unwrap().is_none());
        net.set_now(10_000);
        let (n, from) = b.recv_from(&mut buf).unwrap().unwrap();
        assert_eq!((&buf[..n], from), (&b"hi"[..], a.addr()));
        net.blackout(b.addr(), 20_000, 30_000);
        net.set_now(15_000);
        a.send_to(b"lost", b.addr()).unwrap(); // arrives at 25 ms: dark
        net.set_now(30_000);
        a.send_to(b"ok", b.addr()).unwrap();
        net.set_now(40_000);
        let (n, _) = b.recv_from(&mut buf).unwrap().unwrap();
        assert_eq!(&buf[..n], b"ok");
        assert!(b.recv_from(&mut buf).unwrap().is_none());
        // Rebinding: replies to the old address are lost, the new one works.
        let old = a.addr();
        net.rebind(&a, addr("10.0.0.1:2"));
        b.send_to(b"old", old).unwrap();
        b.send_to(b"new", a.addr()).unwrap();
        net.set_now(60_000);
        let (n, _) = a.recv_from(&mut buf).unwrap().unwrap();
        assert_eq!(&buf[..n], b"new");
        assert!(a.recv_from(&mut buf).unwrap().is_none());
    }

    #[test]
    fn udp_loopback_roundtrip() {
        let mut a = UdpTransport::bind(addr("127.0.0.1:0")).unwrap();
        let mut b = UdpTransport::bind(addr("127.0.0.1:0")).unwrap();
        a.send_to(b"ping", b.local_addr().unwrap()).unwrap();
        let mut buf = [0u8; RECV_BUF];
        let got = b.recv_wait(&mut buf, Duration::from_secs(2)).unwrap();
        let (n, from) = got.expect("datagram");
        assert_eq!(&buf[..n], b"ping");
        assert_eq!(from, a.local_addr().unwrap());
        assert!(b.recv_from(&mut buf).unwrap().is_none());
    }
}
