#!/usr/bin/env bash
# End role sessions after their report: save the screen, print the report line, kill and
# remove the terminal, and stop its Ghidra bridge.
#
# Usage: scripts/orch/reap.sh NAME...
# Per session: "NAME: <last DONE/BLOCKED/PASS/REVISE/SPLIT/ESCALATE line or 'no report line'>
# (screen saved to FILE)".
set -uo pipefail
. "$(dirname "$0")/../harness-lib.sh"

[ $# -gt 0 ] || die "usage: reap.sh NAME..."
dir=$(cache_dir reap)
for name in "$@"; do
  log="$dir/$name-$(date +%Y%m%d-%H%M%S).log"
  hc read "$name" --lines 100 > "$log" 2>&1
  line=$(grep -oE '(DONE|BLOCKED|PASS|REVISE|SPLIT|ESCALATE): .*' "$log" | tail -1)
  case "$name" in
    investigator-?) copy=rescue-arm9-${name#investigator-} ;;
    reviewer) copy=rescue-arm9-r ;;
    reviewer-*) copy=rescue-arm9-r${name#reviewer-} ;;
    *) copy="" ;;
  esac
  if [ -n "$copy" ] && ! ghidra --project "$copy" --program arm9.bin status 2>&1 | grep -q "No bridge running"; then
    ghidra --project "$copy" --program arm9.bin stop >/dev/null 2>&1 && echo "$name: stopped the bridge on $copy"
  fi
  hc kill "$name" >/dev/null 2>&1
  hc rm "$name" >/dev/null 2>&1
  echo "$name: ${line:-no report line} (screen saved to $log)"
done
