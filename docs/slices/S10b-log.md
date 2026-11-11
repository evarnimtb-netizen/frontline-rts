# S10b log (mechanics: mines, AT obstacles, bunker; at most 150 lines)

Spec: common `docs/slices/S10b-obstacles.md`, stages `S10b-build-A.md` ... `-F.md`; targets `targets/S10b.toml`.
Branch `s10b-obstacles` from main `8acc534`. Two other agents may work in this checkout: commit only your files.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)

**Next: Build A, chunk 2** (finish `S10b-build-A.md` step 5), then Build B. Chunk 1 stopped at the turn budget.
* Done: layout `ba7478b` (SAVE 24, PROTOCOL 14; no stage changes either again), data and R29 guard `9a3d197`,
  measures `20ade12` (`factions.<f>.s10b` + `used`, `fortify10.s10b` rows as nulls, `World::lay_mine_now`).
* Left (step 5, sim tests in a new `crates/sim/src/s10b_tests.rs`, add `#[cfg(test)] #[path] mod` in `world.rs` beside
  `s10a_tests`): `s10b_layout_roundtrips_and_hashes`, `empty_s10b_state_changes_nothing`,
  `fort_of_ignores_s10b_blueprints`; `load_rejects_bad_s10b_state` in `crates/sim/tests/save_fuzz.rs` (one case per
  spec 6 line; needs a pub `World::edit_squad` staging tool for the order cases, and a setup whose squad types carry
  bits 12-14 since no card has them yet); CLI `s10b_counters_zero_without_use` (2-match skirmish, `s10b.used` 0, key
  present). Each test names its control and must be seen failing once. Content tests are done (see section below).
* Load checks live in `FortState::validate_s10b` (`fort.rs`); `validate` now takes `tick` (for `boom_tick == tick+1`).
  Lift and place totals: `lift_s * tick_hz * 100`, `demolish_place_s * tick_hz * 100` (work > total is rejected).
* New S10b `FortAccount` counters are not in `counters_ok` (spec 6: any value): bump them with `saturating_add`.
* `guns.rs` (served-crew squad-order list near line 1033) does not list `ClearMines`/`Demolish` yet: C and D add them
  when the commands are accepted. `CommandKind::{ClearMines, Demolish}` refused `NotAllowed` in `game.rs` dispatch.
* Protocol `command_in_limits`: `ClearMines` lane <= `MAX_LANE_M` 200 m (the key's top); map bounds and the match's
  `lane_max_m` are sim checks (C), since the protocol has no map size.
* AI guard sites (step 3): only `ai/fort.rs::fort_of` picks blueprints; other `setup.forts` uses index by a known
  fort (`ai.rs:853`, `ai/fort.rs` jobs, `ai/breach.rs:300` wire layer). No `caps ==`/`count_ones` test in `ai/`; no
  RNG key from an item or fort index (`ai/breach.rs` keys by player). Item and fort indices shift (mines and new
  blueprints sort first); the 40 set is still same 40/40.
* 40-set: `target/s10b-A/skirmish40.json` vs `target/s10b-base40`: same 40/40 (after `20ade12`). Targets parse on
  `target/s10a-ai-verify`: 331 pass, T102-T108 "no such number".
* Checks: `tools/check.sh quick 8acc534` while working; long jobs through `tools/run.sh` (200 s polls).

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

