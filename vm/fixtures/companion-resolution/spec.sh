#!/usr/bin/env bash
#
# companion-resolution — fake cachyos-style kernels with varied companion
# presence in the [fixtures] repo (kernel.cpp:226-234 companion lookups are
# SAME-db and presence-filtered):
#   linux-cachyos-court2 + -zfs + -nvidia + -nvidia-open   (all present)
#   linux-cachyos-court3                                    (no companions)
#   linux-cachyos-court4 + -zfs                             (zfs only)
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

build_fakepkg linux-cachyos-court2 9.9.9 1
build_fakepkg linux-cachyos-court2-headers 9.9.9 1
build_fakepkg linux-cachyos-court2-zfs 9.9.9 1
build_fakepkg linux-cachyos-court2-nvidia 9.9.9 1
build_fakepkg linux-cachyos-court2-nvidia-open 9.9.9 1

build_fakepkg linux-cachyos-court3 9.9.9 1
build_fakepkg linux-cachyos-court3-headers 9.9.9 1

build_fakepkg linux-cachyos-court4 9.9.9 1
build_fakepkg linux-cachyos-court4-headers 9.9.9 1
build_fakepkg linux-cachyos-court4-zfs 9.9.9 1

repo_add_all
pacman_sync
echo "fixture companion-resolution: court2 all, court3 none, court4 zfs"
