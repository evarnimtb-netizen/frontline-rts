# S11a1 log (gun fire missions, infantry guns, artillery AI; at most 150 lines)
Spec: common `docs/slices/S11a1.md`, stages `S11a1-build-A.md`, `-B.md`, `-C.md`; targets `targets/S11a1.toml`;
ablations `targets/S11a1-ablations/`. Branch `claude/project-thread-e7nsak`, base `730e67f` (S10c-ai; code
`c1302b3`). Another agent may work in this checkout: commit only your files (`git commit -m ... -- FILES`), never
`git add -A`, never the untracked `.claude/`, `target/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: Build A done 2026-11-17 (all of G1-G7, every switch at its spec default). Next: Build B (`-build-B.md`).**
- SAVE 27, PROTOCOL 14. Neutral base `target/s11a1-base40/skirmish40.json` (= `target/s10c-F3-40`).
- Code map: `indirect.rs` `solve_gun` (candidates, mount pitch, `arc_clear`); `game.rs` `gun_mission_refusal`,
  `order_gun_mission`, `gun_indirect_weapon`, `gun_he_slot`, `gun_mount`; `gunnery.rs` `gun_mission_tick` (called
  first in `gun_one`, which then picks no main-gun target); `fire_mission.rs` `firing_unit` + `radio_men` (used by
  `linked`), `tube_pos` (used by `observe_bursts`, `end_mission`), `start_error_for` (G4).
- `sq.order` arms: none needed a new `FireMission` arm; `fire_mortars`/`fire_mission_tick` return at once for a
  gun squad (`mortar_gunner` None); `combat.rs` 152, `world.rs` 6485/6572/6636 and `ai/mortar.rs` 216/730 read
  mortar squads only. Added: the vehicle-squad load check and `mission_ok` accept a gun's mission.
- A gun test scene: `s11a1_tests.rs` `add_indirect_gun(setup, lo, hi, min_m)` (Pak 38 clone `test:inf_gun`, leIG
  charges, mount -10..hi, squad weapon = the new weapon), `scene_with(threads, lo, edit)` (gun at (300,500), Grenadiers
  at (330,500)), `order`, `mission`, `run_out`; `add_fo` gives `test:fo` and radio men to every `*_crew`.
- `[indirect]` for a gun (weapon class `cannon`, main mount up to 89 deg with `low_angle`): `charges_m_s = [92, 120,
  150, 180, 210]`, `elevation_deg = [0, 73]`, `low_angle = true`, `range_m = 3550`, `spread_mrad = 8`.
- **Build B must** print `FmCounters::refused` as a map by name (or let `neutral.py` skip it): it grew 7 -> 9 items
  (`masked`, `not_ready`), so `neutral.py` says `DIFFERENT, same 40/40` on that one key; every match is equal.
- Direct mode for a gun is the crew squad's leader by voice (he is an observer at `observe_leader_m`).
- Gun refusals still add to `fm.refused` (the order event is shared); `masked`/`not_ready` also count in `s11`.
- AI: `inf_gun` guns are parked in `take_guns` (no list) until Build C; the mix never buys `inf_gun`.
- Riflemen 200 m off are not seen by a gun crew on flat ground in tests; use a tank for direct-fire scenes.

## Build A (2026-11-17)
Commits: `9ed0bf0` layout (SAVE 27, kill class u16, S11 counters, AI records), `84a972a` keys / schema / radio /
inf_gun / AT-gun split, `030e6ff` G2 solver, then G1-G7 (gun missions, radio, ends, rearm, counters).
Tests (control in brackets): `seventeen_vehicle_types_keep_their_kill_rows` (old u8 pack), `load_rejects_bad_s11a1_state`
(staged save loads), `s11a1_layout_roundtrips_and_hashes`, `at_gun_ai_ignores_infantry_guns` (at_gun clone gets a
job), `load_rejects_bad_s11a1_keys` (`gun_break_m` widened: failed), `indirect_gun_keys_and_radio_trait_parse` (band to
90), six G2 tests in `indirect.rs` (M3 masked; level ground), `gun_takes_a_fire_mission` (`gun_missions` 0),
`gun_mission_refusals_in_order` (accepts last), `gun_mission_fires_counted_he` (no HE), `gun_mission_waits_for_the_crew`
(loader check removed: failed), `gun_on_mission_fires_no_direct_round` (Idle fires), `infantry_gun_counts_only_in_s11`
(at_gun in s9), `gun_mission_deterministic_threads_and_save_load`, `gun_mission_leaves_ranging_memory` (map fire: none),
`gun_map_error_is_twice_the_observed` (mortar keys), `gun_mission_ends_when_packed` (not packed), `gun_mission_breaks_for_self_defence`
(200 m), `radio_links_an_observer_across_the_map` (three controls), `radio_does_not_reveal` (near enemy seen),
`infantry_gun_does_not_rearm_in_base_when_switched_off` (1 rearms).
Neutral: `neutral.py base40 A40` and `... Aoff40`: `DIFFERENT, same 40/40`, the only key `s5b.refused` 7 -> 9 items.
Deviations: the mask never reads the last fifth of a flight (steps 17-20, spec); `Masked` in the sim is tested
through the solver test, not a cratered map; the crew-ordered-off end is "crew link released" (`sv.crew` None);
unit orders that take one man off make the gun wait, not end; `InsideMinimum` only when no candidate exists (a
low-angle gun always has one at short range); `IndirectDef` literals in `indirect.rs` tests gained `..Default`;
the setup check accepts `[indirect]` on a `cannon` with `low_angle`, and mount elevation to 89 for it.
Check: `tools/check.sh quick 730e67f` all ok, 907 tests passed (release CLI rebuilt first).

## Plan revision (2026-11-16, after an independent review: 1 blocker, 6 major, 4 minor; all verified in the code, none rejected)
Spec 3: kill-class carrier widened in the SAVE 27 commit (15-type cap), `fo_lost_at`, `guns_bought`,
`gun_direct_rounds`. G2: mask steps 2..=16 at 0.5 m, hull-pitch limit, test with zero error. G3: mission ttl, load
check and `end_mission` for gun squads, infantry guns counted only in `S11Counters` (T80, T84, mortar rows clean).
A1: role split of `take_guns` / `gun_jobs` / `gun_recrews` and `outside_limit` moved into Build A; Build C wiring
(lists before `assign`, `arty_pot` beside `gun_pot` incl. `ai/fort.rs`, `mortars` before `arty`). Balance note: the
high-angle leIG may favour Germany; the measure reads `arty_off` first.

## Plan (2026-11-16)
Scope (spec 1): gun fire missions on the S5b mission code and the S9 gun entity; low- and high-angle solutions
with a mask test; gun aim-error keys; a radio link; leIG 18, 105 mm M3, VB-Trupp, FO Party at the design's
figures; fortify rows `gun_fm`, `mask`, `barrage`, `breach_prep`; AI: one gun and one observer team a side, static
in base, observed missions inside a 25 % shell budget, lifted near own men; preparation fire for the breach job by
guns and mortars. Cut to S11a2 with reasons (spec 1, outline spec 12): crate-only shells and AI resupply, friendly
fire, registration, counter-battery, displacement and tows, defensive fire, assault hold, HEAT, per-faction delays.
Decisions taken for the owner (all (A), `docs/TODO.md` S11a1-1..9): split; priced base rearm for now; friendly
fire in S11a2; no tows; prices 320 / 380 / 80 / 120; only the breach job waits for preparation fire; T131 hard at
60 %; HEAT later; cards buyable before the presenter pass.
The breach gap, honestly: preparation fire is aimed at the staged row (T131: 30 % -> >= 60 %). In matches breach
jobs do not start (T123), so `prep_missions_per_match` is expected near 0 there; that needs S10c-6 (B).
Stage split: A (SAVE 27 layout, keys, solver, gun missions, radio, rearm switch; neutral by content) -> B (content,
rows, JSON; neutral because the AI buys nothing; gate T128-T130) -> C (AI chunk 1 A1-A6, chunk 2 P1-P3 and the
breach row; fallback: stop after chunk 1 with `prep` 0).
