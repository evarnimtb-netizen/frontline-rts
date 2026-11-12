# S10b log (mechanics: mines, AT obstacles, bunker; at most 150 lines)

Spec: common `docs/slices/S10b-obstacles.md`, stages `S10b-build-A.md` ... `-F.md`; targets `targets/S10b.toml`.
Branch `s10b-obstacles` from main `8acc534`. Two other agents may work in this checkout: commit only your files.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)

**Next: Build E** (`S10b-build-E.md`). Builds A-D are done.
* Build D commits: `3f98c04` (obstacles, R18/R21, Demolish, cards, tests), `0677e5c` (fortify rows), log commit.
* Obstacles: `BlockLayer::update` gets wreck footprints then the `blocks_vehicles` piece footprints (box half, yaw;
  the layer adds `wreck_pad_m`). `update_block` (end of tick, `refresh_hashes`) diffs the standing pieces against
  `VehicleDerived::piece_feet` and calls `mark_piece_block` for each added or removed one, so every removal path
  (`hit_static`, HQ roof, `edit_fort`, load) is seen; nothing new saved. No list is built without pieces.
* R18 (`order_fortify`) now covers `mine_item`, `blocks_vehicles` and every `build_heavy` blueprint (the bunker too:
  `MatchState::heavy_def`). R21: `build_structure` squads may take `build_heavy` blueprints; `build_rate` gives
  tier `build_pct x heavy_by_field_pct / 100` unless the squad has `build_heavy` (Baupioniere 150). Only the S10b
  blueprints are `build_heavy`, and the AI's `fort_of` never returns them.
* Demolish in `crates/sim/src/demolish.rs`: `order_demolish`, `demolish_tick` (status stage, right after `fort_tick`
  in `world.rs`, own `Ctx`; returns unless some squad has `Order::Demolish`), `demolish_piece` (R20). The burst
  knows its piece through `Burst::piece` (not saved): `take_charges` returns `(charge id * 8, piece)` pairs for this
  tick, matched to fused items with `thrower == SimId(u32::MAX)`. The tamped piece is hit whatever its owner, no line
  check, and skipped by the overpressure loop. `guns.rs` served-crew list has `Demolish`.
* For E: the bunker's satchel at the slit uses ordinary thrown satchels (no `Charge::piece`); a `Demolish` against a
  bunker piece would take R20 (x3) on that one piece only.
* Numbers (`target/s10b-D/fortify10.json`): T105a/b crossed 0/0, T105c infantry speed 103.5 %, T105d tank reached 0
  (on the fortify map the tank does not reach a point 60 m behind in 120 s; in the flat test world it drives round a
  30 m hedgehog line's end in about 19 s), T106a destroyed 100, T106c charge laid 15.6 s, **T106b gap crossed 0
  (hard miss)**: with 3 m spacing one hedgehog gone leaves no way: a hedgehog blocks nodes within 0.9 + 2 m (pad)
  of its centre, dilated one 4 m node, plus the margin band; a tank first got through with 5 adjacent hedgehogs gone
  (18 m between standing centres; 1-4 gone: no way). Owner question in the report; T106b left as is.
  `targets --from-json target/s10b-D --runs fortify10`: 31 pass, 6 miss (hard 1: T106b).
* 40-set: `target/s10b-D/skirmish40.json` vs `target/s10b-base40`: same 40/40.
* Checks: `tools/check.sh quick 8acc534` green after the last code commit (825 tests).

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

