# S10c-ai log (defences at the front; at most 150 lines)

Spec: common `docs/slices/S10c-ai.md`, stages `S10c-ai-build-A.md`, `-B.md`, `-C.md`; targets `targets/S10c-ai.toml`.
Branch `claude/project-thread-e7nsak` from main `54009f6`. Another agent may work in this checkout: commit only your
files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)
**Build A done (2026-11-16; `2651354`, `6cd959c`, `233f9b7`; quick 54009f6: all ok, 837 passed). STOP POINT 2.**
**Next: the orchestrator's gate (spec 8), then Build B.**
- Neutral: `neutral.py target/s10c-base40 target/s10c-baseA40` -> `skirmish40.json: same 40/40 (note: overrides 0 -> 1 items)`.
  `target/s10c-A40` = this binary, defaults; `target/s10c-baseA40` = this binary + `c_off` (the base for B and C).
- Gate numbers, `target/s10c-A40` (40 matches; DE / US): `front_plans` 22923 / 22784; `front_calm_pct` **87.8 / 88.9**
  (an UPPER bound, spec 8 finding 10: 80 m round the point; gate < 10 % not hit). `mine_want_why` (plan x point counts):
  not_own_or_trench 37143 / 36750, no_builder 7117 / 7791, slot 3373 / 2789, wire_first 3120 / 3842, refused 1 / 0,
  no_site 0 / 6. `mines_home` / `mines_front` 155 / 0 and 91 / 0 (`front_mines_share_pct` 0); `mines_by_point` DE
  [0,90,0,65,0], US [0,40,0,51,0] (B and D only). `matches_mines_approached_pct` 0 / 0 (of 11 / 8 matches with mines);
  `mines_set_off_per_match` 0 / 0; `wire_seen_by_enemy_pct` 3.2 / 0 (of 31 / 28 matches with wire);
  `knew_enemy_wire_pct` 0 / 2.5; `defence_job_fail_pct` 10.2 / 9.5 (5 of 49, 4 of 42); `truck_give_ups` 23 / 16,
  `route_refusals` 4 / 0; `matches_obstacle_approached_pct` 0 / 0; front_why all 0 (Build B fills it).
- Home works floor (spec 8.4, addendum 4), `target/s10c-baseA40`: MG nests done 33 / 29 (0.825 / 0.725 a match),
  `s10_derived.matches_with_works_pct` 95.0 / 92.5.
- Code map (chunk 2): `ai::FrontLane`, `MAX_FRONT_LANES` (`ai/fort.rs`), `AiState::front_lanes` (checked in
  `validate_fort`); keys `front_slots` 1, `front_wire_len_m` 20 (key count 50). CLI `skirmish.rs`: `S10C_AI`,
  `S10cPlayer` (`PlayerReport::s10c`), `S10cSeen` (`observe` once a second + at the end, `finish`), `S10cFaction`
  (`FactionSummary::s10c`); `json_out::s10c_blocks` -> `factions.<f>.s10c` / `s10c_derived`; `--side-detail` sides
  carry the same raw `s10c`. `front_field` reads `front_lanes[].ap_done`; `matches_with_belt` reads `belt_jobs` > 0.
- For B: fill `front_lanes` (F2) and set `ap_done`; count `front_jobs`, `front_drops`, `front_lost`, `front_resumes`,
  `front_why`. Compare against `target/s10c-baseA40` with `neutral.py` (both now carry `s10c`, so it is compared).
- Ablations: `ABLATIONS=docs/slices/targets/S10c-ai-ablations tools/ablate.sh` takes every `*.toml` there; `c_off` is
  the neutrality control (NEUTRAL_BASE default `target/s10c-base40`); `check.sh quick` dry-runs both sets.
- Targets: every `s10c*` path of `S10c-ai.toml` resolves (copies of A40 as the 160 runs: 0 "no such number").
  T124a-d PASS (report); T125 PASS but unexposed; T119-T122 UNEXPOSED; T123, T126, T127 MISS (report).
- S10c-1..5 not yet in TODO (orchestrator).

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
