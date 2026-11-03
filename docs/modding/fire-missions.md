# Mortar fire missions (S5b)

A mortar squad fires a **fire mission**: a target point and a number of
rounds (the `FireMission` command; it replaced the old `Barrage`). The sim
decides how each round is aimed. Values live in `defs/rules/skirmish.toml`
`[fire_mission]` and in each mortar's `defs/weapons/*.toml` `[indirect]`;
every key has a comment with its unit. Design: `docs/design/roster/
artillery-on-map.md` §4.1, §4.5, §4.8; spec: `docs/slices/S5b-mortars.md`.

## Fire modes

The mode is decided again for every round, when it is fired:

| Mode | When | Aim error at the start | Ranging |
|---|---|---|---|
| **Direct** | the mortar squad's own leader (with the squad, within `observer_link_m` of the gunner) sees the point | `first_round_error_pct` of the range, at least `first_round_error_min_m` | yes |
| **Observed** | another observer of the same player sees it and stands within `observer_link_m` of the gunner (a squad leader of any own squad, an `observer` man, or the mortar's own leader walked out as a spotter) | the same | yes |
| **Map fire** | nobody linked sees it, but the player knows the point (below) | `map_fire_error_pct` of the range, at least `map_fire_error_min_m` | never |

- **Observers.** Every squad's leader observes ground points up to
  `observe_leader_m` (the man with the `leader` trait; when he falls, the
  man leading the formation). A man with the `observer` trait observes up
  to `observe_observer_m` (FO teams; none in ww2 yet). Observing reveals
  nothing: enemy men are still spotted with the squad's `sight_m`.
- **The link.** Only voice in S5b: an observer within `observer_link_m` of
  the gunner. Radio and telephone links come later (S6 HQ, S11 FO teams).
- **Sight lines** end `point_lift_mm` above the ground point (the undug
  ground over a trench or crater) and `burst_seen_lift_mm` above a burst
  (its dust and smoke). Terrain and smoke block them, as for spotting.
- **Upgrades and downgrades.** A map-fire mission whose point comes into a
  linked observer's view starts adjusting from its current error; an
  observed mission that loses its observer keeps firing on its last data
  as map fire.

## Aim error and ranging

Every round lands at the target plus the mission's **aim offset** (where
the group falls; drawn once, its length the start error, its bearing
random) plus the weapon's **precision spread** (`spread_mrad`, triangular
per axis, wider under suppression). Each burst a linked observer **sees**
leaves `ranging_step_pct` of the offset (bracketing: "split the bracket").
A burst hidden by smoke or ground teaches nothing.

**Phases.** Laying takes `fire_direction_s` (Observed, Map) or `direct_s`
(Direct). An observed or direct mission then fires single **adjusting
rounds**: each waits for its own burst plus `adjust_wait_s`. When an
observer has seen a round land and the offset is at most `ffe_error_m`, or
after `adjust_rounds_max` adjusting rounds, the rest are fired **for
effect** at the weapon's rate. Map fire goes straight to effect.

**Ranging memory.** A mortar squad remembers up to `ranging_max_cells`
target areas it ranged on. A new observed or direct mission within
`ranging_cell_m` of one starts from the remembered (smaller) offset and,
when that is within `ffe_error_m`, fires for effect at once. The memory
is forgotten after `ranging_expiry_s` or when the tube moves more than
`ranging_keep_move_m`. Map fire never uses it (S5b-4 (A)).

## Known ground: no blind fire

An order is refused as "unknown ground" unless the player knows the
point:

1. an own observer sees it now (any own squad's leader or `observer` man,
   linked or not; at most 8 sight lines, nearest first);
2. it lies within a capture point's area grown by `intel_snap_m`;
3. an enemy is seen now within `intel_snap_m` of it;
4. a cell of the player's **knowledge map** lies within `intel_snap_m`:
   every `intel_period_s` each enemy soldier or vehicle the player sees
   marks its `intel_cell_m` cell. A cell is forgotten `intel_keep_s` after
   the last sighting, unless an enemy was seen there in a trench
   (`intel_trench_keep_s`, 0 = the whole match).

Ground seen empty and trenches seen without men do not count (S5b-3 (A)).
Other refusals: not a mortar, rounds outside 1-50, off the map, inside
the minimum range, no charge reaches, no bombs. The HUD shows the reason.

## Charges and flight time (`[indirect]`)

```toml
[indirect]
charges_m_s = [75, 105, 130, 152, 174]   # muzzle velocity per charge, rising
elevation_deg = [45, 87]                 # laying limits, high angle only
```

The solver takes the **lowest charge** with a high-angle solution inside
the laying limits and flies the bomb drag-free (as every sim shell), so it
lands where it is aimed after the true time of flight (the GrW 34 on
charge I: about 15 s at 200 m, 14 s at 400 m; charge II beyond 560 m).
`validate` checks the table against the weapon: charges strictly rising
(20-600 m/s, at most 12), limits within 45-89 degrees, the lowest charge
at the top elevation reaching no farther than `min_range_m` + 10 %, the
highest at 45 degrees reaching `range_m`; each error names the fix. A
mortar without `[indirect]` keeps the old rule (`[combat.mortar]
flight_milli`, `min_flight_s`).

## Rules without `[fire_mission]`

Older mods fire a mission exactly as the old barrage: no observers, no aim
error, no knowledge rule, no ranging (S5b-1 (A)).

## The bench

`frontline-cli mortar-bench` fires the same mission observed (a spotter in
the voice link), as map fire, and observed with its bursts hidden by
smoke, at 200-500 m over many seeds and both mortars, and prints the mean
miss of each and their ratio. It passes when map fire misses 2.5-3.5x as
far as observed fire (pooled) and every run ranged as above. Try a change
with `--rules-override FILE`, as for `skirmish`:

```sh
frontline-cli mortar-bench --seeds 50 --rounds 8 --rules-override my.toml
```
