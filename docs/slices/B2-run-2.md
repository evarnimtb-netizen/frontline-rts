# B2 run 2: confirm and apply (stage contract; a Tune-tier agent, medium effort; about 60-80 minutes of machine time)

Read first, and only: this file, `B2-balance.md` sections 5, 6 and 8, the top block of `B2-log.md`. **If the top block names no
candidate, do not start: report "nothing passed the screen" and stop.**

## Rules of the stage
- As run 1: no foreground command over 4 minutes (`tools/run.sh`, polls of at most 200 s); `BATCH_JOBS=4`; one queue at a time;
  commit `B2-results/` and the log after each finished set; after a restart read the log and start the same line again.
- Commit only your files with `git commit -m "B2-2: ..." -- FILES`. Never `git add -A`; never `target/`, `.claude/`. Index
  lock: wait and retry. Do not push. No model names or personal details in files or commit text.
- F1 and F2 are spent once a candidate was read on them: no second search there, no third candidate, no relaxed rule.

## Files you own
`docs/slices/B2-log.md`, `docs/slices/targets/B2-results/*`, `docs/slices/targets/B2.toml` (generated), a middle-dose lever
file in `B2-levers/` if rule 4 needs one, and, **only if a candidate passes**, the one or two keys of that lever in
`mods/ww2/defs/rules/skirmish.toml` or in the card's file under `mods/ww2/defs/units/`, plus the two BASELINE refreshes the
content change needs (`sdk/crates/content/tests/resolved_defs.rs`, `crates/simbench/src/report.rs`; `docs/STATE.md` "Versions"
says so) and the comment beside the key.

## Chunk 0 (5 minutes)
`git log --oneline -3`; `tools/batch.sh world` must print run 1's key. If it differs (the base moved): the screen is void.
STOP and report; the orchestrator restarts run 1.

## Chunk 1: fresh blocks (about 40 minutes for one candidate)
C1 is the first candidate of the top block (a lever name, or `a+b`).
`tools/run.sh target/b2/q3.log docs/slices/targets/B2-levers/queue.sh 2101 C1`
`tools/run.sh target/b2/q4.log docs/slices/targets/B2-levers/queue.sh 2201 C1`   (after q3 ends)
Each base must show no match without a winner. Then the three readings (log all three lines):
```
P=docs/slices/targets/B2-levers/paired.py; T=target/b2
python3 -I $P C1@fresh $T/s2101/base.json $T/s2101/C1.json $T/s2201/base.json $T/s2201/C1.json
python3 -I $P C1@all   $T/s2001/base.json $T/s2001/C1.json $T/s2101/base.json $T/s2101/C1.json $T/s2201/base.json $T/s2201/C1.json
```
(If `target/` was lost for block 2001, give the two CSVs of `B2-results/` instead; floors are then read on the fresh blocks.)

**Decision (`B2-balance.md` 5 "Confirm"; all four, no exception):**
1. fresh line: diff at least +4.0, z at least +1.64, both block figures above 0;
2. all line: z at least +2.50;
3. all line: "lever" between 45.0 and 55.0, and `floors ok`;
4. if "lever" on the all line is above 53.0: play the middle dose on 2101 and 2201 (`mix_line42`, or write the midpoint of
   the lever's range as a new file), read its fresh line; it must pass rule 1; take the dose whose all-or-fresh level is
   nearer 50.
C1 fails and the top block names C2: the same for C2 (bases are cached). Both fail, or time is out: change nothing.

## Chunk 2a: nothing passed
Rewrite the log's top block: "B2-2 done; NO default changed", each candidate's three lines, the rule each failed, and the
recommendation to the owner from `B2-balance.md` 9 (B2-2) with the measured owner-class lines of run 1. Commit. Report. Stop.

## Chunk 2b: a candidate passed (about 20 minutes)
1. Edit the default: the lever's key(s) only, with a comment `# B2: was X (fresh 320: +D points, z Z; 480: z Z2)`.
2. `python3 -I docs/slices/targets/B2-levers/rebase_targets.py BASE_MEASURE_DIR --applied > docs/slices/targets/B2.toml`
   (BASE's measure dir from the top block; made before your change, so its floors are BASE's).
3. `CARGO_BUILD_JOBS=4 tools/run.sh target/b2/check.log tools/check.sh quick 8d2731b` (poll). A content BASELINE test fails
   by design after a TOML value change: refresh the two BASELINEs as the test's message says, run the check again. It must
   end "all ok". Any other failure: revert your edit (`git checkout -- FILE`), log it, stop.
4. Proof that only the lever changed: play block 2001 once on the new default with no override,
   `tools/batch.sh target/b2/s2001/applied.json -- skirmish --ai-vs-ai --seed 2001 --seeds 80 --both-bases --summary --minutes 45 --side-detail`
   (background, 10 min), then compare it with the override run, match by match:
   `python3 -I -c "import json;a=json.load(open('target/b2/s2001/applied.json'))['per_match'];b=json.load(open('target/b2/s2001/C1.json'))['per_match'];print(sum(x['hash']==y['hash'] for x,y in zip(a,b)),sum((x['winner_faction'],x['length_s'])==(y['winner_faction'],y['length_s']) for x,y in zip(a,b)),len(a))"`
   It must print `160 160 160` (the new default plays exactly as the override did). `0 160 160` (same matches, every
   hash different) means only the setup bytes differ: log it and go on. Anything else: revert, log, stop.
5. One commit: `git commit -m "B2-2: <key> <old> -> <new> (fresh 320 +D, z Z; 480 z Z2)" -- <the mods file(s)> <the two
   BASELINE files> docs/slices/targets/B2.toml docs/slices/B2-log.md docs/slices/targets/B2-results`. Log its hash: the
   orchestrator reverts exactly this commit if the judge fails.

## What the log must hold at the end
Top block: "B2-2 done", the change (key, old, new) and its commit, or "NO default changed"; the three paired lines per
candidate; the rule check 1-4 with the numbers; `check.sh quick` result; the proof line `160 160 160`; what the orchestrator runs
next, verbatim:
```
BASELINE=<BASE measure dir> tools/measure.sh target/b2-measure docs/slices/targets/B2.toml
tools/batch.sh target/b2-measure/skirmish160_h301.json -- skirmish --ai-vs-ai --seed 301 --seeds 80 --both-bases --summary --minutes 45
```
and the judge's rule (`B2-balance.md` 5 "Judge"). Before and after of the buys a match per side (line cards and the changed
slot or card), from `target/b2/s*/base.json` and `C1.json`: the review reads it.

## Report (schema)
commits; changed key or "none"; the three lines per candidate; rules 1-4 pass or fail each; checks; next step for the
orchestrator; anything incomplete.
