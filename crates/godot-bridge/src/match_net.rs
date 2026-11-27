//! Net mode on the sim thread (S2 "Threads", "Client pacing", "Hash check
//! and desync dump").
//!
//! In a net match the sim thread does not decide what runs: the relay
//! does. [`NetStepper::poll`] reads the net thread's events (bundles,
//! pauses, a desync, a goodbye) and steps tick `T` at its scheduled slot
//! once bundle `T` is in, never past the newest bundle, catching up at most
//! three ticks per period ([`SimDriver`] from `frontline-net`). It sends
//! every published hash to the net thread, keeps the 300-tick checkpoint
//! the relay confirmed plus the bundles since, and a ring of `state_hash`;
//! on a desync it writes the dump once the world reached the pause turn.
//!
//! The stepper never reads a socket or the wall clock: the link
//! ([`LinkIo`]) gives it the time and the events, so tests drive it on a
//! virtual clock with a fake link. [`NetLink`] is the real one: channels to
//! the `frontline-net` thread and its status snapshot (the slot schedule).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Instant;

use frontline_net::client::{ClientStatus, NetEvent};
use frontline_net::client_io::{StatusReader, ToNet};
use frontline_net::dump::{DesyncRecorder, DumpManifest};
use frontline_net::harness::SimDriver;
use frontline_net::protocol::{ByeReason, PauseCause};
use frontline_sim::{Command, Outcome, World};

use crate::match_thread::MatchCounters;

/// Relay silence before the link shows as unstable, as "reconnecting", and
/// as lost for good (the relay pauses at 10 s and surrenders a silent
/// player 60 s later; S2 "Disconnect").
const UNSTABLE_US: u64 = 2_000_000;
const RECONNECTING_US: u64 = 10_000_000;
const RELAY_TIMEOUT_US: u64 = 70_000_000;
const LOST_US: u64 = 75_000_000;
/// The relay's wait for a silent player after it paused.
const WAIT_US: u64 = 60_000_000;
/// Published hashes kept for tests and the HUD (about 34 minutes).
pub const HASH_KEEP: usize = 4096;
/// S18 H5: allies' map pings waiting for the presenter, at most.
pub const MAX_PINGS: usize = 16;

/// What the sim thread reads from and sends to the net thread. Tests use a
/// fake with a virtual clock.
pub trait LinkIo {
    /// Local time (µs) on the clock the step slots are in.
    fn now_us(&self) -> u64;
    /// The newest snapshot: schedule, clock offset, D, ping, confirmed.
    fn status(&self) -> ClientStatus;
    fn next_event(&mut self) -> Option<NetEvent>;
    fn send(&self, m: ToNet);
}

/// The real link: the sim thread's half of a `NetHandle`.
pub struct NetLink {
    /// Bundles, `Start`, `Paused`, `Resume`, `Desync`, `Bye`
    /// (`NetHandle::take_match_events`).
    pub events: Receiver<NetEvent>,
    pub to_net: Sender<ToNet>,
    pub status: StatusReader,
    /// The instant the net thread's local time 0 stands for.
    pub origin: Instant,
}

impl LinkIo for NetLink {
    fn now_us(&self) -> u64 {
        self.origin.elapsed().as_micros() as u64
    }

    fn status(&self) -> ClientStatus {
        self.status.get()
    }

    fn next_event(&mut self) -> Option<NetEvent> {
        self.events.try_recv().ok()
    }

    fn send(&self, m: ToNet) {
        // The net thread outlives the match; if it is gone, so is the link.
        let _ = self.to_net.send(m);
    }
}

/// What both clients know about the match, for the dump and the HUD.
#[derive(Clone, Debug, Default)]
pub struct NetMatchInfo {
    pub d: u8,
    /// Same on both clients: `<seed>-<8 hex digits>`.
    pub match_id: String,
    /// One per slot (an AI slot's is empty).
    pub names: Vec<String>,
    pub build_id: String,
    pub content_hash: u64,
    pub setup_hash: u64,
    pub seed: u64,
    /// Godot's `user://` as an OS path. Dumps go to
    /// `<user_dir>/desync/<match_id>/slot<N>/`; `None` writes none.
    pub user_dir: Option<PathBuf>,
    /// The host's relay log, copied next to the dump as `relay.jsonl`.
    pub relay_log: Option<PathBuf>,
    /// Dev hook (`--debug_inject_desync TICK`): step this tick with one
    /// extra local-only `StopUnits` for an own soldier.
    pub inject_desync: Option<u64>,
}

/// The link state the HUD shows (`net.state`).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum NetState {
    /// Not a net match.
    #[default]
    Local = 0,
    Ok = 1,
    /// A player (maybe this one) has not been heard from for 2 s.
    Unstable = 2,
    /// The relay waits for a silent player (or this client hears nothing).
    Waiting = 3,
    /// A player paused.
    Paused = 4,
    Desync = 5,
    /// The relay said goodbye; nothing more will come.
    Ended = 6,
}

impl NetState {
    pub fn from_u8(v: u8) -> NetState {
        match v {
            1 => NetState::Ok,
            2 => NetState::Unstable,
            3 => NetState::Waiting,
            4 => NetState::Paused,
            5 => NetState::Desync,
            6 => NetState::Ended,
            _ => NetState::Local,
        }
    }

    /// `net.state` in `get_state()` (an ended link reads "ok": the
    /// disconnect reason carries it).
    pub fn name(self) -> &'static str {
        match self {
            NetState::Local | NetState::Ok | NetState::Ended => "ok",
            NetState::Unstable => "unstable",
            NetState::Waiting => "waiting",
            NetState::Paused => "paused",
            NetState::Desync => "desync",
        }
    }
}

/// The parts of the net readout that are not plain counters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetDetail {
    pub paused_by: Option<u8>,
    pub waiting_for: Option<u8>,
    pub waiting_s_left: f32,
    /// The dump folder once written, or why it could not be.
    pub desync_dump: Option<String>,
    pub dump_error: Option<String>,
    /// Reason code once the link is gone for good (`timed_out`, `flood`,
    /// `relay_closed`, `connection_lost`).
    pub disconnect: Option<&'static str>,
}

/// One tick the stepper ran.
#[derive(Clone, Debug)]
pub struct Stepped {
    /// The world's tick before the step (the bundle's tick).
    pub tick: u64,
    /// Local µs the tick was due: the frame's timestamp.
    pub slot_us: u64,
    pub step_us: u32,
    /// The bundle (every player's commands, without the dev injection).
    pub cmds: Vec<Command>,
}

/// Steps a net match world. See the module docs.
pub struct NetStepper {
    pub info: NetMatchInfo,
    slot: u8,
    driver: SimDriver,
    pause: Option<(PauseCause, u64)>,
    desync: Option<(u64, Vec<Option<u64>>, u64)>,
    bye: Option<ByeReason>,
    said_bye: bool,
    dumped: bool,
    own_executed: u64,
    status: ClientStatus,
}

impl NetStepper {
    pub fn new(slot: u8, info: NetMatchInfo) -> NetStepper {
        let mut driver = SimDriver::new(false);
        driver.recorder = Some(DesyncRecorder::new());
        driver.inject_desync = info.inject_desync.map(|t| (t, slot));
        NetStepper {
            info,
            slot,
            driver,
            pause: None,
            desync: None,
            bye: None,
            said_bye: false,
            dumped: false,
            own_executed: 0,
            status: ClientStatus::default(),
        }
    }

    /// The newest status the stepper read.
    pub fn status(&self) -> &ClientStatus {
        &self.status
    }

    /// Local µs tick `tick` is due, if the match started.
    pub fn slot_of(&self, tick: u64) -> Option<u64> {
        self.status.step_slot(tick)
    }

    /// True while nothing more can be stepped for now: paused, desynced,
    /// or the relay left and every bundle it sent has run.
    pub fn halted(&self, world: &World) -> bool {
        self.driver.halted_at().is_some_and(|p| world.tick() >= p)
            || (self.bye.is_some() && self.driver.backlog() == 0)
    }

    /// Bundles received but not run yet.
    pub fn backlog(&self) -> usize {
        self.driver.backlog()
    }

    /// Reads the link, steps at most one due tick, and updates the
    /// counters. Returns the tick it stepped, if any.
    pub fn poll(
        &mut self,
        world: &mut World,
        io: &mut impl LinkIo,
        counters: &MatchCounters,
    ) -> Option<Stepped> {
        let now = io.now_us();
        self.status = io.status();
        while let Some(ev) = io.next_event() {
            self.driver.on_event(&ev);
            match ev {
                NetEvent::Paused { cause, .. } => self.pause = Some((cause, now)),
                NetEvent::Resume { .. } => self.pause = None,
                NetEvent::Desync { tick, hashes, at } => self.desync = Some((tick, hashes, at)),
                // S18 H5: an ally's map ping, for the presenter.
                NetEvent::MapPing { slot, x, y } => {
                    if let Ok(mut q) = counters.pings.lock() {
                        if q.len() == MAX_PINGS {
                            q.pop_front();
                        }
                        q.push_back((slot, x, y));
                    }
                }
                NetEvent::Bye(r) => self.bye = Some(r),
                _ => {}
            }
        }
        self.driver.confirm(self.status.confirmed);
        let running = world.game().is_none_or(|g| g.outcome() == Outcome::Running);
        // The match is over: say so once (the relay ends the match once
        // both did) and stay on the last tick.
        if !running && !self.said_bye {
            self.said_bye = true;
            io.send(ToNet::Bye(ByeReason::MatchOver));
        }
        let mut stepped = None;
        if running {
            let next = self.driver.next_bundle().map(|(t, c)| (t, c.to_vec()));
            let period = self.status.schedule.map_or(33_333, |s| s.period());
            let status = &self.status;
            let mut hashes: Vec<(u64, u64)> = Vec::new();
            let t0 = diag_now();
            let n = self.driver.advance(
                world,
                now,
                period,
                |t| status.step_slot(t),
                |tick, hash| hashes.push((tick, hash)),
            );
            let step_us = t0.elapsed().as_micros().min(u32::MAX as u128) as u32;
            for &(tick, hash) in &hashes {
                io.send(ToNet::Hash { tick, hash });
            }
            if !hashes.is_empty()
                && let Ok(mut h) = counters.hashes.lock()
            {
                for &x in &hashes {
                    if h.len() == HASH_KEEP {
                        h.pop_front();
                    }
                    h.push_back(x);
                }
            }
            if n > 0
                && let Some((tick, cmds)) = next
            {
                for c in cmds.iter().filter(|c| c.player.0 == self.slot) {
                    self.own_executed = self.own_executed.max(c.seq as u64 + 1);
                }
                stepped = Some(Stepped {
                    tick,
                    slot_us: self.status.step_slot(tick).unwrap_or(now),
                    step_us,
                    cmds,
                });
            }
        }
        if !self.dumped
            && let Some((tick, hashes, at)) = self.desync.clone()
            && world.tick() >= at
        {
            self.dumped = true;
            self.write_dump(world, tick, &hashes, counters);
        }
        self.publish(world, now, counters);
        stepped
    }

    fn write_dump(
        &self,
        world: &World,
        tick: u64,
        hashes: &[Option<u64>],
        counters: &MatchCounters,
    ) {
        let Some(base) = &self.info.user_dir else {
            return;
        };
        let match_dir = base.join("desync").join(&self.info.match_id);
        let dir = match_dir.join(format!("slot{}", self.slot));
        let manifest = DumpManifest {
            build_id: self.info.build_id.clone(),
            content_hash: format!("{:016x}", self.info.content_hash),
            setup_hash: format!("{:016x}", self.info.setup_hash),
            seed: self.info.seed,
            d: self.info.d,
            slot: self.slot,
            desync_tick: tick,
            hashes: hashes
                .iter()
                .map(|h| h.map(|h| format!("{h:016x}")))
                .collect(),
            ..DumpManifest::default()
        };
        let Some(rec) = self.driver.recorder.as_ref() else {
            return;
        };
        let res = rec
            .write_dump(&dir, world, manifest, &self.driver.ring)
            .and_then(|()| copy_relay_log(self.info.relay_log.as_deref(), &match_dir));
        if let Ok(mut d) = counters.net.lock() {
            match res {
                Ok(()) => d.desync_dump = Some(dir.to_string_lossy().into_owned()),
                Err(e) => d.dump_error = Some(format!("{}: {e}", dir.display())),
            }
        }
    }

    /// Counters and the readout for the presenter.
    fn publish(&self, world: &World, now: u64, counters: &MatchCounters) {
        let s = &self.status;
        let mut detail = NetDetail::default();
        let state = if let Some(r) = self.bye {
            // A goodbye only matters once every bundle before it ran and
            // the match is still on (an opponent who quit surrendered,
            // which the end screen shows).
            if self.driver.backlog() == 0
                && world.game().is_none_or(|g| g.outcome() == Outcome::Running)
            {
                detail.disconnect = Some(bye_code(r));
            }
            NetState::Ended
        } else if self.desync.is_some() {
            NetState::Desync
        } else if world
            .game()
            .is_some_and(|g| g.outcome() != Outcome::Running)
        {
            // Over: the other side says goodbye and goes quiet, which is
            // not an unstable link.
            NetState::Ended
        } else if let Some((cause, since)) = self.pause {
            match cause {
                PauseCause::Player(p) => {
                    detail.paused_by = Some(p);
                    NetState::Paused
                }
                PauseCause::Waiting(p) => {
                    detail.waiting_for = Some(p);
                    detail.waiting_s_left =
                        WAIT_US.saturating_sub(now.saturating_sub(since)) as f32 / 1e6;
                    NetState::Waiting
                }
            }
        } else if s.schedule.is_some() && s.silent_us >= LOST_US {
            detail.disconnect = Some("connection_lost");
            NetState::Ended
        } else if s.schedule.is_some() && s.silent_us >= RECONNECTING_US {
            // We hear nothing: the relay is (or soon will be) waiting for
            // us, but cannot tell us so.
            detail.waiting_for = Some(self.slot);
            detail.waiting_s_left = RELAY_TIMEOUT_US.saturating_sub(s.silent_us) as f32 / 1e6;
            NetState::Waiting
        } else if s.silent_us >= UNSTABLE_US || s.peers.iter().any(|p| p.unstable && !p.gone) {
            NetState::Unstable
        } else {
            NetState::Ok
        };
        counters.net_state.store(state as u8, Ordering::Relaxed);
        counters
            .rtt_ms
            .store(s.rtt_ms.map_or(-1, |r| r as i32), Ordering::Relaxed);
        counters.input_delay.store(
            if s.d > 0 { s.d } else { self.info.d } as u32,
            Ordering::Relaxed,
        );
        counters
            .playout
            .store(s.playout.max(1) as u32, Ordering::Relaxed);
        counters.restamps.store(s.own_restamps, Ordering::Relaxed);
        counters
            .stall_ticks
            .store(self.driver.stall_ticks, Ordering::Relaxed);
        counters.desync_tick.store(
            self.desync.as_ref().map_or(-1, |(t, _, _)| *t as i64),
            Ordering::Relaxed,
        );
        counters
            .own_executed
            .store(self.own_executed, Ordering::Relaxed);
        counters.paused.store(self.halted(world), Ordering::Relaxed);
        if let Ok(mut d) = counters.net.lock() {
            detail.desync_dump = d.desync_dump.take();
            detail.dump_error = d.dump_error.take();
            *d = detail;
        }
    }
}

/// `Bye` reason → the presenter's reason code (`godot/net/net_ui.gd`).
pub fn bye_code(r: ByeReason) -> &'static str {
    match r {
        ByeReason::TimedOut => "timed_out",
        ByeReason::Flood => "flood",
        ByeReason::Shutdown => "relay_closed",
        ByeReason::Quit | ByeReason::MatchOver => "left",
    }
}

/// The host's relay log next to the slot dumps (the relay keeps writing
/// it; this is the state at the pause turn).
fn copy_relay_log(log: Option<&Path>, match_dir: &Path) -> std::io::Result<()> {
    match log {
        Some(src) if src.is_file() => std::fs::copy(src, match_dir.join("relay.jsonl")).map(|_| ()),
        _ => Ok(()),
    }
}

/// A stage timer (diagnostics only, never a sim input).
#[allow(clippy::disallowed_methods)]
fn diag_now() -> Instant {
    Instant::now()
}

/// The published hashes as a list, oldest first (tests compare clients).
pub fn hashes(counters: &MatchCounters) -> VecDeque<(u64, u64)> {
    counters
        .hashes
        .lock()
        .map(|h| h.clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use frontline_net::Schedule;
    use frontline_net::protocol::PeerStat;
    use frontline_sim::{CommandKind, PlayerId};

    use super::*;
    use crate::match_frame::tests_support::test_world;

    /// A link on a virtual clock: the test sets the time and the status
    /// and queues the events; what the stepper sends is kept.
    struct FakeLink {
        now: u64,
        status: ClientStatus,
        events: VecDeque<NetEvent>,
        sent: RefCell<Vec<ToNet>>,
    }

    impl LinkIo for FakeLink {
        fn now_us(&self) -> u64 {
            self.now
        }
        fn status(&self) -> ClientStatus {
            self.status.clone()
        }
        fn next_event(&mut self) -> Option<NetEvent> {
            self.events.pop_front()
        }
        fn send(&self, m: ToNet) {
            self.sent.borrow_mut().push(m);
        }
    }

    const START: u64 = 1_000_000;

    fn link() -> FakeLink {
        let status = ClientStatus {
            d: 3,
            schedule: Some(Schedule::new(START, 30, 100)),
            offset_us: Some(0),
            ..ClientStatus::default()
        };
        FakeLink {
            now: 0,
            status,
            events: VecDeque::new(),
            sent: RefCell::new(Vec::new()),
        }
    }

    fn bundles(l: &mut FakeLink, ticks: std::ops::Range<u64>) {
        for tick in ticks {
            l.events.push_back(NetEvent::Bundle {
                tick,
                cmds: Vec::new(),
            });
        }
    }

    fn info() -> NetMatchInfo {
        NetMatchInfo {
            d: 3,
            match_id: "7-0000abcd".into(),
            names: vec!["a".into(), "b".into()],
            build_id: "test".into(),
            seed: 7,
            ..NetMatchInfo::default()
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("frontline-bridge-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    /// Steps at slots, never past the newest bundle, catches up at most
    /// three ticks per period, and stamps each tick with its slot.
    #[test]
    fn paces_ticks_on_the_slot_schedule() {
        let mut w = test_world(false);
        let mut l = link();
        let c = MatchCounters::default();
        let mut st = NetStepper::new(0, info());
        bundles(&mut l, 0..11);
        let sched = l.status.clone();
        let slot = |t: u64| sched.step_slot(t).unwrap();
        // Tick 0 is due one period after turn 0 closes.
        assert_eq!(slot(0), START + 2 * 33_333);
        l.now = slot(0) - 1;
        assert!(st.poll(&mut w, &mut l, &c).is_none());
        assert_eq!(w.tick(), 0);
        l.now = slot(0);
        let s = st.poll(&mut w, &mut l, &c).expect("tick 0 is due");
        assert_eq!((s.tick, s.slot_us), (0, slot(0)));
        assert_eq!(w.tick(), 1);
        assert!(st.poll(&mut w, &mut l, &c).is_none(), "tick 1 is not due");
        // Late: everything up to tick 9 is due. Catch up at <= 3 per period.
        l.now = slot(9);
        let mut stepped = vec![];
        let end = l.now + 4 * 33_333;
        while l.now < end {
            if let Some(s) = st.poll(&mut w, &mut l, &c) {
                assert_eq!(s.slot_us, slot(s.tick), "frames carry the slot");
                stepped.push((l.now, s.tick));
            }
            l.now += 1_000;
        }
        let ticks: Vec<u64> = stepped.iter().map(|s| s.1).collect();
        assert_eq!(ticks, (1..=10).collect::<Vec<_>>());
        for win in stepped.windows(2) {
            assert!(win[1].0 - win[0].0 >= 33_333 / 3, "{stepped:?}");
        }
        // Bundle 10 was the newest: nothing runs past it however late.
        l.now += 10_000_000;
        assert!(st.poll(&mut w, &mut l, &c).is_none());
        assert_eq!(w.tick(), 11);
        assert!(c.stall_ticks.load(Ordering::Relaxed) >= 8);
        assert_eq!(st.backlog(), 0);
    }

    #[test]
    fn pause_resume_and_readout() {
        let mut w = test_world(false);
        let mut l = link();
        let c = MatchCounters::default();
        let mut st = NetStepper::new(1, info());
        bundles(&mut l, 0..20);
        l.status.rtt_ms = Some(41);
        l.status.own_restamps = 2;
        l.events.push_back(NetEvent::Paused {
            at: 5,
            cause: PauseCause::Player(0),
        });
        l.now = l.status.step_slot(19).unwrap();
        for _ in 0..200 {
            st.poll(&mut w, &mut l, &c);
            l.now += 1_000;
        }
        assert_eq!(w.tick(), 5, "stops before the pause turn");
        assert!(st.halted(&w) && c.paused.load(Ordering::Relaxed));
        assert_eq!(
            NetState::from_u8(c.net_state.load(Ordering::Relaxed)),
            NetState::Paused
        );
        assert_eq!(c.net.lock().unwrap().paused_by, Some(0));
        assert_eq!(c.rtt_ms.load(Ordering::Relaxed), 41);
        assert_eq!(c.input_delay.load(Ordering::Relaxed), 3);
        assert_eq!(c.restamps.load(Ordering::Relaxed), 2);
        // The relay waits for slot 0 now: 60 s, counting down.
        l.events.push_back(NetEvent::Paused {
            at: 5,
            cause: PauseCause::Waiting(0),
        });
        st.poll(&mut w, &mut l, &c);
        l.now += 20_000_000;
        st.poll(&mut w, &mut l, &c);
        {
            let d = c.net.lock().unwrap();
            assert_eq!(d.waiting_for, Some(0));
            assert!(
                (d.waiting_s_left - 40.0).abs() < 0.01,
                "{}",
                d.waiting_s_left
            );
        }
        // Resume: turn 5 closes later; the client rebases its schedule.
        let close_at = l.now + 300_000;
        l.events.push_back(NetEvent::Resume { at: 5, close_at });
        if let Some(s) = l.status.schedule.as_mut() {
            s.rebase(5, close_at);
        }
        let until = l.now + 2_000_000;
        while l.now < until {
            st.poll(&mut w, &mut l, &c);
            l.now += 1_000;
        }
        assert_eq!(w.tick(), 20);
        assert!(!c.paused.load(Ordering::Relaxed));
        assert_eq!(
            NetState::from_u8(c.net_state.load(Ordering::Relaxed)),
            NetState::Ok
        );
        // A peer silent for 2 s shows; a silent relay is "reconnecting",
        // then lost.
        l.status.peers = vec![
            PeerStat {
                rtt_ms: 40,
                unstable: true,
                gone: false,
            },
            PeerStat::default(),
        ];
        st.poll(&mut w, &mut l, &c);
        assert_eq!(
            NetState::from_u8(c.net_state.load(Ordering::Relaxed)),
            NetState::Unstable
        );
        l.status.silent_us = 12_000_000;
        st.poll(&mut w, &mut l, &c);
        assert_eq!(c.net.lock().unwrap().waiting_for, Some(1));
        l.status.silent_us = 80_000_000;
        st.poll(&mut w, &mut l, &c);
        assert_eq!(c.net.lock().unwrap().disconnect, Some("connection_lost"));
    }

    /// Hashes go to the relay; the confirmed checkpoint and the bundles
    /// since make the dump, written once the world reaches the pause turn.
    #[test]
    fn confirmed_checkpoint_and_desync_dump() {
        let dir = temp_dir("dump");
        let log = dir.join("relay_last.jsonl");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&log, "{\"ev\":\"ended\",\"t\":1}\n").unwrap();
        let mut w = test_world(false);
        let mut l = link();
        let c = MatchCounters::default();
        let mut inf = info();
        inf.user_dir = Some(dir.clone());
        inf.relay_log = Some(log);
        let mut st = NetStepper::new(0, inf);
        bundles(&mut l, 0..340);
        // An own command at tick 2 (seq 0) and one of the other player.
        if let Some(NetEvent::Bundle { cmds, .. }) = l.events.get_mut(2) {
            cmds.push(Command {
                player: PlayerId(0),
                seq: 0,
                kind: CommandKind::StopUnits { units: vec![] },
            });
            cmds.push(Command {
                player: PlayerId(1),
                seq: 0,
                kind: CommandKind::StopUnits { units: vec![] },
            });
        }
        l.now = l.status.step_slot(0).unwrap();
        let mut ran = 0;
        while w.tick() < 310 {
            if st.poll(&mut w, &mut l, &c).is_some() {
                ran += 1;
            }
            l.now += 33_334;
        }
        assert_eq!(ran, 310);
        assert_eq!(c.own_executed.load(Ordering::Relaxed), 1);
        let sent: Vec<u64> = l
            .sent
            .borrow()
            .iter()
            .filter_map(|m| match m {
                ToNet::Hash { tick, .. } => Some(*tick),
                _ => None,
            })
            .collect();
        assert_eq!(sent, (1..=10).map(|k| k * 30).collect::<Vec<_>>());
        assert_eq!(hashes(&c).len(), 10);
        // The relay confirms 300: the checkpoint at 300 becomes the one.
        l.status.confirmed = 300;
        l.events.push_back(NetEvent::Desync {
            tick: 330,
            hashes: vec![Some(1), Some(2)],
            at: 335,
        });
        for _ in 0..200 {
            st.poll(&mut w, &mut l, &c);
            l.now += 33_334;
        }
        assert_eq!(w.tick(), 335, "stops at the desync pause turn");
        assert_eq!(c.desync_tick.load(Ordering::Relaxed), 330);
        assert_eq!(
            NetState::from_u8(c.net_state.load(Ordering::Relaxed)),
            NetState::Desync
        );
        // Joined per component: the dump path is compared as a string, and
        // on Windows a "/" inside one `join` would not match its "\\".
        let slot = dir.join("desync").join("7-0000abcd").join("slot0");
        assert_eq!(
            c.net.lock().unwrap().desync_dump.as_deref(),
            Some(slot.to_str().unwrap())
        );
        for f in [
            "manifest.json",
            "checkpoint.sav",
            "paused.sav",
            "bundles.jsonl",
        ] {
            assert!(slot.join(f).is_file(), "{f} missing");
        }
        assert!(dir.join("desync/7-0000abcd/relay.jsonl").is_file());
        let m: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(slot.join("manifest.json")).unwrap())
                .unwrap();
        assert_eq!(m["checkpoint_tick"], 300);
        assert_eq!(m["paused_tick"], 335);
        assert_eq!(m["desync_tick"], 330);
        assert_eq!(m["hash_ring"].as_array().unwrap().len(), 300);
        let lines = std::fs::read_to_string(slot.join("bundles.jsonl")).unwrap();
        assert_eq!(lines.lines().count(), 35, "bundles 300..335");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The dev hook makes this client's world diverge; a finished match
    /// says `Bye(MatchOver)` once and stops; a relay goodbye shows once
    /// every bundle ran.
    #[test]
    fn desync_hook_match_over_and_bye() {
        let run = |inject: Option<u64>| {
            let mut w = test_world(false);
            let mut l = link();
            let c = MatchCounters::default();
            let mut inf = info();
            inf.inject_desync = inject;
            let mut st = NetStepper::new(0, inf);
            bundles(&mut l, 0..8);
            l.now = l.status.step_slot(7).unwrap() + 1_000_000;
            for _ in 0..100 {
                st.poll(&mut w, &mut l, &c);
                l.now += 12_000;
            }
            assert_eq!(w.tick(), 8);
            w.state_hash()
        };
        assert_ne!(run(None), run(Some(4)));

        let mut w = test_world(false);
        let mut l = link();
        let c = MatchCounters::default();
        let mut st = NetStepper::new(0, info());
        bundles(&mut l, 0..4);
        l.events.push_back(NetEvent::Bundle {
            tick: 4,
            cmds: vec![Command {
                player: PlayerId(0),
                seq: 0,
                kind: CommandKind::Surrender,
            }],
        });
        bundles(&mut l, 5..10);
        l.events.push_back(NetEvent::Bye(ByeReason::Shutdown));
        l.now = l.status.step_slot(9).unwrap() + 1_000_000;
        for _ in 0..100 {
            st.poll(&mut w, &mut l, &c);
            l.now += 12_000;
        }
        let byes = l
            .sent
            .borrow()
            .iter()
            .filter(|m| matches!(m, ToNet::Bye(ByeReason::MatchOver)))
            .count();
        assert_eq!(byes, 1);
        assert_eq!(w.tick(), 5, "a finished match stays on its last tick");
        // The relay's goodbye after a finished match is not a disconnect.
        assert_eq!(c.net.lock().unwrap().disconnect, None);

        let mut w = test_world(false);
        let mut l = link();
        let mut st = NetStepper::new(0, info());
        bundles(&mut l, 0..3);
        l.events.push_back(NetEvent::Bye(ByeReason::TimedOut));
        l.now = l.status.step_slot(0).unwrap();
        st.poll(&mut w, &mut l, &c);
        assert_eq!(c.net.lock().unwrap().disconnect, None, "bundles left");
        for _ in 0..20 {
            l.now += 33_334;
            st.poll(&mut w, &mut l, &c);
        }
        assert_eq!(w.tick(), 3);
        assert!(st.halted(&w));
        assert_eq!(c.net.lock().unwrap().disconnect, Some("timed_out"));
    }
}
