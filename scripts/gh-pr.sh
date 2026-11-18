#!/usr/bin/env bash
# Open a pull request the way this repo wants it, so no session has to remember the rules:
# - the body's first line is "Closes #N" (the PR solves the issue) or "Refs #N" (part of it);
# - a Refs body may not say close/fix/resolve next to an issue number, or GitHub closes it;
# - assigned to @me, base main.
#
# Usage:
#   scripts/gh-pr.sh --title T --body-file F (--closes N | --refs N | --no-issue) [--draft]
#                    [--base B] [--head BRANCH] [--assignee U] [--dry-run]
#
# The body file holds the rest of the body; do not put the Closes/Refs line in it.
# Prints the PR URL last. Run from the branch's worktree, after pushing.
set -euo pipefail

title="" body="" link="" issue="" base="main" head="" assignee="@me" dry=0 draft=()
while [ $# -gt 0 ]; do
  case "$1" in
    --title) title="$2"; shift 2 ;;
    --body-file) body="$2"; shift 2 ;;
    --closes) link=Closes; issue="$2"; shift 2 ;;
    --refs) link=Refs; issue="$2"; shift 2 ;;
    --no-issue) link=none; shift ;;
    --draft) draft=(--draft); shift ;;
    --base) base="$2"; shift 2 ;;
    --head) head="$2"; shift 2 ;;
    --assignee) assignee="$2"; shift 2 ;;
    --dry-run) dry=1; shift ;;
    -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done
[ -n "$title" ] && [ -n "$body" ] || { echo "needs --title and --body-file" >&2; exit 2; }
[ -f "$body" ] || [ "$body" = - ] || { echo "no such body file: $body" >&2; exit 2; }
[ -n "$link" ] || { echo "say which issue it solves: --closes N, --refs N, or --no-issue" >&2; exit 2; }
[ "$link" = none ] || [[ "$issue" =~ ^[0-9]+$ ]] || { echo "not an issue number: $issue" >&2; exit 2; }

text=$(cat -- "$body")
# GitHub's closing keywords next to an issue reference, in any case.
closing='(close[sd]?|fix(e[sd])?|resolve[sd]?)[[:space:]:]+([[:alnum:]_.-]+/[[:alnum:]_.-]+)?#[0-9]+'
if printf '%s\n%s\n' "$title" "$text" | grep -qiE "^[[:space:]]*(Closes|Refs) #[0-9]+[[:space:]]*$"; then
  echo "the body file already has a Closes/Refs line; drop it, the script adds it" >&2; exit 2
fi
if [ "$link" != Closes ] && printf '%s\n%s\n' "$title" "$text" | grep -qiE "$closing"; then
  echo "a ${link/none/no-issue} PR must not write close/fix/resolve next to an issue number (GitHub would close it):" >&2
  printf '%s\n%s\n' "$title" "$text" | grep -inE "$closing" >&2
  exit 2
fi

tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT
{ [ "$link" = none ] || printf '%s #%s\n\n' "$link" "$issue"; printf '%s\n' "$text"; } > "$tmp"

args=(pr create --title "$title" --body-file "$tmp" --base "$base" "${draft[@]}")
[ -n "$head" ] && args+=(--head "$head")
[ -n "$assignee" ] && args+=(--assignee "$assignee")
if [ "$dry" = 1 ]; then
  printf '+ gh'; printf ' %q' "${args[@]}"; echo; echo "--- body"; cat "$tmp"
else
  gh "${args[@]}"
fi
