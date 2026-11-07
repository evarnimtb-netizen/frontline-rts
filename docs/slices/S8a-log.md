# S8a: half-tracks (log)

Owner 2026-11-06: "start an overnight session for next development process".
Branch `s8a-halftracks` from `17a94df`, not pushed. Spec `S8a-halftracks.md`;
targets `targets/S8a.toml`; owner questions S8a-1..19 in `docs/TODO.md`.

## Commits

- `0592f9c` plan; Build A: `de08c2f`, `1c5574c`, `dbad246`; Build B:
  `493d4bc` sim/AI/CLI, `f9121af` presenter and tests, `1c60fc8` AI weight,
  then the owner-question and close commits.

## Build A: what was built and why

- **Main gun optional (S8a-1).** `VehicleSetup::main_gun() -> Option`,
  `mg_mounts()` (the mounts after the main gun, `VehicleState::mgs` order),
  `main_ammo()`, `primary_weapon()` (card and reveal weapon). Validation: a
  `transport` has 0..=1 main guns, a gunner seat and the `gun` / `ammo_rack`
  components only with one. Gunnery runs the main-gun block only with a gun.
  `VehicleState::gun_out` replaces `component(Gun) == 0` in the bail triggers
  and the AI: without a main gun, every MG dry or unmanned (S8a-16).
- **Pintle MGs and shields (6.7).** `mount = "pintle"`: pivot (hull frame),
  muzzle from it, `traverse_deg` (ring = [-180, 180]), `traverse_deg_s`, seat.
  `MgState.yaw` (saved, hashed, checked inside the arc on load) turns toward
  the target at the rate; the MG fires only when on it (2 mrad). `[[shields]]`:
  a box in the pintle's frame (or the hull's) with one thickness. The
  narrowphase reports `HitVia::Shield(k)`; the Damage stage runs the plate rule
  once (RNG `SHIELD`); a non-bursting round that passes is traced on 8 m into
  the same vehicle (`hit_armour_past`); fragments pierce it by the thin-plate
  rule. Shields block blast overpressure to exposed crew; thrown items bounce.
- **Small arms against thin armour (6.4).** Weapon `penetration` rows
  (`[range_m, mm]` at 0 deg) and `core` (damage, length, max boxes). A round
  from a weapon with a table that strikes a plate goes to the Damage stage
  (`Blow::Armour`): `penetrates` with the table's `pen0` (the same `AmmoTable`
  interpolation as shells), cos from the hit, face-hardened plate x
  `face_hardened_pct` (125), ricochet past `small_arms_ricochet_deg` (70). A
  pass: the core hits at most `max_boxes` inner boxes, no spall, one
  `Penetration` event, `Stats::small_arms_pens`. Anything else is a stopped
  round as before (no event, no attacker, repairs go on).
- **Reach table and targets.** `ArmourTables::reach[weapon][vehicle][zone]`
  (load time; the greatest range at the top of the ±7 % band at 0 deg on the
  slope, 0 never). A soldier whose weapon has a table may pick a seen enemy
  vehicle nearer than the nearest man (soldiers win a tie, S8a-17): (a) the
  head of a living crewman whose seat rises over an open box's rim (S7-7: the
  M10's turret crew, the half-tracks' commanders), else (b) the facing plate of
  the box with the longest reach that covers the distance. One comparison per
  box; `VMark` scratch list per tick. No table (SMG, pistol, StG 44, carbine):
  never targets vehicles (S8a-18; the rifles' ball table beats no plate).
- **Thin-plate fragment rule (S7-8).** HE `frag_pen_mm` (75 mm and 3-inch 12,
  mortars 9, grenades 3; S8a-13) falls with distance as `frag_damage` does; a
  fragment pierces a plate whose effective thickness (thickness x material /
  cos) is below it and strikes the first box inside (`frag_plate_pierces`).
- **Transports (S8a-5).** Role `transport`, outside `[vehicles] limit`, unlock
  floor `[vehicles.transport] unlock_s` (120, never later than `unlock_s`),
  cards `max_alive = 2`. `[transport]` + `[[passenger_slots]]` are parsed and
  validated (slots count as inner boxes; cap 14 -> 24) but unused until Build B.
- **Mobility (6.8).** `type = "halftrack"`: the tracked drive (S8a-6); data
  only: bog 15 %, turn 26 / 24 deg/s, 750 / 850 cm/s, ditch block 19 dm.
- **Content.** SdKfz 251/1 D (3 boxes: lower hull, engine, open compartment;
  14.5 / 8 mm; MG 34 pintle +-35 deg, 60 deg/s, 2,010 rds, 8 mm shield;
  capacity 10) and M3A1 (lower hull, hood, open body; 12.7 / 6.35 FH written
  6.4 mm; M2HB ring, 30 deg/s, 700 rds; capacity 11). Weapon `m2hb`,
  cartridge `50bmg_belt`, pen tables on MG 34/42 (SmK), M1919 (.30 M2 AP),
  rifles and BAR (ball). Cards 290 / 310 MP, pop 3, 60 s, cap 2, `ai_weight 0`.
  Figures quoted from memory are marked "verify" in the TOML.
- **Deviations from the spec (S8a-19).** Door times in ms; `[vehicles.transport]`
  and the gunnery keys are tuning tables (defaults for older rules). The 251 has
  three boxes, not two (the engine deck is its own box). Slot boxes are not hit
  volumes yet (Build B).

## Build A: measurements

- Port proof: golden ballistics 46/46; armour-test cases 1-20 equal the S7
  file (case 19 re-isolated); skirmish 40 with the new data stripped equals
  S7 match for match. With the data, 23 of 40 matches differ, all from 6.4
  (rifles and MGs at the M10's exposed crew); the thin-plate rule alone
  changes none. Case 21: MG 42 vs M3A1 side 80 / 80 / 0 % at 50 / 150 /
  300 m (276 m reach), .50 vs 251 side 100 % at all three, rifles 0.
- Tests: `tests/halftracks.rs` (10); release 600 passed. `s7_ai.rs`
  back-off test scans seeds 5-9 (same assertion).

## Build B: what was built and why

- **Riding (`crate::transport`).** `Unit::riding { vehicle, slot }` and
  `VehicleState::{riders, door_ticks, unload, trip}`. Riders are out of the
  soldier spatial hash (no ORCA, capsule hits, enemy marks or capture), plan
  no velocity, are not steered, never fire or throw; after the vehicles
  move, `follow_slots` puts each at the bottom of his slot box. The frame
  carries the occupied slot boxes: `HitVia::Rider` (struck over the rim,
  tested like exposed seats), `Inner::Rider` (cores, spall, bursts,
  fragments, items inside), blast overpressure as an exposed seat. A rider
  at 0 HP falls in the Damage stage's kill list (`DamageCtx::fallen`);
  `kill` frees his slot (`riders_lost`). Rounds stopped by the vehicle
  suppress riders as a covered near miss (S8a-23). Spotting: eye at the
  slot top; seen while the vehicle is seen and the slot clears the rim.
- **Embark / Disembark** (`CommandKind` at the end; PROTOCOL 10; relay doc).
  Embark refuses a squad that does not fit the slots not promised to others;
  men walk to the door (around the hull corner, S8a-24) and board one per
  `board_ms` within `board_reach_cm` + 1 m while the vehicle is below
  `move_block_cm_s`. Disembark stops the vehicle; one man per `exit_ms` at
  the door or `exit_over_sides_ms` over the side away from the last hit
  (his box raised `exit_raise_cm`). A squad whose last rider is out holds.
  Wreck, abandon or fire: everyone out at once on his side, stunned.
  Squad commands to a squad with men aboard are refused; soldier commands
  drop riders.
- **Fixes found on the way.** The crew rule abandoned two-man half-tracks on
  spawn (crew_below 3): floor capped at seats - 1 (S8a-20). Rider hits no
  longer count as vehicle penetrations (S8a-22). `supply.rs` rearm read
  `mounts[k + 1]` (main gun at 0): crashed with half-tracks in matches.
- **AI (`ai::transport`, `[ai.transport]`).** Jobs Load / Ride / Unload /
  Support (the S7 overwatch habit) / Return, saved per vehicle squad;
  dismount when under fire (sides), when the ground `dismount_back_m` ahead
  is in sight of a known enemy within `dismount_sight_m` (door), on arrival
  or when stuck; failed moves counted (`max_fails`), no repeat move within
  `repath_s`. Buy: per-card eligibility (transports from 2:00 outside the
  limit, tanks from 4:00 under it); `ai_weight` 100 (S8a-21).
- **Presenter.** Half-track blockouts (meshgen `transport`: hull boxes, open
  compartment, tracks and wheels, shield and MG on the pintle yaw), riders
  drawn crouched in their slots, vehicle card "Aboard n/cap" with the
  passenger squads, squad card "Mounted (SdKfz 251)", Board V (shares the
  Crew slot; right-click on an own half-track), Unload U / Shift+U,
  `match_state.md` "S8a", autoplay scene `halftracks`.
- **CLI.** armour-test cases 22-24; JSON `factions.<f>.s8a` (sums and AI
  counters) and `s8a_derived`.

## Build B: measurements

- Checks: `check.sh quick 17a94df` all ok (577 tests); release
  `--include-ignored` 609 passed, 0 failed; Godot `test_match_view` 299/299,
  `test_data_reload` 21/21, `check_project` ok; settings.cfg unchanged; net
  loopback scripts `Embark` and `Disembark` on both slots.
- armour5 (cases 22-24): a bazooka rocket hits a passenger in 90 % (0.83 men
  lost); a stick grenade into a stopped 251 kills 63.7 % of its riders, into
  a moving one lands inside 0 %; unloading under MG 42 fire takes 7.0 s by the
  door and 4.2 s over the sides (2.9 vs 3.3 men hit).
- Targets: skirmish40 + armour5 40 pass, 0 miss; skirmish160 68 pass, 3 miss
  (T5c Germany 40.0 % [45,55]; T41b USA 0.86 transported per match [1,-];
  T48b USA 0.90 trips per half-track [1,-]); timing40 tick p99 0.527 all /
  0.725 peak ms (S7 after the fix 0.38 / 0.56: +39 % / +29 %, unprofiled).
- skirmish160: HE 20.4 %, mortar 13.8 / 12.1 %, Pz IV 16.3 % and Sherman
  12.5 % of their side's kills (T32 holds; S7-15 out of sample not run),
  251 0.8 %, M3A1 3.0 %. Half-tracks bought in 94 / 90 % of matches; rides
  1.7 / 1.2 per match, 1.11 / 0.86 transported; a ride takes 62 / 64 s and
  saves 16 / 38 s against walking the straight line (lower bound, S8a-25);
  riders lost 0.04 / 0.09 per match; dismounts under fire 7 / 10 %.

## What the Tune must know

1. Germany 40.0 % on 1-80 (S7 48.1): Build A's 6.4 targeting already gave
   40 % on the 40 set; half-tracks add little to kills. Pooled 480 not run.
2. Rides: about one trip per half-track; most are lost or wait at the front
   (Support returns only for a waiting squad). Levers: `ai_weight`,
   `[ai.transport]` `support_s`, `wait_s`, `min_ride_m`, PICK_M/MEET_M.
3. Tick time rose ~0.15 ms; profile riders (frame rider boxes, `frame_of`
   in spotting) and the small-arms vehicle search before tuning.
