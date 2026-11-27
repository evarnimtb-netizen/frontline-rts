# S14a Build A: layout (SAVE 32 -> 33, PROTOCOL 16 -> 17), keys, flame F1-F9 (every S14 switch off)

Read first: `S14-assault-kit.md` (the owner-change note at the top, then sections 0, 1, 2, 3, 8, 9 rows R1-R2, 10, 13 risk 7);
`S14-research-code-audit.md` (file:line audit made on `6f7bda0`: find things by name, lines have moved); `S12-log.md` and
`S13-log.md` top blocks (patterns of the last two layout commits); `STATE.md` "How to check work". `<BASE>` is `a27ce63` (main:
S13 and B3 merged; SAVE 32, PROTOCOL 16). Branch `claude/project-thread-aj1tww`. **2 chunks**, about 30 self-counted turns each.
Neutral by construction: every S14 switch stays at its off value (`[flame] enabled` 0, `[melee] enabled` 0, `[clear] enabled` 0,
`[ai.flame]` all 0); no flame weapon or card exists yet (Build B), so with switches off no new code path runs and no RNG draw changes.
Tests turn switches on in the test only and use test-made defs.

Owner rule for this slice (2026-11-25): **no balance runs.** Every number you add is a TOML key with a range and a comment, so a
later balance pass can change it without code. Do not tune anything.

Work rules: `tools/outline.sh` then `tools/show.sh FILE::name` (batch the reads); targeted release tests through `tools/run.sh`;
**no command blocks longer than 4 minutes** (`tools/run.sh LOG CMD...`, then `tools/run.sh --wait LOG`); 40 sets only through
`tools/batch.sh` with `BATCH_JOBS=4`, **two in this stage** (steps 1 and 9); never quote tick times; **commit after every green step**
(`git commit -m "S14a A: ..." -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`,
`godot/match/fort_view.gd.uid`; if git reports an index lock, wait and retry; do not push. No model names in files or commit text.
The machine has 4 cores. The cloud has no IPv6: `udp::ipv6_loopback_handshake` fails here only.

## Chunk 0: base
1. `git log --oneline | head -3`. **Base 40** on the unchanged code: `tools/run.sh target/s14-base40.log env BATCH_JOBS=4
   tools/batch.sh target/s14-base40/skirmish40.json -- skirmish --ai-vs-ai --seed 1 --seeds 20 --both-bases --summary --minutes 45`
   (start it in the background and go on; it is cached by `batch.sh`).

## Chunk 1: layout commit and keys
2. **Layout commit (nothing else):** plan 3.1 in full. `SAVE_VERSION` 32 -> 33, `PROTOCOL_VERSION` 16 -> 17 with `ClearTrench
   { squad, at }` and `Strike { units }` declared with their wire encoding and refused by the sim with a clear reason until Builds B
   and C (relay doc line in `sdk/docs/net/relay-protocol.md`, the net test pinning the number, `crates/godot-bridge/src/intents.rs`
   only to keep the build green). `MatchState::flame`/`next_flame`, `patches`/`next_patch`, `clears`; `Combat::close: Close`;
   `kill_class` FLAME 12 / MELEE 13, `fired_class` FLAME; `PlayerState::s14: S14Counters`; the `AiCounters` fields; all at zero
   defaults, hashed with their neighbours, load-validated (`load_rejects_bad_s14_state`, one case per check), `s14_layout_roundtrips_
   and_hashes`; `save_fuzz.rs` staging gains particles in flight, a patch, a burning man (a man mid-strike and a squad mid-clear
   come with Builds C and B). `STATE.md` "Versions" line. `cargo build --release --workspace` once. Pattern: S12's `e847067` and
   S13's layout commit (`git log --oneline --grep "S13a A"`). Commit.
3. Keys of plan section 2 (all tables, `[ai.flame]` included) with ranges and comments in `mods/ww2/defs/rules/skirmish.toml` and
   `sdk/crates/content/src/tuning.rs`; weapon def key `class = "flame"` and member key `when` parsed and validated (no def uses them
   yet); rows in `docs/modding/tuning.md`; `load_rejects_bad_s14_keys`. `targets/S14-ablations/` with `s14_off.toml` (all switches 0),
   `flame_only.toml`, `no_flame_ai.toml`, `no_clear.toml`, `no_melee.toml`, `no_patches.toml` (written now, run by a later balance
   pass); teach `tools/ablate.sh` the folder as S13's was taught (`NEUTRAL_BASE` default `target/s14-base40`) and its dry run in
   `check.sh quick`. `targets/S14.toml` (copy of the current targets plus T185-T188 and the reports of plan 10; no pooled threshold:
   owner change). Refresh BASELINE files (`sdk/crates/content/tests/resolved_defs.rs`, `crates/simbench/src/report.rs`) and the
   canonical snapshot for new keys only. Commit.

## Chunk 2: the flame rules (each its own commit with its named tests)
4. F1 fire: `WeaponClass::Flame` through `aim_and_fire`, the `FireRequest` flame flag, `spawn_rounds` routing to `MatchState::flame`
   (never the pool), the per-player cap and `flame_capped`, fuel as the feed, crew handover and sidearm when dry. Risk 7: read the S4
   crew-weapon pickup and test `flame_handover_keeps_fuel`. Commit.
5. F2 flight (Ballistics stage, serial, after `fly_thrown`), F3 burn, F4 fear, F8 credit (FLAME in `kills_by`, `downed_by_class`,
   `finished_by_class`; own burns are own fire, not kills). Named tests per rule incl. the bunker slit (particle through the gap hits
   a man inside; closed slit: none). Commit.
6. F5 burning ground (patches, merge, cap, pulse, any owner), F6 step out, F7 fuel resupply through the existing explosives kind and
   price. Named tests. Commit.
7. F9 `flame_never_takes_pool_slots` and `two_flamers_stay_in_cap`; rows R1 (slit, hard) and R2 (pool, hard) in a new
   `sdk/crates/cli/src/fortify/s14.rs` scene (pattern `fortify/s12*.rs` / `s11b.rs`), switches on in the row only, using test-made
   flame defs until Build B adds the real ones (say so in the log); rows into `targets/S14.toml`.
8. Snapshot fields for the presenter (`flame` particles, `patches`, a burning flag per man) in `crates/sim/src/snapshot.rs` and
   `godot/docs/match_state.md` "S14"; JSON `s14_blocks` (counters per faction) in `sdk/crates/cli/src/json_out.rs`. Commit.

## Chunk 3: proof
9. **T185:** the default 40 set (all S14 switches 0) into `target/s14-A-40/`; `python3 -I tools/neutral.py target/s14-base40
   target/s14-A-40` must show no difference except new zero-valued S14 counter keys (list them in the log). Any other difference is
   a bug of this stage: bisect, fix, re-run.
10. Determinism: `tools/check.sh quick a27ce63` green (through `tools/run.sh`); the thread-count hash test (1, 2, 3, 8 threads) with
    `[flame] enabled` 1 in a short fortify scene; save fuzz (0 panics, 0 unloadable) with the staged items.
11. Write `docs/slices/S14-log.md` "For the next stage" block (at most 30 lines): state, code map (where each rule lives), what Build
    B must know (refused commands, test-made flame defs to replace), every deviation from the plan and why.

Files owned: `crates/sim/src/**` (except AI job code beyond `AiCounters`), new `crates/sim/src/s14_tests.rs`,
`crates/sim/tests/save_fuzz.rs`, `crates/net/src/protocol.rs` and the net test pinning the number, `sdk/docs/net/relay-protocol.md`,
`crates/godot-bridge/src/intents.rs` (build only), `sdk/crates/content/src/**`, `mods/ww2/defs/rules/skirmish.toml`,
`sdk/crates/cli/src/{fortify/,json_out.rs,skirmish*}`, BASELINE files and the snapshot, `tools/ablate.sh`, `tools/check.sh`,
`docs/modding/tuning.md`, `docs/STATE.md` "Versions" line, `godot/docs/match_state.md`, `docs/slices/targets/S14*`,
`docs/slices/S14-log.md`. Not the presenter's GDScript.
