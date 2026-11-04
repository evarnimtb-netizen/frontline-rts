# Autotune report: s6_tune2

S6 tune: Germany 45-55 % (S6 baseline 61.9 % on 160), mortar share of kills 10-15 % per side
(baseline 4.9 / 3.6 %), HE share 20-25 % (13.8 %). Knobs: a German card price multiplier (1.00 = shipped, -20 %
cut of S5a-37; 1.25 = no cut), the AI's mortar share of buys (ai.mix.mortar, weight 15 of 140 today) and its rounds per mission. No bomb lethality (S5b-6).

CLI `/Users/evarnimtb/Documents/Claude/code/frontline-rts/target/release/frontline-cli`, `--jobs 6`, 120 min limit. Every batch plays fresh seeds (each seed twice, normal and swapped bases), so every match is an independent observation apart from its swapped twin, which the intervals allow for (cluster-robust). Modelled targets are fitted over all matches of all levels at once: a faction's share by a logistic model in the knobs plus a base-side term, other modelled metrics by least squares. All intervals are 95 %.

Targets:

- `share_germany` 45..55, modelled, weight 1
- `kill_mortar_germany` 10..15, modelled, weight 1
- `kill_mortar_usa` 10..15, modelled, weight 1
- `he_mean` 20..25, modelled, weight 1
- `base_max` -..62, checked near the chosen level
- `len_min` 10..-, checked near the chosen level
- `len_max` -..40, checked near the chosen level
- `len_median` 15..30, checked near the chosen level

## Batches

| # | ger_price_mult | mix_mortar | mission_rounds | seeds | n | share_germany | he_mean | len_median | wall s |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.12 | 32 | 9 | 1001..1020 | 40 | 40.0 [26-55] | 17.8 | 15.9 | 70 |
| 2 | 1 | 32 | 9 | 1021..1040 | 40 | 57.5 [42-71] | 17.8 | 16.7 | 71 |
| 3 | 1.25 | 32 | 9 | 1041..1060 | 40 | 32.5 [20-48] | 17.4 | 16.4 | 61 |
| 4 | 1.12 | 15 | 9 | 1061..1080 | 40 | 47.5 [33-63] | 14.2 | 18.2 | 66 |
| 5 | 1.12 | 50 | 9 | 1081..1100 | 40 | 35.0 [22-50] | 18.6 | 16.8 | 60 |
| 6 | 1.12 | 32 | 6 | 1101..1120 | 40 | 42.5 [29-58] | 16.7 | 17.1 | 59 |
| 7 | 1.12 | 32 | 12 | 1121..1140 | 40 | 30.0 [18-45] | 15.3 | 19.5 | 65 |
| 8 | 1.02 | 50 | 6 | 1141..1160 | 40 | 42.5 [29-58] | 17.8 | 18.0 | 65 |
| 9 | 1.12 | 50 | 6 | 1161..1180 | 40 | 37.5 [24-53] | 19.5 | 17.4 | 60 |
| 10 | 1 | 50 | 6 | 1181..1200 | 40 | 45.0 [31-60] | 17.2 | 17.8 | 75 |
| 11 | 1.14 | 50 | 6 | 1201..1220 | 40 | 37.5 [24-53] | 18.4 | 17.3 | 63 |
| 12 | 1.02 | 50 | 6 | 1221..1240 | 40 | 62.5 [47-76] | 19.0 | 16.7 | 63 |
| 13 | 1.02 | 32 | 6 | 1241..1260 | 40 | 45.0 [31-60] | 17.5 | 16.1 | 59 |
| 14 | 1 | 50 | 6 | 1261..1280 | 40 | 62.5 [47-76] | 18.0 | 17.4 | 72 |
| 15 | 1.04 | 32 | 6 | 1281..1300 | 40 | 50.0 [35-65] | 17.7 | 18.9 | 72 |
| 16 | 1.02 | 50 | 6 | 1301..1320 | 40 | 52.5 [37-67] | 17.8 | 19.4 | 74 |
| 17 | 1.04 | 50 | 9 | 1321..1340 | 40 | 57.5 [42-71] | 17.6 | 19.0 | 76 |

## Per level: observed (pooled) vs model

| ger_price_mult | mix_mortar | mission_rounds | n | share_germany observed | model | kill_mortar_germany observed | model | kill_mortar_usa observed | model | he_mean observed | model |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 32 | 9 | 40 | 57.5 [42-71] | 54.1 [45.4-62.5] | 7.7 | 6.8 [5.9-7.7] | 8.5 | 6.8 [5.6-8.0] | 17.8 | 16.4 [15.4-17.4] |
| 1 | 50 | 6 | 80 | 53.8 [43-64] | 53.8 [47.7-59.8] | 9.4 | 10.1 [9.5-10.6] | 8.1 | 8.5 [8.0-9.1] | 17.6 | 18.3 [17.6-19.0] |
| 1.02 | 32 | 6 | 40 | 45.0 [31-60] | 53.2 [46.1-60.3] | 6.5 | 7.6 [6.5-8.7] | 7.2 | 6.8 [6.2-7.5] | 17.5 | 17.0 [16.1-17.8] |
| 1.02 | 50 | 6 | 120 | 52.5 [44-61] | 51.5 [45.8-57.2] | 9.7 | 10.2 [9.7-10.7] | 8.4 | 8.5 [8.0-9.0] | 18.2 | 18.3 [17.7-19.0] |
| 1.04 | 32 | 6 | 40 | 50.0 [35-65] | 51.0 [44.2-57.7] | 9.6 | 7.7 [6.7-8.7] | 6.5 | 6.8 [6.2-7.4] | 17.7 | 17.0 [16.2-17.9] |
| 1.04 | 50 | 9 | 40 | 57.5 [42-71] | 47.8 [39.9-55.8] | 9.9 | 9.6 [8.9-10.3] | 8.0 | 8.4 [7.4-9.3] | 17.6 | 17.9 [16.9-18.8] |
| 1.12 | 15 | 9 | 40 | 47.5 [33-63] | 42.0 [32.9-51.7] | 4.6 | 5.1 [4.0-6.1] | 4.4 | 5.0 [4.1-5.9] | 14.2 | 15.4 [14.4-16.5] |
| 1.12 | 32 | 6 | 40 | 42.5 [29-58] | 41.9 [34.7-49.4] | 8.3 | 8.2 [7.3-9.1] | 6.7 | 6.6 [6.1-7.2] | 16.7 | 17.3 [16.4-18.2] |
| 1.12 | 32 | 9 | 40 | 40.0 [26-55] | 40.5 [35.3-45.9] | 8.1 | 7.5 [7.0-8.0] | 6.7 | 6.6 [5.9-7.2] | 17.8 | 16.7 [16.1-17.3] |
| 1.12 | 32 | 12 | 40 | 30.0 [18-45] | 39.1 [29.4-49.7] | 6.1 | 6.8 [5.8-7.8] | 5.3 | 6.5 [5.0-7.9] | 15.3 | 16.2 [15.0-17.4] |
| 1.12 | 50 | 6 | 40 | 37.5 [24-53] | 40.2 [33.2-47.6] | 12.0 | 10.8 [10.0-11.5] | 8.8 | 8.3 [7.5-9.1] | 19.5 | 18.6 [17.8-19.5] |
| 1.12 | 50 | 9 | 40 | 35.0 [22-50] | 38.8 [31.6-46.6] | 11.2 | 10.0 [9.2-10.9] | 9.5 | 8.2 [7.3-9.1] | 18.6 | 18.1 [17.2-19.0] |
| 1.14 | 50 | 6 | 40 | 37.5 [24-53] | 38.0 [30.4-46.3] | 10.6 | 10.9 [10.1-11.7] | 7.3 | 8.2 [7.3-9.2] | 18.4 | 18.7 [17.8-19.6] |
| 1.25 | 32 | 9 | 40 | 32.5 [20-48] | 27.3 [19.4-36.8] | 7.4 | 8.2 [7.2-9.3] | 7.1 | 6.3 [5.2-7.4] | 17.4 | 17.1 [15.9-18.3] |

## Fit

- `share_germany` (680 matches): -3.4 [-5.4 to -1.5] points per 0.03 of ger_price_mult; -0.4 [-2.0 to +1.2] points per 4 of mix_mortar; -1.0 [-6.0 to +4.0] points per 2 of mission_rounds; base 0 minus base 1: -15.0 [-22.7 to -7.3] points; lack of fit X2 5.8 on 10 df, p 0.84
- `kill_mortar_germany` (17 batches): +0.2 [-0.0 to +0.4] units per 0.03 of ger_price_mult; +0.6 [+0.4 to +0.8] units per 4 of mix_mortar; -0.5 [-1.0 to +0.1] units per 2 of mission_rounds
- `kill_mortar_usa` (17 batches): -0.1 [-0.3 to +0.2] units per 0.03 of ger_price_mult; +0.4 [+0.2 to +0.5] units per 4 of mix_mortar; -0.0 [-0.7 to +0.6] units per 2 of mission_rounds
- `he_mean` (680 matches): +0.1 [-0.1 to +0.3] units per 0.03 of ger_price_mult; +0.3 [+0.1 to +0.5] units per 4 of mix_mortar; -0.4 [-0.9 to +0.2] units per 2 of mission_rounds

## Solved level

- `share_germany` = 50 at ger_price_mult = 1.033, 95 % interval 0.970 to 1.089 (+-0.059; tolerance +-0.03)
- `share_germany` = 50 at mix_mortar = 25.816, 95 % interval unbounded (slope not clearly non-zero)
- `share_germany` = 50 at mission_rounds = 1.246, 95 % interval unbounded (slope not clearly non-zero)
- `kill_mortar_germany` = 12.5 at ger_price_mult = 1.426, 95 % interval unbounded (slope not clearly non-zero)
- `kill_mortar_germany` = 12.5 at mix_mortar = 64.952, 95 % interval 59.883 to 74.048 (+-7.083; tolerance +-4)
- `kill_mortar_germany` = 12.5 at mission_rounds = -3.001, 95 % interval unbounded (slope not clearly non-zero)
- `kill_mortar_usa` = 12.5 at ger_price_mult = -0.993, 95 % interval unbounded (slope not clearly non-zero)
- `kill_mortar_usa` = 12.5 at mix_mortar = 94.604, 95 % interval 78.918 to 131.229 (+-26.156; tolerance +-4)
- `kill_mortar_usa` = 12.5 at mission_rounds = -161.166, 95 % interval unbounded (slope not clearly non-zero)
- `he_mean` = 22.5 at ger_price_mult = 2.395, 95 % interval unbounded (slope not clearly non-zero)
- `he_mean` = 22.5 at mix_mortar = 103.525, 95 % interval 80.331 to 199.208 (+-59.438; tolerance +-4)
- `he_mean` = 22.5 at mission_rounds = -16.043, 95 % interval unbounded (slope not clearly non-zero)

Best level on the fitted curves: ger_price_mult=1.054, mix_mortar=50, mission_rounds=6; on the grid: **ger_price_mult=1.05, mix_mortar=50, mission_rounds=6**

Override (also in `best_override.toml`; a demonstration until someone ships it):

```toml
[defs.units."ww2:ger_grenadiergruppe"]
cost = 236

[defs.units."ww2:ger_grenadiergruppe_7"]
cost = 168

[defs.units."ww2:ger_smg_gruppe"]
cost = 163

[defs.units."ww2:ger_vg_sturmgruppe"]
cost = 315

[defs.units."ww2:ger_engineer_squad"]
cost = 210

[defs.units."ww2:ger_panzerschreck_team"]
cost = 152

[defs.units."ww2:ger_mortar_team"]
cost = 252

[defs.units."ww2:ger_zugtrupp"]
cost = 168

[defs.units."ww2:ger_munitionstraeger"]
cost = 84

[ai.mix]
mortar = 50

[ai.mortar]
mission_rounds = 6
```

Predicted at the chosen level (model, both bases averaged):

- `share_germany` 45..55: 48.1 [42.6-53.6]: open
- `kill_mortar_germany` 10..15: 10.4 [9.8-10.9]: open
- `kill_mortar_usa` 10..15: 8.4 [7.9-8.9]: out of band (clear)
- `he_mean` 20..25: 18.4 [17.8-19.0]: out of band (clear)

Every target on the 120 matches played within one tolerance of the chosen level (Wilson intervals on shares):

| target | band | value | status |
|---|---|---|---|
| `share_germany` | 45..55 | 52.5 [44-61] | pass (CI overlaps) |
| `kill_mortar_germany` | 10..15 | 9.7 | FAIL |
| `kill_mortar_usa` | 10..15 | 8.4 | FAIL |
| `he_mean` | 20..25 | 18.2 | FAIL |
| `base_max` | -..62 | 52.5 [44-61] | pass (CI inside) |
| `len_min` | 10..- | 10.0 | FAIL |
| `len_max` | -..40 | 30.1 | pass |
| `len_median` | 15..30 | 17.9 | pass |

## Verdict

**Not found.** A modelled target is clearly outside its band at the best level. Widen a range or add a knob you can defend; the tuner will not invent knobs.

Effort: 17 batches, 680 matches (stopped: budget); 1131 s of new match time, 0 batches from cache.
