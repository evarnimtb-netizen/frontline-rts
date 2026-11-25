# S15: USSR, the third faction (Red Army, 1944): plan

Status: **plan only** (2026-11-22, revision 2 after an independent review: 22 findings, all folded in; section 12). Nothing built.
**Owner questions:** the minor ones (S15-1, -2, -4, -6, -9, -11) are settled on (A); the how-it-should-play ones (S15-3, -5, -7, -8,
-10, -12) are **asked**, and the work proceeds on their (A) until answered (owner rule 2, `TODO.md` 45). Section 11 lists them.
Base: `main` at `6f7bda0` (S12 merged; SAVE 30, PROTOCOL 16). S15 builds after S13 (branch `claude/project-thread-aj1tww`, SAVE 31)
and S14 (assault kit) are merged, from a fresh main. Every file:line below is from `6f7bda0`; S13 edits `gunnery.rs`, `armour.rs`,
`damage.rs` and `sim/vehicle.rs` (the R2 and O1 hook sites), so Build A re-reads them after the rebase.
Card: `docs/design/roster/roster-expansion.md` 3.3 and "S15". Design: `infantry-structures.md` 2.3, `vehicles.md` 4.3 and 8.3,
`guns-artillery.md` 6 (USSR rows), `artillery-on-map.md` 2.3. Working notes (inputs; this file decides):
`S15-research-code-audit.md` (file:line audit), `S15-research-history.md` (values, URLs, (verify) flags). Pattern: `S13-heavy-wave.md`.
Owner rules as every slice since S10: features first, no per-slice Tune; Germany's pooled share in 35-65 %, else stop and diagnose;
all tuning in TOML; damage only from projectile or fragment contact; no free ammo; every new behaviour behind a switch; all S15
switches off = `<BASE>`; tick p99 must not regress.

## 0. Summary
- **The faction itself is data.** `FactionDef` loads any number of factions by id, nothing caps them, and no sim file outside
  `#[cfg(test)]` names Germany or the USA (audit 1, 2). Use id `ww2:ussr` and unit prefix `ussr_`: both sort last, so Germany and
  the USA keep their indices, their 3D tint and every append-sensitive test (audit 1).
- **The card's "mostly data, 0.6x" is low.** Read against the code S15 is about **1.3x S3**: three small systems (tank riders with a
  man's aim and hit path, a directional torso plate, captured mortar bombs), a front end and tools that assume two factions, new
  counters and a plate bench to measure them, an AI rule for a tank that carries men, about 22 cards with ~10 vehicle and gun defs
  and ~15 weapons and rounds, and two new matchups. So it follows the S10 owner rule (`TODO.md` 40) and splits like S13: **S15a**
  mechanics, N-faction plumbing and measuring tools, all switches off, proven neutral; **S15b** the USSR data, cards at `ai_weight` 0,
  proven neutral for Germany vs USA; **S15-ai** rider AI, weights, and the three-matchup measure. Engine tier (the review gate fires);
  `workflows.md` 1 lists S15 as data: the Docs stage corrects it.
- **Three-way balance is never one pooled number** (`workflows.md` 32-34): each matchup is its own 480 set (section 8).
- **No map work.** S15 plays on Kessel Ridge only (2 bases, 1v1). Roads and a second map are S16 (`S16-second-map.md`, parallel plan).

## 1. What exists, and what is missing (audit of `main` 6f7bda0; detail in `S15-research-code-audit.md`)
| Needed | State | Where |
|---|---|---|
| Faction def, any count | exists: id, name, colour, roster, start squads | `content/defs.rs` 1513-1526; validate 2830-2860 |
| All factions in every match | exists: every faction lowered into `MatchSetup::factions`, sorted by id | `content/setup.rs` 1922-1958 |
| Sim by faction index | exists (rosters, start squads, stocks, AI rosters) | `sim/game.rs` 1100, 1974-1980; `sim/ai.rs` 1708, 1728 |
| USSR vs X from the CLI | exists (`--faction0/--faction1`; `batch.sh` passes arguments) | `cli/skirmish.rs` 2131-2160 |
| USSR vs X from the menu | missing: `FACTIONS: [&str; 2]`, opponent = "the other one" | `bridge/match_setup.rs` 16-37, 69, 133-161 |
| GDScript faction tables | missing entries (crash on `FACTION_NAMES[f]`) | `godot/autoload/session.gd` 57, `godot/ui/palette.gd` 24-29, `vehicles.gd` 59/63, `match_view.gd` 97-99 |
| 480 measure per matchup | missing: `RunSpec` has no factions; `pool.py` reads `germany` only | `cli/targets.rs` 99-122, 400-410; `tools/pool.py` 45, 96; `tools/measure.sh`; `ablate.sh` 339 |
| armour-test with USSR | missing: the env is GER vs USA | `cli/armour_test.rs` 209 (S13 adds `scenarios::duel_cards`, reuse it) |
| Mirror match | refused by bridge, relay, lobby; one AI bug | `relay.rs` 890-893; `ai/assault.rs` 456-470 (S15-10) |
| Tank riders | half: `passenger_slots` are hit boxes, Embark/Disembark exist; `[transport]` on a tank refused; deck boxes fail validation; the "exactly on a transport" setup check; riders exposed only in an open box; riders aimed at only by weapons with a `penetration` table and only in the first exposed slot; rider hits go `HitVia::Rider` -> `hit_open` (no S12 zone); no dismount on hit or fire; `Exit::Forced` stuns and idles the squad; the AI takes any vehicle with seats as a transport | `content/vehicle.rs` 2523-2542, 2583-2610, 4220-4224; `sim/vehicle.rs` 917-928; `combat.rs` 308-337; `armour.rs` 805-824; `transport.rs` 18-35, 318-332, 535+; `ai/transport.rs` 145-148 |
| Torso plate | missing; S12 hit zones exist; per-man data can come from the member slot; most small arms have no `penetration` row (MP 40, StG 44, P 38, M3, carbine, M1911) and adding one makes a weapon aim at vehicles | `combat.rs` 3461-3482 (bullet), 3236-3245 (fragment), 296-312 (aim); `armour.rs` 318-321 (`pen0` 0 on an empty table) |
| Captured bombs | missing: a weapon has one `feed`; men draw from crates of their own faction and from any own structure or truck | `structures.rs` 1087-1118, 1144, 1236, 1340-1370, 1462, 1542; `content/setup.rs` 336-338 (MAX_FEEDS 4), 2373-2432 |
| Supply, aid post per faction | data: HQ, dump, aid post (test requires one per faction), 4 crates | `content/tests/s12_content.rs` 225-230 |
| Mines for sappers | exist as shared fields laid by the `place_mines` capability, never carried in a kit | `items/at_mine.toml` 1-3; `ww2:at_minefield`, `ap_minefield` |
| Thrown AT grenade | no item kind; only `single_shot_at` (Panzerfaust) | `items/panzerfaust60.toml` 13 |
| Infantry MG shield | none: shields exist only on vehicles and guns | `content/vehicle.rs` 542 (`MAX_SHIELDS`) |
| Per-faction rules keys | none: one `gun_fire_direction_s` for all (S15-9) | `defs/rules/skirmish.toml` 1675 |
| Presenter | blockouts procedural; soldier tint by faction index in a 10-colour shader table | `bridge/meshgen.rs` 444-874; `soldier_skinned.gdshader` 21-24, 80 |
Save and protocol: new defs alone bump nothing. Every new def or rules key of S15a (riders, plate, alternate feed) enters
`MatchSetup`: **SAVE +1 in S15a's one layout commit**, with every S15-ai key and saved AI field too. PROTOCOL stays: no new command
(Embark/Disembark exist). Tests and baselines touched: `resolved_defs.rs` 197-207 (6 hashes), both JSON snapshots, `s11a1_content.rs`
304-316 (loops every faction, expects GER or USA ids: breaks), `s12_content.rs` 225-230 (aid def per faction), `sim/s11a1_tests.rs`
247 (vehicle def count). S15a tests use an `s15_off()` helper as S13's `s13_off()`.

## 2. Scope
**S15a (engine).** R1-R4 tank riders, P1 torso plate, C1 captured bombs, F1 N-faction front end, F2 per-matchup tools and counters,
the plate bench, the faction-selectable armour-test env, the layout commit (all keys at Off), the `s11a1_content.rs` loop fix. No USSR
content beyond one test fixture faction.
**S15b (data).** `defs/factions/ussr.toml` and the core wave (section 5), every card `ai_weight` 0 but every AI tag set; `plate_pen`
rows for every small-arms cartridge (read only by P1); GDScript tables and the shader tint; design-doc fixes (section 9).
**S15-ai.** H1 rider tank stays a tank, H2 riders loaded and dropped, weights on, 3 x 480 measure, ablations, armour-test cases.
**Out (reason).** IS-2, SU-100 (S13 heavy-wave peers; SU-100 in service Oct 1944: a later Soviet heavy data wave, S15-3). 120 mm
PM-38 (peer of S13m's heavy mortars, ships with or after them). ZiS-2 (third AT gun, S15-4). BM-13 (Q23 open). Guards section, sniper
pair, SG-43 variant (variants with no peer card). Per-faction rules values (S15-9). Mirror matches (S15-10). Fixed bayonets (accuracy
effect; melee is S14's). A Maxim shield (no infantry shield exists). A second map, roads (S16). Art beyond tint and HUD colour.

## 3. S15a mechanics (Build A, Opus; every key in the layout commit at its Off value)
### 3.1 R1-R4 tank riders (*tankovy desant*)
| Rule | What | Keys and state | Off |
|---|---|---|---|
| R1 Deck slots | Validation **always** accepts `[transport] deck = true` on a tank, TD or AG (refused today, `content/vehicle.rs` 2523-2542), its `passenger_slots` boxes above a closed top (2583-2610), and an `entry` point on the deck; the "exactly on a transport" check (4220-4224) accepts `deck`. With `[transport_rules] riders` 0 the lowering drops the slots (the def plays as a plain tank). | def `[transport] deck`, `passenger_slots`, `entry`; `[transport_rules] riders` | `riders` 0 |
| R1b Riders are men | A man on a deck slot is a soldier target: every weapon may aim at him (today only weapons with a `penetration` table, and only at the first exposed slot, `combat.rs` 308-337), and a hit on his box takes the soldier path (`Blow::Hit` with an S12 zone, wounds, P1) instead of `HitVia::Rider` -> `hit_open` (`armour.rs` 805-824). Seen as a standing man (`sim/vehicle.rs` 917-928 sets the exposed bit for deck slots). Fragments and blast reach him as a man in the open (`damage.rs` 1144-1160). | none beyond `riders` | `riders` 0 |
| R2 Dismount on contact | Riders leave by a new `Exit::Deck`, the tank does not stop, when (a) a projectile or fragment touches the tank: hooks at `Blow::Armour` (`damage.rs` 398-410), `Blow::Stopped` (`combat.rs` 2063-2065, 3516), the fragment-on-plate branch (`combat.rs` ~3170-3200) and blast on the hull; (b) the tank fires its main gun (`gunnery.rs` ~572); (c) a rider is hit. `Exit::Deck`: stun `deck_stun_ms` (500, not `Exit::Forced`'s), each man placed beside the hull on the side away from the contact within `deck_drop_m` (2), the squad keeps its orders (not `Idle`, unlike `transport.rs` 535+). A knocked-out or burning tank uses `Exit::Forced` as today (precedence over R2). | `dismount_on_hit`, `dismount_on_fire` (1 when `riders` 1), `deck_stun_ms`, `deck_drop_m`; counters `rider_drops_hit`, `rider_drops_fire` | `riders` 0 |
| R3 Ride limits | Coax and bow MG fire keeps riders aboard (history); riders cannot fire from the deck (STATE: no fire from a vehicle). No speed cap with riders (no source). | none | - |
| R4 Embark | As every vehicle today: an own squad boards at the deck `entry` within `board_reach_cm` while the tank is no faster than `move_block_cm_s` (`transport.rs` 18-24). | existing keys | - |
Tests (each with its control): `deck_slots_inert_without_riders` (control `riders` 1: men board), `smg_aims_at_riders` (control: same men in
a closed half-track box are not aimed at), `rider_hit_takes_a_zone` (control `riders` 0: `hit_open`), `mg_round_on_hull_drops_riders`
(Stopped hook; control `dismount_on_hit` 0), `fragment_on_hull_drops_riders`, `pen_hit_drops_riders`, `main_gun_fire_drops_riders` (control
`dismount_on_fire` 0), `mg_fire_keeps_riders`, `rider_hit_drops_the_deck` (control `dismount_on_hit` 0), `tank_keeps_moving_on_drop`,
`squad_keeps_orders_after_drop` (control: `Exit::Forced` idles), `knocked_out_tank_forced_exit_wins`, save fuzz with riders aboard,
threads 1 vs 4.
### 3.2 P1 torso plate (SN-42)
- **Where.** A member slot may carry `plate_mm` (steel), `plate_zones = ["torso"]` (S12 zone names) and `plate_arc_deg` (front
  half-angle, 70): SN-42 covers front torso and groin only (research 1.5). A shot whose horizontal direction comes from outside the
  arc (against the man's yaw) misses the plate.
- **Bullets.** A new per-cartridge table `plate_pen = [[m, mm], ...]`, read **only** by P1. It leaves every weapon's `penetration` row
  and vehicle aiming untouched (adding `penetration` rows to an MP 40 or StG 44 would make them fire at vehicle crews, `combat.rs`
  296-312). A bullet that hits a plated zone inside the arc passes when `plate_pen(range) >= plate_mm / c`, `c` the cosine of the
  angle between the shot and the man's facing in the horizontal plane (a defined angle; capsules have no normal); else it stops and
  the man takes `plate_blunt_pct` (25) of its damage through the S12 wound check. `Blow::Hit` gains the shot distance and the shot
  direction (transient, as `Blow::Static` 2079). A cartridge with no `plate_pen` row passes (never a silent stop).
- **Fragments.** Compare `frag_pen_mm` after the fall-off used for `frag_damage` (`combat.rs` 3228-3232); a fragment def with no
  `frag_pen_mm` (satchels, HEAT, AT mine) passes the plate.
- **Data (S15b).** `plate_pen` rows for every small-arms cartridge of all three factions, each with `sources`; this is data read only
  by plated men, so GER-USA stays neutral (no German or US man has a plate). Rows (verify each): 9x19 and 7.62x25 stopped by 2 mm
  beyond about 50-100 m (the "100 m" is research (M): verify before T305a is hard); 7.92x57, .30-06, 7.62x54R pass at every game range.
- Keys: member `plate_mm`, `plate_zones`, `plate_arc_deg`; cartridge `plate_pen`; `[wounds] plates`, `plate_blunt_pct`. Off: `plates` 0.
- Tests: `plate_stops_9mm_at_100m` (control: no plate, wounded), `plate_fails_vs_792_rifle` (control: 9 mm at the same range stops),
  `plate_misses_from_behind` (control: same shot from the front stops), `plate_only_on_its_zone` (head hit unchanged), `no_row_passes`,
  `fragment_small_stopped_large_passes`, `satchel_fragment_passes`, save fuzz. Bench: `frontline-cli plate-bench` (F2) fires N rounds
  of a cartridge at a plated man at a range, angles uniform inside +-30° of his front, and prints stops (T305a-c).
### 3.3 C1 captured mortar bombs (82 mm fires German 8 cm bombs)
- **Rule.** A weapon may name `alt_feed = "ww2:8cm_wgr34"` with `alt_spread_pct` (200: research "x2", verify) and `alt_range_m` (2600,
  research 2.1). Fire spends the primary first. The mortar fire path picks the feed **before** the flight is computed (`combat.rs` 1191
  `take_bomb` runs after it today), so the alternate's spread and range cap apply to that shot; the AI mission range check uses
  `alt_range_m` when only alternate rounds are left. One way only: the German 8 cm gets no `alt_feed`.
- **Supply.** A cartridge def gains an optional `faction`, read only for alternate feeds (the lend-lease scout car's US .50 and .30
  stay usable by anyone as today). The refill passes the alternate feed's faction as the existing `gun_faction` filter, which already
  filters crates, own structures and trucks (`structures.rs` 1087-1118, 1144); every refill caller gets it (1236, 1340-1370 bearers and
  aid, 1462, 1542). So a Soviet mortar team takes German 8 cm bombs only from a German shells crate, dump or HQ stock it has captured
  or overrun, never from its own HQ, dump or Studebaker. Kilograms are paid as every crate's were at base (no free ammo).
- **Feeds.** The squad's feed slots hold both bombs (`content/setup.rs` 2373-2432; MAX_FEEDS 4, 336-338): the BM-37 crew carries one
  small arm (Mosin clip) so its feeds are Mosin, 82 mm, 8 cm (3 of 4). A content check refuses a squad whose feeds plus alternates
  exceed 4 (no silent drop). Only the German bomb: the card names it; all non-British 81 mm bombs fit (research), so the US M43A1 is a
  later second alternate if `alt_feed` becomes a list.
- Keys: weapon `alt_feed`, `alt_spread_pct`, `alt_range_m`; cartridge `faction`; `[supply] captured_feeds`. Off: 0.
- Tests: `alt_feed_draws_from_enemy_shells_crate` (control switch 0: no draw), `alt_feed_never_from_own_hq_dump_truck` (control: the
  primary refills there), `primary_spent_first` (control: primary empty, alternate fires), `alt_spread_wider` (bench, 50 rounds; control
  primary), `alt_range_capped` (control primary reaches 3,000 m), `german_mortar_has_no_alt`, `feeds_over_four_refused`.
### 3.4 F1 front end, F2 tools and counters (no sim rule change)
- **F1.** `bridge/match_setup.rs`: `FACTIONS` from the content's faction list (no fixed array); `MenuChoice` gains `opponent`; the
  skirmish menu and the lobby get an opponent picker (`main_menu.gd`, `multiplayer_panel.gd`). GDScript tables keyed by short id with a
  fallback entry, so a missing faction never crashes (section 1 sites). Mirrors stay refused (S15-10).
- **F2.** `RunSpec` gains `faction0` / `faction1` (`cli/targets.rs` 99-122, 400-410); `tools/measure.sh` and `batch.sh sets` take a
  matchup argument (default `germany usa`: today's runs unchanged); `pool.py --side ID` reports any faction's share (default
  `germany`); `ablate.sh` labels per matchup; targets rows may name a matchup (default GER-USA).
- **CLI S15 block** (`json_out.rs`, as S13's counters): `rider_loads`, `rider_drops_hit/fire/plan`, rider deaths on the deck vs after
  a drop, alternate rounds fired, plate stops by cartridge. Card buys use the existing `bought_matches_pct` (`json_out.rs` 425-469).
- **armour-test** env faction-selectable through S13's `scenarios::duel_cards`; **plate-bench** as 3.2.
- Tests: `menu_skirmish_picks_any_opponent`, `three_factions_load` (fixture), `three_faction_ai_with_ussr_unplayed` (the USSR loaded but
  not playing: `enemy_mg_weight` filters by player, `ai/assault.rs` 456-470), `pool_py_side_ussr`, every existing targets file unchanged.

## 4. Neutrality (the two proofs that let S15 merge without touching Germany vs USA)
- **N1 (S15a).** All S15a switches Off, no USSR defs: GER-USA 480 equal to `<BASE>` match for match (`neutral.py`), as S13a.
- **N2 (S15b).** The USSR defs, the `plate_pen` rows and cartridge factions are lowered into every match and shift shared sorted lists
  (weapons, ammo, cartridges, items, tags: `45mm_*`, `76mm_*`, `82mm_*` sort among existing ids; audit 6). GER-USA 480 before and after
  the data commit: equal match for match. If not, a rule iterates a global list in a result-changing order: find it, make it order-free
  (by def id or by player), prove N1 again. Never accept a GER-USA change as noise (owner rule: all off = base).

## 5. S15b data: the core wave (Sonnet, two data stages; every value with `sources` from `S15-research-history.md`; flags kept)
### 5.1 Cards (`defs/units/ussr_*.toml`; S15b weight 0; S15-ai weight by the peer card's weight)
| Role (GER / USA peer) | USSR card | Men | Kit (main) |
|---|---|---|---|
| Line squad full / field (Grenadiergruppe / Rifle Squad) | `ussr_strelkovoe_otdelenie` / `_6` | 9 / 6 | 6 Mosin, 2 PPSh, DP-27; 1 F-1 or 2 RGD-33 each (research 1.1) |
| MG (sMG-Gruppe / LMG) | `ussr_maxim_crew` | 4 | Maxim M1910/30 on Sokolov mount, no shield (none exists for infantry), 1,000 belt |
| Assault infantry (VG-Sturmgruppe) | `ussr_avtomatchiki` | 7 | 7 PPSh (71 drum), 2 F-1 each; tag `rider` |
| Engineers + construction (Pioniere / Baupioniere) | `ussr_sapyornoe_otdelenie` | 8 (verify) | Mosin/PPSh; capabilities `place_mines` (shared `ww2:at_minefield` / `ap_minefield`), `build_heavy`; tags `engineer`, `construction` |
| Assault engineers (Sturmpioniere / Assault section) | `ussr_shturmovaya_gruppa` | 8 (verify: an 8-man card is an abstraction) | PPSh, SN-42 (`plate_mm` 2, torso, arc 70), satchels, F-1; ROKS-3 pair if S14 flame shipped (S15-5) |
| Medium mortar (GrW 34 / M1) | `ussr_82mm_bm37_crew` | 5 (verify) | BM-37, O-832D 3.1 kg (verify 3.05-3.3); Mosin; `alt_feed` German 8 cm |
| Light mortar (kz. GrW 42 / M2) | **none** (50 mm withdrawn by 1944, S15-12) | | |
| Infantry AT (Panzerschreck / Bazooka) | `ussr_ptrd_team` | 2 | PTRD-41, 14.5 mm B-32 / BS-41 (reach row: flag, Build B cites); RPG-43 / RPG-6 as `single_shot_at` items with an 18 m range only if that kind can lob (Build B checks; else out and stated) |
| Tank | `ussr_t34_85` | 5 crew + 8 deck slots | 85 mm (D-5T / ZiS-S-53, one def), 56 rounds |
| TD / AG (StuG / M10) | `ussr_su85`; `ussr_su76m` (cheap, open-top casemate) | 4 / 4 | 85 mm D-5S; ZiS-3Sh, 60 rounds |
| Half-track (251 / M3A1) | `ussr_m3a1_scout_car` (lend-lease, 4x4) | 2 + 6 | .50 + .30 skate rail |
| Armoured car (234/1 / M8) | `ussr_ba64b` | 2 | DT, open-top turret |
| Truck and tow (Opel, SdKfz 11 / CCKW) | `ussr_studebaker_us6` | 1 | truck `[supply]` and gun tow |
| AT gun (Pak 40, Pak 38 / 57 mm M1) | `ussr_zis3`, `ussr_45mm_m42` | 7 / 6 | BR-350A/P, OF-350; BR-240/P, O-240 |
| Infantry gun (leIG 18 / 105 mm M3) | `ussr_76mm_m1943` | 6 (verify) | OF-350 low charge, BP-350M HEAT |
| Observer (VB-Trupp / FO party) | `ussr_razvedchiki` | 3 | radio, binoculars |
| Field howitzer (leFH 18 / M2A1) | `ussr_m30_122mm` | 8 | OF-462 21.8 kg (verify vs Shirokorad) |
| HQ, bearers, medics | `ussr_upravlenie_vzvoda`, `ussr_podnoschiki`, `ussr_sanitary` | 3 / 4 / 2 | SMG + pistol; rifles; medics with carbines (research 1.6) |
Prices at the peer's price (S15-11; Germany's 20 % infantry cut stays German, S5a-37). Dates (`available_from`): research 5 (skirmish
sets no date). Start squads: two full rifle sections and the sappers (as S4-14). **Roster order** (S13 G2 breaks vehicle-slot ties by
roster order when no heavy-wave card is in play): T-34-85, SU-85, SU-76M, then the rest.
### 5.2 Penetration rows (S15-2)
Rows are 0° obliquity at 100/500/1000/1500/2000 m, 50 % criterion as the mod's. **B&L** where found (45 mm BR-240 71/54/40/32/28,
BR-240P 108/74/46). **US 50 % calculated** where found (85 mm BR-365 142/125/107/92/78, BR-365K 139/123/105/91/81, BR-365P
175/136/100/73/54). Else **computed**: Build B writes `S15-research-rows.py` (as `S13-research-obliquity.py`): DeMarre from calibre,
mass and velocity, the constant fitted to the 85 mm US50 row, checked against the 45 mm B&L row (must land within 5 % at every range,
else stop and report); the 76 mm BR-350A and BR-350P rows come from it, the Soviet 75 % table (82/75/67/60/53) kept in `sources` as a
cross-check. BR-350P's quoted 92 / 60 (F-34, 965 m/s) is "criterion unknown". 14.5 mm B-32 / BS-41 go in the small-arms reach table.
**O1 slope class** (if S13's O1 is on main; S15-6): `apcr` for the P rounds (BR-240P, BR-350P, BR-365P); no class (cosine law) for the
uncapped sharp-nosed APHE (BR-350A, BR-365); `apcbc` only for a round Build B confirms carries a ballistic cap (BR-240, BR-365K).
### 5.3 Vehicles (thickness @ slope from vertical, mm; research 3)
| Vehicle | Hull front upper / lower | Sides upper / lower | Rear / top | Turret or casemate front / side / rear / roof | Traverse °/s | km/h | t |
|---|---|---|---|---|---|---|---|
| T-34-85 | 45 @ 60 / 45 @ 53 (verify) | 45 @ 40 / 45 @ 0 | 45 @ 45 / 20 | 90 / 75 @ 20 / 52 @ 10 / 20 | 17 (360° in 21 s) | 55 | 32 |
| SU-85 | 45 @ 50 / (verify) | 45 @ 40 / 45 @ 0 (verify; TE "30") | 45 / 20 | casemate front (verify) | casemate ±10 (M) | 55 | 29.6 |
| SU-76M | 25 @ 60 / 30 @ 30 | 15 @ 0 | 15 / open | casemate 25 @ 25 / 10 @ 20 / open / open | ±15 | 43 | 10.6 |
| BA-64B | 15 @ 30 | 6 @ 30 (vehicles.md 9-10; verify) | (verify) | open-top turret (verify) | hand | 80 | 2.4 |
| M3A1 scout car | 12.7 front, 6.35 sides | | | open | - | 80 | (verify) |
| Studebaker US6 | none | | | | | 72 | (verify) |
Frames, hatches, components and racks copy the nearest def: T-34-85 from `ger_pz4_h.toml` (turret) with the deck `[transport]`; SU-85
from `ger_stug3_g.toml` (casemate); **SU-76M from `ger_stug3_g.toml`'s casemate mount with `open = ["top"]`** (not the turreted M10);
BA-64B and **the scout car from `usa_m8.toml`** (wheeled; `usa_m3a1` is the half-track); US6 from `usa_cckw.toml`. T-34-85 rack 36 HE /
19 AP / 1 APCR (split (M), verify; `rare_cap` 1), 8 deck slots (official cap 12; one rider section). Guns: ZiS-3, M-42, M1943, M-30 as
gun entities after `usa_57mm_m1` and `ger_lefh18`.
### 5.4 Supply and presenter data
`ussr_*` HQ, dump, aid post, 4 crates, truck `[supply]` (copies of the USA's with Soviet names). Faction `color` `#5e4a35`
(`S1b-ui-theme.md` 43); shader `TEAM_COLORS[2]` set to the same earth-brown; HUD palette entry; vehicle `PAINT` entry.

## 6. S15-ai (Opus; keys and saved fields already in S15a's layout commit, all off)
| Rule | What | Keys and state | Off |
|---|---|---|---|
| H1 Rider tank stays a tank | A vehicle with `deck` slots is never taken by the transport job (`ai/transport.rs` 145-148 takes any vehicle with seats); it runs the tank habits. **Always on** for `deck` defs (the old path is a bug, not a variant). | none | - |
| H2 Load and drop | When a rider tank leaves base for an assault and an own infantry squad with the `rider` tag (avtomatchiki first) is idle in base, that squad embarks (one squad). R2 drops it on contact; else the AI disembarks it at `drop_m` (250; research 3.2 "200-300 m", (M) verify) from the assault objective and the squad joins the assault. The tank waits at most `wait_s` (20) for a squad. | `[ai.riders] enabled`, `drop_m`, `wait_s`; saved `AiState::rider_plan` (tank, squad); counter `rider_drops_plan` | `enabled` 0 |
Tests: `t34_not_taken_by_transport_job` (control: a half-track is), `riders_load_at_base` (control: no idle squad, tank leaves after
`wait_s`), `riders_drop_at_drop_m` (control `enabled` 0: no load), `riders_drop_on_contact_first`, save fuzz with a rider plan, threads 1
vs 4. Mortar, gun, supply, truck, medic and fort habits need only the USSR's AI tags (audit 4.0: a missing tag only idles a habit;
S15b sets every tag, and a content test lists each habit's tag per faction).

## 7. Stages (`workflows.md` 1-4: engine tier; no per-slice Tune; each Build checked by a separate agent; long runs follow the project rule)
| Part | Stage | Model | Content | Exit |
|---|---|---|---|---|
| S15a | A sim | Opus | ONE layout commit (SAVE +1; every key and field of 3, 6), R1-R4, P1, C1, tests 3.1-3.3 | `check.sh quick` |
| | F front end, tools | Sonnet | F1, F2, CLI S15 block, plate-bench, armour-test env, `s11a1_content.rs` loop fix | menu test; T300a |
| | Review, Fix | Opus | review gate (sim diff) | |
| | Measure | script | N1 (480 GER-USA = base) | T300 |
| S15b | B1 data | Sonnet | faction, infantry, items, cartridges with `plate_pen`, mortar, supply, medics, HQ | content tests; `validate`; T305a-c |
| | B2 data | Sonnet | vehicles, guns, rounds, `S15-research-rows.py`, presenter tables, baselines | content tests; armour-test smoke |
| | Measure | script | N2 (480 GER-USA before = after) | T301 |
| S15-ai | C AI | Opus | H1-H2, weights, armour-test `armour_test/s15.rs` cases 51-55 | `check.sh tests` |
| | Review, Fix | Opus | | |
| | Measure | script | 3 x 480 (GER-USA, USSR-GER, USSR-USA), `targets/S15.toml`; ablations on 480 for `riders`, `plates`, `captured_feeds` in both USSR matchups | T302-T304 |
| | Docs | Haiku | logs, STATE, TODO S15 block, `workflows.md` tier, design-doc fixes (section 9) | |
Run cost: N1 + N2 = 960 matches; 3 x 480 = 1,440; ablations 3 switches x 2 matchups x 480 = 2,880. About 5,300 matches, roughly
2.6 h at today's 14 min per 480 uncached. These are long runs: they go where the project rule says (memory: evening cloud or GitHub checks).

## 8. Measures and targets (`docs/slices/targets/S15.toml`; T300+: S13 holds up to T184; S14 picks its range below 300)
- **Hard.** T300 N1 and T300a every existing targets file unchanged. T301 N2. **T302a Germany's share of GER-USA in 35-65 %** (owner
  rule). T302b each side's share of USSR-GER and of USSR-USA in 35-65 % (480 each). T303 no match without a winner, median 15-30 min in
  each matchup. T304 the measure covers the faction: in USSR matchups the T-34-85 is bought in >= 50 % of matches, riders load in
  >= 25 % and a rider drop happens in >= 25 %, and every USSR card whose peer the AI buys is bought somewhere; else T302b says nothing
  about the USSR: stop and report. T305a SN-42 stops a 9x19 (MP 40) round at 100 m in >= 90 % of front-arc torso hits (plate-bench,
  +-30°; hard only once the 100 m figure is verified, else a report); T305b passes a 7.92x57 at 300 m in >= 95 %; T305c head hits
  unchanged with the plate.
- **armour-test** (10 runs each, each with a switch-off control): 51 T-34-85 frontal at 500 m vs Panzer IV H: report wins and hits by
  zone. 52 T-34-85 vs M4A3 (75) frontal at 500 m: report. 53 ZiS-3 BR-350A vs Panzer IV H side at 500 m: pens >= 8 of 10. 54 T-34-85
  with 8 riders under MG 42 fire at 300 m: riders drop on the first hull hit in 10 of 10, tank keeps moving. 55 PTRD vs Sd.Kfz. 251 side
  at 200 m: pens >= 8 of 10; vs Panzer IV front: 0.
- **Report.** T306 kills by and of each USSR card per matchup; T307 rider deaths on the deck vs after a drop; T308 alternate bombs fired
  per match; T309 plate stops per match; T310 HE share of casualties per matchup (against the 26.3 cap where it applies).
- If a hard target fails: **stop, diagnose, report** (no per-slice Tune). The levers the balance pass will use, in `vehicles.md` 5's
  order: cost, `unlock_s`, `cooldown_s`, `max_alive`, AI weights; never plates or rows.

## 9. Design-doc corrections (Docs stage; from research 7)
`guns-artillery.md` 6: 45 mm BR-240 to the B&L row 71/54/40/32/28 (BR-240P 108/74/46); ZiS-3 BR-350A Soviet row 82/75/67/60/53 labelled
Sov75, MV 680-700 (660 is the F-34); every Soviet row labelled with its criterion. `infantry-structures.md` 2.3: 120 mm crew 6; medics
carry rifles or carbines; shtat 04/550 is dated 10 Dec 1942 with 1944 changes, 05/40 from Dec 1944. `guns-artillery.md`: ZiS-3 and
ZiS-2 crews 7. `vehicles.md`: BA-64B sides 6 mm (verify); SU-100 in service 1944-10; T-34-85 56-60 rounds. `workflows.md` 1: S15 tier.

## 10. Balance risks
1. **The T-34-85 is a medium with a long gun** (BR-365 142 at 100 m): it beats the Sherman (75) frontally and matches the Panzer IV;
   its 45 mm plates fall to every German and US AT weapon. T302b decides.
2. **Computed rows** (5.2) carry a fit error; the 45 mm B&L check bounds it at 5 %.
3. **Weak infantry AT.** The PTRD does not hurt tanks from the front (authentic); RPG-43/6 may be out (5.1); the ZiS-3 and SU-76M carry
   the AT role. If USSR shares fall low, look here first (T306), not at the rows.
4. **Riders die.** R1b makes 8 men on a deck a target for every weapon; historic losses were very high. `drop_m` and R2 decide whether
   the habit helps; the `riders` ablation decides whether it ships on (S15-7).
5. **Merge load.** S13 and S14 touch rosters, baselines, append-tests, `[ai.*]` and the hook sites; start from a fresh main.

## 11. Owner questions (the part in brackets is the first affected)
**Settled on (A) (minor scope, data and price questions, owner rule 2):**
- **S15-1 Split [all].** (A) S15a, S15b, S15-ai, as S13. (B) One slice as the card says (0.6x; it is ~1.3x).
- **S15-2 Penetration rows [S15b].** (A) B&L, else US 50 % calculated, else computed by DeMarre fitted to the 85 mm and checked on the
  45 mm (5.2). (B) Soviet 75 % rows as printed (they under-rate the guns: the S3b H2 bug again).
- **S15-4 Third AT gun [S15b].** (A) ZiS-3 and M-42 only; ZiS-2 later. (B) ZiS-2 too.
- **S15-6 O1 slope class [S15b].** (A) If O1 is on main: `apcr` for P rounds, cosine for uncapped APHE, `apcbc` only for a confirmed cap.
  (B) Cosine law for every Soviet round.
- **S15-9 Per-faction rules [S15b].** (A) None in S15: one `gun_fire_direction_s` for all; the slower Soviet fire direction
  (`artillery-on-map.md` 4.8: 40 s; research 4) waits for a faction-keyed rules table. (B) Build that table now (S).
- **S15-11 Prices [S15b].** (A) Each card at its peer's price; the balance pass prices the faction. (B) A faction-wide discount now.

**Asked (how it should play); the work proceeds on (A) until answered:**
- **S15-3 Heavies [S15b].** (A) IS-2 and SU-100 out; a later Soviet heavy wave after S13's heavies are measured. (B) In S15.
- **S15-5 ROKS-3 [S15b].** (A) In the shturmovaya gruppa only if S14's flamethrower has merged; else the card ships without. (B) S15 waits.
- **S15-7 Riders ship on [S15-ai].** (A) On only if the `riders` ablation is neutral or better for the USSR in both matchups; else off,
  and human players keep the deck slots only when `riders` 1 (so off means no riders at all). (B) On regardless.
- **S15-8 Captured bombs [S15a].** (A) C1 as specified (alternate feed, spread, range cap, feed faction). (B) A one-flag shortcut: any
  enemy crate of the same kind, no accuracy cost. (C) Out.
- **S15-10 Mirror matches [S15a].** (A) Stay refused in the menu and lobby; the CLI allows them for tests; fix `ai/assault.rs` 465 when
  mirrors are wanted. (B) Allow mirrors and fix the AI now.
- **S15-12 Light mortar gap [S15b].** (A) No USSR light mortar (50 mm withdrawn by 1944). (B) A 50 mm RM-41 card for symmetry.

## 12. Review (revision 1, independent Opus reviewer, 2026-11-22): what changed
Folded in: P1 reads a new `plate_pen` cartridge table, not `penetration` (most small arms have none; adding rows makes them aim at
vehicles), has a front arc and a defined cosine, and passes fragments without `frag_pen_mm`; R2 names every contact hook (Armour,
Stopped, fragment-on-plate, blast) and a new `Exit::Deck`; R1 validation always accepts `deck` (switch at lowering) and the 4220 check;
R1b gives riders a man's aim and hit path; C1 filters by the feed's faction through `gun_faction` (own HQ, dump and trucks excluded),
names every refill caller, picks the feed before aiming, caps feeds at 4 with a content check; rows computed instead of mixing a 75 %
criterion; slope classes per cap; GER-USA hard in 35-65 and "stop, diagnose, report" instead of levers; CLI counters, plate-bench and a
faction-selectable armour-test in S15a; scout car from `usa_m8`, SU-76M from the StuG casemate; sappers by `place_mines`, RPG items
conditional, no Maxim shield; values not in the research marked (verify), flags kept; H1 always on; R4 uses the real boarding keys;
S13 target and case numbers, roster order for S13 G2 ties, `s15_off()`; controls on every test; size 1.3x and run cost stated;
how-it-should-play questions asked, not settled.
