# B3 run 2: confirm (stage contract; from `B3-balance.md` 5 "Confirm"; six 160 sets, then the judge measure)

Read first: this file, `B3-balance.md` 5, the top block of `B3-log.md`, `docs/TODO.md` B2-9..12 and B3-1..7.

## Candidates (fixed before any fresh seed is played)
Run 1 left no set; the B3 code stage added `[ai.arty] direct_in_budget`. On block S `code_direct_budget_cap6` passes rule H
(HE 28.01 -> 26.56, -1.45; Germany -0.6, floors ok); `code_direct_budget` alone is a control (+0.0). The orchestrator names the
sets (in place of section 5's block-S gate of 25.8, which no lever sum can be read against without a block-S play):
- **C_he** = `code_direct_budget_cap6` + `own_m17_160_64` (B3-2 (A); HE -0.36 alone, rule H miss logged in run 1).
  File `B3-levers/c_he.toml`.
- **C_all** = C_he + `own_m36_h1` (rule V pass, B3-3 (A)) + `own_class_twins` (armour only, B3-4 (A)). `c_all.toml`.
Not used: `own_rg_gap25` (B2-9 open), `mix_vehicle30` (fails W), every lever that failed its rule.

## Play
`tools/run.sh target/b3/run2.log docs/slices/targets/B3-levers/queue.sh run2`: base, c_he, c_all on F1 (3101-3180) then F2
(3201-3280), both bases, 160 matches a set. Poll with `tools/run.sh --wait`; never block over 4 minutes. At most six 160 sets.
F1 and F2 are spent once read; no set is replayed with another value.

## Decision (no rule is relaxed after a result; a near miss is a miss with its numbers)
On the pooled 320 pairs of F1 + F2:
- **R1** HE share of casualties at or under 26.3 pooled, and under the block's base in each block;
- **R2** Germany's pooled paired change not below -2.0, its level inside 38-62;
- **R3** every floor of section 5 "Screen" (works, wire, mines, mortar share, median 15-30 min, no base side above 62 %,
  infantry gun 90 %, Jumbo bought in at least 10 % of matches);
- **R4** no match without a winner;
- **R5** (C_all only) the M36 above its base share of matches in both blocks.
Apply C_all if R1-R5 hold; else C_he if R1-R4 hold; else nothing (closest set logged).

## Apply (one commit)
The set's values as defaults in `mods/ww2` TOML only (`direct_in_budget = 1` in `skirmish.toml`), with comments; the two
BASELINE refreshes (`resolved_defs.rs`, `simbench/report.rs`) and the snapshot as needed; `tools/check.sh quick 8f9cde6`
green. A test that pins a changed value is reported, not edited. C_all also turns T184d hard in `targets/S13.toml`.

## Judge
`tools/run.sh target/b3/final.log tools/measure.sh target/b3-final docs/slices/targets/S13.toml` (45-70 min), read with
`tools/pool.py`: Germany pooled, HE share of casualties per set and pooled, heavy buys; J1-J5 of section 5 against
`target/s13-on`. Armour test for case 45 (`queue.sh armour`) when `own_class_twins` is applied.
