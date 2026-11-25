#!/usr/bin/env bash
# S17a (docs/slices/S17-team-play.md 10, 11.1): lists every raw owner
# comparison and sight-bit read in non-test sim code, one `file:line: text`
# per site, so a review sees a new one. `docs/slices/S17-sites.md` classes
# them (Own, Hostile, Sees); a Hostile or Sees site asks `crate::teams`.
#
#   tools/owner_sites.sh            # every site
#   tools/owner_sites.sh --count    # counts only (owner, sight)
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
if [ "${1:-}" = "--count" ]; then
    o=$(list "$owner_re" | grep -ciE 'owner|player')
    s=$(list "$sight_re" | wc -l)
    echo "owner $o sight $s"
else
    list "$owner_re" | grep -iE 'owner|player|\bme\b' | sed 's/  */ /g'
    list "$sight_re" | sed 's/  */ /g'
fi
