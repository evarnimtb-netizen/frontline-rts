# Tuning the game

Every gameplay number lives in `mods/ww2` TOML. Edit a value, then test it
in one command (CLI) or with F5 (Godot debug build). No Rust build is
needed.

## Where values live

| What | File | Table |
|---|---|---|
| Economy, tickets, capture | `defs/rules/skirmish.toml` | `[economy]`, `[tickets_drain]`, `[capture]` |
| Walking, paths, formations, slopes | same | `[movement]`, `[movement.formation]` |
| Trench and AT-ditch shapes, digging; HE blowing parapet spoil away (S3b `spoil_crater_pct`) | same | `[dig.profiles.*]`, `[dig.common]` |
| Spotting, accuracy, suppression thresholds | same | `[combat]` |
| Hit capsules, posture points | same | `[combat.body]` |
| Mortar laying and flight (S5b: `flight_milli` / `min_flight_s` only for a mortar without `[indirect]`) | same | `[combat.mortar]` |
| HE bursts and craters | same | `[combat.blast]` |
| Suppression details; S3b duck state of men in trenches (`duck_points`, `duck_release_points`, `duck_decay_pct`) | same | `[combat.suppression]` |
| S3b Suppress order (area fire): aim height, line length, line-of-fire probe, a tank's HE interval, reserve and aim | same | `[combat.suppress]` |
| Cover on arrival | same | `[cover]` |
| Thrown items | same | `[items]`, `[items.throw]` |
| Vehicle rules: unlock, fire, rack, repair | same | `[vehicles]` |
| Ammo fire, rack outcome (S3b) | same | `[vehicles.damage]` |
| S3c bail-out: triggers (`ammo_fire_bail`, `engine_fire_bail_s`, `crew_below` (replaces S3b's `crew_ko_below`), `useless_bail`, `under_fire_s`), exits (`wounded_hp`, `wounded_exit_pct`, `exit_clear_cm`, `dismount_ms`, `bail_suppression`), `abandoned_cap`, `crew_dismiss_s` | same | `[vehicles.bail]` |
| S3c re-crewing an abandoned vehicle: `reach_m`, `board_s`, `min_men`, `any_infantry`, `capture`, untrained penalties (`untrained_reload_pct`, `untrained_traverse_pct`, `untrained_spread_pct`) | same | `[vehicles.recrew]` |
| S9 AI guns (`after_s`, `income_max_pct` set aside for the next gun, `reserve_mp`, `max_guns`, `site_min_m`, `site_max_m`, `site_rings`, `site_bearings`, `site_forward_m`, `site_enemy_point_m`, `site_view_min_pct`, `corridor_m`, `corridor_step_m`, `corridor_clear_m`, `corridor_paths`, `site_apart_m`, `approach_m`, `los_rays`, `enemy_standoff_m`, `arrive_m`, `max_fails`, `repath_s`, `go_max_s`, `contact_m`, `engage_still_m`, `relay_gap_s`, `pit_engineer_m`, `recrew_m`, `recrew_gap_s`), ambush against infantry (`[vehicles.guns] ambush_infantry_m`) | `defs/rules/skirmish.toml` | `[ai.guns]`, `[vehicles.guns]` (absent: starting values) |
| S8b AI recon cars (`after_s`, `second_after_s`, `income_max_pct`, `view_back_m`, `at_threat_m`, `backoff_m`) and supply trucks (`min_vehicles`, `hq_wait_s`, `income_max_pct`, `load_shells_kg`, `load_wait_s`, `min_load_pct`, `follow_m`, `los_rays`, `return_below_pct`, `flee_m`, `arrive_m`, `max_fails`, `repath_s`) | `defs/rules/skirmish.toml` | `[ai.recon]`, `[ai.truck]` (absent: starting values) |
| S10a-ai fortification AI (`docs/slices/S10a-ai.md` 3; `docs/modding/fortifications.md` "The AI"): switches `works`, `wire`, `breach`, `cards` (1 on, 0 off; the ablations) and, under `works`, `nest`, `dugout`, `roof`, `sandbags` (one work kind each); buying and builders (`builders_max`, `works_per_point`, `build_threat_max`, `builder_reach_m`); materials (`load_min_kg`, `materials_max_pct`, `truck_material_kg`, `material_wait_s`); sites (`nest_offset_m`, `wire_out_m`, `wire_len_m`, `dugout_back_m`, `gun_hold_s`, `gun_wall_m`); shelling and shelter (`shelled_pct`, `shelled_clear_m`, `shelter_s`, `shelter_clear_m`); wire sight and breaching (`wire_seen_m`, `wire_rays`, `breach_near_m`, `recheck_s`, `stage_m`, `smoke_frac_pct`, `suppress_lead_s`, `cut_fire_m`, `place_wait_s`, `breach_tries`, `breach_max_s`, `gap_clear_m`, `breach_cards_max`) | `defs/rules/skirmish.toml` | `[ai.fort]` (absent: starting values) |
| S10b-ai obstacle AI (`docs/slices/S10b-ai.md` 2; `docs/modding/fortifications.md` "AI (S10b-ai)"): switches `mines`, `at_mines` (under `mines`), `obstacles`, `bunker`, `sweep`, `demolish` (1 on, 0 off; the ablations); jobs and budget (`jobs_max` inside `[ai.fort] builders_max`, `materials_max_pct` own share of the income earned); minefield places (`ap_beyond_wire_m`, `ap_out_m`, `at_out_m`, `fields_per_point`, `at_fields_max`); obstacle places (`obst_gun_m`, `obst_work_m`, `hedgehogs_row`, `hedgehog_step_m`, `own_route_m`: no obstacle this close to an own vehicle route, 0 = rule off); `bunkers_max`; sweeping (`suspect_s`, `sweep_near_m`, `sweep_point_m`, `sweep_reach_m`); demolition (`demolish_pieces_max` at most 3, `demolish_near_m`); knowing enemy obstacles (`obst_seen_m`); job liveness (`job_max_s`, `refused_s`, `refused_m`, `recheck_s`) | `defs/rules/skirmish.toml` | `[ai.obstacles]` (absent: starting values) |
| S10b-ai Build E (2026-11-14 owner answers; `docs/slices/S10b-ai-build-E.md`): `bunker_assign` (0/1, default 1: a wanted bunker with no builder gets a free `build_heavy`, else `build_structure`, squad sent to its point); `bunker_truck_first` (0/1, default 1: once per bunker job the 250 kg material run may take a truck from tank service, which returns to it after the drop); `barrier_job_max_s` (1..=36000 s, default 1200: the job limit of the log barrier job only, every other job keeps `job_max_s`); `lane_file` (0/1, default 1: attackers walk a swept lane in single file on its centre line); `at_rows` (1..=2, default 2: the AI's AT field gets a second row, one step toward the point and half a step along, laid as its own job once row 1 is laid and only if the field limit and the S10b budget pay for the whole row). Off values (ablations `*_off.toml`, `all_off.toml`): 0, 0, 900, 0, 1 | `defs/rules/skirmish.toml` | `[ai.obstacles]` (absent: starting values) |
| B1-4 builder holds (`docs/slices/B1-log.md`): `hold_max` (0..=16, default 2: at most this many own points hold a builder for their wanted work, the fix F15 assign bonus that keeps a free engineer at a point whose work it could start; points ascending; inside `builders_max`, so with `builders_max` 2 the default is the old behaviour). It caps only the holds: a fit engineer assigned to the point for other reasons still starts the job | `defs/rules/skirmish.toml` | `[ai.fort]` (absent: starting values) |
| B1-5 build gates (`docs/slices/B1-log.md`): `build_after_s` (0..=3600 s, default 0 = off: no fort hold and no new fort job before this much match time) and `build_min_squads` (0..=64, default 0 = off: none while the side has fewer foot squads with a man alive; counted: every own squad with no vehicle that is not a crew, so rifle, MG, AT, mortar, engineer and other squads, those on a job included). Both gate every new job `fort_start` makes: S10a point works and wire, the HQ roof, walls at guns, and the S10b point fields, obstacles, bunker and log barriers (the same builder squads); trench digging, sweeps, demolitions, material loads and the fortify-test hook are not gated. A running job is never cancelled. Same rule for both sides | `defs/rules/skirmish.toml` | `[ai.fort]` (absent: starting values) |
| B1-0 demolition hand-over (owner S10b-ai-E3, 2026-11-15; `docs/slices/B1-log.md`): `demolish_handover` (0/1, default 1: a demolition job whose squad has no satchel for its next piece passes, with its pieces, done count and deadline, to another free own `demolish` squad with a satchel within `sweep_reach_m` of the vehicle, nearest first; none: the job fails as before; a new job prefers a squad with a satchel). Off value (ablation `demolish_handover_off.toml`, `all_off.toml`): 0 | `defs/rules/skirmish.toml` | `[ai.obstacles]` (absent: starting values) |
| B1-0 bunker yields the job slot (owner S10b-ai-E1, 2026-11-15; `docs/slices/B1-log.md`): `bunker_yields` (0/1, default 1: a bunker job whose heavy step waits for material its own squad does not carry, in Watch, does not count toward `jobs_max`, so one other S10b job may start; `[ai.fort] builders_max` and the S10b budget still apply; the bunker goes on when the material comes). Off value (ablation `bunker_yields_off.toml`, `all_off.toml`): 0 | `defs/rules/skirmish.toml` | `[ai.obstacles]` (absent: starting values) |
| S10c-ai defences at the front (`docs/slices/S10c-ai.md` 2): switches `front` (F1-F9: hasty AP field, wire and AT field on the enemy approach lanes of the front points, no trench, nest or wire first), `mine_assign` (C2: a wanted front work with no fit squad at its point takes one free builder from anywhere, nearest the site first, independent of the point assignment; default 1 since S10c-ai Build C chunk 1), `belt` (C4: once a front point's AT row 1 is laid, a row of `belt_pieces` blocking pieces `hedgehog_step_m` apart across the vehicle lane `belt_gap_m` beyond the field, enemy side: hedgehogs one at a time with a `build_heavy` squad, else one log barrier line of the same length; one belt per lane; it counts as a front job under `front_slots`; the frame is read from the AT row 1 line saved in the point's lane when the row is first sited (S10c-ai fix pass 3; before: from the live AT mines, so it moved when one went off), the side from the standing pieces), all 0/1, default 1 (`front` since S10c-ai Build B chunk 2, `mine_assign` since Build C chunk 1, `belt` since Build C chunk 2; off value 0); `front_points` (0..=8, default 2: own points nearest the enemy base), `front_out_m` (0..=500 m, 50: the front wire out along the foot lane), `front_wire_len_m` (0..=500 m, 20), `front_site_threat_m` (0..=500 m, 50: a front job neither starts nor goes on with a known enemy this close to its site), `front_fields_max` (0..=16, 2: front AP fields a player), `front_slots` (0..=16, 1: front jobs at once, C8: FrontAp, FrontWire, an AT row at a front point, a belt job; no new front job starts while this many run, so a second builder slot stays for the home works), `front_home` (0/1, default 0, orchestrator 2026-11-16: with `front` on, 0 makes the home point, the map point nearest the own base, a front point only while the enemy owns more than half of the map's capture points (Build C; before, the front points are the other own points only, so the opening builds the home works as with `front` off); 1 = plain F1, the home point counts like any other), `belt_pieces` (0..=20, 5), `belt_gap_m` (0..=500 m, 10: beyond the AT field), `belt_detour_pct` (0..=1000 %, 130, C5: before a belt starts, the own route vehicle's path base -> point -> way out is searched with the belt's nodes blocked; the belt is taken when that path exists and is at most this share of the path without it, else on the other side of the lane once, else refused; 0 = rule off), `mine_assign_reach_m` (0..=2000 m, default 450, S10c-ai fix pass 3, orchestrator 2026-11-16: `mine_assign` pulls a free builder only within this of the site; sweeps and demolitions keep `sweep_reach_m`; 450 of 450 / 600 / 800 measured, the rule: the largest whose front builders lost stays at or under 0.65 a match for both factions, else 450), `builder_cards_min` (0..=8, default 2 since S10c-ai Build C chunk 2 (orchestrator 2026-11-16, owner question S10c-2 proposed as (B)); 0 = off, the `builders0` ablation: the builder-card floor; with N > 0, fort habits on, an own point and fewer than N own living builder squads (`place_mines` and `build_structure`, no `breach`; pending buys count) the next buy is the faction's cheapest such card the AI buys, before the weighted pick, if ready, affordable and inside the pop cap). Off values (`S10c-ai-ablations/c_off.toml`): the three switches 0, `builder_cards_min` 0, `jobs_max` 1, `front_home` 0 (`mine_assign_reach_m` listed at its default; unread with `mine_assign` 0). C1: `[ai.obstacles] jobs_max` default 2 since Build C chunk 1 (was 1; ablation `jobs1.toml`). With `front` 0 only `front_points` is read (for the `front_plans` counters, without the home rule) | `defs/rules/skirmish.toml` | `[ai.obstacles]` (absent: starting values) |
| S10b-ai paths round known enemy mines: `mine_paths` (switch), `mine_path_cost_pct` (A* cost of a priced step, %), `mine_band_foot_cm` (a foot step this close to a known enemy AP mine is priced), `mine_band_vehicle_cm` (a vehicle step this close to a known enemy AT mine). Only mines the player's side knows; own mines are never priced | same | `[fortify]` |
| S10b-ai F8 fix: `refusal_memory` (switch). With 1 a truck that gives a goal up (an HQ, a drop spot it cannot reach) remembers it as a refused goal and never orders it again within `[ai.obstacles] refused_m`; the memory lasts until an own truck finishes a load, not `refused_s` (`refused_s` 0 still turns the check off). 0: the S8b behaviour (the goal comes back about every 5 minutes) | same | `[ai.truck]` |
| S8b rides that pay: `pick_m`, `meet_ahead_m`, `meet_stand_m`, `ride_lock_s`, `drop_short_m`, `unload_rolling`, `counted_ride_m` | same | `[ai.transport]` |
| S8a infantry carriers: `unlock_s` (transport unlock floor, never later than `[vehicles] unlock_s`), `stun_ms`, `board_reach_cm`, `move_block_cm_s` | same | `[vehicles.transport]` (absent in older rules: 120, 2000, 300, 100) |
| S3c a vehicle's crew squad and hatches (`crew_squad`; `[[hatches]]` `seats`, `frame`, `pos` cm, `exit_ms`); the crew squad defs (role `crew`, never in a roster) and their pistols (`class = "pistol"`) | `defs/vehicles/`, `defs/units/*_tank_crew.toml`, `defs/weapons/p38.toml`, `m1911a1.toml` | the def's own file |
| Driving, yielding, ground pose | same | `[vehicles.driving]` |
| Vehicle path costs | same | `[vehicles.path]` |
| Escort file behind a vehicle | same | `[vehicles.escort]` |
| Coax cone, spall reach; S3b: HE group radius, lead estimation error, when Auto loads APCR; S8a: `face_hardened_pct`, `small_arms_ricochet_deg` | same | `[vehicles.gunnery]` |
| AI difficulty | same | `[difficulty.*]` |
| AI commander (S3b: `tank_reserve_mp` in `[ai.armour]`; fire-and-move, AT and satchel hunts in `[ai.assault]`) | same | `[ai]`, `[ai.infantry]`, `[ai.armour]`, `[ai.assault]` |
| S3c AI armour habits: rearm trips (`rearm_ap_below`, `rearm_full_pct`, `rearm_max_s`), crews re-crewing (`recrew_clear_m`, `recrew_reach_m`), denial fire at enemy abandoned tanks (`deny_m`) | same | `[ai.armour]` |
| S3c sticky satchel hunts (`satchel_stick_s`, `satchel_chase_s`, `satchel_leash_m`, `satchel_crawl_m`, `satchel_hunt_max_s`), the second base of fire (`second_base_below_pct`, `second_base_gap_m`), AT stalks of stuck tanks and following own tanks (`at_hunt_stuck_m`, `at_follow_m`, `at_follow_back_m`) | same | `[ai.assault]` |
| S3c fragments over a falling edge (a burst on a parapet crest reaches the trench behind it): `frag_fall_probe_cm` | same | `[combat.blast]` |
| S3c point-symmetric terrain (the base-0 half mirrored through the centre; woods, points and bases are placed by hand) | `defs/maps/*.toml` | `[terrain] symmetry = "point"` (default `"none"`) |
| How steady a squad type is under fire (S3b `steadiness_pct`, scales the duck and pin thresholds; the veterancy hook) | `defs/units/*.toml` | `steadiness_pct` (default 100) |
| Where bought squads spawn in the base (S3b) | same | `[call_in]` |
| One unit, weapon, item, vehicle, round | `defs/units/`, `defs/weapons/`, `defs/items/`, `defs/vehicles/`, `defs/ammo/` | the def's own file |
| HE fragments and overpressure (S3b) | `defs/weapons/` (mortars), `defs/ammo/` (tank HE, rockets), `defs/items/` (grenades, satchels) | `[he]`: `blast_radius_cm`, `frag_count`, `frag_reach_cm`, `frag_damage`, `frag_far_pct`, `frag_elev_deg`, `frag_track_damage`; plates beaten: `inside_rays`, `inside_damage`, `inside_reach_cm` |
| APHE filler, barrel, crew and component boxes (S3b) | `defs/ammo/` `[filler]` (`rays`, `cone_deg`, `damage`, `reach_cm`); `defs/vehicles/` `barrel` on the main gun, `[[seats]]` `box` (S7; was `[crew.boxes]`), `[[components]]` `box` or `boxes`. A hull-frame box must lie inside the hull-frame armour boxes (S7 `[[boxes]]`); a turret-frame box inside the turret box or below it down to the hull floor (the turret basket, S3b Tune); either may reach 50 cm above an open-topped box (S7) | the def's own file |
| Vehicle MG stowage (S3b Fix): rounds per coax / hull MG, refilled a belt per base rearm step | `defs/vehicles/` | `rounds` on a `coax` or `hull` `[[weapons]]` entry (omit: no limit) |
| Call-in cost and re-call cooldown (S3b) | `defs/units/*.toml` (squads), `defs/vehicles/*.toml` (tanks) | `cost`, `recall_cooldown_s`, `initial_cooldown_s`; optional `cooldown_group` in a unit def (types in one group share a timer) |
| S4 card block: `cooldown_s` / `unlock_s` (the new names of `recall_cooldown_s` / `initial_cooldown_s`), `max_alive`, `limit_group`, `available_from` / `available_to`, `tags`, `capabilities`, `training`, `ai_weight`, `short_name`, `sources`; men by `[[members]]` slots; `extends` (a variant gives only what differs) | `defs/units/*.toml`; `docs/modding/variants.md` | top-level keys, `[[members]]` |
| S4 training profiles (aim, reload, steadiness, spotting, build rate, as % of a regular soldier) | `defs/training/*.toml` | the def's own file |
| S4 match date for card dates, default training, named unit limits, the AI mix by tag | `defs/rules/skirmish.toml` | `[call_in] date`, `[training] default`, `[limit_groups]`, `[ai.mix]` (or the old `mix_*_pct` keys of `[ai]`) |
| S4 pop cap 150 (historical squad sizes, owner Q6 (A)) | `defs/rules/skirmish.toml` | top level `pop_cap` |
| S4 Panzerfaust gap: one single-shot AT item per squad this often (`single_shot_gap_s`) | same | `[items]` |
| S4 crew-weapon pickup reach: only a man this close (m) takes a fallen gunner's MG or launcher (`crew_pickup_reach_m`) | same | `[items]` (absent in older rules: 30) |
| S11b rifle grenades (`docs/slices/S11b.md` 2, G1, G3): switch `rifle_grenades` (0..=1, 1 in ww2 since S11b Build C: 0 drops the `rifle_grenade` kind from every squad type, for every player) and `rifle_grenade_gap_s` (0..=600, 15: one rifle grenade a squad this often) | same | `[items]` (absent keys: these values) |
| S4 AI Panzerfaust hunts (`pzf_hunt_m`, 0 = off; `pzf_clear_m`) | same | `[ai.assault]` (absent in older rules: 100 / 40) |
| S4 AI capture of enemy abandoned tanks by its crews (`capture_m`, 0 = off; needs `[vehicles.recrew] capture`) | same | `[ai.armour]` (absent in older rules: 150) |
| S4 weapons: crew weapon flag (`crew_weapon`, default by class), hand-over time of every crew weapon (`handover_ms`), squad-card name (`short_name`), `sources` | `defs/weapons/*.toml` | top-level keys |
| S4 Panzerfaust: aim time (`windup_ms`), arming distance (`min_range_m`), the launcher def it fires as (`weapon`) | `defs/items/panzerfaust60.toml`; the shot itself `defs/weapons/pzf60.toml`, the round `defs/ammo/pzf60_heat.toml` | top-level keys |
| S5a ammunition prices per tonne by supply kind (`small_arms_mp_per_t`, `explosives_mp_per_t`, `shell_mp_per_t`), the rack factor (`rack_refill_max_card_pct`, `rack_rearm_price_pct`), rare rounds (`rare_cap_max`, `rare_rearm_base_s`), base refill rates (`base_small_arms_rounds_s`, `base_bomb_s`, `base_bomb_after_fire_s`), the HUD low marker (`low_pct`); see `docs/modding/ammo.md` | `defs/rules/skirmish.toml` | `[supply]` (absent: unlimited rounds, free refill) |
| S5a sharing inside a squad (`interval_s`, `reach_m`, `below_pct`, `giver_above_pct`, `share_pct`) and the crew feed of a dry MG or mortar (`crew_feed_m`) | same | `[combat.share]` (absent: the S5a defaults) |
| S5a counted rounds: a cartridge's weight and supply kind | `defs/cartridges/*.toml` | `supply = { kind, g }` |
| S5a a weapon's feed (`feed`), a man's default load (`load`), bursts (`burst_rounds` shots, then `burst_pause_ms` on top of the interval; 0 = none), its own fire-discipline share of its range (`hold_beyond_pct`, 10-100; 0 or absent: the rules') | `defs/weapons/*.toml` | top-level keys |
| S5a fire discipline: a man in the default mode opens fire within `hold_beyond_pct` of his weapon's range; in `hold` only within `hold_open_m`; a held squad springs at `hold_spring_points` | `defs/rules/skirmish.toml` | `[combat.fire]` (absent: 100 / 30 / 30, the pre-S5a behaviour, S5a-29) |
| S5a AI ammunition: refill trips (`return_below_pct`, `dry_pct`, `max_returning_pct`, `refill_full_pct`, `refill_max_s`), mortar reserve (`mortar_reserve_pct`), buys keep `float_mp` back while a unit waits for a paid refill, a paused refill ends after `rearm_stall_s` | same | `[ai.ammo]` (absent: the S5a defaults) |
| S5b fire missions: who observes and how far (`observe_leader_m`, `observe_observer_m`), the voice link (`observer_link_m`), sight-line lifts (`point_lift_mm`, `burst_seen_lift_mm`), laying and adjusting times (`fire_direction_s`, `direct_s`, `adjust_wait_s`), when to fire for effect (`ffe_error_m`, `adjust_rounds_max`), the aim error (`first_round_error_pct` / `_min_m`, `ranging_step_pct`, `map_fire_error_pct` / `_min_m`), the ranging memory (`ranging_cell_m`, `ranging_expiry_s`, `ranging_keep_move_m`, `ranging_max_cells`), the knowledge map for map fire (`intel_cell_m`, `intel_keep_s`, `intel_trench_keep_s`, `intel_period_s`, `intel_snap_m`), HUD mission rounds (`player_rounds`); see `docs/modding/fire-missions.md` | `defs/rules/skirmish.toml` | `[fire_mission]` (absent: missions run as the old barrage, S5b-1) |
| S11a1 gun fire missions (`docs/slices/S11a1.md` 2, 4; `docs/modding/fire-missions.md` "Guns"): switches `gun_missions` (G1: a served gun with an `[indirect]` main weapon takes FireMission), `radio_link` (G5: an observer links by radio), `mask` (G2: a gun's arc must clear the ground), each 0..=1, default 1, off 0 (ablation `a_off`); `gun_fire_direction_s` (0..=3600, 20), `gun_adjust_rounds_max` (1..=50, 1), `gun_first_round_error_pct` / `_min_m` (0..=50 / 0..=2000, 3 / 25), `gun_map_fire_error_pct` / `_min_m` (0..=50 / 0..=2000, 6 / 50; the observed floor at most the map floor), `gun_break_m` (0..=2000, 150: a seen enemy this close ends the gun's mission) | `defs/rules/skirmish.toml` | `[fire_mission]` (absent keys: these values) |
| S11a1 G7 `arty_base_rearm` (0..=1, default 1: an `inf_gun` rearms in the own base at the shells price as an AT gun; 0 = HQ, dump, truck only; ablation `base_rearm_off`) | `defs/rules/skirmish.toml` | `[supply]` |
| S11a1 artillery AI (`docs/slices/S11a1.md` 2, A1-A6, P1-P3; Build C): switches `buy`, `fo`, `prep` (0..=1; 1 in ww2 since Build C, 0 when the table is absent; the post search casts its own `post_rays`, the fire checks use the rays the mortars left); buying `after_s` 240, `income_max_pct` 15, `reserve_mp` 0, `max_guns` 1; observers `fo_max` 1, `fo_rebuy_s` 180, posts `post_m` 300, `post_side_m` 60, `post_rays` 8 (0..=48), `post_clear_m` 150, `post_retry_s` 60; fire `mission_rounds` 4 (1..=50), `reserve_pct` 30, `shell_mp_max_pct` 25, `danger_close_m` 50; preparation `prep_near_m` 40, `prep_rounds_min` 2, `prep_wait_s` 60. Ranges: s 0..=3600, m 0..=2000, % 0..=100, rounds 1..=50, `max_guns` / `fo_max` 0..=8 | `defs/rules/skirmish.toml` | `[ai.arty]` (absent: these values, switches off) |
| S11a2 friendly fire and counter-battery (`docs/slices/S11a2.md` 2, F1-F4, K1-K4; Build A): switches `friendly_fire` (F1: mortar and gun mission bursts hurt own men; own-fire deaths are not kills) and `counter_battery` (K1: infantry-gun mission rounds leave sound evidence with the enemy), 0..=1, 1 in ww2 since Build B, 0 when `[fire_mission]` is absent; `cb_rounds_to_mark` (1..=50, 3), `cb_decay_s` (1..=3600, 300), `cb_delay_s` (1..=3600, 15), `cb_error_pct` / `cb_error_step_pct` / `cb_error_floor_pct` (1..=100, 15 / 80 / 5; the floor at most `cb_error_pct`), `cb_reset_m` (0..=2000, 75), `cb_near_m` (0..=2000, 20; report: counter-fire bursts of live and ended missions). Map fire on an own marker aims with the marker's error (a length uniform in `0..=err_mm`, at most the map-fire error) when `[ai.arty] counter_fire` is 1, else with the map-fire error; counter-fire rounds add evidence with the enemy but no new shots. Ablations `docs/slices/targets/S11a2-ablations/` | `defs/rules/skirmish.toml` | `[fire_mission]` (absent keys: these values) |
| S11a2 artillery AI (`docs/slices/S11a2.md` 2, S1-S3, R1-R3; Build B): switches `ff_safe` (one own-men predicate for the order filter and the lift: a living own man inside `danger_close_m` of the target, or an own squad walking in, goal inside `danger_close_m` and lead inside `lift_m`; the mortar spotter stands `danger_close_m + 20` short; every own mission with own men near its target gets one `Stop`, `ff_lifts`) and `counter_fire` (an AI gun answers an own marker no wider than `cb_max_error_m` with map fire, once per new evidence, after preparation fire and before observed targets; no order the sim would refuse, `cb_masked`), 0..=1, 1 in ww2 since Build B (0: S11a1 behaviour); `lift_m` (0..=2000, 100), `cb_max_error_m` (0..=2000, 60) | `defs/rules/skirmish.toml` | `[ai.arty]` (absent keys: these values) |
| S11b artillery AI (`docs/slices/S11b.md` 2, X3, H1, L1): switches `cb_alternate` (an AI gun does not order counter-fire twice in a row: `AiArty::cb_last` holds a fresh marker until an ordinary mission, `cb_alt_holds`), `buy_pending` (gun buys count the manpower of call-ins still pending), `how` (the AI buys a field howitzer), `light` (cards tagged `light_mortar` join the mortar slot of the buy mix), 0..=1, all four AI-only (the cards stay buyable), 1 in ww2 since S11b Build C but `light` 0 since the S11b fix stage (owner question S11b-14) (`cb_alt_holds` counts planner passes that held, not markers); `how_after_s` (0..=3600, 480), `how_max` (0..=8, 1), `how_income_pct` (0..=100, 15: howitzer cards apart from `income_max_pct`; since the S11b fix stage also the howitzer's pot, which the other buys leave alone, from `how_after_s` while no howitzer is alive or pending), `how_reserve_mp` (0..=100000, 300) | `defs/rules/skirmish.toml` | `[ai.arty]` (absent keys: these values) |
| S5b AI fire missions: rounds per mission (`mission_rounds`, was `ai.barrage_rounds`), spotters and map fire (`spot_samples`, `spot_max_s`, `spot_hold_s`, `map_fire_min_pct`, `map_fire_min_score`, S6b `map_fire_max_miss_m`; see `docs/modding/fire-missions.md` "The AI"); the mortar's support spot `[ai.infantry] support_back_m` (S5b: 120) | same | `[ai.mortar]` (absent: `ai.barrage_rounds` or 6) |
| S5b a mortar's charges and laying limits (`charges_m_s`, `elevation_deg`): the solver takes the lowest charge with a high-angle solution | `defs/weapons/*.toml` | `[indirect]` (absent: the S1 flight rule) |
| S11a1 content for guns and radio (`docs/slices/S11a1.md` 2; `docs/modding/fire-missions.md` "Guns"): `[indirect]` `low_angle` (false: try each charge's high root, then its low root; above +60 deg the def check wants a `low_angle` table), `range_m` (default the weapon's `range_m`), `spread_mrad` (default the weapon's spread); member trait `radio` (the observer's link, `observer_link_m`); vehicle role `inf_gun` (base rearm, `arty_base_rearm`). Shipped in `ger_leig18`, `usa_105mm_m3`, `ger_vb_trupp`, `usa_fo_party` | `defs/weapons/*.toml`, `defs/units/*.toml`, `defs/vehicles/*.toml` | `[indirect]`, `traits`, `role` |
| S5b the `observer` member trait | `defs/units/*.toml` | `traits` in `[[members]]` |
| S5a AI Panzerfaust ambush hold (`ambush_watch_m`, 0 = off; `ambush_offset_m`, `ambush_max_s`, `ambush_release_m`) | same | `[ai.assault]` (absent in older rules: off, 25, 90, 0 = the watch) |
| S5a rounds per man by cartridge id | `defs/units/*.toml` | `rounds = { "<id>" = N }` in `[[members]]` |
| S5a weights of items and rounds (`supply`), rare rounds (`rare`), a vehicle's rare-round cap (`rare_cap`) and stowage (`ammo` counts) | `defs/items/`, `defs/ammo/`, `defs/vehicles/` | top-level keys |

Each key has a comment with its unit and meaning. Units are in the key's
name: `_mm`, `_cm`, `_m`, `_s`, `_ms`, `_pct`, `_deg`, `_mm_s`. Values are
whole numbers. A value outside its range fails to load. The error names the
file, the key, the allowed range and a fix, for example:

```
defs/rules/skirmish.toml: combat.mortar.flight_milli: must be in 1..=5000, got 0 (fix: [combat.mortar] flight_milli = 1)
```

Check a mod without running anything: `frontline-cli validate mods/ww2`.

Engine limits (pool sizes, search sizes, substep rate) stay in code.
`docs/slices/S3b-balance.md` "Tuning surface" lists them.

## Try a change without editing the mod

Write an override file with only the keys you change. It has no `id`,
`name` or `base`. `mods/ww2/tests/overrides/example.toml` shows the format.

```toml
[combat]
near_miss_cm = 250            # top-level tables patch the rules, key by key

[defs.vehicles."ww2:ger_pz4_h"]
cost = 620                    # [defs.<kind>."<id>"] patches one def
```

The def kinds are `units`, `weapons`, `items`, `factions`, `maps`,
`vehicles`, `ammo` and (S4) `training`. An array, such as a vehicle's
`[[weapons]]`, is replaced whole. S4: the patch applies to the def's file
as written and every def then resolves again, so a patch on a parent also
changes each variant that `extends` it (each change is listed); a
`[[defs.units."<id>".members]]` patch replaces a legacy squad's `men` /
`weapon` keys (`docs/modding/variants.md`).

Pass the file with `--rules-override FILE` to `skirmish`, `armour-test` or
`fortify-test`. The flag can be given more than once; files apply in order.
`tools/batch.sh` and `tools/measure.sh` take the same files as `-o FILE`.
S10a-ai's ablations are such files (`docs/slices/targets/S10a-ai-ablations/`:
`all_off`, `works_off`, `wire_off`, `breach_off`, `cards_off`, and the
per-work `nest_off`, `dugout_off`, `roof_off`, `sandbags_off`, each setting
`[ai.fort]` switches to 0); `tools/ablate.sh ONDIR OUTROOT [NAME]...` measures
the named ones (default the first five; `all`: every file)
against a switches-on measure dir and prints one line per ablation (pooled
Germany, its change, whether it matters); `--dry-run` only checks the files
and prints the commands (`tools/check.sh quick` runs it).
The run prints every value the files change to stderr, as
`old -> new`. An override changes the match, so its hashes differ from the
base run. `simulate` refuses overrides, because bench scenarios have no rules.

## A/B test in one command

```sh
B=mods/ww2/tests/overrides/my_change.toml
frontline-cli skirmish --ai-vs-ai --seeds 10 --both-bases --ab --rules-override $B
```

- `--seeds N` plays seeds `--seed .. --seed+N`. Matches run in parallel; set
  the number with `--jobs`. Results and hashes do not depend on it, but the
  tick-time p99 does (the same 160 matches: 0.71 ms peak at `--jobs 3`,
  1.55 ms at `--jobs 6` on an 8-core Mac): quote T10 from a `--jobs 3` run.
- `--both-bases` plays each seed twice, once with the bases swapped.
- `--summary` prints one table instead of every report: wins per faction and
  per base, match length, tanks lost, knock-out causes, HE share, rockets,
  satchels and tick p99, plus one line per match. S3c adds report-only
  rows (`sdk/crates/cli/src/s3c_tally.rs`): tank base returns and how
  long MGs were dry or HE at the reserve, satchel hunts and how they
  ended, crew alive at each knock-out, and per base side when each point
  was first taken and how long it was held. `--report json` also has
  each knock-out's position and its shooter's (`s3c.ko_spots`). S3c
  Part A: the "tank losses" line counts knock-outs (`ko_*`) and
  abandonments (`abandoned_<trigger>`); an `S3c <faction>` line splits
  tanks lost into destroyed and abandoned, says what became of the
  abandoned (re-crewed, captured, destroyed later, scrapped, standing) and
  counts crewmen alive at bail-outs, out and dismissed in base; an `S3c
  crews` line gives the abandonments with crew inside where at least one
  man got out, and the men out still alive 60 s later. Rockets are
  credited for an abandonment by the last hit, as for a knock-out. S3c
  Part B: an `S3c AI <faction>` line sums the AI's rearm trips, satchel
  hunts kept through a loss of sight, satchels thrown by hunters, hunt
  ends by reason (`gone`, `unseen`, `fast`, `leash`, `timeout`, `spent`,
  `retreat`), second bases of fire, AT stalks of stuck tanks and follows,
  re-crew orders, crews sent back and denial orders.
- `--ab` plays the same matches without the overrides too. It prints both
  tables and the change in each headline number.

On an 8-core Mac, 10 seeds × both bases (20 matches) take about 20 s, and
with `--ab` (40 matches) about 40 s. `--report json` gives the same data as
JSON.

For the staged checks, run the same override through
`frontline-cli armour-test --rules-override $B` (duels, AT teams, trench
assault; the test rules are derived after the override) and
`frontline-cli fortify-test --rules-override $B`.

## Results as JSON, targets and checks (read numbers, not text)

Agents and tools should read these files, not the text tables.

**`--json FILE`.** `skirmish` (one match, `--seeds`, `--ab`), `armour-test`
and `fortify-test` write one JSON object with every number their text output
prints; the normal output is unchanged. (A bare `--json` on `armour-test` and
`fortify-test` still prints the old JSON report instead of the text.)

```sh
frontline-cli skirmish --ai-vs-ai --seeds 20 --both-bases --minutes 45 --jobs 6 --summary --json /tmp/s40.json
frontline-cli armour-test --seeds 5 --json /tmp/arm5.json
frontline-cli fortify-test --seeds 10 --json /tmp/fort10.json
```

- Skirmish file (`"kind": "skirmish"`): `matches`, `per_match[]` (`seed`,
  `side` normal or swapped, `winner_faction`, `winner_base`, `length_s`,
  `he_share_pct`, `hash`, tanks lost, rockets and satchels per player),
  `wins` (`germany`, `germany_pct`, `usa`, `usa_pct`, `base0`, `base0_pct`,
  `base1`, `base1_pct`, `base_max_pct`, `no_winner`), `length_min` (`mean`,
  `min`, `median`, `max`, `over_25`), `he_share_pct` (`mean`, `max`),
  `tick_p99_ms` (`peak`, `all`; only meaningful from a `--jobs 3` run
  alone), `tank_losses`, `factions.<name>` (the per-faction tables: kills,
  tanks, `ammo.fired`, `ammo.spend_mp` per kind, `kills_by_source_pct`,
  `s4`, `s5a`, `s5b` and `s5b_derived` such as `ai_observed_direct_pct`,
  `refused_total`, `bombs_total`), `s9` and `s9_derived` (the guns: S9b
  `gun_armour_out`, and `gun_armour_out_per_match`: enemy tanks, assault
  guns and tank destroyers knocked out or abandoned per match when the last
  round that struck them was one of the faction's AT guns', a gun of the
  faction having fired that ammunition at that vehicle within 3 s; each
  vehicle once a match, however often it is recrewed and lost again; a
  bail-out counts, so it is about four times `gun_ko_tanks_per_match`,
  which counts wrecks) and `s3c`. `--ab` writes `{a, b}`.
- armour-test file: `cases[]` (`id` `case9`, `name`, `pass`), each with
  `checks[]` (`id`, `name`, `value`, `limit`, `pass`, `detail`; `value` and
  `limit` are null for checks that are a yes/no on several runs) and
  `rows[]` (the report rows with their `target` text). fortify-test file:
  `checks[]` (rifles, mortar, smoke) and `rows[]` (ranged mortar, grenade
  variants, MG suppression totals); S10a `s10a.{wire, roof, breach}` and
  S10a-ai `s10a.breach_ai` (the breach row driven by the AI's breach job:
  `gap_opened_pct`, `placed_pct`, `defender_loss_pct`, `attacker_loss_pct`
  and the same under `control`).

**`frontline-cli targets FILE.toml [--from-json DIR] [--jobs N] [--runs a,b]
[--out DIR]`.** A targets file lists the runs that feed it and the targets
(`docs/slices/targets/S5b.toml` is the model):

```toml
[runs.skirmish40]          # kind: skirmish | armour-test | fortify-test
kind = "skirmish"
seeds = 20                 # first_seed (default 1), both_bases, minutes, jobs
both_bases = true
minutes = 45

[[target]]
id = "T5a"
metric = "skirmish40.wins.germany_pct"   # <run>.<path into that run's JSON>
min = 35                                  # min and/or max, inclusive
max = 65
kind = "hard"              # hard (default) or report
```

A path walks objects by key and arrays by index or by an element's `id` or
`name` (`armour5.cases.case9.checks.trench_casualties_40_of_open.value`).
The command plays the runs the targets name (in this process, same
results as the CLI; JSON saved to `target/targets/<file>/<run>.json`), or
with `--from-json DIR` reads `DIR/<run>.json`. It prints one line per target
and a count:

```
T5a skirmish40.wins.germany_pct 40 [35,65] PASS
T4a skirmish40.factions.germany.s5b_derived.refused_total 1 [-,0] MISS
T14a timing40.tick_p99_ms.peak - [-,1.5] SKIP (run timing40 not played)
12 pass, 1 miss (hard: 1), 11 skipped
```

`--runs` limits which runs are played (the others print SKIP); `--jobs`
sets matches at once (default 6, the balance-batch setting). A run's own
`jobs = 3` overrides it: put tick-time targets on a separate run with
`jobs = 3` and play it alone. Exit code 1 when a hard target misses
(including a path that does not exist); report targets only print MISS.

**`tools/check.sh quick [BASE] | full`.** Few lines instead of cargo's and
Godot's output; full logs in `target/check/*.log`.

- `quick` runs `cargo fmt --check`, clippy `-D warnings` and `cargo test -q`
  for the crates changed against `BASE` (default: `$CHECK_BASE`, else the
  merge-base with `origin/<branch>`, else with `main`; a slice passes its
  base commit). It prints `tests (cli): ok (29 passed, 0 failed)` and, for a
  failing test, its first 40 lines.
- `full` adds the whole workspace's tests including the ignored ones
  (`--release`), golden ballistics, `validate mods/ww2`, the bridge build,
  the four Godot headless tests (`check_project`, `test_match_view`,
  `test_net_ui`, `test_data_reload`) and a check that Godot's
  `settings.cfg` still equals the saved copy (`$SETTINGS_BAK`). One line each:
  `name: ok` or `name: FAIL (see target/check/name.log)`.
- Both exit non-zero on any failure. `$GODOT` overrides the Godot binary.

## Reload in Godot

Every new match reads `mods/` from disk, so edits apply at the next start.
In a debug build (the editor or a debug export) you can also reload inside a
match:

- press **F5**, or choose **Reload data & restart** in the pause menu (Esc);
- the match restarts with the same faction, difficulty, map and seed;
- if the data does not load, a red banner shows the loader's message and
  the running match goes on. Fix the file and press F5 again.

Release builds do not offer the reload. They play the data shipped with the
game. Net matches do not offer it either.

## Multiplayer

The lobby compares the content hash of both players' data. A player whose
TOML differs, even by one value, is refused with "The two games have
different mod content". The game reads the mod again every time a player
hosts or joins.
