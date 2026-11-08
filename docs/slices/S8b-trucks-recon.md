# S8b: trucks, recon cars, wheels

Owner 2026-11-08: "run another session of development". Roadmap S8 was split for the weekly limit; S8a is on main.
Branch `s8b-trucks-recon` from `0dc13dd`. Design: `docs/design/roster/vehicles.md` 4-10. Targets: `docs/slices/targets/S8b.toml`. Owner questions: section 16.

## 1. Baseline (verified S8a JSON, `scratchpad/verify-s8a`, no rerun)

| | 1-80 | 101-180 | 201-280 | pooled 480 |
|---|---|---|---|---|
| Germany wins % | 42.5 | 45.6 | 48.1 | 45.4 |
| HE share % | 21.0 | 20.3 | 20.2 | |
| obj_saved_s_per_ride GER / USA | -29.4 / -14.4 | -26.3 / -15.4 | -21.0 / -5.9 | |
| obj ride s / straight walk s, GER | 180.7 / 151.3 | 170.3 / 144.1 | 168.5 / 147.5 | |
| obj_rides reached / lost GER | 80 / 76 | 110 / 72 | 113 / 84 | |
| obj_rides reached / lost USA | 68 / 45 | 43 / 61 | 71 / 58 | |
| transported per match USA | 0.975 | 0.894 | 1.044 | |
| trips per bought USA | 0.857 | 0.777 | 0.865 | |

- Tick (timing40, clean): p99 0.488 all, 0.621 peak; a rise above 25 % gets profiled. Top kills share: Pz IV 17.9 / 20.0 / 18.6 %.
- The ride itself is 64 s; order to arrival 170-180 s: 110 s go in boarding, early dismounts and the walked last stretch.
- AI tanks fire 6.4-6.9 main-gun rounds per tank bought and start 0-4 rearm trips in 160 matches; forward rearm is 0.5-0.66
  rounds per match. Trucks have little tank demand today (S8b-14), so crates for mortars and infantry are their measurable work
  (6.5 shell crates issued by hand per match, 40 bombs from crates).

## 2. Code map (what changes where)

| Area | Where | Change |
|---|---|---|
| Component kinds | content `vehicle.rs:154-207`, `:3361-3380`, `:1696-1708` | `WheelsFront`, `WheelsRear`, `Cargo`; `COMPONENTS` 10; `is_running_gear()` for `is_track()`; kind wanted per mobility/role |
| Mobility schema | content `vehicle.rs:331-337, 545-562, 2344-2358, 2858, 3171` | `Wheeled`; `turn_radius_cm`, `woods_speed_pct`, `slope_loss_pct` |
| Role schema | content `vehicle.rs:307-323`, `:1385-1412`, `:1947-1960`, `:3185-3190`, `:3427` | `Recon`, `Truck`; zero weapons on a truck; `[supply]`, `[optics]`, `grenade_screen` |
| Absent parts | sim `vehicle.rs:430-438`, `bail.rs:186-200`, `ai/armour.rs:244-250` | `immobilised` only tests parts the def has (a wheeled car has no track, its hp 0 would read "broken") |
| No mounts | sim `vehicle.rs:409-417`, `spotting.rs:285`, `snapshot.rs:980` | `gun_out` false with no mounts (`.all()` on empty is true: a truck would bail and retreat at once, `bail.rs:389`, `ai.rs:1784`); `primary_weapon()` is index 0: gate the reveal |
| Drive, follow | sim `vehicle.rs:1496-1628`, `:2075-2240`, `:660-668` | Bicycle branch, no pivot (`:2228`) or `face` pivot (`:2086`); reverse out of the turning circle; `track_y` from the first running-gear part |
| Block layer | sim `vehicle.rs:1137-1190, 1293-1305, 1382, 1404, 1630-1634` | Second instance `wheel_block`; **`ditch_dm_of` ignores wheeled defs** (else a 10 dm wheeled def would make every AT ditch dug 1 m a wall for tanks) |
| Path | sim `game.rs:1719, 1767, 1844`, `path.rs:85, 245, 263, 368, 512`, `vehicle.rs:2178` | The layer picked from the squad's vehicle type |
| Damage | sim `damage.rs:571-584, 891-905, 986`, `combat.rs:2790-2800` | Wheels: part from the hit's x; the blast rule on any running gear; the rack rule for `Cargo` |
| Optics | sim `spotting.rs:119`, `vehicle.rs` state | `still_ticks`, one multiplier on the observer's sight |
| Screen | sim `items.rs:420-530, 853-880`, `ai/overwatch.rs:173` | A screened face turns thrown items away; aim and AI skip it |
| Supply | sim `supply.rs:839-930`, `structures.rs:478, 977-1025, 1040-1250, 1257-1345` | `Source::Truck`, `rearm_vehicle` shared; trucks in `serve_men`, never in `feed_mortars` |
| Crate drop | sim `command.rs:179-200`, `game.rs:1103`, `structures.rs:619-733`, net `protocol.rs:584`, `harness.rs:360` | `DropCrates` |
| Limit, AI buy | sim `game.rs:930-938, 958-965, 980`, `vehicle.rs:1731`, `ai.rs:737-743, 1121-1127, 1205-1262, 1408` | `outside_limit()`: Transport, Recon, Truck; both out of the mix, own buys |
| AI rides | sim `ai/transport.rs:177-276, 331-482, 542-566`, `ai.rs:1004-1006` | Meet phase, ride lock, reason counters |
| AI vehicles | sim `ai/overwatch.rs:58-130`, `ai/armour.rs:65-160`, new `ai/recon.rs`, `ai/truck.rs` | Recon, trucks, tanks to the nearest stock |
| CLI | `armour_test.rs`, `skirmish.rs`, `json_out.rs:387` | Cases 25-32, `s8b`, `s8b_derived` |
| Bridge, HUD | `match_node.rs:112, 2531, 2561, 1897, 2334`, `meshgen.rs:256`, `godot/match/hud/{call_in_panel,detail_panel,roster,orders_grid}.gd`, `vehicles.gd:28-31`, `supply_view.gd`, `vehicle_view.gd`, `match_input.gd` | Section 10 |

## 3. Schema additions (every value in `mods/ww2`, unit + meaning comment)

```toml
# vehicle def
role = "truck"                  # tank | assault_gun | tank_destroyer | transport | recon | truck
sight_m = 240
spot_size_pct = 130             # section 6.9: 120-150 for cars
[mobility]
type = "wheeled"                # tracked | halftrack | wheeled
speed_cm_s = 1000               # cross-country practical speed (no roads on our maps; road_speed reserved)
reverse_cm_s = 1000             # 234: second driving position; M8 280
turn_radius_cm = 700            # bicycle model; required with wheeled; turn_deg_s not used
woods_speed_pct = 30            # % of speed in woods (default 50, today's constant)
slope_loss_pct = 85             # % lost at max_slope_pct (default 70, today's constant)
max_slope_pct = 45
ditch_block_dm = 10             # wheeled: any ground dug this deep blocks (a trench is a wall)
bog_pct = 12
unbog_s = [20, 40]
[optics]                        # optional; default 100 %, no effect
settle_s = 4                    # s stopped
stationary_sight_pct = 140      # % of sight_m after that
[[boxes]]
open = ["top"]
grenade_screen = ["top"]        # wire screen on these open faces: stops thrown items, not bullets or fragments
[supply]                        # exactly on role = "truck"
cap_kg_total = 2500             # kg per kind (7.3)
load_kg = { shells = 320, small_arms = 120, explosives = 40 }   # what it takes on while stopped in base (S8b-3)
radius_m = 8                    # men
vehicle_radius_m = 15           # vehicles
round_interval_s = 4            # = rules fwd_rearm_s; rare rounds at rare_rearm_fwd_s
crates = ["ww2:usa_shells_crate", "ww2:usa_small_arms_crate", "ww2:usa_explosives_crate"]
cookoff = "ww2:75mm_m48"        # bursts on detonation, S6 CookOff
[[components]]
type = "cargo"
hp = 60
box = { frame = "hull", center = [-150, 0, 120], half = [180, 110, 60] }
```

Rules additions in `skirmish.toml` (`[ai.transport]` adds in section 4; `[ai.recon]`, `[ai.truck]` in section 9):

```toml
[vehicles.driving]
wheeled_waypoint_reach_m = 6    # m: a wheeled path node counts as passed
wheeled_arrive_mm = 2500        # mm: a wheeled vehicle has arrived
wheeled_creep_pct = 30          # % throttle while it steers on a large heading error
[supply]
fwd_mg_rounds_s = 50            # MG rounds per s forward (7.3 row "50 rounds per s"; S6 left it out)
truck_load_kg_s = 8             # kg/s of small arms and explosives into a truck (shells: base_crate_issue_s per 32 kg)
drop_s = 3                      # s per crate off the truck
drop_reach_m = 8                # m: the truck is at the point
cargo_detonate_min_kg = 40      # kg shells + explosives: below this cargo only burns
```

Validation: wheeled needs `turn_radius_cm` (300..=2000) and anchors-only `wheels_front`, `wheels_rear`, no tracks; tracked and halftrack the opposite. `cargo` and `[supply]` exactly on trucks.
`grenade_screen` only on open faces. A truck may have no weapons. A crate def for each kind it loads. Trucks stay out of `[transport]` (S8b-5).

## 4. Rides that pay (S8a-28, S8a-29, S8a-31): first in Build C

1. **Instrument first.** Add to `AiCounters`: `unload_exposed`, `unload_arrived`, `unload_stuck`, `unload_fire`, `obj_board_ticks`
   (order to last man aboard), `obj_foot_ticks` (unload to the radius). One 40-match run shows where the 110 s go; tune against it.
2. **Do not retask a riding squad.** Half the measured rides never reach their point. While a squad rides and for `ride_lock_s`
   (start 40 s) after it gets off, it stays in `claimed` (`ai.rs:1004`) and its Capture task stays. Enemy contact still releases it.
3. **The half-track goes to the squad.** New phase `Meet` (saved, `TransportPhase = 5`): a pick within `pick_m` (start 250, was
   `PICK_M` 120) drives to `lead + toward point * meet_ahead_m` (start 30 m), never behind the squad; `Embark` follows when it stands
   within 20 m (the squad keeps walking to its point meanwhile). `transport.rs:715` already re-paths the walk to a moved door.
   `Return` goes to the nearest waiting squad, the base only if none. No-loop rule: `max_fails`, `repath_s`, as `drive()`.
4. **Shorter waits.** `wait_s` 20 to 10; walking allowance from the squad's speed, not `WALK_MM_S`.
5. **Shorter `dismount_sight_m`.** 300 to 150 start (rifle fire range); `los_rays` stay. Tune by the `unload_exposed` share.

Target: T50 above 0 on the six set-faction pairs, T41b and T48b at least 1. Recheck S8a-31 (case 24). A saved time bought by a walked last stretch under rifles shows in T43a: report both.

## 5. Wheeled drive and block layer (6.8)

- **Drive.** `drive` branches on `m.kind == Wheeled`: `turn = speed.div(radius) * steer / 127 / hz`; negative speed turns the other
  way by itself. A refused step stands still (no turn-only fallback). `pct` reads `woods_speed_pct` and `slope_loss_pct`.
- **Follow.** No neutral pivot: at a large error the car creeps at `wheeled_creep_pct` and steers full. A waypoint inside the
  turning circle (`|to - c| < R` for the circle on the side it lies) makes it reverse with the opposite lock until it is outside
  (no new state: geometry each tick). `vs.face` is cleared. `waypoint_reach` and `arrive` from `wheeled_*`. Stuck, back-off and partial-
  path rules stay (S7 fix: a goal it cannot reach ends like an arrival).
- **Layer.** `wheel_block` is a second `BlockLayer` (`any_dug`): raw = (dug >= its dm anywhere) or wreck. `dug_dm` is the deepest in the
  cell, so a 1.6 m trench is caught; `ditch_dm_of` is taken from non-wheeled defs only (code map). **Blocked is the raw nodes, not dilated**
  (Build A; the plan's dilated A* closed a 10 m gap), and the dilation is its margin: a car is much narrower than a 4 m grid cell, and a
  truck parked by a trench an engineer digs after it stopped must not read `on_blocked` (S8b-16). Test: such a car drives away.
- **Wheels.** `WheelsFront`, `WheelsRear` are anchors-only, like tracks: band hit picks front or rear by `hit.local.x` (box frame),
  `blast_tracks` and the fragment band use `is_running_gear`; either at 0 immobilises.

## 6. Optics and grenade screen

- **Optics (6.9 lean).** `VehicleState::still_ticks: u16` (saturating), zeroed by a tick with `speed != 0`. At `spotting.rs:119`:
  `sight * stationary_sight_pct / 100` once `still_ticks >= settle_ticks`. Default 100 % leaves tanks unchanged (S8b-10).
- **Not now:** `SetHatch` needs a per-seat exposed box (`[crew.unbuttoned]`) and a command; radio observers need a recon `observer`
  trait in `fire_mission` (`observe_observer_m`) and a link rule. Both have the same state hook (`still_ticks` and sight multiplier).
- **Screen (6.5).** `BoxSetup::screen` bitmask beside `open`. `items.rs:504`: a thrown item reaching a screened open face bounces and
  falls beside the vehicle (at the box edge, outward along the face normal, then the normal ground rule). Bullets and fragments
  ignore the bit. Aim at `items.rs:853` and `open_top_throw` skip screened boxes.

## 7. Supply vehicles and the one-source-function plan (section 7)

**State.** `VehicleState::cargo_g: [u32; 3]` by `SupplyKind::index`, `drop: Option<CrateDrop>`. The truck drives with an ordinary `Order::Move` (Build B: no `Order::DropCrates` / `OrderKind` 10; any other order clears `drop`).

**Loading at base.** Stopped in own base (`base_ticks` > 0): shells 32 kg per `base_crate_issue_s` **on the player's crate queue** (the
same timer as `IssueCrate`, so a truck competes with crates, 7.3); small arms and explosives `truck_load_kg_s`. Up to `load_kg`, never
above `cap_kg_total`. Each kg is paid as it loads through `self.pay` at the kind's price (D1, no surcharge D2); no payment, no load.
Nothing else: no draw from an HQ (S8b-20). Cost of a full default load: 320 + 24 + 20 = 364 MP (paid in steps, the AI loads less).

**One function, three sources.** Today two copies: `rearm_vehicles` (base, paid, due bits) and `rearm_forward` (draws, credits).

`Source { Base, Structure(u32), Truck(SimId), Crate(u32) }`; `source_for(..., reach: Reach)`; `pick_round(vs, v, rare)` (both callers); `rearm_vehicle(id, src, step)` the only code that fills a rack; `rearm_belts(id, src, rounds)` the only code that fills MG stowage.

- `source_for` returns the **fastest in reach**: least interval, ties structures, then trucks, then crates, ascending id. Base always
  wins in base (2 s). The timer stays stateless (`(tick + id) % every` with the chosen source's interval): one source, one draw per due tick, so
  **rates never stack**, by construction. Case 29 proves it with an HQ and two trucks.
- `Reach`: `{ crates: Fx, trucks: Option<VehicleRadius | MenRadius> }`. Vehicles pass `vehicle_radius_m`; `feed_mortars` passes no truck
  reach (`crate_only_if_indirect`: bombs from a truck only through a dropped crate, S8b-4).
- `draw` handles `Source::Truck`; the same `rack_credit` (D4). `serve_men` lists trucks after structures (`radius_m`), before crates.
- MG belts forward: `fwd_mg_rounds_s` from any stock (S6 left the 7.3 row out; S8b-6). `rearm_vehicles` keeps its due-bit payment logic for base.
- Never `mounts[0]`: `main_ammo()`, `mg_mounts()`, a truck with neither is skipped.
- A derived `truck_ids` (ascending, rebuilt with the vehicle index) keeps `source_for` cheap.

**`DropCrates { vehicle, point, count, kind }`** (PROTOCOL 11; `kind` beyond the design's form, S8b-2: S6 made three defs). Valid: own
live truck, `1..=MAX_DROP` (8), the kind's crate def exists, the point on the map. The truck drives to `point` (ordinary vehicle path; a
goal it cannot reach ends like an arrival), stops within `drop_reach_m`, and drops one crate per `drop_s` beside it while `cargo_g[kind] >=
crate kg` and `crate_cap` / `shell_crate_cap` leave room. A dropped crate is a normal `Crate` (S6: stocks in, feeds, cooks off). Grams move
from cargo to crate, nothing paid. Refusals through the S6 `SupplyRefusal` list.

**Cargo.** `Cargo` component with a box; `deal` treats it as `AmmoRack` (rack_min_damage, detonate %, fire) while it holds at least
`cargo_detonate_min_kg` of shells + explosives (S8b-9); below that it burns (`ammo_fire`). A detonation is `KO_RACK`, cargo lost
(`truck_lost_g`, own counters so the T22 stock balance stays closed), and bursts via the S6 `CookOff` with `cookoff_kg_per_burst` (the physical-contact rule: real bursts, real fragments). A
cargo box destroyed always starts an ammo fire while it holds anything; the fire ends in `KO_AMMO_FIRE` and detonates at the same kg rule (Build B: so a penetration detonates a loaded truck about 70 % of the time; case 28 counts both paths). A truck on fire serves nobody. A
wreck's cargo is lost. Trucks are unarmoured (1-2 mm plates), so the S8a small-arms core goes through them into crew and engine.

## 8. Content (figures marked verify are from memory; each def has `sources`)

| Id | Role | Armour front / side | Armament | Speed cm/s | Crew | Cost / pop / cd / cap / unlock |
|---|---|---|---|---|---|---|
| `ger_sdkfz234_1` | recon, 8x8 | 30 at 35 / 8-10 | 2 cm KwK 38 + MG 42, open turret, `grenade_screen` | 1000, rev 1000 | 4 | 280 / 4 / 60 / 2 / 90 s |
| `usa_m8` | recon, 6x6 | 19 turret, 16-19 hull / 10; floor 6 | 37 mm M6 + .30 coax + .50 ring | 1000, rev 280 | 4 | 290 / 4 / 60 / 2 / 90 s |
| `ger_opel_blitz` | truck | 1-2 sheet (verify) | none | 800 | 2 | 170 / 2 / 45 / 2 / 0 s |
| `usa_cckw` | truck | 1-2 sheet (verify) | none (S8b-7) | 800 | 2 | 180 / 2 / 45 / 2 / 0 s |

German prices follow the S5a-37 cut (S8a formula); within 8.4's 260-300 and 150-200. Cards: tags `["vehicle", "recon"]` or `["vehicle", "truck"]`,
`ai_weight 0` in Builds A and B. Turret traverse by hand (verify); the 234's and M8's spot size 130 and 140.

| Round | Mechanism | m/s | Penetration | Source / flag | supply g |
|---|---|---|---|---|---|
| 2 cm PzGr 39 | ap | 780 | about 23 at 100, 14 at 500, plate at 30 deg | verify (memory) | 320 |
| 2 cm PzGr 40 | apcr, rare | 1050 | about 40 at 100, 25 at 500, at 30 deg | vehicles.md 4.5, verify | 280 |
| 2 cm Sprgr | he | 830 | small burst; starting values | verify | 320 |
| 37 mm M51 | ap | 884 | about 61 at 100, 53 at 500, 46 at 1000, at 30 deg | vehicles.md 4.5, verify | 1590 |
| 37 mm M63 | he | 792 | 39 g filler (verify) | verify | 1590 |

- Guns: `kwk38_2cm` (120 rounds a minute practical; a one-shot model with `reload_ms` 600 averaging the 10-round magazine, S8b-12); `m6_37mm`
  (`reload_ms` 2500 verify). Tables through existing `penetration` and `pen_reference_angle_deg = 30`; no new mechanism.
- Stowage (verify): 234/1 480 x 2 cm (AP 200, HE 270, PzGr 40 10), MG 42 2400; M8 80 x 37 mm (AP 50, HE 30), .30 1500, .50 400. M8 .50: `pintle`, 360 deg, hull frame (S8b-18), commander's seat.

## 9. AI

- **Budget.** `mix_slot` returns `None` for recon and truck cards (the `crew` precedent), so they never take a tank's mix buy (S8a's displacement).
  Each has its own buy: `[ai.recon] income_max_pct` (start 8) and `[ai.truck] income_max_pct` (start 5) of income so far, the
  S6 `shell_mp_max_pct` pattern; manpower left for the unit mix first. `is_transport_type` becomes `outside_limit`.
- **Recon (`ai/recon.rs`, one saved job).** From `after_s` (start 90), one car; a second after `second_after_s` (start 480) if the first lives. It takes
  the point the infantry goes for (first non-own by priority), goes to `view_back_m` (start 150) short of it on the own side, through the S7
  `overwatch_spot` (hull-down, stand-off, `SPOT_GIVE_UP_S`), stops for `settle_s`, and stays while nothing threatens. Backs off (ReverseUnits) at
  `at_threat_m` (start 200) from known AT teams, tanks, TDs, and from infantry within `open_top_infantry_m` when open-topped (the S7 block). Failed moves as `ai_fails`
  (`max_fails`, `repath_s`): takes where it stands. Screened cars still back off from infantry (bullets go through screens).
- **Truck (`ai/truck.rs`, one saved job; phases Load, Follow, Serve, Drop, Return).**
  - Buy when 2 or more vehicles (not counting transports) are alive and the HQ exists or `hq_wait_s` passed; one truck.
  - Load: stop in base until `load_kg` reached or `load_wait_s` (start 120), at least `min_load_pct` (start 40); `[ai.truck] load_shells_kg` (start 160) under the
    `shell_mp_max_pct` budget; over budget it waits for `tank_rearm_floor_pct` work only.
  - Follow: `supply_follow_m` (start 150) behind the armour's centre toward base; candidates on a ring, tested by LOS to known enemies (`los_rays`); never in sight if another spot exists.
  - Serve (first priority): **a stranded tank** (immobile, or at the rearm trigger and farther than `tank_crate_far_m` from every stock; `ai/supply.rs` "cannot reach"): go to within `vehicle_radius_m - 3` m, wait until
    it is rearmed, the stock is below `return_below_pct`, or a threat within `flee_m`. Counter `stranded_served`.
  - Drop: dry mortars, HQ below `crate_run_below_pct`, then line squads in trenches low on ammunition (`small_arms`). The bearers' run logic keeps working; a client served by a truck is not also sent a crate.
  - Return when `cargo` under `return_below_pct` (start 20) of load, else Follow. Flee: `flee_m` (start 120) from any seen enemy; a job that fails `max_fails` holds.
- **Tanks (7.5).** `rearm()` adds trucks with stock for 10 rounds or more to `fwd`, nearest first; `at_fwd` with `vehicle_radius_m`. Leaves when standard rounds are full. Stall: no round for `rearm_stall_s` leaves the source until it has stock (saved tick).
  **Float:** `[ai.ammo] ammo_float_mp` (start 100) held from unit buys while a unit waits to rearm. Push-limit reserve (`push_without_supply_m`) is left out (S8b-13).
- Half-tracks change only through section 4.

## 10. Presenter

- **Blockouts** (`meshgen.rs`, `vehicle_view.gd`): cars as the tank layout with open turret (M10 code) plus 8 or 6 wheels (a new `wheels` part, one cylinder multimesh, `VehicleDims::wheels`); 234 screen as a wire frame over the
  turret. Trucks through the `transport` path (`tboxes`): cab, bed with `cargo` box visible.
- **Cards and tiles.** Call-in tiles for the four cards (S4 card list). Fix (1): `is_vehicle(c)` badge, `blocked_reason` and tooltip read a per-card `own_cap` for vehicles outside the limit (`alive/max_alive`, not `0/4`) in `call_in_panel.gd:126, 162, 296, 299, 454`, sent by the bridge beside `max_alive`.
- **Fix (2), S8a-27:** the bridge sets `components` only for parts the def has (`match_node.rs:2531`); `detail_panel.gd:194` and `roster.gd:567` iterate what is present.
- **Fix (3), "MG dry" with 2010 aboard.** Static read found no fault in `snapshot.rs:1035`, `match_node.rs:2561`, `roster.gd:396`. Build C starts with a bridge test asserting `ammo.mg == 2010` for a fresh 251, then prints in the card; do not patch blind.
- **Fix (4):** the squad card's red hatched ammo bar on those vehicles: same trace; `ammo_pct` must be absent or computed from what the vehicle has (`squad_ammo_pct` returns `None` for vehicle squads, so the fault is elsewhere; `detail_panel.gd:304`).
- **Drop Crates**: order row for trucks, hotkey `I` (`order_drop_crates`; V, U, Shift+U, M H R Y B L G T F X C Z O N J P K taken), click a point; default kind = the fullest stock share, cycle on a held key; bar text `count` 4.
- **Stock bars and radius ring** reuse the HQ ones (`supply_view.gd`, `detail_panel.gd:487`); `Vehicles.COMPONENTS` gains the three parts. `match_state.md` "S8b"; one screenshot of a 234/1, an M8 and a truck into `scratchpad/s8b`, looked at.

## 11. Save and protocol

`SAVE_VERSION` 19 and `PROTOCOL_VERSION` 11, once each, in the first commit that changes a layout. `sdk/docs/net/relay-protocol.md` gets `DropCrates`.

| State | Type | Checks on load |
|---|---|---|
| `VehicleState::components` | `[u16; 10]` | as today, per kind the def has |
| `still_ticks` | `u16` | any |
| `cargo_g` | `[u32; 3]` | 0 unless truck, each <= cap kg |
| `drop` | `Option<CrateDrop { point, count, kind, next_tick }>` | only on a truck, count in range, point on the map |
| `AiTransport` phase `Meet`, fields `point`, `lock_until` | | phase in range |
| `AiRecon`, `AiTruck` maps | `BTreeMap<SquadId, _>` bounded by `MAX_RECON`, `MAX_TRUCKS` | keys alive at use |
| `Order::DropCrates`; `AiCounters`, `S6Account` new counters | | on a truck squad; report only |

One test per field: save, load, hash equal; mutate the field, hash differs. Ascending iteration everywhere; RNG only through the per-purpose counter keys (bog, as today).

## 12. Tests

**armour-test (runs through `armour5`, `T60-T68`):**

| Case | What | Checks |
|---|---|---|
| 25 | 234 ordered across a closed trench line; again with a 10 m gap; a tank at the same line | `crossings_closed_line`, `reached_through_gap_pct`, `tank_crosses_same_line_pct` |
| 26 | Bicycle: turn circle, no pivot, reverse, blast under a wheel | `no_neutral_pivot`, `turn_radius_within_pct`, `reverse_speed_234_cm_s`, `wheel_blast_disables_pct` |
| 27 | Tank at 10 % at 15 m from a truck | `seconds_per_round`, `stock_error_g`, `no_mp_charged` |
| 28 | Round through a loaded and an empty truck | `detonated_loaded_pct`, `detonated_empty_pct` |
| 29 | Tank between an HQ and two trucks | `seconds_per_round`, `sources_drawn` |
| 30 | Grenade at the screened 234/1 turret, at the M8, bullets through screen | `screened_inside_pct`, `screened_crew_lost`, `landed_beside_pct`, `unscreened_m8_inside_pct`, `bullets_pass_screen` |
| 31 | MG 42 at a truck, 2 cm and 37 mm against plates, rifle at the M8, stopped sight | `mg42_truck_pen_pct`, `mg42_stops_truck_s`, `pen_2cm_m3a1_front_100_pct`, `pen_37mm_pz4_front_pct`, `rifle_m8_front_pen_pct`, `stationary_sight_gain_pct` |
| 32 | AI truck serves a stranded tank (scripted start, planner on) | `stranded_served_pct`, `rounds_gained_mean` |

If the armour-test harness cannot host the AI planner, case 32 becomes a cargo test and T68 a report; say so in the log.

**cargo:** per-field save/load/hash; `ditch_dm_of` ignores wheeled; truck by a new trench drives away; a truck never rearms in base; truck load pays per kg and pauses without MP; dropped crate grams; `gun_out` false for no mounts; a wheeled def's missing tracks do not immobilise; `outside_limit` call-in; Reach never gives a mortar a truck's bomb; determinism at 1/2/3/8 threads with save/load/continue while a truck serves and a car reverses (seed scan, not a weakened assertion, if a match plays differently); net loopback with `DropCrates`; golden and validate.

## 13. Targets

T50-T72 in `S8b.toml`. Hard: rides above 0 (T50, six pairs), T41b, T48b; recon and trucks bought in 60 % or more of matches (T52, T53); tanks keep their mix (T54); recon at most 10 % of kills and 8 % of manpower, trucks 5 % (T55, T56); crates dropped per match (T57a/b); armour cases (T60-T68); `ai_move_fails_per_match` (T71). Reports: truck tank rounds, stranded per match (low demand, S8b-14), cargo detonations. Balance: T5c/d/e per set, judged pooled (T49); T32 18 %.

New JSON: `factions.<f>.s8b` and `s8b_derived`, list in the `S8b.toml` header. The Plan baseline: 134 pass, 73 miss of 207 (the misses are the new fields, plus T5c, T41b, T48b, T50a-f, as expected).

## 14. Build order

**Build A: wheeled mobility and the cars (no AI).** Schema (roles, wheeled, optics, screen); `COMPONENTS` 10 and absent parts; drive, follow, wheel layer, path; wheel damage; optics; screen; 2 cm and 37 mm guns and rounds; both cars' data and cards `ai_weight 0`; cases 25, 26, 30, 31 (car parts); save 19.
Gate: `skirmish40` winners and lengths equal `verify-s8a/skirmish40.json` per seed (no behaviour change for the old roster); differences are bugs or explained.
**Build B: trucks.** Role and `[supply]`; loading; `Source::Truck`, `source_for`, `rearm_vehicle`, belts, men; `DropCrates`, protocol 11; cargo, detonation; both trucks' data and cards `ai_weight 0`; cases 27, 28, 29, 31 (truck parts); per-field tests; net loopback.
Gate: `skirmish40` winners and lengths equal Build A's, except any change from forward MG belts (report it).
**Build C: AI, presenter, proof.** Section 4 rides, measured then tuned; recon and truck AI; tanks to nearest stock, float; `s8b` JSON; case 32; presenter and four HUD faults; determinism; screenshot. Then balance on 40, the three 160 sets, timing40 clean; `targets S8b.toml`; pooled Germany; profile if a rise above 25 %. A set-level miss with the pool in band is noise.

## 15. What later slices need

SetHatch and unbuttoned seats (6.9), radio observers (fire-mission `observer` on a vehicle), carried `[transport]` on trucks (role gate, AI), M20 (`pintle` ring, supply role), `fire_from`, towing (6.10, wheeled trucks tow), trucks drawing from an HQ, road speeds, push-limit reserve, tank optics values, half-track wheeled front axles.

## 16. Owner questions (work proceeds on (A))

Full text in `docs/TODO.md`. S8b-1 Recon and trucks outside the vehicle limit, capped by their cards: (A) yes. S8b-2 `DropCrates` carries the kind (three crate defs): (A) yes. S8b-3 Trucks auto-load `load_kg` in base, paid per kg: (A) yes.
S8b-4 Bombs from a truck only as dropped crates: (A) yes. S8b-5 Trucks carry no men ([transport] is role-gated): (A) supply only. S8b-6 Forward MG belts at HQ, dump, truck: (A) yes.
S8b-7 Trucks unarmed: (A) yes. S8b-8 No road speed: (A) yes. S8b-9 Detonation needs 40 kg of shells or explosives: (A) yes. S8b-10 Tanks keep `stationary_sight_pct` 100: (A) yes.
S8b-11 Figures from memory are marked verify: (A) use them. S8b-12 2 cm gun: one-shot model with averaged reload: (A) yes. S8b-13 Push-limit reserve left out: (A) yes. S8b-14 AI tanks fire about 7 main rounds each, so truck rearms stay low; crates are the measured work: (A) keep.
S8b-15 Half-track picks squads within 250 m: (A) yes. S8b-16 Wheeled driving uses the undilated layer: (A) yes. S8b-17 AI recon 8 %, trucks 5 % of income: (A) start values. S8b-18 M8 .50 ring is a hull-frame pintle: (A) yes. S8b-19 Drop Crates hotkey `I`: (A) yes. S8b-20 Trucks load only at base: (A) yes.
