# S14 log (assault kit: flame, trench clearing, melee; at most 150 lines)
Spec: `docs/slices/S14-assault-kit.md`; stage `S14a-build-A.md`; targets `targets/S14.toml`; ablations `targets/S14-ablations/`.
Branch `claude/project-thread-aj1tww`, base `<BASE>` = `a27ce63` (main: S13 and B3 merged; SAVE 32, PROTOCOL 16).
Commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`. Do not push.
Owner rule 2026-11-25: no balance runs, no tuning; every number a TOML key with a range, behind switches that stay off.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: S14a Build B done; Build C (melee M1-M5, `Strike`, R5) next.** SAVE 33 / PROTOCOL 17 (unchanged by B). **T185 passes
again:** `target/s14-B-40` (batch at 16b2178) vs `target/s14-base40`: `neutral.py` "same 40/40". Switches stay off.
**Code map (B).** `flame.rs` C1-C5 (after the F-sections): `order_clear` (C1; `game.rs` dispatches `ClearTrench`), `clear_tick`
(C2-C5; `world.rs` Status stage after `step_out_tick`), `next_step` (BFS on dug 4 m cells), `clear_ended_by` (a new unit order
ends it; `apply` calls it on accepted commands), `walkable` (F6). `supply.rs` fuel refill at `[flame] base_refill_ms`. Data:
`mods/ww2/defs/weapons/{fmw41,m1a1_flamethrower,m2_2_flamethrower}.toml`, cartridges `{fmw41,m1a1}_fuel`; `when` pairs in
`ger_sturmpioniergruppe`, `usa_assault_section`; derived capability `flame` (`Capability::Flame` 15). Rows: `cli/fortify/s14.rs`
`flame_world` (defs resolve with `[flame] enabled` 1, `skirmish::match_setup_with`), R1 `slit`, R2 `pool`, R3 `trench`, R4
`clear`, R6 `under_fire`; `json_out::s14_checks` (R2 hard check; R1 only while `SLIT_HARD`). Targets T186 report, T201-T203 new.
**What Build C must know.** `Strike` is still refused (`NotAllowed`); `Close::strike` has load checks; the strike tick fits a
u8 at 20/30 Hz. `save_fuzz` staging has a squad mid-clear, not yet a man mid-strike. The sim tests use the mod's flamers
(`s14_tests.rs` `real_flame_world`, `sturm`). R5 (T188) still has no scene.
**Rows (5 seeds x both owners, at 0c8f742).** R1 report only: out 34 %, runs_4_out 2 of 10, flame man out in 5 of 10, closed
18 %, back 0 (fails). Two causes: the five kill the lone flame man in half the runs (no tuning: later balance pass), and
particles pass under the front timbers' bottom (z base-0.2 m) where the pit lip falls away, into the pit, and leave a patch
on its floor (S10b bunker/pit geometry, also the closed control's 18 %; not changed here: owner question). R2 passes (drop_diff
0, pool_diff_ticks 0, over_cap 0, 1210 fired). R3: attackers lost 100 % with and without flame, none reached the trench, the
flame man never fired (he walks last; the 60 m walk kills the card). R4: 49 steps, 10 clears done, defenders out 100 %,
attackers lost 21 % (AttackMove control: 0 steps, 100 %, 7 %). R6: out 0 %, stepped out 0, 70 fired, the flame man out in 10
of 10 (the MG at 150 m does not keep the trench down). All reports; no tuning.
**Deviations (B).** (1) M2-2 uses `m1a1_fuel` (a third cartridge for 135 g not made). (2) Fuel cartridges carry `caliber_mm`
1 (none). (3) A flamethrower is never a squad's main weapon (setup and the launcher-main check skip Flame). (4) Own burns have
no counter (no spare saved field); an own burn kill is own fire (`ff_dead`). (5) Pulse HP spread statelessly by pulse index.
(6) The fuel refill follows the S5a bombs-wait brake. (7) C2 "fires one burst" is a `Suppress` at the step point until the
advance. (8) Crouch stays after a clear. (9) C3's frontier: cells at walk >= ceil(step_m/4) cells, farthest from the entry.
(10) The fuse wait uses the card's frag item. (11) `walkable` = slope <= `max_slope_pct` and no static box at 0.1/0.5 m.
(12) The setup bytes baseline moved for the new key only. (13) R1 report only (above); R3 walks straight in (no grenades).
(14) R2 takes one flame man from each of two assault cards. (15) Report rows T201-T203 are new ids.

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
