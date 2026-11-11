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
**CLI (`26d3205`).** `s10` adds the 14 AI counters, `works_done`, kg by source, `men_lost_in_wire`; `s10_derived` the 19
fields of spec 5 (`json_out::s10_blocks`); a `fort:` text line per faction and in `pool.py`. **Tests (pass):** all of
spec 7 Build A (both path tests fail on the S10a step rule, checked). **Neutrality:** 40 set same 40/40 against
`target/s10a-base40`; all `fort:` numbers 0.

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

## Build D: owner decisions 2026-11-11 (`6578ff3`, `dacf9fb`, `71c0e4a`)

**D1 smoke.** `smoke_aim` (breach.rs): `smoke_frac_pct` toward the focus, at most half the cloud radius past the crossing;
only a man whose throw reaches it throws (no clamped short cloud). A breach squad with its own smoke stages at min(`stage_m`,
range - 1 - radius/2 - 3) = 22 m (M15 and nbhgr39: 30 m, radius 8, unchanged), arrives within 3 m, throws on the walk up
once in reach (after the base fired), never walks back (the spot moved out when the smoke man fell). Test
`ai_breach_smoke_covers_the_crossing_when_the_carrier_goes` (Build C code: cloud 30 m off, fails; control without smoke).
`ai_breach_retries_...` now counts a `CutWire` as that try's carrier (3rd try: no defender known, no base fire). T93f/h
floor 30, T93g report; T100b/d report. **fortify10: T93f 0, T93h 0 (miss), T93g 77.7, smokes 0 in all rows**: the smoke
man (the leader) falls 35-70 m out and the squad drops under 50 % (the `lost` rule) before it is in reach (traced, seed 1:
US squad 41 % at 32 m, German 25 % at 66 m). With radius 8 and one grenade a squad, 30 % needs the owner: bigger cloud,
smoke on more men, a closer start or a looser `lost` rule.
**D2 wire.** `barbed_wire` max_length 60, limit 100; `wire_len_m` 60 (TOML and default); the 25 m-out retry kept. 60 m =
30 sections, 90 kg (9 MP), inside one 160 kg carry and 3 % of the 600 MP start; 180 s of 2 men; T99 refusals 0/0. **Route
rule** (`wire_shift`, fort.rs 213): when the straight line from the point to the nearest point the AI does not hold crosses
the line over 6 m from its middle, the line slides so its end stops 6 m short (at most half its length); through the middle
(the faced approach, also the enemy's way in) it stays. Exists check reads the unshifted sites too. Test
`ai_wire_keeps_off_its_route_out` (25 degrees off: no section on the route; control straight ahead). Changed:
`ai_lays_wire_across_the_approach` (30 sections, +-31 m), `ai_marks_a_shelled_point_and_digs_a_dugout` (wire 0: the long
line held the single builder for the whole run).
**D3 cards.** `ai_weight` Pioniergruppe 40 (was 100 by default), Baupioniere 100, Sturmpioniere 60: buys Bau, Sturm,
Bau, Pioniere, ...; the German start squad stays a Pioniergruppe. US unchanged. Tests: buy shares, content weights.
**D4 switches.** `[ai.fort]` `nest`, `dugout` (also shelter), `roof`, `sandbags` (`fort::switched_on`); deviation: setup
layout change (SAVE stays 23; BASELINE, canonical snapshot refreshed). Files `{nest,dugout,roof,sandbags}_off.toml`;
`tools/ablate.sh ONDIR OUTROOT nest_off dugout_off roof_off sandbags_off` (default the five; `all` every file; check.sh
dry-runs all nine). **Found:** point wire waits for the nest (`point_wants`, fort.rs 370), so works_off (47.08 %) also laid
no point wire (pool.py: wire in 0 %) and dropped the +80 builder hold: it is "all point building off", kept as measured;
`nest = 0` alone lets the wire go ahead. Test `ai_per_work_switches_stop_exactly_their_kind` (16 scenes, all-on control).
`s5a_ai` stall guard: seed 3 USA won in 12 min with no refill trip (switches off: 4 in 14 min); now the loser of each match
and each player over the eight must make trips.
**D5 (read only).** (1) ai.rs 1818: +80 at an own point for engineers while `wants_works`; Germany's held builder was its
only SMG-and-satchel engineer (Pioniere 2 MP 40, satchel, smoke; US Engineer 1 M3, Garands); a refused or unreachable work
keeps wanting and holds it all match. (2) fort.rs 867: a builder leaves only on a *known* enemy within 80 m or under 60 %;
wire 35 m beyond the trench. (3) ai.rs 1868, fort.rs 253: one MG team per nest is pulled from its slot to a site chosen by
point-index parity, mirrored by base side (unmeasured). (4) ai.rs 1876/1984: one defender squad shelters while the proxy
mark lasts (hidden MG fire reads as shelling). (5) Costs: no German penalty (materials 100 MP/t both, 8-man builders both,
Pioniere 206 / Bau 165 vs US 280). (1) is the likeliest works_off lever; the four ablations will say.
**Numbers.** All off 40 (`target/s10a-ai-D-off`): `neutral.py target/s10a-base40` same 40/40. On 40 (`target/s10a-ai-D`):
Germany 45.0 % (18/40), mean length 20.1 min, no match without a winner; T94 95/87.5, T95 80/65, T96 0/0, T97 8.09/8.50, T98 0.99/0.95, T99 0/0,
T100 0.825/0.025 (USA report), wire 24.0/19.7 sections, works 2.33/2.00; S10a-ai.toml's 12 skirmish40 targets pass.
**Next must know.** Run `ablate.sh` with the four names on the D measure dir; T93f/h miss is the owner's call (above).
