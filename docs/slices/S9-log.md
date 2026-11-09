# S9 log: AT guns

Stages: Plan (5557d6d), Build A (3e52d96, 366add7), Build B (e9eec2a, c3c0027), Build C (32e78fc, 9c1407f, 81dd57d, ...). Spec `S9-at-guns.md`, targets `targets/S9.toml`.

## Build C (Opus 5.5): HE at the crew, AI, presenter, batches

**HE at the crew.** A live gun with men at a slot is an HE aim at its first manned slot (`gunnery::gun_target`), from tanks and from other guns. Case 36 (T75): 100 % HE (69 of 69 rounds), 25 of 30 runs silence the gun inside 120 s, mean 35 s, 4.7 crew lost per run, 1.27 parts hit.

**AI (`ai/guns.rs`, `[ai.guns]`).** Own buy (`buy_gun`, `gun_pot`: 35 % of income so far is set aside up to one card's price, from 2:00 or a seen vehicle; `max_guns` 2). One saved job per gun: Site (sight lines to six points along the approach, then cover, nearest), Go (tow, crew riding, no unhitch in contact), Unhitch, Emplace, Hold (`SetAmbush` 400; re-lay for an armoured target outside the arc), Pit (uncovered guns, engineers within 350 m), Recrew (neutral or crewless guns). `Planner::take_guns` keeps guns, tows and crews out of the other habits. Tractors go home once; the M3A1 returns to `transport_jobs`.

**Sim changes found on the way.** Hold fire now holds against men beyond `ambush_infantry_m` (200): fired early at infantry, guns gave themselves away. A gun pit could never be dug: the line sits under the gun, so the squad never arrived. Men now dig from the rim (`dig.rs::pit_rim`, `pit_approach`; a test covers it). AI infantry was being sent to guns whose crews were still riding (`own_gun_unserved` is true then); fixed (`gun_recrew_orders` from 167 to 9 per 160).

**Presenter.** Snapshot fields `gun_state`, `gun_men`, `gun_slots`, `hitched`, `towing`, `hold_fire_m`; six intents; `COMPONENT_KEYS` and `Vehicles.COMPONENTS` 12; meshgen `gun_carriage_mesh`; gun grid (Y, L, V, U, B, M reuse existing actions, none new); arc rays on an emplaced gun; `gun_left` cause. Scene `armour_guns`, screenshot looked at: gun, shield, trail, crew at slots, arc, hitched gun and crew riding the tractor.

**Checks.** `check.sh quick`: 633 tests passed, clippy ok (fmt fixed after; no code change). Release: 668 passed, 0 failed, 0 ignored, including `ai_guns_emplace_and_are_deterministic` (1/2/3/8 threads, save/load at 5:00). Godot: check_project, test_match_view 339, test_net_ui 143, test_data_reload 21 all pass on the final bridge.

## Results (final build; 3 x 160, both bases, jobs 6; `targets` 252 pass, 15 miss, 14 hard)

| | seeds 1-80 | 101-180 | 201-280 | pooled 480 |
|---|---|---|---|---|
| Germany % | 42.5 | 34.4 | 36.3 | **37.7** (band 45-55, S8b 46.5) |

Guns fire in 80 / 80 / 82.5 % (GER) and 62.5 / 57.5 / 61.3 % (USA) of matches (T80 reads the first set: pass; the USA's second set is below 60). In cover 55.5-56.5 % GER, 41.8-46.6 % USA (T81 miss). Gun MP 16.9-18.8 % (T83 pass). Tank KOs by guns per match: GER 0.05-0.075, USA 0.02-0.025; rounds per match 6-7. Gun kill share 4.8-5.1 % GER, 1.7-2.5 % USA. T32 12.2 / 9.9 % (pass), T33 18.5 / 17.5 % (pass), T14 passes.

**Hard misses, all on the 160 sets:** T5c, T5d (balance), T8a (HE share 19.9, floor 20), T31b, T31c (vehicle kill share 30.6-30.7, cap 30), T40a-d and T72a-b (half-tracks bought in 43-58 % of matches, floor 60), T54e (StuG 71 %, floor 80), T81a-b (cover).

**Reading.** Guns cost 17-19 % of manpower and earn 2-5 % of the kills, so the budget comes out of tanks and half-tracks: that is what T40, T54e and T72 show. Germany pays 180 MP for a tractor that goes home; the USA's M3A1 stays useful as a transport. That and the budget are the likely cause of the 9-point drop against S8b's pool. Not tested here: the balance is the Tune's.

**Cover.** T81 is measured against a 45 cm low line for hull-down. Lowering that line to 25 cm in a test (40 matches) gave 71 / 72 %, but a 25 cm mask covers little; kept 45 cm and reported the miss rather than loosen the definition. Pits are real cover and now finish, but only some guns have engineers near.

**Tick time.** Clean `timing40` alone: p99 0.653 all / 0.769 peak (baseline 0.441 / 0.532, +48 % / +45 %; T14 limit 1.5 passes). `sample` on six long matches: no gun function shows in the top stacks (`gun_status`, `take_guns`, `gun_recrews` are 1-2 of about 19000 samples); the top is `SpatialHash::rebuild` and `combat::search`, as before. Not explained: a rise spread over the frame, possibly from the longer matches with crews and tows. Next stage should compare entity counts and match length before chasing it.

**Tune (no value changed).** Pooled Germany 37.7 % cannot be moved into 45-55 by the allowed knobs; table and intervals in spec section 22. Tractor free: 41.7; no guns: 46.5; one gun per side: 36.9; Germany without guns 72.1, USA without guns 17.5 (the side that buys guns loses 25-30 points); autotune run 1 (tractor 90-180, 57 mm 340-460, 760 matches) best corner measured 38.1 on 480; run 2 (`income_max_pct` 10-35) at 10 still 37.5 [30-45]. Code needed: guns that earn their price (S9-17) or a value-based AI buy.
