# S12 log (wounds and medics; at most 150 lines)
Spec: `docs/slices/S12-wounds-medics.md`; stages `S12a-build-A.md`, `-B.md`, `S12b-build-C.md`; targets `targets/S12.toml`;
ablations `targets/S12-ablations/`. Branch `claude/project-thread-aj1tww`, base `<BASE>` = `1209c99` (main: S11b + B2).
Commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: S12a Build B + Fix F done** (d611c16..HEAD on 0493f07): W6-W11, cards, aid post, commands, snapshot, R1-R6, checker F1-F8,
G1-G9. Switches 0 play as `1209c99`: `neutral.py target/s12-base40 target/s12-F-40` same 40/40 (after G1-G9). Not done: S12b (AI, presenter, gate).
**LAYOUT commit 84e95fd** (the one saved-setup change): `StockSetup::beds`, `staff_min`, `DigProfile::AidPit` 9, `RulesSetup::aid_pit`;
SAVE 30 / PROTOCOL 16 kept (not shipped); BASELINE in `resolved_defs.rs` updated. Fix F changed no layout.
**Code map.** `crates/sim/src/medics.rs`: `medics_tick` (end of `wounds_tick`) = `carry_tick` (W7 fetch/wait/carry/arrive; a walk to
`dest` that ended short counts as arrived: set down there, `carried` only within 2 m), `stabilise_tick` (W6), `auto_tend`, `posts_tick`
(W8; a treat bed drops a man no longer a wounded soldier), `base_evac`, `evacuate`. Orders `order_tend`, `order_carry`, `order_evacuate`,
`order_reinforce`. Helpers `is_medic`, `carriers()`, `staffs`, `post_staff`, `release`, `other_job` (G3), `release_helpers` (G4: called
from `combat.rs kill`/`down` and the dressing's end in `wounds.rs`). Load checks `wounds.rs validate_s12` (+G9, G1 bed state).
**What the AI (Build C) calls.** Commands `Tend{units,target}`, `Carry{units,target,to}`, `Evacuate{units}`, `Reinforce{squad}`
(refusals as `FortEvent`: NotAllowed/Cap/Manpower; NotAllowed also in the card's cooldown); `Build` with the faction's Aid def
(`stock_of(f, Aid, None)`, `built_by` medic); read `MatchState::casualties` (bleed_out_at 0 = stabilised), `Structure::beds`,
`is_medic`, `post_staff`. Cards `ger_sanitaetstrupp` (P 38, 16 rounds each), `usa_aid_men` (0 rounds); ai_weight 0. `[ai.medic]` no code.
**What the AI must do itself** (the sim leaves these men detached on purpose): (1) **reinforcements** walk detached to the squad's
first man and stand there: `Regroup` (or any squad order) once they are within a few metres folds them into the formation; (2) a
**carry set down** (a carrier hit, re-ordered or dropped out) halts the other carrier detached where he stands: Regroup him or re-issue
`Carry`; (3) a **wounded man sent to a post** (`MoveUnits` detached) stays detached until treated (he then rejoins); an untreated one
the AI walked off must be regrouped; (4) **post staff**: medics must stand in the radius with no move order (detached halted or a held
squad); the sim never releases them, but auto-tend may send one to a downed man within `auto_tend_m`; (5) a **Tend** buddy or medic whose job ended rejoins by himself
(G4), but a man the AI halted (`StopUnits`) within 3 m of an undressed wounded friend counts as his buddy and rejoins when the dressing ends.
**Tests.** `s12b_tests.rs` (27: W6-W11, saves mid-job, snapshot, G1-G6, G9), `s12_tests.rs` (+F1-F8), bridge intents, `s12_content.rs`
(cards, aid def, ablations, G7, G8), fuzz `staged_s12`, 1/2/3/8-thread hash of the medic scene.
**Rows, fortify10 seeds 1-10** (`S12_SEEDS=10`, after G1-G9): R1 downed 180/220, outright 0.0 % (report), bled out 80.6 %; R2 bled out
16.9 %, stabilised 66.7 %, drop 63.7 pts (pass); R3 treated 100 %, 75 HP, stops unstaffed 100 %; R4 0 %; R5 100 %, 0, 0; R6 filled
100 %, rifle kit 100 %; R7 100 % (pass), R8 66.7 %.
**Deviations** (plan W6/W9/W11 amended): buddy = own detached halted man within 3 m on no other job (no saved field marks a `Tend`);
Evacuate of a wounded man in base at once; a reinforcement is a member at once, walks detached; Reinforce refused in the cooldown;
basic kit = first slot with no crew weapon, no trait. Earlier: R1/R2 at 100 m, R3 "medics die" = moved out, F1-F8 accepted items.

## S12a F (checker G1-G9 on Build B)
G1 bed drops a downed/dead man (+load check) 4d45f4f, c251fcd. G2 treatment marks the chunk 905c31f. G3+G4 one commit e491a75 (shared
`other_job`): "buddy from Tend only" needs a saved field, so buddies exclude men on another job instead; release on casualty death or
evacuation (tender, carriers, men walking to him), wounded man's death, down or dressing. G5 6c13fe3: slots kept, one appended per
man; the new man walks detached (the smaller change: not in the column, so no strung-out squad) and is folded in by the next squad order.
G6 872c19b: arrival short of `dest` (nav goal = dest, order Idle) ends the carry; no retry counter (none fits the layout). G7 5ec6558.
G8 2f392c3: finding was wrong: the P 38 card counts rounds (weapon `load` 16, no `rounds` key needed); test asserts it; comment fixed.
G9 c251fcd (also: a man in two beds). Plan text 90ab739. Neutral same 40/40; check.sh quick green (but IPv6).
