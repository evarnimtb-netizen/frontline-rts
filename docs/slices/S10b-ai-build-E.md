# S10b-ai Build E: owner changes after Build D (2026-11-14)

The owner answered the open questions. Stage D's item 9 (stage check) was not finished; it is this stage's last item.
Spec sections to read: only the rules each item names in `docs/slices/S10b-ai.md`. No layout change is expected
(SAVE 25, PROTOCOL 14). If an item needs one, stop and report.

## Chunk 1 (STOP POINT 1): bunker and barrier

1. **Bunker must appear in real matches (owner: "needs to be changed"; closes S10b-ai-D1).** Do not change the
   bunker's price, kg or build time. Two changes, each behind an `[ai.obstacles]` key (defaults on) so each has a
   control:
   - `bunker_assign` (0/1): when a bunker is wanted at a point and `bunker_why` says `no_builder` or `no_squad`, the
     AI assigns one free `build_heavy` (else `build_structure`) squad to that point for the bunker job.
   - `bunker_truck_first` (0/1): once per bunker job, the material run for its 250 kg step may take a truck away
     from tank service (the truck returns to its service job after the drop). At most one such run per job.
   Use the existing `bunker_why` counters to show which reasons fall. Test for each key with its failing control.
   Target to report (not a hard gate): bunker built in at least 15 % of the default 40 set's matches for at least one
   side. If still 0, report the remaining `bunker_why` reasons and stop; do not retune prices.
2. **Barrier job limit (closes S10b-ai-C1).** New key `[ai.obstacles] barrier_job_max_s` (default 1200) used as the
   L1 limit for the LogBarrier job only; every other job keeps `job_max_s` 900. Liveness row and fortify row
   `s10b_ai.obstacle` must show `barrier_built_pct` above 0 (it was 0 with 3.5 pieces standing); control
   `barrier_job_max_s` 900.

## Chunk 2 (STOP POINT 2): tank gap, lane, AT rows

3. **A demolished hedgehog row must open for a tank (owner: "needs fixing"; closes S10b-ai-D3).** Today a tank gap
   needs about 24 m between standing piece centres. Find the vehicle block pad of an obstacle piece (S10b code, the
   vehicle block layer) and narrow it so that a 4 m-step hedgehog row opens for a medium tank once 2 adjacent pieces
   are gone (gap of 12 m between standing centres), while an intact 4 m row still stops every vehicle (test both;
   also the log barrier and the wreck pad must still block). The vehicle path layer and vehicle movement must agree
   (the fault where the path served a gap the tank could not drive must be gone: test it). This is a rule change for
   the player too; it is allowed. It must not change all-off AI-vs-AI play (no obstacles exist there).
   Fortify row `s10b_ai.demolish` goes back to the cards' own satchel count and a 4 m row or ring; report
   gap_opened_pct and tank_passed_pct.
4. **Attackers cross a swept lane in single file (closes S10b-ai-D2 on the recommended option).** Key
   `[ai.obstacles] lane_file` (0/1, default 1): squads the AI sends through a swept lane walk it in file on the lane
   centre line until past the lane end. Reuse an existing column/file movement if the sim has one; otherwise order
   the men along the centre line one after another. Fortify row `s10b_ai.sweep`: report attacker_mine_losses (was
   3.0, control 8.0, target at most 1); control `lane_file` 0.
5. **AI AT field has two staggered rows (closes S10b-ai-B2a).** Key `[ai.obstacles] at_rows` (1..=2, default 2): the
   second row lies one step behind the first and shifted half a step along it; if the AI lacks mines for both it
   lays one. Fortify row `s10b_ai.mines`: report tank_stopped_pct on spread crossings (give the tank a lateral offset
   per seed that is not a multiple of the step, so the number is no longer the one-lane rate); control `at_rows` 1.

## Chunk 3 (STOP POINT 3): stage check

6. Ablation files in `docs/slices/targets/S10b-ai-ablations/` for the new keys (`*_off.toml`), and `all_off.toml`
   must also turn them off. `docs/modding/tuning.md` rows for the new keys.
7. Stage check (was D item 9): `frontline-cli targets docs/slices/targets/S10b-ai.toml --runs fortify10`; a default
   40 set and an all-off 40 set (`neutral.py target/s10b-base40 <set>` must say same 40/40); report Germany's share,
   bunker, obstacle, mine, sweep and demolition counts. The orchestrator runs `tools/check.sh tests` and the 480.
8. `docs/TODO.md`: mark B2a, C1, D1, D2, D3 as decided by the owner on 2026-11-14 with what was built; add two open
   owner questions, not built: (a) player control to choose 1-3 mine rows while placing a field (a key, not the
   wheel, which is camera zoom; may need a protocol bump); (b) demolished wire leaves debris that slows walking.
   Keep `docs/STATE.md` at most 150 lines.

Ends when: items 1-8 are committed, quick is green, the log top block is rewritten.
