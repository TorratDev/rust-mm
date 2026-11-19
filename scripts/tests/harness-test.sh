#!/usr/bin/env bash
# Self-test for the harness scripts that need no GitHub, hc or cargo: wt.sh (create path),
# check-diff.sh, commit.sh, report.sh (line checks) and ship.sh (refusals).
# Runs in a throwaway repo pair (bare origin + main checkout). Usage: scripts/tests/harness-test.sh
set -uo pipefail
S=$(cd "$(dirname "$0")/.." && pwd)
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
pass=0 fails=0
ok() { if eval "$2" >/dev/null 2>&1; then pass=$((pass+1)); else fails=$((fails+1)); echo "FAIL: $1"; fi; }

git init -q --bare -b main "$tmp/origin.git"
git clone -q "$tmp/origin.git" "$tmp/rust-mm" 2>/dev/null
cd "$tmp/rust-mm"
git config user.email t@t; git config user.name t
mkdir -p crates/sim/src crates/nds/src docs
echo 'fn a() {}' > crates/sim/src/a.rs; echo 'fn p() {}' > crates/nds/src/p.rs; echo x > docs/x.md
git add -A; git commit -qm init; git push -q origin main 2>/dev/null

wt=$("$S/wt.sh" 7 demo | tail -1)
ok "wt.sh creates the worktree on 7-demo" '[ "$(git -C "$wt" branch --show-current)" = 7-demo ]'
git -C "$wt" config user.email t@t; git -C "$wt" config user.name t

echo 'pub fn b() {}' >> "$wt/crates/sim/src/a.rs"
echo 'fn q() { x.unwrap(); }' >> "$wt/crates/nds/src/p.rs"
git -C "$wt" add -A
out=$("$S/check-diff.sh" "$wt" --staged --files "crates/{sim,nds}/src/*.rs")
ok "scope accepts {a,b} and * patterns" 'grep -q "^scope OK" <<< "$out"'
ok "pub item is flagged" 'grep -q "^pub WARN" <<< "$out"'
ok "unwrap in parser code is flagged" 'grep -q "^panic WARN" <<< "$out"'
ok "scope rejects an unnamed file" '! "$S/check-diff.sh" "$wt" --staged --files "crates/sim/"'
git -C "$wt" reset -q

touch "$wt/game.nds"
ok "commit.sh refuses a ROM file" '! "$S/commit.sh" "$wt" -m "sim: x" --engine claude --model m1'
ok "and leaves nothing staged" '[ -z "$(git -C "$wt" diff --cached --name-only)" ]'
rm "$wt/game.nds"
ok "commit.sh refuses without an engine" '! env -u AGENT_ENGINE "$S/commit.sh" "$wt" -m "sim: x"'
ok "commit.sh commits in scope" '"$S/commit.sh" "$wt" -m "sim: b" --files "crates/" --engine pi --model go/m'
msg=$(git -C "$wt" log -1 --format=%B)
ok "subject gets the issue number" 'grep -q "^sim: b (#7)$" <<< "$msg"'
ok "trailer names the engine and model" 'grep -q "^Co-Authored-By: pi go/m <noreply@pi.dev>$" <<< "$msg"'
out=$("$S/check-diff.sh" "$wt" --files "crates/")
ok "check-diff vs origin/main sees the commit" 'grep -q "^scope OK" <<< "$out"'

echo dirty >> "$wt/docs/x.md"
ok "ship.sh refuses uncommitted changes" '! "$S/ship.sh" "$wt" --refs 7'
ok "ship.sh needs a PR link" '! "$S/ship.sh" "$wt"'

ok "report.sh rejects a wrong executor line" '! "$S/report.sh" "OK done" --role executor'
ok "report.sh rejects DONE without a PR URL" '! "$S/report.sh" "DONE: all good" --role executor'
ok "report.sh rejects a verdict without its file" '! "$S/report.sh" "PASS: fine" --role reviewer'
echo "REVISE: no" > "$tmp/v.md"
ok "report.sh rejects a verdict file with another word" '! "$S/report.sh" "PASS: fine" --role reviewer --pr-comment 1 "$tmp/v.md"'
ok "report.sh rejects investigator DONE without findings" '! "$S/report.sh" "DONE: x" --role investigator'
ok "report.sh prints a valid BLOCKED line" '[ "$(env -u ORCH "$S/report.sh" "BLOCKED: no dump" --role executor --ghidra none 2>/dev/null)" = "BLOCKED: no dump" ]'

echo "harness-test: $pass passed, $fails failed"
[ "$fails" = 0 ]
