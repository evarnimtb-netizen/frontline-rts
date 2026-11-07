# Frontline RTS: state (read this first)

Where the project stands after S8a. Main holds everything through S8a (`fa5c50a`,
verified by the orchestrator and pushed 2026-11-07).
Read the sections you need, then the doc the index names. Checked 2026-11-07.

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
| Perception | ray-budgeted spotting, smoke | `spotting.rs` |
| Infantry combat | physical bullets and fragments, suppression, cover | `combat.rs`, `projectile.rs` |
| Throwables, AT items | frag, smoke, satchel, Panzerfaust (single-shot) | `items.rs` |
| Vehicles | box list, turret or casemate mount, open tops, crew seats, components, pintle MGs and shields (S8a) | `vehicle.rs`, `damage.rs`, `gunnery.rs`, `armour.rs` |
| Small arms vs thin plates (S8a) | weapon penetration tables, reach table, one-comparison target check; thin-plate fragment rule | `combat.rs`, `damage.rs`, `armour.rs` |
| Riding and transport (S8a) | passengers stay soldiers (`Unit::riding`), slot boxes are hit volumes, door and side exits, forced dismount | `transport.rs`, `command.rs`, `ai/transport.rs` |
| Casemate and open top (S7) | arc-limited casemate gun, pivot-to-lay; open faces with no plate | `gunnery.rs`, `damage.rs`, `items.rs` |
| Armour physics | plates with sloped normals, penetration chance (the one ballistics function) | `armour.rs` |
| Bail-out, re-crew | abandoned tanks, crew escape, capture | `bail.rs` |
| Ammunition | counted rounds per man/gun/vehicle, priced base refill | `supply.rs` |
| HQ, dumps, crates (S6) | pits, stock, crates, forward bombs, tank rearm, cook-off | `structures.rs`, `ai/supply.rs` |
| Fire missions | mortars: modes direct/observed/map, observers, ranging, charges | `fire_mission.rs`, `indirect.rs` |
| Match rules | tickets, manpower, capture, call-in cards with cooldowns | `rules.rs`, `game.rs`, `command.rs` |
| AI commander | per-player controller: assault, armour, ammo, mortar, supply, transport habits | `ai.rs`, `ai/*.rs` |
| World, save, hash | tick loop, save/load, state hash, render snapshot | `world.rs`, `hash.rs`, `snapshot.rs` |
| Content | TOML defs, variants (`extends`, `[[members]]`), tuning tables | `sdk/crates/content/src/` |
| Mod `ww2` | 2 factions; cards incl. S6 HQ/bearers and the vehicle cards (tank, casemate TD, half-track) per faction; 6 vehicles (Pz IV H, M4A3 75, StuG III G, M10, SdKfz 251/1 D, M3A1), about 22 weapons; `defs/supply/` | `mods/ww2/defs/` |
| Multiplayer | relay-closed lockstep over UDP, desync dump, reconnect, Embark/Disembark | `crates/net/src/` |
| Godot bridge | GDExtension: match thread, intents, frames | `crates/godot-bridge/src/` |
| Presenter | match view, HUD, menus, net lobby, mock, half-track blockouts | `godot/match/`, `godot/ui/`, `godot/net/` |
| CLI | simulate, skirmish, armour-test, fortify-test, mortar-bench, targets, ballistics, validate, schema, desync-diff | `sdk/crates/cli/src/` |
| Tools | autotune, check.sh, bridge/export | `tools/` |

Not in the game yet: trucks, recon cars, wheeled mobility, supply trucks (S8b);
Jagdpanzer IV, 38(t), M18, AT guns, on-map artillery, fortifications, wounds,
a third faction, a second map, radio, tank riders, fire from a vehicle.

## Versions

`SAVE_VERSION` 18 and `PROTOCOL_VERSION` 10 on main (since S8a).
`SAVE_VERSION` is in `crates/sim/src/world.rs`, `PROTOCOL_VERSION` in
`crates/net/src/protocol.rs`. History: S5b 14 and 8; S6 15 and 9; S7 17 and 9;
S8a 18 (`MgState.yaw`, riding fields) and 10 (Embark, Disembark). A change to a
save layout, the hash or a command bumps these in the same commit.
`sdk/docs/net/relay-protocol.md` lists the commands.

## S8a (on main)

Half-tracks (log `docs/slices/S8a-log.md`, spec `S8a-halftracks.md`, targets
`S8a.toml`). SdKfz 251/1 D (capacity 10) and M3A1 (capacity 11, M2HB): cards
290 / 310 MP, pop 3, cap 2, 60 s, unlock 2:00, outside the vehicle limit.
Pintle MGs with arcs and shields; small arms penetrate thin plates into boxes
(`penetration` tables, `core`); riders stay soldiers in slot boxes, exit by the
door or over the sides (faster), are forced out on wreck, abandon or fire;
AI transport jobs Load, Ride, Unload, Support, Return (`[ai.transport]`).
Presenter: blockouts, crouched riders, Board V, Unload U, Shift+U.

Balance (160 sets, seeds 1-80 / 101-180 / 201-280): Germany 42.5 / 45.6 / 48.1 %,
pooled 45.4 % over 480 (in band); HE 21.0 / 20.3 / 20.2 %; T32 holds (17.9 %, cap 18). Half-tracks bought in 87-94 % of matches; 251 takes 0.8 %
of German kills, M3A1 3.8 % of US kills. AI riding is slower than walking
(S8a-28): Germany -29 / -26 / -21 s per ride, USA -14 / -15 / -6 s.
Targets: 119 pass, 7 miss (hard: T5c, T41b, T48b), 4 skipped. Tick p99 0.493 ms
all, 0.625 ms peak (clean run; S7 0.38 / 0.56), unprofiled.

## S7 (merged into main)

Assault guns and tank destroyers (`docs/slices/S7-log.md`, `S7-casemate-td.md`).
Vehicle schema: 1..=4 armour boxes, `open` faces, optional `[turret]`, mounts
`turret_main` / `casemate` / `coax` / `hull`, seats as a list. StuG III G (500 MP,
pop 10, 120 s) and M10 (520 MP, pop 10, 120 s). AI overwatch 35 m behind own points.
Balance at 35 m: Germany 48.1 / 44.4 / 48.1 % (pooled 46.9 % over 480).

## S6 and S6b (merged into main)

HQ, dumps and crates; mortar AI pass (S6b pooled Germany 47.0 % over 640).

## Owner decisions in force

Source: `docs/TODO.md` "Owner decisions in force" (full text in the archive).

- Our own design, not a recreation. Damage only from physical projectile or
  fragment contact; suppression only from real near-misses.
- Cost-balanced realism: add historical options before changing stats. All
  tuning values live in TOML. Trenches stay strong vs small arms (S4-43).
- Call-in has no queue (buy if affordable, spawn at base, per-card cooldown).
- No free ammo anywhere; distance costs only physical delivery. HQ and dumps
  fill only by carried crates. Tanks rearm at base, HQ or dump. Historical squad
  sizes, pop 150.
- German cards keep the 20 % price cut (S5a-37). Mortars tuned through AI mission
  size, bomb load and map fire, never lethality: 10-15 % of kills, HE share
  20-25 % (S5b-17).
- Tank crews bail out; abandoned tanks can be re-crewed or captured. Artillery:
  on-map crewed units. Vehicles before fortifications. Matches 10-40 min, median 15-30.

## Known gaps

- Balance: T5c (Germany 42.5 % on 1-80, pooled in band), T41b / T48b; S8a-29/30.
- Half-tracks: riding costs time for the AI (S8a-28); "MG dry" shows on the 251
  card while the sim fires (bridge or card bug, not fixed).
- Tick p99 about 30 % above S7 after S8a; not profiled.
- Structures cannot be selected in the HUD (S6-18). AI rarely throws satchels or
  fires Panzerfausts. No radio, registered fire, cross-OS CI.

## Roadmap position

Done and on main: S1 to S7, S6b, infra, autotune fix, S8a half-tracks. Next: S8b trucks, recon cars, wheeled
mobility, recon optics, supply trucks. Then S9 AT guns, S10a/b fortifications,
S11a/b artillery, S12 wounds, S13 heavy wave, S14 assault kit, S15 USSR. Tiers:
`docs/process/workflows.md` section 1.

## How to check work (few lines of output)

- `tools/check.sh quick [BASE]` while working; `full` before a push. Targets:
  `frontline-cli targets docs/slices/targets/S8a.toml --jobs 6`. Ticks: `--jobs 3` only.

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
| S8a half-tracks: riding, small arms, pintles, transport AI, measures | `docs/slices/S8a-halftracks.md`; log `S8a-log.md`; `godot/docs/match_state.md` "S8a" |
| Architecture, determinism, modding, repo layout | `SYSTEM_DESIGN.md` 3, 5, 7, 13 |
