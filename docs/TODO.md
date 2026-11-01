# Frontline RTS: owner to-do queue

Owner requests and known issues. Each item names the slice that handles
it.

## Queued (owner requests, 2026-10-31)

- [x] **Individual soldier selection and orders**, GoH-inspired. Spec:
  `docs/slices/S1b-soldier-control.md`. Built in S1b (see the known issues
  below for what is still rough).
- [x] **Call-in panel on the right side** of the screen. Built in S1b: a
  right-anchored column above the orders.
- [x] **Smaller top bar.** Built in S1b: 28 px, with the capture chips in
  it.
- [x] **HUD direction, decided by the owner 2026-10-31: a docked console, but
  lighter.**
  - **Console:** a translucent bottom home for the roster with per-man pips,
    the soldier detail panel, and the orders, stance and ability buttons. Not
    a heavy solid slab.
  - **Minimap:** free-standing, larger than today's 220 px, and lightly
    framed. The owner suggested the top-left corner; that is being weighed
    against bottom-left. It was **north-up by default**, with an optional
    toggle to rotate with the camera (decided 2026-10-31). Since 2026-11-01
    rotating with the camera is the default, drawn as the turned map alone
    with no disc or hatch behind it (owner request).
  - **Capture points:** the A–E buttons move into the top bar, with capture
    progress drawn inside each button (owner request).
  - **Call-in:** a column at the bottom right, just above or beside the
    orders. The tiles always show the name and cost, and the queue is a
    compact bar with a count. (Since 2026-11-01 there is no queue: see
    "Call-in without a queue" below.)

  Spec: `docs/slices/S1b-hud.md`. Mocks: `docs/slices/mocks/`. Built in S1b.
- [x] **Squad card sized to its content.** Built in S1b
  (`roster.card_width`). Today the card has a fixed width
  and cuts long names short ("Granatwerf…"). The card should grow to fit the
  name and the state text, with a sensible maximum, and only then shorten
  the text.
- [x] **Ability buttons: throw grenade and similar actions.** Built in S1b
  (`docs/slices/S1b-abilities.md`): G frag, T smoke, F satchel; the AI
  throws too, but rarely (see below). They can go on
  the same panel as the orders or elsewhere on the HUD, depending on the
  layout decision. They need sim support:
  - a per-soldier ammo or inventory count;
  - a ballistic frag-grenade throw, about 30 m, with HE blast reusing the
    mortar code;
  - smoke grenades that block LOS for spotting;
  - possibly a satchel charge for engineers.

  Each ability is a `Command` aimed at a position, with a hotkey and a
  cooldown or count shown on the button. Write this up as a spec next to
  S1b; the AI should use grenades as well.

- [x] **Two-player multiplayer (S2).** Spec and slice log:
  `docs/slices/S2-multiplayer.md`. Host or join by IP:port, an in-game
  relay or `frontline-relay` on a server, lobby, relay-closed lockstep,
  pause, hash checks, desync dumps and `desync-diff`, 60 s timeout
  surrender, and a macOS zip (`tools/export_macos.sh`). Built on the
  spec's (A) defaults; the input delay and playout formula was revised
  in the fix pass (owner call, see below). Guide:
  `docs/net/playing-online.md`.
- [ ] **A more military UI look (owner, 2026-10-31).** "This one looks like
  something from GMod or 'the office'. Way too corporate. The look itself is
  good, just not the right mood." Keep the HUD v2 layout and its
  readability; change the mood.
  - Three moods were mocked (field manual, olive steel, map table) and a
    blend of the best parts was revised as HUD v2.3 "Map table": felt
    panels, paper minimap, grease-pencil objectives. Mocks:
    `docs/slices/mocks/mood/`; theme spec: `docs/slices/S1b-ui-theme.md`.
  - Open owner question: the blend's panels are 0.92–0.95 opaque, while
    the owner asked for a lighter, see-through console (0.38–0.55).
  - Applied through theme constants (`frontline_theme.tres`), so it is a
    theme pass, not a re-layout.

- [x] **Tanks and armour (S3).** Spec: `docs/slices/S3-armour.md`; results
  and the fix pass: `docs/slices/S3-log.md`. Panzer IV and Sherman, the
  Panzerschreck and Bazooka teams, per-plate penetration, crew and
  components, fire and ammo rack, wrecks, the AT ditch, escorts, vehicle
  AI, `ballistics` and `armour-test`, net play with vehicles, the HUD and
  presenter additions. Built on the spec's (A) defaults; all ten
  agent-checkable "Done when" items pass.

## S3 owner questions (pending; work used the (A) defaults)

- [ ] **1. Scope.** (A) Tanks and AT teams now; tank destroyers, sticky
  satchels, bail-out crews and AI AT-ditch digging in S3b. (B) The full
  draft in one slice.
- [ ] **2. Vehicle spotting range.** (A) 2× the observer's sight (tanks
  see tanks at about 350 m). (B) About 4× (600–700 m).
- [ ] **3. Do standard trenches stop tanks?** (A) No: they slow tanks to
  1 m/s and can bog them; the AT ditch stops them. (B) Any finished trench
  blocks vehicles.
- [ ] **4. Unlock and limit.** (A) Vehicles from 4:00, at most 4 alive or
  queued per player, 12 pop each. (B) No unlock, cost only. (C) Other
  numbers. (S3b: no queue any more, so "at most 4 alive"; a tank also has
  a 120 s re-call cooldown.)
- [ ] **5. Target for "tanks fix the trench assault"** (armour-test case
  7). (A) Attacker losses at most half the no-tank baseline's, and
  defenders lose at least 75 %. (B) A different bar. Case 7 now **misses**
  both (attackers 74 of 80 vs 80 of 80 without the tank; defenders 68.8 %).
  If the target stays, pick the next lever: tank fear, mortar smoke, or
  stronger pinning (S3-log.md "Case 7 vs target").
  **S3b (2026-11-01):** met on (A) with the tank on Suppress (pinning
  fire, owner's choice): attackers 39 of 160 vs 160 without the tank,
  defenders lose 100 % (seeds 1-10, `docs/slices/S3b-log.md`).
- [ ] **Confirm the narrowed case 4 hard check** (fix pass): "at least
  one rocket hit per run" now covers the flank and front runs. The HE
  variant, where the team is under HE fire and hits 57 % at 100 m, prints
  its no-hit runs instead of failing.

## S3b-balance (done 2026-11-01, branch `s3b-balance`, pushed; owner questions open)

Spec: `docs/slices/S3b-balance.md` (baseline, diagnosis, damage audit,
targets); results and the final targets table: `docs/slices/S3b-log.md`
"Fix stage". Owner decisions of 2026-11-01 in force: pinning fire as rules
data (no mortar smoke, no tank fear), physical contact only, realism
first then costs then historical options then stats, tuning values in
TOML. Work proceeds on the (A) defaults below.

- [ ] **S3b-1. Suppress hotkey.** (A) P. (B) Another free key.
- [ ] **S3b-2. Ammo for suppressive fire.** (A) Infantry pay in rate
  only (no infantry ammo pool exists); tanks spend real HE with a
  reserve. (B) Add an infantry ammo pool (own slice).
- [ ] **S3b-3. Ducking in the open.** (A) Duck state only for men in
  trenches; open-ground suppression unchanged. (B) Men in the open duck
  too.
- [ ] **S3b-4. Sherman stowage.** (A) M4A3 (75) dry stowage (sponson
  racks). (B) M4A3 (75)W wet stowage (fewer rack fires, stronger
  Sherman).
- [x] **S3b-5. Bail-out.** Owner 2026-11-01: (B) build bail-out (S3c). (A) Still out; fewer than 2 crew alive
  knocks the tank out, survivors lost. (B) Build bail-out now.
- [ ] **S3b-6. Moving-target misses.** (A) Lead estimation error per
  engagement. (B) Only a larger moving-target spread.
- [ ] **S3b-7. PzGr 40 APCR.** (A) Build it (6 rounds, small effect).
  (B) Skip.
- [ ] **S3b-8. Towed AT guns** (Pak 40, 57 mm M1). (A) Next slice
  (S3c). (B) Not planned.
- [ ] **S3b-9. Veterancy hook.** (A) `steadiness_pct` per squad type,
  all 100. (B) No hook yet.
- [ ] **S3b-10. Cost changes.** (A) Tune may move tank costs within
  ±15 % and HE loads to reach the win-rate target, logged. (B) Ask
  first.
- [ ] **S3b-11. Harness orders.** (A) Cases 5b and 7 order the tank to
  Suppress the trench; old runs stay as report rows. (B) Keep the old
  harness.
- [ ] **S3b-12. Transmission component.** (A) Add it. (B) Fold it into
  the engine.
- [ ] **S3b-13. Data reload in net matches** (Tooling stage). (A) Not
  offered: F5 / "Reload data & restart" is debug-only and single-player;
  in a lobby, data that differs is refused (`content_mismatch`). (B) A
  synced reload for both players.
- [ ] **S3b-14. Penetration variation as data** (Tooling stage). The ±7 %
  band (`armour.rs` `PEN_BAND`) stays in code: it is part of the
  closed-form penetration maths pinned by the golden ballistics file.
  (A) Leave it in code until a tuning need appears. (B) Move it to
  `[vehicles.gunnery]` and thread it through the armour API and the
  golden runner.

- [x] **Call-in without a queue (owner, 2026-11-01).** "Remove the queue,
  but keep the timer of the specific unit after it was called. Like in
  GOH." and "instead of a delivery time, it would spawn immediately, but
  it will be balanced out by the need of the unit to move to the
  frontline itself". Built in the S3b call-in stage
  (`docs/slices/S3b-balance.md` "Call-in without a queue"): a buy spawns
  the squad in the base at once, then that type waits out its re-call
  cooldown (rifle 24 s, engineers and MG 36 s, AT team 48 s, mortar 60 s,
  tank 120 s, set by the owner, in each unit or vehicle def); refusals
  say why in the feed; Tab then 1–9 buys a tile. `PROTOCOL_VERSION` 4.
- [ ] **S3b-15. Call-in hotkeys.** (A) Tab focuses the column, then 1–9
  buy that tile (outside the column the number keys stay control
  groups). (B) Direct keys without focus (e.g. F1–F7, or Shift+1–9).
- [ ] **S3b-16. Cooldown groups.** Built (`cooldown_group` in a unit def:
  types of one group share a timer, as in the reference game) but unused.
  (A) Each type keeps its own timer. (B) Group related types (e.g. MG and
  mortar as support weapons).
- [ ] **S3b-17. AI tank reserve.** (A) After the vehicle unlock, under
  the limit, the AI's infantry buys leave 200 MP unspent
  (`[ai.armour] tank_reserve_mp`). (B) No reserve, or a larger one.
- [ ] **S3b-18. Spawning with enemies in the base.** (A) As now: squads
  spawn at the base's seven spots whatever is near (the base has no
  protection rule). (B) Refuse or move a spawn when enemies are within
  some distance of the base.

Part A stage (2026-11-01, `docs/slices/S3b-balance.md` "Part A: physical
damage, armour, gunnery and content (stage done)"): new questions, work
proceeds on (A).

- [ ] **S3b-19. Grenade fragments and blast.** The spec's starting values
  (blast 1 m, 48 fragments, 2 per fragment on tracks) left grenades thrown
  into a trench with a third to a half of their old effect. (A) Blast 2 m
  (the old lethal radius, the owner's "about lethal radius"), 96
  fragments, 0 on tracks; Tune measures. (B) The spec's values.
- [ ] **S3b-20. Fragments on tracks.** At 5 per fragment a mortar bomb
  1 m from a hull broke the track by fragments alone (about 20 strike the
  band). (A) 1 per fragment (0 for grenades); the overpressure still
  breaks tracks as before. (B) 5, and tracks break more often.
- [ ] **S3b-21. Moving-target misses.** The lead estimation error (S3b-6
  (A)) is in, but at 300 m a crossing tank's lead (2.9 m at 6 m/s) is
  less than half its 5.8 m length, so even a wrong lead hits: case 3d is
  still 86 % of stationary (40 % error and 0 % give the same, 200 % gives
  82 %). (A) The Tune stage raises `moving_target_spread_pct` (the
  spec's second lever). (B) Add a range-estimation error (the main WW2
  miss on movers) per engagement, in a later slice.
- [ ] **S3b-22. HE against a parapet, the metric.** The spoil rule (A7)
  works when a burst is centred on the parapet (unit-tested: up to
  0.45 m off), but case 9's lip metric averages the lip over 41 m, so one
  0.6 m crater barely moves it (0.0024 m per burst vs the 0.05 target).
  (A) The Tune stage adds a local report row (lip lost within 1 m of each
  burst) before judging T14. (B) Widen the rule (spoil flies from a
  larger radius).
- [ ] **S3b-23. Mortar lethality.** With fragments, a 10-round barrage
  kills 58 in the open (was 74, -22 %, just outside the spec's ±20 %) and
  6 in a trench (was 28). (A) The Tune stage moves the mortar's
  `frag_count`/`frag_damage` to bring the open number within ±20 %;
  trenches protecting this much against mortars is realistic and kept.
  (B) Keep as is.

Part B stage (2026-11-01, `docs/slices/S3b-balance.md` "Part B: AI and
trench pinning (stage done)"): new questions, work proceeds on (A).

- [ ] **S3b-24. Near misses measured to the body.** A near miss used to be
  a round passing within 2 m of a man's middle; a crouched man in a
  trench has his middle 1 m below ground, so rounds cracking over his head
  or striking the parapet in front of his face did not count (an MG at
  150 m gave each man 0.2 near misses a second). (A) Measure to the hit
  capsule (head to feet), as built; it also adds some near misses in the
  open. (B) Back to the middle and tune the duck values only.
- [ ] **S3b-25. Covered near misses.** `covered_near_miss_pct` 50 -> 100:
  the trench protects by geometry already (rounds cannot hit a man below
  the lip), and halving suppression on top made a trench line
  unpinnable. Rifle fortify rows unchanged (trench 0 vs open 80). (A)
  100, as built. (B) 50 and lower duck thresholds instead.
- [ ] **S3b-26. Duck values.** Measured, an MG on a trench line at 150 m
  gives each man about 0.5 near misses a second (wide MG cone, grazing
  fire into the ground), far below the spec's estimate of 3; with 35 / 20
  a line ducked 12-17 % of the time. (A) `duck_points` 25,
  `duck_release_points` 10, `duck_decay_pct` 25 (a ducked man keeps his
  head down ~6 s after the last near miss), `aim_cm` 80: ~60 % ducked and
  ~29 % pinned under an MG. (B) The spec's 35 / 20 / 100 / 30.
- [ ] **S3b-27. Tank HE at a trench kills nobody** (T9's "kills > 0"
  missed). A flat-trajectory shell cannot drop into a 1.6 m trench; it
  bursts on the parapet or behind, and the parapet stops the fragments
  (the physical-contact rule working). (A) Accept: tanks pin (5b: 97 %
  of man-time pinned or ducked), grenades and assaults kill; T9 becomes
  "pins". (B) Make HE bursts over a trench lethal (air-burst or a
  larger close-in blast; against the owner's physical-contact rule).
- [ ] **S3b-28. Harness: attackers advance crouched** in the new
  fortify-test assault rows (both variants alike), as a player would
  under fire. Standing, the MG-supported squad lost 100 %. (A) Crouched.
  (B) Standing (the old rows' style).
- [ ] **S3b-29. B6 angling** (`ai_face_angle_deg` per vehicle def) is not
  built in Part B. (A) The Tune stage adds it if T2 (frontal duel) is
  still missed; it is a data key plus a turn in the AI's facing. (B)
  Drop it.

Tune stage (2026-11-01, `docs/slices/S3b-balance.md` "Tune results"): new
questions, work proceeds on (A).

- [x] **S3b-30. 75 mm M61 penetration table.** Superseded in the Fix stage
  (review H2): all three AP rounds now use Bird & Livingston's 0° rows (one
  source and criterion); the M61 beats the Panzer IV H front at 300 m
  90 % of the time. T2 follows in S3b-38. Tune stage text: the def's row (76/70/63/57/51
  mm at 30°) sat about 6 % above the US published table (Hunnicutt: 2.6 in
  at 500 yd, 2.4 at 1000, 2.2 at 1500, 2.0 at 2000). Corrected to it: the
  M61 beats the Panzer IV H's 80 mm front at 300 m 13 % of the time (was
  88 %); golden rows 12-16, 18, 19, 28, 29, 32 moved. With it (and the
  filler reach below) the Panzer IV wins case 3a 6 of 10 (32 of 40 over
  seeds 1-20). (A) Keep the published table. (B) Restore the S3 row and
  find the Panzer IV's frontal edge elsewhere (S3b-29 angling, optics).
- [ ] **S3b-31. APHE filler reach 150 -> 300 cm** (PzGr 39, M61; as the
  3 m spall reach). At 150 the rays from a burst 1 m behind the front
  plate ended in the empty middle of the hull and tanks survived 5-12
  frontal penetrations. (A) 300. (B) 150 and more crew/part boxes.
- [ ] **S3b-32. Turret basket.** Turret crew boxes now reach down into the
  hull (gunner and commander seated, loader standing on the basket
  floor), and validation lets a turret-frame part reach down to the hull
  floor. A hull hit at visible-mass height used to pass under all three.
  Side effect: case 4's flank disabling-hit rate fell 41 -> 34 % (the jet
  now kills the gunner or loader instead of reaching a part). (A) Keep.
  (B) Turret crew inside the turret box again.
- [ ] **S3b-33. Assault harness: close into the trench.** fortify-test's
  suppressed assault and case 7 (and 7b) now close into the trench after
  the grenade wave (all frags out or 10 s), and the MG / tank lifts its
  Suppress when the squad's lead is within `ai.assault.lift_fire_m`
  (15 m); 7c keeps the S3 run. Before, the squad stayed at 25 m, where
  ducked defenders cannot be hit, so T8/T10 could not be met by any
  value. (A) Keep (fire and move, grenades, then in). (B) The old rows.
- [ ] **S3b-34. Tank costs.** Fix stage: Panzer IV 620, Sherman 590 (S3b-10
  (A), within 15 %; the Tune stage's 600 / 620 gave base 1 70 % of wins).
  40 matches each on the Fix data: 640/560 Germany 12 / USA 27; 600/620
  18 / 22 (base 1 70 %); 620/590 17 / 21 (base 1 55 %). Over 80 matches
  Germany 31, USA 47, 2 draws. (A) Keep 620 / 590. (B) Revert to S3 and
  accept the faction gap.
- [ ] **S3b-35. Case 4 flank target (T5, missed: 34 %).** A flank rocket
  at visible-mass height crosses the turret-basket crew; the Panzer IV's
  rack bins (low, per Panzer Tracts) and its turret ring lie below and
  above that line, and crew kills do not count as "disabling". (A) Keep
  the metric and the miss; check the Panzer IV H side stowage height
  against a source (owner / M2 citation task) before moving any box.
  (B) Count a kill of the gunner or driver as disabling.
- [ ] **S3b-36. AI satchels at vehicles (T7, missed: 0-1 of 10).** Satchel
  hunts start (up to 5 a match at `satchel_hunt_m` 150) but end before a
  throw: the vehicle moves off, drops out of sight, or the engineers die
  on the 100+ m walk. Data (slow speed 2 m/s, 2 s, hunt 250 m) did not
  help. (A) Next slice: a sticky hunt (keep the last seen vehicle for a
  few seconds) and engineers that move with an escort or under smoke.
  (B) Accept satchels as a player tool only.
  S3c: sticky hunts are Part B2 (`docs/slices/S3c-armour.md`); baseline 96 hunts, 1 throw.
- [ ] **S3b-37. Match length.** Met under the owner's loosened T12 (no
  match under 10 or over 40 min, median 15-30): final 11.2-26.2, median
  18.1. Against the old 12-25 band 5 of 20 fall outside. (A) Nothing to
  do. (B) Revisit with a ticket rule if stalemates appear in play.

Fix stage (2026-11-01, `docs/slices/S3b-log.md` "Fix stage"): new
questions, work proceeds on (A).

- [x] **S3b-38. T2, the 300 m frontal duel.** Owner 2026-11-01: (A) accept near-even; 600 m case in S3c. With consistent penetration
  data both tanks beat each other's front at 300 m; the Panzer IV wins 44
  of 80 over seeds 1-40 (4 of 20 on seeds 1-10). (A) Accept a near-even
  300 m duel as historical; judge the Panzer IV's edge at range in a new
  600 m case next slice. (B) Keep "6-8 of 10" with a step (d) stat change.
- [ ] **S3b-39. Vehicle MG stowage.** Built: Panzer IV H 2 x 1575, M4A3 (75)
  2 x 2375 rounds; a dry MG is silent; base rearm refills a belt per step.
  (A) Keep. (B) No limit.
- [ ] **S3b-40. Base-side bias.** Base 1 wins 58 % of 78 decided matches
  (67 % on seeds 1-10; 70 % in the plan baseline before S3b). (A) Study
  the map and AI routes next slice. (B) Accept.
  S3c: cause found in the map (S3c-8).
- [x] **S3b-41. T10 by faction.** Owner 2026-11-01: (A) AI brings a second base of fire (S3c). US attackers (M1919) lose 74 %, German
  attackers (MG 42) 41 % on 10 seeds; T10 pooled misses (57.5 %). (A)
  Accept the historical rate-of-fire gap, keep T10 pooled with a
  per-faction report; the AI gives a US base of fire a second MG or rifle
  squad (next slice). (B) Tune the M1919 as a step (d) deviation.
- [ ] **S3b-42. AI rearm.** The AI never sends a tank to its base to rearm,
  so dry MGs stay dry. (A) Next slice: rearm when both MGs are dry or HE
  is at the reserve. (B) Leave it.
  S3c: Part B1 (S3c-10).
- [ ] **Follow-ups (next slice).** Bail-out (S3b-5 (B)) to raise rocket
  knock-outs (16 of 221 KOs over 80 matches); sticky satchel hunts
  (S3b-36); towed AT guns (S3b-8); MG stowage on the vehicle card, not only
  the tooltip; ammo causes 64 % of knock-outs (an ammo fire always ends
  the tank).

## S3c-armour (in progress, branch `s3c-armour`)

Spec: `docs/slices/S3c-armour.md` (baseline, base-bias diagnosis, history,
bail-out design, Part A / Part B order, targets). Handles S3b-5 (B),
S3b-36, S3b-38 (A), S3b-40, S3b-41 (A), S3b-42 and the MG-on-card
follow-up. Work proceeds on (A) of each question below.

- [ ] **S3c-1. Bail triggers.** (A) Ammo fire at once, engine fire after
  8 s, fewer than 3 crew, gun and mobility both out, disabled and
  penetrated again within 10 s, manual Abandon. (B) Only fires and crew
  losses.
- [ ] **S3c-2. Capture.** (A) On (`vehicles.recrew.capture`): enemy crews
  or infantry may re-crew an abandoned tank; the AI does not capture in
  S3c. (B) Off. (C) On, and the AI captures too.
- [ ] **S3c-3. Who can re-crew.** (A) Crew squads, and any infantry but AT
  teams as untrained crew (reload 150 %, traverse 75 %, spread 150 %).
  (B) Crew squads only.
- [ ] **S3c-4. Crews in base.** (A) Dismissed after 30 s in base, no
  refund (the base refills crew seats free). (B) Kept until the player
  dismisses them. (C) Refund their men's value.
- [ ] **S3c-5. Pop and vehicle limit.** (A) An abandoned tank counts
  toward nobody; re-crewing needs a free slot and pop. (B) It counts
  toward its last owner until destroyed or captured.
- [ ] **S3c-6. Keys.** (A) K = Crew tool / re-crew (right-click context
  with a crew squad); Bail out only as a card button with a confirm click.
  (B) Other keys.
- [ ] **S3c-7. Crew loadouts and hatches.** (A) Germany MP 40 + P 38s,
  USA two M3 + M1911A1s, no grenades; the M4A3 (75) with the late-1943
  loader's hatch. Exact counts are a citation task. (B) Pistols only.
- [ ] **S3c-8. Base bias** (S3b-40). Cause found: the map's one valley
  band runs only across the south and puts base 0's near point B in a
  hollow 15.6 m below C; base 0 lost 134 tanks to base 1's 87 over 80
  matches, mostly at B, shot from ~10 m higher. A mirrored valley probe:
  110 vs 102, base 1 wins 52.5 %. (A) Mirror the valley (data) and make
  the spawn ring symmetric (code). (B) A point-symmetric terrain flag.
  (C) Leave it.
  Part B: (A) built (mirrored valley; spawn ring turned to face the map
  centre). 80 matches: base 1 won 41 of 80, tank losses base 0 112 / base
  1 123; C held 38 % / 37 %. (B) is built as an option (S3c-24).
- [ ] **S3c-9. Shooting abandoned tanks.** (A) Not auto-targeted;
  `AttackUnit` can; the AI denies enemy abandoned tanks when idle. (B)
  Auto-targeted.
- [ ] **S3c-10. AI rearm** (S3b-42). Measured: AI tanks spend 0.2 % of
  their time with both MGs dry and 1.6 % with HE at the reserve, so this is
  not a balance lever. (A) Rearm at both MGs dry, HE <= reserve or AP <= 3
  with no enemy vehicle near; back at 90 % or after 120 s. (B) Dry MGs only.
- [ ] **S3c-11. Second base of fire** (S3b-41). (A) When the base's fire
  weight (rpm x suppression) is below the enemy MG team's and >= 3 squads
  are assigned. (B) Always two bases with >= 3 squads.
- [ ] **S3c-12. Crew all dead.** (A) Abandoned (intact, re-crewable). (B)
  Destroyed as today.
- [ ] **S3c-13. Match-length runs.** (A) Tune measures with `--minutes 45`
  (the CLI stops at 30 and calls it a draw). (B) Keep 30.
- [ ] **S3c-14. Fewest men to re-crew** (Part A). `min_men` 2 (driver and
  gunner) with `crew_below` 3 would make a two-man crew bail out again the
  tick it boards. (A) Re-crewing needs max(`min_men`, `crew_below`) men
  (3 today); a two-man crew cannot man its tank. (B) `crew_below` only
  counts men lost since the last boarding.
- [ ] **S3c-15. Boarding all at once** (Part A). The spec had men board one
  by one, the vehicle becoming the player's at `min_men`. (A) The squad
  gathers on the hull side (1 m out) and the men at the hull (one per
  empty seat) climb in together, `board_s` × ⌈men / hatches⌉; they stand
  exposed meanwhile; leaving restarts it. No half-crewed abandoned
  vehicles to save, validate or bail out again. (B) One by one as specced.
- [ ] **S3c-16. Abandoned entity kind** (Part A). (A) A new entity kind
  `Abandoned` keeps the last owner in the owner column (the snapshot
  sends owner 255 like a wreck plus `last_owner`), so every live-vehicle
  system skips it without a check; the spec's `NEUTRAL` owner value is not
  used. (B) A `NEUTRAL` owner on a live `Vehicle`.
- [ ] **S3c-17. Bail out button** (Part A). (A) In the vehicle orders grid
  (fifth slot of the orders row, "Again?" for 2 s), next to the card,
  with the other vehicle orders. (B) On the vehicle card itself.
- [ ] **S3c-18. Dismount** (Part A). (A) A man out of the hatch cannot fire
  for `dismount_ms` (his weapon cooldown); he may move at once. (B) Also
  hold him in place.
- [ ] **S3c-19. AttackUnit on abandoned tanks** (Part A). (A) Tanks may be
  ordered to shoot any abandoned vehicle (denial); AT teams keep their
  rockets for live vehicles (`AttackUnit` on an abandoned one gives the
  launcher no target). (B) Rockets too.
- [ ] **S3c-20. Satchels at tanks (T3)** (Part B). Sticky hunts are in,
  but over 80 matches 70 of ~130 hunts ended "spent": each engineer
  squad has one satchel, on its leader, who walks first and falls on the
  ~100 m approach to a tank with MGs and escorts. Probe with a satchel for
  every man: 13 thrown in 4 of 40 matches, hunts then end by the squad
  falling below retreat strength (59). So the approach, not the AI's
  stickiness, limits it. (A) Keep the loadouts; the Tune stage tries a
  satchel pick-up from a fallen carrier by the nearest squadmate (physical,
  like the launcher hand-over), then lowers T3 if it still misses. (B) A
  second satchel per squad (pioneer squads carried several charges). (C)
  Lower T3 now.
- [ ] **S3c-21. AT teams following tanks** (Part B, B3). Measured one at
  a time over 80 matches: with AT teams trailing their own assault tanks
  (150 m) rockets knocked out 33 of 233 tanks; without, 48 of 243 and 260
  rockets fired instead of 216 (measured before the S3c-26 shell fix).
  (A) Off (`at_follow_m = 0`). (B) On.
  The stuck-tank stalk (`at_hunt_stuck_m` 500) made 3 hunts and no
  measurable difference; (A) keep it on.
- [ ] **S3c-22. Fragments over a falling edge (T9)** (Part B, B8). Ground
  bursts threw fragments only 2-45 deg above the horizontal, so a tank HE
  shell on a parapet crest could never reach men below the lip (5b: 0
  kills). Now the ground is sampled 1.5 m around a ground burst and the
  steepest fall adds rays below the band at the band's density
  (`combat.blast.frag_fall_probe_cm`). 5b: 4 of 160 defenders killed (was
  0); fortify T10 attackers lose less (grenades on the lip reach the
  trench too). (A) Keep. (B) Off (0).
- [ ] **S3c-23. Parapet erosion (T14)** (Part B, B8). Checked: a 75 mm HE
  crater is 0.6 m x 0.3 m in compact ground; in spoil the depth share
  (150 %) already gives about 1.5x the volume, inside the 1.5-2x loose-fill
  range of cratering rules, so the crater size is not the cause of 0.035 m
  per burst. (A) No change, T14 stays a report. (B) A radius share for
  spoil (about 2x volume).
- [ ] **S3c-24. Point-symmetric terrain** (Part B, B6). Built as a map key
  (`[terrain] symmetry = "point"`), off on Kessel Ridge: over 80 matches
  the mirrored valley (A) gave base 1 41 of 80 wins, tank losses 112 vs
  123, C held 38 vs 37 %; the symmetric terrain gave 45 of 80 wins and
  106 vs 104 (before the S3c-26 shell fix; after it, with (A): base 1 41
  of 80, losses 190 vs 207). (A) Keep the mirrored valley only. (B) Turn
  symmetry on.
- [ ] **S3c-25. Hunter stance** (Part B, B2). (A) Hunters crouch on the
  approach and crawl within 25 m while the tank's hull MG arc covers them;
  a hunting squad screens itself with its smoke when pinned on the move
  and keeps its satchel for the tank (no satchel at trench infantry). (B)
  Automatic stance.
- [ ] **S3c-26. Shells now fly their full range** (Part B, found by case
  3f). A main-gun shell's lifetime was computed in Q16.16 as `range x
  125` first, which saturated, so every shell of a 600 m gun vanished
  after ~330 m. Fixed (`gunnery::shell_ttl`). Effect over 80 AI matches:
  tanks lost 243 -> 397, rocket knock-outs 48 -> 21, rockets fired in 35
  -> 22 of 40 normal-base matches (T2, T4 now missed); 3a (300 m)
  unchanged; 3f Panzer IV 14 of 20. (A) Keep the fix and re-tune T2/T4 in
  the Tune stage (AT reach, tank standoff, spotting). (B) Also cap tank
  fire at the spotting range in the AI (no change to the physics).
- [ ] **S3c-27. 600 m duel target** (S3b-38 (A)). 3f on seeds 1-10: Panzer
  IV 14 of 20, Sherman 6 (Sherman hull hits penetrate 4 of 10, turret 6
  of 6), staged spotted (two still tanks 600 m apart never see each other:
  sight 175 m x 200 %). (A) Target "Panzer IV >= 6 of 10 at 600 m"
  written. (B) Report only.

- [ ] **S3c-28. Tune results: T2 (rocket share) stays about 10 %**
  (Tune stage). With AT teams doubled in the mix, a looser AT-hunt clearance
  and a longer satchel hunt, rockets and satchels cause 9.7 % of tank
  losses (70 of 725, 160 matches), target 20 %. Cause: after the S3c-26 fix
  tanks kill tanks from 300-600 m, and the model has no AT guns, mines or
  air, which were most of the real tank losses; rockets give about one
  loss per 4.3 penetrations (304 in 160 matches; hits that disable but do not make the crew leave are
  not losses). (A) Keep T2 as a report, with the target moved to 10 % (the
  infantry-AT share in open country was roughly 5-15 % of losses; citation
  task). (B) Keep 20 % and add AT guns or mines (S4+). (C) Make a rocket
  hit more lethal by data (jet `core.damage`); not done, no source.
- [ ] **S3c-29. Panzerfaust** (historical option, Tune stage). A one-shot
  60-100 m launcher in German rifle squads (1944-45) would raise the
  infantry-AT share more than any AI change, without new AI rules. Needs a
  weapon, ammo and loadout def and an AT-hunt use for rifle squads, so it
  belongs with the S4 unit variants. (A) Not now; list it in S4. (B) Add
  it to S3c.
- [ ] **S3c-30. Rearm and satchel targets** (Tune stage). T5 (low-ammo
  returns): 1 in 160 matches, 5 rearm trips; tanks are lost, not empty:
  MG dry time is 0.0 %, HE at the reserve 0.1-0.2 %, and `rearm_ap_below`
  8 gave identical hashes to 3 (no tank reaches 8 AP left). T3: satchels
  thrown at a vehicle in 5 of 80 normal-base matches (1 of 80 before), the
  approach under live MGs kills the carrier (case 8). (A) Move T5 to "trips
  work in tests (rearm trip, 1/2/3/8 threads, save/load) and low-ammo time
  does not rise" and T3 to "at least 1 of 20 normal-base matches", both
  met. (B) Keep T5 and T3 as written and add a satchel pick-up from a fallen
  carrier (sim change; probe with a satchel per man gave only 5 throws in 80
  matches).
- [ ] **S3c-31. HE share of kills (T14)** (Tune stage). Mean 43.8 %, over
  60 % in 1 of 160 matches (seed 35 swapped: 11.5 min, 2 tank losses, 65.5
  %). (A) Read T14 as "mean under 50 % and under 60 % in 99 % of matches".
  (B) Keep "under 60 % per match" (missed by 1 match in 160).

## S3 owner checklist (needs a person)

- [ ] **11. Play a match vs AI to victory** using tanks, an AT team and an
  AT ditch. Note whether the turret speeds, bogging, the unlock time and
  the Sherman's cost feel right.
- [ ] **12. One net match with the friend** where both sides field
  vehicles.
- [ ] **13. Read armour-test case 7 against the target** (owner question
  5) and pick the next lever if needed.

## Known issues found during S1

- [ ] Map balance: the south-west base wins about 75 % of AI-vs-AI matches,
  whichever faction holds it. Mirror the valley, then look for a slot-0
  tick-order bias.
- [ ] Mortars cause most kills (2–4× small arms). Tune infantry fire.
- [ ] GitHub Actions is blocked by account billing, so there is no
  cross-OS CI.
- [ ] From M0:
  - paced ticks on macOS are 4–5× slower (efficiency cores);
  - Regiment drops 34 % of shots (pool full);
  - MPL-2.0 text missing from THIRD_PARTY_NOTICES;
  - Metal reports 0 GPU time.

## Known issues found during S1b

- [ ] **Assaulting a finished trench across open ground barely works.**
  Men in a trench lose nobody to rifle fire, while defenders see attackers
  at 150 m and attackers see them at 90 m. `fortify-test` (5 seeds × both
  factions): from 60 m the attackers are wiped in every run, with or
  without frags. Smoke first, then walking into the cloud to 27 m and
  throwing, costs the defenders 61 of 80 men, but the attackers still lose
  75 of 80 once the smoke thins. Frags from 25 m work (defenders 80 of 80
  lost). The open question is how a player gets to 60 m: suppression that
  really pins trench men, mortar smoke, or longer smoke throws. Needs an
  owner decision.
- [ ] The AI throws rarely: 0–4 items per AI-vs-AI match, no satchel seen in
  any seed, because its fights happen at rifle range.
- [ ] Sim step spikes in windowed Godot runs (max 13–44 ms, 5–13 late
  ticks per 12–18 minute run, nothing else running) that headless runs
  never show (P99 under 0.7 ms, max 1.4 ms). Probably the sim thread being
  descheduled (see the M0 efficiency-core note); not investigated.
- [ ] `--autoplay` lands a frag in an enemy trench in only 1 of 3 complete runs
  (2026-11-01). Assault squads sent across the map die on the way, and a
  thrower standing up for his 1 s wind-up is often shot. The committed
  `s1b_frag_tool.png` and `s1b_frag_trench.png` come from that one run;
  the other S1b shots come from the last full run. Autoplay exits 3 when
  the trench shot is missing.
- [ ] Autoplay pushes clicks and keys into the viewport, so the OS
  window-to-viewport path is untested.
- [ ] Throw streaks and frag blasts are small at the 40–55 m RTS camera
  height; a frag's 0.2 m crater is hard to see.
- [ ] At 1280x800 the roster shows 5 cards and a "+3" chip with 8 squads.
- [ ] A man winding up a throw in a trench stands on the fire step even when
  ordered prone (aiming, not suppression).
- [ ] `MoveUnits` puts all detached men on one line, n × spacing wide (about
  117 m for 40 men). The spec says "a small line".
- [ ] Throw flight is 2–2.7 s at 25–30 m (45° lob, deliberate); the spec
  said about 1 s.
- [ ] Fog of war for thrown items and smoke clouds (visible if ours, if we
  see the thrower, or within 40 m of our men) is our own rule, not from
  the spec.
- [ ] Not measured: the cost of 240 soldier dictionaries in `get_state`, and
  picking + rings on an M3 export build. Headless debug measures
  0.27–0.32 ms against the 0.3 ms budget (the view test's own limit is a
  loose 2 ms).
- [ ] `bench/results` S1 hashes are stale since S1b.
- [ ] Capture chips: the progress fill covers the letter (the "C" chip at
  1280 is hard to read). Draw the fill under the letter or as a bar.
- [ ] Trench lines are hard to see among mortar craters at RTS camera
  height; they need a stronger edge or darker floor shading.
- [x] The fix pass cut the call-in queue from 10 to 5 to match the HUD
  spec. That is a game rule; confirm with the owner. **Closed
  2026-11-01:** the owner removed the queue (S3b call-in stage, below).

## S2 owner checklist (needs people or a second network)

- [ ] **One manual match**, two humans with mouse and keyboard, played to
  victory (spec item 7).
- [ ] **One match over Tailscale or the VPS relay**, with ping, D, playout
  and the "late" count from the readout's tooltip written down (item 8).
  Wired if possible.
- [ ] **The friend runs the zip on their own Mac** (item 9). Also not done
  by an agent: a second macOS account on the same Mac (agents may not
  create accounts) and an Intel Mac.
- [ ] **Decide the D / playout revision** (`S2-multiplayer.md` "Turns"):
  D now covers half the round trip and a client playout delay covers the
  other half. Same click-to-reaction time, almost no stalls, but on very
  lossy links 5–10 % of orders run a tick or more late. Reverting is one
  function (`frontline_net::suggest_d`).

## Known issues found during S2

- [ ] **The zip is an "editor runtime" build**, not a template export:
  the Godot 4.7.2 export templates are not installed (the official
  `.tpz` is 1.28 GB). `tools/export_macos.sh` falls back to a copy of the
  Godot editor app with the game packed in (176 MB zip, debug-capable).
  It runs and plays a net match from the unzipped zip. Install the
  templates (Editor > Manage Export Templates) and rerun the script for
  a proper release export; `--templates-only` refuses the fallback.
- [ ] `frontline-relay` is only type-checked for Linux (as
  `aarch64-linux-android`) and Windows; no glibc Linux build or run yet
  (needs `rustup target add x86_64-unknown-linux-gnu` and a linker, or
  building on the VPS as `playing-online.md` says).
- [ ] Lossy links now cost restamps instead of stalls: at 10–30 % loss,
  5–10 % of orders run a tick or more late. At 0–5 % loss: 0–0.4 %.
- [ ] Two windowed instances on one Mac: about 10 stalls per 3-minute
  match, all in the first seconds (start-up and the S1b tour's
  screenshots). Headless runs show none.
- [ ] App Nap: not hit with both windows visible (partly overlapping);
  the spec's `defaults write ... NSAppSleepDisabled` was not run (no
  persistent defaults). A hidden or minimised instance is untested.
- [ ] After a host **crash** the guest waits about 75 s before the
  disconnect screen (10 s of silence, then the "Reconnecting" countdown).
  The texts now say the match is over if the host's game closed, but the
  wait is still long. A clean host quit is fast: the relay delivers the
  host's surrender before it shuts down (tested on the relay and in the
  bridge over real UDP, `host_leaving_mid_match_surrenders_to_the_guest`;
  not yet seen with a human clicking Leave in Godot).
- [ ] Hosting again right after leaving joins the old net threads on the
  main thread: up to about 1.5 s of frozen menu if the guest vanished
  (it frees the port first). Not measured.
- [ ] Possible IPv6 issue (unverified, no global IPv6 here): a relay bound
  to `[::]` may answer from a temporary address, which the client would
  drop (it only accepts its configured relay address).
- [ ] The waiting overlay has no "leave now / claim the win" button: the
  remaining player waits out the 60 s or leaves through the Esc menu.
- [ ] The pending-order marker is a bare dashed ring; nothing links it to
  the ordered unit. At D = 2 it lives one or two frames.
- [ ] `desync-diff --replay` is not built (spec: out of S2). Dumps now
  always have a checkpoint (tick 0 counts as confirmed).
- [ ] In two-instance autoplay the S1/S1b shots share file names, so the
  two instances overwrite each other's shots (test only).
- [ ] `build_id` is stamped when `crates/net` is rebuilt (HEAD or index
  changes); edits to tracked files without staging do not set `-dirty`
  until something rebuilds it. `tools/export_macos.sh` refuses a dirty
  tree unless `--allow-dirty`.
- [ ] `THIRD_PARTY_NOTICES` not regenerated for `socket2`.

## Known issues found during S3 integration

- [ ] **Net stalls in windowed two-instance runs:** the integration saw
  250–550 stall ticks per 3-minute match with tanks (55–61 without, S2:
  10), in 0.3–1.1 s bursts at the same tick on both instances. The
  performance review then saw 483 stalls before any vehicle existed, and
  971 in a headless run with no vehicles at all, at load average 5–17
  with swap nearly full. The fix pass, at load 3, saw no bursts (hash
  arrival at most 43 ms late). Probably machine pressure, not vehicles;
  re-measure on an idle machine.
- [ ] The S1b frag-into-trench autoplay shot still fails most runs; with
  S3 the seed-1 match also ends earlier (AI won on tickets at 13:06 while
  we spent 1,280 manpower on tanks).
- [ ] Call-in tiles hide the "need N" reason when it is wider than the
  room next to the cost: a vehicle tile with "need 509" shows only
  "640 MP" (greyed).
- [ ] The rocket trail is a straight pale box (9 m, no taper); it reads
  as a white stick from 30 m up.
- [ ] A knocked-out tank lit by its cook-off light shows a flat orange
  patch on the hull for about 0.3 s.
- [ ] Staged scenes all sit on point C (the sim's staging lane runs
  through the map centre).

## Known issues found during the S3 fix pass

Balance (numbers in `docs/slices/S3-log.md`; spec numbers, not tuned
here):
- [ ] **The Sherman wins tank fights:** 26 of 30 staged duels (3a frontal
  6–4, 3b angled 10–0, 3c flank 10–0). In AI matches Germany lost 17 tanks
  and the USA 8, and the USA won 8 of 10. The Panzer IV's 14°/s turret
  loses the race to the first hit (about 1.1 s).
- [ ] **Almost every knock-out is an ammo-rack detonation:** the rack
  (60 HP) sits in the hull centre, and an AP core does 100.
- [ ] **AT teams are weak:** case 6 disabled the tank 0 of 10 times
  despite 56 penetrations; case 4 flank rate 42.9 % (target ≥ 50 %). A
  HEAT jet (1.0 m) through the Panzer IV's hull front stops about 5 cm
  short of the driver and radio-operator boxes (x 115–185 cm, plate at
  290 cm). Levers: 1.5 m jet or crew boxes further forward.
- [ ] Crossing-target hit rate is 83 % of stationary (target ≤ 60 %).
- [ ] A tank in front of a trench kills 0 of 80 dug-in men in 3 minutes
  and pins nobody (case 5); from the trench's end it kills 75 of 80 (case
  11). Tank HE erodes a parapet by 0.001 m per burst (case 9).
- [ ] Satchels open a 30 m AT ditch in only 2 of 10 runs (7 each; the
  rest still closed after 16). Engineers sent at a tank from 40 m all die
  (case 8).
- [ ] The AI never throws satchels at vehicles (0 in 10 matches), rarely
  fires its AT teams (0 rockets in 6 of 10), and no tank bogged in 10
  matches. Escorts reach 30 m of an enemy trench 0–4 times per side.
- [ ] Map side: base 1 won 7 of 10 AI matches in this pass (S1: the
  south-west base about 75 %). Too few games to say more.

Other:
- [ ] At 1280x800 the roster now keeps vehicle cards before unselected
  infantry: with 4 tanks and 8 squads it shows the 4 vehicle cards and
  "+8" (before: 5 infantry cards and "+7"). Owner's call which is better.
- [ ] AT-ditch sections are recorded when dug (fix pass), in one shared
  list of 1,024 for both players; no per-player cap.
- [ ] `vehicle_view.gd` draws at most 48 vehicles per type (Battalion
  tier blocker; S3 rules need 40).
- [ ] Vehicle path ticks can serve the 4,000 budget plus one capped
  8,000-expansion search (11,694 expansions, intent stage up to 2.2 ms,
  4.4–5.7 ms under load). Within the spec; sets the worst tick at the
  Battalion tier.
- [ ] The vehicle spatial index is rebuilt twice per tick even with no
  vehicles (about 2 % of step samples); small per-tick allocations in
  gunnery and the block layer.
- [ ] Case 1's 3σ check is nearly empty (σ 0.0–0.6): the aim-point choice
  avoids borderline plates.
- [ ] A 7-minute `--autoplay` can miss the enemy's first tank (called in
  at about 5:40); run at least 10 minutes.
- [ ] The net loopback and soak use 4,000 starting manpower (not in the
  spec) so that 2 tanks per side arrive in 3 minutes.
- [ ] No in-game 1280x800 S3 screenshot: `--resolution 1280x800` keeps a
  1600x900 window for the staged scenes. The mock's `s3_mock_*_1280`
  shots cover the HUD.

