# Slice S5b-mortars: fire missions, observers, ranging-in, map fire, charges

Branch `s5b-mortars` (from `s5a-ammo` at `3cd5717`). Roadmap:
`docs/design/roster/roster-expansion.md` §4.2 "S5: Ammunition" (the S5b
bullets: mortar `FireMission` replacing `Barrage`, the observer rule,
ranging-in, map fire only on known points at 3× scatter, true flight time
from charges, 5-man mortar squads) and "Owner decisions"; detail in
`artillery-on-map.md` §4.1 (fire modes, link, observers), §4.5 (charge
and elevation solver), §4.8 (flight time, fire-direction delay), §4.9 and
§4.10 (the base battery); `docs/slices/S5a-log.md` "What S5b must know".

**In scope:** `FireMission` (target point, round count, fire mode) in place
of `Barrage`; the modes Direct, Observed and Map fire; the `observe` rule
and the voice link; aim error with ranging-in and a per-mortar ranging
memory; the player's knowledge map (intel) that map fire needs; the
high-angle charge solver and true flight time; 5-man mortar squads with
bearers; AI missions through observers and spotters with map fire as the
fallback; the HUD fire-mission tool, observer link line and ranging
progress; the `mortar-bench` CLI; re-measuring the balance, the HE share
and the base battery.

**Not in scope:** radio links (S6 HQ radio box, S11 FO teams), registered
(unobserved) fire on a ranged cell (S11, for guns and mortars together,
S5b-4), counter-battery, friendly fire from indirect HE (Q22, ships in
S11a), crates and the end of base bomb refill (S6), the Panzerfaust AI
(T9, a later slice), smoke rounds (Q5 (A)).

**Owner decisions 2026-11-03 that bind this slice:** the base-parked mortar
keeps unlimited base refill until S6 (`base_bomb_after_fire_s` stays 0) but
S5b **measures** it against the observer rule; trenches stay strong against
small arms (S4-43 (A)); the 20 % German price cut stays (S5a-37 (A)).

---

## Baseline (measured 2026-11-03 on `3cd5717`, release build)

Raw outputs: scratchpad `s5b/base_skirmish40.txt`, `base_skirmish160.txt`,
`base_skirmish160_oos.txt`, `base_armour5.txt`, `base_armour_stress10.txt`,
`base_fortify10.txt`. Every skirmish headline below equals the S5a Fix
table (`S5a-log.md` "Final targets"), so this is the S5a final state.

### AI vs AI (`--jobs 3 --minutes 45`, each run alone)

| Measure | 40 (seeds 1-20 × both bases) | 160 (seeds 1-80 × both) | 160 out of sample (101-180 × both) |
|---|---|---|---|
| Germany wins | 20 (50.0 %) | 70 (43.8 %) | 83 (51.9 %) |
| Wins by base 0 / 1 | 20 / 20 (50.0 %) | 76 / 84 (52.5 %) | 79 / 81 (50.6 %) |
| Length (min) | 10.4-38.5, mean 18.4, median 16.5, 6 over 25 | 10.2-38.5, mean 18.3, median 17.4, 16 over 25 | 10.1-33.6, mean 18.4, median 17.6, 13 over 25 |
| HE share of kills | mean 26.9 %, max 37.2 % | mean 28.3 %, max 52.6 % | mean 28.5 %, max 43.9 % |
| Mortar kills (share of side's kills) G / US | 21.8 / 18.2 % | 23.3 / 18.8 % | 22.3 / 19.1 % |
| Tank HE kills G / US | 7.5 / 6.5 % | 8.1 / 6.9 % | 8.6 / 7.1 % |
| Mortar bombs fired G / US | 7,327 / 6,905 | 28,834 / 25,709 | 27,875 / 26,606 |
| Bombs refilled in base G / US | 7,465 / 6,774 | 28,999 / 25,307 | 28,024 / 26,740 |
| MP on ammo per match G / US (shells share) | 673.7 / 554.8 (99.4 / 99.7 %) | 651.5 / 519.4 | 631.0 / 548.8 |
| Mortar cards bought G / US | 91 / 96 | 363 / 369 | — |
| Mortar reserve holds | 0 / 0 | 0 / 0 | — |
| Tick p99 peak | 0.363 ms | 1.110 ms | 1.113 ms |
| Wall time | 61 s | 257 s | 294 s |

What the baseline says for S5b:

- **The HE target.** The roadmap's "HE share down by at least a third from
  the post-S3b baseline" is 39.5 % × 2/3 = **26.3 %**. The S5a final is
  26.9 % (40) and 28.3 / 28.5 % (160 sets): 0.6-2.2 points short. Mortars
  are 18-23 % of each side's kills, tank HE 6.5-8.6 %; S5b moves the mortar
  part.
- **Mortars fire about 340 bombs a match per side** (160 set), and every
  one of them is refilled in base: the base pays out as many bombs as are
  fired. 99 % of manpower spent on ammunition is shells (bombs and tank
  rounds). The bombs are fired at targets some own soldier sees today; no
  observer, link or aim error.
- **No counter exists today** for bombs fired from inside the own base
  (the base battery, S5a-42). Part A adds it, so the base battery is
  measured, not guessed (T9).

### armour-test `--seeds 5`

All hard checks pass in 223.3 s (as the S5a Fix: 222.1 s). Rows a mortar
change can move:

| Case | Result |
|---|---|
| 9 tank HE vs infantry | trench 23 vs open 84 = 27.4 % (limit 40 %) |
| 10 mortar vs a closed tank (20-round order) | no penetration; 0 tracks over 138 shells (the 3-man team holds 15, so 13.8 bombs a run) |
| 12 trench crossing (the cratered trench is shelled by a 10-round barrage) | crossable, 0 of 600 stuck |
| 16 Panzerfausts | 40 m still 8 fired / 8 hit / 8 pen; 55 m still 7 / 7 / 7; 35 m moving 5 / 5 / 5; 55 m moving 0 |

### armour-test `--stress --seeds 10`

All hard checks pass in 445.9 s (S5a Fix 481.2 s):

| Case | Result |
|---|---|
| 9 tank HE vs infantry | trench 46 vs open 164 = 28.0 % (limit 40 %) |
| 10 mortar vs a closed tank | no penetration; 1 track over 274 shells (0.004 per shell) |
| 12 trench crossing | 0 of 1,200 crossings stuck, no blocked node |
| 14 stress | p99 1.502 ms, max 2.795 ms (limit 10 ms); weapons stage p99 0.548 ms |
| 16 Panzerfausts | 40 m still 14 / 14 / 14 (disabled 7); 55 m still 15 / 14 / 14 (8); 35 m moving 9 / 9 / 9 (4); 55 m moving 0 |

### fortify-test `--seeds 10`

PASS (67 s), equal to the S5a Fix:

| Row | Result |
|---|---|
| Rifles: trench vs open at 150 m | 0 vs 167 (0.0 %, limit 50 %) |
| 10-round 81 mm barrage | trench 46 vs open 136 = **33.8 %** (limit 40 %); 90 s budget |
| Grenade assault from 25 m | defenders 189 lost with grenades vs 8 with rifles |
| Smoke control | hidden side never seen or fired on |
| MG-supported advance (T10, report) | defenders 6 of 220 (2.7 %), attackers 236 of 330 (71.5 %): missed as in S4 (owner call S4-43 (A): trenches stay strong) |

The mortar row becomes a Direct mission in S5b (the mortar's leader sees
the trench 250 m away): its first one or two bombs are adjusting rounds
with a 10 m aim error, so fewer of the ten bombs fall on the squad in
both the trench and the open runs; the ratio should hold, the absolute
losses fall. The budget goes to 180 s.

---

## What exists today (code map): the Barrage path end to end

Line numbers at `3cd5717`.

| # | Step | File:line | Today | S5b replacement |
|---|---|---|---|---|
| 1 | Command | `crates/sim/src/command.rs:51-57` `CommandKind::Barrage { squad, target, rounds }` | Mortar squads fire `rounds` (1..=50) at `target` | `CommandKind::FireMission { squad, target, rounds }` (same fields; the mode is decided by the sim, never sent); `Barrage` removed; `PROTOCOL_VERSION` 7 → 8 |
| 2 | Net limits | `crates/net/src/protocol.rs:554-593` `command_in_limits` (585) | Fixed-size command, always in limits | Same arm renamed; `sdk/docs/net/relay-protocol.md:390` |
| 3 | Order apply | `crates/sim/src/game.rs:861-865` dispatch, `1074-1110` `order_barrage` | Checks owner, map, `Capability::Barrage`, 1..=50 rounds, not inside `range_min`; regroups detached men (`clear_detached`, `unit.rs:550`), holds in place if in range, else paths to 90 % of range | `order_fire_mission`: the same checks plus the **knowledge rule** (refused with a reason event when no observer sees the point and it is not in the player's intel); detached men are **not** regrouped (the squad's own spotter stays out); mission state created |
| 4 | Order state | `game.rs:118-124` `Order::Barrage { target, rounds }`, `154` `OrderKind::Barrage`, `171`, vehicle guard `1656` | Target and rounds left | `Order::FireMission { target, rounds, m: Mission }` (`OrderKind::FireMission` = 5, same number) |
| 5 | Hash | `crates/sim/src/hash.rs:389-392` | target, rounds | target, rounds, every `Mission` field |
| 6 | Engage rule | `combat.rs:143` `Order::Barrage => Engage::Never` | Crews on a barrage never shoot small arms | Same for `FireMission` |
| 7 | Fire | `combat.rs:792-905` `fire_mortars` (Weapons stage, serial, ascending squad id) | Gunner = first non-detached man with an `[he]` weapon; holds; out of range walks to 90 % (`barrage_retry_m`); fires when cooldown and set-up are done | `fire_missions`: the same gunner, range and walk rules, then the **mission phases** (prep delay, adjust rounds one at a time, fire for effect) and the **mode per round** (Direct / Observed / Map) |
| 8 | Bomb count | `combat.rs:852-858` → `supply.rs:923-960` `take_bomb` | Gunner's tube, his pouches, then a squadmate within `crew_feed_m` (assistants, bearers first); none: the order ends | Unchanged; the 5-man squads' bearers feed through it and the 5 s sharing pass (T7 checks all 20 bombs are fired) |
| 9 | Aim and scatter | `combat.rs:859-866` | One spread: triangular per axis, half-width `spread_mrad × range`, × suppression factor, RNG `Purpose::SHELL_AIM` (gunner, tick) | **Two terms**: the mission's aim offset (systematic, `Purpose::MISSION_AIM`) plus the same precision spread on top (`SHELL_AIM`, unchanged) |
| 10 | Flight | `combat.rs:867-893` | Time `sqrt(range × flight_milli)` (≥ `min_flight_s`), velocity solved from that time; about 60° at every range; `[combat.mortar] flight_milli` 350, `min_flight_s` 4 | **Charge solver**: lowest charge of the weapon's `[indirect]` table with a high-angle solution inside its elevation limits; launch velocity from it, flight time follows (drag-free, which matches the sim: shells fly with `drag = 0`, `combat.rs:1439`) |
| 11 | Projectile | `combat.rs:1422-1450` `spawn_rounds`; `projectile.rs` | HE shells first, never dropped while the reserve holds | Unchanged; the projectile gains `mission: u32` (firing squad id + 1, 0 = none) so its burst can be traced to its mission |
| 12 | Burst | damage stage (`damage.rs`, `MatchState::fragments`) | Blast, fragments, crater | Unchanged; then **observation**: the mission's linked observers test a ray to the burst (+`burst_seen_lift_mm`); seen → the aim offset shrinks by `ranging_step_pct`, the adjust phase advances |
| 13 | Order end | `combat.rs:897-900` | `rounds` 0 → Idle; no bombs → Idle | Same, plus the ranging memory is written (`Squad::ranging`) |
| 14 | Base refill | `supply.rs:612-745` `refill_men`, `747-764` `bombs_wait` | One bomb per `base_bomb_s` per man in base, priced; `base_bomb_after_fire_s` 0 | Unchanged (S5a-42 (A), owner 2026-11-03); measured (T9) |
| 15 | AI | `ai.rs:36` docs, `219` `TaskKind::Barrage`, `549`, `1243-1248` assign skip, `1443-1548` `mortars`; `ai/ammo.rs:14` reserve | Scores spotted enemy groups (n·10 + trench·20 + noise) in range, never within `danger_close_m` of own men; fires `barrage_rounds` (6) above the reserve; else moves to the support spot `support_back_m` behind the front point | Same target scoring plus intel trench cells; a mission only where the sim would accept it; **spotters** (the mortar's own leader goes to a sighted spot within the voice link); map fire only as the fallback, above a higher reserve; `TaskKind::FireMission` (= 4) |
| 16 | Stats / summary | `world.rs:313` `Stats::explosions`; `sdk/crates/cli` skirmish summary `S5a` lines | Bombs fired by class; kills by source | `S5b <faction>` lines (missions, refusals, bombs and mean miss by mode, adjust rounds, flight, spotters, bombs from own base) |
| 17 | Bridge | `crates/godot-bridge/src/match_frame.rs:82-84, 514` (order target, rounds left); `match_node.rs:895, 1660, 2496, 2528, 2726-2740` (intent `barrage` with `ai.barrage_rounds`) | Order kind, target, rounds; the barrage intent | Intent `fire_mission` (rounds from `[fire_mission] player_rounds`); squad `mission {mode, phase, error_m, seen, rounds_left, observer, charge, flight_s}`; `fire_preview(squad, point)`; own intel cells; refusal events |
| 18 | HUD | `godot/match/selection_controller.gd:142, 500-501, 892-914, 964-969` (B tool, plain right-click), `ground_overlay.gd:5, 104, 137, 360` (range ring, marker) | Barrage tool: range ring and a marker | Fire-mission tool: mode label and expected error ring at the cursor, refusal reason, observer link line, ranging bar on the mortar card |
| 19 | Godot tests and mock | `godot/tools/test_match_view.gd:31, 259-267, 371, 1260-1274`; `mock_match_source.gd:43, 221, 260-266, 628, 938-942`; `autoplay_s1b.gd:507-518` | `barrage` intent and capability | Renamed and extended (Part B) |
| 20 | CLI tests | `sdk/crates/cli/src/fortify.rs:70, 87, 440-475` (10-round barrage, 90 s); `armour_test.rs:1804, 2009` (mortar vs tank, cratered trench) | `CommandKind::Barrage` | `FireMission`; the staged mortars see their targets on flat ground (250 m, inside the leader's 400 m), so these run as **Direct** missions with ranging; fortify's time budget 90 → 180 s (adjust rounds wait for their burst) |
| 21 | Capability | `sdk/crates/content/src/setup.rs:758` `Capability::Barrage` ("a member carries a mortar") | TOML name `barrage` (derived, not written by modders) | Kept as the internal bit, renamed `FireMission` in Rust; TOML/bridge name `fire_mission` (`barrage` accepted as an alias by the bridge for one slice) |

The knowledge the AI and the player use today is only the live
`Combat::seen_by` bit per soldier (`spotting.rs`); **there is no last-seen
or trench memory anywhere in the sim or the presenter.** Map fire needs
one, so S5b adds the per-player intel map (below).

---

## Data model

### 1. Weapon: the `[indirect]` charge table

```toml
# defs/weapons/grw34.toml (S5b additions)
rounds_per_min = 15          # unchanged: practical sustained rate; "6 rounds in
                             # 8 to 9 seconds (but this rate cannot be maintained)"
                             # (US WD, German Infantry Weapons, 1943)
spread_mrad = 50             # now the PRECISION spread only (game value, S5b-6);
                             # the aim error is the mission's (rules [fire_mission])

[indirect]
# High-angle fire: the solver takes the lowest charge that reaches the
# target with an elevation inside elevation_deg (degrees, whole).
# Charges I-IV of the Wgr 34: 246 / 344 / 427 / 499 ft/s = 75 / 105 / 130 /
# 152 m/s (US War Department, "German Infantry Weapons", 1943, 8-cm heavy
# mortar model 34, section e, via lonesentry.com). Charge V 174 m/s is the
# full-charge figure most sources quote (Wikipedia, 8 cm Granatwerfer 34);
# verify. Check: charge I at 87 deg reaches 60 m drag-free, the firing
# table's first row (60 m, 1,545 mils) and the def's min_range_m.
charges_m_s = [75, 105, 130, 152, 174]
# The firing table runs from 869 to 1,545 mils (48.9-86.9 deg, 6,400 mils
# to the circle); 45-87 deg taken as the laying limits (verify).
elevation_deg = [45, 87]
```

```toml
# defs/weapons/m1_mortar.toml (S5b additions)
rounds_per_min = 18          # "18 rpm sustained", 30-35 maximum (Wikipedia, M1 mortar)
min_range_m = 90             # M43A1 light HE: 100 yd minimum (Wikipedia, M1 mortar)
[indirect]
# M43A1 light HE: 7 zones, ignition cartridge plus 0-6 increments; zone 7
# reaches about 3,300 yd (Wikipedia, M1 mortar). Per-zone velocities are
# NOT sourced yet: an estimate rising from 70 m/s (ignition cartridge
# alone, like the GrW 34's charge I) to 188 m/s; owner question S5b-2
# (A). The firing table to check: "Firing tables, mortar, 81-mm, M1,
# firing shell, H.E., M43A1", War Department, Jan. 1945.
charges_m_s = [70, 95, 117, 137, 155, 172, 188]
elevation_deg = [45, 85]     # verify (FM 23-90)
```

- Validation (content, actionable): charges strictly rising, 1..=12 of
  them, each 20-600 m/s; `elevation_deg` within [45, 89] and min < max
  ("high angle only in S5b; set elevation_deg = [45, 85]"); the lowest
  charge at the top elevation must reach at most `min_range_m` + 10 %
  ("raise the top elevation or add a lower charge: the shortest
  high-angle range is 95 m, min_range_m is 60"), the highest charge at
  45° must reach at least `range_m` ("no charge reaches 600 m"). A weapon
  with `[indirect]` must be `class = "mortar"` and have `[he]`.
- `WeaponSetup::indirect: Option<IndirectSetup { charges: Vec<Fixed> (m/s),
  tan_min: Fixed, tan_max: Fixed }>`: the tangents are computed once at
  load (content crate, f64 allowed there, rounded to `Fixed`), so the sim
  needs no trigonometry.
- A mortar without `[indirect]` (legacy content) keeps the S1 flight rule
  (`[combat.mortar] flight_milli`, `min_flight_s`).

### 2. Members: the observer trait

```toml
# member traits (S4 set + S5b): "leader", "crew_gunner", "crew_assistant",
# "ammo_bearer", "observer"
```

`observer` (new bit 16 in `member_trait`) gives a man the long observe
range (`observe_observer_m`, 1,200 m: binoculars and training, an FO);
no unit in `mods/ww2` carries it in S5b (FO teams come with S11). Every
squad's **leader** observes at `observe_leader_m` (400 m): the man with
the `leader` trait, or, when he has fallen, the man who leads the
formation now (the next man takes over, as squads did).

### 3. Rules: `[fire_mission]`

```toml
# defs/rules/skirmish.toml (S5b)
[fire_mission]
# --- Who observes, and the link (artillery-on-map.md §4.1) ---
# A squad leader adjusts fire on ground points this far (m). It reveals
# nothing: spotting enemy men still uses the squad's sight_m.
observe_leader_m = 400
# Men with the "observer" trait (FO teams, S11; none in ww2 yet) (m).
observe_observer_m = 1200
# Voice link: an observer within this of the mortar's gunner can pass
# corrections (m). "A single section can usually communicate by word of
# mouth" (US WD Intelligence Bulletin, Feb 1944, German instructions for
# concentrating 81-mm mortar fire). Radio: S6/S11.
observer_link_m = 150
# The sight line to a ground point ends this high above the ground (mm),
# and to a burst this high above the burst point: the dust and smoke column
# an observer adjusts on.
point_lift_mm = 500
burst_seen_lift_mm = 3000

# --- Timing (artillery-on-map.md §4.8) ---
# From the order to the first round: the call, the fire command, laying
# the tube (s). Direct: the crew lays on what it sees.
fire_direction_s = 10
direct_s = 4
# After an adjusting round bursts, the observer's correction reaches the
# tube and the tube is re-laid before the next one (s).
adjust_wait_s = 4
# Adjust single rounds until the aim error is at most this (m), or
# adjust_rounds_max rounds, then fire for effect at the weapon's rate.
ffe_error_m = 10
adjust_rounds_max = 4

# --- Aim error (two terms, artillery-on-map.md §4.1 "critic fix C1") ---
# Where the group lands relative to the target. The weapon's spread_mrad
# (precision) scatters each round around that on top.
# Observed and direct: share of range (%) with a floor (m), shrinking to
# ranging_step_pct of itself for each round an observer SEES land
# (bracketing: "split the bracket", FM 23-90).
first_round_error_pct = 3
first_round_error_min_m = 10
ranging_step_pct = 50
# Map fire (no observer, a known point): about 3x the observed first
# round, no ranging. Set by the mortar-bench (T1 of this slice).
map_fire_error_pct = 9
map_fire_error_min_m = 30

# --- Ranging memory (per mortar squad, per target area) ---
ranging_cell_m = 50          # a new mission within this of a ranged target reuses its error
ranging_expiry_s = 600       # forgotten after this (s)
ranging_keep_move_m = 10     # forgotten when the tube moves farther than this from where it ranged (m)
ranging_max_cells = 4        # most remembered target areas per mortar

# --- The knowledge map (intel) for map fire ---
intel_cell_m = 25            # cell size of the player's intel grid (m)
intel_keep_s = 300           # a cell where an enemy was last seen stays known this long (s)
intel_trench_keep_s = 0      # a cell where an enemy was seen in a trench: 0 = for the match
intel_period_s = 1           # intel cells are updated this often (s)
intel_snap_m = 25            # map fire allowed within this of a known cell's centre or a capture point's edge (m)

# --- Player orders ---
player_rounds = 6            # rounds in a mission ordered from the HUD tool (the HUD can change it, 1..=50)
```

`[combat.mortar] flight_milli` / `min_flight_s` stay for legacy weapons
only (comment updated); `barrage_retry_m` keeps its job (renamed in the
comment, key unchanged so old rules files load).

Legacy content (a rules file **without** `[fire_mission]`, e.g. the frozen
S3c fixture) runs `FireMission` exactly as `Barrage` ran: no observer
rule, no aim error, no knowledge rule, old flight (S5b-1 (A)). Old tests
keep their hashes.

`[ai.mortar]` (new; `ai.barrage_rounds` moves here as `mission_rounds`,
the old key still loads with a deprecation note in `validate`):

```toml
[ai.mortar]
mission_rounds = 6           # rounds per AI mission (was ai.barrage_rounds)
spot_samples = 8             # candidate observation spots tried per plan (rays)
spot_max_s = 60              # a spotter that has not seen the target by then comes back (s)
spot_hold_s = 30             # a spotter with no new target stays this long before rejoining (s)
map_fire_min_pct = 60        # map fire only while the mortar holds more than this share of its bombs
map_fire_min_score = 40      # and only at a target scoring at least this (n*10 + trench*20)
```

### 4. Setup layout (`sdk/crates/content/src/setup.rs`)

- `WeaponSetup::indirect: Option<IndirectSetup>` (above).
- `member_trait::OBSERVER = 16`; `NAMES`, `ALL` and `variant::TRAITS`
  gain it.
- `RulesSetup::fire_mission: Option<FireMissionSetup>` (all keys above,
  times in ticks, distances `Fixed`, percentages `u32`), `AiSetup::mortar`.
- `Capability::Barrage` renamed `Capability::FireMission` (bit 2 kept;
  TOML name `fire_mission`; `capabilities = ["barrage"]` in a unit file is
  derived, never written, so no mod breaks).
- `MatchSetup::validate` re-checks every new field (saves are untrusted).
- JSON Schemas (`frontline-cli schema`): `weapon.schema.json` gains
  `indirect`, `rules.schema.json` gains `fire_mission` and `ai.mortar`.
- `resolved_defs` setup-bytes baseline and the canonical snapshot are
  re-blessed with old values in the test comment (as S5a did).

---

## Sim rules

### Fire modes

A mission's mode is decided by the sim, per round, when the round is
fired (never sent in the command):

| Mode | Needs, at the moment the round is fired | Aim error | Ranging |
|---|---|---|---|
| **Direct** | The mortar squad's own leader (not detached, within `observer_link_m` of the gunner) has a sight line to the target point (+`point_lift_mm`) within `observe_leader_m` | starts at `max(first_round_error_pct × range, first_round_error_min_m)` | yes, on rounds the crew sees land |
| **Observed** | Any other linked observer of the same player has that sight line: a squad leader (or `observer` man) of any own infantry squad, the mortar's own **detached** leader included (the spotter), standing within `observer_link_m` of the gunner, the point within his observe range | same start | yes, on rounds a linked observer sees land |
| **Map fire** | Nobody linked sees the point; the order was accepted by the knowledge rule | `max(map_fire_error_pct × range, map_fire_error_min_m)`, no ranging | none |

- The **aim offset** is a vector in the mission (`Mission::offset`),
  drawn once when the mission starts: its length is the mode's start
  error, its bearing uniform (counter RNG `Purpose::MISSION_AIM` = 29,
  keyed by the squad id and the order tick). A mission at a target within
  `ranging_cell_m` of a remembered ranging cell (tube not moved, not
  expired) starts from that cell's stored offset instead, when that is
  smaller.
- **Upgrades and downgrades:** a Map mission whose point becomes seen by a
  linked observer becomes Observed and ranges from its current (map)
  offset, so it needs more adjusting rounds; an Observed mission whose
  observer is lost (dead, moved out of link, sight line cut by smoke)
  keeps firing on its current offset as Map fire (the crew fires on the
  last data). The mode shown in the HUD is the mode of the last round.
- **Precision spread** on top of the aim point, unchanged from today:
  triangular per axis with half-width `spread_mrad × range`, scaled by the
  gunner's suppression (`spread_per_point_pct`), RNG `SHELL_AIM` (gunner,
  tick).

### Mission phases

1. **Prep.** `ready_at = now + fire_direction_s` (Observed or Map) or
   `+ direct_s` (Direct). The squad walks into range first if needed
   (today's rule); set-up (`setup_ms`) runs in parallel with the prep.
2. **Adjust** (Observed or Direct): always at least one adjusting round
   (the observer calls fire for effect only after seeing one land; none
   when the mission starts from a remembered ranging cell whose offset is
   already within `ffe_error_m`), then
   more while the offset is above `ffe_error_m` and fewer than
   `adjust_rounds_max` were fired. One round at a time: the next waits
   until that round's burst plus `adjust_wait_s`
   (`Mission::wait_burst` set, cleared at the burst). A round that never
   bursts (off the map; ttl) clears it at its ttl.
3. **Fire for effect:** the remaining rounds at the weapon's rate
   (`interval`, as today). Rounds seen landing in this phase still shrink
   the offset, for later rounds (bombs already in the air are not
   corrected).
4. **End:** rounds 0 or no bombs (`take_bomb` false) → `Idle`; the
   ranging memory keeps the final offset.

Map fire skips phase 2.

### Observation of a burst

When a projectile with `mission != 0` bursts (Damage stage, serial, slot
order), and its squad still holds that mission (same order tick):
for each linked observer (ascending id; at most `OBSERVE_RAYS` = 4 rays),
cast `Heightfield::line_of_sight` from his eye (the spotting eye height,
fire-step height in a trench) to the burst point + `burst_seen_lift_mm`;
smoke blocks it (`crate::items`, as spotting does). The first clear ray:
`offset = offset × ranging_step_pct / 100`, `seen += 1`. No clear ray:
nothing changes. Counted in `Stats` (`bursts_seen`, `bursts_unseen`).

### The charge solver (`crate::indirect`, new module, pure integer math)

For horizontal distance `R`, height difference `h` (target minus muzzle)
and charge velocity `v`, the high-angle solution in `u = tan θ`:
`a = g R² / (2 v²)`, `u = (R + sqrt(R² − 4a(a + h))) / (2a)`; no real root
→ that charge cannot reach. Valid if `tan_min ≤ u ≤ tan_max`. The lowest
valid charge wins. Launch: `vx = v / sqrt(1 + u²)` along the ground
direction, `vz = u · vx`; flight time `R / vx`. Computed in `i64` on raw
Q16.16 with the distances scaled to decametres where the squares would
overflow (600 m² = 360,000 does not fit `Fx`); a unit test compares
flight time and launch angle with an f64 reference over 60-600 m and
every charge (within 0.5 %), and a burst-point test flies the projectile
in the sim (drag 0, 60 Hz substeps) and lands within 1 m of the aim point
at 100, 300, 600 m and with ±20 m height differences. No valid charge
(target above the arc, or too close) → the order is refused ("cannot
reach: too close for the highest elevation") at order time and the round
is skipped and the mission ends at fire time.

Flight times this gives (drag-free, flat ground; checked in Python, scratchpad
`s5b/mc/`):

| Range | GrW 34 | Elevation | M1 (estimated zones) | Today (both) |
|---|---|---|---|---|
| 60 m | charge I, 15.3 s | 87.0° | below its 87 m minimum | 4.6 s |
| 200 m | I, 15.0 s | 79.8° | zone 1, 14.0 s | 8.4 s |
| 400 m | I, 14.2 s | 67.9° | zone 1, 12.8 s | 11.8 s |
| 500 m | I, 13.2 s | 59.7° | zone 2, 18.6 s | 13.2 s |
| 560 m | I, 11.9 s | 51.2° | zone 2 | 14.0 s |
| 600 m | II, 20.6 s | 73.9° | zone 2, 18.2 s | 14.5 s |

So the bomb lands 3-11 s later than today inside 400 m (where most AI fire
is) and up to 6 s later at the longest ranges: moving men escape
(artillery-on-map.md §4.8), and every adjusting round costs about 20 s
(flight plus `adjust_wait_s`). The M1's lowest zone at 85° reaches 87 m,
which matches the M43A1's sourced 100 yd (91 m) minimum: `min_range_m`
60 → 90 on the M1 (the content check would refuse 60).

### The knowledge map (intel) and the knowledge rule

- `PlayerState::intel: BTreeMap<u32, IntelCell>` keyed by cell index
  (`row × cols + col` of `intel_cell_m` cells over the map), `IntelCell {
  seen: u32 (tick of the last sighting), flags: u8 (TRENCH = 1, VEHICLE = 2) }`.
  Sparse: only cells where something was seen.
- **Update** (Status stage, serial, every `intel_period_s`, players in
  order, entities ascending): every enemy soldier or vehicle the player
  sees (`seen_by` bit) marks its cell (`seen = tick`; `TRENCH` when the
  soldier is `in_trench`; `VEHICLE` for a vehicle). Wrecks and corpses do
  not mark. Cells older than `intel_keep_s` without `TRENCH` (or `TRENCH`
  older than `intel_trench_keep_s` when that is not 0) are removed in the
  same pass.
- **Known point** (the knowledge rule): `P` is known to player `p` when
  any of: (a) an own observer (any own squad's leader, or `observer` man,
  linked or not) has a sight line to `P` within his observe range now
  (at most 8 rays per order, nearest observers first);
  (b) a capture point's area, grown by `intel_snap_m`, contains `P`;
  (c) an intel cell of `p` has its centre within `intel_snap_m` of `P`.
- **Order time:** a `FireMission` whose target is not known is refused
  with `FireMissionRefusal::UnknownGround` ("Unknown ground: no observer
  sees it and nothing was seen there"). That is the only check of (c):
  a mission accepted on a known point keeps firing if the cell later
  expires (the order was given on knowledge the player had).
- What it deliberately does not include (S5b-3 (A)): ground the player
  merely saw empty (an "explored" layer), and trenches seen without men in
  them. Both would need terrain visibility per player, which the
  spotting pass does not compute.
- Cost: one pass over the entities per player per second; a 1 km map has
  41 × 41 cells, of which a few hundred are ever touched.

### Refusals (`FireMissionEvent`, moved to the snapshot like `CallInEvent`)

`NotMortar`, `Rounds` (outside 1..=50), `OffMap`, `InsideMinimum`,
`CannotReach` (no charge solution at the squad's position for a target in
range), `UnknownGround`, `NoBombs` (counted squad with none held). The
HUD shows the reason; the AI never emits a mission the same check would
refuse.

### 5-man squads and bombs

- The mortar squads become five men (content below); `take_bomb` and the
  crew feed are unchanged. The squad's leader is no longer the gunner: the
  gunner is slot 1 (`crew_gunner`), the leader slot 0 (`leader`, observes).
  `fire_missions` already picks the first non-detached man carrying the
  mortar.
- `squad_ammo_pct` (the 30 % refill trip) and `mortar_bombs` count the
  bearers' bombs (they are member `rounds` of the mortar's feed).

### Ranging memory

`Squad::ranging: Vec<RangingCell { target: FxVec2, offset: FxVec2, seen:
u8, at: u64 (tick), from: FxVec2 (gunner position) }>`, at most
`ranging_max_cells`, oldest replaced; written at each seen burst and at
mission end (only for Observed / Direct missions with at least one seen
round). Dropped when older than `ranging_expiry_s`, or on the next mission
when the gunner stands farther than `ranging_keep_move_m` from `from`.

---

## Save, hash and versions

One bump each for the slice: **`SAVE_VERSION` 13 → 14** (new state),
**`PROTOCOL_VERSION` 7 → 8** (`Barrage` → `FireMission`, new events).

| New state | Where | Hashed | Validated on load |
|---|---|---|---|
| `Order::FireMission { target, rounds, m: Mission { mode: u8, phase: u8, offset: FxVec2, err0_mm: u32, order_tick: u64, ready_at: u64, wait_burst: bool, adjust: u8, seen: u8, fired: u16, charge: u8 } }` | `game.rs` | yes, every field | mode/phase in range; `ready_at`, `order_tick` not after now + prep; `adjust ≤ adjust_rounds_max`; offset length ≤ the largest possible start error; `charge` < the weapon's charge count; only on squads with the capability |
| `Squad::ranging: Vec<RangingCell>` | `game.rs` | yes | ≤ `ranging_max_cells`; ticks not in the future; points on the map; only on mortar squads |
| `PlayerState::intel: BTreeMap<u32, IntelCell>` | `game.rs` | yes | cell index < cols × rows; `seen` ≤ now; flags ⊆ known bits |
| `Projectile::mission: u32` | `projectile.rs` | yes (pool slot) | 0 or an existing squad id + 1 at load; a stale id is harmless (lookup fails) but refused for clarity |
| `AiState::spotters: BTreeMap<SquadId, AiSpotter { man, spot, target, since }>` | `ai.rs` | yes | the man is a living member of that squad, detached; ticks not in the future |
| `Stats` S5b counters (missions, refusals, bombs and miss sums by mode, bursts seen/unseen, adjust rounds, flight ticks, bombs from own base, spotters) | `world.rs` | as `Stats` today | — |

Tests: `world::tests::s5b_fire_mission_state_is_saved_hashed_and_checked`
(each field changes the hash, survives save/load, broken values refused);
determinism at 1/2/3/8 threads and save → load → continue mid-adjust (a
round in the air, `wait_burst` set), mid-effect, with a spotter out; net
loopback and both soaks in the release run. `relay-protocol.md`: version
8 entry, `FireMission` in the fixed-size list, limits unchanged.

---

## AI mission logic (`ai.rs` `mortars`, new `ai/mortar.rs`)

Per plan, for each mortar squad (ascending id) not on a mission:

1. **Targets.** As today: spotted enemy groups by squad, in range (`range_min
   + 20` to `range_max − 10`), not within `danger_close_m` of own men,
   score `n·10 + trench·20 + noise`; **plus** the player's intel cells with
   `TRENCH` within `intel_keep_s` (score 30 + 10 per man seen there last,
   no noise). Best first.
2. **Observed or direct now** (the sim's own mode check, read-only, at most
   4 rays): emit `FireMission` with `mission_rounds` (never more than held)
   if above `mortar_reserve_pct` or an own point is threatened. Counted
   per mode.
3. **Spotter.** Else, when the squad's leader is alive and not already
   out: try `spot_samples` spots (on the line from the gunner toward the
   target at 40, 80, 120 and 150 m, and 30 m to either side of the 80 and
   120 m points; clamped inside the voice link; not within 50 m of a seen
   enemy); the first with a clear sight line from a standing eye to the
   target + `point_lift_mm` wins: `MoveUnits { units: [leader], target:
   spot }` and stance crouch (S1b orders), `AiState::spotters` records it.
   Next plans re-check the mode; Observed → mission. A spotter is called
   back (`Regroup`) after `spot_max_s` without a mission, or `spot_hold_s`
   after the last mission with nothing new; also when the mortar moves.
   The squad's regroup-on-walk rule skips a recorded spotter.
4. **Map fire** (fallback): the best target is a known point (spotted men
   always are), the squad holds more than `map_fire_min_pct` of its bombs,
   the score is at least `map_fire_min_score`: `FireMission` (the sim
   will fire it as map fire). Counted.
5. **No blind fire:** nothing is emitted that the sim's knowledge rule
   would refuse (debug assertion + a test that counts AI refusals = 0).
6. Otherwise: the support spot as today (`support_back_m` behind the front
   point). The mortar's assignment skip (`ai.rs:1243`) is unchanged.

Counters in the summary: missions by mode, spotter sends / missions after
a spotter / spotter time-outs, map-fire holds by the reserve, refusals
(must be 0).

---

## HUD (HUD v2 layout, military UI direction)

- **Fire-mission tool** (key B, the Barrage button renamed "Fire
  mission"): while armed, the cursor shows a label "Observed · ±14 m",
  "Direct · ±12 m", "Map fire · ±45 m" or, in red, the refusal ("Unknown
  ground: no observer sees it and nothing was seen there", "Too close",
  "Out of range: the team will move", "No bombs"). The ring at the cursor
  is the expected miss: start aim error (or the remembered one) plus the
  precision spread's typical radius; dashed for map fire. The value comes
  from `fire_preview(squad, point)` in the bridge, which runs the sim's
  read-only mode check on the client's own `World` (not in the sim's ray
  budget, never in the tick). Right-click or the confirm key orders
  `player_rounds` (Shift+wheel changes 1..=50, shown in the label). The
  mortar range ring stays.
- **Observer link line:** for a selected mortar squad on a mission (or
  with the tool armed), a thin line from the gunner to the observer used
  for the last round (Direct: none), and from the observer to the target;
  dotted when the link is broken.
- **Mortar card** (detail panel and strip card): mode tag (DIR / OBS /
  MAP), a ranging bar (current aim error against the first round's, e.g.
  "Ranging 2 · ±6 m"), the phase ("Laying", "Adjusting", "For effect"),
  rounds left, flight time of the last round. Intel cells are drawn
  faintly on the ground while the tool is armed (own player only).
- **Bridge contract** (`godot/docs/match_state.md` "S5b"): squad
  `mission { mode, phase, error_m, first_error_m, seen, rounds_left,
  observer_id, charge, flight_s, target }`; `intel` (own player: cell
  centre, age s, trench); events `fire_mission_refused { squad, reason }`;
  intent `fire_mission { squads, target, rounds }` (`barrage` kept as an
  alias for one slice); `fire_preview`.
- **Mock:** missions with modes, a refusal, an observer line, ranging
  progress; `--capture_setup s5b_mortars`.

---

## The CLI bench for the 3× ratio (`frontline-cli mortar-bench`)

New subcommand in `sdk/crates/cli/src/mortar_bench.rs`, staged like
armour-test (flat ground, `scenarios::staging`), each faction's mortar
squad, `--seeds N` (default 50), `--rounds R` (default 8, the done-when's
"same mission"), `--ranges 200,300,400,500`, `--rules-override`, `--json`.
For each range and seed, three runs of the same mission on the same seed:

1. **Observed:** the mortar's own leader detached 40 m to the side of the
   tube (a spotter, so the mode is Observed), with a clear sight line;
   no other squad on the map.
2. **Map fire:** nobody observes (the bench sets both observe ranges to 0
   in its setup, as armour-test edits its staging setup); the target cell
   is marked in the player's intel before the order.
3. **Observed, unseen bursts:** as 1, but a smoke cloud is laid on the
   observer's sight line right after the order (the point was seen at
   order time; the bursts are not).

Each burst's miss distance (burst point to target, 2D) is read from a
diagnostic burst log (`World::enable_burst_log`, not saved or hashed, like
the hit log). Printed per range: mean miss observed / map / unseen,
ratio map ÷ observed, adjusting rounds, rounds seen, mean flight time and
charge; then the pooled ratio (mean over all ranges and seeds). **Pass:**
pooled ratio 2.5-3.5; every range 2.0-4.5 (report); in run 3 the aim
error after the last round equals the first-round error (no ranging) and
in run 1 it fell by `ranging_step_pct` per seen round (hard checks).

Expected from a Monte Carlo of these rules (scratchpad `s5b/mc/`, Python,
the triangular precision spread at 50 mrad, no suppression): with the
starting values above (3 % / 10 m, 9 % / 30 m, 50 % steps, `ffe_error_m`
10) an 8-round mission gives a pooled ratio of **3.4** (200 m 4.4, 300 m
3.4, 400 m 3.2, 500 m 3.1); 6 rounds 3.3. The observed mean miss is
mostly the precision spread (7-15 m), so the ratio is set by
`map_fire_error_*`; Part A sets the final value on the real bench (likely
8 % / 27 m to centre it near 3.0). A larger first-round error (more
adjusting) changes the ratio little. The floor matters at short range.

---

## Content (Part A), with sources

| Item | Change | Sources / reasoning |
|---|---|---|
| `weapons/grw34.toml` | `[indirect]` charges 75 / 105 / 130 / 152 / 174 m/s, elevation 45-87°; `spread_mrad` 50 now precision only (comment) | US War Department, *German Infantry Weapons* (1943), 8-cm heavy mortar model 34 (charges I-IV 246 / 344 / 427 / 499 ft/s; range 60-1,900 m; firing table 869-1,545 mils; "6 rounds in 8 to 9 seconds (but this rate cannot be maintained)"; "Twenty-four rounds are usually carried by a mortar crew in action"; bombs in steel cases of 4 *(verify; other sources say 3)*; bomb 3.5 kg); Wikipedia, *8 cm Granatwerfer 34* (174 m/s, 2.4 km). Charge I at 87° reaches 60 m drag-free: the table's first row |
| `weapons/m1_mortar.toml` | `[indirect]` 7 zones 70-188 m/s *(estimate)*, elevation 45-85° *(verify)*; `rounds_per_min` 15 → 18; `min_range_m` 60 → 90 (M43A1 minimum 100 yd; the lowest zone at 85° reaches 87 m) | Wikipedia, *M1 mortar* (18 rpm sustained, 30-35 maximum; M43A1 3.2 kg, 100-3,300 yd, zone 7; squad 8: leader, gunner, assistant, 5 bearers; tube 20 kg, mount 21 kg, base plate 20 kg); zones: ignition cartridge + 6 increments (dealer description of an M43A1, secondary). Per-zone velocities from the Jan. 1945 firing table when found (S5b-2) |
| `units/ger_mortar_team.toml` | **Granatwerfergruppe**, 5 men: leader (MP 40, binoculars: observes), gunner (GrW 34, P 38), assistant (Kar98k, 4 bombs), 2 bearers (Kar98k, 8 bombs each: two cases); **20 bombs**; cost 300 × 0.8 = **240** (S5a-37 cut); `sight_m` 150 | roster-expansion.md §3.1 (5 men, ~20 bombs, 300); crew carry from *German Infantry Weapons* (24 rounds a crew in action); Intelligence Bulletin Feb 1944 (section by voice). The historical 1944 Gruppe was larger (1 + 5 or more, verify); 5 is the roster's pop-capped size (Q6 (A)) |
| `units/usa_mortar_team.toml` | **81mm Mortar Squad**, 5 men: squad leader (M1 carbine, observes), gunner (M1 mortar, M1911A1), assistant gunner (carbine, 4 bombs), 2 ammunition bearers (carbine, 8 bombs each); **20 bombs**; cost **300** | roster §3.2 (5 men, 300); Wikipedia *M1 mortar* (8-man squad: leader, gunner, assistant, 5 bearers); per-bearer load not sourced (S5b-5): 8 M43A1 ≈ 26 kg |
| ids | `ww2:ger_mortar_team` and `ww2:usa_mortar_team` keep their ids (names change), so saves of the AI mix, `[ai.mix]` tags and tests stay | S5b-7 (A) |
| `cartridges/8cm_wgr34.toml`, `81mm_m43a1.toml` | comment only: the burst stays on the weapon's `[he]` (S5b-8 (A): moving it to the cartridge waits for a second bomb type) | — |
| `rules/skirmish.toml` | `[fire_mission]`, `[ai.mortar]` (above); `ai.barrage_rounds` → `ai.mortar.mission_rounds`; `[combat.mortar]` comments | artillery-on-map.md §4.1, §4.8 |
| Docs | `docs/modding/fire-missions.md` (new: modes, observers, link, aim error, intel, charges, the bench); `tuning.md` rows; `ammo.md` mortar section; `godot/docs/match_state.md` "S5b"; `relay-protocol.md` | — |

Fire-control practice behind the rules (secondary unless stated):
- **Observers and links.** German instructions for 81 mm mortar fire (US
  WD *Intelligence Bulletin*, Feb 1944): the section is the fire unit; a
  single section is controlled by voice; two sections or a platoon need a
  telephone line; ranging is normally done by one mortar on a
  registration point; fire for effect only after ranging, except against
  fleeting or large targets. US: the 81 mm squad's leader or the platoon's
  observer adjusts; sound-powered telephone and the SCR-536 radio at
  company level (FM 23-90, 1943, *verify*). S5b builds voice only.
- **Bracketing.** The observer brackets the target (one round over, one
  short) and splits the bracket until rounds fall within a set distance,
  then calls fire for effect (FM 23-90 and its successors, "split the
  bracket"). Two to four adjusting rounds are typical; the 50 % step per
  seen round and `adjust_rounds_max` 4 model that.
- **Time to first round.** A voice-linked mortar section fires within
  tens of seconds of a call; 10 s prep plus 13-21 s flight gives 23-31 s
  from order to first burst, inside artillery-on-map.md §4.8's 20-60 s.

---

## Targets

Standard set as S5a: 40 (seeds 1-20 × both bases), 160 (seeds 1-80 × both)
and an out-of-sample 160 (seeds 101-180 × both), `--jobs 3 --minutes 45`,
each run alone; armour-test `--stress --seeds 10` and `--seeds 5`;
fortify-test `--seeds 10`.

| # | Target | Baseline | How |
|---|---|---|---|
| T1 | the same mission, observed and unobserved (map fire): pooled mean miss ratio **2.5-3.5×**; each range 2.0-4.5× (report) | — (no aim error today) | `mortar-bench --seeds 50` |
| T2 | ranging tightens **only** when an observer sees the round land: aim error unchanged over a mission whose bursts are hidden by smoke; halved per seen round otherwise | — | sim test + bench run 3 |
| T3 | a mortar with no resupply stops after its carried bombs, **bearers included** (20 for each 5-man squad), and the order ends "no bombs" | 15 (3-man) | sim test |
| T4 | **no fire into ground the player has never seen:** an order at an unknown point is refused (`UnknownGround`), at a capture point, a last-seen cell, a trench cell or an observed point accepted; AI refusals 0 in the 40 set | — | sim tests + summary |
| T5 | each faction 35-65 % on 40; **45-55 % on 160** (seeds 1-80) **and** on the out-of-sample 160 | 50.0 % / 43.8 % / 51.9 % | summary |
| T6 | no base side > 62 % (40, 160, out of sample) | 50.0 / 52.5 / 50.6 % | summary |
| T7 | no match < 10 or > 40 min; median 15-30 | 10.4-38.5, median 16.5 (40) | summary |
| T8 | **HE share of kills falls by at least a third from the post-S3b baseline: mean ≤ 26.3 %** (39.5 % × 2/3) on the 40 and both 160 sets; max reported | 26.9 / 28.3 / 28.5 % | summary |
| T9 | reported against S5a: mortar bombs fired per side and per match, by mode; MP on ammo per kind; **bombs fired from inside the own base** by mode and the base battery A/B (`base_bomb_after_fire_s` 0 vs 120 on the 160 set: base wins, bombs, HE share) | bombs 28,834 / 25,709 (160); MP 651.5 / 519.4 per match | summary; one A/B run |
| T10 | true flight time: the solver's flight time and angle within 0.5 % of an f64 reference; a fired bomb lands within 1 m of its aim point (no spread) at 100 / 300 / 600 m and ±20 m height; the GrW 34 fires charge I at 60-560 m, charge II beyond | 60° rule | unit + sim tests |
| T11 | the AI fires through observers: in the 40 set Observed + Direct missions ≥ 70 % of AI missions; spotters sent ≥ 1 per match per side (mean); map-fire missions only above `map_fire_min_pct` | — | summary |
| T12 | determinism: hashes equal at 1, 2, 3 and 8 threads and through save and load (mid-adjust with a round in the air, mid-effect, a spotter out); net loopback; soaks; every new field saved, hashed (test per field) and validated | pass | tests |
| T13 | fortify-test PASS (mortar row: trench ≤ 40 % of open, now a Direct 10-round mission with adjusting); armour-test all hard checks at `--stress --seeds 10` **and** `--seeds 5`; golden ballistics; `validate mods/ww2`; clippy `-D warnings`; fmt | see baseline | as listed |
| T14 | tick p99 peak < 1.5 ms (`--jobs 3`); stress case p99 < 10 ms | 0.363 / 1.110 / 1.113 ms | summary; case 14 |
| T15 | HUD: Godot checks pass (`check_project`, `test_match_view`, `test_net_ui`, `test_data_reload`), screenshots at 1280×800 and 1600×900 looked at (tool label and ring, refusal, link line, ranging bar), F5 reload of a charge edit and a broken charge table refused with the fix | 251 / 143 / 17 | Godot |
| T16 | every existing test passes (521 at `3cd5717`; legacy fixture hashes unchanged) | 521 | `cargo test --workspace --release -- --include-ignored` |

---

## Tune results

Stage TUNE, one round, **no data changed**. HEAD `0df7568` (the Part B
binary, data unchanged) was re-checked and then probed with four A/B
variants. Two targets stay open and the Tune stage found no lever inside
its remit that moves them (below).

**Re-check of `0df7568`.** The 40 set (`--jobs 3 --minutes 45`) reproduces
every one of the 40 match hashes of the Part B final run, so the Part B
numbers for the 40, the 160 (seeds 1-80) and the out-of-sample 160 (seeds
101-180) stand as measured (the same binary and data cannot change a
deterministic hash; only timing varies). fortify-test `--seeds 10`: PASS,
output identical to Part B (mortar row 45 vs 114 = 39.5 %, limit 40 %).
armour-test `--seeds 5`: all hard checks pass (229.7 s), case 9 is 23 vs 84
= 27.4 %. armour-test `--stress --seeds 10`: all pass (446.3 s), case 9 is
46 vs 164 = 28.0 %, case 14 p99 1.647 ms (limit 10 ms; Part B 1.516 ms,
timing varies with machine load).

**Targets after the round (unchanged from Part B):** T1-T3, T6-T8, T10-T16
met. **T5** misses out of sample (Germany 40.6 %, seeds 101-180; 47.5 % on
1-80, 40.0 % on the 40 set). **T4** misses by one AI order per 160 matches
refused `no_bombs`. Neither is a data fault, see below.

**T5, the German share.** A third untouched set was added (seeds 201-280,
both bases, 160 matches) so the three 160 sets can be pooled.

| Set (160 matches) | Germany wins | Base 1 wins |
|---|---|---|
| seeds 1-80 | 76 (47.5 %) | 86 |
| seeds 101-180 | 65 (40.6 %) | 97 |
| seeds 201-280 | 77 (48.1 %) | 87 |
| pooled (480) | 218 (45.4 %, standard error 2.3) | 270 (56.3 %) |

Germany wins 39.2 % of the matches where it starts in base 0 and 51.7 %
where it starts in base 1 (240 each): base 1 is worth about 6 points to
whoever holds it (T6 passes, 53.8-60.6 %), and the German deficit is what
remains after the two bases cancel, about 4-5 points (S5a-37 recorded the
same lean: 45.6 % on 1-80, 50.0 % out of sample, 42.5 % on the 40 set).
It is not caused by the mortars: Germany and the USA kill the same absolute
number of men with mortars per match (5.3 % of about 110 and 4.8 % of about
130 kills), the old 21.8 / 18.2 % edge came with the base battery that the
observer rule has removed (bombs fired from inside the own base: 37-39 % in
Part A, 6-9 % now).

Four A/B probes (`--rules-override`, `--jobs 3`, German wins out of 160):

| Probe | Result | Mortar numbers |
|---|---|---|
| baseline, seeds 201-280 | 77 | 62.7 / 56.9 bombs per match (G / US), 5.3 / 4.8 % of kills |
| `[ai.mortar] mission_rounds` 6 -> 8 | 66 | missions 11.3 / 9.0 per match, bombs 73.0 / 61.2, 5.8 / 5.2 % of kills, HE share 15.3 % |
| German mortar card 240 -> 180 (-25 %) | 72 | bombs 79.9 / 53.4 per match |
| German bombs 20 -> 24 (2 + 4 + 9 + 9; the MID 1943 source says 24 are usually carried) | 75 | bombs 70.8 / 55.4 |
| `ww2:ger_grenadiergruppe` 225 -> 205 (-9 %), all three sets | 83 / 65 / 69 = 217 | vs baseline 76 / 65 / 77 = 218 |

None of them moves the pooled share: a quarter off the mortar card and
9 % off the main infantry card (bought about 5 times a match) change
nothing beyond the spread. That spread is large: the same price change
moved single sets by +7, 0 and -8 wins (SD about 7.5 wins = 4.7 points,
against 3.9 points of binomial error for 160 matches), because any change
re-orders the AI's random draws. At that noise a 45-55 % band on a single
160 set is a lottery for a side whose true share is about 45-46 %
(roughly a one in two chance of a pass), and every lever in the Tune remit
(mortar card cost, bombs carried, error and ranging percentages, observe
ranges, AI mission thresholds) was shown or argued to have no effect on it
(`map_fire_min_pct` 85 in Part B: +1 win). Choosing the variant that lands
in band on all three sets would be fitting noise, so none was shipped.
Owner question **S5b-23** records the options (the lever has to be an
infantry or rifle-versus-bolt-action change, the S5a-37 (B) and (C) family,
not a mortar one).

**T4, the one refusal.** Found by running each seed of the 40 set alone:
seed 8, German side, `no_bombs` at tick 12,420. The refused squad had one
member left (its leader; the crew and all bomb carriers had fallen) in the
reaction delay between the AI's plan, which saw at least 2 bombs, and the
order's arrival. The sim is right to refuse and the AI cannot know that its
crew will fall. The order is dropped and the next plan sees the dead crew;
rate 1 to 3 per 160 matches (Germany 2 and USA 1 in seeds 201-280). S5b-22
stays (A): accept as a race. No code was changed (a temporary debug print
used to find it was reverted; the rebuilt binary is byte-identical to the
Part B one).

**Not tuned, on purpose:** the owner's base battery lever stays 0; ammo
prices untouched; the 20 % German cut untouched; mortar kill share (4.6-5.5 %
of kills, S5b-17) is not one of the targets and the HE share (13-14 %,
limit 26.3 %) has room, so no lethality change was made.

---

## Implementation order

Two parts, a commit at each numbered step that leaves the tree green.

### PART A: sim + content (FireMission, modes, observer rule, ranging, charges and flight time, 5-man squads, bench, saves, tests)

1. **A1 Content and setup.** `[indirect]` on weapons (`IndirectSetup`,
   tangents at load), the `observer` trait, `[fire_mission]` and
   `[ai.mortar]` (`FireMissionSetup`, `AiMortarSetup`, the old
   `barrage_rounds` key still read), validation with actionable errors,
   schemas, `Capability::FireMission` (TOML `fire_mission`); content tests
   (charge table refused when not rising / not reaching `range_m` /
   lowest charge too long, with the fix text; legacy has none). No sim
   change yet: every summary line of the 40 set equal to the baseline
   (the setup bytes change, so only hashes differ). Commit.
2. **A2 Charge solver** (`crates/sim/src/indirect.rs`) and its tests
   (T10). Wire it into the mortar fire path behind `indirect.is_some()`;
   legacy keeps the S1 rule. Measure the 40 set once (report only: the
   flight-time change alone).
3. **A3 FireMission order and command.** `CommandKind::FireMission`,
   `Order::FireMission { target, rounds, m }`, `OrderKind`, `order_fire_mission`
   (no regroup of detached men), refusals as `FireMissionEvent`, `Engage::Never`,
   vehicle guard, protocol 8 (`command_in_limits`, relay doc), CLI callers
   (`fortify.rs` with `BARRAGE_S` → 180 s, `armour_test.rs`), bridge
   intent (`fire_mission`, `barrage` alias). Legacy content behaves as
   Barrage did (hash test on the S3c fixture).
4. **A4 Modes, aim error, phases, observation.** `Mission`, the mode check
   (`fire_mission::mode_for`, read-only, ≤ 4 rays), `MISSION_AIM`,
   prep / adjust / effect, `Projectile::mission`, burst observation in the
   Damage stage, ranging memory on the squad.
5. **A5 Intel and the knowledge rule.** `PlayerState::intel`, the 1 s
   update and expiry, `known_point`, `UnknownGround`.
6. **A6 5-man squads** (content above), price and pop checks
   (`variants.rs` cost expectations, `match_mode.rs` refusal test card),
   T3 test with the bearers.
7. **A7 Saves, hash, versions.** `SAVE_VERSION` 14; the per-field test;
   determinism tests extended (mid-adjust, mid-effect); stats counters and
   the `S5b` summary lines; `World::enable_burst_log`.
8. **A8 `mortar-bench`** and its run (T1, T2); set `map_fire_error_*` so
   the pooled ratio sits near 3.0 (the only data change allowed for T1).
9. **A9 Measure (AI unchanged except that it now issues `FireMission` with
   the old targets, so its missions are Direct/Observed/Map by where its
   mortars stand):** 40 set, armour 5 and stress 10, fortify 10, full
   tests, golden, validate, clippy, fmt. Report every target with numbers;
   commit with the Part A log in `docs/slices/S5b-log.md`.

### PART B: AI missions and spotters, HUD tool, Godot tests, measure

1. **B1 AI missions** (`ai/mortar.rs`): targets with intel trench cells,
   the mode check, spotters (`AiState::spotters`, saved, hashed,
   validated), map-fire fallback above `map_fire_min_pct`, no blind fire
   (refusal counter 0 asserted in a test), regroup skip for spotters,
   counters; tests: a spotter goes out and the mission becomes Observed; a
   mortar with no observer and no intel fires nothing; determinism with a
   spotter out (save/load).
2. **B2 Bridge and HUD**: `fire_preview`, mission fields, intel cells,
   refusal events; the fire-mission tool, link line, ranging bar, mode
   tags; mock and `--capture_setup s5b_mortars`; `match_state.md` "S5b".
3. **B3 Godot tests**: `test_match_view` (tool label and ring per mode,
   refusal text, link line, ranging bar, intent `fire_mission`),
   `test_net_ui`, `test_data_reload` (+ a charge edit reloads, a broken
   charge table refused with the fix); screenshots at both sizes; the
   `settings.cfg` checksum before and after.
4. **B4 Measure**: 40, 160 and out-of-sample 160; the base-battery A/B
   (T9); armour 5 and stress 10; fortify 10; tests; tick p99. If T5 (45-55
   % on 160) or T8 (HE ≤ 26.3 %) misses, the levers in order (owner's
   cost-balanced rule): AI rules and data (`[ai.mortar]`, reserve,
   `mission_rounds`), then `[fire_mission]` game values (map-fire error,
   `fire_direction_s`, `adjust_wait_s`, observe ranges), then card costs;
   never the bomb's lethality or the precision spread without the owner
   (S5b-6). Log in `S5b-log.md`, commit.

---

## Owner questions (work proceeds on (A); also in `docs/TODO.md`)

- **S5b-1 Legacy rules without `[fire_mission]`.** (A) `FireMission` runs as
  the old `Barrage` (no observer rule, no aim error, no knowledge rule,
  old flight rule), so the frozen S3c fixture and old mods keep their
  hashes. (B) Refuse such mods.
- **S5b-2 M1 81 mm charge velocities.** (A) Estimated zones 70-188 m/s
  (rising like the GrW 34's charges) until the Jan. 1945 firing table is
  found; only the lowest two zones are used at 600 m. (B) Copy the GrW
  34's charges.
- **S5b-3 What counts as known ground.** (A) Last-seen enemies (5 min),
  cells where an enemy was seen in a trench (the match), capture points,
  and whatever an own observer sees now. Not: ground seen empty, trenches
  seen without men (both need per-player terrain visibility). (B) Add an
  "explored" layer later (S6/S11).
- **S5b-4 Registered fire.** (A) Not in S5b: the ranging memory only helps
  a later observed or direct mission at the same area; map fire never uses
  it (it would let a base mortar fire accurately all match after one
  observed mission). S11 builds registered fire for guns and mortars with
  its expiry and residual error. (B) Build it for mortars now.
- **S5b-5 Bombs per squad.** (A) 20 a squad (assistant 4, two bearers 8
  each; leader and gunner none): the roster's "~20", under the 24 a German
  crew usually carried in action. Per-bearer loads are not sourced.
  (B) 24 (6 cases of 4).
- **S5b-6 Precision spread.** (A) Keep `spread_mrad` 50 (a game value,
  triangular, ±30 m at 600 m) as the precision term; a sourced probable
  error (range and deflection) waits for the firing tables. It makes
  observed fire about 7-15 m mean miss after ranging. (B) Split into
  `range_pe_pct` / `deflection_pe_mrad` now.
- **S5b-7 Card ids.** (A) The 5-man squads keep the ids
  `ww2:ger_mortar_team` / `ww2:usa_mortar_team` (new names
  "Granatwerfergruppe" / "81mm Mortar Squad"), so the AI mix, saves and
  tests keep working; the 3-man teams are gone. (B) New ids and keep the
  3-man teams as understrength cards.
- **S5b-8 The bomb's burst on the cartridge.** (A) Stays on the weapon's
  `[he]` until a second bomb type (heavy HE, smoke) exists. (B) Move it now.
- **S5b-9 One fire-direction time.** (A) One `fire_direction_s` (10 s) for
  both factions (a voice-linked section); the per-faction doctrine values
  of artillery-on-map.md §4.8 (Germany 25, USA 15) are for guns in S11.
  (B) Per faction now.
- **S5b-10 Mortar prices.** (A) 5-man cards at the roster's 300, Germany
  with the S5a-37 cut (240); the load (20 bombs, about 70 / 64 MP at the
  shells price) is in the card. (B) Price the bearers' bombs on top.
- **S5b-11 First-round error.** (A) 3 % of range with a 10 m floor (the
  design's 3 % with a mortar-sized floor: 10-18 m at 200-600 m), so one
  or two adjusting rounds are usual. Observers' eye estimates in 1944
  were often worse (a few rounds more); a larger value changes the 3×
  ratio little but delays fire. (B) The design's 25 m floor.
- **S5b-12 The map-fire error.** (A) "About 3× the observed first round"
  (9 % / 30 m) is the starting value; the done-when's 3× is a whole-mission
  mean-miss ratio, which ranging pushes above 3× (Monte Carlo 3.4 for 8
  rounds), so the bench may set it lower (about 8 % / 27 m). (B) Keep 3×
  the first round and judge the ratio on first rounds only.

---

## Sources consulted in the Plan stage

Read on 2026-11-03; each content file cites its own. Secondary sources
are marked *verify* where they carry a number.

- US War Department, *German Infantry Weapons* (MID 1943), "8-cm Heavy
  Mortar, Model 34", lonesentry.com/manuals/german-infantry-weapons/german-8-cm-heavy-mortar.html:
  charges I-IV and their velocities, 60-1,900 m, firing table 869-1,545
  mils, rate, 24 rounds a crew in action, cases, bomb 3.5 kg.
- Wikipedia, *8 cm Granatwerfer 34* (174 m/s, 2.4 km, extra charges
  between the fins).
- US War Department, *Intelligence Bulletin*, Feb 1944, "Concentrating
  mortar fire" (German instructions), mobile.lonesentry.com/articles/intelligence-report/concentrating-mortar-fire.html:
  section as the fire unit, voice for one section, telephone for more,
  ranging on a registration point, fire for effect after ranging.
- Wikipedia, *M1 mortar*: 18 rpm sustained, 30-35 maximum; M43A1 3.2 kg,
  100-3,300 yd (zone 7); squad of 8 (leader, gunner, assistant, 5
  bearers); weights of tube, mount and base plate.
- lonesentry.com, *Italian 81-mm mortar* (US comparison: M1 18 rpm
  practical, light bomb to 3,288 yd).
- FM 23-90 (2000) / MCRP 3-15.2A, globalsecurity.org: bracketing and
  "split the bracket" (the later form of the WWII drill; the 1943 text
  was not found online, *verify*).
- An M43A1 dealer description (ima-usa.com): seven zones, ignition
  cartridge plus six increments (secondary).
- Not found and wanted for Part A: the M43A1 firing table (War Department,
  Jan. 1945), the German 1944 *Kriegsstärkenachweisung* for the
  Granatwerfergruppe (strength and loads).
