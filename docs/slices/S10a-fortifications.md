# S10a: Fortifications and breaching (mechanics half)

Status: spec (Plan, 2026-11-10). Branch `s10a-fortifications` from `9753cd9` (main `ba0038f` plus docs). Targets
`docs/slices/targets/S10a.toml`; log `docs/slices/S10a-log.md` (each Build stage writes its hand-over there). Design:
`roster-expansion.md` "S10"; `infantry-structures.md` 3.3, 7.1, 7.2, 7.6, 7.7; `SYSTEM_DESIGN.md` 5.10.3-5.10.9.
MECHANICS slice (owner 2026-11-09): new cards at `ai_weight` 0, the AI uses nothing new, AI-vs-AI play is unchanged.

## 1. Scope
**In.** Blueprints and pieces (boxes with an armour value and hit points); materials (`construction` kg carried,
crated, trucked); sandbag wall, MG nest, dugout, HQ roof; barbed wire (infantry at 20 % until cut or blown); wire
cutting; the Bangalore torpedo; tiers field / assault / construction; cards Sturmpioniergruppe, Baupioniergruppe, US
Assault Section; commands, save, hash, net, bridge, blockouts, build menu; CLI counters, fortify rows, neutrality check.

**Out, S10a AI half** (next): AI use of anything new, `ai_weight` above 0, the design's AI done-when line, balance,
prices. **Out, S10b:** mines and detection, AT obstacles, the timber-earth bunker, `place_mines`, `clear_mines`,
`demolish`. **Out:** flamethrowers (S14), structure selection (S6-18), auto-flatten, tunnels.

## 2. Where the design and the code disagree (code and owner decisions win)

1. The S6 HQ is a 1.5 m pit, no roof, 100 MP, no material (S6-4 (A)), and the AI builds it; a required roof would
   change AI play, so the HQ dugout is an optional upgrade over a built HQ (S10a-4).
2. 7.1 `forbidden_materials`, `needs_flat`: no material layer or Flatten op in `terrain.rs`. Left out.
3. 3.3 capability strings: code has `Capability` bits and `variant.rs` `RESERVED_CAPABILITIES`; five move (3.4).
4. 6.3 `construction` as a fourth stocked kind: the S6/S8b arrays are three wide and reach the CLI JSON, so
   `SupplyKind::Construction` never enters them (HQs and dumps never stock it; trucks get their own field, 3.3).
5. `PlanFortification` (5.10.3) is the command `Fortify`; "Lv 1 with carried bags" is S10a-1; camo net drawn only.
6. "Breaks under heavier HE": nothing heavier than the 81 mm exists before S11; a satchel and a test-only 105 mm do.

## 3. Data model

### 3.1 Blueprint defs: `mods/ww2/defs/fortifications/*.toml` (new folder; `FortDef` in `sdk/crates/content/src/defs.rs`)

```toml
id = "ww2:mg_nest"              # faction-free: both factions build the same blueprints
shape = "point"                 # point | line (one piece per segment) | over_hq (snaps to an own built HQ)
pit = "mg_nest_pit"             # optional dig profile dug first (`Order::Dig`, as S6 Build)
capability = "build_structure"  # or place_wire
materials_kg = 60               # point: whole work; line: per segment
work_s = 90                     # at 100 % with work_men men (compressed realism, SYSTEM_DESIGN 5.10.9)
work_men = 4
limit_per_player = 4            # sites (point) or pieces (line), built or building; line: segment_cm, max_length_m
min_spacing_m = 10              # point only
max_slope_pct = 25              # design 7.1
pieces = [ { part = "sandbag", size_cm = [60, 220, 60], offset_cm = [130, 0, 0], armour_mm = 100, hp = 400 }, ... ]
# a piece may set z_from = "base" (z from the undug surface: a roof over a pit); wire defs: wire = { band_cm, height_cm, hp }
```

Lowered to `MatchSetup::forts: Vec<FortSetup>` (`setup.rs`; `PieceSetup`, `WireSetup`). `part`: `sandbag`, `roof`,
`timber` (colour only). Validator: 1..=4 pieces, sizes 10..=600 cm, `armour_mm` 1..=2000, hp 1..=10 000; a
`place_wire` def has `wire` and no pieces; an `over_hq` def has no `pit`.

| Def | Shape, pit | Pieces | kg | Work | Limit | Source |
|---|---|---|---|---|---|---|
| `sandbag_wall` | line, 2 m segments, max 20 m | 1 x `sandbag` 200x60x90 cm, armour 100, hp 400 | 30 / seg | 20 s / seg, 5 men | 40 seg | 7.6 (20 s per 2 m, 30 kg) |
| `mg_nest` | point, `mg_nest_pit` | 3 x `sandbag` ring, 60 cm high | 60 | 90 s, 4 men | 4 | 7.6, 5.10.9 |
| `dugout` | point, `dugout_pit` | 1 x `roof` 320x270x70 cm, `z_from = "base"`, armour 100, hp 250 | 120 | 360 s, 6 men | 3 | 7.6 (6 min, 120 kg), 5.10.3 (2 m pit, 0.5 m earth) |
| `hq_dugout` | over_hq | 1 x `roof` 320x270x70 cm over the HQ pit, as above | 120 | 120 s, 5 men | 1 | 7.2 (120 kg, S6-4); time verify |
| `barbed_wire` | line, 2 m sections, max 30 m, `place_wire` | wire band 150 cm, height 90 cm, hp 200 | 3 / sec | 6 s / sec, 2 men | 50 sec | 7.6 and 5.10.9 (30 s and 15 kg per 10 m) |

Unsourced numbers (sizes, hp, limits, spacing, HQ roof time, wire band) are starting values marked `verify`. Two
follow from rules: roof hp 250 lies between the 81 mm `damage` 150 (`grw34.toml`, `m1_mortar.toml`) and the satchel's
300 (done-when line 4); `armour_mm` 100 is above every `frag_pen_mm` (12 max, S8a-13) and every small-arms pen (7.6).

### 3.2 Rules: `[fortify]` in `defs/rules/skirmish.toml` (`FortTuning` in `tuning.rs`, a `tuning_table!`, so `--set` works)

```toml
[fortify]
wire_speed_pct = 20          # % of a man's speed inside a standing wire band (roster S10 done-when; 7.6)
wire_path_cost_pct = 500     # A* cost of a wire node for infantry (1 / 20 %)
wire_frag_pct = 25           # % of frag_damage a fragment crossing a wire section deals to it (verify)
cut_s = 10                   # s per wire section at 100 % (7.6: 10 s per gap)
cut_reach_m = 25             # CutWire takes the nearest standing section within this of its point
material_reach_m = 15        # builders draw from an own construction crate or truck this close (7.1)
carry_kg_per_man = 20        # construction kg one man carries (verify; 7.1 "each engineer carries some")
construction_mp_per_t = 100  # price at base (verify; owner D1: every kg priced once, at base)
shot_piece_damage = 150      # hp a non-bursting main-gun round takes from a piece (= supply.shell_box_damage)
[fortify.tiers]              # % work rates by tier (7.7: assault clears fast, construction builds fast, clears slow)
field = { build_pct = 100, cut_pct = 100 }
assault = { build_pct = 0, cut_pct = 200 }
construction = { build_pct = 150, cut_pct = 50 }
```

Pits (`DigProfile::MgNestPit = 6`, `DugoutPit = 7`, `engineers_only`, all-round spoil, S6 rate 5,250 points per m3):
`mg_nest_pit` 2.2 x 2.2 m, 90 cm deep (verify); `dugout_pit` 3.0 x 2.5 m, 200 cm (5.10.3 "2 m deep pit").

### 3.3 Materials

* `SupplyKind::Construction = 3`; `ALL` and `NAMES` stay the three ammo kinds; no three-wide array is indexed with it.
* **Carried:** `LoadMaterials { squad, kg }` in the own base, a `build_structure` or `place_wire` squad: up to
  `carry_kg_per_man` x living men, paid `kg x construction_mp_per_t / 1000` (`MatchState::pay`; short: refused).
  Held in `FortState::carried_g`; no speed effect (S10a-5); never loaded automatically.
* **Crated:** `ger_construction_crate`, `usa_construction_crate` (`supply = "construction"`, 40 kg, `carry_speed_pct`
  70, verify) by `IssueCrate { kind: Construction }`, inside `crate_cap`; never emptied into a stock (`stock_in`).
* **Trucked:** `LoadMaterials` on an own truck's squad in base fills `VehicleState::material_g` (new) up to
  `cap_kg_total` less cargo, paid the same; `load_kg` and auto-loading unchanged.
* A work draws its kg when its build step starts: carried, then own crates within `material_reach_m` (ascending id),
  then own trucks. Short: the work waits (`NoMaterials` on the site) until a draw completes it.

### 3.4 Capabilities and tiers

`CAPABILITIES` grows from 7 to 12: `build_structure` 7, `build_heavy` 8, `place_wire` 9, `clear_wire` 10,
`breach` 11 (`Capability` bits, `MASK` 12 bits). `place_mines`, `clear_mines`, `demolish` stay reserved (S10b).
Tier: `build_heavy` gives construction, else `breach` assault, else any of `build_structure`, `place_wire`,
`clear_wire` field. Builders add their tier's `build_pct`, cutters its `cut_pct`; `dig_rate_pct` applies to digging only.

| Card (file) | Men, kit | Caps | MP, cooldown | Source |
|---|---|---|---|---|
| Pioniergruppe, US Engineer squad (existing) | unchanged | + `build_structure`, `place_wire`, `clear_wire` (field) | unchanged | 7.7 field engineers |
| `ger_sturmpioniergruppe` | 8: leader MP 40, 3 StG 44, 4 MP 40; 2 `gestreckte_ladung`, 2 `geballte_ladung`, frags | `dig_basic`, `clear_wire`, `breach`, `recrew` | 264, 60 s | roster 3.1 (320 MP x 0.8 S5a-37 x 1.033 S6 tune) |
| `ger_baupioniergruppe` | 8 Kar98k, `conscript` training | `dig_basic`, `dig_full`, `build_structure`, `build_heavy`, `place_wire`, `clear_wire` | 165, 60 s | roster 3.1 (200 MP, same factors) |
| `usa_assault_section` | 12: leader M1, 5 M1, BAR + asst, M9 bazooka + asst, 2 demolition men with `m1a1_bangalore` | `dig_basic`, `clear_wire`, `breach`, `recrew` | 380, 60 s | roster 3.2 (no cut for US) |

All three: `ai_weight = 0`, tags `["engineer"]` plus `assault` or `construction` (the AI mix skips weight 0, `ai.rs`
`buy`), appended at the end of each faction roster (the S7-S9 rule: existing indices and tie-breaks unchanged).

### 3.5 Bangalore: `ItemKind::Bangalore = 4` (`ITEM_KINDS` 5), items `gestreckte_ladung`, `m1a1_bangalore`

Placed, not thrown. `[he]` as the satchel (damage 300, blast 150 cm, no trench cut), `length_cm` 300, `bursts` 3
(spaced along it, one tick), `place_s` 5, `fuse_s` 8, explosives `supply.g` 13 000 (all verify; M1A1: 5 ft sections).

## 4. Sim state, commands, save and hash

New module `crates/sim/src/fort.rs`; `MatchState::fort: FortState` (saved, hashed, validated on load).

```rust
pub struct FortState { works: BTreeMap<u32, Work>, next_work: u32, pieces: BTreeMap<u32, Piece>, next_piece: u32,
    wire: BTreeMap<u32, WireSection>, next_wire: u32, charges: Vec<Charge> /* ascending id */, next_charge: u32,
    carried_g: BTreeMap<SquadId, u32>, accounts: Vec<FortAccount> /* per player */ }
pub struct Work { owner: PlayerId, fort: u16, line: Vec<FxVec2>, yaw: Fx, builder: SquadId, hq: Option<u32>,
    step: WorkStep /* Dig | Wait | Build */, seg: u8, work: u64, got_g: u32 }
pub struct Piece { owner: PlayerId, fort: u16, part: u8, pos: FxVec2, yaw: Fx, z: Fx, hp: u32, hq: Option<u32> }
pub struct WireSection { owner: PlayerId, fort: u16, a: FxVec2, b: FxVec2, hp: u32 }
pub struct Charge { id: u32, owner: PlayerId, item: u16, from: FxVec2, to: FxVec2, fuse_tick: u64 }
pub struct FortAccount { works_started, pieces_built, pieces_lost, wire_built, wire_cut, wire_blown, wire_crushed,
    bangalores: u32, materials_loaded_g, materials_micro, materials_used_g: u64 }
```

`Order::Fortify { work: u32 }` and `Order::CutWire { section: u32, man: Option<SimId>, work: u64 }` (appended to
`Order` and `OrderKind`); `VehicleState::material_g: u32`. Commands (appended to `CommandKind`, `command.rs`):

| Command | Rule |
|---|---|
| `Fortify { squad, fort: u16, line: Vec<FxVec2> }` | point and over_hq: `[pos, toward]`; line: 2..=16 points, at most `max_length_m`. Refused (`FortEvent`, presentation only, like `SupplyEvent`): not own squad or no capability (`CannotBuild`), cap, spacing, slope, off map or in an enemy base (`BadSpot`), over_hq with no own built HQ within 5 m or one already roofed. Accepted: `works_started` += 1; pit blueprints dig first through `order_dig` (as `order_build`), then `Order::Fortify` |
| `LoadMaterials { squad, kg: u16 }` | section 3.3 |
| `CutWire { squad, at: FxVec2 }` | `clear_wire` squad; the nearest standing enemy or own section within `cut_reach_m` of `at` |
| `PlaceBangalore { units: Vec<SimId>, target: FxVec2 }` | the nearest man carrying a Bangalore in a `breach` squad walks (detached) to within 2 m of `target`, works `place_s`, lays the charge from his spot toward `target`; it bursts at `fuse_tick` |
| `IssueCrate { kind: Construction }` | existing command, new kind |

A work whose builder takes another order or dies is removed, its kg lost (S6-8, S10a-6). Line segments stand one by one.

**Versions.** One layout commit at the start of Build A adds every new state field, enum variant (`Order`,
`CommandKind`, `SupplyKind`, `ItemKind`, `DigProfile`, `Capability`) and setup field, and bumps `SAVE_VERSION` 21 ->
**22** (`world.rs`) and `PROTOCOL_VERSION` 12 -> **13** (`net/src/protocol.rs`); no later stage changes the layout.
Hash: `put_postcard(&g.fort, out)` after `supply6` (`hash.rs`). The setup bytes are hashed too, so **every state hash
changes** even when nothing is built. `FortState::validate`: ids below `next_*`, defs, owners, map, hp, live own
builders and `carried_g` squads within capacity, charges ascending with a live item, one account per player, caps.

## 5. Physical rules (damage only by projectile or fragment contact; all values in TOML)

1. **Pieces are static boxes.** `StaticRef::Piece(id)` boxes are appended in `rebuild_statics` after the S6 boxes and
   crates (S6 indices and ray-order ties unchanged). Bullets, shells and fragments stop at the first enemy piece in
   their path (the existing `first_static` and fragment code); own rounds pass own pieces (S10a-3).
2. **Damage gates (pieces only; S6 boxes unchanged).** A bullet takes `supply.bullet_box_damage` only if its pen at
   that range beats `armour_mm` (`armour::small_reach`); a non-bursting main-gun round takes `shot_piece_damage`; a
   fragment counts only if its `frag_pen_mm` there (S8a thin-plate rule) beats `armour_mm`; overpressure with a clear
   line takes the HE's `damage` (S6 static rule). 0 hp: removed (`pieces_lost`).
3. **Overhead cover.** Overpressure on a man needs a line clear of terrain and of every piece not owned by the
   burst's player (`piece_between(lift, body)`, pieces only, a new check in `burst`); fragments already stop at
   pieces. A burst on a roof is above `crater_lift_mm`, so it stamps no crater (existing rule).
4. **Wire.** A man within `band_cm / 2` of a standing section moves at `wire_speed_pct` of his speed (per man, in
   the steering where `carry_speed_pct` applies). Infantry A* pays `wire_path_cost_pct` per wire node (a `path.rs`
   layer built only when wire exists, like the AT-ditch layer). Overpressure with a clear line takes the HE's
   `damage`; a fragment crossing a section below `height_cm` deals `frag_damage x wire_frag_pct / 100` and flies on;
   a vehicle hull crossing a section breaks it (`wire_crushed`); bullets pass. 0 hp: removed.
5. **Cutting.** `Order::CutWire`: the man nearest the section walks to it; within 1.5 m he adds his tier's `cut_pct`
   (100 % finishes in `cut_s`); the section is removed (`wire_cut`). Another order ends it.
6. **Bangalore.** At `fuse_tick` a charge throws `bursts` HE bursts of its item, evenly along `from -> to`, each
   through the normal `burst` (blast, fragments, wire and pieces as above). Sections it removes count `wire_blown`.
7. **Build work.** As `dig_work`: each builder within `dig.common.reach_mm` adds `build_pct` per tick toward
   `work_s x work_men x 100 x tick_hz`; suppressed x0.3, pinned stops. An HQ roof piece goes with its HQ (`hq`).
8. **Empty fast path.** Every new per-tick system returns at once when `FortState` holds nothing (no RNG draw, no
   list build); new RNG uses its own purpose keys. This is what keeps AI-vs-AI matches identical.

## 6. Presenter, HUD, bridge, net

* **Snapshot** (`snapshot.rs`): `FortSnapshot` (pieces with frame and hp %, wire sections, works with step and
  progress, charges), `SquadSnapshot::materials_kg`, `fort_refusals`; `godot/docs/match_state.md` "S10a" section.
* **Bridge** (`match_node.rs`, `intents.rs`): intents `fortify` (`what`, `line` or `target` + `facing`),
  `load_materials`, `cut_wire`, `bangalore`, `issue_crate` `construction`; first selected squad able, as `build`.
* **HUD** (`orders_grid.gd`): build-menu entries `build_sandbags`, `build_wire` (dragged lines as `dig`),
  `build_mg_nest`, `build_dugout`, `roof_hq`, `load_materials`, `cut_wire`, `bangalore`; tooltips give kg, time, caps;
  shown only to able squads; refusals in the event feed.
* **Blockouts** (`godot/match/fort_view.gd`, new): pieces as boxes (sandbag tan, roof timber), wire as posts every
  2 m with a band, sites as ghosts with a progress ring, charges as a pipe. No selection (S6-18 stays open).
* **Net**: `command_in_limits` arms for the four commands (`line` <= `MAX_DIG_POINTS`, `units` <= `MAX_UNITS`);
  `sdk/docs/net/relay-protocol.md` lists them; the fuzz test round-trips them.

## 7. Tests (named; sim tests in `crates/sim/src/s10a_tests.rs` and `crates/sim/tests/fortify.rs`)

Build A: `fortify_refused_without_capability`, `fortify_refused_at_cap_spacing_slope_and_enemy_base`,
`load_materials_paid_in_base_only`, `materials_drawn_carried_then_crate_then_truck`,
`work_waits_without_materials`, `construction_crate_never_enters_a_stock`, `sandbag_wall_builds_segment_by_segment`,
`rifle_rounds_do_not_damage_sandbags`, `he_and_shot_damage_pieces`, `fragments_below_armour_do_not_damage_pieces`,
`own_rounds_pass_own_pieces`, `interrupted_work_is_removed`, `mg_nest_digs_its_pit_then_builds_its_ring`,
`dugout_roof_survives_an_81mm_hit` (`World::drop_shell_now` on the roof centre: roof hp > 0, five men under it lose
nobody), `dugout_roof_breaks_under_a_satchel` (`drop_item_now`), `dugout_roof_breaks_under_heavier_he` (a test-only
105 mm def, `damage` 400, verify), `hq_roof_only_over_an_own_built_hq`, `hq_roof_goes_with_its_hq`,
`fort_state_saves_loads_and_hashes`, `load_rejects_bad_fort_state`, `empty_fort_state_changes_nothing`
(600 AI ticks with nothing built: entity state equal with the fort stage on and skipped by a test-only switch).
Build B: `wire_slows_infantry_to_wire_speed_pct` (exactly 20 % of the same man's speed outside),
`cut_gap_restores_full_speed`, `infantry_path_goes_through_a_gap`, `vehicle_crushes_wire`,
`he_burst_cuts_wire_in_blast_radius`, `fragments_damage_wire_and_fly_on`, `cut_wire_needs_clear_wire`,
`cut_time_follows_tier` (field 10 s, assault 5 s, construction 20 s), `bangalore_bursts_along_its_length`,
`bangalore_clears_a_gap_and_counts_blown`, `bangalore_needs_breach_and_an_item`; content tests
`s10a_cards_resolve_with_ai_weight_zero`, `promoted_capabilities_parse`, `fort_defs_validate`.
Build C: net `fortify_commands_in_limits` and fuzz; bridge `fortify_intents_make_commands`; snapshot
`fort_snapshot_lists_pieces_wire_sites_charges`; CLI `s10_counters_zero_without_fortifications`, fortify rows below.

**fortify-test rows** (new `s10a` block in the fortify JSON, `sdk/crates/cli/src/fortify.rs`; existing rows and
`pass` unchanged), 10 seeds x both factions:
* `wire`: a rifle squad crosses a 20 m wire line (staged by a new `World::build_now(fort, line, owner)`): leader's
  speed inside / outside the band `inside_speed_pct`; through a `CutWire` gap and a Bangalore gap
  `cut_gap_speed_pct`, `blown_gap_speed_pct`. Design done-when "wire slows infantry to 20 % until cut or blown".
* `roof`: a dugout with five men; an 81 mm bomb on the roof centre (`survived_81mm_pct`, `sheltered_losses`), a
  satchel on it (`broken_heavy_pct`). Design done-when "a dugout roof survives an 81 mm hit ...".
* `breach`: the fortify trench with a rifle squad, an MG team in an MG nest beside it and 30 m of wire 35 m out
  (design 7.6 "30-50 m out") against the assault card (Sturmpioniere / Assault Section) advancing from 150 m with an
  MG suppressing as in the S3b suppressed assault; the engineers blow the wire with a Bangalore, go through the gap,
  throw and close in; 240 s. `defender_loss_pct`, `attacker_loss_pct`, `gap_opened_pct`, and a `control` without
  wire and nest; reported against the S3b bar (T10: defenders >= 75 %, attackers <= 50 %), not pass conditions.

## 8. Neutrality proof (new cards at `ai_weight` 0, no AI use)

The three 160 sets (`tools/measure.sh`) on the S10a head must give exactly the per-match outcomes of base `9753cd9`
(binary of `1bfc92f`). The state hash cannot show it: it covers the setup bytes, which the new defs change. New tool
`tools/neutral.py BASE_DIR NEW_DIR` (Build A; its unittest `tools/test_neutral.py` on small fixtures runs in
`tools/check.sh quick`), over `skirmish40/160/160_oos/160_s201.json` present in both:
* `per_match` records keyed by `(seed, side)` equal in every field but `hash` (`result`, `winner_faction`,
  `winner_base`, `length_s` to the tick, `he_share_pct`, `tanks_lost`, `rockets_fired`, `satchels_at_vehicles`, ...);
* the aggregate blocks (`factions`, `wins`, `length_min`, `he_share_pct`, `s3c`, `s6`, `tank_losses`) equal
  recursively, ignoring `tick_p99_ms`, `jobs`, `schema`, wall times; a key only in NEW must be 0, empty or false
  (the `s10` counters), a key only in BASE fails; fortify-test and armour-test JSON: every BASE row equal;
* one line per file (`same 160/160` or the first 5 differences); exit 0 only when all are equal.
BASE: the orchestrator measures `9753cd9` once before Build A (a `git worktree`, or the `1bfc92f` runs cached in
`target/batch-cache`) and hands the dir to every stage. Each Build stage runs `neutral.py` on the 40 set (background,
polled); the orchestrator runs it on the three 160 sets after Build C. Targets T89-T90 add a second signal.

## 9. Build stages (about 50 agent turns each; checkpoint commit after each step)

**Build A: layout, content, pieces, materials.** Content `defs.rs`, `setup.rs`, `tuning.rs`, `variant.rs`,
`modset.rs`, schema; `mods/ww2/defs/fortifications/` (all five defs), construction crates, `[fortify]`, two pits; sim
`fort.rs` (new), `command.rs`, `game.rs` (`Order`, `MatchState::fort`, dispatch), `structures.rs` (`StaticRef::Piece`,
`rebuild_statics`, `hit_static`, `issue_crate`, `stock_in`), `combat.rs` (gates, cover), `vehicle.rs`, `hash.rs`,
`world.rs` (SAVE 22, `build_now`), net `protocol.rs` (PROTOCOL 13, limits; `CutWire` and `PlaceBangalore` refused as
`NotAllowed` until B), `tools/neutral.py` + test, BASELINE refresh. Guard `Construction` at `S6Account::crates_issued`,
`stock_in`, `stock_g`, `cargo_g`, `truck_loaded_g`, `TruckSupplySetup::crates`, the CLI `crates_issued`. Leaves
working: Build A tests, `tools/check.sh quick` green, 40-set `neutral.py` same 40/40. Hand-over (log): layout commit,
versions, guarded sites, the 40-set result, anything left for B.

**Build B: wire, breaching, the engineer split.** `fort.rs` (wire, cut, charges), `squad.rs`, `unit.rs` (speed),
`path.rs` (wire layer), `vehicle.rs` (crush), `combat.rs` (wire damage), `items.rs` (placing), three cards, engineer
caps, two Bangalore items, rosters, content tests, BASELINE. Leaves working: Build B tests, quick green, 40/40 same.
Hand-over: wire numbers from the tests, card indices, anything for C.

**Build C: presenter, bridge, net, CLI, docs; all tests.** Files: `snapshot.rs`, bridge, `godot/match/fort_view.gd`,
`orders_grid.gd`, `match_input.gd`, `match_state.md`, `relay-protocol.md`, net fuzz, CLI `skirmish.rs` (`s10`
counters per faction from `FortAccount`, all zero in AI play) and `fortify.rs` (section 7 rows),
`docs/modding/fortifications.md` (new: defs, `[fortify]`, tiers), `docs/STATE.md`. Ends with `tools/check.sh full`
all ok (every test of this spec passing, ignored ones included), `frontline-cli targets S10a.toml --runs fortify10`
with T91-T92 passing, and a 40-set `neutral.py` same 40/40. Hand-over: what the
orchestrator must run (measure 480 with `neutral.py`, verify.sh), open points for the AI half.

## 10. Done when (this half)

1. Wire slows infantry to 20 % until cut or blown (T91; `wire_slows_infantry_to_wire_speed_pct`).
2. A dugout roof survives an 81 mm hit and breaks under heavier HE (T92; the three roof tests).
3. fortify-test: trench, wire and MG nest against assault engineers plus an MG, reported against S3b T10 (T93).
4. `neutral.py` reports the same per-match outcomes on all three 160 sets and the 40 set; T89-T90 zero.
5. Every test of section 7 passes; SAVE 22, PROTOCOL 13, one bump each. The AI line belongs to the AI half.

## 11. What S10b and the AI half need from this slice

S10b: mines (a list beside `wire`), obstacle pieces that block vehicle nodes (S10a pieces block nothing, S10a-2), the
bunker slit as an open face (S7 `open`), `place_mines`/`clear_mines`/`demolish` promoted. AI half: the four commands
as its only path, the `s10` counters and fortify rows as measures, weights, prices, the AI done-when line, S9b carry-overs.

## 12. Owner questions (work proceeds on each (A); also in `docs/TODO.md`)

- **S10a-1 Who builds sandbag walls.** (A) `build_structure` squads (engineers) only. (B) Any digging squad (7.6).
- **S10a-2 What pieces block.** (A) Pieces stop rounds and fragments only; sight and walking go through them (men
  step over a wall like a parapet; wire is the one movement obstacle; the camo net is drawn only). (B) They also block
  spotting rays (camo net as a spotting modifier). (C) They also block movement nodes (more path work, S10b needs it).
- **S10a-3 Own fire through own pieces.** (A) Own rounds pass own pieces (the S6 box rule), so an MG fires out of its
  nest ring. (B) Every piece stops every round; nests need a firing slit.
- **S10a-4 The HQ dugout.** (A) Optional roof over a built HQ, 120 kg, 120 s; the S6 HQ unchanged. (B) Every HQ
  needs 120 kg and is roofed (changes AI play: AI half).
- **S10a-5 Materials.** (A) 100 MP per t, 20 kg per man, no walking penalty. (B) Free. (C) Carried kg slow the squad.
- **S10a-6 Interrupted work.** (A) Removed, materials lost (S6-8). (B) The site stays and any squad can finish it.
- **S10a-7 Vehicles and wire.** (A) Vehicles crush sections they cross, not slowed. (B) Wire stops wheeled vehicles.
- **S10a-8 Card prices.** (A) Roster with the German cut and S6 factor: 264 / 165, US 380. (B) Uncut 320 / 200 / 380.
