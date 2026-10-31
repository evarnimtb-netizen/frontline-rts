# Slice S1: Skirmish vs AI with trenches

**Owner decision (2026-10-31):** build playable-first. S1 is the first version
someone can play: launch the game, pick Skirmish, fight one AI opponent on one
map, dig trenches that work as real cover, and win or lose. S2 adds 2-player
networking on top, so S1 must stay lockstep-ready.

Read `SYSTEM_DESIGN.md` for the full design. This file narrows it to S1 and
wins where they differ. Relevant sections: §5.1–5.8 (sim core, infantry),
§5.10 (terrain and fortifications), §5.11 (determinism), §4.1 (Godot rules),
§7 (data layout), §12 (presentation).

## Hard rules

- **All gameplay lives in the deterministic sim** (`crates/sim`), driven only
  by `Command`s. Q16.16 fixed point, counter RNG, no floats, no HashMap, no
  wall-clock, ascending-id iteration (existing lints enforce most of this).
- **The AI is a sim-side player controller.** It runs inside `World::step`
  at a fixed stage, reads sim state, and emits `Command`s through the same
  path a human uses. Same seed + same human command log = same match.
- **The sim does no file IO.** Content is TOML under `mods/`, resolved by
  `sdk/crates/content` into a `MatchSetup` value (fixed point) that is part
  of the saved and hashed state.
- Every new piece of state is saved, loaded and hashed. Determinism tests
  must cover it.
- Keep the presenter rules from §4.1: no Node per unit, MultiMesh batches,
  one buffer upload per batch per frame.

## Out of scope for S1

Vehicles (remove the placeholder tanks from the match scene; keep them in the
bench scene), direct control, buildings, networking, mod loader UI,
campaign, USSR faction, Luau, WASM.

## Gameplay

**Map.** One map, "Kessel Ridge", 1024 × 1024 m, defined in
`mods/ww2/defs/maps/kessel_ridge.toml`: generated heightfield from seed and
shape parameters (rolling hills, one ridge, a shallow valley), 5 capture
points (A–E, the middle one on the ridge), 2 base zones in opposite corners,
and woods patches. Woods reduce spotting range inside them (concealment);
they do not block bullets.

**Factions.** Germany vs USA. Same mechanics, own names, stats and colours.

**Units (squads):**

| Squad | Men | Weapon | Role |
|---|---|---|---|
| Rifle squad | 8 | rifles (Kar98k / M1 Garand) | line infantry, digs at base rate |
| MG team | 3 | MG42 / M1919 | high rate of fire, strong suppression, 3 s setup before firing after moving |
| Engineer squad | 5 | SMGs (MP40 / M3) | digs 2× faster, short range |
| Mortar team | 3 | 81 mm mortar | indirect HE, 60–600 m, needs a target position |

Stats live in `mods/ww2/defs/units/*.toml` and `weapons/*.toml`.

**Economy.** Manpower income per second, plus a bonus per held point. Squads
cost manpower and arrive at the base after a call-in delay. Population cap
120 soldiers per side.

**Victory.** 500 tickets each. While one side holds more points, the other
side's tickets drain at a rate proportional to the difference. A side with 0
tickets, or with no soldiers and no manpower to buy any, loses. A match also
ends if a player surrenders.

**Capture.** A point is captured by having soldiers within 15 m with no
enemies there. Capture takes 20 s (faster with more men, capped). Contested
points freeze.

**Orders (commands):** Move, AttackMove, Stop, Retreat (to base, faster,
ignores enemies), Dig (trench along a polyline, ≤ 60 m, split among the
squad), Barrage (mortar: fire on a position for N rounds), CallIn (buy a
squad), Surrender. Stance is automatic.

**Squads and formation.** Members hold slots around the squad's leader point.
On arrival they spread along a line facing the move direction and seek cover
within 10 m: prefer trench cells, then positions with terrain between them and
the nearest known enemy.

**Pathfinding.** Grid A* on the 4 m heightfield grid with slope cost; trench
cells cost less for infantry already in a trench line. Budgeted node
expansions per tick (deterministic queue). ORCA avoidance stays.

**Spotting.** Per-player visibility. Each soldier has a sight range (about
150 m; prone or in-trench targets are harder to spot; woods reduce it; firing
reveals the shooter). LOS rays against the heightfield including dug terrain,
with a per-tick ray budget and rotating cursors (§2.2, §5.6). Soldiers only
engage spotted targets. The snapshot carries a per-player visibility mask so
the presenter can draw fog of war.

**Combat.**
- Soldiers auto-acquire spotted enemies in range and fire real projectiles
  from the existing pool.
- Accuracy depends on weapon spread, distance, shooter movement and
  suppression.
- Hits are swept segment tests against per-soldier hit capsules. Capsule
  height depends on stance: standing 1.75 m, crouched 1.2 m, prone 0.4 m.
- Projectiles also hit terrain, including dug terrain, so a trench lip
  physically blocks fire. Cover comes from geometry, never from cover values
  (§5.10.4).
- Soldiers have 100 HP. Weapons deal damage per hit. At 0 HP a soldier dies.
  Corpses stay in the snapshot for 30 s for the presenter.

**Suppression.** Near misses within about 2 m and nearby explosions add
suppression; it decays over time. Thresholds:
- crouch;
- pinned: prone, no movement, accuracy down.

**HE (mortar).** Shells fly a ballistic arc. The explosion deals damage with
radius falloff (lethal core about 3 m, damage to about 8 m) and suppression
to about 15 m. Damage to a soldier is blocked when terrain lies between the
blast point (+0.3 m) and the soldier's body point, so trenches protect except
against near-direct hits. Each explosion carves a crater (bowl about 2 m
radius and 0.5–1 m deep, with a rim).

**Digging (core pillar).**
- Terrain gets the §5.10.1 detail tier: sparse 0.25 m tiles that are created
  where terrain is changed, over the 4 m base.
- A Dig order lays a trench along the polyline. Profile:
  - 1.6 m deep;
  - 0.8 m floor;
  - sloped walls;
  - spoil parapet about 0.3 m high on the side facing the nearest enemy.
- Each digger moves earth at a work rate. Pacing follows §5.10.9 compressed
  realism: about 3–4 min for one rifle squad to dig 20 m; engineers dig 2×
  faster; MG and mortar teams don't dig.
- Progress is visible: the trench deepens in passes.
- A soldier standing in a trench is at floor height. Crouched, he is fully
  covered from flat fire. When firing, he stands, with about 0.3 m exposed.

## AI commander

The AI runs inside the sim and is deterministic. It re-plans every 2 s per
AI player:
- scores points by value, distance and threat;
- assigns squads to capture, defend or attack;
- buys units with a target mix;
- has engineers dig trenches at held points, facing the nearest enemy;
- has mortars barrage spotted enemy groups and trench lines;
- retreats squads below 40 % strength.

Difficulty levels are Easy, Normal and Hard. They change income, reaction
delay and the quality of its decisions.

## Data flow to the presenter

The `RenderSnapshot` grows to carry:
- per-soldier: stance, alive/dead, owner, squad, health and suppression;
- squads: type, owner, member count, order state and selection anchor;
- the per-player visibility mask;
- capture points: owner and progress;
- tickets, manpower, income and pop for each player;
- explosions and shots fired this tick, for effects;
- dirty terrain detail tiles, so the presenter rebuilds only changed chunks;
- match state: running, won or lost, and the winner.

## Presenter (Godot)

- **Main menu.** Title, Skirmish setup (faction, AI difficulty, map), Settings
  (resolution, fullscreen, render scale, volumes), Quit.
- **Camera.** RTS camera with edge scroll, rotate, zoom 10–150 m, clamped to
  the map.
- **Selection.** Click, box-select, shift to add, Ctrl+1–9 groups,
  double-click to select the same type on screen.
- **Orders.**
  - Right-click: move. A: attack-move. S: stop. R: retreat.
  - D: dig tool. Click to place points, see a ghost preview, right-click to
    confirm.
  - Mortar barrage: B, or right-click the ground with a mortar selected.
- **HUD.**
  - Top bar: manpower and income, both sides' tickets, pop.
  - Capture point strip.
  - Selected squad cards: men, health, suppression state.
  - Command panel with hotkeys.
  - Call-in panel: costs, queue, timers.
  - Minimap: points, visible units, camera frustum; click to move the camera.
  - Event messages ("Point B lost").
  - Pause menu: resume, settings, surrender, quit to menu.
- **World.**
  - Terrain mesh that rebuilds only dug chunks.
  - Trench and crater dirt shading; woods.
  - Capture point flags and rings, selection rings, order markers.
  - Team colours.
  - Stance poses: stand, crouch, prone. Death fall, then corpses.
  - Muzzle flashes, tracers, explosions with a light flash, dust, smoke.
- **Fog of war.** Enemies are drawn only when spotted by the local player.
- **Audio.** 3D positional sounds: rifle, MG, mortar launch, explosion,
  digging, UI, ambient wind. CC0 only, with a `.license.toml` per file
  (§12.1). Keep total audio under 5 MB and total downloaded assets minimal.
- **End screen.** Victory or defeat, stats (kills, losses, time holding
  points), back to the menu.

## Done when

1. `cargo test --workspace --release`, clippy and fmt pass. They include:
   - AI-vs-AI skirmish determinism: same hashes at 1/2/3/8 threads, and
     through save → load → continue mid-match.
2. `frontline-cli skirmish --ai-vs-ai --seed N` finishes with a winner inside
   30 game minutes for seeds 1–5 and prints stats.
3. `frontline-cli fortify-test` shows trenches working:
   - rifle squad in a trench vs rifle squad in the open at 150 m: the trench
     side's casualties are at most half the open side's;
   - under a mortar barrage, casualties in a trench are at most 40 % of
     casualties in the open.
4. The Godot game boots to the menu. An `--autoplay` mode plays a scripted
   player vs the AI for several minutes with no errors. Screenshots exist of
   the menu, the match, a trench being dug, and the end screen.
5. In an AI-vs-AI match at peak (up to 240 soldiers), sim tick P99 is under
   10 ms on the M3.
