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
timers, ration sizes) are starting values for tuning, not history.

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
     or map fire on known points at 3× scatter;
   - a per-player **shell ration** at base (roadmap Q4 B): every shell and
     bomb restocked anywhere draws from it;
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
   ration) and S11b (field howitzers on the map, light mortars, rifle
   grenades). S11a ships only if S5 met its HE-share target.

**What cannot work, said plainly**

| Idea | Why not | Best version that does |
|---|---|---|
| Range as the limiter | A leIG 18 (≈3.5 km) out-ranges a 2 × 2 km map; a 105 mm out-ranges a 4 × 4 km map. | Observation, ration, counter-battery (§4). |
| A historical battery (4–6 tubes, hundreds of rounds) | On a 1 km map it would erase every position. Pop and performance also suffer. | One gun per card, two artillery cards per player, one shell ration. |
| Field howitzers dropping shells steeply into trenches on 1 km maps | The leFH 18 (max +42°) and M3 (max +30°) cannot fire high-angle at all, and at 0.5–1 km they fire almost flat. History had them 3–8 km back, where shells fell more steeply. | Accept it. On small maps they are flat-fire and direct-fire guns, good against structures and vehicles. On Regiment maps they behave closer to history. The high-angle short-range job belongs to mortars and the leIG 18. |
| Counter-battery from pure physics | Locating a gun by sound is information, not contact. | A listener rule (§4.3). It gives knowledge only. Damage still comes only from real shells. |
| No abstract rule at all | Base resupply is free today, so a gun in base would fire forever. | One availability rule: the shell ration at base. It stands for the historical shell allocation. |
| Off-map batteries kept "for flavour" | Nothing can hit them back, and they would compete with the on-map cards. | Drop them in skirmish (§7). |
| Rolling barrages without friendly fire | Today no own round hurts own men, so a player can shell a trench while his men walk into it. | New owner question Q22: indirect HE hurts own men (A). |

---

## 1. What changes

| Topic | Roadmap until now | This design |
|---|---|---|
| Divisional howitzers (leFH 18, M2A1, M-30) | Off-map battery call, 48 rounds per match | On-map single-gun sections, S11b |
| Infantry guns (leIG 18, M3) | On-map, blocked by Q4 | On-map, S11a, with the shell ration |
| Range | Undecided (Q4: compress or ration) | Authentic; the ration is the brake |
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
| ZiS-3 crew | 76 mm ZiS-3 | ≈1,116 kg | ≈13,000 m | up to ≈+37° | Already in the AT list (dual role). Its indirect fire uses these rules. |
| Howitzer section | 122 mm M-30 | ≈2,450 kg | ≈11,800 m | up to ≈+63.5° | 21.8 kg OF-462 HE. Tow: Studebaker US6. The Soviet field howitzer card. |
| 120-mm mortar crew | 120 mm PM-38 | ≈280 kg on wheels | ≈5,700 m | mortar | Heavy mortar (already planned for S15). |
| BM-13 (Q23) | 16 × 132 mm M-13 rockets on a truck | — | ≈8,500 m | — | Same verdict as the Nebelwerfer: late, capped, lobby option. |

### 2.4 Rocket launchers: does the Nebelwerfer fit?

**Mechanically, yes.** It is an indirect weapon with a salvo. The shell
code, observer rule and ration all apply. It needs one small new thing:
a short powered phase in the projectile integrator (thrust for the burn
time, then ballistic). That is a small, deterministic addition.

**Its historical weakness is physical and fits our rules.** The rocket
trails and dust gave the launcher away at once. Crews fired and moved.
In our rules every launch is a flash event that anyone with line of
sight sees at long range (§4.3), on the first salvo.

**The balance problem is real.** One salvo is six 34 kg rockets: about
204 kg of shells in ten seconds. On a 1 km map that is an area-delete
button, and it pushes the HE share up.

**The ration makes it fair.** At the starting ration (60 kg per minute,
§4.2) one salvo costs about 3.4 minutes of the player's whole shell
supply, mortars included. The player chooses one big blow over minutes
of mortar fire.

**Recommendation:** not in S11. Offer it after S11b as a lobby
option (Q23 A), capped at one, with a 12:00 unlock, after the ration and
the HE share have been measured with tube artillery.

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
4. **Spread grows with range.** Dispersion is in mrad (and the range
   probable error grows with range), so a gun far back misses by more.
   That alone rewards moving forward, where the gun is more exposed. The
   trade is physical.

Mortars: the roadmap keeps today's 600 m mortar range through S5 (Q4 A).
With the ration in S11 they can go authentic too. Switch them in a
separate, measured step, not at the same time as the guns.

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
| **Observed** | A soldier with `observe` has line of sight to the target point, and a **link** to the gun | First round at `first_round_spread_pct` (250 %), tighter by `ranging_step_pct` (25 %) per round the observer sees land, down to `ranging_floor_pct` (40 %) | The normal way to use artillery. |
| **Map fire** | The target is a point in the player's knowledge map (last-seen enemies, seen trench lines and structures, capture points, sound or flash markers) | `map_fire_spread_pct` (300 %), no ranging-in | Harassing fire. No blind fire into unknown ground. |
| **Counter-battery** | Map fire on a sound or flash marker (§4.3) | Map-fire spread plus the marker's own error | A gun shelling a gun. |

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
| Gun section leader | 600 m | For his own gun only, by voice. Lets a gun fire at what it can almost see over a crest. |
| Squad leaders, mortar leaders, officers | 400 m | Voice link only, unless near a radio. |

Observing a ground point is a terrain line-of-sight ray plus the range
limit. It does not need the enemy men there to be spotted. That matches
history: observers adjusted on the bursts and on the ground.

**Registration.** Once a gun has ranged in on a point, it keeps that
ranging (per gun, per target cell) until it moves. A defender can
register the approaches to his trench before the attack, as real
defensive fire plans did. Displacing to dodge counter-battery throws the
registrations away: a real cost of moving.

### 4.2 Finite shells: the ration and the supply chain

**Where shells live** (from `guns-artillery.md` §4.1): the gun's ready
rack and reserve, the crew's kit, the tow, crates, the HQ stock. Every
shell is counted and moved by a man or a vehicle.

**Starting loads** (priced in the card):

| Gun | At the gun | In the tow |
|---|---|---|
| leIG 18 | 12 Igr 18 HE, 2 Igr 38 HL; crew carry 4 more | 30 (tow variant) |
| 105 mm M3 | 10 M1 HE, 2 M67 HEAT | 24 |
| leFH 18 | 12 HE, 2 HEAT (Gr 39 rot HL), 1 Pzgr rot | 24 |
| 105 mm M2A1 | 12 M1 HE, 2 M67 HEAT | 24 |
| Nebelwerfer 41 | 6 loaded | 6 |

**Resupply** uses the S5 and S6 system with no new transport code:

- Shell crates (one class each: "7.5 cm IG ×6", "10.5 cm ×4", "105 mm ×4")
  are carried by men (Munitionsträger, Ammunition Bearers), dropped by
  trucks (S8) or drawn from the HQ stock.
- The HQ fills only by crates carried by the player's men and later by
  trucks (owner decision Q2 C). No trickle into the HQ.
- The tow that brought the gun is its natural ammo carrier and a soft
  target the enemy will hunt.
- A 105 mm projectile weighs about 15 kg; with its case and charge the
  complete round is about 18–20 kg (verify). A man carries about two.
  Heavy guns therefore depend on trucks. That is historical and gives trucks a
  second job.

**The shell ration (roadmap Q4 B).** Base resupply is free today. A gun
parked in base would refill forever. So, from S11:

- Each player has a **shell stock at base**, in kg. It grows at
  `shell_ration_kg_per_min` (start 60) up to `shell_stock_cap_kg` (start
  600). It starts at `shell_stock_start_kg` (start 150).
- **Every shell and mortar bomb taken from the base draws from it**: a
  gun or mortar refilling in the base zone, a crate of shells issued at
  base, a truck loading shells. Small arms, grenades and AT rockets do
  not.
- The HQ, dumps and crates hold only what was carried out of the base,
  so everything traces back to the ration. One choke point.
- The HUD shows "Shells 340 / 600 kg".

What 60 kg per minute buys, for the whole player: about 17 mortar bombs
(3.5 kg), or 10 leIG shells (6 kg), or 4 rounds of 105 mm (15 kg), or a
third of a Nebelwerfer salvo. **A second gun does not add HE. It shares
the same pot.** That is the key property for the HE-share problem (§6.2).

Why a ration and not a manpower price per shell (Q4 C): it caps HE
tonnage per minute directly, whatever the player's income. History
supports it: shells were allocated per gun per day, and both sides
rationed them in 1944 (the German supply crisis in Normandy, the US
ammunition shortage of autumn 1944; verify). It is the one abstract rule
here, and it lives at the base, where units and supplies already appear.
If the owner prefers manpower per shell, (C) works too; it is weaker
against HE spam because a rich player can buy more.

**Guns in their own base get no free crew.** Today vehicles in base
regain crew (`base_crew_s`). Gun crews are soldiers; lost men are
replaced only by buying replacements (S12). Otherwise a gun in base would
be immortal.

### 4.3 Counter-battery: sound and flash

**Sound.** Every shot from an indirect-capable gun is an acoustic event.

- The enemy notes shots from the same spot (within `cb_reset_m`, 75 m)
  inside a window (`cb_window_s`, 120 s).
- After `cb_rounds_to_mark` shots (3 for guns, 5 for mortars), and after
  `cb_delay_s` (15 s), the enemy's knowledge map gets a **battery
  marker**.
- The marker sits at the true position plus an error of `cb_error_pct`
  (15 %) of the distance from the gun to the nearest enemy soldier (the
  listener), drawn from the counter RNG.
- Each further shot multiplies the error by `cb_error_step_pct` (80 %),
  down to `cb_error_floor_pct` (5 %).
- Example: a gun 800 m from the nearest enemy. First marker ±120 m.
  After five more rounds about ±40 m (the 5 % floor). A 105 mm burst kills men within roughly
  10–20 m, so counter-battery needs several rounds, and each round costs
  the ration.

**Flash.** Any enemy soldier with a clear line of sight to the muzzle
within `flash_m` (2,000 m in daylight; dust, smoke and flash) **spots the
gun** for `flash_reveal_ms` (6 s), as `fire_reveal_pct` does today. This
is exact and immediate. A gun behind a crest (reverse slope, firing high
angle or over the crest) is not seen. Rocket launches count as flash for
everyone with line of sight, on the first salvo.

**The enemy strikes back** with real shells. Counter-battery fire is map
fire on the marker. Shells land with real scatter; fragments must reach
the crew. Infantry can also walk to a marked gun. Nothing silences a gun
except contact.

**Displacement.** Pack, move more than 75 m, emplace. The marker on the
enemy side stays where it was, now pointing at empty ground. The gun
loses its registrations. With §4.4's times, a move costs 1–4 minutes.

**What the player sees.** Their own count only: "6 rounds from this
spot: likely located". No view of the enemy's marker (it would leak
knowledge). Owner question Q24 keeps the option to show it.

**Honest notes.**
- Real sound ranging was done by specialist units with microphone bases,
  and took longer. We let any soldier be a listener, and we compress the
  time. It is information, not damage, so the physical-contact rule
  holds.
- Counter-battery duels could crowd out the fight. The ration limits
  them: a round at a gun is a round not fired at men. We report the share
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
apex about 20 m up and a flight of about 6 s.

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

**Danger close** (own men within 50 m of the target): the AI never fires;
a human gets a HUD warning. Whether own shells hurt own men is Q22
(§6.3).

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
- **`limit_group = "artillery"`: 2 per player** on Battalion, 3 on
  Regiment (in `battle_tiers.toml`). Infantry guns, howitzers and
  launchers count; mortars do not (they have their own caps).
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
   at known points, at 3× scatter, with no ranging-in.
2. **The observer must walk.** A VB-Trupp walks to a crest with a view of
   the target. Two men, easy to kill. Kill the radio man and the link is
   gone.
3. **Distance costs accuracy.** Spread is in mrad: at 1 km it misses by
   twice as much as at 500 m.
4. **The ration.** 60 kg per minute is 10 leIG shells for the whole
   player, mortars included. A minute of full fire empties a minute of
   supply.
5. **It is found.** After 3 rounds the enemy has a marker on the base.
   Every enemy gun and mortar can reach it. The base zone is not a safe
   zone.
6. **It cannot hide easily.** A base on open ground is seen by flash on
   the first round.

The base is legal but poor. Forward positions are more accurate and more
exposed. **If tests still show base camping**, one rule switch exists:
`indirect_from_base = false` (a gun emplaced in its own base zone fires
direct only). It is off by default because it is a placement rule, not
physics.

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
last-ditch tank stopper, not an AT gun.

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
   from the post-S3b baseline).
2. **The ration caps HE tonnage per player per minute.** Guns share the
   pot with mortars. Buying a howitzer moves HE between weapons; it does
   not add HE.
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
| Matches where the ration ran dry for a side | reported |

**If HE is still too high, the levers in order:** the ration size, then
map-fire spread and the observer ranges, then the fire-direction delay,
then caps, cost and unlock times. Shell lethality stays authentic.

### 6.3 Friendly fire

S1 decided "no friendly fire": own rounds and blasts pass through own
men. With artillery this becomes an exploit. A player can shell an enemy
trench while his own men walk into it, and they are immune. Real armies
lifted fire because their own shells killed their own men.

New owner question **Q22**. (A) Fragments and blast from **indirect fire**
(mortars, guns, launchers) hurt own men; direct fire stays as today.
This is physical and historical, and it makes timing the lift a skill.
(B) Keep it off; the AI keeps `ai.lift_fire_m` and `danger_close_m`;
humans may exploit it.

### 6.4 The AI

S3 showed that features the AI ignores never get balanced. The AI needs
these rules (weights in data):

| Behaviour | Rule |
|---|---|
| **Buy** | Tag `artillery`. Buy a gun only with a living observer (or buy an FO team first). Buy when enemy trench length or structures seen pass a threshold, or an enemy gun has been marked. Keep a ration reserve (`ai.shell_reserve_kg`). |
| **Site** | Score spots 300–700 m behind the own front (or in base on small maps): no enemy line of sight (the S6 reverse-slope probe), flat firm ground, reachable by the tow, the mask check clears to the three most likely targets, at least 50 m from other own guns. Ask idle engineers for a `howitzer_pit`. |
| **Observe** | The FO walks to a crest point with line of sight onto the enemy trench or approach (a few sampled points, ray-tested), goes prone, stays out of the open. |
| **Fire** | (a) Preparation fire on a trench timed with an assault: starts about 60 s before, lifts when the assault is 100 m out (or 50 m with Q22 B). (b) Defensive fire on registered approaches when attackers enter them. (c) Counter-battery on a marker when its error is under about 60 m and the ration is above the reserve. (d) Direct fire (infantry guns) at spotted MG nests, bunkers and vehicles within 800 m, only with an escort near. |
| **Relocate** | After `ai.displace_after_rounds` (6) from one spot by its own count, or when enemy HE lands within 30 m. It uses only its own knowledge, never the enemy's marker. |
| **Resupply** | Below 50 % rounds, send the tow or bearers to the HQ or base for crates. |
| **Recrew / capture** | As S9: idle infantry within 100 m serve a neutral gun with rounds. |

Cost: a few hundred rays per siting decision, every 10 s at most, inside
the AI's 20 % ray share; the mask check once per order.

### 6.5 Performance

Battalion tier: 700 soldiers, 100 vehicles, 1,800-projectile pool,
2,400 rays per tick, `MAX_FRAG_RAYS_PER_TICK` 4,096 (S3b), 30 Hz.

| Item | Worst case (Battalion, 10 players) | Against the budget |
|---|---|---|
| Guns | 2 per player: 20 guns, plus mortars | Gun entities are vehicle-style; well under 100 vehicles with tanks capped. |
| Shell rate, steady | The ration caps it: 10 × 60 kg/min = 600 kg/min, about 40 rounds of 105 mm per minute, or ≈170 mortar bombs | ≈0.7–3 bursts per second for the whole match |
| Shells in flight | Burst rate × flight time (5–30 s): under 100 | ≈5 % of the 1,800 pool. Shells are never bundled (§2.2). Guard: `max_indirect_in_flight_per_player` (24) delays the next shot, deterministically, if ever hit. |
| Fragment rays | A synchronised strike by 12 guns in one tick: 12 × 160 = 1,920 | Under 4,096. S3b scales `frag_count` down over the cap; S11 reports `frag_rays_capped` and expects 0 in AI matches. |
| Craters | One terrain op per burst; bigger craters touch more cells | Terrain ops are budgeted per tick; craters queue in burst order, never drop. Detail-tile inits stay at most 4 per tick. Test: the "200-shell barrage plus 20 squads digging" target (§2.2). |
| Mask checks | ≈20 segments × a few charge/elevation candidates per order | Per order, not per shell. |
| Sound ranging | One small record per gun per spot; one listener distance query per shot | Negligible. |

Regiment (2,000 / 300 / 5,000; 7,500 rays per tick) scales the same way
with three guns per player.

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
ready = [{ id = "ww2:75mm_igr18", count = 12 }, { id = "ww2:75mm_igr38hl", count = 2 }]
fetch_s = 4

[indirect]
charges_m_s = [92, 110, 135, 165, 210]   # Igr 18, charges 1-5 (verify)
first_round_spread_pct = 250
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
map_fire_spread_pct = 300
observe_m = { fo = 1200, gun_leader = 600, leader = 400 }
cb_rounds_to_mark = { gun = 3, mortar = 5 }
cb_window_s = 120
cb_delay_s = 15
cb_error_pct = 15
cb_error_step_pct = 80
cb_error_floor_pct = 5
cb_reset_m = 75
flash_reveal_ms = 6000
danger_close_m = 50
indirect_from_base = true           # switch off only if tests show base camping
friendly_fire_indirect = true       # Q22 (A)
max_indirect_in_flight_per_player = 24

[supply]
shell_ration_kg_per_min = 60        # Q4 (B)
shell_stock_start_kg = 150
shell_stock_cap_kg = 600

[limits]
artillery = 2                       # Regiment: 3, in battle_tiers.toml
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
| **S11a** Indirect fire and infantry guns | Shell ration; observer links by radio and voice; fire modes; registration; mask check; fire-direction delay; sound and flash ranging; displacement; `howitzer_pit`; crater scaling; gun direct fire with HE and HEAT; AI artillery planner; Q22 friendly fire. Content: VB-Trupp, FO Party, leIG 18 (pushed and towed), 105 mm M3. | ~0.9× | S5, S6, S8, S9, S3b; Q4 (B or C) |
| **S11b** Field howitzers on the map | leFH 18 and M2A1 sections (data plus tuning); 75 mm pack howitzer (data); kz. GrW 42, 60 mm M2; M7 rifle grenade and *Schießbecher*. | ~0.4× | S11a, measured |
| Later | sIG 33; Nebelwerfer 41 if Q23 (A) | data, a small rocket-thrust addition, two models | S11b, measured |
| S15 | 76 mm M1943, M-30 section, (BM-13 if Q23) | data | S11b |

---

## 10. Tests (`frontline-cli gun-test` and AI matches)

1. **Observer rule.** No linked observer: an observed mission is refused.
   Map fire only on knowledge points; mean miss ≈ 3× observed.
2. **Radio link.** Kill the FO's radio man: the link breaks until a man
   picks up the set. Destroy the HQ radio box: observers near it lose
   that relay.
3. **Masks.** A leFH 18 and an M3 told to hit a trench 80 m behind a
   crest from 600 m report "masked". A leIG 18 at high angle hits it.
4. **Registration.** Second mission on a registered point starts at the
   floor spread; after the gun moves 75 m it starts at full spread.
5. **Sound ranging.** A gun firing from one spot gets an enemy marker
   after 3 rounds, error ≤ 15 % of the listener distance, shrinking each
   round. A gun that displaces after 4 rounds survives counter-battery
   more often than one that stays (10 runs each).
6. **Flash.** An enemy with line of sight spots the gun on its first
   round; a reverse-slope gun is not spotted by flash.
7. **Base camping.** A gun in base with a forward FO fires only what the
   ration allows; with the ration empty it stops. AI matches still end
   with a winner inside 25 min (stall guard).
8. **Direct fire.** A leIG 18 at 400 m silences an MG nest; a 105 mm burst
   breaks a dugout roof that survives an 81 mm hit; M67 HEAT penetrates a
   Panzer IV side; 105 mm HE on a Sherman damages only external parts and
   tracks.
9. **Trenches.** fortify-test row "105 mm barrage": trench losses ≤ 40 %
   of open; lip loss per 105 mm burst within 2 m ≥ 0.15 m; a preparation
   barrage cuts attacker losses to at most half the no-support baseline
   (owner sets the bar).
10. **Friendly fire** (if Q22 A). Own men inside the burst take
    fragments; the AI lifts fire in time in ≥ 9 of 10 staged assaults.
11. **HE share.** §6.2 targets over 20 AI matches.
12. **AI use.** The AI buys a gun and an observer in ≥ 6 of 10 matches,
    fires observed or counter-battery missions, never fires indirect
    without an observer except on knowledge points, and displaces after
    counter-fire.
13. **Performance.** Stress scene: 20 guns and 30 mortars firing at the
    ration ceiling on Battalion: `frag_rays_capped` = 0 in AI matches;
    stress p99 inside S3b T13; the 200-shell barrage target holds.
14. **Determinism.** Save and load with shells in flight, a gun mid-pack
    and a marker sharpening; hashes equal at 1, 2, 3 and 8 threads and in
    net loopback.

---

## 11. Owner questions

Also listed in `roster-expansion.md` §5. Work goes on with (A).

- **Q13 (changed). Divisional artillery** [S11]. (A) On-map single-gun
  sections, off-map dropped from skirmish (this file). (B) Off-map
  batteries called by an observer (the old default).
- **Q4 (needs an answer before S11a).** This file recommends (B): the
  per-player shell ration at base. (C), manpower per restocked shell, also
  works but caps HE less firmly.
- **Q22. Friendly fire from indirect HE** [S11a]. (A) Fragments and
  blast from mortars, guns and launchers hurt own men; direct fire stays
  as today. (B) Keep no friendly fire; AI-only danger close.
- **Q23. Rocket launchers** (Nebelwerfer 41, later BM-13) [after S11b]. (A)
  Later, as a lobby option, cap 1, 12:00 unlock, after the ration is
  measured with tube artillery. (B) Missions only. (C) In S11b.
- **Q24. Show the player the enemy's marker on his gun?** [S11a]. (A) No:
  only his own count ("6 rounds from this spot: likely located"). (B)
  Show the enemy marker (leaks enemy knowledge).
- **Q25. Field howitzers on small maps** [S11b]. (A) Allowed on every
  map, 1 per player, 8:00 unlock; on 1 km maps they act as flat-fire and
  direct-fire guns. (B) Only on maps of 2 × 2 km and larger (a map flag),
  where they behave closer to history.
