# B3 run 1: screen (stage contract; a Tune-tier agent, medium effort; eight 160 sets and one or two combinations, about 2 hours of machine time)

Read first, and only: this file, `B3-balance.md` sections 3, 4, 5 and 8, and `docs/TODO.md` for the lines B2-9, B2-12 and
B3-1..7 (which are answered or marked "(A) assumed"). You measure and log. You decide nothing beyond the rules in
`B3-balance.md` 5 "Screen"; you change no file under `mods/`, `crates/`, `sdk/` or `tools/`.

## Rules of the stage
- No foreground command over 4 minutes. Every set runs through `queue.sh` in the background, started with `tools/run.sh LOG ...`
  and polled with `tools/run.sh --wait LOG` (each call waits at most 200 s).
- `BATCH_JOBS=4` (the queue's default). Do not start two queues at once. Do not run `tools/measure.sh` (the orchestrator's).
- The container can restart. After each finished lever: commit `docs/slices/targets/B3-results/` and `B3-log.md`. After a
  restart: `git log --oneline -5`, read the log's top block, start the same queue line again (played sets come from the cache
  if `target/` survived; finished levers already have their line, CSVs and counters in git: skip those names).
- Commit only your files: `git commit -m "B3-1: ..." -- docs/slices/B3-log.md docs/slices/targets/B3-results` (add a file of
  `B3-levers/` only if you had to fix it). Never `git add -A`; never `target/`, `.claude/`, `tools/__pycache__/`,
  `godot/match/fort_view.gd.uid`. Index lock: wait 10 s and retry. Do not push. No model names or personal details in files
  or commit text.
- Numbers come from `paired.py` lines, `--counters` output and `--json` files, never from the text tables.

## Files you own
`docs/slices/B3-log.md` (create it: header as `B2-log.md`'s first six lines with B3's names, a top block "For the next stage"
of at most 30 lines, then one section "B3-1 screen"; at most 150 lines), `docs/slices/targets/B3-results/*`, `target/b3/*`.
Fixes to `B3-levers/*.toml`, `paired.py` or `queue.sh` only if a key, a JSON path or a command is wrong (say so in the log).

## Chunk 0: base state (about 15 minutes)
1. `git log --oneline -3`; `git status --short | head`. The head is BASE. `docs/STATE.md` must say S13 is measured at
   `3b3387f` with Germany 42.08 %; if `mods/` or the sim changed since (`git diff --stat 3b3387f -- mods crates sdk`), log it:
   the judge base `target/s13-on` is then stale and the orchestrator must measure again before the judge.
2. `mkdir -p target/b3`, then `CARGO_BUILD_JOBS=4 tools/run.sh target/b3/build.log cargo build -q --release -p frontline-cli`.
3. `tools/batch.sh world`: log the key (run 2 compares it). `python3 -I tools/pool.py target/s13-on | head -1` if the dir
   exists: log the line.
4. `tools/run.sh target/b3/check.log docs/slices/targets/B3-levers/queue.sh check` (about 4 min). Every file must print at
   least one `old -> new` value; log the count per file. Expected: `hv_ger_reach` 3 values, `own_m17_160_64` 2,
   `own_class_twins` 2, `own_s12_bleed120_300` 2, every other file 1. A file that changes nothing or fails to load (a
   validation rule on a price, a class or a flag): fix its key, or log why it cannot load and drop that lever; do not work
   around a validation error by editing the mod.
5. `tools/run.sh target/b3/armour.log docs/slices/targets/B3-levers/queue.sh armour own_class_twins` (base and lever, 5 seeds,
   all cases). Log: both `pass` values, case 45's `jumbo_survives` base -> lever, and every other check that moved
   (`B3-results/armour_own_class_twins.txt`). If `--armour` cannot read the JSON, fix `paired.py` and say so. This is goal 3's
   whole measurement; nothing else is played for it.

## Chunk 1: base and the three HE levers (about 50 minutes)
`tools/run.sh target/b3/q1.log docs/slices/targets/B3-levers/queue.sh 3001 he_shell_cap6 own_m17_160_64 own_rg_gap25`
- If `docs/TODO.md` shows B2-12 answered (B): skip this chunk's three levers (play the base only: `queue.sh 3001`) and log it.
- When `target/b3/s3001/base.json` exists: `python3 -I docs/slices/targets/B3-levers/paired.py --counters
  target/b3/s3001/base.json`. STOP and report if a match has no winner or Germany's share is outside 35-65 %. Log the base's
  HE share, the three HE classes per side and the S13 card shares.
- After each lever the queue writes its line to `B3-results/paired.txt` and its counters to
  `B3-results/counters_s3001_NAME.txt`. Copy the line into the log's table with the own counter, base -> lever, a match per
  side: for `he_shell_cap6` base-gun rounds, budget holds, shells % of income, and from the JSON
  `s11b_derived.how_fired_matches_pct` and `s11a2_derived.cb_fired_matches_pct`; for `own_m17_160_64` the rifle-grenade
  casualties (outright + downs) per side; for `own_rg_gap25` grenades fired. Commit after each.

## Chunk 2: heavies and the vehicle mix (about 50 minutes)
`tools/run.sh target/b3/q2.log docs/slices/targets/B3-levers/queue.sh 3001 hv_save360 own_m36_h1 hv_ger_reach mix_vehicle30`
Own counters: the five S13 cards' matches-% base -> lever, heavy plans / buys / drops per side, and for `mix_vehicle30` the
vehicle cards a match per side, the mortar share of kills and all vehicles' share of kills per side (logged, T31 is no floor).

## Chunk 3: the combinations (about 12 minutes each)
Apply the rules of `B3-balance.md` 5 exactly:
1. Mark each lever pass or fail by its rule (H: HE at least 0.5 under the base and Germany not below -2.0; V: the card up at
   least 5 points and Germany not below -2.0; W: Germany at least +5.0), each with `floors ok` and its counter moved. A lever
   at 0.4 or at -2.1 fails.
2. Build C_he: passing HE levers in the order of Germany's paired change, an owner-class lever only if its question (B2-9
   for `own_rg_gap25`, B3-2 for `own_m17_160_64`) is answered or assumed (A) in `docs/TODO.md` now, until the summed HE drops
   reach 2.7. Short of 2.7 with `he_shell_cap6` in the set: play `he_shell_cap4` alone first, and use it in place of the 6 %
   file if it passes rule H. Log the set and the sum either way.
3. Play it: `queue.sh 3001 a+b+c` (one name, parts joined by `+`). It goes on only if HE is at or under 25.8, Germany's change
   not below -2.0 and the floors hold.
4. Build C_all = C_he plus every passing rule V lever (an owner-class one only with B3-3 on (A); never both `own_m36_*`) plus
   `mix_vehicle30` if it passed rule W (its lever level above 55 %: play `mix_vehicle25` alone first and use it if it passes).
   Play C_all the same way; it also needs each heavy card it aims at still up by 5 points. If no rule V or W lever passed,
   C_all does not exist. If there is no C_he, a C_all of heavy and mix levers alone may still be played and named.

## Chunk 4: only if time is left, one at a time, in this order
`he_mission3`, `own_m36_w150`, `diag_head300`, `own_s12_bleed120_300`. For the last two log the HE share, the tank-HE
casualties per side and the own-downs line of `--counters` (stabilised, bled out, killed while down). Say what was not played.

## What the log must hold at the end
Top block (at most 30 lines): BASE commit, world key, block S base (Germany %, HE, no winner 0, median); the screen table:
lever | Germany base and lever % | paired change | up / down | z | HE base -> lever | floors | own counter | verdict (pass H / V
/ W, fail: which part, inert, owner class waiting for which question, diagnostic); the armour-test lines of chunk 0; **C_he
and C_all with their block S lines, or "none", and for each owner-class lever that passed its rule but was left out, the
question it waits for and what it would add**; what was not played. Then the section "B3-1 screen" with the commands run and
anything that went wrong. Do not confirm, apply, or replay on other seeds.

## Report (schema)
commits; BASE and world key; the table; C_he and C_all or "none"; the armour-test result for `own_class_twins`; the goal 4
numbers if played; owner questions the result now depends on; anything the next stage must know (a fixed script, a dropped file).

## Orchestrator command
One background command plays the armour test of `own_class_twins` (base and lever side by side, about 15 min) and then the
screen block 3001-3080 (160 matches a set): the base and the eleven single levers in chunk order (`he_shell_cap6,
own_m17_160_64, own_rg_gap25, hv_save360, own_m36_h1, hv_ger_reach, mix_vehicle30, he_mission3, own_m36_w150, diag_head300,
own_s12_bleed120_300`). The combinations (chunk 3) need the rules and stay for the stage agent.

    tools/run.sh target/b3/run1.log docs/slices/targets/B3-levers/queue.sh run1

Poll with `tools/run.sh --wait target/b3/run1.log` (200 s a call). Expected: about 15 min + 12 sets of 12-25 min = 2.5 to
5 hours on 4 cores (a 4-match test set took 35 s). It is resumable: after a restart run the same line; played sets come from
`target/batch-cache` and a finished armour JSON in `target/b3/armour` is kept. The last line printed says where the summary
lands: `docs/slices/targets/B3-results/paired.txt` (one line per lever), `counters_s3001_NAME.txt`, `armour_own_class_twins.txt`
and the CSVs beside them (commit that folder). Armour result already known (prep, this head): base fail, lever pass; case 45
`jumbo_survives` 0 -> 8 of 10, nothing else moved of 143 checks; so chunk 0 step 5 is done and only needs logging.
A prep test with `SEEDS=2 B3_OUT=target/b3t B3_RESULTS=target/b3t/res` also works.
