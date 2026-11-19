# B2: second balance pass (plan, at most 300 lines)

Written 2026-11-19 by the Plan stage; nothing was played for it (S11b was being built in the checkout). Every number below comes
from files that already exist: `target/s11a2-measure/*.json` (480 matches at `e149258`), `target/s10c-verify/*.json`, the B1 log
(all its versions in git) and the S11a1, S11a2 and S11b logs. Stage contracts: `B2-run-1.md` (screen), `B2-run-2.md` (confirm and
apply). Lever files, the paired reader and the queue: `docs/slices/targets/B2-levers/`. Targets: `targets/B2.toml`. Log `B2-log.md`.
Roadmap: `docs/TODO.md` item 14 (fine balance to 45-55 % and the soft per-unit targets). Owner questions B2-1..8 in section 9. Revised the same day after an independent review (3 major, 7 minor findings; log).

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

**H1 (first, but weakly supported). The USA wins the first clash with the free start squads, and the forts then keep that lead.**
- Evidence. US wins are shorter: median 17.8 min against 19.6 for German wins. Matches that end at 14-17 min go to Germany in
  28 of 114, all others in 172 of 366 (47.0 %). **That bin was picked after looking at the data, and "the faster winner's
  matches are short" follows from the USA winning more: it is a selection effect, not proof of an opening.** What does not
  depend on the bin: the start force is 32 men with 27 Garands and 2 BARs against 28 men with 20 Kar 98k and 2 MG 42; by the
  ranking rule in `ai.rs` `buy` (read, not measured) both AIs' first buys are a mortar team and an MG team, not line squads;
  B1's `after600` (no forts for 10 min) turned the under-20-minute matches from 48 / 33 to 67 / 31; the gap appeared in one
  step when the AI began to fortify (48.75 to 41.67 %).
- Experiment. The real test is one diagnostic 160 set, `diag_ger_start3` (a third free Grenadiergruppe; never a candidate):
  H1 predicts at least +10 points. Beside it, free: the base is played with `--side-detail` and `paired.py --detail` prints
  per faction men alive at 5 and 10 min (`pop_at_min`) and foot squads at 1, 3 and 5 min (`squads_at_min`), with kills and
  losses. `hold_s` and `points_held` are NOT used: both are larger for whoever won, so they cannot test H1. No counter gives
  points held or kills by minute; adding one is code and out of this pass.

**H2. A Rifle Squad out-fights a Grenadiergruppe by more than its price, slot for slot.**
- Evidence. The table: US rifles kill 48.3 a match against 28.9; the MG 42 that replaces three rifles in a Gruppe kills at a
  tenth of a rifle's rate per round on both sides (602 k German MG rounds give 5.2 kills a match). The mix counts squads, not
  manpower, so each line slot holds one 12-man squad against one 10-man Gruppe. B1's `line` 60 / 70 result (above) is this
  hypothesis measured from the other side. Germany's edge is outside the line slot: tank and gun HE, cheaper mortar, MG and AT
  teams.
- Experiment: `mix_line35` on 160 (H2 predicts Germany up); `ger_line_vg150` and `ger_line_g7_150` say which German line card
  serves the slot best.

**H3. The 57 mm gun's armed tow.** Every US 57 mm card arrives behind an M3A1 with its .50 (`usa_57mm_m1_squad.toml` `tow`, owner
choice S9-3 (A)); the Pak 40's tow, the Sd.Kfz. 11, is unarmed.
- Evidence. About 290 guns bought per 160 matches bring about 314 M3A1 hulls with the 22 bought as cards; 950 kills are about
  3 a hull. The M3A1 makes 5.7 of the USA's 12.8 vehicle-MG kills a match, most of the 6.1 gap to Germany's 6.7. Its share
  of kills (7.0 %) did not fall when the card's own buys halved, because the hulls come with the guns. It is a real
  asymmetry, not a counting fault; T33, T55 and T56 are sound.
- Experiment: `own_57mm_truck_tow` on 160 (owner class, screened only): the gun card's `tow` becomes the unarmed CCKW (S9-3
  option B). The override is one line, `[defs.units."ww2:usa_57mm_m1_squad"] tow = "ww2:usa_cckw_squad"`; the truck's `[tow]
  max_kg` 3000 carries the gun's 1,230 kg. Not checked by a CLI run: `queue.sh check` must show the change, and run 1 logs
  what the AI does with a tow whose role is "truck".

**H4. Artillery is not the cause; its income share may be a lever.**
- Evidence. Germany 40.00 % before the artillery AI and 41.67 % with it. Kills fell on both sides (Germany -4.8, USA -2.5 a
  match at 480); the USA lost more vehicle buys (-0.71 a match against -0.40). US counter-fire 2.3 missions a match against
  1.2, shells 6.8 % of income against 2.6 %. `income_max_pct` 30 read 57.5 % on one 40 set, which is inside a 40 set's noise.
- Experiment: none in the fixed queue. `arty_share30` is close to inert: `income_max_pct` governs only the infantry-gun card
  (`max_guns` 1, bought in 99-100 % of matches), and shells have their own cap (`shell_mp_max_pct` 25, never binding: budget
  holds 0). It can only move the time of the first gun buy; its counter is the vehicle cards bought a match. It is the first
  lever cut (optional slot). S11b's own ablations (`how_off`, `rg_off`, `light_off`, `how_share_10`) are read from the
  orchestrator's S11b measure, not replayed.

**H5. Germany's price cut turns into men it cannot field.** Germany is held by the pop cap 11.7 plans a match (USA 0), pop at
20 min 120 against 104. But Germany wins 46.7 % of the long matches, where the cap binds most: weak. Experiment: `own_pop170`
(owner class, screened only).

**H6. Builder cards.** The builder floor buys the cheapest builder: Baupioniere for Germany (8 Kar 98k, 165 MP, 2.2 a match),
the full Engineer Squad for the USA (7 Garands, an SMG, a satchel, 280 MP, 2.6 a match), so the USA's builders also fight.
B1 found no fort setting that helps; not screened again. Recorded for the owner (section 9, B2-2).

Expected outcome, said first: H1, H2 and H3 point at unit content (start squads, squad make-up, a gun's tow), which the pass may not change.
The tier 1 levers act on the later buys, not on the first clash. B2 may end as B1 did, with a sharper diagnosis and measured
owner-class levers. That is a valid result (section 8).

## 3. Levers

**Tier 1: the pass may apply these** (TOML, an AI habit or a card's buy weight, inside the range given; none is a price, a
weapon, a squad's make-up, a start squad, a cap or an economy value).

| File (`B2-levers/`) | Key | Now | Screened at | Range | Tests; own counter |
|---|---|---|---|---|---|
| `mix_line35` | `[ai.mix] line` | 50 | 35 | 35-50 | H2 (B1 L5 measured 60 and 70); line cards bought a match |
| `mix_vehicle30` | `[ai.mix] vehicle` | 20 | 30 | 20-30 | S11b-10 displacement, T30, T40, T54; tanks and TDs bought |
| `ger_line_g7_150` | Grenadiergruppe (7) `ai_weight` | 50 | 150 | 50-150 | H1, H2 (an MG 42 per 165 MP); that card's buys |
| `mix_line42` (run 1, optional) | `[ai.mix] line` | 50 | 42 | | the middle dose, only if 35 passes above 53 % |
| `arty_share30` (optional, first cut) | `[ai.arty] income_max_pct` | 15 | 30 | 15-30 | H4, near inert; vehicle cards bought |
| `mix_mg30` (optional, last) | `[ai.mix] mg` | 20 | 30 | 20-30 | splits "fewer line" from "more MG"; MG teams bought |

Symmetric keys act differently because the rosters differ; card weights are per faction, as S7-2, S8a-21 and B1's L6 / L7 were.

**Owner class: screened as override files and reported with numbers, never applied by the pass** (B2-1).

| File | Change | Whose decision |
|---|---|---|
| `own_57mm_truck_tow` | the 57 mm card towed by the unarmed CCKW (H3) | owner, S9-3 |
| `ger_line_vg150` | VG-Sturmgruppe `ai_weight` 50 to 150: the 309 MP squad becomes Germany's most-bought line card | owner (how the faction plays) |
| `own_pop170` (optional) | `pop_cap` 150 to 170 | owner, S4 Q6 |
| `own_ger_price95` (optional) | the nine German infantry cards x0.95 (the S6 tune's knob) | owner, S5a-37; B1 limit "no price of an existing card" |
| `diag_ger_start3` | a third free Grenadiergruppe (diagnostic) | owner (how a faction opens) |

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
- A 40 set decides nothing about the share; the pass plays none.
- The gap to the band's edge is 3.3 points; no affordable run can prove a 3-point gain. **The pass aims at 47-53 %** (a gain of
  6-8 points), which 320 fresh pairs can show at z 1.64 (about +5.7) and the judges then test again (B2-6). A lever that gives +3 is left for the owner, not applied.
- A 160 screen cannot confirm anything. It only ranks. Its false-positive rate at the pass mark below is about 1 in 7 per
  lever, so with five or six levers about one false pass is expected: the confirmation is on matches the screen never saw.

## 5. Method and decision rule (fixed in advance)

Seeds. Screen block S 2001-2080; fresh blocks F1 2101-2180 and F2 2201-2280 (each 80 seeds x both bases = 160 matches; B1 used
1001-1280). Judges 1-80, 101-180, 201-280: played only by the orchestrator's measure, never to search. **Held out 5001-5080**,
once, after acceptance: B1's block 301-380 was already played by S6b (`S6b-log.md` line 61). Checked by a search of `docs/`,
`tools/`, `mods/` and the CLI sources for seed numbers: none at 3000 or above is used anywhere. The base of every block is BASE
unchanged, played with `--side-detail`.

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
**The dose is chosen here, on the screen block only**: if a passing lever puts Germany above 53 % on block S, run 1 plays its
middle dose (`mix_line42`, or the midpoint of the range) on block S; the dose that passes the three rules with its level
nearer 50 is the candidate. F1 and F2 never choose a dose.

**Confirm (run 2).** The candidate on F1 and F2 against their bases (320 fresh pairs). **One rule is the guard:**
1. on the fresh matches alone: **z at least 1.64** (one-sided 5 %; on 320 pairs that is a gain of about +5.7 points), with a
   positive gain in F1 and in F2;
2. Germany's level with the lever on the fresh 320 inside 45-55, and every floor of the screen holds on them.
The line over all three blocks (480) is logged as a description only: it contains the block the lever was picked on, so its
z is inflated and decides nothing. B1's z 2.5 is replaced by two independent tests: the fresh blocks, then the judges.
F1 and F2 are spent once a candidate was read on them. One candidate set is applied, not a mix of partial winners. No rule
is relaxed after seeing a result; a near miss is logged as a miss with its numbers.

**Judge (orchestrator, after run 2).** `tools/measure.sh` on the judges with `B2.toml`. The change stays only if: Germany
pooled 480 inside 45-55; each 160 set inside 40-60; no match without a winner; paired against BASE's measure on the same
seeds the gain is above zero; no hard target that passes in BASE's measure (read with `B2.toml`) now misses; re-based rows are
listed apart and never counted as closed. Otherwise the apply commit is reverted (`git revert`, as B1's `b477036`) and the
log says so. Then the held-out set 5001-5080 once: 40-60 on the set, 45-55 on the pool of 640; if it fails, no retune: report.

## 6. Targets: what is stale, and `targets/B2.toml`

`B2.toml` is generated: `python3 -I docs/slices/targets/B2-levers/rebase_targets.py MEASURE_DIR > docs/slices/targets/B2.toml`.
The committed file was made from `target/s11a2-measure`; run 2 makes it again from BASE's measure (the howitzer displaces
vehicles further, S11b-3). **The new bounds live only in `B2.toml`; no older target file changes, and the owner decides each
(B2-3, (A) assumed).** 16 floors change and 4 notes (T31), each with a `# B2:` line giving the old bound and the reason.

Rule R for floors made stale by design (S11b-10 (A)): 0.85 x the lowest of the three sets, rounded down to 5 (share of matches)
or 0.5 (manpower share); **a near miss (lowest set within 10 % of the old floor) takes 0.95**, so a row that almost holds is
not loosened far. A re-based floor guards against further displacement; it is not a design goal, and the old figure stays in
the comment. Reason for every row: the gun and observer cards take 400-500 MP a match.

| Rows re-based (16) | Old | Measured (three sets) | New |
|---|---|---|---|
| T54a Pz IV bought | 90 | 89.4 / 86.9 / 88.1 | 80 (near miss) |
| T54b M4 bought | 90 | 76.2 / 73.8 / 76.2 | 60 |
| T54c Pz IV manpower | 11 | 10.0 / 10.0 / 10.2 | 9 (near miss) |
| T54d M4 manpower | 10 | 7.0 / 7.0 / 7.4 | 5.5 |
| T30a, T30c, T54e StuG bought | 60, 60, 80 | 60.0 / 48.8 / 48.1 | 40 |
| T30b, T30d, T54f M10 bought | 60, 60, 75 | 32.5 / 30.0 / 33.1 | 25 |
| T40a, T40c, T72a Sd.Kfz. 251/1 bought | 60 | 40.6 / 33.1 / 33.1 | 25 |
| T40b, T40d, T72b M3A1 card bought | 60 | 13.8 / 15.6 / 17.5 | 10 |

Notes only: T31a-d (all vehicles' share of kills, cap 30, Germany 32.0-33.5) stay reports; AT guns (4-4.5 %) and infantry guns
(3.5-4 %) are inside the metric (B2-5). With `--applied` only: T5c, T5d, T5e become the hard per-set band 40-60.

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
- T97a-d (builders' share 16.5-18.2 %, cap 12): NOT re-based. A cap set just above today's value would only bless it; it
  stays a visible hard miss until the owner decides (B2-4).
- T110-T113, T115, T119, T122 (AI use of mines, obstacles, bunker; own-side mistakes): not balance (owner E6, B1-2).
The soft rows named by item 14 are thereby settled: T5c/d (above), T31 (report), T55 (report, owner question), T95 (hard, met).

## 7. Stages and time (4 cores: a 40 set about 2.5 min, a 160 set about 10 min; `BATCH_JOBS=4`)

| Stage | Who | Machine time | Result |
|---|---|---|---|
| B2-1 screen (`B2-run-1.md`) | Tune-tier agent, medium effort (workflows.md 3) | **six 160 sets, about 65 min** with the check: base, `mix_line35`, `own_57mm_truck_tow`, `diag_ger_start3`, `mix_vehicle30`, `ger_line_g7_150` | one line per lever in the log; at most two candidates named, or "none" |
| B2-2 confirm and apply (`B2-run-2.md`) | Tune-tier agent, medium effort | **at most seven 160 sets, about 70 min, plus `check.sh quick` about 5**: two bases, candidate 1 on both, candidate 2 on both only if 1 fails, the applied proof set | the default changed in one commit with before and after, or "nothing passes" |
| Judge | orchestrator | 480 measure about 35 min, held-out set 10 | accept, or revert |
| Review | Review-tier agent, short | no runs | the diff against sections 3 and 5; a cheap stage decided, so a review follows (dev-speed 6) |
| Docs | Docs-tier agent, exact text given | | STATE.md, TODO.md, log at most 150 lines |

Optional sets, only if run 1's six finish early, in this order (so the cut order is the reverse): `mix_line42` (only by the
dose rule), `ger_line_vg150`, `own_ger_price95`, `mix_mg30`, `own_pop170`, `arty_share30`.
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

- B2-1 What the pass may change alone. (A) Only AI rule keys and card `ai_weight`s of section 3's tier 1; prices, pop cap,
  start squads, squad make-up, a gun's tow, the VG-Sturmgruppe's weight and weapons are screened as override files and
  reported with numbers, not applied. (B) Also apply one owner-class lever if it passes the same rule.
- B2-2 If no AI setting passes (the evidence points at unit content). (A) Change nothing, as B1; the owner then picks from the
  measured list: the 57 mm towed by a truck, a third or cheaper German start squad, more VG-Sturmgruppen, the German price
  cut, the pop cap, an AI opening-buy rule (code), or accept about 42 %. (B) The pass applies the best owner-class lever.
- B2-3 The 16 re-based floors (T30a-d, T40a-d, T54a-f, T72a/b; table in section 6; S11b-10 (A) asked for them). (A) As in
  `B2.toml` only. (B) Keep the design floors as visible hard misses.
- B2-4 T97 builders' share (16.5-18.2 %, cap 12). (A) Not re-based: it stays a visible hard miss until the owner chooses a
  cap or lowers `builder_cards_min`. (B) Re-base the cap to about 20.
- B2-5 T31 and T55. (A) Both stay reports; a vehicles-only kill share is a later CLI change; the Sd.Kfz. 234/1 (15 % of
  Germany's kills) is not touched while Germany is under 45 %. (B) Lower `[ai.recon] income_max_pct` now.
- B2-6 The aim. (A) 47-53 %: a gain the runs can prove; a +3 lever is reported, not applied. (B) Apply any gain that reaches 45.
- B2-7 Seeds. (A) Screen 2001-2080, confirm 2101-2280, judges 1-280, held out 5001-5080 once (301-380 was played by S6b).
  (B) Reuse B1's blocks 1001-1280.
- B2-8 The 57 mm's armed tow (H3; S9-3 (A)). (A) Screen the truck tow as an override and report its number; the owner
  decides. (B) Leave S9-3 alone and do not screen it.

**Owner decisions inside this plan, each on (A) assumed 2026-11-19 and to be shown to the owner in the final report:**
the 16 re-based target floors (B2-3, in `B2.toml` only); T97 left a hard miss (B2-4); T31 and T55 left reports (B2-5); the
aim 47-53 % (B2-6); and the three screened-only content levers the pass must never apply: `own_57mm_truck_tow` (B2-8),
`ger_line_vg150` (B2-1), `diag_ger_start3` (B2-2), with `own_pop170` and `own_ger_price95` if played.

## 10. Risks

- Fitting noise: guarded by the fresh blocks, the judges and the held-out set; the screen's mark is weak by design.
- The base moves under the pass: S11b's review or fix changes a default after run 1. Then run 1's lines are void; `queue.sh`
  replays them (the cache key holds the binary and `mods/`). Run 2 compares `tools/batch.sh world` with run 1's key first.
- `mix_line35` may pass by thinning the infantry: fewer line squads on both sides. The floors watch HE, mortar and vehicle
  shares and the match length; the review reads the line squads bought a match before and after.
- A symmetric key that helps Germany by making the US AI buy worse is a fit, not a fix: the log gives both sides' buys.
- No lever file was loaded by the CLI (TOML-parsed only); `paired.py --detail` and `queue.sh check` were written without a CLI run (no `--side-detail` file exists yet): run 1 fixes
  a wrong key name in place and says so.
- Later slices (S12-S15) will move the share again; the pass does not promise more than the judge's number.
