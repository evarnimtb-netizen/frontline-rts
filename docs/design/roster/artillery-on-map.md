# Artillery on the map: crewed, buyable units

Frontline RTS, `ww2` mod. Written 2026-11-01 for the owner.
Status: proposal. Nothing here is built. The repo was only read.

**The request** (owner, 2026-11-01, verbatim): "I do think that having
howitzers or some artillery is better as a unit. Could you think about
that, please?"

The roadmap's default until now (Q13 A) put divisional howitzers
off-map. This file answers the request. It replaces the off-map parts of
`guns-artillery.md` (§0 point 3, P2, §3.9, §7.4, slice G5, question 1).
The gun entity, crew slots, towing and AT-gun parts of that file still
stand, and this file builds on them.

Read with: `roster-expansion.md` (S5, S6, S8, S9, S11), `guns-artillery.md`
§3–4, `SYSTEM_DESIGN.md` §2.2 and §5.7, `docs/slices/S3-armour.md`,
`docs/slices/S3b-balance.md` (fragment rays, parapet erosion, the duck
state).

**About the numbers.** Every historical figure here is from memory. Treat
all of them as *(verify)*, even where the mark is not repeated. Each def
needs a `sources = [...]` entry before it ships. Game values (costs,
timers, crate prices) are starting values for tuning, not history.

**Owner decisions of 2026-11-01** (Q4, Q22, Q24, Q25; recorded in
`roster-expansion.md`, "Owner decisions"). This file is revised to them:

- **No shell ration.** Each gun and mortar has a ready inventory of
  shells. It refills from crates placed or carried near it, or by
  standing near a refill structure (HQ, ammo dump). Those structures fill
  only by crates, later by trucks. The base zone does not refill shells.
  Shell crates cost manpower when taken at base. Starting loads come with
  the card's price (§4.2, §4.10).
- **Q22:** friendly fire from mortars, guns and launchers is on (§6.3).
- **Q24:** the player gets a hint only, never the enemy's marker (§4.3).
- **Q25:** field howitzers on all maps; cap 1 and 8:00 unlock stay as
  TOML defaults to measure (§4.5, §4.7).

**Owner decisions D1–D3 (2026-11-01, later).** They supersede earlier
text where they differ:

- **D1. No free ammo anywhere, the base included.** Every refill costs
  manpower, by supply kind: small arms cheap, explosives moderate, shells
  (mortar, gun, tank main gun) dear (§4.2).
- **D2. Distance costs delivery, not a surcharge.** Crates and loads
  must be carried or driven forward; that time, exposure and those losses
  are the cost of pushing. The price does not depend on where the round
  is used (§4.11).
- **D3. Tanks rearm at base, near an HQ or dump, or from a supply
  truck,** and every round costs manpower. Rare rounds (APCR, HVAP, HEAT)
  refill forward too, but slower, and each vehicle or gun holds only a
  small capped number. The rack is the limit on how far a tank can push
  (`vehicles.md` §7.6).

---

## 0. Summary

**Verdict: yes, artillery can be on-map units.** It works if range is
not the brake. Authentic ranges exceed every map we plan, so something
else must hold artillery back. History already gives the answer: guns
were limited by **what an observer could see, how many shells came
forward, and the enemy's counter-battery fire**, not by how far the tube
could throw.

**What we recommend**

1. **Infantry guns and field howitzers become on-map cards.** One card is
   one gun and its crew (a section), not a battery. Germany: leIG 18,
   leFH 18 (later sIG 33). USA: 105 mm M3, 105 mm M2A1 (75 mm pack
   howitzer as data). USSR later: 76 mm M1943, 122 mm M-30.
2. **Keep authentic ranges.** Compression either does nothing (a 105 mm
   cut to 2 km still covers a 1 km map) or makes a howitzer shorter-ranged
   than a mortar. Neither is worth an unhistorical stat.
3. **Eight brakes do the work**, all physical or historical:
   - observation: indirect fire needs a linked observer's line of sight,
     or map fire on known points with a large aim error (§4.1);
   - **finite shells that cost manpower**: each gun has a ready
     inventory; it refills only from crates or a refill structure, and
     shell crates cost manpower when taken at base (owner decision Q4);
   - **counter-battery**: firing reveals the gun by sound and flash; the
     enemy gets a marker that sharpens with every round, and can shell it;
   - set-up, pack and tow times, so moving away from counter-fire costs
     minutes;
   - minimum range and masks from the real charge tables and elevation
     limits;
   - crews are soldiers in the open; gun pits help but do not save them
     from a near-direct hit;
   - cost, caps (two guns per player) and cooldowns;
   - flight time plus fire-direction delay, so moving men escape and
     artillery hits positions.
4. **Drop off-map batteries from skirmish.** They cannot be hit back, so
   they break the physical rule in spirit. Missions may script edge
   shells later.
5. **Split S11** into S11a (rules, observers, infantry guns, counter-battery,
   ready inventories) and S11b (field howitzers on the map, light mortars,
   rifle grenades). S11a ships only if S5 and S6 met their HE-share
   targets.

**What cannot work, said plainly**

| Idea | Why not | Best version that does |
|---|---|---|
| Range as the limiter | A leIG 18 (≈3.5 km) out-ranges a 2 × 2 km map; a 105 mm out-ranges a 4 × 4 km map. | Observation, priced shell crates, counter-battery (§4). |
| A historical battery (4–6 tubes, hundreds of rounds) | On a 1 km map it would erase every position. Pop and performance also suffer. | One gun per card, two artillery cards per player, every shell paid for. |
| Field howitzers dropping shells steeply into trenches on 1 km maps | The leFH 18 (max +42°) and M3 (max +30°) cannot fire high-angle at all, and at 0.5–1 km they fire almost flat. History had them 3–8 km back, where shells fell more steeply. | Accept it. On small maps they are flat-fire and direct-fire guns, good against structures and vehicles. On Regiment maps they behave closer to history. The high-angle short-range job belongs to mortars and the leIG 18. |
| Counter-battery from pure physics | Locating a gun by sound is information, not contact. | A listener rule (§4.3). It gives knowledge only. Damage still comes only from real shells. |
| No abstract rule at all | Base resupply is free today, so a gun in base would fire forever. | The base no longer refills shells. Shells leave the base only in crates that cost manpower (owner decision Q4, 2026-11-01). No ammunition is free any more, the base included (D1). The price stands for the historical shell allocation. |
| A price that rises with distance from base | It makes the base the cheapest place to fire from, and the base is already the best place for a gun (§4.9). | Distance costs delivery: men and trucks must carry the shells forward (D2, §4.11). |
| Off-map batteries kept "for flavour" | Nothing can hit them back, and they would compete with the on-map cards. | Drop them in skirmish (§7). |
| Rolling barrages without friendly fire | Today no own round hurts own men, so a player can shell a trench while his men walk into it. | Owner decision Q22 (2026-11-01): fragments and blast from indirect fire hurt own men. |

---

## 1. What changes

| Topic | Roadmap until now | This design |
|---|---|---|
| Divisional howitzers (leFH 18, M2A1, M-30) | Off-map battery call, 48 rounds per match | On-map single-gun sections, S11b |
| Infantry guns (leIG 18, M3) | On-map, blocked by Q4 | On-map, S11a, with ready inventories fed by priced crates |
| Range | Undecided (Q4: compress or ration) | Authentic; observers, priced shells and counter-battery are the brakes |
| Shell supply | Free base resupply (Q4 open) | Ready inventory per gun and mortar; refills from crates, HQ or dump only; shells crates cost manpower at base (Q4, decided); every other kind of ammo is priced too (D1) |
| Counter-battery | Mortars and infantry guns only | Every on-map gun; sound and flash |
| Off-map support | `SupportCall`, FO teams, edge shells | Dropped from skirmish; mission scripting only |
| HQ radio | Relay for off-map calls | One of the radio links for on-map guns |
| Nebelwerfer | "later, rocket fire mission" (off-map) | On-map launcher, late and capped, owner question Q23 |

---

## 2. Guns per faction

Columns: mass in firing position; crew as we field it (historical crew in
brackets); maximum range; rate as sustained / short burst in rounds per
minute; elevation band (it decides high-angle fire, §4.5); shells; tow.
All *(verify)*.

### 2.1 Germany

| Card | Gun | Mass | Crew | Range | Rate | Elevation | Shells | Tow | Slice |
|---|---|---|---|---|---|---|---|---|---|
| Infanteriegeschützgruppe | 7.5 cm leIG 18 | ≈400 kg | 5 (hist. ≈6 plus drivers) | ≈3,550 m | 8 / 12 | −10° to +73° | Igr 18 HE (6 kg, ≈0.7 kg filler); Igr 38 HL HEAT; (Igr Deut smoke, not used, Q5) | pushed, or Sd.Kfz. 11 variant (hist. horses; motorised units used light halftracks or trucks) | S11a |
| leichte Feldhaubitze (Geschütz) | 10.5 cm leFH 18 / 18M | ≈1,985 kg (18M ≈2,040) | 6 (hist. ≈9) | ≈10,700 m (18M ≈12,300) | 4 / 6–8 | −5° to +42° | FH Gr 38 HE (14.8 kg, ≈2 kg filler); Gr 39 rot HL HEAT; Pzgr rot AP | Sd.Kfz. 11 | S11b |
| schwere Infanteriegeschützgruppe | 15 cm sIG 33 | ≈1,750 kg | 6 (hist. ≈8) | ≈4,700 m | 2 / 3 | 0° to +73° | 38 kg HE (≈8 kg filler); Stielgranate 42 (90 kg, ≈1 km) | Sd.Kfz. 11 or truck | later |
| Werfergruppe (Q23) | 15 cm Nebelwerfer 41 | ≈540 kg empty | 5 | ≈6,900 m | 6 rockets in ≈10 s, then ≈90 s to reload by hand | up to +45° | 15 cm Wgr 41 Spr (≈34 kg, ≈2 kg filler) | Sd.Kfz. 11 | later, lobby option (Q23) |

Not on the map: the 15 cm sFH 18 (≈5.5 t, ≈13 km, needs an 8 t tractor).
It is a corps and divisional general-support gun; one shell is 43.5 kg.
Missions only.

### 2.2 USA

| Card | Gun | Mass | Crew | Range | Rate | Elevation | Shells | Tow | Slice |
|---|---|---|---|---|---|---|---|---|---|
| Cannon Company Howitzer Section | 105 mm Howitzer M3 | ≈1,130 kg | 6 (hist. ≈8) | ≈7,600 m | 2–3 / 4 | −9° to +30° | M1 HE (15 kg, ≈2.2 kg TNT); M67 HEAT; (M84 smoke, Q5) | GMC CCKW (hist. 1½-ton or 2½-ton truck) | S11a |
| Field Artillery Section | 105 mm Howitzer M2A1 | ≈2,260 kg | 8 (hist. ≈8 plus drivers) | ≈11,300 m | 2–3 / 4 for the first minutes | −5° to +65° | M1 HE; M67 HEAT; (smoke, WP: Q5) | GMC CCKW (hist. 2½-ton truck or M5 tractor) | S11b |
| 75mm Pack Howitzer Section (airborne) | 75 mm M1A1 on M8 carriage | ≈653 kg | 5 (hist. ≈7) | ≈8,800 m | 3 / 6 | −5° to +45° | M48 HE (6.2 kg, ≈0.7 kg TNT); M66 HEAT | pushed, or jeep later | S11b, data only, airborne sub-roster or missions |

Not on the map: the 155 mm M1 howitzer (≈5.8 t, ≈14.6 km, 43 kg shell).
Missions only.

**US rockets.** No fitting towed US rocket launcher in 1944. The T27
"Xylophone" was rare. The T34 "Calliope" sat on a Sherman. Both are out
of scope; the Calliope could come later as a vehicle.

### 2.3 USSR (with S15)

| Card | Gun | Mass | Range | Elevation | Notes |
|---|---|---|---|---|---|
| 76-mm regimental gun | 76 mm M1943 (OB-25) | ≈600 kg | ≈4,200 m | up to ≈+25° | Pushable; HE and BP-350M HEAT. Flat-fire infantry gun. |
| ZiS-3 crew | 76 mm ZiS-3 | ≈1,116 kg | ≈13,000 m | up to ≈+37° | Already in the AT list (dual role). Its indirect fire uses these rules. Its OF-350 HE is crate-only; its AP rearms in base like a tank's, at the shells price (§4.2). It counts toward the artillery cap while it fires indirect (§4.7). |
| Howitzer section | 122 mm M-30 | ≈2,450 kg | ≈11,800 m | up to ≈+63.5° | 21.8 kg OF-462 HE. Tow: Studebaker US6. The Soviet field howitzer card. |
| 120-mm mortar crew | 120 mm PM-38 | ≈280 kg firing (≈480 kg on its wheeled carriage) | ≈5,700 m | mortar | Heavy mortar (already planned for S15). |
| BM-13 (Q23) | 16 × 132 mm M-13 rockets on a truck | — | ≈8,500 m | — | Same verdict as the Nebelwerfer: late, capped, lobby option. |

### 2.4 Rocket launchers: does the Nebelwerfer fit?

**Mechanically, yes.** It is an indirect weapon with a salvo. The shell
code, observer rule and crate price all apply. It needs one small new thing:
a short powered phase in the projectile integrator (thrust for the burn
time, then ballistic). That is a small, deterministic addition.

**Its historical weakness is physical and fits our rules.** The rocket
trails and dust gave the launcher away at once. Crews fired and moved.
In our rules every launch is a flash event that anyone with line of
sight sees at long range (§4.3), on the first salvo.

**The balance problem is real, but smaller than it looks.** One salvo
is six 34 kg rockets: about 204 kg of *ammunition* in ten seconds. Most
of that weight is motor, propellant and casing. The explosive is only
about 2–2.5 kg per rocket *(verify)*, so 12–15 kg per salvo: roughly
six 105 mm shells. The real danger is different. All six land within
about ten seconds, over a wide area, before men can duck. That is
surprise, not tonnage.

**The crate price makes it expensive.** Shell crates are priced by
weight, as real logistics was. At the starting price (1 manpower per kg,
§4.2) one reload of six rockets costs about 204 manpower, two-thirds of a
rifle squad. For the same price he could buy about 11 rounds of 105 mm.
So the Nebelwerfer is a poor deal in explosive per manpower. It buys one
sudden blow, not more HE.

**Recommendation:** not in S11. Offer it after S11b as a lobby
option (Q23 A), capped at one, with a 12:00 unlock, after the shell
price and the HE share have been measured with tube artillery.

---

## 3. The core problem: range

### 3.1 Maps against guns

`SYSTEM_DESIGN.md` §2.2: Battalion maps up to 3 × 3 km (8 GB PCs up to
2 × 2 km); Regiment maps up to 4 × 4 km. Today's only map, Kessel Ridge,
is 1,024 m square with bases in opposite corners.

| Map | Base to base | Longest line |
|---|---|---|
| Kessel Ridge (today) | ≈1.18 km | ≈1.45 km |
| Battalion, 2 × 2 km (est., corner bases) | ≈2.3 km | ≈2.8 km |
| Battalion, 3 × 3 km (est.) | ≈3.5 km | ≈4.2 km |
| Regiment, 4 × 4 km (est.) | ≈4.7 km | ≈5.7 km |

| Weapon | Max range (verify) | Covers from its own base |
|---|---|---|
| kz. GrW 42 | ≈1.1 km | most of Kessel Ridge |
| 60 mm M2 | ≈1.8 km | all of Kessel Ridge |
| 8 cm s.GrW 34 / 81 mm M1 | ≈2.4–2.9 km | a 2 × 2 km map |
| leIG 18 | ≈3.55 km | a 2 × 2 km map; most of 3 × 3 km |
| sIG 33 | ≈4.7 km | every Battalion map |
| Nebelwerfer 41 | ≈6.9 km | every map |
| 105 mm M3 | ≈7.6 km | every map |
| leFH 18, M2A1, M-30 | ≈10.7–11.8 km | every map, twice over |

### 3.2 Decision: authentic ranges stay

Reasons:

1. **Compression does not solve base fire.** To stop a gun in base from
   reaching the centre of Kessel Ridge, its range must fall below about
   600 m. That makes a 105 mm howitzer shorter-ranged than today's
   compressed mortar. Any milder cut still leaves the whole front in
   range. The brake must be somewhere else anyway.
2. **The brakes in §4 are historical.** Guns were held back by observers,
   ammunition and counter-battery fire. Modelling those is the
   "cost-balanced realism" rule: availability and doctrine first, stats
   never.
3. **Authentic range makes counter-battery fair.** Every gun can reach
   every other gun. Nobody gets a safe corner by having the longer tube.
4. **Error grows with range, but only a little on our maps.** The aim
   error (§4.1) is a share of the range, so a gun far back misses by
   more before it ranges in. On a 1 km map the difference between 600 m
   and 1,100 m is small. Do not count on it as a brake (critic note C1).

Mortars: the roadmap keeps today's 600 m mortar range through S5. From
S6, bombs come only in priced crates, like gun shells. After that,
mortars can go authentic too. Switch them in a separate, measured step,
not at the same time as the guns.

### 3.3 Placement is compressed; this has effects

On a 1 km map a gun stands 0.5–1.2 km from its target. History had
infantry guns 1–3 km back and field howitzers 3–8 km back. We do not
change any stat for this, but three things follow:

- **Flat fire.** Low-angle howitzers fire flatter than they did at their
  usual ranges. They hit the front of a parapet or overshoot a trench.
  They cannot reach a reverse slope near the gun (§4.5).
- **Counter-battery is faster.** Everything is close. Listeners are
  nearer, so markers are sharper, and flash is seen more often.
- **Guns are targets for infantry.** A gun 700 m from the front can be
  reached by a squad in a few minutes. That is good: it makes guarding
  your guns a real job.

---

## 4. The limiters

### 4.1 Observation

S5 builds the observer rule, ranging-in and map fire for mortars. S11
extends them to guns.

**Fire modes**

| Mode | Needs | Accuracy | Notes |
|---|---|---|---|
| **Direct** | The gun's own crew sees the target (gunner or section leader) | Gun spread, ranging-in per shot (S3 gunnery) | §5. Exposes the gun. |
| **Observed** | A soldier with `observe` has line of sight to the target point, and a **link** to the gun | Aim error starts at `first_round_error_pct` (3 % of range, at least 25 m) and shrinks by `ranging_step_pct` (50 %) per round the observer sees land, to zero. Precision spread always on top. | The normal way to use artillery. |
| **Registered** | Unobserved fire on a point this gun ranged in on (see Registration) | Aim error `registered_error_pct` (1 % of range, at least 10 m) | Defensive fire plans. Expires. |
| **Map fire** | The target is a point in the player's knowledge map (last-seen enemies, seen trench lines and structures, capture points, sound or flash markers) | Aim error `map_fire_error_pct` (6 % of range, at least 50 m), no ranging-in | Harassing fire. No blind fire into unknown ground. |
| **Counter-battery** | Map fire on a sound or flash marker (§4.3) | Map-fire aim error plus the marker's own error | A gun shelling a gun. |

**Two error terms, not one** (critic fix C1). Mortars in S5 use one
spread: a large game value (`spread_mrad` 50 for the 8 cm GrW 34 today,
about ±30 m at 600 m), multiplied by 250 %, 300 % and so on. That cannot
work for guns. An authentic howitzer spread is only a few mrad *(verify
from firing tables)*. Three times a few metres is still a few metres, so
map fire would be almost as good as observed fire, and the observer
brake would vanish. So guns use two terms:

- **Aim error**: where the whole group of shots lands, relative to the
  target. It comes from map errors, survey and weather. It is a share
  of the range with a floor in metres. The observer removes it by
  ranging. This is the brake.
- **Precision spread**: how shots scatter around their aim point. It is
  authentic, from the gun's firing table, in `range_pe_pct` and
  `deflection_pe_mrad`. Nobody removes it.

A gun that has ranged in is therefore very accurate at these short
ranges. That is historical, and it means the shell price and the trench
model (S3b) must carry the load against HE, not scatter. Mortars keep S5's
single spread until a separate, measured step moves them to the same
two terms. Today's 50 mrad mortar spread is a game value and is out of
line with this, like the mortar crater (§5.3).

**The link** (who can pass a fire order to a gun):

| Link | Rule |
|---|---|
| Voice | The observer stands within `observer_link_m` (150 m) of the gun's section leader. |
| Radio | The observer carries a radio (or stands within 150 m of an own radio), and the gun section's radio man is alive. Range: the whole map. |

Radios are carried by men, not boxes in the air: FO teams, every gun
section (its leader or *Funker*), and the HQ's radio box (S6). Kill the
radio man and the link breaks until another man picks the set up (the
launcher handover code, `handover_ms`). Hit the HQ radio box with
fragments and that relay is gone. All physical.

**Who observes**

| Observer | `observe` range for ground points | Notes |
|---|---|---|
| FO team: VB-Trupp (GER, 2 men), FO Party (USA, 3 men), razvedchiki (USSR) | 1,200 m with binoculars | Has a radio. Cheap (100–120 MP), fragile, walks from base. |
| Gun section leader | 400 m | For his own gun only, by voice. Lets a gun fire at what it can almost see over a crest. (Was 600 m. Critic fix C3: from a base on Kessel Ridge, 600 m reaches the centre point, so a base gun would need no observer at all.) |
| Squad leaders, mortar leaders, officers | 400 m | Voice link only, unless near a radio. |

Observing a ground point is a terrain line-of-sight ray plus the range
limit. It does not need the enemy men there to be spotted. That matches
history: observers adjusted on the bursts and on the ground.
**It reveals nothing.** The `observe` range lets a man aim and adjust
fire. It does not spot enemy men, trenches or structures beyond his
normal spotting range. Without this, an FO with 1,200 m binoculars
would become a free scout (critic fix C4).

**Registration.** Once a gun has ranged in on a point (aim error at
zero, observed), it keeps that registration for its
`registration_cell_m` (50 m) cell until it moves or
`registration_expiry_s` (600 s) pass. Later fire on that cell needs no
observer and starts at `registered_error_pct` (§4.1 table): good, but
not the zero error of observed fire. A defender can register the
approaches to his trench before the attack, as real defensive fire plans
did. Displacing to dodge counter-battery throws the registrations away:
a real cost of moving.

Why the expiry and the residual error (critic fix C2): without them a
gun that never moves (one in base) registers every capture point once,
early, and then fires accurately on them all match long with no
observer alive. The observer would be needed only for the first five
minutes. Real registrations also went stale as weather changed.
`registration_max_cells` (6) per gun keeps the list short.

### 4.2 Finite shells: ready inventories and priced crates

**Owner decision (Q4, 2026-11-01).** "Perhaps, the ration shouldn't
exist, but the artillery will have an inventory with shells at immediate
disposal. But they can be refilled by bringing ammo crates nearby or
placing the artillery near a structure that can refill them." Follow-up:
shell crates cost manpower when taken at base; small-arms crates are free
or cheap; every value is in TOML. **There is no per-player shell ration.**

**Owner decision D1 (later the same day).** "Let's not make shells and
other ammo free. Make small arms ammo cheap, but the artillery and tank
ammo shouldn't be too cheap." No ammunition is free any more, the base
included. Prices per kind are below ("What ammunition costs").

**The ready inventory.** Every gun and mortar holds its own shells:

- A gun's ready inventory is the rounds at the gun (ready rack and
  reserve, `guns-artillery.md` §4.1). A mortar's is the bombs its crew
  carries.
- Its size is the starting load at the gun (table below). It cannot hold
  more (`ready_cap`, per ammo id). HEAT is a rare round: at most 2 at
  the gun (§5.2, D3). The gun and its own tow share that one rare cap;
  the tow holds no rare rounds of its own (critic pass 3).
- The crew fires from it at once, with no supply step.

**Starting loads come with the card's price.** A gun or mortar arrives
with its full load. Nothing is drawn from a stock.

| Gun | At the gun (ready inventory) | In the tow, on arrival |
|---|---|---|
| leIG 18 | 12 Igr 18 HE, 2 Igr 38 HL; crew carry 4 more | 15 of 30 (tow variant) |
| 105 mm M3 | 10 M1 HE, 2 M67 HEAT | 12 of 24 |
| leFH 18 | 12 HE, 2 HEAT (Gr 39 rot HL), 1 Pzgr rot | 12 of 24 |
| 105 mm M2A1 | 12 M1 HE, 2 M67 HEAT | 12 of 24 |
| Nebelwerfer 41 | 6 loaded | 3 of 6 |

The tow arrives half full (`tow_starting_load_pct`, 50). **Price check:**
when content loads, a card's cost must be at least its starting load
priced at the crate price (`load_price_check`). A leFH 18 card carries
about 510 kg of shells, worth about 510 manpower at 1 per kg; the card
costs 480 plus the tow. Without the check, buying a new gun could be a
cheaper way to buy shells than buying crates.

The check **raises** a card that is too cheap to its load's price and
logs a warning; it does not refuse the content. `shell_mp_per_kg` is the
main lever and S6 will move it. If it doubles, every gun card would
otherwise fail the check, or, with the check off, become the cheapest
way to buy shells. Mortar cards follow the same rule for the bombs their
men carry.

Tank, tank destroyer and AT-gun cards are exempt. Their starting rounds
stay in the weapon's own rack and never feed another weapon or a stock
(rule 2 below), so a new tank is never a cheap way to buy shells. Without
the exemption the check would push a Panzer IV (87 rounds, about 850 kg)
above 850 manpower, far over its 640. The exemption does not cover
rounds in a crate-only class (rule 4): an SPG's or 105 mm tank's
indirect HE counts in the check like a howitzer's (critic pass 3).

**Rack refill ceiling (critic pass 3).** The exemption has a cost: at
1.0 per kg, refilling an empty Panzer IV (about 850) or Sherman (104
rounds, about 990) costs more than a new one (640, 560). An empty tank
would be worth less than its refill. So a full rack refill may cost at
most `rack_refill_max_card_pct` (75 %) of the card. Content load gives
each tank, TD and AT gun a rack price factor: the lower of
`rack_rearm_price_pct` and that ceiling. At the start values a Panzer IV
round costs about 5.6, a Sherman round about 4 *(weights verify)*. Rare
rounds use the same factor. Crate-only rounds always pay the full
shells price. The factor applies wherever the rack fills, so no price
depends on position (D2): in base the round is charged at the factor;
forward, the kg drawn into a rack was paid at full price, so the
difference is credited back to the player who paid for that stock.
Rack rounds never leave the rack (rule 2), so the credit cannot be
laundered.

**How a gun or mortar refills.** Only from shells someone brought:

| Source | How | Rate (start values) |
|---|---|---|
| A shells crate within `crate_feed_m` (5 m) of the gun or tube | Placed by bearers, or dropped by a truck (S8) | One round per `fetch_s` per crew man on the `ammo` slot (leIG 18: 4 s) |
| An HQ or ammo dump, with the gun or mortar inside its radius (25 m, 15 m) | The structure's shell stock | Guns: one round per `structure_refill_s` (4 s); mortars: one bomb per 2 s (S6) |
| The gun's own tow or an ammo vehicle within 3 m | Its stowage | `guns-artillery.md` §4.1 transfer rules |
| Any of the above, for a rare round (HEAT at a howitzer, APCR at an AT gun) | Same sources | One round per `rare_rearm_s.forward` (30 s), never above the gun's rare cap (D3) |

Every round a gun or mortar takes from these sources was paid for when
its kg left the base. Nothing here is charged again.

**The base zone does not refill shells.** A gun or mortar parked in base
gets nothing from the base itself. Its crew, or a bearer, must take a
crate at base and set it down by the gun.

**Mortars follow the same rule, from S6 on.** In S5 there are no crates
yet, so the base still refills mortar bombs directly. Each bomb costs its
kg at the shells price (D1); S5 measures that. From S6 the base refills
no bombs. A mortar team takes priced crates at
base, or draws from an HQ, a dump or a crate. One rule for every
indirect-fire weapon is easier to read and closes the "mortar in base"
loop that caused the HE problem.

**Which ammunition this covers** is a TOML list, `crate_only_classes`:
mortar bombs, gun and howitzer shells, artillery rockets and HE rifle
grenades. Small arms, hand grenades, AT rockets, and tank and AT-gun
rounds still refill in the base zone directly, into the weapon's own
racks only, and now at their kind's price (D1). Tanks and AT guns fire
direct; they are not the HE problem. A gun that can also fire indirect
(the ZiS-3) is the exception: its HE is crate-only (rule 4 below).

**What ammunition costs (owner decision D1).** There is one price per
supply kind, in manpower per kg, in `[supply]`:

| Kind | TOML key | Start | What it covers |
|---|---|---|---|
| Small arms | `small_arms_mp_per_kg` | 0.2 | rifle, SMG and MG rounds, vehicle MG belts |
| Explosives | `explosives_mp_per_kg` | 0.5 | hand grenades, satchels, AT rockets, Panzerfausts, mines, flame fuel |
| Shells | `shell_mp_per_kg` | 1.0 | mortar bombs, gun and howitzer shells, tank and AT-gun main-gun rounds, artillery rockets, HE rifle grenades |

The kind belongs to the ammo def. A content check refuses any round fired
by a vehicle main gun or a gun entity that is not of kind `shells`, so an
autocannon or tank round can never be filed as cheap small arms (critic
pass 3).

Why these values:

- **Small arms, 0.2.** A rifle squad that empties its pouches uses about
  20–30 kg: 4–6 manpower, under 2 % of its card. An MG team's full belts
  (about 40 kg) cost about 8. Nobody should hold fire to save manpower,
  but an MG that fires all match is not free.
- **Explosives, 0.5.** A grenade costs about 0.3, a Panzerschreck rocket
  about 1.7, a Panzerfaust 60 about 3, a satchel about 1.5 *(weights
  verify)*. A launcher team's six rockets cost about 10. Explosives kill
  more per kg than shells, but they are thrown or fired by a man at short
  range, and he is the main cost. The price is felt when a player spends
  satchels and rockets freely, not when he throws one grenade.
- **Shells, 1.0.** Unchanged, and still set from S6's measurement. It is
  the price where heavy use competes with buying units: a 24-round 105 mm
  preparation costs about 450, more than a rifle squad. Tank racks pay
  the rack price factor (above): a Panzer IV that fired 40 rounds pays
  about 225 to fill up again, a third of a new tank.

| Round | Complete round, about (verify) | Manpower each |
|---|---|---|
| Rifle round (7.92 mm) | 27 g | ≈0.005 |
| Hand grenade | 0.6 kg | ≈0.3 |
| Panzerschreck rocket | 3.3 kg | ≈1.7 |
| 8 cm / 81 mm mortar bomb | 3.5 kg | ≈3.5 |
| leIG 18 Igr 18 | 6 kg | ≈6 |
| 75 mm tank or Pak 40 round | 9–11 kg | ≈9–11 |
| 105 mm M1 or FH Gr 38 | 15–19 kg | ≈15–19 |
| Nebelwerfer rocket | 34 kg | ≈34 |

**Where it is paid: once, when the kg leaves the base.** The base is the
only unlimited stock, so it is the one place where kg enter play:

- a refill in the base zone (a man's pouches, a tank's rack, an AT gun)
  is charged round by round as it happens;
- a crate is charged when it is issued;
- a truck, half-track or tow load is charged as it is loaded;
- an HQ, dump, crate or supply vehicle passes on kg that were already
  paid; drawing from it costs nothing more.

So every refill costs manpower wherever it happens, and no kg is paid
twice. Captured stock was paid for by the enemy.

**One source at a time (critic pass 3).** A man, gun or vehicle is
served by one source per service tick: the fastest one in reach for
what it needs, ties to the lower source id. Rates never stack, so a
tank parked between an HQ, a dump and two trucks still gets one
standard round per 4 s and one rare round per 30 s.

**Short of manpower.** A base refill pauses when the player cannot pay
for the next round, and the HUD says "No manpower for ammo". Crates and
loads are refused, as before. Nothing goes into debt.

One price per kind, never per calibre. Kinds are pooled by kg
(`infantry-structures.md` §6.3), so a cheaper calibre inside a kind would
be a laundering route (rule 1 below). The one exception is the rack
price factor for rounds that never leave the rack (above): the ceiling
`rack_refill_max_card_pct` (75) and the fallback lever
`rack_rearm_price_pct` (100). If tank rearm still turns out too dear
while the HE share is right, the lever lowers it further. Both apply in
base and forward alike (critic pass 3).

Rare rounds (APCR, HVAP, HEAT) cost the same per kg as other shells.
A premium could not be charged forward: an HQ's stock is pooled kg that
was paid at the normal price. Their rarity comes from a per-weapon cap
and a slow refill rate instead (D3, `vehicles.md` §6.11).

**Refill structures.** The HQ and the ammo dump hold a shell stock. It
fills only by crates carried in by the player's men, and later by trucks
(owner decision Q2 C). It has no trickle. Every shell in it was paid for
at base. HQ shell cap 600 kg, dump shell cap 300 kg (`cap_kg` in the
supply defs; `infantry-structures.md` §6.4).

**Crates.** A shells crate is about 32 kg. One man carries it at 60 %
speed, or two men at 85 %. Live shells crates per player are capped at
`shell_crate_cap` (8), inside the general crate cap. Issuing one at base
takes `base_crate_issue_s` (10 s). A crate cooks off when HE breaks it
(`infantry-structures.md` §6.6).

**Weight decides the carrier.** A 105 mm complete round is about 18–20
kg (verify). A man carries about two. Heavy guns depend on their tow and
on trucks. That is historical and gives trucks a second job.

**No back doors (critic pass 2, revised for D1).** The price only works
if every kg that reaches a weapon passed the price at base. Critic pass
2 wrote four rules when tanks, AT guns and small arms still refilled free
in base. D1 removed that free refill, so each rule is re-checked here:

1. **Every kg loaded as cargo pays its kind's price. Kept, and now
   general.** Any kg loaded at base as cargo (a crate, a truck, a
   half-track, or a gun's or mortar's tow) pays its kind's price,
   whatever calibre it is meant for. Before D1 this stopped free "tank
   rounds" from unloading into a dump as howitzer shells. Now it is
   simply the price rule; the "whatever calibre" part still matters,
   because kinds are pooled by kg (`infantry-structures.md` §6.3).
2. **Rack rounds stay in the rack. Kept, for new reasons.** Rounds that
   go into a tank's or AT gun's own rack (or an AT gun's own tow) feed
   only that weapon: never a crate, a dump, an HQ, another vehicle or
   another weapon (`rack_rounds_transferable = false`). They are no
   longer free, but rack rearm in base does not wait in the crate queue
   (rule 3), and rare rounds have a per-weapon cap (D3). Moving rounds
   out of a rack would bypass both. The rule is also what makes
   the rack price factor, its forward credit and the tank exemption
   from the price check safe.
3. **Vehicle loads wait in the same queue as crates. Kept.** Loading
   shells into a vehicle at base takes one crate's weight (32 kg) per
   `base_crate_issue_s`, shared with crate issue. Without it a player
   could save 2,000 manpower and fill a truck at once, which skips the
   issue limit. This never depended on free refill.
4. **Anything that can fire indirect takes its HE by crate. Kept, for a
   new reason.** An ammo id fired by any weapon with an `[indirect]`
   table is crate-only for every weapon that fires it
   (`crate_only_if_indirect = true`, a content check). This covers the
   ZiS-3 (AT list, indirect to about 13 km) and any later SPG or 105 mm
   tank (M4 105, StuH 42) that shares a howitzer's round. Their AP
   rearms in base like a tank's, at the shells price. Before D1 the rule
   stopped free HE. Now that HE is paid either way, it keeps Q4's one
   rule for every indirect-fire weapon (fed only by crates and refill
   structures) and keeps base HE inside the crate queue.

**Dropped with D1:** the exemption that let small arms, explosives, tank
racks and AT guns refill free in base, and the zero prices for
small-arms and explosives crates. Each now pays its kind's price.

There is also no free source of ammunition at the front. The HQ has no
trickle and there is no automatic carrying party (Q2 C). Since D1 the
content check refuses a `trickle_kg_per_min` above 0 for **any** kind,
in any rules file: a trickle of rifle rounds would now be free ammo too.

**The HUD** shows, with a gun or mortar selected, "Rounds 14 / 15" and
what is in reach: "Crate 5 m: 6 rounds", "In HQ radius". On the call-in
bar every crate shows its price ("Shells crate: 32 MP", "Small-arms
crate: 5 MP"). The match report lists manpower spent on ammunition per
kind.

**Set the price from measurement, not from a guess.** 1.0 per kg is a
placeholder. S6 prices bomb crates first, with mortars only. Set
`shell_mp_per_kg` there so the HE share meets the S5 target. S11a then
checks that guns do not push it back up (test 11).

**Team lobbies.** The price is per player, so HE per km² grows with
lobby size on a fixed map, as it does with mortars today. The price can
be set per battle tier in `battle_tiers.toml`.

**Why this is weaker than the ration, and what replaces it.** The
ration capped HE tonnage per minute whatever a player earned. A price
does not: a player who spends his income on shells can fire more. §4.10
re-checks every exploit the ration closed and says what brakes each one
now.

**Guns in their own base get no free crew.** Today vehicles in base
regain crew (`base_crew_s`). Gun crews are soldiers; lost men are
replaced only by buying replacements (S12). Otherwise a gun in base would
be immortal.

### 4.3 Counter-battery: sound and flash

**Sound.** Every shot from an indirect-capable gun is an acoustic event.

- The enemy keeps an **evidence count** per firing spot. Each shot adds
  one. The count falls steadily, by 1 per `cb_decay_s` (300 s), shot or
  no shot.
  A shot counts for an existing spot if it is within `cb_reset_m`
  (75 m) of it **or within that spot's current marker error**,
  whichever is larger.
- When the count reaches `cb_rounds_to_mark` (3 for guns, 5 for
  mortars), after `cb_delay_s` (15 s), the enemy's knowledge map gets a
  **battery marker**.
- Why not a fixed window (critic fix C6): with "3 shots inside 120 s
  from within 75 m", a gun that fires one round a minute, or moves 76 m
  after every two rounds, is never found. A slow harassing gun would be
  invisible forever. With decay, slow fire is found more slowly, not
  never. Only a gun slower than one round per five minutes stays
  hidden, and that is too slow to matter. With the marker error as the
  reset radius, a short hop does not wipe the marker: only a move
  longer than the enemy's current error does.
- The marker sits at the true position plus an error of `cb_error_pct`
  (15 %) of the distance from the gun to the nearest enemy soldier (the
  listener), drawn from the counter RNG.
- Each further shot multiplies the error by `cb_error_step_pct` (80 %),
  down to `cb_error_floor_pct` (5 %).
- Example: a gun 800 m from the nearest enemy. First marker ±120 m.
  After five more rounds about ±40 m (the 5 % floor). A 105 mm burst kills men within roughly
  10–20 m, so counter-battery needs several rounds, and each round costs
  manpower.

**Flash.** Any enemy soldier with a clear line of sight to the muzzle
within `flash_m` (2,000 m in daylight; dust, smoke and flash) **spots the
gun** for `flash_reveal_ms` (6 s), as `fire_reveal_pct` does today. This
is exact and immediate. A gun behind a crest (reverse slope, firing high
angle or over the crest) is not seen. Rocket launches count as flash for
everyone with line of sight, on the first salvo.

How it runs (critic fix C8): firing does **not** cast a ray from every
enemy soldier. It makes the gun conspicuous: for `flash_reveal_ms` its
spotting signature reaches `flash_m`, and the normal perception
scheduler (`SYSTEM_DESIGN.md` §5.6: candidate grid, cached pairs, the
tier's ray budget) finds who sees it. A volley of 20 guns would
otherwise ask for thousands of rays in one tick. The cost is that a
flash may be seen up to one perception refresh (≤ 1 s, priority pairs
0.2 s) late. Same rule as `fire_reveal_pct` today.

**The enemy strikes back** with real shells. Counter-battery fire is map
fire on the marker. Shells land with real scatter; fragments must reach
the crew. Infantry can also walk to a marked gun. Nothing silences a gun
except contact.

**Displacement.** Pack, move more than 75 m **and** more than the
enemy's current marker error (the player does not see that error, so
he must judge it; Q24 decided), emplace. The marker on the
enemy side stays where it was, now pointing at empty ground. The gun
loses its registrations. With §4.4's times, a move costs 1–4 minutes.

**What the player sees** (owner decision Q24, 2026-11-01: hint only).
Their own count only: "N rounds from this spot: likely located", shown
once the count reaches `cb_hint_rounds` (the enemy's `cb_rounds_to_mark`
for that weapon, so 3 for guns). It is worked out from the player's own
shots, never from the enemy's knowledge. The enemy's marker and its
error are never shown.

**Honest notes.**
- Real sound ranging was done by specialist units with microphone bases,
  and took longer. We let any soldier be a listener, and we compress the
  time. It is information, not damage, so the physical-contact rule
  holds.
- Counter-battery duels could crowd out the fight. The crate price
  limits them: a round at a gun is paid for like a round at men. We report the share
  of shells fired at guns (§10).

### 4.4 Set-up, pack, limber and tow

Times are compressed realism (game values). Real field howitzers took
several minutes to bring into action (verify).

| Gun | `emplace_s` | `pack_s` | `hitch_s` | Push | Move 200 m and back into action |
|---|---|---|---|---|---|
| leIG 18 | 20 | 12 | 15 | yes, ≈1.1 m/s | ≈3.5 min pushed; ≈1.5 min towed |
| 105 mm M3 | 40 | 24 | 20 | barely (≈0.45 m/s) | ≈2 min towed |
| 75 mm pack howitzer | 30 | 18 | 15 | yes, ≈0.9 m/s | ≈4.5 min pushed |
| leFH 18 | 90 | 54 | 25 | no (re-lay only) | ≈4 min towed |
| 105 mm M2A1 | 90 | 54 | 25 | no | ≈4 min towed |
| sIG 33 | 60 | 36 | 25 | no | ≈3 min towed |
| Nebelwerfer 41 | 45 | 25 | 15 | short pushes only | ≈2 min towed |

Re-laying the trail beyond the traverse arc (`relay_trail_s`): leIG 18
10 s; 105 mm 20–30 s. Wheels cannot cross dug ground deeper than about
0.6 m; guns use the ramp of their pit or go round trenches (S9).

**Effect.** Heavy guns are slow to start and slow to leave. A field
howitzer that has been located must choose: keep firing and risk the
counter-battery, or spend about four minutes out of action. That choice is
the historical one.

### 4.5 Minimum range and masks

S5 builds the charge and elevation solver for mortars. S11 uses it for
guns. **Minimum range is not a number we pick. It falls out of the real
charge table and elevation limits.**

**High angle** (above 45°) is how a shell drops steeply behind a crest or
into a trench. Its shortest range is the lowest charge at the steepest
elevation the gun allows. Drag-free estimates (R = v² · sin 2θ / g):

| Gun | Max elevation | Lowest charge (verify) | Shortest high-angle range |
|---|---|---|---|
| 8 cm s.GrW 34 | +85° | ≈70 m/s | ≈90 m |
| leIG 18 | +73° | ≈92 m/s | ≈480 m |
| sIG 33 | +73° | ≈125 m/s | ≈890 m |
| 105 mm M2A1 | +65° | ≈205 m/s | ≈3,300 m |
| leFH 18 | +42° | — | **no high angle** |
| 105 mm M3 | +30° | — | **no high angle** |
| 75 mm pack howitzer | +45° | — | no high angle (45° is the limit) |
| 76 mm M1943 | ≈+25° | — | no high angle |

**Low angle** (below 45°) at short range is almost flat. Example: a leFH
18 on charge 1 (≈200 m/s, verify) at 800 m fires at about 6°, with an
apex about 20 m up and a flight of about 4 s (drag-free).

**The mask check.** Before a mission starts, the solver marches the
chosen arc in about 20 segments against the heightfield (the max-mip
pretest makes this cheap). If no charge and elevation clears the ground
between gun and target, the order is refused with "masked". This runs
once per fire order and per gun move, not per shell.

**What this means in play**
- On a 1 km map the leIG 18 is the only gun that can drop shells behind a
  nearby crest. It is the true close-support howitzer, as it was.
- The leFH 18, M3 and pack howitzer are flat-fire guns at these ranges.
  They hit what they can nearly see: the front of a position, buildings,
  bunkers, vehicles, open ground. They are weak against a trench seen
  edge-on.
- The M2A1 can only fire high angle beyond about 3.3 km, so on Battalion
  maps it also fires low angle.
- Mortars keep the short-range plunging job.

This is a physical differentiation from authentic data, and it keeps the
howitzers from simply being better mortars.

**Field howitzers on small maps (owner decision Q25, 2026-11-01).** They
are allowed on all maps. The owner is unsure about the limit and
suggested a small range tweak. Said plainly: **a range cut does not fix
the flat fire.** Flat fire comes from the short distance to the target,
not from the gun's maximum range. On a 1 km map every target is well
inside even a cut range, so the shell flies the same flat path. Only a
slower charge would make it fall steeper, and that would be an invented
stat. So:

- The cap of 1 per player and the 8:00 unlock stay as TOML defaults
  (`max_alive`, `unlock_s`, §4.7). We measure them in S11b.
- **No per-map max range.** Its only real use would be to stop a gun in
  its base from reaching the enemy base. On Kessel Ridge that needs a cap
  under about 1.2 km, and a gun 300 m forward reaches the enemy base
  again. It moves the gun; it does not stop it. If spawn shelling becomes
  a problem, it gets its own question.
- If field howitzers dominate small maps, the levers are cost, cap,
  unlock time and cooldown, then a map flag that removes the card
  (`allow_field_howitzers`, true). Not range.

**Danger close** (own men within 50 m of the target): the AI never fires;
a human gets a HUD warning. Own shells do hurt own men (owner decision
Q22, §6.3), so the warning matters.

### 4.6 Crew vulnerability and gun pits

- **Crews are soldiers in service slots** (S9, `guns-artillery.md` §3.2).
  Bullets and fragments reach them. A pinned gunner does not lay; a
  pinned loader does not load. MG fire silences a gun, as it did.
- **Shields** (leIG 18, leFH 18, M2A1, verify each; the M3 and pack
  howitzer had little or none) stop fragments and bullets from the front
  only.
- **Size.** Howitzers are big: `spot_size_pct` leIG 18 80 %, 105 mm
  130 %, sIG 33 140 %. They are seen far.
- **Gun pits** (S6 pit kind): `howitzer_pit` about 6 × 6 m, 0.8 m deep,
  with a ramp; the leIG 18 fits an AT pit. In a pit, only bursts in or
  very near the pit reach the crew; fragments from further off hit the
  parapet. Engineers dig them; crews dig slowly with `dig_basic`.
- **Ammunition at the gun can cook off.** The ready rounds are a box; HE
  or AP that breaks it may set it off (`guns-artillery.md` §3.1).
- **Capture.** A gun whose crew dies or flees turns neutral. The enemy
  can serve it with the rounds left at the gun or in captured crates of
  its own class (S9). A captured howitzer shelling its owner is possible
  and historical.

### 4.7 Cost, caps and cooldowns

Starting values on today's scale (rifle squad 300–340, Pak 40 380 plus
tow, income 240 per minute plus 60 per held point):

| Card | Cost | Pop | Cooldown | Max alive | Unlock | Slice |
|---|---|---|---|---|---|---|
| VB-Trupp (GER) | 100 | 2 | 60 s | 2 | 0:00 | S11a |
| Forward Observer Party (USA) | 120 | 3 | 60 s | 2 | 0:00 | S11a |
| Infanteriegeschützgruppe (leIG 18), pushed | 320 | 7 | 90 s | 2 | 3:00 | S11a |
| … with Sd.Kfz. 11 (variant) | 320 + tow | 7 + tow | 90 s | 2 | 3:00 | S11a |
| Cannon Company Howitzer Section (M3) | 380 + tow | 8 + tow | 120 s | 2 | 4:00 | S11a |
| leFH 18 section | 480 + tow | 8 + tow | 180 s | 1 | 8:00 | S11b |
| 105 mm M2A1 section | 500 + tow | 10 + tow | 180 s | 1 | 8:00 | S11b |
| sIG 33 section | 520 + tow | 9 + tow | 240 s | 1 | 10:00 | later |
| Nebelwerfer 41 (Q23) | 420 + tow | 7 + tow | 240 s | 1 | 12:00 | later |

- Pop: the crew plus 2 for the gun.
- Each card's cost includes its starting load of shells (§4.2). The
  content check raises a card worth less than its load at crate price
  to that price, with a warning.
- leFH 18 and M2A1: cap 1 and 8:00 unlock are TOML defaults to measure,
  on all maps (owner decision Q25).
- **`limit_group = "artillery"`: 2 per player** on Battalion, 3 on
  Regiment (in `battle_tiers.toml`). Infantry guns, howitzers and
  launchers count; mortars do not (they have their own caps).
- **Who counts toward the cap** (critic fix C7). A player may serve at
  most `limits.artillery` guns at once, his own or captured. His own gun
  still counts while it sits neutral (crew dead or fled), until it is
  destroyed or the enemy serves it. Otherwise he could let a gun go
  neutral, buy a new one, and re-crew the old one with riflemen: three
  or four guns under a cap of two.
- **Dual-role guns** (ZiS-3, critic pass 2). They are AT cards, outside
  the artillery cap. They may fire indirect only while the player's guns
  firing indirect, this one included, stay within `limits.artillery`.
  Otherwise a Soviet player could field many ZiS-3s as an uncapped
  battery.
- Per-card cooldowns (owner decision Q21 B).
- US Forward Observer Party: a shorter fire-direction delay (§4.8) as a
  doctrine trait, not a stat buff.

### 4.8 Flight time and fire-direction delay

- **Flight time comes from the physics** (S5's charge solver; projectiles
  at 1/60 s substeps). Drag-free examples: leIG 18 high angle to 1 km,
  about 24 s; a 105 mm low angle to 1 km, about 5 s; an 81 mm mortar to
  1 km, about 20 s.
- **Fire-direction delay** between the observer's call and the first
  round: `fire_direction_s` per faction, USA 15 s, Germany 25 s, USSR
  40 s (data; a doctrine trait). Adjusting rounds come first in observed
  mode (`adjust_rounds`, 1), then fire for effect.
- Incoming shells play a whistle 1–2 s before impact (presenter only).

**Effect.** From the call to the first burst is 20–60 s. A squad on the
move is gone by then. Artillery hits positions: trenches, guns, HQs,
dumps, buildings. Men in the open who keep moving are hard to catch.
That steers HE away from the easy kills that drive today's HE share.

### 4.9 How the brakes stack: a gun parked in base

Kessel Ridge. Germany buys a leIG 18 at 3:00 and emplaces it in its
base. The centre point is about 590 m away; the enemy base about 1.2 km.
Everything is in range. What stops it from ruling the map?

1. **No observer, no aim.** Without a linked observer it can only map-fire
   at known points, with an aim error of 6 % of the range (at least
   50 m) and no ranging-in.
2. **The observer must walk.** A VB-Trupp walks to a crest with a view of
   the target. Two men, easy to kill. Kill the radio man and the link is
   gone.
3. **Distance costs a little accuracy.** The first-round aim error is a
   share of the range, so 1 km is worse than 500 m. Once ranged in, the
   difference is small.
4. **Every shell is paid for.** The gun arrives with 14 rounds (plus 4
   the crew carry). After that each leIG shell costs about 6 manpower,
   in 32 kg crates. Ten shells a minute is about 60 manpower a minute,
   a quarter of base income. The base does not refill the gun.
5. **It is found.** After 3 rounds the enemy has a marker on the base.
   Enemy guns can reach it. **Enemy mortars cannot** while mortar range
   stays at today's compressed 600 m: the base is about 1.2 km away.
   So only enemy artillery or a raid on foot can answer a base gun.
6. **It may be seen.** A base in view of the enemy's front is spotted by
   flash on the first round. Many bases will not be.

**Honest verdict (critic fix C9, revised for the Q4 decision).** The base
is not "legal but poor". On a 1 km map it is probably the *best* place
for a gun. Every brake above works the same wherever the gun stands
(observer, price), or favours the base (no carrying, far from enemy
mortars and infantry). The only brakes that punish the base are
counter-battery by enemy guns and a raid. This is also historical: guns
stood in the rear. It is also why the price must not fall or rise with
distance from base (D2, §4.11): any surcharge on forward shells makes
this best place cheaper still.

**What changed with the ration gone.** Before, a base gun only moved HE
between weapons: it shared one capped pot with the mortars. Now a base
gun **adds** HE if the player pays for it. In base the carrying time is
almost nil, so the price is the main brake there. That is weaker. §4.10
lists the levers that make up for it.

What to watch:

- **"You need artillery to fight artillery."** With 600 m mortars, a
  player without guns can answer a base gun only with a raid. Test 7
  reports how often that happens and who wins. If it is a problem, the
  fix is the separate mortar-range step (§3.2), not a gun change.
- **The switch.** `indirect_from_base = false` (a gun emplaced in its
  own base zone fires direct only) exists, off by default. On small maps
  it helps less than it seems: the gun moves 50 m outside the zone and
  is in the same place. Use it only if tests show base guns winning
  matches, and pair it with a raid test.

### 4.10 Without the ration: what brakes each exploit

The critic pass (below) closed several exploits with the ration. The
owner removed the ration (Q4). Each one is re-checked here. "Weaker"
means the new brake is softer than the ration was; each weaker row gets
a TOML lever with a starting value.

| Exploit | What braked it before | What brakes it now | Verdict | Lever (start value) |
|---|---|---|---|---|
| **Base battery.** A gun in base fires all match. | The ration capped its HE; it shared the pot with mortars. | No base refill; every crate costs manpower; carrying a crate from the issue point; the live shells-crate cap; the artillery cap (2); the observer rule; counter-battery by enemy guns. | **Weaker.** In base the walk is short, so price is the main brake. A rich player fires more. | `shell_mp_per_kg` (1.0); `base_crate_issue_s` (10 s per crate, one at a time per player, so at most about 190 kg a minute); `shell_crate_cap` (8); `indirect_from_base` (true; false as fallback). |
| **HE dominance overall.** Guns plus mortars push HE above 40 % of kills. | One hard tonnage cap per player per minute, whatever the income. | Price per kg against men bought with the same manpower; crate caps; gun caps; mortars now also pay (from S6); HE hits positions (§4.8); trench model (S3b). | **Weaker.** HE now scales with income. A player who wins points earns more and can buy more HE. | `shell_mp_per_kg` (1.0), set from S6's measurement. Fallback, off at the start: `shell_price_rise_pct` (0): the price per kg rises by this share for each 100 kg bought in the last 5 minutes. It is a soft ration, so it stays off unless tests fail and the owner agrees. |
| **New guns bring free HE (C5).** Each card arrived with a full load outside the cap. | Starting loads drew from the base stock. | The load is in the card's price. The price check: a card's cost must be at least its load at crate price. The tow arrives half full. Cooldowns (90–180 s), `max_alive`, the artillery cap. | **Closed**, if the price check holds. Buying a gun is never a cheaper way to buy shells. | `load_price_check` (true); `tow_starting_load_pct` (50). |
| **Recrewing past the cap (C7).** Let a gun go neutral, buy another, recrew the old one. | The cap rule (not the ration). | Unchanged: own neutral guns count until destroyed or served by the enemy. New: a spiked or destroyed gun frees its slot, but the next card pays for its load again (price check). | **Same.** | — |
| **Recrewing for shells.** Recrew a neutral gun to get its leftover rounds. | The ration did not cover it. | The rounds were paid for when the card was bought. Nothing new is made. | **Closed.** | — |
| **Capturing enemy shells.** Take an enemy dump or crates, fire his shells from a captured gun. | Captured stock bypassed the ration too. | Crates feed only their own faction's calibres; a captured gun is inside the artillery cap. The enemy paid for those shells. | **Same, and historical.** | — |
| **Forward stockpile spike.** Fill the HQ and three dumps, then fire a "delete this trench" barrage. | The base stock cap (360 kg) bounded one dump of fire. | Every kg paid for at base and carried forward; HQ cap 600 kg, dump cap 300 kg; stocks cook off when HE breaks them; the gun fires no faster than its rate. | **Weaker.** A full HQ and three dumps hold about 1.5 t, about 1,500 manpower. It takes many minutes of carrying and is a big target. | Dump shell cap `cap_kg.shells` (300); HQ (600). Report the largest single barrage per match. |
| **Counter-battery duels crowd out the fight.** | A round at a gun was a round not fired at men, from one pot. | The same, by price. | **Same.** | Reported (share of shells fired at guns). |
| **Direct fire at infantry; HE rifle grenades.** New HE paths. | Both drew from the ration. | Both pay: gun rounds come in priced crates; HE rifle grenades are in `crate_only_classes` and come in shells crates. | **Same.** | `crate_only_classes`. |
| **Dual-role gun in base (critic pass 2).** A ZiS-3 is an AT gun, so it rearmed free in base, and it fires indirect to about 13 km. Free, unlimited HE from base. | The ration covered every restock. | Its HE is crate-only (§4.2 rule 4). It may fire indirect only inside the artillery cap (§4.7). Since D1 its AP rearm in base is priced too. | **Closed.** | `crate_only_if_indirect` (true). |
| **Laundering through cargo (critic pass 2).** Load "tank rounds" free on a truck or tow in base, unload them into a dump next to base; the pooled `shells` kg feed a howitzer. Or move rack rounds out of a tank or AT gun. | The ration covered every restock. | Every kg loaded as cargo pays its kind's price; since D1 no kind is free, and one price per kind leaves no cheap calibre to launder. Rack rounds never leave the rack, so the unqueued rack rearm and the rare-round cap cannot be bypassed (§4.2 rules 1, 2). | **Closed.** | `rack_rounds_transferable` (false). |
| **Free small arms and tank rounds (D1).** Rifles, MGs, rockets, tank and AT-gun racks refilled free in base, so a tank could fire all match at no cost. | — | Every kind has a price; a base refill is charged round by round and pauses with no manpower. | **Closed.** | `small_arms_mp_per_kg` (0.2), `explosives_mp_per_kg` (0.5), `shell_mp_per_kg` (1.0). |
| **Endless tank push (D3).** A tank rearms near a forward HQ or truck and never goes back. | Rare rounds were base-only. | It still needs a stock that was carried forward and paid for. Its rack is the hard limit; rare rounds refill forward only at 1 per 30 s and only to the vehicle's cap (≤ 10). Repair and new crew stay base-only. | **Bounded.** | `rare_rearm_s` (base 10, forward 30); `rare_cap_max` (10); per-vehicle `rare_cap`. |
| **New tank cheaper than a refill (critic pass 3).** Tank cards are exempt from the price check, and a full rack at 1.0 per kg cost more than the tank (Panzer IV about 850 against 640). Running a tank dry and buying another beat refilling it. | — | A full rack refill costs at most 75 % of the card; the factor applies in base and, as a credit, forward (§4.2). | **Closed.** | `rack_refill_max_card_pct` (75). |
| **Stacked sources (critic pass 3).** A tank between an HQ, a dump and two trucks took rounds from each at once, so rare rounds came four times as fast. | — | One source per client per tick, the fastest in reach (§4.2). | **Closed.** | — |
| **Rare rounds in the tow (critic pass 3).** A tow or truck def with no `rare_cap` took the default 10, so a Pak 40's tow could hold 10 PzGr 40 beside the gun's 2. | — | A gun and its own tow share the gun's cap; `rare_cap_max` is the default only for a def that lists a rare round, every other def holds 0 (`vehicles.md` §6.11). | **Closed.** | — |
| **Distance surcharge (rejected, D2).** A price that rises with distance from base. | — | Not built. It would make the base, already the best gun position, the cheapest too (§4.11). | **Not a hole: a rejected lever.** | None. There is no distance key. |
| **Saved-up truck load (critic pass 2).** Save 2,000 manpower, fill a truck with 2 t of shells at once, drive it to a dump. | The ration capped tonnage per minute. | Vehicle loads share the crate issue queue: 32 kg per `base_crate_issue_s` (§4.2 rule 3). 2 t takes about 10 minutes. | **Same as the crate limit.** | `base_crate_issue_s` (10). |
| **Free stock at the front (critic pass 2).** An HQ trickle (an old §6.4 option in `infantry-structures.md`: 15 kg of shells a minute) or a free carrying party fills an HQ beside the base forever. | Not covered: the ration capped issue at base only. | Q2 C: no trickle, no carrying party. The content check refuses any shells trickle. | **Closed.** | `trickle_kg_per_min.shells` must be 0. |

**The bottom line.** Three exploits are as closed as before. Three are
weaker: the base battery, overall HE, and the stockpile spike. All three
come down to one fact: the price limits HE by manpower, not by time. The
main lever is `shell_mp_per_kg`. Tests 7 and 11 measure it, and the
HE-share targets in §6.2 now include the rich side of each match.

**Can a player get unlimited HE from base, or from a dump next to it?**
Through the intended paths, no. Every kg costs manpower, and base issue
stops at about 190 kg a minute per player, whatever the carrier. A dump
beside the base saves the walk, not the price or the queue. Critic pass
2 found four back doors that did give free or unbounded HE: the ZiS-3,
cargo laundering, saved-up truck loads and a shells trickle. Each is
closed by one data rule above. Since D1 nothing at all is free: the
rules that stay do so for the queue, the rare-round cap and Q4's
crate-only rule, not to stop free HE.

### 4.11 Distance: delivery, not a surcharge (owner decision D2)

**The owner's idea:** "maybe a distance of the artillery from the base.
If they are pushing too much, it will take longer for the ammo to
arrive, and thus taking more resources from the overall army."

**Decision: physical delivery only.** Pushing forward costs more because
shells must be carried by men or driven by trucks from the base or the
HQ. That time, the exposure and the losses are the cost. There is no
price surcharge by distance.

**Why no surcharge.** A price that rises with distance from base makes
the base the cheapest place to fire from. The base is already the best
place for a gun (§4.9): far from enemy mortars and infantry, and with no
carrying. A surcharge would reward base batteries even more and punish
the forward guns that can be found and overrun. It is also an abstract
rule: a number from a distance, not contact.

**What pushing forward really costs.** Every shell at a forward gun came
there on a man's back or in a vehicle:

- **Time.** A shells crate (32 kg) moves at 60 % of walking speed with
  one man. At an assumed walk of 1.5 m/s *(check against the squad
  defs)*, carrying it 600 m takes about 11 minutes, and the walk back
  about 7. That is about five leIG 18 rounds, or under two 105 mm
  rounds, per man per 18 minutes.
- **Men.** A carrier cannot fire his main weapon. Men carrying crates
  are men not holding the line. That is "taking more resources from the
  overall army", in the owner's words.
- **Exposure.** Carriers, crates, trucks and dumps can be shot. Shells
  and explosives cook off when HE breaks them. A truck is soft-skinned.
- **Losses.** A carrier killed on the way delivers nothing, and the kg
  he carried was already paid for. The farther the push, the longer the
  route that can be cut.
- **Stock in the wrong place.** An HQ or dump near the front saves the
  walk but holds paid kg where the enemy can reach them (HQ 600 kg, dump
  300 kg of shells).

**Trucks shorten the walk, not the price.** A truck carries 2.5–3 t,
loads at 32 kg per `base_crate_issue_s` at base, costs manpower itself,
and can be lost with its whole cargo. The same holds for tanks (D3): a
tank far forward rearms only from a stock that someone brought there.

**Test.** A shells crate issued at base costs the same whatever its
destination; no price in the game depends on position (test 7c).

---

## 5. Direct fire

### 5.1 Over open sights

As in the genre reference, a howitzer or infantry gun can be pushed or
towed up and fire straight at what its crew sees.

- The gunner or section leader must see the target (S3 gunnery path:
  laying at the gun's traverse and elevation rates, lead, ranging-in per
  shot).
- The gun optic (`optic_sight_pct`, S9) applies inside the gun's arc.
- Firing direct exposes the gun: it is in line of sight, so it can be
  seen and shot back at. That is the trade for accuracy.
- leIG 18 and sIG 33 against bunkers and MG nests, and 105 mm against
  houses and tanks, were real uses. The leFH 18 and M-30 fired over open
  sights at tanks in the East.

### 5.2 Shells against targets

| Target | HE | HEAT (where historical) |
|---|---|---|
| Men in the open | Fragment rays (S3b A1) and overpressure. 105 mm: about 160 rays, reach 20 m (start values, tune). | Weak (few fragments). |
| Men in a trench | Only bursts in or right at the trench hurt them. Near-misses suppress (the duck state, S3b B1). Parapet erosion (§5.3). | — |
| MG nest, bunker, dugout (S10) | Structure pieces have HP and armour faces. HE damage scales with filler. Fragments go through firing slits. A 105 mm burst breaks a dugout roof (S10 done-when: an 81 mm does not). | HEAT jet through a slit or a timber wall. |
| Half-tracks, armoured cars, trucks | HE beats thin plate (S3b "HE beats a plate" with inner fragment rays). 105 mm HE beats about 20–25 mm (verify; set from filler). Open-top crews take fragments through the open face (S7). | Penetrates. |
| Tanks | HE does not penetrate the front of a Panzer IV or Sherman. It damages tracks (overpressure), the barrel and sight boxes, and external MGs. 15 cm HE may crack thinner plates (≈35–45 mm, verify). | Igr 38 HL (leIG 18): ≈85–90 mm flat (verify; sources vary a lot). M66 (75 mm pack): ≈90 mm (verify). M67 (105 mm M3, M2A1): ≈100 mm (verify). Gr 39 rot HL (leFH 18): ≈90–100 mm (verify). BP-350M (76 mm): ≈75–90 mm (verify). Low muzzle velocity: hard to hit a moving tank beyond about 300 m. |
| Guns | HE at the crew behind the shield and at the sight and recoil boxes (`vs_gun_pct`). | — |

AP shells: the leFH 18 had a Pzgr rot (verify). Counted as 1 per gun.

HEAT rounds are counted and scarce (2 per gun). They make a howitzer a
last-ditch tank stopper, not an AT gun. **The 2 is a hard cap**
(`max_ready` per ammo id in the gun def), and HEAT never goes into HQ
or dump stock (critic fix C10). The base no longer refills guns, so a gun
gets new HEAT only from a HEAT crate taken at base (2 rounds, priced like
shells) and carried to it. Otherwise a crate chain could feed a 105 mm a steady
stream of M67s that beat about 100 mm, enough for a Tiger's front at
close range.

### 5.3 Craters and parapet erosion

- **Crater size scales with filler.** The physical rule of thumb is that
  crater size grows with the cube root of the charge. Start from the S3b
  75 mm tank HE crater (0.6 m radius, 0.3 m deep, ≈0.68 kg filler):
  - leIG 18 Igr 18 (≈0.7 kg): ≈0.6 m, 0.3 m;
  - 105 mm (≈2.2 kg): ≈0.9 m, 0.45 m;
  - 15 cm sIG 33 (≈8 kg): ≈1.4 m, 0.7 m;
  - Nebelwerfer (≈2 kg): ≈0.85 m, 0.43 m.
- Today's 8 cm mortar crater (2.0 m radius, 0.7 m deep in `grw34.toml`)
  is out of line with this rule. Flag it for the S5 or S11 tune.
- **Parapet erosion** uses S3b A7 (`spoil_crater_pct`): a crater on loose
  spoil throws it away and leaves no rim. Bigger craters take more.
  Targets: lip lost per burst within 2 m of the lip ≥ 0.05 m for 75 mm
  (S3b T14), ≥ 0.15 m for 105 mm.

---

## 6. Interactions

### 6.1 Trenches (the core pillar)

Artillery is the historical answer to trenches, and the roadmap needs one:
the trench assault is still the weak point. What artillery should do to
a trench, and what it should not:

- **It should not empty it.** Men in a trench die only from bursts in or
  right at the trench. Target (Q16 A): trench losses under a barrage at
  most 40 % of the same barrage on men in the open.
- **It should pin it.** Near-misses fill suppression; men duck (S3b B1)
  and stop firing. That gives the assault its last 60 m.
- **It should wear it down.** Parapet erosion opens gaps; a 105 mm on a
  dugout roof breaks it.
- **It should be timed.** Flight time and fire-direction delay mean the
  attacker must plan: call, wait, then go. Lifting fire is the
  attacker's job.

Low-angle guns on small maps hit parapets and fronts more than trench
floors (§4.5). The leIG 18 and mortars do the plunging work.

### 6.2 The HE-share problem

**Today:** HE (mortars plus tank HE) causes about 40–50 % of kills (S3:
43 %; S3b start: 39.5 % with tanks, 49.8 % without). Mortars killed 406
men against 326 for rifles in S3's five logged matches.

**The risk:** artillery is more HE. Added on top of today's mortars, it
would make the problem worse.

**Rules for S11:**

1. **S11a ships only if S5 met its target** (HE share down by a third
   from the post-S3b baseline) **and S6 kept it there** with priced bomb
   crates.
2. **Every shell costs manpower** (§4.2). Shells compete with men for the
   same income. Unlike the old ration, this does not cap HE tonnage: a
   howitzer can add HE if its owner pays. §4.10 lists what makes up for
   that.
3. **Artillery hits positions** (§4.8), and positions are trenches, where
   HE kills far less.
4. **Counter-battery spends HE on guns**, not on infantry.

**Targets after S11** (20 AI matches, seeds 1–10, both base orders):

| Target | Value |
|---|---|
| HE share of kills, mean | ≤ 30 % (Q16 A: 20–30 %) |
| HE share, worst single match | ≤ 45 % |
| Kills by infantry guns and howitzers | ≤ 10 % of all kills |
| Trench losses under a 105 mm barrage (fortify-test row) | ≤ 40 % of open-ground losses |
| Share of gun shells fired at guns, structures, vehicles | reported |
| HE share of kills by the richer side (more manpower earned) in each match | ≤ 45 % in every match |
| Manpower spent on shells (crates, loads, tank rearm), share of each player's income | reported; AI 10–30 % |
| Manpower spent on small arms and explosives | reported (expected a few % of income) |
| Largest single barrage (rounds landed within 60 s on one 100 m cell) | reported |
| Minutes a gun sat with an empty ready inventory | reported |

**If HE is still too high, the levers in order** (the owner's rule:
cost, availability, AI and historical options before stat changes;
critic fix C11 reordered this list): the shell price
(`shell_mp_per_kg`), the shells-crate cap and the base issue time;
then caps, cost, cooldowns and unlock times; then AI behaviour (when it
buys and what it fires at); then the game values of the new rules
(map-fire and registered aim error, observer ranges, fire-direction
delay). Shell lethality, ranges and precision stay authentic. The soft
ration (`shell_price_rise_pct`, §4.10) is the last resort and needs the
owner's yes.

**Two HE paths to watch:**
- **Direct fire at infantry.** A leIG 18 over open sights at 400 m
  needs no observer and no delay. Humans will use it on men in the open.
  Each round is paid for, and the ≤ 10 % gun-kill target catches it.
- **HE rifle grenades** (S11b) are new HE carried by riflemen. They
  come only in priced shells crates (`crate_only_classes`, §4.2) and
  count in the HE share.

### 6.3 Friendly fire

S1 decided "no friendly fire": own rounds and blasts pass through own
men. With artillery this becomes an exploit. A player can shell an enemy
trench while his own men walk into it, and they are immune. Real armies
lifted fire because their own shells killed their own men.

**Owner decision Q22 (2026-11-01): yes.** Fragments and blast from
**indirect fire** (mortars, guns, launchers) hurt own men. Direct fire
stays as today: own bullets and direct-fire shells pass through own men.
This is physical and historical, and it makes timing the lift a skill.
The AI keeps `ai.lift_fire_m` and `danger_close_m` so it does not shell
its own assault. Ships in S11a; from then on mortars hurt own men too.

Side effect in team games: a player can shell his allies, by
mistake or on purpose. The danger-close HUD warning should also cover
allied men, and the end-of-match report should list friendly-fire kills
per player, so hosts can deal with griefers.

### 6.4 The AI

S3 showed that features the AI ignores never get balanced. The AI needs
these rules (weights in data):

| Behaviour | Rule |
|---|---|
| **Buy** | Tag `artillery`. Buy a gun only with a living observer (or buy an FO team first). Buy when enemy trench length or structures seen pass a threshold, or an enemy gun has been marked. Spend at most `ai.shell_mp_max_pct` (25 %) of income on shells: crates, vehicle loads and tank rearm in base (D1). Over budget, guns and mortars wait for crates; tanks still rearm to `ai.tank_rearm_floor_pct` (50 %), so armour never sits empty (`vehicles.md` §7.5). It keeps `ai.ammo_float_mp` (100) back from unit buys while any own unit waits to rearm. Any AI unit leaves a source that has served it nothing for `ai.rearm_stall_s` (15 s) for lack of manpower or stock, and does not go back until it has the float or the source has stock (critic pass 3). |
| **Site** | Score spots 300–700 m behind the own front (or in base on small maps): no enemy line of sight (the S6 reverse-slope probe), flat firm ground, reachable by the tow, the mask check clears to the three most likely targets, at least 50 m from other own guns. Ask idle engineers for a `howitzer_pit`. |
| **Observe** | The FO walks to a crest point with line of sight onto the enemy trench or approach (a few sampled points, ray-tested), goes prone, stays out of the open. |
| **Fire** | (a) Preparation fire on a trench timed with an assault: starts about 60 s before, lifts when the assault is 100 m out (friendly fire is on, Q22). (b) Defensive fire on registered approaches when attackers enter them. (c) Counter-battery on a marker when its error is under about 60 m and the gun holds more than `ai.ammo_reserve_pct` (30 %) of its ready inventory. (d) Direct fire (infantry guns) at spotted MG nests, bunkers and vehicles within 800 m, only with an escort near. |
| **Relocate** | After `ai.displace_after_rounds` (6) from one spot by its own count, or when enemy HE lands within `ai.displace_near_m` (30). It uses only its own knowledge, never the enemy's marker. |
| **Resupply** | Below `ai.resupply_pct` (50) rounds, send the tow or bearers for a crate: the nearest HQ or dump with shells first, the base (priced) only if none. Keep one crate placed by each gun when the budget allows. |
| **Recrew / capture** | As S9: idle infantry within 100 m serve a neutral gun with rounds, inside the artillery cap (§4.7). |

Every number in this table is an `ai.*` key in TOML (critic fix C12).

Cost: a few hundred rays per siting decision, every 10 s at most, inside
the AI's 20 % ray share; the mask check once per order.

**Feasibility (critic note C13).** This is the largest AI addition in
the plan, and S3 showed that the AI is where features fail. Make it
cheaper and safer:

- **Precompute observation posts at map load.** For each capture point
  and each base approach, sample candidate crest points and keep the
  few with the best ground view. Deterministic, done once, stored with
  the map's other baked data. The FO then just walks to the nearest
  safe post. No live search for crests.
- **Precompute gun areas the same way** (reverse-slope cells reachable
  by the tow, mask-clear to the capture points).
- **Ship in two steps inside S11a.** First: buy with an FO, site, fire
  observed missions at enemy trench cells near the AI's next objective,
  lift on the existing `ai.lift_fire_m` rule, resupply. Second:
  registration-based defensive fire, counter-battery and relocation.
  The second step only after the first passes test 12.
- The timed preparation (fire 60 s before, lift at 100 m) hooks into
  S3b's AI fire-and-move (B3): the attack on a point with a known
  trench is the trigger. S3b has no assault start time, so the AI
  fires when two or more squads are assigned to the point, holds their
  first bound for `ai.prep_fire_s` (60), and lifts on distance.

### 6.5 Performance

Battalion tier: 700 soldiers, 100 vehicles, 1,800-projectile pool,
2,400 rays per tick, `MAX_FRAG_RAYS_PER_TICK` 4,096 (S3b), 30 Hz.

| Item | Worst case (Battalion, 10 players) | Against the budget |
|---|---|---|
| Guns | 2 per player: 20 guns, plus mortars | Gun entities are vehicle-style; well under 100 vehicles with tanks capped. |
| Shell rate, steady | No hard cap now; income bounds it. If each of 10 players spends a quarter of ≈300 MP a minute on shells: ≈750 kg/min, about 40 rounds of 105 mm or ≈210 mortar bombs a minute. Short peaks are bounded by rates of fire and the in-flight guard (20 guns plus 30 mortars at full rate: up to ≈8 bursts per second) | ≈0.7–3.5 bursts per second steady; peaks inside the in-flight guard |
| Shells in flight | Steady: burst rate × flight time (5–30 s), under 100. Peak: a team dumping full stocks at once, bounded by the guard at 10 × 24 = 240 | Steady ≈5 %, peak ≈13 % of the 1,800 pool. Shells are never bundled (§2.2). Guard: `max_indirect_in_flight_per_player` (24) delays the next shot, deterministically. |
| Flash checks | Through the normal spotting scheduler (§4.3), no per-shot rays | A 20-gun volley adds 20 conspicuous targets, not thousands of rays. |
| Targeting overlay (§6.6) | Mask and range grid per gun, computed on the client when the gun emplaces | Presenter only; never touches the sim or its ray budget. |
| Fragment rays | A synchronised strike by 12 guns in one tick: 12 × 160 = 1,920 | Under 4,096. S3b scales `frag_count` down over the cap; S11 reports `frag_rays_capped` and expects 0 in AI matches. |
| Craters | One terrain op per burst; bigger craters touch more cells | Terrain ops are budgeted per tick; craters queue in burst order, never drop. Detail-tile inits stay at most 4 per tick. Test: the "200-shell barrage plus 20 squads digging" target (§2.2). |
| Mask checks | ≈20 segments × a few charge/elevation candidates per order | Per order, not per shell. |
| Sound ranging | One small record per gun per spot; one listener distance query per shot | Negligible. |

Regiment (2,000 / 300 / 5,000; 7,500 rays per tick) scales the same way
with three guns per player.

### 6.6 What the player sees (critic fix C14)

There are many rules here: five fire modes, two kinds of link,
registration, ready inventories and crates, masks, minimum range, markers. A player who
clicks and gets nothing will think the gun is broken. So:

- **Targeting overlay.** With a gun selected, the ground is tinted:
  can hit (observed), can hit (map fire only), masked, too close, out of
  range. Low-angle guns show the masked shadow behind crests. Computed
  on the client from the heightfield when the gun emplaces (§6.5).
- **One reason, always.** A refused order says why, in a few words: "No
  observer sees this", "No radio link", "Masked by the ridge", "Too
  close", "Out of shells: bring a crate", "Packed: emplace first",
  "Own men within 50 m", "No manpower for ammo".
- **The observer shows his link.** A line from the FO to each gun he can
  call, cut when the radio man dies.
- **Registered cells** show as small marks with a timer.
- **The gun's own exposure:** "6 rounds from this spot: likely located"
  (Q24, decided: hint only) and "Seen by flash" when the enemy spotted
  the muzzle.
- **Shells in reach.** "Rounds 14 / 15", "Crate 5 m: 6 rounds", "In HQ
  radius". A shells crate on the call-in bar shows its manpower price.
- **Card tooltips say what the gun is for on this map.** On a 1 km map
  a leFH 18 reads "Flat fire here: good against buildings and vehicles,
  weak against trenches." Without this, players will buy a howitzer to
  clear trenches and find it cannot (Q25).

---

## 7. Off-map artillery: drop it

**Recommendation: drop off-map batteries from skirmish.** Do not build
`SupportCall` in S11.

Why:
1. **Nothing can hit them back.** Counter-battery against an off-map gun
   would be an abstract effect, which the physical-contact rule forbids.
   On-map guns can be found, shelled and overrun.
2. **They would compete with the on-map cards** and make them pointless:
   why risk a gun when a radio call does the same?
3. **It saves work** (S–M): the support def, edge spawning, the
   allocation HUD.
4. **The owner prefers units.**

The alternative, a rare radio call through the HQ (one or two per match,
long cooldown, heavy calibres only), is reasonable. It is what (B) in
Q13 keeps open. It is not recommended for skirmish now. **Missions** may
still script heavy fire from beyond the map edge (Luau spawning real
shells on a true arc), for corps artillery or naval guns. That is a
scripting feature, not a card.

---

## 8. TOML shapes

The gun (vehicle-style def, S9 schema):

```toml
# defs/vehicles/ger_leig18.toml (excerpt)
id = "ww2:ger_leig18"
name = "7,5 cm leichtes Infanteriegeschütz 18"
short_name = "leIG 18"
faction = "ww2:germany"
role = "infantry_gun"
crew_model = "served"
mass_kg = 400                       # (verify)
spot_size_pct = 80
sources = ["placeholder: owner / M2 citation task"]

[mobility]
type = "towed"
push_cm_s = 110
push_men = 4
emplace_s = 20
pack_s = 12
hitch_s = 15
relay_trail_s = 10

[[weapons]]
def = "ww2:leig18_l11"
traverse_deg = [-6, 6]              # (verify)
elevation_deg = [-10, 73]           # (verify)
# Ready inventory: the starting load, and the most the gun can hold (Q4 decision)
ready = [{ id = "ww2:75mm_igr18", count = 12 }, { id = "ww2:75mm_igr38hl", count = 2 }]
max_ready = { "ww2:75mm_igr38hl" = 2 }  # rare cap (D3): HEAT refills from crate, HQ or dump at rare_rearm_s, never above 2
fetch_s = 4                         # one round per crew man from a crate within crate_feed_m

[indirect]
charges_m_s = [92, 110, 135, 165, 221]   # Igr 18, charges 1-5 (verify)
range_pe_pct = 0.8                  # precision, from the firing table (verify)
deflection_pe_mrad = 1.5            # (verify)
flash_m = 2000
cb_rounds_to_mark = 3
```

The card (S4 card block):

```toml
# defs/units/ger_leig18_section.toml
id = "ww2:ger_leig18_section"
extends = "ww2:ger_gun_crew_5"
name = "@loc:unit.ger_leig18_section"   # Infanteriegeschützgruppe
gun = "ww2:ger_leig18"
cost = 320
pop = 7
cooldown_s = 90
max_alive = 2
limit_group = "artillery"
unlock_s = 180
tags = ["gun", "artillery"]
capabilities = ["gun_crew", "dig_basic", "observe", "radio"]

# defs/units/ger_leig18_section_mot.toml
id = "ww2:ger_leig18_section_mot"
extends = "ww2:ger_leig18_section"
tow = "ww2:ger_sdkfz11"
```

Rules:

```toml
# defs/rules/skirmish.toml (additions)
[artillery]
fire_direction_s = { "ww2:germany" = 25, "ww2:usa" = 15, "ww2:ussr" = 40 }
adjust_rounds = 1
# Aim error (guns): share of range, with a floor in metres (critic C1)
first_round_error_pct = 3.0
first_round_error_min_m = 25
ranging_step_pct = 50               # aim error left after each seen round
map_fire_error_pct = 6.0
map_fire_error_min_m = 50
registered_error_pct = 1.0
registered_error_min_m = 10
registration_cell_m = 50
registration_expiry_s = 600
registration_max_cells = 6
observe_m = { fo = 1200, gun_leader = 400, leader = 400 }
observer_link_m = 150               # voice link to a gun section leader
cb_rounds_to_mark = { gun = 3, mortar = 5 }
cb_decay_s = 300                    # evidence falls by 1 per 300 s, steadily
cb_delay_s = 15
cb_error_pct = 15
cb_error_step_pct = 80
cb_error_floor_pct = 5
cb_reset_m = 75
flash_reveal_ms = 6000
danger_close_m = 50
indirect_from_base = true           # switch off only if tests show base camping
friendly_fire_indirect = true       # Q22, owner decision 2026-11-01: yes
cb_hint_rounds = { gun = 3, mortar = 5 }  # Q24: own-count hint only, never the enemy marker
max_indirect_in_flight_per_player = 24

[supply]
# Q4, owner decision 2026-11-01: no ration. Ready inventories refill only
# from crates, HQ or dump stock, or the tow. The base does not refill these.
# D1: no free ammo anywhere. Each kg pays its kind's price once, when it
# leaves the base (base-zone refill, crate issue or vehicle load).
# D2: there is deliberately no distance key; distance costs delivery.
crate_only_classes = ["mortar_bomb", "gun_shell", "rocket_artillery", "rifle_grenade_he"]
small_arms_mp_per_kg = 0.2          # cheap: a rifle squad's full refill is about 5 MP
explosives_mp_per_kg = 0.5          # moderate: a Panzerschreck rocket about 1.7 MP
shell_mp_per_kg = 1.0               # dear: mortar, gun, howitzer and tank main-gun rounds; set from S6
rack_rearm_price_pct = 100          # fallback lever: price share for rounds that go into a tank or AT-gun rack, base or forward
rack_refill_max_card_pct = 75       # critic pass 3: a full rack refill costs at most this share of the card; forward draws credited
# critic pass 3: one source per client per service tick (the fastest in reach); rates never stack
# critic pass 3: a main-gun or gun-entity round must be kind "shells" (content check)
base_crate_issue_s = 10             # one crate at a time per player at base
shell_crate_cap = 8                 # live shells crates per player, inside the general crate cap
crate_feed_m = 5                    # a crate this close feeds a gun or mortar
structure_refill_s = 4              # one round per 4 s to a gun inside an HQ or dump radius
tow_starting_load_pct = 50          # the tow arrives half full; the gun arrives full
load_price_check = true             # card cost raised to >= starting load x price (warning, not refusal); guns and mortars; tanks and AT guns only for crate-only rounds
rack_rounds_transferable = false    # rack rounds never leave the weapon: unqueued rack rearm and rare caps stay whole
crate_only_if_indirect = true       # an ammo id any [indirect] weapon fires is crate-only for all (ZiS-3; Q4 rule)
rearm_s = { base = 2, forward = 4 }        # one standard main-gun round; forward = HQ, dump, supply vehicle
rare_rearm_s = { base = 10, forward = 30 } # D3: one rare round (APCR, HVAP, HEAT), at any source with shells stock
rare_cap_max = 10                   # D3: most rare rounds one vehicle or gun may hold; default only for a def that lists a rare round (others 0); a gun and its tow share one cap
# base_crate_issue_s also paces vehicle shell loads: 32 kg per tick, one queue per player
# trickle_kg_per_min must be 0 for every kind in every rules file (Q2 C, D1; content check)
shell_price_rise_pct = 0            # fallback soft ration, off; per 100 kg bought in the last 5 min

[limits]
artillery = 2                       # Regiment: 3, in battle_tiers.toml; own + captured served guns; dual-role guns (ZiS-3) while firing indirect
allow_field_howitzers = true        # Q25: all maps; a map may set false only if tests demand it

[ai]
shell_mp_max_pct = 25               # most of its income the AI spends on shells (crates, loads, tank rearm)
tank_rearm_floor_pct = 50           # over budget, a tank still rearms to this share of its rack
rearm_stall_s = 15                  # critic pass 3: leave a source that served nothing this long (no manpower or stock)
ammo_float_mp = 100                 # critic pass 3: kept back from unit buys while an own unit waits to rearm
ammo_reserve_pct = 30               # rounds kept for defence and counter-battery
displace_after_rounds = 6
displace_near_m = 30
resupply_pct = 50
prep_fire_s = 60
cb_max_error_m = 60
direct_fire_max_m = 800
site_behind_front_m = [300, 700]
gun_spacing_m = 50
```

The 105 mm HE shell:

```toml
# defs/ammo/105mm_m1_he.toml (excerpt)
id = "ww2:105mm_m1_he"
class = "shell_105_us"
mechanism = "he"
mass_kg = 15.0                      # projectile (verify)
supply_kg = 19.0                    # complete round with case (verify)
he_filler_g = 2180                  # (verify)
[he]
frag_count = 160
frag_reach_cm = 2000
frag_damage = 80
frag_far_pct = 30
blast_radius_cm = 250
pen_mm = 22                         # beats thin plate (verify)
crater_radius_cm = 90               # cube-root scaling from 75 mm
crater_depth_cm = 45
```

---

## 9. Slice placement

Full card in `roster-expansion.md` §4.2 (S11). In short:

| Slice | Content | Size vs S3 | Depends on |
|---|---|---|---|
| **S11a** Indirect fire and infantry guns | Ready inventories for guns, fed by crates, HQ and dump (priced shells crates come in S6); starting loads in the card price; observer links by radio and voice; fire modes; registration; mask check; fire-direction delay; sound and flash ranging; displacement; `howitzer_pit`; crater scaling; gun direct fire with HE and HEAT; AI artillery planner; Q22 friendly fire; Q24 hint. Content: VB-Trupp, FO Party, leIG 18 (pushed and towed), 105 mm M3. | ~0.9× | S5, S6, S8, S9, S3b (Q4 answered) |
| **S11b** Field howitzers on the map | leFH 18 and M2A1 sections (data plus tuning); 75 mm pack howitzer (data); kz. GrW 42, 60 mm M2; M7 rifle grenade and *Schießbecher*. | ~0.4× | S11a, measured |
| Later | sIG 33; Nebelwerfer 41 if Q23 (A) | data, a small rocket-thrust addition, two models | S11b, measured |
| S15 | 76 mm M1943, M-30 section, (BM-13 if Q23) | data | S11b |

---

## 10. Tests (`frontline-cli gun-test` and AI matches)

1. **Observer rule.** No linked observer: an observed mission is refused.
   Map fire only on knowledge points. Gun mean miss: map fire about 2×
   an observed first round, and well above a ranged-in round (mortars:
   S5's 3×).
2. **Radio link.** Kill the FO's radio man: the link breaks until a man
   picks up the set. Destroy the HQ radio box: observers near it lose
   that relay.
3. **Masks.** A leFH 18 and an M3 told to hit a trench 80 m behind a
   crest from 600 m report "masked". A leIG 18 at high angle hits it.
4. **Registration.** Unobserved fire on a registered cell uses the
   registered aim error; after the gun moves 75 m, or after 600 s, the
   cell needs an observer again. A gun in base that registered every
   capture point at 5:00 cannot fire accurately on them at 16:00 with no
   observer alive.
5. **Sound ranging.** A gun firing from one spot gets an enemy marker
   after 3 rounds, error ≤ 15 % of the listener distance, shrinking each
   round. A gun that displaces after 4 rounds survives counter-battery
   more often than one that stays (10 runs each). A gun firing one round
   a minute from one spot is marked by its 4th round. A gun hopping 76 m
   after every two rounds keeps its marker if the error is above 76 m.
6. **Flash.** An enemy with line of sight spots the gun on its first
   round; a reverse-slope gun is not spotted by flash.
6b. **Hint only (Q24).** The firing player sees "N rounds from this spot:
   likely located" after his own 3rd round from one spot. His client
   never receives the enemy's marker or its error (snapshot check).
7. **Base camping and supply.** A gun parked in base does not refill
   from the base: when its ready inventory is empty it stops until a
   crate comes within 5 m. A shells crate taken at base costs its kg ×
   `shell_mp_per_kg` in manpower; with too little manpower none is
   issued; a small-arms crate costs its kg × `small_arms_mp_per_kg`. A
   gun inside an HQ or dump radius refills from its stock, the stock goes
   down by the same kg, and no manpower is charged a second time.
   A called gun arrives with its full starting load; a card worth less
   than its load at crate price is raised to it, with a warning. AI matches
   still end with a winner inside 25 min (stall guard). Report the share
   of gun shells fired from inside the base zone, the manpower each side
   spent on shells, and the win rate of a side with base guns against a
   side with none (raids only).
7b. **No back doors** (critic pass 2, revised for D1). A truck loaded
   with shells at base costs its kg × `shell_mp_per_kg` and loads no
   faster than 32 kg per `base_crate_issue_s`, shared with crates; a
   small-arms load pays its own kind's price. A tank or AT gun rearmed
   in base cannot pass rack rounds to a crate, dump, tow or gun. A ZiS-3
   in base gets no OF-350 from the base zone, and a third gun firing
   indirect past the cap is refused. A rules file with a trickle above 0
   for any kind fails to load. Over 10 AI matches, no weapon fires a
   round whose kg was not paid for at base or captured (a debug count,
   now covering rifles, rockets and tanks too).
7c. **Prices (D1, D2).** A rifle squad refilling in base pays its kg ×
   `small_arms_mp_per_kg`; a tank rearming in base pays each round's kg
   × `shell_mp_per_kg` × its rack price factor; with no manpower the
   refill pauses and resumes when income arrives. A shells crate costs
   the same whether it is set down in base or carried 800 m forward;
   no price depends on position: a tank rearming from an HQ ends up
   paying the same per round as in base (the credit). No vehicle's full
   rack refill costs more than 75 % of its card. A tank between an HQ
   and two trucks rearms no faster than at one of them. A Pak 40 and
   its tow together never hold more than 2 PzGr 40. Report manpower
   spent per kind, per player, per match.
7d. **AI under priced ammo** (critic pass 3). With the AI's manpower held
   at 0 for 2 minutes, no AI tank, gun or squad waits at a source longer
   than `ai.rearm_stall_s`; matches still end with a winner.
8. **Direct fire.** A leIG 18 at 400 m silences an MG nest; a 105 mm burst
   breaks a dugout roof that survives an 81 mm hit; M67 HEAT penetrates a
   Panzer IV side; 105 mm HE on a Sherman damages only external parts and
   tracks.
9. **Trenches.** fortify-test row "105 mm barrage": trench losses ≤ 40 %
   of open; lip loss per 105 mm burst within 2 m ≥ 0.15 m; a preparation
   barrage cuts attacker losses to at most half the no-support baseline
   (owner sets the bar).
10. **Friendly fire** (Q22, decided). Own men inside a mortar, gun or
    launcher burst take fragments and blast; own direct-fire rounds still
    pass through own men; the AI lifts fire in time in ≥ 9 of 10 staged assaults.
11. **HE share.** First a control run: priced crates on, no guns, S6's
    content. The HE share must not rise above S6's. Then §6.2 targets
    over 20 AI matches with guns, including the richer side's HE share
    (≤ 45 % in every match). If it fails, raise `shell_mp_per_kg` first
    (§6.2 lever order).
11b. **Cap.** A player with two guns, one of them neutral, cannot call a
    third gun or re-crew the neutral one past the cap.
12. **AI use.** The AI buys a gun and an observer in ≥ 6 of 10 matches,
    fires observed or counter-battery missions, never fires indirect
    without an observer except on knowledge points, and displaces after
    counter-fire. It keeps its guns fed with crates and spends no more
    than `ai.shell_mp_max_pct` of its income on them.
13. **Performance.** Stress scene: 20 guns and 30 mortars firing at
    their full rate on Battalion, fed by crates: `frag_rays_capped` = 0 in AI matches;
    stress p99 inside S3b T13; the 200-shell barrage target holds.
14. **Determinism.** Save and load with shells in flight, a gun mid-pack
    and a marker sharpening; hashes equal at 1, 2, 3 and 8 threads and in
    net loopback.

---

## 11. Owner questions

Also listed in `roster-expansion.md` §5 and its "Owner decisions" table.

- **Q13 (changed). Divisional artillery** [S11]. (A) On-map single-gun
  sections, off-map dropped from skirmish (this file). (B) Off-map
  batteries called by an observer (the old default). Decided: on-map.
- **Q4. Answered 2026-11-01.** No ration. Each gun and mortar has a ready
  inventory. It refills from crates nearby or from an HQ or dump, whose
  stock comes only by crates (later trucks). The base does not refill
  shells; mortars follow the same rule from S6. Shells crates cost
  manpower at base (`shell_mp_per_kg`). Starting loads come with the
  card. §4.2, §4.10.
- **D1–D3. Answered 2026-11-01 (owner requests, not from the question
  list).** D1: no free ammo anywhere; small arms 0.2, explosives 0.5,
  shells 1.0 manpower per kg (§4.2). D2: distance costs delivery, no
  surcharge (§4.11). D3: tanks rearm at base, HQ, dump or supply truck,
  every round priced; rare rounds refill forward too, slowly, up to a
  per-vehicle cap of at most 10 (`vehicles.md` §6.11, §7.6).
- **Q22. Answered 2026-11-01: yes.** Fragments and blast from mortars,
  guns and launchers hurt own men; direct fire stays as today (§6.3).
- **Q23. Rocket launchers** (Nebelwerfer 41, later BM-13) [after S11b].
  Still open. (A) Later, as a lobby option, cap 1, 12:00 unlock, after
  the shell price is measured with tube artillery. (B) Missions only.
  (C) In S11b.
- **Q24. Answered 2026-11-01: hint only.** The player sees his own count
  ("N rounds from this spot: likely located"), never the enemy's marker.
- **Q25. Answered 2026-11-01: field howitzers on all maps.** Cap 1 per
  player and the 8:00 unlock stay as TOML defaults to measure. A range
  tweak does not fix flat fire on small maps (targets are in range
  anyway), and a per-map max range has no real use, so none is offered
  (§4.5).

---

## Critic notes (2026-11-01)

An adversarial review of this file and of the S11 and Q13 changes in
`roster-expansion.md`. Fixes were made in place; each is tagged
"critic fix Cn" in the text. Nothing here is built or tested. Historical
figures stay *(verify)*.

**Verdict.** The design holds, but it had one hole that would have
undone it (C1) and several cheap exploits. With the fixes, the honest
picture is: artillery will mostly fire from the rear, often from the
base, and that is fine *only* because the ration caps total HE. The
ration is now the whole defence against HE dominance. If the owner
rejects Q4 (B), this design needs rethinking.

**Update 2026-11-01, after the owner's Q4 decision.** The owner rejected
the ration. The rethink is §4.2 (ready inventories, priced shells crates,
no base refill of shells) and §4.10 (each exploit below re-checked). The
fixes that leaned on the ration are marked "now" in the table. The
honest result: the design still holds, but HE is now limited by
manpower, not by time, so a rich player can fire more. That is weaker,
and the main lever is the shell price.

**Exploits and holes fixed**

| # | Problem | Fix |
|---|---|---|
| C1 | **Map fire would be almost as good as observed fire.** Spread was one number multiplied by 300 % for map fire. Mortars use a large game spread (50 mrad). An authentic howitzer spread is a few mrad, so 3× of it is still a few metres. The observer brake would vanish for guns. | Two error terms for guns: an aim error (share of range, with a floor in metres) that the observer removes, plus the authentic precision spread on top (§4.1). |
| C2 | **Registration made the observer a one-time cost.** A gun that never moves (in base) could register every capture point early and fire accurately all match with no observer. | Registration expires (600 s), has a residual error, and is capped at 6 cells per gun. |
| C3 | **A base gun needed no observer.** The section leader observed to 600 m; on Kessel Ridge the centre point is about 590 m from a base. | Section leader observe range 400 m, like other leaders. |
| C4 | **The FO was a free scout.** 1,200 m ground observation could be read as spotting. | `observe` lets a man aim and adjust; it reveals nothing beyond normal spotting. |
| C5 | **Starting loads bypassed the ration.** A leFH 18 and its tow arrive with about 580 kg of shells, ten minutes of ration, free with each card. "A second gun adds no HE" was false. | Starting loads draw from the base stock; the card shows a short load before purchase. **Now:** loads come with the card's price; a content check keeps every card worth at least its load at crate price; the tow arrives half full (§4.10). |
| C6 | **Slow fire and short hops were never found.** With "3 shots in 120 s within 75 m", one round a minute, or a 76 m hop every two rounds, never produced a marker. | Evidence count with steady decay; a shot within the current marker error still counts for that spot. |
| C7 | **Cap dodge.** Let a gun go neutral, buy another, re-crew the old one with riflemen. | A player serves at most the cap, own or captured; his neutral gun still counts. |
| C8 | **Flash could cost thousands of rays in one tick** if done as a ray from every enemy per shot. | Firing makes the gun conspicuous; the normal perception scheduler handles it. |
| C9 | **"The base is legal but poor" was wrong.** Every brake either ignores position or favours the base. Also "every enemy mortar can reach it" was false: mortars stay at 600 m, the enemy base is about 1.2 km away. | Said plainly in §4.9. Base guns are accepted because of the ration. Test 7 now measures base fire and the no-artillery matchup. **Now:** accepted because the base refills no shells and every crate costs manpower; weaker than the ration (§4.10). |
| C10 | **HEAT stream.** Crates could feed a 105 mm many M67s (≈100 mm). | Hard cap of 2 at the gun; HEAT restocks only in base. **Now (D3):** HEAT is a rare round: it refills from a crate, HQ or dump at one per 30 s, never above 2 at the gun. The cap, not the source, stops the stream. |
| C11 | **Lever order broke the owner rule.** Stat-like values (spread, observer range, delay) came before cost and caps. | Reordered: ration, then cost, caps, cooldowns, unlocks, then AI, then rule values. **Now:** the shell price, crate cap and issue time come first in place of the ration. |
| C12 | **Values outside TOML.** `observer_link_m`, the AI thresholds and others were only in prose. | Added to the TOML block (§8). |
| C13 | **AI scope was too big for one step.** Live crest search, timing with an assault start that S3b does not have. | Precomputed observation posts and gun areas at map load; S11a's AI ships in two steps; preparation fire hooks into S3b B3 fire-and-move. |
| C14 | **Too many silent rules for a player.** | §6.6: targeting overlay, one refusal reason per order, link lines, registered-cell marks, map-aware tooltips. |

**Historical and number errors fixed**

- **Nebelwerfer "200 kg of HE" was wrong.** A salvo is about 204 kg of
  ammunition, but only about 12–15 kg of explosive *(verify)*: about six
  105 mm shells. Under a weight-based price the launcher is a poor deal
  in explosive per kg. Its real danger is surprise. The Q23 text is
  corrected here and in `roster-expansion.md`.
- leFH 18 charge 1 at 800 m: the flight is about 4 s drag-free, not 6 s.
- leIG 18 top charge: about 221 m/s *(verify)*, not 210, in the TOML
  sample.
- PM-38: about 280 kg is the firing weight; on its wheels it is nearer
  480 kg *(verify)*.
- Still to verify, not changed: the 105 mm M2A1 burst rate (some
  sources give far more than 4 rounds a minute for the first minutes);
  which German tow the leIG 18 really used (the lighter Sd.Kfz. 10 or a
  light truck is more typical than the Sd.Kfz. 11; one tow model is a
  fair art saving); which 105 mm guns had shields.

**HE dominance: what is left**

- With authentic precision, a ranged-in gun is very accurate at 1 km.
  Trench safety now rests on S3b's fragment and parapet model and the
  "≤ 40 % of open" fortify-test row. If that row fails, the fix is in
  the trench model, not gun scatter.
- The shell price is a guess (1 manpower per kg). S6 sets it from
  mortar use with priced bomb crates, and the first S11a measurement is
  a control run with priced crates and no guns (test 11).
- With no base stock cap, a forward stockpile (HQ plus three dumps,
  about 1.5 t) can feed a large barrage. It is paid for, carried and
  exposed; the largest barrage per match is reported (§6.2).
- Direct fire at infantry and HE rifle grenades are new HE paths. Both
  come in priced crates; both are counted.

**Not fixed, for the owner**

- **Base artillery will be common on 1 km maps.** It is historical and
  capped, but players who dislike being shelled by guns they cannot
  reach will complain, as they do in the genre. The `indirect_from_base`
  switch exists; it does little on small maps (§4.9).
- **Artillery mostly answers artillery.** With 600 m mortars, only guns
  or a raid on foot reach a rear gun. Making mortars authentic fixes
  that but adds HE reach; do it as its own measured step.
- **Q22 (decided yes) in team games** lets a player shell his allies. Warnings and
  a friendly-fire report per player are proposed (§6.3).
- **S11a is big.** About 0.9× S3 is optimistic for ready inventories, links,
  five fire modes, registration, masks, sound and flash, craters, direct
  fire, friendly fire and an AI planner. If it runs long, split it:
  S11a1 (ready inventories, links, fire modes, masks, leIG 18 and M3, basic AI);
  S11a2 (registration, counter-battery, displacement, defensive-fire
  AI).
- **S11b is not "no new code".** HE rifle grenades need a launcher mode
  on the rifleman (switch, fire, switch back). It is small and reuses
  S5's launcher items, but it is code.

**Critic pass 2 (2026-11-01, after the Q4 revision).** The question was
whether a player can now get unlimited or near-unlimited HE from base,
or from a dump next to base. Through the intended paths he cannot:
every kg pays at base, and base issue stops at about 190 kg a minute.
But four back doors gave free or unbounded HE (C15–C18), and the price
check fought the main lever (C19). Each is now closed by one data rule (§4.2 "No back doors", §4.10, test 7b):

| # | Hole | Fix (lever) |
|---|---|---|
| C15 | **ZiS-3 in base.** An AT card, so it rearmed free, and it fires indirect to about 13 km: unlimited free HE from base. It was also outside the artillery cap. | Its HE is crate-only (`crate_only_if_indirect`, also covers later SPGs and 105 mm tanks); it fires indirect only inside `limits.artillery`. **Now (D1):** its AP rearm is priced too; the rule stays for Q4's crate-only rule and the queue. |
| C16 | **Cargo laundering.** `shells` kg are pooled. Free "tank rounds" loaded on a truck or tow in base unload into a dump and feed a howitzer. Rack rounds could be moved out of a tank. | Every `shells` kg loaded as cargo pays; rack rounds never leave the rack (`rack_rounds_transferable = false`). **Now (D1):** every kind pays; the rack rule stays for the queue and the rare-round cap. |
| C17 | **Saved-up truck load.** Only crates were throttled. A truck could take 2 t at once. | Vehicle loads share the crate queue: 32 kg per `base_crate_issue_s`. |
| C18 | **Free stock at the front.** `infantry-structures.md` §6.4 still recommended a 15 kg/min shells trickle, and S6 and S8 still listed a free carrying party, against Q2 C. An HQ beside the base would fill for free forever. | Removed in both docs. The content check refuses a shells trickle above 0. |
| C19 | **The price check fought the main lever.** Raising `shell_mp_per_kg` in S6 would make every gun card fail the check, or, with it off, the cheapest way to buy shells. | The check raises a card's cost to its load's price, with a warning. |

Still weaker than the ration, and accepted by the owner: a player who
earns more can buy more HE. The lever is the price.

**Critic pass 3 (2026-11-01, after D1–D3).** The questions: is any
"free" refill text left; is there a free or near-free route to shells
or rare rounds; can cheap small arms be abused; does the rare-round cap
hold for capture, recrewing and transfer; can the AI handle priced ammo
without stalling. The leftover "free" text is all history ("before D1
this was free") or owner quotes; no rule still says a refill is free.
Each hole below is closed by one data rule (§4.2, §4.10, tests 7c, 7d):

| # | Hole | Fix (lever) |
|---|---|---|
| C20 | **A new tank was cheaper than a refill.** Tank cards are exempt from the price check, so a full rack (Panzer IV about 850, Sherman about 990) cost more than the tank (640, 560). Running a tank dry and buying another beat refilling it. | A full rack refill costs at most `rack_refill_max_card_pct` (75 %) of the card, through a per-vehicle rack price factor. |
| C21 | **`rack_rearm_price_pct` broke D2.** Below 100 it priced only base rearm, so base was the cheapest place to rearm, the asymmetry D2 rejected (test 7c). | The rack price factor applies everywhere a rack fills: charged in base, credited forward to the player who paid for that stock. |
| C22 | **SPG HE escaped the price check.** The tank exemption also covered an SPG's or 105 mm tank's crate-only HE, so the card brought unpriced indirect HE (C5 again). | Crate-only rounds count in the price check and always pay the full shells price. |
| C23 | **Stacked sources.** Each source served every client in its radius, so a tank between an HQ, a dump and two trucks got four times the rate, rare rounds included. | One source per client per service tick, the fastest in reach. |
| C24 | **Rare rounds in the tow.** A tow or truck def with no `rare_cap` took the default 10; a Pak 40's tow could hold 10 PzGr 40 beside the gun's 2. | A gun and its own tow share the gun's cap. `rare_cap_max` is the default only for a def that lists a rare round; every other def holds 0. |
| C25 | **Main-gun rounds as small arms.** Kind is per ammo def. An autocannon or tank round filed as `small_arms` would cost a fifth. | Content check: a round fired by a vehicle main gun or a gun entity must be kind `shells`. |
| C26 | **AI stall.** The AI buys units whenever it can. With no manpower a base refill pauses, and an AI tank that "leaves when full" waited in base for ever. | `ai.ammo_float_mp` (100) kept back from unit buys while a unit waits to rearm; any AI unit leaves a source that served it nothing for `ai.rearm_stall_s` (15 s). |
| C27 | **Truck radius and stock disagreed.** `vehicles.md` gave trucks a 15 m radius and one `stock_kg`; `infantry-structures.md` gave 8 m and stock per kind. | Trucks serve men at 8 m and vehicles at 15 m (`vehicle_radius_m`); stock is kg per kind, like every source. |

**Checked and sound:**

- **Cheap small arms cannot buy shells.** Sources hold kg per kind
  (`infantry-structures.md` §6.3), so small-arms kg never feed a gun,
  mortar or tank. Small-arms crates are unqueued and cheap on purpose.
  The worst a player can do is fire more rifle and MG rounds, which is
  the intended result.
- **Capture.** Captured stock feeds only its own faction's calibres, and
  the enemy paid for it. A captured gun keeps its def's rare cap.
- **Recrewing.** A recrewed gun keeps its rounds and its cap. Nothing
  new is made.
- **Transfer.** Rack rounds never leave the rack, so rare rounds cannot
  be passed on to beat a cap. Squad sharing moves small arms only,
  within one squad.
- **Rare rounds are cheaper per round** than standard AP when they are
  lighter (HVAP about 8 kg). Accepted: the cap and the 30 s forward
  rate keep them rare, not the price.

**Owner-visible choice.** The 75 % ceiling makes a tank round cost about
4–6 manpower instead of about 10. The owner asked that tank ammo not be
"too cheap". A full refill of 420–480 still costs most of a tank. If it
feels too cheap, raise the ceiling toward 100 % or raise tank card
costs; do not lift it past 100 %, or empty tanks become disposable.
