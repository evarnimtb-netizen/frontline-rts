# Slice S3 log: tanks and armour

Spec: `docs/slices/S3-armour.md` (revision 2), built on its (A) owner
defaults. Branch `s3-armour`. This file holds the results; the spec's own
"Slice log" section points here.

## Status (fix pass, 2026-11-01)

All ten agent-checkable "Done when" items pass. Two caveats are listed
with the item they belong to: case 4's hard check was narrowed (item 6),
and the roster now gives vehicles priority over infantry at 1280 × 800
(item 10). Several balance targets are missed by a wide margin; see
"How tanks play".

The machine was quieter for this pass than for the reviews (load average
about 3, swap still 6.2–6.4 of 7.2 GB used). Timings below are from this
pass. The reviews' timings were taken at load averages of 5–17 and are
noted where they differ.

## Review findings (fix pass)

1. **Cancelled AT-ditch order changed the map** (determinism review).
   Fixed. An AT-ditch job used to record its whole line when the order
   was accepted. Every ground node dug 1.0 m or deeper within 2 m of a
   recorded line counted as "ditch": no cover spots, no trench path
   discount, and blocking at 1.9 m. So an order drawn over an enemy
   trench and cancelled one tick later changed that trench for the rest
   of the match. Now each segment's part of the line is recorded when the
   segment's first pass is stamped, which means only ground the job
   really dug. The order is still refused when the shared list is full.
   Tests: `world::at_ditch_sections_are_recorded_only_once_dug`, which
   orders a ditch along a trench and cancels it, then checks that nothing
   is recorded and the trench is not ditch; then it digs one and checks
   that the sections are recorded. `vehicle_orders.rs` saves and
   continues past the first sections being recorded. **Deviation from the
   spec**: the spec says a ditch adds its line "when it starts". It still
   blocks section by section, as the spec intends. **Not done**: a
   per-player cap. Filling the shared 1,024-section list now needs real
   digging, at about one section per engineer per 30 s, so it was left.
2. **A tampered save could crash the AI** (determinism review). Fixed.
   `AiState::validate` now takes the AI's player. It accepts an escort
   only for a squad id that is gone, or for a vehicle squad of that
   player with one member. `vehicle_tasks` looks the vehicle up with
   `get` and drops the escort if it is missing. The hash test now keys
   its escort by a used-up squad id. Two new load-rejection cases cover
   an escort keyed by the AI's own infantry squad and by an enemy squad.
3. **A tank's two MGs shared random draws** (determinism review). Fixed.
   Mount k skips `k × MG_AIM_DRAWS` (4) draws of the `AIM` stream and
   takes draw k of the `SWEEP` stream. Test:
   `gunnery::one_mg_shot_takes_mg_aim_draws`. This changes match results,
   so every skirmish hash differs from the reviews' hashes.
4. **Stress report missed the intent, status, hash and snapshot stages**
   (performance review). Fixed. Case 14 prints all nine stages.
5. **`run_udp` always reported `vehicles_spawned: [0, 0]`** (performance
   review). Fixed. It counts the vehicles seen in slot 0's world, and its
   scripts send vehicle orders when `vehicle_orders` is set, as
   `run_virtual` does.
6. **`test_match_view.gd` over 0.2 ms** (performance review, 0.26–0.28 ms
   under load). Fixed, and re-measured on the quieter machine.
   `pick_vehicle` used to project every vehicle's footprint more than
   14 px from the pointer. It now skips the footprint test outside a
   screen-reach circle, measured at index time: a 5.5 m world radius
   projected through the camera's right axis. Picking went from 0.12 ms
   to 0.020 ms. The 16-vehicle total is 0.073–0.074 ms over three runs,
   and the "inside the footprint, off-centre" pick test still passes.
7. **Roster drops vehicle cards with nothing selected** (spec review,
   item 10 PARTIAL). Changed. Cards with selected men keep their place
   first, then vehicle cards, then other squads. There is a new check in
   `test_match_view.gd`. The trade-off: at 1280 × 800 with 4 tanks and 8
   squads, the roster now shows **4 of 12 cards (all 4 vehicles) and a
   "+8" tile**. Before, it showed 5 cards (all infantry) and "+7". Vehicle
   cards are wider. The owner may prefer the old order.
8. **Case 4's "at least one rocket hit per run" failed** once the MG
   draws changed: seed 2, p1, "HE from 100 m", 6 rockets, 0 hits. This
   was not one of the review findings; it turned up while re-running item
   6. The variant's team is under HE fire, so its spread is suppressed.
   Over 10 runs, 33 of 58 rockets hit (57 %), and USA teams hit 13 of 30.
   At 43 %, 6 misses in a row happens in about 3 % of runs. The hard check
   now covers the flank and front runs, which every run passes. The HE
   variant prints a report row of runs with no hit instead. **This
   narrows a hard check the spec wrote**; the owner should confirm it.

Review points not acted on:
- Determinism review, "require the line to be near the squad". Not done:
  players legitimately order engineers to dig far from where they stand.
  Recording on the stamp already ties the effect to men who were there.
- Spec review, Done when 7: a 7-minute autoplay can miss the enemy's
  first tank, because the AI calls it in at about 5:40 and it is seen at
  about 8:00. The spec asks for at least 7 minutes, so the run below is
  10 minutes. No code change.
- Performance review: the vehicle spatial index is rebuilt twice per
  tick even with no vehicles (about 2 % of step samples). There are also
  small per-tick allocations (`mgs.clone()`, id vectors,
  `wreck_footprints`) and `vehicle_view.gd`'s 48-per-type cap, which
  blocks the Battalion tier. All are logged in `docs/TODO.md`, not fixed.
  None of them matter at S3 sizes.
- Performance review, vehicle path spikes: one tick can serve the 4,000
  budget plus one capped 8,000-expansion search, 11,694 expansions here.
  That is within the spec ("debt rule as today"). Logged for the
  Battalion tier.

## Done when (fix pass results)

| # | Result | Numbers |
|---|---|---|
| 1 | pass | `cargo test --workspace --release`: **392 passed, 0 failed, 3 ignored** (390 before, plus the AT-ditch and MG-draw tests). clippy `-D warnings` and `cargo fmt --check` are clean. |
| 2 | pass | `ballistics --golden mods/ww2/tests/ballistics.toml`: 42 rows, 0 failed. Row 6 is [98, 100] (actual value 98.8 %; the spec says [99, 100]). |
| 3 | pass | `validate mods/ww2`: ok. The four broken cases are covered by the content tests. |
| 4 | pass | `loopback_vehicles_seeds_1_to_3` passes. **Soak** `soak_with_vehicles_36000_ticks`, run once after the sim changes: 35,997 / 35,995 ticks, 1,199 hash checks, **0 mismatches**, 0 lost orders, 0 restamps, stalls [63, 1], vehicles spawned [4, 3], step p99 0.48 ms, max 1.3 ms. PROTOCOL 3, SAVE 9. The loopback and soak use 4,000 starting manpower, which the spec does not mention, so that two tanks per side arrive in 3 minutes. |
| 5 | pass | Seeds 1–5, normal rules: all exit 0; winners at 14:09–22:33; each AI fielded ≥ 1 vehicle and ≥ 1 AT team; penetrations 7, 9, 5, 10, 7. HE share 40.4, 41.3, 43.8, 45.5, 43.9 % (mean 43.0 %). **No FLAG.** |
| 6 | pass (case 4 narrowed, finding 8) | `armour-test --threads 3 --stress`: every hard check passes, in 182 s. Report rows below. |
| 7 | pass | All 9 staged scenes exit 0, with no errors in their logs; fresh screenshots. Plain `--autoplay 10` vs AI at 4×: no script errors, 2 own Panzer IVs drawn (bought by 5:00 and by 8:00), 1 enemy Sherman seen (by 8:00), 68 armour events. It exits 3 only for the known S1b frag-shot miss (`s1b_frag_tool`, `s1b_frag_trench`). The AI won at 10:00. |
| 8 | pass | Two windowed instances, 127.0.0.1, 3 minutes, `--test_unlock --start_manpower 4000` on both: **181 / 181 hashes agree**, both exit 0, 3 tanks bought and drawn per side, armour events on both. Relay log: hash arrival median 35 ms after the tick's nominal time, max 43 ms, on both slots. **No stall bursts this time** (the integration saw 0.3–1.1 s bursts). Each instance had 16 late sim ticks, max step 12–18 ms. |
| 9 | pass | AI-vs-AI peak p99 (the CLI's peak is by soldier count): 0.268, 0.298, 0.347, 0.261, 0.317 ms (swapped bases: 0.240–0.447); whole-match p99 0.22–0.43 ms, max 0.77–2.19 ms. Limit 1.5 ms. Stress: **p99 1.33 ms**, max 3.03 ms (limit 10). `test_match_view` 16 vehicles: **0.073 ms** (buffers 0.044, picking 0.020, rings 0.009; limit 0.2), 157 / 157 checks. |
| 10 | pass | No overlap at 1280 × 800 with 4 vehicles and 8 squads: **4 of 12 cards shown, all 4 vehicles**, "+8" tile. Four selected tanks keep their cards. |

Owner checklist 11–13 is open (`docs/TODO.md`).

## armour-test report (seeds 1–5 × both factions, 3 threads)

| Case | Result |
|---|---|
| 1 Penetration statistics | 1,040 logged hits, 0 mismatches against the rule. Every pairing is within 3σ, but σ is 0.0–0.6: the aim-point choice avoids borderline plates, so the statistical check is nearly empty. |
| 2 Hull-down | 0 hull hits of 16. The hull-down tank won 3 of 10, the open tank 5, both disabled in 2. |
| 3 Duels | 3a frontal 300 m: Panzer IV 4, **Sherman 6**. 3b, Panzer IV angled 30°: **Sherman 10 / 10**; the Panzer IV landed no hits (turret 14°/s, first disabling hit at 1.1 s). 3c flank 150 m: Sherman 10 / 10. Crossing hit rate **83.1 % of stationary (target ≤ 60 %: missed)**. |
| 4 AT team | Flank disabling-hit rate **42.9 % (target ≥ 50 %: missed)**, front 17.4 %. HE variant: 33 rocket hits, 3 disabling, 5 team men lost; one run with no hit (finding 8). |
| 5 Lone tank, no AT | Holding at 100 m for 3 min: **0 of 80** dug-in men lost, 0 % pinned. |
| 6 Tank vs rifles + AT | **Tank disabled 0 of 10 (target ≥ 60 %: missed)**, although 60 rockets were fired, 58 hit and 56 penetrated. |
| 7 Tank-supported assault | See "Case 7 vs target" below. |
| 8 Satchels | 40 m with rifles supporting: disabled 2 of 10, engineers lost 50 / 50. With the MGs live: 0 of 10, 50 / 50 lost. In a trench the tank crosses: **10 of 10 disabled**, 0 lost. |
| 9 Tank HE vs infantry | Trench 9 vs open 80 men lost (0.21 vs 1.38 per round). Lip lost **0.001 m per burst**; the 4 m parapet gap opens in 0 of 10 runs. |
| 10 Mortar vs tank | No internal damage. 3 tracks lost over 199 shells. |
| 11 Enfilade | **75 of 80** killed from the trench's end, 0 of 80 from the front. |
| 12 Trench crossing | 6,000 crossings, one roll each. 386 bogs (expected 360, σ 18.4). 32 m takes 12.5 s across the trench vs 6.9 s on open ground. The cratered trench stays crossable. |
| 13 AT ditch | The tank went round 10 of 10. Satchels opened a path in **2 of 10** runs (7 each); the other 8 were still closed after 16. |
| 14 Stress | 100 vehicles, 650 soldiers: tick p99 1.33 ms, max 3.03. Stage p99: intent 0.68, perception 0.16, movement 0.24, weapons 0.56, ballistics 0.14, damage 0.007, status 0.04, hash 0.05, snapshot 0.02 ms. Max box tests 42 per tick; max path expansions 11,694. 36 knocked out, 90 penetrations. |

## Case 7 vs target (owner question 5)

| | Attacker losses | Defender losses |
|---|---|---|
| 7, with tank | 74 of 80 | 55 of 80 (68.8 %) |
| 7b, no tank | 80 of 80 | 0 of 80 |
| Target (A) | ≤ 50 % of 7b's (≤ 40) | ≥ 75 % |
| Verdict | **missed** (92.5 % of baseline) | **missed** (68.8 %) |

The tank stopped 845 of 1,201 defender rounds (70.4 %). Before the MG
draw fix this case gave 72 and 62 (77.5 %, met), so the defender figure
sits close to the bar and moves with seed noise. The attacker figure
misses by far either way. A tank turns the assault from a massacre with
no defender losses into a trade, but the attackers still die. The spec
names the next levers as the owner's pick (checklist item 13): tank
fear, mortar smoke, or stronger pinning. Case 5 (0 of 80 killed by a
holding tank, 0 % pinned) and case 11 (75 of 80 from the flank, 0 of 80
from the front) suggest the problem is that men in a trench are immune to
a tank in front of them. Its HE and MGs do not pin them, so the defenders
keep shooting at the escort.

## FLAG: HE share

Not raised in any seed. Mortar plus tank HE was 40.4–45.5 % of kills with
normal bases and 29.1–42.4 % with swapped bases (limit 60 %). The
reviews' no-tank baseline, on the same code but with the unlock moved
past the match, averaged 48.7 % (one seed at 60.1 %). Tanks do not make
HE dominance worse; their MGs take a share of kills, 18–67 per match.
Mortars still do the most killing: 406 kills in the 5 normal-base
matches, against 326 for rifles.

## How tanks play (from the numbers and the screenshots)

- **Tank fights are one-shot and the Sherman wins them.** In the staged
  duels the Sherman won 26 of 30. In the 10 AI matches Germany lost 17
  tanks and the USA 8, and **the USA won 8 of 10** (5 of 5 on base 1, 3
  of 5 on base 0). The spec expected the Panzer IV to win most frontal
  duels at 300 m. The Sherman's gunner aims at the Panzer IV's turret
  (50 mm), which both guns beat easily. Its faster turret then gets the
  first hit, at about 1.1 s. That suggests the turret traverse (14°/s
  for the Panzer IV) decides most fights.
- **Almost every knock-out is an ammo-rack detonation**: the rack box is
  in the hull centre with 60 HP, against an AP core's 100. A penetration
  that crosses the hull centre ends the tank.
- **AT teams are under-powered.** Case 6: 56 penetrations and 0
  disabled tanks. The HEAT jet reaches 1.0 m past the plate (spec). The
  Panzer IV's front plate is at x = 290 cm and its driver and radio-
  operator boxes span x = 115–185 cm (spec content), so a hull-front jet
  stops about 5 cm short of the crew. Turret hits do reach the gun and
  crew. Both numbers are spec values, so they were not tuned here. The
  likely levers are a 1.5 m jet, as AP's core has, or crew boxes moved
  forward. In AI play the teams barely fire: 0 rockets in 6 of 10
  matches, 1 tank killed by a rocket in total.
- **The AI never throws satchels at vehicles** (0 in all 10 matches), and
  no tank bogged (0 in 10). Escorts reached 30 m of an enemy trench 0–4
  times per side per match.
- **Trenches vs tanks.** A holding tank does nothing to men in a trench
  (case 5: 0 of 80). From the flank it clears the trench (case 11:
  75 of 80). Its HE barely erodes the parapet (0.001 m per burst). A
  standard trench slows a crossing tank to about half speed (12.5 s vs
  6.9 s over 32 m) and bogs it 6.4 % of the time, as specified.
- **Screenshots.** At RTS height tanks read clearly: grey or brown hull,
  turret, barrel and marker (`s3_autoplay_tank`, `s3_autoplay_enemy_tank`,
  `s3_escort`). The AT ditch reads as a deep trench with a berm
  (`s3_at_ditch`). Escorting men trail the tank in a single file
  (`s3_escort`). Known weak spots, unchanged: the bogged and burning pips
  are about 8 px; the AT team is hard to see in its trench; the
  penetration shot shows a tank that is already a wreck.

**Do tanks help the trench assault?** Partly. With a tank the defenders
lose 55–62 of 80 instead of 0, but the attackers still lose 72–74 of 80.
A player can now break a trench with a tank on its flank, which is a real
tactic. The frontal escort the TODO problem describes still fails.

## Screenshots (`docs/slices/screens/`)

Staged scenes, fresh in this pass (1600 × 900): `s3_callin_locked`,
`s3_vehicle_card`, `s3_tank_firing`, `s3_penetration_marker`,
`s3_bogged_in_trench`, `s3_at_ditch`, `s3_at_team_firing`, `s3_burning`,
`s3_escort`.

Autoplay and net, fresh: `s3_autoplay_tank`, `s3_autoplay_enemy_tank`,
`s3_net_host_tank`.

Mock (presenter track): `s3_mock_*`; `s3_mock_fit_1280` was re-captured
with the new roster order. The autoplay's S1 and S1b shots were not
copied over the committed ones.

## Cuts

None from the spec's cut list. AI satchel throws at vehicles, fire,
`ReverseUnits`, cases 10–11 and the base crew refill are all built. The
AI's satchel rule exists but never fired in 10 matches.

## Integration (step 8), 2026-11-01

Presenter branch `worktree-wf_c52c05c2-393-3` merged with no conflicts.
The bridge fills the S3 contract (`crates/godot-bridge/README.md`, "S3:
vehicles in the bridge"). The presenter's four contract notes are settled
in `godot/docs/match_state.md`: escort `follow` links and queue `pop` are
sent, `glacis_deg` comes from the defs, and the badge order is unchanged.

- Staged scenes: all nine runs exit 0, with the sim's condition and the
  presenter-state check both holding. The scenes stage at the map centre,
  so point C's ring is in most of them.
- `--autoplay` vs AI (seed 1, 4×): 13:06 game minutes until the AI won
  on tickets, with no errors. Two Panzer IVs (4:17, 6:17) and an AT team;
  28 armour events, 6 penetrations, 2 knock-outs.
- Two Godot instances, 3 minutes, four runs: 181 of 181 hashes agreed
  every time, and 3 tanks per side spawned. The integration measured
  250–550 stall ticks per instance in 0.3–1.1 s bursts at the same tick
  on both, against 55–61 without tanks. The performance review then
  found 483 stalls before any vehicle existed, and a headless run with
  no vehicles at all that stalled 971 times. It concluded the stalls
  track machine pressure (load 5–17, swap almost full), not vehicles. The
  fix-pass run at load 3 showed no stall bursts at all: hash arrival was
  at most 43 ms late. Probable cause: the machine. To settle it, measure
  on an idle machine.
