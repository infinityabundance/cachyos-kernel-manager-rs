#!/usr/bin/env bash
#
# nvidia-dkms-profile — chwd reports an `nvidia-dkms` profile (the oracle's
# `is_nvidia_card_prebuild_module`, kernel.cpp:44-47); the repo carries
# linux-cachyos-court2 + headers + nvidia + nvidia-open + zfs; NO dkms
# package installed, NO prebuilt module installed, root not ZFS.
#
# Expected install command (courted):
#   pacman -S --needed linux-cachyos-court2-nvidia \
#                      linux-cachyos-court2 linux-cachyos-court2-headers
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

install_chwd_wrapper "nvidia-dkms"

build_fakepkg linux-cachyos-court2 9.9.9 1
build_fakepkg linux-cachyos-court2-headers 9.9.9 1
build_fakepkg linux-cachyos-court2-nvidia 9.9.9 1
build_fakepkg linux-cachyos-court2-nvidia-open 9.9.9 1
build_fakepkg linux-cachyos-court2-zfs 9.9.9 1

repo_add_all
pacman_sync
install_xterm
echo "fixture nvidia-dkms-profile: chwd=nvidia-dkms, companions present, no dkms/modules installed"
