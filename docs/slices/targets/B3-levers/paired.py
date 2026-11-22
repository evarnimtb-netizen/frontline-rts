#!/usr/bin/env python3
"""B3 paired reading of a lever against the base (docs/slices/B3-balance.md 5).

  python3 -I docs/slices/targets/B3-levers/paired.py [--csv DIR] NAME BASE LEVER [BASE LEVER]...
  python3 -I docs/slices/targets/B3-levers/paired.py --counters FILE [FILE]...
  python3 -I docs/slices/targets/B3-levers/paired.py --armour BASE LEVER

BASE and LEVER are skirmish JSON files of the same seeds and sides (one pair
per seed block). Matches are paired by (seed, side). One line for the log:

  NAME n=160 base 42.5 % lever 44.4 % diff +1.9 up 30 down 27 z +0.40 blocks +1.9 | HE 28.1 -> 26.9 | floors ok

up / down: pairs Germany wins only with the lever / only at the base;
z = (up - down) / sqrt(up + down). HE is T8's metric, the HE share of
casualties (`s12_derived.he_share_casualties_pct.mean`, match-weighted over
the files), NOT the share of kills B2 read.
Floors (each named when it fails; lever files pooled against base files pooled):
  no match without a winner; HE share <= max(26.3, base + 0.3) and >= 20;
  works in >= 90 % and wire in >= 60 % of matches for both sides; mines in
  at least base - 10 %; mortar share of kills per side <= 15 and not under
  min(10, base - 1); median length 15-30 min; no base side above 62 %; an
  infantry gun bought in >= 90 % of matches for both sides; the Jumbo
  bought in >= 10 % of matches when the base has it there (T179d).
All vehicles' share of kills (T31, a report row: B2-5 (A)) is printed by
--counters and is no floor here.
With --csv DIR each input JSON is also written to DIR/<block>_<name>.csv
(seed,side,winner,length_s), small enough to commit.

--counters prints, per file and faction, a match: casualties caused by class
(one per man, by the class that first took him out: kills - finished +
downed), split into outright kills and downs for the three HE classes; HE
rounds; base-gun rounds, shell manpower share, budget holds and missions; rifle
grenades fired; the S13 cards (matches bought in, %), heavy plans / buys /
drops; vehicle cards bought; own downs stabilised, bled out, killed while
down (%); mortar and all-vehicle shares of kills.

--armour reads two `armour-test --json` files and prints the overall pass of
each and every check whose value or pass differs (case, check, base -> lever).
"""
import csv
import json
import math
import os
import sys

GER = "ww2:germany"
FACTIONS = ("germany", "usa")
HE_CLASSES = ("tank_he", "mortar", "rifle_grenade")
JUMBO = "usa_m4a3e2_squad"


def load(path):
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


def card_pct(j, name, card):
    return j["factions"][name].get("s13", {}).get("cards", {}).get(card, {}).get("matches_pct", 0.0)


def pooled(js):
    """Match-weighted means of the floor metrics over several set files."""
    n = sum(j["matches"] for j in js)

    def mean(get):
        return sum(get(j) * j["matches"] for j in js) / n

    out = {
        "n": n,
        "nowin": sum(1 for j in js for m in j["per_match"] if m.get("result") != "won"),
        "he": mean(lambda j: j["s12_derived"]["he_share_casualties_pct"]["mean"]),
        "len_med": mean(lambda j: j["length_min"]["median"]),
        "base_max": max(j["wins"]["base_max_pct"] for j in js),
        "jumbo": mean(lambda j: card_pct(j, "usa", JUMBO)),
    }
    for name in FACTIONS:
        out[name] = {
            "works": mean(lambda j: j["factions"][name]["s10_derived"]["matches_with_works_pct"]),
            "wire": mean(lambda j: j["factions"][name]["s10_derived"]["matches_with_wire_pct"]),
            "mines": mean(lambda j: j["factions"][name]["s10b_ai_derived"]["matches_with_mines_pct"]),
            "mortar": mean(lambda j: j["factions"][name]["kills_by_source_pct"]["mortar"]),
            "gun": mean(lambda j: j["factions"][name]["s11a1_derived"]["gun_bought_matches_pct"]),
        }
    return out


def floors(base, lev):
    bad = []
    if lev["nowin"]:
        bad.append("no-winner %d" % lev["nowin"])
    if lev["he"] > max(26.3, base["he"] + 0.3) or lev["he"] < 20.0:
        bad.append("HE share %.2f (base %.2f)" % (lev["he"], base["he"]))
    if not 15.0 <= lev["len_med"] <= 30.0:
        bad.append("median length %.1f" % lev["len_med"])
    if lev["base_max"] > 62.0:
        bad.append("base side %.1f" % lev["base_max"])
    if base["jumbo"] >= 10.0 and lev["jumbo"] < 10.0:
        bad.append("Jumbo bought %.1f (base %.1f)" % (lev["jumbo"], base["jumbo"]))
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
        if v["gun"] < 90.0:
            bad.append("%s gun bought %.0f" % (name, v["gun"]))
    return bad


def block_name(path):
    # target/b3/s3001/base.json -> s3001_base
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
            write_csv(b, os.path.join(out_dir, block_name(b_path) + ".csv"))
            write_csv(lv, os.path.join(out_dir, block_name(l_path) + ".csv"))
        u = sum(1 for k in b if lv[k][0] and not b[k][0])
        d = sum(1 for k in b if b[k][0] and not lv[k][0])
        blocks.append(100.0 * (u - d) / len(b))
        n += len(b)
        gb += sum(1 for k in b if b[k][0])
        gl += sum(1 for k in lv if lv[k][0])
        up += u
        down += d
        bj.append(bjs)
        lj.append(ljs)
    z = (up - down) / math.sqrt(up + down) if up + down else 0.0
    pb, pl = pooled(bj), pooled(lj)
    bad = floors(pb, pl)
    print("%s n=%d base %.1f %% lever %.1f %% diff %+.1f up %d down %d z %+.2f blocks %s | HE %.2f -> %.2f | floors %s" % (
        name, n, 100.0 * gb / n, 100.0 * gl / n, 100.0 * (gl - gb) / n, up, down, z,
        " ".join("%+.1f" % x for x in blocks), pb["he"], pl["he"],
        "ok" if not bad else "FAIL: " + "; ".join(bad)))


def counters(paths):
    for p in paths:
        j = json.load(open(p))
        n = j["matches"]
        print("%s: %d matches, Germany %.1f %%, HE share of casualties %.2f, median %.1f min" % (
            p, n, j["wins"]["germany_pct"], j["s12_derived"]["he_share_casualties_pct"]["mean"],
            j["length_min"]["median"]))
        for name in FACTIONS:
            f = j["factions"][name]
            s12 = f.get("s12", {})
            kills = {c: f["kills"] * v / 100.0 for c, v in f["kills_by_source_pct"].items()}
            fin, dwn = s12.get("finished_by", {}), s12.get("downed_by", {})
            cas = {c: kills[c] - fin.get(c, 0) + dwn.get(c, 0) for c in kills}
            total = sum(cas.values()) or 1.0
            he = " ".join("%s %.2f (outright %.2f, downs %.2f)" % (
                c, cas[c] / n, (kills[c] - fin.get(c, 0)) / n, dwn.get(c, 0) / n) for c in HE_CLASSES)
            print("  %s casualties caused %.1f a match, HE %.1f %%: %s" % (
                name, total / n, 100.0 * sum(cas[c] for c in HE_CLASSES) / total, he))
            a1, b1, s5 = f["s11a1"], f["s11b"], f.get("s5a", {})
            fired = s5.get("fired", [0] * 9)
            print("    HE rounds %.1f, base-gun rounds %.1f (howitzer %.1f), shells %.2f %% of income, budget holds %.1f, "
                  "gun missions %.2f, rifle grenades fired %.2f" % (
                      fired[8] / n, sum(a1["gun_rounds"].values()) / n, sum(b1["how_rounds"].values()) / n,
                      f["s11a1_derived"]["gun_shell_mp_pct"], a1.get("arty_budget_holds", 0) / n,
                      a1.get("arty_missions", 0) / n,
                      b1["rg_fired"] / n))
            s13 = f.get("s13", {})
            cards = " ".join("%s %.1f %%" % (c.replace("_squad", ""), v["matches_pct"])
                             for c, v in sorted(s13.get("cards", {}).items()))
            print("    S13 cards bought in matches: %s; heavy plans %d buys %d drops %d" % (
                cards, s13.get("heavy_plans", 0), s13.get("heavy_buys", 0), s13.get("heavy_drops", 0)))
            veh = sorted((c, v) for c, v in f["s4"]["bought"].items()
                         if c.replace("_squad", "") in f["s7"]["kills_by_vehicle"])
            print("    vehicle and gun cards a match: %s" % " ".join(
                "%s %.2f" % (c.replace("_squad", ""), v / n) for c, v in veh if v))
            d = f.get("s12_derived", {})
            print("    own downs: stabilised %.1f %%, bled out %.1f %%, killed while down %.1f %%; "
                  "mortar %.1f %% and all vehicles %.1f %% of its kills" % (
                      d.get("stabilised_pct", 0.0), d.get("bled_out_pct", 0.0), d.get("downed_killed_pct", 0.0),
                      f["kills_by_source_pct"]["mortar"], f["s7_derived"]["vehicle_kills_pct"]))


def armour(base_path, lever_path):
    b, lv = json.load(open(base_path)), json.load(open(lever_path))
    print("armour-test pass: base %s, lever %s" % (b.get("pass"), lv.get("pass")))

    def checks(j):
        out = {}
        cases = j.get("cases", {})
        items = cases.items() if isinstance(cases, dict) else ((c.get("id"), c) for c in cases)
        for cid, c in items:
            ch = c.get("checks", {})
            rows = ch.items() if isinstance(ch, dict) else ((k.get("id") or k.get("name"), k) for k in ch)
            for kid, k in rows:
                out[(str(cid), str(kid))] = (k.get("value"), k.get("pass"))
        return out

    cb, cl = checks(b), checks(lv)
    moved = 0
    for key in sorted(set(cb) | set(cl)):
        if cb.get(key) != cl.get(key):
            moved += 1
            print("  %s %s: %s -> %s" % (key[0], key[1], cb.get(key), cl.get(key)))
    print("%d check(s) moved of %d" % (moved, len(set(cb) | set(cl))))


if __name__ == "__main__":
    args = sys.argv[1:]
    if args and args[0] == "--counters":
        counters(args[1:])
    elif args and args[0] == "--armour" and len(args) == 3:
        armour(args[1], args[2])
    else:
        paired(args)
