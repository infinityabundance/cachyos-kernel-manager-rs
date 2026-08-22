#!/usr/bin/env bash
#
# candidate-gitcache.sh — run the CANDIDATE git-cache model in the court VM
# and capture its modeled view of the SAME Configure flow the oracle just
# executed.
#
# Produces (into $1, default /mnt/host/out):
#   candidate-state.json          discovery rows (inspect dump)
#   candidate-transaction.json    modeled git refresh exec chain
#   candidate.stderr              model tool stderr
#   residual.json                 machine residual (must equal the oracle's)
#
# Usage: candidate-gitcache.sh <out-dir>
#
set -euo pipefail

OUT="${1:-/mnt/host/out}"

mkdir -p "$OUT"

# --- iterate without rebaking: prefer the share copy when present ---
if [ -f /mnt/host/scripts/candidate-gitcache.sh ] && [ "$0" != "/mnt/host/scripts/candidate-gitcache.sh" ]; then
    exec /mnt/host/scripts/candidate-gitcache.sh "$OUT"
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
GITCACHE=/opt/cachyos-km-vm/cachyos-kernel-manager-gitcache
[ -x "$GITCACHE" ] || GITCACHE=/mnt/host/inspect/cachyos-kernel-manager-gitcache
if [ ! -x "$GITCACHE" ]; then
    echo "FATAL: candidate git-cache model tool not found" >&2
    exit 4
fi

# machine residual: must be IDENTICAL to the oracle run's (same fixture,
# fresh overlay) — the comparator treats drift as a fixture-integrity
# violation, not a parity pass.
RESIDUAL_SH="/opt/cachyos-km-vm/residual.sh"
[ -f /mnt/host/scripts/residual.sh ] && RESIDUAL_SH=/mnt/host/scripts/residual.sh
"$RESIDUAL_SH" > "$OUT/residual.json" || true

"$INSPECT" dump --format json > "$OUT/candidate-state.json" 2> "$OUT/candidate.stderr"

# The oracle runs as root (HOME=/root): model the SAME cache paths
# prepare_build_environment uses (utils.cpp:198-202). The model probes the
# live fixture filesystem — the identical machine state the oracle saw.
"$GITCACHE" /root/.cache/cachyos-km /root/.cache/cachyos-km/pkgbuilds \
    "https://github.com/cachyos/linux-cachyos.git" \
    > "$OUT/candidate-transaction.json" 2>> "$OUT/candidate.stderr"
echo "candidate git-cache observation complete"
