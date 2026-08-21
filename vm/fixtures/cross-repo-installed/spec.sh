#!/usr/bin/env bash
#
# cross-repo-installed — the fake kernel is installed FROM `[fixtures]` while
# a second repo `[other]` also carries it (different version). The oracle's
# row for `other/linux-cachyos-court` must be present-but-mutable-and-
# unchecked (installed_db "fixtures" != repo "other"), while
# `fixtures/linux-cachyos-court` is installed + immutable + checked.
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

# a second file-based repo with a DIFFERENT version of the same kernel
OTHER_REPO=/srv/other-fixtures
mkdir -p "$OTHER_REPO"
chown -R test:test "$OTHER_REPO"
cat >> /etc/pacman.conf <<EOF

[other]
Server = file://$OTHER_REPO
SigLevel = Never
EOF

build_fakepkg linux-cachyos-court 9.9.9 1
build_fakepkg linux-cachyos-court-headers 9.9.9 1
# build the 9.5.5 variant into the other repo (artifact path differs)
FIXTURES_REPO_DIR="$OTHER_REPO" build_fakepkg linux-cachyos-court 9.5.5 1
FIXTURES_REPO_DIR="$OTHER_REPO" build_fakepkg linux-cachyos-court-headers 9.5.5 1

repo_add_all
(cd "$OTHER_REPO" && repo-add -q -R cachyos-km-fixtures.db.tar.zst *.pkg.tar.zst)
pacman_sync
# install the NEWER version from the fixtures repo (provenance: fixtures)
pacman -S --noconfirm fixtures/linux-cachyos-court fixtures/linux-cachyos-court-headers >/dev/null
echo "fixture cross-repo-installed: installed from fixtures, present in other"
