//! The match sim thread (§4.1 rules 1, 7): steps a match [`World`] at its
//! tick rate, feeds it the local player's commands, and publishes
//! [`MatchFrame`]s (triple buffer, newest wins) and [`TickEvents`]
//! (channel, lossless). It never waits on the presenter.
//!
//! The AI players run inside `World::step`; this thread only adds the local
//! player's commands. Each command the presenter queues is applied in the
//! next tick with the next sequence number, so the match is a pure function
//! of the seed and the command log (S1 "Hard rules"). The log is kept here
//! and can be read back for replays and bug reports.
//!
//! Pause (single player vs AI only) stops stepping; `speed` runs the sim
//! faster than real time (scripted runs, `--autoplay`). Neither changes
//! results: they only change when ticks run.

use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use frontline_sim::{Command, CommandKind, PlayerId, World};

use crate::match_frame::{FrameCtx, MatchFrame, TickEvents};

/// Most commands applied in one tick (the rest wait for the next).
pub const MAX_COMMANDS_PER_TICK: usize = 64;

/// Shared between the sim thread and the presenter.
#[derive(Default)]
pub struct MatchCounters {
    pub ticks: AtomicU64,
    pub late_ticks: AtomicU64,
    pub last_step_us: AtomicU32,
    pub max_step_us: AtomicU32,
    pub commands_sent: AtomicU64,
    pub paused: AtomicBool,
    /// Sim speed in percent of real time (100 = real time).
    pub speed_pct: AtomicU32,
    pub alive: AtomicBool,
    pub panic: Mutex<Option<String>>,
    /// The local player's command log: `(tick applied, command)`.
    pub log: Mutex<Vec<(u64, Command)>>,
}

pub struct MatchHandle {
    output: triple_buffer::Output<MatchFrame>,
    events: Receiver<TickEvents>,
    commands: Sender<CommandKind>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pub counters: Arc<MatchCounters>,
    pub tick_hz: u32,
}

impl MatchHandle {
    /// Moves `world` onto a new thread. `local` is the player slot whose
    /// commands this handle sends and whose fog of war frames carry. Tiles
    /// the world already has (a loaded match) arrive as the first events.
    pub fn spawn(world: World, local: u8) -> std::io::Result<MatchHandle> {
        let setup = world
            .match_setup()
            .cloned()
            .ok_or_else(|| std::io::Error::other("not a match world"))?;
        let tick_hz = setup.tick_hz.max(1);
        let mut ctx = FrameCtx::new(&setup, local);
        let anchor = Instant::now();
        let mut first = MatchFrame::empty();
        first.fill(&world, &mut ctx, 0.0, anchor);
        let (mut input, output) = triple_buffer::triple_buffer(&first);
        let (ev_tx, ev_rx) = channel::<TickEvents>();
        let (cmd_tx, cmd_rx) = channel::<CommandKind>();
        // Tiles that exist before the first tick (none in a new match).
        let tiles = TickEvents::all_tiles(&world);
        if !tiles.is_empty() {
            let _ = ev_tx.send(TickEvents {
                tick: world.tick(),
                tiles,
                ..TickEvents::default()
            });
        }
        let stop = Arc::new(AtomicBool::new(false));
        let counters = Arc::new(MatchCounters::default());
        counters.alive.store(true, Ordering::Relaxed);
        counters.speed_pct.store(100, Ordering::Relaxed);
        let thread = {
            let stop = Arc::clone(&stop);
            let counters = Arc::clone(&counters);
            std::thread::Builder::new()
                .name("frontline-match".into())
                .spawn(move || {
                    let mut world = world;
                    let run = std::panic::catch_unwind(AssertUnwindSafe(|| {
                        run_loop(
                            &mut world, &mut ctx, &mut input, &ev_tx, &cmd_rx, &stop, &counters,
                            tick_hz, local, anchor,
                        );
                    }));
                    if let Err(e) = run {
                        let msg = e
                            .downcast_ref::<&str>()
                            .map(|s| (*s).to_owned())
                            .or_else(|| e.downcast_ref::<String>().cloned())
                            .unwrap_or_else(|| "unknown panic payload".to_owned());
                        if let Ok(mut p) = counters.panic.lock() {
                            *p = Some(msg);
                        }
                        counters.alive.store(false, Ordering::Release);
                    }
                })?
        };
        Ok(MatchHandle {
            output,
            events: ev_rx,
            commands: cmd_tx,
            stop,
            thread: Some(thread),
            counters,
            tick_hz,
        })
    }

    /// Queues a command of the local player for the next tick.
    pub fn send(&self, kind: CommandKind) {
        if self.commands.send(kind).is_ok() {
            self.counters.commands_sent.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn set_paused(&self, paused: bool) {
        self.counters.paused.store(paused, Ordering::Relaxed);
    }

    pub fn is_paused(&self) -> bool {
        self.counters.paused.load(Ordering::Relaxed)
    }

    /// Sim speed relative to real time (0.25–8).
    pub fn set_speed(&self, speed: f32) {
        let pct = (speed.clamp(0.25, 8.0) * 100.0).round() as u32;
        self.counters.speed_pct.store(pct, Ordering::Relaxed);
    }

    pub fn speed(&self) -> f32 {
        self.counters.speed_pct.load(Ordering::Relaxed) as f32 / 100.0
    }

    /// Swaps the current output buffer (the initial frame until the first
    /// publish) into `slot`.
    pub fn take_current(&mut self, slot: &mut MatchFrame) {
        std::mem::swap(slot, self.output.output_buffer_mut());
    }

    /// If a newer frame was published, swaps it into `slot` and returns true.
    pub fn take_newest(&mut self, slot: &mut MatchFrame) -> bool {
        if self.output.update() {
            std::mem::swap(slot, self.output.output_buffer_mut());
            true
        } else {
            false
        }
    }

    /// Every tick's events since the last call, oldest first.
    pub fn drain_events(&self, out: &mut Vec<TickEvents>) {
        out.extend(self.events.try_iter());
    }

    pub fn is_alive(&self) -> bool {
        self.counters.alive.load(Ordering::Acquire)
    }

    pub fn panic_message(&self) -> Option<String> {
        self.counters.panic.lock().ok().and_then(|p| p.clone())
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take()
            && t.join().is_err()
        {
            self.counters.alive.store(false, Ordering::Release);
        }
    }
}

impl Drop for MatchHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

#[allow(clippy::too_many_arguments)]
fn run_loop(
    world: &mut World,
    ctx: &mut FrameCtx,
    input: &mut triple_buffer::Input<MatchFrame>,
    ev_tx: &Sender<TickEvents>,
    cmd_rx: &Receiver<CommandKind>,
    stop: &AtomicBool,
    counters: &MatchCounters,
    tick_hz: u32,
    local: u8,
    anchor: Instant,
) {
    let base_period = Duration::from_secs_f64(1.0 / tick_hz as f64);
    let mut next = anchor + base_period;
    let mut seq: u32 = 0;
    let mut cmds: Vec<Command> = Vec::new();
    let mut was_paused = false;
    while !stop.load(Ordering::Relaxed) {
        let paused = counters.paused.load(Ordering::Relaxed);
        if paused {
            if !was_paused {
                // Republish so the presenter sees the pause flag.
                let f = input.input_buffer_mut();
                f.fill(world, ctx, 0.0, next - base_period);
                f.paused = true;
                f.published = Instant::now();
                input.publish();
            }
            was_paused = true;
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }
        // A finished match stays on its last frame.
        if world
            .game()
            .is_some_and(|g| g.outcome() != frontline_sim::Outcome::Running)
        {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        let speed = counters.speed_pct.load(Ordering::Relaxed).max(1) as f64 / 100.0;
        let period = base_period.div_f64(speed);
        if was_paused {
            was_paused = false;
            next = Instant::now() + period;
        }
        let now = Instant::now();
        if now < next {
            std::thread::sleep((next - now).min(Duration::from_millis(5)));
            continue;
        }
        cmds.clear();
        for kind in cmd_rx.try_iter().take(MAX_COMMANDS_PER_TICK) {
            cmds.push(Command {
                player: PlayerId(local),
                seq,
                kind,
            });
            seq += 1;
        }
        let tick = world.tick() + 1;
        if !cmds.is_empty()
            && let Ok(mut log) = counters.log.lock()
        {
            log.extend(cmds.iter().map(|c| (tick, c.clone())));
        }
        let t0 = Instant::now();
        world.step(&cmds);
        let step = t0.elapsed();
        let ms = step.as_secs_f32() * 1000.0;
        // The scheduled stamp runs at real-time spacing scaled by speed, so
        // interpolation stays smooth at any speed.
        let f = input.input_buffer_mut();
        f.fill(world, ctx, ms, next);
        f.paused = false;
        f.published = Instant::now();
        input.publish();
        let ev = TickEvents::collect(world, local);
        if !ev.is_empty() {
            // The receiver only goes away when the presenter is gone.
            let _ = ev_tx.send(ev);
        }
        let us = step.as_micros().min(u32::MAX as u128) as u32;
        counters.ticks.fetch_add(1, Ordering::Relaxed);
        counters.last_step_us.store(us, Ordering::Relaxed);
        counters.max_step_us.fetch_max(us, Ordering::Relaxed);
        let done = Instant::now();
        next += period;
        if done > next {
            counters.late_ticks.fetch_add(1, Ordering::Relaxed);
            if done > next + 3 * period {
                next = done + period;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::match_frame::tests_support::test_world;

    #[test]
    fn thread_steps_applies_commands_and_pauses() {
        let w = test_world(false);
        let mut h = MatchHandle::spawn(w, 0).unwrap();
        let mut f = MatchFrame::empty();
        h.take_current(&mut f);
        assert_eq!(f.tick, 0);
        h.set_speed(4.0);
        std::thread::sleep(Duration::from_millis(100));
        h.set_paused(true);
        std::thread::sleep(Duration::from_millis(60));
        h.take_newest(&mut f);
        let paused_at = f.tick;
        assert!(f.paused && paused_at > 0);
        std::thread::sleep(Duration::from_millis(100));
        assert!(!h.take_newest(&mut f) || f.tick == paused_at);
        h.set_paused(false);
        std::thread::sleep(Duration::from_millis(100));
        h.take_newest(&mut f);
        assert!(f.tick > paused_at && !f.paused);

        h.send(CommandKind::Surrender);
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(3) {
            h.take_newest(&mut f);
            if f.outcome != frontline_sim::Outcome::Running {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            f.outcome,
            frontline_sim::Outcome::Won {
                winner: PlayerId(1)
            }
        );
        let log = h.counters.log.lock().unwrap().clone();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].1.seq, 0);
        // A finished match stops stepping.
        let end = f.tick;
        std::thread::sleep(Duration::from_millis(100));
        h.take_newest(&mut f);
        assert_eq!(f.tick, end);
        h.stop();
        assert!(h.is_alive(), "{:?}", h.panic_message());
    }
}
