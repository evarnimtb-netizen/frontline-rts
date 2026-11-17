#!/usr/bin/env python3
"""Neutrality check of a mechanics slice (docs/slices/S10a-fortifications.md 8).

    python3 -I tools/neutral.py BASE_DIR NEW_DIR

Compares the CLI JSON files present in both directories:

* skirmish sets (skirmish40/160/160_oos/160_s201.json): the `per_match`
  records keyed by (seed, side) must be equal in every field but `hash`;
  the aggregate blocks equal recursively, ignoring tick_p99_ms, jobs,
  schema and wall times. A key only in NEW must be 0, empty or false (new
  counters that stay zero); a key only in BASE fails. S10c-ai (Build A):
  a key starting `s10c` that BASE lacks is skipped (its observer counters
  read state, so they are not zero with the switches off); when both have
  it, it is compared like any other. S11a1: a key starting `s11a1` that
  BASE lacks must be 0 in every counter (review B-4); only its denominator
  keys (`S11A1_DENOMS`: matches, kills, income) may be non-zero. A list
  under the key `refused` that grew is equal when the new tail is zero
  (`s5b.refused` 7 -> 9 items; review B-3: no other list). The set header's
  `overrides` list (the `-o` files a run was given) is not part of the
  verdict: when it differs the line gets a note `(note: overrides N -> M
  items)`.
* any other JSON in both (fortify-test, armour-test): every BASE value
  equal (keys only in NEW are allowed: new rows).

A skirmish set in BASE but missing from NEW fails (`X: only in BASE`). Other
JSON is compared only when present in both.

Prints one line per file (`same 40/40` or the first 5 differences) and exits
0 only when every file compared is equal (and at least one was compared).
"""

import json
import os
import sys

SKIRMISH = ["skirmish40.json", "skirmish160.json", "skirmish160_oos.json", "skirmish160_s201.json"]
IGNORED = {"tick_p99_ms", "jobs", "schema"}
MATCH_IGNORED = {"hash"}
# Set-header fields that record how a run was made, not what it did: a
# difference is printed as a note, never counted.
HEADER_NOTED = {"overrides"}
# S11a1 (review B-4): the keys of an `s11a1` block that hold denominators,
# not counters; every other value there must be blank in an all-off run.
S11A1_DENOMS = {"matches", "kills", "income_mp", "gun_kills_in_tank_he"}
# S11a1 (review B-3): the only lists that may grow with a zero tail.
GROWN_LISTS = {"refused"}


def ignored(key):
    return key in IGNORED or "wall" in key


def is_blank(v):
    """A value a new counter may hold when nothing new happened."""
    if isinstance(v, bool):
        return v is False
    if isinstance(v, (int, float)):
        return v == 0
    if v is None:
        return True
    if isinstance(v, str):
        return v == ""
    if isinstance(v, list):
        return all(is_blank(x) for x in v)
    if isinstance(v, dict):
        return all(is_blank(x) for x in v.values())
    return False


def blank_but_denoms(v):
    """An `s11a1` block with every counter blank (denominators aside)."""
    if isinstance(v, dict):
        return all(k in S11A1_DENOMS or blank_but_denoms(x) for k, x in v.items())
    return is_blank(v)


def diff(base, new, path, out, strict, skip=frozenset()):
    """Appends `path: base -> new` lines to `out`."""
    if isinstance(base, dict) and isinstance(new, dict):
        for k in sorted(set(base) | set(new)):
            if k in skip or ignored(k):
                continue
            p = f"{path}.{k}" if path else k
            if k not in new:
                out.append(f"{p}: only in BASE")
            elif k not in base:
                if k.startswith("s10c"):
                    continue
                if k.startswith("s11a1"):
                    if strict and not blank_but_denoms(new[k]):
                        out.append(f"{p}: only in NEW and a counter is not zero ({json.dumps(new[k])[:60]})")
                    continue
                if strict and not is_blank(new[k]):
                    out.append(f"{p}: only in NEW and not zero ({json.dumps(new[k])[:60]})")
            else:
                diff(base[k], new[k], p, out, strict, skip)
        return
    if isinstance(base, list) and isinstance(new, list):
        # S11a1 B: a counter array that grew (e.g. `s5b.refused`, 7 -> 9
        # refusal kinds) is equal when the new tail is all zero.
        grown = path.rsplit(".", 1)[-1] in GROWN_LISTS
        if grown and len(new) > len(base) and all(is_blank(x) for x in new[len(base):]):
            new = new[: len(base)]
        if len(base) != len(new) and (strict or len(new) < len(base)):
            out.append(f"{path}: {len(base)} items -> {len(new)}")
            return
        for i, b in enumerate(base):
            diff(b, new[i], f"{path}[{i}]", out, strict, skip)
        return
    if type(base) is not type(new) and not (
        isinstance(base, (int, float)) and isinstance(new, (int, float))
        and not isinstance(base, bool) and not isinstance(new, bool)
    ):
        out.append(f"{path}: {json.dumps(base)[:40]} -> {json.dumps(new)[:40]}")
    elif base != new:
        out.append(f"{path}: {json.dumps(base)[:40]} -> {json.dumps(new)[:40]}")


def header_notes(base, new):
    """Notes for set-header fields in HEADER_NOTED that differ."""
    notes = []
    for k in sorted(HEADER_NOTED):
        b, n = base.get(k), new.get(k)
        if b != n:
            size = lambda v: len(v) if isinstance(v, list) else (0 if v is None else 1)
            notes.append(f"{k} {size(b)} -> {size(n)} items")
    return notes


def skirmish(base, new):
    """(same, total, differences) of two skirmish JSON documents."""
    out = []
    key = lambda m: (m.get("seed"), m.get("side"))
    bm = {key(m): m for m in base.get("per_match", [])}
    nm = {key(m): m for m in new.get("per_match", [])}
    same = 0
    for k in sorted(bm, key=str):
        if k not in nm:
            out.append(f"match {k}: only in BASE")
            continue
        d = []
        diff(bm[k], nm[k], f"match {k[0]}/{k[1]}", d, True, MATCH_IGNORED)
        if d:
            out.extend(d)
        else:
            same += 1
    for k in sorted(set(nm) - set(bm), key=str):
        out.append(f"match {k}: only in NEW")
    agg_b = {k: v for k, v in base.items() if k != "per_match" and k not in HEADER_NOTED}
    agg_n = {k: v for k, v in new.items() if k != "per_match" and k not in HEADER_NOTED}
    diff(agg_b, agg_n, "", out, True)
    return same, len(bm), out


def compare(base_dir, new_dir):
    """Lines to print and whether everything was equal."""
    lines, ok, n = [], True, 0
    names = sorted(
        f for f in os.listdir(base_dir)
        if f.endswith(".json") and os.path.isfile(os.path.join(new_dir, f))
    )
    for f in names:
        with open(os.path.join(base_dir, f)) as fh:
            b = json.load(fh)
        with open(os.path.join(new_dir, f)) as fh:
            m = json.load(fh)
        n += 1
        if f in SKIRMISH or (isinstance(b, dict) and "per_match" in b):
            same, total, d = skirmish(b, m)
            notes = header_notes(b, m)
            note = f" (note: {'; '.join(notes)})" if notes else ""
            if d:
                ok = False
                lines.append(f"{f}: DIFFERENT, same {same}/{total}{note}; first differences:")
                lines.extend(f"    {x}" for x in d[:5])
            else:
                lines.append(f"{f}: same {same}/{total}{note}")
        else:
            d = []
            diff(b, m, "", d, False)
            if d:
                ok = False
                lines.append(f"{f}: DIFFERENT; first differences:")
                lines.extend(f"    {x}" for x in d[:5])
            else:
                lines.append(f"{f}: same (every BASE row)")
    # A skirmish set measured for BASE must be in NEW too (a crashed or
    # unfinished batch is not a pass).
    for f in SKIRMISH:
        if os.path.isfile(os.path.join(base_dir, f)) and not os.path.isfile(
            os.path.join(new_dir, f)
        ):
            lines.append(f"{f}: only in BASE (missing from NEW)")
            ok = False
    if n == 0:
        lines.append("no JSON file in both directories")
        ok = False
    return lines, ok


def main(argv):
    if len(argv) != 3:
        print(__doc__.strip().splitlines()[2].strip(), file=sys.stderr)
        return 2
    lines, ok = compare(argv[1], argv[2])
    for line in lines:
        print(line)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
