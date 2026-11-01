# Roster expansion: roadmap

Frontline RTS, `ww2` mod. Written 2026-11-01 for the owner.
Status: proposal. Nothing here is built. The repo was only read.

This document joins three area designs into one plan:

- `infantry-structures.md` (infantry variants, HQ, aid post, structures, ammo)
- `guns-artillery.md` (mortars, AT guns, infantry guns, howitzers)
- `vehicles.md` (armoured cars, half-tracks, trucks, more tanks and TDs)

All three are in the same folder as this file. They hold the detail:
TOML shapes, penetration tables, AI rules and test cases. This file holds
the decisions, the order and the questions.

Historical figures are starting values from memory. Each needs a
`sources = [...]` entry before it ships, as S3 required.

---

## 1. Summary

### 1.1 What we recommend

1. **Variants are data.** One pattern covers infantry, guns and
   vehicles. A unit card can build on another card (`extends`) and change
   only what differs: the men, their weapons, the vehicle or gun, the
   cost and the cooldown. The code for this is written once, in S4.
   After that most new units are one short TOML file.
2. **Count every round.** Rifle rounds, MG belts, mortar bombs, shells,
   grenades and rockets are carried by a man, gun or vehicle and run out.
   This is the historical brake on mortars, which today kill more men
   than rifles do (406 against 326 in S3's five logged matches; HE is
   about 40–50 % of all kills). It also gives the HQ a real job.
   **No ammunition is free** (owner decision D1): every refill costs
   manpower, cheap for small arms, moderate for explosives, dear for
   mortar, gun and tank shells.
3. **The HQ is a supply and command post, not a spawn.** It holds a
   finite stock of ammunition that starts empty. Men near it refill what
   they used. It
   issues ammo crates that men carry forward. Trucks and half-tracks
   refill it from base. It is the radio link for artillery calls and the
   place damaged squads fall back to.
4. **Build systems before vehicles and guns.** Most of the requested
   list (StuG, half-tracks, armoured cars, AT guns, howitzers) needs one
   new system each. Once that system exists, each further unit is data.
5. **Fix the balance problems before adding more HE.** Finite mortar
   bombs and an observer rule come before any new mortar, howitzer or
   infantry gun.
6. **Ship in twelve slices, S4 to S15** (S15 is the USSR). The first
   six (S4–S9) give a playable Germany and USA expansion: unit variants,
   ammo, the HQ and crates, assault guns and tank destroyers, half-tracks
   and trucks, and AT guns. Those six are about 5 times the size of S3;
   all twelve are about 10 times, before art (every new unit also needs a
   blockout model, markers and a HUD icon). Plan for it as a long
   programme, not one push.

### 1.2 What we push back on, and why

| Owner idea | Problem | Better version |
|---|---|---|
| "Stormtroopers" | *Sturmtruppen* and *Stoßtruppen* are WW1 terms (1916–18). | Germany: **Volksgrenadier-Sturmgruppe** (StG 44 section, autumn 1944) and **Sturmpioniere** (assault engineers). USA: **Assault Section** (the D-Day breaching team) and a **rifle squad with two BARs**. USSR: **avtomatchiki** (SMG section) and the **shturmovaya gruppa** of the assault engineer brigades. |
| Knives for melee | Trenches were cleared with grenades, SMGs, satchels and flamethrowers. The knife was a last resort, not a unit role. | No knife squads. A small physical strike (bayonet, rifle butt, spade, reach 2 m or less) comes last, after flamethrowers and trench clearing. |
| "Heavy engineers" who build bigger forts | Building power came from materials and machines, not heavier men. | Split engineers by **job**: field engineers (dig and build), **assault engineers** (breach wire, mines, bunkers), **construction engineers** (bunkers, obstacles, minefields). Big builds also need materials brought up and, later, a bulldozer. |
| HQ and medical points that heal | Healing does not break the "physical contact" rule (that rule is about damage). The problems are that an aura that tops men up is unhistorical and is an endless manpower loop: a man can be wounded, healed and sent back forever. | **Wound states** (fit, wounded, down, dead). Medics bandage and carry. An aid post works only while real medics staff it. A lightly wounded man returns once at 75 % health; a second wound means evacuation. The badly wounded are evacuated for a partial manpower refund. |
| HQ as a forward spawn | It undoes "spawn at base and walk", the main balance lever. Every match would become a hunt for the HQ. | Units and replacements always spawn at base. The only forward reinforcement that fits the rule is a truck or half-track that **carries** men from base. |
| HQ "spawns ammo crates" | Crates made from nothing at the front are a forward spawn for ammunition. An abstract "trickle" into the HQ is the same thing, only slower. | The HQ **issues** crates from a stock that runs out and starts empty. Trucks, half-tracks or crates carried by the player's men refill it. No trickle and no automatic carrying party (owner decision Q2 C). |
| HQ "helps units reload" | Reload speed is a man's hands and the weapon's mechanism. An aura is abstract. | Read it as **refill**: men inside the radius get back the rounds they fired. No reload-speed bonus. |
| On-map divisional howitzers | Authentic ranges (leIG 18 ≈3.5 km, 105 mm 7.6–11 km) exceed every map, so range cannot be the brake. Placed naively, a gun in base reaches everything with free shells. | **Changed 2026-11-01 at the owner's request: artillery is on-map, crewed and buyable** (`artillery-on-map.md`). One gun per card, two per player. The brakes are the historical ones: a linked observer's line of sight, finite shells (each gun has a ready inventory that refills only from crates, an HQ or a dump; shells crates cost manpower at base; owner decision Q4), counter-battery by sound and flash, set-up and tow times, masks and minimum range from the real charge tables, crews in the open, cost and caps. Off-map batteries are dropped from skirmish. |
| Light vehicles soon | Today rifle and MG rounds do nothing to any vehicle. An M8 or armoured car would be an unkillable infantry hunter. | Small arms against thin armour and open-top exposure ship **with or before** the first light vehicle. |
| Tiger first | 1,347 built, production ended Aug 1944; the hardest vehicle to balance. | The StuG III G first: the most common German AFV of the period. Its casemate system also gives the Jagdpanzer IV and 38(t). Heavies come last, after their counters. |
| 5 cm leGrW 36 as the German light mortar | Withdrawn from front-line companies around 1943. | The **8 cm kurzer GrW 42**. The US 60 mm M2 is genuinely current. |
| Fuel | Battles last 15–25 minutes over 1 km; nothing ran dry. The fuel crisis was strategic. | No fuel resource. The fuel tank stays as a part that can burn. |

### 1.3 Where the three designs disagreed, and what this plan chose

| Topic | Disagreement | Chosen |
|---|---|---|
| Slice names | The vehicle design used S3b, S3c, S3d. S3b is already the balance slice in progress. | Renumbered S4–S15 (section 4). |
| Fragment rays | The guns design asked for a fragment-ray slice (F0). | Already in S3b-balance (part A1). Dropped from this plan. S3b must land first. |
| APCR rounds | Both gun and vehicle designs need the `apcr` mechanism. | Already in S3b-balance (A6, PzGr 40). Reused. |
| Smoke rounds | Two designs proposed mortar and tank smoke as the trench-assault fix. | The owner ruled out mortar smoke for the S3b fix. **No smoke rounds in this plan** unless the owner says so (Q5). Smoke grenades stay. |
| Who pays for ammunition | Infantry and vehicles: cargo from base is free; transport is the cost. Guns: one per-player supply rate (kg per minute) for every restock. | **Owner decisions Q4 and D1 (2026-11-01): no ration, and no free ammo anywhere.** Every kg pays its kind's price once, when it leaves the base: small arms 0.2, explosives 0.5, shells 1.0 manpower per kg (`artillery-on-map.md` §4.2). S5 prices base refill and keeps today's compressed mortar range (600 m); we measure. From S6 the base refills no shells or mortar bombs; they leave the base only in priced crates or loads. Infantry, tanks and AT guns refill in base at the price, into their own racks only; rack rounds never move to a crate, dump or other weapon. The ZiS-3's HE is crate-only because it can fire indirect. Distance costs delivery, not a surcharge (D2). |
| Crates | By supply kind (any calibre) or by ammo class. | By **faction and kind**, counted in kg. A crate feeds only its own faction's calibres, so captured guns need captured crates. |
| Mortar crew | 3 men plus an optional bearer, or 5 men. | **5 men.** The two bearers carry the bombs; when they die, the mortar runs dry sooner. |
| Tank rearm at the HQ | Same rate as base, or half. | **Half the base rate** (1 round per 4 s), and the same at a dump or a supply vehicle (the vehicle design had 1 per 3 s; one forward rate is easier to read). Every round is priced (D1). **Rare rounds (APCR, HVAP, HEAT) refill forward too** (D3): 1 per 30 s forward, 1 per 10 s in base, never above the vehicle's `rare_cap` (at most 10). Repair and new crew stay base-only. |
| AT guns or vehicles first | Guns: AT guns early. Vehicles: casemates first. | Vehicle schema (S7) and trucks (S8) first, because a Pak 40 pushed by its crew takes about 48 min per km. Option to ship AT guns earlier with a minimal tow (Q11). |

---

## 2. The variant system

### 2.1 One pattern for every unit

Every unit the player calls in is built from three layers.

| Layer | What it is | Files |
|---|---|---|
| **Equipment** | Weapons, ammo, items, vehicles, guns. Authentic stats only. | `defs/weapons`, `defs/ammo`, `defs/items`, `defs/vehicles` (guns live here too) |
| **Men** | Who is in the unit: one slot per job, each with a weapon, ammo, items and traits. A training profile changes the men, never the weapons. | `[[members]]` in the unit file; `defs/training` |
| **Card** | What the player buys: cost, pop, cooldown, caps, dates, tags, capabilities, and the vehicle, gun or tow it brings. | `defs/units` (today's unit files become cards) |

A **variant** is a new card that `extends` an existing card and changes
one or two layers. Rules for the loader:

- `extends` works on every def kind. Chains go up to 4 deep. Cycles are
  an error.
- Tables merge; arrays replace. The resolved def is validated as a
  whole.
- Today's fields (`men`, `weapon`, `weapon_men`, `sidearm`, `loadout`)
  still load. They expand into member slots, so no file breaks.

**When a variant earns a card.** It must differ in a way the player can
see and use: a different job, weapon mix, vehicle or timing. "The same
squad with +10 % accuracy" is not a variant. If it is wanted, it is a
training profile.

### 2.2 Three examples

Infantry: an assault section built on the line squad.

```toml
# defs/units/ger_vg_sturmgruppe.toml
id       = "ww2:ger_vg_sturmgruppe"
extends  = "ww2:ger_grenadiergruppe"
name     = "@loc:unit.ger_vg_sturmgruppe"   # Volksgrenadier-Sturmgruppe
available_from = "1944-09"
training = "ww2:training_conscript"
tags     = ["infantry", "assault"]
cost     = 300
cooldown_s = 45

[[members]]
slot = "leader"
count = 1
weapon = "ww2:stg44"
ammo = { "ww2:792k" = 180 }
items = { "ww2:stielhandgranate" = 2, "ww2:nbhgr39" = 1 }
traits = ["leader"]

[[members]]
slot = "rifleman"
count = 7
weapon = "ww2:stg44"
ammo = { "ww2:792k" = 180 }
items = { "ww2:stielhandgranate" = 2 }
```

Gun: a crew card that brings a gun and its tow.

```toml
# defs/units/ger_pak40_squad.toml
id    = "ww2:ger_pak40_squad"
extends = "ww2:ger_gun_crew_6"      # 6 men, Kar98k, gun_crew capability
gun   = "ww2:ger_pak40"             # the gun entity (defs/vehicles)
tow   = "ww2:ger_sdkfz11"           # arrives hitched
cost  = 380                         # gun and crew; the tow is priced in its own def
cooldown_s = 90
tags  = ["at", "gun"]
```

Vehicle: a later tank built on an earlier one.

```toml
# defs/vehicles/usa_m4a3_76w.toml
id      = "ww2:usa_m4a3_76w"
extends = "ww2:usa_m4a3_75"
name    = "M4A3 (76) W"
available_from = "1944-08"          # verify
cost = 680
pop  = 12
cooldown_s = 120
rare_cap = 2                         # D3: HVAP was scarce; rules ceiling rare_cap_max = 10
[turret]                             # new turret box and plates
# ...
[[weapons]]
def  = "ww2:76mm_m1a1"
ammo = [{ id = "ww2:76mm_m62_apc", count = 35 },
        { id = "ww2:76mm_m93_hvap", count = 2 },
        { id = "ww2:76mm_m42_he", count = 30 }]
```

### 2.3 The card block (shared by every unit)

| Field | Meaning | Replaces today |
|---|---|---|
| `cost`, `pop` | Manpower and population | same |
| `cooldown_s` | Per-type wait after each call. The unit spawns at once at base. | `call_in_s` and the queue |
| `cooldown_group` | Which cooldown a call starts. Defaults to the root of the `extends` chain, so the full and understrength Grenadiergruppe share one timer and cannot be alternated to double the call rate. A card may name its own group. | — |
| `max_alive` | Cap for this type alive at once | — |
| `limit_group` | Shared cap, e.g. `heavy = 1`, `armour = 4`, `light = 3` (rules) | `[vehicles] limit = 4` (alive **or queued**) |
| `unlock_s` | Match time before the card can be bought | `[vehicles] unlock_s` (one value for all) |
| `available_from`, `available_to` | Date filter for period lobbies and missions | — |
| `tags` | AI buy mix and HUD filters (`line`, `assault`, `mg`, `at`, `engineer`, `mortar`, `gun`, `recon`, `transport`, `supply`) | `role` and `ai.mix_rifle_pct` style fields |
| `capabilities` | What the unit can do (`dig_basic`, `dig_full`, `build_structure`, `build_heavy`, `place_wire`, `clear_mines`, `breach`, `observe`, `radio`, `medic`, `gun_crew`, `issue_crates` …). Orders follow from them. | the hard-coded `SquadRole` enum |
| `training` | Profile: conscript, regular, veteran, elite | — |

### 2.4 Code written once

| Piece | Where | Size | Slice |
|---|---|---|---|
| `extends` resolver, cycle check, whole-def validation | content crate | S | S4 |
| Card block, call-in rework (immediate spawn, cooldowns, caps, groups, unlocks, dates) | content, `game.rs`, `world.rs`, `ai.rs`, HUD tiles | S–M | S4 |
| `[[members]]`: per-man weapon, ammo, items, traits; explicit pickup of a crew weapon when its carrier falls | content, `unit.rs`, `squad.rs`, `items.rs` | M | S4 |
| Capabilities and tags replace `SquadRole`; AI mix from a tag map | `orders.rs`, `ai.rs` | M | S4 |
| Training profiles (aim, reload, suppression, spotting, build rate) | `combat.rs`, `spotting.rs`, `dig.rs` | S–M | S4 |
| Generic vehicle schema: several armour boxes, open faces, optional turret, seats and mounts as lists, up to 4 ammo types | content `vehicle.rs`, `armour.rs`, `gunnery.rs`, `damage.rs` | M | S7 |
| Gun entity: a vehicle-style carriage and shield, crewed by ordinary soldiers in service slots | `vehicle.rs`, `unit.rs`, `armour.rs` | L | S9 |

After these, a new variant needs no code. New **mechanics** still do:
flamethrowers, transport, wounds, structures, the gun entity.

### 2.5 What we are not doing

- **No in-match veterancy** for now. It hides balance behind kill counts.
  Training profiles cover the need. A veterancy hook is one field later.
- **No Waffen-SS cards and no penal units** (Q15).

---

## 3. Roster for the first playable expansion

Germany and USA first. USSR is listed at the end as a plan only.

Costs are TOML starting values on today's scale: rifle squad 240–250,
Panzer IV H 640, Sherman 560, income 240 per minute plus 60 per held
point, pop cap 120. "Slice" is when the card arrives. Values marked
*(verify)* need a source.

Bigger, historical squads use more pop. A 12-man US squad fills a tenth
of today's pop cap. Q6 asks whether to raise the cap to 150. In big
lobbies the Battalion tier is the real limit: 700 soldiers shared by 10
players is 70 each, about five or six historical squads. The per-player
pop is therefore the lower of the rules `pop_cap` and the tier share.

"From" is the first month the card is offered in a dated lobby;
`1944-06` means from the start of the period.

### 3.1 Germany (Heer, 1944)

**Infantry and teams**

| Card | Men | Weapons | Job | Cost | Cooldown | From | Slice |
|---|---|---|---|---|---|---|---|
| Grenadiergruppe | 10 *(verify 9–10)* | MP 40 leader; MG 42 on bipod with 2 assistants; 6 Kar98k | Line squad built round its MG. Replaces today's 8 × Kar98k. | 300 | 20 s | 1944-06 | S4 |
| Grenadiergruppe (Feldstärke) | 7 | as above | Cheap understrength line squad | 220 | 20 s | 1944-06 | S4 |
| Volksgrenadier-Sturmgruppe | 8 | StG 44 for all | Assault at 50–150 m. Conscript training. A squad of a VG company's *Sturmzug*. | 300 | 45 s | 1944-09 | S5 (needs counted ammo) |
| sMG-Gruppe | 4 | MG 42 on Lafette 42 tripod | Sustained fire to 600 m+. Today's MG team. | 220 | 30 s | 1944-06 | S4 |
| Panzerschrecktrupp | 2 | RPzB 54 | Infantry AT | 200 | 30 s | 1944-06 | exists |
| Pioniergruppe | 8 | MP 40, Kar98k, satchels | Field engineers: dig and build | 280 | 30 s | 1944-06 | S4 (size) |
| Grenadiergruppe (Panzerfaust) | 10 | as line squad, 3 × Panzerfaust: the 30 (30 m) until 1944-08, the 60 (60 m) from 1944-09 *(verify months)* | Line squad with single-use AT. The Panzerfaust 30 was in wide use in Normandy, so the card is not late-only. | 340 | 45 s | 1944-06 | S5 |
| Granatwerfergruppe (8 cm s.GrW 34) | 5 | mortar; 2 ammo bearers | Battalion mortar, ~20 bombs carried | 300 | 45 s | 1944-06 | S5 (changed) |
| Munitionsträger | 2 | Kar98k | Carry crates forward | 60 | 20 s | 1944-06 | S6 |
| Zugtrupp | 3 | MP 40, radio *(verify: German rifle platoons rarely had a radio in 1944; a Kompanietrupp man with a portable set is the authentic carrier)* | Platoon HQ: builds the command post, radio link | 120 | 60 s | 1944-06 | S6 |
| Panzergrenadiergruppe (gp.) | 10 + SdKfz 251/1 | 2 MG 42 (one on the vehicle), Kar98k, MP 40 | Mechanised squad that rides its half-track | 300 + vehicle | 60 s | 1944-06 | S8 |
| Pak-Bedienung (7,5 cm Pak 40) | 6 + Sd.Kfz. 11 | Pak 40 | Concealed AT gun | 380 + tow | 90 s | 1944-06 | S9 |
| Pak-Bedienung (5 cm Pak 38) | 5 | Pak 38, pushable | Cheaper, second-line AT gun | 260 | 60 s | 1944-06 | S9 (data) |
| Sturmpioniergruppe | 8 | MP 40 / StG 44, *gestreckte Ladung* (Bangalore-type pipe charge), satchels (flamethrower added in S14) | Breach wire, mines, bunkers | 320 | 60 s | 1944-06 | S10 |
| Baupioniergruppe | 8 | Kar98k | Builds bunkers, obstacles, minefields fast. Weak in combat. | 200 | 60 s | 1944-06 | S10 |
| VB-Trupp | 2 | binoculars, radio | Forward observer for artillery | 100 | 60 s | 1944-06 | S11a |
| Granatwerfergruppe (kz. GrW 42) | 4 | 8 cm kurzer GrW 42 | Light, short-range mortar | 220 | 45 s | 1944-06 | S11b |
| Infanteriegeschützgruppe (leIG 18) | 5 | 7.5 cm leIG 18, pushed; a variant brings an Sd.Kfz. 11 | Direct and high-angle HE near the front; the only gun that drops shells behind a nearby crest | 320 | 90 s | 1944-06 | S11a |
| leFH 18 section (on-map) | 6 + Sd.Kfz. 11 | 10.5 cm leFH 18, one gun | Field howitzer; flat and direct fire on small maps; HEAT for emergencies | 480 + tow | 180 s | 1944-06 | S11b |
| Sanitätstrupp | 2 | pistol or none, stretcher | Medics, staff the aid post | 80 | 30 s | 1944-06 | S12 |

**Vehicles**

| Card | Role | Cost | Pop | Cooldown | Max alive | Unlock | Slice |
|---|---|---|---|---|---|---|---|
| Panzer IV Ausf. H | tank (stowage raised to 87, *verify*) | 640 | 12 | 120 s | 3 | 4:00 | exists; S5 |
| StuG III Ausf. G | assault gun (casemate) | 520 | 10 | 90 s | 3 | 4:00 | S7 |
| Jagdpanzer IV | tank destroyer (casemate) | 560 | 10 | 90 s | 2 | 5:00 | S7 |
| Jagdpanzer 38(t) | light TD, 41 rounds | 460 | 9 | 90 s | 2 | 4:00, from 1944-07 | S7 |
| SdKfz 251/1 Ausf. D | APC | 300 | 3 | 60 s | 2 | 2:00 | S8 |
| SdKfz 234/1 | recon car, screened open turret | 280 | 4 | 60 s | 2 | 1:30, from 1944-07 *(verify; production began Jun 1944)* | S8 |
| Opel Blitz | supply truck (3 t class) | 180 | 2 | 45 s | 2 | 0:00 | S8 |
| Sd.Kfz. 11 | gun tractor | 180 | 2 | 45 s | 2 | 0:00 | S9 |
| Panther Ausf. G | medium tank by German class (*mittlerer Panzer*); the strongest gun on the field | 1,100 | 16 | 240 s | 2, in the `armour` group | 8:00 | S13 |
| Tiger I | heavy tank, mostly 1944 by date | 1,250 | 16 | 300 s | 1 (heavy) | 12:00 | S13 |
| 12 cm GrW 42 | heavy mortar (gun entity, towed) | 340 | crew (6, *verify*) | 120 s | 1 | 8:00 | S13 |

Later or missions only: SdKfz 251/9 "Stummel", 251/7 engineers, 251/8
ambulance, 15 cm sIG 33, 15 cm Nebelwerfer 41 (Q23), 8.8 cm Pak 43, Puma, Panzer IV/70, Tiger II,
Panzer IV J (a strictly worse H in our balance: missions only).

### 3.2 USA (US Army, 1944)

**Infantry and teams**

| Card | Men | Weapons | Job | Cost | Cooldown | From | Slice |
|---|---|---|---|---|---|---|---|
| Rifle Squad | 12 | M1 Garand; BAR team (BAR, assistant); M7 rifle grenade launcher added in S11b (a new HE weapon, so it waits with the other HE) | Line squad (T/O&E 7-17). Replaces today's 8 × M1. | 340 | 20 s | 1944-06 | S4 |
| Rifle Squad (understrength) | 9 | as above | Cheap; typical by autumn 1944 | 260 | 20 s | 1944-06 | S4 |
| Rifle Squad, two BARs | 12 | as above + second BAR | More automatic fire, common field practice *(verify date)* | 360 | 30 s | 1944-08 | S4 |
| Light MG Section | 4 | M1919A4 on tripod | Company MG. Today's MG team. | 200 | 30 s | 1944-06 | S4 |
| Bazooka Team | 2 | M1A1 / M9 | Infantry AT | 180 | 30 s | 1944-06 | exists |
| Engineer Squad | 8 *(12 historical)* | M1, carbines, demolitions | Field engineers | 280 | 30 s | 1944-06 | S4 (size) |
| 81mm Mortar Squad | 5 | 81 mm M1; 2 ammo bearers | Battalion mortar | 300 | 45 s | 1944-06 | S5 (changed) |
| Ammunition Bearers | 2 | M1 | Carry crates forward | 60 | 20 s | 1944-06 | S6 |
| Platoon HQ | 3 | M1, carbine, SCR-536 | Sites the CP, radio link | 120 | 60 s | 1944-06 | S6 |
| Armored Infantry Squad | 12 + M3A1 | M1, BAR, bazooka in the vehicle | Mechanised squad | 340 + vehicle | 60 s | 1944-06 | S8 |
| 57mm Antitank Squad | 6 + M3A1 or 1½-ton truck | 57 mm M1 (HE only from late 1944, *verify*) | AT gun | 340 + tow | 90 s | 1944-06 | S9 |
| Assault Section | 12 | M1, BAR, bazooka, Bangalore, pole charges (M2-2 flamethrower added in S14) | Breaching fortified lines | 380 | 60 s | 1944-06, West | S10 |
| Forward Observer Party | 3 | carbines, radio | Calls artillery. Shorter fire-direction time (a doctrine trait). | 120 | 60 s | 1944-06 | S11a |
| 60mm Mortar Squad | 5 | 60 mm M2 | Company mortar, many light bombs | 240 | 45 s | 1944-06 | S11b |
| Cannon Company Howitzer Section | 6 + tow *(verify: 1½-ton or 2½-ton truck)* | 105 mm M3 | Regimental howitzer; low-angle only (max +30°, verify), so direct and flat indirect fire | 380 + tow | 120 s | 1944-06 | S11a |
| Field Artillery Section (on-map) | 8 + CCKW | 105 mm M2A1, one gun | Field howitzer; low-angle on Battalion maps; HEAT for emergencies | 500 + tow | 180 s | 1944-06 | S11b |
| Aid Men | 2 | unarmed (US practice) | Medics | 80 | 30 s | 1944-06 | S12 |

**Vehicles**

| Card | Role | Cost | Pop | Cooldown | Max alive | Unlock | Slice |
|---|---|---|---|---|---|---|---|
| M4A3 (75) | tank (stowage per S3b-4) | 560 | 12 | 120 s | 3 | 4:00 | exists |
| M10 | TD, open-top turret, hand traverse | 500 | 10 | 90 s | 3 | 4:00 | S7 |
| M18 Hellcat | TD, fast, thin armour | 520 | 10 | 90 s | 3 | 4:00, from 1944-07 | S7 (data) |
| M3A1 half-track | APC, .50 ring mount | 300 | 3 | 60 s | 2 | 2:00 | S8 |
| M8 Greyhound | recon car, open turret | 280 | 4 | 60 s | 2 | 1:30 | S8 |
| M20 | utility car, ammo carrier | 220 | 3 | 60 s | 2 | 1:30 | S8 |
| GMC CCKW 2½-ton | supply truck, tow | 180 | 2 | 45 s | 2 | 0:00 | S8 |
| M4A3 (76) W | tank, 2 HVAP (HVAP from about Sep 1944, very scarce; `rare_cap` 2) | 680 | 12 | 120 s | 3 | 6:00, from late summer 1944 *(verify month)* | S13 |
| M36 Jackson | TD, 90 mm, from 1944-09 | 760 | 12 | 180 s | 2 | 8:00 | S13 |
| M4A3E2 "Jumbo" | assault tank, 254 built | 900 | 14 | 240 s | 1 (heavy) | 10:00, from 1944-10 | S13 |
| 4.2-inch Chemical Mortar Squad | attached heavy mortar (HE; WP only if Q5 allows) | 340 | crew (*verify*) | 120 s | 1 | 8:00 | S13 |

Later: M4A1 (76) W (the first 76 mm Sherman in action, Jul 1944, named in
SYSTEM_DESIGN §2.3; it has a cast hull, so it is a new hull, not an
`extends` of the M4A3), M4A3E8 and M24 (Dec 1944), M5A1, 75 mm pack
howitzer (airborne), 3-inch M5 towed gun (emplaced only), M16 MGMC.

### 3.3 USSR (plan only, after the Germany/USA expansion)

No new systems are needed by then, apart from three small ones (body
armour plates, captured-ammo rules, and tank riders: S8 passenger seats
placed on a tank's engine deck, fully exposed, with a dismount when the
tank is hit or fires). The faction is mostly data, plus art.

| Group | Cards |
|---|---|
| Infantry | Strelkovoe otdelenie (9, DP-27 team) and a field-strength version; Gvardeyskoe otdelenie (a training variant); Otdelenie avtomatchikov (PPSh); Shturmovaya gruppa (ShISBr assault sappers with SN-42 breastplates and ROKS-3); Sapyornoe otdelenie; Maxim or SG-43 crew; PTRD or PTRS team; sniper pair; medics |
| Mortars and guns | 82 mm (can fire captured German 8 cm bombs); 120 mm PM-38; 45 mm M-42; 57 mm ZiS-2 (not requested, but the best Soviet AT gun of 1944); 76 mm ZiS-3; 76 mm regimental gun M1943; 122 mm M-30 section (on-map); BM-13 only if Q23 allows |
| Vehicles | T-34-85 **with tank riders** (the authentic Soviet transport, not half-tracks); SU-85 / SU-100 by date; SU-76M; BA-64B; IS-2; Studebaker US6; M3A1 scout car |

---

## 4. Slices in order

### 4.1 The order at a glance

Prerequisite: **S3b-balance** (in progress). It brings fragment rays, the
`apcr` mechanism, the Suppress order and the duck state. Everything below
assumes it has landed.

| Slice | Name | Size vs S3 | Depends on | Value |
|---|---|---|---|---|
| S4 | Variants and call-in | ~0.7× | S3b | High: most infantry variants as files; owner's no-queue rule |
| S5 | Ammunition | ~1.0× (split S5a counts and base supply, S5b mortar fire missions, if it runs long) | S4 | High: fixes mortar dominance; makes supply matter |
| S6 | HQ and ammo crates | ~0.5× | S5 | High: the owner's HQ request |
| S7 | Assault guns and tank destroyers | ~0.9× | S4 | High: the most common German AFV; new vehicle schema |
| S8 | Half-tracks, trucks and recon | ~1.1× (largest) | S6, S7 | High: transport, supply vehicles, light armour |
| S9 | AT guns | ~0.9× | S8 (tows) | High: the cheap, concealed tank counter |
| S10 | Fortifications and breaching | ~1.5× (split S10a wire, sandbags, MG nest, dugout and their breaching; S10b mines, AT obstacles, bunker) | S6 | High: the trench pillar and the engineer split |
| S11 | Artillery on the map | ~1.3× (S11a ~0.9× rules, observers, infantry guns; S11b ~0.4× field howitzers, light mortars) | S5, S6, S8, S9 (Q4 answered) | High: the owner's on-map artillery; the trench counter |
| S12 | Wounds and medics | ~0.7× | S6 (aid tent); S10 only for the aid dugout | Medium–high: the owner's medical request |
| S13 | Heavy wave | ~0.5× sim (mostly data), but seven new models | S7, S9, S11 | Medium–high: Panther, Tiger, 76 mm Sherman, M36, Jumbo, heavy mortars |
| S14 | Assault kit | ~0.8× | S5, S10, S12 | Medium–high: flamethrowers, then melee |
| S15 | USSR | ~0.6× (mostly data) | all above | High: the third faction |

**Why this order.**

- **S4 first.** It is mostly content-crate work and it makes every later
  slice cheaper. It also clears the no-queue rule, which the vehicle
  waves need.
- **S5 before any new HE.** The mortar dominance problem is structural.
  Finite bombs and an observer rule fix it without cutting stats. New
  mortars, howitzers and infantry guns wait until S11, after measuring.
- **S6 right after S5.** Finite ammo without forward supply makes long
  fights stall. The HQ is also what the owner asked for.
- **S7 before S8 and S9.** The generic vehicle schema is written once in
  S7. Half-tracks, trucks and guns (a gun is built from the vehicle code)
  all reuse it.
- **S8 before S9.** A Pak 40 pushed by its crew takes about 48 minutes
  per km. With "spawn at base and walk", an AT gun without a tow is
  useless. Q11 offers the faster path (a minimal tow in S9 instead).
- **S10 after the assault tools exist.** Wire, mines and bunkers make the
  defence stronger, and the trench assault is already the weak point. So
  S10 ships with breaching (Bangalores, wire cutters, mine clearing), and
  S11's artillery follows straight after.
- **Heavies last (S13).** Their counters (M36, HVAP, AT guns, artillery)
  must exist first. The vehicle spotting range (S3 question 2) should be
  decided before them.
- **Debatable: the trench pillar comes seventh.** Trenches are a core
  pillar, and S10a (wire, sandbags, MG nest, dugout, plus wire cutters
  and Bangalores) needs only S6. It could move ahead of S7 if the owner
  wants the fortification game before more vehicles (Q20).

**Every slice also carries:** an AI check in its "Done when" (the AI
uses the new unit or structure in a set share of AI-vs-AI matches; S3
showed that features the AI ignores, such as satchels on tanks, never
get balanced), the HUD for what it adds, one `SAVE_VERSION` bump, and
blockout art and icons for new units. Final art waits for the M2 style
gate.

### 4.2 Slice cards

#### S4: Variants and call-in (~0.7× S3)

- **Scope:** the variant system (section 2) and the call-in rework.
- **New systems:** `extends`; the card block; immediate spawn,
  per-type cooldowns and `cooldown_group`, `max_alive`, `limit_group`,
  `unlock_s`, date filter; `[[members]]` with per-man weapons and
  explicit crew-weapon pickup; capabilities and tags replacing
  `SquadRole`; AI mix by tags; training profiles.
- **Content:** Grenadiergruppe (with MG 42), Feldstärke, sMG-Gruppe; US
  Rifle Squad (12, BAR), understrength, two BARs, Light MG Section;
  engineer squads at 8 men. New weapon defs: MG 42 (bipod), BAR,
  M1919A4, M1 carbine. The StG 44 and the VG-Sturmgruppe wait for S5:
  an all-automatic squad with unlimited rounds would set a false
  balance baseline. The M7 rifle grenade (and the German
  *Schießbecher*) wait for S11 with the other new HE.
- **Risks:** bigger squads change pop and costs (Q6). Rifles and MGs
  still fire without limit until S5, so S4 and S5 should ship close
  together. The HUD cooldown ring replaces the queue bar. Replacing
  `SquadRole` touches orders and AI widely: the main size risk.
- **Done when:**
  - every existing unit file loads unchanged and resolves to the same
    def as before (a resolved-def snapshot test); with the legacy
    call-in rules, golden tests and hashes are equal on the old content.
    The call-in rework then changes hashes on purpose and records new
    baselines;
  - a variant made of only `extends` plus a member list loads and plays;
  - calling a unit spawns it at base at once; a second call of the same
    type, or of a variant in its cooldown group, is refused until the
    cooldown ends; nothing is queued;
  - AI-vs-AI seeds 1–10 finish inside 12–25 min with the new squads.

#### S5: Ammunition (~0.7× S3)

- **Scope:** every round counted and priced; the base becomes the first
  supply source; mortars fixed.
- **New systems:** rounds per man by ammo id; burst bundles subtract
  every round they stand for; fire discipline (hold fire beyond about
  70 % of weapon range unless ordered; MG bursts with pauses); sharing
  inside a squad; the `SupplySource` rule with the base as its unlimited
  case; **the price per kind (owner decision D1):** every base refill is
  charged round by round at `small_arms_mp_per_kg` (0.2),
  `explosives_mp_per_kg` (0.5) or `shell_mp_per_kg` (1.0), and pauses
  when the player cannot pay; mortar `FireMission` replacing `Barrage`;
  the observer rule; ranging-in; map fire only on known points at 3×
  scatter; true flight time from charges; AI ammo reserve and fire
  discipline; vehicle MG belts counted; tank stowage restored to
  historical counts, with each round priced in base (1 per 2 s) and rare
  rounds refilled at 1 per 10 s up to the vehicle's `rare_cap` (D3);
  Panzerfaust as a single-use launcher item.
- **Content:** ammo defs for small arms and mortar bombs with `supply`
  weights; `rare_cap` on every vehicle def (at most `rare_cap_max`, 10;
  lower where history says so, e.g. 2 HVAP on the M4A3 (76) W); 5-man
  mortar squads; the Panzerfaust card; StG 44 and the VG-Sturmgruppe
  (moved here from S4). HUD: rounds left per man and per crew weapon, a
  "low" and "dry" marker on the squad card, and manpower spent on ammo.
- **Risks:** AI-vs-AI matches stall when everyone runs dry (stall
  guard test). Mortar range stays at today's 600 m in this slice. In S5
  the base still refills mortar bombs directly, at the shells price:
  there are no crates yet. That ends in S6 (owner decision Q4). Priced
  tank rounds may make players hold fire: measure rounds fired per tank
  against S3b (fallback lever `rack_rearm_price_pct`).
  The scope is large; if it runs long, split it into S5a (counts, base
  supply, HUD) and S5b (fire missions, observer, ranging).
- **Done when:**
  - an MG team firing steadily goes dry in the expected time and refills
    in base, paying its kg × `small_arms_mp_per_kg`; with no manpower the
    refill pauses;
  - a tank rearming in base pays each round's kg × `shell_mp_per_kg`;
    it never holds more rare rounds than its `rare_cap`, and a def above
    `rare_cap_max` fails to load;
  - a mortar with no resupply stops after its carried bombs;
  - the same mission, observed and unobserved: miss distance ratio close
    to 3×; ranging tightens only when observed;
  - AI-vs-AI seeds 1–10: the HE share of kills falls by at least a third
    from the post-S3b baseline, and matches still end with a winner
    inside 25 min; manpower spent on ammo is reported per kind. S3b's
    fragment rays change HE lethality, so the
    baseline is re-measured when S3b lands (S3: 43 %; S3b start: 39.5 %
    with tanks, 49.8 % without);
  - hashes equal at 1, 2, 3 and 8 threads and through save and load.

#### S6: HQ and ammo crates (~0.5× S3)

- **Scope:** the owner's HQ follow-up (appendix A).
- **New systems:** a pit dig profile (the first blueprint shape; also used
  for mortar and gun pits); static objects with hit boxes and HP (reusing
  the wreck-box path that already stops fragments); the HQ (pit, stock
  box, radio box); the ammo dump; crates (issue, carry, place, draw,
  cook-off); the resupply radius; retreat to the HQ; the radio link (used
  by S11); caps (one HQ, three dumps,
  a live-crate cap per player); AI HQ siting on a reverse slope, crate
  runs to mortars and trenches. HUD: stock bars per kind, radius ring.
  **Every crate is priced (owner decisions Q4, D1):** a crate taken at
  base costs its kg × its kind's price: shells 1.0 (a 32 kg crate: 32),
  explosives 0.5, small arms 0.2 (a 25 kg crate: 5). The HQ, dumps and
  crates pass on kg that were paid at base; drawing from them costs
  nothing more. The base stops refilling mortar bombs. A mortar's ready
  inventory (the bombs its crew carries) refills only from a crate
  within 5 m or inside an HQ or dump radius. The HQ and dumps fill only
  by carried crates (Q2 C). **Tanks rearm at the HQ and dumps** (D3):
  1 round per 4 s from the shells stock, rare rounds at 1 per 30 s up to
  the vehicle's `rare_cap`. Pushing forward costs delivery, not a
  surcharge (D2). The S11 guns reuse all of this.
- **Content:** HQ and dump defs per faction; crates per faction and kind;
  Munitionsträger / Ammunition Bearers; Zugtrupp / Platoon HQ; mortar pit.
- **Risks:** forward supply raises sustained fire near the front. The
  counters are physical: crates and stock cook off when HE breaks them,
  and the HQ is a target. Without trucks (S8) the HQ refills only by
  crates carried by the player's men (Q2 C).
  Crates are new static boxes in every fragment-ray test: the live-crate
  cap keeps the box count bounded at Battalion scale. Priced bomb crates
  change how much mortars fire: the HE share must be re-measured, and
  this is where the shell price is set.
- **Done when:**
  - a squad inside the HQ radius refills its rounds, most depleted first;
    a squad outside does not;
  - a crate placed 5 m from a mortar keeps it firing; when empty, the
    mortar stops;
  - an HE burst whose fragments do not reach a crate leaves it intact;
    one that does sets off a shells crate;
  - a mortar barrage that never touches the HQ boxes does no damage to
    it; one that breaks in kills the radio;
  - the AI builds one HQ behind a held point and keeps its mortars fed;
  - a second HQ order is refused while one stands; a crate carrier
    killed on the way delivers nothing;
  - a shells crate taken at base costs its kg × `shell_mp_per_kg` in
    manpower, a small-arms crate its kg × `small_arms_mp_per_kg`; with
    too little manpower no crate is issued; drawing from an HQ, dump or
    crate charges nothing more;
  - a tank next to an HQ or dump rearms at 1 round per 4 s and its
    stock falls by the same kg; rare rounds come at 1 per 30 s and stop
    at the vehicle's `rare_cap`; an HQ with no shells stock rearms
    nothing;
  - a shells crate costs the same wherever it is taken; no price
    depends on distance from base (D2);
  - a mortar team standing in base with no crate does not refill its
    bombs; next to a crate or inside an HQ radius it does;
  - AI-vs-AI seeds 1–10 with priced bomb crates: the HE share is not
    above S5's, and matches still end with a winner inside 25 min. Set
    `shell_mp_per_kg` from this run.

#### S7: Assault guns and tank destroyers (~0.9× S3)

- **Scope:** the general vehicle schema, casemates, open tops.
- **New systems:** several armour boxes with open faces; optional turret;
  seats and mounts as lists; casemate gunnery (limited arc, the driver
  turns the hull); open-top exposure (rounds, fragments and grenades get
  in through the open face; wire screens stop grenades); small arms
  against thin plates (so .50 and SmK rounds can hurt 13 mm armour);
  engine-driven hydraulic traverse.
- **Content:** StuG III G, Jagdpanzer IV, Jagdpanzer 38(t); M10, M18.
- **Risks:** StuGs are strong on defence, which adds to the trench
  problem. Shermans flanking at 24°/s traverse may beat casemates more
  often than history suggests: measure before tuning. One `SAVE_VERSION`
  bump for the schema.
- **Done when:**
  - Panzer IV H and M4A3 (75) play identically under the new schema
    (armour-test and golden tests unchanged);
  - casemate duel: StuG frontal vs Sherman at 300 m, and Sherman flanking
    from 45°, reported;
  - a grenade thrown into a stopped M10 hurts the crew; a mortar
    near-miss wounds open-top crews through the open face only;
  - an MG 42 with ball rounds cannot hurt a StuG; a US .50 holes an
    SdKfz 251 side and a German 2 cm KwK 38 holes an M18 hull side at
    100 m (each tested with the weapon its enemy really fielded);
  - the AI keeps a casemate's front toward the nearest seen enemy tank
    in at least 8 of 10 armour-test runs.

#### S8: Half-tracks, trucks and recon (~1.1× S3, the largest)

- **Scope:** transport, supply vehicles and light armour.
- **New systems:** wheeled and half-track movement (trenches 1 m deep or
  more block wheels; half-tracks cross like tanks with more bogging);
  embark and disembark with passengers kept as soldiers in slot boxes;
  pintle and ring MGs with exposed gunners; recon optics and the
  unbuttoned commander; supply stock on vehicles, `DropCrates`, and a
  `cargo` part that explodes when hit; every kg loaded at base pays its
  kind's price (D1); supply vehicles rearm tanks at 1 round per 4 s and
  rare rounds at 1 per 30 s up to `rare_cap` (D3); AI logistics (truck
  base → HQ → base) and rearming.
- **Content:** SdKfz 251/1, SdKfz 234/1, Opel Blitz; M3A1, M8, M20,
  CCKW; Panzergrenadiergruppe and Armored Infantry Squad cards.
- **Risks:** transport shortens the walk to the front. It spawns at base
  and must shuttle, so this is acceptable, but measure time saved per
  squad and cap it by cost and `max_alive` (no "arrives mounted" cards).
  Passengers add hit-test load.
- **Done when:**
  - a Bazooka hit on a loaded 251 costs passengers; a grenade into a
    stopped one costs most of them;
  - an armoured car stops at a trench line or finds a gap;
  - a tank at 10 % ammo rearms from a truck within 15 m at 1 round per
    4 s, rare rounds at 1 per 30 s, never above its `rare_cap`; the
    truck's stock falls by the same kg; a penetrated truck's cargo
    explodes;
  - the AI keeps its HQ stocked with a truck, and AI matches still
    finish; a truck loading at base pays each kind's price per kg, and
    loads shells at 32 kg per `base_crate_issue_s`, in the same queue as
    crates;
  - a tank that pushes with no truck, HQ or dump behind it stops firing
    its main gun when its rack is empty (D3);
  - the AI uses transport in at least 6 of 10 matches and unloads short
    of the enemy line, not on it;
  - riding, stocks and hatches survive save, load and net loopback.

#### S9: AT guns (~0.9× S3)

- **Scope:** the gun entity, crews as soldiers, towing, gun pits.
- **New systems:** gun carriage and shield as vehicle-style boxes; crews
  in service slots (bullets that miss the shield reach them); emplace,
  pack, push, hitch, re-lay the trail; recrew, abandon and capture
  (neutral guns); gun optic sight bonus inside the arc; tanks fire HE at
  the crew behind a spotted gun; AT gun pit; AI ambush siting.
- **Content:** Pak 40, Pak 38; 57 mm M1; Sd.Kfz. 11 tow. The 3-inch M5
  as data (emplaced only).
- **Risks:** capture makes ownership mutable for the first time (hash,
  snapshot, AI bookkeeping). The gun optic must be set together with the
  vehicle spotting range (Q12).
- **Done when:**
  - MG fire from the front kills crews only with rounds that miss the
    shield (at most 30 % of the losses of the same crew without a gun);
    from 45° at least 3× the frontal losses;
  - a concealed Pak 40 holding fire to 400 m fires first at an advancing
    Sherman in at least 80 % of runs and disables it in at least 60 %;
  - kill the crew: the gun turns neutral; an enemy squad can serve it,
    but only with the rounds left at the gun or in captured crates of
    the gun's own faction (its own crates do not fit);
  - the AI emplaces its AT guns in cover facing an approach and fires
    them in at least 6 of 10 AI-vs-AI matches.

#### S10: Fortifications and breaching (~1.5× S3; split into S10a and S10b)

- **Scope:** the structure framework and the engineer split. S10a:
  blueprints, materials, sandbags, MG nest, dugout, wire, wire cutters,
  Bangalores, the HQ dugout, the engineer split. S10b: mines and their
  detection, AT obstacles, the timber-and-earth bunker.
- **New systems:** blueprints and construction pieces with armour faces
  and HP; materials (`construction` kg) carried, crated or trucked;
  sandbags, MG nest, dugout, timber-and-earth bunker, wire, AP and AT
  mines, hedgehogs, log barriers; capability tiers (field, assault,
  construction); breaching: wire cutters, Bangalore torpedo, mine
  detection and lifting; the HQ upgraded to a roofed dugout.
- **Content:** Sturmpioniergruppe, Baupioniergruppe; Assault Section; mine
  and obstacle defs. Concrete pillboxes stay map and mission pieces only
  (concrete cures for days).
- **Risks:** defence gets stronger while the trench assault is already
  weak. Breaching ships in the same slice, and S11 artillery follows.
  Mines need fair rules: the owner's side knows them, the enemy finds
  them only by searching or by setting one off. In lockstep every client
  simulates every mine, so hiding them is a presentation rule and a
  modified client could show them (the same is true of all fog of war).
  Structure spam: every piece has a per-player cap in rules data, and
  materials must be carried or trucked from base.
- **Done when:**
  - wire slows infantry to 20 % until cut or blown;
  - the AI wires and sandbags a held point, and its engineers cut or blow
    enemy wire before an assault, in at least 6 of 10 matches;
  - a mine goes off only on contact; a detector sweep finds it at the
    expected rate;
  - a dugout roof survives an 81 mm hit and breaks under heavier HE;
  - fortify-test: a trench with wire and an MG nest against assault
    engineers plus a supporting MG, numbers reported against the S3b bar.

#### S11: Artillery on the map (~1.3× S3; split into S11a and S11b)

Rewritten 2026-11-01 after the owner's request for artillery as units.
Design: `artillery-on-map.md`.

- **Scope:** artillery as on-map, crewed, buyable units. One card is one
  gun and its crew. Authentic ranges stay; observation, finite shells
  that cost manpower, counter-battery, set-up times, masks, crews in the open and caps are
  the brakes. Off-map batteries are dropped from skirmish (Q13 A).
  S11a: the rules, observers and infantry guns. S11b: field howitzers on
  the map, light mortars and rifle grenades.
- **New systems (S11a):** ready inventories for guns (owner decision
  Q4: no ration): a gun refills only from a crate within 5 m, inside an
  HQ or dump radius, or from its tow; the base zone does not refill it;
  shells crates cost manpower at base (from S6); observer links by radio (FO
  teams, gun sections, the HQ radio box) or voice (150 m); fire modes
  direct, observed, map fire and counter-battery; registration per gun
  and target, lost when the gun moves; S5's charge solver extended to
  guns, with elevation bands and a mask check (no solution that clears
  the ground: "masked"); fire-direction delay per faction; sound ranging
  (marker after 3 rounds, error 15 % of the listener distance, sharpening
  each round, with an evidence count that decays slowly, so slow fire
  and short hops are found too) and flash (line of sight spots the gun
  on the first round, through the normal spotting scheduler);
  a two-term error for guns (an aim error the observer removes, plus the
  authentic precision spread), registration that expires; starting
  loads in the card's price, with a content check that a card is worth
  at least its load at crate price, and tows arriving half full; the
  artillery cap counting captured and
  neutral guns; a targeting overlay and one refusal reason per order;
  displacement; `howitzer_pit`; HE craters scaled by filler; gun direct
  fire with HE and HEAT; no free crew for guns in base; AI artillery
  planner (buy with an observer, site on reverse slopes, observe from
  crests, prepare, defend, counter-fire, relocate, buy and carry shells
  crates within a budget); friendly fire from mortars, guns and
  launchers (owner decision Q22); the own-count hint "N rounds from this
  spot: likely located", never the enemy's marker (Q24). The AI uses observation posts and gun areas
  precomputed at map load. **S11b:** little new code (a launcher mode
  for HE rifle grenades); otherwise data and tuning for the 2 t field
  howitzers.
- **Content:** S11a: VB-Trupp, FO Party; leIG 18 section (pushed, and a
  variant with an Sd.Kfz. 11), 105 mm M3 section; Igr 18, Igr 38 HL, M1
  HE, M67 HEAT. S11b: leFH 18 and 105 mm M2A1 sections (one gun each);
  75 mm pack howitzer as data; kz. GrW 42, 60 mm M2; M7 rifle grenade and
  *Schießbecher*. Later: sIG 33; Nebelwerfer 41 only if Q23 allows.
- **Size:** S11a ~0.9×, S11b ~0.4× (data, models and tuning). S11a is
  likely optimistic. Split point if it runs long: S11a1 (ready inventories, links,
  fire modes, masks, leIG 18 and M3, basic AI) and S11a2 (registration,
  counter-battery, displacement, defensive-fire AI).
- **Depends on:** S3b (fragment rays, parapet erosion, duck state); S5
  (counted ammo, `FireMission`, observer rule, charge solver); S6 (HQ
  radio box, crates, pit kind); S8 (trucks as tows and ammo carriers); S9
  (gun entity, service slots, towing, capture). Q4 is answered (no
  ration; priced crates). S11b waits for S11a's measurements.
- **Risks:** more HE. S11a ships only if S5 met its HE-share target and
  S6 kept it with priced bomb crates. Without the ration there is no hard
  HE ceiling: a gun adds HE if its owner pays, and a richer player can
  buy more. `artillery-on-map.md` §4.10 re-checks every exploit the
  ration closed; three are weaker (base battery, overall HE, forward
  stockpile), and each has a TOML lever (`shell_mp_per_kg`,
  `base_crate_issue_s`, `shell_crate_cap`, dump caps). A second critic
  pass closed four back doors to free HE (ZiS-3 HE, cargo laundering,
  unthrottled truck loads, a shells trickle). Counter-battery
  duels could crowd out play: report the share of shells fired at guns.
  Base artillery will be common on 1 km maps; in base the walk is short,
  so the crate price is the main brake there (§4.9).
  With 600 m mortars, only guns or a raid reach a rear gun.
  `indirect_from_base = false` exists but helps little on small maps.
  Ranged-in guns are accurate at these ranges, so trench safety rests on
  S3b's trench model. The shell price must come from S6's measurement,
  not from the 1.0 placeholder. Placement is compressed: on 1 km maps
  the leFH 18, M3 and M2A1 fire almost flat (weak against trenches, good
  against structures and in direct fire). A range cut would not change
  that, since targets are in range anyway (Q25 decided: all maps, cap 1,
  8:00 unlock as defaults to measure). Friendly fire (Q22) makes the
  lift a skill; in team games a player can shell allies, so the
  danger-close warning covers them and the match report lists
  friendly-fire kills per player.
- **Done when:**
  - with no linked observer, an observed mission is refused; map fire
    lands only on knowledge points, with a gun's mean miss about 2× an
    observed first round; a registration expires after 600 s;
  - control run with priced crates and no guns: the HE share is not
    above S6's;
  - a called gun arrives with its full starting load; a card worth less
    than its load at crate price is raised to that price, with a
    warning; a player cannot serve more guns than the cap, own, neutral
    or captured, and a ZiS-3 cannot fire indirect past the cap;
  - no back doors (`artillery-on-map.md` test 7b): rack rounds of a tank
    or AT gun rearmed in base never reach a crate, dump or gun; a ZiS-3
    gets no HE from the base zone; a trickle above 0 of any kind fails
    to load;
  - a gun parked in base does not refill from the base; it refills from
    a crate within 5 m or inside an HQ or dump radius, and that stock
    goes down by the same kg;
  - a leFH 18 or M3 told to hit a trench behind a crest from 600 m
    reports "masked"; a leIG 18 at high angle hits it;
  - a gun firing from one spot gets an enemy marker after 3 rounds, with
    an error of at most 15 % of the listener distance that shrinks each
    round; a gun that displaces after 4 rounds survives counter-battery
    more often than one that stays (10 runs each); a gun firing one
    round a minute is marked by its 4th round; an enemy with line of
    sight spots the gun by flash on its first round; the firing player
    sees only his own-count hint, never the enemy's marker;
  - a gun in base with a forward observer stops when its ready inventory
    is empty and no crate is near; report the manpower each side spent
    on shells; AI matches still end with a winner inside 25 min;
  - own men inside a mortar, gun or launcher burst take fragments and
    blast; own direct fire still passes through them; the AI lifts fire
    in time in at least 9 of 10 staged assaults;
  - a leIG 18 at 400 m silences an MG nest over open sights; a 105 mm
    burst breaks a dugout roof that survives an 81 mm hit; M67 HEAT
    penetrates a Panzer IV side; 105 mm HE on a Sherman damages only
    external parts and tracks;
  - fortify-test "105 mm barrage" row: trench losses at most 40 % of
    open; lip lost per 105 mm burst within 2 m at least 0.15 m; a
    preparation barrage cuts attacker losses to at most half the
    no-support baseline (owner sets the bar);
  - 20 AI matches: HE share of kills at most 30 % on average and 45 % in
    any match (Q16), and at most 45 % for the richer side of every match;
    infantry guns and howitzers at most 10 % of kills; manpower spent on
    shells reported per player (AI 10–30 % of income);
  - the AI buys a gun and an observer in at least 6 of 10 matches, fires
    observed or counter-battery missions, never fires indirect without
    an observer except on knowledge points, and displaces after
    counter-fire;
  - stress scene with 20 guns and 30 mortars firing at full rate on
    Battalion: no capped fragment rays in AI matches, stress p99 inside
    S3b T13; hashes equal at 1, 2, 3 and 8 threads and through save and
    load with shells in flight.

#### S12: Wounds and medics (~0.7× S3)

- **Scope:** the medical half of the owner's request.
- **New systems:** hit zones (head, torso, legs); fit, wounded, down and
  dead with bleed-out; self and buddy bandage; medics stabilise and
  carry; aid post (tent or dugout) that works only while staffed;
  evacuation for a manpower refund; replacements bought for depleted
  squads (they spawn at base and walk).
- **Loop guards:** a man is treated at an aid post **once** per match (a
  saved `treated` flag); a second wound sends him to evacuation. The
  evacuation refund (40 % of his share of the squad cost) is paid once
  per man and is always less than the replacement cost. Aid posts are
  capped at two per player.
- **Content:** Sanitätstrupp, Aid Men; aid post defs. No red cross
  emblem unless a legal check clears it (Q14).
- **Risks:** squads lose men more slowly, so lethality and match length
  shift. Re-measure the AI matches. Downed men stay soldiers on the
  ground: they count toward pop and the tier's 700 until they die or are
  evacuated, and the bleed-out timer bounds how long that lasts.
- **Done when:**
  - a squad under fire ends with downed men; with a medic, fewer bleed
    out;
  - a staffed aid post returns wounded men at 75 % health; kill the
    medics and it stops; a man wounded a second time is not treated
    again;
  - AI medics stabilise and carry men in at least 6 of 10 matches;
  - match length stays in 12–25 min.

#### S13: Heavy wave (~0.5× S3 in sim work, mostly data; seven new models)

- **Scope:** heavies and the heavy HE weapons, with their counters.
- **New systems:** little: a shared `heavy` group cap (Tiger I and
  Jumbo; the Panther sits in the `armour` group with its own cap of 2,
  see Q17). Rare rounds already work from S5 and S6 (D3): refilled at
  every source, slowly, up to a per-vehicle cap. S13 sets the heavies'
  and the M36's `rare_cap` from sources.
- **Content:** Panther G, Tiger I; M4A3 (76) W, M36, M4A3E2; 12 cm GrW
  42, 4.2-inch mortar.
- **Risks:** decide the vehicle spotting range first. At 350 m the
  Panther's long-range edge never shows; at 600–700 m it dominates. Do
  not touch armour or guns; balance by cost, pop, cooldown, cap, unlock
  and the counters.
- **Done when:**
  - armour-test: Panther G frontal vs two Shermans flanking; M36 vs
    Panther at 350 m; HVAP used per match, all reported;
  - AI-vs-AI win rates stay within 35–65 % per faction.

#### S14: Assault kit (~0.8× S3)

- **Scope:** the trench-clearing weapons, then melee.
- **New systems:** flamethrowers as many short-lived particles with
  burning patches (they pass through bunker slits and doorways); a
  "clear trench" order; then a minimal melee strike (a swept segment from
  the weapon, used only when a man's weapon cannot fire in time).
- **Content:** Flammenwerfer 41 in the Sturmpioniergruppe; M2-2 in the
  Assault Section.
- **Risks:** flame is the heaviest new projectile kind (pool size,
  burning-patch state in the snapshot). The Battalion pool is 1,800
  projectiles for everyone, so flame particles need their own capped
  sub-pool per player (or a few swept stream segments per burst), never
  a share that can starve rifle and shell rounds. Melee is mostly
  animation work.
- **Done when:**
  - a flamethrower clears a bunker through its firing slit;
  - two flamethrowers firing at once stay inside their sub-pool and do
    not drop any shell or rifle round;
  - the AI uses its flamethrower on an occupied trench or bunker in at
    least 5 of 10 matches;
  - fortify-test: an assault section with flame vs a trench, compared
    with the rifle baseline;
  - melee strikes hit only by touching a capsule.

#### S15: USSR (~0.6× S3, mostly data)

- **Scope:** the third faction (section 3.3).
- **New systems:** small: body armour plates on the torso zone (needs
  S12's hit zones); captured-ammo compatibility (82 mm fires German 8 cm
  bombs); tank riders (S8 seats on the engine deck, exposed to every
  round and fragment that touches them).
- **Done when:** the faction plays AI-vs-AI against both others inside
  the win-rate band.

---

## Owner decisions (2026-11-01)

Q4, Q22, Q24 and Q25 were answered later the same day; §5 marks them
answered. D1–D3 are owner requests on ammunition from later still; they
were not on the question list, and they supersede earlier text where
they differ.

| Q | Decision | Note |
|---|---|---|
| 2. HQ fill before trucks | **(C) Nothing**: only crates carried by the player's own men, later trucks and half-tracks. | The owner was unsure; a carrying party could be abused (free scouts or decoys) and often dies on the way. Revisit after S8 if the HQ is empty too often in play. |
| 4. Base and mortar range; who pays for shells | **No ration.** Owner: "Perhaps, the ration shouldn't exist, but the artillery will have an inventory with shells at immediate disposal. But they can be refilled by bringing ammo crates nearby or placing the artillery near a structure that can refill them." Follow-up: shells crates **cost manpower** when taken at base; small-arms crates free or cheap; all values in TOML. | Each gun and mortar has a ready inventory. It refills from crates placed or carried within 5 m, or inside an HQ or dump radius; their stock fills only by crates (later trucks). The base does not refill shells. **Mortars follow the same rule from S6** (S5 has no crates yet, so its base still refills bombs). Starting loads come with the card's price. Start values: `shell_mp_per_kg` 1.0, small arms 0 (small arms superseded by D1). Weaker than the ration: HE now scales with income. Each exploit is re-checked in `artillery-on-map.md` §4.10, with a lever for each weaker one. A second critic pass closed four back doors to free HE: ZiS-3 HE, cargo laundering, unthrottled truck loads, a shells trickle (critic C15–C18). |
| 6. Squad sizes | **(A)** Historical sizes (German about 10, US 12), cheaper understrength cards, pop cap 150. | |
| 11 / 20. Order | **(A)** Vehicles first: S7 assault guns and TDs, S8 half-tracks and trucks, S9 AT guns; fortifications S10. | |
| 13. Artillery | **Owner prefers artillery as on-map units** (request of 2026-11-01). Default changed to on-map (A). | Design in `artillery-on-map.md`. Off-map batteries dropped from skirmish. S11 rewritten and split. |
| 21. Variant cooldowns | **(B)** Every card has its own cooldown. | Risk: alternating full and understrength cards doubles the call rate; manpower cost and `max_alive` are the brakes. `cooldown_group` stays in the schema (unset by default), so it can be switched per card in TOML. |
| 22. Friendly fire from indirect HE | **(A) Yes.** Fragments and blast from mortars, guns and launchers hurt own men; direct fire stays as today. | Ships in S11a. Allies can be hit too: the danger-close warning covers them, and the match report lists friendly-fire kills per player. |
| 24. Show the enemy's marker on a gun? | **(A) Hint only:** "N rounds from this spot: likely located", from the player's own count. Never the enemy's marker. | `cb_hint_rounds` in TOML. |
| 25. Field howitzers on small maps | **Allowed on all maps.** The owner is unsure about the limit and suggested a small range tweak. | Cap 1 per player and 8:00 unlock stay as TOML defaults to measure in S11b. A range cut does not fix flat fire on small maps: targets are in range anyway. A per-map max range has no real use (it only moves a base gun forward), so none is offered. Levers if they dominate: cost, cap, unlock, cooldown, then the `allow_field_howitzers` map flag. |
| D1. Who pays for ammunition | **No free ammo anywhere, the base included.** Owner: "Let's not make shells and other ammo free. Make small arms ammo cheap, but the artillery and tank ammo, shouldn't be too cheap." | One price per supply kind, in manpower per kg, paid once when the kg leaves the base (base-zone refill, crate issue, vehicle load): `small_arms_mp_per_kg` 0.2 (a rifle squad's refill about 5), `explosives_mp_per_kg` 0.5 (a Panzerschreck rocket about 1.7), `shell_mp_per_kg` 1.0 (mortar, gun and tank rounds; a 75 mm round about 10). HQ, dump, crate and truck stock was paid at base. Replaces every free base refill (items, rockets, small arms, tank racks, AT guns). Back-door rules re-checked in `artillery-on-map.md` §4.2: the cargo, rack, queue and crate-only rules stay; the free-refill exemptions and zero prices are dropped; a trickle of any kind is refused. |
| D2. Distance from base | **Physical delivery only, no distance surcharge.** Owner: "maybe a distance of the artillery from the base. If they are pushing too much, it will take longer for the ammo to arrive, and thus taking more resources from the overall army." | Pushing forward costs more because men must carry crates and trucks must drive them from base or the HQ; that time, exposure and those losses are the cost. A surcharge by distance would make the base, already the best gun position, also the cheapest, so none is built (`artillery-on-map.md` §4.11). |
| D3. Tank ammunition | **Rearm at base, near an HQ or dump, or from a supply truck; every round costs manpower.** Owner on rare rounds: "add rare round to be available for recharge at other point. Maybe they could recharge slower and only to a certain cap, like one vehicle can only have 10 of these. Not more. So that way the ability of the tanks to push infinitely is limited by the ammo storage inside them." | Standard rounds: 1 per 2 s in base, 1 per 4 s forward. Rare rounds (APCR, HVAP, HEAT): 1 per 10 s in base, 1 per 30 s forward (`rare_rearm_s`). Per-vehicle `rare_cap`, default and ceiling `rare_cap_max` 10, lower where history says so (M4A3 (76) W: 2 HVAP). The rack is the hard limit on a push (`vehicles.md` §6.11, §7.3, §7.6). |

All other questions run on their (A) defaults until answered. Q23
(rocket launchers) is still open.

## 5. Owner questions

Each has a recommended **(A)**. Work can go on with (A) until you
answer. The slice in brackets is the first one blocked by the answer.

**HQ and supply**

1. **What the HQ does** [S6]. (A) Supply dump, radio link, retreat point.
   No spawn, no reload aura, no unlocks. (B) As A, plus slow field
   repair of vehicles later. (C) A forward reinforcement point (not
   recommended: it breaks "spawn at base and walk").
2. **How the HQ fills before trucks** [S6]. **Answered 2026-11-01: (C)
   nothing** (see "Owner decisions"). The options as they were asked:
   (A) A free, automatic
   two-man **carrying party** spawns at base every few minutes (data;
   about 40 kg per trip), walks to the HQ and unloads. It takes no pop,
   cannot be ordered and does not capture points; if it is shot, the load
   is lost. Same effect as a trickle, but it obeys "spawn at base and
   walk" and can be interdicted. Off in a "realism" rules file; review
   after S8. (B) An abstract trickle into the HQ (about 10 kg per minute
   of small arms, 15 kg of shells): simpler, but ammunition appears at
   the front from nothing. (C) Nothing; stock comes only by carried
   crates and later trucks.
3. **Supply unit** [S5]. (A) Kilograms in four kinds (small arms,
   explosives, shells, construction); a faction's crate feeds its own
   calibres. (B) Strict per-calibre crates. (C) Abstract supply points.
4. **Base and mortar range** [S5; must be settled before S11].
   **Answered 2026-11-01: no ration; priced shells crates** (see "Owner
   decisions" above; `artillery-on-map.md` §4.2, §4.10). The options as
   they were asked: (A) Base
   resupply stays free and mortar range stays at today's 600 m through
   S5; measure; decide B or C before S11, because the on-map leIG 18 and
   105 mm M3 out-range the map and would fire from base for free.
   (B) Authentic ranges, plus one per-player supply rate (kg per minute)
   for every shell restock, base included. (C) Authentic ranges, plus a
   manpower cost per restocked shell.
   With on-map artillery (Q13 A) this must be answered before S11a.
   The design had recommended (B) as a shell ration at base. The owner
   chose a form of (C) instead: manpower is paid per shells crate taken
   at base, and each gun or mortar refills from crates and refill
   structures, never from the base itself. The HQ still fills only by
   carried crates and trucks (Q2 C).
5. **Smoke rounds** [S5]. You ruled out mortar smoke as the S3b
   trench-assault fix. (A) No smoke rounds in this plan; smoke grenades
   stay. Revisit only if S3b's pinning misses its trench-assault target.
   (B) Allow smoke and WP rounds for mortars and tanks as historical
   options (the cheapest assault lever both designs found).

**Infantry**

6. **Squad sizes** [S4]. (A) Historical sizes (German 10, US 12) with
   cheaper understrength cards, and the pop cap raised from 120 to 150.
   (B) Historical sizes, pop cap unchanged. (C) Keep 8 for everyone.
   Whichever you pick, in 4v4 and 5v5 Battalion lobbies the tier share
   (700 soldiers ÷ players) is lower than either cap, so big lobbies
   field fewer, larger squads.
7. **Names** [S4]. (A) Period names in the faction's language
   (Grenadiergruppe, Volksgrenadier-Sturmgruppe, Sturmpioniere; Rifle
   Squad, Assault Section; avtomatchiki), with an English gloss in the
   tooltip. No "stormtroopers". (B) English names only.
8. **Officer rally** [S4]. It is the one effect that does not come from
   contact (it only speeds recovery from suppression). (A) Cut it.
   (B) Keep it, small and visible.
9. **Melee** [S14]. (A) A minimal physical strike, last, no knife
   squads. (B) Cut melee entirely. (C) Build it earlier.
10. **Wounds** [S12]. (A) Fit, wounded, down, dead, with bleed-out,
    medics, a staffed aid post and an evacuation refund; no healing over
    time. (B) Wounded state only, no "down" state.

**Guns and vehicles**

11. **AT guns or vehicles first** [S7]. (A) Vehicle schema (S7) and
    trucks (S8) before AT guns (S9). (B) AT guns as S7 with a minimal
    unarmoured tow, vehicles after.
12. **Vehicle spotting range** (S3 question 2, still open) [S9]. (A) Keep
    2× (tanks see tanks at about 350 m); AT guns get a 250 % optic bonus
    inside their arc. (B) 4× for all vehicles (600–700 m). Decide both
    together; it matters more for Panthers and Tigers than any stat.
13. **Artillery** [S11]. Changed 2026-11-01 after the owner's request
    ("having howitzers or some artillery is better as a unit"). (A)
    On-map, crewed, buyable units: infantry guns (S11a) and single-gun
    field howitzer sections (S11b), two per player, with authentic
    ranges. Observation, finite shells in priced crates (Q4), counter-battery by
    sound and flash, set-up times, masks from the real charge tables and
    caps hold them back. Off-map batteries are dropped from skirmish.
    Reasons: an on-map gun can be found, shelled and overrun, so every
    brake is physical; an off-map battery can never be hit back, and it
    would make the on-map cards pointless. Range compression does not
    help: any range long enough to be a howitzer still covers the front
    from base. (B) The old default: off-map batteries called by an
    observer, regimental guns on the map.
14. **Red cross** [S12]. (A) A plain armband and stretcher, no emblem,
    unless a legal check clears it. (B) Use the emblem.
15. **Waffen-SS and penal units** [S4]. (A) Both stay out. (B) Revisit
    for missions only.
16. **HE-share target** [S5]. (A) About 20–30 % of kills from HE after
    S5 and S11 (S3: 43 %; to be re-measured after S3b's fragment rays),
    with trench losses under a barrage at most 40 % of those in the
    open. (B) Another bar.
17. **Heavies** [S13]. (A) One shared "heavy" slot (Tiger I or Jumbo),
    late unlock, only after the counters exist. The Panther is not in it:
    it was a medium tank and, in 1944, about as common in a panzer
    division as the Panzer IV, so a one-of-anything cap would be an
    availability rule history does not support. It gets its own cap of 2,
    a high cost and an 8:00 unlock. If it dominates once the spotting
    range is set (Q12), move it into the heavy slot. (B) Panther, Tiger
    and Jumbo share one heavy slot. (C) One cap per type.
18. **Transport** [S8]. (A) From 2:00, at most 2 alive, passengers do not
    fire from vehicles in v1. (B) No cap at first; measure.
19. **Fuel** [S7]. (A) No fuel resource. (B) A fuel bar.

**Order**

20. **Fortifications earlier?** [S7]. Trenches are a core pillar, but
    the plan builds wire, sandbags, MG nests and dugouts seventh (S10).
    (A) Keep the order: vehicles first, because AT guns need tows and
    the vehicle schema serves three slices. (B) Move S10a (wire,
    sandbags, MG nest, dugout and their breaching) to right after S6; it
    needs only S6.
21. **Variant cooldowns** [S4]. (A) Variants share their parent's
    cooldown (`cooldown_group`), so cheap and full versions cannot be
    alternated. (B) Every card has its own cooldown, as "per-unit-type"
    reads literally.

**Artillery** (added 2026-11-01; detail in `artillery-on-map.md` §11)

22. **Friendly fire from indirect HE** [S11a]. **Answered 2026-11-01:
    yes (A).** Today no own round hurts
    own men, so a player could shell a trench while his men walk into it.
    (A) Fragments and blast from mortars, guns and launchers hurt own men;
    direct fire stays as today. (B) Keep no friendly fire; the AI keeps
    its danger-close and lift-fire rules.
23. **Rocket launchers** (Nebelwerfer 41, later BM-13) [after S11b]. One
    salvo is about 200 kg of ammunition but only about 12–15 kg of
    explosive (verify), about six 105 mm shells, all landing within ten
    seconds. (A) Later, as a lobby option, cap 1,
    12:00 unlock, after the shell price is measured with tube artillery.
    (B) Missions only. (C) In S11b.
24. **Show the player the enemy's marker on his gun?** [S11a].
    **Answered 2026-11-01: hint only (A).** (A) No:
    only his own count ("6 rounds from this spot: likely located").
    (B) Show it (leaks enemy knowledge).
25. **Field howitzers on small maps** [S11b]. **Answered 2026-11-01:
    allowed on all maps; cap 1 and 8:00 unlock as TOML defaults to
    measure.** On 1 km maps the leFH 18, M3 and M2A1 fire almost flat. A
    range tweak would not change that, and a per-map max range has no
    real use (`artillery-on-map.md` §4.5). The options were: (A) Allowed
    on every map, 1 per player, 8:00 unlock. (B) Only on maps of 2 × 2 km
    and larger (a map flag).

---

## Appendix A. HQ, ammo and supply in one page

This answers the follow-up: "Perhaps the HQ could allow for spawning
ammo crates or help the units nearby reload it." Detail is in
`infantry-structures.md` §6–7, `guns-artillery.md` §4 and `vehicles.md`
§7.

**Names.** "HQ" here is the structure: *Gefechtsstand* (DE), command
post / CP (US), *komandny punkt* (SU). In the game it should carry those
names, so players do not confuse it with the Zugtrupp / Platoon HQ
squad that builds it.

### A.1 What runs out

| Who | What is counted | Runs out after (starting values, *verify*) |
|---|---|---|
| Rifleman | 60–130 rounds | 4–10 min of steady fire |
| SMG or StG 44 | 140–200 rounds | 1–3 min of bursts |
| MG team (MG 42, M1919, BAR) | 250–1,500 rounds, shared in the team | 1–2 min continuous, 5–8 min in bursts. **The first thing to go quiet.** |
| Every man | grenades, smoke, satchels, Panzerfaust (today's counts) | single use |
| Launcher team | 5–6 rockets | — |
| Mortar squad (5 men) | 18–30 bombs, mostly with the bearers (its ready inventory) | 2–3 fire missions |
| Infantry gun or howitzer | ready inventory at the gun (leIG 18: 12 HE, 2 HEAT; 105 mm: 12–15 rounds), the tow half full on arrival | one or two fire missions |
| AT gun | ready rack and reserve at the gun (Pak 40: 10 AP, 6 HE, 2 APCR), more in the tow | one engagement |
| Tank | historical stowage (Pz IV H 87, *verify*), MG belts counted, rare rounds up to the vehicle's `rare_cap` (at most 10; M4A3 (76) W: 2 HVAP) | rarely in 30 min, but rare rounds go fast and refill slowly; far from a stock, the rack is the limit |

### A.2 How supply flows

```
Base (unlimited) ──truck / half-track──▶ HQ or dump (finite stock)
      │                                      │
      │                                      ├─ resupply radius (25 m / 15 m): men, mortars and guns refill
      │                                      ├─ issue crate ──carried──▶ trench / mortar pit / gun
      │                                      └─ vehicles rearm at half the base rate; rare rounds 1 per 30 s
      ├─ crates carried by the player's men ──walk──▶ HQ, dump, gun or mortar
      ├─ men and vehicles refill in base (today's rates), each kg priced (D1)
      └─ shells and mortar bombs leave the base only in crates or vehicle loads,
         at a manpower price (owner decision Q4); the base refills no gun or mortar
```

Prices (D1), manpower per kg, paid once when the kg leaves the base:
small arms 0.2, explosives 0.5, shells 1.0 (mortar, gun and tank
rounds). Nothing forward is charged again. There is no distance
surcharge (D2): the walk and the drive are the cost of pushing.

| Source | Stock | Radius | Serves |
|---|---|---|---|
| Own base | unlimited; every kg priced (D1) | base zone | men, tanks, AT guns, into their own racks only, each round charged at its kind's price; tanks 1 round per 2 s, rare rounds 1 per 10 s up to `rare_cap`; repair and new crew (today's rules). **Not** guns' shells, mortar bombs or ZiS-3 HE: those leave only in crates or vehicle loads, at `shell_mp_per_kg` (start 1.0) per kg, 32 kg per `base_crate_issue_s`; any cargo pays its kind's price, whatever calibre |
| HQ | finite, **starts empty** (caps about 300 kg small arms, 200 kg explosives, 600 kg shells). Costs 100 manpower plus 120 kg of construction material; one per player; rebuild after 120 s if destroyed | 25 m | men; vehicles at half rate, rare rounds at 1 per 30 s; issues crates |
| Ammo dump | finite, smaller; up to 3 per player | 15 m | men; vehicles at half rate, rare rounds at 1 per 30 s; issues crates; no radio |
| Supply truck | about 2.5–3 t (Opel Blitz 3 t, CCKW 2½ t off-road), soft-skinned | 8–15 m | men, vehicles (half rate, rare rounds at 1 per 30 s); unloads into the HQ; drops crates |
| Half-track | 300–500 kg, armoured | 8 m | men, vehicles (as the truck); works under fire |
| Crate | 20–40 kg; a live-crate cap per player (data, e.g. 12), of which at most 8 shells crates | 5 m | men of any squad of that faction; a gun or mortar within 5 m |
| Squadmate | his own surplus | 3 m | same squad, same calibre |

Rules inside the radius:

- A man is served when he is alive, not pinned and has not fired for 3 s.
- He takes the most depleted thing first.
- Each source serves a few men at a time, in a fixed order, so the sim
  stays deterministic.
- A mortar or gun is fed when it (or its crew) stands in the radius, or
  a crate lies within 5 m of it. In the base zone it is **not** fed: a
  crew in base takes a priced crate and sets it down by the gun.
- Tank shells are too heavy for carried crates: a 75 mm complete round
  weighs 9–11 kg, so 30 rounds are about 300 kg and a full Panzer IV
  load (87) is close to a tonne. Tanks rearm only at the base, the HQ
  or a supply vehicle, and a full HQ shell stock (600 kg) refills about
  two-thirds of one tank, so tanks compete with mortars for it. Every
  round is paid for either way (D1): in base it is charged as it goes
  into the rack; at the HQ it was paid when the crates or truck load
  left the base. A tank far forward rearms only from a stock that
  someone brought there, so its rack is the limit on how far it pushes
  (D3).

### A.3 What keeps it honest

- **Every link can be shot.** Crates, HQ stock and truck cargo have hit
  boxes and HP. Shells and explosives cook off when HE breaks them. A
  small-arms crate is just lost.
- **Nothing is made from nothing**, except the base. Even the early
  supply before trucks is carried by men who walk from base (Q2 C).
- **Nothing is free** (owner decision D1). Every kg pays its kind's
  price once, when it leaves the base. No trickle of any kind exists.
- **No spam.** One HQ, three dumps, two aid posts and a live-crate cap
  per player, all in rules data.
- **Shells cost manpower** (owner decisions Q4, D1). There is no shell
  ration. Every shell, mortar bomb and tank round fired was paid for at
  base, as a card's starting load, a crate, a load or a rack refill. The
  price is the main HE lever (`artillery-on-map.md` §4.10).
- **Distance costs delivery, not a surcharge** (D2). A forward gun or
  tank is fed only by men and trucks that walked or drove from base or
  the HQ. Their time, exposure and losses are the price of pushing
  (`artillery-on-map.md` §4.11).
- **The HQ shortens the loop; it does not remove it.** Repair and new
  crew stay base-only. Rare rounds refill forward, but at 1 per 30 s and
  only up to the vehicle's cap (D3).
- **No reload aura.** Reload speed stays a man's hands and his weapon.

### A.4 Fit with today's base resupply

| Today | After |
|---|---|
| Items and rockets: 1 per 20 s in base, free | The base is a supply source with unlimited stock and those same rates; each item is charged at `explosives_mp_per_kg` (D1). HQ, dump, crates and vehicles run the same code with finite, already-paid stock. |
| Small arms, MG belts unlimited | Counted per man; the base refills them like items, at `small_arms_mp_per_kg`. |
| Mortar bombs unlimited | Counted per man. S5: the base refills them like items, at `shell_mp_per_kg`. From S6: the base refills none; bombs come in priced crates or from the HQ and dumps. |
| (no guns yet) | Infantry guns and howitzers (S11): ready inventory at the gun; refilled only by crates, HQ, dumps or the tow; never by the base. |
| Tanks rearm 1 round per 2 s in base, free | Same rate in base, each round charged at `shell_mp_per_kg`; half rate (1 per 4 s) at the HQ, a dump or a supply vehicle. Rare rounds: 1 per 10 s in base, 1 per 30 s forward, up to `rare_cap` (D3). |
| AT guns rearm in base like vehicles, free | Same as tanks, priced (D1). |
| Repair and crew refill in base | Unchanged. |

A match where nobody builds an HQ plays like today, except that rounds
and bombs now run out, every refill costs manpower, and from S6 mortar
bombs come in crates. Existing base-resupply tests keep their timings;
they gain a manpower check.

### A.5 The AI

- **Reserve:** each mortar and gun keeps a 30 % reserve for defence.
  Barrages only on targets above a score threshold. This alone should cut
  the mortar kill share.
- **Resupply trips:** a squad under 30 % with no enemy near goes to the
  nearest source; mortar bearers and gun crews fetch shells crates (HQ
  or dump first; the base only if none, and it costs manpower).
- **Shell budget:** the AI spends at most `ai.shell_mp_max_pct` (25 %)
  of its income on shells: crates, vehicle loads and tank rearm in base.
  Over budget, guns and mortars wait; a tank still rearms to
  `ai.tank_rearm_floor_pct` (50 %). Small arms and explosives are cheap
  and have no budget; they pause only when manpower runs out.
- **Tanks:** below `ai.rearm_pct` (25 %) a tank goes to the nearest
  stock it can reach (supply truck, HQ, dump, base). It waits for rare
  rounds only when no enemy is seen; it does not sit at an HQ for 5
  minutes topping up HVAP while the front needs it.
- **HQ:** built about 120 m behind the most valuable held point, on a
  reverse slope (a few ray probes for "no line of sight from the enemy").
- **Logistics:** a truck runs base → HQ → base when stock falls under
  40 % (S8). Before trucks, the AI sends its own men with crates (Q2 C:
  no automatic carrying party).
- **Without an HQ:** men walk back to base to refill. The AI must
  learn to send dry squads back in turns, not all at once, or a front
  empties; the S5 stall-guard test covers this.
- **Targeting:** enemy HQs and crates seen by the AI get a bonus as
  mortar targets.
- **Cost:** a few hundred serial operations every 2 s per AI player; no
  new rays except the HQ siting probe. The sim side is bounded too: each
  source serves at most 6 men per 500 ms, ammo counts add a few bytes
  per man (700 men at Battalion), and the crate and structure caps bound
  the new static boxes that fragment rays must test.

---

## Critic notes

Adversarial review, 2026-11-01, of this file against the repo (read
only) and the three area designs.

### What was changed

**Facts and history**

- **Mortar claim.** "Mortars get 2–4× the kills of small arms" is not
  in the logs. S3-log has mortars at 406 kills against 326 for rifles,
  and HE at 40–50 % of kills. The summary now quotes that.
- **HE baseline.** S5 and Q16 measured against "43 %" (S3). S3b's
  fragment rays change HE lethality and S3b's own start measured
  39.5 %, so the bar is now "a third below the post-S3b baseline,
  re-measured".
- **Panzerfaust.** The card was dated 1944-09. The Panzerfaust 30 was in
  wide use in Normandy; the 60 came about Sep 1944. The card is now
  from 1944-06 and swaps the 30 for the 60 by date.
- **Panther.** It was a medium tank by German class and about as common
  as the Panzer IV in 1944 panzer divisions. A shared one-heavy cap was
  an unhistorical availability rule. It now has its own cap of 2, an
  8:00 unlock and its high cost; Q17 keeps the old cap as option (B).
- **Truck payloads.** The Opel Blitz was a 3-tonne truck, not 1.5–2 t.
- **Tank ammunition weight.** "300 kg rearms a tank" misquoted the
  vehicle design, which says 300 kg is about 30 rounds. A full 75 mm load
  is close to a tonne; the text now says so and what that means for the
  HQ's 600 kg.
- **Dates.** M4A3E2 from 1944-10 added. M18 and SdKfz 234/1 given months.
  The M4A1 (76) W (first 76 mm Sherman, Jul 1944, listed in SYSTEM_DESIGN
  §2.3) added to "Later" with a note that it is a new hull.
- **Small points:** the German Bangalore is the *gestreckte Ladung*; the
  Zugtrupp radio is flagged (platoon radios were rare); the Cannon
  Company tow is flagged; "Sturmgruppe" is glossed as a squad of a VG
  *Sturmzug*.
- **Overclaim in the pushback.** Healing does not break the
  physical-contact rule; that rule is about damage. The real objections
  (unhistorical aura, endless manpower loop) now carry the argument.

**Owner decisions**

- **Trickle vs "walk to the front".** An abstract trickle into a forward
  HQ is ammunition appearing at the front. Q2 (A) is now an automatic
  two-man carrying party that spawns at base and walks to the HQ. Same
  effect, but physical and killable. The abstract trickle is (B).
  *Superseded: the owner chose Q2 (C), nothing. With priced shells (Q4)
  a free carrying party or trickle would be free HE, so neither may
  carry shells (`artillery-on-map.md` critic C18). Since D1 no kind of
  ammunition is free, so no trickle of any kind is allowed.*
- **On-map infantry guns.** The plan pushed back on on-map divisional
  howitzers because they reach the whole map from base, but then put the
  leIG 18 (≈3.5 km) and 105 mm M3 (≈7.6 km) on the map with free base
  resupply: the same trap. S11 is now blocked by Q4 (a per-player shell
  supply rate, B, or a manpower cost per shell, C). *Answered
  2026-11-01: no ration; the base refills no shells, and shells crates
  cost manpower at base.* Range compression
  is no longer an option: `artillery-on-map.md` §3.2 shows it does not
  stop fire from base.
- **Variant cooldowns.** With per-card cooldowns, the full and
  understrength Grenadiergruppe could be called alternately at twice
  the rate. `cooldown_group` (default: the root of the `extends` chain)
  was added; Q21 asks the owner.

**Scope and dependencies**

- **S4 "hashes equal" was impossible:** the same slice changes call-in
  behaviour. It now checks a resolved-def snapshot and equal hashes
  under the legacy call-in rules, then records new baselines.
- **S4 moved out:** StG 44 and the VG-Sturmgruppe (to S5: unlimited
  automatic fire would set a false baseline) and the M7 rifle grenade
  (to S11: it is new HE, and the plan holds new HE until S11; the German
  *Schießbecher* added for symmetry).
- **Sizes re-estimated:** S4 0.5→0.7, S5 0.7→1.0 (with an S5a/S5b split
  point), S10 1.0→1.5 (split into S10a/S10b). The totals are now about 5×
  S3 for S4–S9 and about 10× for all twelve, before art.
- **S12** needs only S6 for an aid tent; S10 only for the dugout.
- **USSR** needs a third small system: tank riders.
- **Art** is now named: each slice needs blockout models and icons, and
  S13 ("mostly data") has seven new models.

**Balance traps**

- **HQ spam and stock from nothing:** the HQ now starts empty, costs
  100 manpower plus 120 kg of material, is one per player, with three
  dumps, two aid posts and a live-crate cap (taken from the area design,
  where it was missing from this summary).
- **Healing loop:** a man is treated once per match; the evacuation
  refund is paid once and is below the replacement cost.
- **Forward rearm rates:** the truck rearmed tanks faster than the HQ (1
  per 3 s against 1 per 4 s). Both are now 1 per 4 s.

**AI and performance**

- Every slice now carries an AI-use check (S7–S14 had none or only some).
  S3 showed that features the AI ignores, such as satchels on tanks,
  never get balanced, and co-op PvE is a launch priority.
- Battalion tier: 700 soldiers shared by up to 10 players is 70 each,
  lower than either pop cap in Q6; now stated.
- Flamethrower particles need their own capped sub-pool so they never
  starve the 1,800-projectile Battalion pool.
- Downed men count toward pop and the 700 until they die or are
  evacuated.
- Crates and structures are bounded by caps, so fragment-ray box tests
  stay bounded.
- Hidden mines are a presentation rule in lockstep; a modified client can
  show them (as with all fog of war).

**Confusion**

- "From: base" in the roster tables read like the base zone. It is now
  `1944-06`.
- "HQ" is both a squad (Platoon HQ) and a structure. The structure should
  use *Gefechtsstand* / command post in the game.

### What remains debatable

- **Order (Q20).** The trench pillar's structures come seventh. Vehicles
  first is defensible (tows, one schema for three slices), but S10a
  needs only S6 and could go right after it.
- **Panther cap.** Taking it out of the heavy slot is more historical,
  but at 600–700 m spotting (Q12 B) it may dominate. The fallback is Q17
  (B).
- **Carrying party.** It costs a little more code than a trickle (an
  automatic order and a no-capture rule). If the owner prefers
  simplicity, (B) is acceptable as long as it is switched off once trucks
  work. *Superseded by Q2 (C): no carrying party, no trickle.*
- **Squad numbers** (German 9 or 10 in 1944, the US M7 count, the sMG
  and mortar crews), the M4A3 (76) W arrival month, the 57 mm HE date and
  the 234/1 service date still need sources.
- **Squad sizes vs pop.** Historical sizes with a 70-soldier share in
  5v5 lobbies may feel thin. Q6 (C), 8 men everywhere, is the
  gameplay-first answer; this plan still recommends (A).
- **Fire discipline** (hold fire beyond 70 % of range) is a behaviour
  change players will notice; it may need a visible "hold / free fire"
  toggle.
- **Officer rally (Q8)** and **red cross (Q14)** are unchanged; they are
  owner calls.
