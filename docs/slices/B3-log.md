# B3 log (third balance pass; at most 150 lines)

Plan `docs/slices/B3-balance.md`; stage contracts `B3-run-1.md`, `B3-run-2.md`; levers, `paired.py`, `queue.sh` in `targets/B3-levers/`;
results `targets/B3-results/`. Branch `claude/project-thread-aj1tww`. Another agent may work in this checkout: commit only
your files (`git commit -m ... -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: B3 run 2 done 2026-11-22: C_all confirmed and applied at `f4e610b` (contract `B3-run-2.md`); the judge measure
`target/b3-final` (480) misses J1 and J4: by the plan the apply is to be reverted (`git revert f4e610b`); orchestrator's call.**
- Applied (TOML): `[ai.arty] shell_mp_max_pct` 25 -> 6, `direct_in_budget` 0 -> 1; M17 burst 200 cm / 96 -> 160 / 64; M36
  `ai_heavy`; 7.5 cm PzGr 40 `apcr`, 3-inch M62 `apcbc`; T184d hard. Snapshot blessed; setup BASELINE hashes unchanged.
- Confirm (320 fresh pairs, F1 3101 + F2 3201): C_all HE 27.90 -> 26.05 (27.62 -> 26.24, 28.17 -> 25.86), Germany 49.1 ->
  49.1 (+0.0; +1.2 / -1.2), floors ok, no winner 0; M36 0.6 -> 36.9 / 0.0 -> 43.8 %, Jumbo 12.5 / 15.6 %. R1-R5 hold.
  C_he also held R1-R4 (26.29 pooled; 26.63 / 25.95; Germany -0.3).
- **Judge (`f4e610b`, 480, against `target/s13-on`):** Germany 47.29 % (50.62 / 41.25 / 50.00; +5.2), no winner 0. HE share of
  casualties 27.40 / 24.96 / 26.70, about 26.35 pooled (s13-on 28.0 / 28.3 / 28.2): **J1 MISS** (T8a 27.40; T8b 24.96 pass;
  T8c, the 40 set, 26.91). J2 pass (each set 38-62). J3 pass. **J4 MISS**: hard rows passing at s13-on now missing: T50b (USA
  ride saving -1.8 s), T119b (USA front mines 37.4 % < 40), T121d (USA mines set off 0.28 < 0.3, sibling of the noisy T121b),
  T122c (German wire seen 46.4 % < 50); fixed: T8b, T50f, T121b, T179a-c. J5: T179a-d pass (Panther 28.75, M36 41.25, Tiger
  14.4, Jumbo 16.25 %). Heavy buys (3 sets) G 141 -> 220, U 184 -> 304. Hard misses 51 -> 49.
- Armour test at `f4e610b` (5 seeds): pass False -> True; case 45 `jumbo_survives` 0 -> 8 of 10; case 9 trench casualties
  27.5 -> 21.95 % (still passes; the M17 cut). `B3-results/armour_final_f4e610b.txt`.
- The 5101-5180 held-out set is NOT played (it follows a judge pass).
- Tests edited in the apply (the orchestrator asked for check green; the plan says such tests are reported): 6 pinned values
  (b3 layout, s13 setup keys, t151h, s13_content, s11a1_content, ballistics golden row 44: PzGr 40 vs M4A3 turret 1000 m
  30 deg pen -> 9.6 %), and 3 match-trajectory tests that restore the M17's old burst in their own setup (s10a empty fort:
  0 throws; ai_vs_ai_plays_by_the_rules: 7 of 299 rejected, ordinary races, none a SetAmbush; save_fuzz s11b roundtrip).
  A revert of `f4e610b` takes them back with the values.
- Sets played: 6 (base, c_he, c_all on 3101 and 3201) + the judge measure + 1 armour test. F1, F2 spent.

## B3-1 screen (2026-11-22; stage agent; block S 3001-3080, `BATCH_JOBS=4`)
Run 1 screen table (block S base 3001-3080, 160: Germany 46.2 %, HE 28.01, no winner 0, median 19.7 min):

| lever | Ger base -> lever | change | up/down | z | HE | floors | own counter (G / U) | verdict |
|---|---|---|---|---|---|---|---|---|
| `he_shell_cap6` | 46.2 -> 40.0 | -6.2 | 5/15 | -2.24 | 28.01 -> 28.67 | FAIL HE | base-gun rds 27.1->19.6 / 19.8->11.6, holds 0->106 / 166 | fail H (HE up, Germany) |
| `own_m17_160_64` | 46.2 -> 46.9 | +0.6 | 26/25 | +0.14 | 28.01 -> 27.65 | ok | US RG casualties 5.72->4.72 | fail H (HE -0.36 < 0.5) |
| `own_rg_gap25` | 46.2 -> 43.1 | -3.1 | 25/30 | -0.67 | 28.01 -> 27.45 | ok | RG fired 6.28->5.61 / 8.01->7.27 | fail H (Germany -3.1); B2-9 open |
| `he_mission3` | 46.2 -> 43.8 | -2.5 | 14/18 | -0.71 | 28.01 -> 27.88 | ok | base-gun rds 27.1->22.2 / 19.8->16.4 | fail H (HE -0.13, Germany -2.5) |
| `hv_save360` | 46.2 -> 46.9 | +0.6 | 2/1 | +0.58 | 28.01 -> 28.07 | ok | drops 21->3 / 31->11; Panther +1.2, Tiger +0.6, Jumbo +1.2 | fail V (no card +5) |
| `own_m36_h1` | 46.2 -> 45.0 | -1.2 | 5/7 | -0.58 | 28.01 -> 27.87 | ok (Jumbo 10.0, at floor) | M36 0.0->34.4 %, Jumbo 27.5->10.0 | **pass V** (B3-3 (A)) |
| `hv_ger_reach` | 46.2 -> 45.0 | -1.2 | 1/3 | -1.00 | 28.01 -> 28.10 | ok | Panther 24.4->27.5, Tiger 8.1->13.1 | fail V (Panther +3.1; Tiger +5.0 at the line) |
| `mix_vehicle30` | 46.2 -> 46.2 | +0.0 | 38/38 | +0.00 | 28.01 -> 28.14 | ok | vehicle kills 35.8->41.7 / 28.9->32.5 % | fail W (+0.0 < +5) |
| `own_m36_w150` | 46.2 -> 42.5 | -3.8 | 5/11 | -1.50 | 28.01 -> 27.95 | ok | M36 36.2 %, M4A3 75 0.47->0.05 a match | fail V (Germany -3.8) |
| `diag_head300` | 46.2 -> 43.8 | -2.5 | 31/35 | -0.49 | 28.01 -> 26.59 | ok | tank HE 14.64->13.60 / 7.78->7.87 | diagnostic |
| `own_s12_bleed120_300` | 46.2 -> 41.2 | -5.0 | 13/21 | -1.37 | 28.01 -> 28.58 | FAIL no-winner 1, HE | stabilised 3.6->5.4 / 4.0->6.4 % | fail floors; B3-5 report |

### Chunk 0
- Head `884d46e`; STATE.md: S13 measured at `3b3387f`, Germany 42.08 %; no change to `mods crates sdk` since. Build done by
  the orchestrator (`target/b3/build.log`). `tools/batch.sh world` = `ba09de8def971642465e0c5f`.
- `python3 -I tools/pool.py target/s13-on`: Germany pooled 42.08 % of 480; sets 43.12 / 40.00 / 43.12; no winner 0; HE share
  of casualties 28.0 / 28.3 / 28.2 (T8 cap 26.3).
- `queue.sh check` (`target/b3/check.log`): 19 `old -> new` values, as expected: `hv_ger_reach` 3, `own_m17_160_64` 2,
  `own_class_twins` 2, `own_s12_bleed120_300` 2, every other file 1 (incl. `he_shell_cap4`, `mix_vehicle25`). No file fixed.
- Armour: `B3-results/armour_own_class_twins.txt`: pass False -> True; case 45 `jumbo_survives` 0 -> 8 of 10; 1 of 143
  checks moved. paired.py read the JSON; no fix.
### Chunks 1, 2, 4 (one orchestrator queue: `queue.sh run1`, `target/b3/run1.log`; all 12 sets, exit 0)
- B2-12 is not answered (B), so chunk 1 ran its levers. Base stop rule: Germany 46.2 % inside 35-65, no winner 0: go on.
- Base `--counters` (a match): Germany caused 64.1 casualties, HE 36.0 %: tank HE 14.64, mortar 5.50, rifle grenade 2.96;
  USA caused 79.5, HE 22.7 %: tank HE 7.78, mortar 4.54, rifle grenade 5.72. Tank HE includes the guns' kills
  (`gun_kills_in_tank_he`): G 7.05 of 15.38, U 5.49 of 8.34 tank-HE kills a match. S13 cards in matches: Panther 24.4 %,
  Tiger 8.1 %, M36 0.0 %, M4A3 76W 0.0 %, Jumbo 27.5 %. Heavy plans / buys / drops G 130 / 53 / 21, U 137 / 50 / 31.
- `he_shell_cap6`: shells % of income 7.26->6.42 / 9.47->7.93; budget holds 0->106.2 / 165.9 a match; gun missions
  6.33->4.60 / 3.41->1.52; `s11b_derived.how_fired_matches_pct` 90.7->89.4 / 91.3->87.3; `s11a2_derived.cb_fired_matches_pct`
  75.3->61.7 / 92.8->88.8.
- **Why the 6 % cap RAISED HE and cost Germany 6.2 (from the JSON, a match, G / U):** the cap holds only budgeted missions.
  Map and observed gun rounds fell (G 27.1->19.6, U 19.8->11.6), counter-battery missions fell (2.22->1.27 / 2.88->1.81) and
  gun crews lost halved (1.11->0.61 / 0.62->0.33). But the guns then fire over open sights, which the budget does not hold:
  direct rounds 10.4->14.7 / 6.9->11.1, and direct fire kills more a round. Gun kills ROSE 7.05->7.78 / 5.49->6.16 (howitzer
  kills 2.83->3.06 / 1.94->2.09); tank-only HE kills stayed (8.33->8.41 / 2.86->2.96). So HE casualties went up, not down.
  Germany: the USA (shells 9.5 % of its income, the larger share) turned the saved money into armour: heavy buys 50->65,
  Jumbo 27.5->35.0 % of matches; Germany's heavy buys only 53->58. Germany leans on HE (36 % of what it causes) and lost
  its counter-battery edge (cb fired 75.3->61.7 %). The cap is the wrong tool while direct gun fire sits outside it.
- `own_m17_160_64`: US rifle-grenade casualties 5.72->4.72 (outright 3.47->2.68, downs 2.25->2.04), German 2.96->2.73;
  predicted HE -0.7 to -1.0, read -0.36.
- `own_rg_gap25`: grenades fired 6.28->5.61 / 8.01->7.27; HE -0.56 (B2 read -0.8 to -1.0); Germany -3.1 here against +2.5
  in B2 (inside the 4.9-point SE, but the rule is the rule).
- `he_mission3`: base-gun rounds 27.1->22.2 / 19.8->16.4, gun missions 6.33->6.97 (more, shorter missions); HE -0.13.
- `hv_save360`: drops 21->3 / 31->11; buys 53->60 / 50->51; cards Panther 25.6, Tiger 8.8, Jumbo 28.8 %.
- `own_m36_h1`: M36 34.4 %, Jumbo 27.5->10.0 % (T179d floor 10: held at the line, 16 of 160); US heavy plans / buys / drops
  137 / 50 / 31 -> 206 / 80 / 42. German cards Panther 21.9, Tiger 7.5 %.
- `hv_ger_reach`: Panther 24.4->27.5 % (+3.1, 39 -> 44 matches), Tiger 8.1->13.1 % (+5.0, 13 -> 21); German heavy buys
  53->72, drops 21->16. Read as fail: the lever aims at both cards and the Panther is short (C_all's own test is "each
  heavy card it aims at"). It would not change the result: there is no C_he and no run-2 gate it could pass.
- `mix_vehicle30` (logged, T31 no floor): vehicle and gun cards a match G 8.11->9.03, U 7.00->7.81; mortar share of kills
  G 13.0->10.4, U 10.0->9.6 %; all vehicles' share of kills G 35.8->41.7, U 28.9->32.5 %. Cards: Panther 40.6, Tiger 14.4,
  Jumbo 40.6, M36 3.1 %. Heavy drops 21->92 / 31->62. B2's +8.8 is not repeated (+0.0).
- `own_m36_w150`: M36 36.2 %, M4A3 75 0.47->0.05 a match, Jumbo 26.9 %; German Panther 17.5, Tiger 3.8 %.
- `diag_head300`: HE 26.59 (-1.42); tank-HE casualties G 14.64->13.60 (downs 6.86->5.54, outright 7.78->8.06), U 7.78->7.87;
  own downs G stabilised 3.6 / bled out 49.6 / killed while down 41.2 %, U 4.2 / 48.1 / 41.5 %. More die outright, fewer
  downs are finished: the HE share falls because head hits kill by rifle, not by blast.
- `own_s12_bleed120_300`: one match without a winner; HE 28.58; own downs G stabilised 3.6->5.4, bled out 49.3->39.5,
  killed while down 41.9->46.4 %; U 4.0->6.4, 47.6->37.9, 42.5->47.1 %. Stabilised x1.5 / x1.6, not doubled: the longer
  window mostly gives the enemy longer to finish the man (B3-5: report only).
### Chunk 3
- Rule H: no lever passes (above). C_he: none, sum 0; `he_shell_cap4` not played (its rule needs cap6 in the set, and cap6
  moved HE the wrong way through direct gun fire, which a lower cap would push further).
- Rule V: `own_m36_h1` passes. Rule W: `mix_vehicle30` fails; base under 55 %, so no `mix_vehicle25`.
- C_all: none (no C_he; a heavy-only set is `own_m36_h1` alone, already read: HE 27.87 > 25.8).
- Owner-class levers that passed their rule but are left out: `own_m36_h1` (B3-3 (A)): would add the M36 in 34 % of
  matches at the cost of the Jumbo falling to its floor; waits only on an HE fix, not on a question.
  `own_rg_gap25` passed the HE part of rule H (-0.56) but not Germany (-3.1); it also waits for B2-9.
- Nothing went wrong; no script fixed; no set replayed. Extra sets played by the stage: 0.
- Run 2 (moved from the top by the code stage): nothing to confirm under current rules. With B2-12 (B) at a cap the base meets, it would confirm `own_m36_h1`
  (R5: M36 up and Jumbo >= 10 in both fresh blocks) and, with B2-9 (A), retest `own_rg_gap25`. Not played: `he_shell_cap4`,
  `mix_vehicle25` (base 46.2 % < 55), every combination.

## B3 code (2026-11-22; stage agent; owner option pending, recommended; reversible: revert `073b577`)
- Rule (`crates/sim/src/ai/arty.rs` `direct_budget`, before A6 each plan, emplaced AI `inf_gun` only: leIG, leFH, 75 mm
  M1A1, 105 mm M3, M2A1): with `direct_in_budget` 1 and gun shells over `shell_mp_max_pct` of the income so far (A5's
  test, now `over_shell_budget`), the AI sends `SetAmbush` at `[vehicles.guns] self_defence_m` (40 m): the sim's ambush
  hold (`guns::holds_fire`) stops direct fire at men and vehicles beyond it, and breaks when the gun is struck. A known
  enemy vehicle within `[ai.guns] engage_still_m` (400 m) or the budget back in reach sends `SetAmbush 0`. Missions are
  not touched. "Charged": direct rounds were already bought into `gun_shell_micro` at base rearm; now the budget limits
  them too. Counter `arty_direct_holds` (gun-plans held; `s11a1` JSON block); `paired.py --counters` prints it with
  `gun_direct_rounds` and `gun_kills`.
- Layout: SAVE 31 -> 32 (one `AiCounters` field, one setup key 0..=1); BASELINE, snapshot, `tuning.md`, STATE.md.
- Tests: `direct_budget_holds_ai_gun_direct_fire_over_budget` (leIG, men 100 m off, 67 % spent: 0 direct rounds and a
  40 m hold; controls switch 0: 3 rounds, no hold; budget 100 %: no hold), `direct_budget_keeps_self_defence_fire` (36 m:
  fires), `direct_budget_layout_roundtrips`. In a probe the leIG did not fire at 30 m with the switch off either.
- Neutral: `target/b3c-base40` (played before the edit; cached, binary current) vs `target/b3c-off40`: same 40/40.
  check quick 8f9cde6: all ok (1141 tests). Sets played: 1 (off40; base40 was a cache hit).
- Levers: `code_direct_budget` (switch alone; at 25 % the budget never binds at the S13 head, so expect ~0: control) and
  `code_direct_budget_cap6` (switch + 6 %: `he_shell_cap6` without the direct-fire leak). Both load (`target/b3/check`).
  Risk: a held gun also does not re-lay on a target beyond 40 m, and AT fire waits for armour within 400 m.

## B3 run 2 (2026-11-22; stage agent; F1 3101-3180, F2 3201-3280, `BATCH_JOBS=4`; contract `B3-run-2.md`)
- Sets named by the orchestrator: C_he = `code_direct_budget_cap6` + `own_m17_160_64` (`c_he.toml`), C_all = C_he +
  `own_m36_h1` + `own_class_twins` (`c_all.toml`); `queue.sh check`-style load: 4 and 7 values. No block-S play of them.
- `queue.sh run2` (`target/b3/run2.log`, about 7 min a set, exit 0); lines in `paired.txt`:

| set@block | Ger base -> set | change | up/down | HE | floors | M36 / Jumbo % |
|---|---|---|---|---|---|---|
| `c_he@s3101` | 50.0 -> 50.0 | +0.0 | 23/23 | 27.62 -> 26.63 | ok | 1.2 / 31.2 |
| `c_all@s3101` | 50.0 -> 51.2 | +1.2 | 24/22 | 27.62 -> 26.24 | ok | 36.9 / 12.5 |
| `c_he@s3201` | 48.1 -> 47.5 | -0.6 | 22/23 | 28.17 -> 25.95 | ok | 0.6 / 36.2 |
| `c_all@s3201` | 48.1 -> 46.9 | -1.2 | 22/24 | 28.17 -> 25.86 | ok | 43.8 / 15.6 |
| C_he pooled 320 | 49.1 -> 48.8 | -0.3 | 45/46 | 27.90 -> 26.29 | ok | |
| C_all pooled 320 | 49.1 -> 49.1 | +0.0 | 46/46 | 27.90 -> 26.05 | ok | |

- Own counters (C_all, a match, G / U, F1): base-gun rounds 24.3->19.2 / 18.8->11.9, direct rounds 10.0->8.0 / 6.3->4.4,
  direct holds 0->132 / 214, gun kills 6.14->4.58 / 5.39->3.51; US rifle-grenade casualties 5.84->4.86.
- Decision: C_all R1-R5 hold -> applied (`f4e610b`). `check.sh quick 8f9cde6` all ok (1141 tests) after the test edits above.
- Judge and armour: top block; files `B3-results/judge_f4e610b_*.txt`, `armour_final_f4e610b.txt`.
