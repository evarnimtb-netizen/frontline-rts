# S10c-ai log (defences at the front; at most 150 lines)
Spec: common `docs/slices/S10c-ai.md`, stages `S10c-ai-build-A.md`, `-B.md`, `-C.md`; targets `targets/S10c-ai.toml`.
Branch `claude/project-thread-e7nsak` from main `54009f6`. Another agent may work in this checkout: commit only your
files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)
**Fix pass 2 done (2026-11-16): C-2, C-3, C-4, C-6, C-7, C-8, C-9 fixed; C-5 NOT fixed (it needs a saved AT line,
a layout change; see "Fix pass 2"). Next: verify.**
- Defaults unchanged (`front` 1, `front_home` 0, `jobs_max` 2, `front_slots` 1, `mine_assign` 1, `builder_cards_min`
  2, `belt` 1); no key, save field or layout change (SAVE 26, PROTOCOL 14). Neutral (release CLI rebuilt):
  `python3 -I tools/neutral.py target/s10c-baseA40 target/s10c-F2off40` -> `skirmish40.json: same 40/40 (note:
  overrides 1 -> 5 items)`.
- Default 40 set `target/s10c-F2-40` (DE / US), Germany 47.5 %: mines 75.0 / 87.5 % of matches; front share 35.5 /
  16.2 %; approached 50.0 / 17.1 %; set off 0.475 / 0.075; AT fields 0.325 / 0.425 a match; tanks stopped 0 / 0; belt
  7.5 / 2.5 % (belt jobs 3 / 1, route rejects 0 / 0); wire seen by enemy 47.5 / 13.9 %; knew enemy wire 12.5 / 47.5 %;
  breaches 0 / 0; front builders lost 0.45 / 0.625; builder buys 1.925 / 2.275; MG nests 0.90 / 0.825; works 100 /
  100 %; wire 100 / 90.0 %; defence jobs failed 10.6 / 9.8 %; own_blocked 0 / 0; `jobs_overdue` 0 / 0. Home works
  floor vs baseA40 (0.825 / 0.725, 95.0 / 92.5 %): holds.
- Open: (1) C-5. (2) C-7's reach (`sweep_reach_m` 300) cuts the C2 pick: vs F1-40 front share 55.1 / 37.7 -> 35.5 /
  16.2 %, US set off 0.35 -> 0.075 (T119, T121); diagnostic `target/s10c-F2reach40` (`sweep_reach_m` 1000, sweeps too):
  52.8 / 40.4 %, set off 1.0 / 0.2, but US front builders lost 1.0 (bound 0.5); a reach key of its own is a setup
  change: orchestrator. (3) `front_why` has no AT entry (saved `[u32; 12]`). (4) AT row behind FrontAp ("Fix pass 1").
  (5) R6b not built; S10c-2 waits for the owner.

## Plan (2026-11-16)
Cause chain (spec 0): `trench_done` gate, nest -> wire -> AP order, `fort_threat` (one known enemy within 80 m of
builder or site), trench-frame sites, about one builder squad a match, `jobs_max` 1, 12 m rows. Design: front = the
two own points nearest the enemy base (no map analysis); lanes by one path search per point per `recheck_s` (foot:
from the nearest enemy-owned point; vehicle: R2); hasty AP field, wire, AT field with no trench and ahead of the nest;
site threat 50 m for front jobs only (builder 80 m); a threat drop is no refusal, the rest is wanted again; belts
beyond the AT field so the way round crosses the mines; belt own-route safety by a detour check, not R6's 10 m. Not
designed in: laying under fire, chokepoints, a detour-cost demolition want. Limits: on the open map a 20 m belt never
leaves "no way round" (R14 rare, T126 a report); one builder squad a match makes front and rear works compete (F4:
front first; `builders2` measures a second); B1: fortifying helps the USA more, so the guard band is watched.

## Build A and Build B (2026-11-16, shortened)
- A2 layout `6cd959c` (`front_lanes`, `front_slots`, `front_wire_len_m` 20; SAVE 26); observers, JSON, `neutral.py`
  skip, `ablate.sh` `233f9b7`; observer cost +0.0 to +0.4 %. Deviations: observer test places the enemy squad; a mine
  laid and gone within a sampled second is not seen; extra raw keys; approach counts soldiers on foot.
- B1 `571e841`: F1-F4, F7, F10, `front_why`, `front_jobs`; neutral `B1off40` same 40/40. F10: a FrontAp job from a
  base 590 m away took 312 s. Deviations: US engineers in tests; lane test against the foot path; no detour finding.
- B2 `71b8bdd`: F5, F6, F8, determinism, liveness, fuzz, `front` 1, `front_home`; controls seen. Deviations: threat a
  player-0 truck; F5 lifts `build_threat_max` for front works; resume may finish the mine in hand; liveness row blocks
  with an own tank; S10a/S10b scene tests moved to `front` 0.
- B2 40 set (DE / US), Germany 50.0 %: front share 12.7 / 18.1 %; approached 11.1 / 17.9 %; works 97.5 / 80.0 %;
  nests 0.35 / 0.225 (`c_off` 95.0 / 92.5 %, 0.825 / 0.725). `front_home_on` Germany 47.5 %.

## Build C chunk 1 (2026-11-16)
- Orchestrator additions: (1) F1, `front_home` 0: the home point is a front point only while the enemy owns over half
  the points. (2) C2's pick reads every free squad (`mine`), as `bunker_assign`; the hold bonus no longer decides it,
  but hunts and ambushes run first, and assign sends the builder back to the nearest own untrenched point (+80);
  no_builder 23209 / 25080 (`mine_assign_off`) -> 16181 / 20804 (= "no fit free builder anywhere").
- C1 `jobs_max` 2 (`skirmish.toml`, snapshot, content tests; BASELINE unchanged: it hashes the frozen S3c fixtures).
  C8 `front_slots` gate in `point_starts`, `fort_holds`, `front_block`. C3: "a habit want exists" = fort habits on,
  build gate open, an own point (a want flag would be unsaved). Card: cheapest roster card with `place_mines` and
  `build_structure`, no `breach`, `ai_weight` > 0 (S10a cards need `[ai.fort] cards`); no mix slot needed.
- Tests (control in brackets, seen failing): `ai_two_s10b_jobs_run_at_once` [jobs_max 1: 1], `ai_front_jobs_leave_
  a_slot_for_home_works` [front_slots 2: 2], `ai_mine_assign_sends_a_builder_to_the_front` [0: no job, no_builder],
  `ai_buys_a_builder_under_the_floor` [0: rifles], `ai_builder_floor_never_overbuys` [alive count ignored: 3 buys],
  `own_blocked_pair_is_counted_once` [old list: 2], `ai_bunker_yields_a_slot_with_two_slots` [bunker_yields 0: 1 job
  beside the bunker; `builders_max` 4], F1 home case [`front_home` 1: [C, B]].
- B1-2 cause (C6b): seed 104 reads 0 on main now; B40 had DE 31 in seed 15 normal: one Opel Blitz (truck job Load /
  Return, goal the own base centre) against two pieces of an own log barrier 2-3 m from the base centre (base 0),
  coming and going as it shunted (recount: 2). R6 checks base -> point -> `route_out` only, `own_route` was most
  likely None (no vehicle path from the base centre) and R6 passes every site then. Proposed R6b (own key, off in
  `c_off`): no blocking piece within `own_route_m` + `wreck_pad_m` of the base centre; a failed search refuses.
- Deviations: (1) `mine_assign` default 1 already in chunk 1. (2) Tests on a lone held point or C alone take
  `front_home` 1 (`ai_front_wants_come_before_the_nest`, `..._from_base_ends_inside_its_deadline`, liveness FrontAp).
  (3) Determinism: second save at the first live front job from the end of the first save's replay (seed 1: none
  9:52-24:52; saves 8:22, about 8:52), window 15 min. (4) `yields_run` pins `jobs_max` 1. (5) No key for the C6
  recount (a counter only; `c_off` same 40/40).
- 40 sets (DE / US). Default `C1a40`, Germany 40.0 %: mines 70.0 / 50.0 %; home/front 218/243, 171/117 (front share
  52.7 / 40.6 %); approached 42.9 / 20.0 %; set off 0.475 / 0.25; wire seen 62.1 / 34.8 %; knew 20.0 / 45.0 %; breaches
  0 / 0; front jobs 5.45 / 4.33; lost 0.60 / 0.55; why no_builder 16181 / 20804, slot 5390 / 3870, no_front 4730 /
  4995; buys 0 / 0; nests 0.70 / 0.525; works 87.5 / 77.5 %; wire 72.5 / 57.5 %; own_blocked 0 / 0. `builders2` `C1b40`,
  Germany 42.5 %: mines 95.0 / 82.5 %; 398/510, 342/285 (56.2 / 45.5 %); approached 63.2 / 51.5 %; set off 0.675 /
  0.375; wire seen 45.9 / 54.1 %; knew 50.0 / 42.5 %; breaches 0 / 0; jobs 8.45 / 9.35; lost 0.65 / 0.80; why slot
  7070 / 6602, no_builder 5789 / 5704, no_front 4730 / 5001; buys 1.7 / 1.925; nests 0.90 / 0.80; works 100 / 95.0 %;
  wire 92.5 / 92.5 %; own_blocked 0 / 0. `mine_assign_off` `C1c40`, Germany 45.0 %: mines 45.0 / 30.0 %; front share
  31.9 / 13.0 %; nests 0.775 / 0.55.

## Build C chunk 2 (2026-11-16)
- Commits: `a641988` `builder_cards_min` 2 (orchestrator; `s10c_ai_layout_roundtrips_and_hashes` now sets 3, seen
  failing first; `builders0.toml`); `0130677` belt code and tests; then defaults, CLI row, docs. C4/C5: spec rows; belt
  = front job (orchestrator item 2); R6b not built (item 3).
- Tests [control]: `ai_builds_a_belt_before_its_at_field` [`belt` 0: no piece]; `ai_one_belt_per_lane` [no
  `!front_live` in `belt_wanted`: 2 jobs at once]; `ai_belt_leaves_the_own_way_round` [`belt_detour_pct` 0: tank not
  out]; `ai_belt_deterministic_threads_and_save_load` [job `start` edited: "at 1 threads" fails]; liveness row Belt
  [`refused_s` 0: past its deadline]; fuzz stage "ai_belt_watch" (17 stages).
- Deviations: (1) tests feed the builders (staged refill) and take `bunker` 0; (2) the way-round control asserts "not
  out", not `own_blocked` > 0; (3) the belt determinism test is staged; (4) the liveness row takes `fields_per_point`
  0; (5) `front_carry_ok` counts one step for a point blueprint; (6) the fortify row stages the lane done, takes
  `front_home` 1 and fed 160 kg a second (US log line not built, its control read 0; fix pass 2 C-3 adds a truck).
- 40 sets (`BATCH_JOBS=4`, world `85eeb1da`; DE / US): **C40** (`target/s10c-C40`) = chunk 1's `builders2` C1b40 row
  (Germany 42.5 %) and: AT fields 0.025 / 0; tanks stopped 0 / 0; belt 0 / 0 %; obstacle approached 0 / 0 %;
  demolitions wanted 0 / 0; breaches 0 / 0; own_blocked 0 / 0; own-side mistakes 0 / 0. **belt_off**
  (`target/s10c-C2belt0`): same 40/40 as C40. **builders0** (`target/s10c-C2b0`) = chunk 1's default C1a40 row
  (Germany 40.0 %) and the same zeros (AT fields, stopped, belt, demolitions, breaches, own_blocked, mistakes).

## Fix pass 1 (2026-11-16; findings in `/tmp/claude-0/s10c-open-findings.md`)
- A-1 `mine_block`: laid field or running job not counted, before the trench/threat check (`mine_want_why_skips_a_
  laid_field_at_a_threatened_point`, control old order: 15). A-2: order in `MINE_WANT_WHY` doc. A-3 `ablate.sh`: a key
  counts only echoed or at the mod default (control: 1 of 2 -> 1). A-4 `load_rejects_lanes_over_the_cap_alone` (8 load,
  9 refused). A-5 CLI `s10c_observer_counts_enemy_soldiers_and_vehicles_only` (own squad, soldier at AT mine: 0; tank
  20 m: 2; 60 m: 0); `s10b_counters_zero_without_use` also runs the shipped defaults (no control).
- B-1 FrontWire keeps L1, L3 (refusal on a refused order or a Watch end not built) and L4 (`refused_here` in
  `point_starts`, `front_block`, `fort_holds`); no parking. In C40 136 of 247 FrontWire ends were re-ordered within
  10 s. `front_wire_row` + `front_wire_job_ends_and_is_not_reordered`: 2-5 refusals in 20 min [`refused_s` 0 or old
  code: alive to the end]. Deviation: FrontWire failures write the refusal only; the test hook drives FrontWire.
- B-2 `fort_near_of(.., rear)`: front lines and jobs do not count as the point's 60 m wire or R1 field
  (`front_field_does_not_stand_in_for_the_r1_field` [no R1 job]). B-3 `fort_holds` checks R8 room, cap, L4
  (`a_refused_front_work_holds_no_builder` [held 30 plans]). B-4 "at the site" from the line's nearest point
  (`front_threat_at_site_is_measured_from_the_line`). B-5 load refuses lanes with `front` 0 (`load_rejects_front_
  lanes_with_front_off`).
- C-1 diagnosis (C40 code, temporary prints): the front AT row was wanted on 1169 recheck plans, started 9 times (of
  712 front starts). AP or wire not done on 1030; slot held on 481. Slot free (688): builder under 60 % 296, no builder
  180, F10 no carry 72 (0-81 kg held of 90), threat 55, refused 28, startable 57 (FrontAp took the builder 46). Fix:
  the row is offered every plan and a builder whose full carry holds 90 kg loads it at home (Load `front_at`) ->
  `ai_front_at_row_loads_in_base_and_is_laid` [start after 575 s]. AT fields 0.025 / 0 -> 0.175 / 0.225.
- Design limit (F3 order, one front slot): the row waits while FrontAp is re-tried. Options: (A) offer the AT row
  before FrontAp once a FrontAp job there was dropped or refused; (B) `front_slots` counts FrontAp / FrontWire only.
- F1-40 (DE / US), Germany 52.5 %: mines 97.5 / 85.0 %; front share 55.1 / 37.7 %; set off 0.525 / 0.35; AT 0.175 /
  0.225; belt 2.5 / 0 %; wire seen 41.0 / 29.4 %; nests 0.925 / 0.675; works 100 / 97.5 %; `F1off40` same 40/40.

## Fix pass 2 (2026-11-16; C-2..C-9; each control seen failing, how in brackets)
- C-2: probe `own_route_len_probe`; `ai_belt_leaves_the_own_way_round` measures the own route with the belt over its
  length before: 100 % (`belt_detour_pct` 130, side 1) vs no route (control 0: None). The fortify row adds
  `belt_route_rejects` (and in its control): 0 / 0 on the open map (report).
- C-3: no sim change (in a match `truck_for` lets a log belt start while a truck loads). Row `s10c.belt`: `works` 1
  with every S10a kind off (`material_truck` needs `s10a_on`), a truck per side driven after each Fort plan, refill
  only until the AT row stands, the truck staged back to Load between runs (with no armour it follows for good).
  Seeds 1-5: belt 100 %, US log line 100 %, pieces 5.0, truck runs 30, enemy tank mined 50 % (control `belt` 0: 80 %),
  reached belt 50 %, own_blocked 0 / 0 [no Load staging: 0 runs, log line 0 %]. Left: with `works` and `wire` 0
  `truck_for` says yes but no truck run comes (no default hits it).
- C-4: `belt_log_rest` (from the first segment with no own log piece within 2 m of its middle); `belt_wanted`,
  `point_site(Belt)`, `want_fort`. `ai_finishes_a_partial_log_belt` [old `belt_next` check: 1 piece].
- C-5 NOT fixed (stopped as told): `FrontLane` saves only the FrontAp and FrontWire lines (foot lane); the AT site is
  the R2 vehicle-path search, unsaved, and it moves too. A fixed frame needs e.g. `FrontLane::at` (SAVE 27).
- C-6: the floor counts builder squads at >= 60 % (and pending buys); order: urgent breach (was after the floor),
  urgent AT (S4 rule, an AT card ready), the floor. `ai_builder_floor_counts_fit_squads` [any living counts: no buy],
  `ai_builder_floor_waits_for_the_urgent_at_buy` [no AT check: engineers first]. No cap (a key is a setup change):
  `builder_buys_per_match` 1.925 / 2.275; floor buys cost about 4.8 / 9.5 % of card manpower (builder card
  `mp_share_pct` 5.2 / 11.0 % x floor buys / card buys).
- C-7, C-8: `assign_free`: leader within `sweep_reach_m` (300) of the site, in no running escort, no half-track job
  carries or waits for it, no ride lock (hunts, AT follows: `taken`). `front_block` reads the point's squads plus every
  `assign_free` one. `ai_mine_assign_keeps_its_reach_and_off_escorts` [pick without it: job; old `front_block`:
  no_builder 0]; `ai_mine_assign_sends_a_builder_to_the_front` takes reach 1000 (site 350 m off). Effect: top, open (2).
- C-9: the determinism test stages a builder and a FrontAp job on a saved lane at 12:30, saves 20 s later (Watch),
  loads at 1/2/3/8 threads: 60 s of equal hashes; control: the job's `start` edited -> another hash.
