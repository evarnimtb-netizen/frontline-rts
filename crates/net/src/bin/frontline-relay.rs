//! `frontline-relay`: the standalone relay (S2, §10). Run it on a machine
//! both players can reach (a ~$5/month VPS, or a LAN box); both players
//! join it by address, and neither needs a forwarded port.
//!
//! It serves one match at a time: when a match ends (every player left or
//! timed out) it starts a fresh lobby on the same port, unless `--once`.

use std::fs::OpenOptions;
use std::io::{self, LineWriter};
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use frontline_net::protocol::{DEFAULT_PORT, MAX_D, MAX_SLOTS, MIN_D};
use frontline_net::relay::{DEFAULT_PING_GAP_US, RelayConfig};
use frontline_net::server::{LogSink, RelayServer};

#[derive(Parser)]
#[command(
    name = "frontline-relay",
    version,
    about = "Frontline match relay: closes turns and forwards orders for two to four players (UDP)"
)]
struct Args {
    /// UDP port to listen on.
    #[arg(long, default_value_t = DEFAULT_PORT)]
    port: u16,
    /// Address to bind. The default `::` is dual-stack (IPv4 and IPv6).
    #[arg(long, value_name = "IP", default_value_t = IpAddr::V6(Ipv6Addr::UNSPECIFIED))]
    bind: IpAddr,
    /// Highest input delay D (ticks) a lobby may use, for the suggestion and
    /// slot 0's override.
    #[arg(long, value_name = "TICKS", default_value_t = MAX_D,
          value_parser = clap::value_parser!(u8).range(MIN_D as i64..=MAX_D as i64))]
    max_delay: u8,
    /// Turn pacing in percent of real time. For tests only: both clients
    /// must expect it, and the game runs at 100.
    #[arg(long, value_name = "PCT", default_value_t = 100,
          value_parser = clap::value_parser!(u16).range(10..=1000))]
    speed: u16,
    /// Append the relay log (JSON lines: joins, turns, hashes, pauses,
    /// desyncs) to this file.
    #[arg(long, value_name = "FILE")]
    log: Option<PathBuf>,
    /// Leave closed turns out of the log (they are most of it).
    #[arg(long)]
    log_no_turns: bool,
    /// Let players with different build ids into one lobby (development).
    #[arg(long)]
    allow_build_mismatch: bool,
    /// Exit after one match instead of opening a new lobby.
    #[arg(long)]
    once: bool,
    /// S18: most players a lobby may hold (2 keeps a 1v1-only relay); the
    /// host picks how many of them the match uses.
    #[arg(long, value_name = "N", default_value_t = MAX_SLOTS as u8,
          value_parser = clap::value_parser!(u8).range(2..=MAX_SLOTS as i64))]
    max_slots: u8,
    /// S18: refuse the host's AI fill (every slot must be a person).
    #[arg(long)]
    no_ai: bool,
    /// S18: in a match of three or more, hostile players must pick
    /// different factions (a 1v1 always must).
    #[arg(long)]
    distinct_factions: bool,
    /// S18: shortest gap between two forwarded map pings of one player.
    #[arg(long, value_name = "MS", default_value_t = DEFAULT_PING_GAP_US / 1000,
          value_parser = clap::value_parser!(u64).range(0..=60_000))]
    ping_gap_ms: u64,
}

fn main() -> ExitCode {
    let args = Args::parse();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("frontline-relay: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &Args) -> io::Result<()> {
    let bind = SocketAddr::new(args.bind, args.port);
    let cfg = RelayConfig {
        speed_pct: args.speed,
        max_d: args.max_delay,
        allow_build_mismatch: args.allow_build_mismatch,
        max_slots: args.max_slots,
        allow_ai: !args.no_ai,
        allow_same_faction: !args.distinct_factions,
        ping_gap_us: args.ping_gap_ms * 1000,
        ..RelayConfig::default()
    };
    loop {
        let log: Option<LogSink> = match &args.log {
            Some(p) => Some(Box::new(LineWriter::new(
                OpenOptions::new().create(true).append(true).open(p)?,
            ))),
            None => None,
        };
        let mut server = RelayServer::spawn_with(cfg.clone(), bind, log, !args.log_no_turns)?;
        eprintln!(
            "frontline-relay {} listening on {} (protocol {}, max D {}{})",
            frontline_net::build_id(),
            server.local_addr(),
            frontline_net::PROTOCOL_VERSION,
            args.max_delay,
            if args.speed != 100 {
                format!(", speed {} %", args.speed)
            } else {
                String::new()
            }
        );
        let mut last = String::new();
        while server.is_running() {
            std::thread::sleep(Duration::from_millis(500));
            let s = server.status();
            let slots: Vec<String> = s
                .slots
                .iter()
                .take((s.players as usize).max(2))
                .enumerate()
                .map(|(i, x)| match x {
                    Some(x) => format!(
                        "slot {i}: {} {} {} rtt {} ms{}",
                        x.name,
                        x.addr,
                        x.state,
                        x.rtt_ms,
                        if x.silent_ms >= 2000 {
                            format!(", silent {:.1} s", x.silent_ms as f64 / 1000.0)
                        } else {
                            String::new()
                        }
                    ),
                    None if s.ai_slots.contains(&(i as u8)) => format!("slot {i}: AI"),
                    None => format!("slot {i}: open"),
                })
                .collect();
            let line = format!(
                "{} D {} | {} | {}{}",
                s.phase,
                s.d,
                slots.join(" | "),
                s.paused
                    .as_ref()
                    .map(|p| format!("paused {p} | "))
                    .unwrap_or_default(),
                s.desync_tick
                    .map(|t| format!("DESYNC at tick {t}"))
                    .unwrap_or_default(),
            );
            // Print on change, ignoring the RTT and turn churn.
            let key = format!(
                "{} {:?} {:?} {} {:?}",
                s.phase, s.paused, s.desync_tick, s.players, s.ai_slots
            ) + &s
                .slots
                .iter()
                .map(|x| {
                    x.as_ref()
                        .map(|x| format!("{} {}", x.name, x.state))
                        .unwrap_or_default()
                })
                .collect::<String>();
            if key != last {
                eprintln!("{line}");
                last = key;
            }
        }
        server.stop();
        let s = server.status();
        eprintln!(
            "match over: {} turns, {} commands, {} restamped",
            s.next_turn, s.commands, s.restamps
        );
        if args.once {
            return Ok(());
        }
    }
}
