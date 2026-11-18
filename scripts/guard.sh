#!/usr/bin/env bash
# Block shell commands a role session must not run, and name the script to use instead.
# Called before every shell command by the Claude Code hook (.claude/settings.json) and the pi
# extension (scripts/pi/guard.ts). Does nothing when AGENT_ROLE is unset, so sessions not
# started by scripts/orch/spawn.sh are unaffected.
#
# Usage: scripts/guard.sh --claude-hook     reads the PreToolUse JSON on stdin
#        scripts/guard.sh COMMAND
# Exit 2 with the reason on stderr blocks the command; exit 0 allows it.
#
# shortcut: matches command text, so it stops honest mistakes, not a session that hides a
# command (sh -c, eval, a script it wrote). Upgrade to a sandbox if that ever happens.
set -uo pipefail

role=${AGENT_ROLE:-}
[ -n "$role" ] || exit 0
if [ "${1:-}" = --claude-hook ]; then
  cmd=$(python3 -c 'import json,sys; print(json.load(sys.stdin).get("tool_input", {}).get("command", ""))') || exit 0
else
  cmd=${1:-}
fi

block() { echo "blocked for the $role role: $1" >&2; exit 2; }
# A command word at the start or after ; & | ( or $( , so text inside quotes rarely matches.
at='(^|[;&|(]|\$\()[[:space:]]*'
git="${at}git([[:space:]]+-C[[:space:]]+[^[:space:]]+)?[[:space:]]+"
has() { [[ $cmd =~ $1 ]]; }

# Everyone.
has "${at}gh[[:space:]]+issue[[:space:]]+create" && block "use scripts/gh-issue.sh (it sets the parent, blocked-by links, labels and assignee)"
has "${at}gh[[:space:]]+pr[[:space:]]+create" && block "use scripts/ship.sh (executor) or scripts/gh-pr.sh"
has "${at}gh[[:space:]]+pr[[:space:]]+merge[^;&|]*--auto" && block "never gh pr merge --auto: main has no required checks"
has "${at}herdr([[:space:]]|$)" && block "the old herdr CLI is gone: use hc or herdr-app"
has "${git}push[^;&|]*[[:space:]:]main([[:space:]]|$)" && block "never push to main"

case "$role" in
  orchestrator)
    has "${at}gh[[:space:]]+pr[[:space:]]+merge" && block "use scripts/orch/merge.sh PR"
    has "${at}hc[[:space:]]+run" && block "start sessions with scripts/orch/spawn.sh"
    ;;
  executor|investigator|reviewer)
    has "${at}gh[[:space:]]+pr[[:space:]]+(merge|ready|close)" && block "only the orchestrator marks PRs ready, merges or closes them"
    has "${at}hc[[:space:]]+(run|kill|rm)" && block "only the orchestrator starts and ends sessions"
    ;;
esac

case "$role" in
  executor)
    has "${git}commit" && block "commit with scripts/commit.sh WT -m ... (it checks scope and forbidden files)"
    has "${git}push" && block "push with scripts/ship.sh WT ... (it rebases, checks and verifies first)"
    repo=$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir 2>/dev/null) && repo=${repo%/.git}
    if [ -n "$repo" ]; then
      has "${at}git[[:space:]]+-C[[:space:]]+\"?${repo}/?\"?[[:space:]]+(fetch|worktree)" ||
        ! has "${at}git[[:space:]]+-C[[:space:]]+\"?${repo}/?\"?[[:space:]]" ||
        block "never run git in the main checkout ($repo) except fetch and worktree: use git -C <WT>"
      if [ "$PWD" = "$repo" ] && ! has "${at}cd[[:space:]]" &&
         has "${at}git[[:space:]]+(checkout|switch|reset|rebase|merge|stash|add|rm|restore|clean|cherry-pick)"; then
        block "this shell is in the main checkout: run it as git -C <WT> ..."
      fi
    fi
    ;;
  investigator|reviewer)
    has "${git}(commit|push|add|rm|reset|checkout|switch|rebase|merge|stash|restore|clean|cherry-pick)" && block "read-only role: no git writes"
    has "${at}cargo[[:space:]]+(add|remove|update)" && block "read-only role: Cargo.toml and Cargo.lock stay as they are"
    re="${at}ghidra[^;&|]*--project[[:space:]]+([^[:space:]]+)"
    if [[ $cmd =~ $re ]]; then
      p=${BASH_REMATCH[2]}
      [ "$p" = "${AGENT_GHIDRA:-none}" ] || block "use only your own Ghidra copy (${AGENT_GHIDRA:-none}), not $p"
    fi
    ;;
esac
exit 0
