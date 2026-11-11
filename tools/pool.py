#!/usr/bin/env python3
"""The pooled reading of the three 160 sets (docs/process/dev-speed.md).

  python3 -I tools/pool.py DIR [DIR ...]

Each DIR holds skirmish160.json, skirmish160_oos.json and skirmish160_s201.json
(`tools/batch.sh sets DIR`). One block per DIR: Germany's share per set and
pooled, matches without a winner, and per faction the gun numbers and the
shares the S9 target misses read. With two or more DIRs the first is the
baseline and the others also print their change against it.

Balance is judged on the pool only: one set of 160 has about +-7.7 points of
noise, the pool of 480 about +-4.5 (95 %).
"""
import json
import os
import sys

SETS = ("skirmish160", "skirmish160_oos", "skirmish160_s201")
# S10a-ai: the four "uses it" shares (T94-T96, T100) and the builders' share
# of the infantry plans (T97), per set from factions.<f>.s10_derived.
FORT = ("matches_with_works_pct", "matches_with_wire_pct", "matches_with_gap_pct",
        "breach_when_enemy_wire_pct", "builder_share_pct")
BOUGHT = {"germany": ["ger_sdkfz251_1_squad", "ger_stug3_g_squad", "ger_pz4_h_squad"],
          "usa": ["usa_m3a1_squad", "usa_m10_squad", "usa_m4a3_75_squad"]}


def read(d):
    out = {"dir": d, "sets": [], "n": 0, "ger": 0, "nowin": 0, "he": [], "f": {}}
    for s in SETS:
        p = os.path.join(d, s + ".json")
        if not os.path.exists(p):
            continue
        j = json.load(open(p))
        n = j["matches"]
        w = j["wins"]
        g = w.get("germany", 0)
        out["sets"].append(100.0 * g / n)
        out["n"] += n
        out["ger"] += g
        out["nowin"] += sum(1 for m in j["per_match"] if m.get("result") != "won")
        out["he"].append(j["he_share_pct"]["mean"])
        for name, f in j["factions"].items():
            a = out["f"].setdefault(name, {"s9": {}, "bought": {}, "mp": {}, "veh_kills": [], "n": 0})
            a["n"] += n
            for k, v in f["s9"].items():
                a["s9"][k] = a["s9"].get(k, 0) + v
            d7 = f["s7_derived"]
            for k, v in d7["bought_matches_pct"].items():
                a["bought"][k] = a["bought"].get(k, 0) + v * n
            for k, v in d7["mp_share_pct"].items():
                a["mp"][k] = a["mp"].get(k, 0) + v * n
            a["veh_kills"].append(d7["vehicle_kills_pct"])
            a.setdefault("fired", []).append(f["s9_derived"]["gun_fired_matches_pct"])
            a.setdefault("cover", []).append(f["s9_derived"]["gun_in_cover_pct"])
            a.setdefault("gun_mp", []).append(f["s9_derived"]["gun_mp_pct"])
            if "gun_armour_out_per_match" in f["s9_derived"]:
                a.setdefault("out", []).append(f["s9_derived"]["gun_armour_out_per_match"])
            d10 = f.get("s10_derived")
            if d10 is not None:
                for k in FORT:
                    a.setdefault("fort", {}).setdefault(k, []).append(d10.get(k))
    return out


def fmt(xs, nd=1):
    # A share over an empty set is null in the JSON (S10a-ai fix F10): "-".
    return " / ".join("-" if x is None else f"{x:.{nd}f}" for x in xs)


def show(o, base):
    if not o["n"]:
        print(f"{o['dir']}: no set found")
        return
    pool = 100.0 * o["ger"] / o["n"]
    delta = f" ({pool - 100.0 * base['ger'] / base['n']:+.1f} against the baseline)" if base and base["n"] else ""
    print(f"{o['dir']}: Germany pooled {pool:.2f} % of {o['n']}{delta}; sets {fmt(o['sets'], 2)}; "
          f"no winner {o['nowin']}; HE share {fmt(o['he'])}")
    for name, a in sorted(o["f"].items()):
        n = a["n"]
        s = a["s9"]
        bought = ", ".join(f"{k.replace('_squad', '')} {a['bought'].get(k, 0) / n:.0f} %" for k in BOUGHT.get(name, []))
        mp = ", ".join(f"{k.replace('_squad', '')} {a['mp'].get(k, 0) / n:.1f}" for k in BOUGHT.get(name, []))
        print(f"  {name}: gun tank KOs {s.get('gun_ko_tanks', 0)} ({s.get('gun_ko_tanks', 0) / n:.3f} per match), "
              f"guns bought {s.get('guns_bought', 0) / n:.2f}, rounds {s.get('gun_rounds', 0) / n:.1f}, "
              f"crew lost {s.get('gun_crew_lost', 0) / n:.2f}, re-sites {s.get('gun_resites', 0) / n:.2f}, "
              f"AI fails {s.get('gun_ai_fails', 0) / n:.2f} per match")
        if a.get("out"):
            print(f"    armour put out of action by its guns per match: {fmt(a['out'], 2)} (mean {sum(a['out']) / len(a['out']):.2f})")
        print(f"    fired in {fmt(a['fired'], 0)} % of matches (floor 60), in cover {fmt(a['cover'], 0)} % (floor 60), "
              f"gun MP {fmt(a['gun_mp'])} % (cap 20), vehicle kills {fmt(a['veh_kills'])} % (cap 30)")
        print(f"    bought in matches: {bought}; MP share: {mp}")
        if a.get("fort"):
            fo = a["fort"]
            print(f"    fort: works in {fmt(fo['matches_with_works_pct'], 0)} %, wire in {fmt(fo['matches_with_wire_pct'], 0)} %, "
                  f"gaps in {fmt(fo['matches_with_gap_pct'], 0)} %, breach when enemy wire {fmt(fo['breach_when_enemy_wire_pct'], 0)} %; "
                  f"builders {fmt(fo['builder_share_pct'])} % of infantry plans (cap 12)")
    # One line for scripts and stage gates.
    fs = o["f"].values()
    gate = {"pool": round(pool, 2), "nowin": o["nowin"],
            "fired_min": round(min(min(a["fired"]) for a in fs), 1),
            "cover_min": round(min(min(a["cover"]) for a in fs), 1),
            "gun_mp_max": round(max(max(a["gun_mp"]) for a in fs), 1)}
    for name, a in sorted(o["f"].items()):
        if a.get("out"):
            gate["out_" + name] = round(sum(a["out"]) / len(a["out"]), 3)
    print("  GATE " + " ".join(f"{k}={v}" for k, v in gate.items()))


def main():
    dirs = sys.argv[1:]
    if not dirs:
        print(__doc__)
        sys.exit(2)
    base = None
    for d in dirs:
        o = read(d)
        show(o, base)
        base = base or o


if __name__ == "__main__":
    main()
