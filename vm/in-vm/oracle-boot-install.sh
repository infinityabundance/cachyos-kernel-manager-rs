#!/usr/bin/env bash
#
# oracle-boot-install.sh — the ORACLE side of the boot/system-boot-after-
# install court (Phase 11), phase 1: installs the REAL linux-cachyos-lts
# kernel + headers with the frozen source's literal command
# (`pacman -S --needed linux-cachyos-lts linux-cachyos-lts-headers` —
# commit_transaction, kernel.cpp:288-304) from the fixture's cache, lets
# the post-install hooks (mkinitcpio + the bootloader) run, and records the
# post-install state. The reboot phase (boot-check.sh) verifies the boot.
#
set -euo pipefail
OUT="$1"
mkdir -p "$OUT"

INSTALL_LITERAL='pacman -S --needed linux-cachyos-lts linux-cachyos-lts-headers'
echo "$INSTALL_LITERAL" > "$OUT/install-command.txt"

# the pre-install state
(pacman -Q linux-cachyos linux-cachyos-lts 2>/dev/null || true) > "$OUT/pre-kernels.txt"
ls /boot/ > "$OUT/pre-boot.txt" 2>/dev/null || true

# the install under strace (the exec chain witness); --noconfirm is a
# witness adaptation (the real flow answers the prompt in a terminal)
strace -f -e trace=execve -o /tmp/install.trace \
    bash -lc "$INSTALL_LITERAL --noconfirm" \
    >/tmp/install.log 2>&1 || { cat /tmp/install.log >&2; exit 1; }
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

# the post-install state (the hooks ran: /boot regenerated)
pacman -Q linux-cachyos linux-cachyos-lts > "$OUT/post-kernels.txt" 2>/dev/null || true
ls /boot/ > "$OUT/post-boot.txt" 2>/dev/null || true
# the mkinitcpio hooks' output (the hook evidence)
grep -E "mkinitcpio|initramfs|generating" /tmp/install.log | head -6 > "$OUT/hook-output.txt" || true

pacman -Q > "$OUT/packages.txt" 2>/dev/null || true
