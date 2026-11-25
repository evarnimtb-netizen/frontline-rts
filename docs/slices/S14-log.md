# S14 log (assault kit: flame, trench clearing, melee; at most 150 lines)
Spec: `docs/slices/S14-assault-kit.md`; stage `S14a-build-A.md`; targets `targets/S14.toml`; ablations `targets/S14-ablations/`.
Branch `claude/project-thread-aj1tww`, base `<BASE>` = `a27ce63` (main: S13 and B3 merged; SAVE 32, PROTOCOL 16).
Commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`. Do not push.
Owner rule 2026-11-25: no balance runs, no tuning; every number a TOML key with a range, behind switches that stay off.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: S14-ai Build D done; Review next.** SAVE 33 / PROTOCOL 17. Every S14 switch is 1 in `mods/ww2/defs/rules/skirmish.toml`
(`[flame]`, `[melee]`, `[clear] enabled`; `[ai.flame] jobs/refuel/buy/clear`). **T185:** `target/s14-D-off40` (`s14_off.toml`) vs
`target/s14-base40`: `neutral.py` "same 40/40". **Default 40** (`target/s14-D-40`, breakage read only): no winner 0; length min
10.65 / median 20.43 / max 31.31 min; no panic; Germany 22 of 40 (reported, not judged). **Code map.** `crates/sim/src/ai/flame.rs`:
`AiFlameState` (`AiState::flame`: `jobs` by squad, `failed` targets, `cleared` squads), A1 `flame_start` / `flame_continue` /
`run_flame` (phases Close, Smoke, Go, Burn, Clear; ends `END_*` = `FLAME_END_NAMES`), `flame_target` (bunker slit = roof centre +
facing x roof half depth; else the known enemy in a dug cell nearest the point), `flame_focus` (the base of fire's aim,
`ai/assault.rs` beside `breach_focus`), A2 `tank_pct`/`tank_low` (hook in `ai/ammo.rs` refills), A3 `flame_card` (urgent buy after
the breach card in `ai.rs` `buy`), A4 `flame_clears`; `ai.rs` `throws` skips a squad clearing or with a `ClearTrench` pending (its
`Regroup` ended the clear). JSON `factions.<f>.s14_derived` (`json_out::s14_derived`); targets T189-T200 (`targets/S14.toml`: T190
no winner / length 10-40 and T192 `flame_jobs_open_per_match` <= 4 hard, the rest reports); `tools/pool.py` S14 line.
**S14 rules in the default 40 (ger / usa, totals).** Flame jobs 2 / 4 (ends: usa done 1, deadline 1, weak 1; 2 + 1 open at the end);
flame buys 8 / 4; refuels 0 / 0; AI clear orders 15 / 9, clears done 3 / 4, clear steps 6 / 3; strikes 24 / 25, strike hits 0 / 0;
flame fired 0 / 19, flame hits 0 / 1, in a trench or pit 0 / 0 (`flame_matches_pct` 0); burn downs / kills 0 / 2, 0 / 3; patches
0 / 19; stepped out 6 / 0; capped 0, patches dropped 0.
**Open owner questions.** (1) Layout: the plan said S14-ai adds no saved field, but A1's deadline, stall and L3 recheck need job
state; Build D added `AiState::flame` (one field, inside SAVE 33, which is not on main yet; no SAVE bump). Alternative: a stateless
job read from the men's orders, without L1. (2) Few jobs: a man crouched in a trench is seen only when he rises to fire, so A1's
"known enemy in a dug cell" is rare; German flame never fired in 40 matches. Widen the target (a known dug cell or enemy trench
near the point) or leave to the balance pass? (3) A2 never fired: no tank reached the reserve (19 particles in all). Note the
flamethrower is a crew weapon, so S5a's squad share already is the tank's; A2 adds only "at the reserve, even under threat".
(4) Strikes 49, hits 0 (Build C question 4: the level strike passes over a crouched man). (5) R1 geometry and R3/R6 as Build C.
**Deviations (D).** (1) `AiState::flame` (above). (2) Close and Go use `Move`, not `AttackMove` (as the breach stage: an
AttackMove halts at every enemy in sight and the job stalls). (3) A squad already within `stage_m` skips Close; a flame man
within `stand_m` burns from where he is. (4) A3: "no live own flame squad" read as none attacking that point, with `cards_max`
the cap on living flame squads. (5) A4: the *point* within `clear_near_m` of the known trench enemy (the plan's wording), not the
squad. (6) L3 waits are per target (8 m) for A1 and per squad for A4, both `recheck_s`. (7) Smoke waits `[ai.fort]
suppress_lead_s` (the breach rule). (8) Test worlds: `flat_world_with` resolves with flame off and calls `s14_off`;
`real_flame_world` calls `s14_off` then turns flame on (they were written with the switches off).

## S14a A (Build A)
Commits: 176525e layout; 8aca2e1 keys, when, class, ablations, targets; 3c55b01 F1-F5, F8, F9; f22d8a1 log, clippy; 4d3b6c1
F6, bunker hits, slit test; 94d6bdf F7 test; 6155009 rows R1, R2 and the thread-count test; c352c17 snapshot, match_state; e11a95e
JSON `s14`; 1721c7e neutral.py `fired`; then this log.
Tests added: s14_layout_roundtrips_and_hashes, load_rejects_bad_s14_state, load_rejects_bad_s14_keys, net_s14_commands_in_limits,
s14_member_when_pairs_are_checked, flame_fires_particles_only_when_on, flame_cap_counts_capped, flame_hits_burn_and_credit_flame,
flame_handover_keeps_fuel, patches_merge_refresh_and_cap, patch_burns_any_owner, flame_never_takes_pool_slots,
two_flamers_stay_in_cap; later flame_through_the_slit_hits_men_inside, men_step_out_of_burning_ground,
moving_man_does_not_step_out, flame_fuel_resupplies_as_explosives, snapshot_shows_flame_patches_and_burning, fortify_s14_rows_run,
s14_slit_scene_hash_is_thread_count_free, s14_block_names_every_counter, test_fired_grows_with_a_zero_tail. Rows (5 seeds x both
owners): slit out_pct 4.0, runs_4_out 0 of 10, closed 0.0, back 0.0 (fail); pool dropped 39360 = control, drop_diff 0, over_cap 0.
`tools/check.sh quick a27ce63` at 3c55b01: fmt ok, every ablate dry run ok (S14 included), tests
1266 passed, 1 failed (`udp::ipv6_loopback_handshake`, no IPv6 in the cloud); clippy failed on a constant assert in
`net_s12_commands_in_limits`, fixed with this log (workspace clippy then clean). Risk 7: the S4 pickup gives the new carrier the dropped weapon's `mag` (`WeaponDrop::mag`): fuel kept.
`tools/check.sh quick a27ce63` at 1721c7e: fmt ok, every ablate dry run ok (S14 included), tests 1274 passed, 1 failed
(`udp::ipv6_loopback_handshake`, no IPv6 in the cloud, as before); clippy failed on a `clone` of a `Copy` account in
`flame_fuel_resupplies_as_explosives`, fixed with this log (workspace clippy then clean).

## S14a B (Build B)
Commits: 09b5338 checker fixes; 16b2178 flame data, when pairs, derived `flame`; 0053811 ClearTrench C1-C5, save_fuzz mid-clear;
0c8f742 rows R1-R4, R6, checks, targets; then this log. T185: same 40/40 (`target/s14-B-40`).
Tests added: downed_man_keeps_burning, own_patch_burn_is_not_a_kill, near_miss_particle_suppresses, flame_on_a_box_top_makes_a_patch,
flame_fuel_refills_at_base_refill_ms, dry_flamer_draws_his_sidearm, step_out_points_are_walkable, crews_and_riders_do_not_step_out,
capped_flame_is_not_counted_fired, burn_pulses_lose_no_hp, strike_ticks_fit_a_u8, strike_and_clear_are_refused_while_off,
load_rejects_bad_s14_state_more, flame_weapon_needs_a_feed, flame_off_cards_spawn_todays_men, clear_trench_walks_the_trench,
clear_refused_off_trench, clear_ends_on_new_order, clear_waits_for_own_fire, clear_off_is_refused, clear_saves_and_loads_mid_step,
s14_rows_feed_the_hard_checks; rewritten on the mod's data: flame_never_takes_pool_slots, flame_handover_keeps_fuel,
flame_fuel_resupplies_as_explosives, fortify_s14_rows_run.
`tools/check.sh quick a27ce63` at 0c8f742: fmt ok, clippy ok, neutral.py ok, every ablate dry run ok (S14 included), tests 1296
passed, 1 failed (`udp::ipv6_loopback_handshake`, no IPv6 in the cloud, as before).

## S14a C (Build C)
Commits: 6f5d64f weapon `[melee]` block and data; 98cf249 M1-M4, `Strike`, intent, snapshot, M5 tests; 6241ba4 save_fuzz mid-strike;
ae3131b R5 row, checks, `drop_cm`; then this log. T185: same 40/40 (`target/s14-C-40`). R5: 100 strikes ahead hit 100, aside 0.
Tests added: strike_hits_only_a_touched_capsule, target_steps_back_strike_misses, loaded_smg_shoots_not_strikes,
reloading_rifle_strikes, pinned_man_does_not_strike, no_strike_at_downed, two_strikers_resolve_in_id_order, melee_off_is_base,
strike_command_needs_a_man_in_reach, strike_saves_and_loads_mid_windup, melee_scene_hash_is_thread_count_free,
drop_cm_lowers_the_strike, s14_strike_intent_names_its_men, s14_melee_row_hits_only_ahead; save_fuzz stages a man mid-wind-up
and one recovering; s14_rows_feed_the_hard_checks covers R5.
`tools/check.sh quick a27ce63` at ae3131b: fmt ok, neutral.py ok, every ablate dry run ok (S14 included), tests 1310 passed, 1
failed (`udp::ipv6_loopback_handshake`, no IPv6 in the cloud, as before); clippy failed on too many arguments of
`flame::nearest_enemy`, fixed with this log (workspace clippy then clean).

## S14-ai D (Build D)
Commits: dbb1508 A1-A4, liveness, `AiState::flame`, tests, save fuzz; 28fbc6a `s14_derived`, T189-T200, pool.py; 0fc8d39 switches on,
canonical snapshot, S14-off test worlds; 496748d tuning.md; 530ceac check fixes; then this log. 40 sets: `target/s14-D-40` (default),
`target/s14-D-off40` (`s14_off`: neutral.py same 40/40 against `target/s14-base40`).
Tests added (`crates/sim/src/s14_ai_tests.rs`): ai_flame_job_runs_its_phases, flame_job_ends_when_operator_dies,
flame_job_ends_on_deadline, flame_job_ends_on_stall, refuel_when_tank_dry, flame_buy_once_per_cap, ai_flames_the_slit,
ai_clears_after_burn, ai_clear_orders_near_a_trench, flame_ai_hash_is_thread_count_free (1 vs 4 threads, save/load in each phase
reached), flame_ai_state_saves_and_is_checked; `json_out` s14_derived_reads_per_match; save_fuzz stages a flame job in each phase.
`tools/check.sh quick a27ce63` at 530ceac: fmt ok, clippy ok, neutral.py ok, every ablate dry run ok, tests 1322 passed, 1 failed
(`udp::ipv6_loopback_handshake`, no IPv6 in the cloud, as before). Step 5 (rules that never fired: A2 refuel, strike hits, German
flame) not investigated further for lack of room: see the open questions above.
