# S7: assault guns and tank destroyers (log)

Owner 2026-11-06: "Start S7". Branch `s7-casemate-td` from `9f216e9`, not
pushed. Spec `S7-casemate-td.md`; targets `targets/S7.toml`; owner questions
S7-1..11 in `docs/TODO.md`. Commits: `60dfc91` plan, `a2b104b` Build A schema
and port, then the Build A content commit (this section).

## Build A (schema, port, casemate gunnery, StuG and M10 data)

An earlier Build A agent died on a network error after `a2b104b`; its
uncommitted StuG / M10 defs, roster entries, scenario and armour-test cases
17-18 were checked against the spec and finished, not redone.

### Port proof (before and after the new cards joined the rosters)

- Golden ballistics: 46 / 46 rows pass, file not edited.
- armour-test `--seeds 5`: every check of cases 1-13, 15, 16 (14 is the
  stress case, not in the default run) equals `verify-s6b/armour5.json`.
- skirmish 40 (`--seeds 20 --both-bases --minutes 45 --jobs 6`): every
  `per_match` field except `hash`, every `factions` number and every
  top-level number (wins, HE share, lengths, s3c, s6, tank losses) equals
  `verify-s6b/skirmish40.json`, both at `a2b104b` and again with the StuG
  and M10 cards in the rosters (AI weight 0, below). Added fields only:
  `factions.<f>.s7`, `s7_derived`.
- Expected differences: the state and setup hashes (the setup and
  `VehicleState` change shape) and so save files. `SAVE_VERSION` 16 -> 17
  once; `PROTOCOL_VERSION` stays 9 (no command or wire change).

### What was built

- Schema (spec 1): `[[boxes]]` 1..=4 with own plate tables, `open`,
  `floor_cm`; optional `[turret]`; `casemate` mount (pivot, arc, rates);
  `[[seats]]` with id / role / box / `in`; roles `tank`, `assault_gun`,
  `tank_destroyer`; turret ring iff turret. Zone id = box x 4 + face (S7-9).
- Casemate gunnery (spec 3): `lay_gun` clamps to the arc and reports laid
  only inside it; a bearing outside the arc less `casemate_margin_deg` (3)
  sets `VehicleState::lay_turn`, the driver pivots with the FaceUnits steer
  (proportional, cannot overshoot), cleared back inside the arc, on wreck,
  abandon or a move order. Same path for orders, auto targets and the AI.
- Aim and spotting use the upper box (turret, else the casemate); low
  silhouette only via `spot_size_pct` (StuG 160, M10 190, tanks 200).
- Per-type kill and knock-out counters (`kills_by_vehicle`,
  `kos_by_vehicle`), JSON `s7` / `s7_derived` (spec 10).
- Content: `ger_stug3_g`, `usa_m10`, `stuk40_l48`, `m7_3in`, `3in_m62`,
  `3in_m42a1`, cards `ger_stug3_g_squad` (500 MP), `usa_m10_squad` (520),
  appended to the rosters; sources and "verify" flags in the TOML comments.
  M10 traverse 5 deg/s (whole degrees; vehicles.md 4.4 says about 4.5).
- `ai_weight = 0` now means "the AI never buys this card" (ai.rs buy;
  documented in `variants.md`). Both new cards carry 0 until Build B gives
  the AI its habits; Build B sets 100 (S7-2).
- armour-test cases 17, 18 (StuG vs Sherman; report checks, T34 bounds).

### Tests added

- `crates/sim/tests/casemate.rs`: content lowering (roles, boxes, open top,
  arc); arc clamp (never laid outside, 15° in 2.5 s inside); pivot-to-lay
  both ways (error falls monotonically, never changes sign, gun fires in
  under 8 s, ends inside the arc); StuG vs M10 fight identical at 1/2/3/8
  threads and across save/load/continue (penetrations > 0); `lay_turn`
  saved, loaded, hashed and refused on load when |yaw| > pi, on a tank or on
  a wreck; tanks stay first in the rosters.
- `sdk/crates/content/tests/s7_vehicles.rs`: StuG and M10 defs; schema
  errors (turret and casemate, arc without 0, open hull box, unknown seat
  box, plate on an open face, missing / extra turret ring).
- `world.rs`: per-player S7 counters saved, hashed, length-checked.
- Canonical snapshot re-blessed: additions only (6 defs, 2 cards, rosters).

### Measurements (armour-test `--seeds 5`, 10 runs per case)

| Case | StuG wins | Sherman wins | StuG fired | First hit StuG / Sherman (median s) |
|---|---|---|---|---|
| 17 frontal 300 m | 100 % | 0 % | 100 % | 1.4 / 8.5 (2 runs hit, 0 pen) |
| 18 Sherman 45° at 200 m | 10 % | 90 % | 10 % | 2.5 (1 run) / 1.3 |

Case 18: the Sherman starts laid; 10 hits, 9 penetrations on the 30 mm
side, each disabling the StuG before its pivot (~1.3 s) and lay end. T34e
(StuG fired >= 80 %) misses; owner question S7-11 (A) makes it a report.

`targets S7.toml --from-json` (skirmish40 + armour5 of this stage): 19
pass, 6 miss, 65 skipped (160 sets not played). Misses: T34e (above) and
T35a-c, T36a-b (cases 19-20 are Build B).

### What Build B must know (done in Build B)

`ai_weight` 100, open faces in the sim, T33's zero-bought cards (Zugtrupp,
bearers never bought on skirmish40, not traced; still so in Build B).

## Build B (open top, AI, presenter, balance numbers)

Commits `6aee94b` open top, `0fde907` AI, `c0f14d6` presenter, then this
log. `SAVE_VERSION` stays 17, `PROTOCOL_VERSION` 9.

### What was built

- Open top (spec 4): `hit_armour` reports `HitVia::{Plate, Open, Seat}`; an
  open face has no plate (a box's bottom is its floor, never open); seats in
  an open box are struck directly over the rim (`VehicleArmour::exposed`).
  Rounds through an open face: core along the path to where it leaves the
  box, APHE filler, no spall; HE bursts at the first box (`inside_*`); small
  arms hit the first seat. Fragments: the first box inside, or the exposed
  seat; a burst on an open-topped vehicle's own plate still reaches its
  crew (closed vehicles unchanged). Overpressure reaches an exposed seat's
  head when no plate and no earth is in the line (`blast_open_crew`).
- Items sweep enemy vehicle boxes (S7-5, S7-13): a plate drops them; an
  open face takes them in (`Thrown::inside`), they ride and burst inside
  (overpressure to the box's seats, `inside_rays` up in a 90° cone, RNG
  `INSIDE_ITEM`, outer rays out the top). Lobs over an open box aim at its
  floor + 30 cm (`[vehicles.gunnery] open_top_item_*`).
- AI (`ai/overwatch.rs`): StuG and M10 `ai_weight` 100 (S7-2). Assault guns
  and TDs hold overwatch (anchor 80 m behind the most threatened own point
  toward the threat; best of 9 spots by hull-down lines; face; relocate
  after 2 shots or a penetration; a known vehicle within 300 m turns it);
  open tops reverse 40 m from infantry within 60 m; infantry throw frags
  (else satchels) into stopped open tops within 25 m. `[ai.armour]` and
  `[ai.infantry]` keys with defaults; `AiState::overwatch` and four counters.
- Presenter: `vehicle_types` + `casemate`, `open_top`, `turret_floor`,
  `boxes`, `gun_pivot`, `seat_ids` (`match_state.md` "S7"); casemate block
  fixed to the hull, open turret without roof; mock StuG / M10; call-in up
  to three columns (S7-14); scene `armour_casemate_td`, screenshot looked at.
- CLI: armour-test 19, 20; `World::drop_shell_now`, `edit_thrown`; JSON
  `s7.{open_top_*, overwatch_holds, relocations}` and `*_per_match`.
- Tests: `tests/open_top.rs` (drop in, ride, crew hit, Sherman safe, deck
  bursts, `Thrown::inside` per field, 1/2/3/8 threads + save/load),
  `tests/s7_ai.rs` (release), `world.rs` overwatch per field, meshgen, Godot.

### Numbers (armour-test `--seeds 5`; skirmish 40 = seeds 1-20, 160 = 1-80)

- Case 19: stopped M10 25/30 inside, crew hit 25/30; moving 0/30 (T35 met).
  Case 20: M10 crew hit by 2/60 near misses, Sherman 0/60: T36a misses
  (S7-12, physical: 14-24 cm of crew over the rim). Case 18 / T34e as Build A.
- 40: Germany 45 %, HE 21.1 %, median 16.6 min, p99 0.70 ms (`--jobs 6`).
- 160: Germany 43.75 % (T5c miss, band 45-55), base max 51.3 %, HE 20.7 %,
  mortar 14.9 / 11.3 %, median 18.6 min. StuG bought in 95 %, M10 93 %.
  Vehicle kills 18.6 / 16.3 % of a side's (Pz IV 16.1, StuG 2.5, Sherman
  14.6, M10 1.7); vehicle MP 25.3 / 23.9 % (StuG 9.4, M10 9.1). KOs per
  bought: StuG 0.015, M10 0.032, Pz IV 0.13, Sherman 0.086. Relocations
  1.9 / 1.2 per match; AI open-top throws 0 (no squad came within 25 m of a
  stopped M10); M10 back-offs 50.
- Targets (40 + armour5 + 160): 75 pass, 3 miss (T5c, T34e, T36a), 12 skipped
  (oos 160 and timing40 not played).

### What the Tune stage must know

- The new vehicles hardly fight: overwatch 80 m behind own points keeps
  them out of range; StuG and M10 kill 2.5 / 1.7 % and knock out almost no
  vehicles. Candidates: `overwatch_back_m` lower, `td_hunt_m` higher,
  prices (S7-3 band). Germany fell to 43.75 % with them.
- AI open-top throws need a stopped M10 near infantry; the back-off avoids it.
