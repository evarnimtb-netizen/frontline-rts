//! Save/load roundtrip and mutation fuzz (docs/process/dev-speed.md 7).
//!
//! 1. Roundtrip: an AI-vs-AI skirmish on the shipped mod with the `[ai.fort]`
//!    switches on is saved and loaded every 600 ticks; the hash must match
//!    at the load and, at some points, after both worlds run on.
//! 2. Mutation fuzz: mid-match saves are mutated one encoded value at a time
//!    (positions from the save's own field layout, chosen by a counter rule).
//!    `World::load` must reject the bytes, or the loaded world must run 300
//!    ticks without a panic or a hang.
//!
//! 3. Regressions: the fuzz findings of 2026-11-11, one named test each.
//!
//! The roundtrip and the regressions run in every release test pass; the
//! fuzz (about 45 s on 4 workers) is ignored and runs in
//! `tools/check.sh tests`. Run it alone with
//! `cargo test --release -p frontline-sim --test save_fuzz -- --ignored --nocapture`.

// Wall-clock time is only reported, and the atomic only hands out work to
// the fuzz workers; neither feeds a sim.
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

#[path = "support/save_layout.rs"]
mod save_layout;

use std::path::Path;
use std::time::Instant;

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::{PlayerId, World};

fn setup(seed: u64) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let req = MatchRequest::skirmish(seed, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let mut s = defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
    s.players.iter_mut().for_each(|p| p.is_ai = true);
    s
}

/// What the state holds that the fuzz should reach.
fn holds(w: &World) -> Vec<&'static str> {
    let g = w.game().unwrap();
    let f = g.fort();
    let v = g.vehicles();
    let mut out = Vec::new();
    let mut put = |b: bool, s| {
        if b {
            out.push(s)
        }
    };
    put(!f.works.is_empty(), "works");
    put(!f.pieces.is_empty(), "pieces");
    put(!f.wire.is_empty(), "wire");
    put(!f.charges.is_empty(), "charges");
    put(f.carried_g.values().any(|g| *g > 0), "carried");
    put(v.values().any(|v| v.material_g > 0), "truck_material");
    put(v.values().any(|v| v.served.is_some()), "served_gun");
    put(v.values().any(|v| v.towing.is_some()), "towing");
    put(!v.is_empty(), "vehicles");
    let ai = |p: u8| g.ai(PlayerId(p)).unwrap();
    put((0..2).any(|p| !ai(p).forts.is_empty()), "ai_forts");
    put((0..2).any(|p| !ai(p).breaches.is_empty()), "ai_breaches");
    put(
        (0..2).any(|p| ai(p).material_run.is_some()),
        "ai_material_run",
    );
    put((0..2).any(|p| !ai(p).tasks.is_empty()), "ai_tasks");
    out
}

#[derive(serde::Serialize)]
struct SaveMirror<'a> {
    magic: [u8; 8],
    version: u32,
    state: &'a frontline_sim::world::SimState,
}

fn layout(w: &World) -> save_layout::Layout {
    let l = save_layout::Layout::of(&SaveMirror {
        magic: *b"FLSIM\0\0\0",
        version: 24,
        state: w.state(),
    });
    assert!(
        l.bytes == w.save(),
        "the layout serialiser no longer matches World::save"
    );
    l
}

const SEED: u64 = 1;
/// Seed 1 at 14400 ticks holds works, pieces, wire, carried material, a
/// truck with material and the AI's material run, a served and a towed gun,
/// vehicles, AI fort jobs and tasks (`holds`). No AI-vs-AI seed tried (1, 3,
/// 6, 7 to 22 minutes) shows breaches or charges.
const MUST_HOLD: [&str; 11] = [
    "works",
    "pieces",
    "wire",
    "carried",
    "truck_material",
    "served_gun",
    "towing",
    "vehicles",
    "ai_forts",
    "ai_material_run",
    "ai_tasks",
];

fn run_hashes(w: &mut World, ticks: u64) -> Vec<u64> {
    let mut h = Vec::new();
    for _ in 0..ticks {
        w.step(&[]);
        if w.tick().is_multiple_of(30) {
            h.push(w.state_hash());
        }
    }
    h
}

/// Part 1: save and load every 600 ticks of a 12-minute fort match; the
/// hash matches at the load and, at four points, after 600 more ticks of
/// both worlds, and the loaded world saves the same bytes. Fails if
/// `World::load` drops or rebuilds any state wrongly, or rejects a state
/// the match reached.
#[test]
fn save_roundtrip_through_a_fort_match() {
    let t = Instant::now();
    let mut w = World::new_match(setup(SEED), 1).unwrap();
    let (mut loads, mut runs) = (0, 0);
    let mut seen: Vec<&str> = Vec::new();
    while w.tick() < 21_600 {
        run_hashes(&mut w, 600);
        let tick = w.tick();
        let bytes = w.save();
        let mut b = World::load(&bytes, 1).unwrap_or_else(|e| panic!("load at {tick}: {e}"));
        loads += 1;
        assert_eq!(
            w.state_hash(),
            b.state_hash(),
            "hash at the load, tick {tick}"
        );
        assert_eq!(
            b.state_hash(),
            b.full_state_hash(),
            "cache at the load, tick {tick}"
        );
        assert_eq!(b.save(), bytes, "save of the loaded world, tick {tick}");
        for h in holds(&w) {
            if !seen.contains(&h) {
                seen.push(h);
            }
        }
        if tick == 14_400 {
            let h = holds(&w);
            let miss: Vec<_> = MUST_HOLD.iter().filter(|m| !h.contains(m)).collect();
            assert!(miss.is_empty(), "state at 14400 lacks {miss:?}");
        }
        if [4_800, 9_600, 14_400, 19_200].contains(&tick) {
            let mut a = World::load(&bytes, 2).unwrap();
            assert_eq!(
                run_hashes(&mut a, 600),
                run_hashes(&mut b, 600),
                "after the load at {tick}"
            );
            runs += 1;
        }
    }
    println!(
        "roundtrip: {loads} loads, {runs} continued runs, held {seen:?}, {:?}",
        t.elapsed()
    );
}

/// One mutation: replace the encoded value of `field` with `new`.
#[derive(Clone, Debug)]
struct Mutation {
    save: usize,
    field: save_layout::Field,
    new: Value,
}

#[derive(Clone, Copy, Debug)]
enum Value {
    Int(i128),
    F32(f32),
    F64(f64),
    /// A raw byte (an out-of-range bool or option tag).
    Byte(u8),
}

fn encode(kind: save_layout::Kind, v: Value) -> Vec<u8> {
    use save_layout::Kind as K;
    let mut out = Vec::new();
    match (kind, v) {
        (_, Value::Byte(b)) => out.push(b),
        (_, Value::F32(f)) => out.extend_from_slice(&f.to_le_bytes()),
        (_, Value::F64(f)) => out.extend_from_slice(&f.to_le_bytes()),
        (K::Bool | K::U8 | K::I8 | K::Tag, Value::Int(i)) => out.push(i as u8),
        (K::I(_), Value::Int(i)) => save_layout::varint(save_layout::zigzag(i), &mut out),
        (_, Value::Int(i)) => save_layout::varint(i as u128, &mut out),
    }
    out
}

/// The values one field is set to, in a fixed order.
fn candidates(f: &save_layout::Field) -> Vec<Value> {
    use Value::Int;
    use save_layout::Kind as K;
    let o = f.value;
    match f.kind {
        K::Bool => vec![Int(1 - o), Value::Byte(2)],
        K::Tag => vec![Int(1 - o), Value::Byte(2)],
        K::U8 => vec![Int(0), Int(255), Int(1), Int(254), Int((o + 1) & 255)],
        K::I8 => vec![Int(0), Int(-128), Int(127), Int(-1), Int(1)],
        K::U(b) => {
            let max = if b >= 128 {
                i128::MAX
            } else {
                (1i128 << b) - 1
            };
            vec![
                Int(max),
                Int(0),
                Int(o + 1),
                Int(max - 1),
                Int(1),
                Int((o + 1000).min(max)),
                // Huge but not the top: 3/4 of the range. (Exactly at a
                // load bound would load and then count past it, which
                // the reload check would report.)
                Int(max - max / 4),
            ]
        }
        K::I(b) => {
            let max = (1i128 << (b - 1)) - 1;
            // Fx (Q16.16) coordinates: 20 km off the map either way.
            let far = 20_000i128 << 16;
            vec![
                Int(max),
                Int(0),
                Int(-max - 1),
                Int(-1),
                Int((o + far).min(max)),
                Int(1),
                Int((o - far).max(-max - 1)),
                Int(max - 1),
            ]
        }
        K::Len => vec![
            Int(o + 1),
            Int(0),
            Int(u32::MAX as i128),
            Int((o - 1).max(0)),
            Int(1 << 40),
        ],
        K::Variant => vec![Int(o + 1), Int(255), Int((o - 1).max(0))],
        K::F32 => vec![
            Value::F32(f32::NAN),
            Value::F32(f32::INFINITY),
            Value::F32(-1e30),
        ],
        K::F64 => vec![
            Value::F64(f64::NAN),
            Value::F64(f64::INFINITY),
            Value::F64(-1e300),
        ],
    }
}

/// Fort, vehicle and AI state, and the entity columns that link to it.
fn focus(path: &str) -> bool {
    [
        "state.game.fort",
        "state.game.vehicles",
        "state.game.ai",
        "state.entities",
    ]
    .iter()
    .any(|p| path.starts_with(p))
}

/// The mutation plan for one save, fixed by counters: every field path gets
/// `occ` occurrences (spread over its fields by a multiplicative step) and
/// `vals` values (rotating through `candidates` with the save number).
/// Setup paths get one value in one save of three.
fn plan(save: usize, l: &save_layout::Layout) -> Vec<Mutation> {
    let mut by_path: Vec<Vec<usize>> = vec![Vec::new(); l.paths.len()];
    for (i, f) in l.fields.iter().enumerate() {
        by_path[f.path as usize].push(i);
    }
    let mut out = Vec::new();
    for (p, fields) in by_path.iter().enumerate() {
        let path = &l.paths[p];
        if fields.is_empty() || path.starts_with("magic") || path == "version" {
            continue;
        }
        let setup = path.starts_with("state.game.setup") || path.starts_with("state.config");
        let (occ, vals) = match () {
            _ if setup && p % 3 != save => continue,
            _ if setup => (1, 1),
            _ if focus(path) => (2, 3),
            _ => (1, 2),
        };
        for j in 0..occ.min(fields.len()) {
            let k = (j * 2_654_435_761 + p * 7_919 + save * 104_729) % fields.len();
            let field = l.fields[fields[k]];
            let c = candidates(&field);
            for v in 0..vals {
                let new = c[(p + save * vals + j + v) % c.len()];
                if encode(field.kind, new) != l.bytes[field.offset..field.offset + field.len] {
                    out.push(Mutation { save, field, new });
                }
            }
        }
    }
    out
}

fn apply(bytes: &[u8], m: &Mutation) -> Vec<u8> {
    let f = &m.field;
    let mut out = bytes[..f.offset].to_vec();
    out.extend(encode(f.kind, m.new));
    out.extend_from_slice(&bytes[f.offset + f.len..]);
    out
}

enum Outcome {
    Rejected,
    Ran,
    /// Loaded and ran, but its own save is rejected afterwards.
    Unloadable(String),
    Panicked(String),
}

thread_local! {
    /// The fuzz worker this thread serves (its own thread and the worker
    /// thread of the world it loaded).
    static SLOT: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

/// The last panic message and place per fuzz worker.
static PANICS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Panics on fuzz threads are recorded (message and place), not printed.
fn quiet_fuzz_panics() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let Some(k) = SLOT.with(|s| s.get()) else {
                return default(info);
            };
            let msg = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_default();
            let at = info.location().map(|l| l.to_string()).unwrap_or_default();
            let mut p = PANICS.lock().unwrap_or_else(|e| e.into_inner());
            if p.len() <= k {
                p.resize(k + 1, String::new());
            }
            p[k] = format!("{msg} at {at}");
        }));
    });
}

const RUN_TICKS: u64 = 300;

/// Loads `bytes` on fuzz worker `k`; a loaded world must run `RUN_TICKS`
/// ticks, and its save must load again.
fn try_bytes(k: usize, bytes: &[u8]) -> Outcome {
    SLOT.with(|s| s.set(Some(k)));
    let r = std::panic::catch_unwind(|| match World::load(bytes, 1) {
        Err(_) => Outcome::Rejected,
        Ok(mut w) => {
            w.install(|| SLOT.with(|s| s.set(Some(k))));
            for _ in 0..RUN_TICKS {
                w.step(&[]);
            }
            match World::load(&w.save(), 1) {
                Ok(_) => Outcome::Ran,
                Err(e) => Outcome::Unloadable(e.to_string()),
            }
        }
    });
    r.unwrap_or_else(|_| {
        let p = PANICS.lock().unwrap_or_else(|e| e.into_inner());
        Outcome::Panicked(p.get(k).cloned().unwrap_or_default())
    })
}

fn describe(m: &Mutation, l: &[save_layout::Layout], ticks: &[u64]) -> String {
    let f = &m.field;
    format!(
        "save@{} {} (offset {}, {:?}) {} -> {:?}",
        ticks[m.save], l[m.save].paths[f.path as usize], f.offset, f.kind, f.value, m.new
    )
}

/// Findings that are sim bugs, not load-validation gaps: reported by the
/// fuzz, not failed on. `(path prefix, message part)`.
///
/// 1. An AI `Move` of squad 20 (player 0) at 14400 to any point east of
///    x = 500 m (on the map: the same with x = 1000 m, a legal target) has
///    the unserved own gun 81 boarded by squad 19 while it unhitches:
///    `VehicleState::board` is set on a vehicle that is not abandoned
///    (bail.rs `try_board` for `own_gun_unserved`), which
///    `VehicleState::validate` rejects ("vehicle state is out of range"):
///    a save then does not load.
const KNOWN: [(&str, &str); 1] = [(
    "state.game.ai.[].pending.[].kind.CommandKind.Move.target",
    "vehicle state is out of range",
)];

/// Ticks of the mid-match saves the fuzz mutates.
const FUZZ_SAVES: [u64; 3] = [9_600, 14_400, 20_400];

/// The layouts of the `FUZZ_SAVES` of the seed match (one match per run of
/// this test binary).
fn fuzz_saves() -> &'static [save_layout::Layout] {
    static SAVES: std::sync::OnceLock<Vec<save_layout::Layout>> = std::sync::OnceLock::new();
    SAVES.get_or_init(|| {
        let mut w = World::new_match(setup(SEED), 1).unwrap();
        FUZZ_SAVES
            .iter()
            .map(|&t| {
                while w.tick() < t {
                    w.step(&[]);
                }
                layout(&w)
            })
            .collect()
    })
}

/// Part 2: every planned mutation of the three saves is rejected by
/// `World::load` or loads into a world that runs 300 ticks without a panic.
/// A hang (no progress for 60 s) fails with the mutation in flight.
#[test]
#[ignore = "thousands of loads and runs: runs in the release test pass"]
fn save_mutations_are_rejected_or_run() {
    quiet_fuzz_panics();
    let t0 = Instant::now();
    let saves = fuzz_saves();
    let t_match = t0.elapsed();
    let plans: Vec<Mutation> = (0..saves.len()).flat_map(|s| plan(s, &saves[s])).collect();
    let plans = std::sync::Arc::new(plans);
    let saves: &'static [save_layout::Layout] = saves;
    let workers = std::thread::available_parallelism().map_or(2, |n| n.get().clamp(2, 4));
    let next = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (tx, rx) = std::sync::mpsc::channel::<(usize, Outcome)>();
    let in_flight = std::sync::Arc::new(std::sync::Mutex::new(vec![usize::MAX; workers]));
    for k in 0..workers {
        let (plans, next, tx, in_flight) =
            (plans.clone(), next.clone(), tx.clone(), in_flight.clone());
        std::thread::Builder::new()
            .name(format!("save-fuzz-{k}"))
            .spawn(move || {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(m) = plans.get(i) else { break };
                    in_flight.lock().unwrap()[k] = i;
                    let out = try_bytes(k, &apply(&saves[m.save].bytes, m));
                    if tx.send((i, out)).is_err() {
                        break;
                    }
                }
            })
            .unwrap();
    }
    drop(tx);
    let mut outcomes: Vec<Option<Outcome>> = (0..plans.len()).map(|_| None).collect();
    loop {
        match rx.recv_timeout(std::time::Duration::from_secs(60)) {
            Ok((i, o)) => outcomes[i] = Some(o),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                let f = in_flight.lock().unwrap().clone();
                let hung: Vec<String> = f
                    .iter()
                    .filter(|&&i| i < plans.len() && outcomes[i].is_none())
                    .map(|&i| describe(&plans[i], saves, &FUZZ_SAVES))
                    .collect();
                panic!("no progress for 60 s; in flight: {hung:#?}");
            }
        }
    }
    let (mut rejected, mut ran) = (0, 0);
    let (mut panics, mut unloadable, mut known) = (Vec::new(), Vec::new(), Vec::new());
    for (i, o) in outcomes.into_iter().enumerate() {
        match o.expect("every mutation ran") {
            Outcome::Rejected => rejected += 1,
            Outcome::Ran => ran += 1,
            Outcome::Unloadable(msg) => {
                ran += 1;
                let path = &saves[plans[i].save].paths[plans[i].field.path as usize];
                let line = format!("{}: {msg}", describe(&plans[i], saves, &FUZZ_SAVES));
                if KNOWN
                    .iter()
                    .any(|(p, m)| path.starts_with(p) && msg.contains(m))
                {
                    known.push(line);
                } else {
                    unloadable.push(line);
                }
            }
            Outcome::Panicked(msg) => panics.push(format!(
                "{}: {msg}",
                describe(&plans[i], saves, &FUZZ_SAVES)
            )),
        }
    }
    println!(
        "save fuzz: {} mutations ({} saves, {} workers): {rejected} rejected, {ran} loaded and ran \
         {RUN_TICKS} ticks, {} of them then saved a state load rejects ({} known sim bugs), \
         {} panicked; saves ready after {t_match:?}, total {:?}",
        plans.len(),
        saves.len(),
        workers,
        unloadable.len() + known.len(),
        known.len(),
        panics.len(),
        t0.elapsed()
    );
    for k in &known {
        println!("known sim bug: {k}");
    }
    assert!(
        panics.is_empty() && unloadable.is_empty(),
        "{} mutations panicked:\n{}\n{} ran into a state load rejects:\n{}",
        panics.len(),
        panics.join("\n"),
        unloadable.len(),
        unloadable.join("\n")
    );
}

// ------------------------------------------------- regressions --------
// Each was a panic or a corrupt state the fuzz found (2026-11-11). The
// control in each: the unmutated save loads.

/// The first save (of `FUZZ_SAVES`) with a field at `path` for which
/// `pick(save index, occurrence, field)` holds, with `new` written there.
fn mutated(
    path: &str,
    pick: impl Fn(usize, usize, &save_layout::Field) -> bool,
    new: Value,
) -> Vec<u8> {
    for (si, l) in fuzz_saves().iter().enumerate() {
        let Some(p) = l.paths.iter().position(|x| x == path) else {
            continue;
        };
        let hit = l
            .fields
            .iter()
            .filter(|f| f.path as usize == p)
            .enumerate()
            .find(|(k, f)| pick(si, *k, f));
        if let Some((_, f)) = hit {
            assert!(World::load(&l.bytes, 1).is_ok(), "control: the save loads");
            return apply(
                &l.bytes,
                &Mutation {
                    save: si,
                    field: *f,
                    new,
                },
            );
        }
    }
    panic!("no save of the seed match has {path} (fixture drifted: pick another save tick)");
}

fn any(_: usize, _: usize, _: &save_layout::Field) -> bool {
    true
}

/// Loads `bytes`, which must be rejected (and not panic).
fn assert_rejected(bytes: &[u8], what: &str) {
    match std::panic::catch_unwind(|| World::load(bytes, 1)) {
        Ok(Err(_)) => {}
        Ok(Ok(_)) => panic!("{what}: the corrupt save loads"),
        Err(_) => panic!("{what}: World::load panics"),
    }
}

/// `MatchState::validate` took `i8::abs` of a dig job's `front`, which
/// panics on -128 (inside `World::load`).
#[test]
fn save_fuzz_dig_front_i8_min_is_rejected() {
    let b = mutated(
        "state.game.squads.{}.order.Order.Dig.job.front",
        any,
        Value::Int(-128),
    );
    assert_rejected(&b, "dig front -128");
}

/// A tick near `u64::MAX` overflowed `tick + 36_000` in the AI validator.
#[test]
fn save_fuzz_huge_tick_is_rejected() {
    let b = mutated("state.tick", any, Value::Int(u64::MAX as i128 - 1));
    assert_rejected(&b, "tick u64::MAX - 1");
}

/// Counters at the top of their range overflowed on the next `+= 1`:
/// stats, player, AI and fort-account counters.
#[test]
fn save_fuzz_counters_at_the_top_are_rejected() {
    for (path, v) in [
        ("state.stats.ground_hits", u64::MAX as i128),
        ("state.stats.commands_applied", u64::MAX as i128),
        ("state.game.players.[].point_ticks", u64::MAX as i128 - 1),
        ("state.game.players.[].kills", u32::MAX as i128),
        ("state.game.players.[].fm.flight_ticks", u64::MAX as i128),
        ("state.game.ai.[].counters.plans", u32::MAX as i128),
        (
            "state.game.ai.[].counters.infantry_plans",
            u32::MAX as i128 - 1,
        ),
        ("state.game.ai.[].issued", u32::MAX as i128),
        (
            "state.game.fort.accounts.[].wire_built",
            u32::MAX as i128 - 1,
        ),
    ] {
        assert_rejected(&mutated(path, any, Value::Int(v)), path);
    }
}

/// A fort work's progress beyond its step overflowed `work + add`.
#[test]
fn save_fuzz_fort_work_beyond_its_step_is_rejected() {
    let b = mutated(
        "state.game.fort.works.{}.work",
        any,
        Value::Int(u64::MAX as i128 - 1),
    );
    assert_rejected(&b, "work u64::MAX - 1");
}

/// An id counter one below `u32::MAX` loaded, reached the top at the next
/// fire mission, and the world's own save was then rejected (one more
/// mission would overflow).
#[test]
fn save_fuzz_id_counters_near_the_top_are_rejected() {
    for path in [
        "state.game.next_mission",
        "state.game.next_squad",
        "state.game.fort.next_wire",
    ] {
        assert_rejected(&mutated(path, any, Value::Int(u32::MAX as i128 - 1)), path);
    }
}

/// An AI command due far beyond the reaction delay loaded; the next one
/// queued before it unsorted the queue and the world's own save was
/// rejected.
#[test]
fn save_fuzz_ai_command_due_beyond_the_reaction_delay_is_rejected() {
    let l = &fuzz_saves()[1];
    let p = l
        .paths
        .iter()
        .position(|x| x == "state.game.ai.[].pending.[].due")
        .expect("pending AI commands at 14400");
    // The last field of the path is the last command of the last AI.
    let last = l.fields.iter().filter(|f| f.path as usize == p).count() - 1;
    let b = mutated(
        "state.game.ai.[].pending.[].due",
        |s, k, _| s == 1 && k == last,
        Value::Int(u64::MAX as i128 - 1),
    );
    assert_rejected(&b, "due u64::MAX - 1");
}

/// A served gun still emplacing with `relay_ticks` set loaded; once
/// emplaced the turn target no longer matched and the world's own save was
/// rejected.
#[test]
fn save_fuzz_relay_ticks_on_an_emplacing_gun_is_rejected() {
    use frontline_sim::vehicle::GunState;
    let states: Vec<Vec<GunState>> = (0..FUZZ_SAVES.len())
        .map(|si| {
            let w = World::load(&fuzz_saves()[si].bytes, 1).unwrap();
            let g = w.game().unwrap();
            g.vehicles()
                .values()
                .filter_map(|v| v.served.as_ref().map(|s| s.state))
                .collect()
        })
        .collect();
    let b = mutated(
        "state.game.vehicles.{}.served.relay_ticks",
        |s, k, _| states[s][k] == GunState::Emplacing,
        Value::Int(65_534),
    );
    assert_rejected(&b, "relay_ticks 65534 while emplacing");
}

/// A service slot swap above the seat swap bound loaded; mirrored into the
/// gun's seats it made the world's own save rejected.
#[test]
fn save_fuzz_slot_swap_out_of_range_is_rejected() {
    let b = mutated(
        "state.game.vehicles.{}.served.slots.[].swap",
        any,
        Value::Int(65_535),
    );
    assert_rejected(&b, "slot swap 65535");
}
