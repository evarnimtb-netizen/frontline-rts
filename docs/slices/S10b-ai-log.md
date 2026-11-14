# S10b-ai log (the AI uses mines, obstacles, the bunker; at most 150 lines)

Spec: common `docs/slices/S10b-ai.md`, stages `S10b-ai-build-A.md` ... `-D.md`; targets `targets/S10b-ai.toml`.
Branch `s10b-ai` from main `72b2970`. Another agent may work in this checkout: commit only your files
(`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)
**Build E items 1-8 done (STOP POINT 3; E3 hand-over not built, kept open).** SAVE 25, PROTOCOL 14. Commits: item 6
`77a3a13` (ablations `lane_file_off`, `at_rows_off` (1), `bunker_assign_off`, `bunker_truck_first_off`,
`barrier_job_max_s_off` (900); `all_off.toml` has them; `tools/ablate.sh` check_file knows each key's off value and
the dry-run counts any `N -> M`; `docs/modding/tuning.md` Build E row); item 7 `08f81c8` (fix: `json_out::fortify`
now writes `s10b_ai`, so T110i-T116b read numbers; before: "no such number"); item 8 `d97782b` (TODO: B2a, C1,
D1, D2, D3 decided; open E1, E3, E4 mine-row key, E5 wire debris). quick 72b2970 all ok, 803 passed.
**Stage check (item 7), head d97782b:** all-off 40 `target/s10b-e3-off40`: neutral.py vs `target/s10b-base40` same
40/40. Default 40 `target/s10b-e3-on40` (text `skirmish40.txt`): Germany 21/40 = 52.5 %. s10b used: DE mines_laid
130, obstacles_built 9, bunkers_built 3, sweeps 0, demolitions 0; US mines_laid 91, obstacles 2, bunkers 0, sweeps
0, demolitions 0. s10b-ai: DE mines in 22 % (3.25 laid/match), obstacles 12 %, bunker 12 %; US mines 20 %, obstacles
5 %, bunker 0 %; sweep/demolish when wanted "-" (never wanted). DE bunker why: no_builder 7045, point_not_ready 7712.
fortify10 targets (`target/s10b-e3-fort.log`): 59 pass, 10 miss (hard 4). Pass: T110i 100 (ctl 0), T110j 100,
T111i 85, T111j 0.9 (ctl 6.3), T112i barrier 100 (ctl 0), T113e/f/g 100, T116a/b 0 (ctl 75/53.6).
Hard misses: T114e gap_opened 0 and T114f tank_passed 0 (pieces_used 1: one satchel per engineer squad, a 4 m row
needs 2 pieces = E3); T115e own_blocked 20 with control (own_route_m 0) also 20 (not looked at); T107d S10b
slit_satchel_losses_pct 0 (min 40; S10b mechanics row, not looked at). Report misses: T93a/b/f/g/h, T106b.
**E3 (orchestrator (b)) not built** (turn budget): plan if wanted: in `ai/demolish.rs` `demolish_continue`, at
`DemolishPhase::Go` with the squad's satchels (sum of `ents.kit[m].items[ItemKind::Satchel]`) 0, move the job
(`ai.demolitions` BTreeMap key) to the nearest free `Mine` with `Capability::Demolish` and a satchel within
`sweep_reach_m`; a control needs a new `[ai.obstacles]` key (canonical snapshot + resolved_defs refresh).
**Open:** owner questions E1, E3, E4, E5 (TODO); bunker 12 % DE / 0 % US (target 15 %); T114e/f, T115e, T107d.

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

## Build A chunks 1-2 (2026-11-12, layout `0ae000f`, `30c6a34`, `252f9be`)

Default decisions: refusal helpers take ticks; `MineBlast` sorted by tick on load; `MAX_DEMOLISH_PIECES` 3; staged
checks in `staged_s10b_save_holds_its_state`; fuzz plan asserts entries (`.sweeps.{}` ...). The three "chance"
exposures come from AI state only; `s10b_materials_mp_pct` prices S10b grams at `construction_mp_per_t`.

## Build A code map (short)

CLI: `skirmish.rs` `S10B_AI` names, `S10bAiSeen` chances (lay/obst/bunker from AI state, 0 with switches off);
`json_out.rs` `s10b_ai_blocks` -> `factions.<f>.s10b_ai` + `s10b_ai_derived`; `targets.rs` `exposure`, `exposure_min`,
`control`, `three_parts_from`, verdicts `Unexposed`/`ControlSame`. `tools/measure.sh` plays every `skirmish40_*` run;
`tools/ablate.sh` `ABLATIONS=DIR` (8 `*_off.toml`; `loose.toml` is the control); `tools/pool.py` "s10b-ai:" line.
Liveness (`s10b_ai_tests.rs`): `Stage`, `LIVENESS` rows (Fort x3, Sweep), `job_alive` (TruckGoal false: D fills it),
`drive_row`, `check_liveness`. Keep: `refused_near(.., refused_ticks, near_m)` takes ticks.

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

## Build D chunk 3 (2026-11-12; stage chunk 1 rest: fortify row `s10b_ai.sweep`)

Row geometry as the sweep tests: the line runs through the rifle leader; field rows across it 60 m and 56 m short of
the point (30 m, every 2 m, row 2 shifted 1 m), the blast 60 m short, 1 s old. Debug trace (not committed): with the
sweep, 4 of 6 runs lose 0 men; the losing runs had a second blast inside the lane (a missed mine), then blasts 2-5 m
aside. **Owner question S10b-ai-D2** (T111j, max 1 man per run, "verify"; measured 3.0, control 8.0): (A) leave as is,
a swept lane is not a sure lane (the S10b detection roll); (B) the AI sweeps two parallel lanes or a lane of
`lane_half_cm` 300 for an attack (more sweep time); (C) attackers wait for a second pass over a lane where a mine was
found. Proceeding on (A). Quick after `078e382`: fmt, clippy, neutral.py, ablate.sh ok; tests 786 passed (306 s).

## Build D chunk 4 (2026-11-12; stage chunk 2 without the fortify row)

Bugs met: `squad_of` lacked `Demolish` (the job saw its pending order as refused); the satchel man stuck behind the
waiting tank (the squad now stands 4 m out and 4 m aside); after a failed move the tank is Holding with an empty path,
so "exhausted" is read from the nav goal too. **Owner question S10b-ai-D3:** the vehicle block layer (footprint +
`wreck_pad_m` + clearance) frees a tank node only ~24 m between standing piece centres (measured on a 4 m ring: 1-4
gone blocked, 5 open); `demolish_pieces_max` is at most 3 (layout cap) and engineers carry 1 satchel, so the AI never
opens a 4 m hedgehog row. (A) leave as is (T114e/f miss; L3 stops retries); (B) a later slice narrows the piece pad;
(C) more satchels, higher cap (layout, later slice). Proceeding on (A); tests use a 6 m ring (opens after 3 pieces).

## Build D chunk 5 (2026-11-12; stage chunk 2 rest: fortify row `s10b_ai.demolish`, STOP POINT 2)

Ring, not the spec's line to the map edge (closed on any map); leader satchels 3 (staged); the 60 s on keeps driving
the habit. Faults (1), (2) found with a scratch per-5 s trace. Chunk 6 (P, Q): scratch `sample` profile, per-10 s tank
nav and `outcome()` trace. Chunk 7 (R, S): scratch per-1 s satchel-man nav trace (his Arriving goal jumped to cover).
Controls seen failing: P flag ignored; Q usual drain; R the old Repath rule; S the pre-fix `unit.rs`. (No traces kept.)

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
