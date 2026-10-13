#!/usr/bin/env bash
# Dependency rule (docs/roadmap.md): `rescue-sim` and `rescue-data` depend on nothing
# platform-related. Only `rescue-platform` and `apps/*` may use SDL3.
#
# Fails if a banned crate appears anywhere in the normal or build dependency graph of a
# guarded crate, on any target platform. Dev-dependencies (test-only) are not checked.
#
# Usage: scripts/check-deps.sh
set -euo pipefail

cd "$(dirname "$0")/.."

guarded=(rescue-sim rescue-data)

# Crate names (anchored regex) that must not reach the guarded crates:
#  - SDL bindings and other windowing/graphics/audio/input stacks;
#  - our own platform-facing crates;
#  - OS entropy and thread RNGs (the sim owns a single seeded RNG).
banned='^(sdl[0-9a-z_-]*|winit|wgpu[a-z_-]*|glow|glutin[a-z_-]*|egui[a-z_-]*|eframe|cpal|gilrs[a-z_-]*|pixels|rescue-platform|rescue-render|rescue-audio|rescue-game|getrandom|rand)$'

failed=0
for krate in "${guarded[@]}"; do
    deps=$(cargo tree --locked -p "$krate" -e normal,build --target all --prefix none \
        --format '{p}' | awk '{print $1}' | sort -u)
    bad=$(printf '%s\n' "$deps" | grep -E "$banned" || true)
    if [ -n "$bad" ]; then
        echo "error: $krate must not depend on platform crates, but its dependency graph contains:"
        printf '  %s\n' $bad
        echo "  how it gets there:"
        for b in $bad; do
            cargo tree --locked -p "$krate" -e normal,build --target all -i "$b" | sed 's/^/    /'
        done
        failed=1
    else
        echo "ok: $krate ($(printf '%s\n' "$deps" | grep -c .) crates in graph, none banned)"
    fi
done

exit "$failed"
