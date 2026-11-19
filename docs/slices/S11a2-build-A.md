# S11a2 Build A: SAVE 28 layout, keys, friendly fire of mission rounds, counter-battery by sound (all switches still 0)

Read first: `S11a2.md` 2, 3, 4 (F1-F4, K1-K4), 5 (rows "Own burst", "Spot / marker"), 6; the log's top block; `S11a1-log.md` top block (how a
test spawns a gun, an observer team and gives a mission). **2 chunks**, about 30 self-counted turns each (60-90 minutes in all). No AI behaviour, no
content, no JSON in this stage. The four S11a2 switches stay 0 in `skirmish.toml` until Build B, so the default 40 set must equal the base set match
for match, and `b_off` too. Tests turn the switches on in their own setup.
`<BASE>` is the commit this stage starts from (the S11a1 head: Build C, and its fixes if they are in). Write its hash into the log's top block first.
Work rules: `tools/outline.sh` then `tools/show.sh FILE::name` (batch the reads); targeted release tests through `tools/run.sh`;
`tools/check.sh quick <BASE>` once per chunk end, through `tools/run.sh`; **no command blocks longer than 4 minutes** (`tools/run.sh LOG CMD...`,
then `tools/run.sh --wait LOG`); 40 sets only through `tools/batch.sh` with `BATCH_JOBS=4`; never quote tick times; the container can restart:
**commit after every green step** (`git commit -m "S11a2 A: ..." -- FILES`), never `git add -A`, never `target/`, `.claude/`,
`tools/__pycache__/`; if git reports an index lock, wait a few seconds and retry; do not push. No model names in files or commit text.

Files owned: `sdk/crates/content/src/tuning.rs`; `crates/sim/src/{fire_mission.rs,combat.rs,game.rs,gunnery.rs,world.rs,hash.rs,rng.rs,lib.rs}`,
new `crates/sim/src/counter_battery.rs`, new `crates/sim/src/s11a2_tests.rs`; `crates/sim/src/ai.rs` (the new `AiCounters` fields and their
validation only), `crates/sim/src/ai/arty.rs` (the two `AiArty` fields, their validation, `..Default::default()` in literals; no planner code);
`crates/sim/tests/save_fuzz.rs`; `mods/ww2/defs/rules/skirmish.toml`; `docs/modding/{tuning.md,fire-missions.md}`; the BASELINE files
(`sdk/crates/content/tests/resolved_defs.rs`, `crates/simbench/src/report.rs`) and the canonical snapshot; `tools/ablate.sh`, `tools/check.sh` (the
S11a2 dry-run step); `docs/STATE.md` "Versions" line; `docs/slices/targets/S11a2.toml` only to fix what the CLI refuses to parse (say so in the
log); `docs/slices/S11a2-log.md`.

Start from (line numbers are of `a6c5c72`; S11a1 Build C and its fixes moved some files: find every item by name with `tools/outline.sh`):
`combat.rs` (`Burst`, `hurt`: it pushes a death only on the killing hit; `MatchState::burst`: the owner skip and `piece_between`; `fragments`:
the men list, the own-statics skip and the own-frame skip `f.owner.0 == b.player`; where the shell burst is built; the kill loop);
`fire_mission.rs` (`Mission`, `S11Counters`, `known_by_record`, `observe_bursts`: the gun branch, `update_intel`: the whole-second gate);
`gunnery.rs` `gun_mission_tick` (where `gun_rounds[mode]` counts); `game.rs` (`PlayerState`, `order_gun_mission`: the aim offset has a FIXED
length, which K2 must not copy; the load checks of a gun squad's mission); `world.rs` (`SAVE_VERSION`, the Status-stage calls); `rng.rs`
(purposes end at 38); S11a1 A's layout commit `9ed0bf0` as the pattern for a SAVE bump; `tuning.rs` `FireMissionTuning` and the `[ai.arty]` table.

## Chunk 0 (before any edit, 5 minutes): the base set and the targets file
1. `git log --oneline | head -5`, `git status --short`: the tree must be clean of S11a1 work in progress. If Build C's files are uncommitted, stop
   and report: this stage must not start on a moving base.
2. `tools/run.sh target/s11a2-base40.log env BATCH_JOBS=4 tools/batch.sh target/s11a2-base40/skirmish40.json -- skirmish --ai-vs-ai --seed 1
   --seeds 20 --both-bases --summary --minutes 45` on the unchanged tree (it builds nothing: if `target/release/frontline-cli` is older than
   `<BASE>`, run `tools/run.sh target/s11a2-build.log cargo build -q --release -p frontline-cli` first). Log Germany's wins of 40 and the path.
3. The parse check the Plan stage could not run: `target/release/frontline-cli targets docs/slices/targets/S11a2.toml --from-json
   target/s11a2-base40 | tail -45`. Expected: the file loads; T139-T148 print MISSING-METRIC or SKIP (the metrics come in Build B). A parse error is
   fixed in the file now and logged.

## Chunk 1: one layout commit, keys, friendly fire
1. Layout commit (nothing else in it): `SAVE_VERSION` 27 -> 28; every saved field of spec 3 at its zero default (`Mission::cb`, `S11a2Counters`,
   `PlayerState::s11a2`, `PlayerState::cb` with `CbSpot` and `MAX_CB_SPOTS` in the new `counter_battery.rs`, the two `AiArty` fields, the six
   `AiCounters` fields); load validation and hash for each; `save_fuzz.rs` staged save gains the four items of spec 3;
   `load_rejects_bad_s11a2_state` (one case per check), `s11a2_layout_roundtrips_and_hashes`. `PROTOCOL_VERSION` stays 14: say so in the commit
   text. `STATE.md` "Versions" line. Commit.
2. Keys of spec 2 with ranges, comments in `skirmish.toml` (`friendly_fire`, `counter_battery`, `ff_safe`, `counter_fire` all **0**), rows in
   `docs/modding/tuning.md`; `load_rejects_bad_s11a2_keys` (one widened range must fail). `tools/ablate.sh`: the folder `S11a2-ablations` (mode
   `s11a2`: the S10c listing rules, allowed keys of spec 2, neutral file `b_off`, `NEUTRAL_BASE` default `target/s11a2-base40`) and the dry-run step
   in `tools/check.sh quick` beside the S11a1 one. BASELINE and snapshot refreshed (new keys only). Commit.
3. F1-F4 in this order, each with its tests (spec 4): `Burst::ff`; the two owner tests; the roof rule for an own victim; own statics and own
   vehicle frames as ray blockers without damage (F3); `hurt` returning "killed", the scratch list in `DamageCtx` and the kill loop; `ff_kills`, `ff_wounds`, `ff_bursts`, `mortar_near_own_bursts`. Read every other use of `b.player` as an owner
   test in `burst` and `fragments` and leave it (vehicles, statics, wire, tracks: spec F2); list the ones left in the log. The scene helper for
   tests: a `ww2:ger_mortar_team` / `ww2:usa_mortar_team` Direct mission on a point 300 m off on flat ground (`s6_tests::spawn`), a rifle squad
   halted on the point.
**STOP POINT 1.** `check.sh quick <BASE>` green; commit; rewrite the log's top block (commits, what chunk 2 needs, anything of spec 3 that had to
differ); return if more than 45 self-counted turns are used (then chunk 2 is a new session: say so first).

## Chunk 2: sound evidence, markers, knowledge
4. K1 in `counter_battery.rs` (`cb_note`) with the one call in `gun_mission_tick`; K2 and K3 in `cb_tick`, called beside `update_intel`;
   `Purpose::CB_MARK` (39); `decay_at` and `mark_at` rounded up to a whole second when set, the load bounds in ticks (spec 3); the marker
   offset with a uniform length (spec K2). Tests of spec 4 K1-K3 (helpers to fire N gun mission rounds exist in `s11a1_tests.rs`: `crest_gun`,
   `order`, `mission`, `run_out`; the listener is a rifle squad of the other player 800 m from the gun; map fire with `World::mark_intel`).
5. K4: `known_by_record`, `Mission::cb` in `order_gun_mission`, the four counters (`cb_missions`, `cb_rounds`, `cb_bursts`, `cb_near_bursts`); its
   three tests. The load check accepts `cb` only on a gun squad's mission.
6. Row "Spot / marker" and "Own burst" of spec 5: the named cases as tests (`two_guns_within_reset_share_a_spot`,
   `marker_outlives_the_gun_until_decay`, `bombs_in_the_air_still_hurt_after_the_tube_dies`).
7. Neutrality: `tools/run.sh target/s11a2-A40.log env BATCH_JOBS=4 tools/batch.sh target/s11a2-A40/skirmish40.json -- skirmish --ai-vs-ai --seed 1
   --seeds 20 --both-bases --summary --minutes 45`, the same with `-o docs/slices/targets/S11a2-ablations/b_off.toml` into `target/s11a2-Aoff40/`;
   `python3 -I tools/neutral.py target/s11a2-base40 target/s11a2-A40` and `... target/s11a2-Aoff40`: both must print `same 40/40`. A difference is
   a bug in this stage (a path that runs with the switch 0): find it, do not explain it away.
8. One sanity run for Build B, not a result: the 40 set with a scratch override `friendly_fire = 1` (file under `target/`, not committed) into
   `target/s11a2-A40-ff/`: log Germany's wins of 40 and that every match has a winner. No JSON counters exist yet: nothing else is read from it.
Tests that must exist at the end (each names its control in a comment): the F1-F4 and K1-K4 tests of spec 4, the three of step 6,
`load_rejects_bad_s11a2_state`, `load_rejects_bad_s11a2_keys`, save roundtrip and fuzz roundtrip green.
**STOP POINT 2.** `check.sh quick <BASE>` green (it applies `cargo fmt`; run it last); commit; rewrite the log's top block.

## The stage must log (in `S11a2-log.md`, top block rewritten, at most 30 lines)
`<BASE>` hash; commits; SAVE 28 / PROTOCOL 14; test count from `check.sh`; `neutral.py` lines for A40 and Aoff40; the parse check result of chunk
0; a code map of the new functions (`cb_note`, `cb_tick`, the scratch list's name, where `Burst::ff` is set, where `Mission::cb` is set); the owner
tests left untouched (F2); any rule built differently from spec 4, with the reason; what Build B needs: how a test or a fortify row reads a
player's markers and `S11a2Counters`, how it turns the switches on, how S1 can read a squad's `nav.goal` and lead from the planner, and whether `Stop`
on a gun or mortar squad ends its mission in one tick. Not done items by rule id.
If a step cannot be finished: its switch stays 0 (it is 0 anyway), the neutrality proof is kept, and the log's top block says so first. Friendly
fire (chunk 1) is worth more than markers (chunk 2): if only one fits, it is chunk 1, complete and tested. Never leave the tree with a red
`check.sh quick`.
