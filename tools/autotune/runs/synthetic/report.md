# Synthetic comparison: old GP search vs logistic dose-response

200 repetitions per cell; the true level x* is drawn uniformly from 0.75-0.95 in a knob range 0.70-1.00. Truth model and noise: see the docstring of `tools/autotune/synth.py` (seed effect sd 0.3 on the logit, shared across levels; 6-point base advantage; the swapped twin of a seed shares it). The old method uses the demo's 0.05 grid; the new one a 0.01 grid and tolerance +-0.025.

Columns: **true share in band** = the true Germany share at the chosen level lies in 45-55 (what the owner wants); |err| <= 0.025 / 0.05 = chosen level within that distance of the true 50 % level; **interval covers** = the reported 95 % interval on the level contains the true level (only the new method reports one); **false pass claims** = runs whose verdict claims success (old: 'pass' on confirm and oos; new: 'in band' or 'level resolved') although the true share at the chosen level is outside 45-55; matches = mean matches used.

## 6 pts/step

| method | true share in band | abs err <= 0.025 | abs err <= 0.05 | mean abs err | interval covers | false pass claims | matches | verdicts |
|---|---|---|---|---|---|---|---|---|
| old: GP, 4 screens + confirm + oos | 46 % | 28 % | 50 % | 0.055 | - | 11/58 | 440 | not proven 142, pass 58 |
| old: GP, 6 screens + confirm + oos | 48 % | 32 % | 53 % | 0.050 | - | 10/65 | 519 | not proven 135, pass 65 |
| new: logistic, budget 480 | 95 % | 74 % | 98 % | 0.017 | 96 % (200 bounded) | 5/97 | 467 | in band 95, level resolved 2, not resolved 103 |
| new: logistic, budget 800 | 97 % | 80 % | 100 % | 0.015 | 96 % (200 bounded) | 6/200 | 506 | in band 197, level resolved 3 |
| new: logistic, budget 1200 | 97 % | 80 % | 100 % | 0.015 | 96 % (200 bounded) | 6/200 | 506 | in band 197, level resolved 3 |

## 5 pts/step

| method | true share in band | abs err <= 0.025 | abs err <= 0.05 | mean abs err | interval covers | false pass claims | matches | verdicts |
|---|---|---|---|---|---|---|---|---|
| old: GP, 4 screens + confirm + oos | 45 % | 27 % | 44 % | 0.064 | - | 12/72 | 440 | not proven 128, pass 72 |
| old: GP, 6 screens + confirm + oos | 49 % | 28 % | 48 % | 0.060 | - | 11/74 | 519 | not proven 126, pass 74 |
| new: logistic, budget 480 | 95 % | 72 % | 95 % | 0.019 | 96 % (200 bounded) | 2/103 | 466 | in band 102, level resolved 1, not resolved 97 |
| new: logistic, budget 800 | 98 % | 74 % | 98 % | 0.017 | 96 % (200 bounded) | 3/200 | 505 | in band 199, level resolved 1 |
| new: logistic, budget 1200 | 98 % | 74 % | 98 % | 0.017 | 96 % (200 bounded) | 3/200 | 505 | in band 199, level resolved 1 |

## 8 pts/step

| method | true share in band | abs err <= 0.025 | abs err <= 0.05 | mean abs err | interval covers | false pass claims | matches | verdicts |
|---|---|---|---|---|---|---|---|---|
| old: GP, 4 screens + confirm + oos | 42 % | 38 % | 60 % | 0.047 | - | 9/60 | 440 | not proven 140, pass 60 |
| old: GP, 6 screens + confirm + oos | 50 % | 44 % | 68 % | 0.040 | - | 11/72 | 519 | not proven 128, pass 72 |
| new: logistic, budget 480 | 96 % | 86 % | 100 % | 0.014 | 94 % (200 bounded) | 3/77 | 465 | in band 55, level resolved 22, not resolved 123 |
| new: logistic, budget 800 | 98 % | 86 % | 100 % | 0.013 | 94 % (200 bounded) | 5/200 | 513 | in band 169, level resolved 31 |
| new: logistic, budget 1200 | 98 % | 86 % | 100 % | 0.013 | 94 % (200 bounded) | 5/200 | 513 | in band 169, level resolved 31 |

## 6 pts/step, kinked (half slope above x*)

| method | true share in band | abs err <= 0.025 | abs err <= 0.05 | mean abs err | interval covers | false pass claims | matches | verdicts |
|---|---|---|---|---|---|---|---|---|
| old: GP, 4 screens + confirm + oos | 50 % | 18 % | 38 % | 0.079 | - | 6/59 | 440 | not proven 141, pass 59 |
| old: GP, 6 screens + confirm + oos | 56 % | 23 % | 46 % | 0.070 | - | 5/68 | 518 | not proven 132, pass 68 |
| new: logistic, budget 480 | 95 % | 46 % | 81 % | 0.032 | 83 % (198 bounded) | 4/102 | 468 | in band 102, not resolved 98 |
| new: logistic, budget 800 | 96 % | 49 % | 83 % | 0.030 | 84 % (199 bounded) | 6/198 | 507 | in band 197, level resolved 1, not resolved 2 |
| new: logistic, budget 1200 | 96 % | 49 % | 83 % | 0.030 | 84 % (199 bounded) | 7/200 | 509 | in band 199, level resolved 1 |

Run time 110 s.
