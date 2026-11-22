# S13-ai Build C brief (AI stage)
Plan: `S13-heavy-wave.md` 5 (H1-H3), 6 ("S13-ai C AI"), 7 (targets, armour-test 41-46), 8 (risks). Hand-over: `S13-log.md`
"For the next stage". Every key and saved field exists since S13a A's layout commit: no SAVE or PROTOCOL bump, no layout change.
Owner rule: features first, no tuning in this stage.

## Rules
- **H1 heavy pick** (`ai.rs` `buy`, `note_sightings`). `armour_seen_at`: tick of the last sighting of an enemy tank, TD or assault
  gun. With `heavy_buy` 1 and a sighting within `heavy_seen_s`, the faction's ready `ai_heavy` card (fewest buys, roster order) is
  the plan: `heavy_plan_since` set (`heavy_plans`), bought when affordable (`heavy_buys`), meanwhile no other vehicle and no
  non-urgent buy (only the urgent breach, builder and AT buys). After `heavy_save_max_s` the plan drops (`heavy_drops`) and a new
  one needs a sighting after the drop. "Ready" is the existing card check (limit place, cap, group, cooldown, unlock).
- **H2 heavy overwatch** (`overwatch.rs`): a tank def with `ai_overwatch` takes the TD/AG overwatch path with `heavy_overwatch` 1.
- **H3 push limit** (`ai/armour.rs` `rearm`): with `push_limit` 1, a vehicle with a main gun holding at most `tank_reserve_rounds`
  main rounds, further than `push_without_supply_m` from the nearest rearm source (base, own HQ or dump or truck with shells),
  starts the same rearm trip (`push_turnbacks`).
- CLI JSON: an `s13` block per faction and per side (the four counters, heavy buys per card); zero with the switches off.

## Data
`skirmish.toml` `heavy_buy`, `heavy_overwatch`, `push_limit` 1; `ai_weight` 100 on the 76 W and M36 cards; Panther, Tiger and
Jumbo stay 0 (H1 path). `special_min_pct` stays 0 unless T156 asks.

## Tests (`crates/sim/src/s13_tests.rs`, each with its control)
`heavy_bought_after_armour_seen`, `heavy_plan_drops_after_save_max`, `no_armour_no_heavy`, `heavy_waits_for_a_limit_place`,
`panther_takes_overwatch`, `jumbo_leads`, `tank_turns_back_at_push_limit`; save fuzz with the new fields in play; threads 1 vs 4.

## Targets and armour-test
`docs/slices/targets/S13.toml`: a copy of `S12.toml` plus the S13 rows. The plan's T150-T159 are taken by S11b's rows (ids must be
unique), so S13 numbers them T174-T183 (plan number + 24; T174a = plan T150a). Armour-test cases 41-46 in
`sdk/crates/cli/src/armour_test/s13.rs` (`scenarios::duel_cards`), each with an `o1_off` control run.

## Checks
40 sets (`BATCH_JOBS=4`): default and `s13_off` (= `target/s13-base40` under `python3 -I tools/neutral.py`); Germany wins,
length, heavy buys per type (T155). `tools/check.sh quick 6f7bda0`. Log "For the next stage" rewritten.
