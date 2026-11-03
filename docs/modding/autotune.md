# Autotune: the Tune stage's number search, run locally

`tools/autotune/autotune.py` solves for one to three TOML settings inside
ranges a human or agent picked, by running AI-vs-AI batches with
`frontline-cli`, and writes a report and the chosen override file. Matches cost
no plan usage; the agent only chooses knobs, ranges and targets, starts the run
and reads the report. It never invents a knob: **realism first**. A range must
be one you would defend from sources (price level, rate, cap); the tuner only
decides where inside it the balance lands.

Python 3 standard library only (3.9 works; no venv, no numpy). Run it with `-I`.

## Use in a slice's Tune stage

1. Pick one knob (two or three only if one cannot reach the band) and a range,
   with the historical or design reason in the slice log.
2. Copy `tools/autotune/specs/s5b_german_price.toml`; edit knob, targets, budget.
3. Build once, then start it in the background and do something else:

   ```sh
   CARGO_BUILD_JOBS=4 cargo build --profile bench-release -p frontline-cli
   python3 -I tools/autotune/autotune.py tools/autotune/specs/my.toml
   ```

   Defaults: `--jobs 6`, and the CLI `target/bench-release/frontline-cli` if
   it exists, else `target/release/frontline-cli` (`--cli` picks any binary,
   another checkout's included). `--out DIR` sets the output folder (default
   `tools/autotune/runs/<spec name>/`). Stop it any time; running it again
   resumes from `cache.jsonl`.
4. Read `report.md`. Quote the **solved level with its interval** and the
   predicted share with its interval in the slice log, not a point estimate.
   Copy the values from `best_override.toml` into `mods/ww2` by hand.
5. Check what the targets do not cover (armour-test, fortify-test). **Tick
   p99 only from a clean `--jobs 3` run** with nothing else on the machine;
   the tuner's tick figures (6 parallel matches) are inflated, do not quote them.

## Spec file

```toml
description = "..."          # shown in the report
minutes = 45                 # --minutes
jobs = 6                     # --jobs (the command line wins)
mod_dir = "mods/ww2"         # hashed into the cache key (optional)
tag = ""                     # bump to force fresh runs

[search]
batch_seeds = 20             # one batch: 20 fresh seeds x both bases = 40 matches
max_matches = 800            # budget; a run stops earlier once settled
min_matches = 0
seed_start = 1001            # batch i plays seeds seed_start + 20 i ..

[[knob]]
name = "ger_price_mult"
type = "float"               # or "int"
min = 0.85
max = 1.15
step = 0.01                  # grid the batches are placed on
tol = 0.025                  # settled when the 95 % level interval is within +- tol (default: step)
# one TOML path:  path = 'defs.units."ww2:ger_grenadiergruppe".cost'
# or a multiplier on several base values (rounded to integers):
paths = ['defs.units."ww2:ger_grenadiergruppe".cost', ...]
bases = [225, ...]           # today's values in the mod

[[target]]
metric = "share_germany"
lower = 45                   # either bound optional; no bound at all = reported only
upper = 55
weight = 1.0
# role = "check"             # force a modellable metric to be only checked
```

Dotted paths follow the override format of `tuning.md`. Specs of the old GP
method (`[seeds]`, `max_screens`, `screen_lower`, ...) still parse; those keys
are ignored.

**Modelled metrics** (solved for, fitted over all matches): `share_<faction>`
(% of matches won; a draw counts for neither), `he_mean` (HE share of kills
per match, %), `len_mean` (minutes), `kill_<source>_<faction>` (% of that
side's kills by a source, per batch, weighted by kills: `kill_mortar_germany`,
`kill_tank_he_usa`; sources `mortar tank_he frag satchel rocket tank_ap mg
rifle smg vehicle_mg`). **Checked metrics** (measured on the matches near the
chosen level): `base0_share`, `base1_share`, `base_max`, `len_min`,
`len_median`, `len_max`, `he_max`, `tick_p99`, plus any modelled metric with
`role = "check"`. The first modelled target steers the design.

All summary parsing is in `parse_summary()`; a later step switches it to a
`--json` file.

## Method

The sim is chaotic: a one-point price change reshuffles a match, so "the same
seeds" do not make two levels comparable, and a 40-match share has a 95 %
interval of about +-15 points. The old method (a Gaussian process on the loss
of 40-match screens) mostly fitted that noise: in the S5a demo its screens
read 67.5, 27.5, 47.5, 27.5 % at 0.70, 1.00, 0.85, 0.80. It is gone; see the
comparison below.

1. **Every match is one observation.** A faction's win is fitted by a
   logistic dose-response model, `logit P(win) = b0 + b . knobs + c . side`
   (side = +-1/2 for which base the faction held), by maximum likelihood over
   every match of every level at once (Newton/IRLS). Other modelled metrics
   use least squares. Intervals are cluster-robust: a seed's normal and
   swapped match are one cluster. Prediction averages the two bases.
2. **Fresh seeds every batch.** Re-running a level on the same seeds would
   return the same matches (determinism), so every batch plays new seeds.
3. **Initial design:** the range ends and the middle (with more knobs: the
   centre and each knob's ends), one batch each (120 matches for one knob).
4. **Sequential design.** After each batch the models are refitted and the
   best level is the one whose predicted values minimise the band loss (for a
   share band: its middle). Until the slope is clearly non-zero, batches go to
   the initial points. Then they cycle: the solved level, a flank below, the
   solved level, a flank above; the flanks sit where the predicted share
   moves by about 15 points, so the slope stays estimable while most matches
   land near the level being pinned down.
5. **Stop** when every modelled target is settled: its predicted 95 %
   interval at the chosen level is inside the band ("in band"), or clearly
   outside it at the best level ("out of band", the range cannot reach it), or
   the level interval is within `tol` ("level resolved"); or when the budget
   is spent ("not resolved").
6. **Checks.** Every target is measured on the matches played within `tol`
   of the chosen level (Wilson intervals on shares), so `base_max` and the
   length bounds are feasibility checks on the chosen level.

### How to read the report

- **Solved level**: `share_germany = 50 at ger_price_mult = 0.93, 95 %
  interval 0.90 to 0.96` is a Fieller interval: the levels at which a 50 %
  share is not rejected. It accounts for the slope's uncertainty and becomes
  "unbounded" when the data cannot tell the knob matters. The line under it
  gives the levels where the fitted curve is inside the band.
- **Predicted at the chosen level**: the model's share and its interval. It
  pools all matches through the fitted curve, so it is much tighter than a
  raw 160-match share (+-7.7 points) — as long as the curve's shape is right.
- **Per level table**: observed (pooled, Wilson) next to the model; and the
  **lack-of-fit** test in the Fit section. If lack of fit is flagged
  (p < 0.05) or the observed shares zig-zag around the curve, do not trust
  the intervals; narrow the range around the solved level and run again.
- **Slope**: points of share per `tol` of knob, with its interval; and the
  base-side effect (base 0 minus base 1, points).
- **Verdicts**: *in band* (every modelled target's interval inside its band,
  no check fails), *level resolved* (level known to `tol`, the value not
  distinguishable from the band), *not found* (a target clearly outside its
  band at the best level, or a check clearly fails), *not resolved* (budget).

### Proof on synthetic truth

`tools/autotune/synth.py` draws matches from a known logistic truth (no sim;
seed effect shared across levels, 6-point base advantage, the true level
uniform in 0.75-0.95 of a 0.70-1.00 range) and runs both methods 200 times
per setting; `runs/synthetic/report.md` has the full table. With 6 points of
share per 0.05 step:

| method | true share at the chosen level in 45-55 | 95 % interval covers the true level | matches |
|---|---|---|---|
| old GP, 4 screens + confirm + oos | 46 % | (none reported) | 440 |
| old GP, 6 screens + confirm + oos | 48 % | (none reported) | 519 |
| new, budget 800 | 97 % | 96 % | 506 (mean; stops when settled) |

At 5 and 8 points per step the new method gets 98 % (old 42-56 %). With a
budget of 480 about half the runs stop "not resolved"; plan on **about 500-550
matches for one knob, budget 800**. "In band" verdicts were wrong in about
3 % of runs (true share just outside the band), as a 95 % interval allows.
Weak spot: when the true curve is not logistic (a kink that halves the slope
above the level) the interval covers only ~84 % and the level error doubles;
the chosen level was still in band 96 % of the time. That is what the
lack-of-fit line is for.

Tests: `python3 -I tools/autotune/test_autotune.py` (parser, fitter, Fieller
coverage, a 25-run harness check, the out-of-range case).

## Speed

On the M3 (4P+4E, 8 GB): 20 matches take 35.5 s at `--jobs 3`, 24.7 s at
`--jobs 6`, 22.4 s at `--jobs 8` (identical summaries; ~33 MB peak for 6
jobs), hence the default 6. The `bench-release` profile (thin LTO, one codegen
unit) gives the same 20-match summary as `release` (same hashes and winners,
checked); SPEED_NOTE. A one-knob run of ~500 matches takes about 11 minutes.

## Real run on the current game

SPEC_RESULT
