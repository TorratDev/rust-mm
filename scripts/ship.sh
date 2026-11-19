#!/usr/bin/env bash
# Executor steps 5 and 6 in one command: rebase onto the latest main, check, verify, push, and
# open the draft PR if the branch has none.
#
# Usage: scripts/ship.sh WT (--closes N | --refs N) [--title T --body-file F] [--files LIST]
#                        [--engine E --model M]
#
# --title and --body-file are needed only when the branch has no open PR yet. The body file is
# your text only: scripts/gh-pr.sh adds the Closes/Refs line and this script the attribution.
# Exit 3 with a "BLOCKED: rebase conflict in ..." line when the rebase conflicts (rebase aborted).
# Exit 1 when a check or the verification fails; nothing is pushed then.
# The last line is "ship: <PR URL> <verify summary>".
set -euo pipefail
. "$(dirname "$0")/harness-lib.sh"

wt=${1:?usage: ship.sh WT (--closes N|--refs N) [--title T --body-file F]}; shift
link="" title="" body="" files=${TASK_FILES:-}
while [ $# -gt 0 ]; do
  case "$1" in
    --closes) link=(--closes "$2"); shift 2 ;;
    --refs) link=(--refs "$2"); shift 2 ;;
    --title) title=$2; shift 2 ;;
    --body-file) body=$2; shift 2 ;;
    --files) files=$2; shift 2 ;;
    --engine) export AGENT_ENGINE=$2; shift 2 ;;
    --model) export AGENT_MODEL=$2; shift 2 ;;
    *) die "unknown option: $1" ;;
  esac
done
[ -n "${link[*]}" ] || die "needs --closes N or --refs N (the dispatch's PR link line)"
branch=$(git -C "$wt" branch --show-current)
[ -z "$(git -C "$wt" status --porcelain)" ] || die "uncommitted changes in $wt: run scripts/commit.sh first"

git -C "$wt" fetch -q origin main
if ! git -C "$wt" rebase -q origin/main >/dev/null 2>&1; then
  conflicts=$(git -C "$wt" diff --name-only --diff-filter=U | tr '\n' ' ')
  git -C "$wt" rebase --abort
  echo "BLOCKED: rebase conflict in $conflicts"
  exit 3
fi

"$HARNESS_DIR/check-diff.sh" "$wt" --files "$files" || die "ship.sh: check-diff failed; nothing pushed"
summary=$("$HARNESS_DIR/verify.sh" "$wt" | tee /dev/stderr | tail -1)
[[ $summary == "verify: PASS"* ]] || die "ship.sh: $summary; nothing pushed"

git -C "$wt" push -q --force-with-lease -u origin "$branch"
url=$(cd "$wt" && gh pr list --head "$branch" --state open --json url --jq '.[0].url // empty')
if [ -z "$url" ]; then
  [ -n "$title" ] && [ -f "$body" ] || die "no open PR for $branch: pass --title and --body-file"
  foot=$(footer_line) || exit 2
  tmp=$(mktemp); trap 'rm -f "$tmp"' EXIT
  { cat -- "$body"; printf '\n%s\n' "$foot"; } > "$tmp"
  url=$(cd "$wt" && "$HARNESS_DIR/gh-pr.sh" --draft --title "$title" --body-file "$tmp" "${link[@]}" | tail -1)
fi
echo "ship: $url $summary"
