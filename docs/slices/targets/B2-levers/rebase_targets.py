#!/usr/bin/env python3
"""Writes docs/slices/targets/B2.toml from S11b.toml (docs/slices/B2-balance.md 6).

  python3 -I docs/slices/targets/B2-levers/rebase_targets.py MEASURE_DIR [--applied] > docs/slices/targets/B2.toml

MEASURE_DIR holds the three 160 sets of the last measure BEFORE any B2 change
(skirmish160.json, _oos, _s201). Every other row is copied unchanged; each
changed row gets a line `# B2: ...` above it with the old bound and the reason.

Rule R (floors made stale by design, owner question S11b-10 (A)): new floor =
0.85 x the lowest of the three sets, rounded down to a multiple of 5 (share
of matches) or 0.5 (manpower share), never above the old floor. One 160 set
has a standard error of about 3-4 points on such a share, so 15 % is two to
three of them: the floor guards against further displacement, it is not a
design goal (the old bound stays in the comment as the unmet design figure).
T97 (cap): 1.1 x the highest of both sides' three sets, rounded up to a whole
number (one cap for all four rows).
--applied (run 2, only when B2 changed a default): T5c, T5d, T5e become the
hard per-set band 40-60.
"""
import json
import math
import os
import re
import sys

SRC = "docs/slices/targets/S11b.toml"
SETS = ("skirmish160", "skirmish160_oos", "skirmish160_s201")
WHY = "stale by design: the gun and observer cards (S11a1) and the howitzer (S11b) take the manpower (S11b-10 (A))"
B = "s7_derived.bought_matches_pct."
M = "s7_derived.mp_share_pct."
# id -> (faction, metric under factions.<f>, step)
FLOORS = {
    "T30a": ("germany", B + "ger_stug3_g_squad", 5), "T30c": ("germany", B + "ger_stug3_g_squad", 5),
    "T30b": ("usa", B + "usa_m10_squad", 5), "T30d": ("usa", B + "usa_m10_squad", 5),
    "T40a": ("germany", B + "ger_sdkfz251_1_squad", 5), "T40c": ("germany", B + "ger_sdkfz251_1_squad", 5),
    "T40b": ("usa", B + "usa_m3a1_squad", 5), "T40d": ("usa", B + "usa_m3a1_squad", 5),
    "T72a": ("germany", B + "ger_sdkfz251_1_squad", 5), "T72b": ("usa", B + "usa_m3a1_squad", 5),
    "T54a": ("germany", B + "ger_pz4_h_squad", 5), "T54b": ("usa", B + "usa_m4a3_75_squad", 5),
    "T54c": ("germany", M + "ger_pz4_h_squad", 0.5), "T54d": ("usa", M + "usa_m4a3_75_squad", 0.5),
    "T54e": ("germany", B + "ger_stug3_g_squad", 5), "T54f": ("usa", B + "usa_m10_squad", 5),
}
CAPS = {"T97a": "germany", "T97b": "usa", "T97c": "germany", "T97d": "usa"}
NOTES = {
    "T31a": 1, "T31b": 1, "T31c": 1, "T31d": 1,
}
T31_NOTE = ("B2: stays a report, bound unchanged. AT guns (S9, about 4-4.5 % of Germany's kills) and infantry guns "
            "(S11a1, 3.5-4 %) are counted inside this share; a vehicles-only share is CLI work (owner question B2-5)")


def get(j, f, path):
    v = j["factions"][f]
    for k in path.split("."):
        v = v[k]
    return v


def fmt(x):
    return ("%g" % x)


def main():
    args = [a for a in sys.argv[1:] if a != "--applied"]
    applied = "--applied" in sys.argv[1:]
    if len(args) != 1:
        sys.exit(__doc__)
    sets = [json.load(open(os.path.join(args[0], s + ".json"))) for s in SETS]
    lines = open(SRC).read().split("\n")
    out, changed = [], []
    i = 0
    while i < len(lines):
        ln = lines[i]
        m = re.match(r'id = "(T\w+)"', ln)
        tid = m.group(1) if m else None
        if ln.startswith("name = "):
            out.append('name = "B2: balance pass (S11b targets with the re-based rows marked `# B2:`)"')
            i += 1
            continue
        if tid in FLOORS or tid in CAPS or tid in NOTES or (applied and tid in ("T5c", "T5d", "T5e")):
            # The block runs from this id line to the next blank line or table.
            j = i + 1
            while j < len(lines) and lines[j].strip() and not lines[j].startswith("["):
                j += 1
            block = lines[i:j]
            if tid in FLOORS:
                f, path, step = FLOORS[tid]
                vals = [get(s, f, path) for s in sets]
                old = next(float(b.split("=")[1]) for b in block if b.startswith("min = "))
                new = min(old, math.floor(0.85 * min(vals) / step) * step)
                block = ["min = %s" % fmt(new) if b.startswith("min = ") else b for b in block]
                note = "# B2: floor %s -> %s (rule R on %s). %s" % (
                    fmt(old), fmt(new), " / ".join("%.1f" % v for v in vals), WHY)
            elif tid in CAPS:
                vals = [s["factions"][f]["s10_derived"]["builder_share_pct"] for f in ("germany", "usa") for s in sets]
                old = next(float(b.split("=")[1]) for b in block if b.startswith("max = "))
                new = max(old, math.ceil(1.1 * max(vals)))
                block = ["max = %s" % fmt(new) if b.startswith("max = ") else b for b in block]
                note = ("# B2: cap %s -> %s (1.1 x the highest of both sides' sets, %s). Stale by design: S10c-ai's builder floor "
                        "(builder_cards_min 2) and front jobs (jobs_max 2) put more plans on fort jobs than the "
                        "engineer mix weight the 12 came from (owner question B2-4)" % (
                            fmt(old), fmt(new), " / ".join("%.1f" % v for v in vals)))
            elif tid in NOTES:
                block = ['note = "%s"' % T31_NOTE if b.startswith("note = ") else b for b in block]
                note = "# B2: note only (T31 is settled as a report; see the note)"
            else:
                block = [b for b in block if not b.startswith(("min = ", "max = ", "kind = ", "note = "))]
                block += ["min = 40", "max = 60",
                          'note = "B2 applied: each 160 set inside 40-60 (hard); the pool of 480 inside 45-55 is read in the log"']
                note = "# B2: per-set band 40-60, hard (was the 35-65 guard band or a report)"
            # The comment goes above the [[target]] line already written.
            k = len(out) - 1
            while k >= 0 and out[k].strip() != "[[target]]":
                k -= 1
            out.insert(k if k >= 0 else len(out), note)
            out.extend(block)
            changed.append(tid)
            i = j
            continue
        out.append(ln)
        i += 1
    head = [
        "# B2 targets (docs/slices/B2-balance.md 6). GENERATED: do not edit by hand.",
        "#   python3 -I docs/slices/targets/B2-levers/rebase_targets.py %s%s > docs/slices/targets/B2.toml" % (
            args[0], " --applied" if applied else ""),
        "# A copy of S11b.toml; changed rows (%d) carry a `# B2:` line: %s." % (len(changed), " ".join(changed)),
        "# Not re-based, with reasons (B2-balance.md 6): T8, T9, T55, T56, T81, T88, T95, T110-T115, T119, T122.",
        "",
    ]
    want = set(FLOORS) | set(CAPS) | set(NOTES) | ({"T5c", "T5d", "T5e"} if applied else set())
    missing = sorted(want - set(changed))
    if missing:
        sys.exit("rows not found in %s: %s" % (SRC, " ".join(missing)))
    sys.stdout.write("\n".join(head + out))


if __name__ == "__main__":
    main()
