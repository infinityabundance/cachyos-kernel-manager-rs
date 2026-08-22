#!/usr/bin/env bash
#
# update-available-execute — court2 installed at 1.0-1 FROM the [fixtures]
# repo (provenance == repo -> row immutable + checked), then the repo is
# bumped to 2.0-1 and the sync db refreshed. The row now shows the update
# marker and `update_available` is set (kernel.cpp:56-79).
#
# UNCHECKING the row (the court action) puts it in the change list; the
# install phase sees `is_update_available()` -> install() is called AND the
# removal phase sees is_installed() -> remove() is called (km-window.cpp:
# 48-71). The oracle therefore commits BOTH:
#
#   pacman -S --needed linux-cachyos-court2 linux-cachyos-court2-headers
#   pacman -Rsn linux-cachyos-court2 linux-cachyos-court2-headers
#
# (install first, then removal — kernel.cpp:288-304). This is the oracle's
# "upgrade quirk": the same kernel lands in both lists.
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

install_chwd_wrapper
install_findmnt_wrapper "ext4"

# 1) initial repo state + install from the repo (provenance == fixtures)
build_fakepkg linux-cachyos-court2 1.0 1
build_fakepkg linux-cachyos-court2-headers 1.0 1
repo_add_all
pacman_sync
pacman -S --noconfirm linux-cachyos-court2 linux-cachyos-court2-headers >/dev/null

# 2) bump the repo + refresh the sync db (local stays at 1.0-1)
build_fakepkg linux-cachyos-court2 2.0 1
build_fakepkg linux-cachyos-court2-headers 2.0 1
repo_add_all
pacman_sync
install_xterm
echo "fixture update-available-execute: installed 1.0-1, sync 2.0-1 (update quirk)"
