# B3: third balance pass (plan, at most 300 lines)

Written 2026-11-22 by the Plan stage; nothing was played or built for it. Every number comes from files that exist: the 480
measures `target/s13-on/` (S13 head `3b3387f`, world `ba09de8def971642465e0c5f`), `target/s12-on2/`, the ablations
`target/s12-abl2/*` and `target/s13-abl/s13_off`, `targets/B2-results/`, the B2, S12 and S13 logs, and a reading of `ai.rs` `buy`
and `heavy_plan`, `ai/arty.rs`, `supply.rs`. Stage contract: `B3-run-1.md` (screen). Lever files, the paired reader and the queue:
`docs/slices/targets/B3-levers/`. Targets: `targets/S13.toml`, unchanged (section 6). Log: `B3-log.md` (run 1 starts it).

**The pass runs on the S13 head** (BASE; `claude/project-thread-aj1tww`, `e3db757` or later with the same `mods/` and CLI). It
changes TOML only: no sim code, no SAVE or PROTOCOL change, no plate, no penetration row. Where a goal needs code it says so and
stops (sections 2.2, 8). Goals in order: (1) S12-16 HE share of casualties under the cap 26.3 with Germany inside 35-65 % and
moving toward 45-55; (2) S13-F2 the AI buys the heavies (T179a-c); (3) S13-F1 / S13-1b old rounds; (4) S12-17 stabilisation.

## 1. Starting point

- Germany pooled **42.08 %** of 480 (43.12 / 40.00 / 43.12); 42.71 with `s13_off` (= S12 on main); 44.17 before wounds. No
  match without a winner. 51 hard target misses, among them T8a-c, T179a, T179b (T179c reads 7.5 on the first set).
- HE share of casualties (T8, one per man, by the class that first took him out): **28.0 / 28.3 / 28.2**, cap 26.3; the 40 set
  reads 28.8. With every S12 switch off the same seeds read 26.8 / 26.5 / 26.2: the cap was already touched before wounds.
- Settled before and not repeated: no `[ai.fort]` key helps (B1); more line squads lower Germany (B1, B2); the opening is not
  lost by numbers (B2-11). B2's measured owner-class levers stay with the owner (B2-9..12, section 9).

## 2. Diagnosis

### 2.1 Goal 1: what drives the HE share (480 matches each; casualties caused a match, Germany / USA)

| Class | wounds off (`s12_off`) | S13 head | of which outright + downs | share of all casualties |
|---|---|---|---|---|
| tank and gun HE (`tank_he`) | 12.64 / 6.58 | **15.26 / 8.26** | 8.17 + 7.09 / 4.20 + 4.06 | 15.8 % |
| mortar | 5.49 / 5.54 | 5.65 / 5.21 | 0.70 + 4.95 / 0.88 + 4.33 | 7.3 % |
| rifle grenade | 2.56 / 5.65 | 2.66 / 5.90 | 1.63 + 1.03 / 3.56 + 2.34 | 5.8 % |
| rifle (for scale) | 26.13 / 41.87 | 26.89 / 43.26 | 2.01 + 24.89 / 0.93 + 42.33 | 47.2 % |
| all classes | 63.0 / 78.8 | 66.2 / 82.4 | 21.6 % outright, 78.4 % downs | |

- **Blast kills or fragment downs: neither is the cause of the miss.** HE makes 59.7 % of the outright kills and 20.4 % of the
  downs, and that is why the kills share reads 38 %; but T8 counts each man once whichever way he left the fight. The rise from
  26.5 to 28.2 is **+4.3 casualties a match in the tank-and-gun-HE class alone** (Germany +2.6, USA +1.7); mortars -0.2, rifle
  grenades +0.35, all other classes together +2.3.
- **Which guns.** The JSON has kills by gun (finishing blows on downed men included), not first hits by gun. By that count the
  whole rise is the **four base guns**: leIG 18 2.49 -> 3.73, leFH 18 2.12 -> 3.10, 105 mm M3 2.42 -> 3.85, M2A1 1.63 -> 2.00
  (sum 8.66 -> 12.68, about half of the class's 24.5 kills a match), while every other vehicle and gun fell (Sd.Kfz. 234/1 9.95
  -> 9.36, M8 8.74 -> 7.10, Panzer IV 3.89 -> 2.79, Sherman 2.48 -> 1.56). Gun rounds rose only 25.1 -> 27.7 and 18.0 -> 20.1 a
  match: the shells find more men, they are not many more shells.
- **Why (from the S12 ablations, pooled HE share / tank-HE casualties Germany, USA).** Off 26.5 / 12.6, 6.6. `head_300` (wounds
  on, medics and reinforce off, head x3) 26.6 / 12.5, 6.6. `no_medics` 27.3 / 14.4, 6.8. `no_post` 28.0 / 14.8, 7.7.
  `no_reinforce` 29.1 / 15.6, 7.4. On 28.1 / 15.3, 8.3. Medics in the field add about 0.9 points: the US guns' extra casualties
  need German medics on the map (6.8 without, 8.3 with). The likely mechanism, read in `ai/arty.rs` and not measured: tending,
  bandaging and slowed wounded men are still men, and the gun scorer prefers still men. The German guns' extra 1.8 stays
  without medics and goes only in `head_300`, which changes three things at once; `diag_head300` separates them (optional).
- **Not sources of the miss:** mortars (first hits flat, and T9b, the US mortar share of kills, sits at 11.3-11.7 against its
  floor 10: no mortar key is moved); German rifle grenades (2.7 a match, 0.43 a grenade fired).
- **A source that was over the line before wounds: the US rifle grenade.** 5.9 casualties a match, 7.2 % of everything the USA
  causes, **0.73 men a grenade fired**; the M17 carries the Mk 2 hand grenade's whole burst (blast 200 cm, 96 fragments; the
  105 mm shell has 175 cm), on figures marked "owner to confirm" (S11b-7).
- **Counted as HE, not moved:** the Sd.Kfz. 234/1's 2 cm Sprgr. German tank-HE rounds are 210 a match (160 with wounds off),
  of which the base guns fire 28; most of the rest is the 2 cm. The car makes 15 % of Germany's kills (T55a, a report) and is
  not touched while Germany is under 45 % (B2-5 (A)).
- **What it takes.** 28.17 -> 26.3 is -1.9 points; the screen aims at **25.5 or less** (-2.7), because T8c reads a 40 set and
  one 160 set differs from the next by 0.3. That is 4.1 HE casualties a match moved to another class, or 5.6 removed, of 43.0.
  Levers that take HE from the US side (rifle grenades, the shell budget that binds the USA harder) also help Germany's share;
  levers that take it from the German side (2 cm, tank HE) would lower Germany and are not used.

### 2.2 Goal 2: why the AI rarely buys heavies (480 matches; matches with the card)

Panther 100 (20.8 %), Tiger 33 (6.9 %), Jumbo 163 (34.0 %), M36 0, 76 W 1. Floors T179a-d: 25, 25 (M36), 10, 10.
- **Few vehicle buys at all.** Vehicle-slot buys a match: Germany 0.96 (Panzer IV 0.70, StuG 0.19, Sd.Kfz. 251 0.07), USA 0.53
  (Sherman 0.45, M10 0.06, M3A1 0.02). `[ai.mix] vehicle` is 20 of 175; AT guns, base guns, the howitzer, recon cars and
  builders take their own shares first.
- **M36 and 76 W (code reading, `ai.rs` `buy`).** The slot picks the card with the fewest buys per `ai_weight`, ties by roster
  order; S13 G2 gives a tie to a wave card only if the AI can pay it at that moment. At zero buys the Sherman wins the tie, the
  slot saves for 590 and buys at once, so 690 or 760 is almost never in hand; then the M10, the M3A1 and the 76 W stand
  before the M36. **No key on S13 plan 7's list (cost, unlock, cooldown, `max_alive`, H1 timing) reaches the M36**, short of
  pricing it at the Sherman's 590. In TOML it can be reached two ways, both outside that list: `ai_heavy = true` on its card (H1
  buys it when enemy armour was seen, before the Jumbo by roster order), or `ai_weight` above 100 (the slot picks and saves for
  it first, the Sherman second). Either is a choice of how the USA plays (B3-3).
- **Panther (H1, `heavy_plan`).** Germany starts 417 plans on 480 matches; 141 end in a buy, 99 are dropped at
  `heavy_save_max_s` 240, the rest end when the card stops being ready or the match ends. 240 s of income is 960-1200 MP
  against a price of 1050, and urgent buys (builder, breach, AT team) spend during the saving.
- **Tiger.** H1 picks the ready heavy with the fewest buys, then roster order: the Panther is always first, the Tiger only ever
  the second heavy and only after 12:00 (33 Tigers; 8 second buys were Panthers). TOML can raise it only by more heavy buys in
  all (cheaper, earlier, longer saving). Putting the Tiger first is a rule change: **code, not in this pass** (B3-3).

### 2.3 Goal 3: armour-test case 45 and S13-1b

Case 45 (Jumbo leading into a PaK 40 at 400 m, survives 3 hits in at least 6 of 10): 0 of 10. Every hit is one of the PaK's two
old PzGr 40 (APCR, no `slope_class`), which the cosine law lets through the 101.6 mm @ 47 glacis; the Panther's own PzGr 40/42 is
classed and would not. Two old rounds are twins of rounds S13 classed: the 7.5 cm PzGr 40 (PaK 40, Panzer IV, StuG; 2-3 rounds
each) and the 3-inch M62 (M10; the 76 mm M62's projectile, which now beats the Panther nose at 350 m where its twin does not).
Classing is one TOML line a round and changes no plate and no penetration row; classing the L/48 PzGr 39 too would drop it
against the Sherman glacis at 30 degrees from 100 % to 0 % at 350 m (S13 plan 3.3). The choice is the owner's: B3-4.

### 2.4 Goal 4: stabilisation (report level)

Own downs a match 66.2 Germany, 50.3 USA: stabilised 3.4 / 3.9 %, bled out 49 / 46 %, **killed while down 41 / 44 %**; medics
start a tend on about 6 % of downs (4.0 tends a match from 2.1 teams). Two walls: the 60-180 s bleed-out window, and that downed
men lie where the fire is (the medic rule waits for no spotted enemy within 40 m). More medics would also feed the HE share
(2.1). One optional lever, screened and reported only: the longer window (`own_s12_bleed120_300`, B3-5).

## 3. Levers (`B3-levers/NAME.toml`; predictions fixed before any run)

**Tier 1: the pass may apply these** (an AI key, or cost and unlock of an S13 card: B3-1).

| File | Key | Now -> screened (range) | Goal; predicted; own counter |
|---|---|---|---|
| `he_shell_cap6` | `[ai.arty] shell_mp_max_pct` | 25 -> 6 (4-25) | 1; HE -0.8 to -1.5, Germany 0 to +2 (shells are 7.5 % of Germany's income, 9.4 % of the USA's; the cap never binds now); base-gun rounds, budget holds |
| `he_mission3` (optional) | `[ai.arty] mission_rounds` | 4 -> 3 | 1; HE -0.5; base-gun rounds |
| `he_shell_cap4` (dose rule only) | as the first | 25 -> 4 | 1 |
| `hv_save360` | `[ai.armour] heavy_save_max_s` | 240 -> 360 (240-360) | 2; drops fall, Panther and Jumbo up 5 points, Tiger up; Germany unknown (longer saving); `heavy_drops`, card matches |
| `hv_ger_reach` | Panther `cost`; Tiger `cost`, `initial_cooldown_s` | 1050 -> 950; 1200 -> 1100, 720 -> 600 | 2; Panther to 25-30 %, Tiger to about 10 %; Germany 0 to +3; card matches |
| `mix_vehicle30` (`mix_vehicle25` by the dose rule) | `[ai.mix] vehicle` | 20 -> 30 (20-30) | 2 and the win share; B2 read +8.8 and HE -0.5 on other seeds; vehicle cards a match |

**Owner class: screened as override files, in a candidate set only when `docs/TODO.md` shows the question answered or assumed
(A) when run 1 ends; otherwise reported with numbers.**

| File | Change | Question; predicted |
|---|---|---|
| `own_rg_gap25` | `[items] rifle_grenade_gap_s` 15 -> 25 | **B2-9**; HE -0.8 (B2: -0.8 on 480, -1.0 on fresh 320, old metric), Germany 0 |
| `own_m17_160_64` | M17 burst: blast 200 -> 160 cm, fragments 96 -> 64 | B3-2; US grenade casualties 5.9 -> about 4.3, HE -0.7 to -1.0, Germany 0 to +2 |
| `own_m36_h1` | M36 card `ai_heavy = true` | B3-3; M36 in 30-40 % of matches, Jumbo down toward its second-buy rate (floor 10) |
| `own_m36_w150` (optional) | M36 `ai_weight` 100 -> 150 | B3-3 (B); M36 30-45 %, Sherman from 45 % toward 15-20 % |
| `own_class_twins` | `slope_class` on the 7.5 cm PzGr 40 and the 3-inch M62 | B3-4; armour-test only: case 45 passes, the other checks that move are listed |
| `own_s12_bleed120_300` (optional) | bleed-out 60-180 -> 120-300 s | B3-5; stabilised share about doubles; never applied by the pass |
| `diag_head300` (optional) | `[wounds] head_damage_pct` 300 | diagnostic (2.1); never a candidate (S12-3 (A)) |

**Sums.** Tier 1 alone (`he_shell_cap6`, then `he_mission3` or the 4 % dose) is predicted at -1.3 to -2.5: it may reach the cap
and may not reach 25.5. With B2-9 and B3-2 on (A) the three HE levers sum to -2.3 to -3.3. **Goal 1 most likely needs at
least one of the two owner answers.**

**Not moved, not screened:** plates, penetration rows, weapons other than the M17 burst, shell blast and fragment values, mortar
keys (T9b), the 2 cm Sprgr and `[ai.recon]` (B2-5), prices and weights of cards older than S13, start squads (B2-11), the 57 mm
tow (B2-10), `[ai.fort]`, `[ai.obstacles]`, `[ai.guns]`, `[ai.medic]`, `[economy]`, pop cap, every switch, SAVE, PROTOCOL.

## 4. Noise

- Win share: as `B2-balance.md` 4. A 160 screen ranks and cannot confirm (paired SE 4.9 points; +10 is z 2.0); 320 fresh
  pairs show about +5.7 at z 1.64. The gap to 45 % is 2.9 points: **no affordable run can prove it closed.** B3 therefore sets
  a floor on Germany's share (not down more than 2.0) for the HE and heavy levers and asks a gain only of `mix_vehicle30`.
- HE share: three 160 sets of one binary read within 0.3 (28.0 / 28.3 / 28.2; 26.8 / 26.5 / 26.2), the 40 set 0.6 off. A change
  of 0.5 on a 160 set is real; the cap needs a margin of 0.5 on the fresh blocks and 0.8 on the screen block.
- Card shares: a card bought in 25 % of 160 matches has SE 3.4 points; T179a at 20.8 % of 480 with a floor of 25 can pass or
  miss on one set by chance (23.75 on the first set). A heavy lever must show +5 points on the screen and stay up on both fresh blocks.

## 5. Method and decision rule (fixed in advance)

Seeds: screen block S 3001-3080; fresh F1 3101-3180, F2 3201-3280 (each 80 seeds x both bases = 160 matches); judges 1-80,
101-180, 201-280 (the orchestrator's measure only; BASE's judge reading is `target/s13-on`); held out 5101-5180, once. Used
before: 1-380, 1001-1280, 2001-2280, 5001-5080. The base of every block is BASE unchanged.

**Screen (run 1).** Each lever alone on block S against the base. `paired.py` prints the line, the HE share base -> lever and
the floors: no match without a winner; HE at most max(26.3, base + 0.3) and at least 20; works in at least 90 % and wire in at
least 60 % of matches per side; mines at least base - 10; mortar share of kills per side at most 15 and not under min(10,
base - 1) (T9); median length 15-30 min; no base side above 62 %; an infantry gun bought in at least 90 % of matches per side;
Jumbo bought in at least 10 % (T179d). All vehicles' share of kills (T31) is logged, not a floor: it is a report row (B2-5 (A)).
A lever passes when the floors hold, its own counter moved, and:
- **rule H** (`he_*`, `own_rg_gap25`, `own_m17_160_64`): HE with the lever at least 0.5 under the base, Germany's paired change
  not below -2.0;
- **rule V** (`hv_*`, `own_m36_*`): the card it aims at up by at least 5 points of matches, Germany's change not below -2.0;
- **rule W** (`mix_vehicle30`): Germany's paired gain at least +5.0, as B2.
**Candidate sets, built by rule, at most two, both played on block S as combinations (`a+b+c`):**
- **C_he**: passing HE levers added in the order of Germany's paired change (best first; tier 1 before owner class at equal
  change; an owner-class lever only with its question on (A)) until the summed HE drops reach 2.7. If the passing levers sum to
  less: `he_shell_cap4` in place of `he_shell_cap6` once, then C_he is what there is.
- **C_all**: C_he plus every passing rule V lever (of `own_m36_h1` and `own_m36_w150` only the first) plus `mix_vehicle30` if it
  passes rule W (above 55 % for Germany on block S: `mix_vehicle25` instead).
A combination goes on to run 2 only if on block S: HE at or under **25.8**, Germany's change not below -2.0, floors hold, and
for C_all each heavy card it aims at still up by 5 points. C_all failing while C_he passes: C_he alone goes on.

**Confirm (run 2; its contract is written after run 1, from this section).** Bases and candidates on F1 and F2 (four or six
sets). On the fresh 320 pairs: R1 HE at or under 26.3 pooled and under the base in each block; R2 Germany's pooled change not
below -2.0 and its level inside 38-62; R3 every floor above; R4 no match without a winner; R5 (C_all only) each heavy card it
aims at above its base value in both blocks. **Apply C_all if R1-R5 hold; else C_he if R1-R4 hold; else nothing.** One set is
applied in one commit with the comments, the two BASELINE refreshes (`resolved_defs.rs`, `simbench/report.rs`) and
`check.sh quick`; a test that pins a changed value is reported, not edited, unless the orchestrator allows it (as B2-2). F1 and
F2 are spent once read. No rule is relaxed after a result; a near miss is logged as a miss with its numbers.

**Judge (orchestrator).** `tools/measure.sh` on the judges with `targets/S13.toml`, against `target/s13-on`: J1 T8a and T8b
pass and the pooled 480 is at or under 26.3 (T8c, the 40 set, is logged); J2 Germany pooled inside 35-65, paired change not
below -2.0, each 160 set inside 38-62; J3 no match without a winner; J4 no hard target that passes in `target/s13-on` now
misses. **Rows that cross their bound between the three sets or two measures of one default are noisy and listed apart, not
counted in J4**: T121b (USA mines set off: 0.25 / 0.44 / 0.34 at the S13 head, 0.45 / 0.37 / 0.32 with wounds off; this is the
row that stopped B2-9), T5c, T50e, T50f. J5 the T179 rows the set aimed at are reported pass or miss; a miss does not revert an
HE fix. Otherwise `git revert` of the apply commit. Then 5101-5180 once: Germany 38-62 and HE at or under 26.8; a failure is
reported, no retune.

**Goal 3** is not screened on matches: `queue.sh armour own_class_twins` (armour-test, 5 seeds, every case, base and lever)
lists case 45 and every check that moves. It joins the apply commit only with B3-4 on (A), with T184d turned hard in
`S13.toml`; its effect on the win share is read by the judge (the rounds are 2-3 a vehicle; the M10 is in 6 % of matches).

## 6. Targets

`targets/S13.toml` stays the pass's target file; no `B3.toml` is generated (B2's re-based floors live only in `B2.toml` and are
not carried: B2-3 is the owner's). Rows the pass may close: T8a-c, T179a-c. Rows it may change, only with the apply commit and
the owner answer named: T184d `kind = "report"` -> hard (B3-4 (A)); T179b -> `kind = "report"` (B3-3 (C)). Rows it must not
lose: T179d, T9a/b, T174 (35-65), T176, and J4's list. The vehicle floors T30, T40, T54, T72 (all missing now: Panzer IV in
69 % of matches against 90, Sherman 45 %, StuG 19 %, M10 6 %) are reported before and after for `mix_vehicle30`; they stay hard
misses, not goals of this pass.

## 7. Stages and time (4 cores; a 160 set about 12 min at the S13 head; `BATCH_JOBS=4`)

| Stage | Who | Machine time | Result |
|---|---|---|---|
| B3-1 screen (`B3-run-1.md`) | Tune-tier agent, medium effort | **eight 160 sets and one or two combinations, about 2 h**, plus the lever check and armour-test (10 min) | one line per lever, C_he and C_all named or "none" |
| B3-2 confirm and apply (contract after run 1) | Tune-tier agent, medium effort | four or six 160 sets, 50-75 min, `check.sh quick` | one apply commit, or "nothing passes" |
| Judge | orchestrator | 480 measure about 40 min, held-out set 12 | accept or revert |
| Review | Review-tier agent, short | none | the diff against sections 3 and 5 |
| Docs | Docs-tier agent | none | STATE.md, TODO.md (B3 questions, S12-16, S12-17, S13-F1, S13-F2), log at most 150 lines |

Order of the eight: base, `he_shell_cap6`, `own_m17_160_64`, `own_rg_gap25`, `hv_save360`, `own_m36_h1`, `hv_ger_reach`,
`mix_vehicle30`. Optional, only if time is left, in this order (cut from the end): `he_mission3`, `own_m36_w150`,
`diag_head300`, `own_s12_bleed120_300`. No foreground command over 4 minutes; the queue resumes from the batch cache; each
finished lever's line, CSVs and counters land in `targets/B3-results/` and are committed at once.

## 8. Stop rules

- BASE's block S outside 35-65 % for Germany or with a match without a winner: stop before screening.
- No HE lever passes rule H, or C_he does not read 25.8 on block S: no HE candidate. The log then gives each lever's line and
  the short sum, and names what is left: the owner answers (B2-9, B3-2), B2-12 (B), or code.
- Nothing passes the confirmation, or the judge fails: nothing is applied or the commit is reverted; no second search.
- A lever needs code: not in this pass. Named now: a Tiger-first pick in H1; a vehicle-slot rule that reaches the M36 without
  `ai_heavy` or a weight; a rule that stops fire at downed men (41-44 % of downs are killed while down) or keeps the gun scorer
  off men being tended; first hits by gun in the JSON (2.1 can only count kills by gun).

## 9. Owner questions

**B2-9..12 stay the owner's, unchanged** (full text `docs/TODO.md`):
- B2-9 rifle-grenade gap 15 -> 25 s: (A) apply, (B) leave 15. `own_rg_gap25` depends on it. New evidence for the owner: the
  one miss that reverted it, T121b, reads 0.25 / 0.44 / 0.34 across the three sets of the unchanged S13 head (noise).
- B2-10 57 mm tow: (A) keep the M3A1, take its gun off or price it later, (B) leave. No B3 lever depends on it.
- B2-11 start content: (A) leave, look at the mid game, (B) change the German start now. No B3 lever depends on it; the
  vehicle and heavy levers are the mid-game look (A) asks for.
- B2-12 HE cap 26.3: (A) keep the cap and retune HE sources first, (B) move the cap. **All of goal 1 runs on (A)**; on (B) the
  `he_*`, `own_rg_gap25` and `own_m17_160_64` levers are dropped and run 1 screens goals 2 to 4 only.

**New (to be copied to `docs/TODO.md` by the orchestrator; each with the recommended (A)):**
- **B3-1 What the pass may apply alone.** (A) AI keys (`[ai.arty] shell_mp_max_pct`, `mission_rounds`; `[ai.armour]
  heavy_save_max_s`; `[ai.mix] vehicle`) and cost and unlock of S13's own cards, the levers S13 plan 7 named; everything else
  in section 3 only with its question answered. (B) Also the owner-class levers that pass their rule. (C) AI keys only.
- **B3-2 The M17 rifle grenade's burst** (S11b-7 figures are "owner to confirm"). (A) Reduce it to blast 160 cm and 64
  fragments if it passes rule H: a US grenade takes 0.73 men a shot against 0.43 for the German one, and 7.2 % of all the USA's
  casualties caused. (B) Keep the Mk 2 hand grenade's burst; goal 1 then rests on the shell budget and B2-9. (C) Make it equal
  to the German grenade (160 cm, 48 fragments).
- **B3-3 How the AI gets the M36 and the Tiger.** (A) M36: `ai_heavy = true` (bought when enemy armour was seen, before the
  Jumbo; a tank destroyer's own reason to appear), kept only if the Jumbo stays at or above 10 %. Tiger: no code now; it stays
  the second heavy, T179c stays hard and visible, raised only through `hv_save360` and `hv_ger_reach`. (B) M36 by `ai_weight`
  150: the slot's first buy, the Sherman second. (C) Leave both; T179b becomes a report until an AI rule exists (a later
  engine slice, with the Tiger-first pick).
- **B3-4 Old rounds and case 45 (S13-1b, S13-F1).** (A) Class the two twins only: 7.5 cm PzGr 40 `apcr`, 3-inch M62 `apcbc`;
  T184d hard again; every other old round stays on the cosine law, so medium against medium is bit for bit as now. (B) Class
  every old full-calibre AP and APCR round: consistent, but the L/48 PzGr 39 then fails against a Sherman glacis at 30
  degrees, a large change against Germany at 42 %. (C) Leave all; T184d stays a report and the Jumbo keeps falling to two rare
  rounds. S13-9 (the 3-inch M62 row) is a penetration row and is not part of this pass.
- **B3-5 Stabilisation (S12-17).** (A) Report only: B3 changes nothing for it; the longer bleed-out window is screened if time
  allows and its numbers go to the owner. (B) Apply the longer window if the stabilised share doubles and the floors hold.
  (C) A third medic team (`[ai.medic] max_teams` 3): not recommended, medics in the field already cost 0.9 HE points.
- **B3-6 The aim.** (A) In order: HE under the cap with Germany not down more than 2 points; the heavies bought; Germany
  toward 45-55 only through levers that pass their rule. A set that fixes the cap and leaves Germany at 42-45 % is a valid
  result. (B) Apply nothing unless Germany also reaches 45 %.
- **B3-7 Method.** (A) T31 logged, not a screen floor (it is a report row, B2-5 (A)); noisy hard rows listed apart in the
  judge (section 5); seeds as in section 5. (B) Keep B2's floors and judge rule unchanged.

## 10. Risks

- Goal 1 leans on owner answers (section 3 "Sums"); with tier 1 alone the screen may end "short of 25.5" and say so.
- Fewer gun rounds can move artillery rows (howitzer and gun fired in matches, counter-fire in matches, T88): J4 guards them;
  run 1 logs `how_fired_matches_pct` and `cb_fired_matches_pct` beside `he_shell_cap6`.
- `hv_save360` holds all other buys for up to 6 minutes; rule V's -2.0 floor on Germany's share is the guard, and the USA's
  Jumbo plans lengthen too.
- `own_m36_h1` may push the Jumbo under 10 % (the floor is in `paired.py`); `hv_ger_reach` prices a Panther at 1.53 Panzer IV.
- Levers interact (shell budget and vehicle mix both shift manpower): that is why the combination itself is screened on
  block S and confirmed on fresh blocks, not the sum of single results.
- No lever file was loaded by the CLI (TOML-parsed only); `paired.py` was run on existing JSON for the paired line and
  `--counters`, its `--armour` mode on none. Run 1 fixes a wrong key or path in place and says so.
- Fitting noise: guarded by the fresh blocks, the judges and the held-out set. Later slices (S14, S15) will move both numbers.
