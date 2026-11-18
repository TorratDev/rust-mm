#!/usr/bin/env bash
# Mechanical checks on a task's diff: the executor before each commit and push, the reviewer
# for its checks 1 (scope), 2 (legal) and 6 (engineering rules). Floats and HashMap are
# checked by verify.sh (clippy.toml and check-determinism.sh), not here.
#
# Usage: scripts/check-diff.sh WT [--staged] [--files "PATH ..."] [--base REF]
#   --files   allowed paths, space-separated (default: $TASK_FILES). An entry ending in / allows
#             a directory; * and {a,b} work as in the shell. Without a list, scope is skipped.
#   --staged  check the index (before a commit) instead of BASE...HEAD (default origin/main).
#
# One line per check: "<check> OK|WARN|FAIL <detail>". WARN is for the reviewer to judge.
# Exit 1 if any check FAILs.
set -euo pipefail -o noglob   # noglob: the --files entries are patterns, not paths to expand
shopt -s extglob
. "$(dirname "$0")/harness-lib.sh"

wt=${1:?usage: check-diff.sh WT [--staged] [--files LIST] [--base REF]}; shift
files=${TASK_FILES:-} staged=0 base=origin/main
while [ $# -gt 0 ]; do
  case "$1" in
    --staged) staged=1; shift ;;
    --files) files=$2; shift 2 ;;
    --base) base=$2; shift 2 ;;
    *) die "unknown option: $1" ;;
  esac
done

if [ "$staged" = 1 ]; then range=(--cached); else range=("$base...HEAD"); fi
names=$(git -C "$wt" diff --name-only "${range[@]}")
if [ -z "$names" ]; then echo "diff EMPTY nothing to check"; exit 0; fi
fail=0
report() { echo "$1 $2 $3"; [ "$2" != FAIL ] || fail=1; }

# Scope: every changed path matches an allowed entry.
if [ -n "$files" ]; then
  out=()
  while IFS= read -r f; do
    ok=0
    for a in $files; do
      p=$a; [[ $p == */ ]] && p="$p*"
      p=$(printf '%s' "$p" | sed 's/{/@(/g; s/}/)/g; s/,/|/g')
      # shellcheck disable=SC2053 # $p is a glob on purpose
      [[ $f == $p ]] && { ok=1; break; }
    done
    [ "$ok" = 1 ] || out+=("$f")
  done <<< "$names"
  if [ ${#out[@]} -eq 0 ]; then report scope OK "$(wc -l <<< "$names") files, all named by the task"
  else report scope FAIL "not named by the task: ${out[*]}"; fi
else
  report scope SKIP "no file list (--files or TASK_FILES)"
fi

# Legal: ROM data, saves, Ghidra projects, reference/ and dumps never enter the repo.
legal='\.(nds|gba|srl|sav|dsv|ds[0-9t]|gpr|gzf)$|\.rep/|(^|/)reference/|(^|/)roms/|rescue-re/ghidra'
bad=$(grep -iE "$legal" <<< "$names" || true)
if [ -n "$bad" ]; then report legal FAIL "forbidden files: $(tr '\n' ' ' <<< "$bad")"
else report legal OK "no ROM, save, Ghidra or reference files"; fi
bin=$(git -C "$wt" diff --numstat "${range[@]}" | awk -F'\t' '$1=="-" {print $3}' | tr '\n' ' ')
[ -z "$bin" ] || report binary WARN "binary files: $bin"

# Dependencies: a Cargo change must pass the dependency rule and needs a reason in the task.
cargo=$(grep -E '(^|/)Cargo\.(toml|lock)$' <<< "$names" | tr '\n' ' ' || true)
if [ -n "$cargo" ]; then
  if (cd "$wt" && scripts/check-deps.sh >/dev/null 2>&1); then
    report deps WARN "changed $cargo(check-deps passed; a new dependency needs a reason in the task)"
  else report deps FAIL "changed $cargo and scripts/check-deps.sh fails"; fi
fi

# Added or removed lines, as "file<TAB>+|-<TAB>text", for the two rules below.
lines=$(git -C "$wt" diff -U0 "${range[@]}" -- '*.rs' | awk '
  /^\+\+\+ / { f = substr($0, 7); next } /^--- / { next }
  /^[+-]/ { printf "%s\t%s\t%s\n", f, substr($0, 1, 1), substr($0, 2) }')

# Public API: the task must name any pub item that appears or disappears.
pub=$(awk -F'\t' '$3 ~ /^[[:space:]]*pub(\([a-z]+\))? +(fn|struct|enum|trait|const|static|type|mod|use) / {
  sub(/^[[:space:]]+/, "", $3); print "  " $2 " " $1 ": " $3 }' <<< "$lines" | head -20)
if [ -n "$pub" ]; then report pub WARN "pub items changed (the task must name them):"; echo "$pub"
else report pub OK "no pub item added or removed"; fi

# Parsers and protocol code must not panic on input bytes.
pan=$(awk -F'\t' '$2 == "+" && $1 ~ /^crates\/(nds|formats|trace|save)\// &&
  $3 ~ /\.unwrap\(\)|\.expect\(|panic!|unreachable!|todo!|unimplemented!/ {
  sub(/^[[:space:]]+/, "", $3); print "  " $1 ": " $3 }' <<< "$lines" | head -20)
if [ -n "$pan" ]; then report panic WARN "possible panics added in parser code (fine in tests):"; echo "$pan"
else report panic OK "no unwrap/expect/panic added in parser code"; fi

exit "$fail"
