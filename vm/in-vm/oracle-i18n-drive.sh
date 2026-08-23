#!/usr/bin/env bash
#
# oracle-i18n-drive.sh — Phase 12 hostile-review rendered-i18n court (oracle
# side): launch the frozen Qt GUI under the GENERATED locale (de_DE.UTF-8
# then zh_CN.UTF-8) under Xvfb + AT-SPI, and project the RENDERED main
# window's accessible chrome (i18n-drive.py is side-agnostic; the oracle
# mode waits for the app the wrapper launched).
#
set -euo pipefail

OUT="${1:-/mnt/host/out}"
mkdir -p "$OUT"
cd /tmp

if [ -f /mnt/host/scripts/oracle-i18n-drive.sh ] && [ "$0" != "/mnt/host/scripts/oracle-i18n-drive.sh" ]; then
    exec /mnt/host/scripts/oracle-i18n-drive.sh "$OUT"
fi

if [ ! -f /etc/cachyos-km/fixture.marker ]; then
    echo "REFUSING: /etc/cachyos-km/fixture.marker missing — not an approved court VM" >&2
    exit 3
fi

ORACLE=/usr/bin/cachyos-kernel-manager
if [ ! -x "$ORACLE" ]; then
    echo "REFUSING: $ORACLE missing" >&2
    exit 3
fi

export DISPLAY=:99
eval "$(dbus-launch --sh-syntax)"
export DBUS_SESSION_BUS_ADDRESS
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
trap 'kill $XVFB_PID 2>/dev/null || true; pkill -f cachyos-kernel-manager 2>/dev/null || true' EXIT
sleep 1

RESIDUAL_SH="/opt/cachyos-km-vm/residual.sh"
[ -f /mnt/host/scripts/residual.sh ] && RESIDUAL_SH=/mnt/host/scripts/residual.sh
"$RESIDUAL_SH" > "$OUT/residual.json" || true

DRIVE_PY="/opt/cachyos-km-vm/i18n-drive.py"
[ -f /mnt/host/scripts/i18n-drive.py ] && DRIVE_PY=/mnt/host/scripts/i18n-drive.py
DRIVE_RC=0

run_locale() {
    local lang="$1" key="$2"
    # the oracle resolves QLocale::system() from the ACTUAL generated locale
    # (an ungenerated de_DE silently falls back to C — the fixture bakes
    # both locales so this is real)
    env LANG="$lang" LC_ALL="$lang" "$ORACLE" >/tmp/oracle-i18n.stdout 2>/tmp/oracle-i18n.stderr &
    local app_pid=$!
    sleep 3
    env KM_LOCALE="$key" \
        PYTHONPATH=/opt/a11y/pyatspi2 python3 "$DRIVE_PY" "$OUT" || DRIVE_RC=$?
    kill "$app_pid" 2>/dev/null || true
    sleep 1
    pkill -f cachyos-kernel-manager 2>/dev/null || true
}

run_locale de_DE.UTF-8 de_DE
run_locale zh_CN.UTF-8 zh_CN

cp /tmp/oracle-i18n.stdout "$OUT/oracle.stdout" 2>/dev/null || true
cp /tmp/oracle-i18n.stderr "$OUT/oracle.stderr" 2>/dev/null || true

"$RESIDUAL_SH" > "$OUT/residual.json.after" || true

echo "oracle i18n drive complete (drive_rc=$DRIVE_RC)"
exit "$DRIVE_RC"
