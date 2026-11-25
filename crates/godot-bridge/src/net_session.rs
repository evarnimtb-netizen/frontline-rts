//! The multiplayer lobby on the presenter side (S2 "Session setup"), as
//! plain Rust behind the `FrontlineNet` Godot class ([`crate::net_node`]).
//!
//! A [`NetSession`] hosts (an in-process relay on its own thread, which it
//! then joins over loopback) or joins a relay by address. It owns the
//! client's `frontline-net` thread; the Godot main thread only reads
//! snapshots and sends small messages, so it never waits on the network.
//! Host names are resolved on a short-lived thread.
//!
//! S18: a lobby holds two to four slots (the host picks how many); each
//! player picks a team, the host may fill open slots with AI and a joiner
//! may move to another open slot.
//!
//! When the relay starts the match, [`NetSession::take_match`] resolves the
//! [`MatchSetup`] from `Start` (its slots, human or AI) and hands out the sim
//! thread's half of the link ([`NetLink`]) for `MatchHandle::spawn_net`.
//! The match events are split off the net thread as soon as it starts, so
//! no bundle is lost while the match scene loads.

use std::net::{SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use frontline_content::{MatchSetup, ModDefs};
use frontline_net::client::{ClientConfig, ClientStatus, NetEvent};
use frontline_net::client_io::{NetHandle, ToNet};
use frontline_net::protocol::{
    AiPick, LobbyState, MAX_D, MAX_DIFFICULTY, MAX_NAME, MAX_SLOTS, MIN_D, RejectReason, SlotInfo,
    Start,
};
use frontline_net::relay::RelayConfig;
use frontline_net::rng::OsRandom;
use frontline_net::server::{LogSink, RelayServer};

use crate::match_net::{NetLink, NetMatchInfo, bye_code};
use crate::match_setup::{FACTIONS, MOD_ID, MenuChoice, SlotChoice, resolve_with, short_id};

/// No `Welcome` within this long: nobody answers at that address.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Relay silence in the lobby before the link counts as lost.
const LOBBY_SILENCE_US: u64 = 10_000_000;
pub const DEFAULT_NAME: &str = "Player";

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Phase {
    Idle,
    /// Looking up a host name.
    Resolving,
    Connecting,
    Lobby,
    /// `Start` arrived: the countdown, then the match.
    Started,
    Disconnected,
}

/// The lobby choice this client last sent. A joiner's seed, map, D, size
/// and AI fill are ignored by the relay; slot 0's are the match's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    /// Short ids (`germany`, `kessel_ridge`).
    pub faction: String,
    pub seed: u64,
    pub map: String,
    /// `None`: the relay's suggestion.
    pub d: Option<u8>,
    /// S18: own team (`None`: alone, as in a 1v1).
    pub team: Option<u8>,
    /// S18, host: slots in the match (2 to 4).
    pub players: u8,
    /// S18, host: the AI fill (full faction ids).
    pub ai: Vec<AiPick>,
}

impl Default for Choice {
    fn default() -> Choice {
        Choice {
            faction: String::new(),
            seed: 1,
            map: "kessel_ridge".into(),
            d: None,
            team: None,
            players: 2,
            ai: Vec::new(),
        }
    }
}

/// A partial [`Choice`] (`FrontlineNet.set_choice`): `None` keeps a value.
#[derive(Clone, Debug, Default)]
pub struct ChoiceUpdate {
    pub faction: Option<String>,
    pub seed: Option<u64>,
    pub map: Option<String>,
    pub d: Option<u8>,
    /// S18: `Some(None)` clears the team.
    pub team: Option<Option<u8>>,
    pub players: Option<u8>,
    /// S18: the AI fill, short faction ids (replaces the list).
    pub ai: Option<Vec<AiPick>>,
}

/// One lobby slot as the presenter shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlotView {
    /// Empty: an open slot.
    pub name: String,
    /// Short faction id, empty if none picked yet.
    pub faction: String,
    /// The relay's P99 round trip to that player, -1 unknown.
    pub ping_ms: f32,
    pub ready: bool,
    /// S18: team, -1 none (alone).
    pub team: i32,
    /// S18: an AI slot's difficulty (0 easy, 1 normal, 2 hard), -1 for a
    /// player or an open slot.
    pub ai: i32,
}

/// `FrontlineNet.status()` (`godot/docs/net_api.md`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SessionView {
    /// `idle`, `connecting`, `lobby`, `countdown`, `in_match`,
    /// `disconnected`.
    pub state: &'static str,
    /// Own slot, -1 before `Welcome`.
    pub slot: i32,
    /// The match's slots (S18: 2 to 4; the lobby may show more).
    pub slots: Vec<SlotView>,
    /// S18: slots in the match.
    pub players: u8,
    pub seed: u64,
    pub map: String,
    /// D in ticks: slot 0's choice or the suggestion (the match's once
    /// started).
    pub input_delay: u8,
    pub suggested_delay: u8,
    /// Why the last Ready round failed: a reason code or text; empty: none.
    pub mismatch: String,
    pub countdown_s: f32,
    /// Reason code once disconnected.
    pub error: String,
    /// More about `error`, when there is more (a build mismatch names
    /// both builds); empty otherwise.
    pub error_detail: String,
    /// Where the relay listens when hosting (for the "join me at" line).
    pub host_port: u16,
}

/// What [`NetSession::take_match`] hands the match.
pub struct NetMatch {
    pub setup: MatchSetup,
    pub slot: u8,
    pub link: NetLink,
    pub info: NetMatchInfo,
}

pub struct NetSession {
    defs: Option<Arc<ModDefs>>,
    /// S3 scripted runs: resolve the match with the test rules override
    /// (both players must set it: the setup hash is the `Ready` check).
    test_unlock: bool,
    /// Scripted runs: both players' starting manpower (both must set the
    /// same; the setup hash is checked at `Ready`).
    pub start_manpower: Option<u32>,
    content_hash: u64,
    name: String,
    /// Godot's `user://` (relay log, desync dumps).
    pub user_dir: Option<PathBuf>,
    relay: Option<RelayServer>,
    handle: Option<NetHandle>,
    match_rx: Option<Receiver<NetEvent>>,
    resolver: Option<Receiver<Result<SocketAddr, String>>>,
    /// The previous connection shutting down (joined before hosting again,
    /// so the port is free).
    closing: Option<JoinHandle<()>>,
    phase: Phase,
    since: Instant,
    slot: Option<u8>,
    lobby: LobbyState,
    choice: Choice,
    start: Option<Start>,
    error: String,
    error_detail: String,
    mismatch: String,
    ready_hash: Option<u64>,
    relay_log: Option<PathBuf>,
    host_port: u16,
    match_taken: bool,
    /// Dev hook for the next match (`--debug_inject_desync`).
    pub inject_desync: Option<u64>,
    events: Vec<NetEvent>,
}

impl Default for NetSession {
    fn default() -> NetSession {
        NetSession::new()
    }
}

impl NetSession {
    #[allow(clippy::disallowed_methods)]
    pub fn new() -> NetSession {
        NetSession {
            defs: None,
            test_unlock: false,
            start_manpower: None,
            content_hash: 0,
            name: DEFAULT_NAME.into(),
            user_dir: None,
            relay: None,
            handle: None,
            match_rx: None,
            resolver: None,
            closing: None,
            phase: Phase::Idle,
            since: Instant::now(),
            slot: None,
            lobby: LobbyState::default(),
            choice: Choice::default(),
            start: None,
            error: String::new(),
            error_detail: String::new(),
            mismatch: String::new(),
            ready_hash: None,
            relay_log: None,
            host_port: 0,
            match_taken: false,
            inject_desync: None,
            events: Vec::new(),
        }
    }

    /// The mod content both players must share (its hash goes in `Hello`).
    pub fn set_defs(&mut self, defs: Arc<ModDefs>) {
        self.content_hash = defs.content_hash();
        self.defs = Some(defs);
    }

    /// Ends any connection and shows reason `code` (`FrontlineNet` uses it
    /// when the mod content cannot be loaded).
    pub fn report(&mut self, code: &'static str) {
        self.fail(code);
    }

    /// S3 scripted runs: play the test rules override. The defs must then
    /// hold it, so changing this drops loaded defs (the next host or join
    /// loads them again).
    pub fn set_test_unlock(&mut self, on: bool) {
        if on != self.test_unlock {
            self.test_unlock = on;
            self.defs = None;
        }
    }

    pub fn test_unlock(&self) -> bool {
        self.test_unlock
    }

    pub fn has_defs(&self) -> bool {
        self.defs.is_some()
    }

    /// The name `Hello` carries (at most 24 bytes; empty: "Player").
    pub fn set_name(&mut self, name: &str) {
        let mut n = String::new();
        for c in name.trim().chars().filter(|c| !c.is_control()) {
            if n.len() + c.len_utf8() > MAX_NAME {
                break;
            }
            n.push(c);
        }
        self.name = if n.is_empty() { DEFAULT_NAME.into() } else { n };
    }

    /// Starts the in-game relay on `port` (all interfaces, IPv4 and IPv6)
    /// and joins it as the first player. `Err` is a reason code.
    pub fn host(&mut self, port: u16) -> Result<(), &'static str> {
        self.host_on(frontline_net::transport::any_addr(port))
    }

    /// [`NetSession::host`] on a given address (tests bind loopback, so no
    /// firewall prompt appears).
    pub fn host_on(&mut self, bind: SocketAddr) -> Result<(), &'static str> {
        self.leave();
        if let Some(t) = self.closing.take() {
            let _ = t.join();
        }
        let log = self.open_relay_log();
        let server = match RelayServer::spawn(RelayConfig::default(), bind, log) {
            Ok(s) => s,
            Err(_) => return Err(self.fail("bind_failed")),
        };
        let port = server.local_addr().port();
        // Join over loopback: `[::]` is dual-stack, so IPv4 loopback
        // reaches it too; a specific address is joined as is.
        let local = if bind.ip().is_unspecified() {
            SocketAddr::from(([127, 0, 0, 1], port))
        } else {
            SocketAddr::new(bind.ip(), port)
        };
        self.relay = Some(server);
        self.host_port = port;
        self.connect(local)
    }

    /// Joins the relay at `addr` (`1.2.3.4:47015`, `[::1]:47015`, or
    /// `host.name:47015`, resolved off the main thread). `Err` is a reason
    /// code.
    pub fn join(&mut self, addr: &str) -> Result<(), &'static str> {
        self.leave();
        let addr = addr.trim();
        if let Ok(a) = addr.parse::<SocketAddr>() {
            return self.connect(a);
        }
        // `name:port`, the name without colons (IPv6 needs brackets).
        let Some((host, port)) = addr.rsplit_once(':') else {
            return Err(self.fail("bad_address"));
        };
        let Ok(port) = port.parse::<u16>() else {
            return Err(self.fail("bad_address"));
        };
        if host.is_empty() || host.contains(':') || host.len() > 253 || port == 0 {
            return Err(self.fail("bad_address"));
        }
        let (tx, rx) = channel();
        let host = host.to_owned();
        let spawned = std::thread::Builder::new()
            .name("frontline-resolve".into())
            .spawn(move || {
                let r = (host.as_str(), port)
                    .to_socket_addrs()
                    .map_err(|e| e.to_string())
                    .and_then(|mut it| it.next().ok_or_else(|| "no address".to_owned()));
                let _ = tx.send(r);
            });
        if spawned.is_err() {
            return Err(self.fail("bad_address"));
        }
        self.resolver = Some(rx);
        self.enter(Phase::Resolving);
        Ok(())
    }

    /// Merges `u` into the choice and sends it (the relay clears both
    /// players' Ready on any change).
    pub fn set_choice(&mut self, u: ChoiceUpdate) {
        if let Some(f) = u.faction.filter(|f| FACTIONS.contains(&f.as_str())) {
            self.choice.faction = f;
        }
        if let Some(s) = u.seed {
            self.choice.seed = s;
        }
        if let Some(m) = u.map.filter(|m| !m.is_empty() && m.len() < 48) {
            self.choice.map = m;
        }
        if let Some(d) = u.d {
            self.choice.d = Some(d.clamp(MIN_D, MAX_D));
        }
        if let Some(t) = u.team {
            self.choice.team = t.map(|t| t.min(MAX_SLOTS as u8 - 1));
        }
        if let Some(n) = u.players {
            self.choice.players = n.clamp(2, MAX_SLOTS as u8);
        }
        if let Some(ai) = u.ai {
            let full = |id: &str| format!("{MOD_ID}:{id}");
            self.choice.ai = ai
                .into_iter()
                .filter(|a| {
                    (1..MAX_SLOTS as u8).contains(&a.slot)
                        && FACTIONS.contains(&short_id(&a.faction))
                })
                .map(|a| AiPick {
                    faction: full(short_id(&a.faction)),
                    team: a.team.map(|t| t.min(MAX_SLOTS as u8 - 1)),
                    difficulty: a.difficulty.min(MAX_DIFFICULTY),
                    slot: a.slot,
                })
                .take(MAX_SLOTS)
                .collect();
        }
        self.mismatch.clear();
        self.ready_hash = None;
        self.send_choice();
    }

    /// Sends `Ready` with the hash of the setup this client would play.
    pub fn ready(&mut self) {
        if self.phase != Phase::Lobby {
            return;
        }
        let Some(slot) = self.slot else { return };
        let n = (self.lobby.players as usize).clamp(2, MAX_SLOTS);
        let mut slots = Vec::with_capacity(n);
        for k in 0..n {
            if k == slot as usize {
                slots.push(SlotChoice {
                    faction: self.choice.faction.clone(),
                    human: true,
                    team: self.choice.team,
                    difficulty: None,
                });
                continue;
            }
            let Some(theirs) = self.lobby.slots.get(k).and_then(|s| s.as_ref()) else {
                self.mismatch = if n == 2 {
                    "Waiting for a second player.".into()
                } else {
                    "Waiting for every slot to be filled.".into()
                };
                return;
            };
            slots.push(slot_choice(theirs));
        }
        let (seed, map) = if slot == 0 {
            (self.choice.seed, self.choice.map.clone())
        } else {
            (self.lobby.seed, short_id(&self.lobby.map).to_owned())
        };
        match self.resolve(&slots, seed, &map) {
            Ok(setup) => {
                let h = frontline_net::setup_hash(&setup);
                self.ready_hash = Some(h);
                self.mismatch.clear();
                self.send(ToNet::Ready(h));
            }
            Err(e) => self.mismatch = e,
        }
    }

    /// S18: moves to open slot `slot` of the lobby (not the host's, not
    /// the host; the relay decides).
    pub fn take_slot(&mut self, slot: u8) {
        if self.phase == Phase::Lobby && self.slot.is_some_and(|s| s != 0) && slot != 0 {
            self.ready_hash = None;
            self.send(ToNet::TakeSlot(slot));
        }
    }

    /// Says goodbye and goes back to idle. Safe in any state; the threads
    /// stop in the background.
    pub fn leave(&mut self) {
        let handle = self.handle.take();
        let relay = self.relay.take();
        self.match_rx = None;
        self.resolver = None;
        if handle.is_some() || relay.is_some() {
            if let Some(t) = self.closing.take() {
                let _ = t.join();
            }
            // The client's goodbye first (a quitter surrenders), then the
            // relay's to whoever is left.
            self.closing = std::thread::Builder::new()
                .name("frontline-net-close".into())
                .spawn(move || {
                    if let Some(mut h) = handle {
                        h.stop();
                    }
                    if let Some(mut r) = relay {
                        r.stop();
                    }
                })
                .ok();
        }
        self.slot = None;
        self.lobby = LobbyState::default();
        self.choice = Choice::default();
        self.start = None;
        self.error.clear();
        self.error_detail.clear();
        self.mismatch.clear();
        self.ready_hash = None;
        self.host_port = 0;
        self.match_taken = false;
        self.enter(Phase::Idle);
    }

    /// Reads the net thread's events and moves the lobby along. Cheap;
    /// call it every frame (`status()` does).
    pub fn poll(&mut self) {
        if let Some(rx) = &self.resolver {
            match rx.try_recv() {
                Ok(Ok(a)) => {
                    self.resolver = None;
                    let _ = self.connect(a);
                }
                Ok(Err(_)) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.resolver = None;
                    self.fail("bad_address");
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        let mut evs = std::mem::take(&mut self.events);
        evs.clear();
        if let Some(h) = &self.handle {
            h.drain_events(&mut evs);
        }
        for ev in evs.drain(..) {
            self.on_event(ev);
        }
        self.events = evs;
        let status = self.client_status();
        match self.phase {
            Phase::Connecting if self.since.elapsed() > CONNECT_TIMEOUT => {
                self.fail("no_answer");
            }
            Phase::Lobby if status.is_some_and(|s| s.silent_us >= LOBBY_SILENCE_US) => {
                self.fail("connection_lost");
            }
            _ => {}
        }
    }

    fn on_event(&mut self, ev: NetEvent) {
        match ev {
            NetEvent::Welcome { slot } => {
                self.slot = Some(slot);
                if self.choice.faction.is_empty() {
                    self.choice.faction = FACTIONS[slot as usize % 2].into();
                }
                self.enter(Phase::Lobby);
                self.send_choice();
            }
            NetEvent::Rejected(r) => {
                self.fail(reject_code(&r));
                if let RejectReason::Build { lobby } = &r {
                    self.error_detail = format!(
                        "Their build: {lobby}. Yours: {}.",
                        frontline_net::build_id()
                    );
                }
            }
            NetEvent::WrongVersion { .. } => {
                self.fail("protocol");
            }
            NetEvent::Slot { slot } => {
                self.slot = Some(slot);
                self.ready_hash = None;
            }
            NetEvent::Lobby(l) => {
                if !l.note.is_empty() {
                    self.mismatch = if l.note.contains("setups differ") {
                        "setup_mismatch".into()
                    } else {
                        l.note.clone()
                    };
                }
                self.lobby = l;
            }
            NetEvent::Start(s) => {
                self.start = Some(s);
                self.enter(Phase::Started);
            }
            NetEvent::Bye(r) if self.phase != Phase::Disconnected => {
                self.fail(bye_code(r));
            }
            _ => {}
        }
    }

    /// The presenter's snapshot.
    pub fn view(&mut self) -> SessionView {
        self.poll();
        let status = self.client_status();
        let state = match self.phase {
            Phase::Idle => "idle",
            Phase::Resolving | Phase::Connecting => "connecting",
            Phase::Lobby => "lobby",
            Phase::Started => {
                if self.countdown_s(status.as_ref()) > 0.0 {
                    "countdown"
                } else {
                    "in_match"
                }
            }
            Phase::Disconnected => "disconnected",
        };
        let players = match &self.start {
            Some(s) => s.players.len(),
            None => (self.lobby.players as usize).clamp(2, MAX_SLOTS),
        };
        let mut slots: Vec<SlotView> = if self.slot.is_some() {
            match &self.start {
                Some(s) => s
                    .players
                    .iter()
                    .enumerate()
                    .map(|(k, p)| SlotView {
                        name: p.name.clone(),
                        faction: short_id(&p.faction).to_owned(),
                        ping_ms: self
                            .lobby
                            .slots
                            .get(k)
                            .and_then(|s| s.as_ref())
                            .map_or(-1.0, |s| s.rtt_ms as f32),
                        ready: true,
                        team: p.team.map_or(-1, |t| t as i32),
                        ai: p.ai.map_or(-1, |d| d as i32),
                    })
                    .collect(),
                // Every match slot, open ones too (also before the first
                // lobby state arrived).
                None => (0..players)
                    .map(|k| slot_view(self.lobby.slots.get(k).unwrap_or(&None)))
                    .collect(),
            }
        } else {
            Vec::new()
        };
        // The relay starts without a last lobby update: both were ready.
        if self.phase == Phase::Started {
            for s in &mut slots {
                s.ready = true;
            }
        }
        let (seed, map, d) = match &self.start {
            Some(s) => (s.seed, short_id(&s.map).to_owned(), s.d),
            None => (
                self.lobby.seed,
                short_id(&self.lobby.map).to_owned(),
                self.lobby
                    .d
                    .unwrap_or(self.lobby.suggested_d)
                    .clamp(MIN_D, MAX_D),
            ),
        };
        SessionView {
            state,
            slot: self.slot.map_or(-1, |s| s as i32),
            slots,
            players: players as u8,
            seed,
            map: if map.is_empty() {
                self.choice.map.clone()
            } else {
                map
            },
            input_delay: d,
            suggested_delay: self.lobby.suggested_d.clamp(MIN_D, MAX_D),
            mismatch: self.mismatch.clone(),
            countdown_s: if self.phase == Phase::Started {
                self.countdown_s(status.as_ref())
            } else {
                0.0
            },
            error: self.error.clone(),
            error_detail: self.error_detail.clone(),
            host_port: self.host_port,
        }
    }

    /// The started match: its setup (its slots from `Start`) and the sim
    /// thread's link. Once per match.
    pub fn take_match(&mut self) -> Result<NetMatch, String> {
        self.poll();
        if self.phase != Phase::Started {
            return Err("the net match has not started".into());
        }
        if self.match_taken {
            return Err("the net match was already started".into());
        }
        let (Some(start), Some(slot), Some(h)) = (self.start.clone(), self.slot, &self.handle)
        else {
            return Err("no net match".into());
        };
        let slots: Vec<SlotChoice> = start
            .players
            .iter()
            .map(|p| SlotChoice {
                faction: short_id(&p.faction).to_owned(),
                human: p.ai.is_none(),
                team: p.team,
                difficulty: p.ai.map(|d| difficulty_name(d).to_owned()),
            })
            .collect();
        let setup = self.resolve(&slots, start.seed, short_id(&start.map))?;
        let setup_hash = frontline_net::setup_hash(&setup);
        if self.ready_hash.is_some_and(|r| r != setup_hash) {
            // The relay matched both Ready hashes, so the other client sent
            // ours too; `Start` is what both now build from.
            eprintln!("FrontlineNet: Start differs from the setup this client readied");
        }
        let events = self
            .match_rx
            .take()
            .ok_or("the net match was already started")?;
        self.match_taken = true;
        let info = NetMatchInfo {
            d: start.d,
            match_id: format!(
                "{}-{:08x}",
                start.seed,
                (setup_hash ^ start.start_at) as u32
            ),
            names: start.players.iter().map(|p| p.name.clone()).collect(),
            build_id: frontline_net::build_id(),
            content_hash: self.content_hash,
            setup_hash,
            seed: start.seed,
            user_dir: self.user_dir.clone(),
            relay_log: self.relay_log.clone(),
            inject_desync: self.inject_desync,
        };
        Ok(NetMatch {
            setup,
            slot,
            link: NetLink {
                events,
                to_net: h.sender(),
                status: h.status_reader(),
                origin: h.origin(),
            },
            info,
        })
    }

    /// The players' names from `Start` (or the lobby), one per slot (an AI
    /// slot's is empty).
    pub fn names(&self) -> Vec<String> {
        match &self.start {
            Some(s) => s.players.iter().map(|p| p.name.clone()).collect(),
            None => self
                .lobby
                .slots
                .iter()
                .take((self.lobby.players as usize).clamp(2, MAX_SLOTS))
                .map(|s| s.as_ref().map(|s| s.name.clone()).unwrap_or_default())
                .collect(),
        }
    }

    // ------------------------------------------------------------ inner --

    fn resolve(&self, slots: &[SlotChoice], seed: u64, map: &str) -> Result<MatchSetup, String> {
        let defs = self.defs.as_ref().ok_or("no mod content loaded")?;
        if slots.len() == 2
            && slots.iter().all(|s| s.team.is_none())
            && slots[0].faction == slots[1].faction
        {
            return Err(format!(
                "Both players picked {}; factions must differ.",
                slots[0].faction
            ));
        }
        let own = (self.slot.unwrap_or(0) as usize).min(slots.len().saturating_sub(1));
        let choice = MenuChoice {
            faction: slots
                .get(own)
                .map(|s| s.faction.clone())
                .unwrap_or_default(),
            difficulty: "normal".into(),
            map: map.to_owned(),
            seed,
            ai_vs_ai: false,
            tickets: None,
            start_manpower: self.start_manpower,
            slots: Some(slots.to_vec()),
            // S18: the slots decide; four of them are a 2v2.
            mode: if slots.len() == 4 { "2v2" } else { "1v1" }.into(),
            test_unlock: self.test_unlock,
        };
        resolve_with(defs, &choice)
    }

    fn connect(&mut self, relay: SocketAddr) -> Result<(), &'static str> {
        let cfg = ClientConfig {
            relay,
            name: self.name.clone(),
            build_id: frontline_net::build_id(),
            content_hash: self.content_hash,
            nonce: OsRandom::new().next_u64(),
        };
        match NetHandle::spawn(cfg) {
            Ok(mut h) => {
                // Split the sim's events off at once: bundles queue in the
                // channel until the match takes it.
                self.match_rx = h.take_match_events();
                self.handle = Some(h);
                self.enter(Phase::Connecting);
                Ok(())
            }
            Err(_) => Err(self.fail("bind_failed")),
        }
    }

    fn send_choice(&self) {
        let (Some(slot), Phase::Lobby) = (self.slot, self.phase) else {
            return;
        };
        let c = &self.choice;
        let full = |id: &str| format!("{MOD_ID}:{id}");
        self.send(ToNet::Lobby(frontline_net::protocol::LobbySet {
            faction: if c.faction.is_empty() {
                String::new()
            } else {
                full(&c.faction)
            },
            seed: (slot == 0).then_some(c.seed),
            map: (slot == 0).then(|| full(&c.map)),
            d: if slot == 0 { c.d } else { None },
            team: c.team,
            players: (slot == 0).then_some(c.players),
            ai: (slot == 0).then(|| c.ai.clone()),
        }));
    }

    fn send(&self, m: ToNet) {
        if let Some(h) = &self.handle {
            h.send(m);
        }
    }

    fn client_status(&self) -> Option<ClientStatus> {
        self.handle.as_ref().map(|h| h.status())
    }

    fn countdown_s(&self, status: Option<&ClientStatus>) -> f32 {
        let (Some(start), Some(h)) = (&self.start, &self.handle) else {
            return 0.0;
        };
        let Some(offset) = status.and_then(|s| s.offset_us) else {
            return 3.0;
        };
        let local_start = start.start_at as i128 - offset as i128;
        ((local_start - h.now_us() as i128).max(0) as f64 / 1e6) as f32
    }

    /// `<user>/net/relay_last.jsonl`, line-buffered (a desync dump copies
    /// it). Overwritten by every hosted session.
    fn open_relay_log(&mut self) -> Option<LogSink> {
        self.relay_log = None;
        let dir = self.user_dir.as_ref()?.join("net");
        std::fs::create_dir_all(&dir).ok()?;
        let path = dir.join("relay_last.jsonl");
        let f = std::fs::File::create(&path).ok()?;
        self.relay_log = Some(path);
        Some(Box::new(std::io::LineWriter::new(f)))
    }

    #[allow(clippy::disallowed_methods)]
    fn enter(&mut self, p: Phase) {
        self.phase = p;
        self.since = Instant::now();
    }

    /// Ends the connection with reason `code`; returns it.
    fn fail(&mut self, code: &'static str) -> &'static str {
        self.leave();
        self.error = code.into();
        self.enter(Phase::Disconnected);
        code
    }
}

impl Drop for NetSession {
    fn drop(&mut self) {
        self.leave();
        if let Some(t) = self.closing.take() {
            let _ = t.join();
        }
    }
}

fn slot_view(s: &Option<SlotInfo>) -> SlotView {
    match s {
        Some(s) => SlotView {
            name: s.name.clone(),
            faction: short_id(&s.faction).to_owned(),
            ping_ms: if s.ai.is_some() {
                -1.0
            } else {
                s.rtt_ms as f32
            },
            ready: s.ready || s.ai.is_some(),
            team: s.team.map_or(-1, |t| t as i32),
            ai: s.ai.map_or(-1, |d| d as i32),
        },
        None => SlotView {
            ping_ms: -1.0,
            team: -1,
            ai: -1,
            ..SlotView::default()
        },
    }
}

/// A filled lobby slot as the setup sees it (a player, or AI fill).
fn slot_choice(s: &SlotInfo) -> SlotChoice {
    SlotChoice {
        faction: short_id(&s.faction).to_owned(),
        human: s.ai.is_none(),
        team: s.team,
        difficulty: s.ai.map(|d| difficulty_name(d).to_owned()),
    }
}

/// The wire's AI difficulty → the menu's word.
pub fn difficulty_name(d: u8) -> &'static str {
    match d {
        0 => "easy",
        2 => "hard",
        _ => "normal",
    }
}

fn reject_code(r: &RejectReason) -> &'static str {
    match r {
        RejectReason::Version => "protocol",
        RejectReason::Build { .. } => "build_mismatch",
        RejectReason::Content => "content_mismatch",
        RejectReason::Full => "full",
        RejectReason::Started => "started",
        RejectReason::BadName => "bad_name",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::sync::atomic::Ordering;

    use frontline_net::rng::SplitMix;
    use frontline_sim::{CommandKind, FxVec2, Outcome, PlayerId, SquadId, World};

    use super::*;
    use crate::match_frame::MatchFrame;
    use crate::match_net::{NetState, hashes};
    use crate::match_setup::{find_mods_dir, mods_candidates};
    use crate::match_thread::MatchHandle;

    fn defs() -> Arc<ModDefs> {
        let dir = find_mods_dir(&mods_candidates(None, None)).unwrap();
        Arc::new(ModDefs::load(&dir).unwrap())
    }

    fn temp_dir(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("frontline-bridge-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn session(defs: &Arc<ModDefs>, name: &str, user: &Path) -> NetSession {
        let mut s = NetSession::new();
        s.set_defs(Arc::clone(defs));
        s.set_name(name);
        s.user_dir = Some(user.to_owned());
        s
    }

    const FACTION: [&str; 2] = ["usa", "germany"];

    /// Hosts on a free loopback port and waits for the host's own client
    /// to be in the lobby (slot 0 is whoever's `Hello` arrives first; a
    /// real joiner comes seconds later). Returns the port.
    fn host_lobby(host: &mut NetSession) -> u16 {
        host.host_on("127.0.0.1:0".parse().unwrap()).unwrap();
        let t0 = Instant::now();
        while host.view().state != "lobby" {
            assert!(t0.elapsed() < Duration::from_secs(5), "{:?}", host.view());
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(host.view().slot, 0);
        host.view().host_port
    }

    /// Hosts on loopback, joins, picks factions (host USA) and a seed,
    /// and readies until the relay starts the match.
    fn lobby(host: &mut NetSession, guest: &mut NetSession, seed: u64) {
        let port = host_lobby(host);
        guest.join(&format!("127.0.0.1:{port}")).unwrap();
        let t0 = Instant::now();
        let mut chose = [false; 2];
        let mut readied: [Option<Instant>; 2] = [None; 2];
        loop {
            assert!(
                t0.elapsed() < Duration::from_secs(20),
                "the lobby did not start"
            );
            let mut started = 0;
            for (i, s) in [&mut *host, &mut *guest].into_iter().enumerate() {
                let v = s.view();
                match v.state {
                    "lobby" => {
                        if !chose[i] {
                            chose[i] = true;
                            s.set_choice(ChoiceUpdate {
                                faction: Some(FACTION[i].into()),
                                seed: (i == 0).then_some(seed),
                                ..ChoiceUpdate::default()
                            });
                        }
                        let me = v.slot as usize;
                        let full = v.slots.len() == 2 && v.slots.iter().all(|s| !s.name.is_empty());
                        let set = full
                            && v.slots[me].faction == FACTION[i]
                            && v.slots[1 - me].faction == FACTION[1 - i]
                            && v.seed == seed;
                        // Ready (again, if a lobby change cleared it).
                        if set
                            && !v.slots[me].ready
                            && readied[i].is_none_or(|t| t.elapsed() > Duration::from_millis(400))
                        {
                            readied[i] = Some(Instant::now());
                            s.ready();
                        }
                    }
                    "countdown" | "in_match" => started += 1,
                    "connecting" => {}
                    other => panic!("session {i} is {other}: {:?}", v.error),
                }
            }
            if started == 2 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let v = host.view();
        assert_eq!((v.slot, v.seed), (0, seed));
        assert!((2..=4).contains(&v.input_delay), "D {}", v.input_delay);
        assert_eq!(v.slots[0].name, "Host");
        assert_eq!(guest.view().slot, 1);
    }

    /// S18: a 2v2 lobby over loopback: the host opens four slots and
    /// fills slots 1 (his ally) and 3 with AI, the guest gets slot 2 and
    /// picks team 1; both ready, and both clients build the same match
    /// from `Start`: teams 0 0 1 1, AI in slots 1 and 3.
    #[test]
    fn a_2v2_lobby_with_ai_fill_starts_one_match() {
        let defs = defs();
        let user = temp_dir("lobby2v2");
        let mut host = session(&defs, "Host", &user);
        let port = host_lobby(&mut host);
        let ai = |slot: u8, faction: &str, team: u8| AiPick {
            slot,
            faction: faction.into(),
            team: Some(team),
            difficulty: 2,
        };
        host.set_choice(ChoiceUpdate {
            faction: Some("usa".into()),
            map: Some("harrow_cross".into()),
            seed: Some(5),
            team: Some(Some(0)),
            players: Some(4),
            ai: Some(vec![ai(1, "usa", 0), ai(3, "germany", 1)]),
            ..ChoiceUpdate::default()
        });
        let t0 = Instant::now();
        while host.view().players != 4 || host.view().slots[3].ai != 2 {
            assert!(t0.elapsed() < Duration::from_secs(5), "{:?}", host.view());
            std::thread::sleep(Duration::from_millis(5));
        }
        let mut guest = session(&defs, "Guest", &user);
        guest.join(&format!("127.0.0.1:{port}")).unwrap();
        let t0 = Instant::now();
        let mut chose = false;
        let mut readied: [Option<Instant>; 2] = [None; 2];
        loop {
            assert!(
                t0.elapsed() < Duration::from_secs(20),
                "the lobby did not start: {:?} {:?}",
                host.view(),
                guest.view()
            );
            let mut started = 0;
            for (i, s) in [&mut host, &mut guest].into_iter().enumerate() {
                let v = s.view();
                match v.state {
                    "lobby" => {
                        if i == 1 && !chose {
                            chose = true;
                            assert_eq!(v.slot, 2, "{v:?}");
                            s.set_choice(ChoiceUpdate {
                                faction: Some("germany".into()),
                                team: Some(Some(1)),
                                ..ChoiceUpdate::default()
                            });
                        }
                        let set = v.slots.len() == 4
                            && v.slots[2].faction == "germany"
                            && v.slots[2].team == 1
                            && v.slots[0].team == 0;
                        let me = v.slot as usize;
                        if set
                            && !v.slots[me].ready
                            && readied[i].is_none_or(|t| t.elapsed() > Duration::from_millis(400))
                        {
                            readied[i] = Some(Instant::now());
                            s.ready();
                        }
                    }
                    "countdown" | "in_match" => started += 1,
                    "connecting" => {}
                    other => panic!("session {i} is {other}: {:?}", v.error),
                }
            }
            if started == 2 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let a = host.take_match().unwrap();
        let b = guest.take_match().unwrap();
        assert_eq!((a.slot, b.slot), (0, 2));
        assert_eq!(
            frontline_net::setup_hash(&a.setup),
            frontline_net::setup_hash(&b.setup)
        );
        let teams: Vec<u8> = a.setup.players.iter().map(|p| p.team).collect();
        assert_eq!(teams, [0, 0, 1, 1]);
        let ai: Vec<bool> = a.setup.players.iter().map(|p| p.is_ai).collect();
        assert_eq!(ai, [false, true, false, true]);
        assert_eq!(a.info.names, ["Host", "", "Guest", ""]);
        assert_eq!(host.view().slots[3].ai, 2);
    }

    /// S3b quick iteration: a player who edited the mod's TOML (here
    /// through a tuning override of one rules value) gets a different
    /// content hash, and the relay refuses him in the lobby with
    /// `content_mismatch` (net_ui.gd: "The two games have different mod
    /// content. Both players need the same mods."). The unedited player
    /// joins as before.
    #[test]
    fn edited_mod_data_is_refused_in_the_lobby() {
        let base = defs();
        let dir = find_mods_dir(&mods_candidates(None, None)).unwrap();
        let user = temp_dir("content");
        let patch = user.join("edit.toml");
        std::fs::create_dir_all(&user).unwrap();
        std::fs::write(&patch, "[combat]\nnear_miss_cm = 250\n").unwrap();
        let mut edited = ModDefs::load(&dir).unwrap();
        edited.apply_override(&patch, "ww2:skirmish").unwrap();
        assert_ne!(edited.content_hash(), base.content_hash());
        let edited = Arc::new(edited);

        let mut host = session(&base, "Host", &user);
        let port = host_lobby(&mut host);
        let mut guest = session(&edited, "Edited", &user);
        guest.join(&format!("127.0.0.1:{port}")).unwrap();
        let t0 = Instant::now();
        while guest.view().state != "disconnected" {
            assert!(t0.elapsed() < Duration::from_secs(10), "{:?}", guest.view());
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(guest.view().error, "content_mismatch");
        assert_eq!(host.view().state, "lobby");
        // The same data joins.
        let mut same = session(&defs(), "Same", &user);
        same.join(&format!("127.0.0.1:{port}")).unwrap();
        let t0 = Instant::now();
        while same.view().state != "lobby" {
            assert!(t0.elapsed() < Duration::from_secs(10), "{:?}", same.view());
            std::thread::sleep(Duration::from_millis(5));
        }
        let _ = std::fs::remove_dir_all(&user);
    }

    fn spawn(s: &mut NetSession) -> MatchHandle {
        let m = s.take_match().unwrap();
        assert!(s.take_match().is_err(), "a match is taken once");
        assert!(m.setup.players.iter().all(|p| !p.is_ai));
        let world = World::new_match(m.setup, 1).unwrap();
        MatchHandle::spawn_net(world, m.slot, m.link, m.info).unwrap()
    }

    /// Orders for own squads from the newest frame: a call-in now and
    /// then, moves to random spots otherwise.
    fn order(h: &mut MatchHandle, f: &mut MatchFrame, slot: u8, rng: &mut SplitMix) -> u64 {
        h.take_newest(f);
        let own: Vec<u32> = f
            .squads
            .iter()
            .filter(|s| s.owner == slot)
            .map(|s| s.id)
            .collect();
        let kind = if own.is_empty() || rng.below(5) == 0 {
            CommandKind::CallIn {
                squad_type: rng.below(2) as u16,
            }
        } else {
            CommandKind::Move {
                squad: SquadId(own[rng.below(own.len() as u64) as usize]),
                target: FxVec2::from_int(200 + rng.below(600) as i32, 200 + rng.below(600) as i32),
            }
        };
        h.send(kind).map_or(0, |_| 1)
    }

    /// The bridge end to end on real sockets: host and guest sessions on
    /// loopback, two net `MatchHandle`s for ~60 game seconds with orders
    /// from both, then a surrender. Hashes agree at every check, both
    /// command logs match, and every order ran.
    #[test]
    fn two_net_matches_agree_over_loopback() {
        let secs: u64 = std::env::var("FRONTLINE_NET_TEST_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);
        let user = temp_dir("loopback");
        let defs = defs();
        let mut host = session(&defs, "Host", &user);
        let mut guest = session(&defs, "Guest", &user);
        lobby(&mut host, &mut guest, 21);
        let mut hs = [spawn(&mut host), spawn(&mut guest)];
        let mut frames = [MatchFrame::empty(), MatchFrame::empty()];
        let mut rng = SplitMix::new(5);
        let mut sent = [0u64; 2];
        let t0 = Instant::now();
        // The countdown (3 s), then the match; quiet for the last 3 s.
        let end = Duration::from_secs(3 + secs);
        let mut next = [Instant::now(); 2];
        while t0.elapsed() < end {
            for i in 0..2 {
                if t0.elapsed() + Duration::from_secs(3) < end && Instant::now() >= next[i] {
                    next[i] = Instant::now() + Duration::from_millis(400 + rng.below(1200));
                    let (h, f) = (&mut hs[i], &mut frames[i]);
                    sent[i] += order(h, f, i as u8, &mut rng);
                }
                assert!(hs[i].is_alive(), "{:?}", hs[i].panic_message());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let ticks: Vec<u64> = hs
            .iter()
            .map(|h| h.counters.ticks.load(Ordering::Relaxed))
            .collect();
        let hz = 30;
        assert!(
            ticks.iter().all(|&t| t + hz * 2 >= hz * secs),
            "ticks {ticks:?}"
        );
        // Every published hash both have agrees.
        let a = hashes(&hs[0].counters);
        let b: BTreeMap<u64, u64> = hashes(&hs[1].counters).into_iter().collect();
        let mut checks = 0;
        for (t, h) in &a {
            if let Some(o) = b.get(t) {
                assert_eq!(h, o, "hash at tick {t}");
                checks += 1;
            }
        }
        assert!(checks + 2 >= secs, "{checks} hash checks");
        // Both ran the same commands at the same ticks.
        let logs: Vec<Vec<_>> = hs
            .iter()
            .map(|h| h.counters.log.lock().unwrap().clone())
            .collect();
        let common = ticks[0].min(ticks[1]);
        let upto = |l: &Vec<(u64, frontline_sim::Command)>| {
            l.iter()
                .filter(|e| e.0 < common)
                .cloned()
                .collect::<Vec<_>>()
        };
        assert_eq!(upto(&logs[0]), upto(&logs[1]));
        for i in 0..2 {
            let c = &hs[i].counters;
            assert!(sent[i] > 10, "sent {sent:?}");
            assert_eq!(
                c.own_executed.load(Ordering::Relaxed),
                sent[i],
                "every own order ran (slot {i})"
            );
            let own = logs[i]
                .iter()
                .filter(|e| e.1.player == PlayerId(i as u8))
                .count() as u64;
            assert_eq!(own, sent[i]);
            assert_eq!(
                NetState::from_u8(c.net_state.load(Ordering::Relaxed)),
                NetState::Ok
            );
            assert_eq!(c.desync_tick.load(Ordering::Relaxed), -1);
            assert!(c.rtt_ms.load(Ordering::Relaxed) >= 0);
            eprintln!(
                "slot {i}: ticks {} sent {} D {} stalls {} restamps {} max step {} us late {}",
                ticks[i],
                sent[i],
                c.input_delay.load(Ordering::Relaxed),
                c.stall_ticks.load(Ordering::Relaxed),
                c.restamps.load(Ordering::Relaxed),
                c.max_step_us.load(Ordering::Relaxed),
                c.late_ticks.load(Ordering::Relaxed),
            );
        }
        eprintln!("{checks} hash checks agree");
        // The host surrenders: both see the guest win, and stop there.
        hs[0].send(CommandKind::Surrender);
        let t1 = Instant::now();
        let won = Outcome::Won {
            winner: PlayerId(1),
        };
        while t1.elapsed() < Duration::from_secs(5) && frames.iter().any(|f| f.outcome != won) {
            for i in 0..2 {
                hs[i].take_newest(&mut frames[i]);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(frames.iter().all(|f| f.outcome == won));
        assert_eq!(frames[0].tick, frames[1].tick);
        for h in &mut hs {
            h.stop();
        }
        host.leave();
        guest.leave();
        let _ = std::fs::remove_dir_all(&user);
    }

    /// `debug_inject_desync` on the guest: both clients stop in `desync`
    /// and write their dumps (plus the host's relay log) under
    /// `user://desync/<match_id>/`.
    #[test]
    fn injected_desync_dumps_both_clients() {
        let user = temp_dir("desync");
        let defs = defs();
        let mut host = session(&defs, "Host", &user);
        let mut guest = session(&defs, "Guest", &user);
        guest.inject_desync = Some(45);
        lobby(&mut host, &mut guest, 3);
        let hs = [spawn(&mut host), spawn(&mut guest)];
        let t0 = Instant::now();
        let dumped = |h: &MatchHandle| h.counters.net.lock().unwrap().desync_dump.clone();
        while t0.elapsed() < Duration::from_secs(15) && hs.iter().any(|h| dumped(h).is_none()) {
            std::thread::sleep(Duration::from_millis(20));
        }
        let dirs: Vec<PathBuf> = hs
            .iter()
            .map(|h| PathBuf::from(dumped(h).expect("a dump")))
            .collect();
        for (i, h) in hs.iter().enumerate() {
            let c = &h.counters;
            assert_eq!(
                NetState::from_u8(c.net_state.load(Ordering::Relaxed)),
                NetState::Desync
            );
            // The first hash after tick 45 differs.
            assert_eq!(c.desync_tick.load(Ordering::Relaxed), 60);
            assert!(dirs[i].ends_with(format!("slot{i}")));
            for f in ["manifest.json", "paused.sav", "bundles.jsonl"] {
                assert!(dirs[i].join(f).is_file(), "{f}");
            }
        }
        assert_eq!(dirs[0].parent(), dirs[1].parent(), "one match id");
        assert!(dirs[0].parent().unwrap().join("relay.jsonl").is_file());
        let _ = std::fs::remove_dir_all(&user);
    }

    /// The host leaves mid-match (Leave in the Esc menu): its client says
    /// `Bye(Quit)` and its relay stops right after. The guest still runs
    /// the host's `Surrender` and wins, instead of seeing a closed relay.
    #[test]
    fn host_leaving_mid_match_surrenders_to_the_guest() {
        let user = temp_dir("hostquit");
        let defs = defs();
        let mut host = session(&defs, "Host", &user);
        let mut guest = session(&defs, "Guest", &user);
        lobby(&mut host, &mut guest, 5);
        let hs = [spawn(&mut host), spawn(&mut guest)];
        let t0 = Instant::now();
        while hs[1].counters.ticks.load(Ordering::Relaxed) < 60 {
            assert!(t0.elapsed() < Duration::from_secs(15), "the match runs");
            std::thread::sleep(Duration::from_millis(10));
        }
        host.leave();
        let host_slot = 0u8;
        let surrendered = |h: &MatchHandle| {
            h.counters.log.lock().unwrap().iter().any(|(_, c)| {
                c.player.0 == host_slot && c.kind == frontline_sim::CommandKind::Surrender
            })
        };
        let t0 = Instant::now();
        while !surrendered(&hs[1]) {
            assert!(
                t0.elapsed() < Duration::from_secs(5),
                "the guest never ran the host's surrender"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        std::thread::sleep(Duration::from_millis(200));
        let detail = hs[1].counters.net.lock().unwrap().clone();
        assert_eq!(detail.disconnect, None, "a win, not a closed relay");
        guest.leave();
        drop(hs);
        let _ = std::fs::remove_dir_all(&user);
    }

    #[test]
    fn lobby_failures_have_reason_codes() {
        let user = temp_dir("lobby");
        let defs = defs();
        let mut s = session(&defs, "  A very long name that goes on and on  ", &user);
        assert_eq!(s.name.len(), 24);
        assert_eq!(s.join("no port here"), Err("bad_address"));
        assert_eq!(s.view().state, "disconnected");
        assert_eq!(s.view().error, "bad_address");
        assert_eq!(s.join("[::1:47015"), Err("bad_address"));
        // The host leaving a lobby says goodbye to the guest.
        let mut host = session(&defs, "Host", &user);
        let port = host_lobby(&mut host);
        let mut guest = session(&defs, "Guest", &user);
        guest.join(&format!("127.0.0.1:{port}")).unwrap();
        let t0 = Instant::now();
        while guest.view().state != "lobby" {
            assert!(t0.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(10));
        }
        // Same factions: Ready is refused locally.
        let t0 = Instant::now();
        while guest.view().slots.iter().any(|s| s.name.is_empty()) {
            assert!(t0.elapsed() < Duration::from_secs(5));
            host.view();
            std::thread::sleep(Duration::from_millis(10));
        }
        guest.set_choice(ChoiceUpdate {
            faction: Some(host.view().slots[0].faction.clone()),
            ..ChoiceUpdate::default()
        });
        guest.ready();
        assert!(guest.view().mismatch.contains("must differ"));
        host.leave();
        let t0 = Instant::now();
        while guest.view().state != "disconnected" {
            assert!(t0.elapsed() < Duration::from_secs(5), "no goodbye");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(guest.view().error, "relay_closed");
        // A second host on the same port after leaving works.
        host.host_on(format!("127.0.0.1:{port}").parse().unwrap())
            .unwrap();
        host.leave();
        let _ = std::fs::remove_dir_all(&user);
    }
}
