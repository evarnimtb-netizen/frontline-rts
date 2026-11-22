# S13 log (heavy wave; at most 150 lines)
Spec: `docs/slices/S13-heavy-wave.md`; stages `S13a-build-A.md`, then B (data), S13-ai; ablations `targets/S13-ablations/`.
Branch `claude/project-thread-aj1tww`, base `<BASE>` = `6f7bda0` (main: S12 merged; SAVE 30, PROTOCOL 16).
Commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: S13 G2 (after S13 Fix) done** (head after this log). Next: Measure (480 with `targets/S13.toml`, ablations `heavy_off`,
`push_off` on 480), on a binary with G2 (the 480 started during G does not have it). **T155 (T179a-d) still misses on the 40.**
**Code map** as F's, plus the G2 tie rule in `ai.rs` `buy` (the vehicle slot's `min_by`, `wave_rank`) and
`Game::card_unlock_ticks` (the unlock `call_in_check` uses). Gate: `[ai.armour] heavy_buy` (0 in `s13_off`, `heavy_off`).
**40 sets:** `target/s13-H-40` (G2 default): Germany 20/40, no winner 0, median 20.6 min (min 11.25, max 31.68), as F's
`target/s13-F-40` but one US match (a 76 W instead of a Sherman). Heavy buys (matches with one): Panther 9/40, Tiger 2/40, Jumbo
16/40, M36 0, 76 W 1/40; plans/buys/drops Germany 38/11/10, USA 36/16/6. Vehicle slot per card (bought), G2 / F: USA Sherman
18/19, M10 4/4, M3A1 1/1, 76 W 1/0, M36 0/0; Germany Pz IV 27/27, StuG 5/5, Sdkfz 251 1/1 (Panther, Tiger, Jumbo: H1, not the slot).
`s13_off` is unchanged by G2 (gate 0: roster order), so F's neutral reading stands.
**T155 after G2.** The tie rule works (tests) but rarely applies: the slot saves for the 0-buy Sherman (590) until the budget
reaches it, so at the moment of the pick a 690 / 760 wave card is seldom affordable. F's diagnosis otherwise stands: late budget
(first US slot buy at 16-20 min, about one a match), H1 saving for the Jumbo from 10:00, Panther before Tiger by H1's pick rule.
**Owner levers:** M36 / 76 W: `ai_weight` > 100 (outside plan 7; it also makes the slot save for them), or a rule change
(e.g. let a wave card take the tie when the slot would save anyway). Panther: cost 1050, then `heavy_save_max_s` 240. Tiger: cost
1200, then H1 timing.
**Armour-test 45** is a known miss (T184d `kind = "report"`, TODO S13-F1): PaK 40's old PzGr 40 is unclassed (balance pass).
**Deviations** as F's (1)-(8); G2: (9) "affordable" means manpower and pop cap both, at the pick.

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
Commits: 0439e70 G (the tie to the newer card in the vehicle slot, any card); 385dc6c its log; 3fc795c G2 (narrowed); then this
log. G's 40 set `target/s13-G-40`: Germany 19/40, median 19.65; side effect not acceptable: every tie moved (US slot saved for the
M36: 4 slot buys in 40, F 24; Germany StuG 30 / Pz IV 1, F 5 / 27). **G2:** with `[ai.armour] heavy_buy` 1, a tie in the vehicle
slot goes to a `heavy_wave` card only if the AI can pay it now (manpower and pop); among such cards the later unlock, then the later
in the roster. Any other tie, and an out-of-reach wave card, keeps roster order and its saving, as before. `heavy_buy` 0: as main.
No new key, no saved field, no data change. `check.sh quick 6f7bda0` all ok (1213). Tests `vehicle_tie_goes_to_the_newer_card`,
`vehicle_tie_unaffordable_wave_card_keeps_roster_order`, `vehicle_tie_roster_order_with_heavy_buy_off`. 40 set `target/s13-H-40`:
"For the next stage".
