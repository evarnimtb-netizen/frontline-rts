# S15a Build B: Soviet infantry data (faction file, cards, small arms, items, supply), T210 and T211

Read first: `S15-ussr.md` sections 0, 4, 6.1, 6.2, 11, 14; `S15-research-history.md` (figures and their "(verify)" marks);
`S15-log.md` "For the next stage" (Build A's code map: plates, foreign feed, bayonet blocks, rider slots, matchups); the German and
US infantry defs under `mods/ww2/defs/` as the pattern. `<BASE>` for neutrality: the S14 head `96d4b4b`, 40 set `target/s14-F-40`.
Branch `claude/project-thread-aj1tww`. **2 chunks.** No layout change (if a saved field is truly missing, stop and say so in the log).

Owner rule (2026-11-25): **no balance runs, no tuning.** Prices, cooldowns and weights are the plan's starting values, each with a
comment "S15 starting value". Every figure with `sources`; a "(verify)" figure is read from a source before it enters a def (web
search if available), else it keeps the note's value and the def says "unverified".

Work rules: as S15a A (`tools/run.sh`, no command over 4 minutes, `git commit -m "S15a B: ..." -- FILES`, never `git add -A`, never
`target/`, `.claude/`, `tools/__pycache__/`, `godot/match/*.gd.uid`; do not push; no model names in files or commits). 40 sets only
through `tools/batch.sh` with `BATCH_JOBS=4`, **two** in this stage (step 5; owner-approved neutrality checks; if refused, skip and say
so). One short Germany-USSR smoke (4 seeds) is allowed in step 4.

1. Faction file `ww2:ussr` (colour, names), every `ussr_*` id sorting after every existing id (the plan's T211 rule; add the plan's
   sort-order test if Build A did not), small arms and items of 6.2 (Mosin with butt and bayonet blocks, PPSh, PPS, DP-27, Maxim,
   SG-43 without card, TT-33, PTRD-41, ROKS-3 with its fuel and a `when` pair as S14 4.2, 82-BM-37 with the foreign feed, grenades,
   smoke, satchel, UZ charge), cartridges and the four supply crates. Commit.
2. The infantry cards of 6.1 (plates on the assault group, rider capability where the plan says, start squads), crews. No German or
   US def gains an S15 key. Content validation green. Resolved-defs baseline (`BLESS=1`), canonical snapshot. Commit.
3. Tests: the plan's N1 test, A2's empty-slot test, `ptrd_engages_a_halftrack` (risk 4: does a rifle-class AT rifle target vehicles
   on its own under the S8a rule? if not, find the smallest data or rule-flag way, behind a key, and say which), plates and bayonet on
   the real Mosin and SN-42 defs, the ROKS-3 `when` pair off = no flame man. Commit.
4. A 4-seed Germany-USSR smoke with the plan's override (`skirmish --ai-vs-ai --faction0 ww2:germany --faction1 ww2:ussr --seed 1
   --seeds 2 --both-bases --summary --minutes 45`, through `tools/run.sh`): no panic, each match ends; note what the USSR AI bought.
   Fix only data or plain bugs; no tuning.
5. **T210** (the `s15_off` override: `-o docs/slices/targets/S15-ablations/s15_off.toml`) into `target/s15-B-off40/` and **T211** (the
   shipped defaults) into `target/s15-B-40/`, both the default Germany-USA command (`skirmish --ai-vs-ai --seed 1 --seeds 20
   --both-bases --summary --minutes 45`); each must be `python3 -I tools/neutral.py target/s14-F-40 <dir>` same 40/40. A T211
   difference is a bug (a stray key or an id that sorts early): find and fix it.
6. `tools/check.sh quick 96d4b4b` green (IPv6 excepted). Rewrite `S15-log.md` "For the next stage" for Build C (guns, vehicles).

Files owned: `mods/ww2/defs/**` (new `ussr_*` defs, the faction file; existing defs only if the plan says so), `mods/ww2/tests/`,
`sdk/crates/content/**` (tests and validation only where the data needs it), `sdk/crates/content/tests/resolved_defs.rs`, the
canonical snapshot, `crates/sim/src/s15_tests.rs`, `docs/slices/targets/S15*`, `docs/slices/S15-log.md`, `docs/slices/S15-research-history.md`.
