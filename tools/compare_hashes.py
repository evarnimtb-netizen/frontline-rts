#!/usr/bin/env python3
"""Compare final state hashes across `frontline-cli simulate` reports (§5.11, §14).

CI runs the same scenario and seed on every OS and with 1/2/3/8 sim workers,
uploads each `--report` JSON, then calls this script on all of them. Any
difference in the final hash is a determinism bug and fails the job.

Usage:
    python3 -I tools/compare_hashes.py reports/            # every *.json under it
    python3 -I tools/compare_hashes.py a.json b.json --expect 12

The final hash is read from the first of these report fields that exists:
    final_hash | hash.final | hash (string or int) | state_hash
If the reports also carry a per-tick list (`hashes`, `tick_hashes` or
`hash.ticks`, as a list of values or of {tick, hash} objects), the first
diverging tick is printed to help bisect the desync.

Exit status: 0 all equal, 1 mismatch, 2 bad input (missing hash, too few
reports, unreadable file).
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


def norm(v) -> str:
    """Hashes may be ints or hex strings; compare a canonical zero-padded hex form."""
    if isinstance(v, bool):
        raise TypeError("boolean is not a hash")
    if isinstance(v, int):
        return f"{v:016x}"
    if isinstance(v, str):
        s = v.strip().lower()
        s = s[2:] if s.startswith("0x") else s
        try:
            return f"{int(s, 16):016x}"
        except ValueError:
            return s  # not hex: compare verbatim
    raise TypeError(f"unsupported hash type {type(v).__name__}")


def final_hash(report: dict):
    if "final_hash" in report:
        return report["final_hash"]
    h = report.get("hash")
    if isinstance(h, dict) and "final" in h:
        return h["final"]
    if isinstance(h, (str, int)) and not isinstance(h, bool):
        return h
    if "state_hash" in report:
        return report["state_hash"]
    raise KeyError("no final_hash / hash.final / hash / state_hash field")


def tick_hashes(report: dict) -> list[tuple[int, str]] | None:
    seq = report.get("hashes", report.get("tick_hashes"))
    if seq is None and isinstance(report.get("hash"), dict):
        seq = report["hash"].get("ticks")
    if not isinstance(seq, list):
        return None
    out = []
    for i, item in enumerate(seq):
        if isinstance(item, dict):
            out.append((int(item.get("tick", i)), norm(item.get("hash"))))
        else:
            out.append((i, norm(item)))
    return out


def describe(report: dict, path: Path) -> str:
    """Short label: OS / threads from the report if present, else the filename."""
    parts = []
    for key in ("os", "platform", "target"):
        if key in report:
            parts.append(str(report[key]))
            break
    for key in ("threads", "workers"):
        if key in report:
            parts.append(f"threads={report[key]}")
            break
    return f"{path.name} ({', '.join(parts)})" if parts else path.name


def collect(paths: list[str]) -> list[Path]:
    files: list[Path] = []
    for p in map(Path, paths):
        if p.is_dir():
            files.extend(sorted(p.rglob("*.json")))
        else:
            files.append(p)
    return files


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("paths", nargs="+", help="report JSON files or directories")
    ap.add_argument("--expect", type=int, default=2,
                    help="minimum number of reports required (default 2)")
    args = ap.parse_args()

    files = collect(args.paths)
    if len(files) < args.expect:
        print(f"error: found {len(files)} report(s), expected at least {args.expect}",
              file=sys.stderr)
        for f in files:
            print(f"  {f}", file=sys.stderr)
        return 2

    rows = []  # (label, final, ticks)
    for f in files:
        try:
            report = json.loads(f.read_text(encoding="utf-8"))
            rows.append((describe(report, f), norm(final_hash(report)), tick_hashes(report)))
        except (OSError, ValueError, KeyError, TypeError) as e:
            print(f"error: {f}: {e}", file=sys.stderr)
            return 2

    width = max(len(r[0]) for r in rows)
    for label, h, _ in rows:
        print(f"{label:<{width}}  {h}")

    distinct = sorted({h for _, h, _ in rows})
    if len(distinct) == 1:
        print(f"\nOK: {len(rows)} reports share final hash {distinct[0]}")
        return 0

    print(f"\nMISMATCH: {len(distinct)} distinct final hashes across {len(rows)} reports")
    # Group by hash; the largest group is treated as the reference.
    groups: dict[str, list[str]] = {}
    for label, h, _ in rows:
        groups.setdefault(h, []).append(label)
    ref_hash = max(groups, key=lambda h: (len(groups[h]), h))
    for h, labels in sorted(groups.items(), key=lambda kv: -len(kv[1])):
        tag = "reference" if h == ref_hash else "DIVERGES"
        print(f"  [{tag}] {h}: {', '.join(labels)}")

    ref = next(r for r in rows if r[1] == ref_hash)
    if ref[2]:
        ref_ticks = dict(ref[2])
        for label, h, ticks in rows:
            if h == ref_hash or not ticks:
                continue
            first = next((t for t, th in ticks if t in ref_ticks and ref_ticks[t] != th), None)
            if first is not None:
                print(f"  {label}: first diverging tick {first}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
