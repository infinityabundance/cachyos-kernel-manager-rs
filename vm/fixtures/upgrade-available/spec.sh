#!/usr/bin/env bash
#
# upgrade-available — a fake kernel whose SYNC version is newer than the
# INSTALLED version: local 9.8.8 < sync 9.9.9 -> the oracle renders
# "∧9.9.9" and marks update_available.
#
# Mechanics: build both versions (the 9.8.8 artifact is kept in /tmp for the
# -U install), repo-add keeps only the newest (9.9.9) in the sync db, sync,
# then install 9.8.8 by FILE (installed provenance = unknown -> the row is
# installed + immutable + checked, exactly the oracle's upgrade path).
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

build_fakepkg linux-cachyos-court 9.8.8 1        # artifact -> /tmp
build_fakepkg linux-cachyos-court-headers 9.8.8 1
build_fakepkg linux-cachyos-court 9.9.9 1        # newest -> sync db
build_fakepkg linux-cachyos-court-headers 9.9.9 1
repo_add_all
pacman_sync
install_pkg_file /tmp/fakepkg-linux-cachyos-court-9.8.8-1-any.pkg.tar.zst
install_pkg_file /tmp/fakepkg-linux-cachyos-court-headers-9.8.8-1-any.pkg.tar.zst
echo "fixture upgrade-available: local 9.8.8 < sync 9.9.9"
