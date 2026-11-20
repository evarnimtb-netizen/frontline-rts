# S12b Build C: medic AI (A1-A5, liveness), `s12_derived`, switches on

Read first: `S12-wounds-medics.md` sections 2, 5, 6, 8, 9 (S12b), 11; `S12-log.md` "For the next stage" (S12a code map, the Fix
stage's notes on what the AI must do itself, e.g. releasing men and Regroup) and the gate block below; `S10b-ai.md` liveness L1-L5 as
the pattern; `crates/sim/src/ai/supply.rs` and `ai/arty.rs` for how an AI job module is laid out. **2 chunks**, about 30 turns each.
`<BASE>` is `1209c99`. No layout change (the `AiCounters` medic fields exist since Build A).

Gate (orchestrator, 2026-11-20, at `2c173eb`, `target/s12-wounds-only/measure.txt`): the `wounds_only` 480 (AI blind to medics)
passed. Germany pooled 42.92 % (43.12 / 40.00 / 45.62), no match without a winner, match length median 18.7-19.8 min, min 10.6,
max 37.3. HE share read over kills only rose to 37.3-38.5 % (plan risk 7 / S12-15: from this stage T8 reads
`he_share_casualties_pct`, HE over kills plus downs).

Work rules: as Builds A/B (`tools/run.sh`, no command over 4 minutes, `git commit -m "S12b C: ..." -- FILES`, never `git add -A`,
do not push, no model names in files). 40 sets only through `tools/batch.sh` with `BATCH_JOBS=4`, **at most four**.

## Chunk 1: the AI
1. `crates/sim/src/ai/medic.rs` (plans without RNG, ascending ids): A1 buy, A2 post siting, A3 field team (follow, go to a downed
   own man only with no spotted enemy within `enemy_clear_m`, stabilise, carry), A4 staffing and the retreating squad sending its
   wounded, A5 reinforce. Each behind its `[ai.medic]` switch; counters `medic_buys`, `medic_tends`, `medic_carries`, `post_builds`,
   `reinforce_orders`, `medic_holds`. Named tests per rule.
2. Liveness (plan 5) and the plan-6 AI-side cases as named tests: a job ends when its casualty dies, is evacuated or taken; 120 s
   without progress drops it; no team waits on an empty post; a team whose casualty vanished is released (Regroup) by the AI.

## Chunk 2: numbers and switches
3. JSON `s12_derived` per faction (plan 8 metrics, incl. `he_share_casualties_pct` and `medic_matches_pct`) in
   `sdk/crates/cli/src/json_out.rs`; `targets/S12.toml` rows for them (hard: Germany 35-65 pooled, no match without a winner, length
   per owner rule, `medic_matches_pct` >= 60, T8 on `he_share_casualties_pct` <= 26.3; the rest reports); `tools/pool.py` prints a
   short S12 line if that is the pattern of earlier slices.
4. Switches on in `mods/ww2/defs/rules/skirmish.toml`: `[wounds] enabled` 1, `reinforce` 1, `[ai.medic] buy/field/post/reinforce` 1;
   the medic cards' `ai_weight` as the AI needs (the AI may buy only through A1). Update `targets/S12-ablations/` so `s12_off` still
   turns everything off (it is the neutral control) and the others turn off one part each.
5. 40 sets: default (switches on) and `s12_off` (must equal `target/s12-base40` under `tools/neutral.py`); read Germany wins, length,
   medic buys and `medic_matches_pct`. If a medic rule never fires in matches, find out why before tuning anything (owner rule: features
   first, no Tune).
6. `tools/check.sh quick 1209c99` green (the IPv6 loopback test fails on this machine only). Rewrite the log's hand-over block for the
   orchestrator's 480 and Review: state, code map, numbers, deviations.

Files owned: `crates/sim/src/ai/medic.rs`, `crates/sim/src/ai.rs` (wiring), new `crates/sim/src/s12_ai_tests.rs`,
`sdk/crates/cli/src/json_out.rs`, `sdk/crates/cli/src/skirmish*`, `mods/ww2/defs/` (switch values, `ai_weight`),
`docs/slices/targets/S12*`, `tools/pool.py`, `tools/ablate.sh`, `docs/modding/tuning.md`, `docs/slices/S12-log.md`.
Not `godot/` (the presenter stage runs beside you).
