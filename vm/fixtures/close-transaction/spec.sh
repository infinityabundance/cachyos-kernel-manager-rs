#!/usr/bin/env bash
#
# close-transaction — Phase 12 hostile-review fixture for the gap-010
# close-during-transaction court: the gui-integration stack (winit X11
# client libs, at-spi2-core-2.52.0 for accesskit_unix 0.22.1, a font, the
# base's polkit rule authorizing the cachyos-kernel-manager action without
# password) PLUS a slow-pacman wrapper.
#
# The wrapper makes the transaction terminal stay in-flight for 15s so the
# court's window close deterministically hits the worker while it is
# blocked in the transaction (the oracle's closeEvent releases the alpm
# handle and the app exits while the worker QThread is still running ->
# Qt's "QThread: Destroyed while thread is still running" abort, witnessed
# 2026-08-23; the candidate's runtime-owned task + clean event-loop exit
# has no such abort).
#
set -euo pipefail

# the gui-integration prerequisites (unchanged from that fixture's spec)
cat > /tmp/gui-spec.sh <<'SPEC'
#!/usr/bin/env bash
set -euo pipefail
pacman -Sy --noconfirm >/tmp/sync.log 2>&1 || { cat /tmp/sync.log >&2; exit 1; }
curl -fsSL -o /tmp/at-spi2-core-2.52.0-1-x86_64.pkg.tar.zst \
    https://archive.archlinux.org/packages/a/at-spi2-core/at-spi2-core-2.52.0-1-x86_64.pkg.tar.zst \
    >/tmp/atspi-dl.log 2>&1 || { cat /tmp/atspi-dl.log >&2; exit 1; }
pacman -U --noconfirm /tmp/at-spi2-core-2.52.0-1-x86_64.pkg.tar.zst \
    >/tmp/atspi-downgrade.log 2>&1 || { cat /tmp/atspi-downgrade.log >&2; exit 1; }
pacman -Q at-spi2-core
pacman -S --noconfirm --needed \
    libx11 libxcb libxkbcommon libxkbcommon-x11 \
    libxcursor libxi libxrandr libxinerama libxft \
    xorg-xrandr xdotool ttf-dejavu xterm \
    >/tmp/gui-install.log 2>&1 || { cat /tmp/gui-install.log >&2; exit 1; }
SPEC
bash /tmp/gui-spec.sh

# the slow-pacman wrapper: /usr/local/bin precedes /usr/bin in PATH, so the
# transaction's `pacman -S ...` lands here. It sleeps 30s ONLY for
# TRANSACTION operations (-S/-R/-U — the court's in-flight window); read-
# only queries (the residual witness's `pacman -Q`) pass through instantly.
cat > /usr/local/bin/pacman <<'WRAP'
#!/usr/bin/env bash
for arg in "$@"; do
    if [ "$arg" = "-S" ] || [ "$arg" = "-R" ] || [ "$arg" = "-U" ]; then
        echo "slow-pacman: sleeping 30s (the court's close window)" >&2
        sleep 30
        break
    fi
done
exec /usr/bin/pacman "$@"
WRAP
chmod +x /usr/local/bin/pacman
# the real pacman must still be reachable by its absolute path
test -x /usr/bin/pacman

# the candidate's run_cmd_terminal spawns the PACKAGED terminal-helper +
# rootshell.sh at /usr/lib/cachyos-kernel-manager/ (the candidate binary
# itself runs from the share, but the packaged helper paths are the exec
# contract) — the bake stages the fixture payload files as
# /opt/cachyos-km-vm/payload-*
mkdir -p /usr/lib/cachyos-kernel-manager
install -m 0755 /opt/cachyos-km-vm/payload-terminal-helper /usr/lib/cachyos-kernel-manager/terminal-helper
install -m 0755 /opt/cachyos-km-vm/payload-rootshell.sh /usr/lib/cachyos-kernel-manager/rootshell.sh

# xterm is the terminal-helper's pick in this fixture (installed above)

echo "fixture close-transaction: gui-integration stack + slow-pacman + packaged helpers ready (at-spi2-core $(pacman -Qq at-spi2-core))"
