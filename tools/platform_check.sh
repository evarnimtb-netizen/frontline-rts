#!/usr/bin/env bash
# Manual platform check: the same steps as CI's `build-test` job, for an OS
# that CI does not run (macOS; SteamOS native and SteamOS + Proton, §5.11).
#
#   tools/platform_check.sh                  # build, test, simulate, compare
#   tools/platform_check.sh --quick          # skip the slow debug sim tests
#   tools/platform_check.sh --ci-reports DIR # also compare against CI reports
#
# What it checks:
#   1. `cargo build` and `cargo test --workspace --release` (the sim tests
#      hold a committed golden hash, so this already compares this machine
#      with every other OS);
#   2. the sim's tests again in a debug build (skipped with --quick);
#   3. `frontline-cli simulate` with CI's scenario, seed and ticks at 1, 2, 3
#      and 8 sim workers, and that all four final hashes are equal;
#   4. with --ci-reports, that they also equal the hashes in CI's reports.
#      Get those from the "sim-reports-*" artifacts of a CI run on the same
#      commit (Actions > the run > Artifacts), unzipped into DIR.
#
# Reports are written to build/platform-check/. Exit status 0 means every
# step passed. Keep scenario, seed and ticks equal to SIM_* in
# .github/workflows/ci.yml.
set -euo pipefail
cd "$(dirname "$0")/.."

SIM_SCENARIO="bench/scenarios/battalion.toml"
SIM_SEED="7"
SIM_TICKS="1800"

quick=0
ci_reports=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --quick) quick=1 ;;
        --ci-reports)
            [ "$#" -ge 2 ] || { echo "error: --ci-reports needs a directory" >&2; exit 2; }
            ci_reports="$2"
            shift
            ;;
        -h|--help) sed -n '2,23p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "error: unknown argument: $1 (see --help)" >&2; exit 2 ;;
    esac
    shift
done
if [ -n "$ci_reports" ] && [ ! -d "$ci_reports" ]; then
    echo "error: not a directory: $ci_reports" >&2
    exit 2
fi

step() { printf '\n== %s\n' "$*"; }

step "toolchain"
rustc --version
cargo --version

step "build (release)"
cargo build --workspace --release --locked

step "test (release)"
cargo test --workspace --release --locked

if [ "$quick" -eq 0 ]; then
    step "test sim (debug)"
    cargo test -p frontline-sim --locked
else
    step "test sim (debug): skipped (--quick)"
fi

out="build/platform-check"
rm -rf "$out"
mkdir -p "$out"
os="$(uname -s)-$(uname -m)"
for t in 1 2 3 8; do
    step "simulate threads=$t"
    cargo run --release --locked -p frontline-cli -- simulate \
        --scenario "$SIM_SCENARIO" \
        --ticks "$SIM_TICKS" \
        --seed "$SIM_SEED" \
        --threads "$t" \
        --report "$out/report-${os}-t${t}.json"
done

step "compare final hashes"
if [ -n "$ci_reports" ]; then
    # 4 local reports plus at least one from CI.
    python3 -I tools/compare_hashes.py "$out" "$ci_reports" --expect 5
else
    python3 -I tools/compare_hashes.py "$out" --expect 4
    echo
    echo "Compared this machine's four runs with each other only. To compare"
    echo "with CI as well, rerun with --ci-reports DIR (see --help)."
fi

step "platform check passed on $os at $(git rev-parse --short HEAD)"
