# S8a: half-tracks (log)

Owner 2026-11-06: "start an overnight session for next development process".
Branch `s8a-halftracks` from `17a94df`, not pushed. Spec `S8a-halftracks.md`;
targets `targets/S8a.toml`; owner questions S8a-1..19 in `docs/TODO.md`.

## Commits

- `0592f9c` plan; Build A: `de08c2f` schema, sim, content, tests; the Build A
  close commit (case 21, port proof fixes, this log).

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

- **Port proof.** Golden ballistics 46/46, file unchanged. Armour-test cases
  1-13, 15-20 equal `verify-s7fix/armour5.json` check for check (case 19 after
  its thrower squad lost its rifle and MG rounds: with 6.4 they shot the
  M10's exposed crew and hit it in 30 of 30). Skirmish 40 with the new data
  stripped (no `penetration`/`core`/`frag_pen_mm`; half-track cards in the
  rosters): 40 of 40 `per_match` records equal `verify-s7fix/skirmish40.json`
  except `hash`; every other field equal; added fields only.
- **Skirmish 40 with S8a data:** 17 of 40 matches equal, 23 differ. The
  thin-plate rule changes none of them (the same 40 with `frag_pen_mm`
  stripped: 40 of 40 equal), so the difference is 6.4 targeting: German rifles
  and MGs firing at the M10's exposed turret crew (S7-7). Germany 40.0 % (base
  max 50.0) vs 45.0 (55.0); HE 20.9 vs 20.6 %; length mean 18.0 vs 17.8 min;
  Pz IV 21.4 % of German kills (19.2), Sherman 16.0 (17.7), StuG 3.1, M10 2.0;
  M10 knocked out 5 (4), Shermans 6 (8). 40 matches are not a balance sample:
  Build B measures on 160 x 3.
- **armour-test --seeds 5:** all hard checks pass. Case 21: MG 42 vs M3A1 side
  48 / 48 / 0 of 60 at 50 / 150 / 300 m (80 %; the misses are the running-gear
  band's +5 mm; 300 m is past its 276 m reach); .50 vs 251 side 60 of 60 at
  all three; rifles vs both fronts 0 of 60; MG 42 at the M10 loader's head
  60 of 60.

- **Tests.** `crates/sim/tests/halftracks.rs` (10). Release run
  (`--include-ignored`): 600 passed, 0 failed. `s7_ai.rs`
  `an_ai_open_top_backs_off_from_infantry` now scans seeds 5-9 for one where
  the M10 backs off over 10 m (same assertion): with 6.4 the riflemen shoot its
  crew and seed 5 got 8.5 m. Godot `test_match_view` 293/293,
  `test_data_reload` 21/21 with the new `vehicle_types`; settings.cfg unchanged.

## What Build B must know

1. `ai_weight` is 0 on both cards; the AI's `veh_slot` gate uses
   `vehicles < vr.limit` and `unlocked = tick >= unlock_ticks` (ai.rs ~1168):
   transports need their own gate (transport unlock, `max_alive`), or the AI
   never buys them before 4:00. `vehicles_counted` already skips transports.
2. Riding: `inner_boxes` (damage.rs) lists seats then components; add slots
   (`Inner::Rider`) there; `MAX_INNER_BOXES` is 24 and already counts slots.
   `ExposedSeat::above_rim` and `vehicle_aim` (combat.rs) are where rider
   slots become targets (S8a-9).
3. A half-track whose commander dies is abandoned on the next penetration
   within `under_fire_s` (S8a-16); riders must dismount on abandon.
4. Presenter: `vehicle_types` now carries `has_gun`, `pintles` (pivot, muzzle,
   arc), `shields`, `slots`; the snapshot does not yet carry `MgState.yaw`.
   The meshgen blockout draws box 0 only (a low slab) and a zero turret.
5. SAVE_VERSION is 18 (bumped once); add the riding fields under 18.
   PROTOCOL_VERSION is still 9.
6. JSON: no `s8a` fields yet (small-arms pens are a world stat only).
