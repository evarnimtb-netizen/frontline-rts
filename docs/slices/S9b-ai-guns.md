# S9b: AI gun pass

Status: spec (2026-11-09, written by the orchestrator from the diagnosis below; no Plan stage). Branch `s9b-ai-guns`
from `71cf02f` (main). Owner scope (S9-17, S9-19): a tank-corridor search for gun sites, then a budget tune so
half-tracks and tanks recover, then the 14 hard target misses measured again. No new units, no new commands.
Targets `docs/slices/targets/S9b.toml`. SAVE 20 and PROTOCOL 12 stay unless section 4 says otherwise.

## 1. Baseline (verified S9 build, `tools/pool.py` on the saved sets)

Germany pooled 45.62 % of 480 (sets 48.75 / 46.25 / 41.88), no match without a winner. Guns: bought 2.09 / 1.99 per
match (GER / USA), 7.3 / 9.9 rounds per match, fired in 74-78 / 69-72 % of matches, in cover 83-85 / 78-86 %, gun
manpower 17.5 / 19.4 %. Tank knock-outs by guns 36 / 16 over 480 (0.075 / 0.033 per match). 252 of 267 targets pass,
14 hard misses (S9-19). Tick p99 0.433 / 0.534 ms.

## 2. Diagnosis: why guns put so few tanks out of action

Measured with `frontline-cli skirmish --gun-trace` and `tools/gun_funnel.py` over 80 matches of the S9 build (seeds
1-40, both bases). Per side about 1.9 enemy armoured vehicles (tank, assault gun, tank destroyer) are fielded per match.

| Step, share of the enemy armour fielded | German guns | US guns |
|---|---|---|
| comes within 400 m of a ready gun (emplaced, 2 men or more) | 57 % | 58 % |
| ... inside the gun's arc | 55 % | 56 % |
| ... with a clear sight line from the gun | 19 % | 24 % |
| fired at | 24 % | 28 % |
| struck / penetrated | 22 / 22 % | 21 / 19 % |
| lost to the gun (knock-out or bail-out) | 22 % | 14 % |

1. **The sight line is where it is lost.** Armour is inside reach and arc of a ready gun for 13 % (GER) and 19 % (USA)
   of the ready gun time, and in sight for 0.6 % and 2.4 % of it. `gun_site` puts the gun 100-250 m behind the point,
   scores cover before sight (`key = (cover, sees, ...)`: a covered site with one sight line of six beats an open site
   with six), and the six sample points lie on one straight line toward the enemy base or the nearest seen vehicle,
   not where armour drives or halts. Tanks are lost 200-400 m from a ready enemy gun that never saw them.
2. **43 % of the armour never comes within 400 m** of a ready gun: the site follows one point, the armour works on another.
3. **After the sight line nothing is lost.** Every vehicle a gun gets a line on is aimed at, fired at, and nearly all
   are penetrated and lost. Range, arcs, the ambush rule, crew losses (0.05-0.17 guns knocked out per match) and
   ammunition (ready time with no rounds: 0-1 %) are not the cause.
4. **The S9 counter undercounts.** `gun_ko_tanks` counts wrecks. 88 % of armour losses are bail-outs; counting those
   after a gun's round, German guns took out 0.41 and US guns 0.33 armoured vehicles per match in the trace, against
   0.125 and 0.04 wrecks. That is 23 % and 21 % of the enemy's armour losses for 17-19 % of the manpower.
5. Rounds: 6.5 / 9.7 per match; German guns fire 17 % of them at armour and 58 % at men, US guns 21 % at armour and
   22 % at the Sd.Kfz. 11 tractor.

So the corridor search is the right fix, aimed at sight lines: the gun must see the ground armour uses inside
`engage_still_m`, and it must be at the point the armour comes to.

## 3. What gets built

**3.1 Measure (CLI only).** `factions.<f>.s9_derived.gun_armour_out_per_match` and the raw count in the batch JSON:
enemy armoured vehicles knocked out or abandoned when the last round that struck them was a served gun's. The armour
events carry no shooter, so the tally matches impacts to gun rounds as `gun_funnel.py` does (a gun of the attacker
fired that ammunition at that vehicle within 3 s). Text summary line and `docs/modding/tuning.md` entry. Its value on
the unchanged sim is the baseline for T88; record it for all three sets before changing the AI.

**3.2 Corridor siting (`crates/sim/src/ai/guns.rs`, `[ai.guns]`).** Replace the straight approach line by the routes
armour takes:

- *Corridor.* For the point to cover: the vehicle path (the AI's own path search on the vehicle layer) from each
  seen enemy armoured vehicle to that point, and with none seen from the enemy base. Sample it every `corridor_step_m`
  over the last `corridor_m` before the point, at an enemy's eye height. With no path, the straight line as today.
- *Which point.* As today (armour, threat, priority), but a gun that is re-sited goes where armour is now.
- *Candidates.* Not only behind the point: rings around it out to `site_max_m`, never nearer to the enemy base than
  the point by more than `site_forward_m`, clear of known enemy men (`enemy_standoff_m`) and of the other gun.
- *Score.* Sight first: count the corridor samples within `engage_still_m` that the gun sees from gun height while
  they lie inside its arc when it faces the middle of what it sees. Drop sites that see less than `site_view_min_pct`
  of the best site's count. Among the rest: cover (as today), then the count, then the nearest. Facing: the middle of
  the samples seen, not the approach line. T81 (in cover at least 60 %) must still pass; cover is the tie-breaker now.
- *Re-site.* Keep the S9 rule (`resite_idle_s`, `resite_max`); the new site comes from the corridor of the armour
  known then. Add no new job phase unless the funnel shows a need.
- *Cost.* Path searches run only in the Site phase (a few per gun per match), never per tick; use the existing search
  with its node budget. No move loops (`max_fails`, `repath_s` unchanged).

**3.3 Budget.** After the siting is in, the orchestrator runs the ablations (`tools/measure.sh` with overrides for
`income_max_pct` and `max_guns`); a value changes only when the pool of 480 confirms it. Allowed knobs: `[ai.guns]`
and the S9 gun cards' cost, pop, cooldown, cap, unlock. Never armour, penetration, guns, ammo, lethality or the
prices of older cards.

## 4. Save and protocol

Preferred: no new saved field (the corridor is recomputed from the world at Site time). If the design needs one
(for example a remembered corridor in `AiGun`), bump `SAVE_VERSION` to 21 once, with per-field hash, save/load and
validation tests, and say so in the report. No new command, so `PROTOCOL_VERSION` stays 12.

## 5. Tests

- Unit: on a ridge map a gun sited by the new search sees the corridor inside `engage_still_m`; a covered site
  that sees nothing loses to an open one that does; no path falls back to the line; no site forward of the limit.
- `ai_guns_emplace_and_are_deterministic` (1/2/3/8 threads, save/load at 5:00) and the re-site test keep passing.
- Baselines in `resolved_defs.rs` and `simbench/report.rs` refreshed for each TOML change.

## 6. Done when

- T88: armour put out of action by guns per match at least 0.55 (GER) and 0.45 (USA) on the 160 sets, from 0.41 and
  0.33; the wreck counter (T84c/d) is reported beside it.
- T80 (fires in at least 60 % of matches), T81 (in cover at least 60 %), T83 (gun manpower at most 20 %) pass.
- Germany pooled 45-55 % over 480; no match without a winner.
- As many of the 14 hard misses as the allowed knobs reach are back in band; each one left has its cause in the log.
- Determinism tests pass, 40 hash lines cross-check, clean tick p99 at most 25 % above 0.433 / 0.534 ms.

## 7. Left out

Displacing after shots or under HE, spiking, mixing guns into the main buy by spotted threat, remembered sightings
(a saved heat map of where armour was seen), a third gun, the 3-inch M5 in the AI, HE for the 57 mm.
