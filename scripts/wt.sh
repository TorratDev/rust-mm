#!/usr/bin/env bash
# Set up the worktree for a sub-issue (executor step 2): create it from the latest main, or
# reuse it and show what an earlier session left there.
#
# Usage: scripts/wt.sh N SLUG
#
# Worktree <main checkout>/../rust-mm-worktrees/N-SLUG on branch N-SLUG. On reuse it prints the
# commits on the branch, git status and, when the PR has one, the latest REVISE comment.
# The worktree path is the last line.
set -euo pipefail
. "$(dirname "$0")/harness-lib.sh"

n=${1:?usage: wt.sh N SLUG}; slug=${2:?usage: wt.sh N SLUG}
[[ $n =~ ^[0-9]+$ ]] || die "not an issue number: $n"
branch="$n-$slug"
repo=$(main_repo)
wt=$(wt_path "$branch")
git -C "$repo" fetch -q origin main

if [ -d "$wt" ]; then
  echo "reuse $wt: an earlier session worked here"
  echo "-- commits on $branch:"; git -C "$wt" log --oneline origin/main..HEAD
  echo "-- status:"; git -C "$wt" status --short
  pr=$(cd "$wt" && gh pr list --head "$branch" --state open --json number --jq '.[0].number // empty' 2>/dev/null || true)
  if [ -n "$pr" ]; then
    echo "-- PR #$pr, latest REVISE comment:"
    gh pr view "$pr" --json comments \
      --jq '[.comments[].body | select(test("^[*[:space:]]*REVISE"))] | last // "none"'
  fi
elif git -C "$repo" show-ref -q --verify "refs/heads/$branch"; then
  git -C "$repo" worktree add -q "$wt" "$branch"
  echo "created $wt on the existing branch $branch"
else
  git -C "$repo" worktree add -q -b "$branch" "$wt" origin/main
  echo "created $wt on $branch from origin/main"
fi
echo "$wt"
