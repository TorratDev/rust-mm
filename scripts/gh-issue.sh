#!/usr/bin/env bash
# Create a GitHub issue (or update one) with its real relations, not just mentions in the body:
# labels, assignee, the parent (sub-issue link) and "blocked by" dependencies, in one call.
# A session writes the body and passes numbers; this script does the API work.
#
# Usage:
#   scripts/gh-issue.sh --title "sim: x" --body-file body.md [--label L]... [--assignee U|none]
#                       [--parent N] [--blocked-by N]... [--dry-run]
#   scripts/gh-issue.sh --issue N [same options]     # update or backfill an existing issue
#
# --assignee defaults to @me. Run inside the repository with gh logged in.
# Prints the issue number last. Exit 1 if a link failed; the issue still exists, so rerun
# with --issue N to retry the links (an existing link fails again with a 422, which is harmless).
set -euo pipefail

issue="" title="" body="" assignee="@me" parent="" dry=0
labels=() blockers=()
while [ $# -gt 0 ]; do
  case "$1" in
    --issue) issue="$2"; shift 2 ;;
    --title) title="$2"; shift 2 ;;
    --body-file) body="$2"; shift 2 ;;
    --label) labels+=("$2"); shift 2 ;;
    --assignee) assignee="$2"; shift 2 ;;
    --parent) parent="$2"; shift 2 ;;
    --blocked-by) blockers+=("$2"); shift 2 ;;
    --dry-run) dry=1; shift ;;
    -h|--help) sed -n '2,13p' "$0"; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done
for n in $issue $parent "${blockers[@]}"; do
  [[ "$n" =~ ^[0-9]+$ ]] || { echo "not an issue number: $n" >&2; exit 2; }
done
[ "$assignee" = none ] && assignee=""

gh_() {
  if [ "$dry" = 1 ]; then
    printf '+ gh'; printf ' %q' "$@"; echo
    case "$1 ${2:-}" in "issue create") echo "https://github.com/o/r/issues/0" ;; "api repos"*) echo 0 ;; esac
  else
    gh "$@"
  fi
}
# The relation endpoints take the issue's database id, not its number.
db_id() { gh_ api "repos/{owner}/{repo}/issues/$1" --jq .id | tail -1; }

if [ -z "$issue" ]; then
  [ -n "$title" ] && [ -n "$body" ] || { echo "creating an issue needs --title and --body-file" >&2; exit 2; }
  args=(issue create --title "$title" --body-file "$body")
  for l in "${labels[@]}"; do args+=(--label "$l"); done
  [ -n "$assignee" ] && args+=(--assignee "$assignee")
  url=$(gh_ "${args[@]}" | tail -1)
  issue="${url##*/}"
  echo "created #$issue ($url)"
else
  args=(issue edit "$issue")
  [ -n "$title" ] && args+=(--title "$title")
  [ -n "$body" ] && args+=(--body-file "$body")
  for l in "${labels[@]}"; do args+=(--add-label "$l"); done
  [ -n "$assignee" ] && args+=(--add-assignee "$assignee")
  [ ${#args[@]} -gt 3 ] && gh_ "${args[@]}" >/dev/null
fi

failed=0
if [ -n "$parent" ]; then
  if gh_ api -X POST "repos/{owner}/{repo}/issues/$parent/sub_issues" -F "sub_issue_id=$(db_id "$issue")" >/dev/null; then
    echo "#$issue is a sub-issue of #$parent"
  else echo "failed: parent #$parent" >&2; failed=1; fi
fi
for b in "${blockers[@]}"; do
  if gh_ api -X POST "repos/{owner}/{repo}/issues/$issue/dependencies/blocked_by" -F "issue_id=$(db_id "$b")" >/dev/null; then
    echo "#$issue is blocked by #$b"
  else echo "failed: blocked by #$b" >&2; failed=1; fi
done

[ "$failed" = 0 ] || echo "some links failed; retry with: scripts/gh-issue.sh --issue $issue ..." >&2
echo "$issue"
exit "$failed"
