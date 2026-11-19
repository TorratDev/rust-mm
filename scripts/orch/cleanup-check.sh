#!/usr/bin/env bash
# The end-of-epic checks (orchestrator: Rules, last item), one line each: OK or FAIL.
#
# Usage: scripts/orch/cleanup-check.sh
# Checks: no Ghidra bridge running on a project copy, no task worktree left under
# rust-mm-worktrees ("sm/" branches belong to another tool), no session left that this
# orchestrator ($HC_TERMINAL_ID) started, no stray files in the main checkout. Exit 1 on a FAIL.
set -uo pipefail
. "$(dirname "$0")/../harness-lib.sh"

fail=0
say() { echo "$1 $2 $3"; [ "$2" = OK ] || fail=1; }

bridges=""
for gpr in "$HOME"/rescue-re/ghidra/rescue-arm9*.gpr; do
  [ -e "$gpr" ] || continue
  p=$(basename "$gpr" .gpr)
  ghidra --project "$p" --program arm9.bin status 2>&1 | grep -q "No bridge running" || bridges+="$p "
done
[ -z "$bridges" ] && say bridges OK "none running" || say bridges FAIL "running on: $bridges(stop: ghidra --project <copy> --program arm9.bin stop)"

repo=$(main_repo "$HARNESS_DIR")
wts=$(git -C "$repo" worktree list --porcelain | awk '/^worktree /{p=$2} /^branch /{b=$2; sub("refs/heads/","",b); if (p ~ /rust-mm-worktrees/ && b !~ /^sm\//) print p}')
[ -z "$wts" ] && say worktrees OK "none left" || say worktrees FAIL "left: $(tr '\n' ' ' <<< "$wts")"

kids=$(hc ls 2>/dev/null | python3 -c 'import json,sys,os
me = os.environ.get("HC_TERMINAL_ID")
print(" ".join(t.get("name") or t["address"] for t in json.load(sys.stdin).get("terminals", []) if me and t.get("parent") == me))')
[ -z "$kids" ] && say sessions OK "none left" || say sessions FAIL "still live: $kids(scripts/orch/reap.sh)"

stray=$(git -C "$repo" status --porcelain --untracked-files=all | head -10)
[ -z "$stray" ] && say repo OK "main checkout clean" || say repo FAIL "files in the main checkout: $(tr '\n' ' ' <<< "$stray")"
exit "$fail"
