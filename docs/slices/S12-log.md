# S12 log (wounds and medics; at most 150 lines)
Spec: `docs/slices/S12-wounds-medics.md`; stages `S12a-build-A.md`, `-B.md`, `S12b-build-C.md`; targets `targets/S12.toml`;
ablations `targets/S12-ablations/`. Branch `claude/project-thread-aj1tww`, base `<BASE>` = `1209c99` (main: S11b + B2).
Commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: S12a Build B done** (d611c16..HEAD on 0493f07): W6-W11, cards, aid post, commands, snapshot, R1-R6, checker F1-F8. Switches 0
play as `1209c99`: `neutral.py target/s12-base40 target/s12-B-40` same 40/40 (after F1-F8, R1-R6). Not done: S12b (AI, presenter, gate).
**LAYOUT commit 84e95fd** (the one saved-setup change, a field truly missing): `StockSetup::beds`, `staff_min`, `DigProfile::AidPit` 9,
`RulesSetup::aid_pit`; SAVE 30 / PROTOCOL 16 kept (not shipped); BASELINE in `resolved_defs.rs` updated.
**Code map.** `crates/sim/src/medics.rs`: `medics_tick` (end of `wounds_tick`) = `carry_tick` (W7 phases fetch/wait/carry/arrive;
pace `stretcher_speed_pct` two medics else `drag_speed_pct`; a carrier no longer Fit sets him down), `stabilise_tick` (W6; ready = medic
within 2 m, still, no move order; pinned pauses +1 tick; lowest id tends), `auto_tend` (once a second, `auto_tend_m`), `posts_tick` (W8
beds ascending id, staffed by `staff_min` own medics in radius, treat `treat_s` to `return_hp_pct`, `treated` once; evac beds), `base_evac`
(W9 `Casualty::evac_at`), `evacuate` (refund `evac_refund_pct` of the per-man price if `paid`, once). Orders: `order_tend`,
`order_carry`, `order_evacuate`, `order_reinforce` (W11; dispatched in `game.rs`); helpers `is_medic`, `carriers()`, `staffs`,
`post_staff`, `release`. Buddy bandage (W6) in `wounds.rs` `wounds_tick`. Pace hooks `unit.rs`/`squad.rs`, carriers hold fire
(`combat.rs aim_and_fire`). StockKind::Aid audit in `structures.rs`: `serving` and serve sources skip Aid, load cap `aid_max`,
`order_build` max per kind, box-break note. `call_in` sets `paid` (wounds on, cost > 0, not vehicles).
**What the AI (Build C) calls.** Commands `Tend{units,target}`, `Carry{units,target,to}`, `Evacuate{units}`, `Reinforce{squad}`
(refusals as `FortEvent`: NotAllowed/Cap/Manpower); `Build` with the faction's Aid def (`stock_of(f, Aid, None)`, `built_by` medic);
read `MatchState::casualties` (bleed_out_at 0 = stabilised), `Structure::beds`, `is_medic`, `post_staff`. Cards `ger_sanitaetstrupp`,
`usa_aid_men` (tag medic, ai_weight 0). `[ai.medic]` switches exist, no job code.
**Tests.** `s12b_tests.rs` (20: W6-W11, saves mid-tend/carry/bed/evac, snapshot), `s12_tests.rs` (+F1-F8), bridge intents test,
`s12_content.rs` (cards, aid def, ablations), fuzz `staged_s12` (12021 mutations: 0 panicked, 0 unloadable), 1/2/3/8-thread hash of
the medic scene. Snapshot: `soldier_wound` (`wound_bits`), `casualties`, structure `beds`/`staff` (`godot/docs/match_state.md` S12).
**Rows, fortify10 seeds 1-10** (`S12_SEEDS=10`): R1 mg300 downed 180/220, outright 0.0 % (report miss of 20-45: two LMG hits down,
overkill 60 is rarely reached), bled out 80.6 %; R2 medics bled out 16.4 %, stabilised 67.2 %, drop 64.2 pts (pass); R3 treated 100 %,
75 HP, stops unstaffed 100 %; R4 second treatment 0 %; R5 refund once 100 %, unpaid 0, repeat 0; R6 filled 100 %, rifle kit 100 %;
R7 trench 5.5 % vs 5.5 % = 100 % (pass), R8 prone 66.7 %.
**Deviations.** R1/R2 at 100 m, MGs fire 30 s then leave (cards see 150 m); R2's medics wait 60 m back, move up then. R3 "medics die"
= moved out (same rule; killing is a unit test). Steps 2+3, 4+5 share commits. Buddy = halted own detached man within 3 m; still < 0.5
m/s; tenders rejoin. Evacuate in base is at once; a reinforcement joins at once; basic kit = first slot with no crew weapon, no trait;
Aid Men: M1911A1, 0 rounds. Downed men: snapshot `casualties`, not `corpses` (bridge draws them as bodies). Checker F1 `<` tick, F2
bleed-outs in `stats.deaths`, F3 mine downs at the down (wire at death), F4 two same-tick hits kill, F5 own wounded pace, F6 Downed
spotted, F7/F8 tests; accepted: downed 1 HP, riders/gun crew lost at down, crewman down in place, reload pct on aim. Next: S12b.
