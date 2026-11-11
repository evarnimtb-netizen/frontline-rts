# S10a-ai log (spec `S10a-ai.md`; at most 150 lines for the slice)

## Build A: layout, mechanics items, measures

**Layout commit `9f14467`** (SAVE 22 -> 23, PROTOCOL 13). `AiState::{forts: BTreeMap<SquadId, AiFortJob>, breaches:
BTreeMap<SquadId, AiBreach>, wire_known, wire_cursor, shelled, enemy_mortar_seen, bypass, material_run}`; `ai/fort.rs`
(`AiFortJob`, `FortPhase` Load/Go/Order/Watch/Done, `MAX_FORT_JOBS` 16, `MAX_FORT_TRIES` 16), `ai/breach.rs` (`AiBreach`,
`BreachPhase` Stage/Suppress/Smoke/Go/Retry/Through, `MAX_BREACHES` 8, `MAX_WIRE_KNOWN` 256, `MAX_BREACH_TRIES` 10).
`AiCounters`: `fort_jobs, fort_refused, fort_build_plans, infantry_plans, shelter_moves, material_runs: [truck, crate,
walk], breaches_started, breaches_opened, breaches_given_up, breach_smokes, breach_suppress, wire_bypassed`.
`FortAccount` (no longer `Copy`): `materials_lost_g, men_lost_in_wire, works_done: Vec<u32>` (by fort def),
`materials_used_crate_g/_truck_g`. `[ai.fort]` = `AiFortTuning` (35 keys, spec 3.7 values), `AiSetup::fort`, in
`skirmish.toml`. `AiState::validate_fort`: caps, strictly ascending `wire_known`, points/defs in range, ticks not ahead,
sites on the map, keys are the player's squads. **Deviation:** a key whose squad no longer exists loads (as escorts;
squads die between plans), so Build B must drop such jobs at plan start.
**Counters.** Lost kg: trimmed part (`trim_carried`), `lose_squad_kg` before every `squads.remove`, truck kg at
`make_wreck`, `lose_work` (kg a removed work drew). `men_lost_in_wire` in `kill`; `works_done` at a work's last step.
**S17 (`5dbcc1e`).** A* prices a step whose segment comes within band/2 of a standing section
(`FortState::step_in_wire`, exact), tested only when an end is in the coarse layer (now band/2 + 3 m,
`WIRE_NODE_SLACK_M`, > half a diagonal step); smoothing uses the same exact test. A 2 m gap (0.5 m clear) is found
when a grid step line passes it; one between node columns is not; 4 m gaps off the columns are. **Path cost** (20 m
line, 4 m gap, ignored test `wire_path_cost_report`): before 45 expansions, 14.7 us, round the line end; after 15
expansions, 14.4 us, through the gap. **Wire index:** `FortState::wire_index` (`serde(skip)`, `PartialEq` always true),
16 m cells + coarse layer, rebuilt in `serve_paths` when `(wire.len(), next_wire)` changed; a stale index is never read
(full-scan fallback), so answers never depend on it. **Side effect:** a man now goes round a 20 m line's end, so the
fortify plain-wire row stages 100 m (`WIRE_THROUGH_LEN_M`; gap variants keep 20 m); seed 2 reads 20.0 % again. The
scripted breach row (30 m wire) may path differently (report row).
**CLI (`26d3205`).** `s10` adds the 14 AI counters (`S10_AI` order), `works_done` (map by short def id),
`materials_lost_kg`, `materials_used_crate_kg`, `materials_used_truck_kg`, `men_lost_in_wire`; `s10_derived` the 19
fields of spec 5 (`json_out::s10_blocks`; `works_per_match` = non-wire works; `materials_mp_pct` over start + base
income per whole minute, as AI budgets). Text: a `fort:` line per faction. `pool.py`: a `fort:` line when present.
**Tests (pass).** `ai_fort_state_saves_loads_and_hashes`, `load_rejects_bad_ai_fort_state` (24 isolated edits),
`fort_account_counts_lost_materials`, `works_done_and_kg_by_source_are_counted`, `men_killed_in_enemy_wire_are_counted`,
`path_finds_a_2m_and_a_4m_wire_gap`, `path_never_steps_across_standing_wire_beside_a_gap` (both fail on the S10a step
rule, checked), `wire_index_matches_a_full_scan`, content `ai_fort_tuning_parses_and_validates`, CLI
`s10_ai_fields_zero_with_the_ai_unchanged`, `s10_derived_counts_matches_with_works_wire_gap`. Changed: clones of the
account (`s10a_tests.rs`, `fortify/s10a.rs`); `s10_counters_zero_without_fortifications` reads `works_done` as `{}`.
**Neutrality.** 40 set (`tools/batch.sh target/s10a-ai-A/skirmish40.json -- skirmish --ai-vs-ai --seed 1 --seeds 20
--both-bases --summary --minutes 45`): `neutral.py target/s10a-base40 target/s10a-ai-A` -> `skirmish40.json: same
40/40` (also against `target/s10a-verify`); all `fort:` numbers 0. **For B:** drop gone squads' jobs at plan start;
nothing reads `[ai.fort]` yet; no `ai_weight` changed.

## Build B: buying, materials, building (`385b369`, `492aa6f`)

**What exists.** Cards: `ai_weight` Baupioniere 50, Sturmpioniere 35, Assault Section 35; `cards = 0` skips the three
(`fort::is_s10a_card`: `build_heavy` or `breach`). Trench digging needs `dig_full` or `build_structure`; the own-point
engineer bonus skips `breach` squads and (deviation) also holds builders at an own point that still wants works
(`PointEval::wants_works`, switches on). `ai/fort.rs`: `fort_marks` (shelled, `enemy_mortar_seen`), `fort_continue`
(runs jobs before refills; job squads leave `mine`; drops gone/weak/threatened; base loads of builders under
`load_min_kg`), `fort_start` (after the hunts: nest, wire, dugout per point; HQ roof; wall per gun in Hold),
`material_truck` (called by `run_truck`), `nest_at`/`shelter_at`/`dugout_spot` (read in `order_infantry`: one MG team
per built nest, shelter, back out). Blueprints found by shape and pit, never by id. Sites exist-checked by proximity of
own works, pieces, wire and jobs (a destroyed work is rebuilt). `fort_refused` = AI Fortify/LoadMaterials/CutWire/
PlaceBangalore refused in `apply` (`ai_rejected` takes the command). Overrides: `S10a-ai-ablations/all_off.toml`.
**Deviations.** (1) Crate run NOT built (spec 11 drops it first): `material_runs[1]` stays 0, its test is missing.
(2) Walk-back is a plain Move to base (not task Refill): the Fortify order is replaced, so the mechanics remove the
work (that step's kg lost); a line resumes from its first unbuilt segment. (3) Truck run ends at `material_wait_s` +
180 s from its start (no arrival tick in the layout), when the work builds or is gone, or the load failed. (4) Shelter:
the men stop at the pit edge about 6 m from the dugout centre (path into the pit; mechanics question for Review); the
test checks the order and task. (5) `infantry_plans` and `fort_build_plans` count only with `works` or `wire` on.
(6) S9's pit habit takes engineers near a gun before a wall can (test sets `pit: true`).
**Tests (pass).** All Build B tests of spec 7 except `ai_bearers_carry_a_construction_crate_to_a_waiting_work`
(missing, crate run). `ai_fort_is_deterministic_across_threads_and_save_load` is `crates/sim/tests/ai_fort.rs`
(ignored, release pass; 1/2/3/8 threads, save at 5:00). Each "nothing happens" test has a positive control. Changed:
content `s10a_cards_resolve_with_ai_weight_zero` -> `..._with_their_ai_weights`; `empty_fort_state_changes_nothing`
and CLI `s10_counters_zero_without_fortifications`, `s10_ai_fields_zero_with_the_ai_unchanged` run with all off.
**Checks.** `tools/check.sh quick fa45332`: all ok (643 passed) on `492aa6f`.
**Neutrality.** All off (`-o all_off.toml`, `target/s10a-ai-B-off`): `neutral.py target/s10a-base40` -> same 40/40
per match; the only difference is the set header's `overrides` list (0 -> 4 items), the override's own record.
**40 set on** (`target/s10a-ai-B`, noise 11-15 points): Germany **47.5 %**; read with `targets --from-json` on a
40-set copy of the targets: T94 87.5 / 87.5 %, T95 82.5 / 62.5 %, T97 8.19 / 8.47 %, T98 1.11 / 0.97 %, T99 0 / 0,
T100 **0.05 / 0.025 (miss)**; S10a-ai.toml's 12 skirmish40 targets pass. Works 2.50 / 2.27 a match, wire 12.4 / 9.7
sections, lost kg 149 / 143 a match (GER / USA). An 8-match look (seeds 1-4): every work kind completed (nest, wire,
dugout, roof, wall), truck runs 3 / 5, walk-backs 11 / 13, own tanks crush own wire (34 / 29 sections).
**For C.** T100 misses: by the weight rule a new card is only the 2nd-3rd engineer buy and the slot buys about one a
match; the urgent breach buy (C) adds assault cards, else Tune moves the weights. US wire share 62.5 % sits near the
60 % floor. Shelter moves were 0 in the sample (marks need pinned defenders with no enemy within 150 m).
