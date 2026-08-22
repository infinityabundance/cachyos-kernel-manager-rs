#!/usr/bin/env bash
#
# case-sensitivity — a repo section written with a capital letter:
#   [Fixtures] with a db named fixtures.db
# mINI lowercases section names (ini.hpp INIMap::operator[]), so the oracle
# registers "fixtures" and discovers /var/lib/pacman/sync/fixtures.db.
# A real pacman would register "Fixtures" and look for "Fixtures.db" —
# a captured mINI-vs-pacman difference (the oracle does not use pacman's
# parser). No pacman -Sy here: it would fetch "Fixtures.db" and fail.
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

# build into a repo dir WITHOUT adding any pacman.conf section; the section
# is added manually as [Fixtures] (capitalized) below
REPO=/srv/fixtures-repo
FIXTURES_REPO_DIR="$REPO" build_fakepkg linux-cachyos-case 9.9.9 1
FIXTURES_REPO_DIR="$REPO" build_fakepkg linux-cachyos-case-headers 9.9.9 1
(cd "$REPO" && repo-add -q -R fixtures.db.tar.zst *.pkg.tar.zst)

# a CAPITALIZED section pointing at the (lowercase) db files
cat >> /etc/pacman.conf <<EOF

[Fixtures]
Server = file://$REPO
SigLevel = Never
EOF
# the oracle registers "fixtures" (mINI lowercases) and reads the lowercase
# db; a real pacman would register "Fixtures" and look for "Fixtures.db"
cp "$REPO/fixtures.db" /var/lib/pacman/sync/fixtures.db
echo "fixture case-sensitivity: [Fixtures] registered as fixtures"
