#!/usr/bin/env bash
# The mechanical half of a review (reviewer checks 1, 2, 6 and 8), so the reviewer only judges
# the Objective, the evidence and the spot check.
#
# Usage: scripts/review-prep.sh PR [--files LIST]
#
# Finds the PR's worktree from its branch, then prints: the diff size, the latest REVISE comment
# (on a re-review, confirm each item), check-diff and verify. The same results go, as a
# "Mechanical checks" section, to a file that scripts/report.sh appends to the verdict comment.
# Exit 1 if a check or the verification FAILs (that is a REVISE, or ESCALATE for legal).
set -uo pipefail
. "$(dirname "$0")/harness-lib.sh"

pr=${1:?usage: review-prep.sh PR [--files LIST]}; shift
files=${TASK_FILES:-}
[ "${1:-}" = --files ] && files=$2
branch=$(gh pr view "$pr" --json headRefName --jq .headRefName) || exit 2
wt=$(wt_path "$branch")
[ -d "$wt" ] || die "no worktree for $branch at $wt"
git -C "$wt" fetch -q origin main

out=$(cache_dir review)/review-$pr.md
stat=$(git -C "$wt" diff --shortstat origin/main...HEAD)
echo "PR #$pr, branch $branch, worktree $wt"
echo "diff:$stat"
echo "-- latest REVISE comment:"
gh pr view "$pr" --json comments --jq '[.comments[].body | select(test("^[*[:space:]]*REVISE"))] | last // "none"'
echo "-- check-diff:"
checks=$("$HARNESS_DIR/check-diff.sh" "$wt" --files "$files"); rc1=$?
echo "$checks"
echo "-- verify:"
ver=$("$HARNESS_DIR/verify.sh" "$wt"); rc2=$?
echo "$ver"

{
  echo "### Mechanical checks (scripts/review-prep.sh)"
  echo
  echo "Diff against origin/main:$stat"
  echo
  echo '```'
  echo "$checks"
  grep -E '^[a-z]+ (PASS|FAIL) |^verify: ' <<< "$ver"
  echo '```'
} > "$out"
echo "mechanical checks saved to $out"
[ $rc1 = 0 ] && [ $rc2 = 0 ]
