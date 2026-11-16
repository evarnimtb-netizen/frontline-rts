# S10c-ai Build A: layout, switches, honest counters, observers, target rows

Read first: `S10c-ai.md` 0, 2, 3, 6, 7; the log's top block. **2 chunks**, about 30 self-counted turns each. No AI
behaviour changes in this stage: with every new key at its off value the 40 set must equal main match for match.
Work rules: `tools/outline.sh` then `tools/show.sh FILE::name` (batch the reads); targeted release tests through
`tools/run.sh`; `tools/check.sh quick 54009f6` once per chunk end; no command blocks over 4 minutes; commit only your
files (`git commit -m ... -- FILES`), never `git add -A`.

Start from: `sdk/crates/content/src/tuning.rs` (`AiObstacleTuning`, the `tuning_table!` at about line 1779, B1-4's
`hold_max` as the pattern for a new key: `skirmish.toml`, `docs/modding/tuning.md`, key count, setup-bytes BASELINE in
`sdk/crates/content/tests/resolved_defs.rs` and `crates/simbench/src/report.rs`, canonical snapshot);
`crates/sim/src/ai.rs` (`AiCounters`, `AiState::refuse`); `crates/sim/src/ai/fort.rs` (`s10b_end`, `own_route_side`,
`bunker_why` / `bunker_block` as the pattern for a why-table, `point_starts`, `field_wants`); `ai/truck.rs`
(`truck_give_up`); `sdk/crates/cli/src/skirmish.rs` (`S10B_AI`, `S10bAiSeen::observe`, B1's `death_places` observer
and `--side-detail`); `sdk/crates/cli/src/json_out.rs` (`s10b_ai_blocks`); `tools/neutral.py`, `tools/measure.sh`.

## Chunk 1: layout commit, keys, sim counters
1. One layout commit: `SAVE_VERSION` 25 -> 26; `AiCounters` gains every field of spec 3 (zero default); the
   `[ai.obstacles]` keys of spec 2 at their OFF values (`front` 0, `mine_assign` 0, `belt` 0, `front_points` 2,
   `front_out_m` 50, `front_wire_len_m` 40, `front_site_threat_m` 50, `front_fields_max` 2, `belt_pieces` 5,
   `belt_gap_m` 10, `belt_detour_pct` 130, `builder_cards_min` 0) with ranges (spec 6), comments in `skirmish.toml`,
   rows in `docs/modding/tuning.md`. `STATE.md` "Versions" line. Override files of spec 2 under
   `docs/slices/targets/S10c-ai-ablations/` (`c_off` lists every new key at its off value and `jobs_max = 1`).
2. Fill the counters that need no new behaviour: `fort_job_fails` in `s10b_end` (failed), `truck_give_ups` in
   `truck_give_up`, `route_refusals` in `own_route_side`; `s10b_refused` keeps counting all three (old rows unchanged).
   `mine_want_why` (finding 12): per plan and own point where an R1 or R2 field is switched on and not laid, the first
   check that keeps it from starting (not_own_or_trench, wire_first, cap, no_site, refused, slot, no_builder,
   threat), written beside `bunker_why` and reading state only.
3. Front exposure without the behaviour: a pure helper `front_points(&self) -> Vec<usize>` (F1) and, per plan,
   `front_plans` += 1 when it is not empty and `front_calm_plans` += 1 when no known enemy stands within
   `threat_radius` of any front point. These two count even with `front` 0 (they decide the gate of spec 8); they
   read state only, so the all-off matches stay the same.
Tests (each names its control): `s10c_refusal_split_adds_up` (a failed fort job, a truck give-up and an R6 reject in
one scene: three fields 1/1/1, `s10b_refused` 3; control: `refusal_memory` 0 -> `truck_give_ups` 0),
`ai_front_points_are_nearest_the_enemy_base` (own B and C -> C first; own B only -> B; control `front_points` 0 ->
none), `mine_want_why_names_the_wire_gate` (trenched own point, no wire: `wire_first`; control wire built: next reason),
`load_rejects_bad_s10c_keys` (one case per range), save roundtrip green.
**STOP POINT 1.** Quick green; commit; rewrite the log's top block; return.

## Chunk 2: CLI observers, JSON, targets rows, neutrality
4. `S10cSeen` (`skirmish.rs`), observed when `world.tick() % hz == 0`, per player, read-only: mines approached (ids
   of own mines with an enemy soldier (AP item) or enemy vehicle (AT item) within 30 m, a set; `mine_near_s`), wire
   approached (own section, enemy soldier within 30 m of its middle), obstacle approached (own `blocks_vehicles`
   piece, enemy vehicle within 30 m), wire seen (`AiState::wire_known` of the other AI player holds a section of this
   player: a flag), `works_by_point` / `mines_by_point` (nearest map point of each own work done, mine laid, at the
   end of the match plus what was set off), `mines_home` / `mines_front` (home = the map point nearest the layer's
   base). Use the spatial hash or a 30 m grid of mine positions, not an all-pairs loop; measure the cost: the 40 set
   wall time may rise by at most 5 % (log both numbers).
5. `json_out.rs` `s10c_blocks`: `factions.<f>.s10c` and `s10c_derived` exactly as spec 7 names them (null, not 0,
   where the denominator is 0). `--side-detail` per-side: the same raw counters.
6. `tools/neutral.py`: keys starting `s10c` are skipped when the base file lacks them, compared when both have them.
   Prove neutrality against main: base `tools/batch.sh target/s10c-base40/skirmish40.json -- skirmish --ai-vs-ai --seed
   1 --seeds 20 --both-bases --summary --minutes 45` built from `54009f6` (the orchestrator provides the folder; ask in
   the hand-over if it is missing), then the same set with this binary and `c_off`: `neutral.py` same 40/40. Then
   replay the default 40 set into `target/s10c-A40` and `c_off` into `target/s10c-baseA40` (the base for B and C).
7. `docs/slices/targets/S10c-ai.toml` exists (Plan): check every `s10c*` metric path in it resolves on
   `target/s10c-A40` with `frontline-cli targets` (rows T119-T127 will miss now; none may be `MISSING-METRIC`);
   fix the JSON names, not the targets file, unless a name is impossible, and say so in the log.
Tests: CLI `s10c_observer_counts_an_approach` (a staged enemy rifle squad walked to 20 m from an own AP field: the
mine ids are counted; control: walked to 60 m -> 0), `s10c_json_names_match_the_targets_file`.
Ends: quick green; log the 40-set numbers that the gate of spec 8 needs: `front_plans`, `front_calm_pct`,
`mine_want_why_*`, `mines_home` / `mines_front`, `matches_mines_approached_pct`, `wire_seen_by_enemy_pct`,
`defence_job_fail_pct`, per faction.
**STOP POINT 2.** Commit; rewrite the log's top block (numbers, code map of the new helpers, what B needs); return.
