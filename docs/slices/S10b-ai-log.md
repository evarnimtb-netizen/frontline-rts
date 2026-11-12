# S10b-ai log (the AI uses mines, obstacles, the bunker; at most 150 lines)

Spec: common `docs/slices/S10b-ai.md`, stages `S10b-ai-build-A.md` ... `-D.md`; targets `targets/S10b-ai.toml`.
Branch `s10b-ai` from main `72b2970`. Another agent may work in this checkout: commit only your files
(`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)

**Next: Build A chunk 2** (`S10b-ai-build-A.md` steps 8-12, then STOP POINT 2).
Done (A1, layout commit `0ae000f`): SAVE 25 (the one bump), PROTOCOL 14. `tools/check.sh quick 72b2970` all ok.
Content: `AiObstacleTuning` `[ai.obstacles]` (30 keys, `tuning.rs`, `TABLES`, `TuningRefs::ai_obstacles`,
`AiDef/AiSetup::obstacles`); `FortTuning` += `mine_paths`, `mine_path_cost_pct`, `mine_band_foot_cm`,
`mine_band_vehicle_cm` (serde defaults 1/2000/300/400); `AiTruckTuning::refusal_memory` (default 1). Switches all 1.
Sim: `crates/sim/src/ai/obstacles.rs` (new): `AiSweep`, `AiDemolish`, `AiRefusal::key`, `JobKind`, `SweepPhase`
(Go/Sweep/Done), `DemolishPhase` (Go/Charge/Repath/Done), `MAX_SWEEPS`, `MAX_DEMOLITIONS`, `MAX_REFUSED`,
`REFUSAL_KEEP_MIN_S`, `AiState::{refuse, refused_near, drop_stale_refusals, validate_s10b}`, `AiHabit`, `DriveArgs
{squad, point, others, at}`, `HabitEnd {Done, Failed, NotBuilt}`, `Planner::drive_habit` (each arm NotBuilt),
`MatchState::drive_habit_plan`. `ai.rs`: `AiState::{sweeps, demolitions, refused, suspects_seen}`, 14 `AiCounters`
(`s10b_jobs` ... `refused_reorders`). `ai/fort.rs`: `AiFortJob::start`, `WorkKind::{ApField, AtField, Hedgehog,
LogBarrier, Bunker}` (fort_of by trigger / blocks_vehicles / BunkerPit; `switched_on` reads `[ai.obstacles]`).
`fort.rs`: `MineBlast`, `FortState::blasts` (sorted by tick; in `is_empty`), `MAX_BLASTS`, `BLAST_KEEP_S`, 4
`FortAccount` fields. `world.rs`: `World::{with_ai_state, drive_ai_habit}`; tests `crates/sim/src/s10b_ai_tests.rs`.
Tests (each control seen failing): `s10b_ai_layout_roundtrips_and_hashes`, `load_rejects_bad_s10b_ai_state`,
`refusal_memory_replaces_oldest_failure`, `fort_of_returns_s10b_blueprints_for_new_kinds`,
`drive_ai_habit_skeleton_ends_not_built`, content `ai_obstacle_tuning_parses_and_validates`
(`sdk/crates/content/tests/s10b_ai_content.rs`). Fuzz stage `staged_s10b_ai` in `save_fuzz.rs` (sweep, demolition
with one piece done, two refusals, an ApField fort job with start, a blast): fuzz 0 panics, 0 unloadable.
Semantics chunk 2+ must keep: `refused_near(kind, at, tick, refused_ticks, near_m)` takes TICKS (not s);
`drop_stale_refusals(tick, keep_ticks)` gets max(refused_s, 300 s) in ticks; `suspects_seen` is checked only
`<= MAX_BLASTS` and must be clamped to `blasts.len()` on read; nothing in a match calls the new code yet.
Chunk 2 starts at step 8: `sdk/crates/cli/src/skirmish.rs` (`S10_AI`, `S10Player::ai_counters`), `json_out.rs`.
Failing tests: none. Neutrality 40-set not run in chunk 1 (no behaviour; chunk 2 ends with it).
Neutrality bases: 40 set `target/s10b-base40`; full `target/s10b-verify`. `targets/S10b-ai.toml` parses only
after chunk 2 adds `three_parts_from`, `exposure`, `exposure_min`, `control` (until then `S10b.toml`).
Chunks left: A 1, B 2, C 2, D 3. Owner questions S10b-ai-1..7 (spec 10).

## Plan (2026-11-12)

Spec (scope, switches, data model, rules R1-R14 and L1-L5, job table, save rules, measures, done-when), four stage
files, targets (S10b body; T106b report; T108 on the all-off control; new T109-T118 in three-part form).
Facts used: own mines never hurt or trigger own units (S10b R7), so mines need no own-route rule and own-mine path
avoidance is pointless under S10b-1 (A); obstacles block every vehicle, so they get the own-route rule (R6);
`build_id` and `content_hash` already fence net peers, so the path rule needs no PROTOCOL bump; the AI's fort job
(Load, Go, Order, Watch) already carries the S10a liveness pieces (F13 stall, `REFUSED_S` parking), so the new
works reuse it as new `WorkKind`s and only sweep and demolish are new job types.
Decided by default (A): table `[ai.obstacles]`; own 3 % S10b materials budget; `jobs_max` 1 inside `builders_max`;
no card weight, price or buy change (the AI already buys Pioniere and US Engineers, who lay and sweep); no urgent
buys for sweepers or demolishers; vehicles avoid AT mines only, men AP only; F8 last and droppable (switch 0 if cut);
two 40-match control runs (`skirmish40_off`, `skirmish40_loose`) in `measure.sh`.
Moved to later slices: own-mine avoidance (with S10b-1 (B)); per-man mine paths; chokepoint analysis; tank or
mortar fire at obstacles and mines; presenter work. Owner questions S10b-ai-1..7 (spec 10).

## Build A chunk 1 (2026-11-12, layout commit `0ae000f`)

Decided by default: `refused_near` / `drop_stale_refusals` take ticks (no hz in the helper); `MineBlast` list must be
sorted by tick on load (added to spec 6); `AiDemolish::pieces` uses `MAX_DEMOLISH_PIECES` 3; the staged-save checks
went into `staged_s10b_save_holds_its_state` (the seed-match `MUST_HOLD` cannot reach S10b-ai state before Build B);
`fuzz_plan_reaches_s10b_state` asserts entries (`.sweeps.{}`, `.demolitions.{}`, `.refused.[]`, `.blasts.[]`), since
the bare names also match the new counters (`sweeps_started`, `refused_reorders`). `ablate.sh` dry run failed once on a
stale CLI binary (old code rejects `[ai.obstacles]`); green after the rebuild.
