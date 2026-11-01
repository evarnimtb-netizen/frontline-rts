//! `frontline-net-test`: a headless two-client match (S2 "Testing"). Two
//! worlds, a relay and two clients, with scripted orders, over a simulated
//! link on a virtual clock (`--net sim`, the default: faster than real
//! time) or over UDP loopback in real time behind a simulated bad link
//! (`--net udp`). Prints the chosen input delay D, the restamp rate, stalls
//! and whether the hashes agreed.
//!
//! Exits 0 when the hashes agreed at every check and every stamped order
//! ran exactly once in order; 1 otherwise.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use frontline_net::harness::{self, VirtualReport, VirtualSpec};
use frontline_net::relay::RelayConfig;
use frontline_net::transport::LinkParams;

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum Net {
    /// Simulated network, virtual clock.
    Sim,
    /// UDP on 127.0.0.1, real time; the link is simulated in the clients.
    Udp,
}

#[derive(Parser)]
#[command(
    name = "frontline-net-test",
    version,
    about = "Headless two-client Frontline match over a simulated or loopback network"
)]
struct Args {
    #[arg(long, value_enum, default_value_t = Net::Sim)]
    net: Net,
    /// Game seconds to play.
    #[arg(long, default_value_t = 180)]
    seconds: u64,
    /// Match, script and link seed.
    #[arg(long, default_value_t = 1)]
    seed: u64,
    /// One-way base delay, ms.
    #[arg(long, value_name = "MS", default_value_t = 5)]
    delay: u64,
    /// One-way extra delay, uniform in 0..=MS (reorders datagrams).
    #[arg(long, value_name = "MS", default_value_t = 0)]
    jitter: u64,
    /// Chance each datagram is lost (0–1).
    #[arg(long, value_name = "P", default_value_t = 0.0)]
    drop: f64,
    /// Chance each datagram arrives twice.
    #[arg(long, value_name = "P", default_value_t = 0.0)]
    dup: f64,
    /// Chance a datagram is held back another 40 ms.
    #[arg(long, value_name = "P", default_value_t = 0.0)]
    reorder: f64,
    /// Force the input delay (2–6) instead of the lobby suggestion.
    #[arg(long, value_name = "TICKS")]
    d: Option<u8>,
    /// No sim AI on the slots (only the scripted orders play).
    #[arg(long)]
    no_ai: bool,
    /// No scripted orders.
    #[arg(long)]
    no_script: bool,
    /// Dev desync hook: slot 1 steps this tick with one extra local order
    /// (sim only).
    #[arg(long, value_name = "TICK")]
    inject_desync: Option<u64>,
    /// Override both players' tickets (long runs).
    #[arg(long)]
    tickets: Option<u32>,
    /// Garbage datagrams per second at the relay and clients (sim only).
    #[arg(long, value_name = "N", default_value_t = 0)]
    garbage: u32,
    /// Sim worker threads per world.
    #[arg(long, default_value_t = 1)]
    threads: usize,
    /// Mod directory (default: mods/ww2).
    #[arg(long, value_name = "DIR")]
    mods: Option<PathBuf>,
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("frontline-net-test: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn run(a: Args) -> anyhow::Result<bool> {
    let defs = harness::load_defs(&harness::mods_dir(a.mods.as_deref())?)?;
    let setup = harness::net_setup(&defs, a.seed, !a.no_ai, a.tickets)?;
    let link = LinkParams {
        delay_us: a.delay * 1000,
        jitter_us: a.jitter * 1000,
        drop: a.drop,
        dup: a.dup,
        reorder: a.reorder,
        reorder_us: 40_000,
    };
    let spec = VirtualSpec {
        seed: a.seed,
        seconds: a.seconds,
        link,
        inject_desync: a.inject_desync.map(|t| (1, t)),
        script: !a.no_script,
        garbage_per_s: a.garbage,
        relay: RelayConfig::default(),
        d_override: a.d,
        threads: a.threads,
        ..VirtualSpec::default()
    };
    println!(
        "net {:?}: one-way {} ms + 0–{} ms jitter, drop {:.1} %, dup {:.1} %, reorder {:.1} %; \
         {} s, seed {}, AI {}",
        a.net,
        a.delay,
        a.jitter,
        a.drop * 100.0,
        a.dup * 100.0,
        a.reorder * 100.0,
        a.seconds,
        a.seed,
        if a.no_ai { "off" } else { "on" },
    );
    let r = match a.net {
        Net::Sim => harness::run_virtual(&setup, &spec),
        Net::Udp => harness::run_udp(&setup, &spec)?,
    };
    print_report(&r);
    Ok(r.hash_mismatches == 0
        && r.relay_desync.is_none()
        && r.order_errors == 0
        && r.lost == [0, 0]
        && !r.streams_differ)
}

fn print_report(r: &VirtualReport) {
    let ms = |v: Option<u16>| v.map_or("?".to_owned(), |v| v.to_string());
    println!(
        "input delay: D = {} ({} ms); client RTT p99 {} / {} ms",
        r.d,
        r.d as u64 * 1000 / 30,
        ms(r.rtt_p99_ms[0]),
        ms(r.rtt_p99_ms[1])
    );
    println!("ticks: {} / {}", r.ticks[0], r.ticks[1]);
    match (r.hash_mismatches, r.relay_desync) {
        (0, None) => println!("hashes: {} checks, all agree", r.hash_checks),
        _ => println!(
            "hashes: {} checks, {} MISMATCHED (first at tick {:?}); relay desync at {:?}",
            r.hash_checks, r.hash_mismatches, r.first_mismatch, r.relay_desync
        ),
    }
    let ok = r.lost == [0, 0] && r.order_errors == 0 && !r.streams_differ;
    println!(
        "orders: stamped {} / {}, {}",
        r.stamped[0],
        r.stamped[1],
        if ok {
            "every one ran exactly once, in order".to_owned()
        } else {
            format!(
                "LOST {:?}, order errors {}, streams differ {}",
                r.lost, r.order_errors, r.streams_differ
            )
        }
    );
    println!(
        "restamps: {} of {} commands ({:.3} %)",
        r.relay_restamps,
        r.relay_commands,
        r.restamp_rate() * 100.0
    );
    println!(
        "stalls: {} / {} ticks stepped more than one period late",
        r.stall_ticks[0], r.stall_ticks[1]
    );
    println!(
        "pauses: {}, resumes: {}; sim step p99 {} us, max {} us",
        r.pauses, r.resumes, r.step_p99_us, r.step_max_us
    );
    if r.net.sent > 0 {
        println!(
            "link: {} sent, {} dropped, {} duplicated",
            r.net.sent, r.net.dropped, r.net.duplicated
        );
    }
    if let Some((tick, at)) = r.desync[0] {
        println!("desync: detected at tick {tick}, both paused before turn {at}");
    }
}
