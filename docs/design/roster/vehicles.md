# Roster design: vehicles beyond the two current tanks

Area: light armoured cars and recon, half-tracks and transport, more
tanks, tank destroyers and assault guns per faction, and the systems they
need. Includes the vehicle side of the owner's HQ follow-up (ammo crates,
resupply radius, the ammo model).

Status: design proposal, 2026-11-01. Nothing here is built. Read against
`SYSTEM_DESIGN.md` §2.3, §2.4, §5.7, §5.9, §5.10, §5.12, §7 and
`docs/slices/S3-armour.md` / `S3-log.md`. **Every number is an approximate
public figure or a starting value for tuning.** Armour, gun and production
figures must get real `sources` before they go into TOML (the same rule
S3 used). Entries marked *(verify)* are the ones I am least sure of.

**Owner decisions D1–D3 (2026-11-01)** bind the ammo parts below: no
free ammo, the base included (D1); distance costs delivery, not a
surcharge (D2); tanks rearm at base, HQ, dump or supply truck, with rare
rounds refilled forward too, slowly, up to a per-vehicle cap of at most
10 (D3). See §6.11, §7.3 and §7.6, and the roadmap's "Owner decisions".

---

## 1. Summary

- **Build systems, not vehicles.** Each new vehicle is mostly data once
  its systems exist. Five systems unlock almost the whole list:
  (1) a general vehicle schema (no turret, open compartments, more boxes,
  seats and ammo types), (2) casemate gunnery, (3) small arms against thin
  armour, (4) open-top exposure, (5) transport (embark, riders). Wheeled
  mobility, towing, recon optics and APCR/HVAP are smaller add-ons.
- **First vehicles to add:** StuG III Ausf. G and M10 (casemate and open
  top, already planned for S3b), then SdKfz 251/1 and M3A1 half-tracks
  with the transport system, then SdKfz 234/1 and M8 Greyhound, then the
  heavies (Panther G, M4A3 (76) W, M36, Tiger I, M4A3E2).
- **Panther and Tiger against Shermans:** do not touch their armour or
  guns. Balance them by cost, pop, a per-type cooldown, a cap of one alive
  in a shared "heavy" group, a late per-type unlock, and the counters the
  other side really had (M36, HVAP, M18 flanking, air power later).
- **No fuel resource.** Keep a fuel tank as a damageable component (fire
  risk) only.
- **APCR/HVAP (owner decision D3):** a rare ammo entry, capped per
  vehicle (`rare_cap`, at most 10; lower where history says so, e.g. 2
  HVAP on the M4A3 (76) W). It refills at the base and at forward
  points, but slowly. The cap and the slow refill are the balance.
- **Ammo model for vehicles:** main-gun rounds per type (already counted),
  MG belts counted, all in TOML. **No round is free (D1):** each costs
  its kg at `shell_mp_per_kg` times the vehicle's rack price factor (MG
  belts at `small_arms_mp_per_kg`). A full rack refill costs at most
  75 % of the card (critic pass 3).
  Kg are paid once, when they leave the base. Resupply flows from the base
  (fast, everything), an HQ or dump radius (half rate, rare rounds
  slowly) and supply vehicles carrying a stock in kg (trucks,
  half-tracks). Tank shells are too heavy for man-carried crates: tanks
  rearm from a vehicle, an HQ, a dump or the base. The rack is the limit
  on how far a tank can push (§7.6).
- **USSR transport:** mostly tank riders (*tankodesantniki*) and
  Studebaker trucks, not half-tracks. Lend-lease half-tracks were few and
  went to recon, command and AA roles.

## 2. Honest pushback

1. **Light vehicles before small-arms penetration would be a mistake.**
   Today a rifle or MG round that hits a vehicle box stops with no
   damage (S3 "Small arms on vehicles"). That is right for a Panzer IV and
   wrong for 6–8 mm half-track sides and 10 mm armoured-car plates.
   Rifle-calibre AP (German SmK, US M2 AP) went through those at short
   range, and a .50 or 2 cm went through at any battle range. Without
   this, an M8 or BA-64 is an unkillable infantry hunter for anyone
   without an AT weapon. That breaks the "physical contact" rule in the
   other direction: real contact that should hurt does nothing. Small
   arms vs armour and open-top exposure must ship **with or before** the
   first light vehicle.
2. **Do not build the Tiger first.** It is the least representative
   vehicle on the list: 1,347 built, production ended in August 1944, and
   a few heavy battalions per front. It is also the hardest to balance.
   The StuG III G is the most-produced German AFV of the period, and
   building it brings the casemate system that also gives the Jagdpanzer
   IV, Jagdpanzer 38(t), SU-85 and SU-100 almost for free.
3. **Half-tracks shorten the walk to the front. That is their purpose,
   and it is acceptable.** The owner's rule is that units spawn at the
   base and walk. A half-track spawns at the base too, and must drive back
   for every load. What it buys is speed for a squad, at the risk of
   losing 10 men to one AT hit. Keep it, but gate it with cost and a cap
   (§7). Do **not** add a "squad arrives already mounted" call-in: it would
   make every squad faster for free.
4. **A forward HQ must not rearm everything instantly.** It would remove
   the trip back to base, which is the logistics game. The HQ resupplies
   at half the base's rate, refills rare ammo only slowly and up to a
   small cap (D3), and draws on a paid stock that men and trucks brought
   forward (§7). Vehicles still spawn only at the base.
5. **Fuel adds micro without history.** Battles here last 15–25 minutes
   over 1 km. A Panzer IV's road range was about 200 km. Nothing ran dry
   in a battle of this size; the German fuel crisis was strategic. It
   belongs in availability and cost (fewer Panthers), not in a fuel bar.
6. **"Light armoured vehicles" are recon vehicles, not assault vehicles.**
   Historically they scouted, screened and avoided fights. If the game
   rewards driving them into infantry, the design is wrong. With
   small-arms penetration, grenades into open tops and real trench
   obstacles (§6.8), they will play as recon by themselves.
7. **Pz IV Ausf. J looks like a trap in our current balance.** It differs
   from the H mainly by hand-only turret traverse. The S3 log shows the
   turret race decides most tank duels. A J would be a strictly worse,
   slightly cheaper H. Keep it as a data-only, date-accurate variant for
   missions, not a skirmish card.
8. **Name things by period terms.** Germans: *Panzergrenadiere
   (gepanzert)* in SdKfz 251s, *Sturmgeschütz* (assault gun), *Panzerjäger*
   (tank destroyer units). US: *armored infantry* in M3s, *tank destroyer
   battalions*. Soviets: *tankodesantniki* (tank riders), *SU* =
   *samokhodnaya ustanovka* (self-propelled mount). "Hetzer" was rarely
   used during the war; the official name is Jagdpanzer 38(t). Use
   `name = "Jagdpanzer 38(t)"` and `short_name = "Jagdpanzer 38"`, and
   mention "Hetzer" only in descriptions.
9. **The vehicle list is bigger than the slice cadence.** S3 alone was
   the biggest slice so far. Ship the systems in waves (§9), each wave
   with 2–4 vehicles per faction, and resist adding vehicles that need a
   system that does not exist yet.
10. **Today's call-in model contradicts the owner decision.** S3 has
    `call_in_s = 45` and "at most 4 vehicles alive **or queued**". The new
    rule is immediate spawn with a per-type cooldown and no queue. This
    must change before any new vehicle (§7.1).

---

## 3. What exists today (constraints from the code)

From `sdk/crates/content/src/vehicle.rs`, `crates/sim/src/vehicle.rs` and
S3:

| Today | Limits for new vehicles |
|---|---|
| One hull box + one turret box, each face one plate (`[armor]` 8 zones) | No casemate (turret is required), no open compartments, no MG shields, no cab-plus-compartment layout |
| `SeatRole` enum: driver, gunner, commander, loader, radio_mg; crew are seats, not soldiers | No passengers, no MG gunner on a ring mount, no exposed seats |
| `ComponentKind`: all 6 components on every vehicle (engine, 2 tracks, gun, turret ring, ammo rack) | No wheels, no turretless vehicles, no unarmed trucks |
| `MountKind`: turret_main (exactly one), coax, hull; `MAX_MOUNTS = 4` | No casemate gun, no pintle or ring MG |
| Main gun ammo: at most one penetrator and one HE entry | No APCR/HVAP, no smoke shells |
| `AmmoMechanism`: ap, aphe, he, heat | `apcr` is named in §2.4 but not in code |
| `VehicleRole`: tank; `MobilityKind`: tracked | No wheeled or half-track kinematics |
| Small arms stop on any vehicle box, no damage, no event | Thin-skinned vehicles are immune to rifles and MGs |
| Blasts reach tracks only, never crew of a closed vehicle | No open-top exposure |
| Base zone: rearm 1 round / 2 s, repair 2 %/s, 1 crew seat / 20 s | The only resupply source |
| Vehicles unlock at 240 s, at most 4 alive or queued, `call_in_s` | Conflicts with the no-queue, per-type cooldown decision |

The S3 design choice that crew are **seats with boxes**, not soldier
entities, is good and should stay for crew. Passengers are a different
case (§6.5).

---

## 4. Vehicle catalogue (historical data, per faction)

Columns: approximate armour in mm (front / side, with slope from vertical
where it matters), main armament, speed (road, km/h), crew, and when it
fits our Jun 1944 – early 1945 window. Penetration rows for new guns are
in §4.5.

### 4.1 Germany

| Vehicle | Role | Armour front / side | Armament | Road km/h | Crew | Window and numbers |
|---|---|---|---|---|---|---|
| SdKfz 222 | 4×4 armoured car | 14.5–30 (late) / 8 | 2 cm KwK 30/38 + MG 34, open turret with wire grenade screens | 80 | 3 | Still in use, being replaced by the 250/9; ~990 built to 1943 |
| SdKfz 250/9 | light half-track recon | 14.5 / 8 | Same 2 cm open turret as the 222 | 60 | 3 | 1943–45; the recon workhorse of 1944 *(verify count)* |
| SdKfz 234/1 | 8×8 heavy armoured car | 30 @ 35° / 8–10 | 2 cm KwK 38 + MG 42, open turret with screens | 80 | 4 | From Jun 1944; ~200 |
| SdKfz 234/2 "Puma" | 8×8, closed turret | 30 / 8–10 | 5 cm KwK 39/1 L/60 | 80 | 4 | Sep 1943 – Sep 1944; ~100. Rare |
| SdKfz 234/3 | 8×8 close support | 30 / 8–10 | 7.5 cm K 51 L/24, open | 80 | 4 | From Jun 1944; ~90 |
| SdKfz 251/1 Ausf. D | APC half-track | 14.5 / 8 | 1–2 MG 34/42 (front behind a shield) | 52 | 2 + 10 | Throughout; ~15,000 of all 251s |
| SdKfz 251/9 "Stummel" | support half-track | as 251 | 7.5 cm K 37 L/24 (HE support) | 52 | 3 | Throughout |
| SdKfz 251/7 | engineer half-track | as 251 | MG, assault bridges carried | 52 | 2 + 7 | Throughout. Fits "heavy engineers" |
| SdKfz 251/8 | ambulance | as 251 | none | 52 | 2 + 4 litters | Fits "medical points" |
| SdKfz 251/22 | tank destroyer | as 251 | 7.5 cm PaK 40 | 50 | 4 | Dec 1944 onward |
| Panzer IV Ausf. H | medium tank | 80 / 30 | 7.5 cm KwK 40 L/48 | 38 | 5 | **In game** |
| Panzer IV Ausf. J | medium tank | 80 / 30 | 7.5 cm KwK 40 L/48 | 38 | 5 | From Jun 1944; hand-only turret traverse |
| StuG III Ausf. G | assault gun (casemate) | 80 (50+30) / 30 | 7.5 cm StuK 40 L/48, traverse about ±10° | 40 | 4 | Throughout; ~8,000 G. Height about 2.2 m |
| Jagdpanzer IV | tank destroyer (casemate) | 60 → 80 @ 45–50° / 30–40 | 7.5 cm PaK 39 L/48 | 40 | 4 | From early 1944; ~770. The L/70 Panzer IV/70 from Aug 1944 |
| Jagdpanzer 38(t) | light tank destroyer (casemate) | 60 @ 60° / 20 @ 40° | 7.5 cm PaK 39 L/48, traverse about 5° left, 11° right | 42 | 4 | From Jul 1944; ~2,600 by May 1945. Only 41 rounds |
| Panther Ausf. A | medium (heavy class) tank | 80 @ 55° / 40 | 7.5 cm KwK 42 L/70 | 46 | 5 | Late 1943 – mid 1944; ~2,200 |
| Panther Ausf. G | as above | 80 @ 55° / 50 @ 30° upper | 7.5 cm KwK 42 L/70 | 46 | 5 | From spring 1944; ~2,950 |
| Tiger I (late) | heavy tank | 100 / 80 | 8.8 cm KwK 36 L/56 | 38–45 | 5 | Production ended Aug 1944; 1,347 in all |

Note for later: Tiger II (from Jul 1944, 492 built) and Panzer IV/70 are
in the §2.3 roster but not in this task.

### 4.2 USA

| Vehicle | Role | Armour front / side | Armament | Road km/h | Crew | Window and numbers |
|---|---|---|---|---|---|---|
| M8 Greyhound | 6×6 armoured car | turret 19, hull 16–19 / 10; floor about 6 | 37 mm M6 + .30 coax + .50 on a ring, **open-top turret** | 89 | 4 | Throughout; ~8,500 |
| M20 | 6×6 utility car | as M8, no turret | .50 on a ring mount, open hull | 89 | 2 + up to 4 | Throughout; ~3,700. Command, recon, ammo carrier |
| M3A1 half-track | APC | 12.7 front / 6.35 sides (face-hardened) | .50 on an M49 ring over the cab | 72 | 1–2 + 11 | Throughout; the main armored-infantry carrier |
| M4A3 (75) | medium tank | 51 @ 56° (63.5 @ 47° on later hulls) / 38 | 75 mm M3 | 42 | 5 | **In game** |
| M4A3 (76) W | medium tank | 63.5 @ 47° / 38 | 76 mm M1A1 | 42 | 5 | In ETO from about late summer 1944 *(verify month)*; the M4A1 (76) W was first, Jul 1944 |
| M4A3E2 "Jumbo" | assault tank | 101.6 @ 47° / 76; turret 152 | 75 mm M3 (some re-armed with 76 mm in the field) | 35 | 5 | From Oct 1944; **254 built** |
| M10 (3-inch GMC) | tank destroyer, open-top turret | 38 @ 55° / 19–25; turret 57 | 3-inch M7 | 51 | 5 | Throughout. Hand-traversed turret, slow |
| M18 Hellcat | tank destroyer, open-top turret | 13 hull, 25 turret / 13 | 76 mm M1A1C | 89 | 5 | ETO from summer 1944; ~2,500. Fastest tracked AFV of the war |
| M36 Jackson | tank destroyer, open-top turret | 38 @ 55° / 19–25; turret 76 | 90 mm M3 | 48 | 5 | ETO from Sep 1944; ~1,400. Hand traverse |

Also in §2.3 but not in this task: M4A3E8 (Dec 1944), M5A1, M24 (Dec
1944).

### 4.3 USSR (not built yet; plan only)

| Vehicle | Role | Armour front / side | Armament | Road km/h | Crew | Window and numbers |
|---|---|---|---|---|---|---|
| BA-64B | 4×4 light armoured car | 12–15 sloped / 9–10 | 7.62 mm DT, open-top turret | 80 | 2 | Throughout; ~9,000 BA-64 in all |
| M3A1 Scout Car (lend-lease) | open-top scout car | 12.7 / 6.35 | .50 + .30 on a skate rail | 80 | 2 + 6 | About 3,000 to the USSR *(verify)*; recon and command |
| M3 / M5 half-tracks (lend-lease) | APC, tractor | 6.35–8 | .50 | 64–72 | 1–2 + 10 | Roughly a thousand *(verify)*; few compared with US units. M17 MGMC (quad .50 on M5) for AA |
| Studebaker US6 (lend-lease) | truck | none | none | 70 | 1–2 + cargo | Very common (over 100,000 *(verify)*); towed guns, carried infantry, mounted Katyushas |
| T-34-85 | medium tank | 45 @ 60° / 45 @ 40°; turret 90 | 85 mm ZiS-S-53 (early: D-5T) | 55 | 5 | From spring 1944; the main tank |
| IS-2 (1944) | heavy breakthrough tank | 100–120 @ 60° / 90; turret 100 | 122 mm D-25T, **28 rounds**, separate loading (2–3 rounds/min) | 37 | 4 | From spring 1944 |
| SU-85 | tank destroyer (casemate) | 45 @ 50° / 45 | 85 mm D-5S, about ±10° | 55 | 4 | 1943–44; replaced by the SU-100 |
| SU-100 | tank destroyer (casemate) | 75 @ 50° / 45 | 100 mm D-10S, 33 rounds | 50 | 4 | Front from about Jan 1945 *(verify)*. Late-war only |
| SU-76M | light SPG, open-top | 25–35 / 10–15 | 76 mm ZiS-3Sh | 45 | 4 | Throughout; ~14,000. Infantry support |

Also in §2.3: T-34/76 (1943), ISU-152, lend-lease M4A2 (76) W.

### 4.4 Turret traverse matters most

The S3 log found that turret traverse decides most tank duels (the
Sherman's 24°/s against the Panzer IV's 14°/s). So traverse rates are the
most important numbers in this catalogue. They must come from sources,
not feel.

| Vehicle | Drive | Approximate rate *(verify all)* |
|---|---|---|
| Panzer IV H | electric (auxiliary generator) | 14°/s (in game) |
| Panzer IV J | hand only (two speeds) | well under half the H's rate |
| Panther A / G | hydraulic, from the engine | about 8°/s at idle, about 20°/s with the engine revved |
| Tiger I | hydraulic, from the engine | about 6°/s at idle, faster at high revs |
| M4 series | electro-hydraulic | about 24°/s (in game) |
| M10, M36 | hand only | about 4–5°/s (a full turn took over a minute) |
| M18 | hydraulic powered | fast, close to the Sherman |
| T-34-85 | electric | fast *(verify)* |
| IS-2 | electric | moderate *(verify)* |

Engine-driven hydraulic traverse needs one small mechanic: `traverse_deg_s`
when moving or revving, `traverse_idle_deg_s` when stationary at idle,
and an AI rule that revs the engine (louder: a short spotting reveal by
noise later) when it needs a fast traverse. Size S.

### 4.5 New guns and ammo (penetration in mm against a plate at 30°)

S3 tables are against 30°, as published tables are. Ranges 100 / 500 /
1000 m. All approximate, from public tables; verify before use.

| Gun | Round | Mechanism | m/s | 100 m | 500 m | 1000 m |
|---|---|---|---|---|---|---|
| 2 cm KwK 38 | PzGr 40 | apcr | 1,050 | about 40 | about 25 | — |
| 3.7 cm US M6 (M8) | M51 APC | ap | 884 | about 61 | about 53 | about 46 |
| 5 cm KwK 39/1 L/60 (Puma) | PzGr 39 | aphe | 835 | 67 | 57 | 44 |
| 7.5 cm KwK 42 L/70 (Panther) | PzGr 39/42 | aphe | 925 | 138 | 124 | 111 |
| 7.5 cm KwK 42 | PzGr 40/42 | apcr | 1,120 | about 194 | about 174 | about 149 |
| 8.8 cm KwK 36 L/56 (Tiger) | PzGr 39 | aphe | 773 | 120 | 110 | 100 |
| 76 mm M1 / 3-inch M7 | M62 APC | aphe | 792 | about 99 | 93 | 88 |
| 76 mm M1 | M93 HVAP | apcr | 1,036 | about 155 | about 145 | about 130 |
| 90 mm M3 | M82 APC | aphe | 853 | about 129 | 120 | 112 |
| 90 mm M3 | T30E16 HVAP | apcr | 1,021 | about 220 | about 210 | about 190 |
| 85 mm ZiS-S-53 | BR-365 | aphe | 792 | about 95 | about 87 | about 78 |
| 122 mm D-25T | BR-471 | aphe | 795 | about 140 | about 130 | about 120 |
| 100 mm D-10S | BR-412 | aphe | 895 | about 150 | about 140 | about 125 |
| 76 mm ZiS-3 | BR-350A | aphe | 662 | about 65 | about 58 | about 50 |

Small arms that matter against thin armour (penetration at 0°, about):
.30-06 M2 AP 10–12 mm at 100 m; 7.92 mm SmK 10–13 mm at 100 m; .50 M2 AP
about 25 mm at 100 m and 19 mm at 500 m; 2 cm PzGr 40 about 40 mm at
100 m. Sources disagree; take one source per round and cite it.

---

## 5. Cost-balanced realism for Panther and Tiger against Shermans

The owner's rule: authentic stats, balance by cost, availability and AI,
and add historical options before changing stats. Applied here:

**What decides a Panther vs Sherman fight, by physics.** A Panther's
front (80 mm at 55°) defeats the 75 mm M61 and usually the 76 mm M62 at
battle ranges; its sides (40–50 mm) do not stop either. Its turret is
slow at idle. So the authentic Sherman answer is: two or three Shermans,
one fixes it, one flanks. The authentic US options against heavies are
the M36 (90 mm), HVAP for 76 mm guns, the M18's speed for flanking, the
M4A3E2 to soak hits, and (later) artillery and aircraft. That is a
complete counter-play set without touching a single armour value.

**Spotting range changes the Panther's value.** S3 owner question 2
chose (A): tanks see tanks at about 350 m. At 350 m the Panther's long gun
advantage over 1,000 m never comes up; only its front armour does. At
600–700 m (option B) it would dominate. That owner question should be
answered before balancing heavies, and it is a better lever than any
stat.

**Levers, all TOML:**

| Lever | Field | Effect |
|---|---|---|
| Cost | `cost` | 1,000–1,250 manpower for Panther / Tiger vs 560 for a Sherman: about two Shermans each |
| Pop | `pop` | 15–16, so a heavy crowds out infantry |
| Per-type cooldown | `cooldown_s` | 240–300 s between calls of the same type (owner decision: cooldown, no queue) |
| Cap alive, per type | `max_alive` | 1 for Tiger, Panther, M4A3E2, Puma |
| Shared heavy group | `limit_group = "heavy"` + rules `[vehicles.groups] heavy = 1` | Panther **or** Tiger on the field, not both |
| Unlock by match time | `unlock_s` (per type, overrides the rules default) | Heavies from 10–12 min, light vehicles from 1–2 min |
| Date filter | `available_from` / `available_to` | Missions and "period" lobbies get the right mix (no Tiger I in Jan 1945 West, no M36 in Jun 1944) |
| Ammo scarcity | `rare_cap` and ammo `count` | HVAP 2 per M4A3 (76) W, PzGr 40/42 3 per Panther; forward refill 1 per 30 s (D3, §6.11) |
| Bog risk | `bog_pct` | Heavier tanks bog more in trenches and soft ground (authentic and a real cost) |
| AI | buy rules | The AI fields heavies only when it has seen enemy armour, and keeps them back as overwatch |

**What not to do:** lower Panther front armour, raise Sherman
penetration, add a hidden "damage multiplier". If a heavy is still too
strong after the levers above, the next historical option is a better
counter (M36 earlier, more HVAP), not a weaker heavy.

**Watch the opposite problem.** In S3, the Sherman beats the Panzer IV
26 of 30 times because of traverse. Shermans may also beat Panthers more
than history suggests if they flank with a 24°/s turret against a
Panther at idle. Measure it in `armour-test` before tuning anything.

---

## 6. Systems needed

Each system: what it is, player value, size (S = days, M = about a
slice-week, L = a slice of its own), dependencies, sim modules touched,
and TOML shape. Sizes are relative to S3.

### 6.1 Call-in rework: immediate spawn, per-type cooldown, caps (S)

Owner decision: no queue; a unit spawns at the base at once and walks;
each unit type has a cooldown after a call.

- **Value:** required by the owner decision; also the main balance tool
  for heavies.
- **Change:** `call_in_s` becomes `cooldown_s` (the cooldown starts when
  the unit spawns). `[vehicles] limit` counts alive vehicles only. Add
  per-type `max_alive`, `unlock_s`, `limit_group`, and rules
  `[vehicles.groups]`. Replace the single limit of 4 with per-group caps:
  4 for armour (tanks, assault guns, TDs) was fine in S3, but transports,
  cars and trucks need their own cap or a player can't field both.
- **Touches:** `world.rs` (spawn), `game.rs` (cooldown state per player
  per type: saved and hashed), `ai.rs` (buy), content `vehicle.rs`,
  `defs.rs`, `setup.rs`, HUD call-in tiles (cooldown ring instead of the
  queue bar). `PROTOCOL_VERSION` unchanged (same `CallIn` command),
  `SAVE_VERSION` bump.

```toml
# defs/vehicles/ger_panther_g.toml (excerpt)
cost = 1100
pop = 16
cooldown_s = 240
max_alive = 1
limit_group = "heavy"
unlock_s = 600
available_from = "1944-03"
available_to = "1945-05"

# defs/rules/skirmish.toml (excerpt)
[vehicles.groups]
armour = 4      # tank, assault_gun, tank_destroyer
heavy = 1       # also counts against armour
light = 3       # armoured_car, halftrack, truck
```

### 6.2 General vehicle schema (M)

The S3 schema is shaped for one turreted tank. Generalise it once:

- **Boxes:** up to 4 armour boxes per vehicle (hull, superstructure or
  compartment, turret, optional extra), each with its own 8-zone armour
  table and an optional `open` face. A half-track is a closed cab box
  plus an open compartment box; a StuG is a hull plus a casemate box; an
  M10 is a hull plus an open-top turret box.
- **Optional turret:** `[turret]` optional; no turret means no
  `turret_ring` component and the main gun is a `casemate` mount.
- **Components by layout:** each component optional except the engine.
  Add `wheels_front` / `wheels_rear` (wheeled), `fuel_tank` (fire source
  only, §6.12), `cargo` (supply vehicles, detonates like an ammo rack).
- **Seats as a list** with an id, a role and a box: roles gain
  `mg_gunner` and `passenger`; a seat box may sit partly or fully outside
  the armour (an exposed commander, a ring-mount gunner, a rider).
- **Mounts:** `turret_main`, `casemate`, `coax`, `hull`, `pintle` (ring
  or pedestal MG, own arc and traverse, exposed gunner). `MAX_MOUNTS`
  goes to 6.
- **Ammo list:** up to 4 entries per main gun (AP, APCR, HE, smoke).
- **Shields:** small armour-only boxes (MG shields, gun shields) with one
  thickness, no contents.
- **Value:** none by itself; it unlocks everything else. Do it once, early,
  and bump `SAVE_VERSION` once.
- **Touches:** content `vehicle.rs` (enums become data-driven lists with
  the same validation style), `setup.rs`; sim `vehicle.rs`, `armour.rs`
  (`hit_armour` loops over boxes), `damage.rs` (internal boxes per
  penetration: raise the 14-box cap to about 24 for passengers),
  `gunnery.rs`, `snapshot.rs`, `hash.rs`; the bridge and presenter
  (`vehicle_view.gd` draws boxes per vehicle). The §5.7 armour-mesh path
  (M2) stays: zones keep their names and `hit_armour` stays the single
  narrowphase.

```toml
[[boxes]]
id = "cab"
frame = "hull"
box = { center = [150, 0, 120], half = [130, 105, 60] }
[boxes.armor]
front = { thickness_mm = 14, slope_deg = 30 }
side  = { thickness_mm = 8,  slope_deg = 10 }
top   = { thickness_mm = 8 }

[[boxes]]
id = "compartment"
frame = "hull"
box = { center = [-150, 0, 120], half = [170, 105, 60] }
open = ["top"]                 # no plate: rounds, fragments, grenades enter
floor_cm = 40                  # grenades come to rest here
[boxes.armor]
side = { thickness_mm = 8, slope_deg = 15 }
rear = { thickness_mm = 8, slope_deg = 20 }
```

### 6.3 Casemate gunnery (M)

- **What:** a gun with a limited traverse arc in the hull (StuG ±10°,
  Jagdpanzer 38(t) 5° left / 11° right). Outside the arc the driver turns
  the hull (`hull_assist`, §5.12.2), then the gunner lays within the arc.
  The AI and the RTS path follower use the same `drive` and `lay_gun`.
- **Value:** high. StuG III G (Germany's most numerous AFV), Jagdpanzer
  IV, Jagdpanzer 38(t), SU-85, SU-100, ISU-152 all come almost as data.
  The real trade-off (low silhouette and thick front vs. slow to switch
  targets, weak when flanked) appears by itself.
- **Low silhouette is free:** our spotting already has a size factor
  (`spot_size_pct`, 200 % for tanks). A StuG at about 2.2 m high gets about
  160 %, a Jagdpanzer 38(t) about 150 % *(starting values)*. Spotted
  later, so it ambushes well: authentic and no new code.
- **Dependencies:** §6.2. **Touches:** `gunnery.rs` (arc check, aim
  choice with hull turn), `vehicle.rs` (pivot-to-lay: small heading
  corrections without overshoot, since neutral steer is coarse), `ai.rs`
  (overwatch: face the threat axis, relocate after firing),
  `armour-test` (casemate duel and flank cases).

```toml
[[weapons]]
def = "ww2:stuk40_l48"
mount = "casemate"
pivot = [120, -20, 150]        # hull frame
muzzle = [330, 0, 0]           # from the pivot
traverse_deg = [-10, 10]       # left, right
elevation_deg = [-6, 20]
ammo = [{ id = "ww2:75mm_pzgr39", count = 30 }, { id = "ww2:75mm_sprgr34", count = 24 }]
```

### 6.4 Small arms against thin armour (S–M)

- **What:** small-arms rounds that hit a vehicle box run the same
  `armour::penetrates` test as shells, with a per-weapon penetration
  table. A penetrating bullet continues for a short core length and hits
  crew, passenger or component boxes (low damage, no spall). A
  non-penetrating hit stops, as today, with no event. Only penetrations
  emit events, so MG bursts do not flood the feed (the S3 reason for the
  rule).
- **Ammo share:** German MG belts carried a share of SmK (AP) rounds; US
  MGs were issued M2 AP. Model this as the weapon's pen table (simple) or
  as a belt mix fraction (one roll per round). Simple first.
- **Targeting:** small arms still ignore tanks. A soldier may target a
  vehicle when (a) an exposed occupant box is visible (a commander out of
  the hatch, a ring-mount gunner, riders, men in an open compartment), or
  (b) its weapon can beat the facing plate at that range. Precompute (b)
  per (weapon, vehicle type, zone) at load as a max range, so the per-man
  check is one comparison.
- **Value:** essential before light vehicles (§2 point 1). It also makes
  .50 cal and 2 cm guns meaningful, and the M3's reputation for thin
  armour emerges by physics.
- **Touches:** `combat.rs` (small-arms vehicle hit path), `armour.rs`
  (same function, flat or short tables), `damage.rs`, weapon defs,
  `armour-test` (MG vs half-track at 50 / 150 / 300 m), budgets
  (`armour_box_tests`; S3's max was 42 per tick, expect a few hundred in
  a fire fight around a half-track: still cheap, but measure).

```toml
# defs/weapons/m2hb.toml (excerpt)
class = "mg"
# [range_m, mm] at 0°, M2 AP
penetration = [[0, 28], [100, 25], [500, 19], [1000, 11]]
core = { damage = 60, length_cm = 60, max_boxes = 1 }
```

### 6.5 Open-top exposure (M)

- **What:** an `open` face has no plate. A round, fragment or thrown item
  whose segment enters through it continues into the box and tests
  everything inside (crew seats, passenger slots, components). HE that
  bursts inside an open compartment (a grenade on the floor, a mortar
  round falling in) damages every box inside within its radius, with the
  sides holding the blast in. Fragments from bursts outside reach boxes
  inside by line of fire over the open edge: the S1 burst code already
  tests a line from the blast to each body point; for open compartments
  the crew and passenger boxes are added as targets, with the side plates
  as blockers (a fragment with `pen_mm` below the plate stops). Tree
  bursts and mortar near-misses therefore hurt M10 and 251 crews,
  as they did.
- **Grenade screens:** the 222, 234/1 and 250/9 had hinged wire-mesh
  screens over the open turret. A face flagged `grenade_screen` stops
  thrown items (they bounce off and fall beside the vehicle) but not
  bullets or fragments.
- **Throwing into a vehicle:** uses the S3b `Throw` at a unit. The throw
  aims at the compartment centre; the lob is physical, so against a
  moving vehicle it usually misses, against a stopped one it often
  lands. No "throw succeeds" roll.
- **Value:** high. It gives every open vehicle (M10, M18, M36, SU-76M, all
  half-tracks, M8 turret, M20, scout cars) its real weakness: infantry
  close up and indirect fire. It also gives engineers and riflemen a
  counter to tank destroyers without AT weapons.
- **Dependencies:** §6.2. **Touches:** `armour.rs`, `combat.rs` (burst
  fragments into compartments), `items.rs` (landing inside, screens),
  `damage.rs`. `armour-test`: grenade into a stopped 251 kills the
  passengers; screened 234/1 turret survives a grenade; mortar near-miss
  wounds M10 crew.

### 6.6 Transport: embark, disembark, riders (L)

The biggest system. It is also what makes half-tracks worth having.

**Model:** passengers stay **soldier entities** (they have their own HP,
items, weapons, squad and orders history), with a `Riding { vehicle,
slot }` state. Crew stay seats. While riding:

- the soldier's hit volume is the **slot box** in the vehicle frame
  (seated, so lower than a standing capsule). The vehicle narrowphase
  already tests boxes; a hit on a slot box damages that soldier. A slot
  box that rises above the side plates (heads and shoulders in a 251)
  is exposed to direct fire; one inside is reached only through the
  armour or the open top;
- movement, ORCA, pathing and formation skip him; his transform follows
  the slot;
- he is an **observer** from the slot's eye point (riders do see);
- he does not fire in v1. Later, an open compartment can allow "fire from
  the vehicle" while stationary: the men stand, their boxes rise, they
  fire over the side and are more exposed. Data flag, off by default.

**Commands** (new, `PROTOCOL_VERSION` bump): `Embark { squads, vehicle }`
(walk to the entry point, board one man per `board_s`),
`Disembark { vehicles, toward? }` (exit at the door point one man per
`exit_s`, or over the sides faster where the def allows). A selected
embarked squad accepts only `Disembark`; move orders go to the vehicle.

**Knock-out and fire:** on penetration, passenger slot boxes take core,
spall and filler hits like crew boxes. When the vehicle is knocked out
or burning, all surviving passengers dismount at once (stunned 2 s).

**Riders (tank desant):** `[[rider_slots]]` on a tank's engine deck. Same
`Riding` state, fully exposed boxes. Riders dismount automatically when
suppressed past `crouch_suppression` (suppression only from real
near-misses, so this is physical). This is cheap once embark exists and
gives the USSR its authentic transport (§8.3), and any faction a way to
move infantry with tanks.

**Value:** high. Panzergrenadiers and armored infantry were defined by
their vehicles. It also creates real decisions: dismount out of sight,
or drive closer and risk a 10-man loss.

**Exploit checks:**
- "Walk to the front" (§2 point 3): the transport spawns at base and has
  to shuttle. Measure the time saved per squad on Kessel Ridge; if it is
  too large, raise the transport's cost or lower its cap, not its speed.
- Capture points: embarked men do not capture or contest.
- Pop: passengers count as soldiers, as before.

**Touches:** `unit.rs`, `squad.rs`, `game.rs` (riding state, saved and
hashed), `orders.rs`, `command.rs`, `combat.rs` / `damage.rs` (slot
boxes), `spotting.rs` (riders as observers), `vehicle.rs` (escort and
riders interplay), `ai.rs`, the bridge (riders drawn seated), HUD (a
vehicle card shows its passengers; a squad card shows "mounted").

```toml
[transport]
capacity = 10
entry = [-310, 0, 0]          # rear door, hull frame
board_s = 1.0                 # per man
exit_s = 0.8
exit_over_sides_s = 0.5       # faster, more exposed (open compartments)
fire_from = false

[[passenger_slots]]           # 10 of these; boxes in the hull frame
box = { center = [-60, 70, 150], half = [25, 25, 45] }

# On a tank (riders):
[[rider_slots]]
box = { center = [-180, 60, 250], half = [30, 30, 50] }
```

### 6.7 Mounted MGs and shields (S, after §6.2)

- **What:** `pintle` mounts with their own arc and traverse rate, fired
  by a seat whose box is exposed (M3A1 ring .50, M20 ring .50, M8 .50,
  SdKfz 251 front MG behind its shield, StuG remote MG later). The MG
  shield is a §6.2 shield box.
- **Who fires:** a crew seat (`mg_gunner` or `commander`). For the 251,
  the commander mans the front MG; the historical practice of the squad's
  MG gunner manning it can come later.
- **Value:** gives transports and recon cars real firepower against
  infantry, with a real cost: the gunner is exposed.
- **Touches:** `gunnery.rs` (MG rules already exist for coax/hull mounts),
  content mount kinds.

### 6.8 Wheeled and half-track mobility (M)

- **Wheeled:** a bicycle model in `drive` (§5.5), higher speed, poor
  cross-country factor, no neutral steer, a turning radius. For the
  SdKfz 234, a second driving position: full speed in reverse
  (`reverse_cm_s` equals forward), authentic and useful for recon.
- **Trenches are walls for wheels.** A standard trench (1.6 m deep,
  about 1.6 m wide at the top) cannot be crossed by an armoured car. Any
  ground dug 1.0 m or more blocks wheeled vehicles in the block layer
  (S3 already has the layer and the AT-ditch rule). Half-tracks cross
  trenches like tanks, with a higher bog chance. This makes the trench
  pillar matter against recon and supply traffic with no new rule type.
- **Speeds in our maps:** maps have no roads yet. Use realistic
  cross-country speeds (about 30–40 km/h for armoured cars, not their 80
  km/h road speed); keep `road_speed_cm_s` reserved for when roads exist.
- **Wheels as components:** `wheels_front` / `wheels_rear` replace tracks;
  a blast under a wheel damages it (the S3 track rule). Large tyres were
  vulnerable to fragments: authentic.
- **Touches:** `vehicle.rs` (`drive`, pose from wheel samples), `path.rs`
  (a wheeled block layer), content mobility kinds.

```toml
[mobility]
type = "wheeled"                # tracked | halftrack | wheeled
speed_cm_s = 1000               # cross-country practical speed
reverse_cm_s = 1000             # 234: rear driving position
turn_radius_cm = 700
max_slope_pct = 45
ditch_block_dm = 10             # any trench blocks wheels
bog_pct = 12
```

### 6.9 Recon optics and unbuttoning (S)

- **Stationary observation:** a vehicle stopped for `settle_s` gets its
  `stationary_sight_pct` (the commander with binoculars). Recon vehicles
  have a high value; tanks a modest one. Uses the existing spotting
  code: one multiplier on sight.
- **Unbuttoned commander:** `SetHatch { units, open }`. Open: commander
  sight × `unbuttoned_sight_pct`, and his seat box moves to its
  `unbuttoned` position above the roof, outside the armour, where small
  arms and fragments reach it. Real tank commanders fought this way and
  died this way. AI rule: unbutton when no seen enemy within 150 m,
  button up when suppressed or under fire.
- **Small spot size:** armoured cars at about 120–150 %, not the tanks'
  200 %. Already supported.
- **Radio:** later, a recon vehicle can be the observer for mortar and
  howitzer fire beyond the guns' own sight (ties into the artillery area).
- **Value:** gives recon vehicles their real job without new abstract
  rules. **Touches:** `spotting.rs`, `vehicle.rs`, `orders.rs`, HUD.

```toml
sight_m = 220
spot_size_pct = 140
[optics]
settle_s = 4
stationary_sight_pct = 140
unbuttoned_sight_pct = 125

[crew.unbuttoned]
commander = { frame = "turret", center = [-60, 0, 120], half = [25, 25, 40] }
```

### 6.10 Towing (M; with the AT-gun slice)

- **What:** a hitch point on the tow vehicle, a tow point on the gun, a
  rigid-bar trailer model (the gun follows the hitch at a fixed distance,
  no jack-knife physics). `Hitch` / `Unhitch` commands, `hitch_time_s`
  (§5.12.2 already names it). A hitched gun cannot fire; a gun crew can
  push an unhitched gun slowly (about 1 m/s).
- **Who tows, historically:** US 57 mm M1 by M3 half-tracks (armored
  infantry) or 1½-ton trucks (infantry divisions); German PaK 40 by SdKfz
  11, RSO tractors or trucks (the SdKfz 251 sometimes); Soviet ZiS-3 by
  Studebakers, GAZ trucks or horses (no horses in v1).
- **Value:** high for AT guns and howitzers: under "spawn at base and
  walk", a towed gun without a tow is a 1 m/s crawl to the front. Towing
  is the logistics game for guns.
- **Dependencies:** the AT-gun slice (gun entities, shields, crews).
- **Touches:** `vehicle.rs` (trailer kinematics, push-out), `orders.rs`,
  `path.rs` (longer footprint), AI.

```toml
[tow]
hitch = [-330, 0, 60]          # hull frame
max_kg = 3500
# On the gun def: tow_point = [...], tow_kg = 1425, hitch_time_s = 20
```

### 6.11 APCR, HVAP and smoke shells (S)

- **APCR/HVAP:** add `apcr` to `AmmoMechanism` (it is in §2.4). Its pen
  table drops faster with range; its core is small (lower `core.damage`,
  no filler), so it penetrates more but kills less behind the plate:
  authentic. Ammo choice gains "use rare rounds only against a target the
  common round cannot beat" (the gunnery aim-choice code already computes
  pen chance).
- **Rarity is the balance (owner decision D3).** Owner: "add rare round
  to be available for recharge at other point. Maybe they could recharge
  slower and only to a certain cap, like one vehicle can only have 10 of
  these. Not more." So:
  - **Rare rounds** are APCR, HVAP and HEAT where a vehicle or gun
    carries it (`rare = true` on the ammo def).
  - **A cap per vehicle.** `rare_cap` on the vehicle def is the most
    rare rounds it can hold, all rare types together. The rules key
    `rare_cap_max` (10) is the owner's ceiling. It is also the default
    for a def that lists a rare round but sets no cap. A def that lists
    no rare round holds none, so a truck or tow never stores rare
    rounds. A gun and its own tow share the gun's cap (critic pass 3).
    A def above the ceiling fails to load. A vehicle starts with its
    rare `count`s, which sum to at most its cap.
  - **Refill anywhere, slowly.** Rare rounds refill at every source that
    rearms the vehicle: 1 per 10 s in base, 1 per 30 s at an HQ, dump or
    supply vehicle (`rare_rearm_s`), against 1 per 2 s and 1 per 4 s
    for standard rounds. They come from the same paid `shells` kg.
  - **Same price per kg** as other shells. A premium could not be charged
    forward, since forward stock is pooled kg that was paid at the
    normal price.

  Historical basis: US 76 mm HVAP arrived from about Sep 1944 in very
  small allocations (TD units first); 90 mm HVAP from early 1945; German
  tungsten APCR was scarce from 1943 on (later PzGr 40 rounds used
  substitute cores); Soviet APCR came a few rounds per tank. *(verify
  allocations)*

  | Vehicle | Rare round | `rare_cap` (start, verify) | Why |
  |---|---|---|---|
  | M4A3 (75) W, M4A3E2 | none | 0 | The 75 mm M3 had no HVAP |
  | M4A3 (76) W | M93 HVAP | 2 | Very scarce from about Sep 1944 |
  | M10, M18 | T4 / M93 HVAP | 3 | TD units got HVAP first |
  | M36 | T30E16 HVAP | 2, from 1945-01 | From early 1945, small numbers |
  | M8 | none | 0 | |
  | Panzer IV H/J, StuG III G, Jagdpanzer IV, 38(t) | PzGr 40 | 3 | Tungsten rationed from 1943 |
  | Panther G | PzGr 40/42 | 3 | As above |
  | Tiger I | PzGr 40 (8.8 cm) | 5 | A few per tank |
  | SdKfz 234/1 | 2 cm PzGr 40 | 10 (default) | Small rounds, one magazine |
  | T-34-85, SU-85 | BR-365P | 5 | A few per tank |
  | IS-2, SU-100 | none | 0 | No APCR for these guns in the war |
  | Sherman (105) or StuH 42, later | HEAT | 5 | If added; HEAT counts as rare |

  The cap matters most far from base: it is what a tank has left once
  it is away from every stock (§7.6).
- **Smoke shells:** US 75 mm M64 white phosphorus and M89 smoke, German
  7.5 cm Nebelgranate. A smoke shell bursts into the S1b smoke cloud.
  **This is the cheapest historical answer to the open trench-assault
  problem** (S3 case 7, owner checklist 13): tanks screening the assault
  with smoke is exactly what crews did, and the owner already listed
  "mortar smoke" as a lever. WP also burns men near the burst (physical
  particles, as a small frag burst).
- **Touches:** content `vehicle.rs` (enum, 4 ammo entries), `gunnery.rs`
  (choice), `combat.rs` (smoke burst), `SetAmmo` gains `Apcr` and `Smoke`.

```toml
rare_cap = 2                   # D3: on the vehicle; rules ceiling rare_cap_max = 10
ammo = [
  { id = "ww2:76mm_m62_apc", count = 35 },
  { id = "ww2:76mm_m93_hvap", count = 2 },   # rare; counts against rare_cap
  { id = "ww2:76mm_m42_he", count = 30 },
  { id = "ww2:76mm_m88_smoke", count = 4 },
]

# defs/ammo/76mm_m93_hvap.toml (excerpt)
mechanism = "apcr"
rare = true                    # D3: refills at rare_rearm_s at any source, up to rare_cap
supply_kg = 8                  # kind "shells": priced at shell_mp_per_kg (D1)
```

### 6.12 Fuel (recommend no)

No fuel resource, no consumption. Keep a `fuel_tank` component as a fire
source (petrol-engined Shermans and Panthers burned; diesel T-34s
somewhat less, which the data can say via `fire_start_pct`). Missions
that need "low fuel" can script it in Luau. Reasons in §2 point 5.

---

## 7. Ammo model and resupply (vehicle side of the HQ follow-up)

The owner asked for the HQ to be a supply and command structure that can
produce ammo crates or rearm units nearby. This section covers what that
means for vehicles and how vehicles carry supply. The HQ structure itself
and infantry ammo belong to the structures and infantry write-ups; the
interface here is meant to match them.

### 7.1 What runs out, per vehicle

| Store | Today | Proposed |
|---|---|---|
| Main gun rounds per type | counted (`ammo` list) | unchanged, up to 4 types (§6.11) |
| Rare rounds (APCR/HVAP/HEAT) | none | counted; at most `rare_cap` per vehicle (≤ 10); refilled at every source, slowly (D3) |
| Vehicle MG rounds | unlimited | counted per mount (`rounds` on the mount). Starting values from period stowage, e.g. Panzer IV about 3,000 MG rounds, M4A3 about 4,000–5,000 .30 plus a few hundred .50 *(verify)* |
| Passengers' small arms, grenades, rockets | infantry rules | infantry rules (infantry write-up); a transport may carry spare crates in its stock |
| Crew | seats; base refill 1 per 20 s | unchanged |

Every count lives on the vehicle def, so a patch can change it.

### 7.2 One unit of supply: kilograms

Use **weight** as the common supply unit: each round and item def gets
`supply_kg`, and every stock (a crate, a truck, the HQ) holds kg. It is
authentic, easy to explain, and makes the logistics physical:

| Item | About *(verify)* |
|---|---|
| Rifle/MG ammo, 250-round box or belt | 8–10 kg |
| Hand grenade | 0.6 kg |
| Panzerschreck / bazooka rocket | 3.3 / 1.5 kg |
| 81 mm mortar bomb | 3.5–7 kg |
| 75 mm complete round | 9–11 kg |
| 76 mm / 85 mm complete round | 11–15 kg |
| 88 mm complete round | 15–20 kg |
| 122 mm (projectile + charge) | about 33 kg |

Consequence: one man carrying a 40 kg crate can rearm a rifle squad, but
**rearming a tank with 30 rounds needs about 300 kg**: a supply vehicle,
an HQ, a dump or the base. That is why tanks went back to the trains to
rearm. At the start prices it also costs about 130–170 manpower (D1,
with the rack price factor of `artillery-on-map.md` §4.2). A full rack
refill never costs more than 75 % of the tank, so refilling an empty
tank always beats buying a new one (critic pass 3).

### 7.3 Where supply comes from

Revised for owner decisions D1 and D3 (2026-11-01). Rates are starting
values in `[supply]`:

| Source | Standard main-gun round | Rare round (APCR, HVAP, HEAT) | MG belts | Also | Stock and price |
|---|---|---|---|---|---|
| Base zone | 1 per 2 s (`rearm_s.base`, today's rate) | 1 per 10 s (`rare_rearm_s.base`), up to `rare_cap` | 250 rounds per 2 s | repair, new crew | unlimited; each round charged at the rack price as it goes into the rack (D1) |
| HQ radius (25 m) or dump radius (15 m) | 1 per 4 s (`rearm_s.forward`) | 1 per 30 s (`rare_rearm_s.forward`), up to `rare_cap` | 50 rounds per s | — | the structure's kg stock, paid at base and carried in; the rack price difference is credited back |
| Supply vehicle (truck, half-track, M20), within `vehicle_radius_m` (15 m; men at 8 m) | 1 per 4 s | 1 per 30 s, up to `rare_cap` | 50 rounds per s | drops crates | its own kg stock, paid when loaded at base or drawn from an HQ; credited the same way |
| Placed crate | none: too heavy for tank rounds | none | infantry only | small arms, grenades, rockets, mortar bombs | 20–40 kg (man-carried), paid at issue |

> **Owner decisions (2026-11-01; `artillery-on-map.md` §4.2 "No back
> doors"):** no refill is free, in base or anywhere (D1). Main-gun rounds
> cost their kg at `shell_mp_per_kg` (start 1.0) times the vehicle's rack
> price factor, which keeps a full refill at or under 75 % of the card
> (`rack_refill_max_card_pct`; a 75 mm tank round about 4–6). MG belts
> cost `small_arms_mp_per_kg` (0.2). The factor applies wherever the rack
> fills: charged in base, credited forward, so no price depends on
> position. A vehicle takes from one source at a time, the fastest in
> reach; rates never stack (critic pass 3). Any round a weapon with
> indirect fire uses is crate-only (`crate_only_if_indirect`). Every
> cargo load a truck, half-track or tow takes at base pays its kind's
> price, and shells share the base crate queue (`base_crate_issue_s`).
> Rack rounds stay in that weapon's own racks
> (`rack_rounds_transferable = false`), so they can't be unloaded into a
> dump; that keeps the unqueued base rearm and the rare-round cap whole.
> There is no distance surcharge (D2): a tank far forward pays the same
> per round, but only gets rounds that someone carried or drove there.

- **Supply vehicles can also drop crates** at a point (a `DropCrates
  { vehicle, point, count }` command), for infantry to use. That is the
  "delivered by a truck or half-track" path the owner asked for.
- **Cargo is physical.** A supply vehicle has a `cargo` component with a
  box; a penetrating hit or a fire can detonate it, like an ammo rack. A
  truck has no armour, so MG fire kills its crew and engine. This is the
  counter to forward supply, and it needs no special rule.
- **Who refills what:** the base remains the only source of new crew
  and repair. Rare rounds refill forward too, but at a third of the
  forward rate for standard rounds and only up to the vehicle's cap
  (D3). So a forward HQ shortens the logistics loop but does not remove
  it (§2 point 4).

```toml
# defs/vehicles/usa_cckw.toml (excerpt)
role = "truck"
[supply]
cap_kg_total = 2500             # kg per kind, like every SupplySource (infantry-structures.md §6.3)
radius_m = 8                    # men
vehicle_radius_m = 15           # vehicles
round_interval_s = 4            # = rules rearm_s.forward; rare rounds at rare_rearm_s.forward (30)
crates = { item = "ww2:usa_ammo_crate", kg_each = 40 }
# cargo loaded at base pays its kind's price per kg (D1); shells at 32 kg per base_crate_issue_s

[[components]]
type = "cargo"
hp = 60
box = { frame = "hull", center = [-150, 0, 120], half = [180, 110, 60] }
```

### 7.4 Fit with today's base-zone resupply

- Today's base rules (`base_rearm_s`, `base_repair_pct_s`, `base_crew_s`,
  `[items] resupply_s`) keep their timings and become the "Base zone" row
  above. What changes is the price: each round or belt is charged as it
  goes in (D1), and the refill pauses when the player cannot pay. Repair
  and crew stay free. Existing rearm tests keep their timings and gain a
  manpower check.
- The HQ and supply vehicles reuse the same per-tick rearm code with a
  different rate, radius and stock. One function, three sources: small
  (S) on top of the HQ structure and §6.2's `cargo` component.
- Repair: keep field repair (crew) and base repair. A recovery vehicle
  (Bergepanther, M32) is out of scope; the HQ could add slow repair later
  if the structures write-up wants it.

### 7.5 AI

The S3 AI already retreats vehicles and repairs when clear. Add:
- A vehicle below `ai.rearm_pct` (main-gun rounds, e.g. 25 %) goes to the
  nearest stock it can reach (supply vehicle, HQ, dump, base) when no
  seen enemy is within `threat_radius_m`.
- **It leaves when its standard rounds are full.** It waits for rare
  rounds (1 per 30 s forward) only while no enemy is seen and no own
  point is under attack. A tank does not sit at an HQ for five minutes
  topping up HVAP while the front needs it.
- **Price:** tank rearm counts toward `ai.shell_mp_max_pct` (25 % of
  income, shared with shells crates). Over budget, a tank still rearms
  to `ai.tank_rearm_floor_pct` (50 %), so armour never sits empty.
- **No stall (critic pass 3):** the AI keeps `ai.ammo_float_mp` (100)
  back from unit buys while any own unit waits to rearm. A tank leaves
  a source that has served it nothing for `ai.rearm_stall_s` (15 s),
  for lack of manpower or stock. It does not go back until it has the
  float or the source has stock.
- **Push limit:** an AI tank pushing more than `ai.push_without_supply_m`
  (300 m, start value) beyond its nearest stock keeps a reserve of
  `ai.tank_reserve_rounds` (10) standard rounds for defence (§7.6).
- With 2 or more vehicles alive, the AI buys one supply vehicle and keeps
  it `supply_follow_m` (about 150 m) behind its armour, out of sight of
  known enemies.
- The AI uses rare rounds only by the gunnery choice rule (§6.11).
- The AI fires smoke shells ahead of an escorted assault (§6.11), which
  is also the test of whether smoke fixes case 7.

```toml
# defs/rules/skirmish.toml (excerpt; the full [supply] block is in artillery-on-map.md §8)
[supply]
shell_mp_per_kg = 1.0                      # D1: tank main-gun rounds are shells
small_arms_mp_per_kg = 0.2                 # D1: vehicle MG belts
rearm_s = { base = 2, forward = 4 }        # standard round
rare_rearm_s = { base = 10, forward = 30 } # D3: rare round
rare_cap_max = 10                          # D3: ceiling; default only for a def that lists a rare round
rack_refill_max_card_pct = 75              # critic pass 3: full rack refill at most this share of the card

[ai]
rearm_pct = 25
tank_rearm_floor_pct = 50                  # over the shells budget, still rearm to this
rearm_stall_s = 15                         # critic pass 3: leave a source that served nothing this long
ammo_float_mp = 100                        # critic pass 3: kept back from unit buys while a unit waits to rearm
push_without_supply_m = 300
tank_reserve_rounds = 10
```

### 7.6 How far a tank can push (owner decision D3)

The owner's aim: "the ability of the tanks to push infinitely is limited
by the ammo storage inside them." The rules above give that without a
new mechanic:

- **The rack is the hard limit.** A tank carries its historical stowage
  (Panzer IV H 87, M4A3 (75) W 104, Jagdpanzer 38(t) 41, IS-2 28;
  *verify*). Away from every stock it gets nothing more. When the rack
  is empty, the main gun is silent.
- **Forward rearm needs a stock someone brought.** An HQ or dump holds
  at most 600 or 300 kg of shells, about 60 or 30 tank rounds, and only
  what men or trucks carried in. A truck holds 2.5–3 t, but it is
  soft-skinned and must drive back to base to reload. Pushing farther
  means longer walks and drives, more exposed carriers, and more losses
  (D2). That is the cost of distance.
- **Rare rounds go first and come back slowly.** A tank's best rounds
  against heavy armour are capped (`rare_cap`, at most 10) and refill at
  1 per 30 s forward. A Panther that spends its 3 PzGr 40/42 at the front
  waits 90 s at an HQ to get them back, if the HQ has the stock.
- **Every round costs manpower** (D1), wherever the tank rearms.
- **Repair and new crew stay base-only.** A damaged tank still has to go
  home.

So a tank can push as far as its rack and the supply behind it allow.
The player decides how far the trucks and dumps follow, and pays for it
in men, vehicles and time.

---

## 8. Recommended roster per faction

§2.3 sets a long-term target of 15–25 vehicles per faction. For the next
milestones, aim for about **10–12 per faction**, in waves. Each vehicle
row says which systems it needs.

### 8.1 Germany

| Order | Vehicle | Role | Needs | Why |
|---|---|---|---|---|
| (have) | Panzer IV H | tank | — | |
| 1 | StuG III Ausf. G | assault gun | 6.1, 6.2, 6.3 | Most common German AFV; casemate system |
| 2 | Jagdpanzer IV | tank destroyer | 6.3 (data only) | Already planned for S3b |
| 3 | SdKfz 251/1 Ausf. D | APC | 6.2, 6.4, 6.5, 6.6, 6.7, 6.8 | Panzergrenadiere; transport |
| 4 | SdKfz 234/1 | recon car | 6.4, 6.5, 6.8, 6.9 | Recon with screens; fast reverse. (Or the 222 as a cheaper early car) |
| 5 | Panther Ausf. G | heavy-class tank | 6.1 caps, 6.11 | Heavy slot; G over A for a 1944–45 skirmish (A by date filter) |
| 6 | Jagdpanzer 38(t) | light TD | 6.3 (data) | Cheap, low ambusher, only 41 rounds |
| 7 | SdKfz 251/9 | support | 6.6 off (data) | HE support for Panzergrenadiere |
| 8 | Opel Blitz or SdKfz 11 | truck / tractor | 6.8, 6.10, §7 | Supply and towing |
| 9 | Tiger I | heavy | 6.1 caps | Heavy slot; mostly 1944 by date |
| later | 251/7 (engineers), 251/8 (ambulance), 251/22, Panzer IV/70, 234/2 Puma (cap 1), Panzer IV J (missions), Tiger II | | | |

### 8.2 USA

| Order | Vehicle | Role | Needs | Why |
|---|---|---|---|---|
| (have) | M4A3 (75) | tank | — | |
| 1 | M10 | tank destroyer | 6.2, 6.5 | Open-top system; planned for S3b |
| 2 | M3A1 half-track | APC | 6.4–6.8 | Armored infantry; tows the 57 mm later |
| 3 | M8 Greyhound | recon car | 6.4, 6.5, 6.8, 6.9 | The US recon vehicle |
| 4 | M4A3 (76) W | tank | 6.11 | Better gun plus HVAP; the authentic answer to Panthers |
| 5 | M18 Hellcat | tank destroyer | 6.5 (data) | Speed as armour; flanker |
| 6 | M36 Jackson | tank destroyer | 6.5 (data), 6.11 | From Sep 1944; the heavy-killer |
| 7 | M20 | utility / supply | 6.7, §7 | Command car, ammo carrier, .50 |
| 8 | GMC CCKW 2½-ton | truck | 6.8, 6.10, §7 | Supply, towing |
| 9 | M4A3E2 Jumbo | assault tank | 6.1 caps | 254 built: cap 1, late unlock |
| later | M4A3E8 (Dec 1944), M5A1, M24 (Dec 1944), M4/M21 mortar carriers, M16 MGMC | | | |

### 8.3 USSR (when the faction is built)

Transport is different and should stay different:

| Order | Vehicle | Role | Needs | Why |
|---|---|---|---|---|
| 1 | T-34-85 | tank, **with rider slots** | 6.6 riders | Main tank; tank desant is the authentic Soviet APC |
| 2 | SU-85 or SU-100 | TD | 6.3 | SU-85 for 1944, SU-100 for 1945 by date |
| 3 | SU-76M | light SPG, open top | 6.5 | Infantry support; very numerous |
| 4 | BA-64B | recon car | 6.4, 6.5, 6.8 | The Soviet light armoured car |
| 5 | IS-2 | heavy breakthrough | 6.1 caps | 28 rounds of 122 mm: HE that really hurts trenches and bunkers |
| 6 | Studebaker US6 | truck | 6.8, 6.10, §7 | Supply, towing, carrying infantry |
| 7 | M3A1 Scout Car (lend-lease) | recon / command | 6.6, 6.7 | Common in recon units |
| later | T-34/76 (1943), M4A2 (76) W (lend-lease), ISU-152, M17 MGMC, lend-lease M3/M5 half-tracks in small numbers | | | |

Pushback: giving the USSR a half-track-centred infantry roster would be
wrong for 1944. Tank riders are cheaper to build (same embark code, no
new vehicle), more authentic, and play differently: fast, and deadly
for the riders under any fire.

### 8.4 Starting costs (to tune)

Scale: Panzer IV H 640, Sherman 560, income 240 per minute plus 60 per
held point. All TOML.

| Vehicle | Cost | Pop | Cooldown | Max alive | Unlock |
|---|---|---|---|---|---|
| SdKfz 234/1, M8, BA-64B | 260–300 | 4 | 60 s | 2 | 1:30 |
| SdKfz 251/1, M3A1 | 280–320 | 3 | 60 s | 2 | 2:00 |
| Trucks | 150–200 | 2 | 45 s | 2 | 0:00 |
| StuG III G | 520 | 10 | 90 s | 3 | 4:00 |
| Jagdpanzer 38(t) | 460 | 9 | 90 s | 2 | 4:00 |
| M10 / M18 | 500 / 520 | 10 | 90 s | 3 | 4:00 |
| M4A3 (76) W | 680 | 12 | 120 s | 3 | 6:00 |
| M36 | 760 | 12 | 180 s | 2 | 8:00 |
| Panther G | 1,100 | 16 | 240 s | 1 (heavy) | 10:00 |
| Tiger I | 1,250 | 16 | 300 s | 1 (heavy) | 12:00 |
| M4A3E2 | 900 | 14 | 240 s | 1 (heavy) | 10:00 |

---

## 9. Build waves

Each wave is a slice; the vehicles in it are mostly data once its
systems exist.

| Wave | Systems | New vehicles | Size |
|---|---|---|---|
| **S3b** (planned) | 6.1 call-in rework, 6.2 schema, 6.3 casemate, 6.5 open top, sticky satchels, bail-out | StuG III G, Jagdpanzer IV, M10 (+ Jagdpanzer 38(t), M18 as data) | L |
| **S3c** | 6.4 small arms vs armour, 6.6 transport and riders, 6.7 pintle MGs, 6.8 half-track + wheeled mobility, 6.9 recon optics | SdKfz 251/1, M3A1, SdKfz 234/1, M8, M20 | L |
| **S3d** | 6.11 APCR/HVAP/smoke, engine-driven traverse, heavy caps | Panther G, Tiger I, M4A3 (76) W, M36, M4A3E2 | M |
| **with HQ / AT guns** | §7 supply stocks, trucks, 6.10 towing | Opel Blitz / SdKfz 11, CCKW | M |
| **USSR** | (none new) | T-34-85 with riders, SU-85/100, SU-76M, BA-64B, IS-2, Studebaker | M (content) |

Order rationale: S3b fixes the schema once, so every later wave is
cheaper. Small arms vs armour ships in the same wave as the first light
vehicle (§2 point 1). Heavies come after the counters (TDs, HVAP) exist.

**Content cost per vehicle** dominates after the systems: armour boxes,
crew and slot boxes, components, sources, and a model within §12.1's
budget (45k triangles for tanks, SPGs and half-tracks; style-neutral
blockouts until the M2 style gate). Expect each vehicle to be a day or
two of data and testing, plus art.

---

## 10. Tests to add (`armour-test` and new cases)

- Casemate duel: StuG frontal vs Sherman at 300 m; Sherman flanking a
  StuG from 45°. Report time to first hit and who wins.
- Open top: frag thrown at a stopped 251 with 10 men (expected:
  most passengers lost); at a moving one (mostly misses); at a screened
  234/1 turret (no damage inside); 81 mm near-miss on an M10 (crew
  wounds).
- Small arms: MG 42 and .50 against a half-track side at 50 / 150 /
  300 m; rifle against an M8 front (expect no pen).
- Transport: a loaded 251 hit by one Bazooka rocket (passenger losses);
  disembark under MG fire, rear door vs over the sides.
- Wheeled vs trench line: a 234 must find a gap or stop.
- Heavies: Panther G frontal vs 2 Shermans flanking; M36 vs Panther at
  350 m; HVAP use count per match.
- Smoke: S3 case 7 again with tank smoke shells (does it meet the owner's
  target?).
- Supply: a tank at 10 % ammo rearms from a truck at 15 m at 1 round
  per 4 s, and the truck's stock falls by the same kg; cargo detonation
  when the truck is penetrated.
- Price (D1): a tank rearming in base pays each round's kg ×
  `shell_mp_per_kg`; with no manpower it pauses; MG belts pay
  `small_arms_mp_per_kg`. Rearming from an HQ, dump or truck charges
  nothing more.
- Rare rounds (D3): a tank with `rare_cap = 2` never holds a third rare
  round from any source; rare rounds refill at 1 per 10 s in base and
  1 per 30 s forward; a vehicle def with a `rare_cap` above
  `rare_cap_max` fails to load; a truck or tow def with no rare round
  holds none; a tank cannot pass rack rounds to
  another vehicle, a crate or a dump.
- Critic pass 3: no vehicle's full rack refill costs more than 75 % of
  its card; a tank rearming from an HQ pays the same per round as in
  base, after the credit; a tank between an HQ and two trucks rearms no
  faster than at one; an AI tank in base with no manpower leaves within
  `ai.rearm_stall_s`.
- Push limit (D3): a tank driven 600 m past its last stock with no
  truck behind it fires its rack dry and then cannot fire its main gun;
  report how often AI tanks run dry and where.
- Determinism: every new state (riding, cooldowns, stocks, hatches)
  saved, loaded and hashed; loopback and soak with half-tracks embarking.

---

## 11. Open questions for the owner

1. **Spotting range for vehicles** (S3 question 2) decides how strong the
   Panther and Tiger are. Answer it before balancing heavies.
2. **Heavies in skirmish:** one shared "heavy" slot (Panther or Tiger or
   Jumbo), or one per type? Recommended: one shared slot.
3. **Transport in skirmish:** allow it from 2:00, cap 2? Or test without a
   cap first? Recommended: cap 2, measure the time saved per squad.
4. **Passengers firing from open compartments:** off in v1 (recommended)
   or on?
5. **Supply unit:** kilograms (recommended, authentic and simple to
   explain) or abstract "supply points"?
6. **Tank smoke shells as the next trench-assault lever** (S3 checklist
   13): recommended over mortar smoke because it adds a historical option
   to units the player already has.
7. **Fuel:** confirm no fuel resource (recommended).
