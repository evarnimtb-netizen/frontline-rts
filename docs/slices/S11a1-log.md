# S11a1 log (gun fire missions, infantry guns, artillery AI; at most 150 lines)
Spec: common `docs/slices/S11a1.md`, stages `S11a1-build-A.md`, `-B.md`, `-C.md`; targets `targets/S11a1.toml`;
ablations `targets/S11a1-ablations/`. Branch `claude/project-thread-e7nsak`, base `730e67f` (S10c-ai; code
`c1302b3`). Another agent may work in this checkout: commit only your files (`git commit -m ... -- FILES`), never
`git add -A`, never the untracked `.claude/`, `target/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: Build B done 2026-11-17 (content, A review fixes A-1..A-6, rows, JSON). Next: orchestrator gate T128-T130, then Build C (`-build-C.md`).**
- SAVE 27, PROTOCOL 14. Neutral base `target/s11a1-base40/skirmish40.json`; Build B sets in `target/s11a1-B40/`.
- Cards (roster end, `ai_weight` 0): `ww2:ger_leig18_squad` (vehicle `ww2:ger_leig18`, crew `ger_leig18_crew`),
  `ww2:usa_105mm_m3_squad` (`usa_105mm_m3`, crew `usa_105mm_m3_crew`), `ww2:ger_vb_trupp`, `ww2:usa_fo_party` (tag
  `observer`). Weapons `ww2:leig18`, `ww2:m3_105mm`; rounds `ww2:75mm_igr18`, `ww2:105mm_m1_he` (the only ammo slot).
- **`buy_role_within` needs `ai_weight` > 0**: it takes the first roster card with weight > 0 and the role. Build C
  must set the two gun cards to 5 (as the S9 gun cards) in its own commit; the mix never picks `inf_gun` (`mix_slot`
  skips the role), so 5 is safe. Observer cards: A3 buys by trait, keep weight 0 or give them their own path.
- Test recipe: a real gun arrives emplaced and crewed from `spawn_squad_at` / `s6_tests::spawn(w, p, card, x, y)`
  (facing +x); `s11a1_tests.rs` `crest_gun(card, crest)` (crest band at x 500, gun at (50,500)), `order`, `mission`,
  `run_out`. Observer team: spawn `ww2:usa_fo_party` / `ger_vb_trupp`; beyond `observer_link_m` 150 it links by radio
  (crew commander slot has `radio`; `leader` may only be on slot 0, so the crew leader is the gunner slot).
- Known ground for a mission: a seeing observer, or `World::mark_intel`. Map fire: `observe_*_m` 0.
- Row code: `sdk/crates/cli/src/fortify/s11a1.rs` (`gun_fm`, `mask`, `barrage`; `breach_prep` and `breach_prep_gun`
  print null keys: Build C fills `BreachPrepRow`). Rows run per seed on threads (`par_seeds`); left out with
  `--no-s10b-ai`. One-off: `cargo test --release -p frontline-cli s11a1_rows_one_off -- --ignored --nocapture`.
- JSON: `factions.<f>.s11a1` (S11Counters by name, arrays by mode, every S11a1 AiCounters name, `refused_masked`,
  `refused_not_ready`, `kills`, `income_mp`, `matches_with_*`, `gun_kills_in_tank_he` true) and `s11a1_derived` (spec
  7). `gun_refused_total` = masked + not_ready (other gun refusal kinds are counted nowhere since A-3: if C needs them,
  an AiCounters field is a layout change). `fo_bought_matches_pct` reads `fo_buys` (AI only).
- Gun kills still count as `tank_he` in `kills_by` / `tank_mp_killed` (the JSON flag says so); `gun_kills` separate.
- leIG at 300 m has only low roots (every high root > 73 deg); behind a 25 m crest it needs 450 m (G2 test).
- `neutral.py`: keys `s11a1*` the base lacks are skipped; a list that grew with a zero tail equals the base.

## Build B (2026-11-17)
Commits: `6f22f00` content + fix A-1, `c2afd8c` fixes A-2..A-6, `6a8db14` rows / JSON / neutral.py / doc, `2097289` rows on
threads, `50eedd6` one-off 10-seed test; this log commit. Content as built (`artillery-on-map.md` 2.1 lines 133-136,
2.2 lines 148-151, 4.7 lines 825-840; spec 4 C1-C4): leIG 18 400 kg, crew 5, -10..+73, traverse -5..+6, charges
92-210, 3,550 m, 8 mrad, 8/min (7.5 s), 12 Igr 18 (6.0 kg), 320 / pop 2 / 90 s / 3:00, max 2; M3 1,130 kg, crew 6,
-9..+30, +-22, charges 205-311, 7,600 m, 6 mrad, 3/min (20 s), 10 M1 HE (19 kg; blast 175 cm, 160 rays to 17.5 m,
supp. 18 m, crater 90 cm, depth 44 cm = 30 x 1.46: the spec names no depth), 380 / 2 / 120 s / 4:00, max 2;
VB-Trupp 2 men 80 / 60 s / max 2; FO Party 3 men 120 / 60 s / max 2; all `ai_weight` 0. Geometry (pivot, muzzle,
shield plates) "verify". Changed from spec: the crew commander has `radio` but not `leader` (the def check allows
`leader` on slot 0 only); the crew leader is the gunner slot (first man).
Review fixes: A-1 one rule `vehicle::low_angle_top` (def check to 89, cross-check: above 60 only with a low-angle
table; setup check the same), `radio` moved to TRAITS, the test parses real defs (control: no `low_angle` -> refused);
A-2 `range_m` -> `CannotReach`; A-3 gun refusals out of `fm.refused` (event and s11 kept), JSON flag
`gun_kills_in_tank_he`; A-4a mission ends when `sv.crew` names no live squad; A-4b re-lay to the mission bearing
while holding fire; A-5 `gun_ready()` in `crew_ok`; A-6 tests `real_m3_is_masked_behind_a_crest_through_the_sim`
(control leIG, flat), `gun_refusals_take_the_first_broken_rule`, `gun_refuses_past_its_table_range`. A-4/A-5 have no
own test (no cheap scene: a round every 7.5 s hides a 10 s re-lay). Mask note: the test skips the first tenth and last
fifth of a flight; revisit with S11a2 friendly fire.
Other files touched (smallest fix): `damage.rs` test list (two guns), the 17-type test (now 18 with a clone),
`indirect::candidates` pub(crate). Solver test with real defs lives in `s11a1_tests.rs` (the solver is the sim's).
Gate (`s11a1_rows_one_off`, seeds 1-10, the same code as `fortify-test`): **T128** ratio 600.0 % (observed 8.3 m:
leIG 9.9, M3 6.8; map 49.8 m; exposure 160 rounds; control radio man dead 100.0 %). **T129** M3 refused masked 100 %
(control flat 0 %), leIG over the crest 80 % (control flat 80 %: "x > 540" is crossed short by the 50 m map-fire
error; a report), orders 20. **T130** trench vs open 11.8 % (open 68.0 %, trench 8.0 %; control: dug, squad in front
76.5 %), trench suppressed 2.2 %. Bench: flat map; gun_fm 600 m, FO 300 m short (radio only); mask crest band 25 m,
gun 530 m from target; barrage M3 500 m, FO 350 m. Rows wall 22.6 s for 10 seeds (threads, under load).
Neutral: `neutral.py base40 B40` `same 40/40`; `base40 B40off` (a_off) `same 40/40`. Tests: content `s11a1_content`
4, sim s11a1 25, CLI `fortify_s11a1_rows_run`, `s11a1_json_names_match_the_targets_file`, neutral 13.
Check: `tools/check.sh quick 730e67f` all ok, 915 tests (release CLI rebuilt; first run failed two roster-order
tests, fixed in `aaae560`). Full `fortify-test --seeds 10` (`target/s11a1-B40/fortify10.json`, with the 30-min S10b-ai
rows) and `targets --from-json` were still running at this commit (a container restart lost the first run).

## Build A (2026-11-17)
Commits: `9ed0bf0` layout (SAVE 27, kill class u16, S11 counters, AI records), `84a972a` keys / schema / radio /
inf_gun / AT-gun split, `030e6ff` G2 solver, `067dee7` G1, G3-G7 (gun missions, radio, ends, rearm, counters).
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
