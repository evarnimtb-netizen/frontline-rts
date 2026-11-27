# Frontline RTS: state (read this first)

Main holds S1 to S10b-ai and B1 (merged 2026-11-16, owner OK; **NO default changed**). On branch `claude/project-thread-e7nsak`,
on main since 2026-11-19 (owner OK): S10c-ai, S11a1, S11a2 (built, reviewed, fixed). On the branch, not on main: S11b and B2
(built, reviewed, fixed, measured; S11b waits for the owner's OK to merge). Owner questions: `docs/TODO.md`.

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
| Fire missions | mortars: modes direct/observed/map, observers, ranging, charges; guns (S11a1, branch): infantry gun and 105 mm M3 missions, radio observers, artillery AI; friendly fire and counter-battery by sound (S11a2, branch); field howitzers, light mortars, HE rifle grenades (S11b, branch) | `fire_mission.rs`, `indirect.rs`, `counter_battery.rs`, `ai/arty.rs` |
| Match rules | tickets, manpower, capture, call-in cards with cooldowns | `rules.rs`, `game.rs`, `command.rs` |
| AI commander | per-player controller: assault, armour, ammo, mortar, supply, transport, recon, truck, gun habits | `ai.rs`, `ai/*.rs` |
| World, save, hash | tick loop, save/load, state hash, render snapshot | `world.rs`, `hash.rs`, `snapshot.rs` |
| Content | TOML defs, variants (`extends`, `[[members]]`), tuning tables | `sdk/crates/content/src/` |
| Mod `ww2` | 2 factions; cards for tank, casemate TD, half-track, recon car, truck, AT gun; 15 vehicles incl. 4 guns and the Sd.Kfz. 11; `defs/supply/` | `mods/ww2/defs/` |
| Multiplayer | relay-closed lockstep over UDP, desync dump, reconnect, Embark/Disembark, DropCrates, gun commands, ClearMines, Demolish | `crates/net/src/` |
| Mines, obstacles, bunker (S10b; S10b-ai on main; S10c-ai on branch) | AP and AT minefields, hedgehog and log barriers, bunker pieces, sweep, demolish; the AI lays, builds, sweeps, demolishes (`[ai.obstacles]`), paths avoid known enemy mines, liveness L1-L5, F8 (`[ai.truck] refusal_memory`). S10c-ai: hasty front fields, wire, AT fields and belts at the 2 front points, builder-card floor | `sim/src/sweep.rs`, `demolish.rs`, `fort.rs`; `ai/fort.rs`, `ai/sweep.rs`, `ai/demolish.rs`, `ai/truck.rs`; `ai.rs`; `sdk/crates/cli/src/fortify/s10c_ai.rs` |
| Godot bridge and presenter | GDExtension (match thread, intents, frames); match view, HUD, menus, net lobby, mock, blockouts | `crates/godot-bridge/src/`, `godot/match/`, `godot/ui/`, `godot/net/` |
| CLI and tools | simulate, skirmish, armour-test, fortify-test, mortar-bench, targets, ballistics, validate, schema, desync-diff; autotune, check.sh | `sdk/crates/cli/src/`, `tools/` |

Not in the game yet: Jagdpanzer IV, 38(t), M18, gun tows, displacement, gun crates, registration, flash, HEAT, wounds, a
third faction, a second map, SetHatch or unbuttoned commanders, carried men on trucks, fire from a vehicle, a 3-inch M5
in the AI, HE for the 57 mm.

## Versions

`SAVE_VERSION` **32** and `PROTOCOL_VERSION` **16** on main (`world.rs`, `protocol.rs`); **33** and **17** on the branch
`claude/project-thread-aj1tww` (S14: flame particles, patches, clears, `Combat::close`, `AiState::flame`; commands
`ClearTrench` and `Strike`). SAVE 33 / PROTOCOL 17 also after S14 Fix (no layout change). S15a Build A: SAVE **34**,
PROTOCOL 17 (no new command; `PlayerState::s15`, rider jobs, `Kit::foreign`, `VehicleState::rider_off`, `TransportSetup::exposed`);
SAVE 34 / PROTOCOL 17 also after S15 Fix (no layout change), with three factions in the shipped `[matchups]`. History: S8b 19/11; S9 20/12; S9b 21/12; S10a 22/13;
S10a-ai 23/13; S10b 24/14; S10b-ai 25/14; S10c-ai 26/14; S11a1 27/14; S11a2 28/14; S11b 29/15; S12 30/16; S13 31/16; B3 code 32/16; S14 33/17 (branch); S15a 34/17 (branch). A save, hash or command change
bumps these in the same commit. Relay commands: `sdk/docs/net/relay-protocol.md`. Each TOML value change refreshes the
BASELINE in `sdk/crates/content/tests/resolved_defs.rs` and `crates/simbench/src/report.rs`.

## S10a to S10b-ai (on main; logs `S10a-log.md`, `S10a-ai-log.md`, `S10b-log.md`, `S10b-ai-log.md`)

Fortifications (`fort.rs`, `mods/ww2/defs/fortifications/`; guide `docs/modding/fortifications.md`), mines, obstacles,
bunker, and the AI that lays, sweeps and demolishes (S10b-ai: Germany 41.88 %, 965 tests). AI use stays far under target (owner E6).

## B1 balance pass (on main 2026-11-16; log `docs/slices/B1-log.md`; NO default changed)

Germany 42.29 % of 480 (seeds 1-280); no fort setting raises it on both seed groups. Owner 2026-11-16: accept about 42 % for now (B1-1); B1-2..4 open in `docs/TODO.md`.

## S10c-ai (on main 2026-11-19; log `S10c-ai-log.md`)

Front fields, wire, AT fields and belts at the 2 front points; `builder_cards_min` 2. Germany 40.00 % of 480; 270 targets pass, 71 miss (41 hard). Owner questions S10c-1..6 in `docs/TODO.md`.

## S11a1 gun fire missions and artillery AI (on main 2026-11-19; log `S11a1-log.md`)

SAVE 27. Infantry gun leIG 18 and 105 mm M3 take fire missions (`[fire_mission]` `gun_missions`, `mask`); radio observers (`radio_link`); guns rearm in base at the shell price (`arty_base_rearm`). AI: one gun and one observer team a side (`[ai.arty]` `buy`, `fo`); preparation fire before a breach job (`prep`). Fortify rows T128-T130 pass (T129b a report); breach row T131 a report (S11a1-10); no breach job starts in matches (T138).

## S11a2 friendly fire, counter-battery, AI safety and counter-fire (on main 2026-11-19; log `S11a2-log.md`)

SAVE 28, PROTOCOL 14. Mortar and gun mission bursts hurt own men (`friendly_fire`; own-fire deaths are not kills); infantry-gun rounds leave sound evidence, a marker after 3 rounds (`counter_battery`); the AI lifts fire with own men near (`ff_safe`, `lift_m` 100) and answers a marker no wider than 60 m (`counter_fire`).

## S11b (on main 2026-11-20; log `S11b-log.md`)

Built, reviewed and fixed. Field howitzers as AI base guns (`[ai.arty] how`, with a howitzer pot), HE rifle grenades
(`[items] rifle_grenades`), `buy_pending`, `cb_alternate`. Shipped: howitzers, `buy_pending`, `cb_alternate` and rifle
grenades on; AI light mortars off (`[ai.arty] light` 0; the cards stay buyable by a human). Measured at `88117de` (480):
Germany **44.17 %** (45.00 / 43.75 / 43.75; 41.67 before S11b), HE share 26.8 / 26.5 / 26.2 against the cap 26.3.

## B2 second balance pass (on main 2026-11-20; log `B2-log.md`; NO default changed)

No lever passed the plan's screen rule. `rifle_grenade_gap_s` 15 -> 25 was tried under a separate rule: HE 25.73 on 480,
Germany unchanged at 44.17 %, but unrelated hard target T121b (USA mines set off) fell 0.45 -> 0.281, so it was reverted
(`63e4b04`). Owner questions B2-9..12 in `docs/TODO.md`.

Checks: `tools/verify.sh` NOT run for S11a1, S11a2 or S11b. CI on `affb26e` (S11a1 and S11a2 stage A): Linux, release and
debug sim tests, simulate, clippy, rustfmt, cargo-deny passed; Windows and THIRD_PARTY_NOTICES fail as on main (PR 1).

## Process and owner rules (decided 2026-11-11; also in `docs/TODO.md`)

1. Features first, balance later: no per-slice Tune; each slice measures and records 480 matches. Germany's pooled share
   must stay in a 35-65 % guard band, else stop and diagnose. Fine balance (45-55 %) and soft per-unit targets wait for a
   dedicated balance pass (after S10b-ai or S11).
2. Minor scope and price questions are settled on the recommended (A) and only reported. Irreversible choices, how-it-should-play choices and main updates are still asked.

## Owner decisions in force (source: `docs/TODO.md`)

- Our own design; damage only from projectile or fragment contact; all tuning in TOML; trenches strong vs small arms (S4-43).
- Call-in has no queue; no free ammo; HQ and dumps fill only by carried crates; tanks rearm at base, HQ, dump or truck.
  Historical squad sizes, pop 150. German cards keep the 20 % price cut (S5a-37), except gun cards (S9-4).
- Tank crews bail out. Vehicles before fortifications. Matches 10-40 min, median 15-30.
- S9 (2026-11-08): vehicle spotting 2x; AT gun optic 250 % inside the arc; T33 counts tanks and TDs only. S9b (2026-11-10): S9b-1 (A) keep `income_max_pct` 30, the US T88 shortfall goes to the S10 AI half; S9b-2, S9b-3 (A).

## S12 wounds and medics (on main 2026-11-21, PR 5; log `S12-log.md`)

SAVE 30, PROTOCOL 16. Hit zones; men go fit / wounded / downed (`EntityKind::Downed`) / dead; bleeding, self and buddy
bandage, seeded bleed-out; medics stabilise and carry; aid posts (`kind = "aid"`) treat once and evacuate (refund for bought
men); `Reinforce` refills squads from base. Medic AI `ai/medic.rs` (buy, post, field, staffing, reinforce). Presenter: wound pips,
downed men, aid menu. All shipped on (`[wounds] enabled`, `reinforce`, `[ai.medic]` 1). Measured at `b89ef42` (480): Germany
**42.71 %** (45.00 / 42.50 / 40.62; 44.17 with `s12_off`, which equals main 40/40), no match without a winner, HE share of
casualties 28.1 % against the hard cap 26.3 (known miss S12-16, first item of the next balance pass). Ablations (pooled
Germany): no_medics 41.04, no_post 44.17, no_reinforce 45.00, head_300 43.54. CI run 47 on `91078e9` green.

## S13 heavy wave (branch `claude/project-thread-aj1tww`; log `S13-log.md`; waits for the owner's merge OK)

SAVE 31, PROTOCOL 16. Panther G, Tiger I, M4A3 (76) W, M36, M4A3E2 Jumbo with 4 guns and 11 rounds; obliquity rule O1
(`[armour]`, opt-in per round by `slope_class`, on the 7 new AP rounds only); three armour boxes per new hull (nose,
superstructure, turret); AI heavy buy H1, heavy overwatch H2, push limit H3 (never fires at the defaults); vehicle-slot tie to an
affordable S13 card. Measured at `3b3387f` (480): Germany **42.08 %** (43.12 / 40.00 / 43.12; 42.71 with `s13_off`, equal to S12 on
main), no match without a winner. Heavies bought (160 set): Panther 23.75 %, Tiger 7.5 %, Jumbo about 40 %, M36 0 (T179a-c miss:
S13-F2). Armour duels 41-46 pass except 45 (old PzGr 40 unclassed: S13-F1). CI run 50 green.

## B3 third balance pass (branch `claude/project-thread-aj1tww`; log `B3-log.md`; contract `B3-run-2.md`)

Run 1 (screen) found no TOML set under the HE cap; the B3 code stage added `[ai.arty] direct_in_budget` (SAVE 32). Run 2
confirmed C_all on fresh blocks 3101-3280 (320 pairs: HE share of casualties 27.90 -> 26.05, Germany +0.0, M36 0 -> 37-44 %,
Jumbo 12.5 / 15.6 %) and applied it at `f4e610b`: `[ai.arty] shell_mp_max_pct` 6 and `direct_in_budget` 1, M17 burst 160 cm /
64, M36 `ai_heavy`, PzGr 40 `apcr` and 3-inch M62 `apcbc` (T184d hard). Judge (`target/b3-final`, 480): Germany **47.29 %**
(50.62 / 41.25 / 50.00; +5.2 on `target/s13-on`), no winner 0, HE share of casualties 27.40 / 24.96 / 26.70 (about 26.35
pooled: T8a and T8c miss, T8b passes), T179a-d pass; 4 hard rows that pass at `s13-on` miss (T50b, T119b, T121d, T122c).
By the plan's judge rule (J1, J4) the apply is to be reverted: the orchestrator's call.

## S14 assault kit (branch `claude/project-thread-aj1tww`; log `S14-log.md`; Fix done, Docs/PR next)

SAVE 33, PROTOCOL 17. Flamethrowers (particle stream, burning men and ground, step-out), Clear trench (grenades, burst, advance
along the trench), melee strikes (automatic for a man with nothing to load, `V` with an enemy in reach), flame AI (jobs, buy,
refuel, AI clears). All switches 1 in the mod; every number a key; no balance run. Review findings 1-16 fixed (S14 Fix). `s14_off`
40 equals the base 40 (`neutral.py` same 40/40); default 40: no winner 0, length 10.65 / 21.13 / 32.53 min. R1 (slit) report-only
(S14-8). Owner questions S14-1..13 in `docs/TODO.md`, each on (A).

## S15 USSR (branch `claude/project-thread-aj1tww`; log `S15-log.md`; Fix done, PR next)

SAVE 34, PROTOCOL 17; three factions (`[matchups]`: Germany-USA the default, Germany-USSR; USA-USSR only with `free`). The USSR
roster (14 infantry cards, 5 guns, 7 vehicles; the AT guns towed by the US6 gun tractor, the infantry gun and howitzer without a
tow), tank riders on Soviet decks (10 places on the T-34-85 and IS-2), the SN-42 plate, captured German mortar bombs, bayonets, the
AI's rider job. All switches 1 in the mod; every number a key; no balance run (owner rule). Review findings 1-14 fixed (S15 Fix).
T213c/d (ROKS-3 hits, bayonet strikes in matches) are reports (S15-15). Owner questions S15-1..16 in `docs/TODO.md`, each on (A).

## Known gaps

- Balance: Germany 47.29 % at `f4e610b` (B3 run 2 applied, branch; judge J1/J4 miss, revert pending the orchestrator; 42.08 % at `3b3387f` before it, 42.71 % on main with S12); HE share of casualties about 26.35 pooled against the cap 26.3 (T8a 27.40 misses, B2-12). T88 USA open (S9b-1 (A)). T5c, T50e known noise. Not verified: the eliminated-player path; a per-field hash audit beyond the named tests.
- Net loopback refuses `DropCrates` at a non-truck. Structures not selectable in the HUD (S6-18). No presenter pass for guns (S11a1-9).

## Roadmap position

Done on main: S1 to S12, B1 and B2 (S12 merged 2026-11-21). On the branch: S13, waiting for the owner's merge OK. Next: that merge, then a balance pass starting with S12-16, S13-F1, S13-F2; S14, S15; then the open items (AT guns in cover, preparation fire in matches; S11b-9..11 follow S11a2-10..12).

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
| S10c-ai: front defences, builder floor, belts | `docs/slices/S10c-ai.md`; log `S10c-ai-log.md` |
| S11a1 gun missions, radio, artillery AI, preparation fire; S11a2 friendly fire, counter-battery, AI counter-fire | `docs/slices/S11a1.md` 2, 4, 11; `S11a2.md` 2, 11; logs `S11a1-log.md`, `S11a2-log.md` |
| S11b howitzers, light mortars, rifle grenades; B2 balance pass | `docs/slices/S11b.md`; logs `S11b-log.md`, `B2-log.md`; `docs/TODO.md` B2-1..12 |
| Development speed, audits 2026-11-11 and 2026-11-12 (row honesty, stop points) | `docs/process/dev-speed.md` 7, 8 |
| Architecture, determinism, modding, repo layout | `SYSTEM_DESIGN.md` 3, 5, 7, 13 |
