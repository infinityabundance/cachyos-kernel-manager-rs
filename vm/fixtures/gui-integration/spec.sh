#!/usr/bin/env bash
#
# gui-integration — Phase 12 fixture: the packaged Slint GUI's runtime
# prerequisites for the production-integration closure (the AT-SPI
# differential courts that drive the INSTALLED binary).
#
# The base rootfs ships the headless court tooling (Xvfb, at-spi2,
# pyatspi2, dbus) but NOT the X11 CLIENT libraries winit dlopens at
# startup (libX11/xcb/xkbcommon — the candidate aborts with "Failed to
# load one of xlib's shared libraries" without them). This spec installs
# exactly that client stack (no desktop — the courts run Xvfb like the
# oracle side). Network is available at bake time; the court VMs are
# offline.
#
set -euo pipefail

# refresh the sync dbs (the base's may be stale)
pacman -Sy --noconfirm >/tmp/sync.log 2>&1 || { cat /tmp/sync.log >&2; exit 1; }

# accesskit_unix 0.22.1 (slint 1.17.1's AT-SPI bridge) is INCOMPATIBLE with
# at-spi2-core >= 2.54: the registry's protocol change makes it reject
# accesskit's full-tree updates ("AddAccessible with unknown signature" —
# the tree VANISHES after a model rebuild such as a column sort; observed
# 2026-08-23 on 2.54.1 AND 2.60.6). Pin 2.52.0 — the last release
# accesskit_unix 0.22.1 was actively tested against — from the Arch
# archive BEFORE installing the X11 stack. Qt (the oracle side) and
# libatspi consumers are unaffected (the libatspi.so.0 ABI is stable).
curl -fsSL -o /tmp/at-spi2-core-2.52.0-1-x86_64.pkg.tar.zst \
    https://archive.archlinux.org/packages/a/at-spi2-core/at-spi2-core-2.52.0-1-x86_64.pkg.tar.zst \
    >/tmp/atspi-dl.log 2>&1 || { cat /tmp/atspi-dl.log >&2; exit 1; }
pacman -U --noconfirm /tmp/at-spi2-core-2.52.0-1-x86_64.pkg.tar.zst \
    >/tmp/atspi-downgrade.log 2>&1 || { cat /tmp/atspi-downgrade.log >&2; exit 1; }
pacman -Q at-spi2-core

# the winit X11 client stack + the cursor/input libs + a font (the
# software renderer needs at least one font for the western text)
pacman -S --noconfirm --needed \
    libx11 libxcb libxkbcommon libxkbcommon-x11 \
    libxcursor libxi libxrandr libxinerama libxft \
    xorg-xrandr xdotool ttf-dejavu \
    >/tmp/gui-install.log 2>&1 || { cat /tmp/gui-install.log >&2; exit 1; }

echo "fixture gui-integration: winit X11 client stack ready (at-spi2-core $(pacman -Qq at-spi2-core))"
