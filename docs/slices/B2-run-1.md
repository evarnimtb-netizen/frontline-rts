# B2 run 1: screen (stage contract; a Tune-tier agent, medium effort; six 160 sets, about 65 minutes of machine time)

Read first, and only: this file, `B2-balance.md` sections 3, 4, 5 and 8, the top block of `B2-log.md`. You measure and log. You
decide nothing beyond the rule in `B2-balance.md` 5 "Screen"; you change no file under `mods/`, `crates/`, `sdk/` or `tools/`.

## Rules of the stage
- No foreground command over 4 minutes. Every set runs through `queue.sh` in the background, started with `tools/run.sh LOG ...`
  and polled with `tools/run.sh --wait LOG` (each call waits at most 200 s). Do other work between polls or just poll.
- `BATCH_JOBS=4` (the queue's default). Do not start two queues at once. Do not run `tools/measure.sh` (the orchestrator's).
- The container can restart. After each finished lever: commit `docs/slices/targets/B2-results/` and `B2-log.md`. After a
  restart: `git log --oneline -5`, read the log's top block, start the same queue line again (played sets come from the cache
  if `target/` survived; otherwise they replay; finished levers already have their line and CSVs in git: skip those names).
- Commit only your files: `git commit -m "B2-1: ..." -- docs/slices/B2-log.md docs/slices/targets/B2-results` (add a lever or
  script file of `B2-levers/` only if you had to fix it). Never `git add -A`; never `target/`, `.claude/`. If git reports an
  index lock, wait 10 s and retry. Do not push. No model names or personal details in files or commit text.
- Numbers come from `paired.py` lines and `--json` files, never from the text tables.

## Files you own
`docs/slices/B2-log.md` (rewrite the top block, append one section "B2-1 screen"), `docs/slices/targets/B2-results/*`,
`target/b2/*`. Fixes to `B2-levers/paired.py` or `queue.sh` only if a key name is wrong (say so in the log).

## Chunk 0: base state (about 10 minutes)
1. `git log --oneline -3`; `git status --short | head`. Note the head commit: it is BASE. If `docs/slices/S11b-log.md`'s top
   block does not say stage C and its measure are done, STOP: report "S11b not finished" (B2 runs on the S11b head).
2. `mkdir -p target/b2`, then `CARGO_BUILD_JOBS=4 tools/run.sh target/b2/build.log cargo build -q --release -p frontline-cli` (poll until done).
3. `tools/batch.sh world` : log the key (run 2 compares it).
4. Find BASE's 480 measure dir (the S11b log names it; `ls -d target/s11b*measure*`). `python3 -I tools/pool.py DIR | head -3`:
   log Germany's pooled share and the three sets. Outside 35-65 %: STOP and report.
5. `docs/slices/targets/B2-levers/queue.sh check` in the background (about 3 min). Every file must print at least one
   `old -> new` value; log the count per file (`own_57mm_truck_tow` must show the `tow` change). A file that changes nothing or fails to load: fix its key (it was written
   without a CLI run), log the fix. `diag_ger_start3` must list four start squads.

## Chunk 1: base and three sets (about 40 minutes)
`tools/run.sh target/b2/q1.log docs/slices/targets/B2-levers/queue.sh 2001 mix_line35 own_57mm_truck_tow diag_ger_start3`
- When `target/b2/s2001/base.json` exists (first 10 min): check `no winner 0`
  (`python3 -I -c "import json;j=json.load(open('target/b2/s2001/base.json'));print(j['wins'])"`); a match without a winner:
  STOP. Then `python3 -I docs/slices/targets/B2-levers/paired.py --detail target/b2/s2001/base.json` and paste its table into
  the log: per faction, men alive at 5 and 10 min and foot squads at 1, 3 and 5 min. Read only the "all" rows for H1 (the
  won / lost rows show the spread; the winner has more of everything late). It prints no `hold_s` or points: those follow
  the winner and test nothing. Run the same `--detail` on `diag_ger_start3.json` when it exists.
- After each lever the queue writes one line to `B2-results/paired.txt` (a resumed queue replaces a lever's line, it does not
  repeat it). Copy it into the log's table with the lever's own counter read from the two JSON files, a match per side
  (`factions.<f>.s4.bought` / 160): for a mix lever the line cards and the changed slot; for a card weight that card; for
  `own_57mm_truck_tow` the US vehicle-MG kills a match (`kills * kills_by_source_pct.vehicle_mg / 100 / 160`) and
  `s7.kills_by_vehicle.usa_m3a1`, and what the AI did with the truck tows (`s8b` truck counters beside the base). Commit.

## Chunk 2: two more sets (about 20 minutes)
`tools/run.sh target/b2/q2.log docs/slices/targets/B2-levers/queue.sh 2001 mix_vehicle30 ger_line_g7_150`
Log and commit each line as above. **These six sets are the stage.**

## Chunk 3: only if the six finished inside 65 minutes, one at a time, in this order (10 minutes each)
1. `mix_line42`, only if `mix_line35` passed the screen with Germany's level above 53 % (the dose rule of `B2-balance.md` 5:
   the dose is chosen on block 2001, here, never in run 2); 2. `ger_line_vg150` (owner class); 3. `own_ger_price95`;
4. `mix_mg30`; 5. `own_pop170`; 6. `arty_share30` (near inert; its counter is the vehicle cards bought a match).
A combination `a+b` of two tier 1 levers may replace items 2-6 if both parts passed alone. Say in the log what was not played.

## What the log must hold at the end
Top block (at most 30 lines, rewritten): BASE commit, world key, BASE's pooled share; the screen table: lever | Germany base
and lever % | paired diff | up / down | z | floors | own counter | verdict (pass, fail: which rule, inert, owner class,
diagnostic); the H1 `--detail` table in four lines of reading and the `diag_ger_start3` line (H1's test); the H3 line (`own_57mm_truck_tow`); **the candidates for run 2, at most two tier 1
names in order, or "none: run 2 is not started"**; what was not played. Then the section "B2-1 screen" with the commands run
and anything that went wrong. Rule check before you write a verdict: gain at least +5.0, predicted sign, `floors ok`, own
counter moved (`B2-balance.md` 5). A lever at +4.9 fails. Do not confirm, apply or replay on other seeds.

## Report (schema)
commits; BASE and world key; the table; candidates or "none"; H1 and H3 readings in two lines each; not played; anything the
next stage must know (a fixed script, a failed file).
