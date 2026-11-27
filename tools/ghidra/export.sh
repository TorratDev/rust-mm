#!/usr/bin/env bash
# Dump the rescue-arm9 project to text (tools/ghidra/RescueExport.java) and stamp it with
# the docs/symbols.md it was made from, so a later check can tell when it is stale.
# See docs/ghidra-setup.md, section 7.
#
# Usage: tools/ghidra/export.sh EXPORT_DIR [PROJECT_DIR] [PROGRAM]
#        tools/ghidra/export.sh --check EXPORT_DIR
#   EXPORT_DIR   empty or missing folder, outside the repository (docs/legal.md)
#   PROJECT_DIR  folder holding the Ghidra project (default: ~/rescue-re/ghidra)
#   PROGRAM      program in the project (default: arm9.bin); the project name is
#                $RESCUE_PROJECT (default: rescue-arm9)
#   --check      exit 0 if EXPORT_DIR was made from the current docs/symbols.md,
#                1 if it is stale or unfinished
#
# Needs GHIDRA_INSTALL_DIR (the folder with ghidraRun) and a JDK 21 on PATH or JAVA_HOME.
set -euo pipefail

usage() { sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//'; exit 2; }
script_dir=$(cd "$(dirname "$0")" && pwd)
repo_root=$(cd "$script_dir/../.." && pwd)
symbols=$repo_root/docs/symbols.md

sha1_of() {
  if command -v sha1sum >/dev/null 2>&1; then sha1sum "$1" | cut -d' ' -f1
  else shasum -a 1 "$1" | cut -d' ' -f1; fi
}

if [ "${1:-}" = "--check" ]; then
  [ $# -eq 2 ] || usage
  stamp=$2/STAMP
  [ -f "$stamp" ] || { echo "stale: no finished export in $2 (no STAMP)"; exit 1; }
  recorded=$(sed -n 's/^symbols\.md-sha1: //p' "$stamp")
  current=$(sha1_of "$symbols")
  if [ "$recorded" = "$current" ]; then
    echo "fresh: $2 was exported from the current docs/symbols.md ($(head -1 "$stamp"))"
    exit 0
  fi
  echo "stale: docs/symbols.md changed since the export in $2; rerun tools/ghidra/export.sh" >&2
  exit 1
fi

[ $# -ge 1 ] && [ $# -le 3 ] || usage
out=$(realpath -m "$1")
project_dir=$(realpath -m "${2:-$HOME/rescue-re/ghidra}")
program=${3:-arm9.bin}
project=${RESCUE_PROJECT:-rescue-arm9}

: "${GHIDRA_INSTALL_DIR:?set GHIDRA_INSTALL_DIR to your Ghidra install folder}"
headless=$GHIDRA_INSTALL_DIR/support/analyzeHeadless
[ -x "$headless" ] || { echo "error: $headless not found or not executable" >&2; exit 1; }

case "$out/" in
  "$repo_root"/*)
    if ! git -C "$repo_root" check-ignore -q "$out/x" 2>/dev/null; then
      echo "error: $out is inside the repository and not git-ignored (docs/legal.md)" >&2
      exit 1
    fi ;;
esac
if [ -d "$out" ] && [ -n "$(ls -A "$out")" ]; then
  echo "error: $out is not empty; remove it first so no stale file survives" >&2
  exit 1
fi
mkdir -p "$out"

# Read the symbols before the run: an edit during the export is then reported as stale.
symbols_sha=$(sha1_of "$symbols")
version=$(sed -n 's/^application\.version=//p' "$GHIDRA_INSTALL_DIR/Ghidra/application.properties" 2>/dev/null || true)
log=$out.log
start=$(date +%s)
status=0
"$headless" "$project_dir" "$project" -process "$program" -noanalysis -readOnly \
  -scriptPath "$script_dir" -postScript RescueExport.java "$out" > "$log" 2>&1 || status=$?
elapsed=$(( $(date +%s) - start ))

# analyzeHeadless can exit 0 when a script fails, so the STAMP (written last) is the proof.
if [ "$status" -ne 0 ] || [ ! -f "$out/STAMP" ] || [ ! -f "$out/FAILED" ]; then
  echo "error: export did not finish (exit $status); see $log" >&2
  exit 1
fi
{
  echo "symbols.md-sha1: $symbols_sha"
  echo "ghidra: ${version:-unknown}"
  echo "elapsed-seconds: $elapsed"
} >> "$out/STAMP"
cat "$out/STAMP"
echo "failed functions: $(wc -l < "$out/FAILED") (listed in $out/FAILED)"
echo "OK: $out (elapsed ${elapsed}s)"
