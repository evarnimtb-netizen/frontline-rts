# S14 log (assault kit: flame, trench clearing, melee; at most 150 lines)
Spec: `docs/slices/S14-assault-kit.md`; stage `S14a-build-A.md`; targets `targets/S14.toml`; ablations `targets/S14-ablations/`.
Branch `claude/project-thread-aj1tww`, base `<BASE>` = `a27ce63` (main: S13 and B3 merged; SAVE 32, PROTOCOL 16).
Commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`. Do not push.
Owner rule 2026-11-25: no balance runs, no tuning; every number a TOML key with a range, behind switches that stay off.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: S14 Fix done; Docs/PR next.** SAVE 33 / PROTOCOL 17. Every S14 switch is 1 in `mods/ww2/defs/rules/skirmish.toml`.
Review findings 1-16 fixed (section "S14 Fix" below); 13a kept as was (owner question S14-10). **T185:** `target/s14-F-off40`
(`s14_off.toml`) vs `target/s14-base40`: `neutral.py` "same 40/40". **Default 40** (`target/s14-F-40`, breakage read only): no
winner 0; length min 10.65 / median 21.13 / max 32.53 min; no panic; Germany 22 of 40 (reported, not judged).
**S14 rules in the default 40 (ger / usa).** Flame jobs 7 / 6 (ends: done 6 / 0, deadline 0 / 2; open at the end 1 / 4, oldest
77 / 133 s, T192 <= 185 passes); flame buys 10 / 9; flame fired 8 / 12, hits 1 / 0 (1 on a trench man, 0 in a bunker); burn kills
1 / 0; patches 1 / 7; stepped out 4 / 0; refuels 0 / 0; AI clear orders 26 / 13, clears done 25 / 7, steps 34 / 3; strikes 0 / 1,
strike hits 0 / 0; capped 0, patches dropped 0.
**Why some rules stay at 0 (read, not tuned).** Refuels: no tank reached the 10 % reserve (20 particles in 40 matches against a
120-unit tank). Strikes fell 49 -> 1: fix 3 makes a dry man in reach reload or draw his sidearm first, and the automatic strike
was nearly always a dry man; the staged R5 row and `automatic_strike_hits_a_crouched_man` show a strike that does fire hits. Bunker
flame hits 0: few jobs reach Burn at a bunker; the slit itself works (R1 below, `ai_flames_the_slit`).
**R1 (slit, 5 seeds x 2 owners).** out 22 %, 0 runs with 4 of 5 out, closed control 8 %, back 0 %, flame man out in 5 of 10 runs;
stays report-only (owner question S14-8): a flat stream passes over men crouched 1.5 m down and ends on the back wall's side.
**For Docs/PR.** `docs/modding/tuning.md` has every new key (`[flame] aim_cm`, `slit_near_cm`, `patches`; `[melee] down_cm`,
`up_cm`, `drop_cm` gone, `start_pad_cm` 0; `[ai.flame] memory_s`, `burn_min_s`, `clear_max_s`, `stand_trench_m`): their defaults
were chosen so each rule fires, not tuned. `godot/docs/match_state.md` S14 section is current (V rule, stream 512). Owner questions
S14-1..13 in `docs/TODO.md` (8-13 new, each on (A)). Godot `test_match_view` 456/456. The PR text should list the 16 findings.

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

## S14 presenter (P)
Bridge: snapshot fields `flame[]`, `patches[]` (with `radius`), `soldiers[].burning` / `.strike`, `rules.s14 {flame, melee, clear}`;
intent `clear_trench {squads, target}` -> `ClearTrench` for the first selected infantry squad (`intents::clear_trench_command`);
`strike_units` was already mapped. Tests: `clear_trench_command` in `fortify_commands...` intent test, `frame_carries_the_s14_fields_quiet_when_off`.
Godot: `match/flame_view.gd` (stream, patch discs + lights, burning-man flame, strike bar), Clear (tool) and Strike buttons in the
fire strip's free columns (shown with their switch on), `V` strikes when melee is on and men are selected (else Board, as before);
mock `stage_s14` shows one of each; `test_match_view.gd` `_test_s14`. Deviations: (1) `V` is Board's key, so with melee on it
strikes and Board stays on its button; (2) no tint for burning men (instanced bodies, no per-man colour from GDScript): a flame
marker instead; (3) no rig access, so the strike pose is a bar over the striker (raised, then thrust toward the nearest enemy);
(4) the mock does not act on `strike_units` / `clear_trench`; (5) the 16-vehicle 0.2 ms timing check in `test_match_view`
is a headless timing check that failed once under load (0.355 ms), unrelated to S14. No sound for flame shots yet (MG cue).

## S14 Fix (F)
Commits: 13bebba fix 1; 32c9865 sim and AI 2-8, 11, 13b, 15, 16; 4c76482 cli and targets; 3af174f V key and presenter docs 10, 12.
(1) A flame man's `Suppress` aims at the target's height (`flame_aim_z`: the slit's middle near an enemy bunker, else the ground
or dug floor + `aim_cm`); slit test with the real M1A1 man, closed and back controls. (2) AI flame target: in-trench sightings
remembered `memory_s`, bunker by `known_by_record`, Burn at least `burn_min_s`, trench stand `stand_trench_m` 2. (3) Melee: feet
window `down_cm`/`up_cm`, nearest enemy on the body axis, a dry man reloads or draws first. (4) `cleared` kept while the clear runs,
`clear_max_s` stops an old one, a job's squad kept while its clear runs. (5) The clear's flame burst ends after `burst_rounds`.
(6) A clear ends on an undug cell. (7) `[flame] patches` switch; a 0-hp burn spares the downed. (8) Step-out only while holding;
regroup after. (9) T192 gates `flame_job_oldest_s` <= 185; AI tests assert effects; thread test 1/2/3/4/8. (10) `V` strikes only
with an enemy within 1.5 m (S14-9). (11) The entry cell is step 1 (R4 window 120 -> 180 s for it). (12) Docs and names. (13b)
deadline and stall tests for a busy job squad; 13a not changed (S14-10). (14) no code defect. (15) stance Auto after a clear.
(16) load checks: strike point on the map, Flame-class particle weapon, bounded particle state.
Deviation: 13a reverted (counting other squads' tasks ended every lone job at once).
