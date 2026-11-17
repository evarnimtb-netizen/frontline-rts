# S11a2 Build B: JSON, fortify rows, AI safety under friendly fire, AI counter-fire, switches on, 40 sets

Read first: `S11a2.md` 4 (M1-M3, S1-S3, R1-R3), 5 (rows "Lift", "Counter-fire"), 7, 8 (the fallback if time runs out); the log's top block (Build
A's code map and test recipes); `S11a1-log.md` top block (Build C's notes: `run_arty`, the target order, A6). **2 chunks**, about 30 self-counted
turns each (60-90 minutes in all). No layout change (every field exists since Build A), no version change. Chunk 1 is a complete result by itself:
if chunk 2 does not fit, leave `counter_fire` 0 and the `ff_lift` keys null, say so, and stop at STOP POINT 1.
Work rules as in Build A (4-minute rule, `tools/run.sh`, `BATCH_JOBS=4 tools/batch.sh`, commit after every green step and only your files, index
lock: wait and retry, no push, no tick times, no model names). No RNG in any plan; ascending ids; integer metres and seconds. `<BASE>` is the hash
in the log's top block.

Files owned: `crates/sim/src/ai/arty.rs` (`own_near`, the lift, R1-R3 in `run_arty`), `crates/sim/src/ai/mortar.rs` (the S2 filter and the
spotter stand-off), `crates/sim/src/ai.rs` (the call of the lift step in the plan; a `World` hook for the row only if M3 is built, with
`crates/sim/src/world.rs`), new `crates/sim/src/s11a2_ai_tests.rs`; new `sdk/crates/cli/src/fortify/s11a2.rs`,
`sdk/crates/cli/src/{fortify.rs,json_out.rs,skirmish.rs}`; `tools/neutral.py` (+ `tools/test_neutral.py`); `mods/ww2/defs/rules/skirmish.toml`
(the four switches to 1, BASELINE, snapshot); `docs/modding/{tuning.md,fire-missions.md}`; `docs/slices/targets/S11a2.toml` only to fix a metric
name that cannot be built as written (say so in the log); `docs/slices/S11a2-log.md`.

Start from: `sdk/crates/cli/src/fortify/s11a1.rs` (`spawn`, `order`, `run_out`, `par_seeds`, `gun_fm_row`: a row with control and summary text),
`json_out.rs` `s11a1_blocks` 1076, `skirmish.rs` (`S11a1Faction`, `s11a1_json_names_match_the_targets_file` 4398); `ai/arty.rs` (`run_arty`,
`arty`, the A6 lift as Build C built it), `ai/mortar.rs` (`mortars` 177, `mortar_targets` 448: the `danger` line 460 and the `own` slice,
`spot_for` 616: the stand-off at 635, `fire`), `ai/breach.rs` `drive_breach_plan` 966 with `world.rs` 2353 (how a row drives one planner piece);
`game.rs` `mission_refusal`; `counter_battery.rs` (Build A).

## Chunk 1: JSON, the two sim rows, AI safety, friendly fire on
1. M1 JSON: `factions.<f>.s11a2` and `s11a2_derived` (spec 7 names; null where the denominator is 0); `--side-detail` gets the raw counters;
   `tools/neutral.py` skips keys starting `s11a2` that BASE lacks (+ a line in `tools/test_neutral.py`);
   `s11a2_json_names_match_the_targets_file`. Commit.
2. M2 rows `ff` and `cb` in `fortify/s11a2.rs`, in the fortify JSON under `s11a2` and in the text summary; field names exactly as
   `targets/S11a2.toml` T139-T141 reads them; every row driven by player commands, each with its control computed in the same call. `ff_lift` keys
   are emitted as null for now (T142 prints SKIP, not MISSING-METRIC). Added fortify time under 60 s for 10 seeds (log it). One-off:
   `cargo test --release -p frontline-cli s11a2_rows_one_off -- --ignored --nocapture` (as the S11a1 one). `fortify_s11a2_rows_run` (1 seed, each
   row's control differs from its value). Commit.
3. S1 `own_near`, S2 (mortar filter, gun target pass, spotter stand-off), S3 (lift every own mission) with their tests (spec 4). With `ff_safe` 0
   every mortar and gun decision is as `<BASE>`: `mortar_targets_unchanged_with_ff_safe_off`, and Build C's A6 tests stay green untouched.
   Liveness: `ai_lift_and_filter_do_not_loop`. Commit.
4. Switches on, one commit each so a bad one can be reverted alone: `friendly_fire` 1 with `ff_safe` 1; then `counter_battery` 1 (markers appear,
   nothing answers yet). BASELINE and snapshot refreshed.
5. Runs (each through `tools/run.sh`, `BATCH_JOBS=4`; `batch.sh` caches by binary, mods and overrides, so a second path for the same run is
   free): default 40 set into `target/s11a2-B1-40/skirmish40.json`; `b_off` into `target/s11a2-B1off40/skirmish40.json`; `ff_off` and `ff_safe_off`
   into `target/s11a2-B1-40/skirmish40_ff_off.json` and `skirmish40_ff_safe_off.json`. `python3 -I tools/neutral.py target/s11a2-base40
   target/s11a2-B1off40` -> `same 40/40` (the tool compares same-named files of two folders).
**STOP POINT 1.** `check.sh quick <BASE>` green; commit; log top block with the chunk-1 numbers of "The stage must log"; return if over 45
self-counted turns (then chunk 2 is a new session or moves to S11a3: say which you recommend).

## Chunk 2: counter-fire, the lift row, the full sets
6. R1-R3 in `run_arty` with their tests (spec 4) and the "Counter-fire" row of spec 5. The order is the existing `FireMission`; the AI asks
   `mission_refusal` first and never sends an order the sim refuses (`ai_does_not_order_a_masked_counter_fire`). `counter_fire` 1. Commit.
7. M3 row `ff_lift`: the lift step driven per plan through a `World` hook shaped like `drive_breach_plan`; 10 seeds x 2 factions; control
   `ff_safe` 0. If the hook does not fit in 10 turns: keep the unit tests of S3, leave the keys null, say so first in the log.
8. Runs: fortify 10 seeds into `target/s11a2-B40/fortify10.json` (`tools/run.sh target/s11a2-fort.log env BATCH_JOBS=4 tools/batch.sh
   target/s11a2-B40/fortify10.json -- fortify-test --seeds 10`; it takes about 28 minutes because of the S10b-ai rows: start it first, poll with
   `tools/run.sh --wait`, and work on step 9 meanwhile only if the release binary is not rebuilt under it: `batch.sh` runs a snapshot, so a
   rebuild is safe); default 40 set into `target/s11a2-B40/skirmish40.json`; `ff_off`, `ff_safe_off` into
   `target/s11a2-B40/skirmish40_<name>.json`; `b_off` into `target/s11a2-Boff40/skirmish40.json` and again into
   `target/s11a2-B40/skirmish40_b_off.json`; `a_off` + `b_off` (both `-o` files) into `target/s11a2-Baoff40/skirmish40.json` and again into
   `target/s11a2-B40/skirmish40_a_off.json`; `cb_off` and `counter_fire_off` into `target/s11a2-B40-<name>/skirmish40.json`.
9. `neutral.py target/s11a2-base40 target/s11a2-Boff40` -> `same 40/40`; `neutral.py target/s11a1-base40 target/s11a2-Baoff40` (base
   `730e67f`, if that folder still exists; else say "not checked") -> `same 40/40`. Then `target/release/frontline-cli targets docs/slices/targets/S11a2.toml
   --from-json target/s11a2-B40` and copy the T134 and T139-T148 lines into the log: no `MISSING-METRIC`, `CONTROL-MISSING` or `EXPOSURE-MISSING`
   on them (160-set rows print "not played": the orchestrator's measure fills them).
Tests that must exist at the end: the M, S and R tests of spec 4, `ai_all_s11a2_switches_off_emit_nothing_new`,
`ai_s11a2_deterministic_threads_and_save_load` (1 and 4 threads, save/load at three ticks: one with a lift pending, one with a counter-fire
mission in Laying).
**STOP POINT 2.** `check.sh quick <BASE>` green, last; commit; rewrite the log's top block.

## The stage must log
Commits; per faction on the default 40 set (a 40 set has 11-15 points of noise: sanity numbers, the 480 is the orchestrator's): Germany's wins of
40 for default, `b_off`, `ff_off`, `ff_safe_off`, `cb_off`, `counter_fire_off`; `he_share_pct.mean` for default and `ff_off`;
`s5b_derived.bombs_per_match` for default and `ff_off`; `ff_kills_per_match` for default and `ff_safe_off`; `ff_lifts_per_match`,
`ff_holds_per_match`, `cb_marks_per_match`, `cb_first_error_m`, `cb_fired_matches_pct`, `cb_rounds_per_match`, `cb_near_burst_pct`,
`cb_masked_per_match`, `cb_shell_share_pct`, `gun_observed_or_cb_pct`; the rows `ff`, `cb` (with `answer`) and `ff_lift` with their controls;
`neutral.py` lines; fortify wall time added; test count.
Stop and report instead of tuning when: Germany's 40-set share leaves 25-75 %; `he_share_pct.mean` passes 26.3; a match has no winner; the
default set's `ff_kills_per_match` is above 3 on either side (the safety rule is not working: name the case from one match with
`--side-detail`); `s5b_derived.refused_total` or `gun_refused_total` is not 0. Do not change lethality, error or price values to move a number.
For Review: the list of orders the AI can now emit that it could not before (the `Stop` of S3, the counter-fire `FireMission`), where each is
rate-limited, and every place `ff_safe` 0 and `counter_fire` 0 branch back to Build C's code.
