//! Driving `World`s from the network, for tests and `frontline-net-test`.
//!
//! * [`SimDriver`] steps one client's world from its bundles: at each
//!   tick's scheduled slot, never past the newest bundle, catching up at
//!   most 3 ticks per period. It reports published hashes, keeps the
//!   300-tick `state_hash` ring, and holds the dev desync hook.
//! * [`run_virtual`] plays a whole two-client match on a virtual clock over
//!   a [`SimNet`]: an in-process [`Relay`], two [`NetClient`]s with skewed
//!   and drifting clocks, two worlds, scripted commands, and optional
//!   blackouts, address changes, garbage datagrams and desync injection.
//!
//! **Desync hook** (S2 "Debug hook"): with `inject_desync = Some((T, slot))`,
//! the driver steps tick `T` with one extra, local-only `StopUnits` for the
//! lowest-id living soldier of its own player. Only this client runs it, so
//! the two sims diverge without any sim change. A Godot build can use the
//! same [`desync_command`].

use std::collections::{BTreeMap, VecDeque};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Instant;

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs, PlayerRequest};
use frontline_sim::{
    Command, CommandKind, EntityKind, FxVec2, Outcome, PlayerId, StanceOrder, World,
};

use crate::client::{ClientConfig, ClientPhase, NetClient, NetEvent};
use crate::protocol::{ByeReason, LobbySet, MAX_DATAGRAM};
use crate::relay::{LogRecord, Relay, RelayConfig};
use crate::rng::{SplitMix, TokenSource};
use crate::transport::{LinkParams, RECV_BUF, SimNet, SimNetStats, SimSocket, Transport};

/// `state_hash` ring length (S2 "Hash check").
pub const HASH_RING: usize = 300;
/// Catch-up pace: at most this many ticks per period.
pub const CATCH_UP: u64 = 3;

/// The dev desync command: a `StopUnits` for `slot`'s lowest-id living
/// soldier, with the largest `seq` so it sorts after the tick's real
/// commands. `None` if the player has no soldier.
pub fn desync_command(world: &World, slot: u8) -> Option<Command> {
    let ents = world.entities();
    let unit = ents
        .kind()
        .iter()
        .zip(ents.owner())
        .position(|(k, o)| *k == EntityKind::Soldier && o.0 == slot)?;
    Some(Command {
        player: PlayerId(slot),
        seq: u32::MAX,
        kind: CommandKind::StopUnits {
            units: vec![frontline_sim::SimId(unit as u32)],
        },
    })
}

/// One executed command, as the tests check it.
#[derive(Copy, Clone, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct Executed {
    pub tick: u64,
    pub player: u8,
    pub seq: u32,
    pub surrender: bool,
}

/// Steps one world from its client's bundles. See the module docs.
#[derive(Default)]
pub struct SimDriver {
    queue: VecDeque<(u64, Vec<Command>)>,
    /// Do not step this tick or later (a pause or a desync).
    stop_at: Option<u64>,
    last_step: Option<u64>,
    /// Dev hook: step tick `.0` with [`desync_command`] for slot `.1`.
    pub inject_desync: Option<(u64, u8)>,
    pub injected: bool,
    /// `(tick, state_hash)` after each of the last [`HASH_RING`] steps.
    pub ring: VecDeque<(u64, u64)>,
    /// Every published `(tick, hash)` (when `record`).
    pub published: Vec<(u64, u64)>,
    /// Every executed network command (when `record`).
    pub executed: Vec<Executed>,
    pub record: bool,
    /// Ticks stepped more than one period after their slot, because the
    /// bundle was late or the driver was catching up.
    pub stall_ticks: u64,
    /// Wall-clock µs of each `World::step` (when `record`).
    pub step_us: Vec<u32>,
    pub desync: Option<(u64, u64)>,
    /// Checkpoints and bundles for a desync dump, when enabled.
    pub recorder: Option<crate::dump::DesyncRecorder>,
}

impl SimDriver {
    pub fn new(record: bool) -> SimDriver {
        SimDriver {
            record,
            ..SimDriver::default()
        }
    }

    pub fn on_event(&mut self, ev: &NetEvent) {
        match ev {
            NetEvent::Bundle { tick, cmds } => self.queue.push_back((*tick, cmds.clone())),
            NetEvent::Paused { at, .. } => self.stop_at = Some(*at),
            NetEvent::Resume { .. } => self.stop_at = None,
            NetEvent::Desync { tick, at, .. } => {
                self.stop_at = Some(*at);
                self.desync = Some((*tick, *at));
            }
            _ => {}
        }
    }

    /// The relay confirmed hashes up to `tick` (`NetClient::confirmed`).
    pub fn confirm(&mut self, tick: u64) {
        if let Some(r) = self.recorder.as_mut() {
            r.confirm(tick);
        }
    }

    /// Bundles received but not stepped yet.
    pub fn backlog(&self) -> usize {
        self.queue.len()
    }

    /// The oldest bundle not stepped yet: what the next step runs (plus
    /// the desync command, if one is due).
    pub fn next_bundle(&self) -> Option<(u64, &[Command])> {
        self.queue.front().map(|(t, c)| (*t, c.as_slice()))
    }

    /// The tick the driver does not step past (a pause or a desync).
    pub fn halted_at(&self) -> Option<u64> {
        self.stop_at
    }

    /// Steps every tick that is due at local time `now`. `slot(t)` is the
    /// local time tick `t` is due ([`NetClient::step_slot`]); `on_hash`
    /// gets each published hash for the relay. Returns the ticks stepped.
    #[allow(clippy::disallowed_methods)]
    pub fn advance(
        &mut self,
        world: &mut World,
        now: u64,
        period: u64,
        slot: impl Fn(u64) -> Option<u64>,
        mut on_hash: impl FnMut(u64, u64),
    ) -> u32 {
        let mut n = 0;
        loop {
            let t = world.tick();
            if self.stop_at.is_some_and(|p| t >= p) {
                break;
            }
            let Some(due) = slot(t) else { break };
            if now < due {
                break;
            }
            if self.last_step.is_some_and(|l| now < l + period / CATCH_UP) {
                break;
            }
            let Some(&(bt, _)) = self.queue.front() else {
                break;
            };
            if now > due + period {
                self.stall_ticks += 1;
            }
            debug_assert_eq!(bt, t, "bundles arrive in tick order");
            let (_, mut cmds) = self.queue.pop_front().expect("front exists");
            if self.record {
                for c in &cmds {
                    self.executed.push(Executed {
                        tick: t,
                        player: c.player.0,
                        seq: c.seq,
                        surrender: c.kind == CommandKind::Surrender,
                    });
                }
            }
            if let Some((at, slot)) = self.inject_desync
                && at == t
                && !self.injected
                && let Some(c) = desync_command(world, slot)
            {
                cmds.push(c);
                self.injected = true;
            }
            if let Some(r) = self.recorder.as_mut() {
                r.before_step(world, &cmds);
            }
            let t0 = Instant::now();
            let published = world.step(&cmds);
            if self.record {
                self.step_us
                    .push(t0.elapsed().as_micros().min(u32::MAX as u128) as u32);
            }
            if published.is_some()
                && let Some((tick, hash)) = world.published_hash()
            {
                on_hash(tick, hash);
                if self.record {
                    self.published.push((tick, hash));
                }
            }
            if self.ring.len() == HASH_RING {
                self.ring.pop_front();
            }
            self.ring.push_back((world.tick(), world.state_hash()));
            self.last_step = Some(now);
            n += 1;
        }
        n
    }
}

// ---------------------------------------------------------------- setups --

/// The `mods/ww2` directory: `arg`, else `./mods/ww2`, else the copy in the
/// source tree this crate was built from.
pub fn mods_dir(arg: Option<&Path>) -> anyhow::Result<PathBuf> {
    if let Some(p) = arg {
        return Ok(p.to_owned());
    }
    let here = PathBuf::from("mods/ww2");
    if here.join("mod.toml").is_file() {
        return Ok(here);
    }
    let tree = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    if tree.join("mod.toml").is_file() {
        return Ok(tree);
    }
    anyhow::bail!("cannot find mods/ww2; pass --mods <DIR>")
}

pub fn load_defs(dir: &Path) -> anyhow::Result<ModDefs> {
    ModDefs::load(dir).map_err(|e| anyhow::anyhow!("{e}"))
}

/// A two-player Kessel Ridge setup: slot 0 Germany, slot 1 USA. `ai` puts
/// the sim's AI commander on both slots as well, so a scripted test gets a
/// busy, realistic match (the network carries the scripted commands next
/// to it); a real net match has `ai = false`. `tickets` raises both
/// players' tickets so a long soak does not end early.
pub fn net_setup(
    defs: &ModDefs,
    seed: u64,
    ai: bool,
    tickets: Option<u32>,
) -> anyhow::Result<MatchSetup> {
    let req = MatchRequest {
        rules: "ww2:skirmish".into(),
        map: "ww2:kessel_ridge".into(),
        seed,
        players: vec![
            PlayerRequest {
                faction: "ww2:germany".into(),
                ai,
                difficulty: Difficulty::Normal,
            },
            PlayerRequest {
                faction: "ww2:usa".into(),
                ai,
                difficulty: Difficulty::Normal,
            },
        ],
    };
    let mut setup = defs.resolve(&req).map_err(|e| anyhow::anyhow!("{e}"))?;
    if let Some(t) = tickets {
        setup.rules.tickets = t;
    }
    Ok(setup)
}

/// Faction id of `slot` in `setup`.
pub fn faction_of(setup: &MatchSetup, slot: usize) -> String {
    let f = setup.players[slot].faction as usize;
    setup.factions[f].id.clone()
}

// --------------------------------------------------------------- scripts --

/// Scripted orders for one player: every 0.3–2.5 s one order (move,
/// attack-move, stop, retreat, regroup, call-in, stance, men orders), and
/// now and then a burst of 20 stops, so turns overflow the 16-command
/// quota and roll over. Seeded, so a run repeats.
pub struct Scripter {
    rng: SplitMix,
    slot: u8,
    next_at: u64,
    pub issued: u64,
}

impl Scripter {
    pub fn new(seed: u64, slot: u8) -> Scripter {
        Scripter {
            rng: SplitMix::new(seed.wrapping_mul(0x9e37_79b9).wrapping_add(slot as u64 + 1)),
            slot,
            next_at: 0,
            issued: 0,
        }
    }

    /// Appends the orders due at `now` (any clock, µs).
    pub fn tick(&mut self, now: u64, world: &World, out: &mut Vec<CommandKind>) {
        if now < self.next_at {
            return;
        }
        let first = self.next_at == 0;
        self.next_at = now + 300_000 + self.rng.below(2_200_000);
        if first {
            return;
        }
        let Some(game) = world.game() else { return };
        let setup = game.setup();
        let size = setup.map.size_m.max(8);
        let squads: Vec<_> = game
            .squads()
            .iter()
            .filter(|(_, s)| s.owner.0 == self.slot && !s.members.is_empty())
            .map(|(id, s)| (*id, s.members.clone()))
            .collect();
        let pos = |r: &mut SplitMix| {
            FxVec2::from_int(r.below(size as u64) as i32, r.below(size as u64) as i32)
        };
        let roll = self.rng.below(100);
        let before = out.len();
        if roll < 10 || squads.is_empty() {
            let f = setup.players[self.slot as usize].faction as usize;
            let roster = &setup.factions[f].roster;
            if !roster.is_empty() {
                let k = self.rng.below(roster.len() as u64) as usize;
                out.push(CommandKind::CallIn {
                    squad_type: roster[k],
                });
            }
        } else {
            let (squad, members) = squads[self.rng.below(squads.len() as u64) as usize].clone();
            let men = |r: &mut SplitMix| {
                let n = 1 + r.below(members.len().min(3) as u64) as usize;
                members[..n].to_vec()
            };
            match roll {
                10..40 => out.push(CommandKind::Move {
                    squad,
                    target: pos(&mut self.rng),
                }),
                40..60 => out.push(CommandKind::AttackMove {
                    squad,
                    target: pos(&mut self.rng),
                }),
                60..68 => out.push(CommandKind::Stop { squad }),
                68..72 => out.push(CommandKind::Retreat { squad }),
                72..77 => out.push(CommandKind::Regroup { squad }),
                77..85 => {
                    let stance = match self.rng.below(4) {
                        0 => StanceOrder::Auto,
                        1 => StanceOrder::Stand,
                        2 => StanceOrder::Crouch,
                        _ => StanceOrder::Prone,
                    };
                    out.push(CommandKind::SetStance {
                        units: men(&mut self.rng),
                        stance,
                    });
                }
                85..95 => out.push(CommandKind::MoveUnits {
                    units: men(&mut self.rng),
                    target: pos(&mut self.rng),
                }),
                _ => {
                    for _ in 0..20 {
                        out.push(CommandKind::Stop { squad });
                    }
                }
            }
        }
        self.issued += (out.len() - before) as u64;
    }
}

// ----------------------------------------------------------- virtual run --

/// A two-client match on a virtual clock. Times are µs; "rel" times count
/// from the moment turn 0 opens.
#[derive(Clone, Debug)]
pub struct VirtualSpec {
    pub seed: u64,
    /// Game time to play, from turn 0.
    pub seconds: u64,
    pub link: LinkParams,
    /// `(slot, from_rel_us, len_us)`: nothing reaches or leaves that
    /// slot's client.
    pub blackouts: Vec<(usize, u64, u64)>,
    /// `(slot, at_rel_us)`: the client's source address changes.
    pub rebind: Option<(usize, u64)>,
    /// `(slot, tick)`: the desync hook on that slot's client.
    pub inject_desync: Option<(usize, u64)>,
    pub script: bool,
    /// No new scripted orders in the last this-many µs.
    pub quiet_tail_us: u64,
    /// Unauthenticated garbage datagrams per second, to the relay and to
    /// both clients (random bytes, random kinds, wrong tokens).
    pub garbage_per_s: u32,
    pub relay: RelayConfig,
    pub d_override: Option<u8>,
    /// Sim worker threads per world (results never depend on it).
    pub threads: usize,
    /// A client sends `Bye(MatchOver)` once its world has a winner.
    pub bye_on_end: bool,
    /// Stop early once a desync was detected and both worlds reached the
    /// pause tick.
    pub stop_on_desync: bool,
    /// Keep checkpoints, and on a desync write each client's dump to
    /// `<dir>/slot<N>/` and the relay log to `<dir>/relay.jsonl`.
    pub dump_dir: Option<PathBuf>,
}

impl Default for VirtualSpec {
    fn default() -> VirtualSpec {
        VirtualSpec {
            seed: 1,
            seconds: 60,
            link: LinkParams {
                delay_us: 10_000,
                ..LinkParams::default()
            },
            blackouts: Vec::new(),
            rebind: None,
            inject_desync: None,
            script: true,
            quiet_tail_us: 5_000_000,
            garbage_per_s: 0,
            relay: RelayConfig::default(),
            d_override: None,
            threads: 1,
            bye_on_end: true,
            stop_on_desync: true,
            dump_dir: None,
        }
    }
}

pub struct VirtualReport {
    pub d: u8,
    /// Relay time turn 0 opened.
    pub start_at: u64,
    pub ticks: [u64; 2],
    /// Published hashes both worlds have, and how many differ.
    pub hash_checks: u64,
    pub hash_mismatches: u64,
    pub first_mismatch: Option<u64>,
    pub stamped: [u64; 2],
    pub queued: [u64; 2],
    /// Commands both worlds ran, per player, and problems found with them.
    pub executed: [Vec<Executed>; 2],
    /// Own seqs stamped but never run (for players still connected).
    pub lost: [u64; 2],
    /// `(player, seq)` run more than once or out of order.
    pub order_errors: u64,
    /// The two worlds ran different command streams.
    pub streams_differ: bool,
    pub relay_restamps: u64,
    pub relay_commands: u64,
    pub own_restamps: [u64; 2],
    pub stall_ticks: [u64; 2],
    pub desync: [Option<(u64, u64)>; 2],
    pub relay_desync: Option<u64>,
    pub byes: [Option<ByeReason>; 2],
    pub outcomes: [Outcome; 2],
    /// Players whose `Surrender` the relay injected and world 0 ran.
    pub injected_surrenders: Vec<u8>,
    pub pauses: u32,
    pub resumes: u32,
    pub rtt_p99_ms: [Option<u16>; 2],
    /// Each client's playout delay in ticks.
    pub playout: [u64; 2],
    pub net: SimNetStats,
    pub step_p99_us: u32,
    pub step_max_us: u32,
    pub relay_log: Vec<LogRecord>,
    pub worlds: Vec<World>,
    pub rings: [VecDeque<(u64, u64)>; 2],
}

impl VirtualReport {
    pub fn restamp_rate(&self) -> f64 {
        if self.relay_commands == 0 {
            0.0
        } else {
            self.relay_restamps as f64 / self.relay_commands as f64
        }
    }
}

struct Peer {
    sock: SimSocket,
    client: NetClient,
    world: World,
    driver: SimDriver,
    script: Scripter,
    skew_us: i64,
    ppm: i64,
    welcomed: bool,
    readied: bool,
    queued: u64,
    bye: Option<ByeReason>,
    said_bye: bool,
    pauses: u32,
    resumes: u32,
}

impl Peer {
    /// This client's local clock at virtual time `now` (skewed, drifting).
    fn local(&self, now: u64) -> u64 {
        (now as i64 + self.skew_us + now as i64 / 1_000_000 * self.ppm) as u64
    }
}

/// Random unauthenticated junk: raw bytes, or a valid-looking header of a
/// random kind with a random (never a real) token and a random body.
fn garbage(r: &mut SplitMix) -> Vec<u8> {
    let len = r.below(1400) as usize;
    let mut v: Vec<u8> = (0..len).map(|_| r.next_u64() as u8).collect();
    if r.below(2) == 0 && v.len() >= 15 {
        v[..4].copy_from_slice(b"FLNT");
        v[4..6].copy_from_slice(&crate::PROTOCOL_VERSION.to_le_bytes());
        v[6] = r.below(13) as u8;
        if r.below(3) == 0 {
            v[7..15].copy_from_slice(&0u64.to_le_bytes());
            if v.len() < crate::protocol::HELLO_MIN_LEN {
                v.resize(crate::protocol::HELLO_MIN_LEN, 0);
            }
        }
    }
    v.truncate(MAX_DATAGRAM + 100);
    v
}

/// Plays one match. Panics only on harness bugs (a setup the sim refuses).
pub fn run_virtual(setup: &MatchSetup, spec: &VirtualSpec) -> VirtualReport {
    let net = SimNet::new(spec.seed, spec.link.clone());
    let relay_addr: SocketAddr = "10.0.0.1:47015".parse().expect("addr");
    let mut relay_sock = net.socket(relay_addr);
    let mut relay = Relay::new(spec.relay.clone(), TokenSource::seeded(spec.seed));
    let mut rng = SplitMix::new(spec.seed ^ 0xfee1);
    let hash = crate::setup_hash(setup);
    let skews = [(7_000_000i64, 40i64), (1_500_000, -30)];
    let mut peers: Vec<Peer> = (0..2)
        .map(|i| {
            let addr: SocketAddr = format!("10.0.0.{}:40000", i + 2).parse().expect("addr");
            let world = World::new_match(setup.clone(), spec.threads.max(1))
                .expect("the setup builds a world");
            let mut driver = SimDriver::new(true);
            if spec.dump_dir.is_some() {
                driver.recorder = Some(crate::dump::DesyncRecorder::new());
            }
            Peer {
                sock: net.socket(addr),
                client: NetClient::new(ClientConfig {
                    relay: relay_addr,
                    name: format!("p{i}"),
                    build_id: crate::build_id(),
                    content_hash: 0,
                    nonce: rng.next_u64(),
                }),
                world,
                driver,
                script: Scripter::new(spec.seed, i as u8),
                skew_us: skews[i].0,
                ppm: skews[i].1,
                welcomed: false,
                readied: false,
                queued: 0,
                bye: None,
                said_bye: false,
                pauses: 0,
                resumes: 0,
            }
        })
        .collect();
    let attacker: SocketAddr = "10.0.0.66:666".parse().expect("addr");
    let mut buf = vec![0u8; RECV_BUF];
    let mut both_joined: Option<u64> = None;
    let mut start_at: Option<u64> = None;
    let mut relay_log = Vec::new();
    let mut scripted: Vec<CommandKind> = Vec::new();
    let dt = 1_000u64;
    let mut now = 0u64;
    // The lobby has 60 s to start the match.
    let lobby_limit = 60_000_000;
    loop {
        now += dt;
        net.set_now(now);
        if start_at.is_none() && now > lobby_limit {
            break;
        }
        if let Some(s) = start_at {
            if now >= s + spec.seconds * 1_000_000 {
                break;
            }
            if spec.stop_on_desync
                && peers
                    .iter()
                    .all(|p| p.driver.desync.is_some_and(|(_, at)| p.world.tick() >= at))
            {
                break;
            }
        }
        // Relay in.
        while let Ok(Some((n, from))) = relay_sock.recv_from(&mut buf) {
            relay.on_datagram(from, &buf[..n], now);
        }
        if spec.garbage_per_s > 0 && rng.below(1000) < spec.garbage_per_s as u64 {
            let g = garbage(&mut rng);
            relay.on_datagram(attacker, &g, now);
            for p in peers.iter_mut() {
                let g = garbage(&mut rng);
                let local = p.local(now);
                p.client.on_datagram(relay_addr, &g, local);
            }
        }
        relay.poll(now);
        for (to, bytes) in relay.drain_out() {
            let _ = relay_sock.send_to(&bytes, to);
        }
        relay_log.extend(relay.drain_log());
        // Clients.
        for p in peers.iter_mut() {
            let local = p.local(now);
            while let Ok(Some((n, from))) = p.sock.recv_from(&mut buf) {
                p.client.on_datagram(from, &buf[..n], local);
            }
            p.client.poll(local);
            let events: Vec<NetEvent> = p.client.drain_events().collect();
            for ev in &events {
                p.driver.on_event(ev);
                match ev {
                    NetEvent::Welcome { slot } => {
                        p.welcomed = true;
                        let s = *slot as usize;
                        if let Some((c, t)) = spec.inject_desync
                            && c == s
                        {
                            p.driver.inject_desync = Some((t, *slot));
                        }
                        p.client.set_lobby(LobbySet {
                            faction: faction_of(setup, s),
                            seed: (s == 0).then_some(setup.seed),
                            map: None,
                            d: if s == 0 { spec.d_override } else { None },
                        });
                    }
                    NetEvent::Start(s) => {
                        start_at.get_or_insert(s.start_at);
                    }
                    NetEvent::Paused { .. } => p.pauses += 1,
                    NetEvent::Resume { .. } => p.resumes += 1,
                    NetEvent::Bye(r) => p.bye = Some(*r),
                    _ => {}
                }
            }
            let period = p.client.schedule().map_or(33_333, |s| s.period());
            p.driver.confirm(p.client.confirmed());
            let (client, world, driver) = (&mut p.client, &mut p.world, &mut p.driver);
            let mut hashes = Vec::new();
            driver.advance(
                world,
                local,
                period,
                |t| client.step_slot(t),
                |t, h| hashes.push((t, h)),
            );
            for (t, h) in hashes {
                client.send_hash(t, h);
            }
            if let Some(s) = start_at
                && spec.script
                && client.phase() == ClientPhase::Running
                && now + spec.quiet_tail_us < s + spec.seconds * 1_000_000
                && world
                    .game()
                    .is_some_and(|g| g.outcome() == Outcome::Running)
            {
                scripted.clear();
                p.script.tick(now, world, &mut scripted);
                for k in scripted.drain(..) {
                    if client.send_command(k) {
                        p.queued += 1;
                    }
                }
            }
            if spec.bye_on_end
                && !p.said_bye
                && world
                    .game()
                    .is_some_and(|g| g.outcome() != Outcome::Running)
            {
                p.said_bye = true;
                client.bye(ByeReason::MatchOver);
            }
            for (to, bytes) in client.drain_out() {
                let _ = p.sock.send_to(&bytes, to);
            }
        }
        if both_joined.is_none() && peers.iter().all(|p| p.welcomed) {
            both_joined = Some(now);
        }
        // Ready once both joined and pings had 1.5 s to size D.
        if let Some(j) = both_joined
            && now >= j + 1_500_000
        {
            for p in peers.iter_mut().filter(|p| !p.readied) {
                p.readied = true;
                p.client.ready(hash);
            }
        }
        // Scheduled faults, relative to turn 0.
        if let Some(s) = start_at {
            let of_slot = |c: usize| {
                peers
                    .iter()
                    .position(|p| p.client.slot() == Some(c as u8))
                    .expect("both slots are taken once the match started")
            };
            for &(c, from, len) in &spec.blackouts {
                if now == (s + from).div_ceil(dt) * dt {
                    let a = peers[of_slot(c)].sock.addr();
                    net.blackout(a, s + from, s + from + len);
                }
            }
            if let Some((c, at)) = spec.rebind
                && now == (s + at).div_ceil(dt) * dt
            {
                let p = &peers[of_slot(c)];
                let old = p.sock.addr();
                let new: SocketAddr = SocketAddr::new(old.ip(), old.port().wrapping_add(1000));
                net.rebind(&p.sock, new);
            }
        }
    }
    relay_log.extend(relay.drain_log());
    if let Some(dir) = &spec.dump_dir {
        write_dumps(dir, setup, &relay, &peers, &relay_log);
    }
    report(setup, spec, &relay, &net, peers, start_at, relay_log)
}

/// Each client's desync dump and the relay log, if a desync happened.
fn write_dumps(dir: &Path, setup: &MatchSetup, relay: &Relay, peers: &[Peer], log: &[LogRecord]) {
    if relay.desync_tick().is_none() {
        return;
    }
    let _ = std::fs::create_dir_all(dir);
    if let Ok(mut f) = std::fs::File::create(dir.join("relay.jsonl")) {
        use std::io::Write as _;
        for rec in log {
            if let Ok(line) = serde_json::to_string(rec) {
                let _ = writeln!(f, "{line}");
            }
        }
    }
    for p in peers {
        let (Some(slot), Some(rec), Some((tick, _))) =
            (p.client.slot(), p.driver.recorder.as_ref(), p.driver.desync)
        else {
            continue;
        };
        let hashes = log
            .iter()
            .find_map(|l| match l {
                LogRecord::Desync { hashes, .. } => Some(hashes.clone()),
                _ => None,
            })
            .unwrap_or_default();
        let manifest = crate::dump::DumpManifest {
            build_id: crate::build_id(),
            content_hash: "0000000000000000".into(),
            setup_hash: format!("{:016x}", crate::setup_hash(setup)),
            seed: setup.seed,
            d: relay.d(),
            slot,
            desync_tick: tick,
            hashes,
            ..Default::default()
        };
        let _ = rec.write_dump(
            &dir.join(format!("slot{slot}")),
            &p.world,
            manifest,
            &p.driver.ring,
        );
    }
}

fn report(
    _setup: &MatchSetup,
    _spec: &VirtualSpec,
    relay: &Relay,
    net: &SimNet,
    peers: Vec<Peer>,
    start_at: Option<u64>,
    relay_log: Vec<LogRecord>,
) -> VirtualReport {
    // Index everything by slot (who joined first is up to the network).
    let mut peers = peers;
    peers.sort_by_key(|p| p.client.slot().unwrap_or(u8::MAX));
    let h0: BTreeMap<u64, u64> = peers[0].driver.published.iter().copied().collect();
    let mut checks = 0;
    let mut mismatches = 0;
    let mut first = None;
    for (t, h) in &peers[1].driver.published {
        if let Some(h0) = h0.get(t) {
            checks += 1;
            if h0 != h {
                mismatches += 1;
                first.get_or_insert(*t);
            }
        }
    }
    let common = peers[0].world.tick().min(peers[1].world.tick());
    let stream = |p: &Peer| -> Vec<Executed> {
        p.driver
            .executed
            .iter()
            .copied()
            .filter(|e| e.tick < common)
            .collect()
    };
    let streams_differ = stream(&peers[0]) != stream(&peers[1]);
    // Use the world that ran furthest to judge loss and order.
    let far = if peers[0].world.tick() >= peers[1].world.tick() {
        0
    } else {
        1
    };
    let mut lost = [0u64; 2];
    let mut order_errors = 0u64;
    let mut executed: [Vec<Executed>; 2] = [Vec::new(), Vec::new()];
    let mut injected = Vec::new();
    for pl in 0..2u8 {
        let stamped = peers[pl as usize].client.status().stamped;
        let mine: Vec<Executed> = peers[far]
            .driver
            .executed
            .iter()
            .copied()
            .filter(|e| e.player == pl)
            .collect();
        let mut expect = 0u32;
        for e in &mine {
            if e.seq != expect {
                order_errors += 1;
            }
            expect = e.seq.wrapping_add(1);
            // Scripts never surrender: any `Surrender` came from the relay.
            if e.surrender {
                injected.push(pl);
            }
        }
        let ran_own = mine.iter().filter(|e| (e.seq as u64) < stamped).count() as u64;
        let connected = relay.slot_state(pl) == Some(crate::relay::SlotState::Connected)
            || peers[pl as usize].said_bye;
        if connected {
            lost[pl as usize] = stamped.saturating_sub(ran_own);
        }
        executed[pl as usize] = mine;
    }
    let mut steps: Vec<u32> = peers
        .iter()
        .flat_map(|p| p.driver.step_us.iter().copied())
        .collect();
    steps.sort_unstable();
    let p99 = steps
        .get((steps.len() * 99 / 100).min(steps.len().saturating_sub(1)))
        .copied()
        .unwrap_or(0);
    let max = steps.last().copied().unwrap_or(0);
    let status: Vec<_> = peers.iter().map(|p| p.client.status()).collect();
    let outcome = |p: &Peer| p.world.game().map_or(Outcome::Running, |g| g.outcome());
    let mut rings = [VecDeque::new(), VecDeque::new()];
    let mut worlds = Vec::new();
    let mut byes = [None, None];
    let mut desync = [None, None];
    let mut ticks = [0; 2];
    let mut stalls = [0; 2];
    let mut queued = [0; 2];
    let outcomes = [outcome(&peers[0]), outcome(&peers[1])];
    let (pauses, resumes) = (peers[0].pauses, peers[0].resumes);
    for (i, p) in peers.into_iter().enumerate() {
        rings[i] = p.driver.ring;
        byes[i] = p.bye;
        desync[i] = p.driver.desync;
        ticks[i] = p.world.tick();
        stalls[i] = p.driver.stall_ticks;
        queued[i] = p.queued;
        worlds.push(p.world);
    }
    VirtualReport {
        d: relay.d(),
        start_at: start_at.unwrap_or(0),
        ticks,
        hash_checks: checks,
        hash_mismatches: mismatches,
        first_mismatch: first,
        stamped: [status[0].stamped, status[1].stamped],
        queued,
        executed,
        lost,
        order_errors,
        streams_differ,
        relay_restamps: relay.restamps(),
        relay_commands: relay.status(0).commands,
        own_restamps: [status[0].own_restamps, status[1].own_restamps],
        stall_ticks: stalls,
        desync,
        relay_desync: relay.desync_tick(),
        byes,
        outcomes,
        injected_surrenders: injected,
        pauses,
        resumes,
        rtt_p99_ms: [status[0].rtt_p99_ms, status[1].rtt_p99_ms],
        playout: [status[0].playout, status[1].playout],
        net: net.stats(),
        step_p99_us: p99,
        step_max_us: max,
        relay_log,
        worlds,
        rings,
    }
}

// --------------------------------------------------------------- UDP run --

/// Plays one match in real time over UDP loopback: a [`RelayServer`]
/// thread on `127.0.0.1`, two client net threads whose sockets sit behind
/// an [`Impaired`] link with `spec.link`, and both worlds stepped on the
/// calling thread. Blackouts, rebinding, garbage and desync injection are
/// virtual-run features and are ignored here.
///
/// [`RelayServer`]: crate::server::RelayServer
/// [`Impaired`]: crate::transport::Impaired
pub fn run_udp(setup: &MatchSetup, spec: &VirtualSpec) -> anyhow::Result<VirtualReport> {
    use crate::client_io::{NetHandle, ToNet, client_bind_for};
    use crate::server::RelayServer;
    use crate::transport::{Impaired, UdpTransport};

    let mut server = RelayServer::spawn(spec.relay.clone(), "127.0.0.1:0".parse()?, None)?;
    let relay_addr = server.local_addr();
    let hash = crate::setup_hash(setup);
    let mut rng = SplitMix::new(spec.seed ^ 0x0d0);
    let mut handles = Vec::new();
    let mut worlds = Vec::new();
    let mut drivers = Vec::new();
    let mut scripts = Vec::new();
    for i in 0..2 {
        let sock = UdpTransport::bind(client_bind_for(relay_addr))?;
        let link = Impaired::new(sock, spec.link.clone(), spec.seed.wrapping_add(i));
        handles.push(NetHandle::spawn_with(
            ClientConfig {
                relay: relay_addr,
                name: format!("p{i}"),
                build_id: crate::build_id(),
                content_hash: 0,
                nonce: rng.next_u64(),
            },
            Box::new(link),
        )?);
        worlds.push(World::new_match(setup.clone(), spec.threads.max(1))?);
        drivers.push(SimDriver::new(true));
        scripts.push(Scripter::new(spec.seed, i as u8));
    }
    let mut slots: [Option<u8>; 2] = [None, None];
    let mut readied = false;
    let mut joined_at: Option<Instant> = None;
    let mut started: Option<Instant> = None;
    let mut byes = [None, None];
    let mut pauses = 0;
    let mut resumes = 0;
    let mut queued = [0u64; 2];
    let mut events = Vec::new();
    let mut scripted = Vec::new();
    let t0 = Instant::now();
    let countdown = std::time::Duration::from_micros(spec.relay.countdown_us);
    let length = std::time::Duration::from_secs(spec.seconds);
    let tail = std::time::Duration::from_micros(spec.quiet_tail_us);
    loop {
        if started.is_none() && t0.elapsed().as_secs() > 60 {
            anyhow::bail!("the lobby did not start within 60 s");
        }
        if started.is_some_and(|s| s.elapsed() >= countdown + length) {
            break;
        }
        for i in 0..2 {
            let h = &handles[i];
            events.clear();
            h.drain_events(&mut events);
            for ev in &events {
                drivers[i].on_event(ev);
                match ev {
                    NetEvent::Welcome { slot } => {
                        slots[i] = Some(*slot);
                        let s = *slot as usize;
                        h.send(ToNet::Lobby(LobbySet {
                            faction: faction_of(setup, s),
                            seed: (s == 0).then_some(setup.seed),
                            map: None,
                            d: if s == 0 { spec.d_override } else { None },
                        }));
                    }
                    NetEvent::Start(_) => {
                        started.get_or_insert_with(Instant::now);
                    }
                    NetEvent::Paused { .. } if i == 0 => pauses += 1,
                    NetEvent::Resume { .. } if i == 0 => resumes += 1,
                    NetEvent::Bye(r) => byes[i] = Some(*r),
                    NetEvent::Rejected(r) => anyhow::bail!("relay rejected client {i}: {r:?}"),
                    _ => {}
                }
            }
            let status = h.status();
            let period = status.schedule.map_or(33_333, |s| s.period());
            drivers[i].advance(
                &mut worlds[i],
                h.now_us(),
                period,
                |t| status.step_slot(t),
                |tick, hash| h.send(ToNet::Hash { tick, hash }),
            );
            if spec.script
                && let Some(s) = started
                && s.elapsed() + tail < countdown + length
                && worlds[i]
                    .game()
                    .is_some_and(|g| g.outcome() == Outcome::Running)
            {
                scripted.clear();
                scripts[i].tick(h.now_us(), &worlds[i], &mut scripted);
                for k in scripted.drain(..) {
                    h.send(ToNet::Command(k));
                    queued[i] += 1;
                }
            }
        }
        if joined_at.is_none() && slots.iter().all(|s| s.is_some()) {
            joined_at = Some(Instant::now());
        }
        if !readied && joined_at.is_some_and(|j| j.elapsed().as_millis() >= 1500) {
            readied = true;
            for h in &handles {
                h.send(ToNet::Ready(hash));
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let relay = server.status();
    let statuses: Vec<_> = handles.iter().map(|h| h.status()).collect();
    for h in &mut handles {
        h.stop();
    }
    server.stop();
    // Index by slot.
    let mut order = [0usize, 1];
    if slots[0] == Some(1) {
        order = [1, 0];
    }
    let h0: BTreeMap<u64, u64> = drivers[order[0]].published.iter().copied().collect();
    let (mut checks, mut mismatches, mut first) = (0, 0, None);
    for (t, h) in &drivers[order[1]].published {
        if let Some(a) = h0.get(t) {
            checks += 1;
            if a != h {
                mismatches += 1;
                first.get_or_insert(*t);
            }
        }
    }
    let far = if worlds[order[0]].tick() >= worlds[order[1]].tick() {
        order[0]
    } else {
        order[1]
    };
    let mut lost = [0; 2];
    let mut order_errors = 0;
    let mut executed: [Vec<Executed>; 2] = [Vec::new(), Vec::new()];
    for pl in 0..2 {
        let stamped = statuses[order[pl]].stamped;
        let mine: Vec<Executed> = drivers[far]
            .executed
            .iter()
            .copied()
            .filter(|e| e.player == pl as u8)
            .collect();
        let mut expect = 0;
        for e in &mine {
            if e.seq != expect {
                order_errors += 1;
            }
            expect = e.seq.wrapping_add(1);
        }
        lost[pl] = stamped.saturating_sub(mine.len() as u64);
        executed[pl] = mine;
    }
    let mut steps: Vec<u32> = drivers
        .iter()
        .flat_map(|d| d.step_us.iter().copied())
        .collect();
    steps.sort_unstable();
    let pick = |q: usize| {
        steps
            .get((steps.len() * q / 100).min(steps.len().saturating_sub(1)))
            .copied()
            .unwrap_or(0)
    };
    let outcome = |w: &World| w.game().map_or(Outcome::Running, |g| g.outcome());
    Ok(VirtualReport {
        d: relay.d,
        start_at: 0,
        ticks: [worlds[order[0]].tick(), worlds[order[1]].tick()],
        hash_checks: checks,
        hash_mismatches: mismatches,
        first_mismatch: first,
        stamped: [statuses[order[0]].stamped, statuses[order[1]].stamped],
        queued: [queued[order[0]], queued[order[1]]],
        executed,
        lost,
        order_errors,
        streams_differ: false,
        relay_restamps: relay.restamps,
        relay_commands: relay.commands,
        own_restamps: [
            statuses[order[0]].own_restamps,
            statuses[order[1]].own_restamps,
        ],
        stall_ticks: [drivers[order[0]].stall_ticks, drivers[order[1]].stall_ticks],
        desync: [drivers[order[0]].desync, drivers[order[1]].desync],
        relay_desync: relay.desync_tick,
        byes: [byes[order[0]], byes[order[1]]],
        outcomes: [outcome(&worlds[order[0]]), outcome(&worlds[order[1]])],
        injected_surrenders: Vec::new(),
        pauses,
        resumes,
        rtt_p99_ms: [statuses[order[0]].rtt_p99_ms, statuses[order[1]].rtt_p99_ms],
        playout: [statuses[order[0]].playout, statuses[order[1]].playout],
        net: SimNetStats::default(),
        step_p99_us: pick(99),
        step_max_us: steps.last().copied().unwrap_or(0),
        relay_log: Vec::new(),
        rings: [
            std::mem::take(&mut drivers[order[0]].ring),
            std::mem::take(&mut drivers[order[1]].ring),
        ],
        worlds: {
            let mut w = worlds;
            if order[0] == 1 {
                w.swap(0, 1);
            }
            w
        },
    })
}
