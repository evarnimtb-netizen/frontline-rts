# S10b-ai Build C: obstacles in front of guns and works, the bunker

Read first: `S10b-ai.md` 4 (R5-R8, R13, L1-L4), 5 "Lay" row; the log's top block. **2 chunks**, about 30 self-counted
turns each. No layout change. Work rules as in A (outline and show, targeted release tests via `tools/run.sh`,
`tools/check.sh quick 72b2970` once per chunk end).

Start from: `crates/sim/src/ai/fort.rs` (`start_walls`, `point_site_at`, `nest_at`, `fort_start`, `fort_holds`,
`run_fort_job`); `crates/sim/src/ai.rs` (`order_infantry`: the MG-to-nest rule; `vehicle_tasks`);
`crates/sim/src/ai/guns.rs` (gun in Hold, arc direction); `crates/sim/src/fort.rs` (`blocks_vehicles`, block layer
refresh); `sdk/crates/cli/src/fortify/s10b.rs` (rows `obstacle`, `bunker` as staging patterns).

## Chunk 1: obstacles (R5, R6, R13) and own-route safety
1. `WorkKind::LogBarrier`: an own gun in Hold for `gun_hold_s`, a 20 m line across its arc `obst_gun_m` out (the
   `start_walls` frame); one per gun. `WorkKind::Hedgehog`: a row of `hedgehogs_row` point works `hedgehog_step_m`
   apart, `obst_work_m` in front of an own built nest or bunker; one row per point; the row is `hedgehogs_row` jobs
   in turn by the same builder (one job at a time, `jobs_max`), the row's sites fixed at the first start.
2. R6 own route: at site choice one `find_vehicle_path` own base -> the point -> the nearest non-own point (two
   searches, own node budget); a site whose footprint plus `wreck_pad_m` lies within `own_route_m` of the path is
   rejected; mirrored site (other side of the arc or work) once; else L3 refusal and `own_route_rejects` += 1.
3. `own_blocked`: an own AI vehicle whose move failed (the existing failed-move rule) with an own `blocks_vehicles`
   piece within 8 m of it; counted once per vehicle per piece (a sorted pair list in the plan, not saved: count on the
   first failure only, keyed by the failure tick). Must stay 0 (T115).
4. R13 helper `known_obstacles(&self) -> Vec<u32>` (enemy `blocks_vehicles` pieces within `obst_seen_m` of an own unit,
   ascending ids); used by D, tested here.
5. Liveness table: add LogBarrier (gun in Hold beside a steep slope: every site refused).
Tests: `ai_builds_barrier_before_gun` (control `obstacles` 0), `ai_builds_hedgehogs_before_nest` (control
`hedgehogs_row` 0 -> none), `ai_never_blocks_own_route` (a gun whose arc faces along the own route: site rejected,
mirrored or refused; control `own_route_m` 0 -> the barrier stands on the route and `own_blocked` > 0),
`ai_knows_obstacles_by_distance` (control `obst_seen_m` 0), job-table row `ai_s10b_site_taken_ends_job` (another work
lands on the site; control: skip the site check in Watch).
Fortify row `s10b_ai.obstacle`: staged own emplaced AT gun in Hold, AI builder via `drive_ai_habit(Fort)`; then an enemy
tank ordered to the gun's position and an own tank ordered base -> next point: `barrier_built_pct`,
`enemy_time_in_arc_s` (report), `own_blocked` (must be 0); exposure `enemy_tank_reached_barrier_pct` (within 10 m);
control `control.own_blocked` with `own_route_m` 0 (must be > 0) and `control.enemy_time_in_arc_s` with no barrier.
Fuzz stage: **"fuzz stage: staged_s10b_ai + a LogBarrier job in Watch and one standing hedgehog of a row"**.
**STOP POINT 1.** Quick green; commit; rewrite the log's top block; return.

## Chunk 2: the bunker (R7) and determinism
6. `WorkKind::Bunker` at the own held trenched point nearest the enemy base, trench end opposite the nest by parity,
   facing the enemy (`point_site_at` with the nest offset mirrored); `bunkers_max`; builders: `build_heavy` first,
   else `build_structure` (R21 50 % rate); kg in loads as S10a (250 kg: a truck run or two carries, the S10a paths).
7. Manning: in `order_infantry`, the point's MG team (else the defender squad nearest the bunker) gets the bunker centre
   as its Defend target; the nest rule takes a second MG team only. One squad per bunker.
8. Liveness table: add Bunker (site on a slope over `bunker` slope 15 %: refused).
Tests: `ai_builds_and_mans_one_bunker` (control `bunker` 0), `ai_bunker_needs_heavy_or_field_builder` (US engineers at
50 %; control: Baupioniere only -> USA none), `ai_s10b_deterministic_threads_and_save_load` (1/2/3/8 threads, save/load
at 5:00 and 12:00 with fields, barrier and bunker jobs live; as `ai_fort_is_deterministic_...`; control: drop
`AiFortJob::start` from the save -> hash differs).
Fortify row `s10b_ai.bunker`: staged held trenched point, Baupioniere (and in a second set US engineers) with 250 kg,
`drive_ai_habit(Fort)` up to 25 min: `built_pct`, `built_s` (report), `manned_pct`; exposure `builder_alive_pct`;
control: `bunker` 0 (built 0).
Fuzz stage: **"fuzz stage: staged_s10b_ai + a Bunker job in Watch with its pit dug"**.
Ends: quick green; 40 set on and all-off 40 set (`neutral.py target/s10b-base40` same 40/40); log Germany share,
`matches_with_obstacle_pct`, `matches_with_bunker_pct`, `own_side_mistakes`, `builder_share_pct`.
**STOP POINT 2.** Commit; rewrite the log's top block (numbers, what D needs: `known_obstacles`); return.
