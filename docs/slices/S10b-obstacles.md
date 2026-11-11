# S10b: mines, AT obstacles, the bunker (mechanics half): common part

Status: spec (Plan, 2026-11-11). Branch `s10b-obstacles` from main `8acc534`. Targets `targets/S10b.toml`; log
`S10b-log.md` (read its top block first). Each Build stage reads this file, its own `S10b-build-X.md`, and the log.
Design: `roster-expansion.md` S10 (scope, done-when); `infantry-structures.md` 3.3, 7.1, 7.6, 7.7. Pattern and code:
`S10a-fortifications.md`, `crates/sim/src/fort.rs`, `mods/ww2/defs/fortifications/`, `docs/modding/fortifications.md`.

## 1. Scope (owner rules: mechanics slice, no Tune)
**In.** AP and AT minefields (laid as line blueprints), contact triggering, detection by sweeping, lifting, mines
set off by HE; hedgehogs and log barriers that block vehicle path nodes; the timber-and-earth bunker with a firing
slit; demolition of pieces by a placed satchel; capabilities `place_mines`, `clear_mines`, `demolish`; commands, save,
hash, net, bridge, HUD, blockouts; CLI counters, fortify-test rows, neutrality. **Out, S10b-ai (next):** any AI use,
`ai_weight` above 0 for anything new, path avoidance of mines, balance. **Out:** belly armour (S10b-3), bounding
S-mine airburst, flame (S14), dozer, trees for abatis, structure selection (S6-18). No armour, pen, gun, ammo,
lethality or existing card price changes. Germany's pooled 480 share must be unchanged (it was 41.67 %).

## 2. Where the design and the code disagree (code and owner decisions win)
1. 7.6 "no friendly-fire exemption for mines": the sim's `burst` and `blast_tracks` never hurt own men or vehicles.
   So own and allied units never set off own mines either (S10b-1 (A)); the pathing rule is not needed.
2. 7.6 "belly/track blast through physical armour check": vehicle boxes have no belly plate ("the bottom reads
   top", 12-20 mm), so an AT mine damages running gear only (`blast_tracks`), no penetration (S10b-3 (A)).
3. 7.6 materials "4 kg / 9 kg / 60 kg steel / trees": all drawn as construction kg (S10a 3.3), no new supply kind.
4. 7.1 hedgehog "Lv 3", bunker "Lv 3 (Lv 2 at half rate)": `build_heavy` blueprints; `build_structure` squads build
   them at `heavy_by_field_pct` 50 (the only US route: the USA has no construction card).
5. Mines need no item in any kit: a mine is an item def `kind = "mine"` only so its burst reuses the items path.

## 3. Data model (all new fields land in Build A's layout commit)
**Blueprints** (`defs/fortifications/`, `FortDef` gains `mine_item: Option<String>`, `blocks_vehicles: bool`;
`pieces` 1..=6, was 4): `ap_minefield`, `at_minefield` (line, `place_mines`, no pieces, `mine_item` set; one mine
at each segment end), `hedgehog` (point, `build_heavy`), `log_barrier` (line, `build_structure`), `bunker` (point,
`build_heavy`, pit `bunker_pit`, `DigProfile::BunkerPit = 8`). Values: rules table R1, R2, R14, R15, R22-R24.
**Mine items** (`defs/items/`): `ap_mine`, `at_mine`, `ItemKind::Mine = 5` (`ITEM_KINDS` 6); `[he]` as any item plus
`[mine] trigger = "foot" | "vehicle", trigger_cm`. Faction-free names ("AP mine (Schu-mine 42 / M2)", "AT mine
(Teller 43 / M1A1)"). Validator: a mine item has `[mine]` and `[he]`, appears in no kit, crate or roster.
**`[fortify]` keys** (`FortTuning`, `--set` works): `heavy_by_field_pct, detect_cm, detect_pct_per_s,
sweep_speed_pct, lane_max_m, lane_half_cm, lift_s, demolish_place_s, demolish_fuse_s, demolish_pct,
demolish_reach_m, vehicle_clear_m` (values R8-R21). **Capabilities** bits 12-14 promoted from
`RESERVED_CAPABILITIES` (`CAPABILITIES` 15). Card caps (R27) change in the stage that gives the behaviour.
**Sim state** (`FortState`, saved and hashed through the existing `put_postcard(&g.fort)`):
`mines: BTreeMap<u32, Mine>`, `next_mine`; `Mine { owner, fort: u16, item: u16, pos: FxVec2, known: u8 (player
bits), boom_tick: Option<u64> }`; `Charge::piece: Option<u32>` (a demolition); `FortAccount` += `mines_laid,
mines_set_off, mines_blasted, mines_found, mines_lifted, men_lost_to_mines, vehicles_mined, obstacles_built,
bunkers_built, sweeps, demolitions, pieces_demolished` (u32). `Order::ClearMines { a, b, along: Fx, lift:
Option<u32>, work: u64 }`, `Order::Demolish { piece: u32, man: Option<SimId>, work: u64 }`. Commands
`ClearMines { squad, a, b }`, `Demolish { squad, at }` (mines are laid with the existing `Fortify`).
`FortRefusal::NoTarget` (presentation only). RNG purpose `MineDetect` (new key; nothing else draws).
`FortState::is_empty` also checks `mines`; every new per-tick path returns at once when its list is empty.

## 4. Rules table (every number, cap and limit; "verify" = no design figure)
| Id | Rule | Value | Source | Proved by |
|---|---|---|---|---|
| R1 | AP field: mine every `segment_cm`, line max, kg and work per mine, cap | 200 cm, 30 m, 4 kg, 20 s x 2 men, 60 mines/player | 7.6 (20 s, 4 kg); rest verify | `minefield_lays_one_mine_per_segment` |
| R2 | AT field: as R1 | 400 cm, 40 m, 9 kg, 30 s x 2 men, 30/player | 7.6 (30 s, 9 kg); rest verify | same, `mine_cap_per_player` |
| R3 | AP trigger: an enemy man on foot (not riding) within `trigger_cm` of the mine | 30 cm; vehicles never | 7.6 "foot inside 0.3 m" | `ap_mine_goes_off_on_contact_only`, `ap_mine_ignores_vehicles` |
| R4 | AT trigger: an enemy vehicle with the mine within `trigger_cm` of a track centreline (first to last anchor) or a wheel anchor; men never | 40 cm verify | 7.6 "track or wheel over the plate" | `at_mine_ignores_infantry`, T102d |
| R5 | AT burst: `[he]` damage x `vs_vehicle_pct` on anchors within `blast_radius` (`blast_tracks`) | 400, 100 %, 150 cm verify (> track hp 100) | 7.6; S3 blast rule | `at_mine_immobilises_a_tank_over_it`, T102a/b |
| R6 | AP burst: ground burst, damage, blast, fragments | 150, 100 cm, 60 frags (verify) | 7.6 "HE or bounding burst" | `ap_mine_kills_the_man_who_steps_on_it` |
| R7 | Own and allied units never trigger own mines; a mine's burst belongs to its owner | - | code rule (2.1), S10b-1 | `own_side_never_sets_off_own_mines` |
| R8 | Detection: a sweeping `clear_mines` man, a mine not known to his player within `detect_cm`, chance per second | 150 cm, 50 %/s (roll per tick, p/hz) | 7.6 "1.5 m, chance per s, data"; % verify | `sweep_finds_at_expected_rate`, T104a |
| R9 | Sweep: up to 2 sweepers abreast (+-1 m) at `sweep_speed_pct` of walk along a lane of at most `lane_max_m`; lane half width | 25 %, 50 m, 200 cm (verify) | 7.6 "sweep at walking speed" | `clear_ends_at_lane_end_with_no_known_mine_left` |
| R10 | A sweeper never steps within `trigger_cm` + 50 cm of a mine his player knows; the nearest sweeper lifts it | - | 7.6 "detection then lifting" | `sweeper_stops_short_of_a_found_mine` |
| R11 | Lift: `lift_s` at 100 %, tier `cut_pct` (field 100, assault 200, construction 50), x0.3 suppressed, 0 pinned | 20 s verify | 7.7 (assault fast, construction slow) | `lift_time_follows_tier` |
| R12 | Any HE burst with a clear line from its lift point sets off mines within its `blast_radius`, next tick (`boom_tick`) | - | 7.6 "HE nearby sets them off" | `burst_sets_off_mines_next_tick`, T104e |
| R13 | A mine that goes off becomes known to all; a found mine to the finder's team only | - | 7.6 visibility rule | `found_mine_known_to_finder_only` |
| R14 | Hedgehog: point, 1 steel piece 180x180x130 cm, armour 2000, hp; kg, work; cap; spacing | hp 600, 60 kg, 60 s x 4 men, 20, 3 m | 7.6 (1 min, 60 kg); rest verify | `obstacle_blocks_vehicle_nodes` |
| R15 | Log barrier: line of `timber` 400x80x100 cm pieces, armour 100; per segment hp, kg, work; max length; cap | 400 cm seg, hp 400, 100 kg, 120 s x 4 men, 20 m, 10 seg | 7.6 (2 min); rest verify | same |
| R16 | A `blocks_vehicles` piece's box plus `wreck_pad_m` marks vehicle nodes blocked (tracked and wheeled layers) | existing pad | 7.6 "vehicle nav obstacle" | `obstacle_blocks_vehicle_nodes`, T105a/b |
| R17 | Infantry walks through obstacles at full pace (pieces block no men, S10a-2 (A)) | - | S10a-2 | `infantry_walks_through_obstacles`, T105c |
| R18 | No minefield, obstacle or bunker site within `vehicle_clear_m` of any vehicle hull (`BadSpot`) | 6 m verify | avoids `on_blocked` | `obstacle_refused_near_a_vehicle` |
| R19 | Demolish: the nearest squad man with a `satchel` item walks to 1.5 m of the nearest piece to `at` (within `demolish_reach_m`), works `demolish_place_s`, lays a `Charge` (item used up), fuse `demolish_fuse_s` | 25 m, 10 s, 10 s verify | 7.6 "demolition" | `demolish_lays_a_satchel_and_destroys_a_hedgehog` |
| R20 | The demolished piece takes `demolish_pct` of the charge's damage (tamped); other pieces the normal S6 rule | 300 % (900 from a satchel) verify | 7.6 | same, control `demolish_pct = 100` |
| R21 | `heavy_by_field_pct`: `build_structure` squads build `build_heavy` blueprints at this share of their rate | 50 % | 7.6 "Lv 2 at half rate" | `heavy_blueprint_at_half_rate_for_field_engineers` |
| R22 | Bunker: pit; kg; work; cap; spacing; slope | 350x300x150 cm, 250 kg, 540 s x 5 men, 2, 30 m, 15 % | 7.6 (8-10 min, 250 kg); rest verify | `bunker_digs_pit_then_builds_six_pieces` |
| R23 | Bunker pieces, all armour 100: `roof` on 3 `timber` walls; 2 front walls leave the slit above the surface | roof 420x370x80 hp 1000, walls hp 600, slit 80x40 cm | 7.6 "slit, thick earth roof"; sizes verify | `satchel_at_slit_reaches_men_inside` |
| R24 | Bunker roof survives three 81 mm hits (150 each) and a satchel (300); 7 AP shots (`shot_piece_damage` 150) break it | 1000 hp | 7.6 "heavy HE, AT rounds" | `bunker_roof_survives_three_81mm`, T107b/f |
| R25 | Caps on saved lists | `MAX_MINES` 2048 | - | `load_rejects_bad_s10b_state` |
| R26 | Own rounds pass own pieces (S10a-3 (A)): the slit matters to incoming fire only | - | S10a-3, S10b-4 | T107g |
| R27 | Cards: `place_mines` Pioniere, US Engineers, Baupioniere; `clear_mines` those three plus Sturmpioniere, Assault Section; `demolish` Pioniere, US Engineers, Sturmpioniere (the cards with a satchel). No kit, price or weight change | - | 7.7; 3.3 | `s10b_caps_on_cards` |
| R28 | Neutrality: AI-vs-AI per-match outcomes equal main on the 40 set and the three 160 sets; `s10b.used` 0 | - | owner 2026-11-09 | `tools/neutral.py`, T108 |
| R29 | `fort_of` (AI) never returns a blueprint with `mine_item` or `blocks_vehicles` (they sort before `sandbag_wall`) | - | neutrality | `fort_of_ignores_s10b_blueprints` |
| R30 | Versions: `SAVE_VERSION` 23 -> 24, `PROTOCOL_VERSION` 13 -> 14, once, in Build A's layout commit | - | STATE "Versions" | `s10b_layout_roundtrips_and_hashes` |

## 5. Multi-tick jobs: when each one ends
* **Laying** (a minefield `Fortify` work): per segment, kg drawn, then `work_s x work_men x 100 x tick_hz` points;
  at completion one mine at the segment end (`mines_laid`). Ends: last segment done (`works_done`); builder takes
  another order or the squad dies (work removed, this step's kg lost, laid mines stay); the next mine would pass
  the cap (work removed, `Cap` event).
* **Clearing** (`Order::ClearMines`): ends when `along` reaches the lane length and no mine known to the player lies
  within `lane_half_cm` of the lane; or another order; or no living sweeper. Lifting a mine pauses `along`.
* **Demolition** (`Order::Demolish`): ends when the charge is laid (it bursts at `fuse_tick`); or the piece is gone
  (`NoTarget`); or the man dies or has no satchel (nothing laid); or another order.
* **Bunker, hedgehog, log barrier**: as S10a point and line works (pit, wait for kg, build; interrupted = removed).
* **Mine burst**: same tick as the trigger, through the items path at the charges' tick-order point; a `boom_tick`
  mine bursts on that tick. Keys: a range disjoint from thrown and charge ids.

## 6. Save, hash, validation (each new field; `load_rejects_bad_s10b_state` has one case per line)
* `mines`: at most `MAX_MINES`; id < `next_mine`; owner a player; `fort` a def with `mine_item` and `item` equal to
  it; `pos` on the map; no `known` bit at or above the player count; `boom_tick` only `tick + 1`; per owner and def
  at most `limit_per_player`.
* `Charge::piece`: id < `next_piece`; only with an item of kind `satchel`.
* `Order::ClearMines`: squad has `clear_mines`; `a`, `b` on the map, `|b - a|` <= `lane_max_m`; `0 <= along <=
  |b - a|`; `lift` a standing mine known to the player; `work` below the lift total.
* `Order::Demolish`: squad has `demolish`; `piece` < `next_piece`; `man` a living member with a satchel; `work`
  below the place total. `FortAccount` new counters: any value (report only).
* Setup: content validators for the new def fields, `[mine]`, `[fortify]` keys (ranges in Build A).

## 7. Done when
1. An AT mine immobilises a tank driven over it (T102a 100 %, field T102b >= 75 %); infantry never sets one off.
2. An AP mine goes off only on contact (T103a-c). 3. A sweep finds mines at the expected rate (T104a, +-10 points)
   and lifts what it finds (T104b). 4. Obstacles stop vehicles, not infantry (T105a-c). 5. Demolition destroys a
   hedgehog and opens the line (T106a/b). 6. The bunker holds against small arms, three 81 mm bombs and a roof
   satchel, and a satchel at the slit kills men inside (T107a-e).
7. `neutral.py` same on the 40 set and the three 160 sets; T108 zero; Germany pooled 41.67 %, hard misses exactly
   T8b, T40c, T54e, T88b, T88d (as on main). 8. Every named test passes (`check.sh full`); SAVE 24, PROTOCOL 14.

## 8. Build stages (about 60 turns each; read only your file)
A layout, data, measures, scaffolding (`S10b-build-A.md`); B mines laid and triggered (`-B`); C sweeping, lifting,
mines set off by HE (`-C`); D obstacles and demolition (`-D`); E bunker and the fortify rows (`-E`); F presenter,
bridge, net, HUD, docs (`-F`). Neutrality BASE: `target/s10a-ai-verify` (code of `8acc534`; the 40-only folder
`target/s10b-base40` holds its `skirmish40.json`, made in Build A).
