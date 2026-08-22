#!/usr/bin/env bash
#
# scx-loader-candidate.sh — the CANDIDATE side of the scx/loader-interface
# VM court: run the candidate's typed client (scx-state, built with the
# dbus feature) against the SAME real org.scx.Loader bus, and render the
# candidate's interface descriptor. The comparator proves (a) every
# candidate interface element exists on the real loader with the same
# signature (the frozen surface is a faithful subset of the shipped loader),
# and (b) the candidate reads the same property values the loader reports.
#
set -euo pipefail

OUT="${1:-/mnt/host/out}"
mkdir -p "$OUT"

if [ -f /mnt/host/scripts/scx-loader-candidate.sh ] && [ "$0" != "/mnt/host/scripts/scx-loader-candidate.sh" ]; then
    exec /mnt/host/scripts/scx-loader-candidate.sh "$OUT"
fi

if [ ! -f /etc/cachyos-km/fixture.marker ]; then
    echo "REFUSING: not an approved court VM" >&2
    exit 3
fi

STATE_BIN=/opt/cachyos-km-vm/cachyos-kernel-manager-scx-state
if [ ! -x "$STATE_BIN" ]; then
    if [ -x /mnt/host/inspect/cachyos-kernel-manager-scx-state ]; then
        STATE_BIN=/mnt/host/inspect/cachyos-kernel-manager-scx-state
    else
        echo "FATAL: candidate scx-state tool not found" >&2
        exit 4
    fi
fi
INTROSPECT_BIN=/mnt/host/inspect/cachyos-kernel-manager-scx-introspect
if [ ! -x "$INTROSPECT_BIN" ]; then
    echo "FATAL: candidate scx-introspect tool not found" >&2
    exit 4
fi

# the loader must be on the bus (the oracle side started it; be idempotent)
for i in $(seq 1 10); do
    if busctl list 2>/dev/null | grep -q 'org.scx.Loader'; then
        break
    fi
    systemctl start scx_loader 2>/dev/null || true
    sleep 1
done

"$INTROSPECT_BIN" > "$OUT/candidate-interface.json" 2> "$OUT/candidate.stderr"
"$STATE_BIN" > "$OUT/candidate-readback.json" 2>> "$OUT/candidate.stderr" || {
    echo "FATAL: candidate scx-state readback failed (see candidate.stderr)" >&2
    exit 5
}

echo "scx loader candidate observation complete"
