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
