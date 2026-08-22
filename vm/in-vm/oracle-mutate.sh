#!/usr/bin/env bash
#
# oracle-mutate.sh — run the real oracle GUI under Xvfb with execve tracing,
# drive the REAL Configure window through AT-SPI (custom name + remote patch
# + Build kernel), and capture the PKGBUILD before/after the on_execute
# mutations (conf-window.cpp:716-729).
#
# The oracle is launched with cwd = /root/.cache/cachyos-km/pkgbuilds so the
# oracle's relative `linux-cachyos/PKGBUILD` paths resolve to the seeded
# checkout (D-004: the oracle's build logic assumes the pkgbuilds cache is
# the cwd).
#
# Usage: oracle-mutate.sh <out-dir> [--custom-name <n>] [--patch-url <u>]
#
# Produces (into $1, default /mnt/host/out):
#   oracle-state.json          full AT-SPI tree of the SAME run (rows)
#   pkgbuild-before.txt        PKGBUILD text at Build click time
#   pkgbuild-after.txt         PKGBUILD text after the Build click
#   oracle-trace.log           the raw strace log (immutable evidence)
#   oracle.stdout/stderr       oracle's own output
#   residual.json(.after)      machine residual before/after
#
set -euo pipefail

OUT="${1:-/mnt/host/out}"
shift || true
ACTIONS=("$@")

mkdir -p "$OUT"
# the oracle's cwd: the pkgbuilds cache (D-004 design assumption)
cd /root/.cache/cachyos-km/pkgbuilds

# --- iterate without rebaking: prefer the share copy when present ---
if [ -f /mnt/host/scripts/oracle-mutate.sh ] && [ "$0" != "/mnt/host/scripts/oracle-mutate.sh" ]; then
    exec /mnt/host/scripts/oracle-mutate.sh "$OUT" "${ACTIONS[@]}"
fi

# --- safety gate: only run inside an approved court VM (fail closed) ---
if [ ! -f /etc/cachyos-km/fixture.marker ]; then
    echo "REFUSING: /etc/cachyos-km/fixture.marker missing — not an approved court VM" >&2
    exit 3
fi

# --- session bus + X ---
eval "$(dbus-launch --sh-syntax)"
export DBUS_SESSION_BUS_ADDRESS
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

# --- machine residual BEFORE ---
RESIDUAL_SH="/opt/cachyos-km-vm/residual.sh"
[ -f /mnt/host/scripts/residual.sh ] && RESIDUAL_SH=/mnt/host/scripts/residual.sh
"$RESIDUAL_SH" > "$OUT/residual.json" || true

# --- launch the oracle under strace (execve witness) ---
rm -f /tmp/oracle.stdout /tmp/oracle.stderr /tmp/oracle-trace.log /tmp/oracle-mutate.marker
strace -f -s 256 -o /tmp/oracle-trace.log \
    -e trace=execve,execveat \
    /usr/bin/cachyos-kernel-manager \
    >/tmp/oracle.stdout 2>/tmp/oracle.stderr &
ORACLE_PID=$!

# --- drive the Configure window (a drive failure must NOT abort the
# observation: the tree dump + snapshots are still valuable evidence) ---
DRIVE_PY="/opt/cachyos-km-vm/oracle-mutate.py"
[ -f /mnt/host/scripts/oracle-mutate.py ] && DRIVE_PY=/mnt/host/scripts/oracle-mutate.py
DRIVE_RC=0
PYTHONPATH=/opt/a11y/pyatspi2 python3 "$DRIVE_PY" /tmp/oracle-state.json "${ACTIONS[@]}" || DRIVE_RC=$?
echo "drive rc=$DRIVE_RC actions=${ACTIONS[*]}"

# --- give the app a moment, then close it ---
sleep 1
kill -TERM "$ORACLE_PID" 2>/dev/null || true
sleep 1
kill -KILL "$ORACLE_PID" 2>/dev/null || true

# copy the evidence even when the drive failed (the failure is evidence)
cp /tmp/oracle-state.json "$OUT/oracle-state.json" 2>/dev/null || true
cp /tmp/pkgbuild-before.txt "$OUT/pkgbuild-before.txt" 2>/dev/null || true
cp /tmp/pkgbuild-after.txt "$OUT/pkgbuild-after.txt" 2>/dev/null || true
cp /tmp/conf-debug-*.json "$OUT/" 2>/dev/null || true
cp /tmp/oracle-trace.log "$OUT/oracle-trace.log" 2>/dev/null || true
cp /tmp/oracle.stdout "$OUT/oracle.stdout" 2>/dev/null || true
cp /tmp/oracle.stderr "$OUT/oracle.stderr" 2>/dev/null || true

# --- machine residual AFTER ---
"$RESIDUAL_SH" > "$OUT/residual.json.after" || true

echo "oracle mutation observation complete (drive_rc=$DRIVE_RC)"
exit "$DRIVE_RC"
