# S10b-ai: the AI uses mines, obstacles, the bunker, sweeping and demolition: common part

Status: spec (Plan, 2026-11-12). Branch `s10b-ai` from main `72b2970`. Targets `targets/S10b-ai.toml`; log
`S10b-ai-log.md` (read its top block first). Each Build stage reads this file, its own `S10b-ai-build-X.md`, the log.
Pattern: `S10a-ai.md` 3.3, 3.7, 4, 9 (`ai/fort.rs` jobs, `[ai.fort]` switches). Mechanics: `S10b-obstacles.md` R1-R30.
Owner rules: features first, no Tune; Germany pooled 480 in 35-65 % (now 41.67 %); no armour, pen, gun, ammo,
lethality or existing card price change; every behaviour behind a switch; all new switches off = main, match for match.

## 1. Scope
**In.** (1) Laying: AP and AT minefields, hedgehog rows, log barriers, one bunker, built as S10a fort jobs (new
`WorkKind`s). (2) Sweeping known or suspected enemy mines on attack lines (`ClearMines`). (3) Demolishing enemy obstacles
that leave an AI vehicle no way round, piece by piece, at most 3 pieces (T106b honest form). (4) Paths avoid enemy mines
their player knows (sim rule, all players: the S10b-2 small version). (5) The job liveness contract (L1-L4) with one
generic test; the S8b truck loop F8 behind its own switch, last and droppable. (6) Measures in three-part form.
**Out (later).** Avoiding own mines (harmless by S10b-1 (A); comes with S10b-1 (B)); per-man paths off the lead's line
(formation men may still step on a known mine: R12, S10b-ai-2 (A), measured); terrain analysis for chokepoints; urgent card buys
for sweepers or demolishers; card weights or prices; tank-gun fire at obstacles; mortar fire to clear mines (R12 of
S10b exists, the AI does not aim at mines); presenter work (nothing new to show); S10b-5..9 stay on (A).

## 2. Switches and keys (`[ai.obstacles]`, `AiObstacleTuning`, a `tuning_table!`, `--set` works; Build A)
Switches (1/0): `mines` (AP and AT fields), `at_mines` (under `mines`), `obstacles`, `bunker`, `sweep`, `demolish`.
Values: `jobs_max` 1 (S10b jobs at once, inside `[ai.fort] builders_max`), `materials_max_pct` 3 (own budget, R8),
`ap_beyond_wire_m` 5, `ap_out_m` 40 (no wire), `at_out_m` 60, `fields_per_point` 2, `at_fields_max` 3, `obst_gun_m` 40,
`obst_work_m` 20, `hedgehogs_row` 3, `hedgehog_step_m` 4, `own_route_m` 10, `bunkers_max` 1, `suspect_s` 600,
`sweep_near_m` 20, `sweep_point_m` 30, `sweep_reach_m` 300, `demolish_pieces_max` 3, `demolish_near_m` 30,
`obst_seen_m` 150, `job_max_s` 900, `refused_s` 300, `refused_m` 20, `recheck_s` 60.
Sim keys `[fortify]`: `mine_paths` (switch), `mine_path_cost_pct` 2000, `mine_band_foot_cm` 300, `mine_band_vehicle_cm`
400. `[ai.truck]`: `refusal_memory` (switch, F8). Override files `targets/S10b-ai-ablations/{all_off,mines_off,
obstacles_off,bunker_off,sweep_off,demolish_off,paths_off,truck_memory_off}.toml`; `all_off` sets every new switch 0;
`loose.toml` (control only: `materials_max_pct` 100, `refused_s` 0, `job_max_s` 36000) is not an ablation.

## 3. Data model (every field in Build A's layout commit; later stages change no layout)
`WorkKind` += `ApField, AtField, Hedgehog, LogBarrier, Bunker`; `fort_of` keeps R29 for the five S10a kinds and returns
S10b blueprints only for the new kinds (`fort_of_ignores_s10b_blueprints` still passes). Jobs reuse `AiFortJob`.
`AiState` += `sweeps: BTreeMap<SquadId, AiSweep>` (cap `MAX_SWEEPS` 4), `demolitions: BTreeMap<SquadId, AiDemolish>` (cap
4), `refused: Vec<AiRefusal>` (sorted by (kind, failed, x, y), cap `MAX_REFUSED` 32), `suspects_seen: u32` (cursor into
`FortState::blasts`). `AiSweep { point: u8, a, b: FxVec2, start, since: u64, phase: SweepPhase, tries: u8 }`;
`AiDemolish { vehicle: SimId, pieces: [Option<u32>; 3], done: u8, start, since: u64, phase: DemolishPhase }`;
`AiRefusal { kind: JobKind, at: FxVec2, failed: u64 }` (kept max(`refused_s`, 300 s)), `JobKind` u8: Fort, Sweep,
Demolish, TruckGoal.
`AiFortJob` += `start: u64` (L1). `AiCounters` += `s10b_jobs, s10b_refused, lay_sites, sweep_wanted, sweeps_started,
sweeps_done, demolish_wanted, demolitions_started, gaps_opened, demolish_given_up, own_route_rejects, own_blocked,
jobs_overdue, refused_reorders` (fixed windows, not keys: a job alive over 900 s; a start or command within
`refused_m` of a goal that failed in the last 300 s, so the `loose` control shows them).
Sim: `FortState::blasts: Vec<MineBlast { pos, owner, tick }>` (mines set off, cap 32, dropped after the sim constant
`BLAST_KEEP_S` 600; S10b R13 makes a blast known to all), `FortAccount` += `lost_on_known` (men and vehicles that set
off a mine their player knew), `at_exposure_ticks` (enemy vehicle ticks within 5 m of the owner's AT mine),
`own_mine_losses` (own or allied units hurt by the player's mines: 0 by S10b R7, a guard), `own_pieces_demolished`.
`World::drive_ai_habit(ai, player, AiHabit::{Fort, Sweep, Demolish}, &DriveArgs)`: the fortify-test hook (as
`drive_breach`), one habit alone on a caller-kept `AiState`; skeleton in A, each stage fills its habit.

## 4. Rules (value; source; proving test). Distances integer m, times integer s, no RNG (else `Purpose::AI_OBST`)
| Id | Rule | Value | Source | Proved by |
|---|---|---|---|---|
| R1 | AP field at a held, trenched point with threat < `build_threat_max`: parallel to the trench, wire line + `ap_beyond_wire_m`, else `ap_out_m`; blueprint length | 5 / 40 m, 30 m | design 7.6 "AP mines among and before the wire" | `ai_lays_ap_field_beyond_its_wire` |
| R2 | AT field across the enemy vehicle approach: one vehicle path from the enemy base to the point (per point per `recheck_s`); site at `at_out_m` along it, perpendicular, centred | 60 m, 40 m | 7.6 "on tank approaches"; verify | `ai_lays_at_field_on_the_vehicle_approach` |
| R3 | Mines are never checked against own routes (own side never sets them off, S10b-1 (A)) | - | S10b R7 | `ai_tank_drives_through_own_field_unhurt` |
| R4 | Fields: at most `fields_per_point`, `at_fields_max` AT per player, blueprint caps R1/R2 of S10b | 2, 3 | cap | `ai_mine_caps_hold` |
| R5 | Log barrier across an own gun in Hold `gun_hold_s`, `obst_gun_m` out in its arc; hedgehog row (`hedgehogs_row` x `hedgehog_step_m`) `obst_work_m` in front of an own nest or bunker | 40 m; 3 x 4 m, 20 m | 7.6; S10b-ai-3 (A); verify | `ai_builds_barrier_before_gun`, `ai_builds_hedgehogs_before_nest` |
| R6 | Own route rule: no obstacle footprint (+ `wreck_pad_m`) within `own_route_m` of the own vehicle path base -> point -> nearest non-own point; one search at site choice, mirrored retry, else refusal | 10 m | owner brief | `ai_never_blocks_own_route` (control `own_route_m` 0) |
| R7 | Bunker: own held point nearest the enemy base, trenched, at the trench end opposite the nest (parity), facing the enemy; `bunkers_max`; the point's MG team defends from it (else the nearest defender) | 1 | 7.6; S10b-ai-5 (A) | `ai_builds_and_mans_one_bunker` |
| R8 | S10b jobs: at most `jobs_max` at once, inside `builders_max`; S10b materials MP <= `materials_max_pct` of income (own budget, S10a 3 % unchanged) | 1; 3 % | S10a-18 pattern | `ai_s10b_budget_stops_loading`, T117 |
| R9 | Suspect: an enemy `MineBlast` hurting own units, younger than `suspect_s` | 600 s | R13 of S10b | `ai_marks_suspect_from_blast` |
| R10 | Sweep want: non-own point with assigned attackers; a suspect or a known enemy mine within `sweep_near_m` of the line nearest attacker -> point, or a known one within `sweep_point_m` of the point; lane 10 m short of the first, `lane_max_m` long; sweeper: own `clear_mines` squad of that point, else nearest free within `sweep_reach_m`; no buy | 20, 30, 300 m | 7.6 sweep | `ai_sweeps_suspect_lane_before_attack` |
| R11 | Paths (sim, every player, `mine_paths`): a step within `mine_band_*` of an enemy mine the squad's player knows costs `mine_path_cost_pct`; foot: AP only, vehicles: AT only; exact segment test as S17; smoothing never keeps a priced step | 2000 %; 3 / 4 m | S10b-2 (A), S10b-ai-1 (A) | `paths_avoid_known_enemy_mines` (control `mine_paths` 0) |
| R12 | Limit: only the lead's path is priced; formation men 2-4 m off it may still step on a known mine (`lost_on_known` measures it) | - | honest limit | T116c report |
| R13 | Obstacle knowledge: an enemy `blocks_vehicles` piece within `obst_seen_m` of an own unit (no rays; the vehicle path layer is already global) | 150 m | S10b-ai-4 (A) | `ai_knows_obstacles_by_distance` |
| R14 | Demolish want: own AI vehicle with a task whose path is exhausted or failed twice, a known enemy piece within `demolish_near_m` of it; pieces: nearest the line vehicle -> goal, then neighbours alternating; after each charge the vehicle re-paths; ends `opened` when its path reaches, `gave_up` after `demolish_pieces_max` | 30 m, 3 | S10b-5 (A), S10b-ai-6 (A) | `ai_demolishes_until_a_tank_gap_opens` |
| L1 | Every multi-tick AI job ends by `start + job_max_s` (deadline) whatever its phase | 900 s | audit 7 | `ai_jobs_end_and_never_reorder_a_refused_goal` |
| L2 | A phase with no progress mark change for its stall time ends the job (S10a F13 rule, sweep: `along`; demolish: charge laid) | 3 x step + 60 s | S10a Fix F13 | same |
| L3 | A job ending without success (deadline, stall, rejected command, unreachable) adds `AiRefusal { kind, at, failed: tick }`; full: the oldest `failed` is replaced (never a live one while an older exists) | 300 s, 32 | F8 lesson | same, control `refused_s` 0 |
| L4 | No job starts and no command is sent for a goal within `refused_m` of a refusal of its kind younger than `refused_s`; `refused_reorders` counts violations (must stay 0) | 20 m | audit 7 | same, T118 |
| L5 | F8: with `refusal_memory` 1 a truck's give-up also writes L3 (kind TruckGoal) and the truck checks L4 | - | F8; S10b-ai-7 (A) | `truck_never_reorders_unreachable_hq` |

## 5. What else can be true when a job completes or ends (each row a named test; Build stage in brackets)
| Job | Target gone | Another squad on it | Squad dead or re-ordered | Vehicle in footprint | Save/load mid-job |
|---|---|---|---|---|---|
| Lay field / obstacle / bunker (fort job) | site taken by a work: `ai_s10b_site_taken_ends_job` [C] | one job per site: `ai_two_builders_never_share_site` [B] | job ends, work removed, kg counted: `ai_s10b_builder_dies_mid_work` [B] | BadSpot -> L3: `ai_s10b_refused_spot_is_remembered` [B] | `ai_s10b_deterministic_threads_and_save_load` [C] |
| Sweep | no suspect or mine left: `ai_sweep_ends_when_lane_clear` [D] | one sweep per lane: `ai_two_sweepers_never_share_lane` [D] | `ai_sweep_ends_when_sweeper_dies` [D] | own vehicle on lane: sweep goes on (no mine burst on own) `ai_sweep_beside_own_tank` [D] | same test [D] |
| Demolish | piece gone (shell, other charge): next piece or `opened` `ai_demolish_skips_gone_piece` [D] | one job per vehicle: `ai_one_demolition_per_vehicle` [D] | `ai_demolish_ends_when_vehicle_or_squad_lost` [D] | own tank beside the piece: charge spares it (R7 S10b) `ai_demolish_charge_spares_own_tank` [D] | same [D] |
| Truck goal (F8) | HQ gone: give-up, no refusal needed `truck_hq_gone_ends_goal` [D] | - | existing truck rules | - | `truck_refusal_survives_save_load` [D] |
Generic: `ai_jobs_end_and_never_reorder_a_refused_goal` [A harness, B-D add their kinds] drives each kind at an
unreachable goal for 20 min; asserts end by L1/L2, one refusal, no command for the goal for `refused_s`, at most one retry after.

## 6. Save, hash, validation (`load_rejects_bad_s10b_ai_state`: one case per line; Build A)
`sweeps`, `demolitions`: <= caps; keys own living squads with `clear_mines` / `demolish`; points < point count; `a`, `b`
on the map, `|b-a| <= lane_max_m`; `vehicle` an own vehicle; pieces < `next_piece`; `done <= 3`; `start <= since <= tick`.
`refused`: <= 32, sorted, `failed <= tick`. `blasts`: <= 32, owner a player, `tick <= now`, on the map.
`AiFortJob::start <= since`. `FortAccount` new counters: any value. Setup: `[ai.obstacles]` ranges, switches 0..=1.
**SAVE_VERSION 24 -> 25** once, in Build A's layout commit (AiState, FortState, FortAccount, setup keys). **PROTOCOL
stays 14**: no command, kind, body or limit; the AI emits `Fortify`, `ClearMines`, `Demolish`; the path rule is fenced
by `build_id` and `content_hash` like every sim change. Each saved addition joins the staged save of `save_fuzz.rs`
in the stage that fills it (stage files name it "fuzz stage: ...").

## 7. Measures and row honesty (Build A)
Every new row has value, exposure (counter with `exposure_min`) and control (a run or row with the thing off that must
miss the row's bounds). `frontline-cli targets` gains `exposure`, `exposure_min`, `control` keys and verdicts `UNEXPOSED`
(fails the run, never a hard miss) and `CONTROL-SAME` (fails); a file key `three_parts_from = "T109"` (numeric
compare of the id) makes them required; report rows print exposure and control, no verdict on them. `measure.sh` plays every `skirmish40_*` run of the targets file with its `rules_override` (controls `skirmish40_off`,
`skirmish40_loose`). Fields: `factions.<f>.s10b_ai` raw and `s10b_ai_derived` (Build A stage file lists them); counters
count only with their switch on (all-off JSON stays zero for `neutral.py`).

## 8. Done when
1. T109-T118 pass or are exposed-and-reported as the targets file says, on skirmish160 and skirmish160_oos.
2. Own-side mistakes 0 (T115); `refused_reorders` and `jobs_overdue` 0 (T118). 3. Germany pooled 35-65 % over 480;
no match without a winner; no new hard miss beyond T8b, T40c, T54e, T88b, T88d (T106b is a report).
4. `all_off` ablation: `neutral.py` same 160/160 x3 and 40/40 against `target/s10b-verify`. 5. Every named test passes;
SAVE 25, PROTOCOL 14; save fuzz with the staged S10b-ai save: 0 panics, 0 unloadable. 6. Tick p99 <= 25 % above 0.487.

## 9. Build stages (stop points in each file; one chunk is about 30 self-counted turns)
A layout, switches, measures, targets tool, liveness harness (`-build-A.md`, 2 chunks); B mine paths and AI laying
(`-B`, 2 chunks); C obstacles and the bunker (`-C`, 2 chunks); D sweeping, demolition, F8, docs (`-D`, 3 chunks).
Then orchestrator measure + ablations (`tools/ablate.sh`) -> Review -> Fix -> verify -> Docs. No Tune stage.

## 10. Owner questions (the slice runs on each (A); also in `docs/TODO.md`)
S10b-ai-1 paths avoid only enemy mines the player knows ((B) own too, only with S10b-1 (B)). -2 only the lead's path
is priced ((B) per-man paths near known mines). -3 obstacles in front of own guns and works only ((B) across enemy
approaches beside AT fields). -4 the AI knows enemy obstacles within 150 m without sight rays ((B) rays as wire).
-5 the point's MG team mans the bunker ((B) a rifle squad). -6 demolition only when a vehicle has no way round, at
most 3 pieces ((B) never for vehicles). -7 the F8 truck fix on by default ((B) off until the balance pass).
