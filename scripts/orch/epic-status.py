#!/usr/bin/env python3
"""The state of an epic, derived from GitHub, the worktrees and hc (replaces progress.md).

Usage: scripts/orch/epic-status.py EPIC [--all]

One line per sub-issue (open ones; --all adds closed ones):
  #N  ACTION  | PR, worktree, sessions | title
ACTION is the next step under the orchestrator's Workflow:
  running NAME...       a session with this issue's label is live: wait for it
  review PR             open PR with no verdict, or commits pushed after the last REVISE
  revise PR (k)         latest verdict REVISE; k = REVISE count so far
  split PR              3 REVISE verdicts: create the follow-up (Workflow step 5)
  merge PR              verdict PASS: scripts/orch/merge.sh PR
  fix-checks PR         verdict PASS but a check fails
  ask-user PR           verdict ESCALATE
  follow-up PR          verdict SPLIT
  resume WT             worktree without an open PR
  rest (merged PRs)     a Refs PR merged and the issue is still open
  blocked by #a #b      dependencies still open
  dispatch              ready to start
Branches are matched by name ("<n>-<slug>"); "sm/" branches belong to another tool.
Notes that progress.md used to hold go in comments on the epic.
"""
import json
import os
import re
import subprocess
import sys

VERDICT = re.compile(r"^[\s*]*(PASS|REVISE|SPLIT|ESCALATE)\b")


def run(*cmd):
    return subprocess.run(cmd, capture_output=True, text=True, check=True).stdout


def gh_json(*args):
    return json.loads(run("gh", *args) or "null")


def blockers(n):
    script = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "agent.sh")
    out = run("bash", "-c", f'source "{script}"; gh_blockers {n}')
    return out.split()


def sessions_by_issue():
    found = {}
    try:
        terms = json.loads(run("hc", "ls")).get("terminals", [])
    except (OSError, subprocess.CalledProcessError, ValueError):
        return found
    for t in terms:
        if not t.get("name"):
            continue
        try:
            labels = json.loads(run("hc", "get", t["name"])).get("labels") or {}
        except (subprocess.CalledProcessError, ValueError):
            continue
        if "task.issue" in labels:
            found.setdefault(int(labels["task.issue"]), []).append(t["name"])
    return found


def worktrees():
    out, path, wts = run("git", "worktree", "list", "--porcelain"), None, {}
    for line in out.splitlines():
        if line.startswith("worktree "):
            path = line[9:]
        elif line.startswith("branch refs/heads/"):
            m = re.match(r"(\d+)-", line[18:])
            if m and "rust-mm-worktrees" in path:
                wts.setdefault(int(m.group(1)), []).append(path)
    return wts


def pr_action(pr):
    d = gh_json("pr", "view", str(pr["number"]), "--json",
                "comments,commits,isDraft,statusCheckRollup")
    verdicts = [(m.group(1), c["createdAt"]) for c in d["comments"]
                if (m := VERDICT.match(c["body"]))]
    revises = sum(v == "REVISE" for v, _ in verdicts)
    last_push = d["commits"][-1]["committedDate"] if d["commits"] else ""
    n = f'#{pr["number"]}'
    if not verdicts:
        return f"review {n}"
    v, when = verdicts[-1]
    if v == "REVISE":
        if last_push > when:
            return f"review {n} (after revise {revises})"
        return f"split {n} (3 REVISE)" if revises >= 3 else f"revise {n} ({revises})"
    if v == "PASS":
        states = [(c.get("conclusion") or c.get("state") or "") for c in d["statusCheckRollup"]]
        if any(re.search("FAILURE|ERROR|CANCELLED|TIMED_OUT", s) for s in states):
            return f"fix-checks {n}"
        return f"merge {n}"
    return {"ESCALATE": f"ask-user {n}", "SPLIT": f"follow-up {n}"}[v]


def main():
    if len(sys.argv) < 2 or not sys.argv[1].isdigit():
        sys.exit(__doc__)
    epic, show_all = sys.argv[1], "--all" in sys.argv
    subs = json.loads(run("gh", "api", "--paginate",
                          f"repos/{{owner}}/{{repo}}/issues/{epic}/sub_issues"))
    prs = gh_json("pr", "list", "--state", "all", "--limit", "500",
                  "--json", "number,headRefName,state")
    by_issue = {}
    for p in prs:
        if m := re.match(r"(\d+)-", p["headRefName"]):
            by_issue.setdefault(int(m.group(1)), []).append(p)
    wts, live = worktrees(), sessions_by_issue()

    for s in sorted(subs, key=lambda s: s["number"]):
        n = s["number"]
        if s["state"] != "open" and not show_all:
            continue
        mine = by_issue.get(n, [])
        open_prs = [p for p in mine if p["state"] == "OPEN"]
        merged = [p for p in mine if p["state"] == "MERGED"]
        if s["state"] != "open":
            action = "done"
        elif n in live:
            action = "running " + " ".join(live[n])
        elif open_prs:
            action = pr_action(open_prs[-1])
        elif n in wts:
            action = "resume " + wts[n][0]
        elif merged:
            action = "rest (merged " + " ".join(f'#{p["number"]}' for p in merged) + ")"
        else:
            b = blockers(n)
            action = "blocked by " + " ".join(b) if b else "dispatch"
        where = ", ".join(filter(None, [
            "PRs " + " ".join(f'#{p["number"]}:{p["state"].lower()}' for p in mine) if mine else "",
            "wt " + " ".join(os.path.basename(w) for w in wts.get(n, [])) if n in wts else "",
        ]))
        print(f"#{n}  {action}  | {where or '-'} | {s['title']}")


if __name__ == "__main__":
    main()
