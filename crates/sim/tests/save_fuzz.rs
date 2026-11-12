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
    put(!f.blasts.is_empty(), "blasts");
    put(
        f.accounts.iter().any(|a| a.lost_on_known > 0),
        "lost_on_known",
    );
    // S10b-ai Build B chunk 2: an AI AT field job watching its work, with
    // mines of that field laid.
    put(
        (0..2u8).any(|p| {
            g.ai(PlayerId(p)).is_some_and(|ai| {
                ai.forts.iter().any(|(sq, j)| {
                    let d = &g.setup().forts[j.fort as usize];
                    !d.blocks_vehicles
                        && d.mine_item.is_some_and(|i| {
                            g.setup().items[i as usize]
                                .mine
                                .as_ref()
                                .is_some_and(|m| m.trigger == 1)
                        })
                        && j.phase == frontline_sim::ai::FortPhase::Watch
                        && j.work
                            .is_some_and(|wid| f.works.get(&wid).is_some_and(|k| k.builder == *sq))
                        && f.mines
                            .values()
                            .filter(|m| m.fort == j.fort && m.owner == PlayerId(p))
                            .count()
                            >= 3
                })
            })
        }),
        "ai_at_field_watch",
    );
    // S10b-ai Build C chunk 1: an AI log barrier job watching its work, and
    // an AI hedgehog job of a row with one hedgehog of it standing.
    let job_on = |pred: &dyn Fn(&frontline_content::setup::FortSetup) -> bool, watch: bool| {
        (0..2u8).any(|p| {
            g.ai(PlayerId(p)).is_some_and(|ai| {
                ai.forts.iter().any(|(sq, j)| {
                    let d = &g.setup().forts[j.fort as usize];
                    pred(d)
                        && (!watch
                            || (j.phase == frontline_sim::ai::FortPhase::Watch
                                && j.work.is_some_and(|wid| {
                                    f.works.get(&wid).is_some_and(|k| k.builder == *sq)
                                })))
                        && (watch
                            || f.pieces
                                .values()
                                .any(|q| q.fort == j.fort && q.owner == PlayerId(p)))
                })
            })
        })
    };
    use frontline_content::setup::FortShape;
    put(
        job_on(&|d| d.blocks_vehicles && d.shape == FortShape::Line, true),
        "ai_barrier_watch",
    );
    put(
        job_on(&|d| d.blocks_vehicles && d.shape == FortShape::Point, false),
        "ai_hedgehog_row",
    );
    // S10b-ai Build C chunk 3: an AI bunker job watching its work, the
    // work past its pit (dug).
    put(
        (0..2u8).any(|p| {
            g.ai(PlayerId(p)).is_some_and(|ai| {
                ai.forts.iter().any(|(sq, j)| {
                    g.setup().forts[j.fort as usize].pit
                        == Some(frontline_content::setup::DigProfile::BunkerPit)
                        && j.phase == frontline_sim::ai::FortPhase::Watch
                        && j.work.is_some_and(|wid| {
                            f.works.get(&wid).is_some_and(|k| {
                                k.builder == *sq && k.step != frontline_sim::fort::WorkStep::Dig
                            })
                        })
                })
            })
        }),
        "ai_bunker_watch",
    );
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
        version: 25,
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

/// Index of the staged S10b save in `fuzz_set` (after `FUZZ_SAVES`).
const STAGED: usize = FUZZ_SAVES.len();

/// The S10b state of a save: mines, charges, the S10b orders, the
/// blueprints and the mine items (S10b review R4).
fn s10b_path(path: &str) -> bool {
    path.starts_with("state.game.fort")
        || path.starts_with("state.game.setup.forts")
        || (path.starts_with("state.game.setup.items") && path.contains(".mine"))
        || path.contains("Order.ClearMines")
        || path.contains("Order.Demolish")
        // Fuzz stage `staged_s10b_ai`.
        || (path.starts_with("state.game.ai")
            && [".sweeps.", ".demolitions.", ".refused.", ".suspects_seen", ".forts."]
                .iter()
                .any(|k| path.contains(k)))
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
            // The staged S10b save: its own state, the blueprints' mine
            // items and the mine items' `[mine]` tables, at full weight.
            _ if save == STAGED && !s10b_path(path) => continue,
            _ if save == STAGED => (2, 3),
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
/// fuzz, not failed on. `(path prefix, message part)`. None open (G1, an
/// own gun boarded while it unhitches, was fixed in validation:
/// `save_fuzz_own_gun_boarded_while_unhitching_loads`).
const KNOWN: [(&str, &str); 0] = [];

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

/// The saves the mutation fuzz runs on: the `FUZZ_SAVES` of the seed match
/// and the staged S10b save (`STAGED`), with the tick of each.
fn fuzz_set() -> &'static (Vec<save_layout::Layout>, Vec<u64>) {
    static SET: std::sync::OnceLock<(Vec<save_layout::Layout>, Vec<u64>)> =
        std::sync::OnceLock::new();
    SET.get_or_init(|| {
        let w = staged_s10b();
        let mut saves = fuzz_saves().to_vec();
        saves.push(layout(&w));
        let mut ticks = FUZZ_SAVES.to_vec();
        ticks.push(w.tick());
        (saves, ticks)
    })
}

/// S10b review R4: the fuzz set reaches the S10b state (mine items, fort
/// and boom tick, a charge's piece, the ClearMines lift, the Demolish man,
/// the blueprints' mine items and the `[mine]` tables). Control: drop the
/// staged save from `fuzz_set` (the AI saves hold none of these).
#[test]
fn fuzz_plan_reaches_s10b_state() {
    let (saves, _) = fuzz_set();
    let planned: Vec<&str> = (0..saves.len())
        .flat_map(|s| {
            plan(s, &saves[s])
                .into_iter()
                .map(move |m| saves[s].paths[m.field.path as usize].as_str())
        })
        .collect();
    for want in [
        "state.game.fort.mines",
        "boom_tick",
        "state.game.fort.charges",
        "Order.ClearMines",
        "Order.Demolish",
        "state.game.setup.forts",
    ] {
        assert!(
            planned.iter().any(|p| p.contains(want)),
            "no planned mutation reaches {want}"
        );
    }
    assert!(
        planned
            .iter()
            .any(|p| p.contains(".piece") && p.starts_with("state.game.fort.charges"))
    );
    assert!(
        planned.iter().any(|p| p.starts_with("state.game.ai")
            && p.contains(".forts")
            && p.ends_with(".start")),
        "no planned mutation reaches AiFortJob::start"
    );
    // Fuzz stage `staged_s10b_ai`: entries, not the empty lists' lengths
    // (nor the counters `sweeps_started`, `refused_reorders`, ...).
    for (from, want) in [
        ("state.game.ai", ".sweeps.{}"),
        ("state.game.ai", ".demolitions.{}"),
        ("state.game.ai", ".refused.[]"),
        ("state.game.fort", ".blasts.[]"),
    ] {
        assert!(
            planned
                .iter()
                .any(|p| p.starts_with(from) && p.contains(want)),
            "no planned mutation reaches an entry of {from} {want}"
        );
    }
    assert!(
        planned
            .iter()
            .any(|p| p.contains("Order.ClearMines") && p.contains("lift"))
    );
    assert!(
        planned
            .iter()
            .any(|p| p.starts_with("state.game.setup.forts") && p.ends_with("mine_item"))
    );
    assert!(
        planned
            .iter()
            .any(|p| p.starts_with("state.game.setup.items") && p.contains(".mine"))
    );
}

/// A staged S10b state (spec R4 of the S10b review): an engineer squad of
/// player 0 in `ClearMines` with a known enemy AP mine in its lane, another
/// in `Demolish` against an enemy sandbag wall, a satchel charge laid on
/// that wall, an AT mine and an AP mine set off for the next tick, and
/// mines of both kinds. AI on both sides (the world runs on after a load).
fn staged_s10b() -> World {
    use frontline_content::ItemKind;
    use frontline_content::setup::Capability;
    use frontline_sim::fort::Charge;
    use frontline_sim::fx::{Fx, FxVec2};
    use frontline_sim::game::Order;

    let mut s = setup(SEED);
    for t in s.squad_types.iter_mut() {
        if t.id.contains("engineer") {
            t.caps |= Capability::PlaceMines.bit()
                | Capability::ClearMines.bit()
                | Capability::Demolish.bit();
        }
    }
    let c = |x: i32, y: i32| FxVec2::from_int(x, y);
    let fort = |id: &str| s.forts.iter().position(|f| f.id == id).unwrap() as u16;
    let (wall, ap, at) = (
        fort("ww2:sandbag_wall"),
        fort("ww2:ap_minefield"),
        fort("ww2:at_minefield"),
    );
    let satchel = s
        .items
        .iter()
        .position(|i| i.kind == ItemKind::Satchel)
        .unwrap() as u16;
    let eng = s.squad_type_index("ww2:ger_engineer_squad").unwrap();
    let mut w = World::new_match(s, 1).unwrap();
    let spawn = |w: &mut World, x: i32| {
        w.spawn_squad_at(PlayerId(0), eng, c(x, 300), FxVec2::new(Fx::ONE, Fx::ZERO))
            .unwrap()
    };
    let (sweep, demo) = (spawn(&mut w, 280), spawn(&mut w, 380));
    assert!(w.build_now(wall, &[c(400, 400), c(410, 400)], PlayerId(1)));
    let piece = *w.game().unwrap().fort().pieces.keys().next().unwrap();
    let lane_mine = w.lay_mine_now(ap, c(300, 300), PlayerId(1)).unwrap();
    let mut ids = vec![lane_mine];
    for (k, f) in [ap, at, ap, at].into_iter().enumerate() {
        ids.push(
            w.lay_mine_now(f, c(600 + 20 * k as i32, 600), PlayerId(k as u8 % 2))
                .unwrap(),
        );
    }
    let tick = w.tick();
    w.edit_fort(|f| {
        f.mines.get_mut(&lane_mine).unwrap().known = 0b11;
        for id in &ids[1..3] {
            f.mines.get_mut(id).unwrap().boom_tick = Some(tick + 1);
        }
        f.charges.push(Charge {
            id: f.next_charge,
            owner: PlayerId(0),
            item: satchel,
            from: c(399, 400),
            to: c(400, 400),
            fuse_tick: tick + 90,
            piece: Some(piece),
        });
        f.next_charge += 1;
    });
    w.edit_squad(sweep, |q| {
        q.order = Order::ClearMines {
            a: c(280, 300),
            b: c(320, 300),
            along: Fx::from_int(12),
            lift: Some(lane_mine),
            work: 100,
        }
    });
    w.edit_squad(demo, |q| {
        q.order = Order::Demolish {
            piece,
            man: None,
            work: 200,
        }
    });
    staged_s10b_ai(&mut w, ap, piece);
    w
}

/// Fuzz stage `staged_s10b_ai` (S10b-ai Build A): player 0's AI holds one
/// sweep (the `ClearMines` squad), one demolition with one piece done for
/// an own Panzer IV, two refusals, a fort job for the AP minefield with its
/// start, and the fort state one mine blast.
fn staged_s10b_ai(w: &mut World, ap: u16, piece: u32) {
    use frontline_sim::ai::{
        AiDemolish, AiFortJob, AiSweep, DemolishPhase, FortPhase, JobKind, SweepPhase,
    };
    use frontline_sim::fort::MineBlast;
    use frontline_sim::fx::{Fx, FxVec2};
    use frontline_sim::game::SquadId;

    let c = |x: i32, y: i32| FxVec2::from_int(x, y);
    let s = w.match_setup().unwrap();
    let eng = s.squad_type_index("ww2:ger_engineer_squad").unwrap();
    let tank = s.squad_type_index("ww2:ger_pz4_h_squad").unwrap();
    let east = FxVec2::new(Fx::ONE, Fx::ZERO);
    let own = |w: &World, o: &dyn Fn(&frontline_sim::game::Order) -> bool| -> SquadId {
        let g = w.game().unwrap();
        *g.squads()
            .iter()
            .find(|(_, q)| q.owner == PlayerId(0) && o(&q.order))
            .unwrap()
            .0
    };
    use frontline_sim::game::Order;
    let sweep = own(w, &|o| matches!(o, Order::ClearMines { .. }));
    let demo = own(w, &|o| matches!(o, Order::Demolish { .. }));
    let builder = w
        .spawn_squad_at(PlayerId(0), eng, c(480, 300), east)
        .unwrap();
    let veh_sq = w
        .spawn_squad_at(PlayerId(0), tank, c(380, 330), east)
        .unwrap();
    let vehicle = w.game().unwrap().squads()[&veh_sq].members[0];
    let tick = w.tick();
    assert!(w.with_ai_state(0, |ai| {
        ai.sweeps.insert(
            sweep,
            AiSweep {
                point: 0,
                a: c(280, 300),
                b: c(320, 300),
                start: tick,
                since: tick,
                phase: SweepPhase::Sweep,
                tries: 1,
            },
        );
        ai.demolitions.insert(
            demo,
            AiDemolish {
                vehicle,
                pieces: [Some(piece), None, None],
                done: 1,
                start: tick,
                since: tick,
                phase: DemolishPhase::Charge,
            },
        );
        ai.refuse(JobKind::Fort, c(500, 300), tick);
        ai.refuse(JobKind::TruckGoal, c(200, 200), tick);
        ai.forts.insert(
            builder,
            AiFortJob {
                fort: ap,
                point: 0,
                line: vec![c(500, 310), c(530, 310)],
                phase: FortPhase::Go,
                start: tick,
                since: tick,
                work: None,
                tries: 0,
            },
        );
        ai.suspects_seen = 1;
    }));
    // S10b-ai Build B: a player-1 AP mine on a field player 0 knew, set off
    // by a player-0 man this tick (as `take_mines` leaves it: the mine gone,
    // its blast remembered, the loss counted as on a known field).
    w.edit_fort(|f| {
        f.blasts.push(MineBlast {
            pos: c(600, 600),
            owner: PlayerId(1),
            tick,
        });
        f.accounts[1].mines_set_off += 1;
        f.accounts[0].lost_on_known += 1;
    });
    // S10b-ai Build B chunk 2: an AT field job in Watch, its work on the
    // fourth segment of a 40 m line and the first three mines laid (as
    // `fort::order_fortify` and the build steps leave them).
    let at = s_fort(w, "ww2:at_minefield");
    let layer = w
        .spawn_squad_at(PlayerId(0), eng, c(560, 400), east)
        .unwrap();
    let line = vec![c(560, 410), c(600, 410)];
    for k in 0..3 {
        w.lay_mine_now(at, c(562 + 4 * k, 410), PlayerId(0))
            .unwrap();
    }
    let mut wid = 0;
    w.edit_fort(|f| {
        wid = f.next_work;
        f.next_work += 1;
        f.works.insert(
            wid,
            frontline_sim::fort::Work {
                owner: PlayerId(0),
                fort: at,
                line: line.clone(),
                yaw: Fx::ZERO,
                builder: layer,
                hq: None,
                step: frontline_sim::fort::WorkStep::Build,
                seg: 3,
                work: 0,
                got_g: 0,
            },
        );
        f.accounts[0].mines_laid += 3;
    });
    w.edit_squad(layer, |q| q.order = Order::Fortify { work: wid });
    assert!(w.with_ai_state(0, |ai| {
        ai.forts.insert(
            layer,
            AiFortJob {
                fort: at,
                point: 0,
                line: vec![c(572, 410), c(600, 410)],
                phase: FortPhase::Watch,
                start: tick,
                since: tick,
                work: Some(wid),
                tries: frontline_sim::fort::WorkStep::Build as u8,
            },
        );
    }));
    // S10b-ai Build C chunk 1: a log barrier job in Watch (its work on the
    // second segment, the first piece standing), and a hedgehog row: one
    // hedgehog standing, the job on the row's second site.
    let (bar, hh) = (s_fort(w, "ww2:log_barrier"), s_fort(w, "ww2:hedgehog"));
    let pio = w
        .match_setup()
        .unwrap()
        .squad_type_index("ww2:ger_baupioniergruppe")
        .unwrap_or(eng);
    let (bb, hb) = (
        w.spawn_squad_at(PlayerId(0), eng, c(560, 470), east)
            .unwrap(),
        w.spawn_squad_at(PlayerId(0), pio, c(620, 470), east)
            .unwrap(),
    );
    assert!(w.build_now(bar, &[c(560, 480), c(564, 480)], PlayerId(0)));
    assert!(w.build_now(hh, &[c(620, 480), c(620, 490)], PlayerId(0)));
    let bline = vec![c(560, 480), c(580, 480)];
    let mut bw = 0;
    w.edit_fort(|f| {
        bw = f.next_work;
        f.next_work += 1;
        f.works.insert(
            bw,
            frontline_sim::fort::Work {
                owner: PlayerId(0),
                fort: bar,
                line: bline.clone(),
                yaw: Fx::ZERO,
                builder: bb,
                hq: None,
                step: frontline_sim::fort::WorkStep::Build,
                seg: 1,
                work: 0,
                got_g: 0,
            },
        );
    });
    w.edit_squad(bb, |q| q.order = Order::Fortify { work: bw });
    assert!(w.with_ai_state(0, |ai| {
        ai.forts.insert(
            bb,
            AiFortJob {
                fort: bar,
                point: frontline_sim::ai::NO_POINT,
                line: vec![c(564, 480), c(580, 480)],
                phase: FortPhase::Watch,
                start: tick,
                since: tick,
                work: Some(bw),
                tries: frontline_sim::fort::WorkStep::Build as u8,
            },
        );
        ai.forts.insert(
            hb,
            AiFortJob {
                fort: hh,
                point: 0,
                line: vec![c(624, 480), c(624, 490)],
                phase: FortPhase::Load,
                start: tick,
                since: tick,
                work: None,
                tries: 0,
            },
        );
    }));
    // S10b-ai Build C chunk 3: a bunker job in Watch, its pit dug and its
    // work waiting for the rest of its 250 kg (160 drawn).
    let bk = s_fort(w, "ww2:bunker");
    let bsite = c(700, 520);
    assert!(w.dig_pit_now(
        &[bsite - c(1, 0), bsite + c(1, 0)],
        frontline_content::setup::DigProfile::BunkerPit
    ));
    let bp = w
        .spawn_squad_at(PlayerId(0), pio, c(690, 520), east)
        .unwrap();
    let bkline = vec![bsite, bsite + c(10, 0)];
    let mut kw = 0;
    w.edit_fort(|f| {
        kw = f.next_work;
        f.next_work += 1;
        f.works.insert(
            kw,
            frontline_sim::fort::Work {
                owner: PlayerId(0),
                fort: bk,
                line: bkline.clone(),
                yaw: Fx::ZERO,
                builder: bp,
                hq: None,
                step: frontline_sim::fort::WorkStep::Wait,
                seg: 0,
                work: 0,
                got_g: 160_000,
            },
        );
    });
    w.edit_squad(bp, |q| q.order = Order::Fortify { work: kw });
    assert!(w.with_ai_state(0, |ai| {
        ai.forts.insert(
            bp,
            AiFortJob {
                fort: bk,
                point: 0,
                line: bkline.clone(),
                phase: FortPhase::Watch,
                start: tick,
                since: tick,
                work: Some(kw),
                tries: frontline_sim::fort::WorkStep::Wait as u8,
            },
        );
    }));
}

/// Blueprint `id`'s index.
fn s_fort(w: &World, id: &str) -> u16 {
    w.match_setup()
        .unwrap()
        .forts
        .iter()
        .position(|f| f.id == id)
        .unwrap() as u16
}

/// The staged S10b save holds what the fuzz should reach there, and loads.
/// Control: drop a part of `staged_s10b` (say the AT mines) and this fails.
#[test]
fn staged_s10b_save_holds_its_state() {
    use frontline_sim::game::Order;
    let w = staged_s10b();
    World::load(&w.save(), 1).unwrap_or_else(|e| panic!("the staged S10b save: {e}"));
    let g = w.game().unwrap();
    let f = g.fort();
    let item_of = |id: &str| {
        let s = g.setup();
        let fi = s.forts.iter().position(|f| f.id == id).unwrap();
        s.forts[fi].mine_item.unwrap()
    };
    let (ap, at) = (item_of("ww2:ap_minefield"), item_of("ww2:at_minefield"));
    assert!(f.mines.values().any(|m| m.item == ap) && f.mines.values().any(|m| m.item == at));
    assert!(f.mines.values().any(|m| m.boom_tick.is_some()));
    assert!(f.charges.iter().any(|c| c.piece.is_some()));
    let orders: Vec<&Order> = g.squads().values().map(|q| &q.order).collect();
    assert!(
        orders
            .iter()
            .any(|o| matches!(o, Order::ClearMines { lift: Some(_), .. }))
    );
    assert!(orders.iter().any(|o| matches!(o, Order::Demolish { .. })));
    // Fuzz stage `staged_s10b_ai`.
    let ai = g.ai(frontline_sim::PlayerId(0)).unwrap();
    assert!(!ai.sweeps.is_empty() && !ai.demolitions.is_empty());
    assert!(ai.demolitions.values().any(|d| d.done == 1));
    assert_eq!(ai.refused.len(), 2);
    let ap_fort = g
        .setup()
        .forts
        .iter()
        .position(|f| f.id == "ww2:ap_minefield")
        .unwrap() as u16;
    assert!(ai.forts.values().any(|j| j.fort == ap_fort));
    let h = holds(&w);
    let miss: Vec<_> = STAGED_MUST_HOLD.iter().filter(|m| !h.contains(m)).collect();
    assert!(miss.is_empty(), "the staged save lacks {miss:?}");
}

/// What the staged S10b save must hold beyond the S10b orders (fuzz stage
/// `staged_s10b_ai + blasts and lost_on_known set`, S10b-ai Build B chunk
/// 1; `+ an AtField job in Watch with its work and 3 laid mines`, chunk 2).
/// `+ a LogBarrier job in Watch and one standing hedgehog of a row`, Build
/// C chunk 1. `+ a Bunker job in Watch with its pit dug`, Build C chunk 3.
const STAGED_MUST_HOLD: [&str; 6] = [
    "blasts",
    "lost_on_known",
    "ai_at_field_watch",
    "ai_barrier_watch",
    "ai_hedgehog_row",
    "ai_bunker_watch",
];

/// S10b review R1: `FortSetup::mine_item` of a saved setup was never
/// checked, so a mine whose blueprint names item 9999 loaded and the first
/// step panicked in `apply_blows`. Control: drop the `forts` check in
/// `MatchSetup::validate` and the second guard in `validate_s10b`.
#[test]
fn save_mine_item_out_of_range_is_rejected() {
    let w = staged_s10b();
    let frag = w
        .game()
        .unwrap()
        .setup()
        .items
        .iter()
        .position(|i| i.kind == frontline_content::ItemKind::Frag)
        .unwrap() as i128;
    let l = layout(&w);
    // Every blueprint's mine_item and every mine's item, set together (so
    // the old `mine_item == item` check still holds), applied from the
    // back so earlier offsets stay valid.
    let mut fs: Vec<save_layout::Field> = l
        .fields
        .iter()
        .filter(|f| {
            let p = &l.paths[f.path as usize];
            !matches!(f.kind, save_layout::Kind::Tag)
                && ((p.starts_with("state.game.setup.forts") && p.ends_with("mine_item"))
                    || (p.starts_with("state.game.fort.mines") && p.ends_with(".item")))
        })
        .copied()
        .collect();
    assert!(fs.len() >= 6, "two blueprints and five mines: {}", fs.len());
    fs.sort_by_key(|f| std::cmp::Reverse(f.offset));
    for v in [9999, frag] {
        let mut b = l.bytes.clone();
        for f in &fs {
            b = apply(
                &b,
                &Mutation {
                    save: 0,
                    field: *f,
                    new: Value::Int(v),
                },
            );
        }
        assert_rejected(&b, &format!("mine_item and mine items {v}"));
    }
}

/// Part 2: every planned mutation of the fuzz set (three AI saves and the
/// staged S10b save) is rejected by
/// `World::load` or loads into a world that runs 300 ticks without a panic.
/// A hang (no progress for 60 s) fails with the mutation in flight.
#[test]
#[ignore = "thousands of loads and runs: runs in the release test pass"]
fn save_mutations_are_rejected_or_run() {
    quiet_fuzz_panics();
    let t0 = Instant::now();
    let (saves, ticks) = fuzz_set();
    let t_match = t0.elapsed();
    let plans: Vec<Mutation> = (0..saves.len()).flat_map(|s| plan(s, &saves[s])).collect();
    let plans = std::sync::Arc::new(plans);
    let saves: &'static [save_layout::Layout] = saves;
    let ticks: &'static [u64] = ticks;
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
                    .map(|&i| describe(&plans[i], saves, ticks))
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
                let line = format!("{}: {msg}", describe(&plans[i], saves, ticks));
                if KNOWN
                    .iter()
                    .any(|(p, m)| path.starts_with(p) && msg.contains(m))
                {
                    known.push(line);
                } else {
                    unloadable.push(line);
                }
            }
            Outcome::Panicked(msg) => {
                panics.push(format!("{}: {msg}", describe(&plans[i], saves, ticks)))
            }
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

/// S10b: one bad state per line of docs/slices/S10b-obstacles.md 6 (mines
/// x9, `Charge::piece` x2, `Order::ClearMines` x7, `Order::Demolish` x4) must
/// be rejected; the good base state loads. The engineer card gets bits
/// 12-14 here (no card has them before Builds B-D). Control: comment out
/// any one check in `FortState::validate` / `validate_s10b` -> its case
/// loads (seen with the `boom_tick` check).
#[test]
fn load_rejects_bad_s10b_state() {
    use frontline_content::ItemKind;
    use frontline_content::setup::Capability;
    use frontline_sim::fort::{Charge, FortState, Mine};
    use frontline_sim::fx::{Fx, FxVec2};
    use frontline_sim::game::{Order, SquadId};

    let mut s = setup(SEED);
    for t in s.squad_types.iter_mut() {
        if t.id.contains("engineer") {
            t.caps |= Capability::PlaceMines.bit()
                | Capability::ClearMines.bit()
                | Capability::Demolish.bit();
        }
    }
    let c = |x: i32, y: i32| FxVec2::from_int(x, y);
    let fort = |id: &str| s.forts.iter().position(|f| f.id == id).unwrap() as u16;
    let item = |k: &dyn Fn(&frontline_content::setup::ItemSetup) -> bool| {
        s.items.iter().position(k).unwrap() as u16
    };
    let (wall, field) = (fort("ww2:sandbag_wall"), fort("ww2:ap_minefield"));
    let satchel = item(&|i| i.kind == ItemKind::Satchel);
    let bangalore = item(&|i| i.bangalore.is_some());
    let other_item = item(&|i| i.kind == ItemKind::Satchel);
    let limit = s.forts[field as usize].limit_per_player;
    let ft = s.rules.fortify.clone().unwrap();
    let (hz, size) = (s.tick_hz as u64, s.map.size_m);
    let eng = s.squad_type_index("ww2:ger_engineer_squad").unwrap();
    let rifles = s.squad_type_index("ww2:ger_grenadiergruppe").unwrap();

    let mut w = World::new_match(s, 1).unwrap();
    let spawn = |w: &mut World, t: u16, x: i32| {
        w.spawn_squad_at(PlayerId(0), t, c(x, 300), FxVec2::new(Fx::ONE, Fx::ZERO))
            .unwrap()
    };
    let (sweep, demo, rifle) = (
        spawn(&mut w, eng, 280),
        spawn(&mut w, eng, 380),
        spawn(&mut w, rifles, 480),
    );
    assert!(w.build_now(wall, &[c(400, 400), c(410, 400)], PlayerId(1)));
    let piece = *w.game().unwrap().fort().pieces.keys().next().unwrap();
    let mine = w.lay_mine_now(field, c(300, 300), PlayerId(1)).unwrap();
    let tick = w.tick();
    w.edit_fort(|f| {
        f.mines.get_mut(&mine).unwrap().known = 0b11;
        f.charges.push(Charge {
            id: f.next_charge,
            owner: PlayerId(0),
            item: satchel,
            from: c(399, 400),
            to: c(400, 400),
            fuse_tick: tick + 90,
            piece: Some(piece),
        });
        f.next_charge += 1;
    });
    let clear = Order::ClearMines {
        a: c(280, 300),
        b: c(320, 300),
        along: Fx::from_int(12),
        lift: Some(mine),
        work: 100,
    };
    let dem = Order::Demolish {
        piece,
        man: None,
        work: 200,
    };
    w.edit_squad(sweep, |q| q.order = clear.clone());
    w.edit_squad(demo, |q| q.order = dem.clone());
    let base = w.save();
    World::load(&base, 1).unwrap_or_else(|e| panic!("the good S10b state: {e}"));
    let rifle_man = w.game().unwrap().squads()[&rifle].members[0];

    type Edit = Box<dyn Fn(&mut World)>;
    let fort_edit =
        |f: Box<dyn Fn(&mut FortState)>| -> Edit { Box::new(move |w| _ = w.edit_fort(&*f)) };
    let mine_edit = move |f: Box<dyn Fn(&mut Mine)>| -> Edit {
        Box::new(move |w| _ = w.edit_fort(|s| f(s.mines.get_mut(&mine).unwrap())))
    };
    let order = move |q: SquadId, o: Order| -> Edit {
        Box::new(move |w| _ = w.edit_squad(q, |s| s.order = o.clone()))
    };
    let lift_total = ft.lift_s as u64 * hz * 100;
    let place_total = ft.demolish_place_s as u64 * hz * 100;
    let lane = ft.lane_max_m as i32;
    let off = c(size + 1, 300);
    let many = |n: u32, owner: u8| -> Edit {
        Box::new(move |w| {
            _ = w.edit_fort(|f| {
                let m = f.mines[&mine].clone();
                for k in 0..n {
                    let id = f.next_mine;
                    f.next_mine += 1;
                    let owner = frontline_sim::PlayerId(if k % 2 == 0 { owner } else { 0 });
                    f.mines.insert(
                        id,
                        Mine {
                            owner,
                            pos: c(10 + (k % 1000) as i32, 10),
                            ..m.clone()
                        },
                    );
                }
            })
        })
    };
    let cases: Vec<(&str, Edit)> = vec![
        (
            "mines above MAX_MINES",
            many(frontline_sim::fort::MAX_MINES as u32, 1),
        ),
        (
            "mine id >= next_mine",
            fort_edit(Box::new(move |f| f.next_mine = mine)),
        ),
        (
            "mine owner not a player",
            mine_edit(Box::new(|m| m.owner = PlayerId(2))),
        ),
        (
            "mine fort without mine_item",
            mine_edit(Box::new(move |m| m.fort = wall)),
        ),
        (
            "mine item not its mine_item",
            mine_edit(Box::new(move |m| m.item = other_item)),
        ),
        (
            "mine off the map",
            mine_edit(Box::new(move |m| m.pos = off)),
        ),
        (
            "mine known bit of no player",
            mine_edit(Box::new(|m| m.known = 0b111)),
        ),
        (
            "mine boom_tick not tick + 1",
            mine_edit(Box::new(move |m| m.boom_tick = Some(tick + 2))),
        ),
        ("mines above limit_per_player", many(2 * limit, 1)),
        (
            "charge piece >= next_piece",
            fort_edit(Box::new(move |f| f.charges[0].piece = Some(f.next_piece))),
        ),
        (
            "charge piece on a Bangalore",
            fort_edit(Box::new(move |f| f.charges[0].item = bangalore)),
        ),
        (
            "ClearMines without clear_mines",
            order(rifle, clear.clone()),
        ),
        (
            "ClearMines a off the map",
            order(
                sweep,
                Order::ClearMines {
                    a: off,
                    b: c(size - 10, 300),
                    along: Fx::ZERO,
                    lift: None,
                    work: 0,
                },
            ),
        ),
        (
            "ClearMines b off the map",
            order(
                sweep,
                Order::ClearMines {
                    a: c(size - 10, 300),
                    b: off,
                    along: Fx::ZERO,
                    lift: None,
                    work: 0,
                },
            ),
        ),
        (
            "ClearMines lane above lane_max_m",
            order(
                sweep,
                Order::ClearMines {
                    a: c(100, 300),
                    b: c(101 + lane, 300),
                    along: Fx::ZERO,
                    lift: None,
                    work: 0,
                },
            ),
        ),
        (
            "ClearMines along beyond the lane",
            order(
                sweep,
                Order::ClearMines {
                    a: c(280, 300),
                    b: c(320, 300),
                    along: Fx::from_int(41),
                    lift: None,
                    work: 0,
                },
            ),
        ),
        (
            "ClearMines lifts an unknown mine",
            Box::new(move |w: &mut World| {
                _ = w.edit_fort(|f| f.mines.get_mut(&mine).unwrap().known = 0b10);
            }),
        ),
        (
            "ClearMines work beyond the lift total",
            order(
                sweep,
                Order::ClearMines {
                    a: c(280, 300),
                    b: c(320, 300),
                    along: Fx::ZERO,
                    lift: Some(mine),
                    work: lift_total + 1,
                },
            ),
        ),
        ("Demolish without demolish", order(rifle, dem.clone())),
        (
            "Demolish piece >= next_piece",
            order(
                demo,
                Order::Demolish {
                    piece: piece + 1000,
                    man: None,
                    work: 0,
                },
            ),
        ),
        (
            "Demolish man not a member",
            order(
                demo,
                Order::Demolish {
                    piece,
                    man: Some(rifle_man),
                    work: 0,
                },
            ),
        ),
        (
            "Demolish work beyond the place total",
            order(
                demo,
                Order::Demolish {
                    piece,
                    man: None,
                    work: place_total + 1,
                },
            ),
        ),
    ];
    // The check each case must hit (its error text).
    let why = [
        "too many",
        "mine id",
        "mine id",
        "mine_item",
        "mine_item",
        "position",
        "not in the match",
        "tick other",
        "limit_per_player",
        "demolition charge",
        "demolition charge",
        "without clear_mines",
        "off the map",
        "off the map",
        "lane_max_m",
        "outside its lane",
        "does not know",
        "lift total",
        "without demolish",
        "piece id",
        "living member",
        "place total",
    ];
    assert_eq!(cases.len(), why.len());
    for ((what, edit), why) in cases.iter().zip(why) {
        let mut b = World::load(&base, 1).unwrap();
        edit(&mut b);
        let bytes = b.save();
        assert_rejected(&bytes, what);
        let e = World::load(&bytes, 1).err().unwrap().to_string();
        assert!(e.contains(why), "{what}: rejected by another check: {e}");
    }
}

/// G1 (the fuzz finding of 2026-11-11, was in `KNOWN`): an AI `Move` east
/// of x = 500 m in the 14400 save has an unserved own gun boarded while it
/// unhitches (`board` set, `abandoned` None, as bail.rs `try_board` does
/// for `own_gun_unserved`); the world's own save must load. Control: drop
/// the served-gun exception for `board` in `VehicleState::validate`.
#[test]
fn save_fuzz_own_gun_boarded_while_unhitching_loads() {
    let k = FUZZ_SAVES.iter().position(|&t| t == 14_400).unwrap();
    let l = &fuzz_saves()[k];
    let path = "state.game.ai.[].pending.[].kind.CommandKind.Move.target.x";
    let p = l
        .paths
        .iter()
        .position(|x| x == path)
        .expect("a pending Move");
    for f in l.fields.iter().filter(|f| f.path as usize == p) {
        let b = apply(
            &l.bytes,
            &Mutation {
                save: k,
                field: *f,
                new: Value::Int(1000 << 16),
            },
        );
        let mut w = World::load(&b, 1).unwrap();
        for _ in 0..RUN_TICKS {
            w.step(&[]);
        }
        if let Err(e) = World::load(&w.save(), 1) {
            panic!("pending Move at offset {} to x 1000: {e}", f.offset);
        }
    }
}
