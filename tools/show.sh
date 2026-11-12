#!/usr/bin/env bash
# tools/show.sh FILE:FROM-TO ...  Print each range under a header line.
# FILE::name  Print the item(s) called name (fn, struct, enum, trait, mod, impl,
#   #[test] fn; for an impl, any impl whose header has the word), found with
#   tools/outline.sh. Several matches are all printed.
# Refuses more than 400 lines in total per call.
[ $# -ge 1 ] || { echo "usage: tools/show.sh FILE:FROM-TO ..." >&2; exit 2; }
here=$(cd "$(dirname "$0")" && pwd)
args=()
for a in "$@"; do
  case "$a" in
  *::*)
    f=${a%%::*}; nm=${a#*::}
    [ -f "$f" ] && [ -n "$nm" ] || { echo "bad spec '$a' (want FILE::name)" >&2; exit 2; }
    hits=$("$here/outline.sh" "$f" | awk -v n="$nm" '
      NR==1 {next}
      { range=$1; $1=""; sub(/^ /,""); t=$0; sub(/^#\[test\] /,"",t)
        if (t ~ /^impl/) { ok = (" " t " " ~ "[^A-Za-z0-9_]" n "[^A-Za-z0-9_]") }
        else { ok = 0
          if (match(t, /(fn|struct|enum|trait|mod|union)[ \t]+[A-Za-z0-9_]+/) ||
              match(t, /(const|static|type)[ \t]+[A-Za-z0-9_]+/)) {
            w=substr(t,RSTART,RLENGTH); sub(/^[a-z]+[ \t]+/,"",w); ok = (w==n) } }
        if (ok) print range }')
    [ -n "$hits" ] || { echo "no item named '$nm' in $f (see tools/outline.sh $f)" >&2; exit 2; }
    for r in $hits; do args+=("$f:$r"); done ;;
  *) args+=("$a") ;;
  esac
done
set -- "${args[@]}"
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
