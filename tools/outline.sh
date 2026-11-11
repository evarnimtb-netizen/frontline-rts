#!/usr/bin/env bash
# tools/outline.sh FILE...  One line per item: "from-to name" (no bodies).
# Rust: fn struct enum impl trait mod const static type and #[test].
# GDScript: func class signal. Ranges come from brace depth (Rust) or
# indentation (GDScript); text-only, no build.
[ $# -ge 1 ] || { echo "usage: tools/outline.sh FILE..." >&2; exit 2; }
for f in "$@"; do
  [ -f "$f" ] || { echo "no such file: $f" >&2; continue; }
  echo "== $f"
  case "$f" in
  *.gd)
    awk '
      function ind(s,  m){ match(s,/^[\t ]*/); return RLENGTH }
      function flush(lvl,  i){ for(i=n;i>=1;i--) if(ind_[i]>=lvl){ print from_[i] "-" last " " name_[i]; n=i-1 } }
      /^[ \t]*$/ {next}
      /^[ \t]*#/ {next}
      { i=ind($0); flush(i); last=NR
        if ($0 ~ /^[ \t]*(static[ \t]+)?func[ \t]/ || $0 ~ /^[ \t]*class[ \t]/ || $0 ~ /^[ \t]*signal[ \t]/) {
          s=$0; sub(/^[ \t]*/,"",s); sub(/:[ \t]*$/,"",s)
          n++; ind_[n]=i; from_[n]=NR; name_[n]=s
          if ($0 ~ /signal[ \t]/) { print NR "-" NR " " s; n-- }
        } }
      END{ last=NR; flush(0) }' "$f" | sort -n ;;
  *)
    awk '
      { line=$0
        # strip strings and line comments crudely before counting braces
        t=line; gsub(/"([^"\\]|\\.)*"/,"",t); sub(/\/\/.*/,"",t)
        o=gsub(/\{/,"{",t); c=gsub(/\}/,"}",t)
        if (pend_test==0 && line ~ /^[ \t]*#\[test\]/) { pend_test=1; next }
        if (line ~ /^[ \t]*(pub(\([a-z]+\))?[ \t]+)?(const|static|type)[ \t]/ && line !~ /^[ \t]*(pub(\([a-z]+\))?[ \t]+)?(const|unsafe)[ \t]+fn/) {
          s=line; sub(/^[ \t]*/,"",s); sub(/[ \t]*[=;].*/,"",s)
          if (depth==0 || depth==1) print NR "-" NR " " s
        }
        if (line ~ /^[ \t]*((pub(\([a-z]+\))?|async|unsafe|const|extern)[ \t]+)*(fn|struct|enum|impl|trait|mod|union)[ \t<]/ ) {
          s=line; sub(/^[ \t]*/,"",s); sub(/[ \t]*(\{|;|where).*$/,"",s)
          if (pend_test) { s="#[test] " s; pend_test=0 }
          n++; d_[n]=depth; from_[n]=NR; name_[n]=s; open_[n]=0; done_[n]=0
          if (o>0) open_[n]=1
          if (o>0 && o==c) { print NR "-" NR " " s; n--; }
          else if (o==0 && line ~ /;[ \t]*$/) { print NR "-" NR " " s; n-- }
        } else if (line !~ /^[ \t]*#\[/) pend_test=0
        depth+=o-c
        # close items whose body ended
        while (n>0 && open_[n]==0 && depth>d_[n] ) open_[n]=1
        while (n>0 && open_[n]==1 && depth<=d_[n] && (NR>from_[n])) { print from_[n] "-" NR " " name_[n]; n-- }
      }' "$f" | sort -n ;;
  esac
done
