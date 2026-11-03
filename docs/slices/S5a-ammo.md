# Slice S5a-ammo: every round counted and priced

Branch `s5a-ammo` (from `s4-variants` at `b0374da`). Roadmap:
`docs/design/roster/roster-expansion.md` §4.2 "S5: Ammunition", "Owner
decisions (2026-11-01)" Q4 and D1-D4, Appendix A; detail in
`infantry-structures.md` §6, `artillery-on-map.md` §4.2 and §4.10 (the S5
rows only), `vehicles.md` §6.11, §7.1-§7.6.

S5 is split because of the weekly plan limit (owner-approved roadmap):

- **S5a (this slice):** counted rounds per man by ammo id; burst bundles;
  vehicle MG belts; sharing inside a squad; sidearm when dry; fire
  discipline (hold beyond about 70 % of range unless ordered, MG bursts
  with pauses); the `SupplySource` rule with the base as its only,
  unlimited, case; priced refill (D1) that pauses when the player cannot
  pay; tank stowage restored to historical counts, rare rounds and
  `rare_cap`; the rack price factor (D4); the Panzerfaust AI ambush hold
  (S4-42); AI ammo reserve and refill trips; the stall guard; StG 44 and
  the Volksgrenadier-Sturmgruppe; HUD rounds, "low" / "dry" markers and
  manpower spent on ammo.
- **S5b (not here):** mortar `FireMission` replacing `Barrage`, the
  observer rule, ranging-in, map fire at 3× scatter, true flight time from
  charges, 5-man mortar squads.
- **S6 (not here):** HQ, dumps, crates, forward rearm, `crate_only_classes`,
  the base no longer refilling mortar bombs. In S5a the base still refills
  mortar bombs directly, at the shells price.

---

## Baseline (measured 2026-11-02 on `b0374da`, release build)

Raw outputs: scratchpad `s5a/base_skirmish40.txt`, `base_armour_stress10.txt`,
`base_armour5.txt`, `base_fortify10.txt`.

### AI vs AI: 40 matches (seeds 1-20 × both base orders, `--jobs 3 --minutes 45`)

All 40 per-match hashes are equal to the S4 Fix list (`docs/slices/S4-log.md`
"Hashes"), so this is the S4 final state exactly.

| Measure | Value |
|---|---|
| Wins | Germany 16 (40 %), USA 24 (60 %), no draws |
| Wins by base | base 0 18 (45 %), base 1 22 (55 %) |
| Length | 12.3-27.5 min, mean 17.7, 3 over 25 min |
| HE share of kills | mean 43.7 %, max 55.2 % |
| Kills / losses | Germany 4356 / 4922, USA 4922 / 4356 |
| Tanks lost / bought | Germany 76 / 135, USA 81 / 132; 157 losses (rack 17 %) |
| Knock-outs gun / rocket | Germany 75 / 6, USA 65 / 11 |
| Rockets fired / hit / pen (matches) | Germany 46 / 35 / 30 (17 of 40), USA 47 / 31 / 30 (24 of 40) |
| Panzerfausts | 4 fired in 4 of 40 matches (3 normal-base), 4 hit, 3 pen, 1 knock-out; 50 hunts |
| Tank base returns (low ammo) | Germany 12 (0), USA 12 (0); MG dry 0.0 %; HE at reserve 0.0 % |
| AI rearm trips | 0 / 0 |
| Tick p99 | peak 1.049 ms, all 0.987 ms |
| Wall time | 63 s |

**Not in the summary today:** rounds fired per tank, per squad or per weapon
class; manpower spent on ammunition (nothing is priced); HE kills split by
source (mortar, tank HE, grenade, satchel). Part A step A1 adds these
counters first (report only), so the S4 reference for "rounds fired per
tank" is measured on outcomes identical to this baseline.

The wider faction picture (orchestrator, 480 matches on S4: seeds 1-80,
101-180, 201-280, both bases): Germany wins 40.4 %. The game leans US;
the Tune stage must land Germany at 45-55 % on 160.

### armour-test `--stress --seeds 10`

All hard checks pass (418 s; case 12 324.8 s). Rows that counted ammo can move:

| Case | Result |
|---|---|
| 3a / 3b / 3f duels | Panzer IV 10 / 3 / 14 of 20 wins; shots fired 22 / 26, 4 / 27, 30 / 40 |
| 4 AT team vs tank | flank disabling 46.4 %, front 41.1 %; rockets per run within the team's 6 |
| 5b tank on Suppress at a trench | 48,458 rounds (gun and MGs) over 20 runs, about 2,420 per 3-min run: under the Panzer IV's 3,150 MG rounds, so stowage does not bind yet |
| 6 tank vs rifles + AT | disabled 17 of 20; 69 rockets fired |
| 7 / 7b / 7c assault | attacker losses 137 / 220 / 195; defenders 112 / 1 / 90 of 220 |
| 9 tank HE vs infantry | trench 59 vs open 159 = 37.1 % (limit 40 %) |
| 14 stress | p99 1.101 ms, max 2.855 ms |
| 16 Panzerfausts | 40 m still 13 fired / 13 hit / 13 pen (disabled 7); 55 m still 15 / 15 / 14 (7); 35 m moving 7 / 7 / 6 (3); 55 m moving 0 |

### armour-test `--seeds 5` (S4-47)

**One hard check fails (known gap S4-47):** case 9, trench 33 vs open 82
men lost = 40.2 % (limit 40 %; 92 HE at the trench, 100 in the open: 0.36
vs 0.82 men per round). Every other hard check passes (209 s). The done-when
asks armour-test to pass at the default 5 seeds too, so S5a must close this
(B7, then Tune): find why 10-12-man squads lose about three times as many
men per tank HE round in the trench as the S3c 8-man squads (12.6 %), and
fix the cause (posture, spacing or the trench), not the limit.

### fortify-test `--seeds 10`

PASS (65 s).

| Row | Result |
|---|---|
| Rifles: trench vs open at 150 m | 0 vs 170 (0.0 %, limit 50 %) |
| 10-round 81 mm barrage | trench 46 vs open 136 = 33.8 % (limit 40 %); 10 bombs, inside the S5a team's 15 |
| Grenade assault from 25 m | defenders 166 lost with grenades vs 3 with rifles; attackers 170 of 220 |
| Smoke control | hidden side never seen or fired on; control seen and fired on |
| MG-supported advance (T10, report) | defenders 4 of 220 (1.8 %), attackers 225 of 330 (68.2 %): missed as in S4 (owner call S4-43) |

The rifle row runs 180 s; a Kar98k squad with 60 rounds a man and a
Garand squad with 136 do not run dry in it at their S4 fire rates
(15 and 24 rounds a minute at most), so the row should not move from
counted ammo alone; fire discipline (210 m hold) does not apply at 150 m.

---

## What exists today (code map): where rounds are fired, refilled or assumed infinite

Line numbers at `b0374da`.

| # | Place | File:line | Today | S5a |
|---|---|---|---|---|
| 1 | Small-arms shot | `crates/sim/src/combat.rs:651-683` `fire_round` | `Combat::mag` counts down; at 0 it refills to `w.magazine` from nothing (676-681) | The reload takes `min(magazine, carried)` from the man's `Kit::rounds` for the weapon's feed; with none carried the weapon is dry (crew feed, sidearm, sharing below) |
| 2 | Aim and fire | `combat.rs:509-631` `aim_and_fire` | Fires at any target up to `range_max`; MGs fire whole magazines | Fire discipline by the man's fire mode; MG bursts with pauses (`burst_rounds`, `burst_pause_ms`) |
| 3 | Spawn | `combat.rs:2619` `spawn_combat`; `game.rs:1296-1307` call-in kit; `bail.rs:661-680` crew kit | Full magazine; items by slot | Full magazine plus the slot's load (`Kit::rounds`), the load is in the card's price |
| 4 | Crew-weapon pickup | `combat.rs:1035-1100` `pick_up` (fresh belt 1087-1088) | The picker gets a full belt from nothing | The belt left in the gun goes with it (`WeaponDrop` keeps `mag`); further belts by crew feed |
| 5 | Pool full | `combat.rs:1296-1328` `spawn_rounds` | A small-arms request past the pool cap is dropped and counted (`Stats::shots_dropped`) | Unchanged: the round was taken from the man when requested, so a dropped round is still spent. There are no burst bundles in the code (the S1 §5.7 bundling was never built); the rule "a bundle subtracts every round it stands for" is a test on the request count and a code comment for whoever builds bundling |
| 6 | Mortar | `combat.rs:692-790` `fire_mortars`; AI `ai.rs:1370-1458` (`barrage_rounds` 6) | Unlimited bombs; the order ends after its rounds | Each bomb from the gunner's carried bombs, then crew feed from the crew within `crew_feed_m`; no bombs: the order ends ("no bombs" feed line). Base refill at the shells price (S5a only) |
| 7 | AT launcher | `combat.rs:1137-1290` `fire_launcher` (`l.rounds -= 1` at 1279); refill `items.rs:459-479` | Team count, one rocket per `items.resupply_s` in base, free | Unchanged count; base refill priced at `explosives` (rocket supply weight) |
| 8 | Panzerfaust | `combat.rs:857-1000` `fire_single_shot` (969); refill `items.rs:484-503` | Counted tubes per man, one per `items.resupply_s` in base, free (S4-6 (A)) | Same count; refill priced at `explosives`; AI ambush hold (below) |
| 9 | Grenades, smoke, satchels | `items.rs:270-280` throw; refill `items.rs:484-503` | Counted per man, refill free | Refill priced at `explosives` per item weight |
| 10 | Tank main gun | `gunnery.rs:480-489` load, `575-590` fire; `VehicleState::ammo` | Counted per ammo entry (Panzer IV 66, Sherman 70) | Historical stowage (87, 97); rare rounds and `rare_cap` |
| 11 | Tank MGs | `gunnery.rs:629` dry, `1060-1070` spent; `MgState::spent` | Counted (S3b Fix: 2 × 1,575; 2 × 2,375) | Unchanged count; feed id per weapon; refill priced at `small_arms`; bursts |
| 12 | Tank base rearm | `damage.rs:1037-1056` base step; `rearm_one` `damage.rs:1156-1176` | One main-gun round per 2 s and a belt per MG per step, free | Each round charged (shell price × rack factor; belts at small arms); rare rounds one per `supply.rare_rearm_base_s` (10 s) up to `rare_cap`; pauses when broke |
| 13 | AI tank rearm | `ai/armour.rs:25-60` `needs_rearm` / `rearmed`; `ai.rs:1584` | Trip when MGs dry or HE / AP at reserve; 0 trips in the baseline | Same trip; it waits while the refill is paused for lack of manpower only `ai.ammo.rearm_stall_s`; `ai.ammo.float_mp` kept back from buys while a unit waits |
| 14 | AI infantry | `ai.rs` plan steps 1-6 | No ammo state | Reserve, return to refill when low, staggered (below) |
| 15 | AI Panzerfaust hunt | `ai/assault.rs:811-857` `pzf_hunt` | Walks toward the tank into its escort's fire; 3 shots in 20 normal-base matches | Ambush hold first; the hunt stays as the fallback |
| 16 | Defeat | `rules.rs:200-230` | Wiped when no units and manpower < cheapest card | Unchanged; ammo spending can lower manpower, nothing goes into debt |
| 17 | Bridge | `snapshot.rs:859` (MG rounds left), `crates/godot-bridge/src/match_frame.rs`, `s4_ui.rs` | Vehicle main-gun rounds and MG rounds | Rounds per man and per crew weapon, low / dry, MP on ammo, refill paused |
| 18 | HUD | `godot/match/hud/detail_panel.gd` (squad card), `roster.gd` (strip cards), `end_screen.gd`, `top_bar.gd` | No ammo for infantry; MG stowage only in the vehicle tooltip | See "HUD" |

The existing low-ammo return (S3c B1) never fires in AI play (0 trips in
40 matches): tanks are lost before they run low. S5a raises stowage, so
that stays true for main-gun rounds; the new trips will mostly be
infantry and mortars.

---

## Data model

All values are TOML with a unit and a comment. Whole numbers, as every
other key (`docs/modding/tuning.md`).

### 1. Cartridges: a new def kind for counted rounds whose flight lives on the weapon

Small-arms rounds and (in S5a) mortar bombs have no ballistic def today:
their damage, speed and burst are on the weapon. They get a light def kind,
`defs/cartridges/*.toml`, so every counted round has an id, a supply kind
and a weight without inventing ballistics. Ballistic ammo defs
(`defs/ammo/`, tank rounds, rockets, the Panzerfaust round) get the same
`supply` table. Items (grenades, satchels, the Panzerfaust tube) get it on
the item.

```toml
# defs/cartridges/792x57_belt.toml
# 7.92x57 mm sS in 50-round belts (drums of 50, boxes of 300) for the MG 34
# and MG 42. Weight with its share of belt and box: a box of 300 weighs
# 11.53 kg, so 38 g a round (verify; secondary sources on the MG team's
# load). Riflemen's clip rounds are a separate id (S5a-2).
id = "ww2:792x57_belt"
name = "7.92 mm (belted)"
caliber_mm = 8
supply = { kind = "small_arms", g = 38 }
sources = ["..."]
```

```toml
# defs/cartridges/8cm_wgr34.toml
# 8 cm Wurfgranate 34 for the GrW 34. 3.5 kg (Wikipedia, 8 cm Granatwerfer 34).
# Kind "shells": in S5a the base refills it at shell_mp_per_t (D1, Q4);
# from S6 it leaves the base only in crates. The burst stays on the weapon's
# [he] table until S5b moves it here with FireMission.
id = "ww2:8cm_wgr34"
name = "8 cm Wgr 34"
caliber_mm = 81
supply = { kind = "shells", g = 3500 }
```

- `supply.kind`: `small_arms`, `explosives` or `shells` (the fourth kind,
  `construction`, waits for S6/S10 and is refused with "not before S6").
- `supply.g`: grams per round including its share of packaging (clip,
  belt, link, box, carton): what a crate or the base actually hands out
  (S5a-3). 1-100,000.
- `rare` (ammo defs only, default false): APCR, HVAP, HEAT where a vehicle
  or gun carries it (D3).

Content checks (actionable, with the file, the key and a fix):

- every weapon a soldier carries or a vehicle mounts in a mod that has a
  `[supply]` table names a `feed` (below): "ww2:kar98k fires counted
  rounds since S5a: add `feed = \"<cartridge id>\"` (cartridges of kind
  small_arms: ww2:792x57_clip, ...)";
- a vehicle main gun's or a gun's rounds must be kind `shells` (critic pass
  3: no cheap tank rounds);
- a `feed` names a cartridge or an ammo def that has `supply`;
- a cartridge or ammo def with `rare = true` is never a weapon's only feed.

The frozen S1-S3c fixture (`sdk/crates/content/tests/fixtures/ww2_s3c`)
has no `[supply]` table and no feeds: there every weapon stays unlimited
and every refill free, so its golden, mechanics and setup-bytes tests stay
equal (S5a-22).

### 2. Weapon keys

```toml
# defs/weapons/mg42_bipod.toml (additions)
feed = "ww2:792x57_belt"     # what each shot takes from the man's rounds
load = 50                    # rounds a man carrying it has when his slot gives no `rounds` (the drum in the gun)
# MG fire discipline: bursts of 5-7 rounds, then re-lay (German MG drill as
# summarised in secondary sources, verify). The pause also slows the
# effective rate to about 270 rounds a minute.
burst_rounds = 7             # rounds per burst (0 = fire the magazine through)
burst_pause_ms = 1000        # pause after a burst before the next (ms)
```

- `feed` (cartridge or ammo id). Launchers keep `ammo` (their ballistic
  round) and `rounds_carried` (the team count); their feed is `ammo`.
- `load` (rounds, default `magazine`): the per-man default. A `[[members]]`
  `rounds` table overrides it.
- `burst_rounds`, `burst_pause_ms` (default 0: no bursts). MGs and the BAR
  in S5a; SMGs keep magazine-length fire (their 30-round magazine and 3 s
  reload already make the pause; S5a-24).

### 3. Members: rounds per man by ammo id

S4 parsed `ammo = <rounds>` and ignored it (S4-20 (A)). S5a replaces it
with `rounds`, a table by cartridge or ammo id. A slot without it carries
his weapon's `load` (and his sidearm's).

```toml
[[members]]
slot = "mg_assistant"
weapon = "ww2:p38"
traits = ["crew_assistant"]
# Four 50-round drums and one 300-round box for the MG, plus his pistol's
# two magazines (secondary sources: assistant 4 drums and 1 box, third man
# 2 boxes; with the gunner's drum about 1,150 rounds, verify).
rounds = { "ww2:792x57_belt" = 500, "ww2:9x19" = 16 }

[[members]]
slot = "ammo_bearer"
weapon = "ww2:kar98k"
traits = ["ammo_bearer"]
rounds = { "ww2:792x57_clip" = 60, "ww2:792x57_belt" = 600 }
```

Rules for `rounds`: ids must be cartridges or ammo defs with `supply`; at
most 4 distinct feeds per squad type (`MAX_FEEDS`, the size of
`Kit::rounds`); each 0-65,535. The integer form `ammo = N` (S4) stays
accepted as "N rounds of the man's weapon's feed" so an S4 variant file
keeps loading; a file setting both `ammo` and `rounds` is an error.

### 4. Vehicles: stowage, rare rounds, `rare_cap`

```toml
# defs/vehicles/ger_pz4_h.toml (main gun; S5a)
# 87 rounds, as stowed in the Ausf. H (Jentz & Doyle, Panzer Tracts 4-3;
# Panzer Tracts 23 as quoted by armedconflicts.com). Split half PzGr 39,
# half Sprgr 34 (starting value, verify), and 3 PzGr 40 (tungsten rationed
# from 1943; vehicles.md §6.11, S5a-9).
rare_cap = 3                 # D3: most rare rounds held (rules rare_cap_max = 10)
ammo = [
  { id = "ww2:75mm_pzgr39", count = 42 },
  { id = "ww2:75mm_sprgr34", count = 42 },
  { id = "ww2:75mm_pzgr40", count = 3 },   # rare = true in its ammo def
]
```

- `rare_cap` (rounds): the most rare rounds the vehicle holds, all rare
  types together. Default: the rules' `rare_cap_max` when the def lists a
  rare round, else 0. Above `rare_cap_max`: "ger_pz4_h: rare_cap 12 is
  above the rules' supply.rare_cap_max (10); lower rare_cap or raise
  rare_cap_max in defs/rules/skirmish.toml". The rare `count`s must sum to
  at most `rare_cap`.
- Every shipped vehicle def sets `rare_cap` explicitly (Sherman: 0, the
  75 mm M3 had no HVAP).
- MG mounts keep `rounds` (S3b Fix); their feed comes from the weapon def
  (`mg34` 7.92 belt, `m1919` .30-06 belt).

### 5. Rules

```toml
# defs/rules/skirmish.toml (S5a additions)
[supply]
# D1 (owner, 2026-11-01): no free ammunition anywhere, the base included.
# Price per supply kind in manpower per tonne (whole numbers; 200 per t =
# 0.2 per kg). Charged round by round as it goes into a man's pouches or a
# vehicle's rack in the own base (S5a-1).
small_arms_mp_per_t = 200    # rifle, SMG, MG rounds, vehicle MG belts
explosives_mp_per_t = 500    # grenades, satchels, AT rockets, Panzerfausts
shell_mp_per_t = 1000        # mortar bombs (S5a only, base refill), tank main-gun rounds
# D4: a full rack refill costs at most this share of the card; the rack
# price factor is min(rack_rearm_price_pct, that ceiling) (critic pass 3).
rack_refill_max_card_pct = 75
rack_rearm_price_pct = 100   # fallback lever (%), applied to tank main-gun rounds
rare_cap_max = 10            # D3: most rare rounds any vehicle may hold
rare_rearm_base_s = 10       # D3: one rare round per this in base (standard: vehicles.base_rearm_s)
# Base refill rates per man in his own base (the base is the only source in S5a).
base_small_arms_rounds_s = 30   # rounds per second per man (one supply kind at a time, most depleted first)
base_bomb_s = 4                 # one mortar bomb per this per man
# items and rockets keep [items] resupply_s (20 s), now priced
low_pct = 30                 # HUD "low" marker and AI return threshold (% of the squad's load)

[combat.fire]
# Fire discipline (infantry-structures.md §6.2): in "discipline" mode a man
# opens fire only at targets within this share of his weapon's range_max;
# Suppress orders and "at will" fire to full range (S5a-4, S5a-18).
hold_beyond_pct = 70
hold_open_m = 30             # a man in "hold" mode returns fire at enemies this close

[combat.share]
interval_s = 5               # one sharing pass per squad this often
reach_m = 10                 # giver and taker this close
below_pct = 25               # a man below this share of his load takes ...
giver_above_pct = 50         # ... from the squadmate (same id) above this share, ammo bearers first
share_pct = 20               # ... this share of the giver's load per pass
crew_feed_m = 5              # a crew weapon reloads from a squadmate this close (assistant, bearer first)

[ai.ammo]
return_below_pct = 30        # a squad below this (its main feed) with no known enemy within threat_radius goes to refill
dry_pct = 5                  # below this it goes even under threat (as a retreat)
max_returning_pct = 34       # at most this share of the squads at one point leave to refill in one plan
refill_full_pct = 90         # leave the base at this share of the load ...
refill_max_s = 120           # ... or after this long
mortar_reserve_pct = 30      # a mortar barrages only above this share of its bombs, unless an own point is attacked
float_mp = 100               # kept back from unit buys while any own unit waits for a paid refill
rearm_stall_s = 15           # leave the base after this long with nothing served (no manpower)

[ai.assault]
# S4-42: Panzerfaust ambush hold (below).
ambush_watch_m = 200         # a known enemy vehicle this close to a point the squad holds (or to the squad) starts it
ambush_offset_m = 25         # cover spots this far to the side of the vehicle's projected path
ambush_max_s = 90            # give up after this long
```

Older rules files without `[supply]` load with every price 0 and no
feeds required (legacy, S5a-22). The new `[combat.*]` and `[ai.*]` keys
default to the values above when absent (as S4 did for its AI keys), so
older rules files load; `docs/modding/tuning.md` gains a row per table.

### 6. Setup layout (`sdk/crates/content/src/setup.rs`)

- `MatchSetup::cartridges: Vec<CartridgeSetup { id, name, kind, g, rare }>`;
  `AmmoSetup` and `ItemSetup` gain `supply: Option<SupplySetup { kind, g }>`
  and `AmmoSetup::rare`.
- `WeaponSetup` gains `feed: Option<Feed>` (`Cartridge(u16)` or
  `Ammo(u16)`), `load`, `burst_rounds`, `burst_pause_ticks`.
- `SquadTypeSetup` gains `feeds: Vec<Feed>` (≤ 4, in first-use order over
  the members) and per slot `MemberSetup::rounds: [u16; MAX_FEEDS]`
  (replacing `ammo: u32`), plus `feed_cap: [u16; MAX_FEEDS]` (the largest
  slot load of each feed: sharing never fills a man above his own load,
  load validation uses the cap).
- `VehicleSetup` gains `rare_cap`, `rack_price_pct` (computed at load:
  `min(rack_rearm_price_pct, floor(card × rack_refill_max_card_pct × 1e6 /
  full_rack_micro_mp))`, where `full_rack_micro_mp = Σ count × g ×
  shell_mp_per_t`), and per ammo entry `rare`.
- `RulesSetup` gains `supply: SupplyRules`, `combat.fire`, `combat.share`,
  `ai.ammo`, ambush keys. All part of the setup bytes, so the content hash
  and match hash change on purpose (`sdk/docs/net/relay-protocol.md` notes
  it).
- JSON Schemas (`frontline-cli schema`): `cartridge.schema.json` new; the
  weapon, item, ammo, vehicle, unit and rules schemas gain their keys.
  `docs/modding/` gains `ammo.md` (cartridges, feeds, member rounds,
  prices, `rare_cap`, errors, examples); `tuning.md` rows.

---

## Sim rules

### Counting, reload, crew feed, sidearm

- `Kit::rounds: [u16; MAX_FEEDS]` holds the rounds a man carries outside
  his weapon, by the squad type's feed slots. `Combat::mag` holds the
  rounds in the weapon. A man's total is `rounds[feed] + mag`.
- **Every shot is counted at the request.** `fire_round`, the vehicle MG
  shot, the mortar shot and the launcher shot decrement when they push the
  `FireRequest`. A request later dropped for a full pool is still spent
  (row 5). Any future burst bundle must subtract every round it stands for
  (test: rounds spent = requests made, including dropped ones).
- **Reload:** when `mag` reaches 0 the reload takes `min(magazine,
  rounds[feed])`. With none carried: a crew weapon (`crew = true`) first
  draws a magazine from a squadmate within `crew_feed_m` holding that feed
  (crew assistants and ammo bearers first, then ascending id), which is
  how the assistant fed the MG and the bearers fed the mortar (S5a-6).
  Otherwise the weapon is dry.
- **Sidearm:** a dry man whose slot has a `sidearm` with rounds switches to
  it (`Kit::on_sidearm`); his main weapon's empty `mag` stays 0. He
  switches back at his next reload once his main feed has rounds again
  (sharing or refill; S5a-23). The squad's focus range uses the weapon he
  holds.
- **Pickup (S4):** a crew weapon picked up from a fallen man keeps the
  rounds left in it (`WeaponDrop::mag`, new field); the fallen man's
  carried rounds are lost with him (no looting of bodies in S5a).
- Legacy (no feed): reload refills from nothing as today.

### Sharing inside a squad

Status stage, serial, ascending squad id, every `combat.share.interval_s`
(on ticks `tick % interval == squad_id % interval`, so squads spread over
the interval without state). For each feed slot: the man with the lowest
share of his own load below `below_pct` takes `share_pct` of the giver's
load from the squadmate within `reach_m` with the highest share above
`giver_above_pct` (ammo bearers first, then crew assistants, then the
rest; ties to the lower id), never above his own load (`feed_cap`). One
transfer per feed per squad per pass. Riflemen of a Gruppe carry no
belts in S5a: belts reach the gun only through the bearer and the
assistant (the slots that carry them).

### Fire discipline and fire modes

Per-man fire mode, `Unit::fire` (u8), set by the new command
`SetFire { units, mode }` (S5a-4):

| Mode | Opens fire at | Notes |
|---|---|---|
| `discipline` (default) | targets within `hold_beyond_pct` (70 %) of his weapon's `range_max` | Rifles 210 m, MG 42 tripod 420 m, MG 42 bipod 315 m |
| `at_will` | targets within `range_max` | the "unless ordered" case |
| `hold` | nobody, except an enemy within `hold_open_m` (30 m), or once a squadmate fired or was hit (the squad springs to `discipline`) | Single-shot AT items and launchers fire at `engage_moving` (Panzerfaust 60: 40 m) whatever the target's motion: the ambush |

A Suppress order fires at its line at full range in any mode except
`hold` (it is an order). Vehicle MGs follow `discipline` against
infantry; the main gun keeps its S3 gunnery rules. Mortars and
`Barrage` are unchanged (S5b). The AI uses `discipline` everywhere,
`hold` for ambushes.

**MG bursts:** after `burst_rounds` shots the weapon's cooldown is
`burst_pause_ticks` (on top of the interval); a reload also ends a burst.
State: `Weapon::burst` (rounds in the current burst) for soldiers,
`MgState::burst` for vehicle MGs. Expected cadence for the MG team target
(T5): an MG 42 bipod with 1,150 rounds fires 7-round bursts (0.3 s at
1,200 rpm) with 1.0 s pauses and a 5 s change per 50-round drum: about
14.3 s per drum, 23 drums, about 5.5 min of steady fire. The test computes
the expectation from the def with a pure function
(`combat::expected_fire_ticks(weapon, rounds)`) and checks the sim within
3 %.

### Base refill and payment (the `SupplySource` rule, base case)

One code path, `MatchState::supply_tick` (Status stage, serial), with a
`SupplySource` description that S6 reuses for HQ, dumps, crates and
vehicles: `{ stock: Unlimited | Kg([u32; 3]), radius, rates, priced: bool }`.
In S5a the only source is each player's base zone: unlimited stock,
priced, the base radius.

- **Who is served:** every own soldier and vehicle inside the own base
  radius (the base index of the owner, as today's resupply), every tick,
  at per-man rates; no client cap in the base (S5a-7).
- **Order:** players in slot order; their clients in ascending entity id;
  inside a man, the most depleted thing first (lowest share of its load,
  ties: small arms, then items by `ItemKind` order, then bombs, then
  rockets). Inside a vehicle: rare round on its timer, standard round on
  its timer, then MG belts.
- **Rates:** small arms `base_small_arms_rounds_s` (30) per man, carried
  as a per-man fraction in `Kit::refill`; items and rockets
  `items.resupply_s` (20 s, today's timing); mortar bombs `base_bomb_s`
  (4 s); tanks one standard round per `vehicles.base_rearm_s` (2 s), one
  rare round per `rare_rearm_base_s` (10 s) up to `rare_cap`, one MG belt
  per standard step (today's rate). Existing rearm tests keep their
  timings and gain a manpower check.
- **Price per round,** in micro-manpower (µMP, integers): `g ×
  kind_mp_per_t` (1 g at 1 MP per t is 1 µMP). Tank main-gun rounds,
  standard and rare: `× rack_price_pct / 100`. MG belts and everything
  else: full price. Examples at the start prices: a Kar98k round (28 g)
  5,600 µMP; a 60-round refill 0.34 MP; a grenade (595 g) 0.30 MP; a
  Panzerfaust 60 (6.1 kg) 3.05 MP; a mortar bomb (3.5 kg) 3.5 MP; a PzGr
  39 (11.52 kg) 11.52 MP × 53 % = 6.1 MP.
- **Payment:** `PlayerState::ammo.credit` (µMP, < 1,000,000 + one round)
  holds prepaid manpower. A round costing `c`: if `credit ≥ c` take it from
  the credit; else take `k = ceil((c − credit) / 1e6)` whole manpower if the
  player has it (`manpower -= k; credit += k·1e6 − c`), else **the refill
  pauses** for that client this tick (nothing else changes; the timer
  stays due, so it resumes the first tick the player can pay). Nothing goes
  into debt. The prepaid fraction is below 1 MP and kept for the next round
  (S5a-8). Paid µMP are added to `ammo.paid[kind]` (report, HUD); a paused
  tick adds to `ammo.paused_ticks` and sets the HUD flag "No manpower for
  ammo".
- **Rack only:** rounds go only into the client's own racks, pouches and
  launchers; nothing a vehicle or man holds ever moves to another
  vehicle, a stock or another squad (`rack_rounds_transferable = false`
  is the S6 key; S5a has no path that could move them).
- **Multiplayer:** the owner of the unit pays, always (team lobbies:
  each player pays for his own men; a man in an ally's base is not
  served, as today). A captured vehicle is refilled in its capturer's base
  at its capturer's prices (S5a-15).
- **Starting loads** come with the card's price. `load_price_check`
  (warning, raises a mortar card below its load at the shells price) runs
  at content load; both mortar cards pass (15 bombs, about 50 MP, cards
  280).

### Mortars in S5a

The gunner fires from his own bombs, then crew feed from the crew within
`crew_feed_m`. With none left the `Barrage` order ends and the feed says
"Granatwerfer 1: no bombs". The AI then sends the team to refill (below).
Today's 600 m range, scatter and `Barrage` stay (S5b).

### Tanks

Historical stowage (content), `rare_cap` (content), the priced and
rare-aware base rearm above. `rearm_one` keeps "most short standard entry
first" and skips rare entries (they have their own timer). The S3c AI
rearm trigger reads the same counts. Rounds fired per tank are reported
against the A1 reference (T8): priced rounds may make the AI hold fire
only through the AI's own rules; nothing in the sim makes a tank save
rounds.

---

## Save, hash and versions

`SAVE_VERSION` 12 → 13, once for the slice (at A1, the first state
change; later Part A and Part B fields join the same version before the
slice ships). `PROTOCOL_VERSION` 6 → 7, once, for `SetFire` (Part B, B1);
relay limits unchanged (`SetFire` carries at most `MAX_UNITS` ids like
`StopUnits`); `sdk/docs/net/relay-protocol.md` lists the command and the
content-hash change.

| New state | Where | Validated on load |
|---|---|---|
| `rounds: [u16; 4]`, `on_sidearm: bool`, `refill: u16` | `Kit` | each ≤ the squad's `feed_cap`; `on_sidearm` only with a slot sidearm; slots past `feeds.len()` are 0 |
| `burst: u8` | `ecs::Weapon` | ≤ the weapon's `burst_rounds` |
| `fire: u8` | `Unit` | < 3 |
| `mag: u16` | `WeaponDrop` | ≤ the dropped weapon's magazine |
| `burst: u8` | `MgState` | ≤ burst_rounds |
| `ammo: AmmoAccount { credit: u32, paid: [u64; 3], paused_ticks: u32, fired: FiredCounters }` | `PlayerState` | credit < 1e6 + the dearest round's µMP |
| `FiredCounters` (rounds by class: rifle, SMG, MG, pistol, mortar, rocket, Panzerfaust, tank AP / HE / rare, vehicle MG; refills by kind) | inside `ammo` | — (report) |
| AI: `TaskKind::Refill`, `TaskKind::Ambush`; `ambushes: BTreeMap<SquadId, Ambush { vehicle, since, spot }>` (≤ `MAX_HUNTS`); counters (refill trips, ambushes, ambush shots, rearm stalls) | `AiState` | lengths, ids live or dropped as hunts are |

Rare refill uses the existing `VehicleState::base_ticks` modulo its
period: no new vehicle field. The sharing pass is keyed by tick and squad
id: no new squad field.

Every field gets a hash test (`world.rs` pattern: mutate the field, the
hash changes) and a load-validation test (a bad value is refused).
Determinism: 1/2/3/8 threads for an AI-vs-AI match that runs dry and
refills, save → load → continue mid-refill and mid-burst, net loopback and
both soaks.

---

## AI

1. **Reserve and return.** A squad's ammo share = the lowest share of its
   main feeds (the crew weapon's total when it has one, else the median
   man's main weapon). Below `ai.ammo.return_below_pct` (30 %) with no
   known enemy within `threat_radius`, or below `dry_pct` (5 %) at all, it
   gets task `Refill`: Move to base (Retreat when under threat), wait until
   `refill_full_pct` (90 %) or `refill_max_s`, then back to its task.
   **Staggered:** per point, at most `max_returning_pct` (34 %) of the
   squads assigned to it start a refill in one plan, the lowest first; a
   point's last squad holds unless it is dry. This is the "send dry squads
   back in turns" rule of Appendix A.5.
2. **Mortar reserve.** A mortar barrages only while it holds more than
   `mortar_reserve_pct` (30 %) of its bombs, unless an enemy is within
   `threat_radius` of an own point in range; `barrage_rounds` is capped by
   the bombs it holds. Below the reserve with no target it goes to refill.
3. **Tanks.** S3c rearm trigger unchanged (it reads counts); in base the
   tank waits while it is being served; if it is served nothing for
   `rearm_stall_s` (no manpower) it leaves.
4. **Buys.** While any own unit waits in base for a paid refill, buys keep
   `float_mp` (100) back.
5. **Panzerfaust ambush hold (S4-42 / T12).** A squad whose men carry
   Panzerfausts, assigned to a point (or holding anywhere), sees a known
   enemy vehicle within `ambush_watch_m` (200 m) of the point or itself:
   - if it stands in a trench at the point, or the vehicle's projected path
     (its averaged velocity, else the line to the point) passes within
     `engage_moving` + `ambush_offset_m` of the squad, it stays;
   - else it moves to the best of up to 6 spots `ambush_offset_m` to either
     side of the projected path 40-80 m ahead of the vehicle: dug ground
     first, then a spot with no terrain line of sight from the vehicle now
     (one ray each), else the nearest; ties by position;
   - then `SetFire hold`, `SetStance prone` (crouch in a trench). The men
     do not fire, so the escort's MGs have nothing to shoot back at; the
     carriers fire at `engage_moving` (40 m) by the `hold` rule and the
     squad springs to `discipline` with the first shot;
   - it ends after the last tube, after `ambush_max_s`, when the vehicle
     is gone or farther than `ambush_watch_m`, or when the squad retreats;
     then `SetFire discipline` and its normal task. The S4 hunt
     (`pzf_hunt`) remains as the fallback for a stopped or stuck tank
     within `pzf_hunt_m` when no ambush applies.
6. **Stall guard.** All of the above returns squads to base before they
   are useless, in turns; income (240 per minute plus points) always
   exceeds what a full army spends on small arms, so a refill never waits
   long. The test: AI-vs-AI with every load cut to 10 % (a rules override
   in the test) must still end with a winner inside 40 minutes on seeds
   1-4 both bases, with refill trips > 0 for both sides and no squad
   waiting in base longer than `refill_max_s` + `rearm_stall_s`; plus a
   staged test where every man starts dry and both AIs must have fired
   again within 3 minutes.

Counters for the report: refill trips, mean squads away refilling, dry
man-time share, ambushes started / fired / knock-outs, rearm stalls.

---

## HUD (HUD v2 layout, military UI direction)

- **Squad card rows** (`detail_panel.gd`): each man's row ends with his
  rounds, `Kar98k 45`, a pistol switch shown as `P 38 12`; a crew weapon
  row shows the team's rounds for it, `MG 42 840` (gun plus assistant and
  bearer). A thin bar under the squad card header shows the squad's ammo
  share.
- **Markers** on the strip card (`roster.gd`) and the squad card: an amber
  "LOW" tag under `supply.low_pct`, a red "DRY" tag when the main feed is
  empty; vehicle cards get the same tags from main-gun rounds (rare rounds
  counted apart: `AP 20 HE 31 APCR 2/3`).
- **Refill:** "Refilling" on a card being served in base; a feed line and a
  top-bar flag "No manpower for ammo" while any refill is paused.
- **Manpower spent on ammo:** the top-bar manpower tooltip (this match, by
  kind) and an end-screen row per player: "Ammunition: small arms 34,
  explosives 21, shells 188 MP".
- **Fire mode:** the orders grid gains three toggles (Discipline / At
  will / Hold) in the existing order row style; a held squad shows a small
  "HOLD" tag.
- Contract: `godot/docs/match_state.md` "S5a" (per-man `rounds`, `feed`,
  `sidearm`; squad `ammo_pct`, `ammo_state` low / dry / ok, crew weapon
  rounds; vehicle `rare`, `rare_cap`; player `ammo_mp` by kind,
  `refill_paused`; intent `set_fire`). Godot tests in `test_match_view`
  (rows, tags, end-screen row, fire toggles; Settings detached as now),
  `test_data_reload` (F5 after editing a cartridge's weight and a member's
  rounds). `settings.cfg` checksum before and after.

---

## Reports (`skirmish --summary`, JSON)

New `S5a <faction>` lines: rounds fired by class (rifle, SMG, MG, pistol,
mortar bombs, rockets, Panzerfausts, tank AP / HE / rare, vehicle MG);
main-gun rounds per tank bought and MG rounds per tank, against the A1 S4
reference; small-arms rounds per squad by card; manpower spent on ammo
per kind (total and per match) and as a share of income; refill trips,
paused seconds, dry man-time share; ambushes and Panzerfausts fired in
normal-base matches; HE share of kills split by source (mortar, tank HE,
grenade, satchel, rocket/Panzerfaust), so "the HE share does not rise"
can be read per source.

---

## Content (Part A unless marked B), with sources

Weights include packaging (S5a-3); all are starting values, *verify*
where marked. The TOML comments carry the citations.

| Def | Values | Source |
|---|---|---|
| `792x57_clip` (Kar98k, small arms) | 28 g | complete round about 24-27 g plus clip and carton (*verify*) |
| `792x57_belt` (MG 34, MG 42) | 38 g | box of 300 rounds 11.53 kg; drum of 50 2.45 kg (secondary: skirmishmagazine.com MG team loads) |
| `9x19` (MP 40, P 38) | 15 g | about 12 g round plus carton (*verify*) |
| `792x33` (StG 44, B) | 20 g | 7.92×33 Kurz about 17 g plus carton (*verify*) |
| `3006_clip` (M1 Garand) | 32 g | bandolier of 48 rounds 3.4 lb (battleorder.org, "US ammo load WW2") |
| `3006_bar` (BAR magazines) | 38 g | full BAR belt 20.2 lb (battleorder.org) |
| `3006_belt` (M1919A4, tank M1919) | 35 g | 250-round belt 19.3 lb (battleorder.org) |
| `30_carbine` | 15 g | *verify* |
| `45acp` (M1911A1, M3) | 25 g | *verify* |
| `8cm_wgr34` (shells) | 3,500 g | Wikipedia, 8 cm Granatwerfer 34 |
| `81mm_m43a1` (shells) | 3,200 g | 6.87-7.15 lb (Wikipedia, M1 mortar; inetres.com 81 mm data) |
| `75mm_pzgr39` / `sprgr34` / `pzgr40` (ammo, shells) | 11,520 / 8,710 / 9,000 g; PzGr 40 `rare` | Wikipedia, 7.5 cm KwK 40 (complete rounds); PzGr 40 *verify* |
| `75mm_m61` / `m48` (ammo, shells) | 9,040 / 8,870 g | M3 gun data sheet, theshermantank.com (19.92 lb, 19.56 lb supercharge) |
| `rpzb54_heat` / `m6a3_heat` (explosives) | 3,250 / 1,590 g | *verify* |
| `pzf60_heat` item `panzerfaust60` (explosives) | 6,100 g (the whole tube) | Wikipedia, Panzerfaust (*verify*) |
| `stielhandgranate`, `mk2_grenade` | 595 g each | *verify* |
| `nbhgr39`, `m15_smoke` | 600 g, 880 g | *verify* |
| `geballte_ladung`, `m37_satchel` | 3,000 g, 9,100 g | *verify* |

**Per-man loads** (members' `rounds`, or the weapon's `load`):

| Man | Load | Source |
|---|---|---|
| Kar98k rifleman | 60 (two pouches) | German drill as commonly described (*verify*) |
| MP 40 leader | 192 (6 × 32) | feldgrau.net forum (secondary) |
| P 38 | 16 (2 magazines) | *verify* |
| MG 42 bipod team | gunner 50, assistant 500 (4 drums + 1 box), third man 600 (2 boxes): 1,150 | skirmishmagazine.com (secondary; totals vary 1,150-1,800) |
| sMG-Gruppe (MG 42 on Lafette) | gunner 150, assistant 600, 3 bearers 600 each: about 2,550 | *verify* (heavy MG section loads not found at the source) |
| M1 rifleman | 136 (belt 80, one bandolier 48, clip in the rifle) | battleorder.org (Army squad basis: belt, one bandolier, one clip) |
| BAR team | gunner 220 (11 magazines), assistant 240, bearer 240 BAR rounds plus their M1 rounds | battleorder.org (Army squad 35 magazines, 700 rounds) |
| M1 carbine | 75 (5 × 15) | battleorder.org |
| M3 SMG | 180 (6 × 30) | battleorder.org gives 300 for the Thompson unit of fire; 6 magazines kept (*verify*) |
| Light MG Section (M1919A4) | gunner 250, assistant 500, 2 bearers 500 each: 1,750 | battleorder.org (gunner 1 belt, assistant 2, bearer about 2) |
| Mortar teams (3 men in S5a) | gunner 3 bombs, 2 crew 6 each: 15 | starting value; S5b moves to 5 men with bearers |
| AT teams | 6 rockets (unchanged) | S3 |
| StG 44 (B) | 180 (6 × 30) | infantry-structures.md §6.2; a forum claim of 4 magazines (*verify*) |

**Vehicles** (stowage restored, S5a-10):

| Vehicle | Main gun | MGs | `rare_cap` |
|---|---|---|---|
| Panzer IV Ausf. H | 87: 42 PzGr 39, 42 Sprgr 34, 3 PzGr 40 (was 30 / 30 / 6) | 3,150 (2 × 1,575, unchanged) | 3 |
| M4A3 (75), dry stowage (S3b-4 (A)) | 97: 35 M61, 62 M48 (was 30 / 40) | 4,750 (2 × 2,375, unchanged) | 0 |

Panzer IV: 87 rounds and 3,150 MG rounds (Jentz & Doyle, *Panzer Tracts
4-3*; *Panzer Tracts 23* via armedconflicts.com; some sources give 3,192).
Sherman: 97 rounds dry, 104 wet (M4A3 (75) W) (strijdbewijs.nl M4 data;
historyofwar.org). Rack prices at the start values: Panzer IV full rack
about 877 kg, so `rack_price_pct` 53 (465 MP for a full refill of a 620
card, about 6.1 MP per PzGr 39); Sherman about 866 kg, 51 % (442 MP for a
590 card, about 4.6 MP a round).

**Part B content:**

- **StG 44** (`defs/weapons/stg44.toml`): 7.92×33 Kurz, 30-round magazine,
  about 550-600 rpm cyclic, `range_m` 250 (effective about 300 m in
  semi-automatic fire), bursts 3 rounds with 0.8 s pauses (starting
  values); `feed = "ww2:792x33"`. Sources: Wikipedia, StG 44; lonesentry.com.
- **Volksgrenadier-Sturmgruppe** (`defs/units/ger_vg_sturmgruppe.toml`):
  8 men, StG 44 for all (leader included), 2 Stielhandgranaten each
  (*verify*), conscript training, 300 MP, `cooldown_s` 45,
  `available_from = "1944-09"`, tag `line`, `ai_weight` 50 (S5a-14).
  Source: battleorder.org "Volksgrenadiers 1944" (K.St.N. 131V, 1 Nov
  1944; Sturmzug squads of 8 after the November reduction from 9,
  secondary).
- **Grenadiergruppe (Panzerfaust)** (roster table, 340 MP, 3 tubes): **not
  added** (S5a-12 (A)). The S4 Grenadiergruppe already carries two
  Panzerfaust 60s; S4 Tune showed 6 tubes per squad fired 7 shots against
  4, so tube count is not the limit; the ambush hold is.
- **M1919A4 tripod belt** 125 → 250 rounds (the real belt; battleorder.org)
  with bursts (S5a-11).

---

## Targets

Standard set: skirmish seeds 1-20 × both bases (40, `--jobs 3 --minutes
45`); faction decision on seeds 1-80 × both bases (160); armour-test
`--stress --seeds 10` and `--seeds 5`; fortify-test `--seeds 10`.

| # | Target | Baseline | How |
|---|---|---|---|
| T1 | each faction wins 35-65 % on 40; **45-55 % on 160** (Tune) | 40: G 40 %; 480 (S4): G 40.4 % | summary |
| T2 | no base side wins > 62 % | 55 % | summary |
| T3 | no match < 10 or > 40 min; median 15-30 | 12.3-27.5, median 17.2 | summary |
| T4 | HE share of kills does not rise: mean ≤ 43.7 %; max reported; split by source | 43.7 %, max 55.2 % | summary (the one-third cut is judged after S5b) |
| T5 | an MG team firing steadily goes dry in the expected time (±3 % of `expected_fire_ticks` from the def's rounds, rate, bursts and reloads) and refills in base paying Σ kg × `small_arms_mp_per_t`; with no manpower the refill pauses and resumes when income arrives | — | sim test `s5a_ammo.rs` |
| T6 | a tank rearming in base pays each round's kg × `shell_mp_per_t` × its rack factor (full rack ≤ 75 % of the card); never holds more rare rounds than `rare_cap`; a def above `rare_cap_max` fails to load with the actionable error | — | sim and content tests |
| T7 | a mortar with no resupply stops after its carried bombs (15); in base it refills at `base_bomb_s`, paying the shells price | unlimited | sim test |
| T8 | reported: manpower spent on ammo per kind; rounds fired per tank (main gun by type, MG) and per squad by card, against the A1 S4 reference | — | summary |
| T9 | Panzerfausts fired in ≥ 10 of 20 normal-base matches (S4 T12) | 3 of 20 | summary |
| T10 | stall guard: the low-load AI test ends with a winner < 40 min, refill trips > 0 both sides; the all-dry staged test fires again within 3 min; no match of the 40 set hits the limit | — | tests + summary |
| T11 | fire discipline: in `discipline` no small-arms round is fired at a target beyond `hold_beyond_pct` of range (staged test, hard; skirmish counter 0); `hold` fires nothing until sprung or within `hold_open_m` | — | tests |
| T12 | sharing and sidearm: a dry rifleman receives from a squadmate within `reach_m`; a dry MG reloads from its assistant (crew feed); a dry gunner with a P 38 fires it and switches back after a refill | — | tests |
| T13 | determinism: 1/2/3/8 threads, save → load → continue (mid-burst, mid-refill, mid-ambush), net loopback, soaks; every new field saved, hashed (test per field) and validated on load | pass | tests |
| T14 | fortify-test PASS; armour-test all hard checks pass at `--stress --seeds 10` **and at the default `--seeds 5`** (S4-47: case 9 ≤ 40 %); golden ballistics 46/46; `validate mods/ww2` ok | fortify PASS; armour 10 seeds pass (case 9 37.1 %); **5 seeds FAIL** (case 9 40.2 %) | as listed |
| T15 | tick p99 peak < 1.5 ms (`--jobs 3`); stress case p99 < 10 ms | 1.049 ms; 1.101 ms | summary; case 14 |
| T16 | the AI uses the new content and rules: VG-Sturmgruppe in ≥ 50 % of Germany matches; refill trips ≥ 1 per match per side (mean); mortar barrages limited by its reserve | — | summary |
| T17 | prices read right (content test against these hand figures, ±10 %): Grenadiergruppe full small-arms refill about 12 MP (58 kg, of it 8.7 MP the MG's 1,150 belt rounds); US Rifle Squad about 15 MP (75 kg: 11 × 136 Garand rounds and 700 BAR rounds); a Panzer IV's full rack ≤ 465 MP, a Sherman's ≤ 443 MP. (The design docs' "a rifle squad's refill about 5" was for the 8-rifle S3 squad without an MG.) | — | content test |
| T18 | HUD: Godot checks pass (`check_project`, `test_match_view`, `test_net_ui`, `test_data_reload`), screenshots at 1280 and 1600 looked at, F5 reload with a cartridge edit | 237 / 143 / 13 | Godot |

---

## Implementation order

### PART A: content + sim counting, supply, payment, tank stowage, `rare_cap`, saves, tests

1. **A1 Counters (report only), `SAVE_VERSION` 13.** `PlayerState::ammo`
   with `FiredCounters` (and the HE-by-source kill split) counted at every
   firing site of the code map, nothing else changed; summary lines.
   Measure the 40 set: every summary line except the hash equal to the
   baseline above; the new lines are **the S4 reference** for T4 and T8.
   Commit.
2. **A2 Content.** `cartridges` def kind, `supply` on ammo and items,
   `rare`, weapon `feed` / `load` / `burst_*` (bursts parsed, unused until
   B1), member `rounds` (with the S4 `ammo` integer form), `rare_cap` and
   the `rare_cap_max` check, `[supply]` rules, rack factor at load,
   `load_price_check`, the content checks of §1; setup layout (§6);
   schemas; `docs/modding/ammo.md`, `tuning.md` rows. The frozen S3c
   fixture's setup bytes, resolved snapshot and golden tests stay equal.
   Content: every cartridge, supply weights, per-man loads, both tanks'
   stowage and `rare_cap`.
3. **A3 Counting.** `Kit::rounds`, reload from carried rounds, crew feed,
   sidearm switch, `WeaponDrop::mag`, sharing, mortar bombs, vehicle MG
   feeds; spent at the request (dropped rounds included).
4. **A4 Supply and payment.** `supply_tick` with the base source; credit,
   pause, paid by kind, paused ticks; items, rockets, Panzerfausts, bombs,
   small arms and tank racks priced; rare refill on its timer up to
   `rare_cap`; `rearm_one` skips rare entries.
5. **A5 Saves and determinism.** Every new field hashed, saved, validated
   (a test each); 1/2/3/8 threads; save/load mid-refill; net loopback;
   soaks.
6. **A6 Tests for T5, T6, T7, T12, T17** (`crates/sim/tests/s5a_ammo.rs`,
   content tests); the request-count test for bundles.
7. **A7 Measure** with the AI unchanged: 40 set, armour (10 and 5 seeds),
   fortify, golden, validate, all tests, clippy, fmt. Expect squads to run
   dry and some matches to stall or lean: this is measured and written to
   the log, not tuned. If a fortify or armour hard check fails because a
   scenario squad ran dry, record it and keep counted ammo (no unlimited
   test mode; owner call if it persists). Commit.

### PART B: AI (discipline, reserve, ambush hold, stall guard), HUD, new content, Godot tests

1. **B1 Fire discipline and bursts.** `Unit::fire`, `SetFire` command
   (`PROTOCOL_VERSION` 7, relay doc), the three modes, Suppress override,
   MG bursts on soldiers and vehicle MGs, `Weapon::burst`, `MgState::burst`;
   T11 tests; `expected_fire_ticks` and T5 with bursts.
2. **B2 AI ammo.** Squad ammo share, `Refill` task, staggered returns,
   mortar reserve, tank rearm stall, `float_mp`; counters.
3. **B3 Panzerfaust ambush hold.** `Ambush` task and state, spot choice
   (≤ 6 rays per squad per plan), `hold` and `prone`, end rules, the hunt as
   fallback; armour-test case 16 gains an ambush row (a Grenadiergruppe at
   a point, a Sherman with an escort approaching: tubes fired, at what
   range, carriers lost); counters.
4. **B4 Stall guard tests** (T10).
5. **B5 Content.** `792x33`, StG 44, VG-Sturmgruppe, M1919A4 250-round
   belt, bursts on MGs and the BAR; sources in every file.
6. **B6 Bridge and HUD** (§HUD), `match_state.md` "S5a", Godot tests,
   screenshots, F5 reload.
7. **B7 Measure every target** (40 and 160 sets, armour 10 and 5 seeds,
   fortify, stress, tests, Godot) and write `docs/slices/S5a-log.md`. Look
   at S4-47 (why 10-12-man squads lose three times as many men per tank HE
   round in the trench) while measuring case 9 at 5 seeds. The Tune stage
   follows: faction 45-55 % on 160 with data only (loads, prices, bursts,
   card costs), HE share not above the A1 reference.

Risks: finite rounds plus bursts cut the MG 42's effective output more
than the M1919A4's (on a lean that already favours the US); the AI may
empty a front by sending everyone home (the staggering rule and T10);
priced refills move manpower from units to ammo (reported per kind);
more men consuming state per tick (a few bytes and one Status pass; T15).

---

## Owner questions (work proceeds on (A); also in `docs/TODO.md`)

- **S5a-1. Price keys.** (A) Whole-number keys per tonne
  (`small_arms_mp_per_t = 200`, `explosives_mp_per_t = 500`,
  `shell_mp_per_t = 1000`), as every other rules value is a whole number;
  the same prices as D1's 0.2 / 0.5 / 1.0 per kg. (B) Decimal keys per kg
  as written in the design docs.
- **S5a-2. Ammo ids by packaging.** (A) One cartridge id per cartridge and
  packaging (7.92 clip for rifles, 7.92 belt for MGs; .30-06 clip, BAR
  magazine, belt): riflemen cannot feed an MG with rifle clips, belts reach
  the gun through the slots that carry them. (B) One id per cartridge
  (any 7.92 round feeds any 7.92 weapon).
- **S5a-3. Supply weight.** (A) Grams per round including its share of
  clip, belt, box (what a crate hands out). (B) Bare round weight.
- **S5a-4. "Unless ordered".** (A) A per-man fire mode with a new
  `SetFire` command (discipline / at will / hold), `PROTOCOL_VERSION` 7;
  also the tool for the AI ambush and for human players. (B) No command:
  only a Suppress order fires beyond 70 %; the ambush holds fire through
  an AI-only squad flag.
- **S5a-5. Rockets.** (A) AT rockets stay a team count on the launcher
  (S3), priced per rocket. (B) Per man (gunner and loader).
- **S5a-6. Crew feed.** (A) A crew weapon reloads directly from a
  squadmate within 5 m (assistant, bearer first). (B) Only through the
  5-second sharing pass.
- **S5a-7. Base clients.** (A) The base serves every man in it at
  per-man rates (today's item behaviour). (B) A client cap (6 per 500 ms)
  as the HQ will have.
- **S5a-8. Payment granularity.** (A) Whole manpower is taken one at a
  time as prepaid credit; the fraction below 1 MP is kept for the next
  round (never lost, never debt). (B) Charge only whole-MP batches.
- **S5a-9. Panzer IV APCR.** (A) `rare_cap` 3 and 3 PzGr 40 (vehicles.md
  §6.11: tungsten rationed from 1943); was 6 (S3b-7 (A)). (B) Keep 6.
- **S5a-10. Stowage split.** (A) Panzer IV 42 PzGr 39 / 42 Sprgr 34 /
  3 PzGr 40 (87); Sherman (dry, S3b-4 (A)) 35 M61 / 62 M48 (97), no smoke
  (Q5 (A)). (B) Another split (a sourced loading list).
- **S5a-11. M1919A4 belt.** (A) 250 rounds (the real belt; was 125) with
  bursts. (B) Keep 125.
- **S5a-12. Grenadiergruppe (Panzerfaust) card.** (A) Not added: the S4
  Grenadiergruppe already carries two tubes and tube count was not the
  limit (S4 Tune). (B) Add it (340 MP, 3 tubes).
- **S5a-13. Panzerfaust 100.** (A) Not in S5a (late 1944, a later dated
  card). (B) Add it from 1944-11.
- **S5a-14. VG-Sturmgruppe in the AI mix.** (A) Tag `line`, `ai_weight`
  50. (B) A new `assault` tag with its own mix share.
- **S5a-15. Captured vehicles.** (A) Refilled in the capturer's base at
  his prices. (B) Never refilled.
- **S5a-16. Mortar bombs in S5a.** (A) 15 per 3-man team (gunner 3, crew
  6 each) until S5b's 5-man teams. (B) 18-20 now.
- **S5a-17. Tank MG belts.** (A) Small-arms price, no rack factor. (B)
  Under the rack factor too.
- **S5a-18. Hold-fire distance.** (A) 70 % of each weapon's `range_max`.
  (B) A per-weapon `effective_range_m` key.
- **S5a-19. HE-share target.** (A) S5a: the HE share must not rise above
  the A1 reference; the one-third cut is judged after S5b. (B) Judge the
  cut now.
- **S5a-20. AI shell budget.** (A) `ai.shell_mp_max_pct` waits for S6
  (crates); S5a has only `float_mp` and `rearm_stall_s`. (B) Build it now.
- **S5a-21. Faction balance levers (Tune).** (A) Data only: loads,
  bursts, prices, card costs; no new sim rule to reach 45-55 %. (B) Allow
  an AI rule change if data cannot reach it.
- **S5a-22. Legacy content.** (A) A mod without `[supply]` (the frozen S3c
  fixture, old mods) keeps unlimited rounds and free refill, so old tests
  stay exact. (B) Refuse such mods.
- **S5a-23. Sidearm back.** (A) A man returns to his main weapon at his
  next reload once it has rounds. (B) Only when ordered.
- **S5a-24. SMG bursts.** (A) None in S5a (magazine length and reload
  already pace them). (B) 4-5-round bursts for SMGs too.

---

## Tune results

Stage TUNE (after Part B, `9dd59ba`). Release build, `--jobs 3 --minutes 45`.
"40 set" = seeds 1-20 x both bases, "160 set" = seeds 1-80 x both bases,
"out of sample" = seeds 101-180 x both bases. Experiments were `--rules-override`
files or scratch copies of `mods/ww2` (`--mods DIR`), then the winning values
went into the TOML with their reasons in the comments. Raw outputs: scratchpad
`s5a/t/`. Part B left four targets missed: T1 (Germany 31 %), T9, T14
(case 9) and T10 (all-dry time). One round of data changes fixed T1 and T14;
T9 and T10 are not reachable with data (below).

### What changed (data only; one verbose diagnostic, test expectations)

| Where | Was | Now | Why (measured) |
|---|---|---|---|
| `[dig.common] parapet_top_mm` | 400 | 1400 | armour-test case 9 (S4-47), below |
| `ger_grenadiergruppe` cost | 280 | 225 | German balance price, below |
| `ger_grenadiergruppe_7` | 200 | 160 | same -20 % |
| `ger_smg_gruppe` | 195 | 155 | same |
| `ger_engineer_squad` | 250 | 200 | same |
| `ger_panzerschreck_team` | 180 | 145 | same |
| `ger_mortar_team` | 280 | 225 | same |

Unchanged on purpose: the owner's ammunition prices (200 / 500 / 1000 per
tonne), the VG-Sturmgruppe's 300 MP (owner spec), the US cards, tank costs,
loads, `[ai.ammo]` thresholds, discipline ranges (`hold_beyond_pct` 70),
bursts, `ai_weight`, the ambush keys.

Test-side edits that follow from the prices: `the_s4_content_resolves_with_historical_squads`
(cost expectations), `call_ins_are_refused_with_a_reason` (the 600 MP
rifle squad plus engineers now leave 175, so the sMG-Gruppe at 155 is
affordable; the short-of-manpower step uses the mortar team at 225), and
the canonical snapshot `ww2_canonical.json` (re-blessed: exactly the 7 changed
lines above). `armour-test --case 9 --verbose` now prints each man's last
state when he dies (position, stance, ducked, suppression).

### T1: the German balance (160 matches unless noted; Germany wins)

Where the 31 % came from. Part B on 160: **50** (31.3 %). Single switches
(160 each): fire discipline off 46, bursts off 51, refill trips off 49,
ambush off 40, VG-Sturmgruppe off 50; all five off together **70 (43.8 %)**.
Each switch alone is inside the noise (one 160-match count has a standard
error of about 6 wins; two near-identical configurations differed by 16);
together they cost about 13 points. What is left with them off (counted
rounds and priced refills only) is 43.8 %, close to S4 itself (40-43 %), so the
S4 lean was never closed and the S5a rules add to it. (Part B's "40-match
A/B" reading that discipline was the single largest cost was noise.)

What moved it (every row with the parapet at 1400 mm, so the parapet alone
is row 1):

| Change | Germany wins / 160 |
|---|---|
| none (parapet only) | 47 |
| German line cards -9 % (255 / 180 / VG 270) | 51 |
| ... plus the five other cards -10 % (mortar 250) | 73 |
| ... line -9 % and sMG / engineers / Panzerschreck -10 % (no mortar) | 54 |
| ... line -9 % and mortar 250 | 55 |
| all six German cards -5 % | 51 |
| all six -10 % | 57 |
| all six -15 % | 74; out of sample 70 |
| all six -20 % | 80; out of sample 87 |
| the four infantry cards -20 %, mortar and Panzerschreck unchanged | 66; out of sample 68 |
| -10 % and US rifle cards +5.5 % and the German MG team 1,500 rounds | 54; out of sample 73 |
| -10 % and `ai_weight` 25 on the 7-man and VG cards | 73 (the weights alone: 41) |

Price sweep (all six German cards, VG unchanged): 47 wins at 0 %, 51 at
5 %, 57 at 10 %, 74 at 15 %, 80 at 20 % (about 1.4 wins per percent, ±6
noise per point); 20 % was the centre. The mortar and Panzerschreck
cuts matter (the four-card cut stops at 42 %), as in S4 ("the LMG and
mortar did it"). The US-price and MG-load row did not beat the plain discount
and was dropped; the `ai_weight` row is noise.

Why a price and not a rule: the Kar98k's 15 rounds/min against the
Garand's 24 (and twelve men against ten) are real, and the S5a rules cost
Germany about as much as the US; a 20 % price cut is the S4 lever again
(S4 Tune: 320 -> 280 and so on) and the cost-balanced-realism order puts
prices before stats. The shipped values are rounded to 225 / 160 / 155 /
200 / 145 / 225.

Shipped result:

| Set | Germany | USA | Base 0 / base 1 |
|---|---|---|---|
| 40 set | **17 (42.5 %)** | 23 | 19 / 21 |
| 160 set | **73 (45.6 %)** | 87 | 77 / 83 |
| out of sample (101-180) | **80 (50.0 %)** | 80 | 84 / 76 |
| 160 + out of sample | 153 of 320 (47.8 %) | | 161 / 159 |

T1 is met on the 160 set, at the lower edge of 45-55 % (the 320-match mean
is the better estimate, 47.8 %, standard error 2.8 points). Pop at 5 / 10 /
20 min (160 set): Germany 66 / 74 / 88, USA 57 / 63 / 67 (it was 62 / 64 / 73
against 58 / 69 / 71): the cheaper cards give Germany the bigger army, as
in S4. Mortar bombs fired went up (Germany 28,023, USA 24,930 on 160).

### T14: armour-test case 9 at the default 5 seeds (S4-47)

What the cause is (`armour-test --case 9 --verbose`): every man who dies in
the trench is standing on the fire step (stance Stand, not ducked,
suppression 0 at death). The tank aims at the man who is up; the HE round
bursts on the parapet crest 1.4-1.9 m in front of him (z 20.3) and kills him
and one or two neighbours. A shell that lands elsewhere on the parapet kills
nobody. The ratio is therefore the share of rounds that hit the crest
in front of a standing man, about 0.4 men a round against 0.85 in the open,
whatever the squad size; the 12.6 % of S3c was a thin-margin result of the
eight-man squads. What did **not** help: the duck decay (25 -> 8 %), the
blast suppression behind cover (50 -> 100 %), fire discipline and vehicle MG
bursts (Part B), and the fire exposure (30 -> 25 / 20 / 15 cm give 30 / 22 /
17 of 84, but below 25 cm the trench riflemen can no longer fire over the lip:
fortify rifles row open losses 170 -> 70).

The lever: the parapet is a 40 cm hump (`parapet_top_mm` 400, total base 1.3
m), so the burst sits next to the head. Widening the flat top moves the
burst outwards and lets the bank stop fragments:

| `parapet_top_mm` | case 9, 5 seeds | case 9, 10 seeds |
|---|---|---|
| 400 (was) | 36 / 84 = 42.9 % | 74 / 164 = 45.1 % |
| 700 | 29 / 84 = 34.5 % | 57 / 164 = 34.8 % |
| 1000 | 30 / 84 = 35.7 % | 59 / 164 = 36.0 % |
| **1400 (shipped)** | **23 / 84 = 27.4 %** | **46 / 164 = 28.0 %** |

A real parapet is a bank of spoil about a metre thick (verify FM 5-15), and
the 1.8 m trench digs about 2.2 m2 of earth a metre against the bank's
0.6 m2. Side effects measured: fortify-test PASS (rifles row 0 vs 167 open;
barrage row 33.8 %, unchanged; T10 advance rows defenders 2.7 % / attackers
71.5 %, still the missed report line); no other armour-test row moved.

### Targets, shipped mod (R1)

All runs alone on the machine, `--jobs 3`.

| # | Target | Result | |
|---|---|---|---|
| T1 | 35-65 % on 40; 45-55 % on 160 | 40 set Germany 42.5 %; 160 set 45.6 %; out of sample 50.0 % | met |
| T2 | no base side > 62 % | 52.5 % (40), 51.9 % (160), 52.5 % (out of sample) | met |
| T3 | 10-40 min, median 15-30 | 40 set 10.4-30.2, median 17.2; 160 set 10.2-30.8, median 17.0; out of sample 10.1-32.2, median 16.5; none over 40 | met |
| T4 | HE share of kills <= 43.7 % | mean 27.6 % (40), 28.5 % (160), 29.0 % (out of sample); max 37.8 / 52.6 / 48.1 % (S4 max 55.2 %) | met |
| T5-T7, T12, T13 | tests | 519 tests pass, see "Tests" | met |
| T8 | reported | below | report |
| T9 | Panzerfausts in >= 10 of 20 normal-base matches | 40 set: 1 of 20; 160 set: 8 of 80; out of sample 6 of 80 (17 and 15 tubes fired) | **missed** |
| T10 | stall guard | no match near the limit (longest 30.2 / 30.8 / 32.2 min); low-load test passes; all-dry test fires again after 205 s, not 180 | partly (S5a-35) |
| T14 | fortify PASS; armour-test at 10 and 5 seeds | fortify PASS; `--seeds 5` all hard checks pass in 223.5 s (case 9 27.4 %); `--stress --seeds 10` all pass in 446.6 s (case 9 28.0 %) | met |
| T15 | tick p99 peak < 1.5 ms; stress < 10 ms | 40 set 1.058 ms; 160 set 1.495 ms (just inside); out of sample 0.510 ms; case 14 p99 1.512 ms | met |
| T16 | VG in >= 50 % of Germany's matches; refill trips >= 1 per match; mortar reserve | VG bought in 40 of 40 (66 cards) and 158 of 160; trips 11.9 (Germany) / 10.1 (USA) per match; reserve holds 0 (the 30 % refill trip always comes first) | met / not exercised |
| T18 | Godot | not re-run: Tune changed no code or Godot file (data and test expectations only) | n/a |

T8 (40 set, Germany / USA): main-gun rounds per tank bought 9.8 / 10.6 (AP
1.7 / 2.4, HE 8.1 / 8.2; S4 8.5 / 7.5), vehicle MG rounds per tank 277 /
199 (S4 572 / 292); manpower spent on ammunition per match 675 / 554 (small
arms 3.9 / 2.0, explosives 0, the rest mortar bombs and tank rounds; the
German figure is higher because the cheaper German mortar team is bought
more often); rifle rounds 57,429 / 104,086, MG 66,564 / 23,405, mortar
bombs 7,489 / 6,756 (the S4 reference: MG 137,119 for Germany). Kills by
source, 160 set, Germany / USA: mortar 23.0 / 19.0 %, tank HE 8.1 / 7.0,
rifle 43.7 / 57.0, MG 7.5 / 3.5, vehicle MG 10.5 / 7.3.

### T9: why the Panzerfaust miss stays (probes, 80 matches unless noted)

Part B started 461 ambushes in 40 matches and fired 6 tubes; the shipped
mod's 160 set has 1,836 ambushes and 17 tubes fired. Probes against that:

| Probe | Panzerfausts fired | Normal-base matches with one |
|---|---|---|
| shipped | 17 per 160 | 8 of 80 |
| `ambush_watch_m` 300 and `ambush_max_s` 150 | 5 | 4 of 40 |
| ambush off, `pzf_hunt_m` 200, `pzf_clear_m` 25 | 10 | 4 of 40 |
| `tank_standoff_m` 100 | 7 | 2 of 40 |
| `tank_standoff_m` 130 | 6 | 2 of 40 |
| six Panzerfaust carriers in the Grenadiergruppe instead of two | 6 | 2 of 40 |

More tubes, a longer watch, hunting without the ambush and shorter tank
standoffs all stay at 5-10 % of normal-base matches. Tubes are not the
limit and the keys do not reach it: the AI tanks open fire on infantry at
300-600 m and halt 160 m from points where a Panzerschreck or Bazooka team
was seen (the AT teams, not the Panzerfaust squads, mark "AT seen"); the
carriers die on the 100 m stalk or while waiting. Reaching 10 of 20 needs an
AI change (S5a-39), not data.

### T10: the all-dry recovery time

205 s against the plan's 180 s. A front 400-500 m from the base is 150 s
of walking at 3 m/s, so no threshold in `[ai.ammo]` moves it by 25 s;
the test keeps its 360 s allowance (S5a-35).

### Tests (R1)

Release `cargo test --workspace --no-fail-fast -- --include-ignored`: 519
passed, 0 failed (net loopback and both soaks included); golden ballistics 46/46; `validate mods/ww2` ok; clippy `-D
warnings` and `cargo fmt` clean. Determinism is unchanged by data (the
hash-equality and save/load tests pass on the new prices).

### Owner questions added in Tune

See `docs/TODO.md` S5a-37 to S5a-41; work proceeds on each (A).

---

## Sources consulted in the Plan stage

- [armedconflicts.com, Pz.Kpfw. IV Ausf. H](https://origin.armedconflicts.com/topic/view/6417) (87 rounds, 3,150 MG rounds, from Panzer Tracts 23)
- [Panzer IV, Wikipedia mirror](https://kiwix.lema.org/wikipedia_en_all_mini_2024-02/A/Panzer_IV)
- [strijdbewijs.nl, M4 Sherman medium tank](https://strijdbewijs.nl/tanks/sherman/m4eng2.htm) (97 dry, 104 wet)
- [historyofwar.org, Medium Tank M4A3](https://www.historyofwar.org/articles/weapons_medium_tank_M4A3_sherman_IV.html)
- [skirmishmagazine.com, German MG team loads](https://www.skirmishmagazine.com/?p=1161) (drums 2.45 kg, box of 300 11.53 kg; secondary)
- [battleorder.org, US ammo load WW2](https://www.battleorder.org/post/us-ammo-load-ww2) (M1 80 + 48 + 8, BAR 35 magazines per squad, M1919 belts 250 / 19.3 lb, carbine 5 × 15)
- [battleorder.org, Volksgrenadiers 1944](https://www.battleorder.org/volksgrenadiers-1944) (K.St.N. 131V)
- [Wikipedia, Volksgrenadier](https://en.wikipedia.org/wiki/Volksgrenadier); [Wikipedia, StG 44](https://en.wikipedia.org/wiki/StG_44)
- [feldgrau.net forum](https://www.feldgrau.net/forum/viewtopic.php?p=49402) (MP 40 6 × 32; StG 44 4 × 30; anecdotal)
- [Wikipedia, 8 cm Granatwerfer 34](https://en.wikipedia.org/wiki/8_cm_Granatwerfer_34); [Wikipedia, M1 mortar](https://en.wikipedia.org/wiki/M1_mortar); [inetres.com 81 mm](https://www.inetres.com/gp/military/infantry/mortar/81mm.html)
- [Wikipedia, 7.5 cm KwK 40](https://en.wikipedia.org/wiki/7.5_cm_KwK_40) (PzGr 39 11.52 kg, Sprgr 34 8.71 kg complete)
- [theshermantank.com, M3 gun data](https://www.theshermantank.com/wp-content/uploads/M3-gun-Data.pdf) (M61 19.92 lb, M48 19.56 lb complete)

Not read at the source (citation task carried from S4): K.St.N. 131 /
131V tables, TM-E 30-451, T/O&E 7-17, FM 23-55 (MG burst doctrine), the
German MG drill manuals; per-man loads marked *verify* above.
