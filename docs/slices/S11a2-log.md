# S11a2 log (friendly fire from indirect HE, counter-battery by sound, AI safety and counter-fire; at most 150 lines)
Spec: common `docs/slices/S11a2.md`, stages `S11a2-build-A.md`, `-B.md`; targets `targets/S11a2.toml`; ablations
`targets/S11a2-ablations/`. Branch `claude/project-thread-e7nsak`, base `<BASE>` = `b74ed58` (the S11a1 head when Build A
started; neutral base set `target/s11a2-base40`). Another agent may work in this checkout: commit
only your files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`, `target/`,
`tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: Build A done 2026-11-17, then the S11a1 review fixes C-1..C-7 (`8cb2c4d`..`cddb069`, "S11a1 fix" commits; S11a1
log "Fix"). Next: Build B (`S11a2-build-B.md`).**
- **Neutral base for Build B: `target/s11a2-base40-fix`** (default, seeds 1-20 both bases, head `cddb069`), not
  `s11a2-base40` / A40: the fixes change AI play (same 36/40 against it). Germany 15 of 40 (37.5 %); AI gun missions
  Germany 246 (6.2 a match; 238 observed, 7 map), USA 130 (3.3; 124 observed, 5 map). Check: 961 tests, all ok.
- SAVE 28, PROTOCOL 14. All four switches 0 in `skirmish.toml`. Keys and ranges as spec 2 (absent keys: the defaults).
- Code map: `counter_battery.rs` (`CbSpot`, `MAX_CB_SPOTS`, `cb_note` K1+K3 sharpen, `cb_tick` K2+K3 decay, `near_own_marker`
  K4, `nearest_soldier`, `whole_second`, `validate_spots`); `cb_note` called in `gunnery.rs` `gun_mission_tick` after
  `gun_rounds`; `cb_tick` in `world.rs` beside `update_intel`. `Burst::ff` set in `combat.rs` `apply_blows` (the shell
  burst; false in cook-off and item bursts); scratch lists `DamageCtx::{ff_hit, ff_dead}`, `hurt_by`, kill loop skips
  `ff_dead`. `Mission::cb` set in `game.rs` `order_gun_mission`; `cb_bursts`/`cb_near_bursts`/`mortar_near_own_bursts` in
  `fire_mission.rs` `observe_bursts` (live missions only).
- Reading state: `w.game().unwrap().players()[p].cb` (markers: `marked`, `at`, `err_mm`, `heard_mm`, `shots`) and `.s11a2`
  (`S11a2Counters`); AI counters in `ai.counters` (`ff_lifts` ... `cb_masked`, all 0 in A). Switches on: setup edit
  (`s.rules.fire_mission.as_mut().unwrap().friendly_fire = 1`, `s.rules.ai.arty.ff_safe = 1`) or an override file.
- Test helpers (`s11a2_tests.rs`): `mission_scene` (mortar Direct on an own/enemy squad 300 m off), `cb_scene` (leIG 18 +
  listener 800 m), `map_fire`, `burst_now` (one burst dealt as the Damage stage would), `lone`, `pose`, `note`, `cb_ticks`.
- S1 for Build B: the planner reads `self.g.squads` (owner, `nav.goal` via `sq.nav`, lead `members[0]` transform); look
  at `ai/arty.rs` `run_arty` and `ai/mortar.rs` `fire_targets` by name. `Stop` (`game.rs` `order_stop`) sets a gun or mortar
  squad's order to `Idle` the tick it applies (AI commands wait their reaction delay first); rounds already in the
  air still burst and count (test `bombs_in_the_air_still_hurt_after_the_tube_dies`).
- Not done: nothing of A. Build B: M1-M3, S1-S3, R1-R3, switches on, sets. `ff` on sanity 40 set: below (old base).
- S11a1 changes B builds on: P1/P3 share `[ai.arty] danger_close_m`; P3 lifts only missions aimed at the focus (C-4);
  observer post reset at most once a `repath_s` (C-3); T131a/b are reports (S11a1-10).

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
