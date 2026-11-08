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
| Mortar laying and flight (S5b: `flight_milli` / `min_flight_s` only for a mortar without `[indirect]`) | same | `[combat.mortar]` |
| HE bursts and craters | same | `[combat.blast]` |
| Suppression details; S3b duck state of men in trenches (`duck_points`, `duck_release_points`, `duck_decay_pct`) | same | `[combat.suppression]` |
| S3b Suppress order (area fire): aim height, line length, line-of-fire probe, a tank's HE interval, reserve and aim | same | `[combat.suppress]` |
| Cover on arrival | same | `[cover]` |
| Thrown items | same | `[items]`, `[items.throw]` |
| Vehicle rules: unlock, fire, rack, repair | same | `[vehicles]` |
| Ammo fire, rack outcome (S3b) | same | `[vehicles.damage]` |
| S3c bail-out: triggers (`ammo_fire_bail`, `engine_fire_bail_s`, `crew_below` (replaces S3b's `crew_ko_below`), `useless_bail`, `under_fire_s`), exits (`wounded_hp`, `wounded_exit_pct`, `exit_clear_cm`, `dismount_ms`, `bail_suppression`), `abandoned_cap`, `crew_dismiss_s` | same | `[vehicles.bail]` |
| S3c re-crewing an abandoned vehicle: `reach_m`, `board_s`, `min_men`, `any_infantry`, `capture`, untrained penalties (`untrained_reload_pct`, `untrained_traverse_pct`, `untrained_spread_pct`) | same | `[vehicles.recrew]` |
| S8b AI recon cars (`after_s`, `second_after_s`, `income_max_pct`, `view_back_m`, `at_threat_m`, `backoff_m`) and supply trucks (`min_vehicles`, `hq_wait_s`, `income_max_pct`, `load_shells_kg`, `load_wait_s`, `min_load_pct`, `follow_m`, `los_rays`, `return_below_pct`, `flee_m`, `arrive_m`, `max_fails`, `repath_s`) | `defs/rules/skirmish.toml` | `[ai.recon]`, `[ai.truck]` (absent: starting values) |
| S8b rides that pay: `pick_m`, `meet_ahead_m`, `meet_stand_m`, `ride_lock_s`, `drop_short_m`, `unload_rolling`, `counted_ride_m` | same | `[ai.transport]` |
| S8a infantry carriers: `unlock_s` (transport unlock floor, never later than `[vehicles] unlock_s`), `stun_ms`, `board_reach_cm`, `move_block_cm_s` | same | `[vehicles.transport]` (absent in older rules: 120, 2000, 300, 100) |
| S3c a vehicle's crew squad and hatches (`crew_squad`; `[[hatches]]` `seats`, `frame`, `pos` cm, `exit_ms`); the crew squad defs (role `crew`, never in a roster) and their pistols (`class = "pistol"`) | `defs/vehicles/`, `defs/units/*_tank_crew.toml`, `defs/weapons/p38.toml`, `m1911a1.toml` | the def's own file |
| Driving, yielding, ground pose | same | `[vehicles.driving]` |
| Vehicle path costs | same | `[vehicles.path]` |
| Escort file behind a vehicle | same | `[vehicles.escort]` |
| Coax cone, spall reach; S3b: HE group radius, lead estimation error, when Auto loads APCR; S8a: `face_hardened_pct`, `small_arms_ricochet_deg` | same | `[vehicles.gunnery]` |
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
| APHE filler, barrel, crew and component boxes (S3b) | `defs/ammo/` `[filler]` (`rays`, `cone_deg`, `damage`, `reach_cm`); `defs/vehicles/` `barrel` on the main gun, `[[seats]]` `box` (S7; was `[crew.boxes]`), `[[components]]` `box` or `boxes`. A hull-frame box must lie inside the hull-frame armour boxes (S7 `[[boxes]]`); a turret-frame box inside the turret box or below it down to the hull floor (the turret basket, S3b Tune); either may reach 50 cm above an open-topped box (S7) | the def's own file |
| Vehicle MG stowage (S3b Fix): rounds per coax / hull MG, refilled a belt per base rearm step | `defs/vehicles/` | `rounds` on a `coax` or `hull` `[[weapons]]` entry (omit: no limit) |
| Call-in cost and re-call cooldown (S3b) | `defs/units/*.toml` (squads), `defs/vehicles/*.toml` (tanks) | `cost`, `recall_cooldown_s`, `initial_cooldown_s`; optional `cooldown_group` in a unit def (types in one group share a timer) |
| S4 card block: `cooldown_s` / `unlock_s` (the new names of `recall_cooldown_s` / `initial_cooldown_s`), `max_alive`, `limit_group`, `available_from` / `available_to`, `tags`, `capabilities`, `training`, `ai_weight`, `short_name`, `sources`; men by `[[members]]` slots; `extends` (a variant gives only what differs) | `defs/units/*.toml`; `docs/modding/variants.md` | top-level keys, `[[members]]` |
| S4 training profiles (aim, reload, steadiness, spotting, build rate, as % of a regular soldier) | `defs/training/*.toml` | the def's own file |
| S4 match date for card dates, default training, named unit limits, the AI mix by tag | `defs/rules/skirmish.toml` | `[call_in] date`, `[training] default`, `[limit_groups]`, `[ai.mix]` (or the old `mix_*_pct` keys of `[ai]`) |
| S4 pop cap 150 (historical squad sizes, owner Q6 (A)) | `defs/rules/skirmish.toml` | top level `pop_cap` |
| S4 Panzerfaust gap: one single-shot AT item per squad this often (`single_shot_gap_s`) | same | `[items]` |
| S4 crew-weapon pickup reach: only a man this close (m) takes a fallen gunner's MG or launcher (`crew_pickup_reach_m`) | same | `[items]` (absent in older rules: 30) |
| S4 AI Panzerfaust hunts (`pzf_hunt_m`, 0 = off; `pzf_clear_m`) | same | `[ai.assault]` (absent in older rules: 100 / 40) |
| S4 AI capture of enemy abandoned tanks by its crews (`capture_m`, 0 = off; needs `[vehicles.recrew] capture`) | same | `[ai.armour]` (absent in older rules: 150) |
| S4 weapons: crew weapon flag (`crew_weapon`, default by class), hand-over time of every crew weapon (`handover_ms`), squad-card name (`short_name`), `sources` | `defs/weapons/*.toml` | top-level keys |
| S4 Panzerfaust: aim time (`windup_ms`), arming distance (`min_range_m`), the launcher def it fires as (`weapon`) | `defs/items/panzerfaust60.toml`; the shot itself `defs/weapons/pzf60.toml`, the round `defs/ammo/pzf60_heat.toml` | top-level keys |
| S5a ammunition prices per tonne by supply kind (`small_arms_mp_per_t`, `explosives_mp_per_t`, `shell_mp_per_t`), the rack factor (`rack_refill_max_card_pct`, `rack_rearm_price_pct`), rare rounds (`rare_cap_max`, `rare_rearm_base_s`), base refill rates (`base_small_arms_rounds_s`, `base_bomb_s`, `base_bomb_after_fire_s`), the HUD low marker (`low_pct`); see `docs/modding/ammo.md` | `defs/rules/skirmish.toml` | `[supply]` (absent: unlimited rounds, free refill) |
| S5a sharing inside a squad (`interval_s`, `reach_m`, `below_pct`, `giver_above_pct`, `share_pct`) and the crew feed of a dry MG or mortar (`crew_feed_m`) | same | `[combat.share]` (absent: the S5a defaults) |
| S5a counted rounds: a cartridge's weight and supply kind | `defs/cartridges/*.toml` | `supply = { kind, g }` |
| S5a a weapon's feed (`feed`), a man's default load (`load`), bursts (`burst_rounds` shots, then `burst_pause_ms` on top of the interval; 0 = none), its own fire-discipline share of its range (`hold_beyond_pct`, 10-100; 0 or absent: the rules') | `defs/weapons/*.toml` | top-level keys |
| S5a fire discipline: a man in the default mode opens fire within `hold_beyond_pct` of his weapon's range; in `hold` only within `hold_open_m`; a held squad springs at `hold_spring_points` | `defs/rules/skirmish.toml` | `[combat.fire]` (absent: 100 / 30 / 30, the pre-S5a behaviour, S5a-29) |
| S5a AI ammunition: refill trips (`return_below_pct`, `dry_pct`, `max_returning_pct`, `refill_full_pct`, `refill_max_s`), mortar reserve (`mortar_reserve_pct`), buys keep `float_mp` back while a unit waits for a paid refill, a paused refill ends after `rearm_stall_s` | same | `[ai.ammo]` (absent: the S5a defaults) |
| S5b fire missions: who observes and how far (`observe_leader_m`, `observe_observer_m`), the voice link (`observer_link_m`), sight-line lifts (`point_lift_mm`, `burst_seen_lift_mm`), laying and adjusting times (`fire_direction_s`, `direct_s`, `adjust_wait_s`), when to fire for effect (`ffe_error_m`, `adjust_rounds_max`), the aim error (`first_round_error_pct` / `_min_m`, `ranging_step_pct`, `map_fire_error_pct` / `_min_m`), the ranging memory (`ranging_cell_m`, `ranging_expiry_s`, `ranging_keep_move_m`, `ranging_max_cells`), the knowledge map for map fire (`intel_cell_m`, `intel_keep_s`, `intel_trench_keep_s`, `intel_period_s`, `intel_snap_m`), HUD mission rounds (`player_rounds`); see `docs/modding/fire-missions.md` | `defs/rules/skirmish.toml` | `[fire_mission]` (absent: missions run as the old barrage, S5b-1) |
| S5b AI fire missions: rounds per mission (`mission_rounds`, was `ai.barrage_rounds`), spotters and map fire (`spot_samples`, `spot_max_s`, `spot_hold_s`, `map_fire_min_pct`, `map_fire_min_score`, S6b `map_fire_max_miss_m`; see `docs/modding/fire-missions.md` "The AI"); the mortar's support spot `[ai.infantry] support_back_m` (S5b: 120) | same | `[ai.mortar]` (absent: `ai.barrage_rounds` or 6) |
| S5b a mortar's charges and laying limits (`charges_m_s`, `elevation_deg`): the solver takes the lowest charge with a high-angle solution | `defs/weapons/*.toml` | `[indirect]` (absent: the S1 flight rule) |
| S5b the `observer` member trait | `defs/units/*.toml` | `traits` in `[[members]]` |
| S5a AI Panzerfaust ambush hold (`ambush_watch_m`, 0 = off; `ambush_offset_m`, `ambush_max_s`, `ambush_release_m`) | same | `[ai.assault]` (absent in older rules: off, 25, 90, 0 = the watch) |
| S5a rounds per man by cartridge id | `defs/units/*.toml` | `rounds = { "<id>" = N }` in `[[members]]` |
| S5a weights of items and rounds (`supply`), rare rounds (`rare`), a vehicle's rare-round cap (`rare_cap`) and stowage (`ammo` counts) | `defs/items/`, `defs/ammo/`, `defs/vehicles/` | top-level keys |

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
`vehicles`, `ammo` and (S4) `training`. An array, such as a vehicle's
`[[weapons]]`, is replaced whole. S4: the patch applies to the def's file
as written and every def then resolves again, so a patch on a parent also
changes each variant that `extends` it (each change is listed); a
`[[defs.units."<id>".members]]` patch replaces a legacy squad's `men` /
`weapon` keys (`docs/modding/variants.md`).

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

## Results as JSON, targets and checks (read numbers, not text)

Agents and tools should read these files, not the text tables.

**`--json FILE`.** `skirmish` (one match, `--seeds`, `--ab`), `armour-test`
and `fortify-test` write one JSON object with every number their text output
prints; the normal output is unchanged. (A bare `--json` on `armour-test` and
`fortify-test` still prints the old JSON report instead of the text.)

```sh
frontline-cli skirmish --ai-vs-ai --seeds 20 --both-bases --minutes 45 --jobs 6 --summary --json /tmp/s40.json
frontline-cli armour-test --seeds 5 --json /tmp/arm5.json
frontline-cli fortify-test --seeds 10 --json /tmp/fort10.json
```

- Skirmish file (`"kind": "skirmish"`): `matches`, `per_match[]` (`seed`,
  `side` normal or swapped, `winner_faction`, `winner_base`, `length_s`,
  `he_share_pct`, `hash`, tanks lost, rockets and satchels per player),
  `wins` (`germany`, `germany_pct`, `usa`, `usa_pct`, `base0`, `base0_pct`,
  `base1`, `base1_pct`, `base_max_pct`, `no_winner`), `length_min` (`mean`,
  `min`, `median`, `max`, `over_25`), `he_share_pct` (`mean`, `max`),
  `tick_p99_ms` (`peak`, `all`; only meaningful from a `--jobs 3` run
  alone), `tank_losses`, `factions.<name>` (the per-faction tables: kills,
  tanks, `ammo.fired`, `ammo.spend_mp` per kind, `kills_by_source_pct`,
  `s4`, `s5a`, `s5b` and `s5b_derived` such as `ai_observed_direct_pct`,
  `refused_total`, `bombs_total`) and `s3c`. `--ab` writes `{a, b}`.
- armour-test file: `cases[]` (`id` `case9`, `name`, `pass`), each with
  `checks[]` (`id`, `name`, `value`, `limit`, `pass`, `detail`; `value` and
  `limit` are null for checks that are a yes/no on several runs) and
  `rows[]` (the report rows with their `target` text). fortify-test file:
  `checks[]` (rifles, mortar, smoke) and `rows[]` (ranged mortar, grenade
  variants, MG suppression totals).

**`frontline-cli targets FILE.toml [--from-json DIR] [--jobs N] [--runs a,b]
[--out DIR]`.** A targets file lists the runs that feed it and the targets
(`docs/slices/targets/S5b.toml` is the model):

```toml
[runs.skirmish40]          # kind: skirmish | armour-test | fortify-test
kind = "skirmish"
seeds = 20                 # first_seed (default 1), both_bases, minutes, jobs
both_bases = true
minutes = 45

[[target]]
id = "T5a"
metric = "skirmish40.wins.germany_pct"   # <run>.<path into that run's JSON>
min = 35                                  # min and/or max, inclusive
max = 65
kind = "hard"              # hard (default) or report
```

A path walks objects by key and arrays by index or by an element's `id` or
`name` (`armour5.cases.case9.checks.trench_casualties_40_of_open.value`).
The command plays the runs the targets name (in this process, same
results as the CLI; JSON saved to `target/targets/<file>/<run>.json`), or
with `--from-json DIR` reads `DIR/<run>.json`. It prints one line per target
and a count:

```
T5a skirmish40.wins.germany_pct 40 [35,65] PASS
T4a skirmish40.factions.germany.s5b_derived.refused_total 1 [-,0] MISS
T14a timing40.tick_p99_ms.peak - [-,1.5] SKIP (run timing40 not played)
12 pass, 1 miss (hard: 1), 11 skipped
```

`--runs` limits which runs are played (the others print SKIP); `--jobs`
sets matches at once (default 6, the balance-batch setting). A run's own
`jobs = 3` overrides it: put tick-time targets on a separate run with
`jobs = 3` and play it alone. Exit code 1 when a hard target misses
(including a path that does not exist); report targets only print MISS.

**`tools/check.sh quick [BASE] | full`.** Few lines instead of cargo's and
Godot's output; full logs in `target/check/*.log`.

- `quick` runs `cargo fmt --check`, clippy `-D warnings` and `cargo test -q`
  for the crates changed against `BASE` (default: `$CHECK_BASE`, else the
  merge-base with `origin/<branch>`, else with `main`; a slice passes its
  base commit). It prints `tests (cli): ok (29 passed, 0 failed)` and, for a
  failing test, its first 40 lines.
- `full` adds the whole workspace's tests including the ignored ones
  (`--release`), golden ballistics, `validate mods/ww2`, the bridge build,
  the four Godot headless tests (`check_project`, `test_match_view`,
  `test_net_ui`, `test_data_reload`) and a check that Godot's
  `settings.cfg` still equals the saved copy (`$SETTINGS_BAK`). One line each:
  `name: ok` or `name: FAIL (see target/check/name.log)`.
- Both exit non-zero on any failure. `$GODOT` overrides the Godot binary.

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
