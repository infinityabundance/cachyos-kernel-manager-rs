#!/usr/bin/env bash
#
# nvidia-dkms-installed — chwd reports an `nvidia-dkms` profile AND the
# `nvidia-dkms` package is installed locally, so
# `dkms_modules_not_installed` is false and NO prebuilt nvidia companion is
# added (kernel.cpp:102-110,128-132). Root not ZFS, no modules installed.
#
# Expected install command (courted):
#   pacman -S --needed linux-cachyos-court2 linux-cachyos-court2-headers
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

install_chwd_wrapper "nvidia-dkms"

build_fakepkg linux-cachyos-court2 9.9.9 1
build_fakepkg linux-cachyos-court2-headers 9.9.9 1
build_fakepkg linux-cachyos-court2-nvidia 9.9.9 1
build_fakepkg linux-cachyos-court2-nvidia-open 9.9.9 1
build_fakepkg nvidia-dkms 1.0 1

repo_add_all
pacman_sync
# install the dkms package by file (provenance "unknown" is irrelevant here)
install_pkg_file /tmp/fakepkg-nvidia-dkms-1.0-1-any.pkg.tar.zst
install_xterm
echo "fixture nvidia-dkms-installed: chwd=nvidia-dkms but nvidia-dkms installed -> no companion"
