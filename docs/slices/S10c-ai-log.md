# S10c-ai log (defences at the front; at most 150 lines)

Spec: common `docs/slices/S10c-ai.md`, stages `S10c-ai-build-A.md`, `-B.md`, `-C.md`; targets `targets/S10c-ai.toml`.
Branch `claude/project-thread-e7nsak` from main `54009f6`. Another agent may work in this checkout: commit only your
files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)
**Build B chunk 1 done (2026-11-16; `571e841`; quick 54009f6: all ok, 847 passed). STOP POINT 1. Next: B chunk 2.**
- Neutral: `neutral.py target/s10c-baseA40 target/s10c-B1off40` (this binary + `c_off`) -> `same 40/40`.
  Front-on set (`[ai.obstacles] front = 1` override; default still 0): `target/s10c-B1on40`.
- Code map (`ai/fort.rs`): `WorkKind::FrontAp/FrontWire` (`fort_of`: AP / wire blueprint; `switched_on`: `front` and
  `mines` / `[ai.fort] wire`); `front_on`; Planner `front_lane`, `front_on_points`, `point_order` (front points
  first, else 0..n), `front_kind(job)` (a job is FrontAp/FrontWire when its line ENDS within 2 m of the lane line's
  end; any AT job at a lane's point is a front AT field), `front_live`, `front_fields` (F7, from lanes),
  `front_lanes_update` (in `fort_points`: drop, then search on recheck plans), `front_lane_search` + `lane_line`
  (F2/F3), `front_wants`, `wants_at(i, trench_done)` (F4; used by `fort_points`, `fort_holds`, `point_starts`),
  `front_carry_ok` (F10), `front_work`, `front_mark_done` (sets `ap_done`/`wire_done` when a job ends from Watch with
  its work gone and its mines / wire near the line end), `front_why` + `front_block`. `point_site` for FrontAp,
  FrontWire, AtField, AtRow2 no longer needs the trench frame. `front_jobs` += 1 in `point_starts` on a start.
- For chunk 2: (1) F5: `can_take` and `fort_continue` still use `fort_threat` (80/80) for front jobs, and front points
  still need `p.threat < build_threat_max` (`point_starts`, `fort_points`) and drop on it (`lost`); `front_block`
  reports that gate as `threat_site`. (2) F6: a re-started FrontAp gets the WHOLE saved line; the resume line must
  keep `lane.ap[1]` as its end or `front_kind` stops matching. (3) `front_drops`, `front_lost`, `front_resumes` are
  still 0. (4) FrontWire is an S10a work (S10a budget, outside `jobs_max`, no L1-L4); its Order retry is off.
- Early 40 set, front on (DE / US): Germany 18 / 40 (45 %; `c_off` 20 / 40); `matches_with_front_field_pct` 92.5 /
  62.5; `mines_home` / `mines_front` 667 / 140 and 538 / 50 (`front_mines_share_pct` 17.3 / 8.5): the home point is
  one of the two front points most of the match, so most "front" fields lie at B or D (`mines_by_point` DE [87,352,1,
  315,52], US [11,231,0,307,39]); `matches_mines_approached_pct` 13.5 / 7.4; `mines_set_off_per_match` 0.15 / 0;
  `front_jobs_per_match` 4.0 / 3.1; `front_drop_pct` 0 / 0 (not counted yet); `wire_seen_by_enemy_pct` 11.1 / 3.7;
  `defence_job_fail_pct` 3.0 / 15.7. `front_why` (plan x point): no_builder 20344 / 22544, slot 5901 / 5079,
  lane_none 3819 / 3622, no_front 2001 / 2123, no_carry 315 / 442, builders_max 466 / 206, threat_site 215 / 124,
  refused 0 / 594, cap 2 / 2, threat_squad 5 / 2. Not yet known: how much of lane_none is the 60 s recheck wait and
  how much failed searches (8000 expansions); measure before C.

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
