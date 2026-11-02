# Slice S3c-armour: bail-out, abandoned tanks, AI armour habits, map bias

**Context (2026-11-01).** S3b (physical damage, armour balance, trench
pinning) is done and verified on `s3b-balance` (`b3a8793`). The owner chose
a short armour follow-up before S4 (unit variants). Branch `s3c-armour`.

Read first: `docs/slices/S3b-log.md` ("Fix stage": final targets, known
gaps), `docs/slices/S3b-balance.md` (damage audit, tuning surface),
`docs/modding/tuning.md`, `docs/TODO.md` (S3b-*, S3c-*),
`docs/slices/S1b-hud.md` (hotkeys, feed, minimap).

**Owner decisions in force (2026-11-01):**
1. **Bail-out (S3b-5 (B)).** Surviving crew leave a tank that is burning,
   at risk of its ammunition cooking off, or down to too few crew / lost
   its gun or engine, as a small crew squad of real soldiers with their
   own loadout. They exit physically at hatches and can be shot. An
   abandoned (not destroyed) tank stays and can be re-crewed. Re-crew
   order with hotkey and HUD button, feed messages, a minimap/overlay
   marker; crews can walk back to base. Knock-out stats split into
   destroyed and abandoned.
2. **300 m duel (S3b-38 (A)).** Near-even at 300 m is accepted as
   historical; armour-test gets a 600 m frontal case that reports which
   side wins at range (a target only if the data supports one).
3. **US trench assaults (S3b-41 (A)).** Keep the real M1919 rate of fire;
   the AI gives an assault a second base-of-fire squad when its MG is the
   weaker; report T10 per faction.
4. (A) defaults of S3b-42 (AI rearm), S3b-36 (sticky satchel hunts),
   S3b-40 (study and fix the base-side bias if the cause is in AI or map
   data); T9 tank-HE kills and T14 parapet erosion only with a realistic
   physical cause; MG stowage on the vehicle card.

Standing rules (unchanged from S3b): physical contact only (bailing crew
are hit like any soldier); balance order realism → costs/AI → historical
options → stats as a documented last resort; every gameplay value in
`mods/ww2` TOML with unit and meaning; deterministic sim (Q16.16, no
HashMap, counter RNG per purpose, ascending-id iteration, new state saved,
loaded, hashed, validated, a test per field); `SAVE_VERSION` 10 → 11 and
`PROTOCOL_VERSION` 4 → 5 **once** for the slice; new commands in the relay
limits and `sdk/docs/net/relay-protocol.md`; every existing test, clippy
`-D warnings`, fmt, golden 46/46, validate, fortify-test and armour-test
hard checks keep passing.

## Baseline (measured on `s3c-armour` = S3b Fix data + report-only counters, `de927eb`)

Commands (release build, an 8-core Mac, 6–7 matches at once):
`skirmish --ai-vs-ai --seed 1 --seeds 20 --both-bases --summary
--threads 1 --jobs 6` (the 40 standard matches; seeds 1–40 for 80 where
marked), `armour-test --stress --seeds 10`, `fortify-test --seeds 10`.
The new counters (`sdk/crates/cli/src/s3c_tally.rs`, commit `de927eb`)
read the world after each tick and change nothing: the 40 match hashes
are identical before and after them.

### AI vs AI (40 matches, seeds 1–20 × both base orders)

| | Germany | USA | Both |
|---|---|---|---|
| Wins (of 40) | 17 (42.5 %) | 21 (52.5 %) | 2 time-limit draws at 30:00 (seed 5 and 8 swapped) |
| Wins from base 0 / 1 | 8 / 9 | 9 / 12 | base 0 17, **base 1 21 of 38 (55 %)** |
| Tanks lost / bought | 48 / 121 | 69 / 134 | |
| Enemy tanks knocked out by gun / rocket | 62 / 7 | 44 / 4 | **rockets 11 of 117 (9.4 %)** |
| Rockets fired / hit / pen (matches with any) | 64 / 42 / 41 (19) | 92 / 53 / 51 (18) | rockets in 30 of 40 matches, **7 of 10** normal-base |
| Satchels thrown at vehicles (matches) | 1 (1) | 3 (2) | **0 of 20 normal-base matches** |
| Knock-outs by cause | | | 117: ammo fire 54, rack 23 (20 %), crew 38, engine fire 2 |
| Match length | | | 11.9–30.0 min, median 16.5, 5 over 25 |
| HE share of kills | | | mean 37.1 %, max 52.3 % |
| Peak tick p99 (parallel) | | | 0.79 ms |

Over 80 matches (seeds 1–40 × both): Germany 31, USA 47, 2 draws; base 1
won 45 of 78 (57.7 %); 221 knock-outs, 16 by rockets (7.2 %); satchels at
vehicles in 4 of 80 matches, 0 of 40 normal-base.

### New counters (report only)

| Counter | Germany | USA |
|---|---|---|
| Tank returns into the own base after leaving it (low on ammo) | 9 (0) | 19 (0) |
| Tank time with every limited MG dry (with < 1/4 of stowage) | 0.2 % (4.0 %) | 0.0 % (0.0 %) |
| Tank time with HE at the reserve (6) or below | 1.6 % | 0.1 % |
| Satchel hunts seen | 52 | 44 |
| ... ended: thrown / wiped / target gone / target moving / target unseen / called off / running at the end | 0 / 3 / 1 / 7 / 15 / 26 / 0 | 1 / 1 / 1 / 12 / 8 / 19 / 2 |
| ... leader's mean distance to the spot at the start | 109 m | 112 m |
| Living crew the tick before a knock-out: ammo fire (exact) | 2.82 | 3.16 |
| ... rack detonation (upper bound: the hit's own kills count as alive) | ≤ 3.44 | ≤ 4.21 |
| ... crew knock-out (by rule) | ≤ 1 | ≤ 1 |

Readings:
- **Rearm (S3b-42).** With the real stowage of S3b Fix, AI tanks almost
  never run dry: a tank lives about 12 minutes and spends 0.2 % of it with
  both MGs dry and 1.6 % (Germany) with HE at the reserve. The 28 base
  returns are all damage retreats. AI rearm is cheap to add and right to
  have, but it is not a balance lever. (A low-stowage probe, 150 rounds a
  mount, gave 43 % dry time: the counter works.)
- **Satchel hunts (S3b-36).** 96 hunts, 1 throw. 45 hunts end with the
  target still there, slow and seen (`called_off`: the plan drops the hunt
  because `slow_since` resets on any brief movement or the vehicle leaves
  `satchel_hunt_m` of the squad), 23 end with it unseen, 19 with it
  moving again. Hunts start 110 m out, about 40 s of walking.
- **Bail-out.** Ammo fires, half of all knock-outs, kill about three
  living crewmen each today (2.8–3.2 alive when the fire burns out). That
  is the population bail-out saves.

### Base-side bias (S3b-40): the cause is the map

Capture timeline (80 matches, `s3c` rows): each side takes its near point
first in every match (B at 104 s, D at 105 s), its second point at about
the same time (A 262 s, E 256 s), and C, the value-2 point, first in 42
vs 38 matches. Base 0 holds 2.36 points at 5 min vs 2.17; base 1 pulls
ahead later (2.58 vs 2.42 at 15 min, 2.75 vs 2.25 at 20 min) and holds C
40.2 % of the time vs 32.6 %. The AI's capture order is symmetric.

The difference is in tank losses: **base 0 lost 134 tanks of 250 bought,
base 1 87 of 249**; base 1's guns knocked out 126 tanks, base 0's 79.
Knock-out positions (`s3c.ko_spots`, the wreck and the muzzle of the
enemy tank that fired at it within 3 s):

| Tanks of | Knocked out near B / C / A / D / E / open | Shooter found | Range (median) | Shooter's muzzle above the wreck (median) |
|---|---|---|---|---|
| base 0 | **64** / 44 / 8 / 6 / 5 / 7 | 121 | 297 m | **+10.0 m** (88 of 121 more than 4.5 m higher) |
| base 1 | 3 / 31 / 18 / 25 / 0 / 10 | 77 | 276 m | +2.0 m |

Base 0's tanks die at B, shot from above at ~300 m: from the C ridge. Map
data (`mods/ww2/defs/maps/kessel_ridge.toml`, sampled through the sim's
own terrain, a scratch probe):

| Point | Height | Mean height 100 m around | Its point-mirror (1024 − x, 1024 − y) |
|---|---|---|---|
| A (220, 700) | 27.5 m | 27.7 | E side: 22.5 |
| B (300, 300), base 0's near point | **16.7 m** (a hollow, 1.9 m below its ring) | 18.6 | D: 22.6 |
| C (512, 512), value 2 | 32.3 m | 27.6 | itself |
| D (724, 724), base 1's near point | **22.6 m** (flat) | 22.4 | B: 16.7 |
| E (800, 320) | 23.2 m | 24.6 | A side: 27.9 |

The points, bases and woods are point-symmetric about the centre to within
4–6 m, but **the one `[[terrain.valleys]]` band runs only across the south
(y 280 → 340)** and puts B, base 0's near point, in a 5 m deep valley
15.6 m below C, while D is only 9.7 m below C. The terrain noise
(`seed = 1944`) is not symmetric either (C is seen from 90 % of the
ground within 150 m on base 0's side, 59 % on base 1's, which also has
twice the wood). The AI is not the cause: priorities are
`value·100 − distance/8` (`crates/sim/src/ai.rs:880`) and the points'
distances to their bases match within 5 m; ties break by point index but
the noise (±40 at normal) dominates. The spawn ring
(`crates/sim/src/game.rs:1038` `spawn_spot`) uses absolute angles (spot k
at k·60° from east for both bases), so the same squad id spawns up to
20 m nearer the front at one base than the other: real but small.

**Probe:** the same 80 matches with a mirrored valley band added (override
`valleys = [south, north mirror]`): base 1 wins 42 of 80 (52.5 %, was
57.7 %), tanks lost by base 0 / 1 **110 / 102** (was 134 / 87), Germany
34, USA 46.

### armour-test (seeds 1–10 × both factions, `--stress`): all hard checks pass

| Case | Baseline |
|---|---|
| 3a 300 m frontal | Panzer IV 4, Sherman 12, no knock-out 4 of 20; KOs crew 6, rack 8, ammo fire 2 |
| 3b / 3c / 3e | 6 / 14; flanker 20 / 20; 20 / 20 |
| 3d crossing | 60.0 % of stationary |
| 4 AT team | flank disabling-hit rate 45.8 %; front 27.8 %; HE variant 23.6 % |
| 5b tank on Suppress | pinned 54.6 % + ducked 42.2 %; kills 0 (T9) |
| 6 tank + AT team | disabled 16 of 20 (80 %) |
| 7 supported assault | attackers 39 vs 160 without the tank; defenders 100 % |
| 8 satchels | 40 m with rifles: 7 of 20 disabled, engineers lost 100 of 100 |
| 9 tank HE | trench 17 vs open 151 men; T14 lip 0.033 m per burst within 1 m |
| 14 stress | p99 1.17 ms |

### fortify-test (10 seeds): PASS

Rifles trench 0 vs open 160; mortar 14 vs 120 (11.7 %). T10 (MG +
rifles from 150 m) pooled: defenders lost 85.6 %, attackers 57.5 %
(missed). **USA attacking (M1919) vs a German trench: attackers lost
73.8 %, defenders 78.8 %. Germany attacking (MG 42): attackers 41.2 %,
defenders 92.5 %.** Unsupported rush 100 %. With a defending MG: attackers
98.8 %, defenders 0.9 % (report).

## History: crews, hatches, weapons, survival

| | Panzer IV Ausf. H | M4A3 (75) (dry stowage, 1943–44) |
|---|---|---|
| Crew | 5: commander, gunner, loader (turret); driver, radio operator / hull MG (hull) | 5: commander, gunner, loader (turret); driver, assistant driver / bow MG (hull) |
| Hatches | commander's cupola (single hatch on the H); a two-piece side door each side of the turret (gunner left, loader right; behind the turret skirts on the H); driver's and radio operator's hatches in the superstructure roof | commander's hatch over the turret's right; an oval loader's hatch on the left (production from late 1943; earlier hulls: the loader leaves through the commander's); driver's and assistant driver's hatches in the hull roof; an escape hatch in the hull floor behind the assistant driver (all production Shermans) |
| Crew weapons | pistols (P 08 / P 38) for each man; an MP 40 stowed in the fighting compartment | M1911A1 pistols; M3 submachine guns stowed in the tank (two in the later vehicles; a Thompson earlier) |

Survival: Dupuy (*Attrition*, 1990, pp. 80–81, a sample of 898 US First
Army tank losses, June 1944 – May 1945) found about **one crew casualty
(killed or wounded) per tank lost**; 40 % of the lost tanks burned, with
1.28 crew casualties each against 0.78 for those that did not; infantry
rockets caused 13 % of the losses but 21 % of the crew casualties.
Zaloga, quoting 12th Army Group, gives about one killed and one wounded
per knocked-out Sherman. So a five-man crew usually had three or four men
get out. Crews bailed out of burning tanks within seconds; tanks with
crews dead or wounded, guns knocked out or tracks broken under fire were
abandoned and often recovered later.

Sources: Dupuy Institute, "U.S. Tank Losses and Crew Casualties in World
War II" (2016, summarising Dupuy, *Attrition*);
theshermantank.com, "The small arms of the US Army tanker" and the
escape-hatch notes; Jentz & Doyle, *Panzer Tracts 4-3* (already cited in
the Panzer IV def) for the layout. The hatch and stowage details above are
from secondary sources and model references; exact small-arms counts are
an owner / citation task (S3c-7). Everything else here is a starting value
for the Tune stage.

## Design: bail-out, abandoned vehicles, re-crew (PART A)

### Triggers (rules `[vehicles.bail]`, every value data)

A vehicle's surviving crew bail out when, checked every tick in the
damage stage after hits and fires:

| # | Trigger | (A) default | Reasoning |
|---|---|---|---|
| 1 | Ammo fire burning (`ammo_fire > 0`) | at once (`ammo_fire_bail = true`) | propellant fires gave seconds; crews left at once |
| 2 | Engine fire not out after `engine_fire_bail_s` | 8 s (two `fire_check_s` tries) | crews fought engine fires with the extinguishers first |
| 3 | Living crew below `crew_below` | 3 (two men alive bail) | two men cannot drive, load and fire; replaces `crew_ko_below` |
| 4 | Main gun destroyed **and** immobile (track, engine or transmission out) | `useless_bail = true` | a tank that can neither fight nor leave |
| 5 | Gun destroyed **or** immobile, and penetrated again within `under_fire_s` | 10 s | "disabled under fire": the second penetration tells the crew they are a target |
| 6 | The player's (or AI's) **Abandon** order | — | genre reference; a player may save a doomed crew |

A rack **detonation** still destroys the vehicle at once with everyone in
it (no bail-out). A tank whose crew are all dead is **abandoned, not
destroyed** (an intact tank with a dead crew could be recovered; owner
question S3c-13). Fires keep burning in an abandoned tank: an ammo or
engine fire that burns out turns it into a wreck (destroyed, credited to
`last_attacker` as today).

### Exit physics

- **Hatches** are vehicle data: `[[hatches]]` with `seats = [...]`, a
  `frame` (hull or turret), `pos` (the opening, cm) and `exit_s` (time for
  one man to get out through it). Panzer IV H: cupola (commander, 3 s),
  left turret door (gunner, 3 s), right turret door (loader, 3 s),
  driver's hatch (4 s), radio operator's hatch (4 s). M4A3 (75): commander
  (3 s), loader (3 s; S3c-7 notes the earlier hull without it), gunner
  through the commander's hatch after him (3 s), driver and assistant
  driver (4 s each, roof; the floor escape hatch is used when the hull
  roof hatch is blocked by the turret gun: (A) not modelled).
- **Order:** every living crewman starts at once at his seat's hatch;
  men sharing a hatch go one after another (the commander first). A
  wounded man (HP below `wounded_hp`) takes `wounded_exit_pct` (150 %).
  While inside, a man is still a seat box and takes core, spall and filler
  hits; a detonation or a fire burning out before he is out kills him.
- **Out:** when his time is up he becomes a **soldier entity** of the
  crew squad, standing on the ground beside the hull below his hatch, on
  the side away from the last hit (`last_hit_dir`, new state; else the
  left side), `exit_clear_cm` (60) outside the hull box. For
  `dismount_s` (1.5 s) he cannot fire or move (climbing down); his capsule
  is standing, so he is exposed exactly as a man standing there. Then the
  squad's normal behaviour takes over (it goes prone if suppressed,
  returns fire with pistols and SMGs, or follows its order).
- **Suppression:** bailing men arrive with `bail_suppression` points (40,
  "crouch"): a crew that just left a burning tank keeps low. This is a
  state from the event, not damage; no suppression without real near
  misses is added later.
- **The crew squad** is a new squad of the vehicle def's `crew_squad`
  type (one per bail-out), created when the first man is out; later men
  join it. It is linked to the vehicle (`VehicleState::crew_squad`) while
  the vehicle stays abandoned.

### Crew squad defs (content)

New `SquadRole::Crew` (not buyable: validation refuses a `crew` unit in a
faction roster; the call-in panel and AI mix ignore the role). New weapon
defs `ww2:p38` (P 38, 9 mm) and `ww2:m1911a1` (.45 ACP): class `pistol`,
range 30 m, about 8 rounds a magazine, low suppression; values with
sources in their TOML comments.

| Def | men (at most) | weapon (`weapon_men`) | sidearm | speed | sight |
|---|---|---|---|---|---|
| `ww2:ger_tank_crew` "Panzerbesatzung" | 5 | MP 40 (1, the commander) | P 38 | as riflemen | 150 m |
| `ww2:usa_tank_crew` "Tank crew" | 5 | M3 SMG (2) | M1911A1 | as riflemen | 150 m |

No grenades (owner question S3c-7). Vehicle defs gain `crew_squad`.

### The abandoned vehicle

- The entity stays a `Vehicle` with `VehicleState::abandoned = Some({
  since, last_owner })`; its entity owner becomes `NEUTRAL` (a new
  reserved owner value; `vehicle_flags::ABANDONED` in the snapshot, public,
  seen by everyone like a wreck). Its squad is removed (it is nobody's
  unit); components, ammo, MG stowage, fires and seats (all empty) are
  kept.
- **Targeting:** auto-fire never picks an abandoned vehicle (it has no
  crew to fight); `AttackUnit` may (to deny a re-crew or capture), and any
  round or blast that strikes it acts as on any vehicle (physical rule).
  Its fires burn out to a wreck.
- **Spotting:** seen by everyone (it is an object, not a unit), as wrecks.
- **Pop and vehicle limit:** an abandoned vehicle counts toward nobody's
  pop or vehicle limit. Its crew squad counts its men in its owner's pop.
  Re-crewing needs room: the vehicle's `pop` minus the boarding men under
  the pop cap, and a free slot under `vehicles.limit` (else refused with
  the reason in the feed). So a player cannot exceed the limit by
  abandoning and re-crewing.
- **Cap:** at most `abandoned_cap` (16) on the map; beyond it the oldest
  becomes a wreck (counted "scrapped", not destroyed).

### Re-crew (rules `[vehicles.recrew]`)

- **Command** `Recrew { squad, vehicle }`: the squad walks to the vehicle
  (path to the side of the hull nearest it); within `reach_m` (4) of the
  hull its men board one by one, `board_s` (3 s) each, into the seats in
  seat order (driver, gunner, commander, loader, radio). Boarded men leave
  the world as soldiers and become seats with their HP (a man at 60 HP
  fills a seat at 60). Men beyond the seat count stay as the squad. The
  vehicle becomes the boarding player's when **`min_men`** (2: driver and
  gunner) are in; it gets a new vehicle squad and is a normal unit again.
- **Who:** the vehicle def's crew squad type (any faction's) is trained.
  With `any_infantry = true` (A) any infantry squad except AT teams can
  board, its men marked untrained (`Seat::trained = false`, saved):
  reload × `untrained_reload_pct` (150 %), turret traverse × 75 %, gun
  spread × 150 %. A trained crew squad later boarding replaces nobody;
  the base's crew refill (`base_crew_s`) puts trained men in empty seats
  as today.
- **Capture:** `capture = true` (A, S3c-2): an enemy squad may re-crew an
  abandoned vehicle, the genre reference's rule. The captor needs a free
  vehicle slot and pop. A captured tank keeps its type, ammo and damage;
  it is the captor's unit with the foreign type's name and a "captured"
  tag. `capture = false` restricts re-crewing to the last owner.
  Fix stage: `capture_needs_crew = 1` (A, S3c-32) lets only a tank crew
  squad capture; infantry re-crew only their own side's vehicles. Nobody
  may board a burning vehicle or one with gun and mobility out (it would be
  abandoned again at once; refusals "on fire", "gun and tracks or engine
  out").
- **Interrupted boarding:** if the squad is ordered away or the vehicle
  is destroyed, men already in stay as seats (below `min_men` the vehicle
  stays abandoned and those men bail out again as the squad).
- **Repairs:** a re-crewed vehicle with broken parts field-repairs as
  today (`J`); its crew squad's men are its crew.

### Crew squads afterwards

- A crew squad is an ordinary infantry squad for orders (move, retreat,
  stance, Recrew; no dig, no throws).
- **Walking back:** `Retreat` (R) takes it to base. In the own base a crew
  squad without another order is dismissed after `crew_dismiss_s` (30 s):
  removed, pop freed, **no refund** (S3c-4 (A)). Reasoning: the base
  already refills dead crew seats for free (`base_crew_s`), so crews are
  not a resource to be banked; refunding manpower would reward losing
  tanks. Report counter `crews_home`.
- The AI (Part A minimum): its crew squads retreat to base at once (Part
  B adds re-crewing).

### Commands, saves, protocol

- `Recrew { squad: SquadId, vehicle: SimId }`; `Abandon { squad: SquadId }`
  (a vehicle squad). `PROTOCOL_VERSION` 4 → 5; the relay's
  `command_in_limits` bounds both by `MAX_UNITS`; both documented in
  `sdk/docs/net/relay-protocol.md`.
- New saved, hashed, validated state (a test per field): `Seat::trained`;
  `VehicleState::{abandoned, crew_squad, bail (per-seat exit timers),
  last_hit_dir, fire_bail (engine-fire timer), last_pen_tick}`; per-player
  counters (`vehicles_abandoned`, `vehicles_recrewed`,
  `vehicles_captured`, `crew_bailed`, `crew_out`, `crews_home`); the crew
  squad's link. `SAVE_VERSION` 10 → 11 (once). Load validation: an
  abandoned vehicle has no squad and a neutral owner; exit timers within
  the hatch's `exit_s` × `wounded_exit_pct`; `crew_squad` names a crew
  squad or is cleared.

### Events, stats, report

- `ArmourEventKind` gains `Abandoned` (component: trigger 1–6),
  `CrewOut` (one per man; seat), `Recrewed` (component: 1 own, 2
  captured). `KnockedOut` stays "destroyed".
- Knock-out stats split everywhere (skirmish summary, `PlayerArmour`,
  armour-test): **destroyed** (wreck) and **abandoned** (by trigger), and
  what became of the abandoned (re-crewed, captured, destroyed later,
  scrapped, still standing). Rockets and satchels are credited for an
  abandonment by the last hit, as for a knock-out. Crew: alive at bail-out,
  out alive, alive 60 s after getting out (`s3c` rows).

### HUD (per `docs/slices/S1b-hud.md`)

- **Feed** (own side): "Panzer IV abandoned (fire)" / "(crew losses)" /
  "(gun and tracks out)"; "Crew bailed out: 3 of 5"; "Panzer IV
  re-crewed"; "Captured an enemy Sherman"; "Our Panzer IV was captured";
  enemy: "Enemy Sherman abandoned" when seen. A refused re-crew says why
  ("Vehicle limit reached", "Pop cap", "Too few men").
- **Hotkey K** (free; owner question S3c-6): with infantry selected, K
  arms the Crew tool (click an abandoned vehicle); right-click on an
  abandoned vehicle with a crew squad selected is the context Recrew.
  The orders grid gets a "Crew" button (K) for infantry selections.
- **Bail out** button on the vehicle card (no hotkey): it asks for a
  second click within 2 s ("Click again to abandon").
- **Markers:** an abandoned vehicle is drawn on the minimap as an outlined
  (hollow) tank icon in the last owner's colour at 60 % alpha, with a
  small slash; in the world overlay it gets a hollow vehicle marker and,
  on hover, "Abandoned: Panzer IV (gun out, left track out)". A crew squad
  card shows its tank's name ("Crew — Panzer IV") and a jump button to the
  abandoned vehicle.
- **Vehicle card MG stowage** is Part B (below).

## Design: PART B (AI, map, cases)

### B1. AI rearm (S3b-42 (A))

In `vehicle_upkeep`: a tank whose limited MGs are all dry, or whose HE is
at or below `combat.suppress.tank_he_reserve`, or whose penetrator is at
or below `ai.armour.rearm_ap_below` (3), and that is not in a fight (no
known enemy vehicle within `threat_m`), retreats to base (the existing
retreat path; task `Rearm`, saved) and stays until its ammo and MG
stowage are at `rearm_full_pct` (90 %) or `rearm_max_s` (120 s) pass, then
takes its next task. Counter `rearms`. Baseline: the need is small (0.2 %
of tank time MGs dry, 1.6 % HE low); the target is that trips happen and
the low-ammo time does not grow.

### B2. Sticky satchel hunts (S3b-36 (A))

- New saved AI state `hunts: BTreeMap<SquadId, Hunt { vehicle, since,
  last_seen, last_pos }>` (at most `MAX_HUNTS` 64).
- **Start** as today (a known vehicle slow for `satchel_slow_s` within
  `satchel_hunt_m`), plus any known immobile or bogged vehicle within it.
- **Keep** the hunt on that vehicle while: it is not a wreck or
  abandoned; it was seen within `satchel_stick_s` (20 s; the squad goes to
  its last seen position meanwhile); it is not faster than
  `satchel_slow_mm_s` for more than `satchel_chase_s` (6 s) — a tank
  stopping and starting stays hunted; it is within `satchel_hunt_m` +
  `satchel_leash_m` (50) of the squad; the squad has a satchel and is not
  retreating. Hunting squads are **claimed** (no point assignment, escort
  or base-of-fire role takes them), so other tasks cannot call them back.
- **Approach:** crouched beyond 40 m, prone-crawl inside `satchel_crawl_m`
  (25) when the vehicle's hull MG arc covers the route; a smoke throw
  between the squad and the vehicle when one is carried and the squad is
  under fire (existing smoke rule, aimed by the hunt). End after
  `satchel_hunt_max_s` (120 s).
- Counters: hunts started, kept through a loss of sight, thrown, ended by
  each reason (the `s3c` classes).

### B3. Rockets and satchels share of tank losses

Bail-out is the main lever (case 6 already disables 80 % of tanks;
rocket penetrations that kill two or three crew or wreck the gun now end
the tank's fight as an abandonment). Measure after Part A; only if the
share stays under 20 %: AT hunts also go for immobile and bogged
vehicles beyond `at_hunt_m` up to 500 m, and AT teams follow an escorted
assault's own tanks within 150 m (data keys, measured one at a time).

### B4. A second base of fire (S3b-41 (A))

When the AI picks a base of fire for an assault (`ai::assault`), it
compares the base's fire weight (Σ over its men of the weapon's
`rounds_per_min × suppression`) with the strongest MG team weapon of the
enemy faction's roster (doctrine knowledge, not sight). If the own weight
is below `second_base_below_pct` (100) % of it, and at least three squads
are assigned to the point, a second squad (an MG team, else a rifle squad)
becomes a base of fire too, on the same line from a spot at least
`second_base_gap_m` (30) from the first. M1919: 500 × 14 = 7,000; MG 42:
1,200 × 15 = 18,000, so US assaults get two bases and German ones one.
Lift rules unchanged. Counter `second_bases`. Measured by T10 per faction:
fortify-test gains a row "USA attacking with two bases of fire (MG +
rifle squad suppressing)".

### B5. AI and abandoned vehicles

- **Re-crew:** an AI crew squad whose abandoned tank is not burning, has
  no known enemy soldier within `recrew_clear_m` (80) and no known enemy
  vehicle within `threat_m`, and fits the limit and pop, re-crews it
  (`Recrew`); the existing upkeep then repairs or retreats it. Otherwise
  it retreats to base (and is dismissed there).
- **Other abandoned own tanks:** the nearest idle crew squad within 300 m
  takes them on the same rule.
- **Capture:** (A) the AI does not capture in S3c (S3c-2 (B) later).
- **Denial:** an AI tank with no live target and an enemy abandoned tank
  within 300 m in view fires at it (`AttackUnit`) when `capture` is on.

### B6. Base-side bias (S3b-40)

(A) Fix the map: add the mirrored valley band to Kessel Ridge
(`from = [1024, 744], to = [0, 684]`, the point-mirror of the south one;
measured above: base 1 52.5 %, tank losses 110 / 102), and make the spawn
ring point-symmetric (spot k at k·60° measured from the base's facing to
the map centre, `game.rs` `spawn_spot`). Re-measure the knock-out spots
and the capture timeline; if one side still holds C more than 5 points
longer, (B) a `[terrain] symmetry = "point"` map key (the generator
evaluates the noise and bands on the base-0 half and mirrors them) is the
next step. Both change the map, so every skirmish hash changes (no golden
row does).

### B7. 600 m frontal duel (S3b-38 (A))

armour-test case 3f: as 3a at 600 m (both hulls square, stationary).
Report: wins per side, knock-outs by cause, first-hit time, penetrations
per plate. Expectation from the data: the PzGr 39 (≈ 120 mm at 600 m)
still beats the Sherman's glacis, the M61 (≈ 79 mm at 600 m) is just under the
Panzer IV H's 80 mm hull front (81 mm in line at 10°), so the Panzer IV should win clearly. A
target ("Panzer IV ≥ 6 of 10 at 600 m") is written only if 3f on seeds
1–10 supports it; otherwise it stays a report.

### B8. T9 (tank HE kills in a trench) and T14 (parapet erosion)

Investigate before any change; change only with a physical cause:
- **T9:** log, in case 5b, every HE burst within 3 m of the trench: burst
  height relative to the lip and the trench floor, and the fragment rays'
  elevations. A shell bursting **on the parapet crest** is above the men
  behind it: fragments going down and rearward over the lip are physical.
  If the ray model only emits rays at or above the horizontal (or starts
  them below the crest so the spoil blocks them), allowing downward rays
  from a burst above the local ground is the fix (`frag_elev_deg` range
  per burst height). If rays already cover that and men below the lip are
  simply out of the cone, document and keep "pins" (S3b-27 (A)).
- **T14:** compare the HE crater (radius, depth, rim) with a cratering
  rule for a ~0.7 kg charge in loose spoil (crater volume grows roughly
  with charge mass in a given soil; loose fill craters about 1.5–2× the
  volume of compact ground). If our spoil crater is the compact-ground
  size, scale it for spoil (`spoil_crater_pct` already exists; a radius
  share is the missing half). Else document.

### B9. MG stowage on the vehicle card

The vehicle card shows "MG 2,950" (the snapshot's `mg_rounds`), amber
under a quarter, red "MG dry" at 0; the tooltip keeps the per-mount
detail. Godot check added.

## Targets (the Tune stage measures these)

Standard set: skirmish seeds 1–20 × both base orders (40 matches, `--jobs
6 --threads 1 --minutes 45`); armour-test `--stress` seeds 1–10;
fortify-test seeds 1–10. Decisions on noisy numbers use 80 matches (seeds
1–40).

| # | Target | Baseline | How measured |
|---|---|---|---|
| T1 | Each faction wins 35–65 % of 40; **no base side > 62 %** | GER 17 (42.5 %), USA 21 (52.5 %), 2 draws; base 1 21 of 38 (55 %); 80: base 1 57.7 % | skirmish summary |
| T2 | Rockets + satchels cause **≥ 20 %** of tank losses (destroyed or abandoned, by last hit) | 11 of 117 (9.4 %); 80: 16 of 221 (7.2 %) | summary (new split) |
| T3 | A satchel thrown at a vehicle in **≥ 3 of 10** normal-base matches (≥ 6 of the 20) | 0 of 20 (4 of 80 over all) | per-match line |
| T4 | Rockets fired in ≥ 8 of 10 normal-base matches | 7 of 10 (14 of 20) | per-match line |
| T5 | AI rearm trips occur (≥ 1 low-ammo base return per 10 matches) and tank time with MGs dry or HE ≤ reserve does not rise | 0 low-ammo returns; MG dry 0.2 / 0.0 %, HE ≤ reserve 1.6 / 0.1 % | `s3c` rows |
| T6 | Bailed crews: in ≥ 70 % of abandonments with living crew at least one man gets out alive; ≥ 50 % of the men who get out are alive 60 s later (history: ~1 casualty per 5-man crew per loss) | today 0 (ammo fires kill 2.8–3.2 living men each) | `s3c` crew rows |
| T7 | Abandoned vehicles are used: ≥ 1 re-crew per 10 matches by the AI; abandoned / destroyed split reported | new | summary |
| T8 | T10 per faction: **US attackers lose ≤ 60 %**, German attackers ≤ 50 %; defenders lose ≥ 75 % pooled | USA attacking 73.8 %, Germany 41.2 %; defenders 85.6 % | fortify-test |
| T9 | Match length: no match < 10 or > 40 min, median 15–30 | 11.9–30.0 (2 at the 30-min limit), median 16.5 | summary with `--minutes 45` |
| T10 | Tick p99 at peak < 1.5 ms; stress p99 < 10 ms | 0.79; 1.17 | summary with `--jobs 3` (timing depends on parallel load: the same 160 matches gave 0.71 ms at `--jobs 3`, 1.55 ms at `--jobs 6`); case 14 |
| T11 | Base bias at its cause: tanks lost by base side within ±15 % of each other | 134 vs 87 (+54 %); mirrored-valley probe 110 vs 102 | `s3c` / JSON |
| T12 | 600 m frontal duel (3f): report the winner share; a target only if supported | new | armour-test |
| T13 | T9 kills > 0 / T14 ≥ 0.05 m per burst: only with a physical cause (B8); else documented | 0; 0.033 m | armour-test 5b, 9 |
| T14 | All S3b hard checks still pass: armour-test hard checks, fortify PASS, golden 46/46, validate; HE share < 60 % per match | pass; HE max 52.3 % | as listed |
| T15 | Determinism: hashes equal across thread counts, save/load round trip, net loopback and soaks 0 mismatches | pass | tests |

## Implementation order

### PART A: bail-out, abandoned vehicles, re-crew, HUD

1. **Content:** `SquadRole::Crew`, the two crew squad defs, P 38 and
   M1911A1 weapon defs, `crew_squad` and `[[hatches]]` on both vehicle
   defs, `[vehicles.bail]` and `[vehicles.recrew]` rules (each key with
   unit, meaning, source), schema, validation (hatch seats exist and
   cover every seat; hatch inside its frame's box or on its surface;
   crew units not in a roster), `validate mods/ww2`, `tuning.md` rows.
2. **State:** the new seat, vehicle and player fields; `NEUTRAL` owner;
   save/load/hash/validate with a test per field; `SAVE_VERSION` 11.
3. **Triggers** in `damage.rs` (bail table above); `crew_ko_below`
   replaced by `crew_below` bail (the key is removed; the tests that set
   it to 0 to keep a crewless tank alive, `crates/sim/tests/anti_tank.rs`
   and `vehicle_orders.rs`, set `crew_below = 0` instead); rack
   detonation unchanged; fire
   burn-out of an abandoned vehicle → wreck.
4. **Exit:** hatch timers, soldiers spawned of the crew squad, dismount
   state, the inside-crew-still-hit rule; tests: a burning tank's crew are
   out before `ammo_fire_s`; a man still inside when the fire burns out
   dies; exits on the side away from the hit; a bailing man is hit by a
   round crossing his capsule.
5. **Abandoned vehicle:** owner `NEUTRAL`, squad removed, not
   auto-targeted, `AttackUnit` allowed, cap, spotting; tests.
6. **Re-crew and Abandon commands:** walk, board, seats, `min_men`,
   capture flag, untrained penalties (gunnery reload, traverse, spread),
   limit and pop checks with refusal reasons; `PROTOCOL_VERSION` 5, relay
   limits, protocol doc; tests incl. a net loopback with both commands.
7. **Crew squads:** dismissal in base; Part-A AI: crews retreat.
8. **Snapshot, bridge, HUD:** `ABANDONED` flag, crew squad role, events,
   feed lines, K hotkey and Crew button, Bail-out button with confirm,
   minimap and overlay markers, crew card; Godot checks (test_match_view
   and a data-reload check); settings.cfg untouched (detach the Settings
   node in any test that toggles settings).
9. **Reports:** destroyed / abandoned split and the abandonment fates in
   `skirmish` (summary and JSON) and armour-test; crew rows (`s3c`).
   armour-test **case 15 "bail-out"**: (a) a tank set on ammo fire: crew
   out before the burn-out, exits by hatch; (b) crew losses to two: bail;
   (c) gun + track out: bail; (d) a crew squad re-crews its tank; (e)
   riflemen re-crew (untrained reload measured); (f) an enemy crew
   captures it with `capture` on, is refused with it off. Hard checks for
   the mechanics, report rows for timings.
10. **Measure Part A** (the standard set) and commit.

### PART B: AI, map, cases, card

1. **B1** AI rearm; **B2** sticky satchel hunts; **B3** rocket levers only
   if T2 is still missed after Part A; **B4** second base of fire (+ the
   fortify row); **B5** AI re-crew, crew retreat, denial; AI counters in
   the report.
2. **B6** base bias: mirrored valley + symmetric spawn ring; measure T1
   and T11 over 80 matches.
3. **B7** armour-test 3f (600 m).
4. **B8** T9/T14 investigation; change only with a physical cause.
5. **B9** MG stowage on the vehicle card.
6. Measure everything against the Targets; the Tune stage follows.

## Owner questions (work proceeds on (A); also in `docs/TODO.md`)

1. **S3c-1 Bail triggers.** (A) The six above (ammo fire at once, engine
   fire after 8 s, fewer than 3 crew, gun + mobility out, disabled and
   penetrated again within 10 s, manual Abandon). (B) Only fires and crew
   losses.
2. **S3c-2 Capture.** (A) On (`vehicles.recrew.capture = true`): enemy
   crews or infantry may re-crew an abandoned tank; the AI does not
   capture in S3c. (B) Off. (C) On and the AI captures too.
3. **S3c-3 Who can re-crew.** (A) Crew squads (trained) and any infantry
   but AT teams (untrained: reload 150 %, traverse 75 %, spread 150 %).
   (B) Crew squads only.
4. **S3c-4 Crews in base.** (A) Dismissed after 30 s in base, no refund.
   (B) They stay as squads until the player dismisses them. (C) Refund
   their men's value.
5. **S3c-5 Pop and limit.** (A) An abandoned tank counts toward nobody;
   re-crewing needs a free vehicle slot and pop. (B) It keeps counting
   toward its last owner's limit until destroyed or captured.
6. **S3c-6 Keys.** (A) K = Crew tool / re-crew (right-click context with
   a crew squad); Bail out only as a card button with a confirm click.
   (B) Other keys (U, I).
7. **S3c-7 Crew loadouts and hatches.** (A) Germany: MP 40 (commander) +
   P 38 pistols; USA: two M3 SMGs + M1911A1 pistols; no grenades; M4A3 (75)
   with the loader's hatch (late-1943 hull). Exact counts are a citation
   task. (B) Pistols only.
8. **S3c-8 Base bias.** (A) Mirror the valley band (map data) and make
   the spawn ring symmetric (code). (B) Point-symmetric terrain flag for
   the whole map. (C) Leave the map.
9. **S3c-9 Shooting abandoned tanks.** (A) Not auto-targeted;
   `AttackUnit` can; the AI denies enemy abandoned tanks in view when it
   has nothing else to shoot. (B) Auto-targeted like live tanks.
10. **S3c-10 AI rearm thresholds.** (A) Both MGs dry, HE ≤ reserve or AP
    ≤ 3, and no enemy vehicle near; back when 90 % full or after 120 s.
    (B) Only on dry MGs.
11. **S3c-11 Second base of fire.** (A) When the base's fire weight
    (rpm × suppression) is below the enemy MG team's and ≥ 3 squads are
    assigned. (B) Always two bases for every assault with ≥ 3 squads.
12. **S3c-12 Crew all dead.** (A) The tank is abandoned (intact, can be
    re-crewed). (B) It is destroyed as today.
13. **S3c-13 Match-length runs.** (A) Tune measures with `--minutes 45`
    so T9 can see matches over 30 min (the CLI default stops at 30 and
    counts a draw). (B) Keep 30 and count limit draws as "over".

## Tune results (Tune stage, 2026-11-02)

Three data changes in `mods/ww2/defs/rules/skirmish.toml`, each with its
measurement in the TOML comment:

| Key | Was | Now | Why |
|---|---|---|---|
| `ai.mix_at_pct` | 10 | 20 | AT teams (180-200 MP) reached a tank in only 11 of 20 normal-base matches once tanks fought at 300-600 m (S3c-26) |
| `ai.assault.at_hunt_clear_m` | 40 | 20 | an AT team would not leave while any enemy soldier stood near its route, so tanks shelled the point from 160 m |
| `ai.assault.satchel_hunt_m` | 150 | 250 | tanks halt at `tank_standoff_m` 160 m from a point, engineers dig in 10 m in front of it: a halted tank stood just outside 150 m |

No sim code changed; the armour-test and fortify-test results are identical
to Part B (the AI values do not enter the staged cases). Measured with
`--minutes 45`: the standard set (seeds 1-20 x both bases = 40 matches) and
the 160-match set (seeds 1-80 x both bases), which is the one decisions
use; the standard set alone is too noisy for faction wins (11 G / 29 U there,
72 / 88 in 160).

What was tried and dropped (160 matches unless noted; base 0 / base 1
wins, tank losses by base side):

| Variant | Result |
|---|---|
| `mix_at_pct` 20 only | rockets fired 396, matches with a rocket 70 %, rocket KOs 6.9 %; base wins 82/78, losses 358/382 |
| `mix_at_pct` 20 + `at_hunt_m` 600 | rocket KOs 10 %, matches with a rocket 75-80 %, but base 0 won 90 of 160 (56 %) and base 1 lost 19 % more tanks (328 / 392); `at_hunt_m` 600 alone 93/67 |
| `mix_at_pct` 25 + `at_hunt_m` 500 | rocket KOs 8.8 %, no more matches with a rocket (66 %), base 89/71, losses 347/392 |
| `mix_at_pct` 30 (40 matches) | no more rocket kills than 20 |
| `tank_standoff_m` 120 | inside the Panzerschreck's 120 m engage range, contradicts the rule's own reasoning; rocket KOs 11.6 % vs 10.2 % in 80 matches, within noise |
| a satchel for every engineer (80 matches) | 5 throws in 5 matches, base 0 won 60 % |
| `rearm_ap_below` 8 (80 matches) | results identical to 3: no tank is down to 8 AP rounds alive |
| `symmetry = "point"` terrain (80 matches, with `mix_at_pct` 20 + `at_hunt_m` 600) | base wins 37/43, but Germany won 25 of 80 (31 %), outside T1 |
| `satchel_hunt_m` 350 | 6 throws, no more than 250 |

### Final numbers

| # | Target | Baseline (Part B) | Final | Status |
|---|---|---|---|---|
| T1 | each faction 35-65 %; no base side > 62 % | 80: G 35 / U 45, base 1 41 of 80 | 160: G 72 (45 %) / U 88, base 0 88 / base 1 72 (55 %); standard 40: G 11 / U 29, base 0 21 / base 1 19 | met on 160; standard set misses on faction wins (noise: 2.6 sigma from the 160-match rate) |
| T2 | rockets + satchels >= 20 % of tank losses | 21 of 397 (5.3 %) | 160: 70 of 725 (9.7 %); standard 40: 18 of 187 (9.6 %) | **missed** (S3c-28) |
| T3 | satchel thrown at a vehicle in >= 6 of 20 normal-base matches | 1 of 40 | 5 of 80 normal-base matches (1.25 per 20); standard 40: 2 of 20 | **missed** (S3c-30) |
| T4 | rockets fired in >= 8 of 10 normal-base matches | 22 of 40 (55 %) | 60 of 80 (75 %); standard 40: 14 of 20 | **missed**, narrowly (16 of 20 wanted) |
| T5 | rearm trips occur (>= 1 low-ammo return per 10 matches), low-ammo time does not rise | 2 trips in 80 | 5 rearm trips, 1 low-ammo base return in 160; MG dry 0.0 %, HE at the reserve 0.1-0.2 % (did not rise) | **missed** (S3c-30): tanks are lost before they run low |
| T6 | bailed crews survive | 80-85 % alive after 60 s | 99-100 % of abandonments with a man out; 80 % (G) / 82 % (U) alive after 60 s | met |
| T7 | AI re-crews | 15 in 40 matches | 160: 222 re-crew orders, 56 tanks re-crewed (G 19, U 37) | met |
| T8 | US attackers <= 60 %; defenders >= 75 % | two bases 37.5 % | unchanged: two bases 37.5 %, one base 66.2 %; Germany attacking 36.2 %; defenders 86-100 % | met (two-bases) |
| T9 | no match < 10 or > 40 min, median 15-30 | 10.2-32.8 | 10.5-31.7 min, mean 17.8 | met |
| T10 | tick p99 at peak < 1.5 ms; stress p99 < 10 ms | 0.79 / 1.72 | 0.945 ms (160 matches, parallel) / stress p99 1.565 ms (armour-test case 14) | met |
| T11 | tanks lost by base side within +-15 % | 190 vs 207 | 160: 345 vs 380 (+10 %) | met |
| T12 | 600 m duel reported | Panzer IV 14 of 20 | unchanged | met |
| T13 | T9 kills > 0, T14 >= 0.05 m | 4 of 160; 0.035 m | unchanged (S3c-22, S3c-23) | documented |
| T14 | all hard checks pass; HE share < 60 % per match | HE max 61.1 % | hard checks pass; HE mean 43.8 %, over 60 % in 1 of 160 matches (65.5 %, an 11.5-min match with 2 tank losses) | hard checks met; HE share missed by 1 match (S3c-31) |
| T15 | determinism | pass | `cargo test --workspace --release -- --include-ignored`: 456 passed, 0 failed (soaks included); `ai_assault` moved to seed 50 | met |

### Why T2 stays near 10 %

Rockets and satchels hit and penetrate (304 penetrations in 160 matches),
but a penetration makes the crew leave in about one case in 4.3: the
others disable a gun or a track and the tank stays in the fight. More AT
teams, longer hunts and a looser clearance each moved the rocket share by
about three points, and the cheaper variants moved it by nothing. The
20 % target came from a measurement before the shell-lifetime fix
(19.8 %); with shells now flying their full range, tanks kill tanks from
300-600 m, and the model has no AT guns, mines or air, which caused most
real tank losses. Options and the proposed revision are in TODO S3c-28
(target 10 %), the Panzerfaust in S3c-29 (S4 variants), and the other
misses in S3c-30 and S3c-31.

## Slice log

Stage results go in `docs/slices/S3c-log.md` (created by the next stage).
