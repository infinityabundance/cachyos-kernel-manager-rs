#!/usr/bin/env bash
#
# candidate-i18n-drive.sh — Phase 12 hostile-review rendered-i18n court
# (candidate side): the release Slint binary under the GENERATED locale
# (de_DE.UTF-8 then zh_CN.UTF-8) under Xvfb + AT-SPI (with the
# org.a11y.Status enablement stub — the accesskit_unix 0.22.1 registration
# gate), projecting the RENDERED main-window accessible chrome
# (i18n-drive.py --candidate launches the binary per locale).
#
set -euo pipefail

OUT="${1:-/mnt/host/out}"
mkdir -p "$OUT"
cd /tmp

if [ -f /mnt/host/scripts/candidate-i18n-drive.sh ] && [ "$0" != "/mnt/host/scripts/candidate-i18n-drive.sh" ]; then
    exec /mnt/host/scripts/candidate-i18n-drive.sh "$OUT"
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

# the GPU-less winit-SOFTWARE renderer, EXPLICIT (never rely on backend
# selection)
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
python3 /mnt/host/scripts/a11y-status-stub.py >/tmp/a11y-stub.log 2>&1 &
STUB_PID=$!
export DISPLAY=:99
Xvfb :99 -screen 0 1280x800x24 >/tmp/xvfb.log 2>&1 &
XVFB_PID=$!
trap 'kill $XVFB_PID 2>/dev/null || true; kill $STUB_PID 2>/dev/null || true; pkill -f cachyos-kernel-manager 2>/dev/null || true' EXIT
sleep 1

RESIDUAL_SH="/opt/cachyos-km-vm/residual.sh"
[ -f /mnt/host/scripts/residual.sh ] && RESIDUAL_SH=/mnt/host/scripts/residual.sh
"$RESIDUAL_SH" > "$OUT/residual.json" || true

DRIVE_PY="/opt/cachyos-km-vm/i18n-drive.py"
[ -f /mnt/host/scripts/i18n-drive.py ] && DRIVE_PY=/mnt/host/scripts/i18n-drive.py
DRIVE_RC=0
# the driver launches the binary itself, with the locale env it sets per
# KM_LOCALE (the candidate parses LANG/LC_ALL directly — no OS locale
# needed, but the same generated locales keep the machine witness honest)
env KM_LOCALE=de_DE LANG=de_DE.UTF-8 LC_ALL=de_DE.UTF-8 \
    PYTHONPATH=/opt/a11y/pyatspi2 python3 "$DRIVE_PY" --candidate "$CAND" "$OUT" || DRIVE_RC=$?
env KM_LOCALE=zh_CN LANG=zh_CN.UTF-8 LC_ALL=zh_CN.UTF-8 \
    PYTHONPATH=/opt/a11y/pyatspi2 python3 "$DRIVE_PY" --candidate "$CAND" "$OUT" || DRIVE_RC=$?

"$RESIDUAL_SH" > "$OUT/residual.json.after" || true

echo "candidate i18n drive complete (drive_rc=$DRIVE_RC)"
exit "$DRIVE_RC"
