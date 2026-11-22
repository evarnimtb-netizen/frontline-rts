# S14 "assault kit": code audit (read-only)

Working note for `S14-assault-kit.md` (2026-11-22, plan agent, read on `main` 6f7bda0 = S12 merged). Not a spec. Paths: `sim/` =
`crates/sim/src/`, `content/` = `sdk/crates/content/src/`, `defs/` = `mods/ww2/defs/`, `cli/` = `sdk/crates/cli/src/`.

**Bottom line.** Nothing for flame or melee exists beyond a reserved name: `flame` is in `RESERVED_CAPABILITIES`
(`content/variant.rs` 108). The pieces S14 builds on all exist: the thrown-item list as a model for a separate projectile list,
the bunker slit as plain geometry, S12's wound outcome for every damage path, the MG burst machinery for flame bursts, the S5a
cartridge feed and resupply for fuel, and the S10a-ai breach job as the model for an AI flame job.

## 1. Versions
- `SAVE_VERSION` **30** (`sim/world.rs` 479); `PROTOCOL_VERSION` **16** (`crates/net/src/protocol.rs` 42). The net crate reads the save
  version from the sim source (`crates/net/build.rs`). S13a (being built) takes SAVE 31 and no protocol bump (its plan 1), so S14a
  is **SAVE 32 and PROTOCOL 17** if S13a lands first; the layout commit names whatever is next at its base.
- `docs/STATE.md` "Versions" still says main is 29/15 (stale since the S12 merge): the S14 Docs stage fixes it if S13 has not.

## 2. Projectiles and the 1,800 pool
- `ProjectilePool` (`sim/projectile.rs` 82-188): one capped slot array; `SHELL_RESERVE` 64 (line 40) keeps the last slots for shells;
  `spawn_rounds` (`sim/combat.rs` 2014-2046) spawns shells first, drops small arms at the reserve, counts `shots_dropped`.
  Cap `projectile_cap` 1800 (`sim/world.rs` 179; rules key `content/defs.rs` 1651).
- **Finding.** Flame particles in this pool, even in a sub-quota, would compete with small arms for the non-reserved slots (the RE
  risk). The thrown-item list is the existing precedent for a projectile that is not in the pool: `MatchState::thrown` and
  `next_thrown` (`sim/game.rs` 709-710), `MAX_THROWN` 256 (`sim/items.rs` 61), flown serially in id order in the Ballistics stage by
  `fly_thrown` (`items.rs` 466, called at `world.rs` 1426), saved, hashed, validated (`validate_items`, `items.rs` 839). Flame
  follows it: its own list, its own cap per player, so it **cannot** take a pool slot (plan 3.1).
- `fly` (`combat.rs` 2252-2496) is the per-chunk parallel integrator: vehicles (`first_box`), statics (`structures::first_static`,
  `structures.rs` 2118: a linear scan, own pieces skipped), soldier capsules (`capsule`, `seg_seg`), S12 downed hash, terrain, near
  misses. A serial flame integrator reuses these helpers; it needs `pub(crate)` on `first_box` (private today).
- `shot_velocity` (`combat.rs` 627) aims a gravity-compensated shot with spread from a counter RNG (`Purpose::AIM`): enough for a
  flame stream at 25-30 m/s over 30 m.

## 3. Weapons, feed, bursts
- `WeaponClass` (`content/defs.rs` 582-595): Rifle, Smg, Mg, Mortar, Cannon, Launcher, Pistol. A new `Flame` class touches the
  exhaustive matches: `sim/supply.rs` 124-134 (`fired_class`), `combat.rs` 3355-3395 (`base_class`, falls to `OTHER`),
  `crates/godot-bridge/src/match_events.rs` 570-579 (sound cue), the sidearm class checks `content/modset.rs` 1118-1123 and 1791-1796.
- `WeaponSetup` (`content/setup.rs` 882-937) already has `burst_rounds` / `burst_pause_ticks` (MG bursts), `feed`, `load`,
  `crew`, `handover_ticks`, `hold_beyond_pct`, `fire_reveal_*`: a flamethrower is mostly a weapon def plus the class.
- Fuel as a cartridge: `Feed::Cartridge` (`setup.rs` 353) with `SupplyKind` and grams; `feed_supply` and `micro_price`
  (`supply.rs` 138-166) price it as explosives (D1: flame fuel is explosives, `artillery-on-map.md` 475). The S5a sidearm rule
  (`combat.rs` 716-733): a dry main weapon draws the slot's sidearm, so a flame man with a P 38 / M1911 sidearm fights on.
- `aim_and_fire` (`combat.rs` 670) is per chunk, writes only its own men, reads marks: a strike wind-up can start there; the strike's
  hit needs a serial step (the target's HP is in another chunk).
- `fired_class` (`supply.rs` 36-51, COUNT 11) and `kill_class` (`sim/game.rs` 573-592, COUNT 12, low nibble so at most 16) are saved
  arrays (`PlayerState::kills_by`, `game.rs` 468; `fired`, `supply.rs` 88): new classes FLAME and MELEE are a layout change.

## 4. Damage and wounds (S12)
- `hurt` (`combat.rs` 2568) -> `wounds::deal` (`wounds.rs` 126-180): with `[wounds] enabled` the W3 outcome; a hit on a Downed man
  (or at 0 HP) kills. Burn and patch damage go through `hurt` with `ZONE_NONE` (no zone factor) and the FLAME class, so every S12 rule
  (downs, credit, `downed_by_class`, `finished_by_class`) holds with no new path. `Fallen` = `(SimId, u8, u16, Hit)` (`combat.rs` 2559).
- `Blow` (`combat.rs` 2053) carries hits from `fly` to `apply_blows` (`combat.rs` 3417); `kill` 3830, `credit_kill` 4073, `down`
  4176. The downed hash `World::downed` is rebuilt before Ballistics (`world.rs` 1174).
- `Combat` (`sim/ecs.rs` 209-242) has no burn or strike field; it is hashed with its chunk.

## 5. Fortifications and the bunker slit
- Bunker def `defs/fortifications/bunker.toml`: roof 420 x 370 x 80 on three timber walls, two front walls leaving an 80 x 40 cm slit
  under the roof. The slit is only geometry (S10b R23-R26): a round passes the gap; own rounds pass own pieces (S10a-3, R26).
- `cli/fortify/bunker.rs`: the S10b bunker row (five men in the pit, staged volleys, satchel at the slit, slit-closed control). S14's
  slit row copies its layout.
- `cli/fortify.rs` `assault_run` (729): the S10a trench assault scene; S14's flame-vs-rifle row reuses it.

## 6. Trenches
- Dug depth per 4 m grid cell: `Heightfield::dug_dm` (`sim/terrain.rs` 647), path trench threshold `TRENCH_NAV_DM` 10 (line 65).
  The AI probes enemy trenches by dug depth (`ai.rs` 2732 `enemy_trench`). A walk along a trench is a bounded search over 4 m cells.
- Throws into trenches already aim half-way down over dug ground (`items.rs` module docs; `release_throws` 331).

## 7. Suppression, morale
- No morale system (grep: none). Suppression in `Combat::suppression` (milli-points), `add_suppression` (`combat.rs` 3317); near miss
  `[combat] near_miss_cm` 200, crouch 30, pin 70, duck 25 (`defs/rules/skirmish.toml` 265-411). Flame "fear" is a near-miss value
  and a burn value, physical (design 4.3).

## 8. Commands and capabilities
- `CommandKind` ends with `Reinforce` (`sim/command.rs` 277). New: `ClearTrench`, `Strike` (PROTOCOL +1).
- `Capability` (`content/setup.rs` 1130-1162; `ALL` 15, `MASK`); `CAPABILITIES` and `DERIVED_CAPABILITIES` (`at_hunt` from a
  launcher member) in `content/variant.rs` 87-108: `flame` becomes a derived capability (a member with a Flame-class weapon).
- `member_trait` (`setup.rs` 1093-1124): 7 of 8 bits used; S14 needs no new trait.

## 9. AI
- `ai/breach.rs` (S10a-ai): want / stage / suppress / smoke / go / retry / through phases, `breach_focus` (736) steering the base of
  fire, the urgent buy of the faction's assault card (`ai.rs` 1724 `buy`). The flame job copies the shape.
- `ai/ammo.rs`: Refill by `supply::squad_ammo_pct` (a squad average): a dry flame man in a full squad never triggers it (finding: S14
  needs a refuel rule).
- `ai/assault.rs`: base of fire on `SuppressUnits`; `ai/demolish.rs` and `ai/medic.rs`: liveness patterns (L1 deadline, L2 stall).

## 10. Cards
- `defs/units/ger_sturmpioniergruppe.toml`: 8 men (leader MP 40, 3 StG 44, 4 MP 40), `ai_weight` 60, cost 264.
- `defs/units/usa_assault_section.toml`: 12 men (6 M1, BAR team, bazooka team, 2 demolition men), `ai_weight` 35, cost 380.
- Neutral switching of a card's member: precedent S11b `[items] rifle_grenades` 0 drops the kind from squads at setup build
  (`MatchSetup::s11b_off`, `setup.rs` 367-373). S14 uses a member-level `when` key (plan 4.2) so both cards resolve to today's men
  with `[flame] enabled` 0.

## 11. Presenter
- Snapshot: `projectiles` (`snapshot.rs` 735), `smoke` (733), `ThrownSnapshot` (98); flame particles and patches need their own
  lists. Bridge cues per weapon class (`match_events.rs`).

## 12. Targets numbering
- `docs/slices/targets/S12.toml` already uses T159-T173; S13 (planned T150-T159) was
  renumbered to T174-T184 by its build. S14 starts at **T185**, right after it.
