#!/usr/bin/env python3
"""Why AI guns do or do not put tanks out of action (S9b diagnostics).

  python3 -I tools/gun_funnel.py TRACE.jsonl [TRACE.jsonl ...]

Reads `frontline-cli skirmish --gun-trace` files and prints, per faction and
summed over the matches, the funnel from "an enemy tank exists" to "a gun put
it out of action", the targets of the guns' rounds, how long guns stand
ready, and how far from the nearest ready enemy gun tanks are lost.

"Ready" is an emplaced gun with at least two men. "Armour" is a tank, assault
gun or tank destroyer. A loss is a knock-out or a bail-out; it is the gun's
when the last round that struck the vehicle was a gun's (a gun of the
attacker fired that ammunition at that vehicle within 3 s before the impact).
"""
import json
import math
import sys
from collections import Counter, defaultdict

REACH_M = 400          # [ai.guns] engage_still_m
ARMOUR = {"Tank", "AssaultGun", "TankDestroyer"}
NONE = 4294967295
EMPLACED = 2           # GunState::Emplaced + 1
STATES = ["-", "limbered", "emplaced", "emplacing", "packing", "pushing", "hitched", "hitching", "unhitching"]


def run(path, tot):
    types = players = None
    hz = 30
    owner = {}        # vehicle id -> owner
    vtype = {}        # vehicle id -> type index
    last_pos = {}     # vehicle id -> (x, y)
    ready_guns = {}   # gun id -> (x, y) while ready at the last sample
    shots = defaultdict(list)   # (attacker, ammo, target) -> [ticks] of gun rounds
    last_hit_by_gun = {}        # victim -> bool
    seen_armour = defaultdict(set)   # enemy faction index -> armour ids seen alive
    stage = defaultdict(lambda: defaultdict(set))  # gun owner -> stage -> armour ids
    min_d = {}        # (gun owner, armour id) -> closest approach to a ready gun
    for line in open(path):
        e = json.loads(line)
        ev = e["ev"]
        if ev == "setup":
            types, players, hz = e["types"], e["players"], e["hz"]
            continue
        role = lambda v: types[vtype[v]]["role"] if v in vtype else "Soldier"
        fac = lambda p: players[p].split(":")[-1] if 0 <= p < len(players) else "?"
        if ev == "s":
            ready_guns = {}
            for vid, o, ty, x, y, yaw, fl, gs, gm, tg, seen, ammo in e["veh"]:
                owner[vid], vtype[vid], last_pos[vid] = o, ty, (x, y)
                r = types[ty]["role"]
                if r in ARMOUR:
                    seen_armour[o].add(vid)
                if gs:
                    f = fac(o)
                    tot[f]["gun_s"][STATES[gs] if gm >= 2 or gs != EMPLACED else "emplaced, under 2 men"] += 5
                    if gs == EMPLACED and gm >= 2:
                        ready_guns[vid] = (x, y, tg)
                        tot[f]["ready_s"] += 5
                        if ammo == 0:
                            tot[f]["ready_dry_s"] += 5
            exposed = defaultdict(lambda: [False] * 4)
            for gid, vid, d, off, los in e["pairs"]:
                if gid not in ready_guns or role(vid) not in ARMOUR:
                    continue
                go = owner[gid]
                key = (go, vid)
                min_d[key] = min(min_d.get(key, 1e9), d)
                if d > REACH_M:
                    continue
                arc = types[vtype[gid]]["arc"] or [-180, 180]
                in_arc = arc[0] <= off <= arc[1]
                st = stage[go]
                st["reach"].add(vid)
                x = exposed[go]
                x[0] = True
                if in_arc:
                    st["arc"].add(vid)
                    x[1] = True
                if in_arc and los:
                    st["los"].add(vid)
                    x[2] = True
                if ready_guns[gid][2] == vid:
                    st["aimed"].add(vid)
                    x[3] = True
            for go, x in exposed.items():
                for k, name in enumerate(("reach_s", "arc_s", "los_s", "aimed_s")):
                    tot[fac(go)][name] += 5 * x[k]
            continue
        v, a, t = e["v"], e["a"], e["t"]
        if ev == 0:  # Fired
            if v in vtype and types[vtype[v]]["role"] == "AtGun":
                f = fac(owner.get(v, e["o"]))
                tg = e["tg"]
                cls = "nothing" if tg in (None, NONE) else role(tg)
                if cls in ARMOUR:
                    cls = "armour"
                    stage[owner.get(v, e["o"])]["fired"].add(tg)
                tot[f]["rounds"][cls] += 1
                if tg in last_pos:
                    d = math.dist(last_pos[tg], e["p"])
                    tot[f]["range"][min(int(d // 100), 5)] += 1
                shots[(owner.get(v, e["o"]), e["am"], tg)].append(t)
        elif ev in (1, 2, 3):
            by_gun = any(t - 3 * hz <= s <= t for s in shots.get((a, e["am"], v), ()))
            last_hit_by_gun[v] = by_gun
            if by_gun and role(v) in ARMOUR:
                stage[a]["hit"].add(v)
                tot[fac(a)]["impact"][("ricochet", "no penetration", "penetration")[ev - 1]] += 1
                if ev == 3:
                    stage[a]["pen"].add(v)
        elif ev in (7, 9):
            r = role(v)
            if r == "AtGun":
                tot[fac(e["o"])]["gun_fate"]["knocked out" if ev == 7 else "left (neutral)"] += 1
            if r not in ARMOUR:
                continue
            loser = e["o"]
            f = fac(a) if a != 255 else "nobody"
            tot[fac(loser)]["armour_lost"] += 1
            if last_hit_by_gun.get(v) and a != 255:
                stage[a]["lost"].add(v)
                tot[fac(a)]["loss_kind"]["knock-out" if ev == 7 else "bail-out"] += 1
            # Where it was lost against the enemy's ready guns.
            p = last_pos.get(v)
            near = [math.dist(p, g[:2]) for gid, g in ready_guns.items() if owner[gid] != loser] if p else []
            band = "no ready gun" if not near else ("0-200 m" if min(near) <= 200 else "200-400 m" if min(near) <= 400 else "over 400 m")
            tot[fac(loser)]["lost_at"][band] += 1
            last_hit_by_gun.pop(v, None)
    if players is None:
        return
    for p in range(len(players)):
        f = fac(p)
        tot[f]["matches"] += 1
        enemy_armour = set().union(*(ids for o, ids in seen_armour.items() if o != p))
        tot[f]["stage"]["enemy armour fielded"] += len(enemy_armour)
        for name in ("reach", "arc", "los", "aimed", "fired", "hit", "pen", "lost"):
            tot[f]["stage"][name] += len(stage[p][name])
        for vid in enemy_armour:
            d = min_d.get((p, vid))
            band = "never within 600 m" if d is None else "under 200 m" if d <= 200 else "200-400 m" if d <= 400 else "400-600 m"
            tot[f]["closest"][band] += 1


def main():
    tot = defaultdict(lambda: defaultdict(Counter))
    for f in tot.values():
        pass
    files = sys.argv[1:]
    raw = defaultdict(lambda: {"gun_s": Counter(), "rounds": Counter(), "range": Counter(), "impact": Counter(),
                               "gun_fate": Counter(), "loss_kind": Counter(), "lost_at": Counter(),
                               "stage": Counter(), "closest": Counter(), "matches": 0, "ready_s": 0,
                               "ready_dry_s": 0, "reach_s": 0, "arc_s": 0, "los_s": 0, "aimed_s": 0, "armour_lost": 0})
    for p in files:
        run(p, raw)
    for f, d in sorted(raw.items()):
        n = max(d["matches"], 1)
        if not d["matches"]:
            continue
        print(f"== {f}: its guns against enemy armour, {d['matches']} matches")
        st = d["stage"]
        print("  funnel, enemy armoured vehicles per match (share of fielded):")
        base = max(st["enemy armour fielded"], 1)
        for name, label in (("enemy armour fielded", "fielded"), ("reach", f"within {REACH_M} m of a ready gun"),
                            ("arc", "... and inside its arc"), ("los", "... with a clear sight line"),
                            ("aimed", "... and the gun's target"), ("fired", "fired at"), ("hit", "struck"),
                            ("pen", "penetrated"), ("lost", "lost to a gun (knock-out or bail-out)")):
            print(f"    {label:42s} {st[name] / n:6.2f}  ({100 * st[name] / base:5.1f} %)")
        print("  closest approach of each enemy armoured vehicle to a ready gun: " +
              ", ".join(f"{k} {100 * v / base:.0f} %" for k, v in sorted(d["closest"].items())))
        r = max(d["ready_s"], 1)
        print(f"  ready gun time per match: {d['ready_s'] / n / 60:.1f} gun-min, with no rounds {100 * d['ready_dry_s'] / r:.0f} %; "
              f"armour in reach {100 * d['reach_s'] / r:.1f} % of it, in arc {100 * d['arc_s'] / r:.1f} %, "
              f"in sight {100 * d['los_s'] / r:.1f} %, aimed at {100 * d['aimed_s'] / r:.1f} %")
        g = sum(d["gun_s"].values()) or 1
        print("  gun time by state: " + ", ".join(f"{k} {100 * v / g:.0f} %" for k, v in d["gun_s"].most_common()))
        rs = sum(d["rounds"].values())
        print(f"  gun rounds per match {rs / n:.1f}: " + ", ".join(f"{k} {100 * v / max(rs, 1):.0f} %" for k, v in d["rounds"].most_common()))
        print("  range of gun rounds at vehicles: " + ", ".join(f"{100 * k}-{100 * k + 100} m {v}" for k, v in sorted(d["range"].items())))
        print("  gun rounds on armour: " + (", ".join(f"{k} {v}" for k, v in d["impact"].most_common()) or "none"))
        print("  armour lost to guns: " + (", ".join(f"{k} {v}" for k, v in d["loss_kind"].most_common()) or "none"))
        print("  own guns: " + (", ".join(f"{k} {v / n:.2f} per match" for k, v in d["gun_fate"].most_common()) or "none lost"))
        print(f"  own armour lost per match {d['armour_lost'] / n:.2f}; where, against the enemy's ready guns: " +
              ", ".join(f"{k} {v}" for k, v in d["lost_at"].most_common()))


if __name__ == "__main__":
    main()
