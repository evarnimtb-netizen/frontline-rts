# S11b Build B: content (howitzers, light mortars, rifle grenadiers, pack howitzer data, artillery group); every card weight 0

Read first: `S11b.md` 4 (C1-C7 with their cited lines), 8 (the cut order); the log's top block (Build A's ids, def-check limits, REF1);
`S11a1-log.md` "Build B" (how the leIG 18 and M3 defs were made, what the def check refused). **2 chunks**, about 30 self-counted turns each
(60-90 minutes in all). No sim code, no layout, no version change, no AI change: every new card has `ai_weight = 0`, the S11b switches stay 0,
so the default 40 set must equal REF1 (`target/s11b-A0-40`) match for match. If a def needs a sim or loader change to load, stop that item,
log it and go on (the exceptions: a def-check range the design's figure exceeds, which is widened in `content/src/vehicle.rs` or `defs.rs`
with a test; a roster or type-count test that lists cards).
Work rules as in Build A (4-minute rule, `tools/run.sh`, `BATCH_JOBS=4 tools/batch.sh`, commit after every green step and only your files,
index lock: wait and retry, no push, no tick times, no model names). `<BASE>` is `8d2731b`. **40 sets: one per chunk end (steps 4 and 8),
no more; another only to bisect a set that differs** (halve the chunk's commits with scratch builds, name the def that moved a match).
**Figures:** only the ones spec 4 gives. A design figure is commented with its line (D / GA / RE); a rifle-grenade figure "owner to
confirm, S11b-7"; a value the design does not hold is copied or derived as spec 4 says and commented "verify: plan value, S11b-13" with
the def or formula it came from; never invent a number, never take one from memory.

Files owned: new files under `mods/ww2/defs/{vehicles,weapons,ammo,units,cartridges,items}/` (list them in the log);
`mods/ww2/defs/units/usa_rifle_squad.toml`, `ger_grenadiergruppe.toml` (C5 only), the four gun cards (`limit_group` only);
`mods/ww2/defs/factions/{germany,usa}.toml` (new cards appended at the END of each roster, with a comment as the S11a1 block);
`mods/ww2/defs/rules/skirmish.toml` (`[limit_groups]` only); new `sdk/crates/content/tests/s11b_content.rs`; the roster and type-count tests
that fail on the new cards; `crates/sim/src/s11b_tests.rs` (the `artillery_group_caps_two` test only); `docs/modding/{ammo.md,variants.md,
fire-missions.md}` (a table row per new def); the BASELINE files and the canonical snapshot; `docs/slices/S11b-log.md`.

Start from: `vehicles/ger_leig18.toml`, `vehicles/usa_105mm_m3.toml`, `weapons/leig18.toml`, `weapons/m3_105mm.toml`, the two `*_crew.toml`
and `*_squad.toml` of S11a1, `ammo/105mm_m1_he.toml` (its header states the scaling rule), `ammo/75mm_sprgr34.toml`;
`units/ger_mortar_team.toml`, `units/usa_mortar_team.toml`, `weapons/grw34.toml`, `weapons/m1_mortar.toml`, `cartridges/8cm_wgr34.toml`,
`cartridges/81mm_m43a1.toml`; `items/stielhandgranate.toml`, Build A's three rifle-grenade files; `content/src/vehicle.rs`
(`low_angle_top`, the mobility ranges about 1405-1420 and 2310-2315), `content/src/modset.rs` 1701-1709 (a `limit_group` needs its rules
entry); `sdk/crates/content/tests/s11a1_content.rs` as the test pattern.

## Chunk 1: the two howitzers and their shell, the two light mortars
1. C3 `ammo/105mm_fhgr38.toml`; C1 leFH 18 (vehicle, weapon, crew of 6, card); C2 M2A1 (vehicle, weapon, crew of 8, card; it fires the
   existing `ww2:105mm_m1_he`). Both `role = "inf_gun"`, `crew_model = "served"`, no tow, card tags `["vehicle", "field_howitzer"]`,
   `ai_weight = 0`, appended to the rosters. "No push": try `speed_cm_s = 0`; if the check refuses it, the smallest value it allows with
   `push_men` above the crew size; log which. Service slots: the S11a1 gun's five or six plus `ammo` slots to the crew size, capsules clear
   of the boxes (the def check says so). `supply` of the FH Gr 38 is the complete round, 19000 g, as the M1 HE's (spec C3: one basis for
   both shells). Tests `s11b_defs_load_with_the_cited_figures`, `howitzers_are_inf_guns_with_a_low_angle_table`
   (control: `low_angle` removed from a copy: the leFH 18 loads, the M2A1 with +65 is refused, as review fix A-1 of S11a1 built it). Commit.
2. One sim check that needs no new code, as a test in `s11b_content.rs` or the sim's test file if a `World` is needed:
   `a_howitzer_card_takes_a_fire_mission` (spawn the card, emplace, an observed mission is accepted and a round is fired; control
   `gun_missions` 0). If it fails, the role is not enough: stop, log what the sim asked for, leave the cards in (weight 0).
3. C4 light mortars: weapons `kz_grw42`, `m2_60mm`, cartridge `60mm_m49a2`, cards `ger_kz_grw42_team`, `usa_60mm_mortar_squad` (member
   slots and traits as the heavy teams, one bearer fewer for Germany's four men), tags `["mortar", "light_mortar"]`, `ai_weight = 0`;
   `min_range_m` 60 (kz. GrW 42: the grw34 band's 75 m/s reaches no nearer) and 90 (60 mm). Test
   `light_mortars_reach_their_game_range_and_minimum` (the charge solver reaches 275 m / 370 m and refuses 20 m more, reaches the minimum
   and refuses 10 m less; control: the heavy tube reaches 600). Commit.
4. Neutrality (the chunk's one set): default 40 set into `target/s11b-B1-40/`; `python3 -I tools/neutral.py target/s11b-A0-40 target/s11b-B1-40`: `same 40/40`.
   If it differs, the first suspect is a list the new cards entered (a buy that ignores `ai_weight`, a roster-order tie-break): find the
   card, fix it in the data, log it.
**STOP POINT 1.** `check.sh quick 8d2731b` green; commit; rewrite the log's top block; return if more than 45 self-counted turns are used.

## Chunk 2: grenadiers, the group, the pack howitzer (cut from the end)
5. C5 (skip if Build A left G unbuilt): `items/schiessbecher_sprgr.toml`, `weapons/schiessbecher.toml`, `ammo/gsprgr30.toml` with the
   figures of spec C5 (250 g, 250 m, 60 m/s; `[he]` = the Mk 2 block with `frag_count` 48 and blast and fragment reach x 0.8: NOT the
   stick grenade's block); the `rifle_grenadier` member group in the two squad defs, three grenades each, men's order unchanged. Test
   `rifle_grenadiers_carry_three` (with `rifle_grenades` 1; control 0: none), and which `extends` cards inherited the group. If step 8's
   set shows the member split moving a match with the switch 0, undo the split and give the grenades to the existing `rifleman` group's
   first two men by whatever the member schema allows; if it allows nothing, C5 is cut and logged.
6. C7: `[limit_groups] "ww2:artillery" = 2`, `limit_group = "ww2:artillery"` on the leIG 18, M3, leFH 18 and M2A1 cards; sim test
   `artillery_group_caps_two` (two gun cards alive: the third call-in is refused; control: the group entry removed).
7. C6 pack howitzer data (vehicle, weapon, crew; in no roster; one assert in `s11b_defs_load_with_the_cited_figures`). Skip it first if the
   chunk is short.
8. Neutrality (the chunk's one set, after steps 5-7): default 40 set into `target/s11b-B2-40/`; `neutral.py` against
   `target/s11b-A0-40`: `same 40/40`. If it differs, bisect steps 5, 6, 7 (one scratch set each at most) and fix the data. No sanity run
   with grenades on: Build C's first default set reads it.
**STOP POINT 2.** `check.sh quick 8d2731b` green (run it last); BASELINE and snapshot refreshed; commit; rewrite the log's top block.

## The stage must log (top block rewritten, at most 30 lines)
Commits; the new def files; per def the figures taken with their design line and every "verify" value with the def it was copied from; what
the def check refused and how it was met; the `neutral.py` lines for B1 and B2; test count from `check.sh`; what Build C
needs: the card ids and tags, the howitzers' vehicle ids (for `kills_by_vehicle`), each mortar card's `ai_weight` default as the loader
resolves it for the heavy teams, how a fortify row spawns a howitzer and gets an observed mission accepted, the scene of
`a_howitzer_card_takes_a_fire_mission`. Not done items by rule id (C5, C6, C7).
Cut order if time runs out (spec 8): C6, then C7, then C5; C1-C4 with a green neutral run are the stage's result.
