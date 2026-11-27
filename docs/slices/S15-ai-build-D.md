# S15-ai Build D: rider jobs, card-table test, switches on, JSON, targets; T211-T213 proof

Read first: `S15-ussr.md` sections 0, 2, 7, 11, 14; `S15-log.md` "For the next stage" (Build C's S15-ai list: purchase mix, riders,
the `second_bases` count 72 vs 1, the tag `vehicle_riders`); `crates/sim/src/ai/flame.rs` (the latest AI job module) and `ai/breach.rs`
for the phase shape. Branch `claude/project-thread-aj1tww`. **2 chunks.** No layout change. A presenter stage runs beside you in
another worktree: do not touch `godot/` or `crates/godot-bridge/`.

Owner rule (2026-11-25): **no balance runs, no tuning.** No 480, no ablation runs, no weight or price changes to make win rates move.
The AI only uses the mechanics; a habit that never fires because of a bug is fixed in code.

Work rules: as S15a (`tools/run.sh`, no command over 4 minutes, `git commit -m "S15-ai D: ..." -- FILES`, never `git add -A`, never
`target/`, `.claude/`, `tools/__pycache__/`, `godot/match/*.gd.uid`; do not push; no model names in files or commits). 40 sets only
through `tools/batch.sh` with `BATCH_JOBS=4`: **three** in this stage (step 5; owner-approved neutrality and breakage checks; if
refused, skip and say so).

1. `crates/sim/src/ai/riders.rs`: A1 rider job (Board, Ride, Drop; ends counted; L1-L3) behind `[ai.riders] jobs`; wire in `ai.rs`.
   A2 card table test `ai_habits_find_ussr_cards`. Liveness tests `rider_job_ends_when_tank_dies`, `..._on_deadline`, `..._on_stall`,
   `rider_job_drops_short`; save fuzz per phase; threads 1 vs 4. Commit after each green step.
2. Look into Build C's `second_bases` 72 vs 1 (what the counter means, why the USSR AI does it so often: a bug or a missing card
   habit is fixed in code; a matter of numbers is reported only) and the cards never bought (ZiS-2, ZiS-3, SU-85, SU-76M, scout car):
   which habit should pick each and why it does not. Fix bugs; report the rest.
3. Fix the spotting saturation Build C found (`crates/sim/src/spotting.rs`, `Fx::from(v.sight).mul_int(pct)` saturating above about
   327 m) with a test; shipped sights are 150-220 m, so the default 40 set must not change (T211 proves it).
4. Switches on: the USSR in `[matchups]`, the S15 switches and `[ai.riders] jobs` 1 as the plan says; JSON `s15` and `s15_derived` in
   `sdk/crates/cli/src/json_out.rs`; `targets/S15.toml` T212-T218 as the plan's section 11 (hard: T210-T213; reports: the rest);
   `tools/batch.sh pair DIR F0 F1` if the plan's command needs it. BASELINE refreshes for switch values only. Commit.
5. Three 40 sets: **T210** (`s15_off`) and **T211** (shipped) on the default Germany-USA command, each `python3 -I tools/neutral.py
   target/s14-F-40 <dir>` same 40/40 (`target/s15-D-off40`, `target/s15-D-40`); **T212** Germany-USSR (seeds 1-20, both bases) into
   `target/s15-D-pair40`: 40 winners, no panic, length 10-40 min, hashes equal at 1 and 4 threads on seeds 1 and 2; **T213a-f** each
   rule fires (plan list). Report the USSR's wins without judging them. A rule that never fires: find out why (step 2 rule).
6. `tools/check.sh quick 96d4b4b` green (IPv6 excepted). Rewrite `S15-log.md` "For the next stage" for Review: state, code map,
   numbers, open owner questions, deviations.

Files owned: `crates/sim/src/ai/riders.rs`, `crates/sim/src/ai.rs` (wiring), `crates/sim/src/ai/*.rs` where a hook is needed,
`crates/sim/src/spotting.rs` (step 3), new `crates/sim/src/s15_ai_tests.rs`, `crates/sim/tests/save_fuzz.rs`,
`sdk/crates/cli/src/json_out.rs`, `sdk/crates/cli/src/skirmish*`, `mods/ww2/defs/rules/skirmish.toml`, BASELINE files, `tools/batch.sh`,
`tools/pool.py`, `docs/slices/targets/S15*`, `docs/modding/tuning.md`, `docs/slices/S15-log.md`.
