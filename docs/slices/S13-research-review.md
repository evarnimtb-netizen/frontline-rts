# S13 heavy wave: independent plan review (2026-11-19)

Independent review of revision 1 of `S13-heavy-wave.md` (2026-11-19). Revision 2 answers findings 1-19 (split, opt-in blend rule, call sites, H1 redesign, one layout commit, M36 traverse, T151 restated).

Read-only review against code and data. Calculations: the plan's script rerun (its 3.3 ranges reproduce), plus my own enumeration of every
existing AP round x plate at 0 and 30 deg yaw (`enum.py`, `t151.py` beside this file; chance = the closed form of `armour.rs` 265-286).
The plan's line references check out (armour.rs 234-286, gunnery.rs 1169-1222, ai.rs 2031-2060, overwatch.rs 86, vehicle.rs 52 / 550-552 /
1541-1557); only `[limit_groups]` is skirmish.toml 1235, not 1232.

## Findings

1. **BLOCKER (process). The slice is not split as the owner rule requires.** `docs/TODO.md` 40 and `dev-speed.md` 79: from S10 on an engine slice
   is a mechanics slice (cards at `ai_weight` 0, proven neutral) plus an AI-and-balance slice. The plan ships O1, five cards with `ai_weight`
   20-100, three AI rules and the `3in_m62` row change in one slice with one 480 measure. Fix: S13a = O1 + data, cards at `ai_weight` 0;
   S13-ai = H1-H3 and weights. (The plan already uses the name "S13b" for mortars; rename.)
2. **BLOCKER (measure). The 480 measure cannot say what the plan wants it to say.** (a) "All switches off = base, match for match" (section 0) is
   false after Build B: new cards with `ai_weight` > 0 change AI buys and there is no switch for content; it holds only at Build A's commit.
   (b) O1's effect on existing balance, the new cards, the AI rules and S13-9 are confounded in one number; the ablations run on the 40 set,
   whose noise is 11-15 points (STATE.md "How to check work"), so `o1_off` cannot show a leak. (c) The AI will almost never field heavies
   (finding 9), so T150 passes trivially and says nothing about Panther or Tiger balance. Fix: a full 480 at Build A (O1 on, no new content) vs
   base is the O1 leak measure; then 480 after data with cards at weight 0 (match results must equal the A run's); then 480 with AI. Make T155
   a hard floor for "the measure covers heavies" (e.g. Panther and M36 each in >= 25 % of matches), else the guard band reading is void.
3. **MAJOR. The default `k_max` pair fails the plan's own T151c, so Build A's exit check ("T151a-f with the defaults") fails.** 90 mm M82 vs
   glacis 80 @ 55 at 100 m: T/d 0.889, k 1.311, eff 165.8 vs pen 164, chance **35.5 %** (target <= 20 %). The 3.3 table's "never" hides this:
   the 50 % point is under 100 m but the chance is 35 % at 100 m and about 1 % at 350 m. Feasible pairs exist (grid search, step 0.05):
   `apcbc_k_max` >= 1.45 with `apcr_k_max` only 1.90 or 1.95. The APCR window is one or two grid steps wide (T151b), so it is fragile.
   Fix: recompute 3.3 as chances, not 50 % ranges; state the feasible window; start at 150 / 190.
4. **MAJOR. Quantisation as specified moves results by more than the calibration margins.** k quantised to 1/20: M82 vs glacis k 1.311 -> 1.30
   gives **43.4 %** instead of 35.5 %. A 256-step cosine table without interpolation: at c = 0.574 one step changes c^k by k*dc/c = 0.95 %
   (APCBC) to 1.3 % (APCR), which is 14-19 % of the +-7 % band, so near the 50 % point the chance jumps by 14-19 points between neighbouring yaw buckets.
   Determinism itself is fine (integer series at setup, tables not saved), but `ck_table_is_deterministic` with "threads 1 vs 4" tests nothing
   that can vary. Fix: see finding 5, or interpolate linearly in c and keep k unquantised per (plate, ammo class) pair computed at setup.
5. **MAJOR. A simpler rule was missed: no table, no exp/ln.** Blend the cosine law with a steeper one using operations `Fx` already has
   (`fx.rs` 198 `sqrt`): `c_eff = (1 - w) * c + w * c * sqrt(c)` for AP/APCBC (k 1.5 at w = 1) and `(1 - w) * c + w * c * c` for APCR, with
   `w = clamp((T/d - lo) / (hi - lo))`. Exact fixed point, continuous in c and T/d, two keys fewer. Check: glacis vs 75/76 mm = 184 mm
   (US 1944 "vertical equivalent" 187, history note 174); M82 at 100 m = 172 mm, chance 4.6 % (T151c passes); HVAP = 243 mm, 29 % at 100 m
   (T151b's 30 % floor needs a small weight tweak).
   Second alternative: B&L slope-multiplier rows per class in TOML, linear interpolation; that is the cited source's own form.
6. **MAJOR. The integration is larger than "pen_chance_ppm takes c^k".** The sim decides with `armour::penetrates`, not the closed form:
   `damage.rs` 575 (plates), 544 (shields), shared by the small-arms and mortar paths (damage.rs 519-533, no ammo def and no calibre there).
   Also `armour.rs` 885-893 (`plate_shot`, the `ballistics` report), `effective_thickness` (255), `gunnery.rs` 1211 (`special_better`), the
   hit log record and its invariant in `sdk/crates/cli/src/armour_test.rs` 349-354 ("every logged hit equals the rule"), `armour_test/s8b.rs`
   906. Fix: list all sites; state k = 1 for small arms, mortar `pen_mm`, shields (or not) and fragments (`combat.rs` 2867); `HitRecord`
   carries k or c_eff.
7. **MAJOR. O1 changes existing matchups far more than sections 3.3 and 8.2 admit; they were checked at 0 deg yaw only, and several plates were
   missed.** Chance at 350 m, base -> O1 (defaults):
   - StuG hull front is **80 @ 21** (`ger_stug3_g.toml` 70), not "flat": 75 mm M61 21 % -> 3 %; 57 mm M86 78 % -> 45 %.
   - Sherman glacis vs KwK 40 PzGr 39 at 30 deg yaw: 100 % -> 79 %, 50 % range 1075 m -> 475 m ("defeated everywhere" holds only head-on).
   - Sherman glacis vs 5 cm PzGr 40: 100 % -> 17 % (0 deg). M10 turret front 57 @ 45 (`usa_m10.toml` 93) vs 5 cm PzGr 40 at 30 deg: 100 % -> 0 %.
   - 2 cm PzGr 40 (Sd.Kfz. 234/1) vs M10 hull side 19 @ 38 at 30 deg yaw: 100 % -> 0 %; vs M10 turret side: 100 % -> 6 %; 2 cm PzGr 39 vs M8
     front 74 % -> 60 %. 37 mm M51 (M8) vs Panzer IV turret front at 30 deg: 100 % -> 87 %.
   `rare` as the APCR class also catches the 2 cm and 5 cm PzGr 40 at k 1.9. Fix: put this table in the plan; T151g/T154 need named rows
   for these pairs at 0 and 30 deg; consider a `slope_class` ammo key instead of reusing the supply flag `rare`.
8. **MAJOR. Rounds with `pen_reference_angle_deg = 30` become inconsistent.** 57 mm M86, 5 cm PzGr 39/40, 37 mm M51 and the 2 cm rounds are
   normalised by `/cos 30` (`armour.rs` 301-312). Under O1 they no longer reach their own published 30 deg figure on a plate with T/d >= 1
   (short by cos30^0.4 = 5.6 % for AP, cos30^0.9 = 12 % for APCR). Fix: normalise as `pen_ref * (c / cos_ref)^k`, or convert those rows to
   0 deg rows (a gun data change: owner question), or exempt them.
9. **MAJOR. H1 cannot work as written against the buy code.** (a) The "stall" at ai.rs 2056-2060 is the only way the AI ever saves more than
   `tank_reserve_mp` 200 (skirmish.toml 981): `saving` (ai.rs 1997-2003) looks at `best(r)`, the highest-weight card, not the heavy. Income is
   240-540 MP/min, so `heavy_wait_s` 60 followed by "take the next ready card" spends the savings and a 1050-1200 card is never reached.
   (b) The pick is fewest buys per weight (ai.rs 2046-2050): with Panzer IV, StuG and Sd.Kfz. 251 at weight 100, a Panther at 40 is first
   picked after two buys of each, a Tiger at 20 after four of each; a Jumbo at 30 after three each of four US cards. The AI buys the M10 in
   32 % of matches today (STATE.md), so Tiger and Jumbo buys will be near zero. (c) `[vehicles] limit` 4 is usually full of mediums before 8:00
   and 12:00. (d) `heavy_income_min` has no default. (e) State is under-listed: `enemy_vehicle_seen` is a latch for any vehicle (ai.rs 309,
   2569), so H1 needs a saved tick filtered by role, plus a saved "waiting since" tick. Fix: H1 picks the heavy directly when armour was
   seen and the card is ready, saves for it with a time bound derived from cost and income, and may replace a medium slot; add a test "heavy
   bought within N min at income X"; specify what happens against an opponent who never buys armour (never bought: say so).
10. **MAJOR. One layout commit is not achieved.** H2's `ai_overwatch` def key adds a `VehicleSetup` field, and every `[ai.armour]` key of
    H1-H3 enters `MatchSetup`, which is postcard-encoded into the save and the net `setup_hash`. The plan puts only O1's keys and "H1's field
    if needed" in Build A; Build C would change the layout after the SAVE bump. H3 also needs per-tank saved state ("last rearm source") and a
    counter in `AiCounters` (ai.rs 473, serialised). Fix: Build A's commit carries every key, def field and state field, all defaulting to off.
11. **MAJOR. H3 ignores an existing feature.** `[ai.armour] rearm_ap_below` 3, `rearm_full_pct`, `rearm_max_s` (skirmish.toml 982-990, S3c)
    already send a tank back to rearm. H3 (8 rounds, 600 m) is a second trigger for the same act and differs from the design's numbers
    (`vehicles.md` 7.5: 300 m, 10 rounds) without a reason. Fix: specify H3 as an extension of the S3c rule and state the interaction.
12. **MAJOR (history). M36 traverse 5 deg/s is wrong and contradicts the plan's own note.** `S13-research-history.md` 200 and 259: powered,
    max 24 deg/s (AFV Database); the M10 was the hand-traversed one. `vehicles.md` 4.4 has the same error. Since traverse decides duels, this
    cripples the main US counter. Fix: 24 (owner-visible correction of the design table). Jumbo 18 is unsourced (note 201: "about 20 s per
    360" = 18, marked from memory); keep as (verify) but say so.
13. **MAJOR (history). T151d contradicts the cited source, and the script did not use the planned plate.** Note 176 (Jan 1945 Ordnance pamphlet):
    M82 "penetrates all Panther plates except glacis and gun mantlet"; the 800 yd figure there is for HVAP. With S13-2 (A) the turret front
    is the mantlet, yet T151d demands M82 >= 80 % at 800 m, and risk 8.1 and armour-test case 42 rely on "M36 wins through the turret". The
    script models the turret front as 100 @ 11; the vehicle table says 100 @ 30. Fix: decide which source rules, restate T151d, rerun.
14. **MAJOR (owner rule). "Mantlet 100 @ 30" is a calibrated stat, not a published one; say so and ask.** Published: turret front 100 @ 12, cast
    curved mantlet 100. Under O1, 100 @ 30 is 122 mm effective against 76 mm M62 (base 102): 86 % -> 21 % at 350 m. It does fit Isigny (M62
    holes the mantlet at 200 yd or less: about 60 % at 183 m), so it is defensible, but it is a tuned number and S13-2 presents it as plain
    representation. Tiger 120 @ 0 is the commonly published mantlet figure over a 100 mm face: defensible. Jumbo: the plan folds mantlets
    for the German tanks but gives the Jumbo 152 @ 0 where its gun shield was 178 mm (note 91): inconsistent. Fix: S13-2 becomes a
    "how it should play" question with the three numbers and their effect; Jumbo 178 or a stated reason.
15. **MAJOR. The Panther nose leaves the frontal fight open at the game's own engagement range.** Nose 60 @ 55 vs 76 mm M62: T/d 0.79, k 1.23,
    eff 119; pen at 350 m 119.4, so about 50 % per nose hit at the 350 m spotting range, and HVAP beats the nose past 1 km. The 12th Army Group
    test (note 174) had HVAP at 1 of 4 on the nose at 200 yd and M62 "unsatisfactory". The plan's table calls this "still generous" and
    moves on; the hull box's front face is the nose (as on the Panzer IV, `ger_pz4_h.toml` 99-103), so it will be hit often. The ramp from
    k = 1 at T/d 0.5 is the cause; B&L's dependence on T/d is weak. Fix: add a nose row to T151 (e.g. M62 vs nose at 350 m <= 20 %), lower
    `td_lo` (0.25) or use finding 5 with a floor on w, and report the frontal hit share by zone in armour-test 41-43.
16. **MINOR. Auto will waste the two rare rounds.** `special_better` (gunnery.rs 1220-1222) fires the special round whenever the standard
    chance is under `special_below_pct` 50 and the special is higher by any amount. Under O1, M93 vs glacis at 350 m is 5 % against 0 %.
    Fix: a floor key (`special_min_pct`, e.g. 25) in Build A's layout commit, or accept it and report in T156.
17. **MINOR (data).** (a) 90 mm M82 at 808 m/s is the early loading; `vehicles.md` 4.5 lists 853 m/s. State which the M36 of Oct 1944 fires.
    (b) M62 and M93 at 1500 m are 1 mm off the usual table (97, 147). (c) Fixed traverse 12 and 9 deg/s are midpoints between sourced idle
    (6-8) and high-rev (19-20) rates, not "sourced" rates; on H2 overwatch the idle rate is the honest one. (d) Checked and fine: Panther G
    and Tiger I plates, racks 82 / 92 / 71 / 47 / 104, Jumbo 101.6 @ 47, speeds, weights, the KwK 42 and KwK 36 rows, all velocities.
18. **MINOR (process).** (a) `workflows.md` 1 still lists S13 as a data slice: record the change. (b) The stage table has no Fix row (Opus).
    (c) Build B on Sonnet inside an engine slice deviates from section 3; fine for a pure data stage if said. (d) S13-9 changes an existing
    gun row under "do not touch guns" and confounds T150: move it out or measure it alone. (e) Armour-test cases 41-44 are "report" with no
    bound and no control: give each a bound and an `o1_off` control run.
19. **MINOR (owner questions).** S13-1: keep "asked", but show the table of finding 7 and the simpler rule of finding 5 as an option. S13-2:
    finding 14. S13-3: keep (A), with the M36 at 24. S13-10: (A) only after finding 11. Missing: the S13a / S13-ai split; whether AI heavies
    may displace mediums under `limit` 4; whether 30-degree-reference rounds are converted; `special_min_pct`.

## Verdict

Not ready to build. The diagnosis is correct (cosine law + B&L 0 deg rows lets the 90 mm and HVAP through the Panther glacis) and O1's framing is sound.
But the defaults fail the plan's own T151c, O1 leaks into existing fights well beyond what is stated, and the M36 traverse and T151d contradict the plan's own note.
Revise: split per the owner rule, restate O1 (simpler blend, all call sites, 30-degree rounds, nose row), redesign H1, one layout commit, measure O1 alone on 480.
