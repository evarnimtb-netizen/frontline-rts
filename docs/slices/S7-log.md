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

### What Build B must know

- Set `ai_weight = 100` in both new unit TOMLs (or tune) when the AI
  habits land; until then the skirmish batches equal S6b exactly.
- Open faces are lowered (`ArmourBoxSetup.open` mask, `floor`) but the sim
  still treats every face as plated: `hit_armour`, frag rays, overpressure
  and `fly_thrown` are untouched (spec 4).
- `bought_matches_pct` for `ger_zugtrupp` / `ger_munitionstraeger` reads 0:
  those cards never appear in `s4.bought` on skirmish40 (not traced), so
  `mp_share_pct` leaves them out. Check before using T33.
- The M10 turret seats reach 14-24 cm above the 66 cm rim (turret frame).
