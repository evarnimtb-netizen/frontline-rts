# S10b-ai log (the AI uses mines, obstacles, the bunker; at most 150 lines)

Spec: common `docs/slices/S10b-ai.md`, stages `S10b-ai-build-A.md` ... `-D.md`; targets `targets/S10b-ai.toml`.
Branch `s10b-ai` from main `72b2970`. Another agent may work in this checkout: commit only your files
(`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)

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

