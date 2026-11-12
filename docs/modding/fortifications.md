# Fortifications, wire and breaching (S10a)

Blueprints for sandbags, MG nests, dugouts, HQ roofs and barbed wire; the
`[fortify]` rules; work tiers; construction material; the Bangalore item.
`docs/slices/S10a-fortifications.md` is the design (sections 3 and 5); this
page is the how-to. Every value below lives in TOML; nothing is hard-coded
in the sim. `frontline-cli validate mods/ww2` checks all of it.

## Blueprints: `defs/fortifications/*.toml`

One file per blueprint (`FortDef`). Both factions build the same
blueprints; who may build one is a capability, not a faction.

```toml
id = "ww2:mg_nest"
name = "MG nest"
shape = "point"                 # point | line | over_hq
pit = "mg_nest_pit"             # optional: a dig profile dug first
capability = "build_structure"  # or "place_wire"
materials_kg = 60               # point: the whole work; line: per segment
work_s = 90                     # seconds at 100 % with work_men men
work_men = 4
limit_per_player = 4            # sites (point) or pieces (line), built or building
min_spacing_m = 10              # point only
max_slope_pct = 25
pieces = [
    { part = "sandbag", size_cm = [60, 220, 60], offset_cm = [130, 0, 0], armour_mm = 100, hp = 400 },
]
sources = ["infantry-structures.md 7.6"]
```

| Key | Meaning |
|---|---|
| `shape` | `point`: one site, the command gives `[pos, toward]` (the facing). `line`: one segment of `segment_cm` per piece set (or wire section), built one after another along a dragged line of up to `max_length_m`. `over_hq`: a roof over an own built HQ within 5 m, once per HQ (no `pit`). |
| `pit` | A dig profile from `[dig.profiles.*]` (the shipped `mg_nest_pit`, `dugout_pit`). The squad digs it with its dig rate first; the build step starts when the pit is deep enough. |
| `capability` | `build_structure` or `place_wire` (see Capabilities). |
| `materials_kg` | Construction kg the step draws before work starts (point: once; line: each segment). |
| `work_s`, `work_men` | The step needs `work_s x work_men` man-seconds at 100 %; each builder within reach adds his tier's `build_pct` (x 0.3 while suppressed, nothing while pinned). |
| `limit_per_player` | Point: sites; line: pieces or sections, built or building. |
| `segment_cm`, `max_length_m` | Line only. |
| `min_spacing_m` | Point only: from the player's other sites of the same blueprint. |
| `max_slope_pct` | Refused on steeper ground (`BadSpot`): the undug base terrain, over 2 m, at a point site and along a line every 4 m (trenches, pits and craters do not count). |
| `pieces` | 1 to 4 boxes. `part` is `sandbag`, `roof` or `timber` (the colour only). `size_cm` is along the facing, across, up (10 to 600 cm each); `offset_cm` the centre along, across, and the box bottom above the ground. `z_from = "base"` measures z from the undug surface (a roof over a pit). `armour_mm` 1 to 2000, `hp` 1 to 10 000. |
| `wire` | `place_wire` blueprints only, and then no `pieces`: `{ band_cm, height_cm, hp }`. |

### What pieces do

Pieces are static boxes, like the S6 HQ boxes:

* bullets, shells and fragments stop at the first enemy piece in their
  path; own rounds pass own pieces (an MG fires out of its nest);
* a bullet damages a piece only if its penetration at that range beats
  `armour_mm`; a fragment only if its `frag_pen_mm` there does; a
  non-bursting main-gun round takes `[fortify] shot_piece_damage`;
  overpressure with a clear line takes the HE's `damage`;
* overpressure on a man needs a line clear of every piece not owned by the
  burst's player, so a roof shelters the men under it;
* a thrown item (grenade, satchel) stops at the first piece not its
  thrower's player's: on its top (a roof, a wall's top) it rests and bursts
  there, against a side it drops to the ground just short of it. A hand
  grenade (damage 150) on a dugout roof (250 hp) leaves it standing; a
  satchel (300) breaks it;
* sight and walking go through pieces (owner decision S10a-2 (A)).

At 0 hp a piece is removed. An HQ roof goes with its HQ.

### Wire

A man within `band_cm / 2` of a standing section moves at
`[fortify] wire_speed_pct` of his speed; infantry paths pay
`wire_path_cost_pct` per 4 m node near wire (a 20 m line is cheaper to walk
round than through unless the goal is just past it). HE overpressure with a
clear line takes the burst's `damage`; a fragment crossing a section below
`height_cm` deals `frag_damage x wire_frag_pct / 100` and flies on; a
vehicle hull crossing a section breaks it; bullets pass. Cutting
(`CutWire`), a Bangalore and HE remove sections.

## Rules: `[fortify]` in `defs/rules/skirmish.toml`

```toml
[fortify]
wire_speed_pct = 20          # % of a man's speed inside a standing band
wire_path_cost_pct = 500     # A* cost of a wire node for infantry
wire_frag_pct = 25           # % of frag_damage a crossing fragment deals to wire
cut_s = 10                   # s per wire section at 100 %
cut_reach_m = 25             # CutWire takes the nearest section within this
material_reach_m = 15        # builders draw from own crates and trucks this close
carry_kg_per_man = 20        # construction kg one man carries
construction_mp_per_t = 100  # MP per tonne, paid in the base
shot_piece_damage = 150      # hp a non-bursting main-gun round takes from a piece
[fortify.tiers]              # % work rates by tier
field = { build_pct = 100, cut_pct = 100 }
assault = { build_pct = 0, cut_pct = 200 }
construction = { build_pct = 150, cut_pct = 50 }
```

Every flat key is a tuning key, so `--set fortify.cut_s=5` and override
files work (`docs/modding/tuning.md`). A mod without `[fortify]` has no
fortification systems at all.

## Capabilities and tiers

Unit cards list capabilities (`docs/modding/variants.md`). S10a adds
`build_structure`, `build_heavy`, `place_wire`, `clear_wire` and `breach`
(`place_mines`, `clear_mines`, `demolish` stay reserved for S10b).

| Capability | Lets the squad |
|---|---|
| `build_structure` | build sandbag walls, MG nests, dugouts, HQ roofs; load materials |
| `place_wire` | build wire; load materials |
| `clear_wire` | cut wire (`CutWire`) |
| `breach` | lay a Bangalore it carries (`PlaceBangalore`) |
| `build_heavy` | (with the others) the construction tier |

The tier sets the work rates: `build_heavy` gives construction, else
`breach` assault, else any of `build_structure`, `place_wire`,
`clear_wire` field. Builders add their tier's `build_pct`, cutters its
`cut_pct`; a card's `dig_rate_pct` applies to digging (pits) only. With the
shipped values a section is cut in 10 s (field), 5 s (assault) or 20 s
(construction).

The shipped cards: Pioniergruppe and the US Engineer squad (field), the
Sturmpioniergruppe and the US Assault Section (assault, two Bangalores
each), the Baupioniergruppe (construction, conscripts, dig rate 200 %). The
AI buys and uses all of them (S10a-ai; weights under "The AI" below).

## Materials

Construction material is its own supply kind (`supply = "construction"`),
never stocked in an HQ or dump:

* **carried**: `LoadMaterials` in the own base, up to `carry_kg_per_man`
  per living man of a `build_structure` or `place_wire` squad, paid
  `kg x construction_mp_per_t / 1000` MP; no walking penalty;
* **crated**: a crate def with `supply = "construction"`
  (`ger_construction_crate`, `usa_construction_crate`, 40 kg) issued by
  `IssueCrate`, set down near a site;
* **trucked**: `LoadMaterials` on an own truck's squad in the base fills
  the truck up to its cargo cap (cargo plus material; the base auto-load
  then fills cargo only into the room left). A wrecked truck's material is
  lost.

Carried kg shrink with the squad: a man lost takes his share
(`carry_kg_per_man`) with him, a squad gone loses all of it.

A step draws its kg when it starts: carried first, then own crates within
`material_reach_m` (ascending id), then own trucks. Short, the site waits
("materials" in the HUD) until a draw completes it. A work whose builder
takes another order or dies is removed and its kg are lost.

## Bangalore: `kind = "bangalore"` items

```toml
id = "ww2:gestreckte_ladung"
kind = "bangalore"
supply = { kind = "explosives", g = 13000 }
fuse_ms = 8000                  # from the moment it is laid

[bangalore]
length_cm = 300
bursts = 3                      # evenly along the pipe, in one tick
place_s = 5

[he]                            # each burst, through the normal HE path
damage = 300
blast_radius_cm = 150
crater_radius_cm = 150
cuts_trenches = false
# ... fragments as any [he]
```

`PlaceBangalore` sends the nearest man carrying one in a `breach` squad to
within 2 m of the target; he works `place_s` and lays the pipe from his
spot toward the target. Each burst removes the wire in its blast and
damages what its fragments reach; sections it removes count as blown. One
charge opens about 4 m of the shipped wire; its craters (150 cm, 90 cm
deep) slow men who walk through them.

## The AI (S10a-ai)

The skirmish AI uses the same commands as a player (`Fortify`,
`LoadMaterials`, `CutWire`, `PlaceBangalore`, `Throw`, `SuppressUnits`);
every distance, time and share is in `[ai.fort]` (`docs/modding/tuning.md`),
spec `docs/slices/S10a-ai.md` 3.

* **Buying.** The three S10a cards buy through the `engineer` mix slot by
  their `ai_weight` (owner 2026-11-11: Pioniergruppe 40, Baupioniere 100,
  Sturmpioniere 60; US Engineer Squad 100, Assault Section 35). When a breach is wanted (known
  enemy wire within `breach_near_m` of a point the AI attacks) and no own
  squad can open wire (`clear_wire`, or `breach` with a Bangalore), the next
  buy is the faction's assault card, at most `breach_cards_max` alive.
  `cards = 0` skips the three cards.
* **Building** (`ai/fort.rs`). Builders (`build_structure`, `place_wire`)
  at a held point with a finished AI trench build an MG nest past the
  trench end, one wire line `wire_out_m` out and `wire_len_m` long, slid
  along itself so it does not lie across the straight route to the nearest
  point the AI does not hold unless that route is the approach it faces
  (`wire = 0`: none; it waits for the nest unless `nest = 0`), a dugout
  behind the trench once the point was shelled or enemy mortars were seen,
  the HQ roof once mortars were seen, and a sandbag wall in front of a gun
  that held `gun_hold_s`. They load carried materials in base (at most
  `materials_max_pct` of the income earned); a waiting work gets a truck
  run, else the builder walks back. A builder is held at its point only
  while a job could start there for it (room under `builders_max`, 60 %
  strength, the capability, a step it can carry or a free truck, no known
  enemy soldier or vehicle within `threat_radius` of it or the site); a
  job ends on such a threat, when its point is lost or threatened, or when
  its work stands still for three times the step's time plus a minute. A
  refused work is not tried again for 5 minutes, works over a blueprint's
  `limit_per_player` are never asked for, and an AI keeps at most two
  dugouts. An MG team (never the crew of a deployed MG) mans its point's
  nest.
  Under shelling one defender squad (not an MG or AT team) shelters: its men go one by one to
  trench spots under the dugout roof (the cover rule keeps men
  `[cover] gap_mm` apart, so the shipped 3 m pit holds two), and come out
  with the squad order when the mark is `shelter_s` old.
* **Knowing wire** (`ai/breach.rs`). An enemy section is known once an own
  soldier within `wire_seen_m` has a clear terrain line from 1.5 m over his
  ground to 0.5 m over its middle (`wire_rays` checks per plan). The AI
  never uses wire it has not seen to decide a breach.
* **Breaching.** For an attacked point with known wire near it, the AI
  asks the path finder (wire costs included) once per `recheck_s`. A path
  that crosses no known wire goes round. Otherwise the squad that can open
  it (Bangalore first, else `clear_wire`) stages `stage_m` short of the
  crossing once the point's base of fire fires; the base aims at the known
  defender nearest the crossing (MG-team men first); a living man with
  smoke (the assault squads carry it on three men: leader and two others),
  nearest that spot first, throws it between the crossing and that defender, at most half the
  cloud's radius past the crossing (a squad with its own smoke stages
  within its throw reach of that spot; no cloud is thrown short of it); `suppress_lead_s` later
  a Bangalore goes in if defenders are about (known within `cut_fire_m`,
  the base fired, or the squad is taking fire), else the squad cuts. A
  carrier who falls is a failed try at once, a laid charge or a cut under
  way is not; `breach_tries` tries in all. The job ends when no enemy
  section stands within `gap_clear_m` of the crossing (the attack paths
  through the gap), when the squad drops under half strength, the point is
  taken or `breach_max_s` passes. `breach = 0`: none of this; squads attack
  as on S10a.
* **Measures.** `skirmish --json` `factions.<f>.s10` (the AI counters) and
  `s10_derived` (shares per match); `fortify-test` `s10a.breach_ai` runs the
  breach job alone through `World::drive_breach` against the S10a staging
  (wire 100 m long so the path cannot go round; see the row's docs).

## Measuring

* `frontline-cli fortify-test` adds the S10a rows (`s10a.wire`,
  `s10a.roof`, `s10a.breach`, S10a-ai `s10a.breach_ai` in `--json`): the leader's speed in the wire
  band and through a cut and a blown gap, a dugout roof under an 81 mm bomb
  and a satchel, and the breach against a trench, MG nest and wire.
* `frontline-cli skirmish --summary --json` reports per faction
  `s10.{works_started, pieces_built, pieces_lost, wire_built, wire_cut,
  wire_blown, wire_crushed, bangalores, materials_loaded_kg,
  materials_used_kg, new_cards_bought}`.

## S10b: mines, AT obstacles, the bunker

The mechanics half (`docs/slices/S10b-obstacles.md`); the AI uses none of it
yet: no new blueprint has an `ai_weight` above 0 and `fort_of` never returns
a blueprint with `mine_item` or `blocks_vehicles` (S10b-ai adds that).

### Minefields and mine items

* A minefield is a line blueprint with `mine_item = "<item id>"` and no
  `pieces`: one mine at each segment end (`segment_cm`), `materials_kg` and
  `work_s` x `work_men` per mine, `limit_per_player` counts mines. Shipped:
  `ap_minefield` (200 cm, 30 m, 4 kg, 20 s x 2 men, 60 mines) and
  `at_minefield` (400 cm, 40 m, 9 kg, 30 s x 2 men, 30 mines), both
  `place_mines`.
* A mine item has `kind = "mine"`, a `[mine]` table (`trigger = "foot"` or
  `"vehicle"`, `trigger_cm`) and an `[he]` table like any item; it may
  appear in no kit, crate or roster (the validator refuses that). Shipped:
  `ap_mine` (foot, 30 cm; damage 150, blast 100 cm, 60 fragments) and
  `at_mine` (vehicle, 40 cm; damage 400 x `vs_vehicle_pct` 100, blast
  150 cm, more than a track's 100 hp).
* Rules in plain words: an AP mine goes off when an enemy man on foot (not
  riding) comes within `trigger_cm`; vehicles never set it off (R3). An AT
  mine goes off when the mine is within `trigger_cm` of an enemy vehicle's
  track centreline or a wheel; men never set it off (R4); its burst breaks
  the tracks over it (R5). Own and allied units never set off own mines,
  and a burst belongs to the mine's owner (R7). A sweeping man finds a mine
  his side does not know within `detect_cm` with `detect_pct_per_s` a
  second (R8); up to two sweepers walk the lane at `sweep_speed_pct` of
  walking speed, the lane at most `lane_max_m` long and `lane_half_cm` to
  each side (R9); a sweeper never steps on a mine his side knows, the
  nearest one lifts it in `lift_s` at the tier's `cut_pct` (field 100,
  assault 200, construction 50; x0.3 suppressed, nothing pinned) (R10,
  R11). Any HE burst with a clear line sets off mines within its blast
  radius on the next tick, Bangalores included (R12). A mine that went off
  is known to everyone; a found one to the finder's team only (R13).

### Obstacles and the bunker

* `blocks_vehicles = true` on a blueprint: each piece's box plus the wreck
  pad blocks vehicle path nodes (tracked and wheeled) while it stands;
  infantry walks through at full pace (R16, R17). Shipped: `hedgehog`
  (point, `build_heavy`, one `steel` piece 180x180x130 cm, armour 2000,
  hp 600, 60 kg, 60 s x 4 men, 20 a player, 3 m apart) and `log_barrier`
  (line, `build_structure`, `timber` pieces 400x80x100 cm, armour 100,
  hp 400, 100 kg and 120 s x 4 men a segment, 20 m, 10 segments).
* `pieces` may hold 1 to 6 pieces (was 4). The `bunker` (point,
  `build_heavy`, pit `bunker_pit` 350x300x150 cm without a spoil bank,
  250 kg, 540 s x 5 men, 2 a player, 30 m apart, slope 15 %) has six:
  0 the roof 420x370x80 cm on top (hp 1000; three 81 mm bombs and a satchel
  do not break it, seven AP shots do), 1 the rear wall, 2 and 3 the side
  walls, 4 and 5 the front walls (timber, hp 600, armour 100), which leave
  the slit: 79 cm wide and 40 cm high above the undug surface, facing the
  blueprint's facing. All pieces stand on the site's undug surface.
* No minefield, obstacle or bunker site within `vehicle_clear_m` of a
  vehicle hull (`BadSpot`, R18).
* `build_heavy` blueprints: a `build_structure` squad without
  `build_heavy` builds them at `heavy_by_field_pct` of its rate (R21).

### Demolition

`Demolish { squad, at }`: the nearest squad man with a `satchel` walks to
1.5 m of the piece nearest `at` (within `demolish_reach_m`), works
`demolish_place_s`, lays the satchel as a charge (used up) that bursts
`demolish_fuse_s` later. The piece takes `demolish_pct` of the charge's
damage (tamped; 900 from a 300 satchel); other pieces the normal rule
(R19, R20).

### New `[fortify]` keys (shipped values)

`heavy_by_field_pct = 50`, `detect_cm = 150`, `detect_pct_per_s = 50`,
`sweep_speed_pct = 25`, `lane_max_m = 50`, `lane_half_cm = 200`,
`lift_s = 20`, `demolish_place_s = 10`, `demolish_fuse_s = 10`,
`demolish_pct = 300`, `demolish_reach_m = 25`, `vehicle_clear_m = 6`
(`--set` works for each).

### Capabilities and cards

| Capability | Gives | Shipped cards |
|---|---|---|
| `place_mines` | lay minefields (`Fortify`) | Pioniergruppe, US Engineers, Baupioniergruppe |
| `clear_mines` | sweep and lift (`ClearMines`) | those three, Sturmpioniergruppe, US Assault Section |
| `demolish` | place a satchel on a piece (`Demolish`) | Pioniergruppe, US Engineers, Sturmpioniergruppe (the cards with a satchel) |

No kit, price or `ai_weight` changed. `skirmish --json` reports per faction
`s10b` counters (mines laid, set off, blasted, found, lifted, men lost to
mines, vehicles mined, obstacles and bunkers built, sweeps, demolitions,
pieces demolished) and `used`; `fortify-test` adds the S10b rows (T102-T107).
