# Frontline RTS: state (read this first)

Main holds S1 to S10b-ai and balance pass B1 (merged 2026-11-16 with the owner's OK). S10c-ai (defences at the
front) is done on branch `claude/project-thread-e7nsak`, not on main. B1: **NO default changed** (see B1 below).
Owner questions are in `docs/TODO.md`.

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
| Mines, obstacles, bunker (S10b; S10b-ai on main; S10c-ai on branch) | AP and AT minefields, hedgehog and log barriers, bunker pieces, sweep, demolish; the AI lays, builds, sweeps, demolishes (`[ai.obstacles]`, 52 keys on the branch), paths avoid known enemy mines, liveness L1-L5, F8 (`[ai.truck] refusal_memory`). Build E: bunker assign and truck first, a 4 m row opens for a medium tank after two gaps, single-file lanes, AT fields in two rows. S10c-ai: hasty front fields, wire, AT fields and belts at the 2 front points, builder-card floor | `sim/src/sweep.rs`, `demolish.rs`, `fort.rs`; `ai/fort.rs`, `ai/sweep.rs`, `ai/demolish.rs`, `ai/truck.rs`; `ai.rs`; `sdk/crates/cli/src/fortify/s10c_ai.rs` |
| Godot bridge and presenter | GDExtension (match thread, intents, frames); match view, HUD, menus, net lobby, mock, blockouts | `crates/godot-bridge/src/`, `godot/match/`, `godot/ui/`, `godot/net/` |
| CLI and tools | simulate, skirmish, armour-test, fortify-test, mortar-bench, targets, ballistics, validate, schema, desync-diff; autotune, check.sh | `sdk/crates/cli/src/`, `tools/` |

Not in the game yet: Jagdpanzer IV, 38(t), M18, on-map artillery, wounds, a third faction, a second map, radio,
SetHatch or unbuttoned commanders, carried men on trucks, fire from a vehicle, a 3-inch M5 in the AI, HE for the 57 mm.

## Versions

`SAVE_VERSION` **25** and `PROTOCOL_VERSION` **14** on main (`world.rs`, `protocol.rs`); **27** and **14** on the branch `claude/project-thread-e7nsak` (S10c-ai 26: `AiCounters`, `AiState::front_lanes`, `FrontLane::at`; S11a1 27: kill class `u16`, `PlayerState::s11`, `AiState::{arty, fo, fo_lost_at}`, `AiBreach::prep_at`, two refusals, the S11a1 keys; no version-26 or -27 save exists outside the branch). History: S8b 19/11; S9 20/12; S9b 21/12; S10a 22/13; S10a-ai 23/13; S10b 24/14; S10b-ai 25/14; S10c-ai 26/14, S11a1 27/14 (branch). A save, hash or command change bumps these
in the same commit. Relay commands: `sdk/docs/net/relay-protocol.md`. Each TOML value change refreshes the BASELINE in
`sdk/crates/content/tests/resolved_defs.rs` and `crates/simbench/src/report.rs`.

## S10a fortifications, mechanics (on main since 2026-11-11; log `S10a-log.md`)

Sim `crates/sim/src/fort.rs`; blueprints `mods/ww2/defs/fortifications/`; commands Fortify, LoadMaterials, CutWire,
PlaceBangalore; capabilities build_structure, build_heavy, place_wire, clear_wire, breach. Cards 264 / 165 / 380 MP
(`ai_weight` 0). Guide `docs/modding/fortifications.md`.

## S10a-ai fortification AI (on main since 2026-11-11; log `S10a-ai-log.md`)

Germany pooled **41.67 %** of 480 at merge. USA wire 56-58 % (floor 60 missed). A lone squad cannot breach defended wire
without covering fire (S11). F8 fixed on `s10b-ai` (L5, `refusal_memory` 1).

## S10b mechanics (on main since 2026-11-12; log `S10b-log.md`)

Mines, hedgehog and log barriers, bunker, `ClearMines`, `Demolish` (blueprints `mods/ww2/defs/fortifications/`). 881 tests
at verify; tick p99 0.487 ms. Hard misses: T8b, T40c, T54e, T88b, T88d, **T106b** (owner decision). Questions S10b-5..9 in `docs/TODO.md`.

## S10b-ai (merged to main 2026-11-15; log `S10b-ai-log.md`)

Verify: 965 tests pass; Germany 41.88 % of 480; 410 targets pass, 46 miss, 26 hard. All switches off: 40 set = main 40/40.
AI use far under target (mines, obstacles, bunker, AT fields; owner E6). Open: finding 23 (`piece_near`), finding 12
(mine-want counter needs a save change), barrier row vs `barrier_job_max_s`, off-lattice crossing (finding 7).

## B1 balance pass (merged to main 2026-11-16, NO default changed; log `docs/slices/B1-log.md`)

Built before balancing (owner 2026-11-15): `[ai.obstacles]` `demolish_handover` and `bunker_yields`, both on. New `[ai.fort]`
keys `hold_max` 2, `build_after_s` 0, `build_min_squads` 0 (old behaviour); CLI `--side-detail` (read-only). Suite 974 green.
SAVE_VERSION 25, PROTOCOL 14 unchanged. Germany 42.29 % of 480 on seeds 1-280 (43.12 / 42.50 / 41.25); band 45-55 not met.
No fort setting or gate raises Germany on both seed groups while keeping the floors (works and wire >= 60 % of matches).
Germany gains only as fortification is removed for both sides (works off +7.3, build_after_s 600 +8.5). Triple (+6.25) and
builders_max 1 (+3.96) on fresh seeds 1001-1280 failed on 1-280 (-2.5, -1.7). Held-out seeds 301-380 NOT played.
Rule: about 45 of 160 matches change winner under any change; paired 480 standard error near 2.4 points; accept only z >= 2.5 on both seed groups.
Owner 2026-11-16: accept Germany at about 42 % for now (B1-1). Open questions B1-2..4 in `docs/TODO.md`.

## S10c-ai (done on branch `claude/project-thread-e7nsak`, measured 2026-11-16; not on main)

Built (log `S10c-ai-log.md`): hasty AP field, wire and AT field at the 2 front points nearest the enemy base (`[ai.obstacles]` `front`, `front_points`, `front_site_threat_m` 50); `mine_assign` (a free builder within `mine_assign_reach_m` 450); belts (`belt`, `belt_pieces` 5); `jobs_max` 2; `builder_cards_min` 2 (owner S10c-2 open). Checks: `check.sh quick` ok, 881 tests; c_off 40 set = main 40/40.
480 matches (seeds 1-80, 101-180, 201-280; Germany / USA per set): Germany pooled **40.00 %** (40.6 / 37.5 / 41.9). Front share of mines 46.6 / 45.9, 49.7 / 42.3, 52.7 / 42.3 %; enemy within 30 m of mines 59.0 / 36.6, 58.4 / 32.4, 55.8 / 35.1 %; mines set off 0.73 / 0.42, 0.99 / 0.29, 0.74 / 0.44 a match; own wire seen by enemy AI 55.5 / 40.1, 51.0 / 27.8, 43.0 / 36.2 %; belts 10.0 / 1.9, 8.75 / 1.25, 8.1 / 2.5 %; front builders lost 0.55 / 0.84, 0.61 / 0.76, 0.69 / 0.66 a match; breaches 0; own-side mistakes 3 / 2, 0 / 1, 0 / 0.
Targets (skirmish sets): 270 pass, 71 miss, 41 hard. Misses: builder share of infantry plans 15.9-17.3 % (cap 12, T97); sweeps wanted in 26-31 % (T111); AT fields 0.21-0.29 a match (T110, bound 0.5); bunker 5.6-6.3 % (T113, bound 10); own-side mistakes (T115); US mines set off 0.294 (T121d) and wire seen 40.1 / 27.8 % (T122b, T122d). The 28 hard rows not on the S10b-ai list are named in the log.
Open: owner questions S10c-1..6 (`docs/TODO.md`); own_blocked 3 / 2 on seeds 1-80 (R6b, not built); breaches 0 (S10c-6, covering fire is S11); front builders lost over 0.5 on both sides; belts rare, no demolition wanted. Not run yet: full release suite (`check.sh tests`), `tools/verify.sh`, GitHub CI on the branch.

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

- Balance: S10a-ai Germany 41.67 % (480); S10a 48.75 %; T88 USA open (S9b-1 (A)). T5c, T50e known noise. Not verified: the eliminated-player path; a per-field hash audit beyond the named tests. No radio, registered fire, cross-OS CI.
- Net loopback refuses `DropCrates` at a non-truck. Structures cannot be selected in the HUD (S6-18). AI rarely throws satchels.

## Roadmap position

Done on main: S1 to S10b-ai and B1 (B1-1 decided: accept for now; open B1-2..4 in `docs/TODO.md`). Done on the branch, not merged: S10c-ai (verify steps open). Next: owner answers S10c-1..6, the S10c-ai merge decision, then S11.

## How to check work

- `tools/check.sh quick [BASE]` while working; `tools/check.sh tests` runs the whole release suite with ignored tests
  (2-6 min, background it). `tools/verify.sh` is the orchestrator's full check before a push.
- Balance: `tools/measure.sh OUTDIR docs/slices/targets/S10b-ai.toml` (480 matches, 14 min uncached); `python3 -I tools/pool.py DIR`.
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
