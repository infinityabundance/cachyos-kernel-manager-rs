#!/usr/bin/env bash
#
# git-cache — the oracle's Configure flow calls prepare_build_environment
# (utils.cpp:198-202) which runs prepare_git_repo (utils.cpp:161-196)
# against HOME=<root's home>. The oracle runs as ROOT in the court VM, so
# the cache is /root/.cache/cachyos-km/pkgbuilds.
#
# The fixture pre-creates the checkout as a git repo whose origin is a
# LOCAL bare remote (/root/cachyos-km-remote.git), so the refresh chain
# (`git checkout --force master`, `git clean -fd`, `git pull`) runs fully
# OFFLINE and is witnessed by strace. The fake PKGBUILD carries the
# surfaces the Configure flow touches afterwards (reset_patches_data_tab:
# _major=, prepare(), package_<suffix> functions, a source array with a
# .patch entry).
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

REMOTE=/root/cachyos-km-remote.git
CACHE=/root/.cache/cachyos-km
PKGBUILDS=$CACHE/pkgbuilds

git init -q --bare "$REMOTE"

mkdir -p /tmp/pkgsrc
cat > /tmp/pkgsrc/PKGBUILD <<'EOF'
_major=6
_minor=14
pkgname=linux-cachyos
pkgver=6.14.1
pkgrel=3
pkgdesc="court fixture PKGBUILD"
arch=('x86_64')
url="https://example.invalid"
license=('GPL2')
source=("https://example.invalid/linux-cachyos.tar.gz" "patches/foo.patch")
prepare() {
    true
}
package_linux-cachyos() {
    true
}
package_linux-cachyos-headers() {
    true
}
EOF

# seed the bare remote from a work clone (single master commit)
git init -q /tmp/pkgsrc
(cd /tmp/pkgsrc && git add PKGBUILD \
    && git -c user.name=fixture -c user.email=fixture@invalid commit -qm init \
    && git branch -M master \
    && git remote add origin "$REMOTE" \
    && git push -q origin master)

# the checkout the oracle's refresh chain will operate on
git clone -q "$REMOTE" "$PKGBUILDS"
# advance the remote past the clone so `git pull` has a real (fast-forward)
# refresh to perform — the strongest witness: checkout+clean+pull all
# actually execute their logic, not a no-op
(cd /tmp/pkgsrc && echo "# refreshed" >> PKGBUILD \
    && git add PKGBUILD \
    && git -c user.name=fixture -c user.email=fixture@invalid commit -qm refresh \
    && git push -q origin master)

# sanity: the checkout must be a git repo on master, clean, pullable
(cd "$PKGBUILDS" && git rev-parse --is-inside-work-tree >/dev/null \
    && git branch --show-current | grep -q '^master$' \
    && [ -z "$(git status --porcelain)" ])

echo "fixture git-cache: /root/.cache/cachyos-km/pkgbuilds checkout seeded (remote ahead by one commit)"
