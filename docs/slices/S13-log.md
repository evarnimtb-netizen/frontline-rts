# S13 log (heavy wave; at most 150 lines)
Spec: `docs/slices/S13-heavy-wave.md`; stages `S13a-build-A.md`, then B (data), S13-ai; ablations `targets/S13-ablations/`.
Branch `claude/project-thread-aj1tww`, base `<BASE>` = `6f7bda0` (main: S12 merged; SAVE 30, PROTOCOL 16).
Commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: S13 Fix (after S13-ai Build C) done** (head after this log). Next: Measure (480 with `targets/S13.toml`, ablations
`heavy_off`, `push_off` on 480). **T155 (T179a-d) still misses on the 40: tuning, not a defect; owner levers below.**
**Code map** as C's (H1 `ai.rs` `Planner::heavy_plan` and the heavy step in `buy`; H2 `ai/overwatch.rs`; H3 `ai/armour.rs`), plus
`VehicleSetup::glacis_box` (the hull body's top box: box 0 before S13): the presenter's hull and glacis (V2) and the spotting hull
point (`Geo::body_top`, V3a). `AmmoMechanism::takes_slope_class` (exhaustive allow-list). HUD badge names the binding card cap.
**40 sets** (`target/s13-F-40` default; `target/s13-F-off40` = `s13_off`): default Germany 20/40, no winner 0, median 20.6 min
(C: 20/40, 20.6). Heavy buys (matches with one): Panther 9/40 (22.5 %), Tiger 2/40 (5 %), Jumbo 16/40 (40 %), M36 0, 76 W 0;
plans/buys/drops Germany 39/11/10, USA 36/16/6. Neutral: `neutral.py target/s13-base40 target/s13-F-off40`: same 40/40.
**T155 diagnosis (traced per buy, seeds 1-4).** No rule does other than its doc, no S13 card is filtered out: the 76 W and M36 are
in the vehicle slot's ready list from 6:00 / 8:00. (1) The slot's pick is "fewest buys per weight, ties roster order"
(`min_by` keeps the first minimum). Every vehicle card has weight 100 and 0 buys until the first tank, so the Sherman (roster 8th)
wins over the M10 (11th), the M3A1 (12th), the 76 W (22nd) and the M36 (23rd); unaffordable, the slot saves for it ("no cheaper
card instead"). (2) The AI's budget reaches 590 late: first US tank-slot buys at 16-20 min in the traces (both sides; budget held
at 100-450 by the other slots, the guns' and arty pots, `tank_reserve_mp`); about one tank-slot buy a match (US 24 in 40, base 37).
The 76 W is the 4th slot buy of a match, the M36 the 5th. (3) From 10:00, with armour seen, H1 saves for the Jumbo and blocks all
non-urgent buys (by its doc) for up to 240 s. Germany: H1 picks the Panther first (fewest buys, roster order), so the Tiger comes
only after a Panther; 10 of 39 plans drop unaffordable (1050 / 1200 MP in 240 s).
**Smallest lever set (owner; plan 7 order cost, unlock_s, cooldown_s, max_alive, H1 timing):** M36: none of plan 7's levers moves
it (its rank is weight and roster order, not cost or time): the smallest is `ai_weight` M36 > 100 (101 is enough to win the tie
with the 0-buy Sherman once ready at 8:00), outside plan 7's list. Panther (22.5 %, one match short; read on 480 first): cost
1050, then `heavy_save_max_s` 240. Tiger (5 %): cost 1200, then H1 timing; its order behind the Panther is H1's pick rule (a rule).
**Armour-test 45** is a known miss (T184d `kind = "report"`, TODO S13-F1): the PaK 40's old PzGr 40 is unclassed (S13-1b, balance
pass). Cases 41-46 otherwise as C's (not re-run; V6 moved Panther/Tiger bins and anchors and the 76 mm reload 6.0 -> 6.5 s).
**Deviations.** (1) Targets T174-T184 = plan T150-T159 + 24. (2) `heavy_buy` 0 also gates the 76 W and M36 (tag `heavy_wave`).
(3) A drop forgets the sighting. (4) Cases 41-46 stage their own fights. (5) T156 by type, T157 losses by type, T159: not counted.
(6) V4a has no own test. (7) T184d reported, not hard (case 45). (8) V6's MG round counts stay (no figure read; comments say so).

## S13a A (Build A)
Base 40 `target/s13-base40` (unchanged tree at 6f7bda0; cached from the S12 runs of the same binary and mods): Germany 20/40,
no winner 0. Commits: cb8f20b brief and log; fede1bb layout; 2de7c6f keys checked, ablations, ablate.sh S13 mode; b358a68 O1 and
`special_min_pct`; b333f0b duel by card; 894d9e7 HUD cap badge; then save fuzz staging, clippy fixes and this log.
Default 40 `target/s13-A-40`: Germany 20/40, no winner 0; `neutral.py`: same 40/40. Save fuzz stages `armour_seen_at`,
`heavy_plan_since` and two heavy counters (`staged_s10b_ai`).
**Deviations.** (1) The plan's `armour::pen_cos(c, t, cal, class)` is `AmmoTable::c_pen(c, t)` (the class and calibre live in the
round's table, built once per match). (2) `effective_thickness` stays the cosine law (it serves fragments, `combat.rs`); the
O1 effective thickness is in `plate_shot`. (3) `special_min_pct` shares the O1 commit (one gunnery edit). (4) The HUD badge needed
a small presenter change beside the one bridge change (the panel had no key for an in-limit card's own cap). (5) Ablations add
`o1_off`, `heavy_off`, `push_off` beside `s13_off`. (6) T151h is proven on the tables (every ww2 `AmmoTable` equal with
`obliquity` 1 and 0, no `o1`), not by re-running every armour-test case.

## S13-ai C (Build C)
Commits: 1e1b547 brief; 30565d2 H1-H3, switches and weights, CLI `s13`; a1b30b9 review V1; b2b1ede V5; 8669bca V3b-d; 34a24dd V4a;
ab4e2d6 armour-test 41-46; bc23b06 `targets/S13.toml`; then this log. `tools/check.sh quick 6f7bda0` green at bc23b06
(1207 tests). Numbers and deviations: "For the next stage" above.

## S13 F (Fix)
Commits: 45de15b V2 presenter hull body and glacis; f9c8922 V3a spot point; 7b18bfd V6 data; d5c4c1c V7 plan wording; 21626f1 stage
A N1 N2 N3 N7; b1ddf73 N4 HUD badge; b8ae38f N5 plan 3.2; 375898e case 45 known miss (T184d, TODO); then this log. V4b: covered by
C's `armour_test/s13.rs` (tallies by `zone_name`); the S3 tally in `armour_test.rs` serves only two-box pre-S13 cases. N6: the S13a
layout commit fede1bb and the O1 commit b358a68 were not compiled on their own (stage A built at its head): a bisect over them
needs a per-commit build. T155: traced, no defect (above). 40 sets: default `target/s13-F-40`, `s13_off` `target/s13-F-off40`.


## S13 G (vehicle-slot tie)
Commit 0439e70: the vehicle slot's pick (fewest buys per weight) breaks a tie by the newer card, the later unlock
(`Game::card_unlock_ticks`, the rule `call_in_check` uses) then the later in the roster, when `[ai.armour] heavy_buy` is 1; with
0 (`s13_off`, `heavy_off`, every pre-S13 test via `s13_off()`) roster order as before. No new key, no saved field, no data change.
Tests `vehicle_tie_goes_to_the_newer_card`, `vehicle_tie_roster_order_with_heavy_buy_off`; `check.sh quick 6f7bda0` all ok (1212).
**40 set** `target/s13-G-40` (default): Germany 19/40, no winner 0, median 19.65 min (min 11.25, max 31.86; F: 20/40, 20.6).
Heavy buys (matches with one / bought): Panther 8/9, Tiger 3/5, Jumbo 18/19, M36 3/3, 76 W 1/1; plans/buys/drops Germany
29/14/4, USA 38/19/6. **Side effect:** the rule applies to every tie in the slot, not only the S13 cards. USA: the slot now
saves for the M36 (760) over the Sherman (590): US slot buys 4 in 40 (F: 24, Sherman 19, M10 4, M3A1 1). Germany: the StuG III
(later in the roster) wins over the Pz IV: StuG 30, Pz IV 1 (F: 5, 27). Owner: keep, or narrow the rule to `heavy_wave` cards.
