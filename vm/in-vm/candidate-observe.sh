#!/usr/bin/env bash
#
# candidate-observe.sh — run the CANDIDATE inspection tool in the VM and
# capture its view of the same package state the oracle just observed.
#
# The candidate tool (cachyos-kernel-manager-inspect) is built from this
# workspace and reads the SAME libalpm databases the oracle reads, through
# the same pacman.conf registration rule. Its output is the second half of
# the differential: oracle (real GUI + libalpm) vs candidate (Rust model +
# libalpm) against byte-identical machine state.
#
set -euo pipefail

OUT="${1:-/mnt/host/out}"
mkdir -p "$OUT"

# --- iterate without rebaking: prefer the share copy when present ---
if [ -f /mnt/host/scripts/candidate-observe.sh ] && [ "$0" != "/mnt/host/scripts/candidate-observe.sh" ]; then
    exec /mnt/host/scripts/candidate-observe.sh "$OUT"
fi

# --- safety gate ---
if [ ! -f /etc/cachyos-km/fixture.marker ]; then
    echo "REFUSING: not an approved court VM" >&2
    exit 3
fi

INSPECT=/opt/cachyos-km-vm/cachyos-kernel-manager-inspect
if [ ! -x "$INSPECT" ]; then
    # fall back to the host share (development iteration without rebaking)
    if [ -x /mnt/host/inspect/cachyos-kernel-manager-inspect ]; then
        INSPECT=/mnt/host/inspect/cachyos-kernel-manager-inspect
    else
        echo "FATAL: candidate inspect tool not found" >&2
        exit 4
    fi
fi

# machine residual: must be IDENTICAL to the oracle run's (same fixture,
# fresh overlay) — the comparator treats drift as a fixture-integrity
# violation, not a parity pass.
RESIDUAL_SH="/opt/cachyos-km-vm/residual.sh"
[ -f /mnt/host/scripts/residual.sh ] && RESIDUAL_SH=/mnt/host/scripts/residual.sh
"$RESIDUAL_SH" > "$OUT/residual.json" || true

"$INSPECT" dump --format json > "$OUT/candidate-state.json" 2> "$OUT/candidate.stderr"
echo "candidate observation complete"
