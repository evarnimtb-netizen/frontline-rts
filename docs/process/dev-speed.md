# Development speed: where S9 went, what changed for S9b

Written 2026-11-09 by the S9b orchestrator. Numbers are measured on this Mac (M3, 4+4 cores, 8 GB) with
`tools/wf_stats.py` on the S9 workflow transcripts and with timed runs of each check on `71cf02f`. "Cost units" weigh
tokens as the price list does (cache write 1.25, cache read 0.1, output 5).

## 1. S9, per stage

| Stage | Model | Wall min | Model working | Waiting on shell | Cost units | of it re-cached |
|---|---|---|---|---|---|---|
| Plan | Opus | 23 | 12 | 11 | 1.4 M | 0.3 M |
| Build A | Opus | 90 | 25 | 65 | 7.0 M | 2.4 M |
| Build B | Opus | 111 | 31 | 80 | 8.9 M | 3.9 M |
| Build C | Opus | 148 | 24 | 125 | 9.5 M | 5.4 M |
| Tune | Sonnet | 131 | 5 | 126 | 2.4 M | 1.7 M |
| Review | Opus | 80 | 14 | 65 | 3.7 M | 2.1 M |
| Fix | Opus | 68 | 1 | 67 | 2.4 M | 1.3 M |
| Docs | Haiku | 3 | 3 | 0 | 0.5 M | 0 |
| **Total** | | **654 (10.9 h)** | **115 (1.9 h)** | **539 (9.0 h)** | **36.0 M** | **17.2 M (48 %)** |

- **Agents waited 82 % of the time.** On what: background jobs 240 min (release suite, 160 sets, tuner), `check.sh
  quick` 125 min, batches in the foreground 60 min, other shell 64 min.
- **Waiting is what cost the tokens.** Workflow agents have a 5-minute prompt cache. 46 turns came back from a wait
  longer than that and wrote their whole context to the cache again: 13.7 M of the 16.0 M cache-write tokens, 48 % of
  all cost units. Tune and Fix were almost only this.
- **Contexts were large.** Build agents averaged 305-376 k tokens per turn (peak 485 k): code read with `sed` (about
  125 k), edit scripts (75 k), and a fixed 65-80 k of tool, skill and MCP listings that every agent starts with
  (about a quarter of all cache reads).

## 2. The pieces, timed

| Piece | Time |
|---|---|
| `cargo fmt --check` / clippy (warm; after a sim edit) | 1 s / 19 s; 26 s |
| Release rebuild after a sim edit: CLI; all 58 test binaries | 22 s; 90 s (sccache is on) |
| All workspace tests, debug (628 tests, what `quick` ran when the sim changed) | 18.5 min |
| All workspace tests, release with the ignored ones (676), one binary after the other | 5.8-6.1 min |
| The same 676 tests, three binaries at a time (`tools/ptest.py`) | 2.1 min |
| 40 set at 3 jobs; one 160 set at 6 jobs; the three sets | 1.1; 4.5; 13.4 min |
| `armour-test --seeds 5`; `--stress --seeds 10`; `fortify-test` | 4.9; 9.8; 1.6 min |
| Old verify script, whole | 42.5 min: check full 6.1, release suite a second time 5.1, sets 14.5, armour and fortify 16.3 |
| 80 traced matches with the gun funnel | 3 min |

The build is not the bottleneck: no linker, incremental or `nextest` change is worth making. Ten tests are 60 % of
the release test time, so binaries side by side give the speed-up without a new tool. `target/` is 47 GB (debug
40 GB) on a disk with 13 GB free.

## 3. The suspected waste, checked

| Suspicion | Verdict | Evidence |
|---|---|---|
| Tune ran about 1 280 matches and ablations, changed nothing | confirmed | 131 min, 2.4 M units, commit `89b05cd` is docs only |
| The same three sets replayed by Build C, Tune, Review, Fix, verify | confirmed | at least 7 plays on 4 binaries, 13.4 min each |
| Tune quoted tuner seeds as if pooled | confirmed | Review had to correct "10 gives 37.5" |
| Build C's +48 % tick time was noise | confirmed | two clean Review runs 0.414 / 0.419; cost the Review about 20 min |
| Two stages ended on a fmt failure, last check not rerun | confirmed | Build C and Fix reports |
| Plan missed the 200 % size rule | confirmed, one case | T76b changed by Build A |
| A Build agent returned a one-line report | not found in S9 | all eight reports are 1.4-7.7 kB |
| Docs wrote stale lines | confirmed | "Results" section fixed by the next stage |
| Verify runs everything serially, some of it twice | confirmed | release suite and Godot tests ran twice |

## 4. Changes

| # | Change | Expected saving | Risk | State |
|---|---|---|---|---|
| 1 | Rule: no agent command blocks longer than 4 min; longer jobs run in the background and are polled in steps of at most 200 s | most of the 48 % re-cache cost | a poll costs a cache read; an agent may forget the rule | applied (prompt rule), measured by `wf_stats.py` |
| 2 | `check.sh quick` tests in release, binaries three at a time, and applies `cargo fmt` | 10 min to about 3 per run; no fmt-after-check | a test that fails only under load is run again alone and reported | applied |
| 3 | `tools/batch.sh`: deterministic runs cached by binary + mods + overrides + args | each replay of the sets, 13.4 min | cached tick times are stale: timing only from `verify.sh` | applied |
| 4 | `tools/measure.sh` + `tools/pool.py`: the pooled reading in 30 lines, one definition for every stage | no agent parses JSON by hand or mixes seed sets | none found | applied |
| 5 | Ablations by script before any Tune (feature off, one side, a knob's ends); Tune only if a knob moves the pool; Review and Fix come before Tune | S9's Tune stage (131 min) and a Tune made stale by the Fix | a rare slice may need a second Review after a late value change | applied |
| 6 | `tools/verify.sh`: release suite and Godot once, armour and fortify beside the sets, sets from the cache | 42.5 min to about 20 | none: same checks on the same commit | applied |
| 7 | Tick time is measured once, by `verify.sh`, alone; agents never report it | the Build C scare | a real regression shows one stage later | applied |
| 8 | Stage reports through a schema (commits, checks, numbers, incomplete, next must know); Docs copies numbers from the measure file | thin reports, stale doc lines | none | applied |
| 9 | Short slices: the orchestrator writes the spec from a measured diagnosis; no Plan agent | 23 min, 1.4 M units | orchestrator time | applied to S9b |
| 10 | Each new numeric target cites the rule or data line it comes from | the T76b kind of miss | none | applied (prompt rule) |
| 11 | Build stages of at most about 50 turns, split by subsystem with a written hand-over | cache reads grow with the square of a stage's length: about 15 % | hand-over loss | not needed in S9b; for S10 |
| 12 | Delete `target/debug` (40 GB): nothing but clippy uses it now | disk; `sccache` refills what is needed | 2 min rebuild once | owner approved; done after S9b's verify |
| 13 | Fewer MCP servers, plugins and skills in this project's sessions | 65-80 k tokens on every agent turn: 15-20 % | changes the owner's setup | owner asked for the list (S9b report); nothing changed |
| 14 | Engine slices split into a mechanics slice (cards at `ai_weight` 0, proven neutral) and an AI and balance slice | S9's Tune, Fix and S9b were all the AI half | more owner stops | owner approved: from S10 on |
| 15 | Parallel worktrees for sim, presenter, docs | none here | 8 GB RAM, 13 GB free, one target dir serialises cargo | rejected for this machine |

## 5. Models (owner rule 2026-11-09: Fable orchestrates, Opus builds and reviews, Sonnet measures and tunes, Haiku documents)

In S9 the two cheaper stages each cost a correction, both given raw reports to interpret. From S9b on the cheaper
models get scripts that print the numbers (`measure.sh`, `pool.py`, `gun_trace.sh`) and a decision rule.

## 6. S9b against S9

| Stage | Model | Wall min | Model working | Cost units | of it re-cached |
|---|---|---|---|---|---|
| Build, cut off when the session ended | Opus | 23 | 9 | 1.6 M | 0 |
| Build, retry from the hand-over | Opus | 38 | 11 | 1.2 M | 0 |
| Budget (four values, 480 matches each) | Sonnet | 42 | 0 | 0.4 M | 0.10 M |
| Review | Opus | 13 | 9 | 0.8 M | 0 |
| Fix | Opus | 17 | 5 | 0.8 M | 0 |
| Measure, twice | Sonnet | 48 | 0 | 0.5 M | 0.09 M |
| Docs | Haiku | 5 | 3 | 0.9 M | 0 |
| **Total** | | **186 (3.1 h)** | **37** | **6.2 M** | **0.2 M (3 %)** |

S9b is a smaller slice: totals are not like for like, rates are. S9: 654 min, 36.0 M units, 48 % re-cached, about 11 % of the weekly limit. S9b: about 7 % of the weekly limit (20 to 27 %),
including this process review and the 1.6 M of the cut-off Build. "Cost units" weigh token kinds, not model prices.

After the workflow the orchestrator spent about 90 min, mostly unattended: the save bug, two 480-match measures by
script, the verify (33 min against 42.5, with the suite still serial, 504 s of it). Polling instead of blocking took
re-caching from 48 % to 3 %; nothing was replayed by hand; tick time was read once and was clean.

Where the process still failed, all caught late:

- Review found the T88 count inflated after Budget had chosen on it. A new measure needs its own test before a stage
  decides on it.
- Fix's last check was `quick`, without the ignored tests. The failing save test surfaced two stages later and the
  workflow still ran Docs. The workflow gate must stop on `tests_ok: false`.
- Docs ran before verify and had to be redone. Docs belongs after the orchestrator's verify.

**Models.** Sonnet measuring by script was right in its numbers, but spent 90 min polling what a shell script does for
nothing: from S10 the orchestrator runs `measure.sh` between stages. Sonnet's Budget ran the ablations correctly, but
Opus Review had to correct its conclusion, so a cheap Tune needs a Review after it. Haiku Docs was accurate, at more
cost units than Review (33 turns of small edits): give it the exact text. Opus Build, Review and Fix needed no rework
beyond the above.

## 7. Audit 2026-11-11

Full results: `target/process-audit.jsonl`.

- Build stages are 70 % of S10a-ai's agent cost and 51 % of S10a's. They ran 120-231 turns each, with peak context 258-433 k tokens.
- 38 % of Build, Fix and Plan turns are runs of single read-only calls.
- `fort.rs` was read 50 times by 17 agents. Waiting on background jobs is 9 % of cost; prompt text is under 1 %.

Adopted from the next slice:

- Build stages capped at about 60 turns, with a written hand-over.
- Agents batch reads and read an outline before ranges: `tools/outline.sh` and `tools/show.sh` (to be written).
- One start-and-wait script, `tools/run.sh` (to be written).
- Each stage reads only its spec part and the last hand-over block.
- Pin the known seed in the slow seed-search tests (ai_guns, trucks, transport, ai_fort).
- Switches and measures go into Build A.
- A reusable save/load roundtrip-and-fuzz test.
- One deadline-and-refusal rule for AI jobs, with a generic liveness test.
- Every new test names the control that makes it fail.

Not adopted: one review lens instead of three (revisit after the fuzz test); early stop of the 480 measure (ablations
only); reuse of cached sets after a 40-match check.

The small smoke step used the batching rule and took 58 turns.
