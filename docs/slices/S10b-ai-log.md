# S10b-ai log (the AI uses mines, obstacles, the bunker; at most 150 lines)

Spec: common `docs/slices/S10b-ai.md`, stages `S10b-ai-build-A.md` ... `-D.md`; targets `targets/S10b-ai.toml`.
Branch `s10b-ai` from main `72b2970`. Another agent may work in this checkout: commit only your files
(`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)

**Next: Build C** (`S10b-ai-build-C.md`, chunk 1). Build B done (STOP POINT 2; `a7d5a2c` `667753f` `80cb53c` `eec1f21`).
Layout: SAVE 25, PROTOCOL 14 (unchanged). B1 code map: section "Build B chunk 1" below; A's: "Build A code map".
B2 code map (`crates/sim/src/ai/fort.rs`): `s10b_on`, `s10b_fort(d)` (mine, `blocks_vehicles` or BunkerPit blueprint),
`habits_on` = `s10a_on` or `s10b_on` (base loads, truck runs, S10a plan counters: `s10a_on` only); `point_site_at`:
`ApField` (R1), `AtField` -> `at_site` (memo `Planner::at_sites`, per plan) -> `at_site_search`
(`guns::gun_route` from the enemy base, back `at_out_m`, perpendicular to that leg); `field_has`, `fields_of`,
`recheck_plan` (stateless: one plan per `recheck_s` window), `field_wants` (appended in `point_wants`); `point_starts(.., s10b_only)` (split out of `fort_start`: `s10b_room` = `jobs_max`,
`under_limit`, `refused_here` (L4), `lay_sites` on the recheck plan); `new_fort_job` counts `s10b_jobs` and the fixed
300 s `refused_reorders` guard; `fort_continue`: `drop_stale_refusals`, L1 deadline, `s10b_end(job, failed)`
(L3 refusal at the line middle, `s10b_refused`, `jobs_overdue` > 900 s + a plan), `s10b_laid`; refused S10b jobs are
not parked (the refusal keeps the spot). Budget: `s10b_used_g` (own mines at blueprint g, gone mines at the cheapest,
S10b works' `got_g`, S10b job squads' carry and pending loads), `s10b_room_g`, `room_for(f)`, `load_builder(sq, room)`;
S10a's `materials_room_g` subtracts S10b grams. `drive_fort` (`AiHabit::Fort`). C adds obstacle/bunker sites as new
`point_site_at` arms and kinds in `field_wants`-like lists; `s10b_laid` already accepts pieces; R6 own-route goes in.
Tests: `crates/sim/src/s10b_ai_lay_tests.rs` (`held`, `squad_at`, `Held` are `pub(crate)`; the eight B tests), Fort
liveness row `fort_row` in `s10b_ai_tests.rs` (`StageSetup` is now `fn() -> (World, Stage)`, `Stage::ai`). S10a tests
call `s10a_ai_tests::s10b_ai_off`. Failing: none. Last check: `tools/check.sh quick 72b2970` all ok (769 passed).
Fortify `s10b_ai.mines` (`--seeds 5`): field_laid_pct 100 (builder_alive_pct 100, control 0), laid_s 192.5,
tank_reached_field_pct 100, **tank_stopped_pct 0** (control 0): T110i (min 60) misses; laid_s is the same for every
seed (the staging does not vary by seed but the tank's 0-4 m offset). Open: why no crossing tank triggers the one
40 m row (4 m step, 40 cm trigger: ~40 % by geometry); check the track positions vs the mines (C or D).
40 sets (`target/s10b-ai-b2-off`, `-on`): all-off `neutral.py target/s10b-base40` **same 40/40**. On-set: Germany
22/40 = 55 % (all-off 18/40 = 45 %); mines in 55 % (ger) / 42 % (usa) of matches (7.58 / 5.85 laid a match), AT
fields 0.03 / 0.05 a match (rare: offered on one plan per `recheck_s` and only to a builder assigned there),
tanks stopped by mines 0.00 a match (vehicles_mined 0), refusals/re-orders 0, overdue 0, S10b materials 0.06 % of income.
Fuzz: `staged_s10b_ai` adds an AT field job in Watch, its work (seg 3) and 3 laid mines; `holds()` "ai_at_field_watch".
Chunks left: C 2, D 3. Owner questions S10b-ai-1..7 (spec 10); B2 adds S10b-ai-B2a (log below).

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

