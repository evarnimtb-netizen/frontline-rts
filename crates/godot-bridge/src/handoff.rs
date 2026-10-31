//! The sim thread and the lock-free snapshot handoff (§4.1 rules 1, 7).
//!
//! The [`World`] lives on its own OS thread, outside Godot's main loop. It
//! steps at the match tick rate on a paced clock and never waits on the
//! presenter: a slow frame only means the presenter skips snapshots. After
//! each tick the thread copies the render data into the back buffer of a
//! triple buffer and publishes it (no locks; the main thread swaps the
//! newest buffer out in O(1)).
//!
//! Threads: `World::step` runs the whole tick inside the world's own rayon
//! pool (`sim_threads` workers). The pacing thread here only sleeps, copies
//! the snapshot out and blocks in `install` while a worker runs the tick, so
//! the sim uses `sim_threads` cores, not `sim_threads + 1`. §4.1 rule 7 asks
//! for the sim thread to *join* the pool (`use_current_thread`); that needs
//! the pool to be built on this thread inside the sim crate and is not done
//! in the M0 spike. Thread pinning and the Godot worker cap are not done here
//! either (the worker cap is a project setting, see `godot/project.godot`).

use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use frontline_sim::projectile::SUBSTEP_HZ;
use frontline_sim::{EntityKind, World};

/// One published tick, in sim axes.
#[derive(Clone, Debug)]
pub struct Frame {
    pub tick: u64,
    /// `[x, y, z, yaw]` per live soldier, ascending id.
    pub soldiers: Vec<[f32; 4]>,
    pub ids: Vec<u32>,
    /// Owner (team) per soldier.
    pub teams: Vec<u8>,
    /// Planar speed (m/s) per soldier, from the sim's velocity component.
    pub speeds: Vec<f32>,
    /// `[x, y, z, vx, vy, vz, age_s]` per live projectile (m, m/s, s since
    /// it left the muzzle, as of the end of this tick).
    pub projectiles: Vec<[f32; 7]>,
    /// Wall time of the `World::step` that produced this frame.
    pub step_ms: f32,
    /// Scheduled (not actual) start of the tick that produced this frame.
    /// Interpolation runs on these stamps so step-time jitter does not leak
    /// into the drawn motion.
    pub sched: Instant,
    /// When the frame was published (for the snapshot-age statistic).
    pub published: Instant,
}

impl Frame {
    pub fn empty() -> Frame {
        let now = Instant::now();
        Frame {
            tick: 0,
            soldiers: Vec::new(),
            ids: Vec::new(),
            teams: Vec::new(),
            speeds: Vec::new(),
            projectiles: Vec::new(),
            step_ms: 0.0,
            sched: now,
            published: now,
        }
    }

    fn fill(&mut self, world: &World, step_ms: f32, sched: Instant) {
        let snap = world.snapshot();
        self.tick = snap.tick;
        self.soldiers.clone_from(&snap.soldiers);
        self.ids.clear();
        self.ids.extend(snap.soldier_ids.iter().map(|id| id.0));
        let ents = world.entities();
        let (kind, owner, vel) = (ents.kind(), ents.owner(), ents.velocity());
        self.teams.clear();
        self.speeds.clear();
        for &id in &self.ids {
            let i = id as usize;
            debug_assert_eq!(kind[i], EntityKind::Soldier);
            self.teams.push(owner[i].0);
            let [vx, vy] = vel[i].lin.to_f32();
            self.speeds.push((vx * vx + vy * vy).sqrt());
        }
        // The snapshot has positions only; velocity and remaining lifetime
        // come straight from the pool (read-only, same tick), so tracers can
        // be oriented and clipped at the muzzle without changing the sim.
        let ttl0 = world.config().weapon.ttl_substeps.max(1) as f32;
        let substep_s = 1.0 / SUBSTEP_HZ as f32;
        self.projectiles.clear();
        self.projectiles
            .extend(world.projectiles().slots().iter().flatten().map(|p| {
                let [x, y, z] = p.pos.to_f32();
                let [vx, vy, vz] = p.vel.to_f32();
                let age = (ttl0 - p.ttl as f32).max(0.0) * substep_s;
                [x, y, z, vx, vy, vz, age]
            }));
        self.step_ms = step_ms;
        self.sched = sched;
        self.published = Instant::now();
    }
}

/// Ring of the most recent per-tick step times (ms).
#[derive(Default)]
pub struct StepWindow {
    samples: Vec<f32>,
    next: usize,
}

impl StepWindow {
    /// Most recent samples kept (~36 min at 30 Hz).
    pub const CAP: usize = 1 << 16;

    pub fn push(&mut self, ms: f32) {
        if self.samples.len() < Self::CAP {
            self.samples.push(ms);
        } else {
            self.samples[self.next] = ms;
        }
        self.next = (self.next + 1) % Self::CAP;
    }

    pub fn clear(&mut self) {
        self.samples.clear();
        self.next = 0;
    }

    pub fn samples(&self) -> &[f32] {
        &self.samples
    }
}

/// Counters the sim thread updates and the presenter reads.
#[derive(Default)]
pub struct SimCounters {
    pub ticks: AtomicU64,
    /// Ticks that finished after their scheduled deadline.
    pub late_ticks: AtomicU64,
    /// Times the pacer gave up catching up and re-anchored its clock.
    pub resyncs: AtomicU64,
    pub last_step_us: AtomicU32,
    /// Step-time samples dropped because a reader held the window lock (the
    /// sim thread only ever `try_lock`s, so a reader can never delay a tick).
    pub dropped_samples: AtomicU64,
    /// Cleared when the sim thread exits by panicking.
    pub alive: AtomicBool,
    /// Panic message of the sim thread, if it died.
    pub panic: Mutex<Option<String>>,
    pub window: Mutex<StepWindow>,
}

pub struct SimHandle {
    output: triple_buffer::Output<Frame>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pub counters: Arc<SimCounters>,
    pub tick_hz: u32,
}

impl SimHandle {
    /// Moves `world` onto a new thread that steps it at `tick_hz`.
    pub fn spawn(world: World, tick_hz: u32) -> std::io::Result<SimHandle> {
        let period = Duration::from_secs_f64(1.0 / tick_hz.max(1) as f64);
        let anchor = Instant::now();
        let mut first = Frame::empty();
        // The initial state is "after tick 0", one period before tick 1.
        first.fill(&world, 0.0, anchor);
        let (mut input, output) = triple_buffer::triple_buffer(&first);
        let stop = Arc::new(AtomicBool::new(false));
        let counters = Arc::new(SimCounters::default());
        counters.alive.store(true, Ordering::Relaxed);
        let thread = {
            let stop = Arc::clone(&stop);
            let counters = Arc::clone(&counters);
            std::thread::Builder::new()
                .name("frontline-sim".into())
                .spawn(move || {
                    let mut world = world;
                    let run = std::panic::catch_unwind(AssertUnwindSafe(|| {
                        let mut next = anchor + period;
                        while !stop.load(Ordering::Relaxed) {
                            let now = Instant::now();
                            if now < next {
                                std::thread::sleep((next - now).min(Duration::from_millis(5)));
                                continue;
                            }
                            let t0 = Instant::now();
                            world.step(&[]);
                            let step = t0.elapsed();
                            let ms = step.as_secs_f32() * 1000.0;
                            input.input_buffer_mut().fill(&world, ms, next);
                            input.publish();
                            counters.ticks.fetch_add(1, Ordering::Relaxed);
                            counters.last_step_us.store(
                                step.as_micros().min(u32::MAX as u128) as u32,
                                Ordering::Relaxed,
                            );
                            // Never block on a reader: drop the sample.
                            match counters.window.try_lock() {
                                Ok(mut w) => w.push(ms),
                                Err(_) => {
                                    counters.dropped_samples.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                            let done = Instant::now();
                            next += period;
                            if done > next {
                                counters.late_ticks.fetch_add(1, Ordering::Relaxed);
                                // More than 3 ticks behind: stop trying to
                                // catch up (the presenter never slows the
                                // sim, but an overloaded sim must not spiral
                                // either).
                                if done > next + 3 * period {
                                    next = done + period;
                                    counters.resyncs.fetch_add(1, Ordering::Relaxed);
                                }
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
        Ok(SimHandle {
            output,
            stop,
            thread: Some(thread),
            counters,
            tick_hz,
        })
    }

    /// Swaps the current output buffer (the initial frame until the first
    /// publish) into `slot`. Used once at start, so something is drawn
    /// before the first tick lands.
    pub fn take_current(&mut self, slot: &mut Frame) {
        std::mem::swap(slot, self.output.output_buffer_mut());
    }

    /// If a newer frame was published, swaps it into `slot` (the previous
    /// contents go back to the buffer pool) and returns true.
    pub fn take_newest(&mut self, slot: &mut Frame) -> bool {
        if self.output.update() {
            std::mem::swap(slot, self.output.output_buffer_mut());
            true
        } else {
            false
        }
    }

    /// False once the sim thread has died from a panic.
    pub fn is_alive(&self) -> bool {
        self.counters.alive.load(Ordering::Acquire)
    }

    pub fn panic_message(&self) -> Option<String> {
        self.counters.panic.lock().ok().and_then(|p| p.clone())
    }

    /// Step-time summary over the window: (count, mean, p50, p95, p99, max).
    /// Copies the window under the lock (the sim thread drops a sample if it
    /// finds the lock held) and sorts outside it. Not for per-frame use.
    pub fn step_summary(&self) -> (usize, f32, f32, f32, f32, f32) {
        let mut v = self
            .counters
            .window
            .lock()
            .map(|w| w.samples().to_vec())
            .unwrap_or_default();
        summarize(&mut v)
    }

    pub fn reset_window(&self) {
        if let Ok(mut w) = self.counters.window.lock() {
            w.clear();
        }
        self.counters.late_ticks.store(0, Ordering::Relaxed);
        self.counters.resyncs.store(0, Ordering::Relaxed);
        self.counters.dropped_samples.store(0, Ordering::Relaxed);
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            // A panic is caught inside the thread and reported through
            // `is_alive` / `panic_message`, so join only fails on a panic in
            // the reporting itself.
            if t.join().is_err() {
                self.counters.alive.store(false, Ordering::Release);
            }
        }
    }
}

impl Drop for SimHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// (count, mean, p50, p95, p99, max); sorts `v`.
pub fn summarize(v: &mut [f32]) -> (usize, f32, f32, f32, f32, f32) {
    if v.is_empty() {
        return (0, 0.0, 0.0, 0.0, 0.0, 0.0);
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    let pct = |p: f32| v[(((n - 1) as f32) * p).round() as usize];
    let mean = v.iter().map(|&x| x as f64).sum::<f64>() / n as f64;
    (n, mean as f32, pct(0.5), pct(0.95), pct(0.99), v[n - 1])
}

#[cfg(test)]
mod tests {
    use super::*;
    use frontline_sim::SimConfig;

    #[test]
    fn sim_thread_publishes_frames() {
        let cfg = SimConfig {
            soldiers: 64,
            map_size_m: 256,
            ..SimConfig::default()
        };
        let world = World::new(cfg, 1).unwrap();
        let mut h = SimHandle::spawn(world, 30).unwrap();
        let mut f = Frame::empty();
        h.take_current(&mut f);
        assert_eq!(f.tick, 0);
        assert_eq!(f.soldiers.len(), 64, "initial frame is available at once");
        let t0 = Instant::now();
        let mut seen = 0;
        let mut last_sched = f.sched;
        while t0.elapsed() < Duration::from_millis(400) {
            if h.take_newest(&mut f) {
                seen += 1;
                assert_eq!(f.soldiers.len(), f.ids.len());
                assert_eq!(f.teams.len(), f.ids.len());
                assert_eq!(f.speeds.len(), f.ids.len());
                assert!(f.sched > last_sched, "scheduled stamps increase");
                last_sched = f.sched;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        h.stop();
        assert!(h.is_alive());
        assert!(seen >= 5, "saw {seen} frames");
        assert!(f.tick >= 5);
        let (n, ..) = h.step_summary();
        assert!(n >= 5);
    }

    #[test]
    fn step_window_is_a_ring() {
        let mut w = StepWindow::default();
        for i in 0..StepWindow::CAP + 10 {
            w.push(i as f32);
        }
        assert_eq!(w.samples().len(), StepWindow::CAP);
        assert_eq!(w.samples()[0], StepWindow::CAP as f32);
        w.clear();
        assert!(w.samples().is_empty());
    }

    #[test]
    fn summary_percentiles() {
        let mut v: Vec<f32> = (1..=100).map(|x| x as f32).collect();
        let (n, mean, p50, p95, p99, max) = summarize(&mut v);
        assert_eq!(n, 100);
        assert!((mean - 50.5).abs() < 1e-4);
        assert_eq!((p50, p95, p99, max), (51.0, 95.0, 99.0, 100.0));
    }
}
