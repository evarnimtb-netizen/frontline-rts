# S14 log (assault kit: flame, trench clearing, melee; at most 150 lines)
Spec: `docs/slices/S14-assault-kit.md`; stage `S14a-build-A.md`; targets `targets/S14.toml`; ablations `targets/S14-ablations/`.
Branch `claude/project-thread-aj1tww`, base `<BASE>` = `a27ce63` (main: S13 and B3 merged; SAVE 32, PROTOCOL 16).
Commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`. Do not push.
Owner rule 2026-11-25: no balance runs, no tuning; every number a TOML key with a range, behind switches that stay off.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: S14a mechanics done (Builds A-C); presenter P and S14-ai next.** SAVE 33 / PROTOCOL 17. Every S14 switch is 0 in
`mods/ww2/defs/rules/skirmish.toml`. **T185:** `target/s14-C-40` vs `target/s14-base40`, `neutral.py` "same 40/40". R2, R5 hard and
passing; R1 report only. **Code map (`crates/sim/src/flame.rs`).** Layout and load checks (`Flame`, `Patch`, `Close`, `Clear`,
`S14Counters`, `close_ok`, `validate_s14`); F1 `spawn_flame` (from `aim_and_fire` requests); F2-F5 `flame_stage` (Ballistics, serial,
after `fly_thrown`); F6 `walkable`, `step_out_tick` (Status); C1-C5 `order_clear`, `clear_ended_by`, `clear_tick`, `next_step`;
M1-M5 `melee_of` (weapon `[melee]` block else the rules'), `strike_tick` (called from `combat::aim_and_fire`, per chunk, own fields;
wind-up end -> `World::strike_bufs`), `order_strike`, `resolve_strikes` (`apply_blows`, after every blow, ascending striker id, via
`DamageCtx::strikes`). Fuel refill `supply.rs`; flame data `mods/ww2/defs/weapons/{fmw41,m1a1_flamethrower,m2_2_flamethrower}`,
butt strokes on `kar98k`, `m1_garand`, `m1_carbine`, `bar_m1918a2` (100/50/600/1000) and `mp40`, `m3_smg`, `stg44` (80/40/500/1000).
Content: `WeaponDef::melee` / `WeaponSetup::melee` (`MeleeDef`), `MeleeTuning` (+ `drop_cm`). Rows `sdk/crates/cli/src/fortify/s14.rs`
R1 `slit`, R2 `pool`, R3 `trench`, R4 `clear`, R5 `melee`, R6 `under_fire`; `json_out::s14_checks` (R2, R5; R1 while `SLIT_HARD`).
**Presenter needs.** Snapshot (`RenderSnapshot`): `flame[]`, `patches[]`, `soldier_burning[]`, `soldier_strike[]` (0 none, 1 wind-up,
2 recovery: the strike pose). Intents: `strike_units {units}` (bridge -> `Strike`; key `V`); `ClearTrench {squad, at}` has no bridge
intent yet (presenter adds `clear_trench`). Refusals come as `fort_refused` `not_allowed` / `bad_spot`. `godot/docs/match_state.md`
S14 lists the planned state keys; the mock shows none of it.
**What S14-ai calls.** `ClearTrench` (C1 refusals: switch, squad, no frag, `bad_spot`), `SuppressUnits` at a slit or trench cell for a
flame man (F1), `MatchState::flame_men`, `Capability::Flame` (derived). Strikes need no AI (automatic, M2). Switches to turn on:
`[flame] enabled`, `[melee] enabled`, `[clear] enabled`, `[ai.flame] jobs/refuel/buy/clear`. JSON `s14` block and targets T189-T200.
**Open owner questions.** (1) R1 slit geometry: particles pass under the bunker's front timbers where the pit lip falls away (also
the closed control's 18 %). (2) R1 report only: five alert defenders kill the lone flame man (balance pass). (3) R3: the flame man
walks last and never fires; R6: the MG at 150 m does not keep the trench down (reports). (4) Melee: the plan's level strike from the
chest passes over a crouched or prone man, and a struck man crouches (hit suppression), so a second strike at him misses;
`[melee] drop_cm` (default 0, the plan) lowers the far end for a later pass.
**Deviations (C).** (1) New key `[melee] drop_cm` (above). (2) M2 readiness: the first aim at a new target (half a second times
training) is not "not ready" (a loaded SMG shoots); a dry counted weapon is always not ready. (3) Start distance: horizontal, to the
enemy's position, within reach + `start_pad_cm` + body radius; he need not see him. (4) `Strike` asks no readiness or pinned check,
only no strike running and an enemy in reach. (5) `strike_kills` = struck to 0 HP (downed or killed); `strikes` counted at the
resolution. (6) A phase runs on if he is pinned or ducks mid-strike. (7) `Strike` ends a squad's clear (Build B's rule). (8) R5: one
run (no RNG), 50 strikes each way, staged a tick before the wind-up ends, both men restored fresh each strike. (9) `melee_off_is_base`
compares entity stores (setup bytes are in the state hash). (10) Damage values cite the design doc only. Builds A, B: their sections.

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
