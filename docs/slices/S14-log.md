# S14 log (assault kit: flame, trench clearing, melee; at most 150 lines)
Spec: `docs/slices/S14-assault-kit.md`; stage `S14a-build-A.md`; targets `targets/S14.toml`; ablations `targets/S14-ablations/`.
Branch `claude/project-thread-aj1tww`, base `<BASE>` = `a27ce63` (main: S13 and B3 merged; SAVE 32, PROTOCOL 16).
Commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`. Do not push.
Owner rule 2026-11-25: no balance runs, no tuning; every number a TOML key with a range, behind switches that stay off.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: S14a Build A done.** SAVE 33 / PROTOCOL 17. **T185 passes:** `target/s14-base40` (copied from `target/b3-final/skirmish40.json`,
made on f4e610b whose sim, sdk and mods equal a27ce63) vs `target/s14-A-40` at 768b51a: `neutral.py` "same 40/40". (Old note:
target/s14-base40 target/s14-A-40` (neutral.py now accepts `s5a.fired` 11 -> 12 and `kills_by` 12 -> 14 with zero tails; the new
`s14` blocks and FLAME/MELEE keys of `downed_by`/`finished_by` are new zero keys). Not run: the session's classifier refused batch.sh.
**Code map.** `crates/sim/src/flame.rs`: layout, load checks (`validate_s14`, `close_ok`, `validate_s14_terrain` after the dug
layer), `spawn_flame` (F1), `flame_stage` (F2-F5; `world.rs` after Ballistics when `s14_live()`), `step_out_tick` (F6; Status stage
after `wounds_tick`, uses `send_man`). F1 fire: `combat.rs` `aim_and_fire`, `fire_round` (`FireRequest::flame`); `spawn_rounds`
skips flame. F3/F5 damage: `Blow::Burn` in `apply_blows`. F7: no code (an explosives-kind cartridge feed; S5a base refill). F8:
`base_class` Flame -> `kill_class::FLAME`. Snapshot: `RenderSnapshot::{flame, patches, soldier_burning}`. Rows: `cli/fortify/s14.rs`
(R1 `slit`, R2 `pool`). JSON: `s14` block per faction and per match side (`json_out.rs` `s14_blocks`, `skirmish.rs` `S14Player`).
**What Build B must know.** `ClearTrench`, `Strike` are refused (`NotAllowed`); `MatchState::clears` has load checks. Tests and rows
use test-made flame defs (`s14_tests.rs` `make_flamer`, `fortify/s14.rs` `make_flamer`, fuel `fuel_cartridge`); replace with the
mod's flamethrowers and fuel cartridges. The `flame` capability is not derived yet (plan 4.2). F7 rate: the S5a base serves a
non-small-arms feed at `max(base_bomb_s * hz, 1)`; with `base_bomb_s` 0 that is one fuel unit a tick (a 60-unit tank in 2 s), not
the plan's one per 2 s: owner question (a key would be needed; not tuned here). R1 reads out 4 % (fails): the five in the pit pin and
kill the test flame man in about 4 s (41 particles in 10 runs, 1 hit); R2 passes (drop_diff 0, pool_diff_ticks 0, over_cap 0).
**Deviations.** (1) No 40 sets: T185 unproven. (2) Steps 4-6 share one commit (3c55b01); F6 and the slit test came later (4d3b6c1).
(3) F2 flies each particle through `combat::fly` one substep at a time. (4) Burn and patch pulses are emitted in the flame stage and
applied in Damage as `Blow::Burn`, not in Status. (5) A box-top landing makes no patch; `flame_in_bunker_hits` counts a hit whose
man has a static box over his head within its reach. (6) Particle hits keep the W1 zone of the hit point; burns use `ZONE_NONE`.
(7) `S14Counters::patches_dropped` added. (8) Layout commit missed save_fuzz's version pin (fixed in 3c55b01). (9) simbench
holds no setup baseline. (10) Bridge maps Flame to the MG cue. (11) `net_s12_commands_in_limits` no longer pins the version.
(12) Slit test throws particles down through the slit (the pit men's heads are under the slit's bottom; level particles pass
over them). (13) R1 moves the owner's other men 300 m off (bunker row: 60 m) so only the five can answer the flame man; R2 cuts
`projectile_cap` to 256 in the row, saturates it with 40 staged rounds a tick, and counts refused staged rounds as drops; its flame
men aim 20 m aside (plan 50 m: past the test flamer's 30 m). (14) The thread-count test runs the R1 slit scene (CLI), not the sim.
(15) The snapshot fields have no bridge keys yet (the bridge is not this stage's file); `match_state.md` "S14" names planned keys.

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
