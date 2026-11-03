> Archived 2026-11-03 from `docs/design/roster/roster-expansion.md`: parts that are built (variant system, S4/S5 cards) and the critic notes. Do not edit.

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
  against S3b. A full rack refill costs at most 75 % of the card
  (`rack_refill_max_card_pct`), the same in base and forward; fallback
  lever `rack_rearm_price_pct`.
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

