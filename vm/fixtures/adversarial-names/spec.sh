#!/usr/bin/env bash
#
# adversarial-names — discovery needle adversarial cases in the [fixtures]
# repo:
#   linux-fake-headers         headers WITHOUT the kernel package  -> skipped
#   linux-angel + -headers     normal kernel                        -> discovered
#   linux-demon                kernel WITHOUT headers               -> invisible
#   linux-cachyos-fake + ...   cachyos-prefixed kernel (companions   -> discovered
#                              looked up but absent)
# The oracle's discovery is DRIVEN BY headers packages (needle
# `linux[^ ]*-headers`): a kernel without headers never appears, and a
# headers package without a kernel is skipped (kernel.cpp:198-213).
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

build_fakepkg linux-fake-headers 9.9.9 1          # no linux-fake kernel
build_fakepkg linux-angel 9.9.9 1
build_fakepkg linux-angel-headers 9.9.9 1
build_fakepkg linux-demon 9.9.9 1                 # no headers package
build_fakepkg linux-cachyos-fake 9.9.9 1
build_fakepkg linux-cachyos-fake-headers 9.9.9 1
repo_add_all
pacman_sync
echo "fixture adversarial-names: fake/angel/demon/cachyos-fake"
