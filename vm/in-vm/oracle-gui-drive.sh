#!/usr/bin/env bash
#
# oracle-gui-drive.sh — Phase 12 production-integration slice: drive the
# packaged ORACLE Qt GUI through the SAME sort → stable-identity → toggle
# sequence as the candidate (candidate-drive.py is side-agnostic: it drives
# whatever AT-SPI tree the app exposes). The Qt header click falls back to
# XTEST (Qt exposes no usable a11y actions on this stack); the checkbox
# toggle uses the same path.
#
# Produces (into $1, default /mnt/host/out):
#   drive-seq.json       the full AT-SPI sequence
#   oracle.stdout/stderr
#   oracle-trace.log     strace execve/execveat/openat
#   residual.json(.after)
#
set -euo pipefail

OUT="${1:-/mnt/host/out}"
mkdir -p "$OUT"
cd /tmp

if [ -f /mnt/host/scripts/oracle-gui-drive.sh ] && [ "$0" != "/mnt/host/scripts/oracle-gui-drive.sh" ]; then
    exec /mnt/host/scripts/oracle-gui-drive.sh "$OUT"
fi

if [ ! -f /etc/cachyos-km/fixture.marker ]; then
    echo "REFUSING: /etc/cachyos-km/fixture.marker missing — not an approved court VM" >&2
    exit 3
fi

# the frozen oracle binary (installed by the fixture)
ORACLE=/usr/bin/cachyos-kernel-manager
if [ ! -x "$ORACLE" ]; then
    echo "REFUSING: $ORACLE missing" >&2
    exit 3
fi

export DISPLAY=:99
eval "$(dbus-launch --sh-syntax)"
export DBUS_SESSION_BUS_ADDRESS
# Qt registers its AT-SPI bridge only with accessibility forced on
# (oracle-observe.sh parity — without these the app never appears on the
# registry and the driver finds nothing)
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
export QT_ACCESSIBILITY=1
for _ in 1 2 3; do
    if gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.a11y.Bus.GetAddress >/dev/null 2>&1; then
        break
    fi
    sleep 1
done
Xvfb :99 -screen 0 1280x800x24 >/tmp/xvfb.log 2>&1 &
XVFB_PID=$!
trap 'kill $XVFB_PID 2>/dev/null || true; kill $ORACLE_PID 2>/dev/null || true; pkill -f cachyos-kernel-manager 2>/dev/null || true' EXIT
sleep 1

RESIDUAL_SH="/opt/cachyos-km-vm/residual.sh"
[ -f /mnt/host/scripts/residual.sh ] && RESIDUAL_SH=/mnt/host/scripts/residual.sh
"$RESIDUAL_SH" > "$OUT/residual.json" || true

rm -f /tmp/oracle.stdout /tmp/oracle.stderr /tmp/oracle-trace.log
strace -f -s 256 -o /tmp/oracle-trace.log \
    -e trace=execve,execveat,openat,access,faccessat,connect \
    "$ORACLE" \
    >/tmp/oracle.stdout 2>/tmp/oracle.stderr &
ORACLE_PID=$!

DRIVE_PY="/opt/cachyos-km-vm/candidate-drive.py"
[ -f /mnt/host/scripts/candidate-drive.py ] && DRIVE_PY=/mnt/host/scripts/candidate-drive.py
DRIVE_RC=0
PYTHONPATH=/opt/a11y/pyatspi2 python3 "$DRIVE_PY" "$OUT" || DRIVE_RC=$?

sleep 1
kill -TERM "$ORACLE_PID" 2>/dev/null || true
sleep 1
kill -KILL "$ORACLE_PID" 2>/dev/null || true

cp /tmp/oracle.stdout "$OUT/oracle.stdout"
cp /tmp/oracle.stderr "$OUT/oracle.stderr"
cp /tmp/oracle-trace.log "$OUT/oracle-trace.log"

"$RESIDUAL_SH" > "$OUT/residual.json.after" || true

echo "oracle GUI drive complete (drive_rc=$DRIVE_RC)"
exit "$DRIVE_RC"
