# S11b Build C: JSON, fortify rows, the AI's howitzer and light mortar, switches on, 40 sets

Read first: `S11b.md` 4 (M1-M2, H1-H3, L1), 5 (row "Howitzer buy"), 7, 8 (the fallback if time runs out); the log's top block (Build A's
code map, Build B's card ids and recipes, REF1); `S11a1-log.md` "Build C" (how `buy_arty` and the pot were tested); `S11a2-build-B.md` as
the pattern of this stage. **2 chunks**, about 30 self-counted turns each (60-90 minutes in all). No layout change (every field exists
since Build A), no version change. Chunk 1 is a complete result by itself: if chunk 2 does not fit, `light` and `rifle_grenades` stay 0,
their rows are null, and the log says so first. **40 sets: three in this stage** (steps 3, 4, 7; the second set of step 7 only if turns
are left), a fourth only to bisect a failed proof.
Work rules as in Build A (4-minute rule, `tools/run.sh`, `BATCH_JOBS=4 tools/batch.sh`, commit after every green step and only your files,
index lock: wait and retry, no push, no tick times, no model names). No RNG in any plan; ascending ids and roster order; integer metres and
seconds. `<BASE>` is `8d2731b`.

Files owned: `crates/sim/src/ai/arty.rs` (H1: `arty_pot`, `buy_arty`, a howitzer-card helper), `crates/sim/src/ai/truck.rs`
(`buy_role_within` and `of_role`: a card filter and a manpower argument, the old callers unchanged in effect), `crates/sim/src/ai.rs` (L1 in `buy`),
`crates/sim/src/ai/ammo.rs` (H3), new `crates/sim/src/s11b_ai_tests.rs`; `crates/sim/src/gunnery.rs` only if `how_rounds` was not wired in
Build A; new `sdk/crates/cli/src/fortify/s11b.rs`, `sdk/crates/cli/src/fortify/s11a1.rs` (make `mask_once`, `barrage_once`, `spawn`,
`order`, `run_out`, `par_seeds` take a card and be `pub(super)`; the S11a1 rows keep their values), `sdk/crates/cli/src/{fortify.rs,
json_out.rs,skirmish.rs}`; `tools/neutral.py` (+ `tools/test_neutral.py`); `mods/ww2/defs/rules/skirmish.toml` (the five switches to 1),
the two howitzer cards and two light mortar cards (`ai_weight` only); `crates/sim/tests/save_fuzz.rs` (`save_roundtrip_carries_s11b_state`);
the BASELINE files and the snapshot; `docs/modding/{tuning.md,fire-missions.md}`; `docs/slices/targets/S11b.toml` only to fix a metric name
that cannot be built as written (say so in the log); `docs/slices/S11b-log.md`.

Start from (find by name): `json_out.rs` `s11a2_blocks`, `skirmish.rs` (`S11a2Faction`, `s11a2_json_names_match_the_targets_file`, the HE
share at about 2484, the per-vehicle kill map behind `kills_by_vehicle_pct`); `fortify/s11a2.rs` (a row with its control computed in the
same call; the one-off row test); `ai/arty.rs` `arty_pot` 258-296, `buy_arty` 306-321, `arty` 332-432 (records are made for every own
`inf_gun` squad); `ai/truck.rs` 893-967; `ai.rs` `buy` 1699-2060 (the per-slot card lists at 1717-1722, the pick at 2034-2054), `mix_slot`
1105; `ai/ammo.rs` 217, 268; `content/src/setup.rs` `tag` 364, `has_tag` 1311.

## Chunk 1: JSON, howitzer rows, the howitzer AI, `how` on
1. M1 JSON: `factions.<f>.s11b` and `s11b_derived` (spec 7 names; null where the denominator is 0); `kills_by_source_pct.rifle_grenade`;
   the HE share = mortar + tank HE + rifle grenade (0 more with the switch off); `--side-detail` gets the raw counters; `tools/neutral.py`
   skips keys starting `s11b` that BASE lacks (+ a line in `tools/test_neutral.py`); `s11b_json_names_match_the_targets_file`. Commit.
2. M2 rows `how_mask` (the gun 600 m from the target, D 1492; the S11a1 row's 530 m stays for T129) and `how_barrage` in
   `fortify/s11b.rs`, in the fortify JSON under `s11b` and in the text summary; field names exactly as `targets/S11b.toml` T150-T151 reads
   them; player commands only; each control computed in the same call. A one-off test
   `s11b_rows_one_off` (seed 1, prints the values) so that no stage waits for the whole `fortify-test` (28 minutes): start
   `tools/run.sh target/s11b-C40/fortify10.log target/release/frontline-cli fortify-test --seeds 10 --json target/s11b-C40/fortify10.json`
   in the background once the rows exist and poll it with `tools/run.sh --wait` between other steps; if it has not ended at the stage's
   end, log its path and the one-off values. `fortify_s11b_rows_run`. Commit.
3. H1 with its tests (the two `spent` sums apart, the howitzer's own share, pending call-ins subtracted: always for the howitzer, for
   `buy_arty` only with `buy_pending` 1), H2's two tests (no planner code is expected for H2: if one fails, the cause is logged before any
   change to `run_arty`), `ai_all_s11b_switches_off_emit_nothing_new`, `ai_s11b_deterministic_threads_and_save_load`. `ai_weight = 5` on
   the two howitzer cards (the S11a1 gun cards' value). With every switch still 0: **set 1** `target/s11b-C1off40/`, `neutral.py` against
   `target/s11b-A0-40`: `same 40/40` (the infantry-gun buy must not have moved). Commit.
4. `how = 1`, `buy_pending = 1`, `cb_alternate = 1` in `skirmish.toml`; **set 2** `target/s11b-C1-40/`: log Germany's wins, no match
   without a winner, per faction `how_window_matches_pct`, `how_bought_matches_pct`, `how_bought_window_pct` (expected: bought at about
   minute 11-12, so most window matches), `how_fired_matches_pct`, `how_rounds_per_match`, `how_pct_holds`, `how_mp_holds`,
   `s11a2_derived.cb_shell_share_pct` and `cb_fired_matches_pct` (the diagnosis read USA 47.3 % and 97 %), `cb_alt_holds_per_match`, the
   M10, M3A1 and StuG rows, gun refused 0, HE share mean and max. If no howitzer is bought, read the two hold counters and log which gate
   said no; do not change a default to force a buy (the share and the reserve are owner values, S11b-3). Commit.
**STOP POINT 1.** `check.sh quick 8d2731b` green; commit; rewrite the log's top block; return if more than 45 self-counted turns are used.

## Chunk 2: light mortars, grenades, the remaining rows, the sets
5. L1 with its test; `ai_weight` on the two light cards as spec L1 says (log the resolved weights); `light = 1`. H3: its test first; code
   in `ai/ammo.rs` only if the test shows a trip for grenades.
6. Rows `rg` and, last, `light` (spec M2); `rifle_grenades = 1`; `save_roundtrip_carries_s11b_state` (shipped switches: a save that holds a
   squad mid-aim and a howitzer record; hash, bytes and a continued run equal). BASELINE and snapshot refreshed. Commit.
7. The sets, each through `tools/run.sh` and `tools/batch.sh` (seeds 1-20 both bases, 45 minutes): **set 3** `-o
   docs/slices/targets/S11b-ablations/s11b_off.toml` into `target/s11b-Coff40/`, with `python3 -I tools/neutral.py target/s11b-A0-40
   target/s11b-Coff40`: `same 40/40` (the proof comes before the reading); then, if turns are left, the default `target/s11b-C40/`. Every
   ablation (`rg_off`, `how_off`, `how_share_10`, `light_off`, `a0_off`) is the orchestrator's (`tools/measure.sh` plays the control runs).
8. `target/release/frontline-cli targets docs/slices/targets/S11b.toml --from-json target/s11b-C40 | tail -60` (or the latest default
   set's folder): log pass / miss / skip
   counts and every T149-T158 line that is not PASS or SKIP; no MISSING-METRIC, CONTROL-MISSING or EXPOSURE-MISSING may remain for a row
   whose rule was built.
**STOP POINT 2.** `check.sh quick 8d2731b` green (run it last); commit; rewrite the log's top block.

## The stage must log (top block rewritten, at most 30 lines)
Commits; test count from `check.sh`; the `neutral.py` lines (C1off40, Coff40); from the default 40 set, Germany / USA: Germany's wins,
winners in all 40, HE share mean and max beside `s11b_off`, howitzer bought and fired, rounds, kill share, masked holds; light mortars
bought and their buy share, mortar bombs a match; grenades fired a match and their kill share; counter-fire share, answered matches and
`cb_alt_holds`; T81 ratio; M10, M3A1 and StuG bought. The rows (one-off or full run): masked %, trench against open,
grenades fired and losses with their controls. The targets line counts. Any rule built differently from spec 4, with the reason. Not done
items by rule id. What the Review must look at first (the buy path that changed for the infantry gun, the HE share's new term, `rg` saves).
Guard rails to report, not to fix here: Germany's wins on 40 under 12 or over 28 (noise is 11-15 points: the 480 decides), HE share mean
above 26.3, a match without a winner, a refused AI order. Each is the first line of the log's top block if seen.
If a step cannot be finished: its switch stays 0, the neutrality proof is kept, the row's keys are null, and the log says so first. Never
leave the tree with a red `check.sh quick`.
