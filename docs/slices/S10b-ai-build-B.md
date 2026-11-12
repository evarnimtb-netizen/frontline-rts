# S10b-ai Build B: paths round known mines, the AI lays minefields

Read first: `S10b-ai.md` 1, 4 (R1-R4, R8, R11, R12, L1-L4), 5 "Lay" row; the log's top block (A's code map).
**2 chunks**, about 30 self-counted turns each. No layout change (A's commit holds every field). Work rules as in A:
outline and show, targeted release tests through `tools/run.sh`, `tools/check.sh quick 72b2970` once per chunk end.

Start from: `crates/sim/src/path.rs` (`WireCost`, `search_with`, `smooth`, `VehiclePathCost`); `crates/sim/src/game.rs`
(`serve_paths`); `crates/sim/src/fort.rs` (`take_mines`, `wire_nodes`, `path_nodes`, `refresh_wire_index`);
`crates/sim/src/ai/fort.rs` (`point_wants`, `point_site_at`, `fort_start`, `run_fort_job`, `materials_room_g`);
`crates/sim/src/ai/guns.rs` (the `find_vehicle_path` call, a pattern for R2).

## Chunk 1: mine paths (R11, R12) and the sim counters
1. `path.rs`: a second step-cost layer `MineCost { nodes, cost_pct, crosses }` (same shape as `WireCost`, or one
   generic `StepLayer` both use: pick the smaller diff); foot search takes it beside wire; `VehiclePathCost` gains
   `mines: Option<MineCost>`. Smoothing never keeps a segment the exact test prices (as wire).
2. `game.rs` `serve_paths`: with `mine_paths` 1 and any mine standing, a per-player node list of enemy mines the
   player knows (`known` bit; foot: `trigger = "foot"`, vehicles: `"vehicle"`), band `mine_band_*`. Derived, never
   saved: rebuilt when a fingerprint (mines len, `next_mine`, known-bit popcount per player) changes, and after load.
   No mines or switch off: the old calls unchanged (`find_path` / `find_infantry_path`).
3. `fort.rs` `take_mines`: push `MineBlast` (cap 32, drop older than `BLAST_KEEP_S` 600); count `lost_on_known` (the victim's
   player knew the mine), `own_mine_losses` (guard, stays 0), `at_exposure_ticks` (an enemy vehicle within 5 m of an AT
   mine, per tick; one spatial query per AT mine, only when AT mines stand).
Tests: `paths_avoid_known_enemy_mines` (foot over a known AP field, tank over a known AT field: route round; control
`mine_paths` 0 -> straight through), `paths_ignore_unknown_and_own_mines` (control: price every mine -> detour),
`mine_layer_matches_full_scan_after_load` (control: skip the rebuild after load), `blast_ring_keeps_32_newest`
(control: cap 31). Fortify row `s10b_ai.paths` (`sdk/crates/cli/src/fortify/s10b.rs` or a new `s10b_ai.rs`): a rifle
squad ordered across a known enemy AP field, a tank across a known AT field, 10 seeds: `foot_losses`,
`tank_immobilised_pct`; exposure `routes_through_field_pct` (straight line crosses the field: 100 by staging) and
`priced_pct` (paths that left the straight line by > 5 m); control `control.*` with `mine_paths` 0 (losses > 0).
Fuzz stage: **"fuzz stage: staged_s10b_ai + blasts and lost_on_known set"** (the staged save sets off one mine on a
known field so `blasts` and `lost_on_known` are non-zero; `MUST_HOLD` names them).
**STOP POINT 1.** Quick green; commit; rewrite the log's top block (path cost on a 30 m field before/after, nodes
expanded, what chunk 2 starts with); return.

## Chunk 2: AI lays AP and AT fields (R1-R4, R8)
4. `ai/fort.rs`: `point_wants` offers `ApField` (after wire or with wire off) and `AtField` (switch `at_mines`) at own,
   trenched, low-threat points; `fields_per_point`, `at_fields_max`, blueprint caps (S10a F4: never ask over a limit).
   Sites: AP from `point_site_at` with `wire_out_m + ap_beyond_wire_m` (no wire: `ap_out_m`), 30 m; AT: one
   `find_vehicle_path` from the enemy base to the point (at most one per point per `recheck_s`, its own node budget,
   no cache), the path point `at_out_m` from the point, a 40 m line perpendicular, clamped; none found: no AT site.
5. Jobs: the S10a fort job (Load -> Go -> Order -> Watch); `start` set (L1); `jobs_max` across S10b kinds; S10b
   materials budget `materials_max_pct` on its own MP sum; `lay_sites` once per site offered, `s10b_jobs` per start.
   L1 deadline `job_max_s`; L3 refusal on BadSpot, Cap, deadline, stall; L4 check before every start.
6. Liveness table: add the Fort row (an AP field site inside a vehicle's `vehicle_clear_m`: BadSpot every time).
Tests: `ai_lays_ap_field_beyond_its_wire` (control `mines` 0), `ai_lays_at_field_on_the_vehicle_approach` (asserts the
field crosses the enemy path within 10 m; control `at_out_m` 0 -> refused), `ai_mine_caps_hold` (control
`at_fields_max` 9), `ai_s10b_budget_stops_loading` (control `materials_max_pct` 100),
`ai_tank_drives_through_own_field_unhurt` (control: an enemy-owned field -> immobilised), job-table rows [B]: `ai_two_builders_never_share_site`,
`ai_s10b_builder_dies_mid_work`, `ai_s10b_refused_spot_is_remembered` (control `refused_s` 0).
Fortify row `s10b_ai.mines`: staged held trenched point, an AI engineer squad with materials, `drive_ai_habit(Fort)`
until the AT field stands (or 15 min), then an enemy tank drives the approach to the point: `tank_stopped_pct`,
`field_laid_pct`; exposure `tank_reached_field_pct` (within 5 m of a mine); control: the same with `mines` 0.
Fuzz stage: **"fuzz stage: staged_s10b_ai + an AtField job in Watch with its work and 3 laid mines"**.
Ends: quick green; `tools/batch.sh` 40 set switches on (background) and the all-off 40 set: `neutral.py
target/s10b-base40` same 40/40; on-set Germany share, `matches_with_mines_pct`, `tanks_stopped_per_match` logged.
**STOP POINT 2.** Commit; rewrite the log's top block (numbers, refusals seen, what C needs); return.
