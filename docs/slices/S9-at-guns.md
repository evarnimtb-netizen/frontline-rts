# S9: AT guns

Status: Plan (2026-11-08). Branch `s9-at-guns` from `1ad2b52`. Sources: `docs/design/roster/guns-artillery.md` 3, 4, 5.2,
6-9, 11, `roster-expansion.md` "S9", `vehicles.md` 6.10. Targets: `docs/slices/targets/S9.toml`. SAVE 19 → 20, PROTOCOL 11 → 12.

## 1. Baseline (verified S8b JSON at `scratchpad/verify-src`, no rerun)

Germany 41.9 / 47.5 / 50.0 %, pooled 46.5 % over 480 (T5 is judged on the pool). HE share mean 20.5 / 20.3 / 20.1 %. Mortar
kills about 13.5 % (GER) / 11.5 % (USA). Top vehicle share of kills: Pz IV 17.1 %, USA 14.6 % (T32 cap 18). Vehicle
manpower 36.0 / 35.5 % (S8b-26; T33 now counts tanks/TDs only: 22.3 % GER, from the saved card shares). Tick p99 from a
clean `timing40`: 0.441 all / 0.532 peak. Checks: quick 610, release 644 (S8b). Plan-stage check of S9.toml against the
saved JSON: 203 pass, 64 miss (31 hard: T5c, T50e, T33a/b need a fresh batch, the rest name fields the builds add).

## 2. Code map

The audit found one thing that differs from the design text: ownership already changes. `bail.rs:927` sets
`ents.owner[i] = p` when a re-crew captures an abandoned tank (S3c). What S9 adds is a *neutral* gun and an owner change
that does not consume the crew.

| Area | File:line | What changes |
|---|---|---|
| Entity kind | `ecs.rs:131` | Nothing new. A gun is `EntityKind::Vehicle`; neutral is `Abandoned`. |
| Vehicle state | `vehicle.rs:162` | `VehicleState::served: Option<Served>` (guns), `towing: Option<SimId>` (tows). |
| Seats as soldiers | `vehicle.rs:419,428` (`seat`,`manned`) | Keep them: slots mirror into `vs.seats` every tick (section 4); no gunnery rewrite. |
| Traps | `vehicle.rs:439,456,468` (`gun_out`,`immobile`,`immobilised`), `:1645,:1842` (driver) | `immobilised` reads wheels for served guns; `drive` skips them. |
| Vehicle spawn | `game.rs:1724`, `:1484` (`call_in`) | Spawns the gun, its crew squad, and optional tow hitched with the crew riding. |
| Pop and limit | `game.rs:891`, `:949` (`outside_limit`) | `AtGun`, `Tractor` outside the limit; gun `pop = 1` (validator range 1..). |
| Bail | `bail.rs:182` (`can_bail`) | No hatches, so never bails; neutral rule separate. |
| Recrew/capture | `bail.rs:220,270,856` | `recrew_check`, `order_recrew`, `man_vehicle` branch on `crew_model`; men stay soldiers. |
| Abandoned cap | `bail.rs:982` | Counts neutral guns; a very old one is scrapped. Accepted. |
| Gunnery | `gunnery.rs:444,645,1160,1247` | Work unchanged through mirrored seats. |
| Target choice | `gunnery.rs:906,962,995` | Spotted served gun: HE at the crew point. |
| Laying | `gunnery.rs:208,633` | Casemate mount with the gun's arc; re-lay from `lay_turn`. |
| Spotting | `spotting.rs:105,:300` | Optic inside the arc against vehicles; `spot_size_pct` replaces the moving boost for guns. |
| Bullets | `combat.rs:2200,2663` | Nothing: the shield is a box, a miss meets the man's capsule (ordinary trace). |
| Personal fire | `combat.rs:381,695` | A man at a slot holds fire unless the gun has no target and an enemy is within `self_defence_m`. |
| AP core | `damage.rs:222,852,1090` | New: after a shield or carriage penetration, serving men on the next 1.5 m of the round's line take the core's damage. |
| Blast on parts | `damage.rs:1125` | Sight/recoil/wheel anchors take `vs_gun_pct` (parts ordinary, shield blocks frontal fragments through `hit_armour`). |
| Components | `content/vehicle.rs:156` | `Sight`, `Recoil` added (COMPONENTS 12); wheels reuse `WheelsFront`; barrel stays part of `Gun`. |
| Supply source | `structures.rs:1041` (`source_for`), `supply.rs:839` | A gun draws only from sources of its def's faction (section 9). |
| Crate owner | `structures.rs:296,424,436` | A squad can pick up an enemy crate lying still; owner flips, def faction stays. |
| Dig | `dig.rs:328`, `content/setup.rs:403` | `DigProfile::GunPit` (all round, short, engineer-only). |
| Hash/save | `world.rs:199,435,4256`, `hash.rs` | New fields by postcard; validate links; per-field tests. |
| Snapshot | `snapshot.rs:278,952,967` | Gun mode, tow link, serving flag, neutral. |
| AI | `ai.rs:786,800` (`ai_kind`,`mix_slot`), `ai/truck.rs:756`, `ai/overwatch.rs` | Guns on their own buy; siting job reuses overwatch scoring. |
| Reports | `cli/skirmish.rs:408`, `json_out.rs:358,421`, `armour_test.rs:3901` | `s9`, `s9_derived`, cases 33-40; `tank_td_mp_pct` done in the Plan. |
| Presenter | `godot/match/vehicles.gd`, `vehicle_view.gd`, `match_input.gd`, `hud/` | Gun blockout, states, buttons, neutral tint. |
| Net | `crates/net/src/protocol.rs:32`, `command.rs:27` | Six new commands, appended. |

Vehicle owner assumptions checked: `ai.rs:992,995` and each job's `retain` key on `q.owner == me` and the squad id, and a capture makes
a new squad, so stale jobs drop out. `VehicleFrame.owner` is rebuilt from `ents.owner` per tick. Hash: `mark_entity_chunk` on the owner write. S8b traps
(parts a def lacks read 0, `gun_out`, `immobilised`, `mounts[0]`) are named above; never index seats by role without `seat()`.

## 3. The gun entity

Reuse, not a new entity: a gun is a vehicle def with `role = "at_gun"`, `crew_model = "served"`, `mobility.type = "towed"`, a
squad of one (`Squad.members == [gun]`) like every vehicle, so orders, selection, the vehicle index, damage, reports and `kills_by_vehicle` work.
Its `crew_squad` names a soldier squad (tag `gun_crew`, capability `gun_crew`, not tag `crew`, which the AI sends home).

Parts a def has: `Gun` (breech and barrel box), `Sight` (anchor), `Recoil` (box), `WheelsFront` (anchors, `is_running_gear`), `AmmoRack`.
Destroyed `Gun` wrecks the entity (it is the knock-out a report counts, S9-11). `Recoil` out: cannot fire until field-repaired. `Sight` out: spread ×2.5, damaged ×1.5.
Field repair reuses `[[components]] repair` with at least 2 crew at slots.

Geometry (gun frame cm) is thin on purpose: the design's carriage box would swallow the kneeling men, so a bullet from the flank would stop at it.
- `[[boxes]]`: `shield` (front plate 8 mm for two spaced 4 mm, verify), `axle` (low, wide, 30 cm long), `trail` (low, 40 cm wide, 280 cm long, rear).
- Validator rule: no service slot's capsule inside any box but the shield's.

## 4. Crew service slots

The def lists `[[service]]` slots (`role`, `at` gun frame, `pose` kneel/prone). A served gun's `seats` def is built from them by content (Gunner, Loader, Commander;
`ammo` slots extra). State:
```rust
pub struct Served { pub crew: Option<SquadId>, pub slots: Vec<Slot>, pub state: GunState, pub timer: u16,
    pub hitched: Option<SimId>, pub hold_fire_m: u16, pub abandon_ticks: u16, pub relay_ticks: u16 }
pub struct Slot { pub man: Option<SimId>, pub swap: u16 }
```
Soldier: `Unit::serving: Option<Serving { gun, slot }>` (the `riding` pattern). Crew are ordinary soldiers: spatial hash, spotting, suppression, wounds, kits.
- Assignment (ascending soldier id): lowest slot index first. The men walk to the slot (`UnitOrder::Serve`), keep the slot's stance; each stays until ordered off.
- Each tick (serial, in the Status stage before the mirror is read): the slot's `Seat` is written from the man: hp, `swap`, `untrained` (squad lacks `gun_crew` capability). A missing man is hp 0. So
  `gunnery.rs` manned/reload/spread and `spotting.rs` commander rules run unchanged. A pinned gunner reads as not manned.
- Auto-swap: a dead gunner's slot is taken by the lowest-priority living man after `slot_swap_s` (3); loader next. `untrained_reload_pct`/`untrained_spread_pct` (150) from `[guns]`.
- A man at a slot does not fire his rifle while the gun has a target; with none and an enemy within `self_defence_m` (40) he fires.

## 5. Shield and small arms

No new rule. The shield is a box in `armour.rs`: a round that strikes it stops or follows the plate rule (`Blow::Stopped`/`Blow::Armour`); a round that misses reaches the man's capsule. The shield is 2.2 m wide, top 1.4 m.
AP through a shield: the 1.5 m core step (section 2). Bullets over or round it hit the man. The S8a thin-plate fragment rule covers fragment rays. Done-when numbers rest on geometry: case 33 (T73).

## 6. Mobility

`GunState`: `Limbered`, `Emplaced`, `Emplacing`, `Packing`, `Pushing`, `Hitched`, `Hitching`. All values in `[mobility]` of the gun def (seconds or cm/s).

| Transition | Needs | Time |
|---|---|---|
| Emplace (facing) | Limbered, at least `min_men` (2) at the gun within `hold_m` | `emplace_s` (Pak 40 30, 57 mm 25, Pak 38 20 verify) |
| Pack | Emplaced, crew near | `pack_s` |
| Push to point | Limbered, wheels ok, slope ≤ `push_max_slope_pct` | `push_cm_s` × (men ÷ `push_men`), path on wheel layer, `wheel_block_dm` 6 |
| Hitch | Limbered, tow stopped within `hitch_reach_m`, crew near | `hitch_s` |
| Unhitch | Hitched, tow stopped | `hitch_s`, then Limbered |
| Re-lay trail | Emplaced, target outside arc (`lay_turn`, or `FaceUnits`) | `relay_trail_s` per turn; cannot fire meanwhile |

Hitched: the tow's `towing = Some(gun)`, gun's `Served.hitched = Some(tow)`. Each Movement tick after the tow's `drive`, the gun's transform is a rigid bar behind the tow's hitch point, `settle`d. The tow moves at
`speed × towed_speed_pct` (data, per tow `[tow] max_kg`). A hitched or moving gun cannot fire. Gun yaw, trail and crew slots all follow. Tow death: gun Limbered where it stops.
Crew ride the tow when it has `[transport]` (Sd.Kfz. 11 8 seats, M3A1 12); CCKW does not carry men in this slice (S9-3), its crew walk.

Call-in: a card with `tow` pays gun + tow, spawns the tow in the base with the gun hitched and the crew riding. The tow's `max_alive` gates only the stand-alone tow card (`Sd.Kfz. 11`, 180 MP, 45 s, pop 2, max 2).

## 7. Gun optic and concealment

Spotting (owner question 12 (A), S9-1): a gun whose gunner is at his slot, against a vehicle inside the gun's arc, sees at `sight_m × optic_sight_pct / 100` (175 m × 250 % = 437 m), ignoring the target's size and moving boosts (which would give 875-1090 m, against the design's "400-500 m"). Outside the arc, or with no gunner, the gun sees like a crew (`sight_m` × the target's normal size factor).
Concealment: `spot_size_pct` 90 (Pak 40), woods factor as usual. Firing reveals: `fire_reveal_pct` 300 for `fire_reveal_ms` 5000 on the gun's weapon. A tank sees a silent Pak 40 only at 175 × 0.9 = 157 m.
`SetAmbush { guns, range_m }` holds a gun's fire until a target is within `range_m` (0: off); it fires at any target that fires on it or comes inside `self_defence_m`. Stored in `Served.hold_fire_m`.

## 8. HE at the crew

In `pick_target`, a served enemy gun in the vehicle loop (`gunnery.rs:995`) is a candidate like any target, but the aim is HE at its gunner slot position (`Aim { vehicle: false }`, the Auto ammo rule then picks HE). A tank with no HE left uses AP at the crew point. Infantry AT teams skip guns unless ordered (`ai` target rule).
Crew and gun parts take the burst by the ordinary rules.

## 9. Recrew, abandon, capture

- `Recrew` is reused as the "serve this gun" command: any squad of the rules' `any_infantry` allowed set (not AT teams) may serve an own gun with an empty crew, or a neutral gun. Men become serving soldiers; the squad stays a squad.
- Abandon rule: a gun with no man of its owner at a slot and none within `hold_m` (15) for `abandon_s` (10) turns `EntityKind::Abandoned` (neutral). It keeps its id, state, parts and rounds. `abandon_cause` AB_GUN (new). Skipped while Hitched (the tow's owner holds it).
- Capture: `man_vehicle`'s branch for a gun sets kind Vehicle, new squad of one, `owner = squad owner`, resets `seen_by`, clears targets, tow and hold-fire. `crew.captured` counter (and `s9.guns_captured`).
- A bought crew killed: the rule above turns the gun neutral; the crew squad is empty and removed.
- **Ownership plan.** Owner changes for the gun only (soldiers and their squad keep theirs). Touch points: `ents.owner` with `mark_entity_chunk`; `seen_by`; `forget()` drops targets; AI retains key on owner and squad id; `population` counts gun pop only while kind is Vehicle; hash is the entity column, tested; snapshot owner is shown from the column, neutral as NEUTRAL. A captured gun's kills count for the new owner; the JSON per-type map lists own roster only, and `s9_derived.captured_gun_kills_per_match` reports the rest.
- **Rounds for a captured gun.** Serving uses what's at the gun. New supply rule in `source_for`, for served guns only: a source fits only if its def faction equals the gun's def faction. Own crates of the other faction, own trucks and own HQ do not fit. Base refill: the same check.
  **Captured crates:** a squad may `PickUpCrate` a set-down enemy crate (no carrier, no claim). The crate's owner flips to the player; its def keeps its faction; validation (`structures.rs:424`) accepts that for crates; own-faction weapons cannot draw from it (`source_for` compares the crate's def faction with the consumer's def faction, for men, vehicles and guns).

## 10. Gun pit

`DigProfile::GunPit = 5`, `gun_pit` in `[dig.profiles]`: `all_round`, `pit_length_cm` 500, `depth_cm` 55, `parapet_cm` 40, `work` engineers only (`engineers_only`). Design depth is 60-80 cm with a ramp (3.10); 55 cm stays under `wheel_block_dm` 6, so wheels are not blocked and no ramp is needed (S9-9). Shield top stays above the parapet.
Dig it as a short line centred on the gun. A gun settles (`vehicle::settle`) when the ground under it changes. Crew at slots have `in_trench` (cover rules) from the pit.

## 11. Content (figures from guns-artillery.md 5.2, 6; missing figures marked `verify` in the data)

Defs in `mods/ww2/defs`: `vehicles/ger_pak40.toml` etc., `units/*_crew.toml`, `units/*_squad.toml`, ammo, weapons.

| Item | Crew | Tow | Cost, cooldown | Sources |
|---|---|---|---|---|
| Pak 40 (`ger_pak40`, `ger_pak40_squad`) | 6 | Sd.Kfz. 11 | 380 + tow (560), 90 s | 5.2; pen `75mm_pzgr39` |
| Pak 38 (`ger_pak38`) | 5 | none (pushable) | 260, 60 s | 5.2, 6 (PzGr 39, PzGr 40, Sprgr 38 new) |
| 57 mm M1 (`usa_57mm_m1`) | 6 | M3A1 (S9-3) | 340 + M3A1 (650), 90 s | 5.2, 6 (`57mm_m86` new). AP only: M303 HE is late 1944 (verify) |
| Sd.Kfz. 11 (`ger_sdkfz11`, tractor) | 2 | - | 180, 45 s, pop 2, cap 2, unlock 0:00 | Roster row; `[tow]`, `[transport]` capacity 8 |
| 3-inch M5 (`usa_3in_m5`) | 8 | - | data only, `push_cm_s = 0`, emplaced | 5.2, 6 (`3in_m62`, `3in_m42a1` exist). No roster card |

`VehicleRole::{AtGun, Tractor}` (both outside the limit); M3A1, CCKW, Opel Blitz get `[tow]`. Cards at `ai_weight = 0` until Build C; `max_alive = 2` for gun cards (S9-13). Ammo `rare_cap` 2 for PzGr 40. German 20 % cut not applied to gun cards (S9-4); the Tune moves prices.

## 12. AI

`[ai.guns]`: `after_s` 120, `income_max_pct` 12, `max_guns` 2, `site_ring_m` 100-250, `unhitch_m`, `self_defence_m`. A gun card buys on its own budget (`mix_slot` excludes `AtGun` and `Tractor`, like S8b's recon and truck buys through `buy_role`), never a tank's slot.
One saved job `AiGun` per gun, keyed by squad id, phases:
- **Site.** Threat: the most threatened own or contested point, from the overwatch anchor (`ai/overwatch.rs`). Candidates on a ring 100-250 m behind it. Score: covers the vehicle approach inside the arc (arc centre within `arc_tolerance` of the threat line), woods edge or hull-down line, not within 80 m of known enemy men. Records `in_cover` (woods, hull-down or a pit).
- **Go.** Tow drives to the site, crew rides; **Unhitch**, crew dismounts; **Emplace** facing the threat line. Tow returns to the base and its card. A Pak 38 pushes (slow).
- **Hold.** `SetAmbush` to `engage_still_m` 400 (data). The gun fires by the usual targeting.
- **Pit.** If an idle engineer squad is within 150 m and a pit job is free, order a `GunPit` dig (report `gun_pits_dug`).
- **Recrew.** Idle infantry within 100 m of a neutral gun with rounds serve it (`Recrew`).
Displace (S9-6): not in this slice beyond re-site when a Hold gun is overrun.
Counters: `AiCounters::s9` for `guns_emplaced`, `guns_in_cover`, `gun_ai_fails`. The buy lives beside `buy_recon_truck`.

## 13. Presenter

`godot/match/vehicles.gd`: gun blockout (carriage, shield, barrel, trails), `Vehicles.COMPONENTS` with `Sight`, `Recoil`. `vehicle_view.gd`: gun states (emplaced/limbered/hitched), tow bar, neutral grey. HUD (`hud/`): gun card with state, rounds, buttons Emplace (E), Pack, Hitch (pick a tow), Unhitch, Ambush range; crew squad card keeps the infantry controls and a "Serve" right-click on a neutral gun. `match_input.gd`: Hitch and Push targeting. `crates/godot-bridge`: snapshot fields, six intents. `godot/docs/match_state.md` "S9". Godot checks in the Build C check run; do not change `settings.cfg`.

## 14. Save and protocol

- SAVE 20: `VehicleState::{served, towing}`, `Unit::serving`, `Order` new variants, `PlayerState::s9`, `AiState::guns`, `Crate.owner` semantics, COMPONENTS 12.
- PROTOCOL 12: new commands at the end of `CommandKind`: `Emplace { guns, facing }`, `PackGun { guns }`, `PushGun { guns, point }`, `Hitch { guns, tow }`, `Unhitch { guns }`, `SetAmbush { guns, range_m }`.
- `sdk/docs/net/relay-protocol.md` version note. One bump each for the slice.
- `World::validate` (load): every `Served` only on a served def; slot men alive and in the crew squad of the same owner; `hitched`/`towing` mutual and alive; `Abandoned` guns have no tow; `serving` matches a slot; crate owner in range (a captured crate may carry an enemy-faction def). A per-field save/load/hash test per new field (the `world.rs:4256` pattern).

## 15. Tests

- Content: def resolves; `crew_model`, service slots, validator (slot outside boxes, tow `max_kg`, `push_men`, slopes), BASELINE refresh.
- Sim units: slot mirror; auto-swap; pinned gunner; arc and re-lay; hold fire; optic in/out; emplace/pack/push/hitch state changes; tow death; abandon timer; capture (owner, hash mark, seen_by); own-crate refusal; captured crate draw; pit under wheels.
- Golden and determinism: 1/2/3/8 threads with save/load during emplace, push, hitch, re-lay, capture, neutral; net loopback with the commands.
- `armour-test` (`armour_test/s9.rs`), 5 seeds × at least 8 repeats where it rolls: case 33 shield (T73), 34 ambush Pak 40 v Sherman (T74), 35 57 mm v Pz IV, 36 tank v gun (T75), 37 optic (T76), 38 recrew and capture (T77), 39 mobility (T78), 40 pit (T79).
- Done-when tests all named above. A seed-dependent failure scans seeds, it does not relax the assertion.

## 16. Targets

`docs/slices/targets/S9.toml`: every S8b target and band kept, T33a/b on `tank_td_mp_pct`, T73-T87 new. Done-when: T73a/b, T74a/b, T77, T80, T81. New JSON: `s7_derived.tank_td_mp_pct` (done), `s9`, `s9_derived` per faction, armour cases 33-40.

## 17. Build order

**Build A (entity, shield, fire).** Content schema, `crew_model`, `[service]`, towed `mobility`, roles, components; gun entity spawned emplaced at base; crew slots and mirror; shield and carriage boxes; casemate arc; optic and `SetAmbush` (or hold-fire rule data) ; Pak 40, 57 mm and M3A1/CCKW `[tow]` data, `ai_weight = 0`; `gun-test` cases 33, 34, 35, 37; `s9` JSON stubs; T73, T74, T76 pass. Golden and saves stay green.

**Build B (mobility, tow, capture).** GunState and six commands; tow link and rigid bar; Sd.Kfz. 11, Pak 38, M5 data; gun pit; abandon/neutral; `Recrew` for guns; crate faction rule and captured crates; ownership validate and per-field tests; cases 38, 39, 40; `check.sh quick` and the determinism set; T77, T78, T79 pass.

**Build C (AI, HE, presenter, balance).** Tanks' HE at the crew; `[ai.guns]` and job; presenter; Godot checks; `ai_weight` on; batches and targets: T75, T80-T87, T5 pooled, T32, T33, T54; tick p99 on clean `timing40` (profile above 25 %); docs (log, `STATE.md`).

## 18. Left out (later slices)

Spiking; sound-ranging and counter-battery; camouflage nets; indirect fire, infantry guns and howitzers (S11); 8.8 cm Pak 43 (S13); plank bridges and rolling across trenches; tank overruns of guns; co-op `ShareUnit`; direct-control gun station; the full ammo-class system (S9 has one faction check); fragment rays against capsules (F0); HVAP and a separate barrel part; tow as an AI ammo carrier beyond the existing truck stops; the tow's trailer physics beyond the rigid bar; rally-to-gun for routed crews; captured-tank ammo rules.

## 19. Owner questions

S9-1 to S9-15 are in `docs/TODO.md` with (A) defaults. S9-1 (optic range), S9-2/S9-3 (tow and price) and S9-13 (cap and unlock) are the ones that change how the game plays.
