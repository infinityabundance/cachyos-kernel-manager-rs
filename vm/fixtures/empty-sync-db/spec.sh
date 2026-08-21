#!/usr/bin/env bash
#
# empty-sync-db — a repo whose sync database EXISTS but contains zero
# packages (repo-add on an empty directory): the section registers, the
# search yields nothing, no dialog (other repos still have kernels).
#
set -euo pipefail
EMPTY_REPO=/srv/empty-fixtures
mkdir -p "$EMPTY_REPO"
chown -R test:test "$EMPTY_REPO"
cat >> /etc/pacman.conf <<EOF

[emptyrepo]
Server = file://$EMPTY_REPO
SigLevel = Never
EOF
(cd "$EMPTY_REPO" && repo-add -q -R cachyos-km-fixtures.db.tar.zst) >/dev/null 2>&1 || true
pacman -Sy --noconfirm >/dev/null
echo "fixture empty-sync-db: [emptyrepo] with zero packages"
