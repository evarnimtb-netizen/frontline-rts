# S9b log

Branch `s9b-ai-guns`, from main `71cf02f`. Commits: `8c7ce14` (gun_armour_out measure), `a9fa303` (corridor siting),
`8aad271` (Fix). Not pushed, not merged. Spec `S9b-ai-guns.md`; targets `S9b.toml`. `SAVE_VERSION` 20 -> 21 once
(`8aad271`, saved `[ai.guns]` layout and `AiGun::point`). `PROTOCOL_VERSION` 12.

**Status:** siting built and reviewed. Budget still 35, chosen on the pre-Fix count. On the corrected count T88 misses.
The full suite FAILS on `8aad271` (one test). Not ready to merge.

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
seeds 1-40, both bases). `8aad271` changed only the T88 count and the all-enemy-points fallback.

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

480 matches pooled per row (`tools/measure.sh`, `[ai.guns]` `income_max_pct` override). Runs on `a9fa303`: **pre-Fix count**.

| income_max_pct | pool GER % | sets 160 / oos / s201 | fired_min | cover_min | out GER | out USA | hard misses | gun MP GER / USA |
|---|---|---|---|---|---|---|---|---|
| 35 (in TOML) | 50.21 | 51.88 / 53.12 / 45.62 | 82.5 | 83.7 | 0.565 | 0.452 | 15 | 18.4 / 19.9 |
| 30 | 49.38 | 53.75 / 46.25 / 48.12 | 73.8 | 87.5 | 0.623 | 0.404 | 11 | 16.3 / 17.4 |
| 25 | 52.08 | 57.50 / 50.62 / 48.12 | 40.0 | 85.6 | 0.292 | 0.425 | 14 | 13.5 / 14.1 |
| 15 | 49.58 | 48.75 / 48.12 / 51.88 | 21.2 | 84.0 | 0.115 | 0.073 | 9 | 7.5 / 6.7 |

Chosen: 35, unchanged. On the pre-Fix count it was the only tried value that held every stage constraint (pool 45-55,
no match without a winner, fired and cover at least 60, out GER at least 0.55, out USA at least 0.45). Lower values cut
gun MP and lift half-tracks, but the guns stop firing (fired_min 40 at 25, 21 at 15). **On the corrected count (35),
out GER is 0.542 and out USA 0.398: both constraints now fail.** 30 is not re-measured on the corrected count (TODO S9b-1).

## Final pooled reading and target counts

Latest measurement: `8aad271`, overrides none, `s9b/m3/measure.txt` (40-set cached, 160 sets run; 480 matches).

- Germany pooled **50.00 %** (sets 51.88 / 52.50 / 45.62): pass (band 45-55). No match without a winner: pass.
- Fired GER 84 / 86 / 88 %, USA 85 / 86 / 82 % (floor 60); in cover GER 88 / 90 / 84, USA 87 / 86 / 83 (floor 60); gun MP
  USA 19.9 % (cap 20, at the cap): all pass. Armour out by guns per match (corrected): GER 0.54 mean, USA 0.40 mean.
  Tank KOs by guns: GER 42 (0.087 per match), USA 28 (0.058).
- Targets: **167 pass, 18 miss (hard 17), 86 skipped** (T8d is report-only).

Hard misses left, each with its cause (spec section 6):

| Target | Value | Cause |
|---|---|---|
| T8a/b HE share floor 20 % | 19.21 / 19.15 | Below in every run since before S9b. Not a gun-AI value; siting moved it about 0.6, within the set spread. |
| T9b USA mortar kills floor 10 % | 9.91 | On the floor at baseline (10.0). Set spread. |
| T31a / T31b vehicle kills cap 30 % | 30.80 / 31.71 | Gun budget share. At 30 (pre-Fix count) T31a clears at 29.3, T31b does not (30.6). |
| T40a-d, T72a/b half-tracks bought floor 60 % | 53.1 / 42.5 (GER / USA, sets) | The gun set-aside competes with them; still missed for USA at 30. |
| T54e StuG 80 %, T54f M10 75 % | 75.0 / 69.4 | Same lever; not reached at 30. |
| T55a Sd.Kfz. 234 kill share cap 10 % | 11.32 | Follows the gun budget (11.3 at 35, 9.6 at 30). Mechanism not isolated. |
| T88a/b/d armour out floors 0.55 GER, 0.45 USA | 0.50 / 0.394 / 0.444 | Pooled 480: GER 0.542, USA 0.398. The sight gap (see Known gaps) limits it. |

Spec section 6 is NOT met (T88 misses both sides); T80, T81, T83 and the Germany and no-winner gates pass.

## Review findings and fixes (review on `a9fa303`, fixes on `8aad271`)

- **F1 MEDIUM, fixed.** Setup layout changed with SAVE 20. Bumped to 21 (spec section 4 read as applying; orchestrator
  to confirm). Tests: `a_save_of_the_previous_version_is_refused`, `the_gun_site_keys_are_saved_hashed_and_checked_on_load`.
- **F2 MEDIUM, fixed.** Repeat losses counted (about +3 % GER, +10-12 % USA in 120 traced matches). Now one per vehicle
  per match per attacker. Tests: `a_vehicle_lost_twice_to_guns_counts_once`, `each_side_counts_a_vehicle_for_itself`.
- **F3 MEDIUM, half fixed.** Causes written above. The 35-versus-30 decision was NOT repeated on the corrected count.
- **F4 LOW, fixed.** Every route ends at the point, so rings 100 and 125 m were always cut by `corridor_clear_m`.
- **F5 LOW, fixed.** All points enemy-held: no search, the gun stays put. Test: `with_every_point_enemy_held_a_gun_has_no_point_to_cover`.
- **F6 LOW, fixed.** Dead `AiGun::point` removed in the same bump.
- **F7 LOW, not done.** Site values `site_view_min_pct` 10, `site_forward_m` 0, `corridor_clear_m` 150,
  `site_enemy_point_m` 250 were chosen on 40/80-match traces. No 480-match ablation (about 14 minutes each).

## Checks that ran

- `tools/check.sh quick 71cf02f`: `a9fa303` 556 passed; `8aad271` 563 passed, all ok.
- Whole suite with ignored tests (`tools/check.sh tests`): `a9fa303` 681 passed, 0 failed. `8aad271` **687 passed, 1 failed**:
  `ai_guns_resite_with_the_tow_and_survive_a_save` panics at `crates/sim/tests/ai_guns.rs:105`, `Save("a soldier's rounds
  are inconsistent")`. It failed in both runs in `target/check/test_all.log`. Not in quick (ignored). A regression on the
  committed tree; not diagnosed in this docs stage.
- `--test ai_guns -- --include-ignored` on `a9fa303`: 2 passed. Measure: base, m1, b15, b25, b30 (`a9fa303`); m3 (`8aad271`).
- Not run: `tools/check.sh full`, the 40-hash cross-check, tick timing (rule 4).

## Known gaps

- T88 below its bar on the corrected count; budget not re-chosen; 30 not measured on the corrected count. The full suite fails on `8aad271` (blocks merge).
- F7 ablations not done. No direct test for the no-sight-line fallback; unit tests pin `site_view_min_pct` 50, not 10.
- No real old-save load test (rule 7). Tick p99 not measured. Sight gap: "in reach" 17.6 % (GER) / 21.9 % (USA) of ready
  gun time against "in sight" 1.3 % / 1.8 %.

## What the next slice must know

- `site_view_min_pct` is the sensitive knob (10 vs 25: 24 of 166 vs 61 of 180 guns lost on traces). US gun MP is at cap 20.
- The CLI binary changed: `batch.sh` and `measure.sh` caches miss for old results; hash lines differ across `a9fa303` and `8aad271`.

Models: Build, Review, Fix on Opus; Measure and Budget on Sonnet; Docs on Haiku; orchestration and diagnosis on Fable.
