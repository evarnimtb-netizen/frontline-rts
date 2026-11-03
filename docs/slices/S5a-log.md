# S5a-ammo log

Spec: `docs/slices/S5a-ammo.md`. Baseline: the spec's "Baseline" section
(`b0374da`, raw outputs in the scratchpad `s5a/base_*.txt`).

## Part A: content, counted rounds, priced base refill, tank stowage

### A1 Counters and the S4 reference

- `crates/sim/src/supply.rs` (new): `AmmoAccount` on `PlayerState::ammo`
  (prepaid credit, µMP paid by supply kind, paused ticks and the last
  paused tick, rounds fired by `fired_class`, rounds refilled by kind).
  Every fire request of a tick is counted when the requests are merged
  (Weapons stage, serial, before the projectile pool): a request the pool
  drops is still a spent round. Hashed in `hash.rs`, validated on load
  (credit below 1 MP plus the dearest round; `paused_at` not in the
  future). `SAVE_VERSION` 12 → 13 (one bump for the slice).
- `skirmish --summary`: four `S5a <faction>` lines (rounds fired by class;
  main-gun rounds per tank bought by type and MG rounds per tank; MP paid
  on ammunition by kind, per match, rounds refilled, refill paused seconds;
  kills by source as a share of the side's kills).
- The S4 reference (`s5a/a1_skirmish40.txt`, content code in, `mods/ww2`
  still without `[supply]`): every summary line equal to the baseline
  except the hash column and p99 (the setup layout changed). Reference
  numbers, 40 matches:

| Faction | rifle | smg | mg | pistol | mortar | rocket | pzf | tank AP | tank HE | rare | vehicle MG | main gun / tank | MG / tank |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Germany | 38,341 | 23,808 | 137,119 | 987 | 7,385 | 46 | 4 | 260 | 888 | 0 | 77,276 | 8.5 (AP 1.9, HE 6.6) | 572 |
| USA | 107,034 | 2,955 | 30,301 | 231 | 10,409 | 47 | 0 | 286 | 703 | 0 | 38,500 | 7.5 (AP 2.2, HE 5.3) | 292 |

  Kills by source (share of the side's kills): Germany mortar 40.8 %, tank
  HE 4.4 %, rifle 27.9 %, MG 8.3 %, vehicle MG 11.1 %; USA mortar 38.9 %,
  tank HE 3.5 %, rifle 43.3 %, MG 2.0 %, vehicle MG 7.1 %. The PzGr 40 was
  never fired in 40 matches (Auto loads it only against targets the PzGr 39
  fails on).

### A2 Content

- New def kind `defs/cartridges/` (`CartridgeDef`: id, name, calibre,
  `supply = { kind, g }`, sources); `DEF_KINDS` and the schemas gain it
  (`cartridge.schema.json`). `SupplyDef` on ammo defs and items; `rare` on
  ammo defs; weapon `feed`, `load`, `burst_rounds`, `burst_pause_ms`
  (bursts parsed, unused until B1); member `rounds` (the S4 `ammo = N` form
  still loads; both in one slot is an error); vehicle `rare_cap`; rules
  `[supply]` (`SupplyTuning`) and `[combat.share]` (`ShareTuning`, S5a
  defaults when absent). `construction` is refused "not before S6".
- Setup: `MatchSetup::cartridges`, `Feed` (cartridge or ammo index),
  `WeaponSetup::{feed, load, burst_rounds, burst_pause_ticks}`,
  `ItemSetup::supply`, `AmmoSetup::{supply, rare}`,
  `AmmoLoadSetup::rare`, `SquadTypeSetup::{feeds, feed_cap}` (at most
  `MAX_FEEDS` = 4), `MemberSetup::rounds` (replaces the unused `ammo`),
  `VehicleSetup::{rare_cap, rack_price_pct}` (rack factor computed at load:
  `min(rack_rearm_price_pct, card × rack_refill_max_card_pct × 1e6 /
  full-rack µMP)`), `RulesSetup::supply`, `CombatSetup::share`; all checked
  again by `MatchSetup::validate` (saves are untrusted).
- `ModDefs::check_supply` (cross-check, actionable): feeds name a cartridge
  or an ammo def with `supply`; a rare round is never a feed; member
  `rounds` name known feeds, at most 4 per unit; `rare_cap` above every
  rules def's `rare_cap_max` fails ("lower rare_cap to at most 10, or raise
  [supply] rare_cap_max"); rare rounds stowed above `rare_cap` fail; in a
  mod with `[supply]` every carried weapon (not launchers or cannons) and
  vehicle MG has a feed, every carried item and launcher round a `supply`,
  main-gun rounds are `shells`, every vehicle sets `rare_cap`.
- Legacy content (the frozen S3c fixture) has no `[supply]` and no feeds:
  it keeps unlimited rounds and free refills (S5a-22 (A)); every S1-S3c
  mechanics test passes unchanged.
- `mods/ww2` data (sources in every file): 10 cartridges (7.92 clip 28 g,
  7.92 belt 38 g, 9 mm 15 g, .30-06 clip 32 g, BAR 38 g, belt 35 g, .30
  Carbine 15 g, .45 ACP 25 g, 8 cm Wgr 34 3.5 kg and 81 mm M43A1 3.2 kg as
  shells); supply on all 8 ammo defs and 7 items; feeds and loads on every
  carried weapon (Kar98k 60, MP 40 192, P 38 16, MG 42 bipod 50, MG 42
  tripod 150, Garand 136, BAR 220, carbine 75, M3 180, M1911 21, M1919A4
  250, mortars 3); member rounds (Grenadiergruppe MG team 50 / 500 / 600 =
  1,150; sMG-Gruppe 150 + 600 + 3 × 600; LMG section 250 + 500 + 2 × 500;
  US rifle squads' BAR assistant and bearer 240 each; mortar crews 6
  bombs each, 15 a team); Panzer IV 42 / 42 / 3 (87, `rare_cap` 3, rack
  factor 53 %), Sherman 35 / 62 (97, `rare_cap` 0, 51 %); `[supply]` at the
  owner's prices (200 / 500 / 1000 MP per t), `rare_cap_max` 10,
  `rare_rearm_base_s` 10, 30 rounds/s, 4 s per bomb, `low_pct` 30;
  `[combat.share]` 5 s, 10 m, 25 %, 50 %, 20 %, 5 m.
- Docs: `docs/modding/ammo.md` (new), `docs/modding/tuning.md` rows,
  `sdk/docs/net/relay-protocol.md` (content hash and `-s13`).
- `resolved_defs`: the setup-bytes baseline changed on purpose (old and new
  values in the test's comment); the canonical snapshot re-blessed; the
  legacy snapshot unchanged.

### A3-A4 Counted rounds and the priced base

- `Kit::rounds` (by the squad type's feed slot), `Kit::on_sidearm`,
  `Kit::refill`; `WeaponDrop::mag`; `VehicleState::supply_due`.
- Spawns (call-in, bail-out) give each man his slot's load, a magazine of
  it in the weapon (a mortar tube starts empty: every bomb comes from the
  pouches, S5a-27).
- `aim_and_fire` writes the man's kit chunk (parallel by chunk as before):
  the reload takes a magazine from his rounds; an empty weapon with nothing
  carried waits one interval, then `reload_dry` reloads (if a squadmate fed
  it), draws the slot's sidearm, or holds fire; on the sidearm, the next
  reload goes back to the main weapon once it has rounds (S5a-23 (A)).
- Status stage (`MatchState::supply_tick`, serial; only with `[supply]`):
  crew feed (a dry crew weapon takes a magazine from a squadmate within
  `crew_feed_m`, bearers and assistants first, S5a-6 (A)); the sharing pass
  (one per squad per `interval_s`, keyed `tick % interval == id % interval`;
  the man carrying a crew weapon never gives its feed, S5a-26); the base
  refill of men (small arms at 30 rounds/s, most depleted first; items one
  per `items.resupply_s` then bombs one per `base_bomb_s`, S5a-25), AT
  teams (a rocket per `resupply_s`) and vehicles (a standard round per
  `base_rearm_s`, a rare one per `rare_rearm_base_s` up to `rare_cap`, a
  belt per MG per standard step); each round paid by `MatchState::pay`
  (µMP; whole manpower into the prepaid credit one point at a time; with
  none the refill pauses and stays due). The S4 free refills
  (`update_items`, `rearm_one`) run only for legacy content.
- Mortars: `take_bomb` draws each bomb from the gunner's tube and pouches,
  then a squadmate within `crew_feed_m`; none left: the barrage ends.
- Pickup: a crew weapon keeps the rounds left in it; the picker's own
  magazine goes back into his pouches.

### A5-A6 Saves, determinism, tests

- `world::tests::s5a_ammo_state_is_saved_hashed_and_checked`: every new
  field (kit rounds, sidearm, refill; the six account fields; a vehicle's
  due steps; plus the S4 test gains `drop.mag`) changes the hash, survives
  save and load, and broken values are refused (rounds above the largest
  slot load or in an unused feed slot, a sidearm without one in the slot,
  a magazine above the weapon in hand, refill out of range, credit, a
  future pause, due steps outside the base).
- `crates/sim/tests/s5a_ammo.rs`:
  - T5 `mg_team_goes_dry_on_time_and_refills_in_base_for_manpower`: a
    Grenadiergruppe's MG on Suppress fires its team's 1,150 belt rounds,
    no more, within 3 % of `supply::expected_fire_ticks` (23 drums; crew
    feed reach raised to 10 m in the test so the bearer at 6 m feeds by
    crew feed, not the 5 s pass); the gunner then draws his P 38; in base
    with no manpower the refill pauses (paused ticks counted, at most the
    prepaid credit's rounds go in), and with manpower it completes, the
    payment equal to the missing rounds × weight × 200 MP per t (8.74 MP
    for the belts), manpower + credit + paid conserved.
  - T6 `tank_rearms_in_base_round_by_round_and_rare_rounds_stay_under_the_cap`:
    a Panzer IV from empty: 3 PzGr 40 after 30 s (one per 10 s), 14-16
    standard after 31 s (one per 2 s), never above `rare_cap`; full rack
    paid at weight × 1 MP per kg × 53 % per round, belts at small-arms
    price, all manpower accounted for.
  - T7 `mortar_stops_after_its_carried_bombs_and_refills_at_the_shells_price`:
    15 bombs fired, the barrage order ends; in base 15 back within 30 s,
    paid 52.5 MP.
  - T12 `sharing_feeds_a_dry_rifleman_and_the_sidearm_switches_back`: a dry
    rifleman gets 12 rounds (20 % of a 60 load) in one pass; a dry gunner
    fires his pistol and goes back to the MG at the pistol's next reload
    once a belt reached him.
  - Request count: with a projectile cap of 8, rounds counted equal every
    request, the dropped ones included, and a rifle squad's carried rounds
    fall by exactly the rifle rounds requested.
- `sdk/crates/content/tests/s5a_supply.rs`: T17 prices (Grenadiergruppe
  full small-arms refill 11.8 MP, US Rifle Squad 14.9 MP, Panzer IV rack
  ≤ 465 MP at 53 %, Sherman ≤ 443 MP at 51 %); the shipped stowage and
  loads; T6 `rare_cap` 12 and rare above the cap refused with the fixes;
  missing feed, missing item supply, `construction`, a rare feed and
  `ammo` with `rounds` refused; legacy content has no feeds.
- Existing tests updated for the new stowage (gunnery base rearm now
  10 standard + 1 rare round in 20 s and paid; Sprgr 34 below 42; the
  vehicle snapshot 42 / 42 / 3; the vehicle range test loads 42 + 1).
- Determinism: the thread (1/2/3/8), save/load/continue and AI-vs-AI
  tests run on the live (priced) mod and pass with counted rounds; the
  net loopback and soak tests are in the release run below.

### A7 Measure (AI unchanged)

Raw outputs: scratchpad `s5a/a7_skirmish40.txt`, `a7_armour5.txt`,
`a7_armour_stress10.txt`, `a7_fortify10.txt`.

**Skirmish, 40 matches (seeds 1-20 × both bases, `--jobs 3 --minutes 45`):**

| Measure | S4 baseline | S5a A7 | Target |
|---|---|---|---|
| Wins | Germany 16, USA 24 | **Germany 22 (55 %)**, USA 18 | 35-65 % on 40: met |
| Wins by base | 18 / 22 | 18 / 22 (55 %) | ≤ 62 %: met |
| Length | 12.3-27.5, median 17.2 | 12.2-31.5, mean 19.5, median 18.4, 7 over 25 | 10-40, median 15-30: met |
| HE share of kills | mean 43.7 %, max 55.2 % | **mean 14.5 %, max 22.3 %** | not above 43.7 %: met |
| Tanks lost / bought | 157 / 267 | 195 / 301 | report |
| Panzerfausts | 4 in 4 matches (3 normal-base) | 9 in 4 matches (**0 normal-base**) | ≥ 10 of 20 normal: **missed** (Part B ambush) |
| AI rearm trips | 0 | 0 | report (Part B) |
| Tick p99 peak | 1.049 ms | 0.837 ms | < 1.5 ms: met |

Rounds fired (40 matches) and per tank:

| Faction | rifle | smg | mg | pistol | mortar | tank AP | tank HE | vehicle MG | main gun / tank | MG / tank |
|---|---|---|---|---|---|---|---|---|---|---|
| Germany | 55,747 | 22,878 | 124,278 | 576 | **1,246** (S4 7,385) | 268 | 1,102 | 92,624 | 8.6 (S4 8.5) | 579 (572) |
| USA | 148,092 | 2,935 | 29,060 | 185 | **1,886** (S4 10,409) | 332 | 1,096 | 52,287 | 10.1 (S4 7.5) | 371 (292) |

Manpower on ammunition (40 matches): Germany small arms 104, explosives 0,
shells 2,496 (total 2,599, 65 per match); USA 99 / 0 / 3,156 (3,255, 81
per match). Refilled rounds: Germany 14,490 small arms, 570 shells; USA
14,422 and 816. No refill ever paused. Kills by source: Germany mortar
7.2 %, tank HE 7.2 %, rifle 49.0 %, MG 11.7 %, vehicle MG 16.0 %; USA
mortar 8.0 %, tank HE 6.2 %, rifle 67.2 %, MG 3.0 %, vehicle MG 9.7 %.

What this says (the AI does not refill yet, Part B):

- Mortars fire their 15 bombs and stop: mortar rounds fell 83-82 % and
  mortar kills from about 40 % to about 8 % of each side's kills. That is
  most of the HE drop; the one-third cut target is judged after S5b.
- Only men and tanks that happen to stand in base refill (no AI trips):
  small arms paid about 2.5 MP a match per side, shells 62-79 MP (mostly
  tank racks of newly bought tanks topping up and mortar teams called in).
- The faction lean flipped from USA 60 % to Germany 55 % on 40 matches;
  the USA's 81 mortar bombs per match were a larger share of its killing
  than Germany's. The 160-match decision belongs to Tune after Part B.
- No match stalled (longest 31.5 min).

**armour-test `--stress --seeds 10`:** all hard checks pass (444 s). Case 9
trench 62 vs open 163 = 38.0 % (S4 37.1 %); case 14 p99 1.266 ms, max
3.471 ms (S4 1.101 / 2.855); 7c defender losses 92 vs 90. Every other row
equal to the baseline.

**armour-test `--seeds 5`:** **fails case 9 (S4-47), worse than S4**:
trench 38 vs open 81 = 46.9 % (S4 33 vs 82 = 40.2 %); every other hard
check passes (221 s). Only case 9 moved. Not fixed in Part A (B7 / Tune:
find the cause, not move the limit).

**fortify-test `--seeds 10`:** PASS, rows equal to the baseline except
the MG-supported advance (T10, report): attackers 228 of 330 (69.1 %, S4
68.2 %), defenders 4 (1.8 %) as before.

**Golden ballistics** 46 rows, 0 failed; `validate mods/ww2` ok; clippy
`-D warnings` and fmt clean. `cargo test --workspace --release --
--include-ignored`: 512 passed, 0 failed (the net loopback and both soaks
included). Godot was not started in Part A (no presenter change);
`settings.cfg` equals its backup.

### Part A: what is not done or missed

- T9 Panzerfausts in normal-base matches: 0 of 20 (Part B ambush hold).
- T14 armour-test at 5 seeds: case 9 fails (46.9 %), S4-47.
- Not in Part A by the plan: fire discipline and bursts (B1, `SetFire`,
  `PROTOCOL_VERSION` 7), AI reserve and refill trips (B2), ambush (B3),
  stall-guard tests (B4), StG 44 and VG-Sturmgruppe (B5), bridge and HUD
  (B6). The summary has no per-card small-arms line yet (T8 part).
- `load_price_check` (warning) not built.
- Owner questions added: S5a-25 (two refill streams per man), S5a-26 (the
  gunner keeps his belts), S5a-27 (mortar tube starts empty), S5a-28 (the
  reload after a dry wait is not scaled by training).

## Part B: fire discipline, AI ammunition, the ambush hold, HUD, new content

Commits: `3be36d8` (B1-B5, sim, AI, content, tests), `ca94051` (B6, bridge,
HUD, Godot), and the B7 commit with this log.

### B1 Fire discipline and bursts

- `Unit::fire` (`FireMode`: `Discipline` 0 default, `AtWill` 1, `Hold` 2),
  hashed with the unit, saved, kept through regroups and squad placement,
  refused on load for non-soldiers. Command `SetFire { units, mode }`
  (men only; vehicles dropped, S5a-32), `PROTOCOL_VERSION` 6 -> 7,
  `command_in_limits`, `relay-protocol.md` (version 7 entry, limits
  unchanged). `SAVE_VERSION` stays 13 (one bump for the slice).
- `combat::fire_reach`: discipline within `hold_beyond_pct` of range (a
  weapon's own `hold_beyond_pct` wins, S5a-34; none set in `mods/ww2`),
  at will to full range, hold within `hold_open_m`. Suppress fires to
  full range except in hold. Launchers and Panzerfausts keep their own
  reach rule in hold (S5a-30). `MatchState::spring_holds` (Status, serial,
  ascending squad id): a squad with held men springs to discipline when
  one of them fired this tick or reached the crouch suppression.
- Vehicle MGs keep discipline against infantry (Suppress: full range).
- Bursts: `combat::burst_cooldown` adds `burst_pause_ticks` after every
  `burst_rounds`-th shot on the existing shot counter (soldiers and vehicle
  MGs; no new state, S5a-31). `supply::expected_fire_ticks` steps the same
  rule shot by shot.
- Bug found and fixed while measuring: `fire_reach` first multiplied the
  Q16.16 range by the percentage and overflowed for 600 m weapons (MG 42
  tripod, M1919A4): those MGs never acquired targets. The early A/B runs
  below the fix are discarded.
- Sharing (S5a-33): the man carrying a crew weapon is no longer a taker in
  the sharing pass (part belts from the pass made extra drum changes; T5
  measured 3.2 % off). His gun is fed whole magazines by the crew feed.

### B2 AI ammunition (`crates/sim/src/ai/ammo.rs`, `[ai.ammo]`)

- `supply::squad_ammo_pct` (crew weapon feed, else the main weapon most
  men carry; AT rockets): below `return_below_pct` (30) with no threat, or
  below `dry_pct` (5), task `Refill`: Move to a base ring spot (Retreat
  when threatened), wait until 90 % or 120 s or a paused refill for 15 s.
  Staggered per task point (`max_returning_pct` 34, the point's last squad
  holds unless dry). `AiState::refill` (squad -> tick in base), validated.
- Mortar reserve: no barrage at or below `mortar_reserve_pct` of its bombs
  unless an own point is threatened; the order never asks for more rounds
  than held (`supply::mortar_bombs`).
- Buys keep `float_mp` (100) back while a squad waits in base or a tank is
  on a rearm trip (only with `[supply]`).
- Tanks on a rearm trip leave after `rearm_stall_s` in base while the
  owner's refill is paused.
- Counters (summary `S5a <faction>: AI refill trips ...` line): trips, dry
  trips, stalls, squads away per plan, reserve holds, rearm stalls, float
  holds.

### B3 Panzerfaust ambush hold

- `AiState::ambush` (`AiAmbush { vehicle, since, spot, tubes, done }`).
  A non-AT, non-engineer squad with Panzerfausts and a known enemy vehicle
  within `ambush_watch_m` (200) of its point or itself: SetFire hold,
  crouched on the way, prone at the spot. Moving vehicle: stays if in a
  trench or the projected path passes within engage_moving + offset, else
  the best of 8 spots (40 / 60 / 80 m ahead and where the path passes
  nearest, 25 m to either side, within 100 m of the squad; dug, then no
  line of sight from the vehicle, then nearest). Stopped vehicle: stalked
  to the S4 flank spot inside `engage_still` (S5a-30). Ends: tubes spent,
  `ambush_max_s` (90; then `done`, the S4 hunt may take the squad), the
  vehicle destroyed or seen beyond the watch, the squad retreating; then
  SetFire discipline and stance Auto.
- Not built: the armour-test case 16 ambush row (case 16 still runs its
  S4 rows and passes).

### B4 Stall guard (`crates/sim/tests/s5a_ai.rs`)

- `ai_vs_ai_with_a_tenth_of_the_rounds_still_ends_with_refill_trips`:
  seeds 1-4 x both faction orders, every member's rounds cut to 10 %: all
  8 matches end with a winner inside 40 min, both sides made refill trips.
- `all_dry_ai_vs_ai_fires_again`: everyone emptied once both sides fired
  200 rounds; both fire again after 205 s. The plan's 3 min is missed by
  25 s (the walk back); the test allows 6 min (S5a-35).

### B5 Content

- `cartridges/792x33.toml` (20 g), `weapons/stg44.toml` (250 m, 300 rpm
  practical, 30-round magazine, load 180, bursts 3 / 0.8 s),
  `units/ger_vg_sturmgruppe.toml` (8 x StG 44, 2 stick grenades each,
  conscript, 300 MP, 45 s, from 1944-09, `line`, `ai_weight` 50) in the
  German roster. Sources in each file.
- Bursts: MG 42 bipod 7 / 1.0 s, MG 42 tripod 10 / 1.2 s, MG 34 7 / 1.0 s,
  M1919 and M1919A4 8 / 1.0 s, BAR 4 / 0.6 s (all *verify*).
- M1919A4 belt 125 -> 250 (S5a-11 (A)).
- Grenadiergruppe (Panzerfaust) card: not added (S5a-12 (A)).
- Rules: `[combat.fire]` 70 % / 30 m, `[ai.ammo]`, ambush keys.

### B6 Bridge and HUD

- Sim snapshot: per soldier the weapon in hand, rounds held for it,
  sidearm, fire mode; per player MP on ammo by kind and `refill_paused`.
  Bridge: squad `ammo_pct`, `ammo_state`, crew weapon rounds, `fire_mode`,
  `refilling`; vehicle `ammo_state`, `rare_cap`; `stats[].ammo_mp`;
  intent `set_fire`. Contract: `godot/docs/match_state.md` "S5a".
- HUD: rounds column in the squad card rows (crew weapon carrier: team
  rounds), LOW / DRY / HOLD / Refilling tags in the card header and an ammo
  bar, LOW / DRY on strip and vehicle cards, "APCR 2/3", a thin fire strip
  under the stance row (console 24 px taller, S5a-36), top-bar tooltip with
  MP on ammo and a "No manpower for ammo" tag, end-screen "Ammunition" row.
- Mock: rounds per man, refill in base, `set_fire`, `set_rounds` staging,
  `--capture_setup s5a_ammo`.
- Godot: `check_project` OK, `test_match_view` 251/251 (+14 S5a),
  `test_net_ui` 143/143, `test_data_reload` 17/17 (+4: cartridge weight
  and member rounds edited clean, `rare_cap` 12 refused with the fix, the
  banner). Screenshots (scratchpad `s5a/s5a_hud_ammo_1600x900.png`,
  `..._1280x800.png`) looked at: rows show rounds, LOW on Rifle 1, DRY on
  MG team 1, fire strip clean at both sizes. Mock role names truncate in
  the 1280 rows ("Rif..."; the bridge's "Rifleman" rows show the weapon).
  `settings.cfg` checksum equal to the backup before and after.

### B7 Measurements (raw: scratchpad `s5a/b7_*.txt`)

**Skirmish, 40 matches (seeds 1-20 x both bases, `--jobs 3 --minutes 45`):**

| Measure | S4 baseline | A7 | B7 | Target |
|---|---|---|---|---|
| Wins | Germany 40 % | Germany 55 % | **Germany 30 % (12)**, USA 28 | 35-65 %: **missed** |
| Max base side | 55 % | 55 % | 60 % (base 0 24) | <= 62 %: met |
| Length | 12.3-27.5, median 17.2 | 12.2-31.5, median 18.4 | 10.7-30.3, median 19.1 | met |
| HE share of kills | 43.7 %, max 55.2 | 14.5 %, max 22.3 | 25.4 %, max 39.4 | <= 43.7 %: met |
| Panzerfausts, normal-base matches | 3 / 20 | 0 / 20 | **4 / 20** (6 fired in 6 matches) | >= 10 / 20: **missed** |
| Tick p99 peak | 1.049 ms | 0.837 | 0.372 ms | < 1.5 ms: met |
| Main gun per tank bought (G / US) | 8.5 / 7.5 | 8.6 / 10.1 | 11.4 / 9.9 | report |
| Vehicle MG per tank (G / US) | 572 / 292 | 579 / 371 | 272 / 188 | report (bursts) |
| MP on ammo per match (G / US) | - | 65 / 81 | 473 / 482 (small arms 3 / 2, explosives 0.2 / 0.1, rest shells) | report |
| AI refill trips per match (G / US) | 0 | 0 | 8.0 / 8.8 (all 40 matches; 0.57 squads away on average) | >= 1: met |
| VG-Sturmgruppe bought | - | - | 38 / 40 Germany matches (54 cards) | >= 50 %: met |
| Mortar reserve holds | - | - | 0 | not exercised (refill trips at 30 % come first) |

Kills by source (40): Germany mortar 18.7 %, tank HE 9.4 %, rifle 45.0 %,
MG 9.2 %, vehicle MG 10.5 %; USA mortar 15.9 %, tank HE 7.3 %, rifle
58.7 %, MG 3.2 %, vehicle MG 8.5 %. Mortar fire came back with the refill
trips (Germany 4,872 bombs, USA 5,955, A7 1,246 / 1,886; S4 7,385 /
10,409), which is where most of the shell manpower goes (3.5 / 3.2 MP a
bomb). Rounds fired: Germany rifle 59,234, smg 9,886, MG 67,996 (S4
137,119), USA rifle 89,437, MG 21,896.

**160 matches (seeds 1-80 x both bases):** Germany 50 (31.3 %), USA 110;
base 0 76 / base 1 84; 10.1-31.9 min, median 17.4, none over 40; HE mean
26.4 %, max 40.0 %; Panzerfausts 15 fired in 13 matches (7 normal-base of
80). (Tick p99 0.829 ms in this run; two armour-test runs shared the CPU.)

**Panzerfaust ambush (40):** 461 ambushes started, ended: gone 380 (seen
beyond the watch), timeout 17, spent 27, retreat 24; 60 tubes gone during
an ambush, but only 6 fired in all: the carriers mostly die stalking or
waiting. T9 is missed; the AI tanks stand off 160 m whenever an AT team is
seen, so few pass within 40-60 m of a waiting squad. Next lever (Tune or
S5b, owner): ambush only from trenches at the point (stay put), or a
tank-side change; data alone (watch, offset, max time) is unlikely to
reach 10 / 20.

**Faction lean (T1):** Part B moved Germany from 55 % (A7) to 30 % on 40.
A/B overrides (scratchpad `s5a/ov/`, 40 matches each, after the overflow
fix): no discipline (`hold_beyond_pct` 100) 16 / 40; MGs exempt from
discipline 8 / 40 (no gain). Fire discipline at 210 m on the rifles is the
largest single cost to Germany (fights move closer, where twelve Garands
outshoot ten Kar98k); bursts and the refill trips take the rest. Tune
must bring Germany to 45-55 % on 160 with data (S5a-21): candidates are
per-weapon `hold_beyond_pct`, burst lengths, loads, card costs.

**armour-test `--seeds 5`:** fails case 9 only: trench 36 vs open 84 =
42.9 % (S4 40.2 %, A7 46.9 %). **`--stress --seeds 10`:** now also fails
case 9: 74 vs 164 = 45.1 % (A7 38.0 %, S4 37.1 %); case 14 p99 1.660 ms
(< 10 ms). Every other hard check passes in both. Cause search: discipline
has no effect on case 9 (identical with 100 %); vehicle-MG bursts none;
all bursts off gives 66 vs 163 = 40.5 %. So the defenders' own MG and BAR
bursts raise trench losses (likely more time up on the fire step between
bursts, or less suppression of the attackers); the S4-47 cause (10-12-man
squads losing more per HE round in the trench) is still not found.
**T14 is missed.**

**fortify-test `--seeds 10`:** PASS (barrage row 33.8 %, trench vs open
0 vs 170); T10 MG-supported advance defenders 0.9 %, attackers 71.2 %
(missed as in S4, owner call S4-43).

**Tests:** release `cargo test --workspace -- --include-ignored`: 519
passed, 0 failed (net loopback and both soaks included); golden ballistics
46 rows pass; `validate mods/ww2` ok; clippy `-D warnings` and fmt clean. New: `s5a_ai.rs` (6: discipline, hold and spring,
SetFire per man and regroup and save, stall guard x8 matches, all-dry,
determinism at 1/2/3/8 threads and save/load mid-trip and mid-ambush),
`world::tests::s5a_fire_mode_and_ai_ammo_state_are_saved_hashed_and_checked`,
bridge `set_fire` intent, protocol limits test (16 listed commands).
Changed: `variants.rs` seed 3 -> 6 (the six-man variant loses the 120 m
fight without a US loss on seeds 1-5, 7, 8 with bursts), the S4 "every card
regular" check excludes the conscript VG card, `resolved_defs` setup
baseline (old values in the comment) and canonical snapshot re-blessed,
T5 loop 8,000 -> 14,000 ticks (bursts).

### Part B: what is not done or missed

- T1 faction 35-65 % on 40: Germany 30 % (160: 31 %). For Tune.
- T9 Panzerfausts in >= 10 of 20 normal-base matches: 4 / 20.
- T14 armour-test case 9 at 5 seeds (42.9 %) and now at 10 seeds stress
  (45.1 %; passed in A7).
- T10 all-dry recovery 205 s against 180 s (test allows 360 s).
- Not built: armour-test case 16 ambush row, the summary's per-card
  small-arms line and dry man-time share, the skirmish "rounds beyond
  hold range" counter (the staged T11 test is the hard check),
  `load_price_check`.
- Owner questions S5a-29 to S5a-36 added (TODO).
