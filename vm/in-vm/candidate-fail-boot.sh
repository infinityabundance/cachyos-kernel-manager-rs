#!/usr/bin/env bash
#
# candidate-fail-boot.sh — the CANDIDATE side of the boot/system-boot-
# after-failure court (Phase 11), phase 1: the same setup + removal
# sequence with the command RENDERED BY THE CANDIDATE'S MODEL
# (`cachyos-kernel-manager-installcmd remove`: pacman_remove_argv, exec
# crate) for the RUNNING kernel. The exec chains + the pre/post state are
# compared byte-for-byte; the reboot phase (boot-check-failure.sh)
# witnesses the FAILED state.
#
set -euo pipefail
OUT="$1"
mkdir -p "$OUT"

# the model-rendered command (evidence the candidate executed THIS)
CMD="$(/mnt/host/inspect/cachyos-kernel-manager-installcmd remove linux-cachyos linux-cachyos-headers)"
echo "$CMD" > "$OUT/remove-command.txt"

# setup: the two-kernel state
pacman -S --noconfirm --needed linux-cachyos-lts linux-cachyos-lts-headers \
    >/tmp/setup.log 2>&1 || { cat /tmp/setup.log >&2; exit 1; }

(pacman -Q linux-cachyos linux-cachyos-lts 2>/dev/null || true) > "$OUT/pre-kernels.txt"
ls /boot/ > "$OUT/pre-boot.txt" 2>/dev/null || true
# flush the pre-remove evidence before the courted mutation (a mid-run
# failure must still leave the written surfaces on the host share)
sync

strace -f -e trace=execve -o /tmp/remove.trace \
    bash -lc "$CMD --noconfirm" >/tmp/remove.log 2>&1 \
    || { cat /tmp/remove.log >&2; exit 1; }

sync
grep -oE 'execve\("[^"]+", \[[^]]*\]' /tmp/remove.trace \
    | sed 's/^[0-9]* *//' \
    | sed 's|/tmp/mkinitcpio\.[A-Za-z0-9]*|/tmp/mkinitcpio.TMP|g' \
    | grep -v 'execve("/usr/bin/grep"' \
    > "$OUT/remove-execs.txt"
if [ ! -s "$OUT/remove-execs.txt" ]; then
    echo "EXTRACTION: empty exec chain" >&2
    exit 1
fi
cp /tmp/remove.trace "$OUT/remove-raw.trace" || true

pacman -Q linux-cachyos linux-cachyos-lts > "$OUT/post-kernels.txt" 2>/dev/null || true
ls /boot/ > "$OUT/post-boot.txt" 2>/dev/null || true
grep -E "mkinitcpio|initramfs|removing" /tmp/remove.log | head -6 > "$OUT/hook-output.txt" || true

pacman -Q > "$OUT/packages.txt" 2>/dev/null || true
