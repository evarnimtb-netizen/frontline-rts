# Slice S3b log: physical damage, armour balance, trench pinning

Spec: `docs/slices/S3b-balance.md`. This file holds the Tune stage's
round-by-round measurements and the Fix stage (below), whose targets
table is the slice's final one (the spec's "Tune results" is superseded).

Every round measures the same suite: `skirmish --ai-vs-ai --seeds 10
--both-bases --summary --threads 1 --jobs 6` (20 matches),
`armour-test --stress` (seeds 1–5 × both factions), `fortify-test`
(5 seeds). Decisions on noisy AI-match numbers also used 40 matches
(seeds 1–20 × both bases) and case 3 over seeds 1–20; those are marked.

## Round 0: Part B as committed (`85f3bf0`)

| Target | Value |
|---|---|
| T1 | GER 7 / USA 13; base 1 wins 15 (75 %) |
| T2 | 3a Panzer IV 4 of 10 (8 of 40 over seeds 1–20 after the basket fix) |
| T4 | 47 KOs: ammo fire 26, rack 13 (28 %), crew 7, engine fire 1 |
| T5 | case 6 60 %; case 4 flank 41.2 % |
| T6 | 86.2 % |
| T7 | rockets 6 of 10 (normal bases); satchels 0 of 20 |
| T8 | case 7 attackers 11, defenders 31 of 80 (38.8 %) |
| T9 | 5b pinned 54.2 %, ducked 42.7 %; kills 0 |
| T10 | defenders 53.8 %, attackers 35.0 % |
| T11 | rifles 0 vs 80, mortar 6 vs 58 (10.3 %), unsupported 100 % |
| T12 | 12.2–24.6 min; HE max 44.6 % |
| T13 | peak p99 0.89 ms; stress 1.83 ms |
| T14 | 0.0013 m per burst (41 m mean) |

Diagnosis that drove round 1 (case 3a hit logs): both tanks fired at
0.6 s and both penetrated the other's front on the first exchange; tanks
then survived 5–12 frontal penetrations, because the filler's rays
(reach 1.5 m from a burst 1 m behind the plate) ended in the empty middle
of the hull, and the turret crew sat wholly above the turret ring, so a
hull hit at the visible-mass height passed under them. The Sherman then
won on its 0.5 s faster reload. The M61's penetration row was ~6 % above
the US published table, so it beat the Panzer IV H's 80 mm front at 300 m
88 % of the time.

## Round 1 (`1bd66bd`)

Changes: M61 table to the US figures (golden rows moved), filler reach
300 cm, turret basket (code + data), `moving_target_spread_pct` 600, mortar
`frag_count` 120, `at_hunt_m` 400, `satchel_hunt_m` 150; harness close-in
for case 7 and fortify's suppressed assault; T14 local row.

A/B probes before applying (override files, not committed):
- 3a: M61 only → Panzer IV 4 / Sherman 2 / 4 no knock-out; filler 300
  only → 4 / 6; both → 6 / 4 (32 of 40 over seeds 1–20).
- 3d: spread 500 % → 64.6 %, 550 % → 60.0 %, 600 % → 55.4 %, 700 % → 40 %.
- HEAT jet 150 cm: case 4 and 6 unchanged (41.2 %, 60 %).
- `at_hunt_m` 400 + `satchel_hunt_m` 150: rockets 8 of 10; `mix_at_pct` 15
  on top: a draw at 30:00.
- Mortar 120 fragments: open 60, trench 6 (10.0 %).
- Two frags per rifleman: T10 defenders 66.2 %, case 7 52.5 % (not
  applied; the close-in harness met both without it).
- `spoil_crater_pct` 250 / 400: T14 local 0.033 / 0.034 m (not applied).

| Target | Value |
|---|---|
| T1 | GER 6 / USA 13 / 1 draw; base 1 11 of 19 |
| T2 | 6 of 10 |
| T4 | 40 KOs: ammo fire 21, rack 8 (20 %), crew 10, engine fire 1 |
| T5 | case 6 60 %; flank 34.3 % |
| T6 | 55.4 % |
| T7 | rockets 6 of 10; satchels 1 of 10 (3 of 20, 7 throws) |
| T8 | attackers 21 vs 80; defenders 100 % |
| T10 | defenders 97.5 %, attackers 48.8 % |
| T11 | pass; mortar 6 vs 60 (10.0 %) |
| T12 | 11.6–30.0 min (one time-limit draw); HE max 52.6 % |
| T13 | peak 0.74 ms; stress 3.06 ms (other runs in parallel) |
| T14 | local 0.027 m per burst |

## Round 2 (`38f09b5`)

Change: Panzer IV 600, Sherman 620. Measured over 40 matches first: with
the round-1 data Germany 15 / USA 23 (2 draws); with these costs 23 / 17.
Satchel probes (slow speed 2 m/s, slow 2 s, hunt 250 m): 0–2 throws in 20
matches, not applied. Satchel-hunt traces (seeds 8, 16): hunts start
100–145 m out and end before a throw as the tank moves or drops from view
or the engineers die.

| Target | Value |
|---|---|
| T1 | GER 11 / USA 9; base 1 11 (55 %) |
| T7 | rockets 8 of 10; satchels 0 of 10 |
| T12 | 11.2–26.2 min, median 18.1 (3 over 25, 2 under 12: met under the owner's loosened target of `031791e`, 10–40 min, median 15–30); HE max 44.2 % |
| T13 | 1.57 ms (cargo tests running alongside; re-measured in round 3) |

Staged cases unchanged from round 1.

## Round 3: final measurement and tests

No data change. Test fixes for the data (each says why in a comment):
golden rows and the M61 ballistics test (12.8 % at 300 m), the M61 armour
test (100 m now borderline), the pinned AI values (`at_hunt_m`,
`satchel_hunt_m`), costs, the mortar's fragment count and budget, the
moving-target spread, the seat-box validation test (gunner now below the
basket), and the open-ground fragment test (three fresh men and spread
shells: with 120 rays one prone man under 12 bombs in earlier craters took
none, the earth blocking as it should).

Final numbers: the spec's "Tune results". Peak p99 0.94 ms, stress p99
1.14 ms. Tests: 430 passed, 5 ignored in debug; the release-only ones
(soaks, vehicle stress, fortify, AI-vs-AI winner) pass; clippy `-D
warnings` and fmt clean; Godot headless `check_project` OK,
`test_match_view` 192/192, `test_net_ui` 143/143, `test_data_reload` 8/8;
`settings.cfg` unchanged.

## Fix stage (2026-11-01): review findings, final measurement

Two reviews of the Tune stage (determinism/correctness and
balance/realism) found nothing that breaks determinism and two high-
severity problems with the Tune report: targets judged on too few seeds,
and an M61 penetration row that rested on mixing two nations' acceptance
criteria. This stage fixed every confirmed finding (table below), re-
judged every target on wider seed sets, and records what still misses.

Measured on the committed Fix data (`5f6f27f`): skirmish seeds 1–40 ×
both base orders (80 matches, 6 at once, `--threads 1`; seeds 1–10 are
the standard 20), armour-test `--stress` on seeds 1–10 (and the CI
default 1–5), fortify-test on seeds 1–10 (and 1–5). Raw outputs were in
the stage scratchpad (`fix/final/`); the numbers are copied here.

### What changed and why

| Change | Why | Source / reasoning |
|---|---|---|
| **AP rows from one source** (review H2): M61 88/81/73/65/59 mm, PzGr 39 135/123/109/97/86, PzGr 40 176/154/130/109/92 at 100/500/1000/1500/2000 m, all at 0° (`pen_reference_angle_deg = 0`) | The Tune row was the US 30° table divided by cos 30°; US and German wartime tables used different acceptance criteria, so mixing them under-rated the M61 by ~7 mm (13 % vs the Panzer IV H front at 300 m; now 90 %, the S3 row gave 88 %) | Bird & Livingston, *WWII Ballistics: Armor and Gunnery* (Overmatch Press, 2001), pp. 61–63, 50 % success vs RHA, as quoted on Wikipedia's "75 mm gun M2–M6" and "7.5 cm KwK 40" pages. 17 golden rows moved (4, 5, 7, 10, 12–16, 18, 19, 28, 29, 32, 38, 44, 45), each noted |
| **HEAT spall 3 → 7 rays** (M1), Panzerschreck and Bazooka | ~9 penetrations per disabled tank in case 6; 3 rays usually crossed nobody | Reasoning (no measured source): behind-armour debris of a perforating shaped charge is at least as dense as an AP shot's spall (6 rays). 7 is the most the sim traces; validation allowed 8 and the sim silently kept 7, now both say 7 |
| **APHE filler rays by burster** (L4): PzGr 39 10 → 8, M61 10 → 12 | Equal fillers although the M61 carries ~4× the explosive (~2× TNT-equivalent) | Reasoning; the defs' own filler masses (17 g vs ~70 g). Over 40 seeds case 3a moved 46 → 44 Panzer IV wins (noise) |
| **Moving-target spread × crossing share** (M2) | A tank driving straight at the gunner got the full ×6 spread | Code (`gunnery.rs` `cross_pct`): the extra spread scales with \|sin\| of the angle between the target's velocity and the line of fire. 3d (pure crossing) unchanged: 60.0 % on 10 seeds, 57.3 % on 40 |
| **Vehicle MG stowage** (M4): `rounds` per coax / hull mount (Panzer IV H 2 × 1575 = 3,150; M4A3 (75) 2 × 2375 = 4,750); a dry MG is silent; base rearm refills one belt per MG per step | Suppression cost nothing | Jentz & Doyle, *Panzer Tracts 4-3*; Hunnicutt, *Sherman*, data sheets / TM 9-759. New state `MgState::spent` (saved, hashed, validated; within save version 10, already bumped once this slice). HUD tooltip shows "MG n" |
| **Costs: Panzer IV 620, Sherman 590** (M3) | Re-balanced on the Fix data, smallest move from S3 (640 / 560) | 40 matches each (seeds 1–20 × both bases): 640/560 Germany 12, USA 27, 1 draw, base 1 20 of 39; 600/620 18 / 22, base 1 28 of 40 (70 %); **620/590 17 / 21, 2 draws, base 1 21 of 38** |
| **Heads-down status** (L1): per-man `ducked` in the snapshot and bridge; a squad at least half ducked shows "Heads down" (roster, card) and a "v" marker | A squad not returning fire showed no warning | `match_frame.rs`, `match_node.rs`, `match_view.gd`, `unit_overlay.gd`; Godot check added |
| **Validation** (det 1, 2): seats + component boxes ≤ `MAX_INNER_BOXES` (14) in def and setup checks (the sim's truncate is now a `debug_assert`); vehicle suppress lines go through `suppress_ok`; a wreck with a fire is refused on load | A mod tank with 15+ boxes lost its last boxes silently; crafted saves loaded | Tests per case (content, `world.rs`) |
| **Docs** (det 3, 4): relay protocol lists `SuppressUnits`, `Apcr = 3`, the 1–2 point limit; the `LEAD` and `SUPPRESS` RNG comments match the code | — | — |
| **Harness**: fortify-test gains a report-only row "MG suppressing vs a trench with its own MG team" (M5) and prints T10 per entrenched faction (H1); case 4's "no launcher shot at a moving tank" uses the sim's own ~1 s averaged velocity (L2: seed 8's one violation was a tank pulling away from a stop, still "still" to the launcher rule) | — | — |
| **Comments** (L4): `skirmish.toml` duck comment no longer says "× 50 % covered"; the Sherman's 63.5 mm / 47° glacis is noted as the later hull at near-equal line-of-sight thickness (93 vs 91 mm), kept | — | — |

Not changed: no step (d) stat change; no tank AI angling; no save or
protocol version bump (both were bumped once earlier in the slice).

### Final targets (Fix stage)

| # | Target | Tune claimed | Final, standard set | Final, wide set | Met? |
|---|---|---|---|---|---|
| T1 | each faction 35–65 %; no base side > 70 % | GER 11 / USA 9; base 1 55 % | seeds 1–10: GER 8, USA 10, 2 draws; base 1 12 of 18 (67 %) | seeds 1–40 (80): GER 31 (39 %), USA 47 (59 %), 2 draws; base 1 45 of 78 (58 %); wins by base: USA 20/27, GER 13/18 (base 0/1) | **met**, Germany near the lower edge |
| T2 | 3a: Panzer IV wins 6–8 of 10 | 6 of 10 | seeds 1–10: 4 of 20 (Sherman 12, 4 no KO) | seeds 1–40: 44 of 80 (Sherman 28, 8 no KO) = 5.5 of 10 | **missed** (S3b-38) |
| T3 | 3c ≥ 7, 3e ≥ 7 of 10 | 10 / 10 | 20 / 20, 20 / 20 | — | **met** |
| T4 | rack ≤ 50 % of KOs; crew and fire > 0 | 16 % | 64 KOs: rack 11 (17 %), ammo fire 32, crew 19, engine fire 2 | 221 KOs: rack 40 (18 %), ammo fire 102, crew 76, engine fire 3; **ammo-caused (fire + rack) 142 = 64 %** (L3) | **met** as written; ammo still causes ~2/3 of KOs |
| T5 | case 6 ≥ 60 %; case 4 flank ≥ 50 % | 60 %; 34.3 % | 10 seeds: case 6 16 of 20 (80 %); flank 45.8 % (27 of 59) | 5 seeds: 80 %; flank 50.0 % (14 of 28) | case 6 **met**; flank **missed** on 10 seeds |
| T6 | crossing ≤ 60 % of stationary | 55.4 % | 10 seeds 60.0 % | 40 seeds 57.3 % | **met** (at the limit on 10 seeds) |
| T7 | rockets in ≥ 8 of 10; satchels at a vehicle in ≥ 3 of 10 (normal bases) | 8 of 10; 0 | seeds 1–10: rockets 7 of 10; satchels 1 of 10 | seeds 1–40: rockets 26 of 40; satchels in 3 of 80 matches | **missed** (both) |
| T8 | case 7: attackers ≤ 50 % of 7b's, defenders ≥ 75 % | 21 vs 80; 100 % | 10 seeds: 39 vs 160 (24 %); 160 of 160 | — | **met** |
| T9 | case 5b pins > 0 % and kills > 0 | pins; 0 kills | 10 seeds: pinned 54.6 % + ducked 42.2 %; kills 0 | — | pins **met**; kills **missed** (S3b-27 (A)) |
| T10 | fortify MG + rifles: defenders ≥ 75 %, attackers ≤ 50 % | 97.5 %; 48.8 % | 10 seeds: defenders 137 of 160 (85.6 %), attackers 92 of 160 (**57.5 %**). USA attacking (M1919): 73.8 % lost; Germany attacking (MG 42): 41.2 % | 5 seeds: 97.5 %; 48.8 % (USA attacking 75 %, Germany 22.5 %) | **missed** on 10 seeds (S3b-41) |
| — | (report, M5) the same against a trench with its own MG team | — | 10 seeds: defenders lost 2 of 220 (0.9 %), attackers 158 of 160 (98.8 %) | — | report |
| T11 | fortify S1 rows pass; unsupported rush loses ≥ 75 % | pass | 10 seeds: rifles 0 vs 160, mortar 14 vs 120 (11.7 %), smoke pass; unsupported 100 % | — | **met** |
| T12 | no match < 10 or > 40 min; median 15–30; HE < 60 % | met | seeds 1–10: 12.7–30.0, median 16.4; HE max 45.7 % | 80: 11.0–30.0 (2 draws at the 30-minute limit), median 16.1; HE max 53.6 % | **met** |
| T13 | peak p99 < 1.5 ms; stress p99 < 10 ms | 0.94; 1.14 | peak 0.94–0.96 ms (6 parallel); stress 1.09 ms (5 seeds) | stress 8.05 ms on the 10-seed run while the Godot bridge compiled alongside | **met** |
| T14 | HE lip loss ≥ 0.05 m per burst within 3 m | 0.027 | 5 seeds 0.027 m | 10 seeds 0.033 m | **missed** |
| T15 | determinism, save/load, net loopback/soak | pass | tests below | — | **met** |

Tests: `cargo test --workspace --release` 434 passed, 0 failed, 3 ignored;
the 3 ignored ones pass with `--ignored` (`soak_20_minutes_seeds_1_to_3`,
`soak_with_vehicles_36000_ticks`, `vehicle_stress_report`); clippy `-D warnings` and `cargo fmt --check` clean; golden
ballistics 46 of 46; `validate mods/ww2` ok; armour-test hard checks pass
on seeds 1–10 (case 4's moving check now 0); fortify-test PASS on 5 and
10 seeds. Godot headless (bridge rebuilt): `check_project` OK,
`test_match_view` 193/193, `test_net_ui` 143/143, `test_data_reload`
8/8. A plain `--autoplay 10` at 4× against the AI ran to the time limit:
2 Panzer IVs bought, an enemy Sherman burning in the tank screenshot, the
roster showing "Heads down" on an MG team; 3 S1b shots it needs a longer
match for (frag tool, frag into a trench, 1280 HUD) were not taken.
`settings.cfg` unchanged.

### Review findings and what was done

| Finding | Severity | Done |
|---|---|---|
| det 1: inner boxes silently cut at 14 | low | fixed: `MAX_INNER_BOXES` validated in def and setup checks; `debug_assert` in the sim. Also found: spall rays allowed 8, sim kept 7: now 7 everywhere |
| det 2: vehicle suppress line and burning wrecks not validated | low | fixed, with load-rejection tests |
| det 3: relay protocol reference out of date | low | fixed |
| det 4: two RNG comments wrong | low | fixed |
| H1: targets met only on Tune's seeds | high | fixed in reporting: every target above on ≥ 10 seeds and 80 matches, T1 per base, T10 per faction; T2, T5 flank, T7, T10, T14 marked missed |
| H2: M61 row not a valid realism argument | high | fixed: one source and criterion for all three AP rounds; T2 re-judged (missed) |
| M1: rockets rarely knock out | medium | fixed in part: spall 7 rays (case 6 80 %, flank 45.8 %); rocket KOs in matches still few (16 of 221 over 80 matches): needs bail-out (S3b-5) |
| M2: ×6 spread for head-on movers | medium | fixed (crossing share) |
| M3: cost change hides an edge elsewhere | medium | investigated: on the Fix data at 600/620, kills by weapon class are even (Germany 3879 kills / 3750 losses over 40 matches; rifle, MG and mortar kills within 9 % of each other; Germany's vehicle MGs killed 616 to 474); at 640/560 the USA's edge was its tank losses (Germany 58, USA 41). Costs set to 620/590; base 1 wins 58 % over 80 |
| M4: suppression costs nothing; tank pins from beyond rocket range | medium | MG stowage built. The range point is not changed: a tank suppressing from 200 m, outside the launchers' 100–120 m, is how tanks were used; the counter is mortars and the towed AT guns of S3c (S3b-8) |
| M5: trench fights all-or-nothing | medium | report row added: equal forces (an MG on each side) the attack fails (98.8 % lost, defenders 0.9 %). The 3:1 rule holds; the all-or-nothing shape remains (known gap) |
| L1: ducking invisible | low | fixed (Heads down) |
| L2: case 4 hard check fails on seed 8 | low | fixed: the harness judged "moving" by instantaneous speed, the sim by the 1 s average |
| L3: T4 met partly by renaming | low | reported: ammo (fire + rack) 64 % of KOs |
| L4: equal fillers; stale comments | low | fixed |

### Open owner questions (work proceeds on (A); also in `docs/TODO.md`)

- **S3b-38. T2 (300 m frontal duel).** With consistent penetration data both
  tanks penetrate each other's front at 300 m, and the Panzer IV wins 44 of
  80 (55 %) over 40 seeds, 4 of 20 on seeds 1–10. (A) Treat a near-even
  300 m frontal duel as historical; judge the Panzer IV's edge at range
  in a new 600 m case next slice. (B) Keep "6–8 of 10" and change a stat
  as a documented step (d) deviation.
- **S3b-39. Vehicle MG stowage.** (A) As built: real stowage per mount,
  one belt per base rearm step. (B) No limit.
- **S3b-40. Base-side bias.** Base 1 wins 58 % over 80 matches (67 % on
  seeds 1–10; it was 70 % in the plan baseline before S3b). (A) Look at
  the map and AI routes in the next slice. (B) Accept.
- **S3b-41. T10 by faction.** US attackers with the M1919 lose 74 %, German
  ones with the MG 42 41 % (10 seeds). (A) Accept: the rate-of-fire gap is
  historical; keep T10 pooled with a per-faction report and let the AI
  give the US base of fire a second MG or rifle squad (next slice). (B)
  Tune the M1919's suppression as a step (d) deviation.
- **S3b-42. AI rearm.** The AI never sends a tank back to rearm, so a tank
  whose MGs run dry stays silent with its MGs. (A) Next slice: the AI
  rearms a tank when both MGs are dry or its HE is at the reserve. (B)
  Leave it.

### Known gaps

- T2, T5 flank, T7 (rockets and satchels), T9 kills, T10, T14 are missed
  (see the table and S3b-35, -36, -27, -22, -38, -41).
- Rockets penetrate often but knock out rarely in matches (16 of 221 KOs);
  bail-out (S3b-5) is the next lever.
- Ammo still causes two thirds of knock-outs (an ammo fire always ends
  the tank after `ammo_fire_s`).
- The HUD shows MG stowage only in the vehicle tooltip, not on the card.
- `MAX_INNER_BOXES` and `MAX_SPALL_RAYS` are engine caps in code (they
  size per-tick work), not tuning values.
