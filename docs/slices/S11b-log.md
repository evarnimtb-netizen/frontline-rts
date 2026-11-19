# S11b log (field howitzers as base guns, light mortars, HE rifle grenades, A0 fixes; at most 150 lines)
Spec: common `docs/slices/S11b.md`, stages `S11b-build-A.md`, `-B.md`, `-C.md`; targets `targets/S11b.toml`; ablations
`targets/S11b-ablations/`. Branch `claude/project-thread-e7nsak`, base `<BASE>` = `8d2731b` (also main). Another agent may
work in this checkout: commit only your files (`git commit -m ... -- FILES`), never `git add -A`, never the untracked
`.claude/`, `target/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: Build B done (both chunks, C1-C7 all built). Next: Build C (`S11b-build-C.md`).**
- SAVE 29, PROTOCOL 15 unchanged. Every S11b switch is still 0. REF1 = `target/s11b-A0-40`; B1 and B2 equal it 40/40.
  `neutral.py` now lets `s5a.kills_by` grow with a zero tail (REF0 vs REF1 then shows only the X1 keys).
- Cards (all `ai_weight` 0, appended to the rosters in this order): `ww2:ger_lefh18_squad`, `ww2:ger_kz_grw42_team`;
  `ww2:usa_105mm_m2a1_squad`, `ww2:usa_60mm_mortar_squad`. Tags `["vehicle", "field_howitzer"]` / `["mortar",
  "light_mortar"]`. Howitzer vehicle ids (for `s7.kills_by_vehicle`): `ww2:ger_lefh18`, `ww2:usa_105mm_m2a1`; weapons
  `ww2:lefh18`, `ww2:m2a1_105mm`. The heavy mortar cards set no `ai_weight`: the loader gives them `DEFAULT_AI_WEIGHT`
  100, so L1's "half the heavy card's effective weight" is 50 unless the mix scales it (read `ai.rs` 2034-2054).
- `s11b.how_rounds` is now counted (Build A had the field but no count; `gunnery.rs`, by the card's `field_howitzer`
  tag, beside `s11.gun_rounds`). `s11b.arty_emplaced` / `arty_crew_lost` have no JSON output yet (review A-5: M1).
- A fortify row gets a howitzer mission accepted as `a_howitzer_card_takes_a_fire_mission` does: S11a1's
  `crest_gun(card, crest)` (flat world, the card at (50, 500), a Grenadiergruppe at (600, 500) that sees the target,
  every gun spawned emplaced and manned), `order(w, sq, (650, 500), n)`: accepted, first round within 180 s (both
  howitzers; the leIG 18 too, with `how_rounds` 0). Control `gun_missions` 0: `NotMortar`.
- Rifle grenades: US `rifle_grenadier` x2 (first two of the five riflemen) with `ww2:m7_rifle_grenade` = 3; German
  `rifle_grenadier` x1 (first of the four) with `ww2:schiessbecher_sprgr` = 3 (weapon `ww2:schiessbecher`, ammo
  `ww2:gsprgr30`). With `rifle_grenades` 0 they are riflemen (B2 = REF1). Variants have their own member lists: none.
  `s11b_tests::arm(setup, RIFLES, "rifleman", n)` still arms only the `rifleman` slot (now 3 men).
- Fired rifle grenades are not in `ammo.fired` (review A-3: no class there, the saved array is fixed): read
  `s11b.rg_fired`. Kills: `kill_class::RIFLE_GRENADE`.
- Review A-4 NOT fixed: `cb_alt_holds` still counts every planner pass that holds; counting once per evidence change
  needs a saved field (SAVE 29 is fixed), so it is left for the coordinator. Read it as "plan passes held". Note: with
  `cb_alternate` 1 a gun that never gets an ordinary target never counter-fires again (as the spec intends): read
  `cb_alt_holds` and the share of matches in which each side still counter-fires.
- The light mortars' game range is the weapon's `range_m` (275 / 370 m): the AI's band and the table range. The sim
  does not refuse a mortar order beyond it (as for the heavy tubes); the solver itself reaches 1,124 / 1,800 m.
- `ipv6_loopback_handshake` fails in this container (no IPv6): not S11b code; not skipped.

## Build B (2026-11-19; commits `d3d3170`, `b81c462`, `e35144d`, `6bb4ea6`, `64ff872`, `cbbd276`, docs, `b645588`, log)
New defs: `ammo/105mm_fhgr38`, `ammo/gsprgr30`, `cartridges/60mm_m49a2`, `items/schiessbecher_sprgr`, weapons
`lefh18`, `m2a1_105mm`, `m1a1_75mm`, `kz_grw42`, `m2_60mm`, `schiessbecher`, vehicles `ger_lefh18`, `usa_105mm_m2a1`,
`usa_75mm_m1a1`, units `ger_lefh18_{crew,squad}`, `usa_105mm_m2a1_{crew,squad}`, `usa_75mm_m1a1_crew`,
`ger_kz_grw42_team`, `usa_60mm_mortar_squad`. Changed: the two line squads (C5), the four gun cards (`limit_group`),
`skirmish.toml` `[limit_groups] "ww2:artillery" = 2`, the rosters (appended).
Figures (design line; "verify" = S11b-13 plan value and its source): leFH 18 1,985 kg, crew 6, -5..+42, 10,700 m,
4/min, charge 1 200 (D 143, 757), 12 FH Gr 38 (D 395), 90/54/25 s (D 721), re-lay 25 (D 726-727), shield (D 809),
130 % (D 812), 480 / pop 2 / 180 s / 8:00 / max 1 (D 837, 842); top charge 325 verify (10,700 m at 42 deg drag-free);
traverse +-22, 9 and 6 deg/s verify (M3 def); spread 6 mrad verify (M3). M2A1 2,260 kg, crew 8, -5..+65, 11,300 m,
3/min (D 156), charges M3's + 333 verify (D 750 charge 1), 12 M1 HE (D 396), 90/54/25/25 (D 722), 500 / 2 / 180 s /
8:00 / 1 (D 838). FH Gr 38 19,000 g verify (as M1 HE; D 509), Sprgr 34 x 1.42: 170 / 160 rays / 1,700 / 17 m /
85 / 43 (D 143, 1051). kz. GrW 42: 15/min, 60-275 m (GA 726, game range verify), charges 75, 105 and band 45-87
(grw34), grw34 `[he]`, 18 bombs (GA 592), 181 MP, 45 s (RE 135). 60 mm M2: 18/min, 90-370 m (GA 730), charges
70/95/117/133 verify, band 45-85 (m1_mortar), 1,330 g (GA 730), `[he]` m1_mortar x 0.75 verify, 30 bombs (GA 591),
240, 45 s (RE 179). Pack howitzer (no card): 653 kg, crew 5, -5..+45, 8,800 m, 3/min (D 157), 30/18/15, push 90 cm/s
(D 720); existing `ww2:75mm_m48` (its `[he]` is the Sprgr 34's); one charge 294 verify (8,800 m at 45 deg, as the
howitzer tops); re-lay 10, cost/cooldowns, 10 rounds, traverse verify (leIG / M3). Gewehr-Sprenggranate 250 g, 250 m,
60 m/s, Mk 2 block with 48 rays, blast 160, reach 800 (S11b-7, owner to confirm); 30 m / 3 s / 3 s / 15 mrad as the M7.
Decisions: "no push": the def check allows `speed_cm_s` 0 for a towed gun but the setup check refuses a card speed of
0, so 1 cm/s with `push_men` = crew + 1 (7, 9). kz. GrW 42: 4 men with "2 + 4 + 6 + 6": the leader carries the fourth
6 (one bearer fewer than the heavy team). The rifle-grenadier slot is a split (no fallback needed: B2 = REF1).
Review fixes (coordinator, stage A review): A-1 `kill` and boarding clear `rg` (man, target, aim, stow); A-2 the aim
needs an enemy in the target's slot (ids are not generational), `RgState::target` doc; A-3 `class_of`: rifle
grenades leave `ammo.fired` (no Panzerfaust count); A-4 not fixed (above); A-5 for C. Also a Build A miss:
`how_rounds` was never counted (fixed, `6bb4ea6`).
Tests: sim `s11b_tests` +7 (`killed_grenadier_mid_aim_saves_and_loads`, `boarding_grenadier_saves_and_loads`,
`grenade_aim_breaks_when_the_target_slot_turns_friendly`, `rifle_grenade_is_not_counted_as_a_panzerfaust`,
`a_howitzer_card_takes_a_fire_mission`, `light_mortars_reach_their_game_range_and_minimum`, `artillery_group_caps_two`);
content `s11b_content` +3 (`s11b_defs_load_with_the_cited_figures`, `howitzers_are_inf_guns_with_a_low_angle_table`,
`rifle_grenadiers_carry_three`); `test_neutral.py` +1. Changed: `rifle_grenadier_fires_at_seen_infantry` counts 9.
Neutral: `neutral.py target/s11b-A0-40 target/s11b-B1-40`: `skirmish40.json: same 40/40`; `... target/s11b-B2-40`:
`skirmish40.json: same 40/40` (Germany 19/40 as REF1). BASELINE untouched (it resolves the frozen S3c fixture).
Checks: release CLI rebuilt; `tools/check.sh quick 8d2731b`: fmt, clippy, neutral.py, the five ablate dry runs ok;
tests 1122 passed, 1 failed: `udp` `ipv6_loopback_handshake` (no IPv6 in this container; not skipped). The new cards
broke 6 roster/type-count tests (fixed in `b645588`: 20 vehicle defs, the artillery group is index 0, two appended
cards a side) and the canonical snapshot (re-blessed; only the new defs, rosters, squads and limit group differ).

## Build A (2026-11-19; commits `3edf0cc`, log commit after it)
Chunk 0: release CLI current; REF0 `target/s11b-base40` from the batch cache (base `8d2731b`), Germany 19/40.
Parse check: `frontline-cli targets docs/slices/targets/S11b.toml --from-json target/s11b-base40` loads
(21 pass, 1 miss T149d report, 4 key-missing, 598 skipped): no parse error, the file is unchanged.
Built (one code commit; the steps were written together, so layout, keys, X1, X3 and G share it: deviation
from "each its own commit"): SAVE 29 / PROTOCOL 15 (`ItemKind::RifleGrenade` 6, `ITEM_KINDS` 7), `Squad::rg`
(`RgState` with one field more than spec 3: `target`, the man aimed at, so the aim breaks when he dies or
leaves the band, spec 5), `PlayerState::s11b`, `kill_class::RIFLE_GRENADE` 11 (`COUNT` 12), `AiArty::cb_last`,
four `AiCounters`, `ItemSetup::stow_ticks`, `Purpose::RIFLE_GRENADE` 40; keys of spec 2 (`[items]` keys live
in `ItemRulesDef`, not `tuning.rs`: the `[items]` table is not a tuning table); X1; X3; G1-G4; three US defs.
Code map: `combat.rs` `fire_rifle_grenades` (called after the Panzerfaust loop in `fire_launchers`);
`game.rs` `RgState::for_type` (call-in and `bail.rs` crew, as `pzf`); low root `indirect::low_root`; issue
rule `setup.rs` (member item lowering, `rules.items.rifle_grenades == 0` skips the kind); `cb_last` set in
`counter_fire`, cleared in `arty_fire`, read in `cb_candidate` after the stale gate (`cb_alt_holds`); X1 in
`guns.rs` `gun_mobility` (Emplacing ends) and `combat.rs` `kill` (slot man lost); refill exclusion in
`supply.rs` `refill_men` and `items.rs` (legacy free refill); `base_class`: launcher + HE ammo = rifle grenade.
Rule choices: the aim breaks on the man's single-shot rule, the target dead or out of the band (not on
sight lost: a lone man is seen about 13 % of ticks at 100 m and would never be hit); spread = the weapon's
`spread_mrad` x training `spread_pct` (no movement or range factor); ttl from the solved flight x 1.25.
Aim error, spread 0 (unit integration of the sim's substeps): 60 m 2 mm (5 m up 10 mm), 120 m 2 mm (5 mm),
180 m 3 mm (1 mm); in the match a burst lands within 3 m of the aim point. Control: the flat
`gunnery::solution` aim misses by over 8 m at 150 m.
X1 (REF0 -> REF1): `s9_derived.gun_in_cover_pct` Germany 59.286 -> 86.458, USA 57.627 -> 87.179;
`s9.guns_emplaced` 140 -> 96 / 118 -> 78; `s9.gun_crew_lost` 92 -> 52 / 79 -> 67 (per match 2.3 -> 1.3,
1.975 -> 1.675). `neutral.py base40 A0-40`: "same 40/40" per match; the aggregate keys that differ are exactly
`s9.{guns_emplaced,gun_crew_lost}`, their `s8a.s9` copies, `s9_derived.{guns_emplaced_per_match,
gun_crew_lost_per_match,gun_in_cover_pct}` and `s5a.kills_by` 11 -> 12 items (tail 0, the new class).
REF1 holds all of stage A (not X1 alone): the stronger proof, and the base B and C need. Set 3
`target/s11b-A40` = default with `-o S11b-ablations/s11b_off.toml`: `neutral.py A0-40 A40` "same 40/40
(note: overrides 0 -> 1 items)". `cb_alternate` is not measured here (diagnosis 62.3 -> 47.3 %).
Tests: `s11b_tests` 17 (layout, load rejects 17 cases, X1, X3 x3, G x10), `s11b_content` 3.
Checks: `cargo build --release --workspace` ok (bridge included). `tools/check.sh quick 8d2731b`: fmt, clippy,
neutral.py, the five ablate dry runs (S11b's new) ok; tests 1112 passed, 1 failed: `udp`
`ipv6_loopback_handshake` ("Address family not supported": this container has no IPv6; not S11b code).

## Plan revision (2026-11-19, after the measured diagnosis and an independent review: no blocker, 6 major, 7 minor; all verified)
Diagnosis: X1 also splits `gun_crew_lost`; X2 dropped (`pot_hold` gone; owner question S11b-10 rewritten); X3 is the
`cb_last` flag behind `cb_alternate`, the round budget dropped (a budget on rounds ordered does not bound the fired share).
Review: 1 PROTOCOL 15 in the layout commit. 2 Build A owns the bridge's `intents.rs` and `thrown()`. 3 `buy_pending`:
gun buys subtract pending call-ins. 4 The howitzer has its own share `how_income_pct` 15 (minute 11 / 12 against 19.7 / 22
inside the gun's share; median match 18-19.4 min); T153's exposure is the share of matches of 14 minutes or more. 5 No
base refill for rifle grenades (crate-only by design); the AI refill rule became "no trip for grenades". 6 Set budgets
(A 3, B 2, C 3); A keeps every switch 0, so one set proves default and `s11b_off`. 7 The grenade aims with the low root;
test `rifle_grenade_lands_on_its_aim_point`. 8 kz. GrW 42 minimum 60 m, 60 mm 90 m. 9 FH Gr 38 priced as the complete
round, 19 kg, like the M1. 10 Grenade burst 96 / 48 rays; T150 at 600 m; `scatter_pct` in the item def. 11 The howitzer's
and the infantry gun's `spent` sums are apart. 12 Rifle-grenade figures as given (M17: Mk 2 body, 0.68 kg, 180 m;
Gewehr-Sprenggranate: 250 g, 250 m, 60 m/s, about half the burst), "owner to confirm"; every other unsourced figure in
S11b-13. Ablations: `cb_budget_33` removed, `how_share_10` added; `s11b_off` holds all five switches.

## Plan (2026-11-19)
Scope (spec 0, 1), sized for three Build stages. (1) A0: three fixes from the S11a2 measure, each behind a key or proven
neutral: infantry-gun emplacements leave the AT-gun cover ratio (X1), [first plan: a pot-hold key and a round budget; see
the revision above: X2 has no fix, X3 is the alternate rule]. (2) Field howitzers leFH 18 and 105 mm M2A1 as
`inf_gun` cards with no tow: base guns that emplace where they arrive and fire flat; the AI buys one as a second gun from
8:00 [revised: with its own 15 % income share] (`how`). (3) Light mortars kz. GrW 42 and 60 mm M2 in the AI's mortar slot (`light`).
(4) HE rifle grenades: a new item kind and a launcher mode on the rifleman, two US and one German grenadier a full squad,
three grenades each, their kills a new class inside the HE share (`rifle_grenades`). (5) The 75 mm pack howitzer as data in
no roster; the artillery limit group of 2.
Left out, with the reason (spec 1): tows, siting, displacement and crates (S11a3: tow AI and crate AI are a session each,
and a human-only tow card has no use before a presenter pass); HEAT and Pzgr rot; crate-only grenades and shells; the map
flag; neutral guns in the cap; `howitzer_pit`; the AT rifle grenade; authentic mortar ranges; sIG 33 and Nebelwerfer;
models and HUD (presenter pass).
What the howitzers do without tows: arrive limbered in the base, emplace in 90 s, cannot be pushed, re-lay in 25 s, take
observed missions through the observer team's radio, are heard and counter-fired like the infantry guns, and are refused
"masked" wherever the flat arc from the base does not clear the ridge (the design's own verdict for a 1 km map).
Decisions taken for the owner (all (A), `docs/TODO.md` S11b-1..13 as revised, "assumed 2026-11-19, owner asked not to be asked"): this
scope; no tow cards; the howitzer as a second AI gun inside one share; kz. GrW 42 at 181 MP; light mortar ranges 275 m and
370 m; grenadiers 2 and 1; plan values for the grenade; grenade kills inside the HE share; [revised: no counter-fire twice in a row; vehicle
displacement accepted]; T81 without infantry guns; the artillery group of 2.
Honest expectation: the howitzers will fire little from the base (T154 reports it); rifle grenades favour the USA (two
launchers against one) with Germany at 41.67 %, so `rg_off` is the first ablation to read; the A0 causes are read from the
code, and the diagnosis may replace them.
Stage split: A (chunk 1: A0 fixes, SAVE 29 layout, keys; chunk 2: the rifle-grenade mode; every S11b switch 0) -> B (chunk
1: howitzers, their shell, light mortars; chunk 2: grenadiers, the group, pack howitzer data; all cards weight 0, default =
REF1) -> C (chunk 1: JSON, howitzer rows, the howitzer buy, `how` on; chunk 2: light mortars, grenades on, rows `rg` and
`light`, three 40 sets). Fallbacks in spec 8.
