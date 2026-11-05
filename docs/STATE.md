# Frontline RTS: state (read this first)

Where the project stands after S6 (branch `s6-hq-crates`, from `1575568`; not
pushed). Main is S5b plus the infra step (`9509ae6`). Read the sections you
need, then the doc the index (last section) names. Checked 2026-11-05.

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
| Vehicles | Pz IV H and M4A3 (75): hull/turret plates, components, crew seats | `vehicle.rs`, `damage.rs`, `gunnery.rs` |
| Armour physics | plates with sloped normals, penetration chance (the one ballistics function) | `armour.rs` |
| Bail-out, re-crew | abandoned tanks, crew escape, capture | `bail.rs` |
| Ammunition | counted rounds per man/gun/vehicle, priced base refill | `supply.rs` |
| HQ, dumps, crates (S6) | pits, stock, crates issued and carried, forward bombs, small arms, tank rearm, static boxes, cook-off | `structures.rs`, `ai/supply.rs`, `s6_tests.rs` |
| Fire missions | mortars: modes direct/observed/map, observers, ranging, charges | `fire_mission.rs`, `indirect.rs` |
| Match rules | tickets, manpower, capture, call-in cards with cooldowns | `rules.rs`, `game.rs`, `command.rs` |
| AI commander | per-player controller: assault, armour, ammo, mortar, supply habits | `ai.rs`, `ai/{assault,armour,ammo,mortar,supply}.rs` |
| World, save, hash | tick loop, save/load, state hash, render snapshot | `world.rs`, `hash.rs`, `snapshot.rs` |
| Content | TOML defs, variants (`extends`, `[[members]]`), tuning tables | `sdk/crates/content/src/` |
| Mod `ww2` | 2 factions, 16 cards plus 2 S6 cards per faction (Zugtrupp / Platoon HQ, Munitionsträger / Ammunition Bearers), 2 vehicles, 21 weapons; `defs/supply/`: 4 structures (HQ, dump per faction), 6 crates | `mods/ww2/defs/` |
| Multiplayer | relay-closed lockstep over UDP, desync dump, reconnect | `crates/net/src/` |
| Godot bridge | GDExtension: match thread, intents, frames | `crates/godot-bridge/src/` |
| Presenter | match view, HUD v2, menus, net lobby, mock source, supply grid | `godot/match/`, `godot/ui/`, `godot/net/` |
| CLI | simulate, skirmish, armour-test, fortify-test, mortar-bench, targets, ballistics, validate, schema, desync-diff | `sdk/crates/cli/src/` |
| Tools | autotune, check.sh, bridge/export | `tools/` |

Not in the game yet: other vehicles, AT guns, on-map artillery, fortifications,
wounds and medics, flamethrowers, a third faction, a second map, radio.

## Versions

Main (S5b): `SAVE_VERSION` 14 (`crates/sim/src/world.rs`), `PROTOCOL_VERSION` 8
(`crates/net/src/protocol.rs`). Branch `s6-hq-crates`: 15 and 9, each bumped
once for S6 (`REPORT_VERSION` 1; `--json` files carry `"schema": 1`). A change
to a save layout, the hash or a command bumps these in the same commit.

## Balance now (S6 on branch, head `d355350`)

Measured in `docs/slices/S6-log.md`; before S6, `docs/slices/S5b-log.md`.

- Skirmish 40 after the review fixes (seeds 1-20 x both bases): Germany 50 %
  (T5a/b 50 / 50), base side 55 %, length 10.5-34.5 min (median 17.6), HE
  share 19.4 % (band 20-26.3 loose), mortar share 12.8 / 8.6 %, AI
  observed-or-direct 84 %, 0 matches without a winner. Refusals: 1 `no_bombs` each side.
- Sets of 160 (final binary `6d88ea1`, orchestrator check): Germany 50.6 %,
  51.9 % out of sample; base side up to 64.4 % oos (T6c miss). Mortar share
  Germany 11.6 % (in band), USA 9.2 % (below 10); HE 19.8 / 19.1 % (below 20).
- Base refills no mortar bombs (`base_bomb_s = 0`, owner Q4): base bombs and
  unsourced bombs are both 0 on every run (T20). HQ and dump stock comes only
  from crates (T22).
- Crates: shells crate 32 kg, priced at issue (kg x price); AI issues 4.7-6.9
  shells crates and delivers 4-7 per match; crate MP 180-200 per match. HQ built
  in 100 % of matches (T27). Cook-offs rare (0.05-0.125 per match); tank crate
  runs 0 (AI tanks never run low).
- Tune values in force (`mods/ww2`): German infantry cards x1.03 (intervals in
  TOML comments), `[ai.mix] mortar = 50`.
- Data (`mods/ww2/defs/rules/skirmish.toml`): 500 tickets, 600 start manpower,
  +240/min, pop cap 150, `shell_box_damage` 150, `tank_crate_far_m` 300. HQ 100 MP
  (25 m), dump 50 MP (15 m). Call-in cooldowns: rifle 20 s, engineers/SMG/LMG 30,
  AT 48, mortar 60, tank 120.
- Tests at `d355350`: `check.sh quick` 440 pass, 0 fail (cli, content, sim; the
  Tune run reported 551 over another set, not reconciled). Godot: `test_match_view`
  291/291, `test_data_reload` 21/21, `test_net_ui` 143/143. Tick p99 0.475 ms (`--jobs 6`).

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

- Balance: Germany 50 % on 40 matches, 51.3 % pooled on 320; base 1 is
  worth about 6 points of win share to either faction (S5b-21/23 open).
- USA mortar share 9.2 % and HE share 19.1-19.8 % are below band on 160 sets;
  the Tune levers do not reach them (needs AI or code work).
- Structures cannot be selected in the HUD (S6-18). `docs/slices/screens/s6_hq_supply.png`
  predates the tune and the fixes: retake it.
- One `no_bombs` AI refusal per 40 matches (S5b-22). AI rarely throws satchels
  or fires Panzerfausts. No radio, registered fire, cross-OS CI.
- Full list: `docs/TODO.md` "Known issues still open".

## Roadmap position

Done: S1 to S5b, autotune fix, infra step. S6 HQ
and ammo crates: built, reviewed, fixed on branch `s6-hq-crates` (not pushed).
Next: S6 balance check on 160 + 160 matches, then S7 assault guns and TDs, S8
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
| HQ, dumps, crates: spec, build, review, fixes | `S6-hq-crates.md`; `S6-log.md`; module docs of `structures.rs`, `ai/supply.rs`; `godot/docs/match_state.md` "S6" |
| TOML keys: ammo, fire missions, variants, `[supply]` | `docs/modding/ammo.md`; `fire-missions.md`; `variants.md`; `S6-hq-crates.md` Data model |
| Tuning overrides, A/B, `--json`, targets, check.sh; autotune | `docs/modding/tuning.md`; `docs/modding/autotune.md` |
| Roster plan, slice order, owner decisions, HQ/ammo page | `docs/design/roster/roster-expansion.md` 1, 4, Appendix A |
| Infantry variants, melee, wounds, structures | `docs/design/roster/infantry-structures.md` 2-5, 7 |
| Guns, AT guns, howitzers, penetration data | `docs/design/roster/guns-artillery.md` 3, 5, 6 |
| On-map artillery: range, limiters, direct fire, counter-battery | `docs/design/roster/artillery-on-map.md` 3-6 |
| Vehicle catalogue, systems, ammo, build waves | `docs/design/roster/vehicles.md` 4, 6, 7, 9 |
| Architecture, determinism, modding, repo layout | `SYSTEM_DESIGN.md` 3, 5, 7, 13 |
