#!/usr/bin/env bash
#
# candidate-gui-drive.sh — Phase 12 production-integration slice: drive the
# PACKAGED candidate Slint GUI (from the 9p share) through the
# sort → stable-identity → toggle workflow under Xvfb + AT-SPI, and dump
# every observable state transition (candidate-drive.py).
#
# Produces (into $1, default /mnt/host/out):
#   drive-seq.json       the full AT-SPI sequence (baseline, per-header sort
#                        tree, per-toggle tree, the identity proof)
#   candidate.stdout/stderr
#   candidate-trace.log  strace execve/execveat/openat
#   residual.json(.after)
#
set -euo pipefail

OUT="${1:-/mnt/host/out}"
mkdir -p "$OUT"
cd /tmp

# --- iterate without rebaking: prefer the share copy when present ---
if [ -f /mnt/host/scripts/candidate-gui-drive.sh ] && [ "$0" != "/mnt/host/scripts/candidate-gui-drive.sh" ]; then
    exec /mnt/host/scripts/candidate-gui-drive.sh "$OUT"
fi

# --- safety gate: only run inside an approved court VM (fail closed) ---
if [ ! -f /etc/cachyos-km/fixture.marker ]; then
    echo "REFUSING: /etc/cachyos-km/fixture.marker missing — not an approved court VM" >&2
    exit 3
fi

# --- the packaged candidate binary (host-built release, gui-alpm) ---
CAND=/mnt/host/gui/cachyos-kernel-manager
if [ ! -x "$CAND" ]; then
    echo "REFUSING: $CAND missing (stage the release binary into vm/images/share/gui/)" >&2
    exit 3
fi

# --- session bus + X (same as the oracle observer) ---
# the GPU-less winit-SOFTWARE renderer, EXPLICIT (never rely on backend
# selection): slint's default FemtoVG renderer requires OpenGL, which the
# headless court VM must not depend on.
export SLINT_BACKEND=winit-software
# at-spi2-core >= 2.52 removed the org.a11y.Status enablement dance, but
# accesskit_unix 0.22.1 (slint 1.17.1's a11y stack) still gates registration
# on IsEnabled flipping true. The stub drives the launcher's readwrite
# property through false->true transitions so the app's accesskit completes
# its enablement (verified: without it the candidate NEVER appears on the
# AT-SPI registry; with it, it registers within ~3s).
eval "$(dbus-launch --sh-syntax)"
export DBUS_SESSION_BUS_ADDRESS
for _ in 1 2 3; do
    if gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.a11y.Bus.GetAddress >/dev/null 2>&1; then
        break
    fi
    sleep 1
done
python3 /mnt/host/scripts/a11y-status-stub.py >/tmp/a11y-stub.log 2>&1 &
STUB_PID=$!
export DISPLAY=:99
Xvfb :99 -screen 0 1280x800x24 >/tmp/xvfb.log 2>&1 &
XVFB_PID=$!
trap 'kill $XVFB_PID 2>/dev/null || true; kill $STUB_PID 2>/dev/null || true; pkill -f cachyos-kernel-manager 2>/dev/null || true' EXIT
sleep 1

# --- machine residual BEFORE ---
RESIDUAL_SH="/opt/cachyos-km-vm/residual.sh"
[ -f /mnt/host/scripts/residual.sh ] && RESIDUAL_SH=/mnt/host/scripts/residual.sh
"$RESIDUAL_SH" > "$OUT/residual.json" || true

# --- drive the sort + stable-identity + toggle sequence ---
# candidate mode: the driver RELAUNCHES the app once per header and
# witnesses the sorted order + toggled identity from the app's own
# KM_VERBOSE semantic trace (the accesskit 0.22.1 bridge cannot survive
# tree rebuilds in the court VMs — the ACTIONS reach the app, the tree does
# not persist — so the app's own courted trace is the authority).
DRIVE_PY="/opt/cachyos-km-vm/candidate-drive.py"
[ -f /mnt/host/scripts/candidate-drive.py ] && DRIVE_PY=/mnt/host/scripts/candidate-drive.py
DRIVE_RC=0
PYTHONPATH=/opt/a11y/pyatspi2 python3 "$DRIVE_PY" --candidate "$CAND" "$OUT" || DRIVE_RC=$?

"$RESIDUAL_SH" > "$OUT/residual.json.after" || true

echo "candidate GUI drive complete (drive_rc=$DRIVE_RC)"
exit "$DRIVE_RC"
