#!/usr/bin/env bash
#
# zfs-root — `findmnt -ln -o FSTYPE /` answers `zfs` (the oracle's
# `is_root_on_zfs`, kernel.cpp:41) via a /usr/local/bin/findmnt wrapper; the
# repo carries court2 + headers + zfs. No chwd profiles, no dkms/modules.
#
# Expected install command (courted):
#   pacman -S --needed linux-cachyos-court2-zfs \
#                      linux-cachyos-court2 linux-cachyos-court2-headers
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

install_findmnt_wrapper "zfs"
install_chwd_wrapper

build_fakepkg linux-cachyos-court2 9.9.9 1
build_fakepkg linux-cachyos-court2-headers 9.9.9 1
build_fakepkg linux-cachyos-court2-zfs 9.9.9 1

repo_add_all
pacman_sync
install_xterm
echo "fixture zfs-root: findmnt=zfs, zfs companion present"
