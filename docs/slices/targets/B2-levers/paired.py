#!/usr/bin/env python3
"""B2 paired reading of a lever against the base (docs/slices/B2-balance.md 4).

  python3 -I docs/slices/targets/B2-levers/paired.py NAME BASE LEVER [BASE LEVER]...
  python3 -I docs/slices/targets/B2-levers/paired.py --detail BASE [BASE]...

BASE and LEVER are skirmish JSON files of the same seeds and sides (one pair
per seed block), or the .csv files this script wrote earlier (wins only).
Matches are paired by (seed, side). Prints one line for the log:

  NAME n=320 base 41.9 % lever 48.1 % diff +6.2 up 61 down 41 z +1.98 blocks +5.0 +7.5 | floors ok

up / down: pairs Germany wins only with the lever / only at the base;
z = (up - down) / sqrt(up + down) (the paired sign test, as B1 used).
Floors (JSON inputs only; each named when it fails), read on the lever runs
pooled, against the base runs pooled:
  no match without a winner; works in >= 90 % and wire in >= 60 % of matches
  for both sides; mines in at least base - 10 % of matches; HE share
  <= 26.3 and not under min(20, base - 1); mortar share of kills per side
  <= 15 and not under min(10, base - 1); all vehicles' share of kills per
  side <= max(30, base + 2); median length 15-30 min; no base side above
  62 %; an infantry gun bought in >= 90 % of matches for both sides.
With --csv DIR each input JSON is also written to DIR/<block>_<name>.csv
(seed,side,winner,length_s): small enough to commit, so the pairing can be
redone after target/ is lost.

--detail reads files played with --side-detail and prints, per faction (all
matches, and split by who won), men alive at 5 and 10 min and foot squads at
1, 3 and 5 min. hold_s and points held are NOT printed: both are larger for
whoever won, so they cannot test hypothesis H1.
"""
import csv
import json
import math
import os
import sys

GER = "ww2:germany"
FACTIONS = ("germany", "usa")


def load(path):
    """-> (rows {(seed, side): (german_win, length_s, won)}, json or None)"""
    if path.endswith(".csv"):
        rows = {}
        with open(path, newline="") as f:
            for r in csv.DictReader(f):
                rows[(int(r["seed"]), r["side"])] = (r["winner"] == "G", float(r["length_s"]), r["winner"] != "-")
        return rows, None
    j = json.load(open(path))
    rows = {}
    for m in j["per_match"]:
        won = m.get("result") == "won"
        rows[(m["seed"], m["side"])] = (won and m.get("winner_faction") == GER, m["length_s"], won)
    return rows, j


def write_csv(rows, out):
    os.makedirs(os.path.dirname(out) or ".", exist_ok=True)
    with open(out, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["seed", "side", "winner", "length_s"])
        for (seed, side), (g, length, won) in sorted(rows.items()):
            w.writerow([seed, side, "G" if g else ("U" if won else "-"), length])


def pooled(js, f):
    """Match-weighted means of the floor metrics over several set files."""
    n = sum(j["matches"] for j in js)

    def mean(get):
        return sum(get(j) * j["matches"] for j in js) / n

    out = {
        "n": n,
        "nowin": sum(1 for j in js for m in j["per_match"] if m.get("result") != "won"),
        "he": mean(lambda j: j["he_share_pct"]["mean"]),
        "len_med": mean(lambda j: j["length_min"]["median"]),
        "base_max": max(j["wins"]["base_max_pct"] for j in js),
    }
    for name in FACTIONS:
        out[name] = {
            "works": mean(lambda j: j["factions"][name]["s10_derived"]["matches_with_works_pct"]),
            "wire": mean(lambda j: j["factions"][name]["s10_derived"]["matches_with_wire_pct"]),
            "mines": mean(lambda j: j["factions"][name]["s10b_ai_derived"]["matches_with_mines_pct"]),
            "mortar": mean(lambda j: j["factions"][name]["kills_by_source_pct"]["mortar"]),
            "veh": mean(lambda j: j["factions"][name]["s7_derived"]["vehicle_kills_pct"]),
            "gun": mean(lambda j: j["factions"][name]["s11a1_derived"]["gun_bought_matches_pct"]),
        }
    return out


def floors(base, lev):
    bad = []
    if lev["nowin"]:
        bad.append("no-winner %d" % lev["nowin"])
    if lev["he"] > 26.3 or lev["he"] < min(20.0, base["he"] - 1.0):
        bad.append("HE share %.1f (base %.1f)" % (lev["he"], base["he"]))
    if not 15.0 <= lev["len_med"] <= 30.0:
        bad.append("median length %.1f" % lev["len_med"])
    if lev["base_max"] > 62.0:
        bad.append("base side %.1f" % lev["base_max"])
    for name in FACTIONS:
        b, v = base[name], lev[name]
        if v["works"] < 90.0:
            bad.append("%s works %.0f" % (name, v["works"]))
        if v["wire"] < 60.0:
            bad.append("%s wire %.0f" % (name, v["wire"]))
        if v["mines"] < b["mines"] - 10.0:
            bad.append("%s mines %.0f (base %.0f)" % (name, v["mines"], b["mines"]))
        if v["mortar"] > 15.0 or v["mortar"] < min(10.0, b["mortar"] - 1.0):
            bad.append("%s mortar %.1f (base %.1f)" % (name, v["mortar"], b["mortar"]))
        if v["veh"] > max(30.0, b["veh"] + 2.0):
            bad.append("%s vehicle kills %.1f (base %.1f)" % (name, v["veh"], b["veh"]))
        if v["gun"] < 90.0:
            bad.append("%s gun bought %.0f" % (name, v["gun"]))
    return bad


def block_name(path):
    # target/b2/s2001/base.json -> s2001_base
    d = os.path.basename(os.path.dirname(os.path.abspath(path)))
    return "%s_%s" % (d, os.path.splitext(os.path.basename(path))[0])


def paired(argv):
    out_dir = None
    if argv and argv[0] == "--csv":
        out_dir, argv = argv[1], argv[2:]
    if len(argv) < 3 or len(argv) % 2 == 0:
        sys.exit(__doc__)
    name, files = argv[0], argv[1:]
    n = gb = gl = up = down = 0
    blocks, bj, lj = [], [], []
    for b_path, l_path in zip(files[0::2], files[1::2]):
        b, bjs = load(b_path)
        lv, ljs = load(l_path)
        if set(b) != set(lv):
            sys.exit("%s and %s do not hold the same (seed, side) matches" % (b_path, l_path))
        if out_dir:
            if bjs is not None:
                write_csv(b, os.path.join(out_dir, block_name(b_path) + ".csv"))
            if ljs is not None:
                write_csv(lv, os.path.join(out_dir, block_name(l_path) + ".csv"))
        u = sum(1 for k in b if lv[k][0] and not b[k][0])
        d = sum(1 for k in b if b[k][0] and not lv[k][0])
        blocks.append(100.0 * (u - d) / len(b))
        n += len(b)
        gb += sum(1 for k in b if b[k][0])
        gl += sum(1 for k in lv if lv[k][0])
        up += u
        down += d
        if bjs is not None and ljs is not None:
            bj.append(bjs)
            lj.append(ljs)
    z = (up - down) / math.sqrt(up + down) if up + down else 0.0
    line = "%s n=%d base %.1f %% lever %.1f %% diff %+.1f up %d down %d z %+.2f blocks %s" % (
        name, n, 100.0 * gb / n, 100.0 * gl / n, 100.0 * (gl - gb) / n, up, down, z,
        " ".join("%+.1f" % x for x in blocks))
    if bj:
        bad = floors(pooled(bj, None), pooled(lj, None))
        line += " | floors " + ("ok" if not bad else "FAIL: " + "; ".join(bad))
        if len(bj) < len(files) // 2:
            line += " (read on %d of %d blocks: the others are csv)" % (len(bj), len(files) // 2)
    else:
        line += " | floors not read (csv input)"
    print(line)


def detail(paths):
    """Per faction, all matches and split by who won: men alive at 5 / 10
    min and foot squads at 1 / 3 / 5 min. Split by winner only to show the
    spread: the winner has more of everything late, so only the "all" rows
    and the minute 1-5 columns say anything about the opening (H1)."""
    acc = {}
    for p in paths:
        j = json.load(open(p))
        for m in j["per_match"]:
            if "sides" not in m:
                sys.exit("%s was not played with --side-detail" % p)
            for s in m["sides"]:
                f = s["faction"]
                won = m.get("result") == "won" and m.get("winner_faction") == f
                for key in ((f, "all"), (f, "won" if won else "lost")):
                    a = acc.setdefault(key, {"n": 0, "pop": [0.0, 0.0], "sq": [0.0, 0.0, 0.0], "k": 0, "l": 0})
                    a["n"] += 1
                    pop = s.get("pop_at_min") or []
                    for i in (0, 1):  # minutes 5, 10
                        a["pop"][i] += (pop[i] if i < len(pop) and pop[i] is not None else 0)
                    sq = s.get("squads_at_min") or []
                    for i, k in enumerate((0, 2, 3)):  # minutes 1, 3, 5 of [1, 2, 3, 5, 10]
                        a["sq"][i] += (sq[k][0] if k < len(sq) and sq[k] else 0)
                    a["k"] += s["kills"]
                    a["l"] += s["losses"]
    print("faction matches n pop5 pop10 squads1 squads3 squads5 kills losses (means; a match shorter than the minute counts 0)")
    for (f, w), a in sorted(acc.items()):
        n = a["n"]
        print("%-12s %-4s %3d %5.1f %5.1f %5.2f %5.2f %5.2f %5.1f %5.1f" % (
            f, w, n, a["pop"][0] / n, a["pop"][1] / n, a["sq"][0] / n, a["sq"][1] / n, a["sq"][2] / n, a["k"] / n, a["l"] / n))


if __name__ == "__main__":
    args = sys.argv[1:]
    if args and args[0] == "--detail":
        detail(args[1:])
    else:
        paired(args)
