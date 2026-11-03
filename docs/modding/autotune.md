# Autotune: the Tune stage's number search, run locally

`tools/autotune/autotune.py` searches a few TOML settings inside ranges a
human or agent picked, by running AI-vs-AI batches with `frontline-cli`, and
writes a report and the best override file. Matches cost no plan usage; the
agent only chooses knobs, ranges and targets, starts the run and reads the
report. It never invents a knob: **realism first**. A range must be one
you would defend from sources (price level, rate, cap); the tuner only
decides where inside it the balance lands.

Python 3 standard library only (3.9 works; no venv, no numpy). Run it with `-I`.

## Use in a slice's Tune stage

1. Pick one to three knobs and a range for each, with the historical or design
   reason in the slice log. Prefer one knob; a second only if the first
   cannot reach the band.
2. Copy `tools/autotune/specs/s5a_german_price.toml`, edit knobs, targets, seeds.
3. Build once (`CARGO_BUILD_JOBS=4 cargo build --release -p frontline-cli`),
   then start it in the background and do something else:

   ```sh
   python3 -I tools/autotune/autotune.py tools/autotune/specs/my.toml \
       --cli target/release/frontline-cli --jobs 3
   ```

   `--cli` may point to any built binary (another checkout's included); the
   tool only reads it. `--out DIR` sets the output folder (default
   `tools/autotune/runs/<spec name>/`). Stop it any time; running it again
   resumes from `cache.jsonl`.
4. Read `report.md` in the output folder, then copy the values from
   `best_override.toml` into `mods/ww2` by hand (or ship the override). Quote
   the report's intervals in the slice log, not just the point estimate.
5. Check whatever the targets do not cover (armour-test, fortify-test,
   tick p99 from a clean `--jobs 3` run). Tick p99 in a tuner run is
   inflated if other work shares the machine; do not quote it.

## Spec file

```toml
description = "..."          # shown in the report
minutes = 45                 # --minutes
jobs = 2                     # --jobs (--jobs on the command line wins)
mod_dir = "mods/ww2"         # hashed into the cache key (optional)
tag = ""                     # bump to force fresh runs

[seeds]                      # each seed plays twice (--both-bases): count 20 = 40 matches
screen_start = 1
screen_count = 20
confirm_start = 1
confirm_count = 80
oos_start = 101              # out of sample: other seeds
oos_count = 80

[search]
max_screens = 6      # screens in total (including the initial design)
n_init = 3           # initial design: min, max, middle (1 knob); corners + random (more)
confirm_top = 1      # best screens re-run on the confirm set
noise_frac = 0.15    # GP noise as a share of the loss variance

[[knob]]
name = "ger_price_mult"
type = "float"            # or "int"
min = 0.70
max = 1.00
step = 0.05               # optional grid; without it 21 points
# either one TOML path (dotted, quote ids that contain ":"):
#   path = 'defs.units."ww2:ger_grenadiergruppe".cost'
# or a multiplier on several base values (value is rounded to an integer):
paths = ['defs.units."ww2:ger_smg_gruppe".cost', ...]
bases = [195, ...]

[[target]]
metric = "share_germany"  # see below
lower = 45                # either bound optional
upper = 55
weight = 1.0
screen_lower = 35         # wider bands for the 40-match screen (default: same)
screen_upper = 65
```

Dotted paths follow the override format of `tuning.md` (`[combat] ...`,
`[defs.<kind>."<id>"] ...`). For a multiplier, `bases` are the values it scales
(the unmodified mod's), so list them in the spec.

Metrics (over all matches of the run): `share_<faction>` (e.g.
`share_germany`, `share_usa`; % of matches won; a draw counts for neither),
`base0_share`, `base1_share`, `base_max` (higher of the two),
`len_min`, `len_median`, `len_max` (minutes), `he_mean`, `he_max` (HE share of
kills, %), `tick_p99` (peak ms; reported only, not a target worth setting
under load).

## What it does

1. **Screen** the candidates on the small seed set (default 40 matches).
   Every candidate plays the *same seeds* (common random numbers), so the
   differences between candidates are far less noisy than the matches
   themselves.
2. **Search.** Loss per candidate = sum of weight x (scaled distance outside
   a band)^2, plus a weak pull to the middle of two-sided bands, so the loss
   is smooth and zero in the band. A Gaussian process (RBF kernel, length
   scale chosen by marginal likelihood, a noise term of `noise_frac` of the
   loss variance) is fitted to the screens and the next candidate is the grid
   point with the largest expected improvement. Exact for 1-3 knobs on a grid
   of a few hundred points, written in about 40 lines of pure Python.
   *Why not a bracketing search:* it needs the metric to be monotone in the knob and
   handles only one knob; with 40-match screens whose standard error is
   ~8 points of share, the GP's pooling of neighbours makes better use of
   every noisy sample and extends to two or three knobs unchanged. Its
   weakness: with 4-6 screens it is close to a smoothed interpolation, not a
   global optimiser, and the fixed noise fraction is a simplification.
3. **Confirm.** The best `confirm_top` screens are re-run on the confirm set
   (default 160 matches), the best of those on an out-of-sample set of other
   seeds (default 160). Only the chosen level gets the out-of-sample run.
4. **Report.** `report.md`: every run with metrics, 95 % Wilson intervals on
   every share, the targets' statuses (`pass (CI inside)`, `pass (CI
   overlaps)`, `miss (CI overlaps)`, `FAIL (clear)`), the chosen override and
   a verdict. A faction share on 160 matches has a standard error of about 4
   points (6 wins), so a 10-point band is never confirmed with "CI inside":
   the tool says "not distinguishable from the band" and never "proved".
   The Wilson interval treats matches as independent; the swapped pair of a
   seed is positively related, so it is conservative.

Limits: the sim is chaotic, so the *same seeds* do not stay comparable
across candidates: a one-point price change reshuffles a match. In the demo
the 0.80 level (prices within 1 of the shipped ones) screened at 27.5 % where
the shipped mod scored 42.5 % on the same 40 matches. Common random numbers
therefore reduce noise less than hoped; the binomial error bars are the honest
ones, and the screen is a coarse filter. A screen cannot tell levels apart that differ by less than its
noise (about 8 points of share, more in practice), so a flat region near the band is resolved
by the confirm set, not the screen. If screens and confirm disagree, trust the
confirm and out-of-sample sets. A run uses 4-6 screens and 2 big batches, so
it is cheap but coarse; more `max_screens` or `confirm_top` buys precision at
CPU cost, not plan usage. The cache key covers the CLI binary (size and
mtime), the `mod_dir` TOML contents, `tag`, minutes, seed set and the override.
Rules in `mods/ww2` that you edit by hand while a run is going change the keys.

Demo result (pre-Tune prices x multiplier 0.70-1.00, step 0.05; the shipped
Tune is 0.80): the search chose 0.85, with Germany 42.5 % [35-50] on the 160
confirm set and 48.1 % [41-56] out of sample: not distinguishable from the
45-55 band, and the 0.80 level is equally plausible. The report's verdict says
"not proven" because the confirm point estimate is below the band. One run took
about 35 min at `--jobs 2`.

Demo: `tools/autotune/specs/s5a_german_price.toml` (one multiplier on the six
German card costs); its result is in `tools/autotune/runs/s5a_german_price/report.md`.
