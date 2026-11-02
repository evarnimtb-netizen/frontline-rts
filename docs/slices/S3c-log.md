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

## Implement Part B (2026-11-01): AI armour habits, map, cases, card

### What was built

- **B1 AI rearm** (`ai::armour`, task `Rearm`): every limited MG dry, HE
  at or below `combat.suppress.tank_he_reserve` or AP at or below
  `ai.armour.rearm_ap_below` (3), and no known enemy vehicle within
  `threat_m` → Retreat to base; waits until `rearm_full_pct` (90) full or
  `rearm_max_s` (120 s) in base. A tank stuck on the way drops the trip so
  the field repair rule runs.
- **B2 sticky satchel hunts** (`ai::assault`, saved `AiState::hunts`,
  `AiHunt { vehicle, since, last_seen, last_pos, fast_since, unseen }`,
  at most 64): start as before or on a stuck (immobile, bogged, under
  repair) vehicle; kept while seen within `satchel_stick_s` (20; the squad
  goes to where it was last seen), not fast longer than `satchel_chase_s`
  (6), within `satchel_hunt_m` + `satchel_leash_m` (150 + 50), at most
  `satchel_hunt_max_s` (120), with a satchel, not retreating. Hunters are
  claimed (no point, escort or base role), crouch, crawl within
  `satchel_crawl_m` (25) in the hull MG's arc, throw their smoke toward
  the tank when pinned on the move, and do not spend the satchel on trench
  infantry. Ends counted by reason (`ai::HUNT_END_NAMES`).
- **B3 rockets** (T2 was missed after Part A): `at_hunt_stuck_m` (500)
  and `at_follow_m` / `at_follow_back_m`, measured one at a time over 80
  matches before the shell fix below: following own tanks cut rocket
  knock-outs from 48 to 33 (of ~240) and rockets fired from 260 to 216,
  so it is **off** (`at_follow_m = 0`, S3c-21); the stuck stalk made 3
  hunts and no difference (kept on).
- **B4 second base of fire**: fire weight = rounds/min × suppression over
  the base's men (squad weapon for the first `weapon_men`, sidearm for
  the rest) vs the enemy factions' strongest MG team weapon; under
  `second_base_below_pct` (100) % with ≥ 3 squads at the point, a second
  squad (MG team, else rifles) suppresses the same line from ≥
  `second_base_gap_m` (30) from the first (`base_of_fire`, `support_spot`
  avoid). fortify-test has a fourth variant (MG + rifle squad
  suppressing) and a T10 row per attacker with two bases.
- **B5 crews and abandoned tanks**: a crew squad re-crews its own
  abandoned tank (else the nearest abandoned own one within
  `recrew_reach_m` 300) when it is not burning, not useless, no known
  enemy soldier within `recrew_clear_m` (80), no enemy vehicle within
  `threat_m`, and `recrew_check` passes; it waits by its tank while men
  are still getting out; otherwise it walks back. Denial: a holding tank
  with no live target and no live enemy vehicle within `face_range_m`
  fires (`AttackUnit`) at an enemy abandoned tank within `deny_m` (300)
  in its line of sight, and stops (Stop) once a live enemy vehicle is
  near. The AI does not capture.
- **B6 base bias**: the mirrored valley band in `kessel_ridge.toml`
  (S3c-8 (A)); the spawn ring turns with each base's facing to the map
  centre (`game.rs` `spawn_spot`). (B) is built as an option:
  `[terrain] symmetry = "point"` mirrors the base-0 half (heights only;
  off on Kessel Ridge, S3c-24).
- **B7 case 3f** (600 m, both frontal). Two still tanks 600 m apart never
  spot each other (sight 175 m × 200 % size), so 3f stages them spotted
  (vehicle sight 350 m, `scenarios::DUEL_600_SIGHT_M`). It then showed no
  hit at all, also at 500 m: **bug**, a main-gun shell's lifetime was
  computed in Q16.16 as `range × 125` first, which saturates at 32 767,
  so every shell of a 600 m gun vanished after ~330 m of flight. Fixed
  (`gunnery::shell_ttl`, 64-bit; unit test). Case 3 rows now also give
  shots fired and penetrations per part.
- **B8 T9 / T14**: ground bursts threw fragments only 2–45° above the
  horizontal, so a shell bursting on a parapet crest could never reach
  men below the lip. Physical fix: where the ground falls away within
  `combat.blast.frag_fall_probe_cm` (150; 8 samples) the steepest fall
  adds rays below the band at the band's density (flat ground: no
  change). T14: the spoil crater is already ~1.5× the compact crater's
  volume (depth share 150 %), inside the 1.5–2× loose-fill range, so no
  change (S3c-23).
- **B9**: the vehicle card shows "MG 2,950" (amber under a quarter, red
  "MG dry"); the bridge adds `ammo.mg_full`; roster `mg_label`.
- **Reports**: `S3c AI <faction>` line (rearms, hunts kept unseen, hunt
  throws, hunt ends by reason, second bases, AT stuck hunts / follows,
  re-crews, crew retreats, denials); JSON `armour.ai_s3c`.

### Tests

- `cargo test --workspace --release`: 453 passed, 0 failed, 3 ignored;
  `--ignored` soaks pass (2 soaks + the stress report).
- New `crates/sim/tests/ai_armour.rs` (5): a dry-MG tank drives back,
  rearms and comes out (one trip); rearm trips identical at 1/2/3/8
  threads and across save → load → continue mid-trip; an AI crew re-crews
  its tank while a burning tank's crew walks back; an AI tank fires at an
  enemy abandoned tank; a satchel hunt sticks through a stop-and-start
  and ends when the tank drives off.
- `ai_assault.rs` (now seed 18) also asserts second bases, a satchel hunt
  and its counted end, AI re-crews, crew retreats and denial in the same
  match that runs at 1/2/3/8 threads and save → load → continue.
- `world.rs`: every new counter and each `AiHunt` field changes the hash;
  loads reject hunts with future ticks, `since > last_seen` or an unused
  squad id. `gunnery.rs`: shells live past the gun's range.
- Adjusted: `ai.rs` (crews retreat at any strength), the fragment test
  (fall rays add up to `n` per burst), `ww2_mod.rs` (three bands),
  `tests/skirmish.rs` (8 suppress runs).
- clippy `-D warnings`, fmt clean; golden ballistics 46/46; `validate
  mods/ww2` ok; fortify-test PASS; armour-test all hard checks pass.
- Godot (bridge rebuilt): `check_project` OK, `test_match_view` 215/215,
  `test_net_ui` 143/143, `test_data_reload` 8/8; settings.cfg unchanged;
  no Godot process left.

### Measured (standard set; 80 matches for T1/T11)

Skirmish, seeds 1–20 × both bases, `--minutes 45` (40) and seeds 1–40
(80), all Part B changes including the shell fix:

| # | Target | 40 matches | 80 matches | Result |
|---|---|---|---|---|
| T1 | each faction 35–65 %, no base side > 62 % | GER 19, USA 21; base 1 21 of 40 | GER 35, USA 45; base 1 41 of 80 | met |
| T2 | rockets + satchels ≥ 20 % of tank losses | 12 of 208 (5.8 %) | 21 of 397 (5.3 %) | **missed** (19.8 % before the shell fix) |
| T3 | satchel at a vehicle in ≥ 6 of 20 normal-base matches | 0 of 20 | 1 of 40 | **missed** (S3c-20) |
| T4 | rockets fired in ≥ 8 of 10 normal-base matches | 11 of 20 | 22 of 40 | **missed** (35 of 40 before the shell fix) |
| T5 | rearm trips, low-ammo time not up | 0 trips; MG dry 0.0 %, HE ≤ reserve 0.0 % | 2 trips, 1 low-ammo return; HE ≤ reserve 0.3 / 0.0 % | trips rare (need is rare); time not up |
| T6 | bailed crews survive | some man out 100 / 100 %; alive 60 s 85 / 82 % | 99 / 99 %; 80 / 80 % | met |
| T7 | AI re-crews ≥ 1 per 10 matches | 15 re-crewed (44 orders) | 24 (95 orders) | met |
| T8 | US attackers ≤ 60 %, German ≤ 50 %, defenders ≥ 75 % | fortify: US with two bases 37.5 % (one base 66.2 %); German 36.2 %; defenders 90 % | – | met with the second base |
| T9 | length 10–40 min, median 15–30 | 10.2–32.8, median 17.5 | 10.2–32.8, median 16.8 | met |
| T10 | tick p99 < 1.5 ms; stress < 10 ms | 0.79 ms; stress 1.72 ms | 0.81 ms | met |
| T11 | tank losses by base side within ±15 % | 102 / 106 | 190 / 207 | met |
| T12 | 3f 600 m | Panzer IV 14 of 20, Sherman 6 | – | target "≥ 6 of 10" written, met |
| T13 | T9 kills > 0 / T14 ≥ 0.05 m only with a physical cause | 5b: 4 of 160 killed; T14 0.035 m | – | T9 met (fall rays); T14 documented |
| T14 | S3b hard checks, HE share < 60 % per match | all pass; HE max 61.1 % (1 of 40) | 2 of 80 over 60 % | HE share **missed** narrowly |
| T15 | determinism | tests pass | – | met |

- **The shell fix moves balance a lot.** Before it (80 matches, Part B
  otherwise the same, no fall rays): 243 tanks lost, 48 by rocket, rockets
  fired in 35 of 40 normal-base matches. After: 397 lost (of 619 bought),
  21 by rocket, rockets in 22 of 40: tanks now kill each other from 330 to
  600 m, before AT teams get close. T2 and T4 need re-tuning in the Tune
  stage (S3c-26).
- fortify-test is unchanged by the shell fix (no tank); the fall rays
  lowered attacker losses there (grenades on the lip reach the trench):
  pooled single-base T10 attackers 51.2 % (was 57.5 %), US single base
  66.2 % (was 73.8 %).
- armour-test (stress, seeds 1–10): all hard checks pass; 3a Panzer IV 6,
  Sherman 12, none 2 (unchanged: 300 m was inside the old 330 m reach);
  3f Panzer IV 14, Sherman 6 (Sherman hull hits penetrate 4 of 10, turret
  6 of 6); case 4 flank 48.1 %; case 6 80 %; 5b kills 4; T14 0.035 m;
  stress p99 1.72 ms. "case 3 pooled knock-outs: rack 20 of 20" is the
  Part A artefact (destroyed tanks are only rack detonations now; the
  others are abandonments).

### What the next stage must know

- The shell-lifetime fix (S3c-26) is the biggest change in this stage and
  is not in the spec: AI tank losses rose ~60 %, rocket share fell to
  ~5 %. Tune must look at AT teams' reach, tank standoff and spotting
  (tank sight 175 m × 200 %) with long-range fire now real.
- T3 is limited by the satchel approach, not AI stickiness (S3c-20).
- `second_bases` counts squads becoming a second base (~15 per match per
  side), so second bases churn between squads as assignments change.
- 3f stages the tanks spotted; in a match a 600 m duel needs an observer.
- New owner questions S3c-20…26 in `docs/TODO.md`, all on (A).

## Tune (2026-11-02): AT mix, AT-hunt clearance, satchel hunt range

Spec section "Tune results" in `docs/slices/S3c-armour.md` has the table of
every target, the variants tried and the reasons. Three data values changed
(`ai.mix_at_pct` 10 -> 20, `ai.assault.at_hunt_clear_m` 40 -> 20,
`ai.assault.satchel_hunt_m` 150 -> 250), each with its measurement in the
TOML comment. About 20 skirmish runs of 40-160 matches (40 s per 40
matches); armour-test and fortify-test twice (both identical to Part B,
since AI values do not enter the staged cases).

- Hit: T1 (160 matches: G 45 % / U 55 %, base 0 55 %), T6, T7, T8, T9,
  T10, T11 (+10 %), T12, T13 (documented).
- Missed: T2 (9.7 % vs 20 %; was 5.3 %), T3 (5 of 80 normal-base matches;
  was 1 of 40), T4 (75 % vs 80 %; was 55 %), T5 (1 low-ammo return in 160;
  tanks are lost before they run low), T14's HE-share limit (1 match of
  160 at 65.5 %; mean 43.8 %). Reasons and proposed target changes:
  TODO S3c-28 to S3c-31.
- `rearm_ap_below` 8 gave results identical to 3, so it stays 3.
- Tests: `ai_assault` moved from seed 18 to seed 50 (the only seed of 1-60
  that has rockets, a satchel hunt, second bases, re-crews, crew retreats
  and denial in ten minutes under the new data); the content override test
  expects the new `satchel_hunt_m` and `at_hunt_clear_m`.
  `cargo test --workspace --release -- --include-ignored`: 456 passed, 0
  failed. clippy -D warnings, fmt, validate ok.
- Standard set (40 matches) alone gave G 11 / U 29; the 160-match set gave
  72 / 88. Use 160 for decisions on faction wins.

## Fix and close (2026-11-02)

Two reviews of `090c4a9` (determinism-correctness, balance-play). Every
confirmed medium finding and the cheap low ones are fixed; the rest are
recorded as owner questions or known gaps below.

### What changed and why

- **No re-crew of a burning or useless tank** (correctness review 1,
  medium). Nothing on the player path checked for fire: a crew could board
  a burning tank and it was abandoned again the next tick (one tank counted
  as two losses, the men getting out twice). `bail::board_blocked` now
  refuses `Recrew` (refusals `Burning`, `Useless`) for a tank that burns
  (engine or ammunition) or has gun and mobility out under `useless_bail`,
  and boarding waits while either holds. The AI already avoided both.
- **Capture needs a tank crew** (balance review 1, medium). New rule
  `[vehicles.recrew] capture_needs_crew = 1` (TODO S3c-32 (A)): only a crew
  squad may capture an enemy's vehicle; infantry still re-crew their own
  side's (refusal "only a tank crew can capture it"). Reason in the TOML:
  an enemy tank's controls, sights and ammunition are unfamiliar, and
  captured tanks were taken into service by trained crews (German
  Beutepanzer units, US use of captured Panthers), not by riflemen in the
  field. It removes the free 620 MP tank a human could take with three
  riflemen while the AI never captures. Abandoned tanks still never
  expire (S3c-32 (C), not done: no historical basis).
- **Exits clear of other hulls** (correctness 2, low). A bailing man was
  placed beside his hull without checking other hulls and could stand
  inside a neighbour's. `get_out` now tries the far side, the other side,
  ahead and behind, and takes the first spot `exit_clear_cm` clear of
  every other hull (live, abandoned, wreck), else the one farthest from
  them.
- **A boarding squad keeps its place** (correctness 3, low): a lower-id
  squad arriving later no longer resets its progress.
- **Limit or pop reached while boarding** (correctness 4, low): refused
  once with `VehicleLimit` / `PopCap` in the snapshot and the squad goes
  `Idle` (was a silent, endless wait). Status-stage refusals now reach the
  snapshot (`World::fill_snapshot` drains them; before, only refusals from
  commands did).
- **Tests** (correctness 5, low): the determinism script now really has a
  capture (riflemen next to tank A after its crew walks off) and a re-crew
  (tank C by its crew), asserts both, and adds a save/load while boarding;
  the rearm test checks every MG has rounds again and that the tank leaves
  the base afterwards. New tests: burning and useless refusals, exits
  beside a neighbouring tank, the limit filling while boarding, capture
  refused without a crew. armour-test case 15 f checks capture with
  `capture_needs_crew` off and the crew-only refusal.
- **Feed line** (balance 4, low): "Panzer IV abandoned (ammo fire): 4 of 5
  bailing out." (was cut off at 1600x900).
- **Docs** (balance 2 and 5, low): T10 is quoted from a `--jobs 3` run
  (spec, `docs/modding/tuning.md`); the `min_men` comment says the
  effective minimum is max(`min_men`, `crew_below`) = 3.

Rejected or deferred, with reasons:

- Balance 3 (2.8 crew deaths per tank loss vs ~1 historically): not tuned.
  The deaths come from the penetrating rounds, spall and rack detonations
  that touch the men; `crew_below = 4` only moved men alive at bail-out
  from 3.0 to 3.26 and added 9 % tank losses. TODO S3c-35 (A): revisit
  with the spall model.
- Balance 6 (abandoned enemy tanks shown through fog): kept as specified
  ("like a wreck"). TODO S3c-34 (A).
- Balance 4, second half (the abandoned marker floats ~90 px above the
  hull): cosmetic, left as a known gap.
- AI capture (S3c-2 (C)): TODO S3c-33 (A) S4, behind a flag.

### Final targets

Skirmish: seeds 1-20 x both bases (standard 40) and seeds 1-80 x both
bases (160, `--jobs 3`), `--minutes 45`, `--threads 1`. Every match line
of the 160 set (winner, length, losses, rockets, satchels, HE share) is
identical to the review's run of `090c4a9`; only the hashes differ, since
the match setup (with the new rule field) is part of the state hash. The
fixes change player paths and edge cases the AI does not reach.

| # | Target | Final | Status |
|---|---|---|---|
| T1 | each faction 35-65 %; no base side > 62 % | 160: G 72 (45 %) / U 88, base 0 88 / base 1 72 (55 %); standard 40: G 11 / U 29, base 0 21 / base 1 19 | met on 160; the 40 set misses on faction wins (seed block 1-20 is an outlier: blocks 21-40, 41-60, 61-80 give G 22/21/18 of 40) |
| T2 | rockets + satchels >= 20 % of tank losses | 70 of 725 (9.7 %) | **missed** (S3c-28 (A): target 10 %) |
| T3 | satchel at a vehicle in >= 6 of 20 normal-base matches | 5 of 80 normal-base matches | **missed** (S3c-30) |
| T4 | rockets fired in >= 16 of 20 normal-base matches | 60 of 80 (75 %); 40 set 14 of 20 | **missed**, narrowly |
| T5 | >= 1 low-ammo base return per 10 matches | 5 trips (AI rearms 4 G / 1 U), 1 low-ammo return in 160; MG dry 0.0 %, HE at reserve 0.1-0.2 % | **missed** (S3c-30): tanks are lost before they run low |
| T6 | bailed crews: a man out in >= 70 %; >= 50 % alive 60 s later | 99 % / 100 % with a man out; 80 % (G) / 82 % (U) alive after 60 s | met |
| T7 | AI re-crews >= 1 per 10 matches | 222 orders, 56 tanks re-crewed (G 19, U 37) | met |
| T8 | US attackers <= 60 %; defenders >= 75 % | two bases of fire 37.5 %; one base 66.2 %; Germany attacking 36.2 % (51.2 % with two bases); defenders 90-100 % | met (two bases, the AI's habit when its MG is weaker) |
| T9 | match 10-40 min, median 15-30 | 10.5-31.7 min, mean 17.8 | met |
| T10 | tick p99 at peak < 1.5 ms; stress p99 < 10 ms | 0.997 ms (160, `--jobs 3`); 40 set at `--jobs 6` 0.688 ms; stress p99 1.399 ms | met (`--jobs 6` on 160 gave 1.546 ms in review: load-dependent) |
| T11 | tanks lost by base side within +-15 % | 345 vs 380 (+10 %) | met |
| T12 | 600 m duel reported | 3f: Panzer IV 14, Sherman 6 of 20; 3a 300 m: Panzer IV 6, Sherman 12, none 2 | met (report) |
| T13 | T9 kills > 0 / T14 >= 0.05 m with a physical cause | T9 kills 4 (fragments over a falling edge); lip 0.035 m per burst | T9 met; T14 lip documented (S3c-22, S3c-23) |
| T14 | hard checks pass; HE share < 60 % per match | armour-test all hard checks pass; fortify PASS; golden 46/46; validate ok; HE mean 43.8 %, over 60 % in 1 of 160 (65.5 %) | hard checks met; HE limit missed by 1 match (S3c-31) |
| T15 | determinism | 459 passed, 0 failed (incl. soaks, net loopback, 1/2/3/8 threads, save/load) | met |

### Tests (Fix stage)

- `cargo fmt --check`, `cargo clippy --workspace --all-targets --release
  -D warnings`: clean.
- `CARGO_BUILD_JOBS=4 cargo test --workspace --release --
  --include-ignored`: 459 passed, 0 failed (456 + 3 new; one new test was
  folded into the existing capture test).
- `ballistics --golden mods/ww2/tests/ballistics.toml`: 46 rows, 0 failed.
  `validate mods/ww2`: ok.
- armour-test `--stress --seeds 10`: all hard checks pass (case 15 7 of 7
  checks, 20 of 20 runs; the first full run failed 15 f because riflemen
  were refused under the new default; the case now sets the flag
  explicitly and checks the refusal).
- fortify-test `--seeds 10`: PASS.
- Godot (bridge rebuilt): `check_project` OK, `test_match_view` 215/215,
  `test_net_ui` 143/143, `test_data_reload` 8/8.
- `--autoplay 10` vs AI at 4x: no script errors; 2 own Panzer IVs, 1 enemy
  tank seen, 1 abandonment with 3 men out, 18 shots fired; AI won at 10:00.
  Exit 3 only for the known S1b frag-shot miss (`s1b_frag_tool`,
  `s1b_frag_trench`). Screenshots looked at (tank, battle, end): HUD, call-in
  panel, Crew (K) button and vehicle cards render normally.
- `settings.cfg` unchanged (md5 5755cc78... equals the backup); no Godot
  process left running.

### Open owner questions (all on (A))

S3c-28 (T2 target 10 %), S3c-29 (Panzerfaust in S4), S3c-30 (reword T3
and T5), S3c-31 (T14 HE limit as mean < 50 % and < 60 % in 99 %), S3c-32
(capture needs a crew: done on (A)), S3c-33 (AI capture in S4, flagged),
S3c-34 (abandoned enemy tanks visible map-wide: kept), S3c-35 (crew deaths
per loss: accept, revisit with spall), plus the earlier S3c-1..27 in
`docs/TODO.md`.

### Known gaps

- T2, T3, T4, T5 and T14's HE limit miss the written targets (above).
- Capture is unmeasured in skirmish: the AI never captures (S3c-33).
- 208 of 629 abandoned tanks still stand at match end (160 matches);
  they never expire.
- Crews lose about 2.8 men per tank loss (historical ~1).
- The abandoned marker floats about 90 px above the hull.
- No full autoplay screenshot of an abandoned tank (autoplay triggers are
  fixed; the staged `armour_bailout` scene covers it).
- T10 depends on parallel load; quote it from `--jobs 3`.

### What the next stage (S4) must know

- `RecrewRefusal` has three new variants (`Burning`, `Useless`,
  `CaptureNeedsCrew`); the bridge's `recrew_words` names them.
- `[vehicles.recrew] capture_needs_crew` is a required rules key; mods and
  overrides that copy `[vehicles.recrew]` need it.
- SAVE_VERSION 11 / PROTOCOL_VERSION 5 unchanged by the Fix stage (no
  state or command layout change).
- Use 160 matches (seeds 1-80 x both bases) for faction-win decisions.
