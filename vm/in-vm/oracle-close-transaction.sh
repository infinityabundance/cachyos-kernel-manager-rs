#!/usr/bin/env bash
#
# oracle-close-transaction.sh — gap-010 close-during-transaction court
# (oracle side): drive the frozen Qt GUI through a REAL transaction
# (toggle a kernel row + Execute), wait for the in-flight terminal (the
# slow-pacman fixture keeps it up 15s), close the MAIN window
# (WM_DELETE_WINDOW), and record the process exit outcome.
#
# The documented oracle behavior (witnessed 2026-08-23): the app ABORTS —
# closeEvent (km-window.cpp:327-338) releases the alpm handle and the app
# exits while the worker QThread is still blocked in the transaction
# (Qt: "QThread: Destroyed while thread is still running" -> SIGABRT).
#
set -euo pipefail

OUT="${1:-/mnt/host/out}"
mkdir -p "$OUT"
cd /tmp

if [ -f /mnt/host/scripts/oracle-close-transaction.sh ] && [ "$0" != "/mnt/host/scripts/oracle-close-transaction.sh" ]; then
    exec /mnt/host/scripts/oracle-close-transaction.sh "$OUT"
fi

if [ ! -f /etc/cachyos-km/fixture.marker ]; then
    echo "REFUSING: /etc/cachyos-km/fixture.marker missing — not an approved court VM" >&2
    exit 3
fi

eval "$(dbus-launch --sh-syntax)"
export DBUS_SESSION_BUS_ADDRESS
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1 DISPLAY=:99
Xvfb :99 -screen 0 1280x800x24 >/tmp/xvfb.log 2>&1 &
XVFB_PID=$!
trap 'kill $XVFB_PID 2>/dev/null || true; kill ${APP_PID:-0} 2>/dev/null || true; pkill -9 -f "cachyos-kernel-manag[e]r" 2>/dev/null || true; pkill -9 xterm 2>/dev/null || true' EXIT
sleep 1

RESIDUAL_SH="/opt/cachyos-km-vm/residual.sh"
[ -f /mnt/host/scripts/residual.sh ] && RESIDUAL_SH=/mnt/host/scripts/residual.sh
"$RESIDUAL_SH" > "$OUT/residual.json" || true

/usr/bin/cachyos-kernel-manager >/tmp/oracle.stdout 2>/tmp/oracle.stderr &
APP_PID=$!
sleep 6

# toggle the first installable row + click Execute (the transaction starts)
DRIVE_PY="/opt/cachyos-km-vm/oracle-drive.py"
[ -f /mnt/host/scripts/oracle-drive.py ] && DRIVE_PY=/mnt/host/scripts/oracle-drive.py
PYTHONPATH=/opt/a11y/pyatspi2 python3 "$DRIVE_PY" /tmp/state.json "extra/linux-zen" >/tmp/drive.log 2>&1 || true
echo "drive rc=$? (toggle + execute)"

# wait for the slow-pacman in-flight window (the fixture keeps it 15s)
INFLIGHT=0
for _ in $(seq 30); do
    if grep -q "slow-pacman" /tmp/oracle.stdout 2>/dev/null || pgrep -x xterm >/dev/null 2>&1; then
        INFLIGHT=1
        echo "transaction in-flight"
        break
    fi
    sleep 1
done

if [ "$INFLIGHT" = "1" ]; then
    # close the MAIN window mid-transaction (exact title match)
    python3 /mnt/host/scripts/xclose.py "CachyOS Kernel Manager" >/tmp/xclose.log 2>&1 || true
    cat /tmp/xclose.log
fi
sleep 5

OUTCOME="unknown"
RC=""
if kill -0 "$APP_PID" 2>/dev/null; then
    OUTCOME="still-alive"
    kill -TERM "$APP_PID" 2>/dev/null || true
    sleep 2
    kill -KILL "$APP_PID" 2>/dev/null || true
else
    set +e
    wait "$APP_PID"
    RC=$?
    set -e
    if [ "$RC" -ge 128 ]; then
        OUTCOME="crash"
    else
        OUTCOME="clean"
    fi
fi
python3 - "$OUTCOME" "$RC" "$INFLIGHT" <<'PY' > "$OUT/close-outcome.json"
import json, sys
json.dump({"exit_outcome": sys.argv[1], "exit_rc": sys.argv[2], "transaction_inflight": sys.argv[3] == "1"}, sys.stdout, indent=1)
PY
echo "exit_outcome=$OUTCOME rc=$RC inflight=$INFLIGHT"

"$RESIDUAL_SH" > "$OUT/residual.json.after" || true
cp /tmp/oracle.stderr "$OUT/oracle.stderr" 2>/dev/null || true
echo "oracle close-transaction witness complete"
exit 0
