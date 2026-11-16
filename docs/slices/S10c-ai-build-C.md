# S10c-ai Build C: builder supply, belts on the vehicle lane, B1-2

Read first: `S10c-ai.md` 4 (C1-C8), 8 (home works floor), 5 (rows Belt, Builder buy); the log's top block. **2 chunks**, about 30
self-counted turns each. No layout change. Work rules as in A. Each item sits behind its own key (`jobs_max`,
`mine_assign`, `builder_cards_min`, `belt`); with all at their off values the 40 set equals the `front`-only run
`target/s10c-B40` (not main: `front` stays on), and with `c_off` it equals `target/s10c-baseA40`.

Start from: `crates/sim/src/ai/fort.rs` (`s10b_room`, `bunker_yields`, `point_starts`: the `bunker_assign` pick,
`start_barriers`, `barrier_line`, `hedgehog_row` / `hedgehog_side` / `hedgehog_next`, `own_route` / `on_own_route` /
`own_route_side`, `at_row1_laid`, `at_site`, `count_own_blocked`); `crates/sim/src/ai.rs` (`buy`: the urgent
`breach_wanted` buy is the pattern for C3); `crates/sim/src/ai/guns.rs` (`gun_route`); `crates/sim/src/path.rs`
(the vehicle block layer: can a search take extra blocked nodes?); `sdk/crates/cli/src/fortify/s10b.rs` (row
`obstacle`, `demolish` as staging patterns); `crates/sim/src/s10b_ai_bunker_tests.rs` (`bunker_assign` scene).

## Chunk 1: builder supply (C1-C3) and B1-2 (C6)
1. C1: `jobs_max` default 2 (`skirmish.toml`, BASELINE, snapshot; `c_off` and `jobs1.toml` keep 1). Check
   `bunker_yields` still frees a slot with two slots (its test with `jobs_max` 2: a third job starts while the bunker waits).
   C8 `front_slots` 1: `point_starts` (and the C2 pick) start no front job while `front_slots` front jobs live, so
   one builder slot stays for the home works.
2. C2 `mine_assign`: in `point_starts`, a front want (`FrontAp`, `FrontWire`, `AtField` at a front point) with no fit
   squad assigned to the point takes one free squad from anywhere that `can_take` it (nearest to the site, then id),
   exactly as the `bunker_assign` pick; F5 and F10 (materials carried, in base, or a truck) apply at the pick. `front_why` no_builder then means "none anywhere".
3. C3 `builder_cards_min`: in `buy`, before the weighted pick: when N > 0, a habit want exists (`fort_points` marked a
   point) and fewer than N own living squads are builders (`place_mines` and `build_structure`, no `breach`; squads
   on a job count), the buy is the faction's cheapest card with those capabilities, if affordable and inside the pop
   cap; `builder_buys` += 1. Default stays 0; `builders2.toml` sets 2.
4. C6 / B1-2: first reproduce (`frontline-cli skirmish --ai-vs-ai --seed 104 --side-detail --json F`, `own_blocked`
   13 on main per `docs/TODO.md` B1-2). (a) Recount: `count_own_blocked` keeps a counted pair in `blocked_pairs`
   while its vehicle squad and its piece both live (sorted, cap unchanged), so a pair that reappears is not new.
   (b) Cause: which row at base 0 blocks which own route, and why R6 let it through (R6 checks base -> point ->
   `route_out` only). Fix it if it is a missing route in R6 (for example the route to a second non-own point);
   if it needs more than this chunk, write the finding and a proposed rule in the log and stop there.
Tests: `ai_two_s10b_jobs_run_at_once` (control `jobs_max` 1), `ai_front_jobs_leave_a_slot_for_home_works` (control
`front_slots` 2), `ai_mine_assign_sends_a_builder_to_the_front`
(builder assigned to the rear point, front want open: it takes the front job; control `mine_assign` 0: no job),
`ai_buys_a_builder_under_the_floor` (control 0: the weighted pick), `ai_builder_floor_never_overbuys`,
`own_blocked_pair_is_counted_once` (a vehicle stuck at an own piece, moved off and back: `own_blocked` 1; control:
the old replace-the-list code counts 2).
**STOP POINT 1.** Quick green; commit; rewrite the log's top block; return.

## Chunk 2: belts (C4, C5, C7)
5. `WorkKind::Belt` at a front point whose AT row 1 is laid (`at_row1_laid`) with `belt` 1: `belt_row(i, side)` =
   `belt_pieces` sites `hedgehog_step_m` apart on a line perpendicular to the vehicle lane (the R2 path), centred on
   it, `belt_gap_m` beyond the AT field on the enemy side. With a `build_heavy` squad able to take it: hedgehog
   point jobs in turn (the row logic of `hedgehog_next`, sites fixed at the first start); else one log barrier line
   of the same length (S10c-4 (A)). One belt per lane; `belt_jobs` += 1 per job.
6. C5: before the first start, the own route (`own_route(i)`: base -> point -> `route_out`) is searched again with
   the belt's nodes blocked; the belt is taken when that path exists and is at most `belt_detour_pct` of the path
   without it; else the row moved by its own length to the other side of the lane once; else L3, `belt_route_rejects`
   += 1. If the path search cannot take extra blocked nodes without a change to `path.rs` larger than a parameter,
   use this instead and say so in the log: the belt is taken only when the route vehicle can stand on the two nodes
   one step beyond each row end (passable, slope inside its limit). `belt_detour_pct` 0 turns the rule off (control).
7. C7: no change to R14. Check with a fortify row that the pieces fit together: `s10c.belt`: staged front point,
   own AT field row 1 laid by the AI (`drive_ai_habit(Fort)`), Baupioniere with a loaded truck; then an enemy medium
   tank ordered from its base to the point and an own tank ordered base -> next point: `belt_built_pct`,
   `enemy_tank_mined_pct` (the tank loses a track on the field while rounding the row), `own_blocked` (must be 0);
   exposure `enemy_tank_reached_belt_pct` (within 15 m); controls `control.enemy_tank_mined_pct` with `belt` 0 (the
   tank drives the lane's middle: the row's half-gauge shift already puts a mine under a track, so the control may
   read the same: then the row is a report and the log says the belt adds nothing over the field), and
   `control.own_blocked` with `belt_detour_pct` 0 and the belt staged across a one-node gap (must be > 0).
8. Liveness table row Belt (every site refused: an own tank parked on the row). Fuzz stage: **"fuzz stage:
   staged_s10b_ai + a Belt job in Watch with two pieces standing"**. Turn the defaults on: `mine_assign = 1`,
   `belt = 1` (BASELINE, snapshot); `c_off` keeps 0.
Tests: `ai_builds_a_belt_before_its_at_field` (control `belt` 0), `ai_one_belt_per_lane`,
`ai_belt_leaves_the_own_way_round` (control `belt_detour_pct` 0: `own_blocked` > 0), the determinism test of B with a
belt job live.
Ends: quick green; default 40 set into `target/s10c-C40`; `c_off` 40 set: `neutral.py target/s10c-baseA40` same
40/40; ablation 40 sets `jobs1`, `mine_assign_off`, `belt_off`, `builders2` (`tools/ablate.sh
ABLATIONS=docs/slices/targets/S10c-ai-ablations`). Log per faction and run: Germany share, mines / obstacle / belt /
bunker % of matches, `front_mines_share_pct`, `mines_set_off_per_match`, `tanks_stopped_per_match`,
`at_exposure_s_per_match`, `demolish_wanted_matches`, `builder_buys_per_match`, `builder_share_pct`,
`own_side_mistakes`, `s10b_materials_mp_pct`.
**Home works floor (hard, spec 8):** per faction, MG nests done per match and `matches_with_works_pct` on
`target/s10c-C40` against `target/s10c-baseA40`: more than 10 % (relative) lower -> stop, do not turn the defaults
on, report the numbers and which of `front_slots`, `mine_assign`, `jobs_max` moves it (the ablation sets). `builders2` is the answer to owner question S10c-2: report it, do not
make it the default.
**STOP POINT 2.** Commit; rewrite the log's top block (numbers, open items for Review); update `docs/modding/tuning.md`
and `docs/modding/fortifications.md` (AI section) for the new keys; return.
