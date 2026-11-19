# S11a1 Build C: artillery AI (buy, emplace, observer, fire, budget, lift) and preparation fire for the breach

Read first: `S11a1.md` 4 (A1-A6, P1-P3, liveness), 5, 6, 8 (the fallback if time runs out); the log's top block
(Build B's notes on spawning and card ids). **2 chunks**, about 30 self-counted turns each (60-90 minutes in all).
No layout change (every field exists since Build A), no version change. Each habit sits behind its own switch
(`buy`, `fo`, `prep`); with `a_off` the 40 set equals `target/s11a1-base40`. Chunk 1 is a complete result by
itself: if chunk 2 does not fit, leave `prep` 0, say so, and stop at STOP POINT 1.
Work rules as in Build A (4-minute rule, `tools/run.sh`, `BATCH_JOBS=4 tools/batch.sh`, commit only your files, no
push, no tick times). No RNG in any plan; iterate `BTreeMap`s and ascending ids only; integer metres and seconds.

Files owned: `crates/sim/src/ai/arty.rs` (planner code), `crates/sim/src/ai.rs` (the call order in the plan, the
buy hook), `crates/sim/src/ai/mortar.rs` (the scorer made generic, the P1 entry), `crates/sim/src/ai/breach.rs`
(P2), `crates/sim/src/ai/fort.rs` (only the two `gun_pot()` lines 2824 and 2921), new `crates/sim/src/s11a1_ai_tests.rs`;
`sdk/crates/cli/src/fortify/s11a1.rs` (`breach_prep`), `sdk/crates/cli/src/fortify/s10a.rs` (only to share
`breach_ai_world`); `mods/ww2/defs/rules/skirmish.toml` (`buy`, `fo`, `prep` to 1, BASELINE, snapshot);
`docs/modding/tuning.md`; `docs/slices/S11a1-log.md`.

Start from: `ai/guns.rs` (`gun_pot` 1174, `buy_gun` 1215, `take_guns` 385, `gun_jobs` 438: how a job map is
walked and a dead squad's record dropped, `repath_s`); `ai/mortar.rs` (`mortars` 177: `front`, the ray budget
`mortar_rays`, `fire_observed` 361, `mortar_targets` 448, `spot_for` 616 as the pattern for the post search,
`knows`, `fire`); `ai/breach.rs` (`run_breach` 487: phase Stage, `covered`, `base_firing`, `breach_focus_of`;
`drive_breach_plan` 962); `ai.rs` (`buy`, `buy_role_within`, `Planner`, where `mortars` and `gun_jobs` are
called); `game.rs` (`mission_refusal`, `mode_for`, `known_by_record`); `crates/sim/src/s10a_ai_tests.rs`
(`ai_breach_*` scenes) and `s9_tests.rs` (AI gun scenes) for staging.

## Chunk 1: the gun and its observer (A1-A6)
1. Wiring first (spec 4 A1 "Wiring"): `take_arty` beside `take_guns` moves infantry guns and observer squads out
   of `tanks` / `mine` before `assign` (`ai.rs` about 1341; `order_infantry` 1412 must never see an observer);
   `arty_pot()` joins `gun_pot()` at `ai.rs` 1663 and `ai/fort.rs` 2824, 2921 through one helper (0 with `buy` 0);
   `arty` is called after `mortars` (1413) and uses the rays `mortars` left. Tests
   `ai_observer_gets_no_infantry_order`, `arty_pot_is_kept_from_other_buys` (control `buy` 0: the budgets equal
   base). Then A1 buy and A2 emplace with their tests (Build A's `at_gun_ai_ignores_infantry_guns` stays green).
2. A3 observer: buy, post search (bounded rays, saved cursor), move and crouch, clear and retry; its three tests.
3. A4 fire, A5 budget, A6 lift with their tests. `mortar_targets` becomes generic over (range band, danger-close
   distance) without changing one mortar decision: test `mortar_targets_unchanged_for_mortars` (the existing
   S5b/S6b AI tests stay green untouched) and the neutral run of step 4.
4. Liveness and neutrality: `ai_all_s11a1_switches_off_emit_nothing_new`; `ai_rebuys_a_lost_gun_inside_the_pot`,
   `ai_observer_is_rebought_once`, `ai_arty_deterministic_threads_and_save_load` (1 and 4 threads, save/load at
   three ticks: equal hashes). Turn `buy` and `fo` on in `skirmish.toml`. Runs: default 40 set into
   `target/s11a1-C1-40/`, `a_off` into `target/s11a1-C1off40/`; `neutral.py target/s11a1-base40
   target/s11a1-C1off40` -> `same 40/40`.
**STOP POINT 1.** `check.sh quick 730e67f` green; commit; log top block with the 40-set numbers below; return if
over 45 self-counted turns (then chunk 2 is a new session or moves to S11a2: say which you recommend).

## Chunk 2: preparation fire (P1-P3) and the breach row
5. P1 in `arty.rs` and `mortar.rs`, P2 in `run_breach`, P3 through A6; the three tests of spec 4 and the
   "Breach prep" row of spec 5. With `prep` 0 `run_breach` must take the same branches as today (the existing
   `ai_breach_*` tests stay green untouched).
6. Fortify row `s11a1.breach_prep`: the `breach_ai` scene and force plus ONE mortar team of the attacker's
   faction 250 m behind the stage spot (its leader within the voice link, so no new unit is needed to prove P1-P3);
   the planner pieces driven as `drive_breach_plan` does, plus the mortar planner's P1 step. Fields:
   `gap_opened_pct`, `placed_pct`, `attacker_loss_pct`, `defender_loss_pct`, `prep_fired_pct` (runs where a prep
   mission fired at least `prep_rounds_min` rounds before the carrier left), `control.*` (the same force with
   `prep` 0). A second variant with a leIG 18 in base and a VB-Trupp instead of the mortar goes under
   `breach_prep_gun` (report). If driving the mortar planner inside the row does not fit the chunk: keep the unit
   tests, emit the row's keys as null, and say so first in the log (T131 then prints SKIP).
7. Turn `prep` on. Runs (each through `tools/run.sh`, `BATCH_JOBS=4`): fortify 10 seeds into
   `target/s11a1-C40/fortify10.json`; default 40 set into `target/s11a1-C40/skirmish40.json`; `a_off` into
   `target/s11a1-C40/skirmish40_a_off.json`; `prep_off`, `fo_off`, `arty_off` 40 sets into
   `target/s11a1-C40-<name>/`. `neutral.py target/s11a1-base40` against the `a_off` folder: `same 40/40`. Then
   `target/release/frontline-cli targets docs/slices/targets/S11a1.toml --from-json target/s11a1-C40` and copy the
   T128-T138 lines into the log.
**STOP POINT 2.** `check.sh quick 730e67f` green, last; commit; rewrite the log's top block.

## The stage must log
Commits; per faction on the default 40 set (a 40 set has 11-15 points of noise: these are sanity numbers, the
480 is the orchestrator's): Germany's wins of 40 against base and `arty_off`; `gun_bought_matches_pct`,
`gun_fired_matches_pct`, `gun_rounds_per_match`, `gun_observed_pct`, `gun_kill_share_pct`, `gun_shell_mp_pct`,
`near_own_bursts_per_match`, `gun_refused_total`, `fo_lost_per_match`, `fo_post_fail_pct`, `arty_no_observer_pct`,
`prep_missions_per_match`, `he_share_pct.mean`; `breaches_started_per_match` (expected still near 0: spec 0);
the `breach_prep` row with its control; `neutral.py` lines; test count.
Stop and report instead of tuning when: Germany's 40-set share leaves 25-75 % (twice the noise outside the
guard band); `he_share_pct.mean` passes 26.3; `gun_refused_total` is not 0; the AI buys no gun in more than half
of the matches (name the first blocking check: add a why-counter only if an existing counter cannot tell).
For Review: the list of orders the AI can now emit for a gun squad and for an observer squad, and where each is
rate-limited.
