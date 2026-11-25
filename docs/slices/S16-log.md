# S16 log (second map and roads; at most 150 lines)
Spec: `docs/slices/S16-second-map.md`. Branch `claude/project-thread-1d8xx3`, base `<BASE>` = `a27ce63` (main: B3) plus the plan.
Commit only your files, never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage; at most 30 lines)
**State: S16a Build A done.** Next: the checker after A (plan 10.2), then Presenter P, then S16b Build B (Linden Brook).
**Layout** (one commit): SAVE 32 -> **33**, PROTOCOL 16 -> **17** (setup sent in `Ready` changed; no new command). S14 is built in
parallel and may bump too: the second to merge renumbers. Setup-bytes BASELINE and the canonical snapshot refreshed (`[vehicles.road]`);
`crates/simbench/src/report.rs` needs no refresh (no resolved value changed; its BASELINE is tick timings).
**Kessel Ridge is neutral:** 40 set (seeds 1-20, both bases, 45 min) at `a27ce63` and after Build A: **same 40/40** (`tools/neutral.py`,
note `map - -> ww2:kessel_ridge`). Per-match hashes differ, as for every setup-layout change (the setup is hashed); neutral ignores them.
**Read before S16b:** the R3 heuristic (weight never below plain A*, plan 4) bounds a path at 1.75x the cheapest on a road map. On open
flat ground a search therefore stays near the straight line: a road that bends away from it is taken only when the straight line is
dearer (woods, slope; named test `a_path_follows_a_road_when_faster_and_leaves_it_when_not` uses woods). A truck on a bent road over open
ground can leave it (see finding F1). `road_km_pct` in S16b will show how much this matters on Linden Brook.

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
