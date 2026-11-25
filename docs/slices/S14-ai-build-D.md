# S14-ai Build D: flame jobs, refuel, flame buy, AI clear trench (A1-A4, liveness); JSON; switches on

Read first: `S14-assault-kit.md` (owner-change note at the top; sections 2 `[ai.flame]`, 7, 10); `S14-log.md` "For the next stage"
("What S14-ai calls", open owner questions); `crates/sim/src/ai/breach.rs` (the phase shape A1 copies) and `ai/medic.rs` / `ai/arty.rs`
for how an AI job module is laid out and wired in `ai.rs`; `S12b-build-C.md` as the pattern of an AI stage. `<BASE>` is `a27ce63`.
Branch `claude/project-thread-aj1tww`. **2 chunks**, about 30 self-counted turns each. No layout change (every saved field and key
exists since S14a A). The presenter stage runs beside you in another worktree: do not touch `godot/` or `crates/godot-bridge/`.

Owner rule (2026-11-25): **no balance runs, no tuning.** No 480, no ablation runs, no lever changes. The AI only uses the mechanics;
every number stays a TOML key. One 40 set with the switches on (step 6) is a breakage check only.

Work rules: as S14a (`tools/run.sh`, no command over 4 minutes, `git commit -m "S14-ai D: ..." -- FILES`, never `git add -A`, never
`target/`, `.claude/`, `tools/__pycache__/`, `godot/match/fort_view.gd.uid`; do not push; no model names in files or commits). 40 sets
only through `tools/batch.sh` with `BATCH_JOBS=4`, **two** in this stage (step 6); if refused, skip and say so.

## Chunk 1: the AI
1. `crates/sim/src/ai/flame.rs` (plans without RNG, ascending ids): A1 flame job (Close, Smoke, Go, Burn, Clear phases;
   `Planner::flame_focus` beside `breach_focus`; bunker slit point from the fort def), A2 refuel, A3 buy (urgent buy of the faction's
   flame card, at most `cards_max` alive; the buy reads the derived `Capability::Flame`), A4 AI clear trench. Each behind its
   `[ai.flame]` switch; counters `flame_jobs`, `flame_job_ends[6]`, `flame_buys`, `refuels`, `clear_orders`. Wire in `ai.rs`.
2. Liveness and the plan-7 tests by name: `flame_job_ends_when_operator_dies`, `..._on_deadline`, `..._on_stall`,
   `refuel_when_tank_dry` (control `refuel` 0), `flame_buy_once_per_cap`, `ai_flames_the_slit`, `ai_clears_after_burn`; save fuzz
   with jobs in each phase; threads 1 vs 4 hash equal. New `crates/sim/src/s14_ai_tests.rs`. Commit after each green step.

## Chunk 2: numbers and switches
3. JSON `s14_derived` per faction in `sdk/crates/cli/src/json_out.rs` (`flame_matches_pct`: matches where an AI flame man hit a man
   in a trench or bunker; flame kills and downs per match; strikes, hits, kills per match; clear orders, steps, done; refuels;
   `flame_capped` and patches dropped). `targets/S14.toml` rows T189-T200 as **reports**, except T190 (no match without a winner,
   length 10-40 min) and T192 (every flame job ends) hard: owner change, no balance threshold. `tools/pool.py` S14 line if earlier
   slices have one.
4. Switches on in `mods/ww2/defs/rules/skirmish.toml`: `[flame] enabled` 1, `[melee] enabled` 1, `[clear] enabled` 1, `[ai.flame]`
   `jobs/refuel/buy/clear` 1. Update `targets/S14-ablations/` so `s14_off` turns everything off (neutral control) and each other file
   turns one part off. BASELINE refreshes (resolved defs, canonical snapshot) for the switch values only. Commit.
5. If a rule never fires in matches (A1-A4, strikes, clears), find out why and fix the mechanics or the AI (a bug), never a number.
6. Two 40 sets: default (switches on) into `target/s14-D-40/` and `s14_off` (`-o docs/slices/targets/S14-ablations/s14_off.toml`)
   into `target/s14-D-off40/`; `s14_off` must equal `target/s14-base40` under `python3 -I tools/neutral.py`. Read the default set for
   breakage only: matches without a winner, length min/median/max, panics, how often each S14 rule fired. Report Germany's wins of 40
   without judging them.
7. `tools/check.sh quick a27ce63` green (IPv6 test excepted). Rewrite the log's "For the next stage" block for Review: state, code
   map, numbers, open owner questions, deviations.

Files owned: `crates/sim/src/ai/flame.rs`, `crates/sim/src/ai.rs` (wiring), `crates/sim/src/ai/*.rs` only where a hook is needed,
new `crates/sim/src/s14_ai_tests.rs`, `crates/sim/tests/save_fuzz.rs`, `sdk/crates/cli/src/json_out.rs`, `sdk/crates/cli/src/skirmish*`,
`mods/ww2/defs/rules/skirmish.toml`, BASELINE files, `docs/slices/targets/S14*`, `tools/pool.py`, `docs/modding/tuning.md`,
`docs/slices/S14-log.md`. A mechanics bug found in step 5 may be fixed in `crates/sim/src/flame.rs` with a test.
