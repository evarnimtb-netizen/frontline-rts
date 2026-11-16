# S10c-ai log (defences at the front; at most 150 lines)

Spec: common `docs/slices/S10c-ai.md`, stages `S10c-ai-build-A.md`, `-B.md`, `-C.md`; targets `targets/S10c-ai.toml`.
Branch `claude/project-thread-e7nsak` from main `54009f6`. Another agent may work in this checkout: commit only your
files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)
**Build B done (2026-11-16; chunk 2 `71b8bdd`; quick 54009f6: all ok, 854 passed). STOP POINT 2. Next: Build C.**
- Defaults now: `[ai.obstacles] front` 1 (item 10), `front_home` 0 (orchestrator, below); `c_off` sets both 0.
  Neutral: `python3 -I tools/neutral.py target/s10c-baseA40 target/s10c-B2off40` -> `skirmish40.json: same 40/40`.
  Sets: default `target/s10c-B40`, `front_home_on.toml` `target/s10c-B2home40`, `c_off` `target/s10c-B2off40`.
- Code map additions (`ai/fort.rs`): `front_threat` (F5; `Some(true)` site, `Some(false)` builder) used by
  `can_take_at(.., front)` and `fort_continue`; the point's `build_threat_max` gate now keeps only non-front works
  (`fort_points`, `fort_holds`, `point_starts`, the drop); `front_rest` (F6 resume line, also the done check in
  `front_lanes_update`); `front_drops` / `front_lost` / `front_resumes` in `fort_continue` / `point_starts`;
  `front_points_of` applies `front_home` only with `front` on (so `front_plans` stay Build A's under `c_off`).
- For C: `front_points` 2, `front_out_m` 50, `front_site_threat_m` 50 (F5), `front_lane` = foot path from the nearest
  enemy point; lanes never failed (10 matches: 119 searches, 0 failed), so `lane_none` is all the 60 s recheck wait.
- **Home works floor missed (spec 8.4, hard in C):** MG nests done a match DE 0.825 -> 0.35, US 0.725 -> 0.225; US
  `matches_with_works_pct` 92.5 -> 80.0 (`c_off` -> default). The one builder squad lays front fields instead (front
  jobs take the S10b slot, `slot` is the 2nd `front_why`); C1/C8 (`jobs_max` 2, `front_slots` 1) must win it back.
- `no_builder` is ~70 % of `front_why`: assign gives a held builder +80 at its hold point but also +80 at EVERY own
  untrenched point, so a central point (C) outbids the front point (seen in `ai_front_fields_survive_a_lost_point`);
  C2 `mine_assign` is the planned answer; consider a hold bonus above the dig bonus for front points.
- `front_home` 0 did not move mines off B / D: `mines_home` grew (DE 155 -> 555, US 91 -> 385): early in a match
  the home point is the only own point and so the front. `own_blocked` DE 32 in the default set (1 with
  `front_home` 1): the B1-2 recount (C6) shows up there; not looked into.
- Mines approached in 11.1 / 17.9 % of matches with mines (DE / US): above the 10 % alarm, so the lane source stays.
- Default set (DE / US), Germany 50.0 % (`c_off` 50.0): front field matches 70.0 / 65.0 %; front share of mines
  12.7 / 18.1 %; set off 0.075 / 0.25 a match; enemy wire seen 25.8 / 11.5 %; front jobs 3.3 / 2.65 a match, drops
  24.2 / 23.6 %, builders lost 0.30 / 0.175 a match; breaches 0 / 0; sweeps wanted 276 / 114. Full rows below.

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
- Layout commit `6cd959c` (spec 11 items 1-2): `front_lanes`, `front_slots`, `front_wire_len_m` 20; SAVE stays 26.
  BASELINE and snapshot re-recorded for the two keys only. Sim suite 692 passed, content all passed.
- Observers (item 4) and JSON (item 5), `neutral.py` skip (item 6), `ablate.sh` S10c files: `233f9b7`.
  `tools/check.sh quick 54009f6`: all ok (837 passed, 0 failed).
- Wall time, 40 set, `BATCH_JOBS=4`, `BATCH_NO_CACHE=1`, same machine, back to back: before (observer call switched
  off) 114.7 s; after 114.4 s (defaults), 115.1 s (`c_off`): +0.0 % to +0.4 % (limit +5 %).
- Tests and their controls (each seen failing): `load_rejects_bad_front_lanes` (the S10c lines of `validate_fort` made
  true -> "over the cap" loads); `s10c_observer_counts_an_approach` (built-in control 60 m -> 0; also seen failing
  with `S10C_NEAR_M` 10: left 0); `s10c_json_names_match_the_targets_file` (`knew_enemy_wire_pct` renamed -> "T123a:
  ... is not written"); `test_neutral.py` `test_s10c_keys_skipped_only_when_base_lacks_them` (skip removed -> fails;
  an `x10c` block is the in-test control).
- First 40-set attempt panicked in `S10cSeen` (works-done list indexed past its length when an account's
  `works_done` was shorter than the blueprint index); fixed before any number above was read.
- Deviations: `S10cSeen` places the enemy squad 20 m from the field (`spawn_squad_at`) instead of walking it there.
  A mine laid and gone within one sampled second is not seen (DE 155 seen = 155 laid; US 91 = 91 here).
  Works done are matched to the work that vanished in the same second with its owner's `works_done` risen (all 134 /
  106 placed). Extra raw keys beyond spec 7 (`mines_seen`, `mines_approached`, `mines_set_off`, `s10b_jobs`, the
  `matches_*` tallies). `matches_wire_approached_pct` is over matches with wire; `matches_obstacle_approached_pct`,
  `knew_enemy_wire_pct`, `matches_with_belt_pct`, `matches_with_front_field_pct` over all matches (null only with 0
  matches). Approach counts soldiers on foot (not riding), as a mine trigger does. `c_off.toml` also lists
  `front_wire_len_m` 20 and `front_slots` 1 (addendum 2).

## Build B chunk 1 (2026-11-16)
- Commit `571e841`: lanes, sites, wants, starts (F1-F4, F7, F10 carry rule, `front_why`, `front_jobs`).
  `tools/check.sh quick 54009f6`: all ok (847 passed, 0 failed), after the release CLI rebuild.
- Neutral: `c_off` 40 set into `target/s10c-B1off40`; `python3 -I tools/neutral.py target/s10c-baseA40
  target/s10c-B1off40` -> `skirmish40.json: same 40/40`.
- Tests (`s10c_ai_front_tests.rs`) and controls: each has a built-in control whose run asserts the opposite outcome
  (the main assertion fails under it): lane (no enemy point -> from the base, not across C -> E), lays without a
  trench (`front` 0 -> 0 mines on the line, no lane), before the nest (`front` 0 -> `mg_nest` first), caps
  (`front_fields_max` 9 -> the job; 2 and the 60-mine blueprint cap -> none, `front_why` cap), every plan (lane removed
  -> no job before the 60 s recheck, one after), empty builder (60 kg -> job; 0 kg -> none, `no_carry`), deadline
  (`job_max_s` 120 -> not laid), detour (`front_wire_len_m` 40 -> leader crosses the row line > 15 m off the lane),
  way out (wire 60 m at 10 m out -> longer by > 15 m or through it). Two builders: control by code edit (the live-job
  check removed from `front_wants`) -> "two jobs on one front site: 2 AP, 0 wire", seen.
- F10: 900 s held: from base about 590 m away the FrontAp job took 312 s (load, walk, 15 mines); front kinds keep
  `job_max_s`.
- Deviations: (1) the test squads are US engineers (player 1 of the test world is USA), not Pioniere. (2) `front_jobs`
  counts in `point_starts` after `new_fort_job` (it knows the kind). (3) The lane test checks the lines against the
  foot path itself: A* legs are grid-aligned, so the lane toward E is not the straight line. (4) The way-out test
  makes the other of B/D the AI's so `route_out` is the lane's source (with both at 300 m it chose the point
  behind); its control (60 m wire at 10 m) is mine, the spec names none. (5) `front_why` counts the first
  outstanding front kind per front point, none while a front job runs there, `no_front` once a plan with no front
  point; `site_bad` = no site line. (6) The deadline test keeps only C on the map: with other points the builder
  went to capture D first (assign), then took an R1 job at C. (7) No detour finding: at 20 m the leader crossed the
  mine row within 15 m and a mine went off; the wire was not shortened.

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
  `front_home_on`, Germany 47.5 %: front field 90.0 / 65.0 %; 590/93, 431/32 (13.6 / 6.9 %); approached 8.3 / 7.1 %;
  set off 0.15 / 0.125; wire seen 19.4 / 3.4 %; knew 2.5 / 17.5 %; breaches 0 / 0; jobs 3.63 / 2.58; drops 19.3 /
  20.4 %; lost 0.30 / 0.125; why no_builder 20563 / 22806, slot 4360 / 4287, lane_none 3706 / 3374; works 97.5 /
  80.0 %; nests 0.225 / 0.175.
- lane_none split (instrumented binary, not committed; 10 matches, seeds 1-5 both bases): 1780 lane_none plans, all
  before the first recheck plan after a point became front; 119 lane searches, 0 failed.

