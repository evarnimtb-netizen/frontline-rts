#!/usr/bin/env python3
"""Local automatic balance tuner (docs/modding/autotune.md).

Solves for 1-3 TOML knobs inside ranges somebody chose: runs AI-vs-AI batches
with frontline-cli on fresh seeds, fits every match of every level at once
(logistic for win shares, least squares for other metrics), places the next
batch where it narrows the interval on the solved level, and writes a report
and the chosen override file.

    python3 -I tools/autotune/autotune.py SPEC.toml --cli target/release/frontline-cli

Standard library only.
"""
import argparse, hashlib, json, math, os, re, subprocess, time

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
        self.tol = d.get("tol")     # stop when the level is known to +- this
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

def _short(faction):
    return faction.split(":")[-1]

def parse_summary(text):
    """Everything the tuner reads from a batch lives here: the file written by
    `skirmish --seeds ... --json FILE` (docs/modding/tuning.md). Per match:
    seed, bases, winner, the winner's base, length (min), HE share of kills
    (%). Per faction, batch totals: wins, kills, losses, kill shares by source
    (% of its kills). Also the tick p99 peak (ms)."""
    d = json.loads(text)
    matches = []
    for m in d["per_match"]:
        w = m["winner_faction"]
        matches.append({"seed": m["seed"], "bases": m["side"],
                        "winner": _short(w) if w else None,
                        "base": m["winner_base"],
                        "len_min": m["length_s"] / 60,
                        "he": m["he_share_pct"]})
    sides = {}
    for fac, f in d["factions"].items():
        sides[fac] = {"wins": f["wins"], "kills": f["kills"], "losses": f["losses"],
                      "kill_src": {k: v for k, v in f["kills_by_source_pct"].items() if k != "other"}}
    return {"matches": matches, "tick_p99": d["tick_p99_ms"]["peak"], "sides": sides}

class Runner:
    def __init__(self, spec, cli, outdir, jobs):
        self.cli, self.outdir, self.jobs = cli, outdir, jobs
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
        jpath = os.path.join(self.outdir, "overrides", key + ".json")
        if os.path.exists(jpath):
            os.remove(jpath)
        cmd = [self.cli, "skirmish", "--ai-vs-ai", "--seed", str(seed), "--seeds", str(seeds),
               "--both-bases", "--summary", "--jobs", str(self.jobs), "--minutes", str(self.minutes),
               "--json", jpath]
        if override.strip():
            cmd += ["--rules-override", path]
        t0 = time.time()
        p = subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
        parsed = None
        if p.returncode == 0 and os.path.exists(jpath):
            parsed = parse_summary(open(jpath).read())
        if parsed is None or len(parsed["matches"]) != 2 * seeds:
            n = len(parsed["matches"]) if parsed else 0
            raise SystemExit(f"batch failed (rc={p.returncode}, {n} of {2*seeds} matches):\n{p.stderr[-800:]}")
        os.remove(jpath)
        rec = {"key": key, "seed": seed, "seeds": seeds, "override": override, **parsed,
               "wall_s": round(time.time() - t0, 1)}
        self.cache[key] = rec
        with open(self.cachefile, "a") as f:
            f.write(json.dumps(rec) + "\n")
        return rec, False

# ---------------------------------------------------------------- statistics

Z = 1.96

def wilson(k, n, z=Z):
    if n == 0:
        return 0.0, 100.0
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return 100 * (c - h), 100 * (c + h)

def _dot(a, b):
    return sum(x * y for x, y in zip(a, b))

def _quad(S, a):
    return sum(a[i] * S[i][j] * a[j] for i in range(len(a)) for j in range(len(a)))

def _sig(x):
    return 1 / (1 + math.exp(-max(-40.0, min(40.0, x))))

def _logit(p):
    p = min(max(p, 1e-6), 1 - 1e-6)
    return math.log(p / (1 - p))

def _inv(A):
    """Inverse of a small matrix (Gauss-Jordan, partial pivoting)."""
    n = len(A)
    M = [list(r) + [1.0 if i == j else 0.0 for j in range(n)] for i, r in enumerate(A)]
    for c in range(n):
        p = max(range(c, n), key=lambda r: abs(M[r][c]))
        M[c], M[p] = M[p], M[c]
        if abs(M[c][c]) < 1e-12:
            raise ValueError("singular design")
        d = M[c][c]
        M[c] = [v / d for v in M[c]]
        for r in range(n):
            if r != c and M[r][c]:
                f = M[r][c]
                M[r] = [a - f * b for a, b in zip(M[r], M[c])]
    return [r[n:] for r in M]

def fit(rows, logit, ridge=1e-4):
    """Maximum likelihood fit of y ~ a.beta, logistic (y is 0/1) or weighted
    least squares. rows: (a, y, weight, cluster). Newton steps (IRLS) on the
    rows grouped by a. Returns (beta, cov). cov is cluster-robust (sandwich
    over clusters: a seed's normal and swapped match are one cluster, with a
    G/(G-1) factor) when there are at least 4 clusters per coefficient, else
    the model-based one (least squares: scaled by the residual variance)."""
    k = len(rows[0][0])
    groups = {}
    for a, y, w, _ in rows:
        g = groups.setdefault(tuple(a), [0.0, 0.0])
        g[0] += w
        g[1] += w * y
    beta = [0.0] * k

    def hess_grad():
        H = [[0.0] * k for _ in range(k)]
        g = [0.0] * k
        for a, (sw, swy) in groups.items():
            if logit:
                p = _sig(_dot(a, beta))
                wt, r = sw * p * (1 - p), swy - sw * p
            else:
                wt, r = sw, swy - sw * _dot(a, beta)
            for i in range(k):
                g[i] += a[i] * r
                for j in range(k):
                    H[i][j] += wt * a[i] * a[j]
        for i in range(1, k if logit else 1):     # a whisker of ridge against separation
            H[i][i] += ridge
            g[i] -= ridge * beta[i]
        return _inv(H), g

    for _ in range(60 if logit else 2):
        Hi, g = hess_grad()
        step = [_dot(Hi[i], g) for i in range(k)]
        beta = [b + s for b, s in zip(beta, step)]
        if max(abs(s) for s in step) < 1e-10:
            break
    Hi, _ = hess_grad()
    mu = lambda a: _sig(_dot(a, beta)) if logit else _dot(a, beta)
    scores = {}
    rss = 0.0
    for a, y, w, c in rows:
        r = y - mu(a)
        s = scores.setdefault(c, [0.0] * k)
        for i in range(k):
            s[i] += w * r * a[i]
        rss += w * r * r
    G = len(scores)
    if G >= 4 * k:
        M = [[sum(s[i] * s[j] for s in scores.values()) for j in range(k)] for i in range(k)]
        f = G / (G - 1)
        cov = [[f * sum(Hi[i][a] * M[a][b] * Hi[b][j] for a in range(k) for b in range(k))
                for j in range(k)] for i in range(k)]
    elif logit:
        cov = Hi
    else:
        n = len(rows)
        s2 = rss / (n - k) if n > k else 1e12      # weights only need to be relative
        cov = [[v * s2 for v in r] for r in Hi]
    return beta, cov

def crossing(beta, cov, a0, j, L, z=Z):
    """Fieller interval: the u (coordinate j of a, the rest as in a0) where
    a.beta = L is not rejected at 95 %. -> (point, lo, hi); lo and hi are
    None when the interval is unbounded (the slope is not clearly non-zero)."""
    a0 = list(a0)
    a0[j] = 0.0
    m0, b = _dot(a0, beta) - L, beta[j]
    Sa = [_dot(r, a0) for r in cov]
    v00, v0j, vjj = _dot(a0, Sa), Sa[j], cov[j][j]
    point = -m0 / b if b else None
    A = b * b - z * z * vjj
    if point is None or A <= 0:
        return point, None, None
    B = 2 * b * m0 - 2 * z * z * v0j
    C = m0 * m0 - z * z * v00
    d = math.sqrt(max(0.0, B * B - 4 * A * C))
    return point, (-B - d) / (2 * A), (-B + d) / (2 * A)

def chi2_sf(x, df):
    """Upper tail of chi-square (Wilson-Hilferty; good enough for a warning)."""
    if df <= 0:
        return None
    h = 2 / (9 * df)
    zz = ((x / df) ** (1 / 3) - (1 - h)) / math.sqrt(h)
    return 0.5 * math.erfc(zz / math.sqrt(2))

# ---------------------------------------------------------------- targets

def kill_metric(metric):
    """'kill_tank_he_germany' -> ('tank_he', 'germany'), else None."""
    if not metric.startswith("kill_"):
        return None
    src, _, fac = metric[5:].rpartition("_")
    return (src, fac) if src else None

def modellable(metric):
    return metric.startswith("share_") or metric in ("he_mean", "len_mean") or kill_metric(metric) is not None

class Target:
    def __init__(self, d):
        self.metric, self.w = d["metric"], d.get("weight", 1.0)
        self.lo, self.hi = d.get("lower"), d.get("upper")
        self.model = (modellable(self.metric) and d.get("role", "model") != "check"
                      and (self.lo is not None or self.hi is not None))   # no band: reported only
        self.logit = self.metric.startswith("share_")

    def aim(self):
        if self.lo is not None and self.hi is not None:
            return (self.lo + self.hi) / 2
        return self.lo if self.lo is not None else self.hi

    def scale(self):
        lo, hi = self.lo, self.hi
        return max(1e-9, (hi - lo) / 2 if (lo is not None and hi is not None) else
                   (abs(hi if hi is not None else lo) or 1) * 0.1)

    def loss(self, v):
        """0 inside the band, squared scaled distance outside, plus a weak pull
        to the middle of a two-sided band."""
        lo, hi, s = self.lo, self.hi, self.scale()
        viol = max(0.0, (lo - v) if lo is not None else 0.0, (v - hi) if hi is not None else 0.0)
        l = (viol / s) ** 2
        if lo is not None and hi is not None:
            l += 0.05 * ((v - (lo + hi) / 2) / s) ** 2
        return self.w * l

    def inside(self, v):
        return (self.lo is None or v >= self.lo) and (self.hi is None or v <= self.hi)

    def band(self):
        f = lambda v: "-" if v is None else f"{v:g}"
        return f"{f(self.lo)}..{f(self.hi)}"

def unit(knobs, xs):
    return [(x - k.lo) / ((k.hi - k.lo) or 1) for k, x in zip(knobs, xs)]

def from_unit(knobs, us):
    return [k.lo + u * (k.hi - k.lo) for k, u in zip(knobs, us)]

def normal_base(fac, batches):
    """The base faction `fac` holds in a 'normal' match, read off any decisive match."""
    for b in batches:
        for m in b["rec"]["matches"]:
            if m["winner"] and m["base"] is not None:
                fb = m["base"] if m["winner"] == fac else 1 - m["base"]
                return fb if m["bases"] == "normal" else 1 - fb
    return None

class Model:
    """One modelled target: logistic in the knobs (+ a base-side term) for a
    faction's share, per match; least squares per match for he_mean/len_mean,
    per batch (weighted by kills) for kill_<source>_<faction>."""

    def __init__(self, t, knobs, batches):
        self.t, self.k = t, len(knobs)
        self.side = t.logit
        rows = []
        if t.logit:
            fac = t.metric[6:]
            nb = normal_base(fac, batches)
            for bi, b in enumerate(batches):
                u = unit(knobs, b["xs"])
                for m in b["rec"]["matches"]:
                    s = 0.0
                    if nb is not None:
                        s = 0.5 if (nb if m["bases"] == "normal" else 1 - nb) == 0 else -0.5
                    rows.append(([1.0] + u + [s], 1.0 if m["winner"] == fac else 0.0, 1.0, (bi, m["seed"])))
        elif t.metric in ("he_mean", "len_mean"):
            key = "he" if t.metric == "he_mean" else "len_min"
            for bi, b in enumerate(batches):
                u = unit(knobs, b["xs"])
                for m in b["rec"]["matches"]:
                    rows.append(([1.0] + u, m[key], 1.0, (bi, m["seed"])))
        else:
            src, fac = kill_metric(t.metric)
            for bi, b in enumerate(batches):
                side = b["rec"].get("sides", {}).get(fac, {})
                if side.get("kills") and "kill_src" in side:
                    rows.append(([1.0] + unit(knobs, b["xs"]), side["kill_src"].get(src, 0.0),
                                 float(side["kills"]), bi))
        self.rows, self.n = rows, len(rows)
        try:
            self.beta, self.cov = fit(rows, t.logit)
            self.ok = True
        except (ValueError, IndexError, ZeroDivisionError):
            self.ok = False
        # identified: some knob's slope is clearly not zero
        self.identified = self.ok and any(
            abs(self.beta[1 + j]) > Z * math.sqrt(max(self.cov[1 + j][1 + j], 0.0)) for j in range(self.k))

    def a(self, us):
        return [1.0] + list(us) + ([0.0] if self.side else [])

    def predict(self, us):
        """-> (value, lo, hi) at unit coordinates us (both bases averaged)."""
        a = self.a(us)
        m = _dot(a, self.beta)
        se = math.sqrt(max(0.0, _quad(self.cov, a)))
        vals = (m, m - Z * se, m + Z * se)
        return tuple(100 * _sig(v) for v in vals) if self.t.logit else vals

    def level(self, us, j, value):
        """Fieller interval (unit coordinates) of knob j where the prediction equals value."""
        L = _logit(value / 100) if self.t.logit else value
        return crossing(self.beta, self.cov, self.a(us), 1 + j, L)

    def slope(self, j, du):
        """Change of the prediction (metric units, at 50 % for shares) for a knob move du (unit)."""
        b, se = self.beta[1 + j], math.sqrt(max(self.cov[1 + j][1 + j], 0.0))
        f = 25.0 if self.t.logit else 1.0     # dp/deta = 1/4 at p = 0.5, in points
        return f * b * du, f * (b - Z * se) * du, f * (b + Z * se) * du

    def side_effect(self):
        """Base-0 minus base-1 share (points, at 50 %) with its interval."""
        if not self.side:
            return None
        i = 1 + self.k
        b, se = self.beta[i], math.sqrt(max(self.cov[i][i], 0.0))
        return 25 * b, 25 * (b - Z * se), 25 * (b + Z * se)

    def lack_of_fit(self):
        """Pearson chi-square of the per-level win counts against the fit -> (X2, df, p)."""
        if not self.t.logit or not self.ok:
            return None
        g = {}
        for a, y, w, _ in self.rows:
            p = _sig(_dot(a, self.beta))
            s = g.setdefault(tuple(a[1:1 + self.k]), [0.0, 0.0, 0.0])
            s[0] += y; s[1] += p; s[2] += p * (1 - p)
        x2 = sum((o - e) ** 2 / v for o, e, v in g.values() if v > 0)
        df = len(g) - (self.k + 1)
        return x2, df, chi2_sf(x2, df)

# ---------------------------------------------------------------- analysis

class State:
    pass

def snap(knobs, xs):
    return [min(k.grid(), key=lambda g: abs(g - x)) for k, x in zip(knobs, xs)]

def knob_tol(k):
    return k.tol if k.tol is not None else (k.step or (k.hi - k.lo) / 20)

def analyse(knobs, targets, batches):
    st = State()
    st.models = [Model(t, knobs, batches) for t in targets if t.model]
    good = [m for m in st.models if m.ok]
    nk = len(knobs)
    n = {1: 301, 2: 41}.get(nk, 15)
    pts = [[]]
    for _ in knobs:
        pts = [p + [i / (n - 1)] for p in pts for i in range(n)]

    def ploss(u):
        l = sum(m.t.loss(m.predict(u)[0]) for m in good)
        return l + (1e-3 * sum((x - 0.5) ** 2 for x in u) if nk > 1 else 0.0)
    st.u_hat = min(pts, key=ploss) if good else [0.5] * nk
    st.x_hat = from_unit(knobs, st.u_hat)
    st.chosen = snap(knobs, st.x_hat)
    uc = unit(knobs, st.chosen)
    st.rows = []
    done = True
    for m in st.models:
        r = {"m": m, "t": m.t, "pred": m.predict(uc) if m.ok else None, "levels": []}
        if m.ok:
            for j, k in enumerate(knobs):
                pt, lo, hi = m.level(st.u_hat, j, m.t.aim())
                f = lambda u: None if u is None else k.lo + u * (k.hi - k.lo)
                r["levels"].append((f(pt), f(lo), f(hi)))
        p = r["pred"]
        if p is None:
            r["state"] = "no fit"
        elif m.t.inside(p[1]) and m.t.inside(p[2]):
            r["state"] = "in band (CI inside)"
        elif (m.t.hi is not None and p[1] > m.t.hi) or (m.t.lo is not None and p[2] < m.t.lo):
            r["state"] = "out of band (clear)"
        elif m.t.lo is not None and m.t.hi is not None and any(
                lo is not None and k.lo <= pt <= k.hi and (hi - lo) / 2 <= knob_tol(k)
                for k, (pt, lo, hi) in zip(knobs, r["levels"])):
            r["state"] = "level resolved"
        else:
            r["state"] = "open"
        done = done and r["state"] != "open" and r["state"] != "no fit"
        st.rows.append(r)
    st.done = done
    return st

def checks(knobs, targets, batches, chosen):
    """Every target measured on the matches played near the chosen level
    (all knobs within one tolerance), with Wilson intervals on shares."""
    near = [b for b in batches if all(abs(x - c) <= knob_tol(k) * 1.0001
                                      for k, x, c in zip(knobs, b["xs"], chosen))]
    if not near:
        d = lambda b: sum(((x - c) / ((k.hi - k.lo) or 1)) ** 2 for k, x, c in zip(knobs, b["xs"], chosen))
        best = min(d(b) for b in batches)
        near = [b for b in batches if d(b) == best]
    met = metrics(near)
    rows = []
    for t in targets:
        v = met.get(t.metric)
        if v is None:
            v = 0.0 if t.metric.startswith("share_") else float("nan")
        ci = ci_of(t.metric, met)
        ok = v == v and t.inside(v)
        if ci is None:
            status = "pass" if ok else "FAIL"
        else:
            clear = (t.hi is not None and ci[0] > t.hi) or (t.lo is not None and ci[1] < t.lo)
            inside = t.inside(ci[0]) and t.inside(ci[1])
            status = "pass (CI inside)" if inside else ("pass (CI overlaps)" if ok else
                     ("FAIL (clear)" if clear else "miss (CI overlaps)"))
        rows.append({"t": t, "value": v, "ci": ci, "status": status,
                     "fail": status.startswith("FAIL")})
    return met, rows, near

def metrics(batches):
    ms = [m for b in batches for m in b["rec"]["matches"]]
    n = len(ms)
    out = {"n": n, "tick_p99": max([b["rec"].get("tick_p99") or 0 for b in batches] or [0]) or None}
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
    out["len_mean"] = sum(L) / n
    he = [m["he"] for m in ms]
    out["he_mean"], out["he_max"] = sum(he) / n, max(he)
    kills = {}
    for bt in batches:                      # kill shares: kill-weighted over the batches
        for fac, s in bt["rec"].get("sides", {}).items():
            for src, v in s.get("kill_src", {}).items():
                acc = kills.setdefault((src, fac), [0.0, 0.0])
                acc[0] += v * s.get("kills", 0)
                acc[1] += s.get("kills", 0)
    for (src, fac), (num, den) in kills.items():
        if den:
            out[f"kill_{src}_{fac}"] = num / den
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

# ------------------------------------------------------------------ design

def init_design(knobs):
    mid = snap(knobs, [(k.lo + k.hi) / 2 for k in knobs])
    if len(knobs) == 1:
        return [[knobs[0].lo], [knobs[0].hi], mid]
    out = [mid]
    for j, k in enumerate(knobs):
        for end in (k.lo, k.hi):
            xs = list(mid)
            xs[j] = end
            out.append(xs)
    return out

def next_point(knobs, st, batches, rnd):
    """Sequential design. Until the first modelled target's slope is clearly
    non-zero, add batches to the initial design (fewest matches first). Then
    cycle: the solved level, a flank below it, the solved level, a flank above;
    flanks sit where the prediction moves by about 15 points of share (or
    1.5 half-bands), so the slope stays estimable while most matches land
    near the level that is being pinned down."""
    played = lambda xs: sum(2 * b["seeds"] for b in batches if b["xs"] == list(xs))
    prim = st.models[0] if st.models else None
    if prim is None or not prim.identified:
        return min(init_design(knobs), key=played)
    j, phase = (rnd // 4) % len(knobs), rnd % 4
    centre = snap(knobs, st.x_hat)
    if phase in (0, 2):
        return centre
    t = prim.t
    if t.logit:
        want = 0.62                      # logit(0.65): +-15 points around 50 %
    elif t.lo is not None and t.hi is not None:
        want = 1.5 * (t.hi - t.lo) / 2
    else:
        want = 0.15 * (abs(t.aim()) or 1)
    k = knobs[j]
    span = (k.hi - k.lo) or 1
    h = min(0.5, max(knob_tol(k) / span, want / max(abs(prim.beta[1 + j]), 1e-9)))
    for sgn in ((-1, 1) if phase == 1 else (1, -1)):
        u = list(st.u_hat)
        u[j] = min(1.0, max(0.0, u[j] + sgn * h))
        xs = snap(knobs, from_unit(knobs, u))
        if xs != centre:
            return xs
    return centre

def search(knobs, targets, opt, run_batch, log=lambda s: None):
    """Run batches until every modelled target is settled or the budget is
    spent. run_batch(xs, seed, seeds) -> (record, cached). Every batch plays
    fresh seeds, so every match is a new observation."""
    batches = []
    seed = [opt.get("seed_start", 1)]
    nb = opt.get("batch_seeds", 20)
    used = lambda: sum(2 * b["seeds"] for b in batches)

    def run(xs):
        rec, hit = run_batch(xs, seed[0], nb)
        batches.append({"xs": list(xs), "seed": seed[0], "seeds": nb, "rec": rec, "cached": hit})
        seed[0] += nb
        ms = rec["matches"]
        log(f"[batch {len(batches)}] {fmt_x(knobs, xs)}  seeds {batches[-1]['seed']}..{seed[0]-1}  "
            + "  ".join(f"{t.metric}={metrics([batches[-1]]).get(t.metric, 0):.1f}"
                        for t in targets if t.model)
            + ("  (cached)" if hit else f"  ({rec.get('wall_s', 0):.0f}s)"))

    for xs in init_design(knobs):
        run(xs)
    rnd = 0
    while True:
        st = analyse(knobs, targets, batches)
        log(f"  fit after {used()} matches: {fmt_x(knobs, st.x_hat)}; "
            + "; ".join(f"{r['t'].metric} {r['state']}" for r in st.rows))
        if st.done and used() >= opt.get("min_matches", 0):
            st.stop = "settled"
            break
        if used() + 2 * nb > opt.get("max_matches", 800):
            st.stop = "budget"
            break
        run(next_point(knobs, st, batches, rnd))
        rnd += 1
    st.batches, st.used = batches, used()
    st.check_met, st.checks, st.near = checks(knobs, targets, batches, st.chosen)
    st.verdict = verdict(st)
    return st

def verdict(st):
    fails = [r["t"].metric for r in st.checks if r["fail"]]
    states = [r["state"] for r in st.rows]
    if "out of band (clear)" in states:
        return "not found"
    if fails:
        return "check fails"
    if states and all(s == "in band (CI inside)" for s in states):
        return "in band"
    if states and all(s in ("in band (CI inside)", "level resolved") for s in states):
        return "level resolved"
    return "not resolved"

# -------------------------------------------------------------------- main

def fmt_x(knobs, xs):
    return ", ".join(f"{k.name}={x:.4g}" for k, x in zip(knobs, xs))

def default_cli():
    root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
    for base in (os.getcwd(), root):
        for prof in ("release", "bench-release"):
            p = os.path.join(base, "target", prof, "frontline-cli")
            if os.path.exists(p):
                return p
    return "target/release/frontline-cli"

def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("spec")
    ap.add_argument("--cli", help="frontline-cli binary (default: target/release, else target/bench-release)")
    ap.add_argument("--out", help="output dir (default: tools/autotune/runs/<spec name>)")
    ap.add_argument("--jobs", type=int, help="parallel matches (default: spec jobs, else 6)")
    a = ap.parse_args()

    spec = parse_toml(open(a.spec).read())
    knobs = [Knob(d) for d in spec["knob"]]
    targets = [Target(d) for d in spec["target"]]
    opt = spec.get("search", {})
    name = os.path.splitext(os.path.basename(a.spec))[0]
    outdir = a.out or os.path.join(os.path.dirname(os.path.abspath(__file__)), "runs", name)
    os.makedirs(outdir, exist_ok=True)
    jobs = a.jobs or spec.get("jobs", 6)
    cli = a.cli or default_cli()
    if not os.path.exists(cli):
        raise SystemExit("frontline-cli not found: " + cli)
    if not any(t.model for t in targets):
        raise SystemExit("no modelled target (share_*, he_mean, len_mean, kill_*): nothing to solve")
    runner = Runner(spec, cli, outdir, jobs)

    def run_batch(xs, seed, seeds):
        return runner.run(build_override(knobs, xs), seed, seeds)
    log = lambda s: print(s, flush=True)
    t0 = time.time()
    st = search(knobs, targets, opt, run_batch, log)
    st.wall = time.time() - t0
    st.override = build_override(knobs, st.chosen)
    write_report(outdir, name, spec, knobs, targets, st, cli, jobs)
    open(os.path.join(outdir, "best_override.toml"), "w").write(
        f"# autotune {name}: {fmt_x(knobs, st.chosen)} ({st.verdict})\n"
        "# see report.md for the evidence and its error bars\n" + st.override)
    log(f"verdict: {st.verdict}; report: {os.path.join(outdir, 'report.md')}")

# -------------------------------------------------------------------- report

def _r(v, d=1):
    return "-" if v is None else f"{v:.{d}f}"

def write_report(outdir, name, spec, knobs, targets, st, cli, jobs):
    mods = [r for r in st.rows]
    shares = [t.metric for t in targets if t.metric.startswith("share_")]
    L = [f"# Autotune report: {name}", "", spec.get("description", "").strip(), "",
         f"CLI `{cli}`, `--jobs {jobs}`, {spec.get('minutes', 45)} min limit. Every batch plays fresh seeds "
         "(each seed twice, normal and swapped bases), so every match is an independent observation "
         "apart from its swapped twin, which the intervals allow for (cluster-robust). "
         "Modelled targets are fitted over all matches of all levels at once: a faction's share by a "
         "logistic model in the knobs plus a base-side term, other modelled metrics by least squares. "
         "All intervals are 95 %.", "", "Targets:", ""]
    for t in targets:
        L.append(f"- `{t.metric}` {t.band()}, " + ("modelled" if t.model else "checked near the chosen level")
                 + (f", weight {t.w:g}" if t.model else ""))
    L += ["", "## Batches", "",
          "| # | " + " | ".join(k.name for k in knobs) + " | seeds | n | " + " | ".join(shares)
          + " | he_mean | len_median | wall s |",
          "|---|" + "---|" * (len(knobs) + 6 + len(shares))]
    for i, b in enumerate(st.batches):
        met = metrics([b])
        L.append(f"| {i+1} | " + " | ".join(f"{x:g}" for x in b["xs"])
                 + f" | {b['seed']}..{b['seed']+b['seeds']-1} | {met['n']} | "
                 + " | ".join(f"{met.get(s, 0):.1f} [{ci_of(s, met)[0]:.0f}-{ci_of(s, met)[1]:.0f}]" for s in shares)
                 + f" | {met['he_mean']:.1f} | {met['len_median']:.1f} | "
                 + ("cache" if b["cached"] else f"{b['rec'].get('wall_s', 0):.0f}") + " |")
    # pooled per level, observed vs model
    levels = {}
    for b in st.batches:
        levels.setdefault(tuple(b["xs"]), []).append(b)
    L += ["", "## Per level: observed (pooled) vs model", "",
          "| " + " | ".join(k.name for k in knobs) + " | n | "
          + " | ".join(f"{r['t'].metric} observed | model" for r in mods) + " |",
          "|" + "---|" * (len(knobs) + 1 + 2 * len(mods))]
    for xs in sorted(levels):
        met = metrics(levels[xs])
        cells = []
        for r in mods:
            m, v = r["m"], met.get(r["t"].metric, 0.0)
            ci = ci_of(r["t"].metric, met)
            cells.append(f"{v:.1f}" + (f" [{ci[0]:.0f}-{ci[1]:.0f}]" if ci else ""))
            p = m.predict(unit(knobs, xs)) if m.ok else None
            cells.append("-" if p is None else f"{p[0]:.1f} [{p[1]:.1f}-{p[2]:.1f}]")
        L.append("| " + " | ".join(f"{x:g}" for x in xs) + f" | {met['n']} | " + " | ".join(cells) + " |")
    L += ["", "## Fit", ""]
    for r in mods:
        m = r["m"]
        if not m.ok:
            L.append(f"- `{m.t.metric}`: no fit ({m.n} observations)")
            continue
        parts = []
        for j, k in enumerate(knobs):
            du = knob_tol(k) / ((k.hi - k.lo) or 1)
            s = m.slope(j, du)
            parts.append(f"{s[0]:+.1f} [{s[1]:+.1f} to {s[2]:+.1f}] {'points' if m.t.logit else 'units'} "
                         f"per {knob_tol(k):g} of {k.name}")
        line = f"- `{m.t.metric}` ({m.n} {'batches' if kill_metric(m.t.metric) else 'matches'}): " + "; ".join(parts)
        se = m.side_effect()
        if se:
            line += f"; base 0 minus base 1: {se[0]:+.1f} [{se[1]:+.1f} to {se[2]:+.1f}] points"
        lf = m.lack_of_fit()
        if lf and lf[1] > 0:
            line += (f"; lack of fit X2 {lf[0]:.1f} on {lf[1]} df, p {lf[2]:.2f}"
                     + (" (**the straight logistic does not fit; read the per-level table**)" if lf[2] < 0.05 else ""))
        L.append(line)
    L += ["", "## Solved level", ""]
    for r in mods:
        for k, (pt, lo, hi) in zip(knobs, r["levels"]):
            iv = (f"{lo:.3f} to {hi:.3f} (+-{(hi-lo)/2:.3f}; tolerance +-{knob_tol(k):g})" if lo is not None
                  else "unbounded (slope not clearly non-zero)")
            L.append(f"- `{r['t'].metric}` = {r['t'].aim():g} at {k.name} = {_r(pt, 3)}, 95 % interval {iv}")
        m = r["m"]
        if m.ok and len(knobs) == 1 and r["t"].lo is not None and r["t"].hi is not None:
            ends = [m.level(st.u_hat, 0, v)[0] for v in (r["t"].lo, r["t"].hi)]
            if None not in ends:
                e = sorted(knobs[0].lo + u * (knobs[0].hi - knobs[0].lo) for u in ends)
                L.append(f"  - the fitted curve is inside {r['t'].band()} for {knobs[0].name} "
                         f"{e[0]:.3f} to {e[1]:.3f}")
    L += ["", f"Best level on the fitted curves: {fmt_x(knobs, st.x_hat)}; on the grid: "
          f"**{fmt_x(knobs, st.chosen)}**", "",
          "Override (also in `best_override.toml`; a demonstration until someone ships it):", "",
          "```toml", st.override.rstrip(), "```", "",
          "Predicted at the chosen level (model, both bases averaged):", ""]
    for r in mods:
        p = r["pred"]
        L.append(f"- `{r['t'].metric}` {r['t'].band()}: " + ("-" if p is None else
                 f"{p[0]:.1f} [{p[1]:.1f}-{p[2]:.1f}]") + f": {r['state']}")
    nn = st.check_met["n"]
    L += ["", f"Every target on the {nn} matches played within one tolerance of the chosen level "
          "(Wilson intervals on shares):", "",
          "| target | band | value | status |", "|---|---|---|---|"]
    for c in st.checks:
        v = "-" if c["value"] != c["value"] else (f"{c['value']:.1f}" + (
            f" [{c['ci'][0]:.0f}-{c['ci'][1]:.0f}]" if c["ci"] else ""))
        L.append(f"| `{c['t'].metric}` | {c['t'].band()} | {v} | {c['status']} |")
    L += ["", "## Verdict", ""]
    V = {
        "in band": "**In band.** Every modelled target's 95 % interval at the chosen level lies inside its band, "
                   "and no check fails.",
        "level resolved": "**Level resolved.** The level that puts each two-sided target at the middle of its band "
                          "is known to within the tolerance and no check fails; the target's own interval still "
                          "overlaps a band edge, so the value is \"not distinguishable from the band\", not proven inside it.",
        "not found": "**Not found.** A modelled target is clearly outside its band at the best level. Widen a range or add a knob you can defend; the tuner will not invent knobs.",
        "check fails": "**Solved, but a check fails:** " + ", ".join(
                           r["t"].metric for r in st.checks if r["fail"]) + ". The modelled targets are settled "
                       "(above); the failing check is measured on the matches near the chosen level only. "
                       "Look at it across levels (Batches table, cache.jsonl) before blaming the knob: "
                       "a minimum or maximum over a few hundred matches fails by a hair more often as the pool grows.",
        "not resolved": "**Not resolved.** The match budget ran out before the level was pinned down; "
                        "raise `max_matches` (matches are free, the run resumes from the cache).",
    }
    L.append(V[st.verdict])
    if st.verdict in ("in band", "level resolved"):
        L.append("")
        L.append("Before shipping, still check what the targets do not cover (armour-test, fortify-test, "
                 "tick p99 from a clean `--jobs 3` run).")
    new = sum(b["rec"].get("wall_s", 0) for b in st.batches if not b["cached"])
    L += ["", f"Effort: {len(st.batches)} batches, {st.used} matches (stopped: {st.stop}); "
          f"{new:.0f} s of new match time, {sum(1 for b in st.batches if b['cached'])} batches from cache."]
    open(os.path.join(outdir, "report.md"), "w").write("\n".join(L) + "\n")

if __name__ == "__main__":
    main()
