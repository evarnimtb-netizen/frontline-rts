# S6b: mortar AI pass (log)

Owner choice 2026-11-05: "Mortar AI pass first". Branch `s6b-mortar-ai`
from `55b322d` (S6 head). Spec `S6b-mortar-ai.md`; targets
`targets/S6b.toml` (S6 unchanged; main T9b, T8a, T8b).

## Build

Commits `be43379` (spec, targets), `c2aa665` (code, data, tests, docs), and
this log. `SAVE_VERSION` 16 (two AI counters, two rules keys);
`PROTOCOL_VERSION` 9 unchanged.

### Diagnosis (probes, scratchpad `s6b/`)

- AI map fire starts at max(8 % of range, 27 m) and landed ~36 m off;
  with none ordered, kills per bomb rose on 40 matches but not clearly on
  160. It is now held by `map_fire_max_miss_m` 20 (map start error >= 27 m).
- A temporary per-plan tally (160): tubes are on a mission 23 % of plans,
  walking 34 %, held at the reserve with targets 24 % (mean 2.4 bombs
  left), idle 7-10 %, target but no observer ~6 %.
- Target scoring (trench weight 0; only men within 15 m) and
  `mission_rounds` 4 / 8: no change in kills per bomb (0.10-0.11) on 160.
- What moved the share was bombs: reserve 0 + crate runs at 90 % + one
  bearer squad per 2 mortars (USA 9.0 -> 11.8 % on the same 160 seeds).
- USA buys fewer mortars only because it buys fewer squads (22.5 % vs
  21.6 % of buys); its mortar kills per match were already about
  Germany's; its share was lower because the USA kills more in total.
- A standing observation post (leader to a post toward the front with no
  target) was built and tested, then removed: on 160 it cut Direct bombs
  26 -> 16 per match and the mortar share (Germany 12.9 vs 14.2, USA 11.8
  vs 12.0 without it).

### What was built

- `[ai.mortar] map_fire_max_miss_m` (default 1000; ww2 20) and
  `fm_miss_holds`.
- `[ai.ammo] mortar_reserve_pct` 30 -> 0; `[ai.supply]
  crate_run_below_pct` 50 -> 90; new `mortars_per_bearers` (default 0 =
  one squad; ww2 2).
- C3 (S5b-22): an AI fire mission whose squad is dry when it comes due is
  withdrawn in `take_ai_commands` (`fm_dry_drops`); 160: 4 / 6 withdrawn,
  0 refused.
- CLI: `miss_holds`, `dry_drops` in `s5b.ai` and the summary line.

### Tests

- New `crates/sim/tests/s6b_ai_mortar.rs` (4, release-only): map fire past
  the limit held; dry order withdrawn, no refusal; 2 bearer squads for 3
  mortars (1 with key 0); determinism 1/2/3/8 threads + save/load/continue.
- `s5b_ai_mortar.rs` map-fire tests use limit 1000 (S5b rules).
- `s5a_ai.rs` `all_dry_ai_vs_ai_fires_again`: limit 6 -> 7 min (one seed,
  388 s now, 341 s at S6 values, 337-341 s with either supply value
  reverted alone; timing of the walk back, not a stuck side). Flag for
  review.
- Per-field hash entries; content snapshot and setup-bytes baseline
  re-blessed.
- `tools/check.sh quick 55b322d`: fmt, clippy ok; 440 passed, 0 failed.
  Release: s5a_ai 6, s5b_ai_mortar 4, s6b_ai_mortar 4, ai 9, ai_assault 1,
  ai_vehicles 4, ai_armour 5, determinism 14, match_mode 17, sim lib 145:
  all pass.

### Targets (`frontline-cli targets docs/slices/targets/S6b.toml --jobs 6`)

| Run | Result |
|---|---|
| skirmish40 | 12 pass, 0 miss: T4a/b 0 / 0 refusals; T5 50 / 50 %; base 50 %; length 10.4-27.1, median 17.6; HE 21.8 %; mortar 14.2 / 12.0 % |
| skirmish160 | 31 pass, 0 miss: Germany 52.5 %; base 51.3 %; median 17.6; **T8a HE 21.9 %**; T9a 14.2 %; **T9b USA 12.0 %**; bombs 117 / 115 per match; crate MP 233 / 236 |
| skirmish160_oos (101-180) | 3 pass, **1 miss**: **T5d Germany 43.1 %** (band 45-55); T6c base 59.4 %; **T8b HE 21.5 %**; mortar 14.0 / 11.3 % |

Before (S6, same sets): mortar 11.6 / 9.2 %, HE 19.8 / 19.1 %, Germany
50.6 / 51.9 %.

## Known gaps

- T5d: Germany 43.1 % out of sample (pooled 320: 47.8 %; S6 51.3 %). The
  feeding helps both sides' mortars; the USA gained more (+2.8 vs +2.6
  points). Needs a Tune round (German price lever) on 160 + 160.
- Germany's mortar share 14.0-14.2 % sits near the 15 % cap.
- Not run: tick timing (`timing40`, `--jobs 3`), armour/fortify runs,
  Godot checks (no presenter change), `check.sh full`.
- `mortar_reserve_pct` 0 means no defensive bomb reserve; `defend`
  still overrides nothing now.

## What the next stage must know

- Raw outputs: scratchpad `s6b/` (`final/` targets and JSON, `q_*` 160
  probes, `p_*` 40 probes, `sum.py`).
- The post code is gone; if an owner wants FO behaviour later, the
  leader must not leave the tube's Direct role (measured).
