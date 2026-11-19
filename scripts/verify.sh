#!/usr/bin/env bash
# The CI checks (.github/workflows/ci.yml) on a worktree, one line per step, so a session reads
# a verdict instead of build output. Every step runs even after a failure.
#
# Usage: scripts/verify.sh [WT]          (default: the current directory)
#
# Prints "<step> PASS|FAIL <seconds>s"; a FAIL adds the last 25 lines of that step's log.
# Last line: "verify: PASS (6/6)" or "verify: FAIL (<steps>)". Full logs stay in the printed
# directory, outside the repo. Exit 1 on any FAIL. Not run: the headless game smoke run and
# the fuzz targets (CI only).
set -uo pipefail
. "$(dirname "$0")/harness-lib.sh"

wt=$(cd "${1:-.}" && pwd) || exit 2
logs=$(cache_dir "verify/$(basename "$wt")")
if cargo nextest --version >/dev/null 2>&1; then
  tests="cargo nextest run --locked --workspace --no-tests=pass"
else
  tests="cargo test --locked --workspace"
fi
steps=(
  "fmt|cargo fmt --all --check"
  "deps|scripts/check-deps.sh"
  "determinism|scripts/check-determinism.sh"
  "clippy|cargo clippy --locked --workspace --all-targets -- -D warnings"
  "test|$tests"
  "doctest|cargo test --locked --workspace --doc"
)

failed=()
for s in "${steps[@]}"; do
  name=${s%%|*} cmd=${s#*|} t0=$SECONDS
  if (cd "$wt" && bash -c "$cmd") > "$logs/$name.log" 2>&1; then
    echo "$name PASS $((SECONDS - t0))s"
  else
    echo "$name FAIL $((SECONDS - t0))s: $cmd"
    tail -25 "$logs/$name.log" | sed 's/^/    /'
    failed+=("$name")
  fi
done
echo "logs: $logs"
if [ ${#failed[@]} -eq 0 ]; then echo "verify: PASS (${#steps[@]}/${#steps[@]})"
else echo "verify: FAIL (${failed[*]})"; exit 1; fi
