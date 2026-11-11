# Frontline RTS: state (read this first)

Main holds S1 to S10a mechanics (`fa45332`). Work now: S10a-ai (fortification AI and balance) on branch `s10a-ai`, head `a0aa700`. It is built, reviewed, fixed and verified, but NOT merged to main. Spec `docs/slices/S10a-ai.md`, targets `S10a-ai.toml`, log `docs/slices/S10a-ai-log.md`.
Read the sections you need, then the doc the index names. Checked 2026-11-11.

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
| Multiplayer | relay-closed lockstep over UDP, desync dump, reconnect, Embark/Disembark, DropCrates, gun commands | `crates/net/src/` |
| Godot bridge and presenter | GDExtension (match thread, intents, frames); match view, HUD, menus, net lobby, mock, blockouts | `crates/godot-bridge/src/`, `godot/match/`, `godot/ui/`, `godot/net/` |
| CLI and tools | simulate, skirmish, armour-test, fortify-test, mortar-bench, targets, ballistics, validate, schema, desync-diff; autotune, check.sh | `sdk/crates/cli/src/`, `tools/` |

Not in the game yet: Jagdpanzer IV, 38(t), M18, on-map artillery, wounds, a third faction, a second map, radio,
SetHatch or unbuttoned commanders, carried men on trucks, fire from a vehicle, a 3-inch M5 in the AI, HE for the 57 mm.

## Versions

`SAVE_VERSION` **22** and `PROTOCOL_VERSION` **13** on main (S10a); S10a-ai sets SAVE **23**, PROTOCOL 13 (`world.rs`, `protocol.rs`). History: S5b 14 and 8; S6 15 and 9; S7 17 and 9;
S8a 18 and 10; S8b 19 and 11; S9 20 and 12; S9b 21 and 12; S10a 22 and 13. A save, hash or command change bumps these
in the same commit. Relay commands: `sdk/docs/net/relay-protocol.md`. Each TOML value change refreshes the BASELINE in
`sdk/crates/content/tests/resolved_defs.rs` and `crates/simbench/src/report.rs`.

## S9 and S9b (on main; logs `S9-log.md`, `S9b-log.md`; specs `S9-at-guns.md`, `S9b-ai-guns.md`)

AT guns with crews as soldiers (`crew_model = "served"`, `[[service]]` slots, tow bar, own AI budget). S9: Germany 45.6 %,
14 hard misses; tank KOs by guns 36 GER / 16 USA. S9b corridor siting (`ai/guns.rs`): Germany 48.75 % of 480, 10 hard misses, SAVE 21.

## S10a fortifications, mechanics (on main since 2026-11-11; log `S10a-log.md`)

Sim `crates/sim/src/fort.rs`; blueprints `mods/ww2/defs/fortifications/`; commands Fortify, LoadMaterials, CutWire,
PlaceBangalore; capabilities build_structure, build_heavy, place_wire, clear_wire, breach. Cards ger_sturmpioniergruppe
264 MP, ger_baupioniergruppe 165 MP, usa_assault_section 380 MP, `ai_weight` 0. Guide `docs/modding/fortifications.md`.

## S10a-ai fortification AI and balance (branch `s10a-ai`, NOT on main; log `S10a-ai-log.md`)

Verified by the orchestrator on `72fd940` plus formatting-only `a0aa700` (`target/s10a-ai-verify`).
- Tests: 802 pass, 0 fail (release, ignored included). Tick p99 0.476 ms all, 0.574 ms peak.
- Neutrality: with all `[ai.fort]` switches off, the 40 set is identical to the pre-slice base (40 of 40). The jobs-3
  40 hashes are all in the jobs-6 160 run (40 of 40).
- Balance, 480 matches, AI building on: Germany pooled **41.67 %** (sets 43.75 / 38.75 / 42.50), no match without a
  winner. Before the slice: 48.75 %. Outside the old 45-55 % band, inside the new 35-65 % guard band.
- Fort use (160 sets): Germany works 94 / 93 / 97 %, wire 78 / 82 / 79 %; USA works 82 / 79 / 80 %, wire 56 / 58 / 57 %
  (floor 60: T95b, T95d miss). Builders 7.7-8.2 % of infantry plans (cap 12). Gaps 0 %, breaches started 0 in every set.
- Targets: 329 pass, 26 miss, 21 of them hard. Hard misses accepted from S9b: T8b, T31b, T40c, T54e, T88b, T88d.
  Reports under owner rule 1, still hard in `S10a-ai.toml`: T5c, T5d, T31a, T31c, T31d, T55a, T55b.
- **Open:** T93f 0 and T93h 0 (floor 30); T95b 56.25 and T95d 58.125 (floor 60); T96a-d 0 (floor 60). T96 counts only
  matches where the AI met enemy wire on a route; with none it is null (SKIP).
- Breaching: smoke on three men (owner 2026-11-11), any living carrier throws; the breach row did not move. A throw needs
  25-29 m from the wire; German squads fall 69-95 m out, US squads are pinned 27-38 m out. A lone squad cannot breach
  defended wire without covering fire: S11 combined-arms work.
- Ablations (Build D, 480 each): nest, dugout, roof, sandbags, wire each move Germany under 2 points (all point building off, earlier: +6.46).
- Review: 19 findings (1 blocker, 6 majors after merging duplicates); 18 fixed, each with a test.
  **Open: F8** (an S8b truck forgets an unreachable HQ goal after four give-ups and orders it again about 5 min later).
  It also happens with all switches off, so a fix changes the neutrality base; deferred to a later slice.
- Fix constants (`REFUSED_S` 300, `DUGOUTS_MAX` 2, `WATCH_*`) are code in `ai/fort.rs`, not keys (a key would change the setup layout).

## Process and owner rules (decided 2026-11-11; also in `docs/TODO.md`)

1. Features first, balance later: no per-slice Tune; each slice measures and records 480 matches. Germany's pooled share
   must stay in a 35-65 % guard band, else stop and diagnose. Fine balance (45-55 %) and soft per-unit targets wait for a
   dedicated balance pass (after S10b or S11).
2. Minor scope and price questions are settled on the recommended (A) and only reported. Irreversible choices, how-it-should-play choices and main updates are still asked.
3. S10a-ai decisions: breaching "fix smoke, lower bar" (T93f/h floor 30, T93g report); AI wire in 60 m lines; cards Germany by weight (Bau 100, Sturm 60, Pioniere 40), USA floor a report; smoke on three men.
- Open owner question S10a-20 (TODO): T93f/h and T96 reports until S11 (A, recommended), or hard plus a lever now (B).

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

Done and on main: S1 to S10a mechanics. S10a-ai is verified on its branch; it waits for the owner's merge decision and
S10a-20. Then S10b (mines, AT obstacles, bunker), a dedicated balance pass, S11a/b, S12 to S15. Tiers: `docs/process/workflows.md` 1.

## How to check work

- `tools/check.sh quick [BASE]` while working; `tools/check.sh tests` runs the whole release suite with ignored tests
  (2-6 min, background it). `tools/verify.sh` is the orchestrator's full check before a push.
- Balance: `tools/measure.sh OUTDIR docs/slices/targets/S10a-ai.toml` (480 matches, 14 min uncached); `python3 -I tools/pool.py DIR`.
  Quick look (40 matches, noise 11-15 points): `tools/batch.sh OUT.json -- skirmish --ai-vs-ai --seed 1 --seeds 20 --both-bases --summary --minutes 45`.
- Targets: `frontline-cli targets docs/slices/targets/S10a-ai.toml --jobs 6`. Ablations: `tools/ablate.sh`. Gun funnel:
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
| Development speed, the 2026-11-11 audit and adopted rules | `docs/process/dev-speed.md` section 7 |
| Architecture, determinism, modding, repo layout | `SYSTEM_DESIGN.md` 3, 5, 7, 13 |
