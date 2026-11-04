# Autotune report: s5a_german_price

Demo: re-find the S5a German card price level. One knob: a single multiplier on the six German card costs
(the pre-Tune values, S5a Part B). Shipped Tune chose 0.8 (Grenadiergruppe 280 -> 225 etc.).

CLI `target/release/frontline-cli`, 45 min limit. Seed sets (each seed plays twice, normal and swapped): screen seeds 1..20 (40 matches), confirm 1..80 (160), out of sample 101..180 (160). Every candidate runs the same seeds (common random numbers). Bracketed ranges are 95 % Wilson intervals on the binomial count; they treat matches as independent, which is conservative for the swapped pairs.

Targets (screen bands in the search may be wider):

- `share_germany`: 45 .. 55 (screen 35 .. 65), weight 1
- `base_max`: - .. 62 (screen - .. 62), weight 1
- `len_min`: 10 .. - (screen 10 .. -), weight 1
- `len_max`: - .. 40 (screen - .. 40), weight 1
- `len_median`: 15 .. 30 (screen 15 .. 30), weight 1

## Every run

| stage | ger_price_mult | n | loss | share_germany | base_max | len_min | len_max | len_median | tick p99 | verdict |
|---|---|---|---|---|---|---|---|---|---|---|
| screen | 0.7 | 40 | 0.13 | 67.5 [52-80] | 62.5 [47-76] | 10.8 | 32.2 | 16.9 | 1.156 | out: share_germany, base_max |
| screen | 1 | 40 | 0.38 | 27.5 [16-43] | 52.5 [37-67] | 10.3 | 28.6 | 18.6 | 2.525 | out: share_germany |
| screen | 0.85 | 40 | 0.01 | 47.5 [33-63] | 52.5 [37-67] | 10.9 | 29.5 | 20.0 | 1.664 | all in band |
| screen | 0.8 | 40 | 0.39 | 27.5 [16-43] | 52.5 [37-67] | 10.4 | 27.8 | 16.8 | 1.459 | out: share_germany |
| confirm | 0.85 | 160 | 0.38 | 42.5 [35-50] | 53.8 [46-61] | 10.7 | 31.2 | 18.2 | 11.073 | out: share_germany |
| oos | 0.85 | 160 | 0.02 | 48.1 [41-56] | 55.6 [48-63] | 10.9 | 32.4 | 18.3 | 9.424 | all in band |

## Chosen

**ger_price_mult=0.85**

Override (also in `best_override.toml`):

```toml
[defs.units."ww2:ger_grenadiergruppe"]
cost = 238

[defs.units."ww2:ger_grenadiergruppe_7"]
cost = 170

[defs.units."ww2:ger_smg_gruppe"]
cost = 166

[defs.units."ww2:ger_engineer_squad"]
cost = 212

[defs.units."ww2:ger_panzerschreck_team"]
cost = 153

[defs.units."ww2:ger_mortar_team"]
cost = 238
```

| set | matches | share_germany | base_max | len_min | len_max | len_median | tick p99 |
|---|---|---|---|---|---|---|---|
| confirm | 160 | 42.5 [35-50] | 53.8 [46-61] | 10.7 | 31.2 | 18.2 | 11.073 |
| out of sample | 160 | 48.1 [41-56] | 55.6 [48-63] | 10.9 | 32.4 | 18.3 | 9.424 |

Per target status (confirm / out of sample):

- `share_germany` band 45..55: miss (CI overlaps) / pass (CI overlaps)
- `base_max` band -..62: pass (CI inside) / pass (CI overlaps)
- `len_min` band 10..-: pass / pass
- `len_max` band -..40: pass / pass
- `len_median` band 15..30: pass / pass

## Verdict

**Not proven.** No target clearly fails, but at least one point estimate is outside its band with an interval that overlaps it. More seeds (or a different level) are needed to say.

Search effort: 4 screens, 1 confirms, 1 out-of-sample run; 0 s of new match time (6 runs from cache).
