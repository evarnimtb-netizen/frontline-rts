# S6b: mortar AI pass (spec)

Owner choice 2026-11-05: "Mortar AI pass first" (before S7). Goal: the
S5b-17 bands (mortars 10-15 % of each side's kills, HE share 20-25 %, hard
cap 26.3 %) through AI behaviour only: mission size, bomb load, map fire.
No change to blast, fragments, warheads, dispersion or damage. Branch
`s6b-mortar-ai` from `55b322d`. Targets: `docs/slices/targets/S6b.toml`
(S6 unchanged; main targets T9b, T8a, T8b).

## Diagnosis (S6 binary; 160 = verify-s6 JSON, probes = seeds 1-20 x both bases)

| Probe (40 matches) | mortar % G / U | HE % | bombs/match G / U | kills per bomb G / U | map bombs, mean miss |
|---|---|---|---|---|---|
| S6 as is | 12.8 / 8.6 | 19.4 | 107 / 102 | 0.105 / 0.097 | 43 / 38 %, 21-23 m |
| `mortar_reserve_pct` 10 | 11.9 / 8.1 | 19.0 | 115 / 100 | 0.094 / 0.087 | same |
| `mission_rounds` 10 | 10.7 / 9.2 | 18.6 | 119 / 108 | lower | same |
| AI orders no map fire | 13.4 / 10.3 | 21.0 | 98 / 85 | 0.118 / 0.130 | 32 / 26 %, 15 m |
| no map fire + reserve 10 / rounds 4 / crate run 75 % / spotter 150 s | 13.1-12.3 / 10.3-9.5 | 19.9-21.0 | 79-103 | 0.108-0.145 | 26-34 % |

1. **AI-ordered map fire wastes bombs.** Map fire starts with an error of
   max(8 % of range, 27 m) (`[fire_mission]`), so every AI map mission
   lands about 27-36 m off (the ordered map bombs alone: ~36 m mean planned
   miss, from the drop 21.5 -> 15 m when they go). They are ~15 bombs per
   match per side (15 % of AI missions). With none ordered the same tubes
   fire fewer bombs (85-98) but kill more (USA 9.9 -> 11.1 mortar kills per
   match). The remaining "map" bombs are observed missions whose observer
   was lost (~6-7 downgrades per match); they keep the observed start error.
2. **Tubes run dry and wait.** A quarter of tube-plans hold targets but
   sit at or below the 30 % reserve (mean 2.4 bombs) waiting for a crate
   run that starts only below 50 %, from one bearer squad. Lowering the
   reserve alone adds only small missions (40-match probe); with crate
   runs at 90 % and more bearers it adds 10-20 bombs per match (below).
3. **Few observers for unseen targets.** ~1.1-1.4 spotter missions per
   match; longer spotter stays (150 s) change little.
4. **USA buys fewer mortars because it buys fewer squads** (3,385 vs
   4,483 over 160; mortars 22.5 / 21.6 % of buys). USA mortar kills per
   match are about Germany's (10.4 vs 10.7); its share is lower because it
   kills more in total (113 vs 92 per match). Same logic; no buy change.
5. **`no_bombs` refusal (S5b-22):** bombs checked at plan time; carriers
   can fall during the 1-4 s reaction delay.

## Build findings (seeds 1-80 x both bases, 160 each; a temporary per-plan tally)

Tube-plans: on a mission 23 %, walking 34 % (fresh teams walking out),
held by the reserve with targets 24 % (mean 2.4 bombs left), no target
7-10 %, no observer ~6 %. Target scoring and `mission_rounds` 4 / 8 did
not move kills per bomb (0.10-0.11); more bombs did (reserve 0, earlier
crate runs). A standing observation post (leader sent forward with no
target) took the leader from the tube (Direct bombs 26 -> 16 per match);
removed (owner-facing note).

## Changes (both factions, same code; every value in TOML)

- **C1 no AI map fire past a miss limit** (map fire lever). New
  `[ai.mortar] map_fire_max_miss_m` (m; default 1000 = no limit; ww2
  **20**): the AI orders map fire only where the sim's map start error at
  that range (`fire_mission::start_error`) is at most this, else holds
  (`fm_miss_holds`). The ww2 `[fire_mission]` floor is 27 m, so 20 is an off
  switch: the AI orders no map fire (a player can; better map data, S11,
  brings it back without code). Map bombs remain, 28 % of bombs on 160
  (mean miss 14.4 m, S6 21.7 m): observed missions whose observer fell.
- **C2 keep tubes fed** (bomb load / crate runs, AI only):
  `[ai.ammo] mortar_reserve_pct` 30 -> **0** (a mission still needs 2
  bombs); `[ai.supply] crate_run_below_pct` 50 -> **90**; new
  `[ai.supply] mortars_per_bearers` (default 0 = one bearer squad as in
  S6; ww2 **2**): one bearer squad per 2 mortar squads, rounded up.
- **C3 dry orders withdrawn** (S5b-22): when an AI `FireMission` comes due
  and its squad holds no bombs, the AI withdraws it (`fm_dry_drops`); the
  sim's refusal count for an AI player stays 0.
- Owner note: reserve 0 drops the `defend` override (no bombs kept for
  defence); the 10 % probe cost mortar share (12.8 / 8.6 -> 11.9 / 8.1).
- Not changed: `mission_rounds` 6, bomb load 20 per team (MID 1943 gives
  24 for the German crew; no US source for more), buys, prices, physics.

## State, versions

`AiCounters::{fm_miss_holds, fm_dry_drops}`, saved and hashed with the AI
state; per-field hash entries in `world.rs`. `SAVE_VERSION` 15 -> 16;
`PROTOCOL_VERSION` 9 (no command change). CLI: both counters in `s5b.ai`
and the `S5b ... AI:` line; `ai_missions_total` and that line count
missions sent (planned minus dry orders withdrawn; `ai_missions_planned`).

## Tests

- `crates/sim/tests/s6b_ai_mortar.rs` (release-only, staged like
  `s5b_ai_mortar.rs`): map fire past the limit is held (no mission, no
  refusal); bombs lost while the order waits: withdrawn, no refusal;
  bearer squads 2 for 3 mortars (1 with the key 0); determinism at
  1/2/3/8 threads and save -> load -> continue under the ww2 values.
- `s5b_ai_mortar.rs` map-fire tests set `map_fire_max_miss_m` 1000.
  `s5a_ai.rs` `all_dry_ai_vs_ai_fires_again`: 7 min instead of 6 (one
  seed; 341 s at S6, 388 s at S6b).
- Content snapshot and setup-bytes baseline re-blessed; per-field hashes.

## Tune results

Tune shipped German cards x0.99 (autotune 0.992 [0.970-1.021], 560
matches); the interval holds the S6 prices and T5c / T5d went 52.5 / 43.1
-> 43.1 / 55.6 %. Review: out of scope (non-mortar prices), unsupported;
reverted in the fix stage. Prices are the S6 values; T5: see the log.
