# S13 "heavy wave": code audit (read-only)

Working note for `S13-heavy-wave.md` (2026-11-19, research agent). Not a spec: values here are inputs; the plan decides.

Repo `/home/user/frontline-rts`, read 2026-11-19. Every claim cites the file and line I read. Paths:
`sim/` = `crates/sim/src/`, `content/` = `sdk/crates/content/src/`, `defs/` = `mods/ww2/defs/`.

**Bottom line.** Most of S13 really is data: the card caps, the rare-round rules, the third
ammo slot, open tops, casemates and extra hull boxes all exist. Five things do not:
- engine-driven (variable) turret traverse;
- a mantlet box that turns with the turret;
- any slope or calibre effect beyond the plain cosine law (APCR behaves like APCBC on slopes);
- the push limit (`push_without_supply_m`, S8b-13);
- any AI rule for heavies: no gating, no keep-back, no buy-by-value.

Two hidden traps:
- Two tests require the S11a1 cards to be last in each roster, so appending S13 cards fails them.
- `s11a1_tests.rs` asserts exactly 17 vehicle defs.

---

## 1. Armour model

**The one rule.** `armour::penetrates` (sim/armour.rs:234-245):
- `c <= cos(ricochet) || c <= 0` gives a ricochet;
- otherwise it penetrates when `pen0·(1+v)·c >= thickness + extra·c`, with `v` a ±7 % triangular draw (armour.rs:46-48, 248-251).

Other parts of the rule:
- **Penetration value.** `pen0` is the table row interpolated linearly by range and divided by `cos(pen_reference_angle)` (armour.rs:301-337). Every ww2 AP round uses reference 0° (e.g. defs/ammo/75mm_pzgr39.toml:66-67).
- **Slope.** It is a pure cosine law. The plate normal is tilted by `slope_deg` (armour.rs:164-178), and `c = cos s · cos a` for flat fire (armour.rs:28-31). Effective thickness is `t/c` (armour.rs:255-260).
- **Odds.** The closed-form chance is `pen_chance_ppm` (armour.rs:265-286).
- **Track band.** `extra` is `track_mm`, only in a hull side's track band (armour.rs:28-29, 428-431).

**No overmatch, shatter, normalisation or T/d effect.** `caliber_mm` is validated on the ammo def (content/vehicle.rs:2662) but never read by the sim: a grep of `crates/sim/src/*.rs` for caliber, overmatch or shatter finds no use. Ricochet is a per-round angle from 1 to 80° (vehicle.rs:2689-2696). All AP rounds use 70 (75mm_pzgr39.toml:68, 75mm_pzgr40.toml:26, 3in_m62.toml:130).

**Face-hardened plate** is the only material modifier: thickness × `face_hardened_pct` 125 (armour.rs:398-401; ArmourMaterial content/vehicle.rs:520-526; skirmish.toml:705). There is no cast-armour factor.

**APCR vs APCBC on slopes: no difference in the rule.** There is no `apcr` mechanism; `AmmoMechanism` is Ap, Aphe, He or Heat (content/vehicle.rs:314-325). The PzGr 40 is `mechanism = "ap"` plus `rare = true` (75mm_pzgr40.toml:10, 34). APCR differs only in data:
- a higher, faster-falling penetration table (75mm_pzgr40.toml:24);
- no filler; the filler only bursts for Aphe (sim/damage.rs:651, 761);
- a smaller core (75mm_pzgr40.toml:38-41).

So on the Panther glacis APCR keeps its full table divided by cos 55°. Historically APCR loses far more to slope than APCBC, so in game HVAP or PzGr 40/42 will overperform against sloped fronts.

**Boxes and plates.**
- Limits: `MAX_ARMOUR_BOXES = 4` (content/vehicle.rs:52), giving 16 zones (vehicle.rs:54). Each box has exactly 4 plate groups (front, side, rear, top), with both sides sharing `Side` and the bottom reading `Top` (vehicle.rs:30-46; armour.rs:117-126).
- One plate per face per box. Every non-open face must have a plate. Thickness is 1.0-500.0 mm in tenths (vehicle.rs:1627). Slope is 0-80° and top plates must be flat (vehicle.rs:1633-1650).
- Declaring a box: `[[boxes]] id, frame = "hull"|"turret", box = {center, half}, open = [...], [boxes.armor] front/side/rear/top = {thickness_mm, slope_deg, material}`. Templates: defs/vehicles/ger_pz4_h.toml:76-94 (hull + turret), ger_stug3_g.toml:64-86 (hull + second hull-frame "casemate" box), usa_m10.toml:83-95 (`open = ["top"]`, `floor_cm`).
- Box 0 must be the hull frame (vehicle.rs:1528-1533).
- **A turret needs exactly one `frame = "turret"` box** (vehicle.rs:1541-1557). `Frame` has only Hull and Turret (vehicle.rs:272-276).
- **So a mantlet box that turns with the turret cannot be declared.** A second hull-frame box can be (the StuG does this).
- Overlapping boxes: the first plate the segment enters (smallest `t`) wins; on a tie the lower box index wins (armour.rs:700-741).

**Can S13's figures be expressed?**

| Feature | Expressible? | How / what is lost |
|---|---|---|
| Panther 80 mm @ 55° upper glacis | yes, exact | superstructure box front 80 @ 55 |
| Panther 60 mm @ 55° lower nose | yes | box 0 front; uses box 2 of 4 |
| Panther sides 50 @ 30 upper / 40 @ 0 lower | yes | box 0 side 40/0 with the track band; superstructure side 50/30 |
| Tiger 100 mm flat front | yes, exact | upper 100 @ ~10; nose 100 @ 25 needs a lower box; the 60 mm @ 80° glacis needs the 4th box (80 is the slope cap) |
| Jumbo 152 mm turret front | yes | turret front 152; the 177.8 mm mantlet cannot be layered (one turret box) |
| Mantlets (Panther curved, Tiger 100-120) | no | folded into turret front thickness; no shot trap |
| Cast armour (Jumbo turret, Sherman) | no factor | enter a reduced RHA-equivalent thickness |
| Slope effect beyond `1/cos` | no | the Panther glacis is 139 mm effective in game. The M62 (3in_m62.toml:128, 118 mm at 100 m) still fails it, but HVAP will pass it more often than history suggests |

---

## 2. Turret traverse

- **One fixed integer rate.** `TurretDef.traverse_deg_s: u32` (content/vehicle.rs:932-941), validated 1..=360 (vehicle.rs:1681-1687). Casemate and pintle mounts carry their own rate (vehicle.rs:1084, 1864-1870, 1989-1995). The lowered `VehicleSetup.traverse_deg_s` is the turret's or the casemate gun's (vehicle.rs:3550).
- **No idle, rpm, engine or hand-traverse field anywhere.** A grep for `traverse_idle`, `rpm` or `hand_traverse` over sim and content finds nothing. "Hand traverse" is only data: the M10 has `traverse_deg_s = 5` (usa_m10.toml:63).
- **Where it is consumed.**
  - `gunnery::lay_gun_pct` (sim/gunnery.rs:214-256): the rate is scaled by `traverse_pct`, zeroed with a dead turret ring and halved under 50 HP (gunnery.rs:228-235).
  - The only scaler is the untrained-gunner percentage (gunnery.rs:655-661; rules `vehicles.recrew.untrained_traverse_pct`, content/tuning.rs:966-967).
  - Pintles use `pintle_turn` (gunnery.rs:188-202).
  - **Engine HP is not read**: a Panther with a dead engine traverses at full rate.
- **A hydraulic-traverse mechanic would touch:**
  - content `TurretDef` and `VehicleSetup` (a new optional `traverse_idle_deg_s`) and the lowering at vehicle.rs:3550. `MatchSetup` is postcard-encoded into the save (sim/world.rs:873-879) and the net `setup_hash` (crates/net/src/lib.rs:71-74), so per the repo's rule this means **a SAVE bump** (world.rs:400-470 lists setup-key changes as bumps).
  - `lay_gun_pct`, to pick the rate. Choosing "idle" can be derived from existing saved state: `VehicleState::speed` and `still_ticks` (sim/vehicle.rs:221, 253) and `components` (vehicle.rs:229). So there is no new state, and the hash is unchanged.
  - An AI or player "rev engine" (louder, a spotting reveal) needs a new saved, hashed `VehicleState` field and a spotting change (sim/spotting.rs:314-324). If players can order it, it also needs a new command and **a PROTOCOL bump**.
  - The mock presenter's hard-coded traverse table (godot/match/mock_vehicles.gd:60-61); optional.

---

## 3. Call-ins and caps

**Where each field lives.** All are on the unit card (`defs/units/*_squad.toml`), not the faction file. Faction files only list `roster` ids (defs/factions/germany.toml:4-40, usa.toml:4-36).

Card block (content/defs.rs:156-197):

| Field | Where | Note |
|---|---|---|
| `cooldown_s` | defs.rs:159-161 | alias `recall_cooldown_s` |
| `unlock_s` | defs.rs:166 | alias `initial_cooldown_s` |
| `max_alive` | defs.rs:169, max 100 | variant.rs:119 |
| `limit_group` | defs.rs:173 | |
| `available_from` / `available_to` | defs.rs:177-179 | filtered against rules `[call_in] date` (setup.rs:1838-1847); `date = ""`, so no filter in skirmish (skirmish.toml:1226) |
| `ai_weight` | defs.rs:197, 0..1000 | default 100 (variant.rs:121) |
| `cooldown_group` | defs.rs:124 | |
| `tow` | defs.rs:198-201 | |

- Rules: `[limit_groups]` (`BTreeMap<String,u32>`, defs.rs:1624-1626), lowered sorted (setup.rs:442-443, 2098).
- On a **vehicle card**, soldier keys are rejected, including `recall_cooldown_s` and `initial_cooldown_s` (defs.rs:2141-2183). A vehicle card uses `cooldown_s` and `unlock_s`.
- Lowering a vehicle card (setup.rs:1692-1729): cost, cooldown and unlock come from the card when set, else from the vehicle def.
- **`pop` comes only from the vehicle def** (`SquadTypeSetup::pop`, setup.rs:1296-1300; def range 1..120, content/vehicle.rs:1373). A card cannot override pop.
- So today **vehicle cards support all of: cost, cooldown_s, unlock_s, max_alive, limit_group, available_from/to, ai_weight**.
- A card has **one** `limit_group`. "Heavy also counts against armour" works anyway: a role `tank` card always counts against `[vehicles] limit` (game.rs:1071-1073; `outside_limit` content/vehicle.rs:396-406). The Panther's cap of 2 = `max_alive = 2`.

**Enforcement** is one rule for the command and the AI: `call_in_check` (sim/game.rs:1030-1101). The order is roster, unlock, `max_alive`, limit group, vehicle limit, cooldown, manpower, pop.
- Unlock for an in-limit vehicle = `max(card unlock, rules vehicles.unlock_s)` (game.rs:1050-1057).
- The card unlock seeds `ready_at` at match start (game.rs:763-775).

**AI.** The AI's `ready` closure repeats `max_alive`, limit group, unlock and cooldown (sim/ai.rs:1834-1858).

**HUD.**
- Tiles show cooldown (from `ready_at`, so a late card unlock shows as a cooldown) (crates/godot-bridge/src/match_node.rs:2306-2318).
- The `max_alive` badge shows **only for outside-limit cards** (match_node.rs:2320-2340). A heavy tank card gets no cap badge.
- Refusal texts exist for `MaxAlive` and `GroupLimit` (match_events.rs:85-101).

**Current values.**
- Faction vehicle cards:
  - Germany: `ger_pz4_h_squad`, `ger_stug3_g_squad`, `ger_sdkfz251_1_squad`, `ger_sdkfz234_1_squad`, `ger_opel_blitz_squad`, `ger_pak40_squad`, `ger_pak38_squad`, `ger_sdkfz11_squad`, `ger_leig18_squad` (germany.toml:16-38).
  - USA: `usa_m4a3_75_squad`, `usa_m10_squad`, `usa_m3a1_squad`, `usa_m8_squad`, `usa_cckw_squad`, `usa_57mm_m1_squad`, `usa_105mm_m3_squad` (usa.toml:15-34).
- The tank card sets nothing but `vehicle` (units/ger_pz4_h_squad.toml:4-9). The M10 card adds `ai_weight = 100` (usa_m10_squad.toml:21). The leIG card sets `ai_weight = 5, max_alive = 2`.
- `[vehicles] unlock_s = 240, limit = 4` (skirmish.toml:508-509).
- **`[limit_groups]` is empty** ("None in ww2 yet", skirmish.toml:1232-1235).
- The vehicle def holds `cost/pop/recall_cooldown_s/initial_cooldown_s` (ger_pz4_h.toml:22-31: 620 / 12 / 120 / 0).

## 4. Rare rounds (D3)

All built:
- `rare = true` on the ammo def (75mm_pzgr40.toml:34).
- `rare_cap` on the vehicle def; default `rare_cap_max` when a rare round is listed (content/vehicle.rs:620-626).
- Validation:
  - cap ≤ rules `rare_cap_max` (content/modset.rs:1552-1572);
  - stowed rare ≤ cap (modset.rs:1574-1587);
  - a rare round is never a weapon's only feed (modset.rs:1481-1487).
- Rules: `rare_cap_max = 10`, `rare_rearm_base_s = 10` (skirmish.toml:1253-1254), `rare_rearm_fwd_s = 30` (skirmish.toml:1291).
- Base refill: sim/supply.rs:851-870, 905-915; `pick_round` stops at the cap (supply.rs:946-963).
- Forward refill (HQ, dump, truck): sim/structures.rs:1414-1488, counted as `tank_rare_fwd` (structures.rs:169).
- **`push_without_supply_m` (S8b-13) is NOT built.** It appears in no `.rs` or `.toml`; only in the design (docs/design/roster/vehicles.md:911-913, 936). Neither is its partner `tank_reserve_rounds` (vehicles.md:936).

## 5. Ammo slots

- `MAX_MAIN_AMMO = 3` (content/vehicle.rs:550-552; checked at 1901-1908).
- Fixed slot order: 0 standard penetrator, 1 HE, 2 special penetrator (vehicle.rs:1089-1093, 4130-4140; sim/gunnery.rs:113-115).
- **APCBC + APCR + HE is possible now** (Panzer IV H: ger_pz4_h.toml:160-164).
- A 4th entry (smoke) is impossible.
- Mechanisms: Ap, Aphe, He, Heat (vehicle.rs:314-325). `penetrator()` is true for Heat, so a HEAT round can sit in slot 0 or 2 (vehicle.rs:328-331). No ww2 tank carries one; HEAT defs exist only for launchers (defs/ammo/m6a3_heat, pzf60_heat, rpzb54_heat).
- **No smoke shell** of any kind for guns.
- `AmmoChoice` is Auto, Ap, He or Apcr (sim/vehicle.rs:102-110). A Smoke variant would change the `SetAmmo` command (sim/command.rs:136): **a PROTOCOL bump**.
- Auto picks the special round only when the standard round's chance against the facing plate is under `special_below_pct` 50 and the special's is higher. This is `special_better` (gunnery.rs:1169-1222; skirmish.toml:685), used at gunnery.rs:526-569. **So target-based penetrator choice exists for every gunner, AI included.**

## 6. Heavy mortars

**Two existing paths.**
- **(a) Squad mortar.** A unit card whose member carries a `class = "mortar"` weapon with `[indirect]`, plus a `feed` cartridge. Examples: units/ger_mortar_team.toml:36-63, weapons/grw34.toml, cartridges/8cm_wgr34.toml. The `fire_mission` capability is derived (content/modset.rs:388).
- **(b) Served gun entity.** Role `inf_gun`, `crew_model = "served"`, `mobility type = "towed"`, a `casemate` mount (vehicles/ger_leig18.toml). Missions go through `gun_indirect_weapon` and `gun_mission_refusal` (sim/game.rs:1489-1499, 1550-1600).

**Limits that bite.**
- **(a) mortar range.** Weapon `range_m` is 1..5000 (content/defs.rs:2539), and a mortar table's range is the weapon's `range_m` (defs.rs:812-830). The 12 cm GrW 42's ~6 km fails; the 4.2-inch's ~4 km fits. Moot on a 1,024 m map (the GrW 34 is set to 600 m, grw34.toml:14).
- Charges ≤ 12, 20-600 m/s, strictly rising (defs.rs:736-776). High angle is 45-89° for non-`low_angle` tables (defs.rs:778-791).
- `frag_count` ≤ 256 (`MAX_FRAG_RAYS`, defs.rs:481-491); `blast_radius_cm` and `crater_radius_cm` ≤ 2000 (defs.rs:490, 2516).
- Cartridge `supply.g` ≤ 100 kg (defs.rs:972).
- No per-man weight or carry cap, and no crew or bearer count rule.
- The solver is drag-free i128 math with no range table (sim/indirect.rs:1-80). Flight bound is `2v/g + 20 s` (indirect.rs:241-251), so long flights are fine.
- There is **no rifled-vs-smoothbore distinction** (the 4.2-inch is just a spread value).
- **(b) gun-entity mortar.** A main mount must be `class = "cannon"` (content/vehicle.rs:4087; modset.rs:1157-1170), so a "mortar" served gun means a cannon-class weapon with a `low_angle` table and band [45, 85]. That is accepted (defs.rs:838-880).
  - It then counts as an infantry gun:
    - counter-battery evidence applies (sim/counter_battery.rs:1-9);
    - `[ai.arty] max_guns = 1` (skirmish.toml:1532) is shared with the leIG/M3;
    - kills are "cannon" (TANK_AP), not "mortar" (combat.rs:3084-3085).
  - The AI buys **only the first `inf_gun` card in roster order** (sim/ai/arty.rs:274-280), so a second `inf_gun` card is never bought without code.
- **Verdict.** A 4.2-inch or 12 cm as a *squad* mortar with a carried tube is pure data. A wheeled carriage needs path (b) plus a small AI change (ai/arty.rs:279, card choice), or a validator change to allow mortar-class main mounts.
- AI mortars join the `mortar` mix slot (weight 50, skirmish.toml:909) and are split by `ai_weight`.

**What S11b will touch in the same places** (docs/design/roster/roster-expansion.md:466-519; artillery-on-map.md:1477, 1705-1707):
- new mortar weapons, cartridges and squad cards (kz. GrW 42, 60 mm M2);
- leFH 18 and M2A1 as `inf_gun` vehicles plus ammo, competing for the same first-`inf_gun` pick and `max_guns`;
- rifle grenades: a new launcher mode in code;
- more appends to both rosters;
- `[ai.mix]` / `[ai.arty]` keys;
- the same BASELINE hashes and snapshots.

Expect merge conflicts in germany.toml and usa.toml, skirmish.toml `[ai.*]`, resolved_defs.rs:183-195, and the two "last cards" tests (section 8).

## 7. AI

**Buying** (sim/ai.rs:1700-2100).
1. The army mix is by card **tag**: `[ai.mix]` line 50, mg 20, engineer 15, mortar 50, at 20, vehicle 20 (skirmish.toml:905-911). `mix_slot` is at ai.rs:1105. Every vehicle card is tagged `vehicle`, so tanks, TDs, half-tracks, cars, trucks and even the leIG share one slot.
2. Within the slot, the AI takes the ready card with the fewest buys per `ai_weight`; ties go by roster order (ai.rs:2031-2055).
3. **If it cannot afford that card it returns and saves up; no cheaper card instead** (ai.rs:2056-2060).
4. There is no cost/value rule and no "seen enemy armour" gate for vehicles. Only the AT slot waits for `enemy_vehicle_seen` (ai.rs:1874-1879).
5. Infantry keeps `tank_reserve_mp = 200` back (skirmish.toml:981; ai.rs:2071-2081).

**A new 1,100-1,250 MP card at `ai_weight` 100** would be picked as often as the Panzer IV. Each time it is picked and unaffordable, that plan buys nothing. Levers today: `ai_weight`, `max_alive`, `limit_group`, `unlock_s`.

**Behaviour.**
- Role `tank` leads assaults: the 160 m standoff from seen AT and 40 m to the trench (skirmish.toml:883-884).
- Only `assault_gun` and `tank_destroyer` take **overwatch** (sim/ai/overwatch.rs:86; `[ai.armour] overwatch_back_m 35`, `td_hunt_m 300`, skirmish.toml:1031-1036). So the M36, Jagdpanzer IV and M18 overwatch automatically. **Panther, Tiger and Jumbo (role tank) would assault like a Panzer IV**: there is no "keep heavies back".
- Facing a threat: `face_range_m 600` (skirmish.toml:968).

**Rare rounds.**
- The rearm trip is triggered by MGs dry, HE ≤ `tank_he_reserve`, or **slot-0 AP ≤ `rearm_ap_below` 3** (sim/ai/armour.rs:20-41; skirmish.toml:988). Rare rounds never trigger a trip.
- "Rearmed" means every slot, special included, at ≥ `rearm_full_pct` 90 (ai/armour.rs:43-57). In base the rare rounds refill at 10 s each.
- No AI code issues `SetAmmo`: the grep finds nothing in ai.rs or ai/*.rs. The AI relies on Auto's `special_better` (section 5), which already compares the chance against the actual plate.
- Related `[ai.armour]` keys (skirmish.toml:953-1040): `threat_m`, `tank_reserve_mp`, `rearm_ap_below`, `rearm_full_pct`, `rearm_max_s`, `overwatch_*`, `td_hunt_m`, `td_infantry_standoff_m`, `open_top_infantry_m`, `face_*`, `escort_*`.
- Related `[ai]` keys: `tank_standoff_m`, `assault_close_m`, `at_clear_s`. `[ai.mix] vehicle`, `[ai.ammo] float_mp`, `[ai.guns]`.

## 8. Tests and tools

**armour-test.**
- 40 cases are registered in one list (sdk/crates/cli/src/armour_test.rs:3880-3920). Cases 1-24 are in armour_test.rs, 25-32 in `armour_test/s8b.rs`, 33-40 in `armour_test/s9.rs`. The `--case` help still says "1–15" (armour_test.rs:57).
- The duels are `Duel::{Frontal300, Angled300, Flank150, PzFlank150, Frontal600}` (crates/sim/src/scenarios.rs:360-374, 380-415) and `CasemateDuel` (scenarios.rs:446-492).
- `Frontal600` raises sight to `DUEL_600_SIGHT_M` 350 so 600 m tanks see each other (scenarios.rs:356).
- **Staging spawns by role or tag, not by id**:
  - `tank()` is the first roster card with the vehicle tag (scenarios.rs:107-148);
  - `vehicle_of_role` is the first card of a role (scenarios.rs:417-454).
- To add a Panther/M36 duel:
  1. add a scenario fn in scenarios.rs with a spawn-by-card-id helper (missing; `World::spawn_squad_at` takes a type index);
  2. add `armour_test/s13.rs` with `caseNN`;
  3. register it in the `all` list;
  4. add target rows `armour5.cases.caseNN.checks.<name>.value` (pattern: docs/slices/targets/S11a2.toml:535-550; runs `[runs.armour5] kind = "armour-test"`, S11a2.toml:113-118).
- New cards must be **appended** after the Panzer IV and Sherman, or the existing cases switch vehicles.

**Targets files.** `name`, `[runs.<name>]` (`kind` = skirmish | armour-test | fortify-test, `seeds`, `both_bases`, `minutes`, `jobs`, `stress`), then `[[target]] id, metric, min/max, kind = "report"` (S11a2.toml:24-118, 283-295). The S9 header says new vehicles appear in the `s7_derived` maps with no code change (S9.toml:31-35).

**BASELINEs and snapshots to refresh.**
- `setup_bytes_baseline`: 6 setup hashes (sdk/crates/content/tests/resolved_defs.rs:183-195, test at 270-282). Any def or TOML change shifts them.
- `legacy_view_matches_snapshot` and `canonical_view_matches_snapshot` compare against `tests/snapshots/ww2_resolved.json` and `ww2_canonical.json`, and require "the same defs, no more and no fewer" (resolved_defs.rs:344-365, 429-440). Re-bless with `BLESS=1` (resolved_defs.rs:14).
- **`crates/simbench/src/report.rs` holds no content baseline.** Its "BASELINE ONLY" is a hardware notice (report.rs:72-85), and simbench does not load mods (grep). STATE.md's instruction to refresh it looks stale.

**Tests that break when defs or cards are added.**
- `s11a1_tests.rs:246`: `assert_eq!(s.vehicles.len(), 17)`, and `kills_by_vehicle.len() == 18` (s11a1_tests.rs:261).
- `sdk/crates/content/tests/s11a1_content.rs:295-305`: the last 2 roster cards must be leIG/VB-Trupp and M3/FO Party.
- `s10a_content.rs:45-57`: strips the last 2 and expects the S10a cards before them.
- Not affected: `sim/tests/armour.rs` and `content/tests/vehicles.rs` use the frozen fixture `tests/fixtures/ww2_s3c` (armour.rs:14-15; vehicles.rs:16-17).

**Other tool.** `frontline-cli ballistics` (sdk/crates/cli/src/ballistics.rs) prints the same rule for a plate and round; useful for the plan's penetration tables.

## 9. Presenter

**No per-vehicle assets.**
- Meshes are procedural from the type's dims and boxes (crates/godot-bridge/src/meshgen.rs:444-470, 694, 808, 865-874; godot/match/vehicle_view.gd:3-20).
- A tracked tank's hull is drawn from the hull dims only. Extra hull-frame boxes are drawn only for casemate, served-gun, wheeled and half-track blockouts (meshgen.rs:552-668). So a Panther superstructure box is not drawn separately, which is acceptable for blockouts.
- Up to 4 boxes are passed (`tboxes: [[f32;7];4]`, meshgen.rs:261-268).
- The only per-id Godot data is the mock (godot/match/mock_vehicles.gd:17-61); optional.
- Minimum per vehicle: nothing beyond the TOML.

## 10. Spotting

Range = observer `sight_m` × the target's highest factor (sim/spotting.rs:28-41, 314-361). The factors are:
- `spot_size_pct`;
- `moving_spot_pct` 250 when moving (skirmish.toml:522);
- the gun's `fire_reveal_pct` 300 for 3 s after firing (kwk40_l48.toml / m3_75mm.toml / m7_3in.toml);
- halved in woods;
- soldiers spot within `near_spot_m` 25 with no ray (skirmish.toml:525).

Owner Q12 (A) "vehicle spotting 2×" is just this `spot_size_pct` ≈ 200, not a separate rule (docs/TODO.md:37; roster-expansion.md:781-784).

| Vehicle | `sight_m` | `spot_size_pct` |
|---|---|---|
| Pz IV H, M4A3 (75) | 175 | 200 |
| StuG | 175 | 160 |
| M10 | 175 | 190 |
| M8, 234/1 | 220 | 140 / 130, plus `stationary_sight_pct` 140 |
| AT guns | 175 | 85-90 (optic 250 % in arc, spotting.rs:133-147) |

Tank vs still tank: 175 × 2.0 = **350 m**. Against a mover: **437 m**. Against a tank that just fired: **525 m** for 3 s. All tank guns have `range_m` 600 (kwk40_l48.toml:8, m3_75mm.toml:6, m7_3in.toml:9).

So duels open at about 350 m. The Panther's long-range edge only appears via the fire-reveal window or a recon car or observer. A larger `spot_size_pct` for the Tiger (taller) is a data lever.

## 11. Save, hash, protocol

- `SAVE_VERSION` 28 (sim/world.rs:470); `PROTOCOL_VERSION` 14 (crates/net/src/protocol.rs:38).
- The save carries `MatchState`, which contains the setup (world.rs:873-879). The armour tables are derived and never saved (armour.rs:3-7).
- **New vehicle, ammo, weapon or card defs alone**: no struct layout changes, only data. Per-type vectors are sized from the setup: kill rows are `Vec` (s11a1_tests.rs:261), `bought` is resized (ai.rs:1939). The kill-class carrier is `u16` (`class | (t+1)<<4`, so 4,095 types; s11a1_tests.rs:236). **No SAVE or PROTOCOL bump**; only the setup hash changes, so net peers need the same mod.
- **A traverse mechanic** adds a `TurretDef`/`VehicleSetup` field, which changes the setup layout: **SAVE bump** (the repo's convention, world.rs:419-470). A rev state on `VehicleState` adds a SAVE bump and a hash field. **PROTOCOL only** if a new command or `AmmoChoice` variant is added.
- Other bumps:
  - push limit (AI-only, new `[ai.*]` keys): SAVE bump;
  - an APCR slope factor (new ammo key in `AmmoSetup`): SAVE bump.

---

## Summary table

| Needed for S13 | Exists (file:line) | Missing (size) | Notes |
|---|---|---|---|
| Panther / Tiger / Jumbo / 76 W / M36 plates | 4 boxes, 4 groups, slope 0-80, 1-500 mm (content/vehicle.rs:52, 1541-1557, 1627-1650) | mantlet box in turret frame (M); cast factor (S, or data workaround) | glacis, sides and fronts exact; mantlets folded into turret front |
| Slope or calibre effects | cosine law only (sim/armour.rs:234-286) | APCR slope penalty / overmatch (M; SAVE bump) | HVAP will overperform vs the Panther glacis |
| APCBC + APCR + HE per gun | MAX_MAIN_AMMO 3 (vehicle.rs:552); Pz IV H (ger_pz4_h.toml:160-164) | — | data |
| Smoke rounds for tanks | — | mechanism, 4th slot, `AmmoChoice::Smoke` (M; PROTOCOL bump) | not in S13 core |
| New guns (KwK 42, KwK 36, 76 mm M1, 90 mm M3) and ~11 ammo defs | templates kwk40_l48.toml, 75mm_pzgr39/40, sprgr34 | data (S) | penetration rows from Bird & Livingston at 0° to match existing rows |
| Rare rounds and caps (D3) | modset.rs:1552-1587; supply.rs:851-963; structures.rs:1414-1488; skirmish.toml:1253-1291 | — | set `rare_cap` per def |
| Push limit `push_without_supply_m` (S8b-13) | none (only vehicles.md:911, 936) | AI rule plus keys (S-M; SAVE bump) | partner `tank_reserve_rounds` missing too |
| Engine-driven traverse | fixed rate (vehicle.rs:937; gunnery.rs:214-256) | idle-rate field plus rate pick (S, SAVE bump); AI rev plus noise (M) | `speed` and `still_ticks` already saved (vehicle.rs:221, 253) |
| Heavy cap / Panther cap 2 | `limit_group`, `max_alive` (defs.rs:166-179; game.rs:1064-1073) | `[limit_groups] heavy = 1` entry (data) | heavy still counts vs `[vehicles] limit` 4 |
| Unlock, cooldown, dates, cost | card or def (setup.rs:1692-1729; game.rs:1050-1057) | — | `pop` only on the vehicle def |
| HUD cap badge for in-limit cards | outside-limit only (match_node.rs:2320-2340) | badge for tank cards (S) | refusal text exists |
| AI buys heavies sensibly | tag mix plus `ai_weight` (ai.rs:2031-2060) | gate on seen armour or value rule (S-M) | stalls on saving; lever `ai_weight` |
| AI keeps heavies back | overwatch for TD/AG only (ai/overwatch.rs:86) | heavy overwatch habit (S-M) | M36, M18, Jagdpanzer IV covered by role |
| AI rare-round use | Auto `special_better` (gunnery.rs:1169-1222) | rare-aware rearm trigger (S, optional) | refills in base anyway |
| Heavy squad mortars (12 cm, 4.2 in) | squad path (ger_mortar_team.toml; grw34.toml) | data (S); 12 cm range > 5000 m not allowed (defs.rs:2539) | carried tube, no carriage |
| 12 cm as towed gun entity | inf_gun path (ger_leig18.toml; game.rs:1489-1600) | AI picks only the first inf_gun card (ai/arty.rs:279) (S); cannon-class workaround (vehicle.rs:4087) | competes with `max_guns` 1 and counter-battery |
| armour-test heavy duels | Duel / CasemateDuel (scenarios.rs:360-492); registry (armour_test.rs:3880-3920) | spawn-by-id helper plus cases (S-M) | M36 vs Panther at 350 m needs no sight hack; 600 m does |
| Tests that need updating | resolved_defs.rs:183-195; snapshots; s11a1_tests.rs:246, 261; s11a1_content.rs:295-305; s10a_content.rs:45-57 | edits (S) | roster "last cards" asserts break on append |
| Presenter | procedural meshes (meshgen.rs:444-874) | none | mock_vehicles.gd optional |
| Spotting range decision | 175 m × 200 % = 350 m (spotting.rs:314-361) | owner decision only | duels at about 350-525 m |
| Save / protocol | SAVE 28 (world.rs:470); PROTOCOL 14 (protocol.rs:38) | — | data alone: no bump; traverse field or push keys: SAVE |
