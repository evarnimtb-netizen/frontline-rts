# S10c-ai Build B: front rules (hasty fields and wire on the enemy approach lanes)

Read first: `S10c-ai.md` 0, 4 (F1-F9), 5 (first row); the log's top block (Build A's numbers and code map).
**2 chunks**, about 30 self-counted turns each. No layout change (SAVE stays 26). Work rules as in A
(`tools/check.sh quick 54009f6` once per chunk end). Everything new sits behind `[ai.obstacles] front`; with
`front` 0 the 40 set must equal `target/s10c-baseA40`.

Start from: `crates/sim/src/ai/fort.rs` (`fort_frame`, `point_site_at`, `at_site` / `at_site_search` and the
`at_sites` memo, `field_wants`, `point_wants`, `fort_points`, `fort_holds`, `point_starts`, `can_take`,
`fort_threat`, `fort_continue` (the drop at `fort_threat`, the `lost` rule), `s10b_end`, `run_fort_job` (a line goes
on from the first segment not standing), `field_has`, `fields_of`); `crates/sim/src/ai/breach.rs` (`breach_path`: how
the AI calls `find_foot_path` with the sim's own costs); `crates/sim/src/ai.rs` (`assign`: the engineer bonus reads
`trench_done` and `hold`; `order_infantry` `wants_dig`); `crates/sim/src/s10b_ai_lay_tests.rs` (scene helpers).

## Chunk 1: lanes, sites, wants, starts (F1-F4, F7)
1. Lane memo `front_lanes` on `Planner` beside `at_sites`: `front_lane(i)` = F2 (source: the enemy-owned map point
   nearest point `i`, none: the enemy base; `find_foot_path` as `breach_path`, wire cost off; only when
   `recheck_plan()`, else the last memo is not there: a front work is only *started* on a recheck plan, as R2).
2. Sites: `front_site(i, k)` walks the lane back from the point (the `at_site_search` loop) to `front_out_m`
   (`FrontWire`, length `front_wire_len_m`) or `front_out_m + ap_beyond_wire_m` (`FrontAp`, blueprint length); the
   line is perpendicular to the lane segment there, centred, clamped, at least 2 m. A lane shorter than the distance:
   no site (`front_why` lane_none). `fort_of` maps `FrontAp` -> the AP blueprint, `FrontWire` -> the wire blueprint.
3. Wants: `front_wants(i)` for `i` in `front_points()` with `front` 1: `FrontAp` (under `front_fields_max`,
   `fields_per_point`, the blueprint cap, no own AP mine or work within `SITE_NEAR_M` of the site line), then
   `FrontWire` (no own wire section or work near its line), then `AtField` by R2 (its trench condition lifted for a
   front point). `point_wants` puts them first at a front point (F4) and they need no trench: `fort_points`,
   `fort_holds` and `point_starts` take the `trench_done` test only for the non-front kinds. Front points are
   visited first in `fort_holds` and `point_starts` (then the rest ascending).
4. `front_why`: per recheck plan and front point with a front want that did not start, the first failing check
   (spec 3 names), as `bunker_block`. `front_jobs` += 1 in `new_fort_job` for the three kinds at a front point.
5. `assign`: an engineer without `breach` gets the own-point bonus at a front point that `hold`s it (already the
   rule via `pt.hold`); check that `wants_dig` does not send the held builder digging first (a held builder with a
   startable front job starts the job in `fort_start`, which runs before `order_infantry`; prove it in the test).
Tests: `ai_front_lane_runs_from_the_nearest_enemy_point` (control: no enemy point -> from the enemy base),
`ai_lays_front_field_without_a_trench` (own untrenched point, enemy point 300 m away, a Pioniere squad with 120 kg:
15 AP mines within 5 m of a line across the lane 55 m out; control `front` 0: no job), `ai_front_wants_come_before_
the_nest` (trenched front point, no nest: the first job is FrontAp; control `front` 0: Nest), `ai_front_caps_hold`
(F7; control `front_fields_max` 9: a third field), `ai_two_builders_never_share_a_front_site`.
**STOP POINT 1.** Quick green; commit; rewrite the log's top block; return.

## Chunk 2: threat, drop and resume, ends, determinism, measure (F5, F6, F8)
6. F5: `can_take` and the drop in `fort_continue` take the site radius from a helper `site_threat_m(job kind /
   point)`: `front_site_threat_m` for a front kind at a front point, else `threat_radius`; the builder's own radius
   stays `threat_radius`. `front_why` threat_squad / threat_site tell them apart.
7. F6: a front job dropped by F5 ends with `s10b_end(job, false)` (no L3) and `front_drops` += 1; its work is removed
   by the mechanics but mines laid and wire sections standing stay; the want comes back (`front_wants` reads what
   stands: an AP line with fewer than its mines is still wanted, and the new job's line starts at the first mine site
   not laid; if the mechanics cannot resume a mine line mid-way, lay the remaining part as a shorter line and say so
   in the log). `front_resumes` += 1 when a job starts at a site that already holds part of its work.
8. Ends: point lost mid-job (the existing `lost` rule: job ends, no refusal, laid part stays: F8); builder gone or
   under 60 %: the job ends with no refusal and `front_lost` += 1 (the CLI's `front_builders_lost_per_match`).
9. Liveness table row FrontAp (lane site on a slope over the blueprint's 25 %: refused, L3, no reorder within
   `refused_s`). Fuzz stage: **"fuzz stage: staged_s10b_ai + a FrontAp job in Watch with 5 mines laid"**.
10. Turn the default on: `front = 1` in `skirmish.toml` (BASELINE, snapshot); `c_off` keeps 0.
Tests: `ai_front_job_starts_with_an_enemy_60_m_from_the_site` (control `front_site_threat_m` 80: no job),
`ai_front_job_resumes_after_a_threat_drop` (enemy shown 40 m from the site at mine 6, removed 60 s later: 15 mines in
the end, `front_drops` 1, `front_resumes` 1, no refusal; control: with the resume want disabled in code the field stays
at 5-6 mines), `ai_front_job_ends_when_the_point_is_lost`, `ai_front_builder_dies_mid_field`,
`ai_front_fields_survive_a_lost_point`, `ai_front_deterministic_threads_and_save_load` (1/2/3/8 threads, save/load
at 5:00 and 12:00 with a front job live; control: as `ai_s10b_deterministic_...`).
Ends: quick green; default 40 set into `target/s10c-B40`, `front_off` 40 set: `neutral.py target/s10c-baseA40` same
40/40. Log per faction: Germany share, `matches_with_front_field_pct`, `front_mines_share_pct`,
`matches_mines_approached_pct`, `mines_set_off_per_match`, `wire_seen_by_enemy_pct`, `breaches_started`,
`sweep_wanted`, `front_drop_pct`, `front_builders_lost_per_match`, `front_why_*`, `own_side_mistakes`.
If mines are approached in under 10 % of matches with front fields, the lane source is wrong: say so and propose
the second lane (from the enemy base as well) instead of tuning distances.
**STOP POINT 2.** Commit; rewrite the log's top block (numbers, what C needs: `front_points`, `front_lane`,
`site_threat_m`); return.
