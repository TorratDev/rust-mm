#!/usr/bin/env bash
# After a PASS: mark the PR ready, check its checks and merge it with gh_pr_finish (squash,
# delete the branch, remove the worktree, reopen the issue after a Refs merge).
#
# Usage: scripts/orch/merge.sh PR
# The worktree comes from the branch name, the issue and closes/refs from the body's first line.
# Exit 75 while checks are pending (run it again later; do not block on it), 1 on a failing
# check or a missing PASS.
set -uo pipefail
. "$(dirname "$0")/../harness-lib.sh"
. "$HARNESS_DIR/agent.sh"

pr=${1:?usage: merge.sh PR}
read -r branch first < <(gh pr view "$pr" --json headRefName,body --jq '.headRefName + " " + (.body | split("\n")[0])')
[[ $first =~ ^(Closes|Refs)\ \#([0-9]+) ]] || die "PR #$pr: the body's first line is not 'Closes #N' or 'Refs #N'"
link=$(tr 'A-Z' 'a-z' <<< "${BASH_REMATCH[1]}") issue=${BASH_REMATCH[2]}

line=$(gh_pr_line "$pr")
[[ $line == *" verdict=PASS "* ]] || die "PR #$pr has no PASS verdict: $line"
if [[ $line == *" draft "* ]]; then gh pr ready "$pr" >/dev/null; line=$(gh_pr_line "$pr"); fi
case "$line" in
  *" checks=pending "*) echo "pending: $line"; exit 75 ;;
  *" checks=fail "*) echo "checks fail: $line (dispatch an executor with the Fix checks line)"; exit 1 ;;
esac
gh_pr_finish "$pr" "$(wt_path "$branch")" "$issue" "$link"
