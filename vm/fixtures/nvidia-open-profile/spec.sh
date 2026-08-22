#!/usr/bin/env bash
#
# nvidia-open-profile — chwd reports an `nvidia-open-dkms` profile (the
# oracle's `is_nvidia_card_prebuild_open_module`, kernel.cpp:49-52); the
# repo carries court2 + headers + nvidia + nvidia-open; NO dkms package
# installed, NO prebuilt module installed, root not ZFS.
#
# Expected install command (courted):
#   pacman -S --needed linux-cachyos-court2-nvidia-open \
#                      linux-cachyos-court2 linux-cachyos-court2-headers
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

install_chwd_wrapper "nvidia-open-dkms"

build_fakepkg linux-cachyos-court2 9.9.9 1
build_fakepkg linux-cachyos-court2-headers 9.9.9 1
build_fakepkg linux-cachyos-court2-nvidia 9.9.9 1
build_fakepkg linux-cachyos-court2-nvidia-open 9.9.9 1

repo_add_all
pacman_sync
install_xterm
echo "fixture nvidia-open-profile: chwd=nvidia-open-dkms, no dkms/modules installed"
