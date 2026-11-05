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
- S6-1..10 (`docs/slices/S6-hq-crates.md` "Owner questions"), each on its
  (A): S6-1 a carried shells crate rearms a tank within 5 m (owner request
  2026-11-01 vs Appendix A); S6-2 cook-off bursts belong to whoever broke
  the box; S6-3 a dead carrier's crate drops and stays his; S6-4 HQ costs
  100 MP and dig time, no material until S10; S6-5 bullets stop at boxes,
  only HE cooks off; S6-6 mortar teams dig their own pit; S6-7 radio box is
  a state only until S11; S6-8 build cost paid on order; S6-9 retreat to
  the HQ, else base; S6-10 a broken stock box loses all its stock.
- S6-11..16 (raised in the S6 build), each on its (A): S6-11 the base
  issues one crate per `base_crate_issue_s` per player and refuses (Busy)
  in between instead of queueing; (B) a FIFO queue. S6-12 mortar teams
  do not dig (dig rate 0): a mortar pit is dug by engineers or the
  Zugtrupp with `Dig` profile `mortar_pit` (S6-6 (A) deferred: it needs a
  per-profile dig rate). S6-13 cook-offs burst as the faction's 75 mm HE
  round (no separate cook-off ammo defs). S6-14 a structure whose stock
  box breaks is destroyed (pit stays; the HQ's `rebuild_s` starts). S6-15
  a carrier's whole squad walks at the crate's carry pace. S6-16 the AI
  builds no dumps yet, and its mortars do not target enemy HQs and crates.
- S6-17..19 (raised in the S6 review fix), each on its (A): S6-17 an AI
  tank far from every source (`tank_crate_far_m` 300 m straight line, the
  spec's "path budget") holds for a bearers' crate run instead of driving
  back; (B) path length. S6-18 structures are not selectable yet: the HQ
  squad's strip shows its HQ (else the first dump), other dumps show only
  their ring and boxes, until a later HUD pass; (B) structure selection in
  S6. S6-19 a carrier sets his crate down within 2 x `pickup_m` of the
  point or where he stands once his squad holds its formation there.

## Planned work queue

1. Infra step (done on branch `infra-checks`, to be pushed): `--json` results,
   `frontline-cli targets`, `tools/check.sh`, `docs/STATE.md`.
2. Autotune fix done (branch `claude/tuner-fix`, `c72f91d`, merged by the
   infra step): pooled logistic fit, synthetic proof, real run. The real run
   puts Germany at 50 % with German infantry cards x0.959 [0.913-0.999]
   (today's prices: 44.9 % [40.0-49.9]); input for S6 Tune, not shipped.
3. S6 HQ and ammo crates (`docs/design/roster/roster-expansion.md` S6 card).
4. Sim profiling: deferred until after S6 (owner 2026-11-03).

## Known issues still open

Balance issues from S1-S3c are superseded by later measurements (latest:
`docs/slices/S5b-log.md` "Final measurements"). Still open:

- Map: base 1 is worth about 6 points of win share to either faction.
- Mortars kill too little in AI play (about 5 % of kills): S6 Tune.
- The base-parked mortar fires unlimited from base: S6.
- fortify-test mortar row 39.5 % against a 40 % limit (marginal).
- AI rarely throws satchels, rarely fires Panzerfausts (carriers die
  waiting; AI tanks stand off).
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
