//! Desync checkpoints and dumps (S2 "Hash check and desync dump").
//!
//! Every [`CHECKPOINT_TICKS`] ticks the client takes a `World::save`. It
//! keeps the newest checkpoint whose tick the relay has *confirmed* (both
//! hashes agreed at or after it), plus every bundle stepped since. A newer,
//! unconfirmed checkpoint may already be past the divergence, so it never
//! replaces the confirmed one; it waits until the relay confirms its tick.
//!
//! On a desync, [`DesyncRecorder::write_dump`] writes
//! `<dir>/manifest.json`, `checkpoint.sav` (the confirmed one),
//! `paused.sav` (the world at the pause turn) and `bundles.jsonl` (every
//! bundle since the checkpoint). The relay's own `relay.jsonl` comes from
//! its log. `frontline-cli desync-diff` compares the two clients'
//! `paused.sav`.

use std::collections::VecDeque;
use std::io::{self, Write};
use std::path::Path;

use frontline_sim::{Command, World};
use serde::Serialize;

/// Ticks between checkpoints (one save every 10 s at 30 Hz).
pub const CHECKPOINT_TICKS: u64 = 300;
/// Unconfirmed checkpoints kept while waiting for the relay.
const MAX_UNCONFIRMED: usize = 4;

#[derive(Default)]
pub struct DesyncRecorder {
    confirmed: Option<(u64, Vec<u8>)>,
    unconfirmed: VecDeque<(u64, Vec<u8>)>,
    /// Bundles stepped since the oldest kept checkpoint, `(tick, cmds)`.
    bundles: VecDeque<(u64, Vec<Command>)>,
}

/// What `manifest.json` holds.
#[derive(Clone, Debug, Default, Serialize)]
pub struct DumpManifest {
    pub build_id: String,
    pub content_hash: String,
    pub setup_hash: String,
    pub seed: u64,
    pub d: u8,
    pub slot: u8,
    /// Tick whose hashes differed, and every slot's hash there (slot
    /// order; `null` for a slot that did not report, or an AI slot).
    pub desync_tick: u64,
    pub hashes: Vec<Option<String>>,
    /// The pause turn: `paused.sav` is the world after `paused_tick` ticks.
    pub paused_tick: u64,
    pub state_hash: String,
    pub full_state_hash: String,
    pub checkpoint_tick: Option<u64>,
    /// `(tick, state_hash)` for the last 300 ticks.
    pub hash_ring: Vec<(u64, String)>,
}

#[derive(Serialize)]
struct BundleLine<'a> {
    tick: u64,
    tick_salt: u64,
    cmds: &'a [Command],
}

impl DesyncRecorder {
    pub fn new() -> DesyncRecorder {
        DesyncRecorder::default()
    }

    /// Call before stepping `world` with `cmds`: takes a checkpoint on
    /// checkpoint ticks and records the bundle.
    pub fn before_step(&mut self, world: &World, cmds: &[Command]) {
        let t = world.tick();
        if t == 0 && self.confirmed.is_none() {
            // Tick 0 is the setup both clients hashed equal before `Ready`
            // let the match start: confirmed from the outset, so a desync
            // in the first 10 s still dumps a checkpoint to replay from.
            self.confirmed = Some((0, world.save()));
        } else if t.is_multiple_of(CHECKPOINT_TICKS) && t > 0 {
            if self.unconfirmed.len() == MAX_UNCONFIRMED {
                self.unconfirmed.pop_front();
            }
            self.unconfirmed.push_back((t, world.save()));
        }
        self.bundles.push_back((t, cmds.to_vec()));
    }

    /// The relay confirmed every hash up to `tick`: promote the newest
    /// checkpoint at or below it and forget what came before.
    pub fn confirm(&mut self, tick: u64) {
        while self.unconfirmed.front().is_some_and(|c| c.0 <= tick) {
            self.confirmed = self.unconfirmed.pop_front();
        }
        let keep_from = self
            .confirmed
            .as_ref()
            .map(|c| c.0)
            .or(self.unconfirmed.front().map(|c| c.0))
            .unwrap_or(0);
        // With no checkpoint at all, keep every bundle from tick 0 (the
        // setup is the checkpoint); otherwise drop bundles before it.
        if self.confirmed.is_some() {
            while self.bundles.front().is_some_and(|b| b.0 < keep_from) {
                self.bundles.pop_front();
            }
        }
    }

    pub fn checkpoint_tick(&self) -> Option<u64> {
        self.confirmed.as_ref().map(|c| c.0)
    }

    /// Writes the dump into `dir` (created). `world` is at the pause turn.
    pub fn write_dump(
        &self,
        dir: &Path,
        world: &World,
        mut manifest: DumpManifest,
        ring: &VecDeque<(u64, u64)>,
    ) -> io::Result<()> {
        std::fs::create_dir_all(dir)?;
        manifest.paused_tick = world.tick();
        manifest.state_hash = format!("{:016x}", world.state_hash());
        manifest.full_state_hash = format!("{:016x}", world.full_state_hash());
        manifest.checkpoint_tick = self.checkpoint_tick();
        manifest.hash_ring = ring
            .iter()
            .map(|(t, h)| (*t, format!("{h:016x}")))
            .collect();
        let json = serde_json::to_string_pretty(&manifest).map_err(io::Error::other)?;
        std::fs::write(dir.join("manifest.json"), json)?;
        std::fs::write(dir.join("paused.sav"), world.save())?;
        if let Some((_, save)) = &self.confirmed {
            std::fs::write(dir.join("checkpoint.sav"), save)?;
        }
        let from = self.checkpoint_tick().unwrap_or(0);
        let mut f = io::BufWriter::new(std::fs::File::create(dir.join("bundles.jsonl"))?);
        for (tick, cmds) in self.bundles.iter().filter(|b| b.0 >= from) {
            let line = serde_json::to_string(&BundleLine {
                tick: *tick,
                tick_salt: 0,
                cmds,
            })
            .map_err(io::Error::other)?;
            writeln!(f, "{line}")?;
        }
        f.flush()
    }
}
