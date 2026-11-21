# S12 log (wounds and medics; at most 150 lines)
Spec: `docs/slices/S12-wounds-medics.md`; stages `S12a-build-A.md`, `-B.md`, `S12b-build-C.md`; targets `targets/S12.toml`;
ablations `targets/S12-ablations/`. Branch `claude/project-thread-aj1tww`, base `<BASE>` = `1209c99` (main: S11b + B2).
Commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: S12 F2 done** (review R1-R10, bridge gaps B1-B3 on 39a537a; commits 8575d23..HEAD). Next: orchestrator reads the 480 and
ablations (they run from a snapshot taken before F2: their T8 is the old double-counted reading, see below) and decides T8.
**One layout addition** (8575d23, LAYOUT): `S12Counters::finished_by_class[kill_class::COUNT]` (saved, hashed); SAVE 30 / PROTOCOL 16
unshipped, no new bump. Setup bytes and the canonical setup snapshot do not change (match state, not setup); no BASELINE refresh needed.
**T8 metric (R1).** `he_cas_pct(kills, finished, downs)`: per class `kills_by - finished_by_class + downed_by_class` (one casualty per
man, by the class that first took him out), HE classes (mortar, tank HE, rifle grenade) over all. `finished_by_class` counts deaths of
downed enemies (bleed-out: the down's class; a later hit; a same-tick second hit) via `MatchState::credit_finish`. JSON `s12.finished_by`.
**Corrected 40 sets** (seeds 1-20 both bases, this binary; `target/s12-F2-40`, `target/s12-F2-off40`):
switches on: he_share_casualties_pct mean **29.97** (max 50.8; was 31.2 double-counted), Germany 37.6 / USA 24.3 (were 38.1 / 26.3);
kills-only HE 39.8; Germany 20/40, no winner 0, length median 21.1 (11.3-31.1, mean 21.7, 11 over 25; Build C: 21.5, 10 over 25:
R7/R8 change the AI slightly). s12_off: he_cas 25.48 (= kills share), Germany 32.7 / USA 21.0, Germany 20/40, median 20.1.
**T8 still misses** (29.97 > hard 26.3): the double count was ~1.2 points of the 4.9; blast still kills outright while bullets down.
**T13a (R2).** fortify-test counts a player's men out of the fight = losses + own downed men lying at the window's end. Mortar bar with
the shipped switches: **37.8 %** over 10 seeds (limit 40, PASS; 41.3 % before); fortify-test PASS; `fortify_test_passes` no longer
plays `s12_off` and passes (5 seeds).
**Neutral.** `s12_off` 40 = `target/s12-base40` under `neutral.py`: same 40/40.
**Bridge/presenter.** state `rules.wounds {enabled, reinforce}` (B1), no medic cards in `callin.catalog` with wounds off (B2),
structures `beds_max`, `staff_min` (B3; presenter draws empty cots and staff marks), `map.own_base {pos, radius}`. Evacuate is live
only for selected wounded men in the own base or a staffed post's ring with a free bed (R3); no downed-man click. A downed man's body
no longer replays the fall on death; debug "corpses" excludes downed men (R5). GDExtension not built here: check_project skips
match.tscn, test_data_reload skips (bridge not built), as for the presenter.
**Open.** T8 (owner decision: S12-15 (A) as is, or (B) kills only). Stabilised only ~3 % of downs (unchanged, not tuned).

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

## S12 F2 (review R1-R10, bridge gaps B1-B3)
R1 8575d23 (LAYOUT: `finished_by_class`; T8 one casualty per man; test `r1_finished_downed_men_count_once`, per-class outright =
`kills_by - finished_by_class` equals `killed_outright`). R10 7db6f1d (`wound_ok` bounds the vehicle-type bits by the setup's
vehicles). R6+R7+R8 2545428 (medic AI: scans collected once; a patient medic is on a job; one Regroup per pass; tests r7, r8 fail with
the fix taken out). R5, B1-B3 bridge cbb16d5. R3, R4, B3 presenter 43b3992 (test_match_view 433/434: the known 16-vehicles perf check).
R2 814241d (fortify losses + downed lying; test plays the shipped switches).
`check.sh quick 1209c99` green (1218 passed; only `udp::ipv6_loopback_handshake`, no IPv6 here). Godot: test_match_view 433/434 (perf),
test_net_ui 143/143, check_project OK.
**R9: finding wrong, no change.** An unbuilt aid post whose builder dies or stops digging is removed by the Status stage
(`structures::update_structures`: a site whose builder is gone or no longer digs its pit line is built if deep enough, else dropped),
so `have` is true only while a team digs it; the next pass sees no post and orders the build again (Build on the old site would be
refused by `aid_max` anyway).


## Orchestrator: final measure (2026-11-21)
480 at `b89ef42` (`target/s12-on2`): Germany 42.71 % (45.00 / 42.50 / 40.62), no winner 0, HE share of casualties 28.3 /
28.0 / 28.1 (T8 cap 26.3: known miss S12-16, accepted for now, first item of the next balance pass). Ablations
(`target/s12-abl2`, against 42.71): s12_off 44.17 (+1.46; neutral vs `target/s12-base40` same 40/40), no_medics 41.04
(-1.67), no_post 44.17 (+1.46), no_reinforce 45.00 (+2.29, matters), head_300 43.54 (+0.83; HE share of casualties 26.6).
Paired s12_off vs default moves 1.46 points (< 5: no stop). CI run 47 (workflow_dispatch) on `91078e9`: success.
