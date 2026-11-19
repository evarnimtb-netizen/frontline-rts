# S10c-ai log (defences at the front; at most 150 lines)
Spec: common `docs/slices/S10c-ai.md`, stages `S10c-ai-build-A.md`, `-B.md`, `-C.md`; targets `targets/S10c-ai.toml`.
Branch `claude/project-thread-e7nsak` from main `54009f6`. Another agent may work in this checkout: commit only your
files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (final state 2026-11-16; rewritten, not appended; at most 30 lines)
**S10c-ai is done on branch `claude/project-thread-e7nsak` (code `c1302b3`, fix pass 3), not merged. Waiting: the owner's
answers S10c-1..6 (`docs/TODO.md`), the verify steps below, then the merge decision.**
- Stages done: spec; Build A (SAVE 26 layout, switches, counters, observers); Build B (F1-F10 front rules); Build C
  (chunk 1: `jobs_max` 2, C2 `mine_assign`, C8 slots; chunk 2: C3 builder floor, C4 belts, C5 belt route); fix passes 1-3.
- Checks so far: `tools/check.sh quick 54009f6` all ok, 881 tests passed; c_off 40 set same 40/40 as
  `target/s10c-baseA40`; SAVE_VERSION 26, PROTOCOL 14.
- Orchestrator measure at `cc4af7c` (480 matches: seeds 1-80, 101-180, 201-280; `target/s10c-verify/measure.txt`):
  Germany pooled **40.00 %** (sets 40.62 / 37.50 / 41.88), no winner 0. Per set, Germany / USA: front share of mines
  46.6 / 45.9, 49.7 / 42.3, 52.7 / 42.3 %; mines set off 0.73 / 0.42, 0.99 / 0.29, 0.74 / 0.44 a match (main 0);
  own wire seen by the enemy AI 55.5 / 40.1, 51.0 / 27.8, 43.0 / 36.2 %; belts 10.0 / 1.9, 8.75 / 1.25, 8.1 / 2.5 %;
  front builders lost 0.55 / 0.84, 0.61 / 0.76, 0.69 / 0.66 a match; breaches 0 everywhere; own-side mistakes 3 / 2,
  0 / 1, 0 / 0.
- Targets (skirmish sets only): 270 pass, 71 miss, 41 hard, 4 unexposed, 4 control-same, 161 skipped. Armour, fortify
  and timing rows are in `tools/verify.sh`, not run yet.
- New hard misses (not on the S10b-ai hard list, `S10b-ai-log.md` line 21; 28 rows): T8a (HE share 19.8, bound 20),
  T30b-d, T40b, T40d, T54b, T54d, T54f, T72b, T88a, T88c (bought and armour rows, in `measure.txt`), T97a-d (builder share of infantry plans 15.9-17.3 %, cap 12), T111a-d (sweep when wanted
  26-31 %, bound 50), T112b, T112d (obstacles 11.9-14.4 %, bound 30), T115a, T115b, T115d (own-side mistakes, bound 0),
  T121d (mines set off 0.294, bound 0.3), T122b, T122d (own wire seen 40.1 / 27.8 %, bound 50).
- Reports over their bound (not hard): T96 breach when enemy wire 0, T123 breaches 0, T125 front builders lost (both
  sides, bound 0.5), T126 demolitions wanted 0, T127 AT exposure.
- Waits for the owner: S10c-1 (front builders lost over 0.5 on both sides; (B) 80 m not run); S10c-2 (run on (B),
  floor 2 default: on the 40 set floor 0 gave mines in 70 / 50 % of matches and missed the home works floor, floor 2
  gave 95 / 82.5 %); S10c-3..6 as run on (A) in `docs/TODO.md`; own_blocked 3 / 2 on seeds 1-80 needs R6b (B1-2, not
  built, needs its own key); breaches stay 0 until S10c-6 (B) or covering fire (S11).
- Open, not run: the full release suite (`check.sh tests`), `tools/verify.sh` (tick times, fortify rows, armour) and
  GitHub CI on the branch. Front AT row waits behind AP and wire (`at_wait`; options A/B in fix pass 1 below).

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

## Build C chunk 1 (2026-11-16, shortened)
- Orchestrator: (1) F1 `front_home` 0: the home point is a front point only while the enemy owns over half the points.
  (2) C2's pick reads every free squad, as `bunker_assign` (hunts and ambushes first); no_builder 23209 / 25080
  (`mine_assign_off`) -> 16181 / 20804. C1 `jobs_max` 2; C8 `front_slots` gate in `point_starts`, `fort_holds`,
  `front_block`; C3 "a habit want exists" = fort habits on, build gate open, an own point; card: cheapest roster card
  with `place_mines` + `build_structure`, no `breach`, `ai_weight` > 0.
- Tests [control seen failing]: `ai_two_s10b_jobs_run_at_once` [jobs_max 1], `ai_front_jobs_leave_a_slot_for_home_
  works` [front_slots 2], `ai_mine_assign_sends_a_builder_to_the_front` [0], `ai_buys_a_builder_under_the_floor` [0],
  `ai_builder_floor_never_overbuys` [3 buys], `own_blocked_pair_is_counted_once` [2], `ai_bunker_yields_a_slot_with_
  two_slots` [1 job], F1 home case [`front_home` 1].
- B1-2 cause (C6b): seed 15 normal, DE base 0: an Opel Blitz shunting against an own log barrier 2-3 m from the base
  centre; R6 checks base -> point -> `route_out` only. Proposed R6b (own key, off in `c_off`): not built.
- Deviations: `mine_assign` 1 in chunk 1; lone-point tests take `front_home` 1; determinism save at the first live
  front job (window 15 min); `yields_run` pins `jobs_max` 1; no key for the C6 recount.
- 40 sets (DE / US): default `C1a40` Germany 40.0 %: front share 52.7 / 40.6 %, set off 0.475 / 0.25, lost 0.60 /
  0.55, nests 0.70 / 0.525, works 87.5 / 77.5 %; `builders2` `C1b40` 42.5 %: front share 56.2 / 45.5 %, set off
  0.675 / 0.375, lost 0.65 / 0.80, nests 0.90 / 0.80, works 100 / 95.0 %; `mine_assign_off` `C1c40` 45.0 %: front
  share 31.9 / 13.0 %, nests 0.775 / 0.55.

## Build C chunk 2 (2026-11-16, shortened)
- `a641988` `builder_cards_min` 2 (orchestrator; `builders0.toml`); `0130677` belt code and tests. Belt = front job
  (orchestrator item 2); R6b not built (item 3). Tests [control]: `ai_builds_a_belt_before_its_at_field` [`belt` 0];
  `ai_one_belt_per_lane` [no `!front_live`: 2 jobs]; `ai_belt_leaves_the_own_way_round` [`belt_detour_pct` 0];
  `ai_belt_deterministic_threads_and_save_load` [job `start` edited]; liveness row Belt [`refused_s` 0]; fuzz stage
  "ai_belt_watch". Deviations: tests feed the builders and take `bunker` 0; the way-round control asserts "not out";
  belt determinism test staged; liveness row `fields_per_point` 0; `front_carry_ok` one step for a point blueprint;
  the fortify row stages the lane done, `front_home` 1, fed 160 kg a second.
- 40 sets (DE / US): C40 = C1b40 (Germany 42.5 %) with AT fields 0.025 / 0, belt 0 / 0, demolitions 0, breaches 0,
  own_blocked 0; `belt_off` same 40/40 as C40; `builders0` = C1a40 (40.0 %) with the same zeros.

## Fix pass 1 (2026-11-16, shortened; findings in `/tmp/claude-0/s10c-open-findings.md`)
- A-1 `mine_block` order (laid field before the trench/threat check) [old order: 15]; A-2 doc; A-3 `ablate.sh` counts a
  key echoed or at the mod default; A-4 `load_rejects_lanes_over_the_cap_alone`; A-5 CLI observer counts enemy soldiers
  and vehicles only (tank 20 m: 2; 60 m: 0).
- B-1 FrontWire keeps L1, L3, L4 (`front_wire_job_ends_and_is_not_reordered` [`refused_s` 0: alive to the end]); B-2
  front lines are not the point's R1 field or 60 m wire; B-3 `fort_holds` checks R8 room, cap, L4; B-4 "at the site"
  from the line's nearest point; B-5 load refuses lanes with `front` 0.
- C-1 (C40 code, temporary prints): the front AT row was wanted on 1169 recheck plans, started 9 times; AP or wire not
  done on 1030, slot held on 481 (pass 3 counts this as `at_wait`). Fix: the row is offered every plan and a builder
  whose full carry holds 90 kg loads it at home (`ai_front_at_row_loads_in_base_and_is_laid` [start after 575 s]).
  Design limit: the row waits while FrontAp is re-tried; options (A) AT row before FrontAp once FrontAp was dropped or
  refused, (B) `front_slots` counts FrontAp / FrontWire only.
- F1-40 (DE / US), Germany 52.5 %: front share 55.1 / 37.7 %; set off 0.525 / 0.35; AT 0.175 / 0.225; `F1off40` same.

## Fix pass 2 (2026-11-16, shortened; C-2..C-9)
- C-2 probe `own_route_len_probe`: belt detour measured over the belt's length [`belt_detour_pct` 0: None]; fortify row
  `belt_route_rejects` 0 / 0. C-3: no sim change; row `s10c.belt` drives a truck per side (seeds 1-5: belt 100 %, US
  log line 100 %, enemy tank mined 50 %, control `belt` 0: 80 %) [no Load staging: 0 runs]. C-4 `belt_log_rest`
  (`ai_finishes_a_partial_log_belt` [1 piece]). C-5 not fixed then (pass 3). C-6 builder floor counts squads >= 60 %
  after the urgent breach and AT buys (`ai_builder_floor_counts_fit_squads`, `..._waits_for_the_urgent_at_buy`); floor
  buys cost about 4.8 / 9.5 % of card manpower. C-7, C-8 `assign_free` (reach, no escort, no half-track job, no ride
  lock), `front_block` reads the same squads. C-9 determinism test staged FrontAp job, 1/2/3/8 threads [`start` edited].
- F2-40 (DE / US), Germany 47.5 %: front share 35.5 / 16.2 %; set off 0.475 / 0.075; lost 0.45 / 0.625; `F2off40` same
  40/40; `F2reach40` (`sweep_reach_m` 1000, sweeps too): 52.8 / 40.4 %, set off 1.0 / 0.2, lost 0.65 / 1.0.

## Fix pass 3 (2026-11-16, the last; orchestrator decisions of 2026-11-16; each control seen failing, how in brackets)
- Layout stays SAVE 26: the slice bumped 25 -> 26 and no save of version 26 exists outside this branch.
- 1a (C-5): `FrontLane::at: Option<[FxVec2; 2]>`, the AT row 1 line, saved once when its job starts on the R2 site
  (`new_fort_job`: line = `at_site`), else (row laid before the lane was found, or staged) once from the live mines
  (`at_line_from_mines`: farthest pair moved to the field's enemy edge, the old frame). `belt_frame` reads only `at`:
  centre `belt_gap_m` beyond its middle. Load: `at` on the map. Tests: `ai_belt_keeps_its_frame_when_at_mines_go` (4
  of 10 AT mines removed at one end, belt on the whole row's sites, `at` unchanged) [frame from the live mines: 3 of 5
  on the sites]; `ai_front_at_line_is_saved_when_sited` [no save at the job start: None]; roundtrip/hash edits (value,
  tag, other lane); `load_rejects_bad_front_lanes` "at site off the map" [check removed: loads]; save fuzz staged lane
  holds an `at`. Deviation: the fallback from mines is mine (the orchestrator named the sited case only); without it a
  lane re-found after its row was laid never gets a belt. The liveness row Belt (`s10b_ai_tests.rs` `belt_row`) now
  stages `at` from its laid mines (the drive hook runs no `front_lanes_update`; without it: "Fort: no refusal").
- 1b: `FRONT_WHY` 13 entries, `at_wait` counted beside the first-check reason (not instead): AT row 1 wanted (switched
  on, not laid nor being laid, a site) while the lane's FrontAp or FrontWire is not finished, a front job of the point
  runs, or the slots are full. JSON `front_why_at_wait`. Tests: `front_why_counts_the_at_row_waiting` (control in the
  test: FrontAp and FrontWire done -> 0, no_builder instead); CLI `s10c_json_names_match_the_targets_file` checks every
  `front_why_<name>` [`s10c_blocks` writing 12 names: "front_why_at_wait is not written"].
- 2: `[ai.obstacles] mine_assign_reach_m` (0..=2000) in `assign_free` (C2 pick and `front_block`); sweeps and
  demolitions keep `sweep_reach_m` 300. tuning.rs, skirmish.toml, tuning.md, fortifications.md, key count 52, BASELINE,
  snapshot, `c_off.toml` (at its default), `ablate.sh` allowed values. `ai_mine_assign_keeps_its_reach_and_off_escorts`
  now sets the new key and `sweep_reach_m` 100 [`assign_free` reading `sweep_reach_m`: "within reach" fails].
- Reach 40 sets (`reach450/600/800.toml`, `target/s10c-F3reachN`; DE / US: front share %, set off a match, approached
  %, front builders lost a match; Germany %). Pass 2 300 m: 35.5 / 16.2, 0.475 / 0.075, 50.0 / 17.1, 0.45 / 0.625;
  47.5 %. 450: 55.0 / 43.3, 0.95 / 0.45, 61.5 / 26.3, 0.45 / 1.05; 40.0 %. 600: 57.4 / 47.5, 0.925 / 0.25, 60.0 /
  33.3, 0.65 / 0.90; 40.0 %. 800: 53.0 / 40.3, 0.775 / 0.20, 57.5 / 30.6, 0.625 / 0.95; 42.5 %. No value keeps both
  factions at or under 0.65 lost, so 450 (the rule). These runs include 1a and 1b (pass 2's 300 m row did not).
