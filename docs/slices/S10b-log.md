# S10b log (mechanics: mines, AT obstacles, bunker; at most 150 lines)

Spec: common `docs/slices/S10b-obstacles.md`, stages `S10b-build-A.md` ... `-F.md`; targets `targets/S10b.toml`.
Branch `s10b-obstacles` from main `8acc534`. Two other agents may work in this checkout: commit only your files.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)

**Next: Build C** (`S10b-build-C.md`). Builds A and B are done.
* Build B commits: `034fa03` (laying, `take_mines`, bursts, R18, tests), `456bed3` (place_mines on the three cards,
  content expectations, canonical snapshot), `86f2fa1` (fortify `atmine`/`apmine` rows), plus the log/test commit.
* Laying: `stand_segment` lays a `Mine` at the segment end when `mine_item` is set; cap = standing mines of
  (owner, blueprint) via `mines_of`, which `fort_used` now uses for minefields. Over the cap: work removed, `Cap`.
* R18: `MatchState::near_a_vehicle(line)` (hull or wreck centre within `vehicle_clear_m` + hull half-diagonal of any
  segment). Called for minefields only; D (obstacles) and E (bunker) add their blueprints at the same spot in
  `order_fortify` (`d.mine_item.is_some() && ...`).
* Trigger: `MatchState::take_mines` (fort.rs) runs in `apply_blows` right after `take_charges`. `boom_tick == tick`
  mines burst there too but do NOT bump `mines_set_off` (only contact does): C counts `mines_blasted` where it sets
  `boom_tick`, and makes the mine known to all (R13) there if it wants it shown before the burst.
* Burst key range: `Thrown.id = MINE_KEY_BASE | mine id` (`MINE_KEY_BASE` = 2^31; thrown ids and mine ids stay below
  `MAX_ID_COUNTER` = 2^31; charges use `CHARGE_CRATER`), crater purpose `ITEM_CRATER`.
* Counters: `vehicles_mined` (owner; a vehicle whose `immobilised()` turned true in a mine's `blast_tracks`),
  `men_lost_to_mines` (victim's player; men dropped by a mine item's `burst`), all `saturating_add`.
* World: refusals a tick's systems raise (`game.fort_events`) now reach that tick's `fort_refusals` (before, only on
  the next applied command). Snapshot-only; not hashed.
* Numbers (`target/s10b-B/fortify10.json`, 10 seeds x 2 owners): T102a 100, T102b 75.0 (geometric coverage estimate
  78.05; the floor 75 holds at exactly 75), T102c 0, T102d 0; T103a 0, T103b 100, T103c 0, T103d 53.6 (report).
  `targets --from-json target/s10b-B --runs fortify10`: 20 pass, 5 miss (hard 0; the misses are C-E rows still null).
* fortify CLI: a driven tank keeps to its path-node line, so the AT field moves by the lateral offset, not the tank.
  Tanks are spawned with `spawn_squad_at` (a call-in of the medium tank never arrived in the fortify world).
* 40-set: `target/s10b-B/skirmish40.json` vs `target/s10b-base40`: same 40/40.
* Checks: `tools/check.sh quick 8acc534` green after the last commit (806 tests).

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
