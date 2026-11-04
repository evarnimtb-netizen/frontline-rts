# Slice S6: HQ, ammo dumps and crates

Owner-approved roadmap slice (~0.5x S3, engine tier). Design:
`docs/design/roster/roster-expansion.md` S6 card and Appendix A;
`artillery-on-map.md` §4.2 (ready inventories, crates), §4.9-4.10 (base
battery, exploits). Owner decisions in force: Q2 (C) HQ fills only by
carried crates; Q4 no ration, the base refills no mortar bombs (also ends
S5a-42); D1 every kg priced once at base; D2 no distance surcharge; D3 tanks
rearm forward at 1 round / 4 s, rare 1 / 30 s up to `rare_cap`. Branch
`s6-hq-crates` from `1575568`.

## Baseline (`1575568`, release, `frontline-cli targets docs/slices/targets/S6.toml --jobs 6`)

Wall 23:51 for all runs. The numbers match S5b's final measurements.

| # | Metric | Baseline |
|---|---|---|
| T4 | AI refusals (40) | DE 1 (`no_bombs` race, S5b-22), US 0: **miss** |
| T5 | German wins 40 / 160 / 160 oos | 40.0 / 47.5 / **40.6 %** (oos miss) |
| T6 | base side max | 60.0 / 53.8 / 60.6 % |
| T7 | length (40) min / median / max; median 160 | 11.9 / 17.1 / 30.6; 17.3 min |
| T8 | HE share mean 160 / oos / 40 | **13.9 / 13.7** / 13.3 % (band 20-25 missed) |
| T9 | mortar share of kills DE / US (160) | **5.3 / 4.6 %** |
| T10 | bombs per match DE / US; MP on ammo per match; totals over 160 shells / small arms / explosives MP; bombs fired from inside own base | 68.8 / 56.3; 244.9 / 186.9; DE 38,593 / 588 / 7.0, US 29,519 / 366 / 11.5; 8.4 / 6.0 % |
| T11 | AI observed+direct (40) | 82.1 / 80.3 % |
| T13-T15 | fortify/armour/stress; tick p99 peak (`timing40`), stress; matches without a winner | PASS; 0.475 ms, 1.492 ms; 0 / 0 / 0 |
| T20-T27 | `s6_derived` metrics | not built yet ("no such number") |

Summary line: 28 pass, 24 miss (16 hard: T4a, T5d, T8a/b, T9a/b and the 10
hard `s6_derived` ones). JSON: scratchpad `s6/baseline/`.

## Code map (what changes, file:line at `1575568`)

| Area | Where | Change |
|---|---|---|
| Base bomb refill | `crates/sim/src/supply.rs:617-745` `refill_men` (bombs from 698), `:747` `bombs_wait` | base refills no feed of a `crate_only` ammo kind; bombs only from sources below |
| Bomb draw | `supply.rs:923` `take_bomb` | unchanged; ready bombs refill via new `supply_sources` pass |
| Vehicle rearm | `supply.rs:813-917` `rearm_vehicles`; `damage.rs:974-1047` base test, `base_ticks` | add forward rearm (HQ, dump, crate) on a `fwd_ticks` timer; rack credit (D4) |
| Supply tick | `supply.rs:487` `supply_tick` | add `structures_tick` (build, stock in), `sources_tick` (men, mortars, tanks) |
| Static boxes | `combat.rs:1766` `first_box` (bullets `:1861`), `:2190` `fragments` (vehicle list `:2266`), `damage.rs:718` blast | new `StaticDerived` index (`statics.rs`): oriented boxes of structures and crates tested beside vehicle boxes; hits take box HP |
| Cook-off | `combat.rs` burst path (`Burst`, `fragments`) | a broken shells/explosives box queues bursts of the kind's cook-off HE def |
| Pit profile | `dig.rs` (job, stamp), `sdk/crates/content/src/setup.rs:340` `DigProfile` | `MortarPit`, `HqPit`, `DumpPit`: rectangle with all-round parapet; `allowed_tags` per profile |
| Commands | `crates/sim/src/command.rs:26` `CommandKind`; apply `game.rs:884` | `Build`, `IssueCrate`, `PlaceCrate`, `PickUpCrate`; refusals as `SupplyEvent` |
| Retreat | `game.rs:1007` (`_ => self.base_of(p)`), `squad.rs:391` | retreat goes to the nearest own standing HQ, else base |
| State | `game.rs:509` `MatchState` | `structures: BTreeMap<u32, Structure>`, `crates: BTreeMap<u32, Crate>`, `next_structure`, `next_crate`, `crate_queue: Vec<u64>` (per player), `cookoffs: Vec<CookOff>`, `S6Account` per player |
| Save/hash/validate | `world.rs:423` `SAVE_VERSION`, `:199` `validate`, `:732` `load`; `hash.rs:335` `put_game`; `game.rs:1708` `validate` | SAVE_VERSION 15; every field put, validated, one test per field |
| Snapshot | `snapshot.rs:494` | `structures`, `crates`, carrier per man, `supply_events` |
| Content | `sdk/crates/content/src/modset.rs:501` (def folders), `setup.rs:210/275` | new folder `defs/supply/` (hq, dump, crate defs), `[supply]` and `[ai.supply]` keys, schema |
| AI | `ai.rs:729` `run_ai`, `:923` `retreats`, `:951` `buy`; `ai/ammo.rs:86` `refills`; `ai/armour.rs:25-124` rearm; `ai/mortar.rs:174` | new `ai/supply.rs`: HQ siting, crate runs, tank crate runs, budget |
| CLI | `sdk/crates/cli/src/json_out.rs:185` `faction`, `skirmish.rs:415` counters | `s6_derived` per faction, `s6` per run |
| Net | `crates/net/src/protocol.rs:28` | PROTOCOL_VERSION 9; `sdk/docs/net/relay-protocol.md` |
| Bridge | `crates/godot-bridge/src/intents.rs:190`, `match_frame.rs` | intents `build`, `issue_crate`, `place_crate`, `pick_up_crate`; frame fields |
| HUD | `godot/match/hud/detail_panel.gd`, `orders_grid.gd`, `roster.gd`, `godot/match/ground_overlay.gd` | stock bars, radius ring, carry state, placement ghost |

## Data model

### Supply defs (`mods/ww2/defs/supply/`, new folder, one file each)

```toml
# defs/supply/ger_gefechtsstand.toml
id = "ww2:ger_gefechtsstand"
name = "Gefechtsstand"            # the HQ structure; Zugtrupp is the squad
faction = "ww2:germany"
kind = "hq"                       # hq | dump | crate
cost = 100                        # MP, paid when the order is accepted
pit = "hq_pit"                    # dig profile dug before it stands
radius_m = 25                     # men, mortars and tanks inside are served
cap_kg = { small_arms = 300, explosives = 200, shells = 600 }
rebuild_s = 120                   # s after destruction before a new HQ
built_by = ["hq"]                 # squad tags that may build it
# oriented boxes, centre offset from the pit centre; hp in fragment/bullet damage points
boxes = [
    { part = "stock", size_cm = [200, 120, 90], offset_cm = [0, -150, 0], hp = 400 },
    { part = "radio", size_cm = [50, 40, 40], offset_cm = [120, 80, 0], hp = 40 },
]
```

```toml
# defs/supply/ger_shells_crate.toml
id = "ww2:ger_shells_crate"
faction = "ww2:germany"
kind = "crate"
supply = "shells"                 # small_arms | explosives | shells
kg = 32                           # paid at issue: kg x shell_mp_per_t / 1000
carry_speed_pct = 70              # the carrier's pace
boxes = [{ part = "crate", size_cm = [70, 40, 35], hp = 60 }]
```

Per faction: one HQ (Gefechtsstand / Command Post), one dump
(Munitionslager / Ammo Dump, `radius_m` 15, caps 150/100/300 kg, `hp` 250),
three crates (small arms 25 kg, explosives 20 kg, shells 32 kg).

### Rules (`defs/rules/skirmish.toml`)

```toml
[supply]                          # S5a keys stay; base_bomb_s removed (base refills no bombs)
crate_feed_m = 5                  # m: a crate this close feeds a mortar, men and a tank
base_crate_issue_s = 10           # s per crate, one queue per player
crate_cap = 12                    # live crates per player
shell_crate_cap = 8               # of which shells
hq_max = 1
dump_max = 3
structure_bomb_s = 2              # s per bomb into a mortar squad inside a radius
crate_bomb_s = 4                  # s per bomb from a crate within crate_feed_m
fwd_rearm_s = 4                   # D3: one standard main-gun round, forward
rare_rearm_fwd_s = 30             # D3: one rare round, forward, to rare_cap
serve_men = 6                     # men a source serves per serve_period_ms
serve_period_ms = 500
unfired_s = 3                     # a man is served when he has not fired this long
crate_to_stock = true             # a crate set down inside a radius empties into it
cookoff = { shells = "ww2:cookoff_shells", explosives = "ww2:cookoff_expl" }
cookoff_kg_per_burst = 16         # one burst per this many kg, at most 8, over cookoff_s
cookoff_s = 6
bullet_box_damage = 4             # HP a rifle-calibre bullet takes from a box
[ai.supply]
hq_behind_m = 120                 # behind the most valuable held point
hq_probe_rays = 8                 # rays from known enemy positions for the reverse slope
crate_run_below_pct = 50          # a mortar or HQ below this share gets a run
tank_crate_below_pct = 25         # a tank this low that cannot reach a source
shell_mp_max_pct = 25             # share of income spent on shells (crates)
```

Cook-off HE defs are `defs/ammo/cookoff_*.toml` (HE only, no flight).

### Sim state

`Structure { id, owner, def, pos, facing, built: bool, dig: DigJob ref,
box_hp: [u16; 2], radio: bool, stock_g: [u32; 3], destroyed_tick }`;
`Crate { id, owner, def, pos, carrier: Option<SimId>, g: u32, hp: u16 }`;
`CookOff { pos, owner, def, bursts_left, next_tick }`; per player
`S6Account { crates_issued, crate_mp_micro, crates_delivered, stock_in_g,
base_bomb_refills, unsourced_bomb_refills, tank_rounds_fwd, cookoffs,
hq_built_tick }`. Carried weight lives on the crate (`carrier`), so a
man's kit layout does not change.

## Save, hash and versions

One bump each for the slice: `SAVE_VERSION` 15, `PROTOCOL_VERSION` 9 (four
commands). Every new field is put in `hash.rs` `put_game`, saved, and
validated on load (ids below `next_*`, carriers alive and own, `g <= kg`,
stock <= cap, at most `hq_max`/`dump_max`/`crate_cap`, queue entries per
player). One `load_rejects_*` test and one hash-sensitivity test per field
(pattern: `world.rs:3404`). `StaticDerived` is derived (rebuilt on load, not
hashed, `PartialEq` always true, like `VehicleDerived`). Relay: a command
stays far below the 1,024 B single-command limit; doc updated.

## Sim rules

1. **Base.** Unchanged for small arms, items, rockets and tank racks
   (priced, S5a). It refills **no** mortar bomb (`crate_only` kinds).
2. **Issue.** `IssueCrate { squad, kind }`: a man of the squad in its own
   base takes one crate of the faction's def; paid kg x price at issue (no
   manpower: refused). One crate per `base_crate_issue_s` per player,
   FIFO. Caps refuse. A carrier moves at `carry_speed_pct`, does not fire
   his primary, and drops the crate where he falls (it stays the owner's).
3. **Place / pick up.** `PlaceCrate { squad, pos }` walks the carrier there
   and sets it down; inside an own built structure's radius it empties into
   the stock up to cap (`stock_in_g` counts only this path) and the rest
   stays as a crate. `PickUpCrate` by any own man within 2 m.
4. **Sources.** Each source serves `serve_men` men per period, ascending
   id, alive, not pinned, not fired for `unfired_s`, most depleted first;
   one source per client per service tick, the fastest in reach (rates
   never stack). Mortar squads: one bomb per `structure_bomb_s` inside a
   radius, per `crate_bomb_s` from a crate within 5 m of the gunner; no
   source, no bomb (counter `unsourced_bomb_refills` must stay 0).
   Draws charge nothing (paid at issue).
5. **Tanks** inside a radius or within 5 m of a shells crate: one standard
   round per `fwd_rearm_s`, rare per `rare_rearm_fwd_s` to `rare_cap`, from
   shells kg; the D4 rack factor credits the difference to the owner. An
   empty stock rearms nothing.
6. **Build.** `Build { squad, def, pos, facing }` (squad tag in
   `built_by`): refused at the cap, during `rebuild_s`, without manpower or
   in the enemy base; the squad digs the def's pit; it stands when the pit
   is done. Starts empty.
7. **Boxes.** Built structures' boxes and crates enter `StaticDerived`.
   Bullets and fragments stop at the first box (same tie rule as vehicle
   boxes, earth first); fragments take `frag_damage`, bullets
   `bullet_box_damage`, blast its overpressure damage within its radius.
   HP 0: a crate or stock box is gone with its kg; a radio box sets
   `radio = false`. When HE (fragment or blast) breaks a box holding
   shells or explosives, a `CookOff` fires bursts of the kind's HE def at
   the box, owned by the player whose HE broke it (no friendly-fire change),
   kills credited to that HE's kill class. Bullets break without cook-off.
8. **Retreat** goes to the nearest own built HQ, else the base.
9. **Pits.** `MortarPit` (2.0 x 2.0 m, 1.1 m deep, all-round parapet),
   `HqPit` (3.0 x 2.5 m, 1.5 m), `DumpPit` (2.5 x 2.0 m, 1.2 m): the job's
   line is the pit's long axis, segments like a trench, `all_round` spoil.
   `allowed_tags`: mortar pit for `mortar`, `engineer`, `hq`.

## AI (`ai/supply.rs`, new; called after `refills`)

- Buys one Zugtrupp after its first infantry wave (mix weight in
  `[ai.mix]`), and an Ammunition Bearers card when it has a mortar.
- **HQ siting:** candidate points `hq_behind_m` behind the most valuable
  held point toward the own base, 8 per ring; keep those with no line of
  sight from known enemy positions and enemy-side points
  (`hq_probe_rays` terrain rays, the one new ray cost); pick the best by
  score (reverse slope depth, distance), ties by index.
- **Crate runs:** bearers take a shells crate to the HQ (deposit) or set
  it within 5 m of a mortar below `crate_run_below_pct`; small-arms crates
  to a held trench's squad. Budget `shell_mp_max_pct` of income.
- **Tanks (owner 2026-11-01):** a tank below `tank_crate_below_pct` that
  is immobilised or farther than its path budget from any source gets a
  shells crate carried to within 5 m; otherwise it drives to the nearest
  source as today.
- **Stall guard:** S5a `rearm_stall_s`, `ammo_float_mp` kept; a mortar
  squad with no bombs and no run within 120 s walks to the HQ/base.
- **Targeting:** seen enemy HQ boxes and crates add score in
  `mortar_targets` (`ai/mortar.rs:421`).

## HUD (HUD v2 layout, military UI direction)

- Detail panel for a structure: name, three stock bars (kg / cap per kind,
  low marker at `low_pct`), radio state; build progress while digging.
- Radius ring (ground overlay) for selected structures and during
  placement; crate rings 5 m when a mortar is selected.
- Roster and squad card: crate icon with kind and kg on a carrier.
- Orders grid: Build HQ / Build dump (Zugtrupp), Take crate (kind),
  Place crate, Pick up; refusal text from `SupplyEvent`.
- Godot F5 data reload keeps working; tests in `test_match_view`.

## Content, with sources (cite in TOML comments; weights "verify" where not sourced)

| Def | Notes and sources |
|---|---|
| `ger_zugtrupp` (Zugtrupp), `usa_platoon_hq` (Platoon HQ) | 5 men: leader, deputy, 2 runners, radio man (US: platoon leader, sergeant, guide, 2 messengers); tag `hq`; dig 100 %; TM-E 30-451 (Handbook on German Military Forces, 1945); FM 7-10 (Rifle Company, 1942) |
| `ger_munitionstraeger`, `usa_ammo_bearers` | 4 men, rifles, tag `bearers`, each may carry a crate; FM 23-90 (81-mm mortar, ammunition bearers), TM-E 30-451 |
| HQ, dump defs per faction | caps from Appendix A; HQ 100 MP (A.2 table) |
| Crates per faction and kind | shells 32 kg (8 cm Wgr 34 case 3 bombs ~14 kg, two cases a man; US 81 mm M43A1 packed per TM 9-1904, verify); small arms 25 kg; explosives 20 kg |
| Mortar pit, HQ and dump pits | `[dig.profiles.*]`; FM 23-90 (mortar emplacement), FM 5-15 (Field Fortifications, 1944) |
| Cook-off HE defs | fragments as a 75 mm HE burst scaled down; verify |

## Targets (`docs/slices/targets/S6.toml`; cargo-test targets in its header)

Carried over from S5b: T4 refusals 0, T5 faction 35-65 % (40) and 45-55 %
(both 160 sets), T6 base side <= 62 %, T7 length 10-40 min with median
15-30, T11 observed+direct >= 70 %, T13 fortify/armour PASS, T14 tick p99
< 1.5 ms (`--jobs 3`) and stress < 10 ms. Changed or new:

- **T8** HE share mean 20-26.3 % hard on both 160 sets (owner band 20-25
  reported); <= 26.3 % on 40. **T9** mortars 10-15 % of each side's kills.
- **T10** reported: bombs per match, MP on ammo per kind, crate MP.
- **T12** (cargo) determinism 1/2/3/8 threads, save/load, per-field tests.
- **T15** stall guard: every match has a winner.
- **T20** base bomb refills 0 and unsourced bomb refills 0; (cargo) a dry
  mortar with no crate in reach stops. **T21** (cargo) a crate within 5 m
  keeps it firing until empty.
- **T22** stock enters HQ/dumps only from carried crates (0 kg otherwise).
- **T23** (cargo) cook-off only when HE fragments or blast reach the box;
  cook-offs per match reported.
- **T24** (cargo) tank at an HQ: 1 round / 4 s, rare 1 / 30 s to
  `rare_cap`, stock falls by the same kg; an empty HQ rearms nothing.
- **T25** (cargo) second HQ, fourth dump, 13th crate refused; a dead
  carrier delivers nothing. **T26** (cargo) a crate costs kg x price
  anywhere; no manpower, no crate; draws are free.
- **T27** AI builds an HQ in >= 70 % of matches per side, >= 3 crates
  delivered per match; tank crate runs reported.

## Build order (checkpoint commit after each)

1. Content: supply defs, rules keys, pit profiles, cards, schema, validate.
2. Sim A: `StaticDerived`, structures, crates, commands, sources, base
   bomb stop, tank forward rearm, cook-off, retreat; save/hash/validate;
   cargo tests T12, T20-T26.
3. CLI counters and `s6_derived`; first targets run (expect mortars weaker).
4. AI B: `ai/supply.rs` (HQ siting, crate runs, tank crates, budget,
   targeting); targets run.
5. Bridge, snapshot, HUD, Godot tests and screenshots.
6. Tune: `shell_mp_per_t`, bomb loads, AI mission size and map fire
   (never lethality) toward T8/T9; then the S5b German price autotune
   input (x0.959) if the share still lags.

## Owner questions (work proceeds on (A); also in `docs/TODO.md`)

- **S6-1 Crate to a tank.** Appendix A says tank rounds are too heavy for
  carried crates; the owner asked (2026-11-01) for supply brought to a
  tank. (A) A shells crate within 5 m rearms a tank at the forward rate
  from its kg (32 kg is about 3 rounds of 75 mm). (B) Tanks rearm only at
  base, HQ and dumps.
- **S6-2 Cook-off ownership.** (A) Bursts belong to the player whose HE
  broke the box: they hurt the box owner's men only (no friendly fire until
  S11a, Q22). (B) Neutral bursts that hurt everyone.
- **S6-3 A dead carrier's crate.** (A) It drops where he falls and stays
  his player's; the enemy can neither draw from nor take it. (B) Lost.
  (C) Capturable.
- **S6-4 HQ construction material** (Appendix A: 100 MP + 120 kg). (A)
  100 MP and the dig time only; material comes with S10 blueprints.
- **S6-5 Bullets and boxes.** (A) Bullets stop at boxes and take
  `bullet_box_damage` HP; only HE sets off a cook-off. (B) Bullets pass
  through crates.
- **S6-6 Who digs a mortar pit.** (A) Mortar teams (pit only, 40 % dig
  rate), engineers and the Zugtrupp. (B) Engineers only.
- **S6-7 The radio box.** (A) A state only in S6 (HUD shows it); S11
  observers use it. (B) Also gates observed fire beyond voice range now.
- **S6-8 Build cost.** (A) Paid when the order is accepted, no refund if
  cancelled. (B) Paid when the pit is done.
- **S6-9 Retreat.** (A) To the nearest own built HQ, else base; repair and
  new crew stay base-only. (B) Base always.
- **S6-10 Stock box broken.** (A) The whole stock is lost; cook-off bursts
  scale with the shells + explosives kg (one per 16 kg, at most 8).
