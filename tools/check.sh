#!/usr/bin/env bash
# One-command checks that print a few lines instead of cargo's and Godot's
# full output. Full logs go to target/check/*.log.
#
#   tools/check.sh quick [BASE]   # what an agent runs while it works
#   tools/check.sh tests          # the orchestrator, between stages
#   tools/check.sh full           # what the orchestrator runs before pushing
#
# quick: cargo fmt (applied, not only checked), clippy -D warnings (workspace),
#   tools/test_neutral.py, tools/ablate.sh --dry-run, and the release
#   tests (tools/ptest.py: test binaries three at a time, ignored tests left out)
#   for the crates changed against BASE (default: $CHECK_BASE, else the
#   merge-base with origin/<this branch>, else with main). Prints a pass/fail
#   count per crate and, for a failing test, its first 40 lines.
# tests: every release test of the workspace with the ignored ones, three
#   binaries at a time (same tests as full, about a third of the time).
# full: fmt, clippy, the whole workspace's tests including the ignored ones
#   (release, through ptest.py with PTEST_NO_CACHE=1: always all of them), golden ballistics, validate mods/ww2, the bridge build, the four
#   Godot headless tests and a settings.cfg-unchanged check. One line each:
#   "name: ok" or "name: FAIL (see LOG)".
#
# ptest.py skips a binary that passed with the same inputs ("cached"; see its
# docstring); PTEST_NO_CACHE=1 turns that off.
# Env: CARGO_BUILD_JOBS (default 4), GODOT (default ~/Applications/Godot.app),
# SETTINGS_BAK (the saved copy of the Godot user settings.cfg to compare with).
# Exit status is non-zero when anything fails.
set -u
cd "$(dirname "$0")/.."
ROOT=$(pwd)
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"
LOGS=target/check
mkdir -p "$LOGS"
FAILS=0

SETTINGS_CFG="$HOME/Library/Application Support/Godot/app_userdata/Frontline/settings.cfg"
SETTINGS_BAK="${SETTINGS_BAK:-$HOME/.claude/projects/-Users-evarnimtb-Documents-Claude-code-frontline-rts/keep/settings.cfg.bak}"

# Seconds since the step began (the caller sets T0=$SECONDS).
ok() { echo "$1: ok${2:+ ($2)} [$((SECONDS - ${T0:-$SECONDS})) s]"; }
bad() { echo "$1: FAIL (see $LOGS/$2.log)"; FAILS=$((FAILS + 1)); }

# step NAME LOGNAME CMD...  -> one line; the first 12 log lines on failure
step() {
    local name=$1 log=$2 T0=$SECONDS
    shift 2
    if "$@" >"$LOGS/$log.log" 2>&1; then
        ok "$name"
    else
        bad "$name" "$log"
        head -12 "$LOGS/$log.log" | sed 's/^/    /'
    fi
}

# "N passed, M failed" summed over the "test result:" lines of a cargo log.
counts() {
    awk '/^test result:/ { for (i = 1; i <= NF; i++) { if ($i == "passed;") p += $(i-1); if ($i == "failed;") f += $(i-1) } }
         END { printf "%d passed, %d failed", p, f }' "$1"
}

# The first 40 lines of every failing test's output in a cargo test log.
failing_tests() {
    awk '/^---- .* stdout ----$/ { n = 0; show = 1; print "  " $0; next }
         /^failures:$/ { show = 0 }
         show && n < 40 { print "    " $0; n++ }' "$1" | head -200
}

cargo_tests() { # NAME LOG CARGO-ARGS...
    local name=$1 log=$2 T0=$SECONDS
    shift 2
    if cargo test -q "$@" >"$LOGS/$log.log" 2>&1; then
        ok "$name" "$(counts "$LOGS/$log.log")"
    else
        echo "$name: FAIL ($(counts "$LOGS/$log.log"); see $LOGS/$log.log)"
        FAILS=$((FAILS + 1))
        if grep -q '^---- ' "$LOGS/$log.log"; then
            failing_tests "$LOGS/$log.log"
        else
            grep -E '^error' -A6 "$LOGS/$log.log" | head -12 | sed 's/^/    /'
        fi
    fi
}

# Release tests through tools/ptest.py (binaries side by side).
ptests() { # NAME LOG PTEST-ARGS... -- CARGO-ARGS...
    local name=$1 log=$2 T0=$SECONDS out
    shift 2
    if out=$(python3 -I tools/ptest.py "$LOGS/$log.log" "$@" 2>&1); then
        ok "$name" "$(echo "$out" | head -1)"
    else
        echo "$name: FAIL ($(echo "$out" | head -1); see $LOGS/$log.log)"
        echo "$out" | tail -n +2 | head -200
        FAILS=$((FAILS + 1))
    fi
}

base_commit() {
    if [ -n "${1:-}" ]; then echo "$1"; return; fi
    if [ -n "${CHECK_BASE:-}" ]; then echo "$CHECK_BASE"; return; fi
    local br
    br=$(git rev-parse --abbrev-ref HEAD)
    git merge-base HEAD "origin/$br" 2>/dev/null || git merge-base HEAD main 2>/dev/null || git rev-parse HEAD~1
}

crates_changed() { # BASE -> package names, one per line
    {
        git diff --name-only "$1" --
        git ls-files --others --exclude-standard
    } | sort -u | awk '
        /^crates\/sim\//          { print "frontline-sim" }
        /^crates\/simbench\//     { print "frontline-simbench" }
        /^crates\/godot-bridge\// { print "godot-bridge" }
        /^crates\/net\//          { print "frontline-net" }
        /^sdk\/crates\/content\// { print "frontline-content" }
        /^sdk\/crates\/cli\//     { print "frontline-cli" }
        /^mods\//                 { print "frontline-content"; print "frontline-cli" }
        /^(Cargo\.toml|Cargo\.lock|rust-toolchain\.toml)$/ { print "ALL" }' | sort -u
}

quick() {
    local base pk args=""
    base=$(base_commit "${1:-}")
    # quick formats instead of failing: a stage's last check can then never
    # end on a fmt failure with the tests not run again on the formatted code.
    if cargo fmt --all -- --check >"$LOGS/fmt.log" 2>&1; then
        echo "fmt: ok"
    elif cargo fmt --all >>"$LOGS/fmt.log" 2>&1; then
        echo "fmt: ok (cargo fmt applied; the changed files are in the working tree)"
    else
        bad fmt fmt
    fi
    step clippy clippy cargo clippy -q --workspace --all-targets -- -D warnings
    # S10a: the neutrality checker's unit tests (small fixtures, < 1 s).
    step neutral.py test_neutral python3 -I tools/test_neutral.py
    # S10a-ai: the ablation override files and tools/ablate.sh (dry run:
    # plays 1-minute matches only if the release CLI is built).
    step ablate.sh ablate_dry tools/ablate.sh --dry-run target/ablate-on target/ablate-dry all
    # S10c-ai: its override files (non-switch values allowed there).
    step "ablate.sh S10c" ablate_dry_s10c env ABLATIONS=docs/slices/targets/S10c-ai-ablations \
        tools/ablate.sh --dry-run target/ablate-on target/ablate-dry
    # S11a1: its override files (the S11a1 switches only).
    step "ablate.sh S11a1" ablate_dry_s11a1 env ABLATIONS=docs/slices/targets/S11a1-ablations \
        tools/ablate.sh --dry-run target/ablate-on target/ablate-dry
    pk=$(crates_changed "$base")
    # Release, not debug: the same build the batches use, and the whole
    # workspace's tests take 6 minutes in release against 18 in debug.
    if [ -z "$pk" ]; then
        echo "tests: skipped (no crate changed since ${base:0:9})"
    elif echo "$pk" | grep -qx ALL; then
        ptests "tests workspace" test_quick -- --workspace
    else
        for p in $pk; do args="$args -p $p"; done
        ptests "tests ($(echo $pk | sed 's/frontline-//g; s/ /, /g'))" test_quick -- $args
    fi
}

godot_bin() {
    if [ -n "${GODOT:-}" ]; then echo "$GODOT"; return; fi
    if [ -x "$HOME/Applications/Godot.app/Contents/MacOS/Godot" ]; then
        echo "$HOME/Applications/Godot.app/Contents/MacOS/Godot"
    else
        command -v godot || command -v Godot || true
    fi
}

godot_test() { # NAME SCRIPT
    local name=$1 script=$2 bin T0=$SECONDS
    bin=$(godot_bin)
    if [ -z "$bin" ]; then
        echo "no Godot binary found; set GODOT" >"$LOGS/$name.log"
        bad "$name" "$name"
        return
    fi
    if timeout_run 600 "$bin" --headless --path godot -s "res://tools/$script.gd" >"$LOGS/$name.log" 2>&1 \
        && ! grep -qE '^(SCRIPT ERROR|Parse Error)' "$LOGS/$name.log"; then
        ok "$name"
    else
        bad "$name" "$name"
        grep -E 'FAIL|ERROR' "$LOGS/$name.log" | head -8 | sed 's/^/    /'
    fi
}

# macOS has no timeout(1); run in the background and kill after N seconds.
timeout_run() {
    local secs=$1
    shift
    "$@" &
    local pid=$!
    ( sleep "$secs"; kill "$pid" 2>/dev/null ) &
    local killer=$!
    wait "$pid"
    local rc=$?
    kill "$killer" 2>/dev/null
    wait "$killer" 2>/dev/null
    return $rc
}

full() {
    step fmt fmt cargo fmt --all -- --check
    step clippy clippy cargo clippy -q --workspace --all-targets -- -D warnings
    # The same parallel path as `tests`, never from the cache.
    PTEST_NO_CACHE=1 ptests tests test_full --include-ignored -- --workspace
    step build-cli build_cli cargo build -q --release -p frontline-cli
    step golden golden target/release/frontline-cli ballistics --golden mods/ww2/tests/ballistics.toml
    step validate validate target/release/frontline-cli validate mods/ww2
    step bridge bridge tools/build_bridge.sh
    godot_test check_project check_project
    godot_test test_match_view test_match_view
    godot_test test_net_ui test_net_ui
    godot_test test_data_reload test_data_reload
    if [ -f "$SETTINGS_BAK" ] && cmp -s "$SETTINGS_BAK" "$SETTINGS_CFG"; then
        ok settings.cfg
    else
        echo "settings.cfg differs from $SETTINGS_BAK (or one is missing)" >"$LOGS/settings.log"
        bad settings.cfg settings
    fi
}

case "${1:-}" in
    quick) quick "${2:-}" ;;
    tests) ptests "tests workspace, ignored included" test_all --include-ignored -- --workspace ;;
    full) full ;;
    *)
        echo "usage: tools/check.sh quick [BASE] | tests | full" >&2
        exit 2
        ;;
esac

if [ "$FAILS" -eq 0 ]; then
    echo "${1}: all ok"
else
    echo "${1}: $FAILS failed"
    exit 1
fi
