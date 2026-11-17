# S11a2 log (friendly fire from indirect HE, counter-battery by sound, AI safety and counter-fire; at most 150 lines)
Spec: common `docs/slices/S11a2.md`, stages `S11a2-build-A.md`, `-B.md`; targets `targets/S11a2.toml`; ablations
`targets/S11a2-ablations/`. Branch `claude/project-thread-e7nsak`, base `<BASE>` = the S11a1 head when Build A starts
(S11a1 A and B end at `a6c5c72`; Build A writes the real hash here). Another agent may work in this checkout: commit
only your files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`, `target/`,
`tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: Plan done and revised 2026-11-17 (review: 5 major, 5 minor, all verified in the code, none rejected). Nothing built. Next: Build A (`S11a2-build-A.md`), only after S11a1 Build C (and its
fixes, if the orchestrator runs them first) is committed.**
- `<BASE>`: not fixed yet. Build A chunk 0 records it, plays the base 40 set into `target/s11a2-base40/` and runs the
  targets parse check.
- **The targets file and the five override files were NOT parsed by the CLI** (S11a1 Build C was building in this
  checkout; no cargo or CLI run was allowed). Checked instead: Python `tomllib` loads all six; 587 targets (552 of
  S11a1 + 37), no duplicate id, every new row has metric, exposure, exposure_min, control, note and a min or max, and
  uses only keys the S11a1 rows use; the only old rows changed are T134a-d. Build A chunk 0 step 3 is the real check.
- SAVE 27, PROTOCOL 14 now; Build A makes SAVE 28 in one layout commit holding everything Build B needs (spec 3).
- All four switches (`[fire_mission] friendly_fire`, `counter_battery`; `[ai.arty] ff_safe`, `counter_fire`) are 0 through
  Build A, so A is neutral by default; Build B turns them on one commit at a time.
- Read from the code at `a6c5c72`, the AI names re-read at Build C's `25436fa` (line numbers are hints: find by name):
  - own men are skipped in `combat.rs` `burst` (2375) and `fragments` (2594) by `owner == b.player`; a mission round's
    `BallisticEvent::mission` is not 0; the shell burst is built at 3207; kills are credited at 3399.
  - `piece_between(lift, body, b.player)` leaves out the burst owner's pieces: an own victim needs every piece counted.
  - a gun mission round is counted at `gunnery.rs` 1133 (`gun_mission_tick`); `known_by_record` is `fire_mission.rs` 671.
  - the AI's target scorer is `ai/mortar.rs` `fire_targets`, shared by mortars and guns, with a 35 m own-men test; the
    spotter stands 35 m short (`spot_for`); the planner's `own` list is positions only; Build C's A6 lift sits in
    `run_arty` and must be gated by `ff_safe` (S3), or both send `Stop`.
  - `hurt` records a death only on the killing hit; the fragment code skips own statics and own vehicle frames (F3
    makes them blockers for an own-fire burst); the mission aim offset has a fixed length (K2 draws its own).
  - a served gun outside its base is already fed by crates, HQ, dump and truck (`structures.rs` `rearm_forward`); only
    the own-base skip stands between today and crate-fed guns in base (why crates were left to S11a3).
- Depends on S11a1 Build C: `run_arty` and its target order, A5 / reserve holds, A6, `fire_targets`. If P1-P3 are
  never built: R1 sits "before A4", and P1-P3 keep their own contract (`S11a1-build-C.md` chunk 2).

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
