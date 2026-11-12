# S10b-ai Build D: sweeping, demolition, the truck loop F8, docs

Read first: `S10b-ai.md` 4 (R9, R10, R13, R14, L1-L5), 5 (Sweep, Demolish, Truck rows); the log's top block.
**3 chunks**, about 30 self-counted turns each. No layout change. Work rules as in A (outline and show, targeted
release tests via `tools/run.sh`, `tools/check.sh quick 72b2970` once per chunk end).

Start from: `crates/sim/src/ai/breach.rs` (`breach_start`, `breach_continue`: the want/phase pattern to copy);
`crates/sim/src/sweep.rs` (`order_clear_mines`, `sweep_tick`); `crates/sim/src/demolish.rs` (`order_demolish`,
`demolish_spot`); `crates/sim/src/ai/truck.rs` (`AiTruck::give_up`, `gave_up_on`); `crates/sim/src/ai.rs` (`plan`:
where `breach_continue` / `breach_start` run; `vehicle_tasks`, `MyTank`).

## Chunk 1: sweeping (R9, R10)
1. `ai/sweep.rs` (new): `sweep_continue` beside `breach_continue`, `sweep_start` after `breach_start` (claimed squads
   as breaches). Suspects from `FortState::blasts` (enemy-owned mines that hurt own units, younger than `suspect_s`;
   `suspects_seen` cursor). Want per non-own point with assigned attackers (R10); lane `a` 10 m short of the first
   suspect or known mine on the line, `b` = `a` + `lane_max_m` toward the point, clamped; `sweep_wanted` += 1 per want.
2. Phases Go (walk to `a`) -> Order (`ClearMines`) -> Watch (the order stands; progress mark `along`) -> Done. Ends:
   order gone and lane clear (`sweeps_done`), sweeper dead or below 60 %, point no longer attacked, L1, L2; failure
   -> L3 (kind Sweep, at = lane middle). L4 before every start. Attackers of that point hold short of `a` while the
   sweep runs (their task target), then go on; one sweep per lane, `MAX_SWEEPS`.
3. Liveness table: add Sweep (lane over the map edge: the command is refused).
Tests: `ai_marks_suspect_from_blast` (control `suspect_s` 0), `ai_sweeps_suspect_lane_before_attack` (control `sweep` 0),
job-table rows: `ai_sweep_ends_when_lane_clear`, `ai_two_sweepers_never_share_lane`, `ai_sweep_ends_when_sweeper_dies`,
`ai_sweep_beside_own_tank` (each names its control in the test doc line).
Fortify row `s10b_ai.sweep`: an AI attack force (rifle squad and an engineer squad) on a point behind a two-row enemy AP
field across the approach, one blast already recorded: `drive_ai_habit(Sweep)` then the scripted advance:
`lane_swept_pct`, `mines_lifted`, `attacker_mine_losses`; exposure `sweep_wanted_pct` (must be 100 by staging);
control `sweep` 0: `control.attacker_mine_losses` (must be higher, i.e. miss the row's max).
Fuzz stage: **"fuzz stage: staged_s10b_ai + an AiSweep in Watch mid-lane with one mine lifted"**.
**STOP POINT 1.** Quick green; commit; rewrite the log's top block; return.

## Chunk 2: demolition (R13, R14)
4. `ai/demolish.rs` (new): want = an own AI vehicle with a task (escort, assault, gun tow) whose path is exhausted or
   whose move failed twice (`demolish_wanted` += 1), and a piece of `known_obstacles` within `demolish_near_m`. Squad:
   own `demolish` squad with a satchel man, nearest within `sweep_reach_m`; none: no job (no buy).
5. Pieces: the one nearest the crossing of the line vehicle -> goal, then neighbours alternating sides, up to
   `demolish_pieces_max`; one `Demolish` per piece; after each burst the vehicle's path is requested again; its next
   result reaching the goal ends the job `opened` (`gaps_opened`); else the next piece; after the last
   `demolish_given_up` and L3 (kind Demolish, at = the crossing). Vehicle waits at its spot (no re-path spam: the
   existing `repath_s`). One job per vehicle; a gone piece is skipped.
6. Liveness table: add Demolish (no satchels left: refused at every try).
Tests: `ai_demolishes_until_a_tank_gap_opens` (hedgehog line: opens after 2-3 pieces; control
`demolish_pieces_max` 1 -> `gave_up`, the T106b fact), `ai_demolish_skips_gone_piece`, `ai_one_demolition_per_vehicle`,
`ai_demolish_ends_when_vehicle_or_squad_lost`, `ai_demolish_charge_spares_own_tank`.
Fortify row `s10b_ai.demolish`: a tank with no way round (a hedgehog line from steep ground to the map edge), an AI
Pioniere squad, `drive_ai_habit(Demolish)`: `gap_opened_pct`, `tank_passed_pct`, `pieces_used` (report); exposure
`tank_blocked_pct` (path exhausted at start, 100 by staging); control `demolish` 0 (`control.tank_passed_pct` 0).
Fuzz stage: **"fuzz stage: staged_s10b_ai + an AiDemolish with one piece done and a charge burning"**.
**STOP POINT 2.** Quick green; commit; rewrite the log's top block; return.

## Chunk 3: F8 (L5), docs, the stage check
7. `ai/truck.rs`: with `[ai.truck] refusal_memory` 1, `give_up` also calls `AiState::refuse(TruckGoal, at,
   tick)` and every goal choice checks `refused_near`; with 0 nothing changes (all-off neutral). If this
   does not fit the chunk, leave it: log "F8 deferred", keep the switch at 0 in `skirmish.toml`.
   Tests: `truck_never_reorders_unreachable_hq` (control `refusal_memory` 0 -> re-order within 6 min, the F8 fact),
   `truck_hq_gone_ends_goal`, `truck_refusal_survives_save_load`; liveness table: add TruckGoal.
8. Docs: `docs/modding/tuning.md` (`[ai.obstacles]`, the `[fortify]` path keys, `refusal_memory`),
   `docs/modding/fortifications.md` section "AI (S10b-ai)", `docs/STATE.md` row.
9. `tools/run.sh` `tools/check.sh tests` (background, all release tests incl. ignored: fuzz with the staged save);
   `frontline-cli targets docs/slices/targets/S10b-ai.toml --runs fortify10` (T109-T118 fortify rows read);
   40 set on, all-off 40 set same 40/40.
**STOP POINT 3.** Commit; rewrite the log's top block (every number, open items, what the orchestrator runs: measure
480, `ABLATIONS=docs/slices/targets/S10b-ai-ablations tools/ablate.sh`); return.
