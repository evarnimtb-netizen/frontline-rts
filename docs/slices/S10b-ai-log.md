# S10b-ai log (the AI uses mines, obstacles, the bunker; at most 150 lines)

Spec: common `docs/slices/S10b-ai.md`, stages `S10b-ai-build-A.md` ... `-D.md`; targets `targets/S10b-ai.toml`.
Branch `s10b-ai` from main `72b2970`. Another agent may work in this checkout: commit only your files
(`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)

**Next: Build D chunk 3 = the rest of the stage file's chunk 1 (sweeping: fortify row `s10b_ai.sweep` and the fuzz
stage), then stage chunk 2 (demolition).** D chunk 2 built the AI sweep (`eb20b56` + the clippy/log commit). Layout:
SAVE 25, PROTOCOL 14 (no bump). Start from `crates/sim/src/ai/sweep.rs` (module doc = the rules as built).
**Sweep code:** `sweep_on`, Planner `suspects` (R9: other players' `FortState::blasts` younger than `suspect_s`; the
blast keeps no victim, so every enemy blast counts; sets `suspects_seen` = blasts read), `known_enemy_mines`,
`sweep_lane(p, from, to, suspects, mines, fixed_b)` (R10; lane 10 m short of the first, `lane_max_m` - 10 cm, shortened
to stay on the map; candidates inside an existing lane of the point skipped), `sweep_start` (after `breach_start` in
`plan`, claimed as hunts; sweeper: the point's `clear_mines` squad >= 60 %, else nearest within `sweep_reach_m`; L4;
`refused_reorders`), `sweep_continue` (after `breach_continue`), `run_sweep` (Go: `Move` to `a`, within 8 m sends
`ClearMines`; Sweep: `tries` 0 until the order stands, then 1 + whole m of `along` = L2 mark, stall 3 x `lift_s` + 60 s;
Go stall 3 x `sweep_reach_m`/speed + 60 s; L1 `job_max_s`), `sweep_end` (L3 at lane middle, `s10b_refused`,
`jobs_overdue`), `sweep_hold(p)` (attackers' target `HOLD_M` 10 m short of `a`; used in `order_infantry` via `holds`),
`drive_sweep` (hook: `args.others` = attackers, tasks set Attack each plan, held while alive; `args.at` = fixed `b`).
A swept job stays `SweepPhase::Done` (squad free) while the point is attacked, at most `suspect_s` (one sweep per
lane); `sweep_room` evicts the oldest Done at `MAX_SWEEPS`. `squad_of` now knows `ClearMines` (pending check).
**Tests** (`crates/sim/src/s10b_ai_sweep_tests.rs`, controls in-loop, all seen): `ai_marks_suspect_from_blast`
(`suspect_s` 1), `ai_sweeps_suspect_lane_before_attack` (`sweep` 0), `ai_sweep_ends_when_lane_clear` (`sweep` 0),
`ai_two_sweepers_never_share_lane` (`suspect_s` 1 + fresh blasts -> 2nd sweep), `ai_sweep_ends_when_sweeper_dies`
(one man killed -> lives), `ai_sweep_beside_own_tank` (incl. save/load mid-job, same hash; `sweep` 0). Helpers
`attack_world`, `squads`, `blast`, `Attack::short`. Liveness row `sweep_row` (lane end off the map: BadSpot; control
`refused_s` 0 -> "Sweep: refused_reorders 4", seen). `drive_ai_habit_skeleton_ends_not_built` now checks Demolish only.
**Left in stage chunk 1:** fortify row `s10b_ai.sweep` (sdk/crates/cli/src/fortify/s10b_ai.rs; use `drive_ai_habit(Sweep)`
with `DriveArgs{squad, point, others}` then the scripted advance; exposure `sweep_wanted_pct`; control `sweep` 0) and
fuzz stage "staged_s10b_ai + an AiSweep in Watch mid-lane with one mine lifted" (save_fuzz.rs `staged_s10b_ai`: set
the staged AiSweep's `tries` 13 = along 12 m, `accounts[0].mines_lifted` 1, add a `holds` key + `STAGED_MUST_HOLD`).
Still open from D1: T115e (own tank never moves with obstacles on, `obstacle_once`), slow skirmish binary (now 651 s in
quick; `own_route()` memo per plan, see D1 section), owner question S10b-ai-D1. Owner questions open: D1, C1, B2a, 1..7.
Last check: see the D chunk 2 section. Failing tests: none. Chunks left: D 3 (sweep rest + demolition, F8 + docs).

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

## Build D chunk 1 (2026-11-12; orchestrator items only)

Found with a scratch trace of bunker jobs (seeds 2, 3, 11, 14, 17; not committed): every job died one plan after the
pit (Watch, `Wait`, carried 0, no truck in `Load`). The fix keeps waiting; the rest is scarcity (S10b-ai-D1).
Controls seen failing: `heavy` false; `#[serde(skip)]` back on `blocked_pairs`. CLI test `s10b_ai_derived_fields` now
expects 18 + 16 raw keys. Germany 45.0 % on the default 40 set after the fix (47.5 % before).

## Build D chunk 2 (2026-11-12; stage chunk 1, sweeping, without the fortify row and fuzz stage)

Bugs met while testing: `squad_of` lacked `ClearMines` (the job saw its own pending order as refused); a lane of exactly
`lane_max_m` could be refused by rounding (now 10 cm under); a terrain-clamped lane end bent the lane off the line
(now shortened along it). Quick: fmt, clippy, neutral.py, ablate.sh ok; tests cli+content+sim 786 passed, 0 failed
(skirmish binary 651 s). Two quick runs overlapped by mistake (run.sh returned early); a clean rerun is in the report.
All-off 40 set not replayed this chunk (sweep code returns before any work with `sweep` 0; holds empty).
