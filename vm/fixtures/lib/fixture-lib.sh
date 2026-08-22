#!/usr/bin/env bash
#
# fixture-lib.sh — helpers sourced by fixture bake scripts (run inside the
# chroot via arch-chroot). Builds fake packages into the fixtures repo and
# syncs it.
#
# The fixtures repo (/srv/cachyos-km-fixtures) is a FILE-based pacman repo.
# Because discovery registers every non-options/testing pacman.conf section
# as a sync database, the `[fixtures]` repo is itself observable by BOTH the
# oracle and the candidate — which is exactly what the custom-repo,
# duplicate-across-repos and upgrade/downgrade courts rely on.
#
set -euo pipefail

FIXTURES_REPO_DIR="${FIXTURES_REPO_DIR:-/srv/cachyos-km-fixtures}"
FIXTURES_REPO_NAME="fixtures"

ensure_fixtures_repo() {
    mkdir -p "$FIXTURES_REPO_DIR"
    chown -R test:test "$FIXTURES_REPO_DIR"
    if ! grep -q "^\[$FIXTURES_REPO_NAME\]" /etc/pacman.conf; then
        cat >> /etc/pacman.conf <<EOF

[$FIXTURES_REPO_NAME]
Server = file://$FIXTURES_REPO_DIR
SigLevel = Never
EOF
    fi
}

# build_fakepkg <name> <pkgver> <pkgrel> [epoch]
build_fakepkg() {
    local name="$1" ver="$2" rel="$3" epoch="${4:-0}"
    # the repo dir must exist BEFORE the artifact install below (the dir is
    # otherwise only created by repo_add_all -> ensure_fixtures_repo, and
    # install to a missing dir with a trailing slash fails "Not a directory").
    # NOTE: only the DIRECTORY is ensured here — the pacman.conf [fixtures]
    # section is added by ensure_fixtures_repo (repo_add_all) so fixtures
    # with a custom section name can control the conf themselves.
    mkdir -p "$FIXTURES_REPO_DIR"
    chown -R test:test "$FIXTURES_REPO_DIR"
    local work="/tmp/fakepkg-$name"
    rm -rf "$work"
    mkdir -p "$work"
    cat > "$work/PKGBUILD" <<EOF
pkgname=$name
pkgver=$ver
pkgrel=$rel
epoch=$epoch
pkgdesc="cachyos-km court fixture (fake package)"
arch=('any')
url="https://example.invalid"
license=('MIT')
package() {
    install -d "\$pkgdir/usr/share/cachyos-km-fixtures"
    echo "$name $ver-$rel" > "\$pkgdir/usr/share/cachyos-km-fixtures/$name"
}
EOF
    # makepkg refuses to run as root; build as the `test` user
    chown -R test:test "$work"
    (cd "$work" && sudo -u test makepkg -f --noconfirm --skipinteg)
    # copy the artifact to the repo dir, and keep a PRIVATE copy under a
    # deterministic `fakepkg-` prefix in /tmp for -U installs of OLDER
    # versions than the synced one (the specs reference these paths)
    local artifact
    artifact="$(ls "$work"/*.pkg.tar.zst 2>/dev/null | head -1)"
    [ -n "$artifact" ] || { echo "build_fakepkg: no artifact for $name" >&2; exit 1; }
    install -o test -g test -m 644 "$artifact" "$FIXTURES_REPO_DIR/"
    cp "$artifact" "/tmp/fakepkg-$(basename "$artifact")"
}

repo_add_all() {
    ensure_fixtures_repo
    # the db must be named after the REPO (`$repo.db` is what pacman fetches
    # from the Server URL) — repo-add's filename argument is arbitrary
    (cd "$FIXTURES_REPO_DIR" && repo-add -q -R "$FIXTURES_REPO_NAME.db.tar.zst" *.pkg.tar.zst)
}

pacman_sync() {
    pacman -Sy --noconfirm >/dev/null
}

# install a fake package from /tmp by file (bypasses the repo: the local
# provenance is then "unknown"/external, which courts the
# installed-from-other-repo semantics)
install_pkg_file() {
    local file="$1"
    pacman -U --noconfirm "$file" >/dev/null
}
