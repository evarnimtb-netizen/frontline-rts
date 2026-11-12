# S10b-ai log (the AI uses mines, obstacles, the bunker; at most 150 lines)

Spec: common `docs/slices/S10b-ai.md`, stages `S10b-ai-build-A.md` ... `-D.md`; targets `targets/S10b-ai.toml`.
Branch `s10b-ai` from main `72b2970`. Another agent may work in this checkout: commit only your files
(`git commit -m ... -- FILES`), never `git add -A`, never the untracked `.claude/`.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)

**Next: Build A chunk 1** (`S10b-ai-build-A.md`, steps 1-7, then STOP POINT 1).
State: Plan done (2026-11-12); no code yet. Main and branch: SAVE 24, PROTOCOL 14.
Build A's layout commit bumps SAVE to 25 once (AiState, FortState::blasts, FortAccount +4, `AiFortJob::start`,
`[ai.obstacles]`, `[fortify]` mine path keys, `[ai.truck] refusal_memory`); PROTOCOL stays 14.
Neutrality bases: 40 set `target/s10b-base40` (skirmish40.json of main code); full `target/s10b-verify`
(Germany 41.67 % of 480; 373 pass, 25 miss, 6 hard incl. T106b, which is a report from now on).
`docs/slices/targets/S10b-ai.toml` parses only after Build A chunk 2 adds `three_parts_from`, `exposure`,
`exposure_min`, `control` to `frontline-cli targets` (until then use `S10b.toml` for old rows).
Chunks: A 2, B 2, C 2, D 3 (one chunk is about 30 self-counted turns; stop at each STOP POINT).
Each stage: targeted release tests through `tools/run.sh`; `tools/check.sh quick 72b2970` once per chunk end;
every new saved field joins the staged save of `crates/sim/tests/save_fuzz.rs` in the stage that fills it.

## Plan (2026-11-12)

Spec (scope, switches, data model, rules R1-R14 and L1-L5, job table, save rules, measures, done-when), four stage
files, targets (S10b body; T106b report; T108 on the all-off control; new T109-T118 in three-part form).
Facts used: own mines never hurt or trigger own units (S10b R7), so mines need no own-route rule and own-mine path
avoidance is pointless under S10b-1 (A); obstacles block every vehicle, so they get the own-route rule (R6);
`build_id` and `content_hash` already fence net peers, so the path rule needs no PROTOCOL bump; the AI's fort job
(Load, Go, Order, Watch) already carries the S10a liveness pieces (F13 stall, `REFUSED_S` parking), so the new
works reuse it as new `WorkKind`s and only sweep and demolish are new job types.
Decided by default (A): table `[ai.obstacles]`; own 3 % S10b materials budget; `jobs_max` 1 inside `builders_max`;
no card weight, price or buy change (the AI already buys Pioniere and US Engineers, who lay and sweep); no urgent
buys for sweepers or demolishers; vehicles avoid AT mines only, men AP only; F8 last and droppable (switch 0 if cut);
two 40-match control runs (`skirmish40_off`, `skirmish40_loose`) in `measure.sh`.
Moved to later slices: own-mine avoidance (with S10b-1 (B)); per-man mine paths; chokepoint analysis; tank or
mortar fire at obstacles and mines; presenter work. Owner questions S10b-ai-1..7 (spec 10).
