# S15b Build C: Soviet guns, vehicles and rounds; rider slots; armour duel rows W5 (report); T210 and T211

Read first: `S15-ussr.md` sections 0, 5.1 (riders), 6.3, 6.4, 10 W5, 11, 14; `S15-research-history.md`; `S15-log.md` "For the next
stage" (Build B's hand-over: tag `vehicle_riders`, crews, the cook-off round stand-in); the S13 vehicle defs (`ger_panther_g`,
`usa_m4a3e2`, ...) and `docs/slices/S13-log.md` for the 3-box armour model, `slope_class` rounds and `hull_body_boxes`; S11b's gun
defs for towed guns; `sdk/crates/cli/src/armour_test/s13.rs` as the W5 pattern. `<BASE>` for neutrality: `target/s14-F-40`.
Branch `claude/project-thread-aj1tww`. **2 chunks.** No layout change.

Owner rule (2026-11-25): **no balance runs, no tuning.** Prices, cooldowns and weights are the plan's starting values with the comment
"S15 starting value". Every figure with `sources`; "(verify)" figures read from a source before use (web search if available), else
marked "unverified" in the def. Soviet penetration tables at 0 degrees are converted to the mod's convention (say how in the log).

Work rules: as S15a (`tools/run.sh`, no command over 4 minutes, `git commit -m "S15b C: ..." -- FILES`, never `git add -A`, never
`target/`, `.claude/`, `tools/__pycache__/`, `godot/match/*.gd.uid`; do not push; no model names in files or commits). Two 40 sets
(step 5, owner-approved neutrality checks; if refused, skip and say so) plus one 4-seed Germany-USSR smoke (step 4).

1. Rounds (AP, APBC/APCBC with `slope_class` as S13, HE, the 82 mm bomb if Build B left it, a Soviet HE round for the crates'
   cook-off) and guns of 6.3 (45 mm M-42, ZiS-2 57 mm, ZiS-3 76 mm direct fire only (S15-13), M-30 122 mm if the plan lists it,
   120 mm PM-38 if listed) with tows and crews. Commit.
2. Vehicles of 6.4 (T-34-85, IS-2, SU-85, SU-76M, the M3A1 Scout Car (S15-9), and the rest the plan lists) with S13's 3-box armour,
   `hull_body_boxes`, racks, rare caps, and rider slots + `[riders]` blocks on the Soviet tanks (S15-3). Cards with the plan's costs and
   weights. Content validation, resolved-defs baseline (`BLESS=1`), canonical snapshot. No German or US def gains an S15 key; every id
   `ussr_*` (the `s15_defs_sort_last` test must stay green). Commit.
3. W5 armour duels (report rows; the plan's list) in an `armour_test/s15.rs`; tests that riders board each Soviet tank with slots and
   not a German or US one. Commit.
4. Smoke: `docs/slices/targets/S15-smoke.toml` 4 seeds Germany-USSR (as Build B) through `tools/run.sh`: no panic, every match ends;
   note vehicle and gun buys. Fix only data or plain bugs.
5. **T210** (`s15_off`) into `target/s15-C-off40/` and **T211** (shipped) into `target/s15-C-40/`, the default Germany-USA command;
   both `python3 -I tools/neutral.py target/s14-F-40 <dir>` same 40/40.
6. `tools/check.sh quick 96d4b4b` green (IPv6 excepted). Rewrite `S15-log.md` "For the next stage" for the presenter and S15-ai.

Files owned: `mods/ww2/defs/**` (new `ussr_*` defs), `mods/ww2/tests/`, `sdk/crates/content/**` (tests and validation only where the
data needs it), `sdk/crates/content/tests/resolved_defs.rs`, the canonical snapshot, `sdk/crates/cli/src/armour_test/`,
`crates/sim/src/s15_tests.rs`, `docs/slices/targets/S15*`, `docs/slices/S15-log.md`, `docs/slices/S15-research-history.md`.
