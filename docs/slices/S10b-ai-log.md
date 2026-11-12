# S10b-ai log (the AI uses mines, obstacles, the bunker; at most 150 lines)

Spec: common `docs/slices/S10b-ai.md`, stages `S10b-ai-build-A.md` ... `-D.md`; targets `targets/S10b-ai.toml`.
Branch `s10b-ai` from main `72b2970`. Another agent may work in this checkout: commit only your files
(`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)

**Next: Build D chunk 2 = the stage file's chunk 1 (sweeping R9/R10), not started.** D chunk 1 did the orchestrator
items only: `58a3886` (bunker reasons + fix), `b179bbc` (T115e exposure), `c3e64bb` (`blocked_pairs` saved). Layout:
SAVE 25, PROTOCOL 14 (no bump; `AiCounters::bunker_why [u32;16]` and saved `blocked_pairs` join the unreleased 25).
**Bunker:** `AiCounters::bunker_why` (names `ai::BUNKER_WHY`; JSON `s10b_ai.bunker_why_*`; summary line `s10b-ai bunker
why:`), counted in `fort.rs` `bunker_why()`/`bunker_block()` (per plan, first failing check) and `bunker_end()`. Bug
fixed: in Watch a 250 kg step in `Wait` ended the job the plan its pit was dug unless a truck was in `Load` that plan
(all 4 jobs in 40 matches died so); now `heavy` steps wait while `ai.trucks` is non-empty (L1 bounds it), S10b truck
runs use `room_for` (S10b budget). Test `ai_bunker_waits_for_a_busy_truck`. Still 0 bunkers in the default 40 set
(`target/s10b-d1-fix`, Germany 45.0 %): per-plan reasons ger/usa: point_not_ready 9291/8860, no_builder 6936/6615,
builders_max 2801/1739, no_point 2591/3945, no_squad 1582/2187, s10b_room 1309/1457, other_first 80/66, no_carry 8/9,
no_budget 0/2; jobs started 3/1; after the fix the trucks stay in Follow/Drop/Serve for tanks for minutes, and a
job that gets its 250 kg (seed 11) is still in the 540 s Build step when the match ends (mean 21 min). Honest
scarcity: owner question **S10b-ai-D1**, proceeding on (A) leave as is.
**T115e:** `s10b_ai.obstacle.own_tank_drove_pct` exists and reads **0** (seeds 1-3): with obstacles on (modes 0, 1)
the own tank's Move ends `Idle` at base, never moving; with `obstacles` 0 it drives 588 m. Not yet understood (the
hook drives every tick; maybe its vehicle path is refused); T115e stays unexposed (no hard miss). Look before D ends.
**blocked_pairs:** saved, `MAX_BLOCKED_PAIRS` 32 (truncated in `count_own_blocked`), validated in `validate_s10b`;
fuzz stage `staged_s10b_ai + a stuck own pair`. Test `own_blocked_counts_once_across_save_load`.
**Slow skirmish binary (item 4):** quick's skirmish binary took 332 s. Likely cause: `own_route()` memo lives one
`Planner` (one plan), so every plan re-runs two vehicle A* searches (`gun_route`) per own point for each hedgehog/
barrier site check (`own_route_side`, `on_own_route`); not changed here.
Owner questions open: S10b-ai-D1 (new), C1, B2a, 1..7. Failing: none. Last check: `tools/check.sh quick 72b2970`
all ok (780 passed); save_fuzz incl. ignored ok (15); all-off 40 `target/s10b-d1-off40` vs `target/s10b-base40`:
**same 40/40**. Chunks left: D 3 (sweeping, demolition, F8 + docs + stage check).

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

## Build A code map (the top block before B1)

**Next: Build B** (`S10b-ai-build-B.md`, its chunk 1). Build A is done (chunks 1 and 2; STOP POINT 2 met).
Layout: SAVE 25 (commit `0ae000f`, the one bump), PROTOCOL 14; no later chunk may change either.
Code map (A2): `sdk/crates/cli/src/skirmish.rs`: `S10B_AI` (14 names), `S10Player::{s10b_ai, s10b_chances,
s10b_materials_g, s10b_materials_micro, s10b_ai_counters}`, `CHANCE_LAY/OBST/BUNKER`, `S10bAiSeen` (per tick: AI fort
jobs by blueprint class -> chance bits; grams S10b works draw), `S10bAiFaction::{add, get}`, `FactionSummary::s10b_ai`,
text line "s10b-ai:". `json_out.rs`: `s10b_ai_blocks` -> `factions.<f>.s10b_ai` (14 counters + `lost_on_known`,
`at_exposure_ticks`, `own_mine_losses`, `own_pieces_demolished`) and `s10b_ai_derived` (22 fields of stage A step 8;
`matches_with_bunker_pct`, `sweep_when_wanted_pct`, `demolish_when_wanted_pct` null with no chance/want).
Chances (exposures; all from AI state, so 0 with switches off): lay = `lay_sites > 0` or an AI job on a mine
blueprint; obst = AI job on a `blocks_vehicles` blueprint or `own_route_rejects > 0`; bunker = AI job on the
BunkerPit blueprint. B/C must count `lay_sites`, `own_route_rejects` and keep `AiFortJob::fort` = the S10b blueprint.
`targets.rs`: `TargetSpec::{exposure, exposure_min, control}`, `TargetsFile::three_parts_from`, `id_number`,
`Verdict::{Unexposed, ControlSame}` (hard rows only; report rows print the parts), `tally` counts them as failing.
`S10b-ai.toml` now parses (471 rows). `tools/measure.sh` plays every `skirmish40_*` run (flat TOML read, py 3.9).
`tools/ablate.sh`: `ABLATIONS=DIR` (default `docs/slices/targets/S10b-ai-ablations`), TARGETS beside it,
`NEUTRAL_BASE` default `target/s10b-verify`, default/`all` = every `*_off.toml` (8); `loose.toml` is the control.
`tools/pool.py`: "s10b-ai:" line (mines/obstacles/bunker/sweep shares, own-side mistakes).
Liveness (`crates/sim/src/s10b_ai_tests.rs`): `Stage {habit, args, goal}`, `LIVENESS: &[(JobKind, StageSetup)]`
(`fn(&mut World) -> Stage`; empty: B adds Fort, D Sweep/Demolish/TruckGoal), `job_alive`, `drive_row`, `check_liveness(trace, Limits)`
(L1 deadline + slack, one refusal per failed job, no start within `refused` ticks, at most one retry, reorders 0,
at least one refusal). `job_alive(TruckGoal)` is false: D must fill it.
40 set (defaults, `target/s10b-ai-a2-40`): `neutral.py target/s10b-base40` -> **same 40/40**.
Tests added (controls seen failing): `unexposed_row_fails_not_hard`, `control_inside_bounds_fails`,
`three_parts_required_from_id` (cli lib), `s10b_ai_derived_fields` (cli `--test skirmish`),
`ai_jobs_end_and_never_reorder_a_refused_goal` (sim lib). Failing tests: none.
Keep: `refused_near(.., refused_ticks, near_m)` takes TICKS; `drop_stale_refusals(tick, max(refused_s, 300 s) ticks)`;
clamp `suspects_seen` to `blasts.len()`. Chunks left: B 2, C 2, D 3. Owner questions S10b-ai-1..7 (spec 10).

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

