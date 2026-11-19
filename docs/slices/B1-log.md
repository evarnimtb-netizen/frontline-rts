# B1 log (balance pass; at most 150 lines)

Plan `docs/slices/B1-balance.md`, targets `targets/B1.toml`. Branch `balance-pass` from main `72b2970`. Commit only
your files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)
**B1-5 Build + Measure done (2026-11-16); no default changed. Recommendation: replay NOTHING on seeds 1-280.**
Commits `593ad69` (`[ai.fort] build_after_s`, `build_min_squads`, default 0 = off: no fort hold and no new
`fort_start` job, S10b point jobs and barriers included, before that match time / under that many foot squads;
running jobs go on) and `6cb97a9`, the read-only `--side-detail` `squads_at_min` (foot squads, squads on a fort job at
1/2/3/5/10 min). Default 40 set vs `target/b1-base`: same 40/40; no save or protocol change. Fresh blocks
1001/1101/1201 (480), paired by (seed, side) against `b1-diag/s*/base` (base 43.33 %; all three base blocks replayed
160/160 equal but the final hash, which covers the setup bytes: `target/b1-5/same.py`).
- HYPOTHESIS REFUTED by the counters: no builder is on a job before minute 3, and both sides already field
  7/5 (DE/US) foot squads at minute 1, 9/7 at 3, 8.5/7.0 at 5 (base). Builders on a job, base: 0.70/0.49 at 5 min,
  0.98/0.66 at 10. The opening is never "two squads"; build_min_squads 4/5 could never bind, so 8 and 10 were run.
- after300: 46.88 % (+3.54), 85/68, z 1.37; blocks 48.75/50.00/41.88; <20 min 48/33, 20+ 37/35. Busy at 5 min
  0/0, foot squads unchanged. Floors: works DE 89, US 69 %; wire DE 63, US 37 % FAIL; builders 8.1/6.9 %.
- after600: 51.88 % (+8.54), 105/64, z 3.15; blocks 53.12/55.00/47.50; <20 min 67/31, 20+ 38/33. Foot squads at
  5/10 min as base (8.6/7.0, 10.8/9.3). Floors FAIL: works DE 56, US 46 %; wire DE 13, US 10 %; builders 3.2/3.9.
- minsq8: 46.88 % (+3.54), 57/40, z 1.73; binds the USA only (US busy at 5 min 0.05, DE 0.67). Wire US 31 % FAIL.
- minsq10: 47.29 % (+3.96), 90/71, z 1.50; works US 51 %, wire DE 41, US 14 % FAIL.
- Mines (DE/US % of matches): base 28/15, after300 20/9, after600 2/1, minsq8 25/7, minsq10 10/2.
  own_side_mistakes DE 8 in every run (8/0/0 by block, as base); US 0 at base, 2 with after300, 1 with minsq10.
- Reading: a delayed fort is mostly a lost fort (after600 leaves wire in 1 match in 8): once the front is
  contested `build_threat_max` and the threat checks keep builders off. The gain tracks how much fortification is
  removed (works_off, hold0, after600), not the army size in the opening. after600 is an ablation, not a fix.
- So no existing or new setting passes both z 2.5 and the floors. Next: owner question (keep the fort floors
  and accept 43 %, or look outside fortification: why the USA gains more from forts than Germany).
- Sets `target/b1-5/s{1001,1101,1201}/{base,after300,after600,minsq8,minsq10}`, pool dirs `target/b1-5/pool/*`,
  `python3 -I target/b1-5/paired.py base after300 after600 minsq8 minsq10` (flips, squads_at_min means).
Last check: tools/check.sh quick 72b2970 all ok (829 tests), after the last code change and the measure runs.

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
L1 alone on seeds 1-280 (orchestrator, target/b1-l1): Germany 40.62 % against 42.29 %, flips 29/37, z -0.98; with
the fresh blocks 65/54 over 960, z about 1.0. No existing setting helps on both seed groups; hold_max 0 breaks wire.

## B1-5 build gates (2026-11-16)
`593ad69`: `build_gate_open` in `ai/fort.rs` (match tick, `foot_squads`: own squads with a man alive, no vehicle,
not a crew, those on a job included) gates `fort_holds` and the starts in `fort_start` (point works and wire, roof,
walls, S10b point jobs, barriers: the same engineer squads). Not gated: trench digging, sweeps, demolitions, base
loads, the fortify-test hook. Tests (`s10a_ai_tests.rs`):
`ai_build_after_s_holds_no_builder_and_starts_no_job_before_the_time`,
`ai_build_min_squads_starts_no_job_below_the_count`, `ai_build_gate_never_cancels_a_running_job`;
controls gate 0; seen failing with the gate forced open (first two) and with a closed gate clearing jobs (third).
Key count 42, setup-bytes baseline and canonical snapshot only for the new keys. Queue `target/b1-5/queue.sh`,
world `71eff0a00aa3080adcad6fb2`; lever files `B1-levers/gate_{after300,after600,minsq8,minsq10}.toml`.
