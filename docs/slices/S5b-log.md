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
