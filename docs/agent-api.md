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

## Task scripts (role sessions)

Each does one mechanical step from the role prompts and prints a short verdict, so a session
writes only content. They read `TASK_FILES`, `AGENT_ROLE`, `AGENT_ENGINE`, `AGENT_MODEL`,
`AGENT_GHIDRA` and `ORCH` from the environment the orchestrator starts them with. Plan:
[`plans/agent-harness-scripts.md`](plans/agent-harness-scripts.md). Self-test:
`scripts/tests/harness-test.sh`.

| Script | Does |
|---|---|
| `scripts/wt.sh N SLUG` | creates the worktree `../rust-mm-worktrees/N-SLUG` from the latest main, or reuses it and prints its commits, status and latest REVISE comment; the path is the last line |
| `scripts/check-diff.sh WT [--staged] [--files LIST]` | `scope`, `legal`, `deps`, `pub` and `panic` lines (OK, WARN or FAIL); exit 1 on a FAIL |
| `scripts/verify.sh [WT]` | the CI steps (fmt, deps, determinism, clippy, tests, doctests), one PASS/FAIL line each, ending `verify: PASS (6/6)` or `verify: FAIL (...)` |
| `scripts/commit.sh WT -m SUBJECT [-m PARA]...` | stages all, refuses out-of-scope or forbidden files, adds `(#N)` and the engine's Co-Authored-By line |
| `scripts/ship.sh WT --closes N\|--refs N [--title T --body-file F]` | rebase (exit 3 with a BLOCKED line on conflict), check-diff, verify, push, draft PR with the attribution; last line `ship: <URL> <verify>` |
| `scripts/review-prep.sh PR` | the reviewer's mechanical checks for the PR's worktree, saved for `report.sh` to append to the verdict |
| `scripts/report.sh LINE [--pr-comment PR F] [--issue-comment N F]` | checks the line's format for the role, posts and reads back the comment, stops the Ghidra bridge, sends the line to `$ORCH` |

## Orchestrator scripts

| Script | Does |
|---|---|
| `scripts/orch/epic-status.py EPIC [--all]` | one line per open sub-issue: next action (running, review, revise, split, merge, fix-checks, ask-user, follow-up, resume, rest, blocked, dispatch), PRs, worktree, sessions. Replaces `progress.md` |
| `scripts/orch/spawn.sh ROLE --issue N ...` | starts a role session with its task: next free name, Ghidra copy, model, role tab, dispatch text, the role scripts' environment, model check; `--dry-run` prints it instead |
| `scripts/orch/reap.sh NAME...` | saves the screen, prints the report line, kills and removes the terminal, stops its Ghidra bridge |
| `scripts/orch/merge.sh PR` | after a PASS: `gh pr ready`, then `gh_pr_finish` once checks pass; exit 75 while pending |
| `scripts/orch/cleanup-check.sh` | end-of-epic checks: bridges, worktrees, sessions, stray files in the main checkout |

## Guard

`scripts/guard.sh` runs before every shell command of a session that has `AGENT_ROLE` set, through
the Claude Code `PreToolUse` hook in [`.claude/settings.json`](../.claude/settings.json) and the pi
extension [`scripts/pi/guard.ts`](../scripts/pi/guard.ts) (spawn.sh loads it). It blocks what the
role must not run (bare `gh issue create` / `gh pr create`, merges and session control outside the
orchestrator, executor `git commit`/`push` instead of the scripts, git writes in the main checkout,
writes from read-only roles, another session's Ghidra copy) and says what to use instead.
Sessions without `AGENT_ROLE` are not affected.

## Writes (GitHub)

Run these as their own command, so the permission prompt shows what is written.

| Command | Does |
|---|---|
| `scripts/gh-issue.sh --title T --body-file F [--label L]... [--assignee U\|none] [--parent N] [--blocked-by N]...` | creates the issue with labels, assignee (default `@me`), sub-issue link and blocked-by links. `--issue N` updates an existing issue instead. Never use bare `gh issue create`. |
| `scripts/gh-pr.sh --title T --body-file F (--closes N \| --refs N \| --no-issue) [--draft]` | opens the PR with `Closes #N` or `Refs #N` as the first body line, assigned to `@me`, base `main`; refuses a Refs body with close/fix/resolve next to an issue number. Never use bare `gh pr create`. |
| `gh_pr_finish PR WORKTREE ISSUE closes\|refs` | after a PASS: refuses unless the PR is ready with checks passing or none; squash-merges, deletes the branch, removes the worktree, and for `refs` reopens the issue if the merge closed it |
