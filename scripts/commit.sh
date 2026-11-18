#!/usr/bin/env bash
# Commit everything changed in a worktree, after the checks an executor must not skip.
#
# Usage: scripts/commit.sh WT -m SUBJECT [-m PARAGRAPH]... [--files LIST] [--engine E --model M]
#
# Stages every change in WT and runs scripts/check-diff.sh --staged (scope against --files or
# $TASK_FILES, and the legal check). On a FAIL it unstages everything and commits nothing.
# Adds " (#N)" to the subject when it names no issue (N from the branch "<n>-<slug>"), and the
# Co-Authored-By line for the engine (AGENT_ENGINE/AGENT_MODEL, or --engine/--model).
set -euo pipefail
. "$(dirname "$0")/harness-lib.sh"

wt=${1:?usage: commit.sh WT -m SUBJECT [-m PARAGRAPH]...}; shift
msg=() files=${TASK_FILES:-}
while [ $# -gt 0 ]; do
  case "$1" in
    -m) msg+=("$2"); shift 2 ;;
    --files) files=$2; shift 2 ;;
    --engine) export AGENT_ENGINE=$2; shift 2 ;;
    --model) export AGENT_MODEL=$2; shift 2 ;;
    *) die "unknown option: $1" ;;
  esac
done
[ ${#msg[@]} -gt 0 ] || die "needs -m SUBJECT"
trailer=$(coauthor_line) || exit 2

git -C "$wt" add -A
if ! "$HARNESS_DIR/check-diff.sh" "$wt" --staged --files "$files"; then
  git -C "$wt" reset -q
  die "commit.sh: nothing committed; fix the FAIL lines above"
fi

n=$(issue_of_branch "$(git -C "$wt" branch --show-current)" || true)
subject=${msg[0]}
[ -z "$n" ] || [[ $subject == *"#$n"* ]] || subject="$subject (#$n)"
{
  printf '%s\n' "$subject"
  for p in "${msg[@]:1}"; do printf '\n%s\n' "$p"; done
  printf '\n%s\n' "$trailer"
} | git -C "$wt" commit -q -F -
git -C "$wt" log --oneline -1
