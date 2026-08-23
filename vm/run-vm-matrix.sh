#!/usr/bin/env bash
#
# run-vm-matrix.sh — Phase 12 fail-CLOSED full VM matrix.
#
# Runs EVERY VM-capable court and aggregates the failures: a court failure
# does NOT abort the loop (every court must be witnessed), but any failure
# makes the final exit code nonzero — a forensic system must fail closed.
#
set -uo pipefail

cd "$(dirname "$0")/.."
LOG="${1:-/tmp/km-vm-matrix.log}"
: > "$LOG"

courts=$(cargo run -q -p xtask -- court list --vm-capable 2>/dev/null | grep -v "courts VM-capable")
failed=0
ran=0
for c in $courts; do
    ran=$((ran + 1))
    echo "===== [$ran] $c =====" >> "$LOG"
    if ! cargo run -q -p xtask -- court run "$c" --vm >> "$LOG" 2>&1; then
        echo "COURT FAILED: $c" >> "$LOG"
        failed=1
    fi
    echo "===== done $c =====" >> "$LOG"
done

echo "" >> "$LOG"
echo "matrix complete: ran $ran courts, failed=$failed" >> "$LOG"
exit "$failed"
