# Roster area: crew-served guns and artillery

Design proposal for Frontline RTS, written 2026-11-01. It covers mortars, anti-tank guns,
infantry guns, howitzers, the gun crew model, indirect fire and the ammunition these weapons
need. The HQ and supply follow-up is covered from the guns' side: how shells and bombs reach
a gun. Another area covers the HQ structure itself.

> **Note (2026-11-01): artillery is now on the map.** The owner prefers
> howitzers and other artillery as on-map units. `artillery-on-map.md`
> (same folder) holds that design and replaces this file's off-map
> default: §0 point 3, P2, §3.9, the off-map battery def in §7.4, slice
> G5 and question 1. Divisional howitzers (leFH 18, M2A1, M-30) become
> single-gun on-map sections; off-map batteries are dropped from
> skirmish. It extends §3.8's counter-battery to every gun.
>
> **Owner decisions (2026-11-01).** Question 2 (supply lever): no
> per-player throughput or ration. Each gun and mortar has a ready
> inventory that refills only from crates nearby, an HQ or dump radius,
> or its tow. The base zone does not refill shells or mortar bombs.
> Shells crates cost manpower when taken at base (`shell_mp_per_kg`).
> Question 4: hint only, never the enemy's marker. Friendly fire from
> indirect HE is on. Details: `artillery-on-map.md` §4.2, §4.10; roadmap
> "Owner decisions". The gun entity, crews, mobility, AT guns, mortars
> and ammunition parts of this file still stand.
>
> **Owner decisions D1–D3 (2026-11-01, later).** D1: no free ammo
> anywhere, the base included; small arms 0.2, explosives 0.5, shells
> 1.0 manpower per kg, paid once when the kg leaves the base. AT guns
> and their tows now pay per round in base like tanks. D2: distance
> costs delivery, not a surcharge (`artillery-on-map.md` §4.11). D3:
> rare rounds (APCR, HVAP, HEAT) refill at any source, slowly (1 per
> 10 s in base, 1 per 30 s forward), up to a per-weapon cap of at most
> 10.

Read against: `SYSTEM_DESIGN.md` §2.3, §2.4, §5.7, §5.9, §5.10, §5.12; `docs/slices/S1-skirmish.md`,
`S1b-abilities.md`, `S3-armour.md`, `S3-log.md`; `docs/TODO.md`; `sdk/crates/content/src/defs.rs`;
`mods/ww2/defs/*`; `crates/sim/src/{combat,gunnery,vehicle,armour,damage,items,ai}.rs`.

**About the numbers.** The weights, ranges, rates and penetration figures below are commonly
published values. Use them as starting data. They are not checked citations: each def still
needs its `sources = [...]` entry, an owner / M2 task that is already open for S3. Values I am
less sure of are marked *(verify)*.

---

## 0. Summary

**Recommendations, in priority order**

1. **Fix mortars before adding any more HE.** Mortars already cause 2–4× the kills of small
   arms; HE is 43 % of all kills in the S3 AI matches. The causes are structural, so the fixes
   are structural, not stat cuts:
   - give mortars finite bombs, carried by their crew;
   - fire only on observed targets, or map-fire with wide scatter;
   - range in on a target over several rounds;
   - let enemies locate a firing mortar by sound, which forces it to move.

   This makes slice G0 (§10) a prerequisite for every other gun, and it is also the first
   consumer of the HQ, ammo-crate and supply work.
2. **A gun is an entity built from the vehicle code; its crew are ordinary soldiers.** The
   gun's carriage, shield, plates and components reuse `armour.rs`, `gunnery.rs` and the
   vehicle box model. The crew are *soldiers standing in service slots* around the gun, not
   seats inside it. That is the only way to honour "damage only from physical contact": gun
   crews stood in the open behind a 4–8 mm shield, and bullets and fragments have to be able to
   reach them.
3. **Divisional howitzers go off-map by default.** The leFH 18, the 105 mm M2A1 and the 122 mm
   M-30 become a data-driven `SupportCall` that needs a forward observer. Their shells are still
   real projectiles that enter at the map edge on a true trajectory and kill only by fragments.
   On-map, the "howitzer" slot goes to the *regimental* guns that historically stood near the
   front:
   - Germany: 7.5 cm leIG 18 and 15 cm sIG 33;
   - USA: 105 mm M3 of the Cannon Company, and the 75 mm Pack Howitzer M1A1 with airborne units;
   - USSR: 76 mm regimental gun M1943.
4. **Heavy guns need a prime mover; the "walk to the front" becomes a drive.** A Pak 40
   (1.4 t) pushed 1 km by its crew takes about 40 minutes. Guns heavier than about 700 kg are
   called in hitched to their historical tow, so the AT-gun slice depends on the
   truck/halftrack slice, or must bring a minimal truck with it.
5. **AT guns are where this area pays off most.** They are a cheap, low, concealed and fragile
   counter to tanks. Today the Sherman wins tank fights and AT teams are weak, so an AT gun that
   lands the first shot from concealment fills a real gap.

**Pushback: where the request or an obvious approach goes wrong (details in §2)**

| # | Idea | Problem | Better version |
|---|---|---|---|
| P1 | "5 cm leGrW 36 as the German light mortar" | Largely withdrawn in 1943 as ineffective. The Soviet 50 mm company mortars went the same way. | Germany: **8 cm kz. GrW 42** (short 81 mm). USSR: no light mortar in 1944 line infantry. The US **60 mm M2** is genuinely current. |
| P2 | Divisional howitzers (leFH 18, M2A1, M-30) bought and placed on the map | They fired from 3–10 km behind the line. On a 1–3 km map they either sit in base and reach everything (mortar dominance again) or stand in rifle range (unhistorical). | Off-map battery `SupportCall` with an observer and a shell allocation. On-map regimental guns fill the howitzer slot. On-map divisional guns only in missions or as a mode option. |
| P3 | "The gun is a light vehicle with crew seats" | Seats are boxes inside a hull that small arms never damage (S3 rule). An open gun crew would become immune to bullets. | Gun = vehicle-style entity; crew = soldier entities in service slots. |
| P4 | Authentic mortar ranges (2.4–6 km) on a 1 km map | Every mortar in base reaches the whole map, and today's base resupply is unlimited. | Authentic ranges are fine **only with** the observer rule, finite bombs, and bombs that leave the base only in priced crates (§4.3, owner decisions Q4 and D1). Otherwise keep today's compressed 600 m. |
| P5 | 4.2 inch mortar as a US infantry heavy mortar | It belonged to separate Chemical Mortar Battalions, attached to divisions. Its main job was white phosphorus (WP) smoke and HE. | Offer it as an attached support unit (expensive, limited) or as an off-map smoke and HE fire mission. The US had no 120 mm. |
| P6 | 3 inch M5 as a mobile AT gun | It weighed 2.2–2.6 t and fared badly in the Ardennes; towed TD battalions were converted to self-propelled guns from late 1944. | Keep it, priced as an emplaced defensive gun that cannot be pushed, only re-laid. Mobile US AT belongs to the M10, M18 and M36 (vehicle area). |
| P7 | 57 mm M1 with HE from June 1944 | US 57 mm crews had little or no HE in Normandy; HE came late in 1944 *(verify date)*. | Use the existing date filter: give the HE ammo def an `available_from`. An authentic early-war weakness costs nothing to model. |
| P8 | Mortar squads of 3 men | Historical squads were 5–7 (gunner, assistant, leader, ammo bearers). The bearers *are* the ammunition supply. | Squads of 5: the extra men carry bombs. When they die, the gun runs dry sooner, which is a physical, visible weakness. |
| P9 | Light and medium mortars as gun entities | They were man-portable in three loads (tube, bipod, baseplate). An entity adds towing, slots and capture for no gain. | Light and medium mortars stay a squad weapon, as today, with a carried-parts handover like the launcher's. Heavy mortars (280 kg, wheeled carriage) are gun entities. |

---

## 1. What exists today (facts this design builds on)

- **Mortars.** `grw34` and `m1_mortar` have identical stats: 60–600 m, 15 rpm, 50 mrad spread,
  150 damage, 3 m lethal radius, 8 m damage radius, 15 m suppression radius.
  - The team is 3 men. The weapon belongs to the formation leader, and the shell leaves from his
    position.
  - **Ammo is unlimited.** `Order::Barrage { target, rounds }` fires N rounds at *any* ground
    point. The player needs no sight of it; the AI uses only spotted groups.
  - Flight time is a fudge (`sqrt(range · k)`), not a charge and elevation solution.
  - Spread does not tighten over a barrage, and there is no ranging.
- **HE.** `combat::burst` deals radial damage with linear falloff. A man is protected when
  terrain lies between the blast point (+0.3 m) and his body point. It suppresses to a radius
  and carves a crater. It is gated by line of sight, but it is **not** fragment rays. §5.7
  specifies 32 weighted fragment rays; that is not built.
- **Vehicles (S3).** Hulls and turrets are oriented boxes with per-face plates (`thickness_mm`,
  `slope_deg`).
  - Components are boxes or anchors. Crew are **seats with boxes**, not soldiers.
  - The pen rule is `pen × cos ≥ thickness`. Gunnery covers laying, lead, ranging-in (−20 % per
    shot, floor 50 %) and auto AP/HE.
  - Small-arms rounds stop on vehicle boxes and do no damage.
  - Blasts near a vehicle damage only the tracks (`vs_vehicle_pct`).
  - "Towed and static AT guns … need towing, pushing and gun shields: a slice of their own" is
    explicitly out of S3.
  - So are re-crewing and capturing vehicles.
- **Launchers (S3)** are the nearest thing to a crew-served weapon: one per squad, carried by
  one man, with a `rounds` count, a `handover` when the carrier dies, and base resupply of one
  per 20 s.
- **Resupply today.** Items: one per man per 20 s inside the own base. Rockets: the same.
  Vehicles in base: one round per 2 s, one crew seat per 20 s, 2 % of every component's HP per
  second.
- **Spotting.** Soldiers see about 150 m and tanks about 175 m. Vehicles are seen at
  `spot_size_pct` × the observer's sight (Panzer IV 200 %). Firing reveals the shooter
  (`fire_reveal_pct`).
- **Map.** Kessel Ridge, 1024 × 1024 m. Battalion maps go up to 3 × 3 km (§2.2). Woods are a
  flag layer (they cut spotting, not bullets) with no canopy height.
- **Dig profiles.** Linear only (`trench`, `at_ditch`). Pits, blueprints and pieces (§5.10.3)
  are not built.
- **Known issues this area touches:**
  - mortar dominance;
  - "assaulting a finished trench barely works";
  - weak AT teams;
  - the Sherman wins tank fights;
  - the open owner question 2 on vehicle spotting range.

---

## 2. Pushback in detail

**P1: light mortars.** The 5 cm leGrW 36 (14 kg, about 50–520 m, 0.9 kg bomb) was judged too
weak. Production ended in 1941 and it was withdrawn from front-line rifle companies around
1943. It turns up in second-line and fortress units in 1944. The Soviet 50 mm RM-40/41 went the
same way.

The German 1944 answer is the **8 cm kurzer Granatwerfer 42** ("Stummelwerfer"): about 26 kg,
about 1,100 m, firing the standard 3.5 kg 8 cm bomb. It was designed for paratroops and spread
into Grenadier companies.

So the authentic asymmetry is:
- **USA:** 60 mm M2 at company level plus 81 mm at battalion. The best-equipped light mortar
  user.
- **Germany:** kz. GrW 42 and s.GrW 34 (8 cm), plus 12 cm GrW 42 at regiment.
- **USSR:** 82 mm at battalion and 120 mm at regiment. These are the heaviest and most numerous
  mortars of the three, and the 82 mm can fire captured German 8 cm bombs (see §4.4).

Offer the leGrW 36 only as a cheap option in a "second-line" sub-roster, if at all.

**P2: on-map divisional howitzers.** Covered in §3.9. The short version:
- A 105 mm with an 11 km range on a 1 km map is an off-board weapon, wherever you place its
  model.
- Putting it on the map adds towing, crew, counter-battery and capture work. All of that is
  needed anyway for AT and infantry guns, which actually fought within sight of the enemy.
- It also adds the worst balance risk in the game: a second mortar dominance with 15 kg shells.

**P3: the crew model.** Spelled out in §3.2.

**P4: mortar ranges.** The design favours authentic stats. Historically, mortar range was
limited by **observation and ammunition**, not by the tube. If we model those two limits, the
authentic tube ranges stop being a problem. If we do not, the compressed 600 m in today's data
is the only thing holding mortars back, and it is a silent unhistorical stat change.
Recommendation:
- ship G0 (observer rule plus finite bombs plus priced bomb crates);
- *then* raise the ranges to authentic values;
- watch the kill share in `skirmish --ai-vs-ai` before and after.

**P5–P7.** Covered in the table in §0. All three use authentic history to set *availability and
cost*, not to change stats. That fits "add historical options before changing stats".

**Also: the 15 cm sIG 33 and 8.8 cm guns.** Both are tempting and both are dangerous to balance.
- The sIG 33 (1.75 t, 38 kg shell) is the best trench and house buster of the period.
- The 8.8 cm Pak 43 (4.4 t) and FlaK 18/36 kill any Allied tank at any range on the map.

Both fit the "heavy, rare, needs a tow, can't be pushed" pattern. Put them in a second wave,
behind a strict per-type limit and a long cooldown, never in the first gun slice.

---

## 3. Core model

### 3.1 The gun entity

A **gun** is a new entity kind built from the vehicle data and code:

- **Defined** in `defs/vehicles/*.toml` with `role = "at_gun" | "infantry_gun" | "howitzer" |
  "heavy_mortar"`, `crew_model = "served"` and `mobility.type = "towed"`. It reuses the vehicle
  def parser, `armour.rs` plates, components, `gunnery.rs` laying and leading, and wrecks.
- **Geometry:**
  - a **carriage box** (cradle, trails, wheels);
  - a **shield box** whose front face is a thin plate;
  - a small **barrel anchor**.
  
  No turret. The gun traverses inside a limited arc on the carriage (Pak 40 about ±32°, leIG 18
  about ±6°). Beyond the arc, the crew **re-lays the trail** (`relay_trail_s`): §5.12.2 already
  describes this for direct control.
- **Components:**

  | Component | Kind | Effect |
  |---|---|---|
  | `gun` | breech box | Destroyed: cannot fire. |
  | `barrel` | anchor | Damaged: spread ×2. |
  | `sight` | anchor | Damaged: spread ×1.5, no ranging-in. Destroyed: direct fire only at ×2.5 spread. |
  | `recoil` | box under the barrel | Destroyed: cannot fire. Field repair possible. |
  | `wheels` | two anchors | Destroyed: cannot be pushed or hitched; can still fire. |
  | `ammo` | the ready rounds, a box | Hit by HE or AP: 20 % chance to detonate. |

  Guns have no engine, tracks, turret ring or fuel. Fragment hits on the sight and recoil
  system were the usual way guns were disabled, so they get anchors that blasts can reach
  (`vs_gun_pct` on HE defs, like `vs_vehicle_pct`).
- **Ownership is mutable** (capture, §3.6). This is new: S3 vehicles never change owner. It
  touches hashing, the snapshot and AI bookkeeping.

### 3.2 Crew: soldiers in service slots

**Why not seats.** S3 made tank crews seats inside the hull, which was the right call for a
closed tank. A gun crew kneels in the open behind a 4–8 mm shield:
- MG fire from a flank kills them;
- a mortar bomb 10 m away wounds them;
- a sniper picks off the gunner over the shield.

All of that has to happen by physical contact. Bullets must therefore meet *soldier capsules*,
and S3 seats never take small-arms damage. Crew as soldiers also get recrew, capture, bail-out
("abandon gun") and walking away for free.

**How it works.**
- The gun squad is an ordinary soldier squad (`role = "gun"`, `gun = "ww2:ger_pak40"`).
- Its men carry personal weapons (rifles, an SMG for the leader, a pistol for the gunner) and
  some carry rounds (§4).
- The gun def lists **service slots**: positions in the gun's frame, each with a role, a pose
  and an order of priority.
- When the gun is emplaced, the squad's men walk to the slots and take the pose. Their capsules
  sit there, so the shield covers the gunner and loader from the front only.

| Slot role | Typical position (Pak 40) | Missing → |
|---|---|---|
| `gunner` (Richtschütze) | left of the breech, behind the shield, kneeling | Cannot fire until another man moves in (`slot_swap_s`, about 3 s). |
| `loader` (Ladeschütze) | right of the breech, kneeling | Reload ×2: the gunner loads. |
| `commander` (Geschützführer) | 3–5 m behind or beside, kneeling, binoculars | No ranging-in (each shot starts at full spread); sight against vehicles ×0.8. |
| `ammo` ×1–3 | 4–6 m behind, prone or in the pit | Rounds beyond the gun's ready rack take `fetch_s` each, fetched by the loader. |

- **Auto-swap** follows S3 (lowest-priority man moves up): gunner first, then loader.
- **Suppression works through the men.** A pinned gunner neither lays nor fires. A pinned loader
  stops loading. MG fire on a gun position therefore silences it, as it did historically, and
  under the "near-misses only" rule.
- **Untrained crews.** Any infantry can serve a gun. Squads without the `gun_crew` capability
  get `untrained_reload_pct` (150) and `untrained_spread_pct` (150).

**Mortars (light and medium) stay as today: a squad weapon.** Change one thing: the tube moves
with the formation leader, but its **parts** (tube, bipod, baseplate) are carried by named men.
If the baseplate man dies, the next man picks it up after `handover_ms`, reusing the launcher
handover code. The ammo bearers' bomb counts are their personal kit (§4.1).

### 3.3 Mobility: emplaced, limbered, pushed, hitched

| State | Can fire | Can move | Transition (starting values, compressed realism) |
|---|---|---|---|
| **Emplaced** | yes, within its arc; re-lays the trail beyond it | no | `emplace_s`: Pak 40 30 s, 57 mm 25 s, 45 mm 15 s, leIG 18 20 s, 105 mm M3 40 s, 12 cm GrW 42 45 s. |
| **Limbered** (trails closed) | no | pushed | `pack_s`: about 60 % of `emplace_s`. |
| **Pushed** | no | yes, at `push_cm_s` × (men pushing ÷ `push_men`), and only on slopes up to `push_max_slope_pct` | Instant from limbered. Pushing men are standing, exposed and slow. |
| **Hitched** | no | towed by the vehicle | `hitch_s` (15–30 s). The gun follows the tow's path on the wheeled nav layer. |

**Push speeds** (full crew, flat firm ground; starting values):

| Gun | Mass | `push_cm_s` | 1 km takes |
|---|---|---|---|
| 7.5 cm leIG 18 | 400 kg | 110 | 15 min |
| 45 mm M-42 | 625 kg | 100 | 17 min |
| 75 mm Pack How. M1A1 | 653 kg | 90 | 19 min |
| 5 cm Pak 38 | 1,000 kg | 60 | 28 min |
| ZiS-3 | 1,116 kg | 50 | 33 min |
| 57 mm M1 | 1,230 kg | 50 | 33 min |
| 7.5 cm Pak 40 | 1,425 kg | 35 | 48 min |
| 105 mm M3 | 1,130 kg | 45 | 37 min |
| 3 inch M5 | 2,210+ kg | 0 (re-lay only) | needs a tow |
| 12 cm GrW 42 / PM-38 | 280 kg on wheels | 80 | 21 min |

**Consequence (honest).** With "spawn at base and walk", every gun except the lightest needs a
**prime mover** to be useful on a 1 km map. Historical tows:

| Faction | Tow | Pulls |
|---|---|---|
| Germany | Sd.Kfz. 11 (3 t halftrack) | Pak 40, leFH 18 |
| Germany | Raupenschlepper Ost | Pak 40 in the East |
| Germany | Kettenkrad | leIG 18 and other light loads |
| Germany | Opel Blitz truck | various |
| USA | M3 halftrack, or 1½-ton Dodge WC-63 6×6 | 57 mm |
| USA | M5 high-speed tractor | 3 inch, M2A1 |
| USA | 2½-ton CCKW | 105 mm M3, M2A1 |
| USSR | Studebaker US6 (lend-lease) | ZiS-3 |
| USSR | GAZ-AA, or horses | 45 mm |

Horse teams were in fact the norm in German infantry divisions. Horses are out of scope; a mod
can add them as a mover with `mobility.type = "animal"`.

**Call-in shape.** A gun bought with a tow arrives **hitched**, with its crew riding or walking
(riding needs mount/dismount from the vehicle area). Once unhitched, the tow can:
- drive back to base, which frees its pop;
- stay as the gun's **ammo carrier** (§4.1), a soft target the enemy will hunt;
- carry crates for the HQ supply chain.

The gun type has its own cooldown, and the tow does not have a separate one.

**Trenches and guns.** Wheels are blocked by any dug ground deeper than about 0.6 m, so a gun
cannot cross a trench. The crew can lay a **plank bridge** (an engineer piece, later) or use the
**ramp** that every gun pit has. A pushed gun rolling over a finished trench uses S3's bog roll
with a high bog chance. This makes the placement of guns behind your own trench line a real
decision.

### 3.4 Shields, plates, small arms and HE

**Shield as plates.** The shield box's front face is a plate in the existing cosine rule.

| Gun | Shield (starting data) |
|---|---|
| Pak 40 / Pak 38 | two 4 mm plates spaced about 2.5 cm, sloped about 15–20° *(verify)*. Model as two faces, or one 8 mm face, until spaced armour (§2.4 reserved) exists. |
| 57 mm M1 | 6.35 mm single plate *(verify)* |
| ZiS-3, 45 mm | 6–7 mm single plate *(verify)* |
| leIG 18 | about 4–5 mm |
| howitzers | about 4–6 mm |

**Small arms against shields.** Today a rifle round "stops on a vehicle box with no damage".
For guns that is mostly right, since the shields were bulletproof against ball ammunition at
normal ranges. Two refinements, both physical:
- Small-arms weapon defs get an optional `pen_mm` table (default: stops on any plate). German
  7.92 mm S.m.K. (steel core) went through about 8–10 mm at 100 m *(verify)*, so a shield could
  be holed at close range.
- **Rounds that miss the shield continue.** The shield is a box, not an aura. Over the top, round
  the side, under the axle: they reach the crew capsules. That is the whole point of §3.2.

**AP against a gun.** Any AT round goes through a gun shield. The core continues 1.5 m (S3 rule)
and damages boxes and **soldier capsules** on its line: the gunner behind the shield, then the
breech. This needs a new step in the core trace: soldier capsules near the gun. For gun
entities only, this is cheap.

**HE against a gun.**
- Bursts damage gun anchors (sight, recoil, wheels, barrel) within the lethal radius in clear
  line, at `vs_gun_pct`.
- They hit the crew by ordinary burst or fragment rules. The shield blocks fragments arriving
  from the front.

**Tank gunnery change.** Tanks treat a spotted enemy gun as an **HE target, aimed at the crew
position behind the shield**, not an AP target. That was real doctrine. It is a small addition
to `gunnery.rs`'s target choice.

### 3.5 Profile, concealment, optics

- **Size.** Guns get `spot_size_pct` below a tank's:
  - Pak 40 about 90 % (1.25 m tall);
  - 45 mm about 70 %;
  - leIG 18 about 80 %;
  - 105 mm about 130 %.
- **In a gun pit** (§3.10) the hidden body points are hidden by real geometry: §5.10.4 already
  samples several body points against terrain.
- **Camouflage net** (a construction piece, later): a concealment modifier from data.
- **Firing reveals.** `fire_reveal_pct = 300`, `fire_reveal_ms = 5000` for AT guns: muzzle
  blast and dust; crews wetted the ground in front for this reason. Mortars 150 % for 2 s.
  Howitzers 400 % for 6 s.
- **Optics** (this matters for value). Today soldiers see about 150 m and tanks about 175 m.
  - An AT gun limited to that sight loses its one historical advantage: the long, first,
    aimed shot.
  - The gunner slot should carry the gun's **optic** (Pak 40: ZF 3×8 *(verify)*; 57 mm M1:
    M18 telescope; ZiS-3: panoramic and telescopic sights).
  - The optic gives a sight bonus against vehicles inside the gun's arc, using the "focused
    observation" mechanism from §5.12.3 in data form. Suggested: `optic_sight_pct = 250` inside
    the arc, which puts tanks at about 400–500 m.
  - **This ties to the open S3 owner question 2** (vehicle spotting range 2× or 4×); the two
    should be decided together.

### 3.6 Recrewing, abandoning, capturing, spiking

- **ServeGun command.** Any squad can be ordered onto any gun of its own side or any
  **neutral** gun. Its men take slots by priority; the rest stay as the squad's formation.
  - A gun with no own man in a slot and none within `hold_m` (15 m) for `abandon_s` (10 s)
    becomes **neutral**. Its ammo stays with it.
  - An enemy squad that serves a neutral gun takes ownership. Ownership is player-scoped, so a
    co-op ally's gun can be recrewed by a teammate via `ShareUnit` (§5.12.1).
- **Captured guns use their own ammo class** (§4.4). A captured Pak 40 refills only from German
  7.5 cm crates or its own tow. That is authentic and gives a reason to capture enemy supply.
- **Spike the gun** (optional, S): a `demolish`-capable man spends 10 s with a charge to destroy
  `gun` and `recoil` before abandoning. The AI spikes guns it is about to lose.
- **Bail-out happens naturally.** Under morale break the crew run like any squad, and the gun
  turns neutral after `abandon_s`. No special code beyond the neutral rule.

### 3.7 Direct and indirect fire

**Direct fire** (AT guns, infantry guns firing over open sights, ZiS-3, any howitzer at a
visible target):
- the gunner needs the target **spotted by the gun's own crew** (the gunner's or commander's
  line of sight);
- otherwise exactly the S3 gunnery path: lay at the gun's traverse and elevation rates, lead,
  ranging-in with each shot;
- shells are real projectiles that hit plates, capsules and terrain.

**Indirect fire** (mortars always; infantry guns and howitzers on a fire mission):

1. **Physical trajectory.** The weapon def lists **charges** (muzzle velocities) and an
   elevation band. The sim picks the lowest charge whose high-angle solution reaches the target
   inside the band. The order is deterministic: charges ascending, then elevation ascending.
   - **Minimum range** falls out of the physics (the lowest charge at the steepest elevation).
     `min_range_m` stays as a validator cross-check.
   - Flight time also falls out of the physics: about 20–30 s for an 81 mm at 2 km. That is
     itself a balance brake, because moving targets escape.
2. **The observer rule.** A fire mission is **observed** when the target point is in line of
   sight (terrain plus smoke) of an own soldier with the `observe` capability, **and** that
   observer is **linked** to the firing unit.

   | Link | When it applies |
   |---|---|
   | Voice / runner | Within `observer_link_m` (150 m) of the firing unit. |
   | Radio | The observer carries a radio (FO teams, platoon leaders, the HQ). Range `radio_link_m`; 0 means the whole map. |
   | Wire | Through the HQ's field-telephone net, if the HQ area adopts it (later). |

   Who has `observe`:
   - squad leaders;
   - mortar squad leaders;
   - FO teams (Germany: Vorgeschobener Beobachter with a Funker; USA: Forward Observer party;
     USSR: artillery razvedchik);
   - the HQ.
3. **Observed fire.** Base dispersion, tightened by `ranging_step_pct` for each round the
   observer *sees land* (its burst point in his line of sight), down to `ranging_floor_pct`.
   This reuses S3's ranging-in, keyed by target point.
4. **Map fire / unobserved fire** is allowed, which keeps players free:
   - dispersion × `map_fire_spread_pct` (300 %);
   - no ranging-in;
   - targets restricted to points in the player's **knowledge map**: last-seen positions,
     sound-ranged positions (§3.8), own and enemy capture points. No blind fire into unknown
     ground.
   
   Harassing fire on a known trench line is historical, and it should be inaccurate.
5. **Danger close.** The AI never fires within `danger_close_m` (50 m) of own men; humans get a
   HUD warning. Friendly fire from indirect HE is now on (owner decision Q22, 2026-11-01), so
   map fire near own men is a real risk. Direct fire still passes through own men.

**What changes for today's `Barrage`.** `Barrage` becomes `FireMission { firers, target, rounds,
ammo, mode }`:
- `mode` is `auto`, `observed` or `map`; `auto` means observed if any linked observer sees the
  point, else map;
- `rounds` is capped by the bombs actually carried (§4);
- the same command serves mortars, infantry guns and on-map howitzers.

### 3.8 Counter-battery: sound ranging and displacement

- **Sound ranging.** Every indirect shot emits an acoustic event (§5.6 sensor channel
  `acoustic`).
  - After `sound_ranging_delay_s` (10 s), the enemy's knowledge map gets a "battery" marker at
    the true position plus an error of `sound_ranging_error_pct` (10 %) of the distance to the
    nearest enemy listener (any soldier), from the counter RNG.
  - Repeated fire from the same spot shrinks the error.
  - The marker is a legal map-fire target. This makes counter-battery **physical**: real shells
    land with real scatter and kill only if fragments reach the crew.
- **Displacement.** Pack, move 50–150 m, emplace again. Mortar crews did this ("shoot and
  scoot"). The AI displaces after `displace_after_rounds` or when enemy HE lands within 30 m.
  Owner decision (Q24, 2026-11-01): players see only a hint from their own count ("N rounds
  from this spot: likely located"), never the enemy's marker.
- **Off-map batteries cannot be hit by counter-battery.** Honest limit: anything that "silences"
  an off-map battery would be an abstract effect, which the owner's physical-contact rule rules
  out. Their brakes are the **observer** (physical, killable), the shell allocation, the cooldown
  and the fire-direction delay.

### 3.9 Off-map or on-map artillery: recommendation

| | Off-map battery (`SupportCall`) | On-map gun |
|---|---|---|
| Historical fit for divisional howitzers | **Right**: 3–10 km behind the line | Wrong on 1–3 km maps |
| Historical fit for regimental and infantry guns | Wrong: they were close | **Right** |
| Physical damage rule | Kept: shells spawn at the map edge on the exact parabola from a notional battery position and kill only by fragments | Kept |
| Counter-battery | No (honest limit) | Yes (sound ranging) |
| Cost to build | **S–M**: no gun entity, reuses the shell projectile, needs the observer rule and a support def | Needs the gun entity, towing, slots and capture (built anyway for AT guns) |
| Balance risk | Contained: allocation, cooldown, observer, about 45 s fire-direction delay | High: a 15 kg shell every 15 s with authentic range reaches the whole map |
| Fits "walk to the front" | Yes: the **observer** walks to the front | The gun stays in base |

**Recommendation.**
- **Off-map by default** for the leFH 18, 105 mm M2A1 and 122 mm M-30 (and later corps guns and
  the Nebelwerfer or Katyusha as rocket fire missions).
- **On-map for regimental and infantry guns:** leIG 18, sIG 33, 105 mm M3, 75 mm M1A1, 76 mm
  M1943; for AT guns; for heavy mortars; and for the ZiS-3 as a dual-role gun.
- **On-map divisional howitzers** as a mode flag (`artillery_mode = "offmap" | "onmap_rear" |
  "both"` in `defs/rules/`), restricted to deploying in the own base or a `battery_zone`. They
  are also for missions; the leFH 18 and M-30 really fired over open sights at tanks in the
  East.

**Off-map fire mission flow.**
1. An FO team (or the HQ, or a radio-carrying officer) issues
   `CallSupport { support, target, observer }`. The target must be **observed**: off-map
   batteries never map-fire in skirmish; this is a mode option.
2. After `time_to_fire_s` (fire-direction centre, 30–60 s), the battery fires one **adjusting
   round** per `adjust_rounds`.
3. The observer may nudge the target (`AdjustSupport`) and then call **fire for effect**:
   `tubes × rounds_per_tube` shells, with dispersion from the range-table probable errors at the
   notional range.
4. **Physics.** Each shell spawns at the map boundary on the exact parabola from the notional
   battery position (fixed point, computed once) and flies the rest of its arc as an ordinary
   projectile. Incoming shells play a whistle 1–2 s before impact (presenter, from the snapshot).
   Bursts use fragment rules.
5. **Limits.**
   - per-support cooldown (the same per-type cooldown the call-in decision introduced);
   - a manpower or supply cost;
   - a per-match shell allocation in data (`allocation_rounds`), shown on the HUD;
   - the AI uses the same limits.

### 3.10 Trenches, gun pits and artillery against fortifications

- **New dig shapes.** These need a *pit* profile kind (an ellipse or horseshoe with a ramp)
  alongside today's linear profiles. That is the start of §5.10.3 blueprints.

  | Pit | Shape | Effect |
  |---|---|---|
  | `gun_pit_at` | about 5 × 4 m, 0.6–0.8 m deep, ramp to the rear, parapet front and sides | The shield and barrel sit just above the parapet. Shots at the carriage hit earth. |
  | `mortar_pit` | about 2 m wide, 1.2–1.5 m deep, round, with ammo niches | Crew safe from everything but a burst in the pit. The mortar fires over the lip at high angle. |
  | `howitzer_pit` | about 6 × 6 m, 0.8 m deep, ramp | For emplaced heavy guns. |

  - These are engineer jobs by default: `dig_full`. Gun crews may dig a `mortar_pit` with
    `dig_basic` at their own rate.
  - Times under §5.10.9 compressed realism: AT gun pit about 2 min for 4 engineers; mortar pit
    about 60 s for the crew.
- **A dug-in gun is the real "trench pillar" payoff.** An AT gun in a pit behind a trench line,
  with MGs in the trench, is the 1944 defensive position. It works through geometry alone, with
  no bonus tags.
- **Artillery as the counter to trenches.** Today a finished trench is nearly unassailable
  (S1b known issue). Heavy HE shells should:
  - carve craters that cut parapets and fill trench sections (`crater_cut`, as satchels do,
    scaled by `he_filler_g`);
  - break dugout roofs (roof piece HP, later);
  - suppress by real near-misses: a 105 mm burst within its suppression distance is a near-miss
    for everyone in clear line.
  
  With **smoke fire missions** (81 mm and 4.2 inch WP, 105 mm smoke), this gives the assault the
  missing 60 m: prepare, smoke, close in. That matches the levers the S1b issue already lists.
- **Tree bursts** (optional, S–M). Woods get a canopy height in map data. A shell crossing the
  canopy over woods bursts there, and its fragments rain down into open foxholes. Hürtgen and the
  Ardennes are the historical cases, and this makes overhead cover worth digging. It is physical
  and fits the trench pillar.

### 3.11 Mortar dominance: diagnosis and fix order

| Cause today | Evidence | Fix (no stat nerf) | Where |
|---|---|---|---|
| Unlimited bombs | `Barrage` only counts down the order's own `rounds` | Carried bombs per man; resupply through §4 | G0 |
| Fires at any point, never observed | `Barrage` takes any ground point (the AI uses spotted groups) | Observer rule; map fire at ×3 spread on known points only | G0 |
| Full accuracy from the first round | Fixed 50 mrad, no ranging | Ranging-in from a wider first round; tighter only when observed | G0 |
| No risk from firing | No sound ranging; the team never moves | Sound ranging plus displacement | G0b |
| Radial damage, not fragments | `combat::burst` | 32 fragment rays against capsules (§5.7); prone and trench men are hit far less | F0 |
| AI fires as often as it can | `barrage_rounds` per spotted group | AI ammo budget and reserve | G0 |

**Recommended order:**
1. G0: finite bombs, observer rule, ranging.
2. Measure the HE kill share.
3. F0: fragment rays.
4. Measure again.
5. Only then add on-map infantry guns or heavy mortars.

Target: the HE share of kills falls from about 43 % to roughly 20–30 %, with trench casualties
under a barrage still at most 40 % of those in the open (S1's `fortify-test` bar). Both are
owner calls.

---

## 4. Ammunition for guns and mortars (the guns' side of the HQ / crate design)

### 4.1 Where rounds live

Every round is counted somewhere physical and is moved by a man or a vehicle:

| Store | Holds | Notes |
|---|---|---|
| **Man's kit** | bombs or shells by `ammo_class` and count, up to `carry_kg` | A mortar ammo bearer carries 2 cases of 3 × 8 cm bombs, about 25 kg. Reuses the `Kit` column, with per-class counts beside today's item counts. |
| **Gun ready rack** | `ready_rounds` per ammo id, next to the gun | Counts as the gun's `ammo` component box. Loading from here is instant (loader slot). |
| **Gun reserve** | further rounds "in the pit" | Fetched at `fetch_s` per round by `ammo` slot men, or by the loader if none. |
| **Tow or ammo vehicle** | `stowage` per ammo class | A man within 3 m transfers rounds (the §5.12.2 inventory rules: timed, interrupted by movement or pinning). |
| **Ammo crate** (HQ area) | one ammo class, N rounds | Carried by 1–2 men (speed penalty), placed, or delivered by truck or halftrack. Any man within 2 m can draw from it. |

Starting data:

| Weapon | Carried by the squad at spawn | Ready / reserve at the gun | In the tow |
|---|---|---|---|
| 60 mm M2 (5 men) | 30 bombs (6 per man) | — | — |
| kz. GrW 42 (4 men) | 18 bombs | — | — |
| 8 cm s.GrW 34 / 81 mm M1 / 82 mm (5 men) | 18–24 bombs (leader 0, gunner 3, loader 3, two bearers 6–9) | — | — |
| 12 cm GrW 42 / PM-38 (6 men) | 6 bombs | 12 with the carriage | 30 |
| Pak 40 (6 men) | 0 | 10 AP, 6 HE (plus 2 APCR, scarce) | 30 |
| 57 mm M1 (6 men) | 0 | 12 AP (HE only after its date) | 30 |
| 45 mm M-42 (5 men) | 10 | 15 / 10 | 40 |
| leIG 18 (5 men) | 4 | 12 HE, 2 HEAT | 30 |
| 105 mm M3 (6 men) | 0 | 10 HE, 2 HEAT, 2 smoke | 24 |

These are gameplay starting values, not tables of organisation. Real gun positions had far more
ammunition dumped. The point is that **a gun runs out within one engagement unless supplied**.

**Rare rounds (owner decision D3).** APCR, HVAP and HEAT are rare rounds. Each gun has a rare
cap (`rare_cap` on the gun def; ceiling and default `rare_cap_max`, 10). Starting values: Pak 40
2 APCR, 57 mm M1 none, 3-inch M5 3 HVAP *(from its date)*, 45 mm M-42 10 APCR, ZiS-3 5 APCR,
leIG 18 and 105 mm M3 2 HEAT *(all verify)*. A rare round refills at any source that serves the
gun, at 1 per 10 s in base and 1 per 30 s forward, never above the cap. The gun and its own tow
share one cap; the tow holds no rare rounds of its own (critic pass 3).

### 4.2 What runs out, and what it means

- **Mortars run out first.** About 20 bombs is 80 s of fire at 15 rpm. Each fire mission is a
  decision: use them now, or keep a reserve for the defence.
- **AT guns run out of AP last.** They fire seldom, but **APCR / HVAP** (PzGr 40, T4 HVAP,
  BR-240P) are counted separately and scarce (a per-gun `rare_cap`, `available_from` dated). That
  is historically right: tungsten was rationed from 1943. They refill forward too, but slowly
  (D3).
- **Infantry guns and howitzers run out of HE** in a sustained barrage. Their HEAT rounds
  (Igr 38 HL, M66 for the pack howitzer, BP-350M) are a few per gun.
- **Small arms** belong to the infantry areas. For gun crews only: their personal weapons
  draw from the same rules as everyone else.

### 4.3 How resupply flows

1. **Base zone.** Men in base regain carried small-arms rounds at `base_resupply_round_s` per
   man, each kg charged at `small_arms_mp_per_kg` (D1). AT guns and their tows regain stowage
   like vehicles (1 round per 2 s; rare rounds 1 per 10 s up to `rare_cap`), each round charged
   at `shell_mp_per_kg` times the gun's rack price factor (a full refill at most 75 % of the card;
   the same price forward, by credit; `artillery-on-map.md` §4.2). Those rounds feed only that gun; they never pass to a crate, dump, HQ or
   another weapon (`rack_rounds_transferable = false`). **Mortars, infantry guns and howitzers do
   not refill in base** (owner decision Q4): their shells and bombs leave the base only in crates
   or vehicle loads, at `shell_mp_per_kg` manpower per kg. Vehicle loads take 32 kg per
   `base_crate_issue_s`, in the same queue as crates. **The ZiS-3's HE is crate-only too**,
   because it can fire indirect (`crate_only_if_indirect`); its AP rearms in base at the shells
   price.
2. **HQ.** The HQ (other area) produces **ammo crates by class**: "8 cm Wgr. case ×6",
   "7.5 cm Pak ×12", "105 mm ×8". Each crate draws on the HQ's stock, which was carried in.
   - Men carry crates forward: 1 man, about 25 kg, speed ×0.7; 2 men, about 50 kg.
   - Or the HQ resupplies everyone within its radius at a slow rate, without crates. AT guns
     inside it rearm like vehicles at the forward rate (1 round per 4 s; rare rounds 1 per 30 s).
   - Nothing drawn from the HQ is charged again: its kg were paid when they left the base.
   - Crates are physical entities: a mortar bomb hitting a pile of crates can set them off.
     This is physical, authentic and gives a reason to spread supply out.
3. **Truck or halftrack.** A tow or ammo carrier (for example Sd.Kfz. 11 or Opel Blitz; US M3
   halftrack, Dodge WC, CCKW; the exact list belongs to the vehicle area) holds stowage per class, refills at the HQ, or at base as a paid load (`artillery-on-map.md` §4.2), and drops
   crates or transfers to men within 3 m. The tow that brought a gun is its natural ammo carrier.
4. **Ammo bearers walk.** A mortar bearer who runs dry can be ordered "fetch ammo" (the AI does
   it on its own): he walks to the nearest own crate, truck, HQ or base, loads up, and returns.
   Losing bearers on that walk *is* the supply interdiction. It is also the whole cost of
   distance (D2): a gun pushed forward needs longer walks and longer drives, and there is no
   price surcharge on top.

**The base-camping problem: owner decision (2026-11-01).** With authentic ranges and free base
resupply, a mortar or gun parked in base fires forever. This file recommended a per-player supply
throughput. The owner chose instead:
- no throughput and no ration;
- each gun and mortar has a **ready inventory** (the rounds at the gun, or the bombs the crew
  carries), refilled only from a crate within 5 m, an HQ or dump radius, or its tow;
- the base zone does not refill them;
- shells crates cost manpower at base: `shell_mp_per_kg` (start 1.0, so about 3.5 per 81 mm
  bomb, about 10 per 75 mm AT round and 15–19 per 105 mm round);
- starting loads come with the card's price.

Owner decision D1 (later the same day) priced the rest too: small arms 0.2 and explosives 0.5
per kg, AT-gun and tank rounds at the shells price, in base as well. No refill is free.

It competes with buying men, which is the point. It is weaker than a throughput cap: a richer
player can buy more HE. `artillery-on-map.md` §4.10 re-checks each exploit and lists the levers.

### 4.4 Ammo classes, compatibility and capture

- Each ammo def gets `class` (`mortar_8cm_ger`, `mortar_81_us`, `mortar_82_sov`, `pak_75`,
  `shell_105_us`, …). Each weapon gets `accepts = [classes]`.
- **Authentic quirk:**
  - Soviet 82 mm mortars could fire captured German 8 cm (81.4 mm) bombs, at reduced accuracy;
  - the German 8 cm could not fire Soviet 82 mm bombs;
  - US 81 mm and German 8 cm bombs were close in calibre; cross-use is reported *(verify)*;
    default to not compatible.
- Captured guns use only their own class (§3.6). Captured enemy crates and trucks are valuable.

### 4.5 Fit with today's base-zone resupply

Keep today's resupply timings for items, rockets, vehicle base repair and rearm, and AT-gun
stowage. Change three things:
1. extend it to the new small-arms classes;
2. remove mortar bombs and infantry-gun and howitzer shells from it (owner decision Q4): those
   come only in priced crates (from S6 for mortars);
3. charge every refill at its kind's price (owner decision D1); it pauses when the player cannot
   pay.

The new forward flows (HQ, crates, trucks, bearers) are **additions**. A match with no HQ plays
like today, except that mortars carry finite bombs and refill only from crates, and every refill
costs manpower.

### 4.6 AI

The AI needs these capabilities:

- **Budget.** Each mortar or gun keeps a reserve: `ai.ammo_reserve_pct`, 30 %, held for the
  defence of its point. Below the reserve it fires only at attackers within 200 m of an own held
  point.
- **Resupply task.** Below 50 %, send the bearer or the tow to the nearest own crate, HQ or dump;
  the base (a priced crate) only if none. AT guns do not wait for rare rounds while a tank is
  seen.
  Below 20 %, the whole team displaces toward supply. The AI's HQ planner places crates near
  mortar and gun positions.
- **Observer pairing.** A mortar picks targets the AI's linked observers actually see. If none,
  it moves the mortar squad leader forward as observer, or assigns the nearest rifle squad
  leader within link range.

---

## 5. Roster by faction

Faction names follow the period. The game shows them in the faction's language; the English
gloss is for the owner.

**Value** column: how much the unit adds to play (★ low to ★★★ high). **Cost**: implementation
effort *once its slice's systems exist* (S/M/L).

### 5.1 Mortars

| Faction | Unit (period name) | Weapon | Mass | Range | Bomb | Rate (sustained / max) | Crew | Mobility | Available | Value | Cost |
|---|---|---|---|---|---|---|---|---|---|---|---|
| GER | Granatwerfergruppe (kz.) | 8 cm kz. GrW 42 | ≈26 kg | ≈50–1,100 m | 3.5 kg Wgr 34 HE; Wgr 34 Nb smoke | ≈15 / 20–25 | 4 | carried, 3 loads | 1942– | ★★ | S |
| GER | (2nd line) | 5 cm leGrW 36 | 14 kg | ≈50–520 m | 0.9 kg | ≈15 / 25 | 3 | carried | withdrawn ≈1943 | ★ | S |
| GER | Granatwerfergruppe (s.) | 8 cm s.GrW 34 | ≈57 kg | ≈60–2,400 m | 3.5 kg HE, smoke | ≈15 / 25 | 5 (hist. 6) | carried, 3 loads | all | ★★★ | S (exists) |
| GER | schwere Granatwerfergruppe | 12 cm GrW 42 | ≈280 kg on a 2-wheel carriage | ≈300–6,050 m | 15.8 kg HE | ≈8 / 10–15 | 6 | towed or pushed (gun entity) | 1943– | ★★ | M |
| USA | 60mm Mortar Squad | 60 mm M2 | 19 kg | ≈90–1,800 m | 1.33 kg M49A2 HE; M302 WP | ≈18 / 30–35 | 5 | carried | all | ★★★ | S |
| USA | 81mm Mortar Squad (Hvy Wpns Co.) | 81 mm M1 | ≈62 kg | ≈90–2,900 m (light HE), ≈2,300 m (heavy HE) | 3.1 kg M43A1 / 4.9 kg M56; M57 WP | ≈18 / 30–35 | 5 (hist. 6–8) | carried, 3 loads | all | ★★★ | S (exists) |
| USA | 4.2-inch Chemical Mortar Squad | 4.2 in M2 (rifled) | ≈150 kg | ≈500–4,000 m *(verify min)* | 11.3 kg HE; WP | ≈5 / 20 for 1 min | 6 | handcart or jeep; gun entity | all (separate battalions) | ★★ (smoke) | M |
| USSR | 82-mm minomet crew (battalion mortar company) | 82 mm BM-37 / BM-41 / BM-43 | ≈52–58 kg | ≈85–3,000 m | 3.1–3.4 kg O-832D | ≈15 / 25 | 5 | carried | all | ★★★ | S (with USSR) |
| USSR | 120-mm regimental mortar crew | 120 mm PM-38 / PM-43 | ≈280 kg on wheels | ≈460–5,700 m | 16 kg | ≈10 / 15 | 6 | towed or pushed (gun entity) | all | ★★ | M (with USSR) |

**Differentiation without stat cheating.** Today `grw34` and `m1_mortar` are identical. The real
differences:
- **US 60 mm:** light, at company level, more bombs per man, short range, small bursts. A
  frequent, cheap, accurate suppression tool.
- **US 81 mm:** a light-HE / heavy-HE choice (range against burst).
- **German s.GrW 34:** about the same tube, fewer bombs per man.
- **Germany and USSR heavy mortars:** a big burst, slow, needs a tow. The Red Army's mortar mass
  is a roster and availability trait (cheaper, lower cooldown), not a stat buff.

### 5.2 Anti-tank guns

| Faction | Unit | Gun | Mass | Crew | AP (velocity) | Special | HE | Rate | Traverse | Value | Cost |
|---|---|---|---|---|---|---|---|---|---|---|---|
| GER | Pak-Geschütz (Panzerjäger-Zug) | 7.5 cm Pak 40 | ≈1,425 kg | 6 | PzGr 39, ≈790 m/s | PzGr 40 APCR, ≈930 m/s, 0–2 rounds | Sprgr 34 | ≈12–14 | ≈65° | ★★★ | L (first gun) then S |
| GER | (2nd line, 1944 still common) | 5 cm Pak 38 | ≈1,000 kg | 5 | PzGr 39, ≈820 m/s | PzGr 40, ≈1,180 m/s | Sprgr 38 | ≈13 | ≈65° | ★★ | S |
| GER | (heavy, wave 2) | 8.8 cm Pak 43/41 or Pak 43 | ≈4.4 t | 8 | PzGr 39/43, ≈1,000 m/s | — | Sprgr | ≈6–10 | 56° / 360° | ★★ (danger) | M |
| USA | 57mm Antitank Squad (Inf. Bn.) | 57 mm M1 | ≈1,230 kg | 6 (hist. 10 incl. driver and bearers) | M86 APC, ≈850 m/s | — | M303 HE from late 1944 *(verify)* | ≈12–15 | ≈90° | ★★★ | S |
| USA | 3-inch Gun Squad (TD Bn., towed) | 3 in M5 on M1/M6 carriage | ≈2.2–2.6 t | 8 | M62 APC, ≈790 m/s | T4/M93 HVAP, ≈1,035 m/s, from ≈late 1944, very scarce | M42 HE | ≈12 | ≈45° | ★★ | S |
| USSR | 45-mm crew | 45 mm M-42 | ≈625 kg | 5–6 | BR-240, ≈870 m/s | BR-240P APCR, ≈1,070 m/s | O-240 | ≈15–20 | ≈60° | ★★ | S |
| USSR | 57-mm crew (IPTAP) | 57 mm ZiS-2 | ≈1,250 kg | 6 | BR-271, ≈990 m/s | BR-271P | O-271U | ≈15–25 | ≈56° | ★★★ | S |
| USSR | ZiS-3 crew (divisional / IPTAP) | 76 mm ZiS-3 | ≈1,116 kg | 6–7 | BR-350A APHE, ≈660 m/s | BR-350P APCR, ≈960 m/s | OF-350 (good HE); also indirect to ≈13 km | ≈15–25 | ≈54° | ★★★ (dual role) | S |

The ZiS-2 is not in the request but should be: it was the Red Army's best AT gun of 1944.

### 5.3 Infantry guns and howitzers

| Faction | Unit | Gun | Mass | Crew | Range | Shell | Rate | Elevation / traverse | Placement | Value | Cost |
|---|---|---|---|---|---|---|---|---|---|---|---|
| GER | Infanteriegeschützgruppe | 7.5 cm leIG 18 | ≈400 kg | 5 | ≈3,550 m | 6 kg Igr 18 HE; Igr 38 HL HEAT *(verify pen)* | ≈8–12 | −10° to +73° / ≈11° | **on-map**, pushable, direct and high-angle | ★★★ | M |
| GER | schwere Infanteriegeschützgruppe | 15 cm sIG 33 | ≈1,750 kg | 6–7 | ≈4,700 m | 38 kg HE; Stielgranate 42 demolition | ≈2–3 | 0° to +73° / ≈11° | on-map, needs a tow | ★★ (trench buster; danger) | S after leIG |
| GER | leichte Feldhaubitz-Batterie | 10.5 cm leFH 18 / 18M | ≈2.0 t | 6 | ≈10,700 m (18M ≈12,300) | 14.8 kg FH Gr; FH Gr Nb; Pzgr / Gr 39 HL | ≈4–6 | up to +42° / ≈56° | **off-map** (on-map: mode or mission) | ★★★ | M (`SupportCall`) |
| USA | Cannon Company Howitzer Section | 105 mm Howitzer M3 | ≈1,130 kg | 6 | ≈7,600 m | 15 kg M1 HE; M67 HEAT; smoke | ≈2–4 sustained | −9° to +30° *(verify)* / ≈45° | **on-map**, tow | ★★★ | S after leIG |
| USA | 75mm Pack Howitzer Section (Airborne FA) | 75 mm Pack Howitzer M1A1 (carriage M8) | ≈650 kg | 5 | ≈8,800 m | 6.2 kg M48 HE; M66 HEAT | ≈3–6 | −5° to +45° / ≈6° | on-map, pushable | ★★ | S |
| USA | Field Artillery Battery | 105 mm M2A1 | ≈2.26 t | 8 | ≈11,300 m | 15 kg HE; smoke; WP | ≈2–4 sustained, 10 max | up to +65° / ≈46° | **off-map** | ★★★ | S once `SupportCall` exists |
| USSR | 76-mm regimental gun crew | 76 mm M1943 (OB-25) | ≈600 kg | 5–6 | ≈4,200 m | 6.2 kg HE; BP-350M HEAT | ≈10–12 | up to +25° *(verify)* / ≈60° | on-map, pushable | ★★ | S (with USSR) |
| USSR | Howitzer battery | 122 mm M-30 | ≈2.45 t | 8 | ≈11,800 m | 21.8 kg OF-462 | ≈5–6 | up to +63.5° / ≈49° | **off-map** | ★★★ | S (with USSR) |

### 5.4 Observers (the units that make indirect fire work)

| Faction | Unit | Men | Capabilities | Notes |
|---|---|---|---|---|
| GER | Vorgeschobener Beobachter (VB) with Funker | 2–3 | `observe`, `radio` | Calls off-map leFH and on-map guns. |
| USA | Forward Observer Party (FA) | 3 (officer, sergeant, radio operator) | `observe`, `radio` | US artillery's fast observed fire (time on target) was its signature. Model it as a shorter `time_to_fire_s` for US batteries: a doctrine trait, not a stat buff. |
| USSR | Artillery razvedchik / observer | 2–3 | `observe`, `radio` (wire later) | Longer `time_to_fire_s` and more tubes per mission: the authentic doctrine trade. |
| all | squad leaders, mortar squad leaders, officers | — | `observe` (link by voice or runner only) | Lets mortars fire observed without a dedicated FO. |

FO teams are cheap (about 80–120 MP), fragile, and have to walk to the front. That is where
artillery's "walk is the balance" comes from.

---

## 6. Penetration data in our cosine model

All tables are `[range_m, mm]` at `pen_reference_angle_deg`. The loader normalises to 0°, so
German and US 30° tables and Soviet 0° tables can sit side by side. Soviet tables also used a
different success criterion; this needs a note in the sources. *(Verify all.)*

| Ammo | Mechanism | m/s | Table | Ref. |
|---|---|---|---|---|
| 7.5 cm PzGr 39 (Pak 40, shared with KwK 40) | aphe | 790 | 100 → 106, 500 → 96, 1000 → 85, 1500 → 74, 2000 → 64 | 30° |
| 7.5 cm PzGr 40 | apcr | 930 | 100 → 143, 500 → 120, 1000 → 97, 1500 → 77 | 30° |
| 5 cm PzGr 39 (Pak 38) | aphe | 823 | 100 → 69, 500 → 59, 1000 → 48, 1500 → 38, 2000 → 29 | 30° |
| 5 cm PzGr 40 | apcr | 1,180 | 100 → 130, 500 → 99, 1000 → 72 | 30° |
| 57 mm M86 APC (M1) | apc (aphe-like, no filler) | 850 | 100 → 81, 500 → 73, 1000 → 63, 1500 → 54 | 30° |
| 3 in M62 APC (M5) | aphe | 790 | 100 → 98, 500 → 92, 1000 → 86, 1500 → 80, 2000 → 74 | 30° |
| 3 in T4 / M93 HVAP | apcr | 1,035 | 500 → 150, 1000 → 135, 1500 → 120 | 30° |
| 45 mm BR-240 (M-42) | aphe | 870 | 100 → 70, 500 → 61, 1000 → 51 | 0° |
| 45 mm BR-240P | apcr | 1,070 | 100 → 105, 500 → 84 | 0° |
| 57 mm BR-271 (ZiS-2) | ap | 990 | 100 → 115, 500 → 106, 1000 → 96, 1500 → 85 | 0° |
| 76 mm BR-350A (ZiS-3) | aphe | 660 | 100 → 82, 500 → 70, 1000 → 61, 1500 → 53 | 0° |
| 76 mm BR-350P | apcr | 960 | 100 → 105, 500 → 92 | 0° |
| 7.5 cm Igr 38 HL (leIG 18) | heat | ≈260 *(verify)* | flat ≈85–90 | 0° |
| 75 mm M66 HEAT (pack howitzer) | heat | ≈300 | flat ≈90 | 0° |
| 105 mm M67 HEAT (M3, M2A1) | heat | ≈380 | flat ≈100 | 0° |
| 10.5 cm Gr 39 HL (leFH 18) | heat | ≈ *(verify)* | flat ≈90–100 | 0° |

Mechanism notes:
- `apcr` is already listed in the §2.4 mechanism set. It needs an S-sized sim addition: steep
  loss with range is already captured by its table, but APCR should have **no filler** and a
  **small core box count** (it shatters).
- APCR is also a classic case for the existing ricochet rule at high obliquity: give it a lower
  `ricochet_angle_deg` (60°).
- Low-velocity HEAT rounds (infantry guns and howitzers) fly slow, curved trajectories, so range
  and lead errors matter. The physics does this for free.

---

## 7. TOML shapes

### 7.1 A gun (`defs/vehicles/ger_pak40.toml`)

```toml
id = "ww2:ger_pak40"
name = "7,5 cm Pak 40"
short_name = "Pak 40"
faction = "ww2:germany"
role = "at_gun"
crew_model = "served"             # soldiers in service slots, not seats
mass_kg = 1425
spot_size_pct = 90
eye_height_cm = 110
available_from = "1942-02"
sources = ["placeholder: owner / M2 citation task"]

[mobility]
type = "towed"
push_cm_s = 35                    # full crew, flat firm ground
push_men = 5                      # men needed for full push speed
push_max_slope_pct = 15
emplace_s = 30
pack_s = 18
hitch_s = 20
relay_trail_s = 15                # turning beyond the traverse arc
wheel_block_dm = 6                # dug ground deeper than this blocks the wheels

[carriage]
box = { center = [-60, 0, 55], half = [200, 95, 50] }
[shield]
box = { center = [40, 0, 85], half = [4, 110, 55] }

[armor]                           # S3 plate rule; the shield is two spaced plates
shield_front  = { thickness_mm = 4, slope_deg = 18, material = "rha" }
shield_inner  = { thickness_mm = 4, slope_deg = 18, material = "rha", gap_cm = 3 }
carriage      = { thickness_mm = 6, material = "rha" }

[optic]
sight_pct = 250                   # against vehicles inside the arc (gunner slot filled)

[[service]]                       # gun frame, cm; priority = swap order
role = "gunner"
at = [10, 35, 0]
pose = "kneel"
[[service]]
role = "loader"
at = [0, -40, 0]
pose = "kneel"
[[service]]
role = "commander"
at = [-300, 80, 0]
pose = "kneel"
[[service]]
role = "ammo"
at = [-450, -60, 0]
pose = "prone"
count = 2

[[weapons]]
def = "ww2:pak40_l46"
mount = "carriage"
muzzle = [330, 0, 95]
traverse_deg = [-32, 32]
traverse_deg_s = 8
elevation_deg = [-5, 22]
ready = [{ id = "ww2:75mm_pzgr39", count = 10 }, { id = "ww2:75mm_sprgr34", count = 6 },
         { id = "ww2:75mm_pzgr40", count = 2 }]
reserve = [{ id = "ww2:75mm_pzgr39", count = 10 }, { id = "ww2:75mm_sprgr34", count = 10 }]
rare_cap = 2                        # D3: PzGr 40 was scarce; refills at rare_rearm_s, never above 2
fetch_s = 4

[[components]]
type = "gun"
hp = 80
box = { center = [-20, 0, 85], half = [40, 15, 15] }
[[components]]
type = "barrel"
hp = 60
anchors = [{ pos = [200, 0, 95] }]
[[components]]
type = "sight"
hp = 30
anchors = [{ pos = [20, 30, 105] }]
repair = { to_hp = 30, time_s = 30 }
[[components]]
type = "recoil"
hp = 50
box = { center = [60, 0, 75], half = [60, 10, 10] }
repair = { to_hp = 50, time_s = 45 }
[[components]]
type = "wheels"
hp = 60
anchors = [{ pos = [0, 85, 40] }, { pos = [0, -85, 40] }]
repair = { to_hp = 60, time_s = 30 }
[[components]]
type = "ammo"
hp = 40
box = { center = [-120, -50, 20], half = [30, 25, 15] }

[[stations]]                      # direct control (§5.12.2 "Static guns")
id = "gunner"
slots = ["gunner"]
```

### 7.2 The squad (`defs/units/ger_pak40_squad.toml`)

```toml
id = "ww2:ger_pak40_squad"
name = "Pak-Bedienung (7,5 cm Pak 40)"
faction = "ww2:germany"
role = "gun"
gun = "ww2:ger_pak40"
tow = "ww2:ger_sdkfz11"            # optional; spawns hitched (vehicle area)
men = 6
weapon = "ww2:kar98k"
cost = 380                         # gun + crew; tow priced in its own def
cooldown_s = 90                    # per-type cooldown (call-in decision)
speed_cm_s = 260
sight_m = 150
spacing_dm = 25
dig_rate_pct = 100
capabilities = ["gun_crew", "dig_basic"]
loadout = [{ item = "ww2:stielhandgranate", count = 0, leader_count = 1 }]
```

### 7.3 A mortar with indirect fire and carried bombs

```toml
# defs/weapons/grw34.toml
id = "ww2:grw34"
name = "8 cm s.GrW 34"
class = "mortar"
range_m = 2400                     # authentic, only once G0 ships (§2 P4)
rounds_per_min = 15
spread_mrad = 12                   # observed and ranged-in floor comes from rules
setup_ms = 3000
accepts = ["mortar_8cm_ger"]
parts = ["tube", "bipod", "baseplate"]   # carried by named men; handover like launchers

[indirect]
elevation_deg = [45, 85]
charges_m_s = [70, 95, 120, 145, 174, 211]   # *(verify)* Wgr 34 charge velocities
first_round_spread_pct = 250       # before ranging-in

# defs/ammo/8cm_wgr34.toml
id = "ww2:8cm_wgr34"
name = "8 cm Wgr 34"
class = "mortar_8cm_ger"
mechanism = "he"
mass_kg = 3.5
he_filler_g = 460
[he]                               # unchanged burst block until F0, then [fragments]
# ...
[fragments]                        # F0: physical fragments (§5.7)
rays = 32
damage = 60
range_m = 25
pen_mm = 1
```

The unit def's loadout gains ammo by class:

```toml
loadout = [{ ammo = "ww2:8cm_wgr34", count = 6, leader_count = 0 },
           { ammo = "ww2:8cm_wgr34nb", count = 1 }]
```

### 7.4 An off-map battery (`defs/support/ger_lefh18_battery.toml`)

```toml
id = "ww2:ger_lefh18_battery"
kind = "support_call"
name = "leichte Feldhaubitz-Batterie (10,5 cm leFH 18)"
faction = "ww2:germany"
weapon = "ww2:lefh18"
ammo = ["ww2:105mm_fhgr38", "ww2:105mm_fhgr_nb"]
tubes = 4
rounds_per_tube = 2
adjust_rounds = 1
notional_range_m = 5000            # battery sits this far behind the own map edge
time_to_fire_s = 45
range_pe_m = 22                    # probable error at the notional range *(verify)*
deflection_pe_m = 4
requires = ["observe", "radio"]
observed_only = true               # skirmish default; mode can allow map fire
cost = 300
cooldown_s = 240
allocation_rounds = 48             # per match
```

### 7.5 Rules additions (`defs/rules/skirmish.toml`)

```toml
[indirect]
observer_link_m = 150
radio_link_m = 0                   # 0 = whole map
map_fire_spread_pct = 300
ranging_step_pct = 25
ranging_floor_pct = 40
danger_close_m = 50
sound_ranging_delay_s = 10
sound_ranging_error_pct = 10
artillery_mode = "offmap"          # "offmap" | "onmap_rear" | "both"

[guns]
abandon_s = 10
hold_m = 15
slot_swap_s = 3
untrained_reload_pct = 150
untrained_spread_pct = 150
base_resupply_round_s = 3

[supply]                           # owner decisions Q4, D1-D3; full block in artillery-on-map.md §8
crate_only_classes = ["mortar_bomb", "gun_shell", "rocket_artillery", "rifle_grenade_he"]
small_arms_mp_per_kg = 0.2         # D1: cheap
explosives_mp_per_kg = 0.5         # D1: moderate
shell_mp_per_kg = 1.0              # manpower per kg of shells (mortar, gun, AT and tank rounds) leaving the base
rack_rounds_transferable = false   # AT and tank rack rounds feed only their own weapon
crate_only_if_indirect = true      # ZiS-3 HE (and any ammo an [indirect] weapon fires) is crate-only
rearm_s = { base = 2, forward = 4 }
rare_rearm_s = { base = 10, forward = 30 }   # D3
rare_cap_max = 10                  # D3: default and ceiling for a gun's or vehicle's rare_cap
```

### 7.6 Dig profiles (pit kind)

```toml
[dig.profiles.gun_pit_at]
kind = "pit"
size_cm = [500, 400]
depth_cm = 70
ramp = "rear"
parapet_cm = 40
parapet_sides = ["front", "left", "right"]
work = 30000
engineers_only = true

[dig.profiles.mortar_pit]
kind = "pit"
size_cm = [220, 220]
depth_cm = 130
parapet_cm = 30
work = 9000
capability = "dig_basic"
```

---

## 8. Sim systems touched, and new commands

| Module | Changes |
|---|---|
| `content` (`defs.rs`, `vehicle.rs`, `setup.rs`) | `crew_model`, `service` slots, towed mobility, `ready` and `reserve`, `[indirect]`, `accepts` and ammo `class`, `apcr`, `[fragments]`, `support_call` defs, pit profiles, squad `gun`, `tow`, `capabilities`, ammo loadouts; validator rules (slot positions inside the reach of the gun, charge table monotonic, `available_from` on ammo). |
| `vehicle.rs` | Gun entity state (emplaced, limbered, pushed, hitched), push movement, hitch following, trail re-lay, mutable owner, neutral state. |
| `armour.rs` | Shield plates, the second spaced plate, the AP core continuing into soldier capsules near gun entities. |
| `gunnery.rs` | Gun direct fire through the existing laying code; limited arc; the gun optic sight bonus; tanks treating guns as HE targets. |
| `combat.rs` | `Barrage` → `FireMission`; charge and elevation solver; observer rule and ranging state per target; map-fire spread; mortar parts handover; bombs drawn from kits. |
| `damage.rs` | HE vs gun anchors (`vs_gun_pct`); ammo box detonation; F0 fragment rays against capsules, gun boxes and vehicle anchors. |
| `items.rs` | Per-class ammo counts in `Kit`; transfer between men, crates, tows and guns; base resupply extended; manpower price for shells crates at base. |
| `unit.rs`, `squad.rs` | Service-slot assignment, poses, auto-swap, `ServeGun`, abandon. |
| `spotting.rs` | Gun size; firing reveals; the optic sight bonus; the acoustic events and sound-ranging knowledge entries. |
| `path.rs` | Wheeled or towed nav (blocked by dug ground deeper than `wheel_block_dm`, pit ramps). The wheeled layer is shared with the truck and halftrack slice. |
| `dig.rs`, `terrain.rs` | Pit profiles; crater cutting scaled by filler; woods canopy height for tree bursts (optional). |
| `ai.rs` | §9. |
| `snapshot.rs`, `hash.rs`, `scenarios.rs` | All new state saved, hashed, validated on load; staged scenes for `gun-test`. |
| Presenter | Gun models, crew poses, hitched tow rendering, fire-mission UI (observed or map, rounds, ammo), range rings, sound-ranged markers, incoming-shell whistle, ammo counters on squad cards, supply readout. |

**New or changed commands (P6):**

| Command | Payload |
|---|---|
| `Emplace` / `PackUp` | gun squad, facing |
| `PushGun` | gun squad, target point |
| `Hitch` / `Unhitch` | gun, tow vehicle |
| `ServeGun` | squad, gun (recrew or capture) |
| `SetSector` | gun or squad, arc, hold-fire range (AT ambush) |
| `FireMission` (replaces `Barrage`) | firers, target, rounds, ammo, mode |
| `CallSupport` / `AdjustSupport` | support id, target, observer |
| `TransferAmmo` / `FetchAmmo` | man, source, class, count |
| `SpikeGun` (optional) | man, gun |

---

## 9. AI

| Behaviour | Rule (data-weighted) | Cost |
|---|---|---|
| Buy guns | The mix gains `mix_at_gun_pct` once an enemy vehicle has been seen (as for AT teams today). Infantry guns and mortars scale with spotted enemy trench length. | S |
| Place AT guns | Pick a spot 100–250 m behind the AI's most threatened held point, covering the likely enemy vehicle corridors (A* corridors of seen vehicles toward own points). Score: low ground on a reverse slope, woods edge, line of sight inside the gun's arc. Ask engineers for a `gun_pit_at` if idle. `SetSector` toward the threat, holding fire until `engage_still_m` (250 m) or `engage_moving_m` (150 m). | M |
| Tow and emplace | Unhitch 100–200 m short of the spot, push the rest, send the tow back or keep it as the ammo carrier. | S |
| Displace | After `displace_after_shots` shots once spotted, or when HE lands within 20 m: pack and move to the next scored spot. | S |
| Mortars | Fire only observed targets, or map-fire on trench lines in the knowledge map. Respect the ammo reserve. Pair with observers. Displace after counter-fire. | M (in G0) |
| Off-map support | Call fire on (a) trench lines in front of a point it is about to assault, timed with the assault; (b) large spotted groups attacking an own point. Needs a living FO with line of sight. | M |
| Smoke | Mortar or howitzer smoke between the assault force and the defended trench before the assault. This is the AI's answer to the trench-assault known issue. | S |
| Recrew and capture | Idle infantry within 100 m of a neutral gun serve it if the gun has rounds and points at a threat. | S |
| Spike | A gun crew about to lose its gun (enemies within 30 m, under half strength) spikes it. | S |

---

## 10. Build order (slices)

| Slice | Content | Size | Depends on | Player value |
|---|---|---|---|---|
| **G0** Mortar ammo and observation | Bombs carried per man and per class; `FireMission`; observer rule (voice link); ranging-in; map fire on known points; charge solver and true flight time; AI ammo budget; 5-man mortar squads; smoke bombs. | **M** | — (base resupply exists) | ★★★: fixes the worst balance issue and starts the ammo economy the HQ needs |
| **F0** Fragment rays | §5.7's 32 weighted fragment rays against capsules, gun anchors and vehicle anchors; near-miss suppression from fragment paths. | **M** | — | ★★: owner's physical-contact rule; makes trenches and prone matter against HE |
| **G0b** Sound ranging | Acoustic events, knowledge markers, counter-battery map fire, displacement (player and AI). | **S–M** | G0 | ★★ |
| **G1** Gun entity + AT guns | Pak 40 and 57 mm M1: entity, plates, shields, components, service slots, emplace, pack, push, `ServeGun`, abandon and capture, gun optic, tanks firing HE at guns, `gun-test` CLI. | **L** | S3 (exists); a minimal tow, or the truck/halftrack slice for towing | ★★★: the missing cheap tank counter |
| **G1b** Towing | Hitch and unhitch, wheeled nav, the tow as ammo carrier. | **M** | G1, truck/halftrack slice | ★★ |
| **G2** Mortar variants | 60 mm M2, kz. GrW 42 (and leGrW 36 if wanted); light/heavy HE on the 81 mm. | **S** | G0 | ★★ |
| **G3** Pits | Pit profile kind; `gun_pit_at`, `mortar_pit`, `howitzer_pit`; AI asks engineers to dig them. | **M** | G1; dig system | ★★★: the trench pillar |
| **G4** Infantry guns | leIG 18, 105 mm M3, 75 mm M1A1: direct and indirect fire from one entity, HEAT rounds. | **M** | G0, G1 | ★★ |
| **G5** Off-map artillery | `SupportCall`, FO teams, edge-spawned shells, adjust and fire for effect, allocation, AI preparation fire, smoke missions. | **M** | G0 (observer rule), F0 recommended | ★★★: answers the trench-assault problem |
| **G6** Heavy mortars | 12 cm GrW 42, 4.2 inch M2 (and 120 mm PM-38 with the USSR) as gun entities. | **S–M** | G1, G1b, G0 | ★★ |
| **G7** Supply hookup | Ammo crates by class, HQ production, trucks carrying stowage, bearers fetching, manpower price for every crate and refill by kind, rare-round caps. | **M** | G0, the HQ area, the truck slice | ★★★ |
| **G8** Wave 2 | sIG 33, Pak 38, 3 inch M5, 8.8 cm, tree bursts, spiking, spaced shields. | **S each** | earlier slices | ★–★★ |
| **USSR** | 45 mm, ZiS-2, ZiS-3, 82 mm, 120 mm, 76 mm regimental, M-30 battery. | **S each** (data) | the USSR faction slice | ★★★ |

**Critical path for the owner's request:** G0 → G1 (+ a minimal tow) → G3 → G5. After those
four, every requested gun type is mostly data.

---

## 11. Test and "Done when" seeds (`frontline-cli gun-test`)

1. **Shield.** An MG at 200 m from the front against a Pak 40 crew: casualties come only from
   rounds that miss the shield, and are at most 30 % of the same crew without a gun. From a
   45° flank: at least 3× the frontal casualties.
2. **AT ambush.** A Pak 40 emplaced, concealed, holding fire to 400 m, against one Sherman
   advancing: it fires first in ≥ 80 % of runs and disables the tank in ≥ 60 %. Same for a
   57 mm against a Panzer IV, flank and front. Report the hits per kill.
3. **Tank vs gun.** A Sherman that spots a Pak 40 first: HE at the crew; report the time to
   silence it, crew losses and gun components hit.
4. **Recrew and capture.** Kill the gunner: the swap takes `slot_swap_s`. Kill all the crew: the
   gun turns neutral after `abandon_s`. An enemy squad serves it, and it fires only its own
   ammo class.
5. **Observer rule.** The same 20-round mission observed and unobserved: the mean miss distance
   ratio is close to `map_fire_spread_pct`; ranging-in tightens only when observed.
6. **Mortar economy.** AI vs AI seeds 1–5 after G0: the HE kill share falls by at least a third
   from the S3 baseline (43 %). `fortify-test` still has trench casualties at most 40 % of those
   in the open.
7. **Sound ranging.** A mortar firing 10 rounds from one spot is located within 10 % of the
   listener distance. Counter-fire lands; the displaced team survives more often than the one
   that stays.
8. **Off-map.** An FO calls a leFH mission: the first round lands after `time_to_fire_s` plus
   flight time; shells enter from the own map edge; with no FO line of sight the call is
   refused. Run the case-7-style trench assault: with a preparation barrage plus smoke,
   attacker losses at most 50 % of the no-support baseline. Target number is an owner call.
9. **Determinism.** Save and load mid-push, mid-hitch, mid-fire mission, with a gun neutral and
   an off-map shell in flight; hashes match at 1, 2, 3 and 8 threads, and in net loopback.
10. **Performance.** 40 guns, 30 mortars and 2 batteries firing in the stress scene stay within
    the Battalion budgets (§2.2). Fragment rays per tick are capped and queued, never dropped.

---

## 12. Owner questions

1. **Off-map divisional artillery by default** (recommended), with on-map regimental and
   infantry guns? Or on-map divisional howitzers in skirmish too (`artillery_mode`)?
2. **Supply lever:** one per-player `supply_kg_per_min` throughput (recommended), or a manpower
   cost per restocked round? **Answered 2026-11-01:** no throughput; shells crates cost manpower
   at base, and the base refills no gun or mortar (`artillery-on-map.md` §4.2). Later the same
   day (D1): no ammunition is free, small arms and AT rounds included; (D2) no distance
   surcharge.
3. **Mortar ranges:** authentic ranges after G0 (recommended), or keep today's compressed
   600 m?
4. **Show the player when their position is sound-ranged?** **Answered 2026-11-01: hint only**
   ("N rounds from this spot: likely located", from the player's own count; the threshold is now
   3 rounds for guns, `cb_hint_rounds`). It is useful, but it leaks enemy
   knowledge. Old default: no, only an "enemy counter-fire likely" hint after their own 10th round
   from one spot.
5. **Prime movers:** do AT guns wait for the truck/halftrack slice, or does G1 ship a minimal
   unarmoured tow (wheeled box, hitch only)?
6. **Gun optic sight bonus (250 %)** together with S3 owner question 2 (vehicle spotting range):
   decide both at once.
7. **Light mortars:** drop the 5 cm leGrW 36 entirely, or keep it as a second-line option?
8. **HE-share target** after G0 and F0: is "about 20–30 % of kills" the right bar?
