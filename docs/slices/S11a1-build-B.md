# S11a1 Build B: the two guns, the two observer teams, fortify rows, JSON, neutrality

Read first: `S11a1.md` 4 (C1-C6), 7, 8 (the gate after B); the log's top block (Build A's code map). **2 chunks**,
about 30 self-counted turns each (60-90 minutes in all). No save layout change, no version change, no AI code. The
AI buys none of the new cards in this stage (`[ai.arty] buy` 0, observer cards `ai_weight` 0), so the default 40
set must still equal `target/s11a1-base40` match for match.
Work rules as in Build A (4-minute rule, `tools/run.sh`, `BATCH_JOBS=4 tools/batch.sh`, commit only your files,
no push, no tick times). No new model or art: the presenter is not touched.

Files owned: new `mods/ww2/defs/vehicles/{ger_leig18,usa_105mm_m3}.toml`, `weapons/{leig18,m3_105mm}.toml`,
`ammo/{75mm_igr18,105mm_m1_he}.toml`, `units/{ger_leig18_squad,ger_leig18_crew,usa_105mm_m3_squad,
usa_105mm_m3_crew,ger_vb_trupp,usa_fo_party}.toml`; `mods/ww2/defs/factions/{germany,usa}.toml` (the four cards
at the END of each roster); new `sdk/crates/cli/src/fortify/s11a1.rs`, `sdk/crates/cli/src/{fortify.rs,
json_out.rs,skirmish.rs}`; new `sdk/crates/content/tests/s11a1_content.rs`; `tools/neutral.py` (+
`tools/test_neutral.py`); BASELINE files and canonical snapshot (new defs); `docs/modding/fire-missions.md`;
`docs/slices/targets/S11a1.toml` only to fix a metric name that cannot be built as written (say so in the log);
`docs/slices/S11a1-log.md`.

Start from: `mods/ww2/defs/vehicles/ger_pak38.toml` and `usa_57mm_m1.toml` (boxes, `[[service]]`, `[mobility]`),
`units/ger_pak38_squad.toml`, `ger_pak38_crew.toml`, `ger_mortar_team.toml` (traits), `weapons/grw34.toml`
(`[indirect]`), `ammo/75mm_sprgr34.toml` (`[he]`); `sdk/crates/cli/src/fortify/s10c_ai.rs` (a row with control
and summary text, `belt_row`), `fortify/s10a.rs` (`breach_ai_world`, `breach_ai_run`: the scene Build C extends),
`mortar_bench.rs` (`run`: staging a mission and reading the miss from `take_burst_log`); `json_out.rs`
(`s10c_blocks`), `skirmish.rs` (`S10cSeen`, `s10c_json_names_match_the_targets_file` at about 4266).

## Chunk 1: content
1. C4 rounds, then C1 and C2 guns, then C3 observer teams, exactly the figures of spec 4; every figure carries its
   source in a comment (design table and section) and "verify" where the design says so. Service slots and boxes:
   the Pak 38 layout with the shield lowered to the gun's height (leIG 18: shield 1.0 m wide, 0.9 m tall; M3: the
   57 mm M1 carriage layout, shield 1.6 m wide); thin boxes as S9 (a bullet that misses the shield reaches a man).
   The direct-fire weapon line (`range_m`, `spread_urad`, `reload_ms`) as the S9 guns of the same faction:
   600 m, 2500 urad (verify), reload from the rate of spec 4. The crew commander has `traits = ["leader", "radio"]`.
2. `s11a1_content.rs` (spec 4 C6 tests): the four cards and two rounds load and validate; both guns solve 300 m
   and 900 m on flat ground and the M3 never above 30 deg; a card costs what spec 4 says; `a_off.toml` holds every
   S11a1 switch of `skirmish.toml` (as `s10c_ai_content.rs` does for `c_off`).
3. `frontline-cli validate mods/ww2` clean; BASELINE and snapshot refreshed (new defs only: no old value moves).
4. Neutrality now, before any CLI work: the default 40 set into `target/s11a1-B1-40/` and `neutral.py` against
   `target/s11a1-base40`: `same 40/40`. If the new roster entries change a match, find the code that reads the
   roster by position or count (spec 10 risk 6) and fix that, not the test.
**STOP POINT 1.** `check.sh quick 730e67f` green; commit; log top block; return if over 45 self-counted turns.

## Chunk 2: rows, JSON
5. C5 rows in `fortify/s11a1.rs`, in the fortify JSON under `s11a1` and in the text summary: `gun_fm`, `mask`,
   `barrage`, each with its control; field names exactly as `targets/S11a1.toml` T128-T130 reads them. Every row is
   driven by player commands (`CommandKind::FireMission` on the gun's squad, `Emplace` first), not by AI code. Keep
   the added fortify time under 60 s for 10 seeds (log it); `breach_prep` is Build C's: emit its keys as null so
   the targets print SKIP, not MISSING-METRIC.
6. C6 JSON: `factions.<f>.s11a1` (every `S11Counters` and S11a1 `AiCounters` field by name, the two new refusal
   counts) and `s11a1_derived` (spec 7 names; null where the denominator is 0). `--side-detail` gets the raw
   counters. `tools/neutral.py`: a key starting `s11a1` that BASE lacks is skipped (as `s10c`), with a test line
   in `tools/test_neutral.py`.
7. Run: `tools/run.sh target/s11a1-fort.log env BATCH_JOBS=4 tools/batch.sh target/s11a1-B40/fortify10.json --
   fortify-test --seeds 10` (poll with `--wait`; if one call would pass 4 minutes, that is what `run.sh` is for);
   the default 40 set into `target/s11a1-B40/skirmish40.json`, `a_off` into `target/s11a1-B40/skirmish40_a_off.json`;
   `neutral.py target/s11a1-base40 target/s11a1-B40` -> `same 40/40`. Then
   `target/release/frontline-cli targets docs/slices/targets/S11a1.toml --from-json target/s11a1-B40`: no
   `MISSING-METRIC`, `CONTROL-MISSING` or `EXPOSURE-MISSING` on T128-T130 and on T132-T138 (the skirmish rows will
   read 0 or null and miss: the AI buys nothing yet).
Tests: `fortify_s11a1_rows_run` (1 seed, each row's control differs from its value),
`s11a1_json_names_match_the_targets_file`, `neutral_skips_s11a1_keys_the_base_lacks`.
**STOP POINT 2.** `check.sh quick 730e67f` green, last; commit; rewrite the log's top block.

## The stage must log
Commits; the content table as built (mass, crew, charges, band, rounds, cost, pop, cooldown, unlock per card, with
any figure changed from spec 4 and why); T128, T129, T130 values with exposure and control (the gate of spec 8:
the orchestrator reads them before Build C starts); fortify wall time before and after; `neutral.py` lines;
test count; for Build C: how a test spawns a gun, an observer team and gives a mission, the card ids, and whether
`buy_role_within` needs `ai_weight` > 0 on the gun cards (and what value was set).
If T128 or T130 misses: do not change lethality, fragment or spread values to make it pass. Log the numbers and
the bench set-up and stop: the owner rule is cost and availability before stats, and the orchestrator decides.
