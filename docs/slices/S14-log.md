# S14 log (assault kit: flame, trench clearing, melee; at most 150 lines)
Spec: `docs/slices/S14-assault-kit.md`; stage `S14a-build-A.md`; targets `targets/S14.toml`; ablations `targets/S14-ablations/`.
Branch `claude/project-thread-aj1tww`, base `<BASE>` = `a27ce63` (main: S13 and B3 merged; SAVE 32, PROTOCOL 16).
Commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`. Do not push.
Owner rule 2026-11-25: no balance runs, no tuning; every number a TOML key with a range, behind switches that stay off.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: S14a Build A partly done** (chunks 0-1 and most of chunk 2; head 3c55b01 plus this log). SAVE 33 / PROTOCOL 17.
**Done:** layout (176525e), keys / when / class / ablations / targets (8aca2e1), F1-F5, F8, F9 with tests (3c55b01).
**Not done (next session, in this order):** (1) T185 and the base 40: both 40 sets were not run (the session's permission
classifier refused `tools/batch.sh`, read as a balance run); run `target/s14-base40` on a27ce63's binary and `target/s14-A-40` on
the head, then `neutral.py`. neutral.py (not an owned file) must accept the grown zero-tailed lists `fired` (11 -> 12),
`kills_by`, `downed_by_class`, `finished_by_class` (12 -> 14): add them to `GROWN_LISTS`. (2) F6 step-out (`[flame] step_out`,
`step_out_m` exist, no code; `stepped_out` counter unused). (3) F7: fuel resupply needs no code if the flame weapon's feed is a
cartridge of supply kind explosives (the S5a path); add `flame_fuel_resupplies_as_explosives` once Build B has the cartridge. (4)
Bunker-slit test (particle through the gap hits a man inside; closed slit: none), pattern `satchel_at_slit_reaches_men_inside`.
(5) Step 7: `sdk/crates/cli/src/fortify/s14.rs` rows R1 (slit) and R2 (pool) with the metric names already in `targets/S14.toml`
(`fortify10.s14.slit.*`, `fortify10.s14.pool.*`). (6) Step 8: snapshot `flame`, `patches`, burning flag; `match_state.md` "S14";
JSON `s14` block per faction (`targets/S14.toml` T195-T200 read `skirmish40.factions.<f>.s14.<counter>`). (7) Thread-count hash
test with `[flame] enabled` 1, save fuzz with particles in flight (staging is in; fuzz green at 3c55b01).
**Code map.** `crates/sim/src/flame.rs`: layout types, load checks (`validate_s14`, `close_ok`, `validate_s14_terrain` called in
`World::load` after the dug layer is derived), `spawn_flame` (F1), `flame_stage` (F2-F5: called in `world.rs` after the Ballistics
block when `s14_live()`). F1 fire: `combat.rs` `aim_and_fire` (class check, MG search) and `fire_round` (`FireRequest::flame`);
`spawn_rounds` skips flame requests. F3/F5 damage: `Blow::Burn` in `apply_blows` (counts `burn_downs`, `burn_kills`). F8: `base_class`
Flame -> `kill_class::FLAME`. Keys: `content/tuning.rs` `FlameTuning`, `MeleeTuning`, `ClearTuning`, `AiFlameTuning`; `when`:
`content/variant.rs` `MemberWhen`, filter in `setup.rs` squad-type build.
**What Build B must know.** `ClearTrench` and `Strike` are refused (`FortRefusal::NotAllowed`) in `game.rs` apply; `MatchState::
clears` exists with load checks (squad, dug cells, step, phase). Tests use test-made flame defs (`s14_tests.rs` `make_flamer`:
the Grenadiergruppe's MG 42 turned into a Flame copy); replace with `fmw41` / `m1a1_flamethrower` when the data lands. The
`flame` capability is not derived yet (plan 4.2), and `modset.rs` derives capabilities from every member, `when` sides included.
**Deviations.** (1) No 40 sets (see (1) above): T185 unproven. (2) Steps 4-6 share one commit (3c55b01); F6, F7 open. (3) F2 flies
each particle through `combat::fly` one substep at a time (not a new integrator): the same tests and tie rules; a ground landing
burns at the middle of the last substep. (4) Burn and patch pulses are emitted in the flame stage (Ballistics, after the particles)
and applied in the Damage stage as `Blow::Burn`, not in the Status stage. (5) A box-top landing makes no patch (only ground and a
struck man's feet); `flame_in_bunker_hits` is not counted yet. (6) Particle hits keep the W1 zone of the hit point (F3 names no
zone; burns use `ZONE_NONE`). (7) `S14Counters::patches_dropped` added (plan 10 T200 reports it). (8) The layout commit missed
save_fuzz's mirror version pin (fixed in 3c55b01). (9) `crates/simbench/src/report.rs` holds no setup baseline: nothing to refresh.
(10) `match_events.rs` (bridge) maps Flame to the MG cue to keep the build green. (11) `net_s12_commands_in_limits` no longer pins
the version; `net_s14_commands_in_limits` pins 17.

## S14a A (Build A)
Commits: 176525e layout; 8aca2e1 keys, when, class, ablations, targets; 3c55b01 F1-F5, F8, F9; then this log.
Tests added: s14_layout_roundtrips_and_hashes, load_rejects_bad_s14_state, load_rejects_bad_s14_keys, net_s14_commands_in_limits,
s14_member_when_pairs_are_checked, flame_fires_particles_only_when_on, flame_cap_counts_capped, flame_hits_burn_and_credit_flame,
flame_handover_keeps_fuel, patches_merge_refresh_and_cap, patch_burns_any_owner, flame_never_takes_pool_slots,
two_flamers_stay_in_cap. `tools/check.sh quick a27ce63` at 3c55b01: fmt ok, every ablate dry run ok (S14 included), tests
1266 passed, 1 failed (`udp::ipv6_loopback_handshake`, no IPv6 in the cloud); clippy failed on a constant assert in
`net_s12_commands_in_limits`, fixed with this log (workspace clippy then clean). Risk 7: the S4 pickup gives the new carrier the dropped weapon's `mag` (`WeaponDrop::mag`): fuel kept.
