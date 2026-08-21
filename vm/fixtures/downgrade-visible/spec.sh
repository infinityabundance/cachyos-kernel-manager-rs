#!/usr/bin/env bash
#
# downgrade-visible — a fake kernel whose INSTALLED version is NEWER than
# the sync version: local 9.9.9 > sync 9.8.8 -> the oracle renders
# "∨9.9.9" (downgrade marker, no update flag).
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

build_fakepkg linux-cachyos-court 9.9.9 1        # artifact -> /tmp
build_fakepkg linux-cachyos-court-headers 9.9.9 1
build_fakepkg linux-cachyos-court 9.8.8 1        # newest in sync db
build_fakepkg linux-cachyos-court-headers 9.8.8 1
repo_add_all
pacman_sync
install_pkg_file /tmp/fakepkg-linux-cachyos-court-9.9.9-1-any.pkg.tar.zst
install_pkg_file /tmp/fakepkg-linux-cachyos-court-headers-9.9.9-1-any.pkg.tar.zst
echo "fixture downgrade-visible: local 9.9.9 > sync 9.8.8"
