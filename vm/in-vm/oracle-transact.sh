#!/usr/bin/env bash
#
# oracle-transact.sh — run the real oracle GUI under Xvfb with execve
# tracing, drive a REAL transaction through AT-SPI (toggle a kernel row +
# click Execute), and capture the exec chain it produces.
#
# Usage: oracle-transact.sh <out-dir> <raw> [<raw> ...]
#
# Produces (into $1, default /mnt/host/out):
#   oracle-state.json        full AT-SPI tree of the SAME run (rows)
#   oracle-transaction.json  probe/exec/terminal chains (strace witness)
#   oracle-trace.log         the raw strace log (immutable evidence)
#   oracle.stdout/stderr     oracle's own output
#   residual.json(.after)    machine residual before/after
#
# Why this is the honest Phase 5 witness: the pacman argv (`-S --needed
# <list>` / `-Rsn <list>`) IS the complete observable of the companion
# decision matrix (kernel.cpp:89-163); the terminal-helper argv is the
# runCmdTerminal witness (utils.cpp:122-135). Both are compared against the
# candidate plan tool's modeled chains.
#
set -euo pipefail

OUT="${1:-/mnt/host/out}"
shift || true
TARGETS=("$@")

mkdir -p "$OUT"
cd /tmp

# --- iterate without rebaking: prefer the share copy when present ---
if [ -f /mnt/host/scripts/oracle-transact.sh ] && [ "$0" != "/mnt/host/scripts/oracle-transact.sh" ]; then
    exec /mnt/host/scripts/oracle-transact.sh "$OUT" "${TARGETS[@]}"
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
trap 'kill $XVFB_PID 2>/dev/null || true; kill $ORACLE_PID 2>/dev/null || true; pkill -f "cachyos-kernel-manager" 2>/dev/null || true; pkill -f xterm 2>/dev/null || true' EXIT
sleep 1

# --- machine residual BEFORE ---
RESIDUAL_SH="/opt/cachyos-km-vm/residual.sh"
[ -f /mnt/host/scripts/residual.sh ] && RESIDUAL_SH=/mnt/host/scripts/residual.sh
"$RESIDUAL_SH" > "$OUT/residual.json" || true

# --- launch the oracle under strace (execve witness) ---
rm -f /tmp/oracle.stdout /tmp/oracle.stderr /tmp/oracle-trace.log /tmp/oracle-drive.marker
strace -f -s 256 -o /tmp/oracle-trace.log \
    -e trace=execve,execveat \
    /usr/bin/cachyos-kernel-manager \
    >/tmp/oracle.stdout 2>/tmp/oracle.stderr &
ORACLE_PID=$!

# --- drive the transaction via AT-SPI (a drive failure must NOT abort the
# observation: the tree dump + trace are still valuable evidence) ---
DRIVE_PY="/opt/cachyos-km-vm/oracle-drive.py"
[ -f /mnt/host/scripts/oracle-drive.py ] && DRIVE_PY=/mnt/host/scripts/oracle-drive.py
DRIVE_RC=0
PYTHONPATH=/opt/a11y/pyatspi2 python3 "$DRIVE_PY" /tmp/oracle-state.json "${TARGETS[@]}" || DRIVE_RC=$?
echo "drive rc=$DRIVE_RC targets=${TARGETS[*]}"

# --- unblock the worker: kill the first xterm (terminal-helper then exits
# with 2 — the non-kgx failure path — and the worker proceeds to the next
# commit phase; without this the removal phase would never start) ---
XTERM_PID="$(grep -m1 'execve(".*/xterm"' /tmp/oracle-trace.log 2>/dev/null | awk '{print $1}' || true)"
if [ -n "$XTERM_PID" ]; then
    kill -TERM "$XTERM_PID" 2>/dev/null || true
    echo "killed first xterm ($XTERM_PID) to unblock the commit chain"
fi

# --- wait for the transaction execs to complete (second phase if any) ---
for _ in $(seq 60); do
    if grep -q 'execve(".*"pacman"' /tmp/oracle-trace.log 2>/dev/null; then
        # allow the second phase (removal) to appear after the xterm kill
        sleep 5
        break
    fi
    sleep 1
done

# --- extract the exec-chain witness (explicit normalizer) ---
EXTRACT_PY="/opt/cachyos-km-vm/extract-transaction.py"
[ -f /mnt/host/scripts/extract-transaction.py ] && EXTRACT_PY=/mnt/host/scripts/extract-transaction.py
python3 "$EXTRACT_PY" /tmp/oracle-trace.log /tmp/oracle-transaction.json || EXTRACT_RC=$?
EXTRACT_RC="${EXTRACT_RC:-0}"

# --- give the app a moment, then close it ---
sleep 1
kill -TERM "$ORACLE_PID" 2>/dev/null || true
sleep 1
kill -KILL "$ORACLE_PID" 2>/dev/null || true

cp /tmp/oracle-state.json "$OUT/oracle-state.json"
cp /tmp/oracle-transaction.json "$OUT/oracle-transaction.json"
cp /tmp/oracle-trace.log "$OUT/oracle-trace.log"
cp /tmp/oracle.stdout "$OUT/oracle.stdout"
cp /tmp/oracle.stderr "$OUT/oracle.stderr"

# --- machine residual AFTER ---
"$RESIDUAL_SH" > "$OUT/residual.json.after" || true

echo "oracle transaction observation complete (drive_rc=$DRIVE_RC extract_rc=$EXTRACT_RC)"
[ "$DRIVE_RC" -eq 0 ] || exit "$DRIVE_RC"
exit "$EXTRACT_RC"
