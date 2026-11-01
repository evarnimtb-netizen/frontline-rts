# Slice S3: tanks and armour

**Context (2026-11-01):** S1 (skirmish vs AI with trenches), S1b (soldier
control, grenades, smoke, satchels, HUD v2) and S2 (2-player lockstep) are
done on `s2-multiplayer`. S3 adds the first vehicles: one medium tank per
faction, an infantry anti-tank team per faction, penetration against
sloped armour, component and crew damage, and the interaction with
trenches. It must play in skirmish vs AI and in 2-player net matches.

This is revision 2. Three reviews (determinism and performance, gameplay
and balance, scope and presenter) were checked against the code and the
design doc. The main changes: tank destroyers, sticky satchels, bail-out
crews and AI ditch digging moved to S3b; engagement ranges match spotting;
tanks are blocked by AT-ditch geometry, not by any deep hole; several sim
details are pinned down so they can be built deterministically.

Work proceeds on the (A) defaults in the owner questions at the end until
the owner answers. **Every number in this file is a starting value for
tuning.** Armour and gun figures are approximate public historical values.
The build uses the numbers here. Citations are an owner or M2 task;
`sources` may hold a placeholder in S3.

Read `SYSTEM_DESIGN.md` §2.2 (budgets), §5.5, §5.7 (ballistics and
armour), §5.9 (vehicle model), §5.10 (terrain), §5.12 (direct control,
for later compatibility), §7.4 (def examples), §8.1 (`ballistics` CLI),
§12, §14 and §15 M2. S1, S1b and S2 rules apply; this file wins where
they differ.

This is our own game. Unit names are historical; costs, pop, rules and UI
are ours. Nothing here copies another game's stats, UI or system names.

## Hard rules

- **All gameplay is in the deterministic sim**, driven only by
  `Command`s. Q16.16 fixed point, counter RNG, no floats, no HashMap, no
  wall-clock, ascending-id iteration.
- **Trig:** only `Fx::sin_cos` and `Fx::atan2`; angle differences through
  `Fx::wrap_angle`. No `acos`, `asin` or `tan`. Impact angles are cosines
  from dot products. Products of two world coordinates saturate Q16.16,
  so box-space and range math subtract a local origin first or work in
  i64 (`length_sq_raw`, `sqrt_q32`), as `seg_seg` does.
- **Lockstep-safe.** New commands bump `PROTOCOL_VERSION` (2 → 3); new
  state bumps `SAVE_VERSION` (8 → 9). The relay's `command_in_limits`
  becomes an exhaustive match with no `_ =>` arm, and every new variant
  is bounded by `MAX_UNITS` (120) ids and `MAX_DIG_POINTS` (16) points.
- **Data in TOML** under `mods/ww2/defs/`, resolved by
  `sdk/crates/content` into `MatchSetup`. The content crate has no trig
  and cannot depend on the sim (§7.9), so `MatchSetup` carries angles as
  integer degrees. The sim builds derived tables from them (below). The
  sim does no file IO.
- **Every new piece of state is saved, loaded, hashed**, checked by
  `MatchState::validate` on load, and covered by the determinism tests.
  Derived caches (vehicle index, box frames, armour tables, the vehicle
  block layer) are not saved; they are rebuilt on load before the first
  stage that reads them.
- **One ballistics function.** The penetration math lives in
  `crates/sim/src/armour.rs` as pure functions. The CLI and the golden
  tests call those same functions. No second copy of the math.
- **§5.12-ready.** Vehicles move only through `drive(state, throttle,
  steer)` with the §5.12.3 input domain (i8 throttle and steer), and aim
  only through a rate-limited `lay_gun(yaw, pitch)`. The RTS path
  follower and the AI gunner produce those inputs, so M5's `MoveInput`
  and `AimInput` plug into the same code. A shot leaves along the gun's
  actual laid direction plus spread, never along the solution.
- Presenter rules from §4.1: no Node per vehicle, MultiMesh batches, one
  buffer upload per batch per frame.

## Scope

**In:**
- Germany: Panzer IV Ausf. H, Panzerschreck team (2 men).
- USA: M4A3 (75) Sherman, Bazooka team (2 men).
- Vehicle model: hull and turret as oriented boxes; per-face armour
  plates with thickness and slope; crew seats; components (engine, two
  tracks, main gun, turret ring, ammo rack).
- AP (with HE filler), HE and HEAT ammunition. Penetration vs range,
  ricochet, spall against internal components and crew. Tank HE against
  infantry. Coaxial and hull MGs.
- Fire, ammo-rack detonation, field and base repair, wrecks.
- Vehicles and terrain: slope limits, woods, trench crossing with a bog
  risk, a new **AT ditch** dig profile that blocks vehicles, wrecks as
  cover and obstacles, hull-down by real geometry.
- Infantry vs tanks: AT teams; satchels and blasts on the ground that
  break tracks.
- Escort (infantry following a tank as moving cover).
- Spotting of vehicles; vehicles as observers.
- AI: buying, escorting, facing, AT teams, satchel throws at slow tanks,
  repair, retreat.
- New commands, HUD v2 additions, hotkeys, placeholder art and sound.
- `frontline-cli ballistics` with golden tests, and `frontline-cli
  armour-test`.

**S3b (next slice, owner question 1):**
- Tank destroyers: Jagdpanzer IV (casemate, `hull_assist`) and M10
  (open-top turret, frags and small arms into the opening).
- Satchels that stick to vehicles; `Throw` at a unit.
- Bail-out: survivors leave as a crew squad.
- AI AT-ditch digging and TD overwatch.
- Vehicle hull-down pits (§5.10.3).

**Out:**
- Direct control of vehicles (M5).
- Armour meshes and the per-hull triangle BVH (path below).
- Towed and static AT guns. They need towing, pushing and gun shields: a
  slice of their own.
- Halftracks, trucks, wheeled vehicles, transport, mount and dismount.
- Re-crewing or capturing vehicles. Crew swaps between vehicles.
- Mines, wire, hedgehogs, dragon's teeth.
- Smoke dischargers, APCR/HVAP, ricochets as new projectiles, optics,
  unbuttoning, drag on shells.
- Crushing infantry. Vehicles do not deform terrain in S3.
- Vehicles capturing or contesting points (infantry hold ground).
- Engine-noise knowledge map (§5.6 sound events).
- Buildings, USSR, Luau, mod UI.
- Fixing the S1 map bias and mortar dominance. S3 reports them and flags
  if it makes HE dominance worse; it does not fix them.

## Units

### Tanks

| | Panzer IV H | M4A3 (75) |
|---|---|---|
| Cost (manpower) | 640 | 560 |
| Pop | 12 | 12 |
| Call-in | 45 s | 45 s |
| Crew | 5 | 5 |
| Speed, open ground | 6.0 m/s | 6.5 m/s |
| Reverse | 2.0 m/s | 2.5 m/s |
| Hull turn | 30°/s | 32°/s |
| Turret traverse | 14°/s | 24°/s |
| Gun | 7.5 cm KwK 40 L/48 | 75 mm M3 |
| Gun `range_m` (engagement) | 600 m | 600 m |
| Elevation | −8° / +20° | −10° / +25° |
| Reload (with loader) | 6.5 s | 6.0 s |
| Ammo AP / HE | 30 / 30 | 30 / 40 |
| MGs | coax + hull (MG 34) | coax + hull (M1919A4) |
| Sight | 175 m | 175 m |
| Fire on the move | spread ×4 | spread ×3 (stabiliser) |

The Sherman's cost is 560, down from the draft's 600, because of the
frontal asymmetry below. If it plays badly, tune cost first.

**Armour** (mm, slope in degrees from vertical):

| Face | Panzer IV H | M4A3 (75) |
|---|---|---|
| Hull front | 80 @ 10 | 63.5 @ 47 |
| Hull side | 30 @ 0 | 38 @ 0 |
| Hull rear | 20 @ 10 | 38 @ 10 |
| Hull top | 12 | 19 |
| Turret front | 50 @ 10 | 76 @ 30 |
| Turret side | 30 @ 25 | 51 @ 5 |
| Turret rear | 30 @ 15 | 51 @ 0 |
| Turret top | 16 | 25 |

**What this gives at the ranges the game reaches.** Tanks see each other
at about 350 m (525 m right after one fires), on a 1,024 m map. So tank
fights happen at about 100–400 m, not at 1,000 m. By the formula below
(flat shots, ±7 % band):

| Shot | 150 m | 300 m | 450 m |
|---|---|---|---|
| PzGr 39 vs Sherman glacis (93 mm effective) | 100 % | 100 % | 100 % |
| PzGr 39 vs Sherman glacis, Sherman angled 30° (107 mm) | 100 % | 100 % | 92 % |
| PzGr 39 vs Sherman turret front (88 mm) | 100 % | 100 % | 100 % |
| M61 vs Panzer IV hull front (81 mm) | 99.8 % | 88 % | 58 % |
| M61 vs Panzer IV hull front, angled 20° (86 mm) | 57 % | 20 % | 1 % |
| M61 vs Panzer IV hull front, angled 30° (94 mm) | 0 % | 0 % | 0 % |
| M61 vs Panzer IV turret front (51 mm) | 100 % | 100 % | 100 % |
| Bazooka (100 mm HEAT) vs Panzer IV hull front, angled 40° | 0.9 % at 100 m | | |

Honestly: the KwK 40 goes through the Sherman anywhere it hits, and
angling does not help the Sherman. A Panzer IV that angles 20–30° makes
its hull front almost immune to the Sherman, but its turret front stays
weak at every range. So the Sherman wins by aiming at the turret, by
flanking (24°/s turret, faster hull), and by being cheaper. Expect the
Panzer IV to win most even frontal duels. That asymmetry is historical
(§2.3: authentic) and balanced by cost, not by fake numbers. The 1,000 m
and 1,500 m golden rows below are math tests; they matter for later maps
and the owner's spotting-range question.

The model is a pure cosine model: published 30° tables are divided by
cos 30°, and slope uses 1/cos. No normalisation, no overmatch. Fine as a
starting point; the M2 mesh model can refine it.

### Infantry anti-tank teams

| | Panzerschreck team | Bazooka team |
|---|---|---|
| Men | 2: gunner with launcher, loader with rifle | same |
| Cost / pop / call-in | 200 / 2 / 20 s | 180 / 2 / 20 s |
| Launcher | RPzB 54, HEAT 160 mm, 110 m/s | M9, HEAT 100 mm, 85 m/s |
| Range | 150 m | 120 m |
| Opens fire on a still / moving vehicle within | 120 m / 70 m | 100 m / 60 m |
| Spread | 12 mrad | 15 mrad |
| Rockets carried | 6 | 6 |
| Reload | 8 s (16 s without the loader) | same |
| Backblast reveal | 300 % for 3 s | same |

- Squad defs gain `weapon_men` (how many men carry `weapon`; default all)
  and `sidearm` (what the rest carry). The AT team has `weapon_men = 1`,
  `sidearm` = the faction rifle.
- The engagement distances are weapon-def fields (`engage_still_m`,
  `engage_moving_m`) and apply to every player, human or AI. A target is
  "moving" above 1 m/s.
- Weapon defs gain `fire_reveal_pct` and `fire_reveal_ms` (default: the
  rules' `firing_spot_pct` 150 and `reveal_ms` 2000). Launchers use 300 %
  for 3 s: the backblast gives the team away.
- If the gunner dies, the next living man takes the launcher after 3 s.
- Launchers only target vehicles. The loader's rifle engages infantry.
- In a trench the gunner rises to the fire step to fire, as for a throw.
- Rockets resupply in the own base, one per 20 s, like items.

### Economy and limits

- Vehicles cost manpower only. No fuel resource in S3.
- **Vehicles unlock at `unlock_s` = 240 s** of match time, so the opening
  stays infantry. The call-in tile shows a countdown until then. Tests
  use a rules override `unlock_s = 0` (below).
- **At most 4 vehicles per player**, alive or queued.
- Pop: each soldier squad type counts its `men`, each vehicle squad type
  its `pop`. `MatchState::population` reads that per squad type. The cap
  stays 120.
- AT teams are available from the start. The AI buys them only after
  unlock (AI section).

## Vehicle model

### Why boxes with authored slopes (not armour meshes yet)

§5.7 wants a low-poly armour mesh per vehicle with a triangle BVH. For S3
we use **oriented boxes**: one hull box and one turret box per vehicle.
Each box face is an armour plate from the def's `[armor]` table. The face
is geometrically vertical (or flat for tops), but its **ballistic normal
is tilted by the authored `slope_deg`**. That keeps sloped armour,
compound angles, hull-down and turret-vs-hull hits, at about a tenth of
the work: no mesh authoring, no glTF extras, no BVH.

What we lose: mantlets, lower glacis plates, weak spots, curved castings.
For RTS play at 40–150 m camera height that is acceptable. For direct
control at M5 it is not.

**Path to the full model (M2):**
- Zone names (`hull_front`, `turret_side`, ...) and the `[armor]` table
  shape are the §7.4 ones, so defs keep working.
- The narrowphase is one function, `hit_armour(segment, vehicle) ->
  Option<ArmourHit { t, zone, normal, point }>`. M2 replaces its body with
  the mesh BVH. `slope_deg` is then ignored and later removed.
- Everything after the hit (`penetrates`, spall, components) takes an
  `ArmourHit` and does not change.

### Armour tables (derived)

`armour.rs` builds `ArmourTables` from `MatchSetup` with `Fx::sin_cos`:
per ammo, `cos(pen_reference_angle)` and `cos(ricochet_angle)`; per
vehicle type, each face's tilted unit normal in the box frame. They are
built in `MatchState::new` and after load, are not saved, and depend only
on the (hashed) setup. The CLI and the golden tests build them through
the same function.

### Geometry and pose

- A vehicle has a position, a hull yaw, and a hull **pitch and roll**
  from the ground. Each track samples heights at 5 points along its
  length (0 rear to 4 front). Rear support is the highest of samples 0–1,
  front support the highest of 3–4, each at its own sample position.
  The middle sample only counts for "is this track over a ditch". So a
  vehicle bridges a ditch narrower than about half its track instead of
  nose-diving, and a crest never gives a zero lever arm.
- Pitch and roll are clamped to ±30° and change at most 20°/s.
  `max_slope_pct` is 50 (27°), so a slope the vehicle may climb never
  exceeds the clamp.
- The rotation is a fixed-point 3 × 3 matrix built from the forward and
  up vectors (cross products and `sin_cos` for yaw). The turret turns
  about the hull's up axis around its pivot; the gun pitches about the
  turret's right axis. Turret yaw and gun pitch use `sin_cos`; laying
  uses `atan2`.
- World-space box axes and centres for every vehicle and wreck are
  computed once per tick after Movement (`vehicle_frames`, derived).
- **Hit test:** a swept segment is moved into box space (subtract the box
  centre, then dot products with the box axes, i64) and slab-tested. The
  entry slab names the face. Terrain, capsules and boxes are tested along
  the segment and the **earliest hit wins**. At equal t: terrain, then
  box, then capsule, then the lower id.
- **Terrain re-test:** `Heightfield::segment_hit` steps 1 m on base
  ground (0.25 m on detail tiles), so it can be up to 1 m late. When a
  box hit is found at t_b, terrain is re-tested from the segment start to
  t_b at 0.25 m steps. Only box hits pay this. So a crest just in front
  of a hull blocks the shot (hull-down).
- **Track band:** the lower `track_band_pct` (45 %) of each hull side is
  running gear. Any AP or HEAT hit there deals the ammo's `track_damage`
  to that side's track, penetration or not. AP that goes on into the hull
  meets `track_mm` (20 mm) extra.

### Crew: seats, not soldier entities

Crew are **seats inside the vehicle**, each with a role, HP (100) and a
small box in the hull or turret frame. They are not soldier entities.
A soldier entity riding inside would need special cases in spotting,
movement, ORCA, capsules, squads and the HUD. Seats are what §5.9 and
§5.12 need anyway.

| Role | Wounded (HP < 50) | Dead |
|---|---|---|
| Driver | speed ×0.8 | no movement until someone swaps in |
| Gunner | spread ×1.5 | no main gun or coax until someone swaps in |
| Loader | reload ×1.5 | reload ×2 (the gunner loads) |
| Commander | sight ×0.8 | sight ×0.7 |
| Radio operator (`radio_mg`) | hull MG spread ×1.5 | hull MG silent |

- **Auto swap:** when the driver or gunner seat is empty, the living
  crewman of lowest priority (radio, then loader, then commander) moves
  in over `seat_swap_s` (4 s); both seats count as empty meanwhile.
  Gunner first, then driver.
- **No bail-out in S3.** The crew are lost with the vehicle. Survivors
  leaving as a crew squad is S3b.

### Components

| Component | HP | Damaged (< 50) | Destroyed (0) | Field repair |
|---|---|---|---|---|
| Engine | 100 | speed ×0.5 | immobilised | to 50 in 60 s |
| Left / right track | 100 | — | immobilised | to 100 in 30 s |
| Main gun | 80 | spread ×2 | no main gun | none |
| Turret ring | 80 | traverse ×0.5 | turret fixed | to 50 in 40 s |
| Ammo rack | 60 | — | detonates | none |

- Internal components have a box: engine, ammo rack, gun breech, turret
  ring. External parts have anchor points: each track has three (front,
  middle, rear); the gun barrel has one (the mantlet centre).
- **Fire:** an engine hit for 40+ damage starts a fire with 30 %. While
  burning, the engine loses 5 HP/s. Every 4 s, if a crewman is alive, a
  25 % roll puts it out. A fire still burning after 20 s knocks the
  vehicle out.
- **Ammo rack:** each hit on the rack for 30+ damage detonates it with
  35 %. A rack destroyed by damage also detonates. Detonation knocks the
  vehicle out. One rule, one roll per hit.
- **Field repair** (`RepairUnits`, or the AI): the crew fixes the worst
  repairable component. Meanwhile the vehicle can't move or fire the main
  gun; MGs still fire. Repair stops if the vehicle is hit.
- **Base repair:** inside the own base zone a vehicle regains 2 % of
  every component's HP per second (the gun too), one AP or HE round per
  2 s, and one dead crew seat per 20 s. Free.

No hull hit points. A vehicle that is never penetrated and never loses a
component fights on (§5.9). An immobilised tank with a broken gun is not
removed; it can repair its tracks and drive home for base repair.

### Knock-out and wrecks

- A vehicle is **knocked out** when every seat is dead, when the ammo
  rack detonates, or when a fire burns 20 s. The remaining crew die.
  Losses count its crew; the report counts it as a vehicle lost.
- A knocked-out vehicle keeps its `SimId`; its entity kind becomes
  `Wreck` and its `VehicleState` stays in `vehicles` with `wreck_tick`
  set. There is no second wreck list.
- On knock-out, in the Damage stage: followers get Stop; every
  `AttackUnit` target and gunner target pointing at it is cleared.
- Wreck boxes stay in the ballistic world: hard cover for infantry,
  impassable for vehicles (vehicle block layer), an obstacle for
  infantry avoidance. Wrecks belong to no player and stop everyone's
  rounds. They are visible to every player.
- At most `wreck_cap` (32) wrecks; beyond that the oldest is removed and
  its id freed. The id allocator reuses freed ids (LIFO), so a test
  checks that nothing refers to a removed wreck's id.
- A wreck smokes for 60 s (presenter only).

## Ballistics and penetration

### Ammo

Ammo defs go in `mods/ww2/defs/ammo/*.toml` (a §7.4 def type).
`penetration` is a table of `[range_m, mm]` against a plate at
`pen_reference_angle_deg` (30° for published AP tables, 0° for HEAT).

| Ammo | Mechanism | m/s | Penetration (mm at ref. angle) | Ref. | Ricochet over | `nonpen_damage` | `track_damage` |
|---|---|---|---|---|---|---|---|
| 7.5 cm PzGr 39 | aphe | 790 | 100 m 106, 500 m 96, 1000 m 85, 1500 m 74, 2000 m 64 | 30° | 70° | 40 | 60 |
| 7.5 cm Sprgr 34 | he | 550 | 10 flat | 0° | 80° | — | 40 |
| 75 mm M61 | aphe | 618 | 100 m 76, 500 m 70, 1000 m 63, 1500 m 57, 2000 m 51 | 30° | 70° | 40 | 60 |
| 75 mm M48 | he | 463 | 10 flat | 0° | 80° | — | 40 |
| RPzB 54 rocket | heat | 110 | 160 flat | 0° | 80° | 30 | 50 |
| M6A3 rocket | heat | 85 | 100 flat | 0° | 80° | 30 | 50 |

- Rounds have no drag in S3, as in S1. `drag_coeff` is accepted and must
  be 0; `validate` rejects anything else.
- Rockets fly as ballistic rounds at constant speed (no motor burn).
- HE and HEAT burst on contact whatever the result.

### Penetration rule (fixed point)

```
pen0(range) = interp(penetration, range) / cos(pen_reference_angle)   # normalised to 0°
c           = −(dir · n)     # dir: unit shell direction; n: outward plate normal tilted by slope
if c ≤ cos(ricochet_angle)   # also covers c ≤ 0 (a shell entering "from behind" the tilted normal)
    → Ricochet (shell consumed; event only)
v           = triangular(−7 %, +7 %), sum of two counter-RNG uniforms
pen         = pen0 × (1 + v)
pen × c ≥ thickness_mm (+ track_mm × c in the track band)  → Penetration
else                                                        → NonPenetration
```

- `range` is the distance from the shell's muzzle point (stored on the
  projectile) to the impact point, computed in i64.
- `interp` is piecewise linear in i64, clamped to the table ends.
- The comparison has no division. Every ricochet angle in S3 is at most
  80°, so c stays above cos 80° and no extra cap is needed. The CLI
  prints the effective thickness `thickness / c` for people.
- For a flat shot at a face with slope `s` and horizontal angle `a` from
  the face normal, `c = cos s · cos a`. The CLI prints this.
- HE shells use the same function with their flat `pen_mm`.

### After the hit

**Penetration** (AP and HEAT), in the vehicle's frame:
- The **core** continues along the shell path for 1.5 m (HEAT jet:
  1.0 m). It deals `core_damage` (AP 100, HEAT 120) to every component
  and seat box it crosses, at most 3.
- **Spall:** `spall_rays` rays (AP 6, HEAT 3) in a 25° cone around the
  path from the entry point, `spall_damage` each (AP 35, HEAT 25), first
  box hit only. Each direction is the path direction plus right·tri plus
  up·tri, normalised, as `shot_velocity` does. No rejection sampling.
- **HE filler** (aphe): a burst 1.0 m past the entry point deals 80 to
  boxes within 1.2 m.
- At most 8 rays × 14 internal boxes per penetration (a tank has 9).

**HE contact on a plate:** if the HE's `pen_mm` beats the plate by the
rule above, an inner burst at the contact point deals `inside_damage` to
boxes within `inside_radius` (the HE-filler code). With S3 data no HE
round beats any plate of either tank (10 mm against 12 mm and up); the
rule is there for S3b's open tops and deck satchels.

**Non-penetration:** AP and HEAT deal `nonpen_damage` to external parts
whose anchor is within 1.0 m of the impact, and the track band rule
above. The gun barrel is damaged only this way: by a direct impact
within 1.0 m of its anchor.

**Blasts near a vehicle** (mortar, tank HE, rockets, frags, satchels on
the ground) damage **tracks only**: each track anchor within the blast's
lethal radius and in clear line takes `damage × vs_vehicle_pct`.
Starting `vs_vehicle_pct`: tank HE 40, mortar 40, HEAT 40, frag 10,
satchel 100. Blasts never reach the engine, the gun, internal parts or
crew of a closed vehicle. So a mortar near hit (150 × 40 % = 60) breaks a
track in two hits; mortars can immobilise a parked tank but never kill
one. The report counts track kills by mortar.

**Bursts on a vehicle carve no crater.** A burst more than 0.5 m above
the terrain under it leaves the ground as it is.

**Infantry hit by a shell:** shells now test soldier capsules too. AP
kills the man it hits (300 damage) and stops. HE and HEAT burst at the
hit point.

**Small arms on vehicles:** a rifle or MG round that hits a vehicle box
stops. No damage and no event (otherwise every MG burst would flood the
feed, markers and audio).

**Friendly fire:** none, as in S1. Own rounds and blasts pass through
own vehicles and ignore own soldiers. Enemy rounds stop on them. That is
what lets infantry walk behind their own tank.

### HE against infantry

Tank HE uses the S1 burst code (damage falloff, terrain between blast
and body blocks, suppression, crater).

| HE | Damage | Lethal | Damage to | Suppression | Crater |
|---|---|---|---|---|---|
| 7.5 cm Sprgr 34 / 75 mm M48 | 150 | 2.0 m | 6 m | 40 to 12 m | 0.6 m radius, 0.3 m deep |
| HEAT rockets | 100 | 1.0 m | 3 m | 20 to 6 m | 0.4 m, 0.2 m |

HE defs (tank HE, mortars, frags, satchels) gain `pen_mm`,
`inside_damage` and `inside_radius` in `HeSetup`. Starting `pen_mm`:
tank HE 10, mortar 8, frag 2, satchel 30.

HE fired at infantry uses `he_area_spread_mrad` = 6 instead of the gun's
AP spread. This is a tuning lever, not physics: without it every round
would be a precise, squad-hitting shot from 300 m, in a game where
mortars already cause most kills. HE ammo is limited (30–40 rounds,
refilled only in base). The crater is smaller than the draft's (0.6 m,
0.3 m) so one hit does not remove 2 m of a 0.3 m parapet; the test
measures erosion.

### Gunnery

Vehicle gunnery runs in the Weapons stage, serially in ascending vehicle
id, after the soldiers' parallel aim-and-fire.

- **Targets:** with ammo `Auto`, spotted enemy vehicles in `range_m`
  first (AP), else the best spotted infantry group in range (HE: more
  men, closer, scores higher; never within 30 m of own men).
- **Aim point:** the gunner aims at the centre of the visible part
  (hull or turret) with the better penetration chance. A part is visible
  if a line-of-fire ray (terrain only) reaches its centre. The chance
  comes from `armour.rs` on public def data: the loaded AP against the
  zone that faces the shooter, at the current angle and range. Within 10
  points, the hull wins (the bigger target). So a Sherman facing an
  angled Panzer IV aims at the turret, a hull-down tank is shot at its
  turret, and two tanks that can pen anywhere shoot centre of mass.
- **Lead:** the aim leads the target by its velocity averaged over about
  1 s (an exponential average kept on each vehicle, saved and hashed)
  times the flight time. Turns and acceleration cause misses. Elevation
  uses the drop compensation of `combat::shot_velocity`, two fixed
  iterations. (An exact solution overflows: v⁴ at 790 m/s is 3.9 × 10¹¹.)
- **Spread:** 2.5 mrad stationary. Each further shot at the same target
  is 20 % tighter, down to 50 % (ranging in). × the own vehicle's moving
  factor when moving. × `moving_target_spread_pct` (300 %) against a
  target faster than 1 m/s. × crew and gun damage factors.
- **Laying:** turret and gun turn at their def rates through `lay_gun`.
  The gun fires only when laid within its current spread of the
  solution.
- **Ammo switch:** `Auto` loads the round that fits the next target; if
  the wrong round is loaded, the crew swaps it (one reload). `AP` or `HE`
  forces that round.
- **MGs:** coax follows the turret; the hull MG has a ±15° arc. They
  engage spotted infantry within their weapon range by the S1 MG rules
  (sweep, suppression). Vehicles are never suppressed in S3.
- **Small arms do not target vehicles.** Only main guns, launchers and
  satchel throwers do.

## Movement and terrain

### Driving

- Kinematic tracked model (§5.5): `drive(state, throttle, steer,
  terrain)` with acceleration (1.5 m/s²), braking (4 m/s²), hull turn
  rate, and neutral steer (pivot) when the heading error is over 60° and
  speed is low.
- Speed limits: def max speed × slope factor (linear down to 30 % at
  `max_slope_pct`) × woods (50 %) × crossing cap (1 m/s, below).
- `drive` refuses a step that would put any track sample on a
  vehicle-blocked node (below). A vehicle whose footprint ends up on
  blocked ground (a ditch dug under it) is immobile, shown as Bogged,
  until the ditch there is filled. It can still fire.
- **Reverse:** `ReverseUnits` backs up to the target keeping the front
  toward the enemy. A Move whose target is behind and within 40 m also
  reverses.

### Avoidance

The current ORCA (`systems::plan_velocities`) uses one global radius,
splits avoidance half and half, and takes neighbours only from the
soldier hash. So:

- **Vehicles are not ORCA agents.** They move only through `drive`.
- **Soldiers avoid vehicles and wrecks.** Each vehicle or wreck counts as
  two circles along the hull axis (radius: half width + 0.25 m), with the
  vehicle's velocity. Soldiers take full responsibility (no ½). Up to 4
  nearest vehicle circles are added on top of the 8-neighbour cap, so a
  crowd never pushes a tank out of a man's list.
- **Vehicles yield to vehicles.** If another vehicle or a wreck is inside
  a box 1.5 × hull length ahead, a vehicle slows or stops. Between two
  moving vehicles the higher id yields; one blocked for 5 s re-paths.
- A vehicle slows to walking pace when a friendly soldier is within 4 m
  ahead.
- **Push-out:** an enemy soldier inside a moving vehicle's footprint is
  moved out sideways (perpendicular to the hull axis, never into deeper
  dug ground). A man whose capsule top is below the hull's belly at his
  position (in a trench under a crossing tank) is not moved. A unit test
  drives a tank across an occupied trench; no man changes cell.

### Escort

- Infantry given `FollowUnits` on a friendly vehicle take **single-file
  slots 1.5 m apart, 4 m behind it**, on the line from the nearest known
  enemy (else the vehicle's move target) through the vehicle. A ninth or
  later follower takes a second file beside the first.
- A vehicle with followers caps its speed at their walking speed.
- A whole squad gets a squad-level follow. Detached men follow one by one
  and stay detached.
- Honest limit: a 3 m wide hull shields only a narrow cone. Fire from a
  60 m trench front comes from up to about ±27° at 60 m, so the file is
  only partly covered. Spotting rays ignore vehicle boxes, so escorts stay
  spotted and get shot at; the rounds that would hit them stop on the
  tank. `armour-test` reports the share of rounds the tank stopped.

### Pathing

Vehicles path on the S1 4 m grid with their own cost, through
`PathReq::Vehicle { squad, avoid: Option<node> }` in the shared queue
(saved and hashed like the other requests). `serve_paths` picks the cost
by request kind.

| Ground | Vehicle cost |
|---|---|
| Slope | as infantry, impassable above `max_slope_pct` (50 %) |
| Woods | ×2 |
| Dug 0.4–1.0 m (craters) | ×1.5 |
| Dug 1.0 m or more (trench, big crater) | ×4; crossing rules apply |
| Vehicle-blocked (below) | impassable |

- **Vehicle block layer** (derived bitset per grid node, not saved): a
  node is blocked if it lies within 2 m of an AT-ditch line and its `dug`
  depth is at least `ditch_block_dm` (1.9 m), or if a wreck footprint
  covers it. The layer is dilated by one node, so a 3 m hull never clips
  an edge. It is updated from the terrain dirty rect and on wreck add and
  remove, and rebuilt on load.
- Depth alone never blocks. Craters stack (`Heightfield::crater` is
  additive): one mortar hit in a 1.6 m trench can reach 2.6 m, and two
  overlapping craters in the open can pass 1.9 m. Blocking by depth
  alone would turn every shelled trench line into a tank wall. Depth
  still drives crossing speed and bogging.
- A satchel's levelling crater (`crater_cut`) can fill part of a ditch.
  Where the depth falls below 1.9 m the node opens. Blowing a ditch wall
  with satchels is an intended way to breach it.
- Vehicle path smoothing reads the block layer. A moving vehicle
  re-checks the segments ahead of it every 0.5 s and re-paths if one
  crosses a newly blocked node.
- **Stuck check:** less than 1 m of progress in 5 s while ordered to
  move: re-path with `avoid` set to the blocking node (×8 cost for that
  search). At most one re-path per vehicle per 2 s.

### Trenches as obstacles

Real tanks crossed 2.2–2.3 m gaps. Our standard trench is about 1.6 m
wide at the top and 1.6 m deep. So **a standard trench slows a tank and
can bog it, but does not stop it** (owner question 3). The AT ditch stops
it.

- **Depth at track samples** comes from the detail tier (surface below
  base), not the 4 m `dug` layer. The coarse layer would make a tank
  crawl when it drives 3 m beside a trench. Pathing uses `dug`.
- **Crossing:** while any track sample is over ground dug 1.0 m or more,
  speed is capped at 1 m/s.
- **Bogging:** a crossing starts when any track sample is over ground dug
  1.0 m or more after at least 2 s with none. At that moment, once per
  crossing, roll `bog_pct` (6 %; RNG key: vehicle id, crossing count,
  BOG) and count the crossing. Bogged: immobile for 20–40 s (rolled)
  while the crew works it free. The gun still fires. So diagonal
  crossings and driving along a trench roll once, not once per node.
- **AT ditch** (new dig profile, engineers only):

  | | Standard trench | AT ditch |
  |---|---|---|
  | Depth | 1.6 m | 2.3 m |
  | Floor | 0.8 m | 1.0 m |
  | Wall run | 0.4 m | 1.2 m (top width about 3.4 m) |
  | Spoil | parapet 0.3 m, enemy side | berm 0.6 m, enemy side |
  | Work per metre | 8,400 | 22,000 |
  | Max length per job | 60 m | 30 m |
  | Who | any digger | engineers |

  One 5-man engineer squad digs 10 m in about 3.7 min (5 men × 200 % ×
  100 work/s against 220,000). §5.10.9 asks about 6 min for 8 engineers;
  ours follows our faster trench pace. The dig code generalises from one
  rules profile to named profiles in `[dig.profiles.*]`. Every AT-ditch
  job adds its line to `MatchState::at_ditches` (saved, hashed) when it
  starts, so a ditch blocks section by section as it gets deep enough.
- **Infantry and AT ditches:** men may cross at ×3 path cost. AT-ditch
  nodes get no `trench_cost_pct` discount, so attackers do not use a
  ditch as a covered road to the trench. The cover finder skips them (no
  fire step at 2.3 m). A unit test orders a squad across a ditch; it
  arrives.
- **Honest limit:** a 30 m ditch on open ground is easy to drive around.
  Ditches channel tanks into AT ambushes more than they wall them off.
  Players can chain several jobs.

## Infantry vs tanks

- **AT teams** are the main answer: best from a trench, on the flank,
  inside 100 m. HEAT beats every front at 0°, but a Panzer IV angled 40°
  shrugs off the bazooka (0.9 %).
- **Satchels on the ground** next to a track break it (300 × 100 % on a
  100 HP track). The fuse runs 5 s from the start of the throw and the
  range is 3–8 m, so this works on a stopped, bogged or crossing tank
  (1 m/s), not on one driving past at 6 m/s. Throws stay `Point` throws;
  out-of-range targets are handled by the S1b throw rules.
- **Frags** do 10 % to track anchors: useless against a tank.
- **Spotting help:** any enemy soldier within 25 m of a vehicle spots it
  without a ray (engine noise and size).

## Tanks vs trenches: what to expect

The TODO says assaulting a finished trench across open ground barely
works. Do tanks fix that? **Partly, and only together with infantry.**

What does not change: flat fire still cannot reach a man crouched below
the lip. A tank (175 m sight) sees a crouched man in a trench only within
105 m (60 %), and a man who just fired within 262 m (150 %). Direct HE
hits the parapet; the earth blocks the blast, so it mostly suppresses
(halved behind earth) and slowly cuts the lip. It rarely kills.

What changes:
1. **Rifles can't hurt a tank.** Its coax can sit at range and fire at
   every man who rises to the fire step. Near misses add suppression.
2. **Moving cover.** Enemy rounds stop on its boxes, so an escort file
   walks up behind it (with the cone limit above).
3. **Parapet erosion.** HE craters lower the lip over many shots.
4. **Enfilade.** A tank at the end of a straight trench fires along its
   floor. Our trenches are straight polylines, so this works by geometry.

What stops it: a dug-in AT team or engineers with satchels. A lone tank
that drives up to a trench with AT in it should usually die. The AI keeps
its tanks at 160 m (outside launcher range) when enemy AT has been seen;
from there the coax reaches men who fire, not hidden ones.

Predictions that `armour-test` measures (reported, not gated):
- Tank plus escorted rifle squad vs a dug-in rifle squad without AT:
  attacker losses at most half those of the same walk without a tank,
  and defenders lose at least 75 % (owner question 5).
- Lone tank vs a dug-in rifle squad with an AT team: the tank is
  disabled in most runs.

If the attackers still cannot get in, the next levers are the owner's:
"tank fear" (enemy soldiers without AT near a moving enemy tank gain
suppression), mortar smoke, or suppression that really pins trench men.
S3 does not add them. Also watch the opposite risk: if tanks make
trenches feel useless, AT ditches and AT teams are too weak or too
expensive.

## Spotting

- Vehicles use the existing `Combat::seen_by` column (it exists for
  every entity). The spotting cursor walks enemy soldiers, corpses **and
  vehicles**; wrecks are skipped and always visible.
- The per-player quota (`enemies`) counts vehicles too.
- A vehicle is checked at two points, turret top then hull top centre,
  from each of the nearest `spot_observers` (2) observers in range: up to
  4 rays. The budget early-out reserves 2 × `spot_observers` rays before a
  vehicle check. The first clear ray spots it. A hull-down tank is spotted
  by its turret.
- Concealment, as percent of the observer's sight: 200 % (big), 250 %
  moving, the gun's `fire_reveal_pct` (300 %) for its `fire_reveal_ms`
  after firing the main gun; the highest applies, then × 50 % in woods.
- Vehicles as observers: eye at the commander's hatch (about 2.5 m),
  their def sight, ×0.8 or ×0.7 with the commander wounded or dead.
- `line_of_sight` tests terrain (and smoke) only. Vehicles and wrecks
  block neither spotting nor target search in S3. They do stop rounds.
- What these numbers mean: tanks see tanks at 350 m (437 m moving, 525 m
  after firing); riflemen (150 m) see tanks at 300 m. Owner question 2
  asks whether that is the range the owner wants.

## Commands

New and changed `CommandKind`s. Ids follow the S1b `*Units` validation
(1–120 ids, sorted, deduplicated, own, alive). A vehicle is a squad of
one, so a vehicle id maps to its squad order. `unit_set` gains a variant
that accepts vehicles for the commands that allow them.

| Command | Effect |
|---|---|
| `CallIn { squad_type }` | unchanged; vehicles are squad types with a vehicle def. Rejected before unlock, at the vehicle limit, or over pop |
| `Move`/`AttackMove`/`Stop`/`Retreat` (squad) and `MoveUnits`, `AttackMoveUnits`, `StopUnits`, `RetreatUnits` | accept vehicle squads and ids. AttackMove vehicles halt to fire the main gun |
| `Dig`, `Barrage`, `Regroup`, `SetStance` | rejected for vehicle squads |
| `ReverseUnits { units, target }` | vehicles back up to `target`; soldiers in the set get a Move |
| `FaceUnits { units, toward }` | vehicles turn the hull front to the point and hold |
| `AttackUnit { units, target }` | `target` must be a live enemy vehicle. Vehicles and launcher gunners in the set focus it while it is seen; vehicles without range or a line of fire move toward it until they have both. Other men ignore it |
| `SetAmmo { units, ammo }` | `Auto`, `Ap` or `He` |
| `RepairUnits { units }` | crews repair the worst repairable component in place |
| `FollowUnits { units, vehicle }` | soldiers escort a friendly vehicle (Escort above) |
| `Dig { squad, line, profile }`, `DigUnits { units, line, profile }` | `profile`: `Trench` (default) or `AtDitch` (engineers only) |

`Throw` and `ThrowEach` are unchanged.

### Vehicle squads in soldier-only code (checklist)

These paths assume soldiers today; each needs a vehicle rule:
- `squad::steer_squads`, `Squad::lead`, `update_focus`, `fire_mortars`:
  skip vehicle squads (vehicles move by `drive`, aim by gunnery).
- `order_dig`, `order_barrage`, `Regroup`, `SetStance`: reject.
- `MatchState::population`: per squad type `pop`.
- `rules::check_defeats`: a player with live vehicles is not Wiped.
- `serve_paths`: cost by request kind.
- Capture counts soldiers only (vehicles neither capture nor contest).

## AI

The AI still plans squads every 2 s, reads only what its player sees,
and emits commands. New behaviour, all deterministic:

1. **Buy.** The mix gains `mix_vehicle_pct` (20) after unlock and under
   the vehicle limit, and `mix_at_pct` (10) after unlock **and** after an
   enemy vehicle has been seen (a saved flag). When the enemy has more
   spotted vehicles than the AI has AT teams plus tanks, AT jumps the
   queue.
2. **Tanks escort assaults.** A tank goes with infantry assigned to an
   attack; those squads get `FollowUnits` on it.
   - If no enemy AT has been seen near the point for 30 s, the tank
     advances with its followers to `assault_close_m` (40 m) from the
     enemy trench (or the point), then the followers AttackMove.
   - If enemy AT has been seen, the tank holds at `tank_standoff_m`
     (160 m, outside launcher range) and fires; its followers detach
     there and AttackMove.
   - Whenever the tank halts for any reason, followers that are still
     attached detach and AttackMove, so they never wait behind a parked
     tank in the 60–150 m band.
3. **Hold facing.** Idle vehicles `FaceUnits` toward the nearest known
   enemy vehicle or AT team.
4. **AT teams** defend points under armour threat, in the trench if there
   is one. Their fire discipline comes from the weapon def, as for a
   human.
5. **Satchels.** Engineers throw at an enemy vehicle within 8 m that is
   slower than 1 m/s (stopped, bogged, crossing, immobilised), aimed at
   its predicted position when the fuse runs out, beside the nearer track.
6. **Repair and retreat.** An immobilised vehicle with no seen enemy
   within 150 m repairs. A vehicle with its gun destroyed, two crew dead,
   or engine under 30 % retreats to base when it can move.

Difficulty uses the existing knobs (reaction delay, decision quality).
New AI state (escort assignments, last AT sighting per point, the
enemy-vehicle-seen flag) is saved and hashed.

## Sim structure

### Stages and indices

- **Movement:** soldiers plan against last tick's vehicle poses (read
  only); then vehicles `drive` in ascending id. Then the derived
  `vehicle_index` (a second `SpatialHash` holding vehicles and wrecks at
  their centres) and `vehicle_frames` are rebuilt. The soldier hash keeps
  soldiers only: putting vehicles there would make `fly` test a capsule at
  a tank's centre and ORCA treat a tank like a 0.25 m man.
- **Queries** on `vehicle_index` pad their reach by `max_vehicle_radius`
  (derived from the defs), because the hash stores one point per entity.
- **Weapons:** soldiers as today; then vehicle gunnery, serially.
- **Ballistics** (`combat::fly`, parallel per slot chunk): per substep,
  vehicles near the segment get one bounding-sphere test, then at most 2
  box slab tests. `fly` only records an `ArmourHit` event; it rolls
  nothing. Counted in `Stats::armour_box_tests`.
- **Damage** (serial, slot order): penetration rolls, spall, components,
  crew, fire starts, ammo rack, knock-out and wreck conversion. Then
  thrown items, then deaths, as today.
- **Status:** fire, repair, bog, seat swap, reload and wreck-cap timers.

### RNG keys

Projectiles have no id and their slots are reused, so keys use the slot
and the tick:

| Purpose | Key |
|---|---|
| `PENETRATION`, `SPALL`, `HE_FILLER` | (tick, slot, purpose); spall rays by ray index within the stream |
| `GUN_SPREAD` | (tick, vehicle id, purpose) |
| `BOG` | (crossing count, vehicle id, purpose) |
| `FIRE` (start and each 4 s roll) | (tick, vehicle id, purpose) |
| `AMMO_RACK` | (tick, slot, purpose) |

### Budgets (§2.2)

All in-sim budgets are work counts. Peak numbers are what the tests
report.

| Work | Rule |
|---|---|
| Vehicle path searches | the S1 queue: 4,000 expansions per tick, 8,000 per search, debt rule as today. Dilation is baked into the block layer, so a neighbour check is one bit |
| Shells and rockets | always fully simulated. `HE_RESERVE` (32) becomes `SHELL_RESERVE` = 64 slots of the 2,048-slot pool. Small arms never take them |
| Armour narrowphase | per substep: one bounding-sphere test per nearby vehicle, at most 2 box slab tests |
| Penetration | at most 8 rays × 14 internal boxes |
| Spotting | up to 4 rays per vehicle check, from the existing budget |
| Movement | 10 height samples per vehicle; up to 4 vehicle circles per soldier |

## State

Saved, loaded, hashed and validated:
- `EntityKind::Vehicle` and `EntityKind::Wreck` in the entity store
  (transform, owner, squad, velocity).
- `MatchState::vehicles: BTreeMap<SimId, VehicleState>`: hull pitch and
  roll, turret yaw, gun pitch, speed, averaged velocity, crew seats,
  components, ammo counts, loaded round, ammo choice, reload ticks,
  target, ranging step, flags (bogged, crossing, burning, repairing),
  timers, crossing counter, followers, `wreck_tick`.
- `MatchState::at_ditches`: AT-ditch lines.
- Projectiles gain `ammo` (index, or none for small arms) and `origin`
  (muzzle point): about 14 bytes per pool slot.
- Dig jobs gain their profile. `Order::Follow` and the new unit orders.
- `PathReq::Vehicle` in the path queue.
- New AI state.
- New RNG purposes as above.

Rules:
- `VehicleState`, the AI state and the new orders are hashed through
  postcard bytes, as the AI state is today, not field by field in
  `put_game`. A test changes each `VehicleState` field in turn and checks
  that the hash changes.
- `MatchState::validate` (saves are untrusted): vehicle type index in
  range; seat and component counts match the def; referenced ids
  (targets, followers, escorts) are alive and of the right kind; wreck
  count within the cap.
- `desync-diff` names the new fields.

## Content (`mods/ww2`)

- `defs/vehicles/`: `ger_pz4_h`, `usa_m4a3_75`. Vehicles get their own
  folder and def type rather than `kind = "vehicle"` in `units/` (§7.4),
  because `SquadDef` denies unknown fields.
- `defs/ammo/`: the six rounds above.
- `defs/weapons/`: `kwk40_l48`, `m3_75mm`, `mg34` (coax and hull),
  vehicle mounts of the existing `m1919`, `rpzb54`, `m9_bazooka`.
- `defs/units/`: the two AT teams. Each vehicle also has a squad type
  (`role = "vehicle"`, `vehicle = "<id>"`) so call-in, cards and the AI
  reuse the squad path.
- `defs/items/`, `defs/weapons/`: `pen_mm`, `inside_damage`,
  `inside_radius` on every HE block.
- `defs/rules/skirmish.toml`: `[vehicles]` (unlock, limit, wreck cap,
  seat swap, repair, resupply, fire, spotting percents, near-spot
  distance), `[dig.profiles.trench]` and `[dig.profiles.at_ditch]`
  (replacing `[dig]`), `he_area_spread_mrad`, new `[ai]` keys
  (`mix_vehicle_pct`, `mix_at_pct`, `tank_standoff_m`, `assault_close_m`,
  `at_clear_s`, `repair_clear_m`).
- `mods/ww2/tests/rules_test_unlock.toml`: the test override
  (`unlock_s = 0`) used by the net, determinism and autoplay runs.
- `tests/ballistics.toml`: golden rows (below).

Example vehicle def (shape, not final numbers). Names follow §7.4 where
§7.4 has them (`[mobility] type`, `[crew] seats`, `[[weapons]] def` and
`mount`, `radio_mg`):

```toml
id = "ww2:ger_pz4_h"
name = "Panzer IV Ausf. H"
faction = "ww2:germany"
role = "tank"
cost = 640
pop = 12
call_in_s = 45
sight_m = 175
spot_size_pct = 200
sources = ["..."]

[mobility]
type = "tracked"
speed_cm_s = 600
reverse_cm_s = 200
turn_deg_s = 30
accel_cm_s2 = 150
brake_cm_s2 = 400
max_slope_pct = 50
ditch_block_dm = 19
bog_pct = 6
unbog_s = [20, 40]
moving_spread_pct = 400

[hull]                                   # cm, hull frame: x forward, y left, z up from the ground
box = { center = [0, 0, 108], half = [290, 140, 68] }
track_band_pct = 45
track_mm = 20

[turret]
pivot = [20, 0, 176]
box = { center = [0, 0, 45], half = [115, 95, 45] }
traverse_deg_s = 14

[armor]                                  # zones as in §7.4; slope_deg is the S3 stand-in for mesh normals
hull_front   = { thickness_mm = 80, slope_deg = 10, material = "rha" }
hull_side    = { thickness_mm = 30, slope_deg = 0,  material = "rha" }
hull_rear    = { thickness_mm = 20, slope_deg = 10, material = "rha" }
hull_top     = { thickness_mm = 12, material = "rha" }
turret_front = { thickness_mm = 50, slope_deg = 10, material = "rha" }
turret_side  = { thickness_mm = 30, slope_deg = 25, material = "rha" }
turret_rear  = { thickness_mm = 30, slope_deg = 15, material = "rha" }
turret_top   = { thickness_mm = 16, material = "rha" }

[crew]
seats = ["driver", "gunner", "commander", "loader", "radio_mg"]
[crew.boxes]
driver = { frame = "hull", center = [150, 60, 110], half = [35, 30, 40] }
# gunner, commander, loader: frame = "turret"; radio_mg: frame = "hull"

[[weapons]]
def = "ww2:kwk40_l48"
mount = "turret_main"
muzzle = [380, 0, 25]                    # from the turret pivot
elevation_deg = [-8, 20]
ammo = [{ id = "ww2:75mm_pzgr39", count = 30 }, { id = "ww2:75mm_sprgr34", count = 30 }]

[[weapons]]
def = "ww2:mg34"
mount = "coax"

[[weapons]]
def = "ww2:mg34"
mount = "hull"
arc_deg = 15
seat = "radio_mg"

[[components]]
type = "engine"
hp = 100
box = { center = [-200, 0, 100], half = [70, 100, 45] }
# ammo_rack, gun (breech box + barrel anchor), turret_ring, track_left / track_right (anchors)

[[stations]]                             # §5.12, unused until M5
id = "driver_gunner"
seats = ["driver", "gunner"]
```

Only `rha` exists as a material in S3 (multiplier 1.0). Cast armour and
the §5.7 material matrix come with the mesh model.

## `frontline-cli ballistics`

One-shot (§8.1):

```
frontline-cli ballistics --ammo ww2:75mm_pzgr39 --target ww2:usa_m4a3_75 \
    --zone hull_front --range 300 --angle 0 [--descent 0|auto|DEG] [--track-band] [--json]
```

prints the ammo, the plate (thickness, slope, horizontal angle, descent
angle, compound impact angle, effective thickness), `pen0` at that range
with its ±7 % band, and the result: `penetrates`, `does not penetrate`,
`ricochet`, or `borderline` with the exact probability (closed form from
the triangular distribution, no sampling). `--descent` defaults to 0;
`auto` uses the shell's descent angle at that range.

Also:
- `--table`: one ammo against every zone of a target, at 100 / 150 /
  300 / 450 / 600 / 1000 / 1500 m and 0° / 20° / 30° / 60°.
- `--golden mods/ww2/tests/ballistics.toml`: runs the golden rows;
  `cargo test` runs the same file through the sim's own functions. Each
  row: ammo, target, zone, range, angle, optional `descent` (default 0)
  and `track_band`, and `expect = "pen" | "nonpen" | "ricochet"` or
  `pen_pct = [lo, hi]`.

Golden rows from the starting data (flat shot, descent 0 unless noted).
The file has at least 35 rows: these 27 plus at least 8 more the build
adds (the `--table` grid is a good source).

| # | Ammo | Target, zone | Range | Angle | Expect |
|---|---|---|---|---|---|
| 1 | PzGr 39 | M4A3 hull front (63.5 @ 47 → 93.1 mm) | 150 m | 0° | pen |
| 2 | PzGr 39 | M4A3 hull front | 300 m | 0° | pen |
| 3 | PzGr 39 | M4A3 hull front | 300 m | 30° (impact 53.8°, 107.5 mm) | pen |
| 4 | PzGr 39 | M4A3 hull front | 450 m | 30° | pen_pct [90, 95] (92.3) |
| 5 | PzGr 39 | M4A3 hull front | 1000 m | 0° | pen_pct [95, 98] (96.5) |
| 6 | PzGr 39 | M4A3 hull front, `descent = "auto"` | 1000 m | 0° | pen_pct [99, 100] |
| 7 | PzGr 39 | M4A3 hull front | 1500 m | 0° | nonpen |
| 8 | PzGr 39 | M4A3 hull front | 300 m | 65° (impact 73.2°) | ricochet |
| 9 | PzGr 39 | M4A3 turret front (76 @ 30 → 87.8 mm) | 300 m | 0° | pen |
| 10 | PzGr 39 | M4A3 turret front | 1500 m | 0° | pen_pct [15, 23] (18.8) |
| 11 | PzGr 39 | M4A3 hull side (38) | 300 m | 0° | pen |
| 12 | M61 | Pz IV hull front (80 @ 10 → 81.2 mm) | 100 m | 0° | pen_pct [99, 100] |
| 13 | M61 | Pz IV hull front | 150 m | 0° | pen_pct [99, 100] (99.8) |
| 14 | M61 | Pz IV hull front | 300 m | 0° | pen_pct [86, 91] (88.4) |
| 15 | M61 | Pz IV hull front | 450 m | 0° | pen_pct [55, 60] (57.7) |
| 16 | M61 | Pz IV hull front | 500 m | 0° | pen_pct [40, 46] (43.1) |
| 17 | M61 | Pz IV hull front | 1000 m | 0° | nonpen |
| 18 | M61 | Pz IV hull front | 150 m | 20° (impact 22.3°, 86.4 mm) | pen_pct [54, 60] (57.0) |
| 19 | M61 | Pz IV hull front | 300 m | 20° | pen_pct [18, 23] (20.2) |
| 20 | M61 | Pz IV hull front | 150 m | 30° (93.8 mm) | nonpen |
| 21 | M61 | Pz IV turret front (50 @ 10) | 300 m | 0° | pen |
| 22 | M61 | Pz IV hull side, track band (30 + 20) | 150 m | 0° | pen |
| 23 | M6A3 | Pz IV hull front | 100 m | 0° | pen |
| 24 | M6A3 | Pz IV hull front | 100 m | 40° (106 mm) | pen_pct [0, 2] (0.9) |
| 25 | RPzB 54 | M4A3 hull front | 100 m | 0° | pen |
| 26 | RPzB 54 | M4A3 hull front | 100 m | 40° (impact 58.5°, 121.5 mm) | pen |
| 27 | Sprgr 34 (HE) | M4A3 hull side | 300 m | 0° | nonpen |

The percentages follow from the formula above. Row 12 clears the −7 %
edge by only 0.4 mm and row 13 sits just inside the band (99.8 %), so
both are ranges, not plain "pen": a rounding change must not flip them.
If the data changes, the golden file changes in the same commit, on
purpose.

## `frontline-cli armour-test`

Staged like `fortify-test`: squads and vehicles placed with setup tools,
AI off, real commands, seeds 1–5 × both factions. Text and `--json`
report. **Hard** checks fail the run and test mechanics only. **Report**
rows print the number and, where one exists, the owner's target next to
it. A hard balance threshold would push the build agent to tune numbers
until it passes; that is the owner's call.

The setups live in a `scenarios` module in `crates/sim`, used by both
the CLI and the Godot bridge (which does not link `sdk/crates/cli`, where
`fortify.rs` lives today). Setup tools may force vehicle state (bogged,
burning, component HP, seat HP) and placement. They run only while
building a staged world, never through a `Command`. All ranges are at or
inside 300 m, so real spotting applies.

Statistical checks pool all seeds and factions. A count is "within 3σ"
when |observed − expected| ≤ 3·√(Σ p(1 − p)) + 1.

| # | Case | Check |
|---|---|---|
| 1 | **Penetration statistics.** A tank fires at a stationary enemy tank at 150 m and 300 m, with the target at 0°, 20°, 30° and 90°; at least 100 hits per pairing. | Hard: every hit logs (pen0, c, thickness, v), and its outcome equals the rule. Hard: per pairing, pens within 3σ of the sum of each hit's own closed-form p. A unit test replays logged hits through `armour::penetrates` and the CLI path and gets identical results |
| 2 | **Hull-down.** A tank behind a crest (turret exposed) vs a tank in the open, 300 m. | Hard: hull hits ≤ 2 % of hits on the hull-down tank. Report: duel outcome |
| 3 | **Tank duels.** Panzer IV vs Sherman at 300 m frontal; at 300 m with the Panzer IV angled 30°; at 150 m with the Sherman on the Panzer IV's flank. Plus a crossing target at 6 m/s, 300 m. | Hard: where both parts give the same pen chance, ≥ 40 % of hits land on the hull; against the 30°-angled Panzer IV, the Sherman's gunner aims at the turret. Report: win rates, time to first disabling hit, hit rate on a crossing vs stationary target (target: crossing ≤ 60 % of stationary) |
| 4 | **AT team vs tank.** A tank drives past an AT team in a trench at 60 m (flank), and toward it (front). Variant: the tank knows the team's spot and fires HE from 100 m. | Hard: at least one rocket hit per run; no launcher shot at a moving tank beyond `engage_moving_m`. Report: disabling-hit rate on the flank (target ≥ 50 %), front rate, team losses in the HE variant |
| 5 | **Lone tank vs dug-in rifles (no AT)**, holding at 100 m for 3 min. | Hard: no component or seat damage from small arms; the tank survives every run. Report: defender losses, share of time pinned |
| 6 | **Lone tank vs dug-in rifles + AT team**, advancing from 300 m to 50 m. | Report (target: tank disabled in ≥ 60 %) |
| 7 | **Tank-supported assault** (the TODO problem). A rifle squad follows a tank from 150 m to 25 m, then frags, vs a dug-in rifle squad without AT. **7b:** the same squads, path and timing without the tank, in the same run, as the baseline. | Report: attacker and defender losses in 7 and 7b, share of incoming rounds stopped by the tank. Owner target printed (question 5) |
| 8 | **Satchel.** Engineers 40 m from a stationary tank (MGs live), with and without a rifle squad suppressing it; and engineers in a trench that a tank crosses at 1 m/s. | Hard: a satchel burst with a track anchor inside its lethal radius destroys that track. Report: tank disabled, engineers lost |
| 9 | **Tank HE vs infantry.** 10 HE rounds on a rifle squad, open vs trench. | Hard: trench casualties ≤ 40 % of open. Report: lip height lost per HE hit; hits needed to open a 4 m gap in the parapet |
| 10 | **Mortar vs a closed tank.** 20 mortar shells around and on a stationary tank. | Hard: no penetration, no seat or internal component damage, no knock-out. Report: tracks lost per shell |
| 11 | **Enfilade.** A tank 50 m off the end of a straight trench vs the same tank 50 m in front of it. | Report: defender losses both ways |
| 12 | **Trench crossing.** A tank crosses a finished trench at 0°, 30° and 60°, 200 times per angle per seed. Variant: the trench after 10 mortar craters on it. | Hard: exactly one bog roll per crossing; bogs within 3σ of `bog_pct`; crossing is slower than open ground; the cratered trench stays crossable (no blocked node) |
| 13 | **AT ditch.** A tank ordered across a 30 m AT ditch; then the same after satchels on the ditch wall. | Hard: it never puts a track sample on a blocked node; it goes round or reports unreachable. Report: satchels needed to open a vehicle path |
| 14 | **Stress** (`--stress`, lifts the limits: 50 vehicles per player, no pop cap, says so in the report). 100 vehicles and 600 soldiers fighting on Kessel Ridge for 2 game minutes. | Hard: sim tick P99 < 10 ms on the M3 (3 threads). Report: stage times (Ballistics separately), max `armour_box_tests`, max path expansions per tick |

## Snapshot contract

Frozen after implementation step 1 and written into
`godot/docs/match_state.md`, so the presenter can be built on
`mock_match_source.gd` in parallel. Floats appear only in the snapshot
(`snapshot.rs` rule). Sim axes: x east, y north, z up, metres, radians.

**`VehicleSnapshot`** (one per vehicle and wreck):

| Field | Type | Enemy vehicles |
|---|---|---|
| `id`, `squad` | u32 | yes |
| `type_idx` | u16 | yes |
| `owner` | u8 (255 = wreck) | yes |
| `pos` | [f32; 3] | yes |
| `yaw`, `pitch`, `roll`, `turret_yaw` (hull-relative), `gun_pitch` | f32 | yes |
| `speed` | f32, signed m/s | yes |
| `flags` | u16: MOVING, REVERSING, BOGGED, IMMOBILISED, BURNING, REPAIRING, CROSSING, WRECK, KNOCKED_OUT | only BURNING, WRECK, KNOCKED_OUT |
| `seats` | [u8; 5]: role and state (ok, wounded, dead, swapping) | zeroed |
| `components` | [u8; 6]: HP % of engine, track L, track R, gun, ring, ammo rack | zeroed |
| `repairing` | u8 component index, 255 none | zeroed |
| `ammo` | [u16; 2] AP, HE | zeroed |
| `loaded`, `ammo_choice`, `reload_pct` | u8 | zeroed |
| `target` | u32, `u32::MAX` none | zeroed |
| `followers` | u8 | zeroed |

- Enemy vehicles reach the presenter only while the player sees them;
  wrecks always. The bridge filters in `match_frame.rs`, as it does for
  soldiers.
- **`ShellSnapshot`:** `pos`, `vel` ([f32; 3]), `kind` (u8: ap, he,
  heat), `player` (u8). Sent if fired by the player's own units or by a
  unit the player sees, or within 40 m of the player's men (the S1b
  thrown-item rule).
- **`ArmourEvent`:** `kind` (u8: Fired, Ricochet, NonPenetration,
  Penetration, ComponentHit, CrewHit, Disabled, KnockedOut, Bogged),
  `vehicle` (u32), `component` (u8), `pos` ([f32; 3]), `seq` (u32, for
  later direct-control prediction). Sent for own vehicles, and for
  impacts the player could see: the target vehicle is seen, or the
  impact is within 40 m of the player's men.
- **Per player:** `vehicles_counted` (u8), `vehicle_limit` (u8),
  `unlock_tick` (u64).

## Presenter

**Placeholder art (no downloads).**
- Procedural meshes in `meshgen.rs` (the bench `tank_mesh` is a start):
  hull, turret and barrel as separate parts, under 400 triangles each.
- One MultiMesh per (type, part). Instance transforms are composed in
  Rust (hull × turret yaw × gun pitch). Turret yaw interpolates by the
  shortest arc; pitch and roll are smoothed. Wrecks reuse the meshes,
  darkened. Team colours by instance custom data.
- Effects: muzzle flash and dust, bright shell tracers, spark burst on
  ricochet and non-pen, flash on penetration, fire, cook-off, wreck
  smoke, rocket trail. Caps: fires ≤ 8, wreck smoke on the nearest 16,
  dust for the nearest 8 vehicles moving over 2 m/s. No track decals.
- Audio: four CC0 sounds (cannon, engine loop, AP impact on steel,
  rocket launch), each with a `.license.toml`. Engine loops play from a
  pool of the 4 nearest `AudioStreamPlayer3D`s, never one per vehicle.
  Total audio stays under the S1 5 MB cap.

**HUD v2 additions** (theme constants from `S1b-ui-theme.md`, no new
layout):
- **World marker** per vehicle in `unit_overlay.gd`: a type glyph plus a
  burning or bogged pip, readable at 800p.
- **Vehicle card** in the roster, in at most 2 rows within the squad
  card's maximum width, letters at the 13 px floor: name, a silhouette
  icon drawn in `_draw` from the def's box sizes (no image files), crew
  pips (D G C L R: ok, wounded, dead, swapping), a component strip
  (engine, L and R track, gun, ring), `AP 24 · HE 18` with the loaded
  round and the ammo choice marked, and a state badge (Moving, Bogged,
  Immobilised, Burning, Repairing).
- **Detail panel** for a focused vehicle: the card's data at full size.
  No penetration hints about enemies (`show_pen_hint` off, §5.12.4).
- **Orders grid** with vehicles selected: Move, Attack-move, Stop,
  Retreat, Reverse, Face, Repair, and an ammo toggle (Auto / AP / HE).
- **Call-in:** vehicle tiles with cost and pop; a lock with a countdown
  before unlock; a `2/4` badge for the vehicle limit.
- **Feed:** "Panzer IV knocked out", "Track destroyed", "Sherman bogged
  in trench", and so on. Only main-gun and rocket events.
- **Hit markers:** a short word (Ricochet, No pen, Penetrated, Crew hit)
  above an observed main-gun or rocket impact for 1.5 s, at most 6 at
  once.
- **Minimap:** vehicle icons (rectangles), wrecks as dim crosses.
- Picking: inside the projected footprint or within 14 px of the centre.
  Selection rings sized to the footprint.
- Mixed selections split as in S1b: soldiers get `*Units` orders,
  vehicles theirs. Vehicle-only keys affect only vehicles.

**New hotkeys** (S1b keys unchanged: RMB move, Ctrl+RMB attack-move, H,
R, Y, B, L, X, C, Z, G, T, F):

| Key | Action |
|---|---|
| RMB on an enemy vehicle | Attack target (`AttackUnit`) |
| RMB on a friendly vehicle with infantry selected | Follow / escort |
| Alt+RMB | Reverse move |
| O | Face: click a point to turn the hull front to it |
| N | Next ammo: Auto → AP → HE |
| J | Repair (the direct-control repair key, §5.12.2) |

`Alt`+`RMB`-drag sets aim height only inside direct control (§5.12.5), so
it does not clash in RTS mode. `1`–`4` stay control groups in RTS.

## Implementation order

One build workflow, two tracks.

**Sim track**, each step tested before the next:
1. Content: defs, schema, lowering, `validate`, the rules test override.
   Freeze the snapshot contract.
2. `armour.rs` and `ArmourTables`: plate normals, `penetrates`, ricochet,
   interpolation. `ballistics` CLI and golden tests.
3. Vehicle entity and squad-of-one checklist, pose, `drive`, vehicle
   index and frames, avoidance, block layer and `PathReq::Vehicle`,
   call-in with unlock, limit and pop, **vehicle spotting**, save, hash
   and validate.
4. Box hit tests in `fly`; gunnery, laying, aim point, lead; AP, HE and
   HEAT effects; components, crew, fire, ammo rack, knock-out, wrecks.
5. AT teams, blasts vs tracks, ground satchels.
6. Trench crossing, bogging, AT ditch profile.
7. AI, commands into the relay limits.

**Presenter track** (a worktree, on the mock from step 1): meshes,
MultiMesh batches, world markers, vehicle card, orders, call-in, feed,
hit markers, hotkeys, effects, audio.

Then:
8. Integrate: bridge snapshot and fog filter, staged autoplay scenes.
9. `armour-test`, determinism and net tests, performance.

**If the slice overruns, cut in this order:** AI satchel throws at
vehicles; fire (keep the ammo rack); `ReverseUnits` (the "target behind
and within 40 m" reverse stays); the report-only armour-test cases 10–11;
base refill of dead crew seats. Do not cut the AT ditch, the golden
tests, hash coverage or the net tests.

## Done when

Agent-checkable:

1. `cargo test --workspace --release`, clippy (`-D warnings`) and fmt
   pass. They include:
   - unit tests: plate normals and compound angles; `penetrates` at
     table ends, between rows, at the ricochet edge and for c ≤ 0; box
     slab hits (each face, edges); earliest-hit order of terrain, box and
     capsule with the tie rule; the terrain re-test; aim-point choice;
     spall determinism; crew swap; component effects; fire and ammo rack
     rolls; knock-out clears followers and targets; wreck cap and id
     reuse; one bog roll per crossing; vehicle path costs, block layer
     and dilation; craters never block; `drive` refuses blocked ground;
     AT ditch profile; infantry cross an AT ditch; push-out spares men
     in a trench; `*Units` validation with vehicle ids; rejected orders
     for vehicle squads; vehicle limit, unlock and pop; `check_defeats`
     with only vehicles left; each `VehicleState` field changes the hash;
     `MatchState::validate` rejects a broken vehicle; the relay rejects
     121 ids for every new variant;
   - AI-vs-AI skirmish determinism **with vehicles** (test unlock
     override): the same hashes at 1/2/3/8 threads, and through save →
     load → continue at staged moments: a shell in flight, a burning
     vehicle, a bogged vehicle, a crew swap in progress, a pending
     vehicle path request.
2. `frontline-cli ballistics` exists with one-shot, `--table`, `--golden`,
   `--json` and `--help`. `mods/ww2/tests/ballistics.toml` has at least 35
   rows including all 27 above, and `cargo test` runs it through the
   sim's own functions.
3. `frontline-cli validate` passes on `mods/ww2`. A broken armour zone, a
   missing ammo id, a non-zero `drag_coeff` or a seat box outside its
   frame gives an actionable error. `frontline-cli schema` exports the new
   def types.
4. **Net:** `crates/net/tests/loopback.rs` gains a vehicle run with the
   test unlock override (seeds 1–3, 3 game minutes, 0–80 ms jitter, 5 %
   drop) with scripted human commands (every new command) alongside AI
   vehicles: identical hashes, every `seq` executed exactly once, and at
   least 2 vehicles per side spawned (hard). The 36,000-tick soak with
   vehicles passes once (noted in the slice log). `PROTOCOL_VERSION` and
   `SAVE_VERSION` are bumped. `desync-diff` names vehicle fields (checked
   with `--debug_inject_desync` on a vehicle order).
5. `frontline-cli skirmish --ai-vs-ai --seed N` for seeds 1–5, normal
   rules (unlock at 4:00):
   - hard: a winner inside 30 game minutes; each AI fields at least one
     vehicle and one AT team; at least one penetration per seed;
   - report: vehicles knocked out; kills by weapon class (rifle, SMG, MG,
     mortar, tank AP, tank HE, vehicle MG, rocket, satchel, frag);
     vehicle stats (bought, lost, bogged, repaired); disables by cause
     (penetration, track, fire, ammo rack); manpower lost per manpower
     killed for tanks and AT teams; AI counters (escorts started,
     escorted assaults that reached 30 m of an enemy trench, AT shots,
     satchels at vehicles, repairs, retreats); the map-side win split;
   - flag: if mortar plus tank-HE kills exceed 60 % of all kills, the
     report prints `FLAG: HE share` and the slice log discusses it.
6. `frontline-cli armour-test` passes every hard check and prints every
   report row, for seeds 1–5 × both factions.
7. **Godot:** the game boots. Staged autoplay scenes
   (`--autoplay_scene armour_<name>`, loading a `scenarios` setup) run
   with no errors. Each asserts its snapshot condition before capture
   (for example `BOGGED` set) and exits non-zero otherwise. Screenshots go
   to `docs/slices/screens/`: `s3_callin_locked`, `s3_vehicle_card` (ammo
   toggle on HE), `s3_tank_firing`, `s3_penetration_marker`,
   `s3_bogged_in_trench`, `s3_at_ditch`, `s3_at_team_firing`,
   `s3_burning`, `s3_escort`. A plain `--autoplay` vs AI also runs for at
   least 7 game minutes with no errors and at least one vehicle per side
   drawn (asserted from the snapshot).
8. **Two Godot instances**, 3 minutes of net autoplay with the test
   unlock override: all hashes agree, and at least 2 vehicles per side
   spawned.
9. **Performance (M3, headless):** AI-vs-AI skirmish at peak with
   vehicles: sim tick P99 ≤ 1.5 ms (hard; the S2 peak P99 was
   0.33 ms). `armour-test --stress`: P99 < 10 ms (hard). CPU time in
   `test_match_view.gd` to build the vehicle buffers, picking and rings
   for 16 vehicles is under 0.2 ms (hard; GPU cost is not measured, as
   noted).
10. **HUD fit:** at 1280 × 800 with 4 vehicles and 8 squads, no roster
    cards overlap; the visible card count is reported.

Owner checklist (needs a person; not blocking the agent):

11. Play a match vs AI to victory using tanks, an AT team and an AT
    ditch. Note whether turret speeds, bogging, the unlock time and the
    Sherman's cost feel right.
12. One net match with the friend where both sides field vehicles.
13. Read armour-test case 7 against the target (owner question 5) and
    pick the next lever if needed.

## Design-doc changes to propose (not applied here)

- §5.7: S3 armour is per-face plates on oriented boxes with authored
  slopes; the mesh narrowphase replaces it at M2 behind the same
  `hit_armour` function. Ricochets end the shell until secondary
  projectiles exist. Penetration is a pure cosine model until then.
- §7.4: vehicles in `defs/vehicles/`, ammo in `defs/ammo/`; `slope_deg`
  on armour zones until meshes exist; `[crew.boxes]`, `[hull]` and
  `[turret]` boxes, `pop` and `role`; mobility in cm/s.
- §5.5: vehicles are not ORCA agents; soldiers avoid them as moving
  obstacles with full responsibility.
- §5.10.4 "Obstacles": vehicles are blocked by AT-ditch geometry
  (`ditch_block_dm` inside an AT-ditch footprint) and wrecks, not by
  `trench_crossing_m` and not by depth alone, until nav layers are baked.
- §5.10.9: AT ditch pacing follows our trench pace (10 m, 5 engineers,
  about 4 min).
- §2.2: `HE_RESERVE` becomes a 64-slot shell reserve.
- §13 / M3: `armour::penetrates` and `ArmourTables` must be in the
  `sim-ffi` interface, since the CLI reaches the sim only through it from
  M3 on.

## Alternatives rejected

Review points not adopted, and why:

- **Aim at the hull centre whenever it is visible** (gameplay 1). With
  these tables a Sherman would keep firing at an angled Panzer IV's hull,
  which it cannot beat. The gunner instead picks the visible part with
  the better chance, hull on near-ties. Hull armour and angling still
  matter, and case 3 checks both.
- **A test-only `force_visible` setup tool** (scope 3). Keeps tests off
  the real spotting path. Moving all cases to ≤ 300 m does the same job
  with no new hook.
- **Ballistics stage ≤ 0.3 ms P99 as a hard stress check** (perf 20). No
  measurement backs the number. The stage time is reported; the hard bars
  are the 1.5 ms skirmish P99 and the 10 ms stress P99.
- **Raise the pitch clamp to 32° and keep 60 % slopes** (perf 15, one
  option). Chose 50 % slopes and a 30° clamp: on a 1 km map with one
  ridge, steeper climbs add little.
- **Block vehicles only near finished AT-ditch lines** (perf 5, as
  written). A ditch footprint plus depth instead: a ditch blocks as soon
  as a section is deep enough, and a satchel-cut section opens by
  geometry. Same protection against shelled trenches.
- **Report nodes deeper than the block depth after a barrage, then decide**
  (scope 14). Moot: depth alone no longer blocks. Case 12 checks a
  cratered trench stays crossable.
- **Cut AI satchel throws with the rest of S3b** (scope 1). Kept: ground
  satchels stay in S3, the rule is a few lines on the S1b throw code, and
  it is first on the cut list.
- **An AI-driven variant of armour-test case 7** (gameplay 3). The
  skirmish report counts escorted assaults that reach 30 m of an enemy
  trench, which shows the same AI failure without another staged case.
- **AI digs its AT ditch 60–80 m in front of the trench; a case for a
  ditch in front of a trench** (gameplay 4). AI ditch digging moved to
  S3b. The part that matters now is adopted: AT-ditch cells get no trench
  path discount.
- **A penetration-chance cursor** (gameplay 12). §5.12.4 decided no pen
  cursor in PvP. A vs-AI-only hint can be its own small task. The "Crew
  hit" marker is adopted.
- **Launchers fire HEAT at infantry in the open** (gameplay 14). AT teams
  would become a second HE source in a game where HE already dominates.
- **Replace the Jagdpanzer IV with a second Panzer IV variant**
  (gameplay 19). Both tank destroyers move to S3b together; a variant adds
  content work without testing anything new.
- **Vehicle hull-down pit now, or 60 m AT ditches** (gameplay 17). A pit
  needs a non-line stamp shape, which the line-based dig code does not
  have; S3b. Players can chain 30 m ditch jobs.
- **Tank fear as an S3 owner question** (gameplay 20). Named as the next
  lever after case 7's numbers, not built or asked now.
- **A second case 7 baseline under smoke** (gameplay 18). S1b's
  `fortify-test` already has the smoke numbers.
- **Ctrl+Alt+RMB for reverse** (gameplay 22). Alt+RMB-drag is a
  direct-control binding only (§5.12.5); there is no clash in RTS mode.
- **Keep sticky satchels above fire in the cut order** (gameplay 23).
  Sticky satchels moved to S3b with the scope cut. Ground satchels still
  break the tracks of a tank crossing a trench at 1 m/s.
- **Cut only the tank destroyers** (perf 27). Not enough: with TDs alone
  out, S3 is still about twice S1b. Sticky satchels, bail-out crews and
  AI ditch digging go too.

## Owner questions

Work proceeds on the (A) defaults until the owner answers.

1. **S3 scope.** **(A) Tanks and AT teams now; tank destroyers
   (Jagdpanzer IV, M10), sticky satchels, bail-out crew squads and AI
   AT-ditch digging in S3b** (recommended: the draft was about 2–3× S1b,
   and even this cut is the biggest slice so far) / (B) the full draft
   scope in one slice, accepting that the cut list will decide what
   ships.
2. **Vehicle spotting range.** **(A) 2× the observer's sight, as
   drafted:** tanks see tanks at about 350 m (525 m after firing), so
   fights happen at 100–400 m, where both 75 mm guns beat most fronts and
   angling and aim point decide (recommended: fits the 1 km map and the
   150 m infantry sight) / (B) about 4× (600–700 m): the range tables
   matter, the Panzer IV's edge grows, AT teams meet tanks less often,
   and Kessel Ridge may feel small.
3. **Do standard trenches stop tanks?** **(A) No: they slow tanks to
   1 m/s and can bog them; the AT ditch stops them** (recommended:
   matches real crossing widths, keeps digging meaningful against
   armour) / (B) any finished trench blocks vehicles (simpler, less real,
   makes every trench line a tank wall).
4. **Unlock and limit.** **(A) Vehicles from 4:00, at most 4 alive or
   queued per player, 12 pop each** (recommended: the opening stays
   infantry, and 4 tanks use 48 of 120 pop) / (B) no unlock, cost only /
   (C) other numbers.
5. **Target for "tanks fix the trench assault"** (armour-test case 7,
   report only). **(A) Attacker losses at most half the no-tank
   baseline's, and defenders lose at least 75 %** (recommended: the
   draft's bar; printed next to the numbers so you can judge) / (B) a
   different bar. If the target is missed, you pick the next lever: tank
   fear, mortar smoke, or stronger pinning.
