# Frontline RTS: state (read this first)

Main holds S1 to S10b mechanics (main `72b2970`). **Work now: S10b-ai** (the AI uses mines, obstacles, the bunker,
sweeping, demolition; paths avoid known enemy mines; job liveness) on branch `s10b-ai` from main `72b2970`: spec
`docs/slices/S10b-ai.md`, stages `S10b-ai-build-A.md`..`-D.md`, log `S10b-ai-log.md` (read its top block), targets
`S10b-ai.toml`. Plan done 2026-11-12; next Build A. Owner questions S10b-5..9 and S10b-ai-1..7 in `docs/TODO.md`.

## What the game is now

An original WW2 tactical RTS (Gates of Hell style): a deterministic Rust sim (30 Hz, fixed-point, counter-based RNG,
state hash) and a Godot 4.7.2 presenter. One map (Kessel Ridge, 1024 m, 5 capture points), two factions (Germany, USA),
skirmish vs AI or 1v1 human vs human online. Rust 1.99.0, workspace in `Cargo.toml`.

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
| Multiplayer | relay-closed lockstep over UDP, desync dump, reconnect, Embark/Disembark, DropCrates, gun commands, ClearMines, Demolish | `crates/net/src/` |
| Mines, obstacles, bunker (S10b) | AP and AT minefields, hedgehog and log barriers, bunker pieces, sweep, demolish; AI use in S10b-ai | `sim/src/sweep.rs`, `demolish.rs`, `fort.rs`; `ai/fort.rs` `fort_of` guard |
| Godot bridge and presenter | GDExtension (match thread, intents, frames); match view, HUD, menus, net lobby, mock, blockouts | `crates/godot-bridge/src/`, `godot/match/`, `godot/ui/`, `godot/net/` |
| CLI and tools | simulate, skirmish, armour-test, fortify-test, mortar-bench, targets, ballistics, validate, schema, desync-diff; autotune, check.sh | `sdk/crates/cli/src/`, `tools/` |

Not in the game yet: Jagdpanzer IV, 38(t), M18, on-map artillery, wounds, a third faction, a second map, radio,
SetHatch or unbuttoned commanders, carried men on trucks, fire from a vehicle, a 3-inch M5 in the AI, HE for the 57 mm.

## Versions

`SAVE_VERSION` **24** and `PROTOCOL_VERSION` **14** on main (`world.rs`, `protocol.rs`); S10b-ai Build A bumps SAVE to 25
(PROTOCOL stays). History: S8b 19/11; S9 20/12; S9b 21/12; S10a 22/13; S10a-ai 23/13; S10b 24/14. A save, hash or command change bumps these
in the same commit. Relay commands: `sdk/docs/net/relay-protocol.md`. Each TOML value change refreshes the BASELINE in
`sdk/crates/content/tests/resolved_defs.rs` and `crates/simbench/src/report.rs`.

## S10a fortifications, mechanics (on main since 2026-11-11; log `S10a-log.md`)

Sim `crates/sim/src/fort.rs`; blueprints `mods/ww2/defs/fortifications/`; commands Fortify, LoadMaterials, CutWire,
PlaceBangalore; capabilities build_structure, build_heavy, place_wire, clear_wire, breach. Cards ger_sturmpioniergruppe
264 MP, ger_baupioniergruppe 165 MP, usa_assault_section 380 MP, `ai_weight` 0. Guide `docs/modding/fortifications.md`.

## S10a-ai fortification AI and balance (on main since 2026-11-11; log `S10a-ai-log.md`)

Verified on `72fd940` + `a0aa700` (`target/s10a-ai-verify`, the S10b neutrality base): 802 tests pass; tick p99
0.476 / 0.574 ms. Germany pooled **41.67 %** of 480 (43.75 / 38.75 / 42.50; before: 48.75 %), inside the 35-65 % guard band.
- Fort use (160 sets): Germany works 94-97 %, wire 78-82 %; USA works 79-82 %, wire 56-58 % (T95b/d floor 60 missed).
- Hard misses on the base JSON (read by `S10b.toml`): T8b, T40c, T54e, T88b, T88d; T93f/h, T96a-d reports until S11.
- Breaching: a lone squad cannot breach defended wire without covering fire (throws need 25-29 m; squads fall 69-95 m
  out or are pinned 27-38 m out): S11 combined-arms work. Ablations: each piece type moves Germany under 2 points.
- **Open: F8** (a truck re-orders an unreachable HQ goal after four give-ups): S10b-ai L5, behind `refusal_memory`.

## S10b mechanics (on main since 2026-11-12; log `S10b-log.md`)

Mines (`ap_minefield`, `at_minefield`), hedgehog and log barrier (block vehicle nodes), bunker (six pieces, slit),
`ClearMines` (sweep) and `Demolish` (placed satchel). Blueprints in `mods/ww2/defs/fortifications/`; items `ap_mine`, `at_mine`.
Verify: 881 tests pass; tick p99 0.487 ms (peak 0.638); neutrality base `target/s10b-verify` (40-only `target/s10b-base40`). Germany pooled **41.67 %** of 480 (43.75 / 38.75 / 42.50), 0 no-winner, in the 35-65 % band.
AI-vs-AI identical to main (`neutral.py` vs `target/s10a-ai-verify`). Targets: 373 pass, 25 miss, 6 hard.
Hard misses: T8b, T40c, T54e, T88b, T88d (old) and **T106b** (a tank does not pass one demolished piece in a hedgehog line; owner decision).
Owner questions S10b-5..9 are in `docs/TODO.md`. Fuzz: 9686 mutations, 0 panics, 0 unloadable.

## S10b-ai (in work, branch `s10b-ai`; plan 2026-11-12)

Switches `[ai.obstacles]`, `[fortify] mine_paths`, `[ai.truck] refusal_memory`; all off = main match for match. New
targets T109-T118 have value, exposure and control (`UNEXPOSED`, `CONTROL-SAME` verdicts from Build A).

## Process and owner rules (decided 2026-11-11; also in `docs/TODO.md`)

1. Features first, balance later: no per-slice Tune; each slice measures and records 480 matches. Germany's pooled share
   must stay in a 35-65 % guard band, else stop and diagnose. Fine balance (45-55 %) and soft per-unit targets wait for a
   dedicated balance pass (after S10b-ai or S11).
2. Minor scope and price questions are settled on the recommended (A) and only reported. Irreversible choices, how-it-should-play choices and main updates are still asked.
3. S10a-ai decisions: breaching "fix smoke, lower bar" (T93f/h floor 30, T93g report); AI wire in 60 m lines; cards Germany by weight (Bau 100, Sturm 60, Pioniere 40), USA floor a report; smoke on three men.

## Owner decisions in force (source: `docs/TODO.md`)

- Our own design; damage only from projectile or fragment contact; all tuning in TOML; trenches strong vs small arms (S4-43).
- Call-in has no queue; no free ammo; HQ and dumps fill only by carried crates; tanks rearm at base, HQ, dump or truck.
  Historical squad sizes, pop 150. German cards keep the 20 % price cut (S5a-37), except gun cards (S9-4).
- Tank crews bail out. Vehicles before fortifications. Matches 10-40 min, median 15-30.
- S9 (2026-11-08): vehicle spotting 2x; AT gun optic 250 % inside the arc; T33 counts tanks and TDs only.
- S9b (2026-11-10): S9b-1 (A) keep `income_max_pct` 30, the US T88 shortfall goes to the S10 AI half; S9b-2, S9b-3 (A).

## Known gaps

- Balance: S10a-ai Germany 41.67 % (480); S10a 48.75 %; T88 USA open (S9b-1 (A)). T5c, T50e known noise.
- Not verified: the eliminated-player path; a per-field hash audit beyond the named tests. No radio, registered fire, cross-OS CI.
- Net loopback refuses `DropCrates` at a non-truck. Structures cannot be selected in the HUD (S6-18). AI rarely throws satchels.

## Roadmap position

Done and on main: S1 to S10b mechanics. In work: S10b-ai (branch `s10b-ai`; Build A, B, C, D, measure and
ablations, Review, Fix, verify, Docs; no Tune). Then a balance pass, S11a/b, S12 to S15. Tiers: `workflows.md` 1.

## How to check work

- `tools/check.sh quick [BASE]` while working; `tools/check.sh tests` runs the whole release suite with ignored tests
  (2-6 min, background it). `tools/verify.sh` is the orchestrator's full check before a push.
- Balance: `tools/measure.sh OUTDIR docs/slices/targets/S10b-ai.toml` (`S10b.toml` until S10b-ai Build A chunk 2) (480 matches, 14 min uncached); `python3 -I tools/pool.py DIR`.
  Quick look (40 matches, noise 11-15 points): `tools/batch.sh OUT.json -- skirmish --ai-vs-ai --seed 1 --seeds 20 --both-bases --summary --minutes 45`.
- Targets: `frontline-cli targets docs/slices/targets/S10b.toml --jobs 6`. Ablations: `tools/ablate.sh`. Gun funnel:
  `tools/gun_trace.sh DIR [SEEDS]`. Ticks: `--jobs 3` only, from a clean machine.

## Index: for X read doc Y section Z

| For | Read |
|---|---|
| How a slice is built, tiers, models, review gate, runs | `docs/process/workflows.md` 1-5 |
| Open to-do, owner questions, work queue; old ones in `docs/archive/TODO-2026-11-03.md` | `docs/TODO.md` |
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
| S9 guns (entity, slots, tow, capture, pit, AI, balance); S9b AI siting, corridor, budget, open T88 | `docs/slices/S9-at-guns.md` 1-22, `S9b-ai-guns.md` 3.2, 6; logs `S9-log.md`, `S9b-log.md` |
| S10a fortifications (mechanics): pieces, materials, wire, breaching, engineer split, neutrality | `docs/slices/S10a-fortifications.md`; log `S10a-log.md` |
| S10a-ai: AI buys, materials, building, breaching, ablations, T93 | `docs/slices/S10a-ai.md` 3-9; log `S10a-ai-log.md` |
| S10b: mines, obstacles, bunker, demolition; rules R1-R30; stage files | `docs/slices/S10b-obstacles.md`; `S10b-build-X.md`; log `S10b-log.md` |
| S10b-ai: AI laying, sweeping, demolition, mine paths, liveness L1-L5, three-part targets | `docs/slices/S10b-ai.md`; `S10b-ai-build-X.md`; log `S10b-ai-log.md` |
| Development speed, audits 2026-11-11 and 2026-11-12 (row honesty, stop points) | `docs/process/dev-speed.md` 7, 8 |
| Architecture, determinism, modding, repo layout | `SYSTEM_DESIGN.md` 3, 5, 7, 13 |
