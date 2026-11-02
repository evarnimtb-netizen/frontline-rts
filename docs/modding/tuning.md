# Tuning the game

Every gameplay number lives in `mods/ww2` TOML. Edit a value, then test it
in one command (CLI) or with F5 (Godot debug build). No Rust build is
needed.

## Where values live

| What | File | Table |
|---|---|---|
| Economy, tickets, capture | `defs/rules/skirmish.toml` | `[economy]`, `[tickets_drain]`, `[capture]` |
| Walking, paths, formations, slopes | same | `[movement]`, `[movement.formation]` |
| Trench and AT-ditch shapes, digging; HE blowing parapet spoil away (S3b `spoil_crater_pct`) | same | `[dig.profiles.*]`, `[dig.common]` |
| Spotting, accuracy, suppression thresholds | same | `[combat]` |
| Hit capsules, posture points | same | `[combat.body]` |
| Mortar laying and flight | same | `[combat.mortar]` |
| HE bursts and craters | same | `[combat.blast]` |
| Suppression details; S3b duck state of men in trenches (`duck_points`, `duck_release_points`, `duck_decay_pct`) | same | `[combat.suppression]` |
| S3b Suppress order (area fire): aim height, line length, line-of-fire probe, a tank's HE interval, reserve and aim | same | `[combat.suppress]` |
| Cover on arrival | same | `[cover]` |
| Thrown items | same | `[items]`, `[items.throw]` |
| Vehicle rules: unlock, fire, rack, repair | same | `[vehicles]` |
| Ammo fire, rack outcome (S3b) | same | `[vehicles.damage]` |
| S3c bail-out: triggers (`ammo_fire_bail`, `engine_fire_bail_s`, `crew_below` (replaces S3b's `crew_ko_below`), `useless_bail`, `under_fire_s`), exits (`wounded_hp`, `wounded_exit_pct`, `exit_clear_cm`, `dismount_ms`, `bail_suppression`), `abandoned_cap`, `crew_dismiss_s` | same | `[vehicles.bail]` |
| S3c re-crewing an abandoned vehicle: `reach_m`, `board_s`, `min_men`, `any_infantry`, `capture`, untrained penalties (`untrained_reload_pct`, `untrained_traverse_pct`, `untrained_spread_pct`) | same | `[vehicles.recrew]` |
| S3c a vehicle's crew squad and hatches (`crew_squad`; `[[hatches]]` `seats`, `frame`, `pos` cm, `exit_ms`); the crew squad defs (role `crew`, never in a roster) and their pistols (`class = "pistol"`) | `defs/vehicles/`, `defs/units/*_tank_crew.toml`, `defs/weapons/p38.toml`, `m1911a1.toml` | the def's own file |
| Driving, yielding, ground pose | same | `[vehicles.driving]` |
| Vehicle path costs | same | `[vehicles.path]` |
| Escort file behind a vehicle | same | `[vehicles.escort]` |
| Coax cone, spall reach; S3b: HE group radius, lead estimation error, when Auto loads APCR | same | `[vehicles.gunnery]` |
| AI difficulty | same | `[difficulty.*]` |
| AI commander (S3b: `tank_reserve_mp` in `[ai.armour]`; fire-and-move, AT and satchel hunts in `[ai.assault]`) | same | `[ai]`, `[ai.infantry]`, `[ai.armour]`, `[ai.assault]` |
| S3c AI armour habits: rearm trips (`rearm_ap_below`, `rearm_full_pct`, `rearm_max_s`), crews re-crewing (`recrew_clear_m`, `recrew_reach_m`), denial fire at enemy abandoned tanks (`deny_m`) | same | `[ai.armour]` |
| S3c sticky satchel hunts (`satchel_stick_s`, `satchel_chase_s`, `satchel_leash_m`, `satchel_crawl_m`, `satchel_hunt_max_s`), the second base of fire (`second_base_below_pct`, `second_base_gap_m`), AT stalks of stuck tanks and following own tanks (`at_hunt_stuck_m`, `at_follow_m`, `at_follow_back_m`) | same | `[ai.assault]` |
| S3c fragments over a falling edge (a burst on a parapet crest reaches the trench behind it): `frag_fall_probe_cm` | same | `[combat.blast]` |
| S3c point-symmetric terrain (the base-0 half mirrored through the centre; woods, points and bases are placed by hand) | `defs/maps/*.toml` | `[terrain] symmetry = "point"` (default `"none"`) |
| How steady a squad type is under fire (S3b `steadiness_pct`, scales the duck and pin thresholds; the veterancy hook) | `defs/units/*.toml` | `steadiness_pct` (default 100) |
| Where bought squads spawn in the base (S3b) | same | `[call_in]` |
| One unit, weapon, item, vehicle, round | `defs/units/`, `defs/weapons/`, `defs/items/`, `defs/vehicles/`, `defs/ammo/` | the def's own file |
| HE fragments and overpressure (S3b) | `defs/weapons/` (mortars), `defs/ammo/` (tank HE, rockets), `defs/items/` (grenades, satchels) | `[he]`: `blast_radius_cm`, `frag_count`, `frag_reach_cm`, `frag_damage`, `frag_far_pct`, `frag_elev_deg`, `frag_track_damage`; plates beaten: `inside_rays`, `inside_damage`, `inside_reach_cm` |
| APHE filler, barrel, crew and component boxes (S3b) | `defs/ammo/` `[filler]` (`rays`, `cone_deg`, `damage`, `reach_cm`); `defs/vehicles/` `barrel` on the main gun, `[crew.boxes]`, `[[components]]` `box` or `boxes`. A hull-frame box must lie inside the hull box; a turret-frame box inside the turret box or below it down to the hull floor (the turret basket, S3b Tune) | the def's own file |
| Vehicle MG stowage (S3b Fix): rounds per coax / hull MG, refilled a belt per base rearm step | `defs/vehicles/` | `rounds` on a `coax` or `hull` `[[weapons]]` entry (omit: no limit) |
| Call-in cost and re-call cooldown (S3b) | `defs/units/*.toml` (squads), `defs/vehicles/*.toml` (tanks) | `cost`, `recall_cooldown_s`, `initial_cooldown_s`; optional `cooldown_group` in a unit def (types in one group share a timer) |

Each key has a comment with its unit and meaning. Units are in the key's
name: `_mm`, `_cm`, `_m`, `_s`, `_ms`, `_pct`, `_deg`, `_mm_s`. Values are
whole numbers. A value outside its range fails to load. The error names the
file, the key, the allowed range and a fix, for example:

```
defs/rules/skirmish.toml: combat.mortar.flight_milli: must be in 1..=5000, got 0 (fix: [combat.mortar] flight_milli = 1)
```

Check a mod without running anything: `frontline-cli validate mods/ww2`.

Engine limits (pool sizes, search sizes, substep rate) stay in code.
`docs/slices/S3b-balance.md` "Tuning surface" lists them.

## Try a change without editing the mod

Write an override file with only the keys you change. It has no `id`,
`name` or `base`. `mods/ww2/tests/overrides/example.toml` shows the format.

```toml
[combat]
near_miss_cm = 250            # top-level tables patch the rules, key by key

[defs.vehicles."ww2:ger_pz4_h"]
cost = 620                    # [defs.<kind>."<id>"] patches one def
```

The def kinds are `units`, `weapons`, `items`, `factions`, `maps`,
`vehicles` and `ammo`. An array, such as a vehicle's `[[weapons]]`, is
replaced whole.

Pass the file with `--rules-override FILE` to `skirmish`, `armour-test` or
`fortify-test`. The flag can be given more than once; files apply in order.
The run prints every value the files change to stderr, as
`old -> new`. An override changes the match, so its hashes differ from the
base run. `simulate` refuses overrides, because bench scenarios have no rules.

## A/B test in one command

```sh
B=mods/ww2/tests/overrides/my_change.toml
frontline-cli skirmish --ai-vs-ai --seeds 10 --both-bases --ab --rules-override $B
```

- `--seeds N` plays seeds `--seed .. --seed+N`. Matches run in parallel; set
  the number with `--jobs`. Results and hashes do not depend on it, but the
  tick-time p99 does (the same 160 matches: 0.71 ms peak at `--jobs 3`,
  1.55 ms at `--jobs 6` on an 8-core Mac): quote T10 from a `--jobs 3` run.
- `--both-bases` plays each seed twice, once with the bases swapped.
- `--summary` prints one table instead of every report: wins per faction and
  per base, match length, tanks lost, knock-out causes, HE share, rockets,
  satchels and tick p99, plus one line per match. S3c adds report-only
  rows (`sdk/crates/cli/src/s3c_tally.rs`): tank base returns and how
  long MGs were dry or HE at the reserve, satchel hunts and how they
  ended, crew alive at each knock-out, and per base side when each point
  was first taken and how long it was held. `--report json` also has
  each knock-out's position and its shooter's (`s3c.ko_spots`). S3c
  Part A: the "tank losses" line counts knock-outs (`ko_*`) and
  abandonments (`abandoned_<trigger>`); an `S3c <faction>` line splits
  tanks lost into destroyed and abandoned, says what became of the
  abandoned (re-crewed, captured, destroyed later, scrapped, standing) and
  counts crewmen alive at bail-outs, out and dismissed in base; an `S3c
  crews` line gives the abandonments with crew inside where at least one
  man got out, and the men out still alive 60 s later. Rockets are
  credited for an abandonment by the last hit, as for a knock-out. S3c
  Part B: an `S3c AI <faction>` line sums the AI's rearm trips, satchel
  hunts kept through a loss of sight, satchels thrown by hunters, hunt
  ends by reason (`gone`, `unseen`, `fast`, `leash`, `timeout`, `spent`,
  `retreat`), second bases of fire, AT stalks of stuck tanks and follows,
  re-crew orders, crews sent back and denial orders.
- `--ab` plays the same matches without the overrides too. It prints both
  tables and the change in each headline number.

On an 8-core Mac, 10 seeds × both bases (20 matches) take about 20 s, and
with `--ab` (40 matches) about 40 s. `--report json` gives the same data as
JSON.

For the staged checks, run the same override through
`frontline-cli armour-test --rules-override $B` (duels, AT teams, trench
assault; the test rules are derived after the override) and
`frontline-cli fortify-test --rules-override $B`.

## Reload in Godot

Every new match reads `mods/` from disk, so edits apply at the next start.
In a debug build (the editor or a debug export) you can also reload inside a
match:

- press **F5**, or choose **Reload data & restart** in the pause menu (Esc);
- the match restarts with the same faction, difficulty, map and seed;
- if the data does not load, a red banner shows the loader's message and
  the running match goes on. Fix the file and press F5 again.

Release builds do not offer the reload. They play the data shipped with the
game. Net matches do not offer it either.

## Multiplayer

The lobby compares the content hash of both players' data. A player whose
TOML differs, even by one value, is refused with "The two games have
different mod content". The game reads the mod again every time a player
hosts or joins.
