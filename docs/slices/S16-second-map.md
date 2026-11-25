# S16: second map and roads (plan)

Status: **plan**, written 2026-11-22 from `main` at `6f7bda0` (S12 merged); revised the same day after an independent review (11
findings, all folded in; section 13). Nothing here is built. S13 (heavy wave) is being built, and
S14 (assault kit) and S15 (USSR) are being planned in parallel (`S14-*.md`, `S15-ussr.md`). `<BASE>` is the last commit of S15 when S16a
Build A starts. The roadmap (`docs/design/roster/roster-expansion.md` 4.1) ends at S15, so this file also says why S16 is this item (section 0).
Pattern: `S12-wounds-medics.md`. Owner rules as every slice since S10: features first, no Tune; each matchup's pooled share stays in the
35-65 % guard band; tick p99 must not regress; all tuning in TOML; every behaviour behind a switch or a map key; S16 on Kessel Ridge = `<BASE>`.

## 0. Why this item, and what was weighed
- **The whole game is one map.** Kessel Ridge (`mods/ww2/defs/maps/kessel_ridge.toml`, 1024 m, five points) is the only map. Every AI rule
  and every balance figure since S3 was measured on it, and its base 1 is still worth about 6 points of win share (TODO "Known issues").
  A second map tests whether the AI and the balance generalise, and it is the first thing a player notices after factions.
- **Maps are already data**, so this is a small slice. `MapFileDef` (`sdk/crates/content/src/defs.rs:1532`) holds terrain, woods, points and
  bases; the AI reads points and bases from the map (`ai/fort.rs:461` `front_points_of`, `ai/guns.rs:364` `point_to_cover`), and the only
  1024-m constants in `crates/sim/src` are in tests (`ai/guns.rs:1250`). The gaps are: the CLI hard-codes the map (`skirmish.rs:2137`),
  the measure tools have no map argument, the presenter lists one map (`godot/autoload/session.gd:59` `MAPS`), and maps have no roads. One latent engine bug is
  in the way: `symmetry = "point"` (S3c) copies each far cell from its mirror with no blending (`terrain.rs:247-259`), which leaves a
  step of several metres along the anti-diagonal. No shipped map uses it (Kessel Ridge mirrors its bands by hand, S3c-8 (A)); S16a fixes it.
- **Roads are the one new system.** S8b-8 (A) left trucks at cross-country speed because "no roads on our maps" (`usa_cckw.toml:40`).
  A road is the cheapest feature that makes a second map play differently: supply runs, tows and transport choose routes, and the defender
  gets natural places for mines and AT guns. It is one terrain flag beside `FLAG_WOODS` (`terrain.rs:48`), one speed factor beside
  `woods_speed_pct` (`vehicle.rs:1893`) and one path cost beside the woods x2 (`path.rs:328`).
- **Split per the owner rule (2026-11-09).** **S16a mechanics**: roads, `--map`, tools; Kessel Ridge has no roads, so S16a is neutral by
  construction and proven on the 40 set. **S16b map and AI**: Linden Brook, the AI audit, the measures. Presenter P between.
- **Weighed and not chosen** (owner question S16-1):
  - **2v2 team play** (SYSTEM_DESIGN 2.5 launch scope). Bigger (~1.5x S3: lobby, relay, HUD for allies, AI ally coordination, pop and
    income split) and it needs a four-base map anyway, so the map work here is its first half. Recommended as **S17**.
  - **Late-1944 US wave** (M4A3E8, M24, M5A1, M4A1 (76) W; roster 3.2 "Later"). Mostly data, little new after S13; any time.
  - **Buildings and a village** (milestone M4). The largest option (destruction, interiors, cover, nav); not before the map tools exist.
  - **Balance pass B3.** Already queued after S13 (S12-16 first); it is a pass, not a slice, and runs regardless.
- **Kept / dropped.** Kept: roads as a flag with speed and path cost, a second hand-placed point-symmetric map, map choice in CLI, tools and
  menus, a mirror-placement check. Later: bridges and water (no water system), road grading (S16-9 (B)), infantry road march, map
  rotation in lobbies, a map editor (M8).

## 1. Scope
**S16a (engine).** Build A: `[[roads]]` in map files; `FLAG_ROAD` in the flag layer; vehicle road speed by drive type; vehicle road path
cost; validation; `skirmish --map`; `batch.sh`, `measure.sh`, `pool.py`, `neutral.py` and targets files take a map; JSON reports name the map.
Neutral proof on Kessel Ridge.
**Presenter P (Sonnet, after Build A).** Roads drawn in the scenery and on the minimap (`match_node.rs:4073` `minimap_texture`; `map_info` at `:3387-3435`
gains `roads`), the map's
entry in `Session.MAPS` with blurb and preview, both menus (`main_menu.gd:196`, `multiplayer_panel.gd:358` already loop `MAPS`), the second list in `godot/net/mock_net.gd:34`, and a
Linden Brook mock beside `godot/match/mock_map.gd` (Kessel only; `mock_match_source.gd:1606` hard-codes the name). Defaults stay Kessel
Ridge (`main_menu.gd:21`, `crates/net/src/relay.rs:94`; S16-12).
**S16b (map and AI).** Build B: `mods/ww2/defs/maps/linden_brook.toml`; mirror-placement test; AI audit (section 6); `targets/S16.toml`;
measures on both maps; Review, Fix, verify, Docs.
**Not in S16:** infantry speed on roads; roads changing spotting or cover; new terrain kinds; more than two bases.

## 2. Switches and keys (Build A adds all; stage in brackets turns a switch on)
| Table | Key | Default | Meaning; source |
|---|---|---|---|
| map `[[roads]]` | `points`, `width_m` | none | polyline of at least 2 points in metres; width 6-12 m (6 m: a single-track country road; under 6 m a diagonal road leaves gaps on the 4 m grid) |
| `[vehicles.road]` | `enabled` | 1 | R1-R4 master switch; with 0 a road flag is ignored (A/B on Linden Brook) |
| `[vehicles.road]` | `wheeled_pct` | 175 | speed on road, % of the def's cross-country `speed_cm_s` (verify per type in Build A) |
| `[vehicles.road]` | `halftrack_pct` | 150 | as above |
| `[vehicles.road]` | `tracked_pct` | 140 | as above (Sherman about 40 km/h on road, about 24 km/h practical cross-country; verify) |
| `[vehicles.road]` | `reverse_pct` | 100 | reverse speed on road (today reverse shares the forward `pct`, `vehicle.rs:1922`) |
| `[vehicles.road]` | `path_cost` | 1 | R3 road nodes priced by time, not length |
| `[vehicles.road]` | `heuristic_pct` | 100 | R3 heuristic weight = max(100, 130 x `heuristic_pct` / road_pct); 100 keeps the cap safe |
| vehicle `[mobility]` | `road_speed_pct` | none | optional per-vehicle override of the drive-type value |
The three type keys and `road_speed_pct` validate as 100..=200 (save load rejects `speed > 2 x top + 1`, `vehicle.rs:628`). A hitched gun
moves at its tower's road factor times `towed_speed_pct`; a crew-pushed gun (the same speed code with no tower, `vehicle.rs:1896-1911`) gets
100 % and path road factor 100.

## 3. Data model
- `RoadDef { points: Vec<[u32; 2]>, width_m: u32 }` in `MapFileDef.roads` (`#[serde(default)]`, so Kessel Ridge loads unchanged);
  `RoadSetup { points: Vec<FxVec2>, half_width: Fx }` in `MapSetup.roads` (`setup.rs:829`).
- `FLAG_ROAD = 2` in the per-point flag layer (`terrain.rs:156`), set at generation for grid points within `half_width` of a segment,
  after woods. The flag layer is part of `Heightfield`, which is saved with the state (`terrain.rs:149-158`) and hashed (`hash.rs:316`);
  on a map without roads no bit changes, so Kessel Ridge's bytes and hashes are unchanged.
- The setup is in the save (`world.rs:476-478`) and in `setup_hash` (`crates/net/src/lib.rs:71`, sent in `Ready`,
  `protocol.rs:213-216`): **SAVE 30 -> 31 and PROTOCOL 16 -> 17** in one layout commit (or the next free numbers after S13-S15).
- `VehiclePathCost` gains `road_pct`; its two builders change (`ai/guns.rs:953` `gun_route`, `game.rs:2398`), one line each, in S16a.
- Point symmetry (S3c) becomes a blend: each cell takes the mean of its generated height and its mirror cell's, so the field is exactly
  point-symmetric and continuous. Neutral: no shipped map sets `symmetry`.
- No new entity, no new command. The S16 measures (section 8) are CLI observer counters in `skirmish.rs`, not saved sim state.
- `MobilityDef.road_speed_pct: Option<u32>`; drive type from the existing `type` (`wheeled`, `halftrack`, `tracked`; `towed` follows its tower).

## 4. Rules (integers and `Fx`)
- **R1 Speed.** On a road node the speed cap is `road_pct` of `speed_cm_s` instead of 100 %, and the woods cap (`woods_speed_pct`) does not
  apply. Slope, trench-crossing, damage and towing caps apply after it as today (`vehicle.rs:1887-1924`). Reverse uses `reverse_pct`.
  The node is the one under the hull centre, read the same way as `woods_at`. The drive controller's `top` (`throttle_for(cap, top)`,
  `vehicle.rs:2837-2852`, `2918`) is the road-scaled speed while the hull is on a road node, so braking for the last waypoint, escort pace
  and yield stop distance scale with it.
- **R2 Acceleration** unchanged; a vehicle reaches road speed only on a straight run (it already accelerates to its cap).
- **R3 Path cost.** Cost is per entered node (`vehicle_pct`, `path.rs:312`): entering a road node multiplies it by `100 / road_pct` of that
  vehicle (time, not length) and skips the woods factor (`woods_pct`, `path.rs:328`). Slope cost and the block and clearance layers apply
  as today. Heuristic: weight max(100, 130 x `heuristic_pct` / road_pct), applied **only when the map has roads and `enabled` is 1**, so
  Kessel Ridge searches are bit-identical. With the default 100 the weight stays at least plain A*, so off-road searches do not blow up
  against `path_max_expansions` 8000 and `path_budget_per_tick` 4000 (`skirmish.toml:43-44`); the stated bound is 1.75x on road maps.
  (Dividing the 1.3 by the road factor would give 74 % for wheels and about 16,000 nodes for base 0 to A.) Smoothing (`smooth`,
  `path.rs:658`, accepts a shortcut when `dear <= path_dear.max(100)`) gets one rule: a shortcut may not touch a non-road node when the grid
  stretch it replaces is all road.
- **R4 Infantry** paths and speeds ignore roads (S16-8).
- **R5 Validation** (`frontline-cli validate`): every road point inside the map, width 4-12, at least 2 points, no zero-length segment; on a
  map with `symmetry = "point"` the hand-placed points, bases, woods and roads must mirror through the centre within 2 m, else a warning
  (Kessel Ridge sets no symmetry, so R5 does not apply to it; Linden Brook's mirror is also a named test).

## 5. Linden Brook (`ww2:linden_brook`, 1024 m, S16b)
A shallow brook valley runs north to south through the centre; a low ridge stands on each side. The bases sit west and east, not in the
corners. Point C, worth 2, lies in the valley, overlooked from both ridges, the opposite of Kessel Ridge where C is on the crest.
`symmetry = "point"` (the blended form, section 3), so the terrain is exactly mirrored; the bands are also listed with their mirrors so
the map reads the same with symmetry off. Every hand-placed item below is listed with its mirror.
| Item | Base 0 side | Mirror (base 1 side) |
|---|---|---|
| Bases (r 40) | (72, 300) | (952, 724) |
| Points | B (260, 420) on the west ridge; E (512, 164) south crossing | D (764, 604); A (512, 860) |
| Centre | C (512, 512), `value = 2`, in the valley | itself |
| Valley | brook (440, 0) to (584, 1024), depth 6, width 140 | itself |
| Ridges | west (240, 80) to (320, 960), height 10, width 260 | east (784, 944) to (704, 64) |
| Woods (pairs) | (470, 300) r 50; (180, 600) r 45; (644, 144) r 40; (540, 420) r 30 | (554, 724); (844, 424); (380, 880); (484, 604) |
| Roads (6 m) | R-main base 0, B, C, D, base 1; R-brook A, (470, 700), C, (554, 324), E | each is its own mirror |
| Roads (6 m) | base 0, (300, 200), E | base 1, (724, 824), A |
Base-to-centre distance is 488 m (Kessel Ridge 588 m), so the first fight comes sooner; the roads make the home-to-flank run faster for
vehicles only. Build B may move terrain features (ridge height and width, valley depth, woods) **only** to meet the base-fairness rows; it
never moves points or bases and never touches unit data. Octaves as Kessel Ridge (128 m / 5 m, 32 m / 1 m), its own terrain seed.

## 6. AI (S16b; no new AI code planned)
Every AI habit that cares about vehicle routes already asks the vehicle path search (`route_vehicle`, `own_route` and `gun_corridor` all go
through `gun_route`), so R3 plus the one-line `VehiclePathCost` edit (section 3, in S16a) makes it road-aware with no AI logic change:
- trucks, tows and transport drive the cheapest route, now along roads where that is faster;
- **belts and mines** (S10b-ai) sit on the enemy's predicted vehicle route (`ai/fort.rs:702`, `route_vehicle` at `:1538`) and keep the own
  route clear (`own_route`, `:1556`), so on Linden Brook they move onto the roads into each point (`mines_on_road_pct`). `route_vehicle`
  picks the least slope-able armour, so belts follow tracked routes (140 %), not truck routes;
- **AT guns** face the corridor of enemy armour routes (`ai/guns.rs:978` `gun_corridor`, `gun_route`), which now follow roads
  (`guns_facing_road_pct`).
A capped search returns None there (`ai/guns.rs:966` `r.reached.then`) and these direct AI searches are not charged to
`PathQueue`, so a silent fallback is possible: `ai_route_none` counts them per match. If a measure shows one of these failing, the fix is a named S16b row with its own switch (default off in the log, on after its ablation),
not a silent retune.
- **Audit** (Build B, in the log): every AI key in metres (front works, belts, gun reach, push limit, mortar and artillery siting, minimum
  ranges, medic post distance) read against Linden Brook's distances. A key that is wrong for the new map is reported, not retuned.

## 7. Determinism, cost, guard rails
No RNG. Road flags are a pure function of the setup. Hashes equal at 1, 2, 3, 8 threads and through save/load on both maps. Path cost:
today only a per-tick `path_expansions` stat exists (`armour_test.rs:3801`); Build A adds a per-search expansion count (CLI observer, AI
searches included). Its p99 and tick p99 from `tools/verify.sh` on Linden Brook stay at most 25 % over Kessel Ridge at `<BASE>`.

## 8. Measures (`s16_derived` per faction; `targets/S16.toml`, with a `map` key per run)
`road_km_pct` per drive type (share of vehicle metres driven on road nodes), `truck_trip_s` (median base to HQ or dump), `tow_trip_s`,
`stuck_vehicles_per_match`, `point_flips` per point (times each point changed hands), `point_never_taken_pct`, `mines_on_road_pct`,
`guns_facing_road_pct`, `ai_route_none`, `length_min`, `base0_win_pct` (mirror runs), each matchup's pooled share, `he_share_casualties_pct` (T8). Rows
with no design figure are reports. Definitions: stuck = a vehicle with a move order that gains under 2 m in 60 s; on road = the hull-centre
node has `FLAG_ROAD`; a mine on road = within 6 m of a road node; a gun facing a road = its arc holds a road node within its reach. All are
CLI observer counters (`skirmish.rs`), not saved.

## 9. Done when
**S16a**: Kessel Ridge 40 set at `<BASE>` and after S16a equal 40/40 (`tools/neutral.py`); `--map ww2:kessel_ridge` and no `--map` give the
same hashes; named tests pass: a truck on a straight road reaches `wheeled_pct` of its speed and off-road 100 %; woods on a road do not
slow it; a path between two road ends follows the road when that is faster and leaves it when it is not; `enabled` 0 restores cross-country
on a road map; validation rejects each R5 case; a truck stops on its last road waypoint (R1 `top`); a path along a road is not smoothed
off it (R3); base to the farthest point on Linden Brook reaches its goal under 8000 expansions for each drive type; on a blended
point-symmetric map no adjacent-cell grade across x + y = size exceeds the greatest grade elsewhere; `tools/test_neutral.py` cases for the
new `map` field; SAVE and PROTOCOL bumped; save fuzz 0 panics on a road map.
**S16b** (on Linden Brook unless named):
- each matchup's pooled share in 35-65 % on the 480 (Germany-USA; with S15 merged, also both USSR pairs: the 160 sets per pair);
- **base fairness (hard)**: mirror runs, Germany vs Germany and USA vs USA, 160 seeds each with `--both-bases`: `base0_win_pct` in
  42-58 (45-55 a soft row); Kessel Ridge's figure is printed beside it as a report;
- no match without a winner; length median 15-30, none under 10 or over 40 (owner rule);
- every point changes hands at least once in 50 % of matches (`point_never_taken_pct` report per point); `stuck_vehicles_per_match` not
  above Kessel Ridge's + 0.2;
- `road_km_pct` for wheeled vehicles at least 50 % (report for tracked); `truck_trip_s` lower than with `[vehicles.road] enabled` 0;
- `mines_on_road_pct` and `guns_facing_road_pct` printed against `road_off` (reports: they show the AI reads roads through R3);
- ablation on Linden Brook: `road_off` (`enabled` 0); a pooled move of 5 points or more stops the slice for the owner;
- T8 and every hard target of the B3 or S15 list read on Linden Brook; a new hard miss there is listed, not tuned (features first).

## 10. Build stages (a chunk is about 30 self-counted turns)
1. **S16a Build A** (Opus, 1-2 chunks): data model, R1-R5, the symmetry blend, SAVE and PROTOCOL bump, `--map` through the CLI and tools,
   named tests, neutral 40 on Kessel Ridge. Tools: `skirmish --map` (`skirmish.rs:2137`); the `measure.sh:36-50` inline parser reads `map`,
   `faction0` and `faction1` per run (mirror runs; `skirmish.rs:76-79`); `batch.sh sets` (`:123-128`) and the `measure.sh:34` 40 set take a
   map; JSON reports name the map; `neutral.py` (`:8-11` fails a new non-empty key) gets `map` in `HEADER_NOTED` and an `s16_derived` skip;
   `pool.py` groups by map. The batch world key hashes all of `mods/`, so adding `linden_brook.toml` invalidates every cached Kessel run:
   budget one uncached Kessel 480 in S16b.
2. **Checker after A** (Sonnet, fresh agent): diff against this file, named tests, neutral check; lists misses.
3. **Presenter P** (Sonnet, 1 chunk): section 1 list; Godot headless mock-match smoke test on both maps (Godot 4.7.2 headless downloads to
   `target/godot/`, as in S12).
4. **S16b Build B** (Opus, 1 chunk): `linden_brook.toml`, the mirror-placement test, AI metre audit, JSON `s16`, `S16.toml`,
   40 sets on Linden Brook, terrain shape only by the section 5 rule.
5. Orchestrator: `tools/measure.sh` 480 on Linden Brook, the two mirror 160s, ablations; Review (Opus; gate fires: protocol, path code);
   Fix (Opus); `tools/verify.sh`; long runs where the project's long-run rule says (2026-11-19: GitHub `ci.yml` by `workflow_dispatch`
   on the branch, 480s in the cloud in the evening); Docs (Haiku: STATE, TODO with S16-1..5, roadmap row). Fable consulted where a stage
   stalls.
Size: about 0.5x S3 (Build A small, the map and its measures most of the time).

## 11. Risks
1. **Path search cost.** A lower heuristic on road maps expands more nodes; the per-tick path budget (`PathQueue` debt) bounds the tick but
   can delay repaths. Measured; lever: divide the heuristic by half the road factor.
2. **The AI may have learned Kessel Ridge.** Habits tuned there (front works at the home point, mortar reserve, gun corridors, belts) may
   fail when C is low ground and B/D are high. The audit reports; a match share outside 35-65 % stops the slice for the owner.
3. **Base fairness.** Point symmetry fixes terrain, but the brook's line is diagonal to neither base; the hard row catches it and the
   section 5 rule allows only terrain moves.
4. **Shorter approach, shorter matches.** 488 m to C against 588 m; if the median falls under 15 min, the lever is to move the bases 40 m
   back. That changes how the map plays, so it is asked (S16-4), not done.
5. **Roads help wheels most.** Trucks and half-tracks gain more than tanks; supply and transport may swing Germany or the USA. Read the
   per-faction `truck_trip_s` and the `road_off` ablation.
6. **Parallel slices.** S13, S14 and S15 add units and AI habits on Kessel Ridge; S16b measures them on a new map for the first time, so a
   miss may belong to them. Every miss names the first slice whose rows it touches.

## 12. Design questions
**Owner questions (open; the work proceeds on (A) unless the owner picks another):**
- S16-1 **What S16 is.** (A) Second map and roads (this file). (B) 2v2 team play first. (C) The late-1944 US wave. (D) Buildings and a
  village (M4).
- S16-2 **Map size.** (A) 1024 m, as Kessel Ridge, so match length and artillery ranges stay comparable. (B) 1536 m, more room for
  vehicles and field howitzers, longer walks and matches.
- S16-3 **How Linden Brook plays.** (A) Centre point in a valley overlooked by two ridges, bases west and east (section 5). (B) A Kessel
  Ridge-like crest map with a different point layout. (C) Three points in a line, no flanks.
- S16-4 **If matches run short** (median under 15 min). (A) Stop and ask before moving anything. (B) Move both bases 40 m back without asking.
- S16-5 **Road speeds.** (A) Faster on roads by drive type: wheels 175 %, half-tracks 150 %, tracks 140 % of cross-country (sources verified
  in Build A). (B) One factor, 150 %, for all. (C) No speed change; roads only guide paths.
**Settled on (A)** (minor scope questions, owner rule 2026-11-11):
S16-13 Path cost by time. (A) Yes. (B) A flat 60 % road discount.
S16-6 Woods on a road. (A) The road clears the woods speed cap and path cost, spotting stays woods. (B) Woods win.
S16-7 Name. (A) Linden Brook (`ww2:linden_brook`), plain farmland usable for East and West matches. (B) An East-styled name after S15.
S16-8 Infantry on roads. (A) No change. (B) A road-march bonus for infantry out of contact.
S16-9 Road grading. (A) No terrain change; a road is a flag. (B) Flatten the road's cross-slope at generation.
S16-10 AI road habits. (A) Path cost only; belts, mines and guns follow through their route searches (section 6). (B) Extra road weights
in mine and gun siting.
S16-11 Base fairness band. (A) 42-58 hard, 45-55 soft. (B) 45-55 hard.
S16-12 Default map in menus and relay. (A) Kessel Ridge stays the default; Linden Brook is a choice. (B) Alternate.

## 13. Review record
Independent review (Fable, 2026-11-22, read-only, no runs) checked the code facts and raised 11 findings; this revision answers each:
point symmetry leaves a cliff on the anti-diagonal, now blended (1); the divided heuristic would hit the 8000-node cap, now
`heuristic_pct` with weight at least plain A* (2); smoothing could leave the road, cost is per node, `VehiclePathCost` needs a field (3);
the drive controller's `top`, the 2x save check and reverse (4); flags and setup are saved, so SAVE and PROTOCOL bumps are certain (5);
pushed guns (6); measure definitions and per-search expansions (7); tool changes and the cache rerun (8); presenter places (9); small
facts (10); road speeds and the base move are owner questions S16-4, S16-5 (11). The S17 suggestion stays a label in section 0; the TODO
wire item S17 is a different, older id.
