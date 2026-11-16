# S10c-ai: defences at the front (the AI builds where the enemy comes): common part

Status: spec (Plan, 2026-11-16). Branch `claude/project-thread-e7nsak` from main `54009f6` (S10b-ai + B1). Targets
`targets/S10c-ai.toml`; log `S10c-ai-log.md` (read its top block first). Each Build stage reads this file, its own
`S10c-ai-build-X.md` and the log. Pattern: `S10b-ai.md` (R1-R14, L1-L5, job table), `B1-log.md` (`--side-detail`).
Owner rules: features first, no Tune; Germany pooled 480 in 35-65 % (now 42.29 %, accepted B1-1); no armour, pen,
gun, ammo, lethality or card price change; every behaviour behind a switch; all new switches off = main, match for match.
Owner decisions this slice rests on: S10b-ai-E6 (B) a follow-up slice that raises use (this slice); E8 (A) T113 at
10 % of all matches (set in `S10c-ai.toml`; the older target files still say 20); S10b-ai-3 (B) obstacles across
enemy vehicle approaches beside AT fields; E1 (B) and E3 (A) were built in B1-0 (`bunker_yields`, `demolish_handover`).

## 0. Why (measured 2026-11-15/16; data `/mnt/project-files/ai-defences-2026-11-15/`, `b1main80.json` on main `54009f6`)
80 matches on main: mines in 30 / 18 % of matches (DE / US), obstacles 15 / 4 %, bunker 5 / 0 %, AT fields 0.08 / 0.03
a match; **mines set off 0; of 7908 deaths 0 within 40 m of enemy works, 0 within 10 m of enemy wire** (`--side-detail`
`deaths`); breaches started 0; sweeps and demolitions wanted 0. Traced 40: all 221 mines at the home points B or D;
no enemy soldier within 30 m of any; `wire_known` empty in all 39,500 breach checks. Sides end holding 1 or 4 points,
so A, C, E are owned for long stretches, yet nothing is built there. The chain that stops it (code, not tuning):
1. `point_starts` / `fort_points` need `trench_done`; the trench needs an engineer at a point with threat < 4.
2. AP waits for the wire, the wire for the nest (`point_wants`, `field_wants`): three jobs before the first mine.
3. `can_take` / `fort_continue` use `fort_threat`: ONE known enemy within `threat_radius` 80 m of builder or site
   blocks or drops the job. That is why `build_threat_max` 4 -> 8 changed no match. B1-5: a delayed fort is a lost fort.
4. Sites come from the trench frame (parallel to the trench), not from where the enemy walks or drives.
5. About 0.9 builder squads are bought per match and side (70 Baupioniere, 61 US engineers in 80 matches); the
   bunker's first blocker is `no_builder`. One S10b slot (`jobs_max` 1).
6. `s10b_refused` (66 of 87 jobs) mostly counts truck give-ups (`ai/truck.rs` `truck_give_up`, L5), not defence jobs:
   9 of 40 traced defence jobs failed.
7. A hedgehog row is 12 m in front of a nest; vehicles route round, so R14 ("no way round") never wants a demolition.

## 1. Scope
**In.** (A) Honest measures: the refusal split, exposure of mines, wire and obstacles to the enemy, works by point,
front calm time. (B) Front rules: hasty AP fields, wire and AT fields on the enemy approach lanes of front points,
without trench, nest or wire first; a narrower site threat rule. (C) Builder supply (`jobs_max` 2, `mine_assign`, a
builder-card floor as a measured ablation), belts (a blocking row on the vehicle lane before the AT field), B1-2.
**Out (later).** Covering fire for breaches (S11); terrain chokepoint analysis; laying under fire or at night; a
detour-cost demolition want (S10c-3 (B)); card weights and prices; presenter work; per-man mine paths; S10b-5..9.

## 2. Switches and keys (all added in Build A's layout commit at their OFF value; a stage turns its own on)
`[ai.obstacles]` switches (1/0): `front` (F1-F9), `mine_assign` (C2), `belt` (C4). Values: `front_points` 2,
`front_out_m` 50, `front_wire_len_m` 40, `front_site_threat_m` 50, `front_fields_max` 2, `belt_pieces` 5,
`belt_gap_m` 10, `belt_detour_pct` 130, `builder_cards_min` 0 (C3: 0 = off; the ablation runs 2). `jobs_max` goes
1 -> 2 in Build C. Off values: the three switches 0, `builder_cards_min` 0, `jobs_max` 1.
Override files `targets/S10c-ai-ablations/{c_off,front_off,mine_assign_off,belt_off,jobs1,builders2,site80}.toml`;
`c_off` sets every S10c key to its off value and must equal main (`neutral.py`); `site80` sets `front_site_threat_m` 80.

## 3. Data model (every saved field in Build A's layout commit; B and C change no layout)
`AiCounters` += `fort_job_fails` (S10b fort jobs ended failed, `s10b_end`), `truck_give_ups` (L5), `route_refusals`
(= R6 rejects that also write L3; `s10b_refused` stays their sum plus sweep and demolish refusals, old rows unchanged),
`front_plans` (plans with at least one front point), `front_calm_plans` (of those: no known enemy within
`threat_radius` of the point), `front_jobs`, `front_drops` (front jobs dropped by the threat rule), `front_lost` (front jobs ended by the squad gone or under
60 %), `front_resumes`,
`front_why: [u32; 12]` (names `FRONT_WHY`, the first check that kept a wanted front work from starting, as
`BUNKER_WHY`: no_front, lane_none, site_bad, cap, refused, slot, builders_max, no_builder, no_carry, no_budget,
threat_squad, threat_site), `mine_want_why: [u32; 8]` (review finding 12: the same for the R1/R2 fields),
`belt_jobs`, `belt_route_rejects`, `builder_buys`. `WorkKind` += `FrontAp, FrontWire, Belt` (not saved; jobs stay
`AiFortJob`). No new `AiState` list: lanes are a per-plan memo on `Planner` beside `at_sites` (one search per point
per `recheck_s`); which side a belt took is read from the standing pieces (as `hedgehog_side`).
`blocked_pairs` (B1-2): a counted pair stays while its vehicle and piece live (no recount); same field, same cap.
**SAVE_VERSION 25 -> 26** once (Build A: `AiCounters`, setup keys). **PROTOCOL stays 14** (no command, kind or limit).
CLI only, read-only (no sim state): `S10cSeen` in `skirmish.rs`, observed once a second beside `S10bAiSeen`.

## 4. Rules (value; source; proving test). Integer m and s, no RNG, ascending ids
| Id | Rule | Value | Source | Proved by |
|---|---|---|---|---|
| F1 | Front points: the `front_points` own points nearest the enemy base (straight distance, ties by index). With one own point that point is the front | 2 | diagnosis 0.1 | `ai_front_points_are_nearest_the_enemy_base` |
| F2 | Foot lane of a front point: one `find_foot_path` from the nearest enemy-owned point (none: the enemy base) to the point, per point per `recheck_s`, memo per plan. Vehicle lane: the R2 path (enemy base -> point), unchanged | 60 s | as R2 | `ai_front_lane_runs_from_the_nearest_enemy_point` |
| F3 | Hasty works of a front point, wanted as soon as it is own (no trench, nest or wire): `FrontAp` (blueprint length, across the foot lane `front_out_m` + `ap_beyond_wire_m` from the point), then `FrontWire` (`front_wire_len_m`, across the lane at `front_out_m`), then the R2 AT field. Each line is perpendicular to the lane there, centred, clamped | 55 m; 40 m at 50 m | 7.6 "AP mines among and before the wire" | `ai_lays_front_field_without_a_trench` (control `front` 0: none) |
| F4 | Order: at a front point the hasty wants come before nest, wire, dugout and the R1 field; `fort_points` marks the point `wants_works` without `trench_done`; `fort_holds` and `point_starts` visit front points first. Rear points keep S10a and R1-R7 as on main | - | 0.2, 0.5 | `ai_front_wants_come_before_the_nest` |
| F5 | Threat for a front job: no start and a drop while a known enemy is within `threat_radius` of the builder, or within `front_site_threat_m` of the site (the site lies toward the enemy by design). Every other job keeps 80 / 80 | 80 / 50 m | 0.3; S10c-1 (A) | `ai_front_job_starts_with_an_enemy_60_m_from_the_site` (control `front_site_threat_m` 80: no job) |
| F6 | A front job dropped by F5 writes no refusal (L3 is for failures); the mines and wire sections laid stay; the next calm plan starts a job for the rest (lines go on from the first segment not standing, the S10a rule) | - | B1-5 "delayed = lost" | `ai_front_job_resumes_after_a_threat_drop` |
| F7 | Caps: at most `front_fields_max` `FrontAp` fields a player (blueprint cap R4 holds); `fields_per_point` counts R1 and front fields; the S10b budget R8 pays mines, the S10a budget wire | 2 | cap | `ai_front_caps_hold` |
| F8 | A lost point: its fields stay the layer's (hidden mines keep working); they count toward the caps until set off or swept | - | S10b R7 | `ai_front_fields_survive_a_lost_point` |
| F9 | Nothing new for the attacker: `wire_sight` (150 m), R9/R10 sweeps and the breach job then find the wire and mines by the existing rules | - | 0 | 40 set: `wire_seen_by_enemy_pct`, `sweep_wanted` |
| C1 | `jobs_max` 2 (inside `builders_max` 2) | 2 | E6 (B) | `ai_two_s10b_jobs_run_at_once` (control `jobs_max` 1) |
| C2 | `mine_assign`: a wanted front work with no fit squad assigned to its point takes one free builder from anywhere, nearest first (as `bunker_assign`; F5 holds) | - | E6 (B) | `ai_mine_assign_sends_a_builder_to_the_front` (control 0) |
| C3 | `builder_cards_min` N: with fewer than N living builder squads (`place_mines` and `build_structure`, no `breach`) the next buy is the faction's builder card (affordable, pop room). Default 0; measured as the `builders2` ablation only | 0 | 0.5; S10c-2 (A) | `ai_buys_a_builder_under_the_floor` (control 0) |
| C4 | Belt: after a front point's AT row 1 is laid, a row of `belt_pieces` blocking pieces `hedgehog_step_m` apart across the vehicle lane, centred on it, `belt_gap_m` beyond the AT field (enemy side), so a vehicle rounding the row crosses the 40 m field. Hedgehogs with a `build_heavy` squad, else one log barrier line | 5 x 4 m; 10 m | S10b-ai-3 (B) | `ai_builds_a_belt_before_its_at_field`, fortify row `s10c.belt` |
| C5 | Belt own-route rule (replaces R6's 10 m for belts only): the own route vehicle's path base -> point -> `route_out` with the belt's nodes blocked must exist and be at most `belt_detour_pct` of the path without; else the mirrored side once, else L3 and `belt_route_rejects` | 130 % | R6; T115 | `ai_belt_leaves_the_own_way_round` (control `belt_detour_pct` 0 = rule off: `own_blocked` > 0) |
| C6 | B1-2: `count_own_blocked` counts a (vehicle, piece) pair once while both live; the cause on seed 104 (own rows at base 0) is found first and fixed if it fits the chunk | - | TODO B1-2 | `own_blocked_pair_is_counted_once` |
| C7 | Demolition want stays R14 (no way round). Belts make it possible, not likely; reported | - | S10c-3 (A) | T126 report |
L1-L5 hold unchanged for the new kinds: they are fort jobs (`job_max_s`, stall, L3 on failure, L4 before a start).

## 5. What else can be true when a job completes or ends (each a named test; stage in brackets)
| Job | Target gone | Another squad on it | Squad dead or re-ordered | Enemy arrives | Save/load mid-job |
|---|---|---|---|---|---|
| FrontAp / FrontWire | point lost mid-job: job ends, laid part stays `ai_front_job_ends_when_the_point_is_lost` [B] | one job per site `ai_two_builders_never_share_a_front_site` [B] | job ends, no refusal, rest wanted again `ai_front_builder_dies_mid_field` [B] | F5 drop, F6 resume (above) [B] | `ai_front_deterministic_threads_and_save_load` [B] |
| Belt | AT field swept or blasted: belt stays, no new belt without row 1 [C] | one belt per lane `ai_one_belt_per_lane` [C] | as fort job [C] | F5 | same test as B, with a belt job [C] |
| Builder buy (C3) | builder dies after the buy: one more buy, never above N `ai_builder_floor_never_overbuys` [C] | - | - | - | derived from state |
Generic: `ai_jobs_end_and_never_reorder_a_refused_goal` gains rows FrontAp (lane site on a slope over 25 %) and Belt.

## 6. Save, hash, validation (Build A)
New `AiCounters` fields: any value (counters). Setup: the new keys' ranges (`front_points` 0..=8, metres 0..=500,
`belt_pieces` 0..=20, `belt_detour_pct` 0..=1000, `builder_cards_min` 0..=8, switches 0..=1). The staged save of
`save_fuzz.rs` gains a FrontAp job in Watch (B) and a Belt job (C). Canonical snapshot and setup-bytes BASELINE change
only for the new keys (as B1-4, B1-5).

## 7. Measures and row honesty (Build A; names are the targets file's)
`factions.<f>.s10c` (raw: the section 3 counters by name, `front_why_<name>`, `mine_want_why_<name>`, `works_by_point`
and `mines_by_point` arrays in map point order, `mines_home`, `mines_front`) and `s10c_derived`:
`defence_job_fail_pct` (`fort_job_fails` / `s10b_jobs`), `front_mines_share_pct` (mines laid whose nearest map point
is not the layer's home point = the point nearest its base; null with no mines), `matches_mines_approached_pct` (of
matches with mines: an enemy soldier within 30 m of an own AP mine, or an enemy vehicle within 30 m of an own AT
mine, at a sampled second), `mines_set_off_per_match`, `matches_with_wire`, `wire_seen_by_enemy_pct` (of matches
where this side's wire stood: the enemy AI's `wire_known` held one of its sections), `matches_wire_approached_pct`,
`matches_obstacle_approached_pct` (enemy vehicle within 30 m of an own blocking piece), `front_calm_pct`,
`front_jobs_per_match`, `front_drop_pct`, `front_builders_lost_per_match` (front jobs ended by the squad gone or
under 60 %), `matches_with_belt_pct`, `matches_with_front_field_pct`, `builder_buys_per_match`.
Observer counters read state, so they are not zero with the switches off; `tools/neutral.py` skips the `s10c*` keys
when the base lacks them (Build A), and later stages compare against a base replayed with Build A's binary + `c_off`.
Every new row has value, exposure and control (`skirmish40_c_off`, where the front rows must miss their bounds).

## 8. Done when (480 = seeds 1-80, 101-180, 201-280; per faction on skirmish160 and skirmish160_oos)
1. Mines in >= 45 % of matches, both sides (T109 at 45); front share of mines >= 40 % (T119).
2. An enemy came within 30 m of a mine in >= 30 % of the matches with mines (T120); mines set off >= 0.3 a match (T121).
3. Enemy wire seen in >= 50 % of the matches where it stands (T122); breaches started >= 0.05 a match (T123).
4. Defence jobs failed <= 30 % (T124); front builders lost <= 0.5 squads a match (T125).
5. Reports with exposure: demolitions wanted in >= 5 % of matches (T126), belts and AT exposure (T127), bunker 10 % (T113).
6. Own-side mistakes 0 (T115); `refused_reorders`, `jobs_overdue` 0 (T118). Germany pooled 35-65 %; no match without
   a winner; no new hard miss beyond S10b-ai's list. 7. `c_off` 40 set = main 40/40 (`neutral.py`). 8. Named tests
   pass; SAVE 26, PROTOCOL 14; save fuzz 0 panics, 0 unloadable. 9. Tick p99 <= 25 % above 0.506 ms.
**Gate after Build A (orchestrator):** read `front_calm_pct` and `front_why_*` on the 40 set. If front points are calm
under 10 % of their plans, Build B's premise fails: stop and bring S10c-1 to the owner before building B.

## 9. Build stages (stop points in each file; a chunk is about 30 self-counted turns, 55-60 real)
A layout, switches, counters, observers, targets rows (`-build-A.md`, 2 chunks); B front rules (`-B`, 2 chunks);
C builder supply, belts, B1-2 (`-C`, 2 chunks). Then orchestrator measure + ablations (`tools/ablate.sh`
`ABLATIONS=docs/slices/targets/S10c-ai-ablations`) -> Review -> Fix -> verify -> Docs. No Tune stage.

## 10. Owner questions (the slice runs on each (A); the orchestrator copies them to `docs/TODO.md`)
S10c-1 Builders near the enemy. (A) A front job runs while no known enemy is within 80 m of the builder and 50 m of
the site; losses are measured (T125). (B) Keep 80 m for the site too (safer, far fewer front fields).
S10c-2 Builder squads. The AI buys about 0.9 a match. (A) Keep the buys; the slice measures a floor of 2 as an
ablation and reports win share and use. (B) Make the floor of 2 the default now (one rifle squad fewer).
S10c-3 Demolition. (A) Vehicles keep driving round an obstacle when a way round exists; demolition stays rare and is
reported. (B) Also demolish when the way round is much longer (new path searches; a later slice).
S10c-4 USA belts. The USA has no `build_heavy` card. (A) US engineers build the belt as one log barrier line (500 kg,
a truck run). (B) Belts for Germany only.
S10c-5 T109. (A) Mines in 45 % of matches (the plan Eva approved) replaces the 50 % of S10b-ai. (B) Keep 50 %.
