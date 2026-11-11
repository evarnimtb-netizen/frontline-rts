# Frontline RTS: state (read this first)

Main holds S1 to S9b (`ba0038f`; S9b merged 2026-11-10 with the owner's OK, verified on `1bfc92f`). S10a mechanics
is built, reviewed, fixed and verified on branch `s10a-fortifications` (head `6d7b98c`); not merged, main waits for the owner's OK. Spec `docs/slices/S10a-fortifications.md`.
Read the sections you need, then the doc the index names. Checked 2026-11-11.

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
| Perception | ray-budgeted spotting, smoke, recon optics (S8b), gun optic (S9) | `spotting.rs` |
| Infantry combat | physical bullets and fragments, suppression, cover | `combat.rs`, `projectile.rs` |
| Throwables, AT items | frag, smoke, satchel, Panzerfaust (single-shot) | `items.rs` |
| Vehicles | box list, turret or casemate mount, open tops, crew seats, components, pintle MGs, shields, wheeled drive and wheel layer, grenade screens | `vehicle.rs`, `damage.rs`, `gunnery.rs`, `armour.rs` |
| AT guns (S9) | served guns: crew in service slots, shield and trail boxes, emplace/pack/push/hitch/tow, neutral and capture, gun pit, optic in the arc, HE at the crew | `guns.rs`, `ai/guns.rs`, `dig.rs` |
| Small arms vs thin plates | weapon penetration tables, reach table, thin-plate fragment rule | `combat.rs`, `damage.rs`, `armour.rs` |
| Riding and transport | passengers stay soldiers (`Unit::riding`), slot boxes are hit volumes, door and side exits | `transport.rs`, `command.rs`, `ai/transport.rs` |
| Supply trucks (S8b) | `role = "truck"`, `[supply]` stock, paid loading at base, one source function, `DropCrates`, cargo that detonates | `supply.rs`, `structures.rs`, `ai/truck.rs` |
| Casemate and open top (S7) | arc-limited casemate gun, pivot-to-lay; open faces with no plate | `gunnery.rs`, `damage.rs`, `items.rs` |
| Armour physics | plates with sloped normals, penetration chance (the one ballistics function) | `armour.rs` |
| Bail-out, re-crew, capture | abandoned tanks, crew escape, capture (tanks S3c, guns S9) | `bail.rs` |
| Ammunition | counted rounds per man/gun/vehicle, priced base refill; guns draw only their faction's rounds (S9) | `supply.rs` |
| HQ, dumps, crates (S6) | pits, stock, crates, forward bombs, tank rearm, cook-off | `structures.rs`, `ai/supply.rs` |
| Fire missions | mortars: modes direct/observed/map, observers, ranging, charges | `fire_mission.rs`, `indirect.rs` |
| Match rules | tickets, manpower, capture, call-in cards with cooldowns | `rules.rs`, `game.rs`, `command.rs` |
| AI commander | per-player controller: assault, armour, ammo, mortar, supply, transport, recon, truck, gun habits | `ai.rs`, `ai/*.rs` |
| World, save, hash | tick loop, save/load, state hash, render snapshot | `world.rs`, `hash.rs`, `snapshot.rs` |
| Content | TOML defs, variants (`extends`, `[[members]]`), tuning tables | `sdk/crates/content/src/` |
| Mod `ww2` | 2 factions; cards for tank, casemate TD, half-track, recon car, truck, AT gun; 15 vehicles incl. 4 guns and the Sd.Kfz. 11; `defs/supply/` | `mods/ww2/defs/` |
| Multiplayer | relay-closed lockstep over UDP, desync dump, reconnect, Embark/Disembark, DropCrates, gun commands | `crates/net/src/` |
| Godot bridge | GDExtension: match thread, intents, frames | `crates/godot-bridge/src/` |
| Presenter | match view, HUD, menus, net lobby, mock, blockouts (half-tracks, cars, trucks, guns) | `godot/match/`, `godot/ui/`, `godot/net/` |
| CLI | simulate, skirmish, armour-test, fortify-test, mortar-bench, targets, ballistics, validate, schema, desync-diff | `sdk/crates/cli/src/` |
| Tools | autotune, check.sh, bridge/export | `tools/` |

Not in the game yet: Jagdpanzer IV, 38(t), M18, on-map artillery, fortification AI, wounds, a third faction,
a second map, radio, SetHatch or unbuttoned commanders, carried men on trucks, fire from a vehicle, a 3-inch M5
in the AI (data only, emplaced only), HE for the 57 mm (AP only).

## Versions

`SAVE_VERSION` **21** and `PROTOCOL_VERSION` **12** on main (S9b); **22** and **13** on `s10a-fortifications` (S10a, one bump each).
`SAVE_VERSION` is in `crates/sim/src/world.rs`, `PROTOCOL_VERSION` in `crates/net/src/protocol.rs`.
History: S5b 14 and 8; S6 15 and 9; S7 17 and 9; S8a 18 and 10; S8b 19 and 11; S9 20 and 12; S9b 21 and 12 (one bump each).
A change to a save layout, the hash or a command bumps these in the same commit. `sdk/docs/net/relay-protocol.md`
lists the commands. Each TOML value change refreshes the BASELINE in `sdk/crates/content/tests/resolved_defs.rs`
and `crates/simbench/src/report.rs`.

## S9 (on main since 2026-11-09; S9b siting and budget facts in the S9b section above)

AT guns with crews as soldiers. A gun is a `Vehicle` with `crew_model = "served"`; men sit in `[[service]]` slots
(`Serving { gun, slot }`) and mirror into `vs.seats` each tick. Shield boxes are thin `axle` and `trail` boxes (a
validator keeps slot capsules out of them). Parts `Gun`, `Sight`, `Recoil`. Commands `Emplace`, `PackGun`, `PushGun`,
`Hitch`, `Unhitch`, `SetAmbush`, `Recrew`. Tow bar (`tow_bar_cm`), tow pays with the card (Pak 40 560 MP, 57 mm 650 MP
with M3A1). Unmanned gun goes neutral after `abandon_s` (`EntityKind::Abandoned`); capture keeps the crew alive. Guns
draw only their faction's rounds. Optic in the arc: max(normal, 437 m). AI: `ai/guns.rs`, `[ai.guns]`, own budget
(`gun_pot` 35 %, `max_guns` 2), jobs Site, Go, Unhitch, Emplace, Hold, Pit, Recrew, and re-site of idle guns (Fix).
Tanks fire HE at the crew (`gunnery::gun_target`).

Cards (`ai_weight` 5 for Pak 40 and 57 mm; Pak 38 0): Pak 40, 57 mm M1 (with M3A1 tow), Pak 38 (second line,
limbered), 3-inch M5 (data), Sd.Kfz. 11 tractor (`[transport]`). Cards `max_alive` 2 from 0:00. Prices and tow
figures without a design source are marked verify.

Balance (after Fix, pooled 480): Germany **45.6 %**; 14 hard misses (S9-19). Tank KOs by guns 36 GER / 16 USA (S9-17).
Log `docs/slices/S9-log.md`, spec `S9-at-guns.md`.

## S9b (on main since 2026-11-10; log `S9b-log.md`, spec `S9b-ai-guns.md`)

AI gun siting along the armour corridor (`ai/guns.rs`). Pooled Germany **48.75 %** of 480; 10 hard misses; SAVE 21.

## S10a fortifications, mechanics (branch `s10a-fortifications`, not merged; log `S10a-log.md`)

Sim `crates/sim/src/fort.rs`; blueprints in `mods/ww2/defs/fortifications/`; commands Fortify, LoadMaterials, CutWire, PlaceBangalore; capabilities build_structure, build_heavy, place_wire, clear_wire, breach. Cards ger_sturmpioniergruppe 264 MP, ger_baupioniergruppe 165 MP, usa_assault_section 380 MP, all `ai_weight` 0. HUD build menu (key ` as a stand-in). Guide `docs/modding/fortifications.md`.

Verify 2026-11-11: 742 tests pass, 0 fail; tick p99 0.413 ms all / 0.518 ms peak; targets 282 pass, 12 miss, 10 hard (the S9b ten; T93a/b are the breach report misses). Neutrality: skirmish40, skirmish160, skirmish160_oos, skirmish160_s201 same; Germany 48.75 % of 480, unchanged.

Fortify rows (10 seeds): wire inside 20 %, through a cut or blown gap 100 %; roof survives 81 mm 100 %; breach: no gap in 20 runs (AI and staging work, S10b). Review: 17 findings (4 save-load blockers), fixed with tests; re-review 3 more, fixed.

## Owner decisions in force (source: `docs/TODO.md`)

- Our own design; damage only from projectile or fragment contact; all tuning in TOML; trenches strong vs small arms (S4-43).
- Call-in has no queue; no free ammo; HQ and dumps fill only by carried crates; tanks rearm at base, HQ, dump or truck.
  Historical squad sizes, pop 150. German cards keep the 20 % price cut (S5a-37), except gun cards (S9-4).
- Tank crews bail out. Vehicles before fortifications. Matches 10-40 min, median 15-30.
- S9 (2026-11-08): vehicle spotting 2x; AT gun optic 250 % inside the arc; T33 counts tanks and TDs only.
- S9b (2026-11-10): S9b-1 (A) keep `income_max_pct` 30, the US T88 shortfall goes to the S10 AI half; S9b-2 (A) the
  SAVE 21 bump stays; S9b-3 (A) the 10 hard misses carry to S10. Merge to main approved.
- S10 on: each engine slice is a mechanics slice (new cards `ai_weight` 0, AI-vs-AI proven unchanged), then an AI half.

## Known gaps

- Balance: S9 pooled 45.6 %, 14 hard misses (S9-19); S9b 48.75 % and 10 hard misses (`income_max_pct` 30). T5c, T50e known noise (S8a-30, S8b-27).
- S9b: no failing test on `1bfc92f`; the launcher save bug fix (`7e33e0e`) is on main. T88 for USA open (S9b-1 (A)).
- Not verified: the eliminated-player path; a per-field hash audit beyond the named tests.
- Net loopback refuses `DropCrates` at a non-truck (sim tests cover it). Structures cannot be selected in the HUD (S6-18).
  AI rarely throws satchels or fires Panzerfausts. No radio, registered fire, cross-OS CI.

## Roadmap position

Done and on main: S1 to S9b. S10a mechanics done on `s10a-fortifications` (not merged). Next: merge S10a on the owner's OK, then S10b (fortification AI and balance),
S11a/b, S12 to S15. Tiers: `docs/process/workflows.md` section 1.

## How to check work (few lines of output)

- `tools/check.sh quick [BASE]` while working; `tools/check.sh tests` runs the whole release suite with ignored tests (2-6 min,
  background it). `tools/verify.sh` is the orchestrator's full check of a slice branch before a push.
- Balance: `tools/measure.sh OUTDIR docs/slices/targets/S9b.toml` (480 matches, 14 min uncached); `python3 -I tools/pool.py DIR`.
  Quick look (40 matches, noise 11-15 points): `tools/batch.sh OUT.json -- skirmish --ai-vs-ai --seed 1 --seeds 20 --both-bases --summary --minutes 45`.
- Gun funnel: `tools/gun_trace.sh DIR [SEEDS]` (40 matches by default). Ticks: `--jobs 3` only, from a clean machine.
- Targets: `frontline-cli targets docs/slices/targets/S9b.toml --jobs 6`.

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
| HQ, dumps, crates | `S6-hq-crates.md`; `S6-log.md`; `structures.rs`, `ai/supply.rs` |
| TOML keys: ammo, fire missions, variants, `[supply]` | `docs/modding/ammo.md`; `fire-missions.md`; `variants.md` |
| Tuning overrides, A/B, `--json`, targets, check.sh; autotune | `docs/modding/tuning.md`; `docs/modding/autotune.md` |
| Roster plan, slice order, owner decisions | `docs/design/roster/roster-expansion.md` 1, 4, Appendix A |
| Guns, AT guns, penetration data | `docs/design/roster/guns-artillery.md` 3, 5, 6 |
| Vehicle catalogue, systems, ammo, build waves | `docs/design/roster/vehicles.md` 4, 6, 7, 9, 10 |
| S7 vehicle schema, casemate, open top, StuG/M10 | `docs/slices/S7-casemate-td.md`; log `S7-log.md`; `godot/docs/match_state.md` "S7" |
| S8a half-tracks: riding, small arms, pintles, transport AI | `docs/slices/S8a-halftracks.md`; log `S8a-log.md`; `godot/docs/match_state.md` "S8a" |
| S8b wheels, optics, screens, trucks, DropCrates, recon and truck AI | `docs/slices/S8b-trucks-recon.md` 5-9, 17; log `S8b-log.md` |
| S9 guns: entity, slots, tow, capture, pit, AI, balance | `docs/slices/S9-at-guns.md` (spec, sections 1-22); log `S9-log.md` |
| S9b AI gun siting (corridor), armour-out measure, budget, open T88 | `docs/slices/S9b-ai-guns.md` 3.2, 6; log `S9b-log.md` |
| S10a fortifications (mechanics): pieces, materials, wire, breaching, engineer split, neutrality check | `docs/slices/S10a-fortifications.md`; log `S10a-log.md` |
| Development speed: where S9 time went, limits for check and measure runs | `docs/process/dev-speed.md` |
| Architecture, determinism, modding, repo layout | `SYSTEM_DESIGN.md` 3, 5, 7, 13 |
