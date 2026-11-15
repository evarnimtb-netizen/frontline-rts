# B1 log (balance pass; at most 150 lines)

Plan `docs/slices/B1-balance.md`, targets `targets/B1.toml`. Branch `balance-pass` from main `72b2970`. Commit only
your files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)
**B1-0 done (2026-11-15): both owner-decided AI items built on `balance-pass`.** SAVE 25, PROTOCOL 14, no save layout
change. Owner answers Q1-Q4 in B1-balance.md ("Owner answers 2026-11-15"); TODO marks S10b-ai-E1 (B) and E3 (A) built.
- E3 `[ai.obstacles] demolish_handover` (default 1, `ai/demolish.rs`): see "B1-0 handover" below.
- E1 `[ai.obstacles] bunker_yields` (default 1, `ai/fort.rs` `bunker_yields`, read by `s10b_room`): a bunker job in
  Watch whose work is in `WorkStep::Wait`, whose own squad carries 0 g and whose step is over the squad's full carry
  (the 250 kg truck step) does not count toward `jobs_max`; one other S10b job may start (`builders_max` and the S10b
  budget still apply). Watch itself is unchanged, so the bunker goes on when the kg come. Reads state only.
- Content key count 37; setup bytes baseline and canonical snapshot re-recorded (old values in the comment).
  Ablation `S10b-ai-ablations/bunker_yields_off.toml`; key in `all_off.toml`, `tools/ablate.sh`, `tuning.md`.
- 40 sets (seeds 1-20 both bases, `tools/pool.py` via a symlinked `skirmish160.json`), DE / US:
  bunker_yields 0 (= before E1, E3 on): Germany 52.5 %; mines 22 / 20 %, obstacles 12 / 5 %, bunker 8 / 0 %.
  default (both on): Germany 50.0 %; mines 28 / 20 %, obstacles 12 / 5 %, bunker 8 / 0 % (done when started 12 %).
  all off: Germany 45.0 %, all S10b 0; `neutral.py` vs `target/s10b-base40` same 40/40.
  Reading: E1 lifts Germany's mines by about 2 matches of 40 (22 -> 28 %), not back to the old ~45 %; the bunker
  is not the only thing holding mines down. Bunker and obstacles unchanged. 40-match noise is about +-15 points.
- Next: B1-1 screens on the default (both switches on). Skirmish demolition stays rare (T114a-d).

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

## B1-0 E1 bunker yields (2026-11-15)
Commit `d3e9c77`. Test `ai_bunker_waiting_for_a_truck_yields_the_job_slot` (`s10b_ai_bunker_tests.rs`): the bunker
step waits, the one truck is held in loaded tank service with `bunker_truck_first` 0 (waits up to L1), then AP mines
go on and US engineers with 120 kg stand at the point; an AP job starts within 180 s while the bunker job is still in
Watch. Control `bunker_yields` 0: no AP job (seen); with `bunker_yields` forced off in code the test failed (seen).
The AP job lands at the bunker's own point: a second trenched point either took the bunker (nearer the enemy) or,
trenched late, was not picked, so the test uses one point (the slot rule does not look at points).
All s10b sim tests 118 pass; content tests pass. Check quick 72b2970: first run failed only `ablate.sh` dry run
(stale release CLI, rebuilt by its test step); rerun all ok (824 tests). Sets in `target/b1-0y-{def,off,yoff}40`.
