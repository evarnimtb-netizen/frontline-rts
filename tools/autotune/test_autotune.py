#!/usr/bin/env python3
"""Unit tests for the tuner's fitter, summary parser and synthetic harness.

    python3 -I tools/autotune/test_autotune.py
"""
import json, math, os, random, subprocess, sys, tempfile, unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import autotune as at  # noqa: E402
import synth  # noqa: E402

def _pm(seed, side, winner, base, secs, he):
    return {"seed": seed, "side": side, "result": "won" if winner else "time_limit",
            "winner_faction": winner, "winner_base": base, "length_s": secs,
            "he_share_pct": he, "hash": "0x0"}

_G_SRC = {"rifle": 58.2, "smg": 1.0, "mg": 10.4, "mortar": 4.7, "tank_ap": 5.7,
          "tank_he": 10.4, "vehicle_mg": 9.4, "rocket": 0.0, "satchel": 0.0,
          "frag": 0.2, "other": 0.1}
_U_SRC = {"rifle": 77.0, "smg": 0.0, "mg": 4.0, "mortar": 3.6, "tank_ap": 5.1,
          "tank_he": 5.1, "vehicle_mg": 4.9, "rocket": 0.2, "satchel": 0.0,
          "frag": 0.0, "other": 0.0}

# The part of `skirmish --seeds 2 --both-bases --json FILE` the tuner reads
# (four matches; the real file has many more fields).
SUMMARY = json.dumps({
    "schema": 1, "kind": "skirmish", "matches": 4,
    "per_match": [
        _pm(1, "normal", "ww2:usa", 1, 801.4, 12.2),
        _pm(1, "swapped", "ww2:germany", 1, 745.2, 8.6),
        _pm(2, "normal", "ww2:usa", 0, 1539.9, 15.4),
        _pm(2, "swapped", None, None, 2700.0, 7.6),
    ],
    "tick_p99_ms": {"peak": 3.388, "all": 1.418},
    "factions": {
        "germany": {"wins": 1, "kills": 404, "losses": 447, "kills_by_source_pct": _G_SRC},
        "usa": {"wins": 2, "kills": 447, "losses": 404, "kills_by_source_pct": _U_SRC},
    },
})

def rows_logistic(b0, b1, n, rng):
    rows = []
    for i in range(n):
        u = rng.random()
        rows.append(([1.0, u], 1.0 if rng.random() < at._sig(b0 + b1 * u) else 0.0, 1.0, i))
    return rows

class Parse(unittest.TestCase):
    def test_summary(self):
        p = at.parse_summary(SUMMARY)
        self.assertEqual(len(p["matches"]), 4)
        m = p["matches"]
        self.assertEqual((m[0]["winner"], m[0]["base"], m[0]["bases"]), ("usa", 1, "normal"))
        self.assertAlmostEqual(m[0]["len_min"], 801.4 / 60)
        self.assertEqual((m[3]["winner"], m[3]["base"]), (None, None))
        self.assertEqual(p["tick_p99"], 3.388)
        g = p["sides"]["germany"]
        self.assertEqual((g["wins"], g["kills"], g["losses"]), (1, 404, 447))
        self.assertEqual(g["kill_src"]["mortar"], 4.7)
        self.assertEqual(g["kill_src"]["tank_he"], 10.4)
        self.assertEqual(p["sides"]["usa"]["kill_src"]["vehicle_mg"], 4.9)
        self.assertNotIn("other", g["kill_src"])
        self.assertEqual(set(p["sides"]), {"germany", "usa"})

    @unittest.skipUnless(os.path.exists(os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                                     "..", "..", "target", "release", "frontline-cli")),
                         "needs target/release/frontline-cli")
    def test_real_cli_file_parses(self):
        root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
        cli = os.path.join(root, "target", "release", "frontline-cli")
        with tempfile.TemporaryDirectory() as d:
            out = os.path.join(d, "b.json")
            subprocess.run([cli, "skirmish", "--ai-vs-ai", "--seed", "1", "--seeds", "1", "--both-bases",
                            "--summary", "--minutes", "1", "--jobs", "2", "--json", out],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)
            with open(out) as f:
                p = at.parse_summary(f.read())
        self.assertEqual(len(p["matches"]), 2)
        self.assertIn("germany", p["sides"])
        self.assertIn("mortar", p["sides"]["germany"]["kill_src"])
        self.assertGreater(p["tick_p99"], 0)

    def test_metrics_and_normal_base(self):
        p = at.parse_summary(SUMMARY)
        b = [{"xs": [1.0], "seeds": 2, "rec": p}]
        met = at.metrics(b)
        self.assertEqual(met["share_usa"], 50.0)
        self.assertEqual(met["share_germany"], 25.0)
        self.assertAlmostEqual(met["kill_mortar_germany"], 4.7)
        # usa won normal from base 1 -> germany holds base 0 in a normal match
        self.assertEqual(at.normal_base("germany", b), 0)
        self.assertEqual(at.normal_base("usa", b), 1)

class Fit(unittest.TestCase):
    def test_logistic_recovers_truth(self):
        rng = random.Random(3)
        beta, cov = at.fit(rows_logistic(0.4, -2.0, 6000, rng), True)
        for est, true, i in ((beta[0], 0.4, 0), (beta[1], -2.0, 1)):
            se = math.sqrt(cov[i][i])
            self.assertLess(abs(est - true), 3 * se)
            self.assertLess(se, 0.15)

    def test_least_squares_exact(self):
        rows = [([1.0, u / 10], 3.0 + 2.0 * u / 10, 1.0, u) for u in range(11)]
        beta, cov = at.fit(rows, False)
        self.assertAlmostEqual(beta[0], 3.0, places=6)
        self.assertAlmostEqual(beta[1], 2.0, places=6)

    def test_weighted_per_batch(self):
        rng = random.Random(5)
        rows = [([1.0, u], 10 + 5 * u + rng.gauss(0, 0.3), 400.0, i)
                for i, u in enumerate((0, 0.5, 1, 0.25, 0.75, 0.5))]
        beta, cov = at.fit(rows, False)
        self.assertLess(abs(beta[1] - 5), 3 * math.sqrt(cov[1][1]) + 0.01)

class Fieller(unittest.TestCase):
    def test_zero_variance_is_the_point(self):
        pt, lo, hi = at.crossing([1.0, -2.0], [[0, 0], [0, 0]], [1.0, 0.0], 1, 0.0)
        self.assertAlmostEqual(pt, 0.5)
        self.assertAlmostEqual(lo, 0.5)
        self.assertAlmostEqual(hi, 0.5)

    def test_bounded_contains_point(self):
        pt, lo, hi = at.crossing([1.0, -2.0], [[0.01, 0], [0, 0.04]], [1.0, 0.0], 1, 0.0)
        self.assertTrue(lo < pt < hi)

    def test_unbounded_when_slope_unclear(self):
        pt, lo, hi = at.crossing([1.0, -0.2], [[0.01, 0], [0, 0.04]], [1.0, 0.0], 1, 0.0)
        self.assertIsNone(lo)
        self.assertIsNone(hi)

    def test_coverage(self):
        """The 95 % level interval covers the true crossing about 95 % of the time."""
        rng = random.Random(11)
        hits = n = 0
        for _ in range(150):
            beta, cov = at.fit(rows_logistic(1.0, -2.0, 400, rng), True)
            pt, lo, hi = at.crossing(beta, cov, [1.0, 0.0], 1, 0.0)
            if lo is not None:
                n += 1
                hits += lo <= 0.5 <= hi
        self.assertGreater(n, 140)
        self.assertGreater(hits / n, 0.89)

class Harness(unittest.TestCase):
    def test_new_finds_the_level(self):
        """Synthetic truth, 6 points of share per 0.05 step, budget 800 matches:
        the true share at the chosen level is inside 45-55 in most runs."""
        rng = random.Random(21)
        ok = 0
        reps = 25
        for _ in range(reps):
            t = synth.Truth(rng.uniform(0.75, 0.95), 6)
            t.tag = rng.random()
            r = synth.new_search(t, max_matches=800)
            ok += 45 <= t.share(r["chosen"]) <= 55
            self.assertLessEqual(r["matches"], 800)
        self.assertGreaterEqual(ok, int(0.85 * reps))

    def test_out_of_range_is_not_found(self):
        """A band the range cannot reach must not be reported as solved."""
        t = synth.Truth(0.55, 8)          # 50 % lies far below the 0.70-1.00 range
        t.tag = 0.5
        r = synth.new_search(t, max_matches=800)
        self.assertEqual(r["verdict"], "not found")
        self.assertEqual(r["chosen"], 0.70)

    def test_truth_is_deterministic(self):
        t = synth.Truth(0.8, 6)
        self.assertEqual(t.batch(0.8, 1, 5), t.batch(0.8, 1, 5))
        self.assertAlmostEqual(t.level(), 0.8, places=3)

if __name__ == "__main__":
    unittest.main(verbosity=1)
