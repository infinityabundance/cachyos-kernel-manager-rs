#!/usr/bin/env bash
#
# oracle-remove-boot.sh — the ORACLE side of the boot/system-boot-after-
# remove court (Phase 11), phase 1: SETS UP the two-kernel state (installs
# the cached linux-cachyos-lts), then REMOVES it with the frozen source's
# literal command (`pacman -Rsn linux-cachyos-lts linux-cachyos-lts-headers`
# — commit_transaction, kernel.cpp:288-304) under strace, lets the
# post-remove hooks (mkinitcpio) run, and records the post-remove state.
# The reboot phase (boot-check-remove.sh) verifies the boot + the removal
# persisted.
#
set -euo pipefail
OUT="$1"
mkdir -p "$OUT"

REMOVE_LITERAL='pacman -Rsn linux-cachyos-lts linux-cachyos-lts-headers'
echo "$REMOVE_LITERAL" > "$OUT/remove-command.txt"

# setup: the two-kernel state (the lts is cached in the fixture)
pacman -S --noconfirm --needed linux-cachyos-lts linux-cachyos-lts-headers \
    >/tmp/setup.log 2>&1 || { cat /tmp/setup.log >&2; exit 1; }

# the pre-remove state
pacman -Q linux-cachyos linux-cachyos-lts > "$OUT/pre-kernels.txt" 2>/dev/null || true
ls /boot/ > "$OUT/pre-boot.txt" 2>/dev/null || true
# flush the pre-remove evidence before the courted mutation (a mid-run
# failure must still leave the written surfaces on the host share)
sync

# the removal under strace (the exec chain witness); --noconfirm is a
# witness adaptation
strace -f -e trace=execve -o /tmp/remove.trace \
    bash -lc "$REMOVE_LITERAL --noconfirm" \
    >/tmp/remove.log 2>&1 || { cat /tmp/remove.log >&2; exit 1; }

# the extraction (a hard failure on an empty chain)
sync
grep -oE '^[0-9]+[[:space:]]+execve\("[^"]+", \[[^]]*\]' /tmp/remove.trace \
    | sort -s -k1,1n \
    | sed 's/^[0-9]* *//' \
    | sed 's|/tmp/mkinitcpio\.[A-Za-z0-9]*|/tmp/mkinitcpio.TMP|g' \
    | grep -v 'execve("/usr/bin/grep"' \
    > "$OUT/remove-execs.txt"
if [ ! -s "$OUT/remove-execs.txt" ]; then
    echo "EXTRACTION: empty exec chain" >&2
    exit 1
fi
cp /tmp/remove.trace "$OUT/remove-raw.trace" || true

# the post-remove state (the hooks ran: the lts initramfs gone)
pacman -Q linux-cachyos linux-cachyos-lts > "$OUT/post-kernels.txt" 2>/dev/null || true
ls /boot/ > "$OUT/post-boot.txt" 2>/dev/null || true
grep -E "mkinitcpio|initramfs|removing" /tmp/remove.log | head -6 > "$OUT/hook-output.txt" || true

pacman -Q > "$OUT/packages.txt" 2>/dev/null || true
