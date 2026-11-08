# S8b log: trucks, recon cars, wheels

Branch `s8b-trucks-recon` from `0dc13dd`; spec `S8b-trucks-recon.md` (section 17 "Tune results"); targets
`targets/S8b.toml`. Owner 2026-11-08: "run another session of development". SAVE 19, PROTOCOL 11 (each bumped once). Not pushed.

## Commits

Plan `5966970`. Build A `caf5a07` `e122567` `8d6e865`. Build B `7c4bad7` `abe32ee` `81997a9`. Build C `7610ca3`
(rides, AI, case 32, JSON), `c429227` (presenter), `6ce0584` (tune, docs), `c7ec3e1` `e809c36` `ad73a95` `4f167da`
`d8ed359` (event words, log, comments). Tune `a6712cd` (M3A1 capacity 12), `52052ed` (results). Fix `522760f`.

## Built

**Build A.** `wheeled` bicycle drive (no pivot, reverse out of the turning circle), a wheel block layer (raw nodes; the
dilation is its margin), `WheelsFront`, `WheelsRear`, `Cargo` components (absent parts read 0 HP), `reverse_cm_s`,
`[optics]` as one sight multiplier after `settle_s`, `grenade_screen` (thrown items bounce off and fall beside the hull;
bullets pass), SdKfz 234/1 and M8, 2 cm KwK 38 and 37 mm M6 with five rounds each.

**Build B.** `role = "truck"` (no weapons; turret optional), `[supply]`, paid loading in base, one source function for
HQ, dump, truck and crate (`source_for`: fastest in reach, one draw per due tick, rates never stack), forward MG belts
at HQ, dump and truck (S8b-6), `DropCrates { vehicle, point, count, kind }` (S8b-2), cargo box that detonates on a
penetration or fire at 40 kg or more, Opel Blitz and GMC CCKW, armour-test cases 27-29, 31.

**Build C.**
- *Rides that pay* (S8a-28, -29, -31). Phase `Meet` (the half-track goes to the squad, which keeps walking), `pick_m` 400,
  ride lock (`ride_lock_s` 60), `wait_s` 10, `dismount_sight_m` 150, `drop_short_m` 30, `min_ride_m` 200, `support_s` 15.
  Unloads roll on arrival and over the sides when nothing threatens. Counters `obj_board_ticks`, `obj_foot_ticks`,
  `unload_*`. `counted_ride_m` keeps T41's S8a 120 m definition.
- *Recon AI* on the S7 overwatch entry (S8b-24): `view_back_m` 200, backs off from AT teams and armour at `at_threat_m`
  200, S7 open-top block. Own budget `[ai.recon]` 12 % of income.
- *Truck AI* (`ai/truck.rs`): Load, Follow (150 m behind the armour, unseen spots), Serve (a stranded tank first), Drop
  (mortars, HQ, squads holding a point), Return; flees seen enemies. Own budget `[ai.truck]` 9 %. `mix_slot` keeps both
  out of the tank mix. Tanks take trucks carrying ten rounds or more as forward stock, nearest reachable first, and
  hold while a truck comes (`JOB_MAX_S`).
- *Presenter.* Wheels, truck bed and cargo, wire screen, call-in tiles, Drop Crates (I; I again cycles the kind), truck
  stock bars and reach rings, `match_state.md` "S8b". Screenshot `s3_recon_trucks.png`, looked at.
- *S8a HUD faults.* (1) Outside-limit tiles show their own `alive/max` and unlock. (2) Cards list only the parts a
  vehicle has (closes the red hatched bar, S8a-27). (3) "MG dry" was the `halftracks` screenshot scene, which empties
  the MGs; a bridge test shows a fresh 251 reports its stowage (S8b-23).

**Tune** applied the owner's M3A1 decision (S8a-2 (B)): a twelfth passenger box; sim unchanged.

**Review and Fix.** Review (`52052ed`): 3 medium, 3 low, no high. Fix (`522760f`) addressed M1-M3, L1, L2:
- *M1* A scrapped truck kept its cargo into the wreck, so the save would not load. `abandoned_cap` now calls
  `truck_wrecked`; `abandon()` clears the drop job. Test `a_scrapped_truck_loses_its_cargo_and_the_save_still_loads`.
- *M2* A recon car under way never backed off (`ReverseUnits` only on Idle). Now it backs off from any non-away order,
  one order per 10 s (`BACKOFF_GAP_S`, saved `backed_at`), and the spot it backs to walks toward base inside
  `at_threat_m` + 12 m. Test `a_recon_car_under_way_backs_off_from_known_armour`. Defaults S8b-28, S8b-29.
- *M3* AI truck jobs retried an unreachable goal (`rv ailoop`: a second Drop order 182 s later, same point). Now
  `AiTruck.gave_up` (4 goals, 40 m), a Drop timeout counts as a failure, and a stuck wheeled move ends idle after
  `wheeled_stuck_repaths` 12 re-paths inside 6 m (S8b-28). Test `an_ai_truck_does_not_loop_on_a_goal_it_cannot_reach`
  (`#[ignore]` by design; the release run includes it).
- *L1* The follow spot ignored known vehicles. Fixed. *L2* `AiTransport.pi` and `point` unchecked on load. Fixed, with
  tests. *L3* (new cards lower the tank share) rejected as an owner decision (S8b-26).

## Balance (final: Fix build, three 160 sets, pooled 480)

Set names in the Fix run: `skirmish160` (seeds 1-80), `skirmish160_oos` (101-180), `skirmish160_s201` (201-280).

| | 1-80 | 101-180 | 201-280 |
|---|---|---|---|
| Germany wins % | 41.9 | 47.5 | 50.0 |
| GER saved s per ride | +2.0 | +4.5 | -2.8 |
| USA saved s per ride | +14.9 | +11.4 | +19.9 |
| GER riders moved / trips per bought | 1.21 / 1.22 | | |
| USA riders moved / trips per bought | 1.17 / 1.29 | | |
| Top vehicle kills % (T32, cap 18): GER / USA | 17.07 / 14.64 | | |
| Vehicle share of manpower % (T33a / T33b, cap 35) | 36.01 / 35.54 | | |

Pooled Germany 223 of 480 = 46.46 %, in band. Set noise is about 7.7 points, so the 1-80 miss is not a reason to tune.

Ride value over the stages (saved s per ride, GER / USA; seeds 1-80 / 101-180 / 201-280):

| Stage | GER | USA |
|---|---|---|
| S8a Fix (start) | -29 / -26 / -21 | -14 / -15 / -6 |
| Build C | +4.2 / +2.6 / -8.8 | +17.9 / +8.7 / +11.8 |
| Tune (M3A1 slot 12) | +5.0 / +10.8 / +2.3 | +6.8 / +7.6 / +22.9 |
| Final (Fix build) | +2.0 / +4.5 / -2.8 | +14.9 / +11.4 / +19.9 |

USA rides pay on all three sets; Germany pays on two of three. The 201-280 pair is about 50 rides and moves about
+-10 s by chance (S8b-27). Build C's per-set table is in commit `4f167da`.

Trucks and recon (Build C, 160 sets): crates dropped per match 6.2 / 6.1 / 5.8 (GER), 11.3 / 9.6 / 11.2 (USA); 234/1
about 6 % of German kills, M8 about 8 % of US kills; trucks bought in 94-95 % of matches, cars in 99 %; the tank mix
holds (T54). Final build: stranded served 0 / 0.013 per match, `ai_move_fails` 1.3 / 1.4.

Prices (inside 8.4's bands): 234/1 260, M8 300, Blitz 150, CCKW 200. Pooled Germany read 43.1 % before, 45.4 % after (with arrival unloads).

## Targets

Fix build: skirmish40 12 pass / 0 miss; armour5 55 pass / 0 miss; `skirmish160` 112 pass / 3 miss (T5c, T33a, T33b);
`skirmish160_oos` 16 pass / 0 miss; `skirmish160_s201` 4 pass / 1 miss (T50e). Tune run over the three sets: 132 pass,
4 miss, 71 skipped. Build C over all sets: 199 pass, 8 miss (4 hard).

Hard misses: T5c (Germany 41.9 % on 1-80; pool in band), T33a/b (vehicles 36.0 / 35.5 % of manpower; S8b-26), T50e
(Germany -2.8 s per ride on 201-280; S8b-27). Report misses: T70a and T70c at the Tune run (USA 0.86 and 0.93 on
101-180); the Fix run says T70 passes on the other two sets. Passing: T41, T48, T32, T52-T57, T60-T68, T71.

## Armour tests

- Case 25 (wheeled vs trench line): 0 crossings, the gap is reached in 100 %, the tank crosses in 100 %.
- Case 26 (wheeled drive): no pivot, radius error 0.04 %, 234 reverses at 1000 cm/s, wheel blast damages a wheel in 100 %.
- Case 30 (grenade screen): screened 234 inside 0 %, crew lost 0, grenades land beside in 100 %; bullets pass the screen.
- Case 31 (guns vs plates): 2 cm vs M3A1 front 100 %, 37 mm vs Pz IV front 0 %, rifle vs M8 front 0 %; sight gain 36.4 %.
- Case 27 (truck stock falls by the grams given), case 29 (one source, no stacking), case 32 (stranded tank served
  100 %, 18.9 rounds gained, Build C). `armour5` on the Fix build: 55 pass, 0 miss.

## Tick time

Clean run (Build C): `timing40` alone at 3 jobs, p99 0.427 ms all, 0.522 ms peak (S8a after Fix 0.488 / 0.621). The
Review's clean run before the Fix read 0.406 / 0.487. After the Fix the runs were not clean: 1.101 peak right after
the batches, then 0.672 and 0.677 with load average about 4. Those two are +8-9 % on S8a's 0.621 (below the 25 % line)
but 29 % above Build C's clean peak. Not profiled. Settled by the orchestrator's clean `timing40` on the final build (f6e19b2): 0.449 ms all, 0.568 peak, below S8a. Not profiled.

## Checks

- Fix build: `tools/check.sh quick 0dc13dd` all ok, 610 passed. Release `--include-ignored` 644 passed, 0 failed.
- Godot (Build C): check_project OK, test_match_view 306/306, test_net_ui 143/143, test_data_reload 21/21. Rerun on the
  final build by the orchestrator: same counts, all pass.
- Determinism: per-field save, hash and validate tests for the new AI state; whole-match AI determinism at 1, 2, 3, 8
  threads with save, load and continue; `wheeled.rs` and `trucks.rs` cover wheels and cargo.

## Known gaps

- T50e and T5c read as noise; the ride lever stayed where Build C left it. Rides per set are about 40-60.
- About two thirds of unloads are the `dismount_sight_m` stop. The walk on foot (45-70 s) is now the largest ride cost;
  boarding is about 11 s.
- AI tanks rarely run dry or make rearm trips (0-8 per 160 matches), so `stranded_served` is about 0; case 32 shows the job works.
- Figures marked verify (2 cm AP and HE, stowages, reloads, truck plates) and the 3-axle truck blockouts wait for sources
  (S8b-11, S8b-25). Vehicle manpower is over the 35 % cap because the cap predates recon and trucks (S8b-26).
- The net loopback sends `DropCrates` at a non-truck, so the sim refuses it there; accepted drops are covered by the AI
  determinism test, not loopback.
- Godot `Vehicles.COMPONENTS` lists all 10 parts (checked on the final build).

## What later slices need

- `SAVE_VERSION` 19, `PROTOCOL_VERSION` 11. A TOML value change needs the BASELINE refreshed:
  `sdk/crates/content/tests/resolved_defs.rs` and `crates/simbench/src/report.rs`.
- Never index `mounts[0]`; use `main_ammo()` or `mg_mounts()`. `primary_weapon()` is weapon 0 on a truck.
  `BlockLayer::ditch_dm_of` skips wheeled defs, so a new wheeled def must not make AT ditches walls for tanks.
  `gun_out` is false on a def with no mounts.
- S9 AT guns add cards and shift the mix again: watch T33 (vehicle share), T54 (tank floors) and T32 (18 % cap).
- Open owner decisions: S8b-26 (vehicle cap, T33), S8b-27 (T50e), S8b-28 and S8b-29 (wheeled give-up, the car's held
  spot), S8b-30 and S8b-31 (cargo fire, serve only when stopped).
- Later: SetHatch and unbuttoned seats need a per-seat exposed box and a command (`still_ticks` is the hook). Radio
  observers need a recon `observer` trait and a link rule. Carried `[transport]` on trucks, towing, road speeds,
  trucks drawing from an HQ, the push-limit reserve: spec section 15.
