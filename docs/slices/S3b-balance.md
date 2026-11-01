# Slice S3b-balance: physical damage, armour balance, trench pinning

**Context (2026-11-01).** S3 (tanks, AT teams, armour) is done and
verified on `s3-armour` (6a0402d). It works but plays badly: the USA wins
almost every AI match, almost every knock-out is an ammo-rack detonation,
AT teams barely disable anything, the AI hardly uses rockets or satchels,
and tanks do not help a trench assault. This slice fixes that, on branch
`s3b-balance`.

Read first: `docs/slices/S3-log.md`, `docs/slices/S3-armour.md` (this file
wins where they differ), `docs/TODO.md`, `docs/slices/S1b-hud.md`.

**Owner decisions in force (2026-11-01):**
- Work on the (A) default of every open owner question; record new
  questions in `docs/TODO.md` with an (A) default and go on.
- **Trench assault:** keep the pinning fire, informed by how Gates of
  Hell does it. Pinning is rules data in `mods/ww2` TOML (reversible by
  data). **No** mortar smoke, **no** tank fear.
- **Physical contact:** damage only when a projectile (round, shell,
  fragment, jet, spall) actually touches the model. No "unit is under
  attack" modes. Suppression, too, comes only from real projectiles.
- **Balance order:** (a) fix unrealistic geometry and behaviour, (b)
  costs, timing, quantities and AI, (c) historically accurate options,
  (d) only then stats, documented as a deliberate deviation. Asymmetry
  is fine. Every data change cites its source or reasoning in a TOML
  comment.
- **Quick iteration:** every gameplay tuning value added or touched lives
  in `mods/ww2` TOML with a comment (unit, meaning, source). Engine and
  structural constants (pool sizes, substep rate, hash chunking,
  performance caps) stay in code.

Every number below is a starting value for the Tune stage unless it is a
measured baseline.

## Hard rules (unchanged from S3, restated)

- Deterministic sim: Q16.16, no floats in sim logic, no HashMap, counter
  RNG with per-purpose keys, ascending-id iteration. All new state saved,
  loaded, hashed (a test per field) and validated on load.
- `SAVE_VERSION` 9 → 10 and `PROTOCOL_VERSION` 3 → 4, **once** for the
  slice. The relay's `command_in_limits` gets the new command (bounded by
  `MAX_UNITS` ids and at most 2 points).
- Keep every S1/S1b/S2/S3 test passing (net loopback and soak included),
  clippy `-D warnings`, `cargo fmt`, `fortify-test`, golden ballistics
  (rows change only with a justified data change, named in the log).
- `CARGO_BUILD_JOBS=4`; no extra target dirs; sockets on 127.0.0.1; never
  write the Godot `settings.cfg`; kill every Godot/test process started.

## Baseline (measured on `s3b-balance` = S3 + report-only counters, 66d55a8)

Commands: `frontline-cli skirmish --ai-vs-ai --seed N --threads 2` for
N = 1–10, normal bases and `--swap-bases` (the 20 matches the targets
use), plus a faction-slot swap (`--faction0 ww2:usa --faction1
ww2:germany`, 10 matches) and a no-tank control (the same 20 matches with
`vehicles.unlock_s = 3600`, from a scratch copy of the mod).
`frontline-cli armour-test --threads 3 --stress` (seeds 1–5 × both
factions) and `frontline-cli fortify-test`. Machine load average about 3–4
while measuring (other runs in parallel), so timings are a little high.

New report-only counters (commit 66d55a8): per player gun hits and
penetrations, HE shells on plates, rocket hits and penetrations,
components destroyed by rockets, hits by plate; knock-outs by cause;
duel knock-out causes and time to first shot (case 3); tank rounds and
time on the fire step (case 5); defender posture (case 7).

### AI vs AI

| | Normal bases (GER base 0) | Swapped bases (GER base 1) | Total (20) | Faction slots swapped (USA p0, base 0) | No tanks (20) |
|---|---|---|---|---|---|
| Wins GER / USA | 0 / 10 | 4 / 6 | **4 / 16** | 0 / 10 | 9 / 11 |
| Winner base 0 / 1 | 0 / 10 | 6 / 4 | 6 / 14 | 10 / 0 | 11 / 9 |
| Tanks bought GER / USA | | | 56 / 63 | 25 / 32 | — |
| Tanks lost GER / USA | | | **32 / 20** | 16 / 9 | — |
| Knock-outs: rack / crew / fire | | | **47 / 5 / 0** (90 % rack) | 24 / 1 / 0 | — |
| Penetrations / non-pen / ricochet | | | 161 / 5 / 4 | 95 / 3 / 2 | — |
| Main-gun hits (pen) GER / USA | | | 60 (59) / 72 (67) | 49 (46) / 29 (28) | — |
| Hits by plate, USA guns+rockets | | | turret 60, hull 28 | | — |
| AT rockets fired GER / USA | | | 32 / 21 | 25 / 1 | — |
| Rocket hits (pen) GER / USA | | | 22 (21) / 16 (14) | 21 (20) / 1 (1) | — |
| Components destroyed by rockets GER / USA | | | 0 / 4 | 6 / 0 | — |
| Tanks knocked out by rockets | | | 1 | 2 | — |
| Matches with no rocket fired | | | **9 of 20** | 4 of 10 | — |
| Satchels thrown at vehicles | | | **0** | 0 | — |
| Bogs | | | 0 | 0 | — |
| HE share of kills, mean (range) | | | 39.5 % (21.7–45.5) | 44.5 % (32.5–**63.7**) | 49.8 % |
| Match length (min) | | | 13.6–26.2 (one over 25) | 12.9–29.9 | 12.6–24.3 |
| Peak tick p99 (ms) | | | 0.28–0.50 | 0.27–0.62 | — |

Over all 30 tank matches Germany lost 48 tanks and the USA 29. The US
side killed 3,700 men and lost 3,229; its tank HE killed 350 men (German
119) and its vehicle MGs 532 (German 339). Disabled components (all 30):
German turret ring 57, gun 51, rack 42; US turret ring 35, gun 33, rack
27.

**Reading.** Without tanks the match is even (9 / 11, base 11 / 9). With
tanks the USA wins 26 of 30 whatever base or slot it has. The imbalance
comes from the tanks, not from the map.

### armour-test (seeds 1–5 × both factions)

| Case | Baseline |
|---|---|
| 1 | 1,040 hits, 0 mismatches; σ 0.0–0.6 (nearly empty 3σ check) |
| 2 hull-down | 0 hull hits of 16; hull-down won 3, open 5, both 2 |
| 3a frontal 300 m | Panzer IV 4, Sherman 6. All 10 knock-outs ammo rack. First shot 0.6 s for both. Panzer IV hits 10 hull / 0 turret, Sherman 0 hull / 16 turret |
| 3b Panzer IV angled 30° | Sherman 10 / 10. **The Panzer IV never fired** (its turret starts on the hull line, 30° off; 14°/s); Sherman first shot 0.6 s, first disabling hit 1.1 s. KOs: crew 4, rack 6 |
| 3c Sherman on the flank, 150 m | Sherman 10 / 10, Panzer IV never fired; 10 rack KOs |
| 3d crossing vs stationary | crossing 54/65 (83.1 %), stationary 70/70: **83.1 % of stationary** (target ≤ 60 %) |
| 4 AT team | flank disabling-hit rate **42.9 %** (12 of 28); front 17.4 %; HE variant 9.1 % |
| 5 lone tank holding at 100 m | 0 of 80 killed, 0 % pinned; **the tank fired 0 rounds**; defenders 0 % of the time on the fire step |
| 6 tank + AT, 300 → 50 m | **disabled 0 of 10**; 60 rockets, 58 hit, 56 penetrated |
| 7 tank-supported assault | attackers 74, defenders 55 of 80 (68.8 %); 7b no tank 80 / 0. Defenders pinned 1.4 % of the time, up on the fire step 21 %, mean suppression 5.2 points (7b: 0 %, 7.7 %, 0.0) |
| 8 satchels | 40 m with rifles: 2 of 10 disabled, 50/50 engineers lost; MGs live: 0 of 10; tank crossing a trench: 10 of 10, 0 lost |
| 9 tank HE | trench 9 vs open 80 men lost; **lip lost 0.001 m per burst**, no 4 m gap in 10 runs |
| 10 mortar vs tank | no internal damage; 3 tracks over 199 shells |
| 11 enfilade | 75 of 80 from the trench's end, 0 of 80 from the front |
| 12 crossing | 386 bogs of 6,000 (expected 360, σ 18.4); 12.5 s vs 6.9 s over 32 m |
| 13 AT ditch | went round 10 of 10; satchels opened it in 2 of 10 (7 each) |
| 14 stress | p99 2.34 ms, max 4.70 (S3 log: 1.33 at lower load); 42 box tests per tick max; 11,694 path expansions max |

All hard checks pass (exit 0).

### fortify-test (5 seeds × both factions)

Rifles: trench 0, open 80 (pass, limit 50 %). Mortar: trench 28, open
74, **37.8 %** (limit 40 %, close). Grenade assault from 60 m: defenders
33, attackers 80 (rifle-only: 3 / 80); from 25 m: 80 / 24; smoke from 60
m: 61 / 75. Smoke: pass.

## Diagnosis

Line numbers are on 66d55a8.

### D1. The USA wins with tanks (26 of 30), Germany loses 48 tanks to 29

Not the map (the no-tank control is even). Causes, largest first:

1. **Radial "filler" damage makes every turret penetration a near
   knock-out.** `damage.rs:449` (`within`, `damage.rs:955`): an APHE
   filler deals 80 to **every** box within 1.2 m of a point 1 m past the
   entry, through other boxes. A Sherman round into the Panzer IV's
   turret front (centre at hull z 2.21 m) reaches the gun breech, gunner,
   loader, commander and the turret ring at once (ring box z 1.56–1.76 m,
   45 cm away), so the ring (80 HP) and the gun (80 HP) are destroyed and
   three crew take 80 each plus spall. That is why German disabled
   counts are dominated by turret ring (57) and gun (51).
2. **The gunner aims at the weakest plate, not the centre of mass.**
   `gunnery.rs:150` `aim_choice` picks the part with the better
   penetration chance unless the hull is within `aim_hull_margin_pct`
   (10). The M61 has 100 % on the Panzer IV turret (50 mm) and 88 % on
   its hull front at 300 m, so the Sherman always shoots the turret (60
   of 88 US hits on turrets), where finding 1 multiplies the damage. The
   Panzer IV, which penetrates both parts of a Sherman, shoots hull
   centre. Real gunners were taught to aim at the centre of the visible
   target; the turret is the smaller target.
3. **The ammo rack is a slab across the whole hull.** Both racks are one
   box `half = [50, 110, 25]`: 1 m long, **2.2 m wide** (the full inside
   width), z 0.45–0.95 m (`ger_pz4_h.toml`, `usa_m4a3_75.toml`). Any
   core or spall ray through the middle of the hull crosses it. A core
   (100) destroys it (60 HP) and a destroyed rack always detonates
   (`damage.rs:553`); spall (35 ≥ `rack_min_damage` 30) rolls 35 % per
   ray. Hence 90 % rack knock-outs and one-shot duels.
4. **Staging artefact in 3b and 3c.** The "angled" Panzer IV starts with
   its turret on its hull line, 30° off the Sherman, and needs 2.1 s at
   14°/s; the Sherman disables it at 1.1 s. Angling doctrine is "hull
   angled, turret on the threat". 3c is a flank shot, which the flanker
   should win.
5. **The Sherman has more HE (40 vs 30) and is cheaper (560 vs 640),**
   so the USA fields more tanks (95 vs 81 in 30 matches) that kill more
   infantry (HE 350 vs 119, MG 532 vs 339). This is cost balance (step
   b) once 1–4 are fixed.
6. The turret traverse (14 vs 24°/s) decides only when the turret is off
   target (3b, 3c, AI flank contacts). Historical values; not a lever.

### D2. Almost every knock-out is a rack detonation (90 %)

Finding D1.3 plus: detonation is the only rack outcome. Historically
most penetrations that reached stowed rounds started a propellant fire
that burned the tank out within seconds to a minute (crews often got
out); prompt detonations were the minority. There are no crew knock-outs
in practice because `damage.rs:569` needs **every** seat dead; a tank
with one living crewman fights on. Fire knock-outs are 0 because engine
fires need a ≥ 40 hit on the rear engine box, roll 30 %, and a living
crew puts them out with 25 % every 4 s.

### D3. AT teams disable little (case 6: 0 of 10 with 56 penetrations)

- Panzer IV driver and radio-operator boxes span x 1.15–1.85 m; the hull
  front face is at 2.90 m; the HEAT jet reaches 1.0 m (to 1.90 m). Case
  6 hits are frontal, so the jet stops 5 cm short. Real layout: the
  front plate (Fahrerfrontplatte) sits about 0.6 m behind the nose, and
  the driver and radio operator sit right behind it, with the gearbox
  between them. In our box model the front face *is* the nose, so the
  crew boxes are about 0.35–0.4 m too far back, and the gearbox is
  missing.
- Rockets aim at the hull centre (y = 0), which passes **between** the
  driver (y 0.30–0.90) and the radio operator (−0.90 to −0.30): the box
  model has nothing there. Really the gearbox and final-drive train sit
  there; a hit through it immobilises the tank.
- Case 4 flank (42.9 %): a side jet goes 1.0 m in from y = ±1.40 m, to
  ±0.40; the slab rack (now) and the turret crew (higher) are hit or
  missed by luck.

### D4. Crossing hit rate 83 % of stationary

`gunnery.rs:212` `solution` leads by the target's exact 1 s average
velocity; at constant speed the lead is perfect, so only the spread
×3 (`moving_target_spread_pct`) misses, against a 5.8 m long side at
300 m. Real first-round hit rates on moving targets were far lower, and
the error was mostly in the **lead** (speed and angle estimation), not
in the gun's dispersion.

### D5. Tank HE barely erodes a parapet (0.001 m per burst)

`terrain.rs:842` `crater_with`: an HE crater (0.6 m radius, 0.3 m deep)
lowers the bowl but also raises a rim of depth/4 around it, and the
parapet is loose spoil that should be thrown about. In case 9 only 9 of
43 rounds burst within 3 m of the trench (the rest hit men above it or
overshoot), so the per-run mean is also diluted.

### D6. The AI barely fires rockets and never throws satchels at vehicles

- AT teams only *defend own points* (`ai.rs` `assign`, AntiTank bonus
  150 for points under armour threat), in the trench. The AI's own tanks
  hold `tank_standoff_m` 160 m from points where enemy AT was seen,
  outside both launchers' 150/120 m. So both AIs keep AT and tanks apart:
  9 of 20 matches have no rocket at all.
- Satchels: only engineers carry one (the leader), the throw range is
  3–8 m, and the rule (`ai.rs:1534`) only fires when a slow enemy
  vehicle happens to be within 8 m. Nothing sends engineers toward a
  stopped or bogged tank. 0 in 30 matches.
- No bogs: AI tanks stop 40 m short of enemy trenches and never cross.

### D7. Trench assault: tanks do not pin men in trenches

- Case 5: the tank fired **0 rounds** in 3 minutes. The defenders have
  nothing to shoot at (small arms ignore vehicles), so they stay below
  the lip, where the tank cannot see them (trench 60 % of 175 m, and the
  parapet blocks the line). Nothing fires, nothing is pinned.
- Case 7: defenders spend 21 % of their time up on the fire step and are
  pinned 1.4 % of it (mean 5.2 points). The tank's MGs fire only at men
  they see, i.e. men already on the step; when a man drops, the MG loses
  him. Near misses on covered men are halved (`combat.rs:1537`). A man in
  a trench keeps rising to fire at any suppression below the pin
  threshold of 70 (`combat.rs` `status`, the `in_trench` branch only
  checks `pinned`). So the escort walks into aimed fire.
- What is missing is (1) **fire at the trench without a spotted
  target** and (2) **a duck state** for men in cover below the pin
  threshold.

## Damage audit (PHYSICAL-CONTACT RULE)

Every path that changes soldier HP, kills a man, damages a vehicle
component or crewman, or adds suppression. "Physical" = only a real
projectile, fragment, jet or spall ray touching the body or box causes
it; a close-in blast with line of sight is the one allowed exception
(overpressure, about the lethal radius).

| # | Path | Where | Physical today? | Planned change |
|---|---|---|---|---|
| 1 | Small-arms round hits a soldier capsule | `combat.rs:1201` (capsule test in `fly`), applied `combat.rs:1508` | Yes | None |
| 2 | Main-gun AP hits a soldier (300 dmg) | `fly` capsule hit, `combat.rs:1297`, then #1 | Yes | None |
| 3 | Hit suppression (`hit_suppression`) | `combat.rs:1514` | Yes (on a hit) | None |
| 4 | Near-miss suppression (round passes within `near_miss` of the body, or strikes ground within it) | `combat.rs:1254–1266` (flight time), applied `combat.rs:1529–1543`, halved when covered `combat.rs:1537` | Yes | Halving moves to TOML (`cover_suppression_pct`); feeds the duck state |
| 5 | **HE burst damage to soldiers** (shell, rocket, tank HE): every enemy whose body point has terrain LOS from the blast within `damage_radius` takes falloff damage | `combat.rs:1381–1421` `burst` | **No** (radial; vehicles, wrecks and other men do not block) | **Fragment rays** + close-in overpressure (Part A1) |
| 6 | HE burst suppression within `suppression_radius`, halved behind earth | `combat.rs:1405–1410` | Allowed (a real burst near the man) | Keep; halving to TOML (`burst_cover_pct`) |
| 7 | Thrown frag and satchel bursts | `combat.rs:1601–1640` → `burst` | **No** (as #5) | Fragment rays + overpressure (A1) |
| 8 | Blast near a vehicle damages tracks (anchor within lethal radius, terrain LOS) | `damage.rs:592–667` `blast_tracks`, LOS `damage.rs:639` | Partly (radial overpressure with LOS) | Keep as overpressure only within `blast_radius_cm` with LOS; fragments hitting a hull side's track band also damage that track (A1) |
| 9 | Round strikes the track band → `track_damage` | `damage.rs:410` | Yes (the round touches the band) | None |
| 10 | Penetration core crosses seat and component boxes | `damage.rs:426` | Yes (ray) | Length per ammo stays data; HEAT reach (A4) |
| 11 | Spall rays from the entry point | `damage.rs:435` | Yes (rays) | None |
| 12 | **APHE filler**: every box within 1.2 m of a point 1 m past entry | `damage.rs:449`, `within` `damage.rs:955` | **No** (radial, through boxes) | **Filler fragment rays** from the burst point, first box hit per ray (A2) |
| 13 | **HE beats a plate**: inner burst to boxes within `inside_radius` (mortar, tank HE; satchel data unused) | `damage.rs:457–470` | **No** (radial) | Inner fragment rays from the contact point into the hull (A2) |
| 14 | **Non-penetration**: external parts with an anchor within 1.0 m of the impact take `nonpen_damage` (tracks, barrel) | `damage.rs:473–489`, `EXTERNAL_REACH_MM` `damage.rs:81` | **No** (proximity, not contact) | Barrel becomes an external box the round must strike; tracks only via the band (#9) (A2) |
| 15 | Crew damage from non-penetrating hits | none (`damage.rs:473` touches external parts only) | n/a | Stays none |
| 16 | Engine hit starts a fire | `damage.rs:546` | Yes once #12/#13 are rays | Ammo-fire split (A3) |
| 17 | Ammo-rack hit or destruction detonates | `damage.rs:553` | Yes once #12/#13 are rays | Detonation vs ammo fire (A3) |
| 18 | Fire burns the engine, knocks out after `fire_burnout` | `damage.rs:862–880` | State after a physical crossing | Keep; ammo fires too (A3) |
| 19 | Knock-out kills the remaining crew | `damage.rs:725` | Consequence of #10–#18 | Keep (bail-out stays out, owner Q) |
| 20 | Mortar shell on a plate (8 mm, never penetrates) | `damage.rs:378` | Yes | Inner burst as #13 if it ever penetrates |
| 21 | Satchel `inside_damage`/`inside_radius` | `geballte_ladung.toml`, `m37_satchel.toml` | Dead data (thrown items never strike a plate) | Keep the fields; documented as reserved for deck satchels |
| 22 | Small arms on a vehicle stop with no damage | `combat.rs:1544` | Yes | None |
| 23 | Repair cancelled when hit | `damage.rs:302`, `damage.rs:651` | Yes (on a blow) | None |
| 24 | Suppression decay, crouch, pin | `combat.rs:1777–1779` | State from the meter | Duck state added (B1) |
| 25 | AI threat, armour threat, `last_attacker` | `ai.rs:635`, `ai.rs:811`, `damage.rs:304` | Decisions and credit only; no HP or suppression | None. No new "under fire" or "targeted" flag may feed damage or suppression |

New paths this slice adds, all physical: fragment rays (#5, #7, #8),
filler and inner rays (#12, #13), barrel strikes (#14), suppressive area
fire (real rounds, feeding #4 and #6).

**Status after Part A (2026-11-01):** #5, #7 → overpressure within
`blast_radius_cm` with a clear line plus fragment rays (`combat.rs`
`MatchState::burst`, `fragments`); #8 → overpressure on track anchors
within `blast_radius_cm` plus fragment strikes on a track band; #12 →
filler rays, #13 → inner rays (`damage.rs` `ray_burst`); #14 → the barrel
box (`armour::BoxPart::Barrel`), no proximity damage; #16–#18 → rack
detonation or ammo fire (`VehicleState::ammo_fire`, knock-out cause 3), and
#19 with `crew_ko_below`. Every path that changes HP is now a contact or
the close-in overpressure with a line of sight.

## Fixes and measurable targets, per problem

### P1. Tank duels and the AI-match imbalance (D1)

**(a) Realism first**
- **Aim at the centre of visible mass** (A5). For each part (hull,
  turret) whose centre has a clear line of fire, weight its centre by
  its projected area toward the shooter (box half-extents and the
  direction, dot products only, no trig); aim at the weighted centroid.
  If the centroid itself is not in a clear line (partial hull-down),
  aim at the visible part's centre. Launcher gunners use the same rule.
  `aim_hull_margin_pct` is removed (validate rejects it with a message).
  Effect at 300 m frontal: aim ≈ 1.4 m up on a Panzer IV, so most shots
  strike the upper hull front, some the turret.
- **Filler and inner bursts as rays** (A2): a turret penetration no
  longer wipes the whole turret plus the ring.
- **Rack geometry and outcome** (P2).
- **Staging fix**: case 3b starts the angled Panzer IV with its turret
  laid on the Sherman (doctrine). New case 3e: Panzer IV on the
  Sherman's flank at 150 m (mirror of 3c).

**(b) Costs, quantities, AI**
- AI angling (B6): an idle AI tank facing an enemy tank turns its hull
  `ai_face_angle_deg` off the line (per vehicle def; Panzer IV 25,
  Sherman 25) while its turret stays on target. Both sides use it; the
  Panzer IV gains most, as historically.
- Then, if the 20-match win rate is still outside 35–65 %: tank cost
  first (Sherman 560 → up to 600, the draft's figure; Panzer IV 640 →
  down to 600), then HE load (Sherman 40 → 30 HE + 10 AP, matching the
  Panzer IV's 30), each with a TOML comment. Tune may move costs within
  ±15 % without asking (owner question 10 (A)).

**(c) Historical options**
- **PzGr 40 (APCR) for the Panzer IV**, 6 rounds, a third ammo entry
  (A6). Approximate penetration at 30°: 100 m 120 mm, 500 m 97 mm, 1000
  m 77 mm (placeholder; the L/48's published tables; citations are an
  owner/M2 task). Auto loads it only against a target whose AP
  penetration chance is under 50 % and the APCR's is higher. Honest
  note: the KwK 40's PzGr 39 already defeats a Sherman everywhere at our
  ranges, so this changes little; it is built because it is small and
  historical, and it covers later heavier targets.
- **Towed AT guns** (Pak 40, 57 mm M1): proposed for a follow-up slice
  (S3c), not built here (towing, crews, gun shields, AI placement).

**(d) Stats** only if (a)–(c) leave a severe imbalance; any change is
logged as a deliberate gameplay deviation in the TOML comment and the
slice log.

**Targets:** 20 AI matches each faction wins 35–65 % (7–13), no base
side above 70 % (14); 3a frontal 300 m: Panzer IV wins 6–8 of 10; 3c and
3e: the flanker wins ≥ 7 of 10.

### P2. Rack detonations dominate (D2)

**(a) Realism.**
- **Rack geometry from the real stowage** (A3, content):
  - *Panzer IV H* (87 rounds): bins low along both hull sides of the
    fighting compartment and beside the radio operator; none in the
    turret. Model: three ammo-rack boxes (left side, right side, by the
    radio operator), e.g. left `center = [-20, 95, 70], half = [55, 30,
    25]`, right mirrored, front-right `center = [110, -95, 80], half =
    [25, 25, 30]`.
  - *M4A3 (75)*, dry stowage (owner question 4 (A)): sponson racks above
    the tracks in both hull sides under the turret, plus ready rounds on
    the turret basket floor. Model: left sponson `center = [-30, 105,
    150], half = [60, 22, 25]`, right sponson mirrored, basket floor
    `center = [-10, -40, 60], half = [30, 30, 15]`.
  - Sources: Jentz & Doyle, *Panzer Tracts 4-3*; Hunnicutt, *Sherman*;
    TM 9-759. Positions are approximations within our box model; exact
    citation is an owner/M2 task, as in S3.
  - Needs one engine change: a component may have **several boxes**
    (`boxes = [...]`, one HP pool). `inner_boxes` lists each box with
    the same `Inner::Part`; the 14-box cap stays (Panzer IV: 5 seats + 3
    rack + engine + transmission + gun + ring = 12).
- **Detonation or ammo fire** (A3): a rack hit for at least
  `rack_min_damage` rolls `rack_detonate_pct` (prompt detonation,
  knock-out now) else `rack_fire_pct` (ammo fire: knock-out after
  `ammo_fire_s`, crew cannot put it out). A rack destroyed by damage
  becomes an ammo fire unless the detonation roll succeeds. Starting
  values: `rack_detonate_pct = 20`, `rack_fire_pct = 60`, `ammo_fire_s
  = 8`. Reasoning: wartime surveys (US and British loss studies)
  recorded most burned-out tanks as propellant fires, a minority as
  prompt explosions; exact shares are an owner/M2 citation task.
- **Crew knock-out at too few crew** (A3): knocked out when living crew
  fall below `crew_ko_below` (rules, default 2). Reasoning: one man
  cannot drive, load and fire; real crews abandoned such tanks.
  Survivors are lost with the vehicle until bail-out exists (owner
  question 5).
- **Transmission component** (A3, content): a new `ComponentKind`
  `transmission` (100 HP; damaged → speed ×0.5, destroyed →
  immobilised; field repair to 50 in 60 s; no fire), a box in the nose
  between the driver and radio operator, low. Both tanks have a front
  gearbox and final drives.
- Knock-out causes become: crew, rack detonation, engine fire, ammo
  fire (`ArmourEvent::component` 0–3; the snapshot doc and the bridge
  name the new one).

**Targets:** rack detonations ≤ 50 % of tank knock-outs (20 AI matches
pooled, and the case 3 duels pooled); crew and fire knock-outs both
occur (> 0 each in the 20 matches).

### P3. AT teams (D3)

**(a) Realism.**
- Crew boxes to the real layout (A3, content). Panzer IV: driver
  `center = [190, 60, 115]`, radio operator `[190, -60, 115]` (box front
  0.65 m behind the nose face, i.e. just behind the real front plate).
  Sherman: driver and assistant `center = [200, ±60, 120]`.
  Turret seats are already on the right sides (Panzer IV gunner left,
  loader right, commander rear centre; Sherman gunner right, loader
  left, commander right rear) and stay.
- Transmission box between them (P2): a centre hit through the hull
  front now immobilises.
- **HEAT reach, only if justified by the measurement** (A4): the jet
  is a stream of metal particles whose residual reach after perforation
  grows with the excess penetration. Data: RPzB 54 (160 mm against the
  Sherman's 63.5–93 mm effective) `core.length_cm` 100 → 150; M6A3 (100
  mm against the Panzer IV's 81 mm) stays 100. Applied only if case 6
  misses after the geometry fix.

**(b) AI** (B4): AT teams hunt (below).

**Targets:** case 6 tank disabled ≥ 60 %; case 4 flank disabling-hit
rate ≥ 50 %.

### P4. Moving targets (D4)

**(a) Realism** (A5): **lead estimation error.** The gunner leads by
his estimate of the target's velocity: the 1 s average × (1 + e), with e
triangular within ±`lead_error_pct` (rules, start 40 %), drawn from a
counter RNG keyed by (seed, shooter, target, ranging count, purpose
`LEAD`): no new state, the same target and ranging step give the same
estimate. Each further shot at the same target shrinks e by
`ranging_step_pct` down to `ranging_floor_pct` (the existing ranging-in
rule). `moving_target_spread_pct` stays 300 and is the second lever.

**Target:** crossing hit rate ≤ 60 % of stationary (case 3d).

### P5. HE against a parapet (D5)

**(a) Realism** (A7): a crater whose centre is on raised spoil (the
parapet: ground above the base surface) removes that spoil within the
crater radius down to `spoil_crater_pct` (rules `[dig]`, start 150 %) of
the crater depth and raises no rim above the base surface. Loose spoil
flies; dug walls and undug ground keep today's crater. Satchel cuts are
unchanged.

**Target** (report): lip lost per HE burst within 3 m of the trench ≥
0.05 m (baseline 0.001 m); 4 m gap opened in case 9 reported.

### P6. AI rockets and satchels (D6): Part B (B4, B5)

**Targets:** AI fires AT rockets in ≥ 8 of 10 matches (seeds 1–10,
normal bases; also reported for all 20); throws ≥ 1 satchel at a
vehicle in ≥ 3 of 10.

### P7. Trench assault (D7): Part B (B1–B3), design below

**Targets:** case 7 per owner question 5 (A): attacker losses ≤ 50 % of
7b's and defenders lose ≥ 75 %. Case 5b: a holding tank ordered to
suppress pins men in a trench (> 0 % pinned or ducked, kills > 0). New
fortify-test row (MG suppressing, rifles advance from 150 m) takes the
trench at a reasonable cost. The unsupported frontal rush still fails.

## Fragments and blasts (Part A1, A2)

### Ground and air bursts (shells, rockets, thrown items)

Each HE def (`[he]` on weapons, ammo and items) gains:

| Key | Meaning | Mortar 81 mm | Tank HE 75 mm | HEAT rocket | Frag grenade | Satchel |
|---|---|---|---|---|---|---|
| `frag_count` | fragment rays per burst (≤ 256, validated) | 96 | 80 | 24 | 48 | 32 |
| `frag_reach_cm` | ray length | 1500 | 1200 | 600 | 1000 | 800 |
| `frag_damage` | per fragment hitting a man (falls linearly to `frag_far_pct` at reach) | 70 | 70 | 50 | 60 | 80 |
| `frag_far_pct` | damage at full reach, % | 30 | 30 | 30 | 25 | 30 |
| `frag_elev_deg` | rays leave between this elevation band above the ground plane [min, max] | [2, 50] | [2, 45] | [2, 45] | [2, 50] | [2, 40] |
| `blast_radius_cm` | overpressure: full `damage` to a man within it with a clear line from the blast (and the vehicle `vs_vehicle_pct` to track anchors) | 150 | 120 | 60 | 100 | 400 |
| `frag_track_damage` | a fragment striking a hull side's track band | 5 | 5 | 5 | 2 | 5 |

The existing `lethal_radius_cm` and `damage_radius_cm` become unused
(validate warns, then they are removed in the Tool stage's cleanup).
Starting values are set to match today's lethality in the open (Tune
compares `fortify-test` mortar-in-the-open losses and case 9 open-ground
losses before and after; within ±20 %).

**Ray directions.** Azimuth stratified: ray k at `k·360/N` plus a
triangular jitter of ± half a stratum; elevation uniform in the band.
Counter RNG keyed (seed, tick, burst key, purpose `FRAGMENT`); the burst
key is the existing crater key (`(slot, CRATER)` / `(item id,
ITEM_CRATER)`). Directions from `Fx::sin_cos` only.

**Ray test** (Damage stage, in burst order, serial): from 0.3 m above
the burst point (as today's `BLAST_LIFT_MM`), each ray finds its first
hit among: enemy soldier capsules within reach (spatial hash query once
per burst, candidates sorted by id), vehicle and wreck boxes (vehicle
index; any owner's vehicle stops a fragment, friendly fire stays off for
damage), then terrain only up to that first hit (`segment_hit`; detail
tiles give trench walls and parapets). Rays that meet no capsule or box
need no terrain test. A ray that strikes a man deals its damage (with
the falloff) and adds `hit_suppression`; a ray that strikes a hull
side's track band deals `frag_track_damage`; anything else stops it.
Corpses do not block.

**Overpressure.** A man within `blast_radius_cm` whose body point has a
clear terrain line from the lifted burst point takes full `damage`; a
track anchor within it takes `damage × vs_vehicle_pct` (today's
`blast_tracks`, radius now `blast_radius_cm`).

**Suppression** stays per burst (#6), in `suppression_radius`.

**Budget** (engine constants in code): at most 256 rays per burst,
`MAX_FRAG_RAYS_PER_TICK` = 4,096; bursts over the tick budget get their
`frag_count` scaled down proportionally, in burst order (deterministic),
and `Stats::frag_rays_capped` counts it. Case 14 stress must stay under
10 ms p99 and AI skirmish peak under 1.5 ms.

### Inside a vehicle

- **APHE filler** (`[filler]` on AP ammo): `rays` (start 10), `cone_deg`
  (60, around the shell path), `damage` (60), `reach_cm` (150), burst
  `offset_cm` (100) past the entry; first box per ray. Replaces
  `radius_cm` (removed).
- **HE that beats a plate** (`inside_rays` 16, `inside_damage`,
  `inside_reach_cm` 150): rays into the hull hemisphere behind the plate
  from the contact point. Replaces `inside_radius_cm`.
- Purpose `HE_FILLER` (17, unused today) keys both.
- **Barrel**: the gun component gains an external box along the barrel
  (`barrel = { length_cm, radius_cm }` on the main-gun mount; the box
  turns with the gun). `first_box` tests it as a third box per vehicle
  (no armour plate). A main-gun round or rocket striking it deals
  `nonpen_damage` to the gun and stops; small arms stop on it with no
  damage. `EXTERNAL_REACH_MM` and the anchor rule are removed; track
  anchors remain only for overpressure.

## Trench pinning design (Part B1–B3)

**How the genre reference does it** (orchestrator's summary of Gates of
Hell player reports, feel not numbers): men in cover duck under heavy
incoming fire, stop returning fire and are harder to hit; experienced
men duck less; men in a trench fire more than men in the open but still
duck under a lot of fire; large-calibre HE mostly kills; a 2025 update
added area "suppression fire" without a spotted target, useful when it
pins; players assault by crawling, splitting squads, fire-and-move,
fire support during the approach, then grenades. Players complain the AI
is hard to suppress: here **one rule for AI and player men**.

### B1. The duck state (men in trenches)

Driven only by the existing suppression meter (thousandths of a point,
0–100), which only real rounds, fragments and bursts fill (#3, #4, #6).

| Key (`[combat]`, rules) | Start | Meaning |
|---|---|---|
| `near_miss_cm` | 200 (existing) | a round passing (or striking ground) this close adds its weapon's `suppression` |
| `cover_suppression_pct` | 50 (today hard-coded `/2`) | near misses on a man crouched below cover add this share |
| `burst_cover_pct` | 50 (today hard-coded `/2`) | burst suppression on a man with earth between |
| `duck_suppression` | 35 | **new**: a man in a trench at or above this ducks |
| `duck_release` | 20 | **new**: a ducked man comes back up only below this |
| `pin_suppression` | 70 (existing) | pinned: prone on the floor, no move, no fire, no throw |
| `crouch_suppression` | 30 (existing) | open ground only, unchanged |
| `suppression_decay_per_s` | 8 (existing) | linear decay |

Per squad type (unit defs): `steadiness_pct` (new, default 100, all
current units 100): the duck, release and pin thresholds are scaled by
it. There is no veterancy today; this is the hook for it ("experienced
troops duck less"), owner question 9.

**What a ducked man does** (in a trench, `in_trench`): stays crouched
below the lip (the crouch capsule, `fire_height` unused), does not rise
to the fire step, does not aim, fire or start a throw (a wind-up in
progress is called off, as for pinned men today), keeps his target
memory, may still move along orders. He is harder to see (not firing:
no reveal; trench spotting 60 %) and harder to hit (below the lip only
plunging fire and fragments reach him). A pinned man lies on the trench
floor (today's rule). In the open nothing changes (owner question 3
(A)): crouch at 30, pinned prone at 70 with 200 % spread.

**State:** `Combat::ducked: bool` (hysteresis between `duck_release`
and `duck_suppression`), saved, hashed, validated (false for non-
soldiers and corpses), with a test.

**Interaction with existing code:** `status` (`combat.rs:1741`): the
in-trench branch checks `ducked` before `aiming`; `aim_and_fire` and the
launcher carrier skip ducked men; `items` treats ducked like pinned for
starting a throw; spotting unchanged (a ducked man does not fire, so no
reveal). Near-miss halving (#4) and burst halving (#6) read the new
keys.

### B2. The Suppress order (area fire)

New command `SuppressUnits { units: Vec<SimId>, line: Vec<FxVec2> }`
(1–120 own live soldier or vehicle ids, sorted and deduplicated as the
S1b `*Units`; 1–2 points on the map, a second point at most
`suppress_line_max_m` from the first). Full squads map to a squad order
`Order::Suppress { line }`, detached men to `UnitOrder::Suppress`, as
other `*Units` commands do. Vehicles take a `VehicleState::suppress:
Option<[FxVec2; 2]>`. All saved, hashed, validated.
`PROTOCOL_VERSION` 3 → 4; `command_in_limits`: ids ≤ `MAX_UNITS`,
points 1–2.

**Behaviour** (the same for AI and players):
- Each man holds where he stands (engage rule `Still`; in a trench he
  rises to the fire step as when aiming; a ducked or pinned man does
  not fire). Each shot aims at a point on the line chosen by counter RNG
  (purpose `SUPPRESS`, keyed by shooter and shot count), at
  `suppress_aim_cm` (30) above the ground there (the parapet top on a
  trench line), within the weapon's `range_max`, with the normal spread
  for range and movement. No spotted target is needed; the line of fire
  from the muzzle to the aim point must be clear of terrain (one ray per
  shot; a blocked shot is not fired). MGs sweep along the line. Rounds
  are real rounds: near misses (#4) on men below the lip come from rounds
  striking the parapet or passing just over it.
- Ammo: small arms use magazines and reloads as normal fire (there is no
  infantry ammo pool; owner question 2 (A)). Tank coax and hull MG fire
  belts at the line (hull MG within its arc). The main gun fires HE at
  the line at most every `suppress_he_s` (12) while more than
  `suppress_he_reserve` (6) HE rounds remain.
- Mortars keep `Barrage`. Launchers ignore the order.
- It lasts until another order (`StopUnits` included). Men on Suppress
  do not switch to spotted targets.

| Key (`[combat]`) | Start | Meaning |
|---|---|---|
| `suppress_aim_cm` | 30 | aim height above the ground on the line |
| `suppress_line_max_m` | 40 | longest suppress line |
| `suppress_he_s` (`[vehicles]`) | 12 | a tank's HE interval on Suppress |
| `suppress_he_reserve` (`[vehicles]`) | 6 | HE kept back |

**Expected effect** (to be measured): an MG 42 (20 rounds/s, ±3 m spread
at 150 m) on a 25 m line gives each of 8 men about 3 near misses a
second, ×15 points × 50 % ≈ 24 points/s against a decay of 8: the line
stays ducked. A rifle squad alone (about 2–3 rounds/s) adds 1–2
points/s per man: it does not pin. That is the intended feel: MGs and
tanks pin, rifles alone do not.

**HUD** (S1b-hud conventions): a "Suppress" button in the orders grid
with the hotkey **P** (owner question 1 (A); free key, not used by S1b
or S3), enabled for soldiers and vehicles. P, then LMB places a point,
or LMB-drag draws a line up to 40 m (clamped). The pending line is a
dashed accent (#E0AD4A) line on the ground; units on the order show
"Suppressing" as their card state, and their lines are drawn while
selected. Snapshot: `squads[].order` gains `suppress` with the line;
vehicles gain the line.

**Rejected alternatives:** suppression from "targeted by" or "vehicle
attacking this squad" flags, and from an outnumbered count (owner
rule); auto area fire at last-known positions (not asked, more AI-like
than player-like; may come later); tank fear and mortar smoke (owner).

### B3. AI fire-and-move

- **Base of fire.** When at least two own squads are assigned to attack
  the same point and the AI knows an enemy trench in front of it (the
  existing trench probe) or has seen enemies there in the last
  `ai.suppress_memory_s` (20 s), it designates a base of fire: an MG
  team, else a tank on that point, else one rifle squad. The base moves
  to a spot within `ai.support_range_m` (MG 250, rifles 150, tank 200)
  with a clear line to the line's midpoint (checked with one terrain ray
  per candidate, at most 8 candidates per plan), then issues
  `SuppressUnits` on the trench segment (or the seen enemies' spread,
  clamped to 40 m).
- **Bounds.** The other squads advance by bounds of `ai.bound_m` (40)
  while the base fires; with two or more manoeuvre squads they alternate
  (one moves, one halts and fires normally). Within frag range the
  existing throw rule takes over.
- **Lift.** The base switches to normal fire when a manoeuvre squad's
  leader is within `ai.lift_fire_m` (15) of the line (no friendly fire,
  but it frees the base to engage counter-attacks).
- **Tanks** escorting an assault suppress the trench line while the
  escort advances (the escort no longer walks in only behind a halted
  tank).
- New AI state (bases, bounds, lines) saved and hashed; counters
  `suppress_orders`, `bounds` in `AiCounters` and the skirmish report.

### B4. AI AT teams hunt

When a known enemy vehicle is within `ai.at_hunt_m` (250) of an AT
team's point or of the team, and no known enemy soldier is within 40 m
of the team's route midpoint, the team moves to a firing spot: the
nearest trench cell or wood cell within its `engage_still` of the
vehicle, preferring the vehicle's side or rear (dot product with the
vehicle's facing), else a point at `engage_still − 10` m from it on the
side away from its turret. Re-checked every plan; the weapon's fire
discipline still decides when to fire. Counter `at_hunts`.

### B5. AI satchel hunts

An engineer squad with a satchel goes for a known enemy vehicle that is
bogged, immobilised, or slower than `SATCHEL_SLOW` for at least 3 s,
within `ai.satchel_hunt_m` (60) of the squad: it moves to a spot
`SATCHEL_SIDE` beside the vehicle's side away from its hull MG arc, and
the existing throw rule fires within 8 m. Counter `satchel_hunts`.
Quantities (step b): Tune may give engineers a second satchel (a second
man's loadout) if 3 of 10 is missed.

### B6. AI angling

Above (P1 (b)): `ai_face_angle_deg` per vehicle def.

## Tooling prerequisites (the Tooling stage, before Part A)

Per the quick-iteration rule:

1. **Gameplay constants to TOML** (rules or defs, each with a comment):
   `damage.rs` `EXTERNAL_REACH_MM` (removed in A2), `SPALL_REACH_M`
   (→ spall `reach_cm`); `gunnery.rs` `ELEVATE_DEG_S` (→ vehicle
   `elevate_deg_s`), `COAX_CONE_DEG`; `combat.rs` `TRENCH_MM`,
   `STILL_MM_S`, `HEAD_AIM_MM`, `BLAST_LIFT_MM`, `CRATER_LIFT_MM`,
   `CRATER_DEPTH_PCT`, `SHELL_FLIGHT_MILLI`, `SHELL_MIN_FLIGHT_S`,
   `BARRAGE_RETRY_M`, the near-miss and burst halvings; capsule sizes
   (`STAND_MM`, `CROUCH_MM`, `PRONE_MM`, radii) as `[combat.body]`;
   `ai.rs` `DANGER_CLOSE_M`, `SUPPORT_BACK_M`, `FRAG_GROUP_M`,
   `ESCORT_SQUADS`, `ESCORT_JOIN_M`, `ESCORT_GRACE_S`,
   `ESCORT_REACHED_M`, `HALTED_MM_S`, `FACE_SLACK_DEG`, `FACE_RANGE_M`,
   `ARMOUR_THREAT_M`, `RETREAT_ENGINE_PCT`, `SATCHEL_SLOW_MM_S`,
   `SATCHEL_SIDE_MM`, `BACKUP_M`, `TRENCH_PROBE_*`; `orders.rs`
   `APPROACH_SLACK_M`, `ESCORT_BEHIND_M`, `ESCORT_GAP_MM`; `path.rs`
   vehicle cost percentages; `squad.rs` `COVER_RADIUS_M`; `items.rs`
   `LOB_TAN_MILLI`, `RELEASE_DROP_MM`. Stay in code: pool and list caps
   (`MAX_*`, `SHELL_RESERVE`, `CHUNK_SIZE`, `MAX_RAYS`, `MAX_BOXES`,
   `MAX_FRAG_RAYS_PER_TICK`), `SUBSTEP_HZ`, grid and tile sizes, path
   search parameters that exist for performance, RNG draw counts.
   Moving a constant must not change any hash (same value): tests and
   the skirmish hashes prove it.
2. **Fast A/B**: `--rules <file.toml>` (a partial rules/defs override
   merged over the mod, validated) on `skirmish`, `armour-test` and
   `fortify-test`; and `frontline-cli ab --b <override.toml> [--a
   <override>] --seeds 1-10 [--swap-bases] [--case N]` that runs both
   sides in parallel threads and prints each target metric side by side
   with the delta (win rates per faction and base, KO causes, rockets,
   satchels, HE share, case rows).
3. `skirmish` gains `--seeds 1-10 --both-bases` summarising the 20
   matches (the Targets table rows) in one call.

## Tuning surface (Tooling stage, done)

The Tooling stage moved 107 gameplay constants out of `crates/sim/src`
into `mods/ww2` TOML, with the same values. Each new table is one struct in
`sdk/crates/content/src/tuning.rs`. The def parses it, the setup copies it
unchanged, and both range-check it. Values stay whole numbers in the unit
the key names. The sim converts them where the old constant was converted,
so no result changes (evidence in the Tooling report: AI-vs-AI seeds 1–5,
armour-test and fortify-test give identical reports). `docs/modding/tuning.md`
is the user guide.

Differences from the plan above:
- the flag is `--rules-override` (the task's name), not `--rules`;
- the A/B runner is `skirmish --seeds N --both-bases --summary` plus `--ab`,
  not a separate `ab` subcommand;
- `ELEVATE_DEG_S` became each vehicle def's `turret.elevate_deg_s`;
- `SPALL_REACH_M` is `[vehicles.gunnery] spall_reach_m`, not a per-round
  `reach_cm`. A2 may move it into the ammo `spall` table when it
  rewrites spall;
- `SAVE_VERSION` went 9 → 10 here, because the setup is part of the save
  and its layout changed. **It is the one bump for S3b; later stages add
  state without bumping again.** `PROTOCOL_VERSION` is unchanged: the wire
  carries only setup and content hashes. The Suppress command still bumps
  it once.

### Moved (constant → key, value unchanged)

| Was (file) | Now | Value |
|---|---|---|
| `rules.rs` `CAPTURE_EXTRA_MAN_PCT` | `[capture] extra_man_pct` | 50 |
| `path.rs` `DITCH_COST_PCT` | `[movement] ditch_cost_pct` | 300 |
| `gunnery.rs` `ELEVATE_DEG_S` | vehicle def `[turret] elevate_deg_s` | 6 |
| `combat.rs` `STAND_MM`, `CROUCH_MM`, `PRONE_MM` | `[combat.body] stand_mm`, `crouch_mm`, `prone_mm` | 1750, 1200, 400 |
| `combat.rs` `CAPSULE_R_MM`, `PRONE_R_MM`, `PRONE_HALF_MM` | `[combat.body] radius_mm`, `prone_radius_mm`, `prone_half_mm` | 250, 200, 700 |
| `combat.rs` `MUZZLE_DROP_MM`, `HEAD_AIM_MM`; `spotting.rs` `EYE_DROP_MM` | `[combat.body] muzzle_drop_mm`, `head_aim_mm`, `eye_drop_mm` | 150, 150, 100 |
| `combat.rs` `TRENCH_MM`, `STILL_MM_S` | `[combat.body] trench_depth_mm`, `still_mm_s` | 1000, 300 |
| `combat.rs` `SHELL_MUZZLE_MM`, `SHELL_FLIGHT_MILLI`, `SHELL_MIN_FLIGHT_S`, `BARRAGE_RETRY_M` | `[combat.mortar] muzzle_mm`, `flight_milli`, `min_flight_s`, `barrage_retry_m` | 800, 350, 4, 20 |
| `combat.rs` `BLAST_LIFT_MM`, `CRATER_DEPTH_PCT`, `CRATER_LIFT_MM` | `[combat.blast] lift_mm`, `crater_depth_min_pct`, `crater_depth_max_pct`, `crater_lift_mm` | 300, 75, 140, 500 |
| `combat.rs` near miss on a covered man `/ 2`; burst behind earth `/ 2`; spread `+1 %` per point | `[combat.suppression] covered_near_miss_pct`, `blast_behind_cover_pct`, `spread_per_point_pct` | 50, 50, 100 |
| `ai.rs` `MAX_BUYS_PER_PLAN`, `DANGER_CLOSE_M`, `SUPPORT_BACK_M`, `FRAG_GROUP_M` | `[ai.infantry] buys_per_plan`, `danger_close_m`, `support_back_m`, `frag_group_m` | 2, 35, 200, 5 |
| `ai.rs` `SATCHEL_SLOW_MM_S`, `SATCHEL_SIDE_MM` | `[ai.infantry] satchel_slow_mm_s`, `satchel_side_mm` | 1100, 800 |
| `ai.rs` `TRENCH_PROBE_DM`, `TRENCH_PROBE_M`, `TRENCH_PROBE_STEP_M` | `[ai.infantry] trench_probe_dm`, `trench_probe_m`, `trench_probe_step_m` | 8, 60, 4 |
| `ai.rs` `ESCORT_SQUADS`, `ESCORT_JOIN_M`, `ESCORT_GRACE_S`, `ESCORT_REACHED_M` | `[ai.armour] escort_squads`, `escort_join_m`, `escort_grace_s`, `escort_reached_m` | 2, 150, 8, 30 |
| `ai.rs` `HALTED_MM_S`, `FACE_SLACK_DEG`, `FACE_RANGE_M`, `ARMOUR_THREAT_M`, `RETREAT_ENGINE_PCT`, `BACKUP_M` | `[ai.armour] halted_mm_s`, `face_slack_deg`, `face_range_m`, `threat_m`, `retreat_engine_pct`, `backup_m` | 300, 20, 600, 300, 30, 30 |
| `vehicle.rs` `MAX_TILT_DEG`, `TILT_RATE_DEG_S`, `CIRCLE_PAD_MM` | `[vehicles.driving] max_tilt_deg`, `tilt_rate_deg_s`, `circle_pad_mm` | 30, 20, 250 |
| `vehicle.rs` `WAYPOINT_REACH_M`, `ARRIVE_MM`, `PIVOT_ERR_DEG`, `PIVOT_SPEED_MM_S`, `FULL_STEER_DEG`, `FULL_THROTTLE_DEG` | `[vehicles.driving] waypoint_reach_m`, `arrive_mm`, `pivot_err_deg`, `pivot_speed_mm_s`, `full_steer_deg`, `full_throttle_deg` | 3, 2500, 60, 1000, 10, 20 |
| `vehicle.rs` `REVERSE_ORDER_M`, `YIELD_LEN_PCT`, `YIELD_STILL_MM_S`, `FRIENDLY_AHEAD_M`, `WALK_MM_S` | `[vehicles.driving] reverse_order_m`, `yield_len_pct`, `yield_still_mm_s`, `friendly_ahead_m`, `walk_mm_s` | 40, 150, 500, 4, 1500 |
| `vehicle.rs` `STUCK_PROGRESS_M`, `STUCK_S`, `KEEP_RIGHT_DEG`, `BACK_OFF_S`, `REPATH_GAP_S` | `[vehicles.driving] stuck_progress_m`, `stuck_s`, `keep_right_deg`, `back_off_s`, `repath_gap_s` | 1, 5, 35, 2, 2 |
| `orders.rs` `FACE_DONE_DEG`, `APPROACH_SLACK_M` | `[vehicles.driving] face_done_deg`, `approach_slack_m` | 2, 15 |
| `vehicle.rs` `PUSH_GAP_MM`, `PUSH_DEEPER_MM`, `SPOT_DROP_MM` | `[vehicles.driving] push_gap_mm`, `push_deeper_mm`, `spot_drop_mm` | 100, 100, 100 |
| `path.rs` `VEHICLE_WOODS_PCT`, `VEHICLE_DUG_PCT`, `VEHICLE_DUG_DM`, `VEHICLE_DEEP_PCT`, `VEHICLE_DEEP_DM` | `[vehicles.path] woods_pct`, `dug_pct`, `dug_dm`, `deep_pct`, `deep_dm` | 200, 150, 4, 400, 10 |
| `vehicle.rs` `AVOID_COST_PCT`, `MARGIN_COST_PCT`, `DITCH_REACH_M`, `WRECK_PAD_M` | `[vehicles.path] avoid_pct`, `margin_pct`, `ditch_reach_m`, `wreck_pad_m` | 800, 400, 2, 2 |
| `orders.rs` `ESCORT_BEHIND_M`, `ESCORT_GAP_MM`, `ESCORT_FILE` | `[vehicles.escort] behind_m`, `gap_mm`, `file_men` | 4, 1500, 8 |
| `gunnery.rs` `COAX_CONE_DEG`; `damage.rs` `EXTERNAL_REACH_MM`, `SPALL_REACH_M` | `[vehicles.gunnery] coax_cone_deg`, `external_reach_mm`, `spall_reach_m` | 5, 1000, 3 |
| `squad.rs` and `unit.rs` `WAYPOINT_REACH_M`, `ARRIVE_TIMEOUT_S`; `squad.rs` `CATCH_UP_M`, `STRETCHED_M`, `STRETCHED_SPEED_PCT`, `SLOT_LEAD_MM` | `[movement.formation] waypoint_reach_m`, `arrive_timeout_s`, `catch_up_m`, `stretched_m`, `stretched_speed_pct`, `slot_lead_mm` | 2, 20, 2, 10, 70, 1500 |
| `systems.rs` `SLIDE_GAP_MM`, `STEEP_FREE_PCT`, `STEEP_MIN_PCT` | `[movement.formation] slide_gap_mm`, `steep_free_pct`, `steep_min_pct` | 1500, 60, 15 |
| `squad.rs` `COVER_RADIUS_M`, `COVER_ENEMY_M`, `COVER_GAP_MM`, `TRENCH_MARGIN_MM`, `COVER_EYE_MM`, `COVER_CHEST_MM` | `[cover] radius_m`, `enemy_m`, `gap_mm`, `trench_margin_mm`, `eye_mm`, `chest_mm` | 10, 400, 1200, 400, 1500, 1000 |
| `items.rs` `RELEASE_DROP_MM`, `MIN_FLIGHT_MS`, `LOB_TAN_MILLI`, `SMOKE_BELOW_MM` | `[items.throw] release_drop_mm`, `min_flight_ms`, `lob_tan_milli`, `smoke_below_mm` | 150, 300, 1000, 2000 |
| `dig.rs` `PARAPET_RISE_MM`, `PARAPET_TOP_MM`, `PARAPET_FALL_MM`, `PARAPET_UNDUG_MM`, `DIG_REACH_MM` | `[dig.common] parapet_rise_mm`, `parapet_top_mm`, `parapet_fall_mm`, `parapet_undug_mm`, `reach_mm` | 300, 400, 600, 50, 1500 |

### Kept in code (engine and structural)

| Constant (file) | Why it stays |
|---|---|
| `MAX_PENDING`, `MAX_TRENCHES`, `MAX_ESCORTS`, `NO_POINT`, `MIX_*` (`ai.rs`); `MAX_THROWN`, `MAX_SMOKE` (`items.rs`); `MAX_UNITS` (`unit.rs`); `MAX_BARRAGE_ROUNDS`, `MAX_PATH_POINTS`, `MAX_DIG_POINTS`, `MAX_CALL_IN_QUEUE` (`game.rs`); `MAX_RAYS`, `MAX_BOXES` (`damage.rs`); `MAX_NEIGHBOURS` (`systems.rs`); `MAX_VEHICLE_CIRCLES`, `TRACK_SAMPLES` (`vehicle.rs`) | Pool, list and state-layout caps. Saves are validated against them. |
| `LOS_TRIES`, `SWEEP_POOL` (`combat.rs`); `GROUP_CANDIDATES`, `SEARCH_RAYS`, `SWEEP_POOL`, `MG_AIM_DRAWS` (`gunnery.rs`); `COVER_RAYS`, `TRENCH_STEP_MM`, `COVER_STEP_MM` (`squad.rs`) | Search sizes, ray budgets, sampling grids and RNG draw counts. They are performance and determinism structure. A tuning change goes into the distances these searches use, which are data now. |
| `STRAIGHT`, `DIAGONAL`, `SMOOTH_LOOKAHEAD`, `H_WEIGHT_PCT`, `TRENCH_SEARCH_NODES`, `SMOOTH_GENTLE_PCT` (`path.rs`) | Path search parameters that exist for performance. |
| `VEHICLE_SPHERE_PAD_M` (`combat.rs`) | Broad-phase padding (it only finds candidates). |
| `SHELL_TTL_PCT` (`gunnery.rs`); `MAX_FLIGHT_S` (`items.rs`) | Lifetime safety bounds on projectiles and thrown items. |
| `GRAVITY` (`armour.rs`, `combat.rs`, `gunnery.rs`, `items.rs`, `projectile.rs`); `SUBSTEP_HZ` (`projectile.rs`); `DETAIL_SHIFT`, `TILE_M` and the grid sizes (`terrain.rs`); `PPM` (`armour.rs`); `AP`, `HE`, `KO_*` | Physics, engine units and codes. |
| `HALF_BAND_RAW` / `PEN_BAND` (`armour.rs`) | The ±7 % penetration variation. **Gameplay, kept for now.** It is part of the closed-form penetration maths that the golden ballistics file pins (`ballistics.rs`, `aim_choice`). Moving it means threading a band parameter through the armour API and the golden runner. That is follow-up work (TODO S3b-14), and nothing in the slice tunes it. |
| `TRENCH_X`, `CROSSING_CM_S`, `AT_HE_M`, `WEST`, `SOUTH` (`scenarios.rs`) | Staging coordinates of the test harness. |
| `systems::BENCH_STEEP` | Steep-ground values for bench worlds, which have no rules. They equal the mod's, and bench terrain has no dug ground today. |

Inline numbers were not swept exhaustively. The ones the spec named (the
two halvings and the spread per point) are data now. Others remain in
code, for example the satchel reach margin (+10 m), "two crew dead"
(`dead >= 2`) in the AI retreat rule, and the 0.25 m nav heading update.
The Part A and B stages that touch those rules move them as they go.

### Fast iteration

- `frontline-cli skirmish --ai-vs-ai --seeds 10 --both-bases --summary
  [--ab] --rules-override FILE`: 20 matches in about 20 s on 8 cores
  (`--ab`: 40 matches in 38 s). It prints one table: wins per faction and per base,
  length, tanks lost and bought, knock-outs by gun and rocket and by cause,
  HE share, rockets fired, hit and penetrated, satchels at vehicles, kills
  and losses, tick p99, and one line per match.
- `armour-test` and `fortify-test` take `--rules-override` too. `simulate`
  refuses it, because bench scenarios have no rules.
- `frontline-cli validate mods/ww2` also applies every
  `tests/overrides/*.toml`.
- Godot debug builds: F5 or the pause menu's "Reload data & restart". Data
  errors show as a banner, and the menu shows why a match could not start.

## Call-in without a queue (stage done, 2026-11-01)

Owner, 2026-11-01: "Remove the queue, but keep the timer of the specific
unit after it was called. Like in GOH. So the user can't spam units and
there is some heft and waiting time for spawning many units at once." And:
"instead of a delivery time, it would spawn immediately, but it will be
balanced out by the need of the unit to move to the frontline itself".

- **Sim** (`game.rs`): `CallIn` buys at once when the player has the
  manpower, the type's re-call cooldown is over, the pop cap allows it and,
  for a vehicle, the unlock time has passed and fewer than the limit are
  alive. Manpower is spent and the squad spawns in the same tick. Otherwise
  the command is refused with a `CallInRefusal` (roster, locked, vehicle
  limit, cooldown, manpower with the shortfall, pop cap: the check order).
  Every call-in's result goes to the snapshot (`RenderSnapshot::call_ins`,
  per tick, never saved). `MAX_CALL_IN_QUEUE`, `PlayerState::call_ins`,
  `spawn_call_ins`, `call_in_s` / `call_in_ticks` and the delivery time are
  gone from sim, content, snapshot, HUD and AI.
- **State**: `PlayerState::ready_at` (per squad type, the first tick it can
  be bought again), saved, hashed (per-field hash test) and validated on
  load (one per squad type, never further ahead than the longest
  cooldown). `SAVE_VERSION` stays 10 (the slice's one bump, Tooling).
  `PROTOCOL_VERSION` 3 → 4 here (`CallIn` changed meaning; no queue
  command ever existed, so the relay limits only lost nothing). **This is
  the slice's one protocol bump: the Suppress command (B2) joins version
  4.**
- **Data** (every key with a comment): `recall_cooldown_s` and
  `initial_cooldown_s` per unit def (soldier squads) and vehicle def
  (tanks); optional `cooldown_group` (a namespaced id) on a unit def, so
  types of one group share a timer (built, tested, unused in `ww2`).
  Defaults set by the owner on 2026-11-01 (20 % under the first
  proposal): rifle 24 s, engineers and MG 36 s, AT team 48 s, mortar 60 s,
  tank 120 s. Start cooldowns 0 everywhere (vehicles keep the 4:00
  unlock). New table `[call_in]`: `spawn_ring_pct = 50`,
  `spawn_clear_m = 10`, `spawn_vehicle_weight = 8`. New key
  `[ai.armour] tank_reserve_mp = 200`.
- **Spawn spots**: seven per base (six on a ring at half the 40 m base
  radius, and the centre, all inside the base). A squad tries spot
  `id mod 7` first, then the next, and takes the first with the fewest men
  within 10 m (a vehicle counts 8). Squads bought in quick succession, or
  onto a spot where a squad still stands, land apart (test: seven rifle
  squads in seven ticks take seven spots more than 10 m apart; four tanks
  bought in one tick stand more than 10 m apart).
- **Walk times** from the base (Kessel Ridge, both bases 288 m straight
  from their nearest point and 588 m from C; seconds from the buy until
  the squad leader or tank is inside the 15 m capture radius, Move order,
  `walk_times_from_each_base_to_the_front`):

  | Base | Unit | Nearest point | Centre C |
  |---|---|---|---|
  | 0 (SW) | rifle squad (3 m/s) | B 92.5 s | 192.5 s |
  | 0 (SW) | tank (Panzer IV) | B 49.3 s | 103.5 s |
  | 1 (NE) | rifle squad | D 92.5 s | 192.4 s |
  | 1 (NE) | tank (Sherman) | D 46.1 s | 94.8 s |

  The old delivery times were 20–30 s (infantry) and 45 s (tanks), so the
  walk is the larger delay; the cooldowns (24–120 s) limit how many of a
  type arrive at once.
- **AI**: buys by its mix priorities as before; a role whose type is on
  cooldown when the command applies is passed over for the next role that
  is ready. After the unlock, under the vehicle limit, non-vehicle buys
  leave `tank_reserve_mp` unspent (an urgent AT team, more enemy vehicles
  in sight than own AT teams and vehicles, ignores the reserve).
- **HUD**: no queue bar; tile states ready (lit), cooldown (fill, clock,
  seconds), need N, locked, limit, pop cap. Click, or Tab then 1–9, buys.
  Feed: "Rifle Squad deployed at base." / "... not called in: <reason>."
  (`docs/slices/S1b-hud.md` owner note, `godot/docs/match_state.md`).

Measured after the change (before → after; "before" is the Tooling
stage's numbers on the same seeds):

| Run | Before | After |
|---|---|---|
| AI vs AI seeds 1–5 | USA 5 of 5; 14.2–22.6 min | Germany 3, USA 2; 12.9–16.8 min |
| 20 matches (seeds 1–10, both bases) | Germany 4, USA 16; base 0 6, base 1 14; 13.6–26.2 min, 1 over 25 | Germany 12, USA 8; base 0 8, base 1 12; 11.7–26.7 min, 1 over 25 |
| Tanks lost / bought (20 matches) | Germany 32/56, USA 20/63 | Germany 19/54, USA 26/56 |
| Knock-outs (20 matches) | rack 47, crew 5 | 45 (rack 44, fire 1) |
| HE share of kills | mean 39.5 %, max 45.5 % | mean 44.2 %, max 56.5 % |
| fortify-test | pass | pass; totals identical (rifles 0 vs 80, mortar 28 vs 74 = 37.8 %); 20 per-seed values moved (mortar rows and the smoke control) |
| armour-test | all hard checks pass | all hard checks pass |
| golden ballistics | 42/42 | 42/42 |

The faction swing is not a balance fix: no unit stat changed. The USA
used to win by fielding more (63 tanks bought vs 56); with re-call
cooldowns both sides buy about the same number of tanks (54 / 56), and
the Sherman's duel edge (P1) is untouched. The Tune stage should
measure again after Part A.

## Part A: physical damage, armour, gunnery and content (stage done, 2026-11-01)

Built as specified above, with the deviations listed below. Every new
gameplay value is a commented key in `mods/ww2`; engine caps stay in code.

**A1 Fragments and overpressure** (`combat.rs` `MatchState::burst`,
`fragments`): every HE burst (mortar, tank HE, rocket, grenade, satchel)
does suppression (unchanged), **overpressure** (full `damage` within
`blast_radius_cm` with a clear terrain line to the body point; track
anchors within it take `vs_vehicle_pct`), then `frag_count` **fragment
rays** from 0.3 m above the burst (azimuth stratified with a triangular
jitter, elevation uniform in `frag_elev_deg`; RNG `FRAGMENT` /
`ITEM_FRAGMENT` keyed by the burst key). A ray stops at the first enemy
capsule, vehicle, wreck or barrel box within `frag_reach_cm`, unless
`segment_hit` finds earth first (trench walls, parapets). Men take
`frag_damage` falling linearly to `frag_far_pct` at full reach, plus the
hit suppression; an enemy hull side's track band takes
`frag_track_damage`. `lethal_radius_cm` and `damage_radius_cm` are gone
(an old mod fails to load naming them). Budget: `MAX_FRAG_RAYS_PER_TICK` =
4 096 (code); a crowded tick scales each burst's rays proportionally
(`Stats::frag_rays_capped`). New hashed, saved `Stats`: `frag_rays`,
`frag_hits`, `frag_rays_capped`.

Details the spec left open, decided here:
- A burst well above the ground (on a vehicle or a man, over
  `crater_lift_mm`) throws within `[-max, max]` of the band, not only
  upward.
- A man whose capsule holds the ray origin (a shell burst on him) is left
  to the overpressure, so his body does not shield everyone else.
- The vehicle a bursting round struck stops rays that go into it (a probe
  0.1 m along the ray inside its hull or turret box) and lets the others
  go; it takes no fragment damage (its plate hit is its damage).
- Friendly men are not in the way (no friendly fire); any vehicle is.
- Tank HE target choice keeps its old group radius as a key:
  `[vehicles.gunnery] he_group_m = 6` (it read `damage_radius`).

**A2 Inside a vehicle** (`damage.rs`): APHE filler = `rays` (10) within
`cone_deg` (60) of the shell path from `offset_cm` past the entry, each
`damage` (60) to the first box within `reach_cm` (150); HE that beats a
plate = `inside_rays` (16) within 45° per axis of the inward normal
(`INSIDE_CONE_DEG`, geometry), `inside_damage` each within
`inside_reach_cm` (150). A box that holds the burst point is not in the
rays' way (the core already went through it). **Barrel:** the main-gun
mount's `barrel = { length_cm, radius_cm }` is a box from the muzzle back,
turning and elevating with the gun (`VehicleFrame::barrel`,
`armour::BoxPart::Barrel`); a main-gun round or rocket striking it deals
`nonpen_damage` to the gun; small arms stop on it; mortar shells burst on
it. `external_reach_mm` and the gun's anchor are gone; non-penetrating
hits touch nothing outside the track band.

**A3 Geometry and outcomes** (data with sources in the TOML comments):
multi-box components (`boxes = [...]`, one HP pool, at most 4, each
checked inside its frame); racks moved to the real stowage (Panzer IV:
two hull-side bins and one by the radio operator; Sherman, dry stowage:
two sponson racks and the basket floor); front crew boxes moved forward
(Panzer IV x 150 → 190, Sherman 170 → 200); a `transmission` component
(seventh `ComponentKind`, 100 HP, below half its HP halves speed, 0
immobilises, field repair to 50 in 60 s) between the front seats. Rack
hits: detonation with `rack_detonate_pct` (35 → 20), else an **ammo
fire** with `[vehicles.damage] rack_fire_pct` (60), which knocks out after
`ammo_fire_s` (8; nobody puts it out); a destroyed rack always detonates
or burns. **`crew_ko_below`** (2): fewer living crew knock the vehicle
out. Knock-out cause 3 = ammo fire (`ArmourEvent::component`; bridge
`"ammo_fire"`). New saved, hashed, validated state:
`VehicleState::ammo_fire`.

**A4 HEAT reach: not applied.** Case 6 reached its target (6 of 10
disabled) after the geometry fix, so the RPzB 54 jet stays at 1.0 m.

**A5 Gunnery:** the gunner aims at the **centre of visible mass** (hull
and turret centres weighted by projected area toward the shooter,
`gunnery::mass_centre`), or the turret centre when that point is hidden;
launchers too. `aim_hull_margin_pct` is gone. **Lead estimation error**
(`gunnery::lead_estimate`, RNG `LEAD` keyed by shooter, target and ranging
count, no state): ±`lead_error_pct` (40), shrinking with ranging; rockets
get it per shot without shrinking. Case 3b starts the angled Panzer IV
with its turret laid on the Sherman; new case 3e (Panzer IV on the
Sherman's flank, 150 m). The 3b hard check is now "aims within 0.3 m of
the visible-mass centroid".

**A6 PzGr 40** (`defs/ammo/75mm_pzgr40.toml`, 6 per Panzer IV): a mount
may carry a third round, a special penetrator, only with its HE round
(indices: standard 0, HE 1, special 2; `gunnery::SPECIAL`).
`AmmoChoice::Apcr` (a gun without it fires its standard round); Auto loads
it only when the standard round's chance is under
`[vehicles.gunnery] special_below_pct` (50) and the special one's is
higher. Snapshot `ammo` is `[u16; 3]`, `LoadedSnap::Apcr`, `ammo_choice` 3;
bridge `"apcr"`; HUD: an APCR segment in the ammo strip (lit only for a gun
that has it), N cycles Auto → AP → APCR → HE, counts in the card, tooltip
and detail panel (`godot/docs/match_state.md`). Golden ballistics: four new
rows (43–46), rows 1–42 unchanged, 46 of 46 pass. In the 20 matches it
changed nothing (the PzGr 39 already beats a Sherman everywhere).

**A7 HE vs parapet** (`Heightfield::crater_spoil`): a crater centred on
raised spoil removes it within its radius by `spoil_crater_pct` (150) % of
the crater depth (never below the base surface) and raises no rim above
the base; otherwise today's crater.

**Data deviations from the spec's starting values** (each commented in
its TOML; owner questions S3b-19, S3b-20):
- Grenades: `blast_radius_cm` 100 → 200 (the old lethal radius), 
  `frag_count` 48 → 96, `frag_track_damage` 2 → 0. With the spec's values
  a grenade thrown into a trench did a third to a half of its old harm
  (the abilities test "frags lobbed into a trench" failed: 100 HP lost,
  needs 200; fortify grenade row from 25 m: defenders 70, attackers 73
  instead of 80 / 24).
- `frag_track_damage` 5 → 1 for mortars, tank HE, rockets and satchels: a
  mortar bomb 1 m from a hull sends about 20 fragments into one track
  band and broke the track on fragments alone.

**Measured after Part A** (same commands as the baseline; "before" is the
call-in stage):

| # | Target | Before | After Part A | Met? |
|---|---|---|---|---|
| T1 | each faction 35–65 % of 20; no base > 70 % | GER 12 / USA 8; base 1 12 | **GER 9 / USA 11; base 0 7 / base 1 13 (65 %)** | yes |
| T2 | 3a Panzer IV wins 6–8 of 10 | 4 | **4** (hits: Panzer IV 14 hull / 6 turret, Sherman 62 hull / 2 turret) | no |
| T3 | 3c ≥ 7, 3e ≥ 7 | 10; – | **10; 10** | yes |
| T4 | rack ≤ 50 % of KOs; crew and fire > 0 | 44 of 45 rack | **20 matches: 52 KOs: ammo fire 31, crew 12, rack 8 (15 %), engine fire 1; case 3: rack 5 of 40** | yes |
| T5 | case 6 ≥ 60 %; case 4 flank ≥ 50 % | 0 %; 42.9 % | **60 %; 41.2 %** | case 6 yes, case 4 no |
| T6 | crossing ≤ 60 % of stationary | 83.1 % | **86.2 %** | no (S3b-21) |
| T12 | 12–25 min all 20; HE share < 60 % | 11.7–26.7 | **11.0–25.8 (one under, one over); HE max 44.6 %, mean 28.3 %** | length no, HE yes |
| T13 | peak p99 < 1.5 ms; stress p99 < 10 | – | **0.94 ms; 1.21 ms** | yes |
| T14 | lip ≥ 0.05 m per burst | 0.001 | **0.0024** | no (S3b-22) |
| T11 | fortify S1 rows pass | pass, mortar 37.8 % | **pass: rifles 0 / 80; mortar 6 / 58 = 10.3 %** | yes |

Other numbers: tanks lost / bought GER 26 / 62, USA 26 / 67 (before 19 /
54, 26 / 56); AI-vs-AI seeds 1–5 (normal bases): Germany 2, USA 3,
12.5–18.2 min; case 9 open-ground losses 74 (baseline 80, -7.5 %);
case 4 front 20.3 %, HE variant 9.1 %; case 2 hull-down won 4, open 2,
both 1, none 3; case 10 3 tracks over 199 shells (unchanged); case 7 and
case 5 unchanged in kind (Part B). AI rockets: fired in 4 of 20 matches
per side; satchels at vehicles 0 (Part B). A duel can now end with both
tanks immobilised and blind (the commander lost; sight 70 % of 175 m);
`gunnery.rs` tests allow up to 2 such draws in 6.

**For Part B.** `SAVE_VERSION` stays 10 and `PROTOCOL_VERSION` 4 (the new
`AmmoChoice::Apcr` rides on the slice's protocol bump). Tests with a
"disarmed" target (driver only) set `crew_ko_below = 0` in their setup.
Case 7's defenders still are not pinned (1.8 %); case 5's tank still fires
nothing: both are Part B (Suppress, duck state). `crossing_target` also
sets `rack_fire_pct = 0`, and `toughen` clears `ammo_fire`.

## Part B: AI and trench pinning (stage done, 2026-11-01)

Built as specified in B1–B5 with the deviations below; B6 (AI angling) is
not built (owner question S3b-29). Every new gameplay value is a commented
key in `mods/ww2`; `SAVE_VERSION` stays 10 and `PROTOCOL_VERSION` stays 4
(the slice's bumps).

**B1 Duck state** (`combat.rs` `status`): `Combat::ducked` (saved, hashed,
validated: only a living soldier in a trench, never with `pinned`). A man
in a trench ducks at `[combat.suppression] duck_points` and comes up below
`duck_release_points`, both (and `pin_suppression`) scaled by the squad
type's new `steadiness_pct` (unit defs, default 100, range 25–400; the
veterancy hook). A ducked man stays crouched below the lip, does not aim,
fire, fire his launcher or start a throw (a wind-up is called off), keeps
his target and pays the half second of aiming when he comes up. New:
`duck_decay_pct`, his meter decays at that share of the normal rate (he
keeps his head down until the fire slackens).

**Physical-contact fix (audit #4, owner question S3b-24):** a near miss is
now a round passing (or striking earth) within `near_miss_cm` of the man's
hit-capsule axis, head to feet, not of his middle. A crouched man in a
trench has his middle 1 m below ground, so rounds cracking over his head
or into the parapet in front of his face did not count.

**B2 Suppress order:** `CommandKind::SuppressUnits { units, line }` (1–2
points; a second point beyond `combat.suppress.line_max_m` is pulled in;
relay limit: ids ≤ `MAX_UNITS`, points 1–2, `MAX_SUPPRESS_POINTS`).
Squads named whole get `Order::Suppress { line }` and hold; other men a
detached `UnitOrder::Suppress`; vehicles stop and keep
`VehicleState::suppress`; mortar men are dropped. All saved, hashed and
validated (on the map, no longer than the rules allow, never on a vehicle
squad, cleared on a wreck). Men: engage rule `Still`, no target search;
each shot aims at a counter-RNG point on the line (`Purpose::SUPPRESS`,
tick and shooter) at `aim_cm` above the higher of the ground and the
undug surface, within range, and is fired only with a clear terrain line
to a probe point `clear_cm` short of it (one ray per shot). In a trench
they rise to the fire step to fire. Vehicles: the main gun takes only
vehicle targets (self-defence); with none it lays on the line's middle at
`tank_he_aim_cm` and fires HE (Auto or He) every `tank_he_s` while more
than `tank_he_reserve` are left and no own man is within `he_clear`; the
coax (within its cone) and hull MG (within its arc) fire at random points
on the line. Snapshot: `UnitSnapshot` target = the line's middle,
`VehicleSnapshot::suppress`. Bridge: intent `suppress_units` (`polyline`
or `target`), squad dict `order "suppress"` + `suppress_line`, vehicle
`suppress_line`, rule `suppress_line_max_m`. HUD: a Suppress button in
both grids (soldier row 1 now has five slots), hotkey **P**, click a point
or drag a line (capped, dashed accent preview), "Suppressing" card state,
lines drawn for selected units, tool hint. Docs: `godot/docs/match_state.md`.

**B3–B5 AI** (`ai/assault.rs`, module docs step 8 in `ai.rs`):
fire-and-move (base of fire: the squad already suppressing, else an MG
team, else a rifle squad; up to 8 candidate spots with one terrain ray
each; leapfrog bounds; lift at `lift_fire_m`), tanks halted at their
assault or standoff goal suppress the line and lift with a Stop, AT-team
hunts (hold when in reach for the target's motion, else the nearest of
five spots off the turret's front; a team already hunting keeps at it
near its last spot), satchel hunts (`AiState::slow_since`, saved, hashed,
validated). Counters `suppress_orders`, `bounds`, `at_hunts`,
`satchel_hunts` in `AiCounters` and the skirmish report.

**Harness:** case 5b (the case 5 tank on Suppress at a 30 m trench line);
case 7 now: the tank leads its escort to 60 m, then suppresses while the
rifles go in (7c keeps the S3 run, 7b the run without the tank); "up on
the fire step" no longer counts pinned men. fortify-test: "rifle squad
advancing from 150 m, with and without an MG suppressing" (the squad
advances crouched in both, owner question S3b-28), reported against T10 /
T11, not a pass condition.

**Data changed from the spec's starting values** (each explained in its
TOML comment; owner questions S3b-25, S3b-26): `covered_near_miss_pct`
50 → 100; `duck_points` 35 → 25, `duck_release_points` 20 → 10, new
`duck_decay_pct` 25; `aim_cm` 30 → 80; new `tank_he_aim_cm` 0. Measured
reason: an MG at 150 m on a trench line gave each man ~0.5 near misses a
second (wide MG cone, grazing fire), not the ~3 the spec estimated; with
the spec's values the line ducked 12–17 % of the time.

**Results (seeds 1–5 × both factions unless noted):**

| # | Target | Now | Met? |
|---|---|---|---|
| T7 | rockets in ≥ 8 of 10 matches; satchel at a vehicle in ≥ 3 of 10 (normal bases) | rockets 6 of 10 (10 of 20 per faction, 15 of 20 any); satchels 0 of 10 (0 of 20; satchel hunts start, no throw) | **no** |
| T8 | case 7: attacker losses ≤ 50 % of 7b's, defenders ≥ 75 % | attackers 11 vs 80 (13.8 %); defenders 31 of 80 (38.8 %); 7c 52 / 74 | attackers yes, defenders **no** |
| T9 | case 5b pins (> 0 %) and kills (> 0) | pinned 54.2 %, ducked 42.7 % of man-time; kills 0 | pins yes, kills **no** (S3b-27) |
| T10 | fortify MG + rifles from 150 m: defenders ≥ 75 %, attackers ≤ 50 % | defenders 43 of 80 (53.8 %), attackers 28 of 80 (35.0 %) | attackers yes, defenders **no** |
| T11 | fortify S1 rows pass; unsupported squad from 150 m loses ≥ 75 % | rifles 0 vs 80, mortar 10.3 %, smoke pass; unsupported 80 of 80 | yes |
| T1 | each side 35–65 % of 20; no base side > 70 % | GER 7 / USA 13; base 1 wins 15 (75 %) | factions yes, base **no** |
| T12 | 12–25 min; HE share < 60 % | 12.2–24.6 min; HE max 44.6 % | yes |
| T13 | peak p99 < 1.5 ms; stress p99 < 10 ms | 0.65 ms (20 parallel runs); stress 1.19 ms | yes |

Unchanged from Part A: T2 (3a 4 of 10), T5 (case 4 41.2 %, case 6 60 %),
T6 (86.2 %), T14 (0.0013 m per burst).

## Tune results (stage done, 2026-11-01)

> **Superseded by the Fix stage** (`docs/slices/S3b-log.md` "Fix stage"):
> the reviews found this table judged on too few seeds and the M61 row
> built on mixed acceptance criteria. On ≥ 10 seeds and 80 matches T2,
> T5 (flank), T7, T10 and T14 miss; the AP rows now come from one source
> (Bird & Livingston), costs are Panzer IV 620 / Sherman 590, vehicle MGs
> carry stowed rounds. The table below is kept as the Tune stage's record.

Three rounds on `s3b-balance` (round 1 `1bd66bd`, round 2 `38f09b5`, round
3 the final measurement and the test fixes). Measured on the final data:
skirmish seeds 1–10 × both base orders (20 matches, 6 at once,
`--threads 1`), armour-test seeds 1–5 × both factions with `--stress`,
fortify-test 5 seeds. Round-by-round numbers: `docs/slices/S3b-log.md`.

### What changed (each value explained in its TOML comment)

| Step | Change | Why (measured) |
|---|---|---|
| (a) geometry | Turret crew boxes reach down into the turret basket (gunner and commander seated, loader on the basket floor), both tanks; content validation lets a turret-frame part reach down to the hull floor (`vehicle.rs` `basket_def` / `basket`, code) | a hull hit at visible-mass height passed under all three turret crew |
| (a) data | APHE filler `reach_cm` 150 → 300 (PzGr 39, M61), as the 3 m spall reach | rays from a burst 1 m behind the front plate ended in the empty hull middle; tanks survived 5–12 frontal penetrations |
| (a) data | 75 mm M61 penetration row to the US published table (Hunnicutt: 66 mm at 500 yd at 30°) | the old row was ~6 % high; the M61 beat the Panzer IV H front at 300 m 88 % of the time (now 13 %); golden rows 12–16, 18, 19, 28, 29, 32 moved with it |
| (a) data | `moving_target_spread_pct` 300 → 600 (S3b-21 (A)) | 3d: 500 → 64.6 %, 550 → 60.0 %, 600 → 55.4 % |
| (a) data | mortar `frag_count` 96 → 120 (S3b-23 (A)) | barrage open losses 58 → 60 (−19 % of the S1 74; trench 10 %) |
| (b) AI | `ai.assault` `at_hunt_m` 250 → 400, `satchel_hunt_m` 60 → 150 | rockets in 8 of 10 matches (normal bases) |
| (b) cost | Panzer IV 640 → 600, Sherman 560 → 620 (S3b-10 (A), within 15 %) | 40 matches: Germany 15 / USA 23 → 23 / 17 |
| harness | case 9: T14's local row (lip lost within 1 m of each burst, S3b-22 (A)); case 7/7b and fortify's suppressed assault close into the trench after the grenades, the MG / tank lifting at `ai.assault.lift_fire_m` (S3b-33) | the squad used to stop at 25 m, where ducked defenders cannot be hit |

Not changed: (c) PzGr 40 stays as Part A built it; no (d) stat changes.
Tried and dropped: HEAT `core.length_cm` 150 (no effect on case 4 or 6),
`spoil_crater_pct` 250/400 (T14 0.033/0.034 m), two frags per rifleman
(T10 66 % without the close-in), satchel slow speed 2 m/s / 2 s / hunt
250 m (no satchel throws), `mix_at_pct` 15 (a draw, longer matches).

### Final targets

| # | Target | Baseline (plan) | Final | Met? |
|---|---|---|---|---|
| T1 | each faction 35–65 % of 20; no base side > 70 % | GER 4 / USA 16; base 1 14 | GER 11 / USA 9; base 1 11 (55 %) | **met** |
| T2 | 3a frontal: Panzer IV wins 6–8 of 10 | 4 | 6 of 10 (32 of 40 over seeds 1–20) | **met** (lower edge) |
| T3 | 3c ≥ 7, 3e ≥ 7 | 10 / – | 10 / 10 | **met** |
| T4 | rack ≤ 50 % of KOs; crew and fire > 0 | 90 %; crew 5, fire 0 | 9 of 57 (16 %); crew 13, ammo fire 35 (engine fire 0 in these 20; 1 in rounds 1–2) | **met** |
| T5 | case 6 ≥ 60 %; case 4 flank ≥ 50 % | 0 %; 42.9 % | 60 %; 34.3 % | case 6 met, flank **missed** |
| T6 | crossing ≤ 60 % of stationary | 83.1 % | 55.4 % | **met** |
| T7 | rockets in ≥ 8 of 10; a satchel at a vehicle in ≥ 3 of 10 (normal bases) | 4 of 10; 0 | rockets 8 of 10 (12/10 of 20 per side); satchels 0 of 10 (0 of 20) | rockets met, satchels **missed** |
| T8 | case 7: attackers ≤ 50 % of 7b's, defenders ≥ 75 % | 74 vs 80; 68.8 % | 21 vs 80 (26 %); 80 of 80 (100 %) | **met** (harness S3b-33) |
| T9 | case 5b pins (> 0 %) and kills > 0 | 0 % / 0 | pinned 54.2 % + ducked 42.7 %; kills 0 | pins met; kills **missed**, accepted as "pins" per S3b-27 (A) |
| T10 | fortify MG + rifles from 150 m: defenders ≥ 75 %, attackers ≤ 50 % | new | 97.5 %; 48.8 % | **met** (harness S3b-33; attackers near the limit) |
| T11 | fortify S1 rows pass (rifles ≤ 50 %, mortar ≤ 40 %); unsupported rush ≥ 75 % | pass, 37.8 % | rifles 0 %, mortar 10.0 %, smoke pass; unsupported 100 % | **met** |
| T12 | (owner, 2026-11-01, `031791e`) no match under 10 or over 40 min, median 15–30 min; HE share < 60 % | 13.6–26.2; 45.5 % | 11.2–26.2 min, median 18.1; HE max 44.2 % | **met** (the old 12–25 band would miss: 3 over 25, 2 under 12) |
| T13 | peak p99 < 1.5 ms; stress p99 < 10 ms | 0.28–0.50; 2.34 | 0.94 ms (6 parallel); stress 1.14 ms | **met** |
| T14 | HE lip loss ≥ 0.05 m per burst within 3 m | 0.001 m | 0.027 m (local row); 0.0013 m (41 m mean) | **missed** |
| T15 | determinism, save/load, net loopback/soak | pass | 430 tests + release-only (soaks, stress, fortify, AI) pass | **met** |

### Why the misses

- **T5 flank (34 %).** A flank rocket aimed at the visible-mass centre
  crosses the hull at about 1.3 m, where the turret-basket crew now are;
  the Panzer IV's rack bins (low) and turret ring (high) lie off that line,
  and crew kills are not "disabling". The basket fix lowered it from
  41 % (realism over the metric). S3b-35.
- **T7 satchels (0 of 10).** Hunts start but end before a throw (the tank
  moves, drops out of sight, or the engineers die on a 100 m+ walk); data
  could not fix it. S3b-36 (next slice: sticky hunts).
- **T9 kills.** A flat-trajectory shell cannot reach men below the lip
  (S3b-27 (A)).
- **T12 length** is met under the owner's loosened target (`031791e`,
  committed during this stage); against the old 12–25 band 5 of 20 fall
  outside (routs at 11 min, long games at 25–26 min). Round 1 had one
  30-minute time-limit draw (seed 8), which the cost change removed.
- **T14.** Most bursts near the trench miss the spoil; one 0.6 m crater
  lowers the local lip ~3 cm on average. Larger spoil removal barely moves
  it (0.034 m at 400 %). Option (B) of S3b-22 (spoil from a wider radius)
  is the remaining lever.

## Implementation order

### PART A: physical damage, armour, gunnery and content

1. **A1 Fragment rays and overpressure** for shells, rockets, frags and
   satchels (audit #5, #7, #8): HE def keys, validation, ray test,
   budget, `Stats` counters (`frag_rays`, `frag_hits`,
   `frag_rays_capped`). Tests: a wreck or vehicle between a burst and a
   man blocks every ray; a trench wall blocks rays from a burst outside;
   a burst in the trench reaches men in it; determinism across thread
   counts; per-tick cap scaling. Fortify mortar row still ≤ 40 %.
2. **A2 Vehicle-internal rays** (#12, #13, #14): filler rays, inner HE
   rays, the barrel box; remove radial `within` and anchor proximity.
   Tests per path; case 10 still no internal damage.
3. **A3 Content geometry and outcomes:** multi-box components; three
   rack boxes per tank; crew boxes moved; transmission component; ammo
   fire vs detonation; `crew_ko_below`; KO cause 3. Defs with source
   comments; validate the boxes lie inside the hull or turret box.
4. **A4 HEAT reach** (only if case 6 still misses after A3): RPzB 54
   jet 150 cm, comment with the reasoning.
5. **A5 Gunnery:** centre-of-visible-mass aim (tanks and launchers),
   lead estimation error, staging fix for 3b, new case 3e. Replace the
   3b hard check ("aims at the turret") with "aims within 0.3 m of the
   visible-mass centroid".
6. **A6 PzGr 40** third ammo entry: `AmmoChoice` gains `Apcr` (N cycles
   Auto → AP → APCR → HE on a tank that has it), Auto rule, golden
   ballistics rows for it (new rows, existing rows unchanged).
7. **A7 HE vs parapet** (`spoil_crater_pct`).
8. Measure Part A: the 20 AI matches, armour-test, fortify-test, golden;
   then step (b) costs if needed. Checkpoint commit.

### PART B: AI and trench pinning

1. **B1 Duck state** (`Combat::ducked`, keys, `steadiness_pct`).
2. **B2 Suppress order**: command, relay limits (the protocol is
   already 4 since the call-in stage: no second bump), squad,
   unit and vehicle state, fire rules, snapshot, bridge, HUD button and
   P hotkey, presenter line; save version bump (once, with A3's new
   state).
3. **B3 AI fire-and-move**, **B4 AT hunts**, **B5 satchel hunts**,
   **B6 angling**; AI counters in the report.
4. **Harness:** case 5b (the case 5 tank with `SuppressUnits` on the
   trench line), case 7 orders the tank to suppress the trench during
   the approach (7c keeps today's no-suppress run as a report row);
   fortify-test rows "MG suppressing, rifles advance from 150 m" and
   "rifles advance from 150 m, no support".
5. Measure everything against the Targets table; Tune stage follows.

## Targets (the Tune stage measures these)

| # | Target | Baseline | How measured |
|---|---|---|---|
| T1 | Each faction wins 35–65 % of 20 AI matches (seeds 1–10 × both base orders); no base side > 70 % | GER 4 / USA 16; base 1 wins 14 | `skirmish --ai-vs-ai`, seeds 1–10, normal and `--swap-bases` |
| T2 | Frontal duel 300 m (3a): Panzer IV (historically favoured) wins 6–8 of 10 | 4 of 10 | armour-test case 3 |
| T3 | Flank shots win for the flanker: 3c Sherman ≥ 7/10, new 3e Panzer IV ≥ 7/10 | 3c 10/10; 3e new | case 3 |
| T4 | Rack detonations ≤ 50 % of tank knock-outs; crew and fire knock-outs both > 0 | 90 % rack (47/52), crew 5, fire 0 | 20 AI matches pooled; case 3 pooled reported |
| T5 | Case 6 tank disabled ≥ 60 %; case 4 flank disabling-hit rate ≥ 50 % | 0 %; 42.9 % | armour-test 4, 6 |
| T6 | Crossing hit rate ≤ 60 % of stationary | 83.1 % | case 3d |
| T7 | AI fires AT rockets in ≥ 8 of 10 matches; throws ≥ 1 satchel at a vehicle in ≥ 3 of 10 | 4 of 10 (normal bases; 11 of 20 overall); 0 of 10 | seeds 1–10 normal bases (20 reported) |
| T8 | Case 7: attacker losses ≤ 50 % of 7b's and defenders lose ≥ 75 % | 74 vs 80 (92.5 %); 68.8 % | case 7 (tank on Suppress) |
| T9 | Case 5b: tank on Suppress pins men in a trench (> 0 % of man-time pinned or ducked) and kills > 0 | 0 % / 0 (no order: tank fired 0 rounds) | case 5b |
| T10 | New fortify row, MG suppressing + rifle squad from 150 m: defenders lose ≥ 75 %, attackers ≤ 50 % | new | fortify-test |
| T11 | Trenches stay strong: fortify S1 rows pass (rifles ≤ 50 %, mortar ≤ 40 %); unsupported rifle squad from 150 m loses ≥ 75 % | pass; mortar 37.8 %; new | fortify-test |
| T12 | Skirmish length: no match under 10 min or over 40 min (no blowouts, no stalemates), median 15–30 min (owner 2026-11-01: "That isn't necessarily bad, some matches could be longer"; was 12–25 min in all 20); HE share < 60 % in every match | 13.6–26.2 (one over); max 45.5 % (63.7 % once in the slot-swap set) | 20 AI matches |
| T13 | Sim tick p99 at peak < 1.5 ms (skirmish); stress p99 < 10 ms | 0.28–0.50; 2.34 | skirmish timing; case 14 |
| T14 | HE lip loss ≥ 0.05 m per burst within 3 m (report) | 0.001 m | case 9 |
| T15 | Determinism: all hashes equal across thread counts; save/load round trip; net loopback and soak 0 mismatches | pass | tests |

## Owner questions (work proceeds on (A); also in `docs/TODO.md`)

1. **Suppress hotkey.** (A) P. (B) Another free key (K, U, I).
2. **Ammo for suppressive fire.** (A) Infantry pay only in rate
   (magazines and reloads; there is no infantry ammo pool); tanks spend
   real HE with a reserve. (B) Add an infantry ammo pool and resupply
   (its own slice).
3. **Ducking in the open.** (A) The duck state is for men in trenches;
   open-ground suppression is unchanged (crouch 30, pinned prone 70 with
   200 % spread). (B) Men in the open duck too (stop firing at 35).
4. **Sherman stowage.** (A) M4A3 (75) dry stowage, sponson racks (the
   def's name, 1943–44; flank hits burn Shermans). (B) M4A3 (75)W wet
   stowage (floor bins; rack fires much rarer; the Sherman gets
   stronger).
5. **Bail-out.** (A) Still out; a tank with fewer than 2 crew alive is
   knocked out and the survivors count as lost. (B) Build bail-out (crew
   squad) in this slice.
6. **Moving-target misses.** (A) Lead estimation error per engagement
   (±40 %, shrinking as the gunner ranges in). (B) Only a larger
   `moving_target_spread_pct`.
7. **PzGr 40 APCR.** (A) Build it: 6 rounds, Auto uses it only when AP
   would likely fail; small effect at our ranges. (B) Skip it.
8. **Towed AT guns** (Pak 40, 57 mm M1). (A) Next slice (S3c). (B) Not
   planned.
9. **Veterancy hook.** (A) `steadiness_pct` per squad type, all 100
   now. (B) No hook until veterancy exists.
10. **Cost changes.** (A) Tune may move tank costs within ±15 % and HE
    loads to hit T1, each logged. (B) Ask before any cost change.
11. **Harness orders.** (A) Cases 5b and 7 order the tank to Suppress
    the trench (what a player would do); today's runs stay as report
    rows. (B) Keep the old harness and expect the AI-like behaviour
    without orders.
12. **Transmission component.** (A) Add it (realistic front gearbox;
    frontal centre hits immobilise). (B) Fold it into the engine.

## Slice log

Results go in `docs/slices/S3b-log.md` (created by the Tune stage); the
final targets table is in its "Fix stage" section.
