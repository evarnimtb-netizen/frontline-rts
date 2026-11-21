# S13a Build A: O1 obliquity, layout (SAVE 30 -> 31), keys, HUD cap badge, spawn-by-card (every S13 switch off)

Read first: `S13-heavy-wave.md` sections 0, 1, 2, 3 (O1, 3.2 keys, 3.5 tests, 3.6), 5 (keys and saved fields only), 6 row
"S13a A sim"; `S12-log.md` top block; `STATE.md` "How to check work". `<BASE>` is `6f7bda0` (main: S12 merged; SAVE 30, PROTOCOL 16).
Branch `claude/project-thread-aj1tww`. The plan's line numbers are of `8d2731b`: find things by name.
Neutral by construction: no round has `slope_class` in this stage (the eleven new rounds are Build B), every S13 switch is at its
off value (`[ai.armour] heavy_buy`, `heavy_overwatch`, `push_limit` 0; `[vehicles.gunnery] special_min_pct` 0), and `[armour]
obliquity` 1 changes nothing while no round is classed. Not here: guns, rounds, vehicles, cards (Build B); rules H1-H3 (S13-ai).

Work rules: targeted release tests through `tools/run.sh`; **no command blocks longer than 4 minutes** (`tools/run.sh LOG CMD...`,
then `tools/run.sh --wait LOG`); 40 sets only through `tools/batch.sh` with `BATCH_JOBS=4`, **two in this stage** (steps 1 and 8; a
third only to bisect a failed proof); **commit after every green step** (`git commit -m "S13a A: ..." -- FILES`), never `git add -A`,
never `target/`, `.claude/`, `tools/__pycache__/`, `godot/match/fort_view.gd.uid`; do not push. No model names in files or commits.

## Chunk 0: base
1. **Base 40** on the unchanged tree: `tools/run.sh target/s13-base40.log env BATCH_JOBS=4 tools/batch.sh
   target/s13-base40/skirmish40.json -- skirmish --ai-vs-ai --seed 1 --seeds 20 --both-bases --summary --minutes 45`.

## Chunk 1: layout commit (nothing else)
2. `SAVE_VERSION` 30 -> 31 (PROTOCOL stays 16: no command). Every key and saved field, at its off/zero value:
   - `[armour]` (new table, absent: defaults): `obliquity` 1 (0..=1), `td_lo_pct` 40 / `td_hi_pct` 90 (0..=300, lo < hi),
     `apcbc_w_pct` / `apcr_w_pct` 100 (0..=100). `[vehicles.gunnery] special_min_pct` 0 (0..=100).
   - `[ai.armour]`: `heavy_buy` 0, `heavy_seen_s` 300, `heavy_save_max_s` 240, `heavy_overwatch` 0, `push_limit` 0,
     `push_without_supply_m` 300, `tank_reserve_rounds` 10.
   - Defs: ammo `slope_class` (absent / `"apcbc"` / `"apcr"`; load error on HE and HEAT); card `ai_heavy`; vehicle `ai_overwatch`
     (both default false), carried into `MatchSetup`.
   - Saved: `AiState::armour_seen_at`, `heavy_plan_since` (ticks, 0 = never); `AiCounters` `heavy_plans`, `heavy_buys`,
     `heavy_drops`, `push_turnbacks`. Hashed and load-checked where their neighbours are.
   - Rules TOML rows with comments, `docs/modding/tuning.md` rows, BASELINE refresh (`resolved_defs.rs` setup bytes and the
     canonical snapshot) for new keys only, `STATE.md` "Versions". Pattern: S12's `e847067`. Commit.
3. `targets/S13-ablations/` (`s13_off.toml`: every S13 switch at its off value, `obliquity` 0 too; the neutrality control);
   `tools/ablate.sh` S13 mode (`NEUTRAL_BASE` default `target/s13-base40`) and its dry run in `check.sh quick`;
   `load_rejects_bad_s13_keys`. Commit.

## Chunk 2: rules and tools
4. O1: `armour::pen_cos(c, t, cal, class, rules)` and every call site of plan 3.1 (plates, shields, `effective_thickness`,
   `pen_chance_ppm`, `plate_shot` and its report, `special_better`, the armour-test hit record and its invariant, `s8b.rs`).
   Tests 3.5: `no_class_is_the_cosine_law`, `overmatch_keeps_the_cosine`, `apcr_loses_more_than_apcbc_on_a_slope`,
   `heat_and_he_reject_a_class`, `hit_record_matches_the_rule`. Commit.
5. T151a-h: one test reading the B&L rows of plan 4.1 as inline test rounds (the real rounds are Build B), the plates of 4.2, the
   `pen_chance_ppm` rule at the named range and yaw; T151h runs every pre-S13 armour test and armour-test case with `obliquity` 0 and
   1. The 16 (lo, hi) pairs of 3.2 as a sweep test. Commit.
6. `special_min_pct` in `special_better` with a test and its control (0 = base). Commit.
7. armour-test spawn-by-card helper (`sim/scenarios.rs`: a duel side by card id, not the first card of a role) with a test;
   HUD cap badge (one bridge change: an in-limit card shows its `max_alive` / `limit_group` place count). Commit each.

## Chunk 3: proof
8. Default 40 into `target/s13-A-40`; `python3 -I tools/neutral.py target/s13-base40 target/s13-A-40`: same 40/40.
9. `tools/check.sh quick 6f7bda0` green (`udp::ipv6_loopback_handshake` fails here only: no IPv6); save fuzz with the new fields.
10. `S13-log.md` "For the next stage" block: state, code map, what Build B must know, every deviation and why.
