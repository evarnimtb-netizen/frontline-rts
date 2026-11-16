# S10c-ai log (defences at the front; at most 150 lines)

Spec: common `docs/slices/S10c-ai.md`, stages `S10c-ai-build-A.md`, `-B.md`, `-C.md`; targets `targets/S10c-ai.toml`.
Branch `claude/project-thread-e7nsak` from main `54009f6`. Another agent may work in this checkout: commit only your
files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)
**Build C chunk 2 done (2026-11-16). STOP POINT 2. Next: orchestrator measure + ablations, then Review.**
- Defaults now: `front` 1, `front_home` 0, `jobs_max` 2, `front_slots` 1, `mine_assign` 1, `builder_cards_min` 2
  (orchestrator, S10c-2 proposed (B), waiting for the owner), `belt` 1. `c_off` keeps every switch 0 and the floor 0.
- Neutral (C2 code, release CLI rebuilt): `python3 -I tools/neutral.py target/s10c-baseA40 target/s10c-C2off40`
  -> `skirmish40.json: same 40/40 (note: overrides 1 -> 5 items)`.
- Home works floor holds at defaults (`target/s10c-C40` vs baseA40): DE nests 0.90 (0.825), works 100 % (95.0);
  US 0.80 (0.725), 95.0 % (92.5). So `belt` 1 was turned on.
- **Belts never ran in a match:** `belt_jobs` 0 / 0 in C40; the AI laid a front AT row 1 in 1 of 80 faction-matches
  (`at_fields_per_match` 0.025 / 0). `belt_off` (`target/s10c-C2belt0`) is same 40/40 as C40, and C40 equals
  chunk 1's `builders2` set number for number. The belt's match value is unmeasured until front AT fields happen
  (Review: why front AT rows are so rare; the F-rules lay AP and wire first and `front_slots` 1 holds the slot).
- Fortify row `s10c.belt` (seeds 1-5, both sides): belt built 50 % (Germany hedgehogs; US log line not built, see
  deviations), pieces 2.5, enemy tank mined 80 %, reached belt 20 %, own_blocked 0, field laid 100 %; controls:
  `belt` 0 mined 100 % (the belt adds nothing over the field here: report row, as the stage file allows),
  `belt_detour_pct` 0 own_blocked 0 (the control does not bind in this staging; the sim test covers C5).
- Open for Review: (1) R6b not built (no fitting key; needs its own key, `c_off` 0). (2) Belts in matches: front AT
  rows rare (above). (3) Hedgehog belts need truck runs (F10: no walk home); a partial belt stays partial until
  supply comes; a parked or failed belt job blocks a retry there as R5 does. (4) A partial log line counts as "the
  belt". (5) `front_why` has no belt entry. (6) The way-round test's control does not reach `own_blocked` > 0 (the AI
  re-tasks the tank; it halts beyond `OWN_BLOCKED_M`); it asserts the tank does not get out. (7) Fortify row: US
  log line not built and the `belt_detour_pct` 0 control reads 0. (8) S10c-2 still waits for the owner.
- Ignored save fuzz (`--ignored`, 10852 mutations): 2 panics outside this slice's code, overflow on mutated
  supply accounts (`fwd_belt_rounds` at `structures.rs:1540`, `crate_bombs` at `structures.rs:1245`, value
  4294967294); not fixed here (not S10c code; the played saves changed with the new defaults). Open for Review.
- Code map (chunk 2): `fort.rs` `WorkKind::Belt`, `belt_frame`, `belt_row`, `belt_log`, `belt_side`, `belt_next`,
  `belt_wanted`, `belt_job` (-> `front_kind`), `belt_route_side` (C5), the Belt block in `point_starts`;
  `vehicle.rs` `BlockLayer::with_pieces_at`; `guns.rs` `gun_route_on`; CLI `fortify/s10c_ai.rs`.

## Plan (2026-11-16)
Cause chain read in the code (spec 0): `trench_done` gate, the nest -> wire -> AP order, `fort_threat` (one known
enemy within 80 m of builder or site), trench-frame sites, about one builder squad a match, `jobs_max` 1, 12 m rows.
Design choices: front = the two own points nearest the enemy base (no map analysis); lanes by one path search per
point per `recheck_s` (foot: from the nearest enemy-owned point; vehicle: R2 unchanged); hasty AP field then wire then
AT field with no trench needed and ahead of the nest; the site's threat radius 50 m for front jobs only, the builder's
stays 80 m; a threat drop is not a refusal and the rest of the line is wanted again; belts block the lane beyond the
AT field so the way round crosses the mines; own-route safety for belts by a detour check instead of R6's 10 m.
Not designed in (said in spec 1): laying under fire, chokepoint analysis, a detour-cost demolition want.
Honest limits: on the open map a 20 m belt never leaves "no way round", so R14 demolitions stay rare (T126 is a
report); with one builder squad per match the front work competes with the rear works (F4 puts the front first;
`builders2` measures a second squad); B1 found that fortifying helps the USA more, so the guard band is watched at
every stage end.
Layout: only `AiCounters` fields and setup keys (SAVE 26 in Build A); lanes are a per-plan memo; no new job type.

## Build A and Build B (2026-11-16, shortened)
- A2 layout `6cd959c` (`front_lanes`, `front_slots`, `front_wire_len_m` 20; SAVE 26); observers, JSON, `neutral.py`
  skip, `ablate.sh` `233f9b7`. Observer cost +0.0 to +0.4 %. Deviations: observer test places the enemy squad; a mine
  laid and gone within one sampled second is not seen; extra raw keys; approach counts soldiers on foot.
- B1 `571e841`: lanes, sites, wants, starts (F1-F4, F7, F10, `front_why`, `front_jobs`); neutral `B1off40` same
  40/40. F10: a FrontAp job from a base 590 m away took 312 s. Deviations: US engineers in tests; lane test against
  the foot path; no detour finding (at 20 m the leader crossed the row).
- B2 `71b8bdd`: F5, F6, F8, determinism, liveness row, fuzz stage, `front` 1, `front_home` (orchestrator: 0 = home
  point a front point only as the only own point). SAVE 26, PROTOCOL 14. Every test seen failing with its control.
  Deviations: threat is a player-0 truck; F5 lifts `build_threat_max` for front works; resume may finish the mine in
  hand; the liveness row blocks with an own tank; default on moved S10a/S10b scene tests to `front` 0.
- B2 40 set (DE / US), Germany 50.0 %: front share 12.7 / 18.1 %; approached 11.1 / 17.9 %; works 97.5 / 80.0 %;
  nests 0.35 / 0.225 (`c_off` 95.0 / 92.5 %, 0.825 / 0.725). `front_home_on` Germany 47.5 %.

## Build C chunk 1 (2026-11-16)
- Orchestrator additions (2026-11-16): (1) F1 with `front_home` 0: the home point is a front point only while the
  enemy owns more than half of the map's points; before that front points are the non-home own points only (spec F1
  row, `tuning.md`, `skirmish.toml`). (2) C2's pick reads every free squad (`mine`), not `assigned`, as `bunker_assign`.
  Hold bonus: it no longer decides the pick (test below: the builder's task stays at the home point and it still takes
  the front job), but it still acts around it: hunts and ambushes run before `fort_start` on assigned squads, and
  between jobs assign sends the builder back to the nearest own untrenched point (+80 at every one); no_builder fell
  only from 23209 / 25080 (`mine_assign_off`) to 16181 / 20804 (now = "no fit free builder anywhere").
- C1 `jobs_max` 2 (`skirmish.toml`, snapshot, content tests; BASELINE unchanged: it hashes the frozen S3c fixtures,
  only starting values in `tuning.rs` move it). C8 `front_slots` gate in `point_starts`, `fort_holds`, `front_block`.
  C3 floor: "a habit want exists" is read as fort habits on, build gate open and an own point (`buy` runs before
  `fort_points`; a want flag would be unsaved state). Card: cheapest roster card with `place_mines` and
  `build_structure`, no `breach`, `ai_weight` > 0 (S10a cards need `[ai.fort] cards`); no mix slot needed.
- Tests (each control seen failing by running the main assertion on the control run, or by the named code edit):
  `ai_two_s10b_jobs_run_at_once` (jobs_max 1 -> 1), `ai_front_jobs_leave_a_slot_for_home_works` (front_slots 2 -> 2
  front jobs), `ai_mine_assign_sends_a_builder_to_the_front` (0 -> no job, no_builder), `ai_buys_a_builder_under_the_
  floor` (0 -> first buy rifles), `ai_builder_floor_never_overbuys` (code edit: alive count ignored -> 3 buys),
  `own_blocked_pair_is_counted_once` (code edit: old replace-the-list -> 2), `ai_bunker_yields_a_slot_with_two_slots`
  (bunker_yields 0 -> 1 job beside the bunker; needs `builders_max` 4), F1 home case (`front_home` 1 -> [C, B]).
- B1-2 cause (C6b): seed 104 reads 0 on main now; the default B40 set had DE 31 in seed 15 normal. Replayed with
  9184e0a + a print: all 31 counts are one Opel Blitz (truck job Load / Return, goal = the own base centre) against
  two pieces of an own log barrier 2-3 m from the base centre (base 0); the pair came and went as the truck shunted
  (with the recount: 2). R6 checks only base -> point -> `route_out`; a route starting at the base centre would have
  refused a barrier 2 m from it, so `own_route` was most likely None (no vehicle path from the base centre), and R6
  lets every site through when the search fails. Proposed R6b (own key, off in `c_off`): no blocking piece within
  `own_route_m` + `wreck_pad_m` of the own base centre, and a failed route search refuses instead of passing.
- Deviations: (1) `mine_assign` default 1 in chunk 1 (stage file: chunk 2), so ablation (c) differs from (a).
  (2) Tests on a lone held point or C alone take `front_home` 1 (`ai_front_wants_come_before_the_nest`,
  `ai_front_job_from_base_ends_inside_its_deadline`, liveness row FrontAp). (3) The determinism test's second save is
  the first second with a front job live from the end of the first save's replay (seed 1 now has none from 9:52 to
  24:52; saves at 8:22 and about 8:52), window 15 min. (4) `yields_run` pins `jobs_max` 1. (5) The C6 recount has no
  key: it changes a counter only, and `c_off` reads `own_blocked` 0 (same 40/40).
- 40 sets (DE / US). Default `C1a40`, Germany 40.0 %: mines 70.0 / 50.0 %; home/front 218/243, 171/117 (front share
  52.7 / 40.6 %); approached 42.9 / 20.0 %; set off 0.475 / 0.25; wire seen 62.1 / 34.8 %; knew 20.0 / 45.0 %;
  breaches 0 / 0; front jobs 5.45 / 4.33; lost 0.60 / 0.55; why no_builder 16181 / 20804, slot 5390 / 3870, no_front
  4730 / 4995; builder buys 0 / 0; nests 0.70 / 0.525; works 87.5 / 77.5 %; wire 72.5 / 57.5 %; own_blocked 0 / 0.
  `builders2` `C1b40`, Germany 42.5 %: mines 95.0 / 82.5 %; 398/510, 342/285 (56.2 / 45.5 %); approached 63.2 / 51.5 %;
  set off 0.675 / 0.375; wire seen 45.9 / 54.1 %; knew 50.0 / 42.5 %; breaches 0 / 0; jobs 8.45 / 9.35; lost 0.65 /
  0.80; why slot 7070 / 6602, no_builder 5789 / 5704, no_front 4730 / 5001; buys 1.7 / 1.925; nests 0.90 / 0.80;
  works 100 / 95.0 %; wire 92.5 / 92.5 %; own_blocked 0 / 0.
  `mine_assign_off` `C1c40`, Germany 45.0 %: mines 45.0 / 30.0 %; 169/79, 140/21 (31.9 / 13.0 %); approached 22.2 /
  16.7 %; set off 0.075 / 0.05; wire seen 19.0 / 13.6 %; knew 7.5 / 10.0 %; breaches 0 / 0; jobs 1.95 / 1.40; lost
  0.30 / 0.225; why no_builder 23209 / 25080, no_front 4730 / 4995, lane_none 3440 / 3515; buys 0 / 0; nests 0.775 /
  0.55; works 97.5 / 77.5 %; wire 52.5 / 55.0 %; own_blocked 0 / 0.

## Build C chunk 2 (2026-11-16)
- Commits: `a641988` `builder_cards_min` 2 (orchestrator; it broke `s10c_ai_layout_roundtrips_and_hashes`: its edit
  set the new default 2 and changed nothing, so it now sets 3; seen failing, then fixed; `builders0.toml` added);
  `0130677` belt code and tests; then the defaults, CLI row and docs commit.
- C4/C5 as built: see the spec rows. Belt = front job (orchestrator item 2). R6b not built (orchestrator item 3).
- Tests and how each control was seen failing: `ai_builds_a_belt_before_its_at_field` (`belt` 0 -> "five hedgehogs"
  left 0); `ai_one_belt_per_lane` (code edit: no `!front_live` in `belt_wanted` -> 2 jobs at once); `ai_belt_leaves_
  the_own_way_round` (`belt_detour_pct` 0 -> the tank does not get out); `ai_belt_deterministic_threads_and_save_load`
  (1/2/3/8 threads, save in Watch with work under way; control: an edit of the job's `start` -> "at 1 threads"
  fails); liveness row Belt (`refused_s` 0 -> L1 "past its deadline"); fuzz stage "ai_belt_watch" (17 stages).
- Deviations: (1) tests feed the builders (staged refill: carry is clamped to about 100 kg, trucks were busy) and
  take `bunker` 0; (2) the way-round control asserts "not out", not `own_blocked` > 0; (3) the determinism test is
  staged, not a full skirmish; (4) the liveness row takes `fields_per_point` 0 (an AP job started in the same tick);
  (5) `front_carry_ok` needs one step for a point blueprint (it counted line segments: no belt job started);
  (6) the fortify row feeds 160 kg a second instead of a loaded truck, stages the lane done and takes `front_home` 1;
  the US log line (500 kg job) was not built there; its `belt_detour_pct` 0 control reads 0.
- 40 sets (`BATCH_JOBS=4`, world `85eeb1da`; DE / US): **C40** (`target/s10c-C40`), Germany 42.5 %: mines 95.0 /
  82.5 %; front share 56.2 / 45.5 %; approached 63.2 / 51.5 %; set off 0.675 / 0.375; AT fields 0.025 / 0; tanks
  stopped 0 / 0; belt 0 / 0 %; obstacle approached 0 / 0 %; demolitions wanted 0 / 0; wire seen 45.9 / 54.1 %;
  breaches 0 / 0; front builders lost 0.65 / 0.80; nests 0.90 / 0.80; works 100 / 95.0 %; wire 92.5 / 92.5 %;
  own_blocked 0 / 0; own-side mistakes 0 / 0. **belt_off** (`target/s10c-C2belt0`): same 40/40 as C40.
  **builders0** (`target/s10c-C2b0`), Germany 40.0 %: mines 70.0 / 50.0 %; front share 52.7 / 40.6 %; approached
  42.9 / 20.0 %; set off 0.475 / 0.25; AT fields 0 / 0; stopped 0 / 0; belt 0 / 0 %; obstacle approached 0 / 0 %;
  demolitions wanted 0 / 0; wire seen 62.1 / 34.8 %; breaches 0 / 0; lost 0.60 / 0.55; nests 0.70 / 0.525; works
  87.5 / 77.5 %; wire 72.5 / 57.5 %; own_blocked 0 / 0; mistakes 0 / 0 (= chunk 1's default set).
