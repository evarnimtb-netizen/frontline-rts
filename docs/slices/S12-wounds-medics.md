# S12: wounds and medics (plan)

Status: **plan for owner review**, written 2026-11-19 from `main` at `8d2731b` (S10c-ai, S11a1, S11a2 merged); revised the same day after an
independent review (18 findings, all folded in; section 13). Nothing here is built. S11b (howitzers, light mortars, rifle grenades) and the
balance pass B2 run first on `claude/project-thread-e7nsak`; `<BASE>` is the last commit of B2 when S12a Build A starts. Design sources:
`docs/design/roster/infantry-structures.md` 5 and 7.3-7.4 (cited "IS 5.2"), `roster-expansion.md` S12 and questions 10, 14 (cited "RE").
Pattern: `S11a2.md`. Owner rules as every slice since S10: features first, no Tune; Germany pooled 480 in 35-65 %; tick p99 must not regress;
all tuning in TOML; damage only from projectile or fragment contact; every behaviour behind a switch; all S12 switches off = `<BASE>`.

## 0. Why this shape, and what was weighed
- **Size.** RE sized S12 at ~0.7x S3. Read against the code it is nearer **1.1x**: a new soldier state on every damage path, four commands, a
  structure kind, a refund, replacements, AI jobs and a presenter pass. So it follows the S10 owner rule (2026-11-09) and splits: **S12a
  mechanics** (switches 0 at the end, proven neutral, one report 480 with wounds on that gates S12b) and **S12b AI and balance**.
- **Downed men are a new entity kind, not a flag.** `EntityKind::Downed` (6; the enum ends at 5). Most of the ~86 `kind == Soldier` sites
  outside tests (targeting, capture, `soldiers()`, formation, orders) then skip a downed man with no edit, as corpses did in S1. Sites that DO
  need an edit, listed for Build A: the hit paths (`fly`, `burst`), `population`, `spotting.rs:188-190` (`seen_by` refresh, else a stale
  marker for 3 min), the exhaustive matches at `combat.rs:3692`, `snapshot.rs:855`, `ai/guns.rs:1122`, and `rules.rs:204-228` (W14). A flag
  would need all 86 sites audited now and every future one.
- **Hits on downed men use a second spatial hash** (`downed_spatial`, rebuilt where the soldier hash is), read by `fly` and `burst` only.
- **Wounded men stay in their squad.** Only Downed men leave it. A Wounded man goes to a post by being detached (the existing soldier
  orders) and rejoins by `Regroup`, so no return or squad-of-one machinery is needed.
- **Medics act on their own** near them, for players and the AI alike; orders do the rest. That keeps the AI half small.
- **Kept / dropped.** Kept: hit zones, fit / wounded / down / dead, bleed-out, bandage, medics, a staffed post, treat once, evacuation refund,
  replacements from base (IS 5.2, 7.3, 7.4; RE loop guards). Later: the aid dugout (roof like `hq_dugout`, S12c), stretcher cards,
  ambulances (Sd.Kfz. 251/8), Soviet medics (S15), morale, any healing over time (never: RE Q10).

## 1. Scope
**S12a (engine).** Build A: layout, zones, wound states, bleed, bandage, bleed-out, credit, pop, all switches 0. Build B: `medic` trait,
stabilise, carry, aid post (`kind = "aid"`), treat, evacuate, refund, `Reinforce`, commands, snapshot, cards at `ai_weight` 0. Neutral proof;
then the `wounds_only` 480 report, which **gates** S12b (section 9).
**Presenter P (Sonnet, after Build B).** Downed pose and a wound pip per man, medic cards, the post in the Build menu, Reinforce button, Tend,
Carry, Evacuate on the soldier menu. No red cross (RE Q14 (A)). It ships behind `enabled`, so players see it only when S12b turns wounds on
(S12-14).
**S12b (AI and balance).** AI medics, post, carrying, staffing, reinforcing; switches on; JSON, rows, 480, ablations, Review, Fix, verify, Docs.
**Not in S12:** crew in vehicle seats keep the S3 seat `wounded_hp` model; a crewman who bails out is a soldier and gets wound states.

## 2. Switches and keys (Build A's layout commit adds all; stage in brackets turns a switch on)
| Table | Key | Default | Meaning; source |
|---|---|---|---|
| `[wounds]` | `enabled` | 0 in S12a, 1 in [S12b] | W1-W9 master switch; 0 = today's 0 HP death |
| | `head_top_pct` / `legs_bottom_pct` | 12 / 45 | W1; IS 5.3 |
| | `zone_damage_pct` | head **100**, torso 100, legs 60 | W2. IS 5.3 asked head 300: that kills every man on a fire step in one hit (risk 1, S12-3) |
| | `wounded_hp` | 50 | W3; IS 5.3, same as crew `wounded_hp` |
| | `overkill_kill` / `head_kill_dmg` | 60 / 0 (off) | W3; IS 5.3 `head_kill_dmg` 100 parked with the head factor (S12-3) |
| | `bleed_ms_per_hp` | 4000 | W4; IS 5.3 |
| | `down_bleed_s` | [60, 180] | W5; IS 5.3 (seeded per down) |
| | `wounded_spread_pct` / `wounded_reload_pct` | 130 / 125 | W4; IS 5.2 |
| | `wounded_speed_pct` / `legs_speed_pct` | 85 / 60 | W4; IS 5.2 said 70 / 40 (S12-10) |
| | `bandage_s` / `stabilise_s` | 10 / 15 | W4, W6; IS 5.3 |
| | `drag_speed_pct` / `stretcher_speed_pct` | 30 / 60 | W7; IS 5.3 |
| | `treat_s` / `return_hp_pct` / `evac_s` / `evac_refund_pct` | 60 / 75 / 30 / 40 | W8, W9; IS 5.3 |
| | `auto_tend_m` | 25 | W6 |
| | `reinforce` / `reinforce_cost_pct` | 0 in S12a, 1 in [S12b] / 100 | W11 |
| `[supply]` | `aid_max` | 2 | W8: posts per player (beside `hq_max`, `dump_max`; RE loop guard) |
| `[ai.medic]` | `buy` / `field` / `post` / `reinforce` | 0 | S12b A1-A5 switches |
| | `max_teams` / `min_inf_squads` | 2 / 4 | A1 |
| | `front_m` / `enemy_clear_m` | 60 / 40 | A3; IS 7.3 |
| | `reinforce_below_pct` | 60 | A5 |
Aid post supply def (`kind = "aid"`): `cost` 40, `pit` `aid_pit` (new shallow profile, ~30 s), `radius_m` 12, `beds` 2, `staff_min` 2,
`built_by = ["medic"]`, one `tent` box (hp 60, no stock, no cook-off). IS 7.3 tent 20 kg not charged (S6-4 precedent). `StockKind::Aid`: the
36 `Hq` match sites in `structures.rs` are audited in Build B. Ranges: switches 0..=1; seconds 1..=3600; percents 1..=400.
Ablations (`targets/S12-ablations/`): `s12_off` (all 0: equals `<BASE>`), `wounds_only` (`enabled` 1, rest 0), `no_medics`, `no_post`,
`no_reinforce`, `head_300` (the IS 5.3 head factor, a report).

## 3. Data model (all saved fields in Build A's ONE layout commit; B and S12b change no layout)
- **`SAVE_VERSION` +1** over `<BASE>` (S11b may take 29; the log names it). **`PROTOCOL_VERSION` +1**: `Tend { units, target }`, `Carry {
  units, target, to }`, `Evacuate { units }`, `Reinforce { squad }`; relay doc in Build B.
- `EntityKind::Downed = 6`: keeps `transform`, `owner`, `combat` (prone capsule), `kit` emptied (the crew weapon to the squad's `drops` as at
  death); `squad` None. In `downed_spatial`, not in `soldiers()`. `population` adds own `Downed`.
- `Combat` += `wound: Wound { state: u8 (0 Fit, 1 Wounded, 2 Downed), bandaged: bool, legs: bool (a leg wound: `legs_speed_pct`),
  treated: bool, paid: bool (W9), bleed_acc_ms: u16, bandaging: u16 (ticks left), by: (u8, u16) (last hitter's player and kill class) }`.
  Per entity, hashed with its chunk. `paid` is set at spawn from the card (false for start squads and crew cards, true for bought and
  reinforced men).
- `MatchState::casualties: BTreeMap<SimId, Casualty>` (Downed men only; at most 512): `Casualty { from_squad, card: u16, bleed_out_at: u64
  (0 = stabilised), tend: Option<(SimId, u64)>, carriers: [Option<SimId>; 2], dest: Option<FxVec2>, evac_at: u64 (0 none) }`.
- Aid posts are `Structure`s with `beds: Vec<Bed { man: SimId, done_at: u64, evac: bool }>` (at most `beds`).
- `Fallen` widens to `(id, player, class, Hit { raw: i32, zone: u8, blast: bool })`; `hurt`, `damage.rs:869` and `guns.rs:531` stop using
  `saturating_sub` for the outcome.
- `PlayerState::s12: S12Counters`: `downed`, `killed_outright`, `bled_out`, `downed_killed`, `stabilised`, `bandaged`, `carried`,
  `treated`, `evacuated`, `refund_mp`, `reinforced_men`, `reinforce_mp`, `downed_by_class[kill_class::COUNT]`, `head_hits`, `torso_hits`,
  `leg_hits` (each split trench / open). `AiCounters` += `medic_buys`, `medic_tends`, `medic_carries`, `post_builds`, `reinforce_orders`,
  `medic_holds`.
- RNG purpose `WOUND_BLEED`, one draw per down. Zones use no RNG.
- Load validation `load_rejects_bad_s12_state` (one case per check) and `save_fuzz.rs` staging: a man mid-bandage, a downed man mid-tend, one
  carried, one in a bed, one evacuating in base.

## 4. Rules (integers and `Fx`; men and casualties by ascending id)
- **W1 Zone.** Standing or crouched: the hit point's height above the feet as a share of the capsule: top `head_top_pct` head, under
  `legs_bottom_pct` legs, else torso. **Prone men are torso** (the prone capsule lies along the heading and `seg_seg` gives no stable axis
  share; a head zone there would make pinned men die faster). Fragment rays use their own hit point. Blast overpressure: no zone.
- **W2 Damage.** `weapon.damage * zone_pct / 100`; blast as today.
- **W3 Outcome** (`raw = hp - dmg`, signed): `raw > wounded_hp` Fit; `0 < raw <= wounded_hp` Wounded; `raw <= 0`: **dead** if `raw <=
  -overkill_kill`, or blast overpressure, or head zone with `head_kill_dmg > 0` and `dmg >= head_kill_dmg`; else **Downed**. Per source:
  bullets and fragments (`hurt`), rider core hits (`damage.rs:869`) and gun crews (`guns.rs:531`) all take this rule. Any later hit on a
  Downed man kills him, in the same tick too (hits resolve in event order). Today's numbers (HP 100): Kar98k 70 torso -> 30 wounded, a second
  downs (raw -40); MG42 45 wounds on the 2nd hit, downs on the 3rd (-35); M2HB 100 downs (0); legs take 3-4 rifle rounds.
- **W4 Wounded.** Penalties; the squad walks at its slowest wounded man (a new input beside the S6 crate-carrier pace, `squad.rs:370`).
  Bleeds 1 HP per `bleed_ms_per_hp` until bandaged; at 0 HP he is Downed (credit `wound.by`). Self-bandage starts on its own when his
  suppression is under `crouch_suppression`, **walking included** (a field dressing; firing or a new hit breaks it), `bandage_s`. Bandaged
  men keep the penalties until treated.
- **W5 Downed.** Leaves his squad as `kill` does (slots, focus, crew weapon to `drops`, crates dropped); riders and gun crew are set down at
  the vehicle's S8a exit point. `bleed_out_at` = now + seeded draw; at that tick he dies through a shared `credit_kill` (factored out of the
  kill loop at `combat.rs:3462-3500`: kills, `kills_by`, vehicle and gun kills, mines, own fire when `by.0 == owner`). No fire, capture,
  spotting or squad strength. Counts toward pop until dead or evacuated (S12-6).
- **W6 Stabilise.** A `medic` man within 2 m, not moving, not pinned, for `stabilise_s`. Auto: a medic with no move order tends the nearest
  own downed man within `auto_tend_m` when his suppression is under `crouch_suppression`; `Tend` sends him to one. A non-medic's `Tend`
  bandages a Wounded man (buddy, `bandage_s`); he cannot stabilise.
- **W7 Carry.** One man drags at `drag_speed_pct`; two medics (stretcher) at `stretcher_speed_pct`; two non-medics drag. Carriers do not fire;
  their squad paces to them. Carrier hit or re-ordered: casualty set down there. Unstabilised men keep bleeding.
- **W8 Aid post.** Built like the HQ. Works while `staff_min` own `medic` men stand in its radius with no move order. Beds fill by ascending
  id: a **stabilised Downed man** in the radius is evacuated (automatic); a **detached Wounded man** in the radius not yet `treated` is treated
  in `treat_s` and returns Fit at `return_hp_pct`, `treated = true`, and `Regroup`s; a Wounded man already `treated` is evacuated **only by
  `Evacuate`**. A post that loses its staff pauses its beds. HE that breaks the tent destroys the post (S6-14); men in beds stay.
- **W9 Evacuation.** At a staffed post, or in the own base (a stabilised downed man carried inside, or `Evacuate` on a wounded man there;
  `evac_s`, no worker). The man is removed (no corpse), pop freed, counted a loss. Refund `card cost / card size * evac_refund_pct / 100`
  only when `paid`, once per man. Refund 40 < reinforce price 100: no gain loop; start-squad and crew men pay nothing back.
- **W11 Reinforce.** `Reinforce { squad }` on an infantry squad below its card size (members plus its own living casualties): one man per gap,
  per-man price, spawned at base with the card's **basic member kit** (the first `[[members]]` entry with no crew weapon: no duplicate MG or
  launcher), walks to the squad and joins. Starts the card's cooldown (S12-12). Refused at pop cap. Not for vehicle or gun crews.
- **W12 Targeting.** Nobody aims at a Downed man (auto or `AttackUnit`); stray rounds and fragments hit him. This drops IS 5.2's "human
  players can" on purpose (S12-4).
- **W13 Credit.** A down is a casualty inflicted (`downed_by_class`); the kill counts at death as today. An evacuated man is a loss, not a kill.
- **W14 Out of the match.** Downed men do not keep a player in: a player whose living men are all Downed is wiped as today (`rules.rs`). Test.

## 5. AI (S12b; `ai/medic.rs`, plans without RNG, ascending ids)
- **A1 Buy.** One medic team when `min_inf_squads` infantry squads live and 3+ own downs in the last 120 s; a second when the first staffs a
  post; at most `max_teams`; infantry budget.
- **A2 Post.** One at the own HQ (in its radius, the cell nearest base) or, without one, at the base edge toward the assault target.
- **A3 Field.** A team not staffing follows `front_m` behind the assault group's lead squad; goes to a downed own man only when no spotted enemy
  is within `enemy_clear_m` of him; stabilises; carries him to the post or base when no other unstabilised man is within 30 m. Else holds.
- **A4 Staffing.** With own men waiting in a post's radius, the nearest team staffs it until the beds empty. A retreating squad (S6-9) detaches
  its unbandaged or untreated Wounded to the post on the way.
- **A5 Reinforce.** A squad under `reinforce_below_pct` that is in base or retreated, before a new card of its kind, budget permitting.
- **Liveness** (as S10b-ai L1-L5): a job ends when its casualty dies, is evacuated or is taken by another team; 120 s without progress drops a
  job; no team waits on an empty post. Each a named test.

## 6. What else can be true when a job ends (each a named test; stage in brackets)
| Job | Casualty gone | Another man on it | Worker dead or re-ordered | Enemy arrives | Save/load mid-job |
|---|---|---|---|---|---|
| Bandage | - | self and buddy: one finishes, no double count [A] | hit again: restarts [A] | suppression breaks it [A] | [A] |
| Stabilise | killed mid-tend [B] | two medics: lower id tends [B] | medic dies: casualty keeps bleeding [B] | pinned: pauses [B] | [B] |
| Carry | dies in transit [B] | a second pair: refused [B] | carrier hit: set down [B] | - | [B] |
| Bed | killed by HE in bed [B] | two posts: lower id [B] | staff short: pause [B] | tent broken: post gone [B] | [B] |
| Reinforce | squad dies on the way: man joins nearest own squad of the card with room, else walks to base and is refunded in full [B] | two men, one gap: second refunded [B] | - | - | [B] |

## 7. Determinism, cost, guard rails
One RNG purpose, one draw per down. Rules run in the Damage and Status stages in id order. Cost: one extra hash query per projectile step
and burst; timers once a second (`(tick + id) % hz`). Tick p99 from `tools/verify.sh` only, at most 25 % over `<BASE>`. Hashes equal at 1, 2,
3, 8 threads and through save/load with men bandaging, downed, carried, in beds.

## 8. Measures (`s12_derived` per faction; `targets/S12.toml`)
`downed_per_match`, `outright_kill_pct`, `bled_out_pct`, `stabilised_pct`, `downed_killed_pct`, `treated_per_match`, `evac_per_match`,
`refund_mp_per_match`, `reinforced_men_per_match`, `head_hit_pct` (trench / open), `pop_downed_peak`, `medic_matches_pct` (RE "6 of 10"),
`he_share_casualties_pct` (HE over kills plus downs, for T8: S12-15), `length_min`, `losses_per_match`. Rows with no design figure are reports.

## 9. Done when
**S12a** (switches 0): `s12_off` 40 set equals `<BASE>` 40/40 (`tools/neutral.py`); named tests pass; SAVE and PROTOCOL bumped; save fuzz 0
panics, 0 unloadable. Rows (new `wounds` fortify scene; switches on in the row only): **R7 (hard)** men in a trench under rifles lose no more
than `<BASE>` + 10 %, also under `head_300` as a report; R1 a 10-man squad under two MGs at 300 m ends with downed men, outright share 20-45 %
(report; WW2 small-arms killed-to-wounded about 1:3); R2 with a medic team the bled-out share is 30+ points under R1; R3 a staffed post
returns a wounded man at 75 HP, and stops when its medics die; R4 a treated man wounded again is never treated twice; R5 refund once, only for
`paid` men; R6 Reinforce fills a squad from base with rifle kit; R8 prone men under MG fire die no faster than `<BASE>`.
**Gate to S12b:** the `wounds_only` 480 (AI blind to medics) has Germany in 35-65 % and match length inside the owner rule (median 15-30, none
under 10 or over 40). Otherwise stop for the owner before Build C.
**S12b**: Germany pooled 35-65 %; no match without a winner; length as above (RE "12-25 min" a report); `medic_matches_pct` >= 60; T8 on
`he_share_casualties_pct`; no new hard miss beyond the B2 list other than S12 rows; paired `s12_off` vs default on the same 480 seeds: a
pooled move of 5 points or more stops the slice for the owner.

## 10. Build stages (a chunk is about 30 self-counted turns)
1. **S12a Build A** (Opus, 2 chunks): layout commit; the Downed site list of section 0; W1-W5, W12-W14 with `enabled` 0; R7, R8; `s12_off`.
2. **S12a Build B** (Opus, 2 chunks): `medic` trait (bit 64, into `member_trait::ALL` and `NAMES`), W6-W11, commands and relay doc, post def
   and `aid_pit`, cards Sanitätstrupp (2 men, P 38, 80 MP less the German cut, 30 s) and Aid Men (2 unarmed, 80 MP, 30 s) at `ai_weight` 0,
   snapshot fields, rows R1-R6, then the `wounds_only` 480 (gate).
3. **Checker after A and after B** (Sonnet, a fresh agent each): diff against this file, named tests, neutral check; lists misses.
4. **Presenter P** (Sonnet, 1 chunk): section 1 list; headless mock-match smoke test.
5. **S12b Build C** (Opus, 2 chunks): `ai/medic.rs` A1-A5, liveness, JSON `s12`, `S12.toml`, switches on, 40 sets, ablations.
6. Orchestrator: `tools/measure.sh` 480 + `ablate.sh`; Review (Opus; gate fires: sim, save, protocol); Fix (Opus); `tools/verify.sh`; long runs
   (480s, full suites) where the project's current long-run rule says (2026-11-19: cloud in the evening, GitHub Actions blocked by billing);
   Docs (Haiku). Fable consulted where a stage stalls.

## 11. Risks
1. **Head hits in trenches.** Shooters aim 150 mm under the top of a man on a fire step (`head_aim_mm`), inside the 210 mm head band, so nearly
   every hit on him is a head hit. At IS 5.3's 300 % a Kar98k (210) or MG42 (135) kills in one hit against 2 and 3 today: against owner rule
   S4-43. Hence head 100 % to start, R7 hard, `head_300` as a report; a helmet rule is S12-3 (C).
2. **Lethality and match length.** Most deaths become downs; without medics most still die 1-3 min later. The S12b gate reads `length_min`;
   first lever `overkill_kill` 60 -> 40.
3. **Pop lock.** Downed men hold pop up to 3 min; `pop_downed_peak`; levers `down_bleed_s` [45, 120] or S12-6 (B).
4. **Faction asymmetry.** US squads 12 vs 10, US aid men unarmed. Germany at about 41-42 %; paired reading per faction.
5. **S11b and B2 move under us.** More HE means more blast deaths and fragment downs. Build A re-reads `kills_by_source_pct` at `<BASE>`;
   tests name only weapons on main today (Kar98k, MG42, M1 Garand, M2HB, 8 cm GrW 34); if B2 changes damage, W3's numbers are re-derived in
   the log.
6. **Every way to 0 HP** (`hurt`/`hurt_by`, rider core hits, gun crews, mines, own fire) must carry zone and blast into W3: one test per source.
7. **T8 moves by accounting alone**: blast still kills outright, bullets now down; hence S12-15.

## 12. Open design questions (recommended answer first; how-it-plays ones go to the owner)
S12-1 **Down state** (RE Q10). (A) Fit, wounded, down, dead, bleed-out, medics, staffed post, refund; no healing over time. (B) Wounded only.
S12-2 **Red cross** (RE Q14). (A) White armband and stretcher, no emblem unless a legal check clears it. (B) The emblem.
S12-3 **Head hits.** (A) Head 100 % to start, R7 hard, the 300 % as a report. (B) IS 5.3's 300 % and `head_kill_dmg` 100 (trenches get
deadlier). (C) 300 % plus a helmet deflect chance (new RNG, new key).
S12-4 **Firing at downed men.** (A) Nobody aims at them; stray rounds and fragments still kill them (changes IS 5.2, which let players). (B)
Players may.
S12-5 **Medics act without orders** within 25 m, for players too. (A) Yes. (B) Only on order.
S12-6 **Downed men in pop.** (A) Count until dead or evacuated (RE). (B) Leave pop when downed.
S12-7 **Second wound.** (A) Treated once; evacuated after. (B) Treat twice.
S12-8 **Replacements in S12.** (A) `Reinforce` in Build B, spawn at base and walk, 100 % per-man price, rifle kit. (B) Later.
S12-9 **Post staff.** (A) 2 medics (IS 7.3). (B) 1, the team splits.
S12-10 **Wounded pace.** (A) Torso 85 %, legs 60 %, the squad paces to him. (B) IS 5.2's 70 / 40 %.
S12-11 **Sanitätstrupp price.** (A) 80 MP with the German cut (S5a-37). (B) Same both sides.
S12-12 **Reinforce cooldown.** (A) Starts the card's cooldown (keeps Q21's per-card rule meaningful). (B) No cooldown.
S12-13 **Refund for free men.** (A) None for start squads and crew cards. (B) All men refund.
S12-14 **Wounds for players after S12a.** (A) Off until S12b merges (one change at a time; S12a ships dark). (B) On after S12a.
S12-15 **T8 (HE share, hard 26.3 %).** (A) Measured over kills plus downs from S12b on. (B) Kills only, accept the shift.
Recommendation on all: (A).

## 13. Review record
Independent review (Fable, 2026-11-19, read-only) confirmed the code facts (kind 6 free, `Combat` has no wound field, one `kill` path, trait bit
64 free, SAVE 28 / PROTOCOL 14, W3 numbers) and raised 18 findings; this revision answers each: layout bits `bandaged`/`legs`/`by`/`paid` (1,
10); Wounded men stay in squads, no return machinery (2, 12); head 100 % and R7 hard (3); prone = torso and R8 (4); per-source W3 (5, 6);
`Fallen` widened, any-hit-kills for Downed (6); Downed site list (7); `credit_kill` (8); T8 over casualties (9); Reinforce kit and cooldown
(11); Wounded evacuation only by order (13); bandage while walking (14); W14 (15); the S12b gate and S12-14 (16); `aid_max` and `StockKind`
audit (17); small fixes (18).
