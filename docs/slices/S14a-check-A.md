# S14a Build A checker: misses (read-only review of 747a76b..HEAD)

Scope read: flame.rs, s14_tests.rs, all other crates/sim diffs, content/tuning/setup/variant diffs, protocol, json_out/skirmish, fortify/s14.rs, tools. No build run.

## Neutrality with every S14 switch at 0 (check 2): no code-path leak found
Traced each hunk. With `[flame] enabled` 0 and no def using `class = "flame"` or `when`:
- `aim_and_fire` guard only fires for `WeaponClass::Flame`; `Mg | Flame` search is the old MG branch for MG.
- `spawn_rounds` filter `!r.flame` is a no-op; `spawn_flame` returns at `any(r.flame)` with no state touched.
- `flame_stage` runs only if `s14_live()` (switch on, or a particle or patch exists); `step_out_tick` returns on `enabled == 0`.
- `Blow::Burn` is produced only by `flame_stage`. Snapshot additions are read-only. Member `when` filter clones only when some member has `when`.
- No RNG draw added (flame uses the shot's AIM stream via the existing `fire_round`).
- Hashes and the setup bytes change (new fields hashed), outcomes should not. `kill_class::COUNT` 12 -> 14 and `fired_class::COUNT` 11 -> 12 add zero tails only.
So the code reads neutral, but it is NOT PROVEN (item 1).

## Findings

1. **HIGH, process: T185 neutral proof not run** (S14-log.md "For the next stage"; brief steps 1 and 9). No `target/s14-base40`, no `target/s14-A-40`, no `neutral.py` diff. The most important exit gate of the stage is open. The code reading above is clean, but the brief says any difference is a bug and must be bisected; none was measured. `neutral.py` also now accepts a zero-tail `fired` list anywhere in the tree (tools/neutral.py GROWN_LISTS), wider than `s5a.fired`; low risk.

2. **HIGH, rule F3: a downed man does not keep burning.** `combat.rs` `down()` (~line 4280) rebuilds `*c = Combat { hp:1, ..., wound, ..Combat::default() }`, which resets `close` (burn_until, burn_by) to default. Plan F3: "a downed man keeps burning" and the next pulse kills him. In the code a man floored by a flame hit or burn pulse stops burning on the spot (snapshot `soldier_burning` goes false too); only a patch under him (`flame_stage` landing at his feet) can still finish him. Fix: carry `close.burn_until/burn_by` through `down()` (and `close_ok` already allows Downed). No test covers it: `flame_hits_burn_and_credit_flame` only asserts some men go out to class FLAME.

3. **HIGH, exit: R1 slit row (hard) fails, 4 % vs >= 80 %** (log, `cli/fortify/s14.rs`). Plan 9 R1 and T186 are hard rows and RE done-when 1 ("the bunker falls by geometry") is not demonstrated end to end. Log deviation 12 admits level particles pass over men whose heads are under the slit bottom; the unit test only passes because it throws particles down through the slit (-5 m/s z). Either the test flamer/aim (fire_round gives a flat shot at slit height) or the row geometry is wrong; needs the owner's call or a fix before Build B builds on it. Also the row is not in the `checks` list of `json_out::fortify` (only the `s14` object is inserted), so a failing hard row does not raise `hard_failed`: confirm that is intended.

4. **MEDIUM, F7 rate with no key.** Fuel resupply at base is one unit per tick when `base_bomb_s` is 0 (log; test `flame_fuel_resupplies_as_explosives` documents it). Plan F7 and design 6.5 say one per 2 s. This breaks the owner rule "every number a TOML key" for a later balance pass, and the brief deviation is only reported as an owner question. A key (or a rate for fuel in S5a refill) is needed before Build B ships fuel cartridges.

5. **MEDIUM, F2: box-top landing makes no patch** (log deviation 5). Plan F2 lists "a ground or box-top landing (F5 patch at the point)". flame.rs line ~456 only patches on `ProjectileEnd::GroundHit`. Particles landing on a bunker roof or sandbag top do nothing. Real deviation from the plan, chosen by the builder, not flagged for the owner.

6. **MEDIUM, F6: no walkability test** (`flame.rs` `step_out_tick`, lines ~653-665). Plan: first of 8 points "not in a patch and is walkable". Code checks only on-map and not-in-patch (`terrain.height_at`). A man can be sent into water, a wall, a bunker box or a minefield cell; path request then does whatever `request_unit_path` does. Also `step_out_tick` pulls detached-Idle men (medics tending a casualty, a flame man mid-`SuppressUnits` hold) out of a patch; plan says "formation or holding", so arguably intended, but not tested (`moving_man_does_not_step_out` covers only a moving man, no crew/rider/tender case despite the doc comment claiming crew and rider).

7. **MEDIUM, F1: nothing forces a flame weapon to have a feed.** `fire_round` with `feed == None` refills `mag` to `magazine` forever (combat.rs ~990), so a `class = "flame"` def with no cartridge feed has infinite fuel and no resupply, no dry/sidearm path. No def check in content (`defs.rs`, tuning) requires `feed`, `crew = true` or `magazine` for Flame class. Build B can trip on it silently. The sidearm-when-dry rule of F1 has no test either (brief step 4 names crew handover and sidearm; only handover is tested). Test flamers (`make_flamer`) have no feed at all, so the feed path is only exercised in `flame_fuel_resupplies_as_explosives`.

8. **MEDIUM, layout checks: `load_rejects_bad_s14_state` misses cases** (s14_tests.rs 175-290; plan 3.1 "one case per check"). Implemented checks with no test case: `burn_until` beyond `MAX_TICK + burn_ticks`; strike phase on a Downed man; a clear with `step > steps_max`; a clear on a vehicle squad; clear `at` off map; patches over `patches_max`; flame `ttl` above the cap; flame shooter id out of range; flame `pos.z`/`vel` unbounded (not checked in code; pool projectiles are not either, so consistent, but a huge `vel` is fed to `fly`). Also `s14_layout_roundtrips_and_hashes` has no edit for patch `pos`, clear `at/cell/from/phase`, flame `shooter/weapon/player` (all go through postcard so probably hashed, but not shown).

9. **LOW, F1 counters: capped requests count in `fired[FLAME]`.** `world.rs` calls `count_fired(fire_merged)` before `spawn_flame`, so a request dropped for the cap is counted in `ammo.fired[FLAME]` but not in `flame_fired`; the test only compares them uncapped. Fuel is spent for a dropped request too (`fire_round` already took it). Probably acceptable, but unrecorded.

10. **LOW, F3 integer truncation.** `burn_dmg = burn_hp_per_s * pulse_ms / 1000` (flame.rs ~531): defaults 15*500/1000 = 7 (not 7.5, so 14 HP/s); `burn_hp_per_s` 1 with 500 ms gives 0. A later balance pass lowering the key gets a step function. Same formula as the plan, but undermines "every number a lever". Suggest a remainder accumulator or damage per pulse as the key.

11. **LOW, F5/F3 bookkeeping.** `patch_hits` is bumped for every man in the patch including the owner's (flame.rs ~571); patch expiry and the burn clear happen in the flame stage, not Status (deviation 4 mentions only pulses; patch expiry in Status is plan text). The `flame_in_bunker_hits` heuristic (any static box top >= head-0.5 m within `reach`) counts any roof, wire frame or box, not only a bunker (deviation 5).

12. **LOW, tests named but weak.**
    - `flame_hits_burn_and_credit_flame`: comment says "the enemy holds fire" but the closure is `let _ = s;` (the enemy shoots back); asserts only `flame_out > 0`; the doc claims near-miss suppression and F8 credit by `finished_by_class` but neither is asserted. No test for F4 (near-miss suppression by `near_cm`), none for own burns not counting as kills (`patch_burns_any_owner` checks only hp and `patch_hits`), none for the burn pulse timing or downed-man behaviour (finding 2).
    - `flame_never_takes_pool_slots`: one flame man, 20 m aside, rifle fight (no staged mortar shells); plan F9 says two flame men, staged rifle and mortar fire, 50 m aside, "every shell equal". Documented deviation 13 for the distance; the shell part is only in the CLI row R2 (three bombs).
    - `s14_slit_scene_hash_is_thread_count_free` runs the CLI slit scene, not a sim-level save with particles, patches, burning men (deviation 14). Fine for threads, but the sim has no 1/2/3/8-thread test with particles in flight; `save_fuzz` stages particles, a patch and a burning man only (plan also names strike and clear: expected, Builds B and C).

13. **LOW, M3 range.** `Close::strike` is a `u8` tick count; `windup_ms`/`recover_ms` go to 4000 ms. At `TICK_HZ_ALLOWED` rates above about 63 Hz a legal key value overflows 255 ticks. Check the allowed rates before Build C or add a range check.

14. **LOW, godot/bridge.** `weapon_cue` maps Flame to "mg" (fine), the snapshot fields have no bridge keys (deviation 15, stated). `ClearTrench` and `Strike` refusals go through `refuse_fort(..., NotAllowed)`; no test asserts they are refused (only the wire test `net_s14_commands_in_limits`). Add one sim test (`strike_and_clear_are_refused_until_built`) so Builds B and C notice when the refusal is removed.

## Deviation review (check 5)
Reasonable choices: 1 only because of environment (but see finding 1), 2, 3, 4 (pulse applied in Damage), 6, 7, 8, 9, 10, 11, 12, 15. Real defects or plan misses: 5 (finding 5), 13 (R1 moved men 300 m away, so the row measures the flame man against five men only; fine, but see finding 3), and F7 rate (finding 4). Deviation 14 (thread test in CLI) is a weaker proof than the plan's, not a defect.

## Checked and fine
Keys: all four tables, every key of plan 2, defaults, ranges and comments present in `skirmish.toml` and `tuning.rs`; docs row in `tuning.md`; `load_rejects_bad_s14_keys` covers in/out of range and "in the setup bytes". Saved fields: Flame, Patch, Close, Clear, S14Counters, AiCounters additions, kill/fired classes, all hashed (`hash.rs`), validated, roundtripped (`s14_layout_roundtrips_and_hashes`, `load_rejects_bad_s14_state`). SAVE 33 / PROTOCOL 17, relay doc, net test. Serial, ascending-id order in `flame_stage` and `step_out_tick`; no per-chunk shared writes. Pool isolation by construction (`spawn_rounds` skips flame, separate list, cap).
