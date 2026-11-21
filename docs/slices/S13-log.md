# S13 log (heavy wave; at most 150 lines)
Spec: `docs/slices/S13-heavy-wave.md`; stages `S13a-build-A.md`, then B (data), S13-ai; ablations `targets/S13-ablations/`.
Branch `claude/project-thread-aj1tww`, base `<BASE>` = `6f7bda0` (main: S12 merged; SAVE 30, PROTOCOL 16).
Commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: S13a Build B done** (commits db0754f brief, a1ad12a data; head after this log). Next: S13a Review/Fix (sim diff of A and
the data of B), then Measure (480 at the head = base, T150a), then S13-ai (H1-H3, weights, armour-test cases 41-46).
**Data.** 4 weapons (`kwk42_l70`, `kwk36_l56`, `m1a1_76mm`, `m3_90mm`), 11 rounds (7 AP/APCBC/APCR with `slope_class`: `75mm_pzgr3942`
apcbc, `75mm_pzgr4042` apcr, `88mm_pzgr39` apcbc, `88mm_pzgr40` apcr, `76mm_m62` apcbc, `76mm_m93` apcr, `90mm_m82` apcbc; the 4 HE
rounds carry none, the loader refuses one), 5 vehicles (`ger_panther_g`, `ger_tiger_1`, `usa_m4a3_76w`, `usa_m36`, `usa_m4a3e2`),
5 cards `*_squad` at `ai_weight` 0 (`ai_heavy` on Panther, Tiger, Jumbo; `ai_overwatch` on the Panther and Tiger vehicle defs),
`[limit_groups] "ww2:heavy" = 1` (Tiger, Jumbo cards), roster appends after S12's medic card (Germany 2, USA 3).
**Shape.** Each vehicle has three armour boxes: `hull` (lower: nose, lower sides, engine deck), `superstructure` (glacis, upper
sides, crew deck), `turret`. Zone names for `ballistics --zone`: `hull_front` = nose, `superstructure_front` = glacis,
`turret_front`. Zone ids are therefore not the S3 0..7 for these five (turret_front = 8): code must go by name or `zone_name`.
Cooldown and unlock sit on the vehicle defs (`recall_cooldown_s`, `initial_cooldown_s`), caps and dates on the cards.
**Append-sensitive tests** moved: `s11a1_tests.rs` (25 vehicle defs, indices), `s10a_content`, `s11a1_content`, `s11b_content`,
`s12_content` (roster tails), `s13_content` (exact classed rounds, heavy cards, overwatch defs, weight 0), the canonical JSON
snapshot (`BLESS=1`; the legacy snapshot and `BASELINE` hashes did not change).
**T151a-g read with `frontline-cli ballistics`** (flat shot, 0 deg yaw unless named): a M62 vs Panther glacis 100 m 0 % ; b M93 100 m
28.2 %, 400 m 0 % ; c M82 vs glacis 100 m 0 % ; d M82 vs nose 350 m 100 % ; e M62 vs nose 350 m 0 % ; f PzGr 39/42 vs 76 W glacis
1000 m 100 % (also at 30 deg) ; g 8.8 cm PzGr 39 vs Jumbo glacis 300 m 0 %. All inside the plan's bounds.
**Not done (stage C):** armour-test cases 41-46 (no CLI case file for them; `scenarios::duel_cards` is the helper); AI weights.
**Verify values left open** (marked in the defs): complete-round masses of the 8.8 cm rounds and M93, 8.8 cm Sprgr velocity and
filler, M71 filler, M82 loading (808 m/s used), Tiger nose/rear slopes, Jumbo slopes and nose, Jumbo traverse 18, rack splits,
reload times (6000-7000 ms starting values).
**Neutral.** Default 40 at B (`target/s13-B-40`) = `target/s13-base40`: `neutral.py` same 40/40.

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
