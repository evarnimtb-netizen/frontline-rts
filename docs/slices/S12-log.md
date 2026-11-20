# S12 log (wounds and medics; at most 150 lines)
Spec: `docs/slices/S12-wounds-medics.md`; stages `S12a-build-A.md`, `-B.md`, `S12b-build-C.md`; targets `targets/S12.toml`;
ablations `targets/S12-ablations/`. Branch `claude/project-thread-aj1tww`, base `<BASE>` = `1209c99` (main: S11b + B2).
Commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: S12b Build C done** (78e8f0b..HEAD on aff69f3): medic AI A1-A5 and liveness, `s12_derived`, T8 over casualties, T170-T173,
switches on (`[wounds] enabled` 1, `reinforce` 1, `[ai.medic]` 1, 1, 1, 1; medic cards `ai_weight` 10). Next: orchestrator 480 + ablations.
**No layout change.** SAVE/PROTOCOL as S12a; `s12_off` 40 = `target/s12-base40` under `neutral.py` (same 40/40, final binary).
**Code map.** `crates/sim/src/ai/medic.rs` `Planner::medics` (called after `supply`, before the buys): takes medic teams (tag `medic`) out
of `mine`; `patients` (A4 retreat sends untreated wounded by `MoveUnits`; squads with patients stay out of `mine`); team liveness
(`man_job`: Busy/Free/Stale from casualties, orders, `UnitNav::phase_ticks`; stale or `STALL_S` 120 s -> `Regroup`); `build_post` /
`post_site` (A2), `staff_post` / `post_needed` (A4), `buy_medics` (A1), `field` / `assault_lead` (A3), `reinforce` (A5 + fold-in Regroup
of a halted reinforcement in base); `medic_pot` (pending Reinforce price held from `buy`, whose `bought` counts its card). No saved state:
plans draw no RNG. `ai.rs` wiring: `units_of` Tend/Carry, `squad_of` Reinforce, validate unit counts. `MatchState::downed_by_player`.
**Stateless readings (deviations).** A1 "3 downs in 120 s" = 3+ own downed men lying now (each went down within `down_bleed_max_s`);
"second team when the first staffs" = a team in the built post's radius while men wait. A3 reach 100 m (`FIELD_REACH_M`), carry clear
30 m. "120 s without progress" = 120 s in one nav phase (a carry's Moving phase 600 s). A5 "in base or retreated" = leader in base or
`Retreat` order. Fold-in only for squads in base, Idle or retreating (any squad order folds them in anyway).
**Report.** `s12_derived` per faction (plan 8 names + `team_matches_pct`, `matches`; `medic_matches_pct` = matches with an own man
stabilised or treated); set-level `s12_derived.he_share_casualties_pct` {mean,max} (T8a-c read it, T8d kills only). `pool.py` S12 line.
`neutral.py` skips `s12_derived*` keys BASE lacks (as `s11b`).
**40 sets** (seeds 1-20 both bases): default Germany 20/40 (50 %), no winner 0, length median 21.1 (min 11.3, max 31.1, 10 over 25);
medic buys G 88 / U 80, tends 165/129, carries 41/36, posts 51/58, reinforce orders 134/87; medic_matches_pct G 87.5 / U 77.5;
stabilised 3.3 / 3.0 % of downs, bled out 49.7 / 45.0 %; HE share of kills 39.9 (base 25.5), **he_share_casualties_pct 31.2 > T8 26.3**.
s12_off: Germany 20/40, median 20.1, 11.9-31.8, medic 0, he_cas 25.5 (= base kills share).
**Misses to read (not tuned, owner rule).** (1) T8 over casualties 31.2 (hard 26.3): wounds turn bullet kills into downs while blast still
kills (plan risk 7); the gate already had kills-only 37-38 %. (2) T13a fortify10.pass: the S1 mortar bar reads 41.3 % (limit 40) with
wounds on, 38.6 % with `s12_off` (downed men are not losses inside the 10-round window). The cli test `fortify_test_passes` now plays
`s12_off`. (3) Stabilised only ~3 % of downs: two teams at most, `enemy_clear_m` 40 holds most tends; features, not tuned.
**Fixes found by the shipped-switch save round trips** (sim, outside the owned list): `wounds::wound_ok` checks `kill_class::base` of
`wound.by.1` (a hull MG's class carries the vehicle type); a knock-out and an abandonment clear `supply_due` (a wreck kept a due rearm
step with `base_ticks` 0 and failed the load). Old tests that assume S12 off call `MatchSetup::s12_off()` (beside `s11b_off()`, combat
rig, s10a fight); `tests/ai.rs` allows the medic team's Dig; ww2 canonical snapshot blessed; s12_content asserts the switches on.
**Tests.** `s12_ai_tests.rs` (17: A1 x2, A2, A3 x3, A4 x2, A5 x2, liveness x4, switches off, save/load mid AI carry, the wound.by fix).

## S12a F (checker G1-G9 on Build B)
G1 bed drops a downed/dead man (+load check) 4d45f4f, c251fcd. G2 treatment marks the chunk 905c31f. G3+G4 one commit e491a75 (shared
`other_job`): "buddy from Tend only" needs a saved field, so buddies exclude men on another job instead; release on casualty death or
evacuation (tender, carriers, men walking to him), wounded man's death, down or dressing. G5 6c13fe3: slots kept, one appended per
man; the new man walks detached (the smaller change: not in the column, so no strung-out squad) and is folded in by the next squad order.
G6 872c19b: arrival short of `dest` (nav goal = dest, order Idle) ends the carry; no retry counter (none fits the layout). G7 5ec6558.
G8 2f392c3: finding was wrong: the P 38 card counts rounds (weapon `load` 16, no `rounds` key needed); test asserts it; comment fixed.
G9 c251fcd (also: a man in two beds). Plan text 90ab739. Neutral same 40/40; check.sh quick green (but IPv6).

## S12b C (Build C)
78e8f0b medic AI + 16 tests. Chunk 2 commit: `s12_derived`, T8 over casualties, T170-T173, switches on, `ai_weight` 10, the two save
fixes, `s12_off()` for old tests, pool.py and neutral.py. 40 sets in `target/s12-C-40`, `target/s12-C-off40`. `check.sh quick 1209c99`
green (1215 passed; only `udp::ipv6_loopback_handshake`, no IPv6 on this machine).
