//! The wire protocol (S2 "Protocol"; the reference is
//! `sdk/docs/net/relay-protocol.md`, which must change with this file).
//!
//! Every datagram is a fixed 15-byte [`Header`] (`b"FLNT"`, protocol
//! version, kind, token) and a postcard body, at most [`MAX_DATAGRAM`]
//! bytes in all. Decoding is bounded: the length is checked before postcard
//! runs, every list and string is checked against its limit afterwards
//! ([`Packet::validate`]), and no input can panic. A datagram that fails any
//! check is dropped whole.
//!
//! Every change to the header, a kind number or a body layout bumps
//! [`PROTOCOL_VERSION`]. Only the header and the [`Reject`] body's first
//! field are frozen across versions, so a client can always say "the relay
//! speaks protocol N".

use frontline_sim::{Command, CommandKind, MAX_UNITS, game::MAX_DIG_POINTS};
use serde::{Deserialize, Serialize};

/// Wire version. Bump on any change to a body, a kind or a limit that a
/// peer could observe. 4 (S3b): `CallIn` buys and spawns at once under a
/// per-type re-call cooldown instead of joining a queue (the one bump for
/// the S3b slice; the Suppress command, `SuppressUnits`, lands under the same
/// version). 5 (S3c): the `Recrew` and `Abandon` commands (the one bump for
/// the S3c slice). 6 (S4): unit ids per command 120 -> 150 (`MAX_UNITS`,
/// the S4 pop cap; the one bump for the S4 slice). 7 (S5a): `SetFire`. 8
/// (S5b): `Barrage` becomes `FireMission` (same index and fields, new
/// meaning; the one bump for the S5b slice). 9 (S6): `Build`,
/// `IssueCrate`, `PlaceCrate`, `PickUpCrate` and the pit `DigProfile`s
/// (the one bump for the S6 slice). 10 (S8a): `Embark` and `Disembark`
/// (the one bump for the S8a slice). 11 (S8b): `DropCrates` (the one bump
/// for the S8b slice). 12 (S9): `Emplace`, `PackGun`, `PushGun`, `Hitch`,
/// `Unhitch`, `SetAmbush` and the gun pit `DigProfile` (the one bump for
/// the S9 slice). 13 (S10a): `Fortify`, `LoadMaterials`, `CutWire`,
/// `PlaceBangalore`, `SupplyKind::Construction`, `ItemKind::Bangalore`
/// and the two S10a pit `DigProfile`s (the one bump for the S10a slice).
/// 14 (S10b): `ClearMines`, `Demolish`, `ItemKind::Mine` and the bunker pit
/// `DigProfile` (the one bump for the S10b slice). 15 (S11b):
/// `ItemKind::RifleGrenade` (6) in `Throw` / `ThrowEach` (never thrown:
/// the sim refuses it; the one bump for the S11b slice). 16 (S12): `Tend`,
/// `Carry`, `Evacuate`, `Reinforce` (refused by the sim until S12a Build
/// B) and `StockKind::Aid` in the setup (the one bump for the S12 slice).
/// 17 (S14): `ClearTrench`, `Strike` (refused by the sim until S14a Builds
/// B and C) and the S14 setup keys (the one bump for the S14 slice).
pub const PROTOCOL_VERSION: u16 = 17;
pub const MAGIC: [u8; 4] = *b"FLNT";
/// Magic, version (u16 LE), kind (u8), token (u64 LE).
pub const HEADER_LEN: usize = 15;
/// Largest datagram either side sends or accepts. Below the 1,280-byte
/// IPv6 minimum MTU with room for IP/UDP headers and tunnels (Tailscale).
pub const MAX_DATAGRAM: usize = 1200;
/// `Hello` datagrams are padded to at least this size, and the relay drops
/// shorter ones: no pre-auth reply is larger than its request, so the relay
/// cannot amplify a spoofed-source flood.
pub const HELLO_MIN_LEN: usize = 512;
/// Default UDP port (S2 "Transport"). 27015 is avoided: Source servers.
pub const DEFAULT_PORT: u16 = 47015;
/// Player slots in S2 (1v1).
pub const SLOTS: usize = 2;

/// Longest player name, in bytes of UTF-8.
pub const MAX_NAME: usize = 24;
/// Longest def id (faction, map), build id or free-text note.
pub const MAX_ID: usize = 64;
/// Longest note in a lobby state or a reject.
pub const MAX_NOTE: usize = 160;
/// Own commands per turn; the client rolls the rest into the next turn.
pub const MAX_CMDS_PER_TURN: usize = 16;
/// Encoded bytes of own commands per turn (the client rolls excess over).
pub const MAX_TURN_BYTES: usize = 1024;
/// Commands the relay holds for one player before refusing more with
/// `Bye(Flood)`. An honest client never gets near it: it would take
/// 16 turns of full input arriving while 0 turns close.
pub const MAX_PENDING: usize = 256;
/// Most turns of own input in one `Input`.
pub const MAX_INPUT_TURNS: usize = 64;
/// Most turns in one `Bundle` datagram (an empty turn is about 4 bytes).
pub const MAX_BUNDLE_TURNS: usize = 300;
/// Most commands in one bundled turn: both players' turn quota plus one
/// relay-injected `Surrender` each.
pub const MAX_TURN_CMDS: usize = SLOTS * (MAX_CMDS_PER_TURN + 1);
/// Input delay limits: the lobby suggestion stays in `MIN_D..=SUGGEST_MAX_D`,
/// slot 0 may override within `MIN_D..=MAX_D`.
pub const MIN_D: u8 = 2;
/// `Start` limits a client accepts: the match is 30 Hz at 100 %, and relay
/// tests run faster, but never past these.
pub const MAX_TICK_HZ: u32 = 120;
pub const MAX_SPEED_PCT: u16 = 1000;
pub const SUGGEST_MAX_D: u8 = 4;
pub const MAX_D: u8 = 6;

/// Datagram kinds (the header's `kind` byte). Numbers are part of the
/// protocol version.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Kind {
    Hello = 1,
    Welcome = 2,
    /// Frozen: kind 3 with a `u16` relay version first, in every version.
    Reject = 3,
    CtlUp = 4,
    CtlDown = 5,
    Ack = 6,
    Ping = 7,
    Pong = 8,
    Input = 9,
    Bundle = 10,
}

impl Kind {
    fn from_u8(b: u8) -> Option<Kind> {
        Some(match b {
            1 => Kind::Hello,
            2 => Kind::Welcome,
            3 => Kind::Reject,
            4 => Kind::CtlUp,
            5 => Kind::CtlDown,
            6 => Kind::Ack,
            7 => Kind::Ping,
            8 => Kind::Pong,
            9 => Kind::Input,
            10 => Kind::Bundle,
            _ => return None,
        })
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Header {
    pub version: u16,
    pub kind: u8,
    /// 0 before the relay assigned one (`Hello`, `Welcome`, `Reject`).
    pub token: u64,
}

impl Header {
    /// Reads the header without judging the version or the kind.
    pub fn read(bytes: &[u8]) -> Option<Header> {
        if bytes.len() < HEADER_LEN || bytes[..4] != MAGIC {
            return None;
        }
        Some(Header {
            version: u16::from_le_bytes([bytes[4], bytes[5]]),
            kind: bytes[6],
            token: u64::from_le_bytes(bytes[7..15].try_into().ok()?),
        })
    }

    pub(crate) fn write_to(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&self.version.to_le_bytes());
        out.push(self.kind);
        out.extend_from_slice(&self.token.to_le_bytes());
    }
}

// ------------------------------------------------------------- messages --

/// Client → relay, unauthenticated. Padded to [`HELLO_MIN_LEN`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Hello {
    /// Random per connection attempt; a resent `Hello` with the same nonce
    /// gets the same `Welcome`.
    pub nonce: u64,
    /// [`crate::build_id`]: commit, dirty flag, protocol and save versions.
    pub build_id: String,
    /// `ModDefs::content_hash` of the client's loaded mod.
    pub content_hash: u64,
    pub name: String,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Welcome {
    pub nonce: u64,
    pub token: u64,
    pub slot: u8,
}

/// Frozen layout start: `relay_version` stays the first field forever.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Reject {
    pub relay_version: u16,
    pub nonce: u64,
    pub reason: RejectReason,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum RejectReason {
    /// The client speaks another protocol version (header version).
    Version,
    /// The other player runs another build (`lobby` is theirs).
    Build { lobby: String },
    /// The other player loaded different defs.
    Content,
    /// Both slots are taken.
    Full,
    /// The match already started (no join-in-progress in S2).
    Started,
    /// The name is empty or too long.
    BadName,
}

/// A reliable, ordered control message, client → relay. Resent every
/// 100 ms until an [`Ack`] covers `cseq`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CtlUp {
    pub cseq: u32,
    pub msg: Up,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Up {
    /// Lobby choice. Slot 0 alone may set `seed`, `map` and `d`; the relay
    /// ignores them from slot 1. Any change clears both `Ready` flags.
    LobbySet(LobbySet),
    /// `xxh3(postcard(MatchSetup))` of the client's own resolve.
    Ready {
        setup_hash: u64,
    },
    /// A published sim hash: the world's hash after `tick` ticks.
    Hash {
        tick: u64,
        hash: u64,
    },
    PauseReq,
    ResumeReq,
    Bye(ByeReason),
}

#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct LobbySet {
    pub faction: String,
    pub seed: Option<u64>,
    pub map: Option<String>,
    /// `None`: use the relay's suggestion.
    pub d: Option<u8>,
}

/// A reliable, ordered control message, relay → client.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CtlDown {
    pub cseq: u32,
    pub msg: Down,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Down {
    Lobby(LobbyState),
    Start(Start),
    /// The relay stopped closing turns: turn `at` is the first one not
    /// closed. Clients step up to `at - 1` and wait.
    Paused {
        at: u64,
        cause: PauseCause,
    },
    /// Turn `at` closes at relay time `close_at` (µs), turn `at + k` one
    /// period per `k` later.
    Resume {
        at: u64,
        close_at: u64,
    },
    /// Hashes for `tick` differed. Turn `at` is not closed (as `Paused`);
    /// the match does not resume.
    Desync {
        tick: u64,
        hashes: [u64; 2],
        at: u64,
    },
    Bye(ByeReason),
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum PauseCause {
    /// That slot asked (`PauseReq`).
    Player(u8),
    /// That slot went silent for 10 s; the 60 s timeout runs.
    Waiting(u8),
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ByeReason {
    /// The client quit (mid-match: the relay closes a `Surrender` for it).
    Quit,
    /// The client saw the match end.
    MatchOver,
    /// Silent past the timeout; the relay closed a `Surrender` for it.
    TimedOut,
    /// Over [`MAX_PENDING`] commands waiting at the relay.
    Flood,
    /// The relay shut down.
    Shutdown,
}

#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct LobbyState {
    pub slots: [Option<SlotInfo>; 2],
    pub seed: u64,
    pub map: String,
    /// Slot 0's override, if any.
    pub d: Option<u8>,
    /// The relay's suggestion from both slots' pings (`client::suggest_d`).
    pub suggested_d: u8,
    /// Why the last Ready round failed (empty: none).
    pub note: String,
}

#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct SlotInfo {
    pub name: String,
    pub faction: String,
    pub ready: bool,
    pub rtt_ms: u16,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Start {
    /// Relay time (µs) at which turn 0 opens; it closes one period later.
    pub start_at: u64,
    pub tick_hz: u32,
    /// Turn pacing in percent of real time; 100 except in relay tests.
    pub speed_pct: u16,
    pub d: u8,
    pub seed: u64,
    pub map: String,
    pub factions: [String; 2],
    pub names: [String; 2],
}

/// Cumulative ack of the peer's control stream: every `cseq < next` was
/// processed.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Ack {
    pub next: u32,
}

/// Clock and RTT probe (every 1 s in a match, 4 Hz in the lobby).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Ping {
    /// Client clock (µs), echoed in the `Pong`.
    pub t_client: u64,
    /// The client's measured RTT P99 and jitter (µs), for the lobby's D
    /// suggestion and the peers readout. 0 while unknown.
    pub rtt_p99_us: u32,
    pub jitter_us: u32,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Pong {
    pub t_client: u64,
    /// Relay clock (µs) when the `Ping` was handled.
    pub t_relay: u64,
}

/// Own input plus the bundle ack, every period.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Input {
    pub ack: BundleAck,
    /// Every unacked turn of own input, oldest first, as many as fit.
    pub turns: Vec<InputTurn>,
}

/// Which bundles a client holds: every turn below `held`, and turn
/// `held + 1 + i` when bit `i` of `mask` is set (turn `held` itself is
/// missing by definition).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct BundleAck {
    pub held: u64,
    pub mask: u64,
}

impl BundleAck {
    pub fn holds(&self, tick: u64) -> bool {
        if tick < self.held {
            return true;
        }
        let i = tick - self.held;
        (1..=64).contains(&i) && self.mask & (1 << (i - 1)) != 0
    }
}

/// One stamped turn of own commands: seqs `seq0..seq0 + cmds.len()`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct InputTurn {
    pub target: u64,
    pub seq0: u32,
    pub cmds: Vec<CommandKind>,
}

/// Closed turns, oldest first.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Bundle {
    /// Highest tick whose hashes both clients sent and the relay found
    /// equal (0: none yet). Checkpoints at or below it are safe.
    pub confirmed: u64,
    pub peers: [PeerStat; 2],
    pub turns: Vec<Turn>,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct PeerStat {
    pub rtt_ms: u16,
    /// Silent for 2 s or more.
    pub unstable: bool,
    /// The slot has left or timed out.
    pub gone: bool,
}

/// One closed turn: the commands for `World::step` while `tick() == tick`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Turn {
    pub tick: u64,
    /// Always 0 in S2 (§5.11; the RNG does not read it yet).
    pub salt: u64,
    /// Sorted by `(player, seq)`.
    pub cmds: Vec<BundledCmd>,
}

/// A command with the turn its sender stamped it for; `target < tick`
/// means the relay restamped it (§10 "Re-stamp transparency").
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BundledCmd {
    pub cmd: Command,
    pub target: u64,
}

// ------------------------------------------------------------ the packet --

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Packet {
    Hello(Hello),
    Welcome(Welcome),
    Reject(Reject),
    CtlUp(CtlUp),
    CtlDown(CtlDown),
    Ack(Ack),
    Ping(Ping),
    Pong(Pong),
    Input(Input),
    Bundle(Bundle),
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum DecodeError {
    TooShort,
    TooLong,
    BadMagic,
    /// Another protocol version (the header is still returned by
    /// [`Header::read`]).
    Version(u16),
    UnknownKind(u8),
    Body,
    /// Decoded, but a list or string is over its limit.
    Limit,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum EncodeError {
    /// Over [`MAX_DATAGRAM`] bytes.
    TooLarge,
}

impl Packet {
    pub fn kind(&self) -> Kind {
        match self {
            Packet::Hello(_) => Kind::Hello,
            Packet::Welcome(_) => Kind::Welcome,
            Packet::Reject(_) => Kind::Reject,
            Packet::CtlUp(_) => Kind::CtlUp,
            Packet::CtlDown(_) => Kind::CtlDown,
            Packet::Ack(_) => Kind::Ack,
            Packet::Ping(_) => Kind::Ping,
            Packet::Pong(_) => Kind::Pong,
            Packet::Input(_) => Kind::Input,
            Packet::Bundle(_) => Kind::Bundle,
        }
    }

    /// Kinds a client may send. The relay drops every other kind, so a
    /// client cannot forge relay messages to it.
    pub fn is_up(&self) -> bool {
        matches!(
            self,
            Packet::Hello(_)
                | Packet::CtlUp(_)
                | Packet::Ack(_)
                | Packet::Ping(_)
                | Packet::Input(_)
        )
    }

    /// Kinds the relay may send.
    pub fn is_down(&self) -> bool {
        matches!(
            self,
            Packet::Welcome(_)
                | Packet::Reject(_)
                | Packet::CtlDown(_)
                | Packet::Ack(_)
                | Packet::Pong(_)
                | Packet::Bundle(_)
        )
    }

    /// Checks every list and string against its limit (after postcard,
    /// which has already bounded everything by the datagram length).
    pub fn validate(&self) -> bool {
        match self {
            Packet::Hello(h) => {
                h.build_id.len() <= MAX_ID && h.name.len() <= MAX_NAME && !h.name.is_empty()
            }
            Packet::Welcome(w) => (w.slot as usize) < SLOTS,
            Packet::Reject(r) => match &r.reason {
                RejectReason::Build { lobby } => lobby.len() <= MAX_ID,
                _ => true,
            },
            Packet::CtlUp(c) => match &c.msg {
                Up::LobbySet(l) => {
                    l.faction.len() <= MAX_ID && l.map.as_ref().is_none_or(|m| m.len() <= MAX_ID)
                }
                _ => true,
            },
            Packet::CtlDown(c) => match &c.msg {
                Down::Lobby(l) => {
                    l.map.len() <= MAX_ID
                        && l.note.len() <= MAX_NOTE
                        && l.slots
                            .iter()
                            .flatten()
                            .all(|s| s.name.len() <= MAX_NAME && s.faction.len() <= MAX_ID)
                }
                Down::Start(s) => {
                    // A sane clock (a hostile relay could otherwise make
                    // the period 0 and every schedule lookup spin), and a
                    // D the lobby could have picked.
                    (1..=MAX_TICK_HZ).contains(&s.tick_hz)
                        && (1..=MAX_SPEED_PCT).contains(&s.speed_pct)
                        && (MIN_D..=MAX_D).contains(&s.d)
                        && s.map.len() <= MAX_ID
                        && s.factions.iter().all(|f| f.len() <= MAX_ID)
                        && s.names.iter().all(|n| n.len() <= MAX_NAME)
                }
                _ => true,
            },
            Packet::Ack(_) | Packet::Ping(_) | Packet::Pong(_) => true,
            Packet::Input(i) => {
                i.turns.len() <= MAX_INPUT_TURNS
                    && i.turns.iter().all(|t| {
                        t.cmds.len() <= MAX_CMDS_PER_TURN && t.cmds.iter().all(command_in_limits)
                    })
            }
            Packet::Bundle(b) => {
                b.turns.len() <= MAX_BUNDLE_TURNS
                    && b.turns.iter().all(|t| {
                        t.cmds.len() <= MAX_TURN_CMDS
                            && t.cmds.iter().all(|c| {
                                (c.cmd.player.0 as usize) < SLOTS && command_in_limits(&c.cmd.kind)
                            })
                    })
            }
        }
    }
}

/// Most points of a `SuppressUnits` line (S3b).
pub const MAX_SUPPRESS_POINTS: usize = 2;
/// S10b: the longest `ClearMines` lane any rules allow (the top of
/// `[fortify] lane_max_m`, m).
pub const MAX_LANE_M: i32 = 200;

/// The per-command limits the sim applies anyway (it ignores longer
/// lists), checked on the wire so an oversized command never reaches it,
/// and the turn byte limit, so any one command fits a bundle datagram.
///
/// The match is exhaustive (no `_` arm, S3): a new command variant does not
/// compile until its lists are bounded here.
pub fn command_in_limits(k: &CommandKind) -> bool {
    let lists = match k {
        CommandKind::Dig { line, .. } => line.len() <= MAX_DIG_POINTS,
        CommandKind::DigUnits { units, line, .. } => {
            units.len() <= MAX_UNITS && line.len() <= MAX_DIG_POINTS
        }
        // S3b: a point or a two-point line.
        CommandKind::SuppressUnits { units, line } => {
            units.len() <= MAX_UNITS && (1..=MAX_SUPPRESS_POINTS).contains(&line.len())
        }
        CommandKind::MoveUnits { units, .. }
        | CommandKind::AttackMoveUnits { units, .. }
        | CommandKind::StopUnits { units }
        | CommandKind::RetreatUnits { units }
        | CommandKind::SetStance { units, .. }
        | CommandKind::SetFire { units, .. }
        | CommandKind::Throw { units, .. }
        | CommandKind::ThrowEach { units, .. }
        | CommandKind::ReverseUnits { units, .. }
        | CommandKind::FaceUnits { units, .. }
        | CommandKind::AttackUnit { units, .. }
        | CommandKind::SetAmmo { units, .. }
        | CommandKind::RepairUnits { units }
        | CommandKind::FollowUnits { units, .. } => units.len() <= MAX_UNITS,
        // S8a: squads or vehicles, as many as unit ids.
        // S9: guns, as many as unit ids.
        CommandKind::Emplace { guns, .. }
        | CommandKind::PackGun { guns }
        | CommandKind::PushGun { guns, .. }
        | CommandKind::Hitch { guns, .. }
        | CommandKind::Unhitch { guns }
        | CommandKind::SetAmbush { guns, .. } => !guns.is_empty() && guns.len() <= MAX_UNITS,
        CommandKind::Embark { squads, .. } => !squads.is_empty() && squads.len() <= MAX_UNITS,
        CommandKind::Disembark { vehicles, .. } => {
            !vehicles.is_empty() && vehicles.len() <= MAX_UNITS
        }
        // S10a: a blueprint line as a dig line; a Bangalore's men as units.
        CommandKind::Fortify { line, .. } => (1..=MAX_DIG_POINTS).contains(&line.len()),
        CommandKind::PlaceBangalore { units, .. } => {
            !units.is_empty() && units.len() <= MAX_UNITS
        }
        // S12: men as units.
        CommandKind::Tend { units, .. }
        | CommandKind::Carry { units, .. }
        | CommandKind::Evacuate { units } => !units.is_empty() && units.len() <= MAX_UNITS,
        // S14: men as units.
        CommandKind::Strike { units } => !units.is_empty() && units.len() <= MAX_UNITS,
        // S10b: a lane no longer than the largest `lane_max_m` allowed;
        // the map and the match's own `lane_max_m` are the sim's checks.
        CommandKind::ClearMines { a, b, .. } => {
            let dx = b.x.raw() as i128 - a.x.raw() as i128;
            let dy = b.y.raw() as i128 - a.y.raw() as i128;
            let max = (MAX_LANE_M as i128) << 16;
            dx * dx + dy * dy <= max * max
        }
        CommandKind::MoveSoldier { .. }
        | CommandKind::Spawn { .. }
        | CommandKind::Despawn { .. }
        | CommandKind::Move { .. }
        | CommandKind::AttackMove { .. }
        | CommandKind::Stop { .. }
        | CommandKind::Retreat { .. }
        | CommandKind::FireMission { .. }
        | CommandKind::CallIn { .. }
        | CommandKind::Surrender
        | CommandKind::Regroup { .. }
        // S3c: a squad and a vehicle id, no lists.
        | CommandKind::Recrew { .. }
        | CommandKind::Abandon { .. }
        // S6: a squad, an id or kind and at most two points, no lists.
        | CommandKind::Build { .. }
        | CommandKind::IssueCrate { .. }
        | CommandKind::PlaceCrate { .. }
        | CommandKind::PickUpCrate { .. }
        // S8b: a vehicle id, a point, a count and a kind.
        | CommandKind::DropCrates { .. }
        // S10a: a squad and a count or a point.
        | CommandKind::LoadMaterials { .. }
        | CommandKind::CutWire { .. }
        // S10b: a squad and a point.
        | CommandKind::Demolish { .. }
        // S12: a squad id.
        | CommandKind::Reinforce { .. }
        // S14: a squad and a point.
        | CommandKind::ClearTrench { .. } => true,
    };
    lists && encoded_len(k) <= MAX_TURN_BYTES
}

/// Encodes `packet` behind a header carrying `token`. Clears `out` first.
pub fn encode(token: u64, packet: &Packet, out: &mut Vec<u8>) -> Result<(), EncodeError> {
    out.clear();
    Header {
        version: PROTOCOL_VERSION,
        kind: packet.kind() as u8,
        token,
    }
    .write_to(out);
    let r = match packet {
        Packet::Hello(m) => append(m, out),
        Packet::Welcome(m) => append(m, out),
        Packet::Reject(m) => append(m, out),
        Packet::CtlUp(m) => append(m, out),
        Packet::CtlDown(m) => append(m, out),
        Packet::Ack(m) => append(m, out),
        Packet::Ping(m) => append(m, out),
        Packet::Pong(m) => append(m, out),
        Packet::Input(m) => append(m, out),
        Packet::Bundle(m) => append(m, out),
    };
    if r.is_err() || out.len() > MAX_DATAGRAM {
        return Err(EncodeError::TooLarge);
    }
    if let Packet::Hello(_) = packet
        && out.len() < HELLO_MIN_LEN
    {
        out.resize(HELLO_MIN_LEN, 0);
    }
    Ok(())
}

fn append<T: Serialize>(m: &T, out: &mut Vec<u8>) -> Result<(), ()> {
    // `extend_from_slice` would need a second buffer; postcard's `to_extend`
    // writes straight into the Vec.
    postcard::to_extend(m, std::mem::take(out))
        .map(|v| *out = v)
        .map_err(|_| ())
}

/// Encoded size of one value (for packing turns into a datagram).
pub fn encoded_len<T: Serialize>(m: &T) -> usize {
    postcard::experimental::serialized_size(m).unwrap_or(usize::MAX)
}

/// Decodes one datagram. Never panics; trailing bytes after the body
/// (the `Hello` padding) are ignored.
pub fn decode(bytes: &[u8]) -> Result<(Header, Packet), DecodeError> {
    if bytes.len() > MAX_DATAGRAM {
        return Err(DecodeError::TooLong);
    }
    if bytes.len() < HEADER_LEN {
        return Err(DecodeError::TooShort);
    }
    let h = Header::read(bytes).ok_or(DecodeError::BadMagic)?;
    if h.version != PROTOCOL_VERSION {
        return Err(DecodeError::Version(h.version));
    }
    let kind = Kind::from_u8(h.kind).ok_or(DecodeError::UnknownKind(h.kind))?;
    let body = &bytes[HEADER_LEN..];
    fn de<'a, T: Deserialize<'a>>(b: &'a [u8]) -> Result<T, DecodeError> {
        postcard::from_bytes(b).map_err(|_| DecodeError::Body)
    }
    let p = match kind {
        Kind::Hello => Packet::Hello(de(body)?),
        Kind::Welcome => Packet::Welcome(de(body)?),
        Kind::Reject => Packet::Reject(de(body)?),
        Kind::CtlUp => Packet::CtlUp(de(body)?),
        Kind::CtlDown => Packet::CtlDown(de(body)?),
        Kind::Ack => Packet::Ack(de(body)?),
        Kind::Ping => Packet::Ping(de(body)?),
        Kind::Pong => Packet::Pong(de(body)?),
        Kind::Input => Packet::Input(de(body)?),
        Kind::Bundle => Packet::Bundle(de(body)?),
    };
    if !p.validate() {
        return Err(DecodeError::Limit);
    }
    Ok((h, p))
}

/// Reads the relay version from a `Reject` of any protocol version (the
/// frozen part), so a client on another version can still explain itself.
pub fn reject_version(bytes: &[u8]) -> Option<u16> {
    let h = Header::read(bytes)?;
    if h.kind != Kind::Reject as u8 {
        return None;
    }
    postcard::take_from_bytes::<u16>(&bytes[HEADER_LEN..])
        .ok()
        .map(|(v, _)| v)
}

/// Appends a postcard varint (the length prefix of a sequence).
pub(crate) fn push_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frontline_sim::{Fx, FxVec2, PlayerId, SimId, game::SquadId};

    fn roundtrip(p: Packet) {
        let mut buf = Vec::new();
        encode(77, &p, &mut buf).unwrap();
        let (h, q) = decode(&buf).unwrap();
        assert_eq!(h.token, 77);
        assert_eq!(p, q);
    }

    #[test]
    fn packets_roundtrip() {
        roundtrip(Packet::Hello(Hello {
            nonce: 5,
            build_id: "abc".into(),
            content_hash: 9,
            name: "Ann".into(),
        }));
        roundtrip(Packet::CtlUp(CtlUp {
            cseq: 3,
            msg: Up::LobbySet(LobbySet {
                faction: "ww2:usa".into(),
                seed: Some(4),
                map: None,
                d: Some(3),
            }),
        }));
        roundtrip(Packet::Input(Input {
            ack: BundleAck { held: 10, mask: 5 },
            turns: vec![InputTurn {
                target: 12,
                seq0: 7,
                cmds: vec![CommandKind::Stop { squad: SquadId(2) }],
            }],
        }));
        roundtrip(Packet::Bundle(Bundle {
            confirmed: 30,
            peers: Default::default(),
            turns: vec![Turn {
                tick: 4,
                salt: 0,
                cmds: vec![BundledCmd {
                    cmd: Command {
                        player: PlayerId(1),
                        seq: 0,
                        kind: CommandKind::MoveUnits {
                            units: vec![SimId(3)],
                            target: FxVec2::from_int(1, 2),
                        },
                    },
                    target: 3,
                }],
            }],
        }));
    }

    #[test]
    fn hello_is_padded_and_limits_hold() {
        let mut buf = Vec::new();
        let hello = Packet::Hello(Hello {
            nonce: 1,
            build_id: "x".into(),
            content_hash: 0,
            name: "n".into(),
        });
        encode(0, &hello, &mut buf).unwrap();
        assert_eq!(buf.len(), HELLO_MIN_LEN);
        // A long name is refused on decode.
        let long = Packet::Hello(Hello {
            nonce: 1,
            build_id: "x".into(),
            content_hash: 0,
            name: "n".repeat(MAX_NAME + 1),
        });
        encode(0, &long, &mut buf).unwrap();
        assert_eq!(decode(&buf), Err(DecodeError::Limit));
        // Over the datagram limit: refused on encode.
        let big = Packet::Input(Input {
            ack: BundleAck::default(),
            turns: vec![InputTurn {
                target: 0,
                seq0: 0,
                cmds: vec![
                    CommandKind::StopUnits {
                        units: (0..MAX_UNITS as u32).map(|i| SimId(i + 100_000)).collect()
                    };
                    4
                ],
            }],
        });
        assert_eq!(encode(1, &big, &mut buf), Err(EncodeError::TooLarge));
    }

    /// Every command with a unit list or a line, filled to `n` ids and `k`
    /// points (S3: the new vehicle commands too).
    fn listed(n: usize, k: usize) -> Vec<CommandKind> {
        use frontline_content::{DigProfile, ItemKind};
        use frontline_sim::vehicle::AmmoChoice;
        let units: Vec<SimId> = (0..n as u32).map(SimId).collect();
        let line: Vec<FxVec2> = (0..k as i32).map(|i| FxVec2::from_int(i, i)).collect();
        let t = FxVec2::from_int(5, 5);
        vec![
            CommandKind::MoveUnits {
                units: units.clone(),
                target: t,
            },
            CommandKind::AttackMoveUnits {
                units: units.clone(),
                target: t,
            },
            CommandKind::StopUnits {
                units: units.clone(),
            },
            CommandKind::RetreatUnits {
                units: units.clone(),
            },
            CommandKind::DigUnits {
                units: units.clone(),
                line: line.clone(),
                profile: DigProfile::AtDitch,
            },
            CommandKind::SetStance {
                units: units.clone(),
                stance: frontline_sim::StanceOrder::Crouch,
            },
            CommandKind::SetFire {
                units: units.clone(),
                mode: frontline_sim::FireMode::Hold,
            },
            CommandKind::Throw {
                units: units.clone(),
                item: ItemKind::Satchel,
                target: t,
            },
            CommandKind::ThrowEach {
                units: units.clone(),
                item: ItemKind::Frag,
                target: t,
            },
            CommandKind::ReverseUnits {
                units: units.clone(),
                target: t,
            },
            CommandKind::FaceUnits {
                units: units.clone(),
                toward: t,
            },
            CommandKind::AttackUnit {
                units: units.clone(),
                target: SimId(9),
            },
            CommandKind::SetAmmo {
                units: units.clone(),
                ammo: AmmoChoice::He,
            },
            CommandKind::RepairUnits {
                units: units.clone(),
            },
            CommandKind::FollowUnits {
                units: units.clone(),
                vehicle: SimId(9),
            },
            CommandKind::SuppressUnits {
                units,
                line: line.iter().copied().take(MAX_SUPPRESS_POINTS).collect(),
            },
            CommandKind::Dig {
                squad: SquadId(1),
                line,
                profile: DigProfile::AtDitch,
            },
        ]
    }

    #[test]
    fn every_listed_command_is_bounded_on_the_wire() {
        let input = |k: CommandKind| {
            Packet::Input(Input {
                ack: BundleAck::default(),
                turns: vec![InputTurn {
                    target: 0,
                    seq0: 0,
                    cmds: vec![k],
                }],
            })
        };
        let mut buf = Vec::new();
        // At the limits (150 ids, 16 points): in limits and decoded.
        for k in listed(MAX_UNITS, MAX_DIG_POINTS) {
            assert!(command_in_limits(&k), "{k:?}");
            encode(1, &input(k.clone()), &mut buf).unwrap();
            assert!(decode(&buf).is_ok(), "{k:?}");
        }
        // One id over (151): every listed command is refused, so the relay
        // drops the datagram (`Dig` has no id list).
        let refused = |k: &CommandKind, buf: &mut Vec<u8>| {
            assert!(!command_in_limits(k), "{k:?}");
            if encode(1, &input(k.clone()), buf).is_ok() {
                assert_eq!(decode(buf), Err(DecodeError::Limit), "{k:?}");
            }
        };
        let mut n = 0;
        for k in listed(MAX_UNITS + 1, MAX_DIG_POINTS) {
            if !matches!(k, CommandKind::Dig { .. }) {
                refused(&k, &mut buf);
                n += 1;
            }
        }
        assert_eq!(n, 16, "every command with a unit list");
        // One point over (17): both dig commands are refused.
        for k in listed(MAX_UNITS, MAX_DIG_POINTS + 1) {
            if matches!(k, CommandKind::Dig { .. } | CommandKind::DigUnits { .. }) {
                refused(&k, &mut buf);
            }
        }
    }

    /// S10a: the four fortification commands, at and past their limits.
    #[test]
    fn fortify_commands_in_limits() {
        use frontline_content::setup::SupplyKind;
        let pts = |n: usize| {
            (0..n as i32)
                .map(|i| FxVec2::from_int(i, 0))
                .collect::<Vec<_>>()
        };
        let fortify = |n: usize| CommandKind::Fortify {
            squad: SquadId(4),
            fort: 2,
            line: pts(n),
        };
        let bangalore = |n: usize| CommandKind::PlaceBangalore {
            units: (0..n as u32).map(SimId).collect(),
            target: FxVec2::from_int(30, 40),
        };
        let input = |k: CommandKind| {
            Packet::Input(Input {
                ack: BundleAck::default(),
                turns: vec![InputTurn {
                    target: 0,
                    seq0: 0,
                    cmds: vec![k],
                }],
            })
        };
        let mut buf = Vec::new();
        let ok = [
            fortify(1),
            fortify(2),
            fortify(MAX_DIG_POINTS),
            bangalore(1),
            bangalore(MAX_UNITS),
            CommandKind::LoadMaterials {
                squad: SquadId(4),
                kg: u16::MAX,
            },
            CommandKind::CutWire {
                squad: SquadId(4),
                at: FxVec2::from_int(-5, 9),
            },
            CommandKind::IssueCrate {
                squad: SquadId(4),
                kind: SupplyKind::Construction,
            },
        ];
        for k in ok {
            assert!(command_in_limits(&k), "{k:?}");
            roundtrip(input(k.clone()));
        }
        for k in [
            fortify(0),
            fortify(MAX_DIG_POINTS + 1),
            bangalore(0),
            bangalore(MAX_UNITS + 1),
        ] {
            assert!(!command_in_limits(&k), "{k:?}");
            if encode(1, &input(k.clone()), &mut buf).is_ok() {
                assert_eq!(decode(&buf), Err(DecodeError::Limit), "{k:?}");
            }
        }
    }

    /// S10b: `ClearMines` lanes up to `MAX_LANE_M` pass, a longer one is
    /// refused on the wire; `Demolish` is a point. Control: let a lane of
    /// `MAX_LANE_M` + 1 m pass (`<= (max + 1)^2`) -> the over-long lane is
    /// accepted and the test fails.
    #[test]
    fn net_s10b_commands_in_limits() {
        let lane = |len_mm: i32| CommandKind::ClearMines {
            squad: SquadId(4),
            a: FxVec2::from_int(10, 10),
            b: FxVec2::new(Fx::from_int(10), Fx::from_int(10) + Fx::from_milli(len_mm)),
        };
        let input = |k: CommandKind| {
            Packet::Input(Input {
                ack: BundleAck::default(),
                turns: vec![InputTurn {
                    target: 0,
                    seq0: 0,
                    cmds: vec![k],
                }],
            })
        };
        let mut buf = Vec::new();
        let m = MAX_LANE_M * 1000;
        let ok = [
            lane(0),
            lane(1000),
            lane(m),
            // A diagonal at the limit.
            CommandKind::ClearMines {
                squad: SquadId(4),
                a: FxVec2::from_int(0, 0),
                b: FxVec2::from_int(MAX_LANE_M * 3 / 5, MAX_LANE_M * 4 / 5),
            },
            CommandKind::Demolish {
                squad: SquadId(4),
                at: FxVec2::from_int(-5, 9),
            },
        ];
        for k in ok {
            assert!(command_in_limits(&k), "{k:?}");
            roundtrip(input(k.clone()));
        }
        for k in [lane(m + 1000), lane(m + 1), lane(-m - 1000)] {
            assert!(!command_in_limits(&k), "{k:?}");
            if encode(1, &input(k.clone()), &mut buf).is_ok() {
                assert_eq!(decode(&buf), Err(DecodeError::Limit), "{k:?}");
            }
        }
    }

    /// S12 (version 16): the four wound commands encode and keep their
    /// unit lists to 1..=`MAX_UNITS` ids. Control: an empty or a 151-id
    /// list is out of limits.
    #[test]
    fn net_s12_commands_in_limits() {
        assert!(PROTOCOL_VERSION >= 16);
        let input = |k: CommandKind| {
            Packet::Input(Input {
                ack: BundleAck::default(),
                turns: vec![InputTurn {
                    target: 0,
                    seq0: 0,
                    cmds: vec![k],
                }],
            })
        };
        let ids = |n: u32| (0..n).map(SimId).collect::<Vec<_>>();
        let tend = |n| CommandKind::Tend {
            units: ids(n),
            target: SimId(7),
        };
        let carry = |n| CommandKind::Carry {
            units: ids(n),
            target: SimId(7),
            to: FxVec2::from_int(40, -3),
        };
        let evac = |n| CommandKind::Evacuate { units: ids(n) };
        let max = MAX_UNITS as u32;
        let ok = [
            tend(1),
            carry(2),
            evac(3),
            evac(max),
            CommandKind::Reinforce { squad: SquadId(9) },
        ];
        for k in ok {
            assert!(command_in_limits(&k), "{k:?}");
            if encoded_len(&k) <= 1024 {
                roundtrip(input(k.clone()));
            }
        }
        for k in [tend(0), carry(0), evac(0), tend(max + 1), evac(max + 1)] {
            assert!(!command_in_limits(&k), "{k:?}");
        }
    }

    /// S14 (version 17): `ClearTrench` and `Strike` encode; `Strike` keeps
    /// its unit list to 1..=`MAX_UNITS` ids. Control: an empty or a 151-id
    /// list is out of limits.
    #[test]
    fn net_s14_commands_in_limits() {
        assert_eq!(PROTOCOL_VERSION, 17);
        let input = |k: CommandKind| {
            Packet::Input(Input {
                ack: BundleAck::default(),
                turns: vec![InputTurn {
                    target: 0,
                    seq0: 0,
                    cmds: vec![k],
                }],
            })
        };
        let ids = |n: u32| (0..n).map(SimId).collect::<Vec<_>>();
        let strike = |n| CommandKind::Strike { units: ids(n) };
        let max = MAX_UNITS as u32;
        let ok = [
            strike(1),
            strike(max),
            CommandKind::ClearTrench {
                squad: SquadId(9),
                at: FxVec2::from_int(40, -3),
            },
        ];
        for k in ok {
            assert!(command_in_limits(&k), "{k:?}");
            if encoded_len(&k) <= 1024 {
                roundtrip(input(k.clone()));
            }
        }
        for k in [strike(0), strike(max + 1)] {
            assert!(!command_in_limits(&k), "{k:?}");
        }
    }

    #[test]
    fn suppress_lines_hold_one_or_two_points() {
        let k = |n: usize| CommandKind::SuppressUnits {
            units: vec![SimId(1)],
            line: (0..n as i32).map(|i| FxVec2::from_int(i, 0)).collect(),
        };
        assert!(!command_in_limits(&k(0)));
        assert!(command_in_limits(&k(1)));
        assert!(command_in_limits(&k(2)));
        assert!(!command_in_limits(&k(3)));
    }

    #[test]
    fn bundle_ack_bits() {
        let a = BundleAck {
            held: 10,
            mask: 0b101,
        };
        assert!(a.holds(9));
        assert!(!a.holds(10));
        assert!(a.holds(11));
        assert!(!a.holds(12));
        assert!(a.holds(13));
        assert!(!a.holds(75));
    }

    #[test]
    fn varint_matches_postcard() {
        for v in [0u64, 1, 127, 128, 300, 1 << 40] {
            let mut a = Vec::new();
            push_varint(&mut a, v);
            assert_eq!(a, postcard::to_allocvec(&v).unwrap());
        }
    }
}
