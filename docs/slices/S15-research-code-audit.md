# S15 "USSR": code audit (read-only)

Working note for the S15 plan (2026-11-22, research agent). Not a spec: the plan decides. Repo read at `main` 6f7bda0
(S12 merged; SAVE 30, PROTOCOL 16). Paths: `sim/` = `crates/sim/src/`, `content/` = `sdk/crates/content/src/`,
`cli/` = `sdk/crates/cli/src/`, `bridge/` = `crates/godot-bridge/src/`, `defs/` = `mods/ww2/defs/`.
Card: `docs/design/roster/roster-expansion.md` 205-218 (3.3), 669-679 (S15); tier "data, 0.6" in `docs/process/workflows.md` 26.

**Bottom line.** The sim and the content crate already take N factions: no sim file outside `#[cfg(test)]` names a
faction, nothing caps factions, every faction lookup is by index. The faction itself is data. Not data:
- **tank riders** need a "deck" seat rule (exposure, sight, validation), a dismount trigger, and AI code (M);
- **body plates** need a plate check in two hit paths plus one member key (S);
- **captured ammo** needs a second feed on a weapon and a faction on the feed (M);
- **front end and tools** assume two factions: the bridge's `FACTIONS: [&str; 2]`, the GDScript pickers, `pool.py`'s
  "Germany", and the targets runs without faction fields (S each, M together).
- A mirror match is refused by the bridge and the relay only. The CLI and the sim allow it (one AI bug).

## 1. What exists, and what is missing

| Needed | State | Where |
|---|---|---|
| Faction def (id, name, color, roster, start_squads) | exists, any count | `content/defs.rs` 1513-1526; validate 2830-2860 |
| Loader | one TOML per file in `defs/factions/`, BTreeMap by id | `content/modset.rs` 67, 539-561 |
| Cross checks | squad's faction exists; roster squads of that faction, not crew, not abstract; start squads | `content/modset.rs` 1044-1062, 1264-1310 |
| Vehicle, crew, stock, truck-crate faction checks | exist, by id | `content/modset.rs` 1175-1178, 1325-1341, 1353-1410, 1416-1455 |
| Lowering | **every** faction of the mod goes into `MatchSetup::factions`, sorted by id; players index into it | `content/setup.rs` 1922-1958, 1959 |
| Faction count cap | none (`MAX_MATCH_PLAYERS` 10 is players) | `content/setup.rs` 2616 |
| Sim faction use | by index only: rosters, start squads, stocks, AI rosters | `sim/game.rs` 1100, 1974-1980; `sim/rules.rs` 214; `sim/ai.rs` 1708, 1728 |
| Mirror in content and CLI | allowed (only existence checked) | `content/setup.rs` 1470-1473; `cli/skirmish.rs` 2131-2160 |
| Mirror in bridge, relay, menus | refused | `bridge/match_setup.rs` 159-161; `crates/net/src/relay.rs` 890-893; `godot/ui/menu/multiplayer_panel.gd` 563 |
| Tank riders | half: slots, Embark, hit boxes exist; no deck exposure, no hit/fire dismount, AI treats it as a transport | section 4.1 |
| Torso plate | missing; zones exist | section 4.2 |
| Captured ammo | missing; crate faction is the consumer's own | section 4.3 |
| Faction colour in 3D | by faction **index** into a fixed 10-colour table | `bridge/match_node.rs` 1288; `godot/shaders/soldier_skinned.gdshader` 12, 80 |
| UI theme USSR colour | defined in the doc, not wired (neither are GER/USA theme colours) | `docs/slices/S1b-ui-theme.md` 41-43; `godot/ui/palette.gd` 24-29 |

**A third faction file needs:** `id = "ww2:ussr"`, `name`, `color = "#rrggbb"`, a non-empty `roster` of squads whose
`faction = "ww2:ussr"`, and `start_squads` inside the roster (`content/defs.rs` 2836-2856). **Use the id `ww2:ussr` and unit
prefix `ussr_`.** Ids sort, and the sort sets the index (`content/setup.rs` 11-13). `germany` < `usa` < `ussr`, so the USSR
becomes index 2 and appends. A prefix like `sov_` sorts between `ger_` and `usa_` and moves every USA squad index. A faction id
like `ww2:soviet` makes the USA index 2 instead of 1, and the USA's 3D tint would change (section 5).

## 2. Every site that names Germany/USA or assumes two factions

Classes: **(a)** works for N factions already; **(b)** needs a data entry only; **(c)** needs code. Test-only sites are left out.
`sim/` has **none**: every grep hit sits below a `#[cfg(test)]` line. Examples: `sim/world.rs` 2981, `sim/combat.rs` 4459,
`sim/ai/guns.rs` 1245, `sim/ai/fort.rs` 5230.

| Site | What | Class |
|---|---|---|
| `sim/ai/assault.rs` 456-470 | strongest **enemy** MG weight filters `t.faction != mine`: in a mirror it is 0 | (c) mirror only |
| `sim/guns.rs` 656-661 | captured-gun kills = gun def faction differs from the player's: a mirror never counts them | (c) report, mirror only |
| `sim/structures.rs` 1087-1091, 1144 | men draw only from crates of **their own** faction | (a); see 4.3 |
| `content/setup.rs` 140-164 | `MatchRequest::skirmish(seed, human, ai)`: any ids | (a) |
| `crates/net/src/protocol.rs` 322 | `factions: [String; 2]`: two **players**, any ids | (a) |
| `crates/net/src/relay.rs` 890-893 | refuses equal factions | (c) for mirror |
| `crates/net/src/harness.rs` 292-315 | `net_setup_rules` hard-codes GER vs USA (net test tool) | (a) (test tool) |
| `bridge/match_setup.rs` 69 | `FACTIONS: [&str; 2] = ["germany", "usa"]` | (b) add "ussr" |
| `bridge/match_setup.rs` 133-151 | skirmish opponent = "the other" of `FACTIONS[0..2]` | (c): needs an opponent field in `MenuChoice` (16-37) |
| `bridge/match_setup.rs` 155-161 | unknown faction / mirror refused | (b) / (c) mirror |
| `bridge/net_session.rs` 321, 448 | lobby choice filter; default faction `FACTIONS[slot % 2]` | (a) after 69 |
| `bridge/match_node.rs` 893 | default `"germany"` | (a) |
| `bridge/match_node.rs` 1288 | soldier tint = faction index (mirror: both sides one colour) | (a) for 3 factions; (c) mirror |
| `cli/skirmish.rs` 76-80, 173-174 | `--faction0/--faction1`, defaults GER/USA; passed on by `--seeds` (3920-3930) | (a) |
| `cli/skirmish.rs` 4006-4015 | per-faction tallies keyed by faction id: a mirror merges both players into one row | (a); mirror (c) |
| `cli/json_out.rs` 222-228 | `wins.<short>_pct` for whichever factions played | (a) |
| `cli/targets.rs` 99-122, 400-410 | `RunSpec` has no faction fields; skirmish runs use the GER/USA defaults | (c) |
| `cli/armour_test.rs` 209; `cli/mortar_bench.rs` 200 | bench setups GER vs USA | (a) keeps working; USSR cases (c) |
| `cli/fortify/s10a.rs` 516, 1033-1063; `bunker.rs` 194-237, 329; `s11a1.rs` 41-42; `s11a2.rs` 39-41; `s11b.rs` 40, 105-106, 326-333, 435 | "germany else USA" ids, 2-arrays of cards | (a) on their GER/USA setups; (c) to test USSR |
| `cli/armour_test/s8b.rs` 24-28; `s9.rs` 23 | USA card constants | (a) |
| `tools/pool.py` 32-33 | `BOUGHT` keyed by faction (`.get` with a default) | (b) |
| `tools/pool.py` 45, 96 | `wins.get("germany")` → "Germany pooled": a USSR-vs-USA set reads 0 % | (c) |
| `tools/ablate.sh` 319-339 | reads pool.py's GATE `pool=` as "Germany" | (c) with pool.py |
| `tools/batch.sh` 119-128 | `sets` hard-codes the skirmish args (no faction pass-through) | (c) |
| `tools/measure.sh` 33, 36-57 | the 40 set and `skirmish40_*` controls: no faction field | (c) |
| `docs/slices/targets/S12.toml` 177-194 (and every targets file) | `wins.germany_pct`, `factions.germany.*`, `factions.usa.*` rows | (b) new rows per matchup; a missing key prints `MISS (no such number)` (`cli/targets.rs` 289) |
| `godot/autoload/session.gd` 57, 147, 440 | `FACTIONS := ["germany", "usa"]`; CLI filter; autoplay host/guest | (b) / (a) |
| `godot/ui/palette.gd` 24-29 | `FACTION_COLORS`, `FACTION_NAMES`, `FACTION_SHORT` | (b). `main_menu.gd` 154 and `multiplayer_panel.gd` 334, 533 index `FACTION_NAMES[f]` directly: without the entry they crash |
| `godot/ui/menu/main_menu.gd` 19, 149-164 | picker loops `Session.FACTIONS`; no opponent picker | (a) loop; (c) opponent |
| `godot/ui/menu/multiplayer_panel.gd` 328-346, 527-533, 563 | picker, "taken", mirror blocker | (a); (c) mirror |
| `godot/match/vehicles.gd` 59, 63 | `PAINT`, `GLACIS_DEG` by faction (`.get` with defaults) | (b) |
| `godot/match/match_view.gd` 97-99, 745 | fallback weapon names by faction | (b) |
| `godot/match/vehicle_view.gd` 126-129, 160-162 | tint = `PAINT[faction]` mixed to own/enemy | (b) |
| `godot/match/hud/top_bar.gd` 85; `end_screen.gd` 96-99 | names and colours via `.get` | (b) |
| `godot/net/mock_net.gd` 33, 418; `godot/match/mock_*.gd` | mock: `_other()` flips GER/USA; mock vehicle tables | (c) if the mock should show USSR; else leave |

## 3. Match setup and measuring

- **Choice.** CLI: `--faction0 ID --faction1 ID`, player i at base i, `--swap-bases` / `--both-bases`
  (`cli/skirmish.rs` 74-80, 139-140, 2156-2159). Menu: only the human's faction; the AI gets "the other"
  (`bridge/match_setup.rs` 137-151). Net: each slot picks; the relay hands both (`relay.rs` 914-922).
- **Saved and hashed.** `MatchSetup` (factions, players, all defs) is in the saved state and its postcard bytes are hashed
  (`content/setup.rs` 3-6); the net `Ready` check compares `setup_hash` (`crates/net/src/client.rs` 366).
- **Mirror.** Content, sim and CLI accept it. Bridge, relay and lobby refuse it. AI flaw `ai/assault.rs` 465 (table 2).
- **USSR vs Germany, USSR vs USA, single runs:** work today with `tools/batch.sh OUT.json -- skirmish --ai-vs-ai --faction0
  ww2:ussr --faction1 ww2:germany ...` (`batch.sh` 133-135 passes args through).
- **480 measures:** need code. `batch.sh sets` (119-128) and `measure.sh` (33, 56) pass no factions.
  `RunSpec` has none (`cli/targets.rs` 99-122). `pool.py` reads only `germany` (45). Minimal: a `faction0/faction1` pair in
  `RunSpec` and `SkirmishArgs` construction (400-410); `FACTIONS="a b"` env in batch.sh/measure.sh; a pool.py
  `--side ussr` arg (share of a named faction). Per `workflows.md` 32-34 each matchup is judged on its own set.
- **Targets files.** All skirmish rows hard-code `germany`/`usa` keys (e.g. `targets/S12.toml` 167-194, 261-425). A USSR
  matchup needs its own targets file or its own runs (e.g. `skirmish160_su_ger`) with `ussr`-keyed rows.

## 4. Faction-keyed content and the three new systems

### 4.0 What the USSR must supply (else refusal, crash or silent gap)
| Item | Why | Where |
|---|---|---|
| Supply defs: HQ, dump, aid post, one crate per kind (small_arms, shells, explosives, construction) | one per kind per faction (validated); a missing one makes `IssueCrate`/build refuse (`structures.rs` 616-622, 701-703); `s12_content.rs` 225-230 asserts an aid def **per faction** | `content/modset.rs` 1353-1410; pattern `defs/supply/usa_*.toml` |
| Truck `[supply]` crates of its own faction | validated | `content/modset.rs` 1416-1455 |
| Tank crew squad (role crew) per vehicle | validated | `content/modset.rs` 1325-1341 |
| Tags the AI reads: `line`, `mg`, `engineer`, `mortar`, `at`, `vehicle`, `hq`, `bearers`, `crew`, `truck`, `recon`, `field_howitzer`, `light_mortar`, `medic`, `observer`, `construction`, `assault` | `[ai.mix]` is by tag (`skirmish.toml` 928-934); habits find cards by tag (`ai/supply.rs` 69, `ai/arty.rs` 263, 439). A missing tag means that habit never runs: no error | `defs/units/*.toml` `tags` |
| Cards in the right roster order | AI tie-breaks and benches take the first card of a role (`germany.toml` comments 13-17) | `defs/factions/` |
| `start_squads` | fielded at tick 0 (`sim/game.rs` 1974-1980); must be fielded on the match date (`content/setup.rs` 1925-1934) | faction file |
| Rules keys by faction | **none**: e.g. `gun_fire_direction_s` is one value (`skirmish.toml` 1675); `[limit_groups]` is by group (1259-1263) | - |
| Cartridges and ammo | grams of a supply kind; no faction (`defs/cartridges/8cm_wgr34.toml` 5-8) | `defs/cartridges/`, `defs/ammo/` |
| Menu/HUD entries | palette, Session, vehicles.gd PAINT (table 2) | GDScript |

### 4.1 Tank riders
**Exists.** Riders stay soldiers (`Unit::riding`, `sim/unit.rs` 190-193); slots are hit volumes (`sim/transport.rs` 1-16);
Embark/Disembark take any vehicle with a transport (`transport.rs` 154, 176; no role test). Rounds hit a rider box by segment
test, whether or not the box is open (`sim/armour.rs` 805-824; called for any vehicle with riders, `combat.rs` 3060). Blasts and
fragments reach rider boxes (`sim/damage.rs` 1144-1160). Forced exit of all riders on a wreck, fire or ammo fire
(`transport.rs` 318-332). Riders never fire (`transport.rs` 7). Limits: `MAX_PASSENGERS` 12, `MAX_INNER_BOXES` 24 counting
crew seats + slots + component boxes (`content/vehicle.rs` 539-542, 2102-2118). A tank has 5 seats and about 8 component
boxes (`defs/vehicles/usa_m4a3_75.toml` 160-240), so 8 riders fit; the exact count is not checked.

**Missing (code sites).**
1. Role. `[transport]` and `passenger_slots` are refused unless `role = "transport"` (or a tractor). See
   `content/vehicle.rs` 2523-2542 and the setup check 4220-4224. Precedent: the S9 tractor exception, `tractor_rides`
   (2524). **S.**
2. Slot bounds. A slot must lie inside the hull frame's boxes; only an open box gets a little extra room on top (`check_part`, `vehicle.rs` 2584-2590,
   `frame_bounds` 2599-2610). A seat on top of a closed engine deck fails. **S** (a `deck` flag that allows slots above box 0).
3. Exposure. `slots_above_rim` sets a bit only for a slot inside an **open** box (`sim/vehicle.rs` 729-745, 917-928).
   A deck slot is never "exposed". Effects: enemies never see the riders (`sim/spotting.rs` 222), and fire never aims at
   them (`combat.rs` 328-337), although rounds that pass still hit them (armour.rs 805). **S** (deck slots always set).
4. Dismount on hit or fire. The only forced trigger is wreck/burn/ammo fire (`transport.rs` 321). Hooks:
   - the hit: the armour-hit record in `damage.rs` 398-410 already sets `last_hit_dir`;
   - firing: the gunnery fire step (`gunnery.rs` 394-, reload set at 572-584).

   The rule could set `unload = Some(Unload { over_sides: true, under_fire: true })` (`sim/vehicle.rs` 354-358), or
   call the forced exit. No new saved field. But a door/side unload stops the vehicle (`transport.rs` 26-27); a tank must
   not stop. So: forced exit, or a "keep moving" branch. **S-M.**
5. AI. Every own vehicle with a transport def is taken out of the tank list into the transport job
   (`sim/ai/transport.rs` 145-148, 204). A T-34-85 with seats would stop fighting. It needs a "fights first, carries
   one squad" path. **M.**
6. Presenter: nothing per vehicle; riders draw at their transforms (`bridge/match_frame.rs` 952).

Pure data afterwards: seat boxes on the deck, capacity, timings; the def keys (`deck`, `dismount_on_hit`,
`dismount_on_fire`) enter `TransportSetup`, so the layout changes (section 6).

### 4.2 Body plate (SN-42) on the torso
**Exists.**
- Zones: `ZONE_TORSO/HEAD/LEGS/NONE` (`sim/wounds.rs` 22-27).
- `zone_at` from the hit height (107-119). It runs only with `[wounds]` on; otherwise every hit counts as torso
  (`combat.rs` 2472-2475).
- Damage: `wounds::deal` scales damage by zone (`wounds.rs` 144-154).
- Hit paths: the bullet at `combat.rs` 3461-3482 calls `hurt(.., w.damage, .., (zone, false))`; the fragment at
  `combat.rs` 3236-3245 calls `hurt_by(.., Some(zone))`.
- Small arms have a penetration table, `penetration = [[m, mm], ...]` (`defs/weapons/kar98k.toml` 20). It is lowered to
  `WeaponSetup::pen` (`content/setup.rs` 935) and read through the one `PenTable::pen0(range)` (`sim/armour.rs` 300-337).
- Fragments have `frag_pen_mm` (`defs/weapons/grw34.toml` "frag_pen_mm = 9").

**No armour on men today.** Per-man data comes from the member slot (`ents.kit[i].slot` → `MemberSetup`, as `is_medic` does,
`sim/medics.rs` 44-55). So a plate needs no saved man state.

**Change (S).**
- Content: a member key such as `plate_mm` with `plate_zones` (torso), on `MemberDef` (`content/variant.rs` 32-71) and
  `MemberSetup` (`content/setup.rs` 1198-1218). The trait byte has one free bit (7 of 8 used, `setup.rs` 1093-1120), but a
  thickness needs a field.
- Sim, bullet: before `hurt`, if zone = torso and the man's slot has a plate, compare `pen0(range)` of the weapon with the
  plate. Not beaten: no damage, suppression only. `Blow::Hit` carries no range (`combat.rs` 2053-2056; the event has
  `shooter`, 2083-2091), so add the distance as `Blow::Static` does (2079). This is a transient, not saved.
- Sim, fragment: compare `frag_pen_mm` with the fall-off already used for `frag_damage` (3228-3232).
- Not via `armour::penetrates`: that is the cannon rule with cosine and ricochet (armour.rs 234). The thin-plate small-arms
  rule is the closer model.

### 4.3 Captured ammo (Soviet 82 mm fires German 8 cm bombs)
**Today.**
- A weapon has one `feed`, a cartridge or ammo id (`defs/weapons/grw34.toml` 9).
- A cartridge is a supply kind and grams, with no faction (`content/setup.rs` 358-365; `sim/supply.rs` 157-166).
- A man carries rounds per feed slot, at most 4 feeds per squad type (`content/setup.rs` 336-338, 1331-1356).
  `kit.rounds[k]` is already saved per feed (`sim/supply.rs` 252-264).
- Refill draws grams of a kind from a source. Men take crates only of **their player's** faction: `crate_faction = own`
  (`sim/structures.rs` 1087-1091, 1144). Guns take only their def faction's stock (1100, 1114-1117).
- A captured crate changes owner but keeps its def faction (`structures.rs` 1013; S9 design `docs/slices/S9-at-guns.md`
  126-127).
- So today a Soviet mortar team cannot use a captured German crate. Nothing knows which bombs are "German".

**Minimal change (M).**
1. Weapon key `alt_feed = "ww2:8cm_wgr34"` with `alt_spread_pct` (e.g. 130). Squad feeds then hold both cartridges in
   separate slots (`setup.rs` 2373-2432 builds them). Fire spends the primary, then the alt with the wider spread.
2. A `faction` on the cartridge def (or on the alt feed). `source_for` compares a crate's def faction with that feed's
   faction, not the player's (`structures.rs` 1091, 1144). The refill picks per feed slot (1350-1365).
- No new runtime state: kit rounds per feed exist. New def keys change the setup layout (section 6).
- Simpler fallback: "may draw from enemy crates of the same kind" (one flag, S), with no accuracy cost.

## 5. Presenter and art

- Vehicle blockouts are procedural from the box list (`bridge/meshgen.rs` 444-874, `boxes_blockout` 615); no per-faction
  code. The GDScript fallback reads `GLACIS_DEG[faction]` (`vehicles.gd` 63).
- **Soldier tint:** `INSTANCE_CUSTOM.w` = **faction index** (`bridge/match_node.rs` 1288, 1568, 1643), looked up in a
  constant 10-colour table (`soldier_skinned.gdshader` 21-24, 80; `vehicle.gdshader` 7-15, 20). The USSR at index 2 gets
  `TEAM_COLORS[2]` (0.31, 0.34, 0.39, a grey-blue). For an earth-brown uniform, change that shader constant (data-ish,
  in GLSL). In a mirror both sides share one colour.
- **Vehicle tint:** `PAINT[faction]` mixed toward the own/enemy colour (`vehicle_view.gd` 126-129; `vehicles.gd` 59-62).
  Without an entry: a grey default.
- **HUD theme:** `S1b-ui-theme.md` 41-43 gives GER #46535b, USA #bdb07c and USSR #5e4a35 with emblems. None is wired:
  `palette.gd` 24-29 uses other values and no emblem code exists (grep for the hex and "emblem" in `godot/` finds none).
  Faction colour from the def (`FactionSetup::color_rgb`) reaches the HUD dict only (`match_node.rs` 3413-3438).
- Bridge sends `faction` short ids for vehicles and players (`match_node.rs` 293-296, 2023-2033): generic.

## 6. Save, protocol, tests and baselines

- **Versions.** `SAVE_VERSION` 30 (`sim/world.rs` 479; S13 branch: 31, its `world.rs` 484). `PROTOCOL_VERSION` 16
  (`crates/net/src/protocol.rs` 42).
- **New defs alone bump neither.** The setup layout is unchanged; the content differs, so `setup_hash` and state hashes
  differ (`content/setup.rs` 3-6).
- **Any new def key does bump SAVE.** Rider deck/dismount keys, `plate_mm`, `alt_feed` and a cartridge faction each enter
  `MatchSetup`, so SAVE goes +1, in one layout commit (the S13 pattern).
- **PROTOCOL stays.** No new command: Embark/Disembark exist (`sim/command.rs`; relay doc `sdk/docs/net/relay-protocol.md`).
- **Neutrality risk.** Every USSR def is lowered into every Germany-vs-USA match too (`setup.rs` 1922-1958), and shared
  sorted lists (weapons, ammo, cartridges, items, tags) get new entries in the middle, so indices shift. Proof wanted: the
  GER-USA 40 set equal match for match before and after the data commit (as S13 F did). I did not check whether any rule
  iterates the global lists in a result-changing order.
- **Append-sensitive tests and baselines touched:**

| Test | Why it breaks | Where |
|---|---|---|
| `setup_bytes_baseline` (6 hashes) | whole mod lowered | `content/tests/resolved_defs.rs` 197-207 |
| JSON snapshots (`BLESS=1`) | every def | `content/tests/snapshots/ww2_canonical.json`, `ww2_resolved.json` |
| S11a1 "cards last" | loops **every** faction and expects GER or USA ids (S13 edits it too) | `content/tests/s11a1_content.rs` 304-316 |
| aid def per faction | loops every faction | `content/tests/s12_content.rs` 225-230 |
| vehicle def count | `== 20` on main, `25` with S13 | `sim/s11a1_tests.rs` 247 |
| per-card position tests (listed cards only) | safe if GER/USA rosters stay | `s12_content.rs` 206; `s11b_content.rs` 451-457 |
| fixture `ww2_s3c` | own copy; untouched | `content/tests/ww2_mod.rs` 16-18, 77-80 |

## 7. Anything else beyond "mostly data"

- **AI.** No faction branch in `sim/ai*`. The mirror flaw is `ai/assault.rs` 465. The AI has no use for tank riders
  (4.1, item 5). Every habit runs on the USSR only if the USSR has the tag (4.0). Without a half-track (the card has
  riders instead) the transport habit simply idles.
- **Map.** Kessel Ridge has 2 bases (`defs/maps/kessel_ridge.toml` 94-101). 1v1 only; enough for 3 matchups.
- **Menu skirmish.** It cannot pick the opponent. "USSR vs Germany" from the menu needs an opponent field
  (`bridge/match_setup.rs` 16-37, 137-151) and a GDScript picker.
- **Three-way balance.** `workflows.md` 32-34: matchups measured on separate sets. That is 3 x 480 matches, about 3 x
  14 min (`STATE.md` "How to check work").
- **Date filter.** `[call_in] date` is "" (`skirmish.toml` 1248), so 1944 cards like the IS-2 are all fielded.
- **Not checked:** ammo/weapon content limits for 45/57/76/122 mm (should be data, as for S11b); the exact inner-box count of a
  T-34 with 8 riders; whether `bail.rs` re-crew/capture handles a seat-bearing tank (riders out by `transport.rs` 321
  first).

## 8. S12 / S13 overlaps (short)

- **S13** (`origin/claude/project-thread-aj1tww`, building) does these things:
  - appends 2 GER and 3 USA cards to the rosters (diff on `defs/factions/*.toml`);
  - edits `s11a1_content.rs` 304-316, `s12_content.rs`, `s11b_content.rs`, `s10a_content.rs` (roster offsets) and
    `sim/s11a1_tests.rs` 247 (25 vehicles);
  - refreshes `resolved_defs.rs` and `ww2_canonical.json`;
  - SAVE 31; changes `cli/skirmish.rs`, `json_out.rs`, `targets.rs`-adjacent tooling and `tools/ablate.sh`.

  S15 should start from main **after** S13 merges and re-read those tests. Its USSR cards go in its own roster, so GER/USA
  append order is untouched.
- **S12** (on main): hit zones and the free trait bit (4.2); aid post per faction (4.0); medics need a `medic`-tagged USSR card.

## Code work S15 needs (sized S/M/L)

1. **M** tank riders: allow `[transport]` on a tank (`content/vehicle.rs` 2523-2542, 4220); deck slot bounds (2584-2610);
   deck exposure bits (`sim/vehicle.rs` 917-928); hit/fire dismount without stopping (`damage.rs` ~408, `gunnery.rs` ~572,
   `transport.rs` 318-332); tests.
2. **M** AI for a rider tank: keep it in the tank jobs and carry one squad (`sim/ai/transport.rs` 145-148, 204).
3. **S** torso plate: member key, check in the bullet and fragment paths (`combat.rs` 3461-3482, 3236-3245), range in `Blow::Hit`.
4. **M** captured ammo: `alt_feed` + spread, feed faction in `source_for` (`structures.rs` 1087-1144); or **S** fallback flag.
5. **S** layout commit: every new key at its Off value, SAVE +1, baselines refreshed.
6. **S** front end: `FACTIONS` 3 entries; menu opponent field and picker (`bridge/match_setup.rs` 16-37, 133-161; `main_menu.gd`).
7. **S** measuring: faction fields in `RunSpec` (`cli/targets.rs` 99-122, 400-410), `batch.sh sets` / `measure.sh` pass-through,
   `pool.py --side` (45, 96), `ablate.sh` label (339).
8. **S** test fixes: `s11a1_content.rs` 304-316 loop; neutrality proof (GER-USA 40 set, match for match).
9. **S, optional** mirror: `ai/assault.rs` 465 (enemy = other player), bridge/relay/lobby allow, per-player tallies in the CLI.
10. **S, optional** USSR cases in armour-test / fortify benches (`cli/fortify/*`, `cli/armour_test.rs` 209).

## Pure data

- `defs/factions/ussr.toml` (id `ww2:ussr`, colour, roster, start squads).
- Units `defs/units/ussr_*.toml` with the AI tags; crew squads.
- Weapons, cartridges, ammo, items: PPSh, DP-27, SG-43, PTRD/PTRS (small-arms pen tables), 82 mm, 120 mm, 45/57/76 mm,
  122 mm, RGD-33 and so on.
- Vehicles: T-34-85, SU-85/100, SU-76M, BA-64B, IS-2, Studebaker, M3A1 scout car.
- Supply: HQ, dump, aid post, 4 crates, truck `[supply]`.
- Targets rows per matchup.
- GDScript tables: `palette.gd` 24-29, `session.gd` 57, `vehicles.gd` 59/63, `match_view.gd` 97-99; the shader
  `TEAM_COLORS[2]`; the bridge `FACTIONS` const.
