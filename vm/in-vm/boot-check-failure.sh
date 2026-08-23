#!/usr/bin/env bash
#
# boot-check-failure.sh — the boot/system-boot-after-failure court's
# REBOOT phase, run ONLY when the direct-boot machine came up after
# removing the RUNNING kernel (the expected outcome is that it does NOT:
# the runner records boot-attempt.txt = boot-failed after a bounded probe,
# and this script is the contingency that hard-asserts the FAILED state).
# The harness's direct-kernel boot provides vmlinuz + initramfs from the
# HOST, so if the machine does come up the "failure" is the machine's own
# destroyed boot path: the running kernel's packages + its /boot entry are
# GONE (a real machine would fail its next boot from its own disk). The
# assertions are INVERTED vs boot-check-remove.sh — they hard-fail unless
# the failure residual is exactly present.
#
set -euo pipefail
OUT="$1"
mkdir -p "$OUT"

echo "boot-complete: $(systemctl is-system-running 2>/dev/null || echo degraded)" > "$OUT/boot-status.txt"
uname -r > "$OUT/running-kernel.txt"

pacman -Q linux-cachyos linux-cachyos-lts > "$OUT/kernels.txt" 2>/dev/null || true
ls /boot/ > "$OUT/boot-files.txt" 2>/dev/null || true

# hard assertions: the FAILED state is present — the base (running) kernel
# is GONE, its /boot entry is GONE, the lts remains with its initramfs.
if pacman -Q linux-cachyos >/dev/null 2>&1; then
    echo "BOOT-CHECK: the base kernel SURVIVED the removal (failure residual absent)" >&2
    exit 1
fi
if ls /boot/initramfs-linux-cachyos.img >/dev/null 2>&1; then
    echo "BOOT-CHECK: the base initramfs survived (failure residual absent)" >&2
    exit 1
fi
if ! pacman -Q linux-cachyos-lts >/dev/null 2>&1; then
    echo "BOOT-CHECK: linux-cachyos-lts missing" >&2
    exit 1
fi
if ! ls /boot/initramfs-linux-cachyos-lts.img >/dev/null 2>&1; then
    echo "BOOT-CHECK: the lts initramfs missing" >&2
    exit 1
fi

journalctl -b -n 3 --no-pager 2>/dev/null | head -3 \
    | sed -E 's/^[A-Z][a-z]{2} [0-9]{2} [0-9:]{8} [^ ]+ //' \
    | sed -E 's/\[[0-9]+\]/[PID]/g' \
    > "$OUT/journal-tail.txt" || true
