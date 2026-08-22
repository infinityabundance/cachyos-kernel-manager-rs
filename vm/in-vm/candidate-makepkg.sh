#!/usr/bin/env bash
#
# candidate-makepkg.sh — the CANDIDATE side of the build-env/makepkg-runtime
# court (gap-006). Executes the commands RENDERED BY THE CANDIDATE'S MODEL
# (`cachyos-kernel-manager-buildcmd`: BuildFlowPlan::render's
# build_command = `makepkg -scf --cleanbuild --skipchecksums && touch
# .done-status` for the repo path; makepkg_aur_argv = `makepkg -sicf
# --cleanbuild --skipchecksums` for the AUR path) under strace — the SAME
# scenario sequence as the oracle side. The extracted execve chains must be
# BYTE-IDENTICAL to the oracle's (which uses the frozen source's literal
# strings): the court witnesses at runtime that the model renders the
# oracle's commands exactly.
#
set -euo pipefail
OUT="$1"
mkdir -p "$OUT"

# the model-rendered commands (evidence the candidate executed THESE)
/mnt/host/inspect/cachyos-kernel-manager-buildcmd > "$OUT/commands.txt"
export REPO_CMD="$(grep '^repo_build_command=' "$OUT/commands.txt" | cut -d= -f2-)"
export AUR_CMD="$(grep '^aur_build_command=' "$OUT/commands.txt" | cut -d= -f2-)"

MAKEPKG_VERSION="$(makepkg --version 2>/dev/null | head -1 || true)"
echo "$MAKEPKG_VERSION" > "$OUT/makepkg-version.txt"

# scenario 1: the repo build command (build_command == -scf && touch)
cd /home/test/build-proj
rm -f .done-status
strace -f -e trace=execve -o /tmp/scf.trace \
    bash -lc 'yes | sudo -u test bash -lc "$REPO_CMD"' >/dev/null 2>&1 || true
grep -oE 'execve\("[^"]+", \[[^]]*\]' /tmp/scf.trace | sed 's/^[0-9]* *//' | sed 's/@[0-9]\+/@TS/g' \
    > "$OUT/scf-execs.txt" || true
cp /tmp/scf.trace "$OUT/scf-raw.trace" || true
echo "scf: km-runtime-dep installed? $(pacman -Q km-runtime-dep 2>/dev/null || echo NO)"
echo "scf: km-runtime-kernel installed? $(pacman -Q km-runtime-kernel 2>/dev/null || echo NO)"
echo "scf: .done-status? $(test -f .done-status && echo YES || echo NO)"

# scenario 2: the AUR build command (== -sicf)
rm -f /home/test/build-proj/*.pkg.tar.*
strace -f -e trace=execve -o /tmp/sicf.trace \
    bash -lc 'yes | sudo -u test bash -lc "$AUR_CMD"' >/dev/null 2>&1 || true
grep -oE 'execve\("[^"]+", \[[^]]*\]' /tmp/sicf.trace | sed 's/^[0-9]* *//' | sed 's/@[0-9]\+/@TS/g' \
    > "$OUT/sicf-execs.txt" || true
cp /tmp/sicf.trace "$OUT/sicf-raw.trace" || true
echo "sicf: km-runtime-kernel installed? $(pacman -Q km-runtime-kernel 2>/dev/null || echo NO)"

# scenario 3: the AUR-only dep failure mode (the same -s semantics)
cd /home/test/aur-proj
strace -f -e trace=execve -o /tmp/aur.trace \
    bash -lc 'yes | sudo -u test bash -lc "$REPO_CMD"' >/dev/null 2>&1 || true
grep -oE 'execve\("[^"]+", \[[^]]*\]' /tmp/aur.trace | sed 's/^[0-9]* *//' \
    > "$OUT/aurfail-execs.txt" || true
cp /tmp/aur.trace "$OUT/aurfail-raw.trace" || true
echo "aurfail: km-aur-only built? $(ls /home/test/aur-proj/*.pkg.tar.* 2>/dev/null || echo NO)"

# the machine state (fixture-integrity check)
pacman -Q > "$OUT/packages.txt" 2>/dev/null || true
