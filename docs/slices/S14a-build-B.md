# S14a Build B: Build A fixes, Clear trench C1-C5, the data (flamethrowers, fuel, `when` pairs, derived `flame`), rows R1-R4, R6

Read first: `S14-assault-kit.md` (owner-change note at the top; sections 2, 3.2 F3/F5/F7, 4, 6, 9, 10); `S14-log.md` "For the next
stage" (Build A's code map and deviations); the checker's findings `docs/slices/S14a-check-A.md`. `<BASE>` is `a27ce63`. Branch
`claude/project-thread-aj1tww`. **2 chunks**, about 30 self-counted turns each. No layout change: every saved field exists since
Build A's layout commit (if one is truly missing, stop and say so in the log). Every switch stays 0; tests turn switches on in the
test only. With `[flame] enabled` 0 both cards must spawn exactly today's men (the `when = "flame_off"` side).

Owner rule (2026-11-25): **no balance runs, no tuning.** Every new number is a TOML key with a range and a comment. T185 passed at
Build A (`target/s14-base40` vs `target/s14-A-40`, same 40/40); keep it passing (step 9).

Work rules: as Build A (`tools/run.sh` for anything long, no command over 4 minutes, commit after every green step with
`git commit -m "S14a B: ..." -- FILES`, never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`,
`godot/match/fort_view.gd.uid`; do not push; no model names in files or commit text). One 40 set in this stage (step 9), through
`tools/batch.sh` with `BATCH_JOBS=4`; if the session refuses it, stop that step and say so (do not work around it).

## Chunk 1: Build A fixes (checker findings; one commit each or grouped, with tests)
1. F3: a downed man keeps burning (`combat.rs` `down()` must keep `Combat::close`); test `downed_man_keeps_burning` (and the next pulse
   kills him, W3). Own burns are own fire, not kills: test `own_patch_burn_is_not_a_kill`. F4: test `near_miss_particle_suppresses`.
2. F5: a landing on a box top makes a patch at the point (plan F2 end list); test.
3. F7: new key `[flame] base_refill_ms` (default 2000, range 100..=60000, comment) for the base refill of flame fuel, one unit per that
   time (plan 3.2 F7: "one unit per 2 s"); rows in `tuning.md`; test the rate.
4. Content check: a `class = "flame"` weapon must have a feed (load error otherwise); test the dry-then-sidearm rule (F1).
5. F6: step-out points must be walkable (use the nav/walk test the movement code uses); tests that crews and riders do not step out.
6. `load_rejects_bad_s14_state`: add one case for each implemented check the checker listed (burn end beyond max, strike on a downed
   man, clear step over `steps_max`, clear on a vehicle squad, clear `at` off map, patches over cap, ttl over max, shooter out of range).
7. Lows: capped requests not counted in `fired[FLAME]`; burn damage per pulse without truncation loss (carry the remainder or use
   milli-HP: pick the simplest deterministic way and say which); `patch_hits` only enemies (own burns get their own counter if
   needed, as a zero-default field already saved, else note it); `Close::strike` wide enough for any allowed rate (if that needs a
   layout change, instead cap `windup_ms`/`recover_ms` ranges so it cannot overflow and test it); `flame_never_takes_pool_slots`
   as the plan says (two flamers, mortar fire, 50 m aside, with the real flamethrower's 30 m range: aim at ground within range, 50 m
   from the pool fire); sim tests that `ClearTrench` and `Strike` are refused while their switch is 0. Commit.

## Chunk 2: data and Clear trench
8. Data of plan 4.1-4.2: `fmw41`, `m1a1_flamethrower`, `m2_2_flamethrower` (no card), cartridges `fmw41_fuel`, `m1a1_fuel` (all with
   `sources`); the two `when` pairs in Sturmpioniergruppe and Assault Section; slot-name rule for pairs; derived capability `flame`
   (`Capability::Flame = 15`, from the members that survive the `when` filter only). Test: with `enabled` 0 both cards spawn man for
   man as `<BASE>`. Replace the test-made flame and fuel defs in tests and rows with the real ones where that is the point of the test.
   Resolved-defs baseline (`BLESS=1`), canonical snapshot for new defs only. Commit.
9. **T185 again:** the default 40 set into `target/s14-B-40/`; `python3 -I tools/neutral.py target/s14-base40 target/s14-B-40`.
10. C1-C5 `ClearTrench` (plan 6) with the plan's tests (`clear_trench_walks_the_trench`, `clear_refused_off_trench`,
    `clear_ends_on_new_order`, `clear_waits_for_own_fire`, `clear_off_is_refused`, save/load mid-step); `save_fuzz` staging gains a
    squad mid-clear. Commit.
11. Rows: R1 with the real flame men (if it still fails, find out why: a mechanics defect is this stage's to fix; if it is only that
    five alert defenders kill a lone flamer at 20 m before he fires, say so in the log and make R1 report-only with the reason, as
    the owner's no-tuning rule says: a later balance pass decides); wire R1 and R2 into `json_out`'s `checks` so a failing hard row
    raises `hard_failed`; R3, R4, R6 (reports) in `fortify/s14.rs`; rows into `targets/S14.toml`. Commit.
12. `tools/check.sh quick a27ce63` green (IPv6 test excepted) through `tools/run.sh`. Rewrite the log's "For the next stage" block
    for Build C (melee): state, code map, deviations.

Files owned: as Build A, plus `mods/ww2/defs/` (weapons, cartridges, the two cards), `sdk/crates/content/tests/resolved_defs.rs`.
Not the presenter's GDScript, not `crates/sim/src/ai/`.
