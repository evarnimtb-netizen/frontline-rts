# B1 log (balance pass; at most 150 lines)

Plan `docs/slices/B1-balance.md`, targets `targets/B1.toml`. Branch `balance-pass` from main `72b2970`. Commit only
your files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)
**B1-1 Screen done (2026-11-15), measuring only; no mods/ or sim change.** Targets fix: T114h got its control
(`fortify10...demolish.handover0.gap_opened_pct`); T88b, T88d are report rows (owner Q4). `frontline-cli targets`
parses: on `target/b1-base` 263 pass, 36 miss (21 hard). Fresh seeds 1001-1080 both bases, 160 matches per run, one
override file each in `docs/slices/targets/B1-levers/`, dirs `target/b1-screen/<name>/`, paired by (seed, side),
`target/b1-screen/cmp.py`. Noise of one 160 set +-7.7; paired net flips under about 8 are noise.
Run | Germany % | vs base | flips to DE / US | T94-T97 floors (works, wire >= 60, builders <= 12)
- base (defaults) | 41.88 | - | - | ok (US wire 61, near the floor)
- cause (a) all four [ai.fort] switches off | 32.50 | -9.38 | 28 / 43 | wire 0: FAIL
- cause (b) works off only | 51.25 | +9.38 | 37 / 22 | FAIL (wire 0, US works 42)
- L1 builders_max 1 | 45.62 | +3.75 | 12 / 6 | ok
- L2 works_per_point 2 | 45.62 | +3.75 | 21 / 15 | ok; 1: 41.88, 21 / 21, wire 0 FAIL (wire needs the 2nd work)
- L3 build_threat_max 3 and 2 | 41.88 both | 0 | 0 / 0 | ok; inert (no winner changes at all)
- L4 builder_reach_m 160 / 120 | 42.50 / 45.62 | +0.62 / +3.75 | 14 / 13 and 18 / 12 | ok
- L5 line 60 / 70 / 60 + engineer 10 | 38.75 / 28.75 / 32.50 | -3.12 / -13.12 / -9.38 | 16 / 21, 19 / 40, 16 / 31 | 70 and 60+10 FAIL
- L1+L4(120) 43.12 (+1.25, 18 / 16); L1+L2(2) 45.62 (+3.75, 21 / 15); L2(2)+L4(120) 45.62 (+3.75, 22 / 16);
  all three 46.25 (+4.38, 24 / 17, net +7), floors ok (US wire 62, builders 6.7-7.4 %).
Cause check: works off alone does reproduce the gap (51.25 vs 41.88, same size as the old 47.08 vs 41.7 and net +15
flips, about 2 sigma). All four off does NOT (32.50): cards or breach off costs Germany more than works gain (not
split; the old all-off 48.75 was before S10b). Both end at floors that fail, so the cause is real but not a setting.
Reading: nothing separates from noise alone. The best single levers (L1, L2=2, L4=120) each give +3.75 (net +6 flips,
about 1 sigma of the paired count); together +4.38. Direction is consistent (all three help, L5 hurts clearly), but
the three together reach about 46 %, which is inside 45-55 only barely. L5 (more rifle squads) lowers Germany: drop it.
**Next (B1-2):** autotune on `builders_max` 1-2 is a coin flip (two values); better a joint run with
`works_per_point` 2-3 and `builder_reach_m` 120-200 around the triple (L1=1, L2=2, L4=120), target share 47-53,
seed_start 1101; expect the solved level near 46-47, so a margin is thin. If the autotune triple does not reach 47,
the code fix of the builder hold (owner Q1 C, Opus Build stage) is the next step: works off alone is +9, so most of the
gap sits in the hold. Do not use L3 (inert), L2 1 (kills wire) or L5.
Last check: tools/check.sh quick 72b2970 (see below). Not run: the fort floors in the pool pass (screens read per 160 set).


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
