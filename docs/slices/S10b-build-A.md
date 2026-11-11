# S10b Build A: layout, data, measures, scaffolding (no behaviour)

Read first: `S10b-obstacles.md` 3, 4 (R1-R2, R14-R15, R22-R25, R29-R30), 6; the log's top block. About 60 turns.
Use `tools/outline.sh` and `tools/show.sh` for ranges of `fort.rs`, `game.rs`, `command.rs`, `setup.rs`.

## 0. Neutrality base (first step, no build)
`mkdir -p target/s10b-base40 && cp target/s10a-ai-verify/skirmish40.json target/s10b-base40/`. Check that
`git diff --stat 72fd940 8acc534 -- crates sdk mods` shows only `s10a_ai_tests.rs` (so the verify runs are main's).

## 1. The layout commit (one commit, SAVE 23 -> 24, PROTOCOL 13 -> 14; no later stage changes the layout)
Every field, variant and key of spec 3, defaulted and validated, behaviour off:
* sim `fort.rs`: `Mine`, `FortState::{mines, next_mine}`, `MAX_MINES` 2048, `Charge::piece`, the twelve
  `FortAccount` counters, `FortRefusal::NoTarget`, `is_empty` includes `mines`; `FortState::validate` with every
  spec 6 rule. `game.rs`: `Order::{ClearMines, Demolish}` and `OrderKind` (appended), their spec 6 checks where
  orders are validated. `command.rs`: `CommandKind::{ClearMines, Demolish}` appended, both refused `NotAllowed` (C
  and D accept them). `rng`: purpose `MineDetect` appended.
* content: `ItemKind::Mine = 5`, `ITEM_KINDS` 6 (every `[_; ITEM_KINDS]` array, snapshot and bridge included);
  `ItemSetup::mine: Option<MineSetup { trigger, trigger_cm }>`; `DigProfile::BunkerPit = 8` and
  `RulesSetup::bunker_pit`; `Capability::{PlaceMines = 12, ClearMines = 13, Demolish = 14}` out of
  `RESERVED_CAPABILITIES`, `CAPABILITIES` 15; `FortDef`/`FortSetup` `mine_item` (lowered to `Option<u16>` item
  index), `blocks_vehicles`; pieces 1..=6; `PIECE_PARTS` gains `steel` (appended, hedgehog colour); `FortTuning` twelve keys (`tuning::TABLES`, schema).
* `world.rs` `SAVE_VERSION` 24; `crates/net/src/protocol.rs` `PROTOCOL_VERSION` 14, `command_in_limits` arms
  (`ClearMines`: both points on the map, length <= `lane_max_m`; `Demolish`: point on the map).
* Commit message names the bump. Log: the commit id and the list of fields.

## 2. Data (same stage, after the layout commit; values from the rules table, comments mark `verify`)
* `mods/ww2/defs/fortifications/`: `ap_minefield.toml`, `at_minefield.toml`, `hedgehog.toml`,
  `log_barrier.toml`, `bunker.toml` (pieces of R23: roof `z_from = "base"` at wall height, slit between the two
  front walls). `mods/ww2/defs/items/ap_mine.toml`, `at_mine.toml` (R5, R6; `[he]` keys as `geballte_ladung`).
* `defs/rules/skirmish.toml`: `bunker_pit` (dig profile like `dugout_pit`) and the `[fortify]` keys with values.
* Validators: mine item rules (spec 3); `mine_item` names a `mine` item; a minefield has no pieces and no `wire`;
  `blocks_vehicles` needs pieces; `build_heavy` only on point or line defs with pieces.
* No card changes here (caps come with behaviour, R27).
* BASELINE refresh: `sdk/crates/content/tests/resolved_defs.rs`, `crates/simbench/src/report.rs`.

## 3. AI guard (R29)
`ai/fort.rs` `fort_of`: `Wall` and every other kind skip defs with `mine_item` or `blocks_vehicles`; `Dugout`
already needs `DugoutPit` (the bunker has `BunkerPit`). Grep `ai/` for any other `setup.forts` scan, any test of
whole capability masks (`caps ==`, `count_ones`) and any RNG key built from an item or fort index; note each in the log.

## 4. Measures
* CLI `sdk/crates/cli/src/skirmish.rs` and `json_out.rs`: `factions.<f>.s10b` from the twelve counters plus
  `used` (= `mines_laid + obstacles_built + bunkers_built + sweeps + demolitions`); all 0 in AI play. A `s10b:`
  text line per faction only when non-zero (keeps the old text equal).
* `sdk/crates/cli/src/fortify/s10b.rs` (new, beside `s10a`): `S10bReport` with the six rows `atmine`, `apmine`,
  `sweep`, `obstacle`, `demolish`, `bunker`, every field of the T102-T107 metrics as `Option<f64>` (null until its
  stage), the `control` block for `bunker`; added to `FortifyReport` as `s10b`; summary text lines. Staging helper
  `World::lay_mine_now(fort, pos, owner)` (test and CLI only, like `build_now`).
* `frontline-cli targets docs/slices/targets/S10b.toml --from-json target/s10a-ai-verify` must parse (T102-T108 read
  "no such number" until filled).

## 5. Tests (each names the control that makes it fail; sim tests in `crates/sim/src/s10b_tests.rs`, new)
* `s10b_layout_roundtrips_and_hashes`: a state with a mine (known bits, `boom_tick`), a demolition charge, both new
  orders saves, loads and hashes equal; changing `Mine::known` changes the hash. Control: skip `mines` in the
  serialised state -> load differs.
* `load_rejects_bad_s10b_state`: one case per spec 6 line (extend `crates/sim/tests/save_fuzz.rs`, the roundtrip-and-fuzz
  harness, once it is committed on the branch; another agent is writing it). Control: comment out any one check -> its case loads.
* `empty_s10b_state_changes_nothing`: 600 AI ticks, entity state equal with and without the new data in the setup.
  Control: make `is_empty` ignore `mines` and add one far mine -> the fast path is skipped (assert counts it).
* `fort_of_ignores_s10b_blueprints`: `Wall` resolves to `sandbag_wall`, `Dugout` to `dugout`. Control: remove the
  R29 filter -> `Wall` is `ap_minefield`.
* `s10b_defs_validate` (content): the five defs and two items pass; a mine item in a kit, a minefield with pieces,
  seven pieces, `mine_item` naming a frag fail. Control: each bad def alone.
* `promoted_capabilities_parse`: the three names parse to bits 12-14. Control: leave one in `RESERVED_CAPABILITIES`.
* CLI `s10b_counters_zero_without_use`: a 2-match skirmish has `s10b.used` 0 and the key present.

## 6. Ends when
`tools/check.sh quick 8acc534` green; the layout commit and the data commit made; 40-set neutrality:
`tools/measure.sh` not needed, run `tools/batch.sh target/s10b-A/skirmish40.json -- skirmish --ai-vs-ai --seed 1
--seeds 20 --both-bases --summary --minutes 45` in the background (`tools/run.sh`) and
`python3 -I tools/neutral.py target/s10b-base40 target/s10b-A` -> `same 40/40`.

## 7. Hand-over (rewrite the log's top block)
Layout commit id, versions, the field list, the guard sites found in step 3, the 40-set result, any deviation.
