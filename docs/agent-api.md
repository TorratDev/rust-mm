# Agent helpers

Shell helpers for agent sessions. Work in "code mode": chain several helpers in **one** shell
command (loops, pipes, `grep`, `head`) and print only the result you need, instead of one tool
call per step and reading every intermediate output.

```sh
source scripts/agent.sh
```

Bash (Git Bash on Windows). The Ghidra helpers read the text export (`$RESCUE_EXPORT`, default
`~/rescue-re/export`, see [`ghidra-setup.md`](ghidra-setup.md#7-text-export-for-read-only-questions)).
They do not replace a live bridge for what the export cannot answer.

## Ghidra export (read-only)

| Helper | Prints |
|---|---|
| `re_fn PATTERN` | `address name file` for functions whose address or name matches (regex, any case) |
| `re_src PATTERN` | the decompiled C of the one matching function; with several matches, lists them and fails |
| `re_refs ADDR` | `from from_function type` for every reference to ADDR (ram and overlay forms) |
| `re_grep REGEX` | `address name` of functions whose C matches |
| `re_tree ADDR [DEPTH=2] [down\|up] [MAX=15]` | indented call tree: callees (`down`) or callers (`up`), MAX children per node |

```sh
# Who calls the spawn routine, two levels up, and what each caller's C says about gSpeedTurns:
re_tree 021826a0 2 up
for a in $(re_grep gSpeedTurns | cut -f1); do echo "== $a"; re_src "$a" | grep -n gSpeedTurns; done
```

## GitHub state (read-only)

| Helper | Prints |
|---|---|
| `gh_pr_line PR...` | `#PR STATE [draft] checks=pass\|fail\|pending\|none verdict=PASS\|REVISE\|SPLIT\|ESCALATE\|none branch` |
| `gh_blockers N` | `#n` for each open issue blocking N ("blocked by" links and body lines that mention "depend"); empty = unblocked |
| `gh_ready EPIC` | each open sub-issue of EPIC: `#n ready: title` or `#n blocked by #a #b: title` |

## hc sessions (read-only)

| Helper | Prints |
|---|---|
| `orch_checkin NAME...` | `name status changed\|UNCHANGED` (`gone` when not live), comparing the last 30 screen lines with the previous check-in |

## Writes (GitHub)

Run these as their own command, so the permission prompt shows what is written.

| Command | Does |
|---|---|
| `scripts/gh-issue.sh --title T --body-file F [--label L]... [--assignee U\|none] [--parent N] [--blocked-by N]...` | creates the issue with labels, assignee (default `@me`), sub-issue link and blocked-by links. `--issue N` updates an existing issue instead. Never use bare `gh issue create`. |
| `scripts/gh-pr.sh --title T --body-file F (--closes N \| --refs N \| --no-issue) [--draft]` | opens the PR with `Closes #N` or `Refs #N` as the first body line, assigned to `@me`, base `main`; refuses a Refs body with close/fix/resolve next to an issue number. Never use bare `gh pr create`. |
| `gh_pr_finish PR WORKTREE ISSUE closes\|refs` | after a PASS: refuses unless the PR is ready with checks passing or none; squash-merges, deletes the branch, removes the worktree, and for `refs` reopens the issue if the merge closed it |
