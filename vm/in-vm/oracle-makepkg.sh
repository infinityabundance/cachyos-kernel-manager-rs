#!/usr/bin/env bash
#
# oracle-makepkg.sh — the ORACLE side of the build-env/makepkg-runtime
# court (gap-006). Executes the FROZEN SOURCE's literal build commands
# (the exact strings from conf-window.cpp:734 and aur_kernel.cpp:53,
# shell-wrapped like the terminal-helper path) under strace:
#   - `makepkg -scf --cleanbuild --skipchecksums && touch .done-status`
#     on /home/test/build-proj (depends=('km-runtime-dep') in the repo):
#     -s must resolve the dep via `sudo pacman -S --asdeps`; -scf must NOT
#     install the built package;
#   - `makepkg -sicf --cleanbuild --skipchecksums` (aur_kernel.cpp:53):
#     -s resolves the same dep; -i additionally installs the built package
#     via `sudo pacman -U`;
#   - the failure mode: the repo command on /home/test/aur-proj
#     (depends=('km-aur-only-dep') — resolvable NOWHERE): -s fails with the
#     same outcome for both commands.
#
# The extracted execve chains (normalized) are the compared artifacts —
# the candidate side executes its MODEL-RENDERED strings (which must be
# byte-identical to these literals).
#
set -euo pipefail
OUT="$1"
mkdir -p "$OUT"

export REPO_LITERAL='makepkg -scf --cleanbuild --skipchecksums && touch .done-status'
export AUR_LITERAL='makepkg -sicf --cleanbuild --skipchecksums'
printf 'repo_build_command=%s\naur_build_command=%s\n' "$REPO_LITERAL" "$AUR_LITERAL" > "$OUT/commands.txt"

MAKEPKG_VERSION="$(makepkg --version 2>/dev/null | head -1 || true)"
echo "$MAKEPKG_VERSION" > "$OUT/makepkg-version.txt"

# scenario 1: -scf on build-proj (deps resolved, NOT installed)
cd /home/test/build-proj
rm -f .done-status
strace -f -e trace=execve -o /tmp/scf.trace \
    bash -lc 'yes | sudo -u test bash -lc "$REPO_LITERAL"' >/dev/null 2>&1 || true
grep -oE 'execve\("[^"]+", \[[^]]*\]' /tmp/scf.trace | sed 's/^[0-9]* *//' | sed 's/@[0-9]\+/@TS/g' \
    > "$OUT/scf-execs.txt" || true
cp /tmp/scf.trace "$OUT/scf-raw.trace" || true
echo "scf: km-runtime-dep installed? $(pacman -Q km-runtime-dep 2>/dev/null || echo NO)"
echo "scf: km-runtime-kernel installed? $(pacman -Q km-runtime-kernel 2>/dev/null || echo NO)"
echo "scf: .done-status? $(test -f .done-status && echo YES || echo NO)"

# scenario 2: -sicf on build-proj (deps resolved + the package INSTALLED)
rm -f /home/test/build-proj/*.pkg.tar.*
strace -f -e trace=execve -o /tmp/sicf.trace \
    bash -lc 'yes | sudo -u test bash -lc "$AUR_LITERAL"' >/dev/null 2>&1 || true
grep -oE 'execve\("[^"]+", \[[^]]*\]' /tmp/sicf.trace | sed 's/^[0-9]* *//' | sed 's/@[0-9]\+/@TS/g' \
    > "$OUT/sicf-execs.txt" || true
cp /tmp/sicf.trace "$OUT/sicf-raw.trace" || true
echo "sicf: km-runtime-kernel installed? $(pacman -Q km-runtime-kernel 2>/dev/null || echo NO)"

# scenario 3: the AUR-only dep failure mode (-s cannot resolve it)
cd /home/test/aur-proj
strace -f -e trace=execve -o /tmp/aur.trace \
    bash -lc 'yes | sudo -u test bash -lc "$REPO_LITERAL"' >/dev/null 2>&1 || true
grep -oE 'execve\("[^"]+", \[[^]]*\]' /tmp/aur.trace | sed 's/^[0-9]* *//' \
    > "$OUT/aurfail-execs.txt" || true
cp /tmp/aur.trace "$OUT/aurfail-raw.trace" || true
echo "aurfail: km-aur-only built? $(ls /home/test/aur-proj/*.pkg.tar.* 2>/dev/null || echo NO)"

# the machine state (fixture-integrity check)
pacman -Q > "$OUT/packages.txt" 2>/dev/null || true
