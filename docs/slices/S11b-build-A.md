# S11b Build A: the A0 fixes, SAVE 29 / PROTOCOL 15 layout, keys, the rifle-grenade mode (every S11b switch still 0)

Read first: `S11b.md` 2, 3, 4 (X1-X3, G1-G4), 5 (rows "Grenade aim", "Counter-fire turn"), 6; the log's top block (the measured A0
diagnosis and the plan review's points for this stage); `S11a2-log.md` top block (code map of `ai/arty.rs`). **2 chunks**, about 30
self-counted turns each (60-90 minutes in all). No content beyond the three US rifle-grenade def files, no JSON, no AI buy in this stage.
Every switch of spec 2 stays at its off value through this stage (Build C turns them on), so play never changes: only the X1 counters do.
`<BASE>` is `8d2731b`. Work rules: `tools/outline.sh` then `tools/show.sh FILE::name` (batch the reads); targeted release tests through
`tools/run.sh`; `tools/check.sh quick 8d2731b` once per chunk end, through `tools/run.sh`; **no command blocks longer than 4 minutes**
(`tools/run.sh LOG CMD...`, then `tools/run.sh --wait LOG`); 40 sets only through `tools/batch.sh` with `BATCH_JOBS=4`, **three in this
stage and no more** (steps 2, 7, 11; a fourth only to bisect a failed proof); never quote tick times; the container can restart: **commit
after every green step** (`git commit -m "S11b A: ..." -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`;
if git reports an index lock, wait a few seconds and retry; do not push. No model names in files or commit text.

Files owned: `sdk/crates/content/src/{tuning.rs,defs.rs,setup.rs}` (the keys, `ItemKind::RifleGrenade`, `thrown()`, `stow_ms`, the issue
rule); `crates/sim/src/{guns.rs,fire_mission.rs,combat.rs,game.rs,gunnery.rs,indirect.rs,items.rs,supply.rs,bail.rs,world.rs,hash.rs,
rng.rs,lib.rs}`, new `crates/sim/src/s11b_tests.rs`; `crates/sim/src/ai.rs` (the new `AiCounters` fields and their validation),
`crates/sim/src/ai/arty.rs` (X3 and `AiArty::cb_last`; no buy code); `crates/net/src/protocol.rs` (`PROTOCOL_VERSION` 15 and its comment)
with whatever net test pins the number; `sdk/docs/net/relay-protocol.md`; `crates/godot-bridge/src/intents.rs` (the `"rifle_grenade"` arms
of `item_name` and the name parser at 540-560; a throw intent naming it must be refused as `"mine"` is); `crates/sim/tests/save_fuzz.rs`;
`mods/ww2/defs/rules/skirmish.toml`; `mods/ww2/defs/items/m7_rifle_grenade.toml`, `weapons/m7_launcher.toml`,
`ammo/m17_rifle_grenade.toml`; `docs/modding/{tuning.md,ammo.md}`; the BASELINE files (`sdk/crates/content/tests/resolved_defs.rs`,
`crates/simbench/src/report.rs`) and the canonical snapshot; `tools/ablate.sh`, `tools/check.sh` (the S11b dry-run step); `docs/STATE.md`
"Versions" line; `docs/slices/targets/S11b.toml` and `targets/S11b-ablations/` only to fix what the CLI refuses (say so in the log);
`docs/slices/S11b-log.md`.

Start from (line numbers are of `8d2731b`; find every item by name): `guns.rs` 1205-1225 (`Emplacing` ends: `s9.guns_emplaced`), 922-934
(how `guns_bought` is split by role); `combat.rs` 3562-3572 (`s9.gun_crew_lost`); `ai/arty.rs` `run_arty` 435-572 (the order: prep,
counter-fire, A4), `arty_fire` 579-623, `cb_candidate` 632-668, `counter_fire` 678-733, `validate_arty`; `combat.rs` `fire_single_shot`
1333-1506 and its caller 1313-1323, `base_class` 3059-3088, `LOS_TRIES` 134; `game.rs` `SingleShot` 315-342, `kill_class` 530-570, the load
check `pzf_ok` (about 2629-2640); `items.rs` `Kit`, `full_kit`, `can_throw` 223-249, `validate_items`; `supply.rs` `refill_men` 634-745 (the
item search at 711); `indirect.rs` `solve_root` 49; `gunnery.rs` `solution` 352 (what NOT to aim with); `command.rs` 109-120;
`content/src/defs.rs` `ItemKind` 305-345 and `thrown()`, `ItemDef` 370-377, the item def check 2411-2435; `content/src/setup.rs` (where
`single_shot_gap_ticks` is built, 1999; the squad-type item table); `rng.rs` (purposes end at 39); S11a2 A's layout commit `13b9c82` and
S10b's protocol bump as the patterns.

## Chunk 0 (before any edit, 5 minutes): the base set and the targets file
1. `git log --oneline | head -3`, `git status --short`: the head must be `8d2731b` or S11b plan commits on it, with no uncommitted code.
2. **Set 1 of 3 (REF0):** `tools/run.sh target/s11b-base40.log env BATCH_JOBS=4 tools/batch.sh target/s11b-base40/skirmish40.json --
   skirmish --ai-vs-ai --seed 1 --seeds 20 --both-bases --summary --minutes 45` on the unchanged tree (if `target/release/frontline-cli` is
   older than the head: `tools/run.sh target/s11b-build.log cargo build -q --release -p frontline-cli` first; the batch cache may already
   hold this set). Log Germany's wins of 40.
3. The parse check the Plan stage could not run (another session was playing matches): `target/release/frontline-cli targets
   docs/slices/targets/S11b.toml --from-json target/s11b-base40 | tail -45`. Expected: the file loads; T149-T158 print MISSING-METRIC or
   SKIP. A parse error is fixed in the file now and logged. `tools/ablate.sh --dry-run` on the new folder once step 5 has taught it.

## Chunk 1: one layout commit, keys, the A0 fixes
4. **Layout commit (nothing else in it):** `SAVE_VERSION` 28 -> 29 and `PROTOCOL_VERSION` 14 -> 15 with the relay document's line; every
   saved field of spec 3 at its zero default; `ItemKind::RifleGrenade` with `thrown()` false and the bridge arms, so that
   `cargo build --release --workspace` passes (run it once here through `tools/run.sh`: `check.sh quick` builds changed crates only);
   load validation and hash for each field; `save_fuzz.rs` staged save gains the items of spec 3; `load_rejects_bad_s11b_state` (one case
   per check), `s11b_layout_roundtrips_and_hashes`. `STATE.md` "Versions" line. Commit.
5. Keys of spec 2 with ranges and comments in `skirmish.toml` (`cb_alternate` 0, `buy_pending` 0, `how` 0, `light` 0, `rifle_grenades` 0),
   rows in `docs/modding/tuning.md`; `load_rejects_bad_s11b_keys` (one widened range must fail). `tools/ablate.sh`: the folder
   `S11b-ablations` (allowed keys of spec 2 at the listed values, neutral file `s11b_off`, `NEUTRAL_BASE` default `target/s11b-A0-40`) and
   the dry-run step in `tools/check.sh quick`. BASELINE and snapshot refreshed (new keys only). Commit.
6. X1 (both counters) and X3 (the flag, the hold, its two tests; the switch stays 0), each its own commit with its tests (spec 4). X2 has
   no code.
7. **Set 2 of 3 (REF1):** the default 40 set into `target/s11b-A0-40/`; `python3 -I tools/neutral.py target/s11b-base40 target/s11b-A0-40`
   must show no difference but the X1 keys spec 4 lists (list the differing keys from its output in the log; any other key is a bug of
   this stage). Log from it `s9_derived.gun_in_cover_pct` per faction (the diagnosis read 86.5 / 87.2 %). The effect of `cb_alternate` is
   not measured here: the diagnosis measured it (62.3 -> 47.3 %), Build C turns it on and the orchestrator's 480 reads T148.
**STOP POINT 1.** `check.sh quick 8d2731b` green; commit; rewrite the log's top block (commits, REF0 and REF1 paths, the X1 numbers, what
differs from spec 3 or 4); return if more than 45 self-counted turns are used (then chunk 2 is a new session: say so first).

## Chunk 2: the rifle-grenade mode
8. G1: `stow_ms`, the def check, the issue rule in the setup build; the three US def files (figures of spec G1 commented "owner to confirm,
   S11b-7", plan values commented "verify: plan value, S11b-13"; `scatter_pct = 0`, `fuse_ms = 0`); the three G1 tests. The test scene: a
   `MatchSetup` edit that gives one member slot of a rifle squad 3 grenades (no card carries them until Build B).
9. G2, G3 in `combat.rs` (`fire_rifle_grenades`, `RgState`), `Purpose::RIFLE_GRENADE` (40); the low-root aim through a `pub(crate)`
   wrapper of `indirect.rs` `solve_root` (muzzle point, target point, speed -> launch direction; no root: no shot). Write
   `rifle_grenade_lands_on_its_aim_point` FIRST and see its control (the `gunnery::solution` aim) miss. `rg` set wherever `pzf` is set; the
   other tests of spec G2-G3 and the "Grenade aim" row of spec 5 (`grenade_aim_clears_when_the_target_leaves_the_band`,
   `two_grenadiers_aim_one_at_a_time`). Read how rifles pick a seen enemy before writing the candidate list, and use the same visibility test.
10. G4: `base_class`; `refill_men` leaves the kind out of its item search; the two G4 tests. The CLI's HE share is Build C's file: leave
    `skirmish.rs` alone and say so in the log.
11. **Set 3 of 3:** the default 40 set into `target/s11b-A40/`; `python3 -I tools/neutral.py target/s11b-A0-40 target/s11b-A40`: `same
    40/40`. Every switch is 0 by default, so this one set is the proof for the default and for `s11b_off` (check with
    `tools/ablate.sh --dry-run` that `s11b_off.toml` sets nothing the defaults do not). A difference is a bug in this stage (a path that
    runs with the switch 0): find it, do not explain it away.
Tests that must exist at the end (each names its control in a comment): the X1, X3 and G1-G4 tests of spec 4, the two of step 9,
`load_rejects_bad_s11b_state`, `load_rejects_bad_s11b_keys`, save roundtrip and fuzz roundtrip green.
**STOP POINT 2.** `check.sh quick 8d2731b` green (it applies `cargo fmt`; run it last) and the workspace build of step 4 once more; commit;
rewrite the log's top block.

## The stage must log (in `S11b-log.md`, top block rewritten, at most 30 lines)
Commits; SAVE 29 / PROTOCOL 15; test count from `check.sh`; the workspace build result; the parse check result of chunk 0; REF0 and REF1
(paths, Germany's wins); the X1 table (T81 ratio and crew-lost counts, before and after); the `neutral.py` lines for A0-40 and A40; a code
map (`fire_rifle_grenades`, `RgState::for_type`, the low-root wrapper's name, where the issue rule drops the kind, where `cb_last` is set,
cleared and read, the two X1 lines, the refill exclusion); the measured aim error of the grenade at 60, 120 m and full range; any rule built
differently from spec 4, with the reason; what Build B needs: the exact item, weapon and ammo ids, how a member slot carries grenades, the
def-check limits met (`emplace_s`, `speed_cm_s`, `push_men` maxima and minima, mortar `min_range_m`). Not done items by rule id.
If a step cannot be finished: its switch stays at its off value, the neutrality proof is kept, and the log's top block says so first. The
fixes and the layout (chunk 1) are worth more than the grenade mode (chunk 2): if only one fits, it is chunk 1, complete and tested; G stays
unbuilt with `ItemKind::RifleGrenade` in the layout (and on the wire), and Build B skips C5. Never leave the tree with a red `check.sh quick`.
