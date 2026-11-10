# S9b log

Branch `s9b-ai-guns`, from main `71cf02f`. Commits: `8c7ce14` (gun_armour_out measure), `a9fa303` (corridor siting),
`8aad271` (Fix), `508f6fa` (docs), `7e33e0e` (launcher save fix), `968baa0` (tools only), `1bfc92f` (`income_max_pct` 30,
head). Pushed to the private remote, not merged. Spec `S9b-ai-guns.md`; targets `S9b.toml`. `SAVE_VERSION` 20 -> 21 once
(`8aad271`, saved `[ai.guns]` layout and `AiGun::point`). `PROTOCOL_VERSION` 12.

**Status:** siting built, reviewed and verified on `1bfc92f`. Budget 30 (owner question S9b-1). T88 met for Germany,
not for USA. Not merged.

## Diagnosis (spec section 2)

S9 sites were cover-first, on one straight line toward the enemy base. Armour was in reach and arc of a ready gun for
13-19 % of ready gun time but in sight for 0.6 % (GER) and 2.4 % (USA). Sites within 200 m of an enemy-held point lost
44 of 52 guns. After a sight line the armour was nearly always lost, so the sight line is the bottleneck. The S9 counter
counted wrecks only; 88 % of armour losses are bail-outs.

## What was built and why

- **Measure (`8c7ce14`, CLI only).** `gun_armour_out_per_match` (armour knocked out or abandoned by a served gun's last round; T88).
- **Corridor siting (`a9fa303`, `ai/guns.rs`).** Corridor = vehicle path from the two nearest seen armour (else the enemy
  base) to the point, sampled every 25 m over 500 m. Candidates: 7 rings x 16 bearings. Filters: enemy men, other gun,
  enemy-held points (`site_enemy_point_m`, new key), corridor clearance. Score: corridor samples in sight inside the arc;
  sites under `site_view_min_pct` of the best dropped; then cover, count, nearest. Searches only in Site.
- **Fix (`8aad271`).** F1 save bump 21; F2 armour-out counts each vehicle once per match per attacker; F4 `site_min_m`
  150 and `site_rings` 5 (the rings that survived); F5 no point to cover when all are enemy-held; F6 `AiGun::point` removed.

## Gun funnel, before and after

Share of enemy armour fielded. Before: S9 build (spec section 2). After: `a9fa303`, 80 matches (`tools/gun_trace.sh`,
seeds 1-40, both bases), at `income_max_pct` 35. `8aad271` changed only the T88 count and the all-enemy-points fallback.

| Step | GER before | GER after | USA before | USA after |
|---|---|---|---|---|
| within 400 m of a ready gun | 57 % | 60.9 % | 58 % | 64.1 % |
| inside its arc | 55 % | 59.6 % | 56 % | 61.7 % |
| clear sight line | 19 % | 34.4 % | 24 % | 29.9 % |
| fired at | 24 % | 33.8 % | 28 % | 32.3 % |
| struck / penetrated | 22 / 22 % | 28.5 / 27.8 % | 21 / 19 % | 28.7 / 28.7 % |
| lost to a gun | 22 % (0.41/match) | 25.8 % (0.49/match) | 14 % (0.33/match) | 19.2 % (0.40/match) |

Ready gun time per match 15.3 -> 14.7 gun-min (GER), 13.4 -> 12.6 (USA). Ready time with armour in sight: GER 0.6 -> 1.3 %,
USA 2.4 -> 1.8 % (the USA time share fell while the share of armour seen rose). Own guns lost per match (knocked out +
left): GER 0.26 + 0.25, USA 0.20 + 0.17; at the first siting attempt 0.57 + 0.50 and 0.55 + 0.60.

## Budget table and the value chosen

### Old table, pre-Fix count (superseded)

480 matches pooled per row (`tools/measure.sh`, `[ai.guns]` `income_max_pct` override). Runs on `a9fa303`: **pre-Fix count**.

| income_max_pct | pool GER % | sets 160 / oos / s201 | fired_min | cover_min | out GER | out USA | hard misses | gun MP GER / USA |
|---|---|---|---|---|---|---|---|---|
| 35 (in TOML) | 50.21 | 51.88 / 53.12 / 45.62 | 82.5 | 83.7 | 0.565 | 0.452 | 15 | 18.4 / 19.9 |
| 30 | 49.38 | 53.75 / 46.25 / 48.12 | 73.8 | 87.5 | 0.623 | 0.404 | 11 | 16.3 / 17.4 |
| 25 | 52.08 | 57.50 / 50.62 / 48.12 | 40.0 | 85.6 | 0.292 | 0.425 | 14 | 13.5 / 14.1 |
| 15 | 49.58 | 48.75 / 48.12 / 51.88 | 21.2 | 84.0 | 0.115 | 0.073 | 9 | 7.5 / 6.7 |

On the pre-Fix count 35 was the only tried value that held every stage constraint (pool 45-55, no match without a
winner, fired and cover at least 60, out GER at least 0.55, out USA at least 0.45). Lower values cut gun MP and lifted
half-tracks, but the guns stop firing (fired_min 40 at 25, 21 at 15).

### Corrected count (`968baa0`, the basis of the choice)

480 matches each, `tools/measure.sh`, `income_max_pct` override on build `968baa0`.

| income_max_pct | pool GER % | sets 160 / oos / s201 | fired_min | cover_min | out GER | out USA | hard misses | gun MP GER / USA | half-tracks bought GER / USA |
|---|---|---|---|---|---|---|---|---|---|
| 35 | 50.00 | 51.88 / 52.50 / 45.62 | 82.5 | 83.2 | 0.542 (0.50 / 0.57 / 0.55) | 0.398 (0.39 / 0.44 / 0.36) | 17 | 18.3 / 19.9 | 50 % / 41 % |
| 30 | 48.75 | 52.50 / 46.25 / 47.50 | 73.8 | 86.7 | 0.575 (0.58 / 0.56 / 0.59) | 0.356 (0.38 / 0.34 / 0.36) | 10 | 16.2 / 17.3 | 57 % / 52 % |

**Chosen: 30** (commit `1bfc92f`). 35 passes neither T88 side on the corrected count, so the rule that picked 35 on the
pre-Fix count no longer applies. 30 has 7 fewer hard misses, the half-tracks recover (the slice's purpose), and German
guns reach T88. The price: US guns fall further below T88 (0.356 against 0.45). Open as owner question S9b-1.

## Final reading and target counts (verified on `1bfc92f`)

Verified on `1bfc92f` with `income_max_pct` 30 (`tools/verify.sh`, 33 min). 480 matches.

- Germany pooled **48.75 %** (sets 52.50 / 46.25 / 47.50): pass (band 45-55). No match without a winner: pass.
- Fired in matches GER 79 / 77 / 82 %, USA 74 / 74 / 82 % (floor 60). In cover GER 91 / 91 / 87, USA 95 / 88 / 88 (floor 60).
  Gun MP GER 16.0 / 16.4 / 16.2, USA 17.3 / 17.3 / 17.4 (cap 20). Vehicle kills GER 29.5 / 29.7 / 28.7, USA 30.5 / 29.3 / 31.2 (cap 30).
- Armour out by guns per match (corrected, distinct vehicles): GER **0.575** (0.58 / 0.56 / 0.59), USA **0.356** (0.38 / 0.34 / 0.36).
  Before S9b: 0.41 GER, 0.33 USA (80-match funnel, the basis of T88). The 480-match old count (0.40 / 0.35) is not like for like.
- Tank wrecks by guns per match (T84c/d, reported beside T88): GER 0.085 (S9 0.075), USA 0.035 (S9 0.033). The wreck count
  barely moved; 88 % of armour losses are bail-outs, and the owner chose armour put out of action as the measure (2026-11-09).
- Targets: **261 pass, 10 miss** (all hard), with the armour, fortify and timing runs included.

Hard misses left, each with its cause (spec section 6):

| Target | Value | Cause |
|---|---|---|
| T8b HE share floor 20 % | 19.07 (oos set; 160 set 20.0, passes) | Below or at the floor in every run since before S9b. Not a gun-AI value. |
| T31b USA vehicle kills cap 30 % | 30.53 | 0.5 over, inside the spread of the sets (29.3-31.2). Germany now passes. |
| T40b/d, T72b USA M3A1 half-track bought floor 60 % | 49.4 / 46.3 / 49.4 | Up from 42.5 / 41.9 at 35, still short. The gun set-aside competes; a card-priority matter. |
| T40c GER Sd.Kfz. 251 bought floor 60 %, oos set | 54.4 | Same lever; the 160 set passes (T40a). |
| T54e StuG floor 80 %, T54f M10 floor 75 % | 76.9 / 70.6 | Up from 75.0 / 69.4. Same lever; card priorities. |
| T88b/d USA armour out floor 0.45 | 0.375 / 0.338 | The sight gap and a smaller budget. US guns see armour for 1.8 % of ready gun time. |

Back in band at 30 compared with S9's list: T8a, T8d, T31a, T40a, T54d, T55b, T72a (T31d too), and T88a/T88c. New at 30
against the S9 baseline: T31b only. Of the 14 hard misses S9 left, 8 remain (T8b, T31b, T40b, T40c, T40d, T54e, T54f,
T72b) and 6 are back in band. T88b and T88d are this slice's own targets.

Spec section 6 status: T88 met for Germany (pooled 0.575, and all three sets) and NOT met for USA. T80, T81, T83, the
pool 45-55 gate, no-winner, determinism and tick time pass.

## Review findings and fixes (review on `a9fa303`, fixes on `8aad271`)

- **F1 MEDIUM, fixed.** Setup layout changed with SAVE 20. Bumped to 21 (spec section 4 read as applying; orchestrator
  to confirm). Tests: `a_save_of_the_previous_version_is_refused`, `the_gun_site_keys_are_saved_hashed_and_checked_on_load`.
- **F2 MEDIUM, fixed.** Repeat losses counted (about +3 % GER, +10-12 % USA in 120 traced matches). Now one per vehicle
  per match per attacker. Tests: `a_vehicle_lost_twice_to_guns_counts_once`, `each_side_counts_a_vehicle_for_itself`.
- **F3 MEDIUM, done.** Causes written above. The 35-versus-30 decision was re-made on the corrected count: 30 chosen.
- **F4 LOW, fixed.** Every route ends at the point, so rings 100 and 125 m were always cut by `corridor_clear_m`.
- **F5 LOW, fixed.** All points enemy-held: no search, the gun stays put. Test: `with_every_point_enemy_held_a_gun_has_no_point_to_cover`.
- **F6 LOW, fixed.** Dead `AiGun::point` removed in the same bump.
- **F7 LOW, not done.** Site values `site_view_min_pct` 10, `site_forward_m` 0, `corridor_clear_m` 150,
  `site_enemy_point_m` 250 were chosen on 40/80-match traces. No 480-match ablation (about 14 minutes each).

## Checks that ran

- `tools/check.sh quick 71cf02f`: `a9fa303` 556 passed; `8aad271` 563 passed, all ok.
- `tools/check.sh tests` on `8aad271`: 687 passed, 1 failed. `ai_guns_resite_with_the_tow_and_survive_a_save` panicked at
  `crates/sim/tests/ai_guns.rs:105` with Save "a soldier's rounds are inconsistent". Cause: a team's assistant who took up a
  launcher kept the rifle's magazine count, on a one-round weapon, so any later save was refused on load. Not a regression
  of S9b: the bug is on main since S4 (`406c2e7`), and the new gun sites only made the test seed reach it. Fixed in
  `crates/sim/src/combat.rs` (`7e33e0e`). Regression check added to `the_next_man_takes_the_launcher_after_the_hand_over_and_reloads_alone`
  in `crates/sim/tests/anti_tank.rs` (it failed before the fix).
- On `1bfc92f`: `tools/check.sh full` all ok. 688 passed, 0 failed; golden, validate, bridge, Godot check_project,
  test_match_view, test_net_ui, test_data_reload, and `settings.cfg` unchanged.
- Tick p99 0.407 ms all / 0.509 ms peak (S9: 0.433 / 0.534; limit +25 %). 40 of 40 hash lines cross-check between the
  jobs-3 and jobs-6 runs. Both in `tools/verify.sh` on `1bfc92f`.

## Known gaps

- T88 met for Germany, not for USA (0.356 against 0.45; S9b-1). Sight gap: "in reach" 17.6 % (GER) / 21.9 % (USA) of ready
  gun time against "in sight" 1.3 % / 1.8 % (`a9fa303` trace).
- F7 ablations not done. No direct test for the no-sight-line fallback; unit tests pin `site_view_min_pct` 50, not 10.
- No real old-save load test (rule 7).

## What the next slice must know

- `site_view_min_pct` is the sensitive knob (10 vs 25: 24 of 166 vs 61 of 180 guns lost on traces).
- Gun MP at 30: GER 16.2, USA 17.3 (cap 20). At 35 USA was at the cap (19.9).
- Main still has the launcher save bug until S9b merges; `7e33e0e` on this branch has the fix.
- The CLI binary changed: `batch.sh` and `measure.sh` caches miss for old results; hash lines differ across `a9fa303` and `8aad271`.
- Owner 2026-11-09: delete target/debug after S9b is verified.

Models: Build, Review, Fix on Opus; Measure and Budget on Sonnet; Docs on Haiku. Orchestration moved from Fable to Opus
mid-slice; the launcher bug was diagnosed and fixed by the orchestrator (Opus).
