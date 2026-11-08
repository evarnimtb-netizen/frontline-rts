# Frontline RTS: state (read this first)

Main holds S1 to S8a (`fa5c50a`, pushed 2026-11-07). S8b (trucks, recon cars, wheels) is built and
reviewed on branch `s8b-trucks-recon`, not merged, not pushed. Read the sections you need, then the doc the
index names. Checked 2026-11-08.

## What the game is now

An original WW2 tactical RTS (Gates of Hell style): a deterministic Rust
sim (30 Hz, fixed-point, counter-based RNG, state hash) and a Godot 4.7.2
presenter. One map (Kessel Ridge, 1024 m, 5 capture points), two factions
(Germany, USA), skirmish vs AI or 1v1 human vs human online. Rust 1.99.0,
workspace in `Cargo.toml`.

| System | What it does | Code |
|---|---|---|
| Terrain, trenches | two-tier heightfield; digging, craters, parapets | `crates/sim/src/terrain.rs`, `dig.rs` |
| Movement, squads | A* on a 4 m grid, formations, soldier-level orders | `path.rs`, `squad.rs`, `unit.rs`, `orders.rs` |
| Perception | ray-budgeted spotting, smoke, recon optics (S8b) | `spotting.rs` |
| Infantry combat | physical bullets and fragments, suppression, cover | `combat.rs`, `projectile.rs` |
| Throwables, AT items | frag, smoke, satchel, Panzerfaust (single-shot) | `items.rs` |
| Vehicles | box list, turret or casemate mount, open tops, crew seats, components, pintle MGs, shields (S8a), wheeled drive and wheel layer, grenade screens (S8b) | `vehicle.rs`, `damage.rs`, `gunnery.rs`, `armour.rs` |
| Small arms vs thin plates (S8a) | weapon penetration tables, reach table, thin-plate fragment rule | `combat.rs`, `damage.rs`, `armour.rs` |
| Riding and transport (S8a) | passengers stay soldiers (`Unit::riding`), slot boxes are hit volumes, door and side exits | `transport.rs`, `command.rs`, `ai/transport.rs` |
| Supply trucks (S8b) | `role = "truck"`, `[supply]` stock, paid loading at base, one source function (HQ, dump, truck, crate), `DropCrates`, cargo that detonates | `supply.rs`, `structures.rs`, `ai/truck.rs` |
| Casemate and open top (S7) | arc-limited casemate gun, pivot-to-lay; open faces with no plate | `gunnery.rs`, `damage.rs`, `items.rs` |
| Armour physics | plates with sloped normals, penetration chance (the one ballistics function) | `armour.rs` |
| Bail-out, re-crew | abandoned tanks, crew escape, capture | `bail.rs` |
| Ammunition | counted rounds per man/gun/vehicle, priced base refill | `supply.rs` |
| HQ, dumps, crates (S6) | pits, stock, crates, forward bombs, tank rearm, cook-off | `structures.rs`, `ai/supply.rs` |
| Fire missions | mortars: modes direct/observed/map, observers, ranging, charges | `fire_mission.rs`, `indirect.rs` |
| Match rules | tickets, manpower, capture, call-in cards with cooldowns | `rules.rs`, `game.rs`, `command.rs` |
| AI commander | per-player controller: assault, armour, ammo, mortar, supply, transport, recon, truck habits | `ai.rs`, `ai/*.rs` |
| World, save, hash | tick loop, save/load, state hash, render snapshot | `world.rs`, `hash.rs`, `snapshot.rs` |
| Content | TOML defs, variants (`extends`, `[[members]]`), tuning tables | `sdk/crates/content/src/` |
| Mod `ww2` | 2 factions; cards per faction for tank, casemate TD, half-track, and on S8b recon car and truck; 10 vehicles (Pz IV H, M4A3 75, StuG III G, M10, SdKfz 251/1 D, M3A1, SdKfz 234/1, M8, Opel Blitz, GMC CCKW); `defs/supply/` | `mods/ww2/defs/` |
| Multiplayer | relay-closed lockstep over UDP, desync dump, reconnect, Embark/Disembark, DropCrates (S8b) | `crates/net/src/` |
| Godot bridge | GDExtension: match thread, intents, frames | `crates/godot-bridge/src/` |
| Presenter | match view, HUD, menus, net lobby, mock, blockouts (half-tracks, S8b cars and trucks) | `godot/match/`, `godot/ui/`, `godot/net/` |
| CLI | simulate, skirmish, armour-test, fortify-test, mortar-bench, targets, ballistics, validate, schema, desync-diff | `sdk/crates/cli/src/` |
| Tools | autotune, check.sh, bridge/export | `tools/` |

Not in the game yet: Jagdpanzer IV, 38(t), M18, AT guns (S9), on-map artillery, fortifications, wounds,
a third faction, a second map, radio, SetHatch or unbuttoned commanders, carried men on trucks, fire from a vehicle.

## Versions

`SAVE_VERSION` 18 and `PROTOCOL_VERSION` 10 on main (since S8a); on branch `s8b-trucks-recon`: 19 and 11.
`SAVE_VERSION` is in `crates/sim/src/world.rs`, `PROTOCOL_VERSION` in `crates/net/src/protocol.rs`. History:
S5b 14 and 8; S6 15 and 9; S7 17 and 9; S8a 18 and 10; S8b 19 (wheels, cargo, still_ticks, AI jobs) and 11
(DropCrates). A change to a save layout, the hash or a command bumps these in the same commit.
`sdk/docs/net/relay-protocol.md` lists the commands. Each TOML value change refreshes the BASELINE in
`sdk/crates/content/tests/resolved_defs.rs` and `crates/simbench/src/report.rs`.

## S8b (branch `s8b-trucks-recon`, not on main)

Log `docs/slices/S8b-log.md`, spec `S8b-trucks-recon.md`, targets `docs/slices/targets/S8b.toml`.
- Wheeled mobility: bicycle drive (no pivot, turning radius), a wheel block layer (a ditch of `ditch_block_dm`
  or more blocks wheels; a standard trench is a wall), `WheelsFront` and `WheelsRear` (a blast under a wheel
  damages it), `reverse_cm_s` (234/1 reverse 1000 cm/s).
- Recon optics: `[optics]` `stationary_sight_pct` after `settle_s` stopped, one multiplier through spotting.
- Grenade screens: a screened open face stops thrown items (they bounce off beside the hull); bullets pass.
- Cars: SdKfz 234/1 (2 cm KwK 38 + MG 42, open screened turret; 260 MP) and M8 Greyhound (37 mm M6 + coax + .50 pintle
  ring; 300 MP). Trucks: Opel Blitz (150 MP) and GMC CCKW (200 MP), `role = "truck"`, no weapons.
- Supply trucks: loads stock at base (price per kg, no distance surcharge), rearms vehicles within
  `vehicle_radius_m` and men within `radius_m` through the same source function as HQ and dump (one source at a
  time, fastest in reach, rates never stack). `DropCrates { vehicle, point, count, kind }` (PROTOCOL 11). Cargo that
  is penetrated or set afire detonates at 40 kg or more.
- AI: rides that pay (`Meet` phase, ride lock, shorter waits, unload on arrival); recon cars on the S7 overwatch with
  their own budget (12 % of income); trucks (`ai/truck.rs`: Load, Follow, Serve stranded tanks, Drop, Return; 9 %);
  tanks take the nearest stock including trucks. Presenter: wheels, cargo beds, Drop Crates (I), stock bars.

Balance (Fix build, 3 x 160 sets; `skirmish160` = seeds 1-80): Germany 41.9 / 47.5 / 50.0 %, pooled 46.5 % over 480
(in band 45-55). Top vehicle kills: Pz IV 17.1 %, USA 14.6 % (cap 18, T32 holds). Vehicle manpower 36.0 / 35.5 % (cap
35: T33a/b miss, S8b-26). Saved seconds per ride GER +2.0 / +4.5 / -2.8, USA +14.9 / +11.4 / +19.9 (ride pay: USA on
all three sets, Germany on two). Rider moved 1.21 / 1.17 per match, trips per bought 1.22 / 1.29 on 1-80.

Checks: `check.sh quick` 610 passed; release 644 passed, 0 failed. Tick p99: Build C clean 0.427 / 0.522 ms; post-Fix runs not clean (0.67 ms peak): open.

## S8a (on main)

Half-tracks (`docs/slices/S8a-halftracks.md`, log `S8a-log.md`, targets `S8a.toml`). SdKfz 251/1 D (capacity 10) and
M3A1 (capacity 12): cards 290 / 310 MP, pop 3, cap 2, unlock 2:00, outside the vehicle limit. Pintle MGs and shields;
riders stay soldiers in slot boxes, exit by the door or over the sides; AI transport jobs (`[ai.transport]`). Balance
(160 sets): Germany pooled 45.4 % over 480; T32 holds. Tick p99 0.493 / 0.625 ms (clean).

## S7 and S6b (merged into main)

S7: assault guns and TDs (StuG III G, M10); vehicle schema with armour boxes, `open` faces, `[turret]`, mounts, seats
(`S7-casemate-td.md`). S6 and S6b: HQ, dumps and crates; mortar AI pass (S6b pooled Germany 47.0 % over 640).

## Owner decisions in force

Source: `docs/TODO.md` "Owner decisions in force" (full text in the archive).

- Our own design, not a recreation. Damage only from physical projectile or fragment contact; cost-balanced realism;
  all tuning values in TOML. Trenches stay strong vs small arms (S4-43).
- Call-in has no queue. No free ammo; distance costs only physical delivery. HQ and dumps fill only by carried crates.
  Tanks rearm at base, HQ, dump or truck. Historical squad sizes, pop 150.
- German cards keep the 20 % price cut (S5a-37). Mortars tuned through AI mission size, never lethality (S5b-17).
- Tank crews bail out. Vehicles before fortifications. Matches 10-40 min, median 15-30.

## Known gaps

- Balance: T5c (Germany 41.9 % on seeds 1-80, pool in band, S8a-30), T33a/b (S8b-26), T50e (S8b-27).
- Tick: Build C clean; the Fix runs were not clean, not profiled (S8b log, "Tick time").
- Godot: no run since the 11-seat M3A1; `Vehicles.COMPONENTS` may still list 7 parts (bridge sends 10).
- Net loopback sends `DropCrates` at a non-truck (refused there; covered by sim tests).
- Structures cannot be selected in the HUD (S6-18). AI rarely throws satchels or fires Panzerfausts. No radio,
  registered fire, cross-OS CI.

## Roadmap position

Done and on main: S1 to S7, S6b, infra, autotune fix, S8a half-tracks. S8b trucks, recon cars, wheeled mobility and
recon optics: built, reviewed and fixed on `s8b-trucks-recon`; next is the merge and then S9 AT guns, S10a/b
fortifications, S11a/b artillery, S12 wounds, S13 heavy wave, S14 assault kit, S15 USSR. Tiers:
`docs/process/workflows.md` section 1.

## How to check work (few lines of output)

- `tools/check.sh quick [BASE]` while working; `full` before a push. Targets:
  `frontline-cli targets docs/slices/targets/S8b.toml --jobs 6`. Ticks: `--jobs 3` only, from a clean machine.

## Index: for X read doc Y section Z

| For | Read |
|---|---|
| How a slice is built, tiers, models, review gate, runs | `docs/process/workflows.md` 1-5 |
| Open to-do, owner questions, work queue | `docs/TODO.md` |
| Old owner questions, finished to-do | `docs/archive/TODO-2026-11-03.md` (per-slice `##`) |
| Skirmish rules, AI commander, presenter data flow | `docs/slices/S1-skirmish.md` Gameplay, AI commander |
| Throwables; soldier selection and orders; HUD; theme | `S1b-abilities.md` Items; `S1b-soldier-control.md`; `S1b-hud.md`; `S1b-ui-theme.md` |
| Net protocol, turns, desync dump; playing online | `S2-multiplayer.md` Protocol, Turns, Hash check; `docs/net/playing-online.md` |
| Vehicle model, ballistics, gunnery, commands | `S3-armour.md` Vehicle model, Ballistics and penetration, Commands; log `S3-log.md` |
| Damage audit, fragments, trench pinning | `S3b-balance.md`; log `S3b-log.md` |
| Bail-out, abandoned tanks, re-crew | `S3c-armour.md`; log `S3c-log.md` |
| Variants: data model, content, Panzerfaust | `S4-variants.md`; log `S4-log.md` |
| Ammunition: data model, rules, base refill, AI | `S5a-ammo.md`; log `S5a-log.md` |
| Fire missions: modes, observers, charges, bench | `S5b-mortars.md`; `S5b-log.md` |
| Mortar AI: map fire limit, reserve, bearers | `S6b-mortar-ai.md`; `S6b-log.md` |
| HQ, dumps, crates | `S6-hq-crates.md`; `S6-log.md`; `structures.rs`, `ai/supply.rs`; `godot/docs/match_state.md` "S6" |
| TOML keys: ammo, fire missions, variants, `[supply]` | `docs/modding/ammo.md`; `fire-missions.md`; `variants.md` |
| Tuning overrides, A/B, `--json`, targets, check.sh; autotune | `docs/modding/tuning.md`; `docs/modding/autotune.md` |
| Roster plan, slice order, owner decisions | `docs/design/roster/roster-expansion.md` 1, 4, Appendix A |
| Guns, AT guns, penetration data | `docs/design/roster/guns-artillery.md` 3, 5, 6 |
| Vehicle catalogue, systems, ammo, build waves | `docs/design/roster/vehicles.md` 4, 6, 7, 9, 10 |
| S7 vehicle schema, casemate, open top, StuG/M10 | `docs/slices/S7-casemate-td.md`; log `S7-log.md`; `godot/docs/match_state.md` "S7" |
| S8a half-tracks: riding, small arms, pintles, transport AI | `docs/slices/S8a-halftracks.md`; log `S8a-log.md`; `godot/docs/match_state.md` "S8a" |
| S8b wheels, optics, screens, trucks, DropCrates, recon and truck AI | `docs/slices/S8b-trucks-recon.md` 5-9, 17; log `S8b-log.md`; `godot/docs/match_state.md` "S8b" |
| Architecture, determinism, modding, repo layout | `SYSTEM_DESIGN.md` 3, 5, 7, 13 |
