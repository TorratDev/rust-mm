# shellcheck shell=bash
# Shared helpers for the harness scripts (docs/plans/agent-harness-scripts.md). Source it.

HARNESS_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

die() { echo "$*" >&2; exit 1; }

# main_repo [DIR]: the main checkout, also when DIR is inside a worktree.
main_repo() {
  local c
  c=$(git -C "${1:-.}" rev-parse --path-format=absolute --git-common-dir) || return
  dirname "$c"
}

# wt_path BRANCH [DIR]: where the sub-issue worktree for BRANCH lives (a sibling of the main checkout).
wt_path() { printf '%s/rust-mm-worktrees/%s\n' "$(dirname "$(main_repo "${2:-.}")")" "$1"; }

# cache_dir NAME: a scratch directory outside the repo.
cache_dir() {
  local d="${XDG_CACHE_HOME:-$HOME/.cache}/rescue-harness/$1"
  mkdir -p "$d" && printf '%s\n' "$d"
}

# issue_of_branch BRANCH: the sub-issue number in a "<n>-<slug>" branch name.
issue_of_branch() { [[ $1 =~ ^([0-9]+)- ]] && printf '%s\n' "${BASH_REMATCH[1]}"; }

# Attribution for the engine a session runs on (AGENT_ENGINE, AGENT_MODEL, set by orch/spawn.sh).
coauthor_line() {
  case "${AGENT_ENGINE:-}" in
    claude) echo "Co-Authored-By: Claude ${AGENT_MODEL:?AGENT_MODEL unset} <noreply@anthropic.com>" ;;
    pi) echo "Co-Authored-By: pi ${AGENT_MODEL:?AGENT_MODEL unset} <noreply@pi.dev>" ;;
    *) echo "AGENT_ENGINE must be claude or pi (got '${AGENT_ENGINE:-}'); pass --engine and --model" >&2; return 1 ;;
  esac
}
footer_line() {
  case "${AGENT_ENGINE:-}" in
    claude) echo "🤖 Generated with [Claude Code](https://claude.com/claude-code)" ;;
    pi) echo "🤖 Generated with [pi](https://pi.dev)" ;;
    *) echo "AGENT_ENGINE must be claude or pi" >&2; return 1 ;;
  esac
}
