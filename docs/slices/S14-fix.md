# S14 Fix stage: Review findings

Read first: `docs/slices/S14-review.md` (16 findings, ranked); `S14-log.md` "For the next stage"; the plan's owner-change note
(`S14-assault-kit.md` top: no balance runs, no tuning, every number a TOML key). Branch `claude/project-thread-aj1tww` at the head;
`<BASE>` `a27ce63`. **2 chunks.** Work rules as every S14 stage (`tools/run.sh`, no command over 4 minutes, commit after every green
step with `git commit -m "S14 F: ..." -- FILES`, never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`,
`godot/match/fort_view.gd.uid`, `godot/match/wound_view.gd.uid`; do not push; no model names in files or commits).

Fix every high and medium finding (1-11) and the lows, each with a test that shows the effect (not only the order):
- 1: flame aimed at a slit must reach the slit band (aim height from the target: the slit point's height for a bunker, the trench
  floor/man for a trench); the R1 unit test must fire through the normal `SuppressUnits` path, not hand-thrown particles; also the gap
  under the front timbers if it is a geometry defect of the flame path (do not change the bunker def: S10b rows and base matches must
  not move; if only the def can fix it, write it as an owner question).
- 2: the AI flame job, buy and A4 work from remembered sightings and known enemy works (a trench or bunker the side has seen), not
  only men seen this tick; Burn lasts while the target area holds a remembered enemy within the job's keys; new widths/times only as
  TOML keys (default values chosen so the rule fires; they are not tuning, say so in the log).
- 3: the strike aims at the target's capsule (its current position and height at resolution, within reach), so it hits crouched,
  prone and trench men; start distance inside reach; a dry man with a sidearm draws it rather than striking forever if no strike can
  land. `drop_cm` may go if it is no longer needed (keep the key only if it still means something).
- 4: the AI's clear bookkeeping must not end its own clears.
- 5-11 as the review suggests. 9: T192 must test something real (a job older than `job_max_s` never exists), AI tests check effects
  against armed defenders, thread test 1, 2, 3, 8. 10: Strike gets a key that is free (Board keeps `V`), or `V` strikes only when a
  selected man has an enemy in reach, else boards: pick the simpler, record it as an (A) assumed owner question in `docs/TODO.md`.
- R1: after fix 1, run R1; if it passes make it hard again (T186); if it still fails only because defenders kill the lone flame man,
  keep it report-only and record that in `docs/TODO.md` as an owner question for the balance pass.

Then: **one** default 40 set (`target/s14-F-40/`) and the `s14_off` 40 set (`target/s14-F-off40/`, must equal `target/s14-base40`
under `python3 -I tools/neutral.py`), both through `tools/batch.sh` with `BATCH_JOBS=4`, read for breakage and for each S14 rule
firing (strike hits > 0, flame hits on men in trenches or bunkers > 0, jobs end); no judgement of Germany's share. If a rule still
never fires, find out why. `tools/check.sh quick a27ce63` green (IPv6 test excepted), the Godot headless match test if you touched
the presenter. Update the log (Fix section and "For the next stage" for Docs/PR), `docs/TODO.md` S14 block (S14-8.. new owner
questions, each with (A) assumed), `docs/STATE.md` (S14 entry and Versions line: SAVE 33, PROTOCOL 17).
