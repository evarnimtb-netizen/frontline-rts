# S10c-ai log (defences at the front; at most 150 lines)

Spec: common `docs/slices/S10c-ai.md`, stages `S10c-ai-build-A.md`, `-B.md`, `-C.md`; targets `targets/S10c-ai.toml`.
Branch `claude/project-thread-e7nsak` from main `54009f6`. Another agent may work in this checkout: commit only your
files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)
**Build A chunk 1 done (2026-11-16, `2651354`; quick 54009f6: all ok, 834 passed). Next: A chunk 2 (items 4-7).**
- Layout: `AiCounters` += the 14 fields of spec 3 (`front_why: [u32; 12]`, `mine_want_why: [u32; 8]`); names
  `ai::FRONT_WHY`, `ai::MINE_WANT_WHY` (re-exported like `BUNKER_WHY`). `[ai.obstacles]` += 12 keys at their off
  values (key count 49); setup-bytes BASELINE and canonical snapshot re-recorded for them only. Counters enter the
  state hash (postcard of `AiState`), so hashes differ from main; commands and outcomes do not. `save_fuzz.rs` mirror 26.
- Filled (no new behaviour): `fort_job_fails` (`s10b_end` failed), `truck_give_ups` (`truck_give_up`, only with
  `refusal_memory` 1, as its `s10b_refused`), `route_refusals` (`own_route_side`, = `own_route_rejects`: every R6
  reject writes L3). `s10b_refused` unchanged. `front_plans` / `front_calm_plans`: `Planner::front_count` every plan,
  right after `fort_points` (any switch value; calm = `fort_threat` of known men and vehicles at the front points).
  `mine_want_why`: `Planner::mine_want_why` / `mine_block` in `ai/fort.rs`, called in `fort_start` after
  `bunker_why` (inside the B1-5 build gate). Own points only; not counted: laid, a running job of that blueprint at
  the point, AT off `recheck_s` plans, or nothing blocks. Check order (index): point not trenched or
  `build_threat_max` (0), AP waits for wire (1), not in `field_wants` (2), no site (3), `job_room`/`s10b_room` (5),
  `under_limit` (2), refused (4), no fit free assigned builder (6), all fit builders or site threatened (7).
- F1 helper: `fort::front_points_of(g, me)` (free fn, testable) and `Planner::front_points`.
- Tests: content `load_rejects_bad_s10c_keys` (`s10c_ai_content.rs`); sim `s10c_ai_tests.rs`:
  `s10c_refusal_split_adds_up`, `ai_front_points_are_nearest_the_enemy_base`, `mine_want_why_names_the_wire_gate`,
  `s10c_ai_layout_roundtrips_and_hashes`. Every control seen failing (main path switched to the control value:
  front_points 0 -> `[]`; wire built -> wire_first 0; refusal_memory 0 -> truck scene [0,0,0,0]; an edit `+= 0` ->
  "edit 0 is not hashed"; `front_points: 0..=9` -> "front_points = 9 was accepted"). `fort_row`, `truck_row` made
  `pub(crate)` in `s10b_ai_tests.rs`.
- Deviations: the refusal split uses three staged worlds (fort row, truck row, the `ai_never_blocks_own_route`
  scene), summed, not one world (each needs its own staging). `crates/simbench/src/report.rs` has no setup-bytes
  BASELINE (B1-4/B1-5 did not touch it); nothing changed there. `front_fields_max` range 0..=16 (spec 6 names none).
- For chunk 2 / orchestrator: `tools/ablate.sh` check_file accepts only S10a/S10b switch keys; it will refuse the
  S10c files (`front`, `jobs_max`, `builder_cards_min` 2, `front_site_threat_m` 80) and its default name list is
  `*_off.toml` only. Extend it before `ABLATIONS=docs/slices/targets/S10c-ai-ablations` runs.
  `target/s10c-base40` (a `54009f6` 40 set) is needed for chunk 2 item 6; neutrality is measured there (no new code
  touches RNG, order or commands). Gate (spec 8): `front_calm_pct` < 10 % stops B. S10c-1..5 not yet in TODO.

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
