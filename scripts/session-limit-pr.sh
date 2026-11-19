#!/usr/bin/env bash
# StopFailure hook: when a session dies on a usage/rate limit, save what the chat had found
# so far as a draft PR, so the next session can pick it up instead of starting over.
#
# Reads the hook JSON on stdin (needs transcript_path and cwd). It then:
# - writes docs/handoff/<session>.md: the user's prompts and Claude's text replies, in order;
# - commits that file plus any uncommitted work on the session's branch (a new branch if on main);
# - pushes and opens a draft PR with scripts/gh-pr.sh, or comments on the branch's open PR.
#
# Manual use: scripts/session-limit-pr.sh [--dry-run] < hook.json
#   --dry-run writes the notes file and prints what it would do; no commit, push or PR.
set -euo pipefail

dry=0; [ "${1:-}" = --dry-run ] && dry=1
input=$(cat)
# Only act on limit errors; other API failures (overload, network) are not a cut.
grep -qiE 'limit' <<<"$input" || exit 0

read -r transcript cwd session < <(python3 -c '
import json,sys; d=json.load(sys.stdin)
print(d.get("transcript_path",""), d.get("cwd","."), d.get("session_id","session")[:8])' <<<"$input")
[ -f "$transcript" ] || { echo "session-limit-pr: no transcript at '$transcript'" >&2; exit 0; }
cd "$cwd"
root=$(git rev-parse --show-toplevel)
cd "$root"

notes="docs/handoff/$session.md"
mkdir -p docs/handoff
python3 - "$transcript" > "$notes" <<'PY'
import json, sys
out = ["# Session cut by usage limit\n",
       "Findings from the chat before the cut, oldest first. Unverified: check before relying on them.\n"]
for line in open(sys.argv[1]):
    try: d = json.loads(line)
    except ValueError: continue
    if d.get("isMeta") or d.get("isSidechain"): continue
    role, c = d.get("type"), (d.get("message") or {}).get("content")
    if role not in ("user", "assistant"): continue
    texts = [c] if isinstance(c, str) else [b.get("text", "") for b in c or [] if b.get("type") == "text"]
    for t in texts:
        t = t.strip()
        # Skip harness-injected blocks (reminders, command output) that are not the user's words.
        if not t or t.startswith("<"): continue
        out.append(f"## {'User' if role == 'user' else 'Claude'}\n\n{t}\n")
print("\n".join(out))
PY
entries=$(grep -c '^## ' "$notes" || true)
[ "$entries" -gt 0 ] || { echo "session-limit-pr: transcript had no text to save" >&2; rm -f "$notes"; exit 0; }

branch=$(git branch --show-current)
if [ "$dry" = 1 ]; then
  echo "would commit $notes ($entries entries) and uncommitted work on '${branch:-detached}', push, open draft PR"
  exit 0
fi

if [ -z "$branch" ] || [ "$branch" = main ]; then
  branch="claude/session-limit-$session"; git switch -c "$branch"
fi
git add -A
# --no-verify: this is a rescue snapshot in a draft; a failing pre-commit check must not lose it.
git commit -q --no-verify -m "WIP: session cut by usage limit ($session)" \
  -m "Notes from the chat in $notes; uncommitted work at the cut is included as is." || true
git push -q -u origin "$branch"

# The body points at the notes instead of holding them: chat text can say "fixes #N" (which
# gh-pr.sh refuses for a no-issue PR) and can exceed GitHub's body size limit.
body=$(mktemp); trap 'rm -f "$body"' EXIT
printf 'A session hit its usage limit. What the chat had found before the cut is in [`%s`](%s), and any uncommitted work is in the last commit.\n\nPick it up in a new session, then drop or rewrite the notes file before merging.\n' \
  "$notes" "../blob/$branch/$notes" > "$body"
pr=$(gh pr list --head "$branch" --state open --json number --jq '.[0].number' 2>/dev/null || true)
if [ -n "$pr" ]; then
  gh pr comment "$pr" --body-file "$body"
else
  scripts/gh-pr.sh --draft --no-issue --title "WIP: session cut by usage limit ($session)" --body-file "$body"
fi
