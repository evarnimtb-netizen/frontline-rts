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

## Build C: breaching, AI breach row, ablation tooling, docs (`f5c7863`..`1403732`)

**Fixes first.** (1) `trucks` `an_ai_truck_does_not_loop_...`: not Build B's habit (fails with works/wire off too); the
drop it needed came from a defend squad the old buy mix bought; with `cards` on no drop is wanted. It now runs all off;
twin `..._with_the_fort_switches_on` (empty HQ outside the ring) proves no remembered goal is re-sent (control: breaking
`gave_up_on` fails it). Found: with switches off too, the HQ is re-ordered ~6 min later once four follow-spot give-ups
push it out of `GAVE_UP_GOALS` (S8b rule; untouched, for Review). (2) `neutral.py`: a differing set-header `overrides`
list is a note (`same 40/40 (note: overrides 0 -> 4 items)`), fixture test. (3) Shelter: men stood at the pit edge (squad
Move: formation and `seek_cover`). Now `shelter_men` sends men one by one (`MoveUnits`) to trench spots under the roof
box `cover.gap_mm` apart (so the cover rule keeps them): the shipped 3 m pit holds **2 men**; the throwers' regroup skips a
sheltering squad; back out with the squad order. Test checks positions against the roof box (control: none before).
**What exists.** `ai/breach.rs` per spec 3.4: sight (`wire_sight`), want (`breach_start`, path via `find_foot_path`),
phases, `breach_focus` (base of fire in `fire_and_move` aims at it; breach squads count toward the 3 for a 2nd base),
urgent buy in `buy` (`breach_wanted`: known wire near an attacked point, no squad that opens wire, card `ai_weight` > 0,
alive < `breach_cards_max`; skipped with `cards` 0). Sturmpioniere leader `nbhgr39` (S10a-13 (A)). `World::drive_breach(ai,
player, breach, support, point)` + `AiState::new` pub; fortify row `s10a.breach_ai` (+ `placed_pct`, `control`).
Overrides `{works,wire,breach,cards}_off.toml`; `tools/ablate.sh` (+ `--dry-run`, in `check.sh quick`). Docs: tuning.md,
fortifications.md "The AI". No layout change (SAVE 23, PROTOCOL 13).
**Deviations.** (a) Job start = `bypass[point]` (no path search while a job runs), `since` = phase start. (b) The squad
walks to its stage spot once the base fires (or 2 x `suppress_lead_s`). (c) Bangalore when a known defender is within
`cut_fire_m`, OR the base fired this try, OR the squad lost men / has a man pinned (smoke hides the defenders). (d) A
fallen or called-off carrier fails the try at once; a `CutWire` order under way is progress (a stalled cut ends at
`breach_max_s`). (e) A path through wire not yet seen decides nothing (no bypass, no recheck tick). (f) `point not
attacked` counts the breach squad's own task. (g) breach_ai row wire is **100 m** (`BREACH_AI_WIRE_LEN_M`): the path goes
round 30 m. Drive mode: both support squads are bases (assault line before a defender is known).
**Checks.** `tools/check.sh tests` on `1403732`: all ok (784 passed, 0 failed). **Tests (pass).** All 10 of spec 7 Build C, each with a control: `ai_learns_enemy_wire_only_by_sight`,
`ai_goes_round_wire_when_the_path_avoids_it`, `ai_breach_suppresses_the_nest_and_smokes_before_the_carrier_goes`,
`ai_breach_blows_with_a_bangalore_else_cuts`, `ai_breach_retries_a_pinned_carrier_then_gives_up`,
`ai_assault_goes_through_the_gap` (leaders brush the ~4 m gap's edge: in-band ticks < half the no-gap control),
`ai_buys_an_assault_card_when_a_breach_is_wanted`, `ai_breach_is_deterministic_across_threads_and_save_load` (lib),
`ai_all_switches_off_emit_nothing_new` (`tests/ai_fort.rs`, ignored), CLI `fortify_breach_ai_row_runs_the_ai_job`.
**Numbers.** All off 40 (`target/s10a-ai-C-off`): `neutral.py target/s10a-base40` -> same 40/40. On 40 (`target/s10a-ai-C`):
Germany 47.5 %, median 19.8 min; T94 87.5/87.5, T95 82.5/62.5, T97 8.15/8.46, T98 1.11/0.96, T99 0/0, T100 **0.075/0.025
miss**, T96 **0/0 miss**: breaches_started 0, wire_bypassed 4/3 in 40 matches (AI wire is 30 m: every path goes round).
fortify10: T93f **0 miss**, T93g **77.0 miss** (<= 60), T93h **0 miss**, T93i 15.8, T93j 43.0; T91/T92/T13a pass; the
scripted row unchanged (7.0/88.7/0, control 7.6/87.7). Every AI carrier falls 25-35 m short of the wire (as the scripted
row's): the stage spot is 95 m from the MG, the squad drops under 50 % before or during the first tries.
**Next must know.** T93f-h, T96 are mechanics/design limits, not wiring (spec 11): Tune knobs `wire_len_m` (paths go round
30 m), `stage_m`, `suppress_lead_s`, else S10a-16 (B). T100 misses (urgent buy rare: engineers that can cut live).
