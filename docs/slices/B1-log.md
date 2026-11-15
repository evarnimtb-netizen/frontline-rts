# B1 log (balance pass; at most 150 lines)

Plan `docs/slices/B1-balance.md`, targets `targets/B1.toml`. Branch `balance-pass` from main `72b2970`. Commit only
your files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)
**B1-4 Build + Measure done (2026-11-15); no default changed.** Commit `8e1d075`: new key `[ai.fort] hold_max`
(0..=16, default 2) caps the fix F15 builder holds in `ai/fort.rs` `fort_holds` (stop once `hold_max` points hold;
jobs still start for engineers assigned without a hold; inside `builders_max`). Default bit-identical (40 set vs
`target/b1-base`: same 40/40); no save or protocol change. Noise rule unchanged: z = (toDE - toUS) / sqrt(toDE +
toUS), paired by (seed, side); nothing below z 2.5. Fresh blocks 1001/1101/1201 (480), base Germany 43.33 %.
- hold0 (`B1-levers/hold0.toml`): 47.71 % (+4.38), flips 62/41, z 2.07; blocks 51.25 / 46.25 / 45.62. Base under
  20 min 31/15, 20 min or more 31/26. FLOORS FAIL: wire in DE 41/43/36 %, US 41/29/39 % (floor 60); works DE 94, US
  78 %; builders DE 7.1, US 7.6 %. Mines collapse (DE 28 -> 4 %, US 15 -> 4 % of matches); own_side_mistakes DE 8 ->
  24 (8/7/9 by block), US 0.
- hold1: identical to base (0 flips; block 1001 same 160/160 hashes): with builders_max 2 and one job running,
  `job_room` already allows only one hold, so hold_max 1 never binds in these matches.
- hold0 + L1 (`hold0.toml` + `L1_builders1.toml`): 47.08 % (+3.75), 63/45, z 1.73; blocks 48.75 / 49.38 / 43.12;
  under 20 min 31/14, 20 min or more 32/31. Floors fail the same way (wire DE 39, US 35 %); mines DE 2, US 2 %;
  own_side_mistakes DE 22, US 0.
- Reading: removing the hold moves the opening the same way as works_off (gain in short matches) but below z 2.5 and
  it takes the wire (and with it the AP mines laid beyond wire) away: the hold is how wire gets built. Not a fix.
- For comparison (same blocks, earlier runs, re-pooled by `tools/pool.py`): L1 builders_max 1 +3.96 (36/17, z 2.61)
  keeps the floors as base (wire DE 77, US 56 %, builders DE 7.8, US 7.9 %; mines DE 19, US 10 %).
- Base itself misses the US wire floor (61 / 51 / 56 %, pooled about 56 %); every candidate inherits that.
- RECOMMENDATION: replay ONE candidate on seeds 1-280: L1 alone (`B1-levers/L1_builders1.toml`, a setting). Do not
  apply hold_max 0 or 1; keep hold_max 2. Nothing here is applied from fresh seeds alone.
- Sets: `target/b1-4/s{1001,1101,1201}/{base(1001 only),hold0,hold1,hold0_L1}` (all `--side-detail`), pool dirs
  `target/b1-4/pool/*`, flips `python3 -I target/b1-4/paired.py hold0 hold1 hold0_L1` (base = `b1-diag/s*/base`;
  replay of 1001 base with the new binary: same 160/160 as `b1-diag/d1001/base`).
- T115c note (B1-3) stands: own_blocked at base 0 rises whenever freed engineers build more obstacles.
Last check: tools/check.sh quick 72b2970 all ok (826 tests), after the hold_max change (before the measure runs).

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

## B1-3 diagnosis (2026-11-15)
CLI only, read-only: `skirmish --seeds N --json F --side-detail` adds `per_match[].sides` (per player: kills, losses,
rounds fired and kills by class, buys, pop at 5/10/20 min, hold_s, fort jobs, builder and infantry plans, works,
wire, mines, fort and S10b MP, own_blocked, and `deaths` by place: within 40 m of enemy / own works, of an enemy MG
nest, within 10 m of enemy / own wire, from new corpses). Off by default; hashes equal with it (seed 1001 160 set:
160/160 same as `b1-screen/base`; tuned 101-180: 160/160 same as `b1-tuned/skirmish160_oos.json`).
Runs: `target/b1-diag/queue.sh` (s1001|s1101|s1201 x base, works_off, nest/dugout/wire/roof/sandbags off),
`queue2.sh` (`tuned101`, `d1001/{base,nest_off,works_off}` with the deaths counter; d1101 stopped, not needed).
Override files `docs/slices/targets/B1-levers/diag_{nest,dugout,wire,roof,sandbags}_off.toml`. Card buys barely move
with works_off (DE Sturmpioniere 0.14 -> 0.26, US rifle squads -0.24 a match). Pop at 5 min: DE +0.55 (z 3.3), US
-0.64 (z -4.9), small. Germany at the pop cap 12.2 -> 5.1 plans a match. Sim deaths with a body: 54 of 80 (DE).

## B1-4 hold_max (2026-11-15)
`8e1d075`: `[ai.fort] hold_max` (default 2) in `tuning.rs`, `skirmish.toml`, `docs/modding/tuning.md`; `fort_holds`
breaks when `held.len() >= hold_max` before the `job_room` check. Tests (`s10a_ai_tests.rs`):
`ai_hold_max_0_holds_no_builder_but_jobs_still_start` (control hold_max 2: held, takes the dugout; hold_max 0 on the
only point: the job still starts) and `ai_hold_max_1_holds_at_most_one_point` (scene `held_pair(120, 300, max)`:
own points 0, 1 with neutral points 300 m behind each; jobs 0/1/2 for hold_max 0/1/2). Setup-bytes baseline, key
count 40 and the canonical snapshot changed only for the new key. Queue `target/b1-4/queue.sh`, world
`ba5fb262a4e747500a2d45d3`.
