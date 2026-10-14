#!/bin/sh
# Fetch pret/pmd-red (Red Rescue Team GBA decompilation) as a read-only
# logic reference. It goes into reference/pmd-red, which .gitignore excludes:
# pmd-red has no license file, so it is never vendored into this repository.
# See docs/references.md and docs/legal.md.
#
# Usage: tools/fetch_pmd_red.sh [COMMIT]
#   COMMIT defaults to the commit recorded in docs/references.md.
#   Pass "latest" to take the current default branch tip instead.
set -eu

URL=https://github.com/pret/pmd-red
PINNED=013475aa04f5be3191e5527c186d9bfceae7cae0
REV=${1:-$PINNED}

root=$(cd "$(dirname "$0")/.." && pwd)
dest=$root/reference/pmd-red

if [ ! -d "$dest/.git" ]; then
    mkdir -p "$dest"
    git -C "$dest" init -q
    git -C "$dest" remote add origin "$URL"
fi

if [ "$REV" = latest ]; then
    git -C "$dest" fetch -q --depth 1 origin HEAD
else
    git -C "$dest" fetch -q --depth 1 origin "$REV"
fi
git -C "$dest" checkout -q --detach FETCH_HEAD

echo "pmd-red at $(git -C "$dest" rev-parse HEAD) in $dest"
echo "Read-only reference: do not copy its code or data into this repository."
