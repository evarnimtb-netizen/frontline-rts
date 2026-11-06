# Frontline RTS: state (read this first)

Where the project stands after S7 (branch `s7-casemate-td`, from `9f216e9`; not
pushed; S6b is its base lineage). Main is S5b plus the infra step (`9509ae6`).
Read the sections you need, then the doc the index (last section) names.
Checked 2026-11-06.

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
| Vehicles | Pz IV H, M4A3 (75), StuG III G, M10: box list, turret or casemate mount, open tops, crew seats, components | `vehicle.rs`, `damage.rs`, `gunnery.rs`, `armour.rs` |
| Casemate and open top (S7) | arc-limited casemate gun, pivot-to-lay (`lay_turn`); open faces with no plate; items and rounds into open tops | `gunnery.rs`, `damage.rs`, `items.rs` |
| Armour physics | plates with sloped normals, penetration chance (the one ballistics function) | `armour.rs` |
| Bail-out, re-crew | abandoned tanks, crew escape, capture | `bail.rs` |
| Ammunition | counted rounds per man/gun/vehicle, priced base refill | `supply.rs` |
| HQ, dumps, crates (S6) | pits, stock, crates issued and carried, forward bombs, small arms, tank rearm, static boxes, cook-off | `structures.rs`, `ai/supply.rs`, `s6_tests.rs` |
| Fire missions | mortars: modes direct/observed/map, observers, ranging, charges | `fire_mission.rs`, `indirect.rs` |
| Match rules | tickets, manpower, capture, call-in cards with cooldowns | `rules.rs`, `game.rs`, `command.rs` |
| AI commander | per-player controller: assault, armour, ammo, mortar, supply habits | `ai.rs`, `ai/{assault,armour,ammo,mortar,supply}.rs` |
| World, save, hash | tick loop, save/load, state hash, render snapshot | `world.rs`, `hash.rs`, `snapshot.rs` |
| Content | TOML defs, variants (`extends`, `[[members]]`), tuning tables | `sdk/crates/content/src/` |
| Mod `ww2` | 2 factions, 16 cards plus 2 S6 cards (Zugtrupp / Platoon HQ, Munitionsträger / Ammunition Bearers) and the S7 vehicle card per faction; 4 vehicles (Pz IV H, M4A3 75, StuG III G, M10), 21 weapons plus S7 guns `stuk40_l48`, `m7_3in`; `defs/supply/`: 4 structures, 6 crates | `mods/ww2/defs/` |
| Multiplayer | relay-closed lockstep over UDP, desync dump, reconnect | `crates/net/src/` |
| Godot bridge | GDExtension: match thread, intents, frames | `crates/godot-bridge/src/` |
| Presenter | match view, HUD v2, menus, net lobby, mock source, supply grid | `godot/match/`, `godot/ui/`, `godot/net/` |
| CLI | simulate, skirmish, armour-test, fortify-test, mortar-bench, targets, ballistics, validate, schema, desync-diff | `sdk/crates/cli/src/` |
| Tools | autotune, check.sh, bridge/export | `tools/` |

Not in the game yet: Jagdpanzer IV, 38(t), M18, AT guns, on-map artillery, fortifications, wounds, a third faction, a second map, radio.

## Versions

Main (S5b): `SAVE_VERSION` 14 (`crates/sim/src/world.rs`), `PROTOCOL_VERSION` 8
(`crates/net/src/protocol.rs`). Branch `s6-hq-crates`: 15 and 9, each bumped
once for S6 (`REPORT_VERSION` 1; `--json` files carry `"schema": 1`). A change
S6b on its branch: `SAVE_VERSION` 16, `PROTOCOL_VERSION` 9. S7 (branch `s7-casemate-td`): `SAVE_VERSION` 17 (bumped once), `PROTOCOL_VERSION` 9 (no command or wire change). A change to a save layout, the hash or a command bumps these in the same commit.

## S7 (branch `s7-casemate-td` from `9f216e9`, not pushed)

Assault guns and tank destroyers (`docs/slices/S7-log.md`, spec `S7-casemate-td.md`).
Vehicle schema: 1..=4 armour boxes (own 8-zone plate tables, `open` faces),
optional `[turret]`, mounts `turret_main` / `casemate` / `coax` / `hull`, seats
as a list. Zone id = box x 4 + face (`<box>_<face>`); old tanks keep ids 0-7.
StuG III G (500 MP, casemate arc +-10 deg, pop 10, 120 s) and M10 (520 MP, pop 10,
120 s, hand traverse 5 deg/s). AI: both cards `ai_weight` 100 via the `vehicle` slot;
overwatch 35 m behind own points (`skirmish.toml` `overwatch_back_m`); open tops
back off from infantry; infantry throw into stopped open tops within 25 m.
Balance on 160 sets at 35 m: Germany 48.8 / 45.0 / 48.8 % (pooled 47.5 % over 480);
HE 21.0-21.7 %; StuG and M10 take about 3 % and 2 % of kills (T32a misses on seeds
1-80: Pz IV 18.8 %, S7-15). Targets at final: 25 pass, 0 miss on 40 + armour5.

## S6b (branch `s6b-mortar-ai` from `55b322d`, merged into the S7 lineage)

Mortar AI pass (`docs/slices/S6b-log.md`): map fire held past 20 m expected miss;
`mortar_reserve_pct` 0; crate runs below 90 %; `mortars_per_bearers` 2. Pooled
Germany 47.0 % over 640 (S6b-1). HE 21.9 %, mortar 14.2 / 12.0 % on the 160 set.

## Balance at S6 (head `d355350`)

Skirmish 40: Germany 50 %, base 55 %, median 17.6 min, HE 19.4 %, mortar 12.8 / 8.6 %.
Data: 500 tickets, 600 start manpower, pop cap 150; HQ 100 MP, dump 50 MP. Call-in
cooldowns: rifle 20 s, engineers/SMG/LMG 30, AT 48, mortar 60, tank 120. Details in
`docs/slices/S6-log.md` and `S5b-log.md`.

## Owner decisions in force

Source: `docs/TODO.md` "Owner decisions in force"; full text in `docs/archive/TODO-2026-11-03.md`.

- Our own design, not a recreation. Damage only from physical projectile or
  fragment contact; suppression only from real near-misses.
- Cost-balanced realism: add historical options before changing stats. All
  tuning values live in TOML. Trenches stay strong vs small arms (S4-43).
- Call-in has no queue (buy if affordable, spawn at base, per-card cooldown).
- No free ammo anywhere; distance costs only physical delivery. HQ and dumps
  fill only by carried crates (Q2 C). Tanks rearm at base, HQ or dump (D3).
  Historical squad sizes, pop 150.
- German cards keep the 20 % price cut (S5a-37). The base stops refilling
  mortar bombs (S5a-42, done in S6). Mortars tuned up through AI mission size,
  bomb load and map fire, never lethality: 10-15 % of kills, HE share 20-25 %
  (cap 26.3 %) (S5b-17).
- Tank crews bail out; abandoned tanks can be re-crewed or captured.
- Artillery: on-map crewed units, authentic ranges, observers, counter-battery; no shell ration.
- Vehicles before fortifications. No match under 10 or over 40 min, median 15-30.
- Workflows: `docs/process/workflows.md`.

## Known gaps

- Balance: Germany 47.5 % pooled (S7, in band); base 1 is worth about 6 points (S5b-21/23).
- StuG and M10 take about 3 % and 2 % of kills; T32a misses (S7-15).
- Mortar and HE bands met (S6b, S7); Germany's mortar share is near the 15 % cap.
- Structures cannot be selected in the HUD (S6-18); the S6 screenshot predates the fixes.
- AI rarely throws satchels or fires Panzerfausts. No radio, registered fire, cross-OS CI.

## Roadmap position

Done: S1 to S5b, autotune fix, infra step. S6 HQ
and ammo crates: built, reviewed, fixed on branch `s6-hq-crates` (not pushed).
S6b mortar AI pass: built and fixed on branch `s6b-mortar-ai` (not pushed).
S7 assault guns and TDs: built, reviewed and fixed on branch `s7-casemate-td`
(not pushed). Next: S8
half-tracks/trucks/recon, S9 AT guns, S10a/b fortifications, S11a/b artillery,
S12 wounds and medics, S13 heavy wave, S14 assault kit, S15 USSR. Tiers:
`docs/process/workflows.md` section 1. Sim profiling after S6 (owner).

## How to check work (few lines of output)

- `tools/check.sh quick [BASE]` while working; `tools/check.sh full` before a push.
- Slice numbers: `frontline-cli targets docs/slices/targets/S6.toml --jobs 6`;
  JSON from any run with `--json FILE` (`docs/modding/tuning.md`).
- Balance batches `--jobs 6`; tick times only from a clean `--jobs 3` run.

## Index: for X read doc Y section Z

| For | Read |
|---|---|
| How a slice is built, tiers, models, review gate, runs | `docs/process/workflows.md` 1-5 |
| Open to-do, owner questions, work queue | `docs/TODO.md` |
| Old owner questions, finished to-do, old known issues | `docs/archive/TODO-2026-11-03.md` (per-slice `##`); `docs/archive/roster-done.md` 2 |
| Skirmish rules, AI commander, presenter data flow | `docs/slices/S1-skirmish.md` Gameplay, AI commander |
| Throwables; soldier selection and orders; HUD; theme | `S1b-abilities.md` Items; `S1b-soldier-control.md`; `S1b-hud.md`; `S1b-ui-theme.md` |
| Net protocol, turns, desync dump; playing online | `S2-multiplayer.md` Protocol, Turns, Hash check; `docs/net/playing-online.md` |
| Vehicle model, ballistics, gunnery, commands | `S3-armour.md` Vehicle model, Ballistics and penetration, Commands; log `S3-log.md` |
| Damage audit, fragments, trench pinning, tuning surface | `S3b-balance.md` (same names); log `S3b-log.md` |
| Bail-out, abandoned tanks, re-crew; AI armour habits | `S3c-armour.md` Design Part A, Part B; log `S3c-log.md` |
| Variants: data model, content, Panzerfaust | `S4-variants.md` Data model, Content; log `S4-log.md` |
| Ammunition: data model, rules, base refill, AI | `S5a-ammo.md` Data model, Sim rules, AI; log `S5a-log.md` |
| Fire missions: modes, observers, charges, bench | `S5b-mortars.md` Data model, Sim rules, Targets; `S5b-log.md` "Final measurements", "Final targets" |
| Mortar AI: map fire limit, reserve, bearers, dry orders | `S6b-mortar-ai.md` Changes, Tests; `S6b-log.md` |
| HQ, dumps, crates: spec, build, review, fixes | `S6-hq-crates.md`; `S6-log.md`; module docs of `structures.rs`, `ai/supply.rs`; `godot/docs/match_state.md` "S6" |
| TOML keys: ammo, fire missions, variants, `[supply]` | `docs/modding/ammo.md`; `fire-missions.md`; `variants.md`; `S6-hq-crates.md` Data model |
| Tuning overrides, A/B, `--json`, targets, check.sh; autotune | `docs/modding/tuning.md`; `docs/modding/autotune.md` |
| Roster plan, slice order, owner decisions, HQ/ammo page | `docs/design/roster/roster-expansion.md` 1, 4, Appendix A |
| Infantry variants, melee, wounds, structures | `docs/design/roster/infantry-structures.md` 2-5, 7 |
| Guns, AT guns, howitzers, penetration data | `docs/design/roster/guns-artillery.md` 3, 5, 6 |
| On-map artillery: range, limiters, direct fire, counter-battery | `docs/design/roster/artillery-on-map.md` 3-6 |
| Vehicle catalogue, systems, ammo, build waves | `docs/design/roster/vehicles.md` 4, 6, 7, 9 |
| S7 vehicle schema, casemate, open top, StuG/M10, AI overwatch | `docs/slices/S7-casemate-td.md`; log `S7-log.md`; `godot/docs/match_state.md` "S7" |
| Architecture, determinism, modding, repo layout | `SYSTEM_DESIGN.md` 3, 5, 7, 13 |
