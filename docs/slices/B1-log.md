# B1 log (balance pass; at most 150 lines)

Plan `docs/slices/B1-balance.md`, targets `targets/B1.toml`. Branch `balance-pass` from main `72b2970`. Commit only
your files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)
**B1-3 Diagnose done (2026-11-15); no default or sim change.** Noise rule: about 35-45 of 160 matches change winner
under any change; call nothing an effect below z 2.5, z = (toDE - toUS) / sqrt(toDE + toUS), paired by (seed, side).
Fresh seeds 1001-1280 only (1-280 searched, 301-380 held out). Germany % base 43.33 on blocks 1001/1101/1201 (480).
- Screen + confirm, 480 fresh: works_off +7.29 (105/70, z 2.65); builders_max 1 +3.96 (36/17, z 2.61); triple
  +6.25 (83/53, z 2.57); works_per_point 2 +1.88 (z 0.9); reach 120 +2.92 (z 1.6). Failed triple: on seeds 1-280
  42.29 -> 39.79 (64/76); over all 960 42.81 -> 44.69 (147/129, about 1 sigma). Applied 22ca58c, reverted b477036.
- Work kinds one at a time (480, `B1-levers/diag_*_off.toml`): nest +3.33 (72/56, z 1.4), dugout +1.88 (z 0.9), roof
  +1.04 (z 0.6), wire -0.83 (z -0.4), sandbags -1.67 (z -1.6). None alone; each keeps the builder share at 9-10 %.
- Findings (per-side, paired over 480 with `--side-detail`): works_off cuts the builder plan share 9.5 -> 3.9 % (DE)
  and 8.9 -> 3.9 % (US); the gain is all in matches that ended before 20 min in base (net +33 of +35; under 15 min
  17/4); 20 min or more 38/36. Kill share does not move (DE 44.1 -> 43.7 %); both sides fire and kill more with
  rifles (DE +2.8 rifle kills z 4.1, US +3.0 z 3.1); point hold time DE +60 s, US -63 s (z 1.2).
- Ruled out: (a) MP on works is 1 % of income (DE 59, US 48 MP a match; works_off saves DE 5.5 MP net); (b) as a share
  (builder plans 9.5 vs 8.9 %); (d) works are not where men die: deaths within 40 m of enemy works or 10 m of enemy
  wire are 0.00 (DE) and 0.06 (US) of 54 / 42 a match (block 1001, new `deaths` counter); (e) no faction branch in
  ai/fort.rs, ai/obstacles.rs; the cards differ only in the start engineer (Pioniergruppe 2 MP40 + 6 Kar98k vs 7
  Garands + M3). (c) Germany builds 30-50 % more works (2.0 vs 1.5, jobs 4.4 vs 3.2) at every length; not tested alone.
- Not singled out. Likeliest two: (1) the F15 builder hold (`fort_holds` + the +80 assign bonus in `ai.rs` `assign`)
  keeps engineers home in the opening, where Germany loses its short matches; (2) fort job time itself (any job, held
  or not; the job's squad leaves `mine`). L1 (fewer jobs AND fewer holds) helps; sub-switches (same jobs) do not.
- Separating run = the proposed fix (Opus Build): new key `[ai.fort] hold_max` (default 2 = today, bit-identical),
  `crates/sim/src/ai/fort.rs` `fort_holds`: stop when `held.len() >= hold_max` (before the `job_room` check). Jobs
  still start for free engineers. 480 fresh paired: hold_max 0 vs base. If it gives about works_off (z >= 2.5) with
  the T94-T97 floors kept, (1) is it; set 0 or 1 by the same run. If about 0, (2): use builders_max 1 (a setting).
  Same rule for both sides. Control: hold_max 2.
- T115c (13 own_side_mistakes, `b1-tuned` oos): all 13 are `own_blocked` in ONE match (seed 104 normal, Germany at
  base 0, 3 obstacles built). Elsewhere: base 1001 1 match (8), works_off 10 matches, all at base 0 (Germany normal or
  USA swapped): a latent S10b-ai map-side bug (own obstacle rows cut an own vehicle route at base 0), seen when freed
  engineers build more obstacles; inflated, as a (vehicle, piece) pair recounts each time it reappears. Report only.
Last check: tools/check.sh quick 72b2970 all ok (824 tests), after the `--side-detail` CLI change (B1-3 section).

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
