#!/usr/bin/env bash
#
# removal-plan — court2 + headers + zfs + nvidia all installed locally (via
# -U file installs, provenance unknown -> rows immutable + checked). The
# court UNCHECKS court2, which must produce:
#
#   pacman -Rsn linux-cachyos-court2 linux-cachyos-court2-headers \
#               linux-cachyos-court2-zfs linux-cachyos-court2-nvidia
#
# (kernel first, then installed companions in headers/zfs/nvidia/nvidia-open
# order, kernel.cpp:137-163). nvidia-open is NOT installed -> absent.
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

install_chwd_wrapper
install_findmnt_wrapper "ext4"

build_fakepkg linux-cachyos-court2 9.9.9 1
build_fakepkg linux-cachyos-court2-headers 9.9.9 1
build_fakepkg linux-cachyos-court2-zfs 9.9.9 1
build_fakepkg linux-cachyos-court2-nvidia 9.9.9 1
build_fakepkg linux-cachyos-court2-nvidia-open 9.9.9 1

repo_add_all
pacman_sync
for p in linux-cachyos-court2 linux-cachyos-court2-headers \
         linux-cachyos-court2-zfs linux-cachyos-court2-nvidia; do
    install_pkg_file /tmp/fakepkg-$p-9.9.9-1-any.pkg.tar.zst
done
install_xterm
echo "fixture removal-plan: court2 + headers + zfs + nvidia installed"
