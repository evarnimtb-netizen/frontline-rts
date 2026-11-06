# S7: assault guns and tank destroyers (log)

Owner 2026-11-06: "Start S7". Branch `s7-casemate-td` from `9f216e9`, not
pushed. Spec `S7-casemate-td.md`; targets `targets/S7.toml`; owner questions
S7-1..15 in `docs/TODO.md`, each with its (A).

## Commits

- `60dfc91` plan; `a2b104b` schema, port; `e15ed25` content, armour-test 17-18;
  `6aee94b` open top, 19-20; `0fde907` AI; `c0f14d6` presenter; `588448f` log;
  `1c8898b` Tune (docs only); `421e617`, `1a13f63` review fixes.

## What was built and why

- **Schema (spec 1, 2).** `[[boxes]]` 1..=4, each with its own 8-zone plate
  table, optional `open` face mask and `floor_cm`; optional `[turret]`; mounts
  `turret_main`, `casemate`, `coax`, `hull`; `[[seats]]` with id, role, box, `in`.
  Zone id = box index x 4 + face group (S7-9), so the Pz IV and Sherman keep
  ids 0-7 and their names.
- **Port proof (before and after the cards joined the rosters).** Golden
  ballistics 46/46, file not edited. Armour-test cases 1-13, 15, 16 equal
  `verify-s6b/armour5.json` check for check (14 is not in the default run).
  Skirmish 40: every `per_match` field except `hash`, every faction and top-level
  number equals `verify-s6b/skirmish40.json`; added fields only. Expected
  difference: state and setup hashes, so saves. Review re-ran it: 0 differences
  except the hash.
- **Casemate gunnery (spec 3).** `lay_gun` clamps to the arc (StuG +-10 deg) and
  reports "laid" only inside it. A bearing outside the arc, beyond a 3 deg margin,
  sets `VehicleState::lay_turn`; the driver pivots (proportional steer, no
  overshoot), then the gunner lays. Orders, auto targets and the AI share it.
  Cleared on wreck, abandon, `clear_orders`, bail. Low silhouette via
  `spot_size_pct` (StuG 160).
- **Open top (spec 4).** An open face has no plate; a box's bottom is its floor.
  AP cores go through the boxes inside, HE bursts on the first box inside, small
  arms hit the first seat, no spall. Fragments reach the first box inside or an
  exposed seat; a burst on the M10's deck still reaches its crew. Thrown items
  collide with enemy vehicles (S7-5, S7-13): a plate drops them; an open face
  takes them in (`Thrown::inside`) and they burst inside.
- **Content (spec 5).** `ger_stug3_g` (80 mm casemate, 30 mm sides, 40 km/h,
  500 MP, pop 10, 120 s); `usa_m10` (turret 57 mm, hull 38 mm at 55 deg, 520 MP,
  pop 10, 120 s). Guns `stuk40_l48` (kwk40_l48 ballistics), `m7_3in` with
  `3in_m62` (APC, penetration derived, S7-6) and `3in_m42a1` (HE). Cards appended
  to each roster's end, so `scenarios::tank()` still picks the Pz IV and Sherman.
  M10 traverse 5 deg/s. StuG traverse and casemate/M10 elevation unsourced (S7-10).
- **AI (spec 6).** Both cards `ai_weight` 100 through the `vehicle` slot (S7-2).
  Overwatch: anchor `overwatch_back_m` behind the most threatened own point, best
  of 9 spots, face the threat, relocate after 2 shots or a penetration. Open tops
  reverse 40 m from infantry within 60 m; infantry throw frags (else satchels) into
  a stopped open top within 25 m.
- **Presenter (spec 7).** `vehicle_types` gains casemate, open-top, boxes, gun
  pivot and seat keys (`godot/docs/match_state.md` "S7"); call-in panel has up to
  three columns (S7-14).

## Review, fix and tune

- **M1 (fixed):** overwatch sent FaceUnits back to the axis while a laid gun had
  a target, so the hull swung between shots. Now only with no target. Test
  `overwatch_does_not_face_away_from_an_engaged_target`: 5 faces before, 0 after.
- **M2 (fixed by a knob):** StuG and M10 were bought in nearly every match but
  scored 2.5 / 1.7 % of kills; the 80 m anchor kept them idle.
  `overwatch_back_m` 80 to 35 (`skirmish.toml`, measured there).
- **L1 (fixed):** a fully rejected spot list returns a point behind the anchor.
- **L2 (fixed):** T34e and T36a are `kind = "report"`, min 0.
- **L3 (no change):** AI open-top throws are 0 in play; report targets watch it.
- **Tune (`1c8898b`):** StuG cost 425-575, 800 matches: +0.2 points per 15 MP
  (95 % interval -0.7 to +1.0). Price is no lever; kept 500.

## Measurements

### Armour-test (`--seeds 5`, 10 runs per case)

| Case | Result |
|---|---|
| 17 StuG frontal, 300 m | StuG wins 100 %, fires 100 %; first hit StuG 1.4 s, Sherman 8.5 s |
| 18 Sherman at 45 deg, 200 m | Sherman wins 90 %, StuG fires 10 %; first hit 2.5 s (1 run), 1.3 s |
| 19 grenade, stopped M10 | lands inside 25/30; crew hit 25/30 |
| 19 grenade, moving M10 | lands inside 0/30 |
| 20 81 mm near miss, M10 crew | 2/60 hit; Sherman crew 0/60 |

Case 18: the Sherman starts laid; 9 of 10 hits penetrate the 30 mm side and
disable the StuG before its pivot and lay (about 2.5 s). T34e misses as a hard
target (S7-11, now a report). `casemate.rs` proves the pivot: fires within 8 s on
a target that does not shoot back. Case 20: crew boxes rise 14-24 cm over the
66 cm rim; ground-burst fragments climb at 2-50 deg, so the rim and turret wall
shadow almost every line (S7-12, a report). Deck bursts hit crew in half.

### Skirmish balance

| Set | Germany | Base max | HE share | Mortar G / US |
|---|---|---|---|---|
| 40 (seeds 1-20, both bases), final 35 m | 45 % | 55 % | 21.0 % | median 16.6 min |
| 160 seeds 1-80, 80 m (Build B) | 43.75 % | 51.3 % | 20.7 % | 14.9 / 11.3 % |
| 160 seeds 201-280, 80 m (Tune oos) | 51.9 % | 53.1 % | 21.0 % | 14.1 / 11.9 % |
| 160 seeds 1-80, 35 m | 48.8 % | see note | 21.0-21.7 % | 14.0-14.5 / 11.7-12.2 % |
| 160 seeds 101-180, 35 m | 45.0 % | see note | 21.0-21.7 % | see note |
| 160 seeds 201-280, 35 m | 48.8 % | see note | 21.0-21.7 % | see note |

Note: at 35 m the fix stage reported base max 50.0-60.0 % and mortar ranges
across sets, not per set. Pooled Germany: 47.5 %
over the three 35 m sets (480 matches); 47.8 % over the two 80 m sets (1-80,
201-280); 51.7 % over the 800 Tune matches.

Vehicles at 35 m (sets 1-80 / 101-180 / 201-280):
- StuG 2.8-3.3 % of German kills; M10 1.9-2.3 % of US kills; both together about
  10.3-10.6 % of AI manpower.
- KOs per bought: StuG 0.064 / 0.095 / 0.074, M10 0.070 / 0.054 / 0.103. At 80 m:
  StuG 0.015, M10 0.032; Pz IV 0.13, Sherman 0.086.
- Pz IV share of German kills 18.8 / 17.4 / 16.9 % (T32a limit 18 %).

### Targets

- Build A: 19 pass, 6 miss. Build B (40 + 160 + armour5): 75 pass, 3 miss (T5c,
  T34e, T36a). Tune (out-of-sample 160, 80 m): 83 pass, 3 miss, 4 skipped.
- Final (`--runs skirmish40,armour5`): **25 pass, 0 miss (hard 0), 65 skipped.**
  T34e (10 %) and T36a (3.3 %) pass as reports.
- The 160 sets ran with `skirmish --json`, not `targets`.
- Tick fix (orchestrator): clean p99 1.05 ms (S6b 0.41) came from a vehicle re-searching an unreachable overwatch spot
  every 2 ticks at the 8000 cap; now it stops, re-paths wait `repath_gap_s`, AI adopts its spot after 20 s. p99 0.38.

## Checks (commit `1a13f63`)

- `tools/check.sh quick 9f216e9`: fmt, clippy ok; 524 passed, 0 failed.
- Release `--include-ignored`: 588 passed (Build B); after M2 only the four s7_ai
  tests re-ran (4/4).
- Godot: `test_match_view` 293/293, `test_data_reload` 21/21, `test_net_ui` 143/143,
  `check_project` OK. `validate mods/ww2` ok; bridge builds.
- `SAVE_VERSION` 16 to 17 once; `PROTOCOL_VERSION` 9 unchanged. Determinism at
  1/2/3/8 threads: `casemate.rs`, `open_top.rs`.
- Screenshot `s7/s7_stug_m10.png` looked at (StuG casemate, M10 open turret, call-in tile).

## Known gaps

- Germany's share swings by set (43.75-51.9 % at 80 m); pooled 47.5 % is in band.
- StuG and M10 still take about 3 % and 2 % of kills; T32a misses on seeds 1-80 (S7-15).
- AI open-top throws never happen in play (L3); covered by `s7_ai.rs` only.
- Zugtrupp and Munitionstraeger are never bought on skirmish (not traced); vehicle_view draws <= 48 per type.
- Thrown items hit vehicles (S7-13); no aimed small arms at open-top crews (S7-7).

## What S8 must know

1. The vehicle schema is general (boxes, turret, mounts, seats). Keep box order
   `hull`, `turret` so zone ids stay stable.
2. `SAVE_VERSION` 17 and `PROTOCOL_VERSION` 9 already include S7 state; S8 bumps
   once only if its save layout changes.
3. Balance levers are AI habits, not price: `[ai.armour]` `overwatch_back_m` (35)
   and `td_hunt_m`. Check each on the 160 sets: Germany 45-55 %, HE 20-26.3 %,
   mortar 10-15 %, no type above 18 % of kills.
4. Open tops are live. S8 half-track and truck decks should reuse
   `Thrown::inside` and `HitVia::Open`.
5. Call-in panel: three columns. Jagdpanzer IV, 38(t), M18 remain data work (spec 5).
