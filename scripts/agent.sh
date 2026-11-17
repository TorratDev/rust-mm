# shellcheck shell=bash
# Helpers for agent sessions, in the "code mode" style: chain them in one shell command and
# print only the result, instead of one tool call per step with every intermediate output read.
# Reference: docs/agent-api.md. Use: source scripts/agent.sh; <helper> ...
#
# Reads only, except the functions named *_finish and scripts/gh-issue.sh (writes to GitHub).

RESCUE_EXPORT="${RESCUE_EXPORT:-$HOME/rescue-re/export}"

_re_check() {
  [ -f "$RESCUE_EXPORT/STAMP" ] || { echo "no Ghidra export at $RESCUE_EXPORT (no STAMP); see docs/ghidra-setup.md section 7" >&2; return 1; }
}

# ---- Ghidra text export (read-only) -------------------------------------------------------

# re_fn PATTERN: functions whose address or name matches (case-insensitive regex).
# Prints: address<TAB>name<TAB>file
re_fn() {
  _re_check || return
  awk -F'\t' -v p="$(printf '%s' "$1" | tr 'A-Z' 'a-z')" \
    'NR>1 && (tolower($1) ~ p || tolower($2) ~ p) {print $1 "\t" $2 "\t" $4}' "$RESCUE_EXPORT/functions.tsv"
}

# re_src PATTERN: the decompiled C of the one function that matches; lists the matches if several.
re_src() {
  local m n
  m=$(re_fn "$1") || return
  n=$(printf '%s' "$m" | grep -c .)
  if [ "$n" = 1 ]; then cat "$RESCUE_EXPORT/$(printf '%s' "$m" | cut -f3)"
  else echo "$n matches for '$1':" >&2; printf '%s\n' "$m" | head -20 >&2; return 1; fi
}

# re_refs ADDR: every reference to ADDR (bare hex; matches ram and overlay forms).
# Prints: from<TAB>from_function<TAB>type
re_refs() {
  _re_check || return
  awk -F'\t' -v a="$(_re_bare "$1")" \
    'NR>1 {t=tolower($4); sub(/.*::/,"",t); if (t==a) print $1 "\t" $2 "\t" $3}' "$RESCUE_EXPORT/xrefs.tsv"
}

# re_grep REGEX: functions whose decompiled C matches (extended regex, case-insensitive).
re_grep() {
  _re_check || return
  (cd "$RESCUE_EXPORT" && grep -rliE -- "$1" ram ov_* | sed 's#.*/##; s#\.c$##; s#_#\t#')
}

# re_tree ADDR [DEPTH=2] [down|up] [MAX=15]: call tree from ADDR, one "address name" line per
# node, callees (down) or callers (up), at most MAX children per node.
# shortcut: nodes are keyed by bare address, so two overlays loaded at the same address merge;
# check with re_fn if a name looks wrong.
re_tree() {
  _re_check || return
  awk -F'\t' -v root="$(_re_bare "$1")" -v depth="${2:-2}" -v dir="${3:-down}" -v max="${4:-15}" '
    function bare(s) { s=tolower(s); sub(/.*::/,"",s); sub(/^0x/,"",s); return s }
    function walk(a, d,   n, i, k) {
      printf "%*s%s %s%s\n", d*2, "", a, (a in name ? name[a] : "?"), (a in path ? " (cycle)" : "")
      if (a in path) return
      n = split(kids[a], k, " ")
      if (d >= depth) { if (n) printf "%*s... %d more\n", d*2+2, "", n; return }
      path[a] = 1
      for (i = 1; i <= n && i <= max; i++) walk(k[i], d+1)
      if (n > max) printf "%*s... %d more\n", d*2+2, "", n-max
      delete path[a]
    }
    FNR == 1 { next }
    FILENAME ~ /functions\.tsv$/ { a = bare($1); name[a] = $2; addr[$2] = a; next }
    $3 ~ /CALL/ {
      c = addr[$2]; t = bare($4)
      if (c == "" || (c SUBSEP t) in seen) next
      seen[c, t] = 1
      if (dir == "up") kids[t] = kids[t] " " c; else kids[c] = kids[c] " " t
    }
    END { walk(root, 0) }
  ' "$RESCUE_EXPORT/functions.tsv" "$RESCUE_EXPORT/xrefs.tsv"
}

_re_bare() { local s; s=$(printf '%s' "$1" | tr 'A-Z' 'a-z'); s=${s##*::}; printf '%s' "${s#0x}"; }

# ---- GitHub state (read-only) -------------------------------------------------------------

# gh_pr_line PR...: one line per PR: number, state, draft, checks, latest review verdict, branch.
gh_pr_line() {
  local pr
  for pr in "$@"; do
    gh pr view "$pr" --json number,state,isDraft,statusCheckRollup,comments,headRefName --jq '
      ([.statusCheckRollup[] | (.conclusion // .state // "")] ) as $c
      | (if ($c|length)==0 then "none"
         elif any($c[]; test("FAILURE|ERROR|CANCELLED|TIMED_OUT|ACTION_REQUIRED")) then "fail"
         elif any($c[]; .=="" or test("PENDING|EXPECTED|QUEUED|IN_PROGRESS")) then "pending"
         else "pass" end) as $checks
      | ([.comments[].body | split("\n")[0] | capture("(?<v>PASS|REVISE|SPLIT|ESCALATE)")?.v] | last // "none") as $v
      | "#\(.number) \(.state)\(if .isDraft then " draft" else "" end) checks=\($checks) verdict=\($v) \(.headRefName)"'
  done
}

# gh_blockers N: open issues that block N, from its "blocked by" links and from body lines
# that mention "depend". Prints "#n" per open blocker; nothing means unblocked.
gh_blockers() {
  local n=$1 b
  {
    gh api "repos/{owner}/{repo}/issues/$n/dependencies/blocked_by" --jq '.[] | select(.state=="open") | .number'
    for b in $(gh issue view "$n" --json body --jq .body | grep -i depend | grep -oE '#[0-9]+' | tr -d '#'); do
      [ "$(gh issue view "$b" --json state --jq .state)" = OPEN ] && echo "$b"
    done
  } | sort -un | sed 's/^/#/'
}

# gh_ready EPIC: the epic's open sub-issues, each "ready" or "blocked by #a #b".
gh_ready() {
  local n t blk
  gh api "repos/{owner}/{repo}/issues/$1/sub_issues" --paginate --jq '.[] | select(.state=="open") | "\(.number)\t\(.title)"' |
  while IFS=$'\t' read -r n t; do
    blk=$(gh_blockers "$n" | tr '\n' ' ')
    printf '#%s %s: %s\n' "$n" "${blk:+blocked by $blk}${blk:-ready}" "$t"
  done
}

# ---- hc sessions (read-only) --------------------------------------------------------------

# orch_checkin NAME...: per session "name status changed|UNCHANGED", comparing the last 30
# screen lines with the previous check-in. UNCHANGED twice in a row means stuck (orchestrator.md).
orch_checkin() {
  local dir="${TMPDIR:-/tmp}/orch-checkin" s st h old ls
  mkdir -p "$dir"
  # hc ls has one flat object per live terminal with "name" and "status"; absent means gone.
  ls=$(hc ls | sed 's/},{/}\n{/g')
  for s in "$@"; do
    st=$(printf '%s\n' "$ls" | grep -F "\"name\":\"$s\"" | grep -oE '"status":"[a-z_]+"' | cut -d'"' -f4)
    h=$(hc read "$s" --lines 30 2>/dev/null | cksum | cut -d' ' -f1)
    old=$(cat "$dir/$s" 2>/dev/null); echo "$h" > "$dir/$s"
    printf '%s %s %s\n' "$s" "${st:-gone}" "$([ "$h" = "$old" ] && echo UNCHANGED || echo changed)"
  done
}

# ---- GitHub writes ------------------------------------------------------------------------
# Issue creation with labels, assignee, parent and blocked-by links: scripts/gh-issue.sh.

# gh_pr_finish PR WORKTREE ISSUE closes|refs: after a PASS review. Refuses unless the PR is
# ready (not draft) and its checks pass or there are none. Squash-merges, deletes the branch,
# removes the worktree, and for refs reopens the issue if the merge closed it.
gh_pr_finish() {
  local pr=$1 wt=$2 issue=$3 link=$4 line
  case "$link" in closes|refs) ;; *) echo "usage: gh_pr_finish PR WORKTREE ISSUE closes|refs" >&2; return 2 ;; esac
  line=$(gh_pr_line "$pr") || return
  case "$line" in
    *" OPEN draft "*) echo "still draft: $line" >&2; return 1 ;;
    *" OPEN checks=pass "*|*" OPEN checks=none "*) ;;
    *) echo "not mergeable yet: $line" >&2; return 1 ;;
  esac
  gh pr merge "$pr" --squash --delete-branch || return
  if [ -n "$wt" ] && [ -d "$wt" ]; then git worktree remove "$wt" || echo "worktree left: $wt" >&2; fi
  if [ "$link" = refs ] && [ "$(gh issue view "$issue" --json state --jq .state)" = CLOSED ]; then
    gh issue reopen "$issue" >/dev/null && echo "reopened #$issue (Refs merge)"
  fi
  echo "merged #$pr"
}
