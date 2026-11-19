# B2: second balance pass (plan, at most 300 lines)

Written 2026-11-19 by the Plan stage; nothing was played for it (S11b was being built in the checkout). Every number below comes
from files that already exist: `target/s11a2-measure/*.json` (480 matches at `e149258`), `target/s10c-verify/*.json`, the B1 log
(all its versions in git) and the S11a1, S11a2 and S11b logs. Stage contracts: `B2-run-1.md` (screen), `B2-run-2.md` (confirm and
apply). Lever files, the paired reader and the queue: `docs/slices/targets/B2-levers/`. Targets: `targets/B2.toml`. Log `B2-log.md`.
Roadmap: `docs/TODO.md` item 14 (fine balance to 45-55 % and the soft per-unit targets). Owner questions B2-1..8 in section 9.

**The pass runs on the S11b head** (after its stage C, review and 480 measure; called BASE below). It changes TOML only: no sim
code, no SAVE or PROTOCOL change. If it needs code, it stops and says so (section 8).

## 1. Starting point

- Germany pooled 41.67 % of 480 at `e149258` (sets 46.25 / 38.12 / 40.62); 40.00 % at S10c-ai; 42.29 % at B1; 48.75 % before the
  fortification AI (S10a). 44 hard target misses. S11b will move it again: read BASE's measure first (run 1, chunk 0).
- What B1 settled (not repeated here): no `[ai.fort]` setting passes both z 2.5 and the fortification floors; `builders_max` 1
  failed on seeds 1-280 (40.62 against 42.29); `build_threat_max` is inert; delaying or removing forts raises Germany only as
  far as forts disappear (`after600` +8.5 with wire in 1 match of 8); the builder hold is not the cause (no builder on a job
  before minute 3); manpower on works is 1 % of income. **More line squads lower Germany**: `[ai.mix] line` 60 / 70 gave
  -3.1 / -13.1 points on 160 fresh matches (flips 16 / 21 and 19 / 40). B2 moves no `[ai.fort]` or `[ai.obstacles]` key.

## 2. Why Germany sits near 42 %: hypotheses, ranked

Per match at `e149258` (480): Germany kills 65.7 and loses 81.3 men; income 5,185 MP each.

| Kills a match by | Germany | USA | Rounds fired (480), kills per 100 rounds: Germany / USA |
|---|---|---|---|
| rifle | 28.9 | 48.3 | 317 k / 688 k rounds; 4.4 / 3.4 |
| squad and team MG | 5.2 | 1.4 | 602 k / 165 k; 0.41 / 0.42 |
| vehicle MG | 6.7 | 12.8 | 321 k / 274 k; 1.0 / 2.25 |
| tank and gun HE | 11.3 | 5.7 | 75.6 k / 18.6 k; 7.2 / 14.6 |
| mortar | 7.0 | 6.9 | 36.4 k / 28.9 k bombs; 9.2 / 11.4 |
| tank AP, SMG, grenades, rockets, other | 6.7 | 6.2 | |

Checked and found even, so not causes: card cooldowns (line 20 s, MG 30, AT 48, mortar 60 on both sides), unlock times, the AT-gun
and artillery budgets, mortar results, base side (its lean flips sign between sets). **The manpower exchange is not against
Germany**: its infantry costs 33.0 MP a man bought (USA 46.5), so 81.3 men lost are about 2,680 MP against 65.7 x 46.5 = 3,055 MP
for the USA. Germany loses matches while winning the attrition by value: the gap is ground held, not men.

**H1 (first). The USA wins the first clash with the free start squads, and the fortification AI then keeps that lead.**
- Evidence. By match length (480): under 14 min Germany wins 43.6 % (n 55); **14-17 min 24.6 % (28 of 114)**; 17-20 min 49.6 %
  (117); 20-24 min 46.2 % (104); over 24 min 46.7 % (90). Outside the 14-17 minute bin Germany wins 47.0 % of 366. A match
  that ends at 14-17 min is 500 tickets drained at 40 a minute: a two-point lead held from about minute 3 to the end. The USA
  wins 86 of those, Germany 28. The start force is 32 men with 27 Garands and 2 BARs against 28 men with 20 Kar 98k and 2 MG 42;
  by the ranking rule in `ai.rs` `buy` (read, not measured) both AIs' first buys are a mortar team and an MG team, not line squads.
  B1: Germany wins 37 % of matches under 20 min, 48.5 % of the longer ones; `after600` (no forts for 10 min) turned the
  under-20 matches from 48 / 33 to 67 / 31; the gap appeared in one step when the AI began to fortify (48.75 to 41.67 %).
- Experiment (free): the base of the screen block is played with `--side-detail`; `paired.py --detail` prints wins, `hold_s`,
  points and pop at 5 min per faction and length bin. H1 holds if the 14-17 bin shows the USA with the larger `hold_s` at
  equal pop. Plus one diagnostic 160 set, `diag_ger_start3` (a third free Grenadiergruppe; never a candidate): H1 predicts at
  least +10 points and the 14-17 bin near even.

**H2. A Rifle Squad out-fights a Grenadiergruppe by more than its price, slot for slot.**
- Evidence. The table: US rifles kill 48.3 a match against 28.9; the MG 42 that replaces three rifles in a Gruppe kills at a
  tenth of a rifle's rate per round on both sides (602 k German MG rounds give 5.2 kills a match). The mix counts squads, not
  manpower, so each line slot holds one 12-man squad against one 10-man Gruppe. B1's `line` 60 / 70 result (above) is this
  hypothesis measured from the other side. Germany's edge is outside the line slot: tank and gun HE, cheaper mortar, MG and AT
  teams.
- Experiment: `mix_line35` on 160 (H2 predicts Germany up); `ger_line_vg150` and `ger_line_g7_150` say which German line card
  serves the slot best.

**H3. US vehicle machine guns kill twice as much with fewer rounds; the M3A1 figure may be a counting fault.**
- Evidence. Vehicle MG kills 12.8 against 6.7 a match. `kills_by_vehicle_pct.usa_m3a1` is 7.0 % of US kills (950 in 160
  matches) with the card bought in 13.75 % of matches: about 36 kills a half-track bought, against 0.4 for the Sd.Kfz. 251/1
  (23 kills, bought in 40.6 %). The share was 7.3 % at S10c-ai when the card was bought in 34-41 % of matches: it did not
  fall when the buys halved, which a real effect would.
- Experiment: `diag_m3a1_w0` on a 40 set (2.5 min). Share still near 7 %: the kills are booked to the wrong vehicle (CLI
  fault, B2-8; T33, T55 and T56 vehicle rows are then suspect). Share 0 and Germany up: the M3A1 is real and strong.

**H4. Artillery is not the cause; its income share may be a lever.**
- Evidence. Germany 40.00 % before the artillery AI and 41.67 % with it. Kills fell on both sides (Germany -4.8, USA -2.5 a
  match at 480); the USA lost more vehicle buys (-0.71 a match against -0.40). US counter-fire 2.3 missions a match against
  1.2, shells 6.8 % of income against 2.6 %. `income_max_pct` 30 read 57.5 % on one 40 set, which is inside a 40 set's noise.
- Experiment: `arty_share30` on 160 paired. S11b's own ablations (`how_off`, `rg_off`, `light_off`, `how_share_10`) are read
  from the orchestrator's S11b measure, not replayed.

**H5. Germany's price cut turns into men it cannot field.** Germany is held by the pop cap 11.7 plans a match (USA 0), pop at
20 min 120 against 104. But Germany wins 46.7 % of the long matches, where the cap binds most: weak. Experiment: `own_pop170`
(owner class, screened only).

**H6. Builder cards.** The builder floor buys the cheapest builder: Baupioniere for Germany (8 Kar 98k, 165 MP, 2.2 a match),
the full Engineer Squad for the USA (7 Garands, an SMG, a satchel, 280 MP, 2.6 a match), so the USA's builders also fight.
B1 found no fort setting that helps; not screened again. Recorded for the owner (section 9, B2-2).

Expected outcome, said first: H1 and H2 point at unit content (start squads, squad make-up), which the pass may not change.
The tier 1 levers act on the later buys, not on the first clash. B2 may end as B1 did, with a sharper diagnosis and measured
owner-class levers. That is a valid result (section 8).

## 3. Levers

**Tier 1: the pass may apply these** (TOML, an AI habit or a card's buy weight, inside the range given; none is a price, a
weapon, a squad's make-up, a start squad, a cap or an economy value).

| File (`B2-levers/`) | Key | Now | Screened at | Range | Tests |
|---|---|---|---|---|---|
| `mix_line35` | `[ai.mix] line` | 50 | 35 | 35-50 | H2; B1 L5 measured 60 and 70 |
| `mix_vehicle30` | `[ai.mix] vehicle` | 20 | 30 | 20-30 | S11b-10 displacement; T30, T40, T54 |
| `arty_share30` | `[ai.arty] income_max_pct` | 15 | 30 | 15-30 | H4 |
| `ger_line_vg150` | VG-Sturmgruppe `ai_weight` | 50 | 150 | 50-150 | H2 (8 StG 44 a slot) |
| `ger_line_g7_150` | Grenadiergruppe (7) `ai_weight` | 50 | 150 | 50-150 | H1, H2 (an MG 42 per 165 MP) |
| `mix_mg30` (optional, last) | `[ai.mix] mg` | 20 | 30 | 20-30 | splits "fewer line" from "more MG" |
| `mix_line42` (run 2 only) | `[ai.mix] line` | 50 | 42 | | the middle dose if 35 overshoots |

Symmetric keys act differently because the rosters differ; card weights are per faction, as S7-2, S8a-21 and B1's L6 / L7 were.

**Owner class: screened as override files and reported with numbers, never applied by the pass** (B2-1).

| File | Change | Whose decision |
|---|---|---|
| `own_pop170` | `pop_cap` 150 to 170 | owner, S4 Q6 |
| `own_ger_price95` | the nine German infantry cards x0.95 (the S6 tune's knob) | owner, S5a-37; B1 limit "no price of an existing card" |
| `diag_ger_start3` | a third free Grenadiergruppe (diagnostic) | owner (how a faction opens) |
| `diag_m3a1_w0` | the AI never buys the M3A1 (diagnostic, 40 set) | not a change |

**Not moved, not screened**: weapons, ammunition, armour, penetration, lethality, squad make-up, card cooldowns, `max_alive`,
`unlock_s`, `[economy]`, tickets, capture, the map, `[ai.guns]` (owner Q4 of B1), every `[ai.fort]` and `[ai.obstacles]` key
(B1), `[ai.mix] mortar` (T8 and T9 would fall), `[ai.mix] at` (it would work only by wasting US manpower), engineer card weights
(owner 2026-11-11), `[ai.recon]` (B2-5), every switch, SAVE, PROTOCOL.

## 4. Noise, and what each set size can show

Share of 42 %: standard error sqrt(0.42 x 0.58 / n). Paired by (seed, side): the sim is chaotic, any change flips 20-45 % of
the pairs (B1: 21-36 % on 480; the S11a2 40 sets: 33-48 %), so the difference of two paired runs has a standard error of about
sqrt(0.38 / n), barely under an unpaired one. z = (up - down) / sqrt(up + down), as B1.

| Matches | One share: SE, 95 % | Paired difference: SE | Smallest difference at z 2.0 | at z 2.5 |
|---|---|---|---|---|
| 40 | 7.8, +-15 | 9.7 | 19 points | 24 |
| 160 | 3.9, +-7.7 | 4.9 | 10 | 12 |
| 320 | 2.8, +-5.4 | 3.4 | 7 | 8.6 |
| 480 | 2.25, +-4.5 | 2.8 | 5.6 | 7.0 |

Consequences, fixed before any run:
- A 40 set decides nothing about the share (only the H3 counter, a 7-point share against 0, is read from one).
- The gap to the band's edge is 3.3 points; no affordable run can prove a 3-point gain. **The pass aims at 47-53 %** (a gain of
  6-8 points), which 480 paired matches can show at z 2.5 (B2-6). A lever that gives +3 is left for the owner, not applied.
- A 160 screen cannot confirm anything. It only ranks. Its false-positive rate at the pass mark below is about 1 in 7 per
  lever, so with 6-8 levers one false pass is expected: the confirmation is on matches the screen never saw.

## 5. Method and decision rule (fixed in advance)

Seeds. Screen block S 2001-2080; fresh blocks F1 2101-2180 and F2 2201-2280 (each 80 seeds x both bases = 160 matches; B1 used
1001-1280). Judges 1-80, 101-180, 201-280: played only by the orchestrator's measure, never to search. Held out 301-380 (B1's,
never played): once, after acceptance. The base of every block is BASE unchanged, played with `--side-detail`.

**Screen (run 1).** Each lever at its far end on block S against the base. A lever goes on to confirmation if all hold:
1. the sign is the predicted one and the paired gain for Germany is at least **+5 points** (z about 1.0);
2. every floor holds (`paired.py` prints `floors ok`): no match without a winner; works in at least 90 % and wire in at least
   60 % of matches for both sides (T94, T95); mines in at least base - 10 %; HE share at most 26.3 and not under min(20,
   base - 1) (T8); mortar share of kills per side at most 15 and not under min(10, base - 1) (T9); all vehicles' share of
   kills per side at most max(30, base + 2) (T31); median length 15-30 min (owner: median 15-30); no base side above 62 %
   (T6); an infantry gun bought in at least 90 % of matches for both sides;
3. its own counter moved (the buys or the mix it changes; a lever that changes no purchase is inert and dropped).
At most the **two best** tier 1 levers go on, by paired gain. A combination of two is a new candidate and may go on as the
second one only if both parts passed alone. Owner-class levers and diagnostics are logged, never confirmed or applied.

**Confirm (run 2).** The candidate on F1 and F2 against their bases (320 fresh pairs). It is applied only if all hold:
1. **fresh matches alone** (320): gain at least +4 points, z at least 1.64, and the gain is positive in F1 and in F2;
2. **all three blocks** (480): z at least 2.5 (B1's mark);
3. Germany's level with the lever over the 480 is inside 45-55, and every floor of the screen holds on the 480 pooled;
4. if that level is above 53: the middle dose (`mix_line42`, or the midpoint of the lever's range in a new file) is played on
   F1 and F2 and must pass rule 1 itself; the dose nearer 50 is taken.
One candidate set is applied, not a mix of partial winners. No rule is relaxed after seeing a result; a near miss is logged as
a miss with its numbers.

**Judge (orchestrator, after run 2).** `tools/measure.sh` on the judges with `B2.toml`. The change stays only if: Germany
pooled 480 inside 45-55; each 160 set inside 40-60; no match without a winner; paired against BASE's measure on the same
seeds the gain is above zero; no hard target that passes in BASE's measure (read with `B2.toml`) now misses; re-based rows are
listed apart and never counted as closed. Otherwise the apply commit is reverted (`git revert`, as B1's `b477036`) and the
log says so. Then the held-out set once: 40-60 on the set, 45-55 on the pool of 640; if it fails, no retune: report.

## 6. Targets: what is stale, and `targets/B2.toml`

`B2.toml` is generated: `python3 -I docs/slices/targets/B2-levers/rebase_targets.py MEASURE_DIR > docs/slices/targets/B2.toml`.
The committed file was made from `target/s11a2-measure`; run 2 makes it again from BASE's measure (the howitzer displaces
vehicles further, S11b-3). 24 rows change, each with a `# B2:` line giving the old bound and the reason.

Rule R for floors made stale by design: 0.85 x the lowest of the three sets, rounded down to 5 (share of matches) or 0.5
(manpower share). A 160 set has a standard error of 3-4 points on such a share, so the margin is two to three of them. A
re-based floor guards against further displacement; it is not a design goal, and the old figure stays in the comment.

| Rows | Old | Measured (three sets) | New | Reason |
|---|---|---|---|---|
| T54a Pz IV bought | 90 | 89.4 / 86.9 / 88.1 | 70 | gun and observer cards take 400-500 MP a match (S11b-10 (A)) |
| T54b M4 bought | 90 | 76.2 / 73.8 / 76.2 | 60 | same |
| T54c Pz IV manpower | 11 | 10.0 / 10.0 / 10.2 | 8 | same |
| T54d M4 manpower | 10 | 7.0 / 7.0 / 7.4 | 5.5 | same |
| T30a/c, T54e StuG bought | 60, 80 | 60.0 / 48.8 / 48.1 | 40 | same |
| T30b/d, T54f M10 bought | 60, 75 | 32.5 / 30.0 / 33.1 | 25 | same |
| T40a/c, T72a 251/1 bought | 60 | 40.6 / 33.1 / 33.1 | 25 | same |
| T40b/d, T72b M3A1 bought | 60 | 13.8 / 15.6 / 17.5 | 10 | same; see H3 before trusting any M3A1 row |
| T97a-d builders' share (cap) | 12 | 16.5-16.8 and 18.0-18.2 | 21 | S10c-ai's builder floor and front jobs, by design; the 12 came from the old engineer mix weight (B2-4) |
| T31a-d vehicles' share of kills | 30, report | Germany 32.0-33.5, USA 27.8-29.2 | 30, report | settled as a report: AT guns (4-4.5 %) and infantry guns (3.5-4 %) are inside the metric (B2-5) |
| T5c, T5d, T5e (only with `--applied`) | 35-65 guard | | 40-60 hard | the per-set band of an accepted pass; the pool 45-55 is read in the log |

Not re-based, with reasons:
- T8b (HE share 19.8, floor 20) and T9b (US mortar share 7.7-9.0, floor 10): S11b adds rifle grenades and light mortars to
  both; read again on BASE. `mix_line35` should raise both.
- T55a (Sd.Kfz. 234/1 at 14.7-15.3 % of Germany's kills, cap 10) and T55b (M8 9.5-10.9 %): real, not stale. The car is
  Germany's best card by manpower (6.3 kills a car bought). Lowering `[ai.recon]` would cost Germany points. Stays a report
  (B2-5).
- T81 (AT guns in cover): S11b fixes the count (S11b-11). T88a/c (German guns put 0.26-0.31 armour out of action a match,
  floor 0.55): the targets fell 31 % (US tanks and TDs bought 1.62 to 1.12 a match) but the result fell 40 %: partly real,
  stays hard and open. T88b/d stay reports (B1 Q4).
- T95b/d (US wire 87.5-90 %, floor 60): pass; settled as they are. T56: pass.
- T110-T113, T115, T119, T122 (AI use of mines, obstacles, bunker; own-side mistakes): not balance (owner E6, B1-2).
The soft rows named by item 14 are thereby settled: T5c/d (above), T31 (report), T55 (report, owner question), T95 (hard, met).

## 7. Stages and time (4 cores: a 40 set about 2.5 min, a 160 set about 10 min; `BATCH_JOBS=4`)

| Stage | Who | Machine time | Result |
|---|---|---|---|
| B2-1 screen (`B2-run-1.md`) | Tune-tier agent, medium effort (workflows.md 3) | about 85 min: check 3, base 10, diagnostic 10, five tier 1 levers 50, 40-set diagnostic 5, then owner-class and optional levers while time lasts | one line per lever in the log; at most two candidates named, or "none" |
| B2-2 confirm and apply (`B2-run-2.md`) | Tune-tier agent, medium effort (workflows.md 3) | about 60-80 min: two bases 20, candidate 20, second candidate or middle dose 20, apply and `check.sh quick` 10 | the default changed in one commit with before and after, or "nothing passes" |
| Judge | orchestrator | 480 measure about 35 min, held-out set 10 | accept, or revert |
| Review | Review-tier agent, short | no runs | the diff against sections 3 and 5; a cheap stage decided, so a review follows (dev-speed 6) |
| Docs | Docs-tier agent, exact text given | | STATE.md, TODO.md, log at most 150 lines |

No foreground command over 4 minutes: sets run through `queue.sh` in the background (`tools/run.sh`), polled in steps of at most
200 s. The container can restart: `queue.sh` resumes from the batch cache when `target/` survives; every finished lever's line
and per-match CSV land in `docs/slices/targets/B2-results/` and are committed at once, so the pairing survives without it.
Check base: `tools/check.sh quick 8d2731b`. Tick time and the full suite only from the orchestrator's `tools/verify.sh`.

## 8. Stop rules

- **Nothing passes the screen**: run 1 logs every line, names no candidate, and run 2 is not started. No default changes.
  The log's top block then states: the H1 reading from `--detail`, the diagnostic's result, each owner-class lever's line, and
  the one recommendation for the owner (section 9, B2-2).
- **Nothing passes the confirmation**: run 2 changes nothing and says so, with the numbers. No second search on F1 or F2:
  those blocks are spent once a candidate was read on them.
- **The judge fails**: revert, log, stop.
- BASE's own measure outside 35-65 %, or a match without a winner in a base block: stop and report before screening.
- A lever needs code (a key that does not exist): not in this pass. Named for the owner: an opening-buy rule so the first
  buys are line cards (Germany gets two Gruppen for 464 MP where the USA gets one squad for 360), which no mix weight can
  express (the mix ranks a mortar team and an MG team first for both sides).

## 9. Owner questions (copied to `docs/TODO.md`; each runs on (A): "assumed 2026-11-19, owner asked not to be asked")

- B2-1 What the pass may change alone. (A) Only AI rule keys and card `ai_weight`s inside the ranges of section 3; prices, pop
  cap, start squads, squad make-up and weapons are screened as override files and reported with numbers, not applied. (B)
  Also apply one owner-class lever if it passes the same rule.
- B2-2 If no AI setting passes (the evidence points at the start squads and the line squads). (A) Change nothing, as B1; the
  owner then picks from the measured list: a third German start squad or a cheaper one, the German price cut, the pop cap, an
  AI opening-buy rule (code), or accept about 42 %. (B) The pass applies the best owner-class lever itself.
- B2-3 Re-based floors T30, T40, T54, T72 by rule R (S11b-10 (A) asked for them). (A) As in `B2.toml`. (B) Keep the design
  floors as visible hard misses.
- B2-4 T97 builders' share cap 12 to 21. (A) Re-base: S10c-ai raised it by design. (B) Keep 12 and lower `builder_cards_min`.
- B2-5 T31 and T55. (A) Both stay reports; a vehicles-only kill share is a later CLI change; the Sd.Kfz. 234/1 (15 % of
  Germany's kills) is not touched while Germany is under 45 %. (B) Lower `[ai.recon] income_max_pct` now.
- B2-6 The aim. (A) 47-53 %: a gain the runs can prove; a +3 lever is reported, not applied. (B) Apply any gain that reaches 45.
- B2-7 Seeds. (A) Screen 2001-2080, confirm 2101-2280, judges 1-280, held out 301-380 once. (B) Reuse B1's blocks 1001-1280.
- B2-8 The M3A1's kill count (H3). (A) One 40-set diagnostic; if it is a counting fault, fix the CLI count in a later change
  and treat the vehicle kill rows as suspect until then. (B) Fix it inside B2 (CLI code, a review gate).

## 10. Risks

- Fitting noise: guarded by the fresh blocks, the judges and the held-out set; the screen's mark is weak by design.
- The base moves under the pass: S11b's review or fix changes a default after run 1. Then run 1's lines are void; `queue.sh`
  replays them (the cache key holds the binary and `mods/`). Run 2 compares `tools/batch.sh world` with run 1's key first.
- `mix_line35` may pass by thinning the infantry: fewer line squads on both sides. The floors watch HE, mortar and vehicle
  shares and the match length; the review reads the line squads bought a match before and after.
- A symmetric key that helps Germany by making the US AI buy worse is a fit, not a fix: the log gives both sides' buys.
- `paired.py --detail` and `queue.sh check` were written without a CLI run (no `--side-detail` file exists yet): run 1 fixes
  a wrong key name in place and says so.
- Later slices (S12-S15) will move the share again; the pass does not promise more than the judge's number.
