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

S16a: sets are pooled per map (the JSON `map`; a set without one is Kessel
Ridge, the only map before S16). A DIR whose sets played two maps prints one
block per map, and the change against the baseline is read against the
baseline's block of the same map.

S15 (merge): the same holds per faction pair (the JSON `factions`; Germany
against the USA unless named), so a Germany-USSR set never pools with a
Germany-USA one. A 2v2 set (S17 `--teams`) has no Germany share to pool; read
its `teams` block (`tools/batch.sh teams`).
"""
import json
import os
import sys

SETS = ("skirmish160", "skirmish160_oos", "skirmish160_s201")
# S10a-ai: the four "uses it" shares (T94-T96, T100) and the builders' share
# of the infantry plans (T97), per set from factions.<f>.s10_derived.
FORT = ("matches_with_works_pct", "matches_with_wire_pct", "matches_with_gap_pct",
        "breach_when_enemy_wire_pct", "builder_share_pct")
# S10b-ai: the four "matches with" shares (T109, T111-T113) and the own-side
# mistakes (T115), per set from factions.<f>.s10b_ai_derived.
S10B = ("matches_with_mines_pct", "matches_with_obstacle_pct", "matches_with_bunker_pct",
        "bunker_done_when_started_pct", "sweep_when_wanted_pct", "own_side_mistakes")
# S12b (Build C): per set from factions.<f>.s12_derived (medic_matches_pct is
# hard >= 60; T8 reads the set's s12_derived.he_share_casualties_pct).
S12 = ("medic_matches_pct", "stabilised_pct", "bled_out_pct", "downed_per_match",
       "reinforced_men_per_match", "he_share_casualties_pct")
# S14-ai (Build D): per set from factions.<f>.s14_derived (reports only:
# owner 2026-11-25, no balance threshold in S14).
S14 = ("flame_matches_pct", "flame_kills_per_match", "flame_jobs_per_match",
       "clear_orders_per_match", "strikes_per_match", "refuels_per_match")
BOUGHT = {"germany": ["ger_sdkfz251_1_squad", "ger_stug3_g_squad", "ger_pz4_h_squad"],
          "usa": ["usa_m3a1_squad", "usa_m10_squad", "usa_m4a3_75_squad"]}


DEFAULT_MAP = "ww2:kessel_ridge"
DEFAULT_PAIR = "germany-usa"


def read(d):
    """One pooled block per map, in first-seen order."""
    groups = {}
    for s in SETS:
        p = os.path.join(d, s + ".json")
        if not os.path.exists(p):
            continue
        j = json.load(open(p))
        m = j.get("map") or DEFAULT_MAP
        # S15 merge: a set of another faction pair (a matchup) pools apart.
        pair = "-".join(sorted(j.get("factions", {}))) or DEFAULT_PAIR
        k = (m, pair)
        out = groups.get(k)
        if out is None:
            out = groups[k] = {"dir": d, "map": m, "pair": pair, "sets": [], "n": 0, "ger": 0,
                               "nowin": 0, "he": [], "he_cas": [], "f": {}}
        add(out, j)
    if not groups:
        groups[(DEFAULT_MAP, DEFAULT_PAIR)] = {"dir": d, "map": DEFAULT_MAP, "pair": DEFAULT_PAIR,
                                               "sets": [], "n": 0, "ger": 0, "nowin": 0, "he": [],
                                               "he_cas": [], "f": {}}
    return list(groups.values())


def add(out, j):
    """Adds one set's JSON to a pooled block."""
    n = j["matches"]
    w = j["wins"]
    g = w.get("germany", 0)
    out["sets"].append(100.0 * g / n)
    out["n"] += n
    out["ger"] += g
    out["nowin"] += sum(1 for m in j["per_match"] if m.get("result") != "won")
    out["he"].append(j["he_share_pct"]["mean"])
    if "s12_derived" in j:
        out["he_cas"].append(j["s12_derived"]["he_share_casualties_pct"]["mean"])
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
        d12 = f.get("s12_derived")
        if d12 is not None:
            for k in S12:
                a.setdefault("s12", {}).setdefault(k, []).append(d12.get(k))
        d14 = f.get("s14_derived")
        if d14 is not None:
            for k in S14:
                a.setdefault("s14", {}).setdefault(k, []).append(d14.get(k))
        d10b = f.get("s10b_ai_derived")
        if d10b is not None:
            for k in S10B:
                a.setdefault("s10b", {}).setdefault(k, []).append(d10b.get(k))


def fmt(xs, nd=1):
    # A share over an empty set is null in the JSON (S10a-ai fix F10): "-".
    return " / ".join("-" if x is None else f"{x:.{nd}f}" for x in xs)


def show(o, base):
    if not o["n"]:
        print(f"{o['dir']}: no set found")
        return
    pool = 100.0 * o["ger"] / o["n"]
    delta = f" ({pool - 100.0 * base['ger'] / base['n']:+.1f} against the baseline)" if base and base["n"] else ""
    tags = ([] if o["map"] == DEFAULT_MAP else [o["map"]]) + ([] if o["pair"] == DEFAULT_PAIR else [o["pair"]])
    label = f"{o['dir']} [{', '.join(tags)}]" if tags else o["dir"]
    print(f"{label}: Germany pooled {pool:.2f} % of {o['n']}{delta}; sets {fmt(o['sets'], 2)}; "
          f"no winner {o['nowin']}; HE share {fmt(o['he'])}"
          + (f"; HE share of casualties {fmt(o['he_cas'])} (T8 cap 26.3)" if o["he_cas"] else ""))
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
        if a.get("s10b"):
            sb = a["s10b"]
            print(f"    s10b-ai: mines in {fmt(sb['matches_with_mines_pct'], 0)} %, obstacles in "
                  f"{fmt(sb['matches_with_obstacle_pct'], 0)} %, bunker in {fmt(sb['matches_with_bunker_pct'], 0)} % of matches "
                  f"(done when started {fmt(sb['bunker_done_when_started_pct'], 0)} %), sweep when wanted {fmt(sb['sweep_when_wanted_pct'], 0)} %; "
                  f"own-side mistakes {fmt(sb['own_side_mistakes'], 0)} (must be 0)")
        if a.get("s12"):
            s12 = a["s12"]
            print(f"    s12: medic matches {fmt(s12['medic_matches_pct'], 0)} % (floor 60), downs stabilised "
                  f"{fmt(s12['stabilised_pct'])} %, bled out {fmt(s12['bled_out_pct'])} %, downs per match "
                  f"{fmt(s12['downed_per_match'])}, replacements per match {fmt(s12['reinforced_men_per_match'])}, "
                  f"HE share of its casualties {fmt(s12['he_share_casualties_pct'])} %")
        if a.get("s14"):
            s14 = a["s14"]
            print(f"    s14: flame hit a trench or pit in {fmt(s14['flame_matches_pct'], 0)} % of matches, flame kills "
                  f"{fmt(s14['flame_kills_per_match'])}, flame jobs {fmt(s14['flame_jobs_per_match'])}, AI clears "
                  f"{fmt(s14['clear_orders_per_match'])}, strikes {fmt(s14['strikes_per_match'])}, refuels "
                  f"{fmt(s14['refuels_per_match'])} per match (reports)")
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
    base = {}
    for d in dirs:
        for o in read(d):
            show(o, base.get((o["map"], o["pair"])))
            base.setdefault((o["map"], o["pair"]), o)


if __name__ == "__main__":
    main()
