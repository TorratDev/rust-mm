#!/usr/bin/env bash
# Build a Ghidra project with Blue's ARM9 and all ARM9 overlays mapped at their real
# addresses, from the output of tools/nds_extract_code.py. See docs/ghidra-setup.md.
#
# Usage: tools/ghidra/import.sh CODE_DIR PROJECT_DIR [PROJECT_NAME] [--no-analysis]
#                               [--check ADDR ...]
#   CODE_DIR      directory holding manifest.json, arm9.bin, overlay_*.bin
#   PROJECT_DIR   where the Ghidra project goes: outside the repository or in an
#                 ignored folder (never commit it, docs/legal.md)
#   PROJECT_NAME  default: rescue-arm9
#   --no-analysis map only, skip auto analysis (fast)
#   --check ADDR... print references to each ADDR after analysis (e.g.
#                 ov_0001+0x72688); must come last. A block/function report is always printed.
#
# Needs GHIDRA_INSTALL_DIR (the folder with ghidraRun) and a JDK 21 on PATH or JAVA_HOME.
set -euo pipefail

usage() { sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//'; exit 2; }
[ $# -ge 2 ] || usage
code_dir=$1
project_dir=$2
shift 2
project_name=rescue-arm9
if [ $# -gt 0 ] && [ "${1#--}" = "$1" ]; then
  project_name=$1
  shift
fi
analysis=()
check=()
while [ $# -gt 0 ]; do
  case $1 in
    --no-analysis) analysis=(-noanalysis); shift ;;
    --check) shift; check=("$@"); break ;;
    *) usage ;;
  esac
done

: "${GHIDRA_INSTALL_DIR:?set GHIDRA_INSTALL_DIR to your Ghidra install folder}"
headless=$GHIDRA_INSTALL_DIR/support/analyzeHeadless
[ -x "$headless" ] || { echo "error: $headless not found or not executable" >&2; exit 1; }
[ -f "$code_dir/manifest.json" ] || { echo "error: $code_dir/manifest.json not found; run tools/nds_extract_code.py first" >&2; exit 1; }

script_dir=$(cd "$(dirname "$0")" && pwd)
code_dir=$(cd "$code_dir" && pwd)
project_dir=$(realpath -m "$project_dir")

repo_root=$(cd "$script_dir/../.." && pwd)
case "$project_dir/" in
  "$repo_root"/*)
    if ! git -C "$repo_root" check-ignore -q "$project_dir/x.gpr" 2>/dev/null; then
      echo "error: $project_dir is inside the repository and not git-ignored (docs/legal.md)" >&2
      exit 1
    fi ;;
esac
mkdir -p "$project_dir"

log=$project_dir/$project_name-import.log
echo "Running analyzeHeadless; log: $log"
status=0
"$headless" "$project_dir" "$project_name" \
  -import "$code_dir/arm9.bin" -overwrite \
  -loader BinaryLoader -loader-baseAddr 0x02000000 \
  -processor ARM:LE:32:v5t -cspec default \
  -scriptPath "$script_dir" \
  -preScript RescueNdsImport.java "$code_dir/manifest.json" \
  -postScript RescueNdsReport.java ${check[@]+"${check[@]}"} \
  ${analysis[@]+"${analysis[@]}"} > "$log" 2>&1 || status=$?

grep -E 'RESCUE(-REPORT)?:' "$log" | sed -E 's/^.*(RESCUE(-REPORT)?:)/\1/; s/ \((GhidraScript|HeadlessAnalyzer)\) *$//' || true
# analyzeHeadless exits 0 even when a script fails, so check the log as well.
if [ "$status" -ne 0 ] || grep -q 'SCRIPT ERROR\|RESCUE: FAILED' "$log" || ! grep -q 'RESCUE: mapped' "$log"; then
  echo "error: import failed (exit $status); see $log" >&2
  exit 1
fi
echo "OK: project $project_dir/$project_name.gpr"
