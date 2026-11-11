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
