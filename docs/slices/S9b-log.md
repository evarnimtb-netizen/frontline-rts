# S9b log

Versions: `SAVE_VERSION` 20 -> 21 once (S9b Fix): `[ai.guns]` in the saved setup changed layout (`site_spread_deg` gone,
seven corridor siting keys new) and `AiGun::point` is gone. `PROTOCOL_VERSION` stays 12.

## Target misses at the end of the siting and budget stages

Read from `tools/measure.sh` (480 matches) on a9fa303, values on the set the target names (`base` = the S9 sim, `m1` =
a9fa303 with `income_max_pct` 35, `b30` = the same with 30). Fix changes only the T88 count and a few code paths; the
next measure replaces these numbers.

| Target | base | m1 | b30 | What the runs show |
|---|---|---|---|---|
| T8a/b/d HE share floor 20 % | 18.6 | 19.2 | 20.0 / 19.1 | Missed since before S9b and below the floor in every run. Not a gun-AI value: the siting moved it by about 0.6, inside the 18.2-20.0 spread of the sets. |
| T9b USA mortar kills floor 10 % | 10.0 | 9.9 | 11.0 | Sits on the floor at baseline (the other sets 10.3, 8.9 base; 10.9, 9.4 m1). No shift beyond the set spread. |
| T31a/b vehicle kills cap 30 % | 30.3 / 29.8 | 30.8 / 31.7 | 29.3 / 30.6 | On the cap at baseline; per set 29.2-31.7 at m1. Lower gun money (30) moves Germany under it, not the USA. |
| T40a-d, T72a/b half-track bought floor 60 % | 56 / 41 | 53 / 42 | 64 / 49 | The gun set-aside competes with them: floors move up with lower `income_max_pct` (66 / 54 at 25, 64 / 67 at 15, pooled) and still miss for the USA at 30. |
| T54e/f StuG, M10 bought floors 80 / 75 % | 73 / 69 | 75 / 69 | 77 / 71 | Same lever, same direction, not reached at 30. |
| T55a Sd.Kfz. 234 kill share cap 10 % | 9.9 | 11.3 | 9.6 | Real shift on all three sets at m1 (11.3 / 11.3 / 11.4 against 9.9 / 10.3 / 10.9), gone at 30. Follows the gun budget; the mechanism is not isolated. |
| T88a/b armour out by guns | 0.39 / 0.36 | 0.53 / 0.46 | 0.63 / 0.425 | Counted repeat losses of one vehicle until S9b Fix (about +3 % Germany, +10-12 % USA in 120 traced matches), so these values read high against the bar. Since Fix each vehicle counts once a match; `tools/gun_funnel.py` can be slightly higher, as it also counts a gun's knock-out of a vehicle already abandoned. |

The siting stage's "need a different lever" was wrong for T9b, T31a, T40a, T55a, T72a and T88a: `b30` clears them. The
choice between 35 and 30 has to be read again on the corrected T88.

## Values not ablated at 480 matches

`site_view_min_pct` 10, `site_forward_m` 0, `corridor_clear_m` 150 and `site_enemy_point_m` 250 were chosen on 40- and
80-match traces (guns lost 24 of 166 against 61 of 180 for the view cut). Only the final set was pooled at 480.
