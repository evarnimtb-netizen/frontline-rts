# Slice S3b log: physical damage, armour balance, trench pinning

Spec: `docs/slices/S3b-balance.md`. This file holds the Tune stage's
round-by-round measurements; the final targets table is in the spec's
"Tune results".

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
| T12 | 11.2–26.2 min (3 over 25, 2 under 12); HE max 44.2 % |
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
