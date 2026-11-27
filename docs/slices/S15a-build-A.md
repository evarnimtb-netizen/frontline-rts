# S15a Build A: layout (SAVE 33 -> 34), keys, matchups, riders, plates, captured bombs, bayonets, rows W1-W4 (switches 0)

Read first: `S15-ussr.md` sections 0-5, 9, 10 (W1-W4), 11 (T210), 12 row A, 14; `S15-research-code-audit.md`; `S14-log.md` top block
(the last layout commit's pattern); `STATE.md` "How to check work". `<BASE>` for neutrality is the S14 head `96d4b4b` (its 40 set is
`target/s14-F-40`, switches on as shipped: use it as the base for T210, see below). Branch `claude/project-thread-aj1tww`.
**2 chunks**, about 30 self-counted turns each. Every S15 switch stays at its off value; tests and rows turn switches on in the test or
row only, with stand-in defs under `mods/ww2/tests/` (no USSR def in `mods/ww2/defs/` yet: Builds B and C).

Owner rule (2026-11-25): **no balance runs, no tuning.** Every number is a TOML key with a range and a comment.

Work rules: `tools/outline.sh` then `tools/show.sh FILE::name`; targeted release tests through `tools/run.sh`; **no command blocks
longer than 4 minutes** (`tools/run.sh LOG CMD...`, then `tools/run.sh --wait LOG`); 40 sets only through `tools/batch.sh` with
`BATCH_JOBS=4`, **one in this stage** (T210, step 7; owner-approved neutrality check; if refused, skip it and say so); **commit after
every green step** (`git commit -m "S15a A: ..." -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`,
`godot/match/*.gd.uid`; do not push. No model names in files or commit text. The cloud has no IPv6: `udp::ipv6_loopback_handshake`
fails here only.

## Chunk 1
1. **Layout commit (nothing else):** plan section 3 in full, `SAVE_VERSION` 33 -> 34 (no PROTOCOL change unless the plan's audit
   proves one is needed: then stop and say so in the log), every new saved field at zero, hashed, load-validated
   (`load_rejects_bad_s15_state`), `s15_layout_roundtrips_and_hashes`, `save_fuzz` staging for what exists after this stage;
   `STATE.md` "Versions". Commit.
2. Keys of plan section 2 with ranges and comments in `skirmish.toml` and `tuning.rs`; `tuning.md` rows; `load_rejects_bad_s15_keys`;
   `targets/S15-ablations/s15_off.toml` (+ one file per part, written for a later balance pass); `tools/ablate.sh` taught the folder
   and its dry run in `check.sh quick`; `targets/S15.toml` (T210-T218 as the plan's section 11). BASELINE refreshes for new keys only.
   Commit.
3. Section 4 matchups (content `resolve`, CLI `--faction0/--faction1`, bridge/menu only as far as the build needs; Germany-USA
   default unchanged) with tests. Commit.

## Chunk 2
4. Section 5 rules, each its own commit with its named tests: 5.1 riders, 5.2 plates, 5.3 captured bombs, 5.4 bayonets (reuse S14's
   melee: a bayonet is a reach on the weapon's `[melee]` block or the plan's key). Resolve the plan's unaudited points (risk 4) by
   reading the code; state each answer in the log.
5. Rows W1-W4 (hard) in a new `sdk/crates/cli/src/fortify/s15.rs`, switches on in the row only, wired into `json_out`'s checks;
   rows into `targets/S15.toml`. JSON `s15_blocks` (counters per faction). Commit.
6. Snapshot fields the presenter will need (riders on deck, plate hits if any) in `snapshot.rs` and `godot/docs/match_state.md` "S15".
   Commit.
7. **T210:** the default 40 set into `target/s15-A-40/` (`tools/run.sh target/s15-A-40.log env BATCH_JOBS=4 tools/batch.sh
   target/s15-A-40/skirmish40.json -- skirmish --ai-vs-ai --seed 1 --seeds 20 --both-bases --summary --minutes 45`); `python3 -I
   tools/neutral.py target/s14-F-40 target/s15-A-40` must say same 40/40 (new zero-valued S15 keys allowed; teach `neutral.py` any
   grown list with a zero tail, with its test). Any other difference is a bug of this stage: find and fix it.
8. `tools/check.sh quick 96d4b4b` green (IPv6 excepted); thread-count hash test (1, 2, 3, 8) with S15 switches on in a short row
   scene; save fuzz 0 panics, 0 unloadable. Write `docs/slices/S15-log.md` (new; S14-log's shape) with the "For the next stage" block
   (at most 30 lines): state, code map, what Build B needs, every deviation.

Files owned: `crates/sim/src/**` (not `ai/` beyond counters), new `crates/sim/src/s15_tests.rs`, `crates/sim/tests/save_fuzz.rs`,
`sdk/crates/content/src/**`, `sdk/crates/cli/src/**` (fortify, json_out, skirmish, match setup), `mods/ww2/defs/rules/skirmish.toml`,
`mods/ww2/tests/`, BASELINE files and the snapshot, `tools/ablate.sh`, `tools/check.sh`, `tools/neutral.py`, `tools/test_neutral.py`,
`docs/modding/tuning.md`, `docs/STATE.md` "Versions", `godot/docs/match_state.md`, `crates/godot-bridge/` (build only),
`docs/slices/targets/S15*`, `docs/slices/S15-log.md`. Not the presenter's GDScript.
