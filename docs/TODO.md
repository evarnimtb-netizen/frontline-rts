# Frontline RTS: open to-do

Open items only. The full S1-S5b to-do text (every owner question with its
options, closed items, old issues) is frozen in
`docs/archive/TODO-2026-11-03.md`; look up a question there by its id
(e.g. `grep -n "S4-43" docs/archive/TODO-2026-11-03.md`). New slices add
their questions here and move them to an archive file when the slice after
next starts. Process rules: `docs/process/workflows.md`.

## Owner decisions in force

- Genre game in the Gates of Hell style, our own design, not a recreation.
- Damage only from physical projectile or fragment contact; suppression only
  from real near-misses.
- Balance is cost-balanced realism: add historical options before changing
  stats; stats only for severe imbalance. All tuning values live in TOML.
- Trenches stay strong against small arms; attackers crack them with HE,
  grenades, tanks or flanking (S4-43).
- Call-in has no queue: buy if affordable, spawn at base at once, per-card
  cooldown, each card its own (Q21 B); values from the S4 roster table in
  each unit's TOML (rifle 20 s, engineers/SMG/LMG 30, AT 48, mortar 60).
- No free ammo anywhere, base included. Small arms cheap, explosives
  moderate, shells and tank rounds priced; distance costs only physical
  delivery, never a surcharge (D1, D2). Tanks rearm at base, HQ or truck;
  rare rounds also forward but slower and capped (D3, `rare_cap` 10).
- HQ refills only by carried crates (Q2 C). Historical squad sizes, pop 150.
- German cards keep the 20 % price cut (S5a-37).
- The base stops refilling mortar bombs in S6 (S5a-42).
- Mortars are tuned up in S6 through AI mission size, bomb load and map
  fire, never lethality: 10-15 % of kills, HE share 20-25 % (cap 26.3 %)
  (S5b-17).
- Tank crews bail out; abandoned tanks can be re-crewed or captured by crew.
- Artillery is on-map crewed units with authentic ranges, observers and
  counter-battery; no shell ration; field howitzers on all maps, cap 1.
- Vehicles before fortifications in the roadmap.
- Match length: no match under 10 or over 40 min; median 15-30.
- Vehicle spotting stays 2x; AT guns get a 250 % optic bonus inside their arc (Q12 (A), owner 2026-11-08).
- AT gun capture: kill the crew and the gun turns neutral; an enemy squad can serve it, only with the rounds left at the gun or from captured crates of the gun's faction (owner 2026-11-08).
- T33 (vehicle manpower cap) counts tanks and tank destroyers/assault guns only (owner, S8b-26).
- S10 on: an engine slice is split into a mechanics slice (cards at `ai_weight` 0, proven neutral) and an AI-and-balance slice (owner 2026-11-09).
- Features first, balance later (owner 2026-11-11): no per-slice Tune; each slice measures and records 480 matches;
  Germany's pooled share must stay in a 35-65 % guard band, outside it stop and diagnose. Fine balance to 45-55 % and the
  soft per-unit targets wait for a dedicated balance pass (after S10b-ai or S11). The next slice's targets file uses the
  35-65 % band and marks T31/T55-type targets as reports.
- Minor scope and price questions are settled on the recommended (A) and only reported; irreversible choices,
  how-it-should-play choices and main updates are still asked (owner 2026-11-11).
- S10a-ai decisions (owner 2026-11-11): breaching "fix smoke, lower bar" (T93f/h floor 30, T93g report); AI wire in 60 m
  lines; cards Germany by weight (Bau 100, Sturm 60, Pioniere 40), USA floor a report; smoke on three men.
- Workflows: tiers, review gate and model/effort rules in
  `docs/process/workflows.md` (owner 2026-11-03).

## Owner play tests and checks (need a person)

- [ ] Play the S5b demo `build/Frontline-a6e75c18047e-macos.zip`: are fire
  missions clear to use, do mortars feel too weak; check the mortar card at
  1280x800 (S5b-25).
- [ ] A more military UI look: theme pass "Map table" (`docs/slices/S1b-ui-theme.md`);
  open point: panel opacity (owner asked 70-80 %).
- [ ] Play a match vs AI with tanks, an AT team and an AT ditch (S3 item 11).
- [ ] Net: one match with the friend with vehicles, one over Tailscale or the
  VPS relay, the friend runs the zip on their own Mac (S2/S3 checklists).
- [ ] Decide the D / playout revision (`S2-multiplayer.md` "Turns"); work
  uses the revised formula.

## Open owner questions (work proceeds on each (A))

Full text in the archive file. None blocks a slice; answer any you care about.

- S3: 1-5 and the case 4 check.
- S3b-1..42, S3c (in its section), S4-1..47, S5a-1..46: open ones only
  (each section lists which were decided).
- S5b-1..16, S5b-18..25 (S5b-17 decided). The ones most worth a look:
  S5b-2 (M1 charge velocities are estimates), S5b-10 (mortar card prices),
  S5b-21/23 (German share about 4-5 points low out of sample),
  S5b-25 (card layout at 1280x800).
- S6-1..19 (`docs/slices/S6-hq-crates.md` "Owner questions"), each on its (A):
  S6-1 a shells crate within 5 m rearms a tank at the forward rate (B: base or HQ only, Appendix A).
  S6-2 cook-off bursts belong to whoever broke the box.
  S6-3 a dead carrier's crate drops and stays his player's.
  S6-4 HQ costs 100 MP and dig time; no material until S10.
  S6-5 bullets stop at boxes; only HE cooks off.
  S6-6 mortar teams dig their own pit (deferred, see S6-12).
  S6-7 the radio box is a state only until S11.
  S6-8 build cost is paid on order.
  S6-9 infantry retreat to the nearest HQ only when it is nearer base than they are; vehicle and crew squads to base.
  S6-10 a broken stock box loses all its stock.
  S6-11 the base issues one crate per `base_crate_issue_s` per player and refuses (Busy) in between (B: FIFO queue).
  S6-12 mortar teams do not dig; a mortar pit is dug by engineers or the Zugtrupp with `mortar_pit`.
  S6-13 cook-offs burst as the faction's 75 mm HE round (no separate cook-off defs).
  S6-14 a structure whose stock box breaks is destroyed (the HQ's `rebuild_s` starts).
  S6-15 a carrier's whole squad walks at the crate's carry pace.
  S6-16 the AI builds no dumps yet and its mortars do not target enemy HQs or crates.
  S6-17 an AI tank far from every source (`tank_crate_far_m` 300) holds for a bearers' run (B: path length).
  S6-18 structures are not selectable yet; the strip shows the HQ, else the first dump (B: selection in S6).
  S6-19 a carrier sets his crate down within 2 x `pickup_m` of the point, or where he stands once his squad holds there.
- S6b-1..3 (`docs/slices/S6b-mortar-ai.md`, `S6b-log.md`), each on its (A):
  S6b-1 Germany's win share (T5, band 45-55 %) reads 52.5 / 43.1 / 41.9 / 50.6 % on four 160 sets at S6 prices (pooled 47.0 % over 640, in band). (A) Keep S6 prices; a German price or AI lever is its own Tune slice if wanted.
  S6b-2 `mortar_reserve_pct` 0 means the AI keeps no bombs for defence. (A) Accept; a defence-only reserve needs its own 160 measurement (a 10 % reserve cut mortar share).
  S6b-3 AI map fire is off in ww2 (`map_fire_max_miss_m` 20, under the 27 m floor). (A) Keep it off until better map data (S11).
- S7-1..15 (`docs/slices/S7-casemate-td.md` section 12 and `S7-log.md`), each on its (A):
  S7-1 No StuG roof MG 34 or M10 .50 (pintle mounts out of scope). (A) None; add with pintle mounts in S8. S8a: settled for the half-tracks (251 MG 34, M3A1 M2HB on pintle or ring); StuG and M10 still out.
  S7-2 StuG and M10 buy through the existing `vehicle` mix slot, split by card `ai_weight` (100 each), not new `[ai.mix]` slots (those would bypass the vehicle unlock, limit and savings). (A) Yes.
  S7-3 StuG 500 MP / pop 10 / 120 s; M10 520 / 10 / 120 s (vehicles.md 8.4 said 520/500 and 90 s). (A) As stated; the Tune may move each within +-15 %.
  S7-4 The M10's assistant driver takes the `radio_mg` seat role (no hull MG), labelled by seat id. (A) Yes; a sixth role when a vehicle needs both.
  S7-5 A thrown item that strikes a plate drops to the ground below it (no deck charges, no grenade screens). (A) Yes; deck satchels are a later decision.
  S7-6 3-inch M62 penetration row derived from the US 30° table x 1.066 / cos 30° until a Bird & Livingston row is read. (A) Use it, marked verify. Sourcing pass: not settled; only the 1000 yd input (88 mm) is supported by Wikipedia, so the row keeps verify (sourcing-log.md).
  S7-7 Small arms do not aim at open-top crews (stray rounds through an open face still hit seats). (A) Yes; aimed fire with 6.4 (S8). S8a: settled (reach table; a crewman's head over the rim is a target, S8a-17/18).
  S7-8 Fragments never pierce plates in S7 (all plates here >= 10 mm). (A) Yes; a thin-plate rule with 6.4. S8a: settled (`frag_pen_mm` 12 / 9 / 3, S8a-13).
  S7-9 Armour zone id = box index x 4 + face group, name `<box>_<face>` (`casemate_front`); the two tanks keep ids 0-7 and names. (A) Yes.
  S7-10 StuG gun traverse 6 deg/s and casemate/M10 elevation 4 deg/s have no source. (A) Starting values, marked verify. Sourcing pass: not settled; no source for traverse or elevation rates (sourcing-log.md).
  S7-11 T34e (StuG fires in >= 80 % of armour-test case 18) cannot hold as the case is set: the Sherman starts laid on the StuG at 200 m, hits at 1.3 s (median) and 9 of 10 first hits penetrate the 30 mm side and disable it; the StuG's pivot and lay take 2.5 s (its first hit in the one run it survived). (A) Keep the scenario as vehicles.md 10 states it, make T34e a report; the pivot-to-lay is proven by `crates/sim/tests/casemate.rs` (fires in under 8 s on a target that does not shoot back).
  S7-12 T36a (an 81 mm ground burst 3-6 m from an M10 hits its crew in >= 20 %) does not hold physically: the crew boxes rise only 14-24 cm over the 66 cm rim and ground-burst fragments climb at 2-50°, so the hull edge and turret wall shadow almost every line; armour-test case 20 measures 2 of 60 (3.3 %), a Sherman 0. Bursts on the deck behind the turret hit the crew in about half (sim test). (A) Keep the physics and the Build A seat boxes; T36a becomes a report until the owner wants standing crew (head and shoulders ~40 cm over the rim) or airbursts.
  S7-13 Thrown items now collide with enemy vehicles (S7-5: a plate drops them to the ground below), so S6b matches with tanks differ slightly even before the new cards. (A) Accept; the Build B log measures it with the rest.
  S7-14 With the second vehicle card a faction has 11 call-in tiles; two columns rose above 57.5 % of the screen, so the call-in panel takes up to three columns. (A) Keep three columns; a compact tile row is presenter polish for later.
  S7-15 With overwatch_back_m 35 (S7 Fix, review M2) the Pz IV takes 18.8 % of German kills on the in-sample 160 (seeds 1-80), 0.8 over T32a's 18 % (17.4 and 16.9 % on seeds 101-180 and 201-280; 16.1 % before the fix). 50 m keeps it at 17.4 % but halves the StuG/M10 KOs and gives Germany 45.0 % pooled. (A) Keep 35 m; the Tune of the next vehicle slice rechecks T32 with td_hunt_m (450 m untested). S8a: T32 holds (Germany top type 17.9 %, cap 18); the out-of-sample S7-15 report reads 20.0 %, still open. S8b: T32 holds on the final builds (Pz IV 17.1 % on seeds 1-80, 16.8 % in Tune); the `td_hunt_m` check is still untested.
- S8a-1..31 (`docs/slices/S8a-halftracks.md` section 14; 16-19 Build A, 20-27 Build B, 28-31 Fix), each on its (A):
  S8a-1 Main gun optional (half-tracks have none). (A) Yes.
  S8a-2 M3A1 capacity 11; squads never split, a squad that does not fit is refused (US 12-man squad rides after a loss). ANSWERED 2026-11-08 by the owner: (B), capacity 12 (twelfth slot behind the cab, S8b Tune).
  S8a-3 MG pen tables use the belt's AP round for every round (no belt-mix roll). (A) Yes; `ap_share_pct` later if MGs run high.
  S8a-4 Face-hardened plate x 125 % against every round (`[vehicles] face_hardened_pct`). (A) Yes, verify. Sourcing pass: not settled; reports range 1.1 to 1.5, 125 % lies inside, forum sources only (sourcing-log.md).
  S8a-5 Transports skip the vehicle limit of 4, unlock at 2:00, cap 2 per card. (A) Yes.
  S8a-6 Half-tracks steer like tanks (neutral pivot) until S8b's wheeled model. (A) Yes.
  S8a-7 Hotkeys Embark V, Disembark U, Shift+U over the sides. (A) Yes.
  S8a-8 `Disembark { vehicles, over_sides }`; door is the default. (A) Yes.
  S8a-9 Riders are targets only while their slot box is above the rim and the vehicle is seen. (A) Yes.
  S8a-10 251 commander fires the front MG; no 251 rear AA MG, no M3A1 .30s; StuG roof MG and M10 .50 (S7-1) stay out. (A) Yes.
  S8a-11 Cards 290 / 310 MP, pop 3, 60 s, cap 2, unlock 2:00. (A) Yes; Tune +-15 %.
  S8a-12 The sim emits every small-arms penetration event; the feed coalesces them per vehicle per 2 s. (A) Yes.
  S8a-13 Thin-plate fragment values `frag_pen_mm`: 75 mm 12, mortars 9, grenades 3. (A) Starting values, verify. Sourcing pass: not settled; no terminal-ballistics source reached (sourcing-log.md).
  S8a-14 Cross-country speeds 750 / 850 cm/s (road 52 / 72 km/h). (A) Starting values, verify. Sourcing pass: road speeds confirmed (251 about 52.5, M3A1 72 km/h per TM 9-2800); cross-country values stay gameplay assumptions (sourcing-log.md).
  S8a-15 Half-track crews bail as the faction's tank crew squad. (A) Yes.
  S8a-16 (Build A) A half-track whose MG is dry or unmanned counts as "gun out" for the bail triggers, so a second penetration within `under_fire_s` after the commander falls abandons it. (A) Yes (the driver alone does not fight on). (B) Transports never take the "under fire" trigger.
  S8a-17 (Build A) A soldier picks a vehicle only when it is nearer than the nearest man he can see (soldiers win a tie); vehicle MGs keep engaging men only. (A) Yes. (B) Vehicle MGs also fire at half-tracks they can hurt.
  S8a-18 (Build A) Rifles carry a ball table that beats no plate in the game; it only lets riflemen aim at exposed crews (S7-7). (A) Yes.
  S8a-19 (Build A) `board_ms`/`exit_ms`/`exit_over_sides_ms` in milliseconds and `[vehicles.transport]` (and `face_hardened_pct`, `small_arms_ricochet_deg` in `[vehicles.gunnery]`) as tuning tables, not the spec's `_s` keys in `[vehicles]`. (A) Yes (integers, `--set` works).
  S8a-20 (Build B) The crew rule's floor is `[vehicles.bail] crew_below` capped at seats - 1 (and re-crewing needs at most the seat count), so a two-man half-track is not abandoned at once (with 3 it was abandoned on spawn). Tanks (4-5 seats) unchanged. (A) Yes. (B) A per-vehicle `crew_below` key.
  S8a-21 (Build B) Half-track cards `ai_weight` 100 (the tanks'): bought in 92-95 % of matches, about one ride each. 150 bought them before the first tank but cut tank HE kills (HE share 17.6 % on the 40 set). (A) 100; the Tune weighs rides against HE.
  S8a-22 (Build B) A round, fragment or burst that strikes a passenger over the rim does not count as a penetration of the vehicle (`last_pen`, the bail "under fire" trigger); the AI's "hit" rule also counts riders suppressed past `rider_suppression_pct` (50). (A) Yes.
  S8a-23 (Build B) Rounds stopped by a transport (and small-arms hits on it) suppress its riders as a covered near miss; there is no other near-miss suppression aboard (riders are not in the soldier spatial hash). (A) Yes.
  S8a-24 (Build B) Men walking to a half-track that are beside or in front of it walk to the hull corner on their side first (local steering cannot go around the hull); boarding is only at the door (`board_reach_cm` + 1 m). (A) Yes; boarding over the sides stays out (spec 5).
  S8a-25 (Build B) Ride counters ("time saved") compare the ride (first man in to last man out) with walking the straight line at the squad's speed: a lower bound of the saving. (A) Yes.
  S8a-26 (Build B) The AI's Support phase is the S7 overwatch habit (hull-down or stand-off spot); it returns to the base only when a squad waits there (or an infantry call-in is pending) and meets squads on the road (within 40 m). The Load wait counts the picked squad's walk at 1 m/s and ends after 3 x `wait_s` at most. (A) Yes; the Tune measures rides.
  S8a-27 (Build B) Passengers are drawn crouched at the bottom of their slot (no seated pose yet); the vehicle card lists them in place of the main-gun ammunition row; the components row still shows the gun, ring and rack a half-track lacks. (A) Accept for S8a; presenter polish later. S8b: the card and bar part is fixed (cards list only the parts a vehicle has); the crouched pose stays open for presenter polish.
  S8a-28 (Fix) Ride value, measured as the review asked (Embark order to the squad's lead inside the capture radius of the point it rode for, against a straight walk from where it stood at the order; `AiRideMeas`): AI rides are slower than walking, Germany -29 / -26 / -21 s and USA -14 / -15 / -6 s per ride on seeds 1-80 / 101-180 / 201-280 (about half the measured rides never reach their point: retasked or killed). There is no walk-to-the-front exploit; S8a-25's figure was an upper bound. (A) Accept for S8a; make the ride pay in S8b (board where the squad stands, a shorter `dismount_sight_m`, shorter boarding waits) with a Tune. S8b: largely settled (USA rides pay on all three sets; Germany on two, 201-280 at -2.8 s, S8b-27).
  S8a-29 (Fix) After the M1 picking fix (the point at least `min_ride_m` + the walked last stretch from the squad and `min_ride_m` beyond the door) USA transported per match is 0.98 / 0.89 / 1.04 and trips per bought 0.86 / 0.78 / 0.87 (T41b, T48b floors 1). (A) Accept the misses for S8a (the old numbers counted short hops); retune with S8a-28. S8b: settled; T41b and T48b pass on the final builds.
  S8a-30 (Fix) Germany wins 42.5 % on seeds 1-80 (T5c floor 45), 45.6 % on 101-180, 48.1 % on 201-280: pooled 45.4 % over 480, inside the 45-55 % band. (A) Accept T5c; the pooled band holds.
  S8a-31 (Fix) The AI keeps unloading over the sides under fire: armour case 24 over 80 runs hits 3.02 men by the door and 3.36 over the sides (a gap of about one standard error), while the sides are 2.3 s faster. (A) Keep; recheck with the S8b ride work. S8b: not rechecked; still open.
- S8b-1..31 (`docs/slices/S8b-trucks-recon.md` section 16, log `docs/slices/S8b-log.md`): 1-20 Plan, 21-27 Build C, 28-29 Fix, 30-31 Build B (numbered 21-22 there, a clash with Build C), each on its (A):
  S8b-1 (Plan) Recon cars and trucks are outside the rules' vehicle limit, each capped by its card (2); the half-tracks' rule. (A) Yes (recon does not take a tank's place). (B) Recon counts against the limit.
  S8b-2 (Plan) `DropCrates` carries the kind: `DropCrates { vehicle, point, count, kind }`; the design's form had one crate item, S6 made three crate defs per faction, and the AI needs shells for mortars and small arms for trenches. (A) Yes. (B) Always small arms.
  S8b-3 (Plan) A truck auto-loads its def's `load_kg` while stopped in base, paying per kg as it loads (shells on the player's crate queue); no load menu. A full default load costs 364 MP. (A) Yes.
  S8b-4 (Plan) Mortar bombs from a truck only as dropped crates (D1 `crate_only_if_indirect`). (A) Yes.
  S8b-5 (Plan) Trucks carry no men: `[transport]` is gated on role in the validator and in the AI, so it is not pure data. (A) Supply only in S8b.
  S8b-6 (Plan) Forward MG belts at HQ, dump and truck (vehicles.md 7.3, not built in S6): this changes tanks at HQs slightly. (A) Yes.
  S8b-7 (Plan) Both trucks unarmed; the M36 ring MG on about 1 CCKW in 4 is left out. (A) Yes.
  S8b-8 (Plan) No road speed; both cars and trucks use cross-country speeds (no roads on our maps). (A) Yes.
  S8b-9 (Plan) Cargo detonates only with at least 40 kg of shells or explosives; below that it burns. (A) Yes.
  S8b-10 (Plan) Tanks keep `stationary_sight_pct` 100 (no optics change to the tank balance). (A) Yes, values with a later tank pass.
  S8b-11 (Plan) Figures not in vehicles.md (2 cm AP and HE, stowages, reloads, truck plates) are from memory and marked verify in the defs. (A) Use them; an owner or M2 task checks sources. Sourcing pass: partly settled: CCKW payload now 2270 kg, several plates and stowage totals confirmed; the rest stays verify (sourcing-log.md).
  S8b-12 (Plan) The 2 cm gun is a one-shot model with an averaged reload (no 10-round magazine, no bursts). (A) Yes.
  S8b-13 (Plan) The push-limit reserve (`push_without_supply_m`, 7.6) is left out. (A) Yes, with the heavy wave.
  S8b-14 (Plan) AI tanks fire only 6.4-6.9 main-gun rounds per tank bought and start 0-4 rearm trips in 160 matches (S8a data), so truck rearms of tanks stay few; crates are the measured work and tank-round targets are reports. (A) Keep the truck as the owner asked. (B) Raise AI tank fire first, out of scope.
  S8b-15 (Plan) A half-track picks squads within 250 m and drives to them (was 120 m, waiting at base). (A) Yes.
  S8b-16 (Plan, Build A) A wheeled vehicle drives and paths on the undilated wheel layer (its dilation is the margin), so a truck parked by a new trench is not stranded and a 10 m trench gap stays open. (A) Yes.
  S8b-17 (Plan) AI own budgets: recon at most 8 %, trucks at most 5 % of income so far, outside the unit mix. (A) Start values; the Tune moves them.
  S8b-18 (Plan) The M8's .50 ring is a 360 degree hull-frame pintle, not a turret-mounted gun. (A) Yes.
  S8b-19 (Plan) Drop Crates hotkey `I`. (A) Yes.
  S8b-20 (Plan) Trucks load only at base, not from an HQ (vehicles.md 7.3 allows both). (A) Yes.
  S8b-21 (Build C) `min_ride_m` 120 -> 200 picks longer rides (short ones cost more than they save); "transported" (T41) keeps the S8a 120 m through `counted_ride_m`. (A) Yes.
  S8b-22 (Build C) Own-budget shares above the S8b-17 starts (recon 12 %, trucks 9 % of income): at 8 % and 5 % the first car comes at about 12 minutes. (A) Yes. Owner 2026-11-09: (A), keep recon 12 %, trucks 9 %, and the prices 234/1 260, M8 300, Blitz 150, CCKW 200.
  S8b-23 (Build C) "MG dry" on the 251 card was the `halftracks` screenshot scene, which empties the MGs to hold fire; the bridge test shows a fresh 251 reports its stowage. The "red hatched ammo bar" was the absent parts in the roster's component strip (S8a-27). (A) Closed.
  S8b-24 (Build C) Recon AI reuses the S7 overwatch entry (`AiOverwatch`), not a new saved `AiRecon` job; the float for rearming stays the existing `[ai.ammo] float_mp`; a new `ammo_float_mp` key adds nothing. (A) Yes.
  S8b-25 (Build C) Truck blockouts: 3 axles for every truck, 4 on a long armoured car (no axle field in the defs). (A) Accept until art.
  S8b-26 (ANSWERED 2026-11-08 by the owner; closed in S9): T33 counts tanks and tank destroyers/assault guns only, not recon cars, trucks, half-tracks or guns. Field `tank_td_mp_pct` in `S9.toml`; no game change. S9 Build C reads 17.9 / 16.0 %, pass.
  S8b-28 (Fix) `wheeled_stuck_repaths` 12: a wheeled move re-pathed that often within 6 m ends idle where it stands, so a goal behind a closed trench line ends like an arrival (a DropCrates sets its crates there). Tracked vehicles keep re-pathing as before. (A) Accept. (B) Same rule for tracks in a later slice.
  S8b-29 (Fix) A recon car that backed off holds the place it backed to until its anchor moves or it is fired on, even if the threat is gone from sight; a car facing away drives off instead of reversing. (A) Accept. (B) Return to the spot when the threat has been unseen for some seconds.
  S8b-27 (Build C) T50e (Germany on 201-280) -8.75 s and T41b (USA on 1-80) 0.981: five of six ride pairs are above 0, rides per set are few (about 50), so each pair moves by about +-10 s by chance. (A) Accept; not chased further.
  S8b-30 (Build B, was S8b-21) A destroyed cargo box always starts an ammo fire and the fire detonates the load at the same 40 kg rule (so one penetrating hit detonates a loaded truck about 70 % of the time), matching 7.3 "a penetration or fire". (A) Yes. (B) A `cargo_detonate_pct` of its own.
  S8b-31 (Build B, was S8b-22) A truck need not stop to serve (the radii are short, and `DropCrates` and loading already need it stopped). (A) Yes. (B) Serve only when stopped.
  S9-1 (ANSWERED 2026-11-08 by the owner: (A)) Gun optic range. The owner's 250 % applied as written multiplies the target's size factor (a tank 200 %, moving 250 %) and gives 875-1090 m on the 1 km map; the design says "about 400-500 m". (A) Inside the arc against vehicles the range is `sight_m` 175 x 250 % = 437 m, no size or moving boost. (B) Literal multiply (875 m and more).
  S9-2 (ANSWERED 2026-11-08 by the owner: (A)) A card with a tow pays gun + tow, the tow arrives hitched with the crew riding; the stand-alone Sd.Kfz. 11 card is for spares and its `max_alive` does not limit the gun card. (A) Yes. (B) Guns and tows only as separate cards.
  S9-3 (ANSWERED 2026-11-08 by the owner: (A)) The 57 mm comes with an M3A1 (340 + 310 = 650 MP against the Pak 40's 560): the CCKW is cheaper but carries no men in this slice (its crew would walk). (A) M3A1. (B) CCKW at 540 MP with a walking crew.
  S9-4 (Plan) The S5a-37 German price cut is not applied to gun cards (roster prices 380 and 260). (A) Yes; the Tune may move them. (B) Apply the cut.
  S9-5 (Plan) A gun's own `pop` is 1 (the validator range starts at 1); the crew are counted as soldiers. (A) Yes.
  S9-6 (Plan) AI displacement (`displace_after_shots`, HE within 20 m) is left out; a Hold gun only re-sites when overrun. (A) Yes. (B) Build it now.
  S9-7 (Plan) Any infantry except AT teams can serve a gun (untrained: reload and spread 150 %); crews are tag `gun_crew`, not `crew`. (A) Yes, as designed.
  S9-8 (Plan) Captured crates: a squad can pick up a set-down enemy crate; it becomes that player's and feeds only weapons of the crate's own faction (the captured gun). (A) Yes. (B) Only crates lying within reach of the gun, no pick-up.
  S9-9 (Plan) Gun pit 55 cm deep so wheels are not blocked and no ramp is needed (design: 60-80 cm with a ramp). (A) Yes.
  S9-10 (Plan) A neutral gun is seen by everyone (the S3c abandoned rule). (A) Yes. (B) Fog applies.
  S9-11 (Plan) Destroying the `Gun` part wrecks the gun; `Recoil` is field-repairable; barrel is part of `Gun`; `Sight` and `Recoil` are new parts. (A) Yes.
  S9-12 (Plan) Men at the slots hold their rifles while the gun has a target. (A) Yes. (B) They always fire their personal weapons.
  S9-13 (ANSWERED 2026-11-08 by the owner: (A)) Gun cards `max_alive` 2 and available from 0:00 (the roster gives no cap or unlock; the AI waits until 2:00 and buys only once a tank has been seen or at its date). (A) Yes. (B) Tighter cap or a later unlock.
  S9-14 (Plan) The 57 mm has AP only in S9 (M303 HE is late 1944, verify). (A) Yes. (B) Give it HE from 1944-06 for balance.
  S9-15 (Plan) The AI buys only the Pak 40 and the 57 mm (Pak 38 stays `ai_weight` 0); very old neutral guns are scrapped by the abandoned cap. (A) Yes.
  S9-16 (Build B) Guns run on the wheel block layer of the trucks and cars (shallowest wheeled ditch 10 dm), not their own 6 dm: only standard trenches and deeper block them; a 6-10 dm scrape does not. (A) Accept until a third dig depth exists. (B) Give guns a third layer.
  S9-17 (Build C) AI guns are placed by sight lines and cover, but they kill few tanks in matches: see the S9 log (0.02-0.08 tank knock-outs per match per side; 6-7 rounds per match; 2-5 % of the kills for 17-19 % of the manpower). (A) Accept for this slice: the AI's use is cautious and costs both sides alike; revisit with re-siting and a tank-corridor search (design 9) in the next AI pass. (B) Put AT guns in S9b with a corridor search now. FIX stage: re-siting built (`resite_idle_s`, 615 moves over 480 matches); pool 45.6 %, fires in 69-78 %, but 52 tank kills from 1955 guns: the corridor search stays the open part. (A) Accept for this slice. (B) Corridor search in S9b. Owner 2026-11-09: (B), a short S9b AI gun pass is next: corridor search, then a budget tune. S9b (2026-11-10): corridor siting built (`a9fa303`, fix `8aad271`); guns lost to armour 22 -> 26 % (GER), 14 -> 19 % (USA); sight line 19 -> 34 % (GER), 24 -> 30 % (USA). Open part: the sight gap (in reach 18-22 % of ready time, in sight 1-2 %). See `docs/slices/S9b-log.md`.
  S9-18 (Build C) `ambush_infantry_m` 200: a gun holding fire for an ambush breaks it for men only inside 200 m (done-when counts firing at all). (A) Keep. (B) Hold against men until the gun is struck.
  S9-19 (Fix) Pool 45.6 % (480 matches) with 14 hard target misses left on the 160 sets, mostly at the edge: HE share 18.6 (floor 20), T40/T72 half-tracks bought (41-56 %, floor 60), T54 tank floors (M4 MP share 9.94 of 10, StuG 73 of 80, M10 69 of 75), T31 vehicle kills 30.3-30.4 (cap 30), T55b M8 10.3 (cap 10). (A) Accept for this slice: the guns take their budget share as a cost. (B) A budget or mix tune before merging. Owner 2026-11-09: merged to main with the misses open; S9b re-measures them after the corridor search and budget tune. S9b (2026-11-10, `1bfc92f`): 10 hard misses remain at income_max_pct 30 (pooled GER 48.75 %); T88 met for Germany, not USA; causes in `S9b-log.md`.
  S9b-1 (Budget) US guns put 0.36 armour out per match at income_max_pct 30 against the T88 floor of 0.45 (Germany 0.58 passes); at 35 neither side passes (0.54 / 0.40) and hard misses are 17 against 10. (A) Keep 30 and accept the US shortfall, carrying the sight gap to the AI slice of S10. (B) Go back to 35. (C) One more AI pass on US gun siting before S10a. Owner 2026-11-10: no preference; (A) stands.
  S9b-2 (Fix F1) `SAVE_VERSION` 20 -> 21 for the saved `[ai.guns]` layout (spec section 4 read as applying; rule 5 was the open point). Decided in practice: the bump stays, because the setup is saved (version 16 precedent), unless the owner objects. (A) Accept the bump. (B) Waive it and document the layout change under SAVE 20. Owner 2026-11-10: (A).
  S9b-3 (Fix) 10 hard misses remain (half-tracks T40/T72, StuG and M10 floors, HE share, USA vehicle kills; the USA armour floor is in S9b-1). They need another lever (`max_guns` 1, card priorities or the sight gap). (A) Accept for S9b and carry to S10a. (B) One more AI or card pass (S9c) before S10a. Owner 2026-11-10: no preference; (A) stands.
- S10a-1..11 (`docs/slices/S10a-fortifications.md` section 12; 9-11 from the Build B-C review), each on its (A). All are built on (A); the owner confirms:
  S10a-1 Sandbag walls are built only by `build_structure` squads (engineers). (B) Any digging squad, from carried bags.
  S10a-2 Pieces stop rounds and fragments only; sight and walking pass them (wire is the one movement obstacle; camo net drawn only). (B) They block spotting too. (C) They block movement too.
  S10a-3 Own rounds pass own pieces (the S6 box rule), so an MG fires out of its nest ring. (B) Every piece stops every round; nests need a slit.
  S10a-4 The HQ dugout is an optional roof over a built HQ (120 kg, 120 s); the S6 HQ is unchanged. (B) Every HQ needs 120 kg and is roofed (AI half).
  S10a-5 Materials cost 100 MP per t at base, 20 kg per man, no walking penalty. (B) Free. (C) Carried kg slow the squad.
  S10a-6 Interrupted work is removed and its materials lost (S6-8). (B) The site stays and any squad can finish it.
  S10a-7 Vehicles crush wire they cross and are not slowed. (B) Wire stops wheeled vehicles.
  S10a-8 Card prices: Sturmpioniere 264, Baupioniere 165 (roster with the German cut and S6 factor), Assault Section 380. (B) Uncut 320 / 200 / 380.
  S10a-9 A roof or any enemy piece top stops a thrown grenade or satchel, which bursts on it (a satchel breaks a dugout roof, a grenade does not; own pieces do not stop own throws). (A) As built; owner to confirm.
  S10a-10 A roof that breaks under a burst does not stop that burst's fragments. (A) As built; owner to confirm.
  S10a-11 The build menu key is ` (a stand-in; all letters are taken). (A) As built; owner to confirm.
- S10a-12..19 (`docs/slices/S10a-ai.md` section 12, Plan 2026-11-11): settled on (A), except S10a-12, -13, -14 and -16,
  which the owner's 2026-11-11 decisions (Owner decisions in force) replaced:
  S10a-12 Engineer card buys: (A) engineer mix slot by weight (Pioniere 100, Baupioniere 50, Sturmpioniere and Assault Section 35) plus an urgent breach buy. (B) Assault cards only by the urgent rule. (C) Own `fort` mix slot.
  S10a-13 Smoke for the Sturmpioniere: (A) the leader carries one `ww2:nbhgr39`. (B) No smoke; the German AI only suppresses.
  S10a-14 Lanes in AI wire: (A) one unbroken line, own men go round. (B) A 6 m lane in the middle, usable by the enemy too.
  S10a-15 "Under mortar fire": (A) proxy: half the point's defenders ducked or pinned with no visible enemy within 150 m. (B) A sim-side record of HE bursts (more saved state).
  S10a-16 T93: (A) hard on the AI-driven breach row (gap and placed >= 60 %, attackers <= 60 %); the S3b bar stays a report until S11. (B) All of T93 stays a report.
  S10a-17 S9b-1 US gun sight gap: (A) defer to the next AI gun pass (not cheap here). (B) Fold a sight-first re-site into S10a-ai.
  S10a-18 Materials budget: (A) at most 3 % of income, builders load full at base. (B) No cap.
  S10a-19 HQ roof: (A) roofed once enemy mortars are seen. (B) Always, as soon as the HQ stands.
- S10a-20 (settled 2026-11-11, owner chose (A); done in `S10a-ai.toml`): make T93f/h and T96 reports until S11 combined arms. (A) yes, recommended. (B) Keep them
  hard and add a lever now (a looser give-up rule, a closer start, a bigger cloud).

- S10b-1..4 (`docs/slices/S10b-obstacles.md`; Plan 2026-11-11). The slice runs on (A); the owner confirms:
  S10b-1 Own mines. (A) Own and allied units never set off own mines, and a mine's burst never hurts its owner's side
  (the sim's HE already spares own men and vehicles). (B) Design 7.6: mines hit whoever touches them and own paths avoid
  own mines (needs friendly HE damage and per-player path costs; changes every HE weapon's rule).
  S10b-2 Paths and found mines. (A) No path avoidance in S10b: players steer round marked mines; avoidance comes with
  S10b-ai. (B) Infantry and vehicle paths avoid own and found mines now (per-player path costs; more path work).
  S10b-3 AT mine damage. (A) Running gear only (a track or wheel axle out: immobilised), no belly penetration: the
  vehicle boxes have no belly plate. (B) Belly plates on every vehicle and a belly blast (armour data; a later slice).
  S10b-4 Bunker slit. (A) Own rounds pass own pieces (S10a-3), so the slit matters only to incoming fire. (B)
  Defenders must fire through the slit (own bunker pieces stop own rounds).

- S10b-5..9 (questions from the S10b verify, 2026-11-12). Each (A) is recommended; the owner picks.
  S10b-5 Hedgehog gap (T106b). (A) Keep: a line needs several charges or a tank-gun/HE effort, and T106b becomes a report. (B) Smaller block pad so one destroyed piece opens a tank-wide gap.
  S10b-6 Demolition charge at a bunker. Now a charge at ANY wall kills all five inside (slit 100 %, uphill slit 100 %, back wall 100 %). (A) Only the slit (and the door side) is deadly; a wall charge damages the piece. (B) Keep every wall deadly.
  S10b-7 Sweeping cost. A sweep of a mixed field loses about 1.2 men per sweep (T104a) and lifts 77 % of mines. (A) Keep. (B) Make sweeping safer or slower.
  S10b-8 Obstacle that waits for a vehicle. (A) Keep the wait without limit. (B) End the work with BadSpot after a time limit.
  S10b-9 AP mine against vehicles. (A) Keep 10 % (the hand-grenade value, never breaks a track). (B) 0, no effect on vehicles at all.
- S10b-ai-1..7 (`docs/slices/S10b-ai.md` 10; Plan 2026-11-12). The slice runs on (A); the owner confirms:
  S10b-ai-1 Mine paths. (A) Paths avoid only enemy mines the player knows (own mines are harmless under S10b-1 (A)).
  (B) Also own mines (only meaningful with S10b-1 (B)).
  S10b-ai-2 Formation men. (A) Only the lead's path is priced; men 2-4 m off it may still step on a known mine (measured
  as `lost_on_known`). (B) Per-man paths near known mines (path cost, tick time).
  S10b-ai-3 Obstacle places. (A) In front of own AT guns (log barrier) and own nests or bunkers (hedgehog row), never on
  the own vehicle route. (B) Across enemy vehicle approaches beside AT fields (channelling; may block own counter-attacks).
  S10b-ai-4 Obstacle knowledge. (A) The AI knows enemy obstacles within 150 m of an own unit, no sight rays (vehicle paths
  already use the full block layer). (B) Sight rays as for wire.
  S10b-ai-5 Bunker crew. (A) The point's MG team mans the bunker (the nest takes a second MG team). (B) A rifle squad.
  S10b-ai-6 Demolition for vehicles. (A) Only when an AI vehicle has no way round, at most 3 pieces per job, then give up
  and remember. (B) Never for vehicles (they only route round).
  S10b-ai-7 F8 truck fix. (A) On by default (`[ai.truck] refusal_memory` 1, measured in the 480). (B) Off until the balance pass.
- S10b-ai build questions (log `docs/slices/S10b-ai-log.md`). Decided by the owner 2026-11-14, built in Build E
  (`docs/slices/S10b-ai-build-E.md`):
  S10b-ai-B2a AT field depth: DECIDED (B) two staggered rows. `[ai.obstacles] at_rows` 2: the second row is its own
  job once row 1 is laid, one 4 m step toward the point and half a step along, only if the field limit and the S10b
  budget pay for the whole row (else one row). On the flat map's 4 m grid lanes row 1 already stops every tank.
  S10b-ai-C1 AI barrier against the job deadline: DECIDED (C) for the barrier only: `barrier_job_max_s` 1200, every
  other job keeps `job_max_s` 900.
  S10b-ai-D1 Bunker rare: DECIDED change it, prices kept: `bunker_assign` (a free build_heavy/build_structure squad is
  sent to the point) and `bunker_truck_first` (one truck run per bunker job may leave tank service). Default 40 set:
  Germany bunker in 12 % of matches, USA 0 % (report target 15 %).
  S10b-ai-D2 Sweep lane losses: DECIDED single file: `lane_file` 1, attackers walk a swept lane on its centre line.
  S10b-ai-D3 Tank gap: DECIDED (B) narrowed now: the obstacle piece's vehicle block pad lets a medium tank through a
  4 m hedgehog row once 2 adjacent pieces are gone (12 m between standing centres); path layer and movement agree.
- S10b-ai owner questions (asked 2026-11-15; ALL DECIDED, E4-E8 by the owner 2026-11-16 on the recommendation; (A) is listed first):
  S10b-ai-E1 Bunker and the single job slot: DECIDED 2026-11-15 (B) on the orchestrator's default (owner B1 Q2),
  built in B1-0: switch `[ai.obstacles] bunker_yields` (default 1), see `docs/slices/B1-log.md`. The bunker takes the one S10b job
  slot for long, and Germany's mines in matches fell from about 45 % to about 25 % once the bunker changes went in.
  (A) Keep bunker first. (B) A bunker waiting for a truck gives up the slot. (C) `bunker_truck_first` off.
  S10b-ai-E3 Satchels: DECIDED 2026-11-15 (A) a second engineer squad takes over the job when the first has no
  satchel; no card change. Switch `[ai.obstacles] demolish_handover` (default 1), built in B1-0 (`B1-log.md`).
  S10b-ai-E4 Player key to choose 1-3 mine rows while placing a field (not the wheel; may need a protocol bump).
  Not built. DECIDED 2026-11-16 (A) leave for a later slice (with other work that needs a protocol bump). (B) Build in the next slice.
  S10b-ai-E5 Demolished wire leaves debris that slows walking. Not built. DECIDED 2026-11-16 (A) leave for a later
  slice. (B) Build in the next slice.
  S10b-ai-E6 AI use is far under the targets (mines, obstacles, bunker, AT fields). DECIDED 2026-11-16 (B) a
  follow-up slice that raises use; not scheduled or built yet (B1 built E1 and E3 only). (A) Leave for the balance pass;
  treat T109, T110a-d, T112, T113 as reports until then. (B) A follow-up slice that raises use (more job slots,
  builder assignment for mines).
  S10b-ai-E7 A new blast in a swept lane costs a whole new sweep and the attackers wait. DECIDED 2026-11-16 (A) keep.
  (B) Re-sweep only the part of the lane.
  S10b-ai-E8 T113 bunker target: 20 % was written for the old denominator. DECIDED 2026-11-16 (A) reset to about
  10 % of all matches; NOT YET APPLIED: T113a still has `min = 20` in `targets/S10b-ai.toml` and `targets/B1.toml`,
  change it in the next slice's targets. (B) Keep 20 %.
- S10b-ai not numbered, open: `s10b-ai` was merged into main 2026-11-15 with the owner's OK. The process proposals of the
  2026-11-12 audit were adopted by the orchestrator for this slice; the owner has not yet confirmed them. The parallel
  `tools/verify.sh` took 248 min 52 s (the old serial run took 38 min); it needs a look before the next slice.
- S10b open notes (not numbered): F8 from S10a-ai (an S8b truck re-orders an
  unreachable HQ goal about every 5 min) is planned in S10b-ai Build D (L5, switch `refusal_memory`; droppable). Bunker notes from Build E: MG fire from 150 m does not hit men
  in the pit, with or without the bunker; the bunker pit has no spoil bank because it filled the slit.

## B1 balance pass open questions (asked 2026-11-16; branch `balance-pass`, log `docs/slices/B1-log.md`)

- B1-1 DECIDED by the owner 2026-11-16: (A) accept for now, balance again when the roster is stable. Was: Germany at about 42 % (42.29 % of 480 on seeds 1-280, band 45-55 missed). No fort setting passes both seed groups
  with the floors. (A) Accept for now and balance again when the roster is stable. (B) Let the AI fortify less than the
  minimum-use floors. (C) Another diagnosis outside fortification (why fortifying helps the USA more).
- B1-2 Latent S10b-ai fault T115c: own obstacle rows at base 0 block own vehicles, and `count_own_blocked` recounts a
  (vehicle, piece) pair each time it reappears (seed 104 normal: 13). Needs its own fix.
- B1-3 USA wire floor is already missed at defaults (56-58 % of matches against 60).
- B1-4 Delayed forts are mostly never built (cause not measured).

## S10c-ai open owner questions (asked 2026-11-16; the slice ran on the listed default)

Spec `docs/slices/S10c-ai.md` 10 (the first option is (A)); log `docs/slices/S10c-ai-log.md`; measures from
`target/s10c-verify/measure.txt` (480 matches, commit `cc4af7c`). Two numbers per set are Germany / USA.

- S10c-1 Builders near the enemy. Ran on (A): a front builder works on until a known enemy is within 50 m of the site
  (80 m on the walk). (B) 80 m throughout: safer, far fewer front fields (not run). Measured: front builders lost a
  match 0.55 / 0.84, 0.61 / 0.76, 0.69 / 0.66 (T125 report, bound 0.5): both sides over the bound in all three sets.
- S10c-2 Builder squads. Ran on (B), not (A): `builder_cards_min` 2 is the default, waiting for the owner's answer.
  Measured on the 40 set (floor 0 `C1a40`, floor 2 `C1b40`): mines in 70 / 50 % of matches at floor 0 and 95 / 82.5 %
  at floor 2; floor 0 missed the home works floor, floor 2 kept it. Builder buys a match 1.91 / 2.56, 1.95 / 2.45,
  2.04 / 2.35 on the 480.
- S10c-3 Demolition. Ran on (A): vehicles drive round an obstacle when a way round exists; demolition stays rare and is
  reported. Measured: demolitions wanted 0 in every set (T126 report).
- S10c-4 USA belts. Ran on (A): US engineers build the belt as one log barrier line. Measured: belts in 10.0 / 1.9,
  8.75 / 1.25, 8.1 / 2.5 % of matches.
- S10c-5 T109 mines. Ran on (A): 45 % of matches (`S10c-ai.toml`), in place of 50 %. Measured: T109 is not among the
  misses in `measure.txt`.
- S10c-6 Front wire shape. Ran on (A): a 20 m front line with a 30 m mine row behind it. Measured: own wire seen by the
  enemy AI 55.5 / 40.1, 51.0 / 27.8, 43.0 / 36.2 % (T122 bound 50; the US rows miss); breaches 0 in every set (T123
  report). (B) 60 m lines with a field at each end stays open for a later slice.
- Open items, no answer needed yet:
  - own_blocked 3 / 2 on seeds 1-80 (0 / 1 and 0 / 0 on the other sets; T115, bound 0). Cause in the log (seed 15,
    Germany base 0). Rule R6b (B1-2) is proposed in the log and not built; it needs its own key.
  - Breaches stay 0: the short front line is rounded (S10c-6); covering fire is S11.
  - Front builders lost is over the 0.5 bound on both sides, not only the US.
  - Belts are rare (1.25-10 % of matches) and no demolition is wanted.
  - The front AT row waits behind FrontAp and wire (`at_wait` in the log; options A and B there, not decided).
  - Not run yet: the full release suite (`check.sh tests`), `tools/verify.sh` and GitHub CI on the branch.

## S11a1 open owner questions (asked 2026-11-16 by the Plan stage; each runs on (A): "assumed overnight 2026-11-16, owner asleep")

Spec `docs/slices/S11a1.md` 11; log `docs/slices/S11a1-log.md`; targets `docs/slices/targets/S11a1.toml`. Built on the
branch; measured at `e149258` (item 16 and 17). The design's gate "S11a ships only if S5 and S6 met their HE-share targets" is met (S5b 13.3-13.9 %, S6
19.4 %, cap 26.3 %; S10c-ai 19.8 %), so it is not a question.

- S11a1-1 Split of S11a. (A) Two slices as the design's critic note proposes: S11a1 (gun fire missions, radio
  observers, leIG 18 and 105 mm M3, basic AI, preparation fire for the breach) and S11a2 (crate-only shells, friendly
  fire, registration, counter-battery, displacement, defensive fire). **(A) assumed overnight 2026-11-16, owner
  asleep.** (B) One slice S11a (about 0.9x S3; not buildable in three stages).
- S11a1-2 Shells in S11a1. (A) Infantry guns rearm in base at the S9 shell price (every shell paid; the AI keeps a
  25 % budget) until S11a2 adds crates at the gun and AI crate carrying; switch `[supply] arty_base_rearm`. **(A)
  assumed overnight 2026-11-16, owner asleep.** (B) HQ, dump and truck only now: an AI gun in base fires one load
  (10-12 rounds) a card. Either way Q4's "the base does not refill guns" becomes the default in S11a2.
- S11a1-3 Friendly fire from indirect HE (Q22, decided yes for S11a). (A) Built in S11a2 behind its own switch and
  ablation, because it changes every mortar mission of both sides; until then the AI lifts fire with own men within
  50 m and a human's own shells do not hurt his men. **(A) assumed overnight 2026-11-16, owner asleep.** (B) In S11a1.
- S11a1-4 Tows. (A) Both gun cards come without a tow; the AI emplaces the gun where it arrives (in base: both guns
  cover Kessel Ridge from there); tow variants, siting and displacement in S11a2. **(A) assumed overnight 2026-11-16,
  owner asleep.** (B) The M3 card brings a CCKW now (380 + tow); the AI still does not move it.
- S11a1-5 Prices and caps. (A) leIG 18 320 MP (gun cards have no German cut, S9-4), 105 mm M3 380, VB-Trupp 80 (the
  roster's 100 less the German 20 % cut, S5a-37), FO Party 120; guns at most 2 alive, unlock 3:00 / 4:00; the AI buys
  at most one gun and one observer team. **(A) assumed overnight 2026-11-16, owner asleep.** (B) VB-Trupp 100 (no cut
  for a support team).
- S11a1-6 Preparation fire. (A) Only the breach job waits for it, at most 60 s; ordinary assaults go as today.
  **(A) assumed overnight 2026-11-16, owner asleep.** (B) Every assault on a point with a known trench holds its first
  bound 60 s for the guns (design 6.4; changes every attack, so it needs its own ablation: S11a2 candidate).
- S11a1-7 Breach target T131. (A) Hard: with preparation fire the staged AI breach opens the wire and places its
  charge in at least 60 % of 20 runs (the S10a-16 (A) bar; 0 % today: the base `breach_ai` row, review C-7), attackers' losses a report. **(A) assumed
  overnight 2026-11-16, owner asleep.** (B) A report until S11a2. Note: breach jobs still do not START in matches
  (T123, S10c-6); preparation fire does not change that.
- S11a1-8 HEAT rounds (Igr 38 HL, M67). (A) In S11a2 (2 a gun, rare cap), so this slice's measure does not move the
  armour rows. **(A) assumed overnight 2026-11-16, owner asleep.** (B) Now.
- S11a1-9 Human play before a presenter pass. (A) The four cards are buyable on the branch although the HUD has no
  gun fire-mission tool and no model for them; a presenter pass and the `godot` headless tests come before a merge to
  main is asked. **(A) assumed overnight 2026-11-16, owner asleep.** (B) Add a `hidden` card key and hide them first.
- S11a1-10 T131 while the breach job gives up (review C-7, 2026-11-17). (A) Keep T131a/b as reports until the breach
  job's give-up rule is decided (S10a-16); the staged row's mortar now observes the nest. **(A) recommended, assumed
  overnight 2026-11-17, owner asleep.** (B) Loosen the `run_breach` `lost` rule for breach jobs under preparation fire.

## S11a2 open owner questions (asked 2026-11-17 by the Plan stage; each runs on (A): "assumed overnight 2026-11-17, owner asleep")

Spec `docs/slices/S11a2.md` 11; log `docs/slices/S11a2-log.md`; targets `docs/slices/targets/S11a2.toml`. Built on the
branch and measured at `e149258` (item 17).

- S11a2-1 Scope of S11a2 (two Build stages tonight). (A) Friendly fire from indirect HE (Q22) and counter-battery by
  sound with AI counter-fire (the answer to a gun in base); crates, tows, displacement, registration, flash and
  defensive fire go to S11a3. **(A) assumed overnight 2026-11-17, owner asleep.** (B) Crates and AI crate carrying
  instead of counter-battery.
- S11a2-2 Extent of friendly fire. (A) Own men only (blast, suppression, fragments) from fire-mission rounds of
  mortars and infantry guns; own vehicles, guns, structures, wire and tracks stay immune; a round in flight passes
  through own men. **(A) assumed overnight 2026-11-17, owner asleep.** (B) Everything own is hit.
- S11a2-3 Own-fire deaths in the numbers. (A) Not kills (the HE share and the kill tables keep their meaning); a
  separate count per player; losses and tickets as any death. **(A) assumed overnight 2026-11-17, owner asleep.**
  (B) Counted as kills of the firing player.
- S11a2-4 AI safety distances under friendly fire. (A) No mission, and a lift of a running one, with own men inside
  50 m of the target or walking onto it inside 100 m; the mortar spotter stands 70 m short (was 35).
  **(A) assumed overnight 2026-11-17, owner asleep.** (B) 100 m for every case.
- S11a2-5 What the enemy can hear. (A) Fire-mission rounds of infantry guns only (marker after 3 rounds); mortars
  (5 rounds) and flash ranging later. **(A) assumed overnight 2026-11-17, owner asleep.** (B) Mortars now too.
- S11a2-6 AI counter-fire. (A) Before its observed targets, one mission of 4 rounds per new evidence, map fire on a
  marker no wider than 60 m, inside the reserve and the 25 % shell budget (the one case where the AI fires
  unobserved). **(A) assumed overnight 2026-11-17, owner asleep.** (B) Only when the gun has no observed target.
- S11a2-7 Target T134 (AI gun rounds observed >= 80 %). (A) Counter-fire rounds count with the observed ones.
  **(A) assumed overnight 2026-11-17, owner asleep.** (B) Keep the S11a1 metric and accept a miss.
- S11a2-8 Shells by crate (follow-up of S11a1-2). (A) Infantry guns keep the priced base rearm until guns leave the
  base (S11a3, with tows and siting): inside the base a crate run is a 20 m walk at the same price, and one 32 kg
  crate holds a single 19 kg M1 round. **(A) assumed overnight 2026-11-17, owner asleep.** (B) Turn base rearm off
  now with no AI carrying (an AI gun then fires one load a card).
- S11a2-9 Target T142 (the AI lifts its fire in time in >= 90 % of 20 staged runs; the design's test 10). (A) Hard.
  **(A) assumed overnight 2026-11-17, owner asleep.** (B) A report until S11a3.
- S11a2-10 US counter-fire share over 50 %. Measured at `e149258`: US counter-fire is 62.8 % of its gun rounds (T148d,
  report bound 50). The S11a2 log names the knob outside the spec's keys. (A) recommended: add a per-gun counter-fire
  round budget in the next pass. (B) Accept the share; T148 stays a report. Not decided: the branch runs as measured.
- S11a2-11 AT guns in cover fell under the floor. Measured at `e149258`: 57.8 % (Germany, T81a) and 56.4 % (USA, T81b)
  against a floor of 60; they were 84-94 % at S10c-ai. Cause not found (first suspects: the artillery pot or the
  new guns changing AT-gun siting; the cover metric counting infantry guns). (A) recommended: diagnose and fix before
  merging S11a1 and S11a2 to main. (B) Merge, then fix after.
- S11a2-12 Artillery manpower lowers vehicle buys. M10 bought in 32 % of matches (56 % at S10c-ai), M3A1 16 % (36 %);
  Pz IV bought in 89.4 % (T54a, floor 90) with MP share 9.97 (T54c, floor 11). (A) recommended: lower the artillery pot
  in the balance pass. (B) Accept the lower buys and the two misses.

## S11b open owner questions (asked 2026-11-19 by the Plan stage; each runs on (A): "assumed 2026-11-19, owner asked not to be asked")

Spec `docs/slices/S11b.md` 11; log `docs/slices/S11b-log.md`; targets `docs/slices/targets/S11b.toml`. Planned, not built.
Revised the same day with the measured diagnosis of S11a2-10..12 and a plan review.

- S11b-1 Scope of S11b (three Build stages). (A) Field howitzers as base guns, light mortars, HE rifle grenades, and
  the fixes from the S11a2 measure (S11a2-10, S11a2-11); tows, displacement and crates stay S11a3. **(A) assumed
  2026-11-19, owner asked not to be asked.** (B) Tows and displacement first, S11b after.
- S11b-2 Field howitzers without a tow. (A) The leFH 18 and 105 mm M2A1 cards bring no tow: the gun emplaces where it
  arrives (the base), cannot be pushed, and fires flat from there; often "masked" on Kessel Ridge. **(A) assumed
  2026-11-19, owner asked not to be asked.** (B) Human-only tow variant cards now (the AI still does not move them).
- S11b-3 The AI's howitzer. (A) A second gun from 8:00 with its own 15 % income share (beside the infantry gun's 15 %)
  and 300 MP kept after the buy: bought at about minute 11 (Germany) and 12 (USA); the median match lasts 18-19.4
  minutes. It displaces vehicles further (S11b-10). **(A) assumed 2026-11-19, owner asked not to be asked.** (B) Inside
  the infantry gun's share: bought at about minute 20-22, after most matches end.
- S11b-4 Light mortar prices. (A) kz. GrW 42 181 MP (the roster's 220 with the German cut and the S6 factor, as the
  heavy team's 300 became 247), 60 mm M2 240 MP. **(A) assumed 2026-11-19, owner asked not to be asked.** (B) 220 and 240.
- S11b-5 Light mortar range. (A) Scaled as the heavy tubes are (600 m in the game for 2,400 / 2,900 m): kz. GrW 42
  275 m, 60 mm M2 370 m. **(A) assumed 2026-11-19, owner asked not to be asked.** (B) 600 m like the heavy tubes.
- S11b-6 Rifle grenadiers. (A) Two riflemen of a full US Rifle Squad and one of a full Grenadiergruppe carry three HE
  rifle grenades each; card prices unchanged; the base does NOT refill them (crate-only by design; a crate source comes
  with crates), so three is a man's load. **(A) assumed 2026-11-19, owner asked not to be asked.** (B) One grenadier a
  squad on both sides.
- S11b-7 Rifle grenade figures: the design holds none; **owner to confirm.** (A) M17 on the M7 launcher: the Mk 2
  body, about 0.68 kg, about 57 g TNT, maximum taken as 180 m (about 180-200 m); Gewehr-Sprenggranate from the
  Schiessbecher: about 250 g, about 30 g filler, maximum about 250 m, about 60 m/s; so the German burst is about half
  the US one with more reach. **(A) assumed 2026-11-19, owner asked not to be asked.** (B) No rifle grenades until sourced.
- S11b-8 Rifle grenade kills in the numbers. (A) Their own kill class, counted inside the HE share (design 6.2; T8's
  top of 26.3 % stays hard). **(A) assumed 2026-11-19, owner asked not to be asked.** (B) Counted with hand grenades,
  outside the HE share.
- S11b-9 Counter-fire share (follow-up of S11a2-10 (A); measured). (A) An AI gun never orders counter-fire twice in a
  row: on a 40 set the US share fell from 62.3 to 47.3 % and counter-fire still happened in 97 % of matches. **(A)
  assumed 2026-11-19, owner asked not to be asked.** (B) A per-gun round budget as well.
- S11b-10 Vehicle buys beside the artillery (follow-up of S11a2-12; measured: the gun and observer cards themselves
  take the manpower, about 500 MP a US match; not the pot's timing, not shells). (A) Accept that a new card displaces
  vehicles, and re-base T54, T30 and T40 in the balance pass; they stay as they are in `S11b.toml`. **(A) assumed
  2026-11-19, owner asked not to be asked.** (B) Raise the artillery income share (30 % restored buys but read Germany
  57.5 % on one 40 set).
- S11b-11 T81 (AT guns in cover; follow-up of S11a2-11; measured: a counting error). (A) Infantry guns leave the
  AT-gun emplaced and crew-lost counts; a patched set played identically and read 86.5 / 87.2 %. **(A) assumed
  2026-11-19, owner asked not to be asked.** (B) Keep the metric and accept the miss.
- S11b-12 Artillery limit group. (A) At most two gun cards (infantry guns and field howitzers) alive a player
  (design 4.7). **(A) assumed 2026-11-19, owner asked not to be asked.** (B) Per-card caps only.
- S11b-13 Other figures with no source in the design (a citation task). (A) Use the plan values until sourced:
  `how_reserve_mp` 300; `rifle_grenade_gap_s` 15; grenade minimum 30 m, 3 s to fit and 3 s to stow, M17 speed 50 m/s;
  howitzer top charges 325 / 333 m/s and their traverse (the M3's); the 60 mm bomb's burst (the 81 mm block x 0.75)
  and its charges; the light mortars' game ranges (S11b-5); the FH Gr 38 complete round at 19 kg (as the M1 HE).
  **(A) assumed 2026-11-19, owner asked not to be asked.** (B) Hold the affected defs back until sourced.
- S11b-14 (asked by the S11b fix stage) AI light mortars are off by default because with them Germany wins 27.5 % of a
  40 set (11/40, HE share 23.1 %; 20/40 without them; both measured before the fix stage's howitzer pot, grenades off;
  the shipped default with the pot reads 20/40, HE 25.5 %). Germany's own kz. GrW 42 carries the drop:
  light mortars for Germany only 9/40, for the USA only 17/40. A kz. team fires about 10 of its 18 bombs a match, a 60 mm
  team about 34 (30 carried, the rest resupplied); Germany's light buys take heavy-mortar buys (190 -> 132 a 40 set) and its bombs fall
  93 -> 72 a match. The 60 mm burst scaled to the bomb (55 rays, 900 cm) did not help: 13/40. `[ai.arty] light` is
  AI-only: the cards stay buyable by a human player. (A) Leave off until the balance pass B2 tunes the two cards
  (levers `docs/slices/targets/B2-levers/s11b_light_*.toml`). **(A) assumed 2026-11-19, owner asked not to be asked.**
  (B) On now.

## B2 balance pass open owner questions (asked 2026-11-19 by the Plan stage; each runs on (A): "assumed 2026-11-19, owner asked not to be asked")

Plan `docs/slices/B2-balance.md` 9; stage contracts `B2-run-1.md`, `B2-run-2.md`; log `docs/slices/B2-log.md`; targets
`docs/slices/targets/B2.toml`. Run 2026-11-19 on the S11b head (branch); results in `docs/slices/B2-log.md`; no default changed. Found without playing a match, from the 480 at
`e149258`: Germany wins 24.6 % of the 114 matches that end at 14-17 minutes and 47.0 % of the other 366; its manpower
exchange is not worse than the USA's; US rifles kill 48.3 men a match against 28.9.

- B2-1 What the pass may change alone. (A) Only AI rule keys and card `ai_weight`s of the plan's tier 1 (`[ai.mix]`
  line, vehicle, mg; `[ai.arty] income_max_pct`; the 7-man Grenadiergruppe's weight). Prices, the pop cap, start squads,
  squad make-up, a gun's tow, the VG-Sturmgruppe's weight and weapons are screened as override files and reported with
  numbers, not applied. **(A) assumed 2026-11-19, owner asked not to be asked.** (B) Also apply one owner-class lever if it passes the same rule.
- B2-2 If no AI setting passes (the evidence points at unit content). (A) Change nothing, as B1; the owner picks from the
  measured list: the 57 mm towed by a truck, a third or cheaper German start squad, more VG-Sturmgruppen, the German
  price cut, the pop cap, an AI opening-buy rule (code), or accept about 42 %. **(A) assumed 2026-11-19, owner asked not to be asked.**
  (B) The pass applies the best owner-class lever itself.
- B2-3 **16 target floors re-based, in `B2.toml` only; the owner decides** (S11b-10 (A) asked for them). Rule: 0.85 x the
  lowest measured set, 0.95 for a near miss. T54a Pz IV bought 90 to 80 % of matches; T54b M4 90 to 60; T54c Pz IV manpower
  11 to 9; T54d M4 manpower 10 to 5.5; T30a, T30c, T54e StuG 60 / 60 / 80 to 40; T30b, T30d, T54f M10 60 / 60 / 75 to 25;
  T40a, T40c, T72a Sd.Kfz. 251/1 60 to 25; T40b, T40d, T72b M3A1 card 60 to 10 (made again from the S11b measure).
  (A) As in `B2.toml`. **(A) assumed 2026-11-19, owner asked not to be asked.** (B) Keep the design floors as visible hard misses.
- B2-4 T97, builders' share of the infantry plans (16.5-18.2 %, cap 12 since S10c-ai's builder floor and front jobs). (A) Not
  re-based: a visible hard miss until the owner chooses a cap or lowers `builder_cards_min`. **(A) assumed 2026-11-19, owner asked not to be asked.**
  (B) Re-base the cap to about 20.
- B2-5 T31 (all vehicles' share of kills; Germany 32.0-33.5 %, cap 30) and T55 (Sd.Kfz. 234/1 at 14.7-15.3 % of Germany's
  kills, cap 10). (A) Both stay reports: AT and infantry guns are counted inside T31 (a vehicles-only share is a later CLI
  change), and the armoured car is not touched while Germany is under 45 %. **(A) assumed 2026-11-19, owner asked not to be asked.**
  (B) Lower `[ai.recon] income_max_pct` now.
- B2-6 The aim. (A) 47-53 %: 320 fresh paired matches can prove a gain of about 6 points, not the 3.3 points to the band's
  edge; a lever worth +3 is reported, not applied. **(A) assumed 2026-11-19, owner asked not to be asked.** (B) Apply any gain that reaches 45 %.
- B2-7 Seeds. (A) Screen 2001-2080, confirm 2101-2280, judges 1-280 (the measure only), held out 5001-5080 once (B1's
  301-380 was already played by S6b). **(A) assumed 2026-11-19, owner asked not to be asked.** (B) Reuse B1's blocks 1001-1280.
- B2-8 The 57 mm's armed tow (S9-3 (A)): every US 57 mm card brings an M3A1 with a .50, which makes 5.7 of the USA's 12.8
  vehicle-MG kills a match; the Pak 40's Sd.Kfz. 11 is unarmed. (A) Screen the gun towed by the unarmed CCKW (S9-3 option B)
  as an override and report its number; never applied without the owner. **(A) assumed 2026-11-19, owner asked not to be asked.**
  (B) Leave S9-3 alone and do not screen it.
- B2-9 Rifle-grenade gap 15 -> 25 s (`[items] rifle_grenade_gap_s`; tested under its own rules R1-R4 and J1-J4, `B2-log.md`).
  HE share 25.69 on fresh 320 matches and 25.73 on the 480 judges (cap 26.3; base 26.66 and 26.50); Germany unchanged at
  44.17 %. The one miss is T121b (USA mines set off a match, floor 0.3, 0.45 -> 0.281), a target the lever does not touch.
  (A) Apply it as one commit (a revert of `63e4b04`); the HE cap holds on both sets. (B) Leave 15.
- B2-10 57 mm gun tow. The armed M3A1 (S9-3 (A)) makes 5.7 of the USA's kills a match. A truck tow gives Germany +16.2 points
  on 160 matches but lifts HE to 29.3, and the AI then runs the tow trucks as supply trucks. (A) Keep the M3A1 and take its
  gun off or price it, in a later slice. (B) Leave the tow as it is.
- B2-11 Start content. A third free German squad gives +19.4 on 160 matches (HE 27.2, over the cap). Germany already has more
  men at 5 minutes (49.9 against the USA's 43.7), so the USA does not win by early numbers; the USA leads at 10 minutes.
  (A) Leave start content; look at the mid game (10 minutes on) in the next balance pass. (B) Change the German start now.
- B2-12 HE cap 26.3. It is the floor that stops most levers that help Germany: of the four screened levers that lift Germany
  by 8 points or more, three go over the cap (26.5, 27.2, 29.3). (A) Keep the cap and retune HE sources first. (B) Move the
  cap, a design change; not measured.

## B3 open owner questions (asked 2026-11-22 by the B3 Plan stage; each runs on (A): "assumed 2026-11-22, owner away; Eva may reopen")

Plan `docs/slices/B3-balance.md` 9; stage contract `B3-run-1.md`; levers `docs/slices/targets/B3-levers/`. B2-9..12 above stay unanswered.

- B3-1 What the pass may apply alone. (A) AI keys (`[ai.arty] shell_mp_max_pct`, `mission_rounds`; `[ai.armour] heavy_save_max_s`; `[ai.mix] vehicle`) and cost and unlock of S13's own cards, the levers S13 plan 7 named; everything else in section 3 only with its question answered. (B) Also the owner-class levers that pass their rule. (C) AI keys only. **(A) assumed 2026-11-22, owner away; Eva may reopen.**
- B3-2 The M17 rifle grenade's burst (S11b-7 figures are "owner to confirm"). (A) Reduce it to blast 160 cm and 64 fragments if it passes rule H: a US grenade takes 0.73 men a shot against 0.43 for the German one, and 7.2 % of all the USA's casualties caused. (B) Keep the Mk 2 hand grenade's burst; goal 1 then rests on the shell budget and B2-9. (C) Make it equal to the German grenade (160 cm, 48 fragments). **(A) assumed 2026-11-22, owner away; Eva may reopen.**
- B3-3 How the AI gets the M36 and the Tiger. (A) M36: `ai_heavy = true` (bought when enemy armour was seen, before the Jumbo; a tank destroyer's own reason to appear), kept only if the Jumbo stays at or above 10 %. Tiger: no code now; it stays the second heavy, T179c stays hard and visible, raised only through `hv_save360` and `hv_ger_reach`. (B) M36 by `ai_weight` 150: the slot's first buy, the Sherman second. (C) Leave both; T179b becomes a report until an AI rule exists (a later engine slice, with the Tiger-first pick). **(A) assumed 2026-11-22, owner away; Eva may reopen.**
- B3-4 Old rounds and case 45 (S13-1b, S13-F1). (A) Class the two twins only: 7.5 cm PzGr 40 `apcr`, 3-inch M62 `apcbc`; T184d hard again; every other old round stays on the cosine law, so medium against medium is bit for bit as now. (B) Class every old full-calibre AP and APCR round: consistent, but the L/48 PzGr 39 then fails against a Sherman glacis at 30 degrees, a large change against Germany at 42 %. (C) Leave all; T184d stays a report and the Jumbo keeps falling to two rare rounds. S13-9 (the 3-inch M62 row) is a penetration row and is not part of this pass. **(A) assumed 2026-11-22, owner away; Eva may reopen.**
- B3-5 Stabilisation (S12-17). (A) Report only: B3 changes nothing for it; the longer bleed-out window is screened if time allows and its numbers go to the owner. (B) Apply the longer window if the stabilised share doubles and the floors hold. (C) A third medic team (`[ai.medic] max_teams` 3): not recommended, medics in the field already cost 0.9 HE points. **(A) assumed 2026-11-22, owner away; Eva may reopen.**
- B3-6 The aim. (A) In order: HE under the cap with Germany not down more than 2 points; the heavies bought; Germany toward 45-55 only through levers that pass their rule. A set that fixes the cap and leaves Germany at 42-45 % is a valid result. (B) Apply nothing unless Germany also reaches 45 %. **(A) assumed 2026-11-22, owner away; Eva may reopen.**
- B3-7 Method. (A) T31 logged, not a screen floor (it is a report row, B2-5 (A)); noisy hard rows listed apart in the judge (section 5); seeds as in section 5. (B) Keep B2's floors and judge rule unchanged. **(A) assumed 2026-11-22, owner away; Eva may reopen.**

## S12 open items (2026-11-21; branch `claude/project-thread-aj1tww`, log `docs/slices/S12-log.md`)

- S12-16 **T8 known miss (accepted for now, first item of the next balance pass).** With wounds on, HE's share of
  casualties (each man counted once, by the class that first took him out) reads 29.97 % on the 40 set against the hard
  cap 26.3 (25.5 with every S12 switch off). The double count found in Review was removed; the rest is a real shift.
  Decided 2026-11-21: ship S12 with T8 as a known miss; the next balance pass starts with it. Germany 20/40, length unchanged.
  B3 run 2 (2026-11-22, `f4e610b`, `B3-log.md`): the shell budget 6 % with direct fire inside it and the M17 cut read
  26.05 on 320 fresh pairs, but the 480 judge reads 27.40 / 24.96 / 26.70 (about 26.35 pooled; T8a misses): still open.
- S12-17 Stabilisation stays at about 3 % of downs (medics reach few men); not tuned (features first). Read it in the balance pass.

## S13 open items (2026-11-21; branch `claude/project-thread-aj1tww`, log `docs/slices/S13-log.md`)

- S13-F1 **Armour-test case 45 known miss (for the balance pass, with S13-1b).** The Jumbo leading into a PaK 40 at 400 m
  survives 3 hits in 0 of 10 runs (bound >= 6). Every hit is one of the PaK's 2 old PzGr 40 (APCR without `slope_class`:
  S13-1 (A) classes only the new rounds), which the cosine law lets through the 101.6 mm @ 47 glacis. Not fixed in S13 (no old
  round is classed); `targets/S13.toml` T184d is `kind = "report"` with this note until the balance pass decides S13-1b.
  B3 run 2 (`f4e610b`, B3-4 (A)): PzGr 40 `apcr`, 3-inch M62 `apcbc`, T184d hard again; armour test case 45 in `B3-log.md`.
- S13-F2 **T155 (T179a-d) misses on the 40 set: tuning, not a defect** (diagnosis in `docs/slices/S13-log.md`, Fix stage). The
  M36 and 76 W lose the vehicle slot's pick to the Sherman and the M10 by roster order at equal weight and zero buys; the AI
  rarely makes a second vehicle-slot buy. Smallest lever set for the owner there.
  B3 run 2 (`f4e610b`, B3-3 (A)): M36 `ai_heavy`; the 480 judge reads T179a-d pass (Panther 28.75, M36 41.25, Tiger 14.4,
  Jumbo 16.25 % of skirmish160 matches); the 76 W stays at 0 (T179e floor 0).

## S14 assault kit owner questions (asked 2026-11-22 by the Plan stage; each runs on (A): "(A) assumed 2026-11-22")

Plan `docs/slices/S14-assault-kit.md` 12 (notes `S14-research-history.md`, `S14-research-code-audit.md`). Nothing built; S14
builds after S13. Minor data and scope points (the US weapon is the M1A1 for the 1944-06 card, fuel figures, unchanged prices,
strike numbers) are settled in the plan and only reported there.

- S14-1 Fire and own men. (A) The flame stream passes own men like every direct round, but burning ground burns everyone standing in
  it, own men included (own burns count as own fire, not kills). **(A) assumed 2026-11-22.** (B) Burning ground harms enemies only.
  (C) The stream hits own men too.
- S14-2 Hits on the fuel tank. (A) No rupture rule: a hit on the operator is an ordinary hit (sources: rare in fact; no new random
  draw). **(A) assumed 2026-11-22.** (B) A hit in the back ruptures the tank 1 time in 10: he burns and leaves a 2 m fire.
- S14-3 Where the flamethrowers go. (A) Into the Sturmpioniergruppe and the Assault Section as the roster says, one man's weapon
  swapped; squad size, price, cooldown and AI weights unchanged. **(A) assumed 2026-11-22.** (B) A separate two-man flamethrower
  team card per side. (C) Both.
- S14-4 Melee scope. (A) An automatic strike only when the man's weapon is not ready (reloading, empty, crew weapon), plus a Strike
  key (`V`); rifle butt 1.0 m and spade 0.6 m, no bayonets. **(A) assumed 2026-11-22.** (B) Also a Fix bayonets order (1.8 m reach,
  worse aim while fixed). (C) Automatic only, no key.
- S14-5 Who may use Clear trench. (A) Any infantry squad with grenades, players and AI. **(A) assumed 2026-11-22.** (B) The two
  assault cards only (Sturmpioniergruppe, Assault Section).
- S14-6 Flame on vehicles and works. (A) Flame hurts men only: it stops on vehicles, fortification pieces, crates and wire with no
  effect; open-top crews are safe in S14. **(A) assumed 2026-11-22.** (B) Open-top crews and exposed seats burn.
- S14-7 Burning ground and movement. (A) Paths ignore fire; men standing in it step out on their own, and Clear trench waits for its
  own fire to die. **(A) assumed 2026-11-22.** (B) Paths route around burning ground (a new navigation layer; costs tick time).

S14 Fix (2026-11-25, review `docs/slices/S14-review.md`; log `S14-log.md` "S14 Fix"). Each runs on (A) until answered:

- S14-8 R1, flame through a bunker slit. With the aim fixed (finding 1) the real M1A1 man puts 6 hits into the bunker over 10 runs
  (fortify10 row: 22 % of the men out, 0 runs with 4 of 5 out; closed control 8 %, back control 0 %); in 5 of 10 runs the
  defenders kill him first. Where he empties his tank, 0-2 of 5 are out: a flat 27 m/s stream through a 0.4 m slit passes over
  men crouched on a pit floor 1.5 m down and ends on the back wall's side with no effect (S14-6 (A)). (A) R1 (T186) stays
  report-only; the slit is a balance-pass matter. **(A) assumed 2026-11-25.** (B) A particle ending on a fort piece's inner side
  splashes (burns men within `near_cm` of it). (C) Lower the R1 target (e.g. 2 of 5 out in half the runs).
- S14-9 The `V` key (review finding 10). (A) With `[melee]` on, `V` strikes only when an enemy is within 1.5 m of a selected man,
  otherwise it boards as before. **(A) assumed 2026-11-25.** (B) A new key for Strike, `V` stays Board.
- S14-10 A lone flame job's end (finding 13). The job's own Attack task counts in "point attacked", so "the point is no longer
  attacked" ends a job only with the point taken; counting only other squads' tasks ended every lone job at once. (A) Keep it: a
  lone job is bounded by `[ai.flame] job_max_s` (T192 now gates the oldest open job at the match end, <= 185 s). **(A) assumed
  2026-11-25.** (B) End a lone job when the AI's plan for that point is dropped (needs the plan's id in the job).
- S14-11 The AI's bunker target (finding 2). (A) A bunker counts as a flame target when seen, remembered, or on the AI's record of
  enemy works (`known_by_record`); men in a trench are remembered `[ai.flame] memory_s` (60) after they were last seen. **(A)
  assumed 2026-11-25.** (B) Seen only (the plan's text), with the trench memory alone.
- S14-12 Where the flame man stands at a trench (finding 2). A flat stream enters a 1.8 m trench only from its lip. (A) `[ai.flame]
  stand_trench_m` 2 (bunkers keep `stand_m` 20); a default chosen so the rule fires, not tuned. **(A) assumed 2026-11-25.**
  (B) A lobbed stream (aim higher, longer range) so he can stand off.
- S14-13 Melee reach in height (finding 3). (A) A strike reaches feet up to 2 m below or above the striker's (`[melee] down_cm`,
  `up_cm`): any stance and into a trench from its lip; the enemy nearest the strike point on his body axis is hit. **(A) assumed
  2026-11-25.** (B) Same level only (crouched and prone men hit, no reach into a trench).

## S15 USSR owner questions (asked 2026-11-26 by the Plan stage; each runs on (A): "(A) assumed 2026-11-26")

Plan `docs/slices/S15-ussr.md` 13 (notes `S15-research-history.md`, `S15-research-code-audit.md`). Nothing built. Owner rule
2026-11-25 applies: no balance run in S15; the proof is a neutral 40 set and one breakage 40 set (Germany against the USSR); the
USSR's win rates are reported, not judged; the roster card's "inside the win-rate band" waits for a later balance pass. Minor data
and scope points (every price as a starting value copied from the nearest German or US card, 9 and 6 men, ROKS-3 10 L, T-34-85 55
rounds, ids with the `ussr_` prefix) are settled in the plan and only reported there.

- S15-1 Who the USSR fights. (A) Germany against the USA stays the default match everywhere; Germany against the USSR is the second
  pair (pick Germany, then the opponent; pick the USSR and the opponent is Germany); the USSR against the USA only with a "free
  skirmish" setting, off by default (as `SYSTEM_DESIGN.md` 2.3 decided). **(A) assumed 2026-11-26.** (B) Any two different factions,
  always. (C) Germany against the USSR only, no free setting.
- S15-2 How much of the roster the first build ships. (A) The full list: 14 infantry cards, 5 guns (45 mm, 57 mm ZiS-2, ZiS-3, 76 mm
  regimental, 122 mm M-30), 7 vehicles (T-34-85, SU-85, SU-76M, IS-2, BA-64B, Studebaker truck, M3A1 scout car); the slice is then
  about 1.3x S3, not the card's 0.6x. **(A) assumed 2026-11-26.** (B) A core first (infantry, T-34-85, ZiS-3, truck), the rest in a
  later slice.
- S15-3 Tank riders. (A) Any infantry squad that fits may ride a Soviet tank's deck, fully exposed; the riders jump off by themselves
  when one of them is suppressed, when the tank is hit, or when its main gun fires; only Soviet tanks have rider places. **(A)
  assumed 2026-11-26.** (B) Riders stay on until the player orders them off. (C) German and US tanks get rider places too.
- S15-4 The steel breastplate (SN-42). (A) A real 2 mm plate on the front of the torso of the assault group's men: it stops
  pistol-calibre bullets (MP 40, M3), light fragments and rifle-butt or bayonet strikes; rifles, MGs, blast and flame ignore it; no
  speed cost. **(A) assumed 2026-11-26.** (B) As (A), and the men walk 5-10 % slower. (C) No plate rule (a look only).
- S15-5 Soviet infantry against tanks. The Red Army had no rocket launcher of its own. (A) An AT rifle team (holes half-tracks,
  armoured cars and thin side plates only), satchels, and three AT guns; the thrown HEAT grenades (RPG-43, RPG-6) wait, because they
  need a new item kind. **(A) assumed 2026-11-26.** (B) Add the thrown AT grenade in S15. (C) Captured Panzerfausts as a rare item.
- S15-6 The 50 mm company mortar: **sources disagree.** The design says it had left line companies by 1944; battleorder.org keeps
  two per rifle company until the December 1944 table. (A) No 50 mm card; the 82 mm is the only Soviet mortar. **(A) assumed
  2026-11-26.** (B) A 50 mm RM-38 light mortar card (the AI's light mortars are off for every faction, S11b-14).
- S15-7 Bayonets. (A) Soviet riflemen strike with the fixed bayonet (1.8 m reach) whenever a strike happens; no aim penalty (the
  rifle was sighted with the bayonet on), no order; Germans and Americans stay without bayonets (S14-4). **(A) assumed 2026-11-26.**
  (B) With an aim penalty while fixed. (C) No bayonets, rifle butt only.
- S15-8 Captured mortar bombs. (A) A Soviet 82 mm crew standing at a taken German shells crate fires German 8 cm bombs, less
  accurately; the German mortar cannot fire Soviet bombs; the AI does not go looking for crates. **(A) assumed 2026-11-26.** (B) Drop
  the rule (it is rare in play).
- S15-9 A transport card. (A) The lend-lease M3A1 Scout Car (6 men, open top) as the USSR's carrier beside the tank riders, so the
  AI's transport habit has a card. **(A) assumed 2026-11-26.** (B) Tank riders only, no carrier.
- S15-10 The same faction on both sides of a 1v1. (A) Refused in the menu and the online lobby, as today; the command-line tool
  allows it, as today; team modes follow S17-8. **(A) assumed 2026-11-26.** (B) Offered in the menu and the lobby.
- S15-11 The MG card. (A) The Maxim M1910/30 on its wheeled mount; the SG-43 is added as data without a card. **(A) assumed
  2026-11-26.** (B) The SG-43 as the card.
- S15-12 A map for the Eastern front. (A) None in S15: the USSR plays on the existing maps. **(A) assumed 2026-11-26.** (B) A third
  map in S15.
- S15-13 The ZiS-3's second role. (A) Direct fire only (AP at armour, HE at what it sees); firing it as artillery, with the
  artillery-cap rule, comes later. **(A) assumed 2026-11-26.** (B) Indirect fire in S15.
- S15-14 Rider places on the T-34-85 and IS-2 (review finding 5; plan 6.1 sends the 9-man sections as riders, plan 6.4 gave 8
  places, so only the 6-man card and weakened squads fit). (A) 10 places on both decks (two on the centre line), a consistency fix,
  not a tuned value; no source counts the men on a deck. **(A) assumed 2026-11-26.** (B) Keep 8 and leave the riders to the balance
  pass. (C) 9-man sections lose the `vehicle_riders` tag.
- S15-15 T213c (ROKS-3 hits) and T213d (bayonet strikes) in the Germany-USSR 40 set (both 0 in Build D). (A) Reports, not hard
  rows: the S14 flame habit is rare for every faction and no AI habit closes to melee reach; sim tests prove both rules
  (`roks3_hits_a_man_at_20_m`, `bayonet_reaches_150_cm`, W4). **(A) assumed 2026-11-26: reports; a close-assault habit is later
  work.** (B) Build a close-assault habit now.

## Planned work queue

1. Infra step (done; on main).
2. Autotune fix (done, merged by the infra step).
3. S6 HQ and ammo crates: built, reviewed and fixed on branch `s6-hq-crates`
   (on main). Open: retake the HUD screenshot, S6-18
   structure selection (its own item when scheduled).
4. S6b mortar AI pass: done on branch `s6b-mortar-ai` (on main). Mortar and
   HE bands met on 160 sets; T5 open (S6b-1).
5. S7 assault guns and TDs: done on branch `s7-casemate-td` (on main; tick fix after the log); log `docs/slices/S7-log.md`, spec `S7-casemate-td.md`, targets `S7.toml`. Open: S7-11, S7-12, S7-15.
6. S8 half-tracks, trucks, recon: split. S8a half-tracks: done, verified and on main (`fa5c50a`;
   spec `docs/slices/S8a-halftracks.md`, targets `S8a.toml`, log `S8a-log.md`; Build A, B, Tune, review and Fix).
   Open: S8a-30, S8a-31 (S8a-28 and -29 settled by S8b). S8b trucks, recon cars, wheeled mobility, recon optics, supply trucks: built, tuned, reviewed and fixed on `s8b-trucks-recon` (Plan, Build A, B, C, Tune, Review, Fix, Docs; spec `docs/slices/S8b-trucks-recon.md`, targets `S8b.toml`, log `S8b-log.md`). Merge next.
7. Sim profiling: S7 tick p99 doubling found and fixed (vehicle re-path loop); a general profile pass is still open.
8. S9 AT guns: built, tuned, reviewed and fixed on `s9-at-guns` (Plan, Build A/B/C, Tune, Review, Fix, Docs; log `docs/slices/S9-log.md`, spec `S9-at-guns.md`, targets `S9.toml`). Not merged. Pooled Germany 45.6 %, 14 hard target misses on the 160 sets. On main since 2026-11-09 (owner OK). Next: S9b, a short AI gun pass (corridor search, then a budget tune; S9-17, S9-19). Open: S9-16, S9-18.
9. S9b AI gun pass: built on `s9b-ai-guns` (Plan, Build, Review, Fix; log `docs/slices/S9b-log.md`, spec `S9b-ai-guns.md`, targets `S9b.toml`). Verified on `1bfc92f`: pooled Germany 48.75 %, 10 hard misses, `tools/check.sh full` all ok; T88 met for Germany, not USA. Owner 2026-11-10: S9b-1 (A), S9b-2 (A), S9b-3 (A); merged, main = `ba0038f` (owner OK). Next: S10a mechanics on `s10a-fortifications`.
10. S10a fortifications, mechanics half: built, reviewed, fixed and verified (head `6d7b98c`; log `docs/slices/S10a-log.md`, spec `docs/slices/S10a-fortifications.md`, targets `S10a.toml`). Merged to main `fa45332` (owner OK 2026-11-11).
11. S10a-ai, the fortification AI and balance half: built, reviewed, fixed and verified, merged to main 2026-11-11 with the owner's OK. Germany 41.67 %; 26 target misses, 21 of them hard; F8 open. (Plan done 2026-11-11: spec `docs/slices/S10a-ai.md`, targets `S10a-ai.toml`). Stages: Build A (layout SAVE 23, S17 path fix, wire index, counters), Build B (buys, materials, building), Build C (breaching, AI breach row, ablation tooling), measure and ablations, Review, Fix, Tune only by spec 9, Docs. Main = `8acc534`.
12. S10b mechanics: built, reviewed, fixed and verified on branch `s10b-obstacles` (from `8acc534`) on 2026-11-12. Merged to main 2026-11-12 with the owner's OK. Verify: 881 tests pass; Germany 41.67 % of 480; 373 targets pass, 25 miss, 6 hard (T106b is the new one, owner decision S10b-5). Log `docs/slices/S10b-log.md`, spec `docs/slices/S10b-obstacles.md`, targets `S10b.toml`.
13. **S10b-ai: done and merged into main 2026-11-15** (from main `72b2970`). Verified: 965 tests pass; Germany 41.88 % of 480; 410 targets pass, 46 miss, 26 hard. Spec `docs/slices/S10b-ai.md`, log `S10b-ai-log.md`, targets `S10b-ai.toml`. Questions S10b-ai-E1 and E3-E8 are all decided (see above).
14. Dedicated balance pass, after S10b-ai or S11 once the roster is stable: fine balance to 45-55 % and the soft per-unit targets (T5c, T5d, T31a-d, T55a/b, T95b/d) are settled there. Slice order: S10b, S10b-ai, S11a/b, then this pass, then S12 to S15.
15. **S10c-ai: done on branch `claude/project-thread-e7nsak` (2026-11-16), merged to main 2026-11-19 with the owner's OK.** Verified so far: `check.sh quick 54009f6` ok, 881 tests pass; c_off 40 set = main 40/40; Germany 40.00 % of 480 (sets 40.6 / 37.5 / 41.9); 270 targets pass, 71 miss (41 hard). Full suite, `tools/verify.sh` and CI not run yet. Spec `docs/slices/S10c-ai.md`, log `S10c-ai-log.md`, targets `S10c-ai.toml`. Owner questions S10c-1..6 above. Next: the merge decision after verify, then S11.
16. **S11a1: done on branch `claude/project-thread-e7nsak` (2026-11-17), merged to main 2026-11-19 with the owner's OK.** Built in Build A, B and C, with review fixes C-1..C-7 (B-5 not done). Check at the last fix: `check.sh quick 730e67f` all ok, 961 tests. Measured with S11a2 at `e149258` (item 17). Spec `docs/slices/S11a1.md`, log `S11a1-log.md`, targets `S11a1.toml`. Owner questions S11a1-1..10 above. Open: T131 stays a report (S11a1-10); preparation fire never runs in matches (T138). Waits for the owner's merge decision.
17. **S11a2: done on branch `claude/project-thread-e7nsak` (2026-11-17), merged to main 2026-11-19 with the owner's OK.** Built in Build A and B, with the Fix of Build B (B-1..B-7). Verified: `check.sh quick 730e67f` all ok, 982 tests at the head `e149258`. GitHub CI on `affb26e` (S11a1 complete plus S11a2 stage A): Linux build, release and debug sim tests, simulate, clippy, rustfmt and cargo-deny passed; Windows and THIRD_PARTY_NOTICES fail as on main (fixed by PR 1); macOS cancelled; cross-platform hash check skipped. CI on `e149258` was still running when written. Measured 480 at `e149258`: Germany 41.67 % (sets 46.25 / 38.12 / 40.62; S10c-ai 40.00 %), no winner 0; 318 targets pass, 82 miss (44 hard). New hard misses: T9b, T54a, T54c, T81a, T81b, T119b; gone: T8a, T115d, T121d. `tools/verify.sh` not run. Spec `docs/slices/S11a2.md`, log `S11a2-log.md`, targets `S11a2.toml`. Owner questions S11a2-1..12 above. Next: the merge decisions for S10c-ai, S11a1, S11a2 and PR 1; then the open items in STATE.md; then S11b.
18. **S11b: done on branch `claude/project-thread-e7nsak` (2026-11-19), waits for the owner's merge OK.** Built in Build A, B and C, with the Fix stage (spec `docs/slices/S11b.md`, stages `S11b-build-A.md`, `-B.md`, `-C.md`, targets `S11b.toml`, log `S11b-log.md`). Measured 480 at `88117de`: Germany 44.17 %, HE share 26.8 / 26.5 / 26.2 (cap 26.3). Owner questions S11b-1..14 above.
19. **B2 balance pass: done on branch `claude/project-thread-e7nsak` (2026-11-19); NO default changed.** Plan `docs/slices/B2-balance.md` (item 14; stages `B2-run-1.md` and `B2-run-2.md`, targets `B2.toml`, levers `targets/B2-levers/`, log `B2-log.md`). No lever passed the plan's rule. `rg_gap25` was applied (`fc14f52`) and reverted (`63e4b04`) when judge rule J4 failed on T121b. Owner questions B2-1..12 above.
20. **S14 assault kit: built, reviewed and fixed on `claude/project-thread-aj1tww` (S14 Fix 2026-11-25); Docs/PR next.** Log `docs/slices/S14-log.md`. Owner questions S14-8..13 (above) from the Fix stage, each on (A). Plan `docs/slices/S14-assault-kit.md` (notes `S14-research-history.md`, `S14-research-code-audit.md`). Builds after S13 from a fresh main: S14a mechanics (Build A flame, B clear trench and data, C melee; all switches 0, neutral; the `flame_only` 480 gates the AI half), presenter P, then S14-ai (flame jobs, refuel, flame buy, AI clear trench; 480, ablations). SAVE +1 and PROTOCOL +1 (`ClearTrench`, `Strike`). Targets from T185. Owner questions S14-1..7 above, each on (A).
21. **S15 USSR (the third faction): planned 2026-11-26 on `claude/project-thread-aj1tww`; nothing built.** Plan `docs/slices/S15-ussr.md` (notes `S15-research-history.md`, `S15-research-code-audit.md`). About 1.3x S3. Stages: S15a Build A (ONE layout commit, SAVE +1, no PROTOCOL change; `[matchups]`, riders, plates, captured bombs, bayonets; all switches 0), Checker, Build B (infantry data), S15b Build C (5 guns, 7 vehicles), presenter P, S15-ai Build D (rider job, AI card table, switches on), proof (neutral 40 with the switches off and on, one Germany-USSR breakage 40; no 480, win rates reported only: owner rule 2026-11-25), Review, Fix, Docs. Targets from T210. Overlaps PR 11 (S16-S18: SAVE and PROTOCOL numbers, `match_setup.rs`, menus, lobby, `batch.sh`): whichever merges second renumbers. The card's win-rate band waits for a balance pass with three factions. Owner questions S15-1..13 above, each on (A).

## S10a open items (after the mechanics half; S10a-ai spec 3.5 says where each goes)

1. S17: the wire path layer marks nodes within band/2 + 2 m, so gaps under about 5.5 m do not show in path-finding, and men may cross standing wire beside a small gap. In S10a-ai Build A (step test).
2. Carried material lost with dead men, and truck material lost in a wreck, is counted nowhere (a counter needs a save layout change). In S10a-ai Build A (its SAVE 23 layout commit).
3. No Godot headless check of the truck Materials button (the mock has no truck). Deferred to the next presenter pass (the AI does not need it).
4. The breach row and T93 need AI or staging that suppresses the defenders first: S10a-ai (AI-driven `breach_ai` row, S10a-16).
5. tools/neutral.py: a 40-set-only run now fails against a four-set base folder; stage runs need a 40-only base folder or a set list option (S10b uses `target/s10b-base40`).
6. Owner decisions S10a-9 to S10a-11 and S10a-1..8 (all on (A)): see the owner questions above.
7. S10a-ai F8 (fixed on `s10b-ai` Build D behind `[ai.truck] refusal_memory` 1; all-off stays neutral): an S8b truck forgets an HQ goal it cannot reach after four later give-ups and orders it again about 5 minutes later.

## Known issues still open

- S8a (on main): T5c, T41b, T48b missed on S8a (S8a-29/30); T41b and T48b pass on the S8b build; tick p99 0.493 / 0.625 ms against S7's 0.38 / 0.56, not profiled.
- S8b (on main): T5c (Germany 41.9 % on 1-80; S8a-30), T50e (Germany -2.8 s per ride on 201-280; S8b-27). T33a/b closed by S8b-26. Net loopback refuses `DropCrates` at a non-truck.
- Tank wrecks by guns stay at 0.04-0.09 per match; armour out is the measure (owner 2026-11-09).
- S9 (on main): 14 hard target misses on the 160 sets (T8a/b HE share, T31 vehicle kill share, T40/T72 half-tracks bought, T54d/e/f tank floors, T55b M8 share); S9-19. Guns kill few tanks (S9-17). Tick clean run on the final commit 0.433 / 0.534, level with the 0.441 / 0.532 baseline (the Fix stage measured +10 % / +13 %), the Build C rise did not reproduce. Not verified: the eliminated-player path. S9b (on main) leaves 10 hard misses and T88 USA open (S9b-1, S9b-3 (A): carried to S10).

Balance issues from S1-S3c are superseded by later measurements (latest:
`docs/slices/S6-log.md` "Targets" and "Tune"). Still open:

- Map: base 1 is worth about 6 points of win share to either faction.
- Balance after S6b (160 sets, S6 prices): Germany 52.5 / 43.1 / 41.9 / 50.6 %,
  pooled 47.0 % (T5, band 45-55; S6b-1); base side 59.4 % oos (T6c, max 62).
- Mortars and HE: met after S6b on 160 sets (USA 11.0-12.0 %, band 10-15;
  HE 21.5-21.9 %, band 20-26.3). Germany's mortar share 13.6-14.5 % is near the 15 % cap.
- S7 balance after the tick fix (160 sets 1-80 / 101-180 / 201-280): Germany 48.1 / 44.4 / 48.1 %,
  pooled 46.9 % (T5d misses by 0.6 on 101-180); T32a misses on seeds 1-80 (Pz IV 18.9 %, S7-15). StuG and M10 take
  about 3 % and 2 % of kills; price is no lever (Tune). AI open-top throws 0 in play.
- Cook-offs (0.05 -> 0.19 per match) and crates lost (16 / 6 -> 54 / 21) rose
  under S6b; not diagnosed (S7 to look).
- T4a / T4b: `no_bombs` refusals 0 since S6b (dry orders withdrawn).
- Structures are not selectable in the HUD (S6-18); S6 screenshot shows old labels.
- Tank crate runs and cook-offs are rare in AI play (0 tank runs per match).
- fortify-test mortar row 39.5 % against a 40 % limit (marginal).
- AI rarely throws satchels, rarely fires Panzerfausts (carriers die waiting;
  AI tanks stand off).
- No cross-OS CI (GitHub Actions blocked by billing). MPL-2.0 text and
  `socket2` missing from `THIRD_PARTY_NOTICES`.
- `frontline-relay` not built or run on Linux yet.
- After a host crash the guest waits about 75 s; the waiting overlay has no
  "leave now / claim the win" button; possible IPv6 relay reply issue.
- `vehicle_view.gd` draws at most 48 vehicles per type (Battalion blocker).
- Sim step spikes in windowed Godot runs (13-44 ms max) that headless runs
  never show; probably efficiency-core descheduling; not investigated.
- At 1280x800 the roster shows a limited number of cards plus a "+N" chip.
- HUD readability: capture-chip fill covers the letter; trench lines hard
  to see among craters; frag craters and throw streaks small at RTS height.
- Autoplay: frag-into-trench shot fails most runs; shots of two instances
  share file names.
- `bench/results` S1 hashes stale since S1b.
- `docs/slices/S6-hq-crates.md` is about 312 lines (limit 300).
