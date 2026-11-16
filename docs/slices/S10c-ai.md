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
2. AP waits until the wire exists and the wire until the nest exists (`point_has`: a work or job counts, not "done";
   `point_wants`, `field_wants`): three job starts before the first mine. Digging has its own gate: `wants_dig`
   hard-codes threat < 4 (`ai.rs`), a second reason `build_threat_max` 8 changed no match.
3. `can_take` / `fort_continue` use `fort_threat`: ONE known enemy within `threat_radius` 80 m of builder or site
   blocks or drops the job. That is why `build_threat_max` 4 -> 8 changed no match. B1-5: a delayed fort is a lost fort.
4. AP, wire and hedgehog sites come from the trench frame (parallel to the trench), not from where the enemy walks.
   Only the AT field already lies on the enemy vehicle path (`at_site_search`), and it too needs the trench.
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
`front_out_m` 50, `front_wire_len_m` 20 (was 40 in the first plan; review finding 2), `front_site_threat_m` 50,
`front_fields_max` 2, `front_slots` 1 (front jobs at once; C8), `belt_pieces` 5,
`belt_gap_m` 10, `belt_detour_pct` 130, `builder_cards_min` 0 (C3: 0 = off; default 2 since Build C chunk 2, orchestrator 2026-11-16; ablation `builders0`). `jobs_max` goes
1 -> 2 in Build C. Off values: the three switches 0, `builder_cards_min` 0, `jobs_max` 1.
Override files `targets/S10c-ai-ablations/{c_off,front_off,mine_assign_off,belt_off,jobs1,builders2,builders0,site80}.toml`;
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
`belt_jobs`, `belt_route_rejects`, `builder_buys`. `WorkKind` += `FrontAp, FrontWire, Belt` (not saved; jobs stay `AiFortJob`).
**Saved lanes (review finding 4):** `AiState::front_lanes: Vec<FrontLane>`, ascending by point, at most
`MAX_FRONT_LANES` 8; `FrontLane { point: u8, since: u64, ap: [FxVec2; 2], wire: [FxVec2; 2], ap_done: bool,
wire_done: bool }` holds the two site lines of a front point, fixed when first found, so every plan (not only the
recheck plan) sees the front wants. Validated on load (point in range, unique, ascending, positions on the map).
Which side a belt took is read from the standing pieces (as `hedgehog_side`).
`blocked_pairs` (B1-2): a counted pair stays while its vehicle and piece live (no recount); same field, same cap.
**SAVE_VERSION 25 -> 26** once (Build A: `AiCounters` in chunk 1, `front_lanes` in chunk 2; nothing is released
between the chunks, so one bump). **PROTOCOL stays 14** (no command, kind or limit).
CLI only, read-only (no sim state): `S10cSeen` in `skirmish.rs`, observed once a second beside `S10bAiSeen`.

## 4. Rules (value; source; proving test). Integer m and s, no RNG, ascending ids
| Id | Rule | Value | Source | Proved by |
|---|---|---|---|---|
| F1 | Front points: the `front_points` own points nearest the enemy base (straight distance, ties by index). With one own point that point is the front. Orchestrator 2026-11-16, changed for Build C the same day (`front_home` 0, off value 0): with `front` on, the home point (the map point nearest the own base) is a front point only while the enemy owns more than half of the map's capture points (the side is pushed back); before that the front points are the non-home own points only (none while the home point is the only own one), so the opening builds the home works as on main; `front_home` 1 = plain F1 (ablation `front_home_on`) | 2; 0 | diagnosis 0.1; B1 40 set (most front fields at B / D) | `ai_front_points_are_nearest_the_enemy_base` (home case: home alone -> none, enemy 2 of 5 -> none, 3 of 5 -> home; control `front_home` 1) |
| F2 | Lane of a front point, found once: on a recheck plan with no `front_lanes` entry for the point, one `find_foot_path` (as `breach_path`, wire cost off) from the nearest enemy-owned point (none: the enemy base) to the point; the two site lines of F3 are saved in `front_lanes`. The entry is dropped when the point is no longer an own front point and holds no live front job. Vehicle lane: the R2 path, unchanged | 60 s | as R2; finding 4 | `ai_front_lane_runs_from_the_nearest_enemy_point`, `ai_front_wants_show_on_every_plan` (control: entry removed -> no want until the next recheck) |
| F3 | Hasty works, wanted as soon as the point is own (no trench, nest or wire): `FrontAp` (blueprint length 30 m, across the lane `front_out_m` + `ap_beyond_wire_m` from the point), then `FrontWire` (`front_wire_len_m`, across the lane at `front_out_m`), then the R2 AT field. Both lines perpendicular to the lane, centred on it, clamped. The wire is 10 m shorter than the field: 500 % wire cost sends attackers round a wire end (about 12 m off the lane), which is inside the mine row (15 m each side). `wire_shift` is NOT applied (it would slide the wire off the lane); the own way out stays open because a 20 m line adds at most about 12 m to a foot route, own mines are harmless (S10b R7) and vehicles crush wire | 55 m; 20 m at 50 m | 7.6; findings 2, 3 | `ai_lays_front_field_without_a_trench` (control `front` 0), `front_wire_detour_crosses_the_field` (a staged enemy squad ordered along the lane: its path passes within 15 m of the lane at the mine row and a mine goes off; control `front_wire_len_m` 40: the path passes outside the row), `ai_front_wire_leaves_the_own_way_out` (own foot path point -> `route_out` at most 15 m longer with the wire standing) |
| F4 | Order: at a front point the hasty wants come before nest, wire, dugout and the R1 field; `fort_points` marks the point `wants_works` without `trench_done`; `fort_holds` and `point_starts` visit front points first. Rear points keep S10a and R1-R7 as on main | - | 0.2, 0.5 | `ai_front_wants_come_before_the_nest` |
| F5 | Threat for a front job, the same rule in `can_take` (start) and `fort_continue` (drop): a known enemy within `front_site_threat_m` of the site; or within `threat_radius` of the builder while its leader is more than `FRONT_AT_SITE_M` 20 m from the site (walking there), within `front_site_threat_m` of the builder once it is at the site. Every other job keeps 80 / 80 | 50 m at the site, 80 m on the walk | 0.3; finding 1; S10c-1 (A) | `ai_front_job_survives_an_enemy_60_m_from_the_site` (the job starts and is still alive, builder at the site, 60 s later; control `front_site_threat_m` 80: dropped) |
| F6 | A front job dropped by F5, or ended by its builder gone or weak, writes no refusal; mines and wire sections laid stay. `FrontAp` stays wanted while the lane's `ap_done` is false (set when a FrontAp job ends laid, `s10b_laid`); the new job's line runs from the first mine site of the saved line with no own mine within 1 m to the line's end. `FrontWire` the same with `wire_done` (S10a rule: from the first segment not standing). Mines set off later are not laid again | - | B1-5; finding 6 | `ai_front_job_resumes_after_a_threat_drop`, `ai_front_field_set_off_is_not_relaid` |
| F7 | Caps: one `FrontAp` and one `FrontWire` per saved lane; at most `front_fields_max` lanes hold a field or a FrontAp job (counted from `front_lanes`, not from `fields_of`); the front field does not count in `fields_per_point` (the R1 field still may follow at a trenched point); the blueprint caps hold (60 AP mines = 4 fields: `front_why` cap); S10b budget R8 pays mines, the S10a budget wire | 2 | findings 5 | `ai_front_caps_hold` |
| F8 | A lost or no-longer-front point: its lane entry is dropped (F2), so its field leaves the `front_fields_max` count; the hidden mines keep working and count only toward the blueprint cap until set off or swept | - | S10b R7; finding 5 | `ai_front_fields_survive_a_lost_point` (a new front point still gets its field) |
| F10 | Materials: a front job is taken only by a squad that carries the whole first work (`carried_g` >= its need), or stands in the own base (it loads there), or has a truck (`truck_for`); a builder at the front with nothing carried is not picked (`front_why` no_carry) and no front job sends a squad walking home (fix pass 1, C-1: except the front AT row, offered on every plan, which a builder whose full carry holds the row's 90 kg may take and load in base). L1 for front kinds is `barrier_job_max_s` 1200 if the test shows 900 s cannot hold a 600 m walk plus 15 mines (say which in the log) | - | finding 8 | `ai_front_job_from_base_ends_inside_its_deadline` (base 600 m away: done, `jobs_overdue` 0), `ai_empty_builder_takes_no_front_job` (control carried 60 kg: job) |
| F9 | Nothing new for the attacker: `wire_sight` (150 m), R9/R10 sweeps and the breach job then find the wire and mines by the existing rules | - | 0 | 40 set: `wire_seen_by_enemy_pct`, `sweep_wanted` |
| C1 | `jobs_max` 2 (inside `builders_max` 2) | 2 | E6 (B) | `ai_two_s10b_jobs_run_at_once` (control `jobs_max` 1) |
| C8 | At most `front_slots` front jobs (FrontAp, FrontWire, AT field at a front point, Belt) at once, so one of the two builder slots stays for nest, wire, dugout, roof and the R1 works | 1 | finding 8 | `ai_front_jobs_leave_a_slot_for_home_works` (control `front_slots` 2: both slots at the front) |
| C2 | `mine_assign`: a wanted front work with no fit squad assigned to its point takes one free builder from anywhere, nearest the site first (as `bunker_assign`; F5 and F10 hold). Orchestrator 2026-11-16: the pick reads every free squad, not the point assignment, so the assign bonus at own untrenched points cannot outbid it; `front_why` no_builder then means none anywhere. Fix pass 2 (C-7, C-8): only a squad within `sweep_reach_m` of the site, in no running escort and on no half-track job; `front_why` reads the same squads | - | E6 (B) | `ai_mine_assign_sends_a_builder_to_the_front` (control 0) |
| C3 | `builder_cards_min` N: with fewer than N living builder squads (`place_mines` and `build_structure`, no `breach`) the next buy is the faction's builder card (affordable, pop room). Orchestrator 2026-11-16 (Build C chunk 2): default 2, owner question S10c-2 now proposed as (B) and waiting for the owner; `c_off` keeps 0; ablation `builders0` (0) beside `builders2`. Reason: on the chunk 1 40 sets only the floor of 2 keeps the home works floor, and it lifts mines to 95 / 82.5 % of matches. Fix pass 2 (C-6): a builder squad counts at 60 % strength or more; the floor buys after the urgent breach and urgent AT buys | 2 | 0.5; S10c-2 (B proposed) | `ai_buys_a_builder_under_the_floor` (control 0) |
| C4 | Belt: after a front point's AT row 1 is laid, a row of `belt_pieces` blocking pieces `hedgehog_step_m` apart across the vehicle lane, centred on it, `belt_gap_m` beyond the AT field (enemy side), so a vehicle rounding the row crosses the 40 m field. Hedgehogs with a `build_heavy` squad, else one log barrier line. Built (Build C chunk 2): the frame is read from the laid AT mines (farthest pair = the row's axis), the side from the standing pieces; a belt job is a front job (`front_kind`, so `front_slots` counts it; orchestrator 2026-11-16); default on since chunk 2. Fix pass 2 (C-4): a log line ended part-built is wanted again from its first missing segment; C-5 (frame read from live mines) stays open: a fixed frame needs a saved AT line | 5 x 4 m; 10 m | S10b-ai-3 (B) | `ai_builds_a_belt_before_its_at_field`, fortify row `s10c.belt` |
| C5 | Belt own-route rule (replaces R6's 10 m for belts only): the own route vehicle's path base -> point -> `route_out` with the belt's nodes blocked must exist and be at most `belt_detour_pct` of the path without; else the mirrored side once, else L3 and `belt_route_rejects`. Built with a copy of the vehicle block layer with the belt's pieces added (`BlockLayer::with_pieces_at`, `gun_route_on`; no `path.rs` change); no route vehicle or no base route: the check passes | 130 % | R6; T115 | `ai_belt_leaves_the_own_way_round` (control `belt_detour_pct` 0 = rule off: `own_blocked` > 0) |
| C6 | B1-2: `count_own_blocked` counts a (vehicle, piece) pair once while both live; the cause on seed 104 (own rows at base 0) is found first and fixed if it fits the chunk. R6b (orchestrator 2026-11-16: build only if small and behind an existing key without a layout change): not built, open for Review. No key fits: under `belt` one ablation would measure two rules, under `obstacles` `c_off` would no longer be neutral; it needs its own key | - | TODO B1-2 | `own_blocked_pair_is_counted_once` |
| C7 | Demolition want stays R14 (no way round). Belts make it possible, not likely; reported | - | S10c-3 (A) | T126 report |
L1-L5 hold unchanged for the new kinds: they are fort jobs (`job_max_s`, stall, L3 on failure, L4 before a start). Fix pass 1 (B-1): FrontWire (an S10a wire blueprint) keeps L1, L3 (a refusal on a refused order or a watched line not built) and L4 like an S10b job; liveness row FrontWire.

## 5. What else can be true when a job completes or ends (each a named test; stage in brackets)
| Job | Target gone | Another squad on it | Squad dead or re-ordered | Enemy arrives | Save/load mid-job |
|---|---|---|---|---|---|
| FrontAp / FrontWire | point lost mid-job: job ends, laid part stays `ai_front_job_ends_when_the_point_is_lost` [B] | one job per site `ai_two_builders_never_share_a_front_site` [B] | job ends, no refusal, rest wanted again `ai_front_builder_dies_mid_field` [B] | F5 drop only inside 50 m at the site, F6 resume (above) [B] | `ai_front_deterministic_threads_and_save_load` [B] |
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
under 60 %), `matches_with_belt_pct`, `matches_with_front_field_pct`, `builder_buys_per_match`, `knew_enemy_wire_pct` (matches
where THIS side's AI held an enemy section in `wire_known` at a sampled second: the same-side exposure of T123).
Observer counters read state, so they are not zero with the switches off; `tools/neutral.py` skips the `s10c*` keys
when the base lacks them (Build A), and later stages compare against a base replayed with Build A's binary + `c_off`.
Every new row has value, exposure and control (`skirmish40_c_off`, where the front rows must miss their bounds).

## 8. Done when (480 = seeds 1-80, 101-180, 201-280; per faction on skirmish160 and skirmish160_oos)
1. Mines in >= 45 % of matches, both sides (T109 at 45); front share of mines >= 40 % (T119).
2. An enemy came within 30 m of a mine in >= 30 % of the matches with mines (T120); mines set off >= 0.3 a match (T121).
3. Enemy wire seen in >= 50 % of the matches where it stands (T122). Breaches started (T123) is a REPORT: a 20 m
   front line is cheaper to round than to cut, so the attacker's answer is the sweep after a blast (R9/R10,
   `sweep_wanted_matches` reported beside it); breaches need the long home lines or S10c-6 (B).
4. Reports with an orchestrator bound (hard rows would be CONTROL-SAME, finding 7): defence jobs failed <= 30 %
   (T124), front builders lost <= 0.5 squads a match (T125); over the bound: S10c-1 goes back to the owner.
   Home works floor (hard, Build C and the 480): MG nests done per match and `matches_with_works_pct`, per
   faction, not more than 10 % (relative) under the `c_off` run of the same seeds; else stop and report.
5. Reports with exposure: demolitions wanted in >= 5 % of matches (T126), belts and AT exposure (T127), bunker 10 % (T113).
6. Own-side mistakes 0 (T115); `refused_reorders`, `jobs_overdue` 0 (T118). Germany pooled 35-65 %; no match without
   a winner; no new hard miss beyond S10b-ai's list. 7. `c_off` 40 set = main 40/40 (`neutral.py`). 8. Named tests
   pass; SAVE 26, PROTOCOL 14; save fuzz 0 panics, 0 unloadable. 9. Tick p99 <= 25 % above 0.506 ms.
**Gate after Build A (orchestrator):** read `front_calm_pct` on the 40 set. It checks 80 m round the point, while
front sites lie 50-55 m toward the enemy, so it is an UPPER bound of what front jobs will see (finding 10). If it is
under 10 %, Build B's premise fails: stop and bring S10c-1 to the owner before building B.

## 9. Build stages (stop points in each file; a chunk is about 30 self-counted turns, 55-60 real)
A layout, switches, counters, observers, targets rows (`-build-A.md`, 2 chunks); B front rules (`-B`, 2 chunks);
C builder supply, belts, B1-2 (`-C`, 2 chunks). Then orchestrator measure + ablations (`tools/ablate.sh`
`ABLATIONS=docs/slices/targets/S10c-ai-ablations`) -> Review -> Fix -> verify -> Docs. No Tune stage.

## 10. Owner questions (the slice runs on each (A); the orchestrator copies them to `docs/TODO.md`)
S10c-1 Builders near the enemy. (A) A builder at a front site keeps working until a known enemy is within 50 m of
the site or of the squad (80 m while it is still walking there); losses are measured (T125). (B) 80 m throughout
(safer; an enemy 60 m off stops the work, so far fewer front fields).
S10c-2 Builder squads. The AI buys about 0.9 a match. (A) Keep the buys; the slice measures a floor of 2 as an
ablation and reports win share and use. (B) Make the floor of 2 the default now (one rifle squad fewer).
S10c-3 Demolition. (A) Vehicles keep driving round an obstacle when a way round exists; demolition stays rare and is
reported. (B) Also demolish when the way round is much longer (new path searches; a later slice).
S10c-4 USA belts. The USA has no `build_heavy` card. (A) US engineers build the belt as one log barrier line (500 kg,
a truck run). (B) Belts for Germany only.
S10c-5 T109. (A) Mines in 45 % of matches (the plan Eva approved) replaces the 50 % of S10b-ai. (B) Keep 50 %.
S10c-6 Front wire shape. (A) A short 20 m line with a 30 m mine row behind it: attackers go round the wire into the
mines; sweeps are the answer, breaches stay rare. (B) Long 60 m lines with a field at each end, so cutting or blowing
the wire is the answer (two fields a point, more materials; a later slice).

## 11. Addendum for Build A chunk 2 (added after the plan review; chunk 1 was already running)
1. Layout, same SAVE 26: `AiState::front_lanes: Vec<FrontLane>` exactly as section 3 (empty in Build A),
   `MAX_FRONT_LANES` 8, load validation (point in range, ascending, unique, positions on the map), in the save
   roundtrip and hash tests, a `load_rejects_bad_front_lanes` case.
2. Keys: add `[ai.obstacles] front_slots` 1 (range 0..=16); change `front_wire_len_m` to 20; both in `c_off.toml`,
   `skirmish.toml` comments, `docs/modding/tuning.md`, key count, BASELINE, snapshot.
3. CLI: `s10c_derived.knew_enemy_wire_pct` (section 7), read from this player's `AiState::wire_known` each sampled
   second; T123's exposure reads it.
4. Log: record `front_calm_pct` as an upper bound (section 8 gate), and MG nests done per match and
   `matches_with_works_pct` per faction on `target/s10c-baseA40` (the home works floor of section 8).
