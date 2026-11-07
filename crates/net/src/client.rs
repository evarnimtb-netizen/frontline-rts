//! The client side of the protocol, as a sans-IO state machine (S2
//! "Turns", "Session setup"): feed it the relay's datagrams with
//! [`NetClient::on_datagram`], call [`NetClient::poll`] every ms or so,
//! send what [`NetClient::drain_out`] returns, and read [`NetEvent`]s.
//! `now` is the local clock in µs.
//!
//! It stamps the player's queued commands once per period with
//! `target_tick = open relay turn + D` (from the synced clock, not from the
//! newest bundle) and resends every unacked turn of them in each `Input`
//! until their `seq` shows up in a bundle. It buffers bundles that arrive
//! out of order and hands them out in tick order, each exactly once, and
//! acks what it holds every period. It never runs the sim: the caller steps
//! the world (see [`crate::harness::SimDriver`]) at [`NetClient::step_slot`].

use std::collections::{BTreeMap, VecDeque};
use std::net::SocketAddr;

use frontline_sim::{Command, CommandKind};

use crate::clock::ClockSync;
use crate::ctl::CtlOut;
use crate::protocol::*;
use crate::schedule::Schedule;

/// `Hello` resend interval (µs).
pub const HELLO_RESEND_US: u64 = 250_000;
/// Ping interval in the lobby (fast, to size D) and in a match.
pub const LOBBY_PING_US: u64 = 250_000;
pub const MATCH_PING_US: u64 = 1_000_000;
/// Bundles held ahead of the next needed tick; later ones are dropped (the
/// relay resends them).
pub const MAX_BUFFERED: u64 = 4096;
/// Own turns kept in the stamp log (`Client::stamp_target`).
const STAMP_LOG: usize = 64;
/// Commands queued but not yet stamped; more are refused, so a stuck
/// presenter cannot grow the queue without bound. An honest player never
/// gets near it.
pub const MAX_QUEUED: usize = 1024;

/// Most playout ticks (see [`playout_ticks`]): 200 ms.
pub const MAX_PLAYOUT: u64 = 6;
/// Relay silence after which the client stops stamping: it cannot see
/// whether the relay paused, so later stamps could land far ahead of the
/// turns that actually close next. Queued commands wait for the relay.
pub const STAMP_SILENCE_US: u64 = 2_000_000;

/// Input delay suggestion (S2 "D", revised in the S2 fix pass):
/// `clamp(ceil((p99_rtt/2 + jitter) / period), 2, 4)`.
///
/// D only has to cover the trip to the relay: a command stamped for
/// `open + D` must arrive before that turn closes, between D and D+1
/// periods later. Asymmetric paths do not matter, because the synced
/// clock is off by half the asymmetry in the direction that compensates.
/// The trip back is covered by the playout delay ([`playout_ticks`]),
/// which the spec's first formula left at one tick: the old D was about
/// twice what a link needs, and a jittery downlink stalled anyway. On a
/// LAN this is 2; on a 60 ms path with 10 ms jitter, 2; on 120 ms with
/// 20 ms jitter, 3.
pub fn suggest_d(p99_rtt_us: u64, jitter_us: u64, period_us: u64) -> u8 {
    let period = period_us.max(1);
    let ticks = (p99_rtt_us / 2 + jitter_us).div_ceil(period);
    ticks.clamp(MIN_D as u64, SUGGEST_MAX_D as u64) as u8
}

/// Playout delay in ticks: the client steps tick `T` this many periods
/// after turn `T` closes, so a bundle that takes `p99_rtt/2 + jitter` to
/// come back is in time: `clamp(ceil((p99_rtt/2 + jitter) / period), 1,
/// 6)`. Local only (when this client shows each tick), so it changes no
/// result; it is fixed at `Start` from the lobby's pings.
pub fn playout_ticks(p99_rtt_us: u64, jitter_us: u64, period_us: u64) -> u64 {
    let period = period_us.max(1);
    (p99_rtt_us / 2 + jitter_us)
        .div_ceil(period)
        .clamp(1, MAX_PLAYOUT)
}

#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub relay: SocketAddr,
    pub name: String,
    pub build_id: String,
    pub content_hash: u64,
    /// Random per connection (`Hello` resends carry the same one).
    pub nonce: u64,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ClientPhase {
    Connecting,
    Lobby,
    /// `Start` received (the countdown runs until turn 0 opens).
    Running,
    Ended,
}

/// What the client tells its owner, in order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum NetEvent {
    Welcome {
        slot: u8,
    },
    Rejected(RejectReason),
    /// The relay speaks another protocol version.
    WrongVersion {
        relay: u16,
    },
    Lobby(LobbyState),
    Start(Start),
    /// The commands for `World::step` while `tick() == tick`. Handed out
    /// in tick order, each tick exactly once.
    Bundle {
        tick: u64,
        cmds: Vec<Command>,
    },
    Paused {
        at: u64,
        cause: PauseCause,
    },
    /// Turn `at` closes at relay time `close_at`.
    Resume {
        at: u64,
        close_at: u64,
    },
    Desync {
        tick: u64,
        hashes: [u64; 2],
        at: u64,
    },
    Bye(ByeReason),
}

/// The readout a HUD or a test wants.
#[derive(Clone, Debug, Default)]
pub struct ClientStatus {
    pub slot: Option<u8>,
    pub d: u8,
    pub rtt_ms: Option<u16>,
    pub rtt_p99_ms: Option<u16>,
    pub jitter_ms: u16,
    pub offset_us: Option<i64>,
    /// Next tick whose bundle is missing (every earlier one arrived).
    pub held: u64,
    /// Highest tick both clients' hashes agreed on (from the relay).
    pub confirmed: u64,
    /// Own commands stamped, and own commands that ran in a later turn
    /// than stamped.
    pub stamped: u64,
    pub own_restamps: u64,
    /// Bundled commands of either player not run at their stamped turn.
    pub restamps: u64,
    pub bundled: u64,
    /// Turn the match is paused before, if paused.
    pub paused_at: Option<u64>,
    pub peers: [PeerStat; 2],
    /// Local µs since the relay was last heard from.
    pub silent_us: u64,
    pub schedule: Option<Schedule>,
    /// Playout delay in ticks ([`playout_ticks`]; 0 reads as 1).
    pub playout: u64,
}

impl ClientStatus {
    /// See [`NetClient::step_slot`].
    pub fn step_slot(&self, tick: u64) -> Option<u64> {
        step_slot(self.schedule.as_ref()?, self.offset_us?, tick, self.playout)
    }
}

/// Local time at which tick `tick` should be stepped: `playout` periods
/// after its turn closes, mapped through the clock offset.
fn step_slot(s: &Schedule, offset: i64, tick: u64, playout: u64) -> Option<u64> {
    let relay = s
        .close_at(tick)
        .saturating_add(s.period().saturating_mul(playout.max(1)));
    Some((relay as i128 - offset as i128).clamp(0, u64::MAX as i128) as u64)
}

struct StampedTurn {
    target: u64,
    seq0: u32,
    cmds: Vec<CommandKind>,
    /// Encoded size of the `InputTurn`.
    size: usize,
}

pub struct NetClient {
    cfg: ClientConfig,
    phase: ClientPhase,
    token: u64,
    slot: Option<u8>,
    hello_at: Option<u64>,
    ctl_out: CtlOut<Up>,
    ctl_in: u32,
    clock: ClockSync,
    next_ping: u64,
    last_heard: u64,
    start: Option<Start>,
    schedule: Option<Schedule>,
    d: u8,
    playout: u64,
    paused_at: Option<u64>,
    ended: bool,
    // Own input.
    queue: VecDeque<CommandKind>,
    next_seq: u32,
    last_target: u64,
    unacked: VecDeque<StampedTurn>,
    /// The last `STAMP_LOG` own turns as `(seq0, count, target)` (the test
    /// harness checks that orders a finished match never ran were stamped
    /// for a tick at or after its end).
    stamp_log: VecDeque<(u32, u32, u64)>,
    /// Own seqs below this were seen in a bundle.
    own_seen: u32,
    next_input: u64,
    // Bundles.
    held: u64,
    buffer: BTreeMap<u64, Vec<Command>>,
    events: VecDeque<NetEvent>,
    out: Vec<(SocketAddr, Vec<u8>)>,
    buf: Vec<u8>,
    status: ClientStatus,
}

impl NetClient {
    pub fn new(cfg: ClientConfig) -> NetClient {
        NetClient {
            cfg,
            phase: ClientPhase::Connecting,
            token: 0,
            slot: None,
            hello_at: None,
            ctl_out: CtlOut::new(),
            ctl_in: 0,
            clock: ClockSync::new(),
            next_ping: 0,
            last_heard: 0,
            start: None,
            schedule: None,
            d: MIN_D,
            playout: 1,
            paused_at: None,
            ended: false,
            queue: VecDeque::new(),
            next_seq: 0,
            last_target: 0,
            unacked: VecDeque::new(),
            stamp_log: VecDeque::new(),
            own_seen: 0,
            next_input: 0,
            held: 0,
            buffer: BTreeMap::new(),
            events: VecDeque::new(),
            out: Vec::new(),
            buf: Vec::with_capacity(MAX_DATAGRAM),
            status: ClientStatus::default(),
        }
    }

    pub fn phase(&self) -> ClientPhase {
        self.phase
    }

    pub fn slot(&self) -> Option<u8> {
        self.slot
    }

    pub fn start_info(&self) -> Option<&Start> {
        self.start.as_ref()
    }

    pub fn clock(&self) -> &ClockSync {
        &self.clock
    }

    /// Local µs at which tick `tick` should be stepped, once the match
    /// started and the clock is synced.
    pub fn step_slot(&self, tick: u64) -> Option<u64> {
        step_slot(
            self.schedule.as_ref()?,
            self.clock.offset()?,
            tick,
            self.playout,
        )
    }

    /// Playout delay in ticks, fixed at `Start`.
    pub fn playout(&self) -> u64 {
        self.playout
    }

    pub fn schedule(&self) -> Option<Schedule> {
        self.schedule
    }

    /// Highest tick whose hashes the relay confirmed equal.
    pub fn confirmed(&self) -> u64 {
        self.status.confirmed
    }

    /// The target tick own command `seq` was stamped for, if it is among
    /// the last `STAMP_LOG` turns.
    pub fn stamp_target(&self, seq: u32) -> Option<u64> {
        self.stamp_log
            .iter()
            .find(|&&(s0, n, _)| seq >= s0 && seq - s0 < n)
            .map(|&(_, _, t)| t)
    }

    pub fn status(&self) -> ClientStatus {
        let mut s = self.status.clone();
        s.slot = self.slot;
        s.d = self.d;
        let ms = |us: u64| (us / 1000).min(u16::MAX as u64) as u16;
        s.rtt_ms = self.clock.rtt_last().map(ms);
        s.rtt_p99_ms = self.clock.rtt_p99().map(ms);
        s.jitter_ms = ms(self.clock.jitter());
        s.offset_us = self.clock.offset();
        s.held = self.held;
        s.paused_at = self.paused_at;
        s.schedule = self.schedule;
        s.playout = self.playout;
        s
    }

    /// The lobby's D suggestion from this client's pings alone.
    pub fn local_d_suggestion(&self) -> u8 {
        let period = self.schedule.map_or(1_000_000 / 30, |s| s.period());
        suggest_d(
            self.clock.rtt_p99().unwrap_or(0),
            self.clock.jitter(),
            period,
        )
    }

    pub fn drain_events(&mut self) -> std::collections::vec_deque::Drain<'_, NetEvent> {
        self.events.drain(..)
    }

    pub fn drain_out(&mut self) -> std::vec::Drain<'_, (SocketAddr, Vec<u8>)> {
        self.out.drain(..)
    }

    // ------------------------------------------------------ owner calls --

    /// Queues a command of the local player; it is stamped at the next
    /// period. Returns false if the queue is full (never in normal play).
    pub fn send_command(&mut self, kind: CommandKind) -> bool {
        if self.queue.len() >= MAX_QUEUED || !command_in_limits(&kind) {
            return false;
        }
        self.queue.push_back(kind);
        true
    }

    /// Reports a published sim hash (`World::published_hash`).
    pub fn send_hash(&mut self, tick: u64, hash: u64) {
        if self.phase == ClientPhase::Running {
            self.ctl_out.push(Up::Hash { tick, hash });
        }
    }

    pub fn set_lobby(&mut self, l: LobbySet) {
        self.ctl_out.push(Up::LobbySet(l));
    }

    pub fn ready(&mut self, setup_hash: u64) {
        self.ctl_out.push(Up::Ready { setup_hash });
    }

    pub fn request_pause(&mut self) {
        self.ctl_out.push(Up::PauseReq);
    }

    pub fn request_resume(&mut self) {
        self.ctl_out.push(Up::ResumeReq);
    }

    /// Says goodbye; keep polling until [`NetClient::bye_acked`] (or a
    /// short timeout) so the relay hears it.
    pub fn bye(&mut self, reason: ByeReason) {
        if !self.ended {
            self.ctl_out.push(Up::Bye(reason));
            self.ended = true;
        }
    }

    pub fn bye_acked(&self) -> bool {
        self.ended && self.ctl_out.is_empty()
    }

    // ------------------------------------------------------------ input --

    /// Handles one datagram. Only the relay's address is listened to, and
    /// after `Welcome` only datagrams carrying our token.
    pub fn on_datagram(&mut self, from: SocketAddr, bytes: &[u8], now: u64) {
        if from != self.cfg.relay {
            return;
        }
        let (h, p) = match decode(bytes) {
            Ok(x) => x,
            Err(DecodeError::Version(_)) if self.phase == ClientPhase::Connecting => {
                if let Some(v) = reject_version(bytes) {
                    self.phase = ClientPhase::Ended;
                    self.events.push_back(NetEvent::WrongVersion { relay: v });
                }
                return;
            }
            Err(_) => return,
        };
        if !p.is_down() {
            return;
        }
        match p {
            Packet::Welcome(w)
                if self.phase == ClientPhase::Connecting && w.nonce == self.cfg.nonce =>
            {
                self.token = w.token;
                self.slot = Some(w.slot);
                self.phase = ClientPhase::Lobby;
                self.last_heard = now;
                self.next_ping = now;
                self.events.push_back(NetEvent::Welcome { slot: w.slot });
            }
            Packet::Reject(r)
                if self.phase == ClientPhase::Connecting && r.nonce == self.cfg.nonce =>
            {
                self.phase = ClientPhase::Ended;
                self.events.push_back(NetEvent::Rejected(r.reason));
            }
            _ if self.token == 0 || h.token != self.token => {}
            Packet::CtlDown(c) => {
                self.last_heard = now;
                let fresh = c.cseq == self.ctl_in;
                if fresh {
                    self.ctl_in = self.ctl_in.wrapping_add(1);
                }
                self.send(&Packet::Ack(Ack { next: self.ctl_in }));
                if fresh {
                    self.on_down(c.msg, now);
                }
            }
            Packet::Ack(a) => {
                self.last_heard = now;
                self.ctl_out.ack(a.next);
            }
            Packet::Pong(p) => {
                self.last_heard = now;
                self.clock.on_pong(p.t_client, p.t_relay, now);
            }
            Packet::Bundle(b) => {
                self.last_heard = now;
                self.on_bundle(b);
            }
            _ => {}
        }
    }

    fn on_down(&mut self, msg: Down, now: u64) {
        match msg {
            Down::Lobby(l) => self.events.push_back(NetEvent::Lobby(l)),
            Down::Start(s) if self.phase == ClientPhase::Lobby => {
                let schedule = Schedule::new(s.start_at, s.tick_hz, s.speed_pct);
                self.playout = playout_ticks(
                    self.clock.rtt_p99().unwrap_or(0),
                    self.clock.jitter(),
                    schedule.period(),
                );
                self.schedule = Some(schedule);
                self.d = s.d;
                self.phase = ClientPhase::Running;
                self.next_input = now;
                self.events.push_back(NetEvent::Start(s.clone()));
                self.start = Some(s);
            }
            Down::Paused { at, cause } => {
                self.paused_at = Some(at);
                self.events.push_back(NetEvent::Paused { at, cause });
            }
            Down::Resume { at, close_at } => {
                if let Some(s) = self.schedule.as_mut() {
                    s.rebase(at, close_at);
                }
                // Stamps made while the pause was unseen (a blackout) may
                // be far ahead of `at`; the next one starts from `at + D`.
                self.last_target = self.last_target.min((at + self.d as u64).saturating_sub(1));
                self.paused_at = None;
                self.events.push_back(NetEvent::Resume { at, close_at });
            }
            Down::Desync { tick, hashes, at } => {
                self.paused_at = Some(at);
                self.events.push_back(NetEvent::Desync { tick, hashes, at });
            }
            Down::Bye(r) => {
                self.phase = ClientPhase::Ended;
                self.ended = true;
                self.events.push_back(NetEvent::Bye(r));
            }
            Down::Start(_) => {}
        }
    }

    fn on_bundle(&mut self, b: Bundle) {
        if self.phase != ClientPhase::Running {
            return;
        }
        let slot = self.slot.unwrap_or(u8::MAX);
        self.status.confirmed = self.status.confirmed.max(b.confirmed);
        self.status.peers = b.peers;
        for t in b.turns {
            if t.tick < self.held
                || t.tick >= self.held + MAX_BUFFERED
                || self.buffer.contains_key(&t.tick)
            {
                continue;
            }
            let mut cmds = Vec::with_capacity(t.cmds.len());
            for c in t.cmds {
                if c.target != t.tick {
                    self.status.restamps += 1;
                }
                if c.cmd.player.0 == slot {
                    self.own_seen = self.own_seen.max(c.cmd.seq.saturating_add(1));
                    if c.target != t.tick {
                        self.status.own_restamps += 1;
                    }
                }
                cmds.push(c.cmd);
            }
            self.status.bundled += cmds.len() as u64;
            self.buffer.insert(t.tick, cmds);
        }
        while let Some(cmds) = self.buffer.remove(&self.held) {
            self.events.push_back(NetEvent::Bundle {
                tick: self.held,
                cmds,
            });
            self.held += 1;
        }
        // Own input up to `own_seen` is in a bundle: stop resending it.
        while let Some(t) = self.unacked.front_mut() {
            let end = t.seq0 + t.cmds.len() as u32;
            if end <= self.own_seen {
                self.unacked.pop_front();
            } else {
                if t.seq0 < self.own_seen {
                    let k = (self.own_seen - t.seq0) as usize;
                    t.cmds.drain(..k);
                    t.seq0 = self.own_seen;
                    t.size = encoded_len(&InputTurn {
                        target: t.target,
                        seq0: t.seq0,
                        cmds: t.cmds.clone(),
                    });
                }
                break;
            }
        }
    }

    // ------------------------------------------------------------- poll --

    pub fn poll(&mut self, now: u64) {
        self.clock.update(now);
        self.status.silent_us = now.saturating_sub(self.last_heard);
        match self.phase {
            ClientPhase::Connecting => {
                if self
                    .hello_at
                    .is_none_or(|t| now.saturating_sub(t) >= HELLO_RESEND_US)
                {
                    self.hello_at = Some(now);
                    let hello = Packet::Hello(Hello {
                        nonce: self.cfg.nonce,
                        build_id: self.cfg.build_id.clone(),
                        content_hash: self.cfg.content_hash,
                        name: self.cfg.name.clone(),
                    });
                    if encode(0, &hello, &mut self.buf).is_ok() {
                        self.out.push((self.cfg.relay, self.buf.clone()));
                    }
                }
                return;
            }
            ClientPhase::Lobby | ClientPhase::Running => {}
            ClientPhase::Ended => {
                // Still deliver a pending Bye.
                self.flush_ctl(now);
                return;
            }
        }
        if now >= self.next_ping {
            let interval = if self.phase == ClientPhase::Lobby {
                LOBBY_PING_US
            } else {
                MATCH_PING_US
            };
            self.next_ping = now + interval;
            let ping = Packet::Ping(Ping {
                t_client: now,
                rtt_p99_us: self.clock.rtt_p99().unwrap_or(0).min(u32::MAX as u64) as u32,
                jitter_us: self.clock.jitter().min(u32::MAX as u64) as u32,
            });
            self.send(&ping);
        }
        if self.phase == ClientPhase::Running && now >= self.next_input {
            let period = self.schedule.map_or(33_333, |s| s.period());
            // Keep the grid; after a long stall, restart it from now.
            self.next_input = if now - self.next_input > period {
                now + period
            } else {
                self.next_input + period
            };
            self.stamp(now);
            self.send_input();
        }
        self.flush_ctl(now);
    }

    /// Stamps up to one turn of queued commands for `open turn + D`.
    fn stamp(&mut self, now: u64) {
        if self.queue.is_empty()
            || self.paused_at.is_some()
            || self.ended
            || now.saturating_sub(self.last_heard) >= STAMP_SILENCE_US
        {
            return;
        }
        let (Some(s), Some(relay_now)) = (self.schedule, self.clock.to_relay(now)) else {
            return;
        };
        // Each stamp gets its own turn, so no turn holds more than one
        // stamp's quota. Consecutive stamps are a period apart and get
        // consecutive turns anyway; this matters only while the relay clock
        // stands still (the lead-in after a `Resume`).
        let floor = if self.next_seq > 0 {
            self.last_target.saturating_add(1)
        } else {
            0
        };
        let target = s
            .open_at(relay_now)
            .saturating_add(self.d as u64)
            .max(floor);
        let mut cmds = Vec::new();
        let mut bytes = 0;
        while let Some(k) = self.queue.front() {
            let size = encoded_len(k);
            if cmds.len() >= MAX_CMDS_PER_TURN
                || (bytes + size > MAX_TURN_BYTES && !cmds.is_empty())
            {
                break;
            }
            // A command over the byte limit on its own was refused by
            // `send_command`'s limits only if it breaks the list caps; one
            // that is merely large still goes alone.
            bytes += size;
            cmds.push(self.queue.pop_front().expect("front exists"));
        }
        let seq0 = self.next_seq;
        self.next_seq += cmds.len() as u32;
        self.status.stamped += cmds.len() as u64;
        self.last_target = target;
        if !cmds.is_empty() {
            if self.stamp_log.len() >= STAMP_LOG {
                self.stamp_log.pop_front();
            }
            self.stamp_log.push_back((seq0, cmds.len() as u32, target));
        }
        let size = encoded_len(&InputTurn {
            target,
            seq0,
            cmds: cmds.clone(),
        });
        self.unacked.push_back(StampedTurn {
            target,
            seq0,
            cmds,
            size,
        });
    }

    /// One `Input`: the bundle ack and the oldest unacked turns that fit.
    fn send_input(&mut self) {
        let mut mask = 0u64;
        for (&t, _) in self.buffer.range(self.held + 1..=self.held + 64) {
            mask |= 1 << (t - self.held - 1);
        }
        let ack = BundleAck {
            held: self.held,
            mask,
        };
        // Header, ack (two varints) and the turn count.
        let mut room = MAX_DATAGRAM - HEADER_LEN - 20 - 2;
        let mut turns = Vec::new();
        for t in &self.unacked {
            if turns.len() >= MAX_INPUT_TURNS || (t.size > room && !turns.is_empty()) {
                break;
            }
            room = room.saturating_sub(t.size);
            turns.push(InputTurn {
                target: t.target,
                seq0: t.seq0,
                cmds: t.cmds.clone(),
            });
        }
        self.send(&Packet::Input(Input { ack, turns }));
    }

    fn flush_ctl(&mut self, now: u64) {
        if self.token == 0 {
            return;
        }
        for (cseq, msg) in self.ctl_out.due(now) {
            self.send(&Packet::CtlUp(CtlUp { cseq, msg }));
        }
    }

    fn send(&mut self, p: &Packet) {
        if encode(self.token, p, &mut self.buf).is_ok() {
            self.out.push((self.cfg.relay, self.buf.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d_suggestion() {
        let p = 33_333;
        // LAN: sub-ms RTT.
        assert_eq!(suggest_d(400, 50, p), 2);
        assert_eq!(playout_ticks(400, 50, p), 1);
        // 60 ms path with 10 ms jitter: ceil(40/33.3) = 2, and 2 of playout.
        assert_eq!(suggest_d(60_000, 10_000, p), 2);
        assert_eq!(playout_ticks(60_000, 10_000, p), 2);
        // 120 ms with 20 ms jitter: ceil(80/33.3) = 3.
        assert_eq!(suggest_d(120_000, 20_000, p), 3);
        // Terrible paths stay capped at 4 (D) and 6 (playout).
        assert_eq!(suggest_d(500_000, 100_000, p), 4);
        assert_eq!(playout_ticks(500_000, 100_000, p), 6);
    }
}
