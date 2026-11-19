# Agent harness scripts

**Status:** active
**Date:** 2026-11-18
**Roadmap phase:** tooling for every phase (the role sessions in [`prompts/`](../../prompts/))

## Goal
The role sessions (orchestrator, investigator, executor, reviewer) write only content: commit
messages, PR bodies, findings and the judgment part of a review. Every deterministic step
(setup, checks, formatting, posting, validation, reporting) runs in a script, so a cheap model
cannot get it wrong and an expensive one does not spend tokens on it. Rules the prompts state
are enforced by a guard, not only written down.

## Steps
1. **Phase A, executor and reviewer.** In `scripts/`:
   - `wt.sh`: create or reuse the sub-issue worktree.
   - `check-diff.sh`: scope, legal, dependency, `pub` and panic checks on a diff.
   - `verify.sh`: the CI steps, one PASS/FAIL line each.
   - `commit.sh`: check what is staged and add the engine's Co-Authored-By line.
   - `ship.sh`: rebase, check, verify, push, open the draft PR.
   - `review-prep.sh`: the mechanical half of a review.
   - `report.sh`: validate the final line, post the comment and send it to the orchestrator.
2. **Phase B, orchestrator.** In `scripts/orch/`:
   - `spawn.sh`: next free name, Ghidra copy, tab, dispatch text, engine start and model check.
   - `reap.sh`: save the screen, kill, remove and stop the bridge.
   - `epic-status.sh`: derive each sub-issue's state and next action from GitHub, worktrees
     and hc. This replaces `progress.md`; notes go in comments on the epic.
   - `merge.sh`: mark ready, check the checks and merge.
   - `cleanup-check.sh`: the end-of-epic checks.
3. **Phase C, guards.** `scripts/guard.sh` blocks commands a role must not run. A Claude Code
   `PreToolUse` hook and a pi extension both call it. It does nothing when `AGENT_ROLE` is unset,
   so sessions outside the orchestrator are not affected.
4. After each phase, shorten the prompts so they call the scripts.

## Depends on
- hc and herdr-app (session and tab management); pi with OpenCode Go for the overflow lane.

## Risks and open questions
- The guard matches command text. It stops honest mistakes, not a model that tries to work
  around it (`sh -c`, `eval`).
- `epic-status.sh` infers state from branch names (`<n>-<slug>`) and comment first lines
  (`PASS`, `REVISE`, ...). A branch or a verdict outside that format is not seen.

## Outcome
Filled in when the plan is done or dropped.
