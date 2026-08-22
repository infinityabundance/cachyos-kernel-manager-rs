#!/usr/bin/env bash
#
# epoch-versions — fake kernels exercising epoch and unusual Arch version
# syntax in the display, plus upgrade/downgrade markers across epochs:
#   linux-cachyos-epoch    epoch 1, 9.9.9-1   -> "1:9.9.9-1" (plain)
#   linux-cachyos-beta     2.9.9.beta-1       -> unusual pkgver segment
#   linux-cachyos-gitrel   1.0-1.2            -> dotted pkgrel
#   linux-cachyos-eup      local 1:9.8.8-1 < sync 1:9.9.9-1 -> "∧1:9.9.9-1"
#   linux-cachyos-edown    local 2:9.9.9-1 > sync 1:9.9.9-1 -> "∨2:9.9.9-1"
# Both sides use libalpm's alpm_pkg_vercmp (epoch-aware), never semver.
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

# plain epoch display
build_fakepkg linux-cachyos-epoch 9.9.9 1 1
build_fakepkg linux-cachyos-epoch-headers 9.9.9 1 1
# unusual pkgver segment
build_fakepkg linux-cachyos-beta 2.9.9.beta 1
build_fakepkg linux-cachyos-beta-headers 2.9.9.beta 1
# dotted pkgrel
build_fakepkg linux-cachyos-gitrel 1.0 1.2
build_fakepkg linux-cachyos-gitrel-headers 1.0 1.2
# upgrade pair across equal epochs (1:9.8.8 installed, 1:9.9.9 synced)
# NOTE: makepkg puts the EPOCH in the artifact filename
# (linux-cachyos-eup-1:9.8.8-1-any.pkg.tar.zst), so the -U paths below
# carry the epoch prefix.
build_fakepkg linux-cachyos-eup 9.8.8 1 1
build_fakepkg linux-cachyos-eup-headers 9.8.8 1 1
build_fakepkg linux-cachyos-eup 9.9.9 1 1
build_fakepkg linux-cachyos-eup-headers 9.9.9 1 1
repo_add_all
pacman_sync
install_pkg_file /tmp/fakepkg-linux-cachyos-eup-1:9.8.8-1-any.pkg.tar.zst
install_pkg_file /tmp/fakepkg-linux-cachyos-eup-headers-1:9.8.8-1-any.pkg.tar.zst
# downgrade pair across different epochs: INSTALL 2:9.9.9-1 (built first,
# installed by file), then rebuild the repo with 1:9.9.9-1 (the artifact
# filename CARRIES the epoch, so the 2: file must be removed from the repo
# before repo-add — otherwise the sync db would keep the NEWER 2: version)
build_fakepkg linux-cachyos-edown 9.9.9 1 2
build_fakepkg linux-cachyos-edown-headers 9.9.9 1 2
install_pkg_file /tmp/fakepkg-linux-cachyos-edown-2:9.9.9-1-any.pkg.tar.zst
install_pkg_file /tmp/fakepkg-linux-cachyos-edown-headers-2:9.9.9-1-any.pkg.tar.zst
build_fakepkg linux-cachyos-edown 9.9.9 1 1
build_fakepkg linux-cachyos-edown-headers 9.9.9 1 1
rm -f /srv/cachyos-km-fixtures/linux-cachyos-edown-2:9.9.9-1-any.pkg.tar.zst
rm -f /srv/cachyos-km-fixtures/linux-cachyos-edown-headers-2:9.9.9-1-any.pkg.tar.zst
repo_add_all
pacman_sync
echo "fixture epoch-versions: epoch + unusual syntax + cross-epoch markers"
