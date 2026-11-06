# S8a: half-tracks (spec)

Owner 2026-11-06: "start an overnight session for next development process". Roadmap S8 is split for the plan
limit: **S8a** = SdKfz 251/1 Ausf. D, M3A1 and their systems (vehicles.md 6.4, 6.6 lean, 6.7, 6.8 half-tracks);
**S8b** = trucks, recon cars, wheels, recon optics, supply trucks (6.8 wheels, 6.9, 7). Branch `s8a-halftracks`
from `17a94df`; targets `docs/slices/targets/S8a.toml`; questions S8a-N in section 14.

## 1. Baseline (verified S7 JSON after the tick fix, no rerun)

| Set | Germany % | base max % | HE % | mortar G / U % | vehicle kills G / U % | top type G / U % |
|---|---|---|---|---|---|---|
| 160 (1-80) | 48.1 | 50.6 | 20.9 | 14.2 / 11.9 | 21.8 / 17.9 | Pz IV 18.9 / Sherman 16.1 |
| oos (101-180) | 44.4 | 58.1 | 20.6 | 13.9 / 11.7 | 20.7 / 18.0 | 17.4 / 16.0 |
| s201 (201-280) | 48.1 | 59.4 | 21.7 | 14.6 / 12.3 | 20.1 / 18.1 | 16.9 / 15.8 |
| 40 (timing40) | 45.0 | 55.0 | 20.6 | 13.4 / 11.3 | 22.8 / 20.0 | 19.2 / 17.7 |

Pooled Germany 46.9 % / 480. Vehicle MGs 6.6-8.5 % of kills; StuG 3.0-3.6, M10 1.9-2.3 %. Tick p99 0.38 / 0.56
ms. S8a.toml on this JSON: 91 pass, 37 miss (T5d, T32a as in S7; the rest are new fields and cases 21-24).

## 2. Code map (where each change goes)

| Area | File:line (at 17a94df) | Change |
|---|---|---|
| Vehicle schema | `sdk/crates/content/src/vehicle.rs` 239 `MountKind`, 303 `VehicleRole`, 323 `MobilityKind`, 344 `ArmourMaterial`, 376 `VehicleDef`, 674 `MountDef`, 1303 "exactly one main gun", 2186 `VehicleSetup`, 2246 `main_gun()`, 353/360 `MAX_MOUNTS`/`MAX_INNER_BOXES` | `Pintle`, `Transport`, `Halftrack`, `FaceHardened`; `[transport]`, `[[passenger_slots]]`, `[[shields]]`; main gun 0..=1; inner cap 14 -> 24 |
| Weapon schema | `sdk/crates/content/src/defs.rs` 536 `WeaponDef`, 1026 `HeDef`; `setup.rs` 709 `WeaponSetup`, 807 `HeSetup` | `penetration`, `core`; `frag_pen_mm` |
| Rules | `defs.rs` 1285 vehicle rules; `mods/ww2/defs/rules/skirmish.toml` 443 `[vehicles]`, 796 `[ai.mix]` | `transport_unlock_s`, `face_hardened_pct`, `[vehicles.transport]`, `[ai.transport]` |
| Main gun optional | `gunnery.rs` 180, 379, 655 (`skip(1)`), 893, 1138, 1182, 1264, 1295; `damage.rs` 1488; `snapshot.rs` 962; `spotting.rs` 270; `supply.rs` 903; `structures.rs` 1291; `scenarios.rs` 328; content `setup.rs` 1468, 2829; `modset.rs` 1122, 1416-1463 | `main_gun() -> Option`, `mg_mounts()` slice |
| Small arms vs plates | `combat.rs` 1774 `first_box`, 1993-2008 (`Blow::Stopped`), 386 `search`, 295 `valid_target`, 320; `armour.rs` 231 `penetrates`, 349 `VehicleArmour`, 465 `HitVia`, 562 `hit_armour` | pen-table path, core, vehicle targets |
| Fragments | `combat.rs` 2277 `fragments` (2440-2530 box rays) | thin-plate rule |
| Damage inside | `damage.rs` 155 `Inner`, 189 `inner_boxes`, 598 `open_hit`, 717 `deal`, 910 `hit_open`, 945 `blast_open_crew`, 1025 `inside_item`, 1119 `knock_out_with` | `Inner::Rider(slot)`, small-arms core |
| Pintle MG | `gunnery.rs` 140 `mg_muzzle`, 650-720 MG loop, 1061 `mg_fire`; `vehicle.rs` 125 `MgState` | arc + traverse, `yaw` |
| Mobility | `vehicle.rs` 1263 `drive`, 1370 bog roll, 1839 `follow` (S7 fix 1886-1905) | kind flag only |
| Riding | `ecs.rs` 239 `Entities`; `unit.rs` 165 `Unit`, 620 `steer_units`; `squad.rs` 350 `steer_squads`; `systems.rs` 184 `plan_velocities`, 360 `integrate_soldiers`; `combat.rs` 1671/1899 capsule; `spotting.rs` 76; `rules.rs` 133 `update_capture`; `bail.rs` 553 `get_out`, 749 `boarding` (patterns to reuse) | `Unit.riding`, skips |
| Orders, commands | `game.rs` 105 `Order`, 225 `Squad`, 882-920 call-in checks, 1022 apply; `command.rs` 27 `CommandKind`; `crates/net/src/protocol.rs` 30; `sdk/docs/net/relay-protocol.md` | `Order::Embark`, `Embark`, `Disembark`, PROTOCOL 10 |
| Save, hash | `world.rs` 431 `SAVE_VERSION`, 743 `load` (validation); `hash.rs` 135 `Unit`, 541 vehicles (postcard: new `VehicleState` fields hash themselves) | SAVE 18 |
| AI | `ai.rs` 16-153 docs, 732 `MyTank`, 906, 1183/1309 limit, 1662 upkeep; `ai/armour.rs`, `ai/overwatch.rs` 61/237 | new `ai/transport.rs`; tanks code skips `Transport` |
| CLI | `armour_test.rs` 2884 `case19`, 3241 table; `json_out.rs` 358 `s7_derived` | cases 21-24, `s8a`/`s8a_derived` |
| Presenter | `crates/godot-bridge/src/match_node.rs`, `present.rs`; `godot/match/vehicle_view.gd`, `vehicles.gd`, `mock_vehicles.gd`, `match_input.gd` 21-46, `hud/detail_panel.gd`, `hud/orders_grid.gd`, `hud/roster.gd`; `godot/docs/match_state.md` | blockouts, riders, cards, buttons |

## 3. Schema additions

**Main gun optional (S8a-1).** `0..=1` main mounts. `VehicleSetup::main_gun()` returns `Option<&MountSetup>`;
`mg_mounts()` = the mounts after the main gun. `VehicleState::mgs` keeps one entry per MG mount in that order,
so the four existing vehicles keep their indices. A vehicle with no main gun never runs main-gun targeting,
laying, reload, HE suppress or rack rearm; the bail "useless" trigger (4) reads "every MG out or unmanned and
broken down".

**Role and limits (S8a-5).** `role = "transport"`. Transports do not count in `[vehicles] limit` (4) and take
`[vehicles] transport_unlock_s` (120) as the unlock floor instead of `unlock_s` (240); their cards set
`max_alive = 2`. The AI tank code (`MyTank`, assault, overwatch, rearm) skips transports.

**Material (S8a-4).** `material = "face_hardened"` on a plate multiplies its thickness by `[vehicles]
face_hardened_pct` (125, verify: FH plate resists small calibre about 1.2-1.4x RHA) in `armour::penetrates` and
the reach table.

**Weapons (6.4).** Optional `penetration` (`[range_m, mm]` at 0 deg, linear between rows, flat past the last)
and `core` on any small arm. No table: the weapon never beats a plate (pistols, SMGs, 7.92 Kurz, .30 carbine).

```toml
# defs/weapons/m2hb.toml (new; .50 M2HB flexible, belt of M2 AP)
class = "mg"
feed = "ww2:50bmg_belt"           # new cartridge, ~120 g a round with link (verify)
penetration = [[0, 28], [100, 25], [500, 19], [1000, 11]]  # mm RHA at 0 deg (vehicles.md 6.4; verify)
core = { damage = 60, length_cm = 60, max_boxes = 1 }      # HP to a man/part; reach past the plate; boxes hit
# defs/weapons/mg34.toml and mg42*.toml: SmK as the table (S8a-3), verify
penetration = [[0, 13], [100, 11], [300, 7], [600, 4]]
core = { damage = 35, length_cm = 40, max_boxes = 1 }
# m1919*.toml (.30 M2 AP): [[0,12],[100,10],[300,7],[600,4]]; rifles and BAR
# (ball): [[0,7],[100,5],[300,3]]; all verify.
```

**Thin-plate fragment rule (S7-8, one rule).** HE defs get `frag_pen_mm` (mm RHA at the burst, falling with
distance as `frag_damage` does through `frag_far_pct`). A fragment ray that strikes a plate pierces it when the
plate's effective thickness (thickness x material / cos) is below that value, then hits the first seat, rider or
component inside (as an open-face ray does, `combat.rs` 2514-2530). Values (verify): 75 mm and 3-inch HE 12, 81
mm and 8 cm mortar 9, hand grenades 3, rockets 0. Plates of 10 mm or more stay proof against all but a close 75
mm burst; the M3A1's 6.35 FH side (7.9 mm effective) falls to mortar bursts within a few metres. Tanks:
unchanged except roofs under 12 mm against a 75 mm burst on them.

**Transport (6.6).**

```toml
[transport]
capacity = 10                 # passengers (slots)
entry = [-300, 0, 60]         # rear door, hull frame, cm: men board and exit here
board_s = 1.0                 # s per man boarding (door)
exit_s = 0.8                  # s per man out of the door
exit_over_sides_s = 0.5       # s per man over the sides; omit = no side exits
exit_raise_cm = 50            # a man going over the side stands this much higher for his exit time
[[passenger_slots]]           # one per seat, hull frame; the box is the man's hit volume
box = { center = [-60, 70, 150], half = [22, 20, 45] }   # seated, top 1.95 m vs a 1.75 m rim
```

`[vehicles.transport]` rules: `stun_s = 2` (dismount after knock-out or fire: no fire, `bail_suppression`),
`board_reach_m = 3` (a man boards within this of `entry`), `move_block_cm_s = 100` (no boarding or exit while
faster).

**Pintle and shields (6.7).** `mount = "pintle"`: `pivot` (hull frame), `muzzle` (from the pivot), `traverse_deg
= [min, max]` from the hull front (positive left; `[-180, 180]` = ring), `traverse_deg_s`, `seat` (`commander`
for both half-tracks, S8a-10), `rounds`. `MgState.yaw` (saved, hashed) turns at the rate; the MG fires only
within its spread of the target bearing.

```toml
[[shields]]                   # armour only, no contents; at most 2 per vehicle
mount = 1                     # follows that pintle's yaw about its pivot; omit = hull frame
box = { center = [25, 0, 20], half = [1, 28, 22] }
thickness_mm = 8              # one plate all round (verify)
```

A round or fragment that strikes a shield runs the penetration test once; a pass continues the same segment
(shield excluded); a stop ends it. Shields have no zone id; small arms stopped on one emit nothing.

**Mobility (6.8, S8a-6).** `type = "halftrack"` (`wheeled` reserved, S8b). Same `drive` as tracked (neutral
pivot allowed); the difference is data: higher `bog_pct`, lower `turn_deg_s`, `ditch_block_dm = 19` (crosses
trenches like tanks, stops at AT ditches). Speeds are cross-country practical, not road.

## 4. Small arms against thin armour (6.4; settles S7-7)

- **Hit path.** A small-arms round striking a plate with a weapon that has a table runs `armour::penetrates`
  (pen0 from the table at the round's flown range, the plate's cos and material). Stop: `Blow::Stopped` as now,
  no event. Pass: the core continues `length_cm` and hits at most `max_boxes` inner boxes (seats, riders,
  components) for `core.damage`; no spall, no filler, one `ArmourEventKind::Penetrated` event (presenter
  coalesces per vehicle per 2 s, S8a-12). Rounds into open faces and exposed seats as S7.
- **Reach table.** At load, per (weapon, vehicle type, zone): the greatest range at which the table beats the
  plate at 0 deg incidence on its slope (`effective_thickness`), cm, `0` = never. Not saved (derived).
- **Targets.** In `search`, after soldiers, a man may pick a seen enemy vehicle within weapon range when (a) it
  has an occupied exposed box (an exposed seat, an exposed rider slot, S8a-9) or (b) the reach of its facing
  zone (from the bearing to the hull yaw) is at least the distance. Nearest target wins, soldiers before
  vehicles on a tie. Aim: (a) the exposed box's top centre, (b) the facing plate's centre. Vehicles never
  attract suppress fire. AI soldiers use the same rule (no AI code).
- **Budget.** `armour_box_tests` counts the tests; T43c reports pens per match; T14b (stress tick) holds.

## 5. Transport (6.6 lean)

- **State.** `Unit.riding: Option<Riding { vehicle: SimId, slot: u8 }>`; `VehicleState.riders:
  Vec<Option<SimId>>` (one per slot), `door_ticks: u16` (time to the next man in or out), `unload: Option<Unload
  { over_sides }>`; `Order::Embark { vehicle }` on the squad. Riders keep HP, kit, weapons, squad and
  suppression.
- **Riding man.** Skips path requests, squad and unit steering, ORCA, push-out, integrate and capsule hits; his
  transform is set to the slot after vehicles move (stage 3); his hit volume is the slot box
  (`Inner::Rider(slot)` for penetrations, bursts inside and fragments; `HitVia::Rider(slot)` when the box rises
  above the rim, tested like exposed seats). He takes near-miss suppression. He observes from the slot top minus
  `eye_drop`. He is seen when the vehicle is seen and his slot is exposed (bits copied, no rays). He does not
  fire, throw, capture, contest or count in a point. Pop unchanged.
- **Embark { squads, vehicle }.** Own live transport, not burning, not abandoned. A squad whose living men
  exceed the free slots is refused (`NoRoom`; squads never split, S8a-2). Men walk to `entry`; while the vehicle
  is slower than `move_block_cm_s`, one man within `board_reach_m` boards per `board_s` (squad order, lowest
  free slot). A selected mounted squad takes only `Disembark`; its move orders are ignored by the sim (the HUD
  sends them to the vehicle).
- **Disembark { vehicles, over_sides }.** Stops the vehicle; one man per `exit_s` out of the door (placed at
  `entry`, then spread as `bail.rs` `get_out` places crews), or per `exit_over_sides_s` over the side away from
  the last hit (box raised by `exit_raise_cm` during his exit time). The squad forms up and goes `Idle`.
- **Knock-out, abandon, burning.** All surviving riders out at once, stunned `stun_s`; the knock-out's
  penetration has already hit their boxes. A rack or wreck fire with men inside kills them as it kills crew.
- **Not now:** tank riders (desant), fire from the vehicle (`fire_from` stays unparsed), boarding over the
  sides.

## 6. Content (both vehicles: boxes and numbers verify unless a source says)

| | SdKfz 251/1 Ausf. D | M3A1 |
|---|---|---|
| Card (id) | `ger_sdkfz251_1_squad`, vehicle `ger_sdkfz251_1` | `usa_m3a1_squad`, `usa_m3a1` |
| Cost / pop / cooldown / cap / unlock | 290 MP / 3 / 60 s / 2 / 120 s | 310 MP / 3 / 60 s / 2 / 120 s |
| Size L x W x H | 5.98 x 2.10 x 1.75 m | 6.18 x 2.22 x 2.26 m (with ring) |
| Boxes | `hull` (engine, closed), `compartment` (cab + troops, open top) | `hull` (hood, closed), `body` (cab + troops, open top) |
| Armour | front 14.5 @ 30, sides 8 @ 30, rear 8, engine top 5.5 | front 12.7 FH, sides and rear 6.35 FH, hood top 6.35 FH |
| Crew | driver, commander (MG) | driver, commander (.50 on the ring) |
| Capacity | 10 (heads and shoulders over the rim) | 11 (1 cab middle seat + 10 rear) |
| MG | MG 34 pintle front, arc +-35, shield 8 mm, 60 deg/s, 2,010 rds | M2HB on the M49 ring, 360, 30 deg/s, 700 rds |
| Speed / turn / bog | 750 cm/s / 26 deg/s / 15 % | 850 / 24 / 15 % |
| Exits | rear double door 0.8 s, over sides 0.5 s | rear door 0.8 s, over sides 0.5 s |
| Crew squad, hatches | `ger_tank_crew`, one hatch per side (open top) | `usa_tank_crew`, as the 251 |
| Components | engine, track_left/right (rear tracks), transmission | as the 251 |

Cost (S8a-11): vehicles.md 8.4's 300 scaled like S7-3 (x 620/640 and x 590/560). Sources for the TOML comments
(verify each title): Jentz and Doyle, *Panzer Tracts* 15 (mittlerer Schützenpanzerwagen); Spielberger,
*Halftracked Vehicles of the German Army 1909-1945*; TM 9-710 (1944) *Basic Half-Track Vehicles*; Hunnicutt,
*Half-Track* (2001); Zaloga, *M3 Infantry Half-Track 1940-73* (Osprey New Vanguard 11). The 251's rear AA MG and
the M3A1's .30 pintle sockets are left out (S8a-10). S7-1: StuG roof MG and M10 .50 stay out (they change S7
balance and need exposed loader or commander boxes; data-only in S8b).

## 7. AI (`ai/transport.rs`, `[ai.transport]`)

Bought through the `vehicle` mix slot by card `ai_weight` (Build A: 0; Build B: 100, the tanks'). Per transport
one saved job `{ vehicle, squad, phase, goal, fails, since }`; phases:

1. **Load** at base: the newest idle line, engineer or MG squad in base whose objective is farther than
   `min_ride_m` (150) and that fits; `Embark`. Waits at most `wait_s` (30), then goes alone to Support.
2. **Ride** toward the squad's assault point. Every `check_s` (1) it tests the path ahead: the first point
   within `dismount_sight_m` (300) of a known enemy with a clear line (at most `los_rays` 8 rays per check);
   dismount `dismount_back_m` (40) before it. Immediate dismount when the vehicle is hit (any `Penetrated`,
   `Stopped` within 2 s counts), a rider is suppressed past `crouch_suppression`, or the vehicle stops for
   `stuck_s` (10).
3. **Unload** `Disembark { over_sides: under fire }`; the squad gets the normal assault order to its point.
4. **Support**: overwatch spot of `ai/overwatch.rs` (hull-down or stand-off `standoff_m` 150 from the point,
   outside the AT stand-off); the MG fights from there for `support_s` (60).
5. **Return** to base when another squad waits, else stay in Support.

**No loops.** A move that ends `Idle` (the S7 `follow()` fix ends unreachable goals) farther than `arrive_m`
(15) from its goal adds a fail; at `max_fails` (2) the job uses where the vehicle stands (Unload there, or
Support there). No job issues the same move twice in `repath_s` (5).

## 8. Presenter

Blockouts from the boxes (S7 `vehicle_types` path): cab, open compartment, MG and its shield turning with the
pintle yaw. Riders drawn seated at their slot (`UnitSnapshot.riding`); a vehicle card lists its passengers
(squad name, men, HP pips); a squad card shows "Mounted (vehicle)". Embark tool **V** (click an own transport),
Disembark **U**, Shift+U over the sides (S8a-7); buttons in `orders_grid.gd`. Feed coalesces small-arms
penetrations. `match_state.md` "S8a". Autoplay scene `halftracks`: a loaded 251 and an M3A1, screenshot to
`scratchpad/s8a/`.

## 9. Save, hash, protocol

`SAVE_VERSION` 17 -> 18 once (Build A, with `MgState.yaw`; Build B adds the riding fields under 18 before
anything leaves the branch). New fields: `MgState.yaw`; `Unit.riding` (manual `StableHash` in `hash.rs` 135);
`VehicleState.{riders, door_ticks, unload}` (postcard hash); `Order::Embark`; the AI transport jobs; per-player
`S8aCounters`. Load validation: every `riders[k]` names a live own soldier whose `riding` names this vehicle and
slot, and the reverse; `slot < capacity`; `door_ticks <= max(board, exit)`; `unload` only on a transport; `yaw`
within the arc. `PROTOCOL_VERSION` 9 -> 10 once (`Embark`, `Disembark`); `sdk/docs/net/relay-protocol.md`
updated.

## 10. Tests

- **armour-test 21** (Build A): MG 42 vs M3A1 side, .50 vs 251 side at 50 / 150 / 300 m (30 shots each, %
  penetrating); rifle vs both fronts (0 %); also MG 42 aimed at the M10's exposed loader hits him (S7-7).
- **22** (B): a loaded 251, one bazooka rocket at the side at 100 m: % of runs with a rider hit, mean riders
  lost.
- **23** (B): stick grenade at a loaded 251 from 15 m, stopped vs 15 km/h: stopped share of riders lost (most),
  moving inside % (mostly misses).
- **24** (B): 10 riders disembark under an MG 42 at 200 m, door vs sides: exit time, men hit; `sides_faster` =
  1.
- **Unit** (sim): pen table interpolation; reach table; shield pass/stop; thin-plate fragments (plate 6.35 FH
  pierced near a mortar burst, 14.5 not); pintle stays in arc and turns at its rate; half-track bog roll at
  `bog_pct`; main-gun-less vehicle never lays; Embark refused `NoRoom`; boarding cadence; exits door vs sides;
  knock-out dismount with stun; riders skip capture; a move order to a mounted squad is ignored; AI transport
  job with an unreachable goal ends after 2 fails (no re-path every tick).
- **Save/hash per field**: `MgState.yaw`, `Unit.riding`, `riders`, `door_ticks`, `unload`, `Order::Embark`, AI
  jobs, S8a counters (change one, hash differs; round-trip equal; bad values rejected on load).
- **Determinism**: 1/2/3/8 threads equal hashes with a save/load/continue while embarked; net loopback with
  Embark/Disembark; soak with half-tracks.
- **Port proof (Build A)**: golden ballistics and armour-test cases 1-18 equal; skirmish numbers move (S7-7
  aimed fire at M10 crews, thin-plate rule) and are re-measured, not compared.

## 11. Targets (S8a.toml T40-T49, plus all S7 targets)

T40 half-tracks bought in >= 60 % of matches; T41 >= 1 squad transported per match and the time saved per ride
(report); T42 each half-track <= 10 % of its side's kills, S7-15 recheck out of sample; T43 riders lost,
small-arms pens, dismounts under fire (reports); T44 case 21 (MG 42 vs M3A1 side at 50 m >= 80 %, .50 vs 251
side >= 80 % at all three ranges, rifles 0); T45 rocket hits a rider in >= 50 %; T46 stopped >= 50 % of riders
lost, moving <= 20 % inside; T47 sides faster; T48 >= 1 trip per half-track bought (no loops). Balance: Germany
45-55 % pooled over 480, HE 20-26.3 %, mortar 10-15 %, T32 18 % (recheck S7-15 with `td_hunt_m` in the Tune).
JSON fields: S8a.toml header.

## 12. Build order

**Build A** (6.4, thin plates, pintle, shields, mobility, data, save):
1. Content: main gun optional, `Pintle`, `Transport`, `Halftrack`, `FaceHardened`, `[[shields]]`, `[transport]`
   + slots parsed and validated (unused by the sim), weapon `penetration`/`core`, HE `frag_pen_mm`, rules keys;
   schema regen; validate.
2. Sim: optional main gun (port proof), small-arms hit path and core, reach table and vehicle targets,
   thin-plate fragments, shields, pintle (`MgState.yaw`), halftrack kind, transport unlock/limit; SAVE 18.
3. Data: both vehicles, cards (`ai_weight 0`), `m2hb`, `50bmg_belt`, pen tables, `frag_pen_mm`; presenter
   blockout only (box path).
4. armour-test 21; unit and save tests; `check.sh quick`; 40-match run.

**Build B** (transport, commands, AI, presenter, balance):
5. Riding state, Embark/Disembark, exits, knock-out dismount, PROTOCOL 10, relay doc, riders in damage,
   spotting, capture.
6. armour-test 22-24; save/hash/determinism/loopback tests.
7. `ai/transport.rs`; `ai_weight` 100; JSON `s8a` fields.
8. Presenter: riders seated, cards, buttons, hotkeys, screenshot.
9. Balance: targets on 40 + 160 x 3 + armour5; timing40 alone at the end.

**Tune** (Sonnet): `ai_weight`, `[ai.transport]`, `td_hunt_m` (S7-15).

## 13. What S8b will need

`Wheeled` with a bicycle `drive` (turn radius, no pivot), a wheeled block layer (trench >= 1.0 m), wheel parts;
`fire_from`; recon optics (6.9); truck cargo kg and supply (section 7) with `[transport]` for men; M20 ring =
`pintle`; 222/234 open turrets with `grenade_screen`; StuG roof MG and M10 .50 as pintle data (S7-1).

## 14. Owner questions (each with its (A); work proceeds on (A))

- S8a-1 Half-tracks have no main gun: main gun becomes optional. (A) Yes.
- S8a-2 M3A1 capacity 11 vs the 12-man US rifle squad: squads never split, one that does not fit is refused. (A)
  Yes (the 12-man squad rides after a loss). (B) capacity 12 (the armored-infantry driver came from the squad).
- S8a-3 MG pen tables use the AP round for every round. (A) Yes; an `ap_share_pct` roll if T42/T43c run high.
- S8a-4 Face-hardened plate x 125 % against every round. (A) Yes, verify.
- S8a-5 Transports skip the vehicle limit of 4 and unlock at 2:00; cap 2 each. (A) Yes.
- S8a-6 Half-tracks steer like tanks (neutral pivot) until S8b's wheeled model. (A) Yes.
- S8a-7 Hotkeys Embark V, Disembark U, Shift+U over the sides. (A) Yes.
- S8a-8 `Disembark` carries `over_sides`; door is the default. (A) Yes.
- S8a-9 Riders are targets only while their slot box is above the rim and the vehicle is seen (no rays). (A) Yes.
- S8a-10 The 251 commander fires the front MG; no rear AA MG; no M3A1 .30s. (A) Yes.
- S8a-11 Cards 290 / 310 MP, pop 3, 60 s, cap 2. (A) Yes; Tune +-15 %.
- S8a-12 The feed coalesces small-arms penetration events (sim emits all). (A) Yes.
- S8a-13 Thin-plate fragment values (75 mm 12, mortars 9, grenades 3). (A) Starting values, verify.
- S8a-14 Cross-country speeds 750 / 850 cm/s (road 52 / 72 km/h). (A) Starting values, verify.
- S8a-15 Half-track crews bail as tank crews (`*_tank_crew`). (A) Yes.
