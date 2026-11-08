# S8b log: trucks, recon cars, wheels

Branch `s8b-trucks-recon` from `0dc13dd`; spec `S8b-trucks-recon.md`; targets `targets/S8b.toml`.
Owner 2026-11-08: "run another session of development". SAVE 19, PROTOCOL 11.

## Commits

Plan `5966970`. Build A `caf5a07` `e122567` `8d6e865`. Build B `7c4bad7` `abe32ee` `81997a9`.
Build C `7610ca3` (rides, AI, case 32, JSON), `c429227` (presenter), `6ce0584` (tune, docs), then the check fixes.

## Built

**Build A.** `wheeled` bicycle drive (no pivot, reverse out of the turning circle), wheel block layer
(raw nodes, the dilation is its margin), wheels components, `reverse_cm_s`, `[optics]` as one sight
multiplier after `settle_s`, `grenade_screen` (items bounce off, bullets pass), 234/1 and M8, guns, rounds.

**Build B.** `role = "truck"`, `[supply]`, paid loading in base, one source function for HQ, dump, truck and
crate (`source_for`; fastest in reach, one draw per due tick: rates never stack), forward MG belts,
`DropCrates` with a kind (S8b-2), cargo box and detonation, Opel Blitz and CCKW, cases 27-29, 31.

**Build C.**
- *Rides that pay* (S8a-28, -29, -31). Phase `Meet` (the half-track goes to the squad, which keeps walking),
  `pick_m` 400, ride lock (`ride_lock_s` 60: a squad that got off keeps going to its point unless enemies
  are near), `wait_s` 10, `dismount_sight_m` 150, `drop_short_m` 30, `min_ride_m` 200, `support_s` 15,
  unload rolling on arrival and over the sides when nothing threatens. Instrumented: `obj_board_ticks`,
  `obj_foot_ticks`, `unload_{exposed,arrived,stuck,fire}`. `counted_ride_m` keeps T41's S8a 120 m threshold.
- *Recon AI* on the S7 overwatch (`view_back_m` 200 short of the infantry's point, backs off from AT teams and
  armour at `at_threat_m` 200, S7 open-top block, `SPOT_GIVE_UP_S`), own budget `[ai.recon]` 12 % of income.
- *Truck AI* (`ai/truck.rs`): Load, Follow (150 m behind the armour, unseen spots), Serve (a stranded tank
  first; `stranded_served`), Drop (mortars, HQ, squads on a point), Return; flee from seen enemies; move
  fails stop after `max_fails`. Own budget `[ai.truck]` 9 %. `mix_slot` keeps both out of the tank mix.
- *Tanks* take trucks with shells for ten rounds or more as forward stock, nearest first, and hold while
  a truck comes (time limit `JOB_MAX_S`). The float is the existing `[ai.ammo] float_mp` (S8b-24).
- *Presenter.* Wheels, truck bed and cargo, wire screen, call-in tiles, Drop Crates (I; I again cycles the
  kind), truck stock bars and reach rings, `match_state.md` "S8b". Screenshot `s3_recon_trucks.png`, looked at.
- *S8a HUD faults.* (1) Outside-limit tiles show their own `alive/max` and unlock. (2) Cards list only parts the
  vehicle has. (3) "MG dry" was the `halftracks` screenshot scene, which empties the MGs; a bridge test proves a
  fresh 251 reports its stowage. (4) The red hatched bar was the absent parts in the roster strip, fixed by (2).

## Balance (160 sets; pooled 480)

| | 1-80 | 101-180 | 201-280 |
|---|---|---|---|
| Germany wins % | 41.2 | 47.5 | 47.5 |
| HE share % | 20.4 | 20.6 | 19.5 |
| ger saved s/ride | 4.171 | 2.606 | -8.75 |
| ger transported/match | 1.113 | 1.213 | 1.094 |
| ger trips/bought | 1.127 | 1.252 | 1.129 |
| ger riders lost/match | 0.119 | 0.194 | 0.188 |
| ger crates dropped/match | 6.238 | 6.05 | 5.831 |
| ger stranded served/match | 0.0 | 0.0 | 0.0 |
| ger top kills % | 16.655 | 16.82 | 16.31 |
| ger move fails/match | 0.069 | 0.169 | 0.113 |
| usa saved s/ride | 17.896 | 8.702 | 11.764 |
| usa transported/match | 0.981 | 0.838 | 0.875 |
| usa trips/bought | 1.09 | 0.887 | 0.875 |
| usa riders lost/match | 0.1 | 0.094 | 0.088 |
| usa crates dropped/match | 11.288 | 9.606 | 11.156 |
| usa stranded served/match | 0.0 | 0.025 | 0.006 |
| usa top kills % | 13.964 | 14.372 | 14.066 |
| usa move fails/match | 0.025 | 0.075 | 0.1 |
| GER 234/1 kills % | 6.041 | 5.557 | 5.99 |
| USA M8 kills % | 7.778 | 8.432 | 7.799 |
| GER 234/1 MP % | 6.112 | 6.082 | 6.273 |
| USA M8 MP % | 6.206 | 6.101 | 6.231 |
| median length min | 17.532 | 17.946 | 17.879 |

Pooled Germany 218 of 480 = 45.42 % (S8a 45.4 %), in band. Seeds 1-80 read 41.2 % (S8a 42.5): set noise, T5c, not tuned against. HE share holds (19.5-20.6 %).

## Targets

`targets S8b.toml` over all runs: 199 pass, 8 miss (hard: 4). Hard misses: T5c (Germany 41.2 % on 1-80, the pool is in band), T33a (vehicles 36.3 % of manpower, S8b-26), T41b (USA 0.981), T50e (Germany -8.75 s on 201-280, S8b-27). Reports missed: T70a-d (USA transported and trips on the other sets, 0.84-0.89). Two extra sets on the final build, seeds 301-380 and 401-480: Germany 45.0 and 43.8 %, 800 matches pooled 45.0 %.

## Tick time

timing40 alone at 3 jobs: p99 0.427 ms all, 0.522 ms peak (S8a after Fix 0.488 / 0.621): down 12 %, not profiled (no rise above 25 %). Why it fell is not measured.

## Checks

`tools/check.sh quick 0dc13dd`: all ok, 608 passed. Release `--include-ignored`: 641 passed, 0 failed (the bridge's event test fix included). Godot: check_project OK, test_match_view 306/306, test_net_ui 143/143, test_data_reload 21/21. armour-test seeds 5: all hard checks pass (case 32: 100 % served, 19 rounds gained). Determinism: AI recon and trucks at 1, 2, 3, 8 threads, save, load and continue (`transport.rs`), trucks and wheels likewise. Net loopback sends `DropCrates` (see Known gaps). Screenshot `scratchpad/s8b/s3_recon_trucks.png`, looked at.

## What was tuned, and why

- Start values spent the first half-hour of runs (40 set, then 160). `min_ride_m` 150 to 200 moved saved time from
  -6 / -3 s to +8 / +19 s (40 set); `support_s` 30 to 15 and `pick_m` 250 to 400 raised USA trips per bought 0.79 to 1.04.
- Pooled Germany was 43.1 % over 480 after those. Prices moved inside 8.4's bands (234/1 280 to 260, M8 290 to 300,
  Blitz 170 to 150, CCKW 180 to 200) and arrived rides unload over the sides: 45.4 %.
- Own budgets started at 8 % and 5 %: the first car came at 12 minutes; now 12 % and 9 % (S8b-22). T52, T53, T56 hold.

## Known gaps

- T50e Germany on 201-280 reads -8.75 s per ride (five of six pairs above 0); T41b USA on 1-80 reads 0.981.
  Rides per set are small (40-60), so a pair moves about +-10 s by chance; left alone rather than chased.
- About two thirds of unloads are the `dismount_sight_m` stop: contested points have enemies within 150 m. The
  last stretch on foot (45-70 s) is the larger cost now, not boarding (11 s).
- AI tanks rarely run dry, so `stranded_served` is about 0 per match; case 32 shows the job works.
- Figures marked verify in the defs are from memory (S8b-11). Truck blockouts share 3 axles (S8b-25).
- The net loopback sends `DropCrates` at a vehicle that is not a truck, so the sim refuses it there; a real drop goes
  through the determinism test with AI trucks at 1, 2, 3, 8 threads and across a save.

## What later slices need

SetHatch and unbuttoned seats need a per-seat exposed box and a command (the `still_ticks` hook is there). Radio
observers need a recon `observer` trait in `fire_mission` and a link rule. Carried `[transport]` on trucks, towing,
road speeds, trucks drawing from an HQ, the push-limit reserve: see spec 15.

## Tune (M3A1 capacity 12)

Owner decision applied first: twelfth slot, tests `halftracks` (cap 12) and `transport`
(`a_squad_that_does_not_fit_is_refused`: the 12-man squad now fits alone, the refusal is for a pair; the first
squad in id order that fits wins), canonical snapshot re-accepted (only the M3A1 changed). The sim is unchanged.
Remeasure, 3 x 160: Germany 43.75 / 45.0 / 47.5 %, pooled 45.42 % (218 / 480); 0 no-winner. T41b (USA 1.13) and
T50e (Germany +2.3 s) now pass; T70a / T70c miss on 101-180 only (USA 0.86 / 0.93), report type. Hard misses: T5c (one
set, pool in band) and T33a (35.95 against 35, S8b-26). The tuner was not run: nothing a price move could fix
without risking the pooled Germany reading, and the owner rule forbids touching existing cards.
