# S10b-ai log (the AI uses mines, obstacles, the bunker; at most 150 lines)

Spec: common `docs/slices/S10b-ai.md`, stages `S10b-ai-build-A.md` ... `-D.md`; targets `targets/S10b-ai.toml`.
Branch `s10b-ai` from main `72b2970`. Another agent may work in this checkout: commit only your files
(`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)
**Next: Build D stage chunk 3, the rest: item 8 docs and item 9 stage check, then STOP POINT 3.** SAVE 25, PROTOCOL 14.
**Item 7 (F8, L5) done** (`fcc5239`, `12e1308`, clippy `HEAD`): `ai/truck.rs` `truck_give_up` (give-up + `AiState::refuse
(TruckGoal)` when `[ai.truck] refusal_memory` 1, `s10b_refused` += 1) and `truck_refused` (the job's ring, or a TruckGoal
refusal within `refused_m`) at every goal choice (base Return, follow spot, stranded tank, drop wants). **Deviation:** a
truck refusal does not age out at `refused_s`; it lives until an own truck finishes a Load (all TruckGoal refusals cleared
there); `drop_stale_refusals` keeps them; `refused_s` 0 still turns the check off. Why: with the spec's 300 s window the
F8 scene re-ordered the HQ at tick 11761, 326 s after the refusal (tick 1980), same as the control (orders 901, 11761).
Tests (sim `tests/trucks.rs`, scene `f8_world`/`f8_run`): `truck_never_reorders_unreachable_hq` (ignored; control memory
0 -> 2 HQ orders, seen), `truck_hq_gone_ends_goal` (HQ at 400,400 reachable; control HQ at 700,700 -> refusal at the 180 s
drop limit, seen), `truck_refusal_survives_save_load` (ignored). Liveness table row `(TruckGoal, truck_row)` in
`s10b_ai_tests.rs`, hook habit `AiHabit::Truck` -> `Planner::drive_truck`; `job_alive` TruckGoal = truck job in `Drop`
(control memory 0 -> "TruckGoal: no refusal", seen). The staged fuzz save already holds a TruckGoal refusal.
**Not yet run after F8:** the 40 sets. `refusal_memory` is 1 in `skirmish.toml`, so the defaults-on 40 set may differ from
`target/s10b-d7-on40` (F8 explains it); the all-off set must stay same 40/40 vs `target/s10b-base40`.
**Item 8 docs:** `docs/modding/tuning.md` (`[ai.obstacles]`, `[fortify]` path keys, `refusal_memory` incl. the "until a
load" rule), `docs/modding/fortifications.md` section "AI (S10b-ai)", a `docs/STATE.md` row (STATE.md at most 150 lines),
owner questions D1 (bunker rare), D2 (sweep lane losses), D3 (tank gap needs 24 m), C1 copied into `docs/TODO.md`.
**Item 9:** `tools/check.sh tests` (background), `frontline-cli targets docs/slices/targets/S10b-ai.toml --runs fortify10`,
40 set on + all-off 40 set (same 40/40); STOP POINT 3; the orchestrator runs measure 480 and `ablate.sh` (S10b-ai-ablations).
Earlier numbers (chunk 7): demolish row gap_opened 50.0, tank_passed 50.0, pieces 3.0; obstacle row drove 100,
own_blocked 0, control 0; 40 sets d7 same 40/40. **Open follow-ups (not done):** `count_own_blocked` (`ai/fort.rs`) misses
vehicles Holding short of the goal (proposed: Holding > 10 m from `nav.goal` with an own blocking piece within
`OWN_BLOCKED_M` ahead); outside this slice: after 2 ring pieces the vehicle path layer serves a gap the tank cannot drive
(stops ~12 m on; D3). Owner questions: D3, D2, D1, C1, B2a, 1..7. Quick after `HEAD~1` all ok (795 passed). Failing: none.

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

## Build A chunk 1 (2026-11-12, layout commit `0ae000f`)

Decided by default: `refused_near` / `drop_stale_refusals` take ticks (no hz in the helper); `MineBlast` list must be
sorted by tick on load (added to spec 6); `AiDemolish::pieces` uses `MAX_DEMOLISH_PIECES` 3; the staged-save checks
went into `staged_s10b_save_holds_its_state` (the seed-match `MUST_HOLD` cannot reach S10b-ai state before Build B);
`fuzz_plan_reaches_s10b_state` asserts entries (`.sweeps.{}`, `.demolitions.{}`, `.refused.[]`, `.blasts.[]`), since
the bare names also match the new counters (`sweeps_started`, `refused_reorders`). `ablate.sh` dry run failed once on a
stale CLI binary (old code rejects `[ai.obstacles]`); green after the rebuild.

## Build A chunk 2 (2026-11-12)

Commits `30c6a34` (fields, targets tool, measure/ablate/pool, ablation files), `252f9be` (liveness harness).
Decided by default: the three "chance" exposures come from AI state only (a nonzero fact-based chance would break
`neutral.py`'s "new keys stay 0" on the defaults run); `s10b_materials_mp_pct` prices the grams S10b works draw at
`construction_mp_per_t`; a null control is never CONTROL-SAME and a control run not played prints "(not played)"
without failing; `ablate.sh` default set moved to S10b-ai (S10a via `ABLATIONS=`).

## Build A code map (short)

CLI: `skirmish.rs` `S10B_AI` names, `S10bAiSeen` chances (lay/obst/bunker from AI state, 0 with switches off);
`json_out.rs` `s10b_ai_blocks` -> `factions.<f>.s10b_ai` + `s10b_ai_derived`; `targets.rs` `exposure`, `exposure_min`,
`control`, `three_parts_from`, verdicts `Unexposed`/`ControlSame`. `tools/measure.sh` plays every `skirmish40_*` run;
`tools/ablate.sh` `ABLATIONS=DIR` (8 `*_off.toml`; `loose.toml` is the control); `tools/pool.py` "s10b-ai:" line.
Liveness (`s10b_ai_tests.rs`): `Stage`, `LIVENESS` rows (Fort x3, Sweep), `job_alive` (TruckGoal false: D fills it),
`drive_row`, `check_liveness`. Keep: `refused_near(.., refused_ticks, near_m)` takes ticks.

## Build B chunk 1 (2026-11-12)

Mine paths (R11), mine layer, blasts, sim counters, `s10b_ai.paths` row, fuzz stage. Deviations: the fuzz stage
stages the blast by edit (stepping would fire the staged boom-tick mines); fortify field names follow the targets
(`foot_mine_losses_pct`, plus `foot_losses`); `at_exposure_ticks` counts distinct enemy vehicles per owner per tick.
B1 code map: `path.rs` `MineCost` (= `WireCost`), `find_foot_path(.., wire, mines, ..)`, `VehiclePathCost::mines`,
`search_with` prices a mine step like wire, `smooth` keeps no unpriced-mine shortcut; `fort.rs` `MineKey`, `MineSet`,
`MineLayer` (`#[serde(skip)]`), `FortState::{scan_mine_set, refresh_mine_layer, mine_set}`; `take_mines` blasts ring,
`lost_on_known`, `own_mine_losses`; `MatchState::at_exposure`; `game.rs` `serve_paths` `mine_rule`/`mined` -> `mines_of`.

## Build B chunk 2 (2026-11-12)

Decided by default: S10b jobs reuse `AiFortJob` and the S10a phases; the per-plan AT site memo lives on `Planner`
(never saved); "one search per point per `recheck_s`" is stateless (only the window's plan offers the AT field);
`lay_sites` counts a field site offered on that plan; the S10b budget is derived from state (mines gone are priced at
the cheapest S10b mine: a lower bound); S10a's budget leaves S10b grams out; spec control "`at_out_m` 0" is out of the
key's range (1..=1000): the test uses 1 (field at the point, not 60 m out); the AT test checks "crosses the approach"
against the straight base-point line (flat map) with the field's ends on both sides of it; `World::set_point_owner`
(staging tool) added for the fortify row. Refusals seen: BadSpot (own tank on the site) in the liveness row and test.
Owner question S10b-ai-B2a: one AI AT field is one 40 m row at the blueprint's 4 m step; a crossing tank sets one off
rarely (fortify 0/10, matches 0). (A) keep the spec's single row; (B) two staggered rows per AT field.

## Build C chunk 2 (2026-11-12; stage chunk 1 rest)

Decided by default: R6 for a gun barrier uses the map point nearest the gun; its mirror is the line moved along itself
by its length to the side farther from the route; the hedgehog row runs from in front of the anchor along the trench
away from its middle (mirror: toward it), R6 checks the whole row once; "same builder" not enforced (`jobs_max` 1 keeps
one job at a time); `own_route_m` 0 turns R6 off (the spec control); `own_blocked` = an own vehicle standing at its
served path's end that is not its goal, own blocking piece within 8 m, pair counted when it first appears (the spec's
control "barrier on the route -> own_blocked > 0" cannot happen on open ground: separate test `ai_counts_own_blocked`
orders a tank onto its own barrier); hedgehog control is "no nest" (`hedgehogs_row` 0 is out of range 1..=16);
liveness LogBarrier row refuses by an own tank on the site (the flat test map has no steep slope); fortify gun 60 m
beside the point, 1500 s drive, 600 kg carried; enemy tank starts 150 m in front of the gun.

## Build C chunk 3 (2026-11-12; stage chunk 2)

Decided by default: the bunker comes first among the S10b wants (one strongpoint; hedgehog rows anchor on it);
build_structure-for-heavy is allowed for the bunker only (R7 names it; hedgehogs keep build_heavy); a 250 kg step needs
a truck or kg at hand (`at_hand`): two carries do not exist on the S10a path (a walk back ends the work); tests and the
fortify row give a full carry plus three 40 kg crates at the site (carried kg are trimmed to the full carry); manning
in the fortify row runs through the Fort hook (`DriveArgs::others`), as the hook has no `order_infantry`; the shelter
rule still comes before the bunker for a rifle crew; bunker row keys `us_*` for the US set (targets T113g); liveness
Bunker row refuses by an own tank on the site (flat map); determinism saves moved to the first live S10b job.

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
