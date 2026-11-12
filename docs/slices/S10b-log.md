# S10b log (mechanics: mines, AT obstacles, bunker; at most 150 lines)

Spec: common `docs/slices/S10b-obstacles.md`, stages `S10b-build-A.md` ... `-F.md`; targets `targets/S10b.toml`.
Branch `s10b-obstacles` from main `8acc534`. Two other agents may work in this checkout: commit only your files.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)

**Next: Build D** (`S10b-build-D.md`). Builds A, B and C are done.
* Build C commits: `acd86f2` (sweep, HE hook, cards, tests), `21c19da` (fortify `sweep` row), plus the log commit.
* Sweeping lives in `crates/sim/src/sweep.rs`: `order_clear_mines`, `sweep_tick` (called from `fort_tick` before its
  `is_empty` return, gated by any `FortAccount::sweeps > 0`, so an empty lane still ends), `blast_mines`.
* Detection as built: roll = top 32 bits of `mix64(rng(seed, tick, sweeper, MINE_DETECT) ^ mine id)`; found when
  `roll * 100 * hz < detect_pct_per_s << 32` (`sweep::detects`). Sweepers: the two able members nearest the sweep
  point `a + dir * along` (no saved field holds them); the rest hold 5 m behind `a`. Lifter's spot 1 m short of the
  mine along the lane, every sweeper spot clamped back until clear of known mines by `trigger_cm + 50 cm`.
* HE hook tick-order point: `combat::burst` right after the S10a `blast_wire`, before fragments (any owner's HE,
  mines in `blast_radius` 2D with a line from the lift point to 10 cm above the mine): `boom_tick = tick + 1`,
  `mines_blasted` on the owner, `known` = all players. `take_mines` (after `take_charges` in `apply_blows`) bursts
  them next tick, so chains move one hop per tick.
* Load check changed: a saved `boom_tick` may equal the saved tick (a burst on T sets T + 1 = the tick saved between
  steps; before, such a save failed to load). The fuzz case (tick + 2) still fails.
* `steer_squads`: a `ClearMines` squad walks at `sweep_speed_pct` in `Arriving`/`Holding` only (the walk to `a` is at
  full pace). `guns.rs` served-crew list has `ClearMines`. `Demolish` is still refused `NotAllowed` in `game.rs`.
* For D: `order_fortify`'s R18 call is still minefields only; `FortRefusal::NoTarget` unused so far.
* Numbers (`target/s10b-C/fortify10.json`): T104a found_vs_expected 3.8 (found 82.0, expected 78.2), T104b lifted of
  found 95.1, T104d sweeper losses 76 men over 20 sweeps, T104e Bangalore cleared 100. T102/T103 unchanged.
  `targets --from-json target/s10b-C --runs fortify10`: 25 pass, 5 miss (hard 0; D-E rows null).
* 40-set: `target/s10b-C/skirmish40.json` vs `target/s10b-base40`: same 40/40.
* Checks: `tools/check.sh quick 8acc534` green after the last code commit (816 tests).

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

