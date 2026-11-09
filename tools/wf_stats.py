#!/usr/bin/env python3
"""Where a workflow run's time and tokens went (docs/process/dev-speed.md).

  python3 -I tools/wf_stats.py DIR [--cmds N]

DIR is a workflow's transcript directory (the one holding journal.jsonl and
agent-*.jsonl, under ~/.claude/projects/<project>/<session>/subagents/workflows/).
Per agent: wall time, model turns, tokens (cache write, cache read, output),
how much of the cache write came after a wait longer than the 5-minute cache
life ("re-cached"), and the wall time of its shell calls by kind. --cmds N
also lists each agent's N longest commands. "Cost units" weigh the tokens as
the price list does: cache write 1.25, cache read 0.1, output 5, input 1.
"""
import json, sys, glob, os, re
from datetime import datetime
from collections import defaultdict

KINDS = [
    ("autotune", r"autotune"),
    ("batch (skirmish)", r"skirmish"),
    ("targets", r"frontline-cli targets|cli targets| targets docs"),
    ("armour/fortify/bench", r"armour-test|fortify-test|mortar-bench"),
    ("release tests", r"cargo test[^|;&]*--release|--include-ignored"),
    ("check.sh", r"check\.sh"),
    ("cargo test (debug)", r"cargo test"),
    ("cargo build/clippy/fmt", r"cargo (build|clippy|fmt|check)|build_bridge"),
    ("godot", r"[Gg]odot"),
    ("wait/sleep", r"\bsleep\b|\bwait\b|until |while "),
]

def ts(s):
    return datetime.fromisoformat(s.replace("Z", "+00:00"))

def kind(cmd):
    for name, pat in KINDS:
        if re.search(pat, cmd):
            return name
    return "other shell"

def main():
    d = sys.argv[1]
    ncmd = int(sys.argv[sys.argv.index("--cmds") + 1]) if "--cmds" in sys.argv else 0
    tot = defaultdict(float)
    rows = []
    for f in sorted(glob.glob(os.path.join(d, "agent-*.jsonl")), key=os.path.getmtime):
        meta = {}
        mp = f.replace(".jsonl", ".meta.json")
        if os.path.exists(mp):
            meta = json.load(open(mp))
        first = last = None
        turns = 0
        tok = defaultdict(int)
        seen = set()
        uses = {}
        by = defaultdict(float)
        cmds = []
        bg = 0
        prev_turn = None
        used_model = ""
        for line in open(f):
            try:
                r = json.loads(line)
            except ValueError:
                continue
            t = r.get("timestamp")
            if not t:
                continue
            t = ts(t)
            first = first or t
            last = t
            m = r.get("message") or {}
            if r.get("type") == "assistant":
                mid = m.get("id")
                if mid not in seen:
                    seen.add(mid)
                    turns += 1
                    used_model = m.get("model") or used_model
                    u = m.get("usage") or {}
                    cw_, cr_ = u.get("cache_creation_input_tokens", 0), u.get("cache_read_input_tokens", 0)
                    if prev_turn and (t - prev_turn).total_seconds() > 300 and cr_ < cw_:
                        tok["recache"] += cw_
                        tok["recache_n"] += 1
                    prev_turn = t
                    tok["in"] += u.get("input_tokens", 0)
                    tok["cw"] += u.get("cache_creation_input_tokens", 0)
                    tok["cr"] += u.get("cache_read_input_tokens", 0)
                    tok["out"] += u.get("output_tokens", 0)
                for c in m.get("content") or []:
                    if isinstance(c, dict) and c.get("type") == "tool_use":
                        inp = c.get("input") or {}
                        uses[c["id"]] = (t, c.get("name"), inp.get("command", ""), inp.get("run_in_background"))
            elif r.get("type") == "user" and isinstance(m.get("content"), list):
                for c in m["content"]:
                    if isinstance(c, dict) and c.get("type") == "tool_result" and c.get("tool_use_id") in uses:
                        t0, name, cmd, isbg = uses.pop(c["tool_use_id"])
                        dt = (t - t0).total_seconds()
                        if name == "Bash":
                            if isbg:
                                bg += 1
                            k = kind(cmd)
                            by[k] += dt
                            cmds.append((dt, k, " ".join(cmd.split())[:110]))
                        else:
                            by["tool:" + str(name)] += dt
        if not first:
            continue
        wall = (last - first).total_seconds()
        tool = sum(by.values())
        label = meta.get("description", os.path.basename(f))
        model = (used_model or meta.get("pickedEffortModel") or "").replace("claude-", "")
        print(f"\n== {label} [{model}] wall {wall/60:.0f} min, turns {turns}, tools {tool/60:.0f} min, model {max(wall-tool,0)/60:.0f} min, bg cmds {bg}")
        units = tok["in"] + 1.25 * tok["cw"] + 0.1 * tok["cr"] + 5 * tok["out"]
        print(f"   tokens: cache-write {tok['cw']/1e3:.0f}k (re-cached after waits over 5 min: {tok['recache']/1e3:.0f}k in {tok['recache_n']} turns), "
              f"cache-read {tok['cr']/1e6:.1f}M, out {tok['out']/1e3:.0f}k; cost units {units/1e6:.1f}M")
        for k, v in sorted(by.items(), key=lambda x: -x[1]):
            if v >= 30:
                print(f"   {v/60:6.1f} min  {k}")
            tot[k] += v
        tot["_wall"] += wall
        tot["_model"] += max(wall - tool, 0)
        for k in tok:
            tot["_" + k] += tok[k]
        for dt, k, c in sorted(cmds, reverse=True)[:ncmd]:
            print(f"     {dt/60:5.1f}  {c}")
    print(f"\n== TOTAL wall {tot['_wall']/3600:.1f} h, model time {tot['_model']/3600:.1f} h")
    units = tot["_in"] + 1.25 * tot["_cw"] + 0.1 * tot["_cr"] + 5 * tot["_out"]
    print(f"   tokens: cache-write {tot['_cw']/1e6:.2f}M (re-cached {tot['_recache']/1e6:.2f}M in {tot['_recache_n']:.0f} turns), "
          f"cache-read {tot['_cr']/1e6:.1f}M, out {tot['_out']/1e3:.0f}k; cost units {units/1e6:.1f}M, "
          f"of which re-caching {1.25*tot['_recache']/units*100:.0f} %")
    for k, v in sorted(tot.items(), key=lambda x: -x[1]):
        if not k.startswith("_") and v >= 60:
            print(f"   {v/60:6.1f} min  {k}")

if __name__ == "__main__":
    main()
