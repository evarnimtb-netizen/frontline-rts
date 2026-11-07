# S8a: half-tracks (log)

Owner 2026-11-06 overnight session. Branch `s8a-halftracks` from `17a94df`,
head `b3627a6`, not pushed. Spec `S8a-halftracks.md`; targets `targets/S8a.toml`;
questions S8a-1..31 in `docs/TODO.md`.

## Commits

- `0592f9c` plan. Build A: `de08c2f`, `1c5574c`, `dbad246`. Build B: `493d4bc`,
  `f9121af`, `1c60fc8`, `69ed42c`, `45d2cc7`, `2308e7c`. Tune: `2240a40`.
- Review: no repo change. Fix: `b3627a6`.

## What was built, and why

Build A
- **Main gun optional (S8a-1).** `main_gun()` returns Option; `mg_mounts()`,
  `main_ammo()`, `primary_weapon()`; about 40 call sites ported. `gun_out` (no
  gun: every MG dry or unmanned) drives the bail triggers and the AI. MG order in
  `VehicleState::mgs` unchanged.
- **Pintle MGs and shields (6.7).** Own arc and traverse; `MgState.yaw` saved,
  hashed, load-checked; fires only on target (2 mrad). Shields are hit-test boxes
  with one thickness and no contents.
- **Small arms vs thin plates (6.4).** Weapon `penetration` tables and `core`. A
  penetrating round hits at most `max_boxes` inner boxes, no spall, one
  Penetration event; anything else is a stopped round. Face-hardened x125,
  ricochet past 70 deg. Reach table `[weapon][vehicle][zone]` built at load; a
  soldier picks a vehicle only if nearer than any visible man (S8a-17): first an
  exposed crewman's head over the rim (settles S7-7), else a beatable plate.
- **Thin-plate fragments (S7-8).** `frag_pen_mm` 12 for 75 mm and 3-inch HE, 9
  mortars, 3 grenades; falls with distance like fragment damage.
- **Transports (S8a-5).** Role `transport`, outside the vehicle limit, unlock
  floor 120 s, card `max_alive` 2. **Mobility (6.8):** `type = "halftrack"`,
  tracked drive, cross-country 750 / 850 cm/s, bog 15 %.
- **Content.** SdKfz 251/1 D: 3 boxes, MG 34 pintle +-35 deg, 8 mm shield,
  capacity 10. M3A1: face-hardened 6.35 mm written 6.4, M2HB ring, capacity 11.
  Cards 290 / 310 MP, pop 3, 60 s, cap 2. Memory-quoted values marked verify.
- **Deviations (S8a-19):** door times in ms; tuning tables; the 251 has 3 boxes.

Build B
- **Riding (`crates/sim/src/transport.rs`).** `Unit::riding`;
  `VehicleState::{riders, door_ticks, unload, trip}`. Riders are out of the
  spatial hash, ORCA, enemy marks and capture; each tick placed at their slot.
  Hit via `HitVia::Rider` (outside, over the rim) or `Inner::Rider` (inside).
  Rounds stopped by the vehicle suppress riders (S8a-23).
- **Embark / Disembark (PROTOCOL 10).** One man per `board_ms` at the door; exit
  per `exit_ms` at the door or `exit_over_sides_ms` over the side away from the
  last hit. Wreck, abandon or fire: all out at once, stunned.
- **AI (`ai/transport.rs`, `[ai.transport]`).** Load, Ride, Unload, Support (S7
  overwatch), Return. Dismount under fire over the sides, out of sight at the
  door, on arrival or when stuck; `max_fails` and `repath_s` stop move loops.
  Buy gated per card: half-tracks from 2:00 outside the limit, tanks from 4:00.
- **Presenter.** Blockouts, crouched riders, "Aboard n/cap", "Mounted", V / U.
- **Bugs fixed on the way.** Two-man half-tracks abandoned on spawn (floor capped
  at seats - 1, S8a-20); supply rearm read `mounts[k + 1]` and crashed matches;
  rider hits no longer count as penetrations (S8a-22).

## Final targets (Fix run)

`skirmish40` + `armour5` + three 160 sets (seeds 1-80, 101-180, 201-280):
**119 pass, 7 miss, 4 skipped.** Hard misses: T5c Germany 42.5 % on 1-80 (band
45-55, S8a-30); T41b USA 0.975 transported per match (floor 1); T48b USA 0.857
trips per half-track bought (floor 1) (both S8a-29). Soft misses: T41c-f, ride
vs walk, all negative.

## Balance (Fix, 160 sets)

| Seeds | Germany wins | HE share | No winner | Longest match |
|---|---|---|---|---|
| 1-80 | 42.5 % | 21.0 % | 0 | 33.1 min |
| 101-180 | 45.6 % | 20.3 % | 0 | 35.4 min |
| 201-280 | 48.1 % | 20.2 % | 0 | 34.0 min |

Germany pooled over 480: 45.4 % (in band). T32: Germany top type 17.9 % (cap
18), USA 15.1 %; T42c report 20.0 %. Half-tracks bought in 87-94 % of matches;
251 takes 0.8 % of German kills, M3A1 3.8 % of US. `ai_fails` 0.

## Armour tests

- Case 21 (Build A): MG 42 vs M3A1 side 80 / 80 / 0 % at 50 / 150 / 300 m (reach
  276 m); .50 vs 251 side 100 % at all three; rifles vs either front 0 %.
- Case 22: bazooka rocket into a loaded 251: passenger hit 90 %, 0.83 men lost.
- Case 23: stick grenade into a stopped 251: 63.7 % of riders lost; moving: 0 %
  land inside (target at most 20).
- Case 24: unload under MG 42 fire. Build B (10 runs): door 7.0 s, sides 4.2 s,
  2.9 vs 3.3 men hit. Fix (80 runs): 3.02 vs 3.36 men hit, sides 2.3 s faster.

## Time saved per transported squad

- Build B figure, retired (review M2: clock from the first man aboard): +16 s
  Germany, +38 s USA per ride.
- Fix figure (`AiRideMeas`: Embark order to the lead inside the point's capture
  radius, vs a straight walk from the order): per ride, seeds 1-80 / 101-180 /
  201-280: Germany -29 / -26 / -21 s, USA -14 / -15 / -6 s. About half the rides
  never reach their point. **AI riding is slower than walking** (S8a-28).

## Tick time

timing40 (clean, `--jobs 3`, after Fix): **p99 0.493 ms all, 0.625 ms peak**, vs S7
0.38 / 0.56 (+30 % / +12 %). Unprofiled. Suspects: `vehicle_aim` on soldier
retargets, `follow_slots`, `transport_status`, `frame_of`.

## Tune

`[ai.transport]`: `min_ride_m` 150 to 120, `wait_s` 30 to 20, `support_s` 60 to 30.
HE slope vs `ht_cost_mult` +0.0 per 0.05, so costs stay. Spec section 15.

## Review and Fix (17a94df..2240a40: 0 high, 4 medium, 6 low)

| ID | Finding | Fix |
|---|---|---|
| M1 | Rides picked from the vehicle: 41 % of German rides were short; some squads walked backwards | Point at least `min_ride_m` + `dismount_back_m` + 20 m from the lead, and `min_ride_m` beyond the door |
| M2 | Time saved overstated | Measure as above; `AiState::ride_meas` (cap 32) and four counters, per-field test |
| M3 | T15c stall (seed 175 swapped, 45 min) from the Tune | Gone with M1 and M4: seed 175 wins at 11:26; no stalled match in three 160 sets |
| M4 | HE band missed out of sample (T8b 19.1 %) | Half-tracks no longer count toward the vehicle slot share of AI buys; HE 20.2 % or more on all three sets; weapon values unchanged |
| L1 | Tick unprofiled | Small-arms search skips weapons without tables; still +30 % p99 |
| L2 | Soak assertion loosened | Client logs its last 64 stamped turns; lost orders must be due at or after the end tick |
| L3 | Refusals untested | Table test: Embark into enemy, wrecked, burning or unloading vehicle; Disembark of empty; Move and Retreat to mounted squad; control row accepted |
| L4 | "AP 0 HE 0" on a gunless vehicle | Bridge sends AP/HE only with a main gun; panels hide them |
| L5, L6 | Sides hit more men; MG comments wrong | Kept (S8a-31); comments fixed |

## Checks
- `check.sh quick 17a94df`: ok, 578 passed. Release `--include-ignored`: 610
  passed, 0 failed. Godot tests exit 0; `settings.cfg` unchanged.
- Determinism at 1/2/3/8 threads, save/load/continue while embarked: equal.

## Known gaps

- AI riding is slower than walking (S8a-28); half-tracks make about 1.2 trips each.
- T5c, T41b, T48b miss (S8a-29, S8a-30). Pooled Germany is in band.
- Tick p99 +30 % over S7, unprofiled.
- "MG dry" shows on the 251 card while the sim reports 2,010 MG rounds: bridge or
  card bug, not fixed. Gun, ring and rack bars still show (S8a-27). Riders are
  crouched, not seated.
- Vehicle MGs do not fire at half-tracks (S8a-17 B). Pen tables are AP for every
  round (S8a-3). Squads never split (S8a-2). Out of scope: tank riders, fire from
  the vehicle; StuG roof MG and M10 .50 (S7-1); the mock has no half-track.

## What S8b must know

- Transports reuse `crate::transport`, `[transport]`, `[[passenger_slots]]`,
  `transport_unlock_s`, `max_alive` and the per-card buy gate in `ai.rs`.
- Wheeled mobility: `type = "wheeled"` is reserved.
- Make rides pay before adding trucks (S8a-28): board where the squad stands,
  shorter `dismount_sight_m` and boarding waits. Tune with `AiRideMeas` (`obj_*`
  keys); T41g/h are the old upper bound.
- Recon optics: copy the spotting eye at the slot top. Supply trucks: never
  index `mounts[0]` as the gun; use `main_ammo()` and `mg_mounts()`.
- Profile the tick first (L1). Versions: SAVE 18, PROTOCOL 10; an S8b save or
  wire change bumps once. Hotkeys in use: V, U, Shift+U.
