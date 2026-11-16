# S11a1 log (gun fire missions, infantry guns, artillery AI; at most 150 lines)
Spec: common `docs/slices/S11a1.md`, stages `S11a1-build-A.md`, `-B.md`, `-C.md`; targets `targets/S11a1.toml`;
ablations `targets/S11a1-ablations/`. Branch `claude/project-thread-e7nsak`, base `730e67f` (S10c-ai; code
`c1302b3`). Another agent may work in this checkout: commit only your files (`git commit -m ... -- FILES`), never
`git add -A`, never the untracked `.claude/`, `target/`, `tools/__pycache__/`. Do not push.

## For the next stage (rewritten by every stage, not appended; at most 30 lines)
**State: Plan done 2026-11-16. Nothing built. Next: Build A (`S11a1-build-A.md`), starting with its chunk 0.**
- Versions now: SAVE_VERSION 26, PROTOCOL 14. Build A makes the one bump of this slice: SAVE 27; PROTOCOL stays 14.
- Base for every neutrality proof: `target/s11a1-base40/skirmish40.json`, played by Build A chunk 0 from the
  unchanged tree (`BATCH_JOBS=4 tools/batch.sh ...`, command in the stage file). `target/s10c-F3-40/skirmish40.json`
  is the same set from code `c1302b3` if the cache is gone (compare the two with `neutral.py` first).
- The owner is asleep: questions S11a1-1..9 are in `docs/TODO.md`, each assumed on (A). Do not ask anyone; a new
  question goes into the log's top block and into `docs/TODO.md` with a recommended (A) and the same mark.
- Gate "S11a only if S5 and S6 met their HE-share targets": met (13.3-13.9 % and 19.4 % against 26.3 %; now 19.8 %).
- Targets file parses (2026-11-16): `target/release/frontline-cli targets docs/slices/targets/S11a1.toml
  --from-json EMPTY_DIR` prints 552 SKIP lines, exit 0. The six override files name keys that do not exist before
  Build A's key commit: nothing loads them until then (`tools/check.sh` gets its S11a1 dry-run step in Build A).
- Code facts the plan rests on (read 2026-11-16, check before relying on a line number):
  - A mortar mission is `Order::FireMission { target, rounds, m: Mission }` on the squad; `game.rs`
    `mission_refusal` 1402 and `order_fire_mission` 1457; fired in `combat.rs` `fire_mission_tick` 1039 by the man
    `mortar_gunner` returns; bombs through `supply::take_bomb`; the `FireRequest` carries `mission: m.id`.
  - `indirect::solve` takes only the high root `(r + sqrt)/(2a)` of each charge: a +30 deg gun has no solution
    today. `IndirectDef.elevation_deg` is `[u32; 2]`.
  - A served gun is a vehicle squad of one (`guns.rs` `is_gun_squad`); its crew is a separate infantry squad in
    `Served` slots; rounds in `VehicleState::ammo`; it rearms in base, at an HQ, dump or truck through
    `base_ticks` and `supply.rs` `rearm_vehicles` 845 at the shells price.
  - `fire_mission.rs` `linked` 389 knows the voice link only; trait `observer` (1,200 m) exists and no ww2 unit
    has it; `squad_leader` returns `None` for a vehicle squad.
  - `vehicle.rs` 4020 requires `served() == (role == AtGun)`. The AI's AT-gun BUY selects by role (`ai/guns.rs`
    1180-1226), but `take_guns` 400-405, `gun_jobs` 448-455 and `gun_recrews` take every `vs.served` gun: Build A
    splits them by role. `outside_limit` (`vehicle.rs` 385) must list the new role.
  - ww2 has 15 vehicle defs = `kill_class::MAX_VEHICLE_TYPES` (`game.rs` 548), sorted by id; the class u8 is
    saved (`structures.rs` 129). Two more defs need the widening of spec 3 in the SAVE 27 commit.
  - The load check `game.rs` 2484-2531 and `end_mission` (`combat.rs` 1260) assume a mortar squad.
  - `ai/breach.rs` `run_breach` 487: phase Stage leaves on `covered` (a base of fire, or twice `suppress_lead_s`);
    `breach_max_s` 180; breach jobs start 0 times in 480 matches (the front wire is rounded).
  - `tools/neutral.py` compares `per_match` records field by field; S10c-ai added the rule "a key starting `s10c`
    that BASE lacks is skipped" (Build B adds the same for `s11a1`).
- Stop rules for every stage: red `check.sh quick` is never left; a non-neutral `a_off` 40 set is a bug to find;
  no value of lethality, armour, penetration or price is changed to reach a target (owner rule: features first,
  cost and availability before stats).

## Plan revision (2026-11-16, after an independent review: 1 blocker, 6 major, 4 minor; all verified in the code, none rejected)
Spec 3: kill-class carrier widened in the SAVE 27 commit (15-type cap), `fo_lost_at`, `guns_bought`,
`gun_direct_rounds`. G2: mask steps 2..=16 at 0.5 m, hull-pitch limit, test with zero error. G3: mission ttl, load
check and `end_mission` for gun squads, infantry guns counted only in `S11Counters` (T80, T84, mortar rows clean).
A1: role split of `take_guns` / `gun_jobs` / `gun_recrews` and `outside_limit` moved into Build A; Build C wiring
(lists before `assign`, `arty_pot` beside `gun_pot` incl. `ai/fort.rs`, `mortars` before `arty`). Balance note: the
high-angle leIG may favour Germany; the measure reads `arty_off` first.

## Plan (2026-11-16)
Scope (spec 1): gun fire missions on the S5b mission code and the S9 gun entity; low- and high-angle solutions
with a mask test; gun aim-error keys; a radio link; leIG 18, 105 mm M3, VB-Trupp, FO Party at the design's
figures; fortify rows `gun_fm`, `mask`, `barrage`, `breach_prep`; AI: one gun and one observer team a side, static
in base, observed missions inside a 25 % shell budget, lifted near own men; preparation fire for the breach job by
guns and mortars. Cut to S11a2 with reasons (spec 1, outline spec 12): crate-only shells and AI resupply, friendly
fire, registration, counter-battery, displacement and tows, defensive fire, assault hold, HEAT, per-faction delays.
Decisions taken for the owner (all (A), `docs/TODO.md` S11a1-1..9): split; priced base rearm for now; friendly
fire in S11a2; no tows; prices 320 / 380 / 80 / 120; only the breach job waits for preparation fire; T131 hard at
60 %; HEAT later; cards buyable before the presenter pass.
The breach gap, honestly: preparation fire is aimed at the staged row (T131: 30 % -> >= 60 %). In matches breach
jobs do not start (T123), so `prep_missions_per_match` is expected near 0 there; that needs S10c-6 (B).
Stage split: A (SAVE 27 layout, keys, solver, gun missions, radio, rearm switch; neutral by content) -> B (content,
rows, JSON; neutral because the AI buys nothing; gate T128-T130) -> C (AI chunk 1 A1-A6, chunk 2 P1-P3 and the
breach row; fallback: stop after chunk 1 with `prep` 0).
