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
  S7-6 3-inch M62 penetration row derived from the US 30° table x 1.066 / cos 30° until a Bird & Livingston row is read. (A) Use it, marked verify.
  S7-7 Small arms do not aim at open-top crews (stray rounds through an open face still hit seats). (A) Yes; aimed fire with 6.4 (S8). S8a: settled (reach table; a crewman's head over the rim is a target, S8a-17/18).
  S7-8 Fragments never pierce plates in S7 (all plates here >= 10 mm). (A) Yes; a thin-plate rule with 6.4. S8a: settled (`frag_pen_mm` 12 / 9 / 3, S8a-13).
  S7-9 Armour zone id = box index x 4 + face group, name `<box>_<face>` (`casemate_front`); the two tanks keep ids 0-7 and names. (A) Yes.
  S7-10 StuG gun traverse 6 deg/s and casemate/M10 elevation 4 deg/s have no source. (A) Starting values, marked verify.
  S7-11 T34e (StuG fires in >= 80 % of armour-test case 18) cannot hold as the case is set: the Sherman starts laid on the StuG at 200 m, hits at 1.3 s (median) and 9 of 10 first hits penetrate the 30 mm side and disable it; the StuG's pivot and lay take 2.5 s (its first hit in the one run it survived). (A) Keep the scenario as vehicles.md 10 states it, make T34e a report; the pivot-to-lay is proven by `crates/sim/tests/casemate.rs` (fires in under 8 s on a target that does not shoot back).
  S7-12 T36a (an 81 mm ground burst 3-6 m from an M10 hits its crew in >= 20 %) does not hold physically: the crew boxes rise only 14-24 cm over the 66 cm rim and ground-burst fragments climb at 2-50°, so the hull edge and turret wall shadow almost every line; armour-test case 20 measures 2 of 60 (3.3 %), a Sherman 0. Bursts on the deck behind the turret hit the crew in about half (sim test). (A) Keep the physics and the Build A seat boxes; T36a becomes a report until the owner wants standing crew (head and shoulders ~40 cm over the rim) or airbursts.
  S7-13 Thrown items now collide with enemy vehicles (S7-5: a plate drops them to the ground below), so S6b matches with tanks differ slightly even before the new cards. (A) Accept; the Build B log measures it with the rest.
  S7-14 With the second vehicle card a faction has 11 call-in tiles; two columns rose above 57.5 % of the screen, so the call-in panel takes up to three columns. (A) Keep three columns; a compact tile row is presenter polish for later.
  S7-15 With overwatch_back_m 35 (S7 Fix, review M2) the Pz IV takes 18.8 % of German kills on the in-sample 160 (seeds 1-80), 0.8 over T32a's 18 % (17.4 and 16.9 % on seeds 101-180 and 201-280; 16.1 % before the fix). 50 m keeps it at 17.4 % but halves the StuG/M10 KOs and gives Germany 45.0 % pooled. (A) Keep 35 m; the Tune of the next vehicle slice rechecks T32 with td_hunt_m (450 m untested). S8a: T32 holds (Germany top type 17.9 %, cap 18); the out-of-sample S7-15 report reads 20.0 %, still open.
- S8a-1..31 (`docs/slices/S8a-halftracks.md` section 14; 16-19 Build A, 20-27 Build B, 28-31 Fix), each on its (A):
  S8a-1 Main gun optional (half-tracks have none). (A) Yes.
  S8a-2 M3A1 capacity 11; squads never split, a squad that does not fit is refused (US 12-man squad rides after a loss). (A) Yes. (B) capacity 12.
  S8a-3 MG pen tables use the belt's AP round for every round (no belt-mix roll). (A) Yes; `ap_share_pct` later if MGs run high.
  S8a-4 Face-hardened plate x 125 % against every round (`[vehicles] face_hardened_pct`). (A) Yes, verify.
  S8a-5 Transports skip the vehicle limit of 4, unlock at 2:00, cap 2 per card. (A) Yes.
  S8a-6 Half-tracks steer like tanks (neutral pivot) until S8b's wheeled model. (A) Yes.
  S8a-7 Hotkeys Embark V, Disembark U, Shift+U over the sides. (A) Yes.
  S8a-8 `Disembark { vehicles, over_sides }`; door is the default. (A) Yes.
  S8a-9 Riders are targets only while their slot box is above the rim and the vehicle is seen. (A) Yes.
  S8a-10 251 commander fires the front MG; no 251 rear AA MG, no M3A1 .30s; StuG roof MG and M10 .50 (S7-1) stay out. (A) Yes.
  S8a-11 Cards 290 / 310 MP, pop 3, 60 s, cap 2, unlock 2:00. (A) Yes; Tune +-15 %.
  S8a-12 The sim emits every small-arms penetration event; the feed coalesces them per vehicle per 2 s. (A) Yes.
  S8a-13 Thin-plate fragment values `frag_pen_mm`: 75 mm 12, mortars 9, grenades 3. (A) Starting values, verify.
  S8a-14 Cross-country speeds 750 / 850 cm/s (road 52 / 72 km/h). (A) Starting values, verify.
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
  S8a-27 (Build B) Passengers are drawn crouched at the bottom of their slot (no seated pose yet); the vehicle card lists them in place of the main-gun ammunition row; the components row still shows the gun, ring and rack a half-track lacks. (A) Accept for S8a; presenter polish later.
  S8a-28 (Fix) Ride value, measured as the review asked (Embark order to the squad's lead inside the capture radius of the point it rode for, against a straight walk from where it stood at the order; `AiRideMeas`): AI rides are slower than walking, Germany -29 / -26 / -21 s and USA -14 / -15 / -6 s per ride on seeds 1-80 / 101-180 / 201-280 (about half the measured rides never reach their point: retasked or killed). There is no walk-to-the-front exploit; S8a-25's figure was an upper bound. (A) Accept for S8a; make the ride pay in S8b (board where the squad stands, a shorter `dismount_sight_m`, shorter boarding waits) with a Tune.
  S8a-29 (Fix) After the M1 picking fix (the point at least `min_ride_m` + the walked last stretch from the squad and `min_ride_m` beyond the door) USA transported per match is 0.98 / 0.89 / 1.04 and trips per bought 0.86 / 0.78 / 0.87 (T41b, T48b floors 1). (A) Accept the misses for S8a (the old numbers counted short hops); retune with S8a-28.
  S8a-30 (Fix) Germany wins 42.5 % on seeds 1-80 (T5c floor 45), 45.6 % on 101-180, 48.1 % on 201-280: pooled 45.4 % over 480, inside the 45-55 % band. (A) Accept T5c; the pooled band holds.
  S8a-31 (Fix) The AI keeps unloading over the sides under fire: armour case 24 over 80 runs hits 3.02 men by the door and 3.36 over the sides (a gap of about one standard error), while the sides are 2.3 s faster. (A) Keep; recheck with the S8b ride work.

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
   Open: S8a-28 to S8a-31. Next: S8b trucks, recon cars, wheeled mobility, recon optics, supply trucks.
7. Sim profiling: S7 tick p99 doubling found and fixed (vehicle re-path loop); a general profile pass is still open.

## Known issues still open

- S8a (branch `s8a-halftracks`): T5c, T41b, T48b miss (S8a-29/30); AI riding is slower
  than walking (S8a-28, make rides pay in S8b); tick p99 0.493 / 0.625 ms against S7's
  0.38 / 0.56, not profiled; "MG dry" on the 251 card while the sim fires (bridge or card);
  the gun, ring and rack bars still show on half-tracks (S8a-27).

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
