# S14 Review findings (branch claude/project-thread-aj1tww at c7e637c, base a27ce63)

Read: plan, briefs A-D and presenter, check-A, the log, the whole diff of `crates/sim`, `crates/net`, `crates/godot-bridge`,
`godot/`, targets, and the fortify row docs. Run: `cargo test -p frontline-sim --lib s14` (58 passed). No match batch run.
Everything below is from reading the code unless it says "test"; nothing was edited.

Summary of the three dead rules (the stage's 40-set facts): all three are **code defects, not tuning**. No value of an existing
key makes them work, so a later balance pass cannot repair them (findings 1-4).

## High

1. **HIGH, rule (R1, A1 Burn at a bunker, RE done-when 1). Flame "at the slit" is aimed at the roof's front face.**
   `crates/sim/src/combat.rs:217-238` (`suppress_aim_at`), `crates/sim/src/ai/flame.rs:275`, `mods/ww2/defs/fortifications/bunker.toml`.
   A `SuppressUnits` line is 2D; its aim height is `ground + [combat.suppress] aim_cm` = base + 0.8 m. The bunker's timbers are
   0.4 m high from the base and the roof box spans 0.4-1.2 m, so the slit is the band 0-0.4 m (0.79 m wide) and the aim point
   (roof centre + facing x 2.1 m, z 0.8 m) is the middle of the roof's front face. A box side ends a particle with no effect
   (S14-6). Only the low tail of the 60 mrad spread enters. Failure: R1 `out_pct` 4 % against 80 %; 0 bunker hits in 40 matches;
   the AI's Burn at a bunker cannot work whatever `stand_m` or damage is. Plan 7 says "at slit height, from the fort def", which
   a 2D line cannot carry. The unit test `flame_through_the_slit_hits_men_inside` passes only because it throws particles down
   through the gap by hand. Fix (code, now): a flame man's suppress aim takes its height from the target (a point inside an enemy
   bunker footprint: the slit centre from the fort def, here base + 0.2 m; else a `[flame] aim_cm` key instead of the MG's 80).
   Then add a test that fires the real weapon by `SuppressUnits` and asserts `flame_in_bunker_hits > 0`. Separate geometry leak
   named by Build C (particles pass under the front timbers where the pit lip falls away; closed control 18 % > 10 %) is an S10b
   bunker defect to fix with it.

2. **HIGH, AI rule (A1). The job's target and its Burn end read only enemies seen this tick.**
   `crates/sim/src/ai/flame.rs:246-284` (`flame_target`), `:559-561` and `:631` (Burn end), `crates/sim/src/ai.rs:1643` (`known_enemies` =
   `seen_by` bit now). (a) A job, a buy (A3) and an A4 clear all need a currently seen enemy with `in_trench`; a trench man is seen
   only while he is up to fire, a bunker's men almost never (`BUNKER_MANNED_M`). (b) Burn ends at the first plan with no seen enemy
   within 6 m of the target: the job's own base of fire (`flame_focus`) makes them duck, so the job goes to Clear or ends "done"
   after at most one plan interval, usually with no particle fired. Failure: 6 jobs and 19 particles in 40 matches, refuels 0 as a
   consequence. This is a mechanism, not a number: no `[ai.flame]` key changes it. Fix (code, now): target = a dug cell where an
   enemy was seen within the last N s (keep a small dated list in `AiState::flame`, it is already a saved field), or an enemy
   bunker piece near the point regardless of sighting; Burn runs at least one full burst (or `burn_min_s`, a key) before the
   "nobody left" test, and that test uses the same dated memory.

3. **HIGH, rule (M1/M2). The strike cannot hit a crouched, prone or trench man; 49 strikes, 0 hits.**
   `crates/sim/src/flame.rs:1343-1420` (`resolve_strikes`, chest at `:1368`), `:1162` (`start_reach`), `:1245`.
   Mechanism, three parts: (a) the segment is level at `fire_height - 0.4 m`: a standing striker's is at 1.35 m, a crouched
   target's capsule axis tops at 0.95 m with radius 0.25 (miss by 0.15 m), a prone man's at 0.2 m; a man on the lip is 1.5 m
   above a man in the trench. Any man near an enemy is suppressed, so he is crouched or prone: the plan's own test
   `drop_cm_lowers_the_strike` asserts the miss at the default. (b) A strike starts within `reach + start_pad_cm + radius` but
   reaches only `reach + radius`: a start in the outer 30 cm shell (where an approaching enemy first enters) misses a man who
   stands still. (c) `strike_at` is the target's position at the wind-up; a walking man is gone in 0.5 s. `[melee] drop_cm` is
   not a repair: no single value hits a crouched man from standing and a standing man from a trench. Side effect: a dry man
   (`fed && mag == 0`, `:1245`) with an enemy in start range never reaches `reload_dry` (the strike takes his turn first), so he
   never draws his sidearm and strikes without end; a dry flame man likewise (against F1). Fix (code, now; no layout change):
   resolve the strike as a horizontal test of the segment against the capsule axis plus a vertical overlap window (keys
   `[melee] up_cm` / `down_cm`), or aim the far end at the body of the enemy nearest `strike_at`; start only within
   `reach + radius` (pad 0 by default or drop the pad from the start test); let a dry man reload or draw his sidearm during the
   recovery. Test: an automatic strike in a live scene against a suppressed (crouched) man hits.

4. **HIGH, AI liveness / orders that fight each other (A4). The AI ends its own clears after `recheck_s`.**
   `crates/sim/src/ai/flame.rs:443-456`. `AiFlameState::cleared` is both the "once per `recheck_s`" timer and the list that keeps a
   clearing squad out of `mine`. The entry is dropped after `recheck_s` (60 s) even while `g.clears` still has the squad; the
   squad returns to the assignment, gets an Attack or Capture order, and `clear_ended_by` ends the clear (not counted done). A
   clear needs approach + up to 6 x (fuse + `settle_s` + up to `patch_s` + the walk), far over 60 s. Failure: 24 AI clear orders,
   7 done, 9 steps in 40 matches. The same happens to a clear left running when its flame job ends on deadline or stall. Fix:
   keep the entry (and the squad out of `mine`) while `g.clears` has it, with its own deadline key so it cannot hang (phases 0
   and 2 of `clear_tick` have no timeout: an AttackMove that halts to fight stays `Moving`, `flame.rs:968-990`).
   `ai_clear_orders_near_a_trench` only asserts the order is given once; no test lets an AI clear run to its end.

## Medium

5. **MEDIUM, rule (C3). The clearing squad's flame man empties his tank on the first step, and every step waits the full cap.**
   `crates/sim/src/flame.rs:1071-1087` (`clear_throw`), `:998-1003`. Plan C3: "fires one burst". Code: a detached
   `UnitOrder::Suppress` with no end; he fires until the advance. His fire keeps refreshing own patches within 2 m of the point,
   so `burning` stays true until the cap `wait + patch_s` (about 22 s): 120 units are gone at step 1 and the squad always waits
   the maximum. `clear_waits_for_own_fire` asserts only "fired > 0" and "waited longer". Fix: end his Suppress after
   `burst_rounds` shots (or `burst_rounds x interval` ticks from `phase_at`, stateless); test: fuel used per step <= one burst.

6. **MEDIUM, save/load. A save the sim wrote can be refused: a clear whose cell is no longer dug.**
   `crates/sim/src/flame.rs:326-341` (`validate_s14_terrain`), `crates/sim/src/terrain.rs:873` (`crater_cut`). Load requires
   `Clear::cell` and `Clear::from` to be dug >= `TRENCH_NAV_DM` (10 dm). A satchel, Geballte Ladung or bangalore crater levels
   trench to 0.9 m (`crater_depth_cm` 90), and those are thrown into exactly these trenches; `clear_tick` never re-checks its
   cells, and `from` is kept for the whole clear. Scenario: a short trench end or two charges level the entry cell while the
   clear runs; a save (or a rejoin snapshot) from then until the clear ends fails with "a clear cell is not dug". Fix: `clear_tick`
   ends the clear when either cell is no longer dug (first thing in the loop), and a test stages it; or drop the terrain check.

7. **MEDIUM, rule and ablation. A zero-HP pulse still goes through `hurt`; `no_patches` does not turn patches off.**
   `crates/sim/src/flame.rs:580-600`, `crates/sim/src/combat.rs:3553-3586`, `crates/sim/src/wounds.rs:156-167`,
   `docs/slices/targets/S14-ablations/no_patches.toml`. (a) `per_pulse` can be 0 (`patch_hp_per_s` 0, or a small `burn_hp_per_s`:
   1 HP/s at 500 ms deals 0, 1, 0, 1). `wounds::deal` with 0 damage kills a downed man (W3), resets `bandaging` and overwrites
   `wound.by`. (b) A patch also sets every man in it burning, so with `patch_hp_per_s` 0 he still takes `burn_hp_per_s` for
   `burn_s`, refreshed every pulse: the `no_patches` ablation measures nearly the full patch effect. The owner's rule (every part
   behind a switch so a later pass can fix it) is not met for patches. Fix: skip `hurt` when `dmg == 0` (suppression only), add a
   real switch (`[flame] patches` 0/1, or allow `patches_max` 0) and point `no_patches.toml` at it.

8. **MEDIUM, rule (F6). Step-out strands men.** `crates/sim/src/flame.rs:678-740` (`:695`). Any formation member with feet in a patch is
   detached, also in a squad that is walking (Move, Retreat, a clear's advance): he walks 2 m aside, goes idle and is left
   behind until the squad's next order. A holding squad's men never return to the trench after the patch is out (the AI's
   Regroup rule, `ai.rs:3334-3345`, covers moving squads only, and there it pulls them back through the patch each plan). Plan
   conform ("formation or holding") but wrong in play: a player's squad sheds men, AI defenders stand in the open after 15 s of
   fire. Fix: step out only men of a squad that is `Holding`/`Arriving` (walkers leave by themselves), and have the AI Regroup
   a holding squad with detached idle men once no patch is near; test a squad marching across a patch keeps its men.

9. **MEDIUM, tests and gates that prove nothing.**
   - T192 (`docs/slices/targets/S14.toml:5472`, `:5567`; `sdk/crates/cli/src/json_out.rs:1440`): `flame_jobs_open_per_match <= 4`
     is true by construction (`MAX_FLAME_JOBS` 4 per AI). The plan's T192 is "no job older than `job_max_s`". Fix: report the
     oldest open job's age at the match end from `AiState::flame.jobs` (no layout change) and gate on it.
   - R1 (T186a-d) was changed from hard to report by Build B without the owner; with finding 1 fixed it should go back to hard
     or the owner should confirm the report status.
   - `ai_flame_job_runs_its_phases`, `ai_flames_the_slit`, `ai_clears_after_burn` assert the order given (`Suppress` line,
     `ClearTrench` present), never an effect (a particle fired, a man burnt, a clear done); the defenders are unarmed and never
     duck, so the tests miss finding 2. `flame_through_the_slit_hits_men_inside` hand-throws particles (finding 1). No melee test
     has an unstaged strike hit a man (finding 3). `flame_ai_hash_is_thread_count_free` covers 1 vs 4 threads only (plan 8:
     1, 2, 3, 8).

10. **MEDIUM, presenter. `V` no longer boards.** `godot/match/selection_controller.gd:1765`. With `rules.s14.melee` on (now the shipped
    default) and men selected, `V` is always Strike; Board needs men selected, so its hotkey is dead. The plan chose `V` without
    seeing the clash. Fix: another free key for Strike, or strike on `V` only when an enemy is within about 1.5 m of a selected
    man (else Board). Needs the owner's key choice.

11. **MEDIUM, AI rule (A4 with C2/C3). The clear's first grenades skip the cell the enemy stands in.**
    `crates/sim/src/ai/flame.rs:758-770`, `crates/sim/src/flame.rs:964-985`. A4 orders `ClearTrench` at the seen enemy's position, so
    his cell is the entry cell. The squad AttackMoves to 6 m short of him, then the first step point is `step_m` (8 m) farther
    along the trench. Fix: step 0 throws into the entry cell before the walk (or A4 aims the entry one step before him).

## Low

12. **LOW, docs/presenter vs `godot/docs/match_state.md`.** `:1650` still says "all 0 in the shipped rules" (they are 1 since 0fc8d39;
    also the header comment of `flame_view.gd` and the bridge test name `..._quiet_when_off`). The stream MultiMesh holds 256
    particles, the rules allow 256 per player (512 in a 1v1): the overflow is not drawn. Burning downed men have no flag
    (`soldier_burning` is per soldier only). The bridge takes the first infantry squad, the GDScript comment says "first able".
    No other key mismatch found between `match_node.rs` and the doc (`flame[]`, `patches[]` with `seconds_left` and `radius`,
    `soldiers[].burning`, `.strike`, `rules.s14`, both intents).
13. **LOW, AI.** `ai/flame.rs:543`: the "point no longer attacked" end can never fire, the job's own Attack task counts in
    `point_attacked`; a lone flame squad goes on for up to `job_max_s` after the attack is called off. `:459`: a busy job squad
    skips the deadline and stall tests for that plan (bounded by the reaction delay).
14. **LOW, tuning matters (no code defect).** Refuels 0 follows from finding 2. A full tank takes 240 s at base (120 units at
    `[flame] base_refill_ms` 2000): a key. German flame fired 0 against 19: no data defect found (both cards resolve their
    `flame_on` men, the fortify rows fire with both owners); the 30 m range against 35 m and two jobs that never reached Burn
    explain it, on numbers too small to mean anything.
15. **LOW.** `flame.rs:877`: `ClearTrench` sets Crouch on the men and nothing sets it back when the clear ends (the AI resets
    stances elsewhere; a player's squad stays crouched).
16. **LOW, load validation.** `validate_s14` does not require `Flame::weapon` to be a Flame-class weapon, nor bound `vel`, `pos.z`
    or `Close::strike_at`. Harmless as far as read (`Fx` saturates; other blow kinds are dropped in `flame_stage`), but untested;
    checker-A item 8's missing cases (`step > steps_max`, vehicle squad, patches over the cap, ttl over the cap) should be
    confirmed in `load_rejects_bad_s14_state_more`.

## Determinism, layout, protocol: no defect found
Checked: particles, patches, burn pulses, step-out, clears and strike resolution are serial in ascending id; `strike_tick` writes
only the man's own `Combat::close` inside his chunk (`changed` is set for every soldier, so the chunk hash is marked) and reads
only immutable columns and the movement hash; strike requests merge in chunk order = id order and are resolved in the same
tick (the buffer is not state); no new RNG purpose (flame uses AIM/SUPPRESS/SWEEP as an MG does). Every new field is saved and
hashed: `Combat::close` (hash.rs:143), `flame`, `next_flame`, `patches`, `next_patch`, `clears`, `PlayerState::s14`,
`AiState::flame` and the counters (postcard). `kill` and `despawn` reset `close`; `down()` keeps the burn and drops the strike.
SAVE 33 / PROTOCOL 17 with one bump each; `Strike` is bounded to 1..=MAX_UNITS on the wire, `ClearTrench.at` is clamped by the
sim. The only save defect is finding 6.

## The stage's open owner questions
- **Saved field `AiState::flame`:** keep. It is inside SAVE 33, which is not on main; it is saved, hashed, validated and
  fuzzed. No code fix, no owner decision needed; say it in the docs stage (plan 3.1 said S14-ai adds none). Finding 2 and 4
  want two more small things in the same struct, before the merge to main.
- **Strike geometry:** code fix now (finding 3). `drop_cm` cannot repair it later.
- **Flame target rule:** code fix now (finding 2); the widths and times stay keys. Tell the owner, it is not a choice of numbers.
- **R1 slit geometry:** the aim height and the gap under the timbers are code fixes now (finding 1). Whether R1 is hard again,
  and "five alert defenders kill the lone flame man", are for the owner (balance).
- **Owner questions that remain:** the `V` key (finding 10); R1 hard or report (finding 9).
