#!/usr/bin/env bash
#
# makepkg-runtime — gap-006 witness: the RUNTIME dependency-resolution
# semantics of the oracle's build commands (conf-window.cpp:734
# `makepkg -scf --cleanbuild --skipchecksums` vs aur_kernel.cpp:53
# `makepkg -sicf --cleanbuild --skipchecksums`).
#
# The command CONSTRUCTION is courted by build-env/lifecycle +
# aur/enablement-matrix (byte-identical argv); THIS fixture enables the
# runtime witness:
#   - `km-runtime-dep` in the fixtures repo — the -s (--syncdeps) resolution
#     target: makepkg -s must install it via `pacman -S --asdeps`;
#   - passwordless sudo for the `test` user — makepkg -s (as a non-root
#     user) internally runs `sudo pacman` for the dep install, and -i runs
#     `sudo pacman -U` for the built package;
#   - a build project /home/test/build-proj with depends=('km-runtime-dep')
#     (the -scf scenario: deps resolved, NOT installed),
#   - a second project /home/test/aur-proj with depends=('km-aur-only-dep')
#     — a dep that exists NOWHERE (the -s failure mode, identical for -scf
#     and -sicf).
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

ensure_fixtures_repo
build_fakepkg km-runtime-dep 1.0.0 1
repo_add_all
pacman_sync

# passwordless sudo for the test user (makepkg -s / -i internally sudo).
# mkfs.ext4 -d strips setuid bits from the baked tree — restore sudo's.
chmod 4755 /usr/bin/sudo
cat > /etc/sudoers.d/cachyos-km-test <<'EOF'
test ALL=(ALL) NOPASSWD: ALL
EOF
chmod 440 /etc/sudoers.d/cachyos-km-test

# the -scf / -sicf build project (a repo-resolvable dep)
chown test:test /home/test
chmod 700 /home/test
mkdir -p /home/test/build-proj
chown -R test:test /home/test/build-proj
cat > /home/test/build-proj/PKGBUILD <<'EOF'
# Maintainer: court fixture (gap-006 runtime witness)
pkgname=km-runtime-kernel
pkgver=1.0.0
pkgrel=1
pkgdesc="court fixture: -s resolves km-runtime-dep from the repo"
arch=('any')
url="https://example.invalid"
license=('MIT')
depends=('km-runtime-dep')
build() {
    echo "building km-runtime-kernel"
}
package() {
    install -d "$pkgdir/usr/share/cachyos-km-fixtures"
    echo "km-runtime-kernel" > "$pkgdir/usr/share/cachyos-km-fixtures/km-runtime-kernel"
}
EOF

# the AUR-only-dep project (the -s failure mode)
mkdir -p /home/test/aur-proj
chown -R test:test /home/test/aur-proj
cat > /home/test/aur-proj/PKGBUILD <<'EOF'
# Maintainer: court fixture (gap-006: an AUR-only dep is NOT resolvable by
# makepkg -s — makepkg can only install deps from the sync repos)
pkgname=km-aur-only
pkgver=1.0.0
pkgrel=1
pkgdesc="court fixture: depends on a package that exists nowhere"
arch=('any')
url="https://example.invalid"
license=('MIT')
depends=('km-aur-only-dep')
build() {
    echo "building km-aur-only"
}
package() {
    install -d "$pkgdir/usr/share/cachyos-km-fixtures"
    echo "km-aur-only" > "$pkgdir/usr/share/cachyos-km-fixtures/km-aur-only"
}
EOF

echo "fixture makepkg-runtime: km-runtime-dep + build-proj + aur-proj (gap-006)"
