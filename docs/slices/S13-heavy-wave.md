# S13: heavy wave (Panther G, Tiger I, M4A3 (76) W, M36, M4A3E2): plan

Status: **plan only** (2026-11-19, revision 2 after an independent review), nothing built. Base: `main` at `8d2731b` (S10c-ai,
S11a1, S11a2 merged). S13 builds after S11b, the balance pass (branch `claude/project-thread-e7nsak`) and S12 (wounds and medics) are
merged, from a fresh main. Card: `docs/design/roster/roster-expansion.md` 4.2 "S13"; design: `docs/design/roster/vehicles.md` 4.4,
4.5, 5, 6.11, 7.5-7.6, 8.4. Working notes: `S13-research-history.md` (values, URLs, trial data), `S13-research-code-audit.md`
(file:line audit), `S13-research-obliquity.py` (calculation), `S13-research-review.md` (independent review of revision 1); notes are inputs, this file decides. Owner questions: section 10.

## 0. Summary
- **The one surprise.** The card calls S13 "mostly data". The vehicles are, but with today's pure cosine law (`crates/sim/src/armour.rs`
  234-286: no calibre, overmatch or APCR slope effect) and the Bird & Livingston (B&L) 0° rows the mod uses, the US 90 mm M82 defeats
  the Panther glacis head-on to about 900 m and 76 mm HVAP past 1 km. US trials say the M82 failed the glacis and HVAP only worked at
  400 yd or less, often not. The slice's central fight would be wrong in the direction the owner rule forbids ("authentic stats").
- So S13 adds **one small sim rule, O1** (obliquity by plate-to-calibre ratio and round class, section 3). It is **opt-in per round**
  (`slope_class` ammo key): S13a sets it only on the eleven new rounds, so every existing duel is unchanged by construction.
- **Split (owner rule 2026-11-09, `TODO.md` 40):** **S13a** mechanics: O1, all data, five cards at `ai_weight` 0, every new key in one
  layout commit, proven neutral (all S13a switches on = base, match for match, because no AI buys a new card and no old round is
  classed). **S13-ai**: the AI rules H1-H3 and card weights, measured on 480. **S13m**: heavy mortars, after S11b (S13-4).
- Size about 0.6x S3 over both parts. Engine tier (review gate fires); `workflows.md` 1 lists S13 as data: the Docs stage corrects it.

## 1. What exists, and what is missing (audit of `main` 8d2731b)
| Needed | State | Where |
|---|---|---|
| Plates for the five hulls | glacis, nose, sides, flat Tiger faces exact; 4 boxes max | `content/vehicle.rs` 52, 1541-1557, 1627-1650 |
| Mantlet that turns with the turret | missing: exactly one turret-frame box | `content/vehicle.rs` 1541-1557 (S13-2) |
| Slope effect beyond `1/cos` | missing (O1) | `sim/armour.rs` 234-286 |
| APCBC + APCR + HE on one gun | exists (`MAX_MAIN_AMMO` 3: std / HE / special) | `content/vehicle.rs` 550-552; `ger_pz4_h.toml` 160-164 |
| Rare rounds, `rare_cap`, slow forward refill (D3) | exists | `content/modset.rs` 1552-1587; `sim/supply.rs` 851-963 |
| Auto fires the special round when the standard one fails | exists (`special_better`, any margin) | `sim/gunnery.rs` 1169-1222 |
| Card caps and keys: `max_alive`, `limit_group`, `unlock_s`, `cooldown_s`, `available_from/to`, `ai_weight` | exist; `[limit_groups]` empty | `content/defs.rs` 156-197; `skirmish.toml` 1235 |
| Engine-rpm traverse | missing: one fixed rate | `sim/gunnery.rs` 214-256 (S13-3) |
| AI buys heavies | missing: one vehicle slot, fewest buys per `ai_weight`; saving happens only by the stall at an unaffordable pick | `sim/ai.rs` 1997-2003, 2031-2060 |
| AI holds heavies back | missing: overwatch only for `tank_destroyer` / `assault_gun` | `sim/ai/overwatch.rs` 86 |
| Rearm trip | exists for low AP (`rearm_ap_below` 3, `rearm_full_pct`, `rearm_max_s`, S3c); no distance rule (TODO S8b-13) | `sim/ai/armour.rs` 20-57; `skirmish.toml` 982-990 |
| HUD cap badge on an in-limit tank card | missing | `godot-bridge/src/match_node.rs` 2320-2340 |
| Presenter models | none needed: procedural blockouts | `godot-bridge/src/meshgen.rs` 444-874 |
| armour-test duel by card id | missing (first card of a role) | `sim/scenarios.rs` 107-148, 417-454 |
Save and protocol: new defs alone bump nothing. New rules keys, the `slope_class` ammo key and the `ai_overwatch` def key enter
`MatchSetup` (saved; net `setup_hash`): **SAVE +1 in S13a's one layout commit**, with every S13-ai key and saved AI field too. PROTOCOL
stays (no command). Append-sensitive tests: `crates/sim/src/s11a1_tests.rs` 246, 261 (17 vehicle defs); `content/tests/s11a1_content.rs`
295-305 and `s10a_content.rs` 45-57 ("S11a1 cards last"; S11b will touch them first: re-read after the rebase). Baselines:
`resolved_defs.rs` 183-195 and two JSON snapshots (`BLESS=1`). `STATE.md`'s "refresh `simbench/src/report.rs`" is stale: fix in Docs.

## 2. Scope
**S13a.** Sim: O1; `special_min_pct` (3.6); spawn-by-card helper for armour-test; HUD cap badge (one bridge change). Data: guns 7.5 cm
KwK 42 L/70, 8.8 cm KwK 36 L/56, 76 mm M1A1, 90 mm M3; 11 rounds; Panther G, Tiger I (late), M4A3 (76) W, M36, M4A3E2 (75 mm); five
cards at `ai_weight` 0; `[limit_groups] heavy = 1`. Layout: every key of section 5 too, all off.
**S13-ai.** H1-H3, `ai_weight`s, `special_min_pct` on if T156 asks, armour-test cases 41-46, 480 measure.
**Out (reason).** Heavy mortars: S13m (S13-4). Engine-rpm traverse (S13-3). Mantlet boxes (S13-2). Classing existing rounds (S13-1b).
90 mm HVAP T30E16 (S13-6: Feb 1945). The `3in_m62` row fix (S13-9: changes an old gun; balance pass). Jumbo 76 mm, Panther A,
Tiger II, M4A3E8, M18, Jagdpanzer IV, Panzer IV/70 (S13-8). Tank smoke (4th ammo slot, `AmmoChoice`: PROTOCOL). Fuel (Q19 (A)). Art.

## 3. O1: obliquity by plate-to-calibre ratio (S13a Build A, sim)
### 3.1 Rule
Today: penetrates when `pen0 * (1 + v) * c >= T + extra * c` (`c` the cosine of the impact angle to the plate normal, compound with
yaw; `v` +-7 % triangular). O1 changes only the `c` on the penetration side, and only for a round with a `slope_class`:
`c_pen = c * ((1 - w) + w * s(c))`, with `s(c) = sqrt(c)` for `slope_class = "apcbc"` (exponent 1.5 at full weight) and `s(c) = c`
for `"apcr"` (exponent 2), and `w = clamp((T/d - td_lo) / (td_hi - td_lo), 0, 1) * w_max`, `T` the plate thickness after
`face_hardened_pct`, `d` the round's `caliber_mm` (validated today, never read by the sim). Absent `slope_class`: `c_pen = c`, exactly
today's rule. Ricochet unchanged; the track band's `extra * c` unchanged.
- Physics: a round much wider than the plate overmatches and normalises (w near 0); a round meeting a plate about its own calibre at
  a steep slope loses more than `1/cos`; an APCR core loses most (B&L ch. 3; US Ordnance and BRL trials, section 9).
- **Fixed point, no tables:** `Fx::sqrt` exists (`sim/fx.rs` 196-203); `w` is an `Fx` ratio; no exp, ln or quantisation; continuous in
  `c` and `T/d`. Deterministic as today's rule.
- **Call sites** (all take `c_pen` from one helper `armour::pen_cos(c, t, cal, class)`): `damage.rs` 575 (plates) and 544 (shields;
  shields take `c_pen` too), `armour::effective_thickness` 255 and `pen_chance_ppm` 265 (penetration side), `plate_shot` 867-893 (the
  `ballistics` report prints `c_pen`), `gunnery.rs` 1211 (`special_better`), the armour-test hit record and its invariant
  (`sdk/crates/cli/src/armour_test.rs` 349-354, "every logged hit equals the rule": the record carries `c_pen`), `armour_test/s8b.rs`
  906. Small arms, mortar `pen_mm`, fragments (`combat.rs` 2867) and HEAT have no class: unchanged.
### 3.2 Keys (S13a layout commit)
| Table | Key | Default | Off | Meaning |
|---|---|---|---|---|
| `[armour]` | `obliquity` | 1 | 0 | 0 = ignore `slope_class` (cosine law everywhere) |
| | `td_lo_pct` / `td_hi_pct` | 40 / 90 | - | ramp of `w` over T/d |
| | `apcbc_w_pct` / `apcr_w_pct` | 100 / 100 | - | `w_max` per class |
| ammo def | `slope_class` | absent | absent | `"apcbc"` (AP, APC, APCBC) or `"apcr"` (APCR, HVAP); load error on HE/HEAT |
Ranges: switch 0..=1; `td_*` 0..=300, lo < hi; `*_w_pct` 0..=100. Every one of the 16 (lo, hi) pairs in {25,30,40,50} x {75,80,90,100}
passes T151a-g below (script); 40 / 90 is the middle of that window, so the calibration is not fragile.
### 3.3 Chance to penetrate, cosine law -> O1 (defaults; B&L rows; per hit; 0° yaw / 30° yaw)
| Round | Plate | 100 m | 350 m | 1000 m | History |
|---|---|---|---|---|---|
| 76 mm M62 | Panther glacis 80 @ 55 | 0 -> 0 | 0 -> 0 | 0 -> 0 | never (Isigny Jul 1944; ARMET) |
| 76 mm M93 HVAP | Panther glacis | 100 -> 28 / 100 -> 0 | 100 -> 0 | 100 -> 0 | 1 of 2 at 400 yd, 0 at 600 yd (12th AG, Aug 1944) |
| 90 mm M82 | Panther glacis | 100 -> 0 / 72 -> 0 | 100 -> 0 | 28 -> 0 | fails the glacis (Ord. pamphlet Jan 1945) |
| 8.8 cm PzGr 39 | Panther glacis | 100 -> 0 | 100 -> 0 | 36 -> 0 | no trial |
| 76 mm M62 | Panther nose 60 @ 55 | 100 -> 15 / 87 -> 0 | 100 -> 0 | 67 -> 0 | "unsatisfactory" vs heavy armour |
| 90 mm M82 | Panther nose | 100 -> 100 | 100 -> 100 | 100 -> 100 | M82 pierces all Panther plates but glacis and mantlet |
| 76 mm M93 HVAP | Panther nose | 100 -> 100 | 100 -> 100 | 100 -> 100 | 1 of 4 at 200 yd on a measured 65 mm nose: O1 generous (60 mm plate data) |
| 76 mm M62 | Panther turret 100 @ 12 | 100 -> 100 / 98 -> 20 | 100 -> 100 / 65 -> 0 | 88 -> 79 | mantlet holed at 200 yd only: see S13-2 |
| 7.5 cm PzGr 39/42 | Jumbo glacis 101.6 @ 47 | 100 -> 79 / 100 -> 0 | 100 -> 13 | 50 -> 0 | the Jumbo's front rarely pierced |
| 8.8 cm PzGr 39 | Jumbo glacis | 100 -> 0 | 91 -> 0 | 0 -> 0 | |
| 8.8 cm PzGr 40 | Jumbo glacis | 100 -> 54 | 100 -> 2 | 100 -> 0 | |
| 7.5 cm PzGr 39/42 | Sherman glacis 63.5 @ 47 | 100 -> 100 | 100 -> 100 | 100 -> 100 (also at 30°) | defeated everywhere |
| 76 mm M62 | Tiger front 100 @ 8 | 100 -> 100 / 100 -> 40 | 100 -> 100 / 78 -> 2 | 95 -> 92 | front holed at short range |
Existing rounds (PzGr 39 L/48, M61, 3-inch M62, 57 mm, 5 cm, 2 cm, 37 mm, Panzerfaust, Bazooka, Panzerschreck) have no class in
S13a, so every old matchup, including the 30°-reference rows (`armour.rs` 301-312), is bit for bit as today. Two consequences, stated:
the M10's 3-inch M62 (same projectile as the 76 mm M62) and the Panzer IV's PzGr 40 keep the cosine law against the new heavies
(3-inch M62 vs Panther nose: 100 % at 350 m instead of 0 %); the balance pass decides S13-1b with the leak table of the review
(classing the L/48 PzGr 39 would drop it vs the Sherman glacis at 30° yaw from 100 % to 0 % at 350 m: a large change).
### 3.4 Calibration targets (hard; `ballistics` checks at the named yaw; each cites the trial line in 3.3)
T151a M62 vs Panther glacis at 100 m, 0°: 0 %. T151b M93 vs Panther glacis: >= 25 % at 100 m, <= 15 % at 400 m. T151c M82 vs
Panther glacis at 100 m: <= 20 %. T151d M82 vs Panther nose at 350 m: >= 80 %. T151e M62 vs Panther nose at 350 m: <= 30 %.
T151f 7.5 cm PzGr 39/42 vs Sherman glacis at 1000 m, 0° and 30°: >= 90 %. T151g 8.8 cm PzGr 39 vs Jumbo glacis at 300 m: <= 20 %.
T151h every pre-S13 armour-test case and `armour.rs` test identical with `obliquity` 1 and 0 (no old round is classed).
### 3.5 Tests (each with its control)
`no_class_is_the_cosine_law` (every armour.rs test green, bit for bit; control: an `apcbc` round on a T/d 1 plate at 55° is lower),
`overmatch_keeps_the_cosine` (T/d 0.3: same chance; control T/d 1.2: lower), `apcr_loses_more_than_apcbc_on_a_slope` (control at
0°: equal), `heat_and_he_reject_a_class` (load error), `hit_record_matches_the_rule` (invariant with `c_pen`), save fuzz with the new
keys; `neutral.py` on the 40 set and a full 480 with S13a's defaults vs base: match for match (T150a).
### 3.6 `special_min_pct` (gunnery, S13a layout, default 0 = base)
Auto fires the special round when the standard chance is under `special_below_pct` 50 and the special's is higher by any margin
(`gunnery.rs` 1220-1222); under O1 an HVAP at 5 % against 0 % would burn the two rare rounds. New key: the special must reach at
least `special_min_pct` (S13-ai sets 25 if T156 shows waste).

## 4. Vehicles and data (S13a Build B, Sonnet, a pure data stage; every value with `sources`)
Penetration rows are B&L 0°, 50 % criterion, as the mod's. Check 2026-11-19: Wikipedia's "calculated penetration (90°), US/UK 50 %"
table for the KwK 40 L/48 equals the mod's B&L rows exactly (135/123/109/97/86; 176/154/130/109/92), and those tables cite B&L p. 61,
so the same pages' rows for the new guns are taken as B&L. Build B confirms each against the book if it can.
### 4.1 Guns and rounds (complete round kg)
| Gun (weapon def) | Round (ammo def) | Kind, class | m/s | Pen 100/500/1000/1500/2000 | kg | Notes |
|---|---|---|---|---|---|---|
| KwK 42 L/70 `kwk42_l70` | PzGr 39/42 `75mm_pzgr3942` | aphe, apcbc | 935 | 185/168/149/132/116 | 14.3 | 18 g filler |
| | PzGr 40/42 `75mm_pzgr4042` | ap rare, apcr | 1120 | 265/234/199/170/145 | 11.6 | |
| | SprGr 42 `75mm_sprgr42` | he | 700 | - | ~11 (verify) | 0.65 kg filler |
| KwK 36 L/56 `kwk36_l56` | PzGr 39 `88mm_pzgr39` | aphe, apcbc | 773 | 162/151/138/126/116 | ~15.3 (verify) | 59 g filler |
| | PzGr 40 `88mm_pzgr40` | ap rare, apcr | 930 | 219/200/179/160/143 | (verify) | |
| | SprGr `88mm_sprgr` | he | ~820 (verify) | - | (verify) | ~0.9 kg (verify) |
| 76 mm M1A1 `m1a1_76mm` | M62 APC `76mm_m62` | aphe, apcbc | 792 | 125/116/106/97/89 | 11.1 | 500 m interpolated |
| | M93 HVAP `76mm_m93` | ap rare, apcr | 1036 | 239/208/175/147/124 | ~8.4 (verify) | |
| | M42A1 HE `76mm_m42a1` | he | 820 | - | 10.0 | 0.39 kg filler |
| 90 mm M3 `m3_90mm` | M82 APC `90mm_m82` | aphe, apcbc | 808 or 853 (verify; the B&L row's loading) | 164/152/137/126/114 | 19.4 | |
| | M71 HE `90mm_m71` | he | 820 | - | 19.0 | ~0.9 kg (verify) |
Range 600 m, `ricochet_angle_deg` 70 and fire reveal as the existing guns. Reload from S3's `reload_of` inputs with a cited rate.
### 4.2 Vehicles (thickness @ slope from vertical, mm)
| Vehicle | Hull front (upper / nose) | Sides (upper / lower) | Rear / top | Turret front / side / rear / roof | Traverse °/s | Rack std / HE / rare | km/h | t |
|---|---|---|---|---|---|---|---|---|
| Panther G | 80 @ 55 / 60 @ 55 | 50 @ 30 / 40 @ 0 | 40 @ 30 / 40 crew, 16 engine | 100 @ 12 / 45 @ 25 / 45 @ 25 / 16 | 12 (S13-3) | 40 / 40 / 2 = 82 | 46 | 45 |
| Tiger I late | 100 @ 8 / 100 @ 24 | 80 @ 0 / 60 @ 0 | 80 @ 8 / 25 | 100 @ 8 / 80 / 80 / 40 | 9 (S13-3) | 46 / 44 / 2 = 92 | 38 | 57 |
| M4A3 (76) W | 63.5 @ 47 / 63.5 @ 47 (cast nose 51-108) | 38 @ 0 / 38 @ 0 | 38 @ 10 / 25 | 89 @ 0 / 63.5 @ 10 / 63.5 / 25 | 24 | 36 / 33 / 2 = 71 | 42 | 34 |
| M36 | 38 @ 55 / 51 @ 30 | 19 @ 38 / 25 @ 0 | 19 @ 38 / 19 | 76 @ 0 / 32 @ 5 / 127 / **open** | 24 (powered; verify) | 20 / 27 / 0 = 47 | 42 | 29 |
| M4A3E2 (75) | 101.6 @ 47 / 114 @ 30 (verify) | 76 @ 0 / 38 @ 0 | 38 @ 10 / 25 | 152 @ 0 / 152 / 152 / 25 | 18 (verify; from memory) | 38 / 66 / 0 = 104 | 35 | 38 |
Turret fronts are the published face plates; mantlets are not modelled (S13-2): Panther 100 mm mantlet over a 100 mm face, Tiger
100-110 (to 200 at the gun), Jumbo 178. M36 traverse: powered, up to 24°/s (afvdatabase); `vehicles.md` 4.4 groups it with the
hand-traversed M10: wrong, corrected in Docs. Panther 12 and Tiger 9 are midpoints between the idle (6-8) and high-rev (19-20) figures,
not sourced point values (S13-3). Frames, seats, hatches, components, MGs and rack boxes copy the nearest def (Panther, Tiger from
`ger_pz4_h.toml`; 76 W and Jumbo from `usa_m4a3_75.toml` with wet floor racks; M36 from `usa_m10.toml`, `open = ["top"]`). `bog_pct`
by ground pressure: Panther 8, Tiger 10, Jumbo 9, 76 W 6, M36 7. `sight_m` 175, `spot_size_pct` 200. Rack price check 75 % holds.
### 4.3 Cards (`defs/units/*_squad.toml`, appended after S11b's cards; S13a weight 0, S13-ai weight in brackets)
| Card | Cost | Pop | Cooldown s | Unlock | max_alive | limit_group | ai_weight |
|---|---|---|---|---|---|---|---|
| Panther G | 1050 | 16 | 240 | 8:00 | 2 | - | 0 (H1 picks it) |
| Tiger I | 1200 | 16 | 300 | 12:00 | 1 | heavy | 0 (H1) |
| M4A3 (76) W | 690 | 12 | 120 | 6:00 | 3 | - | 0 (100) |
| M36 | 760 | 12 | 180 | 8:00 | 2 | - | 0 (100) |
| M4A3E2 | 900 | 14 | 240 | 10:00 | 1 | heavy | 0 (H1) |
Scale: Panzer IV 620, Sherman 590. Q17 (A): one `heavy` slot (Tiger or Jumbo; `heavy = 1`), the Panther outside it with cap 2 and
8:00 unlock. All count against `[vehicles] limit` 4. `available_from`: Panther G 1944-03, Tiger I to 1944-08, 76 W 1944-07, M36
1944-09, Jumbo 1944-10 (skirmish sets no date: no filter). History in the defs: 254 Jumbos, 75 mm as delivered; ~130 Tiger I in
the West in three battalions; 76 mm HVAP ~2-3 per tank from Sep 1944 (`rare_cap` 2); PzGr 40 nearly unobtainable (S13-6).

## 5. S13-ai: AI (Opus; keys and saved fields already in S13a's layout commit, all off)
| Rule | What | Keys and state | Off |
|---|---|---|---|
| H1 Heavy pick | A card with `ai_heavy = true` on its def (Panther, Tiger, Jumbo) is bought by its own path, not by weight: once an enemy tank, TD or AG was seen in the last `heavy_seen_s` (300) and the card is ready, the vehicle slot targets it and the AI saves for it (no medium bought meanwhile) for at most `heavy_save_max_s` (240); then it drops the plan until the next sighting. Needs a free `[vehicles] limit` place; heavies never replace a medium. Against an opponent with no armour it never buys one (stated, by design). | def `ai_heavy`; `[ai.armour] heavy_buy`, `heavy_seen_s`, `heavy_save_max_s`; saved `AiState::armour_seen_at` (role-filtered tick; `enemy_vehicle_seen` at `ai.rs` 309 is a latch for any vehicle) and `heavy_plan_since`; `AiCounters` `heavy_plans`, `heavy_buys`, `heavy_drops` | `heavy_buy` 0 |
| H2 Heavy overwatch | A def with `ai_overwatch = true` (Panther, Tiger) joins the TD/AG overwatch habit (`overwatch.rs` 86): `overwatch_back_m` behind the assault, facing the threat. The Jumbo keeps leading (its real job). | def `ai_overwatch`; `heavy_overwatch` | 0 |
| H3 Push limit (TODO S8b-13) | Extends the S3c rearm trip (`ai/armour.rs` 20-41): a tank also goes back when it is more than `push_without_supply_m` (300, `vehicles.md` 7.5) from the nearest rearm source (base, own HQ, dump, truck with shells) and holds at most `tank_reserve_rounds` (10) main rounds. Same trip code, same `rearm_full_pct` / `rearm_max_s`. | `push_limit`, `push_without_supply_m`, `tank_reserve_rounds`; counter `push_turnbacks` (no per-tank state: distance computed each plan) | 0 |
Tests: `heavy_bought_after_armour_seen` (income 300/min, Panther ready: bought within 5 min; control `heavy_buy` 0: never, weight 0),
`heavy_plan_drops_after_save_max` (control: kept), `no_armour_no_heavy`, `heavy_waits_for_a_limit_place`, `panther_takes_overwatch`
(control flag off: assaults), `jumbo_leads`, `tank_turns_back_at_push_limit` (control `push_limit` 0), save fuzz with the new fields,
threads 1 vs 4.

## 6. Stages (`workflows.md` 1-4: engine tier; no per-slice Tune; each Build checked by a separate agent; GitHub checks on the branch)
| Part | Stage | Model | Content | Exit |
|---|---|---|---|---|
| S13a | A sim | Opus | O1, `special_min_pct`, ONE layout commit (SAVE +1; every key and field of 3.2, 3.6, 5, def `slope_class`, `ai_heavy`, `ai_overwatch`), HUD badge, spawn-by-card helper, tests 3.5 | `check.sh quick`; T151a-h |
| | B data | Sonnet | 4 weapons, 11 rounds, 5 vehicles, 5 cards (weight 0), `[limit_groups]`, roster appends, append-tests, baselines | content tests; `validate` |
| | Review, Fix | Opus | review gate (sim diff); Fix on findings | |
| | Measure | script | 480 at the head vs base: identical (T150a); `verify.sh` | |
| S13-ai | C AI | Opus | H1-H3, weights, armour-test `armour_test/s13.rs` cases 41-46 | `check.sh tests` |
| | Review, Fix | Opus | | |
| | Measure | script | 480 `targets/S13.toml`; ablations on 480 (not 40: noise 11-15 points) for `heavy_buy` and `push_limit` | T150, T155 |
| | Docs | Haiku | logs, STATE.md (stale simbench line), TODO S13 block, `workflows.md` tier, `vehicles.md` 4.4 M36 | |

## 7. Measures and targets (`docs/slices/targets/S13.toml`; T-numbers after the balance pass's last, here T150+)
- **Hard.** T150a S13a head = base on 480, match for match. T150 S13-ai: Germany pooled 35-65 % (S11a2 41.67 %). T151a-h (3.4).
  T152 no winner 0; match median 15-30 min. **T155 the measure covers the heavies:** Panther and M36 each bought in >= 25 % of
  matches, Tiger and Jumbo in >= 10 %; if not, the T150 reading says nothing about heavy balance: stop and report.
- **armour-test** (10 runs each, each with an `o1_off` control run): 41 Panther G frontal at 350 m vs 2 Shermans (75) flanking from
  45°: report wins, time to first pen, hits by zone. 42 M36 vs Panther frontal at 350 m: M36 kills only via nose or turret (bound:
  glacis pens 0). 43 M4A3 (76) W with 2 HVAP vs Panther frontal 350 m: glacis pens 0, HVAP fired reported. 44 Tiger vs 2 Shermans (75)
  frontal 350 m: Tiger wins >= 6 of 10. 45 Jumbo leading into a PaK 40 at 400 m: survives >= 3 hits in >= 6 of 10. 46 M4A3 (76) W
  flanks a Panther at 200 m: kills in >= 7 of 10.
- **Report.** T156 rare rounds fired per match by type and their pen share; T157 kills by and of each new type, per side; T158 H3
  turn-backs; T159 rack-dry tanks.
- Levers if T150 fails, in `vehicles.md` 5's order: cost, `unlock_s`, `cooldown_s`, `max_alive`, H1 timing; never plates or rows;
  then more historical counters.

## 8. Balance risks
1. **Germany gets stronger** (41.67 % now): Panther and Tiger fronts resist US 75/76 mm. Counters stay real: Panther sides 50 @ 30
   fall to the 75 mm M61 at 1 km; nose and turret to the M82; AT guns; Bazookas (HEAT has no class). Inside 35-65 % is a pass.
2. **The turret front is soft** (S13-2 (A)): with the published 100 @ 12 the 76 mm M62 pierces the Panther turret head-on to 1 km;
   the curved mantlet that stopped it is not modelled. Armour-test 41-43 report hits by zone so the gap is visible.
3. **Spotting at 350 m** (Q12 (A)) hides the long guns; the heavies' edge is armour. Fire reveal (525 m, 3 s) and recon remain.
4. **Traverse decides duels** (S3 log); 12 and 9°/s are midpoints (S13-3); T157 shows over- or under-performance.
5. **AI**: H1 makes heavies appear only against armour; T155 guards the measure's meaning.
6. **Merge load**: S11b, the balance pass and S12 touch rosters, `[ai.*]`, baselines and append-tests; start from a fresh main.

## 9. Sources (detail and URLs in `S13-research-history.md`; (verify) values are open for Build B)
B&L, *WWII Ballistics: Armor and Gunnery* (2001) p. 61 (via the matching Wikipedia "calculated 90°" tables: KwK 40, KwK 42, KwK 36,
76 mm M1, 90 mm M3). Jentz, *Germany's Panther Tank*, *Germany's Tiger Tanks*; panzerworld.com; wwiitanks.co.uk (Panther G).
Hunnicutt, *Sherman*; TM 9-759; tanks-encyclopedia (M4A3E2); afvdatabase.com (M36). US trials: Isigny (12-30 Jul 1944), 12th Army
Group (20-21 Aug 1944), Ordnance pamphlet on 90 mm ammunition (Jan 1945, lonesentry.com), ARDEC ARMET-TR-17002 (76 mm vs Panther),
summarised in "The Chieftain's Hatch: US guns vs German armour" 1-2. Production and arrival: Wikipedia Panther, Tiger I, M36, M4.

## 10. Owner questions (work proceeds on (A); the part in brackets is the first blocked)
- **S13-1 Armour rule [S13a].** (A) Add O1, opt-in per round, on the 11 new rounds only, calibrated to US trials (3.3, 3.4).
  (B) Keep the cosine law: the 90 mm and 76 mm HVAP pierce the Panther glacis to ~1 km, the Jumbo falls to the 8.8 cm at 600 m.
  (C) Lower the new rows instead: breaks "do not touch guns". **How it should play: asked.**
- **S13-1b Old rounds [balance pass].** (A) Leave existing rounds on the cosine law in S13; the balance pass decides with the leak
  table (the 3-inch M62 at least, as the 76 mm M62's twin). (B) Class all full-calibre AP and APCR rounds now and measure the change.
- **S13-2 Mantlets [S13a].** (A) Published turret face plates (Panther 100 @ 12, Tiger 100 @ 8, Jumbo 152); the soft Panther turret
  is a stated gap. (B) A mantlet box in the turret frame (M; second turret-frame box). (C) A calibrated equivalent plate (e.g.
  Panther 100 @ 30, matches Isigny; a tuned number, not a published one).
- **S13-3 Traverse [S13a].** (A) One fixed rate per tank (Panther 12, Tiger 9: midpoints; M36 24; Jumbo 18 verify). (B) The
  idle/moving engine-traverse mechanic of `vehicles.md` 4.4 (SAVE field, AI revving; S-M).
- **S13-4 Heavy mortars [all].** (A) S13m after S11b's light mortars and HE share (T8 cap 26.3 %) are measured; squad mortars,
  pure data. (B) In S13a.
- **S13-5 Jumbo gun [S13a].** (A) 75 mm M3, as delivered (Sep-Nov 1944). (B) 76 mm (the Feb 1945 field re-gun).
- **S13-6 Rare rounds [S13a].** (A) 76 W 2 HVAP, Panther 2 and Tiger 2 PzGr 40, M36 none. (B) M36 1 HVAP. (C) German 0.
- **S13-7 Caps [S13a].** (A) As Q17 (A): `heavy = 1` (Tiger, Jumbo), Panther own cap 2. (B) Panther in the heavy slot too.
- **S13-8 Extra vehicles [S13a].** (A) None; M18, Jagdpanzer IV, Panzer IV/70 as a later data wave. (B) M18 and Jagdpanzer IV now.
- **S13-9 3-inch M62 row [balance pass].** (A) Keep the derived row in S13; align to B&L (+6 % at 100 m) in the balance pass with its
  own measure. (B) Align now.
- **S13-10 Push limit [S13-ai].** (A) H3 as an extension of the S3c rearm trip, on only if its 480 ablation is neutral or better. (B) Out.
- **S13-11 Heavies and the vehicle limit [S13-ai].** (A) A heavy waits for a free place under `limit` 4. (B) The AI may send a
  medium home to make room (new behaviour).
