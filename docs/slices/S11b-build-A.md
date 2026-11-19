# S11b Build A: the A0 fixes, SAVE 29 layout, keys, the rifle-grenade mode (every S11b switch still 0)

Read first: `S11b.md` 2, 3, 4 (X1-X3, G1-G4), 5 (rows "Grenade aim", "Counter-fire budget"), 6; the log's top block, **above all its "A0
diagnosis" lines** (another session measured the three open items; where it names another cause or fix than spec X1-X3, it wins: build its fix
in the spec's form, a key with an off value or a proof that only named counters moved, and log the change); `S11a2-log.md` top block (code
map of `ai/arty.rs`). **2 chunks**, about 30 self-counted turns each (60-90 minutes in all). No content beyond the three rifle-grenade def
files, no JSON, no AI buy in this stage.
`<BASE>` is `8d2731b`. Work rules: `tools/outline.sh` then `tools/show.sh FILE::name` (batch the reads); targeted release tests through
`tools/run.sh`; `tools/check.sh quick 8d2731b` once per chunk end, through `tools/run.sh`; **no command blocks longer than 4 minutes**
(`tools/run.sh LOG CMD...`, then `tools/run.sh --wait LOG`); 40 sets only through `tools/batch.sh` with `BATCH_JOBS=4`; never quote tick
times; the container can restart: **commit after every green step** (`git commit -m "S11b A: ..." -- FILES`), never `git add -A`, never
`target/`, `.claude/`, `tools/__pycache__/`; if git reports an index lock, wait a few seconds and retry; do not push. No model names in files
or commit text.

Files owned: `sdk/crates/content/src/{tuning.rs,defs.rs,setup.rs}` (the keys, `ItemKind::RifleGrenade`, `stow_ms`, the issue rule);
`crates/sim/src/{guns.rs,fire_mission.rs,combat.rs,game.rs,gunnery.rs,items.rs,bail.rs,world.rs,hash.rs,rng.rs,lib.rs}`, new
`crates/sim/src/s11b_tests.rs`; `crates/sim/src/ai.rs` (the new `AiCounters` fields, their validation, `held_pots` callers only as X2 needs),
`crates/sim/src/ai/arty.rs` (X2, X3, the two `AiArty` fields; no buy code); `crates/sim/tests/save_fuzz.rs`;
`mods/ww2/defs/rules/skirmish.toml`; `mods/ww2/defs/items/m7_rifle_grenade.toml`, `weapons/m7_launcher.toml`, `ammo/m17_rifle_grenade.toml`;
`docs/modding/{tuning.md,ammo.md}`; the BASELINE files (`sdk/crates/content/tests/resolved_defs.rs`, `crates/simbench/src/report.rs`) and the
canonical snapshot; `tools/ablate.sh`, `tools/check.sh` (the S11b dry-run step); `docs/STATE.md` "Versions" line;
`docs/slices/targets/S11b.toml` and `targets/S11b-ablations/` only to fix what the CLI refuses (say so in the log); `docs/slices/S11b-log.md`.

Start from (line numbers are of `8d2731b`; find every item by name): `guns.rs` 1205-1225 (`Emplacing` ends: `s9.guns_emplaced`), 922-934
(how `guns_bought` is kept apart by role); `ai/guns.rs` 675-707 (`guns_in_cover`); `sdk/crates/cli/src/json_out.rs` 598-604 (the ratio; read
only); `ai/arty.rs` `arty_pot` 258-296, `held_pots` 301-303, `run_arty` 435-572 (the order prep, counter-fire, A4), `arty_fire`,
`cb_candidate`, `counter_fire`, `validate_arty`; `ai.rs` 1788 and `ai/fort.rs` 2824, 2921 (who reads `held_pots`); `combat.rs`
`fire_single_shot` 1333-1506 and its caller 1313-1323, `base_class` 3059-3088, `LOS_TRIES` 134; `game.rs` `SingleShot` 315-342, `kill_class`
530-570, the load check `pzf_ok` (about 2629-2640); `items.rs` `Kit`, `full_kit`, `validate_items`; `supply.rs` `refill_men` 634-745;
`content/src/defs.rs` `ItemKind` 305-345, the item def check 2431; `content/src/setup.rs` (where `single_shot_gap_ticks` is built, 1999; the
squad-type item table); `rng.rs` (purposes end at 39); S11a2 A's layout commit `13b9c82` as the pattern for a SAVE bump.

## Chunk 0 (before any edit, 5 minutes): the base set and the targets file
1. `git log --oneline | head -3`, `git status --short`: the head must be `8d2731b` or S11b plan commits on it, with no uncommitted code.
2. `tools/run.sh target/s11b-base40.log env BATCH_JOBS=4 tools/batch.sh target/s11b-base40/skirmish40.json -- skirmish --ai-vs-ai --seed 1
   --seeds 20 --both-bases --summary --minutes 45` on the unchanged tree (if `target/release/frontline-cli` is older than the head:
   `tools/run.sh target/s11b-build.log cargo build -q --release -p frontline-cli` first). This is REF0. Log Germany's wins of 40.
3. The parse check the Plan stage could not run (another session was playing matches): `target/release/frontline-cli targets
   docs/slices/targets/S11b.toml --from-json target/s11b-base40 | tail -45`. Expected: the file loads; T149-T158 print MISSING-METRIC or
   SKIP. A parse error is fixed in the file now and logged. Then `tools/ablate.sh --dry-run` on the new folder once step 5 has taught it.

## Chunk 1: A0 fixes, one layout commit, keys
4. **Layout commit (nothing else in it):** `SAVE_VERSION` 28 -> 29; every saved field of spec 3 at its zero default; load validation and
   hash for each; `save_fuzz.rs` staged save gains the items of spec 3; `load_rejects_bad_s11b_state` (one case per check),
   `s11b_layout_roundtrips_and_hashes`. `PROTOCOL_VERSION` stays 14: say so in the commit text. `STATE.md` "Versions" line. Commit.
5. Keys of spec 2 with ranges and comments in `skirmish.toml` (`cb_budget_pct` **100**, `pot_hold` **1**, `how` 0, `light` 0,
   `rifle_grenades` 0), rows in `docs/modding/tuning.md`; `load_rejects_bad_s11b_keys` (one widened range must fail). `tools/ablate.sh`:
   the folder `S11b-ablations` (allowed keys of spec 2 at the listed values, neutral file `s11b_off`, `NEUTRAL_BASE` default
   `target/s11b-A0-40`) and the dry-run step in `tools/check.sh quick`. BASELINE and snapshot refreshed (new keys only). Commit.
6. X1, X2, X3 in this order, each its own commit with its tests (spec 4). Then the 40 set with every fix at its off value
   (`target/s11b-A0off40/`, override `S11b-ablations/a0_off.toml`): `python3 -I tools/neutral.py target/s11b-base40 target/s11b-A0off40` must
   show no difference but the X1 keys spec 4 lists (if `neutral.py` cannot say that in one line, list the differing keys from its output in
   the log; any other key is a bug of this stage).
7. Turn-on: `cb_budget_pct` 50 in `skirmish.toml`. For `pot_hold`: one 40 set with a scratch override `pot_hold = 0` (under `target/`),
   read `factions.usa.s7_derived.bought_matches_pct.usa_m10_squad`, `...usa_m3a1_squad`, `factions.<f>.s11a1_derived.gun_bought_matches_pct`
   and the winners beside the `cb_budget_pct` 50 set; apply spec X2's turn-on rule and log both rows whatever the outcome. Then the default
   set `target/s11b-A0-40/skirmish40.json`: this is **REF1**. Log from it: Germany's wins, `s9_derived.gun_in_cover_pct` and
   `s11a2_derived.cb_shell_share_pct` per faction, the two vehicle rows. BASELINE and snapshot refreshed. Commit.
**STOP POINT 1.** `check.sh quick 8d2731b` green; commit; rewrite the log's top block (commits, REF0 and REF1 paths, the A0 numbers, what
differs from spec 3 or 4); return if more than 45 self-counted turns are used (then chunk 2 is a new session: say so first).

## Chunk 2: the rifle-grenade mode
8. G1: the kind, `stow_ms`, the def check, the issue rule in the setup build; the three def files (plan values of spec G1, each number
   commented "verify: no design figure, S11b-7"); `rifle_grenade_item_needs_an_he_launcher`, `switch_off_issues_no_rifle_grenades`. The test
   scene: a `MatchSetup` edit that gives one member slot of a rifle squad 3 grenades (no card carries them until Build B).
9. G2, G3 in `combat.rs` (`fire_rifle_grenades`, `RgState`), `Purpose::RIFLE_GRENADE` (40); `rg` set wherever `pzf` is set; the tests of
   spec G2-G3 and the "Grenade aim" row of spec 5 (`grenade_aim_clears_when_the_target_leaves_the_band`,
   `two_grenadiers_aim_one_at_a_time`). Read how rifles pick a seen enemy before writing the candidate list, and use the same visibility test.
10. G4: `base_class`, the refill test. The CLI's HE share is Build C's file: leave `skirmish.rs` alone and say so in the log.
11. Neutrality: default 40 set into `target/s11b-A40/` and with `-o docs/slices/targets/S11b-ablations/s11b_off.toml` into
    `target/s11b-Aoff40/`; `python3 -I tools/neutral.py target/s11b-A0-40 target/s11b-A40` and `... target/s11b-Aoff40`: both `same 40/40`.
    A difference is a bug in this stage (a path that runs with the switch 0): find it, do not explain it away.
Tests that must exist at the end (each names its control in a comment): the X1-X3 and G1-G4 tests of spec 4, the two of step 9,
`load_rejects_bad_s11b_state`, `load_rejects_bad_s11b_keys`, save roundtrip and fuzz roundtrip green.
**STOP POINT 2.** `check.sh quick 8d2731b` green (it applies `cargo fmt`; run it last); commit; rewrite the log's top block.

## The stage must log (in `S11b-log.md`, top block rewritten, at most 30 lines)
Commits; SAVE 29 / PROTOCOL 14; test count from `check.sh`; the parse check result of chunk 0; REF0 and REF1 (paths, Germany's wins); the A0
table (T81 ratio, counter-fire share, M10 and M3A1 bought, before and after, and the `pot_hold` decision with its two rows); the `neutral.py`
lines for A0off40, A40 and Aoff40; a code map (`fire_rifle_grenades`, `RgState::for_type`, where the issue rule drops the kind, where
`rounds_all` / `rounds_cb` are added, the X1 line); any rule built differently from spec 4, with the reason; what Build B needs: the exact
item, weapon and ammo ids, how a member slot carries grenades, whether a split member group moved any match in a scratch run (not required:
B proves it), the def-check limits met (`emplace_s`, `speed_cm_s`, `push_men` maxima and minima). Not done items by rule id.
If a step cannot be finished: its switch stays at its off value, the neutrality proof is kept, and the log's top block says so first. The A0
fixes and the layout (chunk 1) are worth more than the grenade mode (chunk 2): if only one fits, it is chunk 1, complete and tested; G stays
unbuilt with `ItemKind::RifleGrenade` in the layout, and Build B skips C5. Never leave the tree with a red `check.sh quick`.
