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
  S7-1 No StuG roof MG 34 or M10 .50 (pintle mounts out of scope). (A) None; add with pintle mounts in S8.
  S7-2 StuG and M10 buy through the existing `vehicle` mix slot, split by card `ai_weight` (100 each), not new `[ai.mix]` slots (those would bypass the vehicle unlock, limit and savings). (A) Yes.
  S7-3 StuG 500 MP / pop 10 / 120 s; M10 520 / 10 / 120 s (vehicles.md 8.4 said 520/500 and 90 s). (A) As stated; the Tune may move each within +-15 %.
  S7-4 The M10's assistant driver takes the `radio_mg` seat role (no hull MG), labelled by seat id. (A) Yes; a sixth role when a vehicle needs both.
  S7-5 A thrown item that strikes a plate drops to the ground below it (no deck charges, no grenade screens). (A) Yes; deck satchels are a later decision.
  S7-6 3-inch M62 penetration row derived from the US 30° table x 1.066 / cos 30° until a Bird & Livingston row is read. (A) Use it, marked verify.
  S7-7 Small arms do not aim at open-top crews (stray rounds through an open face still hit seats). (A) Yes; aimed fire with 6.4 (S8).
  S7-8 Fragments never pierce plates in S7 (all plates here >= 10 mm). (A) Yes; a thin-plate rule with 6.4.
  S7-9 Armour zone id = box index x 4 + face group, name `<box>_<face>` (`casemate_front`); the two tanks keep ids 0-7 and names. (A) Yes.
  S7-10 StuG gun traverse 6 deg/s and casemate/M10 elevation 4 deg/s have no source. (A) Starting values, marked verify.
  S7-11 T34e (StuG fires in >= 80 % of armour-test case 18) cannot hold as the case is set: the Sherman starts laid on the StuG at 200 m, hits at 1.3 s (median) and 9 of 10 first hits penetrate the 30 mm side and disable it; the StuG's pivot and lay take 2.5 s (its first hit in the one run it survived). (A) Keep the scenario as vehicles.md 10 states it, make T34e a report; the pivot-to-lay is proven by `crates/sim/tests/casemate.rs` (fires in under 8 s on a target that does not shoot back).
  S7-12 T36a (an 81 mm ground burst 3-6 m from an M10 hits its crew in >= 20 %) does not hold physically: the crew boxes rise only 14-24 cm over the 66 cm rim and ground-burst fragments climb at 2-50°, so the hull edge and turret wall shadow almost every line; armour-test case 20 measures 2 of 60 (3.3 %), a Sherman 0. Bursts on the deck behind the turret hit the crew in about half (sim test). (A) Keep the physics and the Build A seat boxes; T36a becomes a report until the owner wants standing crew (head and shoulders ~40 cm over the rim) or airbursts.
  S7-13 Thrown items now collide with enemy vehicles (S7-5: a plate drops them to the ground below), so S6b matches with tanks differ slightly even before the new cards. (A) Accept; the Build B log measures it with the rest.
  S7-14 With the second vehicle card a faction has 11 call-in tiles; two columns rose above 57.5 % of the screen, so the call-in panel takes up to three columns. (A) Keep three columns; a compact tile row is presenter polish for later.
  S7-15 With overwatch_back_m 35 (S7 Fix, review M2) the Pz IV takes 18.8 % of German kills on the in-sample 160 (seeds 1-80), 0.8 over T32a's 18 % (17.4 and 16.9 % on seeds 101-180 and 201-280; 16.1 % before the fix). 50 m keeps it at 17.4 % but halves the StuG/M10 KOs and gives Germany 45.0 % pooled. (A) Keep 35 m; the Tune of the next vehicle slice rechecks T32 with td_hunt_m (450 m untested).

## Planned work queue

1. Infra step (done on branch `infra-checks`, to be pushed).
2. Autotune fix (done, merged by the infra step).
3. S6 HQ and ammo crates: built, reviewed and fixed on branch `s6-hq-crates`
   (head `d355350`, not pushed). Open: retake the HUD screenshot, S6-18
   structure selection (its own item when scheduled).
4. S6b mortar AI pass: done on branch `s6b-mortar-ai` (not pushed). Mortar and
   HE bands met on 160 sets; T5 open (S6b-1).
5. S7 assault guns and TDs: done and pushed on branch `s7-casemate-td` (from `9f216e9`, tick fix after the log); log `docs/slices/S7-log.md`, spec `S7-casemate-td.md`, targets `S7.toml`. Open: S7-11, S7-12, S7-15.
6. S8 half-tracks, trucks, recon: next (roadmap in `docs/STATE.md`).
7. Sim profiling: S7 tick p99 doubling found and fixed (vehicle re-path loop); a general profile pass is still open.

## Known issues still open

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
