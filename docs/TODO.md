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
  cooldown (rifle 24 s, engineers/MG 36, AT 48, mortar 60, tank 120); each
  card its own cooldown.
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

## Planned work queue

1. Infra step: `--json` results, `frontline-cli targets`, `tools/check.sh`,
   `docs/STATE.md` (workflow staged).
2. Autotune fix (session `tuner-fix`, branch `claude/tuner-fix`, paused):
   pooled logistic fit, synthetic proof done; real run and timing left.
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
