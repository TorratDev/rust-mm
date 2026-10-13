#!/usr/bin/env bash
# Phase 0 check V7: scripted DeSmuME dumps are repeatable.
#
# Builds the synthetic test ROM, saves a savestate at frame 30, then plays a
# fixed input sequence from that state three times and compares the dumps:
#   - 16-byte state block on an exec trigger (stand-in for a per-turn hook)
#   - all 4 MiB of main RAM at the end of every frame
# A different input sequence must give a different dump (negative control).
#
# Needs: python3 with py-desmume (pip install py-desmume).
# Usage: verification/v7-trace/run_v7.sh [WORKDIR] [PYTHON]
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
work="${1:-$(mktemp -d)}"
py="${2:-python3}"
export SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy
mkdir -p "$work"
cd "$work"

dump() { "$py" "$here/pydesmume_dump.py" "$@" 2>&1 | grep -E '^[0-9]+ frames' || true; }

"$py" "$here/make_test_rom.py" test.nds
"$py" "$here/pydesmume_dump.py" test.nds --make-state s.dst --frames 30 >/dev/null 2>&1

echo "== baseline (no dump)"
dump test.nds none.bin --frames 600 --addr 0x02100000 --len 0x0 --state s.dst

echo "== state block on exec trigger, 600 frames, x3"
for i in 1 2 3; do
  dump test.nds small$i.bin --frames 600 --addr 0x02100000 --len 0x10 \
    --trigger 0x02000044 --state s.dst --inputs "$here/inputs.txt"
done

echo "== full main RAM every frame, 60 frames, x3"
for i in 1 2 3; do
  dump test.nds full$i.bin --frames 60 --addr 0x02000000 --len 0x400000 \
    --state s.dst --inputs "$here/inputs.txt"
  sha256sum full$i.bin | cut -d' ' -f1 > full$i.sha256
  rm full$i.bin
done

echo "== negative control: different inputs"
printf '0 -\n10 B\n' > other.txt
dump test.nds other.bin --frames 600 --addr 0x02100000 --len 0x10 \
  --trigger 0x02000044 --state s.dst --inputs other.txt

same() { [ "$(sort -u "$@" | wc -l)" -eq 1 ]; }
sha256sum small?.bin | cut -d' ' -f1 > small.sha256
fail=0
same small.sha256 && echo "PASS state-block dumps identical" || { echo "FAIL state-block dumps differ"; fail=1; }
same full?.sha256 && echo "PASS full-RAM dumps identical" || { echo "FAIL full-RAM dumps differ"; fail=1; }
if [ "$(sha256sum < other.bin)" != "$(sha256sum < small1.bin)" ]; then
  echo "PASS different inputs give a different dump"
else
  echo "FAIL inputs had no effect"; fail=1
fi
exit $fail
