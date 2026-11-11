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
**S17 (`5dbcc1e`).** A* prices a step within band/2 of a standing section (`FortState::step_in_wire`, exact; coarse
layer band/2 + 3 m); smoothing too. 2 m gaps on a grid step line and 4 m gaps are found (cost test
`wire_path_cost_report`: 45 -> 15 expansions). `FortState::wire_index` (`serde(skip)`, never read stale). The fortify
plain-wire row stages 100 m (`WIRE_THROUGH_LEN_M`: men go round a 20 m line's end).
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
**Deviations.** (1) Crate run NOT built: `material_runs[1]` stays 0, its test is missing. (2) Walk-back is a plain
Move to base: the mechanics remove the work (that step's kg lost); a line resumes from its first unbuilt segment.
(3) Truck run ends at `material_wait_s` + 180 s from its start. (4) Shelter: see C (3). (5) `infantry_plans` and
`fort_build_plans` count only with `works` or `wire` on. (6) S9's pit habit takes engineers near a gun first.
**Tests (pass).** All Build B tests of spec 7 except `ai_bearers_carry_a_construction_crate_to_a_waiting_work`
(missing, crate run). `ai_fort_is_deterministic_across_threads_and_save_load` is `crates/sim/tests/ai_fort.rs`
(ignored, release pass; 1/2/3/8 threads, save at 5:00). Each "nothing happens" test has a positive control. Changed:
content `s10a_cards_resolve_with_ai_weight_zero` -> `..._with_their_ai_weights`; `empty_fort_state_changes_nothing`
and CLI `s10_counters_zero_without_fortifications`, `s10_ai_fields_zero_with_the_ai_unchanged` run with all off.
**Neutrality (how every stage runs it).** `tools/batch.sh DIR/skirmish40.json -o
docs/slices/targets/S10a-ai-ablations/all_off.toml -- skirmish --ai-vs-ai --seed 1 --seeds 20 --both-bases --summary
--minutes 45`, then `tools/neutral.py target/s10a-base40 DIR` -> same 40/40 (note: the header's `overrides` list).
**40 set on** (`target/s10a-ai-B`): Germany 47.5 %; T100 0.05 / 0.025 (miss). Every work kind completed in an 8-match
look; own tanks crush own wire (34 / 29 sections).

## Build C: breaching, AI breach row, ablation tooling, docs (`f5c7863`..`1403732`)

**Fixes first.** (1) `trucks` `an_ai_truck_does_not_loop_...` runs all off; twin `..._with_the_fort_switches_on`
proves no remembered goal is re-sent. Found: the HQ is re-ordered ~6 min later once four follow-spot give-ups push it
out of `GAVE_UP_GOALS` (S8b rule, also all off). (2) `neutral.py`: a differing `overrides` header is a note.
(3) Shelter: `shelter_men` sends men one by one (`MoveUnits`) to trench spots under the roof `cover.gap_mm` apart: the
shipped 3 m pit holds **2 men**; back out with the squad order.
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
**Checks.** `check.sh tests` on `1403732` all ok (784). **Tests.** The 10 of spec 7 Build C, each with a control (lib
`ai_learns_enemy_wire_...`, `ai_goes_round_wire_...`, `ai_breach_*`, `ai_assault_goes_through_the_gap`, `ai_buys_an_assault_card_...`;
`tests/ai_fort.rs` `ai_all_switches_off_emit_nothing_new`, ignored; CLI `fortify_breach_ai_row_runs_the_ai_job`).
**Numbers.** All off 40 (`target/s10a-ai-C-off`) same 40/40. On 40 (`target/s10a-ai-C`): Germany 47.5 %; T94 87.5/87.5,
T95 82.5/62.5, T97 8.15/8.46, T98 1.11/0.96, T99 0/0, T100 **0.075/0.025 miss**, T96 **0/0 miss**: breaches_started 0,
wire_bypassed 4/3 (30 m wire: paths go round). fortify10: T93f/h **0 miss**, T93g **77.0** (<= 60), T93i 15.8, T93j 43.0;
T91/T92/T13a pass. AI carriers fall 25-35 m short of the wire (under 50 % in the first tries). **Next:** T93f-h, T96 are
mechanics limits (spec 11): `wire_len_m`, `stage_m`, `suppress_lead_s`, else S10a-16 (B); T100 misses.

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

## Fix (review `target/s10a-ai-review.json`; `723618d`..)

Each fix has a test that failed on the code before (run on a detached copy of `723618d`). **F1** `FortState::validate`
`checked_add`; the account counters saturate. **F15** `fort_holds`: the +80 goes to one builder per wanting point that
could start now (room, 60 %, capability, carry or free truck, no threat; nearest); T97 adds a held builder left without a
job. **F12** a known enemy vehicle or soldier within `threat_radius` of the builder or its site ends the job (free: it
throws, hunts). **F13** Watch: `since` = the step/segment last moved (or the leader still walking, under half pinned),
`tries` = the step; 3 x the step's time + 60 s still ends it; a lost or threatened point ends any job. **F14** a step
the squad cannot carry and no free truck: no job, no walk. **F4/F17** a refused work is parked 300 s (`REFUSED_S`; Done,
`tries` = 16, squad free; the job counts as the work existing); works over `limit_per_player` are never asked; the wire is
at most the blueprint's length. **F5** at most 2 dugouts. **F3** a new job counted once. **F2** the run deadline is read
every plan. **F18** a line's run lasts the line (truck parks on the base side: it crushed the line driving along it) and
loads less what it holds. **F6** the Idle dugout arm needs a built dugout. **F19** no MG or AT team shelters. Constants,
not keys (no setup change). **F7** s5a sums trips by faction; only a winner under 15 min may skip. **F10** T96 counts
matches where the enemy's wire stood on a route (bypassed or a breach started); none: null, `targets` SKIP; same for
`hq_roofed_pct`. **F11** `ablate.sh` fails without `NEUTRAL_BASE` or on a non-same verdict. **F9** doc weights.
**Left open: F8** (the twin counting HQ orders fails: 2 in 7 min, the S8b `GAVE_UP_GOALS` eviction, all off too; a fix
changes all-off play). Changed tests: CLI synthetic T96 50 -> 100, all-off derived nulls. **Numbers:** all off 40
(`target/s10a-ai-F-off`) same 40/40; on 40 (`target/s10a-ai-F`): Germany 45.0 %, T97 7.6/8.6, T99 0/0.025, 12/12 pass.
## Smoke on three men (owner 2026-11-11; `ab90c3d`)
Sturmpioniere: leader + 2 StG men (slot `sturmpionier_stg_nebel`) one nbhgr39 each; Assault Section: leader + 2 riflemen
(`rifleman_smoke`) one M15 each. `breach_smoke`: any unpinned man with smoke, nearest the aim first. Test
`ai_breach_smoke_outlives_the_leader` (fails on `7bf0494`). Canonical snapshot blessed (BASELINE unchanged). All off 40
(`target/s10a-ai-G-off`) same 40/40; on 40 (`target/s10a-ai-G`) same 40/40 as F, Germany 45.0 %. **fortify10 still T93f 0,
T93h 0, T93g 77.4, smokes 0**: `lost` (<50 %) ends every job in Stage, US carriers pinned/killed 27-38 m out, German 69-95 m.
