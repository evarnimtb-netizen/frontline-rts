# S12a Build B: medics, stabilise, carry, aid post, treat, evacuate, refund, Reinforce, commands, cards (switches still 0)

Read first: `S12-wounds-medics.md` sections 2, 3, 4 (W6-W11), 6, 7, 9; `S12-log.md` "For the next stage" block (Build A's code
map, refused commands, deviations). **2 chunks**, about 30 self-counted turns each. `<BASE>` is `1209c99`. No layout change in this
stage: every saved field exists since Build A's layout commit; if one is truly missing, stop and say so in the log (a second layout
change needs the orchestrator). Every switch stays 0 (`[wounds] enabled`, `reinforce`, `[ai.medic]`), cards at `ai_weight` 0, so the
default game still plays exactly as `<BASE>`. Tests turn switches on in the test only.

Work rules: as Build A (`tools/run.sh` for anything over a few seconds, no command over 4 minutes, commit after every green step with
`git commit -m "S12a B: ..." -- FILES`, never `git add -A`, do not push, no model names in files). 40 sets only through `tools/batch.sh`
with `BATCH_JOBS=4`, **one in this stage** (step 8).

## Chunk 1: medics and the field
1. `medic` member trait (bit 64, into `member_trait::ALL` and `NAMES`; content validation). Cards: Sanitätstrupp (Germany; 2 men, P 38,
   80 MP less the German cut, 30 s cooldown) and Aid Men (USA; 2 unarmed men, 80 MP, 30 s), both `ai_weight` 0, as unit defs under
   `mods/ww2/defs/`. BASELINE files and snapshot refreshed for the new defs. Commit.
2. W6 stabilise (a medic within 2 m, not moving, not pinned, `stabilise_s`; auto-tend within `auto_tend_m` with no move order when
   suppression is under `crouch_suppression`), the buddy bandage of a non-medic's `Tend` on a Wounded man. `Tend` command accepted.
   Plan 6 row "Stabilise" and the buddy half of row "Bandage" as named tests. Commit.
3. W7 carry: `Carry { units, target, to }` (one man drags, two medics stretcher, two non-medics drag; carriers do not fire; squad paces
   to them; carrier hit or re-ordered sets the casualty down; unstabilised men keep bleeding). Plan 6 row "Carry" tests. Commit.

## Chunk 2: post, evacuation, reinforce
4. Aid post supply def `kind = "aid"` (plan 2: cost 40, pit `aid_pit` ~30 s, `radius_m` 12, `beds` 2, `staff_min` 2, `built_by`
   medic, one `tent` box hp 60, no stock, no cook-off), `[supply] aid_max` 2; the `StockKind::Aid` audit of every `Hq` match site in
   `structures.rs` (list the sites you changed in the log). W8 staffing, beds by ascending id, treat (`treat_s`, `return_hp_pct`,
   `treated`, `Regroup`), a treated man only by `Evacuate`; tent broken destroys the post. Plan 6 row "Bed" tests. Commit.
5. W9 evacuation at a staffed post or in the own base (`evac_s`), removal with no corpse, pop freed, refund only for `paid` men and
   once; `Evacuate { units }`. W11 `Reinforce { squad }` (switch `reinforce`): one man per gap, per-man price, basic member kit, walks
   and joins, starts the card cooldown, refused at pop cap and for crews; plan 6 row "Reinforce" tests. Commit.
6. Commands through the bridge (`crates/godot-bridge/src/intents.rs`) and the relay doc; snapshot fields for the presenter (wound
   state per man, downed men, casualties, posts and beds) in `crates/sim/src/snapshot.rs` and `godot/docs/match_state.md` "S12".
7. Rows R1-R6 in the `wounds` fortify scene (plan 9), added to `targets/S12.toml`; ablation files `no_medics`, `no_post`,
   `no_reinforce` in `targets/S12-ablations/`.

## Chunk 3: proof
8. The default 40 set into `target/s12-B-40/`; `python3 -I tools/neutral.py target/s12-base40 target/s12-B-40` shows no difference
   except zero-valued new counter keys. Save fuzz with a man mid-tend, one carried, one in a bed, one evacuating in base: 0 panics,
   0 unloadable; thread-count hash test with switches on. `tools/check.sh quick 1209c99` green.
9. Rewrite `S12-log.md` "For the next stage" for Build C (AI): code map of W6-W11, what the AI will need to call, deviations.

Files owned: as Build A, plus `mods/ww2/defs/` (the two cards, the aid post def, `aid_pit`), `crates/godot-bridge/src/intents.rs`,
`godot/docs/match_state.md`. Not the presenter's GDScript (a later stage).
