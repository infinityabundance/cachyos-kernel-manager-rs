#!/usr/bin/env bash
#
# duplicated-sections — the [fixtures] section appears TWICE in pacman.conf.
# mINI's INIMap merges duplicated sections into the FIRST position (the
# second occurrence's keys update the same entry), so the oracle registers
# the repo ONCE and every kernel appears exactly once. (Real pacman ERRORS
# on duplicated repositories — a captured mINI-vs-pacman difference.) The
# db is placed directly into /var/lib/pacman/sync/ (no pacman -Sy: real
# pacman would refuse the duplicated section).
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

build_fakepkg linux-cachyos-dupe 9.9.9 1
build_fakepkg linux-cachyos-dupe-headers 9.9.9 1
repo_add_all
# duplicate the section with a DIFFERENT Server value
cat >> /etc/pacman.conf <<EOF

[fixtures]
Server = file:///srv/wrong-location
SigLevel = Never
EOF
cp "$FIXTURES_REPO_DIR/fixtures.db" /var/lib/pacman/sync/fixtures.db
echo "fixture duplicated-sections: [fixtures] declared twice, registered once"
