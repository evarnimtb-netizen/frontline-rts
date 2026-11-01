//! Frontline multiplayer (S2, §2.2, §10): relay-closed lockstep over UDP.
//!
//! * [`protocol`]: the wire format, limits and bounded decoding. The
//!   reference is `sdk/docs/net/relay-protocol.md`.
//! * [`relay`]: the relay as a sans-IO state machine (lobby, turns,
//!   restamps, bundle delivery, hash checks, pause and timeouts).
//! * [`client`]: the client as a sans-IO state machine (stamping, input
//!   resends, bundle ordering, acks, clock sync).
//! * [`clock`], [`schedule`]: the relay clock estimate and turn timing.
//! * [`transport`]: real UDP (`socket2`, dual-stack) and a seeded
//!   in-memory network for tests.
//! * [`server`], [`client_io`]: the relay thread and the client net
//!   thread around the state machines, on real sockets.
//! * [`dump`]: confirmed checkpoints and desync dumps.
//! * [`harness`]: steps `World`s from a client's bundles (pacing, hashes,
//!   the desync injection hook) and runs whole two-client matches on a
//!   virtual clock, for tests and `frontline-net-test`.
//!
//! Nothing here changes the sim: a match is a pure function of its
//! `MatchSetup` and the bundles fed to `World::step`.

pub mod client;
pub mod client_io;
pub mod clock;
pub mod ctl;
pub mod dump;
pub mod harness;
pub mod protocol;
pub mod relay;
pub mod rng;
pub mod schedule;
pub mod server;
pub mod transport;

pub use client::{ClientConfig, ClientPhase, ClientStatus, NetClient, NetEvent, suggest_d};
pub use protocol::{DEFAULT_PORT, PROTOCOL_VERSION};
pub use relay::{Relay, RelayConfig, RelayStatus};
pub use schedule::Schedule;

use frontline_content::MatchSetup;

/// The sim's save format version, read from the sim source by `build.rs`
/// (S2 may not change the sim, which keeps the constant private).
pub const SAVE_VERSION: u32 = parse_u32(env!("FRONTLINE_SAVE_VERSION"));

const fn parse_u32(s: &str) -> u32 {
    let b = s.as_bytes();
    let mut i = 0;
    let mut v = 0u32;
    while i < b.len() {
        v = v * 10 + (b[i] - b'0') as u32;
        i += 1;
    }
    v
}

/// What two players must share to play (S2 "Session setup"): the git
/// commit (with `-dirty` for uncommitted changes), [`PROTOCOL_VERSION`]
/// and the sim's save version, e.g. `3f2a9c1d0b7e-p2-s8`.
pub fn build_id() -> String {
    format!(
        "{}-p{}-s{}",
        env!("FRONTLINE_GIT_COMMIT"),
        PROTOCOL_VERSION,
        SAVE_VERSION
    )
}

/// `xxh3(postcard(MatchSetup))`: the `Ready` hash. The setup is the sim's
/// only content, so equal hashes mean both clients start the same match.
pub fn setup_hash(setup: &MatchSetup) -> u64 {
    let bytes = postcard::to_allocvec(setup).expect("encoding a setup cannot fail");
    xxhash_rust::xxh3::xxh3_64(&bytes)
}
