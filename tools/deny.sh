#!/usr/bin/env bash
# Licence / bans / source check. Uses cargo-deny when available; otherwise
# falls back to a cargo-metadata scan against the allowlist in deny.toml
# (licences only; no bans, sources or advisories).
#
#   tools/deny.sh            # cargo deny check licenses bans sources
#   tools/deny.sh advisories # extra args go to cargo-deny (needs network)
set -euo pipefail
cd "$(dirname "$0")/.."

if command -v cargo-deny >/dev/null 2>&1; then
    if [ "$#" -gt 0 ]; then
        exec cargo deny check "$@"
    fi
    exec cargo deny check licenses bans sources
fi

echo "cargo-deny not found; running fallback licence scan (licences only)" >&2
exec python3 -I - <<'PY'
import json, re, subprocess, sys

# Parse [licenses] allow = [...] without tomllib (Python 3.9 compatible).
cfg = re.sub(r"#[^\n]*", "", open("deny.toml", encoding="utf-8").read())
m = re.search(r"\[licenses\][^\[]*?allow\s*=\s*\[(.*?)\]", cfg, re.S)
if not m:
    sys.exit("deny.toml: no [licenses] allow list found")
allow = set(re.findall(r'"([^"]+)"', m.group(1)))

r = subprocess.run(["cargo", "metadata", "--format-version", "1"],
                   capture_output=True, text=True)
if r.returncode != 0:
    sys.stderr.write(r.stderr)
    sys.exit(r.returncode)
meta = json.loads(r.stdout)

def allowed(expr):
    # Accept if any OR-alternative has all of its AND-terms allowed.
    # "Apache-2.0 WITH LLVM-exception" is matched as one allowlist entry.
    for alt in re.split(r"\s+OR\s+|/", expr):
        terms = [t.strip(" ()") for t in re.split(r"\s+AND\s+", alt.strip(" ()"))]
        if terms and all(t in allow for t in terms):
            return True
    return False

bad = []
for p in meta["packages"]:
    if p["source"] is None:
        continue  # workspace crate (private, see [licenses.private])
    lic = p.get("license") or ""
    if not lic or not allowed(lic):
        bad.append("%s %s: %s" % (p["name"], p["version"], lic or "<none>"))
if bad:
    print("Disallowed or unknown licences:\n  " + "\n  ".join(sorted(bad)))
    sys.exit(1)
print("licence scan OK (%d third-party packages)" %
      sum(1 for p in meta["packages"] if p["source"] is not None))
PY
