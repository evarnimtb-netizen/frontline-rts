# S3c-armour log

Spec: `docs/slices/S3c-armour.md`. Branch `s3c-armour` (from `s3b-balance`
at `b3a8793`).

## Plan stage (2026-11-01)

`de927eb` report-only skirmish counters (`s3c` rows), `13229c5` the spec
and TODO S3c-1…13. Baseline numbers are in the spec.

## Implement Part A (2026-11-01): bail-out, abandoned tanks, re-crew

### What was built

- **Content.** `SquadRole::Crew` (validation refuses it in a roster or the
  start squads), `WeaponClass::Pistol`; `ww2:p38`, `ww2:m1911a1`,
  `ww2:ger_tank_crew` (MP 40 + P 38s), `ww2:usa_tank_crew` (two M3 + M1911A1s);
  `crew_squad` and `[[hatches]]` on both tanks (Panzer IV: cupola, two turret
  side doors, two hull hatches; M4A3 (75): commander's hatch shared with the
  gunner, loader's hatch, two hull hatches); `[vehicles.bail]` and
  `[vehicles.recrew]` tuning tables (every key with unit, meaning and
  source in `skirmish.toml`); `crew_ko_below` removed (replaced by
  `crew_below`). Validation: hatch seats exist and cover every seat once,
  openings on or inside their frame's box, crew squad of the vehicle's
  faction and role `crew`.
- **Sim** (`crates/sim/src/bail.rs`, Status stage after the vehicles' own
  status). The six triggers; a new entity kind `Abandoned` (last owner kept
  in the owner column; nobody's pop or vehicle limit; seen by all; blocks
  vehicles like a wreck; struck and damaged like a live vehicle; its fires
  burn on and burn it out to a wreck with anyone still inside); exits by
  hatch with per-seat timers (shared hatches queue, wounded × 150 %), men
  appear beside the hull below their hatch on the side the last round
  travelled toward, 0.6 m out, with their HP, the crew squad's weapons,
  1.5 s weapon cooldown and 40 suppression; one crew squad per bail-out
  (a new one if the last was wiped out); `Recrew` (walk to the hull,
  gather 1 m out along the near side, board together, `board_s` ×
  ⌈men / hatches⌉, limit and pop checked at the order and again at the
  end, refusals to the snapshot for the feed); capture flag; untrained
  seats (reload × 150 %, traverse × 75 %, spread × 150 %); `Abandon`
  (acted on in the next Status stage); crew squads dismissed after 30 s
  holding in their own base (no refund, no losses); the abandoned cap (16;
  the oldest is scrapped to a wreck); `AttackUnit` may name any abandoned
  vehicle (tanks fire at it; auto-fire never picks one; AT teams keep their
  rockets for live vehicles); Part-A AI: its crew squads retreat to base.
  Per-player `CrewCounters` (abandoned, recrewed, captured, bailed, out,
  home, scrapped). New armour events `Abandoned` (trigger), `CrewOut`
  (seat), `Recrewed` (own / captured). `SAVE_VERSION` 11,
  `PROTOCOL_VERSION` 5 (once each); `Recrew` and `Abandon` in
  `command_in_limits` (no lists) and `sdk/docs/net/relay-protocol.md`.
- **Reports.** Skirmish: tank losses split destroyed / abandoned (by
  trigger), what became of the abandoned (re-crewed, captured, destroyed
  later, scrapped, standing), crew alive at bail-out / out / home, rockets
  credited for abandonments by the last hit; `s3c` crew rows (abandonments
  with someone out; men out alive 60 s later). armour-test: duel causes
  split, abandonments count as disabling for case 4, **case 15** (bail-out:
  a–f, hard checks). The net harness scripts `Abandon` and `Recrew` too.
- **Bridge / HUD.** `vehicles[]`: `abandoned`, `last_owner`,
  `abandon_cause`, `crew_squad`, `inside`, `boarding`, `parts_out`;
  ABANDONED (512) and BOARDING (1024) flags; badge "abandoned"; squads:
  type `"crew"`, `crew_of`, `crew_of_name`, `recrew`; intents `recrew`,
  `abandon`; feed lines (abandoned with "Crew bailing out: n of m", re-crewed,
  captured either way, refused re-crew with the reason). Godot: K Crew tool
  (orders grid row 3; the stance strip is three columns wide now), right-click
  context re-crew with a tank crew selected, Bail out in the vehicle orders
  grid (asks "Again?" for 2 s), hollow slashed minimap and overlay markers in
  the last owner's colour, the abandoned tank drawn in its greyed paint,
  hover "Abandoned: Panzer IV (main gun out, left track out)", crew card
  "Crew — Panzer IV" with a To tank button. `godot/docs/match_state.md`
  updated. Mock: `stage_abandoned`, abandon / re-crew. New staged scene
  `bailout` (`--autoplay_scene armour_bailout`).

### Deviations from the spec (TODO S3c-14…19, all on (A))

- S3c-14: re-crewing needs max(`min_men`, `crew_below`) = 3 men, else a
  two-man crew would bail out again the tick it boards.
- S3c-15: boarding is all at once after the gather (no half-crewed
  abandoned state to save and re-bail).
- S3c-16: a new entity kind instead of a `NEUTRAL` owner value.
- S3c-17: Bail out sits in the vehicle orders grid, not on the card.
- S3c-18: the dismount holds fire only (the man may move at once).
- S3c-19: `AttackUnit` on an abandoned vehicle: tanks only.

### Tests

- `cargo test --workspace --release`: 447 passed, 0 failed, 3 ignored
  (was 434); the ignored soaks pass with `--ignored`
  (`soak_with_vehicles_36000_ticks`, `soak_20_minutes_seeds_1_to_3`, 29 s).
  New: `crates/sim/tests/bail.rs` (11: every trigger, the order and hatch
  exits with times and side, men inside dying in a burn-out, not
  auto-targeted / `AttackUnit` allowed, crew re-crew, riflemen untrained
  with the reload measured, AT team refused, capture on / off, limit and
  pop refusals, dismissal in base, the cap, and a scripted bail-out +
  re-crew + capture match identical at 1/2/3/8 threads and across two
  save → load → continue points, one mid-exit, one while walking); hash
  tests for every new `VehicleState` field, `Seat::untrained` and each crew
  counter; load rejections for bail state on a live vehicle, exit timers,
  crew squad links, boarding, causes, penetration ticks, hit direction and an
  abandoned state on a live entity; content tests (crews and hatches resolve,
  crew in a roster refused, wrong crew squad refused, hatch missing a seat
  refused); the bridge feed test with every S3c line; the net loopback
  scripts `Abandon` and `Recrew` on both slots (every vehicle command kind
  must be scripted). AI-vs-AI determinism across thread counts: the
  existing `ai_vs_ai_with_vehicles_is_identical_across_thread_counts` and
  `determinism.rs` pass.
- Existing tests adjusted for the new rule: `anti_tank.rs` and
  `vehicle_orders.rs` set `bail.crew_below = 0` (was `crew_ko_below = 0`);
  the engine-fire odds test turns the engine-fire bail-out off; the gunner
  refill test and the satchel AI test turn the crew-loss bail-out off; the
  AI save/load staging kills the gunner of the best-manned tank;
  `order_world` (state hashing) runs without bail-out; the crew knock-out
  unit test now expects no knock-out from `deal` (the Status stage abandons).
  armour-test's toughened targets (`pen_stats`, `crossing_target`) never
  bail out (`scenarios::no_bail`), else "disabled under fire" took them
  out of case 1 after two penetrations.
- clippy `-D warnings` and `cargo fmt --check` clean; golden ballistics
  46/46; `validate mods/ww2` ok.
- Godot (bridge rebuilt): `check_project` OK, `test_match_view` 213/213
  (196 + 17 S3c), `test_net_ui` 143/143, `test_data_reload` 8/8;
  `settings.cfg` unchanged (md5 equal to the backup); no Godot process
  left.
- Screenshot `s3_bailout.png` (scratch): the greyed Panzer IV with its
  four men beside the left track, the hollow slashed marker above it, the
  crew card "Crew — Panzer IV" with To tank, the K Crew button and the feed
  line "Panzer IV abandoned (gun and tracks out)…".

### Measured (Part A, before Part B)

Skirmish, seeds 1–20 × both bases, `--minutes 45 --jobs 6 --threads 1`:

| # | Target | Part A | Baseline |
|---|---|---|---|
| T1 | each faction 35–65 %, no base side > 62 % | GER 20, USA 20 (50 / 50 %); base 1 24 of 40 (60 %) — met, barely | 17 / 21, base 1 55 % |
| T2 | rockets + satchels ≥ 20 % of tank losses | 18 of 135 by rocket (13.3 %), satchels 0 — **missed** | 9.4 % |
| T3 | satchel at a vehicle in ≥ 6 of 20 normal-base matches | 1 of 20 (1 of 40 overall) — **missed** (Part B2) | 0 |
| T4 | rockets fired in ≥ 8 of 10 normal-base | 17 of 20 — met | 14 of 20 |
| T6 | ≥ 70 % of abandonments with crew: a man out alive; ≥ 50 % of men out alive 60 s later | 117 of 118 (99 %); 266 of 326 (82 %) — met | 0 |
| T7 | AI re-crews ≥ 1 per 10 matches; split reported | split reported; AI re-crews 0 (Part B5) | new |
| T9 | no match < 10 or > 40 min, median 15–30 | 12.1–28.0, median 16.8 — met | 11.9–30.0 |
| T10 | peak tick p99 < 1.5 ms | 1.05 ms — met (was 0.79) | 0.79 |
| T11 | tanks lost by base side within ±15 % | base 0 78, base 1 57 (+37 %) — **missed** (Part B6 map fix) | 134 / 87 over 80 |
| T14 | HE share < 60 % | max 52.8 % — met | 52.3 % |

Tank losses 135: abandoned 118 (ammo fire 66, crew 32, under fire 16,
engine fire 2, gun and mobility 2), destroyed 17 (all rack detonations).
Of the 118 abandoned: destroyed later 72 (mostly the ammo fire burning
them out), standing at the end 46, re-crewed 0, captured 0 (the AI does
neither in Part A). Crew: 359 men alive at bail-outs, 353 got out, 199
dismissed in base. Casualties per tank lost are still about 2.9 (the
crews had lost ~2 men before the trigger), against Dupuy's ~1 per tank;
the crew-loss trigger at fewer than 3 is the lever if the owner wants
history closer.

armour-test `--stress --seeds 10`: **all hard checks pass** (15 cases,
376 s; case 15 7 of 7 checks on 20 of 20 runs). 3a 300 m frontal: Panzer IV
won 6, Sherman 12, no knock-out 2 of 20 (baseline 4 / 12 / 4); the duel
losses are now mostly abandonments (3a: 6 rack detonations destroyed, 12
abandoned: ammo fire 6, crew 6). 3c / 3e flankers 20 of 20; 3d crossing
60.0 % of stationary. Case 4 flank disabling-hit rate 48.1 % (45.8),
front 48.4 % (27.8: an abandonment after a rocket now counts). Case 6
tank disabled 16 of 20 (80 %). 5b kills 0 (T9 unchanged), T14 0.033 m per
burst (unchanged; Part B8). Case 7 attackers 39 vs 160. Case 8 7 of 20.
Stress p99 1.49 ms (1.17). First run failed case 1 (52–74 hits of the
100 needed in four cells): the toughened target (gun destroyed on purpose)
was abandoned "under fire" after two penetrations; fixed with
`scenarios::no_bail` for the staging targets, then case 1 passed (160
runs).

fortify-test `--seeds 10`: PASS, identical to the baseline (no vehicles):
T10 pooled defenders 85.6 %, attackers 57.5 % (missed); USA attacking
73.8 %, Germany attacking 41.2 % (Part B4).

### What the next stage must know

- Part B is untouched: AI rearm, sticky satchel hunts, second base of fire,
  AI re-crew / crew retreat beyond "retreat at once" / denial, the map fix,
  600 m duel case 3f, T9 / T14 investigation, MG stowage on the card.
- T2 (13.3 %) and T3 (1 of 20) are still missed; B2 (sticky hunts) and
  B3 decide them. T11 (+37 %) waits for B6.
- `inside` in the snapshot is the crew still in an abandoned vehicle; the
  `s3c` crew rows read it at the `Abandoned` event.
- The skirmish "tank losses" line now includes `abandoned_<trigger>`
  keys; `KnockedOut` of an already-abandoned vehicle is not a second loss
  (`abandoned_fates.destroyed_later`).
- Bail-out makes most tank losses "abandoned"; an abandoned tank with an
  ammo fire still becomes a wreck 8 s later (72 of 118 did).
- Case 1 / 3d staging needs `scenarios::no_bail`; any new staging that
  toughens a target should call it too.
