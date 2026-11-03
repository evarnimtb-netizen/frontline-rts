#!/usr/bin/env python3
"""Synthetic proof harness for autotune.py: no sim, matches drawn from a known
truth, so we can count how often the tuner finds the true level.

    python3 -I tools/autotune/synth.py --reps 200 --out tools/autotune/runs/synthetic/report.md

Truth (one knob x, a price multiplier on Germany's cards):
    logit P(Germany wins) = b (x - x*) [x halves b above x* when kinked]
                            + 0.24 s (s = +-1/2: about 6 points of base advantage)
                            + u_seed (N(0, 0.3), the same seed at every level)
b is set so that one 0.05 step moves the share by `pts` points near 50 %.
A match is a deterministic function of (truth, level, seed, side), like the
real sim: re-running a level on the same seeds returns the same matches.

The old search (GP + expected improvement on 40-match screens, then confirm
and out-of-sample sets) is kept here, and only here, for the comparison.
Standard library only.
"""
import argparse, math, os, random, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import autotune as at  # noqa: E402

BASE = 0.24
SEED_SD = 0.3

class Truth:
    def __init__(self, x_star, pts, kink=1.0, step=0.05):
        self.x_star, self.kink = x_star, kink
        self.b = -4 * (pts / 100) / step      # Germany loses as its prices rise
        self.tag = random.random()

    def eta(self, x):
        d = x - self.x_star
        return self.b * d * (self.kink if d > 0 else 1.0)

    def share(self, x):
        """True Germany share (%), averaged over seeds and bases."""
        tot = 0.0
        for i in range(-12, 13):            # normal quadrature on a grid
            u = i / 4 * SEED_SD
            w = math.exp(-0.5 * (i / 4) ** 2)
            for s in (0.5, -0.5):
                tot += w * at._sig(self.eta(x) + BASE * s + u)
        return 100 * tot / (2 * sum(math.exp(-0.5 * (i / 4) ** 2) for i in range(-12, 13)))

    def level(self, share=50.0):
        lo, hi = 0.0, 2.0
        for _ in range(60):
            mid = (lo + hi) / 2
            lo, hi = (mid, hi) if self.share(mid) > share else (lo, mid)
        return (lo + hi) / 2

    def batch(self, x, seed, seeds):
        """A record shaped like Runner.run's: Germany holds base 0 in 'normal'."""
        ms = []
        for sd in range(seed, seed + seeds):
            u = random.Random(f"{self.tag}|seed|{sd}").gauss(0, SEED_SD)
            for bases, s in (("normal", 0.5), ("swapped", -0.5)):
                r = random.Random(f"{self.tag}|{round(x, 6)}|{sd}|{bases}")
                ger = r.random() < at._sig(self.eta(x) + BASE * s + u)
                gbase = 0 if bases == "normal" else 1
                ms.append({"seed": sd, "bases": bases, "winner": "germany" if ger else "usa",
                           "base": gbase if ger else 1 - gbase,
                           "len_min": math.exp(r.gauss(math.log(18), 0.25)), "he": max(0.0, r.gauss(11, 4))})
        return {"matches": ms, "tick_p99": None, "sides": {}, "wall_s": 0.0}

# ------------------------------------------------- the old method (GP + EI)

def _chol(A):
    n = len(A)
    L = [[0.0] * n for _ in range(n)]
    for i in range(n):
        for j in range(i + 1):
            s = A[i][j] - sum(L[i][k] * L[j][k] for k in range(j))
            if i == j:
                L[i][i] = math.sqrt(max(s, 1e-12))
            else:
                L[i][j] = s / L[j][j]
    return L

def _solve(L, b):
    n = len(b)
    y = [0.0] * n
    for i in range(n):
        y[i] = (b[i] - sum(L[i][k] * y[k] for k in range(i))) / L[i][i]
    x = [0.0] * n
    for i in reversed(range(n)):
        x[i] = (y[i] - sum(L[k][i] * x[k] for k in range(i + 1, n))) / L[i][i]
    return x, y

def gp_fit(X, y, noise_frac):
    n = len(y)
    mu = sum(y) / n
    var = max(1e-6, sum((v - mu) ** 2 for v in y) / n)
    kf = lambda a, b, ls: math.exp(-0.5 * (a - b) ** 2 / (ls * ls))
    best = None
    for ls in (0.12, 0.2, 0.3, 0.5, 0.8):
        K = [[var * kf(X[i], X[j], ls) + (noise_frac * var + 1e-8 if i == j else 0) for j in range(n)] for i in range(n)]
        L = _chol(K)
        a, _ = _solve(L, [v - mu for v in y])
        ll = -0.5 * sum((v - mu) * ai for v, ai in zip(y, a)) - sum(math.log(L[i][i]) for i in range(n))
        if best is None or ll > best[0]:
            best = (ll, ls, L, a)
    _, ls, L, a = best

    def predict(x):
        ks = [var * kf(x, xi, ls) for xi in X]
        m = mu + sum(k * ai for k, ai in zip(ks, a))
        _, v = _solve(L, ks)
        return m, math.sqrt(max(1e-12, var - sum(t * t for t in v)))
    return predict

def _ei(m, s, best):
    if s < 1e-9:
        return 0.0
    z = (best - m) / s
    return (best - m) * 0.5 * (1 + math.erf(z / math.sqrt(2))) + s * math.exp(-0.5 * z * z) / math.sqrt(2 * math.pi)

def legacy_loss(v, lo, hi):
    s = (hi - lo) / 2
    viol = max(0.0, lo - v, v - hi)
    return (viol / s) ** 2 + 0.05 * ((v - (lo + hi) / 2) / s) ** 2

def legacy_search(truth, lo=0.70, hi=1.00, step=0.05, max_screens=4):
    """The old autotune: 40-match screens on seeds 1..20 (band 35-65), GP + EI,
    best screen confirmed on seeds 1..80 (band 45-55), then seeds 101..180."""
    grid = [round(lo + i * step, 6) for i in range(int(round((hi - lo) / step)) + 1)]
    share = lambda rec: 100 * sum(m["winner"] == "germany" for m in rec["matches"]) / len(rec["matches"])
    screened = {}
    for x in (grid[0], grid[-1], grid[len(grid) // 2]):
        screened[x] = legacy_loss(share(truth.batch(x, 1, 20)), 35, 65)
    while len(screened) < max_screens:
        xs = list(screened)
        pred = gp_fit([(x - lo) / (hi - lo) for x in xs], [screened[x] for x in xs], 0.15)
        best = min(screened.values())
        todo = [x for x in grid if x not in screened]
        if not todo:
            break
        sc = sorted(((_ei(*pred((x - lo) / (hi - lo)), best), pred((x - lo) / (hi - lo))[0], x) for x in todo),
                    key=lambda t: (-t[0], t[1]))
        if sc[0][0] < 1e-4 and best == 0.0 and len(screened) >= 4:
            break
        screened[sc[0][2]] = legacy_loss(share(truth.batch(sc[0][2], 1, 20)), 35, 65)
    chosen = min(screened, key=screened.get)
    conf = share(truth.batch(chosen, 1, 80))
    oos = share(truth.batch(chosen, 101, 80))
    both = 45 <= conf <= 55 and 45 <= oos <= 55
    matches = 40 * len(screened) + 320 - 40     # the confirm set reuses the chosen screen's 40
    return {"chosen": chosen, "x_hat": chosen, "lo": None, "hi": None, "matches": matches,
            "verdict": "pass" if both else "not proven"}

# ----------------------------------------------------------- the new method

def new_search(truth, lo=0.70, hi=1.00, step=0.01, tol=0.025, max_matches=480, batch_seeds=20):
    knob = at.Knob({"name": "ger_price_mult", "min": lo, "max": hi, "step": step, "tol": tol,
                    "paths": ["x"], "bases": [1]})
    tgt = at.Target({"metric": "share_germany", "lower": 45, "upper": 55})
    st = at.search([knob], [tgt], {"max_matches": max_matches, "batch_seeds": batch_seeds},
                   lambda xs, seed, n: (truth.batch(xs[0], seed, n), False))
    lv = st.rows[0]["levels"][0]
    return {"chosen": st.chosen[0], "x_hat": st.x_hat[0], "lo": lv[1], "hi": lv[2],
            "matches": st.used, "verdict": st.verdict}

# -------------------------------------------------------------- comparison

CONFIGS = [  # name, points per 0.05 step, kink factor above x*
    ("6 pts/step", 6, 1.0),
    ("5 pts/step", 5, 1.0),
    ("8 pts/step", 8, 1.0),
    ("6 pts/step, kinked (half slope above x*)", 6, 0.5),
]

METHODS = [  # label, function
    ("old: GP, 4 screens + confirm + oos", lambda t: legacy_search(t, max_screens=4)),
    ("old: GP, 6 screens + confirm + oos", lambda t: legacy_search(t, max_screens=6)),
    ("new: logistic, budget 480", lambda t: new_search(t, max_matches=480)),
    ("new: logistic, budget 800", lambda t: new_search(t, max_matches=800)),
    ("new: logistic, budget 1200", lambda t: new_search(t, max_matches=1200)),
]

def evaluate(method, pts, kink, reps, seed=1):
    rng = random.Random(f"synth|{pts}|{kink}|{seed}")
    out = {"in_band": 0, "err25": 0, "err50": 0, "cover": 0, "has_iv": 0, "matches": 0, "abs_err": 0.0, "claims": 0, "false": 0,
           "verdicts": {}}
    for _ in range(reps):
        x_star = rng.uniform(0.75, 0.95)
        t = Truth(x_star, pts, kink)
        t.tag = rng.random()
        xt = t.level(50.0)
        r = method(t)
        err = abs(r["chosen"] - xt)
        out["abs_err"] += err
        ok = 45 <= t.share(r["chosen"]) <= 55
        out["in_band"] += ok
        if r["verdict"] in ("in band", "pass", "level resolved"):
            out["claims"] += 1
            out["false"] += not ok
        out["err25"] += err <= 0.025
        out["err50"] += err <= 0.05
        if r["lo"] is not None:
            out["has_iv"] += 1
            out["cover"] += r["lo"] <= xt <= r["hi"]
        out["matches"] += r["matches"]
        out["verdicts"][r["verdict"]] = out["verdicts"].get(r["verdict"], 0) + 1
    return out

def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--reps", type=int, default=200)
    ap.add_argument("--out")
    a = ap.parse_args()
    t0 = time.time()
    L = ["# Synthetic comparison: old GP search vs logistic dose-response", "",
         f"{a.reps} repetitions per cell; the true level x* is drawn uniformly from 0.75-0.95 in a knob range "
         "0.70-1.00. Truth model and noise: see the docstring of `tools/autotune/synth.py` (seed effect sd 0.3 "
         "on the logit, shared across levels; 6-point base advantage; the swapped twin of a seed shares it). "
         "The old method uses the demo's 0.05 grid; the new one a 0.01 grid and tolerance +-0.025.", "",
         "Columns: **true share in band** = the true Germany share at the chosen level lies in 45-55 "
         "(what the owner wants); |err| <= 0.025 / 0.05 = chosen level within that distance of the true "
         "50 % level; **interval covers** = the reported 95 % interval on the level contains the true level "
         "(only the new method reports one); **false pass claims** = runs whose verdict claims success (old: 'pass' on confirm and oos; new: 'in band' or 'level resolved') although the true share at the chosen level is outside 45-55; matches = mean matches used.", ""]
    for name, pts, kink in CONFIGS:
        L += [f"## {name}", "",
              "| method | true share in band | abs err <= 0.025 | abs err <= 0.05 | mean abs err | interval covers | false pass claims | matches | verdicts |",
              "|---|---|---|---|---|---|---|---|---|"]
        for label, fn in METHODS:
            o = evaluate(fn, pts, kink, a.reps)
            n = a.reps
            cov = f"{100*o['cover']/o['has_iv']:.0f} % ({o['has_iv']} bounded)" if o["has_iv"] else "-"
            vd = ", ".join(f"{k} {v}" for k, v in sorted(o["verdicts"].items()))
            L.append(f"| {label} | {100*o['in_band']/n:.0f} % | {100*o['err25']/n:.0f} % | {100*o['err50']/n:.0f} % | "
                     f"{o['abs_err']/n:.3f} | {cov} | {o['false']}/{o['claims']} | {o['matches']/n:.0f} | {vd} |")
            print(L[-1], flush=True)
        L.append("")
    L.append(f"Run time {time.time() - t0:.0f} s.")
    text = "\n".join(L) + "\n"
    if a.out:
        os.makedirs(os.path.dirname(a.out), exist_ok=True)
        open(a.out, "w").write(text)
    else:
        print(text)

if __name__ == "__main__":
    main()
