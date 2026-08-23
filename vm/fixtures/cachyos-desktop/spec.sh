#!/usr/bin/env bash
#
# cachyos-desktop — Phase 11+ demo fixture: a REAL CachyOS desktop VM.
# The base rootfs already ships linux-cachyos + the base system; this
# spec installs the full X stack + the XFCE desktop + a VNC server so the
# candidate GUI runs in a real desktop environment (not the minimal
# headless base). Network is available at bake time; the court VMs are
# offline.
#
set -euo pipefail

# refresh the sync dbs (the base's may be stale)
pacman -Sy --noconfirm >/tmp/sync.log 2>&1 || { cat /tmp/sync.log >&2; exit 1; }

# the real desktop: X, XFCE, a VNC server for the display, a terminal +
# window tools, and the X11 client libs winit dlopens at runtime
pacman -S --noconfirm --needed \
    xorg-server xorg-xinit xorg-xsetroot xorg-xrandr \
    xfce4 xfce4-terminal \
    x11vnc xterm xdotool \
    ttf-dejavu \
    libxcursor libxi libxrandr libxinerama libxft \
    >/tmp/desktop-install.log 2>&1 || { cat /tmp/desktop-install.log >&2; exit 1; }

echo "fixture cachyos-desktop: XFCE desktop + x11vnc ready"
