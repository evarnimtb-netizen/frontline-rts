# S10c-ai log (defences at the front; at most 150 lines)

Spec: common `docs/slices/S10c-ai.md`, stages `S10c-ai-build-A.md`, `-B.md`, `-C.md`; targets `targets/S10c-ai.toml`.
Branch `claude/project-thread-e7nsak` from main `54009f6`. Another agent may work in this checkout: commit only your
files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)
**Build C chunk 1 done (2026-11-16; quick 54009f6: all ok, 861 passed). STOP POINT 1. Next: Build C chunk 2 (belts, C4/C5/C7).**
- Defaults now: `front` 1, `front_home` 0 (new meaning, F1), `jobs_max` 2 (C1), `front_slots` 1 (C8, now enforced),
  `mine_assign` 1 (C2, turned on here so the `mine_assign_off` ablation means something), `builder_cards_min` 0
  (C3; `builders2` ablation). `c_off` neutral: `python3 -I tools/neutral.py target/s10c-baseA40 target/s10c-C1off40`
  -> `skirmish40.json: same 40/40 (note: overrides 1 -> 3 items)`. The stage file's "all C keys off = `target/s10c-B40`"
  no longer holds: the F1 change moves the `front`-only run on purpose. Sets: default `target/s10c-C1a40`,
  `builders2` `target/s10c-C1b40`, `mine_assign_off` `target/s10c-C1c40`, `c_off` `target/s10c-C1off40`.
- **Home works floor still missed at defaults** (limit: nests >= 0.7425 / 0.6525 a match, works >= 85.5 / 83.25 %):
  default DE nests 0.70, works 87.5 %; US 0.525, 77.5 %. `mine_assign_off`: DE 0.775 / 97.5 % (passes), US 0.55 /
  77.5 % (misses: the US loss comes with `front` itself, B40 had US works 80.0 %). `builders2` passes both (DE 0.90 /
  100 %, US 0.80 / 95.0 %). So: `mine_assign` costs DE nests (one builder goes to the front, F4 visits front points
  first); the US miss needs a second builder. Chunk 2 must not turn `belt` on while this stands (stage file rule);
  the orchestrator decides: `mine_assign` 0, or `builders2` (owner S10c-2), or a rule "a front pick never takes the
  last free builder while a home work is wanted" (not built; proposed).
- Front now works: front share of mines DE 52.7 / US 40.6 % (B40 12.7 / 18.1), mines approached 42.9 / 20.0 %, set off
  0.475 / 0.25 a match, enemy wire seen 62.1 / 34.8 %. But mines in only 70.0 / 50.0 % of matches (B40 90 / 70): with
  the new F1 the opening no longer lays front fields at the home point. Germany 40.0 % (c_off 50.0; 40-set noise 11-15).
- B1-2 (C6): recount fixed (a pair stays in `blocked_pairs` while vehicle squad and piece live). Cause found, not
  fixed: see the chunk 1 section. Proposed rule R6b below; it needs a key (c_off neutrality).
- Code map (chunk 1): `fort.rs` `front_slot_free` (C8; used by `point_starts`, `fort_holds`, `front_block` -> `slot`),
  `mine_assign_pick` (C2; after the assigned pick in `point_starts`), `front_block` reads all of `mine` with
  `mine_assign` on; `ai.rs` `builder_floor_card` + `urgent_builder` in `buy` (C3); `count_own_blocked` keeps pairs.
- For C4: `front_kind` counts an AtField job at a lane point as front, so a Belt kind must join `front_kind` to fall
  under `front_slots` (C8 names it). `builders_max` 2 caps all builder jobs: three jobs need `builders_max` >= 3.

## Plan (2026-11-16)
Cause chain read in the code (spec 0): `trench_done` gate, the nest -> wire -> AP order, `fort_threat` (one known
enemy within 80 m of builder or site), trench-frame sites, about one builder squad a match, `jobs_max` 1, 12 m rows.
Design choices: front = the two own points nearest the enemy base (no map analysis); lanes by one path search per
point per `recheck_s` (foot: from the nearest enemy-owned point; vehicle: R2 unchanged); hasty AP field then wire then
AT field with no trench needed and ahead of the nest; the site's threat radius 50 m for front jobs only, the builder's
stays 80 m; a threat drop is not a refusal and the rest of the line is wanted again; belts block the lane beyond the
AT field so the way round crosses the mines; own-route safety for belts by a detour check instead of R6's 10 m.
Not designed in (said in spec 1): laying under fire, chokepoint analysis, a detour-cost demolition want.
Honest limits: on the open map a 20 m belt never leaves "no way round", so R14 demolitions stay rare (T126 is a
report); with one builder squad per match the front work competes with the rear works (F4 puts the front first;
`builders2` measures a second squad); B1 found that fortifying helps the USA more, so the guard band is watched at
every stage end.
Layout: only `AiCounters` fields and setup keys (SAVE 26 in Build A); lanes are a per-plan memo; no new job type.

## Build A chunk 2 (2026-11-16)
- Layout `6cd959c` (`front_lanes`, `front_slots`, `front_wire_len_m` 20; SAVE 26); observers, JSON, `neutral.py` skip,
  `ablate.sh` files `233f9b7`. Quick all ok (837). Observer cost on the 40 set +0.0 to +0.4 % (limit 5 %).
- Tests with controls seen failing: `load_rejects_bad_front_lanes`, `s10c_observer_counts_an_approach` (60 m -> 0),
  `s10c_json_names_match_the_targets_file`, `test_neutral.py` s10c skip. A panic in `S10cSeen` (works-done index) was
  fixed before any number was read.
- Deviations: the observer test places the enemy squad (no walk); a mine laid and gone within one sampled second is
  not seen; extra raw keys (`mines_seen`, `mines_approached`, `mines_set_off`, `s10b_jobs`, `matches_*`); approach
  counts soldiers on foot; `c_off.toml` also lists `front_wire_len_m` 20 and `front_slots` 1.

## Build B chunk 1 (2026-11-16)
- Commit `571e841`: lanes, sites, wants, starts (F1-F4, F7, F10 carry rule, `front_why`, `front_jobs`). Quick all ok
  (847). Neutral: `c_off` `target/s10c-B1off40` vs `target/s10c-baseA40` same 40/40.
- Tests (`s10c_ai_front_tests.rs`), each with a built-in control asserting the opposite: lane (no enemy point -> from
  the base), lays without a trench (`front` 0), before the nest (`front` 0 -> `mg_nest`), caps, every plan (lane
  removed -> after the recheck), empty builder (0 kg -> `no_carry`), deadline (`job_max_s` 120), detour
  (`front_wire_len_m` 40), way out; two builders by code edit. F10: a FrontAp job from a base 590 m away took 312 s.
- Deviations: US engineers in tests; `front_jobs` counted after `new_fort_job`; lane test against the foot path; the
  way-out test owns the other of B/D; `front_why` counts the first outstanding kind per front point; the deadline
  test keeps only C; no detour finding (at 20 m the leader crossed the row and a mine went off).

## Build B chunk 2 (2026-11-16)
- Commit `71b8bdd`: F5, F6, F8 ends, determinism, liveness row, fuzz stage, `front` 1, `front_home`. SAVE 26 (the setup
  gains one key, no bump: nothing released since 26), PROTOCOL 14. Quick 54009f6: all ok (854 passed).
- Orchestrator decision 2026-11-16: `[ai.obstacles] front_home` (0/1, default and `c_off` 0): with `front` on, the
  home point (map point nearest the own base) is a front point only as the only own point; 1 = plain F1 (ablation
  `front_home_on.toml`). Spec F1 row updated; tuning.rs, skirmish.toml, tuning.md, key count 51, BASELINE, snapshot.
- Tests and controls (each seen failing): `ai_front_points_are_nearest_the_enemy_base` home case (control
  `front_home` 1 -> [C, B]); `ai_front_job_survives_an_enemy_60_m_from_the_site` (control `front_site_threat_m` 80 ->
  dropped; code edit: front jobs back on `fort_threat` -> "alive" fails); `ai_front_job_resumes_after_a_threat_drop`
  (control: `ap_done` set at the drop -> 6-7 mines; code edit: resume from the whole line -> 22 mines, fails);
  `ai_front_field_set_off_is_not_relaid` (control `ap_done` cleared -> relaid); `ai_front_job_ends_when_the_point_is_
  lost` (control point kept -> alive); `ai_front_builder_dies_mid_field` (control one man killed -> goes on; code edit
  `front_lost` not counted -> fails); `ai_front_fields_survive_a_lost_point` (control `front_points` 2 -> cap);
  `ai_front_deterministic_threads_and_save_load` (1/2/3/8 threads over 12:30; saves at 8:24 and 12:00, the first
  seconds from 5:00 / 12:00 with a front job live; control: a lane edit changes the hash); liveness row FrontAp
  (control `refused_s` 0 as the other rows); fuzz `staged_s10b_ai + a FrontAp job in Watch with 5 mines laid`.
- Deviations: (1) the threatening enemy in the F5/F6 tests is a player-0 truck (known vehicle, no gun), so builders
  are not shot; it is moved in and out with `place_vehicle`. (2) The F5 rule also lifts the point's `build_threat_max`
  gate for front works (spec F5: "the same rule"; chunk 1 note (1)). (3) Resume: a dropped squad's standing order
  can finish the mine in hand (control allows +1). (4) `ai_front_fields_survive_a_lost_point` shows the new front
  point's field by its want (`front_why` no_builder, not cap) and moves the front by taking B instead of losing C:
  a builder placed at the new front point was sent by assign to C (see `no_builder` above). (5) The liveness row
  blocks the site with an own tank (flat map: no 25 % slope), as the LogBarrier row. (6) Default on broke tests of
  main behaviour: S10a/S10b scene helpers (`s10b_ai_off`, lay `held`), the played save-fuzz match and the CLI
  `s10b_counters_zero_without_use` (`c_off`) take `front` 0; `ai_takes_the_map_from_an_idle_human` waits 8 min
  (the 4th point falls at 7:30, was before 6:00). (7) A set-off mine before `ap_done` is relaid by a resume (rule).
- 40 sets, `BATCH_JOBS=4` (DE / US). Default (`front` 1, `front_home` 0), Germany 50.0 %:
  front field 70.0 / 65.0 %; `mines_home`/`mines_front` 555/81, 385/85 (share 12.7 / 18.1 %); approached 11.1 / 17.9 %;
  set off 0.075 / 0.25; wire seen 25.8 / 11.5 %; knew enemy wire 7.5 / 20.0 %; breaches 0 / 0; front jobs 3.3 /
  2.65; drops 24.2 / 23.6 %; lost 0.30 / 0.175; why no_builder 17395 / 19556, slot 4837 / 3520, lane_none 3550 /
  3185; works 97.5 / 80.0 %; nests 0.35 / 0.225 a match (`c_off`: 95.0 / 92.5 %, 0.825 / 0.725).
  `front_home_on` (`target/s10c-B2home40`), Germany 47.5 %: front share 13.6 / 6.9 %; works 97.5 / 80.0 %; nests
  0.225 / 0.175. lane_none (10 matches): all before the first recheck plan; 119 lane searches, 0 failed.

## Build C chunk 1 (2026-11-16)
- Orchestrator additions (2026-11-16): (1) F1 with `front_home` 0: the home point is a front point only while the
  enemy owns more than half of the map's points; before that front points are the non-home own points only (spec F1
  row, `tuning.md`, `skirmish.toml`). (2) C2's pick reads every free squad (`mine`), not `assigned`, as `bunker_assign`.
  Hold bonus: it no longer decides the pick (test below: the builder's task stays at the home point and it still takes
  the front job), but it still acts around it: hunts and ambushes run before `fort_start` on assigned squads, and
  between jobs assign sends the builder back to the nearest own untrenched point (+80 at every one); no_builder fell
  only from 23209 / 25080 (`mine_assign_off`) to 16181 / 20804 (now = "no fit free builder anywhere").
- C1 `jobs_max` 2 (`skirmish.toml`, snapshot, content tests; BASELINE unchanged: it hashes the frozen S3c fixtures,
  only starting values in `tuning.rs` move it). C8 `front_slots` gate in `point_starts`, `fort_holds`, `front_block`.
  C3 floor: "a habit want exists" is read as fort habits on, build gate open and an own point (`buy` runs before
  `fort_points`; a want flag would be unsaved state). Card: cheapest roster card with `place_mines` and
  `build_structure`, no `breach`, `ai_weight` > 0 (S10a cards need `[ai.fort] cards`); no mix slot needed.
- Tests (each control seen failing by running the main assertion on the control run, or by the named code edit):
  `ai_two_s10b_jobs_run_at_once` (jobs_max 1 -> 1), `ai_front_jobs_leave_a_slot_for_home_works` (front_slots 2 -> 2
  front jobs), `ai_mine_assign_sends_a_builder_to_the_front` (0 -> no job, no_builder), `ai_buys_a_builder_under_the_
  floor` (0 -> first buy rifles), `ai_builder_floor_never_overbuys` (code edit: alive count ignored -> 3 buys),
  `own_blocked_pair_is_counted_once` (code edit: old replace-the-list -> 2), `ai_bunker_yields_a_slot_with_two_slots`
  (bunker_yields 0 -> 1 job beside the bunker; needs `builders_max` 4), F1 home case (`front_home` 1 -> [C, B]).
- B1-2 cause (C6b): seed 104 reads 0 on main now; the default B40 set had DE 31 in seed 15 normal. Replayed with
  9184e0a + a print: all 31 counts are one Opel Blitz (truck job Load / Return, goal = the own base centre) against
  two pieces of an own log barrier 2-3 m from the base centre (base 0); the pair came and went as the truck shunted
  (with the recount: 2). R6 checks only base -> point -> `route_out`; a route starting at the base centre would have
  refused a barrier 2 m from it, so `own_route` was most likely None (no vehicle path from the base centre), and R6
  lets every site through when the search fails. Proposed R6b (own key, off in `c_off`): no blocking piece within
  `own_route_m` + `wreck_pad_m` of the own base centre, and a failed route search refuses instead of passing.
- Deviations: (1) `mine_assign` default 1 in chunk 1 (stage file: chunk 2), so ablation (c) differs from (a).
  (2) Tests on a lone held point or C alone take `front_home` 1 (`ai_front_wants_come_before_the_nest`,
  `ai_front_job_from_base_ends_inside_its_deadline`, liveness row FrontAp). (3) The determinism test's second save is
  the first second with a front job live from the end of the first save's replay (seed 1 now has none from 9:52 to
  24:52; saves at 8:22 and about 8:52), window 15 min. (4) `yields_run` pins `jobs_max` 1. (5) The C6 recount has no
  key: it changes a counter only, and `c_off` reads `own_blocked` 0 (same 40/40).
- 40 sets (DE / US). Default `C1a40`, Germany 40.0 %: mines 70.0 / 50.0 %; home/front 218/243, 171/117 (front share
  52.7 / 40.6 %); approached 42.9 / 20.0 %; set off 0.475 / 0.25; wire seen 62.1 / 34.8 %; knew 20.0 / 45.0 %;
  breaches 0 / 0; front jobs 5.45 / 4.33; lost 0.60 / 0.55; why no_builder 16181 / 20804, slot 5390 / 3870, no_front
  4730 / 4995; builder buys 0 / 0; nests 0.70 / 0.525; works 87.5 / 77.5 %; wire 72.5 / 57.5 %; own_blocked 0 / 0.
  `builders2` `C1b40`, Germany 42.5 %: mines 95.0 / 82.5 %; 398/510, 342/285 (56.2 / 45.5 %); approached 63.2 / 51.5 %;
  set off 0.675 / 0.375; wire seen 45.9 / 54.1 %; knew 50.0 / 42.5 %; breaches 0 / 0; jobs 8.45 / 9.35; lost 0.65 /
  0.80; why slot 7070 / 6602, no_builder 5789 / 5704, no_front 4730 / 5001; buys 1.7 / 1.925; nests 0.90 / 0.80;
  works 100 / 95.0 %; wire 92.5 / 92.5 %; own_blocked 0 / 0.
  `mine_assign_off` `C1c40`, Germany 45.0 %: mines 45.0 / 30.0 %; 169/79, 140/21 (31.9 / 13.0 %); approached 22.2 /
  16.7 %; set off 0.075 / 0.05; wire seen 19.0 / 13.6 %; knew 7.5 / 10.0 %; breaches 0 / 0; jobs 1.95 / 1.40; lost
  0.30 / 0.225; why no_builder 23209 / 25080, no_front 4730 / 4995, lane_none 3440 / 3515; buys 0 / 0; nests 0.775 /
  0.55; works 97.5 / 77.5 %; wire 52.5 / 55.0 %; own_blocked 0 / 0.
