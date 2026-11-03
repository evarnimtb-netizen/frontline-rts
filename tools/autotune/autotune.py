#!/usr/bin/env python3
"""Local automatic balance tuner (docs/modding/autotune.md).

Searches 1-3 TOML knobs inside ranges somebody chose, by running AI-vs-AI
batches with frontline-cli, and writes a report and the best override file.

    python3 -I tools/autotune/autotune.py SPEC.toml --cli target/release/frontline-cli

Standard library only.
"""
import argparse, hashlib, json, math, os, random, re, subprocess, sys, time

# ---------------------------------------------------------------- spec / TOML

def _val(s):
    s = s.strip()
    if s.startswith('"""'):
        return s[3:-3].lstrip("\n")
    if s.startswith('"'):
        return json.loads(s)
    if s.startswith("'"):
        return s[1:-1]
    if s.startswith('['):
        body = s[1:-1].strip()
        items, cur, q, depth = [], "", False, 0
        for ch in body:
            if ch in "\"'":
                q = not q
            if ch == ',' and not q and depth == 0:
                items.append(cur); cur = ""
            else:
                cur += ch
        if cur.strip():
            items.append(cur)
        return [_val(x) for x in items]
    if s in ("true", "false"):
        return s == "true"
    s = s.replace("_", "")
    try:
        return int(s)
    except ValueError:
        return float(s)

def _strip_comment(line):
    q = False
    for i, ch in enumerate(line):
        if ch == '"' and (i == 0 or line[i - 1] != '\\'):
            q = not q
        if ch == '#' and not q:
            return line[:i]
    return line

def parse_toml(text):
    """Small TOML subset: [t], [[t]], key = scalar | one-line array."""
    try:
        import tomllib
        return tomllib.loads(text)
    except ImportError:
        pass
    root = cur = {}
    pending = ""
    lines = iter(text.splitlines())
    for raw in lines:
        if pending:
            line = pending + " " + _strip_comment(raw).strip()
            pending = ""
        else:
            line = _strip_comment(raw).strip()
        if not line:
            continue
        if line.count('"""') == 1:      # multi-line string (description)
            buf = [line.split("=", 1)[1].strip()[3:]]
            for r in lines:
                if '"""' in r:
                    buf.append(r.split('"""')[0]); break
                buf.append(r)
            cur[line.split("=", 1)[0].strip()] = "\n".join(buf).strip()
            continue
        if "=" in line and not line.startswith("[") and line.count("[") > line.count("]"):
            pending = line
            continue
        if line.startswith('[['):
            name = line[2:-2].strip()
            cur = {}
            root.setdefault(name, []).append(cur)
        elif line.startswith('['):
            name = line[1:-1].strip()
            cur = root.setdefault(name, {})
        else:
            k, v = line.split('=', 1)
            cur[k.strip()] = _val(v)
    return root

def split_path(p):
    """'defs.units."ww2:x".cost' -> ['defs','units','ww2:x','cost']"""
    out = re.findall(r'"([^"]*)"|([^.\s"]+)', p)
    return [a or b for a, b in out]

def _tv(v):
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, int):
        return str(v)
    if isinstance(v, float):
        return repr(v)
    return json.dumps(v)

def _tk(k):
    return k if re.fullmatch(r'[A-Za-z0-9_-]+', k) else json.dumps(k)

def write_toml(tree, prefix=()):
    lines, subs = [], []
    for k, v in tree.items():
        (subs if isinstance(v, dict) else lines).append((k, v))
    out = ""
    if lines and prefix:
        out += "[" + ".".join(_tk(x) for x in prefix) + "]\n"
    for k, v in lines:
        out += f"{_tk(k)} = {_tv(v)}\n"
    if lines or not prefix:
        out += "\n" if out else ""
    for k, v in subs:
        out += write_toml(v, prefix + (k,))
    return out

# ----------------------------------------------------------------- knobs

class Knob:
    def __init__(self, d):
        self.name = d["name"]
        self.type = d.get("type", "float")
        self.lo, self.hi = d["min"], d["max"]
        self.step = d.get("step")
        if "path" in d:
            self.paths, self.bases = [d["path"]], None
        else:                       # multiplier on base values
            self.paths, self.bases = d["paths"], d["bases"]
            assert len(self.paths) == len(self.bases)

    def grid(self):
        if not self.step:
            n = 21
            return [self.lo + (self.hi - self.lo) * i / (n - 1) for i in range(n)]
        n = int(round((self.hi - self.lo) / self.step))
        g = [self.lo + i * self.step for i in range(n + 1)]
        return [int(round(x)) if self.type == "int" else round(x, 6) for x in g]

    def apply(self, tree, x):
        for i, p in enumerate(self.paths):
            v = x if self.bases is None else self.bases[i] * x
            if self.bases is not None or self.type == "int":
                v = int(round(v))
            node = tree
            keys = split_path(p)
            for k in keys[:-1]:
                node = node.setdefault(k, {})
            node[keys[-1]] = v

def build_override(knobs, xs):
    tree = {}
    for k, x in zip(knobs, xs):
        k.apply(tree, x)
    return write_toml(tree)

# ---------------------------------------------------------------- running

MATCH_RE = re.compile(
    r'^(\d+)\s+(normal|swapped)\s+(\S+)\s+(\S+)\s+(\d+):(\d+)\s+.*?\s([\d.]+)\s+0x[0-9a-f]+\s*$')
TICK_RE = re.compile(r'tick p99 ms.*?peak ([\d.]+)')

def parse_summary(text):
    matches = []
    for line in text.splitlines():
        m = MATCH_RE.match(line)
        if m:
            w = m.group(3).replace("ww2:", "")
            matches.append({"seed": int(m.group(1)), "bases": m.group(2),
                            "winner": w if w not in ("-", "none") else None,
                            "base": int(m.group(4)) if m.group(4).isdigit() else None,
                            "len_min": int(m.group(5)) + int(m.group(6)) / 60,
                            "he": float(m.group(7))})
    t = TICK_RE.search(text)
    return matches, float(t.group(1)) if t else None

class Runner:
    def __init__(self, spec, cli, outdir, jobs, log):
        self.cli, self.outdir, self.jobs, self.log = cli, outdir, jobs, log
        self.minutes = spec.get("minutes", 45)
        self.cachefile = os.path.join(outdir, "cache.jsonl")
        self.cache = {}
        self.env_id = self._env_id(spec)
        if os.path.exists(self.cachefile):
            for line in open(self.cachefile):
                if line.strip():
                    r = json.loads(line)
                    self.cache[r["key"]] = r

    def _env_id(self, spec):
        st = os.stat(self.cli)
        h = hashlib.sha256(f"{st.st_size}:{int(st.st_mtime)}:{spec.get('tag','')}".encode())
        mods = spec.get("mod_dir")
        if mods:                     # changing a mod file invalidates the cache
            for root, _, files in sorted(os.walk(mods)):
                for f in sorted(files):
                    if f.endswith(".toml"):
                        h.update(open(os.path.join(root, f), "rb").read())
        return h.hexdigest()[:16]

    def run(self, override, seed, seeds):
        key = hashlib.sha256(f"{self.env_id}|{self.minutes}|{seed}|{seeds}|{override}".encode()).hexdigest()[:24]
        if key in self.cache:
            return self.cache[key], True
        os.makedirs(os.path.join(self.outdir, "overrides"), exist_ok=True)
        path = os.path.join(self.outdir, "overrides", key + ".toml")
        open(path, "w").write(override)
        cmd = [self.cli, "skirmish", "--ai-vs-ai", "--seed", str(seed), "--seeds", str(seeds),
               "--both-bases", "--summary", "--jobs", str(self.jobs), "--minutes", str(self.minutes)]
        if override.strip():
            cmd += ["--rules-override", path]
        t0 = time.time()
        p = subprocess.run(cmd, capture_output=True, text=True)
        matches, tick = parse_summary(p.stdout)
        if p.returncode != 0 or len(matches) != 2 * seeds:
            raise SystemExit(f"batch failed (rc={p.returncode}, {len(matches)} of {2*seeds} matches):\n{p.stderr[-800:]}")
        rec = {"key": key, "seed": seed, "seeds": seeds, "override": override,
               "matches": matches, "tick_p99": tick, "wall_s": round(time.time() - t0, 1)}
        self.cache[key] = rec
        with open(self.cachefile, "a") as f:
            f.write(json.dumps(rec) + "\n")
        return rec, False

# ---------------------------------------------------------------- metrics

def wilson(k, n, z=1.96):
    if n == 0:
        return 0.0, 100.0
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return 100 * (c - h), 100 * (c + h)

def metrics(rec):
    ms = rec["matches"]
    n = len(ms)
    out = {"n": n, "tick_p99": rec["tick_p99"]}
    counts = {}
    for m in ms:
        if m["winner"]:
            counts[m["winner"]] = counts.get(m["winner"], 0) + 1
    out["_counts"] = counts
    for f, c in counts.items():
        out["share_" + f] = 100 * c / n
    b = [sum(1 for m in ms if m["base"] == i) for i in (0, 1)]
    out["base0_share"], out["base1_share"] = 100 * b[0] / n, 100 * b[1] / n
    out["_bases"] = b
    out["base_max"] = max(out["base0_share"], out["base1_share"])
    L = sorted(m["len_min"] for m in ms)
    out["len_min"], out["len_max"] = L[0], L[-1]
    out["len_median"] = (L[(n - 1) // 2] + L[n // 2]) / 2
    he = [m["he"] for m in ms]
    out["he_mean"], out["he_max"] = sum(he) / n, max(he)
    return out

def ci_of(metric, met):
    """95 % Wilson interval for share-type metrics, else None."""
    n = met["n"]
    if metric.startswith("share_"):
        return wilson(met["_counts"].get(metric[6:], 0), n)
    if metric in ("base0_share", "base1_share"):
        return wilson(met["_bases"][int(metric[4])], n)
    if metric == "base_max":
        return wilson(max(met["_bases"]), n)
    return None

class Target:
    def __init__(self, d):
        self.metric, self.w = d["metric"], d.get("weight", 1.0)
        self.lo, self.hi = d.get("lower"), d.get("upper")
        self.slo, self.shi = d.get("screen_lower", self.lo), d.get("screen_upper", self.hi)

    def bounds(self, stage):
        return (self.slo, self.shi) if stage == "screen" else (self.lo, self.hi)

def evaluate(targets, met, stage):
    """-> (loss, rows). loss is 0 inside every band; squared, scaled violations
    plus a weak pull to the middle of two-sided bands (keeps the loss smooth)."""
    loss, rows = 0.0, []
    for t in targets:
        lo, hi = t.bounds(stage)
        v = met.get(t.metric)
        if v is None:
            v = 0.0           # e.g. a faction that never won
        scale = max(1e-9, ((hi - lo) / 2) if (lo is not None and hi is not None) else
                    (abs(hi if hi is not None else lo) or 1) * 0.1)
        viol = max(0.0, (lo - v) if lo is not None else 0.0, (v - hi) if hi is not None else 0.0)
        l = t.w * (viol / scale) ** 2
        if lo is not None and hi is not None:
            l += 0.05 * t.w * (((v - (lo + hi) / 2) / scale)) ** 2
        loss += l
        ci = ci_of(t.metric, met)
        ok = viol == 0
        if ci is None:
            status = "pass" if ok else "FAIL"
        else:
            clear = (hi is not None and ci[0] > hi) or (lo is not None and ci[1] < lo)
            inside = (lo is None or ci[0] >= lo) and (hi is None or ci[1] <= hi)
            status = "pass (CI inside)" if inside else ("pass (CI overlaps)" if ok else
                     ("FAIL (clear)" if clear else "miss (CI overlaps)"))
        rows.append({"metric": t.metric, "value": v, "ci": ci, "lo": lo, "hi": hi,
                     "ok": ok, "status": status, "clear_fail": status == "FAIL (clear)" or (ci is None and not ok)})
    return loss, rows

# -------------------------------------------------- GP + expected improvement

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

def _k(a, b, ls):
    return math.exp(-0.5 * sum((p - q) ** 2 for p, q in zip(a, b)) / (ls * ls))

def gp_fit(X, y, noise_frac):
    """RBF GP on [0,1]^d inputs; length-scale picked by marginal likelihood."""
    n = len(y)
    mu = sum(y) / n
    var = max(1e-6, sum((v - mu) ** 2 for v in y) / n)
    best = None
    for ls in (0.12, 0.2, 0.3, 0.5, 0.8):
        K = [[var * _k(X[i], X[j], ls) + (noise_frac * var + 1e-8 if i == j else 0) for j in range(n)] for i in range(n)]
        L = _chol(K)
        a, z = _solve(L, [v - mu for v in y])
        ll = -0.5 * sum((v - mu) * ai for v, ai in zip(y, a)) - sum(math.log(L[i][i]) for i in range(n))
        if best is None or ll > best[0]:
            best = (ll, ls, L, a)
    _, ls, L, a = best
    def predict(x):
        ks = [var * _k(x, xi, ls) for xi in X]
        m = mu + sum(k * ai for k, ai in zip(ks, a))
        _, v = _solve(L, ks)
        s2 = max(1e-12, var - sum(t * t for t in v))
        return m, math.sqrt(s2)
    return predict

def _phi(z): return math.exp(-0.5 * z * z) / math.sqrt(2 * math.pi)
def _Phi(z): return 0.5 * (1 + math.erf(z / math.sqrt(2)))

def expected_improvement(m, s, best):
    if s < 1e-9:
        return 0.0
    z = (best - m) / s
    return (best - m) * _Phi(z) + s * _phi(z)

# -------------------------------------------------------------------- search

def fmt_x(knobs, xs):
    return ", ".join(f"{k.name}={x:g}" for k, x in zip(knobs, xs))

def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("spec")
    ap.add_argument("--cli", default="target/release/frontline-cli")
    ap.add_argument("--out", help="output dir (default: tools/autotune/runs/<spec name>)")
    ap.add_argument("--jobs", type=int, help="override spec jobs")
    a = ap.parse_args()

    spec = parse_toml(open(a.spec).read())
    knobs = [Knob(d) for d in spec["knob"]]
    targets = [Target(d) for d in spec["target"]]
    S = spec.get("seeds", {})
    screen = (S.get("screen_start", 1), S.get("screen_count", 20))
    confirm = (S.get("confirm_start", 1), S.get("confirm_count", 80))
    oos = (S.get("oos_start", 101), S.get("oos_count", 80))
    sr = spec.get("search", {})
    max_screens = sr.get("max_screens", 6)
    n_init = sr.get("n_init", 3)
    confirm_top = sr.get("confirm_top", 1)
    noise_frac = sr.get("noise_frac", 0.15)
    rng = random.Random(sr.get("rng", 7))
    name = os.path.splitext(os.path.basename(a.spec))[0]
    outdir = a.out or os.path.join(os.path.dirname(os.path.abspath(__file__)), "runs", name)
    os.makedirs(outdir, exist_ok=True)
    jobs = a.jobs or spec.get("jobs", 2)
    if not os.path.exists(a.cli):
        raise SystemExit("frontline-cli not found: " + a.cli)

    def log(s):
        print(s, flush=True)
    runner = Runner(spec, a.cli, outdir, jobs, log)

    grids = [k.grid() for k in knobs]
    allpts = [[]]
    for g in grids:
        allpts = [p + [x] for p in allpts for x in g]
    if len(allpts) > 3000:
        allpts = rng.sample(allpts, 3000)
    def unit(xs):
        return [(x - k.lo) / ((k.hi - k.lo) or 1) for k, x in zip(knobs, xs)]

    cands = []   # every evaluated run, in order

    def evaluate_at(xs, stage, rng_):
        ov = build_override(knobs, xs)
        rec, hit = runner.run(ov, rng_[0], rng_[1])
        met = metrics(rec)
        loss, rows = evaluate(targets, met, stage)
        c = {"stage": stage, "xs": list(xs), "met": met, "loss": loss, "rows": rows,
             "override": ov, "wall_s": rec["wall_s"], "cached": hit}
        cands.append(c)
        log(f"[{stage}] {fmt_x(knobs, xs)}  loss {loss:.3f}  "
            + "  ".join(f"{r['metric']}={r['value']:.1f}" for r in rows if r['metric'].startswith('share_'))
            + ("  (cached)" if hit else f"  ({rec['wall_s']:.0f}s)"))
        return c

    # ---- screening on the small seed set (common random numbers: same seeds)
    screened = {}
    def screen_at(xs):
        t = tuple(xs)
        if t not in screened:
            screened[t] = evaluate_at(xs, "screen", screen)
        return screened[t]

    init = []
    if len(knobs) == 1:
        g = grids[0]
        init = [[g[0]], [g[-1]], [g[len(g) // 2]]]
    else:
        init = [[g[0] for g in grids], [g[-1] for g in grids]]
    while len(init) < n_init:
        init.append(rng.choice(allpts))
    for xs in init[:max_screens]:
        screen_at(xs)
    while len(screened) < max_screens:
        pts = list(screened.values())
        pred = gp_fit([unit(c["xs"]) for c in pts], [c["loss"] for c in pts], noise_frac)
        best = min(c["loss"] for c in pts)
        todo = [p for p in allpts if tuple(p) not in screened]
        if not todo:
            break
        scored = []
        for p in todo:
            m, s = pred(unit(p))
            scored.append((expected_improvement(m, s, best), m, p))
        scored.sort(key=lambda t: (-t[0], t[1]))
        if scored[0][0] < 1e-4 and best == 0.0 and len(screened) >= n_init + 1:
            log("EI negligible and a candidate already passes the screen; stopping the search")
            break
        screen_at(scored[0][2])

    # ---- confirm the most promising screens on the big set, then out-of-sample
    ranked = sorted(screened.values(), key=lambda c: c["loss"])
    confirmed = []
    for c in ranked[:confirm_top]:
        confirmed.append(evaluate_at(c["xs"], "confirm", confirm))
    confirmed.sort(key=lambda c: c["loss"])
    chosen = confirmed[0]
    oos_c = evaluate_at(chosen["xs"], "oos", oos)

    write_report(outdir, name, spec, knobs, targets, cands, chosen, oos_c, a.cli, screen, confirm, oos)
    open(os.path.join(outdir, "best_override.toml"), "w").write(
        f"# autotune {name}: {fmt_x(knobs, chosen['xs'])}\n# see report.md for the evidence and its error bars\n" + chosen["override"])
    log(f"report: {os.path.join(outdir, 'report.md')}")

# -------------------------------------------------------------------- report

def cell_ci(r):
    if r["ci"]:
        return f"{r['value']:.1f} [{r['ci'][0]:.0f}-{r['ci'][1]:.0f}]"
    return f"{r['value']:.1f}"

def write_report(outdir, name, spec, knobs, targets, cands, chosen, oos_c, cli, screen, confirm, oos):
    L = [f"# Autotune report: {name}", "",
         spec.get("description", ""), "",
         f"CLI `{cli}`, {spec.get('minutes', 45)} min limit. Seed sets (each seed plays twice, normal and swapped): "
         f"screen seeds {screen[0]}..{screen[0]+screen[1]-1} ({2*screen[1]} matches), "
         f"confirm {confirm[0]}..{confirm[0]+confirm[1]-1} ({2*confirm[1]}), "
         f"out of sample {oos[0]}..{oos[0]+oos[1]-1} ({2*oos[1]}). "
         "Every candidate runs the same seeds (common random numbers). "
         "Bracketed ranges are 95 % Wilson intervals on the binomial count; they treat matches as independent, "
         "which is conservative for the swapped pairs.", "",
         "Targets (screen bands in the search may be wider):", ""]
    for t in targets:
        L.append(f"- `{t.metric}`: {t.lo if t.lo is not None else '-'} .. {t.hi if t.hi is not None else '-'}"
                 f" (screen {t.slo if t.slo is not None else '-'} .. {t.shi if t.shi is not None else '-'}), weight {t.w:g}")
    metrics_cols = [t.metric for t in targets]
    L += ["", "## Every run", "",
          "| stage | " + " | ".join(k.name for k in knobs) + " | n | loss | " + " | ".join(metrics_cols) + " | tick p99 | verdict |",
          "|---|" + "---|" * (len(knobs) + 4 + len(metrics_cols))]
    for c in cands:
        bad = [r["metric"] for r in c["rows"] if not r["ok"]]
        verdict = "all in band" if not bad else "out: " + ", ".join(bad)
        tick = c["met"]["tick_p99"]
        L.append(f"| {c['stage']} | " + " | ".join(f"{x:g}" for x in c["xs"]) +
                 f" | {c['met']['n']} | {c['loss']:.2f} | " + " | ".join(cell_ci(r) for r in c["rows"]) +
                 f" | {tick if tick is not None else '-'} | {verdict} |")
    L += ["", "## Chosen", "", f"**{fmt_x(knobs, chosen['xs'])}**", "",
          "Override (also in `best_override.toml`):", "", "```toml", chosen["override"].rstrip(), "```", "",
          "| set | matches | " + " | ".join(r["metric"] for r in chosen["rows"]) + " | tick p99 |",
          "|---|---|" + "---|" * (len(chosen["rows"]) + 1)]
    for lab, c in (("confirm", chosen), ("out of sample", oos_c)):
        L.append(f"| {lab} | {c['met']['n']} | " + " | ".join(cell_ci(r) for r in c["rows"]) +
                 f" | {c['met']['tick_p99']} |")
    L += ["", "Per target status (confirm / out of sample):", ""]
    for r1, r2 in zip(chosen["rows"], oos_c["rows"]):
        b = f"{r1['lo'] if r1['lo'] is not None else '-'}..{r1['hi'] if r1['hi'] is not None else '-'}"
        L.append(f"- `{r1['metric']}` band {b}: {r1['status']} / {r2['status']}")
    both = all(r["ok"] for r in chosen["rows"]) and all(r["ok"] for r in oos_c["rows"])
    clear = [r["metric"] for r in chosen["rows"] + oos_c["rows"] if r["clear_fail"]]
    L += ["", "## Verdict", ""]
    if both:
        L.append("Every target is met by the point estimate on both the confirm and the out-of-sample set. "
                 "A faction share measured on 160 matches has a standard error of about 4 points (about 6 wins), "
                 "so the true value is only known to within the bracketed interval; treat the chosen level as "
                 "\"not distinguishable from the band\", and prefer the middle of the band when several levels pass.")
    elif clear:
        L.append("**Not found.** A target clearly fails (its 95 % interval lies outside the band): " + ", ".join(sorted(set(clear))) +
                 ". Widen the ranges, add a knob, or accept a lower tier of target; the tuner will not invent knobs.")
    else:
        L.append("**Not proven.** No target clearly fails, but at least one point estimate is outside its band "
                 "with an interval that overlaps it. More seeds (or a different level) are needed to say.")
    L += ["", f"Search effort: {sum(1 for c in cands if c['stage']=='screen')} screens, "
          f"{sum(1 for c in cands if c['stage']=='confirm')} confirms, 1 out-of-sample run; "
          f"{sum(c['wall_s'] for c in cands if not c['cached']):.0f} s of new match time "
          f"({sum(1 for c in cands if c['cached'])} runs from cache)."]
    open(os.path.join(outdir, "report.md"), "w").write("\n".join(L) + "\n")

if __name__ == "__main__":
    main()
