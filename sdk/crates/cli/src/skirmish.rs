//! `skirmish`: run a whole Kessel Ridge match headlessly, AI against AI, and
//! report the winner, the match length, per-player stats and tick timings
//! (S1 "Done when" 2 and 5).
//!
//! The match runs unpaced (as fast as the machine allows) unless `--paced`
//! is given. Tick times are measured around `World::step` and summarised
//! twice: over the whole match and over the "peak" ticks, those with at
//! least [`PEAK_SHARE_PCT`] % of the match's highest live soldier count.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs, PlayerRequest};
use frontline_sim::{BlastKind, Outcome, PlayerId, SquadId, World};
use serde::Serialize;

use crate::report::{Summary, hex, ns, round3};

/// Ticks with at least this share of the peak soldier count are "peak".
pub const PEAK_SHARE_PCT: usize = 90;

/// AI strength on the command line.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum DifficultyArg {
    Easy,
    #[default]
    Normal,
    Hard,
}

impl From<DifficultyArg> for Difficulty {
    fn from(d: DifficultyArg) -> Difficulty {
        match d {
            DifficultyArg::Easy => Difficulty::Easy,
            DifficultyArg::Normal => Difficulty::Normal,
            DifficultyArg::Hard => Difficulty::Hard,
        }
    }
}

/// How `skirmish` prints its result.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum ReportFormat {
    /// A short human summary.
    #[default]
    Text,
    /// The full [`SkirmishReport`] as pretty JSON on stdout.
    Json,
}

/// Options for [`skirmish`]; also the clap definition of the subcommand.
#[derive(Clone, Debug, clap::Args)]
pub struct SkirmishArgs {
    /// Both players are AI. Required (unless --load): the headless CLI has
    /// no human player.
    #[arg(long, required_unless_present = "load")]
    pub ai_vs_ai: bool,
    /// Match seed.
    #[arg(long, value_name = "N", default_value_t = 1, conflicts_with = "load")]
    pub seed: u64,
    /// AI difficulty for both players.
    #[arg(long, value_enum, default_value_t = DifficultyArg::Normal)]
    pub difficulty: DifficultyArg,
    /// Overrides --difficulty for player 1.
    #[arg(long, value_enum, value_name = "DIFFICULTY")]
    pub difficulty1: Option<DifficultyArg>,
    /// Faction of player 0 (base 0).
    #[arg(long, value_name = "ID", default_value = "ww2:germany")]
    pub faction0: String,
    /// Faction of player 1 (base 1).
    #[arg(long, value_name = "ID", default_value = "ww2:usa")]
    pub faction1: String,
    /// Stop after this many game minutes if nobody has won.
    #[arg(long, value_name = "M", default_value_t = 30)]
    pub minutes: u32,
    /// Sim worker threads (default: all cores). Never changes results.
    #[arg(long, value_name = "T")]
    pub threads: Option<usize>,
    /// Mod directory (default: `mods/ww2` here or in the source tree).
    #[arg(long, value_name = "DIR")]
    pub mods: Option<PathBuf>,
    /// Starting manpower override for both players (stress runs that fill
    /// the pop cap early). Changes the match, so hashes differ.
    #[arg(long, value_name = "MP")]
    pub start_manpower: Option<u32>,
    /// Player 0 starts at base 1 and player 1 at base 0 (balance checks:
    /// separates the map side from the player slot).
    #[arg(long)]
    pub swap_bases: bool,
    /// Run at the real tick rate (sleep to each tick's deadline).
    #[arg(long)]
    pub paced: bool,
    /// Save the full sim state to this file: at --save-at, or at the end of
    /// the run when it is not given or the match ends first.
    #[arg(long, value_name = "FILE")]
    pub save: Option<PathBuf>,
    /// Absolute tick at which to write --save.
    #[arg(long, value_name = "K", requires = "save")]
    pub save_at: Option<u64>,
    /// Continue a saved AI-vs-AI match instead of starting one. The match
    /// options (seed, factions, difficulty, manpower, bases) come from the
    /// save.
    #[arg(
        long,
        value_name = "FILE",
        conflicts_with_all = ["difficulty1", "start_manpower", "swap_bases"]
    )]
    pub load: Option<PathBuf>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = ReportFormat::Text)]
    pub report: ReportFormat,
}

impl Default for SkirmishArgs {
    fn default() -> SkirmishArgs {
        SkirmishArgs {
            ai_vs_ai: true,
            seed: 1,
            difficulty: DifficultyArg::Normal,
            difficulty1: None,
            faction0: "ww2:germany".into(),
            faction1: "ww2:usa".into(),
            minutes: 30,
            threads: None,
            mods: None,
            start_manpower: None,
            swap_bases: false,
            save: None,
            save_at: None,
            load: None,
            paced: false,
            report: ReportFormat::Text,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SkirmishReport {
    pub seed: u64,
    /// Tick the run started from (non-zero after --load).
    pub start_tick: u64,
    pub loaded_from: Option<String>,
    pub saved: Option<SavedMatch>,
    pub threads: usize,
    pub paced: bool,
    /// `"won"`, `"draw"` or `"time_limit"`.
    pub result: &'static str,
    pub winner: Option<u8>,
    pub winner_faction: Option<String>,
    pub tick_hz: u64,
    pub ticks: u64,
    pub game_seconds: f64,
    pub limit_minutes: u32,
    pub final_hash: String,
    pub players: Vec<PlayerReport>,
    pub totals: Totals,
    pub timing: SkirmishTiming,
    pub wall_ms: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SavedMatch {
    pub tick: u64,
    pub path: String,
    pub bytes: usize,
    pub hash: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlayerReport {
    pub player: u8,
    pub faction: String,
    pub difficulty: String,
    /// `tickets`, `wiped` or `surrendered` when the player lost.
    pub defeat: Option<String>,
    pub tickets: u32,
    pub manpower: u32,
    pub kills: u32,
    pub losses: u32,
    /// Squads this player fielded during this run (starters included; after
    /// --load, those alive at the load plus later ones).
    pub squads_fielded: u32,
    pub squads_alive: u32,
    pub soldiers_alive: u32,
    pub points_held: u8,
    /// Point-seconds held over the match.
    pub hold_s: u32,
    pub ai_commands: u32,
    pub ai_rejected: u32,
    pub trenches_planned: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct Totals {
    pub shots_fired: u64,
    pub shots_dropped: u64,
    pub soldier_hits: u64,
    pub near_misses: u64,
    pub explosions: u64,
    pub deaths: u64,
    /// Thrown items released (S1b), and the frag and satchel bursts and
    /// smoke clouds among them, counted since the start (or the load).
    pub throws: u64,
    pub frag_bursts: u64,
    pub satchel_bursts: u64,
    pub smoke_clouds: u64,
    pub detail_tiles: usize,
    pub peak_soldiers: usize,
    pub peak_tick: u64,
    pub peak_projectiles: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct SkirmishTiming {
    /// Every tick of the match.
    pub all: Summary,
    /// Ticks with at least `peak_share_pct` % of `totals.peak_soldiers`.
    pub peak: Summary,
    pub peak_ticks: usize,
    pub peak_share_pct: usize,
    /// Ticks over the tick period (33.3 ms at 30 Hz).
    pub overruns: u64,
}

/// The `mods/ww2` directory: `--mods`, else `./mods/ww2`, else the copy in
/// the source tree this binary was built from.
pub fn mods_dir(arg: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = arg {
        return Ok(p.to_owned());
    }
    let here = PathBuf::from("mods/ww2");
    if here.join("mod.toml").is_file() {
        return Ok(here);
    }
    let tree = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mods/ww2");
    if tree.join("mod.toml").is_file() {
        return Ok(tree);
    }
    bail!("cannot find mods/ww2; pass --mods <DIR>")
}

/// Loads the mod and resolves an AI-vs-AI match from `args`.
pub fn match_setup(args: &SkirmishArgs) -> Result<MatchSetup> {
    let dir = mods_dir(args.mods.as_deref())?;
    let defs = ModDefs::load(&dir).map_err(|e| anyhow!("{e}"))?;
    let d0: Difficulty = args.difficulty.into();
    let d1: Difficulty = args.difficulty1.unwrap_or(args.difficulty).into();
    let req = MatchRequest {
        rules: "ww2:skirmish".into(),
        map: "ww2:kessel_ridge".into(),
        seed: args.seed,
        players: vec![
            PlayerRequest {
                faction: args.faction0.clone(),
                ai: true,
                difficulty: d0,
            },
            PlayerRequest {
                faction: args.faction1.clone(),
                ai: true,
                difficulty: d1,
            },
        ],
    };
    let mut setup = defs.resolve(&req).map_err(|e| anyhow!("{e}"))?;
    if let Some(mp) = args.start_manpower {
        setup.rules.start_manpower = mp;
    }
    if args.swap_bases {
        let (b0, b1) = (setup.players[0].base, setup.players[1].base);
        (setup.players[0].base, setup.players[1].base) = (b1, b0);
    }
    Ok(setup)
}

/// Runs the match described by `args` to its end or the time limit.
pub fn skirmish(args: &SkirmishArgs) -> Result<SkirmishReport> {
    if !args.ai_vs_ai && args.load.is_none() {
        bail!("only --ai-vs-ai is supported headlessly");
    }
    let threads = match args.threads {
        Some(0) => bail!("--threads must be at least 1"),
        Some(t) => t,
        None => std::thread::available_parallelism().map_or(1, |n| n.get()),
    };
    let mut world = match &args.load {
        Some(path) => {
            let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
            let w = World::load(&bytes, threads).map_err(|e| anyhow!("{}: {e}", path.display()))?;
            match w.match_setup() {
                Some(s) if s.players.iter().all(|p| p.is_ai) => {}
                _ => bail!("{} is not an AI-vs-AI match save", path.display()),
            }
            w
        }
        None => World::new_match(match_setup(args)?, threads)?,
    };
    let hz = world.match_setup().expect("match world").tick_hz as u64;
    let limit = args.minutes as u64 * 60 * hz;
    let start_tick = world.tick();
    let save_at = match (&args.save, args.save_at) {
        (Some(_), Some(k)) if k < start_tick || k > limit => bail!(
            "--save-at {k} is outside this run's ticks {start_tick}..={limit} \
             (it is an absolute tick number)"
        ),
        (Some(_), k) => Some(k),
        (None, _) => None,
    };
    let mut saved = None;
    let mut maybe_save = |w: &World, at_end: bool| -> Result<()> {
        if let Some(path) = &args.save
            && saved.is_none()
            && (save_at == Some(Some(w.tick())) || at_end)
        {
            let bytes = w.save();
            fs::write(path, &bytes).with_context(|| format!("writing {}", path.display()))?;
            saved = Some(SavedMatch {
                tick: w.tick(),
                path: path.display().to_string(),
                bytes: bytes.len(),
                hash: hex(w.state_hash()),
            });
        }
        Ok(())
    };
    maybe_save(&world, false)?;

    let period = Duration::from_nanos(1_000_000_000 / hz);
    let mut tick_ns: Vec<u64> = Vec::with_capacity(limit as usize);
    let mut soldiers: Vec<usize> = Vec::with_capacity(limit as usize);
    let mut fielded: BTreeMap<SquadId, PlayerId> = BTreeMap::new();
    let (mut overruns, mut peak_proj) = (0u64, 0usize);
    let (mut peak, mut peak_tick) = (0usize, 0u64);
    let note_squads = |w: &World, fielded: &mut BTreeMap<SquadId, PlayerId>| {
        for (&id, sq) in w.game().expect("match world").squads() {
            fielded.entry(id).or_insert(sq.owner);
        }
    };
    note_squads(&world, &mut fielded);

    let start = Instant::now();
    let running = |w: &World| w.game().is_some_and(|g| g.outcome() == Outcome::Running);
    let (mut frag_bursts, mut satchel_bursts) = (0u64, 0u64);
    let smoke_at_start = world.snapshot().smoke.last().map_or(0, |c| c.id as u64 + 1);
    let mut smoke_seen = smoke_at_start;
    let throws_at_start = world.stats().throws;
    while running(&world) && world.tick() < limit {
        if args.paced {
            let deadline = start + period * (world.tick() - start_tick) as u32;
            let now = Instant::now();
            if now < deadline {
                std::thread::sleep(deadline - now);
            }
        }
        let t = Instant::now();
        world.step(&[]);
        let dt = t.elapsed();
        if dt > period {
            overruns += 1;
        }
        tick_ns.push(ns(dt));
        let n = world.entities().soldier_count();
        soldiers.push(n);
        if n > peak {
            (peak, peak_tick) = (n, world.tick());
        }
        peak_proj = peak_proj.max(world.projectiles().live());
        for e in &world.snapshot().explosions {
            match e.kind {
                BlastKind::Frag => frag_bursts += 1,
                BlastKind::Satchel => satchel_bursts += 1,
                BlastKind::Shell => {}
            }
        }
        for c in &world.snapshot().smoke {
            smoke_seen = smoke_seen.max(c.id as u64 + 1);
        }
        note_squads(&world, &mut fielded);
        maybe_save(&world, false)?;
    }
    let wall = start.elapsed();
    maybe_save(&world, true)?;

    let threshold = (peak * PEAK_SHARE_PCT).div_ceil(100);
    let mut peak_ns: Vec<u64> = tick_ns
        .iter()
        .zip(&soldiers)
        .filter(|(_, n)| **n >= threshold)
        .map(|(t, _)| *t)
        .collect();
    let timing = SkirmishTiming {
        peak: Summary::from_nanos(&mut peak_ns),
        peak_ticks: peak_ns.len(),
        all: Summary::from_nanos(&mut tick_ns),
        peak_share_pct: PEAK_SHARE_PCT,
        overruns,
    };

    let g = world.game().expect("match world");
    let s = g.setup();
    let snap = world.snapshot();
    let (result, winner) = match g.outcome() {
        Outcome::Won { winner } => ("won", Some(winner.0)),
        Outcome::Draw => ("draw", None),
        Outcome::Running => ("time_limit", None),
    };
    let faction = |p: usize| s.factions[s.players[p].faction as usize].id.clone();
    let players = (0..s.players.len())
        .map(|p| {
            let pl = &g.players()[p];
            let ps = &snap.players[p];
            let ai = g.ai(PlayerId(p as u8));
            let owned = |o: &PlayerId| o.0 as usize == p;
            PlayerReport {
                player: p as u8,
                faction: faction(p),
                difficulty: format!("{:?}", s.players[p].difficulty).to_lowercase(),
                defeat: pl.defeat.map(|d| format!("{d:?}").to_lowercase()),
                tickets: pl.tickets,
                manpower: pl.manpower,
                kills: pl.kills,
                losses: pl.losses,
                squads_fielded: fielded.values().filter(|o| owned(o)).count() as u32,
                squads_alive: g.squads().values().filter(|q| owned(&q.owner)).count() as u32,
                soldiers_alive: snap
                    .soldier_owner
                    .iter()
                    .filter(|o| **o as usize == p)
                    .count() as u32,
                points_held: ps.points_held,
                hold_s: ps.hold_s,
                ai_commands: ai.map_or(0, |a| a.issued),
                ai_rejected: ai.map_or(0, |a| a.rejected),
                trenches_planned: ai.map_or(0, |a| a.trenches.len() as u32),
            }
        })
        .collect();
    let st = world.stats();
    Ok(SkirmishReport {
        seed: s.seed,
        start_tick,
        loaded_from: args.load.as_ref().map(|p| p.display().to_string()),
        saved,
        threads,
        paced: args.paced,
        result,
        winner,
        winner_faction: winner.map(|w| faction(w as usize)),
        tick_hz: hz,
        ticks: world.tick(),
        game_seconds: round3(world.tick() as f64 / hz as f64),
        limit_minutes: args.minutes,
        final_hash: hex(world.state_hash()),
        players,
        totals: Totals {
            shots_fired: st.shots_fired,
            shots_dropped: st.shots_dropped,
            soldier_hits: st.soldier_hits,
            near_misses: st.near_misses,
            explosions: st.explosions,
            deaths: st.deaths,
            throws: st.throws - throws_at_start,
            frag_bursts,
            satchel_bursts,
            smoke_clouds: smoke_seen - smoke_at_start,
            detail_tiles: world.terrain().detail().tiles().len(),
            peak_soldiers: peak,
            peak_tick,
            peak_projectiles: peak_proj,
        },
        timing,
        wall_ms: round3(wall.as_secs_f64() * 1e3),
    })
}

fn mmss(seconds: f64) -> String {
    let s = seconds as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Human summary of a report (several lines, no trailing newline).
pub fn summary_text(r: &SkirmishReport) -> String {
    let head = match (r.result, &r.winner_faction) {
        ("won", Some(f)) => format!("winner: player {} ({f})", r.winner.unwrap_or(0)),
        ("draw", _) => "result: draw".into(),
        _ => format!("result: no winner within {} min", r.limit_minutes),
    };
    let mut out = format!(
        "skirmish seed {} on {} thread(s): {head} after {} ({} ticks); final hash {}\n",
        r.seed,
        r.threads,
        mmss(r.game_seconds),
        r.ticks,
        r.final_hash
    );
    if let Some(from) = &r.loaded_from {
        out += &format!("continued from {from} at tick {}\n", r.start_tick);
    }
    if let Some(sv) = &r.saved {
        out += &format!(
            "saved tick {} ({} bytes, hash {}) to {}\n",
            sv.tick, sv.bytes, sv.hash, sv.path
        );
    }
    out += "player  faction       difficulty  outcome      tickets  manpower  kills  losses  \
            squads  points  hold_s  ai_cmds  rejected\n";
    for p in &r.players {
        let outcome = match (&p.defeat, r.winner) {
            (Some(d), _) => format!("lost ({d})"),
            (None, Some(w)) if w == p.player => "won".into(),
            _ => "-".into(),
        };
        out += &format!(
            "{:<7} {:<13} {:<11} {:<12} {:>7}  {:>8}  {:>5}  {:>6}  {:>6}  {:>6}  {:>6}  {:>7}  {:>8}\n",
            p.player,
            p.faction,
            p.difficulty,
            outcome,
            p.tickets,
            p.manpower,
            p.kills,
            p.losses,
            p.squads_fielded,
            p.points_held,
            p.hold_s,
            p.ai_commands,
            p.ai_rejected,
        );
    }
    let t = &r.totals;
    out += &format!(
        "shots {} ({} dropped), hits {}, near misses {}, explosions {}, deaths {}, \
         terrain tiles {}\n\
         throws {}: frag bursts {}, satchel bursts {}, smoke clouds {}\n",
        t.shots_fired,
        t.shots_dropped,
        t.soldier_hits,
        t.near_misses,
        t.explosions,
        t.deaths,
        t.detail_tiles,
        t.throws,
        t.frag_bursts,
        t.satchel_bursts,
        t.smoke_clouds
    );
    let (a, k) = (&r.timing.all, &r.timing.peak);
    out += &format!(
        "tick ms: mean {:.3}, p95 {:.3}, p99 {:.3}, max {:.3}; peak {} soldiers at {} \
         ({} ticks >= {} %): p99 {:.3}, max {:.3}; overruns {}; wall {:.1} s{}",
        a.mean_ms,
        a.p95_ms,
        a.p99_ms,
        a.max_ms,
        t.peak_soldiers,
        mmss(t.peak_tick as f64 / r.tick_hz as f64),
        r.timing.peak_ticks,
        r.timing.peak_share_pct,
        k.p99_ms,
        k.max_ms,
        r.timing.overruns,
        r.wall_ms / 1e3,
        if r.paced { " (paced)" } else { "" }
    );
    out
}
