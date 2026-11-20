# S12 log (wounds and medics; at most 150 lines)
Spec: `docs/slices/S12-wounds-medics.md`; stages `S12a-build-A.md`, `-B.md`, `S12b-build-C.md`; targets `targets/S12.toml`;
ablations `targets/S12-ablations/`. Branch `claude/project-thread-aj1tww`, base `<BASE>` = `1209c99` (main: S11b + B2).
Commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: S12a Build A done** (e847067..HEAD on `1209c99`): SAVE 30 / PROTOCOL 16, keys, W1-W5, W12-W14, R7/R8 rows, `s12` JSON block;
every S12 switch 0. Neutral proof: `target/s12-base40` (REF0) vs `target/s12-A-40` (REF1) `neutral.py`: same 40/40, Germany 20/40 both;
only new keys: `factions.*.s12` and `sides[].s12` (all 0). REF0: length mean 20.15 min, HE share 25.5; kills_by_source_pct ger rifle 41.5,
tank_he 19.9, vehicle_mg 10.1, mortar 9.1, mg 6.9; usa rifle 53.0, vehicle_mg 16.5, tank_he 8.1, rifle_grenade 7.2, mortar 5.8.
**Code map.** `crates/sim/src/wounds.rs`: types, `zone_at` (W1), `deal` (W2/W3 per hit; `None` tuning = the old `saturating_sub`),
`wounds_tick` (W4 bandage/bleed, W5 bleed-out; once a second at `(tick + id) % hz`), `validate_s12`/`wound_ok`. `combat.rs`:
`hurt`/`hurt_by` -> `deal`; `resolve_falls` (W3 outcome, stable id order; old loop kept verbatim when `enabled` 0), `down` (W5: rider
to `door_spot`, gun crew slot freed, seeded `WOUND_BLEED` draw), `credit_kill` (shared), `leave_squad` (factored out of `kill`), `fly`/
`burst` read `World::downed` (spatial hash, rebuilt only when casualties exist), wounded-shooter penalties in fire. `damage.rs` rider core
hit and `guns.rs` crew core hit call `deal`. `squad.rs` pace, `game.rs` population (+casualties), `spotting.rs`, `snapshot.rs` (Downed
shown as a corpse with ticks_left 0 until Build B). W14 needs no `rules.rs` edit (`check_defeats` counts `soldiers()` only).
Tests: `crates/sim/src/s12_tests.rs` (16), `sdk/crates/content/tests/s12_content.rs`, `fortify/s12.rs` (rows + 1/2/3/8-thread hash),
`save_fuzz.rs` `staged_s12` (`World::down_soldier` hook). Fuzz 0 panics, 0 unloadable.
**Build B must know.** Tend/Carry/Evacuate are refused `FortRefusal::NotAllowed` (squad None), Reinforce with `Some(squad)`: the
dispatch arms in `game.rs` (grep `NotAllowed`). An `aid` stock def is refused at load (`defs.rs` "aid posts are not built yet");
lift it and audit the `StockKind` matches: `structures.rs` (11), `ai/supply.rs` (4), `ai/truck.rs` (3), `ai/fort.rs`, `ai/armour.rs`,
`fort.rs`, `scenarios.rs`, `world.rs`, godot-bridge `intents.rs`/`match_node.rs`, cli `armour_test/s8b.rs`/`s9.rs`, content `modset.rs`.
Beds are validated only on Aid structures. AI medic counters exist (`AiCounters`), no job code.
**R7/R8 (fortify10.s12, T159-T161), seeds 1-10, 220 men a variant** (`S12_SEEDS=10` one-off test): trench out 5.9 % vs base 5.5 %
= 107.3 % (R7 pass, 13 vs 12 men: thin), head_300 6.4 %; prone out 2.7 % vs 2.7 % = 100 % (R8 holds). Every downed man died in the 300 s.
**Deviations.** Keys and BASELINE went into the layout commit (setup bytes change with the layout). `Hit` also carries `dmg` (for
`head_kill_dmg`). Downed men keep hp 1. Zone hit counters count on the victim's owner, only with `enabled` 1. Bleed-outs credit a kill
to the hitter but do not add `stats.deaths`. A detached wounded man has no own speed penalty (only the squad pace). `riders_lost` and
`gun_crew_lost` count at the down. Steps 4-6 are one commit (the pieces share `Fallen` and `resolve_falls`). R7 "lose" = left the squad
(dead or downed), the stricter reading. New commands refused with the existing `NotAllowed` reason (no new refusal variant).
