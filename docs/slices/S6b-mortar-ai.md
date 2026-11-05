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
2. **Tubes with bombs have no observer.** With map fire gone, bombs per
   match fall: tubes hold bombs and targets but nobody sees them. A spotter
   goes out only toward a target already seen (3-4 per match), walks up to
   60 s and is recalled 60 s after arriving; ~1.1-1.4 spotter missions per
   match. Longer spotter stays (150 s) alone change little: the leader is
   rarely out when a target appears.
3. **The reserve is not the cap.** `mortar_reserve_holds` (400-500 per
   match) counts squad-plans of tubes at or below 30 % with targets; lowering
   the reserve adds small missions (2-6 bombs) at lower kills per bomb.
   Bombs fired are near what is carried plus 25-34 forward bombs per match.
4. **USA buys fewer mortars because it buys fewer squads.** Mortar teams
   are 21.6 % (G) and 22.5 % (U) of AI buys; the USA buys 3,385 squads vs
   4,483 over 160. USA mortar kills per match are about Germany's (10.4 vs
   10.7) and per bomb a little higher; its share is lower because USA kills
   more in total (113 vs 92 per match: Germany fields more, cheaper men).
   Same logic for both sides; no buy change.
5. **`no_bombs` refusal (S5b-22):** the AI checks bombs (>= 2) at plan time;
   the men carrying them can fall during the 1-4 s reaction delay.

## Changes (both factions, same code; values in `[ai.mortar]`)

- **C1 map fire only when it can hit** (map fire lever). New key
  `map_fire_max_miss_m` (m; default 1000 = no limit, legacy content plays
  as before; ww2 **20**). The AI orders map fire only if the sim's own map
  start error at that range (`fire_mission::start_error`, the value the
  sim draws from) is at most this; else it holds (`fm_miss_holds`). With the
  ww2 `[fire_mission]` (minimum 27 m) the AI orders no map fire; a player
  still can. A mod with better map data (registered fire, S11) gets AI map
  fire back without code.
- **C2 standing observation post** (fire-mission use, doctrine: the section
  leader observes from a forward OP). New key `post_max_s` (s; default 0 =
  no posts; ww2 **180**). A set-up tube (not walking, front point in band)
  with at least 2 bombs and more than the reserve, no spotter out and none
  recalled within `spot_max_s`, that has no target to fire on now and sends
  no target spotter: its leader goes to a post, found by the same spot
  search (same rays, same budget) toward the front point. He stays while
  missions come; he is called back `post_max_s` after he arrived (or after
  the last mission) with nothing, `3 x spot_max_s` if he never arrives, and
  at once when the front point leaves the tube's band (the tube then moves).
  Missions through him are Direct (own leader). Record: `AiSpotter::post`.
- **C3 dry orders withdrawn** (S5b-22): when an AI `FireMission` comes due
  and its squad holds no bombs, the AI withdraws it (`fm_dry_drops`); the
  sim's refusal count for an AI player stays 0.
- Not changed: mission size (`mission_rounds` 6: 4 and 10 gave no clear
  gain), bomb load (20 per team; MID 1943 gives 24 for the German crew, but
  it would raise only Germany), reserve, crate runs, buys, prices.

## State, versions

`AiSpotter::post: bool`; `AiCounters::{fm_posts, fm_miss_holds,
fm_dry_drops}`. Saved and hashed with the AI state (postcard, whole);
per-field hash tests in `world.rs`; validation unchanged (bool, counters).
`SAVE_VERSION` 15 -> 16 (layout). No command change: `PROTOCOL_VERSION` 9.
CLI: the three counters in `s5b.ai` (JSON) and the `S5b ... AI:` line.

## Tests

- `crates/sim/tests/s6b_ai_mortar.rs` (release-only, staged like
  `s5b_ai_mortar.rs`): (a) no target spotter possible, expected map miss
  above the limit: no mission, `fm_miss_holds` > 0, no refusal; at limit
  1000: map fire as before; (b) a set-up tube with no target sends its
  leader to a post with a sight line to the front, he stays past
  `spot_max_s`, an enemy that walks into his view gets a Direct mission
  through him, he is recalled after `post_max_s`; (c) bombs removed while
  the AI's order waits: no refusal, `fm_dry_drops` 1; (d) determinism at
  1/2/3/8 threads and save -> load -> continue with a post out.
- `s5b_ai_mortar.rs`: the map-fire tests set `map_fire_max_miss_m` 1000 and
  `post_max_s` 0 (they test the S5b rules).
- Per-field hash/save entries for the new fields; content snapshot
  re-blessed; tuning key docs.

**Build order:** content keys and ww2 values; C3, C1, C2 and the CLI
counters; tests, `check.sh quick 55b322d`, release-only AI tests; targets
on 40 and one 160 run; commit.
