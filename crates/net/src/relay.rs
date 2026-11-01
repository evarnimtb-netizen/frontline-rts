//! The relay (S2 "Turns", "Session setup", "Hash check", "Disconnect"),
//! as a sans-IO state machine: feed it datagrams with
//! [`Relay::on_datagram`], call [`Relay::poll`] often (every ms or so), and
//! send what [`Relay::drain_out`] returns. Time is an explicit `now` in
//! relay-clock µs, so tests run on a virtual clock.
//!
//! The relay is the match clock. It closes turn `T` at
//! `start + (T+1)·period` whatever has arrived, so no client ever waits on
//! another's CPU. Input that arrives for a closed turn goes, in `seq` order,
//! to the next open one (a restamp); input is never dropped. It does not
//! run the sim and executes nothing it receives: it only reorders and
//! forwards `CommandKind`s, overwriting `Command.player` with the sender's
//! slot.
//!
//! Identity is the random token in every datagram header, not the source
//! address, so a client whose NAT mapping changes keeps its slot.

use std::collections::{BTreeMap, VecDeque};
use std::net::{IpAddr, SocketAddr};

use frontline_sim::{Command, CommandKind, PlayerId};
use serde::Serialize;

use crate::client::suggest_d;
use crate::ctl::CtlOut;
use crate::protocol::*;
use crate::rng::TokenSource;
use crate::schedule::Schedule;

/// Turns may be stamped at most this far past `open + D` (a client that
/// missed a pause, or a clock gone wrong); later targets are pulled in.
pub const LEAD_SLACK: u64 = 30;
/// Encoded bytes of one closed turn, so any turn fits a datagram next to
/// the bundle header. Commands past it roll to the next turn.
pub const MAX_CLOSED_TURN_BYTES: usize = 1100;
/// Bundle datagrams per client per period during catch-up.
pub const MAX_BURST: u32 = 8;
/// Pre-auth `Hello`s per source IP: bucket size and refill per second.
pub const HELLO_BURST: u32 = 8;
pub const HELLO_PER_S: u32 = 4;
/// Source IPs tracked by the pre-auth limiter.
pub const MAX_RATE_ENTRIES: usize = 4096;

#[derive(Clone, Debug)]
pub struct RelayConfig {
    pub tick_hz: u32,
    /// Turn pacing in percent of real time; tests only (`--speed`).
    pub speed_pct: u16,
    /// Cap on D, for both the suggestion and slot 0's override.
    pub max_d: u8,
    /// Let players with different build ids into one lobby (dev only).
    pub allow_build_mismatch: bool,
    /// Lobby → first turn (the 3-2-1 countdown), µs.
    pub countdown_us: u64,
    /// Silence before a slot shows as unstable.
    pub unstable_us: u64,
    /// Silence before the relay pauses for that slot.
    pub pause_us: u64,
    /// Pause length after which a silent slot surrenders.
    pub timeout_us: u64,
    /// `Resume`: the paused turn closes this long after the decision.
    pub resume_lead_us: u64,
    /// Lobby silence after which a slot is freed.
    pub lobby_drop_us: u64,
    pub default_map: String,
}

impl Default for RelayConfig {
    fn default() -> RelayConfig {
        RelayConfig {
            tick_hz: 30,
            speed_pct: 100,
            max_d: MAX_D,
            allow_build_mismatch: false,
            countdown_us: 3_000_000,
            unstable_us: 2_000_000,
            pause_us: 10_000_000,
            timeout_us: 60_000_000,
            resume_lead_us: 300_000,
            lobby_drop_us: 10_000_000,
            default_map: "ww2:kessel_ridge".into(),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize)]
pub enum Phase {
    Lobby,
    Running,
    /// Every player left, timed out or said `Bye`.
    Ended,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize)]
pub enum SlotState {
    Connected,
    /// Said `Bye` or was refused for flooding.
    Left,
    TimedOut,
}

#[derive(Clone, Debug)]
struct Pending {
    seq: u32,
    target: u64,
    /// The turn it may close in (target clamped, never before an earlier
    /// seq's).
    eff: u64,
    kind: CommandKind,
    size: usize,
}

struct Slot {
    token: u64,
    nonce: u64,
    addr: SocketAddr,
    name: String,
    build_id: String,
    content_hash: u64,
    faction: String,
    ready: Option<u64>,
    state: SlotState,
    last_heard: u64,
    rtt_p99_us: u32,
    jitter_us: u32,
    ctl_in: u32,
    ctl_out: CtlOut<Down>,
    /// Next `seq` accepted from this player.
    next_seq: u32,
    last_eff: u64,
    pending: VecDeque<Pending>,
    ack: BundleAck,
    /// Turn → relay time it was last sent to this slot (turns not acked).
    sent: BTreeMap<u64, u64>,
    burst_start: u64,
    burst_count: u32,
    restamps: u64,
}

impl Slot {
    fn active(&self) -> bool {
        self.state == SlotState::Connected
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize)]
pub enum PauseKind {
    Player(u8),
    Waiting(u8),
    Desync,
}

#[derive(Copy, Clone, Debug)]
struct PauseState {
    at: u64,
    kind: PauseKind,
    since: u64,
}

/// One line of the relay log (`--log`, `relay.jsonl` in a desync dump).
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "ev", rename_all = "snake_case")]
pub enum LogRecord {
    Joined {
        t: u64,
        slot: u8,
        name: String,
        addr: String,
        build: String,
    },
    Rejected {
        t: u64,
        addr: String,
        reason: String,
    },
    Moved {
        t: u64,
        slot: u8,
        from: String,
        to: String,
    },
    Started {
        t: u64,
        start_at: u64,
        d: u8,
        seed: u64,
        map: String,
    },
    /// A closed turn with commands (empty turns are not logged).
    Turn {
        t: u64,
        tick: u64,
        cmds: Vec<(u8, u32, u64, String)>,
    },
    Hash {
        t: u64,
        slot: u8,
        tick: u64,
        hash: String,
    },
    Paused {
        t: u64,
        at: u64,
        cause: PauseKind,
    },
    Resumed {
        t: u64,
        at: u64,
        close_at: u64,
    },
    Desync {
        t: u64,
        tick: u64,
        hashes: [String; 2],
        at: u64,
    },
    TimedOut {
        t: u64,
        slot: u8,
    },
    Left {
        t: u64,
        slot: u8,
        reason: String,
    },
    Flood {
        t: u64,
        slot: u8,
    },
    Ended {
        t: u64,
    },
}

/// What the host's UI and `frontline-relay` show.
#[derive(Clone, Debug, Default, Serialize)]
pub struct RelayStatus {
    pub phase: String,
    pub slots: [Option<SlotStatus>; 2],
    /// Next turn to close.
    pub next_turn: u64,
    pub d: u8,
    pub paused: Option<String>,
    pub confirmed: u64,
    pub desync_tick: Option<u64>,
    pub restamps: u64,
    pub commands: u64,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SlotStatus {
    pub name: String,
    pub addr: String,
    pub rtt_ms: u16,
    pub silent_ms: u64,
    pub state: String,
    pub held: u64,
    pub pending: usize,
}

struct Bucket {
    tokens: u32,
    last: u64,
}

pub struct Relay {
    cfg: RelayConfig,
    tokens: TokenSource,
    phase: Phase,
    slots: [Option<Slot>; 2],
    seed: u64,
    map: String,
    d_override: Option<u8>,
    note: String,
    /// Lobby state last broadcast (to resend only on change).
    lobby_sent: Option<LobbyState>,
    lobby_sent_at: u64,
    // Running.
    d: u8,
    schedule: Schedule,
    next_turn: u64,
    /// Closed turns from `turns_base`, postcard-encoded `Turn`s.
    turns: VecDeque<Vec<u8>>,
    turns_base: u64,
    pause: Option<PauseState>,
    hashes: BTreeMap<u64, [Option<u64>; 2]>,
    confirmed: u64,
    desync_tick: Option<u64>,
    restamps: u64,
    commands: u64,
    ended_at: Option<u64>,
    rate: BTreeMap<IpAddr, Bucket>,
    out: Vec<(SocketAddr, Vec<u8>)>,
    log: Vec<LogRecord>,
    log_turns: bool,
    buf: Vec<u8>,
}

impl Relay {
    pub fn new(cfg: RelayConfig, tokens: TokenSource) -> Relay {
        let schedule = Schedule::new(0, cfg.tick_hz, cfg.speed_pct);
        Relay {
            map: cfg.default_map.clone(),
            cfg,
            tokens,
            phase: Phase::Lobby,
            slots: [None, None],
            seed: 1,
            d_override: None,
            note: String::new(),
            lobby_sent: None,
            lobby_sent_at: 0,
            d: MIN_D,
            schedule,
            next_turn: 0,
            turns: VecDeque::new(),
            turns_base: 0,
            pause: None,
            hashes: BTreeMap::new(),
            confirmed: 0,
            desync_tick: None,
            restamps: 0,
            commands: 0,
            ended_at: None,
            rate: BTreeMap::new(),
            out: Vec::new(),
            log: Vec::new(),
            log_turns: true,
            buf: Vec::with_capacity(MAX_DATAGRAM),
        }
    }

    /// Whether closed turns go into the log (they are the bulk of it).
    pub fn set_log_turns(&mut self, on: bool) {
        self.log_turns = on;
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn d(&self) -> u8 {
        self.d
    }

    pub fn next_turn(&self) -> u64 {
        self.next_turn
    }

    pub fn restamps(&self) -> u64 {
        self.restamps
    }

    pub fn schedule(&self) -> Option<Schedule> {
        (self.phase != Phase::Lobby).then_some(self.schedule)
    }

    /// The slot's state, if the slot was ever taken.
    pub fn slot_state(&self, slot: u8) -> Option<SlotState> {
        self.slots.get(slot as usize)?.as_ref().map(|s| s.state)
    }

    pub fn desync_tick(&self) -> Option<u64> {
        self.desync_tick
    }

    pub fn paused(&self) -> Option<(u64, PauseKind)> {
        self.pause.map(|p| (p.at, p.kind))
    }

    /// True a few seconds after the match ended, once every `Bye` was
    /// acked or given up on: a binary may exit or start a new relay.
    pub fn is_finished(&self, now: u64) -> bool {
        self.ended_at.is_some_and(|t| {
            now.saturating_sub(t) > 5_000_000
                || self.slots.iter().flatten().all(|s| s.ctl_out.is_empty())
        })
    }

    pub fn drain_out(&mut self) -> std::vec::Drain<'_, (SocketAddr, Vec<u8>)> {
        self.out.drain(..)
    }

    pub fn drain_log(&mut self) -> std::vec::Drain<'_, LogRecord> {
        self.log.drain(..)
    }

    /// Relay time at which `poll` next has turn work to do (a turn to
    /// close), if the match runs and is not paused. Control resends and
    /// timeouts want a poll at least every few ms anyway.
    pub fn next_deadline(&self) -> Option<u64> {
        (self.phase == Phase::Running && self.pause.is_none())
            .then(|| self.schedule.close_at(self.next_turn))
    }

    pub fn status(&self, now: u64) -> RelayStatus {
        let slot = |s: &Slot| SlotStatus {
            name: s.name.clone(),
            addr: s.addr.to_string(),
            rtt_ms: (s.rtt_p99_us / 1000).min(u16::MAX as u32) as u16,
            silent_ms: now.saturating_sub(s.last_heard) / 1000,
            state: format!("{:?}", s.state),
            held: s.ack.held,
            pending: s.pending.len(),
        };
        RelayStatus {
            phase: format!("{:?}", self.phase),
            slots: [
                self.slots[0].as_ref().map(slot),
                self.slots[1].as_ref().map(slot),
            ],
            next_turn: self.next_turn,
            d: self.d,
            paused: self.pause.map(|p| format!("{:?} at {}", p.kind, p.at)),
            confirmed: self.confirmed,
            desync_tick: self.desync_tick,
            restamps: self.restamps,
            commands: self.commands,
        }
    }

    // ------------------------------------------------------------ input --

    /// Handles one datagram from `from`. Anything malformed, from an
    /// unknown token, or of a relay-only kind is dropped without a reply.
    pub fn on_datagram(&mut self, from: SocketAddr, bytes: &[u8], now: u64) {
        let (h, p) = match decode(bytes) {
            Ok(x) => x,
            Err(DecodeError::Version(_)) => {
                // A client on another protocol version: tell it ours, if it
                // is a padded Hello (never amplify) and the address is
                // within its rate.
                if let Some(h) = Header::read(bytes)
                    && h.kind == Kind::Hello as u8
                    && h.token == 0
                    && bytes.len() >= HELLO_MIN_LEN
                    && self.rate_ok(from.ip(), now)
                {
                    self.reply_reject(from, 0, RejectReason::Version, now);
                }
                return;
            }
            Err(_) => return,
        };
        if !p.is_up() {
            return;
        }
        if let Packet::Hello(hello) = p {
            if h.token == 0 && bytes.len() >= HELLO_MIN_LEN && self.rate_ok(from.ip(), now) {
                self.on_hello(from, hello, now);
            }
            return;
        }
        if h.token == 0 {
            return;
        }
        let Some(i) = self.slot_of(h.token) else {
            return;
        };
        {
            let s = self.slots[i]
                .as_mut()
                .expect("slot_of returned a taken slot");
            if s.addr != from {
                let rec = LogRecord::Moved {
                    t: now,
                    slot: i as u8,
                    from: s.addr.to_string(),
                    to: from.to_string(),
                };
                s.addr = from;
                self.log.push(rec);
            }
            s.last_heard = now;
        }
        match p {
            Packet::CtlUp(c) => self.on_ctl(i, c, now),
            Packet::Ack(a) => {
                if let Some(s) = self.slots[i].as_mut() {
                    s.ctl_out.ack(a.next);
                }
            }
            Packet::Ping(ping) => {
                let s = self.slots[i].as_mut().expect("slot exists");
                s.rtt_p99_us = ping.rtt_p99_us;
                s.jitter_us = ping.jitter_us;
                let (addr, token) = (s.addr, s.token);
                self.send(
                    addr,
                    token,
                    &Packet::Pong(Pong {
                        t_client: ping.t_client,
                        t_relay: now,
                    }),
                );
            }
            Packet::Input(input) => self.on_input(i, input, now),
            _ => {}
        }
    }

    fn slot_of(&self, token: u64) -> Option<usize> {
        self.slots
            .iter()
            .position(|s| s.as_ref().is_some_and(|s| s.token == token))
    }

    /// Token bucket per source IP for pre-auth traffic.
    fn rate_ok(&mut self, ip: IpAddr, now: u64) -> bool {
        if !self.rate.contains_key(&ip) && self.rate.len() >= MAX_RATE_ENTRIES {
            // Forget addresses whose buckets have refilled; if all are
            // busy, refuse newcomers rather than grow without bound.
            let full = |b: &Bucket| {
                b.tokens as u64 + now.saturating_sub(b.last) * HELLO_PER_S as u64 / 1_000_000
                    >= HELLO_BURST as u64
            };
            self.rate.retain(|_, b| !full(b));
            if self.rate.len() >= MAX_RATE_ENTRIES {
                return false;
            }
        }
        let b = self.rate.entry(ip).or_insert(Bucket {
            tokens: HELLO_BURST,
            last: now,
        });
        let refill = now.saturating_sub(b.last) * HELLO_PER_S as u64 / 1_000_000;
        if refill > 0 {
            b.tokens = (b.tokens as u64 + refill).min(HELLO_BURST as u64) as u32;
            b.last += refill * 1_000_000 / HELLO_PER_S as u64;
        }
        if b.tokens == 0 {
            return false;
        }
        b.tokens -= 1;
        true
    }

    fn on_hello(&mut self, from: SocketAddr, hello: Hello, now: u64) {
        // A resent Hello (lost Welcome): same answer.
        if let Some(i) = self.slots.iter().position(|s| {
            s.as_ref()
                .is_some_and(|s| s.nonce == hello.nonce && s.addr == from)
        }) {
            let s = self.slots[i].as_ref().expect("found");
            let w = Packet::Welcome(Welcome {
                nonce: hello.nonce,
                token: s.token,
                slot: i as u8,
            });
            self.send(from, 0, &w);
            return;
        }
        let reject = if self.phase != Phase::Lobby {
            Some(RejectReason::Started)
        } else if hello.name.trim().is_empty() {
            Some(RejectReason::BadName)
        } else if let Some(other) = self.slots.iter().flatten().next() {
            if other.build_id != hello.build_id && !self.cfg.allow_build_mismatch {
                Some(RejectReason::Build {
                    lobby: other.build_id.clone(),
                })
            } else if other.content_hash != hello.content_hash {
                Some(RejectReason::Content)
            } else {
                None
            }
        } else {
            None
        };
        let free = self.slots.iter().position(|s| s.is_none());
        let reason = match (reject, free) {
            (Some(r), _) => r,
            (None, None) => RejectReason::Full,
            (None, Some(i)) => {
                let token = self.tokens.next_token();
                self.log.push(LogRecord::Joined {
                    t: now,
                    slot: i as u8,
                    name: hello.name.clone(),
                    addr: from.to_string(),
                    build: hello.build_id.clone(),
                });
                self.slots[i] = Some(Slot {
                    token,
                    nonce: hello.nonce,
                    addr: from,
                    name: hello.name,
                    build_id: hello.build_id,
                    content_hash: hello.content_hash,
                    faction: String::new(),
                    ready: None,
                    state: SlotState::Connected,
                    last_heard: now,
                    rtt_p99_us: 0,
                    jitter_us: 0,
                    ctl_in: 0,
                    ctl_out: CtlOut::new(),
                    next_seq: 0,
                    last_eff: 0,
                    pending: VecDeque::new(),
                    ack: BundleAck::default(),
                    sent: BTreeMap::new(),
                    burst_start: 0,
                    burst_count: 0,
                    restamps: 0,
                });
                self.send(
                    from,
                    0,
                    &Packet::Welcome(Welcome {
                        nonce: hello.nonce,
                        token,
                        slot: i as u8,
                    }),
                );
                self.lobby_changed();
                return;
            }
        };
        self.reply_reject(from, hello.nonce, reason, now);
    }

    fn reply_reject(&mut self, to: SocketAddr, nonce: u64, reason: RejectReason, now: u64) {
        self.log.push(LogRecord::Rejected {
            t: now,
            addr: to.to_string(),
            reason: format!("{reason:?}"),
        });
        self.send(
            to,
            0,
            &Packet::Reject(Reject {
                relay_version: PROTOCOL_VERSION,
                nonce,
                reason,
            }),
        );
    }

    fn on_ctl(&mut self, i: usize, c: CtlUp, now: u64) {
        let s = self.slots[i].as_mut().expect("slot exists");
        let fresh = c.cseq == s.ctl_in;
        if fresh {
            s.ctl_in = s.ctl_in.wrapping_add(1);
        }
        let (addr, token, next) = (s.addr, s.token, s.ctl_in);
        self.send(addr, token, &Packet::Ack(Ack { next }));
        if fresh {
            self.on_up(i, c.msg, now);
        }
    }

    fn on_up(&mut self, i: usize, msg: Up, now: u64) {
        match msg {
            Up::LobbySet(l) if self.phase == Phase::Lobby => {
                let s = self.slots[i].as_mut().expect("slot exists");
                s.faction = l.faction;
                if i == 0 {
                    if let Some(seed) = l.seed {
                        self.seed = seed;
                    }
                    if let Some(m) = l.map {
                        self.map = m;
                    }
                    self.d_override = l.d.map(|d| d.clamp(MIN_D, self.cfg.max_d.max(MIN_D)));
                }
                for s in self.slots.iter_mut().flatten() {
                    s.ready = None;
                }
                self.note.clear();
                self.lobby_changed();
            }
            Up::Ready { setup_hash } if self.phase == Phase::Lobby => {
                self.slots[i].as_mut().expect("slot exists").ready = Some(setup_hash);
                self.try_start(now);
                if self.phase == Phase::Lobby {
                    self.lobby_changed();
                }
            }
            Up::Hash { tick, hash } if self.phase == Phase::Running => {
                self.on_hash(i, tick, hash, now)
            }
            Up::PauseReq if self.phase == Phase::Running && self.pause.is_none() => {
                self.pause_for(PauseKind::Player(i as u8), now);
            }
            Up::ResumeReq => {
                if let Some(p) = self.pause
                    && matches!(p.kind, PauseKind::Player(_))
                {
                    self.resume(now);
                }
            }
            Up::Bye(reason) => self.on_bye(i, reason, now),
            _ => {}
        }
    }

    fn on_bye(&mut self, i: usize, reason: ByeReason, now: u64) {
        match self.phase {
            Phase::Lobby => {
                self.slots[i] = None;
                for s in self.slots.iter_mut().flatten() {
                    s.ready = None;
                }
                self.lobby_changed();
            }
            Phase::Running => {
                let s = self.slots[i].as_mut().expect("slot exists");
                if s.state == SlotState::Connected {
                    s.state = SlotState::Left;
                    self.log.push(LogRecord::Left {
                        t: now,
                        slot: i as u8,
                        reason: format!("{reason:?}"),
                    });
                    // Quitting mid-match is surrendering. After the match is
                    // over the extra command changes nothing on screen, but
                    // both sims still apply it identically.
                    if reason != ByeReason::MatchOver {
                        self.inject_surrender(i);
                    }
                    if let Some(p) = self.pause
                        && p.kind == PauseKind::Waiting(i as u8)
                    {
                        self.resume(now);
                    }
                }
                self.check_ended(now);
            }
            Phase::Ended => {}
        }
    }

    fn check_ended(&mut self, now: u64) {
        if self.phase == Phase::Running && self.slots.iter().flatten().all(|s| !s.active()) {
            self.phase = Phase::Ended;
            self.ended_at = Some(now);
            self.log.push(LogRecord::Ended { t: now });
        }
    }

    fn try_start(&mut self, now: u64) {
        let (Some(a), Some(b)) = (&self.slots[0], &self.slots[1]) else {
            return;
        };
        let (Some(ha), Some(hb)) = (a.ready, b.ready) else {
            return;
        };
        let problem = if a.faction.is_empty() || b.faction.is_empty() {
            Some("both players must pick a faction")
        } else if a.faction == b.faction {
            Some("the two factions must differ")
        } else if ha != hb {
            Some("match setups differ (different mod content or game build)")
        } else {
            None
        };
        if let Some(p) = problem {
            self.note = p.to_owned();
            for s in self.slots.iter_mut().flatten() {
                s.ready = None;
            }
            return;
        }
        self.d = self
            .d_override
            .unwrap_or_else(|| self.suggested_d())
            .clamp(MIN_D, self.cfg.max_d.max(MIN_D));
        let start_at = now + self.cfg.countdown_us;
        self.schedule = Schedule::new(start_at, self.cfg.tick_hz, self.cfg.speed_pct);
        self.phase = Phase::Running;
        let names = [a.name.clone(), b.name.clone()];
        let factions = [a.faction.clone(), b.faction.clone()];
        let start = Start {
            start_at,
            tick_hz: self.cfg.tick_hz,
            speed_pct: self.cfg.speed_pct,
            d: self.d,
            seed: self.seed,
            map: self.map.clone(),
            factions,
            names,
        };
        self.log.push(LogRecord::Started {
            t: now,
            start_at,
            d: self.d,
            seed: self.seed,
            map: self.map.clone(),
        });
        for s in self.slots.iter_mut().flatten() {
            s.last_heard = now;
        }
        self.broadcast(Down::Start(start));
    }

    /// The worst slot's [`suggest_d`], capped by `max_d`.
    fn suggested_d(&self) -> u8 {
        let period = self.schedule.period();
        self.slots
            .iter()
            .flatten()
            .map(|s| suggest_d(s.rtt_p99_us as u64, s.jitter_us as u64, period))
            .max()
            .unwrap_or(MIN_D)
            .min(self.cfg.max_d.max(MIN_D))
    }

    fn lobby_state(&self) -> LobbyState {
        let info = |s: &Slot| SlotInfo {
            name: s.name.clone(),
            faction: s.faction.clone(),
            ready: s.ready.is_some(),
            rtt_ms: (s.rtt_p99_us / 1000).min(u16::MAX as u32) as u16,
        };
        LobbyState {
            slots: [
                self.slots[0].as_ref().map(info),
                self.slots[1].as_ref().map(info),
            ],
            seed: self.seed,
            map: self.map.clone(),
            d: self.d_override,
            suggested_d: self.suggested_d(),
            note: self.note.clone(),
        }
    }

    fn lobby_changed(&mut self) {
        let l = self.lobby_state();
        if self.lobby_sent.as_ref() != Some(&l) {
            self.lobby_sent = Some(l.clone());
            self.broadcast(Down::Lobby(l));
        }
    }

    fn on_input(&mut self, i: usize, input: Input, now: u64) {
        let next_turn = self.next_turn;
        let (d, running) = (self.d as u64, self.phase == Phase::Running);
        let s = self.slots[i].as_mut().expect("slot exists");
        // The ack: never beyond what was closed, never backwards.
        let mut ack = input.ack;
        if ack.held > next_turn {
            ack = BundleAck {
                held: next_turn,
                mask: 0,
            };
        }
        if ack.held >= s.ack.held {
            s.ack = ack;
            let held = s.ack.held;
            s.sent = s.sent.split_off(&held);
        }
        if !running || !s.active() {
            return;
        }
        let mut flood = false;
        'turns: for t in input.turns {
            for (k, kind) in t.cmds.into_iter().enumerate() {
                let Some(seq) = t.seq0.checked_add(k as u32) else {
                    break 'turns;
                };
                if seq < s.next_seq {
                    continue;
                }
                if seq > s.next_seq {
                    // A gap: wait for the client's resend.
                    break 'turns;
                }
                if s.pending.len() >= MAX_PENDING {
                    flood = true;
                    break 'turns;
                }
                let lead_cap = next_turn + d + LEAD_SLACK;
                let eff = t.target.min(lead_cap).max(s.last_eff);
                let size = encoded_len(&kind);
                s.pending.push_back(Pending {
                    seq,
                    target: t.target,
                    eff,
                    kind,
                    size,
                });
                s.next_seq += 1;
                s.last_eff = eff;
            }
        }
        if flood {
            s.state = SlotState::Left;
            s.ctl_out.push(Down::Bye(ByeReason::Flood));
            self.log.push(LogRecord::Flood {
                t: now,
                slot: i as u8,
            });
            self.inject_surrender(i);
            self.check_ended(now);
        }
    }

    fn on_hash(&mut self, i: usize, tick: u64, hash: u64, now: u64) {
        if tick > self.next_turn || tick <= self.confirmed || self.desync_tick.is_some() {
            return;
        }
        self.log.push(LogRecord::Hash {
            t: now,
            slot: i as u8,
            tick,
            hash: format!("{hash:016x}"),
        });
        let other = 1 - i;
        if !self.slots[other].as_ref().is_some_and(|s| s.active()) {
            return;
        }
        let e = self.hashes.entry(tick).or_insert([None, None]);
        e[i] = Some(hash);
        if let [Some(a), Some(b)] = *e {
            self.hashes.remove(&tick);
            if a == b {
                self.confirmed = self.confirmed.max(tick);
                self.hashes = self.hashes.split_off(&(self.confirmed + 1));
            } else {
                self.desync(tick, [a, b], now);
            }
        }
        while self.hashes.len() > 512 {
            self.hashes.pop_first();
        }
    }

    fn desync(&mut self, tick: u64, hashes: [u64; 2], now: u64) {
        let at = self.next_turn;
        self.desync_tick = Some(tick);
        self.pause = Some(PauseState {
            at,
            kind: PauseKind::Desync,
            since: now,
        });
        self.log.push(LogRecord::Desync {
            t: now,
            tick,
            hashes: hashes.map(|h| format!("{h:016x}")),
            at,
        });
        self.broadcast(Down::Desync { tick, hashes, at });
    }

    fn pause_for(&mut self, kind: PauseKind, now: u64) {
        let at = self.next_turn;
        self.pause = Some(PauseState {
            at,
            kind,
            since: now,
        });
        self.log.push(LogRecord::Paused {
            t: now,
            at,
            cause: kind,
        });
        let cause = match kind {
            PauseKind::Player(s) => PauseCause::Player(s),
            PauseKind::Waiting(s) => PauseCause::Waiting(s),
            PauseKind::Desync => return,
        };
        self.broadcast(Down::Paused { at, cause });
    }

    fn resume(&mut self, now: u64) {
        let Some(p) = self.pause else {
            return;
        };
        if p.kind == PauseKind::Desync {
            return;
        }
        let close_at = now + self.cfg.resume_lead_us;
        self.schedule.rebase(p.at, close_at);
        self.pause = None;
        self.log.push(LogRecord::Resumed {
            t: now,
            at: p.at,
            close_at,
        });
        self.broadcast(Down::Resume { at: p.at, close_at });
    }

    /// Queues a `Surrender` from slot `i` with its next `seq`, due in the
    /// next turn to close (after any of its input already waiting).
    fn inject_surrender(&mut self, i: usize) {
        let next_turn = self.next_turn;
        let s = self.slots[i].as_mut().expect("slot exists");
        let kind = CommandKind::Surrender;
        let size = encoded_len(&kind);
        s.pending.push_back(Pending {
            seq: s.next_seq,
            target: next_turn,
            eff: s.last_eff.max(next_turn),
            kind,
            size,
        });
        s.next_seq += 1;
    }

    // ------------------------------------------------------------- poll --

    /// Advances timers: closes due turns, sends bundles and control
    /// resends, and runs the silence and timeout rules.
    pub fn poll(&mut self, now: u64) {
        match self.phase {
            Phase::Lobby => self.poll_lobby(now),
            Phase::Running => self.poll_running(now),
            Phase::Ended => {}
        }
        self.flush_ctl(now);
    }

    fn poll_lobby(&mut self, now: u64) {
        let mut changed = false;
        for slot in self.slots.iter_mut() {
            if slot
                .as_ref()
                .is_some_and(|s| now.saturating_sub(s.last_heard) >= self.cfg.lobby_drop_us)
            {
                *slot = None;
                changed = true;
            }
        }
        if changed {
            for s in self.slots.iter_mut().flatten() {
                s.ready = None;
            }
        }
        // Pings move the RTT readout and the D suggestion; refresh at most
        // once a second.
        if changed || now.saturating_sub(self.lobby_sent_at) >= 1_000_000 {
            self.lobby_sent_at = now;
            self.lobby_changed();
        }
    }

    fn poll_running(&mut self, now: u64) {
        self.check_silence(now);
        if self.pause.is_none() {
            while now >= self.schedule.close_at(self.next_turn) {
                self.close_turn(now);
            }
        }
        self.prune_turns();
        for i in 0..SLOTS {
            self.send_bundles(i, now);
        }
    }

    fn check_silence(&mut self, now: u64) {
        let (pause_us, timeout_us) = (self.cfg.pause_us, self.cfg.timeout_us);
        let silent = |s: &Option<Slot>| {
            s.as_ref()
                .filter(|s| s.active())
                .map(|s| now.saturating_sub(s.last_heard))
        };
        let silence = [silent(&self.slots[0]), silent(&self.slots[1])];
        match self.pause {
            Some(p) if p.kind == PauseKind::Desync => {}
            Some(p) => {
                if let PauseKind::Waiting(w) = p.kind {
                    let w = w as usize;
                    let s = self.slots[w].as_ref().expect("waiting on a taken slot");
                    if !s.active() {
                        self.resume(now);
                    } else if silence[w].is_some_and(|q| q < 1_000_000) && s.ack.held >= p.at {
                        // Back, and holds every bundle before the pause.
                        self.resume(now);
                    } else if now.saturating_sub(p.since) >= timeout_us
                        && silence[w].is_some_and(|q| q >= pause_us)
                    {
                        self.time_out(w, now);
                        self.resume(now);
                    }
                } else if let Some(w) =
                    (0..SLOTS).find(|&k| silence[k].is_some_and(|q| q >= pause_us))
                {
                    // A player pause turns into a wait for the silent slot,
                    // so the timeout still runs.
                    self.pause = None;
                    self.pause_for(PauseKind::Waiting(w as u8), now);
                }
            }
            None => {
                if let Some(w) = (0..SLOTS).find(|&k| silence[k].is_some_and(|q| q >= pause_us)) {
                    self.pause_for(PauseKind::Waiting(w as u8), now);
                }
            }
        }
        // After a resume, the other slot may itself be silent.
        if self.pause.is_none()
            && let Some(w) = (0..SLOTS).find(|&k| {
                self.slots[k]
                    .as_ref()
                    .is_some_and(|s| s.active() && now.saturating_sub(s.last_heard) >= pause_us)
            })
        {
            self.pause_for(PauseKind::Waiting(w as u8), now);
        }
    }

    fn time_out(&mut self, i: usize, now: u64) {
        let s = self.slots[i].as_mut().expect("slot exists");
        s.state = SlotState::TimedOut;
        s.ctl_out.push(Down::Bye(ByeReason::TimedOut));
        self.log.push(LogRecord::TimedOut {
            t: now,
            slot: i as u8,
        });
        self.inject_surrender(i);
        self.check_ended(now);
    }

    /// Closes `next_turn`: takes each slot's due commands round-robin, one
    /// at a time, within the per-player and per-turn limits; the rest wait
    /// for the next turn (a restamp).
    fn close_turn(&mut self, now: u64) {
        let tick = self.next_turn;
        let mut cmds: Vec<BundledCmd> = Vec::new();
        let mut used = 0usize;
        let mut per = [(0usize, 0usize); SLOTS];
        loop {
            let mut took = false;
            // Alternate who goes first, so neither player's large command
            // waits behind the other's stream.
            for k in 0..SLOTS {
                let i = (k + tick as usize) % SLOTS;
                let Some(s) = self.slots[i].as_mut() else {
                    continue;
                };
                let Some(p) = s.pending.front() else {
                    continue;
                };
                let (n, bytes) = per[i];
                // Per-command overhead in the bundle: player, seq, target.
                let cost = p.size + 16;
                // The first command of a turn always fits (the wire limits
                // keep any one command far below the turn budget), so a
                // large command can never block its player's queue.
                let first = used == 0;
                // The quota is 16; a relay-injected `Surrender` may be a
                // 17th.
                if p.eff > tick
                    || (n >= MAX_CMDS_PER_TURN
                        && (n > MAX_CMDS_PER_TURN || p.kind != CommandKind::Surrender))
                    || (!first
                        && (bytes + p.size > MAX_TURN_BYTES
                            || used + cost > MAX_CLOSED_TURN_BYTES - 24))
                {
                    continue;
                }
                let p = s.pending.pop_front().expect("front exists");
                if p.target != tick {
                    s.restamps += 1;
                    self.restamps += 1;
                }
                per[i] = (n + 1, bytes + p.size);
                used += cost;
                cmds.push(BundledCmd {
                    cmd: Command {
                        player: PlayerId(i as u8),
                        seq: p.seq,
                        kind: p.kind,
                    },
                    target: p.target,
                });
                took = true;
            }
            if !took {
                break;
            }
        }
        cmds.sort_by_key(|c| (c.cmd.player, c.cmd.seq));
        self.commands += cmds.len() as u64;
        if self.log_turns && !cmds.is_empty() {
            self.log.push(LogRecord::Turn {
                t: now,
                tick,
                cmds: cmds
                    .iter()
                    .map(|c| {
                        (
                            c.cmd.player.0,
                            c.cmd.seq,
                            c.target,
                            format!("{:?}", c.cmd.kind),
                        )
                    })
                    .collect(),
            });
        }
        let turn = Turn {
            tick,
            salt: 0,
            cmds,
        };
        let bytes = postcard::to_allocvec(&turn).expect("encoding a turn cannot fail");
        self.turns.push_back(bytes);
        self.next_turn += 1;
    }

    /// Drops closed turns every active slot holds.
    fn prune_turns(&mut self) {
        let min_held = self
            .slots
            .iter()
            .flatten()
            .filter(|s| s.active())
            .map(|s| s.ack.held)
            .min()
            .unwrap_or(self.next_turn);
        while self.turns_base < min_held && !self.turns.is_empty() {
            self.turns.pop_front();
            self.turns_base += 1;
        }
    }

    fn send_bundles(&mut self, i: usize, now: u64) {
        let period = self.schedule.period();
        let peers = self.peers(now);
        let confirmed = self.confirmed;
        let Some(s) = self.slots[i].as_mut() else {
            return;
        };
        if !s.active() {
            return;
        }
        let resend_after = if s.rtt_p99_us == 0 {
            200_000
        } else {
            (s.rtt_p99_us as u64 + period / 2).clamp(period, 1_000_000)
        };
        if now.saturating_sub(s.burst_start) >= period {
            s.burst_start = now;
            s.burst_count = 0;
        }
        let first = s.ack.held.max(self.turns_base);
        let mut t = first;
        while s.burst_count < MAX_BURST && t < self.next_turn {
            // One datagram: header, confirmed, peers, turn count, turns.
            let head = postcard::to_extend(&confirmed, Vec::with_capacity(64))
                .and_then(|h| postcard::to_extend(&peers, h))
                .expect("encoding a bundle head cannot fail");
            let mut body: Vec<u8> = Vec::new();
            let mut n = 0u64;
            while t < self.next_turn && n < MAX_BUNDLE_TURNS as u64 {
                let due = !s.ack.holds(t)
                    && s.sent
                        .get(&t)
                        .is_none_or(|&at| now.saturating_sub(at) >= resend_after);
                if due {
                    let tb = &self.turns[(t - self.turns_base) as usize];
                    // Room for the header, head, and a 2-byte count.
                    if HEADER_LEN + head.len() + 3 + body.len() + tb.len() > MAX_DATAGRAM {
                        break;
                    }
                    body.extend_from_slice(tb);
                    s.sent.insert(t, now);
                    n += 1;
                }
                t += 1;
            }
            if n == 0 {
                break;
            }
            self.buf.clear();
            Header {
                version: PROTOCOL_VERSION,
                kind: Kind::Bundle as u8,
                token: s.token,
            }
            .write_to(&mut self.buf);
            self.buf.extend_from_slice(&head);
            push_varint(&mut self.buf, n);
            self.buf.extend_from_slice(&body);
            self.out.push((s.addr, self.buf.clone()));
            s.burst_count += 1;
        }
    }

    fn peers(&self, now: u64) -> [PeerStat; 2] {
        let stat = |s: &Option<Slot>| match s {
            Some(s) => PeerStat {
                rtt_ms: (s.rtt_p99_us / 1000).min(u16::MAX as u32) as u16,
                unstable: s.active() && now.saturating_sub(s.last_heard) >= self.cfg.unstable_us,
                gone: !s.active(),
            },
            None => PeerStat {
                gone: true,
                ..PeerStat::default()
            },
        };
        [stat(&self.slots[0]), stat(&self.slots[1])]
    }

    // ------------------------------------------------------------ output --

    fn broadcast(&mut self, msg: Down) {
        for s in self.slots.iter_mut().flatten() {
            if s.state != SlotState::TimedOut {
                s.ctl_out.push(msg.clone());
            }
        }
    }

    fn flush_ctl(&mut self, now: u64) {
        for i in 0..SLOTS {
            let Some(s) = self.slots[i].as_mut() else {
                continue;
            };
            let due = s.ctl_out.due(now);
            let (addr, token) = (s.addr, s.token);
            for (cseq, msg) in due {
                self.send(addr, token, &Packet::CtlDown(CtlDown { cseq, msg }));
            }
        }
    }

    fn send(&mut self, to: SocketAddr, token: u64, p: &Packet) {
        if encode(token, p, &mut self.buf).is_ok() {
            self.out.push((to, self.buf.clone()));
        }
    }
}
