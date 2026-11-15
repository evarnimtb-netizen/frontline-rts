# S10b-ai log (the AI uses mines, obstacles, the bunker; at most 150 lines)

Spec: common `docs/slices/S10b-ai.md`, stages `S10b-ai-build-A.md` ... `-D.md`; targets `targets/S10b-ai.toml`.
Branch `s10b-ai` from main `72b2970`. Another agent may work in this checkout: commit only your files
(`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)
**S10b-ai final state (2026-11-15): done and verified on branch `s10b-ai` (head `d42c8fe`, then docs commits). NOT merged;
main stays `72b2970`.** SAVE 25, PROTOCOL 14 (main 24 and 14). Stages: Plan, Tools, Build A-E, 480 measure, three-lens Opus
review (24 findings: 1 blocker, 10 major, 13 minor), Fix in two passes (22 closed). The blocker: a normal save failed to load
after a sweeping or demolition squad (or its tank) was lost before the AI's next plan. Fixed.
**Verify** (orchestrator, `tools/verify.sh target/s10b-ai-verify`, first parallel run, 248 min 52 s wall, cause not
examined): 965 tests pass, 0 fail; check full all ok; settings.cfg unchanged; tick p99 0.506 ms (peak 0.709). Germany pooled
41.88 % of 480 (43.75 / 41.25 / 40.62), no-winner 0 (main 41.67 %). All switches off: 40 set same 40/40 as main
(`tools/neutral.py` vs `target/s10b-verify`). Targets: 410 pass, 46 miss, 26 hard, 6 unexposed.
**AI use (480, DE / US):** mines 27/22/30 % and 15/12/12 % (target 50); obstacles 12/8/4 % and 4/7/4 % (target 30, DE);
bunker 9/4/4 % and 0/1/1 % of all matches (target 20, DE; finished when started 19/9/8 %); AT fields at most 0.05 per
match (target 0.5); own-side mistakes 0; sweep and demolition never wanted (covered by fortify rows only).
**Fortify rows (10 seeds):** T110i 100 (ctl 0); T111i 85; T111j 0.9 (ctl sweep off 6.3, lane_file off 3.1); T112i 100;
T113e/f/g 100; T115e 0 (ctl 80); T116a 0 (ctl 75); T116b 0 (ctl 53.6); T107d 83 (was 0, fixed); T114e 0 and T114f 0
(one satchel per engineer squad; 100 / 100 with three staged).
**Hard misses (26):** T8b, T40c, T54e, T88b, T88d; near T6a (62.5 vs 62), T40a, T72a (58.75 vs 60); AI use T109a-h,
T110a-d, T112a, T112c, T113a, T113c; T114e, T114f.
**Left open (not done):** finding 23 (tightening `DriveCtx::piece_near` broke `vehicle_on_obstacle_nodes_drives_out`,
reverted); finding 12 (per-reason counter for mine wants, needs a save layout change); the liveness barrier row does not
exercise `barrier_job_max_s` (finding 14, second half); off-lattice crossing for the second AT row (finding 7; T110k
report-only); satchels for T114e/f (TODO E3).
**Waits for the owner:** OK to merge into main; TODO questions S10b-ai-E1 and E3-E8; the 2026-11-12 audit process
proposals (adopted for this slice, not yet confirmed); the 249-minute parallel verify needs a look. Next after the OK:
merge, then the balance pass or S11 (owner's choice).

## Plan (2026-11-12)

Spec (scope, switches, data model, rules R1-R14 and L1-L5, job table, save rules, measures, done-when), four stage
files, targets (S10b body; T106b report; T108 on the all-off control; new T109-T118 in three-part form).
Facts used: own mines never hurt or trigger own units (S10b R7), so mines need no own-route rule and own-mine path
avoidance is pointless under S10b-1 (A); obstacles block every vehicle, so they get the own-route rule (R6);
`build_id` and `content_hash` already fence net peers, so the path rule needs no PROTOCOL bump; the AI's fort job
(Load, Go, Order, Watch) already carries the S10a liveness pieces (F13 stall, `REFUSED_S` parking), so the new
works reuse it as new `WorkKind`s and only sweep and demolish are new job types.
Decided by default (A): table `[ai.obstacles]`; own 3 % S10b materials budget; `jobs_max` 1 inside `builders_max`;
no card weight, price or buy change (the AI already buys Pioniere and US Engineers, who lay and sweep); no urgent
buys for sweepers or demolishers; vehicles avoid AT mines only, men AP only; F8 last and droppable (switch 0 if cut);
two 40-match control runs (`skirmish40_off`, `skirmish40_loose`) in `measure.sh`.
Moved to later slices: own-mine avoidance (with S10b-1 (B)); per-man mine paths; chokepoint analysis; tank or
mortar fire at obstacles and mines; presenter work. Owner questions S10b-ai-1..7 (spec 10).

## Build A chunks 1-2 + code map (2026-11-12, layout `0ae000f`, `30c6a34`, `252f9be`; shortened)
Refusal helpers take ticks; `MineBlast` sorted by tick on load; `MAX_DEMOLISH_PIECES` 3. CLI: `skirmish.rs`
`S10B_AI`, `json_out.rs` `s10b_ai_blocks`, `targets.rs` `exposure`/`control`/`three_parts_from`; `tools/measure.sh`
plays every `skirmish40_*` run; `tools/ablate.sh` `ABLATIONS=DIR`. Liveness (`s10b_ai_tests.rs`): `LIVENESS` rows,
`job_alive`, `drive_row`, `check_liveness`.

## Build B chunks 1-2 (2026-11-12, shortened)
Chunk 1: mine paths (R11), mine layer, blasts, sim counters, `s10b_ai.paths` row, fuzz stage (`path.rs` `MineCost`,
`fort.rs` `MineSet`/`MineLayer` (not saved), `game.rs` `mines_of`). Chunk 2: S10b jobs reuse `AiFortJob` and the
S10a phases; the AT site memo lives on `Planner` (not saved); the S10b budget is derived from state; owner question
B2a (one AT field = one 40 m row) since decided (TODO).

## Build C chunks 2-3 (2026-11-12, shortened)
R6 for a gun barrier uses the map point nearest the gun, mirror = line moved by its length away from the route;
hedgehog rows run along the trench from the anchor; `own_route_m` 0 = R6 off; `own_blocked` counted per new
(vehicle, piece) pair; liveness LogBarrier/Bunker rows refuse by an own tank on the site; fortify gun 60 m aside.
Bunker first among S10b wants; build_structure-for-heavy for the bunker only; fortify bunker row gives a full carry
plus three 40 kg crates; manning via `DriveArgs::others`; US set keys `us_*` (T113g).

## Build D chunks 1-2 (2026-11-12; orchestrator items; sweeping without row and fuzz stage)
Bunker jobs died one plan after the pit (Watch, `Wait`, carried 0, no truck in `Load`): fixed by waiting; the rest is
scarcity (S10b-ai-D1). Controls seen failing: `heavy` false; `#[serde(skip)]` on `blocked_pairs`. Germany 45.0 % (40
set). Bugs: `squad_of` lacked `ClearMines`; a lane of `lane_max_m` refused by rounding; a clamped lane end bent it.

## Build D chunks 3-7 (2026-11-12; shortened)
Sweep row: field rows across the rifle line 60/56 m short of the point, the blast 60 m short. **Owner questions**
S10b-ai-D2 (T111j: a swept lane is not a sure lane; proceeding on (A) leave as is) and D3 (the vehicle block layer
frees a tank node only ~24 m between piece centres; engineers carry 1 satchel, so the AI never opens a 4 m hedgehog
row; proceeding on (A)). Bugs fixed: `squad_of` lacked `ClearMines`/`Demolish`; the satchel man stuck behind the
tank; "exhausted" read from the nav goal; chunk 7 (S): the satchel man's arrival sought cover (`173ccd8`).
Demolish row: a ring, not the spec's line to the map edge; leader satchels 3 (staged). Traces not kept.

## Fix save (2026-11-14; review findings 1, 2, 3, 22, 24 fixed; 4 not done)
Commits `97e8255` + `0e9d0bd` (1: `validate_s10b` as `validate_fort`: a key squad gone since the plan loads, the
demolition vehicle is any entity no infantry squad holds), `1592e22` (2: Charge needs `pieces[done]` on load;
`run_demolish` ends Failed without it), `00cf868` (3: S10b budget saturates), `d38d62c` (22: `demolish_piece`
counts `own_pieces_demolished`; the satchel overpressure already spares own boxes), `c1f8332` (24: divert only when
`waiting_work` is the bunker's and the new `run_kg` (shared with the Load branch) passes; no saved state). Each test
seen failing first (24: 121 sends with the S10b budget at 0). `ai_busy`'s demolition lists a second piece.
Not done: finding 4 (fuzz stage extension: Done sweep, Repath demolition, dead sweeper, wrecked tank, AtRow2 job,
refused and blasts at 32); the new load cases are unit tests in `s10b_ai_tests.rs` only. Residual for 24: cargo
loaded at base after arrival could still shrink the room (not seen). quick 72b2970 all ok (808 passed); all-off 40
`target/s10b-fixsave-off40`: neutral.py vs `target/s10b-base40` same 40/40.

## Fix behaviour (2026-11-14; review findings 6+16, 15, 21, 23 done; 17, 18, 19, 20 not done)
`874a8f5` (6+16: `count_own_blocked` counts a (vehicle, piece) pair only when `gun_route` from the lead to
`nav.goal` finds no way; a Stop or target halt beside an own piece is no block; test
`own_blocked_ignores_a_halt_beside_an_own_piece` read 1 before). The measured own_side_mistakes 1 in oos and s201
(Germany) were false positives: replayed at `874a8f5` (`target/s10b-fixbeh-ob`) both sets read 0, Germany's shares
unchanged (41.25 / 41.25 %, mines 22 / 30, obstacles 8 / 5, bunker 9 / 8). `29a2760` (15: side 1 of a hedgehog row
starts one step beyond side 0's first site; test `hedgehog_row_keeps_its_mirrored_side`). `6212c31` (21: a base
TruckGoal give-up ages out after `refused_s`, the job's own base memory included, via `base_refused`; HQ/drop
refusals keep until-a-load). `85d9596` (23: test `intact_row_holds_a_grazing_approach` passes on the code; tightening
piece_near with "blocked samples may not grow" breaks `vehicle_on_obstacle_nodes_drives_out`, so not done).
17 tried: charge laid 30 cm into the box from the face (toward the centre): fortify slit_satchel_losses_pct still 0
(demolish row unchanged), reverted. Needs a trace of where the man stands and the blast origin vs the slit. 18, 19,
20 not started (turns). quick 72b2970 all ok (812 passed); all-off 40 `target/s10b-fixbeh-off40`: neutral.py vs
`target/s10b-base40` same 40/40.

## Fix measures (2026-11-14; review findings 5, 7, 8, 9, 10, 13, 14 done, 11 note; 12 left)
`c5d872f` (5: obstacle row's R6 pair puts the gun on the point's axis, no enemy tank; control builds across the way
out and its tank, sent to the piece nearest point -> next point, is blocked; the run's R6 refuses (6 rejects seed 1)
and its tank reaches the spot; test `obstacle_r6_blocks_only_the_control`, `R6_SIDE_M=60` = old row, run == control).
`f0f8147` T110k (7: at_rows 2 vs 1 is report only; crossings stay on the 4 m lattice; no off-lattice crossing built).
`241899c` (8: `matches_with_bunker_pct` over all matches, `bunker_done_when_started_pct` = old rate,
`bunker_ready_matches` = exposure; control reads 0). `a0bd8dc` T116c/d exposures swapped (9). `aeb2a59` (10: targets print
CONTROL-MISSING / EXPOSURE-MISSING and fail; demolish control writes `pieces_used`). `6753cf4` T117a-d report (11).
`b73f2d1` (13: tests say the 3 satchels are staged). `36ba68a` (14: loose `barrier_job_max_s` 36000; docs).
Not done: 12 (a lay_why counter would live in the saved AI counters); the LIVENESS barrier row still refuses at once
(14, second half). No sim code changed, so no all-off set was played. Fortify and skirmish sets not replayed.

## Fix2 lane (2026-11-14; review findings 18, 19, 20 done)
`61ff1b7` (18: `lane_file` counts a man whose path ended at the spot beyond the lane end as past, at the lane start
as in it, wherever arrival cover put him; a squad files at most 3 file walks + 60 s after the sweep). `a245cf4` (19:
`sweep_continue` drops a Done lane with a known enemy mine or an enemy blast later than its `since` inside it;
`sweep_lane`'s `covered` ignores a Done lane for a blast later than its sweep; so a new sweep starts and the
attackers hold, no filing; test `ai_swept_lane_is_checked_again`; `ai_two_sweepers_never_share_lane`'s run now
blasts fresh only until the first sweep ends). `dd8b6c8` (20: `at_point_count` = own AT mines + works + live jobs
within the field radius; row 2 wanted only while under 1.5 rows; test `ai_at_field_lays_no_third_row_when_row_two_moved`,
cap and budget lifted, pre-fix 21 -> 31). Fortify10 (`target/s10b-fix2lane-fort`): attacker_mine_losses 0.9,
controls sweep off 6.3, lane_file_off 3.1 (unchanged vs Build E); targets 61 pass, 9 miss (hard 3: T107d, T114e/f;
T115e now passes, own_blocked 0, control 80). All-off 40 `target/s10b-fix2lane-off40`: same 40/40. quick
72b2970 all ok (819 passed).

## Fix2 slit-fuzz (2026-11-15; review findings 17, 4 done)
`a05dcb3` (17, trace: the five were not the issue; the Demolish man's stand spot was 1.5 m out from the box point
nearest *him*, so from the engineers' line 15 m out the roof's corner won and the charge lay at (2.1, 1.85) m,
1.45 m beside the slit; pre-173ccd8 arrival cover moved him into the pit, which hid it. Now a click outside the box
sends him out from the box point nearest the *click* (a click on the box: as before; the AI clicks piece centres, so
AI play is unchanged); seek_cover not restored, satchel damage unchanged. Seeds 1-10 both owners: slit 83/100 dead
(was 0), back wall 0/100, uphill 0; test `demolish_clicked_at_slit_lays_charge_in_slit` (old spot: charge 1.85 m off
the slit, seen) and `bunker_row_fills_every_field` slit >= 40; `demolish_man_places_the_second_piece` green).
`8e5daf9` (4: `staged_s10b_ai` adds a Done sweep, a sweep of a squad wiped out and a demolition of a tank wrecked
just before the save, a Repath demolition, an AtRow2 job, refusals of every kind and blasts at 32; 7 new
STAGED_MUST_HOLD keys; control (no kill/wreck) seen failing; mutation fuzz 10399, 0 panics). Not done from 4: the
`done = 3` / `mines_laid = u32::MAX` load cases (unit tests in `s10b_ai_tests.rs` cover them since Fix save).
quick 72b2970 all ok (820 passed); all-off 40 `target/s10b-fix2slit-off40`: neutral.py same 40/40. Fortify not replayed.

