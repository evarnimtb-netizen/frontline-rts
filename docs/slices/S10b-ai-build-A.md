# S10b-ai Build A: layout, switches, measures, targets tool, liveness harness

Read first: `S10b-ai.md` 2, 3, 4 (L1-L4), 6, 7; the log's top block. **2 chunks**, about 30 self-counted turns each
(real turns are about 1.75x). No AI behaviour in this stage: every switch exists, nothing acts on it yet.
Work rules: read with `tools/outline.sh` and `tools/show.sh FILE:FROM-TO` (several ranges per call); targeted release
tests only (`tools/run.sh LOG cargo test --release -p CRATE --test FILE name`); never debug `cargo test`;
`tools/check.sh quick 72b2970` once at the end of each chunk. Commit only your files: `git commit -m ... -- FILES`.

Start from: `crates/sim/src/ai.rs` (`AiState`, `AiCounters`, `AiState::validate`, `validate_fort`);
`crates/sim/src/ai/fort.rs` (`WorkKind`, `fort_of`, `AiFortJob`); `crates/sim/src/fort.rs` (`FortState`, `FortAccount`,
`FortState::validate`); `sdk/crates/content/src/tuning.rs` (`AiFortTuning`, the `("ai.fort", ...)` key list, `FortTuning`);
`crates/sim/src/world.rs` (`SAVE_VERSION`, `drive_breach`); `crates/sim/tests/save_fuzz.rs` (`staged_s10b`, `MUST_HOLD`).

## Chunk 1: the layout commit (one commit, SAVE 24 -> 25)
1. Content: `AiObstacleTuning` (`tuning_table!`, spec 2 keys and ranges, switches 0..=1) as `[ai.obstacles]` in
   `defs.rs`/`setup.rs`/`tuning.rs` key list; `[fortify]` += `mine_paths` 0..=1, `mine_path_cost_pct` 100..=10000,
   `mine_band_foot_cm` 0..=1000, `mine_band_vehicle_cm` 0..=1000; `[ai.truck]` += `refusal_memory` 0..=1. Values in
   `mods/ww2/defs/rules/skirmish.toml` with one comment each (as `[ai.fort]`). Serde defaults = the spec values.
2. Sim: every field of spec 3 (`AiSweep`, `AiDemolish`, `AiRefusal`, `JobKind`, `SweepPhase`, `DemolishPhase`,
   `AiFortJob::start`, the `WorkKind`s, `AiCounters` +14, `FortState::blasts` + `MineBlast`, `FortAccount` +4,
   `MAX_SWEEPS`, `MAX_DEMOLITIONS`, `MAX_REFUSED`, `MAX_BLASTS`). `fort_of` returns S10b blueprints only for the new kinds
   (R29 kept for the S10a five). `AiState::new` empty; hash through the existing paths. `SAVE_VERSION` 25,
   `save_fuzz.rs` mirror version 25. PROTOCOL untouched.
3. Validation (spec 6), one `load_rejects_bad_s10b_ai_state` case per line; `start <= since` for fort jobs.
4. Helpers (no caller yet): `AiState::refuse(kind, at, tick)` (L3: sorted insert, full -> replace the oldest `failed`),
   `AiState::refused_near(kind, at, tick, refused_s, m) -> bool` (L4), `AiState::drop_stale_refusals(tick, keep)`.
5. Hook skeleton: `World::drive_ai_habit(ai, player, AiHabit, &DriveArgs) -> Option<HabitEnd>` (as `drive_breach`:
   the habit's plan every `replan_ticks`, commands through `ai.pending`); each arm returns `Some(HabitEnd::NotBuilt)`.
6. Fuzz stage: **"fuzz stage: staged_s10b_ai"**: extend `staged_s10b()` (or a sibling the fuzz set also reads) with a
   test hook `World::with_ai_state(player, |ai| ..)` that inserts one `AiSweep`, one `AiDemolish` (one piece done),
   two `AiRefusal`s, a fort job of kind `ApField` with `start`, and one `MineBlast`; add their paths to `MUST_HOLD`
   and to `fuzz_plan_reaches_s10b_state`'s path filter (`s10b_path`).
7. Baselines: `resolved_defs.rs` BASELINE, canonical snapshot, `simbench/src/report.rs` if a setup line changed.
Tests (each with its failing control): `s10b_ai_layout_roundtrips_and_hashes` (control: skip `blasts` in the hash ->
hash equal across a changed blast), `load_rejects_bad_s10b_ai_state` (control: drop the `refused` sort check),
`ai_obstacle_tuning_parses_and_validates` (control: `jobs_max` 0 accepted), `refusal_memory_replaces_oldest_failure`
(control: replace the newest), `fort_of_ignores_s10b_blueprints` still passes and
`fort_of_returns_s10b_blueprints_for_new_kinds` (control: R29 guard on every kind).
**STOP POINT 1.** `tools/check.sh quick 72b2970` green; commit (layout commit id in the message); rewrite the log's top
block (commit, field names, what chunk 2 starts with); return.

## Chunk 2: measures, targets tool, ablation files, liveness harness
8. Counters count only with their switch on (all-off JSON stays zero for `neutral.py`). CLI `skirmish.rs`, `json_out.rs`:
   `factions.<f>.s10b_ai` = the 14 `AiCounters` + `lost_on_known`, `at_exposure_ticks`, `own_mine_losses`,
   `own_pieces_demolished`. `s10b_ai_derived`: `matches_with_mines_pct`, `mines_laid_per_match`, `at_fields_per_match`
   (`works_done.at_minefield`), `lay_chance_matches`, `tanks_stopped_per_match` (own `vehicles_mined`),
   `at_exposure_s_per_match`, `matches_with_obstacle_pct`, `obstacles_per_match`, `obst_chance_matches`,
   `matches_with_bunker_pct` (null with no chance), `bunker_chance_matches`, `sweep_when_wanted_pct` (null if no
   want), `sweep_wanted_matches`, `mines_swept_per_match` (`mines_lifted`), `demolish_when_wanted_pct`,
   `demolish_wanted_matches`, `gaps_opened_per_match`, `own_side_mistakes` (sum: `own_blocked`, `own_mine_losses`,
   `own_pieces_demolished`), `s10b_materials_mp_pct`, `jobs_overdue`, `refused_reorders`, `lost_on_known_per_match`.
   Text summary line "s10b-ai:" per faction; `tools/pool.py` prints the four "matches with" shares and mistakes.
9. Targets tool (`sdk/crates/cli/src/targets.rs`): `TargetSpec` += `exposure`, `exposure_min`, `control`; file key
   `three_parts_from` (compare the id's number: T109 > T99). Verdicts: exposure below min -> `UNEXPOSED` (counted apart, run fails, never "hard miss");
   control inside the row's bounds -> `CONTROL-SAME` (fails); missing parts on a row at/after `three_parts_from` ->
   parse error. `tally` prints them. `measure.sh` plays every `skirmish40_*` run in the targets file with its
   `rules_override` (cached by `batch.sh`). Tests: `unexposed_row_fails_not_hard`, `control_inside_bounds_fails`,
   `three_parts_required_from_id` (control each: the old evaluate path).
10. Ablation files `docs/slices/targets/S10b-ai-ablations/*.toml` (spec 2) and the control `loose.toml`; `tools/ablate.sh` takes `ABLATIONS=DIR`,
    `NEUTRAL_BASE` default `target/s10b-verify`, the dry run accepts the new tables' switch keys.
11. Liveness harness in `crates/sim/src/s10b_ai_tests.rs`: `ai_jobs_end_and_never_reorder_a_refused_goal` iterates a
    table `LIVENESS: &[(JobKind, fn(&mut World) -> Stage)]` (empty in A; B, C, D add rows); per row: 20 min run,
    asserts L1/L2 end, one refusal, no command for the goal for `refused_s`, at most one retry after. With the table
    empty it checks only the harness on a dummy stage. Control: a dummy kind that never refuses fails it.
12. Synthetic CLI test `s10b_ai_derived_fields` on fake per-match records (every field of 8; nulls when unexposed).
Ends: quick green; 40 set with defaults (`tools/batch.sh`, background) and `python3 -I tools/neutral.py
target/s10b-base40 DIR` **same 40/40** (no behaviour yet). **STOP POINT 2.** Commit; rewrite the log's top block (field
names, targets keys, 40-set line, a code map: file -> symbols added); return.
