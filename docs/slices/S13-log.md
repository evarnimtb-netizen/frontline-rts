# S13 log (heavy wave; at most 150 lines)
Spec: `docs/slices/S13-heavy-wave.md`; stages `S13a-build-A.md`, then B (data), S13-ai; ablations `targets/S13-ablations/`.
Branch `claude/project-thread-aj1tww`, base `<BASE>` = `6f7bda0` (main: S12 merged; SAVE 30, PROTOCOL 16).
Commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: S13-ai Build C done** (head after this log; brief `S13-ai-build-C.md`). Next: S13-ai Review/Fix, then Measure (480 with
`targets/S13.toml`, ablations `heavy_off`, `push_off` on 480). **T155 fails on the 40 (stop-and-report per plan 7): read below.**
**Code map.** H1 `ai.rs` `Planner::heavy_plan` (+ the heavy step and `saving_heavy` in `buy`; sightings in `note_sightings`); with
`heavy_buy` 0 no card tagged `heavy_wave` (all five S13 cards) is bought. H2 `ai/overwatch.rs` `overwatch` (tank def `ai_overwatch`).
H3 `ai/armour.rs` `rearm` (`push`). `MatchSetup::s13_off` (content `setup.rs`) for the tests written before C. Review fixes:
`VehicleSetup::hull_body_boxes`/`hull_body` (V3b overwatch, V3c `gunnery::special_better`, V3d `scenarios::hull_down`), V4a zone
names in `skirmish.rs` hits by zone (3-box defs only), V1 track band 75 % (M36 90 %), V5 full-length superstructures. CLI `s13`
block per faction and side (`S13Player`/`S13Faction`, `json_out::s13_blocks`: counters, per card bought/matches/matches_pct, kills
and kos by vehicle). Armour-test 41-46 in `armour_test/s13.rs` (own staging, sight 1 km; each with an `o1_off` run).
**40 sets** (`target/s13-C-40` default; `target/s13-C-off40` = `s13_off`): default Germany 20/40, no winner 0, length median 20.6
min (base 20/40, 21.1). `neutral.py target/s13-base40 target/s13-C-off40`: same 40/40. Heavy buys (matches with one): Panther 9/40
(22.5 %), Tiger 2/40 (5 %), Jumbo 16/40 (40 %), M36 0, 76 W 0; heavy plans/buys/drops Germany 40/12/11, USA 36/16/6; push
turn-backs 0; rare tank rounds fired Germany 128, USA 0.
**Why T155 fails.** The AI buys about one tank-slot vehicle a match (base too: Sherman 28, M10 9 in 40). The first, at the 4:00
unlock, is the Sherman (only card ready); later vehicle-slot buys pick the fewest buys per weight with roster-order ties, so the M10
(earlier in the roster) comes before the 76 W (6:00) and M36 (8:00); and while a Jumbo plan saves (10:00 on) no medium is bought.
Panther 1050 and Tiger 1200 are often unaffordable within `heavy_save_max_s` (11 German drops). Levers are tuning (owner rule: none
in C): weights, H1 timing, roster order. **H3 is dead at the defaults**: `tank_reserve_rounds` 10 is under the S3c marks (AP 3 +
HE `tank_he_reserve` 6 + 1), so a tank needing H3 already rearms; its test raises the reserve to 20.
**Armour-test (5 seeds x 2, O1 / o1_off):** 41 Panther vs 2 Shermans from 45 deg: Panther 0/10 (superstructure sides 12/11 pens,
first pen 1.5 s; same off). 42 M36 vs Panther: glacis pens 0 (off 2), Panther wins 8/10. 43 76 W vs Panther: glacis pens 0 (off 2),
Panther 10/10 (off 8), HVAP fired 4 (off 6). 44 Tiger vs 2 Shermans: Tiger 10/10 (both). 45 Jumbo into PaK 40: **FAIL** 0/10 survive
3 hits: every hit is one of the PaK's 2 PzGr 40 (14 fired; an old, unclassed APCR round on the cosine law, S13-1b; the V3c fix lets
Auto pick it against the glacis). 46 76 W on a Panther's flank: 10/10.
**Deviations.** (1) Targets T174-T184 = plan T150-T159 + 24 (ids taken). (2) `heavy_buy` 0 also gates the 76 W and M36 (tag
`heavy_wave`), so `s13_off` is neutral and `heavy_off` measures the whole wave. (3) A drop forgets the sighting (`armour_seen_at` 0);
a new plan needs a later sighting (no extra state). (4) Cases 41-46 stage their own fights (`duel_cards` holds two tanks only).
(5) T156 rare rounds by type, T157 losses by type, T159 rack-dry: not counted (only rare rounds per faction, kills/kos by type).
(6) V4a has no own test (report keys only); V2, V3a, V6, V7 left to Fix. `special_min_pct` stays 0 (T156 not read on 480 yet).

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
