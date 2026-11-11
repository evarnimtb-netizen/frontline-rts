#!/usr/bin/env bash
# tools/show.sh FILE:FROM-TO ...  Print each range under a header line.
# Refuses more than 400 lines in total per call.
[ $# -ge 1 ] || { echo "usage: tools/show.sh FILE:FROM-TO ..." >&2; exit 2; }
total=0
for a in "$@"; do
  r=${a##*:}; f=${a%:*}
  case "$r" in *[!0-9-]*|"") echo "bad range in '$a' (want FILE:FROM-TO)" >&2; exit 2;; esac
  from=${r%-*}; to=${r#*-}
  [ -f "$f" ] && [ -n "$from" ] && [ -n "$to" ] || { echo "bad spec '$a' (want FILE:FROM-TO)" >&2; exit 2; }
  [ "$to" -ge "$from" ] || { echo "empty range in '$a'" >&2; exit 2; }
  total=$((total + to - from + 1))
done
if [ "$total" -gt 400 ]; then
  echo "refused: $total lines asked, the limit is 400 per call. Narrow the ranges (use tools/outline.sh first)." >&2
  exit 1
fi
for a in "$@"; do
  r=${a##*:}; f=${a%:*}; from=${r%-*}; to=${r#*-}
  echo "=== $f:$from-$to"
  awk -v a="$from" -v b="$to" 'NR>=a && NR<=b {printf "%d\t%s\n", NR, $0} NR>b {exit}' "$f"
done
