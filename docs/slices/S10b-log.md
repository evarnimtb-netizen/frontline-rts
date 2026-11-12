# S10b log (mechanics: mines, AT obstacles, bunker; at most 150 lines)

Spec: common `docs/slices/S10b-obstacles.md`, stages `S10b-build-A.md` ... `-F.md`; targets `targets/S10b.toml`.
Branch `s10b-obstacles` from main `8acc534`. Two other agents may work in this checkout: commit only your files.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)

**Fix done (chunk 3, 2026-11-12): all review findings R1-R19 and G1 fixed, each with a test seen failing first.**
Findings in `target/s10b-review.json`. Open only by owner decision:
* T106b (tank uses the demolished gap) still reads `gap_crossed_pct` 0 (MISS; the tank now really drives, R5).
* A demolition charge at any bunker wall kills the five inside (slit T107d 100, uphill T107k 100, back wall
  T107l 100): the slit does not matter. How it should play is an owner question.
Numbers (fortify10 into `target/s10b-fix`): every T102-T107 line PASS except T106b; new T102e close passes 143,
T104e Bangalore (blast-only count) 100, T107i/j controls 23920 hp / 45 %. 40 set vs `target/s10b-base40`:
same 40/40. Checks: `tools/check.sh quick` all ok (856 tests), `tools/check.sh tests` all ok (881 tests),
Godot test_match_view 379/379, check_project ok, test_net_ui 143/143, test_data_reload 21/21 (bridge rebuilt).
Save fuzz with the staged S10b save: 9686 mutations, 0 panics, 0 unloadable. SAVE_VERSION 24, PROTOCOL 14 kept.
Neutrality reasoning for chunk 3: CLI rows and tallies only observe (R8, R9, R13); R10/R11 change which selected
squad the bridge picks (player intents, never AI); R14 and the grid relayout are HUD/test only.

## Plan (2026-11-11)

Wrote the common spec (scope, data model, rules R1-R30, job ends, save rules, done-when), six stage files, the
targets (S10a-ai body; T5c/T5d 35-65; T31a-d, T55a/b, T95b/d reports; new T102-T108), this log, STATE and TODO.
Read: STATE, roster S10, infantry-structures 7.6/7.7, S10a spec, `fort.rs` outline, `ai/fort.rs` `fort_of`,
`path.rs`/`vehicle.rs` block layer, `damage.rs` `blast_tracks`, `combat.rs` `Burst`, `fort.rs` `take_charges`.
Design-code facts used: `burst` and `blast_tracks` never hurt own units (so S10b-1 (A)); vehicle boxes have no belly
plate (S10b-3 (A)); charges become `Thrown` items, so a mine is an item def `kind = "mine"`; the block layer already
takes wreck footprints, obstacles join them.

## Build A chunk 1 (2026-11-11)

Commits `ba7478b` (layout), `9a3d197` (data, guard), `20ade12` (measures). Layout fields: sim `Mine`,
`FortState::{mines, next_mine}`, `MAX_MINES`, `Charge::piece`, twelve `FortAccount` counters, `FortRefusal::NoTarget`,
`Order::{ClearMines, Demolish}` (`OrderKind` 12, 13), `CommandKind::{ClearMines, Demolish}`, rng `MINE_DETECT` (36);
content `ItemKind::Mine` (ITEM_KINDS 6), `[mine]`/`MineSetup`, `DigProfile::BunkerPit` + `bunker_pit`, capabilities
12-14 (`RESERVED` keeps observe, radio, medic, issue_crates, flame), `FortDef`/`FortSetup` `mine_item` (item index)
and `blocks_vehicles`, pieces 1..=6, `PIECE_PARTS` `steel`, `FORT_CAPABILITIES` + `build_heavy`, `place_mines`,
twelve `[fortify]` keys (serde defaults = the rules values). Validators: mine items need `[mine]` and `[he]`, never
in a kit (loadout or member items); `mine_item` names a mine item (cross check); a minefield is a line with no
pieces or wire; `blocks_vehicles` needs pieces; `build_heavy` not over an HQ and needs pieces.
Existing tests changed (forced by the layout): kit arrays 6 wide (`ww2_mod.rs`, `abilities.rs`), the reserved
capability case in `variants.rs` uses `observe`, the S10a blueprint list (`s10a_content.rs`), the unknown blueprint
in the bridge intent test is `pillbox`, `save_fuzz.rs` mirror version 24; setup baseline and canonical snapshot.
Tests added: `s10b_defs_validate`, `promoted_capabilities_parse` (`sdk/crates/content/tests/s10b_content.rs`);
controls seen failing: `demolish` back in RESERVED, the kit-mine check off, the minefield-pieces check off.
Bunker data: walls back/left/right 40 cm tall from the surface, roof bottom at 40 cm, two front walls 144 cm wide
leave an 80 cm gap (the 80 x 40 slit). Mine items: `range_m` 1, `supply` explosives 4 / 9 kg (never refilled: no kit).
`crates/simbench/src/report.rs` has no setup baseline to refresh (nothing changed there).

## Build A chunk 2 (2026-11-12)

Commits `9180432` (sim tests, `World::edit_squad`), `bde4ce7` (load cases), `76734d8` (CLI test). Tests and the
control each was seen failing with: `s10b_layout_roundtrips_and_hashes` (`#[serde(skip)]` on `mines`: load fails),
`empty_s10b_state_changes_nothing` (`is_empty` ignoring `mines`: the laid far mine leaves it empty),
`fort_of_ignores_s10b_blueprints` (R29 filter off: `Wall` is `ap_minefield`), `load_rejects_bad_s10b_state` (22
cases; the `boom_tick` check off: its case loads), `s10b_counters_zero_without_use` (the `s10b` insert in
`json_out.rs` off: the block is the raw struct). Deviation: "without the new data" strips only the S10b blueprints
from the setup (removing the mine items would shift every item index); the mine items are in no kit.


## Build B (2026-11-12)

Laying (R1, R2, R18), triggering (R3-R7), counters, the place_mines caps (R27) and the `atmine`/`apmine` rows. Tests
in `s10b_tests.rs`, each seen failing with its control: `minefield_lays_one_mine_per_segment` (segment 300: 10
mines), `mine_cap_per_player` (limit 100: no Cap), `mine_laying_ends_when_line_done_or_interrupted` (minefield works
kept on re-order), `ap_mine_goes_off_on_contact_only` (trigger 60), `ap_mine_kills_the_man_who_steps_on_it` (damage
10), `ap_mine_ignores_vehicles` / `at_mine_ignores_infantry` (triggers swapped), `at_mine_immobilises_a_tank_over_it`
(vs_vehicle 10: track 60 hp), `own_side_never_sets_off_own_mines` (either owner filter off),
`mine_trigger_is_deterministic` (per-run random burst key; the spec's HashMap-order control passes because one tick's
bursts commute), `site_refused_near_a_vehicle` (vehicle_clear_m 0). Existing tests changed: `promoted_capabilities_parse`
(the three cards gain `PlaceMines`), canonical snapshot blessed.

## Build C (2026-11-12)

Sweeping (R8-R11), HE sets mines off (R12, R13), `clear_mines` on the five R27 cards, the `sweep` row. Tests in
`s10b_tests.rs`, each seen failing with its control: `sweep_finds_at_expected_rate` (sim detect 25: found 59.3 vs
78.6), `sweeper_stops_short_of_a_found_mine` (lift spot on the mine, no clamp: lifter at 0.40 m), `lift_time_follows_tier`
(field tier for all), `clear_mines_needs_capability` (grenadiers given the bit), `clear_ends_at_lane_end_with_no_known_mine_left`
(end ignoring known mines), `found_mine_known_to_finder_only` (all bits on find), `burst_sets_off_mines_next_tick` (radius
x 0.5), `bangalore_clears_mines_along_its_length` (laid 3 m short), `sweep_is_deterministic` (roll keyed by a
process-wide counter); `sweep::tests::detection_roll_rate_is_p_per_tick` (control `<< 31` not run). Deviations: the
rate test's expected n is the full-pass n from lane geometry with the run's pace (checked against the run's own n on
misses): the run's n stops at a find, so `1 - (1 - p)^n` on it is biased low (47.7 vs 80 found); 1600 passes, not 400
(at 400 the sd is 1.9 points against a 3-point bound). The fortify row uses the run's n for misses and the geometry
for finds. The spec's determinism control (roll by tick only) stays deterministic, so another control was used. The
Bangalore case stages its field after the lay (the carrier's walk set mines off otherwise). Existing tests changed:
`promoted_capabilities_parse` (five cards gain `ClearMines`); canonical snapshot blessed.

## Build D (2026-11-12)

Obstacles block vehicle nodes (R16, R17), R18 for obstacles and the bunker, R21, Demolish (R19, R20), `demolish` on
the three R27 cards, the `obstacle` and `demolish` rows. Tests in `s10b_tests.rs`, each seen failing with its
control: `obstacle_blocks_vehicle_nodes` (`blocks_vehicles` false), `removed_obstacle_reopens_the_path` (no mark for
removed pieces), `infantry_walks_through_obstacles` (pieces halve pace in `wire_pace`: 192 vs 129 ticks),
`obstacle_refused_near_a_vehicle` (`vehicle_clear_m` 0), `heavy_blueprint_at_half_rate_for_field_engineers` (no
`heavy_by_field_pct` scaling), `demolish_lays_a_satchel_and_destroys_a_hedgehog` (`demolish_pct` 100: 300 hp left),
`demolish_needs_capability_and_satchel` (grenadiers given the bit), `demolish_ends_when_charge_laid_or_target_gone`
(order kept), `demolition_is_deterministic` (fuse + 1 tick in the reloaded run). Deviations: the reopen test removes
nine hedgehogs, not one (one gap never opens a way, see T106b); the log line's blocked check starts 1 m in (its ends
sit half way between two nodes, the wheeled layer's raw node there is 4.0001 m from the last piece); the tamped
piece takes R20 whatever its owner (the order may target any owner's piece; the S6 overpressure spares own boxes).
Existing tests changed: `promoted_capabilities_parse` (three cards gain `Demolish`); canonical snapshot blessed.

## Build E (2026-11-12)

The bunker (R21-R24, R26) and the `bunker` row. Tests in `s10b_tests.rs`: `bunker_digs_pit_then_builds_six_pieces`
(five-piece def: 5 vs 6, seen), `bunker_needs_build_heavy_or_half_rate_field`, `small_arms_do_not_damage_bunker`,
`bunker_roof_survives_three_81mm`, `roof_satchel_kills_nobody_inside`, `satchel_at_slit_reaches_men_inside`,
`ap_shots_break_the_roof_after_seven`, `bunker_saves_loads_and_hashes` (each runs its control in-test: heavy 100,
armour 1, roof 400 / 250, slit closed, `shot_piece_damage` 100, dropped piece), `bunker_pieces_share_one_base_on_a_slope`
(per-piece base: fails, seen); CLI `bunker_row_fills_every_field` (control: default row). Deviations: the def's
pieces moved to the stage file's sizes (A had 30 cm walls); tests give `carry_kg_per_man` 50 (one squad carries
250 kg; the cap is 20 a man); `bunker_pit` parapet 40 -> 0 (verify value; it filled the slit); `z_from = "base"`
pieces on the site's surface; the row's bunker faces down the slope; the row is its own module. No save change.
Existing tests changed: canonical snapshot blessed (parapet).

## Build F (2026-11-12)

Read: log top block, stage file F, spec 1, 3, 4, 7. Snapshot test `fort_snapshot_lists_mines_with_known_bits`
(control `known: 0xff` in `fill_fort`: fails, seen); bridge `s10b_intents_make_commands` (control demolish -> `CutWire`:
fails, seen); net `net_s10b_commands_in_limits` (control limit + 1 m: fails, seen), fuzz roundtrip extended; Godot
`_test_s10b` 15 checks (control: drop the known filter in `fort_view.gd` -> "unknown enemy one hidden" fails, seen).
Docs: `fortifications.md` S10b, `match_state.md` S10b, `relay-protocol.md` v14. Numbers above in the top block.

## Fix (2026-11-12)

Chunk 1: `ae746b1` R1 (setup blueprints checked at load), `35b3f29` R16 (a finished lift clears other squads' same
lift) and R2 (sweep gated on a standing order), `0269ced` R15 (obstacle waits while it covers a vehicle; drive escape)
and R19, `2bb07ad` R18 (ap_mine vs_vehicle_pct 10), `bf803a4` G1 (validation accepts `board` on a served gun; KNOWN
empty). Chunk 2: `c7bfe66` R5 (tank_through stops on a wreck, not `alive`; approach and no-line control), `a39b7a9`
R6 (staged volleys with an armour-1 control; open-ground MG control), `1ea1390` R7 (real Demolish at the slit, both
slope directions, back-wall control), `b6fba60` R3 (DEMO_CRATER/DEMO_FRAGMENT, charge ids < 2^28 with guards),
`b5f458d` R17 (stand-off from the mine toward the lifter), `12bba9d` R12 (Demolish sees only visible pieces).
Chunk 3: `7844b15` R4 (staged S10b save in the fuzz set), `70028f9` R8 (squad walks the whole AT field, close
passes T102e) and R9 (Bangalore counts blast set-offs only), `487c3e4` R13 (T108 `used` + works started and
ClearMines/Demolish orders taken), `31ee326` R10/R11/R14 (Demolish to a squad with a satchel, own-capability
squad first, grid relayout on the S10b row, Godot lane ends and Demolish path, frame mine filter).
Tests (control in each comment, seen failing): chunks 1-2 as named in their commits; chunk 3
fuzz_plan_reaches_s10b_state, infantry_walks_the_whole_at_field, contact_set_off_is_not_a_blast_clear,
t108_counts_s10b_use_that_does_not_finish, s10b_intents_make_commands (updated: hedgehog to bau, satchels),
frame_drops_unknown_enemy_mines, Godot `_test_s10b` (R10 case, lane ends, Demolish target).
Code outside S10b changed: `VehicleState::validate`, `drive` (escape), `lay_charge` (id guard), CLI `world_with`,
`orders_grid.gd` relayout key, `save_layout::Layout` Clone.
