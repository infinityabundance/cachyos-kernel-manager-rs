#!/usr/bin/env bash
#
# boot-check.sh — the boot/system-boot-after-install court's REBOOT phase:
# runs after the VM reboots on the SAME overlay (the kernel install
# persisted). Verifies the system came up, the new kernel is installed, its
# initramfs exists in /boot, and the boot journal shows a clean boot.
#
set -euo pipefail
OUT="$1"
mkdir -p "$OUT"

echo "boot-complete: $(systemctl is-system-running 2>/dev/null || echo degraded)" > "$OUT/boot-status.txt"
uptime -p > "$OUT/uptime.txt" 2>/dev/null || true

# the running kernel (the qemu direct boot still uses the base kernel)
uname -r > "$OUT/running-kernel.txt"

# the installed kernels (the mutation persisted across the reboot)
pacman -Q linux-cachyos linux-cachyos-lts > "$OUT/kernels.txt" 2>/dev/null || true
ls /boot/ > "$OUT/boot-files.txt" 2>/dev/null || true

# the new kernel's initramfs must exist (the hook output survived)
if ! ls /boot/initramfs-linux-cachyos-lts.img >/dev/null 2>&1; then
    echo "BOOT-CHECK: linux-cachyos-lts initramfs missing" >&2
    exit 1
fi
if ! pacman -Q linux-cachyos-lts >/dev/null 2>&1; then
    echo "BOOT-CHECK: linux-cachyos-lts not installed" >&2
    exit 1
fi

# the last boot's journal (a clean boot log tail)
journalctl -b -n 3 --no-pager 2>/dev/null | head -3 \
    | sed -E 's/^[A-Z][a-z]{2} [0-9]{2} [0-9:]{8} [^ ]+ //' \
    | sed -E 's/\[[0-9]+\]/[PID]/g' \
    > "$OUT/journal-tail.txt" || true
