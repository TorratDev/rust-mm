#!/usr/bin/env bash
# A role session's last step: check the final line, post the comment that goes with it, stop
# the Ghidra bridge, and send the line to the orchestrator.
#
# Usage: scripts/report.sh "LINE" [--pr-comment PR FILE] [--issue-comment N FILE]
#                          [--role R] [--to ADDR] [--ghidra COPY]
#
# Defaults: --role $AGENT_ROLE, --to $ORCH, --ghidra $AGENT_GHIDRA (all set by orch/spawn.sh).
# Formats, by role:
#   executor      DONE: ... <PR URL> | BLOCKED: ...
#   investigator  DONE: ... | BLOCKED: ...      DONE needs --issue-comment (the findings file)
#   reviewer      PASS|REVISE|SPLIT|ESCALATE: ...   needs --pr-comment (the verdict file); its
#                 first line must start with the same word. The section saved by review-prep.sh
#                 is appended to the comment.
# Exit 2 when the line or the files do not fit; nothing is posted or sent then.
set -euo pipefail
. "$(dirname "$0")/harness-lib.sh"

line=${1:?usage: report.sh LINE [options]}; shift
role=${AGENT_ROLE:-} to=${ORCH:-} ghidra=${AGENT_GHIDRA:-} prc=() isc=()
while [ $# -gt 0 ]; do
  case "$1" in
    --pr-comment) prc=("$2" "$3"); shift 3 ;;
    --issue-comment) isc=("$2" "$3"); shift 3 ;;
    --role) role=$2; shift 2 ;;
    --to) to=$2; shift 2 ;;
    --ghidra) ghidra=$2; shift 2 ;;
    *) die "unknown option: $1" ;;
  esac
done
bad() { echo "report.sh: $*" >&2; exit 2; }
[[ $line != *$'\n'* ]] || bad "the line must be one line"
word=${line%%:*}

case "$role" in
  executor|investigator) [[ $line =~ ^(DONE|BLOCKED):\ .+ ]] || bad "$role line must start with 'DONE: ' or 'BLOCKED: '" ;;
  reviewer) [[ $line =~ ^(PASS|REVISE|SPLIT|ESCALATE):\ .+ ]] || bad "reviewer line must start with PASS:, REVISE:, SPLIT: or ESCALATE:" ;;
  *) bad "unknown role '$role' (set AGENT_ROLE or pass --role)" ;;
esac

if [ "$role" = executor ] && [ "$word" = DONE ]; then
  url=$(grep -oE 'https://github\.com/[^ ]+/pull/[0-9]+' <<< "$line" | head -1) || true
  [ -n "$url" ] || bad "an executor DONE line must carry the PR URL"
  [ "$(gh pr view "$url" --json state --jq .state)" = OPEN ] || bad "$url is not an open PR"
fi
if [ "$role" = investigator ] && [ "$word" = DONE ]; then
  [ ${#isc[@]} = 2 ] || bad "investigator DONE needs --issue-comment N FINDINGS_FILE"
fi
if [ "$role" = reviewer ]; then
  [ ${#prc[@]} = 2 ] || bad "a verdict needs --pr-comment PR VERDICT_FILE"
  first=$(grep -m1 . "${prc[1]}" || true)
  [[ $first =~ ^[*[:space:]]*$word ]] || bad "the verdict file's first line must start with $word (got: $first)"
fi

# post BODY_FILE KIND NUMBER: comment, then read it back.
post() {
  gh "$2" comment "$3" --body-file "$1" >/dev/null
  local got want
  got=$(gh "$2" view "$3" --json comments --jq '.comments | last | .body' | grep -m1 . || true)
  want=$(grep -m1 . "$1")
  [ "$got" = "$want" ] || die "report.sh: the comment on $2 #$3 did not land; nothing sent"
}
if [ ${#prc[@]} = 2 ]; then
  tmp=$(mktemp); trap 'rm -f "$tmp"' EXIT
  cat -- "${prc[1]}" > "$tmp"
  mech="$(cache_dir review)/review-${prc[0]}.md"
  if [ "$role" = reviewer ] && [ -f "$mech" ]; then printf '\n' >> "$tmp"; cat "$mech" >> "$tmp"; fi
  post "$tmp" pr "${prc[0]}"
fi
[ ${#isc[@]} = 2 ] && post "${isc[1]}" issue "${isc[0]}"

if [ -n "$ghidra" ] && [ "$ghidra" != none ]; then
  ghidra --project "$ghidra" --program arm9.bin stop >/dev/null 2>&1 || true
fi
if [ -n "$to" ]; then hc send "$to" "$line" >/dev/null
else echo "report.sh: no ORCH address; the orchestrator reads the line below" >&2; fi
echo "$line"
