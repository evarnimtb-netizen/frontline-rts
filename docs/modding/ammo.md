# Ammunition (S5a)

Every round is counted from S5a on. A man carries rounds by cartridge id; a
vehicle carries main-gun rounds in its rack and belts for its MGs; an AT
team carries a count of rockets; items (grenades, satchels, Panzerfausts)
are counted per man as before. Refills in the own base cost manpower,
round by round (owner decision D1: no free ammunition anywhere).

A mod without a rules `[supply]` table keeps the S4 behaviour: unlimited
rounds and free refills (legacy content, owner question S5a-22 (A)).

## Cartridges (`defs/cartridges/*.toml`)

A cartridge is a counted round whose flight lives on the weapon (rifle,
SMG, pistol and MG rounds, mortar bombs). It has no ballistics.

```toml
id = "ww2:792x57_clip"
name = "7.92 mm (clips)"
caliber_mm = 8
supply = { kind = "small_arms", g = 28 }
sources = ["..."]
```

- `supply.kind`: `small_arms`, `explosives` or `shells` (`construction`
  is refused: it comes with S6).
- `supply.g`: grams per round, 1..=100000, its share of clip, belt, box or
  carton included (what the base hands out; S5a-3 (A)).
- One id per cartridge and packaging (S5a-2 (A)): rifle clips do not feed
  an MG; belts reach the gun through the men who carry them.

Ballistic rounds (`defs/ammo/`) and items (`defs/items/`) take the same
`supply` table. An ammo def may set `rare = true` (APCR, HVAP): rare rounds
refill on their own timer up to the vehicle's `rare_cap` and are never a
weapon's feed.

## Weapons

```toml
feed = "ww2:792x57_clip"   # what each shot takes from the man's rounds
load = 60                  # rounds he carries by default, the magazine included
burst_rounds = 7           # bursts: after every 7th shot ...
burst_pause_ms = 1000      # ... this pause on top of the interval (0 = none)
hold_beyond_pct = 0        # own fire-discipline share of range (0: the rules')
```

Bursts count on the man's (or vehicle MG's) shot counter, so they need no
saved state; a magazine change also pauses. The shipped bursts: MG 42
bipod 7 / 1.0 s, MG 42 tripod 10 / 1.2 s, MG 34 (vehicles) 7 / 1.0 s,
M1919 and M1919A4 8 / 1.0 s, BAR 4 / 0.6 s, StG 44 3 / 0.8 s (sources in
each file). SMGs fire their magazines (S5a-24 (A)).

In a mod with `[supply]`, every weapon a soldier carries (or a vehicle
mounts as an MG) needs a `feed`. Launchers fire their `ammo` (a team count,
`rounds_carried`), cannons their mount's rounds.

## Men

```toml
[[members]]
slot = "ammo_bearer"
weapon = "ww2:kar98k"
rounds = { "ww2:792x57_belt" = 600 }
```

`rounds` gives each man of the slot his load by cartridge id, counting the
rounds in his weapon. A feed of his weapon or sidearm not listed gets that
weapon's `load`. At most 4 different rounds per unit. The S4 form
`ammo = N` still loads as "N rounds of his weapon's feed"; a slot may not
set both.

In the sim:

- every shot is counted when it is requested (a round the projectile pool
  drops is still spent);
- the reload takes a magazine from his own rounds; with none, a crew
  weapon (MG, mortar) takes a magazine from a squadmate within
  `[combat.share] crew_feed_m` (ammo bearers and assistants first);
- a dry man with a sidearm in his slot draws it and goes back to his main
  weapon at the next reload once it has rounds again (S5a-23 (A));
- every `[combat.share] interval_s` the man lowest on a round (below
  `below_pct` of his own load) takes `share_pct` of a squadmate's load
  (within `reach_m`, above `giver_above_pct`), never above his own load;
- a mortar fires bombs from the gunner, then from his crew; with none left
  the barrage ends.

## Vehicles

```toml
rare_cap = 3   # the most rare rounds held; at most the rules' rare_cap_max
```

Every vehicle of a mod with `[supply]` sets `rare_cap`. The rare rounds
stowed must not exceed it. A `rare_cap` above `[supply] rare_cap_max`
fails to load:

```
vehicle `ww2:ger_pz4_h`: rare_cap: rare_cap 12 is above the rules' supply.rare_cap_max (10, rules `ww2:skirmish`) (fix: lower rare_cap to at most 10, or raise [supply] rare_cap_max in defs/rules)
```

Main-gun rounds must be of kind `shells`.

## Prices and the base refill (`[supply]`)

A round of `g` grams of a kind priced at `P` manpower per tonne costs
`g × P` millionths of a manpower point. Main-gun rounds also take the rack
factor: the lower of `rack_rearm_price_pct` and the share that keeps a full
rack within `rack_refill_max_card_pct` of the vehicle's cost (Panzer IV
53 %, Sherman 51 % at the shipped prices).

In his own base:

- a man gets `base_small_arms_rounds_s` small-arms rounds per second (the
  most depleted round first), and alongside them items one per
  `[items] resupply_s` and mortar bombs one per `base_bomb_s`, but no
  bomb until `base_bomb_after_fire_s` after the squad's mortar last fired
  (the base battery brake; 0: no wait);
- an AT team gets a rocket per `[items] resupply_s`;
- a vehicle gets a standard round per `[vehicles] base_rearm_s` (the entry
  furthest below its load), a rare round per `rare_rearm_base_s` up to its
  `rare_cap`, and a belt into each MG with spent stowage per standard step.

Whole manpower is taken one point at a time as prepaid credit (S5a-8 (A));
the fraction below one point is kept for the next round. When the owner
cannot pay, that refill pauses and resumes the first tick he can. Nothing
goes into debt. The unit's owner always pays.

Shipped weights and loads, with sources, are in each def file's comments
and in `docs/slices/S5a-ammo.md` "Content".

## Fire discipline (`[combat.fire]`)

Every man has a fire mode (`SetFire`; the HUD's fire strip): `discipline`
(the default) opens fire only within `hold_beyond_pct` (70 %) of his
weapon's range, or the weapon's own `hold_beyond_pct`; `at_will` to full
range; `hold` only at enemies within `hold_open_m` (30 m), while his
Panzerfaust or launcher fires by its own reach. A held squad springs to
discipline once one of its men fires or is suppressed to
`hold_spring_points` (0: own fire only).
A Suppress order fires to full range in any mode but `hold`. Vehicle MGs
keep discipline. A rules file without `[combat.fire]` fires to full range
(the behaviour before S5a, S5a-29).

## The AI (`[ai.ammo]`, ambush keys in `[ai.assault]`)

A squad whose ammunition share (its crew weapon's feed, else its men's
main weapon; AT rockets too) falls under `return_below_pct` with no known
enemy within `threat_radius_m`, or under `dry_pct` at all, walks back to
the base, waits there until `refill_full_pct` or `refill_max_s` (or until
its paused refill has waited `rearm_stall_s`), then takes a task again. At
most `max_returning_pct` of the squads at one point leave in one plan; a
point's last squad stays unless it is dry. Mortars barrage only above
`mortar_reserve_pct` of their bombs (unless an own point is under threat)
and never order more rounds than they hold. Buys keep `float_mp` back
while a unit waits in base for a refill. A Panzerfaust squad that sees an
enemy vehicle within `ambush_watch_m` of its point or itself holds fire in
an ambush beside the vehicle's path (or stalks a stopped vehicle to a
flank spot inside the Panzerfaust's reach) for at most `ambush_max_s`,
until the vehicle is seen beyond `ambush_release_m`
(`crates/sim/src/ai/ammo.rs` has the rules).
