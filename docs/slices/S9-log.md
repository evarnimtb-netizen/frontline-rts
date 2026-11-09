# S9 log: AT guns

Branch `s9-at-guns` (from 1ad2b52; not pushed, not merged). Spec `S9-at-guns.md` (sections 1-22), targets `targets/S9.toml` (267 targets), owner questions S9-1 to S9-19 in `docs/TODO.md`.

Commits: Plan 5557d6d. Build A 3e52d96, 366add7. Build B e9eec2a, c3c0027. Build C 32e78fc, 9c1407f, 81dd57d, 3a4151d. Tune 89b05cd (docs only). Fix 682880e, 165878a. Review made no commit. Docs: this commit.
Versions: SAVE_VERSION 19 -> 20 (Build A), PROTOCOL_VERSION 11 -> 12 (Build B), each bumped once. `sdk/docs/net/relay-protocol.md` updated.

Owner decisions in force: vehicle spotting stays 2x (tanks see tanks at about 350 m); AT guns get a 250 % optic bonus inside the arc, applied as 175 m x 250 % = 437 m, with no size or moving boost (S9-1 (A)); capture as designed (kill the crew and the gun turns neutral; an enemy squad serves it only with rounds from the gun's own faction); T33 counts tanks and TDs only (S8b-26, closed; `tank_td_mp_pct`). Owner also chose S9-2 (A) tow pays with the card, S9-3 (A) M3A1 for the 57 mm, S9-13 (A) cap 2 from 0:00.

## What was built

**Gun entity.** A gun is `EntityKind::Vehicle` with a squad of one and `crew_model = "served"`, so reports and `kills_by_vehicle` work unchanged. Crew are ordinary soldiers in `[[service]]` slots (`Unit::serving`, `Serving { gun, slot }`). Each tick the slots mirror into `vs.seats` (hp, swap, untrained), so gunnery's manned, reload and spread rules and the spotting commander rule run unchanged. Lost men leave the slot, the gunner and loader swap, and men walk back to their slots.

**Shield.** The design's single carriage box would swallow kneeling men and stop flank bullets, so it is split into thin `axle` and `trail` boxes. A validator rejects any slot capsule inside a box. Done-when 1 depends on this.

**Parts and state.** Parts `Gun`, `Sight`, `Recoil` (COMPONENTS 12). A Gun part at 0 HP wrecks the gun (state Wrecked). Recoil out stops firing. Sight out or damaged scales spread. Immobilised reads the wheels, not the driver seat (S8b trap).

**Optic.** Inside the arc, sight range is max(normal range, 437 m) with the woods factor. Outside the arc the normal range applies. `[optic]` in the def.

**Hold fire.** `hold_fire_m` keeps the gun laying and holds fire until struck. Since Build C an ambush holds only against men beyond `[vehicles.guns] ambush_infantry_m` 200 (S9-18), otherwise guns gave themselves away on riflemen at 400 m.

**Found in Build A.** `ditch_dm_of` excluded towed defs, so guns' 6 dm made every ditch stricter (`wheeled.rs` test). `mul_int` overflowed for `Fx` sight times pct, now i64.

**Commands (PROTOCOL 12).** `Emplace`, `PackGun`, `PushGun`, `Hitch`, `Unhitch`, `SetAmbush`. A plain `Move` to a gun squad is refused. Work needs 2 men at the slots. Timings: hitch 20 s, unhitch 20 s, emplace 25-30 s (AI waits accordingly). Recrew orders: see Fix H2.

**Tow.** A hitched gun follows a rigid bar (`tow_bar_cm`) at 80 % of the tow's speed. A card with a tow pays gun + tow (Pak 40 560 MP; 57 mm 650 MP = 340 + 310); the tow arrives hitched with the crew riding. Pak 38 arrives limbered and is pushed. Tow death, bail or capture leaves the gun Limbered and counts `tows_lost`.

**Neutral and capture.** An unmanned gun turns neutral after `abandon_s` if nobody is within `hold_m`. A neutral gun is `EntityKind::Abandoned` with the last owner in the column, seen by all, outside pop. `Recrew` serves an own unmanned gun or a neutral one, and capture sets owner, seen_by and a new squad of one. Counters: `crew.captured` and `s9.guns_captured`, per-player accounts use the S9 counters.

**Rounds and crates.** `source_for` takes the gun's def faction: structures, trucks and crates must match it. Captured crates flip owner on pick-up. Base rearm is refused for captured guns. `stock_in` skips a crate of another faction than the structure (Fix M1).

**Pit.** `gun_pit`, 55 cm (`dig_profiles.gun_pit`, `DigProfile::GunPit`), under `wheel_block_dm` 6, so no ramp. Men dig from the rim (`dig.rs::pit_rim`); the line sits under the gun, so the old approach never arrived.

**Content (mods/ww2).** Pak 40 (`ger_pak40`, weapon `pak40_l46`, 560 MP). 57 mm M1 (`usa_57mm_m1`, `m1_57mm`, `57mm_m86` AP only; S9-14). Pak 38 (`ai_weight` 0, second line). 3-inch M5: data only, emplaced only, speed 0. Sd.Kfz. 11 tractor (`[transport]`, 180 MP, verify). Crews as `ger_*`/`usa_*` squads, cards `max_alive` 2. Roster rows and figures without a design source are marked verify.

**AI (`ai/guns.rs`, `[ai.guns]`).** Own budget: `gun_pot` sets aside 35 % of income so far, from 2:00 or a seen vehicle, `max_guns` 2. One saved job per gun: Site (sight lines to six points on the approach, then cover), Go, Unhitch, Emplace, Hold (`SetAmbush` 400, re-lay onto an armoured target outside the arc), Pit (uncovered guns, engineers within 350 m), Recrew. `Planner::take_guns` keeps guns, tows and crews out of the other habits. Fix H3: the tractor returns to the transport habits, and idle guns re-site (`resite_idle_s` 120, `resite_max` 3): pack, fetch a free tow (never a truck), hitch, restart at Site.

**Tanks fire HE at the crew.** A live gun with a manned slot is an HE aim at its first manned slot (`gunnery::gun_target`), from tanks and other guns.

**Presenter.** Snapshot fields (`gun_state`, `gun_men`, `gun_slots`, `hitched`, `towing`, `hold_fire_m`), six intents, gun blockout and crew at slots, arc rays on a selected gun, gun grid on Y/L/V/U/B/M (existing actions), neutral cause `gun_left`, scene `armour_guns`. The mock has no gun.

## Checks

- Armour (`armour-test --seeds 5`, cases 33-40 in `armour_test/s9.rs`): all hard checks pass at Build B and C.
- Determinism: `ai_guns_emplace_and_are_deterministic` (1/2/3/8 threads, save/load at 5:00) and a whole-life gun test (ambush, pack, push, hitch, tow, unhitch, emplace, re-lay, neutral, capture) with save/load every 20 s, equal hashes. Per-field hash tests extended; load validation for every new field.
- Release (`--include-ignored`): Fix 676 passed, 0 failed, 0 ignored (`scratchpad/s9/fix_release.txt`).
- `check.sh quick 1ad2b52`: clippy ok, 640 passed (Fix run). fmt was fixed with `cargo fmt`; the release suite ran on the formatted code.
- Godot (Build C): check_project OK, test_match_view 339/339, test_net_ui 143/143, test_data_reload 21/21. `settings.cfg` unchanged.

## Armour cases (last run: Build B/C; the Fix stage changed target order for served guns, and these were not re-measured)

| Done-when | Case | Result | Target |
|---|---|---|---|
| 1 front losses | 33 (T73a) | 22.0 % of open crew, shield stops 9.2 % of rounds | at most 30 % |
| 1 flank 45 deg | 33 (T73b) | 3.62x frontal | at least 3x |
| 2 Pak 40 fires first | 34 (T74a) | 100 % of 50 runs, 1.24 penetrations per disable | at least 80 % |
| 2 Pak 40 disables | 34 (T74b) | 100 % | at least 60 % |
| 2 57 mm v Pz IV | 35 | fires first 100 %, disables 96 %, 2.9 penetrations per disable | report |
| HE at crew | 36 (T75) | 69 of 69 HE rounds at the crew, silenced in 25 of 30 runs, mean 35 s | report |
| optic | 37 (T76) | in arc 430 m, out of arc 340 m | in-arc 400, out 350 (T76b max raised from 175; tank 200 % size factor) |
| swap, neutral, capture | 38 (T77) | swap 3.0 s, neutral 10.0 s, enemy serves 100 %, own crate 0 rounds, gun-faction crate 35 rounds | as spec |
| emplace, pack, push, hitch, re-lay | 39 (T78) | emplace 27.5 s, pack 17.0 s, push 37.5 cm/s, towed 660 cm/s, re-lay 13.5 s, crew riding 100 % | as spec |
| pit | 40 (T79) | carriage hits reaching gun in pit 0 % (open 48 over 2 seeds), pit crew losses 2.2 % of open | 20 % |

Case 34's 100 % is a clean head-on case with a blind tank; it sets no balance expectation. The 57 mm is AP only.

## Balance (3 x 160 sets: seeds 1-80, 101-180, 201-280; pooled 480)

Germany pooled **45.6 %** after Fix (sets 48.75 / 46.25 / 41.9; band 45-55; S8b 46.5). Build C was 37.7 % (sets 42.5 / 34.4 / 36.3), and the Tune could not move it (table below). The Fix stage got back into band by re-siting, the tractor and the transport habits, not by tuning values (Fix did not tune; `income_max_pct` stayed 35).

| Pooled Germany % (Tune, Build C code, 480 matches) | |
|---|---|
| Build C as it stood | 37.7 |
| No guns bought (`max_guns` 0) | 46.5 |
| One gun alive per side (`max_guns` 1) | 36.9 |
| Sd.Kfz. 11 free | 41.7 |
| Germany buys no guns | 72.1 |
| USA buys no guns | 17.5 |
| Best tuner corner (tractor 90, 57 mm 460) | 38.1 |

Autotune run 1 (760 matches) and run 2 (`income_max_pct` 10-35) are in spec section 22; the 50 % level was not resolved. Run 2's "10 gives 37.5" came from the tuner's seeds and did not hold on the pooled sets (Review).

### Targets (final build, `frontline-cli targets --from-json`)

- Fix run, three 160 sets: **154 pass, 15 miss (14 hard), 98 skipped** (not played in that run). 40 set: 12 pass, 0 miss, Germany 45.0 %.
- Hard misses, all 160 sets: T8a, T8b HE share 18.6 (floor 20); T31a, T31d vehicle kill share 30.3-30.4 (cap 30); T40a-d and T72a-b half-tracks bought 40-56 % (floor 60); T54d M4 MP share 9.94 (floor 10); T54e StuG bought 73 % (floor 80); T54f M10 bought 69 % (floor 75); T55b M8 kill share 10.33 (cap 10). Report miss: T8d.
- S9 targets pass: T5c, T5d (balance); T32 top vehicle share 13.6 / 10.3 (cap 18); T33 tank/TD manpower 17.9 / 16.0 (cap 35); T80 gun fired in matches GER 74-78 %, USA 69-72 % (floor 60); T81 cover GER 83-85 %, USA 78-86 % (floor 60); T82 guns bought; T83 gun MP 17.4 / 19.3 % (cap 20); T84-T86 counts; T87 `gun_ai_fails` 98 / 63 per 160 matches (report only, not yet analysed).
- Other: T14a tick limit passes on the final run (below).

Gun numbers, pooled 480 (Fix build): tank knock-outs by guns 36 GER, 16 USA (0.08 and 0.03 per match), from about 1955 guns bought. Gun rounds per match 7.6 GER, 10.1 USA. Crew lost per match 1.4 and 1.8 (set 1). Neutral guns 0.12 per match each. Captured guns 0.025 GER, 0.05 USA per match. Tows lost 23 GER / 32 USA per 160 matches. Re-sites ordered 378 GER / 237 USA over 480. Gun MP 17.4 % / 19.3 % (Build C 16.9 / 18.7).

Guns still earn little. That is S9-17. The commit subject "AI guns earn their keep" overstates what the Fix shows; it is kept because history is not rewritten.

## Review findings and fixes (Review on 89b05cd, Fix on 682880e)

- **H1** A wrecked gun that is still hitched made every later save unloadable (`unlink_tow` left state Hitched; validation requires `on_tow == hitched`). Fix: state Limbered, timer 0. Test: a wrecked hitched gun saves and loads.
- **H2** A stale Recrew let one player's squad crew another player's gun (save unloadable), and two own squads overwrote `sv.crew`. Fix: Recrew stays only for an Abandoned gun or `own_gun_unserved(owner)`, which also excludes Hitched and Hitching; the same condition is inside `ready`. Test: the review's cross-owner case.
- **H3** Guns cost manpower and kill little (see Balance). Fix: tractor back to transport habits, idle guns re-site (S9-17 part 1). The corridor search is open (S9-17).
- **M1** Captured crates merged into the captor's own stock. Fix: `stock_in` skips another faction's crate.
- **M2** An ambushing gun froze on a nearer vehicle outside its arc. Fix: in-arc targets sort first for served guns; the AI re-lays when `lay_turn` is set.
- **M3** Embark did not release a crew from its gun (the transport seated them while Status pulled them to the slots). Fix: `crew_leaves_guns` from `order_embark`, except onto the tow of the gun they crew.
- **M4** The T81 cover miss: site scoring and the cover check used different sight lines, and a tow stopped up to 15 m short. Fix: the job stores `point` and `site_cover`; a covered site counts as reached only within `arrive_cover_m` 5 m. T81 definition unchanged; T81 passes now.
- **L1** A gun card's pop check omitted its crew. Fix: crew added to `tow_pop`, with a test.

Review checked and found no fault with: SAVE 20 / PROTOCOL 12 single bumps; no floats, HashMap or faction-id branches in the sim diff; Tune commit is docs only.

## Tick time

Baseline (verify-src `timing40`): p99 0.441 all / 0.532 peak.
- Build C clean run: 0.653 / 0.769 (+48 % / +45 %). `sample` on six long matches found no gun function in the top stacks (1-2 of about 19000 samples).
- Review, two clean runs alone: 0.414 / 0.519 and 0.419 / 0.519. The Build C rise did not reproduce, so it is not a property of the code.
- Fix, clean `timing40` at `--jobs 3`, alone: 0.487 / 0.602 (+10 % / +13 %), under the 25 % profiling trigger. T14a passes.
- Orchestrator verify on `fee52a7`, same clean run: 0.433 / 0.534, level with the baseline. 40 of 40 hashes, 676 tests, 252 of 267 targets, pool 45.62 % (219 of 480).

Build C's 0.653 is unexplained; the later clean runs do not show it.

## Known gaps (not done in S9)

- Balance: Germany 45.6 pooled but 14 hard target misses on the 160 sets (S9-19). The T40/T54/T72 misses coincide with the gun budget (Build C reading, not isolated).
- Guns kill few tanks (S9-17): no corridor search, the AI does not re-site on contact.
- Armour cases not re-run after Fix M2 (target order).
- Not verified: the eliminated-player path (a surrender ends a 1v1); per-field hash coverage beyond the tests named above; net loopback for the new commands is covered by the release run only.
- 57 mm AP only (S9-14); 3-inch M5 emplaced only, data only; Pak 38 second-line; Sd.Kfz. 11 figures verify.
- Guns path on the wheel block layer of 10 dm, not their own 6 dm (S9-16).
- Pushed guns are excluded from the stuck detector.

## What the next slice must know

- A served gun does not move as a vehicle: `move_vehicles` gives it throttle 0; movement is through Pack, Push, Hitch and the tow. A gun does not pivot through the vehicle follower; `gun_relay` runs from `lay_turn` and `vs.face`.
- `Served.crew` links the crew squad. A squad order breaks it, and an idle crew walks back. Use Unhitch, wait for Limbered, then Emplace.
- A bare neutral gun has no squad; AI bookkeeping must allow it. Abandon does not act on guns.
- Guns spawned by tools arrive Emplaced; call-in cards arrive Hitched or Limbered.
- Crew deaths clear the slot at once; a Hitching/Emplacing timer stalls silently with fewer than 2 men.
- Captured guns: rounds only from the gun's own faction (crates, structures, trucks).
- `ambush_infantry_m` 200 is a rule, not a bug: counts of firing at all are what the done-when measures.
- Case 40 uses one gun per round because one penetration can wreck the gun at once.
- Target misses and the open decision are in S9-19 (`docs/TODO.md`).
