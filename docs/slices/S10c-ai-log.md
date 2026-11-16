# S10c-ai log (defences at the front; at most 150 lines)

Spec: common `docs/slices/S10c-ai.md`, stages `S10c-ai-build-A.md`, `-B.md`, `-C.md`; targets `targets/S10c-ai.toml`.
Branch `claude/project-thread-e7nsak` from main `54009f6`. Another agent may work in this checkout: commit only your
files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)
**Plan done (2026-11-16). Next: Build A chunk 1 (`S10c-ai-build-A.md`).** No code changed yet; SAVE 25, PROTOCOL 14.
- Base facts (main `54009f6`, 80 matches seeds 1-40 both bases, `--side-detail`; file
  `/mnt/project-files/ai-defences-2026-11-15/b1main80.json` if the orchestrator copied it, else replay): Germany 45.0 %;
  mines 30 / 18 % of matches, obstacles 15 / 4 %, bunker 5 / 0 %; mines set off 0; deaths within 40 m of enemy works
  0 of 7908; breaches, sweeps, demolitions wanted 0; builder cards bought about 0.9 a match and side.
- The orchestrator must provide `target/s10c-base40` (the 40 set from a `54009f6` build) before Build A chunk 2.
- Gate after Build A (spec 8): `front_calm_pct` under 10 % stops Build B (owner question S10c-1 first).
- Owner questions S10c-1..5 (spec 10) are not yet in `docs/TODO.md` (another thread edits it: the orchestrator adds
  them). E6 (B) and E8 (A) are decided; T113 is 10 % and T109 45 % in `S10c-ai.toml` only.
- Not checked by the Plan (no cargo run): that `frontline-cli targets` accepts the new rows as written (float
  `exposure_min` 0.5 in T125; a `control` on another namespace than the metric). Build A chunk 2 item 7 checks it.

## Plan (2026-11-16)
Cause chain read in the code (spec 0): `trench_done` gate, the nest -> wire -> AP order, `fort_threat` (one known
enemy within 80 m of builder or site), trench-frame sites, about one builder squad a match, `jobs_max` 1, 12 m rows.
Design choices: front = the two own points nearest the enemy base (no map analysis); lanes by one path search per
point per `recheck_s` (foot: from the nearest enemy-owned point; vehicle: R2 unchanged); hasty AP field then wire then
AT field with no trench needed and ahead of the nest; the site's threat radius 50 m for front jobs only, the builder's
stays 80 m; a threat drop is not a refusal and the rest of the line is wanted again; belts block the lane beyond the
AT field so the way round crosses the mines; own-route safety for belts by a detour check instead of R6's 10 m.
Not designed in (said in spec 1): laying under fire, chokepoint analysis, a detour-cost demolition want.
Honest limits: on the open map a 20 m belt never leaves "no way round", so R14 demolitions stay rare (T126 is a
report); with one builder squad per match the front work competes with the rear works (F4 puts the front first;
`builders2` measures a second squad); B1 found that fortifying helps the USA more, so the guard band is watched at
every stage end.
Layout: only `AiCounters` fields and setup keys (SAVE 26 in Build A); lanes are a per-plan memo; no new job type.
