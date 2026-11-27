# S14a Build C: melee M1-M5, `Strike`, melee blocks, row R5 (every switch off)

Read first: `S14-assault-kit.md` (owner-change note; sections 2 `[melee]`, 3.1 `Close`, 4.3, 5, 9 R5, 10 T188); `S14-log.md` "For the
next stage" (Build B's hand-over). `<BASE>` is `a27ce63`. Branch `claude/project-thread-aj1tww`. **1 chunk**, about 30 self-counted
turns. No layout change (every field exists since Build A). `[melee] enabled` stays 0; tests turn it on in the test only.

Owner rule (2026-11-25): no balance runs, no tuning; every number a TOML key with a range and comment.

Work rules: as Builds A and B (`tools/run.sh`, no command over 4 minutes, `git commit -m "S14a C: ..." -- FILES`, never `git add -A`,
never `target/`, `.claude/`, `tools/__pycache__/`, `godot/match/fort_view.gd.uid`; do not push; no model names in files or commits).
One 40 set (step 5) through `tools/batch.sh` with `BATCH_JOBS=4`; if refused, skip it and say so.

1. Weapon def `[melee] reach_cm, damage, windup_ms, recover_ms` (ranges; absent: the `[melee]` rules spade). Data of plan 4.3 on the
   rifles, SMGs and StG 44 with `sources`. Resolved-defs baseline (`BLESS=1`). Commit.
2. M1-M4: automatic strike (M2 conditions), wind-up in `aim_and_fire` writing only the man's own fields, strike requests buffered per
   chunk and resolved serially in the Damage stage by ascending striker id (a striker downed earlier in the same stage does not
   strike), recovery; damage through `hurt` with the W1 zone and class MELEE, `hit_suppression` on the struck man; counters
   `strikes`, `strike_hits`, `strike_kills`. `Strike { units }` accepted when `[melee] enabled` 1 (refused at 0 or with nobody in reach);
   bridge intent and relay doc line if not already there. The M5 tests by name. Commit.
3. `save_fuzz` staging: a man mid-strike (wind-up and recovery); save/load mid-wind-up hashes equal; thread-count hash test (1, 2, 3,
   8) with `[melee] enabled` 1 in a short scene. Commit.
4. Row R5 (hard, T188) in `fortify/s14.rs`: 100 strikes at a man 0.9 m ahead hit 100; 0.9 m aside hit 0; wired into
   `json_out::s14_checks`. Commit.
5. **T185 again** (`target/s14-C-40/` vs `target/s14-base40` with `python3 -I tools/neutral.py`).
6. `tools/check.sh quick a27ce63` green (IPv6 test excepted). Rewrite the log's "For the next stage" block for the presenter stage
   and S14-ai: state, full S14a code map (flame, clear, melee), snapshot fields and commands the presenter needs, what the AI will
   call, open owner questions found so far (R1 slit geometry, R1/R3/R6 report-only reasons), every deviation.

Files owned: as Build B. Not the presenter's GDScript, not `crates/sim/src/ai/`.
