#!/usr/bin/env bash
#
# boot-check-remove.sh — the boot/system-boot-after-remove court's REBOOT
# phase: verifies the system came up after the kernel removal, the base
# kernel is intact, the lts kernel is GONE, and its initramfs was removed.
#
set -euo pipefail
OUT="$1"
mkdir -p "$OUT"

echo "boot-complete: $(systemctl is-system-running 2>/dev/null || echo degraded)" > "$OUT/boot-status.txt"
uname -r > "$OUT/running-kernel.txt"

pacman -Q linux-cachyos linux-cachyos-lts > "$OUT/kernels.txt" 2>/dev/null || true
ls /boot/ > "$OUT/boot-files.txt" 2>/dev/null || true

# hard assertions: the base kernel boots, the lts is GONE
if ! pacman -Q linux-cachyos >/dev/null 2>&1; then
    echo "BOOT-CHECK: the base kernel is missing" >&2
    exit 1
fi
if pacman -Q linux-cachyos-lts >/dev/null 2>&1; then
    echo "BOOT-CHECK: linux-cachyos-lts still installed" >&2
    exit 1
fi
if ls /boot/initramfs-linux-cachyos-lts.img >/dev/null 2>&1; then
    echo "BOOT-CHECK: the lts initramfs was not removed" >&2
    exit 1
fi

journalctl -b -n 3 --no-pager 2>/dev/null | head -3 \
    | sed -E 's/^[A-Z][a-z]{2} [0-9]{2} [0-9:]{8} [^ ]+ //' \
    | sed -E 's/\[[0-9]+\]/[PID]/g' \
    > "$OUT/journal-tail.txt" || true
