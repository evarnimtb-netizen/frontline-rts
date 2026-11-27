# S15 "USSR": code audit (read-only)

Working note for `S15-ussr.md` (2026-11-26, plan agent, read on `claude/project-thread-aj1tww` at `96d4b4b` = S14 with its Fix; SAVE 33,
PROTOCOL 17). Not a spec. Paths: `sim/` = `crates/sim/src/`, `content/` = `sdk/crates/content/src/`, `defs/` = `mods/ww2/defs/`, `cli/` =
`sdk/crates/cli/src/`, `bridge/` = `crates/godot-bridge/src/`, `net/` = `crates/net/src/`. PR 11 (`claude/project-thread-1d8xx3` at
`fa85a89`, S16-S18) was read with `git show` / `git diff`, not checked out; section 9 lists where the two meet.

**Bottom line.** The sim and the content crate do not know how many factions there are: a faction is a TOML file, every def names its
faction by id, the setup holds indices, and the AI buys from its own roster by tag, capability and weight. A third faction in the sim is
data. The number two is written down in four other places: (1) the **bridge and the menus** (`FACTIONS` constants, "the other
faction"), (2) the **relay and the lobby** ("the two factions must differ", `[String; 2]`), (3) the **tools and target files**
(`germany` as the pooled number, default CLI factions), (4) **tests and scenes** that name `ww2:germany` / `ww2:usa`. The three small
systems of the card (riders, plates, captured ammunition) each meet one refusing check, listed in 6-8.

## 1. Versions
- `SAVE_VERSION` **33** (`sim/world.rs` 492), `PROTOCOL_VERSION` **17** (`net/protocol.rs` 45). S15 adds saved counters and AI state: SAVE
  **34** on this branch. No new command (riders use `Embark` / `Disembark`), so **no PROTOCOL bump**.
- PR 11 also took 33 / 17 for S16a, then SAVE 34 (S17a) and SAVE 35 / PROTOCOL 18 (S18). Whichever merges second renumbers.

## 2. Content crate: factions are data
- `FactionDef` (`content/defs.rs` 1556): `id`, `name`, `color`, `roster`, `start_squads`; files `defs/factions/germany.toml`, `usa.toml`.
  `ModDefs::factions` is a `BTreeMap` (`content/modset.rs` 67); every index is `index_of(self.factions.keys(), ..)` (`content/setup.rs`
  1810, 2033, 2411, 2590): `ww2:germany` 0, `ww2:usa` 1, and **`ww2:ussr` sorts after `ww2:usa`: 2, the others unchanged**.
- All defs lower into every `MatchSetup` (`setup.rs` 1996-2040 builds every faction; the lists are "sorted by id", header 11-13), and the
  setup's postcard bytes are hashed (header 4-6). So adding USSR defs changes **every match hash** (as every data slice did) but, if every
  new id sorts after the existing ones of its list, **no existing index**. Units, vehicles and supply defs already carry a faction prefix
  (`ger_`, `usa_`); weapons, ammo, cartridges and items do not (`kar98k`, `75mm_m61`, `9x19`, `mk2_grenade`). An id starting `ussr_` sorts
  last in all seven lists (the highest ids today: `stuk40_l48`, `rpzb54_heat`, `m1a1_fuel`, `stielhandgranate`, `usa_tank_crew`,
  `usa_m8`, `usa_small_arms_crate`). Plan rule N1.
- Reference checks that already hold for any faction: a squad's faction exists and its vehicle is of it (`modset.rs` 1047-1065); roster
  entries belong to the faction (1267-1289); a vehicle's crew squad is a crew of its faction (1328-1344); a supply def's faction exists and
  a faction has at most one def per kind and one crate per supply kind (1356-1411); a truck's crates are its faction's (1419-1458); a
  card's tow is a vehicle card of its faction that can pull the gun (`setup.rs` 3554-3575).
- Date filter: `available_from` / `available_to` on cards against `[call_in] date` (`defs.rs` 178-180, `setup.rs` 1985-1992).
- Two-faction text: `MatchRequest::skirmish(seed, human, ai, ..)` (`setup.rs` 148-163) takes two ids (any two); players 2..=`MAX_MATCH_PLAYERS`
  and not more than the map's bases (`setup.rs` 1494-1506, 3625). A request with the same faction twice resolves (mirror runs in
  `kessel_ridge.toml` 22-25 used it).
- `Capability` list and reserved names: `content/variant.rs` 137 (`RESERVED_CAPABILITIES`), 142 (`RESERVED_TAGS`: `vehicle`, `crew`, `at`,
  `understrength`).

## 3. Sim: where the faction index is read (all by index, none by name)
- Call-in: the card must be in the player's faction roster (`sim/game.rs` 1144); start squads per faction (`game.rs` 2018-2019);
  `sim/rules.rs` 214.
- Supply: structures and crates carry their def's faction; a man or gun draws only from stock of the right faction
  (`sim/structures.rs` 405, 616-620, 701, 1042-1043, 1085-1150: `crate_faction = gun_faction.unwrap_or(own)`); an enemy's set-down crate
  can be picked up and keeps its faction (`structures.rs` 826-829); base rearm skips a served gun of another faction (`sim/supply.rs`
  908-911); gun capture compares factions (`sim/guns.rs` 657). **This is the rule X1 (captured bombs) has to open, for one weapon.**
- Demolish: `sim/demolish.rs` 102-107.
- AI, all through `s.factions[s.players[me].faction].roster`: the buy mix by tag (`sim/ai.rs` 1782-1800, `[ai.mix]` in
  `defs/rules/skirmish.toml` 932-938: `line`, `mg`, `engineer`, `mortar`, `at`, `vehicle`); builder card (`ai.rs` 1762-1775); heavy card by
  `ai_heavy` (`ai.rs` 2310-2325); kinds by the card's type word (`ai.rs` 1157-1168); guns (`ai/guns.rs` 999, 1189), artillery
  (`ai/arty.rs` 304, 351, 1060; tag `field_howitzer`, capability `FireMission`), medics (`ai/medic.rs` 296, 386; tag `medic`), supply
  (`ai/supply.rs` 67, 142, 394, 421), trucks (`ai/truck.rs` 950), forts (`ai/fort.rs` 292, 1540), flame (capability `Flame`). The enemy's
  roster is read as doctrine knowledge in two places: `ai/assault.rs` 464-472 (strongest enemy MG among the factions **in the match**)
  and `ai/guns.rs` 999 (the enemy's armour for corridor routes). Both already work for any faction in play.
- **Finding.** No non-test sim file names a faction or a unit id. `scenarios.rs` (test and CLI scenes) says "German" / "US" in messages
  (1169-1293) and picks cards by role from player 0 and 1's rosters (115, 360, 469, 788-793, 1192, 1308): scenes work for any pair.
- Not audited here: whether the AI tolerates an **empty slot** of its mix (a faction with no card of a tag). The USSR roster fills every
  slot, and Build B has a test for a roster with one slot empty.

## 4. CLI, JSON, tools, targets
- `skirmish`: `--faction0` (default `ww2:germany`) and `--faction1` (default `ww2:usa`) (`cli/skirmish.rs` 75-80, 173-174); `--swap-bases`,
  `--both-bases` (97, 138-140, 2412-2415, 4179-4200); the map is fixed (`skirmish.rs` 2393; PR 11 adds `--map`). Reports and summaries key
  on the faction id (`FactionSummary`, 3473-3539; `BTreeMap` by id, 4240-4286), so a third faction needs no report change.
- JSON: `wins.<short>` and `wins.<short>_pct`, `factions.<short>` (`cli/json_out.rs` 37-39, 228-253): `ussr` appears by itself.
- Hard-coded pairs: `cli/validate.rs` 135-139 resolves "the first two factions" only (a USSR def error in resolve would not be reported by
  `validate`); `cli/armour_test.rs` 210 and the fortify scenes start from `MatchRequest::skirmish(seed, "ww2:germany", "ww2:usa", ..)`
  and pick cards by role (`cli/fortify.rs` 435-437); `cli/skirmish.rs` tests 5107-5113, 5187-5193 find `ww2:usa` by name.
- Tools: `tools/batch.sh sets` (119-131) runs the default pair only; `tools/pool.py` 36-37 (`BOUGHT` per `germany` / `usa`), 49 (`w.get
  ("germany")`), 103 ("Germany pooled"); `tools/ablate.sh` 366-386 (Germany gate); `tools/autotune` (`share_germany`, `autotune.py` 385);
  `tools/neutral.py` is faction-blind (keys by seed and side).
- Target files name `wins.germany_pct`, `factions.germany.*`, `factions.usa.*` (1,035 lines of `targets/S14.toml` do).

## 5. Bridge, net, presenter
- `bridge/match_setup.rs` 69: `pub const FACTIONS: [&str; 2] = ["germany", "usa"]`; 138-142: the AI gets "the other one"; 156-158: a
  slot's faction must be in `FACTIONS`; 160-161: both slots the same is refused. `bridge/match_node.rs` 898 (default `germany`), 1293
  (`team_tint` = the faction index as the shader tint), 1924-1960 and 2410 (the local faction's mortar and items), 2034-2044 and 3496-3521
  (faction short ids into the frame). `bridge/net_session.rs` 321 (a lobby pick must be in `FACTIONS`), 349-357 and 563-567
  (`[String; 2]`), 448 (default pick `FACTIONS[slot % 2]`), 622-627 ("factions must differ").
- `net/protocol.rs` 232, 310 (faction id as a string, `MAX_ID` 63), 325 (`Start::factions: [String; 2]`); `net/relay.rs` 890-893 ("both
  players must pick a faction", "the two factions must differ"); `net/harness.rs` 277-310 (the test setup, Germany and USA). The relay
  never loads defs: it cannot know which pairs are allowed; the clients' equal setup hash (`Ready`) is the check that both resolved the
  same match.
- Godot: `godot/autoload/session.gd` 57 (`FACTIONS`), 69, 146-148, 440; `godot/ui/palette.gd` 25-29 (colours, names, short names);
  `godot/ui/menu/main_menu.gd` 19, 149-164, 242 (faction buttons, start); `godot/ui/menu/multiplayer_panel.gd` 328-346, 527-533, 563
  ("Factions must differ"); `godot/match/match_view.gd` 98-99 (weapon names per faction for five roles), 752; `godot/match/vehicle_view.gd`
  160; `godot/match/vehicles.gd` 59 (`PAINT`), 63 (`GLACIS_DEG`); `godot/match/hud/top_bar.gd` 85, `end_screen.gd` 96-99; mocks
  (`mock_vehicles.gd`, `mock_net.gd`, `mock_match_source.gd`). No Godot file outside the mocks names a unit id.

## 6. Riders (R rules): what exists
- `[transport]` and `[[passenger_slots]]` (`content/vehicle.rs` 656-662, 779-793): a slot box is the rider's hit volume (S8a).
  `VehicleDef::validate` refuses them on anything but `role = "transport"` or a tractor (`vehicle.rs` 2557-2577): **the one content check
  riders must relax.** `TransportSetup` (3100-3108) and `MAX_PASSENGERS`.
- `sim/transport.rs`: `transport_of` (80), `boardable` (150-159: own, live, not burning, not unloading), `order_embark` (162-204),
  `order_disembark` (207-246), `under_fire` (260: a penetration within `[ai.transport] hit_s` or a rider past `rider_suppression_pct`, the
  AI's rule only), exits (`rider_out` 535, with `stun_ms` on a knock-out, 587-589). Riding men do not capture (S8a). **No sim rule gets
  riders off by itself except a knock-out, abandon or fire** (module docs 54): R3 is new.
- `VehicleState::riders` is sized by the slots, so a tank with no rider slots saves exactly as today.

## 7. Plates (P rules): what exists
- Hit zones: `sim/wounds.rs` 23-27 (`ZONE_TORSO`, `HEAD`, `LEGS`, `NONE`), `zone_at` (108-119: prone men are all torso), `deal` (125-180:
  the zone factor, then HP). Every bullet, fragment, flame and strike reaches a man through `hurt` -> `deal` (S12, S14).
- A man's facing is in his `Transform`; a round's direction is its velocity at the hit: the front test needs no new state.
- Round strength: small arms have `penetration = [[m, mm], ..]` only where S8a needed it (`defs/weapons/kar98k.toml` 20: 7 / 5 / 3 mm at
  0 / 100 / 300 m; `m2hb.toml` 27); pistol-calibre weapons (`mp40`, `p38`, `m3_smg`, `m1911a1`) have **no table**. Fragments carry
  `frag_pen_mm` (`grw34.toml`: 9). Adding tables to the pistol-calibre weapons would touch the S8a thin-plate rule for Germany and the
  USA, so the plate rule keeps its own value for a weapon without a table (plan P2).
- Members have no armour field; `Kit::slot` (`sim/items.rs` 90-92) gives a man's member slot, so a plate needs no per-man state.

## 8. Captured ammunition (X rules): what exists
- A mortar's rounds: `feed = "ww2:8cm_wgr34"` on the weapon (`defs/weapons/grw34.toml`), `Feed::Cartridge` (`content/setup.rs` 353), the
  cartridge's `supply = { kind = "shells", g = 3500 }`. The HE values are on the **weapon**, not the cartridge: an 82 mm firing a German
  bomb bursts with the 82 mm's `[he]` (close enough; said in the plan).
- Stock is kilograms per supply kind and faction, not rounds per calibre: a taken German shells crate is "German shells, n kg". X1 lets
  one weapon draw from it; nothing else changes.
- `Kit` (`sim/items.rs` 87-100) is the per-man carried state; how it holds rounds per cartridge was not read to the end: Build A reads it
  and the layout commit carries the count of foreign rounds either in the existing per-cartridge store or in one new field.

## 9. PR 11 (S16 second map and roads, S17 teams, S18 online teams): where S15 meets it
PR 11 changes 138 files from the common base `a27ce63`. Its plans name S15 as a neighbour (`S17-team-play.md` 219-222, S17-8;
`S18-online-teams.md` 37, 78: "1v1 follows S15-10", "collision with S15: lobby opponent picker, mirror rule at `relay.rs` 890-893").

| File | PR 11 | S15 | Meeting |
|---|---|---|---|
| `bridge/match_setup.rs` | `slots: Vec<SlotChoice>` with `team` and `difficulty`, `mode` 1v1 / 2v2, `FACTIONS` kept at two (its line 96), "the other faction" kept (200-204) | the faction list from the defs, the opponent pick | same function `resolve_with`: the second merger rewrites its hunk on the other's |
| `net/relay.rs`, `net/protocol.rs` | four slots, teams, `--distinct-factions`, `allow_same_faction`, PROTOCOL 18 | no protocol change; the relay rule stays | S15 touches no relay line if PR 11 is first; if S15 is first it changes nothing there either |
| `bridge/net_session.rs` | team lobby (+328 lines) | `FACTIONS` -> the defs' list at 321, 448 | small textual conflict |
| `godot/autoload/session.gd`, `ui/menu/main_menu.gd`, `multiplayer_panel.gd`, `ui/palette.gd` | maps, modes, ally colour, 2v2 lobby | third faction button, opponent pick, USSR colour and names | every hunk is near one of PR 11's |
| `cli/skirmish.rs` | `--map`, `--teams`, `--faction2/3`, team reports (+552 lines) | nothing but tests (the flags exist) | none expected |
| `tools/batch.sh`, `pool.py`, `measure.sh`, `neutral.py` | a map per set, team sets | a pair per set | both add a parameter to the same `sets` loop |
| `content/setup.rs`, `defs.rs`, `tuning.rs` | `team`, `[teams]`, `[ai.team]`, roads | `[matchups]`, `[riders]`, `[body_armour]`, plate and rider-slot defs | appended tables; conflicts only where both append at a file's end |
| `sim/` | 60 files swept to `crate::teams` (hostile, allied, sees); `tools/owner_sites.sh --check` runs in `check.sh quick` and fails on a raw owner comparison that `docs/slices/S17-sites.md` does not list | new rules in `combat.rs` (plates), `transport.rs` (riders), `structures.rs` (X1), `ai/riders.rs` | S15's new sites must be in the table or use `teams` after the merge |
| `content/tests/snapshots/ww2_canonical.json`, `resolved_defs.rs` | new snapshot of the mod | every USSR def lands in both | regenerate after the merge (`BLESS=1`), never hand-merge |
| `defs/rules/skirmish.toml` | `[vehicles.road]`, `[teams]`, `[ai.team]` appended | S15 tables appended | append conflict at the file's end |
| `docs/STATE.md`, `docs/TODO.md`, targets numbering | S16 uses T201-T203 | S15 starts at T210 | renumber on the second merge |

Team modes and S15: PR 11 lets any faction sit in any slot (its test puts Germany and the USA on one team), so the pair rule of S15 is
defined on **hostile** players only and reads `teams` once both are merged (plan M1).
