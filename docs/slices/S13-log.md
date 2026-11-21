# S13 log (heavy wave; at most 150 lines)
Spec: `docs/slices/S13-heavy-wave.md`; stages `S13a-build-A.md`, then B (data), S13-ai; ablations `targets/S13-ablations/`.
Branch `claude/project-thread-aj1tww`, base `<BASE>` = `6f7bda0` (main: S12 merged; SAVE 30, PROTOCOL 16).
Commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: S13a Build A done** (commits cb8f20b..HEAD). Next: S13a Build B (data: 4 guns, 11 rounds with `slope_class`, 5 vehicles,
5 cards at `ai_weight` 0, `[limit_groups] heavy = 1`), then Review/Fix and the 480 (T150a).
**Layout** (fede1bb, the one bump of the slice): SAVE 31, PROTOCOL 16 (no command). Setup: `[armour]` (`obliquity` 1, `td_lo_pct` 40,
`td_hi_pct` 90, `apcbc_w_pct` / `apcr_w_pct` 100), `[vehicles.gunnery] special_min_pct` 0, `[ai.armour]` `heavy_buy` 0,
`heavy_seen_s` 300, `heavy_save_max_s` 240, `heavy_overwatch` 0, `push_limit` 0, `push_without_supply_m` 300,
`tank_reserve_rounds` 10; `AmmoSetup::slope_class`, `SquadTypeSetup::ai_heavy`, `VehicleSetup::ai_overwatch`. Saved:
`AiState::{armour_seen_at, heavy_plan_since}` (load: not in the future), `AiCounters` `heavy_plans`, `heavy_buys`, `heavy_drops`,
`push_turnbacks` (not yet in the CLI JSON: S13-ai adds a block when it fills them). Build B adds data only: no bump.
**Code map.** O1: `crates/sim/src/armour.rs` `Obliquity` (`of`, `weight`, `c_pen`), `AmmoTable::{o1, with_o1, c_pen}`,
`penetrates_at`, `pen_chance_ppm_at` (old fns = `c_pen == c`), `ArmourTables::new(.., &rules.armour)`, `PlateShot::c_pen`; call
sites `damage.rs` (plates, shields, `HitRecord::c_pen`), `gunnery.rs` `special_better` (+ `special_pick`, plan 3.6),
`sdk/crates/cli/src/ballistics.rs` (text "O1 cos", JSON `impact_cos_pen`), `armour_test.rs` invariant. Card helper:
`scenarios::{type_by_card, vehicle_by_card, duel_cards}`. HUD: bridge catalog `card_alive`, `card_max`, `card_group` for an in-limit
vehicle card with `max_alive` or a limit group; `call_in_panel.gd` badge "v/limit n/max", blocked "card". Tests: `crates/sim/src/
s13_tests.rs` (layout, O1, T151, special_min_pct, helper), `sdk/crates/content/tests/s13_content.rs` (keys, def checks).
**What Build B must know.** `slope_class` is refused on `he`/`heat` rounds (def and setup); `ai_heavy` only on vehicle cards.
T151a-g are proven in `s13_tests.rs` on inline B&L rows (plan 4.1) and plates (4.2); once the real defs exist, check them with
`frontline-cli ballistics` too (same function). The window test uses flat shots (the plan's script): with the aimed descent the
corner (lo, hi) = (50, 100) gives T151e 31 % (> 30); the defaults pass both ways. Append-sensitive tests and baselines: plan 1.
Use `scenarios::duel_cards` for armour-test cases 41-46. `targets/S13.toml` does not exist yet (ablate dry run only prints it).
**Neutral.** Default 40 at the head (`target/s13-A-40`) = `target/s13-base40`: `neutral.py` says same 40/40.
**Checks.** `tools/check.sh quick 6f7bda0` all ok (1196 passed; net unchanged, so the IPv6 test did not run); test_match_view
436/437 (the known 16-vehicles perf check, headless debug).

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
