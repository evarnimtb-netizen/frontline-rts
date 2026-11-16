# S11a1 Build A: layout, keys, solver and mask, gun fire missions, radio link, rearm switch

Read first: `S11a1.md` 2, 3, 4 (G1-G7), 5 (row "Gun mission"), 6; the log's top block. **2 chunks**, about 30
self-counted turns each (60-90 minutes in all). No AI behaviour and no ww2 unit changes in this stage: with the
content of base `730e67f` no unit has an `[indirect]` gun or a `radio` man, so the default 40 set must equal the base
match for match, and `a_off` too.
Work rules: `tools/outline.sh` then `tools/show.sh FILE::name` (batch the reads); targeted release tests through
`tools/run.sh`; `tools/check.sh quick 730e67f` once per chunk end, through `tools/run.sh`; **no command blocks longer
than 4 minutes** (`tools/run.sh LOG CMD...`, then `tools/run.sh --wait LOG`); 40 sets only through `tools/batch.sh`
with `BATCH_JOBS=4`; never quote tick times; commit only your files (`git commit -m "S11a1 A: ..." -- FILES`), never
`git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`; do not push.

Files owned: `sdk/crates/content/src/{tuning.rs,defs.rs,setup.rs,vehicle.rs}`; `crates/sim/src/{fire_mission.rs,
indirect.rs,game.rs,combat.rs,gunnery.rs,guns.rs,supply.rs,damage.rs,world.rs,hash.rs,ai.rs}` (in `ai.rs` only the
new fields and their validation), `crates/sim/src/ai/breach.rs` (field `prep_at` only), `crates/sim/src/ai/guns.rs` (the role split only),
`crates/sim/src/structures.rs` and the other holders of a kill class (the widening only), new
`crates/sim/src/ai/arty.rs` (types `AiArty`, `ArtyPhase`, `AiFo`, caps, validation; no planner code), new
`crates/sim/src/s11a1_tests.rs`; `crates/sim/tests/save_fuzz.rs`; `mods/ww2/defs/rules/skirmish.toml`;
`docs/modding/{tuning.md,fire-missions.md}`; the BASELINE files (`sdk/crates/content/tests/resolved_defs.rs`,
`crates/simbench/src/report.rs`) and the canonical snapshot; `tools/ablate.sh`, `tools/check.sh` (the S11a1
dry-run step); `docs/STATE.md` "Versions" line; `docs/slices/S11a1-log.md`.

Start from: `fire_mission.rs` (module docs, `Mission`, `FireMissionRefusal`, `linked`, `mode_for`, `start_error`,
`observe_bursts`, `remember`); `indirect.rs` (`solve_charge`, `solve`, the three tests); `game.rs`
(`mission_refusal` 1402, `order_fire_mission` 1457, the load checks at 2488-2530); `combat.rs`
(`fire_mission_tick` 1039, `end_mission`, `set_mission`); `gunnery.rs` (`gun_one` 415: the fire block at 660-717,
`lay_gun`, `main_muzzle`, `reload_of`); `guns.rs` (`is_gun_squad`, `gun_relay`, `order_pack`, `order_push`,
`order_hitch`, `gun_abandon`, `crew_ordered_off`, `Served` slots); `supply.rs` (`rearm_vehicles` 845); `damage.rs`
1558 (`base_ticks`); `sdk/crates/content/src/setup.rs` (`IndirectSetup` 885, `member_trait` 1005),
`vehicle.rs` (`VehicleRole` 365-391, the served check at 2282 and 4020), `tuning.rs` (`FireMissionTuning`,
`AiObstacleTuning` and the `tuning_table!` as the pattern for the new `[ai.arty]` table; B1-4's `hold_max` shows
every place a new key touches); S10c-ai Build A's layout commit (`git log --oneline --grep "S10c-ai A"`) as the
pattern for a SAVE bump.

## Chunk 0 (before any edit, 5 minutes): the base set
`tools/run.sh target/s11a1-base40.log env BATCH_JOBS=4 tools/batch.sh target/s11a1-base40/skirmish40.json -- skirmish
--ai-vs-ai --seed 1 --seeds 20 --both-bases --summary --minutes 45` on the unchanged tree (release CLI built from
`730e67f`; the cache may already hold it). Log Germany's wins of 40 and the file's path.

## Chunk 1: one layout commit, keys, solver
1. Layout commit (nothing else in it): `SAVE_VERSION` 26 -> 27; the kill-class carrier widened to `u16` (spec 3
   "Vehicle type cap": `game.rs` `kill_class` 540-573, every store and pass of it; test
   `seventeen_vehicle_types_keep_their_kill_rows`); every saved field of spec 3 at its zero default
   (`S11Counters`, `AiState::arty`, `AiState::fo`, `AiState::fo_lost_at`, `AiBreach::prep_at`, the `AiCounters` fields, `FireMissionRefusal`
   `Masked`, `NotReady`); load validation and hash for each; `save_fuzz.rs` staged save gains one `AiArty`, one
   `AiFo` and a `prep_at`; `load_rejects_bad_s11a1_state` (one case per check). `PROTOCOL_VERSION` stays 14: say so
   in the commit text. `STATE.md` "Versions" line.
2. Keys of spec 2 with ranges, comments in `skirmish.toml`, rows in `docs/modding/tuning.md`: `[fire_mission]`
   (`gun_missions` 1, `radio_link` 1, `mask` 1 and the seven values), `[supply] arty_base_rearm` 1, new table
   `[ai.arty]` (`buy`, `fo`, `prep` 0 and the values). Content schema: `[indirect]` `low_angle`, `range_m`,
   `spread_mrad`; member trait `radio` (bit 32, `NAMES`, `ALL`); `VehicleRole::InfGun` ("inf_gun": served like
   `AtGun` in the two checks of `vehicle.rs`, in the vehicle-role group of `ai.rs` 1073 so the mix never buys it, in
   `VehicleRole::outside_limit`). `ai/guns.rs` `take_guns` 400-405, `gun_jobs` 448-455 and `gun_recrews` select by
   `vs.served` today: split them by role `AtGun` now (test `at_gun_ai_ignores_infantry_guns` with a test-only
   `inf_gun` def; an `inf_gun` then stays in the AI's `tanks` list: check no armour habit orders it, else park it
   in an empty list until Build C). `load_rejects_bad_s11a1_keys`.
3. Override files exist (Plan) under `docs/slices/targets/S11a1-ablations/`: make `tools/ablate.sh --dry-run`
   accept them (`ABLATIONS=docs/slices/targets/S11a1-ablations`; the allowed keys and values are those six files)
   and add the step to `tools/check.sh quick` beside the S10c one.
4. G2: `indirect.rs` low root, candidate order, mask test; the four G2 tests. The three existing tests must pass
   without an edit.
**STOP POINT 1.** `check.sh quick 730e67f` green; commit; rewrite the log's top block (commits, what chunk 2
needs, anything of spec 3 that had to differ); return if more than 45 self-counted turns are used.

## Chunk 2: gun missions, radio, ends, rearm
5. G1, G3, G4, G6 in this order, each with its tests (spec 4). G3 includes: the load check of `game.rs`
   2484-2531 for a gun squad's mission and ranging cells, `end_mission` using the gun's position, the mission
   ttl formula, hull-pitch laying, and the counter split (an `inf_gun` only in `S11Counters`). Test scenes use a test-only gun def built in
   `s11a1_tests.rs` (the Pak 38 def with an `[indirect]` table, band 0..73, `low_angle` true; a second with band
   0..30): ww2 gets its guns in Build B. Read every `match` on `sq.order` that a vehicle squad reaches (spec 10
   risk 3) and list in the log the ones that needed a `FireMission` arm.
6. G5 radio link with its two tests (a test-only observer squad with `observer` and `radio` men).
7. G7 `arty_base_rearm` with its test; `gun_shell_micro`.
8. Counters of `S11Counters` filled where the events are (no JSON yet: Build B).
9. Neutrality: `tools/run.sh target/s11a1-A40.log env BATCH_JOBS=4 tools/batch.sh target/s11a1-A40/skirmish40.json
   -- skirmish --ai-vs-ai --seed 1 --seeds 20 --both-bases --summary --minutes 45`, the same with `-o
   docs/slices/targets/S11a1-ablations/a_off.toml` into `target/s11a1-Aoff40/`; `python3 -I tools/neutral.py
   target/s11a1-base40 target/s11a1-A40` and `... target/s11a1-Aoff40`: both must print `same 40/40`. A
   difference is a bug in this stage (a base unit took a new path): find it, do not explain it away.
Tests that must exist at the end (each names its control in a comment): the G1-G7 tests of spec 4,
`load_rejects_bad_s11a1_state`, `load_rejects_bad_s11a1_keys`, save roundtrip and fuzz roundtrip green.
**STOP POINT 2.** `check.sh quick 730e67f` green (it applies `cargo fmt`; run it last); commit; rewrite the log's
top block.

## The stage must log (in `S11a1-log.md`, top block rewritten, at most 30 lines)
Commits; SAVE 27 / PROTOCOL 14; test count from `check.sh`; `neutral.py` lines for A40 and Aoff40; a code map of
the new functions (`solve_gun`, `gun_mission_refusal`, `gun_mission_tick`, the firing-unit helper of `linked`);
the `sq.order` arms added; any rule built differently from spec 4, with the reason; what Build B needs (how a
test builds a gun mission scene; the exact TOML shape of `[indirect]` for a gun; whether `FmCounters::refused`
is printed as an array). Not done items by rule id.
If a step cannot be finished: leave its switch at the off value, keep the neutrality proof, and say so first in
the top block. Never leave the tree with a red `check.sh quick`.
