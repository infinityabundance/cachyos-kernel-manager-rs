#!/usr/bin/env bash
#
# build-mutation — the Configure window's PKGBUILD mutation surfaces
# (patch-injection + custom-name courts). Same base as git-cache (the
# oracle runs as ROOT: HOME=/root, cache at /root/.cache/cachyos-km) but:
#   - the remote is NOT ahead (refresh is a no-op -> the PKGBUILD is stable
#     across the Configure/Build prepare_build_environment calls),
#   - the fake PKGBUILD carries every surface on_execute touches:
#     _major= (pkgbase insertion point), prepare() (source-array insertion
#     point), package_<suffix> functions (artifact globs), and a source
#     array with a .patch entry (the patches tab's initial list),
#   - the repo mirrors the REAL linux-cachyos repo: the cachyos variant
#     PKGBUILD lives in a `linux-cachyos/` subdirectory (the oracle's
#     relative `linux-cachyos/PKGBUILD` paths resolve against the cache,
#     D-004).
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

REMOTE=/root/cachyos-km-remote.git
CACHE=/root/.cache/cachyos-km
PKGBUILDS=$CACHE/pkgbuilds

git init -q --bare "$REMOTE"
git -C "$REMOTE" symbolic-ref HEAD refs/heads/master

mkdir -p /tmp/pkgsrc/linux-cachyos
cat > /tmp/pkgsrc/linux-cachyos/PKGBUILD <<'EOF'
# Maintainer: court fixture
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

git init -q /tmp/pkgsrc
(cd /tmp/pkgsrc && git add linux-cachyos/PKGBUILD \
    && git -c user.name=fixture -c user.email=fixture@invalid commit -qm init \
    && git branch -M master \
    && git remote add origin "$REMOTE" \
    && git push -q origin master)

# the checkout; the remote stays AT this commit so `git pull` is a no-op
git clone -q "$REMOTE" "$PKGBUILDS"

# sanity: git repo on master, clean, pull is a no-op, and the variant
# subdir the oracle accesses exists
(cd "$PKGBUILDS" && git rev-parse --is-inside-work-tree >/dev/null \
    && git branch --show-current | grep -q '^master$' \
    && [ -z "$(git status --porcelain)" ] \
    && [ "$(git rev-parse HEAD)" = "$(git rev-parse origin/master)" ] \
    && [ -f linux-cachyos/PKGBUILD ])

echo "fixture build-mutation: stable /root/.cache/cachyos-km/pkgbuilds checkout with mutation-surface PKGBUILD"
