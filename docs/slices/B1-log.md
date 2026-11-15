# B1 log (balance pass; at most 150 lines)

Plan `docs/slices/B1-balance.md`, targets `targets/B1.toml`. Branch `balance-pass` from main `72b2970`. Commit only
your files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)
**B1-2 Apply done (2026-11-15): the triple is now the default.** `[ai.fort]` `builders_max` 1, `works_per_point` 2,
`builder_reach_m` 120 in `mods/ww2/defs/rules/skirmish.toml` and in `AiFortTuning::default()` (`tuning.rs`); both
factions share the keys; nothing else changed. Old values (2 / 3 / 200): `docs/slices/targets/B1-levers/revert_old_defaults.toml`.
Pinned values fixed: golden setup hashes in `resolved_defs.rs` and `ww2_canonical.json` (changed only by the new
defaults), three s10a and one s10b bunker test now set the old value they assume, tuning.md row 28.
Revert check (release CLI, 40 set with the revert file vs `target/b1-base`): `skirmish40.json: same 40/40 (note:
overrides 0 -> 3 items)`. Check: tools/check.sh quick 72b2970 (last run, see the commit); full tests and measure.sh not run.
Screen (seeds 1001-1080, 160 per run, Germany %, base 41.88): cause (a) all four switches off 32.50; (b) works off
51.25 (floors fail); L1 45.62; L2=2 45.62 (L2=1 kills wire); L3 inert; L4=120 45.62; L5 lowers Germany (38.75 / 28.75).
Confirmation (3 fresh blocks 1001-1080, 1101-1180, 1201-1280, 480 matches per run, paired by seed and side):
- base 43.33 % (41.88 / 46.88 / 41.25); triple 49.58 % (46.25 / 52.50 / 50.00), flips 83 to Germany / 53 to USA,
  net +30 (about 2.6 sigma). L1 alone 47.29 (+19), L4=120 alone 46.25 (+14), L2=2 alone 45.21 (+9), L2+L4 46.46 (+15);
  works off 50.62 (+35, floors fail).
- Fort use with the triple: Germany works 92-96 %, wire 76-82 %, builders 6.7-6.9 %; USA works 78-84 %, wire 56-62 %
  (base USA wire 51-61 % on the same seeds: not lowered), builders 6.4-7.4 %.
Decided: L3 (inert) and L5 (more rifle squads, lowers Germany) are rejected. L6-L8 and the code fix of the builder
hold (owner Q1 C) were not needed and not done. Targets rows that pinned old fort numbers move with the next measure.sh.
Next: the orchestrator runs tools/measure.sh and the full tests on the new defaults, re-reads targets, then merges.


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
