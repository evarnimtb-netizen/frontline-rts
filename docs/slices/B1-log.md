# B1 log (balance pass; at most 150 lines)

Plan `docs/slices/B1-balance.md`, targets `targets/B1.toml`. Branch `balance-pass` from main `72b2970`. Commit only
your files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)
**B1-0 handover (2026-11-15).** Owner answers Q1-Q4 recorded in B1-balance.md ("Owner answers 2026-11-15"); TODO marks
S10b-ai-E1 and E3 decided. E3 is built: `[ai.obstacles] demolish_handover` (0/1, default 1).
- What it does (`crates/sim/src/ai/demolish.rs`): in Go, before the `Demolish` order, a job whose squad has no
  satchel returns `NoSatchel`; `demolish_continue` gives the job (pieces, `done`, `start`) to the free own `demolish`
  squad in `mine` with a satchel within `sweep_reach_m` of the vehicle, nearest then id (`handover_pick`), re-keys
  `AiState::demolitions`, sends it to the stand point; the first squad stays in `mine` (free). None: Failed (L3).
  `demolish_start` prefers a squad with a satchel. The fortify habit (`drive_demolish`) now ends when no job is left
  (any key), not when its own squad's key is gone (same for one-job rows).
- No save layout change (re-keyed BTreeMap entry). SAVE 25, PROTOCOL 14. Setup bytes baseline and canonical snapshot
  re-recorded (new key); content key count 36.
- Fortify row `s10b_ai.demolish` now has a second engineer squad (mirrored, 40 m the other side, card satchels) and a
  new control `handover0`. One-off seeds 1-3 both players: gap_opened 100, tank_passed 100, pieces_used 2.0, blocked
  100; control (demolish 0) 0 / 0 / 0; handover0 0 / 0 / 1.0. New report row T114h (B1.toml) on handover0.
- Ablation `S10b-ai-ablations/demolish_handover_off.toml`; key added to `all_off.toml` and `tools/ablate.sh`.
- Next: the E1 item (orchestrator's default; not in this stage), then B1-1 screens. Skirmish AI use of demolition
  stays rare (T114a-d: vehicles route round first); this change moves only fortify T114e/f.

## B1-0 handover (2026-11-15)
Tests (`crates/sim/src/s10b_ai_demolish_tests.rs`, card satchel 1 each): `ai_demolish_hands_over_to_a_second_squad`
(control handover 0: one charge, Failed), `ai_demolish_one_squad_alone_still_fails` (Failed, s10b_refused 1, no
`Demolish` order sent; control with a second squad: Done), `ai_demolish_handover_saves_and_loads` (the hand-over AI
state in a save loads through `validate_s10b`, a second save/load runs 60 s to the same hash and AI state; control
handover 0: no hand-over). Failing controls seen: with `handover_on` forced false all three fail. All s10b sim tests
(117) and `save_fuzz` (14, staged save included) pass.
All-off 40 (`tools/batch.sh target/b1-0-off40/skirmish40.json -o docs/slices/targets/S10b-ai-ablations/all_off.toml --
skirmish --ai-vs-ai --seed 1 --seeds 20 --both-bases --summary --minutes 45 --jobs 6`): `python3 -I tools/neutral.py
target/s10b-base40 target/b1-0-off40` same 40/40 (14 override items). The first quick run failed only `ablate.sh`
dry run: the release CLI was older than the new TOML key (rebuilt by the test step); the rerun is the last check.
