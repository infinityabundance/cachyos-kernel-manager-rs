#!/usr/bin/env bash
#
# candidate-boot-install.sh — the CANDIDATE side of the boot/system-boot-
# after-install court (Phase 11), phase 1: installs the REAL linux-cachyos-
# lts kernel + headers with the command RENDERED BY THE CANDIDATE'S MODEL
# (`cachyos-kernel-manager-installcmd`: pacman_install_argv, exec crate) —
# the same sequence as the oracle side. The exec chains + the post-install
# state are compared byte-for-byte.
#
set -euo pipefail
OUT="$1"
mkdir -p "$OUT"

# the model-rendered command (evidence the candidate executed THIS)
CMD="$(/mnt/host/inspect/cachyos-kernel-manager-installcmd linux-cachyos-lts linux-cachyos-lts-headers)"
echo "$CMD" > "$OUT/install-command.txt"

# the pre-install state
(pacman -Q linux-cachyos linux-cachyos-lts 2>/dev/null || true) > "$OUT/pre-kernels.txt"
ls /boot/ > "$OUT/pre-boot.txt" 2>/dev/null || true

strace -f -e trace=execve -o /tmp/install.trace \
    bash -lc "$CMD --noconfirm" >/tmp/install.log 2>&1 \
    || { cat /tmp/install.log >&2; exit 1; }
# the extraction (a hard failure on an EMPTY chain: the court must not
# pass on missing evidence). The module-scan greps and the random
# mkinitcpio tmp dirs are filtered (nondeterministic find/hash order).
sync
grep -oE 'execve\("[^"]+", \[[^]]*\]' /tmp/install.trace \
    | sed 's/^[0-9]* *//' \
    | sed 's|/tmp/mkinitcpio\.[A-Za-z0-9]*|/tmp/mkinitcpio.TMP|g' \
    | grep -v 'execve("/usr/bin/grep"' \
    > "$OUT/install-execs.txt"
if [ ! -s "$OUT/install-execs.txt" ]; then
    echo "EXTRACTION: empty exec chain" >&2
    exit 1
fi
cp /tmp/install.trace "$OUT/install-raw.trace" || true

pacman -Q linux-cachyos linux-cachyos-lts > "$OUT/post-kernels.txt" 2>/dev/null || true
ls /boot/ > "$OUT/post-boot.txt" 2>/dev/null || true
grep -E "mkinitcpio|initramfs|generating" /tmp/install.log | head -6 > "$OUT/hook-output.txt" || true

pacman -Q > "$OUT/packages.txt" 2>/dev/null || true
