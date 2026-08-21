#!/usr/bin/env bash
#
# empty-sync-db — a repo whose sync database EXISTS but contains zero
# packages: the section registers, the search yields nothing, no dialog
# (other repos still have kernels).
#
# NOTE: `repo-add` with no package files creates NOTHING (verified), so the
# empty db is built by hand: an uncompressed empty tar named `$repo.db`.
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
(cd "$EMPTY_REPO" && tar -cf emptyrepo.db -T /dev/null)
pacman -Sy --noconfirm >/dev/null
echo "fixture empty-sync-db: [emptyrepo] with zero packages"
