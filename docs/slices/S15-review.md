# S15 review (USSR), branch claude/project-thread-aj1tww at ab57b20, diff 96d4b4b..HEAD

Read-only review. No matches were run; evidence for the in-match findings is the stage's own set
`target/s15-D-pair40/pair_germany_ussr40.json` (per-faction counters) plus the code paths named.
Not re-run here: the three failing bridge tests (taken from the stage facts), Godot tests.

Summary of the ranking: no determinism, save-layout or Germany-USA neutrality defect found. The real
defects are in AI liveness and data: the Soviet artillery never fires and Soviet gun crews are left
behind (both from one data choice, the US6 as every gun's tow), and a rider job can orphan a squad.

## Findings

1. **HIGH. Soviet infantry guns and howitzers are bought and never fire (AI liveness, data + code).**
   `mods/ww2/defs/units/ussr_76mm_m1943_squad.toml:12`, `ussr_122mm_m30_squad.toml:12` (`tow = "ww2:ussr_us6_squad"`);
   `crates/sim/src/guns.rs:927-953` (`gun_arrives`: a card with a tow arrives `Hitched`);
   `crates/sim/src/ai/arty.rs:591-607` (`run_arty` A2 sends `Emplace` only when the state is `Limbered`);
   `crates/sim/src/ai/guns.rs:402-417` (`take_guns` drops a towing vehicle from every habit; only `at_gun` gets the unhitch job).
   Defect: German and US infantry-gun cards have no `tow` and arrive limbered; Build C deviation 2 gave the M1943 and M-30 a tow.
   No habit unhitches an infantry gun, so it stays hitched in the base for the whole match.
   Evidence (40 matches, Germany | USSR): `s11b.arty_emplaced` 74 | 1; `s11a1.arty_missions` 153 | 0; `gun_rounds` 428 | 2;
   `how_rounds` 147 | 2; `fo_buys` 40 | 1; `matches_gun_fired` 29 | 1. The USSR paid for 37 M1943 and 18 M-30 (each with a US6) for nothing.
   This also biases the reported 26:14 and the lengths (T212c, T214, T216).
   Fix (not tuning): remove `tow` from the two cards (as the leIG 18 / leFH 18 cards, which the plan cites as the model), or make
   `run_arty` send `Unhitch` for a `Hitched` gun. Then add a test that drives the real habit: a USSR AI with an M1943 reaches `Emplaced`.

2. **HIGH. Soviet AT-gun crews are left in the base; the gun stands unmanned at its site and turns neutral.**
   `mods/ww2/defs/vehicles/ussr_us6.toml` (role `truck`, `[tow]` and `[supply]`, no `[transport]`);
   `crates/sim/src/guns.rs:956-986` (`hitch_now`: "its crew aboard if it fits"); `crates/sim/src/guns.rs:1470-1537` (`gun_abandon`:
   no own man within `hold_m` 15 for `abandon_s` 10 turns it neutral); `mods/ww2/defs/rules/skirmish.toml:843-845`.
   Defect: the German tow (Sd.Kfz. 11, `tractor`, 8 seats) and the US tow (M3A1) carry the crew. The US6 has no seats, so the
   5 to 7 men walk across the map after the gun was unhitched at the front.
   Evidence: `s9.guns_neutral` 0 | 44; `guns_recrewed` 0 | 38; the loss table shows 44 USSR "abandoned_none" (cause `AB_GUN` 7 has no
   name in `sdk/crates/cli/src/skirmish.rs:2142` `ABANDON_CAUSES`), which inflates "tanks lost 144".
   Fix: a tow def that seats the crew (a `tractor`-role US6 variant with `[transport]`, the Sd.Kfz. 11 pattern), and a content check
   that a gun card's tow seats its crew squad. Add "gun" as the 8th name in `ABANDON_CAUSES`.

3. **MEDIUM. Every gun's US6 becomes a supply truck, and Soviet AT guns can never be re-sited.**
   `crates/sim/src/ai/truck.rs:147-152` (`truck_jobs` takes every `role == Truck` vehicle once it tows nothing);
   `crates/sim/src/ai/guns.rs:839` (`gun_resite` refuses a tow with `role == Truck`: "never a truck on its supply round").
   Evidence: `s9.gun_resites` 21 | 0; `s8b.truck_loaded_kg` 18505 | 51540; MP on ammo 709 | 1102 per match; `tows_lost` 6 | 16;
   the USSR bought 9 US6 cards but got about 170 with its guns.
   Fix: same as finding 2 (a tractor-role tow is not a truck, so both problems go), or let `gun_resite` take the gun's own `job.tow`
   and keep `truck_jobs` off a vehicle that some gun job names as its tow.

4. **MEDIUM. A rider job that ends while the squad still walks to the tank leaves the squad with `Order::Embark` and no owner.**
   `crates/sim/src/ai/riders.rs:160-168` (Board: `aboard == 0 && age >= stall_s` ends as a stall; also the Board deadline at 150-153
   and "tank lost" at 135-140), no `Stop` for the squad. `crates/sim/src/ai.rs:1522` keeps every squad with `Order::Embark` out of
   the free list; `crates/sim/src/transport.rs:789-797` (`embark_upkeep`) makes it follow the door.
   Failure: the tank goes back to the tank habits and drives to the front; the squad follows it on foot under no habit, boards when it
   catches up and rides with no job (which also blocks a new job on that tank, `riders.rs:302`). The S8a job stops its squad in this
   case (`ai/transport.rs:522-528`). One of the two jobs in the 40 set ended as a stall.
   Fix: on every end where the squad's order is `Embark { vehicle: j.tank }`, emit `CommandKind::Stop { squad }`. Extend
   `rider_job_ends_on_stall` and `rider_job_ends_when_tank_dies` to assert the squad's order is not `Embark` afterwards.

5. **MEDIUM. Rider jobs can hardly start: 8 deck slots against 9-man sections (plan data inconsistent, not a code defect).**
   `mods/ww2/defs/vehicles/ussr_t34_85.toml:201` and `ussr_is2.toml:187` (`capacity = 8`); `crates/sim/src/ai/riders.rs:319`
   (`sq.members.len() > free` skips); `riders.rs:86-94` (only `role == Tank` without heavy overwatch: the SU-85's 6 slots and the IS-2
   are never used by the AI). Plan 6.1 tags the 9-man SMG section `riders`, plan 6.4 gives 8 slots.
   Mechanism for "2 jobs in 40": only the 6-man card (8 bought) or a section that lost a man but is still over 50 % fits, and only 16
   T-34-85 were bought. `MAX_PASSENGERS` is 12, so this is the `capacity` key and two more `[[rider_slots]]`.
   Owner question with a recommendation: set 10 slots on the T-34-85 and IS-2 (a consistency fix), or leave it for the balance pass.

6. **MEDIUM. Three godot-bridge tests fail after the merge; the right expectations.**
   `crates/godot-bridge/src/match_setup.rs:353-378, 386-430, 433-439`. The shipped rules now enable the USSR, so:
   - `menu_choice_without_opponent_is_base`: keep "no opponent = Germany against the USA, same setup as naming `usa`"; expect
     `menu_factions` = germany, usa, ussr; `menu_opponents` germany = usa, ussr; usa = germany; ussr = germany. Control: `faction: "usa",
     opponent: Some("ussr")` is refused (the pair is off without `free`), not `germany` against `ussr`.
   - `menu_options_follow_the_matchups_table`: assert the three-faction table on the shipped rules directly (no mutation). Control:
     remove `ww2:ussr` and its pair from the loaded table and expect two factions and one opponent each (the hidden picker).
   - `rejects_bad_choices`: `faction: "ussr"` now resolves (against Germany); use an id that no rules enable (e.g. `"japan"`).

7. **MEDIUM. Tests that prove little.**
   - `crates/sim/src/s15_ai_tests.rs:312` `ai_habits_find_ussr_cards` re-states each habit's pick rule inside the test and checks a
     card with the tag exists; it calls no habit. It passed while findings 1 to 3 were live. Replace the gun, howitzer, tow and
     observer rows with driven habits (`drive_*` hooks exist for trucks and fire missions).
   - `crates/sim/src/s15_tests.rs:1414-1479` `s15_scene_is_deterministic_across_threads_and_save_load` says it covers captured bombs
     and bayonets, but no weapon in the scene has a foreign feed and no strike is asserted: only boards, exits and plate hits are.
     Plan 9 wants the hash check "with foreign bombs carried"; add a mortar man with `Kit::foreign` > 0 firing one, and a strike.
   - `crates/godot-bridge/src/match_frame.rs:1532` `frame_carries_riders_and_plates` only checks vector lengths and all-default
     values; a swapped or missing copy of `riding` / `plate` passes. Use a world with a rider and a plated man.
   - No test fires the real ROKS-3 at a man (`roks3_pair_off_is_no_flame_man` checks the `when` pair only), so T213c's 0 hits has no
     deterministic backstop. Add one (operator 20 m from a standing squad: `flame_hits` > 0).
   - Plan 5.1 names `riders_off_is_base`; it does not exist (W2's control covers `Embark` refused, not "the tank is today's tank").

8. **LOW. Load validation is weaker than its comments say (no crash or desync found).**
   - `crates/sim/src/s15.rs:132-148` `validate_s15`: "foreign rounds only on a man whose weapon has a foreign feed" is checked as
     `foreign <= ` one global cap for any soldier. With the switch on, a save may give a German mortar man foreign rounds;
     `supply::mortar_bombs` (`supply.rs:473`) then counts bombs `take_bomb` cannot take (a mission that ends at once).
   - `crates/sim/src/vehicle.rs:710-720`: `rider_off` is accepted on any vehicle with passenger slots (a half-track too), where
     `riders_jump` never counts it down: the `RIDERS_OFF` flag then stays set for good. Check `transport.exposed`.
   - `crates/sim/src/s15.rs:169-193` `AiRiders::valid`: the doc says "own vehicles"; ownership of the tank is not checked (the next
     plan ends the job as tank lost, so harmless).
   - `sdk/crates/content/src/setup.rs` check: a setup with `foreign_ammo` 0 but a `foreign_feed`, or `riders.enabled` 0 but exposed
     slots, loads and partly runs (the sim reads the def part, not the switch, for X1-X2 and `Embark`). N2 holds only for built setups.

9. **LOW. `take_foreign` (and the older `take_bomb`) change a crew giver's kit without marking his hash chunk.**
   `crates/sim/src/supply.rs:1099-1116`, caller `crates/sim/src/combat.rs:1231-1261` (marks the gunner's chunk only).
   A crew that spans a 256-id chunk border would leave a stale incremental hash for one tick class. Same pattern as before S15;
   mark the giver's chunk in both.

10. **LOW. Two behaviour changes outside any switch (both bug fixes; Germany-USA 40 sets unchanged).**
    `crates/sim/src/flame.rs:606-612` (a flame landing off the map makes no patch) and `crates/sim/src/spotting.rs:71-76` (`pct_of`
    saturates instead of wrapping above about 327 m). Only the second is in the log. Say both in the log; no action otherwise.

11. **LOW. R4 check is the box centre against hull boxes only.** `sdk/crates/content/src/vehicle.rs:2715-2728` and `4470-4483`.
    The plan says "no rider box inside a hull or turret box"; a rider box that overlaps a hull box with its centre outside, or sits in
    the turret box, passes. Logged as deviation 2; keep or tighten to box overlap.

12. **LOW. Plan 9's `hostile_to` helper was not made.** `crates/sim/src/s15.rs:363` (`burst_riders`: `owner == b.player`) is a new
    hostility test written inline; the PR 11 merge has one more site to find.

13. **LOW. Presenter and docs.**
    - `godot/docs/match_state.md` "S15" (and the comment at `match_frame.rs:1527`) say the switches are "all 0 in the shipped rules";
      Build D set them to 1 and enabled the USSR. The keys themselves match the bridge (`deck`, `riders_off`, `riding`, `ride_slot`,
      `plate`, `rules.vehicle_types[t].deck`, `match_options`).
    - `crates/godot-bridge/src/net_session.rs:321-326`: before `host` / `join` loads the defs, `set_choice({faction: "ussr"})` is
      dropped without a word (the fallback list is the old two).
    - JSON `s15.rider_job_end_0..4` are unnamed while `s15_derived.rider_job_ends` has names.

14. **LOW. Data notes (spot check; nothing found that contradicts the cited sources).**
    Checked: BR-365 / BR-365P, BR-471 (152/142/133/122 at 500-2000 m), BR-350A (82/75/67/60/53), BR-350P, BR-271 / BR-271P, BR-240;
    T-34-85 (45 at 60 / 53, turret 90/75/52, 55 rounds), IS-2 (120 at 60, 100 at 30, sides 90, turret 100/90/90, 28 rounds),
    SU-85 (45 at 50), gun masses 625 / 1250 / 1116 / 600 / 2450 kg. Points to note:
    - BR-471 has no row under 500 m (the table clamps); add 100 m.
    - BR-471 (sharp-nosed, uncapped) and all Soviet AP are `slope_class = "apcbc"`; the enum has only that and `apcr`, so no error.
    - `ussr_ppsh41` rate 600 for 1,250 cyclic, while the MP 40 is 400 for 500 (log B deviation 6): a different ratio, a data question.
    - `ussr_roks3` equals the M1A1 flamethrower but for load 96, range 35 and 26 m/s; nothing in it explains 0 hits.

## Rules that never or rarely fired: code defect or not

- **ROKS-3, 0 hits from 46 units: not an S15 code defect.** The def is the US flamethrower with other numbers; the S14 flame habit is
  rare for everyone (flame jobs 7 Germany, 6 USSR in 40; Germany 3 hits from 147, about 2 %). 46 units is about four bursts.
- **Bayonet, 0 strikes: not a code defect.** `flame.rs:1352-1447`: a strike starts only when the man's weapon cannot fire within
  the wind-up and an enemy is within reach plus a body radius (about 2 m). No AI habit closes to that range (Germany: 1 strike in 40).
- **Rider jobs, 2 in 40: data** (finding 5). **Riders all off at the first shot: the plan's R3c**, key `off_on_fire`.
- **Captured bombs, 0: by design** (plan 11: the AI does not seek crates; W3 is the proof). The sim path reads correctly.
- **Plates, 8 stops from 48 meetings: works**; prone men have no plate (`prone` 0) and rifles and MGs go through.

## The log's open owner questions

1. T212c (one match at 41.5 min): **question for the owner, after the Fix.** Findings 1 to 3 change the USSR's strength and so the
   lengths; rerun the pair 40 first. The match had a winner inside the 45 min run.
2. T213c / T213d: **no code fix.** Add the ROKS-3 test of finding 7; ask the owner to make both rows reports (W4 and the new test
   prove the rules) or to order a close-assault habit later.
3. Riders do not fit: **question for the owner with a recommended data fix** (10 slots), finding 5.
4. Cards never bought: ZiS-2 / ZiS-3 (the AT-gun habit takes the first `at_gun` card of the roster, the weakest gun): **owner
   question** (it is a buy choice). SU-76M, medics, bearers, HQ: leave for the balance pass. M1943, M-30 and the observer are bought
   or skipped because of finding 1: **code/data fix now.**
5. `second_bases` 439: **no action**, a report for the balance pass.
The failing bridge tests (the log's check note): **code fix now**, finding 6.
