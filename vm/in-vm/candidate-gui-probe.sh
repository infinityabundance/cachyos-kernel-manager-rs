#!/usr/bin/env bash
#
# candidate-gui-probe.sh — Phase 12 feasibility probe: run the PACKAGED
# candidate Slint GUI (from the 9p share) under Xvfb + AT-SPI exactly like
# the oracle side, and dump its accessibility tree. This answers whether the
# candidate exposes a drivable AT-SPI surface (slint's accesskit_unix bridge)
# with the same roles the oracle driver expects (tree rows, checkboxes,
# column headers).
#
# Produces (into $1, default /mnt/host/out):
#   candidate-state.json  full AT-SPI tree (reuses oracle-observe.py)
#   candidate.stdout/stderr
#   candidate-trace.log   strace execve/execveat/openat
#   residual.json(.after)
#
set -euo pipefail

OUT="${1:-/mnt/host/out}"
mkdir -p "$OUT"
cd /tmp

# --- iterate without rebaking: prefer the share copy when present ---
if [ -f /mnt/host/scripts/candidate-gui-probe.sh ] && [ "$0" != "/mnt/host/scripts/candidate-gui-probe.sh" ]; then
    exec /mnt/host/scripts/candidate-gui-probe.sh "$OUT"
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
eval "$(dbus-launch --sh-syntax)"
export DBUS_SESSION_BUS_ADDRESS
for _ in 1 2 3; do
    if gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.a11y.Bus.GetAddress >/dev/null 2>&1; then
        break
    fi
    sleep 1
done
export DISPLAY=:99
Xvfb :99 -screen 0 1280x800x24 >/tmp/xvfb.log 2>&1 &
XVFB_PID=$!
trap 'kill $XVFB_PID 2>/dev/null || true; kill $CAND_PID 2>/dev/null || true; pkill -f cachyos-kernel-manager 2>/dev/null || true' EXIT
sleep 1

# --- machine residual BEFORE ---
RESIDUAL_SH="/opt/cachyos-km-vm/residual.sh"
[ -f /mnt/host/scripts/residual.sh ] && RESIDUAL_SH=/mnt/host/scripts/residual.sh
"$RESIDUAL_SH" > "$OUT/residual.json" || true

# --- launch the candidate under strace ---
rm -f /tmp/candidate.stdout /tmp/candidate.stderr /tmp/candidate-trace.log
strace -f -s 256 -o /tmp/candidate-trace.log \
    -e trace=execve,execveat,openat,access,faccessat,connect \
    "$CAND" \
    >/tmp/candidate.stdout 2>/tmp/candidate.stderr &
CAND_PID=$!

# --- observe via AT-SPI (the oracle's tree dumper: finds any app whose name
# contains cachyos/kernel) ---
OBSERVE_PY="/opt/cachyos-km-vm/oracle-observe.py"
[ -f /mnt/host/scripts/oracle-observe.py ] && OBSERVE_PY=/mnt/host/scripts/oracle-observe.py
OBSERVE_RC=0
PYTHONPATH=/opt/a11y/pyatspi2 python3 "$OBSERVE_PY" /tmp/candidate-state.json || OBSERVE_RC=$?

sleep 1
kill -TERM "$CAND_PID" 2>/dev/null || true
sleep 1
kill -KILL "$CAND_PID" 2>/dev/null || true

cp /tmp/candidate-state.json "$OUT/candidate-state.json"
cp /tmp/candidate.stdout "$OUT/candidate.stdout"
cp /tmp/candidate.stderr "$OUT/candidate.stderr"
cp /tmp/candidate-trace.log "$OUT/candidate-trace.log"

"$RESIDUAL_SH" > "$OUT/residual.json.after" || true

echo "candidate GUI probe complete (observe_rc=$OBSERVE_RC)"
exit "$OBSERVE_RC"
