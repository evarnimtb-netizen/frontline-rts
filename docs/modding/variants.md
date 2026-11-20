# Variants, unit cards and training (S4)

A def file can start from another def of the same kind and give only what
differs. Unit cards can list their men slot by slot, say what they can do
(capabilities) and what they are (tags), and name a training profile.
`docs/slices/S4-variants.md` is the design; this page is the how-to.

S4 was built in two parts. **Part A** is the content crate: every key
below loads, merges and is checked, and `frontline-cli validate` reports
mistakes with a fix. **Part B** (now) runs it in the sim: per-man weapons
from `[[members]]`, crew-weapon pickup, capabilities and tags instead of
the S1-S3c role, training profiles, the card limits (`max_alive`, limit
groups, per-card `unlock_s`, dates), the AI's tag mix, and the
single-shot AT item (Panzerfaust). `mods/ww2` itself is written this way
(the shipped cards are good examples: `defs/units/ger_grenadiergruppe.toml`
and its variant `ger_grenadiergruppe_7.toml`).

## `extends`

```toml
# mods/ww2/tests/variants/units/ger_grenadiergruppe_example.toml
id      = "ww2:ger_grenadiergruppe_example"
extends = "ww2:ger_grenadiergruppe"
name    = "Grenadiergruppe (example)"

[[members]]
slot = "leader"
weapon = "ww2:mp40"
traits = ["leader"]
items = { "ww2:stielhandgranate" = 2 }

[[members]]
slot = "mg_gunner"
weapon = "ww2:mg42_bipod"          # a crew weapon
traits = ["crew_gunner"]

[[members]]
slot = "mg_assistant"
weapon = "ww2:p38"
traits = ["crew_assistant"]        # picks the MG up first

[[members]]
slot = "rifleman"
count = 3
weapon = "ww2:kar98k"
items = { "ww2:stielhandgranate" = 1 }
```

- Every kind can extend: `units`, `weapons`, `items`, `ammo`, `vehicles`,
  `factions`, `maps`, `rules`, `training`. The parent is a def of the
  **same kind** in the same mod.
- A chain is at most **4 parents** deep. An unknown parent (the error
  suggests the nearest id), a parent of another kind, or a cycle
  (`a -> b -> a`, all named) is an error on the child's file.
- Merging goes from the root down: **tables merge key by key; arrays and
  values replace.** `[he] frag_count = 10` changes one key of the
  parent's `[he]`; `tags = [...]` or `[[members]]` replaces the parent's
  whole list.
- `id` and `abstract` are never inherited; `extends` itself is gone after
  loading.
- The merged def is checked as a whole. An error in a value the child
  inherited names its file and the parent: `defs/units/x.toml: sight_m:
  must be in 1..=2000, got 0 (from \`ww2:parent\`)`.
- `abstract = true` (units): a template. It is checked, but it is never in
  a roster, the start squads or a match.
- Tuning overrides (`--rules-override`, `[defs.<kind>."<id>"]`) patch the
  file as written, and then every def resolves again: a patch on a parent
  reaches every variant of it, and the run's change list shows each.

### Units: the two merge rules

1. **Men.** A unit gives its men either by the legacy keys (`men`,
   `weapon`, `weapon_men`, `sidearm`, `[[loadout]]`) or by `[[members]]`.
   A level that sets `[[members]]` drops the legacy keys it inherited. A
   level that sets a legacy key over a parent with `[[members]]` is an
   error ("parent uses [[members]]; set [[members]] here").
2. **Renamed card keys.** `cooldown_s` was `recall_cooldown_s`, `unlock_s`
   was `initial_cooldown_s`. Both names load. One file may set only one of
   the two; a level that sets one drops the other inherited name.

A tuning override that patches `[[defs.units."<id>".members]]` onto a
legacy squad replaces its legacy soldier keys the same way.

## `[[members]]`

| Key | Meaning |
|---|---|
| `slot` | free name shown on the squad card: lower-case letters, digits, `_` |
| `count` | men in the slot, 1..=16 (default 1); all slots 1..=16 |
| `weapon` | weapon id each man carries |
| `sidearm` | what he fights with after handing a crew weapon on (stored; nobody hands one on in S4) |
| `items` | `{ "<item id>" = n }` per man; one item of each kind (frag, smoke, satchel, single_shot_at, S11b rifle_grenade) per man, and one item id per kind in the whole squad (S4-26); n ≤ 10. S11b: a `rifle_grenade` item is issued only with `[items] rifle_grenades` 1 (ww2: the `rifle_grenadier` slot of `usa_rifle_squad` and `ger_grenadiergruppe`; a variant with its own `[[members]]` list has none) |
| `ammo` | rounds he carries; stored, but rounds stay unlimited until S5 |
| `traits` | `leader` (first slot, count 1), `crew_gunner`, `crew_assistant`, `ammo_bearer`; S5b `observer` (adjusts fire missions at `[fire_mission] observe_observer_m`; every squad's leader already observes at `observe_leader_m`, `docs/modding/fire-missions.md`) |

Slots are in spawn and formation order. The first man of the first slot
leads the squad (he gets the leader's items). `officer`, `radio`, `medic`
and `sniper` are reserved traits and refused until their slice.

**Each man keeps his own weapon** from spawn. **Crew weapons** (a weapon
with `crew_weapon = true`; by default every `mg`, `mortar` and `launcher`
class weapon) are picked up: when the carrier falls, after the weapon's
`handover_ms` the nearest living man of his squad who carries no crew
weapon takes it (`crew_assistant` and `ammo_bearer` men first, then the
nearest, ties by id); he drops his own weapon. Only men within the rules'
`items.crew_pickup_reach_m` (30 m) of the weapon take it; otherwise it
waits where it fell until one comes near. Other weapons stay with the
man who carried them (a tank crew's SMG is not passed on). The man firing a
mortar is the first one in formation order who carries it. The squad's
focus range is its longest-reaching non-crew weapon (S4-23).

**Launchers** (S4 review): a squad has one launcher carrier, and the
launcher must be its main weapon (the first crew weapon among the members,
so a launcher slot goes before any MG or mortar slot). Two launcher men,
or a launcher behind an MG, is an error: the second man would never fire.
Put a second launcher in its own AT team. Crew squads (tag `crew`) carry
no launcher; a Panzerfaust (single-shot AT item) is allowed.

**Legacy squads** expand into slots when they load, so every card has
`members` afterwards:

| Slot | count | weapon | items |
|---|---|---|---|
| `leader` | 1 | `weapon` | each `[[loadout]]` item's `leader_count` |
| `weapon` | `weapon_men` − 1 | `weapon` | each item's `count` |
| `sidearm` | `men` − `weapon_men` | `sidearm` | each item's `count` |

A legacy launcher squad (`weapon` is a launcher) has one launcher man
whatever `weapon_men` says; the others carry `sidearm` (the S3 rule).
Without `sidearm` every man carries `weapon`. That is the S1-S3c
behaviour: a legacy MG team fields three MGs and every mortar man carries
the tube (a fallen man's gun is then lost: nobody without a crew weapon is
left to take it, S4-29). The shipped `ww2` cards give one gun and
assistants (S4-2).

## The card block (unit files, top-level keys)

| Key | Meaning | Default |
|---|---|---|
| `cost` | manpower per call-in | required on soldier cards |
| `cooldown_s` | seconds before this card can be called in again; the unit spawns at the base at once, nothing is queued | 0 (alias `recall_cooldown_s`) |
| `unlock_s` | match seconds before the first call-in | 0 (alias `initial_cooldown_s`) |
| `cooldown_group` | cards naming the same group share one timer | unset: each card its own (owner Q21 (B)) |
| `max_alive` | most squads of this card alive per player | 0 = no cap |
| `limit_group` | a group of the rules' `[limit_groups]`; alive units of all its cards count | none |
| `available_from`, `available_to` | `"YYYY-MM"`; outside the rules' `[call_in] date` the card leaves the roster | always |
| `tags` | AI mix and HUD labels, at most 8, `[a-z0-9_]` | from `role` |
| `capabilities` | what the unit can do; orders follow | from `role` |
| `training` | training profile id | the rules' `[training] default` |
| `ai_weight` | the AI's weight among ready cards of one mix tag; 0: the AI never buys it (S7) | 100 |
| `short_name` | HUD tile name, ≤ 14 characters | `name` |
| `sources` | citations, a list of strings | none |
| `abstract` | a template (see above) | false |

Vehicle cards (`vehicle = "..."`) may set `cost`, `cooldown_s` and
`unlock_s` to override the vehicle def (owner question S4-15 (A)); unset,
they come from the vehicle def as before. They must use the S4 names.

`role` is legacy: it fills `tags` and `capabilities` left empty. A card
without `role` needs `tags`; the loader then takes the S3 role from the
first of the tags `crew`, `vehicle`, `at`, `mortar`, `engineer`, `mg`,
`line` (or from a mortar or launcher among the weapons), which the HUD and
today's AI use.

### Tags

Free labels. Four are reserved:

| Tag | Rule |
|---|---|
| `vehicle` | exactly the cards that name a vehicle |
| `crew` | crew squads; never in a roster or the start squads |
| `at` | the AI buys it after the vehicle unlock, once an enemy vehicle was seen |
| `understrength` | the HUD's understrength marker |

Legacy roles map to tags as: rifle `line`, mg `mg`, engineer `engineer`,
mortar `mortar`, anti_tank `at`, vehicle `vehicle`, crew `crew`.

### Capabilities

| Capability | Gates | From `role` |
|---|---|---|
| `dig_basic` | the Dig order (trench) | any infantry role with `dig_rate_pct > 0` |
| `dig_full` | dig profiles that need it (the AT ditch) | engineer |
| `fire_mission` | the FireMission order (S5b; `barrage` before it) | always derived: a member with a mortar |
| `recrew` | boarding an own abandoned vehicle as untrained crew | rifle, mg, engineer, mortar |
| `vehicle_crew` | trained crew (capture, dismissal in base) | crew |
| `at_hunt` | AT hunts | always derived: a member with a launcher |

The tag `crew` and the capability `vehicle_crew` go together: a crew
squad has both, any other squad neither (the sim keys crew rules on the
capability, the vehicle's `crew_squad` check on the tag).
`dig_basic` and `dig_full` need `dig_rate_pct > 0`. Setting `fire_mission` or
`at_hunt` without the weapon is an error. Reserved for later slices, and
refused with a hint: `build_structure`, `build_heavy`, `place_wire`,
`clear_wire`, `place_mines`, `clear_mines`, `breach`, `demolish`,
`observe`, `radio`, `medic`, `gun_crew`, `issue_crates`, `flame`.

## Training profiles (`defs/training/*.toml`)

```toml
id = "ww2:training_veteran"
name = "Veteran"
spread_pct = 85             # aim error: multiplies the weapon's spread
aim_time_pct = 80           # time to open fire on a new target
reload_pct = 90             # reload time
throw_scatter_pct = 85      # scatter of thrown items
steadiness_pct = 125        # multiplies the card's steadiness_pct
suppression_decay_pct = 120 # suppression decay per second
spot_pct = 110              # the men's sight range (50..=200)
build_rate_pct = 100        # multiplies dig_rate_pct
```

Every value is a percentage of the regular soldier (25..=400 unless
noted); a missing key is 100. `mods/ww2` ships `conscript`, `regular`,
`veteran` and `elite`. Every S4 card is regular (owner question S4-10
(A)). No in-match veterancy.

## Rules keys (S4)

```toml
[call_in]
date = ""                         # "YYYY-MM": the match month; "" = no filter

[training]
default = "ww2:training_regular"  # cards without `training`

[limit_groups]                    # "<group id>" = most alive per player
# "ww2:heavy_armour" = 2

[ai.mix]                          # the target mix by tag, % of squads
line = 50
mg = 20
engineer = 15
mortar = 15
vehicle = 20                      # after the vehicle unlock
at = 20                           # once an enemy vehicle was seen
```

`[ai.mix]` replaces `mix_rifle_pct`, `mix_mg_pct`, `mix_engineer_pct`,
`mix_mortar_pct`, `mix_vehicle_pct` and `mix_at_pct` (tags `line`, `mg`,
`engineer`, `mortar`, `vehicle`, `at`, one to one). A rules file uses one
form; both at once is an error naming the keys to move.

## What a match refuses

`frontline-cli validate` accepts everything above, and a match runs it. A
match start still refuses (naming the card and the key):

- a `limit_group` the rules' `[limit_groups]` do not list;
- two different items of one kind in one squad (S4-26).

## Single-shot AT items (Panzerfaust)

An item of kind `single_shot_at` is fired, not thrown. It names the
launcher weapon it fires as:

```toml
# defs/items/panzerfaust60.toml
kind = "single_shot_at"
weapon = "ww2:pzf60"   # a class = "launcher" weapon: range, engage_still_m,
                       # engage_moving_m, spread, ammo (a HEAT round), reveal
min_range_m = 10       # arming distance
range_m = 60
windup_ms = 1500       # aim time before the shot
```

A man carrying one aims at a seen enemy vehicle within the weapon's
`engage_still_m` (`engage_moving_m` when it moves), then fires it as a
physical round (the jet and spall do the damage); the tube is spent. A
squad fires at most one per the rules' `items.single_shot_gap_s`. Cards
whose men carry one get the derived capability `at_hunt`; the AI hunts
close vehicles with them (`ai.assault.pzf_hunt_m`).

## Trying a variant

`mods/ww2/tests/variants/` holds the example above
(`units/ger_grenadiergruppe_example.toml`) and `roster.toml`, a tuning
override that puts it in the German roster. In Rust:

```rust
let mut defs = ModDefs::load(&mods)?;
defs.add_defs_dir(&mods.join("tests/variants"))?;      // the variant
defs.apply_override(&mods.join("tests/variants/roster.toml"), "ww2:skirmish")?;
```

`frontline-cli validate mods/ww2` loads and resolves it too. Godot's F5
reload reads the mod again, `extends` included.

## Errors

Every problem in every file is reported at once, each with its file, key
and, where one exists, a fix:

```
defs/units/zz.toml: extends: unknown units def `ww2:usa_rifle_sqad` (did you mean `ww2:usa_rifle_squad`?)
defs/units/zz.toml: members[0].traits[0]: trait `officer` is reserved for a later slice and has no effect yet (fix: remove `officer` (S4 traits: leader, crew_gunner, crew_assistant, ammo_bearer))
defs/units/zz2.toml: capabilities[0]: capability `breach` is reserved for a later slice and has no effect yet (from `ww2:zz_variant`) (fix: remove `breach` ...)
```

The JSON Schemas (`frontline-cli schema --out DIR`) include `extends` on
every def kind (a file with `extends` needs only `id`), the card block,
`members` and the new `training` kind.
