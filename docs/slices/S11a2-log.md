# S11a2 log (friendly fire from indirect HE, counter-battery by sound, AI safety and counter-fire; at most 150 lines)
Spec: common `docs/slices/S11a2.md`, stages `S11a2-build-A.md`, `-B.md`; targets `targets/S11a2.toml`; ablations
`targets/S11a2-ablations/`. Branch `claude/project-thread-e7nsak`, base `<BASE>` = `b74ed58` (the S11a1 head when Build A
started; neutral base set `target/s11a2-base40`). Another agent may work in this checkout: commit
only your files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`, `target/`,
`tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: Fix of Build B done (B-1..B-7); the 480 measure at `e149258` done 2026-11-17 (see Measure and docs). Next: the owner's merge decision (S11a2-10..12); `tools/verify.sh` not yet run.**
- SAVE 28, PROTOCOL 14. All four switches 1 in ww2. Neutral base `target/s11a2-base40-fix`; `b_off` = it 40/40 (Fix).
- Counter-fire share: USA 62.8 % of its gun rounds at 480 (T148d, bound 50, report). The next knob outside the spec's keys is a per-gun counter-fire round budget (S11a2-10).
- Weakness: with `ff_safe` the spotter must stand 70 m to `observe_leader_m - 10` from the target; a mod with a short observe range gets no spotter. The `ff` row's own_vs_enemy 104.8 % is the blast as built, not tuned.
- Tick check (A-5): `observe_bursts` scans every squad and own soldier per mortar burst even with the switch 0; measure it in `verify.sh`.
- Code map: `ai/arty.rs` `own_near` / `own_near_within`, `ff_lift`, `cb_candidate`, `counter_fire`; `counter_battery.rs` `cb_note`, `own_marker_err`; `game.rs` `order_gun_mission`; `fire_mission.rs` `CB_ROUND`, `count_cb_burst`; rows `sdk/crates/cli/src/fortify/s11a2.rs`.

## Fix of Build B (2026-11-17; independent review of Build B: 1 blocker, 2 major, 4 minor; all verified in the code)
Commits: `11f5bb0` B-1, `792afbb` B-3, `c4938cb` B-2, `4feaa7d` B-4, `60461a0` B-6, `d80e28a` B-1 (2/2), `1a587fd` B-5, `db577e6` B-7.
B-1 (blocker) counter-fire fed itself: every counter-fire round added `shots` with the enemy, so one heard round ordered a 4-round
answer, and so on. Decided: a round of a `cb` mission adds evidence (the firer's marker still forms and sharpens: counter-fire is
heard) but no `shots`; R2 skips a spot with 0 shots. Evidence stays (markers no longer immortal once the loop is gone: a gun that
stops firing loses its marker by decay). The first 40 set after it still showed USA at 68.2 %, so R2 also needs
`cb_rounds_to_mark` new shots (as the review asked): 62.3 % after. The load check drops "marked needs a shot". Tests
`counter_fire_rounds_add_evidence_but_no_shots` (control: no marker, shots 3), `ai_counter_fire_waits_for_new_evidence` (2 new
rounds held, control 3: a mission).
B-2: with `[ai.arty] counter_fire` 1 map fire on an own marker aims with the marker's error: bearing then a length uniform in
`0..=err_mm` (two draws of `MISSION_AIM`), at most the map-fire error (so the load bound holds); no new key. Bursts of ended
counter-fire missions count: the round's projectile carries `CB_ROUND` (bit 31) in its mission id (no layout change; the load
check and the burst log mask it). JSON docs and `tuning.md` updated. Tests `counter_fire_aims_with_the_marker_error` (8 seeds;
control `counter_fire` 0: >= 49 m), `counter_fire_bursts_count_after_the_mission_ended` (4 of 4, fewer live; control no marker: 0).
B-3: R2 new evidence is `shots >= cb_shots + cb_rounds_to_mark` or `shots < cb_shots` (fewer = another marker). Test
`ai_counter_fire_answers_a_new_marker_with_fewer_shots` (control: no second marker, no mission).
B-4: counter-fire's own-men check uses `danger_close_m` + the aim error the sim will use (map: min(err, map-fire error); observed:
first-round error). Test `ai_counter_fire_spares_own_men_within_the_aim_error` (70 m held; control 120 m: fires).
B-5: the `ff_lift` squad starts 130 m out (outside `lift_m` 100; the run bails if not); targets comment updated; the row test checks
lifted/clean 100 and control clean 0 on seed 1. Rows 1-10 as above (T142a-c pass).
B-6: S1 (ii) also reads the AI's own pending `Move` / `AttackMove` (the plan's own and earlier ones not yet applied); the mortar
scorer passes a copy (only with `ff_safe` 1). Test `own_near_sees_a_pending_move_onto_the_target` (control: no pending: false).
B-7: S5b `the_ai_sends_a_spotter_with_ff_safe` (leader observes to 150 m; the 100 m scene has no stand-off); save_fuzz
`save_roundtrip_carries_s11a2_state` (shipped switches, arty AI on: a save at tick 17400 holds a marker and an answered one, hash,
bytes and a 2-thread continued run equal). The old played saves stay with the switches off.
Sets (`BATCH_JOBS=4`, seeds 1-20 both bases, `target/s11a2-F40`): default Germany 19/40; gun rounds Ger obs 467 / map 320 / cb 218
(27.7 %), USA 207 / 435 / 400 (62.3 %); markers 0.975 / 0.975 a match; near bursts 86 of 205 / 131 of 394; ff_kills 0 / 0; ff_lifts
6.15 / 3.85 a match; refused 0. `counter_fire_off` Germany 18/40. `neutral.py base40-fix s11a2-Foff40`: same 40/40.
Check: `tools/check.sh quick 730e67f` all ok, 982 tests (release CLI rebuilt first).

## Build B (2026-11-17)
Commits: `f705d08` M1 JSON, `8ad7075` fix A-1/A-2, `7a3f240` M2 rows, `d7d075b` S1-S3 (+fix A-3), `0ca316e` ff + ff_safe on,
`11b725b` cb on, `4ec1712` R1-R3 + fix A-4, `6411fc1` M3 ff_lift, `2173fbd` counter_fire on, `b649cd2` fix A-6, `3bc4a4c` docs, `1c5ace8` old tests switches off; log.
Sets (seeds 1-20 both bases, `BATCH_JOBS=4`): default `target/s11a2-B40` Germany 21/40, `b_off` 15/40, `ff_off` 18/40; no match
without a winner; refused 0, gun refused 0. he_share 21.86 (ff_off 20.00); bombs a match Ger 92.9 / USA 72.0 (ff_off 98.6 / 74.1).
Default, Ger / USA a match: ff_kills 0.05 / 0 (2 mortar, 0 gun in 40), ff_wounds 0.075 / 0; ff_lifts 5.0 / 2.9; ff_holds 251 / 162;
cb_marks 0.98 / 1.0; cb_first_error_m 95.9 / 92.5; cb_fired_matches 94.9 / 95.0 %; counter-fire missions 10.35 / 6.7, rounds
39.0 / 24.6; cb_near_burst 0 / 0 %; gun crews killed by counter-fire: none seen (near bursts 0); cb_masked 0 / 0.075; cb_wide holds
25.8 / 36.1; cb_shell_share 80.6 / 90.8 %; gun_observed_or_cb 95.9 / 97.1 %.
`neutral.py target/s11a2-base40-fix target/s11a2-Boff40`: same 40/40 (overrides 0 -> 4 items). Chunk 1 (no counter-fire): b_off
same 40/40, default Germany 15/40.
Rows (10 seeds): `ff` own_vs_enemy 104.8 %, own loss 41.4 % (control friendly_fire 0: 0). `cb` marked 100 %, mean error 7.7 %, worst
14.3 %, last 23.9 m (control: 0 marked); answer (M3 runs only) near 1.3 %, crew loss 0, 80 rounds, refused 0 (control refused 10).
`ff_lift` (12 bombs, squad 90 m off ordered onto the point at the first bomb): clean 100 %, lifted 100 %, 20 rounds; control ff_safe 0:
clean 0 %, 240 rounds. Wall time added by the rows: 18.5 s.
Targets (`--from-json target/s11a2-B40`): 95 pass, 12 miss (hard 0). T139a/b, T140a-d, T141b, T145a-d PASS; T141a MISS (report,
near 1.3 < 10); T142a-c SKIP (fortify ff_lift null, values above); T134, T143, T144, T146-T148 SKIP (160 not played); no MISSING-METRIC.
Check: `tools/check.sh quick 730e67f` all ok, 974 tests (release CLI rebuilt first; the first run failed 7 old fixture tests, fixed in
`1c5ace8`: save_fuzz played saves keep all four switches 0; S5b/S6b spotter scenes observe to 100 m only, so they keep ff_safe 0).
Fixes of the stage A review: A-1 saturating load bounds (u64::MAX times load-rejected, no panic); A-2 err/heard bound 1.5 x map side;
A-3 P3 runs only with ff_safe 0, S3 lifts older prep missions and the walk-in; A-4 a gun-dry drop of a cb mission takes back
cb_missions; A-5 report only (above); A-6 test-only fixed-length draw for the spread control, grenade, satchel and AP mine cases.
Built differently from spec, with reasons: JSON names the AI counter `ai_cb_missions` (clash with the sim's `cb_missions`); S1 (ii)
counts a walking squad only when its phase is not Holding (a parked squad's stale goal held every mission); P1 uses `own_near` with
ff_safe on; counter-fire missions are not counted in `arty_missions` (T134 reads them apart); own AP mines hurt own men as before
S11a2 ("own mines count as today": the test asserts ff 1 = ff 0); the cb answer is played in the M3 runs only (the leIG scene is the
listener side); content tests and two old sim tests (`s10a` empty fort, `s11a1` gun lift) set their switches explicitly (outside the
contract's file list); AI tests use the AI's own mortar mission (a test order is cancelled by the AI's move to its spot), which needs
`barrage_rounds` = `mission_rounds` 12. Tests: 11 AI (s11a2_ai_tests.rs), rows test, JSON name test, neutral test; each with a control.

## Build A (2026-11-17)
Commits: `13b9c82` layout + keys, `625b079` ablate/check/doc, `2a43103` F1-F4, `2d4d9b7` fmt, `23fb60b` K1-K4; log.
Chunk 0: base set `target/s11a2-base40` from `b74ed58` (batch cache = `s11a1-C40`; Germany 15 of 40). Targets parse
check (CLI): the file loads; 16 pass, 2 miss (T145c/d: no metric yet, Build B), 571 skipped; no change needed.
Neutrality: `neutral.py base40 s11a2-A40` same 40/40; `... s11a2-Aoff40` (`b_off`) same 40/40 (also after F alone,
`s11a2-A1-40`). Sanity `friendly_fire = 1` (scratch override): Germany 18 of 40, no match without a winner.
Check: `tools/check.sh quick 730e67f` all ok, 957 tests (release CLI rebuilt first).
Built differently from spec, with reasons: the layout commit also holds the keys (the setup is saved and the load
bounds read `cb_decay_s` / `cb_delay_s`); `mortar_near_own_bursts` is counted always (a report like the gun twin, so
`ff_off` can read it), for live missions only; `cb_heard` counts once per round heard by anyone; the marker length
is an integer mm draw in `0..=err_mm`; a spot's `decay_at` is not reset by new evidence (spec K3 as written); F1's
"grenade" control is the same mortar burst with mission 0 (items set `ff: false` by construction).
Owner tests left as they are (S11a2-2): statics in the blast (`hit_static` filter), `blast_wire`, `blast_mines`,
`blast_open_crew`, `blast_tracks`, fragment damage to own frames (the ray still ends there), `frag_wire`, tracks.
Tests (s11a2_tests.rs 27, s11a2_content.rs 1): layout 2, F 11, K 14 (names in the commits; each with its control,
seen to fail where the control is a code change: the validate_spots call, the own-statics rule, the own-cover
exclusion, the ff_dead branch).

## Plan revision (2026-11-17, after an independent review against `a6c5c72` + `25436fa`: 5 major, 5 minor; all verified)
1 Save gap: `decay_at` / `mark_at` rounded up to a whole second when set, load bounds in ticks with one second of slack
(a save between the fire tick and `cb_tick` loads). 2 Marker offset: its own draw, bearing then a uniform length (the
mission offset has a fixed length); `CbSpot::heard_mm` keeps the distance used; T140b is now the mean miss 3-12 % of
it (a fixed-length draw or a marker on the gun fails), the worst miss a report (T140d). 3 Own statics and own vehicle
frames stop own-fire fragments without damage (F3 test). 4 `cb_rounds` by mode; T134's metric counts no round twice.
5 `ff_lift`: 12 bombs, the squad starts 90 m off; T142c requires the control clean in at most 50 % of runs. 6 Own-fire
death marked only on the killing hit; mortar or gun from the burst's class. 7 No unit test compares with `<BASE>`
(`neutral.py` does). 8 S2 in `fire_targets`, S1 walks squads, A6 gated by `ff_safe`, the lift before the mortar and
gun steps; contracts say "find by name". 9 Build B chunk 2: fortify run first in the background, three 40 sets only.
10 `S11a2Counters` / `PlayerState::s11a2`. Also: the mask note is gone (Build C fix `afd7891` reads the last fifth).

## Plan (2026-11-17)
Scope (spec 0, 1): two of the S11a1 cut items, sized for two Build stages. (1) Friendly fire of fire-mission rounds on
own men (Q22): blast, suppression and fragments; own-fire deaths are not kills; behind `friendly_fire`. (2)
Counter-battery by sound (D 4.3): evidence per firing spot, a marker after 3 rounds at 15 % of the listener distance,
sharpening to 5 %, decay 1 per 300 s, markers count as known ground; behind `counter_battery`. AI: one own-men
predicate for the order filter and the lift of every own mission (`ff_safe`), and counter-fire on a marker no wider
than 60 m, once per new evidence (`counter_fire`). Rows `ff`, `cb` (with the answer), `ff_lift`; JSON `s11a2`.
Left out, with the reason (spec 1, 12): crates and AI carrying (a 20 m walk inside the base while guns never leave
it; one crate = one M1 round; goes with tows and siting); displacement and tows; registration; flash; sound evidence
from mortars; defensive fire and the assault hold; HEAT; per-faction delays; friendly fire on own vehicles and works.
Decisions taken for the owner (all (A), `docs/TODO.md` S11a2-1..9): this scope; own men only; own-fire deaths are not
kills; 50 m / 100 m / 70 m safety distances; infantry guns only leave evidence; counter-fire before observed targets;
T134 counts counter-fire with observed rounds; base rearm stays until guns leave the base; T142 hard.
Honest expectation: counter-fire is map fire (floor 50 m) on a marker 40-120 m wide at 800 m, so the answer will
rarely kill a crew; T141 and T148 report it. The balance reading of friendly fire is default against `ff_off` on the
same seeds (spec 8).
Stage split: A (SAVE 28 layout, keys, F1-F4, K1-K4; every switch 0, neutral by default) -> B (chunk 1: JSON, rows
`ff` and `cb`, S1-S3, friendly fire and markers on; chunk 2: R1-R3, row `ff_lift`, the sets; fallback: stop after
chunk 1 with `counter_fire` 0).

## Measure and docs (2026-11-17)
Measure: `target/s11a2-measure/measure.txt` at `e149258` (480 matches: seeds 1-80, 101-180, 201-280; shared with S11a1).
Germany pooled **41.67 %** (sets 46.25 / 38.12 / 40.62; S10c-ai 40.00 %), no winner 0, inside the 35-65 % band; HE share
21.4 / 19.8 / 20.6 %. Skirmish targets: 318 pass, 82 miss (44 hard), 4 unexposed, 4 control-same, 181 skipped.
S11a2 row missed: T148d US counter-fire 62.76 % of its gun rounds (report bound 50). New hard misses against S10c-ai:
T9b, T54a, T54c, T81a, T81b, T119b; gone: T8a, T115d, T121d. The artillery may explain the vehicle and cover misses (S11a2-11,
S11a2-12; cause not found).
Checks: `tools/check.sh quick 730e67f` all ok, 982 tests at the head. CI on `affb26e` (S11a1 complete plus S11a2 stage A):
Linux, release and debug sim tests, simulate, clippy, rustfmt, cargo-deny passed; Windows and THIRD_PARTY_NOTICES fail as on
main (PR 1); macOS cancelled; cross-platform hash check skipped. CI on `e149258` still running when written. `tools/verify.sh`
NOT run (its tick check, item A-5, is open with it). Owner questions: S11a2-1..12 in `docs/TODO.md`.
