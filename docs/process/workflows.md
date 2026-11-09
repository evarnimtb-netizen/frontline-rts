# How slices are built: workflow sizing and cost rules

Owner decisions of 2026-11-03. The weekly plan limit is the project's real
constraint; these rules decide how much agent work each slice gets.

## 1. Two slice tiers

**Engine slices** add or change sim systems, saves, the network protocol or
the AI. Stages: Plan → Build → Tune → Review → Fix → Docs.

**Data slices** add units, weapons or a faction through systems that already
exist (variant system, ammo, fire missions, vehicles). Stages: Build → Tune →
Docs, plus a Review only when the review gate below fires. The orchestrator
writes the spec itself (at most 100 lines, from the roster card) instead of
a Plan agent.

| Slice | Size (× S3) | Tier | Notes |
|---|---|---|---|
| S6 HQ and crates | 0.5 | engine | new static objects, saves, AI supply |
| S7 assault guns, TDs | 0.9 | engine | casemate gunnery, AI |
| S8 half-tracks, trucks, recon | 1.1 | engine | transport, supply vehicles |
| S9 AT guns | 0.9 | engine | towed guns, crews |
| S10a/b fortifications | 1.5 | engine | blueprints, breaching |
| S11a/b artillery | 1.3 | engine | on-map guns, counter-battery, radio |
| S12 wounds and medics | 0.7 | engine | new soldier state, saves |
| S13 heavy wave | 0.5 | data | seven models, existing systems |
| S14 assault kit | 0.8 | engine | flamethrowers, melee |
| S15 USSR | 0.6 | data | third faction, AI data |

S13 and S15 may run in **one workflow** (two Build agents in turn, one Tune,
one Docs) when both turn out to be data-only after S14. Tune them on
separate match sets per matchup; never judge a three-faction balance from
one pooled number.

## 2. Review gate

A Review stage is mandatory when the slice's diff (`git diff --name-only
<base>`) touches any of:

- `crates/sim/src/` outside pure data loading;
- `SAVE_VERSION` or `PROTOCOL_VERSION`, the save layout, the hash;
- `crates/net/`, the relay, the bridge's command path;
- tick order, RNG use, threading.

A data slice whose diff stays in `mods/`, `docs/` and data loaders skips it.
The workflow script checks the diff after Build and runs the Review stage
only when the gate fires.

## 3. Models and reasoning effort

| Stage | Model | Effort |
|---|---|---|
| Plan (engine slices) | Opus | default |
| Build, engine slices | Opus | default |
| Build, data slices | Sonnet | default |
| Tune | Sonnet + autotune | medium allowed |
| Review | Opus | default |
| Fix | Opus | default |
| Docs | Haiku | low allowed |

Owner rule: lowered effort only on routine stages (Tune, Docs). Never on
engine Build, Review or Fix.

## 4. Runs and checks

- Balance batches: `--jobs 6` (measured 2026-11-03 on the M3: 1.44× the
  throughput of `--jobs 3`, identical results, ~33 MB for six matches).
  Tick-time figures only from a clean `--jobs 3` run with nothing else
  running.
- The batch CLI stays the normal release build: `bench-release` (thin LTO,
  codegen-units 1) gave identical results but no speed gain at `--jobs 6`
  (tuner-fix session, 2026-11-03).
- Tuning goes through `tools/autotune` (free local matches); agents pick the
  knobs, ranges and targets and read the report.
- Agents run `tools/check.sh quick <base commit>` only (release tests of the
  changed crates, `cargo fmt` applied); `tools/check.sh tests` is the whole
  release suite between stages; the orchestrator runs `tools/verify.sh`
  (which runs `check.sh full`) before pushing. All print a few lines (logs
  in `target/check/`).
- Since S9b (`docs/process/dev-speed.md`): no agent command blocks longer
  than 4 minutes (the prompt cache lasts 5); the 160 sets, the pooled reading
  and the targets come from `tools/measure.sh` (cached by `tools/batch.sh`),
  never from an agent's own batch; tick time comes only from `verify.sh`;
  ablations by script decide whether a Tune stage runs, and Review and Fix
  come before it.
- Slice numbers come from `frontline-cli targets docs/slices/targets/<slice>.toml`
  (one line per target) and from `--json FILE` results, never from reading
  the text tables (`docs/modding/tuning.md`).

## 5. What agents read

- `docs/STATE.md` first (one page), then only the sections they need.
- `docs/TODO.md` holds open items only. Finished slices' to-do text lives in
  `docs/archive/`; read it only to look up an old question's full text.
- Slice specs at most 300 lines, logs at most 150.
