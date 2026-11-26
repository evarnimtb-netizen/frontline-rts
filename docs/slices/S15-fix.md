# S15 Fix stage: Review findings

Read first: `docs/slices/S15-review.md` (14 findings, rule verdicts, owner-question verdicts); `S15-log.md` "For the next stage"; the
plan's owner rule (`S15-ussr.md`: no balance runs, no tuning, every number a TOML key). Branch `claude/project-thread-aj1tww` at the
head. **2 chunks.** Work rules as every S15 stage (`tools/run.sh`, no command over 4 minutes, commit after every green step with
`git commit -m "S15 F: ..." -- FILES`, never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`, `godot/match/*.gd.uid`;
do not push; no model names in files or commits).

Fix every high and medium finding and the lows, each with a test that shows the effect in play (not only the order):
- 1-3: Soviet guns must arrive, emplace, fire, be crewed and be re-sited like the German and US ones. Prefer the pattern the other
  factions use (no tow on infantry gun and howitzer cards if that is how the German cards work; a tractor-role tow with seats for the
  AT guns, as the Sd.Kfz. 11 pattern), with sources for any new def; add a content check that a gun's tow seats its crew; name the
  gun cause in the CLI's `ABANDON_CAUSES`.
- 4: a rider job that ends sends `Stop` (or the transport job's equivalent) so the squad returns to the habits.
- 5: rider decks of 10 on the T-34-85 and IS-2 (the review's consistency fix; the plan's 6.1 vs 6.4 disagree), with a source note if
  one exists; record it in `docs/TODO.md` as S15-14 "(A) assumed 2026-11-26".
- 6: the three godot-bridge tests expect the shipped three-faction table as the review says.
- 7: tests that prove something (a habit test that calls the habits on a USSR roster and sees the guns emplace; the determinism
  scene with captured bombs and bayonets actually in it; the bridge frame test checks values; a real ROKS-3 hits a man; the plan's
  `riders_off_is_base`).
- Lows as the review lists; `godot/docs/match_state.md` says what the switches ship at.
- T213c (ROKS-3 hits) and T213d (bayonet strikes) become `kind = "report"` in `targets/S15.toml` with the review's reason, recorded as
  S15-15 in `docs/TODO.md` with "(A) assumed 2026-11-26: reports; a close-assault habit is later work".

Then three 40 sets through `tools/batch.sh` with `BATCH_JOBS=4` (owner-approved checks; if refused, skip and say so): T210 and T211
(Germany-USA, `python3 -I tools/neutral.py target/s14-F-40 <dir>` same 40/40) and T212 Germany-USSR (`target/s15-F-pair40`): winners,
length min/median/max, each S15 rule's counts, guns emplaced/missions/rounds for both sides, the USSR's wins (reported, not judged).
If T212c (length <= 40 min) still misses, record it as owner question S15-16 for the balance pass; tune nothing.
`tools/check.sh quick 96d4b4b` green (IPv6 excepted); Godot headless `test_net_ui.gd` and `test_match_view.gd` if the presenter or
bridge changed. Update the log (Fix section, "For the next stage" for the PR), `docs/TODO.md` S15 block, `docs/STATE.md` (S15 entry,
Versions line SAVE 34 / PROTOCOL 17, three factions).
