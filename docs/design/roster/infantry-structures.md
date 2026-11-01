# Infantry variants, structures, ammo and supply: design proposal

Area: infantry variants and structures (Frontline RTS, `ww2` mod).
Status: proposal for owner review. Nothing here is built.
Written 2026-11-01 against the repo as of that date (read only).

Sources read: `SYSTEM_DESIGN.md` (§2, §5.3–5.12, §7, §9, §12),
`docs/slices/S1-skirmish.md`, `S1b-*.md`, `S3-armour.md`, `S3-log.md`,
`docs/TODO.md`, `sdk/crates/content/src/defs.rs`, `mods/ww2/defs/**`,
`crates/sim/src/{items,combat,ai}.rs` headers.

Historical figures marked **(verify)** are from memory. They are starting
values and must get a `sources = [...]` citation before they ship (§2.3).

---

## 0. Verdicts first (the honest pushback)

| Owner idea | Verdict | Better version |
|---|---|---|
| "Stormtroopers" with automatic rifles | **Wrong term.** *Sturmtruppen* / *Stoßtruppen* are WW1 (1916–18). | Germany: **Volksgrenadier-Sturmgruppe** (StG 44 section, from autumn 1944), **Sturmpioniere** (assault engineers). USA: **assault section** (the D-Day boat-team layout), **rifle squad with two BARs**. USSR: **avtomatchiki** (SMG section), **shturmovaya gruppa** of a ShISBr (assault engineer-sapper brigade). |
| Knives for melee | **Cut as a unit identity.** Assault infantry cleared trenches with SMGs, grenades, satchels and flamethrowers. The knife was a last resort, not a role. | Keep a **minimal close-combat strike** (bayonet, rifle butt, sharpened spade) as a physical contact sweep at ≤ 2 m, used only when a man's weapon is not ready. Build it after trench clearing works (§4). |
| Engineers vs heavy engineers who build bigger forts | **Half right.** The historical jump in building capacity came from **materials and machines** (timber, sandbags, wire, bulldozers), not from a heavier kind of man. | Split engineers by **job**, not size: **assault engineers** (breaching: Bangalore, satchels, flamethrower, mine clearing; weak builders) and **construction engineers** (bunkers, obstacles, mines; weak fighters). Big builds also need **materials delivered** (§6) and, for earthmoving, the **dozer** (§5.10.9 vehicle tier). |
| HQs and medical points that heal units | **HQ yes, as supply and command. Healing aura no.** HP regeneration is unhistorical and fights the physical-damage rule. | A **wound model** (fit / wounded / down / dead, bleed-out). **Medics stabilise** men. An **aid station staffed by real medics** treats the wounded and evacuates the downed for a manpower refund (§5). |
| HQ as a forward spawn | **No.** It undoes "spawn at base and walk", which is the stated balance lever. A forward spawn also turns every match into HQ sniping. | The HQ is a **supply dump + radio + retreat point**. Units still spawn at base. Replacements for a squad also spawn at base and walk (§7.4). |
| HQ "spawns ammo crates" | **Only from a finite stock.** Crates made from nothing at the front are a forward spawn for ammunition. | The HQ **issues** crates from its stock. Stock arrives by **truck or half-track from base**, or by men carrying crates. An optional slow **trickle** (off-map battalion supply) is a rules value, 0 in realism lobbies (§6.4). |
| HQ "helps units nearby reload" | **No reload-speed aura.** Reloading is a man's hands and the weapon's mechanism. | Read it as **refill**: men inside the supply radius restock what they have used (§6.5). |

Problems I found in today's data that this proposal fixes with historical
options, not stat changes:

1. **Rifle squads have no organic light MG.** Both armies' squads were built
   around one (MG 42 in the *Gruppe*, BAR in the US squad, DP-27 in the
   Soviet section). Today's separate "MG team" is really the heavy
   (tripod) MG section of the MG company.
2. **Squad sizes are below establishment.** German rifle *Gruppe* 1944:
   9–10 **(verify)**. US rifle squad: 12 (T/O&E 7-17). Soviet rifle section:
   about 9 **(verify)**. Today both are 8. Field strength was often lower
   (Soviet sections of 5–7 were common): offer that as a cheaper variant.
3. **Small arms, MG belts and mortar bombs are unlimited today.** Mortars
   cause 2–4× the kills of small arms (TODO, "Known issues found during
   S1"). Finite mortar ammunition is the historical lever for that, under
   the cost-balanced realism rule (§6).
4. **Tank main-gun stowage is below historical.** The Panzer IV H def
   carries 30 + 30 = 60 rounds; the Ausf. H stowed 87 **(verify)**. The
   M4A3(75)W stowed 104 (dry-stowage M4A3: 97) **(verify)**. Restore the real
   counts once field resupply exists.
5. **Concrete bunkers built by players are unrealistic** in a 30-minute
   battle (concrete cures for days). Keep concrete pillboxes (*Regelbau*,
   Soviet *DOT*) as **map or mission pieces only**. Players build
   timber-and-earth bunkers (*Holz-Erde-Bunker*, US log-and-earth bunker,
   Soviet *DZOT*).

---

## 1. What exists today (constraints for this design)

- **Squad def** (`SquadDef`): one `weapon`, `weapon_men` (first N men in
  member order carry it), `sidearm` for the rest, `men`, `cost`,
  `call_in_s`, `hp` (100), `speed_cm_s`, `sight_m`, `dig_rate_pct`, and a
  `loadout` of items with `count` / `leader_count`.
- **`SquadRole` is a hard-coded enum**: rifle, mg, engineer, mortar,
  anti_tank, vehicle. It drives which orders apply and the AI mix
  (`mix_rifle_pct`, …). A new kind of squad today means engine code.
- **Dig profiles are a fixed struct** (`trench`, `at_ditch`). The §5.10.3
  blueprint and construction-piece system is designed but not built.
- **Ammo and resupply today:**
  - small arms, MG belts, mortar bombs: unlimited (magazine reload only);
  - grenades, smoke, satchels: per-man `Kit` counts, refilled 1 per 20 s
    in the own base (`items.resupply_s`);
  - launcher rockets: `rounds_carried` (6), refilled 1 per 20 s in base;
  - tank main gun: finite per ammo type; base gives 1 round per 2 s,
    2 %/s component repair and 1 dead crew seat per 20 s.
- **Infantry damage**: 100 HP, death at 0. No wounds, no medics. Vehicle
  crew already have a `wounded_hp` threshold.
- **Owner decisions** that bind everything below: damage and suppression
  only from physical contact; no call-in queue, spawn at base and walk,
  per-type cooldown; authentic stats balanced by cost and availability;
  every gameplay value in TOML; trenches are a core pillar.

---

## 2. Rosters by faction

Conventions: *men* is establishment strength. "Lv" = build/capability
tier (§3.3). "From" = `available_from`. All weapons need weapon defs;
new ones are listed in §2.4.

### 2.1 Germany (Heer, 1944)

| Unit (in-game name) | Men | Weapons | Role | From | Notes |
|---|---|---|---|---|---|
| **Grenadiergruppe** | 9–10 (verify) | Gruppenführer MP 40; MG 42 on bipod (Schütze 1) + 2 MG assistants (Kar98k, belts); 5–6 Kar98k, maybe one G 43 | Line rifle squad built around its lMG | base | Replaces today's 8 × Kar98k. 1–2 Stielhandgranaten each; Panzerfaust optional per §2.4 |
| **Grenadiergruppe (Feldstärke)** | 6–7 | as above | Cheaper understrength variant | base | Same file via `extends`, fewer members, lower cost |
| **Volksgrenadier-Sturmgruppe** | 8–9 (verify) | StG 44 for most men; leader StG 44 | Assault section, strong at 50–150 m | 1944-09 | Two of three platoons of a VG rifle company had StG 44 (verify). Conscript training profile (§3.2) |
| **Panzergrenadiergruppe (gp.)** | 10 + vehicle | 2 MG 42 (one stays on the Sd.Kfz. 251), Kar98k, MP 40 | Mechanised squad that rides its half-track | base | Needs the vehicle area's Sd.Kfz. 251; squad + vehicle as one call-in |
| **Fallschirmjäger-Gruppe** | 9–10 | FG 42, MP 40, MG 42, Kar98k | Elite rifle squad, West only | base (Normandy) | Veteran training. Optional |
| **sMG-Gruppe** (heavy MG) | 4 in game (6 real, verify) | MG 42 on Lafette 42 tripod | Sustained fire to 600+ m | base | Today's "MG-Trupp" becomes this. Tripod: tighter spread, long setup |
| **Granatwerfertrupp 8 cm** | 3 (+ ammo bearer option) | 8 cm GrW 34 | Battalion mortar | base | Exists. Finite bombs (§6) |
| **Granatwerfertrupp 12 cm** | 4–5 | 12 cm GrW 42 | Regimental heavy mortar | base | 285 kg with carriage (verify): towed or crew-pushed slowly |
| **kurzer GrW 42 Trupp** | 3 | kurzer 8 cm GrW 42 | Light, short-range | base | Paratroop and assault use. Optional |
| **Panzerschrecktrupp** | 2 | RPzB 54 + Kar98k | Infantry AT | base | Exists |
| **Pioniergruppe** | 8–9 (game 5 today) | MP 40, Kar98k, satchels, Teller mines, wire cutters | Field engineer: digs and builds everything at Lv 2 | base | Exists, renamed by job |
| **Sturmpioniergruppe** | 8 | MP 40 / StG 44, Flammenwerfer 41 (2-man pair inside the squad), gestreckte Ladung (Bangalore), 2–3 satchels | Breaching: wire, mines, bunkers, trench clearing | base | Builds only Lv 1 + wire/mine clearing; fights well close in |
| **Baupioniergruppe** (construction) | 8 | Kar98k | Builds bunkers, dugouts, obstacles, minefields faster (Lv 3) | base | Older, poorly trained men. Weak in combat. Historically rear-area construction troops: keep rare and cheap |
| **Scharfschützentrupp** | 2 | Kar98k + ZF 39 (or G 43 + ZF 4), spotter with binoculars | Long-range precision, targets leaders, gunners, crews | base | Optic feeds spotting (§5.12.5) |
| **Zugtrupp** (platoon HQ) | 3–4 | Zugführer MP 40, Funker with portable radio, messenger | Radio link, rally, can site an HQ | base | Officer and radio traits (§3.4) |
| **VB-Trupp** (forward observer) | 2–3 | binoculars, radio | Calls on-map and off-map artillery | base | Shared with the artillery area |
| **Sanitätstrupp** | 2–3 | P 38 or unarmed, stretcher | Stabilise and carry wounded, staff the aid post | base | See §5 |

Do not ship: **Waffen-SS** units (their insignia fall under StGB §86a and
the design is Heer-only), the **5 cm leGrW 36** (withdrawn from the front
line before 1944, verify).

### 2.2 USA (US Army, 1944)

| Unit | Men | Weapons | Role | From | Notes |
|---|---|---|---|---|---|
| **Rifle squad** | 12 | M1 Garand ×10–11, BAR team (BAR + assistant + ammo bearer), one rifleman with M7 grenade launcher | Line rifle squad | base | T/O&E 7-17 (1944). Replaces today's 8 × M1 |
| **Rifle squad (understrength)** | 8–9 | as above | Cheaper variant, typical by autumn 1944 | base | `extends` |
| **Rifle squad, two BARs** | 12 | as above + 2nd BAR | More automatic fire, a common field practice late 1944 (verify) | 1944-08 | The US "more automatic weapons" variant, without Thompson fantasy |
| **Assault section** | 12–15 in game (D-Day boat teams were about 30, verify) | M1, 1 BAR team, 1 bazooka, M2-2 flamethrower, 2 demolition men (pole charges, Bangalore M1A1), wire cutters | Breaching fortified lines | 1944-06, West | The genuine US assault squad |
| **Armored infantry squad** | 12 + M3 half-track | M1, BAR, bazooka in vehicle, M1919A4 + M2 .50 on vehicle | Mechanised | base | Needs the vehicle area's M3 |
| **Airborne rifle squad** | 12 | M1, M1A1 carbine, M1A1 Thompson for leaders, light MG at platoon | Elite, Normandy / Market Garden / Bastogne | base | Veteran training |
| **Light MG section** | 3–4 | M1919A4 on tripod | Company weapons platoon | base | Today's "MG team" |
| **Heavy MG section** | 4–5 | M1917A1 water-cooled | Battalion heavy weapons company: long sustained fire | base | Optional |
| **60 mm mortar squad** | 3 | M2 60 mm | Company mortar | base | |
| **81 mm mortar squad** | 3–4 | M1 81 mm | Battalion mortar | base | Exists |
| **4.2-inch mortar squad** | 5 | M2 4.2-in (HE, WP) | Chemical mortar battalion: heavy, smoke and WP | base | Needs a vehicle or long setup |
| **Bazooka team** | 2 | M1A1 / M9 / M9A1 | Infantry AT | base | Exists |
| **Engineer squad** | 12 (game 5 today) | M1, carbines, demolitions, Bangalore, SCR-625 mine detector, mines | Field engineer, Lv 2 | base | Exists |
| **Engineer dozer** | vehicle | D7 or M4 dozer | Earthmoving Lv V | base | The US "heavy engineer" is the machine (vehicle area) |
| **Sniper team** | 2 | M1903A4 (M73B1 scope); M1C from late 1944 in small numbers (verify) | Precision | base | |
| **Platoon HQ** | 3–4 | Lt and platoon sergeant (M1, carbine), SCR-536 radio | Radio, rally, HQ | base | |
| **Artillery FO party** | 3 | carbines, SCR-610 / SCR-300 radio (verify) | Calls artillery | base | Shared with artillery area |
| **Aid men** | 2 | unarmed (US practice) | Medics | base | See §5 |

### 2.3 USSR (Red Army, 1944): not built yet

| Unit (in-game name) | Men | Weapons | Role | From | Notes |
|---|---|---|---|---|---|
| **Strelkovoe otdelenie** (rifle section) | 9 (verify) | Mosin M91/30 or M1944 carbine, DP-27 LMG team, 1–2 PPSh-41 | Line rifle section | base | Bayonets always fixed (relevant to melee, §4) |
| **Rifle section (field strength)** | 6–7 | as above | Typical 1944 strength, cheap | base | `extends` |
| **Gvardeyskoe otdelenie** (Guards rifle section) | 9 | as above, more PPSh | Better trained | base | "Guards" is a unit honour: a training variant, not a new weapon set |
| **Otdelenie avtomatchikov** (SMG section) | 7–9 | PPSh-41 / PPS-43 | Assault and tank riders (*tankovy desant*) | base | From the regiment's SMG company |
| **Shturmovaya gruppa** (ShISBr assault sappers) | 8 | PPSh, SN-42 steel breastplate, ROKS-3 flamethrower pair, satchels, grenades | Breaching fortified lines and towns | base | The real Soviet "heavy" assault engineer. The breastplate is a physical plate in the hit model (§3.5) |
| **Sapyornoe otdelenie** (sappers) | 7–9 | Mosin, PPSh, TM-41 / PMD-6 mines | Field engineer, Lv 2 | base | |
| **Pulemyotny raschyot** (HMG crew) | 4 | Maxim M1910/30 or SG-43 Goryunov | Sustained fire | base | |
| **82 mm mortar crew** | 3–4 | 82-BM-37 / BM-41 / BM-43 | Battalion mortar | base | |
| **120 mm mortar crew** | 5 | PM-38 / PM-43 | Regimental heavy mortar | base | |
| **PTR team** | 2 | PTRD-41 or PTRS-41 | AT rifle: only vs half-tracks, armoured cars, thin sides | base | Physical penetration decides; no stat fudging. Captured Panzerfausts as a rare item (*trofeyny*) |
| **Snaiperskaya para** (sniper pair) | 2 | Mosin 91/30 with PU scope; SVT-40 sniper | Precision | base | |
| **Upravlenie vzvoda** (platoon HQ) | 3 | PPSh, field telephone or RBM radio (scarce) | Rally, comms | base | Radios were scarce at company level: wire telephone is an optional later detail |
| **Sanitary** (medics) | 2 | unarmed or pistol | Medics | base | Many Soviet medics were women; voice and models should reflect that |
| **ROKS-3 team** | 2 | ROKS-3 | Flamethrower, from the separate flamethrower companies | base | |

Do not ship: **penal units** (*shtrafbaty*): contested history, little play
value, high controversy.

### 2.4 New weapon and item defs this roster needs

Small arms: StG 44, G 43 (+ ZF 4), FG 42, Kar98k ZF 39, P 38 (sidearm),
BAR M1918A2, M1A1 Thompson, M1 carbine, M1903A4, M1917A1, Mosin M91/30 (+ PU),
M1944 carbine, SVT-40, PPSh-41, PPS-43, DP-27, Maxim M1910/30, SG-43,
PTRD-41 / PTRS-41.
Indirect: kurzer GrW 42, 12 cm GrW 42, 60 mm M2, 4.2-in M2, 82 mm BM,
120 mm PM-38.
Items: **Panzerfaust 30 / 60** (single-use launcher as a kit item, like a
grenade with a HEAT warhead; Pzf 60 from about Sept 1944, verify), M7 rifle
grenade launcher with M9A1 AT and frag rifle grenades, F-1 and RGD-33
grenades, Bangalore torpedo / *gestreckte Ladung*, mines (Teller 42/43,
S-Mine 35/44, Schü-Mine 42, M1A1, M2, TM-41, PMD-6), wire cutters, mine
detectors, flamethrowers (FmW 41, M2-2, ROKS-3), SN-42 breastplate.

---

## 3. Variants as data

Goal: a variant is **one short TOML file**; new code only for new
mechanics.

### 3.1 Squad file: members instead of one weapon

```toml
# defs/units/ger_vg_sturmgruppe.toml
id       = "ww2:ger_vg_sturmgruppe"
extends  = "ww2:ger_grenadiergruppe"   # inherit every field not set here
name     = "@loc:unit.ger_vg_sturmgruppe"
available_from = "1944-09"
training = "ww2:training_conscript"     # §3.2
tags     = ["infantry", "assault"]       # AI mix and HUD filters; not engine roles
cost     = 300
call_in_cooldown_s = 60                  # owner rule: per-type cooldown, no queue
sources  = ["..."]

# Replaces the parent's member list (arrays replace, tables merge).
[[members]]
slot   = "leader"
count  = 1
weapon = "ww2:stg44"
ammo   = { "ww2:792k" = 180 }            # rounds carried, by ammo id (§6)
items  = { "ww2:stielhandgranate" = 2, "ww2:nbhgr39" = 1 }
traits = ["leader"]

[[members]]
slot   = "rifleman"
count  = 7
weapon = "ww2:stg44"
ammo   = { "ww2:792k" = 180 }
items  = { "ww2:stielhandgranate" = 2 }
```

```toml
# defs/units/ger_sturmpioniergruppe.toml (excerpt)
extends      = "ww2:ger_pioniergruppe"
capabilities = ["dig_basic", "place_wire", "clear_wire", "clear_mines", "demolish"]
build_rate_pct = 100

[[members]]
slot = "flame"
count = 1
weapon = "ww2:fmw41"
ammo = { "ww2:fmw41_fuel" = 7 }          # litres (verify)
[[members]]
slot = "flame_assistant"
count = 1
weapon = "ww2:mp40"
traits = ["ammo_bearer"]                 # carries and hands over fuel / belts
```

Rules for the loader (content crate only, no sim code):
- `extends` chains up to 4 deep; cycles are an error. Tables merge,
  arrays replace. The resolved def is validated as a whole.
- `members` replaces `men` / `weapon` / `weapon_men` / `sidearm` /
  `loadout`. The old fields stay as sugar that expands into one or two
  member slots, so today's files keep loading.
- Member order is the order men stand in at spawn. When the man holding
  a crew weapon (MG, launcher, flamethrower, mortar) falls, the nearest
  living man in that squad picks it up after the weapon's `handover_ms`
  (generalising today's launcher handover). Today's rule, "the first
  `weapon_men` men in member order", silently passes the MG to whoever is
  first in line; that should become an explicit pickup.

### 3.2 Training profiles (instead of in-match veterancy)

```toml
# defs/training/veteran.toml
id = "ww2:training_veteran"
spread_pct            = 85    # aim error multiplier on the weapon's spread
aim_time_pct          = 80    # time to open fire
reload_pct            = 90
suppression_gain_pct  = 75    # near-miss suppression taken
suppression_decay_pct = 125
pin_threshold_add     = 10    # added to rules.combat.pin_suppression
spot_pct              = 110
throw_scatter_pct     = 80
build_rate_pct        = 100
```

Profiles: `conscript`, `regular` (default), `veteran`, `elite`. They
change the **man**, never the weapon or armour stats, so authenticity holds.

**In-match veterancy: defer.** It is a CoH-style reward loop, it hides
balance behind kill counts, and the persistent campaign (§17 Q5, the only
place where veterancy is historically meaningful) is post-launch. If
wanted later, it is one more field (`training_after_kills = [[10,
"ww2:training_veteran"]]`) on top of the same profiles. Cost S once
profiles exist.

### 3.3 Capabilities replace the role enum

§5.10.9 already says capabilities are data. Extend the list:

| Tier | Capability strings | Who (default) |
|---|---|---|
| Lv 1 basic | `dig_basic` (scrape, foxhole, slit trench, short trench) | any infantry with an entrenching tool |
| Lv 2 field | `dig_full`, `build_structure` (MG nest, dugout, sandbag wall), `place_wire`, `place_mines`, `demolish`, `build_command`, `build_supply` | Pioniere, US engineers, sappers |
| Lv 2 assault | `clear_wire`, `clear_mines`, `breach` (Bangalore, pole charge), `flame` | Sturmpioniere, assault section, ShISBr |
| Lv 3 construction | `build_heavy` (timber-earth bunker, AT obstacles, minefield patterns), at a higher rate | Baupioniere; US and Soviet engineers with a materials truck |
| Lv V vehicle | `dig_heavy`, `fill`, `flatten` | dozers (§5.10.9) |
| Support | `medic`, `build_medical`, `radio`, `observer` | medics, platoon HQs, FOs |

Orders follow from what a squad has: a mortar weapon gives `Barrage`, any
`dig_*` gives `Dig`, `medic` gives `Treat`, and so on. The AI buys by
`tags` with a data map (`ai.mix = { line = 45, mg = 15, engineer = 15,
mortar = 10, assault = 10, at = 5 }`) instead of `mix_rifle_pct` fields.

### 3.4 Traits on men

`leader`, `officer` (rally: men within `rally_m` 30 m decay suppression
`rally_decay_pct` faster; small and visible on the HUD), `radio` (a link
for support calls, §7.2), `medic`, `ammo_bearer` (carries extra rounds
for the crew weapon and hands them over, §6.5), `sniper` (target priority
list: officers, MG gunners, crew heads above hatches).

The officer rally is the only non-physical effect in this document. It
does not cause or remove damage or suppression; it only speeds recovery.
If the owner wants zero abstract effects, cut it.

### 3.5 Body armour (SN-42)

A breastplate is an **armour plate on the torso part of the hit capsule**
(`armour = { zone = "torso_front", thickness_mm = 2, material = "steel" }`,
verify). Small-arms rounds and fragments that touch it run the normal
penetration check. Real effect: it stops many pistol and SMG rounds and
fragments, not rifle rounds at close range (verify). Needs hit zones on
the capsule (§5.2), cost S after that.

### 3.6 What is data and what is code

| Change | Data only? | Code | Size |
|---|---|---|---|
| `extends` inheritance | yes (content crate) | resolver + cycle check | S |
| `members` slots, per-man weapon, ammo, items, traits | no | `Kit` gains ammo per id; `weapon_of` reads the slot; crew-weapon pickup | M |
| Training profiles | yes after code | modifiers in aim, suppression, spotting, build | S–M |
| Capabilities / tags replace `SquadRole` | no | order gating and the AI mix read data | M |
| `available_from/to`, `sources` on squads | yes | date filter in the call-in list | S |
| Snipers, officers, radio | mostly | target priority, rally, radio link | S–M |
| Flamethrower | no | new projectile kind (§4.3) | L |
| Body armour | no | capsule hit zones + plate check | S–M |
| Every variant after that | **yes: one file** | none | — |

---

## 4. Melee and close combat

### 4.1 Is it worth it?

Not as a headline. The open S1b issue is **getting to the trench alive**
(defenders see attackers at 150 m and lose nobody to rifle fire). Melee
does nothing until a man stands at the parapet. Once he is there, a
grenade or an SMG burst already decides it. Historical trench clearing
was grenades first, then SMGs and pistols along the trench; bayonets and
sharpened spades were for when weapons were empty or jammed.

What actually fixes the trench assault, in order: assault variants with
SMGs and StG 44s (§2), mortar smoke, flamethrowers, finite defender
ammunition (§6), and a "clear trench" order (throw ahead at each bend,
then advance). Those are worth building before melee.

### 4.2 Recommendation: minimal, physical, late

Keep a **close-combat strike** and build it after the items above:
- **Physical:** a strike is a short swept segment from the weapon (or
  hand) along the man's facing: bayonet thrust 1.8 m reach, rifle butt
  1.0 m, spade or knife 0.6 m (data, `defs/weapons/*_melee.toml`). It
  tests enemy capsules exactly like a bullet's substep, with no flight
  time. No hit, no damage. Wind-up 0.4–0.8 s, recovery 1 s (data).
- **When men choose it (AI and RTS control):** an enemy within the
  weapon's reach **and** the man's weapon cannot fire within the wind-up
  (empty, reloading a bolt or clip, a crew weapon). Otherwise he shoots:
  at 2 m an SMG burst beats a bayonet.
- **Direct control:** `V` (already bound in §5.12.2).
- **Damage:** data per strike (a bayonet thrust to the torso is usually
  lethal or "down", §5). No suppression from strikes; a man who sees a
  comrade struck next to him is near-missed only if the swing passed
  within `near_miss_cm` of him (physical).
- **Soviet sections** keep bayonets fixed (accuracy penalty in their
  weapon def, verify), so they strike with full reach; Germans and
  Americans fix bayonets only on an order (`FixBayonets`, 3 s).

Cost **M** (sim: a strike request in the Weapons stage reusing the capsule
test; AI: the choice rule; presenter: strike animations are the larger
part). Value: low to medium. **Keep, but schedule after trench clearing,
flamethrowers and the ammo model.** Cut "knives" as a squad feature.

### 4.3 Flamethrowers (the real trench-clearing weapon)

Under the physical-contact rule a flame stream is **many short-lived
projectiles** (e.g. 12 per second of trigger) on a ballistic arc with
drag, range 25–40 m (FmW 41 about 25–30 m, ROKS-3 about 35–40 m, M2-2
20–40 m, verify). Each particle that touches a capsule sets the man
burning (damage over time while he stays in contact with fire). Each
particle that lands leaves a **burning patch** (radius 1 m, 10–20 s,
data) that harms whoever stands in it. Particles pass through openings
(embrasures, doors, §5.12.3), so bunkers fall to flame. A particle
passing within `near_miss_cm` suppresses heavily (data per weapon), which
is the historical "fear" of flame done physically. Fuel is ammunition: 7–18
litres, about 8–10 s of burst (verify), refilled only from a source that
stocks it. The operator's tank is a hit zone; a hit can ignite it.

Cost **L** (projectile pool kind, burning-patch list saved and hashed,
burn status, presenter effects). Value **high** for the trench pillar.
Dependencies: §6 ammo, hit zones.

---

## 5. Wounded and healing

### 5.1 Options

| | Simple HP regen | Realistic wound states |
|---|---|---|
| Historical | No | Yes |
| Physical-contact rule | Healing without cause is the same kind of abstraction as damage without contact | Wounds come only from hits; recovery comes from people doing work |
| Player value | Low: invisible, encourages "sit back and heal" | High: medics, aid posts, carrying wounded, real decisions |
| Cost | S | L (phased) |

**Recommendation: wound states, no regeneration.**

### 5.2 The model

**Hit zones.** The capsule gets three zones by height fraction (data):
head (top 12 %), torso, legs. Damage per hit = weapon damage × zone
factor (head 300 %, torso 100 %, legs 60 %, data).

**States** (per soldier, saved and hashed):

| State | Entered when | Effect |
|---|---|---|
| **Fit** | spawn | normal |
| **Wounded** (light) | HP ≤ `wounded_hp` (50) and > 0 | aim spread +30 %, speed −30 % (legs: −60 %), reload +25 %; **bleeding** −1 HP per 4 s until bandaged; he keeps fighting |
| **Down** (incapacitated) | HP ≤ 0, and not killed outright | prone, cannot fight or move; counts for nothing (capture, squad strength); **bleed-out timer** 60–180 s (seeded per wound, data) |
| **Dead** | head hit for ≥ `head_kill_dmg`, overkill ≥ `overkill_kill` (e.g. 60), a blast's lethal core, or the bleed-out timer runs out | today's corpse rules |

Downed men are still bodies on the ground: fragments and rounds that
touch them can kill them. The AI does not aim at downed men (target
filter), human players can but gain nothing.

**Treatment** (all timed work by real men, interrupted by moving or
being pinned):

| Action | By | Time (data) | Result |
|---|---|---|---|
| Self or buddy bandage | the man, or any squadmate within 2 m | 10 s | stops a light wound's bleeding |
| Stabilise | a medic within 2 m of a downed man | 15 s | stops the bleed-out timer; he stays down |
| Carry | 1 man drags (speed 30 %) or 2 men with a stretcher (speed 60 %) | — | moves a downed man |
| Treat | aid station, staffed (§7.3) | 60 s per man, `beds` at once | a wounded man returns to Fit at `return_hp_pct` (75 %) of max HP and walks back to his squad |
| Evacuate | aid station or own base | 30 s | a stabilised downed man leaves the field; the player gets `evac_refund_pct` (40 %) of that man's share of the squad cost |

This is how "aid stations return men at partial strength" works without
a healing aura: light wounds come back weaker, heavy wounds come back as
manpower, never as a man who pops up at the front.

### 5.3 Rules data

```toml
[wounds]
zones = { head_top_pct = 12, legs_bottom_pct = 45 }
zone_damage_pct = { head = 300, torso = 100, legs = 60 }
wounded_hp = 50
bleed_ms_per_hp = 4000   # integers only: the content crate lowers to fixed point
down_bleed_s = [60, 180]
overkill_kill = 60
head_kill_dmg = 100
bandage_s = 10
stabilise_s = 15
drag_speed_pct = 30
stretcher_speed_pct = 60
treat_s = 60
return_hp_pct = 75
evac_s = 30
evac_refund_pct = 40
```

### 5.4 Medics and the red cross

- Medics carry `medic` and `build_medical`. US aid men were unarmed;
  German and Soviet medics sometimes carried a pistol (data per unit).
- No in-sim protection for medics (the AI may deprioritise them by a
  data target-priority list; physics treats them like anyone).
- **Legal note:** the red cross emblem is protected under the Geneva
  Conventions and national law, and the ICRC has asked game makers not to
  use it on game items. Use a plain white armband, a stretcher, or another
  marker unless a legal check clears the historical armband. Same treatment
  as the §2.3 symbol validator.

### 5.5 Cost and phasing

- Phase A (**M**): hit zones, Fit / Wounded / Down / Dead, bleed-out, self
  and buddy bandage. Already useful: squads lose men more slowly and
  recover some.
- Phase B (**M**): medics (stabilise, carry), AI medics.
- Phase C (**M**): aid station (needs the structure framework, §7) and
  evacuation refund.
- Systems: Damage (wound roll), Status (bleed, timers), Intent (treat and
  carry orders), AI, snapshot (state per man), HUD (roster pips get wound
  states).

---

## 6. Ammunition and supply model

### 6.1 Principles

> **Owner decision Q4 (2026-11-01).** No per-player shell ration. Each
> gun and mortar has a ready inventory that refills only from crates
> nearby or from an HQ or dump radius. The base zone does not refill
> shells or mortar bombs (mortars from S6, when crates exist). Shells
> crates **cost manpower** when taken at base (`shell_mp_per_kg`, start
> 1.0); small-arms and explosives crates stay free. Full rule in
> `artillery-on-map.md` §4.2; roadmap "Owner decisions". The text below
> is updated to match.

- **Everything a man, gun or vehicle fires is carried, counted and runs
  out.** Rounds by ammo id, items by count.
- **Supply moves physically.** Base → truck or half-track → HQ / dump →
  crates or the supply radius → men. Each link is a real entity that can
  be shot.
- **Small arms and explosives cost no currency.** They cost **time and
  transport**, not manpower, so "the walk is the balance" also holds for
  them. The vehicles and HQ cost manpower; that cargo from base is free.
- **Shells are the exception.** Mortar bombs, gun and howitzer shells,
  artillery rockets and HE rifle grenades (`crate_only_classes`) leave
  the base only in crates or vehicle loads, at `shell_mp_per_kg` per kg.
  They are the HE that dominates kills today, so they compete with men
  for manpower.
- **The base is just an infinite source** for everything else. Today's
  base resupply becomes the first instance of the general mechanism.

### 6.2 What runs out (starting values, all verify)

| Weapon / item | Carried (per man unless stated) | Runs out after (game fire rates) | Notes |
|---|---|---|---|
| Kar98k / Mosin | 60–90 rounds | 4–6 min of steady fire | 2 pouches of 30 + a bandolier |
| M1 Garand | 80 + 48–96 in bandoliers | 6–10 min | |
| MP 40 / M3 / Thompson | 180–200 | 1–2 min of bursts | 6 magazines |
| StG 44 | 180 | 2–3 min | 6 × 30 |
| PPSh-41 | 2 drums (142) + boxes | 1–2 min | |
| BAR | 240–260 (team) | 2 min | gunner + bearer |
| DP-27 | about 9 pans (430) | 2–3 min | team |
| MG 42 / M1919A4 / Maxim | 1,200–1,500 (team) | 1–2 min continuous; 5–8 min in bursts | **the most important limit** |
| 8 cm / 81 mm / 82 mm mortar | 12–18 bombs (team; +6 per ammo bearer) | 2–3 barrages | the fix for mortar dominance |
| 12 cm / 120 mm / 4.2-in mortar | 8–12 | 1–2 barrages | needs a vehicle for more |
| Panzerschreck / Bazooka | 5–6 rockets | — | today 6, keep |
| Panzerfaust | 0–1 per rifleman (variant data) | single use | |
| Grenades, smoke, satchels | today's loadouts (+1 frag per rifleman to match history, verify) | — | |
| Flamethrower fuel | 7–18 L | 8–10 s of bursts | §4.3 |
| Tank main gun | historical stowage (Pz IV H 87, M4A3(75)W 104, verify) | rarely in 30 min | restore once field resupply exists |
| Tank and vehicle MGs | historical (Pz IV: 3,150, verify) | long | |

Small-arms numbers are per man, so a squad that has fought for five
minutes at 150 m is visibly running low, and the MG goes quiet first.
That is historical and creates the need for the supply chain.

**Burst bundles.** When small-arms rounds beyond the pool quota merge
into a burst bundle (§5.7), the bundle must subtract all N rounds from
the shooter's count. Note this in the implementing slice.

**Fire discipline.** With finite rounds, the AI and RTS-controlled men
need a rule: hold fire beyond `effective_range_pct` (e.g. 70 %) of weapon
range unless ordered "fire at will"; MGs fire bursts with pauses (data
`burst_rounds`, `burst_pause_ms`). Without this, AI squads empty
themselves at targets they cannot hit.

### 6.3 Supply categories and weights

Each ammo or item def gains a supply entry:

```toml
# in defs/ammo/792k.toml (new: small-arms ammunition gets its own def)
id = "ww2:792k"
name = "7.92×57 mm"
supply = { kind = "small_arms", g = 27 }      # grams, integer (verify)

# in defs/items/stielhandgranate.toml
supply = { kind = "explosives", g = 600 }       # verify

# in defs/ammo/81mm_he.toml (mortar bombs become ammo defs)
supply = { kind = "shells", g = 3500 }           # verify
```

Four kinds: `small_arms`, `explosives` (grenades, satchels, rockets,
Panzerfausts, mines, flame fuel), `shells` (mortar, gun, tank),
`construction` (timber, sandbags, wire, pickets). Sources stock **kg per
kind**. Within a kind, any calibre can be drawn: a crate of "small arms"
holds the faction's small-arms mix. That is a simplification (real crates
were by calibre) that keeps logistics readable. A rules flag
`supply_by_calibre = false` reserves the strict version.

Cross-faction: captured stock serves only weapons of its faction's
calibres; Soviet troops using captured Panzerfausts draw from captured
`explosives` only.

### 6.4 Sources

One component, `SupplySource`, on several kinds of entity:

```toml
# defs/supply/ger_hq.toml
id = "ww2:supply_hq"
stock_kg = { small_arms = 0, explosives = 0, shells = 0, construction = 0 }  # starts empty
cap_kg   = { small_arms = 300, explosives = 200, shells = 600, construction = 600 }
radius_m = 25
serves   = ["small_arms", "explosives", "shells"]
max_clients = 6           # men served at once
trickle_kg_per_min = { small_arms = 0 }   # rules may override (see below)
issue_crates = ["ww2:crate_small_arms", "ww2:crate_shells", "ww2:crate_explosives"]
```

| Source | Stock | Radius | Serves | Notes |
|---|---|---|---|---|
| **Own base zone** | infinite | base radius | men, vehicle rearm, repair, crew; **not** guns or mortars (Q4) | Today's behaviour, now one instance of the rule. Shells crates issued here cost manpower |
| **HQ / command post** | finite, cap from def | 25 m | men + crate issue | §7.2 |
| **Ammo dump** | finite | 15 m | men + crate issue | Cheaper, no radio; any number (limit in data) |
| **Supply truck** (Opel Blitz, GMC CCKW, Studebaker US6) | finite, large (1–2 t, verify) | 8 m | men, vehicles, unloads to HQ/dump | Soft-skinned: rifle rounds that touch the cargo bed can set off `shells` stock (physical) |
| **Half-track** (Sd.Kfz. 251, M3) | finite, small (300–500 kg) | 8 m | men, vehicles | Armoured: can resupply under fire |
| **Crate** | finite, small (20–35 kg) | 5 m | men of any squad | Carried or placed (§6.6) |
| **Squadmate** | his own surplus | 3 m | same squad, same calibre | Automatic sharing (§6.5) |

**Trickle (owner choice):**
- (A) **No trickle.** HQ stock comes only from trucks, half-tracks and
  carried crates. Purest; the AI must run trucks.
- (B) **Small trickle** (e.g. 10 kg/min small arms, 15 kg/min shells,
  data) standing for off-map battalion supply runs. Keeps the HQ useful
  before trucks exist and makes the AI simpler.
- **Recommendation:** ship (B) first, because trucks belong to the
  vehicle area and come later. Keep `trickle = 0` in a "realism" rules
  file, and move the default to (A) once trucks and AI logistics work.

### 6.5 The resupply radius

Status stage, serial, **ascending source id, then ascending man id**
(deterministic). Every `service_tick_ms` (500 ms):
- A man is served if he is inside the radius, alive, Fit or Wounded,
  not pinned, has **not fired in the last 3 s** (data), and is below his
  loadout in something the source has in stock.
- Each served man takes the **most depleted** thing first (lowest
  fraction of loadout), at a per-kind rate (data): small arms 30 rounds
  per second, 1 grenade or Panzerfaust per 3 s, 1 rocket per 5 s,
  1 mortar bomb per 2 s, flame fuel 1 L per 2 s. The kg is taken from the
  source.
- At most `max_clients` men per source per service tick, round-robin by
  a saved cursor so a big crowd is served fairly.
- **Crew weapons:** a mortar or gun refills its ready inventory if it
  (or its crew) is in an HQ or dump radius, **or** a crate lies within
  5 m of it (an ammo bearer drops it at the pit). This is how mortars
  were really fed. The base zone does not feed them (Q4).
- **Vehicles:** a vehicle within `vehicle_radius_m` of a truck,
  half-track, dump, HQ or the base gets 1 main-gun round per 2 s
  (today's `base_rearm_s`) and MG belts at 50 rounds per second, from
  `shells` / `small_arms` stock. Repair and crew refill stay **base-only**
  until a repair vehicle exists (vehicle area).
- **Squad sharing:** every 5 s, inside one squad, within 3 m, same
  calibre: the man with the most rounds (above 50 % of his loadout)
  passes 10 % of his loadout to the man with the least. Men with
  `ammo_bearer` give first. Riflemen in a German *Gruppe* carry MG belts
  for the MG (historical: the whole squad fed the gun).

The base keeps its existing timings as data on its source def
(`items.resupply_s` becomes the base source's item rate), so today's
tests stay meaningful.

### 6.6 Crates

- **Made at** an HQ or dump: `IssueCrate { source, crate_def }`. The crate
  appears next to the source after `issue_s` (10 s) and its kg leave the
  source's stock. Crates can also be issued at base (infinite stock), or
  unloaded from a truck. At base, small-arms and explosives crates are
  free; a shells crate costs `shell_mp_per_kg` × its kg in manpower (a
  32 kg crate: 32 at the start value). Loading shells onto a vehicle at
  base costs the same per kg.
- **Carried by men:** `CarryCrate` / `PlaceCrate`. One man carries a light
  crate (≤ 25 kg) at 80 % speed; a shells crate (30–35 kg) at 60 %, or two
  men at 85 % (data). A carrier cannot fire his main weapon; he can drop
  the crate in 1 s. Any squad can carry one; a dedicated **ammo carrier
  pair** (*Munitionsträger*) is a cheap unit option.
- **Delivered by vehicles:** a truck or half-track carries N crates or
  bulk kg; `Unload` at a point puts crates on the ground or fills an
  HQ/dump.
- **Physical damage:** a crate is a box with a hit volume and HP. Rounds
  and fragments that touch it damage it. A `shells` or `explosives` crate
  destroyed by HE or fire **cooks off** (an HE burst from its def, scaled
  to its remaining kg). A `small_arms` crate is just lost. No damage
  without contact.
- **Fog of war:** enemy crates are seen like any object (spotting rays).
- **Cap:** at most `crate_cap` per player (32), oldest empty first; of
  these at most `shell_crate_cap` (8) shells crates.

```toml
# defs/supply/crate_shells.toml
id = "ww2:crate_shells"
kind = "shells"
kg = 32
carry_men = 1
carry_speed_pct = 60
hp = 60
cook_off = { damage = 150, lethal_radius_cm = 250, damage_radius_cm = 700, suppression_radius_m = 15 }
# Price at base: kg x [supply] shell_mp_per_kg in rules (Q4); 0 for small-arms crates
```

### 6.7 How this fits today's base resupply

| Today | After |
|---|---|
| Items and rockets refill 1 per 20 s in base | Base is a `SupplySource` with infinite stock and those rates; HQ, dump, crates and vehicles use the same code with finite stock |
| Small arms, belts unlimited | Finite per man (§6.2); base refills them like items |
| Mortar bombs unlimited | Finite per man. S5: base refills them like items. From S6: the base refills none; priced crates, HQ and dumps only (Q4) |
| Tanks rearm in base, 1 round per 2 s | Same rate at any source with `shells` stock in vehicle range |
| Repair and crew refill in base only | Unchanged (repair truck later) |
| `Kit.items[ITEM_KINDS]` per soldier | `Kit` gains `rounds: SmallVec<[(AmmoId, u16); 2]>`; saved and hashed |

The risk is balance, not code: once rounds run out, a 30-minute AI-vs-AI
match may stall. Gate it with a test (§9).

### 6.8 AI and the ammo model

- **Squad ammo state:** the planner reads each squad's fraction of
  loadout (min over men of the primary weapon, plus the crew weapon).
- **Resupply:** a squad below `ai.resupply_pct` (30 %) with no spotted
  enemy within `threat_radius` goes to the nearest source with stock (HQ,
  dump, crate, truck), base only if nothing nearer. It returns to its task
  afterwards. Mortar teams below one barrage move to the HQ or ask for a
  crate.
- **Fire discipline:** mortar barrages only on targets scoring above
  `ai.barrage_min_score`, and `barrage_rounds` scales with stock. This
  alone should cut the mortar kill share the TODO complains about.
- **Logistics:** keep an HQ stocked: when stock < 40 % and the AI has a
  truck or half-track, send it base → HQ → base. Buy a truck when there
  is an HQ and none alive. With trickle (B), skip trucks on Easy.
- **Crates:** an engineer or a cheap carrier pair takes a shells crate to
  each mortar position; riflemen carry a small-arms crate to a trench
  under sustained fire. Shells crates come from the HQ or a dump first;
  from base only if none, since they cost manpower there
  (`ai.shell_mp_max_pct`, 25 % of income at most).
- Budget: all of this is serial planning every 2 s per AI player, a few
  hundred operations; no new rays except the HQ siting probe (§7.2).

---

## 7. Structures

All structures are **blueprints** (§5.10.3): digs + construction pieces
with armour faces and HP. They get protection only from their geometry
and are destroyed only by things that touch them (rounds, fragments,
blast rays, flame particles, satchels placed against them). Today none of
this exists beyond the trench and AT ditch, so the **structure framework
is the main dependency** (§8).

### 7.1 Placement rules (data, per structure)

```toml
[placement]
limit_per_player     = 1        # HQ: 1; dumps: 3; aid stations: 2
min_spacing_m        = 40
max_slope_pct        = 25
forbidden_materials  = ["rock", "concrete", "road"]
needs_flat           = true      # auto-Flatten (§5.10.6)
max_from_own_point_m = 0         # 0 = no limit (§5.10.9 "no hidden rules"); modes may set one
```

Builders must physically reach the site (§5.10.9). Materials come from
carried `construction` kg (each engineer carries some), a crate, or a
truck within 15 m. Building under fire is slow (×0.3 suppressed, stops
pinned), as today.

### 7.2 HQ / command post

Names: *Gefechtsstand* (DE), command post / CP (US), *komandny punkt, KP*
(SU).

**What it is:** a small dugout (2 m pit, timber roof, 0.5 m earth on top)
with a **radio set** (component inside, HP 40) and a **supply stock**
(stored inside the dugout). Built by squads with `build_command` (field
engineers, platoon HQ) in about 3 min with a 5-man engineer squad
(compressed realism), 120 kg `construction`, plus `cost_mp` 100
(stands for the staff and radio).

**What it does:**
1. **Supply source** (§6.4): 25 m radius, issues crates, takes deliveries.
2. **Radio link:** off-map support calls (artillery, smoke, recon; the
   §2.4 `SupportCall`) need a radio link: the HQ, a `radio` man, or an FO
   within `radio_range_m` (800 m) of the caller. Without one, no support
   calls. This is the HQ's command value and is historical.
3. **Retreat point:** `Retreat` goes to the nearest own HQ when it is
   closer than the base (`retreat_to_hq = true`). Men resupply there; they
   do **not** get crew refill or repair (base only).
4. **Rally:** an officer standing in the HQ uses his rally trait over
   `rally_m` (§3.4). No aura from the building itself.

**What it does not do:** no unit spawn, no replacement spawn, no reload
speed, no tech unlocks. On unlocks: gating call-ins behind an HQ is a tech
tree, which is unhistorical. If the owner wants a light gate, gate **only**
off-map support calls on having a radio link, which the rule above already
does.

**Why not a forward spawn, even limited:** "spawn at base and walk" is the
owner's balance lever. Any forward spawn (even with a long cooldown) makes
the HQ the one target that matters, rewards HQ sniping over fighting, and
breaks the AI's travel-time assumptions. If a forward reinforcement point
is ever wanted, the only form compatible with the rule is a **truck that
carries the men** from base (they still travel; the truck can be shot).

**Destruction:** roof pieces have thickness and HP (§5.10.4). A mortar
bomb that breaks the roof and bursts inside reaches the radio and the
stock. Stock of `shells`/`explosives` above 50 kg cooks off when its
container is destroyed (so a full HQ hit by HE is dangerous, which
pushes players to keep big stocks in dumps away from the CP, as real
armies did). Destroyed HQ: stock lost, radio lost, rebuild allowed after
`rebuild_cooldown_s` (120).

**AI:** builds one HQ when it holds a point and has a free engineer squad:
`hq_offset_m` (120 m) behind its most valuable held point, on the reverse
slope (a few ray probes from the nearest enemy points; choose the
candidate cell with no line of sight). Keeps it stocked (§6.8), retreats
damaged squads to it, and targets the enemy HQ with mortars when spotted
(score bonus).

Value **high**. Cost **M** after the structure framework and the supply
source. Systems: World (construction), Status (supply), Intent (orders,
AI), Damage (pieces, cook-off), snapshot, HUD (stock bar, radius ring).

### 7.3 Aid station

Names: *Truppenverbandplatz* (DE), Battalion Aid Station (US),
*batalyonny meditsinsky punkt* (SU).

- **Two builds:** tent (30 s, 20 kg, no protection) or dugout (5 min,
  150 kg, protected like the HQ).
- **Staffed, not an aura:** it works only while at least `staff_min`
  (2) men with `medic` stand inside it. They treat `beds` (2) men at a
  time (§5.2: Treat, Evacuate). Kill or drive off the medics and it stops.
- **Placement:** like the HQ; limit 2 per player.
- **AI:** medics build one near the HQ; downed men within 60 m of the
  front are stabilised and carried there when no enemy is within 40 m.

Value **medium–high** (depends on §5). Cost **M** after §5 and the framework.

### 7.4 Replacements (related)

A `Reinforce` order on a depleted squad buys missing men at their per-man
cost. They **spawn at base** and walk to the squad (owner rule). The HQ
does not shorten this. Cost S–M (spawn path exists; join-squad on arrival).

### 7.5 Ammo / supply dump

*Munitionslager* / ammunition point / *sklad boepripasov*. A sandbagged
pit (1 min, 40 kg) holding a finite stock: a supply source without radio,
limit 3. Shell cap 300 kg (`cap_kg.shells`), filled only by carried
crates or trucks. Guns and mortars inside its 15 m radius refill from it
(Q4). Cheap way to put shells near the mortars and keep the HQ's stock
small. Cooks off when HE breaks it. AI: one per mortar group. Cost **S**
once the HQ exists.

### 7.6 Combat structures

| Structure | Names | Builders (cap.) | Time, 5 engineers, clay (compressed) | Materials | Protection / effect | Destroyed by | AI use | Cost |
|---|---|---|---|---|---|---|---|---|
| Foxhole / slit trench | *Schützenloch*, foxhole, *okop* | any (`dig_basic`) | 30 s per man | — | dug ground | craters fill it | already digs trenches | exists (profile) |
| Sandbag wall | *Sandsackwall* | Lv 1 with carried bags, Lv 2 | 20 s per 2 m | 30 kg | sandbag pieces with thickness (stops rifle rounds) | HE, AT rounds | in front of MG positions | S (first piece type) |
| MG nest | *MG-Nest*, MG emplacement | Lv 2 | 1.5 min | 60 kg | gun pit + sandbag ring + camo net (concealment) | HE in the pit, satchel, flame | for every MG team on defence | M |
| Mortar pit | *Granatwerferstellung* | Lv 1–2 | 1 min | — | pit (fragment cover) | HE in the pit | for every mortar team | S |
| Dugout | *Unterstand*, dugout, *zemlyanka* | Lv 2 | 6 min (exists in design) | 120 kg timber | timber roof + earth: overhead cover vs mortars | heavy HE on the roof, satchel | shelter behind trench lines under barrage | M |
| Timber-earth bunker | *Holz-Erde-Bunker*, log bunker, *DZOT* | Lv 3 (Lv 2 at half rate) | 8–10 min | 250 kg | firing slit (an opening), thick earth roof | heavy HE, AT rounds through the slit, flame, satchel against the slit | at a key point, if it holds it long | M–L |
| Concrete pillbox | *Regelbau*, pillbox, *DOT* | **map/mission only** | — | — | thick concrete | AT rounds into the embrasure, satchels, flame | garrisons | data only |
| Barbed wire | *Stacheldraht*, concertina, *provolochnoe zagrazhdenie* | Lv 2 (`place_wire`) | 30 s / 10 m (exists) | 15 kg | nav obstacle for infantry; crossing slows to 20 % | wire cutters (`clear_wire`, 10 s per gap), Bangalore, HE fragments that touch segments | in front of trenches, 30–50 m out | M |
| AP mines | S-Mine 35/44, Schü-Mine 42, M2, PMD-6 | Lv 2 (`place_mines`) | 20 s each | 4 kg each | contact trigger (foot inside 0.3 m) → HE or bounding burst | detection (detector, probing) then lifting; HE nearby (fragments set them off) | approaches to own trenches | M |
| AT mines | Teller 42/43, M1A1, TM-41 | Lv 2 | 30 s each | 9 kg each | track or wheel over the trigger plate → belly/track blast through physical armour check | as above | roads and tank approaches | M |
| Czech hedgehog | *Tschechenigel*, hedgehog | Lv 3 | 1 min each | 60 kg steel | vehicle nav obstacle and collision | demolition, heavy HE, dozer | gaps between AT ditches | S–M |
| Log barrier / abatis | *Baumsperre*, abatis, *zaval* | Lv 2 | 2 min | trees nearby | vehicle obstacle on roads | demolition, dozer | forest roads | S–M |
| AT ditch | *Panzergraben* | Lv 2 | exists | — | blocks vehicles | fill, satchels | exists | exists |

Mines: under the physical rule they trigger only on contact. Own player
knows his mines (pathing avoids them); allies see them as marked; the
enemy sees a mine only when an engineer finds it (detector sweep at
walking speed, chance per mine per second within 1.5 m, data) or when one
goes off. No friendly-fire exemption for mines: they hit whoever steps
on them, which is physical and historical, and the pathing rule keeps
it fair for the owner.

### 7.7 Who builds what (summary)

| Squad | Lv 1 dig | Trench | MG nest / dugout | Bunker / obstacles | Wire | Mines | Clear wire / mines | Breach | HQ / dump | Aid post |
|---|---|---|---|---|---|---|---|---|---|---|
| Rifle / assault infantry | yes | slow | — | — | — | — | — | — | — | — |
| Field engineers | yes | yes | yes | half rate | yes | yes | yes | satchel | yes | yes |
| Assault engineers | yes | slow | — | — | — | — | yes (fast) | Bangalore, flame, satchels | — | — |
| Construction engineers | yes | fast | fast | yes | yes | yes (fast) | slow | — | yes | yes |
| Platoon HQ | yes | — | — | — | — | — | — | — | HQ only | — |
| Medics | yes | — | — | — | — | — | — | — | — | yes |
| Dozer | — | fill / ditch | pits | — | — | — | — | — | flatten | — |

---

## 8. Build order: value, cost, dependencies

| # | Item | Player value | Cost | Depends on | Sim systems touched |
|---|---|---|---|---|---|
| 1 | `extends` + `members` + tags/capabilities (no new mechanics) | **High**: unlocks most variants as files | M | — | content crate, `weapon_of`, order gating, AI buy mix, snapshot card data |
| 2 | Training profiles | Medium | S–M | 1 | Weapons (aim), Status (suppression), Perception, World (build rate) |
| 3 | Historical squad sizes, organic LMGs, new small arms | High (authenticity, fixes MG split) | S (data) + weapon defs | 1 | data only; balance pass |
| 4 | Finite small arms and mortar ammo, base as `SupplySource`, squad sharing, fire discipline | **High** (fixes mortar dominance, creates logistics) | M–L | 1 | `Kit`, Weapons (consume), Status (serve), AI (fire discipline), snapshot, HUD |
| 5 | Crates (issue at base, shells crates priced in manpower, carry, place, cook-off) | High | M | 4 | entities, Intent (carry orders), Damage (crate hits), presenter |
| 6 | **Structure framework** (pieces with armour faces + HP, blueprints) | **High**: core pillar, everything below needs it | L | — (the §5.10.3 design) | World, Ballistics (piece hits), nav, cover, snapshot, presenter |
| 7 | HQ (supply, radio, retreat), dump | High | M | 4, 5, 6 | Status, Intent, AI siting |
| 8 | MG nest, sandbags, wire, mortar pit, dugout | High | M | 6 | World, nav (wire), AI dig planner |
| 9 | Wound states, bleed-out, bandage (Phase A) | High | M | hit zones | Damage, Status, AI targeting, HUD |
| 10 | Medics, stabilise, carry, aid station, evacuation (B, C) | Medium–high | M + M | 6, 9 | Intent, Status, AI, economy |
| 11 | Assault kit: Bangalore, wire cutting, mine laying and clearing | High (breaching) | M | 6, 8 | Intent, World, Damage |
| 12 | Flamethrowers | High for trenches | L | 4, hit zones | projectile pool, Damage, Status (burning), presenter |
| 13 | Timber-earth bunker, hedgehogs, AT mines | Medium | M | 6, 11 | as 8 |
| 14 | Trucks / half-tracks as supply carriers | High once 7 exists | M (here) | vehicle area | Status, AI logistics |
| 15 | SN-42 body armour | Low–medium (Soviet flavour) | S–M | hit zones | Damage |
| 16 | Melee strike | Low–medium | M | hit zones | Weapons, Damage, AI, presenter animations |
| 17 | In-match veterancy | Low | S | 2 | Status |

Suggested slices:
- **S4a "Squad variants"**: 1, 2, 3 (+ the call-in cooldown rule if not
  done elsewhere). Mostly data; the AI mix moves to tags.
- **S4b "Ammunition and supply"**: 4, 5, 7 (HQ on a minimal placed-piece
  stub if 6 is not ready), 14 when trucks exist.
- **S5 "Fortification framework"**: 6, 8, 11, 13.
- **S6 "Wounds and medics"**: 9, 10.
- **S6b "Assault"**: 12, 15, 16.

---

## 9. Tests to add (headless, `frontline-cli`)

1. `supply-test`: an MG team firing at a target goes dry in the expected
   time; a crate within 5 m refills it; a mortar with no crate stops
   after its carried bombs. Same hashes at 1/2/3/8 threads and through
   save → load.
1b. `supply-test` price: a shells crate issued at base costs its kg ×
   `shell_mp_per_kg` in manpower, a small-arms crate costs nothing; a
   mortar team standing in base with no crate does not refill its bombs.
2. `supply-test` contact rule: an HE burst whose rays do not reach a
   crate leaves it intact; one that does cooks off a shells crate.
3. AI-vs-AI skirmish seeds 1–5 still end with a winner inside 30 min with
   finite ammo (the stall guard).
4. Mortar kill share in AI-vs-AI drops below small-arms kills (today
   2–4×; target ≤ 1×, owner to confirm).
5. `fortify-test` trench assault: an assault variant (StG 44 /
   SMG section) with mortar smoke, against today's rifle-squad baseline.
   Report numbers.
6. Wounds: a squad under fire ends with downed men; with a medic, the
   share who bleed out drops; aid station returns men at 75 % HP.
7. HQ: a mortar barrage that never touches the HQ pieces does no damage
   to it; one that breaks the roof kills the radio.

---

## 10. Owner questions

1. **Trickle at the HQ**: (A) none, trucks only; (B) small trickle by
   default, 0 in realism rules. Recommended (B) now, (A) later.
2. **Squad sizes**: authentic establishment (German 9–10, US 12, Soviet 9)
   with understrength variants, or keep 8? Recommended authentic; it
   changes pop and cost per squad, not stats.
3. **Officer rally** (the one non-physical effect): keep (small) or cut?
4. **Wound model**: approve Fit / Wounded / Down / Dead with bleed-out and
   evacuation refund, instead of HP regeneration?
5. **Melee**: approve "minimal strike, late, no knife squads"?
6. **Red cross**: plain armband unless a legal check clears the emblem?
7. **Tank stowage**: restore historical counts once field resupply
   exists?
8. **Penal units and Waffen-SS**: confirm both stay out.
