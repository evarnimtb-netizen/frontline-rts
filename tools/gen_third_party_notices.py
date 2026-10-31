#!/usr/bin/env python3
"""Generate THIRD_PARTY_NOTICES for the Rust workspace (§7.9).

Reads `cargo metadata --format-version 1`, walks the dependency graph from the
workspace members, and for every third-party crate records its name, version,
SPDX licence expression, repository and the licence/notice files shipped in
its package (found next to its Cargo.toml in the local cargo registry source).

Identical licence texts (after whitespace normalisation) are printed once and
referenced by number, so the dual MIT/Apache-2.0 boilerplate does not repeat
sixty times. MIT/BSD texts usually differ by copyright line and so stay
separate, which is what their licences require.

Usage (from anywhere; run with `python3 -I`):
    python3 -I tools/gen_third_party_notices.py                # writes ./THIRD_PARTY_NOTICES
    python3 -I tools/gen_third_party_notices.py --out -        # stdout
    python3 -I tools/gen_third_party_notices.py --locked --include-dev

Dev-only dependencies are excluded by default because they never ship.
Build and proc-macro dependencies are included (conservative: they shape the
shipped binary). Packages for every target platform are included, since one
notices file covers all of our builds.

Exit status: 0 on success, 1 if cargo metadata fails. Crates with no licence
file found are listed in the output and reported on stderr, but do not fail.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

# File names (case-insensitive prefixes) treated as licence or notice texts.
LICENSE_PREFIXES = ("license", "licence", "copying", "notice", "unlicense", "copyright")
# Skip obvious non-text siblings such as LICENSE.tpl generators or scripts.
SKIP_SUFFIXES = (".rs", ".py", ".sh", ".toml", ".json", ".yml", ".yaml", ".tpl")
MAX_TEXT_BYTES = 256 * 1024


def cargo_metadata(manifest: Path, locked: bool) -> dict:
    cmd = ["cargo", "metadata", "--format-version", "1", "--manifest-path", str(manifest)]
    if locked:
        cmd.append("--locked")
    try:
        out = subprocess.run(cmd, check=True, capture_output=True, text=True)
    except FileNotFoundError:
        sys.exit("error: cargo not found on PATH")
    except subprocess.CalledProcessError as e:
        sys.stderr.write(e.stderr)
        sys.exit(1)
    return json.loads(out.stdout)


def shipped_package_ids(meta: dict, include_dev: bool) -> set[str]:
    """Package ids reachable from workspace members, skipping dev-only edges."""
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    workspace = set(meta["workspace_members"])
    seen: set[str] = set()
    stack = list(workspace)
    while stack:
        pid = stack.pop()
        if pid in seen:
            continue
        seen.add(pid)
        for dep in nodes[pid].get("deps", []):
            kinds = {k.get("kind") for k in dep.get("dep_kinds", [])}
            # kind is None for normal deps, "build" or "dev" otherwise.
            if not include_dev and kinds and kinds <= {"dev"}:
                continue
            stack.append(dep["pkg"])
    return seen - workspace


def find_license_files(pkg: dict) -> list[Path]:
    pkg_dir = Path(pkg["manifest_path"]).parent
    found: dict[Path, None] = {}
    if pkg.get("license_file"):
        lf = (pkg_dir / pkg["license_file"]).resolve()
        if lf.is_file():
            found[lf] = None
    try:
        entries = sorted(pkg_dir.iterdir(), key=lambda p: p.name.lower())
    except OSError:
        entries = []
    for p in entries:
        name = p.name.lower()
        if not p.is_file() or name.endswith(SKIP_SUFFIXES):
            continue
        if name.startswith(LICENSE_PREFIXES):
            found[p.resolve()] = None
    # Some crates keep texts in a LICENSES/ directory (REUSE layout).
    for sub in ("LICENSES", "licenses"):
        d = pkg_dir / sub
        if d.is_dir():
            for p in sorted(d.iterdir(), key=lambda p: p.name.lower()):
                if p.is_file():
                    found[p.resolve()] = None
    return list(found)


def read_text(path: Path) -> str | None:
    try:
        data = path.read_bytes()[:MAX_TEXT_BYTES]
    except OSError:
        return None
    text = data.decode("utf-8", errors="replace").replace("\r\n", "\n").strip("\n")
    return text or None


def normalise(text: str) -> str:
    return re.sub(r"\s+", " ", text).strip().lower()


def build(meta: dict, include_dev: bool) -> tuple[str, list[str]]:
    by_id = {p["id"]: p for p in meta["packages"]}
    ids = shipped_package_ids(meta, include_dev)
    pkgs = sorted((by_id[i] for i in ids), key=lambda p: (p["name"], p["version"]))

    texts: list[tuple[str, list[str]]] = []  # (text, [crate@ver, ...])
    text_index: dict[str, int] = {}  # normalised hash -> index into texts
    entries = []
    missing: list[str] = []

    for p in pkgs:
        label = f"{p['name']} {p['version']}"
        refs = []
        for f in find_license_files(p):
            body = read_text(f)
            if body is None:
                continue
            key = hashlib.sha256(normalise(body).encode()).hexdigest()
            if key not in text_index:
                text_index[key] = len(texts)
                texts.append((body, []))
            idx = text_index[key]
            texts[idx][1].append(label)
            refs.append((f.name, idx + 1))
        if not refs:
            missing.append(label)
        entries.append((p, refs))

    out: list[str] = []
    w = out.append
    w("THIRD-PARTY SOFTWARE NOTICES")
    w("=" * 28)
    w("")
    w("Frontline RTS includes the third-party Rust crates listed below. Each is")
    w("used under the licence shown (an SPDX expression; for `A OR B` we use the")
    w("crate under either). Licence and notice texts follow, numbered [T#].")
    w("")
    w("Generated by tools/gen_third_party_notices.py from `cargo metadata`;")
    w("do not edit by hand. Non-Rust components (Godot, Luau, fonts, the")
    w("Steamworks redistributables) are listed separately (§7.9).")
    w("")
    w(f"Crates: {len(entries)}    Unique licence texts: {len(texts)}")
    w("")
    w("-" * 78)
    w("CRATES")
    w("-" * 78)
    for p, refs in entries:
        w("")
        w(f"{p['name']} {p['version']}")
        w(f"  Licence:    {p.get('license') or '(none declared; see texts)'}")
        if p.get("repository"):
            w(f"  Repository: {p['repository']}")
        src = p.get("source") or ""
        if src.startswith("registry+"):
            w(f"  Source:     https://crates.io/crates/{p['name']}/{p['version']}")
        elif src:
            w(f"  Source:     {src}")
        if refs:
            w("  Texts:      " + ", ".join(f"{name} [T{i}]" for name, i in refs))
        else:
            w("  Texts:      NOT FOUND in package; the standard text for the SPDX")
            w("              licence above applies.")
    w("")
    w("-" * 78)
    w("LICENCE AND NOTICE TEXTS")
    w("-" * 78)
    for i, (body, users) in enumerate(texts, start=1):
        w("")
        w(f"[T{i}] used by: {', '.join(users)}")
        w("." * 78)
        w(body)
        w("." * 78)
    w("")
    return "\n".join(out), missing


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--manifest-path", type=Path, default=REPO_ROOT / "Cargo.toml")
    ap.add_argument("--out", default=str(REPO_ROOT / "THIRD_PARTY_NOTICES"),
                    help="output file, or '-' for stdout")
    ap.add_argument("--locked", action="store_true", help="pass --locked to cargo metadata")
    ap.add_argument("--include-dev", action="store_true", help="include dev-only dependencies")
    args = ap.parse_args()

    meta = cargo_metadata(args.manifest_path, args.locked)
    text, missing = build(meta, args.include_dev)

    if args.out == "-":
        sys.stdout.write(text)
    else:
        tmp = Path(args.out + ".tmp")
        with open(tmp, "w", encoding="utf-8", newline="\n") as f:
            f.write(text)
        os.replace(tmp, args.out)
        crates = text.count("\n  Licence:    ")
        print(f"wrote {args.out} ({crates} crates, {len(text.encode())} bytes)", file=sys.stderr)
    if missing:
        print(f"warning: no licence file found for {len(missing)} crate(s): "
              + ", ".join(missing), file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
