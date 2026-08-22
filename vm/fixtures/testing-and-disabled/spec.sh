#!/usr/bin/env bash
#
# testing-and-disabled — section registration edge cases:
#   [testing]      ENABLED section with packages -> SKIPPED (alpm_utils skips
#                  exactly the lowercased name "testing")
#   [core-testing] -> registered (only the EXACT name "testing" is skipped)
#   [disabled-repo] commented out -> not in the parsed file -> not registered
# The sync dbs are placed directly into /var/lib/pacman/sync/ (no pacman -Sy:
# a real pacman would also accept these, but the point is the oracle's mINI
# registration, which reads the db files only).
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

# --- [testing] repo: a fake kernel that MUST NOT appear ---
TEST_REPO=/srv/testing-fixtures
mkdir -p "$TEST_REPO"
chown -R test:test "$TEST_REPO"
build_fakepkg linux-cachyos-secret 9.9.9 1
# move the artifact out of the fixtures repo into the testing repo
mv /srv/cachyos-km-fixtures/*.pkg.tar.zst "$TEST_REPO/"
(cd "$TEST_REPO" && repo-add -q -R testing.db.tar.zst *.pkg.tar.zst)

# --- [core-testing] repo: a fake kernel that MUST appear as core-testing/ ---
CT_REPO=/srv/ct-fixtures
mkdir -p "$CT_REPO"
chown -R test:test "$CT_REPO"
build_fakepkg linux-cachyos-ct 9.9.9 1
build_fakepkg linux-cachyos-ct-headers 9.9.9 1
mv /srv/cachyos-km-fixtures/*.pkg.tar.zst "$CT_REPO/"
(cd "$CT_REPO" && repo-add -q -R core-testing.db.tar.zst *.pkg.tar.zst)

cat >> /etc/pacman.conf <<EOF

[testing]
Server = file://$TEST_REPO
SigLevel = Never

[core-testing]
Server = file://$CT_REPO
SigLevel = Never

#[disabled-repo]
#Server = file:///srv/never
#SigLevel = Never
EOF

# place the db files directly (mINI registration reads /var/lib/pacman/sync)
cp "$TEST_REPO/testing.db" /var/lib/pacman/sync/testing.db
cp "$CT_REPO/core-testing.db" /var/lib/pacman/sync/core-testing.db
echo "fixture testing-and-disabled: testing skipped, core-testing registered"
