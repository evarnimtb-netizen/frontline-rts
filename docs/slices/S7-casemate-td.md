# Slice S7: assault guns and tank destroyers

Owner 2026-11-06: "Start S7". Branch `s7-casemate-td` from `9f216e9`. Engine tier, ~0.9x S3.
Scope: vehicles.md 6.2 (lean), 6.3, 6.5; StuG III Ausf. G and M10 3-inch GMC (8.1/8.2 order 1, 9).
Targets: `docs/slices/targets/S7.toml`. Owner questions S7-1..10 (section 12, `docs/TODO.md`).

## Baseline (verified S6b JSON, `scratchpad/verify-s6b`, not re-run)

| Measure | 160 (seeds 1-80) | oos (101-180) | 40 |
|---|---|---|---|
| Germany wins | 52.5 % | 50.6 % (pooled 51.6 % of 320) | 50.0 % |
| HE share of kills | 21.9 % | 21.7 % | 21.8 % |
| Mortar share G / U | 14.2 / 12.0 % | 13.2 / 11.9 % | 14.2 / 12.0 % |
| Vehicles bought / match G / U | 3.09 / 3.03 | 3.21 / 3.06 | 2.98 / 3.00 |
| Vehicle share of card MP G / U | 28.5 / 27.0 % | 28.3 / 26.0 % | 28.5 / 27.6 % |
| Vehicle share of kills G / U (1) | 23.5 / 21.2 % | 25.7 / 20.7 % | 25.0 / 22.2 % |

(1) tank_ap + tank_he + vehicle_mg. Base side max 51.3 / 60.6 %, median length 17.6 / 19.2 min.
All 52 S6b targets pass; tick p99 0.493 ms (timing40), case 14 1.512 ms. Save 16, protocol 9.

## Code map (what changes; lines at 9f216e9)

- content `vehicle.rs`: fixed `ArmourZone` 36-48, `SeatRole` 102, `ComponentKind` 146 ("each
  exactly once"), `MountKind` 231, `VehicleRole` 283 (only `Tank`), `MAX_*` 321-336, `VehicleDef`
  344-406 (`hull`, `turret`, `armor`, `crew` mandatory), `HullDef`/`TurretDef` 494-520, `ArmorDef`
  573, `CrewBoxesDef` 611 (keyed by role), `MountDef` 639, validation 918-1500, `VehicleSetup`
  1865 (`hull`, `turret_pivot`, `turret`, `armour: [_; 8]`), `lower_vehicle` 2032-2140.
- `armour.rs`: `BoxPart` 61, `Face::zone`/`of_zone` 105-131, `VehicleArmour.faces: [[_; 6]; 2]`
  331-366, `ArmourHit` 385, `hit_armour` 485-521 (hull, turret, barrel), `in_track_band` 523.
- `vehicle.rs`: `Geo` 503-583, `VehicleFrame`/`spot_points` 639-680 (turret top), `frame_of`
  682-722, `drive` 1186 (instant turn rate 1258), `halts_to_fire` 1392, FaceUnits pivot 1742-1762.
- `gunnery.rs`: `main_muzzle` 131, `lay_gun_pct` 158-188 (ring factor, no arc), `mass_centre`/
  `aim_choice`/`aim_at` 200-235, `f.turret.center` as the gun position 727-732, 804, 886.
- `damage.rs`: `part_box`/`anchor_at` 162-188, `inner_boxes` 189, `ray_burst` 232, `resolve` 408,
  `blast_tracks` 693, seat donors 1100. `combat.rs`: frag rays vs vehicles 2385-2470 (hull/turret
  probe 2399), `burst` 2127, `first_box` 1774, `fly` 1821, turret aim 1286/1497. `items.rs`:
  `fly_thrown` 401 (terrain only), `take_fused` 426, `lob_height` 718.
- `snapshot.rs` 904-960 (seats by role), `hash.rs` 335, `world.rs` 428 (`SAVE_VERSION`),
  `game.rs` 379 (`kills_by`), 2168 (`validate_vehicles`), `scenarios.rs` 144 `tank()` (first roster
  card tagged `vehicle`), 374 `duel`.
- AI: `ai.rs` `buy` 1002-1260 (the `vehicle` slot gates unlock, limit, savings), `vehicle_tasks`
  1723, `satchel_vehicle` 1973; `ai/assault.rs` 459, 863.
- CLI: `ballistics.rs` (zone names), `armour_test.rs` 435/630/679 (hull vs turret), 2581,
  `json_out.rs` ~316, `skirmish.rs` 499/1051. Bridge: `match_node.rs` 194-260 (`"role" => "tank"`
  229), 2292, 2465; `meshgen.rs` 230-280, 641, 835; `vehicle_view.gd` 154-230; `mock_vehicles.gd`
  23-32; `godot/docs/match_state.md` 497.

## 1. General vehicle schema (lean 6.2)

- `[[boxes]]`: 1..=4 armour boxes. Box 0 is the running-gear hull (frame `hull`; track band,
  footprint, avoidance circles). Each box: `id`, `frame` (`hull` | `turret`), `box`, a 4-plate table
  `[boxes.armor]` front / side / rear / top (bottom reads top, as in S3), optional `open = ["top",
  ...]` (no plate on that face) and `floor_cm` (items rest there). Two boxes are exactly the S3
  8-zone table, so each box "has its own table" without empty rows. **Zone id** = box index x 4 +
  group (front 0, side 1, rear 2, top 3); name `<box id>_<group>`. Boxes named `hull`, `turret` in
  that order give the S3 numbers and names (hull_front = 0 ... turret_top = 7), so events, stats,
  the bridge and the golden file stay byte-identical (S7-9).
- `[turret]` optional: `pivot`, `traverse_deg_s`, `elevate_deg_s`; needs one `turret`-frame box and
  a `turret_ring` component; forbidden with a `casemate` mount. No turret: the gun frame is a zero
  box at the mount pivot (`VehicleFrame.turret` keeps its role as the gun frame).
- Mounts: `turret_main` | `casemate` (`pivot` hull frame, `traverse_deg = [left, right]`,
  `traverse_deg_s`, `elevate_deg_s`) | `coax` | `hull`. `MAX_MOUNTS` stays 4; ammo list stays at
  most 3.
- `[[seats]]`: `id`, `role`, `frame`, `box`, `in` (armour box id; default `hull`). Roles unchanged
  (5). `seat_ids` go to the presenter as labels.
- Components: `turret_ring` optional (absent iff no turret; its state slot stays 0 and nothing reads
  it); the rest required as now.
- `role`: `tank` | `assault_gun` | `tank_destroyer`.
- Old keys `[hull] box`, `[armor]`, `[crew]` are removed (one format); `[hull]` keeps
  `track_band_pct`, `track_mm`.

```toml
# ger_pz4_h.toml after the port (excerpt; values unchanged). [hull] keeps the track band.
[[boxes]]
id = "hull"                    # box 0: running gear
frame = "hull"
box = { center = [0, 0, 108], half = [290, 140, 68] }
[boxes.armor]
front = { thickness_mm = 80, slope_deg = 10 }
side = { thickness_mm = 30 }
rear = { thickness_mm = 20, slope_deg = 10 }
top = { thickness_mm = 12 }
[[boxes]]
id = "turret"                  # then the turret: zones 4-7 as before
frame = "turret"
box = { center = [0, 0, 45], half = [115, 95, 45] }
# [boxes.armor] front 50 @ 10, side 30 @ 25, rear 30 @ 15, top 16
[[seats]]
id = "driver"
role = "driver"
box = { frame = "hull", center = [190, 60, 115], half = [35, 30, 40] }

[[weapons]]                    # ger_stug3_g.toml: no [turret], casemate gun
def = "ww2:stuk40_l48"
mount = "casemate"
pivot = [110, -15, 165]        # hull frame; the gun sat right of centre
muzzle = [300, 0, 0]           # 1.39 m overhang (6.77 m with gun - 5.38 m hull)
traverse_deg = [-10, 10]
traverse_deg_s = 6
elevate_deg_s = 4
elevation_deg = [-6, 20]
# usa_m10.toml turret box: frame = "turret", open = ["top"], floor_cm = 0
```

## 2. Port plan and proof of no behaviour change

Build A: (1) content lowering gives the two tanks the same plates, boxes and zone ids; (2) sim
`VehicleArmour.faces[box]`, `hit_armour` loops boxes in index order then the barrel (S3 tie order
"hull, then turret" holds), `Geo`/`VehicleFrame` read box 0 and the upper box (turret, else the
highest) where they read hull/turret, mass centre sums boxes in index order (two boxes: the same
arithmetic); (3) CLI and bridge read box names. No RNG draw is added or reordered.

Proof, before adding the new cards to the rosters (new defs present but unbought do not change
play):
- `frontline-cli ballistics --golden mods/ww2/tests/ballistics.toml`: all 46 rows pass unchanged
  (file not edited).
- `armour-test --seeds 5 --json`: every check value of cases 1-16 equal to `verify-s6b/armour5.json`
  (except wall time); diff by a python one-liner.
- `skirmish --seeds 20 --both-bases --json`: every `per_match` field except `hash` and every
  `factions` number equal to `verify-s6b/skirmish40.json`.
- `determinism.rs` `GOLDEN_CONFIG7_TICK600` unchanged (no vehicles).
- Expected differences, explained not hidden: the state hash and setup hash (postcard bytes of
  `MatchSetup` and `VehicleState` change shape), so `per_match.hash` and save files differ. Any
  other difference stops the port until explained in the log.

## 3. Casemate gunnery (6.3)

- `lay_gun` clamps the gun yaw (hull-relative, kept in `turret_yaw`) to the mount arc; the ring
  factor applies only with a turret. Laid = inside the arc and on the solution.
- Target bearing outside the arc minus `vehicles.casemate.margin_deg` (3): the gunner sets
  `VehicleState.lay_turn = Some(world yaw)`. In Movement the driver pivots toward it with the
  FaceUnits steer (proportional, `face_done_deg`); the drive's turn per tick is at most the
  remaining error, so it cannot overshoot. Cleared when the bearing is back inside the arc, the
  target is gone, or an order moves it. Precedence: a followed path, a player `FaceUnits`, then
  `lay_turn`; attack-move halts to fire, then pivots. Orders, auto targets and the AI share it.
- `aim_choice`: the "upper" option is the upper box (StuG casemate). `spot_points`: upper box top,
  then hull top. Low silhouette only through `spot_size_pct`: StuG 160 (2.16 m vs Sherman 2.74 m x
  200 %), M10 190 (2.57 m).

## 4. Open-top exposure (6.5)

- `hit_armour` reports the entry face and, for an open entry, the exit (t, face). A round or
  fragment entering an open face meets no plate. It tests seat and component boxes inside
  between entry and exit (`ray_boxes`). AP/HEAT: a penetration with no spall (core damage, filler as
  now); HE: bursts at the first inner box or at the exit point (`inside_rays` / `inside_damage` /
  `inside_reach_cm`, cone around the path); small-arms rounds: their damage to the first seat box,
  nothing to components. Leaving through a plated face from inside: stops there.
- Exposed seats (`in` an open box) are also frag-ray and bullet targets from outside: the earliest
  of terrain, plate, exposed seat box, man wins (plates block; no fragment pierces a plate in S7,
  S7-8). Burst overpressure reaches an exposed seat within `blast_radius` when the line from the
  lift point to the seat box centre crosses no plate of that vehicle (the S1 line test plus
  `segment_box` blockers). Seat boxes of M10 turret crew reach 10-25 cm above the rim, so
  near-misses wound them; closed vehicles are unchanged.
- Thrown items sweep enemy vehicle boxes as well as terrain (`fly_thrown`, broadphase
  `segment_near`). A plate strike: the item drops to the ground below the strike point (S7-5). An
  open-face entry: it lands on the box floor and rides with the vehicle (`Thrown.inside =
  Some((vehicle, box, local))`); at fuse it bursts inside: `inside_rays` rays in a 90° cone up from
  the floor (`vehicles.open_top.item_cone_deg`), `inside_damage` each to the first box; outer frag
  rays from it leave only through the open face. Own vehicles are ignored (no friendly fire).
  `lob_height`: an aim point inside an enemy open box's footprint aims at its floor + 30 cm, so a
  lob drops in; the physics decides (no success roll). Grenade screens: not now.
- RNG: new purpose `INSIDE_ITEM` (item id) for inside rays; open-entry rounds reuse the pen keys.

## 5. Content (sources in TOML comments; "verify" flags stay)

**StuG III Ausf. G** (`ger_stug3_g`, card `ger_stug3_g_squad`). Sources: Jentz & Doyle, Panzer
Tracts 8-2 (Sturmgeschütz); Wikipedia "Sturmgeschütz III" (spec table). Boxes: hull 540x290x104 cm
(z 38-142), casemate (hull frame) about 300x220x66 cm on top (z 142-208). Armour: hull front 80
(50+30) @ 21, side 30 @ 0, rear 50 @ 10 (verify), top 16; casemate front 80 (50+30) @ 10, side 30 @
11, rear 30 @ 30 (verify), top 10. Gun 7.5 cm StuK 40 L/48 (`stuk40_l48`: KwK 40 data, same
ballistics; PzGr 39 / Sprgr 34 / PzGr 40 reused), arc 10/10, 6 deg/s hand traverse (no source,
S7-10), elevation -6/+20. Stowage 54 (PT 8-2): 27 PzGr 39, 24 Sprgr 34, 3 PzGr 40 (rare_cap 3). Crew
4: driver (hull front left), gunner (left of gun), commander (behind gunner), loader/radio (right);
hatches: commander+gunner roof (driver leaves by it), loader roof. No MG (S7-1). Components: engine,
transmission, tracks, breech (hull frame), rack bins in the casemate sides; no turret ring. Speed 40
km/h road -> 620 cm/s (Pz IV 38 -> 600), turn 30, reverse 200. Cost 500 = Pz IV 620 x 82,500 /
103,462 RM (unit prices, Wikipedia), pop 10, cooldown 120 s (owner's tank cooldown; 8.4 said 90).

**M10 3-inch GMC** (`usa_m10`, card `usa_m10_squad`). Sources: Hunnicutt, Sherman (1978) M10
chapter; Zaloga, M10 and M36 Tank Destroyers (Osprey NV 57); TM 9-752. Boxes: hull 580x300x140 cm
(M4A2 lower hull), open-top turret (frame turret, pivot about [-20, 0, 190]) 220x210x60 cm. Armour:
hull front 38 @ 55, side 19 @ 38, rear 19 @ 38 (verify), top 19 (verify); turret front 57 @ 45, side
25 @ 20, rear 25 (verify), top open. Turret traverse 4.5 deg/s by hand (a full turn about 80 s;
vehicles.md 4.4), elevation -10/+19, 4 deg/s. Gun 3-inch M7 (`m7_3in`: range 600, reload 6500 ms,
spread 2.5 mrad, 792 m/s). Rounds: `3in_m62` APC (792 m/s, 12.4 kg complete (verify), filler as the
M61 with 12 rays); penetration row derived (S7-6): US 30° table 93 / 88 / 82 / 75 mm at 500-2000 yd
(Hunnicutt firing data, verify) / cos 30° x 1.066 (the B&L/US ratio measured on the M61, 81/76) ->
[[100,118],[457,114],[914,108],[1371,101],[1829,92]] at 0°. `3in_m42a1` HE (853 m/s, 10.9 kg
(verify)): the M48's HE scaled to the lighter shell and filler (0.86 vs 1.47 lb TNT): blast_radius
100 cm (filler^(1/3)), frag_count 70 (body mass 12.9 vs 14.6 lb), other HE values as the M48.
Stowage 54 (Hunnicutt): 36 M62, 18 M42A1 (split verify). Crew 5: driver, assistant driver
(`radio_mg` role, no hull MG, S7-4) in the hull front; gunner, loader, commander in the turret, seat
boxes 10-25 cm above the rim. Rack: hull sponson bins both sides plus turret-bustle rounds (verify
count). Speed 30 mph (48 km/h) -> 740 cm/s (Sherman 42 -> 650), turn 32. No .50 (S7-1). Cost 520
(8.4's 500 vs a 560 Sherman, x 590/560): the best gun of the four at combat range, against an open
top, 19 mm sides and a slow turret. Pop 10, cooldown 120 s. Both cards are appended to the rosters
(scenario and AI tie-breaks keep picking the tanks first).

**Later (data):** Jagdpanzer IV: hull + casemate 60 @ 50 (80 late), PaK 39 L/48 (KwK 40 data), 79
rounds, arc ~+-10. Jagdpanzer 38(t): casemate 60 @ 60, arc [-5, 11], 41 rounds, spot ~150 %. M18:
open turret 25 / hull 13 mm, powered traverse, 76 mm M62 + HVAP (6.11), 13 mm needs 6.4, 89 km/h.

## 6. AI

- Buy: StuG and M10 cards carry tag `vehicle` and join the existing `vehicle` mix slot; the split is
  the cards' `ai_weight` (tank 100, new 100: the min-buys-per-weight pick alternates them, so with
  about 3 vehicle buys a match the new card comes second). New `[ai.mix]` slots were rejected:
  unlock, vehicle limit and saving logic key on the `vehicle` slot, so a `td` slot would bypass them
  (S7-2). Vehicle count and MP share stay as now; only the mix of types changes.
- Casemate and TD habits (`[ai.armour]`, role `assault_gun` or `tank_destroyer`): overwatch instead
  of leading assaults. Anchor: `overwatch_back_m` (80) behind the most threatened own or contested
  point, on the line toward the threat axis (the known enemy vehicle nearest that point, else the
  enemy side of it). Spot: best of `overwatch_candidates` (8) on a `overwatch_ring_m` (30) ring by
  terrain lines: hull-down (gun-height line clear, hull-centre line blocked) 2, clear 1, ties
  nearest. Then `FaceUnits` toward the axis and hold; it fires through `lay_turn`. After
  `relocate_after_shots` (2) shots at a spot, or a penetration, it moves to the next best spot at
  least `relocate_m` (40) away. With a known enemy vehicle within `td_hunt_m` (300), the anchor
  moves to face that vehicle.
- Stand off: an open-top vehicle or casemate keeps `td_infantry_standoff_m` (120) from known enemy
  infantry when choosing spots; an open-top vehicle with known enemy infantry within
  `open_top_infantry_m` (60) reverses away (`ReverseUnits`).
- Infantry: a squad with frags or satchels within `[ai.infantry] open_top_throw_m` (25, inside item
  range) of a known enemy open-top vehicle slower than `open_top_slow_mm_s` (300) throws at its open
  box centre (existing `Throw`; extends the satchel/frag target choice).
- New AI state (saved, hashed): `overwatch: BTreeMap<SquadId, {spot, axis, shots, since}>`, 4 counters.

## 7. Presenter

- `rules.vehicle_types[t]` gains `role`, `casemate`, `boxes: [{id, frame, center, half, open}]`,
  `gun_pivot`, `seat_ids`; `match_state.md` "S7".
- `meshgen.rs` builds one mesh per box (open faces left out, 6 cm walls), style-neutral blockouts:
  StuG = low hull + casemate block with the gun through its front; M10 = hull + open-topped turret
  walls. `vehicle_view.gd`: one MultiMesh per (type, box) plus the gun; hull-frame boxes take the
  hull transform, turret-frame boxes the turret's; a casemate gun turns about its pivot by
  `turret_yaw`. The mock source gets one casemate and one open-top type.
- Call-in tiles for the two cards; a screenshot of a StuG and an M10 in `scratchpad/s7/`, looked at.

## 8. State, save and versions

`SAVE_VERSION` 16 -> 17 once (Build A, first state change). New fields, each saved, loaded, hashed,
validated on load, with a per-field test: `VehicleState.lay_turn` (|yaw| <= pi); `Thrown.inside`
(vehicle exists, box index has an open face, local point in the box); per-player `kills_by_vehicle`,
`kos_by_vehicle` (length = setup vehicle count); AI `overwatch` and the four counters.
`PROTOCOL_VERSION` stays 9: no command or wire change (`Throw` stays a point; setup by hash only).

## 9. Tests

- Port proof (section 2); content test: Pz IV and Sherman lower to the S6b plate normals,
  thicknesses, boxes and zone names; validation (box count, frames, open faces, arc bounds, casemate
  xor turret, seats in their box, ring iff turret, unique zone names).
- Sim: pivot-to-lay (bearing 40° off: error falls monotonically, ends within `face_done_deg`, gun
  lays and fires); arc clamp; AP through the M10 top hits a seat; a frag ray over the rim hits an
  exposed seat, one into the wall stops; item lands inside and rides; inside burst; overpressure
  blocked by a side plate.
- armour-test (vehicles.md 10): 17 StuG vs Sherman frontal 300 m; 18 Sherman 45° off the StuG's
  nose at 200 m (wins %, first hit s per side, StuG fired %); 19 frags at a stopped and a moving M10
  from 15 m (inside %, crew hit %); 20 81 mm bombs 3-6 m from an M10 and a Sherman (crew hit %).
- AI: new cards bought; overwatch held and left after firing; infantry throws into a stopped M10;
  an M10 backs off from infantry. Determinism 1/2/3/8 threads with a StuG and an M10 fighting and
  grenades inside, save/load/continue; soak passes. Release run with `--include-ignored` per report.

## 10. Targets (S7.toml)

S6b T4-T27 unchanged; new T30-T38 (thresholds and baselines in the file). JSON fields to add:
`factions.<f>.s7.{kills_by_vehicle, kos_by_vehicle, open_top_throws, relocations}` and
`factions.<f>.s7_derived.{bought_matches_pct.<card>, mp_share_pct.<card>, vehicle_mp_pct,
kills_by_vehicle_pct.<vehicle>, vehicle_kills_pct, kills_by_vehicle_max_pct,
kos_per_bought.<vehicle>, open_top_throws_per_match}`; armour-test cases 17-20 with the check names
in S7.toml. A kill counts for a vehicle type when the round, spall or burst came from a vehicle of
that type (projectile owner); later fire deaths stay in `kills` only. MP shares = bought x card
cost. Balance: Germany 45-55 % per 160 set and pooled; mortar 10-15 %, HE 20-26.3 % stay met.

## 11. Build order

Build A: schema, port, casemate, data, armour tests 17-18, save v17. Build B: open faces, cases 19-20,
AI, presenter, balance runs (prices, `ai_weight`, AI distances only; never lethality). Done; see the log.

## 12. Owner questions (work proceeds on (A); also in `docs/TODO.md`)

- S7-1 No StuG roof MG 34 or M10 .50 (pintle mounts out of scope). (A) None; add with pintles, S8.
- S7-2 New cards buy through the `vehicle` mix slot by card `ai_weight` (100 each). (A) Yes.
- S7-3 StuG 500 MP / pop 10 / 120 s; M10 520 / 10 / 120 s. (A) As stated; Tune within +-15 %.
- S7-4 M10 assistant driver takes the `radio_mg` role (no hull MG), labelled by seat id. (A) Yes.
- S7-5 An item striking a plate drops to the ground below (no deck charges, no screens). (A) Yes.
- S7-6 M62 penetration row = US 30° table x 1.066 / cos 30° until a B&L row is read. (A) Use it.
- S7-7 Small arms do not aim at open-top crews (stray rounds still hit). (A) Yes; aimed with 6.4.
- S7-8 Fragments never pierce plates in S7 (all plates here >= 10 mm). (A) Yes; rule with 6.4.
- S7-9 Zone id = box x 4 + face group, name `<box>_<face>` (`casemate_front`). (A) Yes.
- S7-10 StuG traverse 6 deg/s, elevation rates 4 deg/s: no source. (A) Starting values, verify.

## 13. Tune results (Tune stage, autotune on the StuG price)

One knob: `ww2:ger_stug3_g` cost 425-575 (S7-3 +-15 %), 800 matches (seeds 3001-3400 both bases), target
Germany 45-55 %. Slope +0.2 points per 15 MP [-0.7 to +1.0]: the price does not move the result
(Germany 49.3 / 56.7 / 51.1 % at 425 / 500 / 575, pooled 51.7 %; model at 500: 52.1 % [48.6-55.6]).
Prices stay as shipped. T5c's 43.75 % (seeds 1-80) is sampling noise (+-7.7 points); seeds 201-280:
51.9 % (T5d band met, base max 53.1 %), HE mean 21.0 %, mortar share 14.1 / 11.9 %. T34e and T36a stay
report targets (S7-11, S7-12). The StuG/M10 weakness (kill shares 2.5 / 1.8 %) needs AI code, not price.

