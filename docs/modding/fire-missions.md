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

Long flights are allowed: a heavy mortar or a gun table may fly a minute
or more. A save taken while an adjusting round is in the air checks the
mission's burst deadline against the weapon's own bound (`2 v / g` of the
highest charge + 20 s for a target below the muzzle + 4 s, at least 60 s),
so long-flight tables load (S5b review L1).

## Guns (`[indirect] low_angle`, S11a1)

A served gun (role `inf_gun` or `at_gun`, `crew_model = "served"`) whose
main weapon has an `[indirect]` table takes a fire mission on its own
squad (`[fire_mission] gun_missions`). A gun's table sets `low_angle`:

```toml
[indirect]
charges_m_s = [92, 120, 150, 180, 210]   # rising
elevation_deg = [0, 73]                  # the band may start below 45 deg
low_angle = true                         # try each charge's high root, then its low root
range_m = 3550                           # indirect range (default: the weapon's range_m)
spread_mrad = 8                          # mission round scatter (default: the weapon's spread)
```

Candidates run lowest charge first, the high root before the low root,
each inside the band; with `[fire_mission] mask` 1 a candidate counts only
if its arc clears the ground at steps 2..=16 of 20 (0.5 m above it) and its
elevation, taken onto the hull's pitch where the gun stands, lies inside
the mount's own elevation limits. None in the band: `cannot_reach` (or
`inside_minimum`); none clear: `masked`. A mortar's table (no `low_angle`)
solves exactly as before. Refusal order for a gun: `off_map`,
`not_mortar`, `rounds`, `not_ready` (not emplaced, or gunner or loader
slot unmanned), `no_bombs` (no HE round in the rack), `unknown_ground`,
`inside_minimum` / `cannot_reach`, `masked`. A gun's mission uses the
`gun_*` timing and error keys; an infantry gun counts in the player's
`S11Counters`, never in the mortar counters. Radio (`radio_link`): an
observer whose squad has a living `radio` man near him is linked when the
firing unit also has a living `radio` man within `observer_link_m` of the
tube (a gun: its crew squad). The gun's weapon is a `cannon`; with a
low-angle table its main mount may elevate to 89 deg (60 otherwise). The
mission ends (order Idle, ranging memory kept) when its rounds or its HE
are spent, no solution is left, the gun is packed, pushed, hitched,
abandoned or captured, its crew is ordered off, or a seen enemy comes
within `gun_break_m` (direct fire resumes). With `[supply]
arty_base_rearm` 0 an `inf_gun` gets nothing in its base zone.

Checks (S11a1 Build B): the main mount may name up to +89 deg in the
vehicle file; above +60 the cross-check wants a weapon with a `low_angle`
table (one rule, `vehicle::low_angle_top`, in the def check and the setup
check). A gun order past the table's `range_m` is `cannot_reach`. A gun
squad's refusals are not added to the mortar row `fm.refused` (the event
and `S11Counters::masked` / `not_ready` still count). The `radio` trait
is allowed on any member; `leader` stays first-slot only, so a gun crew's
radio man is its commander slot without `leader`.

The ww2 guns and observers (S11a1; `artillery-on-map.md` 2.1, 2.2, 4.7):

| Card | Gun | Crew | Elevation | Charges (m/s) | Range | Rate | Rounds | Cost / pop / cooldown / unlock |
|---|---|---|---|---|---|---|---|---|
| `ger_leig18_squad` | 7.5 cm leIG 18, 400 kg | 5 | -10..+73 | 92-210 | 3,550 m | 8/min | 12 Igr 18 | 320 / 2 / 90 s / 3:00 |
| `usa_105mm_m3_squad` | 105 mm M3, 1,130 kg | 6 | -9..+30 | 205-311 | 7,600 m | 3/min | 10 M1 HE | 380 / 2 / 120 s / 4:00 |
| `ger_vb_trupp` | observer (leader; Funker `radio`) | 2 | | | | | | 80 / 2 / 60 s / 0:00 |
| `usa_fo_party` | observer (officer; radio operator `radio`) | 3 | | | | | | 120 / 3 / 60 s / 0:00 |

S11b adds (card tags `["vehicle", "field_howitzer"]` and `["mortar",
"light_mortar"]`; `ai_weight` 5 on the howitzers, the S11a1 gun cards'
value, and 50 on the light mortars, half the heavy card's default 100):

| Card | Gun | Crew | Elevation | Charges (m/s) | Range | Rate | Rounds | Cost / pop / cooldown / unlock |
|---|---|---|---|---|---|---|---|---|
| `ger_lefh18_squad` | 10.5 cm leFH 18, 1,985 kg | 6 | -5..+42 | 200, 325 | 10,700 m | 4/min | 12 FH Gr 38 | 480 / 2 / 180 s / 8:00 |
| `usa_105mm_m2a1_squad` | 105 mm M2A1, 2,260 kg | 8 | -5..+65 | 205-311, 333 | 11,300 m | 3/min | 12 M1 HE | 500 / 2 / 180 s / 8:00 |
| `ger_kz_grw42_team` | 8 cm kz. GrW 42 (Wgr 34 bomb) | 4 | 45..87 | 75, 105 | 60-275 m (game) | 15/min | 18 bombs | 181 / 4 / 45 s / 0:00 |
| `usa_60mm_mortar_squad` | 60 mm M2 (M49A2 bomb) | 5 | 45..85 | 70, 95, 117, 133 | 90-370 m (game) | 18/min | 30 bombs | 240 / 5 / 45 s / 0:00 |
| (no card) `usa_75mm_m1a1` | 75 mm pack howitzer, 653 kg, data only | 5 | -5..+45 | 294 | 8,800 m | 3/min | 10 M48 HE | - |

The two howitzers are base guns (no tow): 90 s to emplace, re-lay 25 s,
`max_alive` 1, and "no push" as 1 cm/s with `push_men` above the crew (the
setup check refuses a card whose speed is 0); a full crew still crawls at
crew / `push_men` of 1 cm/s (about 0.5 m a minute), and the AI never orders
one pushed or hitched.

**S11b AI** (`[ai.arty]`, `docs/modding/tuning.md`): with `how` 1 the AI
buys one field howitzer from `how_after_s` (8:00) inside its own
`how_income_pct` (15 %) of the income so far, keeping `how_reserve_mp`
(300) of its manpower less the pending call-ins; the infantry-gun buy and
its pot skip `field_howitzer` cards (the two spent sums are apart), and with
`buy_pending` 1 it too counts pending call-ins. A bought howitzer gets the
infantry gun's record (emplace, observed missions, reserve, budget, lift,
counter-fire). With `light` 1 (0 in ww2 since the S11b fix stage: the AI
buys no light mortar, a human player still can; owner question S11b-14)
the light mortar cards join the mortar slot of the buy mix (about one light team in three mortar buys at weight 50
against 100); the mortar habit drives them like the heavy tubes, their
`range_m` (275 / 370 m) being the AI's band, not a refusal. The light mortars' game
ranges scale the heavy tubes' 600 m by their real ranges (1,100 / 2,400 and
1,800 / 2,900). The four gun cards (leIG 18, M3, leFH 18, M2A1) share
`limit_group = "ww2:artillery"`, 2 alive a player (`[limit_groups]`).

Both S11a1 guns are pushed (no tow in S11a1), `max_alive` 2; the AI buys none
until the S11a1 AI stage (`ai_weight` 0). The M1 HE's blast, fragment
reach, suppression and crater are the 75 mm values times 1.46 (the cube
root of the filler ratio 2.2 / 0.7), 160 fragment rays.

## The AI (`[ai.mortar]`, S5b Part B)

Each plan, a mortar squad that is not on a mission (`crates/sim/src/ai/mortar.rs`):

1. **Targets**: seen enemy squads in range of the gunner, not within
   `ai.infantry.danger_close_m` of own men, scored `still men x 10 +
   moving men x 5 + men in trenches x 20` (a man still for 3 s; the bombs
   land 20-30 s after the order, so a group on the move counts half), at
   least 20; plus remembered trench cells (30, +10 per manned neighbour).
2. **Observed or direct now**: the sim's own mode check on the best three;
   the first an observer sees gets a mission of `mission_rounds` (never
   more than it holds) while it holds more than `ai.ammo.mortar_reserve_pct`
   of its bombs (or an own point is threatened).
3. **Spotter**: else its leader walks forward to the first of 16 spots on
   the line to a target (inside the voice link, not past danger close, not
   within 50 m of a seen enemy) with a sight line to it; at most
   `spot_samples` rays a plan. While he is out the squad waits; it fires
   through him once he sees a target. He comes back `spot_max_s` after he
   arrived with nothing seen (or `3 x spot_max_s` if he never arrives) and
   `spot_hold_s` after the last mission; a recalled spotter blocks a new
   one for `spot_max_s`.
4. **Map fire** only as a fallback: no spotter out, the best target scores
   at least `map_fire_min_score`, the squad holds more than
   `map_fire_min_pct` of its bombs, it is not walking, and (S6b) the map
   start error at that range is at most `map_fire_max_miss_m` (ww2: 20 m
   against a 27 m minimum, so the ww2 AI orders no map fire). S6b: an AI
   mission whose squad is dry when it comes due is withdrawn, not refused.
5. **Never blind**: a target is ordered only where the knowledge rule's
   lasting part holds (a capture point or an intel cell within
   `intel_snap_m`; an enemy seen only this instant may be gone a reaction
   delay later), else passed over; a squad with fewer than 2 bombs orders
   nothing. The sim refuses no AI mission.
6. Otherwise it moves to its support spot `ai.infantry.support_back_m`
   behind the front point (ww2: 120 m, so the leaders of the squads at the
   point are inside the voice link) and stays there while the front point
   is between `range_min + danger_close_m + 20` and `range_max - 200` m.

The planner casts at most 48 sight-line rays a plan over all its mortars
(a mode check counts 4, a spot one); squads take turns from a start that
rotates each plan, and one that finds the budget spent waits a plan.

The skirmish summary's `S5b <faction> AI:` line counts missions by the
mode the AI expected, spotters sent, missions through a spotter, spotter
time-outs and recalls, map fire held (reserve / walking), blind targets
passed over and waits for the ray budget.

### Own men and counter-fire (`[ai.arty]` `ff_safe`, `counter_fire`; S11a2)

With `[fire_mission] friendly_fire` 1 own mission bursts hurt own men, so
with `ff_safe` 1 the AI keeps one own-men rule for mortars and guns
(`crates/sim/src/ai/arty.rs` `own_near`): a target is "near own men" when a
living own man (not of the firing squad) stands inside `[ai.arty]
danger_close_m` (50 m) of it, or an own squad on the move has its goal
inside 50 m and its leader inside `lift_m` (100 m). Such a target is not
ordered (`ff_holds`); every own mission whose target becomes near own men
gets one `Stop` before the mortars and guns plan (`ff_lifts`; bombs in the
air still land). A mortar's spotter stands `danger_close_m + 20` short of
its target. With `ff_safe` 0 the S11a1 rules hold (35 m mortar filter, the
gun lift A6, the prep lift P3).

With `[fire_mission] counter_battery` 1 the enemy's infantry-gun rounds
leave markers; with `counter_fire` 1 a Ready AI gun with no mission answers
its own sharpest marker (no wider than `cb_max_error_m`, 60 m) with map
fire of `mission_rounds`, after preparation fire and before observed
targets, once per new round heard (`cb_shots`), inside the reserve and the
shell budget, never on own men, and never an order the sim would refuse
(`cb_masked`: the low-angle M3 behind a crest). Counter-fire bursts are
counted only while their mission runs (`cb_bursts`, `cb_near_bursts`).

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
