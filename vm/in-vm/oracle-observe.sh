#!/usr/bin/env bash
#
# oracle-observe.sh — run the real oracle GUI under Xvfb with full tracing,
# capture its accessibility state + every external command it executes.
#
# Produces (into $1, default /mnt/host/out):
#   oracle-state.json   full AT-SPI tree (see oracle-observe.py)
#   oracle.stdout       oracle's stdout
#   oracle.stderr       oracle's stderr
#   oracle-trace.log    strace execve/execveat/openat of the oracle process
#   residual.json       machine residual before/after the run
#
set -euo pipefail

OUT="${1:-/mnt/host/out}"
mkdir -p "$OUT"
cd /tmp

# --- iterate without rebaking: prefer the share copy when present ---
if [ -f /mnt/host/scripts/oracle-observe.sh ] && [ "$0" != "/mnt/host/scripts/oracle-observe.sh" ]; then
    exec /mnt/host/scripts/oracle-observe.sh "$OUT"
fi

# --- safety gate: only run inside an approved court VM (fail closed) ---
if [ ! -f /etc/cachyos-km/fixture.marker ]; then
    echo "REFUSING: /etc/cachyos-km/fixture.marker missing — not an approved court VM" >&2
    exit 3
fi

# --- session bus + X ---
eval "$(dbus-launch --sh-syntax)"
export DBUS_SESSION_BUS_ADDRESS
# pre-activate the AT-SPI bus BEFORE the oracle starts: Qt's accessibility
# bridge registers the application at startup and never retries if the
# registry is not there yet
for _ in 1 2 3; do
    if gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.a11y.Bus.GetAddress >/dev/null 2>&1; then
        break
    fi
    sleep 1
done
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
export QT_ACCESSIBILITY=1
export DISPLAY=:99
Xvfb :99 -screen 0 1280x800x24 >/tmp/xvfb.log 2>&1 &
XVFB_PID=$!
trap 'kill $XVFB_PID 2>/dev/null || true; kill $ORACLE_PID 2>/dev/null || true; pkill -f "cachyos-kernel-manager" 2>/dev/null || true' EXIT
sleep 1

# --- machine residual BEFORE (prefer the share copy for iteration) ---
RESIDUAL_SH="/opt/cachyos-km-vm/residual.sh"
[ -f /mnt/host/scripts/residual.sh ] && RESIDUAL_SH=/mnt/host/scripts/residual.sh
"$RESIDUAL_SH" > "$OUT/residual.json" || true

# --- launch the oracle under strace (probe-command archaeology) ---
rm -f /tmp/oracle.stdout /tmp/oracle.stderr /tmp/oracle-trace.log
strace -f -o /tmp/oracle-trace.log \
    -e trace=execve,execveat,openat,access,faccessat,connect \
    /usr/bin/cachyos-kernel-manager \
    >/tmp/oracle.stdout 2>/tmp/oracle.stderr &
ORACLE_PID=$!

# --- observe via AT-SPI (vendored pyatspi2 over gi.repository.Atspi) ---
OBSERVE_PY="/opt/cachyos-km-vm/oracle-observe.py"
[ -f /mnt/host/scripts/oracle-observe.py ] && OBSERVE_PY=/mnt/host/scripts/oracle-observe.py
PYTHONPATH=/opt/a11y/pyatspi2 python3 "$OBSERVE_PY" /tmp/oracle-state.json || OBSERVE_RC=$?
OBSERVE_RC="${OBSERVE_RC:-0}"

# give the app a moment to settle, then close it (SIGTERM -> clean closeEvent)
sleep 1
kill -TERM "$ORACLE_PID" 2>/dev/null || true
sleep 1
kill -KILL "$ORACLE_PID" 2>/dev/null || true

cp /tmp/oracle-state.json "$OUT/oracle-state.json"
cp /tmp/oracle.stdout "$OUT/oracle.stdout"
cp /tmp/oracle.stderr "$OUT/oracle.stderr"
cp /tmp/oracle-trace.log "$OUT/oracle-trace.log"

# --- machine residual AFTER ---
"$RESIDUAL_SH" > "$OUT/residual.json.after" || true

echo "oracle observation complete (observe_rc=$OBSERVE_RC)"
exit "$OBSERVE_RC"
