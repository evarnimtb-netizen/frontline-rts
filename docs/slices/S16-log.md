# S16 log (second map and roads; at most 150 lines)
Spec: `docs/slices/S16-second-map.md`. Branch `claude/project-thread-1d8xx3`, base `<BASE>` = `a27ce63` (main: B3) plus the plan.
Commit only your files, never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage; at most 30 lines)
**State: S16b Build B done** (owner rule 2026-11-25: no balance runs; the plan's 480, mirror 160s and ablations are dropped,
every number stays in the TOML). Next: Review of A + B (gate: path code), Fix, `tools/verify.sh`, Docs. Presenter P is separate.
**Versions unchanged:** SAVE 33, PROTOCOL 17 (Build B adds no saved or sent field; the woods fix changes play only on a road node
in woods with road factor 100, which Kessel Ridge never has).
**Kessel Ridge is neutral:** 40 set after Build B **same 40/40** against both `a27ce63` and Build A (`target/s16b/kr`).
**Read before Review:** F3 below (path search cost on a road map is about 4.5x Kessel Ridge's, the plan 7 guard of +25 % is far
off) and F1 (no fix: the search does follow the roads; the low in-match road share comes from where the AI sends wheels).

## Build A (S16a, plan 10 step 1)
Commits: `312f7e6` layout, `a5c2165` rules, `c822a73` tools, `d69feb2` CLI, `ff31950` S6 load bound, then docs.
- **Data** (plan 2-3): `[[roads]]` (`RoadDef { points, width_m }`, serde default) in `MapFileDef`; `RoadSetup { points, half_width }` in
  `MapSetup`; `FLAG_ROAD = 2` set at generation after woods (exact integer point-to-segment test over the segment's box). `[vehicles.road]`
  (`RoadTuning`: enabled 1, wheeled 175, halftrack 150, tracked 140, reverse 100, path_cost 1, heuristic 100), all in `skirmish.toml`;
  a rules def without the table ignores roads. `[mobility] road_speed_pct` (100..=200, refused on a towed gun). Speeds checked against the
  defs in the TOML comment (Opel Blitz 29 -> 50 km/h, Sd.Kfz. 251 27 -> 41, Pz IV 22 -> 30, all at or under historical road speeds).
- **R1**: `road_factors` in `drive`: on a road node forward cap = road pct x the other caps (slope, driver, engine, towing, gearbox,
  trench crossing as before), reverse = `reverse_pct`; woods skipped on a road node. Off a road both factors are 100 and the arithmetic
  is the old one. Follower `top` (`road_top`: last-waypoint braking, escort, yield cap) and `yield_cap`'s stop distance road-scaled.
  A towed gun gets 100 (pushed: no gain; hitched: it moves with its tower, whose own factor and `towed_speed_pct` apply).
- **R3**: `VehiclePathCost::road_pct: Option<RoadPathCost { pct, heuristic_pct }>`, built by `VehiclePathCost::road_for(map, road,
  mobility)` in its two builders (`ai/guns.rs` `gun_route_on`, `game.rs` vehicle path queue): `None` unless the map has roads and
  `enabled` and `path_cost` are 1, so road-free searches are bit-identical. Road node: `pct * 100 / road pct`, no woods factor. Weight
  `max(100, 130 * heuristic_pct / road pct)`. Smoothing: `all_road` like `all_trench`; a shortcut over an all-road stretch must keep every
  2 m sample on a road node (`segment_on_road`).
- **R4**: infantry untouched. **R5**: `MapFileDef::validate` (2..=64 points, inside the map, no zero-length segment, width 4..=12, at most
  64 roads) and the setup check (half width 2..=6 m); `MapFileDef::mirror_warnings` (symmetry `point`: points, bases, woods and road
  points need a mirror within 2 m) printed by `frontline-cli validate` as `warning:` lines (never a failure; JSON `warnings`).
- **Symmetry blend** (`terrain.rs`): each cell and its mirror take the mean of both (`div_euclid(2)` on raw), exact and continuous.
- **CLI**: `skirmish --map ID` (default `ww2:kessel_ridge`, same hashes with and without it); reports and `--json` name the map
  (`map`); `targets` runs take `map`, `faction0`, `faction1`. Per-search expansions: `path::observe` sink set on each pool thread by
  `World::observe_path_searches` (observer only; AI searches included); JSON `s16_derived.path_search {searches, mean, p99, max}`
  (p99 to a 64-node bucket), text line `S16 path searches`. Kessel 40 set: 96,983 searches, mean 206, p99 1087, max 8000 (the cap).
- **Tools**: `batch.sh sets OUTDIR [--map ID]`, `measure.sh OUTDIR TARGETS [--map ID]` (+ per-run `map`/`faction0`/`faction1`),
  `pool.py` pools per map, `neutral.py` (`map` header note, different maps fail, `s16_derived` skip), `test_neutral.py` 3 new cases.
- **Tests** (plan 9 "S16a", without Linden Brook): `crates/sim/tests/roads.rs` (12), path unit tests (2), `sdk/crates/content/tests/
  s16_roads.rs` (5), CLI `map_flag_defaults_to_kessel_ridge_with_the_same_hashes`, save fuzz with a road-map save (`ROADS`).

### Results
- Tests: sim 983 passed, 0 failed, 14 ignored (release); content, CLI (incl. skirmish.rs 17), net all pass (`ipv6_loopback_handshake`
  excluded as usual). Clippy `-D warnings`, all targets: sim, content, cli, net clean.
- Neutral: Kessel Ridge 40 set, `a27ce63` vs Build A: **same 40/40** (`target/s16a/base`, `target/s16a/new`).
- Save fuzz: 12,254 mutations, 0 panicked, 0 bad re-saves (after `ff31950`; the first run found one panic, see F2).
- Kessel path searches (40 set): 96,983, mean 206, p99 1087, max 8000.

### Deviations from the plan, with reasons
- `VehiclePathCost.road_pct` is `Option<RoadPathCost>` (pct and heuristic_pct) rather than a bare percent: `None` is the "map has no
  roads or the switch is off" case, which keeps road-free searches bit-identical without a sentinel value.
- `[vehicles.road]` absent means roads off (`Default`: enabled 0, factors 100), not the plan's defaults in Rust: every number stays in
  the TOML (owner rule); ww2 sets all seven keys.
- `MAX_ROADS` 64 and 64 points per road (not in the plan): bounds terrain generation from an untrusted setup.
- The "base to the farthest point on Linden Brook under 8000 expansions" test needs Linden Brook: deferred to S16b. Build A tests the
  same bound on a road map built in the test (base corner to corner, each drive type).
- Plan 9 asks for a straight-road speed test; Build A also tests half-track and tracked factors and the per-vehicle override.

### Findings
- **F1 (for the owner / S16b):** with the plan's heuristic (weight at least 100 while a road node costs 57 % for wheels) the search is
  a weighted A* with a 1.75x bound on road maps. On open flat ground a road that bends away from the straight line is often not taken:
  e.g. a road dipping 40 m off a 400 m straight was left for the straight line, which costs about 1.7x the road by time. Roads through woods or round
  slopes are followed. `heuristic_pct` cannot fix it (the max(100, ...) floor). Options if S16b's `road_km_pct` misses: lower the floor
  for road maps (more expansions, the plan's 16,000-node concern) or a road-aware heuristic. Not changed here (plan rule).
- **F2 (fixed, `ff31950`):** the save fuzz found an S6 account `stock_in_g` near `u64::MAX` that loaded and then overflowed an add
  (`structures.rs`). Pre-existing; the layout change moved which mutations are picked. S6 tallies over 2^62 are now refused at load.

## Build B (S16b, plan 10 step 4)
Commits: `a86202a` checker fixes, `cd53db7` map and tests, `74cbcc9` CLI road observer, `1283ea8` test and fmt, then docs.
- **Map** `mods/ww2/defs/maps/linden_brook.toml` exactly as plan 5 (points, bases, woods, roads, bands; seed 1716, octaves as
  Kessel Ridge, `symmetry = "point"`); no terrain feature moved (no fairness run). `frontline-cli validate mods/ww2`: ok, no
  mirror warning. Heights: bases 18.3 m, B/D 26.8, A/E 16.1, C 14.2 (12.6 m under B/D); ridge crests about 30 m.
- **Tests** `crates/sim/tests/linden_brook.rs`: R5 (no mirror warning; every band, woods radius and C mirrored; a point moved 3 m
  warns twice); heights and flags exactly point-symmetric; every self-propelled vehicle of both factions (all three drive types)
  reaches the farthest point and the far base from each base under 8000 expansions (deferred from Build A); a truck from base 0
  to C, D, E drives at least 85 % of its metres on road nodes (planned path to C at least 90 %), under 30 % with roads off.
  CLI: Linden Brook equal at 1 and 3 threads; the road observer counts km (Kessel Ridge: 0 on road).
- **Observer** (plan 8 `road_km_pct`, CLI only): km driven (|speed| per tick, towed guns out) and km with the hull centre on a
  road node, per drive type; text `S16 vehicle km on road`, JSON `s16_derived.road_km_pct`. `neutral.py` skips a new key inside
  an `s16_derived` both sets have (`test_neutral.py` case).
- **Checker after A** (orchestrator relay): (1) R1 skipped woods only when the road factor was not 100, so a pushed gun (factor
  100) or `road_speed_pct` 100 on a road in woods was still slowed while R3 skips woods on every road node: the skip now reads
  "on a road node" (`vehicle.rs` `on_road`); tests: pushed gun in woods on a road 30-40 cm/s (17 before), truck with factor 100.
  (2) `reverse_pct` test (100 shipped, 150 gives 150 % of `reverse_cm_s`). (4) `observe::record` misses no early return:
  `search_with` has none; a start == goal search is counted with 0 expansions (unit test), no code change.

### Results (breakage checks only; nothing below is a target)
- Tests (release): sim 990 passed, 0 failed, 14 ignored; content + CLI 179 passed, 0 failed, 11 ignored; clippy `-D warnings` all targets on sim, content, cli clean; fmt clean.
- Linden Brook 40 set (seeds 1-20, both bases, 45 min; `target/s16b/lb`, 1 thread, and `target/s16b/lb4`, 4 threads):
  exit 0, no panic; all 40 per-match records (hashes included) equal at 1 and 4 threads; 40/40 won, no time limit, no draw.
  Printout only: length median 19.2 min (11.7-25.9), Germany 31/40 (77.5 %), base 0 17/40.
- Kessel Ridge 40 set: same 40/40 (above).
- Path searches, 40 set: Linden Brook 74,520, mean 918, p99 5119, max 8000; Kessel Ridge 96,983, mean 206, p99 1087, max 8000.
- Road km, Linden Brook 40 set: wheeled 21.9 % of 116.9 km, half-track 28.0 % of 135.3 km, tracked 45.1 % of 27.3 km
  (Kessel Ridge: 0 % of 206.9 / 223.0 / 61.9 km). Per def, 8 matches: Opel Blitz 22 %, CCKW 23 %, Sd.Kfz. 234 17 %, M8 34 %,
  Sd.Kfz. 11 tow 24 %, M3A1 31 %.

### F1 outcome (checked; no fix)
- Paths: truck searches base to each point follow the road 75-100 % (A from base 0 75 %, the rest 100 %); driven 94-96 % on road
  to C, D, E, 71 % to A. Over a 64 m grid of 196 goals from base 0, the shipped weight (floor 100) gives 61 % on road and a
  wheels' time cost of 498; an exact (admissible) search 66 % and 488 (2 % better) at 4.3x the expansions (mean 8499, max
  30,090). Lowering the floor is not a fix: with floor 74 or 57, 88 of 196 goals hit the 8000 cap (4 at floor 100).
- So F1 costs about 5 points of road share and 2 % of trip time on Linden Brook; the in-match 22 % comes from destinations off
  the roads (truck `follow_m` 150 behind the armour, gun sites off the corridor by design, recon `view_back_m`, overwatch
  rings), not from the search. A smaller search fix would not move it; no key added (owner rule: no tuning). Plan 9's
  "wheeled at least 50 %" is therefore a miss by AI destinations, for the owner (TODO S16 note).

### Findings
- **F3 (for the owner / Review): search cost on a road map.** Every vehicle search on Linden Brook runs at weight 100 (plain
  A*: 130 x 100 / road pct is under 100 for all drive types) instead of 130: mean expansions 918 vs 206, p99 5119 vs 1087 (plan 7
  allows +25 %). 4 of 196 off-road goals from base 0 (behind the east ridge) hit the 8000 cap and get partial paths. Tick p99 in
  the parallel 40 set: peak 3.08 ms vs Kessel's 3.90 (not a quiet-machine figure). Levers (not applied): `heuristic_pct` above
  100 (175 gives every drive type weight 130 or more again: the 1.3 bound off road, F1 worse), or the plan 11.1 lever.
- **F4: Germany 31/40 on Linden Brook** (Kessel Ridge 23/40 in the same 40 set). A printout of 40 matches, not a measure; the
  480 and mirror runs that would tell are dropped by the owner rule.

### AI metre audit (plan 6; distances on Linden Brook (LB) against Kessel Ridge (KR); nothing retuned)
Distances: base to B (home point) 223 m (KR 288), to C 488 (588), to E 461, A 712, D 756 (KR A 617, E 739, D 888); base to base
977 (1176). Points: B-C 268 (KR 300), C-A and C-E 348 (347), B-E 359, B-A and D-E 507, B-D 525, A-E 696. C lies 12.6 m below B/D.
- **Fits as on KR:** `[ai.infantry]`, `[ai.assault]` (support 150-250, bounds, hunts: `at_hunt_m` 400 from B reaches C and E),
  `[ai.fort]` (wire 35/60, nest, breach: point-local), `[ai.medic]` 60, `[wounds]`, `[call_in]`, `[supply]` reaches, `[ai.recon]`
  `view_back_m` 200 (from B toward C: on the west ridge), `tank_standoff_m` 160 (puts tanks on the ridge slope over C: high ground),
  mortar ranges (GrW 34 60-600, M2 90-370; points at least 268 m apart, no minimum-range conflict).
- **`[ai.supply] hq_behind_m` 120:** behind C toward base 0 is (404, 460), on the west ridge's forward slope in view of the east
  ridge (on KR behind the crest); behind B it is 103 m from the base. The reverse-slope rays decide; report.
- **`[ai.arty] post_m` 300:** for C the post lands at (242, 382) on the west crest by B, a good view; for B (223 m from base) it is
  behind the base, as on KR (288). Report.
- **`[ai.guns]` sites 150-250 m and `site_enemy_point_m` 250:** with C enemy-held, the C-facing half of the ring round B or D is
  excluded (B-C 268 m against KR 300): fewer sites. `corridor_m` 500 now reaches back into the enemy base on the road to C
  (491 m); `corridor_clear_m` 150 keeps sites 150 m off the roads, which are the corridors. `engage_still_m` 400 < B-D 525: guns do
  not duel ridge to ridge. Report.
- **`[ai.obstacles]`:** belts go on the enemy's predicted (now road) route, but R-main (base, B, C, D, base) and R-brook (A, C, E)
  are each both sides' route, so `own_route_m` 10 can refuse belts exactly on the road segments between points. Watch
  `mines_on_road_pct` when a measure is allowed. `mine_assign_reach_m` 450, `sweep_reach_m` 300: fit (base to C 488).
- **`[ai.armour] push_without_supply_m` 300, `[ai.supply] tank_crate_far_m` 300:** base covers B only (KR the same); C needs an HQ.
- **`[ai.transport] min_ride_m` 200:** base to B 223 m, just a ride (KR 288). `pick_m` 400 fits.
- **`[ai.truck] follow_m` 150, `flee_m` 120:** the off-road destination behind F1's 22 %. Report.

## Presenter P (roads and Linden Brook in the presenter)
- **Bridge** (`match_node.rs`): `map_info` gains `roads`, an array of `{points: PackedVector3Array (ground height), width}`
  (full width, metres). `minimap_texture` paints road pixels (distance to a segment within the half width, at least 0.7 px;
  `MINIMAP_ROAD_COLOR`; no contour line on a road). Unit test for the segment distance.
- **Scenery**: `godot/match/road_style.gd` (`RoadStyle`) holds every tunable (`COLOR`, `WIDTH_SCALE`, `LIFT_M`, `STEP_M`,
  `MAP_COLOR`) and `RoadStyle.build()`, one flat strip mesh that follows the ground at 4 m steps; `WorldFx` (real match) and
  `MockWorld` (mock) both call it.
- **Menus and sessions**: `Session.MAPS` has `linden_brook` (name, blurb); `mock_net.gd` `MAPS` the same. The skirmish menu
  looped `MAPS` but ignored the choice: the picker now sets `_map`, the preview image and the stats and blurb lines follow it.
  New `--map ID` flag in `session.gd`. Defaults stay Kessel Ridge.
- **Mock**: `MockMap` holds the current map (`use(id)`; `POINTS`, `BASES`, `WOODS`, `ROADS`, `OPENING`, `OWNED` are static
  vars; Kessel data is the old consts), `mock_map_linden.gd` is the Linden Brook stand-in (point-symmetric analytic height,
  points, bases, woods and roads copied from the map file). `MockMatchSource.start` calls `MockMap.use`; the name, the opening
  squads (player 1 mirrors player 0) and the owned points follow the map. The hard-coded Kessel name is gone.
- **Tests**: `cargo test -p godot-bridge` all pass; clippy `-D warnings --all-targets` clean; fmt clean.
  Godot 4.7.2 headless (downloaded to `target/godot/dl/`): new `godot/tools/test_mock_maps.gd` (in `tools/check.sh`) runs the
  mock match on both maps: 19/19 (name, ids, points owned, road mesh and road pixels only on Linden Brook, ticks advance);
  `check_project` OK; `test_net_ui` 143/143; `test_match_view` 439/440 (the known 16-vehicle perf check, 0.4 ms headless debug).
- Real bridge (release build, headless, `--map linden_brook`): `map_info` gives 4 roads of width 6 and WorldFx builds the strip.
- **Not done**: the mock ground does not rebuild if `MockMap.use` switches after `MockWorld` is ready (menus switch only
  before a match); no real-bridge windowed look at the roads (headless only).
