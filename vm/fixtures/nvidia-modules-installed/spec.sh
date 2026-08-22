#!/usr/bin/env bash
#
# nvidia-modules-installed — NO chwd profile, but `linux-cachyos-court2-nvidia`
# is already installed, so the install-time probe
# `pacman -Qqs '^linux-cachyos.*-nvidia$'` (kernel.cpp:114) is non-empty and
# the oracle takes the "modules already installed -> reuse them, skipping
# chwd detection" branch (kernel.cpp:112-126).
#
# Expected install command (courted):
#   pacman -S --needed linux-cachyos-court2-nvidia \
#                      linux-cachyos-court2 linux-cachyos-court2-headers
# (reason: ExistingModuleFamily)
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

# no chwd profiles: the wrapper prints a Name-less table (grep finds nothing)
install_chwd_wrapper

build_fakepkg linux-cachyos-court2 9.9.9 1
build_fakepkg linux-cachyos-court2-headers 9.9.9 1
build_fakepkg linux-cachyos-court2-nvidia 9.9.9 1
build_fakepkg linux-cachyos-court2-nvidia-open 9.9.9 1

repo_add_all
pacman_sync
# install ONLY the prebuilt module package (the kernel itself stays
# uninstalled so the court2 row is an INSTALL candidate)
install_pkg_file /tmp/fakepkg-linux-cachyos-court2-nvidia-9.9.9-1-any.pkg.tar.zst
install_xterm
echo "fixture nvidia-modules-installed: prebuilt nvidia module installed, no chwd profile"
