#!/usr/bin/env bash
# Determinism check for the simulation crates (rescue-sim, rescue-data).
#
# Clippy does most of the work (crates/{sim,data}/clippy.toml bans HashMap/HashSet,
# f32/f64, SystemTime/Instant, env vars and thread spawning; `clippy::float_arithmetic`
# is denied in both crates). Clippy cannot see a float that is only inferred, e.g.
# `let x = 1.5; x as u32`, so this script greps the sources for float types, float
# suffixes and float literals as a backstop.
#
# Line comments are ignored. To allow a line on purpose, end it with `// determinism: allow`
# and explain why in the same comment.
#
# Usage: scripts/check-determinism.sh [crate-dir ...]   (default: crates/sim crates/data)
set -euo pipefail

cd "$(dirname "$0")/.."

dirs=("$@")
if [ ${#dirs[@]} -eq 0 ]; then
    dirs=(crates/sim crates/data)
fi

# ERE patterns, applied to each line after stripping `//` comments.
patterns=(
    '(^|[^[:alnum:]_])f(32|64)([^[:alnum:]_]|$)'          # f32 / f64 as a type or path
    '[0-9_]f(32|64)([^[:alnum:]_]|$)'                     # 1f32, 1.0_f64
    '(^|[^.[:alnum:]_])[0-9][0-9_]*\.[0-9]'               # 1.5 (not tuple fields like x.0.1)
    '(^|[^.[:alnum:]_])[0-9][0-9_]*[eE][+-]?[0-9]'        # 1e5 (not hex like 0x1e5)
)

combined=$(IFS='|'; echo "${patterns[*]}")

failed=0
while IFS= read -r -d '' file; do
    # Blank out allowed lines and strip comments, keeping line numbers intact.
    if hits=$(sed -e 's#.*// determinism: allow.*##' -e 's#//.*##' "$file" | grep -nE -- "$combined"); then
        while IFS= read -r hit; do
            echo "error: float use in deterministic crate: $file:${hit%%:*}: $(sed -n "${hit%%:*}p" "$file")"
        done <<< "$hits"
        failed=1
    fi
done < <(find "${dirs[@]}" -name '*.rs' -not -path '*/target/*' -print0)

if [ "$failed" -ne 0 ]; then
    echo
    echo "The simulation must use integer or fixed-point math only (see docs/roadmap.md)."
    exit 1
fi
echo "determinism check passed: no float types or literals in ${dirs[*]}"
