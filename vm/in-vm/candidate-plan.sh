#!/usr/bin/env bash
#
# candidate-plan.sh — run the CANDIDATE plan tool in the court VM and
# capture its modeled view of the SAME transaction the oracle just executed.
#
# Produces (into $1, default /mnt/host/out):
#   candidate-state.json          discovery rows (inspect dump)
#   candidate-transaction.json    probe/exec/terminal modeled chains (plan)
#   candidate.stderr              plan tool stderr
#   residual.json                 machine residual (must equal the oracle's)
#
# Usage: candidate-plan.sh <out-dir> <raw> [<raw> ...]
#
set -euo pipefail

OUT="${1:-/mnt/host/out}"
shift || true
TARGETS=("$@")

mkdir -p "$OUT"

# --- iterate without rebaking: prefer the share copy when present ---
if [ -f /mnt/host/scripts/candidate-plan.sh ] && [ "$0" != "/mnt/host/scripts/candidate-plan.sh" ]; then
    exec /mnt/host/scripts/candidate-plan.sh "$OUT" "${TARGETS[@]}"
fi

# --- safety gate ---
if [ ! -f /etc/cachyos-km/fixture.marker ]; then
    echo "REFUSING: not an approved court VM" >&2
    exit 3
fi

INSPECT=/opt/cachyos-km-vm/cachyos-kernel-manager-inspect
[ -x "$INSPECT" ] || INSPECT=/mnt/host/inspect/cachyos-kernel-manager-inspect
if [ ! -x "$INSPECT" ]; then
    echo "FATAL: candidate inspect tool not found" >&2
    exit 4
fi
PLAN=/opt/cachyos-km-vm/cachyos-kernel-manager-plan
[ -x "$PLAN" ] || PLAN=/mnt/host/inspect/cachyos-kernel-manager-plan
if [ ! -x "$PLAN" ]; then
    echo "FATAL: candidate plan tool not found" >&2
    exit 4
fi

# machine residual: must be IDENTICAL to the oracle run's (same fixture,
# fresh overlay) — the comparator treats drift as a fixture-integrity
# violation, not a parity pass.
RESIDUAL_SH="/opt/cachyos-km-vm/residual.sh"
[ -f /mnt/host/scripts/residual.sh ] && RESIDUAL_SH=/mnt/host/scripts/residual.sh
"$RESIDUAL_SH" > "$OUT/residual.json" || true

"$INSPECT" dump --format json > "$OUT/candidate-state.json" 2> "$OUT/candidate.stderr"

if [ "${#TARGETS[@]}" -gt 0 ]; then
    SELECT_ARGS=()
    for t in "${TARGETS[@]}"; do
        SELECT_ARGS+=(--select "$t")
    done
    "$PLAN" "${SELECT_ARGS[@]}" > "$OUT/candidate-transaction.json" 2>> "$OUT/candidate.stderr"
else
    echo "{}" > "$OUT/candidate-transaction.json"
fi
echo "candidate plan observation complete"
