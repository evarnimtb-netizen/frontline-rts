# S5b-mortars log

Spec: `docs/slices/S5b-mortars.md`. Baseline: the spec's "Baseline"
section (`3cd5717`, raw outputs in the scratchpad `s5b/base_*.txt`).

## Part A: FireMission, modes, observers, ranging, charges, 5-man squads, bench

Commits: `f065df0` (WIP of the interrupted first run, unfinished and
untested), `3ee376a` (review fixes, tests, `mortar-bench`, docs), and the
Part A close commit with this log. Raw outputs: scratchpad
`s5b/partA2/` (`skirmish40.txt`, `bench50.txt`, `fortify10.txt`,
`armour5.txt`, `armour_stress10.txt`), the bench sweep in `s5b/bench/`,
the fortify diagnosis in `s5b/fort/`.

### What was built (A1-A8)

- **Content and setup (A1).** Weapon `[indirect]` (`IndirectDef`:
  `charges_m_s`, `elevation_deg`; lowered to `IndirectSetup` with the
  tangents computed once at load), checked with actionable fixes (charges
  rising, 1-12, 20-600 m/s; limits 45-89 deg; the lowest charge at the top
  elevation within `min_range_m` + 10 %; the highest at 45 deg reaching
  `range_m`; only on a mortar with `[he]`). Member trait `observer`
  (bit 16). Rules `[fire_mission]` (`FireMissionTuning`, 25 keys) and
  `[ai.mortar]` (`AiMortarTuning`; `ai.barrage_rounds` still loads, both
  set is an error). `Capability::Barrage` → `FireMission` (TOML
  `fire_mission`, derived). GrW 34: charges 75/105/130/152/174 m/s, 45-87
  deg. M1: zones 70-188 m/s (estimate, S5b-2), 45-85 deg, `min_range_m` 60
  → 90, `rounds_per_min` 15 → 18.
- **Charge solver (A2)**, `crates/sim/src/indirect.rs`: the lowest charge
  with a high-angle root inside the limits, i128 integer math on Q16.16,
  half a substep of gravity added to `vz` so the sim's semi-implicit
  integrator passes through the solved arc. Tests: within 0.5 % of an f64
  reference (flight time and angle) at 60-600 m every 10 m, h = -20/0/+20
  m, both mortars; a bomb flown by the sim's own integrator lands within
  1 m of its aim point at 100/300/600 m and ±20 m; the GrW 34 fires
  charge I at 60-560 m and charge II at 600 m (T10). Legacy weapons keep
  the S1 rule.
- **FireMission (A3)**: `CommandKind::FireMission` (same variant index and
  fields as Barrage), `Order::FireMission { target, rounds, m: Mission }`,
  `OrderKind::FireMission` = 5, `TaskKind::FireMission` = 4,
  `Engage::Never`, refusals as `FireMissionEvent` in the snapshot
  (`not_mortar`, `rounds`, `off_map`, `inside_minimum`, `cannot_reach`,
  `unknown_ground`, `no_bombs`), detached men not regrouped (the old
  regroup only for legacy rules or when nobody in the formation carries
  the tube). Bridge intent `fire_mission` with `barrage` as an alias;
  the HUD still says "barrage" until Part B.
- **Modes, aim error, phases, observation, ranging memory (A4)**,
  `crates/sim/src/fire_mission.rs` and `combat.rs` `fire_mission_tick`:
  as the spec. Review fixes on the WIP: fire for effect on the error alone
  only after an observer saw a round land (an unseen adjusting round at
  the 10 m floor no longer counts as "within `ffe_error_m`"); the start
  error from the gunner's range, not the squad anchor's; an observer lost
  or gained while laying counts as a downgrade / upgrade.
- **Intel and the knowledge rule (A5)**: `PlayerState::intel` (sparse
  `BTreeMap` of 25 m cells), updated every second in the Status stage,
  expiry 300 s (trench cells: the match); `known_point` (own observer
  sees it, capture point, an enemy seen now within 25 m, an intel cell
  within 25 m).
- **5-man squads (A6)**: Granatwerfergruppe and 81mm Mortar Squad, ids
  kept (S5b-7), leader + gunner + assistant + 2 ammunition bearers,
  20 bombs (gunner 2 ready at the tube, assistant 4, bearers 7 each; the
  spec's S5b-5 (A) said 0/4/8/8, same total), 240 / 300 MP.
- **Saves (A7)**: `SAVE_VERSION` 13 → 14, `PROTOCOL_VERSION` 7 → 8 (relay
  doc updated: version 8 entry, `FireMission` in the variant list).
  Hashed and saved: every `Mission` field, `Squad::ranging`,
  `PlayerState::{intel, fm}`, `Projectile::mission` (hashed only when
  non-zero, so other rounds hash as before), `MatchState::next_mission`.
  Load checks: mission mode/phase/id/ticks/adjust/charge/offset/observer,
  ranging cells (count, ticks, on map, offset size, mortar squads only),
  intel keys/ticks/flags, projectile mission ids below `next_mission`.
  `S5b <faction>` summary lines (missions and bombs by mode, refusals,
  planned miss by mode, adjusting rounds, bursts seen/unseen, upgrades,
  downgrades, no solution, flight, bombs fired from inside the own base).
- **`mortar-bench` (A8)**, `sdk/crates/cli/src/mortar_bench.rs`: flat
  Kessel Ridge, the mortar 250 m from the west edge, a ground target
  200/300/400/500 m east; per range, seed and faction three runs of the
  same mission: observed (the leader walked out 120 m forward, 30 m to the
  side, inside the voice link), map fire (observe ranges 0, the target
  cell marked), observed with the bursts hidden by smoke laid after the
  order. Misses from the burst log (`World::enable_burst_log`).

### Tests

- New `crates/sim/tests/s5b_fire_missions.rs` (7): Direct mission ranges
  in on seen bursts (10 m → 5 → 2.5 ..., one adjusting round, charge I,
  14.6 s flight, the burst a flight time after the shot, the ranging
  memory reused by a later mission with no adjusting round); hidden bursts
  teach nothing (smoke after the adjusting round: error unchanged, a
  downgrade, nothing remembered); unknown ground refused, known by a
  trench cell (kept after 400 s), a capture point, a last-seen cell (and
  refused again after `intel_keep_s`), map fire at 27 m at 300 m;
  refusals (minimum, off map, rounds, not a mortar, no bombs); the
  detached leader spots (Observed, stays detached) and beyond the 150 m
  voice link the mission is map fire; determinism at 1/2/3/8 threads and
  save → load → continue mid-adjust (round in the air) and mid-effect with
  a spotter out; legacy rules fire as the old barrage.
- `world::tests::s5b_fire_mission_state_is_saved_hashed_and_checked`:
  each of 41 fields changes the hash and survives save/load; 19 broken
  values refused on load.
- Updated: S5a T7 tests (20 bombs, the bearers carry 14, all fired, all
  back in base within 30 s at 20 × the shells price); content tests (cost
  and men of the mortar cards, 20 bombs); `setup_bytes_baseline` and the
  canonical snapshot re-blessed (old values in the comment);
  `ai_vs_ai_matches_end_with_a_winner` cap 30 → 40 min (seed 2 now ends at
  32:26; 40 is the slices' match-length bound).

### Results (Part A; the AI is unchanged except that it now orders FireMission)

Each run alone, release build, `--jobs 3 --minutes 45` for skirmish.

| # | Target | Baseline (`3cd5717`) | Part A | Verdict |
|---|---|---|---|---|
| T1 | same mission, map fire vs observed: pooled mean-miss ratio 2.5-3.5; each range 2.0-4.5 (report) | — | **3.04** (observed 10.6 m, map 32.4 m; 200 m 4.08, 300 m 3.07, 400 m 2.80, 500 m 2.75), 50 seeds × both mortars, 8 rounds | met |
| T2 | ranging only on seen bursts | — | bench: every observed run ranged by 50 % per seen burst, no map or hidden-burst run moved (1,200 runs); sim tests | met |
| T3 | no resupply: stops after the carried bombs, bearers included (20) | 15 (3-man) | 20 fired, all carried bombs gone, order ends; back in base within 30 s at 20 × the shells price | met |
| T4 | no fire into ground never seen; AI refusals 0 | — | unknown ground refused; trench cell, capture point, last-seen cell (until expiry), observed point accepted (tests); AI refusals **0** in the 40 set | met |
| T5 | each faction 35-65 % on 40; 45-55 % on both 160 sets | 50.0 % / 43.8 % / 51.9 % | 40 set: Germany 22 (**55.0 %**), USA 18; the 160 sets are Part B (B4) | 40 met; 160 not run |
| T6 | no base side > 62 % | 50.0 % | base 0 16, base 1 24 (**60.0 %**) | met (close) |
| T7 | no match < 10 or > 40 min; median 15-30 | 10.4-38.5, median 16.5 | **10.5-32.4**, median **17.8** (mean 18.2) | met |
| T8 | HE share of kills mean ≤ 26.3 % | 26.9 % (max 37.2 %) | **11.8 %** (max 19.0 %) | met, for the wrong reason (below) |
| T9 | report bombs, MP on ammo, base battery | bombs 7,327 / 6,905; MP 673.7 / 554.8 per match | bombs **6,015 / 5,296** (-18 % / -23 %); MP **555.1 / 440.7** per match; bombs from inside the own base **36.5 % / 39.3 %** (almost all map fire); A/B below | reported |
| T10 | solver within 0.5 % of f64; bomb lands within 1 m; GrW charge I to 560 m, II beyond | 60° rule | unit tests pass; in the sim 14.6 s at 300 m, charge I | met |
| T11 | AI: Observed + Direct ≥ 70 % of missions; ≥ 1 spotter per match per side | — | **7.4 % / 7.6 %** observed+direct; 0 spotters | missed (Part B: the AI is not updated) |
| T12 | determinism, saves, loopback, soaks, per-field test | pass | 1/2/3/8 threads and save/load mid-adjust and mid-effect equal; per-field test; net loopback and both soaks pass | met |
| T13 | fortify PASS; armour hard checks at `--seeds 5` and `--stress --seeds 10`; golden; validate; clippy; fmt | pass | fortify **PASS, mortar row 45 vs 114 = 39.5 %** (limit 40 %); armour all hard checks (221.6 s; stress 443.7 s); golden, validate, clippy, fmt clean | met (fortify marginal) |
| T14 | tick p99 peak < 1.5 ms (`--jobs 3`); stress case < 10 ms | 0.363 ms; 1.502 ms | **0.387 ms**; stress **1.521 ms** | met |
| T15 | Godot checks | 251 / 143 / 17 | bridge rebuilt: `check_project` OK, `test_match_view` 251/251, `test_net_ui` 143/143, `test_data_reload` 17/17; `settings.cfg` unchanged (sha1 `cf03883`); the new HUD is Part B | existing pass |
| T16 | every existing test passes | 521 | **532 pass, 0 fail** (`--include-ignored`: both net soaks and the vehicle stress report; +11: 3 solver unit tests, 7 in `s5b_fire_missions.rs`, the per-field test) | met |

**The mortar lost almost all its killing power in AI play.** Mortar kills
as a share of each side's kills: Germany **3.3 %** (baseline 21.8 %), USA
**2.5 %** (18.2 %). The AI still sends its mortars to the S4 support spot
behind the front and fires at spotted groups with no spotter, so 93 % of
its missions are map fire (Germany 1,070 of 1,155, USA 930 of 1,006):
mean planned miss 43 m against the 15 m fragment reach. Observed missions
miss 22-26 m on average (many are map missions upgraded mid-way that range
from the map offset), Direct 8-12 m. The HE-share target is met because
the mortar barely kills, not by design; T8 and the balance must be judged
again after Part B gives the AI spotters (B1). Mean flight 15.7 / 16.9 s.
Adjusting rounds 177 / 120; bursts seen 119 / 87, unseen 80 / 50;
upgrades 92 / 54, downgrades 112 / 66; no-solution ends 0.

**Base battery (owner 2026-11-03: measure, do not brake).** 36.5 %
(Germany) and 39.3 % (USA) of all bombs left a tube standing in its own
base, nearly all as map fire. A/B on the 40 set with
`base_bomb_after_fire_s` 0 → 120 (`s5b/partA2/skirmish40_bb120.txt`):
Germany 22 → 16 wins, base 0/1 16/24 → 18/22, bombs 6,015 / 5,296 →
4,046 / 3,311, MP on ammo per match 555.1 / 440.7 → 339.6 / 263.6, HE
share 11.8 % → 11.6 %, mortar kills 3.3 / 2.5 % → 2.1 / 2.2 %, length
10.5-27.2 min. The brake still moves Germany by 6 wins of 40 (about two
standard errors) even with the mortar this weak; the lever stays 0 (S6).
The 160-set A/B is Part B (T9).

**fortify-test diagnosis** (`s5b/fort/`, 10 seeds, 20 runs per cell). The
mortar row is now the spec's Direct mission adjusting on the target:
trench 45 vs open 114 = 39.5 % (baseline 46 vs 136 = 33.8 %). Reported
beside it, the same mission after ranging on a reference point 40 m off
(fire for effect without a warning round): 52 vs 117 = 44.4 %. With the
first-round error set to 0 (`--rules-override`) the ranged variant gives
42 vs 132 = 31.8 % and the adjusting one 65 vs 134 = 48.5 %; with the
M1 back at 15 rounds a minute 44 vs 120 = 36.7 % (adjusting). So the
open-ground losses fall with the aim offset that stays after one
adjusting round (5 m at 250 m), the trench losses stay where they were,
and the ratio moves toward the limit; with 2-5 losses per run the ratio
carries about ±6 points of noise. The WIP had made the reference-ranged
variant the pass row; Part A went back to the spec (S5b-13).

**Not done in Part A:** the spec's A2 "flight-time change alone" 40-set
measurement (the solver and the missions landed together in the WIP); the
literal "legacy hashes unchanged" check (every save hashes the new fields
now; legacy behaviour is covered by the unchanged S3c-fixture combat tests,
which pass, and the legacy test above); Godot HUD, AI missions and the 160
sets (Part B).

### What Part B must know

- The AI fires 93 % map fire from its support spots: T11 (≥ 70 %
  observed+direct) needs the spotter logic (B1). Expect the mortar's kill
  share, the HE share (T8) and the balance (T5, T6) to move a lot once
  spotters work; re-measure everything then.
- Observation stops when the mission's last round leaves the tube (the
  order ends then): with 14-16 s of flight and a 4 s interval about half
  of an 8-round mission's bursts land after the order ended, are logged
  `live = false`, are not counted seen or unseen and do not update the
  ranging memory (S5b-14). Corrections only matter for rounds not yet
  fired, so the mission's accuracy is unaffected.
- A mission whose observer is lost while laying fires as map fire on the
  observed start error (the observer passed the data; counted as a
  downgrade, S5b-15).
- The HUD still names the order `barrage` (`OrderKind::FireMission` →
  "barrage" in `match_node.rs`); the bridge accepts `fire_mission` and
  `barrage`. `fire_preview`, mission fields and refusal events in the
  frame are Part B; `RenderSnapshot::fire_mission_refusals` is filled.
- `FmCounters::spotters` exists (always 0 so far); the spec's
  `AiState::spotters` map does not exist yet: Part B adds it with its own
  save, hash and load checks.
- `mortar-bench` takes `--rules-override`; a full 50-seed run is 90 s at
  `--jobs 3`.

## Part B: AI missions and spotters, HUD tool, Godot tests, measure

Commits `b15dc4d` and `e30bbff` on `s5b-mortars` (plus this log). No
version bump: `SAVE_VERSION` 14 and `PROTOCOL_VERSION` 8 from Part A cover
the new AI state (the AI state is saved and hashed whole, through
postcard); no command changed.

### What was built (B1-B3)

- **AI fire missions (B1, `crates/sim/src/ai/mortar.rs`)**, the spec's six
  steps with these additions (S5b-20): targets score a man standing still
  10 and a moving man 5 (the bombs land 20-30 s after the order), plus
  remembered trench cells (30 + 10 per manned neighbour cell); map fire is
  held while the mortar walks to its support spot; a set-up tube stays
  while the front point is in its band; at most 48 sight-line rays a plan
  over all mortars (`PLAN_RAYS`, turn order rotating with the plan count);
  orders only at points the knowledge rule's lasting part accepts
  (`MatchState::known_by_record`: a capture point or an intel cell; an
  enemy seen only this instant may be gone a reaction delay later) and no
  mission below 2 bombs. Spotters: the mortar's own leader (a living
  `leader` slot man, not the gunner) walks to the first of 16 spots with a
  sight line from a standing eye (S5b-19: automatic stance); records in
  `AiState::spotters` (saved, hashed, validated: count, squad ids, man id,
  tick order). `FmCounters::spotters` (always 0 in Part A) is gone; the AI
  counts in `AiCounters::fm_*` and the summary prints an `S5b <faction>
  AI:` line. Legacy rules keep the old planner unchanged
  (`mortars_legacy`).
- **Data (B4 levers, AI data first):** `[ai.infantry] support_back_m`
  200 -> 120 (S5b-16), `[ai.mortar] spot_samples` 8 -> 16 (now the ray
  budget of one squad's spot search), comments on `spot_max_s` and
  `map_fire_min_score`. Canonical content snapshot re-blessed (those two
  values).
- **Bridge and HUD (B2):** `MatchState::fire_preview` (read-only: the
  order's refusal check and mode check, the start error or the remembered
  one, the ring = error + half the precision spread's extent); the bridge
  asks it on the sim thread between ticks (`PreviewShared`, republished
  once per request while paused) and sends `fire_preview` and `intel`;
  own squads carry `mission`; refused orders become `fire_mission_refused`
  events; intent `fire_mission {squads, target, rounds}` (`barrage` kept).
  HUD: the tool's cursor label (mode, expected miss, rounds, or the
  refusal in red), Shift+wheel rounds, the ring (solid / dashed / red
  cross), the knowledge map, the observer link line, the mortar card's
  phase and rounds in the header and its mission strip (DIR / OBS / MAP
  tag, "Ranging 2 · ±3 m · 15.1 s", ranging bar). The grid button reads
  **Mission** (S5b-18). Mock source and screenshot stagings
  `s5b_mortars` / `s5b_card`; `godot/docs/match_state.md` "S5b".

### Tests

- `crates/sim/tests/s5b_ai_mortar.rs` (4): the AI sends its leader forward
  and the mission is Observed through him (inside the voice link, no map
  fire before he went, refusals 0); with no spotter possible it falls back
  to map fire above `map_fire_min_pct`; it holds map fire below it, and a
  mortar that knows of no enemy fires at nothing for two minutes;
  determinism with a spotter out at 1/2/3/8 threads and through save ->
  load -> continue.
- `world::tests::s5b_ai_spotter_state_is_saved_hashed_and_checked`: 17
  fields change the hash and survive save/load; 7 broken records refused.
- `ai_vs_ai_plays_by_the_rules` asserts no AI fire mission is refused.
- Bridge: `fire_missions_previews_and_refusals_reach_the_frame` (preview
  only while asked, unknown ground refused, a capture point accepted, the
  enemy's squad never previewed, own mission view, refusal event).
- Godot: `test_match_view` +24 checks (tool label per mode, ring sizes,
  observer, Shift+wheel rounds, intent `fire_mission` with rounds, mission
  view, ranging, strip line and status, tags, refusals for unknown ground
  and too close, feed line, preview cleared with the tool),
  `test_data_reload` +4 (a charge edit checks clean; a charge table that
  does not rise is refused with the fix; banner, match goes on).

### Results (final binary; each run alone, `--jobs 3 --minutes 45`)

| # | Target | Baseline (`3cd5717`) | Part B | Verdict |
|---|---|---|---|---|
| T1 | map/observed miss ratio 2.5-3.5 | — | 3.04 (unchanged from Part A; `mortar-bench --seeds 50`) | met |
| T2 | ranging only on seen bursts | — | bench PASS; sim tests | met |
| T3 | stops after carried bombs, bearers included | 15 | 20 (tests) | met |
| T4 | no fire into unknown ground; AI refusals 0 | — | unknown-ground refusals 0 in all three final sets; **1 `no_bombs` refusal** (Germany, a seed in 1-20, so in the 40 and the 160 set) | missed by 1 (S5b-22) |
| T5 | faction 35-65 % (40); 45-55 % (160 and out of sample) | 50.0 / 43.8 / 51.9 % | Germany **40.0 %** (16/40), **47.5 %** (76/160), **40.6 %** (65/160 out of sample) | 40 and 160 met; **out of sample missed** (S5b-21) |
| T6 | no base side > 62 % | 50.0 / 52.5 / 50.6 % | base 1: 60.0 / 53.8 / 60.6 % | met (40 and OOS close) |
| T7 | no match < 10 or > 40 min; median 15-30 | 10.4-38.5, median 16.5 | 11.9-30.6 (median 17.1) / 10.2-33.7 (17.3) / 10.8-31.5 (18.4) | met |
| T8 | HE share of kills mean <= 26.3 % | 26.9 / 28.3 / 28.5 % | **13.3 / 13.9 / 13.7 %** (max 20.9 / 28.2 / 24.5) | met |
| T9 | bombs, MP on ammo, base battery | bombs 180.2 / 160.7 per match (160); MP 651.5 / 519.4 | below | reported |
| T10 | charge solver | — | unchanged | met |
| T11 | observed+direct >= 70 % of AI missions; >= 1 spotter per match per side | Part A 7.4 / 7.6 % | sim-counted 74.9 / 76.3 % (40), 75.6 / 78.3 % (160), 76.4 / 76.5 % (OOS); spotters 1.80 / 1.82, 1.75 / 1.76, 1.77 / 1.46 per match | met |
| T12 | determinism, saves, loopback, soaks, per-field | pass | all pass (AI with a spotter out at 1/2/3/8 threads and save/load; per-field test for the AI state) | met |
| T13 | fortify PASS; armour hard checks at 5 and stress 10; golden; validate; clippy; fmt | pass | all pass; fortify mortar row 39.5 % (limit 40 %) | met (fortify marginal, as Part A) |
| T14 | tick p99 peak < 1.5 ms; stress case < 10 ms | 0.363 / 1.110 / 1.113; 1.502 | **0.369 / 0.404 / 0.477 ms**; stress case 14 p99 1.516 ms | met (the first Part B binary had one 1.770 ms peak on the out-of-sample set; the 48-ray plan budget came after it) |
| T15 | Godot checks, screenshots | 251 / 143 / 17 | `check_project` OK, `test_match_view` 275/275, `test_net_ui` 143/143, `test_data_reload` 21/21; `settings.cfg` sha1 `cf03883` before and after; screenshots at 1600x900 and 1280x800 | met |
| T16 | every test passes | 521 | **538 pass, 0 fail** (`--include-ignored`) | met |

**T9 detail** (160 set, seeds 1-80 × both bases; Germany / USA):

- Bombs fired: 11,015 / 9,002 (68.8 / 56.3 per match; S5a 180.2 / 160.7);
  by the round's mode direct 2,050 / 1,790, observed 3,860 / 3,533, map
  5,105 / 3,679. Missions 2,378 / 2,024 (14.9 / 12.7 per match). Mean
  planned miss: direct 10.0 / 9.6 m, observed 11.9 / 11.1 m, map 25.5 /
  24.9 m. Many map bombs come from missions ordered observed whose
  observer walked off or lost sight (downgrades, S5b-15), not from map
  missions.
- MP on ammunition per match: 244.9 / 186.9 (S5a 651.5 / 519.4), almost
  all shells.
- Bombs fired from inside the own base: 928 (8.4 %) / 540 (6.0 %), Part A
  36.5 / 39.3 % (40 set): map fire on the way out is held now.
- Mortar share of each side's kills: 5.3 / 4.6 % (S5a 23.3 / 18.8 %);
  S5b-17.
- **Base battery A/B** (`base_bomb_after_fire_s` 0 -> 120, 160 set, final
  binary): Germany 76 -> 69 wins, base 0/1 74/86 -> 77/83, bombs 11,015 /
  9,002 -> 9,756 / 8,445 (68.8 / 56.3 -> 61.0 / 52.8 per match), bombs
  from the own base 928 / 540 -> 689 / 395, MP on ammo 244.9 / 186.9 ->
  205.9 / 170.8 per match, HE share 13.9 -> 14.1 %, lengths 10.2-34.5
  (median 17.2). On the first Part B binary the same A/B gave Germany 84
  -> 82. The brake takes about a tenth of the bombs and moves Germany by
  2-7 wins of 160 (within noise); the lever stays 0 (S6).

**Balance.** Final binary: Germany 47.5 % on seeds 1-80 but 40.6 % on
101-180 (base 1 60.6 % there); over the 320 matches 44.1 % (S5a final:
48.1 %). The out-of-sample miss is about 1 standard error below the band (one
set of 160 has a standard error of about 4 points), but the same code change
that only re-orders the AI's random draws moved that set from 56 to 68
German wins and back to 65, so the two sets disagree by more than their
own noise: Germany is probably about 44-46 % now, a little worse than in
S5a, and below the band on one set. One lever was tried on the
out-of-sample set: `map_fire_min_pct` 60 -> 85 (less German map fire):
68 -> 69 wins, no effect, not kept. Prices were not touched (the owner
keeps the 20 % German cut). S5b-21.

**Hard checks** (final binary). fortify-test `--seeds 10` PASS, mortar row
45 vs 114 = 39.5 % (Part A 39.5 %, limit 40 %); armour-test `--seeds 5`
all hard checks pass in 220.1 s (case 9: 23 vs 84 men = 27.4 %);
`--stress --seeds 10` all pass in 442.1 s (case 9 46 vs 164 = 28.0 %;
case 14 p99 1.516 ms, max 2.709); mortar-bench 3.04 PASS (91.6 s); golden
ballistics 46/46; `validate mods/ww2` ok; clippy `-D warnings` and fmt
clean.

**Screenshots** (scratchpad `s5b/`): `s5b_mortars_1600x900.png`,
`s5b_mortars_1280x800.png` (the tool on an enemy squad: "FIRE MISSION ·
Observed · ±24 m · 6 rounds · Shift+wheel rounds", the ring, the
knowledge map squares, the dashed link), `s5b_card_1600x900.png`,
`s5b_card_1280x800.png` (a mortar ranging in: header "For effect · 6
rds", strip "OBS Ranging 2 · ±3 m · 15.1 s", the ranging bar about 85 %).

### What the Fix stage must know

- T5 out of sample (40.6 %) and the single `no_bombs` AI refusal are the
  open misses (S5b-21, S5b-22). Mortars kill little now (S5b-17).
- Raw outputs: scratchpad `s5b/partB/` (`f_40.txt`, `f_160.txt`,
  `f_oos.txt`, `f_160_bb120.txt`, `f_fortify10.txt`, `f_bench50.txt`,
  `f_armour5.txt`, `f_armour_stress10.txt` final; `m_*` the first Part B
  binary;
  `r_*` the ray-budget step and the `map_fire_min_pct` 85 A/B;
  `test_final.txt`, Godot outputs).
- The HUD's preview runs `fire_preview` on the sim thread when a frame is
  published; it never touches the tick, but it does cost up to 4 + 8 rays
  per frame while the tool is armed.

## Tune (one round, no data changed)

See `docs/slices/S5b-mortars.md` "Tune results" for the numbers. In short:
the Part B binary and data were re-checked (the 40 set reproduces all 40
hashes; fortify-test, armour-test `--seeds 5` and `--stress --seeds 10`
pass, case 14 p99 1.647 ms), four A/B probes on a fresh set (seeds
201-280) and on all three 160 sets showed that no mortar lever and no 9 %
infantry price cut moves the German share (pooled 45.4 % on 480; set spread
4.7 points), and the one `no_bombs` AI refusal is a crew-wipe race (seed 8,
tick 12,420, squad with one man left). T4 and T5 (out of sample) stay open
as S5b-22 (A) and the new S5b-23. Raw outputs: scratchpad `s5b/tune/`
(`x_*` probes, `r1_*` re-checks, `seeds/` per-seed 40-set runs).

## Fix and close

Commits `f18b49c` (code, HUD, tests, TODO) and the close commit with this
log; branch pushed to `origin/s5b-mortars`. The review (read-only,
`3cd5717..2fa50be`) found no high or medium findings; it confirmed four
lows and one observation. All five were acted on:

| Finding | Fix | Effect on hashes |
|---|---|---|
| **L1** a save with a modded long-flight mortar (e.g. charges `[100, 300]`, elevation `[45, 89]`, flights to 61 s) was refused on load: `mission_ok` capped the burst deadline at a fixed 60 s | `indirect::flight_bound_s`: `2 v / g` of the highest charge + 20 s for a target up to 1,000 m below the muzzle; the load check allows that + 4 s, at least the old 60 s. For the ww2 tables the window is 60 s (GrW 34: highest charge 174 m/s, 36 + 20 + 4 s) and 63 s (M1, 188 m/s); their real flights stay under about 21 s. Unit test `flight_bound_covers_long_modded_tables` (every solved flight of the review's table, 40-5,000 m, drops to 200 m, is under the bound; the table does fly past 60 s). `docs/modding/fire-missions.md` says so | none (load check only) |
| **L2** a mission ordered beyond range drew its start error at the order-time range although the team walks to 90 % of `range_max` and fires from there (800 m: map 64 m instead of 43 m) | `order_fire_mission` and `fire_preview` clamp the range to 90 % of `range_max` when walking (the HUD ring and its precision spread too). Test `a_mission_beyond_range_starts_with_the_error_of_its_firing_range` (map fire at 800 m: 43.2 m) | none in AI play (the AI orders inside range); the 40, 160 and out-of-sample hashes are unchanged |
| **L3** mission accuracy comes mostly from having an observer at order time; T11 counts missions by their order-time mode | metric only: the skirmish summary's bombs line now prints the observed+direct share **by bomb** beside the per-mission share (40 set: 52.8 % / 57.2 % by bomb, 74.9 % / 76.3 % by mission). The rule itself is an owner call: S5b-24 (A) keep S5b-15 (A) | none |
| **L4** the observer link line was about 1 px and faint at the 330 m camera | `ground_overlay.gd`: link and preview lines are at least 3 px (gunner → observer, with a 1 px dark outline) or 2 px (the thin legs) on screen, from the camera's metres per pixel (`_m_per_px`), dashes scaled the same way, alpha 0.75-0.95 | presenter only |
| **L5** the S5b screenshots used a stale 3-man, 200 MP, 15-round mock mortar | mock: "Granatwerfergruppe", 5 men, 240 MP, bombs by slot 0/2/4/7/7 (leader with his MP 40, gunner, assistant, two bearers), one bomb taken per round from the gunner first, the mission ends when none is left; fallback man names Leader / Gunner / Assistant / Bearer. `test_match_view`'s S4 call-in checks free 4 pop first (4 engineers, unused afterwards; the bigger mortar filled the cap and the heavy-MG card read "pop cap"); the check now prints the blocking reason | presenter only |

Rejected: none. Not done: S5b-15 (B) (re-draw the error on a downgrade)
stays an owner question (S5b-24); the review's 1280x800 real-bridge card
check could not be forced (S5b-25).

### Final measurements (this binary; each CLI run alone except where noted)

- `cargo fmt --check` clean; `cargo clippy --workspace --all-targets
  --release -- -D warnings` clean; `cargo test --workspace --release --
  --include-ignored`: **540 pass, 0 fail** (+2: the L1 and L2 tests).
- `ballistics --golden mods/ww2/tests/ballistics.toml` 46/46;
  `validate mods/ww2` ok.
- skirmish 40 (`--seeds 20 --both-bases --jobs 3 --minutes 45`): **all 40
  hashes equal** the review's and Part B's; Germany 16 (40.0 %), base 1 24
  (60.0 %), length 11.9-30.6 min, median 17.1, HE share 13.3 % (max 20.9),
  tick p99 peak **0.531 ms**; refusals: 1 `no_bombs` (Germany, the S5b-22
  race), 0 unknown ground.
- skirmish 160 (seeds 1-80) and out of sample (101-180, `--seed 101`):
  re-run on this binary, **all 160 + 160 hashes equal** Part B's `f_160` /
  `f_oos`. Germany 76 (47.5 %) / 65 (40.6 %); base 1 86 / 97; length
  10.2-33.7 / 10.8-31.5 min; HE share 13.9 / 13.7 %; tick p99 peak 0.393 /
  0.439 ms; observed+direct by bomb 53.7 / 59.1 % and 52.8 / 56.2 %
  (Germany / USA).
- `mortar-bench --seeds 50`: PASS, map/observed **3.04** (observed 10.6 m,
  map 32.4 m, hidden bursts 14.7 m); ranging PASS.
- `fortify-test --seeds 10`: PASS, output identical to the review's;
  mortar row 45 vs 114 = 39.5 % (limit 40 %).
- `armour-test --seeds 5`: all hard checks pass (255.3 s), output
  identical to the review's (case 9: 23 vs 84 men = 27.4 %).
- `armour-test --stress --seeds 10`: all hard checks pass (454.4 s; case 9
  46 vs 164 = 28.0 %; case 14 p99 **1.640 ms**, max 2.776, limit 10 ms;
  the Godot headless tests ran beside it for part of the time).
- Godot (bridge rebuilt with `tools/build_bridge.sh`): `check_project` OK,
  `test_match_view` **275/275**, `test_net_ui` **143/143**,
  `test_data_reload` **21/21**. `settings.cfg` sha1 `cf03883` before and
  after every Godot run (equal to the backup).
- Mock stagings re-shot at 1600x900 and 1280x800 (scratchpad
  `s5b/fix/shots/`): the 5-man card with 192 / 2 / 4 / 7 / 7 rounds, 240
  MP tile, "OBS Ranging 2 · ±3 m · 15.1 s" strip and the link line as a
  clear 2-3 px dashed trace; the card fits at both sizes.
- `--autoplay 10` at 4x vs the AI (real bridge, 1600x900): plays to the
  time limit (lost on surrender at 10:00, as scripted); the battle shot
  shows the "Granatwerfer 5/5" tile and the 240 MP card, the 1280x800 HUD
  shot the "Granatwerfer · Fire mission · 5/5" tile; the mortar fired at an
  enemy trench from 320 m. Exit code 3 only because `s1b_frag_tool` and
  `s1b_frag_trench` were not taken (they need the default 18-minute run;
  not an S5b problem).

### Final targets

| # | Target | Baseline (`3cd5717`) | Final | Verdict |
|---|---|---|---|---|
| T1 | map/observed miss ratio 2.5-3.5 (bench) | — | **3.04** (observed 10.6 m, map 32.4 m) | met |
| T2 | ranging only on seen bursts | — | bench PASS; sim tests (hidden bursts teach nothing) | met |
| T3 | no resupply: stops after carried bombs, bearers included | 15 (3-man) | 20 (2 + 4 + 7 + 7), tests | met |
| T4 | no fire into unknown ground; AI refusals 0 | — | unknown ground refused (tests), 0 such AI refusals; **1 `no_bombs` AI refusal** per 40 / 160 (a crew-wipe race, S5b-22) | met for unknown ground; missed by 1 race refusal |
| T5 | each faction 35-65 % (40); 45-55 % (160 and out of sample) | 50.0 / 43.8 / 51.9 % | Germany **40.0 %** (40), **47.5 %** (160), **40.6 %** (out of sample); pooled 480 (Tune) 45.4 % | 40 and 160 met; **out of sample missed** (S5b-21, S5b-23) |
| T6 | no base side > 62 % | 50.0 / 52.5 / 50.6 % | base 1 60.0 / 53.8 / 60.6 % | met |
| T7 | no match < 10 or > 40 min; median 15-30 | 10.4-38.5, median 16.5 | 11.9-30.6 (median 17.1) / 10.2-33.7 (17.3) / 10.8-31.5 (18.4) | met |
| T8 | HE share of kills falls by >= 1/3 from S3b's 39.5 % (<= 26.3 %) | 26.9 / 28.3 / 28.5 % | **13.3 / 13.9 / 13.7 %** | met |
| T9 | bombs and MP on ammo per kind vs S5a; base battery measured | S5a bombs 180.2 / 160.7 per match (160); MP 651.5 / 519.4 | bombs 68.8 / 56.3 per match (160); MP on ammo 244.9 / 186.9; bombs from inside the own base 8.4 / 6.0 % (S5a 36.5 / 39.3 %); `base_bomb_after_fire_s` 120 A/B: -7 German wins of 160, within noise; lever stays 0 (S6) | reported |
| T10 | charge solver: true flight, high angle | 60° rule | solver within 0.5 % of f64, bombs land within 1 m; GrW 34 charge I to 560 m, 13.5-15 s | met |
| T11 | AI observed+direct >= 70 % of missions; >= 1 spotter per match per side | Part A 7.4 / 7.6 % | 74.9 / 76.3 % of missions (40); **by bomb 52.8 / 57.2 %** (new line, S5b-24); spotters 1.80 / 1.82 per match | met as defined (per mission) |
| T12 | determinism 1/2/3/8 threads and save/load; per-field tests | pass | all pass; L1 widens only the load check | met |
| T13 | fortify PASS; armour hard checks at `--seeds 5` and `--stress --seeds 10`; golden; validate; clippy; fmt | pass | all pass; fortify mortar row 39.5 % (limit 40 %) | met (fortify marginal) |
| T14 | tick p99 peak < 1.5 ms (`--jobs 3`); stress < 10 ms | 0.363 ms; 1.502 ms | **0.531 ms**; stress 1.640 ms | met |
| T15 | Godot checks and screenshots | 251 / 143 / 17 | OK, 275 / 143 / 21; stagings re-shot at both sizes; `settings.cfg` unchanged | met |
| T16 | every test passes | 521 | **540** pass, 0 fail | met |

### Open owner questions (each proceeds on its (A))

- **S5b-21 / S5b-23** German share: (A) accept, judge on pooled 320+
  matches (480 pooled: 45.4 %).
- **S5b-22** the `no_bombs` race refusal: (A) accept.
- **S5b-24** accuracy from the observer at order time (review L3): (A) keep
  S5b-15 (A); the per-bomb share is reported.
- **S5b-25** the real-bridge mortar card at 1280x800: (A) check it in the
  S6 owner play test.
- Earlier ones still open with their (A): S5b-1..20 (see `docs/TODO.md`).

### Known gaps

- T5 out of sample (40.6 %) and the single T4 race refusal (above).
- Mortars kill little in AI play (5.5 / 4.4 % of kills; S5a 21.8 / 18.2 %):
  S5b-17. The trench vs open mortar row sits at 39.5 % against a 40 %
  limit (36.8 % on seeds 11-30, review).
- The base-parked mortar fires unlimited from its base (measured, not
  braked; 8-9 % of bombs, almost all weak map fire); the brake waits for
  S6 (`base_bomb_after_fire_s` 0).
- Radio links, registered fire and the S5b-15 (B) downgrade rule are not
  in S5b.
- The solid gunner → observer link can show small gaps where the ground
  rises between its 2 m segments (depth-tested overlay); readable, not
  fixed.

### What S6 must know

- The base-battery brake (`[supply] base_bomb_after_fire_s`, 0 now) is the
  S6 lever; the A/B (120 s) took about a tenth of the bombs and moved
  Germany by -7 wins of 160 (noise).
- Judge faction balance on pooled sets (320+); one 160 set has a spread
  of about 4.7 points (Tune). Base 1 is worth about 6 points to either
  side.
- Long-flight weapons (S11 guns, heavy mortars) load fine now (L1); the
  ww2 GrW 34 / M1 tables fly at most about 21 s.
- The skirmish summary has the per-bomb observed+direct share; use it
  with T11.
- `SAVE_VERSION` 14 and `PROTOCOL_VERSION` 8 (one bump each for S5b); the
  Fix stage changed no layout or command.
- Raw outputs: scratchpad `s5b/fix/` (`z_*.txt` CLI, `g_*.txt` Godot,
  `test.txt`, `shots/`, `auto/`, `autoplay.log`).
