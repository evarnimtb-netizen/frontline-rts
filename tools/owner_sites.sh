#!/usr/bin/env bash
# S17a (docs/slices/S17-team-play.md 10, 11.1): lists every raw owner
# comparison and sight-bit read in non-test sim code, one `file:line: text`
# per site, so a review sees a new one. `docs/slices/S17-sites.md` classes
# them (Own, Hostile, Sees); a Hostile or Sees site asks `crate::teams`.
#
#   tools/owner_sites.sh            # every site
#   tools/owner_sites.sh --count    # counts only (owner, sight)
#   tools/owner_sites.sh --check    # sites whose text the table lacks (exit 1)
#
# --check (S17b, plan 11.1; run by `tools/check.sh quick`): a site is known
# when `docs/slices/S17-sites.md` has a row for its file with the same text
# (line numbers may move). A new raw owner comparison or sight read makes the
# step fail until it is classed in the table.
#
# Test files (`*_tests.rs`, `scenarios.rs`) and `#[cfg(test)] mod tests`
# blocks at the end of a file are left out. The patterns are textual: a
# comparison written another way is not listed.
set -u
cd "$(dirname "$0")/../crates/sim/src"
files=$(ls ./*.rs ai/*.rs | grep -v '_tests\.rs$' | grep -v '^./scenarios\.rs$')
owner_re='(owner|player|\bme\b|\bp\b)[^;]*[!=]= |[!=]= [^;]*(owner|player|\bme\b)'
sight_re='seen_by'
list() { # PATTERN
    for f in $files; do
        # Stop at the first `#[cfg(test)]` that opens a test module.
        awk -v re="$1" -v f="${f#./}" '
            /^#\[cfg\(test\)\]/ { t = 1 }
            t == 0 && $0 ~ re && $0 !~ /^[[:space:]]*\/\// { printf "%s:%d: %s\n", f, NR, $0 }
        ' "$f"
    done
}
if [ "${1:-}" = "--check" ]; then
    table="$(cd ../../.. && pwd)/docs/slices/S17-sites.md"
    { list "$owner_re" | grep -iE 'owner|player|\bme\b'; list "$sight_re"; } | sed 's/  */ /g' |
        python3 -I -c '
import re, sys
known = set()
for row in open(sys.argv[1]):
    m = re.match(r"\| `([^:`]+):\d+` \| [^|]+ \| `(.*)` \|$", row.rstrip("\n"))
    if m:
        known.add((m.group(1), m.group(2).replace("\\|", "|").strip()))
# A long text is cut in the table and ends with "...": a prefix match.
cut = {(f, t[:-3]) for f, t in known if t.endswith("...")}
new = []
for line in sys.stdin:
    f, _, text = line.rstrip("\n").split(":", 2)
    text = text.strip()
    if (f, text) not in known and not any(f == g and text.startswith(t) for g, t in cut):
        new.append(line.rstrip())
for n in new:
    print("unclassified:", n)
print(f"owner sites: {len(new)} not in the table")
sys.exit(1 if new else 0)
' "$table"
    exit $?
elif [ "${1:-}" = "--count" ]; then
    o=$(list "$owner_re" | grep -ciE 'owner|player')
    s=$(list "$sight_re" | wc -l)
    echo "owner $o sight $s"
else
    list "$owner_re" | grep -iE 'owner|player|\bme\b' | sed 's/  */ /g'
    list "$sight_re" | sed 's/  */ /g'
fi
