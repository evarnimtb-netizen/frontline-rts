# B1: balance pass (plan, at most 140 lines)

Branch `balance-pass`. Targets `docs/slices/targets/B1.toml` (S10b-ai.toml plus a held-out set, see 3). Log `B1-log.md`.
Owner limits: no change to armour, penetration, guns, ammo, lethality or the price of any existing card. SAVE 25 and
PROTOCOL 14 stay. Minor scope questions are settled on the default and reported; how-it-should-play choices are asked.

## 1. Starting point and where the gap comes from

- Germany pooled 41.88 % of 480 (sets 43.75 / 41.25 / 40.62), `target/s10b-ai-verify`. Pool noise about +-4.5 points,
  so 41.88 sits about 3 points under the 45 floor. No match without a winner. 410 pass, 46 miss, 26 hard.
- History: 45-52 % from S5b to S10a (48.75 % at S10a), one step to 41.67 % at S10a-ai, unchanged since.
- S10a-ai ablations (480 each): all switches off 48.75 %, `works_off` 47.08 %. Wire, breach, cards, nest, dugout,
  roof, sandbags off one at a time: 37.5-43 %. So point building as a whole is the lever, not one work kind.
  `works_off` also stops point wire and the +80 builder hold (S10a-ai log D4).
- Likeliest mechanism (S10a-ai D5, not measured): the fort AI holds an engineer at an own point while the point
  "wants works" (`crates/sim/src/ai/fort.rs` `fort_holds`). Germany's held builder is often its only SMG and satchel
  engineer. A refused work keeps wanting, so the hold can last the whole match.
- The loss is infantry output: rifle rounds 359k vs USA 687k, kill share 43.8 %. Tank losses are even (1997 / 1976),
  Germany's guns do better (0.61 vs 0.34 armour out per match). Germany sits at the pop cap far more (5015 vs 74).
- Short matches carry the gap: under 20 min Germany wins 37.4 %, at 20 min or more 48.5 %. Base side is not the
  cause (its lean flips sign between sets).

## 2. Levers

Allowed by default (TOML only, AI habits or card buy weights; none is a price, armour, gun, ammo or lethality value):

| # | Key | File, section | Now | Range | Why it may help |
|---|---|---|---|---|---|
| L1 | `builders_max` | `mods/ww2/defs/rules/skirmish.toml` `[ai.fort]` | 2 | 1-2 | fewer engineers held by fort jobs |
| L2 | `works_per_point` | same | 3 | 1-3 | a point stops wanting works sooner, hold ends |
| L3 | `build_threat_max` | same | 4 | 2-4 | no building (and no hold) at a contested point |
| L4 | `builder_reach_m` | same | 200 | 120-200 | builders not pulled far from the line |
| L5 | `line`, `engineer` | `skirmish.toml` `[ai.mix]` | 50, 15 | 50-70, 10-15 | more rifle squads, the lost output |
| L6 | `ai_weight` StuG | `defs/units/ger_stug3_g_squad.toml` | 100 | 100-150 | T54e StuG bought 75 % vs 80 |
| L7 | `ai_weight` 251/1 | `defs/units/ger_sdkfz251_1_squad.toml` | 100 | 100-130 | T40a/c, T72a 58.75 % vs 60 |
| L8 | `mortar` | `skirmish.toml` `[ai.mix]` | 50 | 50-60 | T8b oos HE share 19.1 vs 20 (S6: +0.6 points per 4) |

L1-L5 and L8 are symmetric values whose effect differs because the rosters differ. L6 and L7 are per faction, like the
S7-2 and S8a-21 weight choices that were settled on the default. S8a-21 found 150 on the half-track cut HE kills, hence
the lower cap on L7. Fort switches stay 1: switching works off would close the gap by deleting a feature and would
break T94-T97. A lever that drops any of T94-T97 below its floor is rejected.
Only with an owner yes (section 6, ask): engineer card weights (Pioniere 40, Bau 100, Sturm 60 are an owner decision of
S10a-ai); a small AI code change to the builder hold; new per-faction rule values (Germany income or tickets; no such key
exists, it would be a new rule); card `cooldown_s`, `max_alive`, `unlock_s`.
Not moved: armour, penetration, weapons and gun cards, `[ai.guns]` (the owner's "guns"; S9b measured it, 35 cost more
hard misses), ammo loads, `[supply]` prices and `[ai.supply]`, lethality, every card `cost` (S8b, S9, S10 cards
included), squad sizes and members, `start_squads`, `[economy]`, tickets, pop cap, capture, the map, any `[ai.fort]`
or `[ai.obstacles]` switch, SAVE, PROTOCOL, the save layout, tick order, RNG use.

## 3. Method

Tools: `tools/batch.sh` A/B screens, `tools/autotune` for the solve, `tools/measure.sh` and `tools/pool.py` for every
reported number. Overrides in `docs/slices/targets/B1-levers/`. Seeds are split so the pass does not fit its judges:
- tuning and screening: seeds 1001 and up only (screens 1001-1080, autotune `seed_start` 1101);
- measurement: the three 160 sets (1-80, 101-180, 201-280), used to confirm, never to search;
- held out: seeds 301-380, both bases (run `skirmish160_h301`, row T5f), played once on the frozen change set.

Search order and machine time (`--jobs 6`, about 1.25 s per 45-minute match; start in the background, poll in steps of
at most 200 s):
1. Smoke: each lever at the far end of its range, 40 matches (seeds 1001-1020), about 1 min each. Kept if no-winner
   stays 0 and its own counters move.
2. Screen: each kept lever at the far end on 160 matches (seeds 1001-1080), paired against an unchanged base on the
   same seeds, read with `tools/pool.py BASE DIR`. About 3.5 min each, about 25 min, plus 3.5 min for the base.
   A lever goes on if Germany moves at least +3 points with T94-T97 counters still above their floors.
3. Solve: autotune on the one or two best levers (L1-L5), target `share_germany` 47-53 (inside 45-55 with room),
   checks `base_max`, `len_*`, `he_mean`. One knob about 550 matches, 11 min; two knobs about 700, 15 min.
   L6-L8 are set after, one at a time, by a 160 paired screen each (they barely touch the share).
4. Confirm: `BASELINE=target/s10b-ai-verify tools/measure.sh target/b1-measure docs/slices/targets/B1.toml`. About
   13-15 min uncached. One measure per accepted change set, not per try.
5. Held out: `tools/batch.sh target/b1-measure/skirmish160_h301.json -- skirmish --ai-vs-ai --seed 301 --seeds 80
   --both-bases --summary --minutes 45`, then `frontline-cli targets` for T5f. About 3.5 min. Played once. If it
   fails, no retune on it: the stage stops and reports.
6. Tick time and the full suite only from the orchestrator's `tools/verify.sh`. Pass total, verify excluded: 1.5-2 h.

Acceptance (all must hold):
- Germany pooled 480 inside 45-55; each 160 set inside 40-60 (set noise is about +-7.7);
- held-out 160 inside 40-60, and the 640 pool (480 plus held out) inside 45-55;
- no match without a winner; no hard target that passes in `target/s10b-ai-verify` misses; T94-T97 pass;
- re-labels (section 5) listed apart, never counted as closed; tick p99 (verify.sh) at most 10 % over 0.506 ms;
- every change has a before and after from `pool.py` or `frontline-cli targets` in the log.
What replaces the all-switches-off neutrality (defaults move, so main's numbers no longer repeat):
- revert check: the 40 set with `-o B1-levers/revert.toml` (every changed key at its old value) must give the same
  hashes as `target/s10b-ai-verify` 40 set (`tools/neutral.py`). This proves only the listed keys changed;
- `skirmish40_off` is re-recorded as the B1 control and must still show zero fort and obstacle use (T108);
- determinism as always: verify.sh's repeat runs give equal hashes.

## 4. Stages (each about 35 agent turns; `CARGO_BUILD_JOBS=4`; no command blocks over 4 min)

- B1-0 Owner answers. STOP until the "ask" questions of section 6 are answered. No runs before.
- B1-1 Screen (Sonnet, medium effort). Write the lever override files and `revert.toml`, run steps 1-2, log one row
  per lever. STOP and report to the owner if no allowed lever moves Germany +3 points on the screen.
- B1-2 Tune (Sonnet, medium). Spec `tools/autotune/specs/b1_tune.toml`, step 3, copy the solved values into `mods/ww2`,
  step 4. STOP if the pool is outside 45-55 or a hard target got worse; report the solved level with its interval.
- B1-3 Secondary targets (Sonnet, medium). L6, L7, L8 one at a time (160 screens), then one step 4 measure, the revert
  check, the soft per-unit look (T31, T55, T9b, T33, T54 reads, before and after), then step 5 once. STOP if the held
  out set fails.
- B1-4 Review (Opus, short). Diff against the owner limits, the seed split and the acceptance list.
- B1-5 Verify (orchestrator): `tools/verify.sh target/b1-verify`, tick p99, settings.cfg unchanged.
- B1-6 Docs (Haiku, low allowed): STATE.md, TODO.md, log at most 150 lines. A code lever (owner yes in B1-0) adds an
  Opus Build stage before B1-1 and the Review gate of workflows.md.

## 5. Hard misses: try to close, or re-label

Try to close: Germany's pooled share (T5c-e plus the log); T54e StuG bought (L6); T40a, T40c, T72a 251/1 bought (L7);
T8b oos HE share (L8, or as a side effect of more line squads).
Re-label as report (in B1.toml, with a note), each with its reason:
- T88b, T88d USA gun armour out: the only lever is `[ai.guns]`, which the pass does not move (owner "guns" limit;
  S9b-1 left it to the AI half; Germany's guns already do better). Re-labelled only if the owner agrees (ask Q4).
- T6a 40-set base_max 62.5 vs 62: one match of noise; kept hard, not chased (T6b-c on the 160 sets pass).
- T109, T110a-d, T112a/c, T113a/c, T114e/f AI use: not balance; follow the owner's E1, E3, E6, E8 answers.
- T93a/b/f/g, T106b fortify mechanics: fortify-test rows independent of the share; stay as they are.
- T8b becomes a report only if L8 fails and the three-set mean is inside 20-26.3.

## 6. Owner questions

Ask (how it should play):
- Q1 How should Germany's gap be closed? (A) AI habits only: fort builder keys and `[ai.mix]`, symmetric values, TOML
  only (recommended first try). (B) Germany's AI buys differently: per-faction card weights, including the S10a-ai
  engineer weights. (C) A small AI code fix to the builder hold (a refused work stops wanting; the hold skips the
  last satchel engineer), an engine change with review, no layout change. (D) Faction-specific rule values (new keys).
- Q2 Are S10b-ai-E1, E3, E6 (and E8) settled before balance or after? They change what is balanced: more mines,
  obstacles or bunkers will move the share again. Recommended: before; then this pass balances the game as it plays.
- Q3 May the fort AI do less (fewer works per point, fewer builders) if that closes the gap? T94-T97 floors stay.
- Q4 T88b/d: accept USA gun results as a report, or open `[ai.guns]` to this pass?
Decided by default (reported only):
- S8b, S9 and S10 card prices count as existing prices and are not moved.
- Vehicle card weights (L6, L7) may move inside their ranges, as S7-2 and S8a-21 did.
- Cooldowns, `max_alive`, `unlock_s` are not moved.
- Seeds 301-380 are the held-out set (row T5f, report, band 40-60); tuning uses 1001 and up.
- Soft per-unit targets (T31, T55, T9b) are read and reported, not tuned. T6a kept hard, not chased.

## 7. Risks

- Fitting noise: a 3-point gap is under one set's noise. Guarded by the seed split and the held-out set.
- The lever deletes the feature: lower builder keys may close the gap by building nothing. Guarded by T94-T97.
- The mechanism is a guess (D5): if no TOML lever moves Germany, only Q1 (B)-(D) is left; stop and ask.
- E1, E3, E6 answered after the pass would undo it; Q2 is asked first. Not in the data: income, end cause, buy timing.
- Symmetric keys move USA too (T95b/d USA wire 57.5 / 56.25 vs 60). Side effects on other hard rows (S9b: one lever
  turned 10 misses into 17): every measure diffs the targets.
- Verify time: the last parallel verify took 249 min, cause not examined.
