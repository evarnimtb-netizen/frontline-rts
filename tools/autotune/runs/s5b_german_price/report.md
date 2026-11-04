# Autotune report: s5b_german_price

Demonstration on the current game (S5b): one multiplier on today's seven German infantry-card costs
(the tank card left out, as in the S5a demo). 1.00 = the shipped prices. Target Germany 45-55 %. Not shipped.

CLI `/Users/evarnimtb/Documents/Claude/code/frontline-rts/.claude/worktrees/tuner-fix/target/bench-release/frontline-cli`, `--jobs 6`, 45 min limit. Every batch plays fresh seeds (each seed twice, normal and swapped bases), so every match is an independent observation apart from its swapped twin, which the intervals allow for (cluster-robust). Modelled targets are fitted over all matches of all levels at once: a faction's share by a logistic model in the knobs plus a base-side term, other modelled metrics by least squares. All intervals are 95 %.

Targets:

- `share_germany` 45..55, modelled, weight 1
- `base_max` -..62, checked near the chosen level
- `len_min` 10..-, checked near the chosen level
- `len_max` -..40, checked near the chosen level
- `len_median` 15..30, checked near the chosen level
- `he_mean` -..-, checked near the chosen level
- `kill_mortar_germany` -..-, checked near the chosen level
- `kill_mortar_usa` -..-, checked near the chosen level

## Batches

| # | ger_price_mult | seeds | n | share_germany | he_mean | len_median | wall s |
|---|---|---|---|---|---|---|---|---|
| 1 | 0.85 | 1001..1020 | 40 | 67.5 [52-80] | 14.1 | 16.5 | cache |
| 2 | 1.15 | 1021..1040 | 40 | 27.5 [16-43] | 14.5 | 17.9 | cache |
| 3 | 1 | 1041..1060 | 40 | 42.5 [29-58] | 14.2 | 17.4 | cache |
| 4 | 0.97 | 1061..1080 | 40 | 40.0 [26-55] | 14.4 | 18.0 | cache |
| 5 | 0.85 | 1081..1100 | 40 | 60.0 [45-74] | 15.9 | 16.1 | cache |
| 6 | 0.94 | 1101..1120 | 40 | 57.5 [42-71] | 15.0 | 17.4 | cache |
| 7 | 1.07 | 1121..1140 | 40 | 40.0 [26-55] | 14.2 | 17.3 | cache |
| 8 | 0.96 | 1141..1160 | 40 | 42.5 [29-58] | 14.9 | 16.6 | cache |
| 9 | 0.85 | 1161..1180 | 40 | 67.5 [52-80] | 14.0 | 16.9 | cache |
| 10 | 0.95 | 1181..1200 | 40 | 55.0 [40-69] | 15.1 | 18.3 | cache |
| 11 | 1.07 | 1201..1220 | 40 | 42.5 [29-58] | 14.0 | 16.2 | cache |
| 12 | 0.96 | 1221..1240 | 40 | 45.0 [31-60] | 14.4 | 16.1 | cache |

## Per level: observed (pooled) vs model

| ger_price_mult | n | share_germany observed | model |
|---|---|---|---|
| 0.85 | 120 | 65.0 [56-73] | 63.3 [55.3-70.6] |
| 0.94 | 40 | 57.5 [42-71] | 52.4 [47.4-57.3] |
| 0.95 | 40 | 55.0 [40-69] | 51.1 [46.3-55.9] |
| 0.96 | 80 | 43.8 [33-55] | 49.9 [45.2-54.6] |
| 0.97 | 40 | 40.0 [26-55] | 48.6 [44.0-53.3] |
| 1 | 40 | 42.5 [29-58] | 44.9 [40.0-49.9] |
| 1.07 | 80 | 41.2 [31-52] | 36.5 [29.9-43.7] |
| 1.15 | 40 | 27.5 [16-43] | 27.8 [19.6-37.8] |

## Fit

- `share_germany` (480 matches): -3.1 [-4.6 to -1.7] points per 0.025 of ger_price_mult; base 0 minus base 1: -9.2 [-18.3 to -0.2] points; lack of fit X2 4.1 on 6 df, p 0.66

## Solved level

- `share_germany` = 50 at ger_price_mult = 0.959, 95 % interval 0.913 to 0.999 (+-0.043; tolerance +-0.025)
  - the fitted curve is inside 45..55 for ger_price_mult 0.919 to 0.999

Best level on the fitted curves: ger_price_mult=0.959; on the grid: **ger_price_mult=0.96**

Override (also in `best_override.toml`; a demonstration until someone ships it):

```toml
[defs.units."ww2:ger_grenadiergruppe"]
cost = 216

[defs.units."ww2:ger_grenadiergruppe_7"]
cost = 154

[defs.units."ww2:ger_smg_gruppe"]
cost = 149

[defs.units."ww2:ger_vg_sturmgruppe"]
cost = 288

[defs.units."ww2:ger_engineer_squad"]
cost = 192

[defs.units."ww2:ger_panzerschreck_team"]
cost = 139

[defs.units."ww2:ger_mortar_team"]
cost = 230
```

Predicted at the chosen level (model, both bases averaged):

- `share_germany` 45..55: 49.9 [45.2-54.6]: in band (CI inside)

Every target on the 200 matches played within one tolerance of the chosen level (Wilson intervals on shares):

| target | band | value | status |
|---|---|---|---|
| `share_germany` | 45..55 | 48.0 [41-55] | pass (CI overlaps) |
| `base_max` | -..62 | 55.0 [48-62] | pass (CI inside) |
| `len_min` | 10..- | 9.9 | FAIL |
| `len_max` | -..40 | 33.7 | pass |
| `len_median` | 15..30 | 17.2 | pass |
| `he_mean` | -..- | 14.8 | pass |
| `kill_mortar_germany` | -..- | 5.6 | pass |
| `kill_mortar_usa` | -..- | 5.7 | pass |

## Verdict

**Solved, but a check fails:** len_min. The modelled targets are settled (above); the failing check is measured on the matches near the chosen level only. Look at it across levels (Batches table, cache.jsonl) before blaming the knob: a minimum or maximum over a few hundred matches fails by a hair more often as the pool grows.

Effort: 12 batches, 480 matches (stopped: settled); 0 s of new match time, 12 batches from cache.
