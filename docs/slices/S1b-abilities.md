# Slice S1b, part 3: grenades, smoke and satchel charges

**Owner request (2026-10-31):** "add the buttons for throwing grenades and
other stuff." The buttons sit in the ability row of the orders grid (see
`S1b-hud.md`). This file specifies the sim side. The S1 hard rules apply:
everything is a deterministic `Command`, there is no file IO in the sim,
data lives in TOML, and all state is saved, hashed and tested.

**Why it matters for play.** In S1, trenches stop all rifle fire: the
fortify-test shows 0 trench casualties against 80 in the open. Grenades are
the close-range answer to a dug-in squad. The attacker has to get within
throwing range, about 30 m, under fire. That matches the design intent:
trenches are strong, but not invulnerable.

## Items

Defined in `mods/ww2/defs/items/*.toml`. Each squad type has a loadout in
its unit TOML. Starting values, for tuning later:

| Item | Who carries it | Count | Throw range | Effect |
|---|---|---|---|---|
| Frag grenade (Stielhandgranate / Mk 2) | rifle and engineer men | 1 each; squad leader 2 | 5–30 m | HE: lethal about 2 m, damage to 6 m, suppression to 10 m, small crater (0.2 m) |
| Smoke grenade (NbHgr 39 / M15-style) | rifle and engineer squad leaders | 1 | 5–30 m | cloud blocks spotting |
| Satchel charge | engineer squads | 1 per squad (carried by one man) | 3–8 m (placed or thrown) | HE: lethal about 4 m, damage to 10 m, suppression to 18 m, big crater (1.5 m) that cuts trench lines and parapets |

MG and mortar teams carry nothing in S1b.

**Resupply.** A soldier inside his own base zone gets one item back every
20 s, up to his loadout.

## Mechanics

**Throw.**
- The thrower is eligible if he is alive, not pinned, carries the item, and
  the target is within his range.
- He stops, turns, and after a 1.0 s wind-up the item flies on a ballistic
  arc. Flight time follows from distance, about 1 s at 30 m.
- Scatter is 6 % of the distance, using the counter RNG keyed by the
  thrower and his throw count.
- The fuse runs out on landing: 4 s total from the start of the throw for a
  frag, 5 s for a satchel. There is no bouncing or rolling in S1b.
- The arc can hit terrain on the way; the item then lands where the arc
  meets the ground. A grenade lobbed over a parapet lands inside the trench.

**HE.** Frags and satchels reuse the mortar explosion code:
- Damage falls off with distance.
- Damage is blocked by terrain between the blast point (+0.3 m) and the
  body.
- They create a crater.
- A frag landing in a trench floor hits the men in that trench section. The
  trench walls only shield the men on the far side of a bend or traverse.

**Smoke.**
- A cylinder grows from 0 to 8 m radius over 3 s and holds for 25 s. Then
  it thins over 5 s, during which spotting rays are blocked with falling
  probability, using the counter RNG.
- A spotting ray that passes through any active cloud is blocked.
- Bullets are not blocked, but soldiers only fire at spotted targets, so
  smoke stops aimed fire.
- Smoke does not drift with wind in S1b.
- Clouds live in a fixed-capacity, sorted list (cap 64). The oldest cloud
  is dropped first.

## Commands

| Command | Effect |
|---|---|
| `Throw { units, item, target }` | From the sent units, the sim picks one eligible thrower: the one nearest the target, ties broken by lowest id. One item per command. If nobody is eligible, the command is ignored. |
| `ThrowEach { units, item, target }` | Optional, Shift+key: every eligible unit throws one item at the target, each with its own scatter. |

- `units` is 1–120 ids, with the same validation as the `*Units` orders.
- A whole-squad selection sends its member ids too; there is no
  squad-level throw.
- A thrower given a throw becomes detached, as with any unit order, and
  stays in place afterwards.

## AI

The AI keeps planning by squad. It gets three new behaviours:
- **Frag:** throw at spotted enemies in a trench, or in a group of at least
  3 men, within 30 m of one of its men. At most one throw per squad every
  10 s.
- **Smoke:** when one of its squads is pinned while moving or
  attack-moving, its leader throws smoke halfway towards the nearest seen
  enemy.
- **Satchel:** engineers throw at enemy trench sections within 8 m.

Difficulty scales how quickly the AI reacts, using the existing reaction
delay.

## Snapshot and presenter

**Snapshot additions:**
- per soldier: item counts;
- per squad: total frag, smoke and satchel counts, for the ability buttons;
- items in flight: position and kind;
- active smoke clouds: position, radius, density;
- explosions: they already exist; add a kind for frag and satchel.

**Presenter:**
- Ability buttons: G frag, T smoke, F satchel. Each shows the count for the
  current selection and is greyed out when the count is 0.
- The armed tool shows a range ring around the selected men, and a target
  cursor that turns red out of range.
- Left-click throws. Shift+click throws with all eligible men.
- Effects: thrown-item streak, smoke volume (billboard particles, pooled),
  blast effects scaled by kind.

## Done when

1. Tests, clippy and fmt pass. They include:
   - unit tests for thrower choice, range and validation, resupply, smoke
     ray blocking and cloud expiry;
   - determinism with throws in the command log, at 1/2/3/8 threads and
     through save → load → continue.
2. `frontline-cli fortify-test` gains a **grenade assault** case: a rifle
   squad advances on a dug-in rifle squad and throws frags from 25 m. The
   defenders must take clearly more losses than they do from rifle fire
   alone. Report the numbers; no fixed threshold yet.
3. A smoke test: a squad behind smoke is not spotted, and so not fired on,
   by an MG at 100 m.
4. `--autoplay` throws a frag into an enemy trench and a smoke screen, with
   screenshots.
5. Sim tick P99 at peak stays under 10 ms.
