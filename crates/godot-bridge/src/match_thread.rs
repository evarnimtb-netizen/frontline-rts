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
//!
//! **Net mode** (S2, [`MatchHandle::spawn_net`]): the loop pulls ticks from
//! a [`TickSource`]. `Local` is the path above; `Net` takes them from the
//! relay through the `frontline-net` thread ([`crate::match_net`]): each
//! tick runs at its scheduled slot once its bundle is in, frames carry that
//! slot as their timestamp, `send()` feeds the net thread, `set_paused()`
//! asks the relay to pause or resume both players, and speed stays 1x.

use std::panic::AssertUnwindSafe;
use std::sync::atomic::{
    AtomicBool, AtomicI32, AtomicI64, AtomicU8, AtomicU32, AtomicU64, Ordering,
};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use std::collections::VecDeque;

use frontline_net::client_io::ToNet;
use frontline_sim::{Command, CommandKind, PlayerId, World};

use crate::match_frame::{FrameCtx, MatchFrame, TickEvents};
use crate::match_net::{LinkIo, NetDetail, NetLink, NetMatchInfo, NetStepper};

/// Most commands applied in one tick (the rest wait for the next).
pub const MAX_COMMANDS_PER_TICK: usize = 64;

/// Shared between the sim thread and the presenter.
#[derive(Default)]
pub struct MatchCounters {
    pub ticks: AtomicU64,
    pub late_ticks: AtomicU64,
    pub last_step_us: AtomicU32,
    pub max_step_us: AtomicU32,
    /// Tick whose step took `max_step_us`.
    pub max_step_tick: AtomicU64,
    pub commands_sent: AtomicU64,
    pub paused: AtomicBool,
    /// Sim speed in percent of real time (100 = real time).
    pub speed_pct: AtomicU32,
    pub alive: AtomicBool,
    pub panic: Mutex<Option<String>>,
    /// Every command the sim ran from outside (the local player's in a
    /// skirmish, every player's in a net match), keyed by the world's tick
    /// before the step: `(tick, command)`. AI commands are made inside the
    /// sim and are not here.
    pub log: Mutex<Vec<(u64, Command)>>,
    // ---- Net mode (S2); zero / -1 in a skirmish.
    /// Round trip to the relay (ms), -1 unknown.
    pub rtt_ms: AtomicI32,
    /// D in ticks.
    pub input_delay: AtomicU32,
    /// Playout delay in ticks (this client steps a tick this long after
    /// its turn closes).
    pub playout: AtomicU32,
    /// Own commands the relay moved to a later turn.
    pub restamps: AtomicU64,
    /// Ticks stepped more than a period after their slot.
    pub stall_ticks: AtomicU64,
    /// [`crate::match_net::NetState`] as `u8`.
    pub net_state: AtomicU8,
    /// Tick whose hashes differed, -1 none.
    pub desync_tick: AtomicI64,
    /// Own commands run so far (the next own `seq`): commands with a
    /// smaller send ordinal ([`MatchHandle::send`]) have executed.
    pub own_executed: AtomicU64,
    /// Published `(tick, hash)`s, oldest first (at most
    /// [`crate::match_net::HASH_KEEP`]).
    pub hashes: Mutex<VecDeque<(u64, u64)>>,
    pub net: Mutex<NetDetail>,
}

/// Where the sim thread gets its ticks (S2 "Code changes").
pub enum TickSource {
    /// Skirmish: the local player's commands, applied in the next tick.
    Local(Receiver<CommandKind>),
    /// Net match: bundles from the relay.
    Net(Box<(NetLink, NetStepper)>),
}

enum Input {
    Local(Sender<CommandKind>),
    Net(Sender<ToNet>),
}

pub struct MatchHandle {
    output: triple_buffer::Output<MatchFrame>,
    events: Receiver<TickEvents>,
    input: Input,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pub counters: Arc<MatchCounters>,
    pub tick_hz: u32,
    /// S5b: the fire-mission tool's request (frames answer it).
    pub preview: Arc<crate::match_frame::PreviewShared>,
}

impl MatchHandle {
    /// Moves `world` onto a new thread. `local` is the player slot whose
    /// commands this handle sends and whose fog of war frames carry. Tiles
    /// the world already has (a loaded match) arrive as the first events.
    pub fn spawn(world: World, local: u8) -> std::io::Result<MatchHandle> {
        MatchHandle::spawn_scene(world, local, None)
    }

    /// [`MatchHandle::spawn`] for a staged scene (S3 `--autoplay_scene`,
    /// `frontline_sim::scenarios`): frames carry whether its condition
    /// holds ([`MatchFrame::scene_ready`]).
    pub fn spawn_scene(
        world: World,
        local: u8,
        scene: Option<String>,
    ) -> std::io::Result<MatchHandle> {
        let (cmd_tx, cmd_rx) = channel::<CommandKind>();
        MatchHandle::spawn_source(
            world,
            local,
            scene,
            TickSource::Local(cmd_rx),
            Input::Local(cmd_tx),
        )
    }

    /// Moves a net match `world` onto a new thread (S2). `slot` is this
    /// client's player; the ticks come from the relay over `link`, the
    /// commands from [`MatchHandle::send`] go to the net thread.
    pub fn spawn_net(
        world: World,
        slot: u8,
        link: NetLink,
        info: NetMatchInfo,
    ) -> std::io::Result<MatchHandle> {
        let to_net = link.to_net.clone();
        let stepper = NetStepper::new(slot, info);
        MatchHandle::spawn_source(
            world,
            slot,
            None,
            TickSource::Net(Box::new((link, stepper))),
            Input::Net(to_net),
        )
    }

    fn spawn_source(
        world: World,
        local: u8,
        scene: Option<String>,
        source: TickSource,
        input_tx: Input,
    ) -> std::io::Result<MatchHandle> {
        let setup = world
            .match_setup()
            .cloned()
            .ok_or_else(|| std::io::Error::other("not a match world"))?;
        let tick_hz = setup.tick_hz.max(1);
        let mut ctx = FrameCtx::new(&setup, local);
        ctx.scene = scene;
        let preview = Arc::clone(&ctx.preview);
        let anchor = Instant::now();
        let mut first = MatchFrame::empty();
        first.fill(&world, &mut ctx, 0.0, anchor);
        let (mut input, output) = triple_buffer::triple_buffer(&first);
        let (ev_tx, ev_rx) = channel::<TickEvents>();
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
        counters.rtt_ms.store(-1, Ordering::Relaxed);
        counters.desync_tick.store(-1, Ordering::Relaxed);
        let thread = {
            let stop = Arc::clone(&stop);
            let counters = Arc::clone(&counters);
            std::thread::Builder::new()
                .name("frontline-match".into())
                .spawn(move || {
                    let mut world = world;
                    let mut source = source;
                    let run = std::panic::catch_unwind(AssertUnwindSafe(|| {
                        let mut out = Out {
                            ctx: &mut ctx,
                            input: &mut input,
                            ev_tx: &ev_tx,
                            local,
                        };
                        match &mut source {
                            TickSource::Local(cmd_rx) => run_loop(
                                &mut world, &mut out, cmd_rx, &stop, &counters, tick_hz, anchor,
                            ),
                            TickSource::Net(net) => {
                                let (link, stepper) = &mut **net;
                                run_net_loop(&mut world, &mut out, link, stepper, &stop, &counters)
                            }
                        }
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
            input: input_tx,
            stop,
            thread: Some(thread),
            counters,
            tick_hz,
            preview,
        })
    }

    /// Queues a command of the local player: for the next tick, or in a
    /// net match for the net thread to stamp. Returns its send ordinal (in
    /// a net match, its `seq`: it has executed once
    /// `counters.own_executed` is past it), or `None` if it was dropped.
    pub fn send(&self, kind: CommandKind) -> Option<u64> {
        let ok = match &self.input {
            Input::Local(tx) => tx.send(kind).is_ok(),
            // The net client refuses commands over the wire limits; drop
            // them here so the ordinal stays the `seq`.
            Input::Net(tx) => {
                frontline_net::protocol::command_in_limits(&kind)
                    && tx.send(ToNet::Command(kind)).is_ok()
            }
        };
        ok.then(|| self.counters.commands_sent.fetch_add(1, Ordering::Relaxed))
    }

    /// Skirmish: stops or resumes stepping. Net match: asks the relay to
    /// pause or resume both players (the pause shows once it comes back).
    pub fn set_paused(&self, paused: bool) {
        match &self.input {
            Input::Local(_) => self.counters.paused.store(paused, Ordering::Relaxed),
            Input::Net(tx) => {
                let _ = tx.send(if paused { ToNet::Pause } else { ToNet::Resume });
            }
        }
    }

    pub fn is_net(&self) -> bool {
        matches!(self.input, Input::Net(_))
    }

    pub fn is_paused(&self) -> bool {
        self.counters.paused.load(Ordering::Relaxed)
    }

    /// Sim speed relative to real time (0.25–8). A net match stays at 1x.
    pub fn set_speed(&self, speed: f32) {
        if self.is_net() {
            return;
        }
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

/// Where the sim thread publishes.
struct Out<'a> {
    ctx: &'a mut FrameCtx,
    input: &'a mut triple_buffer::Input<MatchFrame>,
    ev_tx: &'a Sender<TickEvents>,
    local: u8,
}

impl Out<'_> {
    /// Publishes a frame stamped `sched`, and after a step the tick's
    /// events.
    fn publish(&mut self, world: &World, step_ms: f32, sched: Instant, paused: bool, step: bool) {
        let f = self.input.input_buffer_mut();
        f.fill(world, self.ctx, step_ms, sched);
        f.paused = paused;
        f.published = Instant::now();
        self.input.publish();
        if step {
            let ev = TickEvents::collect(world, self.ctx);
            if !ev.is_empty() {
                // The receiver only goes away when the presenter is gone.
                let _ = self.ev_tx.send(ev);
            }
        }
    }
}

fn record_step(counters: &MatchCounters, world: &World, step: Duration) {
    let us = step.as_micros().min(u32::MAX as u128) as u32;
    counters.ticks.fetch_add(1, Ordering::Relaxed);
    counters.last_step_us.store(us, Ordering::Relaxed);
    if counters.max_step_us.fetch_max(us, Ordering::Relaxed) < us {
        counters
            .max_step_tick
            .store(world.tick(), Ordering::Relaxed);
    }
}

fn run_loop(
    world: &mut World,
    out: &mut Out,
    cmd_rx: &Receiver<CommandKind>,
    stop: &AtomicBool,
    counters: &MatchCounters,
    tick_hz: u32,
    anchor: Instant,
) {
    let local = out.local;
    let base_period = Duration::from_secs_f64(1.0 / tick_hz as f64);
    let mut next = anchor + base_period;
    let mut seq: u32 = 0;
    let mut cmds: Vec<Command> = Vec::new();
    let mut was_paused = false;
    while !stop.load(Ordering::Relaxed) {
        let paused = counters.paused.load(Ordering::Relaxed);
        if paused {
            // Republish so the presenter sees the pause flag, and (S5b) for
            // each new fire-mission tool request.
            let asked = out.ctx.preview.generation.load(Ordering::Relaxed) != out.ctx.preview_gen;
            if !was_paused || asked {
                out.publish(world, 0.0, next - base_period, true, false);
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
        let tick = world.tick();
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
        out.publish(world, ms, next, false, true);
        record_step(counters, world, step);
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

/// Net mode: steps whatever the relay's schedule makes due (see
/// [`NetStepper`]), stamps each frame with the tick's slot, and sleeps
/// until the next slot (at most 2 ms, so events are read promptly).
fn run_net_loop(
    world: &mut World,
    out: &mut Out,
    link: &mut NetLink,
    stepper: &mut NetStepper,
    stop: &AtomicBool,
    counters: &MatchCounters,
) {
    let origin = link.origin;
    let mut was_halted = false;
    while !stop.load(Ordering::Relaxed) {
        let stepped = stepper.poll(world, link, counters);
        let halted = stepper.halted(world);
        if let Some(s) = &stepped {
            if !s.cmds.is_empty()
                && let Ok(mut log) = counters.log.lock()
            {
                log.extend(s.cmds.iter().map(|c| (s.tick, c.clone())));
            }
            let sched = origin + Duration::from_micros(s.slot_us);
            out.publish(world, s.step_us as f32 / 1000.0, sched, halted, true);
            record_step(counters, world, Duration::from_micros(s.step_us as u64));
            let period = stepper.status().schedule.map_or(33_333, |p| p.period());
            if link.now_us() > s.slot_us + period {
                counters.late_ticks.fetch_add(1, Ordering::Relaxed);
            }
        } else if halted != was_halted {
            // Republish so the presenter sees the pause flag change.
            let sched = stepper
                .slot_of(world.tick().saturating_sub(1))
                .map_or_else(Instant::now, |s| origin + Duration::from_micros(s));
            out.publish(world, 0.0, sched, halted, false);
        }
        was_halted = halted;
        if stepped.is_some() {
            continue;
        }
        let now = link.now_us();
        let wait = stepper
            .slot_of(world.tick())
            .map_or(2_000, |due| due.saturating_sub(now))
            .clamp(500, 2_000);
        std::thread::sleep(Duration::from_micros(wait));
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
