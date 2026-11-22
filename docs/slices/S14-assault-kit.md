# S14: assault kit (flamethrowers, trench clearing, then melee): plan

Status: **plan only** (2026-11-22), nothing built. **Every design question settled on its (A)** (section 12; the real owner questions
S14-1..7 are in `docs/TODO.md` marked "(A) assumed"; any can be reopened before S14a Build A starts). Base: `main` at `6f7bda0` (S12
merged). S14 builds **after S13** (plan `S13-heavy-wave.md`, being built), from a fresh main with S13a and S13-ai merged; `<BASE>` is
that main's head when Build A starts. Card: `docs/design/roster/roster-expansion.md` "S14: Assault kit"; design: `infantry-structures.md`
4.2-4.3, 6.5; roster rows Sturmpioniergruppe (3.1) and Assault Section (3.2). Working notes: `S14-research-history.md` (figures,
sources), `S14-research-code-audit.md` (file:line audit); notes are inputs, this file decides. Pattern: `S12-wounds-medics.md`.
Owner rules as every slice since S10: features first, no Tune; Germany pooled 480 in 35-65 %; tick p99 must not regress; all tuning in
TOML; damage only from projectile or fragment contact (a flame particle and a strike segment are contact); every behaviour behind a
switch; all S14 switches off = `<BASE>`, match for match.

## 0. Summary and why this shape
- **Size.** `workflows.md` sizes S14 at 0.8x S3. Read against the code it is nearer **1.0x**: a new projectile kind with its own list,
  a burning state and burning ground, a weapon class, fuel, a trench-walking order, a melee strike, two commands, an AI job and a
  presenter pass. So it follows the S10 owner rule and splits: **S14a mechanics** (switches 0 at the end, proven neutral; one
  `flame_only` 480 that gates the AI half) and **S14-ai** (AI flame jobs, refuel, trench clearing, switches on, 480, ablations).
- **Flame never touches the projectile pool.** The RE risk was flame starving rifle and shell rounds in the shared 1,800-slot pool.
  Particles live in their own list (`MatchState::flame`, as thrown items live in `MatchState::thrown`), capped per player. A flame
  request can never take a pool slot, and a full pool can never drop a particle: the done-when rule holds by construction, and a test
  proves it (F9).
- **The bunker falls by geometry.** The S10b slit is a gap between boxes; a particle that passes it hits men inside or lands on the
  pit floor and burns there. No bunker rule is added.
- **Damage goes through S12.** Particle hits, burning and burning ground all call `hurt` -> `wounds::deal` with a new FLAME class, so
  downs, bleed-out, credit and the HE share keep their meaning.
- **Order in the card** ("flamethrowers, then melee"): Build A flame, Build B trench clearing and the data, Build C melee.

## 1. Scope
**S14a (engine).** Build A: layout commit, flame weapon class, particle list, burn, burning patches, step-out, fuel and resupply,
snapshot fields. Build B: `ClearTrench` order; the data (two flamethrowers, two fuel cartridges, flame men in two cards behind `when`,
melee blocks on infantry weapons). Build C: melee strike and `Strike`. Fortify rows R1-R6. Neutral proof; the `flame_only` 480 gate.
**Presenter P (Sonnet, after Build C).** Stream, patches, burning men, strike pose, Clear trench button, Strike key `V`.
**S14-ai.** Flame job, refuel, flame buy, AI clear trench; switches on; 480; ablations; Review; Fix; verify; Docs.
**Out (reason).** Flame on vehicles, pieces, crates, wire (S14-6 (A)); fuel-tank rupture (S14-2 (A)); fixing bayonets and a
`FixBayonets` order (S14-4 (A); Soviet fixed bayonets come with S15); flamethrower tanks and the Wasp; ROKS-3 and Soviet flame (S15);
path costs around fire (S14-7 (A)); smoke from fire; a separate flamethrower team card (S14-3 (A)); the M2-2 as a card (4.1).

## 2. Switches and keys (Build A's layout commit adds all; the stage in brackets turns a switch on)
| Table | Key | Default | Meaning; source |
|---|---|---|---|
| `[flame]` | `enabled` | 0 in S14a, 1 in [S14-ai] | F1-F9 master switch; 0: flame members resolve to today's men (4.2), no particle |
| | `particles_per_player` | 64 | F1 cap (hard ceiling `MAX_FLAME_PER_PLAYER` 256); 2 throwers x 12/s x 2.5 s = 60 |
| | `ttl_ms` | 2500 | F2 lifetime (30 m at 25 m/s is about 1.3 s) |
| | `near_cm` | 300 | F4 near miss of a particle (`[combat] near_miss_cm` is 200: flame frightens wider; design 4.3) |
| | `burn_s` / `burn_hp_per_s` / `pulse_ms` | 4 / 15 / 500 | F3 a man struck burns (refreshed by each hit) |
| | `burn_suppression` | 40 | F3 points per pulse while burning (he is pinned: physical, he touches fire) |
| | `patch_radius_cm` / `patch_s` / `patch_hp_per_s` / `patch_z_cm` | 100 / 15 / 20 / 100 | F5; design 4.3 "radius 1 m, 10-20 s" |
| | `patch_merge_cm` / `patches_max` | 50 / 128 | F5 a landing within 50 cm refreshes a patch; hard ceiling `MAX_PATCHES` 256 |
| | `step_out` / `step_out_m` | 1 / 2 | F6 men leave burning ground |
| `[melee]` | `enabled` | 0 in S14a, 1 in [S14-ai] | M1-M5 switch |
| | `reach_cm` / `damage` / `windup_ms` / `recover_ms` | 60 / 60 / 500 / 1000 | M1 default strike (entrenching spade) for a weapon without `[melee]` |
| | `start_pad_cm` | 30 | M2 starts when an enemy capsule is within reach + pad |
| `[clear]` | `enabled` | 0 in S14a, 1 in [S14-ai] | C1-C5 (`ClearTrench` refused at 0) |
| | `step_m` / `steps_max` / `throwers` / `settle_s` / `walk_cells` | 8 / 6 / 2 / 2 / 64 | C2-C4 |
| `[ai.flame]` | `jobs` / `refuel` / `buy` / `clear` | 0 | S14-ai A1-A4 switches |
| | `target_m` / `stage_m` / `stand_m` / `reserve_pct` / `job_max_s` / `stall_s` / `cards_max` / `clear_near_m` / `recheck_s` | 40 / 40 / 20 / 10 / 180 / 45 / 2 / 15 / 60 | A1-A4 |
Weapon def keys (Build A): `class = "flame"`; `[melee] reach_cm, damage, windup_ms, recover_ms` on any infantry weapon (Build C).
Member key (Build A): `when = "flame_on" | "flame_off"` (absent: always). Ranges: switches 0..=1; seconds 1..=3600; cm 1..=1000;
percents 0..=400; `*_hp_per_s` 0..=1000; `particles_per_player` 1..=256, `patches_max` 1..=256. Ablations (`targets/S14-ablations/`): `s14_off` (all 0:
equals `<BASE>`), `flame_only` (`[flame] enabled` 1, rest 0), `no_flame_ai`, `no_clear`, `no_melee`, `no_patches` (`patch_s` 0 is
refused by range: `patch_hp_per_s` 0 instead).

## 3. Flame rules (S14a Build A; integers and `Fx`, ascending ids)
### 3.1 Data model (all saved fields in Build A's ONE layout commit; B, C and S14-ai change no layout)
- **`SAVE_VERSION` +1** over `<BASE>` (32 if S13a took 31). **`PROTOCOL_VERSION` +1** (17): `ClearTrench { squad, at: FxVec2 }`,
  `Strike { units: Vec<SimId> }`, declared with their wire encoding and refused by the sim until Builds B and C; relay doc line.
- `MatchState::flame: Vec<Flame>` (ascending id; at most `particles_per_player` per player) with `next_flame: u32`:
  `Flame { id, pos: FxVec3, vel: FxVec3, ttl: u16, player: u8, shooter: SimId, weapon: u16 }`. `MatchState::patches: Vec<Patch>`
  (ascending id, at most `patches_max`) with `next_patch`: `Patch { id, pos: FxVec3, ends_at: u64, player: u8, by: (u8, u16) }`.
- `Combat` += `close: Close { burn_until: u64 (0: not burning), burn_by: (u8, u16), strike: u8 (ticks left), strike_phase: u8 (0 none,
  1 wind-up, 2 recovery), strike_at: FxVec2 }`; hashed with its chunk.
- `kill_class` += `FLAME = 12`, `MELEE = 13` (`COUNT` 14, still under the low nibble's 16); `fired_class` += `FLAME` (COUNT 12).
- `MatchState::clears: BTreeMap<SquadId, Clear>` (C1-C5): `Clear { at: FxVec2, cell: (u16, u16), from: (u16, u16), step: u8, phase:
  u8, phase_at: u64 }`.
- `PlayerState::s14: S14Counters`: `flame_fired`, `flame_capped` (requests over the cap), `flame_hits`, `flame_in_trench_hits`,
  `flame_in_bunker_hits`, `burn_downs`, `burn_kills`, `patches_made`, `patch_hits`, `stepped_out`, `strikes`, `strike_hits`,
  `strike_kills`, `clears`, `clear_steps`, `clears_done`. `AiCounters` += `flame_jobs`, `flame_job_ends[6]` (done, spent, dead, weak,
  deadline, stall), `flame_buys`, `refuels`, `clear_orders`.
- No new RNG purpose: particle spread uses the shot's `Purpose::AIM` stream (as every shot). Melee uses no RNG.
- Load validation `load_rejects_bad_s14_state` (one case per check: cap, ids ascending, ttl, patch end in the past, strike phase,
  clear cell dug); `save_fuzz.rs` staging: particles in flight, a patch, a burning man, a man mid-strike, a squad mid-clear.

### 3.2 Rules
- **F1 Fire.** `WeaponClass::Flame` fires like an MG through `aim_and_fire`: target search, `hold_beyond_pct`, `burst_rounds` /
  `burst_pause_ticks`, one particle per shot interval, each taking one fuel unit from the man's feed. Its `FireRequest` gets a `flame`
  flag; `spawn_rounds` (`combat.rs` 2014) routes flagged requests to `MatchState::flame` (ascending request order) and never to the
  pool; one over the player's cap is dropped and counted in `flame_capped`, not in `shots_dropped`. `SuppressUnits` at a point works
  as for MGs (the AI's way to flame a slit). A flame man is a crew weapon carrier (`crew = true`, handover to `crew_assistant`); dry,
  he draws his sidearm (S5a rule, `combat.rs` 716-733).
- **F2 Flight** (Ballistics stage, serial, after `fly_thrown`, ascending id): 60 Hz substeps with gravity (no drag; nothing beyond
  the weapon's `range_m` is engaged, as for every weapon), each swept segment tested in this order of `t`: terrain (dug ground included), enemy static boxes
  (`structures::first_static`: own pieces passed, so a bunker's own crew is safe from its own flame and the slit gap lets particles
  in), vehicle and wreck boxes (`combat::first_box`, made `pub(crate)`), soldier capsules and downed men of other players
  (`capsule`, `seg_seg`; own men passed as by every direct round, S14-1). Ends: a capsule hit (F3, and a patch at his feet), a
  ground or box-top landing (F5 patch at the point), a box side or a vehicle (no effect, S14-6), `ttl`, map edge.
- **F3 Burn.** A struck man takes the weapon's `damage` at once and burns until `now + burn_s` (refreshed by each hit; `burn_by` the
  firer and FLAME). Every `pulse_ms` (Status stage, `(tick + id) % pulse` in id order) a burning man takes `burn_hp_per_s * pulse /
  1000` through `hurt` (`ZONE_NONE`, not blast) and `burn_suppression` points. With `[wounds] enabled` he goes wounded, then downed,
  and the next pulse on a downed man kills him (W3: any later hit kills); a downed man keeps burning. Numbers: one particle 20, then
  60 over 4 s: a man in the stream for 1 s (3-5 hits) is down or dead; one stray particle leaves him wounded and pinned.
- **F4 Fear.** A particle passing within `near_cm` of a man's capsule axis (the `fly` near-miss test with a wider radius) adds the
  weapon's `suppression` (35: two near misses pin; a ducked man in a trench stays ducked). Physical: only real particles count.
- **F5 Burning ground.** A landing makes a patch (or refreshes one within `patch_merge_cm` of the point, keeping the older id); at
  `patches_max` a new one is dropped and counted. Every `pulse_ms` each patch hurts every soldier and downed man whose feet are within
  `patch_radius_cm` horizontally and `patch_z_cm` vertically of it (any owner: S14-1 (A)) by `patch_hp_per_s * pulse / 1000` (FLAME,
  credited to the patch's `by`; own-side burns count as own fire, not kills, as S11a2) and sets him burning. Expired patches go in the
  Status stage. Patches do not block sight or paths (S14-7 (A)).
- **F6 Step out.** With `step_out` 1, a man whose feet are in a patch and who has no move order of his own (formation or holding) takes
  a detached move to the first of 8 points `step_out_m` beyond the patch edge (ascending angle from the direction away from the
  patch centre) that is not in a patch and is walkable; he rejoins at the squad's next order or `Regroup` (the S12 detached rule).
  Crews and riders do not step out. This is what drives defenders out of a burning bay.
- **F7 Fuel and resupply.** Fuel is a cartridge (`supply = { kind = "explosives", g }`), `load` units a man. Resupply serves it at the
  existing per-kind rate (one unit per 2 s at a source; design 6.5) and price (`explosives_mp_per_kg`, D1). No other source.
- **F8 Credit.** A flame down or kill is class FLAME in `kills_by`, `downed_by_class`, `finished_by_class`; it is not HE (T8's HE
  share is unchanged in meaning; flame takes some of the trench kills HE took).
- **F9 Pool isolation (test).** `flame_never_takes_pool_slots`: a pool saturated by staged rifle and mortar fire, two flame men
  firing at empty ground 50 m aside: `shots_dropped`, the pool contents and every shell equal the scene without them, tick for tick; flame live never exceeds the
  cap; control: the cap at 4 counts `flame_capped`. `two_flamers_stay_in_cap` (the RE done-when line).

## 4. Data (S14a Build B; every value with `sources`; figures in `S14-research-history.md`)
### 4.1 Weapons and fuel
| Def | Range m | Particles/s (`rounds_per_min`) | Burst (`burst_rounds`, pause ms) | Fuel (`load`, units) | Unit g | m/s | Spread mrad | Damage | Supp. |
|---|---|---|---|---|---|---|---|---|---|
| `fmw41` Flammenwerfer 41 | 30 | 12 (720) | 12, 1000 (10 ignitions) | 120 = 11.8 L, 10 s | 100 | 25 | 60 | 20 | 35 |
| `m1a1_flamethrower` M1A1 | 35 | 12 (720) | 12, 1000 | 108 = 17.8 L, 9 s | 130 | 27 | 60 | 20 | 35 |
Both: `class = "flame"`, `crew = true`, `handover_ms` 5000, `fire_reveal_pct` 300 for 5000 ms, `hold_beyond_pct` 100, `magazine` =
`load` (no reload: the tank is the magazine). Cartridges `fmw41_fuel`, `m1a1_fuel`. Refill cost about 6 MP (12 kg at 0.5 MP/kg) and
7 MP (14 kg; napalm-gasoline about 0.78 kg/L): cheap, as D1 wants small arms and explosives. **US model:** the roster says M2-2; the sources put the
M1A1 with the June 1944 assault teams (about 150 landed on D-Day) and do not confirm the M2-2 in France then. Settled: the 1944-06
card carries the M1A1; `m2_2_flamethrower` (`extends` the M1A1: 15 L = 84 units at `g` 135, 7 s, 40 m, bursts of 17 = 5 ignitions)
is added as a def with no card, for a later date-filtered variant. If the data stage finds the M2-2 with US units in France from
June 1944, it swaps them and says so.
### 4.2 Cards (no new card; size, price, cooldown and weights unchanged: S14-3 (A))
- **Sturmpioniergruppe** (8): the `sturmpionier_smg` man (MP 40) becomes a `when` pair under one new slot name `flammenwerfer`, at
  his place: `when = "flame_off"` (today's man: MP 40, two stick grenades) and `when = "flame_on"` (weapon `fmw41`, sidearm `p38`,
  `crew_gunner`, two stick grenades). The `sturmpionier_stg` man becomes a pair the same way (slot `flammenwerfer_helfer`), `flame_on`
  adding `crew_assistant` (he takes the weapon up). Historical: pioneers alone carried flamethrowers, attached for an assault (US IB
  1944: 2 per 60-man pioneer platoon).
- **Assault Section** (12): the `rifleman` slot (count 3) becomes count 2 followed by the pair `rifleman_flame`: `flame_off` (today's
  M1 rifleman) and `flame_on` (`m1a1_flamethrower`, sidearm `m1911a1`, `crew_gunner`); the `rifleman_smoke` slot (count 2) becomes
  count 1 followed by the pair `rifleman_smoke_flame`, `flame_on` adding `crew_assistant`. Men, order and kit with `flame_off` are
  exactly today's; only slot names on the squad card change.
- The setup build drops every member whose `when` does not match `[flame] enabled` before anything else reads the card (as S11b's
  `rifle_grenades` 0 drops a kind, `setup.rs` 367-373); the slot-name uniqueness check (`content/variant.rs` 358) counts a
  `flame_on` / `flame_off` pair as one slot, so the pair shares one name. Test: with `enabled` 0 both cards spawn the same men, man for man, as `<BASE>`. `flame` is a
  **derived** capability (a member with a Flame-class weapon), like `at_hunt`: added to `CAPABILITIES` and `DERIVED_CAPABILITIES`,
  removed from `RESERVED_CAPABILITIES` (`content/variant.rs` 87-108); `Capability::Flame = 15`.
### 4.3 Melee blocks (Build C data; reach cm / damage / wind-up ms / recovery ms)
Rifles (Kar98k, M1 Garand, M1 Carbine, BAR): butt stroke 100 / 50 / 600 / 1000. SMGs and the StG 44: 80 / 40 / 500 / 1000. Pistols,
MGs, launchers, mortars, flamethrowers: no block, the rules' spade (60 / 60 / 500 / 1000). Bayonet (180 cm) only as a later
`FixBayonets` (S14-4 (A)). Source: `infantry-structures.md` 4.2 (reach and timings); damage chosen so a strike to the torso wounds
(W3: 50-60 against 100 HP), two down a man: the design's "usually lethal or down" with the spade.

## 5. Melee (S14a Build C)
- **M1 Strike.** A short segment from the striker's chest (his `fire_height` less 40 cm) toward `strike_at`, `reach_cm` long. It
  tests enemy soldier capsules exactly like a bullet substep (`capsule`, `seg_seg`; nearest along the segment, ties by id), no flight
  time; no hit, no damage. Downed men are never struck (W12); vehicles and pieces stop nothing (a strike never reaches past 1 m).
- **M2 When.** Automatic (players and AI alike): a soldier, not pinned, ducked, prone, riding, crewing or carrying, with an enemy
  soldier's capsule within `reach_cm + start_pad_cm`, **and** his active weapon cannot fire within the wind-up: `cooldown` (reload
  included) longer than the wind-up, or dry with no reload, or a launcher, mortar or dry flamethrower in his hands. Otherwise he shoots
  (at 1 m an SMG burst beats a butt stroke). `Strike { units }` (direct control `V`) starts one at once at the nearest enemy in reach (refused with
  `[melee] enabled` 0, or with nobody in reach).
- **M3 Phases.** Wind-up starts in `aim_and_fire` (per chunk, his own fields only): `strike_at` = the target's position then, he does
  not fire. At its end the strike request goes to a per-chunk buffer, merged and **resolved serially in the Damage stage** in
  ascending striker id; a striker downed or killed earlier in the same stage does not strike. Then `recover_ms` with no fire.
- **M4 Damage.** `hurt` with the zone at the hit point (W1) and class MELEE; the struck man gets `[combat] hit_suppression` as any hit.
  No near miss and no suppression to others (design 4.2: only a swing within `near_miss_cm` would; not modelled, stated).
- **M5 Tests.** `strike_hits_only_a_touched_capsule` (target 0.9 m in front: hit; control 0.9 m to the side of the segment: none),
  `target_steps_back_strike_misses`, `loaded_smg_shoots_not_strikes`, `reloading_rifle_strikes`, `pinned_man_does_not_strike`,
  `no_strike_at_downed`, `two_strikers_resolve_in_id_order`, `melee_off_is_base` (40/40).

## 6. Clear trench (S14a Build B)
- **C1 Order.** `ClearTrench { squad, at }`: an own infantry squad with at least one frag among its men (S14-5 (A)); `at` within 8 m
  of a cell dug `TRENCH_NAV_DM` or deeper (`terrain.rs` 65, `dug_dm` 647). Refused otherwise, and with `[clear] enabled` 0. Any later
  order to the squad ends it.
- **C2 Approach.** AttackMove to 6 m short of the entry cell on the squad's side, crouched.
- **C3 Step.** From the current cell a breadth-first walk over dug cells (at most `walk_cells`, 4 m cells) picks the next point: the
  reached cell at walk distance >= `step_m` that lies farthest from the entry, ties by lower grid index; none: the farthest reached.
  Up to `throwers` men with frags nearest the point (ids ascending) `Throw` at it; a flame man with fuel fires one burst at it
  (a `SuppressUnits` of the point for one burst); the squad waits the fuse plus `settle_s`, and with `[flame]` on, until no own patch
  burns within 2 m of the point (at most `patch_s`).
- **C4 Advance.** AttackMove into the point; at arrival the next step. **C5 End:** `steps_max` steps, no further dug cell, the squad
  under half strength, or no frag and no fuel left: the job ends (`clears_done`), the squad holds the trench.
- Tests: `clear_trench_walks_the_trench` (an L-shaped trench: the steps follow the bend), `clear_refused_off_trench`,
  `clear_ends_on_new_order`, `clear_waits_for_own_fire`, `clear_off_is_refused`, save/load mid-step.

## 7. S14-ai (Opus; `ai/flame.rs`; every key and saved field already in S14a's layout commit, all off)
- **A1 Flame job** (`jobs`). Want: an own squad with the `flame` capability and a fuelled flame man (fuel over `reserve_pct`),
  attacking point `i`, and a known enemy within `target_m` of the point standing in a dug cell, or a known enemy bunker (its centre)
  near the point. At most one job per point, four in all. Phases (S10a-ai breach shape): **Close** (AttackMove to `stage_m` short,
  crouched; the point's base of fire aims at the target by `Planner::flame_focus` beside `breach_focus`, `ai/breach.rs` 736);
  **Smoke** (the breach smoke rule if a man carries smoke); **Go** (the flame man `MoveUnits` to `stand_m` from the target, the squad
  follows by AttackMove); **Burn** (`SuppressUnits` at the target: for a bunker the slit point, centre plus facing times half depth at
  slit height, from the fort def; else the trench cell), until fuel <= `reserve_pct` or no known enemy within 6 m of it; **Clear**
  (a trench target and `clear` 1: `ClearTrench` at it). Ends (counted): done, spent, operator dead with no pickup, squad under half,
  point no longer attacked, `job_max_s` (L1), no phase progress for `stall_s` (L2); a failed target is not retried for `recheck_s` (L3).
- **A2 Refuel** (`refuel`). A flame squad whose flame man is at or under `reserve_pct` and has no job takes the S5a Refill task
  (`ai/ammo.rs`) even when `squad_ammo_pct` is high (the squad average hides a dry tank).
- **A3 Buy** (`buy`). A point attacked with a known trench or bunker defender and no live own flame squad: the urgent buy (`ai.rs`
  1724) takes the faction's flame card, at most `cards_max` alive. Weights stay (Sturm 60, Assault Section 35).
- **A4 Clear** (`clear`). An own infantry squad attacking a point within `clear_near_m` of a known enemy in a dug cell, with two frags
  or more and no other job: `ClearTrench` at that cell, once per squad per `recheck_s`.
- Liveness tests (L1-L3 as S10b-ai): `flame_job_ends_when_operator_dies`, `..._on_deadline`, `..._on_stall`, `refuel_when_tank_dry`
  (control `refuel` 0: stays), `flame_buy_once_per_cap`, `ai_flames_the_slit` (staged bunker: the `SuppressUnits` line is the slit),
  `ai_clears_after_burn`, save fuzz with jobs in each phase, threads 1 vs 4.

## 8. Determinism, cost, guard rails
No new RNG purpose. Particles, patches, strikes and clears run serially in ascending id; per-chunk work writes only its chunk.
Cost: at most 64 particles a player, each a serial 60 Hz substep (soldier hash, downed hash, vehicle index, the statics scan);
patches once per pulse, one hash query each. Tick p99 from `tools/verify.sh` only, at most 25 % over `<BASE>`; a flame-heavy staged
save (`flame_bench`: four flamers and 128 patches) is timed too. Hashes equal at 1, 2, 3, 8 threads and through save/load with
particles in flight, patches, burning men, strikes mid-wind-up and clears mid-step.

## 9. Fortify rows (`fortify10.s14` scene; switches on in the row only; player commands, no AI)
- **R1 Slit (hard).** The S10b bunker row layout (`cli/fortify/bunker.rs`): five of the owner's men in the pit; the other side's flame
  man 20 m in front `SuppressUnits` at the slit for his tank. `out_pct` (dead or downed) 30 s after he starts: pass when the mean is
  >= 80 % and 8 of 10 runs have at least 4 out. Controls: slit closed (front walls meet, as `satchel_at_slit_reaches_men_inside`)
  <= 10 %; the same fire at the back wall <= 10 %. (RE done-when 1.)
- **R2 Pool (hard).** F9 as a row: two flame men and a rifle and MG volley that saturates the pool, plus three mortar bombs; rifle and
  shell drops equal the no-flame control run for run; flame live <= cap. (RE done-when 2.)
- **R3 Flame vs trench (report; RE done-when 4).** The S10a `assault_run` scene: the assault card with flame (`[flame] enabled` 1)
  against the rifle baseline (`enabled` 0): defenders out, attackers lost, time to the trench.
- **R4 Clear trench (report).** A squad ordered `ClearTrench` into an L-trench held by four men 6 m apart: steps done, defenders out,
  attackers lost; control the same squad by AttackMove.
- **R5 Melee (hard).** M5's capsule rule as a row: 100 strikes at a man 0.9 m ahead hit 100; at 0.9 m aside hit 0. (RE done-when 5.)
- **R6 Trench under fire (report).** A ducked trench line (MG suppression as S3b) with a flame man at 25 m: out %, stepped-out men.

## 10. Measures and targets (`docs/slices/targets/S14.toml`, written by Build A as a copy of the then-current targets plus T185+)
- **Gate to S14-ai:** `flame_only` 480 (the AI blind to flame: flame men fire only by auto-targeting): Germany 35-65 %, match
  length inside the owner rule (median 15-30, none under 10 or over 40), no match without a winner. Otherwise stop for the owner.
- **S14a hard:** T185 `s14_off` 40 set = `<BASE>` 40/40 (`tools/neutral.py`); T186a-c R1 and controls; T187 R2; T188 R5.
- **S14-ai hard:** T189 Germany pooled 35-65 %; T190 no winner 0, length rule; **T191 `flame_matches_pct` >= 50** (matches in which
  an AI flame man hit a man in a trench or bunker: RE "5 of 10"); T192 every flame job ends (no job older than `job_max_s`); T193
  tick p99; paired `s14_off` vs default on the same 480: a pooled move of 5 points or more stops the slice for the owner (as S12).
- **Reports:** T195 flame kills and downs per match by side; T196 HE share of casualties (T8) beside the flame share; T197 strikes,
  strike hits and kills per match; T198 clear orders, steps, done; T199 refuels; T200 `flame_capped` and patches dropped; R3, R4, R6.
- Ablations on 480: `no_flame_ai`, `no_clear`, `no_melee`, `no_patches`. Levers if T189 fails, in order: `[ai.flame] cards_max`,
  `jobs` timing, `burn_hp_per_s`, `patch_hp_per_s`; never squad sizes or prices (owner class).

## 11. Stages (`workflows.md` 1-4: engine tier; no per-slice Tune; stage briefs `S14a-build-X.md` written at each stage's start, as S12)
| Part | Stage | Model | Content | Exit |
|---|---|---|---|---|
| S14a | A flame (2 chunks) | Opus | ONE layout commit (3.1, every key of 2, SAVE +1, PROTOCOL +1 with both commands refused); F1-F9; snapshot `flame`, `patches`, burning flag; tests | `check.sh quick`; T185 |
| | Checker | Sonnet | diff vs this file, named tests, neutral check | list of misses |
| | B clear + data (2 chunks) | Opus (data part may be Sonnet) | C1-C5; weapons, cartridges, `when` pairs, derived `flame`; rows R1-R4, R6; resolved-defs baseline (`BLESS=1`) | R1, R2 |
| | C melee (1 chunk) | Opus | M1-M5, `Strike`, melee blocks, R5 | R5; `s14_off` 40/40 |
| | Gate | script | `flame_only` 480 (`tools/measure.sh`) | section 10 gate |
| | P presenter (1 chunk) | Sonnet | stream, patches, burning men, strike, Clear trench button, `V`; headless mock smoke | |
| S14-ai | D AI (2 chunks) | Opus | A1-A4, liveness, JSON `s14`, `S14.toml` T189-T200, switches on, 40 sets | `check.sh tests` |
| | Measure, Review, Fix | script, Opus, Opus | 480 + ablations; review gate fires (sim, save, protocol) | T189-T193 |
| | Docs | Haiku | logs, STATE (versions line), TODO S14 block, `workflows.md` S14 size, modding guide `docs/modding/flame.md` | |

## 12. Design questions: all settled on (A) (2026-11-22; the first affected part in brackets; S14-1..7 are in `docs/TODO.md`)
- **S14-1 Fire and own men [S14a].** (A) The stream passes own men like every direct round; burning ground burns everyone in it, own
  men included (fire does not know sides; own burns count as own fire, not kills). (B) Patches harm enemies only. (C) The stream hits
  own men too.
- **S14-2 Fuel-tank hits [S14a].** (A) No rupture: a hit on the operator is an ordinary hit (sources: rare in fact, a film myth; no
  new RNG). (B) A rear torso hit ruptures the tank at 10 %: he burns and leaves a 2 m patch.
- **S14-3 Where the flamethrowers go [S14a].** (A) Into the Sturmpioniergruppe and the Assault Section as the roster says, one man's
  weapon swapped; size, price, cooldown and weights unchanged. (B) A separate two-man flamethrower team card per side. (C) Both.
- **S14-4 Melee scope [S14a].** (A) Automatic strike only when the weapon is not ready, plus `Strike` on `V`; butt stroke and spade,
  no bayonets. (B) Also a `FixBayonets` order (1.8 m reach, an aim penalty while fixed). (C) Automatic only, no command.
- **S14-5 Who may clear trenches [S14a].** (A) Any infantry squad with grenades, players and AI. (B) The `assault`-tagged cards only (Sturmpioniergruppe, Assault Section).
- **S14-6 Flame on vehicles and works [S14a].** (A) Men only: particles stop on vehicles, pieces, crates and wire with no effect (crews
  behind plates, and open-top crews too, are safe in S14). (B) Open-top crews and exposed seats burn (the S7 open-face path).
- **S14-7 Burning ground and paths [S14a].** (A) Paths ignore fire; men caught in it step out (F6), and `ClearTrench` waits for its
  own fire. (B) A path cost around live patches (a new nav layer; tick cost).
Settled in the plan only (data and scope, owner rule 2026-11-11): the US weapon is the M1A1 for the 1944-06 card (4.1); fuel
11.8 L / 17.8 L and the particle figures of 4.1; card prices unchanged; strike numbers of 4.3; the T185 numbering; the AI weights.

## 13. Risks
1. **Flame too strong in trenches** (owner rule S4-43: trenches stay strong vs small arms; flame is not small arms, and the design
   wants it to crack trenches). The `flame_only` gate reads the swing before any AI uses it on purpose; levers in section 10.
2. **Lethality and length.** Most flame casualties die (downed men keep burning). `length_min` is in the gate; `burn_s` the first lever.
3. **Patches as area denial.** A defender can burn his own trench mouth; S14-1 (A) makes that cost him too. T200 counts patches.
4. **Cost.** Particles and patches are serial; `flame_bench` and verify's p99. Fallback: `particles_per_player` 32.
5. **S13 moves under us.** S13a takes the next SAVE, appends cards and touches `[ai.armour]`; S14 touches `combat.rs`, `items.rs`,
   `ai/`: re-read the append-sensitive tests and the baselines (`resolved_defs.rs`, JSON snapshots) after the rebase. S13 renumbered its
   targets to T174-T184 (S12 uses T159-T173); S14 starts at T185.
6. **HE share (T8, S12-16 known miss).** Flame takes trench kills from HE in the AI half; T196 shows by how much; not a lever here.
7. **Crew-weapon pickup of fuel.** The flamethrower must pass to the assistant with the operator's fuel units; whether the S4
   crew-weapon pickup carries the feed's rounds is not audited: Build A reads it and tests `flame_handover_keeps_fuel`.

## 14. Sources
`S14-research-history.md`: Wikipedia "Flammenwerfer 41", "M1 flamethrower", "M2 flamethrower"; dday-overlord.com FmW 41 and M2 pages;
US *Intelligence Bulletin*, April 1944, "Portable Flame Throwers (German)" (lonesentry.com); TM 3-376A (to read in the data stage);
`infantry-structures.md` 4.2-4.3 for reach, timings and the particle model.
