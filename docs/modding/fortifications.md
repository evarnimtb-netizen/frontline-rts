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
three new cards have `ai_weight = 0` in the mechanics half: the AI does not
buy or use them.

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

## Measuring

* `frontline-cli fortify-test` adds the S10a rows (`s10a.wire`,
  `s10a.roof`, `s10a.breach` in `--json`): the leader's speed in the wire
  band and through a cut and a blown gap, a dugout roof under an 81 mm bomb
  and a satchel, and the breach against a trench, MG nest and wire.
* `frontline-cli skirmish --summary --json` reports per faction
  `s10.{works_started, pieces_built, pieces_lost, wire_built, wire_cut,
  wire_blown, wire_crushed, bangalores, materials_loaded_kg,
  materials_used_kg, new_cards_bought}`.
