#!/usr/bin/env bash
# Start one role session with its task (orchestrator: Session management). Picks the next free
# name, the Ghidra copy, the model and the session's own tab, writes the dispatch text, starts Claude
# or pi under hc with the environment the role scripts read, and checks it came up.
#
# Usage: scripts/orch/spawn.sh ROLE --issue N [fields] [--engine pi|go|claude] [--model M] [--dry-run]
#   Every role gets RESCUE_ROM (the main checkout's pmdb.nds) and RESCUE_PY (the py-desmume
#   interpreter, default ~/venvs/desmume/bin/python). An investigator is refused when either is
#   missing. A tracker issue with a "## Members" section names its member issues in the dispatch.
#   --engine pi (default): pi with Claude models. go: pi with OpenCode Go models (on a Claude
#   limit). claude: Claude Code.
#   investigator  --points "P1, P2" [--findings FILE]
#   executor      --slug S --objective O --files "F ..." --verify CMD --evidence E
#                 --link closes|refs [--revise PR | --fix-checks "NAME: OUTPUT" | --resume]
#   reviewer      --pr PR --slug S --objective O --files "F ..." --executor-line LINE
# --dry-run prints the hc command and the dispatch text and starts nothing.
# Last line: "started NAME engine=E model=M ghidra=C tab=T".
set -euo pipefail
. "$(dirname "$0")/../harness-lib.sh"

role=${1:?usage: spawn.sh ROLE --issue N ...}; shift
declare -A a=([engine]=pi)
dry=0
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) dry=1; shift ;;
    --resume) a[resume]=1; shift ;;
    --*) a[${1#--}]=$2; shift 2 ;;
    *) die "unknown argument: $1" ;;
  esac
done
need() { local k; for k in "$@"; do [ -n "${a[$k]:-}" ] || die "$role needs --$k"; done; }
need issue
n=${a[issue]} engine=${a[engine]}
case "$engine" in pi|go|claude) ;; *) die "--engine must be pi, go or claude" ;; esac

# Model per role and engine; this table is the one source (orchestrator.md points here).
declare -A models=(
  [pi.investigator]=anthropic/claude-sonnet-5-5 [go.investigator]=opencode-go/beam-free
  [pi.executor]=anthropic/claude-sonnet-5-5     [go.executor]=opencode-go/glm-5.3-flash
  [pi.reviewer]=anthropic/claude-opus-5-5       [go.reviewer]=opencode-go/beam-free
  [claude.investigator]=claude-sonnet-5-5 [claude.executor]=claude-sonnet-5-5 [claude.reviewer]=claude-opus-5-5
)
model=${a[model]:-${models[$engine.$role]:-}}
[ -n "$model" ] || die "unknown role: $role (investigator, executor or reviewer)"
[ "$engine" = go ] && engine=pi  # go is pi with other models from here on

orch=${HC_TERMINAL_ID:-}
[ -n "$orch" ] || [ "$dry" = 1 ] || die "HC_TERMINAL_ID is empty: run the orchestrator under hc"
repo=$(main_repo "$HARNESS_DIR")
rom="$repo/pmdb.nds" py=${RESCUE_PY:-$HOME/venvs/desmume/bin/python}
if [ "$role" = investigator ]; then
  [ -f "$rom" ] || die "spawn.sh: no ROM at $rom; put the supported dump there (docs/supported-dumps.md)"
  "$py" -I -c 'import desmume' 2>/dev/null || die "spawn.sh: $py cannot import desmume (py-desmume)"
fi
# Members of a batch tracker: the #N numbers in its "## Members" section.
members=$(gh issue view "$n" --json body --jq .body 2>/dev/null | python3 -c 'import re,sys
m = re.search(r"^## Members\n(.*?)(?=^## |\Z)", sys.stdin.read(), re.S | re.M)
print(" ".join(sorted(set(re.findall(r"#(\d+)", m.group(1))), key=int)) if m else "")' || true)
live=$(hc ls 2>/dev/null | python3 -c 'import json,sys
for t in json.load(sys.stdin).get("terminals", []):
    if t.get("name"): print(t["name"])' || true)
free() { ! grep -qxF "$1" <<< "$live"; }

# Next free name in the role's sequence; its Ghidra copy follows from the name.
name=""
case "$role" in
  investigator) for l in a b c d e f g h; do free "investigator-$l" && { name=investigator-$l; copy=rescue-arm9-$l; break; }; done ;;
  executor) for i in $(seq 1 30); do free "executor-$i" && { name=executor-$i; copy=none; break; }; done ;;
  reviewer) free reviewer && { name=reviewer; copy=rescue-arm9-r; } ||
    for i in $(seq 2 9); do free "reviewer-$i" && { name=reviewer-$i; copy=rescue-arm9-r$i; break; }; done ;;
esac
[ -n "$name" ] || die "no free $role name"
[ "$copy" = none ] || [ -d "$HOME/rescue-re/ghidra/$copy.rep" ] || copy=none

case "$role" in
  investigator)
    need points
    findings=${a[findings]:-$(cache_dir findings)/$n-$name.md}
    files=""
    task="Dispatch: you are investigator ${name#investigator-}. Engine and model: $engine $model. Ghidra copy: $copy. Sub-issue #$n.${members:+ Member issues: $members.} Your points: ${a[points]}. Findings file: $findings. Start now." ;;
  executor)
    need slug objective files verify evidence link
    case "${a[link]}" in closes) link="Closes #$n" ;; refs) link="Refs #$n" ;; *) die "--link closes|refs" ;; esac
    branch="$n-${a[slug]}" wt=$(wt_path "$branch" "$repo") files=${a[files]}
    found=$(gh issue view "$n" --json comments --jq '[.comments[].body | select(test("suggested task|\\| ID \\|"; "i"))] | length')
    [ "$found" -gt 0 ] && findings="the investigator comments on #$n" || findings=none
    extra=""
    [ -n "${a[revise]:-}" ] && extra=" Revise: PR #${a[revise]}. The worktree and branch already hold the work. Read git log and the latest review comment on the PR, and make every change it lists."
    [ -n "${a[fix-checks]:-}" ] && extra=" Fix checks: the worktree and branch already hold the work. Failing check: ${a[fix-checks]}."
    [ -n "${a[resume]:-}" ] && extra=" Resume: the worktree already holds earlier work; read git log and git status first."
    task="Dispatch: you are executor ${name#executor-}. Engine and model: $engine $model. Sub-issue #$n.${members:+ Member issues: $members.} Worktree: $wt. Branch: $branch. Objective: ${a[objective]}. Files you may touch: $files. Verification: ${a[verify]}. Evidence to record: ${a[evidence]}. Findings: $findings. PR link: $link.$extra Start now." ;;
  reviewer)
    need pr slug objective files executor-line
    branch="$n-${a[slug]}" wt=$(wt_path "$branch" "$repo") files=${a[files]}
    task="Dispatch: you are $name. Engine and model: $engine $model. Ghidra copy: $copy. Sub-issue #$n.${members:+ Member issues: $members.} PR #${a[pr]}. Worktree: $wt. Branch: $branch. Objective: ${a[objective]}. Files the task named: $files. Executor's line: ${a[executor-line]}. Start now." ;;
esac

env_args=(env ORCH="$orch" AGENT_ROLE="$role" AGENT_NAME="$name" AGENT_ENGINE="$engine"
  AGENT_MODEL="$model" AGENT_GHIDRA="$copy" TASK_FILES="$files" TASK_ISSUE="$n"
  RESCUE_ROM="$rom" RESCUE_PY="$py")
role_line="Read prompts/$role.md in your working directory; it defines your role."
readonly_role=0; [ "$role" = executor ] || readonly_role=1
if [ "$engine" = claude ]; then
  # The prompt goes before --disallowedTools, which takes every argument after it.
  agent=(claude --model "$model" "$role_line $task")
  [ "$readonly_role" = 1 ] && agent+=(--disallowedTools Edit Write NotebookEdit)
else
  # Claude sessions get scripts/guard.sh from the hook in .claude/settings.json; pi from this extension.
  agent=(pi --model "$model" --name "$name" --append-system-prompt "prompts/$role.md" -e "$repo/scripts/pi/guard.ts")
  [ "$readonly_role" = 1 ] && agent+=(--exclude-tools edit,write)
fi

# Each session gets its own tab in the orchestrator's space, named after the session.
space=""
[ -n "$orch" ] && space=$(hc get "$orch" | python3 -c 'import json,sys; print((json.load(sys.stdin).get("labels") or {}).get("herdr.space",""))')
run=(hc run --bg --name "$name" --cwd "$repo" --label "task.issue=$n" --label "task.role=$role")
[ -n "$space" ] && run+=(--label "herdr.space=$space")
run+=("${env_args[@]}" "${agent[@]}")

if [ "$dry" = 1 ]; then
  printf '+'; printf ' %q' "${run[@]}"; echo
  [ "$engine" = pi ] && echo "+ hc wait $name; hc send $name <task>"
  echo "task: $task"
  exit 0
fi

state=$("${run[@]}" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("state",""))')
[ "$state" = live ] || die "spawn.sh: $name did not start (state: $state)"
if [ "$engine" = pi ]; then
  hc wait "$name" --timeout 60 >/dev/null
  hc send "$name" "$role_line $task" >/dev/null
fi
if [ -n "$space" ]; then
  tabid=$(herdr-app ls | python3 -c 'import json,sys
sp, name = sys.argv[1], sys.argv[2]
for s in json.load(sys.stdin).get("spaces", []):
    for t in s.get("tabs", []) if s["id"] == sp else []:
        if any(x.get("name") == name for x in t.get("terminals", [])): print(t["id"])' "$space" "$name" | head -1)
  [ -z "$tabid" ] || herdr-app do tab.rename --id "$tabid" --name "$name" >/dev/null
fi

# Model check: Claude shows the family in its header (sonnet, opus), pi the model in its footer.
want=$model; [ "$engine" = claude ] && want=$(cut -d- -f2 <<< "$model"); want=${want#*/}; want=${want%%:*}
seen=0
for _ in 1 2 3 4 5 6 7 8 9 10; do
  hc read "$name" --lines 0 2>/dev/null | grep -qi -- "$want" && { seen=1; break; }
  sleep 1
done
[ "$seen" = 1 ] || echo "warning: '$want' not on $name's screen yet; check with hc read $name --lines 0" >&2
echo "started $name engine=$engine model=$model ghidra=$copy tab=$name"
