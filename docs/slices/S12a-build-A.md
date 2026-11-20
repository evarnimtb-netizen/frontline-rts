# S12a Build A: layout (SAVE 30 / PROTOCOL 16), keys, wound states W1-W5, W12-W14 (every S12 switch 0)

Read first: `S12-wounds-medics.md` (the whole plan, 223 lines; sections 0, 2, 3, 4, 6 row "Bandage", 7, 9, 11); `S11b-log.md` top
block (patterns of the last layout commit); `STATE.md` "How to check work". **2 chunks**, about 30 self-counted turns each.
`<BASE>` is `1209c99` (main: S11b + B2 merged). Branch `claude/project-thread-aj1tww`. Every switch of spec 2 stays at its off value
through this stage (`[wounds] enabled` 0, `reinforce` 0, `[ai.medic]` all 0), so play never changes: with `enabled` 0 a soldier at
0 HP dies exactly as at `<BASE>` and no new RNG draw happens.

Work rules: `tools/outline.sh` then `tools/show.sh FILE::name` (batch the reads); targeted release tests through `tools/run.sh`;
`tools/check.sh quick 1209c99` once per chunk end, through `tools/run.sh`; **no command blocks longer than 4 minutes**
(`tools/run.sh LOG CMD...`, then `tools/run.sh --wait LOG`); 40 sets only through `tools/batch.sh` with `BATCH_JOBS=4`, **two in this
stage** (steps 1 and 9; a third only to bisect a failed proof); never quote tick times; **commit after every green step**
(`git commit -m "S12a A: ..." -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`; if git reports an
index lock, wait and retry; do not push. No model names in files or commit text. The machine has 4 cores.

## Chunk 0: the base set
1. `git log --oneline | head -3` (head `1209c99` or docs commits on it). **Set 1 (REF0):** `tools/run.sh target/s12-base40.log env
   BATCH_JOBS=4 tools/batch.sh target/s12-base40/skirmish40.json -- skirmish --ai-vs-ai --seed 1 --seeds 20 --both-bases --summary
   --minutes 45` on the unchanged tree. Log Germany's wins of 40 and `kills_by_source_pct` (plan risk 5).

## Chunk 1: layout commit and keys
2. **Layout commit (nothing else):** `SAVE_VERSION` 29 -> 30, `PROTOCOL_VERSION` 15 -> 16 (relay doc line in
   `sdk/docs/net/relay-protocol.md`; the four commands of plan 3 are declared now with their wire encoding but refused by the sim with
   a clear reason until Build B), `EntityKind::Downed = 6`, `Combat::wound: Wound`, `MatchState::casualties`, `Structure` beds field,
   the widened `Fallen`, `PlayerState::s12: S12Counters`, the `AiCounters` fields, RNG purpose `WOUND_BLEED`, `StockKind::Aid` (no
   behaviour yet), every one at its zero default; load validation and hash for each; `load_rejects_bad_s12_state` (one case per check),
   `s12_layout_roundtrips_and_hashes`; `save_fuzz.rs` staging gains the plan-3 items that exist after this stage (a man mid-bandage, a
   downed man); `STATE.md` "Versions" line. `cargo build --release --workspace` once through `tools/run.sh`. Pattern: S11a2's
   `13b9c82` (`git show --stat 13b9c82`). Commit.
3. Keys of plan 2 with ranges and comments in `mods/ww2/defs/rules/skirmish.toml` and `sdk/crates/content/src/tuning.rs`; rows in
   `docs/modding/tuning.md`; `load_rejects_bad_s12_keys`. `targets/S12-ablations/` with `s12_off.toml` (all switches 0),
   `wounds_only.toml` (`enabled` 1, rest 0), `head_300.toml` (`enabled` 1, head 300; a report); teach `tools/ablate.sh` the folder as
   S11b's was taught (`NEUTRAL_BASE` default `target/s12-base40`), and its dry run in `check.sh quick`. Refresh BASELINE files
   (`sdk/crates/content/tests/resolved_defs.rs`, `crates/simbench/src/report.rs`) and the canonical snapshot for new keys only. Commit.

## Chunk 2: the rules (each its own commit with its named tests; all tests turn `enabled` on in the test only)
4. The Downed site list of plan 0: `downed_spatial` (rebuilt where the soldier hash is), the hit paths `fly` and `burst` reading it,
   `population`, `spotting.rs` `seen_by` refresh, the exhaustive matches (`combat.rs`, `snapshot.rs`, `ai/guns.rs`), `rules.rs` (W14).
   Find each by name; line numbers in the plan are of `8d2731b`.
5. W1 zones, W2 damage, W3 outcome for **every** way to 0 HP (`hurt`/`hurt_by`, rider core hits, gun crews, mines, own fire), one
   test per source (plan risk 6). `Fallen` carries `Hit { raw, zone, blast }`; stop using `saturating_sub` for the outcome.
6. W4 wounded (penalties, squad paces to its slowest wounded man beside the S6 crate-carrier pace, bleed, self-bandage incl. walking,
   broken by firing or a new hit), W5 downed (leaves squad as `kill` does; riders and gun crew set down at the S8a exit point; seeded
   bleed-out; shared `credit_kill` factored out of the kill loop and used by both), W12 (nobody aims at Downed; strays hit them), W13
   credit (`downed_by_class`), W14. Plan 6 row "Bandage" tests (self and buddy no double count is Build B's buddy half: test the self
   half now). Save/load mid-bandage and with a downed man: hashes equal.
7. Fortify rows: a new `wounds` scene in `sdk/crates/cli/src/fortify/` (pattern `sdk/crates/cli/src/fortify/s11b.rs`), switches on in the row only: **R7
   (hard)** trench men under rifles lose no more than `<BASE>` + 10 % (also under `head_300` as a report) and **R8** prone men under MG
   fire die no faster than `<BASE>`. Rows print through `frontline-cli fortify`; add them to `targets/S12.toml` (new; pattern
   `targets/S11b.toml`), with the plan-8 metrics that exist so far as reports.
8. JSON: `s12_blocks` in `json_out` (counters of plan 3 per faction) so later stages can read them; no `s12_derived` thresholds yet.

## Chunk 3: proof
9. **Set 2 (REF1):** the default 40 set (all S12 switches 0) into `target/s12-A-40/`; `python3 -I tools/neutral.py
   target/s12-base40 target/s12-A-40` must show no difference except new zero-valued S12 counter keys (list them in the log). Any
   other difference is a bug of this stage: bisect, fix, re-run.
10. Determinism: `tools/check.sh quick 1209c99` green; the thread-count hash test (1, 2, 3, 8 threads) with `enabled` 1 in a short
    fortify scene; save fuzz (0 panics, 0 unloadable) with a man mid-bandage and a downed man staged.
11. Write `docs/slices/S12-log.md` "For the next stage" block (at most 30 lines): state, code map (where each rule lives), what Build
    B must know (the refused commands, `StockKind::Aid` sites still to audit), every deviation from the plan and why.

Files owned: `crates/sim/src/**` (except AI job code beyond `AiCounters`), new `crates/sim/src/s12_tests.rs`, `crates/sim/tests/save_fuzz.rs`,
`crates/net/src/protocol.rs` and the net test pinning the number, `sdk/docs/net/relay-protocol.md`, `crates/godot-bridge/src/intents.rs`
(only to keep the build green), `sdk/crates/content/src/{tuning.rs,defs.rs,setup.rs}`, `mods/ww2/defs/rules/skirmish.toml`,
`sdk/crates/cli/src/{fortify/,json_out.rs,skirmish*}`, BASELINE files and the snapshot, `tools/ablate.sh`, `tools/check.sh`,
`docs/modding/tuning.md`, `docs/STATE.md` "Versions" line, `docs/slices/targets/S12*`, `docs/slices/S12-log.md`.
