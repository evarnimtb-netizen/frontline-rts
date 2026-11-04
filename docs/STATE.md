# Frontline RTS: state (read this first)

Where the project stands after S5b plus the infra step (branch
`infra-checks`, from `9509ae6`). Read the sections you need, then the doc the
index (last section) names. Checked against code and logs on 2026-11-04.

## What the game is now

An original WW2 tactical RTS (Gates of Hell style): a deterministic Rust
sim (30 Hz, fixed-point, counter-based RNG, state hash) and a Godot 4.7.2
presenter. One map (Kessel Ridge, 1024 m, 5 capture points), two factions
(Germany, USA), skirmish vs AI or 1v1 human vs human online. Rust 1.99.0, workspace in
`Cargo.toml`.

| System | What it does | Code |
|---|---|---|
| Terrain, trenches | two-tier heightfield; digging, craters, parapets | `crates/sim/src/terrain.rs`, `dig.rs` |
| Movement, squads | A* on a 4 m grid, formations, soldier-level orders | `path.rs`, `squad.rs`, `unit.rs`, `orders.rs` |
| Perception | ray-budgeted spotting, smoke | `spotting.rs` |
| Infantry combat | physical bullets and fragments, suppression, cover | `combat.rs`, `projectile.rs` |
| Throwables, AT items | frag, smoke, satchel, Panzerfaust (single-shot) | `items.rs` |
| Vehicles | Pz IV H and M4A3 (75): hull/turret plates, components, crew seats | `vehicle.rs`, `damage.rs`, `gunnery.rs` |
| Armour physics | plates with sloped normals, penetration chance (the one ballistics function) | `armour.rs` |
| Bail-out, re-crew | abandoned tanks, crew escape, capture | `bail.rs` |
| Ammunition | counted rounds per man/gun/vehicle, priced base refill | `supply.rs` |
| HQ, dumps, crates (S6) | pits, stock, crates issued and carried, forward bombs, small arms and tank rearm, static boxes, cook-off | `structures.rs`, `ai/supply.rs` |
| Fire missions | mortars: modes direct/observed/map, observers, ranging, charges | `fire_mission.rs`, `indirect.rs` |
| Match rules | tickets, manpower, capture, call-in cards with cooldowns | `rules.rs`, `game.rs`, `command.rs` |
| AI commander | per-player controller: assault, armour, ammo, mortar habits | `ai.rs`, `ai/{assault,armour,ammo,mortar}.rs` |
| World, save, hash | tick loop, save/load, state hash, render snapshot | `world.rs`, `hash.rs`, `snapshot.rs` |
| Content | TOML defs, variants (`extends`, `[[members]]`), tuning tables | `sdk/crates/content/src/` |
| Mod `ww2` | 2 factions, 16 cards (18 unit files with 2 crews), 2 vehicles, 21 weapons | `mods/ww2/defs/` |
| Multiplayer | relay-closed lockstep over UDP, desync dump, reconnect | `crates/net/src/` |
| Godot bridge | GDExtension: match thread, intents, frames | `crates/godot-bridge/src/` |
| Presenter | match view, HUD v2, menus, net lobby, mock source | `godot/match/`, `godot/ui/`, `godot/net/` |
| CLI | simulate, skirmish, armour-test, fortify-test, mortar-bench, targets, ballistics, validate, schema, desync-diff | `sdk/crates/cli/src/` |
| Tools | autotune, check.sh, bridge/export scripts | `tools/` |

Not in the game yet (S6 on branch `s6-hq-crates` adds HQ and crates): any other vehicle, AT guns, on-map
artillery, fortification blueprints, wounds and medics, flamethrowers, a
third faction, a second map, radio.

## Versions

`SAVE_VERSION` 14 (`crates/sim/src/world.rs`), `PROTOCOL_VERSION` 8
(`crates/net/src/protocol.rs`); S6 (branch `s6-hq-crates`): 15 and 9, `REPORT_VERSION` 1 (simulate reports), JSON
files from `--json` carry `"schema": 1`. A change to a save layout, the hash
or a command bumps these in the same commit.

## Balance now (S5b final binary, `docs/slices/S5b-log.md` "Final measurements")

- Skirmish 40 (seeds 1-20 x both bases): Germany 16 wins (40.0 %), base 1
  wins 60.0 %, length 11.9-30.6 min (median 17.1), HE share of kills 13.3 %
  (max 20.9), tick p99 peak 0.53 ms at `--jobs 3`, 1 AI mortar order refused
  (`no_bombs`, a crew-wipe race). Re-run on this build: same numbers, all 40
  hashes equal to `9509ae6`.
- Sets of 160: Germany 47.5 % (seeds 1-80), 40.6 % (out of sample 101-180);
  base 1 53.8 / 60.6 %. Pooled 480: 45.4 %. One 160 set has a spread of about
  4.7 points: judge on 320+ matches.
- Autotune (docs/TODO.md queue item 2): German infantry card price x0.959
  [0.913-0.999] gives 50 %; today 44.9 % [40.0-49.9]. Input for S6, not shipped.
- Mortars: 5.5 / 4.4 % of kills in AI play (target 10-15 %, S5b-17).
- Armour and fortify: all hard checks pass; fortify mortar row 39.5 % (limit
  40 %); stress case p99 1.64 ms (limit 10).
- Tests: 548 pass, 0 fail on this branch (540 at S5b close; `tools/check.sh full`);
  Godot: `test_match_view` 275, `test_net_ui` 143, `test_data_reload` 21 checks.
- Data in force (`mods/ww2/defs/rules/skirmish.toml`): 500 tickets, 600 start
  manpower, +240/min (+60 per point), pop cap 150, vehicles unlock at 240 s.
  Call-in cooldowns in the unit files: rifle 20 s, engineers/SMG/LMG/BAR 30,
  VG 45, AT 48, mortar 60, tank 120.

## Owner decisions in force

Source: `docs/TODO.md` "Owner decisions in force"; each question's full text:
`docs/archive/TODO-2026-11-03.md` (look up by id, e.g. S4-43).

- Our own design, not a recreation. Damage only from physical projectile or
  fragment contact; suppression only from real near-misses.
- Cost-balanced realism: add historical options before changing stats. All
  tuning values live in TOML. Trenches stay strong vs small arms (S4-43).
- Call-in has no queue (buy if affordable, spawn at base, per-card cooldown).
- No free ammo anywhere; distance costs only physical delivery. HQ refills only
  by carried crates (Q2 C). Historical squad sizes, pop 150.
- German cards keep the 20 % price cut (S5a-37). The base stops refilling
  mortar bombs in S6 (S5a-42). Mortars tuned up in S6 via AI mission size,
  bomb load and map fire, never lethality: 10-15 % of kills, HE share 20-25 %
  (cap 26.3 %) (S5b-17).
- Tank crews bail out; abandoned tanks can be re-crewed or captured.
- Artillery is on-map crewed units with authentic ranges, observers,
  counter-battery; no shell ration; field howitzers on all maps, cap 1.
- Vehicles before fortifications. No match under 10 or over 40 min, median 15-30.
- Workflows: tiers, review gate, models, run rules in `docs/process/workflows.md`.

## Known gaps

- Out of sample the German share is 40.6 % (S5b-21/23 open); one `no_bombs`
  AI refusal per 40 matches (S5b-22).
- Mortars kill too little; the base-parked mortar fires unlimited from base (S6).
- Base 1 is worth about 6 points of win share to either faction.
- AI rarely throws satchels or fires Panzerfausts. No radio, registered fire.
- No cross-OS CI; MPL-2.0 text and `socket2` missing from
  `THIRD_PARTY_NOTICES`; `frontline-relay` not run on Linux yet.
- Host crash: guest waits about 75 s, no "claim the win" button. Presenter:
  48 vehicles per type drawn; sim step spikes in windowed runs not investigated.
- Owner play tests pending. Full list: `docs/TODO.md` "Known issues still open".

## Roadmap position

Done: S1 skirmish, S1b soldier control and HUD, S2 multiplayer, S3 armour,
S3b balance, S3c bail-out, S4 variants, S5a ammunition, S5b mortars, the
autotune fix, this infra step. Next: S6 HQ and ammo crates (engine tier,
`docs/design/roster/roster-expansion.md` S6 card), then S7 assault guns and
TDs, S8 half-tracks/trucks/recon, S9 AT guns, S10a/b fortifications, S11a/b
artillery, S12 wounds and medics, S13 heavy wave (data), S14 assault kit,
S15 USSR (data). Sizes and tiers: `docs/process/workflows.md` section 1.
Sim profiling is deferred until after S6.

## How to check work (few lines of output)

- `tools/check.sh quick [BASE]` while working; `tools/check.sh full` before a push.
- Slice numbers: `frontline-cli targets docs/slices/targets/S5b.toml --jobs 6`
  (model file); JSON from any run with `--json FILE`. Details:
  `docs/modding/tuning.md` "Results as JSON, targets and checks".
- Balance batches `--jobs 6`; tick times only from a clean `--jobs 3` run.

## Index: for X read doc Y section Z

| For | Read |
|---|---|
| How a slice is built, tiers, models, review gate, runs | `docs/process/workflows.md` 1-5 |
| Open to-do, owner questions, work queue | `docs/TODO.md` |
| Old owner question text, finished to-do, old known issues; the variant design as proposed | `docs/archive/TODO-2026-11-03.md` (per-slice `##`); `docs/archive/roster-done.md` 2 |
| Skirmish rules, AI commander, presenter data flow | `docs/slices/S1-skirmish.md` Gameplay, AI commander |
| Throwables; soldier selection and orders; HUD; theme | `S1b-abilities.md` Items; `S1b-soldier-control.md`; `S1b-hud.md`; `S1b-ui-theme.md` |
| Net protocol, turns, desync dump; playing online | `S2-multiplayer.md` Protocol, Turns, Hash check; `docs/net/playing-online.md` |
| Vehicle model, ballistics, gunnery, commands | `S3-armour.md` Vehicle model, Ballistics and penetration, Commands; log `S3-log.md` |
| Damage audit, fragments, trench pinning, tuning surface | `S3b-balance.md` (same names); log `S3b-log.md` |
| Bail-out, abandoned tanks, re-crew; AI armour habits | `S3c-armour.md` Design Part A, Part B; log `S3c-log.md` |
| Variants: data model, content, Panzerfaust | `S4-variants.md` Data model, Content; log `S4-log.md` |
| Ammunition: data model, rules, base refill, AI | `S5a-ammo.md` Data model, Sim rules, AI; log `S5a-log.md` |
| Fire missions: modes, observers, charges, bench | `S5b-mortars.md` Data model, Sim rules, Targets; `S5b-log.md` "Final measurements", "Final targets" (every log has "Final targets") |
| HQ, dumps, crates: data, rules, AI, HUD | `S6-hq-crates.md`; `S6-log.md`; module docs of `structures.rs`, `ai/supply.rs`; `godot/docs/match_state.md` "S6" |
| TOML keys: ammo; fire missions and `[ai.mortar]`; variants (`extends`, `[[members]]`) | `docs/modding/ammo.md`; `fire-missions.md`; `variants.md` |
| Tuning overrides, A/B, `--json`, targets, check.sh; autotune | `docs/modding/tuning.md`; `docs/modding/autotune.md` |
| Roster plan, slice order, owner decisions, HQ/ammo page | `docs/design/roster/roster-expansion.md` 1, 4, Appendix A |
| Infantry variants, melee, wounds, structures | `docs/design/roster/infantry-structures.md` 2-5, 7 |
| Guns, AT guns, howitzers, penetration data | `docs/design/roster/guns-artillery.md` 3, 5, 6 |
| On-map artillery: range, limiters, direct fire, counter-battery | `docs/design/roster/artillery-on-map.md` 3-6 |
| Vehicle catalogue, systems, ammo, build waves | `docs/design/roster/vehicles.md` 4, 6, 7, 9 |
| Architecture, determinism, modding, repo layout | `SYSTEM_DESIGN.md` 3, 5, 7, 13; build and run: `README.md` |
