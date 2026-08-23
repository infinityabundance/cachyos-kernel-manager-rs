#!/usr/bin/env bash
#
# candidate-close-transaction.sh — gap-010 close-during-transaction court
# (candidate side): drive the release Slint binary through a REAL
# transaction (toggle a kernel row + Execute), wait for the in-flight
# terminal (the slow-pacman fixture keeps it up 15s), close the MAIN
# window (WM_DELETE_WINDOW), and record the process exit outcome.
#
# The documented candidate behavior (D-008 INTENTIONAL_CORRECTION,
# witnessed 2026-08-23): CLEAN exit — the close exits the event loop and
# the transaction task is a runtime-owned detached thread; there is no
# QThread-destroyed abort.
#
set -euo pipefail

OUT="${1:-/mnt/host/out}"
mkdir -p "$OUT"
cd /tmp

if [ -f /mnt/host/scripts/candidate-close-transaction.sh ] && [ "$0" != "/mnt/host/scripts/candidate-close-transaction.sh" ]; then
    exec /mnt/host/scripts/candidate-close-transaction.sh "$OUT"
fi

if [ ! -f /etc/cachyos-km/fixture.marker ]; then
    echo "REFUSING: /etc/cachyos-km/fixture.marker missing — not an approved court VM" >&2
    exit 3
fi

CAND=/mnt/host/gui/cachyos-kernel-manager
if [ ! -x "$CAND" ]; then
    echo "REFUSING: $CAND missing (stage the release binary into vm/images/share/gui/)" >&2
    exit 3
fi

eval "$(dbus-launch --sh-syntax)"
export DBUS_SESSION_BUS_ADDRESS
export SLINT_BACKEND=winit-software DISPLAY=:99
# the accesskit_unix 0.22.1 registration gate (org.a11y.Status.IsEnabled)
python3 /mnt/host/scripts/a11y-status-stub.py >/tmp/a11y-stub.log 2>&1 &
STUB_PID=$!
Xvfb :99 -screen 0 1280x800x24 >/tmp/xvfb.log 2>&1 &
XVFB_PID=$!
trap 'kill $XVFB_PID 2>/dev/null || true; kill $STUB_PID 2>/dev/null || true; kill ${APP_PID:-0} 2>/dev/null || true; pkill -9 -f "cachyos-kernel-manag[e]r" 2>/dev/null || true; pkill -9 -f "terminal-helpe[r]" 2>/dev/null || true; pkill -9 xterm 2>/dev/null || true' EXIT
sleep 1

RESIDUAL_SH="/opt/cachyos-km-vm/residual.sh"
[ -f /mnt/host/scripts/residual.sh ] && RESIDUAL_SH=/mnt/host/scripts/residual.sh
"$RESIDUAL_SH" > "$OUT/residual.json" || true

"$CAND" >/tmp/cand.stdout 2>/tmp/cand.stderr &
APP_PID=$!
sleep 6

# toggle the first installable row + click Execute (the transaction starts)
# — the target must be a VISIBLE row: the ScrollView only exposes the rows
# in view to the accesskit bridge (the last row is below the fold)
PYTHONPATH=/opt/a11y/pyatspi2 python3 /mnt/host/scripts/candidate-transact.py "cachyos/linux-cachyos-bmq" >/tmp/drive.log 2>&1
DRIVE_RC=$?
echo "drive rc=$DRIVE_RC (toggle + execute)"
if [ "$DRIVE_RC" != "0" ]; then
    echo "drive failed — no transaction will start; the close witnesses nothing"
fi

# wait for the slow-pacman in-flight window (the fixture keeps it 15s)
INFLIGHT=0
for _ in $(seq 30); do
    if grep -q "slow-pacman" /tmp/cand.stderr 2>/dev/null || pgrep -f "terminal-helper" >/dev/null 2>&1; then
        INFLIGHT=1
        echo "transaction in-flight"
        break
    fi
    sleep 1
done

if [ "$INFLIGHT" = "1" ]; then
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
cp /tmp/cand.stderr "$OUT/candidate.stderr" 2>/dev/null || true
echo "candidate close-transaction witness complete"
exit 0
