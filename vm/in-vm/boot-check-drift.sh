#!/usr/bin/env bash
#
# boot-check-drift.sh — the boot/system-boot-drift court's REBOOT phase,
# invoked ONCE PER REBOOT with the reboot number: records the surface with
# a per-reboot suffix (boot-status-$N.txt, ...) so the SAME overlay's
# surfaces across multiple reboots are compared byte-for-byte (no drift)
# — and across the oracle/candidate sides. Hard-asserts the machine came
# up and the install mutation persisted (the base + the lts + the lts
# initramfs all present).
#
set -euo pipefail
OUT="$1"
N="${2:-1}"
mkdir -p "$OUT"

echo "boot-complete: $(systemctl is-system-running 2>/dev/null || echo degraded)" > "$OUT/boot-status-$N.txt"
uname -r > "$OUT/running-kernel-$N.txt"

pacman -Q linux-cachyos linux-cachyos-lts > "$OUT/kernels-$N.txt" 2>/dev/null || true
ls /boot/ > "$OUT/boot-files-$N.txt" 2>/dev/null || true

# hard assertions: the machine came up and the install persisted
if ! pacman -Q linux-cachyos >/dev/null 2>&1; then
    echo "DRIFT-CHECK: the base kernel is missing" >&2
    exit 1
fi
if ! pacman -Q linux-cachyos-lts >/dev/null 2>&1; then
    echo "DRIFT-CHECK: linux-cachyos-lts not installed" >&2
    exit 1
fi
if ! ls /boot/initramfs-linux-cachyos-lts.img >/dev/null 2>&1; then
    echo "DRIFT-CHECK: the lts initramfs missing" >&2
    exit 1
fi

journalctl -b -n 3 --no-pager 2>/dev/null | head -3 \
    | sed -E 's/^[A-Z][a-z]{2} [0-9]{2} [0-9:]{8} [^ ]+ //' \
    | sed -E 's/\[[0-9]+\]/[PID]/g' \
    > "$OUT/journal-tail-$N.txt" || true
