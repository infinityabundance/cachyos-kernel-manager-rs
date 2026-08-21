#!/usr/bin/env bash
#
# duplicate-across-repos — the same kernel NAME in two sync repos with
# DIFFERENT versions: discovery must yield two rows
# (fixtures/linux-cachyos-court and other/linux-cachyos-court), neither
# installed.
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

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
FIXTURES_REPO_DIR="$OTHER_REPO" build_fakepkg linux-cachyos-court 9.5.5 1
FIXTURES_REPO_DIR="$OTHER_REPO" build_fakepkg linux-cachyos-court-headers 9.5.5 1

repo_add_all
(cd "$OTHER_REPO" && repo-add -q -R other.db.tar.zst *.pkg.tar.zst)
pacman_sync
echo "fixture duplicate-across-repos: two rows, none installed"
